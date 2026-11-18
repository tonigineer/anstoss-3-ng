//! What the computer clubs do on their own: the week's bulk training
//! (`FUN_0074ca91`). `FUN_0064f981` runs it on the second weekday,
//! before the day's event dice, for every club without a human: the
//! club picks one of the game's own 18 weekly plans out of its squad's
//! condition and freshness, trains each player who is neither away nor
//! a week out — he may keep his complaint, worsen it, or get a new
//! one, and a knock may turn into an operation — and shifts its own
//! tactics by the plan's flags. Since the bulk draws from the shared
//! dice, its draw order is part of the day's.

use std::io;

use a3_files::exe::Exe;
use chrono::NaiveDate;

use crate::load::days_to;
use crate::medical::{ACHILLES, GROIN, OPERATION_DUE};
use crate::random::Random;
use crate::records::{Records, TEMPLATES, TEMPLATE_NAME, TEMPLATE_SIZE};
use crate::statistics::age_on;
use crate::training::{LAST_UNIT, TrainingTables, WEEK};

/// The drill settings per unit (`DAT_008aa590`, 0x34 bytes): freshness
/// and condition as shorts at +0 and +6, the tail's flags word at +0x14,
/// and 26 complaint risks as bytes from +0x18 (`FUN_0074bfc3`).
const DRILL_AT: u32 = 0x8aa590;
const DRILL_SIZE: usize = 0x34;
const RISKS: usize = 0x1a;
/// The game's 18 weekly templates, as in `training::ORIGINAL_WEEKS_AT`.
const WEEKS_AT: u32 = 0x8aad90;
/// A player's complaint candidates by position (`DAT_008ab1d8`, 12
/// shorts as bits; `FUN_0074c113`).
const POSITION_AT: u32 = 0x8ab1d8;
const POSITIONS: usize = 12;
/// What each complaint hurts, for field players and keepers: the two
/// rows of `individual::DIFFICULTIES_AT`.
const SEVERITY_AT: [u32; 2] = [0x8aa4e4, 0x8aa4f4];
const SEVERITY: usize = 16;
/// Bit 11 of the shown word: a smoker, whose condition reads lower
/// (`FUN_006fffd9`; the bit stands as `rollover::SMOKES`).
const SMOKER: u32 = 0x800;

/// One weekly plan, summed over its 21 units as `FUN_0074bfc3` does:
/// the freshness and condition it brings, the tail's flags of what it
/// teaches, and the 26 complaint risks.
#[derive(Clone, Debug, Default)]
struct Plan {
    fresh: i32,
    cond: i32,
    flags: u32,
    risk: [i32; RISKS],
}

/// The game's own training tables for the computer clubs' week.
pub struct Tables {
    plans: Vec<Plan>,
    /// Each position's complaint candidates, as bits.
    positions: Vec<u16>,
    /// What each complaint hurts, field players and keepers.
    severities: [Vec<u8>; 2],
}

impl Tables {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        let drill = exe.bytes(DRILL_AT, DRILL_SIZE * (usize::from(LAST_UNIT) + 1))?;
        let weeks = exe.bytes(WEEKS_AT, TEMPLATES * TEMPLATE_SIZE)?;
        let plans = (0..TEMPLATES)
            .map(|p| {
                let mut plan = Plan::default();
                for u in 0..WEEK {
                    let unit = usize::from(weeks[p * TEMPLATE_SIZE + TEMPLATE_NAME + u]);
                    let row = match drill.get(unit * DRILL_SIZE..(unit + 1) * DRILL_SIZE) {
                        Some(row) => row,
                        None => continue,
                    };
                    plan.fresh +=
                        i32::from(i16::from_le_bytes([row[0], row[1]]));
                    plan.cond += i32::from(i16::from_le_bytes([row[6], row[7]]));
                    plan.flags |= u32::from_le_bytes([row[0x14], row[0x15], row[0x16], row[0x17]]);
                    for i in 0..RISKS {
                        plan.risk[i] += u32::from(row[0x18 + i]) as i32;
                    }
                }
                plan
            })
            .collect();
        Ok(Self {
            plans,
            positions: exe.u16s(POSITION_AT, POSITIONS)?,
            severities: [
                exe.bytes(SEVERITY_AT[0], SEVERITY)?.to_vec(),
                exe.bytes(SEVERITY_AT[1], SEVERITY)?.to_vec(),
            ],
        })
    }
}

