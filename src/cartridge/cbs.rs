//! CBS RAM Plus (FA): three 4K banks and 256 bytes of RAM (`cartridge/cbs.py`).
//!
//! Specification: Horton's notes, nocash 2k6specs. An access with A12 high
//! to $1FF8-$1FFA selects bank 0-2. The RAM is written at $1000-$10FF and
//! read at $1100-$11FF, in every bank; without an R/W line any access to the
//! write window stores the byte on the bus. **The power-on bank** is
//! Stella's, bank 2.

use super::connector::Cartridge;

const BANK: usize = 0x1000;

pub struct CbsFA {
    rom: Vec<u8>,
    pub bank: usize,
    pub ram: [u8; 256],
}

impl CbsFA {
    pub fn new(image: &[u8], start_bank: usize, ram_fill: u8) -> Result<Self, String> {
        if image.len() != 3 * BANK {
            return Err(format!("FA needs 12288 bytes, not {}", image.len()));
        }
        Ok(CbsFA {
            rom: image.to_vec(),
            bank: start_bank,
            ram: [ram_fill; 256],
        })
    }
}

impl Cartridge for CbsFA {
    fn name(&self) -> &'static str {
        "FA"
    }
    fn access(&mut self, address: u16, value: u8, _t: i64) -> Option<u8> {
        if address & 0x1000 == 0 {
            return None;
        }
        if (0x1FF8..=0x1FFA).contains(&address) {
            self.bank = (address - 0x1FF8) as usize;
        }
        if address < 0x1200 {
            if address < 0x1100 {
                self.ram[(address & 0xFF) as usize] = value;
                return None;
            }
            return Some(self.ram[(address & 0xFF) as usize]);
        }
        Some(self.rom[(self.bank << 12) | (address & 0xFFF) as usize])
    }
}
