//! A Rust Atari 2600 board transcribed from
//! [a2600-python](https://github.com/alewman/a2600-python) at the tag
//! `a2600-rust-pin-1`, on [6502-rust](https://github.com/alewman/6502-rust).
//!
//! Each module is the transcription of one `a2600_python` module, names and
//! hardware comments carried over; where Rust forced a difference the module
//! header says so.

pub mod audio;
pub mod board;
pub mod cartridge;
pub mod controllers;
pub mod machine;
pub mod palettes;
pub mod riot;
pub mod system;
pub mod television;
pub mod tia;
pub mod trace;

pub use machine::{Machine, MACHINES, VCS_NTSC, VCS_PAL, VCS_SECAM};
pub use system::{Mos6507, System};
