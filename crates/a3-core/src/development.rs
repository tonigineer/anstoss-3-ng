//! The played countries' players develop (`FUN_0040f99c`, per club
//! `FUN_0040fa91`): at the season's end before its retirements, and at
//! the winter break (the 56th half week, [`half_week`]) without the
//! season's matches.
//!
//! Every club in a league notes each player's strength into his history
//! (`FUN_00705001`) and draws his change, one point up or down at a time:
//! by his age the chance to rise (40 below 22 — 20 below 22 with the human
//! trainer's youth work on top, all of it below 20 —, 20 below 26) or else
//! to fall (10 for a keeper or a defender and 20 for the others from 30,
//! 30 from 34, 50 from 40), weighed by his talent and never above what it
//! reaches; his season's form (`FUN_006ff257`) as far above or below ten
//! as the chance to rise or fall again. At the season's end the club's
//! goals help: a striker who played half the matches of a club scoring
//! more than three a match may rise, and so may one of its defenders when
//! it let in less than one. A weak player in his late twenties gains a
//! point. A human manager's players rise by playing and scoring, and fall
//! by sitting out (the thresholds by strength and league at 0x876d86 and
//! 0x876e40). The change stays within two points, the league's ceiling
//! (13, 9 in the second league, 7 below) and 1; a player who rose from 8
//! to 11 may take a quality (0x40 or 0x200 of his second character word),
//! one far below his best is marked (R+0x27f bit 6). The human trainer's
//! young risers bring him youth marks, the sum of the changes moves the
//! human manager's nerves.
//!
//! Left out: the report the human sees of it (`FUN_00410f40`,
//! `FUN_00410e60`, event 0xca).

use chrono::NaiveDate;

use crate::random::Random;
use crate::records::{Player, Records};

