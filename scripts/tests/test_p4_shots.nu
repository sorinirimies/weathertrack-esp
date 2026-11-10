#!/usr/bin/env nu
# The pure parts of scripts/p4_shots.nu.

use std/assert
use runner.nu *
use ../p4_shots.nu [pages shot_url list_text DEMO_PLACES]

def "test p4 shots: every page has a unique name and a tab the wide layout has" [] {
    assert equal (pages | get name | uniq | length) (pages | length)
    for p in (pages) { assert ($p.tab in 0..5) $"($p.name): tab ($p.tab)" }
}

def "test p4 shots: the url names the page and encodes a search" [] {
    assert equal (shot_url "10.0.0.2" {name: today tab: 0 search: null}) "http://10.0.0.2/screenshot.bmp?tab=0"
    assert equal (shot_url "10.0.0.2" {name: s tab: 4 search: "Bad Rappenau"}) "http://10.0.0.2/screenshot.bmp?tab=4&search=Bad%20Rappenau"
}

def "test p4 shots: a theme can be forced for the terminal ui" [] {
    let page = {name: today tab: 0 search: null}
    assert equal (shot_url "10.0.0.2" $page 0) "http://10.0.0.2/screenshot.bmp?tab=0&theme=0"
    assert equal (shot_url "10.0.0.2" {name: s tab: 4 search: "A"} 2) "http://10.0.0.2/screenshot.bmp?tab=4&search=A&theme=2"
}

def "test p4 shots: the demo list is stored in the line format the device stores" [] {
    let lines = (list_text $DEMO_PLACES | lines)
    assert equal ($lines | length) ($DEMO_PLACES | length)
    assert equal ($lines | first) "48.78232|9.17702|Stuttgart"
    assert (($DEMO_PLACES | length) <= 8)
}

def "test p4 shots: both uis are captured under their own names" [] {
    assert ("place-list" in (pages | get name))
    assert ("place-search" in (pages | get name))
}

def main [] { run-tests }
