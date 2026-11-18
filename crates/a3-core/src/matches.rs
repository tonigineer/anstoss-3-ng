//! The league matchdays of a running season: every game the league blocks
//! still hold open on a calendar day is scored here and written back where
//! the original's played matches put them. The 3D match of the human's club
//! is out of the replica's scope, so its games are scored like the others.

use crate::calendar::Calendar;
use crate::cards;
use crate::individual::IndividualTables;
use crate::finances::CLUB_TO_TEAM;
use crate::opponent::{CUPS_AT, League, PAIRING, PAIRINGS, cup_pairings};
use crate::random::Random;
use crate::records::{PLANNER_DAYS, PLANNER_ENTRY, Records, Team};
use crate::report;
use crate::season::Season;
use crate::settlement;
use crate::statistics::{self, ALL};
use crate::tickets;
use crate::world::{self, World};

/// The stadiums (`0x1a48138[index]`), and the capacity within.
const STADIUM_AT: u32 = 0x1a48138;
const CAPACITY: usize = 0x3c;
/// The plan's attendance and its season-ticket holders (club +0x120,
/// +0x124), and the gate money booked match by match — the league's in
/// club +0x230, the cup's in +0x234, against the cup's planned gate at
/// +0x12c, a money and not a crowd.
const PLAN_ATTENDANCE: usize = 0x120;
const PLAN_SEASON_TICKETS: usize = 0x124;
const BOOKED_LEAGUE: usize = 0x230;
const BOOKED_CUP: usize = 0x234;

/// Strengths to reckon by when a team's record (G+0xb8) carries none: the
/// goalkeeper, defenders, midfield and attack in tenths, attack and hold
/// even so the luck alone makes the goals.
pub(crate) const EVEN: [i16; 4] = [340; 4];
/// The home side's edge, in the strengths' tenths.
pub(crate) const HOME_EDGE: i32 = 50;
/// How much the difference of the strengths is worth a goal.
const GOAL_UNITS: i32 = 200;

/// Play the games of the season's day: for every country the game follows,
/// every league whose calendar row holds this day as one of its matchdays
/// and has not played that matchday yet gets its pairs scored, and so does
/// the country's cup when the day is one of its rounds. The leagues that
/// played, by country and level, and the human clubs whose matches left
/// them a report. `listed` is the day's match list ([`list_day`]) the
/// original plays (`FUN_00710839` on the buffer at `0x1baf714`): a league
/// on it has its controlled pairs there, one left off it none.
pub fn play_day(
    records: &mut Records,
    world: &World,
    calendar: &Calendar,
    season: &mut Season,
    individual: &IndividualTables,
    listed: &[Listed],
) -> MatchDay {
    let day = season.day;
    let mut played = Vec::new();
    let mut faced = Vec::new();
    let mut controls = Vec::new();
    let mut cups = Vec::new();
    for slot in 0..world::SLOTS {
        let Some(nation) = world.country(slot).map(|country| country.nation) else {
            continue;
        };
        if !records.globals.played(i16::try_from(nation).unwrap_or(0)) {
            continue;
        }
        faced.extend(cup_day(records, nation, calendar, day, individual, &mut season.random, &mut cups));
        supercup_day(records, nation, calendar, day, &mut season.random);
        league_cup_day(records, nation, calendar, day, &mut season.random);
        for level in 0..records.countries.leagues(usize::from(nation)) {
            let level = level as u8;
            let Some(row) = calendar.row(nation, usize::from(level)) else {
                continue;
            };
            let Some(matchday) = calendar.matchday(row, day) else {
                continue;
            };
            let due = League::of(records, nation, level).is_some_and(|league| {
                league.played() + 1 == matchday && !league.open_games().is_empty()
            });
            if due {
                let picks = listed.iter().find(|(league, _)| *league == (nation, level)).map_or(&[][..], |(_, p)| p);
                faced.extend(settle(records, nation, level, day, individual, picks, &mut season.random, &mut controls));
                played.push((nation, level));
            }
        }
    }
    crate::training_day::refresh_individual(records, &mut season.clubs, world);
    let reported = faced.iter().map(|f| f.club).collect();
    MatchDay { leagues: played, reported, faced, controls, cups }
}

/// A calendar entry's competition, its top bits (`& 0x7f000000`); none
/// on row 0 lets the leagues list their matchdays.
const LISTING_KIND: u32 = 0x7f00_0000;
/// A league matchday's entry runs below this (`FUN_0062a325`'s `< 0x3c`).
const LISTING_MATCHDAYS: u32 = 0x3c;
/// The groups of a split league's second phase each list four pairs
/// (`FUN_00690a9e`'s `FUN_0078a810(4)`).
const SPLIT_PAIRS: u32 = 4;

/// The draws of the day's match list (`FUN_0062a325` into the buffer at
/// `0x1baf714`): with no international or European kind on row 0 of the
/// calendar, the main country's leagues by level (`DAT_00b5cbb0`, its
/// `0xae60c4` count), then each further country's first league
/// (`DAT_00b5cbb4`, `DAT_00b5cbae` of them), each whose entry for the
/// day is a league matchday (1 to 0x3b) lists its pairs and marks one of
/// them for the doping control (`FUN_00692045` copying the league, no
/// draw; `FUN_00690a9e`, [`listing_picks`]). The cups' kinds list their
/// ties without these: the relegation's (0x10000000, `FUN_006d73a3`)
/// and row 0's world matches (0x02000000, `FUN_00675698`'s shuffle) draw
/// as well, not here yet. The picks, league by league.
pub fn list_day(records: &Records, calendar: &Calendar, day: usize, random: &mut Random) -> Vec<Listed> {
    let mut picks = Vec::new();
    if calendar.entry(0, day) & LISTING_KIND != 0 {
        return picks;
    }
    let globals = &records.globals;
    let main = u16::try_from(globals.main_country()).unwrap_or(0);
    let countries = globals.more_countries();
    let more = usize::try_from(globals.more_count()).unwrap_or(0).min(countries.len());
    let mains = (0..records.countries.leagues(usize::from(main)).max(0)).map(|level| (main, level as u8));
    let firsts = countries[..more].iter().map(|&c| (u16::try_from(c).unwrap_or(0), 0));
    for (country, level) in mains.chain(firsts) {
        let Some(row) = calendar.row(country, usize::from(level)) else { continue };
        let entry = calendar.entry(row, day);
        if entry == 0 || entry >= LISTING_MATCHDAYS {
            continue;
        }
        if let Some(league) = League::of(records, country, level) {
            picks.push(((country, level), listing_picks(&league, entry as usize, random)));
        }
    }
    picks
}

/// A league on the day's match list, by country and level, and the pairs
/// its doping control takes ([`listing_picks`]).
pub type Listed = ((u16, u8), Vec<u32>);

/// The pair of a league's matchday the doping control takes
/// (`FUN_00690a9e`, the list's +0x460): one of its `teams / 2`; past the
/// regular rounds of a league split in two (G+0x1c; `FUN_0068fd13`'s
/// `teams * 2 - 2`, or `* 4 - 4` with four rounds at G+0x1a) one of four
/// in each group.
fn listing_picks(league: &League, matchday: usize, random: &mut Random) -> Vec<u32> {
    if league.split() == 0 || matchday <= league.regular_matchdays() {
        vec![random.below(u32::try_from(league.teams() / 2).unwrap_or(0))]
    } else {
        vec![random.below(SPLIT_PAIRS), random.below(SPLIT_PAIRS)]
    }
}

/// What a day's matches leave for the dialogs behind them: the leagues
/// that played, by country and level, and of each match a human's club
/// played, that club (`FUN_00551818`: the home side where it is a
/// human's, else the away side), in the order of the day's matches, and
/// the match itself; and the doping controls of the human clubs, for
/// dialog 41 each.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MatchDay {
    pub leagues: Vec<(u16, u8)>,
    pub reported: Vec<(u8, i16)>,
    pub faced: Vec<Faced>,
    pub controls: Vec<crate::doping_control::Control>,
    /// The humans' national cup ties, as the interview reads them.
    pub cups: Vec<crate::interview::Fixture>,
}

/// A match a human's club played: the club its report is shown for, both
/// sides, home first, the crowd, and the visiting fans where the replica
/// draws them (the match record's +0x34 and +0x54, which the original
/// reckons before the match, `FUN_0078b26c` in `FUN_006ef296`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Faced {
    pub club: (u8, i16),
    pub sides: [(u8, i16); 2],
    pub attendance: i32,
    pub fans: Option<i32>,
    /// The half's score, home first (+0x12, +0x13).
    pub half: [u8; 2],
}

/// What a match of today is: a league's, on its matchday counted from 1,
/// or a national cup's tie, by its calendar code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fixture {
    League { level: u8, matchday: usize },
    Cup { code: u32 },
}

/// The matches a human's club plays today, home side first, in the order
/// [`play_day`] plays them, before any is played: the national cup's ties
/// and the leagues' matchdays that are due (the day's match records the
/// original marks at +0x474 in `FUN_006ef296`, `FUN_00677428` over both
/// sides).
pub fn human_fixtures(records: &Records, world: &World, calendar: &Calendar, day: usize) -> Vec<([(u8, i16); 2], Fixture)> {
    let mut fixtures = Vec::new();
    for slot in 0..world::SLOTS {
        let Some(nation) = world.country(slot).map(|country| country.nation) else {
            continue;
        };
        if !records.globals.played(i16::try_from(nation).unwrap_or(0)) {
            continue;
        }
        let nat = u8::try_from(nation).unwrap_or(0);
        if let Some((code, ties)) = cup_ties(records, nation, calendar, day) {
            let turned = code & CUP_SECOND != 0;
            let u16_at = |p: &[u8], at: usize| u16::from_le_bytes([p[at], p[at + 1]]) as i16;
            for (_, p) in ties {
                let (h, a) = if turned { (4, 0) } else { (0, 4) };
                fixtures.push(([(p[h], u16_at(&p, h + 2)), (p[a], u16_at(&p, a + 2))], Fixture::Cup { code }));
            }
        }
        for level in 0..records.countries.leagues(usize::from(nation)) {
            let level = level as u8;
            let Some(row) = calendar.row(nation, usize::from(level)) else { continue };
            let Some(matchday) = calendar.matchday(row, day) else { continue };
            let Some(league) = League::of(records, nation, level) else { continue };
            if league.played() + 1 != matchday {
                continue;
            }
            for (home, away, _) in league.open_games() {
                fixtures.push(([(nat, league.team(home)), (nat, league.team(away))], Fixture::League { level, matchday }));
            }
        }
    }
    fixtures.retain(|&(sides, _)| report::human_side(records, sides).is_some());
    fixtures
}

/// The country's cup round of the day, if the day is one: its calendar
/// code and the ties still to play, by their pairing's index and bytes —
/// a first game finds its pairing open, a second leg or a replay its first
/// game played and nothing decided yet.
fn cup_ties(records: &Records, nation: u16, calendar: &Calendar, day: usize) -> Option<(u32, Vec<(usize, [u8; PAIRING])>)> {
    let row = calendar.row(nation, 0)?;
    let entry = calendar.entry(row, day);
    if entry & 0x7f00_0000 != CUP_CODE {
        return None;
    }
    let round = usize::try_from(entry & 0xffff).unwrap_or(0);
    let slot = records.countries.slot(usize::from(nation));
    let cup = records.block(&format!("{CUPS_AT:#x}[{slot}]"))?;
    let mode = usize::from(*cup.get(0x24 - 0x20)?).min(5);
    let (start, count) = cup_pairings(mode, round)?;
    let turned = entry & CUP_SECOND != 0;
    let ties = (start..start + count)
        .filter_map(|q| {
            let p = cup.get(PAIRINGS + q * PAIRING..PAIRINGS + (q + 1) * PAIRING)?;
            let open = if turned {
                p[0xc] & CUP_RESULT != 0 && p[0xc] & (CUP_PLAYED | CUP_SECOND_PLAYED) == 0
            } else {
                p[0xc] & CUP_RESULT == 0
            };
            (p[0] != 0 && p[4] != 0 && open).then(|| Some((q, p.try_into().ok()?)))?
        })
        .collect();
    Some((entry, ties))
}

/// The game's switch for the table after the matchday (byte 0x08 of the
/// switches at `0xb5d13c`, `DAT_00b5d144`): 0 off, 1 the leagues with a
/// human's club, 2 all.
const TABLES_SWITCH: usize = 0x08;
const ALL_TABLES: u8 = 2;
/// The most leagues its list holds (`FUN_0072c650`, 0x16 at 0x1c2eff2).
pub const TABLES_MOST: usize = 22;

/// The leagues the table after the matchday shows, in its order
/// (`FUN_00729216`, at the matchday's end in `FUN_007108d3`): of the
/// leagues that played today, the main country's by their levels, then
/// each further country's first league — with the switch at 2 all of
/// them, else those a human's club plays in (`FUN_006774f5`); none with
/// it off, as the dialog then does not open (`FUN_00729370`, event 200).
pub fn tables_after(records: &Records, played: &[(u16, u8)]) -> Vec<(u16, u8)> {
    let switch = records.globals.game_options()[TABLES_SWITCH];
    if switch != 1 && switch != ALL_TABLES {
        return Vec::new();
    }
    let humans = statistics::human_clubs(records);
    let shown = |country: u16, level: u8| {
        played.contains(&(country, level))
            && (switch == ALL_TABLES
                || humans.iter().any(|&(c, club)| {
                    u16::from(c) == country
                        && u16::try_from(club)
                            .ok()
                            .and_then(|n| records.team(country, n))
                            .is_some_and(|t| t.level() == level)
                }))
    };
    let globals = &records.globals;
    let main = u16::try_from(globals.main_country()).unwrap_or(0);
    let more = usize::try_from(globals.more_count()).unwrap_or(0).min(globals.more_countries().len());
    let leagues = records.countries.leagues(usize::from(main)).clamp(0, 0xff) as u8;
    let mains = (0..leagues).map(|level| (main, level));
    let countries = globals.more_countries();
    let firsts = countries[..more].iter().map(|&c| (u16::try_from(c).unwrap_or(0), 0));
    mains.chain(firsts).filter(|&(c, l)| shown(c, l)).take(TABLES_MOST).collect()
}

