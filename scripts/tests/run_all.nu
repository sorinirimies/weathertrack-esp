#!/usr/bin/env nu
# Find and run all test_*.nu files in scripts/tests/

def main [] {
    let test_files = (ls scripts/tests/test_*.nu | get name)
    let total = ($test_files | length)

    let results = ($test_files | each {|file|
        print $"Running ($file)..."
        let ok = (do { nu $file } | complete | get exit_code) == 0
        if $ok {
            print $"  PASSED"
        } else {
            print $"  FAILED"
        }
        $ok
    })

    let passed = ($results | where $it == true | length)
    let failed = ($results | where $it == false | length)

    print $"\nResults: ($passed)/($total) passed, ($failed) failed"
    if $failed > 0 { exit 1 }
}

main
