//! Activision's DPC (Pitfall II): eight data fetchers, a random number
//! generator, music (`cartridge/dpc.py`).
//!
//! Primary source: David Crane, US patent 4,644,495 "Video memory system"
//! (1987), cited P column:line in the reference. **Program**: 8K in two 4K
//! banks switched by any access to $1FF8/$1FF9, as F8; power-on bank 1. The
//! chip owns $1000-$107F: $1000-$103F are reads it answers, $1040-$107F
//! writes it takes from the bus, A5-A3 the function and A2-A0 the fetcher.
//! **Fetchers** DF0-DF7: an 11-bit down-counter each addressing the 2K
//! display ROM (stored complemented), a Top and a Bottom register and a
//! flag. **Music**: DF5-DF7 clocked by the oscillator (20 kHz by default,
//! unmeasured); reads of $1004-$1007 return the three music flags weighted
//! 4, 5 and 6 and added; the cartridge has no audio pin, the program copies
//! that into AUDV0. **Random numbers**: an 8-bit shift register taking
//! NOT(b7 xor b5 xor b4 xor b3), stepped on every cartridge access
//! (`rng_clock` "cartridge", the patent) or as Stella does ("dpc").
//!
//! Difference forced by Rust: the oscillator's catch-up divides floats with
//! Python's `//`, which is not `floor(a / b)` in every case; [`py_floordiv`]
//! is CPython's algorithm, so the ticks fall on the same cycles.

use super::connector::Cartridge;

pub const NTSC_CPU_HZ: f64 = 3_579_545.0 / 3.0;
/// (DF7, DF6, DF5) as a 3-bit number -> the summed level
const MIX: [u8; 8] = [0, 4, 5, 9, 6, 10, 11, 15];

/// CPython's float floor division (`Objects/floatobject.c`, `_float_div_mod`).
pub fn py_floordiv(vx: f64, wx: f64) -> f64 {
    let mut m = vx % wx; // fmod
    let mut div = (vx - m) / wx;
    if m != 0.0 {
        if (wx < 0.0) != (m < 0.0) {
            m += wx;
            div -= 1.0;
        }
    } else {
        m = 0.0f64.copysign(wx);
    }
    let _ = m;
    if div != 0.0 {
        let mut floordiv = div.floor();
        if div - floordiv > 0.5 {
            floordiv += 1.0;
        }
        floordiv
    } else {
        0.0f64.copysign(vx / wx)
    }
}

pub fn rng_next(value: u8) -> u8 {
    let bit = !((value >> 7) ^ (value >> 5) ^ (value >> 4) ^ (value >> 3)) & 1;
    (value << 1) | bit
}

pub struct ActivisionDPC {
    rom: Vec<u8>,
    display: Vec<u8>,
    pub bank: usize,
    pub top: [u8; 8],
    pub bottom: [u8; 8],
    pub low: [u8; 8],
    pub high: [u8; 8],
    pub flag: [bool; 8],
    /// DF5-DF7 only
    pub music: [bool; 8],
    pub osc: [bool; 8],
    pub rng: u8,
    every_access: bool,
    cycles_per_tick: f64,
    /// the cycle of the oscillator's next tick
    next_tick: f64,
}

impl ActivisionDPC {
    pub fn new(
        image: &[u8],
        start_bank: usize,
        osc_hz: f64,
        cpu_hz: f64,
        rng_clock: &str,
    ) -> Result<Self, String> {
        if image.len() < 0x2800 {
            return Err(format!(
                "DPC needs 8K of program and 2K of display data, not {}",
                image.len()
            ));
        }
        if rng_clock != "cartridge" && rng_clock != "dpc" {
            return Err(format!(
                "rng_clock is 'cartridge' or 'dpc', not {rng_clock:?}"
            ));
        }
        let cycles_per_tick = cpu_hz / osc_hz;
        Ok(ActivisionDPC {
            rom: image[..0x2000].to_vec(),
            display: image[0x2000..0x2800].to_vec(),
            bank: start_bank,
            top: [0; 8],
            bottom: [0; 8],
            low: [0; 8],
            high: [0; 8],
            flag: [false; 8],
            music: [false; 8],
            osc: [false; 8],
            rng: 1,
            every_access: rng_clock == "cartridge",
            cycles_per_tick,
            next_tick: cycles_per_tick,
        })
    }

