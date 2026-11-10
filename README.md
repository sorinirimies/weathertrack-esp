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
- **Two UI styles per board**, same data and look: a graphical Slint UI and a terminal (ratatui) UI, on the C6's 320×172 LCD and the P4's 1024×600 touch panel
- **ESP32-P4 7" touch app** (`weathertrack-p4`): five touch tabs — today, 24-hour temperature chart, 7-day forecast, details (sun arc, wind, UV, pressure) and alerts; WiFi through the board's ESP32-C6 coprocessor

## Screenshots

The **P4 screenshots are taken from the real screen** (a firmware built with the `shots` feature streams the frame buffer: `just flash-p4-shots`, then `just screenshots-p4 <ip> slint`; `flash-p4-tui-shots` / `tui` for the terminal UI). The C6 and theme images are rendered on the host from the same code and fonts the firmware uses (`just screenshots`).

| | Graphical (Slint) | Terminal (ratatui) |
|---|---|---|
| **ESP32-P4, 7" touch** (1024×600) | ![P4 Slint](docs/screenshots/p4-slint-today.png) | ![P4 TUI](docs/screenshots/p4-tui-today.png) |
| **ESP32-C6, 1.47" LCD** (320×172, shown 3×) | ![C6 Slint](docs/screenshots/c6-slint-today.png) | ![C6 TUI](docs/screenshots/c6-tui-today.png) |

Hourly chart in the Slint UI:

![P4 hourly](docs/screenshots/p4-slint-hourly.png)

