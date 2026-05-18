#!/usr/bin/env nu
# Tests for version.nu — validates workspace version consistency.

def main [] {
    print "(ansi cyan_bold)Running version tests...(ansi reset)"

    # Test 1: version has three semver parts
    let version = (open Cargo.toml | get workspace.package.version)
    let parts = ($version | split row ".")
    if ($parts | length) != 3 {
        print $"  ❌ version should have 3 parts, got ($parts | length)"
        exit 1
    }
    print $"  ✅ version has 3 parts: ($version)"

    # Test 2: all parts are numeric
    for part in $parts {
        let num = ($part | into int)
        if ($num | describe) != "int" {
            print $"  ❌ version part '($part)' is not numeric"
            exit 1
        }
    }
    print "  ✅ all parts are numeric"

    # Test 3: workspace dep version matches package version
    let dep_version = (open Cargo.toml | get workspace.dependencies.weathertrack-core.version)
    if $version != $dep_version {
        print $"  ❌ workspace version ($version) != dep version ($dep_version)"
        exit 1
    }
    print $"  ✅ workspace dep version matches: ($dep_version)"

    print "(ansi green_bold)All version tests passed!(ansi reset)"
}

main
