//! Settings window: GTK4 preferences-style, plain widgets + CSS classes.
//! Structure per group: title label OUTSIDE the card box, so card rows are
//! the only children — :first-child/:last-child corner rounding works.
//! Save = write config.toml + live-apply via callback.

use gettextrs::gettext;
use gtk4::prelude::*;

use crate::config::Config;

pub const CSS: &str = include_str!("settings.css");

/// A labeled card; rows appended to the card become its only children.
struct Group {
    card: gtk4::Box,
}

fn group(page: &gtk4::Box, title: &str) -> Group {
    let t = gtk4::Label::new(Some(title));
    t.add_css_class("waytranslate-pref-group-title");
    t.set_xalign(0.0);
    page.append(&t);
    let card = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    card.add_css_class("waytranslate-pref-card");
    page.append(&card);
    Group { card }
}

fn row(g: &Group, label: &str) -> gtk4::Box {
    let r = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    r.add_css_class("waytranslate-pref-row");
    let l = gtk4::Label::new(Some(label));
    l.set_xalign(0.0);
    l.set_hexpand(true);
    l.add_css_class("waytranslate-pref-label");
    r.append(&l);
    g.card.append(&r);
    r
}

fn spin(value: f64, min: f64, max: f64, step: f64) -> gtk4::SpinButton {
    let s = gtk4::SpinButton::with_range(min, max, step);
    s.set_value(value);
    s
}

fn dropdown(items: &[&str], sel: u32) -> gtk4::DropDown {
    let dd = gtk4::DropDown::new(Some(gtk4::StringList::new(items)), gtk4::Expression::NONE);
    dd.set_selected(sel);
    dd
}

fn entry(text: &str, placeholder: &str) -> gtk4::Entry {
    gtk4::Entry::builder()
        .text(text)
        .placeholder_text(placeholder)
        .hexpand(false)
        .width_chars(24)
        .build()
}

/// Everything the save handler needs, collected while building the groups.
struct Widgets {
    backend_dd: gtk4::DropDown,
    target_dd: gtk4::DropDown,
    lang_dd: gtk4::DropDown,
    min_chars: gtk4::SpinButton,
    autostart: gtk4::Switch,
    /// (backend index, entry) — backend-specific fields, in spec order
    field_entries: Vec<(u32, gtk4::Entry)>,
}

impl Widgets {
    fn field_val(&self, idx: u32, nth: usize) -> String {
        self.field_entries
            .iter()
            .filter(|(i, _)| *i == idx)
            .nth(nth)
            .map(|(_, e)| e.text().to_string())
            .unwrap_or_default()
    }

    fn collect(&self, cfg: &Config) -> Config {
        let nonempty = |v: String| (!v.is_empty()).then_some(v);
        Config {
            backend: crate::translate::BACKEND_IDS[self.backend_dd.selected() as usize].into(),
            expand: cfg.expand.clone(),
            target: ["auto", "zh", "en"][self.target_dd.selected() as usize].into(),
            language: ["auto", "zh-CN", "en"][self.lang_dd.selected() as usize].into(),
            min_chars: self.min_chars.value() as usize,
            openai: crate::config::OpenAi {
                base_url: self.field_val(10, 0),
                model: self.field_val(10, 1),
                api_key: nonempty(self.field_val(10, 2)),
            },
            deepl: crate::config::DeepL {
                api_key: nonempty(self.field_val(3, 0)),
                base_url: cfg.deepl.base_url.clone(),
            },
            baidu: crate::config::Baidu {
                appid: nonempty(self.field_val(9, 0)),
                api_key: nonempty(self.field_val(9, 1)),
            },
            libretranslate: crate::config::LibreTranslate {
                base_url: self.field_val(2, 0),
                api_key: nonempty(self.field_val(2, 1)),
            },
            deeplx: crate::config::DeepLX {
                base_url: self.field_val(4, 0),
            },
            microsoft: crate::config::Microsoft {
                api_key: nonempty(self.field_val(5, 0)),
                region: nonempty(self.field_val(5, 1)),
            },
            niutrans: crate::config::NiuTrans {
                api_key: nonempty(self.field_val(6, 0)),
            },
            tencent: crate::config::Tencent {
                secret_id: nonempty(self.field_val(7, 0)),
                secret_key: nonempty(self.field_val(7, 1)),
                region: cfg.tencent.region.clone(),
            },
            volcengine: crate::config::Volcengine {
                access_key: nonempty(self.field_val(8, 0)),
                secret_key: nonempty(self.field_val(8, 1)),
            },
            custom: cfg.custom.clone(),
        }
    }
}

