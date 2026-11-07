//! Parker Brothers' E0: four 1K slices of the 4K window, the last fixed
//! (`cartridge/parker.py`).
//!
//! Specification: Horton's notes, nocash 2k6specs "Cart 1K-Banking". The 8K
//! ROM is eight 1K slices. An access with A12 high to $1FE0-$1FE7 points the
//! window's first 1K at slice 0-7, $1FE8-$1FEF the second 1K, $1FF0-$1FF7
//! the third; the last 1K is always slice 7. **The power-on slices** are
//! Stella's (4, 5 and 6), the detector these dumps are compared with.

use super::connector::Cartridge;

const SLICE: usize = 0x400;

pub struct ParkerE0 {
    rom: Vec<u8>,
    /// the ROM offset of each 1K of the window
    pub offsets: [usize; 4],
}

impl ParkerE0 {
    pub fn new(image: &[u8], start_slices: [usize; 3]) -> Result<Self, String> {
        if image.len() != 8 * SLICE {
            return Err(format!("E0 needs 8192 bytes, not {}", image.len()));
        }
        let [a, b, c] = start_slices;
        Ok(ParkerE0 {
            rom: image.to_vec(),
            offsets: [a * SLICE, b * SLICE, c * SLICE, 7 * SLICE],
        })
    }
}

impl Cartridge for ParkerE0 {
    fn name(&self) -> &'static str {
        "E0"
    }
    fn access(&mut self, address: u16, _value: u8, _t: i64) -> Option<u8> {
        if address & 0x1000 == 0 {
            return None;
        }
        if (0x1FE0..=0x1FF7).contains(&address) {
            let index = (address - 0x1FE0) as usize;
            self.offsets[index >> 3] = (index & 7) * SLICE;
        }
        Some(self.rom[self.offsets[((address >> 10) & 3) as usize] | (address & 0x3FF) as usize])
    }
}
