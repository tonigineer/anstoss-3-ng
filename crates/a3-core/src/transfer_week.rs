//! The clubs' week on the transfer market (`FUN_0042958d`, at the week's
//! start after the turns: `FUN_006da3a9`'s state 2, held back after a
//! load as the market's roll beside it): the clubs of the kept countries
//! first (`FUN_0042793c`), then those of the main country and of the more
//! countries (`FUN_004297e1`), each club in its country's order.
//!
//! Late in the season (`FUN_0075827a`: after day 216) the listings of the
//! players with under two years left lose their terms (`FUN_0075926f`),
//! so their clubs cannot take them off any more.
//!
//! A club of the country's first six leagues then goes through its squad
//! (`FUN_004279f8`'s first loop; a human club's `FUN_00428678`): a player
//! not retiring, not passed over, with no deal for next season and a
//! contract still running is weighed in its last year — late in the
//! season, by how content he is (`FUN_004284d2`, `FUN_00770877`) — and
//! then either says he retires, when his strength or his age says so
//! (`FUN_00426eaf`), and goes off the transfer list (`FUN_0075a71f`), or
//! goes onto it (`FUN_0075a277` → `FUN_00758e3e`). One over thirty, free
//! of talks (`FUN_00426e3b`), may announce his retirement the same way
//! (`FUN_0075a636`): one in a thousand draws of 24 at 37 (a keeper 39),
//! of 18 more at 35 (37), of ten more at 32 (35). A human club's player
//! is weighed thus only in his last two years.
//!
//! Left out so far: the rest of the computer clubs' business that follows
//! in `FUN_004279f8` and `FUN_004297e1` — their scouting and bids, sales
//! and loans, the wanted positions (`FUN_006c41b1`, `FUN_0042991e`,
//! `FUN_00427494`) — and the European clubs' (`FUN_0069d3dd`) and the
//! closing passes (`FUN_004290b3`, `FUN_00428ea5`); the draws between
//! clubs are not the original's while those are missing.

use chrono::NaiveDate;

use crate::random::Random;
use crate::records::{Player, Records};

/// The kept countries' kind, and the countries' count.
const KEPT: u8 = 2;
const COUNTRIES: usize = 0x75;
/// After this day the clubs weigh their players' last years.
const LATE: i32 = 216;
/// A club of a league this low or lower does no business (club +0x105).
const LEVELS: u8 = 6;
/// R+0x26f: he retires at the season's end; he is on the transfer list.
const RETIRES: u8 = 0x80;
const LISTED: u8 = 1;
/// R+0x271's top two bits at 1: he is passed over (`FUN_00706a7e`).
const PASSED_OVER: u8 = 1;
/// The terms' bit that keeps him from retiring (`FUN_00426e3b`).
const HELD: u16 = 0x100;
/// The ids above are the managers who play.
const LAST_REAL_ID: u16 = crate::mood::LAST_REAL_ID;
/// The keepers' position.
const KEEPER: u8 = 1;
/// The calendar month before which a computer club weighs a last year.
const BEFORE_MONTH: u16 = 7;

/// The week's business of every club, in the original's order.
pub fn week(records: &mut Records, random: &mut Random) {
    let today = i32::from(records.globals.day());
    if today > LATE {
        crate::transfers::fix_late_terms(records);
    }
    let kept = (1..COUNTRIES).filter(|&c| records.countries.kind(c) == KEPT).map(|c| c as u16);
    let countries: Vec<u16> = kept.chain(crate::opponent::played_nations(records)).collect();
    for country in countries {
        for number in 0..records.countries.team_count(usize::from(country)).max(0) as u16 {
            let Some(team) = records.team(country, number) else { continue };
            if team.level() >= LEVELS {
                continue;
            }
            let human = team.humans() != [0, 0];
            for id in team.players() {
                weigh(records, id, human, today, random);
            }
        }
    }
    stale_listings(records, false, random);
}

