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
#   nu scripts/esp.nu backup-p4 [--port P]   # dump the parts of the P4 flash worth keeping
# ──────────────────────────────────────────────────────────────────────────────

const ROOT = path self ..

# Crates that still use the 8 MB generated sdkconfig (ESP32-C6 boards).
export const C6_CRATES = ["weathertrack-tui" "weathertrack-gui"]

# sdkconfig.defaults for a C6 crate; `partitions` is the absolute path of its CSV.
export def c6_sdkconfig [partitions: string]: nothing -> string {
    [
        "CONFIG_ESP_MAIN_TASK_STACK_SIZE=65536"
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

# Parts of the P4 flash worth keeping (a full 32 MB read takes about an hour over USB):
# partition table, bootloader + NVS area, and the factory app.
def "main backup-p4" [--port: string = "", --dir: path = "backups"] {
    mkdir $dir
    let p = if ($port | is-empty) { [] } else { ["--port" $port] }
    ^espflash read-flash ...$p "0x8000" "0x1000" ($dir | path join ptable.bin)
    ^espflash read-flash ...$p "0" "0x110000" ($dir | path join p4-0x0-boot-nvs.bin)
    ^espflash read-flash ...$p "0x110000" "0x560000" ($dir | path join p4-factory-app-0x110000.bin)
    print $"Backed up to ($dir). Restore with: espflash write-bin 0x0 <boot-nvs.bin>; espflash write-bin 0x110000 <factory-app.bin>"
}

def main [] { help main }
