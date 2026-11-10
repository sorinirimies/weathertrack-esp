# esp32-c6-weathertrack

[![CI](https://github.com/sorinirimies/esp32-c6-weathertrack/actions/workflows/ci.yml/badge.svg)](https://github.com/sorinirimies/esp32-c6-weathertrack/actions/workflows/ci.yml)
[![Security](https://github.com/sorinirimies/esp32-c6-weathertrack/actions/workflows/security.yml/badge.svg)](https://github.com/sorinirimies/esp32-c6-weathertrack/actions/workflows/security.yml)
[![Fuzz](https://github.com/sorinirimies/esp32-c6-weathertrack/actions/workflows/fuzz.yml/badge.svg)](https://github.com/sorinirimies/esp32-c6-weathertrack/actions/workflows/fuzz.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A multi-frontend weather station for ESP32 boards, powered by [Open-Meteo](https://open-meteo.com/): the [Waveshare ESP32-C6-LCD-1.47](https://www.waveshare.com/wiki/ESP32-C6-LCD-1.47) (tiny 1.47" LCD) and the [Waveshare ESP32-P4-WIFI6-Touch-LCD-7B](https://docs.waveshare.com/ESP32-P4-WIFI6-Touch-LCD-7B) (7" 1024×600 touch screen). No cloud account or API key required.

## Features

- **ESP32-C6 + ST7789 1.47" LCD** weather station
- **Two UI options**: TUI ([Ratatui](https://ratatui.rs)) and GUI ([Slint](https://slint.dev))
- **OpenMeteo weather API** — no API key needed
- **IP-based geolocation** — auto-detects your location
- **NVS persistent configuration** — survives reboots
- **7-day forecast** with auto-refresh every 5 minutes
- **RGB LED status indicator** (WS2812 on GPIO8)
- **BOOT button page navigation** (GPIO9) — cycle through forecast days
- **ESP32-P4 7" touch app** (`weathertrack-p4`): five touch tabs — today, 24-hour temperature chart, 7-day forecast, details (sun arc, wind, UV, pressure) and alerts; WiFi through the board's ESP32-C6 coprocessor

## Architecture

The project follows the **Elm architecture** pattern: `Model → Msg → update → view`.

```
weathertrack-tui ──┐
weathertrack-gui ──┼──▶ weathertrack-core ──▶ Open-Meteo API
weathertrack-p4  ──┘
```

The workspace is split into these crates:

| Crate | Purpose |
|-------|---------|
| `weathertrack-core` | Shared logic — types, API URL building, JSON parsing, WMO weather codes, alerts. Platform-agnostic, testable on any host. |
| `weathertrack-tui` | Ratatui-based terminal UI rendered on the ST7789 LCD via embedded-graphics. |
| `weathertrack-gui` | Slint-based declarative GUI rendered on the ST7789 LCD. |
| `weathertrack-p4` | Slint touch UI for the 7" ESP32-P4 board (MIPI-DSI panel, GT911 touch, WiFi via the C6 coprocessor). Its own crate because the P4 needs newer `esp-idf-*` crates. |
| `weathertrack-p4-preview` | Host-side renderer: draws every P4 tab to PNG with sample data, so the UI can be checked without a board (`just preview-p4`). |
| `weathertrack-hal` | ESP32 hardware helpers shared by the C6 firmware. |

The core crate has no ESP32 dependencies and is the only workspace member. The firmware crates are excluded from the workspace (they need the ESP toolchain and their own `.cargo/config.toml`) and build separately.

## Hardware

- **Board**: [Waveshare ESP32-C6-LCD-1.47](https://www.waveshare.com/wiki/ESP32-C6-LCD-1.47)
- **MCU**: ESP32-C6 (RISC-V, `riscv32imac-esp-espidf`)
- **Display**: 1.47" ST7789 TFT, 172×320 pixels, SPI @ 40 MHz (landscape 320×172)
- **LED**: WS2812B RGB on GPIO8 (status indicator)
- **Button**: BOOT on GPIO9 (page navigation)
- **WiFi**: 802.11 b/g/n (built-in)

### Pin Mapping

| Function | GPIO | Notes |
|----------|------|-------|
| SPI SCLK | GPIO6 | Display clock |
| SPI MOSI | GPIO7 | Display data |
| LCD CS | GPIO14 | Chip select |
| LCD DC | GPIO15 | Data/command |
| LCD RST | GPIO21 | Reset |
| LCD BL | GPIO22 | Backlight |
| WS2812 LED | GPIO8 | RGB status LED |
| BOOT Button | GPIO9 | Page navigation |

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (nightly toolchain for ESP32 crates)
- ESP-IDF toolchain via [espup](https://github.com/esp-rs/espup)
- [espflash](https://github.com/esp-rs/espflash) for flashing
- [just](https://github.com/casey/just) task runner
- [nu](https://www.nushell.sh/) (for scripts and tests)
- [git-cliff](https://github.com/orhun/git-cliff) (for changelog generation)

### Build & Flash

1. Clone the repository:

   ```sh
   git clone https://github.com/sorin-irimies/esp32-c6-weathertrack.git
   cd esp32-c6-weathertrack
   ```

2. Copy `config.toml.example` to `config.toml` (git-ignored) and set your WiFi and location. It is baked into the firmware at build time.

3. Build and flash (release mode recommended for performance):

   ```sh
   just run-tui-release    # Ratatui frontend, ESP32-C6
   just run-gui-release    # Slint frontend, ESP32-C6
   just flash-p4           # 7" touch app, ESP32-P4 (builds, flashes, opens the monitor)
   ```

   Before the first P4 flash, `just backup-p4` saves the board's original firmware.
   Preview the P4 UI without hardware: `just preview-p4` writes PNGs to `target/p4-ui/`.

## Development

List all available commands:

```sh
just --list
```

Common workflows:

| Command | Description |
|---------|-------------|
| `just check-all` | The CI gate — fmt, clippy, tests, P4 UI tests, docs, version check, unused deps, nu tests |
| `just test` | Rust tests (core) |
| `just test-nu` | Nushell script tests |
| `just fuzz-all` | Short run of every fuzz target (nightly + cargo-fuzz) |
| `just audit` / `just deny` | Known vulnerabilities / licenses, bans, sources |
| `just secrets` | Scan the git history for secrets (gitleaks) |
| `just check-firmware` | Build every firmware image (slow, needs the ESP toolchain) |
| `just changelog` | Generate changelog with git-cliff |
| `just bump X.Y.Z` | Bump the version, update the changelog, commit and tag |

### CI

GitHub and Gitea both run the gate on every push. Nightly workflows update the lockfiles, run
`cargo audit` and `cargo deny`, scan the history with gitleaks and fuzz every target
(`fuzz/fuzz_targets`: forecast and location parsing, cache JSON, WMO/alert mapping). The
firmware itself is not built in CI (the ESP toolchain needs ~10 minutes and several GB); run
`just check-firmware` before a release. Commits follow [Conventional Commits](https://www.conventionalcommits.org);
`CHANGELOG.md` is generated by git-cliff.

## Project Structure

```
esp32-c6-weathertrack/
├── Cargo.toml                  # Workspace root (core only; firmware crates excluded)
├── cliff.toml                  # git-cliff changelog config
├── deny.toml, clippy.toml      # cargo-deny and clippy policy
├── justfile                    # Task runner recipes (Nushell)
├── config.toml.example         # WiFi + location (copy to config.toml)
├── crates/
│   ├── weathertrack-core/      # Shared library (no ESP32 deps): types, Open-Meteo parsing, WMO codes
│   ├── weathertrack-hal/       # ESP32-C6 hardware helpers
│   ├── weathertrack-tui/       # Ratatui frontend (ESP32-C6, 172x320 LCD)
│   ├── weathertrack-gui/       # Slint frontend (ESP32-C6, 172x320 LCD)
│   ├── weathertrack-p4/        # Slint touch app (ESP32-P4, 1024x600 MIPI-DSI)
│   │   ├── ui/                 # app.slint, pages/, components/, theme.slint
│   │   ├── src/                # main, platform (frame buffer + touch), bridge, wifi, net
│   │   └── components/wt_display/  # C shim: EK79007 panel, backlight, GT911
│   └── weathertrack-p4-preview/    # Host renderer: every P4 tab to PNG
├── fuzz/                       # cargo-fuzz targets for the core parsers
├── .github/, .gitea/           # CI, release, nightly security / fuzz / deps / secrets
└── scripts/
    ├── esp.nu                  # sdkconfig generation, P4 flash + backup
    ├── project.nu              # What a release ships (BINS, PUBLISH, …)
    ├── ci/                     # Quality gate, release, nightly helpers
    └── tests/                  # Nushell tests for all of the above
```

## License

MIT
