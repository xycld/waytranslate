//! Bing Translator free web endpoint (translate-shell style):
//! scrape IG/IID + abuse-prevention token from bing.com/translator, then POST
//! ttranslatev3. No key. Fragile by nature — fails loudly when Bing changes.

use super::{Lang, Translator};

pub struct Bing {
    http: reqwest::Client,
}

impl Bing {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    async fn tokens(&self) -> anyhow::Result<(String, String, String, String)> {
        let page = self
            .http
            .get("https://www.bing.com/translator")
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        let ig = extract(&page, "IG:\"", 4, '"')
            .ok_or_else(|| anyhow::anyhow!("bing: IG token not found"))?;
        let iid = extract(&page, "data-iid=\"", 10, '"')
            .ok_or_else(|| anyhow::anyhow!("bing: IID not found"))?;
        // var params_AbusePreventionHelper = [key,"token",...]
        let abuse = extract(&page, "params_AbusePreventionHelper = [", 32, ']')
            .ok_or_else(|| anyhow::anyhow!("bing: abuse token not found"))?;
        let mut it = abuse.splitn(3, ',');
        let key = it.next().unwrap_or_default().trim().to_string();
        let token = it
            .nth(0)
            .unwrap_or_default()
            .trim()
            .trim_matches('"')
            .to_string();
        Ok((ig, iid, key, token))
    }
}

/// Extract a snippet after `marker` (skipping `skip` chars past marker end)
/// up to terminator `term`.
fn extract(hay: &str, marker: &str, skip: usize, term: char) -> Option<String> {
    let start = hay.find(marker)? + skip;
    let rest = &hay[start..];
    let end = rest.find(term)?;
    Some(rest[..end].to_string())
}

#[async_trait::async_trait]
impl Translator for Bing {
    async fn translate(&self, text: &str, to: Lang) -> anyhow::Result<String> {
        let (ig, iid, key, token) = self.tokens().await?;
        let target = match to {
            Lang::Zh => "zh-Hans",
            Lang::En => "en",
        };
        let resp: serde_json::Value = self
            .http
            .post("https://www.bing.com/ttranslatev3")
            .query(&[("isVertical", "1"), ("IG", &ig), ("IID", &iid)])
            .form(&[
                ("text", text),
                ("fromLang", "auto-detect"),
                ("to", target),
                ("token", &token),
                ("key", &key),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let out = resp
            .pointer("/0/translations/0/text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("bing: unexpected response shape"))?
            .to_string();
        Ok(out)
    }
}
