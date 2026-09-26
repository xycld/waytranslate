//! Google Translate free endpoint (translate.googleapis.com/translate_a/single).
//! No key, no config — the zero-friction default. Same endpoint translate-shell uses.

use super::{Lang, Translator};

pub struct Google {
    http: reqwest::Client,
}

impl Google {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }
}

#[async_trait::async_trait]
impl Translator for Google {
    async fn translate(&self, text: &str, to: Lang) -> anyhow::Result<String> {
        let resp: serde_json::Value = self
            .http
            .get("https://translate.googleapis.com/translate_a/single")
            .query(&[
                ("client", "gtx"),
                ("sl", "auto"),
                ("tl", to.code()),
                ("dt", "t"),
                ("q", text),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        // Shape: [[["translated", "source", ...], ...], ...]
        let segments = resp
            .as_array()
            .and_then(|a| a.first())
            .and_then(|a| a.as_array())
            .ok_or_else(|| anyhow::anyhow!("unexpected google response shape"))?;

        let mut out = String::new();
        for seg in segments {
            if let Some(s) = seg
                .as_array()
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
            {
                out.push_str(s);
            }
        }
        anyhow::ensure!(!out.is_empty(), "empty translation");
        Ok(out)
    }
}
