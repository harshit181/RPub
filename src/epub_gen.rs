use crate::epub_message::EpubPart;
use crate::feed::Article;
use crate::image::process_images;
use anyhow::Result;
use chrono::Utc;
use epub_builder::{EpubBuilder, EpubContent, EpubVersion, ReferenceType, ZipLibrary};
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{Seek, SeekFrom, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::info;

pub async fn generate_epub_data<W: Write + Seek + Send + 'static>(
    articles: &[Article],
    output: W,
) -> Result<()> {
    use crate::epub_message::{CompletionMessage, EpubPart};
    use crate::util;
    use std::collections::HashMap;
    let mut articles_by_source: HashMap<String, Vec<&Article>> = HashMap::new();
    for article in articles {
        articles_by_source
            .entry(article.source.clone())
            .or_default()
            .push(article);
    }

    let mut sources: Vec<_> = articles_by_source.keys().cloned().collect();
    sources.sort();

    let mut article_filenames = HashMap::new();
    for (i, _article) in articles.iter().enumerate() {
        article_filenames.insert(i, format!("chapter_{}.xhtml", i));
    }

    let mut next_seq_id = 0;

    let master_toc_seq_id = 0;
    next_seq_id += 1;

    let mut source_toc_seq_ids = HashMap::new();
    let mut article_seq_ids = HashMap::new();

    for source in &sources {
        source_toc_seq_ids.insert(source.clone(), next_seq_id);
        next_seq_id += 1;

        for article in &articles_by_source[source] {
            let index = articles
                .iter()
                .position(|a| std::ptr::eq(a, *article))
                .unwrap();
            article_seq_ids.insert(index, next_seq_id);
            next_seq_id += 1;
        }
    }

    let total_parts = next_seq_id;
    info!("Total EPUB parts to write: {}", total_parts);

    let (tx, mut rx) = tokio::sync::mpsc::channel::<CompletionMessage>(32);
    let (tx_m, mut rx_m) = tokio::sync::mpsc::channel::<CompletionMessage>(32);
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_again = Arc::clone(&counter);
    let builder_handle = tokio::task::spawn_blocking(move || -> Result<()> {
        let mut builder =
            EpubBuilder::new(ZipLibrary::new().map_err(|e| anyhow::anyhow!("{}", e))?)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

        builder.epub_version(EpubVersion::V33);
        builder
            .metadata("author", "RSSPub RSS Book")
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        builder
            .metadata(
                "title",
                format!("RSS Digest - {}", Utc::now().format("%Y-%m-%d")),
            )
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let cover_path = util::COVER_LOCATION;
        if std::path::Path::new(cover_path).exists() {
            match std::fs::read(cover_path) {
                Ok(cover_data) => {
                    builder
                        .add_cover_image("cover.jpg", cover_data.as_slice(), "image/jpeg")
                        .map_err(|e| anyhow::anyhow!("Failed to add cover image: {}", e))?;
                }
                Err(e) => info!("Failed to read cover image: {}", e),
            }
        }

        let mut current_seq = 0;
        let mut buffer: HashMap<usize, Vec<EpubPart>> = HashMap::new();

        let pb = ProgressBar::new(total_parts as u64);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) Articles")
            .unwrap()
            .progress_chars("#>-"));

        while let Some(msg) = rx.blocking_recv() {
            buffer.insert(msg.sequence_id, msg.parts);
            while let Some(parts) = buffer.remove(&current_seq) {
                //info!("Writing sequence {} to EPUB", current_seq);
                populate_epub_data(&mut builder, parts)?;
                current_seq += 1;
                pb.inc(1);
            }

            if current_seq >= total_parts {
                pb.finish_with_message("Articles processed");
                info!("All parts received. Moving to images");
                break;
            }
        }
        current_seq = 0;
        let total_images = &counter_again.load(Ordering::Relaxed);
        info!("Total images are {}", &total_images);

        let pb_images = ProgressBar::new(*total_images as u64);
        pb_images.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) Images")
            .unwrap()
            .progress_chars("#>-"));

        while let Some(msg) = rx_m.blocking_recv() {
            //info!("Got image with seq id {} {}", msg.sequence_id, &current_seq);
            let parts = msg.parts;
            populate_epub_data(&mut builder, parts)?;
            current_seq += 1;
            pb_images.inc(1);
            if current_seq >= *total_images {
                pb_images.finish_with_message("Images processed");
                info!("All images received. Finishing EPUB.");
                break;
            }
        }

        builder
            .generate(output)
            .map_err(|e| anyhow::anyhow!("Failed to generate EPUB: {}", e))?;

        Ok(())
    });

    let mut master_toc_html = String::from("<h1>Table of Contents</h1><ul>");
    for source in &sources {
        let source_slug = source
            .replace(|c: char| !c.is_alphanumeric(), "_")
            .to_lowercase();
        let source_toc_filename = format!("toc_{}.xhtml", source_slug);
        master_toc_html.push_str(&format!(
            "<li><a href=\"{}\">{}</a></li>",
            source_toc_filename,
            util::escape_xml(source)
        ));
    }
    master_toc_html.push_str("</ul>");
    let master_toc_content =
        util::wrap_xhtml("Table of Contents", &util::fix_xhtml(&master_toc_html));

    tx.send(CompletionMessage {
        sequence_id: master_toc_seq_id,
        parts: vec![EpubPart::Content {
            filename: "toc.xhtml".to_string(),
            title: "Table of Contents".to_string(),
            content: master_toc_content,
            reftype: Some(ReferenceType::Toc),
        }],
    })
    .await
    .map_err(|_| anyhow::anyhow!("Failed to send Master TOC"))?;

    for source in &sources {
        let source_slug = source
            .replace(|c: char| !c.is_alphanumeric(), "_")
            .to_lowercase();
        let source_toc_filename = format!("toc_{}.xhtml", source_slug);
        let source_articles = &articles_by_source[source];

        let mut source_toc_html = format!(
            "<h1>{}</h1><p style='text-align: center;'><a href=\"toc.xhtml\">Back to Master TOC</a></p></br><ul>",
            util::escape_xml(source)
        );
        for article in source_articles {
            let index = articles
                .iter()
                .position(|a| std::ptr::eq(a, *article))
                .unwrap();
            let filename = &article_filenames[&index];
            source_toc_html.push_str(&format!(
                "<li><a href=\"{}\">{}</a></li>",
                filename,
                util::escape_xml(&article.title)
            ));
        }
        source_toc_html.push_str("</ul></br><p style='text-align: center;'><a href=\"toc.xhtml\">Back to Master TOC</a></p>");
        let source_toc_content = util::wrap_xhtml(source, &util::fix_xhtml(&source_toc_html));

        let seq_id = source_toc_seq_ids[source];
        tx.send(CompletionMessage {
            sequence_id: seq_id,
            parts: vec![EpubPart::Content {
                filename: source_toc_filename,
                title: source.clone(),
                content: source_toc_content,
                reftype: None,
            }],
        })
        .await
        .map_err(|_| anyhow::anyhow!("Failed to send Source TOC"))?;
    }

    let mut join_set = JoinSet::new();
    for (i, article) in articles.iter().enumerate() {
        let article = article.clone();
        let chapter_filename = article_filenames[&i].clone();
        let temp_log = article_filenames[&i].clone();
        let seq_id = article_seq_ids[&i];
        let tx = tx.clone();

        let source_slug = article
            .source
            .replace(|c: char| !c.is_alphanumeric(), "_")
            .to_lowercase();
        let back_link = format!("toc_{}.xhtml", source_slug);
        let tx_m = tx_m.clone();
        let counter_ref = Arc::clone(&counter);
        join_set.spawn(async move {
            let cleaned_content = util::clean_html(&article.content);
            let (processed_content,total_images_for_seq) = process_images(&cleaned_content,&tx_m,&seq_id).await;
            counter_ref.fetch_add(total_images_for_seq, Ordering::Relaxed);
            let fixed_content = util::fix_xhtml(&processed_content);
            let content_html = format!(
                "<h1>{}</h1><p><strong>Source:</strong> {} <br /> <strong>Date:</strong> {}</p><hr />{}<p><a href=\"{}\">Read original article</a></p><p><a href=\"{}\">Back to Feed TOC</a></p>",
                util::escape_xml(&article.title),
                util::escape_xml(&article.source),
                article.pub_date.format("%Y-%m-%d %H:%M"),
                fixed_content,
                util::escape_xml(&article.link),
                back_link
            );
            let final_content = util::wrap_xhtml(&article.title, &content_html);

            let mut parts = Vec::new();

            parts.push(EpubPart::Content {
                filename: chapter_filename,
                title: article.title,
                content: final_content,
                reftype: None,
            });
                info!("Sending Completed Part {}", temp_log);
            if let Err(_) = tx.send(CompletionMessage {
                sequence_id: seq_id,
                parts,
            }).await {
                info!("Failed to send article {} (receiver might be closed)", i);
            }
        });
    }
    drop(tx);

    while let Some(res) = join_set.join_next().await {
        if let Err(e) = res {
            info!("Article processing task failed: {}", e);
        }
    }

    builder_handle
        .await
        .map_err(|e| anyhow::anyhow!("Builder task joined error: {}", e))??;

    info!("EPUB generated successfully");
    Ok(())
}

fn populate_epub_data(builder: &mut EpubBuilder<ZipLibrary>, parts: Vec<EpubPart>) -> Result<()> {
    for part in parts {
        match part {
            EpubPart::Content {
                filename,
                title,
                content,
                reftype,
            } => {
                let mut content = EpubContent::new(filename, content.as_bytes()).title(title);
                if let Some(rt) = reftype {
                    content = content.reftype(rt);
                }
                builder
                    .add_content(content)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
            }
            EpubPart::Resource {
                filename,
                mut content,
                mime_type,
            } => {
                content.seek(SeekFrom::Start(0))?;
                builder
                    .add_resource(filename, content, mime_type)
                    .map_err(|e| anyhow::anyhow!("Failed to add resource: {}", e))?;
            }
        }
    }
    Ok(())
}
