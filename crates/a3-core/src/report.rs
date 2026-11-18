//! The report a human's club keeps of its last match (`0xa53e38[human-1]`,
//! 0x634 a slot, the report's 0x614 bytes at its start), which the
//! Spielbericht after the match (dialog 332) and the statistics' page show.
//!
//! Before the match the pre-match walk empties it and draws the TV
//! station that shows it (`FUN_006ef296`'s fourth loop with
//! `FUN_006fc45a`, `FUN_006fc8c6`); the engine books goals and events into
//! its tail; at the matchday's end `FUN_00550f29` copies the match record
//! over its head and reckons the match's marks — the ball's share, the
//! referee's verdict, the rating, the playing time, the TV's share. The
//! replica scores its matches without the original's engine, so the
//! record carries what the stand-in knows: the sides, the score and the
//! half's, the first leg, the competition, the ground, the crowd and the
//! gate, and the scorers without a minute. The engine's counters —
//! chances, cards, corners, fouls, the referee and the weather — stay at
//! nought, and the marks are reckoned over them as the original does.

use crate::random::Random;
use crate::records::Records;

const REPORTS: u32 = 0xa53e38;
/// The report within its slot (`FUN_006fc45a`'s memset).
pub const REPORT_SIZE: usize = 0x614;
/// The other side's figures stand this far behind the first side's.
const SIDE: usize = 0x1f4;
/// The record's flag byte (+0x1d) after a match played to its end:
/// its minute up and over (`sim::MINUTE_UP | sim::OVER`, as every kept
/// report reads); bit 0 a match that was not played, every mark blank.
const PLAYED: u8 = crate::sim::MINUTE_UP | crate::sim::OVER;
const NOT_PLAYED: u8 = crate::sim::FINISHED;
/// A national side's number.
const NATIONAL: i16 = 200;
/// A goal of the report: ten bytes from +0x49c, their count at +0x498 —
/// the scorer, the man who laid it on, the side (1 the home side), the
/// score after it, its minute and its kind (5 an own goal, 0 a penalty,
/// 100 a penalty missed; above 100 the list shows no minute).
const GOALS: usize = 0x498;
const GOAL_AT: usize = 0x49c;
const GOAL: usize = 10;
const GOALS_MOST: usize = 30;
/// The stand-in's goals have no minute.
pub const NO_MINUTE: u8 = 0xff;
/// The events' count (+0x49a) and the marks: the ball's share (+0x604),
/// the verdict (+0x608), the rating (+0x60a), the playing time (+0x60c),
/// the TV's share (+0x60e) and its station (+0x60f: the country's slot
/// times eight and the station).
const EVENTS: usize = 0x49a;
const SHARE: usize = 0x604;
const VERDICT: usize = 0x608;
const RATING: usize = 0x60a;
const MINUTES: usize = 0x60c;
const TV_SHARE: usize = 0x60e;
const STATION: usize = 0x60f;
/// A station not drawn yet (`FUN_006ef296`'s `local_8c9`).
const NO_STATION: u8 = 99;

/// A match the replica scored, as its record would hold it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Played {
    /// Home side first, by country and number.
    pub sides: [(u8, i16); 2],
    pub goals: [u8; 2],
    pub half: [u8; 2],
    /// +0x20: a league's matchday from 0, else the cup's calendar code.
    pub kind: u32,
    /// +0x0c: 0x10 a second leg, 0x80 extra time, 0x40 penalties.
    pub flags: u8,
    /// +0x08, +0x09: a second leg's first leg, the away side's goals
    /// first.
    pub leg: [u8; 2],
    pub attendance: i32,
    pub gate: i32,
    /// Each side's scorers, a man once for every goal.
    pub scorers: [Vec<u16>; 2],
}

/// The human manager whose report a side's club keeps (`FUN_0063f525`:
/// the club's +0xb5 between 1 and 4).
fn keeper(records: &Records, (country, number): (u8, i16)) -> Option<u8> {
    let team = records.team(country.into(), u16::try_from(number).ok()?)?;
    (1..=4).contains(&team.human()).then(|| team.human())
}

