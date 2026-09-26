//! Daemon state machine: selection → settle → icon → card, plus all popup
//! actions. GTK main thread only — everything is Rc/Cell/RefCell.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use gettextrs::gettext;
use gtk4::glib;

use crate::config::Config;
use crate::pointer::PointerTracker;
use crate::popup::{Action, Popup};
use crate::translate::{Translator, auto_direction};
use crate::wayland::{self, Cmd};
use crate::{pointer, settings};

/// Selection must be stable this long before the icon shows.
const SETTLE_MS: u64 = 150;

/// `Popup` lifecycle. Empty-selection events (apps clearing primary on focus
/// loss, e.g. Telegram) must never kill an expanded card — that race was the
/// "flash and vanish" bug.
#[derive(Debug, PartialEq, Clone, Copy)]
pub(super) enum Phase {
    Idle,
    Icon,
    Card,
}

pub(super) struct Daemon {
    pub cfg: RefCell<Config>,
    pub backend: RefCell<Arc<dyn Translator>>,
    /// Session-scoped engine override from the card's selector (not persisted).
    pub session_backend: RefCell<Option<Arc<dyn Translator>>>,
    pub paused: Cell<bool>,
    pub phase: Cell<Phase>,
    pub generation: Cell<u64>,
    /// Consume the pointer press used to choose a GtkDropDown popup item.
    pub ignore_next_outside_press: Cell<bool>,
    pub pending: RefCell<String>,
    pub translation: RefCell<String>,
    pub popup: RefCell<Option<Popup>>,
    pub app: gtk4::Application,
    pub cmd: wayland::CmdSender,
    pub tracker: PointerTracker,
    pub screen: (i32, i32),
    pub rt: tokio::runtime::Runtime,
    pub res_tx: async_channel::Sender<(u64, anyhow::Result<String>)>,
}

impl Daemon {
    /// Exact cursor position: XWayland query first, evdev tracker as fallback.
    fn cursor(&self) -> (i32, i32) {
        if let Some((x, y)) = pointer::cursor_pos() {
            // Compositor-native reading is exact; heal the tracker's drift too.
            self.tracker.resync(x, y);
            (x as i32, y as i32)
        } else {
            let p = self.tracker.position();
            (p.x as i32, p.y as i32)
        }
    }

    pub(super) fn open_settings(self: Rc<Self>) {
        let s = self.clone();
        settings::open(&self.app, &self.cfg.borrow(), move |new_cfg| {
            s.apply_config(new_cfg);
        });
    }

    /// Live-apply a saved config (called by the settings window).
    fn apply_config(&self, new_cfg: Config) {
        match crate::translate::make_backend(&new_cfg) {
            Ok(backend) => {
                *self.backend.borrow_mut() = backend;
                crate::i18n::apply(&new_cfg.language);
                *self.cfg.borrow_mut() = new_cfg;
                tracing::info!("config applied");
            }
            Err(e) => tracing::error!("config rejected: {e:#}"),
        }
    }

    pub(super) fn on_selection(self: Rc<Self>, text: &str) {
        if self.paused.get() {
            return;
        }
        let trimmed = text.trim();
        if trimmed.chars().count() < self.cfg.borrow().min_chars {
            self.generation.set(self.generation.get() + 1);
            // Only retract the icon; an expanded card survives selection loss.
            if self.phase.get() == Phase::Icon {
                self.hide_popup();
            }
            return;
        }
        // Apps (Telegram/Qt) re-emit the same selection when losing focus —
        // e.g. right after our icon is clicked. An identical text while the
        // card is open cannot be a new selection; ignoring it fixes the
        // expand→collapse→teleport race.
        if self.phase.get() == Phase::Card && trimmed == self.pending.borrow().as_str() {
            return;
        }

        let job = self.generation.get() + 1;
        self.generation.set(job);
        *self.pending.borrow_mut() = trimmed.to_string();

        glib::timeout_add_local_once(std::time::Duration::from_millis(SETTLE_MS), move || {
            if job != self.generation.get() {
                return; // superseded by a newer selection
            }
            self.clone().show_icon(job);
        });
    }

    /// Source edited inside the card: debounce, then retranslate.
    fn on_edited(self: Rc<Self>, text: String) {
        let trimmed = text.trim();
        if trimmed.chars().count() < self.cfg.borrow().min_chars {
            return;
        }
        let job = self.generation.get() + 1;
        self.generation.set(job);
        *self.pending.borrow_mut() = trimmed.to_string();
        let settle = SETTLE_MS.max(400); // typing needs a longer fuse
        glib::timeout_add_local_once(std::time::Duration::from_millis(settle), move || {
            if job != self.generation.get() {
                return;
            }
            self.clone().fire(job);
        });
    }