/// The week's bulk: every country's clubs without a human, in the
/// game's order (`FUN_0074ca91`), before the day's other dice.
pub fn week(
    records: &mut Records,
    tables: &Tables,
    training: &TrainingTables,
    date: NaiveDate,
    random: &mut Random,
) {
    for country in 1..0x75u16 {
        let count = records.countries.team_count(usize::from(country)).max(0) as u16;
        for number in 0..count {
            let index = match records.team_index(country, number) {
                Some(index) => index,
                None => continue,
            };
            if records.teams[index].humans()[0] != 0 {
                continue;
            }
            club_week(records, tables, training, index, date, random);
        }
    }
}

/// The club's week (`FUN_0074c3f5`): the plan, the squad, the tactics.
fn club_week(
    records: &mut Records,
    tables: &Tables,
    training: &TrainingTables,
    index: usize,
    date: NaiveDate,
    random: &mut Random,
) {
    let squad = records.teams[index].players();
    // The squad's average condition and freshness, out of the players
    // less than a week out.
    let mut cond = 0i16;
    let mut fresh = 0i16;
    let mut count = 0i16;
    for &id in &squad {
        let Some(p) = records.player(id) else { continue };
        if days_to(&records.globals, p.injured_to()) < 7 {
            cond = cond.wrapping_add(cond_percent(p));
            fresh = fresh.wrapping_add(fresh_percent(p));
            count += 1;
        }
    }
    if 0 < count {
        cond /= count;
        fresh /= count;
    }
    // The plan: easy when the squad is tired, the co-trainer's own
    // table when it is fit.
    let plan = if cond < 40 && fresh < 40 {
        if fresh < cond { 13 } else { 1 }
    } else if cond < 40 {
        1
    } else if fresh < 40 {
        13
    } else {
        let [first, second, _] = records.teams[index].staff();
        let staff = if first != 0 { first } else { second };
        let kind = if staff == 0 {
            0
        } else {
            records.persons.get(usize::from(staff)).map(|p| p.training_kind()).unwrap_or(0)
        };
        training.co_template(kind, random.below(100))
    };
    let plan = tables.plans.get(plan).unwrap_or(&tables.plans[0]);
    for &id in &squad {
        // A knock may become an operation here (`FUN_0076785f`); the
        // operated player is then a week out and rests.
        let knocks = match records.player(id) {
            Some(p) => p.knocks(),
            None => continue,
        };
        if knocks & 0xc000 != 0 {
            let out = days_to(&records.globals, records.player(id).unwrap().injured_to());
            let (bit, kind, days) = if knocks & ACHILLES != 0 {
                (ACHILLES, 65, 56)
            } else {
                (GROIN, 66, 42)
            };
            if out < days {
                let today = records.globals.day();
                if let Some(p) = records.player_mut(id) {
                    p.set_injury_kind(kind);
                    p.set_injured_on(today);
                }
                crate::medical::injure_for(records, id, days);
            }
            if let Some(p) = records.player_mut(id) {
                p.set_knocks(knocks & !bit & !OPERATION_DUE);
            }
        }
        // Away, or a week out already: the week skips him.
        let (away, out) = match records.player(id) {
            Some(p) => (
                days_to(&records.globals, p.away_to()),
                days_to(&records.globals, p.injured_to()),
            ),
            None => continue,
        };
        if away != 0 || out >= 7 {
            continue;
        }
        if let Some(p) = records.player_mut(id) {
            player_week(p, plan, tables, date, out < 1, random);
        }
    }
    tactics(&mut records.teams[index], plan, random);
}

