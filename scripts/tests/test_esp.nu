#!/usr/bin/env nu
# The pure parts of scripts/esp.nu.

use std/assert
use runner.nu *
use ../esp.nu [c6_sdkconfig p4_sdkconfig newest_bootloader p4_flash_args C6_CRATES]

def "test esp: c6 sdkconfig carries the partition path and 8 MB flash" [] {
    let s = c6_sdkconfig "/x/partitions.csv"
    assert ($s | str contains "CONFIG_PARTITION_TABLE_CUSTOM_FILENAME=\"/x/partitions.csv\"")
    assert ($s | str contains "CONFIG_ESPTOOLPY_FLASHSIZE_8MB=y")
    assert ($s | str ends-with "\n")
}

def "test esp: p4 sdkconfig fills every placeholder" [] {
    let s = p4_sdkconfig "A=\"@PARTITIONS@\"\nB=\"@PARTITIONS@\"\n" "/abs/p.csv"
    assert equal $s "A=\"/abs/p.csv\"\nB=\"/abs/p.csv\"\n"
}

def "test esp: no bootloader gives null" [] {
    assert equal (newest_bootloader "/definitely/not/here") null
}

def "test esp: the newest bootloader wins" [] {
    let root = mktemp --directory
    for h in [aaa bbb] {
        let d = $root | path join esp-idf-sys $h out build bootloader
        mkdir $d
        "x" | save ($d | path join bootloader.bin)
        sleep 20ms
    }
    assert equal (newest_bootloader $root | path split | reverse | get 4) bbb
    rm -rf $root
}

def "test esp: flash args add the port only when given" [] {
    let with = p4_flash_args /b.bin /app /dev/ttyACM0
    assert ($with | any {|a| $a == "--port" })
    assert equal ($with | last) "/app"
    let without = p4_flash_args /b.bin /app ""
    assert not ($without | any {|a| $a == "--port" })
    assert ($without | any {|a| $a == "esp32p4" })
}

def "test esp: the c6 crates exist" [] {
    const root = path self ../..
    for c in $C6_CRATES {
        assert ($root | path join crates $c partitions.csv | path exists) $"($c) has partitions.csv"
    }
}

def main [] { run-tests }
