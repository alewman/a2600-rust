//! Tigervision's 3F: the window's lower 2K selected by a write to TIA space
//! (`cartridge/tigervision.py`).
//!
//! Specification: Horton's notes (four 2K slices, the last fixed at
//! $1800-$1FFF, "any STA in the $00-$3F range will change banks", the low
//! two bits chosen, one 74LS173), nocash 2k6specs. The cartridge has no R/W
//! line and no clock, so the latch's clock is A12: alex_79 found the switch
//! happens when an access with A6 and A7 low (and A12 low) is followed by one
//! with A12 high. The 74LS173 then takes the byte still on the data bus as
//! A12 rises: the byte of the access before, which for `STA $3F` is the byte
//! stored. **The power-on slice** is Stella's, slice 0.

use super::connector::Cartridge;

const SLICE: usize = 0x800;

pub struct Tigervision3F {
    rom: Vec<u8>,
    mask: usize,
    pub lower: usize,
    upper: usize,
    /// the last access had A12, A7 and A6 low
    armed: bool,
    /// the data bus at the end of the last access
    last: u8,
}

impl Tigervision3F {
    pub fn new(image: &[u8], start_slice: usize) -> Result<Self, String> {
        let slices = image.len() / SLICE;
        if image.len() % SLICE != 0 || slices < 2 || slices & (slices - 1) != 0 {
            return Err(format!(
                "3F needs a power of two 2K slices, not {} bytes",
                image.len()
            ));
        }
        Ok(Tigervision3F {
            rom: image.to_vec(),
            mask: slices - 1,
            lower: start_slice * SLICE,
            upper: (slices - 1) * SLICE,
            armed: false,
            last: 0,
        })
    }
}

impl Cartridge for Tigervision3F {
    fn name(&self) -> &'static str {
        "3F"
    }
    fn watches_bus(&self) -> bool {
        true
    }
    fn access(&mut self, address: u16, _value: u8, _t: i64) -> Option<u8> {
        if address & 0x1000 == 0 {
            return None;
        }
        if self.armed {
            self.lower = (self.last as usize & self.mask) * SLICE;
        }
        let offset = if address & 0x800 != 0 {
            self.upper
        } else {
            self.lower
        };
        Some(self.rom[offset | (address & 0x7FF) as usize])
    }
    fn bus(&mut self, address: u16, value: u8, _t: i64) {
        self.armed = address & 0x10C0 == 0;
        self.last = value;
    }
}
