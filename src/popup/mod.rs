//! Layer-shell popup: borderless overlay surface, never takes keyboard focus.
//! Two pages in one `GtkStack`:
//!   "icon" — 28px circular icon shown on selection; hover/click expands
//!   "card" — full translation card (source / streaming target / actions)
//! Visual design lives in popup.css, embedded at build time.

mod card;
mod icons;

use gettextrs::gettext;
use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use icons::{ACCENT, GRAY, icon_svg, svg_image};

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Icon hovered/clicked: expand into the card and start translating.
    Expand,
    Copy,
    TogglePin,
    Retry,
    Close,
    /// Switch translation engine for this session (backend index).
    SwitchBackend(u32),
    /// The backend list opened; its GTK popup lives outside the card rect.
    BackendMenuOpened,
    /// Source text edited in the card; retranslate.
    Edited(String),
}

const CSS: &str = include_str!("popup.css");

/// Position with edge flipping: prefer below-right of the cursor; flip to
/// above/left when that would overflow; always fully on-screen.
fn place(cx: i32, cy: i32, w: i32, h: i32, sw: i32, sh: i32) -> (i32, i32) {
    const PAD: i32 = 8;
    const OFF_X: i32 = 12;
    const OFF_Y: i32 = 18;
    let mx = if cx + OFF_X + w > sw - PAD {
        (cx - OFF_X - w).max(PAD)
    } else {
        cx + OFF_X
    };
    let my = if cy + OFF_Y + h > sh - PAD {
        (cy - OFF_Y - h).max(PAD)
    } else {
        cy + OFF_Y
    };
    (
        mx.clamp(PAD, (sw - w - PAD).max(PAD)),
        my.clamp(PAD, (sh - h - PAD).max(PAD)),
    )
}

/// Keep in sync with .waytranslate-popup sizing for edge clamping.
pub const CARD_W: i32 = 400;
pub const CARD_H: i32 = 220; // conservative upper bound pre-measure
pub const ICON: i32 = 30;

type PopupRect = std::rc::Rc<std::cell::Cell<Option<(i32, i32, i32, i32)>>>;

pub struct Popup {
    window: gtk4::ApplicationWindow,
    stack: gtk4::Stack,
    source: gtk4::TextView,
    edit_guard: std::rc::Rc<std::cell::Cell<bool>>,
    target: gtk4::Label,
    caret: gtk4::Box,
    skeleton: gtk4::Box,
    error_box: gtk4::Box,
    copy_btn: gtk4::Button,
    pinned: std::cell::Cell<bool>,
    /// Current on-screen rect (x, y, w, h) for outside-click dismissal;
    /// updated on show and on drag. None when hidden.
    rect: PopupRect,
    tracker: std::rc::Rc<std::cell::RefCell<Option<crate::pointer::PointerTracker>>>,
    screen: std::rc::Rc<std::cell::Cell<(i32, i32)>>,
    pin_btn: gtk4::Button,
    copy_img: gtk4::Image,
}

