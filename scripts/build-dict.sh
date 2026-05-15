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
# Find all dict.yaml files in rime-ice subdirectories
mapfile -t DICT_FILES < <(find "$DATA_DIR" -name "*.dict.yaml" | sort)
if [ ${#DICT_FILES[@]} -eq 0 ]; then
    echo "Error: No .dict.yaml files found in $DATA_DIR" >&2
    exit 1
fi
"$DICT_COMPILER" "$OUTPUT_DICT" "${DICT_FILES[@]}"

echo "Dict compiled: $OUTPUT_DICT"
ls -lh "$OUTPUT_DICT"
