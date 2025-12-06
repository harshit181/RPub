use anyhow::Result;
use image::{DynamicImage, ImageFormat};
use regex::Regex;
use reqwest::Client;
use std::io::Cursor;
use std::sync::Arc;
use tokio::{sync::Semaphore, task::JoinSet};
use tracing::{error, info, warn};

pub async fn process_images(html: &str) -> (String, Vec<(String, Cursor<Vec<u8>>, String)>) {
    let mut processed_html = html.to_string();
    let mut images = Vec::new();

    // Regex to find img tags and extract src
    let img_regex = Regex::new(r#"<img[^>]+src="([^"]+)"[^>]*>"#).unwrap();

    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()
        .unwrap_or_else(|_| Client::new());

    // Collect all matches first
    let mut matches = Vec::new();
    for cap in img_regex.captures_iter(html) {
        if let Some(src) = cap.get(1) {
            matches.push(src.as_str().to_string());
        }
    }

    // Deduplicate matches
    matches.sort();
    matches.dedup();

    let mut join_set = JoinSet::new();
    let semaphore = Arc::new(Semaphore::new(50));
    for (i, src) in matches.into_iter().enumerate() {
        let client = client.clone();
        let src_clone = src.clone();
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        join_set.spawn(async move {
            let _permit = permit;
            info!("Processing image: {}", src_clone);
            match download_image(&client, &src_clone).await {
                Ok((img_data, format)) => match resize_and_grayscale(img_data, format) {
                    Ok(processed_data) => {
                        let extension = "jpg";
                        let filename = format!(
                            "image_{}_{}.{}",
                            chrono::Utc::now().timestamp_millis(),
                            i,
                            extension
                        );
                        let mime_type = "image/jpeg".to_string();
                        let cursor = Cursor::new(processed_data);
                        Ok((src_clone, filename, cursor, mime_type))
                    }
                    Err(e) => Err((src_clone, format!("Processing failed: {}", e))),
                },
                Err(e) => Err((src_clone, format!("Download failed: {}", e))),
            }
        });
    }

    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(Ok((src, filename, cursor, mime_type))) => {
                // Replace src in HTML
                processed_html = processed_html.replace(&src, &filename);
                images.push((filename, cursor, mime_type));
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

async fn download_image(client: &Client, url: &str) -> Result<(Vec<u8>, ImageFormat)> {
    let resp = client.get(url).send().await?;
    //let _content_length = &resp.content_length().unwrap_or(0);
    let bytes = resp.bytes().await?.to_vec();

    //info!("Image size is {}  {}", content_length, &bytes.capacity());
    // Guess format
    let format = image::guess_format(&bytes)?;

    Ok((bytes, format))
}
fn test(_data: DynamicImage) {}
fn resize_and_grayscale(data: Vec<u8>, format: ImageFormat) -> Result<Vec<u8>> {
    let img = image::load_from_memory_with_format(&data, format)?;

    // Resize
    let resized = img.resize(600, 800, image::imageops::FilterType::Nearest);
    test(img);
    let grayscale = resized.grayscale();
    test(resized);
    // Encode to JPEG
    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    grayscale.write_to(&mut cursor, ImageFormat::Jpeg)?;

    Ok(buffer)
}
