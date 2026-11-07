//! A whole console: 6502-rust's `Mos6502`, as the 6507, on a [`Board`]
//! (`system.py`).
//!
//! The 6507 is the 6502 die with A13-A15, IRQ and NMI not bonded out: the
//! board masks every address to 13 bits and never requests an interrupt, and
//! nothing else differs. The CPU keeps its 16-bit PC, as MAME and Stella show
//! it.

use sixfiveohtwo::Mos6502;

use crate::board::Board;
use crate::cartridge::{self, Cartridge, Options};
use crate::machine::{Machine, VCS_NTSC};

/// The 6507: a 6502 whose board decodes 13 address lines and wires no interrupts.
pub type Mos6507 = Mos6502<Board>;

/// Machine + cartridge, powered on.
pub struct System {
    pub machine: Machine,
    pub cpu: Mos6507,
    pub instructions: u64,
    /// The scheme as built (e.g. "F6SC"), and how it was chosen.
    pub scheme: String,
    pub source: &'static str,
    pub title: Option<String>,
}

impl System {
    pub fn new(cartridge: Box<dyn Cartridge>, machine: Machine) -> Result<Self, String> {
        let scheme = cartridge.name().to_owned();
        let board = Board::new(machine, cartridge)?;
        let mut system = System {
            machine,
            cpu: Mos6502::new(board),
            instructions: 0,
            scheme,
            source: "explicit",
            title: None,
        };
        system.power();
        Ok(system)
    }

    /// Load `image` (identified as `cartridge::load` does). A scheme with
    /// its own clock in seconds (the DPC's oscillator, the Supercharger's
    /// tape) is given the machine's CPU clock.
    pub fn from_rom(
        image: &[u8],
        machine: Machine,
        scheme: Option<&str>,
        options: &Options,
    ) -> Result<Self, String> {
        let mut options = options.clone();
        options.cpu_hz = Some(machine.timing.cpu_hz());
        let loaded = cartridge::load(image, scheme, None, cartridge::MAME_HASH, &options)?;
        let mut system = System::new(loaded.cartridge, machine)?;
        system.scheme = loaded.scheme;
        system.source = loaded.source;
        system.title = loaded.title;
        Ok(system)
    }

    pub fn ntsc(image: &[u8]) -> Result<Self, String> {
        System::from_rom(image, VCS_NTSC, None, &Options::default())
    }

    /// Switch on: registers from the machine's power-on parameters, then the reset sequence.
    pub fn power(&mut self) {
        let power = self.machine.power_on;
        let cpu = &mut self.cpu;
        cpu.a = power.a;
        cpu.x = power.x;
        cpu.y = power.y;
        cpu.s = power.s;
        cpu.p = power.p;
        cpu.frame.request_reset();
    }

    pub fn board(&self) -> &Board {
        &self.cpu.bus
    }

    pub fn board_mut(&mut self) -> &mut Board {
        &mut self.cpu.bus
    }

    // -- running ---------------------------------------------------------

    pub fn cycles(&self) -> i64 {
        self.cpu.bus.cycles
    }

    /// One instruction (or the reset sequence); returns CPU cycles, halted ones included.
    pub fn step(&mut self) -> i64 {
        let before = self.cpu.bus.cycles;
        self.cpu.step();
        self.instructions += 1;
        self.cpu.bus.cycles - before
    }

    /// Run whole instructions until at least `cycles` more CPU cycles have passed.
    pub fn run_cycles(&mut self, cycles: i64) {
        let end = self.cpu.bus.cycles + cycles;
        while self.cpu.bus.cycles < end {
            self.cpu.step();
            self.instructions += 1;
        }
    }

    /// Run until the program has started `frames` more frames (VSYNC rising edges).
    pub fn run_frames(&mut self, frames: u64) {
        let target = self.cpu.bus.tia.frame + frames;
        while self.cpu.bus.tia.frame < target {
            self.cpu.step();
            self.instructions += 1;
        }
    }

    /// Run until `done()` (checked after every instruction); false on timeout.
    pub fn run_until(&mut self, mut done: impl FnMut(&System) -> bool, max_cycles: i64) -> bool {
        let end = self.cpu.bus.cycles + max_cycles;
        while self.cpu.bus.cycles < end {
            self.cpu.step();
            self.instructions += 1;
            if done(self) {
                return true;
            }
        }
        false
    }
}