/// The cup's days sit on the country's first league row, coded 0x40 in the
/// top bits with the round in the low ones (`FUN_004cea5a`); Germany's row
/// names its six rounds at days 59, 115, 157, 234, 262 and 328. The round's
/// pairings stand where the mode's row of the table at 0x8a5e10 says, the
/// final one pairing behind the last of them.
const CUP_CODE: u32 = 0x4000_0000;
/// A pairing's flags: the result stands, the tie went to extra time (text
/// 0x761) or to penalties (text 0x762), whose score stays as it was.
const CUP_PLAYED: u8 = 0x08;
const CUP_RESULT: u8 = 0x20;
const CUP_EXTRA: u8 = 0x80;
const CUP_SHOOT: u8 = 0x40;
/// The second game of a tie stands played (`FUN_006cc255` sets 0x30).
const CUP_SECOND_PLAYED: u8 = 0x10;
/// A cup day's code: the second game of its tie, the venue turned — a
/// second leg, or with 0x200000 England's replay of a level first game;
/// a first game that decides (0x80000), or one a replay follows when it
/// ends level (0x100000). Spain's and Italy's rounds go over two legs,
/// Germany's are single games.
const CUP_SECOND: u32 = 0x8000_0000;
const CUP_REPLAY: u32 = 0x0020_0000;
const CUP_REPLAYABLE: u32 = 0x0010_0000;
const CUP_DECIDES: u32 = 0x0008_0000;
/// The cup's last round, its final: the sixth, whatever the mode, one
/// pairing behind the mode's last row. The original tests the same with
/// `FUN_006d12a8`, which asks the round against the rounds table's sixth.
const FINAL_ROUND: usize = 6;

/// The year calendar's entry of a fixture: kind 6, its code at +4, the
/// clubs at +8 and +0xc, the score at +0x10, the flags at +0x14 — 0x20
/// for a result that stands, 0x08 on a cup tie — and the hosting club
/// again at +0x16 (`FUN_0062a0ad`). The season's booking keeps every
/// cup day from the start, a not-yet-drawn round naming no club; when
/// a club leaves the cup its day's entry falls away (`FUN_0072ded2`).
const YEAR_MATCH: u8 = 6;
const YEAR_HOME: usize = 8;
const YEAR_AWAY: usize = 0xc;
const YEAR_GOALS: usize = 0x10;
const YEAR_FLAGS: usize = 0x14;
const YEAR_VENUE: usize = 0x16;
const YEAR_RESULT: u8 = 0x20;
const YEAR_CUP: u8 = 0x08;

/// Play the country's cup round when the day is one: every drawn and still
/// open pairing is scored, the home side with its edge — a single game
/// or a second leg runs a tie to extra time, whose goals join the score,
/// and from there to a shootout, which only names the winner in the byte
/// at +0xd, 0 the tie's home side. A first leg only stands (+8/+9, flag
/// 0x20); the second, its venue turned, stores its goals at +0xa/+0xb and
/// decides on the sum, then the away goals; England's level first game
/// waits for its replay. Once the round's last game is played its winners are shuffled
/// and paired anew — the home side drawn too — where the next round's row
/// says, and the block's last drawn round (the byte at +0x28) follows.
/// Each home club books its gate: a crowd as in the league, the money on
/// its season's cup takings (club +0x234). The human's year calendar keeps
/// its entries: the played round fills its score into the day's entry, the
/// new drawing names his next tie there — and drops it once he is out.
fn cup_day(
    records: &mut Records,
    nation: u16,
    calendar: &Calendar,
    day: usize,
    individual: &IndividualTables,
    random: &mut Random,
    cups: &mut Vec<crate::interview::Fixture>,
) -> Vec<Faced> {
    let Some((entry, ties)) = cup_ties(records, nation, calendar, day) else {
        return Vec::new();
    };
    let round = usize::try_from(entry & 0xffff).unwrap_or(0);
    let slot = records.countries.slot(usize::from(nation));
    let name = format!("{CUPS_AT:#x}[{slot}]");
    let Some(mode) = records.block(&name).and_then(|cup| cup.get(0x24 - 0x20)).copied() else {
        return Vec::new();
    };
    let mode = usize::from(mode).min(5);
    let (Some((start, count)), Some(row)) = (cup_pairings(mode, round), calendar.row(nation, 0)) else {
        return Vec::new();
    };
    let u16_at = |p: &[u8], at: usize| u16::from_le_bytes([p[at], p[at + 1]]);
    // The scores drawn before the block is written, as in the leagues, the
    // sides they mark, and the gate money the home clubs book with them.
    let turned = entry & CUP_SECOND != 0;
    let replay = turned && entry & CUP_REPLAY != 0;
    let replayable = !turned && entry & CUP_REPLAYABLE != 0;
    let decides = entry & CUP_DECIDES != 0 || turned;
    let mut scored: Vec<(usize, [u8; PAIRING])> = Vec::new();
    let mut marks: Vec<(u8, u16, u8, u16, u8, u8)> = Vec::new();
    let mut gates: Vec<(u8, u16, u8, u16, i32)> = Vec::new();
    let mut through: Vec<(u8, u16)> = Vec::new();
    let mut beaten: Vec<(u8, u16)> = Vec::new();
    let mut reports: Vec<report::Played> = Vec::new();
    for (q, open) in ties {
        let p: &[u8] = &open;
        let mut tie = open;
        let held = |country: u8, at: usize| {
            records
                .team(u16::from(country), u16_at(p, at))
                .map(|team| team.strengths())
                .filter(|s| *s != [0; 4])
                .unwrap_or(EVEN)
        };
        // The second game turns the venue round (`FUN_006cc1c6`): the
        // tie's away side at home.
        let (h, a) = if turned { (4, 0) } else { (0, 4) };
        let (home, away) = (held(p[h], h + 2), held(p[a], a + 2));
        let (mut hg, hh) = goals(home, away, HOME_EDGE, random);
        let (mut ag, ah) = goals(away, home, -HOME_EDGE, random);
        // The result as `FUN_006cc255` stores it, in the tie's own sides:
        // a first leg only stands, a game that decides names the winner —
        // two legs by their sum, then the away goals (`FUN_00677ed4`); a
        // game a replay may follow decides only when it is not level.
        let own = |hg: u8, ag: u8| if turned { (ag, hg) } else { (hg, ag) };
        let sum = |f: u8, s2: u8| {
            if turned && !replay {
                (u16::from(tie[8]) + u16::from(f), u16::from(tie[9]) + u16::from(s2))
            } else {
                (u16::from(f), u16::from(s2))
            }
        };
        let mut flags = CUP_RESULT;
        let mut winner = None;
        let (first, second) = own(hg, ag);
        if replayable && first != second {
            flags |= CUP_PLAYED;
            winner = Some(u8::from(first < second));
        } else if decides {
            flags |= CUP_PLAYED;
            let (ta, tb) = sum(first, second);
            let mut won = ta.cmp(&tb);
            if won == std::cmp::Ordering::Equal && turned && !replay {
                // The tie's away side scored its away goals in the first
                // leg, its home side in this one.
                won = u16::from(first).cmp(&u16::from(tie[9]));
            }
            if won == std::cmp::Ordering::Equal {
                flags |= CUP_EXTRA;
                let (et, _) = goals(home, away, 0, random);
                let (at, _) = goals(away, home, 0, random);
                hg = hg.saturating_add(et / 2 + u8::from(et % 2 != 0));
                ag = ag.saturating_add(at / 2 + u8::from(at % 2 != 0));
                let (f, s2) = own(hg, ag);
                let (ta, tb) = sum(f, s2);
                won = ta.cmp(&tb);
                if won == std::cmp::Ordering::Equal {
                    flags |= CUP_SHOOT;
                    let total = |s: &[i16; 4]| s.iter().map(|&v| i32::from(v)).sum::<i32>().max(1);
                    let (hs, as_) = (total(&home), total(&away));
                    let home_wins = i32::try_from(random.below((hs + as_) as u32)).unwrap_or(0) < hs;
                    won = if home_wins != turned { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Less };
                }
            }
            winner = Some(u8::from(won == std::cmp::Ordering::Less));
        }
        let (first, second) = own(hg, ag);
        if turned {
            tie[0xa] = first;
            tie[0xb] = second;
            tie[0xc] |= flags | CUP_SECOND_PLAYED;
        } else {
            tie[8] = first;
            tie[9] = second;
            tie[0xc] = flags;
        }
        tie[0xd] = winner.unwrap_or(0);
        scored.push((q, tie));
        marks.push((p[h], u16_at(p, h + 2), p[a], u16_at(p, a + 2), hg, ag));
        if let Some(w) = winner {
            let at = usize::from(w);
            through.push((p[at * 4], u16_at(p, at * 4 + 2)));
            beaten.push((p[(1 - at) * 4], u16_at(p, (1 - at) * 4 + 2)));
        }
        // The home club's gate: a crowd as in the league, and the money it
        // brings — the played match's reckoning (FUN_0078b26c) at a mark
        // no cup round holds, which leaves it as it is.
        let (country, number) = (p[h], u16_at(p, h + 2));
        let (an, away_id) = (p[a], u16_at(p, a + 2));
        let mut crowd = 0;
        let mut takings = 0;
        if let Some(t) = records.team(u16::from(country), number) {
            let cap = capacity(records, u16::from(country), number);
            let att = attendance(t, cap, random);
            let money = tickets::booked_gate(records, t, att, 0);
            if money != 0 {
                gates.push((country, number, an, away_id, money));
            }
            crowd = att;
            takings = money;
        }
        book_tie_crowd(records, (country, number), (an, away_id), [hg, ag], crowd);
        // The match as its record would keep it for the report: a second
        // leg with the first's goals as the tie keeps them, the away
        // side's first (+0x08, +0x09), and the extra time and penalties.
        let leg = turned && !replay;
        reports.push(report::Played {
            sides: [(country, number as i16), (an, away_id as i16)],
            goals: [hg, ag],
            half: [hh, ah],
            kind: entry,
            flags: (flags & (CUP_EXTRA | CUP_SHOOT)) | if leg { CUP_SECOND_PLAYED } else { 0 },
            leg: if leg { [tie[8], tie[9]] } else { [0, 0] },
            attendance: crowd,
            gate: takings,
            scorers: Default::default(),
        });
    }
    if let Some(cup) = records.block_mut(&name) {
        for (q, tie) in scored {
            let at = PAIRINGS + q * PAIRING;
            if let Some(p) = cup.get_mut(at..at + PAIRING) {
                p.copy_from_slice(&tie);
            }
        }
    }
    // The final decided goes into the country's cup winners and its
    // winner's count of cups ([`crate::honours::cup_final`]).
    if round >= FINAL_ROUND {
        let tie = records
            .block(&name)
            .and_then(|b| b.get(PAIRINGS + start * PAIRING..PAIRINGS + start * PAIRING + crate::honours::FINAL_PAIRING))
            .map(<[u8]>::to_vec);
        if let Some(tie) = tie.filter(|t| t[0xc] & CUP_PLAYED != 0) {
            crate::honours::cup_final(records, nation, &tie);
        }
    }
    book_cup_gates(records, &gates, entry);
    // What the ties leave on the men who played them, and the human
    // clubs' reports of their ties (`FUN_00550f29`).
    let mut reported = Vec::new();
    for (&(hc, hn, ac, an, hg, ag), mut played) in marks.iter().zip(reports) {
        let extra = played.flags & CUP_EXTRA != 0;
        let scorers = mark_cup(records, hc, hn, ac, an, hg, ag, extra, individual, random);
        if let Some(club) = report::human_side(records, played.sides) {
            played.scorers = scorers;
            cups.push(crate::interview::Fixture {
                sides: played.sides,
                goals: played.goals.map(i16::from),
                kind: played.kind,
                leg: played.leg.map(i16::from),
                engine: false,
            });
            report::write(records, &played, random);
            reported.push(Faced { club, sides: played.sides, attendance: played.attendance, fans: None, half: played.half });
        }
    }
    // Every club that came through its tie gets what the round pays: the
    // trainer's premium for reaching the round and the main sponsor's cup
    // extra (`FUN_006cebcd`; its counter of cup rounds `FUN_006cdb29` the
    // replica reads off the block instead). The final won adds the
    // manager's premium for a title and the sponsor's second extra
    // (`FUN_006cec7f`, the round `FUN_006d12a8` names the cup's last).
    for ((country, number), (bc, bn)) in through.into_iter().zip(beaten) {
        settlement::cup_premium(records, u16::from(country), number, round >= FINAL_ROUND);
        // The final counts both ways in the men who played it: its winner
        // the cup (kind 4), its loser the finalist's part (kind 0xd,
        // `FUN_006b50a4` at `FUN_006ce00e`'s last round, which
        // `FUN_006d12a8` names) — a kind the game marks nowhere, so it
        // only fills the career record's own counter.
        if round >= FINAL_ROUND {
            crate::honours::title(records, u16::from(country), number, crate::honours::CUP);
            crate::honours::title(records, u16::from(bc), bn, crate::honours::CUP_FINALIST);
        }
    }
    // On the human's year calendar the round's day fills its score, the
    // flags saying the result stands (0x20) and the tie a cup one (0x08);
    // the calendar slots its human clubs hold are kept for the drawing.
    let mut slots: Vec<usize> = Vec::new();
    for (hc, hn, ac, an, hg, ag) in &marks {
        for (country, number) in [(*hc, *hn), (*ac, *an)] {
            let Some(slot) = human_slot(records, country, number) else {
                continue;
            };
            slots.push(slot);
            if let Some(cal) = records.calendars.get_mut(slot) {
                let mut e = cal.entry(day);
                if e[0] == YEAR_MATCH
                    && u32::from_le_bytes([e[4], e[5], e[6], e[7]]) & 0x7f00_0000 == CUP_CODE
                {
                    e[YEAR_GOALS] = *hg;
                    e[YEAR_GOALS + 1] = *ag;
                    e[YEAR_FLAGS] |= YEAR_RESULT | YEAR_CUP;
                    cal.set_entry(day, e);
                }
            }
        }
    }
    slots.sort();
    slots.dedup();
    // The round is through when its every drawn pairing stands played; the
    // final's winner needs no drawing on.
    if round >= 6 {
        return reported;
    }
    let mut winners: Vec<(u8, u16)> = Vec::new();
    let drawn = records.block(&name).is_some_and(|cup| {
        for q in start..start + count {
            let Some(p) = cup.get(PAIRINGS + q * PAIRING..PAIRINGS + (q + 1) * PAIRING) else {
                continue;
            };
            if p[0] == 0 {
                continue;
            }
            if p[0xc] & CUP_PLAYED == 0 {
                return false;
            }
            let at = usize::from(p[0xd] != 0);
            winners.push((p[at * 4], u16_at(p, at * 4 + 2)));
        }
        true
    });
    let Some((next, pairs)) = drawn.then(|| cup_pairings(mode, round + 1)).flatten() else {
        return reported;
    };
    if winners.len() != 2 * pairs {
        return reported;
    }
    for i in (1..winners.len()).rev() {
        let j = usize::try_from(random.below(i as u32 + 1)).unwrap_or(i);
        winners.swap(i, j);
    }
    if let Some(cup) = records.block_mut(&name) {
        for (j, pair) in winners.chunks_exact(2).enumerate() {
            let (a, b) = if random.below(2) == 1 {
                (pair[1], pair[0])
            } else {
                (pair[0], pair[1])
            };
            let [alo, ahi] = a.1.to_le_bytes();
            let [blo, bhi] = b.1.to_le_bytes();
            let row = [a.0, 0, alo, ahi, b.0, 0, blo, bhi, 0, 0, 0, 0, 0, 0];
            if let Some(p) = cup.get_mut(
                PAIRINGS + (next + j) * PAIRING..PAIRINGS + (next + j + 1) * PAIRING,
            ) {
                p.copy_from_slice(&row);
            }
        }
        if let Some(p) = cup.get_mut(0x28 - 0x20) {
            *p = u8::try_from(round + 1).unwrap_or(u8::MAX);
        }
    }
    // The next round's day on the country's first row carries the human's
    // new tie — both clubs, the home one as venue again — while the club
    // gone from the cup sees its entry fall away to nothing.
    // A round over two legs has two days, the second's venue turned (a
    // replay's day stays as it is, the first game not yet played).
    let next_days: Vec<(usize, u32)> = (0..PLANNER_DAYS)
        .map(|d| (d, calendar.entry(row, d)))
        .filter(|&(_, entry)| entry & 0x7f00_0000 == CUP_CODE && entry & CUP_REPLAY == 0 && usize::try_from(entry & 0xffff).unwrap_or(0) == round + 1)
        .collect();
    if next_days.is_empty() {
        return reported;
    }
    let mut ties: Vec<(usize, (u8, u16, u8, u16))> = Vec::new();
    for j in 0..pairs {
        let Some(p) = records.block(&name).and_then(|cup| {
            cup.get(PAIRINGS + (next + j) * PAIRING..PAIRINGS + (next + j + 1) * PAIRING)
        }) else {
            continue;
        };
        if p[0] == 0 {
            continue;
        }
        let tie = (p[0], u16_at(p, 2), p[4], u16_at(p, 6));
        for (country, number) in [(tie.0, tie.1), (tie.2, tie.3)] {
            if let Some(slot) = human_slot(records, country, number) {
                ties.push((slot, tie));
            }
        }
    }
    for slot in slots {
        let Some(cal) = records.calendars.get_mut(slot) else {
            continue;
        };
        for &(next_day, code) in &next_days {
            let Some((_, tie)) = ties.iter().find(|(s, _)| *s == slot) else {
                cal.set_entry(next_day, [0; PLANNER_ENTRY]);
                continue;
            };
            let (hc, hn, ac, an) = if code & CUP_SECOND != 0 { (tie.2, tie.3, tie.0, tie.1) } else { *tie };
            let mut e = cal.entry(next_day);
            if e[0] != YEAR_MATCH {
                e = [0; PLANNER_ENTRY];
                e[0] = YEAR_MATCH;
                e[1] = 1;
                e[4..8].copy_from_slice(&code.to_le_bytes());
            }
            e[YEAR_HOME] = hc;
            e[YEAR_HOME + 2..YEAR_HOME + 4].copy_from_slice(&hn.to_le_bytes());
            e[YEAR_AWAY] = ac;
            e[YEAR_AWAY + 2..YEAR_AWAY + 4].copy_from_slice(&an.to_le_bytes());
            e[YEAR_VENUE] = hc;
            e[YEAR_VENUE + 2..YEAR_VENUE + 4].copy_from_slice(&hn.to_le_bytes());
            e[YEAR_GOALS] = 0;
            e[YEAR_GOALS + 1] = 0;
            e[YEAR_FLAGS] = 0;
            cal.set_entry(next_day, e);
        }
    }
    reported
}

