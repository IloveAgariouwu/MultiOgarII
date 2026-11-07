#!/bin/sh
set -e
cd "$(dirname "$0")"
cargo build --release --target wasm32-unknown-unknown -p ogar-web "$@"
"${WASM_BINDGEN:-wasm-bindgen}" --target web --out-dir ui/src/pkg target/wasm32-unknown-unknown/release/ogar_web.wasm