    /// Show the trigger icon; it stays until outside-click / new selection.
    fn show_icon(self: Rc<Self>, _job: u64) {
        self.phase.set(Phase::Icon);
        let pos = self.cursor();
        {
            let popup = self.popup.borrow();
            let Some(popup) = popup.as_ref() else { return };
            popup.show_icon_at(pos.0, pos.1, self.screen.0, self.screen.1);
        }
    }

    /// Show the card for the pending selection and start translation.
    /// Shared by the settle timer, icon expand, and Retry.
    fn fire(self: Rc<Self>, job: u64) {
        let was_card = self.phase.get() == Phase::Card;
        self.phase.set(Phase::Card);
        let text = self.pending.borrow().clone();
        if text.is_empty() {
            return;
        }
        {
            let popup = self.popup.borrow();
            let Some(popup) = popup.as_ref() else { return };
            if was_card && popup.is_visible() {
                popup.reset_result();
            } else {
                popup.begin(&text);
            }
            // Always switch to the card page; only the FIRST expand
            // positions the card. Re-translations (edit / engine switch /
            // retry) refresh content in place — never yank the card back
            // to the cursor or off a user's drag.
            if !was_card || !popup.is_visible() {
                let pos = self.cursor();
                popup.show_at(pos.0, pos.1, self.screen.0, self.screen.1);
            } else {
                popup.present_card();
            }
        }

        let to = self
            .cfg
            .borrow()
            .target_lang()
            .unwrap_or_else(|| auto_direction(&text));
        let backend = self
            .session_backend
            .borrow()
            .clone()
            .unwrap_or_else(|| Arc::clone(&self.backend.borrow()));
        let res_tx = self.res_tx.clone();
        self.rt.spawn(async move {
            let result = backend.translate(&text, to).await;
            let _ = res_tx.send((job, result)).await;
        });
    }

    // Click outside the popup rect dismisses it (unless pinned).
    pub(super) fn on_pointer_press(self: Rc<Self>) {
        let Some((x, y, w, h)) = self.popup.borrow().as_ref().and_then(|p| p.rect()) else {
            return;
        };
        let (px, py) = self.cursor();
        if self.ignore_next_outside_press.replace(false) {
            return;
        }
        let inside = px >= x - 8 && px <= x + w + 8 && py >= y - 8 && py <= y + h + 8;
        if !inside {
            self.generation.set(self.generation.get() + 1);
            self.hide_popup();
        }
    }

    fn hide_popup(&self) {
        if let Some(p) = self.popup.borrow().as_ref() {
            p.hide();
        }
        self.phase.set(Phase::Idle);
    }

    pub(super) fn handle_action(self: Rc<Self>, action: Action) {
        if action == Action::Expand {
            // Invalidate any pending settle timer, then translate.
            let job = self.generation.get() + 1;
            self.generation.set(job);
            self.clone().fire(job);
            return;
        }
        let popup_ref = self.popup.borrow();
        let Some(popup) = popup_ref.as_ref() else {
            return;
        };
        match action {
            Action::BackendMenuOpened => {
                self.ignore_next_outside_press.set(true);
            }
            Action::Copy => {
                let text = self.translation.borrow().clone();
                if !text.is_empty() {
                    let _ = self.cmd.send(Cmd::SetClipboard(text));
                    popup.set_copied();
                }
            }
            Action::TogglePin => popup.toggle_pin(),
            Action::Retry => {
                drop(popup_ref);
                self.clone().fire(self.generation.get());
            }
            Action::Close => {
                popup.force_hide();
                self.phase.set(Phase::Idle);
            }
            Action::SwitchBackend(idx) => {
                let mut cfg = self.cfg.borrow().clone();
                cfg.backend = crate::translate::BACKEND_IDS
                    .get(idx as usize)
                    .unwrap_or(&"google")
                    .to_string();
                drop(popup_ref);
                match crate::translate::make_backend(&cfg) {
                    Ok(b) => {
                        *self.session_backend.borrow_mut() = Some(b);
                        self.clone().fire(self.generation.get());
                    }
                    Err(e) => {
                        if let Some(p) = self.popup.borrow().as_ref() {
                            p.set_error(&format!("{}{e:#}", gettext("Translation failed: ")));
                        }
                    }
                }
            }
            Action::Edited(text) => {
                drop(popup_ref);
                self.clone().on_edited(text);
            }
            Action::Expand => unreachable!(),
        }
    }
}
