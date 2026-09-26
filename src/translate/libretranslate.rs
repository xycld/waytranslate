//! LibreTranslate — self-hostable open translation server.
//! Default points at localhost; public instances go in config base_url.

use super::{Lang, Translator};

pub struct LibreTranslate {
    http: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl LibreTranslate {
    pub fn new(http: reqwest::Client, base_url: String, api_key: Option<String>) -> Self {
        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        }
    }
}

#[async_trait::async_trait]
impl Translator for LibreTranslate {
    async fn translate(&self, text: &str, to: Lang) -> anyhow::Result<String> {
        let target = match to {
            Lang::Zh => "zh",
            Lang::En => "en",
        };
        let mut body = serde_json::json!({
            "q": text,
            "source": "auto",
            "target": target,
        });
        if let Some(key) = &self.api_key {
            body["api_key"] = serde_json::Value::String(key.clone());
        }
        let resp: serde_json::Value = self
            .http
            .post(format!("{}/translate", self.base_url))
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        resp.pointer("/translatedText")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("libretranslate: unexpected response shape"))
    }
}
