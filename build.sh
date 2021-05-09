#!/bin/bash
target_src="target/thumbv7em-none-eabihf/release/wio-sauna-monitor"
target_bin="target/thumbv7em-none-eabihf/release/wio-sauna-monitor.bin"
target="wio-sauna-monitor.uf2"
device_dir="/path/to/wio-terminal"

## update
cargo update

## build
cargo build --release

## elf to bin

arm-none-eabi-objcopy -O binary $target_src $target_bin

## bin to ef2
python3 uf2conv.py -c -b 0x4000 -o $target $target_bin

## Flash to target
cp $target $device_dir
