#!/usr/bin/env nu
# Print the current workspace version from Cargo.toml

def main [] {
    let version = (open Cargo.toml | get workspace.package.version)
    print $version
}

main
