use crate::db::EmailConfig;
use anyhow::{Context, Result};
use lettre::message::header::ContentType;
use lettre::message::{Attachment, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::AsyncSmtpTransport;
use lettre::Tokio1Executor;
use lettre::{AsyncTransport, Message};
use std::fs;
use std::path::Path;
use std::time::Duration;
use tracing::{error, info};

pub async fn send_epub(config: &EmailConfig, epub_path: &Path) -> Result<()> {
    info!("Preparing to send email to {}", config.to_email);

    let filename = epub_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("digest.epub");

    let filebody = fs::read(epub_path).context("Failed to read EPUB file")?;
    let content_type = ContentType::parse("application/epub+zip").unwrap();

    let attachment = Attachment::new(String::from(filename)).body(filebody, content_type);

    let email = Message::builder()
        .from(
            config
                .email_address
                .parse()
                .context("Invalid 'from' email")?,
        )
        .to(config.to_email.parse().context("Invalid 'to' email")?)
        .subject(format!("RSS Digest: {}", filename))
        .multipart(
            MultiPart::mixed()
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_PLAIN)
                        .body(String::from("Here is your requested RSS Digest EPUB.")),
                )
                .singlepart(attachment),
        )
        .context("Failed to build email")?;

    let creds = Credentials::new(config.email_address.clone(), config.smtp_password.clone());

    info!(
        "Sending email via {}:{}...",
        config.smtp_host, config.smtp_port
    );

    let mailer: AsyncSmtpTransport<Tokio1Executor> =
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.smtp_host)
            .context("Failed to create SMTP transport")?
            .port(config.smtp_port)
            .credentials(creds)
            .timeout(Some(Duration::from_mins(3)))
            .build();

    //mailer.send(email).await.context("Failed to send email")?;
    match mailer.send(email).await {
        Ok(_x) => {
            info!("Email sent successfully!");
        }
        Err(y) => {
            error!("Failed to send email: {}", y);
        }
    }

    Ok(())
}
