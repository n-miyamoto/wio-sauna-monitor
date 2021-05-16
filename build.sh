#!/bin/bash
#features=""
features="--features without_sensors"
build_type="release" #release or debug
target_src="target/thumbv7em-none-eabihf/${build_type}/wio-sauna-monitor"
target_bin="target/thumbv7em-none-eabihf/${build_type}/wio-sauna-monitor.bin"
target="wio-sauna-monitor.uf2"
device_dir="/media/miyamoto/Arduino"

## update
cargo update

## build
#cargo build 
if [ $build_type = "release" ]; then
    cargo build --$build_type  $features
else
    cargo build $features
fi

## elf to bin
arm-none-eabi-objcopy -O binary $target_src $target_bin

## bin to ef2
python3 uf2conv.py -c -b 0x4000 -o $target $target_bin

## Flash to target
cp $target $device_dir