/// The TV station a report names by its +0x60f (`FUN_006810fc`): the slot
/// of the country it broadcasts in, times eight, and its place among the
/// country's stations — the name at the head of its 0x32 bytes in the
/// country's block (`DAT_019fab6a`), none for a country the game does not
/// follow.
pub fn station_name(records: &Records, station: u8) -> Option<String> {
    const STATIONS_AT: usize = 0x2b9a - 0x20;
    const STATION_SIZE: usize = 0x32;
    const NAME: usize = 0x1c;
    let slot = usize::from(station >> 3);
    let country = records.countries.slot_country(slot + 1);
    if !records.globals.played(country) {
        return None;
    }
    let at = STATIONS_AT + usize::from(station & 3) * STATION_SIZE;
    let block = records.block(&format!("0x19f7fd0[{slot}]"))?;
    Some(crate::records::text(block.get(at..at + NAME)?, NAME))
}

/// The human club of a match (the pre-match walk's +0x474, `FUN_00677428`
/// over both sides): the home side where it is a human's, else the away
/// side where that is (`FUN_00551818`); none for the computer's matches.
pub fn human_side(records: &Records, sides: [(u8, i16); 2]) -> Option<(u8, i16)> {
    sides.into_iter().find(|&(c, n)| crate::statistics::is_human(records, c, n))
}

/// Write the reports of a match's human sides: emptied, the station
/// drawn, the match laid over the head, the scorers listed, the marks
/// reckoned. Whether a side kept one.
pub fn write(records: &mut Records, played: &Played, random: &mut Random) -> bool {
    let keepers = played.sides.map(|side| keeper(records, side));
    if keepers.iter().all(Option::is_none) {
        return false;
    }
    // Before the match (`FUN_006fc8c6`): the station of a country the
    // game follows — the home side's for the home report, the away side's
    // for the away one, the other's where it is not followed — and a
    // second report of one country shares the first's.
    let [(home, _), (away, _)] = played.sides;
    let slot = |c: u8| records.countries.slot(usize::from(c));
    let followed = |c: u8| records.globals.played(i16::from(c));
    let mut stations = [NO_STATION; 2];
    if keepers[0].is_some() {
        let from = if followed(home) { home } else { away };
        stations[0] = (random.below(3) as u8).wrapping_add(slot(from).wrapping_mul(8));
    }
    if keepers[1].is_some() {
        stations[1] = if home == away && stations[0] < NO_STATION {
            stations[0]
        } else {
            let from = if followed(away) { away } else { home };
            (random.below(3) as u8).wrapping_add(slot(from).wrapping_mul(8))
        };
    }
    let mut report = vec![0u8; REPORT_SIZE];
    lay_record(&mut report, played);
    lay_goals(&mut report, played);
    let marks = marks(&report, random);
    for (side, keeper) in keepers.into_iter().enumerate() {
        let Some(human) = keeper else { continue };
        let mut own = report.clone();
        own[STATION] = stations[side];
        own[SHARE..SHARE + 2].copy_from_slice(&marks.share.to_le_bytes());
        own[VERDICT..VERDICT + 2].copy_from_slice(&marks.verdict.to_le_bytes());
        own[RATING..RATING + 2].copy_from_slice(&marks.rating.to_le_bytes());
        own[MINUTES..MINUTES + 2].copy_from_slice(&marks.minutes.to_le_bytes());
        own[TV_SHARE] = marks.tv_share;
        if let Some(b) = records.block_mut(&format!("{REPORTS:#x}[{}]", human - 1))
            && let Some(slot) = b.get_mut(..REPORT_SIZE)
        {
            slot.copy_from_slice(&own);
        }
    }
    true
}