/// What the plan day does to one player: the fit one trains and works
/// on his complaint, the others heal.
fn player_week(
    p: &mut crate::records::Player,
    plan: &Plan,
    tables: &Tables,
    date: NaiveDate,
    fit: bool,
    random: &mut Random,
) {
    let mut fresh = p.freshness();
    let mut cond = p.condition();
    if fit {
        // Training with the plan's full gain, scaled by his morale.
        let morale = i32::from(p.morale());
        cond = train(cond, (plan.cond.wrapping_mul(morale) / 10000) as i16);
        fresh[0] = train(fresh[0], (plan.fresh.wrapping_mul(morale) / 10000) as i16);
        fresh[1] = 0;
        fresh[2] = 0;
        p.set_freshness(fresh);
        p.set_condition(cond);
        complaint(p, plan, tables, date, random);
    } else {
        // Half a week out: less of the plan, and freshness grows on
        // its own.
        cond = train(cond, (plan.cond / 200) as i16);
        fresh[0] = train(fresh[0], (plan.fresh / 200) as i16);
        if fresh[0] < 30 {
            fresh[0] = fresh[0].wrapping_add(2);
        } else if fresh[0] < 60 {
            fresh[0] = fresh[0].wrapping_add(1);
        }
        if cond_percent_of(cond, p.shown()) < 20 {
            cond = train(cond, 2);
        } else if cond_percent_of(cond, p.shown()) < 40 {
            cond = train(cond, 1);
        }
        fresh[1] = 0;
        fresh[2] = 0;
        p.set_freshness(fresh);
        p.set_condition(cond);
    }
}

/// What the plan does to the player's complaint (`FUN_0074c3f5`'s
/// middle): none of his good ones means an injury, a bad week brings
/// a new complaint or a fresh injury, and weeks on an old one may end
/// in an injury of their own.
fn complaint(p: &mut crate::records::Player, plan: &Plan, tables: &Tables, date: NaiveDate, random: &mut Random) {
    let ctype = i32::from(p.favourite() & 0x1f);
    if ctype == 0 {
        new_complaint(p, tables, random);
        return;
    }
    if bit_on(p.good(), ctype - 1) {
        p.set_favourite(p.favourite() & 0xe0);
        new_complaint(p, tables, random);
        return;
    }
    // The complaint to hurt: keepers' complaints sit far in the
    // plan's risk row.
    let mut index = ctype - 1;
    if p.positions()[0] == 1 {
        index = ctype + 0xf;
    }
    let draw = random.below(if age_on(p, date) < 23 { 98 } else { 100 });
    let risk = plan.risk.get(usize::try_from(index).unwrap_or(usize::MAX)).copied().unwrap_or(i32::MAX);
    if i32::try_from(draw).unwrap_or(i32::MAX) >= risk {
        return;
    }
    let weeks = i32::from(p.favourite() >> 5);
    if weeks + 1 < 8 {
        p.set_favourite(p.favourite() & 0x1f | ((weeks + 1) as u8) << 5);
    } else {
        worsen(p, tables, index);
        new_complaint(p, tables, random);
    }
}

/// A new complaint (`FUN_0074c113`): out of his bad ones, or, when he
/// has few good ones, out of what his position could still hurt.
fn new_complaint(p: &mut crate::records::Player, tables: &Tables, random: &mut Random) {
    let limit: i32 = if p.positions()[0] == 1 { 9 } else { 16 };
    let bad = p.bad();
    let cands: Vec<i32> = (0..limit).filter(|&i| bit_on(bad, i)).collect();
    if !cands.is_empty() {
        p.set_favourite((cands[random.below(cands.len() as u32) as usize] + 1) as u8);
        return;
    }
    let good = p.good();
    if (0..limit).filter(|&i| bit_on(good, i)).count() < 3 {
        let table = tables.positions.get(usize::from(p.positions()[0])).copied().unwrap_or(0);
        let cands: Vec<i32> = (0..limit)
            .filter(|&i| (table >> i) & 1 == 1 && !bit_on(good, i))
            .collect();
        let ctype = if cands.is_empty() {
            random.below(limit as u32) as i32 + 1
        } else {
            cands[random.below(cands.len() as u32) as usize] + 1
        };
        p.set_favourite(ctype as u8);
        return;
    }
    // Well kept: the complaint clears, its weeks stay.
    p.set_favourite(p.favourite() & 0xe0);
}

