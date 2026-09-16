#!/bin/sh
set -eu
cd "$(dirname "$0")"
# rustup selects one consistent compiler/toolchain for native and WASM targets.
rustup run stable cargo test --lib
rustup run stable cargo build --release --lib --target wasm32-unknown-unknown
rustup run stable cargo build --release --bin serve
mkdir -p bin
cp target/wasm32-unknown-unknown/release/orb_weaver_pet.wasm web/
cp target/release/serve bin/
chmod +x 'Start Orb.command'
