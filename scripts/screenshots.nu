#!/usr/bin/env nu
# Copy the preview renders the README shows into docs/screenshots/ (run `just screenshots`).

const ROOT = path self ..

# (source under target/, README name)
export const SHOTS = [
    [src dest];
    ["p4-ui/1-today.png" "p4-slint-today.png"]
    ["p4-ui/2-hourly.png" "p4-slint-hourly.png"]
    ["gui-ui/1-today.png" "c6-slint-today.png"]
    ["gui-ui/2-hourly.png" "c6-slint-hourly.png"]
    ["tui-ui/p4-1-today.png" "p4-tui-today.png"]
    ["tui-ui/c6-1-today.png" "c6-tui-today.png"]
    ["tui-ui/p4-2-hourly.png" "p4-tui-hourly.png"]
    ["tui-ui/p4-3-7days.png" "p4-tui-week.png"]
    ["tui-ui/p4-4-details.png" "p4-tui-details.png"]
    ["tui-ui/c6-3-7days.png" "c6-tui-week.png"]
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