/// The complaint the long bad week turns into an injury
/// (`FUN_0074749c`): the bad one goes, or the good one the player was
/// best at; the new injury takes its place among the good marks.
fn worsen(p: &mut crate::records::Player, tables: &Tables, index: i32) {
    if bit_on(p.bad(), index) {
        p.set_bad(p.bad() & !bit_mask(index));
        return;
    }
    if bit_on(p.good(), index) {
        return;
    }
    let keeper = p.positions()[0] == 1;
    let limit: i32 = if keeper { 9 } else { 16 };
    let severity = &tables.severities[usize::from(keeper)];
    let marked = (0..limit).filter(|&i| bit_on(p.good(), i) || bit_on(p.bad(), i)).count();
    if marked == 3 {
        // The mildest of his marks gives way; with no good one, the
        // mildest bad one.
        let mut best = -1i32;
        let mut mild = 5u8;
        for i in 0..limit {
            let hurt = severity.get(usize::try_from(i).unwrap_or(0)).copied().unwrap_or(0);
            if bit_on(p.good(), i) && hurt < mild {
                mild = hurt;
                best = i;
            }
        }
        if best < 0 {
            for i in 0..limit {
                let hurt = severity.get(usize::try_from(i).unwrap_or(0)).copied().unwrap_or(0);
                if bit_on(p.bad(), i) && hurt < mild {
                    p.set_bad(p.bad() & !bit_mask(i));
                    break;
                }
            }
        } else {
            p.set_good(p.good() & !bit_mask(best));
        }
    }
    p.set_good(p.good() | bit_mask(index));
}

/// What the plan teaches the club, and what it forgets again; the
/// flags say which, the rest is seven rolls (`FUN_0074c3f5`'s tail).
fn tactics(team: &mut crate::records::Team, plan: &Plan, random: &mut Random) {
    let flags = plan.flags;
    if flags & 1 != 0 {
        let offside = team.offside().wrapping_add(5);
        team.set_offside(if 200 < offside { 200 } else { offside });
    }
    if flags & 2 != 0 && random.below(100) < 70 {
        team.set_anti_offside(tactic(team.anti_offside(), 1));
    }
    if flags & 4 != 0 && random.below(100) < 70 {
        team.set_set_pieces(field(team.set_pieces(), 3, 0xffc7, 1));
    }
    if flags & 8 != 0 && random.below(100) < 70 {
        team.set_set_pieces(field(team.set_pieces(), 6, 0xfe3f, 1));
    }
    if flags & 0x10 != 0 && random.below(100) < 70 {
        team.set_set_pieces(field(team.set_pieces(), 0, 0xfff8, 1));
    }
    if flags & 0x20 != 0 {
        if random.below(100) < 20 {
            team.set_tactic_extra(tactic(team.tactic_extra(), 1));
        }
        if random.below(100) < 20 {
            team.set_set_pieces(field(team.set_pieces(), 12, 0x8fff, 1));
        }
    }
    if flags & 0x40 != 0 && random.below(100) < 70 {
        team.set_pressing(tactic(team.pressing(), 1));
    }
    if random.below(100) < 20 {
        team.set_anti_offside(tactic(team.anti_offside(), -1));
    }
    if random.below(100) < 20 {
        team.set_set_pieces(field(team.set_pieces(), 3, 0xffc7, -1));
    }
    if random.below(100) < 20 {
        team.set_set_pieces(field(team.set_pieces(), 6, 0xfe3f, -1));
    }
    if random.below(100) < 20 {
        team.set_set_pieces(field(team.set_pieces(), 0, 0xfff8, -1));
    }
    if random.below(100) < 15 {
        team.set_set_pieces(field(team.set_pieces(), 12, 0x8fff, -1));
    }
    if random.below(100) < 15 {
        team.set_tactic_extra(tactic(team.tactic_extra(), -1));
    }
    if random.below(100) < 15 {
        team.set_pressing(tactic(team.pressing(), -1));
    }
}

/// A tactic level 0 to 5, one step (`FUN_00698a4b` and its like).
fn tactic(level: u8, step: i32) -> u8 {
    let level = (i32::from(level) + step) as i16;
    level.clamp(0, 5) as u8
}

/// One three-bit field of the set pieces, one step (`FUN_0069851b`
/// and its like: free kicks at 0, corners at 3, throw-ins at 6, dirty
/// tricks at 12).
fn field(pieces: u16, shift: u16, mask: u16, step: i32) -> u16 {
    let level = (i32::from((pieces >> shift) & 7) + step) as i16;
    let level = level.clamp(0, 5) as u16;
    pieces & mask | level << shift
}

/// A training step on condition or freshness: a hundredth each, the
/// word wraps, then sits within 0 and 10000 (`FUN_00411130`).
fn train(value: i16, steps: i16) -> i16 {
    let value = value.wrapping_add(steps.wrapping_mul(100));
    value.clamp(0, 10000)
}

