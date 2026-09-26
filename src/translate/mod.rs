//! Translation backends.
//!
//! One trait, one async fn, twelve implementations:
//! google / bing / libretranslate / deepl / deeplx / microsoft / niutrans /
//! tencent / volcengine / baidu / openai / custom.
//!
//! Streaming is intentionally not in the trait yet — the popup consumes
//! complete results; SSE streaming lands behind the same trait later.

mod baidu;
mod bing;
mod custom;
mod deepl;
mod deeplx;
mod google;
mod libretranslate;
mod microsoft;
mod niutrans;
mod openai;
mod sign;
mod tencent;
mod volcengine;

pub use baidu::Baidu;
pub use bing::Bing;
pub use custom::Custom;
pub use deepl::DeepL;
pub use deeplx::DeepLX;
pub use google::Google;
pub use libretranslate::LibreTranslate;
pub use microsoft::Microsoft;
pub use niutrans::NiuTrans;
pub use openai::OpenAiCompat;
pub use tencent::Tencent;
pub use volcengine::Volcengine;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    Zh,
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::Zh => "zh-CN",
            Lang::En => "en",
        }
    }
}

/// Pick a translation direction: Chinese text → English, everything else → Chinese.
pub fn auto_direction(text: &str) -> Lang {
    let cjk = text
        .chars()
        .filter(|c| {
            matches!(c,
                '\u{4e00}'..='\u{9fff}'
                | '\u{3400}'..='\u{4dbf}'
                | '\u{f900}'..='\u{faff}'
                | '\u{3000}'..='\u{303f}'
                | '\u{ff00}'..='\u{ffef}')
        })
        .count();
    if cjk * 2 >= text.chars().count().max(1) {
        Lang::En
    } else {
        Lang::Zh
    }
}

#[async_trait::async_trait]
pub trait Translator: Send + Sync {
    async fn translate(&self, text: &str, to: Lang) -> anyhow::Result<String>;
}

/// Backend ids in settings-dropdown order; `make_backend` keys off these.
pub(crate) const BACKEND_IDS: &[&str] = &[
    "google",
    "bing",
    "libretranslate",
    "deepl",
    "deeplx",
    "microsoft",
    "niutrans",
    "tencent",
    "volcengine",
    "baidu",
    "openai",
    "custom",
];

pub(crate) fn make_backend(
    cfg: &crate::config::Config,
) -> anyhow::Result<std::sync::Arc<dyn Translator>> {
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    Ok(match cfg.backend.as_str() {
        "google" => std::sync::Arc::new(Google::new(http)),
        "bing" => std::sync::Arc::new(Bing::new(http)),
        "libretranslate" => std::sync::Arc::new(LibreTranslate::new(
            http,
            cfg.libretranslate.base_url.clone(),
            cfg.libretranslate.api_key.clone(),
        )),
        "deepl" => std::sync::Arc::new(DeepL::new(
            http,
            cfg.deepl
                .api_key
                .clone()
                .ok_or_else(|| anyhow::anyhow!("deepl: 缺 api_key"))?,
            cfg.deepl.base_url.clone(),
        )),
        "baidu" => std::sync::Arc::new(Baidu::new(
            http,
            cfg.baidu
                .appid
                .clone()
                .ok_or_else(|| anyhow::anyhow!("baidu: 缺 appid"))?,
            cfg.baidu
                .api_key
                .clone()
                .ok_or_else(|| anyhow::anyhow!("baidu: 缺 api_key"))?,
        )),
        "deeplx" => std::sync::Arc::new(DeepLX::new(http, cfg.deeplx.base_url.clone())),
        "microsoft" => std::sync::Arc::new(Microsoft::new(
            http,
            cfg.microsoft
                .api_key
                .clone()
                .ok_or_else(|| anyhow::anyhow!("microsoft: 缺 api_key"))?,
            cfg.microsoft.region.clone(),
        )),
        "niutrans" => std::sync::Arc::new(NiuTrans::new(
            http,
            cfg.niutrans
                .api_key
                .clone()
                .ok_or_else(|| anyhow::anyhow!("niutrans: 缺 api_key"))?,
        )),
        "tencent" => std::sync::Arc::new(Tencent::new(
            http,
            cfg.tencent
                .secret_id
                .clone()
                .ok_or_else(|| anyhow::anyhow!("tencent: 缺 secret_id"))?,
            cfg.tencent
                .secret_key
                .clone()
                .ok_or_else(|| anyhow::anyhow!("tencent: 缺 secret_key"))?,
            cfg.tencent.region.clone(),
        )),
        "volcengine" => std::sync::Arc::new(Volcengine::new(
            http,
            cfg.volcengine
                .access_key
                .clone()
                .ok_or_else(|| anyhow::anyhow!("volcengine: 缺 access_key"))?,
            cfg.volcengine
                .secret_key
                .clone()
                .ok_or_else(|| anyhow::anyhow!("volcengine: 缺 secret_key"))?,
        )),
        "custom" => std::sync::Arc::new(Custom::new(
            http,
            cfg.custom.url.clone(),
            cfg.custom.method.clone(),
            cfg.custom.headers.clone().into_iter().collect(),
            cfg.custom.body_template.clone(),
            cfg.custom.query_template.clone(),
            cfg.custom.response_path.clone(),
        )),
        "openai" => std::sync::Arc::new(OpenAiCompat::new(
            http,
            cfg.openai.base_url.clone(),
            cfg.openai.api_key.clone(),
            cfg.openai.model.clone(),
        )),
        other => anyhow::bail!(
            "config: unknown backend {other:?} (google|bing|libretranslate|deepl|deeplx|microsoft|niutrans|tencent|volcengine|baidu|openai|custom)"
        ),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn auto_direction_detection() {
        use super::{Lang, auto_direction};
        assert_eq!(auto_direction("你好世界"), Lang::En);
        assert_eq!(auto_direction("hello world"), Lang::Zh);
        assert_eq!(
            auto_direction("这是一段中文 with a bit 混合内容为主"),
            Lang::En
        );
        assert_eq!(auto_direction("mostly English with 少量中文"), Lang::Zh);
        assert_eq!(auto_direction("12345 !!!"), Lang::Zh);
    }
}
