use anyhow::Result;
use image::ImageFormat;
use regex::Regex;
use reqwest::Client;
use std::io::Cursor;
use tokio::sync::mpsc::Sender;

use crate::epub_message::{CompletionMessage, EpubPart};
use tracing::info;
use uuid::Uuid;

pub async fn process_images(
    html: &str,
    tx_m: &Sender<CompletionMessage>,
    seq_id: &usize,
) -> (String, usize) {
    let mut processed_html = html.to_string();

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
    let total_images = matches.len();
    for (i, src) in matches.into_iter().enumerate() {
        let client = client.clone();
        let src_clone = src.clone();
        let extension = "jpg";
        let uuid = Uuid::new_v4();
        let filename = format!("image_{}_{}.{}", uuid, i, extension);
        processed_html = processed_html.replace(&src, &filename);
        let tx_m = tx_m.clone();
        let sq = *seq_id;
        tokio::spawn(async move {
            info!("Processing image: {}", src_clone);
            match download_image(&client, &src_clone).await {
                Ok((img_data, format)) => match resize_and_grayscale(img_data, format) {
                    Ok(processed_data) => {
                        let mime_type = "image/jpeg".to_string();
                        let cursor = Cursor::new(processed_data);
                        let res_part = EpubPart::Resource {
                            filename: filename,
                            content: Box::new(cursor),
                            mime_type,
                        };
                        let mut parts = Vec::new();
                        parts.push(res_part);
                        if let Err(_) = tx_m
                            .send(CompletionMessage {
                                sequence_id: sq,
                                parts,
                            })
                            .await
                        {
                            info!("Failed to send images {} (receiver might be closed)", i);
                        }
                        drop(tx_m);
                        Ok("Completed")
                    }
                    Err(_e) => {
                        if let Err(_) = tx_m
                            .send(CompletionMessage {
                                sequence_id: sq,
                                parts: Vec::new(),
                            })
                            .await
                        {
                            info!("Failed to send images {} (Error)", i);
                        }
                        drop(tx_m);
                        Err("Failed")
                    }
                },
                Err(_e) => {
                    if let Err(_) = tx_m
                        .send(CompletionMessage {
                            sequence_id: sq,
                            parts: Vec::new(),
                        })
                        .await
                    {
                        info!("Failed to send images {} (Error)", i);
                    }
                    drop(tx_m);
                    Err("Failed")
                }
            }
        });
    }

    (processed_html, total_images)
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

fn resize_and_grayscale(data: Vec<u8>, format: ImageFormat) -> Result<Vec<u8>> {
    let img = image::load_from_memory_with_format(&data, format)?;

    // Resize
    let resized = img.resize(600, 800, image::imageops::FilterType::Nearest);
    drop(img);
    let grayscale = resized.grayscale();
    drop(resized);
    // Encode to JPEG
    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    grayscale.write_to(&mut cursor, ImageFormat::Jpeg)?;

    Ok(buffer)
}