/// The condition as the game reads it: a smoker's reads five lower,
/// never below zero (`FUN_00457e00`, `FUN_006fffd9`).
pub(crate) fn cond_percent(p: &crate::records::Player) -> i16 {
    cond_percent_of(p.condition(), p.shown())
}

fn cond_percent_of(condition: i16, shown: u32) -> i16 {
    let hundredths = (i32::from(condition) / 100) as u16;
    if shown & SMOKER == 0 {
        hundredths as i16
    } else if (hundredths as i16) < 5 {
        0
    } else {
        hundredths as i16 - 5
    }
}

/// The freshness as a percent: three words of a hundredth, the whole
/// capped (`FUN_006ff649`).
pub(crate) fn fresh_percent(p: &crate::records::Player) -> i16 {
    let [first, second, third] = p.freshness();
    let sum = first.wrapping_add(second).wrapping_add(third);
    if sum < 0 {
        0
    } else if sum < 10001 {
        sum / 100
    } else {
        100
    }
}

/// The game's bit test on the good and bad complaint words: the word
/// reads as a short, so its sign covers the high bits (`1 << i` and
/// `(short)` in the program).
fn bit_on(bits: u16, bit: i32) -> bool {
    i32::from(bits as i16) & (1 << (bit & 0x1f)) != 0
}

/// The game's bit set and clear: the shift is cut to the word, so a
/// keeper's high complaint never marks the word (`(ushort)(1 << i)`).
fn bit_mask(bit: i32) -> u16 {
    (1u32 << (bit & 0x1f)) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plan_table_adds_up() {
        let dir = std::env::var("ANSTOSS3_DIR").unwrap_or_default();
        if dir.is_empty() {
            eprintln!("no installation; the game's tables go unchecked");
            return;
        }
        let exe = Exe::open(&std::path::Path::new(&dir).join("anstoss3.exe")).unwrap();
        let tables = Tables::load(&exe).unwrap();
        assert_eq!(tables.plans.len(), TEMPLATES);
        assert_eq!(tables.positions.len(), POSITIONS);
        // The first of the game's plans, summed over its 21 units: 480
        // freshness short, 380 condition on; its tail's flags word holds
        // the free-kick lesson (0x10) and one bit the tail never looks
        // at (0x40000); the complaint risks stay under 20 apiece.
        assert_eq!((tables.plans[0].fresh, tables.plans[0].cond, tables.plans[0].flags), (-480, 380, 0x40010));
        for plan in &tables.plans {
            assert!(plan.risk.iter().all(|&r| (0..=(21 * 255) as i32).contains(&r)));
        }
    }

    #[test]
    fn the_week_trains_the_computer_clubs() {
        let dir = std::env::var("ANSTOSS3_DIR").unwrap_or_default();
        let game = std::path::Path::new(&dir);
        if dir.is_empty() || !game.join("saves/A.an3").exists() {
            eprintln!("no A among the saves; the week goes unchecked");
            return;
        }
        use a3_files::an3::Save;
        let exe = Exe::open(&game.join("anstoss3.exe")).unwrap();
        let tables = Tables::load(&exe).unwrap();
        let training = TrainingTables::load(&exe).unwrap();
        let mut records =
            Records::from_save(Save::read(&game.join("saves/A.an3")).unwrap()).unwrap();
        let mut clubs = 0;
        for country in 1..0x75u16 {
            let count = records.countries.team_count(usize::from(country)).max(0) as u16;
            clubs += (0..count)
                .filter_map(|number| records.team_index(country, number))
                .filter(|&index| records.teams[index].humans()[0] == 0)
                .count();
        }
        assert!(0 < clubs);
        let before: Vec<u16> =
            (0..records.teams.len()).map(|i| records.teams[i].set_pieces()).collect();
        let mut random = Random::from_state(12345, 67890);
        let [day, month, year] = records.globals.date();
        let date = NaiveDate::from_ymd_opt(i32::from(year), u32::from(month), u32::from(day))
            .unwrap();
        week(&mut records, &tables, &training, date, &mut random);
        // The week drew at least once per computer club, and the dice
        // moved the tactics somewhere.
        assert_ne!(before, (0..records.teams.len()).map(|i| records.teams[i].set_pieces()).collect::<Vec<_>>());
    }
}