/// The calendar slot of a cup club's human manager, if it has one.
/// A supercup's days sit on the country's first league row, coded 0x30
/// with the round in the low bits: Germany's tournament plays its rounds
/// 0 to 3 and its final (4) on days 33 to 45, the others one final, Italy
/// and Spain over two legs — the second turned (the code's high bit) —,
/// the code's 0x80000 deciding the tie in that game.
const SUPERCUP_CODE: u32 = 0x3000_0000;
const SUPERCUP_DECIDES: u32 = 0x0008_0000;
const SUPERCUP_TURNED: u32 = 0x8000_0000;
const SUPERCUP_FINAL: usize = 4;
/// The countries' lists of finals (`DAT_00b19150` in the lists block, by
/// slot): twenty entries of the year and the final tie; the day of the
/// last one (`DAT_00b5df90`).
const FINALS_BLOCK: &str = "0xb0b6b8";
const FINALS_AT: usize = 0xb19150 - 0xb0b6b8;
const FINALS_PER_SLOT: usize = 0x140;
const FINAL_ENTRY: usize = 0x10;
const LAST_FINAL_DAY: usize = 0xb5df90;

/// Play the country's supercup round when the day is one (the round's
/// game scored as a cup tie is, `FUN_006cd4cf`): its tie's result stored
/// as `FUN_006cc255` stores it — the first leg's score at +8, the second's
/// at +0xa, the flags at +0xc (0x20 played, 0x08 decided, 0x80 extra time,
/// 0x40 penalties), the winner's side at +0xd, two legs by their sum and
/// the away goals —, and in a tournament the winner moved on: from the
/// first two ties to the third and fourth against the first and second
/// of the six, from those to the final's home and away side. The human's
/// year calendar takes the score and his next tie, and the final decided
/// goes into the country's list of finals. The supercup's gate books
/// nothing on the clubs: `FUN_0078b26c`'s branch for its code 0x30 only
/// fills the match's record. Its men keep the match's wear; the rest of
/// the players' marks are not built.
fn supercup_day(records: &mut Records, nation: u16, calendar: &Calendar, day: usize, random: &mut Random) {
    use crate::supercup::{FINAL_TIE, SIX, TIE, TIES};
    let Some(row) = calendar.row(nation, 0) else { return };
    let code = calendar.entry(row, day);
    if code & 0x7f00_0000 != SUPERCUP_CODE {
        return;
    }
    let round = (code & 0x3f) as usize;
    let tournament = crate::supercup::is_tournament(records, nation);
    let tie_at = if tournament && round < SUPERCUP_FINAL { TIES + round * TIE } else { FINAL_TIE };
    if !tournament && round != SUPERCUP_FINAL {
        return;
    }
    let slot = records.countries.slot(usize::from(nation));
    let name = format!("{:#x}[{slot}]", crate::supercup::OBJECT);
    let Some(tie) = records.block(&name).and_then(|b| b.get(tie_at..tie_at + TIE)).map(<[u8]>::to_vec) else { return };
    let u16_at = |p: &[u8], at: usize| u16::from_le_bytes([p[at], p[at + 1]]);
    if tie[0] == 0 || tie[4] == 0 {
        return;
    }
    let turned = code & SUPERCUP_TURNED != 0;
    if turned && tie[0xc] & CUP_RESULT == 0 || !turned && tie[0xc] & CUP_RESULT != 0 {
        return;
    }
    let held = |country: u8, number: u16| {
        records.team(u16::from(country), number).map(|team| team.strengths()).filter(|s| *s != [0; 4]).unwrap_or(EVEN)
    };
    let (sides, home_first) = if turned { ([(tie[4], u16_at(&tie, 6)), (tie[0], u16_at(&tie, 2))], false) } else { ([(tie[0], u16_at(&tie, 2)), (tie[4], u16_at(&tie, 6))], true) };
    let (home, away) = (held(sides[0].0, sides[0].1), held(sides[1].0, sides[1].1));
    let (mut hg, _) = goals(home, away, HOME_EDGE, random);
    let (mut ag, _) = goals(away, home, -HOME_EDGE, random);
    // The home side's crowd, drawn as at the cup's; the supercup books
    // no money for it, only its row in the international list.
    let crowd = records
        .team(u16::from(sides[0].0), sides[0].1)
        .map_or(0, |t| attendance(t, capacity(records, u16::from(sides[0].0), sides[0].1), random));
    // The tie's own sides' goals: the first leg's as played, the
    // second's turned back.
    let (own_first, own_second) = if home_first { (hg, ag) } else { (ag, hg) };
    let (total_home, total_away) = if turned {
        (u16::from(tie[8]) + u16::from(own_first), u16::from(tie[9]) + u16::from(own_second))
    } else {
        (u16::from(own_first), u16::from(own_second))
    };
    let mut flags = CUP_RESULT;
    let mut winner = None;
    if code & SUPERCUP_DECIDES != 0 || turned {
        flags |= CUP_PLAYED;
        let mut won = total_home.cmp(&total_away);
        if won == std::cmp::Ordering::Equal && turned {
            // Away goals (`FUN_00677ed4`): the tie's home side's in the
            // second leg against its away side's in the first.
            won = u16::from(own_first).cmp(&u16::from(tie[9]));
        }
        if won == std::cmp::Ordering::Equal {
            flags |= CUP_EXTRA;
            let (et, _) = goals(home, away, 0, random);
            let (at, _) = goals(away, home, 0, random);
            hg = hg.saturating_add(et / 2 + u8::from(et % 2 != 0));
            ag = ag.saturating_add(at / 2 + u8::from(at % 2 != 0));
            let (f, s2) = if home_first { (hg, ag) } else { (ag, hg) };
            won = (u16::from(f) + if turned { u16::from(tie[8]) } else { 0 }).cmp(&(u16::from(s2) + if turned { u16::from(tie[9]) } else { 0 }));
            if won == std::cmp::Ordering::Equal {
                flags |= CUP_SHOOT;
                let sum = |s: &[i16; 4]| s.iter().map(|&v| i32::from(v)).sum::<i32>().max(1);
                let (hs, as_) = (sum(&home), sum(&away));
                let home_wins = i32::try_from(random.below((hs + as_) as u32)).unwrap_or(0) < hs;
                won = if home_wins == home_first { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Less };
            }
        }
        winner = Some(u8::from(won == std::cmp::Ordering::Less));
    }
    book_tie_crowd(records, sides[0], sides[1], [hg, ag], crowd);
    // What the match leaves on both squads' fitness (`FUN_006f86e1`,
    // [`crate::sim::competition_wear`]): each side's eleven — the saved
    // formation where its record stands one, as the engine walk reads it
    // (`sim::saved_eleven`), else its strongest fit men — stand the ninety
    // minutes, extra time's hundred and twenty where it went there; the
    // rest lose a condition step.
    let extra = flags & CUP_EXTRA != 0;
    for (country, number) in sides {
        let Some(team) = records.team(u16::from(country), number) else { continue };
        let lineup = team.lineup();
        let men = if lineup.iter().all(|&id| id != 0) { lineup.to_vec() } else { eleven(&team.players(), records) };
        let full = if extra { 120 } else { 90 };
        let minutes: Vec<(u16, i32)> = men.into_iter().map(|id| (id, full)).collect();
        crate::sim::competition_wear(records, (country, number), &minutes, extra);
    }
    let (own_first, own_second) = if home_first { (hg, ag) } else { (ag, hg) };
    if let Some(b) = records.block_mut(&name) {
        if let Some(t) = b.get_mut(tie_at..tie_at + TIE) {
            if turned {
                t[0xa] = own_first;
                t[0xb] = own_second;
                t[0xc] |= flags | 0x10;
            } else {
                t[8] = own_first;
                t[9] = own_second;
                t[0xc] = flags;
            }
            t[0xd] = winner.unwrap_or(0);
        }
        // The winner moves on in a tournament.
        if let (true, Some(w)) = (tournament, winner) {
            let won_side = [tie[w as usize * 4], tie[w as usize * 4 + 1], tie[w as usize * 4 + 2], tie[w as usize * 4 + 3]];
            let country = b[0];
            match round {
                0 | 1 => {
                    let next = TIES + (round + 2) * TIE;
                    let seed = [b[SIX + 2 * round], b[SIX + 2 * round + 1]];
                    b[next..next + 4].copy_from_slice(&won_side);
                    b[next + 4] = country;
                    b[next + 6..next + 8].copy_from_slice(&seed);
                }
                2 => b[FINAL_TIE..FINAL_TIE + 4].copy_from_slice(&won_side),
                3 => b[FINAL_TIE + 4..FINAL_TIE + 8].copy_from_slice(&won_side),
                _ => {}
            }
        }
    }
    // The final decided goes into the country's list of finals, the
    // oldest of twenty dropped (`FUN_006ccc2a`, `FUN_006cd999`), and the
    // day is noted.
    if round == SUPERCUP_FINAL && winner.is_some() {
        // Its winner counts the supercup in its trainer's career
        // (`FUN_006cc61b`: `FUN_006b50a4` with kind 5).
        let w = usize::from(winner.unwrap_or(0) != 0) * 4;
        crate::honours::title(records, u16::from(tie[w]), u16_at(&tie, w + 2), crate::honours::SUPERCUP);
        let final_tie = records.block(&name).and_then(|b| b.get(FINAL_TIE..FINAL_TIE + FINAL_ENTRY - 2)).map(<[u8]>::to_vec);
        let year = records.globals.start_year();
        if let (Some(t), Some(b)) = (final_tie, records.block_mut(FINALS_BLOCK)) {
            let at = FINALS_AT + usize::from(slot) * FINALS_PER_SLOT;
            if let Some(list) = b.get_mut(at..at + FINALS_PER_SLOT) {
                list.copy_within(FINAL_ENTRY.., 0);
                let last = FINALS_PER_SLOT - FINAL_ENTRY;
                list[last..last + 2].copy_from_slice(&year.to_le_bytes());
                list[last + 2..].copy_from_slice(&t);
            }
        }
        records.globals.put::<u16>(LAST_FINAL_DAY, day as u16);
    }
    // The human's calendar: the day's score, and his supercup days anew.
    for (country, number) in [(tie[0], u16_at(&tie, 2)), (tie[4], u16_at(&tie, 6))] {
        let Some(slot) = human_slot(records, country, number) else { continue };
        if let Some(cal) = records.calendars.get_mut(slot) {
            let mut e = cal.entry(day);
            if e[0] == YEAR_MATCH && u32::from_le_bytes([e[4], e[5], e[6], e[7]]) & 0x7f00_0000 == SUPERCUP_CODE {
                e[YEAR_GOALS] = hg;
                e[YEAR_GOALS + 1] = ag;
                e[YEAR_FLAGS] |= YEAR_RESULT | YEAR_CUP;
                cal.set_entry(day, e);
            }
        }
        crate::year::rebook_supercup(records, calendar, slot, (u16::from(country), number), day + 1);
    }
}