/// Open (or focus) the settings window. `on_save` receives the new config
/// after it has been written to disk.
pub fn open(app: &gtk4::Application, cfg: &Config, on_save: impl Fn(Config) + 'static) {
    let provider = gtk4::CssProvider::new();
    provider.load_from_string(CSS);
    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().expect("no display"),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let window = gtk4::ApplicationWindow::builder()
        .application(app)
        .title(gettext("waytranslate Settings"))
        .default_width(600)
        .default_height(720)
        .build();
    window.add_css_class("waytranslate-settings");

    let scroll = gtk4::ScrolledWindow::new();
    window.set_child(Some(&scroll));
    let page = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    page.add_css_class("waytranslate-settings-page");
    scroll.set_child(Some(&page));

    let w = build_groups(&page, cfg);

    let save = gtk4::Button::with_label(&gettext("Save"));
    save.add_css_class("waytranslate-save");
    save.set_halign(gtk4::Align::End);
    page.append(&save);

    let on_save = std::rc::Rc::new(on_save);
    let win = window.clone();
    let cfg_orig = cfg.clone();
    save.connect_clicked(move |_| {
        let cfg = w.collect(&cfg_orig);
        set_autostart(w.autostart.is_active());
        match crate::config::save(&cfg) {
            Ok(()) => {
                on_save(cfg);
                win.close();
            }
            Err(e) => tracing::error!("save config failed: {e:#}"),
        }
    });

    window.present();
}

/// 翻译 + 触发 + 通用 groups; returns all widgets the save handler needs.
fn build_groups(page: &gtk4::Box, cfg: &Config) -> std::rc::Rc<Widgets> {
    let g_tr = group(page, &gettext("Translation"));

    let backends: Vec<String> = vec![
        gettext("Google (free)"),
        gettext("Bing (free)"),
        "LibreTranslate".into(),
        "DeepL".into(),
        gettext("DeepLX (free)"),
        "Microsoft".into(),
        gettext("NiuTrans"),
        gettext("Tencent"),
        gettext("Volcengine"),
        gettext("Baidu"),
        gettext("OpenAI-compatible API"),
        gettext("Custom"),
    ];
    let backends: Vec<&str> = backends.iter().map(String::as_str).collect();
    let backend_dd = dropdown(
        &backends,
        crate::translate::BACKEND_IDS
            .iter()
            .position(|b| *b == cfg.backend)
            .unwrap_or(0) as u32,
    );
    row(&g_tr, &gettext("Backend")).append(&backend_dd);

    let field_rows = build_backend_fields(&g_tr, cfg);
    let field_entries: Vec<(u32, gtk4::Entry)> =
        field_rows.iter().map(|(i, _, e)| (*i, e.clone())).collect();
    let sync_rows = |sel: u32, rows: &[(u32, gtk4::Box, gtk4::Entry)]| {
        for (i, r, _) in rows {
            r.set_visible(*i == sel);
        }
    };
    sync_rows(backend_dd.selected(), &field_rows);
    backend_dd.connect_selected_notify(move |dd| sync_rows(dd.selected(), &field_rows));

    let target_dd = dropdown(
        &[
            &*gettext("Auto (zh  ↔  en)"),
            &*gettext("Always to Chinese"),
            &*gettext("Always to English"),
        ],
        match cfg.target.as_str() {
            "zh" => 1,
            "en" => 2,
            _ => 0,
        },
    );
    row(&g_tr, &gettext("Direction")).append(&target_dd);

    // ---------- 通用 ----------
    let g_ge = group(page, &gettext("General"));

    let min_chars = spin(cfg.min_chars as f64, 1.0, 100.0, 1.0);
    row(&g_ge, &gettext("Minimum characters")).append(&min_chars);

    let lang_dd = dropdown(
        &[&*gettext("Follow system"), "中文", "English"],
        match cfg.language.as_str() {
            "zh-CN" => 1,
            "en" => 2,
            _ => 0,
        },
    );
    row(&g_ge, &gettext("Language")).append(&lang_dd);

    let autostart = gtk4::Switch::new();
    autostart.set_active(autostart_enabled());
    autostart.set_valign(gtk4::Align::Center);
    row(&g_ge, &gettext("Autostart")).append(&autostart);

    std::rc::Rc::new(Widgets {
        backend_dd,
        target_dd,
        lang_dd,
        min_chars,
        autostart,
        field_entries,
    })
}

