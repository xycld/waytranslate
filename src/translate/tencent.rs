//! 腾讯机器翻译 TMT (tmt.tencentcloudapi.com), TC3-HMAC-SHA256 签名。
//! Needs secret_id + secret_key ( /
//!). 未实测：无测试账号，按官方文档实现。

use super::sign;
use super::{Lang, Translator};

pub struct Tencent {
    http: reqwest::Client,
    secret_id: String,
    secret_key: String,
    region: String,
}

impl Tencent {
    pub fn new(
        http: reqwest::Client,
        secret_id: String,
        secret_key: String,
        region: String,
    ) -> Self {
        Self {
            http,
            secret_id,
            secret_key,
            region,
        }
    }
}

#[async_trait::async_trait]
impl Translator for Tencent {
    async fn translate(&self, text: &str, to: Lang) -> anyhow::Result<String> {
        let target = match to {
            Lang::Zh => "zh",
            Lang::En => "en",
        };
        let host = "tmt.tencentcloudapi.com";
        let ts = sign::now_utc();
        let payload = serde_json::json!({
            "SourceText": text,
            "Source": "auto",
            "Target": target,
            "ProjectId": 0,
        })
        .to_string();

        // TC3-HMAC-SHA256
        let content_type = "application/json; charset=utf-8";
        let canonical_headers = format!("content-type:{content_type}\nhost:{host}\n");
        let signed_headers = "content-type;host";
        let canonical_request = format!(
            "POST\n/\n\n{canonical_headers}\n{signed_headers}\n{}",
            sign::sha256_hex(payload.as_bytes())
        );
        let scope = format!("{}/tmt/tc3_request", ts.date);
        let string_to_sign = format!(
            "TC3-HMAC-SHA256\n{}\n{scope}\n{}",
            ts.epoch,
            sign::sha256_hex(canonical_request.as_bytes())
        );
        let secret_date = sign::hmac_sha256(
            format!("TC3{}", self.secret_key).as_bytes(),
            ts.date.as_bytes(),
        );
        let secret_service = sign::hmac_sha256(&secret_date, b"tmt");
        let secret_signing = sign::hmac_sha256(&secret_service, b"tc3_request");
        let signature = sign::hmac_sha256_hex(&secret_signing, string_to_sign.as_bytes());

        let authorization = format!(
            "TC3-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
            self.secret_id
        );

        let resp: serde_json::Value = self
            .http
            .post(format!("https://{host}"))
            .header("Authorization", authorization)
            .header("Content-Type", content_type)
            .header("Host", host)
            .header("X-TC-Action", "TextTranslate")
            .header("X-TC-Version", "2018-03-21")
            .header("X-TC-Timestamp", ts.epoch.to_string())
            .header("X-TC-Region", &self.region)
            .body(payload)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        if let Some(err) = resp
            .pointer("/Response/Error/Message")
            .and_then(|v| v.as_str())
        {
            anyhow::bail!("tencent: {err}");
        }
        resp.pointer("/Response/TargetText")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("tencent: unexpected response shape"))
    }
}
