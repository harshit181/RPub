use crate::db;
use crate::email;
use crate::models::{AddFeedRequest, AddScheduleRequest, AppState, EmailConfig, Feed, GenerateRequest, ScheduleResponse};
use crate::opds;
use crate::processor;
use crate::scheduler;
use crate::util;
use axum::{
    extract::{Json, Multipart, Path, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use base64::Engine;
use chrono::{Local, Timelike};
use chrono_tz::Tz;

use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

pub async fn opds_handler(headers: HeaderMap) -> Result<impl IntoResponse, (StatusCode, String)> {
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

pub async fn list_downloads() -> Result<Json<Vec<String>>, (StatusCode, String)> {
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
pub async fn list_feeds(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Feed>>, (StatusCode, String)> {
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


pub async fn add_feed(
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

pub async fn delete_feed(
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


pub async fn list_schedules(
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



pub async fn add_schedule(
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

pub async fn delete_schedule(
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

pub async fn generate_handler(
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
                tracing::error!("Failed to error: {}", e);
            }
        }
    });

    // 3. Return Accepted
    Ok(StatusCode::ACCEPTED)
}

pub async fn upload_cover(mut multipart: Multipart) -> Result<StatusCode, (StatusCode, String)> {
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

pub async fn get_email_config_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Option<EmailConfig>>, (StatusCode, String)> {
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

pub async fn update_email_config_handler(
    State(state): State<Arc<AppState>>,
    Json(mut payload): Json<EmailConfig>,
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

pub async fn import_opml(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<StatusCode, (StatusCode, String)> {
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Failed to read multipart field: {}", e),
        )
    })? {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            let data = field.bytes().await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to read field bytes: {}", e),
                )
            })?;

             let opml_str = String::from_utf8(data.to_vec()).map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("Invalid UTF-8 sequence: {}", e),
                )
            })?;

            let document = opml::OPML::from_str(&opml_str).map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("Failed to parse OPML: {}", e),
                )
            })?;

            let db = state.db.lock().map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "DB lock failed".to_string(),
                )
            })?;

            for outline in document.body.outlines {
                if let Some(xml_url) = outline.xml_url {
                     let _ = db::add_feed(
                        &db,
                        &xml_url,
                        Some(&outline.text),
                        0, 
                    );
                }
                
                if !outline.outlines.is_empty() {
                     for child in outline.outlines {
                        if let Some(xml_url) = child.xml_url {
                            let _ = db::add_feed(
                                &db,
                                &xml_url,
                                Some(&child.text),
                                0,
                            );
                        }
                     }
                }
            }
            
            return Ok(StatusCode::CREATED);
        }
    }

    Err((StatusCode::BAD_REQUEST, "No file field found".to_string()))
}

pub async fn auth(
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