/// A cup tie's gate on the clubs' season's cup takings (club +0x234),
/// as `FUN_0078b26c` books the cup's and England's league cup's alike
/// (codes 0x40 and 0x01): a single game that decides (0x80000 without
/// the second game's bit) belongs to both clubs, each booking half; where
/// the home club has a human's manager and the visitor has none, the
/// manager keeps his cup share (the getter `FUN_0069c962` reads the
/// manager's +0x1a4) of the visitor's half back for the black cash box
/// (`FUN_0069c869` adds it), at most all of it and never below nought,
/// and the visitor books the rest. A leg, a first game a replay may
/// follow or a replay leaves the whole gate with the home club.
fn book_cup_gates(records: &mut Records, gates: &[(u8, u16, u8, u16, i32)], code: u32) {
    let shared = code & (CUP_SECOND | CUP_DECIDES) == CUP_DECIDES;
    for &(hc, hn, ac, an, money) in gates {
        let mut books = vec![(hc, hn, money)];
        if shared {
            let half = money / 2;
            let kept = match (human_slot(records, hc, hn), human_slot(records, ac, an)) {
                (Some(i), None) => records.managers.get(i).map_or(0, |m| {
                    ((half as f32 as f64 * f64::from(m.cup_share_kept())) as i32).max(0)
                }),
                _ => 0,
            };
            if kept > 0
                && let Some(m) = human_slot(records, hc, hn).and_then(|i| records.managers.get_mut(i))
            {
                m.set_black_cash(m.black_cash().wrapping_add(kept));
            }
            books = vec![(hc, hn, half), (ac, an, half - kept)];
        }
        for (country, number, money) in books {
            if let Some(t) = records.team_mut(u16::from(country), number) {
                let old = t.get::<i32>(BOOKED_CUP + CLUB_TO_TEAM);
                t.put::<i32>(BOOKED_CUP + CLUB_TO_TEAM, old.wrapping_add(money));
            }
        }
    }
}

/// England's league cup days on its first league row, coded 0x01 with the
/// round low: rounds over two legs (the second turned, the code's high
/// bit) and single games that decide (0x80000).
const LEAGUE_CUP_CODE: u32 = 0x0100_0000;
const ENGLAND: u16 = 0x10;

/// Play England's league cup round when the day is one: every tie of the
/// round scored as a cup tie is and stored as the supercup's are (the
/// first leg at +8, the second at +0xa, the flags, the winner, two legs
/// by their sum and the away goals); once the round stands decided its
/// winners and the entrants the next round takes in beside them are
/// shuffled and paired anew there, the home side drawn too, and the
/// block's last drawn round follows. The home clubs book their gates as
/// the cup's ([`book_cup_gates`]). (The players' marks and the premiums
/// are not built.)
fn league_cup_day(records: &mut Records, nation: u16, calendar: &Calendar, day: usize, random: &mut Random) {
    use crate::cup_draw::{ENTRANTS_AT, LEAGUE_CUP, joining};
    if nation != ENGLAND {
        return;
    }
    let Some(row) = calendar.row(nation, 0) else { return };
    let code = calendar.entry(row, day);
    if code & 0x7f00_0000 != LEAGUE_CUP_CODE {
        return;
    }
    let round = (code & 0x3f) as usize;
    let turned = code & SUPERCUP_TURNED != 0;
    let decides = code & SUPERCUP_DECIDES != 0 || turned;
    let Some(block) = records.block(LEAGUE_CUP).map(<[u8]>::to_vec) else { return };
    let mode = usize::from(u16::from_le_bytes([block[4], block[5]]));
    let Some((start, count)) = cup_pairings(mode, round) else { return };
    let u16_at = |p: &[u8], at: usize| u16::from_le_bytes([p[at], p[at + 1]]);
    let held = |country: u8, number: u16| {
        records.team(u16::from(country), number).map(|team| team.strengths()).filter(|s| *s != [0; 4]).unwrap_or(EVEN)
    };
    let mut results: Vec<(usize, [u8; PAIRING])> = Vec::new();
    let mut gates: Vec<(u8, u16, u8, u16, i32)> = Vec::new();
    let mut crowds = Vec::new();
    for q in start..start + count {
        let at = PAIRINGS + q * PAIRING;
        let Some(tie) = block.get(at..at + PAIRING) else { continue };
        let mut tie: [u8; PAIRING] = tie.try_into().unwrap_or([0; PAIRING]);
        if tie[0] == 0 || tie[4] == 0 || turned && tie[0xc] & CUP_RESULT == 0 || !turned && tie[0xc] & CUP_RESULT != 0 {
            continue;
        }
        let (a, b) = ((tie[0], u16_at(&tie, 2)), (tie[4], u16_at(&tie, 6)));
        let (home, away) = if turned { (held(b.0, b.1), held(a.0, a.1)) } else { (held(a.0, a.1), held(b.0, b.1)) };
        let (hg, _) = goals(home, away, HOME_EDGE, random);
        let (ag, _) = goals(away, home, -HOME_EDGE, random);
        // The home club's gate, a crowd as at the cup's.
        let ((hc, hn), (ac, an)) = if turned { (b, a) } else { (a, b) };
        if let Some(t) = records.team(u16::from(hc), hn) {
            let att = attendance(t, capacity(records, u16::from(hc), hn), random);
            let money = tickets::booked_gate(records, t, att, 0);
            if money != 0 {
                gates.push((hc, hn, ac, an, money));
            }
            crowds.push(((hc, hn), (ac, an), [hg, ag], att));
        }
        // The tie's own sides' goals.
        let (mut first, mut second) = if turned { (ag, hg) } else { (hg, ag) };
        let mut flags = CUP_RESULT;
        let mut winner = 0;
        if decides {
            flags |= CUP_PLAYED;
            let sum = |f: u8, s2: u8| if turned { (u16::from(tie[8]) + u16::from(f), u16::from(tie[9]) + u16::from(s2)) } else { (u16::from(f), u16::from(s2)) };
            let (ta, tb) = sum(first, second);
            let mut won = ta.cmp(&tb);
            if won == std::cmp::Ordering::Equal && turned {
                won = u16::from(first).cmp(&u16::from(tie[9]));
            }
            if won == std::cmp::Ordering::Equal {
                flags |= CUP_EXTRA;
                let (et, _) = goals(home, away, 0, random);
                let (at2, _) = goals(away, home, 0, random);
                let (eh, ea) = (et / 2 + u8::from(et % 2 != 0), at2 / 2 + u8::from(at2 % 2 != 0));
                let (ef, es) = if turned { (ea, eh) } else { (eh, ea) };
                first = first.saturating_add(ef);
                second = second.saturating_add(es);
                let (ta, tb) = sum(first, second);
                won = ta.cmp(&tb);
                if won == std::cmp::Ordering::Equal {
                    flags |= CUP_SHOOT;
                    let total = |s: &[i16; 4]| s.iter().map(|&v| i32::from(v)).sum::<i32>().max(1);
                    let (hs, as_) = (total(&home), total(&away));
                    let home_wins = i32::try_from(random.below((hs + as_) as u32)).unwrap_or(0) < hs;
                    won = if home_wins != turned { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Less };
                }
            }
            winner = u8::from(won == std::cmp::Ordering::Less);
        }
        if turned {
            tie[0xa] = first;
            tie[0xb] = second;
            tie[0xc] |= flags | 0x10;
        } else {
            tie[8] = first;
            tie[9] = second;
            tie[0xc] = flags;
        }
        tie[0xd] = winner;
        results.push((q, tie));
    }
    if results.is_empty() {
        return;
    }
    book_cup_gates(records, &gates, code);
    for (home, away, score, att) in crowds {
        book_tie_crowd(records, home, away, score, att);
    }
    let Some(b) = records.block_mut(LEAGUE_CUP) else { return };
    for (q, tie) in &results {
        let at = PAIRINGS + q * PAIRING;
        b[at..at + PAIRING].copy_from_slice(tie);
    }
    // The round decided: its winners and the next round's entrants on.
    let decided: Option<Vec<[u8; 4]>> = (start..start + count)
        .map(|q| {
            let at = PAIRINGS + q * PAIRING;
            let p = &b[at..at + PAIRING];
            if p[0] == 0 {
                Some(None)
            } else if p[0xc] & CUP_PLAYED == 0 {
                None
            } else {
                let w = usize::from(p[0xd] != 0) * 4;
                Some(Some([p[w], p[w + 1], p[w + 2], p[w + 3]]))
            }
        })
        .collect::<Option<Vec<_>>>()
        .map(|v| v.into_iter().flatten().collect());
    let Some(mut sides) = decided else { return };
    if round >= 6 {
        // The final decided ends the cup. It credits no career: the
        // tournament's honours (`FUN_0067e911` with kinds 0xb and 0xc)
        // are a national team's world's and European title, awarded only
        // when the kind of the final is a tournament's (0xe) — a cup's
        // kinds below 8 award nothing.
        return;
    }
    let Some((next, pairs)) = cup_pairings(mode, round + 1) else { return };
    let (from, joined) = joining(mode, round + 1);
    for k in from..from + joined {
        let at = ENTRANTS_AT + k * 4;
        if let Some(e) = b.get(at..at + 4) {
            sides.push([e[0], e[1], e[2], e[3]]);
        }
    }
    if sides.len() != 2 * pairs {
        return;
    }
    for i in (1..sides.len()).rev() {
        let j = usize::try_from(random.below(i as u32 + 1)).unwrap_or(i);
        sides.swap(i, j);
    }
    for (j, pair) in sides.chunks_exact(2).enumerate() {
        let (x, y) = if random.below(2) == 1 { (pair[1], pair[0]) } else { (pair[0], pair[1]) };
        let at = PAIRINGS + (next + j) * PAIRING;
        if let Some(p) = b.get_mut(at..at + PAIRING) {
            p.fill(0);
            p[0] = x[0];
            p[2..4].copy_from_slice(&x[2..4]);
            p[4] = y[0];
            p[6..8].copy_from_slice(&y[2..4]);
        }
    }
    if let Some(d) = b.get_mut(0x28 - 0x20) {
        *d = u8::try_from(round + 1).unwrap_or(u8::MAX);
    }
}

