#!/usr/bin/env nu
# ──────────────────────────────────────────────────────────────────────────────
# WeatherTrack — supply-chain checks (cargo audit, cargo deny)
# ──────────────────────────────────────────────────────────────────────────────
# The workspace and the crates that are their own workspace (the Topcoat web app needs a newer
# Rust than the rest) are checked together, so a dependency of the web app cannot slip past.
#
# Usage: nu scripts/ci/security.nu audit
#        nu scripts/ci/security.nu deny
# ──────────────────────────────────────────────────────────────────────────────

# Crates outside the root workspace whose dependencies are checked too.
export const EXTRA_WORKSPACES = ["crates/weathertrack-web"]

# The cargo invocations `audit` runs.
export def audit_commands []: nothing -> list<list<string>> {
    [[cargo audit]] ++ ($EXTRA_WORKSPACES | each {|w| [cargo audit -f $"($w)/Cargo.lock"] })
}

# The cargo invocations `deny` runs.
export def deny_commands []: nothing -> list<list<string>> {
    [[cargo deny check]] ++ ($EXTRA_WORKSPACES | each {|w| [cargo deny --manifest-path $"($w)/Cargo.toml" check] })
}

def run_all [commands: list<list<string>>] {
    mut failed = false
    for c in $commands {
        print $"(ansi cyan)▶(ansi reset) ($c | str join ' ')"
        let r = (^($c | first) ...($c | skip 1) | complete)
        print $r.stdout
        if $r.exit_code != 0 {
            print -e $r.stderr
            $failed = true
        }
    }
    if $failed { exit 1 }
}

def "main audit" [] { run_all (audit_commands) }
def "main deny" [] { run_all (deny_commands) }
def main [] { print "usage: security.nu audit | deny" }