/// The exe's bytes from 0x876d00: the thresholds by strength and league
/// (0x876d86, two a strength, a row of 0x1a a league) and by league for
/// the weakest (0x876e40, a row of 12), read where the original reads them
/// — rows before the first for short leagues included.
const TABLES_AT: usize = 0x876d00;
const TABLES: [u8; 416] = [
    0x20, 0x25, 0x64, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x25, 0x73, 0x20, 0x67, 0x65, 0x68, 0x74, 0x20,
    0x6e, 0x61, 0x63, 0x68, 0x20, 0x25, 0x73, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x6b, 0x65, 0x69, 0x6e,
    0x20, 0x56, 0x65, 0x72, 0x65, 0x69, 0x6e, 0x20, 0x66, 0xfc, 0x72, 0x20, 0x6d, 0x69, 0x74, 0x20,
    0x25, 0x73, 0x0a, 0x00, 0x6e, 0x69, 0x63, 0x68, 0x74, 0x20, 0x68, 0x61, 0x75, 0x70, 0x74, 0x20,
    0x77, 0x65, 0x67, 0x20, 0x6d, 0x69, 0x74, 0x20, 0x25, 0x73, 0x0a, 0x00, 0x8c, 0x7c, 0x83, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x2e, 0x3f, 0x41, 0x56, 0x43, 0x44, 0x41, 0x46, 0x69, 0x6c, 0x65, 0x40,
    0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x64, 0x00, 0x64, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x64, 0x00, 0x64, 0x00, 0x64, 0x00, 0x64, 0x00, 0x64, 0x00, 0x64, 0x00, 0x64, 0x00, 0x64, 0x00,
    0x64, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x02, 0x03, 0x03, 0x05, 0x05, 0x06, 0x06,
    0x09, 0x09, 0x0b, 0x0b, 0x10, 0x0e, 0x15, 0x12, 0x18, 0x14, 0x1b, 0x15, 0x1e, 0x17, 0x63, 0x63,
    0x63, 0x63, 0x02, 0x02, 0x03, 0x03, 0x04, 0x04, 0x05, 0x05, 0x07, 0x07, 0x08, 0x08, 0x0d, 0x0c,
    0x12, 0x0f, 0x14, 0x10, 0x17, 0x12, 0x19, 0x13, 0x63, 0x63, 0x63, 0x63, 0x02, 0x02, 0x03, 0x03,
    0x05, 0x05, 0x06, 0x06, 0x08, 0x08, 0x09, 0x09, 0x0d, 0x0c, 0x13, 0x10, 0x15, 0x11, 0x18, 0x12,
    0x1b, 0x16, 0x63, 0x63, 0x63, 0x63, 0x02, 0x02, 0x03, 0x03, 0x05, 0x05, 0x06, 0x06, 0x08, 0x08,
    0x0a, 0x0a, 0x0e, 0x0c, 0x13, 0x10, 0x15, 0x11, 0x18, 0x12, 0x1e, 0x18, 0x63, 0x63, 0x63, 0x63,
    0x02, 0x02, 0x03, 0x03, 0x05, 0x05, 0x06, 0x06, 0x08, 0x08, 0x0a, 0x0a, 0x0f, 0x0e, 0x15, 0x12,
    0x17, 0x12, 0x1a, 0x13, 0x20, 0x1a, 0x63, 0x63, 0x63, 0x63, 0x02, 0x02, 0x03, 0x03, 0x06, 0x06,
    0x07, 0x07, 0x09, 0x09, 0x0b, 0x0b, 0x11, 0x0f, 0x17, 0x14, 0x19, 0x14, 0x1c, 0x16, 0x23, 0x1c,
    0x63, 0x63, 0x63, 0x63, 0x02, 0x02, 0x04, 0x04, 0x06, 0x06, 0x08, 0x08, 0x09, 0x09, 0x0c, 0x0c,
    0x12, 0x11, 0x19, 0x16, 0x1c, 0x16, 0x1f, 0x17, 0x26, 0x1e, 0x63, 0x63, 0x63, 0x63, 0x00, 0x00,
    0x03, 0x01, 0x05, 0x02, 0x08, 0x04, 0x0b, 0x05, 0x00, 0x19, 0x00, 0x1f, 0x03, 0x01, 0x05, 0x02,
    0x08, 0x04, 0x0b, 0x05, 0x00, 0x14, 0x00, 0x19, 0x03, 0x01, 0x05, 0x02, 0x08, 0x04, 0x0b, 0x05,
    0x00, 0x16, 0x00, 0x1c, 0x03, 0x01, 0x05, 0x02, 0x08, 0x04, 0x0b, 0x05, 0x00, 0x19, 0x00, 0x1f,
    0x03, 0x01, 0x05, 0x02, 0x09, 0x04, 0x0c, 0x05, 0x00, 0x1b, 0x00, 0x22, 0x04, 0x01, 0x06, 0x02,
    0x0b, 0x05, 0x0d, 0x06, 0x00, 0x1e, 0x00, 0x25, 0x04, 0x01, 0x06, 0x02, 0x0c, 0x06, 0x0e, 0x06,
    0x00, 0x21, 0x00, 0x29, 0x0c, 0x00, 0x0b, 0x00, 0x0a, 0x00, 0x09, 0x00, 0x08, 0x00, 0x00, 0x00
];
const BY_STRENGTH: usize = 0x876d86;
const BY_STRENGTH_ROW: i32 = 0x1a;
const WEAKEST: usize = 0x876e40;
const WEAKEST_ROW: i32 = 12;

fn table(at: i64) -> i32 {
    usize::try_from(at - TABLES_AT as i64).ok().and_then(|k| TABLES.get(k)).map_or(0, |&b| i32::from(b))
}

/// R+0x271's top two bits at 1: he is passed over (`FUN_00706a7e`).
const PASSED_OVER: u8 = 0x40;
/// R+0x27f's bit 6: far below his best (`FUN_0070785e`).
const BELOW_BEST: u8 = 0x40;
/// The human manager's nerves, 0 to 100 (the manager object's +0x32).
const NERVES: usize = 0x11e;
/// Youth marks a young riser brings the human trainer (`FUN_006dd5f1`).
const YOUTH_MARKS: i32 = 10;
const HISTORY: usize = 0x2bd;
const HISTORY_LEN: usize = 20;
/// R+0x274, R+0x23d, R+0x23e: the goals the playing rise counts; R+0x243
/// to +0x245 the second count beside them; R+0x247 the assists.
const COUNTED_GOALS: [usize; 3] = [0x274, 0x23d, 0x23e];
const SECOND_COUNT: [usize; 3] = [0x243, 0x244, 0x245];
const ASSISTS: usize = 0x247;
/// The matches of the cup kinds and of the European ones (R+0x25e,
/// R+0x264) beside the league's, and the two league counts behind it.
const CUP_MATCHES: usize = 0x25e;
const EUROPE_MATCHES: usize = 0x264;

