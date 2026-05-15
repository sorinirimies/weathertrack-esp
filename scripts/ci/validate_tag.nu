#!/usr/bin/env nu
# Validate that a tag matches vX.Y.Z and matches Cargo.toml version.
# Exports `validate` for unit testing.

export def validate [tag: string]: nothing -> record<tag: string, version: string> {
    if not ($tag =~ '^v\d+\.\d+\.\d+$') {
        error make {msg: $"Invalid tag format '($tag)' — expected vX.Y.Z"}
    }

    let version = ($tag | str substring 1..)
    let cargo_version = (open Cargo.toml | get workspace.package.version)

    if $version != $cargo_version {
        error make {msg: $"Tag version ($version) != Cargo.toml version ($cargo_version)"}
    }

    {tag: $tag, version: $version}
}

def main [tag: string] {
    let result = (validate $tag)
    # Machine-readable output (for CI)
    print $"tag=($result.tag)"
    print $"version=($result.version)"
    # Human-readable to stderr
    print --stderr $"(ansi green)✔ Tag ($result.tag) is valid(ansi reset)"
}

main $env.args.0?
