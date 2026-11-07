//! Atari's own schemes: 2K/4K without banking, and F8/F6/F4 with optional
//! Super Chip RAM (`cartridge/atari.py`).
//!
//! Specification: Kevin Horton's bankswitching notes. Every access with A12
//! high to a hotspot selects a 4K bank, read or write, dummy or not: F8
//! $1FF8-$1FF9 (2 banks), F6 $1FF6-$1FF9 (4), F4 $1FF4-$1FFB (8).
//!
//! **The byte a hotspot read returns** is the new bank's: the bank latch
//! changes during the access and the ROM's output settles from the new bank
//! before the cycle ends (C20).
//!
//! **The Super Chip** (SC) adds 128 bytes of RAM: written at $1000-$107F,
//! read at $1080-$10FF, the ROM disabled for those 256 bytes. Without an R/W
//! line, any access to the write window writes the byte on the bus.
//!
//! **The power-on bank** is arbitrary on a console. The default is MAME
//! 0.285's (F6 bank 0, F4 bank 7, F8 bank 1).

use super::connector::Cartridge;

const BANK: usize = 0x1000;

/// scheme: (first hotspot, banks, MAME's power-on bank)
fn hotspots(scheme: &str) -> Option<(u16, usize, usize)> {
    match scheme {
        "F8" => Some((0x1FF8, 2, 1)),
        "F6" => Some((0x1FF6, 4, 0)),
        "F4" => Some((0x1FF4, 8, 7)),
        _ => None,
    }
}

/// 2K or 4K with no banking; a 2K image appears twice in the 4K window.
pub struct Plain {
    name: &'static str,
    rom: Vec<u8>,
}

impl Plain {
    pub fn new(image: &[u8]) -> Result<Self, String> {
        if image.len() != 0x800 && image.len() != 0x1000 {
            return Err(format!(
                "2K/4K cartridge must be 2048 or 4096 bytes, not {}",
                image.len()
            ));
        }
        let name = if image.len() == 0x800 { "2K" } else { "4K" };
        Ok(Plain {
            name,
            rom: image.repeat(0x1000 / image.len()),
        })
    }
}

impl Cartridge for Plain {
    fn name(&self) -> &'static str {
        self.name
    }
    fn access(&mut self, address: u16, _value: u8, _t: i64) -> Option<u8> {
        if address & 0x1000 != 0 {
            return Some(self.rom[(address & 0xFFF) as usize]);
        }
        None
    }
}

/// F8, F6 or F4, with `super_chip` RAM if asked.
pub struct Banked {
    name: &'static str,
    rom: Vec<u8>,
    first_hotspot: u16,
    last_hotspot: u16,
    pub bank: usize,
    pub ram: Option<[u8; 128]>,
}

impl Banked {
    pub fn new(
        image: &[u8],
        scheme: &str,
        super_chip: bool,
        start_bank: Option<usize>,
        ram_fill: u8,
    ) -> Result<Self, String> {
        let (first, banks, mame_start) =
            hotspots(scheme).ok_or_else(|| format!("{scheme} is not an Atari scheme"))?;
        if image.len() != banks * BANK {
            return Err(format!(
                "{scheme} needs {} bytes, not {}",
                banks * BANK,
                image.len()
            ));
        }
        let name = match (scheme, super_chip) {
            ("F8", false) => "F8",
            ("F8", true) => "F8SC",
            ("F6", false) => "F6",
            ("F6", true) => "F6SC",
            ("F4", false) => "F4",
            _ => "F4SC",
        };
        Ok(Banked {
            name,
            rom: image.to_vec(),
            first_hotspot: first,
            last_hotspot: first + banks as u16 - 1,
            bank: start_bank.unwrap_or(mame_start),
            ram: if super_chip {
                Some([ram_fill; 128])
            } else {
                None
            },
        })
    }
}

impl Cartridge for Banked {
    fn name(&self) -> &'static str {
        self.name
    }
    fn access(&mut self, address: u16, value: u8, _t: i64) -> Option<u8> {
        if address & 0x1000 == 0 {
            return None;
        }
        if self.first_hotspot <= address && address <= self.last_hotspot {
            self.bank = (address - self.first_hotspot) as usize;
        }
        if let Some(ram) = &mut self.ram {
            if address < 0x1100 {
                if address < 0x1080 {
                    ram[(address & 0x7F) as usize] = value;
                    return None; // the ROM is disabled and the RAM is being written
                }
                return Some(ram[(address & 0x7F) as usize]);
            }
        }
        Some(self.rom[(self.bank << 12) | (address & 0xFFF) as usize])
    }
}
