//! 小牛翻译 (niutrans.com 开放平台). Needs apikey.
//! GET https://api.niutrans.com/NiuTransServer/translation

use super::{Lang, Translator};

pub struct NiuTrans {
    http: reqwest::Client,
    api_key: String,
}

impl NiuTrans {
    pub fn new(http: reqwest::Client, api_key: String) -> Self {
        Self { http, api_key }
    }
}

#[async_trait::async_trait]
impl Translator for NiuTrans {
    async fn translate(&self, text: &str, to: Lang) -> anyhow::Result<String> {
        let target = match to {
            Lang::Zh => "zh",
            Lang::En => "en",
        };
        let resp: serde_json::Value = self
            .http
            .get("https://api.niutrans.com/NiuTransServer/translation")
            .query(&[
                ("srcText", text),
                ("from", "auto"),
                ("to", target),
                ("apikey", self.api_key.as_str()),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if let Some(msg) = resp.get("error_msg").and_then(|v| v.as_str()) {
            anyhow::bail!("niutrans: {msg}");
        }
        resp.pointer("/tgt_text")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("niutrans: unexpected response shape"))
    }
}
