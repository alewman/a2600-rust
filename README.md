# a2600-rust

A Rust Atari 2600 board transcribed from
[a2600-python](https://github.com/alewman/a2600-python) (frozen at the tag
`a2600-rust-pin-1`) on [6502-rust](https://github.com/alewman/6502-rust),
and proven equivalent to it: judged directly against the die where the
reference is, and in lockstep with the reference everywhere else. It is the
base for `a2600-rust-fast`.

The board is the reference's, module for module: a half-colour-clock model of
the TIA's circuit (picture, sync, RDY, HMOVE, collisions, both sound
channels), the RIOT with its lazy timer, the 13-bit bus with WSYNC as a
stretched read, every cartridge scheme the licensed dumps use (2K/4K, F8, F6,
F4 and their Super Chip forms, E0, 3F, FE, E7, FA, CV, UA, FC, F0, the
32-in-1, the DPC and the Supercharger with its stand-in BIOS and tape), the
controllers (joystick, paddles, keypads, driving controller, Booster Grip,
SaveKey), identification through MAME's software list and the board's own
detector, the NTSC, PAL and SECAM rows and the #699 consoles' TIA revisions.
Names and hardware comments are carried over; where Rust forced a difference,
the module header says so. Not yet: the reference's pygame window.

```text
cargo build --release
target/release/a2600 ROM.zip --frames 120 --png last.png --wav sound.wav
```

It runs at about two to three times real time (100 to 160 frames a second on
Combat and Pitfall! on a loaded machine), some fifteen times the reference on
PyPy: correct, not yet fast.

## Certification

| What | Version |
| --- | --- |
| a2600-python (the reference) | tag `a2600-rust-pin-1` (`21080ca`) |
| 6502-rust | `71747eb` (from 6502-python `741419a`, the revision the reference pins) |
| tia-fpga's die traces | lrrosa/tia-fpga `7bcd84e`, `sim/traces` only |

Ladder status at this commit (Linux x86_64, rustc 1.93.1; the reference on
PyPy 7.3.20):

| Rung | What | Result | Script |
| --- | --- | --- | --- |
| 1 | The TIA against the die: every Sim2600 trace, tia-fpga's 22 and the reference's 17, every pin and both audio pads, half clock by half clock | 39 of 39 pass: 37 match everywhere, `hmove_cycles` and `hmove_resp0` by exactly the reference's counted C27 departures (lum 230 / col 220 / blank 32 and 2638 / 2638 / 352) | `rung1.sh` |
| 2 | The reference's test programs in lockstep: the Stella issue ROMs, the #699 serial ROMs on the PAL row under each of the seven revisions, the die cartridges | 132 of 132 traces identical | `rung2.sh` |
| 3 | The smoke group in lockstep (every scheme, PAL, the sweep's residue, classics), 150 slices each | 112 of 112 identical | `rung3.sh` |
| 4 | Long runs with inputs in lockstep: Pitfall! (3,000 slices), River Raid, Adventure, Kaboom! (paddles), Space Invaders, Pitfall II (DPC), Combat on PAL with both joysticks, BurgerTime (E7), Decathlon (FE), Dragonstomper (AR) | 10 of 10 identical | `rung4.sh` |
| 5 | Identification: scheme, Super Chip and source for every local dump against the reference's `cartridge.identify` | 1,586 files, every line identical | `rung5.sh` |

A slice is 262 lines of CPU cycles. The trace (`src/trace.rs`) carries, each
slice: the cycle, instruction and halted-cycle counts, the CPU's registers,
the RIOT's RAM and raw timer state, a CRC of every line the TIA drew and of
every audio sample, and the TIA's counters; with `--steps A:B`, one line per
instruction to narrow a difference down. The command-line tool's WAV is
byte-identical to the reference's and its PNG decodes to the same pixels.

**Seeded faults** (`scripts/seeded_faults.py`): fourteen one-line faults, in
the TIA rules the die settled (C24, C25, C26, C27, C28, C29), a console's
revision field, the RIOT's wrap-cycle rule both ways (C6), FE, 3F, the DPC,
the Supercharger and the WSYNC halt: all caught, six by the die alone. Two
of the first set changed nothing observable (a guard term the test already
implied, and RDY sampled on another step the board documents as equivalent)
and were replaced by faults that do.

Two things found on the way: the TIA, transcribed, matched all 39 die traces
at its first build; and a bug in this board's reading of MAME's list (a
closing tag's name) made Dig Dug and Video Life build the wrong scheme, which
rungs 3 and 5 caught.

CI runs the build, `cargo clippy -D warnings`, the tests and rung 1 on
tia-fpga's public traces. The reference is private and the ROMs are local, so
rungs 2-5 run by hand:

```text
scripts/fetch_a2600python.sh external/a2600-python /path/to/a2600-python   # the pin, with a PyPy venv
scripts/fetch_traces.sh                                                       # tia-fpga's traces
external/a2600-python/.venv-pypy/bin/python external/a2600-python/scripts/die_carts.py \
    --out external/a2600-python/tests/die_carts
scripts/rung1.sh
scripts/rung2.sh && scripts/rung3.sh && scripts/rung4.sh   # --reference-cache external/refcache reuses traces
scripts/rung5.sh
scripts/seeded_faults.py
```

`scripts/cases.tsv` (written by `scripts/make_cases.py`) names the cases;
`docs/handoff-brief.md` is the brief this was built from.

## Licence

MIT (see `LICENSE`). Nothing third-party is committed: no ROMs, no traces of
games, no die traces (fetched), no reference code.
