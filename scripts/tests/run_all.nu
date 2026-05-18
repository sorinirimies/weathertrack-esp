#!/usr/bin/env nu
# Find and run all test_*.nu files in scripts/tests/

def main [] {
    let test_files = (ls scripts/tests/test_*.nu | get name)
    let total = ($test_files | length)

    let results = ($test_files | each {|file|
        print $"Running ($file)..."
        let result = (do { nu $file } | complete)
        if $result.exit_code == 0 {
            print "  ✅ PASSED"
            true
        } else {
            print "  ❌ FAILED"
            false
        }
    })

    let passed = ($results | filter {|r| $r } | length)
    let failed = ($results | filter {|r| not $r } | length)

    print $"\nResults: ($passed)/($total) passed, ($failed) failed"
    if $failed > 0 { exit 1 }
}

main
