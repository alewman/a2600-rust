//! The controllers: joystick, paddles, keypads, driving controller, Booster
//! Grip, SaveKey (`controllers.py`).
//!
//! A port's nine pins reach the console as four RIOT port A bits (pins 1-4:
//! up, down, left, right; left port D7-D4, right port D3-D0), the fire
//! button (pin 6) on INPT4/INPT5, and the two pot pins (9 and 5) on
//! INPT0/INPT1 (left) or INPT2/INPT3 (right).
//!
//! **Paddles**: each of I0-I3 has a 1,800 Ohm resistor and a 0.068 uF
//! capacitor to ground on the console board, charged through the paddle's
//! pot; VBLANK D7 grounds the pads, and from its release the pad reads 1 once
//! the voltage passes the trip point. **A fit, labelled**: the pot taken as 0
//! to 1 MOhm, the trip point as half the supply. **Keypads**: twelve keys in
//! four rows driven from SWCHA and three columns read on the pads.
//!
//! Differences forced by Rust: the reference installs devices as callables
//! (`TIA.pad_source`, `RIOT.port_a_device`); here the board owns them,
//! [`Pads`] asked for I0-I5 when the 6507 reads a pad, the SaveKey as the
//! RIOT's [`PortA`] device.

use crate::riot::{PortA, Riot};
use crate::tia::Tia;

pub const R_SERIES: f64 = 1800.0; // ohms, console board
pub const CAPACITANCE: f64 = 0.068e-6; // farads, console board
pub const POT_MAX: f64 = 1.0e6; // ohms, a fit
pub const TRIP: f64 = 0.5; // of the supply, a fit

/// Port A bits per port: (up, down, left, right).
pub const PORT_BITS: [[u8; 4]; 2] = [[0x10, 0x20, 0x40, 0x80], [0x01, 0x02, 0x04, 0x08]];

/// Four paddles on I0-I3; `position[i]` from 0.0 (no resistance) to 1.0
/// (POT_MAX). `None` leaves a pad to `Tia::inputs`: 0 on the board, as a pad
/// with no pot never charges (C19).
#[derive(Clone, Debug)]
pub struct Paddles {
    pub position: [Option<f64>; 4],
    seconds_per_step: f64,
    trip: f64,
}

impl Paddles {
    pub fn new(colour_clock_hz: f64) -> Self {
        Paddles {
            position: [None; 4],
            seconds_per_step: 1.0 / (2.0 * colour_clock_hz),
            trip: -(1.0 - TRIP).ln(),
        }
    }

    /// Half colour clocks from the dump's release until pad `i` reads 1.
    pub fn trip_steps(&self, i: usize) -> f64 {
        let resistance = R_SERIES + POT_MAX * self.position[i].unwrap_or(0.0);
        resistance * CAPACITANCE * self.trip / self.seconds_per_step
    }

    pub fn levels(&self, tia: &Tia) -> u8 {
        let mut levels = tia.inputs;
        if tia.vblank & 0x80 != 0 {
            return levels;
        }
        let elapsed = (tia.now() - tia.dump_released) as f64;
        for i in 0..4 {
            if self.position[i].is_some() {
                if elapsed >= self.trip_steps(i) {
                    levels |= 1 << i;
                } else {
                    levels &= !(1 << i);
                }
            }
        }
        levels
    }
}

/// Two 12-key keypads; `pressed[port]` holds (row, column), 0-3 x 0-2.
#[derive(Clone, Debug, Default)]
pub struct Keypads {
    pub pressed: [Vec<(u8, u8)>; 2],
}

impl Keypads {
    const COLUMN_PADS: [[u8; 3]; 2] = [[0, 1, 4], [2, 3, 5]];

    pub fn levels(&self, tia: &Tia, riot: &Riot) -> u8 {
        let driven_low = riot.ddr_a & !riot.out_a;
        let mut levels = tia.inputs;
        for port in 0..2 {
            let shift = if port == 0 { 4 } else { 0 };
            for &(row, column) in &self.pressed[port] {
                if (driven_low >> (shift + row)) & 1 != 0 {
                    levels &= !(1 << Self::COLUMN_PADS[port][column as usize]);
                }
            }
        }
        levels
    }
}

/// The CBS Booster Grip's two extra buttons on a port's pot pins: pressed, a
/// button ties its pin to +5 V, so the pad reads 1 unless VBLANK D7 is
/// grounding it; released, it reads 0, as with no paddle (C19). The trigger
/// is on pin 5 and the booster on pin 9: INPT1 and INPT0 on the left port,
/// INPT3 and INPT2 on the right.
#[derive(Clone, Debug)]
pub struct BoosterButtons {
    pub booster: bool,
    pub trigger: bool,
    /// pin 9, pin 5
    pads: (u8, u8),
}

