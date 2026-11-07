//! The board's own scheme detector, for dumps MAME's software list does not
//! carry (`cartridge/signatures.py`).
//!
//! The size picks the candidates; the program then shows its scheme by the
//! accesses it must make to work: a bank-switching program reads or writes
//! its hotspots with absolute addressing, writes its cartridge RAM's write
//! window, or (Tigervision) stores to $3F. Each test below is that access,
//! counted as the 3-byte instruction that makes it, anywhere in the image; a
//! scheme needs at least `MIN_HITS` of them.

/// absolute-mode loads, compares, BIT and NOP, and stores
const READS: [u8; 10] = [0xAD, 0xAE, 0xAC, 0x2C, 0xCD, 0xEC, 0xCC, 0x0C, 0xBD, 0xB9];
const WRITES: [u8; 5] = [0x8D, 0x8E, 0x8C, 0x9D, 0x99];
const MIN_HITS: usize = 2;
const JMP: u8 = 0x4C;
const JSR: u8 = 0x20;

fn read_or_write(opcode: u8) -> bool {
    READS.contains(&opcode) || WRITES.contains(&opcode)
}

/// Instructions in READS | WRITES whose absolute operand, as A0-A12, is in [low, high].
fn accesses(image: &[u8], low: u16, high: u16) -> usize {
    (0..image.len().saturating_sub(2))
        .filter(|&i| {
            let target = ((image[i + 2] as u16) << 8 | image[i + 1] as u16) & 0x1FFF;
            read_or_write(image[i]) && low <= target && target <= high
        })
        .count()
}

fn touches(image: &[u8], low: u16, high: u16) -> bool {
    accesses(image, low, high) >= MIN_HITS
}

fn stores_to_3f(image: &[u8]) -> usize {
    // STA $3F, counted as Python's bytes.count does: non-overlapping
    let mut count = 0;
    let mut i = 0;
    while i + 1 < image.len() {
        if image[i] == 0x85 && image[i + 1] == 0x3F {
            count += 1;
            i += 2;
        } else {
            i += 1;
        }
    }
    count
}

/// Instructions `opcode` (absolute) whose operand's high byte is in the range.
fn operands(image: &[u8], opcode: u8, high_low: u8, high_high: u8) -> usize {
    (0..image.len().saturating_sub(2))
        .filter(|&i| image[i] == opcode && high_low <= image[i + 2] && image[i + 2] <= high_high)
        .count()
}

fn eight_k(image: &[u8]) -> Option<&'static str> {
    let windows: Vec<usize> = [0x1FE0, 0x1FE8, 0x1FF0]
        .iter()
        .map(|&a| accesses(image, a, a + 7))
        .collect();
    let f8 = accesses(image, 0x1FF8, 0x1FF9);
    if windows[0] >= 2 && windows[1] >= 1 && windows[2] >= 2 {
        return Some("E0"); // all three switchable 1K windows (before 3F: Parker code stores to $3F)
    }
    if stores_to_3f(image) >= 4 {
        return Some("3F");
    }
    let ua = accesses(image, 0x0220, 0x0220) + accesses(image, 0x0240, 0x0240);
    if ua >= 2 * MIN_HITS {
        return Some("UA");
    }
    if windows[0] >= 4 && f8 == 0 {
        return Some("E7"); // the 8K board's $1FE4-$1FE7, and no F8 hotspot
    }
    let (bank0, bank1) = image.split_at(0x1000);
    if f8 <= 2
        && operands(bank0, JMP, 0xF0, 0xFF) >= MIN_HITS
        && operands(bank1, JMP, 0xD0, 0xDF) >= MIN_HITS
        && operands(bank0, JMP, 0xD0, 0xDF) == 0
        && operands(bank1, JMP, 0xF0, 0xFF) == 0
        && operands(bank0, JSR, 0xD0, 0xDF) >= MIN_HITS
    {
        // the first bank assembled at $Fxxx, the second at $Dxxx, and the
        // first calling into the second, which only FE's JSR can do
        return Some("FE");
    }
    None
}

/// The family the image's own accesses show, or None to fall back to its size.
///
/// CommaVid (2K) and FC are left to MAME's list or an explicit choice: their
/// accesses are too few to tell from chance bytes in a plain image.
pub fn scheme(image: &[u8]) -> Option<&'static str> {
    match image.len() {
        0x2000 => eight_k(image),
        0x3000 => {
            let e7 = accesses(image, 0x1FE0, 0x1FE7);
            Some(if e7 > accesses(image, 0x1FF8, 0x1FFA) {
                "E7"
            } else {
                "FA"
            })
        }
        0x4000 => {
            let e7 = touches(image, 0x1FE0, 0x1FE7) && !touches(image, 0x1FF6, 0x1FF9);
            if e7 {
                Some("E7")
            } else {
                None
            }
        }
        0x10000 => {
            // 32 2K games each end in their own reset vector; the Megaboy's 4K banks do not
            let vectors = (0x800..=0x10000)
                .step_by(0x800)
                .all(|end| image[end - 3] & 0x10 != 0);
            Some(if vectors { "32IN1" } else { "F0" })
        }
        _ => None,
    }
}
