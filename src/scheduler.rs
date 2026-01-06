use crate::{db, email, processor};
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
                let job_type = schedule.schedule_type.clone();
                Box::pin(async move {
                    info!("Running scheduled generation for type: {}", job_type);
                    if job_type == "rss" {
                        if let Err(e) = run_scheduled_generation(db).await {
                             error!("Scheduled generation (RSS) failed: {}", e);
                        }
                    } else if job_type == "read_it_later" {
                         if let Err(e) = run_read_it_later_generation(db).await {
                             error!("Scheduled generation (Read It Later) failed: {}", e);
                         }
                    } else {
                        error!("Unknown schedule type: {}", job_type);
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

    let filename = processor::generate_and_save(feeds, &db, crate::util::EPUB_OUTPUT_DIR).await?;
    info!("Scheduled generation completed: {}", filename);
    email::check_and_send_email(db, &filename).await?;

    Ok(())
}

async fn run_read_it_later_generation(db: Arc<Mutex<Connection>>) -> Result<()> {
    let (articles, image_timeout) = {
         let conn = db.lock().map_err(|_| anyhow::anyhow!("DB lock failed"))?;
         let articles = db::get_read_it_later_articles(&conn, true)?;
         let config = db::get_general_config(&conn)?;

         
         (articles, config.image_timeout_seconds)
    };

    if articles.is_empty() {
        info!("No unread articles to deliver.");
        return Ok(());
    }
    let article_ids: Vec<i64> = articles.iter().filter_map(|a| a.id).collect();

    let filename = processor::generate_read_it_later_epub(articles, crate::util::EPUB_OUTPUT_DIR, image_timeout).await?;
    info!("Read It Later generation completed: {}", filename);


    if !article_ids.is_empty() {
        match db.lock() {
            Ok(conn) => {
                if let Err(e) = db::mark_articles_as_read(&conn, &article_ids) {
                    error!("Failed to mark articles as read: {}", e);
                } else {
                    info!("Marked {} articles as read", article_ids.len());
                }
            }
            Err(_) => error!("Failed to lock DB to mark articles as read"),
        }
    }

    email::check_and_send_email(db, &filename).await?;
    Ok(())
}

async fn cleanup_old_files() -> Result<()> {
    let output_dir = crate::util::EPUB_OUTPUT_DIR;
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
