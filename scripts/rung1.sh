#!/usr/bin/env bash
# Rung 1: the TIA against the die: every Sim2600 trace given, every pin and
# both audio pads, half clock by half clock; hmove_cycles and hmove_resp0 by
# exactly the reference's counted C27 departures.
# Usage: scripts/rung1.sh [TRACE_DIR ...]   (default: tia-fpga's, then the
#        reference's local ones when present)
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
DIRS=("$@")
if [ ${#DIRS[@]} -eq 0 ]; then
    DIRS=("$ROOT/external/tia-fpga/sim/traces")
    [ -d /data/emu/a2600-python/tests/die_traces ] && DIRS+=(/data/emu/a2600-python/tests/die_traces)
fi
cargo build --release -q --manifest-path "$ROOT/Cargo.toml"
find "${DIRS[@]}" -name '*.trace' -print0 | sort -z | xargs -0 "$ROOT/target/release/die-trace"
