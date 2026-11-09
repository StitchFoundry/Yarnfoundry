#!/bin/sh
# Builds the browser version into apps/yarnfoundry-web/pkg (needs wasm-bindgen-cli matching Cargo.lock).
set -eu
cd "$(dirname "$0")/../.."
target_dir="${CARGO_TARGET_DIR:-target}"
cargo build --release --locked --target wasm32-unknown-unknown -p yarnfoundry-web
wasm-bindgen --target web --no-typescript --out-dir apps/yarnfoundry-web/pkg --out-name yarnfoundry_web \
  "$target_dir/wasm32-unknown-unknown/release/yarnfoundry_web.wasm"
