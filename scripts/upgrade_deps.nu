#!/usr/bin/env nu
# Upgrade all workspace dependencies, run quality gate, commit.
# Usage: nu scripts/upgrade_deps.nu [--check]

def header [msg: string] { print $"\n(ansi cyan_bold)═══ ($msg) ═══(ansi reset)" }
def step [msg: string] { print $"  (ansi green_bold)▶(ansi reset) ($msg)" }
def ok [msg: string] { print $"  (ansi green)✔(ansi reset) ($msg)" }

def main [--check (-c)] {
    header "Dependency Upgrade"

    if (which cargo-upgrade | is-empty) {
        step "Installing cargo-edit..."
        cargo install cargo-edit --locked
    }

    if $check {
        step "Dry-run: checking for outdated deps"
        cargo upgrade --incompatible allow --dry-run
        return
    }

    step "Upgrading Cargo.toml versions"
    cargo upgrade --incompatible allow
    ok "Cargo.toml deps upgraded"

    step "Updating lockfile"
    cargo update
    ok "Cargo.lock updated"

    header "Quality Gate"
    nu scripts/ci/quality_gate.nu

    step "Committing"
    git add -A
    git commit -m "chore(deps): upgrade all dependencies"
    ok "Dependencies upgraded and committed"
}

main
