use anyhow::Result;
use chrono::{DateTime, Utc};
use tokio::fs;

pub async fn generate_opds_feed(base_url: &str, dir_path: &str) -> Result<String> {
    let mut entries = Vec::new();
    let mut dir = fs::read_dir(dir_path).await?;

    while let Some(entry) = dir.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("epub") {
            if let Ok(metadata) = fs::metadata(&path).await {
                if let Ok(modified) = metadata.modified() {
                    let modified: DateTime<Utc> = modified.into();
                    let filename = path.file_name().unwrap().to_string_lossy().to_string();
                    entries.push((filename, modified));
                }
            }
        }
    }

    entries.sort_by(|a, b| b.1.cmp(&a.1));

    let updated = if let Some(first) = entries.first() {
        first.1.to_rfc3339()
    } else {
        Utc::now().to_rfc3339()
    };

    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:dc="http://purl.org/dc/terms/" xmlns:opds="http://opds-spec.org/2010/catalog">
  <id>urn:uuid:rpub-feed</id>
  <title>RPub RSS Digest</title>
  <author>
    <name>RPub</name>
  </author>
"#,
    );

    xml.push_str(&format!("  <updated>{}</updated>\n", updated));
    xml.push_str(&format!("  <link rel=\"self\" href=\"{}/opds\" type=\"application/atom+xml;profile=opds-catalog;kind=navigation\"/>\n", base_url));
    xml.push_str(&format!("  <link rel=\"start\" href=\"{}/opds\" type=\"application/atom+xml;profile=opds-catalog;kind=navigation\"/>\n", base_url));

    for (filename, modified) in entries {
        let date_str = modified.format("%Y-%m-%d").to_string();
        let download_url = format!("{}/epubs/{}", base_url, filename);

        xml.push_str("  <entry>\n");
        xml.push_str(&format!("    <title>RSS Digest - {}</title>\n", date_str));
        xml.push_str(&format!("    <id>urn:rpub:epub:{}</id>\n", filename));
        xml.push_str(&format!(
            "    <updated>{}</updated>\n",
            modified.to_rfc3339()
        ));
        xml.push_str(&format!(
            "    <content type=\"text\">RSS Digest for {}</content>\n",
            date_str
        ));
        xml.push_str(&format!("    <link rel=\"http://opds-spec.org/acquisition\" href=\"{}\" type=\"application/epub+zip\" />\n", download_url));
        xml.push_str("  </entry>\n");
    }

    xml.push_str("</feed>");

    Ok(xml)
}
