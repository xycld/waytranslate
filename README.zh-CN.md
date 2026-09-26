# waytranslate (Wayland)

简体中文 | [English](README.md)

waytranslate 是原生 Wayland 下的划词翻译工具：在任意应用里选中文字，光标旁会弹出一个小图标——点击即可在 GTK4 卡片弹窗里看到译文，无需切换窗口。

## 运行要求

- 支持 `ext-data-control-v1` 的 Wayland 合成器（KDE Plasma 6、较新的 Sway / Hyprland）。可运行 `waytranslate doctor` 检查当前会话
- GTK 4.14+ 和 gtk4-layer-shell
- 可读 `/dev/input/event*`（用于指针位置追踪）

注意：指针追踪需要 `input` 用户组
（`sudo usermod -aG input $USER`，重新登录生效）。`doctor` 也会检查此项，你可以输入`waytranslate doctor fix`来自动修复可以修复的问题。

## 安装

### Arch Linux（PKGBUILD）

```bash
paru -S waytranslate-git
or
yay -S waytranslate-git
```

### 手动构建

```bash
cargo build --release
sudo install -Dm755 target/release/waytranslate /usr/local/bin/waytranslate
```

完整文件清单（图标、桌面项、语言文件等）见 `PKGBUILD`。

## 使用

```bash
waytranslate run   # 启动守护进程——这就是应用本体
```

选中一段文字，图标会出现在选区旁边。悬停或点击展开翻译卡片；点击别处
关闭；需要常驻可钉住卡片。

| 命令 | 说明 |
|------|------|
| `waytranslate run` | 启动守护进程（默认） |
| `waytranslate settings` | 打开设置窗口（守护进程运行时转发进其进程） |
| `waytranslate doctor` | 检查合成器 / 协议 / 输入设备兼容性 |
| `waytranslate doctor fix` | 自动修复可修复项（配置损坏、`input` 用户组） |
| `waytranslate translate "文本" --backend bing` | 终端里单次翻译 |
| `waytranslate config-sample` | 打印带完整注释的示例配置 |

开机自启：在设置窗口里开关，或由 Arch 包的 `/etc/xdg/autostart` 条目接管。

## 配置

`~/.config/waytranslate/config.toml` —— 完全可选，每个键都有默认值。
常用选项也可在设置窗口里直接改。

| 选项 | 说明 | 默认值 |
|------|------|--------|
| `backend` | 后端 id，见下表 | `google` |
| `expand` | `click` 或 `hover`——图标展开为卡片的方式 | `click` |
| `target` | `auto`（按内容中↔英）、`zh`、`en` | `auto` |
| `language` | 界面语言：`auto`、`zh-CN`、`en` | `auto` |
| `min_chars` | 短于该字符数的选区不弹窗 | `2` |

各后端的密钥类配置（`api_key`、`base_url` 等）见
`waytranslate config-sample` 的注释说明。

## 技术说明

### 翻译后端

| 后端 | ID | 需要的配置 |
|------|----|-----------|
| Google | `google` | 无 |
| Bing | `bing` | 无 |
| DeepLX | `deeplx` | `base_url`（自建或公共中继） |
| LibreTranslate | `libretranslate` | `base_url`，可选 `api_key` |
| DeepL | `deepl` | `api_key`，Pro 用户加 `base_url` |
| Microsoft | `microsoft` | `api_key`，可选 `region` |
| 小牛 | `niutrans` | `api_key` |
| 腾讯 | `tencent` | `secret_id`、`secret_key`，可选 `region` |
| 火山 | `volcengine` | `access_key`、`secret_key` |
| 百度 | `baidu` | `appid`、`api_key` |
| OpenAI 兼容 | `openai` | `base_url`、`model`，可选 `api_key`（Ollama、vLLM 等） |
| 自定义 | `custom` | 任意 HTTP 模板（`config-sample` 有完整示例） |

### 数据来源

- **选区**：自建 Wayland 连接（独立于 GTK）监听主选区，`ext-data-control-v1`
- **指针**：evdev（`/dev/input`）追踪，有合成器原生接口时用精确坐标校正
  （KWin 脚本 / Hyprland IPC / GNOME Shell eval）
- **弹窗**：GTK4 layer-shell 覆盖层，永不抢占键盘焦点
- **托盘**：StatusNotifierItem（D-Bus）

## 许可证

MIT，见 `LICENSE`。
