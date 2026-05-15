#!/usr/bin/env nu
# Tests for version.nu
use runner.nu [run-tests, assert]

def "test version: has three semver parts" [] {
    let version = (open Cargo.toml | get workspace.package.version)
    let parts = ($version | split row ".")
    assert equal ($parts | length) 3
}

def "test version: parts are numeric" [] {
    let version = (open Cargo.toml | get workspace.package.version)
    let parts = ($version | split row ".")
    for part in $parts {
        assert ($part | into int | describe | str starts-with "int")
    }
}

def "test version: matches workspace dep" [] {
    let ws_version = (open Cargo.toml | get workspace.package.version)
    let dep_version = (open Cargo.toml | get workspace.dependencies.weathertrack-core.version)
    assert equal $ws_version $dep_version
}

def main [] { run-tests }
