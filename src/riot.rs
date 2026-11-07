//! The MOS 6532 RIOT: 128 bytes of RAM, two 8-bit ports, the interval timer
//! (`riot.py`).
//!
//! **Decode** (6532 data sheet): the board selects the RIOT when A12 = 0 and
//! A7 = 1. Then A9 = 0 is RAM (A0-A6). A9 = 1 is I/O: with A2 = 0, A1-A0 pick
//! SWCHA, SWACNT, SWCHB, SWBCNT; with A2 = 1, a read with A0 = 0 is INTIM and
//! with A0 = 1 the interrupt flags, a write with A4 = 1 loads the timer (A1-A0
//! the interval, A3 its interrupt enable) and with A4 = 0 sets the PA7 edge
//! (A0 = 1 positive). The IRQ pin is not connected on the 2600.
//!
//! **The timer** is evaluated lazily, in CPU cycles (one phi2 per cycle):
//!
//! - A write loads the counter and selects the interval; the first decrement
//!   comes one cycle after the write, then one every interval.
//! - When it decrements past 0 it reads $FF, sets the timer flag (TIMINT D7),
//!   and from then on decrements every cycle.
//! - Reading INTIM clears the flag and the counter goes back to the interval,
//!   continuing from the prescaler's current phase (Stella issue #97, C7).
//! - A read of INTIM, or a write to the timer, in the very cycle the counter
//!   wraps does not clear the flag (MCS6500 Family Hardware Manual p.78;
//!   Stella issue #606's console results, C6).
//!
//! Differences forced by Rust: the reference's `port_a_device` callable is
//! the [`PortA`] trait, an object the RIOT owns.

pub const TIMER_FLAG: u8 = 0x80;
pub const PA7_FLAG: u8 = 0x40;
const INTERVALS: [u32; 4] = [1, 8, 64, 1024];

/// A port A device that drives lines itself (SaveKey's EEPROM pulls SDA
/// low): called with the levels the RIOT and the controllers drive after each
/// SWCHA or SWACNT write, it returns the bits it pulls low as 0s.
pub trait PortA {
    fn lines(&mut self, levels: u8, cycle: i64) -> u8;
}

pub struct Riot {
    pub ram: [u8; 128],
    pub out_a: u8,
    pub ddr_a: u8,
    pub out_b: u8,
    pub ddr_b: u8,
    /// The pins' input levels, set by the controllers and console switches:
    /// port A = left joystick in D7-D4, right in D3-D0 (active low).
    pub input_a: u8,
    pub input_b: u8,
    pub port_a_device: Option<Box<dyn PortA>>,
    pub device_a: u8,
    pub flags: u8,
    pub edge_positive: bool,
    pa7: bool,
    // The timer: the counter, the interval, the cycles left until the
    // prescaler's next decrement (1..divider), the cycle the state is as of,
    // and the cycle the counter last wrapped past 0 (-1: never).
    pub value: u8,
    pub divider: u32,
    pub countdown: i64,
    pub as_of: i64,
    pub wrapped_at: i64,
}

impl Riot {
    pub fn new(ram_fill: u8, timer_value: u8, timer_divider: u32, first_decrement: i64) -> Self {
        Riot {
            ram: [ram_fill; 128],
            out_a: 0,
            ddr_a: 0,
            out_b: 0,
            ddr_b: 0,
            input_a: 0xFF,
            input_b: 0x3F,
            port_a_device: None,
            device_a: 0xFF,
            flags: 0,
            edge_positive: false,
            pa7: true,
            value: timer_value,
            divider: timer_divider,
            countdown: first_decrement,
            as_of: 0,
            wrapped_at: -1,
        }
    }

    // -- the timer ---------------------------------------------------------