/// A tie's crowd into the biggest-attendance lists — the cup's, the
/// supercup's and England's league cup's alike: only the international
/// one, as the settlement (`FUN_007108d3` at 0x71295c) masks the record's
/// family at +0x20 with `0x7f000000` and compares it against `0x3f`, so
/// that only the league's family nought reaches the league's list. The
/// row's ground is the home club's and its second result stays nought.
fn book_tie_crowd(records: &mut Records, home: (u8, u16), away: (u8, u16), score: [u8; 2], att: i32) {
    statistics::book_crowd(
        records,
        false,
        u16::from(home.0),
        0,
        home,
        statistics::Crowd {
            attendance: u32::try_from(att.max(0)).unwrap_or(0),
            home: (home.0, home.1 as i16),
            away: (away.0, away.1 as i16),
            goals: score,
            second: [0, 0],
            flags: 0,
        },
    );
}

fn human_slot(records: &Records, country: u8, number: u16) -> Option<usize> {
    records
        .team(u16::from(country), number)
        .filter(|team| team.human() > 0)
        .map(|team| usize::from(team.human()) - 1)
}

/// What a cup tie leaves on its men: the eleven each side puts on the
/// pitch — its own suspended men (+0x234) held off the round — stand for
/// their season's cup appearances in the career's five bytes (+0x25f),
/// and every goal, drawn over the eleven as in the league, stands in the
/// season's cup goals (+0x23d) and the career's (+0x23f). Cards follow
/// the league's odds into the cup's counts (+0x231: yellow, red, yellow-
/// red) and the country's cup rule (cards::cup); a suspending card marks
/// the cup's suspension byte (+0x234: 0x80 red, 0x40 yellow-red, else a
/// yellow accumulation) and holds its man off the next round, where the
/// tie clears it. The league's 46 matchday rows hold its games alone, so
/// a cup tie leaves no row there, and the season's cup grade (+0x233)
/// and the substitutes' shares are the original's match engine, still
/// unportrayed.
fn mark_cup(
    records: &mut Records,
    hc: u8,
    hn: u16,
    ac: u8,
    an: u16,
    full: u8,
    a_full: u8,
    extra: bool,
    individual: &IndividualTables,
    random: &mut Random,
) -> [Vec<u16>; 2] {
    let mut scorers: [Vec<u16>; 2] = Default::default();
    for (side, (country, team, goals)) in [(hc, hn, full), (ac, an, a_full)].into_iter().enumerate() {
        let nation = u16::from(country);
        let squad = records
            .team(nation, team)
            .map(|t| t.players())
            .unwrap_or_default();
        let fit: Vec<u16> = squad
            .iter()
            .copied()
            .filter(|&id| {
                records.player(id).is_some_and(|p| available(records, id) && p.cup_ban() == 0)
            })
            .collect();
        let eleven = eleven(&fit, records);
        let weights: Vec<u32> = eleven
            .iter()
            .map(|&id| {
                records.player(id).map_or(0, |p| {
                    u32::from(p.strength()) * KIND_WEIGHTS[usize::from(p.positions()[0]).min(10)]
                })
            })
            .collect();
        let mut scored = vec![0u8; eleven.len()];
        for _ in 0..u32::from(goals) {
            let Some(k) = draw(&weights, random) else {
                break;
            };
            scored[k] = scored[k].saturating_add(1);
            scorers[side].push(eleven[k]);
            // Every goal teaches its scorer (`FUN_00740aad`); the
            // replica's draw names no kind, so only Torinstinkt.
            crate::training_day::goal_lessons(records, individual, (nation, team), eleven[k], -1);
        }
        // A fresh card's suspension of this round's marking stands over
        // the serving of the last; the men who wore one into the tie sat
        // it out and stand free of it now.
        let mut carded: Vec<u16> = Vec::new();
        for (&id, &n) in eleven.iter().zip(&scored) {
            let card = if random.below(110) == 0 {
                3
            } else if random.below(340) == 0 {
                2
            } else if random.below(9) == 0 {
                1
            } else {
                0
            };
            let suspends_yellow = card == 1
                && records.player(id).is_some_and(|p| {
                    let mut q = p.clone();
                    let mut counted = q.cup_cards();
                    counted[0] = counted[0].saturating_add(1);
                    q.set_cup_cards(counted);
                    cards::cup(records, nation, &q)
                });
            if let Some(p) = records.player_mut(id) {
                let mut apps = p.career_cup_appearances();
                apps[0] = apps[0].saturating_add(1);
                p.set_career_cup_appearances(apps);
                if n > 0 {
                    let mut goals = p.goals();
                    goals[1] = goals[1].saturating_add(n);
                    p.set_goals(goals);
                    p.set_career_cup_goals(p.career_cup_goals().saturating_add(n));
                }
                if card != 0 {
                    let mut cards = p.cup_cards();
                    match card {
                        1 => cards[0] = cards[0].saturating_add(1),
                        2 => cards[2] = cards[2].saturating_add(1),
                        _ => cards[1] = cards[1].saturating_add(1),
                    }
                    p.set_cup_cards(cards);
                    if card >= 2 || suspends_yellow {
                        p.set_cup_ban(match card {
                            3 => 0x80,
                            2 => 0x40,
                            _ => 1,
                        });
                        carded.push(id);
                    }
                }
            }
        }
        // What the tie leaves on the squad's fitness (`FUN_006f86e1`): the
        // eleven stand the whole tie, extra time's hundred and twenty
        // where it went there, the rest lose a condition step.
        let full = if extra { 120 } else { 90 };
        let minutes: Vec<(u16, i32)> = eleven.iter().map(|&id| (id, full)).collect();
        crate::sim::competition_wear(records, (country, team), &minutes, extra);
        // The men who stood leave the youth round (`FUN_007471bd`),
        // in the squad's order as the graded pass walks them.
        for &id in squad.iter().filter(|id| eleven.contains(id)) {
            crate::training_day::leave_youth_round(records, (nation, team), id);
        }
        for id in squad {
            if carded.contains(&id) {
                continue;
            }
            if let Some(p) = records.player_mut(id) {
                p.set_cup_ban(0);
            }
        }
    }
    scorers
}

