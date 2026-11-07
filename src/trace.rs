//! The lockstep trace: what both boards write for the same cartridge and
//! inputs (docs/handoff-brief.md, "The trace"; `scripts/a2600_trace.py` is
//! the reference's half and writes the same text).
//!
//! ```text
//! # a2600-trace 1
//! S <slice> c=<cycles> i=<instructions> h=<halted cycles> cpu=<A X Y S PC P>
//!   ram=<crc32 of RIOT RAM> timer=<value>/<flags>/<divider>/<countdown>/<as of>/<wrapped at>
//!   px=<crc32 of the lines drawn>/<lines> au=<crc32 of the samples>/<samples>
//!   tia=<frame>/<line>/<step in line>/<hcount>/<collisions>/<vsync><vblank> bus=<data bus>
//! I <instruction> c=<cycles> cpu=<A X Y S PC P>          (with --steps A:B)
//! ```
//!
//! A slice is 19,912 CPU cycles (262 lines); each runs whole instructions
//! until the board's cycle count reaches the slice's end. The lines drawn are
//! each line's 228 pixel bytes and VSYNC as the TIA hands them out; the
//! samples are the audio's floats as IEEE doubles, little-endian. The RIOT's
//! timer is its raw lazy state, read without catching up. Inputs change at
//! slice boundaries.

pub const HEADER: &str = "# a2600-trace 1";
pub const SLICE_CYCLES: i64 = 19_912;

/// CRC-32 (IEEE), as Python's `zlib.crc32`, kept running across calls.
#[derive(Clone, Copy, Debug)]
pub struct Crc32(u32);

impl Default for Crc32 {
    fn default() -> Self {
        Crc32(0xFFFF_FFFF)
    }
}

impl Crc32 {
    pub fn update(&mut self, data: &[u8]) {
        let mut crc = self.0;
        for &byte in data {
            crc ^= byte as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        self.0 = crc;
    }

    pub fn value(&self) -> u32 {
        !self.0
    }
}

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = Crc32::default();
    crc.update(data);
    crc.value()
}
