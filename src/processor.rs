use crate::db::Feed;
use crate::{epub_gen, feed};
use anyhow::Result;
use chrono::{Duration as ChronoDuration, Utc};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::info;

pub async fn generate_epub(
    feeds: Vec<Feed>,
    _db: &Arc<Mutex<Connection>>,
    output_path: &str,
) -> Result<()> {
    info!("Fetching {} feeds...", feeds.len());

    let (fetched_feeds, errors) = feed::fetch_feeds(&feeds).await;
    let since = Utc::now() - ChronoDuration::hours(24);
    let articles = feed::filter_items(fetched_feeds, errors, since).await;

    if articles.is_empty() {
        return Err(anyhow::anyhow!("No articles found in the last 24 hours."));
    }

    let temp_path = get_temp_file_path(output_path);
    info!("Generating EPUB to temporary file: {:?}", temp_path);
    let file = std::fs::File::create(&temp_path)?;

    match epub_gen::generate_epub_data(&articles, file).await {
        Ok(_) => {
            info!("EPUB generation successful. moving to {}", output_path);
            std::fs::rename(&temp_path, output_path)?;
            Ok(())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&temp_path);
            Err(anyhow::anyhow!("Failed to generate EPUB: {}", e))
        }
    }?;

    Ok(())
}

fn get_temp_file_path(output_path: &str) -> PathBuf {
    let output_path_obj = std::path::Path::new(output_path);
    let parent_dir = output_path_obj
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let temp_filename = format!(
        "{}.part",
        output_path_obj
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    );
    let temp_path = parent_dir.join(&temp_filename);
    temp_path
}

pub async fn generate_and_save(
    feeds: Vec<Feed>,
    db: &Arc<Mutex<Connection>>,
    output_dir: &str,
) -> Result<String> {
    let filename = format!("rss_digest_{}.epub", Utc::now().format("%Y%m%d_%H%M%S"));
    let filepath = format!("{}/{}", output_dir, filename);

    generate_epub(feeds, db, &filepath).await?;

    Ok(filename)
}
