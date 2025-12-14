mod db;
mod email;
mod epub_gen;
mod epub_message;
mod feed;
#[cfg(feature = "mem_opt")]
mod image;
#[cfg(not(feature = "mem_opt"))]
#[path = "image_inmem.rs"]
mod image;
mod opds;
mod processor;
mod scheduler;
mod util;

use tokio_cron_scheduler::JobScheduler;

use crate::db::Feed;
use axum::{
    Router,
    extract::{Json, Multipart, Path, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::{delete, get, post},
};
use chrono::{Local, Timelike};
use chrono_tz::Tz;
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex as TokioMutex;

use base64::Engine;
use tower::ServiceBuilder;
use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer};
use tracing::{info, warn};

#[cfg(feature = "alternative-alloc")]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

struct AppState {
    db: Arc<Mutex<rusqlite::Connection>>,
    scheduler: Arc<TokioMutex<JobScheduler>>,
}

#[derive(Deserialize)]
struct GenerateRequest {
    #[serde(default)]
    feeds: Vec<Feed>,
    #[serde(default)]
    send_email: bool,
}

#[tokio::main]
async fn main() {
    #[cfg(feature = "mem_opt")]
    let _vips_app = libvips::VipsApp::new("rsspub", false).expect("Failed to initialize libvips");
    #[cfg(feature = "alternative-alloc")]
    tikv_jemalloc_ctl::background_thread::write(true).expect("failed to enable background threads");

    tracing_subscriber::fmt().init();

    let conn = db::init_db("./db/rpub.db").expect("Failed to initialize database");
    let db_mutex = Arc::new(Mutex::new(conn));
    let sched = scheduler::init_scheduler(db_mutex.clone())
        .await
        .expect("Failed to initialize scheduler");

    let state = Arc::new(AppState {
        db: db_mutex.clone(),
        scheduler: Arc::new(TokioMutex::new(sched)),
    });

    tokio::fs::create_dir_all(util::EPUBS_OUTPUT_DIR).await.unwrap();

    let public_routes = Router::new().route("/opds", get(opds_handler));

    let protected_routes = Router::new()
        .route("/generate", post(generate_handler))
        .route("/feeds", get(list_feeds).post(add_feed))
        .route("/feeds/{id}", delete(delete_feed))
        .route("/schedules", get(list_schedules).post(add_schedule))
        .route("/schedules/{id}", delete(delete_schedule))
        .route("/downloads", get(list_downloads))
        .route("/cover", post(upload_cover))
        .route(
            "/email-config",
            get(get_email_config_handler).post(update_email_config_handler),
        )
        .route("/auth/check", get(|| async { StatusCode::OK }));

    let protected_routes =
        if std::env::var("RPUB_USERNAME").is_ok() && std::env::var("RPUB_PASSWORD").is_ok() {
            info!("Authentication enabled");
            protected_routes.layer(axum::middleware::from_fn(auth))
        } else {
            warn!("Authentication disabled (RPUB_USERNAME and/or RPUB_PASSWORD not set)");
            protected_routes
        };

    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .fallback_service(
            ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::overriding(
                    header::CACHE_CONTROL,
                    header::HeaderValue::from_static("public, max-age=3600"),
                ))
                .service(ServeDir::new("static")),
        )
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

    let xml = opds::generate_opds_feed(&base_url, util::EPUBS_OUTPUT_DIR)
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
    let mut entries = tokio::fs::read_dir(util::EPUBS_OUTPUT_DIR).await.map_err(|e| {
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
#[derive(serde::Serialize)]
struct ScheduleResponse {
    id: i64,
    time: String,
    active: bool,
}

async fn list_schedules(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ScheduleResponse>>, (StatusCode, String)> {
    let db = state.db.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB lock failed".to_string(),
        )
    })?;
    let schedules =
        db::get_schedules(&db).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut response = Vec::new();
    for s in schedules {
        let parts: Vec<&str> = s.cron_expression.split_whitespace().collect();
        if parts.len() >= 5 {
            if let (Ok(minute), Ok(hour)) = (parts[1].parse::<u32>(), parts[2].parse::<u32>()) {
                let now = Local::now();
                if let Some(date) = now.date_naive().and_hms_opt(hour, minute, 0) {
                    if let Some(local_dt) = date.and_local_timezone(Local).single() {
                        response.push(ScheduleResponse {
                            id: s.id.unwrap_or_default(),
                            time: local_dt.to_rfc3339(),
                            active: s.active,
                        });
                        continue;
                    }
                }
            }
        }

        warn!(
            "Skipping invalid/unparseable schedule cron: {}",
            s.cron_expression
        );
    }

    Ok(Json(response))
}

#[derive(Deserialize)]
struct AddScheduleRequest {
    hour: u32,
    minute: u32,
    timezone: String,
}