/// The device answering for I0-I5 at a pad read (the reference's
/// `TIA.pad_source`), chained as the reference chains a Booster Grip onto
/// the source before it.
#[derive(Clone, Debug, Default)]
pub enum Pads {
    #[default]
    None,
    Paddles(Paddles),
    Keypads(Keypads),
    Booster(BoosterButtons, Box<Pads>),
}

impl Pads {
    pub fn levels(&self, tia: &Tia, riot: &Riot) -> u8 {
        match self {
            Pads::None => tia.inputs,
            Pads::Paddles(paddles) => paddles.levels(tia),
            Pads::Keypads(keypads) => keypads.levels(tia, riot),
            Pads::Booster(buttons, previous) => {
                let mut levels = previous.levels(tia, riot);
                for (pad, pressed) in [
                    (buttons.pads.0, buttons.booster),
                    (buttons.pads.1, buttons.trigger),
                ] {
                    if pressed && tia.vblank & 0x80 == 0 {
                        levels |= 1 << pad;
                    } else {
                        levels &= !(1 << pad);
                    }
                }
                levels
            }
        }
    }
}

/// A joystick's four switches and its button, pressed or not.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stick {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub fire: bool,
}

/// A joystick on `port` (0 left, 1 right): four switches and a button.
pub fn joystick(riot: &mut Riot, tia: &mut Tia, port: usize, stick: Stick) {
    let bits = PORT_BITS[port];
    let mut level = riot.input_a | bits.iter().fold(0, |a, b| a | b);
    for (pressed, bit) in [stick.up, stick.down, stick.left, stick.right]
        .into_iter()
        .zip(bits)
    {
        if pressed {
            level &= !bit;
        }
    }
    riot.set_input_a(level);
    tia.fire[port] = stick.fire;
}

/// The CX20 driving controller (Indy 500): a 2-bit Gray code on pins 1 and 2.
///
/// Turning the knob steps the code through 3, 1, 0, 2 one way and back the
/// other (pins 1 and 2 as bits 0 and 1). Which way is clockwise is not
/// established; `turn(+1)` steps the sequence forward. The button is fire.
#[derive(Clone, Debug)]
pub struct Driving {
    pub port: usize,
    pub index: usize,
}

impl Driving {
    const GRAY: [u8; 4] = [0b11, 0b01, 0b00, 0b10];

    pub fn new(riot: &mut Riot, port: usize) -> Self {
        let driving = Driving { port, index: 0 };
        driving.apply(riot);
        driving
    }

    pub fn turn(&mut self, riot: &mut Riot, steps: i64) {
        self.index = (self.index as i64 + steps).rem_euclid(4) as usize;
        self.apply(riot);
    }

    pub fn press(&self, tia: &mut Tia, fire: bool) {
        tia.fire[self.port] = fire;
    }

    fn apply(&self, riot: &mut Riot) {
        let [up, down, _, _] = PORT_BITS[self.port];
        let gray = Self::GRAY[self.index];
        let mut level = riot.input_a | up | down;
        if gray & 1 == 0 {
            level &= !up;
        }
        if gray & 2 == 0 {
            level &= !down;
        }
        riot.set_input_a(level);
    }
}

/// Plug a Booster Grip's extra buttons onto `port`, chained onto `pads`.
pub fn booster_grip(pads: Pads, port: usize) -> Pads {
    let buttons = BoosterButtons {
        booster: false,
        trigger: false,
        pads: if port == 0 { (0, 1) } else { (2, 3) },
    };
    Pads::Booster(buttons, Box::new(pads))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    Address,
    High,
    Low,
    Write,
    Read,
}

/// Richard Hutchinson's SaveKey: a 24LC256 (32K I2C EEPROM) on pins 3 and 4.
///
/// SDA is pin 3 and SCL pin 4: SWCHA D2 and D3 on the right port, D6 and D7
/// on the left. The EEPROM follows Microchip's 24LC256 data sheet (DS21203):
/// START, STOP, MSB-first bytes sampled on SCL's rise, the receiver
/// acknowledging in a ninth clock, control byte 1010 000 R/W, a write's two
/// address bytes (15 bits used) and up to 64 data bytes wrapping within their
/// page, stored at the STOP, then a 5 ms write cycle (the data sheet's
/// maximum); a read streams until the master does not acknowledge.
#[derive(Clone, Debug)]
pub struct SaveKey {
    pub data: Vec<u8>,
    sda_bit: u8,
    scl_bit: u8,
    write_cycles: i64,
    scl: u8,
    sda: u8,
    /// the chip's own SDA: 0 pulls the line low
    drive: u8,
    phase: Phase,
    /// bits of the current byte
    count: u32,
    byte: u8,
    /// in the ninth clock of a byte
    acking: bool,
    pub pointer: usize,
    pending: Vec<u8>,
    busy_until: i64,
}

