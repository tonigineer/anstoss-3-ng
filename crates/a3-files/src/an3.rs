//! ANSTOSS 3 savegames (`saves/*.an3`): the file split into the blocks the
//! original's loader reads, each decompressed and named after the memory it
//! fills — the base for comparing two saves block by block — and written
//! back, every block as it was read unless its data changed.
//!
//! The writer is `FUN_006ed31e`, the loader `CDiLoad::FUN_00678f07`; the
//! order of the blocks below is the loader's, for version 50 and later (the
//! loader refuses anything older). A save is:
//!
//! - a header: four objects of 120 bytes as they are (manager, club …), a u32,
//!   a u8, the date as u16 day, month and year, the version and a u16;
//! - then blocks. Most are packed (`FUN_00678c6e`): a u32 length, then a
//!   chunk — an i32 word; zero, nothing; negative, its negation in bytes as
//!   they are; positive, LZ77 (as in the archives, [`crate::cpr`]) to that
//!   many bytes. A packed block fills either a global of known size or a
//!   memory object's data, which starts 0x20 bytes into the object;
//! - a few objects stored as they are, whose sizes the file does not say
//!   (the object's header does, at run time): fixed here;
//! - the players: 17 blocks of 1000 records and one of the last 6, each the
//!   buffer of 0x354 bytes a record that the writer packs them into without
//!   their headers, 0x334 bytes apart (`FUN_007cfff4`);
//! - the pool of names: a count, a size, the strings;
//! - two lists with a u16 count: entries of 4000 bytes as they are, and
//!   blocks of 0x324;
//! - up to ten blocks, one for each flag set in the block before them.
//!
//! Blocks are named after the address the loader reads them to, with the
//! loop's rounds as indices: `0x1a72ff8[17]` is the 18th of the objects at
//! 0x1a72ff8, 0x990 bytes apart. The loader reads one object twice in each
//! round of its first loop (`0xaf5d74[i]`), and the writer writes it twice.

use std::collections::HashMap;
use std::fmt;
use std::io;
use std::path::Path;

use crate::cpr::{inflate, invalid};

/// The oldest version the original loads.
const OLDEST: u16 = 50;
/// The version the original writes.
const WRITTEN: u16 = 50;

/// Blocks that hold real time rather than the game's: field 1 of a timer
/// at 0xae6e30, seconds of `clock()` (`FUN_00788410`). Two runs of the same
/// game, with the same random numbers, differ in them.
pub const REAL_TIME: &[&str] = &["0xae6e34"];
/// A block the writer fills from a stack buffer it never sets (`local_bc` of
/// `FUN_006ed31e`) and the loader reads into one it never uses: whatever
/// earlier calls left on the stack, not state.
pub const LEFTOVERS: &[&str] = &["local_b8"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// `DAT_01b9934c`.
    pub value: u32,
    /// `DAT_01b99348`.
    pub flag: u8,
    /// The game's date: day, month, year.
    pub date: (u16, u16, u16),
    pub version: u16,
    /// `DAT_01bac63c`.
    pub extra: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// A packed block whose length word was zero: it leaves its memory as is.
    Empty,
    /// A packed block stored as it is.
    Stored,
    /// A packed block with LZ77, this many bytes in the file.
    Packed(usize),
    /// Bytes as they are, without a length.
    Raw,
}

#[derive(Debug, Clone)]
pub struct Block {
    /// Where the block goes: an address, with the loop's rounds as indices.
    pub name: String,
    /// Where it starts in the file, and how many bytes it takes there.
    pub offset: usize,
    pub size: usize,
    pub form: Form,
    /// Its bytes, decompressed.
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Save {
    pub header: Header,
    pub blocks: Vec<Block>,
    /// The file as it was read, whose blocks are written again while their
    /// data stays the same.
    source: Source,
}

#[derive(Clone)]
struct Source(Vec<u8>);

impl fmt::Debug for Source {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} bytes", self.0.len())
    }
}

impl Save {
    pub fn read(path: &Path) -> io::Result<Self> {
        let bytes = std::fs::read(path)?;
        Self::parse(&bytes).map_err(|err| invalid(&format!("{}: {err}", path.display())))
    }

    pub fn parse(bytes: &[u8]) -> io::Result<Self> {
        let mut file = Reader { bytes, pos: 0, blocks: Vec::new() };
        for i in 0..4 {
            file.raw(format!("{:#x}[{i}]", 0x1b990d8), 120)?;
        }
        let value = file.u32()?;
        let flag = file.u8()?;
        let date = (file.u16()?, file.u16()?, file.u16()?);
        let version = file.u16()?;
        let extra = file.u16()?;
        if version < OLDEST {
            return Err(invalid(&format!("version {version}, older than the original loads")));
        }
        file.steps(LAYOUT, &mut Vec::new())?;
        if file.pos != bytes.len() {
            return Err(invalid(&format!(
                "{} bytes left over at {:#x}",
                bytes.len() - file.pos,
                file.pos
            )));
        }
        let header = Header { value, flag, date, version, extra };
        Ok(Self { header, blocks: file.blocks, source: Source(bytes.to_vec()) })
    }

    pub fn write(&self, path: &Path) -> io::Result<()> {
        std::fs::write(path, self.to_bytes()?)
    }

    /// The file: every block as it was read while its data is what it was,
    /// one whose data changed stored as it is — the loader copies a chunk
    /// with a negative word (`FUN_007d30f0`) — and the header as it is now.
    pub fn to_bytes(&self) -> io::Result<Vec<u8>> {
        self.encode(true)
    }

