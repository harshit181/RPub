use crate::opds;
use crate::util;
use axum::{
    extract::{ Multipart},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
};
use tokio::io::AsyncWriteExt;
use tracing::{info};

pub mod download_handler;
pub mod feed_handler;
pub mod schedule_handler;
pub mod read_it_later_handler;
pub mod auth_handler;
pub mod email_handler;
pub mod config_handler;

pub async fn opds_handler(headers: HeaderMap) -> Result<impl IntoResponse, (StatusCode, String)> {
    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("127.0.0.1:3000");

    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("http");

    let base_url = format!("{}://{}", scheme, host);

    let xml = opds::generate_opds_feed(&base_url, util::EPUB_OUTPUT_DIR)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        header::CONTENT_TYPE,
        "application/atom+xml;profile=opds-catalog;kind=navigation"
            .parse()
            .unwrap(),
    );

    Ok((response_headers, xml))
}

pub async fn upload_cover(mut multipart: Multipart) -> Result<StatusCode, (StatusCode, String)> {
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("Failed to read multipart field: {}", e),
        )
    })? {
        let name = field.name().unwrap_or("").to_string();
        if name == "cover" {
            let data = field.bytes().await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to read field bytes: {}", e),
                )
            })?;

            if data.is_empty() {
                return Err((StatusCode::BAD_REQUEST, "Empty file".to_string()));
            }

            let path = util::COVER_LOCATION;
            let mut file = tokio::fs::File::create(path).await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to create file: {}", e),
                )
            })?;

            file.write_all(&data).await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to write file: {}", e),
                )
            })?;

            info!("Cover image updated successfully");
            return Ok(StatusCode::OK);
        }
    }

    Err((StatusCode::BAD_REQUEST, "No cover file found".to_string()))
}

