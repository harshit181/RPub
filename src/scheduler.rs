use crate::{db, processor};
use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{error, info};

pub async fn init_scheduler(db_conn: Arc<Mutex<Connection>>) -> Result<JobScheduler> {
    let sched = JobScheduler::new().await?;

    let cleanup_job = Job::new_async("0 0 * * * *", |_uuid, _l| {
        Box::pin(async {
            info!("Running cleanup task...");
            if let Err(e) = cleanup_old_files().await {
                error!("Cleanup failed: {}", e);
            }
        })
    })?;
    sched.add(cleanup_job).await?;

    let schedules = {
        let conn = db_conn
            .lock()
            .map_err(|_| anyhow::anyhow!("DB lock failed"))?;
        db::get_schedules(&conn)?
    };

    for schedule in schedules {
        if schedule.active {
            let db_clone = db_conn.clone();
            info!("Adding schedule: {}", schedule.cron_expression);

            match Job::new_async(schedule.cron_expression.as_str(), move |_uuid, _l| {
                let db = db_clone.clone();
                Box::pin(async move {
                    info!("Running scheduled generation...");
                    if let Err(e) = run_scheduled_generation(db).await {
                        error!("Scheduled generation failed: {}", e);
                    }
                })
            }) {
                Ok(job) => {
                    sched.add(job).await?;
                }
                Err(e) => error!(
                    "Failed to create job for schedule {}: {}",
                    schedule.cron_expression, e
                ),
            }
        }
    }

    sched.start().await?;
    Ok(sched)
}

async fn run_scheduled_generation(db: Arc<Mutex<Connection>>) -> Result<()> {
    let feeds = {
        let conn = db.lock().map_err(|_| anyhow::anyhow!("DB lock failed"))?;
        let stored_feeds = db::get_feeds(&conn)?;
        stored_feeds
    };

    if feeds.is_empty() {
        info!("No feeds to generate.");
        return Ok(());
    }

    let filename = processor::generate_and_save(feeds, &db, crate::util::EPUBS_OUTPUT_DIR).await?;
    info!("Scheduled generation completed: {}", filename);

    let send_email = {
        let conn = db.lock().map_err(|_| anyhow::anyhow!("DB lock failed"))?;
        match db::get_email_config(&conn)? {
            Some(config) => config.enable_auto_send,
            None => false,
        }
    };

    if send_email {
        info!("Auto-send enabled. Sending email...");
        
        let config_opt = {
            let conn = db.lock().map_err(|_| anyhow::anyhow!("DB lock failed"))?;
            db::get_email_config(&conn)?
        };

        if let Some(config) = config_opt {
             let epub_path = std::path::Path::new(crate::util::EPUBS_OUTPUT_DIR).join(&filename);
             if let Err(e) = crate::email::send_epub(&config, &epub_path).await {
                 error!("Failed to auto-send email: {}", e);
             } else {
                 info!("Auto-send email sent successfully.");
             }
        }
    }

    Ok(())
}

async fn cleanup_old_files() -> Result<()> {
    let output_dir = crate::util::EPUBS_OUTPUT_DIR;
    if !Path::new(output_dir).exists() {
        return Ok(());
    }

    let mut entries = tokio::fs::read_dir(output_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let metadata = entry.metadata().await?;
        if let Ok(modified) = metadata.modified() {
            if modified.elapsed().unwrap_or_default() > Duration::from_secs(48 * 3600) {
                info!("Deleting old file: {:?}", entry.path());
                tokio::fs::remove_file(entry.path()).await?;
            }
        }
    }
    Ok(())
}
