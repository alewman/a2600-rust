#!/usr/bin/env bash
# Rung 2: the reference's test programs in lockstep (Stella issue ROMs, #699
# serial ROMs under each console's revision, the die cartridges).
exec python3 "$(dirname "$0")/lockstep.py" --only '^(test|hw|die)-' --jobs "${JOBS:-12}" "$@"
