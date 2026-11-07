#!/usr/bin/env bash
# Rung 4: long runs with inputs in lockstep.
exec python3 "$(dirname "$0")/lockstep.py" --only '^long-' --jobs "${JOBS:-12}" "$@"
