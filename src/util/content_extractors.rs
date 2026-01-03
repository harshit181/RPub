use crate::models::{CustomExtractorConfig, FeedProcessor, ProcessorType};
use dom_query::Document;
use dom_smoothie::{CandidateSelectMode, Config, TextMode};
use reqwest::Client;

pub trait ContentExtractor: Send + Sync {
    fn extract(&self, html: &str, url: &str) -> anyhow::Result<(String, String)>;
}

pub struct DefaultExtractor;

impl ContentExtractor for DefaultExtractor {
    fn extract(&self, html: &str, url: &str) -> anyhow::Result<(String, String)> {
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
}

pub struct DomSmoothieExtractor;

impl ContentExtractor for DomSmoothieExtractor {
    fn extract(&self, html: &str, url: &str) -> anyhow::Result<(String, String)> {
        let cfg = Config {
            text_mode: TextMode::Markdown,
            candidate_select_mode: CandidateSelectMode::DomSmoothie,
            ..Default::default()
        };
        let mut readability = dom_smoothie::Readability::new(html, Some(url), Some(cfg))?;
        let extracted = readability
            .parse()
            .map_err(|e| anyhow::anyhow!("DomSmoothie error: {:?}", e))?;
        Ok((extracted.title, extracted.content.to_string()))
    }
}

pub struct CustomExtractor {
    pub config: CustomExtractorConfig,
}

impl CustomExtractor {
    pub fn new(yaml_config: &str) -> anyhow::Result<Self> {
        let config: CustomExtractorConfig = serde_yaml::from_str(yaml_config)
            .map_err(|e| anyhow::anyhow!("Invalid YAML config: {}", e))?;
        Ok(Self { config })
    }
}

impl ContentExtractor for CustomExtractor {
    fn extract(&self, html: &str, _url: &str) -> anyhow::Result<(String, String)> {
        let use_text_mode = self.config.output_mode.to_lowercase() == "text";

        let document = Document::from(html);

        let title = match document.try_select("title") {
            None => {"Untitled".to_string()},
            Some(x) => {x.text().to_string()}
        };


        let cleaned_html = html.to_string();
        let doc = Document::from(cleaned_html.as_str());
        let discard_selector =&self.config.discard.join(", ");
        let selector =&self.config.selector.join(", ");
        doc.select(discard_selector).remove();
        match doc.try_select(discard_selector) {
            None => {}
            Some(dd) => {dd.remove()}
        };
        let selected_content=doc.try_select(selector);
        let mut content=String::new();

        match selected_content {
            None => {return Err(anyhow::anyhow!("Not able to extract content via Yaml config "));},
            Some(selected) => {
                if use_text_mode {
                    content=selected.text().to_string();
                }
                else{
                    for x in selected.iter() {
                        content=format!("{} {}",content,x.html().to_string());
                    }

                }
            }
        }
        let content=content.to_string();

        Ok((title, content))
    }
}

pub fn create_extractor(processor: Option<&FeedProcessor>) -> anyhow::Result<Box<dyn ContentExtractor>> {
    let processor_type = processor.map(|p| p.processor).unwrap_or(ProcessorType::Default);

    match processor_type {
        ProcessorType::DomSmoothie => Ok(Box::new(DomSmoothieExtractor)),
        ProcessorType::Custom => {
            let custom_config = processor
                .and_then(|p| p.custom_config.as_ref())
                .ok_or_else(|| anyhow::anyhow!("Custom processor requires custom_config"))?;
            Ok(Box::new(CustomExtractor::new(custom_config)?))
        }
        ProcessorType::Default => Ok(Box::new(DefaultExtractor)),
    }
}

pub async fn fetch_full_content(client: &Client, url: &str) -> anyhow::Result<(String, String)> {
    fetch_full_content_with_processor(client, url, None).await
}

pub async fn fetch_full_content_with_processor(
    client: &Client,
    url: &str,
    processor: Option<&FeedProcessor>,
) -> anyhow::Result<(String, String)> {
    let html = client.get(url).send().await?.text().await?;
    let extractor = create_extractor(processor)?;
    extractor.extract(&html, url)
}