//! `waytranslate doctor`: probe session capabilities, declare compatibility
//! loudly.
//!
//! Contract: never degrade silently. If a hard requirement is missing, say so
//! and name the culprit compositor/protocol.
//!
//! `waytranslate doctor fix` repairs what's locally repairable: a config file
//! that fails to parse (daemon refuses to start) and missing `input` group
//! membership. Compositor protocol gaps are not fixable from here.

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

/// Everything the report/fix steps need, collected once.
struct Report {
    desktop: String,
    probes: Vec<Probe>,
    evdev_ok: bool,
    config_ok: bool,
}

fn collect(paint: &Paint) -> anyhow::Result<Report> {
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
    let config_ok = crate::config::load().is_ok();
    // Compositor-native cursor position (KWin script / Hyprland IPC)
    probes.push(Probe {
        name: "compositor-native cursor position (mouse-coords)",
        purpose: "exact popup position at the real cursor (fallback: evdev tracking)",
        required: false,
        found: mouse_coords::get_position().is_ok(),
    });
    probes.push(Probe {
        name: "/dev/input/event* readable (group: input)",
        purpose: "pointer position for popup anchoring (`doctor fix` repairs)",
        required: false,
        found: evdev_ok,
    });
    probes.push(Probe {
        name: "config.toml parses",
        purpose: "a broken config kills the daemon at startup (`doctor fix` repairs)",
        required: false,
        found: config_ok,
    });

    Ok(Report {
        desktop,
        probes,
        evdev_ok,
        config_ok,
    })
}

/// Print probe lines; returns true when a required protocol is missing.
fn print_probes(paint: &Paint, report: &Report) -> bool {
    let mut failed_required = false;
    for p in &report.probes {
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
    failed_required
}

fn verdict(paint: &Paint, failed_required: bool, desktop: &str) {
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
}

pub fn run() -> anyhow::Result<()> {
    let paint = Paint::new();
    let report = collect(&paint)?;
    let failed_required = print_probes(&paint, &report);
    verdict(&paint, failed_required, &report.desktop);
    Ok(())
}

pub fn fix() -> anyhow::Result<()> {
    let paint = Paint::new();
    let report = collect(&paint)?;
    let failed_required = print_probes(&paint, &report);

    println!("{}", paint.bold("── fix ──"));
    let mut fixed = 0;
    let mut manual = 0;

    // 1. Broken config.toml: move it aside, defaults take over.
    if report.config_ok {
        println!("  {} config.toml parses", paint.green("✓"));
    } else {
        let path = crate::config::path();
        let bak = path.with_extension("toml.bak");
        std::fs::rename(&path, &bak)?;
        // Prove the repair: defaults must load now.
        crate::config::load()?;
        println!(
            "  {} broken config moved to {}",
            paint.green("✓"),
            bak.display()
        );
        fixed += 1;
    }

    // 2. /dev/input unreadable: add the user to the `input` group.
    if report.evdev_ok {
        println!("  {} /dev/input/event* readable", paint.green("✓"));
    } else {
        match fix_input_group(&paint) {
            Ok(msg) => {
                println!("  {} {msg}", paint.green("✓"));
                fixed += 1;
            }
            Err(msg) => {
                println!("  {} {msg}", paint.yellow("✗"));
                manual += 1;
            }
        }
    }

    println!();
    if failed_required {
        println!(
            "{}",
            paint.yellow(
                "Compositor protocol gaps above are NOT fixable here — switch compositor or session."
            )
        );
    }
    match (fixed, manual) {
        (0, 0) => println!("{}", paint.green("nothing to fix.")),
        _ => println!("fixed {fixed}, need manual action {manual}."),
    }
    if failed_required {
        std::process::exit(1);
    }
    Ok(())
}

/// Add $USER to the `input` group via pkexec (GUI auth) or sudo.
fn fix_input_group(paint: &Paint) -> Result<String, String> {
    let user = std::env::var("USER").map_err(|_| "$USER is not set".to_string())?;

    let in_group = std::process::Command::new("id")
        .arg("-Gn")
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .any(|g| g == "input")
        })
        .unwrap_or(false);
    if in_group {
        return Err(
            "already in group 'input' but /dev/input is still unreadable — re-login first;\n       otherwise check udev permissions on /dev/input/event* manually"
                .to_string(),
        );
    }

    let usermod = ["/usr/sbin/usermod", "/sbin/usermod", "/usr/bin/usermod"]
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .map(|s| s.to_string())
        .ok_or_else(|| "usermod not found".to_string())?;

    let shell_cmd = format!("{usermod} -aG input {user}");
    let attempts = [
        ("pkexec", vec![shell_cmd.clone()]),
        ("sudo", vec!["sh".into(), "-c".into(), shell_cmd.clone()]),
    ];
    for (elev, args) in attempts {
        let Some(path) = which(elev) else { continue };
        println!(
            "       {}",
            paint.dim(&format!(
                "→ {elev} {shell_cmd}  (authenticate in the prompt)"
            ))
        );
        match std::process::Command::new(path).args(&args).status() {
            Ok(s) if s.success() => {
                return Ok(format!(
                    "added {user} to group 'input'. Group changes only apply to NEW sessions —\n       log out/in (or `newgrp input`), then start waytranslate."
                ));
            }
            _ => continue,
        }
    }
    Err(format!(
        "needs root; run manually: sudo usermod -aG input {user}  (then re-login)"
    ))
}

fn which(cmd: &str) -> Option<String> {
    std::process::Command::new("sh")
        .args(["-c", &format!("command -v {cmd}")])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
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
