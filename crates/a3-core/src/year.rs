//! A human club's year calendar (JAHRESKALENDER.md in the RE repo): an
//! entry for every day of the season — training camps, tournaments,
//! friendlies, matches, parties and events — which the calendar screen
//! shows a month at a time, books by dragging, and changes by a click.

use std::io;

use a3_files::exe::Exe;

use crate::records::{PLANNER_DAYS, PLANNER_ENTRY, Planner, Records};

/// The kinds of days.
pub const CAMP: u8 = 1;
pub const TOURNAMENT: u8 = 2;
pub const INDOOR_TOURNAMENT: u8 = 3;
pub const BEACH_TOURNAMENT: u8 = 4;
pub const FRIENDLY: u8 = 5;
pub const MATCH: u8 = 6;
pub const PARTY: u8 = 7;
pub const EVENT: u8 = 8;
/// The last day anything may be booked on.
pub const LAST_BOOKING: usize = 336;

/// A match entry's fields: its competition code, both teams (country and
/// number), the goals, whether the result is known.
const MATCH_CODE: usize = 4;
const MATCH_HOME: usize = 8;
const MATCH_AWAY: usize = 0xc;
const MATCH_GOALS: usize = 0x10;
const MATCH_FLAGS: usize = 0x14;
const RESULT_KNOWN: u8 = 0x20;
/// A camp marker's +4: 1 its first day, 0 its last; +6 its place.
const CAMP_START: usize = 4;
const CAMP_PLACE: usize = 6;

const KINDS_AT: u32 = 0x8a8140;
const KIND_SIZE: usize = 5;
const KINDS: usize = 12;
/// A kind's columns in the table: shown, days needed, allowed in a camp,
/// deleted by a click.
const SHOWN: usize = 0;
const DAYS_NEEDED: usize = 2;
const IN_CAMP: usize = 3;
const DELETABLE: usize = 4;
const EVENTS_AT: u32 = 0x882b4c;
const EVENTS: usize = 7;
const PARTIES_AT: u32 = 0x882b5c;
const PARTIES: usize = 6;
/// The camps' places: 26 records of 0x38 bytes ([`Place`]).
const PLACES_AT: u32 = 0x8ab318;
const PLACES: u32 = 26;
const PLACE_SIZE: u32 = 0x38;
/// The places' pictures and balls, by the records' indices (a u16 every
/// four bytes).
const PLACE_PICTURES_AT: u32 = 0x88b72c;
const PLACE_PICTURES: usize = 21;
const BALL_PICTURES_AT: u32 = 0x88b780;
const BALL_PICTURES: usize = 5;
/// A group of places with one map point for all (`FUN_005cd5b7`): the
/// sub-choice offers at most this many of the others.
const GROUP_OFFERS: usize = 3;

/// A training camp's place (`DAT_008ab318`): its picture's and its ball's
/// indices, what is drunk there (0 vodka, 1 water, 2 to 4 that many
/// glasses of sparkling wine less one), the cost, its point on the world
/// map (x below 0 none), the season it opens after (below 1 always), set
/// when only the lottery's lucky season shows it, the control that stands
/// for it, its group (below 1 none), and its name's text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Place {
    pub picture: u8,
    pub drink: u8,
    pub ball: u8,
    pub cost: i32,
    pub at: (i16, i16),
    pub first_year: i16,
    pub lucky: bool,
    pub control: i16,
    pub group: i16,
    pub name: u16,
}

/// A day of the calendar, as stored.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Day([u8; PLANNER_ENTRY]);

impl Day {
    pub fn kind(&self) -> u8 {
        self.0[0]
    }

    fn word(&self, at: usize) -> i16 {
        i16::from_le_bytes([self.0[at], self.0[at + 1]])
    }

    /// A party's or an event's value, a camp marker's start flag.
    pub fn value(&self) -> i16 {
        self.word(4)
    }

    /// A camp's place, from 1 (0 not known).
    pub fn place(&self) -> i16 {
        self.word(CAMP_PLACE)
    }

    pub fn camp_start(&self) -> bool {
        self.kind() == CAMP && self.word(CAMP_START) != 0
    }

    /// A tournament's or friendly's club: country and number.
    pub fn club(&self) -> (u8, i16) {
        (self.0[4], self.word(6))
    }

    /// A match's competition code, teams, and result if known.
    pub fn match_code(&self) -> u32 {
        u32::from_le_bytes(self.0[MATCH_CODE..MATCH_CODE + 4].try_into().unwrap_or_default())
    }

    pub fn teams(&self) -> [(u8, i16); 2] {
        [
            (self.0[MATCH_HOME], self.word(MATCH_HOME + 2)),
            (self.0[MATCH_AWAY], self.word(MATCH_AWAY + 2)),
        ]
    }

    pub fn result(&self) -> Option<(u8, u8)> {
        (self.0[MATCH_FLAGS] & RESULT_KNOWN != 0)
            .then_some((self.0[MATCH_GOALS], self.0[MATCH_GOALS + 1]))
    }
}