    // -- the oscillator ---------------------------------------------------

    fn catch_up(&mut self, t: i64) {
        if (t as f64) < self.next_tick {
            return;
        }
        let ticks = py_floordiv(t as f64 - self.next_tick, self.cycles_per_tick) as i64 + 1;
        self.next_tick += ticks as f64 * self.cycles_per_tick;
        for i in [5, 6, 7] {
            if self.music[i] && self.osc[i] {
                self.music_ticks(i, ticks);
            }
        }
    }

    fn music_ticks(&mut self, i: usize, mut ticks: i64) {
        let period = self.top[i] as i64 + 1;
        if ticks > 2 * period {
            // whole periods change nothing but the high stage
            let whole = (ticks - period).div_euclid(period);
            ticks -= whole * period;
            self.high[i] = ((self.high[i] as i64 - whole).rem_euclid(8)) as u8;
        }
        let (top, bottom) = (self.top[i], self.bottom[i]);
        let (mut low, mut flag) = (self.low[i], self.flag[i]);
        for _ in 0..ticks {
            if low == 0 {
                low = top;
                self.high[i] = self.high[i].wrapping_sub(1) & 7;
            } else {
                low -= 1;
            }
            if low == top {
                flag = true;
            } else if low == bottom {
                flag = false;
            }
        }
        self.low[i] = low;
        self.flag[i] = flag;
    }

    fn fetch(&mut self, function: u16, i: usize) -> u8 {
        let low = self.low[i];
        if low == self.top[i] {
            self.flag[i] = true;
        } else if low == self.bottom[i] {
            self.flag[i] = false;
        }
        let flag = self.flag[i];
        let byte = self.display[0x7FF - (((self.high[i] as usize) << 8) | low as usize)];
        let mask = if flag { 0xFF } else { 0x00 };
        let result = match function {
            1 => byte,
            2 => byte & mask,
            3 => byte.rotate_left(4) & mask,
            4 => byte.reverse_bits() & mask,
            5 => (byte >> 1) & mask,
            6 => (byte << 1) & mask,
            _ => mask,
        };
        if !(self.music[i] && self.osc[i]) {
            if low == 0 {
                self.low[i] = 0xFF;
                self.high[i] = self.high[i].wrapping_sub(1) & 7;
            } else {
                self.low[i] = low - 1;
            }
        }
        result
    }

    fn write(&mut self, function: u16, i: usize, value: u8) {
        match function {
            0 => {
                self.top[i] = value;
                self.flag[i] = false;
            }
            1 => self.bottom[i] = value,
            2 => self.low[i] = if self.music[i] { self.top[i] } else { value },
            3 => {
                self.high[i] = value & 7;
                if i >= 5 {
                    self.music[i] = value & 0x10 != 0;
                    self.osc[i] = value & 0x20 != 0;
                }
            }
            6 => self.rng = 1,
            _ => {}
        }
    }
}

impl Cartridge for ActivisionDPC {
    fn name(&self) -> &'static str {
        "DPC"
    }
    fn access(&mut self, address: u16, value: u8, t: i64) -> Option<u8> {
        if address & 0x1000 == 0 {
            return None;
        }
        let rng = self.rng;
        if self.every_access {
            self.rng = rng_next(rng);
        }
        if address & 0xF80 != 0 {
            if address == 0x1FF8 || address == 0x1FF9 {
                self.bank = (address - 0x1FF8) as usize;
                if !self.every_access {
                    self.rng = rng_next(rng);
                }
            }
            return Some(self.rom[(self.bank << 12) | (address & 0xFFF) as usize]);
        }
        if !self.every_access {
            self.rng = rng_next(rng);
        }
        self.catch_up(t);
        let i = (address & 7) as usize;
        let function = (address >> 3) & 7;
        if address & 0x40 != 0 {
            self.write(function, i, value);
            return None;
        }
        if function == 0 {
            if i < 4 {
                return Some(rng);
            }
            let mut levels = (self.music[5] && self.flag[5]) as usize
                | ((self.music[6] && self.flag[6]) as usize) << 1;
            levels |= ((self.music[7] && self.flag[7]) as usize) << 2;
            return Some(MIX[levels]);
        }
        Some(self.fetch(function, i))
    }
}
