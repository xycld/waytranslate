//! Custom backend: one `TOML` block → any HTTP translation API.
//!
//! ```toml
//! backend = "custom"
//! [custom]
//! url = "https://example.com/translate"
//! method = "POST"                      # default GET
//! headers = { Authorization = "Bearer x" }
//! body_template = "{\"q\":\"{{text}}\",\"target\":\"{{target}}\"}"  # POST only
//! # GET 时拼接 ?<query_template>，如 "q={{text}}&tl={{target}}"
//! query_template = "..."
//! response_path = "/data/0/text"       # `JSON` pointer
//! ```
//! Placeholders: {{text}} (`JSON`-escaped in body, URL-encoded in query),
//! {{target}} (zh-CN / en).

use super::{Lang, Translator};

pub struct Custom {
    http: reqwest::Client,
    url: String,
    method: String,
    headers: Vec<(String, String)>,
    body_template: Option<String>,
    query_template: Option<String>,
    response_path: String,
}

impl Custom {
    pub fn new(
        http: reqwest::Client,
        url: String,
        method: String,
        headers: Vec<(String, String)>,
        body_template: Option<String>,
        query_template: Option<String>,
        response_path: String,
    ) -> Self {
        Self {
            http,
            url,
            method: method.to_uppercase(),
            headers,
            body_template,
            query_template,
            response_path,
        }
    }
}

#[async_trait::async_trait]
impl Translator for Custom {
    async fn translate(&self, text: &str, to: Lang) -> anyhow::Result<String> {
        let target = to.code();
        let mut req = if self.method == "POST" {
            let body = self
                .body_template
                .as_deref()
                .unwrap_or(r#"{"q":"{{text}}","target":"{{target}}"}"#)
                .replace("{{text}}", &json_escape(text))
                .replace("{{target}}", target);
            self.http
                .post(&self.url)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body)
        } else {
            let query = self
                .query_template
                .as_deref()
                .unwrap_or("q={{text}}&tl={{target}}")
                .replace("{{text}}", &urlencoding(text))
                .replace("{{target}}", target);
            self.http.get(format!("{}?{query}", self.url))
        };
        for (k, v) in &self.headers {
            req = req.header(k, v);
        }
        let resp: serde_json::Value = req.send().await?.error_for_status()?.json().await?;
        resp.pointer(&self.response_path)
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("custom: response_path {} 未命中", self.response_path))
    }
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn json_escaping() {
        use super::json_escape;
        assert_eq!(json_escape("say \"hi\""), "say \\\"hi\\\"");
        assert_eq!(json_escape("a\nb\tc"), "a\\nb\\tc");
    }

    #[test]
    fn url_encoding() {
        use super::urlencoding;
        assert_eq!(urlencoding("hello world"), "hello%20world");
        assert_eq!(urlencoding("你好"), "%E4%BD%A0%E5%A5%BD");
        assert_eq!(urlencoding("a-b_c.d~e"), "a-b_c.d~e");
        assert_eq!(urlencoding("q=1&x=2"), "q%3D1%26x%3D2");
    }
}
