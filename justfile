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
        echo "❌ git-cliff not found. Install with: cargo install git-cliff"; exit 1; \
    }

# Check nu (nushell) is available
_check-nu:
    @command -v nu >/dev/null 2>&1 || { \
        echo "❌ nu (nushell) not found. Install: https://www.nushell.sh"; exit 1; \
    }

_check-cargo-outdated:
    @command -v cargo-outdated >/dev/null 2>&1 || { \
        echo "❌ cargo-outdated not found. Install with: cargo install cargo-outdated"; exit 1; \
    }

# Install all recommended development tools
install-tools:
    @echo "Installing development tools…"
    @command -v git-cliff >/dev/null 2>&1 || cargo install git-cliff --locked
    @command -v nu >/dev/null 2>&1 || cargo install nu --locked
    @command -v cargo-outdated >/dev/null 2>&1 || cargo install cargo-outdated --locked
    @echo "✅ All tools installed!"

# ── Build ─────────────────────────────────────────────────────────────────────

# Generate sdkconfig.defaults with absolute partition table paths (machine-specific)
gen-sdkconfig:
    #!/usr/bin/env sh
    for crate in weathertrack-tui weathertrack-gui; do
        dir="crates/$crate"
        csv="$dir/partitions.csv"
        out="$dir/sdkconfig.defaults"
        abs=$(readlink -f "$csv")
        printf '%s\n' \
            'CONFIG_ESP_MAIN_TASK_STACK_SIZE=65536' \
            'CONFIG_ESP_WIFI_ENABLED=y' \
            'CONFIG_MBEDTLS_CERTIFICATE_BUNDLE=y' \
            'CONFIG_MBEDTLS_CERTIFICATE_BUNDLE_DEFAULT_FULL=y' \
            'CONFIG_ESP_HTTP_CLIENT_ENABLE_HTTPS=y' \
            'CONFIG_PARTITION_TABLE_CUSTOM=y' \
            "CONFIG_PARTITION_TABLE_CUSTOM_FILENAME=\"$abs\"" \
            'CONFIG_ESPTOOLPY_FLASHSIZE_8MB=y' \
            'CONFIG_ESPTOOLPY_FLASHSIZE="8MB"' \
            > "$out"
        echo "Generated $out (partition: $abs)"
    done

# Build the core library (host target)
build-core:
    cargo build -p weathertrack-core

# Build the TUI crate (ESP32 target)
build-tui: gen-sdkconfig
    cd crates/weathertrack-tui && cargo build

# Build the GUI crate (ESP32 target)
build-gui: gen-sdkconfig
    cd crates/weathertrack-gui && cargo build

# Build release for TUI
build-tui-release: gen-sdkconfig
    cd crates/weathertrack-tui && cargo build --release

# Build release for GUI
build-gui-release: gen-sdkconfig
    cd crates/weathertrack-gui && cargo build --release

# Remove build artifacts
clean:
    cargo clean

# ── Flash & Run ───────────────────────────────────────────────────────────────

# Flash and monitor the TUI on ESP32
run-tui: gen-sdkconfig
    cd crates/weathertrack-tui && cargo run

# Flash and monitor the GUI on ESP32
run-gui: gen-sdkconfig
    cd crates/weathertrack-gui && cargo run

# Flash release TUI
run-tui-release: gen-sdkconfig
    cd crates/weathertrack-tui && cargo run --release

# Flash release GUI
run-gui-release: gen-sdkconfig
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
    @echo "✅ All Rust and Nu tests passed!"

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

# Run all quality checks (format, clippy, test, nu) — must pass before a release.

# Auto-formats first, then verifies no changes remain (catches unstaged format diffs).
check-all: fmt clippy test test-nu
    @echo "🔍 Verifying formatting is clean…"
    cargo fmt --all -- --check
    @echo "✅ All checks passed!"

# Full pre-release quality gate — everything in check-all plus release builds.
check-release: check-all build-tui-release build-gui-release
    @echo "✅ Release quality gate passed (fmt + clippy + test + nu + release builds)!"

# ── Changelog ─────────────────────────────────────────────────────────────────

# Regenerate the full CHANGELOG.md from all tags
changelog: _check-git-cliff
    @echo "Generating full changelog…"
    git-cliff --output CHANGELOG.md
    @echo "✅ CHANGELOG.md updated."

# Prepend only unreleased commits to CHANGELOG.md
changelog-unreleased: _check-git-cliff
    git-cliff --unreleased --prepend CHANGELOG.md
    @echo "✅ Unreleased changes prepended."

# Preview changelog for the next release without writing the file
changelog-preview: _check-git-cliff
    @git-cliff --unreleased

# ── Version bump ──────────────────────────────────────────────────────────────

# Show the current workspace version
version: _check-nu
    @nu scripts/version.nu

# Validate that a version string will produce a valid vX.Y.Z tag.
validate-tag version: _check-nu
    @nu scripts/ci/validate_tag.nu "v{{ version }}" 2>&1 >/dev/null

# Fail fast if the requested version is the same as the current one.
_check-version-changed version: _check-nu
    #!/usr/bin/env sh
    current=$(nu scripts/version.nu)
    if [ "$current" = "{{ version }}" ]; then
        echo "❌ Version {{ version }} is already the current version. Nothing to bump."
        exit 1
    fi
    echo "✅ Version will change: $current → {{ version }}"

# Bump the workspace version, regenerate Cargo.lock + CHANGELOG.md, commit and tag.

# Validation runs first (cheap), quality gate runs second (expensive).
bump version: (validate-tag version) (_check-version-changed version) check-release _check-git-cliff
    nu scripts/bump_version.nu {{ version }}