/// A listing stands this long before its club takes it off: a kept
/// country's, a played country's (`FUN_00428e4d`).
const STALE_KEPT: i32 = 30;
const STALE_PLAYED: i32 = 140;
/// A stale listing's player this old retires (`FUN_00428c9f`).
const OLD: i32 = 32;
/// The days the season's end takes off every listing's day
/// (`FUN_00759d0c`).
const SEASON_SHIFT: i16 = 360;
/// A country of no data, and one played in full.
const ABROAD: u8 = 0;

/// The listings that stood too long (`FUN_00428ea5`, behind every week's
/// market and at the season's end): of each listing whose player is in
/// no talks, a club abroad sends him away at once — from the second
/// week of his listing unless the season ends —, a kept country's club
/// after thirty days, a played country's after a hundred and forty
/// (`FUN_00428e4d`): with no year left he leaves for good, in his last
/// he retires once over 32, with more he comes off the list — over 32
/// retiring and down to one year (`FUN_00428c9f`). His listing's mark
/// stays.
pub(crate) fn stale_listings(records: &mut Records, season_end: bool, random: &mut Random) {
    let today = i32::from(records.globals.day());
    for l in crate::transfers::listings(records) {
        if crate::transfers::pending(records, l.id) {
            continue;
        }
        let age = today - i32::from(l.day);
        let club = (l.country, l.number as i16);
        let kind = records.countries.kind(usize::from(l.country));
        if kind == ABROAD {
            if age == 0 && !season_end {
                continue;
            }
            leave(records, club, l.id, random);
            continue;
        }
        let played = records.globals.played(i16::from(l.country));
        if age <= if played { STALE_PLAYED } else { STALE_KEPT } {
            continue;
        }
        let in_squad = records.team(u16::from(l.country), l.number).is_some_and(|t| t.players().contains(&l.id));
        let Some(p) = records.player(l.id).filter(|_| in_squad) else { continue };
        let (years, old) = (years_left(records, p), age_of(records, p) > OLD);
        match years {
            0 => leave(records, club, l.id, random),
            1 => {
                if old {
                    crate::rollover::delist_transfer(records, l.id);
                    if let Some(p) = records.player_mut(l.id) {
                        p.set_retiring(p.retiring() | RETIRES);
                    }
                }
            }
            _ => {
                crate::rollover::delist_transfer(records, l.id);
                if old {
                    let year = records.globals.start_year();
                    if let Some(p) = records.player_mut(l.id) {
                        p.set_retiring(p.retiring() | RETIRES);
                        p.set_contract(year + 1);
                    }
                }
            }
        }
    }
}

/// A listed player leaves his club for good (`FUN_0070de6c`): out of its
/// squad, line-up and bench (`FUN_0070e01a`), off the list, his career
/// over (`FUN_0066d975`).
fn leave(records: &mut Records, (country, number): (u8, i16), id: u16, random: &mut Random) {
    let Ok(number) = u16::try_from(number) else { return };
    if let Some(team) = records.team_mut(u16::from(country), number) {
        let squad: Vec<u16> = team.players().into_iter().filter(|&p| p != id).collect();
        team.set_players(&squad);
        let mut lineup = team.lineup();
        lineup.iter_mut().filter(|s| **s == id).for_each(|s| *s = 0);
        team.set_lineup(lineup);
        let mut bench = team.bench();
        bench.iter_mut().filter(|s| **s == id).for_each(|s| *s = 0);
        team.set_bench(bench);
    }
    crate::rollover::delist_transfer(records, id);
    if let Some(p) = records.player_mut(id) {
        p.set_retiring(p.retiring() & !LISTED);
    }
    crate::rollover::end_career(records, id, random);
}

/// The transfer list at the season's end (`FUN_00428fa2`, after the
/// ranking's roll): the stale listings are dealt with as every week
/// (`FUN_00428ea5`), and every listing's day moves 360 back into the new
/// season's count (`FUN_00759d0c`). (`FUN_00428b95` before them would
/// take the countries not played off, but it removes from a copy it
/// never loaded: nothing changes.)
pub fn close_season(records: &mut Records, random: &mut Random) {
    stale_listings(records, true, random);
    crate::transfers::shift_days(records, SEASON_SHIFT);
}

