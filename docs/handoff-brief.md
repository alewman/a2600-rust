# Handoff brief: a2600-rust

## The request

Aubrey, 2026-11-07: after a2600-python's seven milestones, "get ready for
starting the rust version (and then the rust fast version)", then "push it,
make both public, include the base class". Step 1, the Rust 6502 core, is
done: [6502-rust](https://github.com/alewman/6502-rust) (crate
`sixfiveohtwo`), transcribed from 6502-python `741419a` with all five rungs
passing. This is step 2: the board.

## What it is

A Rust transcription of a2600-python frozen at the annotated tag
**`a2600-rust-pin-1`** (`21080ca`), on 6502-rust at `76767e4`, proven
equivalent to it by lockstep and judged directly against the die where the
reference is. The reference is a correct-not-fast board; this one is the
base for `a2600-rust-fast`.

## What to transcribe

From `src/a2600_python/` at the pin, module for module, names and hardware
comments carried over: `machine`, `board`, `system`, `tia`, `audio`, `riot`,
`television`, `palettes`, `controllers`, and `cartridge/` (the connector,
identification with MAME's list and the signature detector, and every
scheme: 2K/4K, F8/F6/F4 with Super Chip, E0, 3F, FE, E7, FA, CV, UA, FC,
F0, 32-in-1, DPC, the Supercharger with its stand-in BIOS and tape), and
the `a2600-python ROM --png --wav` command. Not yet: the pygame window
(`frontend.py`); a window comes with the fast version.

Differences Rust forces, each to be named in its module header:

- The board is the 6507's `Bus`; the TIA runs inside each bus call, as in
  the reference.
- Python's callables (`TIA.pad_source`, `RIOT.port_a_device`, `on_line`)
  become devices the board owns and asks at the same moments.
- The DPC's oscillator uses Python float floor division (`//`), which is
  not `floor(a / b)` in every case: transcribe CPython's algorithm.

## The ladder

1. **The die** (the TIA's own judge, natively): every Sim2600 trace,
   tia-fpga's 22 and this project's 17, replayed as `scripts/die_trace.py`
   does, every pin and both audio pads scored: all match except the counted
   C27 departures (`tests/test_die_traces.py` `KNOWN_DEPARTURES`), to the
   half clock.
2. **Lockstep, test programs**: the reference's own test ROMs (the Stella
   issue ROMs, the #699 serial ROMs on the PAL row with each console's
   revision, the die cartridges) on both boards, the traces identical.
3. **Lockstep, games**: the smoke group (`tests/smoke_dumps.txt` at the pin,
   every scheme and PAL), 150 frames each, traces identical.
4. **Long runs**: classics for thousands of frames with inputs (Reset held,
   joystick moves, paddles), traces identical.
5. **Identification**: the scheme and Super Chip for every local dump agree
   with the reference's `cartridge.identify`.

**The trace.** Both boards write the same text for the same cartridge and
inputs: one line per slice of 19,912 CPU cycles (262 lines) with the cycle
and instruction counts, the CPU's registers, digests of RIOT RAM and the
timer, the lines the TIA drew in that slice and the audio samples, and the
TIA's counters; with `--steps A:B` one line per instruction too, for
narrowing a difference down. The reference's half is `scripts/a2600_trace.py`
here, run on a2600-python at the pin.

**Seeded faults** check the gate bites: a fault in each of the TIA's rules
the die settled, in a scheme and in the RIOT's timer must fail some rung.

## Working rules

- Do not modify a2600-python. If the reference looks wrong, stop and show
  the divergence.
- Transcribe; do not redesign.
- Never copy GPL code (Stella, Gopher2600, MAME) or CERN-OHL-S code (the
  tia-fpga RTL); the die traces are fetched data, not committed.
- Never commit ROMs, traces of commercial games, or anything fetched.
- `nice`, at most about 16 processes on the shared machine.
- CI is small: the reference is private, so CI runs the build, the tests and
  rung 1 on tia-fpga's public traces; rungs 2-5 run locally.
- Commit trailer: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
  The repository is public; push when Aubrey asks.
