//! The console board: the 13-bit bus, the floating data bus, WSYNC, and the
//! one clock (`board.py`).
//!
//! **One timeline.** The 6507 calls [`Bus::read`] or [`Bus::write`] exactly
//! once per CPU cycle, in chip order, dummy accesses included, so each call
//! *is* a CPU cycle: the TIA runs the cycle's six half colour clocks, and the
//! access happens on the sixth, where the 6507's Phi2 falls. A read takes the
//! TIA's D7/D6 as the die drives them after the fifth, while Phi2 is high.
//! `step()`'s returned count is never added on top.
//!
//! **The cycle grid is the TIA's**: CPU cycles end on steps 4, 10, 16, ...
//! of every line, so the power-on point must lie on that grid.
//!
//! **WSYNC as a stretched read.** A WSYNC write pulls RDY low; the NMOS 6502
//! ignores RDY on writes and halts on its next read, repeating it until RDY
//! rises. The board samples RDY on each read cycle's third step. A halted
//! cycle performs no access here (the halted read is always a cartridge-ROM
//! fetch on the 2600, so performing it once is exact).
//!
//! **Decode**: A12 high is the cartridge, else A7 low is the TIA, else the
//! RIOT. The cartridge sees every cycle, and a scheme that latches the data
//! bus sees each cycle's final byte as well. **The floating bus**:
//! [`Board::data_bus`] is the last byte on the data bus; TIA reads drive D7
//! and D6 only and the rest float.
//!
//! Differences forced by Rust: the board is the 6507's [`Bus`]; the
//! controllers' pads are [`Pads`], which the board owns; `bus_observers` is
//! one optional closure.

use sixfiveohtwo::{Bus, Frame};

use crate::cartridge::Cartridge;
use crate::controllers::Pads;
use crate::machine::Machine;
use crate::riot::Riot;
use crate::tia::Tia;

pub const CYCLE_STEPS: u32 = 6; // half colour clocks per CPU cycle

/// An observer of every CPU-bus cycle: (kind 'R' or 'W', address, value, cycle).
pub type BusObserver = Box<dyn FnMut(char, u16, u8, i64)>;

pub struct Board {
    pub machine: Machine,
    pub cartridge: Box<dyn Cartridge>,
    watch: bool,
    pub tia: Tia,
    pub riot: Riot,
    pub pads: Pads,
    /// CPU cycles since power, halted ones included
    pub cycles: i64,
    /// of which spent with RDY low
    pub halted_cycles: i64,
    pub data_bus: u8,
    pub bus_observer: Option<BusObserver>,
}

impl Board {
    pub fn new(machine: Machine, cartridge: Box<dyn Cartridge>) -> Result<Self, String> {
        let power = machine.power_on;
        if (2 * power.hclock) % CYCLE_STEPS != 4 {
            return Err("PowerOn.hclock must put power-on on the CPU cycle grid (2 mod 3)".into());
        }
        let mut tia = Tia::new(power.hclock);
        let revision = machine.revision;
        tia.revision = (revision.stuffed_pulse_kept, revision.merged_pulse_steps);
        // I0-I3 with joysticks (or nothing) plugged in: no pot, so nothing
        // charges the paddle capacitors and they read 0 (C19); I4/I5 have
        // pull-ups. controllers::Paddles supplies them when paddles are in.
        tia.inputs = 0x30;
        let mut riot = Riot::new(
            power.ram_fill,
            power.timer_value,
            power.timer_divider,
            power.timer_first_decrement,
        );
        riot.input_b = machine.switches;
        Ok(Board {
            machine,
            watch: cartridge.watches_bus(),
            cartridge,
            tia,
            riot,
            pads: Pads::None,
            cycles: 0,
            halted_cycles: 0,
            data_bus: 0,
            bus_observer: None,
        })
    }
}

impl Bus for Board {
    // -- one CPU cycle ---------------------------------------------------

    fn read(&mut self, address: u16, _frame: &mut Frame) -> u8 {
        loop {
            self.tia.step(None);
            self.tia.step(None);
            self.tia.step(None);
            if !self.tia.rdy_low {
                break;
            }
            // a halted cycle: the 6507 repeats this read
            self.tia.step(None);
            self.tia.step(None);
            self.tia.step(None);
            self.cycles += 1;
            self.halted_cycles += 1;
        }
        self.tia.step(None);
        self.tia.step(None);
        let address = address & 0x1FFF;
        let cycle = self.cycles;
        let driven = self.cartridge.access(address, self.data_bus, cycle);
        let value = if address & 0x1000 != 0 {
            driven.unwrap_or(self.data_bus)
        } else if address & 0x80 == 0 {
            let pads = if address & 0x0F >= 8 {
                self.pads.levels(&self.tia, &self.riot)
            } else {
                self.tia.inputs
            };
            self.tia.read_with(address as u8, pads) | (self.data_bus & 0x3F)
        } else {
            self.riot.read(address, cycle)
        };
        self.data_bus = value;
        if self.watch {
            self.cartridge.bus(address, value, cycle);
        }
        if let Some(observer) = &mut self.bus_observer {
            observer('R', address, value, cycle);
        }
        self.tia.step(None);
        self.cycles = cycle + 1;
        value
    }

    fn write(&mut self, address: u16, value: u8, _frame: &mut Frame) {
        for _ in 0..CYCLE_STEPS - 1 {
            self.tia.step(None);
        }
        let address = address & 0x1FFF;
        let cycle = self.cycles;
        self.data_bus = value;
        self.cartridge.access(address, value, cycle);
        if self.watch {
            self.cartridge.bus(address, value, cycle);
        }
        if address & 0x1000 != 0 {
            self.tia.step(None);
        } else if address & 0x80 == 0 {
            self.tia.step(Some((address as u8, value)));
        } else {
            self.riot.write(address, value, cycle);
            self.tia.step(None);
        }
        if let Some(observer) = &mut self.bus_observer {
            observer('W', address, value, cycle);
        }
        self.cycles = cycle + 1;
    }
}
