use anyhow::Context;
use anyhow::Result;
use libvips::{ops, VipsImage};
use regex::Regex;
use reqwest::Client;
use std::io::Write;
use std::sync::Arc;
use tempfile::NamedTempFile;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{error, info, warn};

pub async fn process_images(html: &str) -> (String, Vec<(String, NamedTempFile, String)>) {
    let mut processed_html = html.to_string();
    let mut images = Vec::new();

    let img_regex = Regex::new(r#"<img[^>]+src="([^"]+)"[^>]*>"#).unwrap();

    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()
        .unwrap_or_else(|_| Client::new());

    let mut matches = Vec::new();
    for cap in img_regex.captures_iter(html) {
        if let Some(src) = cap.get(1) {
            matches.push(src.as_str().to_string());
        }
    }

    matches.sort();
    matches.dedup();

    let semaphore = Arc::new(Semaphore::new(50));
    let mut join_set = JoinSet::new();

    for (i, src) in matches.into_iter().enumerate() {
        let client = client.clone();
        let src_clone = src.clone();

        let permit = semaphore.clone().acquire_owned().await.unwrap();

        join_set.spawn(async move {
            let _permit = permit;
            info!("Processing image: {}", &src_clone);
            match download_image(&client, &src_clone).await {
                Ok(temp_file) => {
                    let file_path = temp_file.path().to_str().unwrap().to_string();

                    let res =
                        tokio::task::spawn_blocking(move || resize_and_grayscale(&file_path)).await;

                    match res {
                        Ok(Ok(processed_temp_file)) => {
                            let extension = "jpg";
                            let filename = format!(
                                "image_{}_{}.{}",
                                chrono::Utc::now().timestamp_millis(),
                                i,
                                extension
                            );
                            let mime_type = "image/jpeg".to_string();
                            info!("Processed image: {}", &src_clone);
                            Ok((src_clone, filename, processed_temp_file, mime_type))
                        }
                        Ok(Err(e)) => Err((src_clone, format!("Processing failed: {}", e))),
                        Err(e) => Err((src_clone, format!("Blocking task join error: {}", e))),
                    }
                }
                Err(e) => Err((src_clone, format!("Download failed: {}", e))),
            }
        });
    }

    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(Ok((src, filename, temp_file, mime_type))) => {
                processed_html = processed_html.replace(&src, &filename);
                images.push((filename, temp_file, mime_type));
            }
            Ok(Err((src, e))) => {
                warn!("Failed to process image {}: {}", src, e);
            }
            Err(e) => {
                error!("Task join error: {}", e);
            }
        }
    }

    (processed_html, images)
}

async fn download_image(client: &Client, url: &str) -> Result<NamedTempFile> {
    let mut resp = client.get(url).send().await?;
    let mut temp_file = NamedTempFile::new()?;

    while let Some(chunk) = resp.chunk().await? {
        temp_file.write_all(&chunk)?;
    }

    Ok(temp_file)
}

fn resize_and_grayscale(file_path: &str) -> Result<NamedTempFile> {
    let image = ops::thumbnail(file_path, 600)?;

    let resized = image;

    let grayscale = ops::colourspace(&resized, ops::Interpretation::BW)?;

    let temp_file = tempfile::Builder::new().suffix(".jpg").tempfile()?;
    let temp_path = temp_file
        .path()
        .to_str()
        .context("Failed to convert temp file path to valid UTF-8 string")?;
    info!("Created temp image at {}", &temp_path);
    grayscale.image_write_to_file(temp_path)?;

    Ok(temp_file)
}