fn today(records: &Records) -> Option<NaiveDate> {
    let [day, month, year] = records.globals.date();
    NaiveDate::from_ymd_opt(i32::from(year), u32::from(month), u32::from(day))
}

/// A club's league matches in a season (`FUN_00692997`).
fn season_matches(records: &Records, country: u16, level: u8) -> i32 {
    if level >= 6 || !records.globals.played(country as i16) {
        return 18;
    }
    if i32::from(level) < records.countries.leagues(usize::from(country)) as i32 {
        crate::opponent::League::of(records, country, level).map_or(0, |l| l.matchdays() as i32)
    } else {
        34
    }
}

/// His season's form times ten (`FUN_006ff257`): 100 with none.
fn form(p: &Player) -> i32 {
    match p.season_form_weeks() {
        0 => 100,
        weeks => i32::from(p.season_form()) * 10 / i32::from(weeks),
    }
}

/// His best strength: now or in his history (`FUN_00704f34`).
fn best(p: &Player) -> u8 {
    (0..HISTORY_LEN).map(|k| p.get::<u8>(HISTORY + k)).fold(p.strength(), |b, h| b.max(h & 0xf).max(h >> 4))
}

/// What the club brings to every player's development.
struct Club {
    matches: i32,
    row: i32,
    scored: f32,
    let_in: f32,
    youth_work: f64,
    ceiling: i16,
    trainer: u8,
    manager: u8,
}

/// The played countries' players develop (`FUN_0040f99c`); `half` at the
/// winter break, else at the season's end.
pub fn develop_played(records: &mut Records, half: bool, random: &mut Random) {
    for country in crate::opponent::played_nations(records) {
        for number in 0..records.countries.team_count(usize::from(country)).max(0) as u16 {
            develop_club(records, country, number, half, random);
        }
    }
}

fn develop_club(records: &mut Records, country: u16, number: u16, half: bool, random: &mut Random) {
    let Some(team) = records.team(country, number) else { return };
    let level = team.level();
    if level >= 10 {
        return;
    }
    let matches = season_matches(records, country, level);
    let [trainer, manager] = team.humans();
    let [scored, let_in] = team.goals();
    let youth_work = usize::from(trainer)
        .checked_sub(1)
        .and_then(|slot| records.managers.get(slot))
        .map_or(0, |m| m.abilities()[m.standing().clamp(0, 15) as usize][6]);
    let mut club = Club {
        matches,
        row: (matches - 22) / 4,
        scored: f32::from(scored) / matches as f32,
        let_in: f32::from(let_in) / matches as f32,
        youth_work: f64::from(youth_work),
        ceiling: match level {
            0 => 13,
            1 => 9,
            _ => 7,
        },
        trainer,
        manager,
    };
    let squad = team.players();
    let today = today(records);
    let mut sum = 0i16;
    for id in squad {
        let Some(p) = records.player(id) else { continue };
        if p.picks() >> 6 == PASSED_OVER >> 6 {
            continue;
        }
        let age = today.map_or(0, |d| crate::statistics::age_on(p, d));
        let mut p = p.clone();
        let (drawn, young_riser) = develop_player(&mut p, age, &mut club, half, random);
        if club.trainer != 0 {
            sum += drawn;
        }
        if let Some(slot) = records.player_mut(id) {
            *slot = p;
        }
        if young_riser
            && let Some(m) = usize::from(club.trainer).checked_sub(1).and_then(|s| records.managers.get_mut(s))
        {
            m.set_youth_marks(m.youth_marks() + YOUTH_MARKS);
        }
    }
    if club.manager != 0 && (club.trainer != 0 || club.manager != 0)
        && let Some(m) = usize::from(club.manager).checked_sub(1).and_then(|s| records.managers.get_mut(s))
    {
        let nerves = (m.get::<i16>(NERVES) + sum).clamp(0, 100);
        m.put::<i16>(NERVES, nerves);
    }
}

