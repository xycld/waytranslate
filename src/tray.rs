//! System tray via StatusNotifierItem (D-Bus, KDE native).
//! The daemon has no persistent window — the tray is its front door:
//! settings, pause/resume, quit.

use gettextrs::gettext;
use ksni::TrayMethods as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCmd {
    OpenSettings,
    TogglePause,
    Quit,
}

struct AppTray {
    tx: async_channel::Sender<TrayCmd>,
    paused: bool,
}

impl ksni::Tray for AppTray {
    const MENU_ON_ACTIVATE: bool = true; // left click opens menu; no default window

    fn id(&self) -> String {
        "waytranslate".into()
    }

    fn title(&self) -> String {
        gettext("waytranslate — Selection Translator")
    }

    fn icon_name(&self) -> String {
        "waytranslate".into()
    }

    // Embedded pixmap fallback: icon themes can lag (Plasma caches SNI icons
    // per session); a pixmap always renders.
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        tray_icons()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: gettext("waytranslate — Selection Translator"),
            description: if self.paused {
                gettext("Paused — no popup on selection")
            } else {
                gettext("Running — select text, click the ball to translate")
            },
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        let paused = self.paused;
        vec![
            StandardItem {
                label: gettext("Settings…"),
                icon_name: "configure".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.tx.send_blocking(TrayCmd::OpenSettings);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: if paused {
                    gettext("Resume")
                } else {
                    gettext("Pause")
                },
                icon_name: if paused {
                    "media-playback-start"
                } else {
                    "media-playback-pause"
                }
                .into(),
                activate: Box::new(|t: &mut Self| {
                    t.paused = !t.paused;
                    let _ = t.tx.send_blocking(TrayCmd::TogglePause);
                }),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: gettext("Quit"),
                icon_name: "application-exit".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.tx.send_blocking(TrayCmd::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Spawn the tray on the given tokio runtime. Returns the command receiver.
pub fn spawn(rt: &tokio::runtime::Handle) -> async_channel::Receiver<TrayCmd> {
    let (tx, rx) = async_channel::unbounded();
    rt.spawn(async move {
        let tray = AppTray { tx, paused: false };
        match tray.spawn().await {
            Ok(_handle) => {
                tracing::info!("tray up");
                // Handle dropped = service stops; park forever instead.
                std::future::pending::<()>().await;
            }
            Err(e) => tracing::error!("tray failed: {e}"),
        }
    });
    rx
}

/// Decode the bundled PNGs once; return ksni ARGB32 icons.
fn tray_icons() -> Vec<ksni::Icon> {
    static ICONS: std::sync::OnceLock<Vec<ksni::Icon>> = std::sync::OnceLock::new();
    ICONS
        .get_or_init(|| {
            [
                (
                    24,
                    include_bytes!("../assets/waytranslate-24.png").as_slice(),
                ),
                (
                    48,
                    include_bytes!("../assets/waytranslate-48.png").as_slice(),
                ),
            ]
            .into_iter()
            .filter_map(|(size, png)| decode_argb(png, size))
            .collect()
        })
        .clone()
}

/// PNG (RGBA8) → ksni `Icon` (ARGB32, network byte order).
fn decode_argb(png: &[u8], size: i32) -> Option<ksni::Icon> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
    decoder.set_transformations(png::Transformations::ALPHA);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let rgba = &buf[..info.buffer_size()];
    let mut data = Vec::with_capacity(rgba.len());
    let (pixels, _) = rgba.as_chunks::<4>();
    for &[r, g, b, a] in pixels {
        data.extend_from_slice(&[a, r, g, b]);
    }
    Some(ksni::Icon {
        width: size,
        height: size,
        data,
    })
}
