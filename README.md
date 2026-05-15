# esp32-c6-weathertrack

A multi-frontend weather station for the [Waveshare ESP32-C6-LCD-1.47](https://www.waveshare.com/wiki/ESP32-C6-LCD-1.47) board, powered by [Open-Meteo](https://open-meteo.com/). Displays current conditions and a 7-day forecast on a tiny 1.47" LCD — no cloud account or API key required.

## Features

- **ESP32-C6 + ST7789 1.47" LCD** weather station
- **Two UI options**: TUI ([Ratatui](https://ratatui.rs)) and GUI ([Slint](https://slint.dev))
- **OpenMeteo weather API** — no API key needed
- **IP-based geolocation** — auto-detects your location
- **NVS persistent configuration** — survives reboots
- **7-day forecast** with auto-refresh every 5 minutes
- **RGB LED status indicator** (WS2812 on GPIO8)
- **BOOT button page navigation** (GPIO9) — cycle through forecast days

## Architecture

The project follows the **Elm architecture** pattern: `Model → Msg → update → view`.

```
weathertrack-tui ──┐
                   ├──▶ weathertrack-core ──▶ Open-Meteo API
weathertrack-gui ──┘
```

The workspace is split into three crates:

| Crate | Purpose |
|-------|---------|
| `weathertrack-core` | Shared logic — types, API URL building, JSON parsing, WMO weather codes, alerts. Platform-agnostic, testable on any host. |
| `weathertrack-tui` | Ratatui-based terminal UI rendered on the ST7789 LCD via embedded-graphics. |
| `weathertrack-gui` | Slint-based declarative GUI rendered on the ST7789 LCD. |

The core crate has no ESP32 dependencies. The TUI and GUI crates are ESP32-only, excluded from the workspace default members, and build separately.

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

2. Edit WiFi credentials in the TUI or GUI main source:

   ```rust
   const WIFI_SSID: &str = "YOUR_WIFI_SSID";
   const WIFI_PASSWORD: &str = "YOUR_WIFI_PASSWORD";
   ```

3. Build and flash (release mode recommended for performance):

   ```sh
   just run-tui-release    # Ratatui frontend
   just run-gui-release    # Slint frontend
   ```

## Development

List all available commands:

```sh
just --list
```

Common workflows:

| Command | Description |
|---------|-------------|
| `just check-all` | Full quality gate — format, clippy, tests, nu tests |
| `just test` | Run `weathertrack-core` unit tests |
| `just fmt` | Format all code |
| `just clippy` | Lint the core crate |
| `just test-nu` | Run nushell script tests |
| `just changelog` | Generate changelog with git-cliff |

## Project Structure

```
esp32-c6-weathertrack/
├── Cargo.toml                  # Workspace root
├── cliff.toml                  # git-cliff changelog config
├── justfile                    # Task runner recipes
├── crates/
│   ├── weathertrack-core/      # Shared library (no ESP32 deps)
│   │   └── src/
│   │       ├── lib.rs          # Crate root & re-exports
│   │       ├── forecaster.rs   # Weather data fetching & parsing
│   │       ├── location.rs     # IP-based geolocation
│   │       ├── types.rs        # Data models (Model, Forecast, etc.)
│   │       └── wmo.rs          # WMO weather code descriptions
│   ├── weathertrack-tui/       # Ratatui frontend (ESP32)
│   │   └── src/
│   │       ├── main.rs         # Entry point & WiFi setup
│   │       ├── app.rs          # Elm-style app loop
│   │       ├── hal.rs          # Hardware abstraction (SPI, LED)
│   │       ├── net.rs          # HTTP client
│   │       └── ui/             # UI pages
│   │           ├── mod.rs
│   │           ├── hero.rs     # Current weather page
│   │           ├── forecast.rs # Daily forecast pages
│   │           ├── details.rs  # Detail views
│   │           ├── splash.rs   # Splash screen
│   │           └── style.rs    # Colors & theming
│   └── weathertrack-gui/       # Slint frontend (ESP32)
│       └── src/
│           ├── main.rs         # Entry point
│           ├── app.rs          # App loop
│           ├── bridge.rs       # Slint ↔ Rust bridge
│           ├── hal.rs          # Hardware abstraction
│           ├── net.rs          # HTTP client
│           └── platform.rs     # Platform init
└── scripts/
    ├── tests/
    │   ├── run_all.nu          # Test runner entry point
    │   ├── runner.nu           # Test framework
    │   └── test_version.nu     # Version consistency tests
    ├── ci/
    │   ├── quality_gate.nu     # CI quality checks
    │   ├── release_notes.nu    # Release note generation
    │   └── validate_tag.nu     # Tag validation
    ├── bump_version.nu         # Version bumping
    ├── upgrade_deps.nu         # Dependency upgrades
    └── version.nu              # Version helpers
```

## License

MIT