/// Score a league's next matchday and write it to its block: the four goal
/// bytes of each pair, the count of matchdays played, and the crowd — the
/// matchday's gates together, as the original keeps them by the hundred
/// and fifty thousand — then the places the table shows after the
/// matchday, and each club's season record, goals and table place. Every
/// home club books its gate: the attendance on its played matches, the
/// money on its season's takings, and the league's attendance row
/// follows. Each match leaves its mark on its players: appearances and
/// scorers' goals in their records, and in the league's top-scorers list.
/// Both clubs keep their gate figures: the home club its crowd, income and
/// the visiting fans, the away club the travelling gate of the game. The
/// human's year calendar fills the day's fixture with the new score, its
/// flag saying the result stands.
fn settle(
    records: &mut Records,
    nation: u16,
    level: u8,
    day: usize,
    individual: &IndividualTables,
    picks: &[u32],
    random: &mut Random,
    controls: &mut Vec<crate::doping_control::Control>,
) -> Vec<Faced> {
    let Some(league) = League::of(records, nation, level) else {
        return Vec::new();
    };
    let Some(name) = League::block_name(records, nation, level) else {
        return Vec::new();
    };
    let games = league.open_games();
    // The byte where each place writes its place after this matchday, and
    // where the count and the crowd stand — all taken before the count is
    // raised, since they lie on the matchday as it was played.
    let strengths: Vec<[i16; 4]> = (0..league.teams())
        .map(|place| {
            let held = records
                .team(nation, league.team(place) as u16)
                .map(|team| team.strengths())
                .unwrap_or(EVEN);
            if held == [0; 4] { EVEN } else { held }
        })
        .collect();
    let places: Vec<Option<usize>> = (0..league.teams())
        .map(|place| league.place_slot(place))
        .collect();
    let played_at = league.played_slot();
    let crowd_at = league.crowd_slot();
    let played = (league.played() + 1) as i16;
    // The row of this matchday in the players' forty-six.
    let md = usize::from(league.played());
    let matchday = i32::try_from(league.played() + 1).unwrap_or(0);
    let scored: Vec<[u8; 4]> = games
        .iter()
        .map(|&(home, away, _)| {
            let (full, half) = goals(strengths[home], strengths[away], HOME_EDGE, random);
            let (a_full, a_half) = goals(strengths[away], strengths[home], -HOME_EDGE, random);
            [full, a_full, half, a_half]
        })
        .collect();
    // The gate each home club books for its match — the attendance, the
    // money and the stadium it fills — reckoned before the block and the
    // clubs are written.
    let homes: Vec<u16> = games.iter().map(|&(home, _, _)| league.team(home) as u16).collect();
    let aways: Vec<u16> = games.iter().map(|&(_, away, _)| league.team(away) as u16).collect();
    let booked: Vec<(i32, i32, i32)> = homes
        .iter()
        .map(|&team| match records.team(nation, team) {
            Some(t) => {
                let cap = capacity(records, nation, team);
                let att = attendance(t, cap, random);
                (att, tickets::booked_gate(records, t, att, matchday), cap)
            }
            None => (0, 0, 0),
        })
        .collect();
    let crowd: i32 = booked.iter().map(|&(att, _, _)| att).sum();
    let att_slot = statistics::attendance_slot(records, nation, level);
    if let Some(block) = records.block_mut(&name) {
        for ((_, _, at), result) in games.iter().zip(&scored) {
            if let Some(slot) = block.get_mut(*at..*at + 4) {
                slot.copy_from_slice(result);
            }
        }
        if let Some(slot) = block.get_mut(played_at..played_at + 2) {
            slot.copy_from_slice(&played.to_le_bytes());
        }
        if let Some(slot) = block.get_mut(crowd_at..crowd_at + 4) {
            slot.copy_from_slice(&crowd.to_le_bytes());
        }
        // The matchday's total goals stand for the betting shop's goals'
        // tip to guess at (`FUN_007811c5` reads the field at +0x18).
        let goals: i16 = scored.iter().map(|g| i16::from(g[0]) + i16::from(g[1])).sum();
        let at = crate::opponent::LEAGUE_FIELDS + 0x18;
        if let Some(slot) = block.get_mut(at..at + 2) {
            slot.copy_from_slice(&goals.to_le_bytes());
        }
    }
    let nat = u8::try_from(nation).unwrap_or(0);
    let takings: Vec<(i32, i32)> = booked.iter().map(|&(att, money, _)| (att, money)).collect();
    let mut visiting = Vec::with_capacity(games.len());
    for ((((home, _, _), (&team, (att, money, cap))), &away), &result) in
        games.iter().zip(homes.iter().zip(booked)).zip(&aways).zip(&scored)
    {
        // The visiting fans: a part of the away club's own plan (save A
        // reads 3926 with the away club planning 34000), never more than
        // came to the ground. The original draws them inside the match
        // (the match record's +0x54); the replica draws them over the plan.
        let plan = records
            .team(nation, away)
            .map_or(0, |a| a.get::<i32>(PLAN_ATTENDANCE + CLUB_TO_TEAM));
        let luck = 8 + i32::try_from(random.below(7)).unwrap_or(8);
        let fans = plan.max(0).wrapping_mul(luck) / 100;
        let fans = fans.clamp(0, att.max(0));
        visiting.push(fans);
        // The home club: its gate held among its gates, the money booked,
        // and — where it has a human's extra block — the visiting fans and
        // the gate's income of this home game, as the statistics pages
        // read them (the match record's +0x54 and +0x3c into extra +0x3f4
        // and +0x450, `FUN_007108d3`).
        // A human's home club against a computer's keeps its manager's
        // share of the gate back for his black cash box, at most all of it
        // and never below nought (`FUN_0078b26c` with `FUN_0069c90c`,
        // `FUN_0069c869`): ftol of the gate as a float times the share.
        let human = |n: u16| records.team(nation, n).map_or(0, |t| t.humans()[0]);
        let kept = match (human(team), human(away)) {
            (h, 0) if h > 0 => records.managers.get(usize::from(h) - 1).map_or(0, |m| {
                ((money as f32 as f64 * f64::from(m.gate_kept())) as i32).max(0)
            }),
            _ => 0,
        };
        if kept > 0
            && let Some(m) = usize::from(human(team)).checked_sub(1).and_then(|i| records.managers.get_mut(i))
        {
            m.set_black_cash(m.black_cash().wrapping_add(kept));
        }
        let mut host = (0usize, 0u8);
        if let Some(t) = records.team_mut(nation, team) {
            let old = t.get::<i32>(BOOKED_LEAGUE + CLUB_TO_TEAM);
            t.put::<i32>(BOOKED_LEAGUE + CLUB_TO_TEAM, old.wrapping_add(money - kept));
            let held = usize::from(t.home_games());
            let mut gates = t.gates();
            if let Some(entry) = gates.get_mut(held) {
                *entry = att;
                t.set_gates(gates);
            }
            t.set_home_games(u8::try_from(held + 1).unwrap_or(u8::MAX));
            host = (held, t.human());
        }
        if host.1 > 0
            && let Some(x) = records.extras.get_mut(usize::from(host.1) - 1)
        {
            let mut fan_gates = x.fan_gates();
            let mut incomes = x.home_incomes();
            if let (Some(f), Some(m)) = (fan_gates.get_mut(host.0), incomes.get_mut(host.0)) {
                *f = fans;
                *m = money;
                x.set_fan_gates(fan_gates);
                x.set_home_incomes(incomes);
            }
        }
        // The away club: its count of away games up, and the gate of the
        // travelling — the whole gate at the opponent's ground, the match
        // record's +0x34 into extra +0x398 (`FUN_0063f09b`).
        let mut guest = (0usize, 0u8);
        if let Some(a) = records.team_mut(nation, away) {
            let held = usize::from(a.away_games());
            a.set_away_games(u8::try_from(held + 1).unwrap_or(u8::MAX));
            guest = (held, a.human());
        }
        if guest.1 > 0
            && let Some(x) = records.extras.get_mut(usize::from(guest.1) - 1)
        {
            let mut away_gates = x.away_gates();
            if let Some(entry) = away_gates.get_mut(guest.0) {
                *entry = att;
                x.set_away_gates(away_gates);
            }
        }
        if let Some(slot) = att_slot {
            statistics::book_attendance(records, slot, *home, att, cap);
        }
        // The crowd goes into the biggest-attendance lists: the league's
        // own and the international one, as the original's settlement of
        // the day's record reaches both. The ground is the home club's,
        // and the row's second result — the other leg's score — stays
        // nought for a league match.
        statistics::book_crowd(
            records,
            true,
            nation,
            level,
            (nat, team),
            statistics::Crowd {
                attendance: u32::try_from(att.max(0)).unwrap_or(0),
                home: (nat, team as i16),
                away: (nat, away as i16),
                goals: [result[0], result[1]],
                second: [0, 0],
                flags: 0,
            },
        );
    }
    // What the goals leave on the players of both sides, and the human
    // clubs' reports of their matches (`FUN_00550f29`), the matchday from
    // 0 their kind.
    let mut reported = Vec::new();
    for ((((&home, &away), result), &(attendance, gate)), &fans) in
        homes.iter().zip(&aways).zip(&scored).zip(&takings).zip(&visiting)
    {
        let scorers =
            mark_match(records, nation, level, md, home, away, result[0], result[1], individual, random);
        let sides = [(nat, home as i16), (nat, away as i16)];
        if let Some(club) = report::human_side(records, sides) {
            let played = report::Played {
                sides,
                goals: [result[0], result[1]],
                half: [result[2], result[3]],
                kind: md as u32,
                attendance,
                gate,
                scorers,
                ..Default::default()
            };
            report::write(records, &played, random);
            reported.push(Faced { club, sides, attendance, fans: Some(fans), half: [result[2], result[3]] });
        }
    }
    // The matchday's doping control: the pair the day's list marked
    // (+0x460, drawn in `FUN_00690a9e`; in a split league's second phase
    // one in each group of four), both sides' men picked (`FUN_005dad80`,
    // home first) and those of a human's club tested (`FUN_00741aef` →
    // `FUN_005dae5f`).
    for (group, &pick) in picks.iter().enumerate() {
        let at = group * SPLIT_PAIRS as usize + pick as usize;
        if let (Some(&home), Some(&away)) = (homes.get(at), aways.get(at)) {
            let picked = [home, away].map(|n| (n, crate::doping_control::pick(records, nation, n, random)));
            for (n, men) in picked {
                controls.extend(crate::doping_control::test(records, nation, n, &men, random));
            }
        }
    }
    // Every offered Toto match keeps its result, wherever it stands in
    // the offer, so the settlement can pay the tips (`FUN_0078017d`).
    for (&[hg, ag, _, _], (&home, &away)) in scored.iter().zip(homes.iter().zip(&aways)) {
        crate::betting::matchday_result(records, (nat, home), (nat, away), (hg, ag));
    }
    // The human's year calendar keeps the matchday's result: the season's
    // booking named the fixture on the day's entry — kind 6, the matchday
    // plain as its code — and the played match fills its score in, the
    // flag saying the result stands (0x20; the cup's 0x08 stays off).
    for ((&team, &away), &[hg, ag, _, _]) in homes.iter().zip(&aways).zip(&scored) {
        let mut slots: Vec<usize> = Vec::new();
        for number in [team, away] {
            if let Some(slot) = human_slot(records, u8::try_from(nation).unwrap_or(0), number)
                && !slots.contains(&slot)
            {
                slots.push(slot);
            }
        }
        let code = u32::try_from(matchday).unwrap_or(0);
        for slot in slots {
            let Some(cal) = records.calendars.get_mut(slot) else {
                continue;
            };
            let mut e = cal.entry(day);
            if e[0] != YEAR_MATCH
                || u32::from_le_bytes([e[4], e[5], e[6], e[7]]) != code
                || e[YEAR_HOME + 2..YEAR_HOME + 4] != team.to_le_bytes()
                || e[YEAR_AWAY + 2..YEAR_AWAY + 4] != away.to_le_bytes()
            {
                continue;
            }
            e[YEAR_GOALS] = hg;
            e[YEAR_GOALS + 1] = ag;
            e[YEAR_FLAGS] |= YEAR_RESULT;
            cal.set_entry(day, e);
        }
    }
    // And the day has served: one match off every suspension of the
    // clubs that played. A card drawn this day set its count two matches
    // high — the day itself among them — so that one still stands when
    // the next matchday picks its men.
    for &team in homes.iter().chain(&aways) {
        let squad =
            records.team(nation, team).map(|t| t.players()).unwrap_or_default();
        for id in squad {
            if let Some(p) = records.player_mut(id) {
                let old = p.suspension();
                if old & 0x3f != 0 {
                    p.set_suspension((old & 0xc0) | ((old & 0x3f) - 1));
                }
            }
        }
    }
    // The table as it stands after the matchday, and the places its rows
    // write into the blocks' history.
    let Some(league) = League::of(records, nation, level) else {
        return reported;
    };
    let rows = statistics::table(records, &league, ALL);
    if let Some(block) = records.block_mut(&name) {
        for (place, rank) in rows.iter().enumerate().map(|(rank, row)| (row.place, rank + 1)) {
            if let Some(at) = places.get(place).and_then(|slot| *slot)
                && let Some(byte) = block.get_mut(at)
            {
                *byte = rank as u8;
            }
        }
    }
    for (rank, row) in rows.iter().enumerate() {
        let record = [
            u8::try_from(row.won).unwrap_or(u8::MAX),
            u8::try_from(row.drawn).unwrap_or(u8::MAX),
            u8::try_from(row.lost).unwrap_or(u8::MAX),
        ];
        let goals = [
            u8::try_from(row.goals_for).unwrap_or(u8::MAX),
            u8::try_from(row.goals_against).unwrap_or(u8::MAX),
        ];
        if let Some(team) = records.team_mut(nation, row.team as u16) {
            team.set_record(record);
            team.set_goals(goals);
            team.set_table_place(rank as u8 + 1);
        }
    }
    reported
}

/// Weights a goal is drawn by, over the position: nought for the keeper,
/// little for the defence, most for the attack — times the player's
/// strength. The positions run 1 to 10, keeper to striker; the special
/// kinds from 11 on (the exe's table at 0x89e23c) count among the attack.
const KIND_WEIGHTS: [u32; 11] = [0, 0, 1, 1, 1, 2, 2, 3, 4, 5, 6];

/// The minute the replica's substitutions take place at: the original
/// substitutes inside its engine (`FUN_006f8dc7` reading the minute at
/// the man's +0x58), which the league matches do not walk yet; Probe1's
/// first league day (the oracle's day 42) took its three men off between
/// the sixtieth and the eightieth minute and brought two of them on for
/// less than thirty.
const SUB_MINUTE: i32 = 70;

/// What a scored match leaves on its players: the eleven each side puts
/// on the pitch stand for their league match (+0x253) and their run of
/// matches in a row (+0x34e), broken for those listed but not picked;
/// every goal goes to one of the eleven, drawn over their weights — the
/// scorer's weight is his strength times his position's kind — and stands
/// in his season's league goals (+0x23c), his goals in the last match
/// (+0x249), his home goals when his side played at home (+0x252), and in
/// the league's top-scorer list. Each man on the pitch — the eleven and
/// the one to three who came on for the weakest of them — keeps his row of
/// the matchday (+0x319: starter, substitution touched, grade, card) with
/// his goals of it in +0x2eb; one to three substitutions a side, grades,
/// cards and the suspensions they bring are the replica's own, held to
/// what the saves read. The fitness its men keep follows the graded
/// pass ([`crate::sim::competition_wear`]).
fn mark_match(
    records: &mut Records,
    nation: u16,
    level: u8,
    md: usize,
    home: u16,
    away: u16,
    full: u8,
    a_full: u8,
    individual: &IndividualTables,
    random: &mut Random,
) -> [Vec<u16>; 2] {
    let mut scorers: [Vec<u16>; 2] = Default::default();
    for (team, goals, at_home) in [(home, full, true), (away, a_full, false)] {
        let squad = records
            .team(nation, team)
            .map(|t| t.players())
            .unwrap_or_default();
        let eleven = eleven(&squad, records);
        let weights: Vec<u32> = eleven
            .iter()
            .map(|&id| {
                records.player(id).map_or(0, |p| {
                    u32::from(p.strength()) * KIND_WEIGHTS[usize::from(p.positions()[0]).min(10)]
                })
            })
            .collect();
        let mut scored = vec![0u8; eleven.len()];
        for _ in 0..u32::from(goals) {
            let Some(k) = draw(&weights, random) else {
                break;
            };
            scored[k] = scored[k].saturating_add(1);
            let id = eleven[k];
            scorers[usize::from(!at_home)].push(id);
            // Every goal teaches its scorer (`FUN_00740aad`); the
            // replica's draw names no kind, so only Torinstinkt.
            crate::training_day::goal_lessons(records, individual, (nation, team), id, -1);
            let names = records.player(id).map(|p| (p.first_name(), p.last_name()));
            if let Some((first, last)) = names {
                statistics::book_goal(
                    records,
                    nation,
                    level,
                    id,
                    first,
                    last,
                    u8::try_from(team).unwrap_or(0),
                );
            }
            // The league's first scorer of the season stands for the
            // Toto's scorer tip, once and in the low leagues only
            // (`FUN_00692045` writes the field at +0x1a04).
            if level < 6 {
                if let Some(name) = League::block_name(records, nation, level)
                    && let Some(b) = records.block_mut(&name)
                {
                    let at = crate::opponent::LEAGUE_FIELDS + 0x1a04;
                    if b.get(at..at + 2).is_some_and(|f| f == [0, 0]) {
                        b[at..at + 2].copy_from_slice(&id.to_le_bytes());
                    }
                }
            }
        }
        // The side's substitutions: one to three of the strongest waiting
        // men replace the weakest field men of the eleven. The original
        // substitutes at its manager's word; the replica takes the
        // strongest who wait.
        let strength = |id: u16| u32::from(records.player(id).map_or(0, |p| p.strength()));
        let mut bench: Vec<u16> = squad
            .iter()
            .copied()
            .filter(|&id| !eleven.contains(&id) && available(records, id))
            .collect();
        bench.sort_by_key(|&id| std::cmp::Reverse(strength(id)));
        let mut field: Vec<u16> = eleven
            .iter()
            .copied()
            .filter(|&id| {
                records.player(id).is_some_and(|p| p.positions()[0] != world::GOALKEEPER)
            })
            .collect();
        field.sort_by_key(|&id| strength(id));
        let subs = (1 + usize::try_from(random.below(3)).unwrap_or(1))
            .min(field.len())
            .min(bench.len());
        let outs: Vec<u16> = field.iter().copied().take(subs).collect();
        let ins: Vec<u16> = bench.iter().copied().take(subs).collect();
        // The matchday row of every man on the pitch (R+0x319): bit 6 he
        // started, bit 7 a substitution touched him — off for a starter,
        // on for one coming in —, bits 2 to 5 his grade in half steps,
        // the low two his card: 1 yellow, 2 yellow-red, 3 red. His goals
        // of the matchday stand in the high nibble of R+0x2eb, his
        // coming-ons counted at +0x254, his grade of the last match at
        // +0x273. The original writes these from its match engine; the
        // grades (four to eleven half steps about the mean, as the saves
        // read), the cards — a yellow to about a ninth of the men, the
        // reds and yellow-reds far rarer — and the substitutions are the
        // replica's own. A red or yellow-red suspends, and so does every
        // rule's worth of yellows; the suspension counts the matchday
        // just played among its matches, which settle serves off at the
        // day's end.
        let appearances: Vec<(u16, u8, bool, bool)> = eleven
            .iter()
            .zip(&scored)
            .map(|(&id, &n)| (id, n, true, outs.contains(&id)))
            .chain(ins.iter().map(|&id| (id, 0, false, false)))
            .collect();
        for (id, n, starter, went_off) in appearances {
            let grade = (5 + u32::from(random.below(5)) + u32::from(n)).min(11) as u8;
            let card = if random.below(110) == 0 {
                3
            } else if random.below(340) == 0 {
                2
            } else if random.below(9) == 0 {
                1
            } else {
                0
            };
            // Whether this yellow is the country's rule's last: drawn over
            // the cards as they would stand with it (`cards::league`).
            let suspends_yellow = card == 1
                && records.player(id).is_some_and(|p| {
                    let mut q = p.clone();
                    let mut counted = q.league_cards();
                    counted[0] = counted[0].saturating_add(1);
                    q.set_league_cards(counted);
                    cards::league(records, nation, level, &q)
                });
            if let Some(p) = records.player_mut(id) {
                p.set_league_matches(p.league_matches().saturating_add(1));
                p.set_matches_in_a_row(p.matches_in_a_row().saturating_add(1));
                p.set_last_goals(n);
                p.set_grade(grade);
                if !starter {
                    let mut less = p.league_less();
                    less[0] = less[0].saturating_add(1);
                    p.set_league_less(less);
                }
                if n > 0 {
                    let mut goals = p.goals();
                    goals[0] = goals[0].saturating_add(n);
                    p.set_goals(goals);
                    if at_home {
                        p.set_home_goals(p.home_goals().saturating_add(n));
                    }
                }
                if md < p.matchdays().len() {
                    let mut bytes = p.matchdays();
                    let mut nibbles = p.matchday_goals();
                    bytes[md] =
                        (if starter { 0x40 } else { 0x80 }) | (grade << 2) | card;
                    if went_off {
                        bytes[md] |= 0x80;
                    }
                    nibbles[md] = n << 4;
                    p.set_matchdays(bytes);
                    p.set_matchday_goals(nibbles);
                }
                if card != 0 {
                    let mut cards = p.league_cards();
                    match card {
                        1 => cards[0] = cards[0].saturating_add(1),
                        2 => cards[2] = cards[2].saturating_add(1),
                        _ => cards[1] = cards[1].saturating_add(1),
                    }
                    p.set_league_cards(cards);
                    if card >= 2 || suspends_yellow {
                        let old = p.suspension();
                        let low = ((u32::from(old & 0x3f) + 2).min(63)) as u8;
                        p.set_suspension((old & 0xc0) | low);
                    }
                }
            }
        }
        // What the match leaves on the squad's fitness (`FUN_006f86e1`,
        // [`crate::sim::competition_wear`]): the eleven stand the ninety
        // minutes, the men the replica's substitutions touch split them at
        // [`SUB_MINUTE`], and the rest lose a condition step.
        let mut minutes: Vec<(u16, i32)> =
            eleven.iter().map(|&id| (id, if outs.contains(&id) { SUB_MINUTE } else { 90 })).collect();
        minutes.extend(ins.iter().map(|&id| (id, 90 - SUB_MINUTE)));
        crate::sim::competition_wear(records, (u8::try_from(nation).unwrap_or(0), team), &minutes, false);
        // The men who stood leave the youth round (`FUN_007471bd`),
        // in the squad's order as the graded pass walks them.
        for &id in squad.iter().filter(|id| eleven.contains(id) || ins.contains(id)) {
            crate::training_day::leave_youth_round(records, (nation, team), id);
        }
        for &id in squad
            .iter()
            .filter(|&id| !eleven.contains(&id) && !ins.contains(&id))
        {
            if let Some(p) = records.player_mut(id) {
                p.set_matches_in_a_row(0);
                // The man who sat the matchday out keeps a nought row —
                // the saves read no leftovers of older seasons in it.
                if md < p.matchdays().len() {
                    let mut bytes = p.matchdays();
                    bytes[md] = 0;
                    p.set_matchdays(bytes);
                }
                if md < p.matchday_goals().len() {
                    let mut nibbles = p.matchday_goals();
                    nibbles[md] = 0;
                    p.set_matchday_goals(nibbles);
                }
            }
        }
    }
    scorers
}

