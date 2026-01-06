use crate::epub_message::{CompletionMessage, EpubPart};
use anyhow::Result;
use image::ImageFormat;
use regex::Regex;
use reqwest::Client;
use std::any::Any;
use std::io::Cursor;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use tracing::{error, info};
use uuid::Uuid;

pub async fn process_images(
    html: &str,
    tx_m: &Sender<CompletionMessage>,
    seq_id: &usize,
    timeout_seconds: u64,
) -> (String, usize) {
    let mut processed_html = html.to_string();

    static IMG_REGEX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"<img[^>]+src="([^"]+)"[^>]*>"#).unwrap());

    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(Duration::from_secs(timeout_seconds))
        .build()
        .unwrap_or_else(|_| Client::new());

    let mut matches = Vec::new();
    //TODO: compare performance regex vs dom_query
    for cap in IMG_REGEX.captures_iter(html) {
        if let Some(src) = cap.get(1) {
            matches.push(src.as_str().to_string());
        }
    }

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
            match download_image(&client, &src_clone).await
            {
                    Ok((img_data, format)) => match resize_and_grayscale(img_data, format).await {
                        Ok(processed_data) => {
                            let mime_type = "image/jpeg".to_string();
                            let cursor = Cursor::new(processed_data);
                            let res_part = EpubPart::Resource {
                                filename,
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
                        Err(e) => {
                            error!(
                                "error while processing image {} with error {}",
                                src_clone, e
                            );
                          return  send_empty_message_on_error(i, tx_m, sq).await;
                        }
                    },
                    Err(_e) => {
                        return send_empty_message_on_error(i, tx_m, sq).await;
                    }
                }
        });
    }

    (processed_html, total_images)
}

async fn send_empty_message_on_error(i: usize, tx_m: Sender<CompletionMessage>, sq: usize) ->Result<&'static str, &'static str> {
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
    return Err("Failed");
}

async fn download_image(client: &Client, url: &str) -> Result<(Vec<u8>, ImageFormat)> {
    let resp = client.get(url).send().await?;
    //let _content_length = &resp.content_length().unwrap_or(0);
    let bytes = resp.bytes().await?.to_vec();

    //info!("Image size is {}  {}", content_length, &bytes.capacity());

    let format = image::guess_format(&bytes)?;

    Ok((bytes, format))
}

async fn resize_and_grayscale(data: Vec<u8>, format: ImageFormat) -> Result<Vec<u8>> {
    let handle = tokio::spawn(async move {
        let img = image::load_from_memory_with_format(&data, format)?;
        let resized = img.resize(600, 800, image::imageops::FilterType::Nearest);
        drop(img);
        let grayscale = resized.grayscale();
        drop(resized);

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        grayscale.write_to(&mut cursor, ImageFormat::Jpeg)?;
        Ok(buffer)
    });

    match handle.await {
        Ok(result) => result,
        Err(e) if e.is_panic() => {
            let msg = extract_panic_msg(e.into_panic());
            Err(anyhow::anyhow!("Tokio Task Panic: {}", msg))
        }
        Err(_) => Err(anyhow::anyhow!("Tokio Task Cancelled")),
    }
}
fn extract_panic_msg(payload: Box<dyn Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        return s.to_string();
    }
    if let Some(s) = payload.downcast_ref::<String>() {
        return s.clone();
    }
    "Unknown panic".to_string()
}
