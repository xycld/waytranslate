//! OpenAI-compatible chat backend (cloud LLM or local: ollama, llama.cpp, vllm).
//! Higher quality ceiling; needs base_url + api_key (env or config).

use super::{Lang, Translator};

pub struct OpenAiCompat {
    http: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    model: String,
}

impl OpenAiCompat {
    pub fn new(
        http: reqwest::Client,
        base_url: String,
        api_key: Option<String>,
        model: String,
    ) -> Self {
        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            model,
        }
    }
}

#[async_trait::async_trait]
impl Translator for OpenAiCompat {
    async fn translate(&self, text: &str, to: Lang) -> anyhow::Result<String> {
        let target = match to {
            Lang::Zh => "Simplified Chinese",
            Lang::En => "English",
        };
        let body = serde_json::json!({
            "model": self.model,
            "temperature": 0.3,
            "messages": [
                {
                    "role": "system",
                    "content": format!(
                        "You are a translation engine. Translate the user's text into {target}. \
                         Output ONLY the translation — no explanations, no quotes, no notes. \
                         Preserve formatting and line breaks."
                    )
                },
                { "role": "user", "content": text }
            ]
        });

        let mut req = self
            .http
            .post(format!("{}/chat/completions", self.base_url))
            .json(&body);
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }

        let resp: serde_json::Value = req.send().await?.error_for_status()?.json().await?;
        let out = resp
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .ok_or_else(|| anyhow::anyhow!("unexpected openai response shape"))?;
        anyhow::ensure!(!out.is_empty(), "empty translation");
        Ok(out)
    }
}
