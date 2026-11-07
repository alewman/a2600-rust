//! Cartridge files: reading them, identifying the scheme, building the
//! scheme (`cartridge/__init__.py`).
//!
//! There is no header in a 2600 dump, so the scheme comes from, in order:
//!
//! 1. an explicit choice by the caller (`scheme`, and `super_chip`);
//! 2. MAME's software list, read at run time from the installed MAME
//!    (`/usr/share/games/mame/hash/a2600.xml`, never committed): the part's
//!    `slot` feature, a `ram` data area meaning Super Chip, by SHA-1;
//! 3. the board's own detector: the size picks the candidates, and a Super
//!    Chip is recognised by the RAM window it leaves unused (the first 256
//!    bytes of every bank hold one repeated value).
//!
//! Differences forced by Rust: Python's `**options` are the [`Options`]
//! struct; MAME's XML is read by a small scanner of the few elements used.

pub mod activision;
pub mod atari;
pub mod cbs;
pub mod commavid;
pub mod connector;
pub mod dpc;
pub mod mnetwork;
pub mod multicarts;
pub mod parker;
pub mod sha1;
pub mod signatures;
pub mod supercharger;
pub mod tigervision;

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

pub use connector::Cartridge;

pub const MAME_HASH: &str = "/usr/share/games/mame/hash/a2600.xml";
const ROM_SUFFIXES: [&str; 3] = [".a26", ".bin", ".rom"];

fn size_scheme(size: usize) -> Option<&'static str> {
    match size {
        0x800 => Some("2K"),
        0x1000 => Some("4K"),
        0x2000 => Some("F8"),
        0x4000 => Some("F6"),
        0x8000 => Some("F4"),
        _ => None,
    }
}

/// MAME slot names -> ours, for the schemes built (`a26_dc` is MAME's name
/// for the Megaboy's F0). `Some(None)`: decided by size.
fn mame_slot(slot: &str) -> Option<Option<&'static str>> {
    Some(match slot {
        "a26_2k_4k" => None,
        "a26_f8" | "a26_f8sw" => Some("F8"),
        "a26_f6" => Some("F6"),
        "a26_f4" => Some("F4"),
        "a26_e0" => Some("E0"),
        "a26_3f" => Some("3F"),
        "a26_fe" => Some("FE"),
        "a26_e7" => Some("E7"),
        "a26_fa" => Some("FA"),
        "a26_cv" => Some("CV"),
        "a26_ua" => Some("UA"),
        "a26_dc" => Some("F0"),
        "a26_32in1" => Some("32IN1"),
        "a26_dpc" => Some("DPC"),
        "a26_ss" => Some("AR"),
        _ => return None,
    })
}

/// A cartridge built and identified.
pub struct Loaded {
    pub cartridge: Box<dyn Cartridge>,
    /// e.g. "F6SC"
    pub scheme: String,
    /// "explicit", "MAME softlist", "detector"
    pub source: &'static str,
    /// MAME's description when the list carries the dump
    pub title: Option<String>,
    pub sha1: String,
}

/// The ROM bytes and a name: a plain file, or the one ROM inside a .zip or .7z.
pub fn read_image(path: &Path) -> Result<(Vec<u8>, String), String> {
    let suffix = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let is_rom = |name: &str| {
        ROM_SUFFIXES
            .iter()
            .any(|s| name.to_lowercase().ends_with(s))
    };
    let stem = |name: &str| {
        Path::new(name)
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    };
    if suffix == "zip" {
        let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("{}: {e}", path.display()))?;
        let members: Vec<String> = archive
            .file_names()
            .filter(|n| is_rom(n))
            .map(str::to_owned)
            .collect();
        if members.len() != 1 {
            return Err(format!(
                "{}: expected one ROM, found {members:?}",
                path.display()
            ));
        }
        let mut entry = archive.by_name(&members[0]).map_err(|e| e.to_string())?;
        let mut data = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut data).map_err(|e| e.to_string())?;
        return Ok((data, stem(&members[0])));
    }
    if suffix == "7z" {
        let listing = std::process::Command::new("7z")
            .args(["l", "-slt"])
            .arg(path)
            .output()
            .map_err(|e| format!("7z: {e}"))?;
        let listing = String::from_utf8_lossy(&listing.stdout);
        let members: Vec<&str> = listing
            .lines()
            .filter_map(|line| line.strip_prefix("Path = "))
            .filter(|name| is_rom(name))
            .collect();
        if members.len() != 1 {
            return Err(format!(
                "{}: expected one ROM, found {members:?}",
                path.display()
            ));
        }
        let data = std::process::Command::new("7z")
            .args(["x", "-so"])
            .arg(path)
            .arg(members[0])
            .output()
            .map_err(|e| format!("7z: {e}"))?
            .stdout;
        return Ok((data, stem(members[0])));
    }
    let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((data, stem(&path.to_string_lossy())))
}