/// Per-backend field rows: (backend index, row, entry) in spec order.
fn build_backend_fields(g_tr: &Group, cfg: &Config) -> Vec<(u32, gtk4::Box, gtk4::Entry)> {
    let specs: &[(u32, &str, &str, &str)] = &[
        (
            2,
            &gettext("LibreTranslate URL"),
            &cfg.libretranslate.base_url,
            "http://localhost:9090",
        ),
        (
            2,
            "LibreTranslate Key",
            cfg.libretranslate.api_key.as_deref().unwrap_or(""),
            &gettext("optional"),
        ),
        (
            3,
            "DeepL API Key",
            cfg.deepl.api_key.as_deref().unwrap_or(""),
            &gettext("from deepl.com"),
        ),
        (
            4,
            &gettext("DeepLX URL"),
            &cfg.deeplx.base_url,
            "http://localhost:1188",
        ),
        (
            5,
            "Microsoft Key",
            cfg.microsoft.api_key.as_deref().unwrap_or(""),
            &gettext("from Azure portal"),
        ),
        (
            5,
            &gettext("Microsoft region"),
            cfg.microsoft.region.as_deref().unwrap_or(""),
            &gettext("optional, e.g. eastasia"),
        ),
        (
            6,
            &gettext("NiuTrans key"),
            cfg.niutrans.api_key.as_deref().unwrap_or(""),
            &gettext("from niutrans.com"),
        ),
        (
            7,
            &gettext("Tencent SecretId"),
            cfg.tencent.secret_id.as_deref().unwrap_or(""),
            &gettext("from Tencent Cloud"),
        ),
        (
            7,
            &gettext("Tencent SecretKey"),
            cfg.tencent.secret_key.as_deref().unwrap_or(""),
            &gettext("from Tencent Cloud"),
        ),
        (
            8,
            &gettext("Volcengine AccessKey"),
            cfg.volcengine.access_key.as_deref().unwrap_or(""),
            &gettext("from Volcengine console"),
        ),
        (
            8,
            &gettext("Volcengine SecretKey"),
            cfg.volcengine.secret_key.as_deref().unwrap_or(""),
            &gettext("from Volcengine console"),
        ),
        (
            9,
            &gettext("Baidu AppID"),
            cfg.baidu.appid.as_deref().unwrap_or(""),
            &gettext("from Baidu open platform"),
        ),
        (
            9,
            &gettext("Baidu Key"),
            cfg.baidu.api_key.as_deref().unwrap_or(""),
            &gettext("from Baidu open platform"),
        ),
        (
            10,
            &gettext("Compatible API URL"),
            &cfg.openai.base_url,
            "http://localhost:11434/v1",
        ),
        (10, &gettext("Model name"), &cfg.openai.model, "qwen2.5:7b"),
        (
            10,
            &gettext("API key"),
            cfg.openai.api_key.as_deref().unwrap_or(""),
            &gettext("optional for local services"),
        ),
    ];

    let mut out = Vec::new();
    for (idx, label, value, ph) in specs {
        let e = entry(value, ph);
        let r = row(g_tr, label);
        r.append(&e);
        out.push((*idx, r, e));
    }
    // custom 后端字段多且模板化，GUI 只给提示，编辑 config.toml
    let hint = row(
        g_tr,
        &gettext("Custom API: edit config.toml (see template)"),
    );
    out.push((11, hint, entry("", "")));
    out
}

// ---------- autostart ----------

fn autostart_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| "~/.config".into())
        .join("autostart/io.github.xycld.waytranslate.desktop")
}

const AUTOSTART_DESKTOP: &str = "[Desktop Entry]\nType=Application\nName=waytranslate\nComment=Wayland 划词翻译弹窗\nExec=waytranslate run\nIcon=waytranslate\nNoDisplay=true\nStartupNotify=false\n";

fn autostart_enabled() -> bool {
    // A user-level `Hidden=true` file shadows everything else (that's the
    // "disabled" state). System package entry counts as enabled otherwise.
    if let Ok(content) = std::fs::read_to_string(autostart_path())
        && content.contains("Hidden=true")
    {
        return false;
    }
    std::path::Path::new("/etc/xdg/autostart/io.github.xycld.waytranslate.desktop").exists()
        || autostart_path().exists()
}

fn set_autostart(on: bool) {
    let path = autostart_path();
    if on {
        // Deleting the override restores the system entry (and keeps the
        // user file from shadowing future package updates).
        let _ = std::fs::remove_file(&path);
    } else {
        if let Some(p) = path.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        // User-level Hidden override shadows the system autostart entry.
        let _ = std::fs::write(&path, format!("{AUTOSTART_DESKTOP}Hidden=true\n"));
    }
}
