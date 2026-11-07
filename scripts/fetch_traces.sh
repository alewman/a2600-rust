#!/usr/bin/env bash
# tia-fpga's die traces (data recorded from Sim2600, fetched, never committed)
# at the commit the reference pins, into external/tia-fpga: only sim/traces,
# so none of the CERN-OHL-S RTL is checked out.
set -euo pipefail
REV=7bcd84ebf4715e6a1538d4c6c6b379594b03ba70
ROOT=$(cd "$(dirname "$0")/.." && pwd)
DEST=${1:-"$ROOT/external/tia-fpga"}
if [ ! -d "$DEST/.git" ]; then
    git clone -q --filter=blob:none --no-checkout https://github.com/lrrosa/tia-fpga "$DEST"
fi
git -C "$DEST" sparse-checkout set --no-cone /sim/traces/
git -C "$DEST" checkout -q "$REV"
echo "tia-fpga $(git -C "$DEST" rev-parse --short HEAD): $(find "$DEST/sim/traces" -name '*.trace' | wc -l) traces"