/// One player's development (`FUN_0040fa91`'s loop): his change as drawn,
/// before the bounds, and whether he is a young riser of a human trainer.
fn develop_player(p: &mut Player, age: i32, club: &mut Club, half: bool, random: &mut Random) -> (i16, bool) {
    let strength = p.strength();
    crate::rollover::note_strength(p, strength);
    let talent = p.talent();
    let top = i16::from(crate::rollover::top_strength(talent));
    let mut ceiling = club.ceiling.min(top);
    let position = p.positions()[0];
    let s = i32::from(strength);
    let byte = |p: &Player, at: usize| i32::from(p.get::<u8>(at));
    let league = byte(p, 0x253);
    let subs = byte(p, 0x254);
    let benched = byte(p, 0x255);
    let all = league + byte(p, EUROPE_MATCHES) + byte(p, CUP_MATCHES);
    let started = league - subs - benched;
    let (mut up_bits, mut down_bits) = (0u16, 0u16);
    let mut change: i16 = 0;
    let row = i64::from(club.row);
    let weakest = |k: i64| table(WEAKEST as i64 + row * i64::from(WEAKEST_ROW) + k);
    if club.manager != 0 && !half {
        let playing = match strength {
            1 => league + subs / 2 >= 4 || age < 30 && weakest(0) < all && weakest(1) < started,
            2 => all + subs / 2 >= 7 || age < 30 && weakest(2) < all && weakest(3) < started,
            3 => club.matches * 2 / 3 < all || age < 34 && weakest(5) < all - subs - benched,
            4 => age < 34 && weakest(7) < all - subs - benched,
            5 => age < 30 && weakest(8) < all && weakest(9) < started,
            6 => age < 30 && weakest(10) < all && weakest(11) < started,
            _ => false,
        };
        if playing {
            up_bits |= 1;
            change += 1;
        }
        if age < 34 && strength < 13 {
            let goals: i32 = COUNTED_GOALS.iter().map(|&at| byte(p, at)).sum();
            let second: i32 = SECOND_COUNT.iter().map(|&at| byte(p, at)).sum();
            let by_strength = |k: i64| table(BY_STRENGTH as i64 + i64::from(s) * 2 + row * i64::from(BY_STRENGTH_ROW) + k);
            if position == 10 && goals + byte(p, ASSISTS) / 3 < s * 9 / 10 && club.matches * 6 / 10 < league {
                down_bits |= 2;
                change -= 1;
            } else if position != 1 && (by_strength(0) < goals || by_strength(1) < second) {
                up_bits |= 2;
                change += 1;
            }
        }
    }
    let form = form(p);
    let form_f = form as f32;
    if club.manager != 0 && !half && age > 19 {
        if strength < 11 {
            if up_bits & 1 == 0 {
                let cup = byte(p, CUP_MATCHES);
                if league + cup == 0 {
                    if position != 1 || random.below(2) != 0 {
                        down_bits |= 0x20;
                        change -= 1;
                    }
                } else if league == subs && league < 5 && (position != 1 || random.below(5) == 0) {
                    down_bits |= 0x20;
                    change -= 1;
                }
            }
        } else if league < club.matches / 3 && (position != 1 || random.below(5) == 0) {
            down_bits |= 0x20;
            change -= 1;
        }
    }
    let (mut up, mut down): (f64, f64) = match age {
        ..22 => ((if age < 20 { club.youth_work } else { (club.youth_work as i32 / 2).into() }) + 40.0, 0.0),
        22..26 => (20.0, 0.0),
        26..30 => (0.0, 0.0),
        30..34 => (0.0, if matches!(position, 1 | 2) { 10.0 } else { 20.0 }),
        34..40 => (0.0, 30.0),
        _ => (0.0, 50.0),
    };
    match talent {
        0 => {
            if age < 25 {
                up += up / 2.0;
            } else if age < 29 {
                up += up / 4.0;
            }
        }
        1 => {
            if strength < 12 {
                if age < 25 {
                    up += up / 4.0;
                }
            } else {
                up = 0.0;
            }
        }
        3 => {
            if strength > 8 {
                up = 0.0;
            }
            down += down / 4.0;
        }
        4 => {
            if strength > 7 {
                up = 0.0;
            }
            down += down / 2.0;
        }
        _ => {
            if strength > 9 {
                up = 0.0;
            }
        }
    }
    if up <= 0.0 {
        if down > 0.0 {
            down_bits |= 0x800;
            if f64::from(random.below(100)) < down {
                change -= 1;
            }
        }
    } else {
        up_bits |= 8;
        if f64::from(random.below(100)) < up {
            change += 1;
        }
    }
    if form_f > 100.0 {
        up_bits |= 4;
        let chance = (form_f - 100.0) as i16;
        if (random.below(100) as i16) < chance {
            change += 1;
        }
    } else if form_f < 100.0 {
        down_bits |= 4;
        let chance = (100.0 - form_f) as i16;
        if (random.below(100) as i16) < chance {
            change -= 1;
        }
    }
    if !half && change == 0 && club.matches / 2 <= league && i16::from(strength) < ceiling {
        if club.scored > 3.0 && position == 10 && random.below(5) == 0 {
            up_bits |= 0x80;
            change += 1;
            club.scored -= 2.0;
        }
        if club.let_in < 1.0 && (2..=5).contains(&position) && random.below(5) == 0 {
            up_bits |= 0x40;
            change += 1;
            club.let_in = 1.0;
        }
    }
    if age > 24 && age < 30 && strength < 3 {
        up_bits |= 0x100;
        change += 1;
    }
    let mut young_riser = false;
    if club.trainer != 0 {
        if strength == 12 && form_f > 130.0 && random.below(500) == 0 && up_bits == 0 && down_bits == 0 {
            ceiling = 13;
            change = 1;
        }
        young_riser = age < 22 && change > 0;
    }
    let drawn = change;
    let s16 = i16::from(strength);
    if strength < 13 {
        if ceiling < s16 {
            change = change.min(0);
        } else if ceiling < s16 + change {
            change = ceiling - s16;
        }
    } else {
        change = change.min(0);
    }
    if s16 + change < 1 {
        change = 1 - s16;
    }
    change = change.clamp(-2, 2);
    if change > 0 {
        let first = match strength {
            8 => Some(5),
            9 | 10 => Some(10),
            11 => Some(15),
            _ => None,
        };
        if let Some(first) = first {
            let mut traits = p.traits();
            if random.below(100) < first {
                traits[1] = 0x40;
                p.set_traits(traits);
            } else if random.below(100) < 5 {
                traits[1] = 0x200;
                p.set_traits(traits);
            }
        }
    }
    let now = (s16 + change) as u8;
    p.set_strength(now);
    if i16::from(now) + 2 < i16::from(best(p)) {
        p.set_problems(p.problems() | BELOW_BEST);
    }
    let _ = (up_bits, down_bits);
    (drawn, young_riser)
}

