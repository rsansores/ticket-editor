#!/usr/bin/env bash
# Build the ticket-core renderer to a browser-ready wasm module.
# Output lands in packages/ticket-editor/src/wasm and is imported by the editor.
#
# Requires: rustup target add wasm32-unknown-unknown
#           cargo install wasm-bindgen-cli --version <matches wasm-bindgen crate>
set -euo pipefail
cd "$(dirname "$0")/.."

echo "==> cargo build (wasm32, release)"
cargo build --release --target wasm32-unknown-unknown -p ticket-wasm

echo "==> wasm-bindgen"
wasm-bindgen target/wasm32-unknown-unknown/release/ticket_wasm.wasm \
  --out-dir packages/ticket-editor/src/wasm \
  --target web

echo "==> done: packages/ticket-editor/src/wasm"
