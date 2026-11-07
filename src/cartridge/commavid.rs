//! CommaVid (CV): 2K of ROM and 1K of RAM, no banking (`cartridge/commavid.py`).
//!
//! The ROM fills $1800-$1FFF; the RAM is read at $1000-$13FF and written at
//! $1400-$17FF. A 4K dump carries a 1K RAM image ahead of the ROM.

use super::connector::Cartridge;

pub struct CommaVid {
    rom: Vec<u8>,
    pub ram: Vec<u8>,
}

impl CommaVid {
    pub fn new(image: &[u8], ram_fill: u8) -> Result<Self, String> {
        match image.len() {
            0x1000 => Ok(CommaVid {
                ram: image[..0x400].to_vec(),
                rom: image[0x800..].to_vec(),
            }),
            0x800 => Ok(CommaVid {
                ram: vec![ram_fill; 0x400],
                rom: image.to_vec(),
            }),
            n => Err(format!("CV needs 2048 or 4096 bytes, not {n}")),
        }
    }
}

impl Cartridge for CommaVid {
    fn name(&self) -> &'static str {
        "CV"
    }
    fn access(&mut self, address: u16, value: u8, _t: i64) -> Option<u8> {
        if address & 0x1000 == 0 {
            return None;
        }
        if address & 0x800 != 0 {
            return Some(self.rom[(address & 0x7FF) as usize]);
        }
        if address & 0x400 != 0 {
            self.ram[(address & 0x3FF) as usize] = value;
            return None;
        }
        Some(self.ram[(address & 0x3FF) as usize])
    }
}
