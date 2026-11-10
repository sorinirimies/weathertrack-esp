#!/usr/bin/env nu
# ──────────────────────────────────────────────────────────────────────────────
# WeatherTrack: screenshots of the real P4 screen (docs/screenshots/p4-<ui>-<page>.png)
# ──────────────────────────────────────────────────────────────────────────────
# The P4 must run a firmware built with the `shots` feature (`just flash-p4-shots`,
# `just flash-p4-tui-shots`); it then serves `GET /screenshot.bmp` (the live frame buffer, with
# `?tab=N` and `&search=city` to show a page first). A demo list of saved places is put on the
# device for the Place page and the device's own list is put back afterwards.
#
# Usage: nu scripts/p4_shots.nu 192.168.3.172 slint     (or: tui [--theme N], default theme 0)
# ──────────────────────────────────────────────────────────────────────────────

const ROOT = path self ..

# Places shown in the list (the place in use should be one of them so that it is highlighted).
export const DEMO_PLACES = [
    [lat lon name];
    [48.78232 9.17702 "Stuttgart"]
    [52.52 13.405 "Berlin"]
    [48.13743 11.57549 "München"]
    [47.37174 8.54226 "Zürich"]
    [50.11552 8.68417 "Frankfurt"]
]

# What to capture: the page name, its tab number and an optional search to run.
export def pages []: nothing -> table {
    [
        [name tab search];
        [today 0 null]
        [hourly 1 null]
        [week 2 null]
        [details 3 null]
        [place-list 4 null]
        [place-search 4 "Stuttgart"]
        [alerts 5 null]
    ]
}

# The URL of one capture; `theme` (a terminal UI theme index, or null for the stored one) makes
# the screenshots the same whatever theme is stored on the device.
export def shot_url [ip: string, page: record, theme: any = null]: nothing -> string {
    let base = $"http://($ip)/screenshot.bmp?tab=($page.tab)"
    let base = if $page.search == null { $base } else { $"($base)&search=($page.search | url encode)" }
    if $theme == null { $base } else { $"($base)&theme=($theme)" }
}

# A list of places in the `lat|lon|name` lines the device stores.
export def list_text [places: table]: nothing -> string {
    $places | each {|p| $"($p.lat)|($p.lon)|($p.name)" } | str join "\n"
}

# The device's own list, in the same text form.
def device_list [ip: string]: nothing -> string {
    http get $"http://($ip)/api/weather.json" | get saved | each {|p| $"($p.lat)|($p.lon)|($p.name)" } | str join "\n"
}

def put_list [ip: string, text: string] {
    http post --content-type application/x-www-form-urlencoded $"http://($ip)/saved" {list: $text} | ignore
}

def main [ip: string, ui: string, --theme: int] {
    if $ui not-in [slint tui] { error make { msg: $"ui must be slint or tui, not ($ui)" } }
    let theme = if $ui == tui { $theme | default 0 } else { null }
    let out = ($ROOT | path join docs screenshots)
    let original = (device_list $ip)
    put_list $ip (list_text $DEMO_PLACES)
    sleep 2sec
    try {
        for page in (pages) {
            let bmp = (mktemp --suffix .bmp)
            let png = ($out | path join $"p4-($ui)-($page.name).png")
            ^curl -sf -m 90 -o $bmp (shot_url $ip $page $theme)
            ^magick $bmp $png
            rm $bmp
            print $"($png | path relative-to $ROOT)"
        }
    } catch {|e|
        put_list $ip $original
        error make { msg: $e.msg }
    }
    put_list $ip $original
}