    /// The file with every packed block stored as it is, none compressed.
    pub fn to_bytes_stored(&self) -> io::Result<Vec<u8>> {
        self.encode(false)
    }

    fn encode(&self, keep: bool) -> io::Result<Vec<u8>> {
        let mut file =
            Writer { save: self, keep, next: 0, out: Vec::with_capacity(self.source.0.len()) };
        for i in 0..4 {
            file.raw(&format!("{:#x}[{i}]", 0x1b990d8))?;
        }
        let Header { value, flag, date, version, extra } = self.header;
        file.out.extend(value.to_le_bytes());
        file.out.push(flag);
        for word in [date.0, date.1, date.2, version, extra] {
            file.out.extend(word.to_le_bytes());
        }
        file.steps(LAYOUT, &mut Vec::new())?;
        if let Some(block) = self.blocks.get(file.next) {
            return Err(invalid(&format!("{}: a block the layout has no place for", block.name)));
        }
        Ok(file.out)
    }

    /// The block of this name (the first, if the loader reads it twice).
    pub fn block(&self, name: &str) -> Option<&Block> {
        self.blocks.iter().find(|block| block.name == name)
    }

    /// A save of nothing yet, where a new game's records start: every
    /// block the loader reads, at the size the original's object or global
    /// has — the same in every save —, zeroed. The pool of names and the
    /// list at 0x1be0b40 are empty, the pointer list has no entries, and
    /// as the flags at 0x8ef338 are nought no flagged object follows them.
    /// The header has the version the original writes and no date.
    pub fn blank() -> Self {
        let mut file = Blank { blocks: Vec::new() };
        for i in 0..4 {
            file.raw(format!("{:#x}[{i}]", 0x1b990d8), 120);
        }
        file.steps(LAYOUT, &mut Vec::new());
        let header = Header { value: 0, flag: 0, date: (0, 0, 0), version: WRITTEN, extra: 0 };
        Self { header, blocks: file.blocks, source: Source(Vec::new()) }
    }
}

/// How two saves differ: their headers, and block by block.
#[derive(Debug, Default)]
pub struct Differences {
    pub header: bool,
    pub blocks: Vec<Difference>,
}

#[derive(Debug)]
pub struct Difference {
    pub name: String,
    /// The first byte that differs.
    pub first: usize,
    /// How many bytes differ, within the length both have.
    pub bytes: usize,
    /// The lengths, if they differ; `None` for a block only one save has.
    pub lengths: (Option<usize>, Option<usize>),
}

impl fmt::Display for Difference {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.lengths {
            (Some(_), None) => write!(f, "{}: only in the first", self.name),
            (None, Some(_)) => write!(f, "{}: only in the second", self.name),
            (Some(a), Some(b)) if a != b => write!(
                f,
                "{}: {a} against {b} bytes, {} of the common ones differ",
                self.name, self.bytes
            ),
            _ => write!(f, "{}: {} bytes differ, from {:#x}", self.name, self.bytes, self.first),
        }
    }
}

/// Compares two saves. Blocks are paired by name and, for a name read twice,
/// by turn.
pub fn compare(a: &Save, b: &Save) -> Differences {
    let (a, b) = (keyed(a), keyed(b));
    let mut blocks = Vec::new();
    for (key, x) in &a.1 {
        let y = b.1.get(key).map(|block| &block.data);
        if let Some(difference) = differ(&x.name, Some(&x.data), y) {
            blocks.push(difference);
        }
    }
    for (key, y) in &b.1 {
        if !a.1.contains_key(key) {
            blocks.extend(differ(&y.name, None, Some(&y.data)));
        }
    }
    let at = |name: &String| a.0.get(name).or_else(|| b.0.get(name)).copied();
    blocks.sort_by_key(|difference| (at(&difference.name), difference.name.clone()));
    Differences { header: a.2 != b.2, blocks }
}

type Keyed<'a> = (HashMap<String, usize>, HashMap<(String, usize), &'a Block>, &'a Header);

/// The blocks by name and turn, and where each name first comes.
fn keyed(save: &Save) -> Keyed<'_> {
    let mut first = HashMap::new();
    let mut blocks = HashMap::new();
    for (index, block) in save.blocks.iter().enumerate() {
        first.entry(block.name.clone()).or_insert(index);
        let mut turn = 1;
        while blocks.contains_key(&(block.name.clone(), turn)) {
            turn += 1;
        }
        blocks.insert((block.name.clone(), turn), block);
    }
    (first, blocks, &save.header)
}

fn differ(name: &str, a: Option<&Vec<u8>>, b: Option<&Vec<u8>>) -> Option<Difference> {
    let (Some(x), Some(y)) = (a, b) else {
        return Some(Difference {
            name: name.into(),
            first: 0,
            bytes: 0,
            lengths: (a.map(Vec::len), b.map(Vec::len)),
        });
    };
    let mut differing = x.iter().zip(y).enumerate().filter(|(_, (p, q))| p != q).map(|(i, _)| i);
    let first = differing.next();
    if first.is_none() && x.len() == y.len() {
        return None;
    }
    Some(Difference {
        name: name.into(),
        first: first.unwrap_or(x.len().min(y.len())),
        bytes: usize::from(first.is_some()) + differing.count(),
        lengths: (Some(x.len()), Some(y.len())),
    })
}

