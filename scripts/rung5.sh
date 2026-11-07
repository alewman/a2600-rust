#!/usr/bin/env bash
# Rung 5: identification agrees with the reference's cartridge.identify on
# every local dump (the No-Intro zips, then the retrobat 7z set).
# Usage: scripts/rung5.sh [ROMS] [RETRO]
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
ROMS=${1:-/data/emu/roms/atari2600}
RETRO=${2:-/data/emu/roms-retrobat/atari2600}
PYTHON="$ROOT/external/a2600-python/.venv-pypy/bin/python"
OUT="$ROOT/external/identify"
mkdir -p "$OUT"
cargo build --release -q --manifest-path "$ROOT/Cargo.toml"
find "$ROMS" -maxdepth 1 -name '*.zip' -print0 | sort -z > "$OUT/files"
find "$RETRO" -maxdepth 1 -name '*.7z' -print0 | sort -z >> "$OUT/files"
xargs -0 -n 200 nice "$ROOT/target/release/identify" < "$OUT/files" > "$OUT/rust.tsv"
xargs -0 -n 200 nice "$PYTHON" "$ROOT/scripts/identify.py" < "$OUT/files" > "$OUT/reference.tsv"
total=$(wc -l < "$OUT/reference.tsv")
if cmp -s "$OUT/reference.tsv" "$OUT/rust.tsv"; then
    echo "identify: $total files, every line identical"
else
    diff "$OUT/reference.tsv" "$OUT/rust.tsv" | head -20
    echo "identify: $total files, lines differ"
    exit 1
fi
