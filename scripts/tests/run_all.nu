#!/usr/bin/env nu
# Find and run all test_*.nu files in scripts/tests/

def main [] {
    let test_files = (ls scripts/tests/test_*.nu | get name)
    let total = ($test_files | length)
    mut passed = 0
    mut failed = 0

    for file in $test_files {
        print $"Running ($file)..."
        try {
            nu $file
            $passed = $passed + 1
            print $"  PASSED"
        } catch {
            $failed = $failed + 1
            print $"  FAILED"
        }
    }

    print $"\nResults: ($passed)/($total) passed, ($failed) failed"
    if $failed > 0 { exit 1 }
}

main