impl SaveKey {
    pub const SIZE: usize = 0x8000;
    pub const PAGE: usize = 64;

    pub fn new(port: usize, data: Option<Vec<u8>>, cpu_hz: f64) -> Result<Self, String> {
        let data = data.unwrap_or_else(|| vec![0xFF; Self::SIZE]);
        if data.len() != Self::SIZE {
            return Err(format!(
                "a SaveKey holds {} bytes, not {}",
                Self::SIZE,
                data.len()
            ));
        }
        let [_, _, sda_bit, scl_bit] = PORT_BITS[port];
        Ok(SaveKey {
            data,
            sda_bit,
            scl_bit,
            write_cycles: (0.005 * cpu_hz) as i64,
            scl: 1,
            sda: 1,
            drive: 1,
            phase: Phase::Idle,
            count: 0,
            byte: 0,
            acking: false,
            pointer: 0,
            pending: Vec::new(),
            busy_until: -1,
        })
    }

    fn receiving(&self) -> bool {
        matches!(
            self.phase,
            Phase::Address | Phase::High | Phase::Low | Phase::Write
        )
    }

    fn start(&mut self) {
        self.phase = Phase::Address;
        self.count = 0;
        self.byte = 0;
        self.acking = false;
        self.drive = 1;
        self.pending.clear();
    }

    fn stop(&mut self, cycle: i64) {
        if self.phase == Phase::Write && !self.pending.is_empty() {
            let page = self.pointer & !(Self::PAGE - 1);
            let mut offset = self.pointer;
            for &value in &self.pending {
                // past 64 bytes the page wraps, overwriting
                self.data[page | (offset & (Self::PAGE - 1))] = value;
                offset += 1;
            }
            self.pointer = page | (offset & (Self::PAGE - 1));
            self.busy_until = cycle + self.write_cycles;
        }
        self.phase = Phase::Idle;
        self.drive = 1;
        self.acking = false;
        self.pending.clear();
    }

    fn rise(&mut self, sda: u8) {
        if self.acking {
            return;
        }
        if self.receiving() && self.count < 8 {
            self.byte = (self.byte << 1) | sda;
            self.count += 1;
        } else if self.phase == Phase::Read {
            if self.count < 8 {
                self.count += 1;
            } else if sda == 0 {
                // the master acknowledges: the next byte
                self.pointer = (self.pointer + 1) % Self::SIZE;
                self.acking = true;
            } else {
                self.phase = Phase::Idle;
            }
        }
    }

    fn fall(&mut self, cycle: i64) {
        if self.acking {
            self.acking = false;
            self.drive = 1;
            self.count = 0;
            self.byte = 0;
            if self.phase == Phase::Read {
                self.drive = (self.data[self.pointer] >> 7) & 1;
            }
            return;
        }
        if self.receiving() && self.count == 8 {
            self.acking = true;
            self.drive = if self.take(self.byte, cycle) { 0 } else { 1 };
        } else if self.phase == Phase::Read {
            self.drive = if self.count < 8 {
                (self.data[self.pointer] >> (7 - self.count)) & 1
            } else {
                1
            };
        }
    }

    /// A received byte; true to acknowledge it.
    fn take(&mut self, byte: u8, cycle: i64) -> bool {
        match self.phase {
            Phase::Address => {
                if byte >> 1 != 0b1010000 || cycle < self.busy_until {
                    self.phase = Phase::Idle;
                    return false;
                }
                self.phase = if byte & 1 != 0 {
                    Phase::Read
                } else {
                    Phase::High
                };
            }
            Phase::High => {
                self.pointer = ((byte as usize & 0x7F) << 8) | (self.pointer & 0xFF);
                self.phase = Phase::Low;
            }
            Phase::Low => {
                self.pointer = (self.pointer & 0x7F00) | byte as usize;
                self.phase = Phase::Write;
            }
            _ => self.pending.push(byte),
        }
        true
    }
}

impl PortA for SaveKey {
    /// SCL and SDA as the program leaves them; returns the bits the chip pulls low.
    fn lines(&mut self, levels: u8, cycle: i64) -> u8 {
        let scl = (levels & self.scl_bit != 0) as u8;
        let sda = (levels & self.sda_bit != 0) as u8;
        if scl != 0 && self.scl != 0 && sda != self.sda {
            if sda != 0 {
                self.stop(cycle);
            } else {
                self.start();
            }
        } else if scl != 0 && self.scl == 0 {
            self.rise(sda);
        } else if scl == 0 && self.scl != 0 {
            self.fall(cycle);
        }
        self.scl = scl;
        self.sda = sda;
        if self.drive != 0 {
            0xFF
        } else {
            !self.sda_bit
        }
    }
}