/// (slot, has RAM, description) by SHA-1, from MAME's software list.
type MameList = HashMap<String, (String, bool, String)>;

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    let key = format!(" {name}=\"");
    let start = tag.find(&key)? + key.len();
    let end = tag[start..].find('"')? + start;
    Some(unescape(&tag[start..end]))
}

/// The few elements identification uses: each `software`'s description, and
/// each `part`'s slot feature, RAM data area and ROM SHA-1s.
fn parse_mame_list(text: &str) -> MameList {
    let mut table = MameList::new();
    let mut description = String::new();
    let mut software_name = String::new();
    let mut slot: Option<String> = None;
    let mut ram = false;
    let mut roms: Vec<String> = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('<') {
        let after = &rest[open + 1..];
        let close = match after.find('>') {
            Some(close) => close,
            None => break,
        };
        let tag = &after[..close];
        rest = &after[close + 1..];
        // the element's name: "part" for <part ...>, "/part" for </part>,
        // "rom" for <rom .../>
        let name = tag
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_end_matches('/');
        match name {
            "software" => {
                software_name = attribute(tag, "name").unwrap_or_default();
                description.clear();
            }
            "description" => {
                if let Some(end) = rest.find("</description>") {
                    description = unescape(&rest[..end]);
                }
            }
            "part" => {
                slot = None;
                ram = false;
                roms.clear();
            }
            "feature" => {
                if attribute(tag, "name").as_deref() == Some("slot") && slot.is_none() {
                    slot = attribute(tag, "value");
                }
            }
            "dataarea" => {
                if attribute(tag, "name").as_deref() == Some("ram") {
                    ram = true;
                }
            }
            "rom" => {
                if let Some(sha1) = attribute(tag, "sha1") {
                    roms.push(sha1);
                }
            }
            "/part" => {
                let title = if description.is_empty() {
                    software_name.clone()
                } else {
                    description.clone()
                };
                for sha1 in roms.drain(..) {
                    table.insert(sha1, (slot.clone().unwrap_or_default(), ram, title.clone()));
                }
            }
            _ => {}
        }
    }
    table
}

fn mame_list(path: &str) -> &'static MameList {
    static LIST: OnceLock<(String, MameList)> = OnceLock::new();
    let (cached, table) = LIST.get_or_init(|| {
        let table = std::fs::read_to_string(path)
            .map(|t| parse_mame_list(&t))
            .unwrap_or_default();
        (path.to_owned(), table)
    });
    assert_eq!(cached, path, "one MAME list per process");
    table
}

/// Every 4K bank's first 256 bytes (the RAM's two windows) hold one value.
pub fn detect_super_chip(image: &[u8]) -> bool {
    if image.len() < 0x2000 {
        return false;
    }
    image.chunks(0x1000).all(|bank| {
        let window = &bank[..0x100.min(bank.len())];
        window.iter().all(|&b| b == window[0])
    })
}

/// (family, super chip, source, title) for `image`.
pub fn identify(
    image: &[u8],
    mame_hash: &str,
) -> Result<(String, bool, &'static str, Option<String>), String> {
    let sha1 = sha1::hex(image);
    let entry = mame_list(mame_hash).get(&sha1);
    if let Some((slot, ram, title)) = entry {
        if !slot.is_empty() {
            let family = match mame_slot(slot) {
                None => return Err(format!("{title}: MAME slot {slot} is not implemented yet")),
                Some(family) => family.or_else(|| size_scheme(image.len())).unwrap_or("?"),
            };
            // a RAM data area is a Super Chip only on Atari's schemes; E7, FA
            // and CV carry their RAM in the scheme itself
            let sc = *ram && matches!(family, "F8" | "F6" | "F4");
            return Ok((family.to_owned(), sc, "MAME softlist", Some(title.clone())));
        }
    }
    let title = entry.map(|e| e.2.clone());
    let family = if image.len() % supercharger::LOAD == 0 {
        Some("AR") // Supercharger loads: 8448 bytes each, no other scheme's size
    } else if image.len() == 0x2800 || image.len() == 0x28FF {
        Some("DPC") // Pitfall II: 8K + 2K display data (+ the RNG table some dumps carry)
    } else {
        signatures::scheme(image).or_else(|| size_scheme(image.len()))
    };
    let family = family.ok_or_else(|| format!("no scheme for a {}-byte image yet", image.len()))?;
    let super_chip = matches!(family, "F8" | "F6" | "F4") && detect_super_chip(image);
    Ok((family.to_owned(), super_chip, "detector", title))
}