/// The half week the winter break falls in (`DAT_00b5cbd2 == 0x38`).
pub const WINTER_BREAK: u16 = 56;

/// The half weeks since the season's start (`0xb5cbd2`), as the original
/// counts them: one on every day from the first the week turns half (day
/// % 7 = 4) or whole (0) — 18 on day 63, 60 on day 210, as its saves keep
/// them. On the 56th (day 196) the players develop for the winter break,
/// without the season's matches (`FUN_0040f99c(1)`).
pub fn half_week(records: &mut Records, today: i32, random: &mut Random) {
    if today < 1 || !matches!(today % 7, 0 | 4) {
        return;
    }
    let count = today / 7 * 2 + i32::from(today % 7 >= 4);
    let Ok(count) = u16::try_from(count) else { return };
    records.globals.set_half_weeks(count);
    if count == WINTER_BREAK {
        develop_played(records, true, random);
    }
    // The champions' day honours every first league's leader
    // ([`crate::honours::champions_day`]).
    if count == crate::honours::CHAMPIONS_DAY {
        crate::honours::champions_day(records);
        crate::honours::top_scorers_day(records);
    }
    // And the autumn's half weeks their Herbstmeister.
    if (crate::honours::AUTUMN_FIRST..=crate::honours::AUTUMN_LAST).contains(&count) {
        crate::honours::autumn_day(records, count);
    }
}

