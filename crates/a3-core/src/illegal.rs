//! The black cash box (dialog 442, `CDiIllegales`; the Finanzen screen's
//! picture button 2224): money kept from the gates and the cup ties, and
//! what it may buy — the referee, another team's opponents, the next
//! opponent or one of his players, an attack on another human's club —
//! or where it may go: into the books or the manager's pocket.

use crate::records::Records;

/// The amounts' slider scale (`0x8824b0`): to 10.000 in hundreds, to
/// 100.000 in thousands, then in ten thousands.
pub const STEPS: [(i64, i64); 3] = [(10_000, 100), (100_000, 1000), (crate::finances::ON, 10_000)];

/// What is closed for the season, bits of the manager's +0x1a8 (object
/// +0xf8): the referee bribed, another team incited, a player bribed, the
/// opponent bribed, and the attacks.
pub const REFEREE_DONE: u32 = 0x80;
pub const INCITED: u32 = 0x2;
pub const PLAYER_DONE: u32 = 0x8;
pub const OPPONENT_DONE: u32 = 0x20;
pub const ATTACKS_DONE: u32 = 0xe000_0000;
/// The turns in which the box may be opened: up to this half-week of the
/// season (`FUN_004c74f8`, `DAT_00b5cbd2`); after it text 4599.
pub const LAST_HALF_WEEK: i16 = 92;

/// A match of the day's list (`0x1baf714`, 0x498 bytes each, its count at
/// `0x1baf710`; the save's block `0x1baf6f0`): both sides by country and
/// number, and which of them may not be incited (+0x474: bit 0 the home
/// side, bit 1 the away side).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pairing {
    pub home: (u8, i16),
    pub away: (u8, i16),
    pub closed: u16,
}

const MATCH_LIST: &str = "0x1baf6f0";
const PAIRING: usize = 0x498;
const PAIRING_CLOSED: usize = 0x474;

/// The day's matches, as the save keeps the last day's.
pub fn pairings(records: &Records) -> Vec<Pairing> {
    let Some(block) = records.block(MATCH_LIST) else { return Vec::new() };
    let word = |at: usize| block.get(at..at + 2).map_or(0, |b| i16::from_le_bytes([b[0], b[1]]));
    let count = block.get(..4).map_or(0, |b| i32::from_le_bytes(b.try_into().unwrap()));
    (0..usize::try_from(count).unwrap_or(0))
        .map(|i| 4 + i * PAIRING)
        .take_while(|&at| at + PAIRING <= block.len())
        .map(|at| Pairing {
            home: (block[at], word(at + 2)),
            away: (block[at + 4], word(at + 6)),
            closed: word(at + PAIRING_CLOSED) as u16,
        })
        .collect()
}

/// The most the gates may give up for the box, in tenths of a percent:
/// 3% in a first league, 5% below (club +0x105).
pub fn gate_most(league: u8) -> i32 {
    if league < 2 { 30 } else { 50 }
}

/// The cup opponents' share at most kept back, in tenths of a percent.
pub const CUP_SHARE_MOST: i32 = 50;
