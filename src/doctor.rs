//! `waytranslate doctor`: probe session capabilities, declare compatibility
//! loudly.
//!
//! Contract: never degrade silently. If a hard requirement is missing, say so
//! and name the culprit compositor/protocol.

use wayland_client::{Connection, globals::registry_queue_init};

struct Probe {
    name: &'static str,
    purpose: &'static str,
    required: bool,
    found: bool,
}

// ANSI colors: on when writing to a terminal, off under NO_COLOR or pipes.
struct Paint {
    on: bool,
}
impl Paint {
    fn new() -> Self {
        Self {
            on: std::io::IsTerminal::is_terminal(&std::io::stdout())
                && std::env::var_os("NO_COLOR").is_none(),
        }
    }
    fn green(&self, s: &str) -> String {
        if self.on {
            format!("\x1b[32m{s}\x1b[0m")
        } else {
            s.into()
        }
    }
    fn red(&self, s: &str) -> String {
        if self.on {
            format!("\x1b[31m{s}\x1b[0m")
        } else {
            s.into()
        }
    }
    fn yellow(&self, s: &str) -> String {
        if self.on {
            format!("\x1b[33m{s}\x1b[0m")
        } else {
            s.into()
        }
    }
    fn dim(&self, s: &str) -> String {
        if self.on {
            format!("\x1b[2m{s}\x1b[0m")
        } else {
            s.into()
        }
    }
    fn bold(&self, s: &str) -> String {
        if self.on {
            format!("\x1b[1m{s}\x1b[0m")
        } else {
            s.into()
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    let paint = Paint::new();
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "unknown".into());
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".into());

    println!("{}", paint.bold("waytranslate doctor"));
    println!("  {} {session}", paint.dim("session type :"));
    println!("  {} {desktop}", paint.dim("desktop      :"));
    println!();

    if session != "wayland" {
        println!(
            "{}",
            paint.red("✗ not a Wayland session. waytranslate is Wayland-only by design;")
        );
        println!("  there is no X11 backend and none is planned.");
        std::process::exit(1);
    }

    let conn = Connection::connect_to_env()?;
    let globals = registry_queue_init::<State>(&conn)?.0;

    let mut ifaces: Vec<String> = Vec::new();
    globals.contents().with_list(|list| {
        ifaces = list.iter().map(|g| g.interface.clone()).collect();
    });
    let has = |iface: &str| ifaces.iter().any(|i| i == iface);

    let mut probes = vec![
        Probe {
            name: "ext_data_control_manager_v1 / zwlr_data_control_manager_v1",
            purpose: "selection monitoring (read what you select, without Ctrl+C)",
            required: true,
            found: has("ext_data_control_manager_v1") || has("zwlr_data_control_manager_v1"),
        },
        Probe {
            name: "zwlr_layer_shell_v1",
            purpose: "overlay popup placement",
            required: true,
            found: has("zwlr_layer_shell_v1"),
        },
        Probe {
            name: "ext_foreign_toplevel_list_v1 / zwlr_foreign_toplevel_manager_v1",
            purpose: "focused-app blacklist (never pop up in password managers)",
            required: false,
            found: has("ext_foreign_toplevel_list_v1") || has("zwlr_foreign_toplevel_manager_v1"),
        },
    ];

    // evdev: pointer position + button-release detection
    let evdev_ok = std::fs::read_dir("/dev/input")
        .map(|rd| {
            rd.filter_map(Result::ok).any(|e| {
                e.file_name().to_string_lossy().starts_with("event")
                    && std::fs::OpenOptions::new()
                        .read(true)
                        .open(e.path())
                        .is_ok()
            })
        })
        .unwrap_or(false);
    // Compositor-native cursor position (KWin script / Hyprland IPC)
    probes.push(Probe {
        name: "compositor-native cursor position (mouse-coords)",
        purpose: "exact popup position at the real cursor (fallback: evdev tracking)",
        required: false,
        found: mouse_coords::get_position().is_ok(),
    });
    probes.push(Probe {
        name: "/dev/input/event* readable (group: input)",
        purpose: "pointer position for popup anchoring",
        required: false,
        found: evdev_ok,
    });

    let mut failed_required = false;
    for p in &probes {
        let mark = if p.found {
            paint.green("✓")
        } else if p.required {
            paint.red("✗")
        } else {
            paint.yellow("✗")
        };
        let req = if p.required { "required" } else { "optional" };
        println!("  {mark} [{}] {}", paint.dim(req), p.name);
        println!("       {}", paint.dim(p.purpose));
        if p.required && !p.found {
            failed_required = true;
        }
    }
    println!();

    if failed_required {
        println!(
            "{}",
            paint.red(&paint.bold(&format!(
                "INCOMPATIBLE: this compositor ({desktop}) lacks required protocols above."
            )))
        );
        println!("Known-good: KDE Plasma (KWin), Sway, Hyprland, river, COSMIC.");
        println!(
            "Known-bad : GNOME (Mutter) — no protocol for selection monitoring, by upstream decision."
        );
        std::process::exit(1);
    }

    println!(
        "{}",
        paint.green(&paint.bold("COMPATIBLE: all required protocols present."))
    );
    Ok(())
}

// Minimal registry state; we only enumerate globals.
struct State;

impl
    wayland_client::Dispatch<
        wayland_client::protocol::wl_registry::WlRegistry,
        wayland_client::globals::GlobalListContents,
    > for State
{
    fn event(
        _: &mut State,
        _: &wayland_client::protocol::wl_registry::WlRegistry,
        _: wayland_client::protocol::wl_registry::Event,
        _: &wayland_client::globals::GlobalListContents,
        _: &Connection,
        _: &wayland_client::QueueHandle<State>,
    ) {
    }
}

wayland_client::delegate_noop!(State: wayland_client::protocol::wl_seat::WlSeat);
