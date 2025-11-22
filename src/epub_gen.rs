use crate::feed::Article;
use anyhow::{Context, Result};
use epub_builder::{EpubBuilder, EpubContent, ReferenceType, ZipLibrary};
use std::fs::{self, File};
use std::path::Path;
use chrono::Utc;
use tracing::info;

pub fn generate_epub_data(articles: &[Article]) -> Result<Vec<u8>> {
    let mut builder = EpubBuilder::new(ZipLibrary::new().map_err(|e| anyhow::anyhow!("{}", e))?).map_err(|e| anyhow::anyhow!("{}", e))?;

    // Set metadata
    builder.metadata("author", "RPub RSS Aggregator").map_err(|e| anyhow::anyhow!("{}", e))?;
    builder.metadata("title", format!("RSS Digest - {}", Utc::now().format("%Y-%m-%d"))).map_err(|e| anyhow::anyhow!("{}", e))?;

    // Group articles by source
    use std::collections::HashMap;
    let mut articles_by_source: HashMap<String, Vec<&Article>> = HashMap::new();
    for article in articles {
        articles_by_source.entry(article.source.clone()).or_default().push(article);
    }

    // Create Master TOC content
    let mut master_toc_html = String::from("<h1>Table of Contents</h1><ul>");
    
    // Sort sources for consistent order
    let mut sources: Vec<_> = articles_by_source.keys().cloned().collect();
    sources.sort();


    master_toc_html.push_str("</ul>");
    
    // Re-plan:
    // 1. Assign filenames to all articles.
    // 2. Build Master TOC and Source TOCs.
    // 3. Add all content.

    let mut article_filenames = HashMap::new();
    for (i, _article) in articles.iter().enumerate() {
        article_filenames.insert(i, format!("chapter_{}.xhtml", i));
    }

    // Master TOC
    let mut master_toc_html = String::from("<h1>Table of Contents</h1><ul>");
    
    for source in &sources {
        let source_slug = source.replace(|c: char| !c.is_alphanumeric(), "_").to_lowercase();
        let source_toc_filename = format!("toc_{}.xhtml", source_slug);
        
        master_toc_html.push_str(&format!(
            "<li><a href=\"{}\">{}</a></li>",
            source_toc_filename, source
        ));
    }
    master_toc_html.push_str("</ul>");

    builder.add_content(
        EpubContent::new("toc.xhtml", master_toc_html.as_bytes())
            .title("Table of Contents")
            .reftype(ReferenceType::Toc),
    ).map_err(|e| anyhow::anyhow!("{}", e))?;

    // Source TOCs and Chapters
    for source in &sources {
        let source_slug = source.replace(|c: char| !c.is_alphanumeric(), "_").to_lowercase();
        let source_toc_filename = format!("toc_{}.xhtml", source_slug);
        let source_articles = &articles_by_source[source];

        let mut source_toc_html = format!("<h1>{}</h1><ul>", source);
        
        for article in source_articles {
            // Find index in original list to get filename
            let index = articles.iter().position(|a| std::ptr::eq(a, *article)).unwrap();
            let filename = &article_filenames[&index];
            
            source_toc_html.push_str(&format!(
                "<li><a href=\"{}\">{}</a></li>",
                filename, article.title
            ));
        }
        source_toc_html.push_str("</ul>");

        builder.add_content(
            EpubContent::new(source_toc_filename, source_toc_html.as_bytes())
                .title(source)
        ).map_err(|e| anyhow::anyhow!("{}", e))?;
    }

    // Add Chapters
    for (i, article) in articles.iter().enumerate() {
        let chapter_filename = &article_filenames[&i];
        
        let content_html = format!(
            "<h1>{}</h1><p><strong>Source:</strong> {} <br/> <strong>Date:</strong> {}</p><hr/>{}<p><a href=\"{}\">Read original article</a></p>",
            article.title,
            article.source,
            article.pub_date.format("%Y-%m-%d %H:%M"),
            article.content,
            article.link
        );

        builder.add_content(
            EpubContent::new(chapter_filename, content_html.as_bytes())
                .title(&article.title)
        ).map_err(|e| anyhow::anyhow!("{}", e))?;
    }


    let mut buffer = Vec::new();
    builder.generate(&mut buffer).map_err(|e| anyhow::anyhow!("Failed to generate EPUB: {}", e))?;

    Ok(buffer)
}

pub fn generate_epub(articles: &[Article], output_dir: &str) -> Result<()> {
    let data = generate_epub_data(articles)?;

    // Ensure output directory exists
    fs::create_dir_all(output_dir).context("Failed to create output directory")?;

    // Generate filename
    let filename = format!("rss_digest_{}.epub", Utc::now().format("%Y%m%d_%H%M%S"));
    let output_path = Path::new(output_dir).join(filename);
    
    fs::write(&output_path, data).context("Failed to write output file")?;

    info!("Generated EPUB at: {:?}", output_path);

    Ok(())
}

