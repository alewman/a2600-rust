//! The TIA's two sound channels, as the die wires them (`audio.py`).
//!
//! Each channel is three pieces of logic (TIA-1A logic diagram, sheet 4,
//! "Audio circuits (2 req'd)"):
//!
//! - the **frequency divider**: a 5-bit binary counter *compared* with AUDF,
//!   not loaded from it. A match enables the channel's next tick and clears
//!   the counter; otherwise it counts on, wrapping after 31, so lowering AUDF
//!   below the count lets it run on to 31 first (tia-fpga "Audio");
//! - the **noise generator**: a 5-bit shift register, which AUDC D1-D0 make a
//!   5-bit polynomial, the low end of a 9-bit one with the pulse counter, a
//!   gate that holds the pulse counter, or a plain divider;
//! - the **pulse counter**: four set/reset latches that shift as a 4-bit
//!   polynomial, a Johnson counter (divide by 6 or 2) or a follower of the
//!   noise register's output, picked by AUDC D3-D2. Its low bit is the tone;
//!   the volume register gates the four weighted pull-downs of the pad with it.
//!
//! **Ticks.** The horizontal counter clocks both channels twice a line. Each
//! tick has two phases (tia-fpga sim/README.md "Audio"): phase A compares the
//! divider and, on an enabled tick, latches the hold decision and the noise
//! feedback from AUDC as it then stands; phase B shifts the registers, and the
//! pads change there. Channel 0's phase A acts on the enable it latches in
//! that same phase, channel 1's on the enable from the phase A before.
//!
//! **Output.** [`Audio::take_samples`] returns one float per tick, the pads'
//! level at phase B, two a line. An unmodified console joins the two pads: a
//! 1 kOhm pull-up against each pad's pull-downs, weighted 30k, 15k, 7.5k and
//! 3.75k. With v the sum of the two pads' levels the pin falls by v/(30 + v)
//! of the supply; [`mix`] scales that to 0..1 at v = 30. **A fit**: the
//! transistors are taken as ideal switches and no 2600's audio has been
//! recorded to check it.
//!
//! **No lockup guards.** The pulse counter can reach %0000 and %1010 and the
//! netlist has no term to steer it away (tia-fpga). In AUDC C-F that makes
//! %0000 and %0001 step to %1111, which then alternates with %0000, a
//! divide-by-two; the die does exactly that in the `audio` trace (C26).

#[derive(Clone, Debug)]
pub struct Channel {
    pub audc: u8,
    pub audf: u8,
    pub audv: u8,
    pub divider: u8,
    pub enable: bool,
    pub hold: bool,
    /// 5 bits; bit 0 is the cell that feeds out.
    pub noise: u8,
    /// The feedback latched in phase A.
    pub noise_in: bool,
    /// The noise register's output, latched in phase A.
    pub noise_out: bool,
    /// 4 bits; bit 0 is the tone.
    pub pulse: u8,
    same_phase: bool,
}

impl Channel {
    fn new(same_phase: bool) -> Self {
        Channel {
            audc: 0,
            audf: 0,
            audv: 0,
            divider: 0,
            enable: false,
            hold: false,
            noise: 0,
            noise_in: false,
            noise_out: false,
            pulse: 0,
            same_phase,
        }
    }

    fn phase_a(&mut self) {
        let matched = self.divider == self.audf;
        if self.same_phase {
            self.enable = matched;
        }
        if self.enable {
            self.latch();
        }
        if !self.same_phase {
            self.enable = matched;
        }
        self.divider = if matched || self.divider == 31 {
            0
        } else {
            self.divider + 1
        };
    }