/// The game's tables of the calendar.
pub struct YearTables {
    kinds: Vec<u8>,
    /// The events' and parties' texts, by their values.
    pub event_texts: Vec<u16>,
    pub party_texts: Vec<u16>,
    /// The camps' places' names, from place 1.
    pub place_texts: Vec<u16>,
    /// The camps' places, and the pictures of their places and balls.
    pub places: Vec<Place>,
    pub place_pictures: Vec<u16>,
    pub ball_pictures: Vec<u16>,
}

impl YearTables {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        let places: Vec<Place> = (0..PLACES)
            .map(|p| {
                let b = exe.bytes(PLACES_AT + p * PLACE_SIZE, PLACE_SIZE as usize)?;
                let word = |at: usize| i16::from_le_bytes([b[at], b[at + 1]]);
                Ok(Place {
                    picture: b[0],
                    drink: b[1],
                    ball: b[2],
                    cost: i32::from_le_bytes([b[4], b[5], b[6], b[7]]),
                    at: (word(8), word(0xa)),
                    first_year: word(0xc),
                    lucky: word(0xe) >= 1,
                    control: word(0x10),
                    group: word(0x12),
                    name: word(0x36) as u16,
                })
            })
            .collect::<io::Result<_>>()?;
        Ok(Self {
            kinds: exe.bytes(KINDS_AT, KINDS * KIND_SIZE)?.to_vec(),
            event_texts: exe.u16s(EVENTS_AT, EVENTS)?,
            party_texts: exe.u16s(PARTIES_AT, PARTIES)?,
            place_texts: places.iter().map(|p| p.name).collect(),
            places,
            place_pictures: every_fourth(exe, PLACE_PICTURES_AT, PLACE_PICTURES)?,
            ball_pictures: every_fourth(exe, BALL_PICTURES_AT, BALL_PICTURES)?,
        })
    }

    /// Whether a place may be chosen this season (`FUN_005ccf5a`): it opens
    /// after the season's year, and a lucky one only while the lottery
    /// (`DAT_00b5d174`) stands at 0.
    pub fn open(&self, place: usize, year: i32, lottery: u16) -> bool {
        self.places.get(place).is_some_and(|p| {
            (p.first_year < 1 || i32::from(p.first_year) < year) && (!p.lucky || lottery == 0)
        })
    }

    /// Whether a place's first season has come: the sub-choice tells the
    /// others "Noch nicht möglich!" (`FUN_005cd5b7`).
    pub fn begun(&self, place: usize, year: i32) -> bool {
        self.places.get(place).is_some_and(|p| p.first_year < 1 || i32::from(p.first_year) < year)
    }

    /// The other places of a place's group, at most three, in their order;
    /// none for a place without one.
    pub fn group_of(&self, place: usize) -> Vec<usize> {
        let Some(group) = self.places.get(place).map(|p| p.group).filter(|&g| g >= 1) else {
            return Vec::new();
        };
        (0..self.places.len())
            .filter(|&k| k != place && self.places[k].group == group)
            .take(GROUP_OFFERS)
            .collect()
    }

    fn of(&self, kind: u8, column: usize) -> u8 {
        self.kinds.get(usize::from(kind) * KIND_SIZE + column).copied().unwrap_or(0)
    }

    /// Whether a day's kind shows in the calendar (`FUN_0072ef9e`).
    pub fn shown(&self, kind: u8) -> bool {
        self.of(kind, SHOWN) != 0
    }

    /// Whether a click deletes it: parties and events (`FUN_0072daf8`).
    pub fn deletable(&self, kind: u8) -> bool {
        self.of(kind, DELETABLE) != 0
    }

    /// Whether the days a kind needs from a day on are all free
    /// (`FUN_0072e917`).
    pub fn free(&self, calendar: &Planner, day: usize, kind: u8) -> bool {
        let days = usize::from(self.of(kind, DAYS_NEEDED));
        (day..day + days).all(|d| d < PLANNER_DAYS && calendar.day(d).kind == 0)
    }

    /// Whether a kind may be booked during a camp: friendlies only
    /// (`FUN_0072f552`).
    pub fn allowed_in_camp(&self, kind: u8) -> bool {
        self.of(kind, IN_CAMP) != 0
    }
}

pub fn day(calendar: &Planner, day: usize) -> Day {
    Day(calendar.entry(day))
}

/// Whether a day lies in a training camp: a camp marker, or after the
/// last one marked as starting one (`FUN_0072f096`).
pub fn in_camp(calendar: &Planner, d: usize) -> bool {
    if day(calendar, d).kind() == CAMP {
        return true;
    }
    (0..d)
        .rev()
        .find(|&e| day(calendar, e).kind() == CAMP)
        .is_some_and(|e| day(calendar, e).camp_start())
}

/// A party or an event booked on a day, what else its entry held cleared
/// (`FUN_0072ede4`).
pub fn book(calendar: &mut Planner, d: usize, kind: u8, value: i16) {
    let mut entry = [0; PLANNER_ENTRY];
    entry[0] = kind;
    entry[1] = 1;
    entry[4..6].copy_from_slice(&value.to_le_bytes());
    calendar.set_entry(d, entry);
}

