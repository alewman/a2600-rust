//! Multi-game and many-bank oddities: Dynacom's F0, UA Ltd.'s UA, Amiga's FC,
//! the 32-in-1 (`cartridge/multicarts.py`). Each is specified by the sources
//! the reference names in its class, all secondary.

use super::connector::Cartridge;

const BANK: usize = 0x1000;

/// F0, the Dynacom Megaboy: sixteen 4K banks, each access to $1FF0 the next;
/// the wrap from 15 to 0 and start in bank 15 are Stella's.
pub struct DynacomF0 {
    rom: Vec<u8>,
    pub bank: usize,
}

impl DynacomF0 {
    pub fn new(image: &[u8], start_bank: usize) -> Result<Self, String> {
        if image.len() != 16 * BANK {
            return Err(format!("F0 needs 65536 bytes, not {}", image.len()));
        }
        Ok(DynacomF0 {
            rom: image.to_vec(),
            bank: start_bank,
        })
    }
}

impl Cartridge for DynacomF0 {
    fn name(&self) -> &'static str {
        "F0"
    }
    fn access(&mut self, address: u16, _value: u8, _t: i64) -> Option<u8> {
        if address & 0x1000 == 0 {
            return None;
        }
        if address == 0x1FF0 {
            self.bank = (self.bank + 1) & 15;
        }
        Some(self.rom[(self.bank << 12) | (address & 0xFFF) as usize])
    }
}

/// UA: two 4K banks selected by accesses to $0220 and $0240, below the
/// cartridge; the decode compares A12, A9, A6 and A5 only. Power-on bank 0.
pub struct UaLimited {
    rom: Vec<u8>,
    swap: usize,
    pub bank: usize,
}

impl UaLimited {
    pub fn new(image: &[u8], swapped: bool, start_bank: usize) -> Result<Self, String> {
        if image.len() != 2 * BANK {
            return Err(format!("UA needs 8192 bytes, not {}", image.len()));
        }
        Ok(UaLimited {
            rom: image.to_vec(),
            swap: swapped as usize,
            bank: start_bank,
        })
    }
}

impl Cartridge for UaLimited {
    fn name(&self) -> &'static str {
        "UA"
    }
    fn access(&mut self, address: u16, _value: u8, _t: i64) -> Option<u8> {
        let decoded = address & 0x1260;
        if decoded == 0x0220 {
            self.bank = self.swap;
        } else if decoded == 0x0240 {
            self.bank = 1 - self.swap;
        }
        if address & 0x1000 != 0 {
            return Some(self.rom[(self.bank << 12) | (address & 0xFFF) as usize]);
        }
        None
    }
}

/// FC, Amiga's Power Play Arcade Video Game Album: 4K banks chosen in two
/// steps; without an R/W line the board latches on any access and combines
/// the halves plainly. Power-on bank 0.
pub struct AmigaFC {
    rom: Vec<u8>,
    banks: usize,
    pub bank: usize,
    low: usize,
    high: usize,
}

impl AmigaFC {
    pub fn new(image: &[u8], start_bank: usize) -> Result<Self, String> {
        let banks = image.len() / BANK;
        if image.len() % BANK != 0 || banks < 2 {
            return Err(format!(
                "FC needs whole 4K banks, not {} bytes",
                image.len()
            ));
        }
        Ok(AmigaFC {
            rom: image.to_vec(),
            banks,
            bank: start_bank,
            low: 0,
            high: 0,
        })
    }
}

impl Cartridge for AmigaFC {
    fn name(&self) -> &'static str {
        "FC"
    }
    fn access(&mut self, address: u16, value: u8, _t: i64) -> Option<u8> {
        if address & 0x1000 == 0 {
            return None;
        }
        if address == 0x1FF8 {
            self.low = (value & 3) as usize;
        } else if address == 0x1FF9 {
            self.high = value as usize;
        } else if address == 0x1FFC {
            self.bank = ((self.high << 2) | self.low) % self.banks;
        }
        Some(self.rom[(self.bank << 12) | (address & 0xFFF) as usize])
    }
}

/// The 32-in-1 (Europe, Australia): 32 2K games, one chosen per power-on
/// (`game`, 0-31).
pub struct ThirtyTwoInOne {
    rom: Vec<u8>,
    pub game: usize,
}

impl ThirtyTwoInOne {
    pub fn new(image: &[u8], game: usize) -> Result<Self, String> {
        if image.len() != 32 * 0x800 {
            return Err(format!("32IN1 needs 65536 bytes, not {}", image.len()));
        }
        if game >= 32 {
            return Err(format!("game must be 0-31, not {game}"));
        }
        Ok(ThirtyTwoInOne {
            rom: image[game * 0x800..(game + 1) * 0x800].to_vec(),
            game,
        })
    }
}

impl Cartridge for ThirtyTwoInOne {
    fn name(&self) -> &'static str {
        "32IN1"
    }
    fn access(&mut self, address: u16, _value: u8, _t: i64) -> Option<u8> {
        if address & 0x1000 != 0 {
            return Some(self.rom[(address & 0x7FF) as usize]);
        }
        None
    }
}
