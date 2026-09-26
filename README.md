# waytranslate

Select text anywhere on Wayland, get a translation popup right at the cursor.
No hotkeys, no window switching: a small icon appears next to your selection —
hover or click it to expand the full translation card.

## Features

- **Works in any app** — watches the primary selection over `ext-data-control-v1`, no plugins needed
- **Translation card at the cursor** — editable source text (retranslates as you edit), copy / pin / retry, dismiss by clicking elsewhere
- **12 backends** — Google, Bing, DeepLX (zero-config); DeepL, Microsoft, NiuTrans, Tencent, Volcengine, Baidu, LibreTranslate; any OpenAI-compatible endpoint (Ollama, vLLM, cloud APIs); or a custom HTTP template
- **Switch engine on the fly** from the card (session-scoped, config untouched)
- **GUI settings** with autostart toggle; system tray for pause / settings / quit
- Chinese and English UI

## Requirements

- Wayland compositor with `ext-data-control-v1` (e.g. KDE Plasma 6, recent Sway/Hyprland). Run `waytranslate doctor` to check your session
- Read access to `/dev/input/event*` for pointer tracking (add yourself to the `input` group: `sudo usermod -aG input $USER`, then re-login)
- Runtime libraries: GTK 4, gtk4-layer-shell

## Install

**Arch Linux** — build the included PKGBUILD (installs binary, icons, desktop entry, autostart, locale):

```sh
makepkg -si
```

**Manually:**

```sh
cargo build --release
sudo install -Dm755 target/release/waytranslate /usr/local/bin/waytranslate
```

## Usage

```sh
waytranslate run              # start the daemon (this is the app)
```

Select some text — the icon pops up next to it. Hover or click to translate.

| Command | Purpose |
| --- | --- |
| `waytranslate settings` | Open the settings window (forwards into the running daemon) |
| `waytranslate doctor` | Probe compositor/protocol/input compatibility and report |
| `waytranslate translate "text" --backend bing` | One-off translation in the terminal |
| `waytranslate config-sample` | Print the fully commented sample config |

Autostart: toggle it in the settings window, or let the Arch package's
`/etc/xdg/autostart` entry handle it.

## Configuration

`~/.config/waytranslate/config.toml` — entirely optional; every key has a
default. Common settings (backend, direction, API keys) live in the GUI; the
file also exposes a `custom` backend template for arbitrary HTTP translation
APIs. See `waytranslate config-sample`.

## How it works

- Own Wayland connection (separate from GTK's) watches the primary selection via `ext-data-control-v1`
- Pointer position tracked from evdev (`/dev/input`), corrected with exact compositor-native coordinates where available (KWin script / Hyprland IPC / GNOME Shell eval)
- Popup is a GTK4 layer-shell overlay surface — never steals keyboard focus
- Tray via StatusNotifierItem (D-Bus)

## License

MIT
