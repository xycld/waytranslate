//! Microsoft Translator (Azure Cognitive Services).
//! Needs key + optional region. Endpoint: api.cognitive.microsofttranslator.com.

use super::{Lang, Translator};

pub struct Microsoft {
    http: reqwest::Client,
    api_key: String,
    region: Option<String>,
}

impl Microsoft {
    pub fn new(http: reqwest::Client, api_key: String, region: Option<String>) -> Self {
        Self {
            http,
            api_key,
            region,
        }
    }
}

#[async_trait::async_trait]
impl Translator for Microsoft {
    async fn translate(&self, text: &str, to: Lang) -> anyhow::Result<String> {
        let target = match to {
            Lang::Zh => "zh-Hans",
            Lang::En => "en",
        };
        let mut req = self
            .http
            .post("https://api.cognitive.microsofttranslator.com/translate")
            .query(&[("api-version", "3.0"), ("to", target)])
            .header("Ocp-Apim-Subscription-Key", &self.api_key)
            .json(&serde_json::json!([{ "text": text }]));
        if let Some(region) = &self.region {
            req = req.header("Ocp-Apim-Subscription-Region", region);
        }
        let resp: serde_json::Value = req.send().await?.error_for_status()?.json().await?;
        resp.pointer("/0/translations/0/text")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("microsoft: unexpected response shape"))
    }
}
