# Amlogic USB Burning Tool

A Linux GUI for flashing firmware to Amlogic-based devices (Android TV boxes, SBCs, etc.)  
Built with Rust + GTK4 / libadwaita.

![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)

---

## Features

- **Multi-device support** — flash multiple devices simultaneously (classic engine; ADNL one at a time)
- **Auto device detection** — polls USB every 3 seconds (paused while burning), no manual refresh needed
- **Dual burn engine** — classic `aml_flash_tool` for legacy SoCs, `adnl` for newer ones (auto-selected)
- **Partition selection** — choose which partitions to burn in the Partitions row of the side panel
- **Drag & drop** — drop a `.img` file onto the window to load it (or open it with the app from your file manager)
- **Recent images** — quick access to last 10 used firmware files
- **Live log** — timestamped, color-coded output with copy and save-to-file
- **Persistent settings** — all options (SoC, erase flags, window size) saved across sessions
- **Safety confirmation** — warns before erasing bootloader

---

## Supported SoCs

| SoC String | Chips |
|---|---|
| `gxl` | S905X, S905W, S912 |
| `g12a` | S905D2, S905X2 |
| `g12b` | S922X, A311D |
| `sm1` | S905D3, S905X3 |
| `sc2` | S905X4 |
| `axg` | A113X |
| `a1` | A1 |
| `m8` | S805, A111 |
| `txlx` | T962E |
| `t5d` | T5D |
| `t7` | T7 |
| `s4` | S4 |
| `s5` | S5 |
| `t3` | T3 |

> **Note:** g12b and sm1 use the g12a protocol internally — this is handled automatically.

> **Engine limits:** The ADNL engine burns one device per run and does not implement
> **Erase Flash** or **Erase Bootloader**. The app aborts with a clear error if either
> option is selected in ADNL mode — uncheck it or use the classic engine.

---

## Requirements

- Linux x86_64 (source includes ARM tools; the released `.deb` is amd64-only; bundled ADNL binaries are x86_64-only)
- libusb 1.0 + libusb-compat (0.1)
- GTK 4.12+
- libadwaita 1.5+
- `pv` (optional, for `sdcard_burn_tool.sh`)

---

## Installation

### From `.deb` package (recommended)

```bash
sudo dpkg -i aml-flash-tool_*.deb
```

The package installs the binary, desktop entry, icon, udev rules, and all bundled tools automatically.

### udev rules (manual install)

If not using the `.deb`, install the udev rule so the tool can access USB devices without root:

```bash
sudo cp assets/99-amlogic-flash.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
```

Then add yourself to the `plugdev` group (log out and back in after):

```bash
sudo usermod -aG plugdev $USER
```

---

## Building from source

```bash
# Dependencies (Debian/Ubuntu)
sudo apt install libgtk-4-dev libadwaita-1-dev libglib2.0-dev libusb-0.1-4 cargo

# Build
cargo build --release

# Run directly
cargo run --release

# Build .deb
cargo install cargo-deb
cargo deb
```

---

## Usage

1. Connect your Amlogic device in **USB burning mode**  
   (hold the reset button while connecting USB, or use a toothpick on the AV port)
2. Launch the tool — the device will appear automatically within a few seconds
3. Load a firmware image: **Open Image** (or the ▾ next to it for recent files), or drag & drop a `.img` file
4. Check the SoC type (auto-detected from the image) and, optionally, which partitions to burn
5. Configure options (Erase Flash, Reset After Burn, etc.)
6. Click **Start**

### Keyboard shortcuts

| Key | Action |
|---|---|
| `Ctrl+O` | Open firmware image |
| `Ctrl+Enter` | Start burning |
| `Shift+Ctrl+Enter` | Stop burning |
| `Ctrl+L` | Clear log |
| `Ctrl+?` | Show shortcuts |
| `F5` or `Ctrl+R` | Refresh devices |

---

## Troubleshooting

**Device not detected**
- Make sure the udev rule is installed and you reloaded rules
- Try `lsusb` — the device should show as `1b8e:c003`
- Some boards require holding the reset/update button before connecting USB

**"No partitions selected"**
- This means no image is loaded — use **Open Image** first

**"Failed to start" error**
- Check that the bundled tools are executable: `ls -la /usr/lib/amlogicburn/tools/linux-x86/`
- If running from source, tools should be in `./tools/linux-x86/`

---

## Support

Enjoying this project? A small donation is greatly appreciated and helps keep it going:

<p align="center">
  <a href="https://www.paypal.me/antoxab1978" target="_blank">
    <img src="https://www.paypalobjects.com/en_US/i/btn/btn_donateCC_LG.gif" alt="Donate with PayPal" />
  </a>
</p>

## License

MIT © 2026 Anton