async fn add_schedule(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AddScheduleRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let tz: Tz = payload
        .timezone
        .parse()
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid timezone: {}", e)))?;

    let now_in_tz = chrono::Utc::now().with_timezone(&tz);

    let target_time_in_tz = now_in_tz
        .date_naive()
        .and_hms_opt(payload.hour, payload.minute, 0)
        .ok_or((StatusCode::BAD_REQUEST, "Invalid time".to_string()))?
        .and_local_timezone(tz)
        .unwrap();
    let target_in_server = target_time_in_tz.with_timezone(&Local);

    let server_hour = target_in_server.hour();
    let server_minute = target_in_server.minute();

    let cron_expression = format!("0 {} {} * * *", server_minute, server_hour);

    info!(
        "Converting {} {:02}:{:02} -> Server {:02}:{:02} (Cron: {})",
        payload.timezone, payload.hour, payload.minute, server_hour, server_minute, cron_expression
    );

    {
        let db = state.db.lock().map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB lock failed".to_string(),
            )
        })?;
        db::add_schedule(&db, &cron_expression)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    {
        let mut sched = state.scheduler.lock().await;
        if let Err(e) = sched.shutdown().await {
            warn!("Failed to shutdown previous scheduler: {}", e);
        }
        match scheduler::init_scheduler(state.db.clone()).await {
            Ok(new_sched) => *sched = new_sched,
            Err(e) => {
                tracing::error!("Failed to restart scheduler: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to restart scheduler".to_string(),
                ));
            }
        }
    }

    Ok(StatusCode::CREATED)
}

async fn delete_schedule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    {
        let db = state.db.lock().map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB lock failed".to_string(),
            )
        })?;
        db::delete_schedule(&db, id)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    // Restart scheduler
    {
        let mut sched = state.scheduler.lock().await;
        if let Err(e) = sched.shutdown().await {
            warn!("Failed to shutdown previous scheduler: {}", e);
        }
        match scheduler::init_scheduler(state.db.clone()).await {
            Ok(new_sched) => *sched = new_sched,
            Err(e) => {
                tracing::error!("Failed to restart scheduler: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to restart scheduler".to_string(),
                ));
            }
        }
    }

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
    let send_email = payload.send_email;

    tokio::spawn(async move {
        info!("Starting background EPUB generation...");
        match processor::generate_and_save(feeds_to_fetch, &db_clone, util::EPUBS_OUTPUT_DIR).await {
            Ok(filename) => {
                info!("Background generation completed successfully: {}", filename);
                if send_email {
                    info!("Email sending requested. Fetching config...");
                    let config_result = {
                        let db = db_clone.lock().unwrap();

                        db::get_email_config(&db)
                    };

                    match config_result {
                        Ok(Some(config)) => {
                            let epub_path = std::path::Path::new(util::EPUBS_OUTPUT_DIR).join(&filename);
                            info!("Sending email for {}...", filename);
                            if let Err(e) = email::send_epub(&config, &epub_path).await {
                                tracing::error!("Failed to send email: {}", e);
                            }
                        }
                        Ok(None) => {
                            tracing::warn!("Email sending requested but no email config found.");
                        }
                        Err(e) => {
                            tracing::error!("Failed to fetch email config: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!("Background generation failed: {}", e);
            }
        }
    });

    // 3. Return Accepted
    Ok(StatusCode::ACCEPTED)
}

async fn upload_cover(mut multipart: Multipart) -> Result<StatusCode, (StatusCode, String)> {
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Failed to read multipart field: {}", e),
        )
    })? {
        let name = field.name().unwrap_or("").to_string();
        if name == "cover" {
            let data = field.bytes().await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to read field bytes: {}", e),
                )
            })?;

            if data.is_empty() {
                return Err((StatusCode::BAD_REQUEST, "Empty file".to_string()));
            }

            let path = "static/cover.jpg";
            let mut file = tokio::fs::File::create(path).await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to create file: {}", e),
                )
            })?;

            file.write_all(&data).await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to write file: {}", e),
                )
            })?;

            info!("Cover image updated successfully");
            return Ok(StatusCode::OK);
        }
    }

    Err((StatusCode::BAD_REQUEST, "No cover file found".to_string()))
}

async fn get_email_config_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Option<db::EmailConfig>>, (StatusCode, String)> {
    let db = state.db.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB lock failed".to_string(),
        )
    })?;
    let config = db::get_email_config(&db)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if let Some(mut c) = config {
        c.smtp_password = "".to_string();
        Ok(Json(Some(c)))
    } else {
        Ok(Json(None))
    }
}

async fn update_email_config_handler(
    State(state): State<Arc<AppState>>,
    Json(mut payload): Json<db::EmailConfig>,
) -> Result<StatusCode, (StatusCode, String)> {
    let db = state.db.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB lock failed".to_string(),
        )
    })?;

    if payload.smtp_password.is_empty() {
        if let Ok(Some(existing)) = db::get_email_config(&db) {
            payload.smtp_password = existing.smtp_password;
        }
    }

    db::save_email_config(&db, &payload)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::OK)
}

async fn auth(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok());

    let username = std::env::var("RPUB_USERNAME").unwrap_or_default();
    let password = std::env::var("RPUB_PASSWORD").unwrap_or_default();

    if let Some(auth_header) = auth_header {
        if let Some(token) = auth_header.strip_prefix("Basic ") {
            if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(token) {
                if let Ok(credentials) = String::from_utf8(decoded) {
                    if let Some((u, p)) = credentials.split_once(':') {
                        if u == username && p == password {
                            return next.run(req).await.into_response();
                        }
                    }
                }
            }
        }
    }

    // Return 401 WITHOUT the WWW-Authenticate header to prevent browser popup
    (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()).into_response()
}