/// A camp booked from a day: its first day with the place, from 1, and
/// the next as its last (`FUN_0072e973`, `FUN_0072f30d`); a click in the
/// calendar's camp column moves the end.
pub fn book_camp(calendar: &mut Planner, d: usize, place: i16) {
    for (day, start) in [(d, 1i16), (d + 1, 0)] {
        let mut entry = [0; PLANNER_ENTRY];
        entry[0] = CAMP;
        entry[1] = 1;
        entry[CAMP_START..CAMP_START + 2].copy_from_slice(&start.to_le_bytes());
        if start != 0 {
            entry[CAMP_PLACE..CAMP_PLACE + 2].copy_from_slice(&place.to_le_bytes());
        }
        calendar.set_entry(day, entry);
    }
}

/// A day's entry gone (`FUN_0072dc32`).
pub fn clear(calendar: &mut Planner, d: usize) {
    calendar.set_entry(d, [0; PLANNER_ENTRY]);
}

/// The last camp's end moved to a day after today, if that may be: there
/// is a camp starting before it, not over before today, and nothing but
/// it from its start to the day (`FUN_0072f235`, `FUN_0072f191`,
/// `FUN_0072f39a`). Whether anything moved.
pub fn move_camp_end(calendar: &mut Planner, d: usize, today: usize) -> bool {
    if d <= today || d >= PLANNER_DAYS {
        return false;
    }
    let Some(start) = (0..d).rev().find(|&e| day(calendar, e).camp_start()) else {
        return false;
    };
    let over = start < today
        && (start + 1..today).any(|e| {
            let entry = day(calendar, e);
            entry.kind() == CAMP && !entry.camp_start()
        });
    if over {
        return false;
    }
    if (start..=d).any(|e| !matches!(day(calendar, e).kind(), 0 | CAMP)) {
        return false;
    }
    if day(calendar, d).kind() == CAMP {
        return false;
    }
    let end = |e: usize, calendar: &Planner| {
        let entry = day(calendar, e);
        entry.kind() == CAMP && !entry.camp_start()
    };
    if in_camp(calendar, d) {
        // Inside the camp: the end comes forward to the day.
        let Some(old) = (d + 1..PLANNER_DAYS).find(|&e| day(calendar, e).kind() == CAMP) else {
            return false;
        };
        if !end(old, calendar) {
            return false;
        }
        let mut entry = calendar.entry(d);
        clear(calendar, old);
        entry[0] = CAMP;
        calendar.set_entry(d, entry);
    } else {
        // After it: the end goes on to the day.
        let Some(old) = (1..d).rev().find(|&e| day(calendar, e).kind() == CAMP) else {
            return false;
        };
        if !end(old, calendar) {
            return false;
        }
        clear(calendar, old);
        let mut entry = [0; PLANNER_ENTRY];
        entry[0] = CAMP;
        calendar.set_entry(d, entry);
    }
    true
}

/// The season's days a club's fixtures may fall on (`FUN_0072e5d0`).
const SEASON_DAYS: usize = 364;
/// A calendar code's competition (its top byte less the high bit): below
/// this a league matchday, then the cup's, the relegation's.
const COMPETITION: u32 = 0x7f00_0000;
const LEAGUE_CODES: u32 = 0x3f;
const CUP: u32 = 0x4000_0000;
const RELEGATION: u32 = 0x1000_0000;
const SUPERCUP: u32 = 0x3000_0000;
/// England's league cup's days (`FUN_006ceeb9`), its object a cup's.
const LEAGUE_CUP: u32 = 0x0100_0000;
/// The code's matchday or round, and the final's mark of a neutral ground.
const MATCHDAY: u32 = 0x3f;
const FINAL: usize = 6;
const NEUTRAL: u32 = 0x8008_0000;
const NEUTRAL_FINAL: u32 = 0x0008_0000;
/// A cup code's high bit: the tie stands the other way round.
const TURNED: u32 = 0x8000_0000;
/// A cup pairing's flags at +0xc, and the flag a turned tie keeps.
const PAIRING_FLAGS: usize = 0xc;
const KEPT_TURNED: u8 = 0x10;
/// The clubs' leagues below this are leagues (club +0x105).
const NO_LEAGUE: u8 = 10;
/// A match entry's ground: the hosting club again.
const MATCH_VENUE: usize = 0x16;
/// The cups' grounds of a neutral final, by the country's cup system
/// (`DAT_00b21aa6`, a u16 every four bytes, in the block from 0xb1f860).
const FINALS_BLOCK: &str = "0xb1f860";
const FINAL_GROUNDS: usize = 0xb21aa6 - 0xb1f860;

