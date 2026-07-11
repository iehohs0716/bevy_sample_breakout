#!/usr/bin/env bash
#
# Bevy アプリを WASM ビルドし、wasm-bindgen で JS グルーを生成して
# frontend/public/ に配置する。アセットも public/assets/ にコピーする。
#
# 事前準備（一度きり）:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version 0.2.126   # Cargo.lock と一致させること
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRONTEND_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ENGINE_DIR="$(cd "$FRONTEND_DIR/../game_engine" && pwd)"
OUT_DIR="$FRONTEND_DIR/public"
CRATE_NAME="bevy_sample"
OUT_NAME="breakout"

echo "==> cargo build (release, wasm32-unknown-unknown)"
cargo build --release --target wasm32-unknown-unknown \
  --manifest-path "$ENGINE_DIR/Cargo.toml"

WASM_IN="$ENGINE_DIR/target/wasm32-unknown-unknown/release/${CRATE_NAME}.wasm"

echo "==> wasm-bindgen --target web"
wasm-bindgen --out-dir "$OUT_DIR/wasm" --out-name "$OUT_NAME" --target web "$WASM_IN"

# 任意: binaryen が入っていればサイズ最適化する。
if command -v wasm-opt >/dev/null 2>&1; then
  echo "==> wasm-opt -Oz"
  wasm-opt -Oz -o "$OUT_DIR/wasm/${OUT_NAME}_bg.wasm" "$OUT_DIR/wasm/${OUT_NAME}_bg.wasm"
else
  echo "==> wasm-opt が無いためサイズ最適化はスキップ (brew install binaryen で有効化)"
fi

echo "==> assets を public/assets へコピー"
rm -rf "$OUT_DIR/assets"
cp -r "$ENGINE_DIR/assets" "$OUT_DIR/assets"

echo "==> 完了: $OUT_DIR/wasm/${OUT_NAME}.js"
ls -lh "$OUT_DIR/wasm"
