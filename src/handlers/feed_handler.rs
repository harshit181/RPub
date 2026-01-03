use std::sync::Arc;
use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use crate::db;
use crate::models::{AddFeedRequest, AppState, Feed, FeedProcessor, ProcessorType};

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

    let feed_id=db::add_feed(
        &db,
        &payload.url,
        payload.name.as_deref(),
        payload.concurrency_limit,
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    if let Some(processor) = payload.processor {
        if processor != ProcessorType::Default {
        let _ = db::save_feed_processor(&db,feed_id,processor,payload.custom_config.as_deref(),);
        }
    }
    
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

pub async fn get_feed_processor(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<Option<FeedProcessor>>, (StatusCode, String)> {
    let db = state.db.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB lock failed".to_string(),
        )
    })?;
    let processor = db::get_feed_processor(&db, id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(processor))
}

#[derive(Deserialize)]
pub struct UpdateFeedProcessorRequest {
    pub processor: ProcessorType,
    pub custom_config: Option<String>,
}

pub async fn update_feed_processor(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateFeedProcessorRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let db = state.db.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB lock failed".to_string(),
        )
    })?;
    
    if payload.processor == ProcessorType::Default {
        db::delete_feed_processor(&db, id)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    } else {
        db::save_feed_processor(&db, id, payload.processor, payload.custom_config.as_deref())
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    
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
                    let _ = db::add_feed(&db, &xml_url, Some(&outline.text), 0);
                }

                if !outline.outlines.is_empty() {
                    for child in outline.outlines {
                        if let Some(xml_url) = child.xml_url {
                            let _ = db::add_feed(&db, &xml_url, Some(&child.text), 0);
                        }
                    }
                }
            }

            return Ok(StatusCode::CREATED);
        }
    }

    Err((StatusCode::BAD_REQUEST, "No file field found".to_string()))
}