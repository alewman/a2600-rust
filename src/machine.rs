//! The consoles, as data: one board, several machines (`machine.py`).
//!
//! Every console modelled is the same board with a different TIA (colour
//! encoding, crystal) and different power-on state, so each is a row of
//! [`MACHINES`]; adding PAL or SECAM is adding a row, not a code path. The
//! reference is the NTSC console with the visual6502-photographed 10444D
//! TIA, the chip Sim2600 models.

/// A video standard's clocks.
///
/// The TIA's oscillator is the colour clock; the 6507 runs at a third of it
/// and a line is always 228 colour clocks, 76 CPU cycles. The number of lines
/// is the program's (there is no vertical counter); `nominal_lines` is only
/// what well-behaved games produce, for the television stage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    pub name: &'static str,
    pub colour_clock_hz: f64,
    pub nominal_lines: u32,
}

impl Timing {
    pub fn cpu_hz(&self) -> f64 {
        self.colour_clock_hz / 3.0
    }
}

// Console schematics and the Stella Programmer's Guide (specification tier).
pub const NTSC: Timing = Timing {
    name: "NTSC",
    colour_clock_hz: 315e6 / 88.0,
    nominal_lines: 262,
};
pub const PAL: Timing = Timing {
    name: "PAL",
    colour_clock_hz: 3_546_894.0,
    nominal_lines: 312,
};
pub const SECAM: Timing = Timing {
    name: "SECAM",
    colour_clock_hz: 3_546_894.0,
    nominal_lines: 312,
};

/// What a console holds when it is switched on; real consoles vary.
///
/// On hardware nearly all of this is undefined: the 6507's A/X/Y/S, RIOT
/// RAM, the interval timer, the TIA's latches and the phase of its horizontal
/// counter against the CPU's first cycle. The defaults are chosen to match
/// MAME 0.285's power-on state, as the reference's are (its machine.py
/// documents each); `hclock` must be 2 mod 3, on the CPU cycle grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PowerOn {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub s: u8,
    pub p: u8,
    pub ram_fill: u8,
    pub timer_value: u8,
    pub timer_divider: u32,
    pub timer_first_decrement: i64,
    pub hclock: u32,
}

impl Default for PowerOn {
    fn default() -> Self {
        PowerOn {
            a: 0,
            x: 0x80,
            y: 0,
            s: 0,
            p: 0x26,
            ram_fill: 0x00,
            timer_value: 0xFF,
            timer_divider: 1024,
            timer_first_decrement: 1023,
            hclock: 224,
        }
    }
}

/// P0 P1 M0 M1 BL as bits 4..0, the TIA's order.
pub const ALL_OBJECTS: u8 = 0x1F;
pub const P0: u8 = 0x10;
pub const P1: u8 = 0x08;
pub const M0: u8 = 0x04;
pub const M1: u8 = 0x02;
pub const BL: u8 = 0x01;

/// How one console's TIA departs from the reference 10444D, field by field.
///
/// A field exists only where a hardware capture shows a console differing
/// from the reference; each defaults to the 10444D, whose die Sim2600 models.
///
/// - `stuffed_pulse_kept`: the objects whose HMOVE pulse is kept when it
///   arrives during the RESxx strobe itself, stepping the ring once the hold
///   lets go (C28, the `hmove_resp0` die trace); one arriving in the hold
///   that follows is kept on every console. ale-79's serial tables (Stella
///   #699) show four PAL consoles losing it for the missiles and the light
///   sixer for the players too.
/// - `merged_pulse_steps`: the objects that an HMOVE pulse falling outside
///   HBLANK (merging with MOTCK) steps early before it absorbs their next
///   MOTCK, for no net movement (the die). On the PAL 7800 and Jr. '91 such a
///   pulse only absorbs the MOTCK: each costs the object a clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Revision {
    pub name: &'static str,
    pub stuffed_pulse_kept: u8,
    pub merged_pulse_steps: u8,
}

pub const REFERENCE: Revision = Revision {
    name: "10444D (reference)",
    stuffed_pulse_kept: ALL_OBJECTS,
    merged_pulse_steps: ALL_OBJECTS,
};

/// The consoles of ale-79's #699 tables, as they differ from the reference.
pub const REVISIONS: [Revision; 7] = [
    REFERENCE,
    Revision {
        name: "PAL 7800 (#699)",
        merged_pulse_steps: 0,
        ..REFERENCE
    },
    Revision {
        name: "SECAM woody (#699)",
        ..REFERENCE
    },
    Revision {
        name: "PAL Jr. '84 (#699)",
        stuffed_pulse_kept: P0 | P1 | BL,
        ..REFERENCE
    },
    Revision {
        name: "PAL Jr. '91 (#699)",
        stuffed_pulse_kept: P0 | P1 | BL,
        merged_pulse_steps: 0,
    },
    Revision {
        name: "PAL Vader (#699)",
        stuffed_pulse_kept: P0 | P1 | BL,
        ..REFERENCE
    },
    Revision {
        name: "PAL light sixer (#699)",
        stuffed_pulse_kept: BL,
        ..REFERENCE
    },
];

pub fn revision(name: &str) -> Option<Revision> {
    REVISIONS.iter().copied().find(|r| r.name == name)
}

/// One console model.
///
/// `switches` is SWCHB's input bits as the console presents them with no
/// switch pressed: Reset (D0) and Select (D1) released, Colour (D3), both
/// difficulties on B (D6, D7 low), and the unconnected D2/D4/D5 reading
/// high. $3F is what MAME and Stella both present.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Machine {
    pub name: &'static str,
    pub timing: Timing,
    pub certified: bool,
    pub power_on: PowerOn,
    pub switches: u8,
    pub revision: Revision,
}

impl Machine {
    const fn row(name: &'static str, timing: Timing) -> Self {
        Machine {
            name,
            timing,
            certified: false,
            power_on: PowerOn {
                a: 0,
                x: 0x80,
                y: 0,
                s: 0,
                p: 0x26,
                ram_fill: 0,
                timer_value: 0xFF,
                timer_divider: 1024,
                timer_first_decrement: 1023,
                hclock: 224,
            },
            switches: 0x3F,
            revision: REFERENCE,
        }
    }
}

pub const VCS_NTSC: Machine = Machine::row("Atari VCS (NTSC)", NTSC);
pub const VCS_PAL: Machine = Machine::row("Atari VCS (PAL)", PAL);
pub const VCS_SECAM: Machine = Machine::row("Atari VCS (SECAM)", SECAM);
pub const MACHINES: [Machine; 3] = [VCS_NTSC, VCS_PAL, VCS_SECAM];

/// A machine by its name ("Atari VCS (PAL)") or its standard ("pal").
pub fn machine(name: &str) -> Option<Machine> {
    MACHINES
        .iter()
        .copied()
        .find(|m| m.name == name || m.timing.name.eq_ignore_ascii_case(name))
}
