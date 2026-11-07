//! Activision's FE (SCABS): the bank latched from the data bus the cycle
//! after $01FE (`cartridge/activision.py`).
//!
//! Specification: David Crane's patent, EP 84300730.3: a comparator watches
//! the address bus for $01FE; one cycle later a latch takes D7-D5 from the
//! data bus and enables one 4K bank. A JSR with S = $FF pushes the return
//! address's low byte at $01FE, and the next cycle fetches the target's high
//! byte, so a JSR to $Dxxx (D5 clear) lands in the second bank and one to
//! $Fxxx in the first; an RTS pulls the low byte from $01FE and the high byte
//! from $01FF the next cycle. **Which bits**: D5, the one latch bit a
//! two-bank board needs (Stella's choice; Gopher2600's differs only on bytes
//! these programs never put there). **Power-on**: the reset sequence's stack
//! reads ($0100, $01FF, $01FE with S = 0) choose the bank.

use super::connector::Cartridge;

const BANK: usize = 0x1000;

pub struct ActivisionFE {
    rom: Vec<u8>,
    pub bank: usize,
    /// the last cycle's address was $01FE
    armed: bool,
}

impl ActivisionFE {
    pub fn new(image: &[u8], start_bank: usize) -> Result<Self, String> {
        if image.len() != 2 * BANK {
            return Err(format!("FE needs 8192 bytes, not {}", image.len()));
        }
        Ok(ActivisionFE {
            rom: image.to_vec(),
            bank: start_bank,
            armed: false,
        })
    }
}

impl Cartridge for ActivisionFE {
    fn name(&self) -> &'static str {
        "FE"
    }
    fn watches_bus(&self) -> bool {
        true
    }
    fn access(&mut self, address: u16, _value: u8, _t: i64) -> Option<u8> {
        if address & 0x1000 != 0 {
            return Some(self.rom[(self.bank << 12) | (address & 0xFFF) as usize]);
        }
        None
    }
    fn bus(&mut self, address: u16, value: u8, _t: i64) {
        if self.armed {
            self.bank = if value & 0x20 != 0 { 0 } else { 1 };
        }
        self.armed = address == 0x01FE;
    }
}