fn age_of(records: &Records, p: &Player) -> i32 {
    age(records, p)
}

fn today(records: &Records) -> Option<NaiveDate> {
    let [day, month, year] = records.globals.date();
    NaiveDate::from_ymd_opt(i32::from(year), u32::from(month), u32::from(day))
}

fn age(records: &Records, p: &Player) -> i32 {
    today(records).map_or(0, |d| crate::statistics::age_on(p, d))
}

/// The years his contract has left, as his object holds them.
fn years_left(records: &Records, p: &Player) -> i32 {
    (i32::from(p.contract() & 0xfff) - i32::from(records.globals.start_year())).max(0)
}

fn passed_over(p: &Player) -> bool {
    p.picks() >> 6 == PASSED_OVER
}

/// One player of a club's squad weighed for the week.
fn weigh(records: &mut Records, id: u16, human: bool, today: i32, random: &mut Random) {
    let Some(p) = records.player(id) else { return };
    let years = years_left(records, p);
    if passed_over(p) || p.retiring() & RETIRES != 0 || p.next_club()[0] != 0 || years <= 0 {
        return;
    }
    let month = records.globals.date()[1];
    if years == 1 && (human || month < BEFORE_MONTH) && weighed(p, today, random) {
        if wants_to_retire(records, p) {
            retire(records, id);
        } else {
            crate::transfers::list_for_terms(records, id, 0);
        }
    }
    let Some(p) = records.player(id) else { return };
    if age(records, p) > 30 && (!human || years < 2) && free_to_retire(records, p) {
        let age = age(records, p);
        let keeper = p.positions()[0] == KEEPER;
        let past = |field: i32, keeper_age: i32| age > field && !keeper || age > keeper_age;
        let roll = random.below(1000);
        let announces = match roll {
            0..24 => past(36, 38),
            24..42 => past(34, 36),
            42..52 => past(31, 34),
            _ => false,
        };
        if announces {
            retire(records, id);
        }
    }
}

/// He says he retires: off the transfer list, his listing's mark gone
/// (`FUN_0075a636`, `FUN_0075a71f`), his retirement marked.
fn retire(records: &mut Records, id: u16) {
    crate::rollover::delist_transfer(records, id);
    if let Some(p) = records.player_mut(id) {
        p.set_retiring((p.retiring() & !LISTED) | RETIRES);
    }
}

/// Whether he is free to say he retires (`FUN_00426e3b`): not retiring,
/// not passed over, no deal for next season, his terms no bar, no talks
/// pending about him.
fn free_to_retire(records: &Records, p: &Player) -> bool {
    p.retiring() & RETIRES == 0
        && !passed_over(p)
        && p.next_club()[0] == 0
        && p.terms() & HELD == 0
        && !crate::transfers::pending(records, p.id())
}

/// Whether his last year makes him retire rather than go onto the list
/// (`FUN_00426eaf`): free to, and at 26 of strength 1, at 34 of strength
/// 2 at most, or at 41.
fn wants_to_retire(records: &Records, p: &Player) -> bool {
    let age = age(records, p);
    let strength = p.strength();
    free_to_retire(records, p) && (age >= 26 && strength == 1 || age >= 34 && strength <= 2 || age >= 41)
}

/// Whether his club weighs his last year this week (`FUN_004284d2`):
/// late in the season by how little he is content, more often as the
/// season ends, and every week from day 330.
fn weighed(p: &Player, today: i32, random: &mut Random) -> bool {
    if today <= LATE {
        return false;
    }
    let class = discontent(p);
    let below = |random: &mut Random, n: u32| random.below(100) < n;
    match today {
        ..240 => match class {
            1 => below(random, 4),
            2 => below(random, 8),
            3 => below(random, 20),
            _ => false,
        },
        ..270 => match class {
            1 => below(random, 8),
            2 => below(random, 12),
            3 => below(random, 20),
            _ => false,
        },
        330.. => true,
        _ => match class {
            1 => below(random, 15),
            2 => below(random, 35),
            3 => true,
            _ => false,
        },
    }
}