/// The new season's calendars (`FUN_00775219`'s `FUN_0072dcbf`, then
/// `FUN_0072f6f3` once the fixtures stand): every human's week planner and
/// year calendar emptied, and the year calendar given his club's fixtures
/// of the season (`FUN_0072e5d0`) — the league's matchdays, home and away,
/// and the cup's rounds as long as the club is in it, the drawn tie named.
/// The supercup's days follow its object ([`book_supercup`]). (The league
/// cup's, the European cups', the world cup's and the relegation's days,
/// and a national coach's matches, are not built.)
pub fn open_season(records: &mut Records, calendar: &crate::calendar::Calendar) {
    for planner in records.planners.iter_mut().chain(records.calendars.iter_mut()) {
        for d in 0..PLANNER_DAYS {
            planner.set_entry(d, [0; PLANNER_ENTRY]);
        }
    }
    for n in 0..usize::from(records.globals.humans()) {
        let Some(m) = records.managers.get(n) else { continue };
        let (country, number) = (m.country(), m.team());
        if country == 0 {
            continue;
        }
        let Some(mut planner) = records.calendars.get(n).cloned() else { continue };
        book_fixtures(records, calendar, (country, number), &mut planner);
        if let Some(c) = records.calendars.get_mut(n) {
            *c = planner;
        }
    }
}

/// A club's fixtures into its calendar (`FUN_0072e5d0`).
fn book_fixtures(records: &Records, calendar: &crate::calendar::Calendar, (country, number): (u16, u16), planner: &mut Planner) {
    let Some(team) = records.team(country, number) else { return };
    for d in 0..SEASON_DAYS {
        let code = fixture(records, calendar, team, d);
        if code == 0 {
            continue;
        }
        match code & COMPETITION {
            c if c < LEAGUE_CODES => book_league(records, team, d, code, planner),
            CUP => book_cup(records, team, d, code, planner),
            SUPERCUP => book_supercup(records, team, d, code, planner),
            _ => {}
        }
    }
}

/// What a club has on a day (`FUN_0062a0ad`): its league's matchday or
/// relegation tie, else its country's day (the cup's), else the
/// calendar's first row's.
fn fixture(records: &Records, calendar: &crate::calendar::Calendar, team: &crate::records::Team, d: usize) -> u32 {
    let country = u16::from(team.country());
    if records.countries.kind(country.into()) == 1 {
        if team.level() < NO_LEAGUE
            && let Some(row) = calendar.row(country, usize::from(team.level()))
        {
            let code = calendar.entry(row, d);
            if code != 0 && code & COMPETITION < LEAGUE_CODES || code & COMPETITION == RELEGATION {
                return code;
            }
        }
        if let Some(row) = calendar.row(country, 0) {
            let code = calendar.entry(row, d);
            if code & COMPETITION != 0 {
                return code;
            }
        }
    }
    let code = calendar.entry(0, d);
    if code & COMPETITION != 0 { code } else { 0 }
}

fn put_word(e: &mut [u8; PLANNER_ENTRY], at: usize, v: u16) {
    e[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

/// A league matchday (`FUN_0072dd36`): both clubs, the hosting one again,
/// the score once the league has played it.
fn book_league(records: &Records, team: &crate::records::Team, d: usize, code: u32, planner: &mut Planner) {
    let Some(league) = crate::opponent::League::of(records, u16::from(team.country()), team.level()) else { return };
    let mut e = planner.entry(d);
    e[0] = MATCH;
    e[1] = 1;
    e[MATCH_CODE..MATCH_CODE + 4].copy_from_slice(&code.to_le_bytes());
    let country = team.country();
    e[MATCH_HOME] = country;
    e[MATCH_AWAY] = country;
    let matchday = (code & MATCHDAY) as usize;
    let place = usize::from(team.league_place());
    let own = u16::from(team.number());
    let game = league.result(matchday, place);
    let other = game.map_or(0, |(home, away, _)| league.team(if home == place { away } else { home }) as u16);
    let at_home = game.is_some_and(|(home, _, _)| home == place);
    let (home, away) = if at_home { (own, other) } else { (other, own) };
    put_word(&mut e, MATCH_HOME + 2, home);
    put_word(&mut e, MATCH_AWAY + 2, away);
    if matchday <= league.played()
        && let Some((_, _, Some((h, a)))) = game
    {
        e[MATCH_GOALS] = h;
        e[MATCH_GOALS + 1] = a;
        e[MATCH_FLAGS] = RESULT_KNOWN;
    }
    e[MATCH_VENUE] = country;
    put_word(&mut e, MATCH_VENUE + 2, home);
    planner.set_entry(d, e);
}

/// A cup round's day (`FUN_0072ded2`): the club still in the cup — its
/// tie of the round named when drawn, the host as its ground (a final on
/// neutral ground the cup's) — or the day's match taken off.
fn book_cup(records: &Records, team: &crate::records::Team, d: usize, code: u32, planner: &mut Planner) {
    let slot = records.countries.slot(usize::from(team.country()));
    let Some(cup) = records.block(&format!("{:#x}[{slot}]", crate::opponent::CUPS_AT)) else { return };
    let mut e = planner.entry(d);
    if !in_cup(cup, team, code) {
        if e[0] == MATCH {
            e[0] = 0;
            planner.set_entry(d, e);
        }
        return;
    }
    e[0] = MATCH;
    e[1] = 1;
    e[MATCH_CODE..MATCH_CODE + 4].copy_from_slice(&code.to_le_bytes());
    let mode = usize::from(cup.get(0x24 - 0x20).copied().unwrap_or(0));
    let round = (code & MATCHDAY) as usize;
    let Some((start, count)) = crate::opponent::cup_pairings(mode, round) else {
        planner.set_entry(d, e);
        return;
    };
    let pairing = |k: usize| {
        let at = crate::opponent::PAIRINGS + k * crate::opponent::PAIRING;
        cup.get(at..at + crate::opponent::PAIRING)
    };
    let own = |side: &[u8]| side[0] == team.country() && u16::from_le_bytes([side[2], side[3]]) == u16::from(team.number());
    let found = (start..start + count).filter_map(pairing).find(|p| own(&p[0..4]) || own(&p[4..8]));
    if let Some(p) = found {
        let turned = code & TURNED != 0;
        let (first, second, goals) = if turned { (4, 0, [p[0xb], p[0xa]]) } else { (0, 4, [p[8], p[9]]) };
        e[MATCH_HOME..MATCH_HOME + 4].copy_from_slice(&p[first..first + 4]);
        e[MATCH_AWAY..MATCH_AWAY + 4].copy_from_slice(&p[second..second + 4]);
        e[MATCH_GOALS..MATCH_GOALS + 2].copy_from_slice(&goals);
        e[MATCH_FLAGS] = p[PAIRING_FLAGS];
        if turned && e[MATCH_FLAGS] & KEPT_TURNED == 0 {
            e[MATCH_FLAGS] = 0;
        }
        let ground: [u8; 4] = e[MATCH_HOME..MATCH_HOME + 4].try_into().unwrap_or_default();
        e[MATCH_VENUE..MATCH_VENUE + 4].copy_from_slice(&ground);
        if round == FINAL && code & NEUTRAL == NEUTRAL_FINAL {
            let system = usize::from(records.countries.cup_system(usize::from(team.country())));
            let at = FINAL_GROUNDS + system * 4;
            let number = records.block(FINALS_BLOCK).and_then(|b| b.get(at..at + 2)).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]));
            e[MATCH_VENUE] = cup.first().copied().unwrap_or(0);
            e[MATCH_VENUE + 1] = 0;
            put_word(&mut e, MATCH_VENUE + 2, number);
        }
    }
    planner.set_entry(d, e);
}

