# esp32-c6-weathertrack — AI Agent Rules

## Architecture

This is a Cargo workspace with three crates:

| Crate | Role |
|-------|------|
| `weathertrack-core` | Shared weather logic (types, OpenMeteo parsing, WMO codes). No UI or platform deps. |
| `weathertrack-tui` | ESP32-C6 Ratatui + mousefood TUI on the Waveshare LCD. Excluded from workspace. |
| `weathertrack-gui` | ESP32-C6 Slint GUI scaffold on the Waveshare LCD. Excluded from workspace. |

## Rules

1. **All shared weather logic belongs in `weathertrack-core`** — types, API URL building, JSON parsing, WMO code mappings.
2. **Core must NOT depend on any UI framework or ESP32 crate** — no ratatui, slint, mousefood, embedded-graphics, or esp-idf crates.
3. **TUI and GUI crates are ESP32-only** — they are excluded from the workspace and build with `cd crates/weathertrack-{tui,gui} && cargo build`.
4. **Always run `cargo fmt --all`** and **`cargo clippy -p weathertrack-core -- -D warnings -A deprecated`** before committing.
5. **Always run `cargo test -p weathertrack-core`** after changes to core.
6. Use `anyhow::Result` for error handling throughout.
7. **Use `just` for common tasks** — run `just` to see available commands.
