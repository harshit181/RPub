use std::sync::LazyLock;
use regex::Regex;
use ammonia::Builder;
use reqwest::Client;
use dom_smoothie::{Config, TextMode};

pub const EPUB_OUTPUT_DIR: &str = "static/epubs";
pub const COVER_LOCATION: &str = "static/cover.jpg";
pub fn clean_html(html: &str) -> String {
    let mut builder = Builder::new();
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

pub fn fix_xhtml(html: &str) -> String {
    let mut fixed = html.to_string();
    static AMP_REGEX: LazyLock<Regex> =LazyLock::new(|| Regex::new(r"&([a-zA-Z][a-zA-Z0-9]*;|#\d+;|#x[0-9a-fA-F]+;)?").unwrap());
    fixed = AMP_REGEX
        .replace_all(&fixed, |caps: &regex::Captures| {
            if caps.get(1).is_some() {
                caps[0].to_string()
            } else {
                "&amp;".to_string()
            }
        })
        .to_string();
    static ATTR_REGEX: LazyLock<Regex> =LazyLock::new(|| Regex::new(r#"\b(alt|title)\s*=\s*(?:"([^"]*)"|'([^']*)')"#).unwrap());
    fixed = ATTR_REGEX
        .replace_all(&fixed, |caps: &regex::Captures| {
            let attr_name = &caps[1];

            let (quote, value) = if let Some(val) = caps.get(2) {
                ("\"", val.as_str())
            } else {
                ("'", caps.get(3).unwrap().as_str())
            };

            let escaped_value = value.replace("<", "&lt;");
            format!("{}={}{}{}", attr_name, quote, escaped_value, quote)
        })
        .to_string();
    static IMG_REGEX: LazyLock<Regex> =LazyLock::new(|| Regex::new(r"<img([^>]*[^/])>").unwrap());
    fixed = IMG_REGEX.replace_all(&fixed, "<img$1 />").to_string();
    static BR_REGEX: LazyLock<Regex> =LazyLock::new(|| Regex::new(r"<br\s*>").unwrap());
    fixed = BR_REGEX.replace_all(&fixed, "<br />").to_string();
    static HR_REGEX: LazyLock<Regex> =LazyLock::new(|| Regex::new(r"<hr\s*>").unwrap());
    fixed = HR_REGEX.replace_all(&fixed, "<hr />").to_string();

    fixed
}

pub fn wrap_xhtml(title: &str, content: &str) -> String {
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

pub fn escape_xml(s: &str) -> String {
    s.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")
        .replace("'", "&apos;")
}

pub async fn fetch_full_content(client: &Client, url: &str) -> anyhow::Result<(String, String)> {
    let html = client.get(url).send().await?.text().await?;
    let cfg = Config {
        text_mode: TextMode::Markdown,
        ..Default::default()
    };
    let mut readability = dom_smoothie::Readability::new(html, Some(url), Some(cfg))?;
    let extracted = readability
        .parse()
        .map_err(|e| anyhow::anyhow!("DomSmoothie error: {:?}", e))?;
    Ok((extracted.title, extracted.content.to_string()))
}