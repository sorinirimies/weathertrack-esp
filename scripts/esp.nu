#!/usr/bin/env nu
# ──────────────────────────────────────────────────────────────────────────────
# WeatherTrack — ESP firmware helpers (the justfile calls these)
# ──────────────────────────────────────────────────────────────────────────────
# The firmware crates are not workspace members (they need the ESP toolchain), so
# their generated files and flashing live here, tested, instead of in sh recipes.
#
#   nu scripts/esp.nu gen-sdkconfig          # tui + gui sdkconfig.defaults
#   nu scripts/esp.nu gen-sdkconfig-p4       # p4 sdkconfig.defaults from the .in file
#   nu scripts/esp.nu flash-p4 [--port P]    # flash the release P4 build and monitor
#   nu scripts/esp.nu backup [--port P]      # any chip: partition table, boot area, each app's real size
# ──────────────────────────────────────────────────────────────────────────────

const ROOT = path self ..

# Crates that still use the 8 MB generated sdkconfig (ESP32-C6 boards).
export const C6_CRATES = ["weathertrack-tui" "weathertrack-gui"]

# sdkconfig.defaults for a C6 crate; `partitions` is the absolute path of its CSV.
export def c6_sdkconfig [partitions: string]: nothing -> string {
    [
        "CONFIG_ESP_MAIN_TASK_STACK_SIZE=40960"
        "CONFIG_ESP_WIFI_ENABLED=y"
        "CONFIG_MBEDTLS_CERTIFICATE_BUNDLE=y"
        "CONFIG_MBEDTLS_CERTIFICATE_BUNDLE_DEFAULT_FULL=y"
        "CONFIG_ESP_HTTP_CLIENT_ENABLE_HTTPS=y"
        "CONFIG_PARTITION_TABLE_CUSTOM=y"
        $"CONFIG_PARTITION_TABLE_CUSTOM_FILENAME=\"($partitions)\""
        "CONFIG_ESPTOOLPY_FLASHSIZE_8MB=y"
        "CONFIG_ESPTOOLPY_FLASHSIZE=\"8MB\""
    ] | str join "\n" | $"($in)\n"
}

# The P4 template with its @PARTITIONS@ placeholder filled in.
export def p4_sdkconfig [template: string, partitions: string]: nothing -> string {
    $template | str replace --all "@PARTITIONS@" $partitions
}

# The newest IDF-built bootloader under a release build dir, or null.
export def newest_bootloader [build_dir: path]: nothing -> any {
    let found = glob ($build_dir | path join "esp-idf-sys" "*" "out" "build" "bootloader" "bootloader.bin")
    if ($found | is-empty) { return null }
    $found | each {|f| { path: $f, modified: (ls $f | get modified.0) } } | sort-by modified | last | get path
}

# espflash arguments for the P4 (`port` empty = let espflash pick).
export def p4_flash_args [bootloader: path, image: path, port: string]: nothing -> list<string> {
    let base = [
        "flash" "--chip" "esp32p4" "--flash-size" "32mb" "--partition-table" "partitions.csv"
        "--bootloader" $bootloader "--monitor"
    ]
    let port_args = if ($port | is-empty) { [] } else { ["--port" $port] }
    [...$base ...$port_args $image]
}

# espflash arguments for the C6 (8 MB flash). `port` empty = let espflash pick; `monitor` opens the
# serial monitor afterwards (it needs a terminal, so leave it off from scripts and agents).
export def c6_flash_args [bootloader: path, image: path, port: string, monitor: bool]: nothing -> list<string> {
    let base = [
        "flash" "--chip" "esp32c6" "--flash-size" "8mb" "--partition-table" "partitions.csv"
        "--bootloader" $bootloader
    ]
    let port_args = if ($port | is-empty) { [] } else { ["--port" $port] }
    let monitor_args = if $monitor { ["--monitor"] } else { [] }
    [...$base ...$port_args ...$monitor_args $image]
}

# Little-endian unsigned integer from `n` bytes at `at`.
export def le [bytes: binary, at: int, n: int]: nothing -> int {
    $bytes | bytes at $at..<($at + $n) | into int --endian little
}