/// One read of the loader.
enum Step {
    /// A packed block into the memory object at `at`, plus `strides` times
    /// the rounds of the loops around it (outermost first).
    Object {
        at: u32,
        strides: &'static [u32],
    },
    /// A packed block into `size` bytes at `at`.
    Global {
        at: u32,
        size: usize,
    },
    /// An object's data as it is, `size` bytes.
    Raw {
        at: u32,
        strides: &'static [u32],
        size: usize,
    },
    /// `size` bytes as they are at `at`.
    RawGlobal {
        at: u32,
        size: usize,
    },
    /// A packed block into one of the loader's locals.
    Local {
        name: &'static str,
        size: usize,
    },
    Repeat {
        count: usize,
        steps: &'static [Step],
    },
    /// `FUN_00678df5`: the 17006 player records (0x354 bytes each), 1000 to a
    /// block, the last 6 in a block of their own.
    Players,
    /// The pool of names: their count and size, then the strings.
    Names,
    /// `FUN_0070fe5b` into the object at `at`: a u16 count and a u16 last
    /// index; with a count, last + 1 entries of 4000 bytes as they are.
    List {
        at: u32,
    },
    /// `FUN_004376a5`, a `CPtrList` at `at`: a u16 count, then that many
    /// packed blocks.
    PtrList {
        at: u32,
    },
    /// Objects at `at` + i × `stride` for those of ten flags that are set —
    /// the bytes 0, 4, 8 … of the block before (`DAT_008ef358`).
    Flagged {
        at: u32,
        stride: u32,
    },
}

use Step::*;

const fn object(at: u32) -> Step {
    Object { at, strides: &[] }
}

