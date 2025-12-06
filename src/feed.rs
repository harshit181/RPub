use anyhow::Result;
use chrono::{DateTime, Utc};
use dom_smoothie::Config;
use dom_smoothie::TextMode;
use feed_rs::model::Feed;
use feed_rs::parser;
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{error, info, warn};

#[derive(Debug, Clone)]
pub struct Article {
    pub title: String,
    pub link: String,
    pub content: String,
    pub pub_date: DateTime<Utc>,
    pub source: String,
}

pub struct FeedWrapper {
    pub feed: Feed,
    pub limit: usize,
}

pub async fn fetch_feeds(
    db_feeds: &Vec<crate::db::Feed>,
) -> (Vec<FeedWrapper>, Vec<(String, String)>) {
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .cookie_store(true)
        .build()
        .unwrap_or_else(|_| Client::new());

    let mut feeds = Vec::new();
    let mut errors = Vec::new();
    let urls = db_feeds
        .into_iter()
        .map(|f| (f.url.clone(), f.concurrency_limit))
        .collect::<Vec<(String, usize)>>();
    for (string_url, limit) in urls {
        let url: &str = &string_url;
        match client.get(url).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    let msg = format!("Failed to fetch feed: HTTP {}", resp.status());
                    warn!("{} - {}", msg, url);
                    errors.push((format!("{}", url), msg));
                    continue;
                }

                match resp.bytes().await {
                    Ok(content) => match parser::parse(&content[..]) {
                        Ok(feed) => {
                            info!("Successfully fetched and parsed feed: {}", url);
                            feeds.push(FeedWrapper {
                                feed: feed,
                                limit: limit,
                            });
                        }
                        Err(e) => {
                            let msg = format!("Failed to parse RSS feed: {}", e);
                            warn!("{} - {}", msg, url);
                            errors.push((format!("{}", url), msg));
                        }
                    },
                    Err(e) => {
                        let msg = format!("Failed to read response body: {}", e);
                        warn!("{} - {}", msg, url);
                        errors.push((format!("{}", url), msg));
                    }
                }
            }
            Err(e) => {
                let msg = format!("Failed to fetch URL: {}", e);
                warn!("{} - {}", msg, url);
                errors.push((format!("{}", url), msg));
            }
        }
    }

    (feeds, errors)
}

pub async fn filter_items(
    feeds: Vec<FeedWrapper>,
    errors: Vec<(String, String)>,
    since: DateTime<Utc>,
) -> Vec<Article> {
    let mut articles = Vec::new();
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .cookie_store(true)
        .build()
        .unwrap_or_else(|_| Client::new());
    let mut join_set = tokio::task::JoinSet::new();

    for (url, error_msg) in errors {
        articles.push(Article {
            title: format!("Error loading feed: {}", url),
            link: url.clone(),
            content: format!("<h1>Error loading feed</h1><p><strong>URL:</strong> {}</p><p><strong>Error:</strong> {}</p>", url, error_msg),
            pub_date: Utc::now(),
            source: "System Errors".to_string(),
        });
    }

    for feedx in feeds {
        let feed = feedx.feed;
        let limit = feedx.limit;
        let semaphore = if limit > 0 {
            Some(Arc::new(Semaphore::new(limit)))
        } else {
            None
        };
        let source_title = feed
            .title
            .map(|t| t.content)
            .unwrap_or("Unknown Source".to_string());
        for entry in feed.entries {
            if let Some(pub_date) = entry.published.or(entry.updated) {
                if pub_date >= since {
                    let title = entry
                        .title
                        .as_ref()
                        .map(|t| t.content.clone())
                        .unwrap_or("No Title".to_string());

                    let link = entry
                        .links
                        .iter()
                        .find(|l| l.rel.as_deref() == Some("alternate") || l.rel.is_none())
                        .map(|l| l.href.clone())
                        .unwrap_or_default();

                    let client = client.clone();
                    let source_title = source_title.clone();
                    let entry = entry.clone();
                    let semaphore = semaphore.clone();

                    join_set.spawn(async move {
                        let _permit = if let Some(sem) = semaphore {
                            Some(sem.acquire_owned().await.unwrap())
                        } else {
                            None
                        };
                        info!("Processing article: {}", title);


                        let content = if !link.is_empty() {
                            match fetch_full_content(&client, &link).await {
                                Ok(c) => c,
                                Err(e) => {
                                    error!("Error fetching full content for '{}': {}", link, e);
                                    let error_html = format!("<p style=\"color:red\"><strong>Error fetching full content:</strong> {}</p><hr/>", e);
                                    let fallback = entry.content.map(|c| c.body.unwrap_or_default())
                                        .or(entry.summary.map(|s| s.content))
                                        .unwrap_or_default();
                                    format!("{}{}", error_html, fallback)
                                }
                            }
                        } else {
                            entry.content.map(|c| c.body.unwrap_or_default())
                                .or(entry.summary.map(|s| s.content))
                                .unwrap_or_default()
                        };

                        Article {
                            title,
                            link,
                            content,
                            pub_date,
                            source: source_title,
                        }
                    });
                }
            }
        }
    }

    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(article) => articles.push(article),
            Err(e) => error!("Task join error: {}", e),
        }
    }

    articles.sort_by(|a, b| b.pub_date.cmp(&a.pub_date));

    articles
}

async fn fetch_full_content(client: &Client, url: &str) -> Result<String> {
    let html = client.get(url).send().await?.text().await?;
    let cfg = Config {
        text_mode: TextMode::Markdown,
        ..Default::default()
    };
    let mut readability = dom_smoothie::Readability::new(html, Some(url), Some(cfg))?;
    let extracted = readability
        .parse()
        .map_err(|e| anyhow::anyhow!("DomSmoothie error: {:?}", e))?;
    Ok(extracted.content.to_string())
}
