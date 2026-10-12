# Amlogic USB Burning Tool

A Linux GUI for flashing firmware to Amlogic-based devices (Android TV boxes, SBCs, etc.)  
Built with Rust + GTK4 / libadwaita.

![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)

---

## Features

- **Multi-device support** — flash multiple devices simultaneously
- **Auto device detection** — polls USB every 2 seconds, no manual refresh needed
- **Dual burn engine** — classic `aml_flash_tool` for legacy SoCs, `adnl` for newer ones (auto-selected)
- **Partition selection** — choose which partitions to burn via Advanced Settings → Partitions
- **Drag & drop** — drop a `.img` file onto the window to load it
- **Recent images** — quick access to last 10 used firmware files
- **Live log** — timestamped output with optional save-to-file
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

---

## Requirements

- Linux x86_64 (ARM build included but less tested)
- libusb 1.0
- GTK 4.12+
- libadwaita 1.5+

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
sudo apt install libgtk-4-dev libadwaita-1-dev libglib2.0-dev cargo

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
2. Launch the tool — the device will appear automatically within 2 seconds
3. Load a firmware image: **File → Import Image** or drag & drop a `.img` file
4. Select the correct SoC type in the right panel
5. Configure options (Erase Flash, Reset After Burn, etc.)
6. Click **Start**

### Keyboard shortcuts

| Key | Action |
|---|---|
| `Ctrl+O` | Open firmware image |
| `Ctrl+Enter` or `Ctrl+S` | Start burning |
| `Shift+Ctrl+Enter` or `Shift+Ctrl+S` | Stop burning |
| `F5` or `Ctrl+R` | Refresh devices |

---

## Troubleshooting

**Device not detected**
- Make sure the udev rule is installed and you reloaded rules
- Try `lsusb` — the device should show as `1b8e:c003`
- Some boards require holding the reset/update button before connecting USB

**"No partitions selected"**
- This means no image is loaded — use File → Import Image first

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