const LAYOUT: &[Step] = &[
    object(0x1ba4b68),
    object(0xb5cb68),
    Global { at: 0xb1f860, size: 0x3d304 },
    Global { at: 0xb0b6b8, size: 0x141a4 },
    Repeat {
        count: 10,
        steps: &[
            Object { at: 0xae6e50, strides: &[0x154] },
            Object { at: 0xae7b98, strides: &[0x154] },
            Object { at: 0xae88e0, strides: &[0xc4] },
            Object { at: 0xae9088, strides: &[0xc4] },
            Object { at: 0xae9830, strides: &[0xf4] },
            Object { at: 0xaea1b8, strides: &[0xf4] },
            Object { at: 0xaeab40, strides: &[0xf4] },
            Object { at: 0xaeb4c8, strides: &[0x154] },
            Object { at: 0xaec210, strides: &[0xc4] },
            Object { at: 0xaec9b8, strides: &[0xf4] },
            Object { at: 0xaed340, strides: &[0x1a0] },
            Object { at: 0xaee380, strides: &[0x124] },
            Object { at: 0xaeeee8, strides: &[0x124] },
            Object { at: 0xaefa50, strides: &[0x154] },
            Object { at: 0xaf0798, strides: &[0xf4] },
            Object { at: 0xaf1120, strides: &[0xf4] },
            Object { at: 0xaf5d74, strides: &[0x214] },
            Object { at: 0xaf1aa8, strides: &[0x160] },
            Object { at: 0xaf5d74, strides: &[0x214] },
            Object { at: 0xb0affc, strides: &[0xac] },
        ],
    },
    Repeat {
        count: 5,
        steps: &[
            Object { at: 0xaf2868, strides: &[0x174] },
            Object { at: 0xaf2fac, strides: &[0x1ec] },
            Object { at: 0xaf3948, strides: &[0x174] },
            Object { at: 0xaf408c, strides: &[0x174] },
            Object { at: 0xaf47d0, strides: &[0x174] },
            Object { at: 0xaf4f14, strides: &[0x2e0] },
            Object { at: 0xaf7450, strides: &[0x37c] },
            Object { at: 0xaf85bc, strides: &[0x354] },
            Object { at: 0xaf9660, strides: &[0x890] },
            Object { at: 0xafc130, strides: &[0x21e0] },
            Object { at: 0xb08364, strides: &[0x5d4] },
        ],
    },
    object(0xaf723c),
    object(0xb06cd8),
    object(0xb07558),
    object(0xb07ff0),
    object(0xb06bb4),
    object(0xb06a90),
    object(0xb0a088),
    object(0xb080d8),
    Repeat {
        count: 4,
        steps: &[
            Raw { at: 0x1bdd6b8, strides: &[0x38], size: 24 },
            Object { at: 0x1c26dd8, strides: &[0x2058] },
            Object { at: 0x1bdd618, strides: &[0x28] },
        ],
    },
    // The random generator's state.
    Global { at: 0x89dcc0, size: 4 },
    Global { at: 0x89dcc4, size: 4 },
    Global { at: 0xae6de8, size: 0x3c },
    Global { at: 0xae6dd4, size: 0xc },
    Players,
    // The teams (0x990 bytes each, the field report's).
    Repeat { count: 0x1ec, steps: &[Object { at: 0x1a72ff8, strides: &[0x990] }] },
    Repeat { count: 0x10c, steps: &[Object { at: 0x1a48138, strides: &[0x290] }] },
    Repeat { count: 0xe0, steps: &[Object { at: 0x1a3af38, strides: &[0xf0] }] },
    object(0xa6d348),
    object(0xa75968),
    Repeat { count: 0x28, steps: &[Object { at: 0x1a28dd8, strides: &[0x13c] }] },
    Repeat {
        count: 10,
        steps: &[
            Object { at: 0x1a09d48, strides: &[0x1a88] },
            Object { at: 0x1a06008, strides: &[0x620] },
        ],
    },
    Repeat {
        count: 5,
        steps: &[
            Object { at: 0x19f7fd0, strides: &[0x2cd8] },
            Repeat {
                count: 0x28,
                steps: &[Object { at: 0x1a1a698, strides: &[0x28 * 0x128, 0x128] }],
            },
        ],
    },
    Repeat {
        count: 4,
        steps: &[
            // The managers (0x2158 bytes each).
            Object { at: 0xa62578, strides: &[0x2158] },
            Object { at: 0xa58078, strides: &[0x2940] },
            Object { at: 0xa4de58, strides: &[0x874] },
            Object { at: 0xa4d0f8, strides: &[0x358] },
            Object { at: 0xa53e38, strides: &[0x634] },
            Object { at: 0xa52568, strides: &[0x634] },
            Object { at: 0xa51ea8, strides: &[0x1b0] },
            Object { at: 0x1c39620, strides: &[0x297c] },
            Object { at: 0x1c2f030, strides: &[0x297c] },
            Object { at: 0xa55978, strides: &[0x9c0] },
            Object { at: 0xa55708, strides: &[0x9c] },
            Object { at: 0xa50028, strides: &[0x7a0] },
            Object { at: 0x1ba7b28, strides: &[0x8e4] },
            Object { at: 0xa32860, strides: &[0xd54] },
        ],
    },
    Global { at: 0xb68ea8, size: 36000 },
    Global { at: 0xb628d8, size: 0x65cc },
    Repeat { count: 0x5dc, steps: &[Object { at: 0xbe3920, strides: &[0x54] }] },
    Repeat { count: 0x65, steps: &[Object { at: 0xbe17f8, strides: &[0x54] }] },
    Repeat {
        count: 5,
        steps: &[
            Object { at: 0x8e27a0, strides: &[0x9c] },
            Object { at: 0x8e2490, strides: &[0x9c] },
        ],
    },
    Global { at: 0x8e1d88, size: 0x708 },
    Global { at: 0x8e0f78, size: 0xe10 },
    object(0x8e91e0),
    object(0x8e8940),
    object(0x8e7c88),
    Repeat {
        count: 5,
        steps: &[
            Object { at: 0x1ba5390, strides: &[0x698] },
            Object { at: 0x1ba5148, strides: &[0x74] },
            Object { at: 0xb77500, strides: &[0x7efc] },
        ],
    },
    object(0x1ba7488),
    object(0x1c51940),
    object(0x1c4be78),
    Global { at: 0x1bab640, size: 0x1a4 },
    Global { at: 0x1bab498, size: 0x1a4 },
    Repeat { count: 0xf1, steps: &[Object { at: 0x19f4ed8, strides: &[0x34] }] },
    object(0x1c5c468),
    object(0xb77338),
    Repeat { count: 0x10c, steps: &[Object { at: 0xb71b48, strides: &[0x54] }] },
    object(0x1c49278),
    object(0x1c54000),
    object(0x1c53ee8),
    object(0x1c53d98),
    object(0xbde1a8),
    Names,
    object(0x1ba0438),
    object(0xa2e420),
    Global { at: 0xb620ec, size: 0x10 },
    List { at: 0x1be0b40 },
    Raw { at: 0xbdee80, strides: &[], size: 10584 },
    Raw { at: 0xbded80, strides: &[], size: 220 },
    object(0x1c44370),
    Global { at: 0xae6e34, size: 4 },
    object(0xa2e0d8),
    Global { at: 0x1ba9f38, size: 0x294 },
    object(0x1c53b88),
    object(0x1c53a48),
    Repeat {
        count: 0x75,
        steps: &[
            Object { at: 0xa9d660, strides: &[0x990] },
            Object { at: 0xa81678, strides: &[0x3d4] },
        ],
    },
    object(0xa7c240),
    object(0x1ba07c0),
    object(0xae5b18),
    Local { name: "local_b8", size: 10 },
    // 0x45 u16, which the loader copies to 0x890198 + i × 0x1c.
    Local { name: "local_a8", size: 0x8a },
    object(0x1baa1e0),
    Global { at: 0x1b9ffc8, size: 0x468 },
    PtrList { at: 0x8eb8b0 },
    Global { at: 0x1c53cc8, size: 0xca },
    object(0xb9f8f8),
    Repeat {
        count: 4,
        steps: &[
            Object { at: 0x1c67ce0, strides: &[0x1dc] },
            Object { at: 0x8ed3e0, strides: &[0x420] },
        ],
    },
    object(0x1c65e88),
    object(0x1c645d8),
    object(0x1c68450),
    object(0x8be408),
    Repeat { count: 4, steps: &[Object { at: 0x8dc6e0, strides: &[0x374] }] },
    Repeat { count: 0x10c, steps: &[Object { at: 0x8dd4c8, strides: &[0x34] }] },
    RawGlobal { at: 0x1ba06d0, size: 0xea },
    Repeat {
        count: 0x15,
        steps: &[Repeat {
            count: 0xc,
            steps: &[Object { at: 0x1a2c2f8, strides: &[0xb40, 0xf0] }],
        }],
    },
    object(0x8ef338),
    Flagged { at: 0x8ef380, stride: 0x1fd88 },
    Raw { at: 0xb9f6a8, strides: &[], size: 432 },
    object(0x1baf6f0),
    object(0x1c46f48),
    object(0x1c45838),
    object(0x1c449f8),
    object(0x1c44700),
    object(0x8eabe8),
    object(0xa3b640),
    object(0xb9f028),
    object(0x1c1e4c8),
];

