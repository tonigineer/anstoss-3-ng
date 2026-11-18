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
//! Before that, every fourth week, a club of the main or a more country
//! with no human manager offers the men it counts on fresh contracts
//! (`FUN_0042991e`, the club's number and the day alike modulo four).
//!
//! After it a computer club's transfer budget is refilled and, in the
//! first week of November, its wanted positions planned anew
//! ([`crate::search_plan::refill`]); then it may let go of a man of a
//! position it holds too many of, for a fee or, early in the season, on
//! loan (the end of `FUN_004279f8`).
//!
//! Left out so far: the rest of the computer clubs' business that follows
//! in `FUN_004279f8` and `FUN_004297e1` — the men they scouted, their bids
//! for the wanted positions (`FUN_006c41b1`, `FUN_00427494`) — and
//! the European clubs' (`FUN_0069d3dd`) and the closing passes
//! (`FUN_004290b3`, `FUN_00428ea5`); the draws between clubs are not the
//! original's while those are missing.

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
pub fn week(records: &mut Records, rules: &crate::season::Rules, random: &mut Random) {
    let today = i32::from(records.globals.day());
    if today > LATE {
        crate::transfers::fix_late_terms(records);
    }
    let kept: Vec<u16> = (1..COUNTRIES).filter(|&c| records.countries.kind(c) == KEPT).map(|c| c as u16).collect();
    let played = crate::opponent::played_nations(records);
    let countries = kept.iter().map(|&c| (c, false)).chain(played.iter().map(|&c| (c, true)));
    for (country, played) in countries.collect::<Vec<_>>() {
        for number in 0..records.countries.team_count(usize::from(country)).max(0) as u16 {
            if played && today & 3 == i32::from(number & 3) {
                renew(records, rules, (country, number), random);
            }
            let Some(team) = records.team(country, number) else { continue };
            if team.level() >= LEVELS {
                continue;
            }
            let human = team.humans() != [0, 0];
            for id in team.players() {
                weigh(records, id, human, today, random);
            }
            if !human {
                crate::search_plan::refill(records, random, (country as u8, number as i16));
                sell(records, rules, (country, number), SALE_ODDS[usize::from(played)], random);
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

/// The month after which, up to June, a club offers the men it counts
/// on more alike (`FUN_0042991e`).
const SPRING: std::ops::RangeInclusive<u16> = 3..=6;
/// R+0x27f's 0x40: he takes no further extension (`FUN_00707834`).
const NO_EXTENSION: u8 = 0x40;
/// The terms' bit of a man his club counts on (R+0x15e,
/// [`crate::squad_rank`]).
const COUNTED_ON: u16 = 0x40;
/// The reason his club pleases him with his contract, and the one of
/// the club it clears (`FUN_00770800`, `FUN_00770942`).
const CONTRACT_PLEASES: u32 = 0x100000;
const CONTRACT_GRIEVANCE: u32 = 0x4000;
/// What his new terms are worth to him, within these (R+0x160, L+0x99).
const WORTH_RANGE: (i32, i32) = (80, 200);

/// A club offers the men it counts on fresh contracts (`FUN_0042991e`),
/// unless a human manages it. Of those it may keep (`FUN_00426cc6`) and
/// would (`FUN_004270b0`), one with three years left takes two more one
/// time in a hundred, one with two four times in a hundred, either off
/// the transfer list then; every other one is offered terms afterwards,
/// in his squad's order, by a chance in a hundred: from March to June 30
/// for one in his last year stronger than 9, 7 or 5 (by the club's
/// league: the first, the second, any other), else 15; in the other
/// months twice his strength and 14 for such a one, else his strength
/// and 7.
fn renew(records: &mut Records, rules: &crate::season::Rules, (country, number): (u16, u16), random: &mut Random) {
    let Some(team) = records.team(country, number) else { return };
    if team.humans()[1] != 0 {
        return;
    }
    let spring = SPRING.contains(&records.globals.date()[1]);
    let bar = match team.level() {
        0 => 9,
        1 => 7,
        _ => 5,
    };
    let squad = team.players();
    let mut offered = Vec::new();
    for &id in &squad {
        let Some(p) = records.player(id) else { continue };
        if !may_keep(records, p) || !would_keep(records, p) {
            continue;
        }
        let years = years_left(records, p);
        let strength = i32::from(p.strength());
        let longer = match years {
            3 => random.below(100) == 0,
            2 => random.below(100) < 4,
            _ => {
                let top = years == 1 && strength > bar;
                offered.push((id, match (spring, top) {
                    (true, true) => 30,
                    (true, false) => 15,
                    (false, true) => strength * 2 + 14,
                    (false, false) => strength + 7,
                }));
                false
            }
        };
        if longer {
            if let Some(p) = records.player_mut(id) {
                p.set_contract((p.contract() & !0xfff) | ((p.contract() & 0xfff) + 2));
            }
            off_list(records, id);
        }
    }
    for (id, chance) in offered {
        if (random.below(100) as i32) < chance {
            extend(records, rules, (country, number), id, random);
            off_list(records, id);
        }
    }
}

/// A computer club gives him a new contract (`FUN_00426f52`) when he is
/// under 35 or stronger than 4, and no talks are pending about him: the
/// years he has left and one more, and under 31 up to three, under 36 up
/// to one more drawn; fresh terms over his own (`FUN_00709fb6`); what
/// they are worth to him at the club (`FUN_00709c52`), within 80 and
/// 200, as the least his will to stay must reach — from 80 on his club
/// pleasing him with them, the grievance gone.
fn extend(records: &mut Records, rules: &crate::season::Rules, (country, number): (u16, u16), id: u16, random: &mut Random) {
    let Some(p) = records.player(id) else { return };
    let age = age(records, p);
    if !(age < 35 || p.strength() > 4) || crate::transfers::pending(records, id) {
        return;
    }
    let years = years_left(records, p) as i8;
    let years = match age {
        ..31 => years + 1 + random.below(4) as i8,
        ..36 => years + 1 + random.below(2) as i8,
        _ => years + 1,
    };
    let Some(team) = records.team(country, number) else { return };
    let tables = (&rules.contract, &rules.value);
    let old = crate::transfers::terms(records, id);
    let Some(mut t) = crate::free_agents::terms_over(records, id, team.level(), tables, old, random) else { return };
    t[0xc] = years as u8;
    let word = |at: usize| u16::from_le_bytes([t[at], t[at + 1]]);
    let start = i32::from(records.globals.start_year());
    if let Some(p) = records.player_mut(id) {
        p.set_salary(word(0));
        p.set_appearance_bonus(word(2));
        p.set_goal_bonus(word(4));
        p.set_fixed_fee(word(6));
        p.set_terms(word(0xe));
        p.set_contract((p.contract() & !0xfff) | (start + i32::from(years)) as u16);
    }
    let (Some(p), Some(team)) = (records.player(id), records.team(country, number)) else { return };
    let mut own = crate::transfers::terms(records, id);
    own[8..12].fill(0);
    let Some(worth) = crate::transfers::offer_worth(records, tables, p, team, &own) else { return };
    let worth = worth.min(WORTH_RANGE.1);
    let p = records.player_mut(id).expect("he is there");
    if worth < WORTH_RANGE.0 {
        p.put::<u8>(0x160, WORTH_RANGE.0 as u8);
    } else {
        rules.discipline.mood.set_reasons(p, CONTRACT_PLEASES);
        crate::mood::clear_reasons(p, CONTRACT_GRIEVANCE);
        p.put::<u8>(0x160, worth as u8);
    }
}

/// Whether his club may keep him longer (`FUN_00426cc6`): free to say he
/// retires, open to an extension, with under six years left and his age
/// and those under 37.
fn may_keep(records: &Records, p: &Player) -> bool {
    let years = years_left(records, p);
    free_to_retire(records, p) && p.problems() & NO_EXTENSION == 0 && years < 6 && age(records, p) + years < 37
}

/// Whether his club would (`FUN_004270b0`): not retiring, open to an
/// extension, not passed over, under 36, counted on, under four years
/// left — of three only stronger than 3, of two stronger than 1 — and no
/// deal for next season.
fn would_keep(records: &Records, p: &Player) -> bool {
    let (years, strength) = (years_left(records, p), p.strength());
    p.retiring() & RETIRES == 0
        && p.problems() & NO_EXTENSION == 0
        && !passed_over(p)
        && age(records, p) < 36
        && p.terms() & COUNTED_ON != 0
        && years < 4
        && !(years == 3 && strength < 4)
        && !(years == 2 && strength < 2)
        && p.next_club()[0] == 0
}

/// Off the transfer list, his listing's mark gone (`FUN_0075a636`).
fn off_list(records: &mut Records, id: u16) {
    crate::rollover::delist_transfer(records, id);
    if let Some(p) = records.player_mut(id) {
        p.set_retiring(p.retiring() & !LISTED);
    }
}

/// He says he retires: off the transfer list, his listing's mark gone
/// (`FUN_0075a636`, `FUN_0075a71f`), his retirement marked.
fn retire(records: &mut Records, id: u16) {
    off_list(records, id);
    if let Some(p) = records.player_mut(id) {
        p.set_retiring(p.retiring() | RETIRES);
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
pub(crate) fn discontent(p: &Player) -> u8 {
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

/// The odds a computer club's week of selling and lending is drawn
/// against: a kept country's, a played one's (`FUN_004279f8`'s first
/// parameter, by `FUN_0042793c` and `FUN_004297e1`), and the draws below
/// this that bring it.
const SALE_ODDS: [u32; 2] = [400, 100];
const SALE_DRAWS: u32 = 20;
/// The men of a position a club keeps before it lets one go, by position
/// 1 to 10 (`DAT_008773ec`): the second row for the countries 0x10
/// (England), 0x1c (Italy) and 0x3f.
const KEPT_BY_POSITION: [[u8; 11]; 2] =
    [[0, 3, 2, 4, 2, 2, 2, 2, 2, 2, 4], [0, 4, 2, 6, 3, 3, 3, 3, 3, 3, 7]];
const WIDE_SQUADS: [u16; 3] = [0x10, 0x1c, 0x3f];
/// The chance in a hundred a calendar month gives a sale
/// (`DAT_00877404`).
const MONTH_CHANCE: [i32; 13] = [0, 10, 10, 10, 10, 10, 10, 0, 3, 6, 10, 15, 20];
/// The best of a position a club ranks (`FUN_00429dd0`), and the days
/// out from which a man is not ranked (L+0x2c).
const RANKED: usize = 8;
const OUT_DAYS: i32 = 14;
/// The half weeks of the season in which a club lends (`FUN_00426bfc`),
/// the age it lends below, the squad it lends from and the league
/// matches under which a man is lent.
const LENDING: std::ops::RangeInclusive<u16> = 20..=40;
const LENT_BELOW: i32 = 22;
const LENDING_SQUAD: u16 = 20;
const LENT_MATCHES: u8 = 2;
/// The class's base the sale's chance counts a surplus man with
/// (`10 - club+0x11e0`), the class (team +0x875).
const CLASS_BASE: i32 = 10;
const CLASS: usize = 0x875;
/// R+0x161 (L+0x98): how often he may still move this season, two at
/// least for a sale before the season's late weeks (`FUN_00426d83`).
const MOVES: usize = 0x161;
const SALE_MOVES: u8 = 2;

/// A computer club lets go of a man of a position it has too many of
/// (`FUN_004279f8`'s end): on twenty draws of its odds it ranks its men
/// out under fourteen days by position — the best eight by their promise
/// a thousand times and the best players' pick (`FUN_006ff516`,
/// `FUN_00708b80`) — and of each position that holds more than the club
/// keeps of it takes the last of those ranked. One it does not count on
/// and may sell (`FUN_00426d83`) goes onto the transfer list for a fee
/// (`FUN_0075a129`) by a chance in a hundred: the month's, and for every
/// man over those kept ten less the club's class. Otherwise, in the
/// season's half weeks 20 to 40, a club of the main or a more country
/// lends one under 22 it may lend (`FUN_00426c22`) at his fair prices
/// (`FUN_00622f7d`, `FUN_0075b0da`): one it counts on for the fee alone,
/// another with an option to buy him.
fn sell(records: &mut Records, rules: &crate::season::Rules, (country, number): (u16, u16), odds: u32, random: &mut Random) {
    if random.below(odds) >= SALE_DRAWS {
        return;
    }
    let Some(team) = records.team(country, number) else { return };
    let (squad, size) = (team.players(), team.player_count());
    let class = i32::from(team.get::<u8>(CLASS));
    let lends = LENDING.contains(&records.globals.half_weeks());
    let mut ranked: [Vec<(u16, i16)>; 11] = Default::default();
    for id in squad {
        let Some(p) = records.player(id) else { continue };
        if crate::load::days_to(&records.globals, p.injured_to()) >= OUT_DAYS {
            continue;
        }
        let key = (i32::from(crate::free_agents::promise(p, age(records, p))) * 1000
            + i32::from(rules.value.pick_value(p) as i16)) as i16;
        let Some(list) = ranked.get_mut(usize::from(p.positions()[0])) else { continue };
        let last = if list.len() < RANKED { 0 } else { list[RANKED - 1].1 };
        if key > last {
            let at = list.iter().position(|&(_, k)| k < key).unwrap_or(list.len());
            list.insert(at, (id, key));
            list.truncate(RANKED);
        }
    }
    let row = usize::from(WIDE_SQUADS.contains(&country));
    let month = usize::from(records.globals.date()[1]);
    for position in 1..=10 {
        let (list, kept) = (&ranked[position], KEPT_BY_POSITION[row][position]);
        if list.len() <= usize::from(kept) {
            continue;
        }
        let id = list[list.len() - 1].0;
        let Some(p) = records.player(id) else { continue };
        if id > LAST_REAL_ID {
            continue;
        }
        let counted_on = p.terms() & COUNTED_ON != 0;
        if !counted_on && may_sell(records, p) {
            let surplus = (list.len() - usize::from(kept)) as i32;
            let chance = MONTH_CHANCE.get(month).copied().unwrap_or(0) + surplus * (CLASS_BASE - class);
            if (random.below(100) as i32) < chance {
                crate::transfers::list_for_sale(records, id, false);
            }
            continue;
        }
        if !lends || age(records, p) >= LENT_BELOW || !may_lend(records, p, size) || !records.globals.played(country as i16) {
            continue;
        }
        let star = records.globals.stars().get(usize::from(p.positions()[0])) == Some(&id);
        let worth = crate::value::Worth {
            strength: p.strength(),
            position: p.positions()[0],
            talent: p.talent(),
            age: age(records, p),
            good: p.good(),
            character: p.traits(),
            qualities: p.qualities(),
            star,
            kind_12_matches: p.kind_12_matches(),
        };
        let prices = rules.value.loan_prices(&worth, years_left(records, p), month as u32);
        if counted_on {
            crate::transfers::list_for_loan(records, id, false, prices.fee, 0);
        } else {
            crate::transfers::list_for_loan(records, id, true, prices.with_option, prices.option);
        }
    }
}

/// Whether his club may sell him (`FUN_00426d83`): a real man, not
/// retiring, open to an extension, not passed over, no deal for next
/// season, his terms no bar, no talks pending about him, and before the
/// season's late weeks free to move twice more.
fn may_sell(records: &Records, p: &Player) -> bool {
    p.id() <= LAST_REAL_ID
        && p.retiring() & RETIRES == 0
        && p.problems() & NO_EXTENSION == 0
        && !passed_over(p)
        && p.next_club()[0] == 0
        && p.terms() & HELD == 0
        && !crate::transfers::pending(records, p.id())
        && (i32::from(records.globals.day()) > LATE || p.get::<u8>(MOVES) >= SALE_MOVES)
}

/// Whether his club may lend him (`FUN_00426c22`): not retiring, open
/// to an extension, not passed over, no deal for next season, no talks
/// pending about him, two years left at least, under two league matches
/// this season, and a squad of twenty at least.
fn may_lend(records: &Records, p: &Player, squad: u16) -> bool {
    p.retiring() & RETIRES == 0
        && p.problems() & NO_EXTENSION == 0
        && !passed_over(p)
        && p.next_club()[0] == 0
        && !crate::transfers::pending(records, p.id())
        && years_left(records, p) >= 2
        && p.league_matches() < LENT_MATCHES
        && squad >= LENDING_SQUAD
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

    fn rules() -> crate::season::Rules {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        crate::data::Data::load(&dir).unwrap().rules
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
        week(&mut records, &rules(), &mut Random::new(5));
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
        let rules = rules();
        // A last year goes onto the list at terms 0; the sales are for a
        // fee.
        let listed = |records: &Records| -> Vec<u16> {
            crate::transfers::listings(records).iter().filter(|l| l.terms == 0).map(|l| l.id).collect()
        };
        let listed_before = listed(&records);
        for day in (70..=364).step_by(7) {
            on_day(&mut records, day);
            week(&mut records, &rules, &mut random);
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

    /// The men who stayed at a computer club of the main or a more
    /// country from one game to another, and of those whose contract runs
    /// longer in the second: how many by the years it had left in the
    /// first, and the least worth a last year's new contract holds
    /// (R+0x160; those with more years only add two).
    fn extended(a: &Records, b: &Records) -> ([usize; 4], u8) {
        let (mut by_years, mut least) = ([0; 4], u8::MAX);
        for country in crate::opponent::played_nations(a) {
            for number in 0..a.countries.team_count(usize::from(country)).max(0) as u16 {
                let (x, y) = (a.team(country, number).unwrap(), b.team(country, number).unwrap());
                let ours = y.players();
                if x.humans() != [0, 0] {
                    for id in x.players().into_iter().filter(|id| ours.contains(id)) {
                        assert_eq!(a.player(id).unwrap().contract(), b.player(id).unwrap().contract(), "human club's {id}");
                    }
                    continue;
                }
                for id in x.players().into_iter().filter(|id| ours.contains(id)) {
                    let (p, q) = (a.player(id).unwrap(), b.player(id).unwrap());
                    if q.contract() & 0xfff > p.contract() & 0xfff {
                        let years = years_left(a, p);
                        by_years[(years as usize).min(3)] += 1;
                        if years == 1 {
                            least = least.min(q.get::<u8>(0x160));
                        }
                    }
                }
            }
        }
        (by_years, least)
    }

    /// From save A's day 63 to the season's end the computer clubs of the
    /// main and the more countries keep the men they count on longer as
    /// often as the original's season did (its SeasonEnd364: 233 of them
    /// in their last year, 265 with two left, 108 with three, the human
    /// club none): each count within a quarter (the draws move with every
    /// further part of the market built, the seed with them: of seeds 1
    /// to 8 two leave the replica's three-year count a third over), the
    /// replica's every new
    /// contract of a last year worth 80 at least to the man (the
    /// original's one of 233 below came another way).
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_clubs_keep_their_men_longer_as_the_original() {
        let a = kept("A.an3");
        let (theirs, _) = extended(&a, &kept("SeasonEnd364.an3"));
        let mut records = kept("A.an3");
        let (rules, mut random) = (rules(), Random::new(8));
        for day in (70..=364).step_by(7) {
            on_day(&mut records, day);
            week(&mut records, &rules, &mut random);
        }
        let (ours, least) = extended(&a, &records);
        assert_eq!((theirs[0], ours[0]), (0, 0));
        for k in 1..4 {
            let off = (ours[k] as f64 / theirs[k] as f64 - 1.0).abs();
            assert!(off < 0.25, "{k} years left: {} against the original's {}", ours[k], theirs[k]);
        }
        assert!(least >= 80, "{least}");
    }

    /// The computer clubs' listings for a fee standing in a game, of the
    /// main and more countries listed after `since`, of the kept
    /// countries in the last thirty days; and the loans listed after it,
    /// as (age today, counted on, for the fee alone).
    fn sales(records: &Records, since: i16) -> (usize, usize, Vec<(i32, bool, bool)>) {
        let (mut played, mut kept, mut loans) = (0, 0, Vec::new());
        let today = records.globals.day() as i16;
        for l in crate::transfers::listings(records) {
            let human = records.team(l.country.into(), l.number).is_some_and(|t| t.humans() != [0, 0]);
            if human || l.day <= since {
                continue;
            }
            let p = records.player(l.id).unwrap();
            match (l.kind, l.terms, records.globals.played(l.country.into())) {
                (1, 1, true) => played += 1,
                (1, 1, false) if l.day > today - 30 => kept += 1,
                (2 | 3, _, _) => loans.push((age(records, p), p.terms() & COUNTED_ON != 0, l.kind == 2)),
                _ => {}
            }
        }
        (played, kept, loans)
    }

    /// From save A's day 63 to day 210 the computer clubs let go of the
    /// men of the positions they hold too many of about as the original's
    /// two runs to that day (S2, S2c): of the main and more countries at
    /// least as many listings for a fee as the original still holds (100
    /// and 78; the bids that take some away are not built) and less than
    /// half again; of the kept countries, whose listings go after thirty
    /// days, from as many as the original's last thirty days hold (9 in
    /// both) to twice; loans for the fee alone of men they count on, as
    /// the original's three and four, each under 22 when listed in the
    /// half weeks 20 to 40 (days 70 to 140), more of them standing (the
    /// original's bids take loans as they come).
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_clubs_let_go_of_their_spare_men_as_the_original() {
        let (s2, s2c) = (sales(&kept("S2.an3"), 63), sales(&kept("S2c.an3"), 63));
        assert_eq!((s2.0, s2c.0, s2.1, s2c.1), (100, 78, 9, 9));
        assert!([&s2.2, &s2c.2].iter().all(|l| l.len() >= 3 && l.iter().all(|&(_, on, fee)| on && fee)));
        let mut records = kept("A.an3");
        let (rules, mut random) = (rules(), Random::new(7));
        for day in (70..=210).step_by(7) {
            on_day(&mut records, day);
            records.globals.set_half_weeks(day / 7 * 2);
            week(&mut records, &rules, &mut random);
        }
        let (played, kept, loans) = sales(&records, 63);
        assert!((100..150).contains(&played), "{played} listed for a fee");
        assert!((9..=18).contains(&kept), "{kept} of the kept countries");
        assert!(loans.len() >= 4, "{loans:?}");
        assert!(loans.iter().all(|&(age, on, fee)| age <= 22 && on && fee), "{loans:?}");
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