    fn latch(&mut self) {
        let (noise, pulse, audc) = (self.noise, self.pulse, self.audc);
        self.noise_out = noise & 1 != 0;
        let mode = audc & 3;
        self.hold = match mode {
            2 => (noise & 0x1E) != 0x02, // the pulse counter steps once per 31 noise steps
            3 => noise & 1 == 0,         // it steps when the 5-bit polynomial's output is 1
            _ => false,
        };
        self.noise_in = if mode == 0 {
            // The noise register follows the pulse counter: the low half of a
            // 9-bit polynomial when AUDC D3-D2 are 0 too (AUDC 0 and 8), with
            // the all-zeros escape both registers need to start.
            (pulse ^ noise) & 1 != 0 || (noise == 0 && pulse == 0x0A) || audc & 0x0C == 0
        } else {
            // a 5-bit polynomial, x^5 + x^3 + 1, which escapes all-zeros
            ((noise >> 2) ^ noise) & 1 != 0 || noise == 0
        };
    }

    fn phase_b(&mut self) {
        if !self.enable {
            return;
        }
        let pulse = self.pulse;
        let feed = match self.audc >> 2 {
            // 4-bit polynomial; AUDC 0 has no feedback and fills with ones
            0 => ((pulse >> 1) ^ pulse) & 1 != 0 && self.audc & 3 != 0,
            // Johnson counter: divide by 6 (the tone of AUDC 4/5)
            1 => pulse & 0x08 == 0,
            // follow the noise register (AUDC 8-B)
            2 => !self.noise_out,
            // Divide by 6, from any state but %0000 and %0001: those step to
            // %1111 and then alternate with %0000, dividing by 2 (the die
            // steps %0000 to %1111 in AUDC C; tia-fpga "Audio").
            _ => pulse & 0x02 == 0,
        };
        self.noise = (self.noise >> 1) | if self.noise_in { 0x10 } else { 0 };
        if !self.hold {
            self.pulse = ((!pulse >> 1) & 0x07) | if feed { 0x08 } else { 0 };
        }
    }

    /// The pad's weighted pull-downs: AUDV while the tone is high, else 0.
    pub fn level(&self) -> u8 {
        if self.pulse & 1 != 0 {
            self.audv
        } else {
            0
        }
    }
}

/// The joined pads' swing for a summed level `v` (0-30), as 0..1 (a fit).
pub fn mix(v: u8) -> f64 {
    2.0 * v as f64 / (30.0 + v as f64)
}

/// Both channels; [`Audio::tick_a`] and [`Audio::tick_b`] come from the TIA's line.
#[derive(Clone, Debug)]
pub struct Audio {
    pub channels: [Channel; 2],
    pub samples: Vec<f64>,
    table: [f64; 31],
}

impl Default for Audio {
    fn default() -> Self {
        Self::new()
    }
}

impl Audio {
    pub fn new() -> Self {
        let mut table = [0.0; 31];
        for (v, entry) in table.iter_mut().enumerate() {
            *entry = mix(v as u8);
        }
        Audio {
            channels: [Channel::new(true), Channel::new(false)],
            samples: Vec::new(),
            table,
        }
    }

    /// The samples since the last call, one per tick, 0..1 (see [`mix`]).
    pub fn take_samples(&mut self) -> Vec<f64> {
        std::mem::take(&mut self.samples)
    }

    /// AUDC0/1 $15/$16, AUDF0/1 $17/$18, AUDV0/1 $19/$1A.
    pub fn write(&mut self, address: u8, value: u8) {
        let i = ((address - 0x15) & 1) as usize;
        let c = &mut self.channels[i];
        if address <= 0x16 {
            c.audc = value & 0x0F;
        } else if address <= 0x18 {
            c.audf = value & 0x1F;
        } else {
            c.audv = value & 0x0F;
        }
    }

    pub fn tick_a(&mut self) {
        for c in &mut self.channels {
            c.phase_a();
        }
    }

    pub fn tick_b(&mut self) {
        self.channels[0].phase_b();
        self.channels[1].phase_b();
        let level = self.channels[0].level() + self.channels[1].level();
        self.samples.push(self.table[level as usize]);
    }
}
