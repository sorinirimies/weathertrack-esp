# esp32-c6-weathertrack workspace — task runner
# Install just:      cargo install just
# Install git-cliff: cargo install git-cliff
# Usage: just <task>
# ── Default ───────────────────────────────────────────────────────────────────

default:
    @just --list

# ── Tool checks ───────────────────────────────────────────────────────────────

_check-git-cliff:
    @command -v git-cliff >/dev/null 2>&1 || { \
        echo "git-cliff not found. Install with: just install-git-cliff"; exit 1; \
    }

_check-nu:
    @command -v nu >/dev/null 2>&1 || { \
        echo "nu (nushell) not found. Install with: just install-nu"; exit 1; \
    }

_check-cargo-outdated:
    @command -v cargo-outdated >/dev/null 2>&1 || { \
        echo "cargo-outdated not found. Install with: just install-cargo-outdated"; exit 1; \
    }

# ── Install tools ────────────────────────────────────────────────────────────

# Install git-cliff (changelog generator)
install-git-cliff:
    cargo install git-cliff --locked

# Install nu (nushell scripting)
install-nu:
    cargo install nu --locked

# Install cargo-outdated (dependency checker)
install-cargo-outdated:
    cargo install cargo-outdated --locked

# Install all recommended development tools
install-tools:
    @echo "Installing development tools..."
    @command -v git-cliff >/dev/null 2>&1 || just install-git-cliff
    @command -v nu >/dev/null 2>&1 || just install-nu
    @command -v cargo-outdated >/dev/null 2>&1 || just install-cargo-outdated
    @echo "All tools installed!"

# ── Build ─────────────────────────────────────────────────────────────────────

# Build the core library (host target)
build-core:
    cargo build -p weathertrack-core

# Build the TUI crate (ESP32 target)
build-tui:
    cd crates/weathertrack-tui && cargo build

# Build the GUI crate (ESP32 target)
build-gui:
    cd crates/weathertrack-gui && cargo build

# Build release for TUI
build-tui-release:
    cd crates/weathertrack-tui && cargo build --release

# Build release for GUI
build-gui-release:
    cd crates/weathertrack-gui && cargo build --release

# Remove build artifacts
clean:
    cargo clean

# ── Flash & Run ───────────────────────────────────────────────────────────────

# Flash and monitor the TUI on ESP32
run-tui:
    cd crates/weathertrack-tui && cargo run

# Flash and monitor the GUI on ESP32
run-gui:
    cd crates/weathertrack-gui && cargo run

# Flash release TUI
run-tui-release:
    cd crates/weathertrack-tui && cargo run --release

# Flash release GUI
run-gui-release:
    cd crates/weathertrack-gui && cargo run --release

# ── Test ──────────────────────────────────────────────────────────────────────

# Run the core library test suite
test:
    cargo test -p weathertrack-core --all-features --all-targets

# Run Nu script tests
test-nu: _check-nu
    nu scripts/tests/run_all.nu

# Run both Rust and Nu tests
test-all: test test-nu
    @echo "All Rust and Nu tests passed!"

# ── Code quality ──────────────────────────────────────────────────────────────

# Check without building
check:
    cargo check -p weathertrack-core

# Format all code
fmt:
    cargo fmt --all

# Check formatting without modifying files
fmt-check:
    cargo fmt --all -- --check

# Run clippy on the core crate
clippy:
    cargo clippy -p weathertrack-core --all-targets --all-features -- -D warnings -A deprecated

# Build and open crate documentation
doc:
    cargo doc -p weathertrack-core --no-deps --open

# Run all quality checks (format, clippy, test, nu)
check-all: fmt clippy test test-nu
    @echo "Verifying formatting is clean..."
    cargo fmt --all -- --check
    @echo "All checks passed!"

# Full release quality gate: all checks + release builds
check-release: check-all build-tui-release build-gui-release
    @echo "Release quality gate passed!"

# ── Changelog ─────────────────────────────────────────────────────────────────

# Generate changelog
changelog: _check-git-cliff
    git-cliff -o CHANGELOG.md

# Write only unreleased changes to CHANGELOG.md
changelog-unreleased: _check-git-cliff
    git-cliff --unreleased -o CHANGELOG.md

# Preview unreleased changes without writing
changelog-preview: _check-git-cliff
    git-cliff --unreleased

# ── Version ───────────────────────────────────────────────────────────────────

# Print current workspace version
version: _check-nu
    @nu scripts/version.nu

# Validate a version tag against CI rules
validate-tag version: _check-nu
    nu scripts/ci/validate_tag.nu v{{ version }}

# Check that the version is actually changing
_check-version-changed version: _check-nu
    #!/usr/bin/env sh
    current=$(nu scripts/version.nu)
    if [ "$current" = "{{ version }}" ]; then
        echo "Version {{ version }} is already current. Nothing to bump."
        exit 1
    fi
    echo "Version will change: $current -> {{ version }}"

# Bump version across all manifests (cheap validation first, expensive quality gate second)
bump version: (validate-tag version) (_check-version-changed version) check-release _check-git-cliff
    nu scripts/bump_version.nu {{ version }}

# ── Dependencies ──────────────────────────────────────────────────────────────

# Upgrade all dependencies
upgrade-deps: _check-nu
    nu scripts/upgrade_deps.nu

# Update the lockfile only
update:
    cargo update

# Update dependencies, run quality gate, commit and push
update-deps: _check-nu
    @echo "Updating dependencies..."
    cargo update
    @echo "Running quality gate..."
    just check-all
    @echo "Committing..."
    git add Cargo.lock
    git diff --cached --quiet || git commit -m "chore: update dependencies"
    just push-all

# Show outdated dependencies
outdated: _check-cargo-outdated
    cargo outdated

# ── Git remotes ───────────────────────────────────────────────────────────────

# Push to all remotes (continues on failure)
push-all:
    #!/usr/bin/env sh
    failed=""
    git push origin main     || failed="$failed origin"
    git push starscream main || failed="$failed starscream"
    git push github main     || failed="$failed github"
    if [ -n "$failed" ]; then
        echo "Failed to push to:$failed"
        exit 1
    else
        echo "Pushed to all remotes"
    fi

# Force-push to all remotes (continues on failure)
push-all-force:
    #!/usr/bin/env sh
    failed=""
    git push origin main --force-with-lease     || failed="$failed origin"
    git push starscream main --force-with-lease  || failed="$failed starscream"
    git push github main --force-with-lease      || failed="$failed github"
    if [ -n "$failed" ]; then
        echo "Failed to force-push to:$failed"
        exit 1
    else
        echo "Force-pushed to all remotes"
    fi

# Pull from origin
pull:
    git pull origin main

# Pull from all remotes
pull-all:
    git pull origin main

# Push tags to origin
push-tags:
    git push origin --tags

# Push tags to all remotes (continues on failure)
push-tags-all:
    #!/usr/bin/env sh
    failed=""
    git push origin --tags     || failed="$failed origin"
    git push starscream --tags || failed="$failed starscream"
    git push github --tags     || failed="$failed github"
    if [ -n "$failed" ]; then
        echo "Failed to push tags to:$failed"
        exit 1
    else
        echo "Pushed tags to all remotes"
    fi

# List all remotes
remotes:
    git remote -v

# ── Release ───────────────────────────────────────────────────────────────────

# Tag, bump, and push a release
release version: (bump version)
    just push-all
    just push-tags-all

# Preview what a release would look like
release-preview: _check-git-cliff
    @echo "Current version: $(just version)"
    @echo ""
    @echo "Unreleased commits:"
    @git-cliff --unreleased
