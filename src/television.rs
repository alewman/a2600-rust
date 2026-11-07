//! The television: lines and VSYNC in, frames out. Nothing on the board
//! depends on it (`television.py`).
//!
//! The TIA has no vertical counter: a frame is whatever the program puts
//! between its VSYNC pulses. This stage collects the TIA's lines and starts a
//! new frame when VSYNC ends, as a television's vertical sync does, with a
//! bound on runaway frames for programs that stop sending it.
//!
//! A frame is a list of lines, each 160 bytes (the colour clocks after the
//! 68-clock horizontal blank, `Tia::pixels` encoding: colour with D0 set, or
//! 0 for blank). [`ntsc_rgb`] turns them into colours for a picture.
//!
//! Difference forced by Rust: the television attaches to the TIA's
//! `on_line` and keeps its state behind `Rc<RefCell<…>>`.

use std::cell::RefCell;
use std::rc::Rc;

use crate::tia::Tia;

pub const HBLANK_CLOCKS: usize = 68;
pub const MAX_LINES: usize = 400; // a frame longer than this is cut (no VSYNC)

#[derive(Default)]
pub struct Television {
    pub frames_seen: u64,
    /// the last complete frame
    pub frame: Option<Vec<Vec<u8>>>,
    lines: Vec<Vec<u8>>,
    vsync: bool,
}

impl Television {
    /// A television on `tia`'s output (it takes the TIA's `on_line`).
    pub fn attach(tia: &mut Tia) -> Rc<RefCell<Television>> {
        let tv = Rc::new(RefCell::new(Television::default()));
        let sink = Rc::clone(&tv);
        tia.on_line = Some(Box::new(move |pixels, vsync| {
            sink.borrow_mut().line(pixels, vsync)
        }));
        tv
    }

    pub fn line(&mut self, pixels: &[u8; 228], vsync: bool) {
        if (self.vsync && !vsync) || self.lines.len() >= MAX_LINES {
            self.frame = Some(std::mem::take(&mut self.lines));
            self.frames_seen += 1;
        }
        self.vsync = vsync;
        self.lines.push(pixels[HBLANK_CLOCKS..].to_vec());
    }
}

/// Python's float `%`: the result takes the divisor's sign.
fn py_mod(x: f64, y: f64) -> f64 {
    let mut m = x % y;
    if m != 0.0 && ((y < 0.0) != (m < 0.0)) {
        m += y;
    }
    m
}

/// Python's `colorsys.hls_to_rgb`.
fn hls_to_rgb(h: f64, l: f64, s: f64) -> (f64, f64, f64) {
    const ONE_THIRD: f64 = 1.0 / 3.0;
    const ONE_SIXTH: f64 = 1.0 / 6.0;
    const TWO_THIRD: f64 = 2.0 / 3.0;
    fn v(m1: f64, m2: f64, hue: f64) -> f64 {
        let hue = py_mod(hue, 1.0);
        if hue < ONE_SIXTH {
            return m1 + (m2 - m1) * hue * 6.0;
        }
        if hue < 0.5 {
            return m2;
        }
        if hue < TWO_THIRD {
            return m1 + (m2 - m1) * (TWO_THIRD - hue) * 6.0;
        }
        m1
    }
    if s == 0.0 {
        return (l, l, l);
    }
    let m2 = if l <= 0.5 {
        l * (1.0 + s)
    } else {
        l + s - (l * s)
    };
    let m1 = 2.0 * l - m2;
    (
        v(m1, m2, h + ONE_THIRD),
        v(m1, m2, h),
        v(m1, m2, h - ONE_THIRD),
    )
}

/// Python's `round` to an integer: halves to even.
pub fn py_round(x: f64) -> i64 {
    x.round_ties_even() as i64
}

/// An NTSC colour as RGB: hue 0 grey; hues 1-15 step about 27 degrees round
/// the colour wheel from gold through red, blue and green back to orange;
/// eight luminances. A labelled approximation: no palette has been measured
/// from a console.
pub fn ntsc_rgb(colour: u8) -> [u8; 3] {
    let (hue, lum) = ((colour >> 4) as i64, ((colour >> 1) & 7) as f64);
    let lightness = 0.08 + 0.84 * lum / 7.0;
    if hue == 0 {
        let grey = py_round(255.0 * lightness) as u8;
        return [grey; 3];
    }
    let angle = (45 - (hue - 1) * 27).rem_euclid(360) as f64 / 360.0;
    let (r, g, b) = hls_to_rgb(angle, lightness, 0.7);
    [
        py_round(255.0 * r) as u8,
        py_round(255.0 * g) as u8,
        py_round(255.0 * b) as u8,
    ]
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// A PNG of `frame`: blank as black, each colour clock `scale_x` pixels wide.
pub fn frame_png(frame: &[Vec<u8>], scale_x: usize) -> Vec<u8> {
    let mut rows = Vec::new();
    for line in frame {
        rows.push(0u8);
        for &value in line {
            let rgb = if value & 1 != 0 {
                ntsc_rgb(value)
            } else {
                [0, 0, 0]
            };
            for _ in 0..scale_x {
                rows.extend_from_slice(&rgb);
            }
        }
    }
    let width = frame.first().map_or(0, |l| l.len() * scale_x) as u32;
    let height = frame.len() as u32;
    let chunk = |kind: &[u8], data: &[u8]| {
        let mut out = (data.len() as u32).to_be_bytes().to_vec();
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
        out
    };
    let mut header = width.to_be_bytes().to_vec();
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 2, 0, 0, 0]);
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(6));
    std::io::Write::write_all(&mut encoder, &rows).expect("compress");
    let compressed = encoder.finish().expect("compress");
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend(chunk(b"IHDR", &header));
    png.extend(chunk(b"IDAT", &compressed));
    png.extend(chunk(b"IEND", b""));
    png
}
