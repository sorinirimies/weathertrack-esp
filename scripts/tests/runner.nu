#!/usr/bin/env nu
# Test runner with auto-discovery.
# Test files should define commands starting with "test " and call `run-tests` in main.

# Re-export std assertions for convenience
export use std/assert

export def run-tests [] {
    let commands = (scope commands | where name =~ '^test ' | get name)
    let total = ($commands | length)
    mut passed = 0
    mut failed = 0
    mut failures = []

    print $"(ansi cyan_bold)Running ($total) tests...(ansi reset)"

    for cmd in $commands {
        try {
            # Skip tests with "ignore" in the description
            let info = (scope commands | where name == $cmd | first)
            if ($info.description? | default "" | str contains "ignore") {
                print $"  (ansi yellow)⊘(ansi reset) ($cmd) (skipped)"
                continue
            }

            do (scope commands | where name == $cmd | first).closure
            $passed = $passed + 1
            print $"  (ansi green)✔(ansi reset) ($cmd)"
        } catch {|e|
            $failed = $failed + 1
            $failures = ($failures | append $cmd)
            print $"  (ansi red_bold)✖(ansi reset) ($cmd): ($e.msg)"
        }
    }

    print $"\n(ansi cyan_bold)Results:(ansi reset) ($passed) passed, ($failed) failed, ($total) total"

    if $failed > 0 {
        print $"(ansi red_bold)Failed:(ansi reset) ($failures | str join ', ')"
        exit 1
    }
}