/// A supercup's day (`FUN_0072e066`): the club in the supercup's round
/// ([`in_supercup`]), its tie named when the club stands in it — a
/// tournament's tie of the round, else the one tie, the other way round
/// when the code says so (`FUN_006cda6f`, `FUN_0041a943`) — the host as
/// its ground.
fn book_supercup(records: &Records, team: &crate::records::Team, d: usize, code: u32, planner: &mut Planner) {
    let slot = records.countries.slot(usize::from(team.country()));
    let Some(object) = records.block(&format!("{:#x}[{slot}]", crate::supercup::OBJECT)) else { return };
    let tournament = crate::supercup::is_tournament(records, u16::from(team.country()));
    let mut e = planner.entry(d);
    if !in_supercup(object, team, code, tournament) {
        if e[0] == MATCH {
            e[0] = 0;
            planner.set_entry(d, e);
        }
        return;
    }
    e[0] = MATCH;
    e[1] = 1;
    e[MATCH_CODE..MATCH_CODE + 4].copy_from_slice(&code.to_le_bytes());
    let (tie, turned) = if tournament {
        (crate::supercup::TIES + (code & MATCHDAY) as usize * crate::supercup::TIE, false)
    } else {
        (crate::supercup::FINAL_TIE, code & TURNED != 0)
    };
    if let Some(p) = object.get(tie..tie + crate::supercup::TIE)
        && (supercup_side(&p[0..4], team) || supercup_side(&p[4..8], team))
    {
        let (first, second, goals) = if turned { (4, 0, [p[0xb], p[0xa]]) } else { (0, 4, [p[8], p[9]]) };
        e[MATCH_HOME..MATCH_HOME + 4].copy_from_slice(&p[first..first + 4]);
        e[MATCH_AWAY..MATCH_AWAY + 4].copy_from_slice(&p[second..second + 4]);
        e[MATCH_GOALS..MATCH_GOALS + 2].copy_from_slice(&goals);
        e[MATCH_FLAGS] = p[PAIRING_FLAGS];
        if turned && e[MATCH_FLAGS] & KEPT_TURNED == 0 {
            e[MATCH_FLAGS] = 0;
        }
        let ground: [u8; 4] = e[MATCH_HOME..MATCH_HOME + 4].try_into().unwrap_or_default();
        e[MATCH_VENUE..MATCH_VENUE + 4].copy_from_slice(&ground);
    }
    planner.set_entry(d, e);
}

/// A club's next match as the club object keeps it (+0xb44 …, team
/// +0x250 …): the day, the opponent, where (0 at home, 1 away), the kind
/// (2 the league, 3 the cup, 4 the supercup) and the round.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NextMatch {
    pub day: i16,
    pub opponent: (u8, i16),
    pub venue: i16,
    pub kind: u8,
    pub round: u8,
}

const LOOK_AHEAD: usize = 63;
const NEXT_LEAGUE: u8 = 2;
const NEXT_CUP: u8 = 3;
const NEXT_SUPERCUP: u8 = 4;
const SUPERCUP_STAGES: [u8; 5] = [5, 5, 6, 6, 7];