/// Whether the player stands fit and free for his club's match: no
/// injury (+0x228), no match still suspended (+0x229) and no known
/// number thereof (bit 7), no weeks of a ban (+0x22a), and not marked
/// suspended for the match (+0x26b bit 4).
fn available(records: &Records, id: u16) -> bool {
    records.player(id).is_some_and(|p| {
        p.injury_kind() == 0
            && p.suspension() & 0x3f == 0
            && p.banned_weeks() == 0
            && p.away_kind() & 0x10 == 0
    })
}

/// The eleven a side puts on the pitch when no formation stands: of the
/// players it lists, those fit and free — no injury (+0x228), no match
/// suspension (+0x229) or weeks of a ban (+0x22a), and not marked
/// suspended for the match (+0x26b bit 4) — the keeper first, the
/// strongest who names him one, and the ten strongest field players
/// beside him. The engine walk reads the manager's saved eleven instead
/// (`sim::saved_eleven`); this stands for the sides whose record lists
/// none, and for the fixture paths (`mark_match`, the cup ties), where
/// the original still picks from its own squads.
pub(crate) fn eleven(squad: &[u16], records: &Records) -> Vec<u16> {
    let strength = |id: u16| u32::from(records.player(id).map_or(0, |p| p.strength()));
    let mut fit: Vec<u16> = squad.iter().copied().filter(|&id| available(records, id)).collect();
    fit.sort_by_key(|&id| std::cmp::Reverse(strength(id)));
    let keeper = fit
        .iter()
        .copied()
        .find(|&id| records.player(id).is_some_and(|p| p.positions()[0] == world::GOALKEEPER));
    let mut eleven: Vec<u16> = keeper.into_iter().collect();
    eleven.extend(fit.iter().copied().filter(|&id| {
        id != keeper.unwrap_or(0)
            && records.player(id).is_some_and(|p| p.positions()[0] != world::GOALKEEPER)
    }));
    eleven.truncate(11);
    eleven
}

/// Which of the eleven scored a goal: drawn over their weights, or of
/// all of them alike when none of them weighs aught.
fn draw(weights: &[u32], random: &mut Random) -> Option<usize> {
    if weights.is_empty() {
        return None;
    }
    let total: u32 = weights.iter().sum();
    let cap = if total == 0 {
        u32::try_from(weights.len()).unwrap_or(1)
    } else {
        total
    };
    let mut draw = random.below(cap.max(1));
    if total == 0 {
        return Some((draw as usize).min(weights.len() - 1));
    }
    for (k, &w) in weights.iter().enumerate() {
        if draw < w {
            return Some(k);
        }
        draw -= w;
    }
    Some(weights.len() - 1)
}

/// How many the club's stadium holds (`0x1a48138[index]` +0x3c).
pub(crate) fn capacity(records: &Records, nation: u16, team: u16) -> i32 {
    records
        .team_index(nation, team)
        .and_then(|i| records.block(&format!("{STADIUM_AT:#x}[{i}]")))
        .map_or(0, |b| {
            b.get(CAPACITY..CAPACITY + 4).map_or(0, |c| i32::from_le_bytes(c.try_into().unwrap()))
        })
}

/// How many come to one of the club's home matches: the plan's attendance
/// with three in ten off to three in ten on, held up by its season-ticket
/// holders and capped by the stadium when it holds places. The original
/// draws its crowd over the plan too (`FUN_0078e5e6`, its clubs' fills a
/// little above and below the plan); the replica's luck rides a tenth to
/// two tenths about it.
fn attendance(team: &Team, capacity: i32, random: &mut Random) -> i32 {
    let plan = team.get::<i32>(PLAN_ATTENDANCE + CLUB_TO_TEAM);
    if plan <= 0 {
        return 0;
    }
    let season = team.get::<i32>(PLAN_SEASON_TICKETS + CLUB_TO_TEAM).max(0);
    let luck = 70 + i32::try_from(random.below(61)).unwrap_or(70);
    let a = plan.wrapping_mul(luck) / 100;
    let a = a.max(season);
    if capacity > 0 { a.min(capacity) } else { a }
}

/// How many goals one side scores past the other: its midfield and attack
/// over their goalkeeper and defenders — in tenths, GOAL_UNITS of the
/// difference being a goal, and the home side playing the stronger by
/// HOME_EDGE — with a luck from nought to nine lifting the count by half of
/// what it is beyond two, and sharing the goals over the halves by its
/// remainder in threes. Calibrated
/// on a save's played matchdays, which bring some two goals to a game.
pub(crate) fn goals(att: [i16; 4], hold: [i16; 4], edge: i32, random: &mut Random) -> (u8, u8) {
    let attack = i32::from(att[2]) + i32::from(att[3]);
    let defence = i32::from(hold[0]) + i32::from(hold[1]);
    let luck = random.below(10);
    let full =
        ((attack - defence + edge) / GOAL_UNITS + (i32::try_from(luck).unwrap_or(0) - 2) / 2)
            .clamp(0, 6) as u8;
    let half = (u32::from(full) * (luck % 3) / 2) as u8;
    (full, half)
}

#[cfg(test)]
mod supercup_tests {
    use super::*;

    /// The supercups played over their days from the original's new
    /// season (NewSeason000, the supercups set up): Germany's tournament
    /// moves each winner on to the waiting seed and into the final, every
    /// tie decided; England's one final decided on its day; Italy's two
    /// legs stand open after the first and decided after the second; the
    /// human's calendar takes his club's scores, and the final's winner's
    /// trainer the title.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_supercups_are_played_on_their_days() {
        use a3_files::an3::Save;
        use crate::supercup::{FINAL_TIE, OBJECT, SIX, TIE, TIES};
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let mut records = Records::from_save(Save::read(&keeps.join("NewSeason000.an3")).unwrap()).unwrap();
        let data = crate::data::Data::load(&dir).unwrap();
        crate::year::open_season(&mut records, &data.calendar);
        let mut random = Random::new(8);
        let object = |r: &Records, country: u16| -> Vec<u8> {
            let slot = r.countries.slot(usize::from(country));
            r.block(&format!("{OBJECT:#x}[{slot}]")).unwrap().to_vec()
        };
        let six = object(&records, 14);
        let titles: Vec<i16> = records.persons.iter().map(|p| p.get::<i16>(0x42)).collect();
        for day in [33, 34] {
            for country in [14u16, 16, 28] {
                supercup_day(&mut records, country, &data.calendar, day, &mut random);
            }
        }
        let b = object(&records, 14);
        for r in 0..2 {
            let (tie, next) = (TIES + r * TIE, TIES + (r + 2) * TIE);
            assert_eq!(b[tie + 0xc] & (CUP_RESULT | CUP_PLAYED), CUP_RESULT | CUP_PLAYED, "tie {r} decided");
            let w = usize::from(b[tie + 0xd]) * 4;
            assert_eq!(b[next..next + 4], b[tie + w..tie + w + 4], "tie {r}'s winner moves on");
            assert_eq!(b[next + 6..next + 8], six[SIX + 2 * r..SIX + 2 * r + 2], "against the seed {r}");
        }
        for day in [40, 41, 45] {
            for country in [14u16, 16, 28] {
                supercup_day(&mut records, country, &data.calendar, day, &mut random);
            }
        }
        let b = object(&records, 14);
        for r in 2..4 {
            let tie = TIES + r * TIE;
            assert_ne!(b[tie + 0xc] & CUP_PLAYED, 0, "tie {r} decided");
            let w = usize::from(b[tie + 0xd]) * 4;
            let side = FINAL_TIE + (r - 2) * 4;
            assert_eq!(b[side..side + 4], b[tie + w..tie + w + 4], "tie {r}'s winner in the final");
        }
        assert_ne!(b[FINAL_TIE + 0xc] & CUP_PLAYED, 0, "Germany's final decided");
        // Its winner's trainer counts the title.
        let w = FINAL_TIE + usize::from(b[FINAL_TIE + 0xd]) * 4;
        let winner = records.team(u16::from(b[w]), u16::from_le_bytes([b[w + 2], b[w + 3]])).unwrap();
        if winner.humans()[1] == 0 {
            let [mt, tr, _] = winner.staff();
            let id = usize::from(if tr != 0 { tr } else { mt });
            assert_eq!(records.persons[id].get::<i16>(0x42), titles[id] + 1, "the winner's trainer {id}");
        }
        // The final heads the list of finals, the year before it.
        let finals = statistics::league_cup_finals(&records, 14);
        assert_eq!(finals.first().map(|f| f.0), Some(records.globals.start_year() as i16));
        let lists = records.block(FINALS_BLOCK).unwrap();
        let slot = usize::from(records.countries.slot(14));
        let last = FINALS_AT + slot * FINALS_PER_SLOT + FINALS_PER_SLOT - FINAL_ENTRY;
        assert_eq!(lists[last + 2..last + FINAL_ENTRY], b[FINAL_TIE..FINAL_TIE + FINAL_ENTRY - 2]);
        let england = object(&records, 16);
        assert_ne!(england[FINAL_TIE + 0xc] & CUP_PLAYED, 0, "England's final decided");
        let italy = object(&records, 28);
        assert_eq!(italy[FINAL_TIE + 0xc] & (CUP_RESULT | CUP_PLAYED), CUP_RESULT, "Italy's first leg open");
        supercup_day(&mut records, 28, &data.calendar, 55, &mut random);
        let italy = object(&records, 28);
        assert_ne!(italy[FINAL_TIE + 0xc] & CUP_PLAYED, 0, "Italy's second leg decides");
        // Union's days: every supercup day it played holds a score.
        let cal = &records.calendars[0];
        let played: Vec<usize> = (0..60)
            .filter(|&d| {
                let e = cal.entry(d);
                e[0] == YEAR_MATCH && u32::from_le_bytes([e[4], e[5], e[6], e[7]]) & 0x7f00_0000 == SUPERCUP_CODE
            })
            .collect();
        assert!(!played.is_empty());
        assert!(played.iter().all(|&d| cal.entry(d)[YEAR_FLAGS] & YEAR_RESULT != 0), "{played:?}");
    }

    /// England's league cup played over its days from the original's
    /// NewSeason000 (its first round drawn there): every round's ties
    /// decided, two legs where the calendar plays two, the 25 winners of
    /// the first round and the seven waiting met in the second's 16 ties,
    /// on to one final with its winner, the last drawn round following —
    /// and no career title in it: `FUN_0067e911` pays a cup kind its money
    /// and awards its kinds 0xb and 0xc only in the final of a tournament
    /// (kind 0xe), a national team's world's and European title.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_league_cup_is_played_on_its_days() {
        use a3_files::an3::Save;
        use crate::cup_draw::LEAGUE_CUP;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let mut records = Records::from_save(Save::read(&keeps.join("NewSeason000.an3")).unwrap()).unwrap();
        let data = crate::data::Data::load(&dir).unwrap();
        let mut random = Random::new(9);
        let titles: Vec<[i16; 5]> = records
            .persons
            .iter()
            .map(|p| [0x3c, 0x3e, 0x40, 0x42, 0x44].map(|at| p.get::<i16>(at)))
            .collect();
        let row = data.calendar.row(ENGLAND, 0).unwrap();
        let days: Vec<usize> = (0..364).filter(|&d| data.calendar.entry(row, d) & 0x7f00_0000 == LEAGUE_CUP_CODE).collect();
        assert!(days.len() >= 8, "{days:?}");
        let booked = |records: &Records| -> i64 {
            records.teams.iter().filter(|t| u16::from(t.country()) == ENGLAND).map(|t| i64::from(t.get::<i32>(BOOKED_CUP + CLUB_TO_TEAM))).sum()
        };
        let before = booked(&records);
        for &d in &days {
            let was = booked(&records);
            league_cup_day(&mut records, ENGLAND, &data.calendar, d, &mut random);
            if data.calendar.entry(row, d) & 0x3f != 0 {
                assert!(booked(&records) > was, "day {d} books its gates on the cup takings");
            }
        }
        assert!(booked(&records) > before);
        let b = records.block(LEAGUE_CUP).unwrap();
        let mode = usize::from(u16::from_le_bytes([b[4], b[5]]));
        for round in 1..=6 {
            let Some((start, count)) = cup_pairings(mode, round) else { continue };
            for q in start..start + count {
                let p = &b[PAIRINGS + q * PAIRING..PAIRINGS + (q + 1) * PAIRING];
                assert!(p[0] != 0 && p[4] != 0, "round {round} tie {q} drawn");
                assert_ne!(p[0xc] & CUP_PLAYED, 0, "round {round} tie {q} decided");
            }
        }
        assert_eq!(cup_pairings(mode, 2).map(|r| r.1), Some(16));
        assert_eq!(b[0x28 - 0x20], 6, "the final the last drawn round");
        // No career title in it. The final stands decided; its winner's
        // trainer counts nothing, no person's counters of titles move over
        // the whole cup.
        let (start, _) = cup_pairings(mode, 6).unwrap();
        let p = &b[PAIRINGS + start * PAIRING..PAIRINGS + (start + 1) * PAIRING];
        assert_ne!(p[0xd], 0, "the final's winner");
        for (was, pl) in titles.iter().zip(&records.persons) {
            let now = [0x3c, 0x3e, 0x40, 0x42, 0x44].map(|at| pl.get::<i16>(at));
            assert_eq!(now, *was, "the cup of this kind credits no career");
        }
    }
}

