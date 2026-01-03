use crate::models::{Feed, ReadItLaterArticle};
use crate::{epub_gen, feed};
use anyhow::Result;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use reqwest::Client;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{info, warn};
use crate::feed::Article;
use crate::util::content_extractors;

pub async fn generate_epub(
    feeds: Vec<Feed>,
    _db: &Arc<Mutex<Connection>>,
    output_path: &str,
) -> Result<()> {
    info!("Fetching {} feeds...", feeds.len());

    let (fetched_feeds, errors) = feed::fetch_feeds(&feeds).await;

    let (since, image_timeout) = {
        let conn = _db.lock().map_err(|_| anyhow::anyhow!("DB lock failed"))?;
        let config = crate::db::get_general_config(&conn)?;
        (Utc::now() - ChronoDuration::hours(config.fetch_since_hours as i64), config.image_timeout_seconds)
    };
    info!("Filtering items since: {}", since);
    let articles = feed::filter_items(fetched_feeds, errors, since).await;

    if articles.is_empty() {
        return Err(anyhow::anyhow!("No articles found in the last 24 hours."));
    }

    generate_epub_from_articles(output_path, &articles, image_timeout).await?;

    Ok(())
}

async fn generate_epub_from_articles(output_path: &str, articles: &Vec<Article>, image_timeout: i32) -> Result<()> {
    let temp_path = get_temp_file_path(output_path);
    info!("Generating EPUB to temporary file: {:?}", temp_path);
    let file = std::fs::File::create(&temp_path)?;

    match epub_gen::generate_epub_data(&articles, file, image_timeout).await {
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

pub async fn generate_read_it_later_epub(
    articles: Vec<ReadItLaterArticle>,
    output_dir: &str,
    image_timeout: i32,
) -> Result<String> {
    let filename = format!(
        "read_it_later_{}.epub",
        Utc::now().format("%Y%m%d_%H%M%S")
    );
    let filepath = format!("{}/{}", output_dir, filename);

    info!("Fetching content for {} Read It Later articles...", articles.len());

    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(Duration::from_secs(45))
        .cookie_store(true)
        .build()
        .unwrap_or_else(|_| Client::new());

    let mut fetched_articles = Vec::new();

    fetch_all_article_with_content(articles, &client, &mut fetched_articles).await;

    if fetched_articles.is_empty() {
        return Err(anyhow::anyhow!("No content could be fetched."));
    }
    generate_epub_from_articles(&filepath, &fetched_articles, image_timeout).await?;
    Ok(filename)
}

async fn fetch_all_article_with_content(articles: Vec<ReadItLaterArticle>, client: &Client, fetched_articles: &mut Vec<Article>) {
    for article in articles {
        info!("Fetching: {}", article.url);
        match content_extractors::fetch_full_content(&client, &article.url).await {
            Ok((title, content)) => {
                fetched_articles.push(crate::feed::Article {
                    title,
                    link: article.url.clone(),
                    content,
                    pub_date: DateTime::parse_from_rfc3339(&article.created_at)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    source: "Read It Later".to_string(),
                });
            }
            Err(e) => {
                warn!("Failed to fetch {}: {}", article.url, e);
                fetched_articles.push(crate::feed::Article {
                    title: format!("Error: {}", article.url),
                    link: article.url.clone(),
                    content: format!("<p>Failed to fetch content: {}</p>", e),
                    pub_date: Utc::now(),
                    source: "Read It Later Errors".to_string(),
                });
            }
        }
    }
}
