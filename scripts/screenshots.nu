#!/usr/bin/env nu
# Copy the preview renders the README shows into docs/screenshots/ (run `just screenshots`).

const ROOT = path self ..

# (source under target/, README name)
# The P4 screenshots are not here: they are captured from the real screen (scripts/p4_shots.nu).
export const SHOTS = [
    [src dest];
    ["gui-ui/1-now.png" "c6-slint-today.png"]
    ["gui-ui/3-chart.png" "c6-slint-hourly.png"]
    ["gui-ui/5-wind.png" "c6-slint-wind.png"]
    ["tui-ui/c6-1-now.png" "c6-tui-today.png"]
    ["tui-ui/c6-4-wind.png" "c6-tui-wind.png"]
    ["tui-ui/c6-3-week.png" "c6-tui-week.png"]
    ["gui-ui/8-place.png" "c6-slint-place.png"]
    ["tui-ui/c6-7-place.png" "c6-tui-place.png"]
    ["tui-ui/themes/p4-today-nord.png" "p4-tui-nord.png"]
    ["tui-ui/themes/c6-today-catppuccin-latte.png" "c6-tui-latte.png"]
]

# The (from, to) absolute path pairs for `shots` under `root`.
export def pairs [root: path, shots: table]: nothing -> table {
    $shots | each {|s| { from: ($root | path join target $s.src), to: ($root | path join docs screenshots $s.dest) } }
}

def main [] {
    mkdir ($ROOT | path join docs screenshots)
    for p in (pairs $ROOT $SHOTS) {
        if not ($p.from | path exists) { error make { msg: $"missing ($p.from): run the previews first \(just screenshots)" } }
        cp $p.from $p.to
        print $"($p.to | path relative-to $ROOT)"
    }
}
