#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
wasm-pack build --target web crates/fabula-wasm --out-dir ../../docs/static/wasm/pkg
