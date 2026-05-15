#!/usr/bin/env nu
# Bump version across all workspace manifests, run quality gate, commit & tag.
# Usage: nu scripts/bump_version.nu 0.2.0

def header [msg: string] { print $"\n(ansi cyan_bold)═══ ($msg) ═══(ansi reset)" }
def step [msg: string] { print $"  (ansi green_bold)▶(ansi reset) ($msg)" }
def ok [msg: string] { print $"  (ansi green)✔(ansi reset) ($msg)" }

export def validate_semver [ver: string] {
    if not ($ver =~ '^\d+\.\d+\.\d+$') {
        error make {msg: $"Invalid semver: '($ver)' — expected X.Y.Z"}
    }
}

def main [new_version: string] {
    header $"Version Bump"

    # Validate
    validate_semver $new_version
    let old_version = (open Cargo.toml | get workspace.package.version)

    if $old_version == $new_version {
        print $"(ansi yellow)Version is already ($new_version), nothing to do.(ansi reset)"
        return
    }

    step $"($old_version) -> ($new_version)"

    # Update workspace Cargo.toml
    step "Updating workspace Cargo.toml"
    let cargo_toml = (open Cargo.toml --raw)
    let updated = ($cargo_toml | str replace $'version = "($old_version)"' $'version = "($new_version)"' --all)
    $updated | save Cargo.toml --force
    ok "Cargo.toml"

    # Update ESP32 crate manifests
    for crate_dir in ["crates/weathertrack-tui", "crates/weathertrack-gui"] {
        let crate_toml_path = $"($crate_dir)/Cargo.toml"
        if ($crate_toml_path | path exists) {
            let content = (open $crate_toml_path --raw)
            let updated_content = ($content | str replace $'version = "($old_version)"' $'version = "($new_version)"' --all)
            $updated_content | save $crate_toml_path --force
            ok $crate_toml_path
        }
    }

    # Quality gate
    header "Quality Gate"
    nu scripts/ci/quality_gate.nu

    # Changelog
    step "Generating changelog"
    git-cliff -o CHANGELOG.md
    ok "CHANGELOG.md"

    # Commit and tag
    step "Committing"
    git add -A
    git commit -m $"chore\(release\): bump version to ($new_version)"
    git tag -a $"v($new_version)" -m $"Release v($new_version)"

    header $"Done! Tagged v($new_version)"
    print "Push with: just push-all && just push-tags"
}

main $env.args.0?
