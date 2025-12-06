use crate::db::Feed;
use crate::{epub_gen, feed};
use anyhow::Result;
use chrono::{Duration as ChronoDuration, Utc};
use rusqlite::Connection;
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

    // 4. Generate EPUB Data
    let file = std::fs::File::create(output_path)?;
    epub_gen::generate_epub_data(&articles, file)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to generate EPUB: {}", e))?;

    Ok(())
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
