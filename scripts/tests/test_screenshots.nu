#!/usr/bin/env nu
# The screenshot list in scripts/screenshots.nu.

use std/assert
use runner.nu *
use ../screenshots.nu [SHOTS pairs]

def "test screenshots: every entry maps a preview render to a docs file" [] {
    for p in (pairs /repo $SHOTS) {
        assert ($p.from | str starts-with "/repo/target/")
        assert ($p.to | str starts-with "/repo/docs/screenshots/")
        assert ($p.to | str ends-with ".png")
    }
}

def "test screenshots: destinations are unique" [] {
    assert equal ($SHOTS.dest | uniq | length) ($SHOTS.dest | length)
}

def main [] { run-tests }
