use crate::feed::Article;
use crate::image::process_images;
use ammonia::Builder;
use anyhow::Result;
use chrono::Utc;
use epub_builder::{EpubBuilder, EpubContent, EpubVersion, ReferenceType, ZipLibrary};
use regex::Regex;
use tokio::task::JoinSet;
use tracing::info;

pub async fn generate_epub_data(articles: &[Article]) -> Result<Vec<u8>> {
    // Group articles by source
    use std::collections::HashMap;
    let mut articles_by_source: HashMap<String, Vec<&Article>> = HashMap::new();
    for article in articles {
        articles_by_source
            .entry(article.source.clone())
            .or_default()
            .push(article);
    }

    // Sort sources for consistent order
    let mut sources: Vec<_> = articles_by_source.keys().cloned().collect();
    sources.sort();

    // Assign filenames to all articles.
    let mut article_filenames = HashMap::new();
    for (i, _article) in articles.iter().enumerate() {
        article_filenames.insert(i, format!("chapter_{}.xhtml", i));
    }

    // Master TOC
    let mut master_toc_html = String::from("<h1>Table of Contents</h1><ul>");

    for source in &sources {
        let source_slug = source
            .replace(|c: char| !c.is_alphanumeric(), "_")
            .to_lowercase();
        let source_toc_filename = format!("toc_{}.xhtml", source_slug);

        master_toc_html.push_str(&format!(
            "<li><a href=\"{}\">{}</a></li>",
            source_toc_filename,
            escape_xml(source)
        ));
    }
    master_toc_html.push_str("</ul>");

    // Wrap Master TOC
    let master_toc_content = wrap_xhtml("Table of Contents", &fix_xhtml(&master_toc_html));

    // Process Chapters - Parallel Processing
    let mut join_set = JoinSet::new();

    for (i, article) in articles.iter().enumerate() {
        let article = article.clone(); // Clone for the task
        let chapter_filename = article_filenames[&i].clone();

        join_set.spawn(async move {
            // Clean content first
            let cleaned_content = clean_html(&article.content);

            // Process images in content
            let (processed_content, images) = process_images(&cleaned_content).await;

            // Fix XHTML (close void tags)
            let fixed_content = fix_xhtml(&processed_content);

            // Wrap in XHTML skeleton
            let content_html = format!(
                "<h1>{}</h1><p><strong>Source:</strong> {} <br /> <strong>Date:</strong> {}</p><hr />{}<p><a href=\"{}\">Read original article</a></p><p><a href=\"{}\">Back to Feed TOC</a></p>",
                escape_xml(&article.title),
                escape_xml(&article.source),
                article.pub_date.format("%Y-%m-%d %H:%M"),
                fixed_content,
                escape_xml(&article.link),
                format!("toc_{}.xhtml", article.source.replace(|c: char| !c.is_alphanumeric(), "_").to_lowercase())
            );
            let final_content = wrap_xhtml(&article.title, &content_html);

            (i, article, chapter_filename, final_content, images)
        });
    }

    // Collect results
    let mut processed_articles_map = HashMap::new();
    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(result) => {
                processed_articles_map.insert(result.0, result);
            }
            Err(e) => info!("Task join error: {}", e),
        }
    }

    // Initialize builder after async tasks are done to avoid Send issues
    let mut builder = EpubBuilder::new(ZipLibrary::new().map_err(|e| anyhow::anyhow!("{}", e))?)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    // Set metadata
    builder.epub_version(EpubVersion::V33);
    builder
        .metadata("author", "RPub RSS Book")
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    builder
        .metadata(
            "title",
            format!("RSS Digest - {}", Utc::now().format("%Y-%m-%d")),
        )
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Add cover image
    info!("Adding cover image");
    let cover_path = "static/cover.jpg";
    if std::path::Path::new(cover_path).exists() {
        match std::fs::read(cover_path) {
            Ok(cover_data) => {
                info!("Added cover image");
                builder
                    .add_cover_image("cover.jpg", cover_data.as_slice(), "image/jpeg")
                    .map_err(|e| anyhow::anyhow!("Failed to add cover image: {}", e))?;
            }
            Err(e) => {
                info!("Failed to read cover image: {}", e);
            }
        }
    } else {
        info!("Cover image not found at {}", cover_path);
    }

    builder
        .add_content(
            EpubContent::new("toc.xhtml", master_toc_content.as_bytes())
                .title("Table of Contents")
                .reftype(ReferenceType::Toc),
        )
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Source TOCs and Chapters
    for source in &sources {
        let source_slug = source
            .replace(|c: char| !c.is_alphanumeric(), "_")
            .to_lowercase();
        let source_toc_filename = format!("toc_{}.xhtml", source_slug);
        let source_articles = &articles_by_source[source];

        let mut source_toc_html = format!("<h1>{}</h1><ul>", escape_xml(source));

        for article in source_articles {
            // Find index in original list to get filename
            let index = articles
                .iter()
                .position(|a| std::ptr::eq(a, *article))
                .unwrap();
            let filename = &article_filenames[&index];

            source_toc_html.push_str(&format!(
                "<li><a href=\"{}\">{}</a></li>",
                filename,
                escape_xml(&article.title)
            ));
        }
        source_toc_html.push_str("</ul>");

        // Wrap Source TOC
        let source_toc_content = wrap_xhtml(source, &fix_xhtml(&source_toc_html));

        builder
            .add_content(
                EpubContent::new(source_toc_filename, source_toc_content.as_bytes()).title(source),
            )
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        // Add Chapters for this Source
        for article in source_articles {
            let index = articles
                .iter()
                .position(|a| std::ptr::eq(a, *article))
                .unwrap();

            if let Some((_i, article, chapter_filename, processed_content, images)) =
                processed_articles_map.remove(&index)
            {
                // Add images to EPUB
                for (img_filename, img_data, mime_type) in images {
                    builder
                        .add_resource(img_filename, img_data.as_slice(), mime_type)
                        .map_err(|e| anyhow::anyhow!("Failed to add image resource: {}", e))?;
                }

                builder
                    .add_content(
                        EpubContent::new(chapter_filename, processed_content.as_bytes())
                            .title(&article.title),
                    )
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
            } else {
                info!("Skipping article {} due to processing error", index);
            }
        }
    }

    let mut buffer = Vec::new();
    builder
        .generate(&mut buffer)
        .map_err(|e| anyhow::anyhow!("Failed to generate EPUB: {}", e))?;
    info!("EPUB generated successfully");
    Ok(buffer)
}

