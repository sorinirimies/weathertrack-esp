# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]
### Bug Fixes
- fix(gui): embed fonts for Slint software renderer
- fix: restore WiFi credentials, update Gitea remote URL
- fix: ESP32 TUI compiles and boots on hardware
- fix: exclude ESP32 crate build artifacts from git
### CI/CD
- ci: cancel in-progress runs on new push
### Documentation
- docs: remove real WiFi credentials from README
### Features
- feat(gui): implement full Slint platform adapter for ESP32-C6
- feat: redesign TUI and GUI for bigger fonts, less content per page
- feat: upgrade justfile and nushell scripts to match gitkraft patterns
- feat: Slint GUI crate with weather UI and animations
- feat: integrate tui-spinner and tui-piechart widgets
- feat: add weather icons with Unicode symbols
- feat: splash screen, NVS storage, and IP geolocation
- feat: complete gitkraft-style infrastructure
- feat: initial esp32-c6-weathertrack project
### Miscellaneous
- chore: complete project setup — justfile, workflows, cliff.toml, README
### Refactoring
- refactor: Elm architecture + module breakdown for TUI and GUI