impl Popup {
    pub fn build(
        app: &gtk4::Application,
        on_action: impl Fn(Action) + 'static,
        hover_expand: bool,
        initial_backend: u32,
    ) -> Self {
        let provider = gtk4::CssProvider::new();
        provider.load_from_string(CSS);
        gtk4::style_context_add_provider_for_display(
            &gtk4::gdk::Display::default().expect("no display"),
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let on_action: std::rc::Rc<dyn Fn(Action)> = std::rc::Rc::new(on_action);

        let window = gtk4::ApplicationWindow::builder()
            .application(app)
            .decorated(false)
            .resizable(false)
            .build();
        window.add_css_class("waytranslate-popup");

        // Layer-shell: overlay layer, no keyboard focus, anchored by margin.
        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::None);
        window.set_namespace(Some("waytranslate"));
        window.set_anchor(Edge::Top, true);
        window.set_anchor(Edge::Left, true);

        // Icon page must size small: default GtkStack is homogeneous (sizes
        // to the LARGEST page), which turned our icon into a card-sized blob.
        let stack = gtk4::Stack::new();
        stack.set_hhomogeneous(false);
        stack.set_vhomogeneous(false);
        window.set_child(Some(&stack));

        // ---- icon page: tiny trigger, expand on hover or click ----
        let icon = gtk4::Button::new();
        icon.set_child(Some(&svg_image("languages", "#ffffff")));
        icon.add_css_class("waytranslate-icon");
        icon.set_tooltip_text(Some(&gettext("Translate")));
        {
            let cb = on_action.clone();
            icon.connect_clicked(move |_| cb(Action::Expand));
            if hover_expand {
                let hover = gtk4::EventControllerMotion::new();
                let cb = on_action.clone();
                hover.connect_enter(move |_, _, _| cb(Action::Expand));
                icon.add_controller(hover);
            }
        }
        stack.add_named(&icon, Some("icon"));

        let edit_guard = std::rc::Rc::new(std::cell::Cell::new(false));
        let card::CardWidgets {
            source,
            target,
            caret,
            skeleton,
            error_box,
            pin_btn,
            copy_img,
            copy_btn,
        } = card::build_card(&stack, &on_action, &edit_guard, initial_backend);

        // Editing needs seat focus, but holding it would suppress other
        // apps' xdg_popups (context menus die when the parent loses focus).
        // Compromise: request OnDemand only while the pointer presses the
        // card; every hide path resets to None.
        let rect_cell: PopupRect = std::rc::Rc::new(std::cell::Cell::new(None));
        let tracker_cell = std::rc::Rc::new(std::cell::RefCell::new(None));
        let screen_cell = std::rc::Rc::new(std::cell::Cell::new((1920, 1080)));
        if let Some(card) = stack.child_by_name("card") {
            let win = window.clone();
            let press = gtk4::GestureClick::new();
            press.connect_pressed(move |_, _, _, _| {
                win.set_keyboard_mode(KeyboardMode::OnDemand);
            });
            card.add_controller(press);
        }
        Self {
            window,
            stack,
            source,
            edit_guard,
            target,
            caret,
            skeleton,
            error_box,
            copy_btn,
            pinned: std::cell::Cell::new(false),
            rect: rect_cell,
            tracker: tracker_cell,
            screen: screen_cell,
            pin_btn,
            copy_img,
        }
    }

    /// Show the small trigger icon near (x, y), flipping sides at edges.
    pub fn show_icon_at(&self, x: i32, y: i32, sw: i32, sh: i32) {
        self.stack.set_visible_child_name("icon");
        let (mx, my) = place(x, y, ICON, ICON, sw, sh);
        self.set_pos(mx, my);
        self.rect.set(Some((mx, my, ICON, ICON)));
        self.window.present();
    }

    /// Switch to the card page and present (no repositioning).
    pub fn present_card(&self) {
        self.stack.set_visible_child_name("card");
        if !self.window.is_visible() {
            self.window.present();
        }
    }

    /// Show the full card near (x, y), flipping sides at edges.
    pub fn show_at(&self, x: i32, y: i32, sw: i32, sh: i32) {
        self.screen.set((sw, sh));
        let (mx, my) = place(x, y, CARD_W, CARD_H, sw, sh);
        self.set_pos(mx, my);
        self.rect.set(Some((mx, my, CARD_W, CARD_H)));
        self.present_card();
    }

    fn set_pos(&self, mx: i32, my: i32) {
        self.window.set_margin(Edge::Left, mx);
        self.window.set_margin(Edge::Top, my);
    }

