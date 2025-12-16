use std::sync::Arc;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use tracing::info;
use axum::Json;
use crate::{db, email, processor, util};
use crate::models::{AddReadItLaterRequest, AppState, ReadItLaterArticle, UpdateReadItLaterStatusRequest};

pub async fn list_read_it_later(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ReadItLaterArticle>>, (StatusCode, String)> {
    let db = state.db.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB lock failed".to_string(),
        )
    })?;
    let articles = db::get_read_it_later_articles(&db, false)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(articles))
}

pub async fn add_read_it_later(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AddReadItLaterRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let db = state.db.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB lock failed".to_string(),
        )
    })?;
    db::add_read_it_later_article(&db, &payload.url)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::CREATED)
}

pub async fn update_read_it_later_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateReadItLaterStatusRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let db = state.db.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB lock failed".to_string(),
        )
    })?;
    db::update_read_it_later_status(&db, id, payload.read)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::OK)
}

pub async fn delete_read_it_later(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    let db = state.db.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB lock failed".to_string(),
        )
    })?;
    db::delete_read_it_later_article(&db, id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn deliver_read_it_later(
    State(state): State<Arc<AppState>>,
) -> Result<StatusCode, (StatusCode, String)> {
    info!("Received request to deliver Read It Later articles");

    let articles = {
        let db = state.db.lock().map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB lock failed".to_string(),
            )
        })?;
        db::get_read_it_later_articles(&db, true) // fetching only unread
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    if articles.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "No unread articles to deliver.".to_string(),
        ));
    }

    let db_clone = state.db.clone();

    tokio::spawn(async move {
        info!("Starting background Read It Later EPUB generation...");
        match processor::generate_read_it_later_epub(articles, util::EPUB_OUTPUT_DIR).await {
            Ok(filename) => {
                info!("Background generation completed successfully: {}", filename);
                let config_result = {
                    let db = db_clone.lock().unwrap();
                    db::get_email_config(&db)
                };

                match config_result {
                    Ok(Some(config)) => {
                        let epub_path = std::path::Path::new(util::EPUB_OUTPUT_DIR).join(&filename);
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
            Err(e) => {
                tracing::error!("Failed to generate EPUB: {}", e);
            }
        }
    });

    Ok(StatusCode::ACCEPTED)
}