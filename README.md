# wio-sauna-monitor

## rustc version
```sh
-> % rustc --version
rustc 1.61.0-nightly (3b348d932 2022-02-19)
```

## settings
- Modify `secrets.rs` to your own parameters
- Modify `$device_dir` of `build.sh` to your own parameters
- install `uf2conv.py` 
    * download from [here](https://github.com/microsoft/uf2/blob/11212a684e0378eda8f8cd22b163381cc8d07528/utils/uf2conv.py)

## build & flash

- Turn on wio-terminal with boot mode.
- Build and flash program with command below.

```sh
./build.sh
```

