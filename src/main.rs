mod db;
mod epub_gen;
mod feed;
#[cfg(feature = "mem_opt")]
mod image;
#[cfg(not(feature = "mem_opt"))]
#[path = "image_inmem.rs"]
mod image;
mod opds;
mod processor;
mod scheduler;
mod epub_message;

use crate::db::Feed;
use axum::{
    extract::{Json, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post},
    Router,
};
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tower_http::services::ServeDir;
use tracing::info;

struct AppState {
    db: Arc<Mutex<rusqlite::Connection>>,
}

#[derive(Deserialize)]
struct GenerateRequest {
    #[serde(default)]
    feeds: Vec<Feed>,
}

#[tokio::main]
async fn main() {
    #[cfg(feature = "mem_opt")]
    let _vips_app = libvips::VipsApp::new("rpub", false).expect("Failed to initialize libvips");

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info,html5ever=error".into());
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let conn = db::init_db("rpub.db").expect("Failed to initialize database");
    let db_mutex = Arc::new(Mutex::new(conn));
    let state = Arc::new(AppState {
        db: db_mutex.clone(),
    });

    let _sched = scheduler::init_scheduler(db_mutex)
        .await
        .expect("Failed to initialize scheduler");

    tokio::fs::create_dir_all("static/epubs").await.unwrap();

    let app = Router::new()
        .route("/generate", post(generate_handler))
        .route("/feeds", get(list_feeds).post(add_feed))
        .route("/feeds/{id}", delete(delete_feed))
        .route("/schedules", get(list_schedules).post(add_schedule))
        .route("/schedules/{id}", delete(delete_schedule))
        .route("/downloads", get(list_downloads))
        .route("/opds", get(opds_handler))
        .fallback_service(ServeDir::new("static"))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn opds_handler(headers: HeaderMap) -> Result<impl IntoResponse, (StatusCode, String)> {
    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("127.0.0.1:3000");

    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("http");

    let base_url = format!("{}://{}", scheme, host);

    let xml = opds::generate_opds_feed(&base_url, "static/epubs")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        header::CONTENT_TYPE,
        "application/atom+xml;profile=opds-catalog;kind=navigation"
            .parse()
            .unwrap(),
    );

    Ok((response_headers, xml))
}

async fn list_downloads() -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let mut files = Vec::new();
    let mut entries = tokio::fs::read_dir("static/epubs").await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to read downloads: {}", e),
        )
    })?;

    while let Some(entry) = entries.next_entry().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to read entry: {}", e),
        )
    })? {
        if let Ok(name) = entry.file_name().into_string() {
            if name.ends_with(".epub") {
                files.push(name);
            }
        }
    }
    // Sort by name (date) descending
    files.sort_by(|a, b| b.cmp(a));
    Ok(Json(files))
}

// Feed Handlers
async fn list_feeds(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<db::Feed>>, (StatusCode, String)> {
    let db = state.db.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB lock failed".to_string(),
        )
    })?;
    let feeds =
        db::get_feeds(&db).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(feeds))
}

#[derive(Deserialize)]
struct AddFeedRequest {
    url: String,
    name: Option<String>,
    #[serde(default)]
    concurrency_limit: usize,
}

async fn add_feed(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AddFeedRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let db = state.db.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB lock failed".to_string(),
        )
    })?;
    db::add_feed(
        &db,
        &payload.url,
        payload.name.as_deref(),
        payload.concurrency_limit,
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::CREATED)
}

async fn delete_feed(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    let db = state.db.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB lock failed".to_string(),
        )
    })?;
    db::delete_feed(&db, id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

// Schedule Handlers
async fn list_schedules(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<db::Schedule>>, (StatusCode, String)> {
    let db = state.db.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB lock failed".to_string(),
        )
    })?;
    let schedules =
        db::get_schedules(&db).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(schedules))
}

#[derive(Deserialize)]
struct AddScheduleRequest {
    cron_expression: String,
}

async fn add_schedule(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AddScheduleRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let db = state.db.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB lock failed".to_string(),
        )
    })?;
    db::add_schedule(&db, &payload.cron_expression)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::CREATED)
}

async fn delete_schedule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    let db = state.db.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB lock failed".to_string(),
        )
    })?;
    db::delete_schedule(&db, id).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn generate_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<GenerateRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    info!("Received request to generate EPUB");

    // 1. Determine Feeds to Fetch
    let feeds_to_fetch = if payload.feeds.is_empty() {
        let db = state.db.lock().map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB lock failed".to_string(),
            )
        })?;
        let stored_feeds =
            db::get_feeds(&db).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        stored_feeds
    } else {
        payload.feeds
    };

    if feeds_to_fetch.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "No feeds provided and no stored feeds found.".to_string(),
        ));
    }

    // 2. Spawn Background Task
    let db_clone = state.db.clone();
    tokio::spawn(async move {
        info!("Starting background EPUB generation...");
        match processor::generate_and_save(feeds_to_fetch, &db_clone, "static/epubs").await {
            Ok(filename) => {
                info!("Background generation completed successfully: {}", filename);
            }
            Err(e) => {
                tracing::error!("Background generation failed: {}", e);
            }
        }
    });

    // 3. Return Accepted
    Ok(StatusCode::ACCEPTED)
}
