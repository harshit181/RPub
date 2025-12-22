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
    #[serde(default = "default_schedule_type")]
    pub schedule_type: String,
}

fn default_schedule_type() -> String {
    "rss".to_string()
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
    pub schedule_type: String,
    pub cron_expression: String,
}

#[derive(Deserialize)]
pub struct AddScheduleRequest {
    pub hour: u32,
    pub minute: u32,
    pub timezone: String,
    #[serde(default = "default_schedule_type")]
    pub schedule_type: String,
    #[serde(default = "default_frequency")]
    pub frequency: String,
    pub day_of_week: Option<u32>,
    pub day_of_month: Option<u32>,
}

fn default_frequency() -> String {
    "daily".to_string()
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ReadItLaterArticle {
    pub id: Option<i64>,
    pub url: String,
    pub read: bool,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct AddReadItLaterRequest {
    pub url: String,
}

#[derive(Deserialize)]
pub struct UpdateReadItLaterStatusRequest {
    pub read: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GeneralConfig {
    pub fetch_since_hours: i32,
    #[serde(default = "default_timeout")]
    pub image_timeout_seconds: i32,
}

fn default_timeout() -> i32 {
    45
}