/// How discontent he is, 0 to 3 (`FUN_00770877`): 3 when his trainer,
/// team and club please him less than 70 over the mean, 2 or 1 when all
/// seven parts come to less than 170 or 201 over it; a manager who plays
/// counts 1.
fn discontent(p: &Player) -> u8 {
    if p.id() > LAST_REAL_ID {
        return 1;
    }
    let parts = crate::mood::parts(p);
    let first = parts[0] + parts[1] + parts[2] - 200;
    let rest = parts[3] + parts[4] + parts[5] + parts[6] - 300;
    if first < 70 {
        3
    } else if first + rest < 170 {
        2
    } else if first + rest < 201 {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kept(name: &str) -> Records {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap()
    }

    /// The game on a day of its season, as the week's start has it.
    fn on_day(records: &mut Records, day: u16) {
        use chrono::Datelike;
        records.globals.set_day(day);
        let d = records.globals.date_of(day as i16).unwrap();
        records.globals.set_date([d.day() as u16, d.month() as u16, d.year() as u16]);
    }

    /// The business clubs, and of their players those weighed in a last
    /// year: not retiring, not passed over, no deal.
    fn last_years(records: &Records) -> Vec<(u16, bool)> {
        let mut v = Vec::new();
        for country in 1..COUNTRIES {
            if !matches!(records.countries.kind(country), 1 | KEPT) {
                continue;
            }
            for number in 0..records.countries.team_count(country).max(0) as u16 {
                let team = records.team(country as u16, number).unwrap();
                if team.level() >= LEVELS {
                    continue;
                }
                for id in team.players() {
                    let p = records.player(id).unwrap();
                    if years_left(records, p) == 1
                        && !passed_over(p)
                        && p.retiring() & RETIRES == 0
                        && p.next_club()[0] == 0
                    {
                        v.push((id, team.humans() != [0, 0]));
                    }
                }
            }
        }
        v
    }

    /// From day 330 every club weighs each last year every week: over
    /// the original's save A at day 336 every such player of a club of
    /// the first six leagues — the human club's too — goes onto the
    /// transfer list or says he retires, while the list has room, and
    /// the late listings stand at terms 0; the save's own listings, by
    /// then long stale, come off at the week's end.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_last_weeks_list_every_last_year() {
        let mut records = kept("A.an3");
        on_day(&mut records, 336);
        let before = last_years(&records);
        let listed_before = crate::transfers::listings(&records).len();
        week(&mut records, &mut Random::new(5));
        let listed: Vec<u16> = crate::transfers::listings(&records).iter().map(|l| l.id).collect();
        let (mut onto, mut retiring, mut left) = (0, 0, 0);
        for &(id, _) in &before {
            let p = records.player(id).unwrap();
            if p.retiring() & RETIRES != 0 {
                assert!(!listed.contains(&id), "{id} retires and is listed");
                retiring += 1;
            } else if listed.contains(&id) {
                assert_ne!(p.retiring() & LISTED, 0, "{id}'s mark");
                onto += 1;
            } else {
                left += 1;
            }
        }
        assert!(before.iter().any(|&(_, human)| human), "no human last year");
        assert!(onto > 200 && retiring > 0, "{onto} listed, {retiring} retiring of {}", before.len());
        // Those left over found the list full — before the week's end took
        // its stale listings off —, or were in talks.
        assert!(left == 0 || listed_before + onto >= 400, "{left} left, {onto} onto {listed_before}");
        assert!(listed.len() > listed_before);
        let start = i32::from(records.globals.start_year());
        for l in crate::transfers::listings(&records) {
            let years = i32::from(records.player(l.id).unwrap().contract() & 0xfff) - start;
            assert!(years >= 2 || l.terms == 0, "{} at terms {} with {years} years", l.id, l.terms);
        }
    }

    /// The weeks from save A's day 63 to the season's end announce as
    /// many retirements as the original's own season did (its
    /// SeasonEnd364 holds 557 marks in the clubs of the first six
    /// leagues, ages 32 to 45), and never before day 217 does a club list
    /// a last year.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_season_announces_as_many_retirements_as_the_original() {
        let mut records = kept("A.an3");
        let theirs = kept("SeasonEnd364.an3");
        let count = |records: &Records| {
            let mut ages = Vec::new();
            let today = records.globals.date_of(records.globals.day() as i16).unwrap();
            for country in 1..COUNTRIES {
                if !matches!(records.countries.kind(country), 1 | KEPT) {
                    continue;
                }
                for number in 0..records.countries.team_count(country).max(0) as u16 {
                    let team = records.team(country as u16, number).unwrap();
                    if team.level() >= LEVELS {
                        continue;
                    }
                    for id in team.players() {
                        let p = records.player(id).unwrap();
                        if p.retiring() & RETIRES != 0 {
                            ages.push(crate::statistics::age_on(p, today));
                        }
                    }
                }
            }
            ages
        };
        let mut random = Random::new(7);
        let listed = |records: &Records| -> Vec<u16> {
            crate::transfers::listings(records).iter().map(|l| l.id).collect()
        };
        let listed_before = listed(&records);
        for day in (70..=364).step_by(7) {
            on_day(&mut records, day);
            week(&mut records, &mut random);
            if day <= 210 {
                assert!(listed(&records).iter().all(|id| listed_before.contains(id)), "day {day}");
            }
        }
        let (ours, theirs) = (count(&records), count(&theirs));
        let off = (ours.len() as f64 / theirs.len() as f64 - 1.0).abs();
        assert!(off < 0.1, "{} marks against the original's {}", ours.len(), theirs.len());
        assert!(ours.iter().all(|&age| age >= 26), "{ours:?}");
        assert_eq!(ours.iter().max(), theirs.iter().max());
    }
}

