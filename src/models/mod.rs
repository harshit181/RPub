use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as TokioMutex;
use tokio_cron_scheduler::JobScheduler;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Feed {
    pub id: Option<i64>,
    pub url: String,
    pub name: Option<String>,
    #[serde(default)]
    pub concurrency_limit: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Schedule {
    pub id: Option<i64>,
    pub cron_expression: String,
    pub active: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmailConfig {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_password: String,
    pub email_address: String,
    pub to_email: String,
    #[serde(default)]
    pub enable_auto_send: bool,
}

pub struct AppState {
    pub db: Arc<Mutex<rusqlite::Connection>>,
    pub scheduler: Arc<TokioMutex<JobScheduler>>,
}

#[derive(Deserialize)]
pub struct GenerateRequest {
    #[serde(default)]
    pub feeds: Vec<Feed>,
    #[serde(default)]
    pub send_email: bool,
}

#[derive(Deserialize)]
pub struct AddFeedRequest {
    pub url: String,
    pub name: Option<String>,
    #[serde(default)]
    pub concurrency_limit: usize,
}

#[derive(Serialize)]
pub struct ScheduleResponse {
    pub id: i64,
    pub time: String,
    pub active: bool,
}

#[derive(Deserialize)]
pub struct AddScheduleRequest {
    pub hour: u32,
    pub minute: u32,
    pub timezone: String,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ReadItLaterArticle {
    pub id: Option<i64>,
    pub url: String,
    pub title: Option<String>,
    pub read: bool,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct AddReadItLaterRequest {
    pub url: String,
    pub title: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateReadItLaterStatusRequest {
    pub read: bool,
}