/// The match record's fields the stand-in knows, at their places.
fn lay_record(r: &mut [u8], p: &Played) {
    let [(hc, hn), (ac, an)] = p.sides;
    r[0] = hc;
    r[2..4].copy_from_slice(&hn.to_le_bytes());
    r[4] = ac;
    r[6..8].copy_from_slice(&an.to_le_bytes());
    r[8] = p.leg[0];
    r[9] = p.leg[1];
    r[0xc] = p.flags;
    r[0xe..0x10].copy_from_slice(&i16::from(p.goals[0]).to_le_bytes());
    r[0x10..0x12].copy_from_slice(&i16::from(p.goals[1]).to_le_bytes());
    r[0x12] = p.half[0];
    r[0x13] = p.half[1];
    r[0x1c] = (p.half[0].min(15) << 4) | p.half[1].min(15);
    // The flag byte carries the extra time and the penalties as well,
    // which the dialog's score names (`FUN_00554583`).
    r[0x1d] = PLAYED | (p.flags & (crate::sim::EXTRA | crate::sim::PENS));
    r[0x20..0x24].copy_from_slice(&p.kind.to_le_bytes());
    r[0x28..0x2a].copy_from_slice(&u16::from(hc).to_le_bytes());
    r[0x2a..0x2c].copy_from_slice(&hn.to_le_bytes());
    r[0x34..0x38].copy_from_slice(&p.attendance.to_le_bytes());
    r[0x3c..0x40].copy_from_slice(&p.gate.to_le_bytes());
}

/// The scorers, the home side's goals before the away side's, each with
/// the score after it.
fn lay_goals(r: &mut [u8], p: &Played) {
    let mut score = [0u8; 2];
    let mut count = 0;
    for side in 0..2 {
        for &id in &p.scorers[side] {
            if count == GOALS_MOST {
                break;
            }
            score[side] = score[side].saturating_add(1);
            let at = GOAL_AT + count * GOAL;
            r[at..at + 2].copy_from_slice(&id.to_le_bytes());
            r[at + 4] = u8::from(side == 0);
            r[at + 5] = score[0];
            r[at + 6] = score[1];
            r[at + 8] = NO_MINUTE;
            count += 1;
        }
    }
    r[GOALS..GOALS + 2].copy_from_slice(&(count as i16).to_le_bytes());
    r[EVENTS..EVENTS + 2].copy_from_slice(&0i16.to_le_bytes());
}

/// The marks `FUN_00550f29` reckons, the same in both sides' reports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Marks {
    pub share: i16,
    pub verdict: i16,
    pub rating: i16,
    pub minutes: i16,
    pub tv_share: u8,
}