The terminal UI draws its charts with [tui-chart](https://github.com/sorinirimies/tui-chart): a smoothed braille line chart with a rain sparkline, a dumbbell chart for the 7-day low/high, and gauge widgets (dial, ring, arc, segments) for wind, humidity, pressure and UV.

| Hourly | 7 days | Details |
|---|---|---|
| ![P4 TUI hourly](docs/screenshots/p4-tui-hourly.png) | ![P4 TUI week](docs/screenshots/p4-tui-week.png) | ![P4 TUI details](docs/screenshots/p4-tui-details.png) |

### Made to be read from a distance (C6)

The C6's 320×172 screen is small, so its UIs trade density for size: **nothing under 12 px, one idea per page, headline numbers 40–84 px**. The Slint UI has nine pages (now, next hours, 24-hour chart, week, wind + humidity, UV + pressure, sun + rain, your place, alerts), the terminal UI eight (the next hours sit on the *now* page); the BOOT button flips through them. The terminal UI uses a 9×15 font (35×11 cells) with block-letter digits.

| Slint · now | Slint · wind + humidity | Terminal · now | Terminal · wind + humidity |
|---|---|---|---|
| ![](docs/screenshots/c6-slint-today.png) | ![](docs/screenshots/c6-slint-wind.png) | ![](docs/screenshots/c6-tui-today.png) | ![](docs/screenshots/c6-tui-wind.png) |

Terminal UI, 7 days on the C6:

![C6 TUI week](docs/screenshots/c6-tui-week.png)

### The device's website

Every board also serves a small website on its WiFi address (shown on the Place page, e.g. `http://192.168.3.210/`): the same weather as the screens, readable on a phone or a laptop, refreshing itself every minute.

| Desktop (live from the C6) | Phone |
|---|---|
| ![dashboard](docs/screenshots/web-dashboard.png) | ![phone](docs/screenshots/web-phone.png) |

- `/` the weather: now, details, alerts, a 24-hour chart (inline SVG) and the next days
- `/place` change the place (see below)
- `/api/weather.json` the same data as JSON, for scripts and other apps

It is server-rendered HTML from plain functions in `weathertrack-view` (no JavaScript, no framework, about 14 KB per page), so it is unit-tested and fuzzed on the host and light enough for the ESP32-C6. A Topcoat app would not fit there (it needs tokio and a full HTTP stack), so the PC-side [web app](#the-web-app-topcoat) is where Topcoat lives. `just preview-web` writes the pages with sample data to `target/site/`. The pages have no login: anyone on your WiFi can open them and change the place.

### Choosing the place

By default every board looks up where it is from the network on boot (IP geolocation: city level, and an ISP's registered city can be tens of kilometres off). To pick the place yourself. On the P4 you can type the city right on the screen (Place tab: keyboard, results, **Use network**); on any board, from a phone or computer:

1. Open the **Place** page on the device (or the **Place** link on the website). It shows the place in use, where it came from, and an address like `http://192.168.3.210/`.
2. Open that address on a phone or computer on the same WiFi, type a city and tap the right result (Open-Meteo geocoding, no key).
3. The device switches at once and remembers the choice across reboots. **Use my network location** goes back to automatic.

#### My places (a list of favourites)

Keep up to 8 places and switch between them. On any board's website: **Save this place to my list**, tap a name to show it, **x** to remove it. On the P4 screen (Slint or terminal UI) the Place tab is the list (tap a place to show it, **x** to remove, **Save this place**, **Search city** opens the keyboard). The list is stored in flash, so it survives reboots, and removing a place removes it from flash too. In the web app the same list is on the Place page.

With **Keep my devices on this place** on, the web app and the boards keep the *same list*: a place saved or removed on one side reaches the others (the last change wins), removals included.

The P4's terminal UI has the same list and keyboard:

| P4 · my places (terminal) | P4 · search (terminal) |
|---|---|
| ![P4 TUI places](docs/screenshots/p4-tui-place-list.png) | ![P4 TUI search](docs/screenshots/p4-tui-place-search.png) |

Order of precedence: a place pinned in `config.toml` (`[location] fixed = true`) beats your choice, which beats the network lookup, which beats the last known place. The page logic lives in `weathertrack-core` (`place`, `geocode`, `web`: pure, tested and fuzzed); each firmware only serves it.

| P4 · Place, my places (Slint) | P4 · Place, search (Slint) | C6 · Place (Slint) | C6 · Place (terminal) |
|---|---|---|---|
| ![P4 places](docs/screenshots/p4-slint-place-list.png) | ![P4 search](docs/screenshots/p4-slint-place-search.png) | ![C6 place](docs/screenshots/c6-slint-place.png) | ![C6 TUI place](docs/screenshots/c6-tui-place.png) |

### The web app (Topcoat)

`weathertrack-web` is a companion for your PC, built with [Topcoat](https://github.com/tokio-rs/topcoat) (server-rendered Rust). It shows the same dashboard as the devices for any place, and it keeps **one place for all your boards**:

```
just web -d 192.168.3.210 -d 192.168.3.211
```

| Place | Devices |
|---|---|
| ![web app place](docs/screenshots/webapp-place.png) | ![web app devices](docs/screenshots/webapp-devices.png) |

- **Weather**: the full dashboard (it renders the very same `weathertrack-view` code as the devices, so they always look alike).
- **Place**: search a city, or go back to your network location.
- **Devices**: add boards by address, see what each shows and whether it matches. **Keep my devices on this place** turns the sync on.

How the sync works (every 30 s, and right after you choose): the web app reads each device's `/api/weather.json` and decides with a pure, tested planner (`weathertrack_core::sync`):

1. A city chosen on the web is sent to every reachable device (`POST /set`, or `/auto` for the network location).
2. A city chosen on a device's own page is adopted by the web app and sent to the other devices (the last change wins).
3. A device with a place pinned in `config.toml` is left alone; one that is off is skipped and caught up when it returns.

Settings (devices, chosen place, sync) are kept in the user's config directory (`weathertrack/web.json`). It listens on `127.0.0.1:3030` only by default and has no login, so keep it that way unless you trust your network. It needs Rust 1.98 (Topcoat), so it is its own workspace: the gate checks it and `cargo deny` / `cargo audit` cover it, but the 1.90 MSRV job does not build it.

### Themes

The terminal UI is themed with [tui-nuances](https://github.com/sorinirimies/tui-nuances): WeatherTrack (the default), Catppuccin, Tokyo Night, Nord, Dracula, Gruvbox, Solarized (dark and light), One Dark, Rosé Pine, Monokai, Kanagawa, Everforest, Night Owl, Synthwave, Pip-Boy, Porsche, High contrast, GitHub Light. Text, borders and chart colours are contrast-checked against each theme's background, so light themes stay readable.

- **C6:** a short BOOT press changes the page, **holding BOOT (0.8 s) changes the theme**.
- **P4:** tap **Theme: name >** under the Refresh button.
- The choice is kept in flash across reboots. `just preview-themes` renders every theme.

| P4 · Nord | C6 · Catppuccin Latte (shown 3×) |
|---|---|
| ![P4 Nord](docs/screenshots/p4-tui-nord.png) | ![C6 Latte](docs/screenshots/c6-tui-latte.png) |

## Architecture

The project follows the **Elm architecture** pattern: `Model → Msg → update → view`.

```
weathertrack-tui ─┐                          ┌─ weathertrack-tui-ui (ratatui)
weathertrack-gui ─┼─▶ weathertrack-view ◀────┤
weathertrack-p4  ─┘        │                 └─ Slint bridge (weathertrack-view/slint)
                           ▼
                   weathertrack-core ──▶ Open-Meteo API
```

The workspace is split into these crates:

| Crate | Purpose |
|-------|---------|
| `weathertrack-core` | Shared logic — types, API URL building, JSON parsing, WMO weather codes, alerts. Platform-agnostic, testable on any host. |
| `weathertrack-tui` | Ratatui-based terminal UI rendered on the ST7789 LCD via embedded-graphics. |
| `weathertrack-view` | Presentation layer shared by every UI: formatted values, hourly window, chart geometry, sample data. Host-tested. |
| `weathertrack-tui-ui` | The ratatui UI (charts from `tui-chart`, themes from `tui-nuances`), responsive: compact (≈53×17 cells, C6) and wide (≈102×30 cells with a touch tab bar, P4). |
| `weathertrack-gui` | Slint UI for the C6's 320×172 LCD (BOOT button changes page). |
| `weathertrack-p4` | 7" ESP32-P4 firmware (MIPI-DSI panel, GT911 touch, WiFi via the C6 coprocessor) with two UIs chosen by cargo feature: `slint-ui` (default) or `tui`. Its own crate because the P4 needs newer `esp-idf-*` crates. |
| `weathertrack-p4-preview`, `-gui-preview`, `-tui-preview` | Host-side renderers: draw every page of a UI to PNG with sample data, so the UI can be checked without a board (`just preview-p4`, `preview-gui`, `preview-tui`). |
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

2. Copy `config.toml.example` to `config.toml` (git-ignored) and set your WiFi. It is baked into the firmware at build time. **The weather follows where the device is**: on every boot its location is looked up from the network (IP geolocation, city level). The `[location]` place in `config.toml` is only the fallback if that lookup fails; set `fixed = true` there to pin it instead.

3. Build and flash (release mode recommended for performance):

   ```sh
   just run-tui-release    # Ratatui frontend, ESP32-C6
   just run-gui-release    # Slint frontend, ESP32-C6
   just flash-p4           # 7" touch app, ESP32-P4 (builds, flashes, opens the monitor)
   just build-p4-tui-release  # the same P4 firmware with the terminal UI
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
│   ├── weathertrack-view/      # Presentation layer: formatted values, chart geometry, sample data
│   │   └── slint/              # Slint data types + bridge shared by the two Slint UIs
│   ├── weathertrack-tui-ui/    # Responsive ratatui UI (compact C6 / wide P4)
│   ├── weathertrack-web/       # PC web app (Topcoat): dashboard, city search, one place for all devices
│   ├── weathertrack-hal/       # ESP32-C6 hardware helpers
│   ├── weathertrack-tui/       # C6 firmware: ratatui UI on the 172x320 LCD
│   ├── weathertrack-gui/       # C6 firmware: Slint UI on the 172x320 LCD
│   ├── weathertrack-p4/        # P4 firmware: Slint (default) or ratatui UI on 1024x600 MIPI-DSI
│   │   ├── ui/                 # app.slint, pages/, components/, theme.slint
│   │   ├── src/                # main, hw (frame buffer, touch), ui_slint, ui_tui, wifi, net
│   │   └── components/wt_display/  # C shim: EK79007 panel, backlight, GT911
│   └── weathertrack-*-preview/ # Host renderers: every page of each UI to PNG
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