    fn catch_up(&mut self, cycle: i64) {
        let mut n = cycle - self.as_of;
        if n <= 0 {
            return;
        }
        self.as_of = cycle;
        let divider = self.divider as i64;
        if self.flags & TIMER_FLAG == 0 {
            let countdown = self.countdown;
            if n < countdown {
                self.countdown = countdown - n;
                return;
            }
            // The decrement due after `countdown` cycles, then one per interval.
            n -= countdown;
            let steps = 1 + n / divider;
            self.countdown = divider - n % divider;
            let value = self.value as i64;
            if steps <= value {
                self.value = (value - steps) as u8;
                return;
            }
            // Past 0: the wrap happens on the (value + 1)th decrement, at the
            // cycle below; every cycle after it decrements once more.
            let wrap = cycle - n + value * divider;
            self.flags |= TIMER_FLAG;
            self.wrapped_at = wrap + (cycle - wrap).div_euclid(256) * 256; // and every 256 after
            self.value = ((0xFF - (cycle - wrap)) & 0xFF) as u8;
            return;
        }
        // Wrapped: one decrement per cycle, the prescaler still turning; the
        // counter wraps again whenever it passes 0. The k-th of these n
        // decrements wraps when k = value + 1 (mod 256).
        let value = self.value as i64;
        if n > value {
            let last = n - (n - value - 1).rem_euclid(256);
            self.wrapped_at = cycle - n + last;
        }
        self.value = ((value - n).rem_euclid(256)) as u8;
        self.countdown = divider - (divider - self.countdown + n).rem_euclid(divider);
    }

    /// INTIM as of `cycle`, without a read's side effect (for debuggers and detectors).
    pub fn timer_at(&mut self, cycle: i64) -> u8 {
        self.catch_up(cycle);
        self.value
    }

    fn load_timer(&mut self, value: u8, divider: u32, cycle: i64) {
        self.value = value;
        self.divider = divider;
        self.countdown = 1;
        self.as_of = cycle;
        if self.wrapped_at != cycle {
            self.flags &= !TIMER_FLAG;
        }
    }

    // -- the bus -----------------------------------------------------------

    pub fn read(&mut self, address: u16, cycle: i64) -> u8 {
        if address & 0x200 == 0 {
            return self.ram[(address & 0x7F) as usize];
        }
        self.catch_up(cycle);
        if address & 0x04 == 0 {
            return match address & 0x03 {
                // Wired-AND: an output bit drives low only where SWACNT makes it an output.
                0 => (self.out_a | !self.ddr_a) & self.input_a & self.device_a,
                1 => self.ddr_a,
                2 => (self.out_b & self.ddr_b) | (self.input_b & !self.ddr_b),
                _ => self.ddr_b,
            };
        }
        if address & 0x01 == 0 {
            if self.wrapped_at != cycle {
                self.flags &= !TIMER_FLAG;
            }
            return self.value;
        }
        let flags = self.flags;
        self.flags &= !PA7_FLAG;
        flags
    }

    pub fn write(&mut self, address: u16, value: u8, cycle: i64) {
        if address & 0x200 == 0 {
            self.ram[(address & 0x7F) as usize] = value;
            return;
        }
        self.catch_up(cycle);
        if address & 0x04 == 0 {
            let register = address & 0x03;
            match register {
                0 => self.out_a = value,
                1 => self.ddr_a = value,
                2 => self.out_b = value,
                _ => self.ddr_b = value,
            }
            if register < 2 {
                if let Some(device) = &mut self.port_a_device {
                    let levels = (self.out_a | !self.ddr_a) & self.input_a;
                    self.device_a = device.lines(levels, cycle);
                }
            }
            self.edge_detect();
        } else if address & 0x10 != 0 {
            self.load_timer(value, INTERVALS[(address & 0x03) as usize], cycle);
        } else {
            self.edge_positive = address & 0x01 != 0;
        }
    }

    /// The port A pins as the controllers drive them (active low).
    pub fn set_input_a(&mut self, value: u8) {
        self.input_a = value;
        self.edge_detect();
    }

    fn edge_detect(&mut self) {
        let pa7 = (self.out_a | !self.ddr_a) & self.input_a & self.device_a & 0x80 != 0;
        if pa7 != self.pa7 && pa7 == self.edge_positive {
            self.flags |= PA7_FLAG;
        }
        self.pa7 = pa7;
    }
}
