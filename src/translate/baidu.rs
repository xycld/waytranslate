//! 百度翻译开放平台 (fanyi-api.baidu.com). Needs appid + key in config
//! or

use super::{Lang, Translator};

pub struct Baidu {
    http: reqwest::Client,
    appid: String,
    key: String,
}

impl Baidu {
    pub fn new(http: reqwest::Client, appid: String, key: String) -> Self {
        Self { http, appid, key }
    }
}

#[async_trait::async_trait]
impl Translator for Baidu {
    async fn translate(&self, text: &str, to: Lang) -> anyhow::Result<String> {
        let salt = format!(
            "{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0)
        );
        let sign = md5_hex(&format!("{}{}{}{}", self.appid, text, salt, self.key));
        let target = match to {
            Lang::Zh => "zh",
            Lang::En => "en",
        };
        let resp: serde_json::Value = self
            .http
            .post("https://fanyi-api.baidu.com/api/trans/vip/translate")
            .form(&[
                ("q", text),
                ("from", "auto"),
                ("to", target),
                ("appid", self.appid.as_str()),
                ("salt", salt.as_str()),
                ("sign", sign.as_str()),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        if let Some(code) = resp.get("error_code").and_then(|v| v.as_str()) {
            let msg = resp
                .get("error_msg")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            anyhow::bail!("baidu: {code} {msg}");
        }
        resp.pointer("/trans_result/0/dst")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("baidu: unexpected response shape"))
    }
}

fn md5_hex(input: &str) -> String {
    format!("{:x}", md5::compute(input.as_bytes()))
}