/// The new season lifts a weak computer squad (`FUN_0077655c`, from the
/// second season on): when its strength averages below a mark by its
/// league — 10, 9 or 8 in the first by its place in the list (the first
/// six, the next six, the rest), 7 or 6 in the second (the first nine),
/// 5 below —, of its players under 30 below the mark whose talent reaches
/// above it up to five (more for the first league's best placed: ten less
/// half its place) gain a point, drawn when there are more.
pub(crate) fn lift_weak(records: &mut Records, country: u16, number: u16, random: &mut Random) {
    let Some(team) = records.team(country, number) else { return };
    if team.humans() != [0, 0] || records.globals.seasons() <= 1 {
        return;
    }
    let place = i32::from(team.league_place());
    let (mut lifts, threshold, mark) = match team.level() {
        0 => (
            10 - (place >> 1),
            if place < 6 { 100 } else if place < 12 { 90 } else { 80 },
            if place < 6 { 10 } else if place < 12 { 9 } else { 8 },
        ),
        1 => (5, if place < 10 { 70 } else { 60 }, if place < 10 { 7 } else { 6 }),
        _ => (5, 50, 5),
    };
    let squad = team.players();
    let today = today(records);
    let sum: i32 = squad.iter().filter_map(|&id| records.player(id)).map(|p| i32::from(p.strength())).sum();
    if sum * 10 >= threshold * squad.len() as i32 {
        return;
    }
    lifts = lifts.max(5);
    let candidates: Vec<u16> = squad
        .iter()
        .copied()
        .filter(|&id| {
            records.player(id).is_some_and(|p| {
                i32::from(p.strength()) < mark
                    && i32::from(crate::rollover::top_strength(p.talent())) > mark
                    && today.is_some_and(|d| crate::statistics::age_on(p, d) < 30)
            })
        })
        .collect();
    let lifted: Vec<u16> = if (lifts as usize) < candidates.len() {
        crate::cup_draw::choose(lifts as usize, candidates.len(), random).into_iter().map(|k| candidates[k]).collect()
    } else {
        candidates
    };
    for id in lifted {
        if let Some(p) = records.player_mut(id) {
            p.set_strength(p.strength() + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn club(level_ceiling: i16) -> Club {
        Club {
            matches: 34,
            row: 3,
            scored: 1.5,
            let_in: 1.5,
            youth_work: 0.0,
            ceiling: level_ceiling,
            trainer: 0,
            manager: 0,
        }
    }

    fn player(strength: u8, talent: i8, position: u8) -> Player {
        let mut p = Player::new(&vec![0; Player::SIZE]).unwrap();
        p.set_strength(strength);
        p.set_talent(talent);
        p.set_positions([position, 0, 0]);
        p
    }

    #[test]
    fn a_weak_player_in_his_late_twenties_gains_a_point() {
        let mut p = player(2, 2, 6);
        let mut random = Random::new(3);
        let (drawn, young) = develop_player(&mut p, 27, &mut club(9), false, &mut random);
        assert_eq!((drawn, young, p.strength()), (1, false, 3));
        assert_eq!(p.get::<u8>(HISTORY), 2, "the old strength noted");
    }

    #[test]
    fn a_league_below_the_second_holds_its_players_at_seven() {
        let mut random = Random::new(3);
        for _ in 0..200 {
            let mut p = player(7, 0, 6);
            p.set_season_form(200);
            p.set_season_form_weeks(10);
            develop_player(&mut p, 18, &mut club(7), false, &mut random);
            assert!(p.strength() <= 7);
        }
    }

    #[test]
    fn form_far_above_ten_lifts_and_far_below_drops() {
        let (mut ups, mut downs) = (0, 0);
        let mut random = Random::new(5);
        for _ in 0..400 {
            let mut good = player(5, 2, 6);
            good.set_season_form(19);
            good.set_season_form_weeks(1);
            develop_player(&mut good, 28, &mut club(13), true, &mut random);
            ups += i32::from(good.strength() > 5);
            let mut bad = player(5, 2, 6);
            bad.set_season_form(1);
            bad.set_season_form_weeks(1);
            develop_player(&mut bad, 28, &mut club(13), true, &mut random);
            downs += i32::from(bad.strength() < 5);
        }
        assert!((300..400).contains(&ups) && (300..400).contains(&downs), "{ups} up, {downs} down of 400");
    }

    #[test]
    fn the_best_strength_counts_the_history() {
        let mut p = player(5, 2, 6);
        p.put::<u8>(HISTORY, 0x9a);
        assert_eq!(best(&p), 10);
    }

    /// Over the original's pair (SeasonEnd364 → NewSeason000), the season
    /// end's development run by the replica after its draw and counts: every
    /// player of the countries played still at his club has the original's
    /// strength history to the byte; the falls come near the original's,
    /// the rises stay below its (which the new season's lift of weak squads,
    /// `FUN_0077655c`, adds to) and above half of them.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_season_end_develops_the_played_countries_players_as_the_original() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs, before) =
            (load("SeasonEnd364.an3"), load("NewSeason000.an3"), load("SeasonEnd364.an3"));
        let mut random = Random::new(3);
        let _ = crate::promotion::draw(&mut ours, &mut random);
        crate::promotion::count(&mut ours);
        develop_played(&mut ours, false, &mut random);
        let (mut compared, mut ups, mut downs, mut their_ups, mut their_downs) = (0, 0, 0, 0, 0);
        for country in crate::opponent::played_nations(&before) {
            for number in 0..before.countries.team_count(usize::from(country)).max(0) as u16 {
                let level = before.team(country, number).unwrap().level();
                for id in before.team(country, number).unwrap().players() {
                    let (o, x, y) = (before.player(id).unwrap(), ours.player(id).unwrap(), theirs.player(id).unwrap());
                    if o.first_name() != y.first_name() || o.club() != y.club() {
                        continue;
                    }
                    compared += 1;
                    for k in 0..HISTORY_LEN {
                        assert_eq!(x.get::<u8>(HISTORY + k), y.get::<u8>(HISTORY + k), "player {id}'s history");
                    }
                    if level >= 10 {
                        assert_eq!(x.strength(), o.strength(), "player {id} of no league");
                        continue;
                    }
                    ups += i32::from(x.strength() > o.strength());
                    downs += i32::from(x.strength() < o.strength());
                    their_ups += i32::from(y.strength() > o.strength());
                    their_downs += i32::from(y.strength() < o.strength());
                }
            }
        }
        assert!(compared > 4000, "{compared} players compared");
        assert!(their_ups / 2 < ups && ups < their_ups, "{ups} rises, the original {their_ups}");
        assert!((downs - their_downs).abs() * 4 < their_downs, "{downs} falls, the original {their_downs}");
    }

    /// The season's turn over the original's pair as the cycle runs it
    /// (the season end's steps, the settlement, the new season's opening):
    /// at the clubs of no league in the old season nor either new one (the
    /// more countries' risers are drawn) — whose players only the lift
    /// moves — the
    /// players lifted by a point come within a tenth of the original's, none
    /// by more, every one of them under 30, below 5 and of a talent reaching
    /// above; and over all the clubs of the countries played the rises and
    /// falls by a point come within a fifth of the original's.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_new_season_lifts_weak_squads_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs, before) =
            (load("SeasonEnd364.an3"), load("NewSeason000.an3"), load("SeasonEnd364.an3"));
        let data = crate::data::Data::load(&dir).unwrap();
        let exe = a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap();
        let tables = crate::league::LeagueTables::load(&exe).unwrap();
        let mut random = Random::new(3);
        let drawn = crate::promotion::draw(&mut ours, &mut random);
        crate::promotion::count(&mut ours);
        develop_played(&mut ours, false, &mut random);
        crate::rollover::retire_announced(&mut ours, &mut random);
        crate::statistics::archive_leagues(&mut ours);
        crate::statistics::fold_eternal(&mut ours);
        crate::promotion::flag(&mut ours, &data.rules.forecast.tickets);
        crate::rollover::count_first_seasons(&mut ours);
        let agents = crate::free_agents::release(&mut ours, &mut random);
        crate::deals::execute(&mut ours, &mut random);
        crate::rollover::close_played(&mut ours, &mut random);
        crate::rollover::close_kept(&mut ours, &mut random);
        crate::free_agents::place(&mut ours, &agents, (&data.rules.contract, &data.rules.value), &mut random);
        crate::settlement::settle_season(&mut ours, &tables);
        crate::promotion::take(&mut ours, &drawn);
        crate::settlement::open_season(&mut ours, &mut random);
        let today = today(&before).unwrap();
        let (mut lifted, mut their_lifted) = (0, 0);
        let (mut moves, mut their_moves) = ([0; 2], [0; 2]);
        for country in crate::opponent::played_nations(&before) {
            for number in 0..before.countries.team_count(usize::from(country)).max(0) as u16 {
                let level = |r: &Records| r.team(country, number).unwrap().level();
                let none = [&before, &ours, &theirs].into_iter().all(|r| level(r) >= 10);
                for id in before.team(country, number).unwrap().players() {
                    let (o, x, y) = (before.player(id).unwrap(), ours.player(id).unwrap(), theirs.player(id).unwrap());
                    if o.first_name() != y.first_name() || o.club() != y.club() || x.club() != o.club() {
                        continue;
                    }
                    let (mine, their) = (i16::from(x.strength()) - i16::from(o.strength()), i16::from(y.strength()) - i16::from(o.strength()));
                    for (k, by) in [1, -1].into_iter().enumerate() {
                        moves[k] += i32::from(mine == by);
                        their_moves[k] += i32::from(their == by);
                    }
                    if !none {
                        continue;
                    }
                    assert!((0..=1).contains(&mine), "player {id} of no league by {mine}");
                    if mine == 1 {
                        lifted += 1;
                        assert!(o.strength() < 5 && crate::rollover::top_strength(o.talent()) > 5, "player {id}");
                        assert!(crate::statistics::age_on(o, today) < 30, "player {id}'s age");
                    }
                    their_lifted += i32::from(their == 1);
                }
            }
        }
        assert!((lifted - their_lifted).abs() * 10 < their_lifted, "{lifted} lifted, the original {their_lifted}");
        for k in 0..2 {
            assert!((moves[k] - their_moves[k]).abs() * 5 < their_moves[k], "{:?} against the original's {:?}", moves, their_moves);
        }
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_half_weeks_count_as_the_originals_saves_and_the_winter_break_develops() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let mut random = Random::new(3);
        for (name, day) in [("Probe1.an3", 35), ("A.an3", 63), ("S2.an3", 210), ("S2late.an3", 298)] {
            let mut records = load(name);
            let kept = records.globals.half_weeks();
            records.globals.set_half_weeks(0);
            half_week(&mut records, day, &mut random);
            assert_eq!(records.globals.half_weeks(), kept, "{name} on day {day}");
        }
        let mut records = load("A.an3");
        let before = load("A.an3");
        half_week(&mut records, 195, &mut random);
        assert_eq!(records.globals.half_weeks(), before.globals.half_weeks(), "no tick on day 195");
        half_week(&mut records, 196, &mut random);
        assert_eq!(records.globals.half_weeks(), WINTER_BREAK);
        let (mut noted, mut moved) = (0, 0);
        for country in crate::opponent::played_nations(&before) {
            for number in 0..before.countries.team_count(usize::from(country)).max(0) as u16 {
                let team = before.team(country, number).unwrap();
                if team.level() >= 10 {
                    continue;
                }
                for id in team.players() {
                    let (o, x) = (before.player(id).unwrap(), records.player(id).unwrap());
                    noted += i32::from((0..HISTORY_LEN).any(|k| o.get::<u8>(HISTORY + k) != x.get::<u8>(HISTORY + k)));
                    moved += i32::from(o.strength() != x.strength());
                }
            }
        }
        assert!(noted > 3000 && moved > 300, "{noted} noted, {moved} moved");
    }
}
