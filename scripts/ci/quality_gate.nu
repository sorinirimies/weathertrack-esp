#!/usr/bin/env nu
# CI quality gate: fmt + clippy + test + nu tests
# Usage: nu scripts/ci/quality_gate.nu [--skip-fmt] [--skip-test] [--skip-nu]

def header [msg: string] { print $"\n(ansi cyan_bold)═══ ($msg) ═══(ansi reset)" }
def step [n: int, total: int, msg: string] { print $"  (ansi green_bold)▶(ansi reset) [($n)/($total)] ($msg)" }
def ok [msg: string] { print $"  (ansi green)✔(ansi reset) ($msg)" }
def fail_msg [msg: string] { print $"  (ansi red_bold)✖(ansi reset) ($msg)" }

def run-step [name: string, cmd: string]: nothing -> bool {
    let result = (do { nu -c $cmd } | complete)
    if $result.exit_code == 0 {
        ok $"($name) OK"
        true
    } else {
        fail_msg $"($name) FAILED"
        false
    }
}

def main [--skip-fmt, --skip-test, --skip-nu] {
    header "Quality Gate"
    let total = 4

    let r1 = if not $skip_fmt {
        step 1 $total "Formatting"
        run-step "Format" "cargo fmt --all -- --check"
    } else { step 1 $total "Formatting (skipped)"; true }

    step 2 $total "Clippy"
    let r2 = (run-step "Clippy" "cargo clippy -p weathertrack-core --all-targets --all-features -- -D warnings -A deprecated")

    let r3 = if not $skip_test {
        step 3 $total "Rust tests"
        run-step "Tests" "cargo test -p weathertrack-core --all-features --all-targets"
    } else { step 3 $total "Rust tests (skipped)"; true }

    let r4 = if not $skip_nu {
        step 4 $total "Nu tests"
        run-step "Nu tests" "nu scripts/tests/run_all.nu"
    } else { step 4 $total "Nu tests (skipped)"; true }

    let all_passed = [$r1, $r2, $r3, $r4] | all {|r| $r == true }

    if not $all_passed {
        let failures = ([$r1, $r2, $r3, $r4]
            | zip ["fmt", "clippy", "test", "nu-tests"]
            | where {|pair| $pair.0 == false }
            | each {|pair| $pair.1 }
            | str join ", ")
        header $"Quality Gate FAILED: ($failures)"
        exit 1
    }
    header "Quality Gate PASSED"
}

main
