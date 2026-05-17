#!/usr/bin/env nu
# Test runner with auto-discovery.
# Test files should define commands starting with "test " and call `run-tests` in main.

# Re-export std assertions for convenience
export use std/assert

export def run-tests [] {
    let commands = (scope commands | where name =~ '^test ' | get name)
    let total = ($commands | length)

    print $"(ansi cyan_bold)Running ($total) tests...(ansi reset)"

    let results = ($commands | each {|cmd|
        let info = (scope commands | where name == $cmd | first)
        if ($info.description? | default "" | str contains "ignore") {
            print $"  (ansi yellow)⊘(ansi reset) ($cmd) (skipped)"
            return {name: $cmd, ok: true, skipped: true}
        }

        let result = (do { do ($info.closure) } | complete)
        if $result.exit_code == 0 {
            print $"  (ansi green)✔(ansi reset) ($cmd)"
            {name: $cmd, ok: true, skipped: false}
        } else {
            print $"  (ansi red_bold)✖(ansi reset) ($cmd)"
            {name: $cmd, ok: false, skipped: false}
        }
    })

    let passed = ($results | where ok == true and skipped == false | length)
    let failed = ($results | where ok == false | length)

    print $"\n(ansi cyan_bold)Results:(ansi reset) ($passed) passed, ($failed) failed, ($total) total"

    if $failed > 0 {
        let failure_names = ($results | where ok == false | get name | str join ", ")
        print $"(ansi red_bold)Failed:(ansi reset) ($failure_names)"
        exit 1
    }
}