/// A club's next match from the day after `today` on, within 63 days
/// (`FUN_0062bfc9` with `FUN_0062d181`): the first day its league plays a
/// matchday, its cup — or England's league cup — a round it is in, its
/// supercup a round it plays —
/// the opponent named once the tie stands, none before. (A European
/// cup's, the world's and the relegation's days are not built: they are
/// passed over.)
pub fn next_match(records: &Records, calendar: &crate::calendar::Calendar, team: &crate::records::Team, today: usize) -> Option<NextMatch> {
    let country = team.country();
    for d in today + 1..=(today + LOOK_AHEAD).min(SEASON_DAYS - 1) {
        let code = fixture(records, calendar, team, d);
        if code == 0 {
            continue;
        }
        let at = |opponent: (u8, i16), venue: i16, kind: u8, round: u8| NextMatch { day: d as i16, opponent, venue, kind, round };
        match code & COMPETITION {
            c if c < LEAGUE_CODES => {
                let Some(league) = crate::opponent::League::of(records, u16::from(country), team.level()) else { continue };
                let place = usize::from(team.league_place());
                let Some((home, away, _)) = league.result((code & MATCHDAY) as usize, place) else { continue };
                let (other, venue) = if home == place { (away, 0) } else { (home, 1) };
                return Some(at((country, league.team(other)), venue, NEXT_LEAGUE, 0));
            }
            CUP | LEAGUE_CUP => {
                let slot = records.countries.slot(usize::from(country));
                let name = if code & COMPETITION == LEAGUE_CUP {
                    crate::cup_draw::LEAGUE_CUP.to_owned()
                } else {
                    format!("{:#x}[{slot}]", crate::opponent::CUPS_AT)
                };
                let Some(cup) = records.block(&name) else { continue };
                if !in_cup(cup, team, code) {
                    continue;
                }
                let round = (code & MATCHDAY) as u8;
                let mode = usize::from(cup.get(0x24 - 0x20).copied().unwrap_or(0));
                let tie = crate::opponent::cup_pairings(mode, usize::from(round)).and_then(|(start, count)| {
                    (start..start + count)
                        .filter_map(|k| {
                            let p = crate::opponent::PAIRINGS + k * crate::opponent::PAIRING;
                            cup.get(p..p + 8)
                        })
                        .find(|p| supercup_side(&p[0..4], team) || supercup_side(&p[4..8], team))
                        .map(<[u8]>::to_vec)
                });
                // A round not drawn yet is passed over.
                let Some(p) = tie else { continue };
                let at_home = supercup_side(&p[0..4], team) != (code & TURNED != 0);
                let o = if supercup_side(&p[0..4], team) { &p[4..8] } else { &p[0..4] };
                return Some(at((o[0], i16::from_le_bytes([o[2], o[3]])), i16::from(!at_home), NEXT_CUP, round));
            }
            SUPERCUP => {
                let slot = records.countries.slot(usize::from(country));
                let Some(object) = records.block(&format!("{:#x}[{slot}]", crate::supercup::OBJECT)) else { continue };
                let tournament = crate::supercup::is_tournament(records, u16::from(country));
                if !in_supercup(object, team, code, tournament) {
                    continue;
                }
                let round = (code & MATCHDAY) as usize;
                let tie_at = if tournament && round < 4 { crate::supercup::TIES + round * crate::supercup::TIE } else { crate::supercup::FINAL_TIE };
                let p = object.get(tie_at..tie_at + 8).map(<[u8]>::to_vec).unwrap_or_default();
                let theirs = |side: &[u8]| (side[0], i16::from_le_bytes([side[2], side[3]]));
                // A seed waiting for its opponent is passed over; the
                // rounds count on from 5 — 5 the first two ties, 6 the
                // next two, 7 the final (`FUN_006cd7fb`).
                if p.len() != 8 || !(supercup_side(&p[0..4], team) || supercup_side(&p[4..8], team)) {
                    continue;
                }
                let first = supercup_side(&p[0..4], team);
                let at_home = first != (code & TURNED != 0);
                let stage = SUPERCUP_STAGES[round.min(SUPERCUP_STAGES.len() - 1)];
                return Some(at(theirs(if first { &p[4..8] } else { &p[0..4] }), i16::from(!at_home), NEXT_SUPERCUP, stage));
            }
            _ => continue,
        }
    }
    None
}

/// Every played club's next match noted anew (`FUN_0062bfc9`): its
/// opponent, kind and referee cleared first, then the match found set; a
/// human club keeps its referee, which the original draws anew (not
/// built).
pub fn note_next_matches(records: &mut Records, calendar: &crate::calendar::Calendar, today: usize) {
    for country in crate::opponent::played_nations(records) {
        for number in 0..records.countries.team_count(usize::from(country)).max(0) as u16 {
            let Some(team) = records.team(country, number) else { continue };
            // A club in no league is not looked at; it keeps what it had.
            if team.level() >= NO_LEAGUE {
                continue;
            }
            let next = next_match(records, calendar, team, today);
            let human = team.humans() != [0, 0];
            let Some(team) = records.team_mut(country, number) else { continue };
            team.set_next_country(0);
            team.set_next_number(0);
            team.set_next_kind(0);
            if !human {
                team.set_referee(0);
            }
            if let Some(m) = next {
                team.set_next_day(m.day);
                team.set_next_country(m.opponent.0);
                team.set_next_number(m.opponent.1);
                team.set_next_venue(m.venue);
                team.set_next_kind(m.kind);
                team.set_next_round(m.round);
            }
        }
    }
}

