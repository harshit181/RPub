use serde::Deserialize;
use std::fs;
use anyhow::Context;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub feeds: Vec<String>,
    pub output_dir: String,
}

pub fn load_config(path: &str) -> anyhow::Result<Config> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file at {}", path))?;
    let config: Config = serde_json::from_str(&content)
        .with_context(|| "Failed to parse config JSON")?;
    Ok(config)
}