# Partition table entries from the raw table bytes (0x8000 in flash, 32 B per entry).
export def parse_ptable [bytes: binary]: nothing -> table {
    0..(($bytes | bytes length) // 32 - 1)
    | each {|i|
        let e = $bytes | bytes at ($i * 32)..<($i * 32 + 32)
        if ($e | bytes at 0..<2) != 0x[aa 50] { null } else {
            {
                name: ($e | bytes at 12..<28 | bytes split 0x[00] | first | decode utf-8)
                type: (le $e 2 1)
                subtype: (le $e 3 1)
                offset: (le $e 4 4)
                size: (le $e 8 4)
            }
        }
    }
    | take while {|r| $r != null }
}

# Bytes an app image really occupies, read through `read: {|offset, len| binary}`;
# null when there is no image (erased partition).
export def image_size [read: closure, offset: int]: nothing -> any {
    let head = do $read $offset 24
    if ($head | bytes at 0..<1) != 0x[e9] { return null }
    mut pos = 24
    for _ in 0..<(le $head 1 1) {
        let seg = do $read ($offset + $pos) 8
        $pos = $pos + 8 + (le $seg 4 4)
    }
    # + checksum byte padded to 16, + 32-byte SHA-256 digest
    (($pos + 1 + 15) // 16) * 16 + 32
}

def hex [n: int]: nothing -> string { $n | format number | get lowerhex }

def "main gen-sdkconfig" [] {
    for crate in $C6_CRATES {
        let dir = $ROOT | path join crates $crate
        let csv = $dir | path join partitions.csv | path expand
        let out = $dir | path join sdkconfig.defaults
        c6_sdkconfig $csv | save -f $out
        print $"Generated ($out) \(partition: ($csv))"
    }
}

def "main gen-sdkconfig-p4" [] {
    let dir = $ROOT | path join crates weathertrack-p4
    let csv = $dir | path join partitions.csv | path expand
    p4_sdkconfig (open --raw ($dir | path join sdkconfig.defaults.in)) $csv | save -f ($dir | path join sdkconfig.defaults)
    print $"Generated ($dir | path join sdkconfig.defaults)"
}

def "main flash-p4" [--port: string = ""] {
    let dir = $ROOT | path join crates weathertrack-p4
    let out = $dir | path join target riscv32imafc-esp-espidf release
    let bl = newest_bootloader ($out | path join build)
    if $bl == null { error make { msg: $"bootloader.bin not found under ($out)/build; run `just build-p4-release`" } }
    cd $dir
    ^espflash ...(p4_flash_args $bl ($out | path join weathertrack-p4) $port)
}

# Flash the release build of a C6 firmware (`gui` or `tui`): `just build-gui-release` first.
def "main flash-c6" [ui: string, --port: string = "", --no-monitor] {
    if $ui not-in [gui tui] { error make { msg: $"ui must be gui or tui, not ($ui)" } }
    let crate = $"weathertrack-($ui)"
    let dir = $ROOT | path join crates $crate
    let out = $dir | path join target riscv32imac-esp-espidf release
    let bl = newest_bootloader ($out | path join build)
    if $bl == null { error make { msg: $"bootloader.bin not found under ($out)/build; run `just build-($ui)-release`" } }
    cd $dir
    ^espflash ...(c6_flash_args $bl ($out | path join $crate) $port (not $no_monitor))
}

# Parts of the flash worth keeping, read over USB (~10 KB/s, so no full dump): the partition
# table, everything before the first app (bootloader, NVS, otadata) and each app's real size.
def "main backup" [--port: string = "", --dir: path = "backups"] {
    mkdir $dir
    let p = if ($port | is-empty) { [] } else { ["--port" $port] }
    let tmp = mktemp
    let read = {|off: int, len: int|
        ^espflash read-flash ...$p (hex $off) (hex $len) $tmp | complete | ignore
        open --raw $tmp
    }
    do $read 0x8000 0x1000 | save -f ($dir | path join ptable.bin)
    let table = parse_ptable (open --raw ($dir | path join ptable.bin))
    if ($table | is-empty) { error make { msg: "no partition table at 0x8000 (is this an ESP board in a normal boot?)" } }
    let apps = $table | where type == 0 | sort-by offset
    let first_app = $apps | get -o 0.offset | default 0x10000
    ^espflash read-flash ...$p 0 (hex $first_app) ($dir | path join $"boot-and-data-0x0.bin")
    for app in $apps {
        let size = image_size $read $app.offset
        if $size == null { print $"($app.name): empty, skipped"; continue }
        let file = $dir | path join $"($app.name)-(hex $app.offset).bin"
        ^espflash read-flash ...$p (hex $app.offset) (hex $size) $file
    }
    rm -f $tmp
    print $"Backed up to ($dir). Restore with: espflash write-bin 0x0 <boot-and-data-0x0.bin>; espflash write-bin <offset> <app.bin>"
}

def main [] { help main }
