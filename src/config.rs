//! Config: zero-config by contract. The file is optional; every key has a
//! default. Only ~/.config/waytranslate/config.toml, flat keys, nothing else.

use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Config {
    /// "google" (zero config) or "openai" (any OpenAI-compatible endpoint).
    pub backend: String,
    /// Icon expands on "click" (default) or "hover".
    pub expand: String,
    pub target: String,
    /// UI language: "auto" (follow system), "zh-CN", "en".
    pub language: String,
    /// Selections shorter than this never trigger the popup.
    pub min_chars: usize,
    pub openai: OpenAi,
    pub deepl: DeepL,
    pub baidu: Baidu,
    pub libretranslate: LibreTranslate,
    pub deeplx: DeepLX,
    pub microsoft: Microsoft,
    pub niutrans: NiuTrans,
    pub tencent: Tencent,
    pub volcengine: Volcengine,
    pub custom: Custom,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct OpenAi {
    pub base_url: String,
    /// Prefer the env var over writing keys here.
    pub api_key: Option<String>,
    pub model: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            backend: "google".into(),
            expand: "click".into(),
            target: "auto".into(),
            language: "auto".into(),
            min_chars: 2,
            openai: OpenAi::default(),
            deepl: DeepL::default(),
            baidu: Baidu::default(),
            libretranslate: LibreTranslate::default(),
            deeplx: DeepLX::default(),
            microsoft: Microsoft::default(),
            niutrans: NiuTrans::default(),
            tencent: Tencent::default(),
            volcengine: Volcengine::default(),
            custom: Custom::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct DeepL {
    pub api_key: Option<String>,
    /// Pro accounts: https://api.deepl.com
    pub base_url: Option<String>,
}
#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Baidu {
    pub appid: Option<String>,
    pub api_key: Option<String>,
}
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct LibreTranslate {
    pub base_url: String,
    pub api_key: Option<String>,
}
impl Default for LibreTranslate {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:9090".into(),
            api_key: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct DeepLX {
    pub base_url: String,
}
impl Default for DeepLX {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:1188".into(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Microsoft {
    pub api_key: Option<String>,
    pub region: Option<String>,
}
#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct NiuTrans {
    pub api_key: Option<String>,
}
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Tencent {
    pub secret_id: Option<String>,
    pub secret_key: Option<String>,
    pub region: String,
}
impl Default for Tencent {
    fn default() -> Self {
        Self {
            secret_id: None,
            secret_key: None,
            region: "ap-shanghai".into(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Volcengine {
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
}
/// 通用自定义后端：模板化请求 + `JSON` 指针取结果
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Custom {
    pub url: String,
    pub method: String,
    pub headers: std::collections::HashMap<String, String>,
    pub body_template: Option<String>,
    pub query_template: Option<String>,
    pub response_path: String,
}
impl Default for Custom {
    fn default() -> Self {
        Self {
            url: String::new(),
            method: "POST".into(),
            headers: Default::default(),
            body_template: None,
            query_template: None,
            response_path: "/0/0".into(),
        }
    }
}

impl Default for OpenAi {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:11434/v1".into(),
            api_key: None,
            model: "qwen2.5:7b".into(),
        }
    }
}

pub fn path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("waytranslate/config.toml")
}

pub fn load() -> anyhow::Result<Config> {
    let path = path();
    if !path.exists() {
        return Ok(Config::default());
    }
    let text = std::fs::read_to_string(&path)?;
    let cfg: Config =
        toml::from_str(&text).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
    Ok(cfg)
}
impl Config {
    /// None = auto-detect direction.
    pub fn target_lang(&self) -> Option<crate::translate::Lang> {
        match self.target.as_str() {
            "auto" => None,
            "zh" => Some(crate::translate::Lang::Zh),
            "en" => Some(crate::translate::Lang::En),
            _ => None,
        }
    }
}

/// Sample config for `waytranslate config` / first-run documentation.
pub const SAMPLE: &str = r#"# waytranslate 配置（全部可选，删掉这文件就等于默认配置）

# 翻译后端：google / bing / deeplx（免费免配置）；deepl / microsoft /
# niutrans / tencent / volcengine / baidu（填 key，见下方各节）；
# libretranslate（自托管）；openai（任意 OpenAI 兼容端点）；custom（任意 HTTP 模板）
backend = "google"

# 图标展开方式：click（点击，默认）| hover（悬停即开）
# expand = "click"

# 翻译方向：auto（中文→英，其他→中）| zh | en
target = "auto"

# 界面语言：auto（跟随系统）| zh-CN | en
# language = "auto"

# 少于多少字符不弹窗
min_chars = 2

# [openai]
# base_url = "http://localhost:11434/v1"   # ollama / vllm / 云端兼容端点
# model = "qwen2.5:7b"
# api_key = "..."

# [deepl]
# api_key = "..."   # Pro 用户加 base_url = "https://api.deepl.com"

# [baidu]
# appid = "..."
# api_key = "..."

# [libretranslate]
# base_url = "http://localhost:9090"   # 自托管；公共实例填对应地址
# api_key = "..."   # 可选

# [deeplx]
# base_url = "http://localhost:1188"   # 免费 DeepL 中继，自建或公共实例

# [microsoft]
# api_key = "..."   # region 可选

# [niutrans]
# api_key = "..."

# [tencent]
# secret_id = "..."
# secret_key = "..."
# region = "ap-shanghai"

# [volcengine]
# access_key = "..."
# secret_key = "..."

# [custom]  # 通用模板：任意 HTTP 翻译接口
# url = "https://example.com/translate"
# method = "POST"                       # 或 GET（用 query_template 拼参数）
# headers = { Authorization = "Bearer x" }
# body_template = "{\"q\":\"{{text}}\",\"target\":\"{{target}}\"}"
# response_path = "/data/0/text"        # JSON 指针
"#;

/// Write config to ~/.config/waytranslate/config.toml (0600: may hold an API key).
pub fn save(cfg: &Config) -> anyhow::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let path = path();
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)?;
    f.write_all(toml::to_string_pretty(cfg)?.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn target_lang_parsing() {
        use super::Config;
        use crate::translate::Lang;
        let mut c = Config::default();
        assert_eq!(c.target_lang(), None);
        c.target = "zh".into();
        assert_eq!(c.target_lang(), Some(Lang::Zh));
        c.target = "en".into();
        assert_eq!(c.target_lang(), Some(Lang::En));
        c.target = "bogus".into();
        assert_eq!(c.target_lang(), None);
    }

    #[test]
    fn defaults_are_sane() {
        let c = super::Config::default();
        assert_eq!(c.backend, "google");
        assert_eq!(c.language, "auto");
    }
}
