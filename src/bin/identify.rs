//! Rung 5: the scheme each image is identified as, one line per file
//! (`scripts/identify.py` writes the reference's).
//!
//!     identify FILE [...]
//!
//! Each line: the file name, then the family, Super Chip and source as
//! `cartridge::identify` gives them, or `unsupported` with no further
//! detail when no scheme is built for it.

use a2600_rust::cartridge::{identify, read_image, MAME_HASH};
use std::path::Path;

fn main() {
    for path in std::env::args().skip(1) {
        let path = Path::new(&path);
        let name = path.file_name().unwrap().to_string_lossy();
        let line = match read_image(path) {
            Err(_) => "unreadable".to_owned(),
            Ok((image, _)) => match identify(&image, MAME_HASH) {
                Ok((family, sc, source, _)) => {
                    // as Python prints a bool, so the two files compare byte for byte
                    let sc = if sc { "True" } else { "False" };
                    format!("{family}\t{sc}\t{source}")
                }
                Err(_) => "unsupported".to_owned(),
            },
        };
        println!("{name}\t{line}");
    }
}
