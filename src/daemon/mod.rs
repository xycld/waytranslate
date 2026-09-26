//! Daemon: glue between selection watcher, pointer tracker, popup, translator,
//! tray, and settings.
//!
//! Flow: primary selection → settle → small icon near the pointer; hover/click
//! expands the card and translates. Clicking outside dismisses (unless pinned).
//!
//! Single instance: `waytranslate settings` while running opens the settings
//! window inside the daemon process (HANDLES_COMMAND_LINE).

mod state;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gettextrs::gettext;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;

use crate::pointer::PointerEvent;
use crate::popup::Popup;
use crate::tray::{self, TrayCmd};
use crate::wayland::{self, SelectionEvent};
use crate::{pointer, settings};

use state::{Daemon, Phase};

pub fn run() -> anyhow::Result<()> {
    // Detach from the launcher: a tray daemon must outlive whatever started
    // it (shell, gtk-launch, klauncher). Fails quietly if already a leader.
    let _ = rustix::process::setsid();

    let app = gtk4::Application::new(Some("io.github.xycld.waytranslate"), Default::default());
    app.add_main_option(
        "settings",
        b's'.into(),
        glib::OptionFlags::NONE,
        glib::OptionArg::None,
        "open the settings window",
        None,
    );
    app.set_flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE);
    app.connect_command_line(|app, cmd| {
        // Every invocation lands here (single instance): `waytranslate run`
        // inits the daemon once; `waytranslate settings` opens settings.
        let want_settings = cmd.arguments().iter().any(|a| a == "settings");
        handle_command(app, want_settings);
        glib::ExitCode::SUCCESS
    });
    let code = app.run();
    std::process::exit(i32::from(code));
}

// GTK main thread only — thread_local avoids Sync requirements.
thread_local! {
    static DAEMON_SLOT: RefCell<Option<Rc<Daemon>>> = const { RefCell::new(None) };
}

fn handle_command(app: &gtk4::Application, want_settings: bool) {
    DAEMON_SLOT.with(|slot| {
        let mut borrow = slot.borrow_mut();
        match borrow.as_ref() {
            Some(daemon) => {
                if want_settings {
                    daemon.clone().open_settings();
                }
            }
            None => {
                if want_settings {
                    // Standalone settings editor: saves the file; no live daemon.
                    let cfg = crate::config::load().unwrap_or_default();
                    settings::open(app, &cfg, |_| {});
                    std::mem::forget(app.hold());
                } else {
                    match activate(app) {
                        Ok(d) => {
                            *borrow = Some(d);
                        }
                        Err(e) => {
                            // Contract: fail loudly, never run degraded.
                            eprintln!("waytranslate: {e:#}");
                            std::process::exit(2);
                        }
                    }
                }
            }
        }
    });
}

fn activate(app: &gtk4::Application) -> anyhow::Result<Rc<Daemon>> {
    std::mem::forget(app.hold()); // no persistent window; hold for app lifetime

    let (sel_rx, cmd) = wayland::spawn()?;

    let (sw, sh) = primary_output_size().unwrap_or((1920.0, 1080.0));
    let (tracker, ptr_events) = pointer::spawn((sw, sh))?;

    let cfg = crate::config::load()?;
    crate::i18n::apply(&cfg.language);
    let backend = crate::translate::make_backend(&cfg)?;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;

    let tray_rx = tray::spawn(rt.handle());

    let (res_tx, res_rx) = async_channel::unbounded::<(u64, anyhow::Result<String>)>();

    let state = Rc::new(Daemon {
        cfg: RefCell::new(cfg),
        backend: RefCell::new(backend),
        session_backend: RefCell::new(None),
        paused: Cell::new(false),
        phase: Cell::new(Phase::Idle),
        generation: Cell::new(0),
        ignore_next_outside_press: Cell::new(false),
        pending: RefCell::new(String::new()),
        translation: RefCell::new(String::new()),
        popup: RefCell::new(None),
        app: app.clone(),
        cmd,
        tracker,
        screen: (sw as i32, sh as i32),
        rt,
        res_tx,
    });

    {
        let s = Rc::clone(&state);
        let hover_expand = state.cfg.borrow().expand == "hover";
        let backend_idx = crate::translate::BACKEND_IDS
            .iter()
            .position(|b| *b == state.cfg.borrow().backend)
            .unwrap_or(0) as u32;
        let popup = Popup::build(
            app,
            move |a| s.clone().handle_action(a),
            hover_expand,
            backend_idx,
        );
        popup.install_resync(state.tracker.clone());
        *state.popup.borrow_mut() = Some(popup);
    }

    spawn_loops(&state, sel_rx, ptr_events, tray_rx, res_rx);
    Ok(state)
}

/// All glib-side event loops, one per input channel.
fn spawn_loops(
    state: &Rc<Daemon>,
    sel_rx: async_channel::Receiver<SelectionEvent>,
    ptr_events: tokio::sync::mpsc::UnboundedReceiver<PointerEvent>,
    tray_rx: async_channel::Receiver<TrayCmd>,
    res_rx: async_channel::Receiver<(u64, anyhow::Result<String>)>,
) {
    // Selection events (wayland thread).
    {
        let s = Rc::clone(state);
        glib::spawn_future_local(async move {
            while let Ok(SelectionEvent::PrimaryChanged(text)) = sel_rx.recv().await {
                s.clone().on_selection(&text);
            }
        });
    }

    // Pointer buttons (evdev threads): click outside the popup dismisses it.
    {
        let s = Rc::clone(state);
        glib::spawn_future_local(async move {
            let mut ptr_events = ptr_events;
            while let Some(ev) = ptr_events.recv().await {
                if ev == PointerEvent::Press {
                    s.clone().on_pointer_press();
                }
            }
        });
    }

    // Tray menu commands.
    {
        let s = Rc::clone(state);
        glib::spawn_future_local(async move {
            while let Ok(cmd) = tray_rx.recv().await {
                match cmd {
                    TrayCmd::OpenSettings => s.clone().open_settings(),
                    TrayCmd::TogglePause => s.paused.set(!s.paused.get()),
                    TrayCmd::Quit => std::process::exit(0),
                }
            }
        });
    }

    // Translation results → popup (stale generations discarded).
    {
        let s = Rc::clone(state);
        glib::spawn_future_local(async move {
            while let Ok((job, result)) = res_rx.recv().await {
                if job != s.generation.get() {
                    continue;
                }
                let popup = s.popup.borrow();
                let Some(popup) = popup.as_ref() else {
                    continue;
                };
                match result {
                    Ok(text) => {
                        *s.translation.borrow_mut() = text.clone();
                        popup.set_translation(&text);
                        popup.finish();
                    }
                    Err(e) => popup.set_error(&format!("{}{e:#}", gettext("Translation failed: "))),
                }
            }
        });
    }
}

fn primary_output_size() -> Option<(f64, f64)> {
    let display = gtk4::gdk::Display::default()?;
    let monitors = display.monitors();
    let monitor = monitors.item(0)?.downcast::<gtk4::gdk::Monitor>().ok()?;
    let geo = monitor.geometry();
    Some((geo.width() as f64, geo.height() as f64))
}
