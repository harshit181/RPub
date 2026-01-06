use std::sync::{Arc, MutexGuard};
use axum::{
    extract::{Path, State, Json},
    http::StatusCode,
};
use rusqlite::Connection;
use crate::models::{AddDomainOverrideRequest, AppState, DomainOverride};
use crate::db;
use crate::util::content_extractors;

pub async fn list_domain_overrides(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<DomainOverride>>, (StatusCode, String)> {
    let db = state.db.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB lock failed".to_string(),
        )
    })?;

    let overrides = db::get_domain_overrides(&db)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(overrides))
}

pub async fn add_domain_override(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AddDomainOverrideRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let db = state.db.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB lock failed".to_string(),
        )
    })?;

    db::add_domain_override(&db, &payload.domain, payload.processor, payload.custom_config.as_deref())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    refresh_domain_processor_map(&db);
    Ok(StatusCode::CREATED)
}

pub async fn delete_domain_override(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    let db = state.db.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB lock failed".to_string(),
        )
    })?;

    db::delete_domain_override(&db, id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    refresh_domain_processor_map(&db);
    Ok(StatusCode::OK)
}

fn refresh_domain_processor_map(db: &MutexGuard<Connection>) {
    if let Ok(overrides) = db::get_domain_overrides(&db) {
        let override_list: Vec<_> = overrides.into_iter()
            .map(|o| (o.domain, crate::models::ContentProcessor {
                id: o.id,
                processor: o.processor,
                custom_config: o.custom_config,
            }))
            .collect();
        content_extractors::refresh_domain_overrides(override_list);
    }
}
