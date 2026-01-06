use crate::models::{CustomExtractorConfig, ContentProcessor, OutputMode, ProcessorType};
use arc_swap::ArcSwap;
use dom_query::Document;
use dom_smoothie::{CandidateSelectMode, Config, TextMode};
use reqwest::Client;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

static DOMAIN_OVERRIDES: OnceLock<ArcSwap<HashMap<String, Arc<ContentProcessor>>>> = OnceLock::new();

pub fn refresh_domain_overrides(overrides: Vec<(String, ContentProcessor)>) {
    let map: HashMap<String, Arc<ContentProcessor>> = overrides
        .into_iter()
        .map(|(k, v)| (k, Arc::new(v)))
        .collect();
    
    match DOMAIN_OVERRIDES.get() {
        Some(swap) => swap.store(Arc::new(map)),
        None => { let _ = DOMAIN_OVERRIDES.set(ArcSwap::from_pointee(map)); }
    }
}

pub fn get_domain_override(url: &str) -> Option<Arc<ContentProcessor>> {
    let domain = extract_domain(url)?;
    DOMAIN_OVERRIDES.get()?.load().get(&domain).cloned()
}

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

pub struct TextOnlyExtractor;

impl ContentExtractor for TextOnlyExtractor {
    fn extract(&self, html: &str, url: &str) -> anyhow::Result<(String, String)> {
        let cfg = Config {
            text_mode: TextMode::Markdown,
            candidate_select_mode: CandidateSelectMode::DomSmoothie,
            ..Default::default()
        };
        let mut readability = dom_smoothie::Readability::new(html, Some(url), Some(cfg))?;
        let extracted = readability
            .parse()
            .map_err(|e| anyhow::anyhow!("TextOnly error: {:?}", e))?;
        
        let content_html = extracted.content.to_string();
        let doc = Document::from(content_html.as_str());
        if let Some(images) = doc.try_select("img") {
            images.remove();
        }
        
        Ok((extracted.title, doc.html().to_string()))
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
        let use_text_mode = self.config.output_mode == OutputMode::Text;

        let document = Document::from(html);

        let title = match document.try_select("title") {
            None => {"Untitled".to_string()},
            Some(x) => {x.text().to_string()}
        };


        let cleaned_html = html.to_string();
        let doc = Document::from(cleaned_html.as_str());
        let discard_selector =&self.config.discard.join(", ");
        let selector =&self.config.selector.join(", ");
        match doc.try_select(discard_selector) {
            None => {}
            Some(dd) => {dd.remove()}
        };
        let mut selected_content =doc.try_select(selector);
        let mut content=String::new();

        while let Some(selected) =selected_content{
            if use_text_mode {
                content=selected.text().to_string();
            }
            else{
                let temp=selected.html().to_string();
                content=format!("{} {}",content,temp);
            }
            selected.select(selector).remove();
            selected_content=selected.try_select(selector);
        }
        let content=content.to_string();

        Ok((title, content))
    }
}
pub fn create_extractor(processor: Option<&ContentProcessor>) -> anyhow::Result<Box<dyn ContentExtractor>> {
    let processor_type = processor.map(|p| p.processor).unwrap_or(ProcessorType::Default);

    match processor_type {
        ProcessorType::DomSmoothie => Ok(Box::new(DomSmoothieExtractor)),
        ProcessorType::TextOnly => Ok(Box::new(TextOnlyExtractor)),
        ProcessorType::Custom => {
            let custom_config = processor
                .and_then(|p| p.custom_config.as_ref())
                .ok_or_else(|| anyhow::anyhow!("Custom processor requires custom_config"))?;
            Ok(Box::new(CustomExtractor::new(custom_config)?))
        }
        ProcessorType::Default => Ok(Box::new(DefaultExtractor)),
    }
}

pub fn extract_domain(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
}

pub async fn fetch_full_content(client: &Client, url: &str) -> anyhow::Result<(String, String)> {
    fetch_full_content_with_processor(client, url, None).await
}
pub async fn fetch_full_content_with_processor(
    client: &Client,
    url: &str,
    processor: Option<&ContentProcessor>,
) -> anyhow::Result<(String, String)> {
    let html = client.get(url).send().await?.text().await?;

    let extractor = if let Some(content_processor) = get_domain_override(url) {
        create_extractor(Some(&*content_processor))?
    } else {
        create_extractor(processor)?
    };
    
    extractor.extract(&html, url)
}