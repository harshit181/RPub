use std::sync::Arc;
use axum::{extract::{State, Json}, http::StatusCode};
use crate::models::{AppState, GeneralConfig};
use crate::db;

pub async fn get_general_config(
    State(state): State<Arc<AppState>>,
) -> Result<Json<GeneralConfig>, (StatusCode, String)> {
    let db = state.db.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB lock failed".to_string(),
        )
    })?;

    let config = db::get_general_config(&db)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(config))
}

pub async fn update_general_config(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<GeneralConfig>,
) -> Result<StatusCode, (StatusCode, String)> {
    let db = state.db.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB lock failed".to_string(),
        )
    })?;

    db::update_general_config(&db, &payload)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::OK)
}
