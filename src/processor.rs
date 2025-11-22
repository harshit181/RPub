use crate::{feed, epub_gen, db};
use anyhow::Result;
use chrono::{Utc, Duration as ChronoDuration};
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use tracing::info;

pub async fn generate_epub(feeds: Vec<String>, db: &Arc<Mutex<Connection>>) -> Result<Vec<u8>> {
    info!("Fetching {} feeds...", feeds.len());

    // 1. Fetch Feeds
    let (fetched_feeds, errors) = feed::fetch_feeds(&feeds).await;

    // 2. Filter Articles (Last 24 hours)
    let since = Utc::now() - ChronoDuration::hours(24);
    let articles = feed::filter_items(fetched_feeds, errors, since).await;

    if articles.is_empty() {
        return Err(anyhow::anyhow!("No articles found in the last 24 hours."));
    }


    // 4. Generate EPUB Data
    let epub_data = epub_gen::generate_epub_data(&articles).await
        .map_err(|e| anyhow::anyhow!("Failed to generate EPUB: {}", e))?;

    Ok(epub_data)
}

pub async fn generate_and_save(feeds: Vec<String>, db: &Arc<Mutex<Connection>>, output_dir: &str) -> Result<String> {
    let epub_data = generate_epub(feeds, db).await?;

    let filename = format!("rss_digest_{}.epub", Utc::now().format("%Y%m%d_%H%M%S"));
    let filepath = format!("{}/{}", output_dir, filename);
    
    tokio::fs::write(&filepath, &epub_data).await?;
    
    Ok(filename)
}
