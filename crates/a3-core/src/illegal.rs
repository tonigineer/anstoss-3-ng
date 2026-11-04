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

/// How dear bribes come by the club's league (`FUN_004c8d07`): in the
/// countries 63 and 28 fifteen, else ten in a first league, four in a
/// second, two below.
pub fn league_factor(country: u8, league: u8) -> i64 {
    match (country, league) {
        (63 | 28, _) => 15,
        (_, 0) => 10,
        (_, 1) => 4,
        _ => 2,
    }
}

/// What makes a referee's bribe sure, by the league's factor
/// (`DAT_00a3b3a0` = 30 · 1000), and the unit its chance counts in
/// (`DAT_00a3b328` = 1250 · 1000).
const REFEREE_SURE: i64 = 30_000;
const REFEREE_UNIT: i64 = 1_250_000;

/// What the referee promises, by the bits his bribe sets (+0x1a8): a
/// penalty at the first dive, a red card at the first chance, a generous
/// offside rule; texts 4549 to 4551.
pub const REFEREE_PROMISES: [(u16, u32); 3] =
    [(4549, 0x1000_0000), (4550, 0x0800_0000), (4551, 0x0400_0000)];
/// His greeting when the money is sure to do it (text 4552), and his
/// refusals (`0x882500`).
pub const REFEREE_GREETING: u16 = 4552;
pub const REFEREE_REFUSALS: [u16; 5] = [4553, 4554, 4555, 4556, 4557];
/// A bribed referee (+0x1a8).
pub const REFEREE_BRIBED: u32 = 0x40;

/// The referee asked (`OnClicked_IDC_SCHIEDSRICHTER_OK`): sure where he
/// takes money and the amount reaches thirty thousand by the league's
/// factor; else by chance, a hundredth of the amount in 12.500s by the
/// factor above a hundred — which takes over 1.25 million. Returns whether
/// he takes it and whether it was sure; `below` draws the game's random
/// numbers.
pub fn bribe_referee(
    amount: i64,
    corruptible: bool,
    factor: i64,
    below: &mut impl FnMut(u32) -> u32,
) -> (bool, bool) {
    let chance = if corruptible { amount * 10 / REFEREE_UNIT * factor } else { 0 };
    let sure = corruptible && amount >= REFEREE_SURE * factor;
    (sure || i64::from(below(100)) < chance - 100, sure)
}

/// What the next opponent's captain asks (`FUN_004ca876`): 500.000; past
/// the season's day 300, against a side in places 9 to 12, 500.000 and
/// up to five more 100.000s by chance, times the league's factor.
pub fn opponent_price(day: i16, place: u8, factor: i64, below: &mut impl FnMut(u32) -> u32) -> i64 {
    const BASE: i64 = 500_000;
    const MORE: i64 = 100_000;
    if day > 300 && (9..=12).contains(&place) {
        (i64::from(below(6)) * MORE + BASE) * factor
    } else {
        BASE
    }
}

/// The opponent bribed to lose (+0x1a8), and the captain's answers:
/// yes (4603, 4604, one of two by chance), no (4605-4607).
pub const OPPONENT_BRIBED: u32 = 0x10;
pub const OPPONENT_YES: [u16; 2] = [4604, 4603];
pub const OPPONENT_NO: [u16; 3] = [4605, 4606, 4607];

/// A player of the next opponent bribed (+0x1a8): he takes any amount
/// with scruples under 85, else more than 100.000 and 20.000 for each
/// point of his strength (`DAT_00a3b37c`, `DAT_00a3b3ac`).
pub const PLAYER_BRIBED: u32 = 0x4;
pub fn player_takes(amount: i64, scruples: u8, strength: u8) -> bool {
    scruples < 85 || amount > 100_000 + 20_000 * i64::from(strength)
}
/// His answers: yes (4612, 4613, one of two by chance), no (4614-4616).
pub const PLAYER_YES: [u16; 2] = [4613, 4612];
pub const PLAYER_NO: [u16; 3] = [4614, 4615, 4616];

/// Another team's opponents incited (+0x1a8): the money is enough when it
/// reaches a hundred times the average salary of the manager's own next
/// opponent's squad — the side the original sums, not the one chosen;
/// a squad of none gives a chance of 12 in 100 (`OnClicked_IDC_AUFSTACHELN_OK`,
/// `DAT_00a3b3fc` = 100).
pub const INCITED_DONE: u32 = 0x1;
pub fn incite_takes(amount: i64, salaries: &[u16], below: &mut impl FnMut(u32) -> u32) -> bool {
    if salaries.is_empty() {
        return below(100) < 12;
    }
    let sum: i64 = salaries.iter().map(|&s| i64::from(s)).sum();
    amount >= sum / salaries.len() as i64 * 100
}
/// The captain's answers: yes (4608, 4609), no (4610, 4611), one of two
/// by chance.
pub const INCITE_YES: [u16; 2] = [4609, 4608];
pub const INCITE_NO: [u16; 2] = [4611, 4610];

/// The attacks on another human (`OnClicked_IDC_ANSCHLAG_OK`): a player
/// put out, the merchandise destroyed, the black cash box robbed — what
/// each costs (`DAT_00a3b350`, `DAT_00a3b35c`, `DAT_00a3b36c`) and the bit
/// it sets (+0x1a8), closing all three for the season; "Ist in Auftrag!"
/// (4600), or with too little in the box 4601.
pub const ATTACKS: [(i64, u32); 3] = [(400_000, 0x8000_0000), (300_000, 0x2000_0000), (200_000, 0x4000_0000)];
pub const ATTACK_ORDERED: u16 = 4600;
pub const ATTACK_TOO_DEAR: u16 = 4601;