/// The options a scheme takes (the reference's keyword arguments); `None`
/// keeps the scheme's default.
#[derive(Clone, Debug, Default)]
pub struct Options {
    pub start_bank: Option<usize>,
    pub ram_fill: Option<u8>,
    pub start_slices: Option<[usize; 3]>,
    pub start_slice: Option<usize>,
    pub swapped: Option<bool>,
    pub game: Option<usize>,
    pub osc_hz: Option<f64>,
    pub rng_clock: Option<String>,
    pub bios: Option<Vec<u8>>,
    pub tape_start: Option<i64>,
    /// the stand-in BIOS's timer value; `Some(None)` leaves the timer alone
    pub timer: Option<Option<u8>>,
    /// the machine's CPU clock, for the schemes with a clock of their own
    pub cpu_hz: Option<f64>,
}

pub fn build(
    image: &[u8],
    family: &str,
    super_chip: bool,
    options: &Options,
) -> Result<Box<dyn Cartridge>, String> {
    let o = options;
    let ram_fill = o.ram_fill.unwrap_or(0);
    Ok(match family {
        "2K" | "4K" => Box::new(atari::Plain::new(image)?),
        "E0" => Box::new(parker::ParkerE0::new(
            image,
            o.start_slices.unwrap_or([4, 5, 6]),
        )?),
        "3F" => Box::new(tigervision::Tigervision3F::new(
            image,
            o.start_slice.unwrap_or(0),
        )?),
        "FE" => Box::new(activision::ActivisionFE::new(
            image,
            o.start_bank.unwrap_or(0),
        )?),
        "E7" => Box::new(mnetwork::MNetworkE7::new(image, ram_fill)?),
        "FA" => Box::new(cbs::CbsFA::new(image, o.start_bank.unwrap_or(2), ram_fill)?),
        "CV" => Box::new(commavid::CommaVid::new(image, ram_fill)?),
        "UA" => Box::new(multicarts::UaLimited::new(
            image,
            o.swapped.unwrap_or(false),
            o.start_bank.unwrap_or(0),
        )?),
        "FC" => Box::new(multicarts::AmigaFC::new(image, o.start_bank.unwrap_or(0))?),
        "F0" => Box::new(multicarts::DynacomF0::new(
            image,
            o.start_bank.unwrap_or(15),
        )?),
        "32IN1" => Box::new(multicarts::ThirtyTwoInOne::new(image, o.game.unwrap_or(0))?),
        "DPC" => Box::new(dpc::ActivisionDPC::new(
            image,
            o.start_bank.unwrap_or(1),
            o.osc_hz.unwrap_or(20_000.0),
            o.cpu_hz.unwrap_or(dpc::NTSC_CPU_HZ),
            o.rng_clock.as_deref().unwrap_or("cartridge"),
        )?),
        "AR" => Box::new(supercharger::Supercharger::new(
            image,
            o.bios.as_deref(),
            ram_fill,
            o.tape_start
                .unwrap_or((3.0 * supercharger::NTSC_CPU_HZ) as i64),
            o.cpu_hz.unwrap_or(supercharger::NTSC_CPU_HZ),
            o.timer.unwrap_or(Some(0x0A)),
        )?),
        "F8" | "F6" | "F4" => Box::new(atari::Banked::new(
            image,
            family,
            super_chip,
            o.start_bank,
            ram_fill,
        )?),
        _ => return Err(format!("scheme {family} is not implemented yet")),
    })
}

/// Read, identify and build a cartridge.
pub fn load(
    image: &[u8],
    scheme: Option<&str>,
    super_chip: Option<bool>,
    mame_hash: &str,
    options: &Options,
) -> Result<Loaded, String> {
    let sha1 = sha1::hex(image);
    let (family, sc, source, title) = match scheme {
        Some(scheme) => {
            let family = scheme.strip_suffix("SC").unwrap_or(scheme).to_owned();
            (
                family,
                super_chip.unwrap_or(scheme.ends_with("SC")),
                "explicit",
                None,
            )
        }
        None => {
            let (family, sc, source, title) = identify(image, mame_hash)?;
            (family, super_chip.unwrap_or(sc), source, title)
        }
    };
    let cartridge = build(image, &family, sc, options)?;
    Ok(Loaded {
        scheme: cartridge.name().to_owned(),
        cartridge,
        source,
        title,
        sha1,
    })
}
