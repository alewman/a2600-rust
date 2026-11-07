//! The Arcadia/Starpath Supercharger (AR): 6K of RAM, a 2K BIOS ROM, a tape
//! input (`cartridge/supercharger.py`).
//!
//! Sources, all secondary (the reference's module header cites each): Kevin
//! Horton's mapper document and FPGA model, sctech.txt, Eckhard Stolberg on
//! the .bin format. **Memory**: three 2K RAM banks and the BIOS; the control
//! register (DDDB BBWE) picks the pair with D4-D2, enables RAM writes with D1
//! and powers the ROM with D0 = 0. **Writing the register**: any access with
//! A12 high and A11-A8 low latches A7-A0; a later access to $1FF8 copies the
//! latch into the register. **Writing RAM**: the $F0xx access starts a count
//! of address-bus changes; the fifth change, if W is set and that half is
//! RAM, stores the latched byte and drives it on the bus (an $F0xx access
//! re-arms only when W is clear or nothing was pending before it).
//! **The tape** (bring-your-own BIOS: built, never run here) and **the fast
//! load**: without a BIOS the ROM holds the reference's 83-byte stand-in,
//! which reaches the load by an access the cartridge watches ($FA00 the
//! first load, $FB00+n multiload n) and then leaves what the BIOS leaves.

use super::connector::Cartridge;

pub const LOAD: usize = 8448;
const BANK: usize = 0x800;
const ROM: usize = 3;
/// D4-D2 -> (bank at $1000-$17FF, bank or ROM at $1800-$1FFF), banks 0-based
const MAPS: [(usize, usize); 8] = [
    (2, ROM),
    (0, ROM),
    (2, 0),
    (0, 2),
    (2, ROM),
    (1, ROM),
    (2, 1),
    (1, 2),
];
pub const NTSC_CPU_HZ: f64 = 3_579_545.0 / 3.0;

/// The stand-in BIOS, assembled by hand ($F800 multiload entry, $F80A reset).
#[rustfmt::skip]
const DUMMY_BIOS_CODE: [u8; 83] = [
    0xA6, 0xFA,              // F800 LDX $FA          the wanted load
    0xBD, 0x00, 0xFB,        // F802 LDA $FB00,X      the cartridge loads it
    0x4C, 0x10, 0xF8,        // F805 JMP $F810
    0xEA, 0xEA,              // F808
    0x78,                    // F80A SEI              reset: the first load
    0xD8,                    // F80B CLD
    0xAD, 0x00, 0xFA,        // F80C LDA $FA00        the cartridge loads it
    0xEA,                    // F80F
    0xA9, 0x00,              // F810 LDA #0
    0xA2, 0x1D,              // F812 LDX #$1D
    0x95, 0x80,              // F814 STA $80,X        $81-$9D
    0xCA,                    // F816 DEX
    0xD0, 0xFB,              // F817 BNE $F814
    0xA2, 0x29,              // F819 LDX #$29
    0x95, 0x03,              // F81B STA $03,X        TIA $04-$2C
    0xCA,                    // F81D DEX
    0xD0, 0xFB,              // F81E BNE $F81B
    0xAD, 0x02, 0xFC,        // F820 LDA $FC02        the control byte
    0x85, 0x80,              // F823 STA $80
    0xA9, 0xCD, 0x85, 0xFA,  // F825 LDA #$CD / STA $FA   CMP $FFF8
    0xA9, 0xF8, 0x85, 0xFB,  // F829 LDA #$F8 / STA $FB
    0xA9, 0xFF, 0x85, 0xFC,  // F82D LDA #$FF / STA $FC
    0xA9, 0x4C, 0x85, 0xFD,  // F831 LDA #$4C / STA $FD   JMP start
    0xAD, 0x00, 0xFC,        // F835 LDA $FC00
    0x85, 0xFE,              // F838 STA $FE
    0xAD, 0x01, 0xFC,        // F83A LDA $FC01
    0x85, 0xFF,              // F83D STA $FF
    0xA6, 0x80,              // F83F LDX $80
    0xBD, 0x00, 0xF0,        // F841 LDA $F000,X      latch the control byte
    0xA9, 0x0A,              // F844 LDA #$0A
    0x8D, 0x96, 0x02,        // F846 STA TIM64T       the BIOS leaves the timer running
    0xA2, 0xFF,              // F849 LDX #$FF
    0x9A,                    // F84B TXS
    0xA0, 0x00,              // F84C LDY #0
    0xA9, 0x00,              // F84E LDA #0
    0x4C, 0xFA, 0x00,        // F850 JMP $00FA
];
const FIRST_LOAD: usize = 0x200;
const NUMBERED_LOAD: usize = 0x300;
const HEADER: usize = 0x400; // offsets in the 2K ROM
const TIMER_VALUE: usize = 0x45; // offset of the stand-in's LDA #$0A operand (F845)

