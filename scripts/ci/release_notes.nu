#!/usr/bin/env nu
# Generate release notes for a tagged version using git-cliff.

def main [tag?: string] {
    let version = if $tag != null { $tag } else { $"v(open Cargo.toml | get workspace.package.version)" }
    print $"Generating release notes for ($version)..."

    git-cliff --latest --strip header -o RELEASE_NOTES.md
    git-cliff -o CHANGELOG.md
    print "Generated RELEASE_NOTES.md and CHANGELOG.md"
}

main $env.args.0?
