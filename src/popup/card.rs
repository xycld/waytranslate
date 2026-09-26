//! Translation card page: editable source, skeleton/caret loading feedback,
//! error row, and the action bar with the session-scoped backend selector.

use gettextrs::gettext;
use gtk4::prelude::*;

use super::icons::{self, GRAY};
use super::{Action, CARD_W};

/// Backend names shown in the card's engine selector (proper nouns; the
/// settings window localizes its own longer labels).
const BACKEND_LABELS: &[&str] = &[
    "Google",
    "Bing",
    "LibreTranslate",
    "DeepL",
    "DeepLX",
    "Microsoft",
    "NiuTrans",
    "Tencent",
    "Volcengine",
    "Baidu",
    "OpenAI API",
    "Custom",
];

/// Card widgets handed back to the `Popup` state holder.
pub(super) struct CardWidgets {
    pub source: gtk4::TextView,
    pub target: gtk4::Label,
    pub caret: gtk4::Box,
    pub skeleton: gtk4::Box,
    pub error_box: gtk4::Box,
    pub pin_btn: gtk4::Button,
    pub copy_img: gtk4::Image,
    pub copy_btn: gtk4::Button,
}

pub(super) fn build_card(
    stack: &gtk4::Stack,
    on_action: &std::rc::Rc<dyn Fn(Action)>,
    edit_guard: &std::rc::Rc<std::cell::Cell<bool>>,
    initial_backend: u32,
) -> CardWidgets {
    // ---- card page ----
    let card = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    card.set_size_request(CARD_W, -1);
    stack.add_named(&card, Some("card"));

    // backend selector (session-scoped switch, config untouched) —
    // lives in the bottom action row, left side.
    let backend_dd = gtk4::DropDown::new(
        Some(gtk4::StringList::new(BACKEND_LABELS)),
        gtk4::Expression::NONE,
    );
    backend_dd.add_css_class("waytranslate-backend-dd");
    backend_dd.set_selected(initial_backend);
    {
        let cb = std::rc::Rc::clone(on_action);
        backend_dd.connect_selected_notify(move |dd| cb(Action::SwitchBackend(dd.selected())));
    }
    {
        let cb = std::rc::Rc::clone(on_action);
        let press = gtk4::GestureClick::new();
        press.connect_pressed(move |_, _, _, _| cb(Action::BackendMenuOpened));
        backend_dd.add_controller(press);
    }

    // source: editable (kiss-style) — edits retrigger translation via daemon
    let source = gtk4::TextView::builder()
        .wrap_mode(gtk4::WrapMode::WordChar)
        .accepts_tab(false)
        .height_request(44)
        .build();
    source.add_css_class("waytranslate-source");

    // Long text scrolls instead of overflowing the screen.
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .max_content_height(320)
        .propagate_natural_height(true)
        .build();
    scroll.set_child(Some(&content));
    content.append(&source);
    {
        let cb = std::rc::Rc::clone(on_action);
        let buf = source.buffer();
        let guard = edit_guard.clone();
        buf.connect_changed(move |b| {
            if guard.get() {
                return; // programmatic set (begin), not a user edit
            }
            let text = b.text(&b.start_iter(), &b.end_iter(), false).to_string();
            cb(Action::Edited(text));
        });
    }

    let target_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    let target = gtk4::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .max_width_chars(46)
        .selectable(true)
        .hexpand(true)
        .build();
    target.add_css_class("waytranslate-target");
    // Loading indicator from design/popup.html: a 2px accent caret which
    // blinks while the request is in flight. GTK CSS has no keyframes.
    let caret = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    caret.add_css_class("waytranslate-caret");
    caret.set_valign(gtk4::Align::Center);
    {
        let caret = caret.clone();
        gtk4::glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
            if caret.is_visible() {
                if caret.has_css_class("dim") {
                    caret.remove_css_class("dim");
                } else {
                    caret.add_css_class("dim");
                }
            }
            gtk4::glib::ControlFlow::Continue
        });
    }
    target_row.append(&target);
    target_row.append(&caret);

    let skeleton = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    skeleton.add_css_class("waytranslate-skeleton");
    for width in [342, 290, 205] {
        let bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        bar.add_css_class("waytranslate-skeleton-bar");
        bar.set_size_request(width, 12);
        skeleton.append(&bar);
    }
    {
        let skeleton = skeleton.clone();
        gtk4::glib::timeout_add_local(std::time::Duration::from_millis(600), move || {
            if skeleton.is_visible() {
                if skeleton.has_css_class("dim") {
                    skeleton.remove_css_class("dim");
                } else {
                    skeleton.add_css_class("dim");
                }
            }
            gtk4::glib::ControlFlow::Continue
        });
    }
    content.append(&skeleton);
    content.append(&target_row);
    card.append(&scroll);

    let error_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    error_box.add_css_class("waytranslate-error");
    let error_text = gtk4::Label::builder().xalign(0.0).hexpand(true).build();
    error_text.add_css_class("waytranslate-error-text");
    error_text.set_label(&gettext("Translation failed"));
    let retry = gtk4::Button::with_label(&gettext("Retry"));
    retry.add_css_class("waytranslate-retry");
    {
        let cb = on_action.clone();
        retry.connect_clicked(move |_| cb(Action::Retry));
    }
    error_box.append(&error_text);
    error_box.append(&retry);
    error_box.set_visible(false);
    card.append(&error_box);

    let actions = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
    actions.add_css_class("waytranslate-actions");
    actions.set_halign(gtk4::Align::Fill);
    actions.append(&backend_dd);
    let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    actions.append(&spacer);

    let copy_img = icons::svg_image("copy", GRAY);
    let (copy_btn, pin_btn, close_btn) = (
        gtk4::Button::new(),
        gtk4::Button::new(),
        gtk4::Button::new(),
    );
    copy_btn.set_child(Some(&copy_img));
    pin_btn.set_child(Some(&icons::svg_image("pin", GRAY)));
    close_btn.set_child(Some(&icons::svg_image("close", GRAY)));
    for (b, tip, action) in [
        (&copy_btn, &*gettext("Copy translation"), Action::Copy),
        (
            &pin_btn,
            &*gettext("Pin: keep visible (click to unpin)"),
            Action::TogglePin,
        ),
        (&close_btn, &*gettext("Close"), Action::Close),
    ] {
        b.add_css_class("waytranslate-btn");
        b.set_tooltip_text(Some(tip));
        let cb = on_action.clone();
        b.connect_clicked(move |_| cb(action.clone()));
        actions.append(b);
    }
    card.append(&actions);
    CardWidgets {
        source,
        target,
        caret,
        skeleton,
        error_box,
        pin_btn,
        copy_img,
        copy_btn,
    }
}
