use std::sync::Arc;
use axum::Json;
use axum::http::StatusCode;
use axum::extract::State;
use tracing::info;
use crate::models::{AppState, GenerateRequest};
use crate::{db, email, processor, util};

pub async fn list_downloads() -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let mut files = Vec::new();
    let mut entries = tokio::fs::read_dir(util::EPUB_OUTPUT_DIR)
        .await
        .map_err(|e| {
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

    files.sort_by(|a, b| b.cmp(a));
    Ok(Json(files))
}

pub async fn generate_epub_adhoc(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<GenerateRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    info!("Received request to generate EPUB");

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

    let db_clone = state.db.clone();
    let send_email = payload.send_email;

    tokio::spawn(async move {
        info!("Starting background EPUB generation...");
        match processor::generate_and_save(feeds_to_fetch, &db_clone, util::EPUB_OUTPUT_DIR).await
        {
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
                            let epub_path =
                                std::path::Path::new(util::EPUB_OUTPUT_DIR).join(&filename);
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

    Ok(StatusCode::ACCEPTED)
}