    /// Current on-screen rect, for outside-click dismissal.
    pub fn rect(&self) -> Option<(i32, i32, i32, i32)> {
        self.rect.get()
    }

    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }

    pub fn hide(&self) {
        if !self.pinned.get() {
            self.window.set_visible(false);
            self.window.set_keyboard_mode(KeyboardMode::None);
            self.rect.set(None);
        }
    }

    pub fn force_hide(&self) {
        self.window.set_visible(false);
        self.window.set_keyboard_mode(KeyboardMode::None);
        self.rect.set(None);
    }

    /// First card display: populate the editable source without emitting an edit.
    pub fn begin(&self, source_text: &str) {
        self.edit_guard.set(true);
        let buf = self.source.buffer();
        buf.set_text(source_text);
        // A programmatic load should start at the top; user edits use reset_result.
        buf.place_cursor(&buf.start_iter());
        self.edit_guard.set(false);
        self.reset_result();
    }

    /// Start a retranslation without touching the user's TextView or cursor.
    pub fn reset_result(&self) {
        self.target.set_text("");
        self.target.set_visible(false);
        self.skeleton.remove_css_class("dim");
        self.skeleton.set_visible(true);
        self.caret.set_visible(false);
        self.copy_btn.set_sensitive(false);
        self.error_box.set_visible(false);
    }

    /// Stream/refresh translated text (call with the full accumulated text).
    pub fn set_translation(&self, text: &str) {
        self.skeleton.set_visible(false);
        self.target.set_visible(true);
        self.caret.remove_css_class("dim");
        self.caret.set_visible(true);
        self.copy_btn.set_sensitive(true);
        self.target.set_text(text);
    }

    /// Translation finished: hide the streaming caret.
    pub fn finish(&self) {
        self.caret.set_visible(false);
    }

    pub fn set_error(&self, msg: &str) {
        self.skeleton.set_visible(false);
        self.target.set_visible(false);
        self.caret.set_visible(false);
        self.copy_btn.set_sensitive(false);
        self.error_box.set_visible(true);
        if let Some(l) = self
            .error_box
            .first_child()
            .and_then(|w| w.downcast::<gtk4::Label>().ok())
        {
            l.set_text(msg);
        }
    }

    pub fn set_pinned(&self, pinned: bool) {
        self.pinned.set(pinned);
        // State visible: accent pin while pinned, gray when free.
        self.pin_btn
            .set_child(Some(&svg_image("pin", if pinned { ACCENT } else { GRAY })));
        if pinned {
            self.pin_btn.add_css_class("active");
        } else {
            self.pin_btn.remove_css_class("active");
        }
    }

    /// Copy feedback: swap the copy icon to a check for a beat.
    pub fn set_copied(&self) {
        self.copy_img.set_paintable(Some(
            &gtk4::gdk::Texture::from_bytes(&gtk4::glib::Bytes::from(
                icon_svg("check", ACCENT).as_bytes(),
            ))
            .expect("embedded svg must parse"),
        ));
        let img = self.copy_img.clone();
        gtk4::glib::timeout_add_local_once(std::time::Duration::from_millis(800), move || {
            img.set_paintable(Some(
                &gtk4::gdk::Texture::from_bytes(&gtk4::glib::Bytes::from(
                    icon_svg("copy", GRAY).as_bytes(),
                ))
                .expect("embedded svg must parse"),
            ));
        });
    }

    pub fn toggle_pin(&self) {
        self.set_pinned(!self.pinned.get());
    }
}

impl Popup {
    /// Keep the pointer tracker synchronized while the popup is idle.
    pub fn install_resync(&self, tracker: crate::pointer::PointerTracker) {
        *self.tracker.borrow_mut() = Some(tracker.clone());
        let win = self.window.clone();
        let motion = gtk4::EventControllerMotion::new();
        motion.connect_motion(move |_, x, y| {
            let ml = gtk4_layer_shell::LayerShell::margin(&win, Edge::Left);
            let mt = gtk4_layer_shell::LayerShell::margin(&win, Edge::Top);
            tracker.resync(f64::from(ml) + x, f64::from(mt) + y);
        });
        self.window.add_controller(motion);
    }
}