/// A human's supercup days from `from` on booked again from the object as
/// it now stands, his club's next ties named once drawn.
pub(crate) fn rebook_supercup(records: &mut Records, calendar: &crate::calendar::Calendar, slot: usize, (country, number): (u16, u16), from: usize) {
    let Some(team) = records.team(country, number).cloned() else { return };
    let Some(mut planner) = records.calendars.get(slot).cloned() else { return };
    for d in from..SEASON_DAYS {
        let code = fixture(records, calendar, &team, d);
        if code != 0 && code & COMPETITION == SUPERCUP {
            book_supercup(records, &team, d, code, &mut planner);
        }
    }
    if let Some(c) = records.calendars.get_mut(slot) {
        *c = planner;
    }
}

/// A side of a supercup tie is the club's (`FUN_006782aa`).
fn supercup_side(side: &[u8], team: &crate::records::Team) -> bool {
    side[0] == team.country() && u16::from_le_bytes([side[2], side[3]]) == u16::from(team.number())
}

/// Whether a club plays a supercup round (`FUN_006ccf10`): in the one
/// tie; in a tournament the round's tie, or — for the third and fourth
/// round — the first or second of the six waiting there, or a tie of an
/// earlier round not played yet that holds it, the final any tie not
/// played that might still send it there.
fn in_supercup(object: &[u8], team: &crate::records::Team, code: u32, tournament: bool) -> bool {
    use crate::supercup::{FINAL_TIE, SIX, TIE, TIES};
    let in_tie = |at: usize| object.get(at..at + 8).is_some_and(|p| supercup_side(&p[0..4], team) || supercup_side(&p[4..8], team));
    let open = |k: usize| object.get(TIES + k * TIE + PAIRING_FLAGS).copied().unwrap_or(0) == 0;
    let seed = |k: usize| object.get(SIX + 2 * k..SIX + 2 * k + 2).is_some_and(|w| u16::from_le_bytes([w[0], w[1]]) == u16::from(team.number()));
    if !tournament {
        return in_tie(FINAL_TIE);
    }
    match code & MATCHDAY {
        0 => in_tie(TIES),
        1 => in_tie(TIES + TIE),
        2 => in_tie(TIES + 2 * TIE) || seed(0) || open(0) && in_tie(TIES),
        3 => in_tie(TIES + 3 * TIE) || seed(1) || open(1) && in_tie(TIES + TIE),
        4 => {
            in_tie(FINAL_TIE)
                || open(2) && seed(0)
                || open(3) && seed(1)
                || (0..4).any(|k| open(k) && in_tie(TIES + k * TIE))
        }
        _ => false,
    }
}

