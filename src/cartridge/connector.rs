//! The cartridge connector, as signals: the only thing a scheme knows of the
//! console (`cartridge/connector.py`).
//!
//! - **Every CPU cycle**, whatever its address, reaches
//!   [`Cartridge::access`]: the slot carries A0-A12 always, and schemes
//!   watch TIA writes (3F) and stack accesses (FE) as well as their own
//!   window.
//! - **No R/W.** `value` is the byte on the data bus as the cartridge would
//!   see it before driving: the CPU's byte on a write cycle, the floating bus
//!   on a read cycle. A scheme decides by address alone, as the hardware
//!   must.
//! - **The return value** is the byte the cartridge drives, or `None` if it
//!   leaves the bus alone. The board uses it on read cycles only.
//! - `t` is the CPU cycle count at the access (the board's one timeline),
//!   for schemes with their own clock (DPC).
//! - **The data bus at the cycle's end.** A scheme that latches the data bus
//!   (FE, 3F) answers `watches_bus` and gets [`Cartridge::bus`] once per
//!   cycle, after `access`, with the byte the bus settled to.

pub trait Cartridge {
    fn name(&self) -> &'static str;

    /// One CPU cycle; `address` is A0-A12.
    fn access(&mut self, address: u16, value: u8, t: i64) -> Option<u8>;

    fn watches_bus(&self) -> bool {
        false
    }

    /// The cycle's end: `value` is the byte on the data bus (`watches_bus` only).
    fn bus(&mut self, _address: u16, _value: u8, _t: i64) {}
}

/// No cartridge: nothing driven, nothing remembered.
pub struct NoCartridge;

impl Cartridge for NoCartridge {
    fn name(&self) -> &'static str {
        "none"
    }
    fn access(&mut self, _address: u16, _value: u8, _t: i64) -> Option<u8> {
        None
    }
}
