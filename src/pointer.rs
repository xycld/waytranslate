//! Pointer observation via evdev.
//!
//! Wayland clients cannot ask the compositor where the pointer is, so we
//! track it ourselves from /dev/input. Mice report relative deltas (we
//! integrate), touchpads/tablets report absolute positions (we scale).
//! The GTK popup corrects drift on every pointer-enter with exact coords.
//!
//! Requires read access to /dev/input/event* (group `input`) — doctor checks.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use evdev::{AbsoluteAxisCode, Device, EventSummary, KeyCode, RelativeAxisCode};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointerEvent {
    /// Left button pressed (potential drag-select start).
    Press,
    /// Left button released (selection finished).
    Release,
}

#[derive(Debug, Clone, Copy)]
pub struct PointerState {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone)]
pub struct PointerTracker {
    state: Arc<Mutex<PointerState>>,
}

impl PointerTracker {
    pub fn position(&self) -> PointerState {
        *self.state.lock().unwrap()
    }

    /// Correct accumulated drift with an exact Wayland-side coordinate
    /// (called when the popup sees pointer-enter/motion).
    pub fn resync(&self, x: f64, y: f64) {
        *self.state.lock().unwrap() = PointerState { x, y };
    }
}

/// Spawn tracker threads over every pointer-like `evdev` node.
/// `screen` is the primary output size used to seed/scale coordinates.
pub fn spawn(
    screen: (f64, f64),
) -> anyhow::Result<(
    PointerTracker,
    tokio::sync::mpsc::UnboundedReceiver<PointerEvent>,
)> {
    let state = Arc::new(Mutex::new(PointerState {
        x: screen.0 / 2.0,
        y: screen.1 / 2.0,
    }));
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    let devices = find_pointer_devices();
    anyhow::ensure!(
        !devices.is_empty(),
        "no readable pointer device under /dev/input (need group `input`)"
    );

    for path in devices {
        let state = Arc::clone(&state);
        let tx = tx.clone();
        std::thread::Builder::new()
            .name(format!("evdev:{}", path.display()))
            .spawn(move || watch_device(&path, screen, state, tx))?;
    }

    Ok((PointerTracker { state }, rx))
}

fn find_pointer_devices() -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir("/dev/input") else {
        return Vec::new();
    };
    rd.filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("event"))
        })
        .filter(|p| {
            Device::open(p).is_ok_and(|d| {
                // Never watch our own injection devices.
                if d.name().is_some_and(|n| {
                    n.starts_with("waytranslate-inject") || n.starts_with("waytranslate-inject")
                }) {
                    return false;
                }
                d.supported_keys()
                    .is_some_and(|k| k.contains(KeyCode::BTN_LEFT))
                    && (d.supported_relative_axes().is_some_and(|r| {
                        r.contains(RelativeAxisCode::REL_X) && r.contains(RelativeAxisCode::REL_Y)
                    }) || d.supported_absolute_axes().is_some_and(|a| {
                        a.contains(AbsoluteAxisCode::ABS_X) && a.contains(AbsoluteAxisCode::ABS_Y)
                    }))
            })
        })
        .collect()
}

fn watch_device(
    path: &std::path::Path,
    screen: (f64, f64),
    state: Arc<Mutex<PointerState>>,
    tx: tokio::sync::mpsc::UnboundedSender<PointerEvent>,
) {
    let mut dev = match Device::open(path) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("{}: open failed: {e}", path.display());
            return;
        }
    };
    tracing::debug!("watching pointer device {}", path.display());

    // Absolute-axis ranges for scaling (touchpads/tablets).
    let abs_range = |axis: AbsoluteAxisCode| {
        dev.get_abs_state().ok().and_then(|st| {
            let info = &st[axis.0 as usize];
            (info.maximum > info.minimum).then_some((info.minimum, info.maximum))
        })
    };
    let x_range = abs_range(AbsoluteAxisCode::ABS_X);
    let y_range = abs_range(AbsoluteAxisCode::ABS_Y);

    loop {
        let events = match dev.fetch_events() {
            Ok(evs) => evs,
            Err(e) => {
                tracing::warn!("{}: read failed: {e}", path.display());
                std::thread::sleep(std::time::Duration::from_millis(200));
                continue;
            }
        };
        for ev in events {
            match ev.destructure() {
                EventSummary::RelativeAxis(_, RelativeAxisCode::REL_X, value) => {
                    let mut st = state.lock().unwrap();
                    st.x = (st.x + value as f64).clamp(0.0, screen.0 - 1.0);
                }
                EventSummary::RelativeAxis(_, RelativeAxisCode::REL_Y, value) => {
                    let mut st = state.lock().unwrap();
                    st.y = (st.y + value as f64).clamp(0.0, screen.1 - 1.0);
                }
                EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_X, value) => {
                    if let Some((min, max)) = x_range
                        && max > min
                    {
                        let mut st = state.lock().unwrap();
                        st.x = f64::from(value - min) / f64::from(max - min) * screen.0;
                    }
                }
                EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_Y, value) => {
                    if let Some((min, max)) = y_range
                        && max > min
                    {
                        let mut st = state.lock().unwrap();
                        st.y = f64::from(value - min) / f64::from(max - min) * screen.1;
                    }
                }
                EventSummary::Key(_, KeyCode::BTN_LEFT, value) => {
                    let _ = match value {
                        1 => tx.send(PointerEvent::Press),
                        0 => tx.send(PointerEvent::Release),
                        _ => continue,
                    };
                }
                _ => {}
            }
        }
    }
}

// ---------- Exact cursor position (compositor-native) ----------
//
// mouse-coords routes per compositor: KWin scripting on KDE Wayland (live,
// never frozen), Hyprland IPC, GNOME Shell Eval. XQueryPointer on XWayland
// is NOT used — KWin only feeds it while the pointer is over an X11 window,
// so it freezes over native Wayland apps (SceneSwitcher #512, KWin #520910).
//
// mouse-coords returns physical pixels (global, summed across monitors);
// we divide by the monitor's fractional scale to get layer-shell's logical
// coordinates. Unknown Wayland compositors error out — the daemon falls
// back to the evdev tracker there.

/// Exact cursor position, or None when the compositor has no native API.
pub fn cursor_pos() -> Option<(f64, f64)> {
    // Wayland-native backends (KWin script / Hyprland IPC / GNOME Eval)
    // already report compositor-logical coordinates — no scaling needed.
    let p = mouse_coords::get_position().ok()?;
    Some((p.x as f64, p.y as f64))
}
