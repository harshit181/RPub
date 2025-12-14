use regex::Regex;
use ammonia::Builder;

pub const EPUBS_OUTPUT_DIR: &str = "static/epubs";

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

    let attr_regex = Regex::new(r#"\b(alt|title)\s*=\s*(?:"([^"]*)"|'([^']*)')"#).unwrap();
    fixed = attr_regex
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

    let img_regex = Regex::new(r"<img([^>]*[^/])>").unwrap();
    fixed = img_regex.replace_all(&fixed, "<img$1 />").to_string();

    let br_regex = Regex::new(r"<br\s*>").unwrap();
    fixed = br_regex.replace_all(&fixed, "<br />").to_string();

    let hr_regex = Regex::new(r"<hr\s*>").unwrap();
    fixed = hr_regex.replace_all(&fixed, "<hr />").to_string();

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