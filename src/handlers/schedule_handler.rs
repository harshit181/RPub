use std::sync::Arc;
use axum::extract::{Path, State};
use axum::Json;
use axum::http::StatusCode;
use chrono_tz::Tz;
use chrono::{Local, Timelike};
use tracing::{info, warn};
use crate::{db, scheduler};
use crate::models::{AddScheduleRequest, AppState, ScheduleResponse};

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
                            schedule_type: s.schedule_type,
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
        db::add_schedule(&db, &cron_expression, &payload.schedule_type)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    if let Some(value) = restart_schedule(state).await {
        return value;
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

    if let Some(value) = restart_schedule(state).await {
        return value;
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn restart_schedule(state: Arc<AppState>) -> Option<Result<StatusCode, (StatusCode, String)>> {
    {
        let mut sched = state.scheduler.lock().await;
        if let Err(e) = sched.shutdown().await {
            warn!("Failed to shutdown previous scheduler: {}", e);
        }
        match scheduler::init_scheduler(state.db.clone()).await {
            Ok(new_sched) => *sched = new_sched,
            Err(e) => {
                tracing::error!("Failed to restart scheduler: {}", e);
                return Some(Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to restart scheduler".to_string(),
                )));
            }
        }
    };
    None
}