fn clean_html(html: &str) -> String {
    let mut builder = Builder::new();
    // Configure ammonia to keep images and basic formatting
    builder.add_tags(&[
        "img",
        "p",
        "br",
        "b",
        "i",
        "strong",
        "em",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "ul",
        "ol",
        "li",
        "blockquote",
        "hr",
        "a",
        "div",
        "span",
    ]);
    builder.add_generic_attributes(&["src", "href", "alt", "title", "class", "id"]);
    builder.clean(html).to_string()
}

fn fix_xhtml(html: &str) -> String {
    let mut fixed = html.to_string();

    // Fix unescaped ampersands
    // Matches & optionally followed by a valid entity body
    let amp_regex = Regex::new(r"&([a-zA-Z][a-zA-Z0-9]*;|#\d+;|#x[0-9a-fA-F]+;)?").unwrap();
    fixed = amp_regex
        .replace_all(&fixed, |caps: &regex::Captures| {
            if caps.get(1).is_some() {
                caps[0].to_string()
            } else {
                "&amp;".to_string()
            }
        })
        .to_string();

    // Fix unescaped < in attributes (specifically alt and title)
    // Regex matches: attribute_name="value" or attribute_name='value'
    // Rust regex doesn't support backreferences, so we match both quote types separately
    let attr_regex = Regex::new(r#"\b(alt|title)\s*=\s*(?:"([^"]*)"|'([^']*)')"#).unwrap();
    fixed = attr_regex
        .replace_all(&fixed, |caps: &regex::Captures| {
            let attr_name = &caps[1];
            // Check which group matched
            let (quote, value) = if let Some(val) = caps.get(2) {
                ("\"", val.as_str())
            } else {
                ("'", caps.get(3).unwrap().as_str())
            };

            let escaped_value = value.replace("<", "&lt;");
            format!("{}={}{}{}", attr_name, quote, escaped_value, quote)
        })
        .to_string();

    // Fix img: <img ... > (without / before >)
    // Regex: <img([^>]*[^/])>
    let img_regex = Regex::new(r"<img([^>]*[^/])>").unwrap();
    fixed = img_regex.replace_all(&fixed, "<img$1 />").to_string();

    // Fix br: <br>
    let br_regex = Regex::new(r"<br\s*>").unwrap();
    fixed = br_regex.replace_all(&fixed, "<br />").to_string();

    // Fix hr: <hr>
    let hr_regex = Regex::new(r"<hr\s*>").unwrap();
    fixed = hr_regex.replace_all(&fixed, "<hr />").to_string();

    fixed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_html_attributes() {
        let html = r#"<img src="test.jpg" alt="foo < bar">"#;
        // clean_html doesn't escape < in attributes, but fix_xhtml should
        let cleaned = clean_html(html);
        let fixed = fix_xhtml(&cleaned);
        assert_eq!(fixed, r#"<img src="test.jpg" alt="foo &lt; bar" />"#);
    }

    #[test]
    fn test_fix_xhtml_ampersands() {
        let cases = vec![
            ("Foo & Bar", "Foo &amp; Bar"),
            ("Foo &amp; Bar", "Foo &amp; Bar"),
            ("AT&T", "AT&amp;T"),
            ("Q&A", "Q&amp;A"),
            (
                "http://example.com?a=1&b=2",
                "http://example.com?a=1&amp;b=2",
            ),
            (
                "http://example.com?a=1&amp;b=2",
                "http://example.com?a=1&amp;b=2",
            ),
            (
                "ValuePickr Forum & Latest Posts",
                "ValuePickr Forum &amp; Latest Posts",
            ),
            ("&utm_medium=rss", "&amp;utm_medium=rss"),
            ("&#123;", "&#123;"),
            ("&#xAB;", "&#xAB;"),
            ("&nbsp;", "&nbsp;"),
            ("&T", "&amp;T"),
        ];

        for (input, expected) in cases {
            let result = fix_xhtml(input);
            assert_eq!(result, expected, "Input: {}", input);
        }
    }
}

fn wrap_xhtml(title: &str, content: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd">
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
    <title>{}</title>
    <link rel="stylesheet" type="text/css" href="stylesheet.css" />
</head>
<body>
{}
</body>
</html>"#,
        escape_xml(title),
        content
    )
}

fn escape_xml(s: &str) -> String {
    s.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")
        .replace("'", "&apos;")
}
