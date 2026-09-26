//! 火山引擎机器翻译 (translate.volcengineapi.com), V4 HMAC 签名。
//! Needs access_key + secret_key ( /
//!). 未实测：无测试账号，按官方文档实现。

use super::sign;
use super::{Lang, Translator};

pub struct Volcengine {
    http: reqwest::Client,
    access_key: String,
    secret_key: String,
}

impl Volcengine {
    pub fn new(http: reqwest::Client, access_key: String, secret_key: String) -> Self {
        Self {
            http,
            access_key,
            secret_key,
        }
    }
}

#[async_trait::async_trait]
impl Translator for Volcengine {
    async fn translate(&self, text: &str, to: Lang) -> anyhow::Result<String> {
        let target = match to {
            Lang::Zh => "zh",
            Lang::En => "en",
        };
        let host = "translate.volcengineapi.com";
        let ts = sign::now_utc();
        let query = "Action=TranslateText&Version=2020-06-01";
        let body = serde_json::json!({
            "SourceLanguage": "auto",
            "TargetLanguage": target,
            "TextList": [text],
        })
        .to_string();
        let body_hash = sign::sha256_hex(body.as_bytes());

        let canonical_headers = format!("host:{host}\nx-date:{}\n", ts.datetime);
        let signed_headers = "host;x-date";
        let canonical_request =
            format!("POST\n/\n{query}\n{canonical_headers}\n{signed_headers}\n{body_hash}");
        let scope = format!("{}/cn-north-1/translate/request", ts.date);
        let string_to_sign = format!(
            "HMAC-SHA256\n{}\n{scope}\n{}",
            ts.datetime,
            sign::sha256_hex(canonical_request.as_bytes())
        );
        let k_date = sign::hmac_sha256(self.secret_key.as_bytes(), ts.date.as_bytes());
        let k_region = sign::hmac_sha256(&k_date, b"cn-north-1");
        let k_service = sign::hmac_sha256(&k_region, b"translate");
        let k_signing = sign::hmac_sha256(&k_service, b"request");
        let signature = sign::hmac_sha256_hex(&k_signing, string_to_sign.as_bytes());

        let authorization = format!(
            "HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
            self.access_key
        );

        let resp: serde_json::Value = self
            .http
            .post(format!("https://{host}/?{query}"))
            .header("Authorization", authorization)
            .header("Content-Type", "application/json")
            .header("Host", host)
            .header("X-Date", &ts.datetime)
            .header("X-Content-Sha256", &body_hash)
            .body(body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        if let Some(err) = resp
            .pointer("/ResponseMetadata/Error/Message")
            .and_then(|v| v.as_str())
        {
            anyhow::bail!("volcengine: {err}");
        }
        resp.pointer("/TranslationList/0/Translation")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("volcengine: unexpected response shape"))
    }
}
