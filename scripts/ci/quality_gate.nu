#!/usr/bin/env nu
# CI quality gate: fmt + clippy + test + nu tests
# Usage: nu scripts/ci/quality_gate.nu [--skip-fmt] [--skip-test] [--skip-nu]

def header [msg: string] { print $"\n(ansi cyan_bold)═══ ($msg) ═══(ansi reset)" }
def step [n: int, total: int, msg: string] { print $"  (ansi green_bold)▶(ansi reset) [($n)/($total)] ($msg)" }
def ok [msg: string] { print $"  (ansi green)✔(ansi reset) ($msg)" }
def fail [msg: string] { print $"  (ansi red_bold)✖(ansi reset) ($msg)" }

def main [--skip-fmt, --skip-test, --skip-nu] {
    header "Quality Gate"
    mut failures = []
    let total = 4

    if not $skip_fmt {
        step 1 $total "Formatting"
        try { cargo fmt --all -- --check; ok "Format OK" } catch { $failures = ($failures | append "fmt"); fail "Format FAILED" }
    } else { step 1 $total "Formatting (skipped)" }

    step 2 $total "Clippy"
    try { cargo clippy -p weathertrack-core --all-targets --all-features -- -D warnings -A deprecated; ok "Clippy OK" } catch { $failures = ($failures | append "clippy"); fail "Clippy FAILED" }

    if not $skip_test {
        step 3 $total "Rust tests"
        try { cargo test -p weathertrack-core --all-features --all-targets; ok "Tests OK" } catch { $failures = ($failures | append "test"); fail "Tests FAILED" }
    } else { step 3 $total "Rust tests (skipped)" }

    if not $skip_nu {
        step 4 $total "Nu tests"
        try { nu scripts/tests/run_all.nu; ok "Nu tests OK" } catch { $failures = ($failures | append "nu-tests"); fail "Nu tests FAILED" }
    } else { step 4 $total "Nu tests (skipped)" }

    if ($failures | length) > 0 {
        header $"Quality Gate FAILED: ($failures | str join ', ')"
        exit 1
    }
    header "Quality Gate PASSED"
}

main
