# waytranslate (Wayland)

[简体中文](README.zh-CN.md) | English

waytranslate is a native Wayland selection translator: select text in any app and a small icon appears at the cursor — click it to see the translation in a GTK4 popup card, without switching windows.

## Requirements

- Wayland compositor with `ext-data-control-v1` (KDE Plasma 6, recent Sway / Hyprland). Run `waytranslate doctor` to check your session
- GTK 4.14+ and gtk4-layer-shell
- Read access to `/dev/input/event*` (for pointer tracking)

Note: pointer tracking needs the `input` group
(`sudo usermod -aG input $USER`, re-login to take effect). `doctor` checks
this too, and `waytranslate doctor fix` can auto-repair what's fixable.

## Installation

### Arch Linux (AUR)

```bash
paru -S waytranslate-git
or
yay -S waytranslate-git
```

### Manual Build

```bash
cargo build --release
sudo install -Dm755 target/release/waytranslate /usr/local/bin/waytranslate
```

See `PKGBUILD` for the full file list (icons, desktop entries, locale).

## Usage

```bash
waytranslate run   # start the daemon — this is the app
```

Select some text; the icon pops up next to it. Hover or click to expand the
translation card. Click anywhere else to dismiss; pin the card to keep it.

| Command | Description |
|---------|-------------|
| `waytranslate run` | Start the daemon (default) |
| `waytranslate settings` | Open the settings window (forwards into the running daemon) |
| `waytranslate doctor` | Probe compositor / protocol / input compatibility |
| `waytranslate doctor fix` | Auto-repair what's locally fixable (broken config, `input` group) |
| `waytranslate translate "text" --backend bing` | One-off translation in the terminal |
| `waytranslate config-sample` | Print the fully commented sample config |

Autostart: toggle in the settings window, or via the Arch package's
`/etc/xdg/autostart` entry.

## Configuration

`~/.config/waytranslate/config.toml` — entirely optional; every key has a
default. Common options are also editable in the settings window.

| Option | Description | Default |
|--------|-------------|---------|
| `backend` | Backend id from the table below | `google` |
| `expand` | `click` or `hover` — how the icon expands into the card | `click` |
| `target` | `auto` (zh↔en by content), `zh`, `en` | `auto` |
| `language` | UI language: `auto`, `zh-CN`, `en` | `auto` |
| `min_chars` | Selections shorter than this never pop up | `2` |

Per-backend keys (`api_key`, `base_url`, ...) are documented in
`waytranslate config-sample`.

## Technical Notes

### Backends

| Backend | ID | Configuration |
|---------|----|---------------|
| Google | `google` | None |
| Bing | `bing` | None |
| DeepLX | `deeplx` | `base_url` (self-hosted or public relay) |
| LibreTranslate | `libretranslate` | `base_url`, optional `api_key` |
| DeepL | `deepl` | `api_key`, optional `base_url` (Pro) |
| Microsoft | `microsoft` | `api_key`, optional `region` |
| NiuTrans | `niutrans` | `api_key` |
| Tencent | `tencent` | `secret_id`, `secret_key`, optional `region` |
| Volcengine | `volcengine` | `access_key`, `secret_key` |
| Baidu | `baidu` | `appid`, `api_key` |
| OpenAI-compatible | `openai` | `base_url`, `model`, optional `api_key` (Ollama, vLLM, ...) |
| Custom | `custom` | Arbitrary HTTP template (`config-sample` has a full example) |

### Data Sources

- **Selection**: own Wayland connection watching the primary selection (`ext-data-control-v1`), separate from GTK's
- **Pointer**: evdev (`/dev/input`) tracking, corrected with exact compositor-native coordinates where available (KWin script / Hyprland IPC / GNOME Shell eval)
- **Popup**: GTK4 layer-shell overlay surface; never takes keyboard focus
- **Tray**: StatusNotifierItem over D-Bus

## License

MIT. See `LICENSE`.
