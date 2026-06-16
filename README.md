# `ptrack`

A simple pitch tracker plugin using the YIN algorithm

## Installation instructions

Make sure you have the latest stable version of [Rust](https://rust-lang.org/tools/install/)

Simply run:
```shell
cargo install cargo-nice-plug
cargo nice-plug bundle ptrack --release
```

Then locate the file named `ptrack.clap` in `target/bundled` and copy it wherever your DAW scans for CLAP plugins. The VST3 bundle can be found deeper in that same folder.

Enjoy!
