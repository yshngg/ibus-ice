#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

DICT_COMPILER="${DICT_COMPILER:-$PROJECT_DIR/target/release/dict-compiler}"
DATA_DIR="${DATA_DIR:-$PROJECT_DIR/rime-ice}"
OUTPUT_DIR="${OUTPUT_DIR:-$PROJECT_DIR/build}"
OUTPUT_DICT="${OUTPUT_DIR}/ice.dict"

mkdir -p "$OUTPUT_DIR"

echo "Building dictionary compiler..."
cargo build --release -p dict-compiler

echo "Compiling dictionaries..."
"$DICT_COMPILER" "$OUTPUT_DICT" \
    "$DATA_DIR/cn_dicts/8105.dict.yaml" \
    "$DATA_DIR/cn_dicts/base.dict.yaml" \
    "$DATA_DIR/cn_dicts/ext.dict.yaml" \
    "$DATA_DIR/cn_dicts/others.dict.yaml" \
    "$DATA_DIR/en_dicts/en.dict.yaml" \
    "$DATA_DIR/en_dicts/en_ext.dict.yaml"

echo "Dict compiled: $OUTPUT_DICT"
ls -lh "$OUTPUT_DICT"
