use crate::models::{
    AddReadItLaterRequest, AppState, ReadItLaterArticle, UpdateReadItLaterStatusRequest,
};
use crate::{db, email, processor, util};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use reqwest::Url;
use std::sync::Arc;
use tracing::info;

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
    let is_valid = is_valid_web_url(&payload.url);
    if is_valid {
        db::add_read_it_later_article(&db, &payload.url)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    } else {
        return Ok(StatusCode::BAD_REQUEST);
    }
    Ok(StatusCode::CREATED)
}

fn is_valid_web_url(input: &str) -> bool {
    match Url::parse(input) {
        Ok(parsed_url) => parsed_url.scheme() == "http" || parsed_url.scheme() == "https",
        Err(_) => false,
    }
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
    let article_ids: Vec<i64> = articles.iter().filter_map(|a| a.id).collect();

    let db_clone = state.db.clone();
    tokio::spawn(async move {
        info!("Starting background Read It Later EPUB generation...");
        let image_timeout = {
            match db_clone.lock() {
                Ok(conn) => match db::get_general_config(&conn) {
                    Ok(cfg) => cfg.image_timeout_seconds,
                    Err(e) => {
                        tracing::error!("Failed to fetch config, using default timeout: {}", e);
                        45
                    }
                },
                Err(_) => {
                    tracing::error!("Failed to lock DB for config, using default timeout");
                    45
                }
            }
        };
        match processor::generate_read_it_later_epub(articles, util::EPUB_OUTPUT_DIR, image_timeout)
            .await
        {
            Ok(filename) => {
                info!("Background generation completed successfully: {}", filename);
                if !article_ids.is_empty() {
                    match db_clone.lock() {
                        Ok(conn) => {
                            if let Err(e) = db::mark_articles_as_read(&conn, &article_ids) {
                                tracing::error!("Failed to mark articles as read: {}", e);
                            } else {
                                info!("Marked {} articles as read", article_ids.len());
                            }
                        }
                        Err(_) => tracing::error!("Failed to lock DB to mark articles as read"),
                    }
                }

                match email::check_and_send_email(db_clone, &filename).await {
                    Ok(_ok) => {}
                    Err(_error) => {}
                }
            }
            Err(e) => {
                tracing::error!("Failed to generate EPUB: {}", e);
            }
        }
    });

    Ok(StatusCode::ACCEPTED)
}