#[cfg(test)]
mod season_tests {
    use super::*;

    /// The original's turn (SeasonEnd364 → NewSeason000) run by the
    /// replica's season end: its transfer list comes out byte for byte as
    /// the original's — the stale listings off (a kept country's three of
    /// day 336 staying, being under thirty days old), every day 360 back,
    /// and the removed entries' places moved as the original moves them.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_transfer_list_closes_the_season_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut b, n) = (load("SeasonEnd364.an3"), load("NewSeason000.an3"));
        let data = crate::data::Data::load(&dir).unwrap();
        let mut r = Random::new(3);
        let drawn = crate::promotion::draw(&mut b, &mut r);
        crate::promotion::count(&mut b);
        crate::statistics::archive_strengths(&mut b);
        crate::development::develop_played(&mut b, false, &mut r);
        crate::rollover::retire_announced(&mut b, &mut r);
        crate::statistics::archive_leagues(&mut b);
        crate::statistics::fold_eternal(&mut b);
        crate::promotion::flag(&mut b, &data.rules.forecast.tickets);
        crate::europe_entry::qualify(&mut b, &mut r);
        crate::rollover::count_first_seasons(&mut b);
        let agents = crate::free_agents::release(&mut b, &mut r);
        crate::deals::execute(&mut b, &mut r);
        crate::rollover::close_played(&mut b, &mut r);
        crate::rollover::close_kept(&mut b, &mut r);
        crate::free_agents::place(&mut b, &agents, (&data.rules.contract, &data.rules.value), &mut r);
        crate::settlement::settle_season(&mut b, &data.rules.league);
        crate::settlement::close_managers(&mut b, &mut r);
        crate::statistics::roll_ranking(&mut b);
        close_season(&mut b, &mut r);
        crate::promotion::take(&mut b, &drawn);
        let name = "0x1c49278";
        let (x, y) = (b.block(name).unwrap(), n.block(name).unwrap());
        assert_eq!(crate::transfers::listings(&b).len(), 66);
        assert!(x == y, "{} bytes differ", (0..x.len()).filter(|&k| x[k] != y[k]).count());
    }
}