pub fn dummy_bios(timer: Option<u8>) -> Vec<u8> {
    let mut rom = vec![0u8; BANK];
    rom[..DUMMY_BIOS_CODE.len()].copy_from_slice(&DUMMY_BIOS_CODE);
    match timer {
        None => rom[TIMER_VALUE + 1..TIMER_VALUE + 4].copy_from_slice(&[0xEA, 0xEA, 0xEA]),
        Some(value) => rom[TIMER_VALUE] = value,
    }
    rom[0x7FC..0x800].copy_from_slice(&[0x0A, 0xF8, 0x0A, 0xF8]); // reset and IRQ: $F80A
    rom
}

pub fn split_loads(image: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    if image.is_empty() || image.len() % LOAD != 0 {
        return Err(format!(
            "AR images are whole 8448-byte loads, not {} bytes",
            image.len()
        ));
    }
    Ok(image.chunks(LOAD).map(|c| c.to_vec()).collect())
}

pub struct Supercharger {
    loads: Vec<Vec<u8>>,
    fast_load: bool,
    rom: Vec<u8>,
    pub ram: Vec<u8>,
    pub control: u8,
    lower: usize,
    upper: usize,
    writes: bool,
    rom_on: bool,
    latch: u8,
    pending: bool,
    /// address changes since the latch
    changes: u32,
    last_address: i32,
    tape: Option<Tape>,
}

impl Supercharger {
    pub fn new(
        image: &[u8],
        bios: Option<&[u8]>,
        ram_fill: u8,
        tape_start: i64,
        cpu_hz: f64,
        timer: Option<u8>,
    ) -> Result<Self, String> {
        let loads = split_loads(image)?;
        if let Some(bios) = bios {
            if bios.len() != BANK {
                return Err(format!(
                    "the Supercharger BIOS is 2048 bytes, not {}",
                    bios.len()
                ));
            }
        }
        let tape = bios.map(|_| Tape::new(&loads, cpu_hz, tape_start));
        let mut cart = Supercharger {
            fast_load: bios.is_none(),
            rom: match bios {
                None => dummy_bios(timer),
                Some(bios) => bios.to_vec(),
            },
            loads,
            ram: vec![ram_fill; 3 * BANK],
            control: 0,
            lower: 0,
            upper: 0,
            writes: false,
            rom_on: true,
            latch: 0,
            pending: false,
            changes: 0,
            last_address: -1,
            tape,
        };
        cart.map();
        Ok(cart)
    }

    fn map(&mut self) {
        (self.lower, self.upper) = MAPS[((self.control >> 2) & 7) as usize];
        self.writes = self.control & 2 != 0;
        self.rom_on = self.control & 1 == 0;
    }

    /// The fast load: copy a load's pages into RAM and post its header to the stand-in.
    pub fn load(&mut self, number: Option<usize>) {
        let mut chosen = 0;
        if let Some(number) = number {
            if let Some(found) = self
                .loads
                .iter()
                .position(|ld| ld[0x2005] as usize == number)
            {
                chosen = found;
            }
        }
        let load = &self.loads[chosen];
        let header = &load[0x2000..0x2100];
        for j in 0..header[3] as usize {
            let entry = header[0x10 + j];
            let (bank, page) = ((entry & 3) as usize, ((entry >> 2) & 7) as usize);
            if bank == ROM {
                continue;
            }
            let start = bank * BANK + page * 0x100;
            self.ram[start..start + 0x100].copy_from_slice(&load[j * 0x100..(j + 1) * 0x100]);
        }
        let posted = [header[0], header[1], header[2]]; // start low, start high, control
        self.rom[HEADER..HEADER + 3].copy_from_slice(&posted);
    }
}