/// Whether a club takes part in a cup round (`FUN_006d1bb5`): with no tie
/// from the drawn round on, when it has entered; with one, unless the
/// round lies before the drawn one, or it fell out where a later tie
/// stands.
fn in_cup(cup: &[u8], team: &crate::records::Team, code: u32) -> bool {
    let mode = usize::from(cup.get(0x24 - 0x20).copied().unwrap_or(0)).min(crate::opponent::ROUNDS.len() - 1);
    let rounds = &crate::opponent::ROUNDS[mode];
    let drawn = usize::from(cup.get(0x26 - 0x20).copied().unwrap_or(0)).min(rounds.len() - 1);
    let word = |at: usize| cup.get(at..at + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]));
    let final_at = rounds[5].0 + rounds[5].1;
    let number = u16::from(team.number());
    let (mut first, mut own) = (None, None);
    for k in (rounds[drawn].0..=final_at).rev() {
        let at = crate::opponent::PAIRINGS + k * crate::opponent::PAIRING;
        if cup.get(at).copied().unwrap_or(0) == 0 {
            continue;
        }
        first.get_or_insert(k);
        if word(at + 2) == number || word(at + 6) == number {
            own = Some(k);
            break;
        }
    }
    let round_of = |k: usize| rounds.iter().position(|&(s, n)| (s..s + n).contains(&k)).map_or(-1, |r| r as i32);
    let asked = (code & 0xff) as i32;
    match own {
        None => {
            let entrants = usize::try_from(i16::from_le_bytes([cup[0x2a - 0x20], cup[0x2b - 0x20]])).unwrap_or(0);
            (0..entrants).any(|k| word(0x528 - 0x20 + 4 * k) == number)
        }
        Some(k) => {
            let (own, first) = (round_of(k), round_of(first.unwrap_or(k)));
            if asked < drawn as i32 {
                false
            } else if own < asked {
                own >= first
            } else {
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On every kept save of the original — mid-season, the new season's
    /// first day, the second season after a cup exit — the calendar the
    /// replica builds from the save's fixtures holds the league's, the
    /// cup's and the supercup's days byte for byte as the original's
    /// calendar there; the other competitions' days (European cups, a
    /// national coach's) stay empty, the planner too.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_season_books_its_fixtures_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let data = crate::data::Data::load(&dir).unwrap();
        let (mut days, mut saves, mut supercup) = (0, 0, 0);
        for name in ["A.an3", "Ap.an3", "Toni.an3", "SeasonEnd364.an3", "NewSeason000.an3", "S2c.an3", "S2late.an3"] {
            let load = || Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
            let (mut ours, theirs) = (load(), load());
            open_season(&mut ours, &data.calendar);
            saves += 1;
            assert!(ours.planners.iter().all(|p| (0..PLANNER_DAYS).all(|d| p.entry(d)[0] == 0)), "{name}");
            for d in 0..PLANNER_DAYS {
                let (x, y) = (ours.calendars[0].entry(d), theirs.calendars[0].entry(d));
                let code = u32::from_le_bytes([y[4], y[5], y[6], y[7]]) & COMPETITION;
                if y[0] == MATCH && (code < LEAGUE_CODES || code == CUP || code == SUPERCUP) {
                    supercup += i32::from(code == SUPERCUP);
                    assert_eq!(x, y, "{name} day {d}");
                    days += 1;
                } else {
                    assert_eq!(x, [0; PLANNER_ENTRY], "{name} day {d}");
                }
            }
        }
        assert!(days > 36 * saves && supercup > 0, "{days} {supercup}");
    }

    /// On the original's saves — the new season's first day, mid-season,
    /// late in the second — every played club's next match as the replica
    /// finds it from the day before the save agrees with the one the
    /// original noted: the league's, the cup's and the supercup's (those of
    /// the European cups not built, the clubs in no league not looked at).
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_clubs_next_matches_come_out_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let data = crate::data::Data::load(&dir).unwrap();
        let mut misses = Vec::new();
        let mut all = 0;
        for name in ["NewSeason000.an3", "A.an3", "S2late.an3"] {
            let n = Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
            // The saves stand before their day's matches: what they noted
            // was looked for from the day before on.
            let today = (n.globals.day() as usize).saturating_sub(1);
            for country in crate::opponent::played_nations(&n) {
                for t in 0..n.countries.team_count(usize::from(country)).max(0) as u16 {
                    let team = n.team(country, t).unwrap();
                    // Not looked at: a club in no league, and one whose next
                    // match is a European or a world one (not built).
                    if team.next_country() == 0 && team.next_kind() == 0 || team.level() >= NO_LEAGUE || (5..=10).contains(&team.next_kind()) {
                        continue;
                    }
                    all += 1;
                    let mine = next_match(&n, &data.calendar, team, today);
                    let theirs = NextMatch {
                        day: team.next_day(),
                        opponent: (team.next_country(), team.next_number()),
                        venue: team.next_venue(),
                        kind: team.next_kind(),
                        round: team.next_round(),
                    };
                    if mine != Some(theirs) {
                        misses.push(format!("{name} {country}/{t}: mine {mine:?} theirs {theirs:?}"));
                    }
                }
            }
        }
        assert!(all > 400 && misses.is_empty(), "{} misses of {all}:\n{}", misses.len(), misses.join("\n"));
    }

    fn calendar() -> Planner {
        Planner::new(&vec![0; Planner::SIZE]).unwrap()
    }

    fn camp(calendar: &mut Planner, start: usize, end: usize) {
        let mut first = [0; PLANNER_ENTRY];
        first[0] = CAMP;
        first[1] = 1;
        first[4] = 1;
        first[6] = 7;
        calendar.set_entry(start, first);
        let mut last = [0; PLANNER_ENTRY];
        last[0] = CAMP;
        last[1] = 1;
        calendar.set_entry(end, last);
    }

    #[test]
    fn a_camp_ends_later_or_sooner() {
        let mut c = calendar();
        camp(&mut c, 70, 71);
        assert!(in_camp(&c, 70) && in_camp(&c, 71) && !in_camp(&c, 72));
        assert!(move_camp_end(&mut c, 75, 65));
        assert_eq!(day(&c, 71).kind(), 0);
        assert!(in_camp(&c, 73) && day(&c, 75).kind() == CAMP && !in_camp(&c, 76));
        assert!(move_camp_end(&mut c, 72, 65));
        assert!(!in_camp(&c, 73) && day(&c, 72).kind() == CAMP);
        // Not on today or before.
        assert!(!move_camp_end(&mut c, 65, 65));
    }

    #[test]
    fn a_party_is_booked_and_cleared() {
        let mut c = calendar();
        book(&mut c, 80, PARTY, 2);
        assert_eq!((day(&c, 80).kind(), day(&c, 80).value()), (PARTY, 2));
        clear(&mut c, 80);
        assert_eq!(day(&c, 80).kind(), 0);
    }
}

/// A table of u16 values a u32 apart.
fn every_fourth(exe: &Exe, at: u32, count: usize) -> io::Result<Vec<u16>> {
    let bytes = exe.bytes(at, 4 * count)?;
    Ok(bytes.chunks(4).map(|c| u16::from_le_bytes([c[0], c[1]])).collect())
}
