#!/usr/bin/env nu
# Shared test utilities.
# Tests are standalone scripts — this file provides optional helpers.

# Assert two values are equal, or exit with error.
export def assert-eq [actual: any, expected: any, msg?: string] {
    if $actual != $expected {
        let message = if $msg != null { $msg } else { "Assertion failed" }
        print $"❌ ($message): expected ($expected), got ($actual)"
        exit 1
    }
}

# Assert condition is true, or exit with error.
export def assert-true [condition: bool, msg?: string] {
    if not $condition {
        let message = if $msg != null { $msg } else { "Assertion failed: expected true" }
        print $"❌ ($message)"
        exit 1
    }
}
