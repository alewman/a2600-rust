//! Run a cartridge headless: `a2600 ROM --frames N --png out.png --wav out.wav`
//! (the reference's `python -m a2600_python`).
//!
//! The picture is the last complete frame (`television`, NTSC colours as
//! labelled there); the sound is every sample of the run (`audio`), written
//! at the tick rate, two a line, as 16-bit mono.

use std::path::PathBuf;

use a2600_rust::cartridge::{read_image, Options};
use a2600_rust::machine::machine;
use a2600_rust::television::{frame_png, py_round, Television};
use a2600_rust::System;

fn main() {
    let mut args = std::env::args().skip(1);
    let mut rom: Option<PathBuf> = None;
    let (mut frames, mut machine_name, mut scheme) = (60u64, "ntsc".to_owned(), None::<String>);
    let (mut png, mut wav) = (None::<PathBuf>, None::<PathBuf>);
    while let Some(arg) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| panic!("{arg} needs a value"));
        match arg.as_str() {
            "--frames" => frames = value().parse().expect("a number"),
            "--machine" => machine_name = value(),
            "--scheme" => scheme = Some(value()),
            "--png" => png = Some(PathBuf::from(value())),
            "--wav" => wav = Some(PathBuf::from(value())),
            _ if rom.is_none() => rom = Some(PathBuf::from(arg)),
            _ => panic!("unknown argument {arg}"),
        }
    }
    let rom = rom.expect("usage: a2600 ROM [--frames N] [--machine ntsc|pal|secam] [--scheme S] [--png FILE] [--wav FILE]");
    let (image, _) = read_image(&rom).unwrap_or_else(|e| panic!("{e}"));
    let row = machine(&machine_name).unwrap_or_else(|| panic!("no machine {machine_name}"));
    let mut system = System::from_rom(&image, row, scheme.as_deref(), &Options::default())
        .unwrap_or_else(|e| panic!("{e}"));
    let television = Television::attach(&mut system.board_mut().tia);
    let mut samples = Vec::new();
    for _ in 0..frames {
        system.run_frames(1);
        samples.extend(system.board_mut().tia.audio.take_samples());
    }
    if let Some(path) = png {
        let tv = television.borrow();
        let Some(frame) = &tv.frame else {
            eprintln!("no complete frame");
            std::process::exit(1);
        };
        std::fs::write(path, frame_png(frame, 2)).expect("write the PNG");
    }
    if let Some(path) = wav {
        let rate = py_round(2.0 * row.timing.colour_clock_hz / 228.0) as u32; // two ticks a line
        let mut data = Vec::with_capacity(samples.len() * 2);
        for s in &samples {
            data.extend_from_slice(&((py_round(s * 32767.0) - 16384) as i16).to_le_bytes());
        }
        let mut out = b"RIFF".to_vec();
        out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&1u16.to_le_bytes()); // mono
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * 2).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&data);
        std::fs::write(path, out).expect("write the WAV");
    }
    let name = rom.file_name().unwrap().to_string_lossy();
    println!(
        "{name}: {}, {frames} frames, {} cycles",
        system.scheme,
        system.cycles()
    );
}