impl Cartridge for Supercharger {
    fn name(&self) -> &'static str {
        "AR"
    }
    fn access(&mut self, address: u16, _value: u8, t: i64) -> Option<u8> {
        let change = if address as i32 == self.last_address {
            false
        } else {
            self.last_address = address as i32;
            true
        };
        let mut written = None;
        let armed = self.pending; // the commit below must not let this access arm again
        if self.pending && change {
            self.changes += 1;
            if self.changes == 5 {
                self.pending = false;
                if address & 0x1000 != 0 && self.writes {
                    let bank = if address & 0x800 != 0 {
                        self.upper
                    } else {
                        self.lower
                    };
                    if bank != ROM {
                        written = Some(self.latch);
                        self.ram[bank * BANK + (address & 0x7FF) as usize] = self.latch;
                    }
                }
            }
        }
        if address & 0x1000 == 0 {
            return None;
        }
        if address & 0xF00 == 0 && !(armed && self.writes) {
            self.latch = address as u8;
            self.pending = true;
            self.changes = 0;
        } else if address == 0x1FF8 {
            self.control = self.latch;
            self.pending = false;
            self.map();
        } else if address == 0x1FF9 {
            if let Some(tape) = &self.tape {
                return Some(tape.level(t));
            }
        }
        if written.is_some() {
            return written;
        }
        let bank = if address & 0x800 != 0 {
            self.upper
        } else {
            self.lower
        };
        if bank == ROM {
            if !self.rom_on {
                return None;
            }
            let offset = (address & 0x7FF) as usize;
            if self.fast_load {
                if offset == FIRST_LOAD {
                    self.load(None);
                } else if (NUMBERED_LOAD..NUMBERED_LOAD + 0x100).contains(&offset) {
                    self.load(Some(offset - NUMBERED_LOAD));
                }
            }
            return Some(self.rom[offset]);
        }
        Some(self.ram[bank * BANK + (address & 0x7FF) as usize])
    }
}

/// The loads as the cassette plays them: a level at each CPU cycle (Horton's framing).
pub struct Tape {
    edges: Vec<f64>,
    periods: Vec<f64>,
}

impl Tape {
    const ZERO_US: f64 = 227.0;
    const ONE_US: f64 = 340.0;
    const PREAMBLE_US: f64 = 1e6 / 750.0;

    pub fn new(loads: &[Vec<u8>], cpu_hz: f64, start: i64) -> Self {
        let mut periods: Vec<f64> = vec![Self::PREAMBLE_US; 800];
        for load in loads {
            let header = &load[0x2000..0x2100];
            let mut stream = bits(&[0x55; 256]);
            stream.push(0); // the start mark: one 0 pulse
            let mut body: Vec<u8> = header[0..8].to_vec();
            for j in 0..header[3] as usize {
                body.push(header[0x10 + j]);
                body.push(header[0x40 + j]);
                body.extend_from_slice(&load[j * 0x100..(j + 1) * 0x100]);
            }
            stream.extend(bits(&body));
            stream.extend(std::iter::repeat_n(0, 256));
            periods.extend(stream.iter().map(|&bit| {
                if bit != 0 {
                    Self::ONE_US
                } else {
                    Self::ZERO_US
                }
            }));
        }
        let mut edges = Vec::with_capacity(periods.len() + 1);
        let mut now = start as f64;
        let cycles_per_us = cpu_hz / 1e6;
        for &period in &periods {
            edges.push(now);
            now += period * cycles_per_us;
        }
        edges.push(now);
        let periods = periods.iter().map(|p| p * cycles_per_us).collect();
        Tape { edges, periods }
    }

    pub fn level(&self, t: i64) -> u8 {
        let t = t as f64;
        let i = self.edges.partition_point(|&e| e <= t) as i64 - 1; // bisect_right - 1
        if i < 0 || i as usize >= self.periods.len() {
            return 0;
        }
        let i = i as usize;
        if t - self.edges[i] < self.periods[i] / 2.0 {
            1
        } else {
            0
        }
    }
}

fn bits(data: &[u8]) -> Vec<u8> {
    data.iter()
        .flat_map(|&byte| (0..8).rev().map(move |bit| (byte >> bit) & 1))
        .collect()
}