#[cfg(test)]
mod crowd_tests {
    use super::*;

    /// England's league cup crowds from A's day on rank in the international list alone:
    /// rows of English home grounds join it over the season's ties, the
    /// list stays descending, and England's league lists never move, as
    /// the family gate lets only the league's matches reach those.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn league_cup_crowds_rank_internationally_only() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let mut records = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let data = crate::data::Data::load(&dir).unwrap();
        let mut random = Random::new(9);
        let row = data.calendar.row(ENGLAND, 0).unwrap();
        let leagues: Vec<Vec<statistics::Crowd>> = (0..4).map(|l| statistics::crowds(&records, ENGLAND, l)).collect();
        // An emptied international list, so every crowd ranks in.
        records.block_mut("0xaf723c").unwrap().fill(0);
        let today = usize::from(records.globals.day());
        for d in (today..364).filter(|&d| data.calendar.entry(row, d) & 0x7f00_0000 == LEAGUE_CUP_CODE) {
            league_cup_day(&mut records, ENGLAND, &data.calendar, d, &mut random);
        }
        let after = statistics::international_crowds(&records);
        let atts: Vec<u32> = after.iter().map(|c| c.attendance).collect();
        let mut sorted = atts.clone();
        sorted.sort_by(|a, b| b.cmp(a));
        assert_eq!(atts, sorted, "the list stands descending");
        assert!(
            after.iter().all(|c| u16::from(c.home.0) == ENGLAND && c.attendance > 0),
            "the league cup's crowds fill it: {after:?}"
        );
        for (l, was) in leagues.iter().enumerate() {
            assert_eq!(&statistics::crowds(&records, ENGLAND, l as u8), was, "league {l} untouched");
        }
    }

    /// One league matchday's gates rank into both biggest-attendance
    /// lists: every gate better than the international list's weakest
    /// stands in it afterwards, the list keeps its descending order, and
    /// the league's own list carries the day's rows as well (the cup's
    /// do not reach it, per the family gate).
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_matchdays_gates_rank_in_the_crowd_lists() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let mut records = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let league = League::of(&records, 14, 0).expect("Germany's first league");
        let homes: Vec<u16> = league.open_games().iter().map(|&(h, _, _)| league.team(h) as u16).collect();
        // Where each home club's next gate will stand among its gates.
        let held: Vec<(u16, usize)> = homes
            .iter()
            .map(|&t| (t, usize::from(records.team(14, t).unwrap().home_games())))
            .collect();
        let before = statistics::international_crowds(&records);
        let weakest = before.iter().rev().find(|c| c.attendance != 0).map_or(0, |c| c.attendance);
        let mut random = Random::new(7);
        let day = usize::from(records.globals.day());
        let exe = a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap();
        let tables = IndividualTables::load(&exe).unwrap();
        settle(&mut records, 14, 0, day, &tables, &[], &mut random, &mut Vec::new());
        let gates: Vec<i32> = held
            .iter()
            .map(|&(t, at)| records.team(14, t).unwrap().gates()[at])
            .collect();
        assert!(gates.iter().all(|&g| g > 0), "every home club booked a gate");
        let after = statistics::international_crowds(&records);
        let atts: Vec<u32> = after.iter().map(|c| c.attendance).collect();
        let mut sorted = atts.clone();
        sorted.sort_by(|a, b| b.cmp(a));
        assert_eq!(atts, sorted, "the list stands descending");
        for gate in gates.iter().filter(|&&g| u32::try_from(g).unwrap() > u32::try_from(weakest).unwrap()) {
            assert!(atts.contains(&(*gate as u32)), "the gate {gate} ranks in");
        }
        assert_eq!(
            after[0].attendance,
            before[0].attendance.max(gates.iter().copied().max().unwrap() as u32),
            "the top row is the best of the old list and the day"
        );
        let mine = statistics::crowds(&records, 14, 0);
        for gate in &gates {
            assert!(
                mine.iter().any(|c| i32::try_from(c.attendance).unwrap_or(0) == *gate
                    && c.home.1 >= 0 && homes.contains(&(c.home.1 as u16)))
                    || u32::try_from(*gate).unwrap() <= u32::try_from(weakest).unwrap(),
                "the league's list carries the day's gate {gate}"
            );
        }
    }
}
#[cfg(test)]
mod two_legs_tests {
    use super::*;

    /// Italy's cup over its first round's two days from the original's
    /// new season: the first leg leaves every tie standing open, the
    /// second, played at the other ground, decides each on the sum of
    /// the legs and then the away goals (`FUN_00677ed4`), and the winners
    /// are drawn into the next round.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_two_legged_cup_round_decides_on_the_sum() {
        use a3_files::an3::Save;
        const ITALY: u16 = 28;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let mut records = Records::from_save(Save::read(&keeps.join("NewSeason000.an3")).unwrap()).unwrap();
        let data = crate::data::Data::load(&dir).unwrap();
        let mut random = Random::new(11);
        let row = data.calendar.row(ITALY, 0).unwrap();
        let days: Vec<usize> = (0..364)
            .filter(|&d| data.calendar.entry(row, d) & 0x7f00_00ff == CUP_CODE | 1)
            .collect();
        assert_eq!(days.len(), 2, "{days:?}");
        assert_ne!(data.calendar.entry(row, days[1]) & CUP_SECOND, 0);
        let slot = records.countries.slot(usize::from(ITALY));
        let name = format!("{CUPS_AT:#x}[{slot}]");
        let mode = usize::from(records.block(&name).unwrap()[0x24 - 0x20]).min(5);
        let (start, count) = cup_pairings(mode, 1).unwrap();
        let ties = |r: &Records| -> Vec<Vec<u8>> {
            let b = r.block(&name).unwrap();
            (start..start + count).map(|q| b[PAIRINGS + q * PAIRING..PAIRINGS + (q + 1) * PAIRING].to_vec()).filter(|p| p[0] != 0).collect()
        };
        let drawn = ties(&records);
        assert!(!drawn.is_empty(), "the first round drawn");
        cup_day(&mut records, ITALY, &data.calendar, days[0], &data.rules.individual, &mut random, &mut Vec::new());
        for p in ties(&records) {
            assert_eq!(p[0xc] & (CUP_RESULT | CUP_PLAYED | CUP_SECOND_PLAYED), CUP_RESULT, "the first leg only stands");
        }
        cup_day(&mut records, ITALY, &data.calendar, days[1], &data.rules.individual, &mut random, &mut Vec::new());
        for p in ties(&records) {
            assert_eq!(p[0xc] & 0x38, 0x38, "the second leg decides");
            if p[0xc] & CUP_EXTRA != 0 {
                continue;
            }
            let (a, b) = (u16::from(p[8]) + u16::from(p[0xa]), u16::from(p[9]) + u16::from(p[0xb]));
            // Level on the sum: the away goals, B's in the first leg
            // against A's in the second.
            let b_wins = if a != b { b > a } else { p[9] > p[0xa] };
            assert_eq!(p[0xd], u8::from(b_wins), "{p:?}");
        }
        let b = records.block(&name).unwrap();
        assert_eq!(b[0x28 - 0x20], 2, "the second round drawn");
    }
}

#[cfg(test)]
mod listing_tests {
    use super::*;

    /// The week's end on A looks at day 69, its last day: eight leagues
    /// play then — the probe's eight draws of 9 or 10 that the traced
    /// original made at the load, before the turn's screen — each drawing
    /// its doping pair out of its teams' half, nothing else drawn; the
    /// turn's own day 63 lists no league match.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_week_end_lists_its_last_days_matches() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let save = dir.parent().unwrap().join("saves-kept").join("A.an3");
        let records = Records::from_save(Save::read(&save).unwrap()).unwrap();
        let data = crate::data::Data::load(&dir).unwrap();
        let mut random = Random::from_state(12345, 67890);
        assert!(list_day(&records, &data.calendar, 63, &mut random).is_empty());
        let picks = list_day(&records, &data.calendar, 69, &mut random);
        let ranges: Vec<u32> = picks
            .iter()
            .map(|&((country, level), _)| (League::of(&records, country, level).unwrap().teams() / 2) as u32)
            .collect();
        assert_eq!(ranges.len(), 8);
        assert!(ranges.iter().all(|&n| n == 9 || n == 10), "{ranges:?}");
        let mut replay = Random::from_state(12345, 67890);
        for ((_, drawn), &n) in picks.iter().zip(&ranges) {
            assert_eq!(drawn, &vec![replay.below(n)]);
        }
        assert_eq!(random, replay);
    }

    /// The doping control takes the pair the day's list marked: on A's
    /// human league, marking the human club's own pair tests his men
    /// (dialog 41's control), marking the next pair leaves him alone, and
    /// a league off the list draws no control at all.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_control_takes_the_lists_pair() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let save = dir.parent().unwrap().join("saves-kept").join("A.an3");
        let load = || Records::from_save(Save::read(&save).unwrap()).unwrap();
        let records = load();
        let data = crate::data::Data::load(&dir).unwrap();
        let (country, number) = statistics::human_clubs(&records)[0];
        let nation = u16::from(country);
        let level = records.team(nation, number as u16).unwrap().level();
        let league = League::of(&records, nation, level).unwrap();
        let games = league.open_games();
        let own = games
            .iter()
            .position(|&(h, a, _)| [h, a].iter().any(|&p| league.team(p) == number))
            .unwrap();
        let controlled = |picks: &[u32]| {
            let mut records = load();
            let mut controls = Vec::new();
            let mut random = Random::from_state(4711, 815);
            settle(&mut records, nation, level, 69, &data.rules.individual, picks, &mut random, &mut controls);
            controls.iter().map(|c| c.club).collect::<Vec<_>>()
        };
        assert_eq!(controlled(&[own as u32]), [(country, number)]);
        assert!(controlled(&[((own + 1) % games.len()) as u32]).is_empty());
        assert!(controlled(&[]).is_empty());
    }
}
