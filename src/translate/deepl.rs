//! DeepL API (free tier host). Needs api_key in config or
//! Pro users: set base_url to api.deepl.com.

use super::{Lang, Translator};

pub struct DeepL {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl DeepL {
    pub fn new(http: reqwest::Client, api_key: String, base_url: Option<String>) -> Self {
        Self {
            http,
            base_url: base_url
                .unwrap_or_else(|| "https://api-free.deepl.com".into())
                .trim_end_matches('/')
                .to_string(),
            api_key,
        }
    }
}

#[async_trait::async_trait]
impl Translator for DeepL {
    async fn translate(&self, text: &str, to: Lang) -> anyhow::Result<String> {
        let target = match to {
            Lang::Zh => "ZH",
            Lang::En => "EN",
        };
        let resp: serde_json::Value = self
            .http
            .post(format!("{}/v2/translate", self.base_url))
            .form(&[
                ("auth_key", self.api_key.as_str()),
                ("text", text),
                ("target_lang", target),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        resp.pointer("/translations/0/text")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("deepl: unexpected response shape"))
    }
}
