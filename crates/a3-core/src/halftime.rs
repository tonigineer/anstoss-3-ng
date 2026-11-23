//! Half time of a human club's match: the break (dialog 93,
//! `CHalbzeitpause`) and the talk to the team (dialog 499,
//! `CHalbzeitAnsprache`).
//!
//! At the walk's 45th minute (`FUN_006f352e`) `FUN_0064987a` looks at the
//! day's live flag (`FUN_006ef262`): with it down every side gets the
//! computer's break (`FUN_0064ab9b`); with it up each match side of a
//! human's club (+0x474 bits 1 and 2, the home side first) gets its
//! manager's break block (`0xb5e188 + (manager − 1) · 0x858`, filled by
//! `FUN_0064975d`: the manager, the match's number, whether the club is
//! the home side, the side's 0x1f4 bytes), event 0xb6 opens it
//! (`FUN_00412b4d`, `FUN_0064a816`), and `FUN_006497f8` lays the block's
//! side back on the record. `FUN_0064a816` runs the break until it
//! answers done (100 the break, 200 its tactics, 1 done), then the talk:
//! OK with a line keeps its number, one up from the row, at the block's
//! +0x5f, which the walk hands to `FUN_0064ad03` as the side's +0x47.
//!
//! The replica scores its matches before the dialogs come, so the break
//! shows the half's score it drew and the talk's line is kept for the
//! match it no longer moves: `FUN_0064ad03`'s mood shifts on the side and
//! the club (`FUN_007147ce`) wait with the live match engine, as do the
//! substitutions and the tactics at the break.

use crate::matches::{Faced, Fixture};
use crate::records::{Records, Team};

/// The talks the dialog lists (`DAT_00881a14`), in its order; the line
/// kept is the row plus one.
pub const TALKS: [u16; 12] = [4413, 4414, 4415, 4416, 4417, 4418, 4419, 4420, 4421, 4422, 4425, 4423];

/// One human side's break.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Break {
    /// The human's club, and both sides, home first.
    pub club: (u8, i16),
    pub sides: [(u8, i16); 2],
    /// Whether the club is the home side (the block's +0x08).
    pub home: bool,
    /// The half's score, home first (the record's +0x0e and +0x10 at the
    /// 45th minute).
    pub goals: [u8; 2],
    /// The half's chances, home first (+0x95, +0x289): the replica's
    /// stand-in scores without them.
    pub chances: [u8; 2],
    /// The match's referee (+0x44e), 0 none.
    pub referee: i16,
    /// The talk's line, 1 to 12, 0 none (the block's +0x5f).
    pub talk: u8,
}

impl Break {
    /// The club's goals first (`FUN_006782aa` in `FUN_0064a816`).
    pub fn own_goals(&self) -> [u8; 2] {
        if self.home { self.goals } else { [self.goals[1], self.goals[0]] }
    }
}

/// The breaks of the day's matches with a human's club, before they are
/// played: one for each human side, the home side first, in the order of
/// the matches; none without the live flag. The referee is the one the
/// human club holds for this match (its +0xb50), as the last information
/// reads it.
pub fn breaks(records: &Records, day: usize, fixtures: &[([(u8, i16); 2], Fixture)]) -> Vec<Break> {
    if !crate::prematch::live(records, day) {
        return Vec::new();
    }
    let team = |(country, number): (u8, i16)| records.team(country.into(), u16::try_from(number).ok()?);
    let mut breaks = Vec::new();
    for &(sides, _) in fixtures {
        for (k, &side) in sides.iter().enumerate() {
            if !crate::statistics::is_human(records, side.0, side.1) {
                continue;
            }
            breaks.push(Break {
                club: side,
                sides,
                home: k == 0,
                referee: team(side).map_or(0, Team::referee),
                ..Break::default()
            });
        }
    }
    breaks
}

/// The half's score of each break, once the replica has played its match.
pub fn scores(breaks: &mut [Break], faced: &[Faced]) {
    for b in breaks {
        if let Some(f) = faced.iter().find(|f| f.sides == b.sides) {
            b.goals = f.half;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_club_reads_its_own_goals_first() {
        let home = Break { home: true, goals: [2, 1], ..Break::default() };
        let away = Break { home: false, ..home.clone() };
        assert_eq!(home.own_goals(), [2, 1]);
        assert_eq!(away.own_goals(), [1, 2]);
    }

    #[test]
    fn the_half_comes_from_the_played_match() {
        let sides = [(1, 14), (1, 84)];
        let mut breaks = vec![Break { club: (1, 14), sides, home: true, ..Break::default() }];
        let faced = [Faced { club: (1, 14), sides, attendance: 0, fans: None, half: [0, 1] }];
        scores(&mut breaks, &faced);
        assert_eq!(breaks[0].goals, [0, 1]);
    }
}
