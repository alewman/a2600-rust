#!/usr/bin/env bash
# The reference, a2600-python at the tag a2600-rust-pin-1, into external/ with
# a venv (PyPy runs it several times faster). The repository is private:
# SOURCE is a local clone or a URL you can read.
# Usage: scripts/fetch_a2600python.sh [DEST] [SOURCE]
set -euo pipefail
PIN=a2600-rust-pin-1
ROOT=$(cd "$(dirname "$0")/.." && pwd)
DEST=${1:-"$ROOT/external/a2600-python"}
SOURCE=${2:-git@github.com:alewman/a2600-python.git}
PYTHON=${PYTHON:-pypy3}
if [ ! -d "$DEST/.git" ]; then
    git clone -q "$SOURCE" "$DEST"
fi
git -C "$DEST" fetch -q --tags origin 2>/dev/null || true
git -C "$DEST" checkout -q "$PIN"
if [ ! -x "$DEST/.venv-pypy/bin/python" ]; then
    "$PYTHON" -m venv "$DEST/.venv-pypy"
fi
"$DEST/.venv-pypy/bin/python" -m pip install -q -e "$DEST"
echo "a2600-python $(git -C "$DEST" describe --tags) in $DEST"
