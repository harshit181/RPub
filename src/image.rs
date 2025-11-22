use anyhow::Result;
use image::io::Reader as ImageReader;
use image::{DynamicImage, ImageFormat};
use regex::Regex;
use reqwest::Client;
use std::io::Cursor;
use tracing::{error, info, warn};
use tokio::task::JoinSet;

pub async fn process_images(html: &str) -> (String, Vec<(String, Vec<u8>, String)>) {
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

    // Deduplicate matches to avoid downloading the same image multiple times
    matches.sort();
    matches.dedup();

    let mut join_set = JoinSet::new();

    for (i, src) in matches.into_iter().enumerate() {
        let client = client.clone();
        let src_clone = src.clone();
        
        join_set.spawn(async move {
            info!("Processing image: {}", src_clone);
            match download_image(&client, &src_clone).await {
                Ok((img_data, format)) => {
                    match resize_and_grayscale(&img_data, format) {
                        Ok(processed_data) => {
                            let extension = "jpg";
                            // Use a hash or simple index for filename. Index is easier but requires coordination if we want global uniqueness.
                            // Here we are processing per article, so index is fine if we scope it.
                            // But wait, 'i' is passed in.
                            let filename = format!("image_{}_{}.{}", chrono::Utc::now().timestamp_millis(), i, extension);
                            let mime_type = "image/jpeg".to_string();
                            Ok((src_clone, filename, processed_data, mime_type))
                        },
                        Err(e) => Err((src_clone, format!("Processing failed: {}", e))),
                    }
                },
                Err(e) => Err((src_clone, format!("Download failed: {}", e))),
            }
        });
    }

    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(Ok((src, filename, data, mime_type))) => {
                // Replace src in HTML
                processed_html = processed_html.replace(&src, &filename);
                images.push((filename, data, mime_type));
            },
            Ok(Err((src, e))) => {
                warn!("Failed to process image {}: {}", src, e);
            },
            Err(e) => {
                error!("Task join error: {}", e);
            }
        }
    }

    (processed_html, images)
}

async fn download_image(client: &Client, url: &str) -> Result<(Vec<u8>, ImageFormat)> {
    let resp = client.get(url).send().await?;
    let bytes = resp.bytes().await?.to_vec();
    
    // Guess format
    let format = image::guess_format(&bytes)?;
    
    Ok((bytes, format))
}

fn resize_and_grayscale(data: &[u8], format: ImageFormat) -> Result<Vec<u8>> {
    let img = ImageReader::with_format(Cursor::new(data), format)
        .decode()?;

    // Resize
    let resized = img.resize(600, 800, image::imageops::FilterType::Lanczos3);

    // Grayscale
    let grayscale = resized.grayscale();

    // Encode to JPEG
    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    grayscale.write_to(&mut cursor, ImageFormat::Jpeg)?;

    Ok(buffer)
}