/// The marks of a match from its record (`FUN_00550f29`): the rating by
/// the chances (+0x95, +0x289) and goals, a hard match's cards (yellows
/// +0x130, second yellows +0x138 thrice, reds +0x13a five times, both
/// sides) lowering it; the playing time sixty minutes less the cards, the
/// stoppages (+0xa7) and three for every injury (+0xa0), at least 45; the
/// ball's share 52 and the chances' difference and a draw below seven;
/// the verdict by the fouls (+0x99) and the whistles (+0x45a); the TV's
/// share drawn where it paid (+0x140) — a national side's by its stage
/// (+0x26). A match not played gets the blank marks.
pub fn marks(r: &[u8], random: &mut Random) -> Marks {
    let word = |at: usize| i16::from_le_bytes([r[at], r[at + 1]]);
    let byte = |at: usize| i16::from(r[at]);
    let below = |random: &mut Random, n: u32| random.below(n) as i16;
    let cards = word(0x130) + word(0x138) * 3 + word(0x13a) * 5 + word(0x130 + SIDE) + word(0x138 + SIDE) * 3 + word(0x13a + SIDE) * 5;
    let chances = byte(0x95) + byte(0x95 + SIDE);
    let [home, away] = [word(0xe), word(0x10)];
    let goals = home + away;
    let calm = cards < 0xb;
    let (class, rating) = if chances < 0x14 && (goals < 7 || home < 2 || away < 2) {
        if chances < 0x10 && goals < 6 {
            if chances < 0xc && goals < 4 {
                if chances < 8 && goals < 3 {
                    if chances < 5 && goals < 2 {
                        (6, if calm { below(random, 2) + 0xd } else { 0xc })
                    } else {
                        (5, if calm { 0xb } else { 10 })
                    }
                } else {
                    (4, if calm { below(random, 3) + 7 } else { 6 })
                }
            } else {
                (3, below(random, 3) + 3)
            }
        } else {
            (2, 2)
        }
    } else {
        (1, below(random, 2))
    };
    let stopped = word(0x130) + word(0x138) + word(0x13a) + word(0x130 + SIDE) + word(0x138 + SIDE) + word(0x13a + SIDE);
    let minutes = (0x3c - stopped - byte(0xa7) - byte(0xa7 + SIDE) - (byte(0xa0) + byte(0xa0 + SIDE)) * 3).max(0x2d);
    let share = below(random, 7) + 0x34 + byte(0x95) - byte(0x95 + SIDE);
    let whistles = word(0x45a).max(1);
    let (fouls, foe_fouls) = (byte(0x99), byte(0x99 + SIDE));
    let fouled = fouls + foe_fouls;
    let league = u32::from_le_bytes([r[0x20], r[0x21], r[0x22], r[0x23]]) & 0x7f00_0000 < 0x3f;
    let verdict = if fouls + 2 < foe_fouls && home == away + 1 && league {
        0
    } else if foe_fouls + 2 < fouls && away == home + 1 && league {
        1
    } else if fouls + 4 < foe_fouls {
        2
    } else if foe_fouls + 4 < fouls {
        3
    } else if class >= 5 && fouled == 0 {
        4
    } else if class == 1 && fouled > 3 && fouled * 10 / whistles > 6 {
        5
    } else if whistles < 5 {
        if fouled < 2 { below(random, 4) + 6 } else { below(random, 3) + 10 }
    } else if whistles < 10 {
        if fouled < 2 {
            below(random, 3) + 0xd
        } else if fouled < 5 {
            below(random, 3) + 0x10
        } else {
            below(random, 3) + 0x13
        }
    } else if fouled < 2 {
        below(random, 5) + 0x16
    } else if fouled < 6 {
        below(random, 4) + 0x1b
    } else {
        below(random, 4) + 0x1f
    };
    let paid = i32::from_le_bytes([r[0x140], r[0x141], r[0x142], r[0x143]]);
    let tv_share = if word(2) == NATIONAL {
        (below(random, 10) + 0x32 + word(0x26) * 5) as u8
    } else if paid > 0 && r[TV_SHARE] == 0 {
        (below(random, 10) + 0x32) as u8
    } else if paid == 0 {
        0
    } else {
        r[TV_SHARE]
    };
    if r[0x1d] & NOT_PLAYED != 0 {
        return Marks { share: 0, verdict: 6, rating: 0xe, minutes: 0, tv_share };
    }
    Marks { share, verdict, rating, minutes, tv_share }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Save A's report of Union's 2:0 at Hertha (matchday 3): its
    /// counters give the rating 11 — five chances and one, two goals, no
    /// cards — and the playing time 58 off its two stoppages; one foul
    /// and two whistles leave the verdict to a draw of four from six.
    #[test]
    fn the_marks_follow_the_record_counters() {
        let mut r = vec![0u8; REPORT_SIZE];
        r[0xe] = 2;
        r[0x20] = 2;
        r[0x95] = 5;
        r[0x95 + SIDE] = 1;
        r[0x99 + SIDE] = 1;
        r[0xa7] = 1;
        r[0xa7 + SIDE] = 1;
        r[0x45a] = 2;
        r[0x136] = 1;
        let mut random = Random::new(7);
        let m = marks(&r, &mut random);
        assert_eq!(m.rating, 0xb);
        assert_eq!(m.minutes, 58);
        assert!((6..10).contains(&m.verdict), "{m:?}");
        assert!((56..63).contains(&m.share), "{m:?}");
        assert_eq!(m.tv_share, 0);
    }

    /// A goalless match of no counters is the calmest class: a rating
    /// of 13 or 14, the full sixty minutes, and the verdict 4.
    #[test]
    fn a_match_without_counters_rates_by_its_goals() {
        let mut r = vec![0u8; REPORT_SIZE];
        let mut random = Random::new(3);
        let m = marks(&r, &mut random);
        assert!((13..15).contains(&m.rating), "{m:?}");
        assert_eq!((m.minutes, m.verdict), (60, 4));
        r[0xe] = 3;
        r[0x10] = 1;
        let m = marks(&r, &mut random);
        assert!((3..6).contains(&m.rating), "four goals are the third class: {m:?}");
        assert!((6..10).contains(&m.verdict), "{m:?}");
        r[0xe] = 5;
        let m = marks(&r, &mut random);
        assert_eq!(m.rating, 2, "six goals are the second class");
    }
}
