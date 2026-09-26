//! i18n via gettext (GNOME standard). Source strings are English msgids;
//! translations live in po/<lang>.po → installed .mo files.
//!
//! Lookup order: $WAYTRANSLATE_LOCALEDIR (dev), <exe>/../locale (dev),
//! /usr/share/locale (installed).

use gettextrs::{bind_textdomain_codeset, bindtextdomain, textdomain};

pub const DOMAIN: &str = "waytranslate";

pub fn init() {
    bindtextdomain(DOMAIN, locale_dir()).ok();
    bind_textdomain_codeset(DOMAIN, "UTF-8").ok();
    textdomain(DOMAIN).ok();
}

fn locale_dir() -> String {
    if let Ok(dir) = std::env::var("WAYTRANSLATE_LOCALEDIR") {
        return dir;
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let dev = dir.join("locale");
        if dev.is_dir() {
            return dev.to_string_lossy().into_owned();
        }
    }
    "/usr/share/locale".into()
}

/// Manual language override. GNU gettext honors $LANGUAGE above everything,
/// which is exactly the "manual beats system" semantics we want.
/// "auto" clears the override. Applies to strings rendered afterwards
/// (reopen open windows to relabel them).
pub fn apply(language: &str) {
    // SAFETY: called from the GTK main thread only; glibc env access is
    // thread-safe for our purposes (gettextrs reads $LANGUAGE per lookup).
    unsafe {
        match language {
            "auto" | "" => std::env::remove_var("LANGUAGE"),
            // gettext wants POSIX locale names (zh_CN), config stores BCP47 (zh-CN).
            other => std::env::set_var("LANGUAGE", other.replace('-', "_")),
        }
    }
}