/// The size of each memory object's data the layout reads, by its base
/// address: what the object's header says at run time, the same for every
/// round of a loop and in every save.
const OBJECT_SIZES: &[(u32, usize)] = &[
    (0x8be408, 0x1e2b4), (0x8dc6e0, 0x354), (0x8dd4c8, 0x14), (0x8e2490, 0x7c),
    (0x8e27a0, 0x7c), (0x8e7c88, 0xc94), (0x8e8940, 0x880), (0x8e91e0, 0xf4c),
    (0x8eabe8, 0x1f8), (0x8ed3e0, 0x400), (0x8ef338, 0x28), (0xa2e0d8, 0xdc),
    (0xa2e420, 0x624), (0xa32860, 0xd34), (0xa3b640, 0x1c20), (0xa4d0f8, 0x338),
    (0xa4de58, 0x854), (0xa50028, 0x780), (0xa51ea8, 0x190), (0xa52568, 0x614),
    (0xa53e38, 0x614), (0xa55708, 0x7c), (0xa55978, 0x9a0), (0xa58078, 0x2920),
    (0xa62578, 0x2138), (0xa6d348, 0x8600), (0xa75968, 0x68b4), (0xa7c240, 0x5418),
    (0xa81678, 0x3b4), (0xa9d660, 0x970), (0xae5b18, 0x129c), (0xae6e50, 0x120),
    (0xae7b98, 0x120), (0xae88e0, 0x90), (0xae9088, 0x90), (0xae9830, 0xc0),
    (0xaea1b8, 0xc0), (0xaeab40, 0xc0), (0xaeb4c8, 0x120), (0xaec210, 0x90),
    (0xaec9b8, 0xc0), (0xaed340, 0x180), (0xaee380, 0xf0), (0xaeeee8, 0xf0),
    (0xaefa50, 0x120), (0xaf0798, 0xc0), (0xaf1120, 0xc0), (0xaf1aa8, 0x140),
    (0xaf2868, 0x140), (0xaf2fac, 0x1b8), (0xaf3948, 0x140), (0xaf408c, 0x140),
    (0xaf47d0, 0x140), (0xaf4f14, 0x2c0), (0xaf5d74, 0x1e0), (0xaf723c, 0x1e0),
    (0xaf7450, 0x348), (0xaf85bc, 0x320), (0xaf9660, 0x870), (0xafc130, 0x21c0),
    (0xb06a90, 0xf0), (0xb06bb4, 0xf0), (0xb06cd8, 0x860), (0xb07558, 0xa78),
    (0xb07ff0, 0xc8), (0xb080d8, 0x258), (0xb08364, 0x5a0), (0xb0a088, 0xf54),
    (0xb0affc, 0x8c), (0xb5cb68, 0x1590), (0xb71b48, 0x34), (0xb77338, 0x190),
    (0xb77500, 0x7edc), (0xb9f028, 0x640), (0xb9f8f8, 0x57c), (0xbde1a8, 0xbb8),
    (0xbe17f8, 0x34), (0xbe3920, 0x34), (0x19f4ed8, 0x14), (0x19f7fd0, 0x2cb8),
    (0x1a06008, 0x600), (0x1a09d48, 0x1a68), (0x1a1a698, 0x108), (0x1a28dd8, 0x11c),
    (0x1a2c2f8, 0xd0), (0x1a3af38, 0xd0), (0x1a48138, 0x270), (0x1a72ff8, 0x970),
    (0x1ba0438, 0x11c), (0x1ba07c0, 0x3a74), (0x1ba4b68, 0x100), (0x1ba5148, 0x54),
    (0x1ba5390, 0x678), (0x1ba7488, 0x678), (0x1ba7b28, 0x8c4), (0x1baa1e0, 0x1224),
    (0x1baf6f0, 0x2df04), (0x1bdd618, 0x8), (0x1c1e4c8, 0x780), (0x1c26dd8, 0x2038),
    (0x1c2f030, 0x295c), (0x1c39620, 0x295c), (0x1c44370, 0x348), (0x1c44700, 0x2d4),
    (0x1c449f8, 0xe1c), (0x1c45838, 0xe1c), (0x1c46f48, 0x3e8), (0x1c49278, 0x2bc8),
    (0x1c4be78, 0x5aa4), (0x1c51940, 0x20d4), (0x1c53a48, 0x11c), (0x1c53b88, 0x11c),
    (0x1c53d98, 0x130), (0x1c53ee8, 0xf4), (0x1c54000, 0x3b64), (0x1c5c468, 0x438),
    (0x1c645d8, 0x188c), (0x1c65e88, 0x1e38), (0x1c67ce0, 0x1bc), (0x1c68450, 0x2664),
];

/// The players: 17000 and 6 records of 0x354 bytes.
const PLAYERS: usize = 17006;
const PLAYER: usize = 0x354;
const PLAYERS_PER_BLOCK: usize = 1000;

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
    blocks: Vec<Block>,
}

