//! M-Network's E7: a selectable lower 2K (ROM or 1K of RAM) and 256 bytes of
//! banked RAM (`cartridge/mnetwork.py`).
//!
//! The 16K ROM is eight 2K slices; $1800-$1FFF always shows the last. An
//! access to $1FE0-$1FE6 puts a slice at $1000-$17FF; $1FE7 puts the first
//! 1K of RAM there instead, written at $1000-$13FF and read at $1400-$17FF.
//! $1800-$19FF always holds one 256-byte block of the second 1K, written at
//! $1800-$18FF and read at $1900-$19FF, the block chosen by $1FE8-$1FEB.
//! **Smaller ROMs** (8K, 12K) keep the 16K board's selects with Stella's
//! mapping. **Power-on**: slice 0 below and RAM block 0.

use super::connector::Cartridge;

const SLICE: usize = 0x800;

/// the slice each of $1FE0-$1FE6 selects, by ROM size (None: no switch)
fn selects(size: usize) -> Option<[Option<usize>; 7]> {
    match size {
        0x2000 => Some([None, None, None, None, Some(0), Some(1), Some(2)]),
        0x3000 => Some([
            Some(0),
            Some(1),
            Some(0),
            Some(1),
            Some(2),
            Some(3),
            Some(4),
        ]),
        0x4000 => Some([
            Some(0),
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            Some(5),
            Some(6),
        ]),
        _ => None,
    }
}

/// The lower 2K: a ROM slice, or the first 1K of RAM.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lower {
    Slice(usize),
    Ram,
}

pub struct MNetworkE7 {
    rom: Vec<u8>,
    selects: [Option<usize>; 7],
    pub lower: Lower,
    upper: usize,
    /// 1K for the lower 2K, then four 256 B blocks
    pub ram: Vec<u8>,
    pub block: usize,
}

impl MNetworkE7 {
    pub fn new(image: &[u8], ram_fill: u8) -> Result<Self, String> {
        let selects = selects(image.len())
            .ok_or_else(|| format!("E7 needs 8K, 12K or 16K, not {} bytes", image.len()))?;
        Ok(MNetworkE7 {
            rom: image.to_vec(),
            selects,
            lower: Lower::Slice(0),
            upper: image.len() - SLICE,
            ram: vec![ram_fill; 0x800],
            block: 0x400,
        })
    }
}

impl Cartridge for MNetworkE7 {
    fn name(&self) -> &'static str {
        "E7"
    }
    fn access(&mut self, address: u16, value: u8, _t: i64) -> Option<u8> {
        if address & 0x1000 == 0 {
            return None;
        }
        if (0x1FE0..=0x1FEB).contains(&address) {
            let index = (address - 0x1FE0) as usize;
            if index < 7 {
                if let Some(slice) = self.selects[index] {
                    self.lower = Lower::Slice(slice);
                }
            } else if index == 7 {
                self.lower = Lower::Ram;
            } else {
                self.block = 0x400 + ((index - 8) << 8);
            }
        }
        let offset = (address & 0xFFF) as usize;
        if offset < 0x800 {
            return match self.lower {
                Lower::Ram => {
                    if offset < 0x400 {
                        self.ram[offset] = value;
                        None
                    } else {
                        Some(self.ram[offset - 0x400])
                    }
                }
                Lower::Slice(slice) => Some(self.rom[slice * SLICE + offset]),
            };
        }
        if offset < 0xA00 {
            if offset < 0x900 {
                self.ram[self.block + (offset & 0xFF)] = value;
                return None;
            }
            return Some(self.ram[self.block + (offset & 0xFF)]);
        }
        Some(self.rom[self.upper + (offset & 0x7FF)])
    }
}
