#!/usr/bin/env bash
# Rung 3: the smoke group in lockstep, 150 slices each.
exec python3 "$(dirname "$0")/lockstep.py" --only '^smoke-' --jobs "${JOBS:-12}" "$@"
