//! The cup results after the matchday (dialog 161, `CPokalNachSpieltag`):
//! which national cups played a round today, and which drew the next one.
//!
//! At the matchday's end (`FUN_007108d3`) `FUN_0062eac3` empties the day's
//! cup block at `0xa3b488` (`FUN_0062dd51`) and walks the main country,
//! then each further played country, by the code its first league row
//! holds for the day (`FUN_00698f51`): a national cup's day (0x40 in the
//! top bits, either leg) notes its round (the low six bits) under the
//! country's slot on the results page (`0xa3b490`) — a further country's
//! only from its cup's first round on (the cup's +0x26). Once every
//! pairing of the round stands decided (0x08, `FUN_006d130a`; a round
//! before the first counts as done), a round below the final draws the
//! next (`FUN_006cdfb7`, the replica's draw standing in
//! [`crate::matches`]) and notes it on the draw page (`0xa3b4c4`, one
//! up). With anything noted, event 0xc9 follows from the season's 42nd
//! day, or before it with the preparation normal (`DAT_00b5d152`, 5);
//! `FUN_0052b2bf` copies the block into the dialog (`FUN_0062e6c5`) and
//! opens it with "Pokalergebnisse" on (byte 9 of the options,
//! `DAT_00b5d145`).
//!
//! Left out: the league cup (0x01), the supercup (0x30) and the
//! relegation games (0x10, `FUN_0062e8fb`, `FUN_0062e9b7`) on the same
//! row, the European cups (`FUN_0062dfa3`) and the internationals
//! (`FUN_0062f34f`), which fill the other parts of the block and open the
//! dialog by the same event; and the cup's prize and bonus dialogs the
//! entries behind it (`0xa3b4f8`, `FUN_0077d510`) open after it.

use crate::calendar::Calendar;
use crate::opponent::{CUPS_AT, PAIRING, PAIRINGS, cup_pairings};
use crate::records::Records;

/// The block's slots: one a played country, by its slot.
pub const SLOTS: usize = 5;
/// The final, the last round: no round is drawn behind it.
pub const FINAL_ROUND: i16 = 6;
/// A national cup's day on the country's first league row.
const CUP_CODE: u32 = 0x4000_0000;
const KIND: u32 = 0x7f00_0000;
const ROUND: u32 = 0x3f;
/// A pairing decided (`FUN_004197ea`).
const DECIDED: u8 = 0x08;
/// The cup's first round (+0x26 of the cup object).
const FIRST_ROUND: usize = 0x26 - 0x20;
const MODE: usize = 0x24 - 0x20;
/// From this day on the event follows every cup day (`0x29 <
/// DAT_00b5cbd4`); before it only with the preparation normal.
const SHOWN_FROM: usize = 42;
const NORMAL_PREPARATION: u8 = 5;
/// "Pokalergebnisse": the dialog's switch among the options at
/// `0xb5d13c` (`DAT_00b5d145`).
const SWITCH: usize = 0x09;

/// The national cups' part of the day's block, by slot: the round played
/// today (`0xa3b490`, the results page) and the round drawn behind it
/// (`0xa3b4c4`, the draw page), −1 where none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CupDay {
    pub results: [i16; SLOTS],
    pub draws: [i16; SLOTS],
}

impl Default for CupDay {
    fn default() -> Self {
        Self { results: [-1; SLOTS], draws: [-1; SLOTS] }
    }
}

impl CupDay {
    /// Whether the page holds a round (`FUN_0052c679`'s and OK's tests,
    /// of the national cups' part).
    pub fn any(&self, draw: bool) -> bool {
        let page = if draw { &self.draws } else { &self.results };
        page.iter().any(|&r| r >= 0)
    }
}

/// The day's national cups as `FUN_0062eac3` notes them, once the day's
/// games are played; `None` where the dialog does not open.
pub fn day(records: &Records, calendar: &Calendar, day: usize) -> Option<CupDay> {
    let globals = &records.globals;
    let mut noted = CupDay::default();
    let main = globals.main_country();
    let further = globals.more_countries();
    let more = usize::try_from(globals.more_count()).unwrap_or(0).min(further.len());
    let countries = std::iter::once(main).chain(further[..more].iter().copied());
    for country in countries {
        let Ok(nation) = u16::try_from(country) else { continue };
        let Some(row) = calendar.row(nation, 0) else { continue };
        let code = calendar.entry(row, day);
        if code & KIND != CUP_CODE {
            continue;
        }
        let slot = usize::from(records.countries.slot(usize::from(nation)));
        if slot >= SLOTS {
            continue;
        }
        let Some(cup) = records.block(&format!("{CUPS_AT:#x}[{slot}]")) else { continue };
        let round = (code & ROUND) as i16;
        let first = cup.get(FIRST_ROUND..FIRST_ROUND + 2).map_or(0, |b| i16::from_le_bytes([b[0], b[1]]));
        if country != main && round < first {
            continue;
        }
        noted.results[slot] = round;
        if decided(cup, round, first) && round < FINAL_ROUND {
            noted.draws[slot] = round + 1;
        }
    }
    let event = (noted.any(false) || noted.any(true))
        && (day >= SHOWN_FROM || globals.preparation() == NORMAL_PREPARATION);
    (event && globals.game_options()[SWITCH] == 1).then_some(noted)
}

/// Whether every pairing of the round stands decided (`FUN_006d130a`):
/// a round before the cup's first is.
fn decided(cup: &[u8], round: i16, first: i16) -> bool {
    if round < first {
        return true;
    }
    let mode = usize::from(cup.get(MODE).copied().unwrap_or(0)).min(5);
    let Some((start, count)) = usize::try_from(round).ok().and_then(|r| cup_pairings(mode, r)) else {
        return true;
    };
    (start..start + count).all(|q| cup.get(PAIRINGS + q * PAIRING + 0xc).is_some_and(|&f| f & DECIDED != 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_holds_a_round_where_a_slot_names_one() {
        let mut day = CupDay::default();
        assert!(!day.any(false) && !day.any(true));
        day.results[3] = 1;
        assert!(day.any(false) && !day.any(true));
        day.draws[0] = 2;
        assert!(day.any(true));
    }

    #[test]
    fn a_round_stands_decided_once_each_pairing_is() {
        let mut cup = vec![0u8; PAIRINGS + 64 * PAIRING];
        cup[MODE] = 0;
        let (start, count) = cup_pairings(0, 1).unwrap();
        for q in start..start + count {
            cup[PAIRINGS + q * PAIRING + 0xc] = 0x28;
        }
        assert!(decided(&cup, 1, 1));
        cup[PAIRINGS + start * PAIRING + 0xc] = 0x20;
        assert!(!decided(&cup, 1, 1));
        // A round before the cup's first counts as done.
        assert!(decided(&cup, 1, 2));
    }
}
