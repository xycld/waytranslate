//! DeepLX — free DeepL relays (deeplx). Same engine, no official key:
//! POST {base}/translate {text, source_lang: "auto", target_lang}.

use super::{Lang, Translator};

pub struct DeepLX {
    http: reqwest::Client,
    base_url: String,
}

impl DeepLX {
    pub fn new(http: reqwest::Client, base_url: String) -> Self {
        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }
}

#[async_trait::async_trait]
impl Translator for DeepLX {
    async fn translate(&self, text: &str, to: Lang) -> anyhow::Result<String> {
        let target = match to {
            Lang::Zh => "ZH",
            Lang::En => "EN",
        };
        let resp: serde_json::Value = self
            .http
            .post(format!("{}/translate", self.base_url))
            .json(&serde_json::json!({
                "text": text,
                "source_lang": "auto",
                "target_lang": target,
            }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        resp.pointer("/data")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("deeplx: unexpected response shape"))
    }
}