impl Reader<'_> {
    fn take(&mut self, len: usize) -> io::Result<&[u8]> {
        let bytes = self
            .bytes
            .get(self.pos..self.pos + len)
            .ok_or_else(|| invalid(&format!("the file ends inside a block at {:#x}", self.pos)))?;
        self.pos += len;
        Ok(bytes)
    }

    fn u8(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> io::Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    /// A block as it is.
    fn raw(&mut self, name: String, len: usize) -> io::Result<&Block> {
        let offset = self.pos;
        let data = self.take(len)?.to_vec();
        self.blocks.push(Block { name, offset, size: len, form: Form::Raw, data });
        Ok(self.blocks.last().unwrap())
    }

    /// A packed block (`FUN_00678c6e`), decompressed; at most `limit` bytes
    /// when the space it fills is known.
    fn packed(&mut self, name: String, limit: Option<usize>) -> io::Result<&Block> {
        let offset = self.pos;
        let len = self.u32()? as usize;
        let (form, data) = if len == 0 {
            (Form::Empty, Vec::new())
        } else {
            let chunk = self.take(len)?;
            let word = i32::from_le_bytes(
                chunk
                    .get(..4)
                    .ok_or_else(|| invalid("block without a length word"))?
                    .try_into()
                    .unwrap(),
            );
            let body = &chunk[4..];
            if word <= 0 {
                let stored = body.get(..word.unsigned_abs() as usize).ok_or_else(|| {
                    invalid(&format!("stored block at {offset:#x} shorter than it says"))
                })?;
                (Form::Stored, stored.to_vec())
            } else {
                let mut out = Vec::with_capacity(word as usize);
                inflate(body, word as usize, &mut out)
                    .map_err(|err| invalid(&format!("block at {offset:#x}: {err}")))?;
                (Form::Packed(len), out)
            }
        };
        if let Some(limit) = limit.filter(|&limit| data.len() > limit) {
            return Err(invalid(&format!(
                "{name} at {offset:#x}: {} bytes for {limit}",
                data.len()
            )));
        }
        self.blocks.push(Block { name, offset, size: self.pos - offset, form, data });
        Ok(self.blocks.last().unwrap())
    }

    fn steps(&mut self, steps: &[Step], rounds: &mut Vec<usize>) -> io::Result<()> {
        for step in steps {
            match *step {
                Object { at, strides } => {
                    self.packed(name(at, strides, rounds), None)?;
                }
                Global { at, size } => {
                    self.packed(format!("{at:#x}"), Some(size))?;
                }
                Local { name, size } => {
                    self.packed(name.into(), Some(size))?;
                }
                Raw { at, strides, size } => {
                    self.raw(name(at, strides, rounds), size)?;
                }
                RawGlobal { at, size } => {
                    self.raw(format!("{at:#x}"), size)?;
                }
                Repeat { count, steps } => {
                    for round in 0..count {
                        rounds.push(round);
                        self.steps(steps, rounds)?;
                        rounds.pop();
                    }
                }
                Players => {
                    for block in 0..PLAYERS.div_ceil(PLAYERS_PER_BLOCK) {
                        let count = PLAYERS_PER_BLOCK.min(PLAYERS - block * PLAYERS_PER_BLOCK);
                        self.packed(format!("players[{block}]"), Some(count * PLAYER))?;
                    }
                }
                Names => {
                    self.packed("names.count".into(), Some(4))?;
                    let size = self.packed("names.size".into(), Some(4))?;
                    let size =
                        size.data.get(..4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
                    self.packed("names".into(), size.map(|size| size as usize))?;
                }
                List { at } => {
                    let offset = self.pos;
                    let count = self.u16()?;
                    let last = self.u16()?;
                    let entries = if count == 0 { 0 } else { usize::from(last) + 1 };
                    self.pos = offset;
                    self.raw(format!("{at:#x}"), 4 + entries * 4000)?;
                }
                PtrList { at } => {
                    let count = self.u16()?;
                    for i in 0..count {
                        self.packed(format!("{at:#x}[{i}]"), Some(0x324))?;
                    }
                }
                Flagged { at, stride } => {
                    let flags = self.blocks.last().map(|b| b.data.clone()).unwrap_or_default();
                    for i in 0..10 {
                        if flags.get(i * 4).is_some_and(|&flag| flag != 0) {
                            self.packed(name(at, &[stride], &[i]), None)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// Lays out a blank save's blocks in the order of the layout, zeroed.
struct Blank {
    blocks: Vec<Block>,
}

impl Blank {
    fn raw(&mut self, name: String, len: usize) {
        self.blocks.push(Block { name, offset: 0, size: 0, form: Form::Raw, data: vec![0; len] });
    }

    fn packed(&mut self, name: String, len: usize) {
        self.blocks.push(Block { name, offset: 0, size: 0, form: Form::Stored, data: vec![0; len] });
    }

    fn steps(&mut self, steps: &[Step], rounds: &mut Vec<usize>) {
        for step in steps {
            match *step {
                Object { at, strides } => {
                    let size = OBJECT_SIZES.iter().find(|&&(base, _)| base == at).map(|&(_, size)| size);
                    self.packed(name(at, strides, rounds), size.expect("every object's size"));
                }
                Global { at, size } => self.packed(format!("{at:#x}"), size),
                Local { name, size } => self.packed(name.into(), size),
                Raw { at, strides, size } => self.raw(name(at, strides, rounds), size),
                RawGlobal { at, size } => self.raw(format!("{at:#x}"), size),
                Repeat { count, steps } => {
                    for round in 0..count {
                        rounds.push(round);
                        self.steps(steps, rounds);
                        rounds.pop();
                    }
                }
                Players => {
                    for block in 0..PLAYERS.div_ceil(PLAYERS_PER_BLOCK) {
                        let count = PLAYERS_PER_BLOCK.min(PLAYERS - block * PLAYERS_PER_BLOCK);
                        self.packed(format!("players[{block}]"), count * PLAYER);
                    }
                }
                Names => {
                    self.packed("names.count".into(), 4);
                    self.packed("names.size".into(), 4);
                    self.packed("names".into(), 0);
                }
                // A u16 count and a u16 last index, both nought.
                List { at } => self.raw(format!("{at:#x}"), 4),
                PtrList { .. } | Flagged { .. } => {}
            }
        }
    }
}

/// Writes a save's blocks in the order of the layout, as [`Reader`] reads
/// them.
struct Writer<'a> {
    save: &'a Save,
    /// Whether blocks whose data stayed the same are written as they were
    /// read.
    keep: bool,
    /// The next block to write.
    next: usize,
    out: Vec<u8>,
}

impl<'a> Writer<'a> {
    /// The next block, which the layout calls `name`.
    fn block(&mut self, name: &str) -> io::Result<&'a Block> {
        let save = self.save;
        let block =
            save.blocks.get(self.next).ok_or_else(|| invalid(&format!("no block {name}")))?;
        if block.name != name {
            return Err(invalid(&format!("{} where the layout has {name}", block.name)));
        }
        self.next += 1;
        Ok(block)
    }

    /// The next block if the layout may have it here.
    fn peek(&self, name: &str) -> bool {
        self.save.blocks.get(self.next).is_some_and(|block| block.name == name)
    }

    fn raw(&mut self, name: &str) -> io::Result<()> {
        let block = self.block(name)?;
        self.out.extend(&block.data);
        Ok(())
    }

    fn packed(&mut self, name: &str) -> io::Result<()> {
        let block = self.block(name)?;
        let read = &self.save.source.0[block.offset..block.offset + block.size];
        if self.keep && unchanged(block, read) {
            self.out.extend(read);
        } else if block.data.is_empty() && block.form == Form::Empty {
            self.out.extend(0u32.to_le_bytes());
        } else {
            let len = i32::try_from(block.data.len()).map_err(|_| invalid("a block of 2 GB"))?;
            self.out.extend((len as u32 + 4).to_le_bytes());
            self.out.extend((-len).to_le_bytes());
            self.out.extend(&block.data);
        }
        Ok(())
    }

    fn steps(&mut self, steps: &[Step], rounds: &mut Vec<usize>) -> io::Result<()> {
        for step in steps {
            match *step {
                Object { at, strides } => self.packed(&name(at, strides, rounds))?,
                Global { at, .. } => self.packed(&format!("{at:#x}"))?,
                Local { name, .. } => self.packed(name)?,
                Raw { at, strides, .. } => self.raw(&name(at, strides, rounds))?,
                RawGlobal { at, .. } => self.raw(&format!("{at:#x}"))?,
                Repeat { count, steps } => {
                    for round in 0..count {
                        rounds.push(round);
                        self.steps(steps, rounds)?;
                        rounds.pop();
                    }
                }
                Players => {
                    for block in 0..PLAYERS.div_ceil(PLAYERS_PER_BLOCK) {
                        self.packed(&format!("players[{block}]"))?;
                    }
                }
                Names => {
                    for name in ["names.count", "names.size", "names"] {
                        self.packed(name)?;
                    }
                }
                List { at } => self.raw(&format!("{at:#x}"))?,
                PtrList { at } => {
                    let mut count = 0u16;
                    while self.peek(&format!("{at:#x}[{count}]")) {
                        count += 1;
                    }
                    self.out.extend(count.to_le_bytes());
                    for i in 0..count {
                        self.packed(&format!("{at:#x}[{i}]"))?;
                    }
                }
                Flagged { at, stride } => {
                    let flags = self.save.blocks[..self.next]
                        .last()
                        .map(|b| b.data.clone())
                        .unwrap_or_default();
                    for i in 0..10 {
                        if flags.get(i * 4).is_some_and(|&flag| flag != 0) {
                            self.packed(&name(at, &[stride], &[i]))?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// Whether a packed block's data is still what its bytes in the file give.
fn unchanged(block: &Block, read: &[u8]) -> bool {
    match block.form {
        Form::Empty => block.data.is_empty(),
        Form::Stored => read.get(8..8 + block.data.len()) == Some(block.data.as_slice()),
        Form::Packed(_) => {
            let mut out = Vec::with_capacity(block.data.len());
            inflate(&read[8..], block.data.len(), &mut out).is_ok() && out == block.data
        }
        Form::Raw => false,
    }
}

/// `0x1a72ff8[17]`: the base address and the rounds it is indexed with.
fn name(at: u32, strides: &[u32], rounds: &[usize]) -> String {
    let mut name = format!("{at:#x}");
    // The innermost loops index the object; outer ones only repeat it.
    for round in &rounds[rounds.len() - strides.len()..] {
        name += &format!("[{round}]");
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_save_writes_and_reads_back() {
        let blank = Save::blank();
        let read = Save::parse(&blank.to_bytes().unwrap()).unwrap();
        assert_eq!(read.header, blank.header);
        let shape = |save: &Save| -> Vec<(String, usize)> {
            save.blocks.iter().map(|block| (block.name.clone(), block.data.len())).collect()
        };
        assert_eq!(shape(&read), shape(&blank));
        assert!(compare(&read, &blank).blocks.is_empty());
        assert!(read.blocks.iter().all(|block| block.data.iter().all(|&b| b == 0)));
    }

    /// A blank save has every block of every save in $ANSTOSS3_DIR/saves,
    /// in the same order and of the same size, but for those whose number
    /// or size the save's data decides: the pool of names, the list at
    /// 0x1be0b40, the pointer list's entries and the flagged objects.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_blank_save_has_every_saves_blocks() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let shape = |save: &Save| -> Vec<(String, usize)> {
            let decided = |name: &str| {
                name == "names" || name == "0x1be0b40" || name.starts_with("0x8eb8b0[") || name.starts_with("0x8ef380[")
            };
            save.blocks.iter().filter(|b| !decided(&b.name)).map(|b| (b.name.clone(), b.data.len())).collect()
        };
        let blank = shape(&Save::blank());
        let mut read = 0;
        for entry in std::fs::read_dir(dir.join("saves")).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|extension| !extension.eq_ignore_ascii_case("an3")) {
                continue;
            }
            let save = Save::read(&path).unwrap();
            assert_eq!(save.header.version, WRITTEN, "{}", path.display());
            let shape = shape(&save);
            let apart = shape.iter().zip(&blank).find(|(a, b)| a != b);
            assert!(apart.is_none() && shape.len() == blank.len(), "{}: {apart:?}", path.display());
            read += 1;
        }
        assert!(read > 0, "no saves in {}", dir.display());
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_saves_decode_to_the_end() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let mut read = 0;
        for entry in std::fs::read_dir(dir.join("saves")).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|extension| !extension.eq_ignore_ascii_case("an3")) {
                continue;
            }
            let save = Save::read(&path).unwrap();
            let players: usize = save
                .blocks
                .iter()
                .filter(|block| block.name.starts_with("players["))
                .map(|block| block.data.len())
                .sum();
            assert_eq!(players, PLAYERS * PLAYER, "{}", path.display());
            let size = save.block("names.size").unwrap().data.clone();
            let size = u32::from_le_bytes(size.try_into().unwrap()) as usize;
            assert_eq!(save.block("names").unwrap().data.len(), size, "{}", path.display());
            // The generator's state, as the probe reads it.
            assert_eq!(save.block("0x89dcc0").unwrap().data.len(), 4);
            assert!(compare(&save, &save).blocks.is_empty());
            read += 1;
        }
        assert!(read > 0, "no saves in {}", dir.display());
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_saves_write_back() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let mut written = 0;
        for entry in std::fs::read_dir(dir.join("saves")).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|extension| !extension.eq_ignore_ascii_case("an3")) {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            let mut save = Save::parse(&bytes).unwrap();
            assert!(
                save.to_bytes().unwrap() == bytes,
                "{} does not write back as it was",
                path.display()
            );
            let stored = Save::parse(&save.to_bytes_stored().unwrap()).unwrap();
            assert!(stored.blocks.iter().all(|block| !matches!(block.form, Form::Packed(_))));
            assert!(compare(&save, &stored).blocks.is_empty(), "{}", path.display());
            // A block that changed is stored; the others stay as they were.
            let players = save.blocks.iter().position(|block| block.name == "players[3]").unwrap();
            save.blocks[players].data[0x19c] ^= 0xff;
            let changed = Save::parse(&save.to_bytes().unwrap()).unwrap();
            assert_eq!(changed.blocks[players].form, Form::Stored);
            assert!(changed.blocks[players].data == save.blocks[players].data);
            assert_eq!(compare(&save, &changed).blocks.len(), 0);
            written += 1;
        }
        assert!(written > 0, "no saves in {}", dir.display());
    }

    /// The dumps `tools/oracle/check.sh` has the original write.
    #[test]
    #[ignore = "reads the oracle's dumps from $ANSTOSS3_ORACLE (tools/oracle/check.sh)"]
    fn the_oracles_dumps_decode() {
        let oracle =
            std::path::PathBuf::from(std::env::var_os("ANSTOSS3_ORACLE").expect("ANSTOSS3_ORACLE"));
        let mut dirs = vec![oracle.join("runs/check")];
        let mut read = 0;
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    dirs.push(path);
                } else if path.extension().is_some_and(|extension| extension == "an3") {
                    let save = Save::read(&path).unwrap();
                    assert!(save.to_bytes().unwrap() == std::fs::read(&path).unwrap());
                    read += 1;
                }
            }
        }
        assert!(read > 0, "no dumps under {}", oracle.display());
    }

    #[test]
    fn a_stored_and_a_packed_block() {
        // A stored block of three bytes, then an empty one.
        let mut bytes = vec![7, 0, 0, 0];
        bytes.extend((-3i32).to_le_bytes());
        bytes.extend([1, 2, 3]);
        bytes.extend(0u32.to_le_bytes());
        let mut file = Reader { bytes: &bytes, pos: 0, blocks: Vec::new() };
        file.packed("a".into(), Some(3)).unwrap();
        file.packed("b".into(), None).unwrap();
        assert_eq!(file.pos, bytes.len());
        assert_eq!(file.blocks[0].data, [1, 2, 3]);
        assert_eq!(file.blocks[0].form, Form::Stored);
        assert_eq!(file.blocks[1].form, Form::Empty);
        // Too long for its global.
        let mut file = Reader { bytes: &bytes, pos: 0, blocks: Vec::new() };
        assert!(file.packed("a".into(), Some(2)).is_err());
    }

    #[test]
    fn names_index_the_inner_loops() {
        assert_eq!(name(0x1a72ff8, &[0x990], &[17]), "0x1a72ff8[17]");
        assert_eq!(name(0x1a2c2f8, &[0xb40, 0xf0], &[3, 7]), "0x1a2c2f8[3][7]");
        assert_eq!(name(0x1a1a698, &[0x2e30, 0x128], &[4, 39]), "0x1a1a698[4][39]");
        assert_eq!(name(0xaf723c, &[], &[]), "0xaf723c");
    }
}