# ── Housekeeping ──────────────────────────────────────────────────────────────

# Update all dependencies (Cargo.lock only)
update:
    cargo update

# Update dependencies, run the full quality gate, then commit and push if all green.
update-deps:
    @echo "⬆️  Updating dependencies…"
    cargo update
    @echo "🔍 Running quality gate…"
    cargo fmt --all -- --check
    cargo clippy -p weathertrack-core --all-targets --all-features -- -D warnings -A deprecated
    cargo test -p weathertrack-core --all-features --all-targets
    @echo "✅ All checks passed — committing dependency updates…"
    git add Cargo.lock
    git diff --cached --quiet || git commit -m "chore: update dependencies"
    just push-all
    @echo "✅ Dependency updates pushed to all remotes."

# Upgrade all dependencies (nu script)
upgrade-deps: _check-nu
    nu scripts/upgrade_deps.nu

# Show outdated dependencies (requires cargo-outdated)
outdated: _check-cargo-outdated
    cargo outdated

# Show all configured remotes
remotes:
    @git remote -v

# ── Git remotes & pushing ─────────────────────────────────────────────────────

# Push the current branch to origin (Gitea 192.168.1.204)
push:
    git push origin main

# Push the current branch to GitHub
push-github:
    git push github main

# Push the current branch to Starscream (Gitea 192.168.1.44)
push-starscream:
    git push starscream main

# Push the current branch to all remotes (continues on failure)
push-all:
    #!/usr/bin/env sh
    failed=""
    git push origin main      || failed="$failed origin"
    git push starscream main  || failed="$failed starscream"
    git push github main      || failed="$failed github"
    if [ -n "$failed" ]; then
        echo "⚠️  Failed to push to:$failed"
    else
        echo "✅ Pushed to origin, Starscream, and GitHub!"
    fi

# Force-push the current branch to all remotes
push-all-force:
    #!/usr/bin/env sh
    failed=""
    git push --force-with-lease origin main      || failed="$failed origin"
    git push --force-with-lease starscream main   || failed="$failed starscream"
    git push --force-with-lease github main       || failed="$failed github"
    if [ -n "$failed" ]; then
        echo "⚠️  Failed to force-push to:$failed"
    else
        echo "✅ Force-pushed to origin, Starscream, and GitHub!"
    fi

# Pull the current branch from origin
pull:
    git pull origin main

# Pull the current branch from all remotes (continues on failure)
pull-all:
    #!/usr/bin/env sh
    failed=""
    git pull origin main      || failed="$failed origin"
    git pull starscream main  || failed="$failed starscream"
    git pull github main      || failed="$failed github"
    if [ -n "$failed" ]; then
        echo "⚠️  Failed to pull from:$failed"
    else
        echo "✅ Pulled from origin, Starscream, and GitHub!"
    fi

# Push all tags to origin
push-tags:
    git push origin --tags

# Push all tags to all remotes (continues on failure)
push-tags-all:
    #!/usr/bin/env sh
    failed=""
    git push origin --tags      || failed="$failed origin"
    git push starscream --tags  || failed="$failed starscream"
    git push github --tags      || failed="$failed github"
    if [ -n "$failed" ]; then
        echo "⚠️  Failed to push tags to:$failed"
    else
        echo "✅ Tags pushed to all remotes!"
    fi

# ── Release workflows ─────────────────────────────────────────────────────────

# Bump, commit, tag, then push to origin — primary release target.
release version: (bump version)
    @echo "Pushing release v{{ version }} to origin…"
    git push --follow-tags origin main
    @echo "✅ Release v{{ version }} pushed to origin."

# Bump, commit, tag, then push to all remotes (continues on failure).
release-all version: (bump version)
    #!/usr/bin/env sh
    echo "Pushing release v{{ version }} to all remotes…"
    failed=""
    git push --follow-tags origin main      || failed="$failed origin"
    git push --follow-tags starscream main  || failed="$failed starscream"
    git push --follow-tags github main      || failed="$failed github"
    if [ -n "$failed" ]; then
        echo "⚠️  Release v{{ version }} failed to push to:$failed"
    else
        echo "✅ Release v{{ version }} pushed to origin, Starscream, and GitHub!"
    fi

# Preview what a release would look like
release-preview: _check-git-cliff
    @echo "Current version: $(just version)"
    @echo ""
    @echo "Unreleased commits:"
    @git-cliff --unreleased
    @echo ""
    @echo "Workspace version:"
    @grep -A5 '^\[workspace\.package\]' Cargo.toml | grep '^version'

# Push the latest commit and all tags to every remote (no bump, continues on failure).
push-release-all: check-all
    #!/usr/bin/env sh
    failed=""
    git push --follow-tags origin main      || failed="$failed origin"
    git push --follow-tags starscream main  || failed="$failed starscream"
    git push --follow-tags github main      || failed="$failed github"
    if [ -n "$failed" ]; then
        echo "⚠️  Failed to push to:$failed"
    else
        echo "✅ Latest commit + tags pushed to all remotes."
    fi

# Force-sync all remotes with origin (continues on failure)
sync-all:
    #!/usr/bin/env sh
    failed=""
    git push starscream main --force       || failed="$failed starscream"
    git push starscream --tags --force     || failed="$failed starscream-tags"
    git push github main --force           || failed="$failed github"
    git push github --tags --force         || failed="$failed github-tags"
    if [ -n "$failed" ]; then
        echo "⚠️  Failed to sync:$failed"
    else
        echo "✅ All remotes force-synced with origin."
    fi
