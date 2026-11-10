#!/usr/bin/env nu
# The pure parts of scripts/esp.nu.

use std/assert
use runner.nu *
use ../esp.nu [c6_sdkconfig p4_sdkconfig newest_bootloader p4_flash_args C6_CRATES parse_ptable image_size]

# One 32-byte partition entry.
def pt_entry [name: string, type: int, offset: int, size: int]: nothing -> binary {
    let n = ($name | encode utf-8) | bytes add --end (0..<(16 - ($name | str length)) | each { 0x[00] } | bytes collect)
    let u32 = {|v| $v | into binary --compact | bytes add --end 0x[00 00 00 00] | bytes at 0..<4 }
    [0x[aa 50] ($type | into binary --compact | bytes at 0..<1) 0x[00] (do $u32 $offset) (do $u32 $size) $n 0x[00 00 00 00]] | bytes collect
}

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

def "test esp: the partition table is parsed up to the first empty entry" [] {
    let table = [(pt_entry nvs 1 0x9000 0x6000) (pt_entry factory 0 0x10000 0x100000) (0..<32 | each { 0x[ff] } | bytes collect)] | bytes collect
    let rows = parse_ptable $table
    assert equal ($rows.name) [nvs factory]
    assert equal $rows.1.offset 0x10000
    assert equal $rows.1.size 0x100000
}

def "test esp: an image size follows its segment lengths" [] {
    # header (24) + 2 segments of 4 and 8 data bytes, read through a fake flash
    let seg = {|len| [0x[00 00 00 00] ($len | into binary --compact | bytes add --end 0x[00 00 00 00] | bytes at 0..<4) (0..<$len | each { 0x[aa] } | bytes collect)] | bytes collect }
    let flash = [0x[e9 02] (0..<22 | each { 0x[00] } | bytes collect) (do $seg 4) (do $seg 8) (0..<64 | each { 0x[00] } | bytes collect)] | bytes collect
    let read = {|off, len| $flash | bytes at $off..<($off + $len) }
    assert equal (image_size $read 0) (((24 + 12 + 16 + 1 + 15) // 16) * 16 + 32)
}

def "test esp: erased flash has no image" [] {
    let read = {|off, len| 0..<$len | each { 0x[ff] } | bytes collect }
    assert equal (image_size $read 0x10000) null
}

def main [] { run-tests }
