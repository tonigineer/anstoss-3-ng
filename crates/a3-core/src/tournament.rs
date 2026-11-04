//! Tournaments (JAHRESKALENDER.md §4.3 in the RE repo): the list of the
//! season's tournaments (`CDatenTurnierliste`, `0x1c4be78`) — a Turnier of
//! four clubs, a Hallenturnier or a Strandturnier of eight — which a human
//! club joins or hosts from its year calendar.
//!
//! The computer's clubs host Turniere in the pre-season and Hallenturniere
//! in the winter break; a human hosts his own by inviting the clubs the
//! turn offers him for the day, the friendly's ([`crate::friendly`]).

use crate::random::Random;
use crate::records::{PLANNER_ENTRY, Planner, Records};

/// The kinds of tournament; the calendar's kind is two more.
pub const TURNIER: u8 = 0;
pub const HALLE: u8 = 1;
pub const STRAND: u8 = 2;
const CALENDAR_KIND: u8 = 2;
/// The clubs each kind needs (`DAT_008abe90`).
const NEEDED: [usize; 3] = [4, 8, 8];
/// At most this many of a day and kind are offered to join
/// (`FUN_0076090e`).
const OFFERED: usize = 10;

/// The list: its count, then two hundred entries of 0x74 bytes.
const LIST_AT: &str = "0x1c4be78";
const ENTRIES: usize = 4;
const ENTRY: usize = 0x74;
const MOST: usize = 200;
/// An entry's fields: the kind, its id, the clubs in it, its first day,
/// the clubs' slots of four bytes (country, a byte, number), the host's
/// first.
const KIND: usize = 0;
const ID: usize = 4;
const COUNT: usize = 8;
const DAY: usize = 0xa;
const SLOTS: usize = 0xc;
const SLOT: usize = 4;

/// A tournament as the list keeps it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Tournament([u8; ENTRY]);

impl Tournament {
    fn word(&self, at: usize) -> i16 {
        i16::from_le_bytes([self.0[at], self.0[at + 1]])
    }

    fn set_word(&mut self, at: usize, value: i16) {
        self.0[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }

    pub fn kind(&self) -> u8 {
        self.0[KIND]
    }

    pub fn id(&self) -> u32 {
        u32::from_le_bytes(self.0[ID..ID + 4].try_into().unwrap_or_default())
    }

    /// Its first day; it takes the next as well.
    pub fn day(&self) -> i16 {
        self.word(DAY)
    }

    pub fn count(&self) -> usize {
        usize::try_from(self.word(COUNT)).unwrap_or(0)
    }

    /// The clubs it may take (`FUN_0076006a`): four for a Turnier, eight
    /// for the others.
    fn room(&self) -> usize {
        if self.kind() == TURNIER { NEEDED[0] } else { NEEDED[1] }
    }

    /// The clubs in it, the host first.
    pub fn clubs(&self) -> Vec<(u8, i16)> {
        (0..self.count().min(8)).map(|k| self.club(k)).collect()
    }

    fn club(&self, k: usize) -> (u8, i16) {
        let at = SLOTS + k * SLOT;
        (self.0[at], self.word(at + 2))
    }

    pub fn host(&self) -> (u8, i16) {
        self.club(0)
    }

    /// The host's slot as it stands, its second byte with it: what the
    /// calendar's days of it carry.
    fn host_bytes(&self) -> [u8; 4] {
        self.0[SLOTS..SLOTS + SLOT].try_into().unwrap_or_default()
    }

    /// Whether the club is in it (`FUN_0075ffdd`).
    pub fn has(&self, club: (u8, i16)) -> bool {
        self.clubs().contains(&club)
    }

    /// Takes a club into the next slot while there is room
    /// (`FUN_0076006a`); it does not look whether the club is in already.
    fn add(&mut self, (country, number): (u8, i16)) -> bool {
        let count = self.count();
        if count >= self.room() {
            return false;
        }
        let at = SLOTS + count * SLOT;
        self.0[at] = country;
        self.set_word(at + 2, number);
        self.set_word(COUNT, count as i16 + 1);
        true
    }

    /// Whether it has the clubs its kind needs.
    pub fn full(&self) -> bool {
        self.count() >= self.room()
    }
}

/// The season's tournaments.
pub fn tournaments(records: &Records) -> Vec<Tournament> {
    let Some(block) = records.block(LIST_AT) else { return Vec::new() };
    let count = usize::try_from(i16::from_le_bytes([block[0], block[1]])).unwrap_or(0).min(MOST);
    (0..count)
        .filter_map(|k| block.get(ENTRIES + k * ENTRY..ENTRIES + (k + 1) * ENTRY))
        .filter_map(|e| e.try_into().ok().map(Tournament))
        .collect()
}

/// Writes the list back (`FUN_00760759`): the entries past it cleared.
fn store(records: &mut Records, list: &[Tournament]) {
    let Some(block) = records.block_mut(LIST_AT) else { return };
    let old = usize::try_from(i16::from_le_bytes([block[0], block[1]])).unwrap_or(0).min(MOST);
    for (k, t) in list.iter().enumerate().take(MOST) {
        block[ENTRIES + k * ENTRY..ENTRIES + (k + 1) * ENTRY].copy_from_slice(&t.0);
    }
    for k in list.len()..old {
        block[ENTRIES + k * ENTRY..ENTRIES + (k + 1) * ENTRY].fill(0);
    }
    block[0..2].copy_from_slice(&(list.len().min(MOST) as i16).to_le_bytes());
}

/// A tournament's id (`FUN_00760e8d`): the host's country and number in
/// the top bytes, its first day below.
pub fn id(host: (u8, i16), day: i16) -> u32 {
    crate::friendly::id(host, day)
}

/// The tournament with an id (`FUN_00761444`).
pub fn find(records: &Records, id: u32) -> Option<Tournament> {
    tournaments(records).into_iter().find(|t| t.id() == id)
}

/// The tournaments of a kind starting on a day, ten at most
/// (`FUN_0076090e`).
pub fn on_day(records: &Records, day: i16, kind: u8) -> Vec<Tournament> {
    tournaments(records).into_iter().filter(|t| t.day() == day && t.kind() == kind).take(OFFERED).collect()
}

/// A tournament of the club's own on a day (`FUN_00760d7a`): the club its
/// host and only club so far, in the list unless it is full. Its id.
pub fn open_own(records: &mut Records, club: (u8, i16), day: i16, kind: u8) -> Option<u32> {
    let mut list = tournaments(records);
    if list.len() >= MOST {
        return None;
    }
    let mut own = Tournament([0; ENTRY]);
    own.0[KIND] = kind;
    own.0[ID..ID + 4].copy_from_slice(&id(club, day).to_le_bytes());
    own.set_word(DAY, day);
    own.add(club);
    list.push(own);
    store(records, &list);
    Some(own.id())
}

/// Takes a tournament out of the list (`FUN_00760de0`).
pub fn remove(records: &mut Records, id: u32) {
    let mut list = tournaments(records);
    if let Some(k) = list.iter().position(|t| t.id() == id) {
        list.remove(k);
        store(records, &list);
    }
}

/// The calendar of a human club's manager, by his number from 1
/// (`FUN_0067764c`); none for a computer club.
fn calendar_of(records: &Records, club: (u8, i16)) -> Option<usize> {
    let manager = crate::friendly::manager_of(records, club);
    (1..=4).contains(&manager).then(|| usize::from(manager) - 1).filter(|&m| m < records.calendars.len())
}

/// Whether a calendar's two days of the tournament are free.
fn free(calendar: &Planner, day: i16) -> bool {
    let d = usize::try_from(day).unwrap_or(usize::MAX);
    calendar.entry(d)[0] == 0 && calendar.entry(d + 1)[0] == 0
}

/// The tournament's two days in a calendar (`FUN_0072ede4` with kinds 2 to
/// 4): its kind, the first marked 1 and the second 2, the host's slot.
pub fn mark(calendar: &mut Planner, t: &Tournament) {
    let d = usize::try_from(t.day()).unwrap_or(usize::MAX);
    for (k, mark) in [(d, 1), (d + 1, 2)] {
        let mut entry = [0; PLANNER_ENTRY];
        entry[0] = t.kind() + CALENDAR_KIND;
        entry[1] = mark;
        entry[4..8].copy_from_slice(&t.host_bytes());
        calendar.set_entry(k, entry);
    }
}

/// A club joins a tournament (`FUN_00760f5d`): a human club only with both
/// days free in his calendar, which then shows it; any club while there is
/// room.
pub fn join(records: &mut Records, club: (u8, i16), id: u32) -> bool {
    let mut list = tournaments(records);
    let Some(t) = list.iter_mut().find(|t| t.id() == id) else { return false };
    let calendar = calendar_of(records, club);
    if calendar.is_some_and(|m| !free(&records.calendars[m], t.day())) || !t.add(club) {
        return false;
    }
    let t = *t;
    if let Some(m) = calendar {
        mark(&mut records.calendars[m], &t);
    }
    store(records, &list);
    true
}

/// Why a tournament of one's own is not to be: a guest's calendar taken
/// on its days, no room left, or not the clubs its kind needs
/// (`FUN_0076029f`'s 2, 3 and 4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Short {
    Taken,
    NoRoom,
    TooFew,
}

/// The guests invited to a tournament of one's own (`FUN_0076029f`): each
/// human guest needs its days free; then the tournament must have the
/// clubs its kind needs, four or eight. So it goes into the list, and the
/// human guests' calendars show it.
pub fn invite(records: &mut Records, id: u32, guests: &[(u8, i16)]) -> Result<(), Short> {
    let Some(mut t) = find(records, id) else { return Err(Short::TooFew) };
    for &guest in guests {
        if calendar_of(records, guest).is_some_and(|m| !free(&records.calendars[m], t.day())) {
            return Err(Short::Taken);
        }
        if !t.add(guest) {
            return Err(Short::NoRoom);
        }
    }
    if t.count() != NEEDED[usize::from(t.kind().min(STRAND))] {
        return Err(Short::TooFew);
    }
    for &guest in guests {
        if let Some(m) = calendar_of(records, guest) {
            mark(&mut records.calendars[m], &t);
        }
    }
    let mut list = tournaments(records);
    match list.iter().position(|e| e.id() == id) {
        Some(k) => list[k] = t,
        None if list.len() < MOST => list.push(t),
        None => {}
    }
    store(records, &list);
    Ok(())
}

/// How many clubs a kind needs.
pub fn needed(kind: u8) -> usize {
    NEEDED[usize::from(kind.min(STRAND))]
}

/// What a guest costs its host (`FUN_006263ee`): 100 000 for a club from
/// abroad or a champion, else 50 000 from the first division, 20 000 from
/// the second, 10 000 from below.
pub fn fee(records: &Records, host: (u8, i16), guest: (u8, i16)) -> i32 {
    const CHAMPION: u16 = 0x1;
    let team = u16::try_from(guest.1).ok().and_then(|n| records.team(guest.0.into(), n));
    if guest.0 != host.0 || team.is_some_and(|t| t.get::<u16>(0x110) & CHAMPION != 0) {
        return 100_000;
    }
    match team.map_or(10, |t| t.level()) {
        0 => 50_000,
        1 => 20_000,
        _ => 10_000,
    }
}

/// The clubs' flags of the tournaments they host (club +0xf7, team
/// +0x125): a Turnier, a hall's.
const HOSTING: usize = 0x125;
const HOSTS_TURNIER: u8 = 1;
const HOSTS_HALL: u8 = 2;
/// A host's slot keeps in its second byte what the original's stack held
/// there as it made the entry: always this for a Turnier, this for a hall.
const TURNIER_STACK: u8 = 0x1a;
const HALL_STACK: u8 = 0xc7;
/// The main country's Turniere of the pre-season, up to day 39, the
/// lower divisions' only before day 21, a second on these weekdays from
/// day 19 (`DAT_008ac840`); the other played countries' from day 14 to 33.
/// No tournament starts on a day that leaves 6 by 7.
const PRESEASON_LAST: i16 = 39;
const LOWER_UNTIL: i16 = 21;
const LOWER: u8 = 2;
const SECOND_FROM: i16 = 19;
const SECOND: [bool; 7] = [false, false, true, true, false, true, true];
const OTHERS: std::ops::RangeInclusive<i16> = 14..=33;
const NO_START: i16 = 6;
/// A pick tries this many clubs (`FUN_00771dfc`, `FUN_0064cadd`).
const TRIES: usize = 10;
/// The UEFA Cup's clubs host no Turnier (team +0x110).
const IN_UEFA: u16 = 0x20;
/// The winter's halls: Germany's from half week 48, ten days from the
/// week's first day plus 10 and thirteen from plus 20 (two a day, one of
/// them from the lower divisions), France's from half week 52, twelve
/// days from plus 10.
pub const GERMANY_HALLS: u16 = 48;
pub const FRANCE_HALLS: u16 = 52;
const GERMANY: u8 = 14;
const FRANCE: u8 = 20;

/// A club of the country to host (`FUN_00771dfc`, `FUN_0064cadd`): ten
/// tries at random, each taking a club of at least the division `lowest`
/// (0 the first) that is no human's and hosts no tournament of the kind
/// yet — a Turnier's not in the UEFA Cup either.
fn pick(records: &Records, country: u8, lowest: u8, flag: u8, random: &mut Random) -> Option<i16> {
    let clubs = u32::try_from(records.countries.team_count(country.into())).unwrap_or(0);
    for _ in 0..TRIES {
        let number = random.below(clubs) as i16;
        let Some(team) = records.team(country.into(), number as u16) else { continue };
        let fits = team.level() >= lowest
            && (flag != HOSTS_TURNIER || team.get::<u16>(0x110) & IN_UEFA == 0)
            && !crate::statistics::is_human(records, country, number)
            && team.get::<u8>(HOSTING) & flag == 0;
        if fits {
            return Some(number);
        }
    }
    None
}

/// A computer club's tournament on a day (`FUN_00760036`): the host its
/// only club so far; the host marked as hosting one of the kind
/// (`FUN_007719bf`, `FUN_0064c72d`).
fn host(records: &mut Records, (country, number): (u8, i16), day: i16, kind: u8) -> Tournament {
    let (flag, stack) = if kind == TURNIER { (HOSTS_TURNIER, TURNIER_STACK) } else { (HOSTS_HALL, HALL_STACK) };
    let mut t = Tournament([0; ENTRY]);
    t.0[KIND] = kind;
    t.0[ID..ID + 4].copy_from_slice(&id((country, number), day).to_le_bytes());
    t.set_word(DAY, day);
    t.add((country, number));
    t.0[SLOTS + 1] = stack;
    if let Some(team) = records.team_mut(country.into(), number as u16) {
        team.put::<u8>(HOSTING, team.get::<u8>(HOSTING) | flag);
    }
    t
}

/// The pre-season's Turniere, as the new season opens (`FUN_007719a6`'s
/// `FUN_00771a61`): the list starts afresh; the main country's clubs host
/// one a day up to day 39 — before day 21 a club below the second
/// division — and a second on some weekdays from day 19, the other played
/// countries' one a day from day 14 to 33. A club hosts one at most.
pub fn open_preseason(records: &mut Records, random: &mut Random) {
    let mut list: Vec<Tournament> = Vec::new();
    let upsert = |list: &mut Vec<Tournament>, t: Tournament| match list.iter().position(|e| e.id() == t.id()) {
        Some(k) => list[k] = t,
        None if list.len() < MOST => list.push(t),
        None => {}
    };
    let main = records.globals.main_country() as u8;
    for day in 1..=PRESEASON_LAST {
        if day % 7 == NO_START {
            continue;
        }
        let lowest = if day < LOWER_UNTIL { LOWER } else { 0 };
        if let Some(number) = pick(records, main, lowest, HOSTS_TURNIER, random) {
            let t = host(records, (main, number), day, TURNIER);
            upsert(&mut list, t);
        }
        if day >= SECOND_FROM
            && SECOND[day as usize % 7]
            && let Some(number) = pick(records, main, 0, HOSTS_TURNIER, random)
        {
            let t = host(records, (main, number), day, TURNIER);
            upsert(&mut list, t);
        }
    }
    let more = usize::try_from(records.globals.more_count()).unwrap_or(0);
    let countries: Vec<u8> = records.globals.more_countries().iter().take(more).map(|&c| c as u8).collect();
    for country in countries {
        for day in OTHERS {
            if day % 7 == NO_START {
                continue;
            }
            if let Some(number) = pick(records, country, 0, HOSTS_TURNIER, random) {
                let t = host(records, (country, number), day, TURNIER);
                upsert(&mut list, t);
            }
        }
    }
    store(records, &list);
}

/// The winter's Hallenturniere of a country (`FUN_00771959`'s
/// `FUN_0064c795`), as half week 48 (Germany) or 52 (France) begins,
/// counted from the week's first day as the original's cycle has it then
/// — 161 and 175 —, appended to the list; a club hosts one at most.
pub fn open_winter(records: &mut Records, country: u8, half_weeks: u16, random: &mut Random) {
    let week = (i16::try_from(half_weeks).unwrap_or(0) / 2 - 1) * 7;
    let days: Vec<(i16, &[u8])> = match country {
        GERMANY => (0..=9)
            .map(|i| (week + 10 + i, &[0][..]))
            .chain((0..=12).map(|i| (week + 20 + i, &[0, LOWER][..])))
            .collect(),
        FRANCE => (0..=11).map(|i| (week + 10 + i, &[0][..])).collect(),
        _ => return,
    };
    let mut list = tournaments(records);
    for (day, lowests) in days {
        if day % 7 == NO_START {
            continue;
        }
        for &lowest in lowests {
            if let Some(number) = pick(records, country, lowest, HOSTS_HALL, random) {
                let t = host(records, (country, number), day, HALLE);
                if list.len() < MOST {
                    list.push(t);
                }
            }
        }
    }
    store(records, &list);
}

/// A Turnier's four games (0xe bytes from +0x2c): the semifinals, the
/// game for third place, the final. Each: the home club's slot, the away
/// club's, the goals, the flags, the winning side (0 the home).
const GAMES: usize = 0x2c;
const GAME: usize = 0xe;
pub const SEMI_ONE: usize = 0;
pub const SEMI_TWO: usize = 1;
pub const THIRD: usize = 2;
pub const FINAL: usize = 3;
const GAME_GOALS: usize = 8;
const GAME_FLAGS: usize = 0xc;
const GAME_WINNER: usize = 0xd;
/// The crowds, the gate, the fees the host paid and its costs.
const CROWDS: usize = 0x64;
const GATES: usize = 0x68;
const FEES: usize = 0x6c;
const COSTS: usize = 0x70;
/// What a Turnier costs its host, and the prize money of the first three
/// (into the plan's premiums, club +0x148 — where the original books it).
const HOST_COSTS: i32 = 70_000;
const PRIZES: [i32; 3] = [40_000, 20_000, 10_000];
/// The respect the winner gains, the third and the fourth lose (club
/// +0x92, team +0xb6), within 0 and 200.
const RESPECT_WINNER: i16 = 5;
const RESPECT_THIRD: i16 = -3;
const RESPECT_FOURTH: i16 = -8;
const RESPECT_MOST: i16 = 200;
/// The club's money the tournaments book (club offsets): the other
/// costs and incomes, the gates of friendlies and tournaments, the plan's
/// premiums.
const OTHER_COSTS: usize = 0x2c0;
const OTHER_INCOME: usize = 0x2c4;
const GATE: usize = 0x23c;
const PLAN_PREMIUMS: usize = 0x148;
/// The fill: thirty tries at most (`FUN_006261b5`), a near club in five of
/// ten, one of the first division in four, one abroad in one
/// (`FUN_00771ed5`); the near ones looked at up to 108.
const FILL_TRIES: usize = 30;
const NEAR_MOST: usize = 108;
/// The first pre-season's matches stand still while `DAT_00b5d19a` is set
/// (`FUN_005ef59d`).
const STILL: usize = 0xb5d19a;
const STILL_UNTIL: i16 = 42;

impl Tournament {
    fn slot(&self, k: usize) -> [u8; 4] {
        self.0[SLOTS + k * SLOT..SLOTS + (k + 1) * SLOT].try_into().unwrap_or_default()
    }

    fn game(&self, i: usize) -> &[u8] {
        &self.0[GAMES + i * GAME..GAMES + (i + 1) * GAME]
    }

    fn game_mut(&mut self, i: usize) -> &mut [u8] {
        &mut self.0[GAMES + i * GAME..GAMES + (i + 1) * GAME]
    }

    fn int(&self, at: usize) -> i32 {
        i32::from_le_bytes(self.0[at..at + 4].try_into().unwrap_or_default())
    }

    fn set_int(&mut self, at: usize, value: i32) {
        self.0[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    /// A game's two clubs, home first.
    pub fn sides(&self, i: usize) -> [(u8, i16); 2] {
        let g = self.game(i);
        [(g[0], i16::from_le_bytes([g[2], g[3]])), (g[4], i16::from_le_bytes([g[6], g[7]]))]
    }

    /// A game's result once played: the goals, extra time, penalties, the
    /// winning side (0 the home).
    pub fn result(&self, i: usize) -> Option<((u8, u8), bool, bool, u8)> {
        let g = self.game(i);
        (g[GAME_FLAGS] & PLAYED != 0)
            .then(|| ((g[GAME_GOALS], g[GAME_GOALS + 1]), g[GAME_FLAGS] & EXTRA != 0, g[GAME_FLAGS] & PENALTIES != 0, g[GAME_WINNER]))
    }

    /// A game's winner's and loser's slots (`FUN_00422a7c`, `FUN_00422ad8`).
    fn winner_loser(&self, i: usize) -> ([u8; 4], [u8; 4]) {
        let g = self.game(i);
        let (home, away): ([u8; 4], [u8; 4]) = (g[0..4].try_into().unwrap_or_default(), g[4..8].try_into().unwrap_or_default());
        if g[GAME_WINNER] == 0 { (home, away) } else { (away, home) }
    }

    fn set_sides(&mut self, i: usize, home: [u8; 4], away: [u8; 4]) {
        let g = self.game_mut(i);
        g[0..4].copy_from_slice(&home);
        g[4..8].copy_from_slice(&away);
    }

    /// The crowds, the gate, the fees its host paid, its costs.
    pub fn takings(&self) -> (i32, i32, i32, i32) {
        (self.int(CROWDS), self.int(GATES), self.int(FEES), self.int(COSTS))
    }
}

/// A game's flags: played, after extra time, on penalties.
pub const PLAYED: u8 = 0x20;
pub const EXTRA: u8 = 0x80;
pub const PENALTIES: u8 = 0x40;

/// A game of the day's list that a tournament plays (`FUN_00625974`): the
/// tournament, which of its games, the sides, the ground (the host's).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Fixture {
    pub id: u32,
    pub game: usize,
    pub home: (u8, i16),
    pub away: (u8, i16),
    pub ground: (u8, i16),
}

/// How a game of the day went: the goals, extra time, penalties, the
/// winning side, the crowd and the gate.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Outcome {
    pub goals: (u8, u8),
    pub extra: bool,
    pub penalties: bool,
    pub winner: u8,
    pub crowd: i32,
    pub gate: i32,
}

/// Whether the first pre-season's matches stand still on a day
/// (`FUN_005ef59d`).
pub(crate) fn still(records: &Records, day: i16) -> bool {
    day < STILL_UNTIL && records.globals.seasons() == 1 && records.globals.get::<u8>(STILL) != 0
}

/// Books money onto a club's record by its club-object offset.
fn book(records: &mut Records, (country, number): (u8, i16), at: usize, amount: i32) {
    if let Some(team) = u16::try_from(number).ok().and_then(|n| records.team_mut(country.into(), n)) {
        let at = at + crate::finances::CLUB_TO_TEAM;
        team.put::<i32>(at, team.get::<i32>(at).wrapping_add(amount));
    }
}

/// Changes a club's respect within 0 and 200.
fn respect(records: &mut Records, (country, number): (u8, i16), by: i16) {
    if let Some(team) = u16::try_from(number).ok().and_then(|n| records.team_mut(country.into(), n)) {
        team.set_respect((team.respect() + by).clamp(0, RESPECT_MOST));
    }
}

fn club_of(slot: [u8; 4]) -> (u8, i16) {
    (slot[0], i16::from_le_bytes([slot[2], slot[3]]))
}

/// One more club for a computer host's Turnier on its day (`FUN_00771ed5`):
/// in five of ten a near club of the host's country free on both days, in
/// four one of its first division, in one a club abroad.
fn fill_one(records: &mut Records, calendar: &crate::calendar::Calendar, id: u32, random: &mut Random) {
    let Some(t) = find(records, id) else { return };
    let (country, number) = t.host();
    let day = t.day();
    let free = |records: &Records, club: (u8, i16)| {
        !crate::statistics::is_human(records, club.0, club.1)
            && !crate::friendly::busy(records, calendar, club, day)
            && !crate::friendly::busy(records, calendar, club, day + 1)
    };
    let r = random.below(10);
    let picked = if r < 5 {
        let mut near = Vec::new();
        for c in 0..records.countries.team_count(country.into()) {
            if c == number || !crate::friendly::near(records, calendar, (country, number), (country, c)) {
                continue;
            }
            if crate::statistics::is_human(records, country, c) {
                continue;
            }
            if !free(records, (country, c)) || near.len() >= NEAR_MOST {
                continue;
            }
            near.push(c);
        }
        (!near.is_empty()).then(|| (country, near[random.below(near.len() as u32) as usize]))
    } else if r < 9 {
        let first: Vec<i16> = crate::opponent::League::of(records, country.into(), 0)
            .map(|league| (0..league.teams()).map(|place| league.team(place)).collect())
            .unwrap_or_default();
        (!first.is_empty())
            .then(|| (country, first[random.below(first.len() as u32) as usize]))
            .filter(|&club| free(records, club))
    } else {
        crate::friendly::club_abroad(records, calendar, country, random).filter(|&club| free(records, club))
    };
    if let Some(club) = picked {
        let mut list = tournaments(records);
        if let Some(t) = list.iter_mut().find(|t| t.id() == id) {
            t.add(club);
        }
        store(records, &list);
    }
}

/// The tournaments' games of the day (`FUN_006257cd`): a Turnier on its
/// first day filled by its computer host (`FUN_006261b5`), its guests'
/// fees booked and its semifinals set — the second slot against the third,
/// the host against the fourth (`FUN_00625ac8` with 0); on its second day
/// the game for third place and the final. A Turnier short of clubs plays
/// nothing.
pub fn day_games(records: &mut Records, calendar: &crate::calendar::Calendar, today: i16, random: &mut Random) -> Vec<Fixture> {
    let mut games = Vec::new();
    let ids: Vec<(u32, i16)> =
        tournaments(records).iter().filter(|t| t.kind() == TURNIER).map(|t| (t.id(), t.day())).collect();
    for (id, day) in ids {
        if day == today {
            let host = find(records, id).map(|t| t.host());
            if host.is_some_and(|(c, n)| !crate::statistics::is_human(records, c, n)) {
                for _ in 0..FILL_TRIES {
                    if find(records, id).is_none_or(|t| t.count() >= NEEDED[0]) {
                        break;
                    }
                    fill_one(records, calendar, id, random);
                }
            }
            let Some(t) = find(records, id) else { continue };
            if still(records, today) || !t.full() {
                continue;
            }
            let host = t.host();
            let guests = [club_of(t.slot(1)), club_of(t.slot(2)), club_of(t.slot(3))];
            let mut total = 0;
            for guest in guests {
                let fee = fee(records, host, guest);
                book(records, guest, OTHER_INCOME, fee);
                total += fee;
            }
            book(records, host, OTHER_COSTS, total);
            let mut list = tournaments(records);
            if let Some(e) = list.iter_mut().find(|e| e.id() == id) {
                e.set_int(FEES, total);
                let (a, b, c, d) = (e.slot(0), e.slot(1), e.slot(2), e.slot(3));
                e.set_sides(SEMI_ONE, b, c);
                e.set_sides(SEMI_TWO, a, d);
            }
            store(records, &list);
            for game in [SEMI_ONE, SEMI_TWO] {
                let [home, away] = find(records, id).map(|t| t.sides(game)).unwrap_or_default();
                games.push(Fixture { id, game, home, away, ground: host });
            }
        } else if day == today - 1 {
            let Some(t) = find(records, id) else { continue };
            if still(records, today - 1) || !t.full() {
                continue;
            }
            for game in [THIRD, FINAL] {
                let [home, away] = t.sides(game);
                games.push(Fixture { id, game, home, away, ground: t.host() });
            }
        }
    }
    games
}

/// A tournament game played (`FUN_00625d73`): its result kept, the gate
/// booked to the host and counted with the crowd.
pub fn record(records: &mut Records, fixture: &Fixture, outcome: &Outcome) {
    book(records, fixture.ground, GATE, outcome.gate);
    let mut list = tournaments(records);
    if let Some(e) = list.iter_mut().find(|e| e.id() == fixture.id) {
        let g = e.game_mut(fixture.game);
        g[GAME_GOALS] = outcome.goals.0;
        g[GAME_GOALS + 1] = outcome.goals.1;
        g[GAME_FLAGS] = PLAYED | if outcome.extra { EXTRA } else { 0 } | if outcome.penalties { PENALTIES } else { 0 };
        g[GAME_WINNER] = outcome.winner;
        e.set_int(GATES, e.int(GATES).wrapping_add(outcome.gate));
        e.set_int(CROWDS, e.int(CROWDS).wrapping_add(outcome.crowd));
    }
    store(records, &list);
}

/// The tournaments' evening (`FUN_006258eb`'s `FUN_00625ed6`): after a
/// Turnier's first day the semifinals' winners meet in the final and the
/// losers for third place, the host at home where it plays; after its
/// second its first three take their prize money and the winner respect,
/// the third and the fourth lose some, the host pays its costs. The
/// Turniere whose humans should see them, by id.
pub fn evening(records: &mut Records, today: i16) -> Vec<u32> {
    let mut shown = Vec::new();
    let ids: Vec<(u32, i16)> = tournaments(records)
        .iter()
        .filter(|t| t.kind() == TURNIER && t.full() && (t.day() == today || t.day() == today - 1))
        .map(|t| (t.id(), t.day()))
        .collect();
    for (id, day) in ids {
        let mut list = tournaments(records);
        let Some(e) = list.iter_mut().find(|e| e.id() == id) else { continue };
        let host = e.slot(0);
        if day == today {
            let (w0, l0) = e.winner_loser(SEMI_ONE);
            let (w1, l1) = e.winner_loser(SEMI_TWO);
            e.set_sides(FINAL, w0, w1);
            e.set_sides(THIRD, l0, l1);
            let is_host = |s: [u8; 4]| club_of(s) == club_of(host);
            if is_host(l1) {
                e.set_sides(THIRD, l1, l0);
            } else if is_host(w1) {
                e.set_sides(FINAL, w1, w0);
            }
            store(records, &list);
        } else {
            e.set_int(COSTS, HOST_COSTS);
            let (first, second) = e.winner_loser(FINAL);
            let (third, fourth) = e.winner_loser(THIRD);
            let played = e.result(FINAL).is_some();
            store(records, &list);
            if played {
                book(records, club_of(first), PLAN_PREMIUMS, PRIZES[0]);
                respect(records, club_of(first), RESPECT_WINNER);
                book(records, club_of(second), PLAN_PREMIUMS, PRIZES[1]);
                book(records, club_of(third), PLAN_PREMIUMS, PRIZES[2]);
                respect(records, club_of(third), RESPECT_THIRD);
                respect(records, club_of(fourth), RESPECT_FOURTH);
            }
            book(records, club_of(host), OTHER_COSTS, HOST_COSTS);
        }
        if find(records, id).is_some_and(|t| t.clubs().iter().take(4).any(|&(c, n)| crate::statistics::is_human(records, c, n))) {
            shown.push(id);
        }
    }
    shown
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: u8, host: (u8, i16), day: i16) -> Tournament {
        let mut t = Tournament([0; ENTRY]);
        t.0[KIND] = kind;
        t.0[ID..ID + 4].copy_from_slice(&id(host, day).to_le_bytes());
        t.set_word(DAY, day);
        assert!(t.add(host));
        t
    }

    /// A Turnier takes four clubs, a hall's and a beach's eight; the host
    /// stays first, the others in the order they came, and one more than
    /// the room is refused (`FUN_0076006a`).
    #[test]
    fn a_tournament_takes_its_kinds_clubs_and_no_more() {
        for (kind, room) in [(TURNIER, 4), (HALLE, 8), (STRAND, 8)] {
            let mut t = entry(kind, (14, 82), 37);
            for k in 1..room {
                assert!(!t.full());
                assert!(t.add((14, k as i16)));
            }
            assert!(t.full());
            assert!(!t.add((14, 99)));
            assert_eq!(t.count(), room);
            assert_eq!(t.host(), (14, 82));
            assert_eq!(t.clubs()[1..], (1..room).map(|k| (14, k as i16)).collect::<Vec<_>>()[..]);
            assert!(t.has((14, 1)) && !t.has((14, 99)));
            assert_eq!(needed(kind), room);
        }
    }

    /// The id holds the host and the day (`FUN_00760e8d`): Ulm's of the 37th
    /// is 0e520025, as the original wrote it.
    #[test]
    fn the_id_holds_the_host_and_the_day() {
        assert_eq!(id((14, 82), 37), 0x0e52_0025);
        let t = entry(TURNIER, (14, 82), 37);
        assert_eq!((t.id(), t.day(), t.kind()), (0x0e52_0025, 37, TURNIER));
    }

    /// The calendar shows both days, the first marked 1 and the second 2,
    /// with the host's slot — its second byte as the slot has it.
    #[test]
    fn the_calendar_shows_both_days_with_the_host() {
        let mut calendar = Planner::new(&vec![0; Planner::SIZE]).unwrap();
        let mut t = entry(HALLE, (14, 56), 172);
        t.0[SLOTS + 1] = 0xc7;
        mark(&mut calendar, &t);
        let (first, second) = (calendar.entry(172), calendar.entry(173));
        assert_eq!(first[..8], [3, 1, 0, 0, 14, 0xc7, 56, 0]);
        assert_eq!(second[..8], [3, 2, 0, 0, 14, 0xc7, 56, 0]);
        assert!(first[8..].iter().chain(&second[8..]).all(|&b| b == 0));
        assert_eq!(calendar.entry(174)[0], 0);
        assert!(!free(&calendar, 172) && !free(&calendar, 171) && free(&calendar, 174));
    }
}

#[cfg(test)]
mod saves {
    use super::*;
    use a3_files::an3::Save;
    use std::collections::BTreeMap;

    fn read(name: &str) -> Records {
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap()
    }

    /// The tournaments of a kind by day and host country, how many each.
    fn by_day(records: &Records, kind: u8) -> BTreeMap<(i16, u8), usize> {
        let mut days = BTreeMap::new();
        for t in tournaments(records).iter().filter(|t| t.kind() == kind) {
            *days.entry((t.day(), t.host().0)).or_default() += 1;
        }
        days
    }

    /// What every computer host keeps to: one tournament of the kind, no
    /// human's, its flag set, its slot's second byte the original's.
    fn hosts_keep_the_rules(records: &Records, kind: u8, flag: u8, stack: u8) {
        let mut seen = std::collections::BTreeSet::new();
        for t in tournaments(records).iter().filter(|t| t.kind() == kind) {
            let (country, number) = t.host();
            assert!(seen.insert((country, number)), "{country}/{number} hosts twice");
            assert!(!crate::statistics::is_human(records, country, number));
            let team = records.team(country.into(), number as u16).unwrap();
            assert_eq!(team.get::<u8>(HOSTING) & flag, flag, "{country}/{number}");
            assert_eq!(t.0[SLOTS + 1], stack);
            assert_eq!((t.count(), t.id()), (1, id((country, number), t.day())));
        }
    }

    /// The new season's Turniere (SeasonEnd364 → NewSeason000) fall on the
    /// original's days in its countries, the main country's below the
    /// second division before day 21. The hosts are drawn, and a thin
    /// country may find none in ten tries — the original's England did not
    /// on day 32 —, so some draw of the new season's state reproduces the
    /// original's days.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_new_season_holds_the_originals_turniere() {
        let (mut ours, theirs) = (read("SeasonEnd364.an3"), read("NewSeason000.an3"));
        crate::settlement::open_season(&mut ours, &mut Random::new(7));
        // Only the hosts' flags and the list are the pre-season's: taken
        // back, the state stands as the Turniere found it.
        let seed = (0..400).find(|&seed| {
            for team in &mut ours.teams {
                team.put::<u8>(HOSTING, team.get::<u8>(HOSTING) & !HOSTS_TURNIER);
            }
            store(&mut ours, &[]);
            open_preseason(&mut ours, &mut Random::new(seed));
            by_day(&ours, TURNIER) == by_day(&theirs, TURNIER)
        });
        assert!(seed.is_some(), "no draw holds the original's days");
        hosts_keep_the_rules(&ours, TURNIER, HOSTS_TURNIER, TURNIER_STACK);
        hosts_keep_the_rules(&theirs, TURNIER, HOSTS_TURNIER, TURNIER_STACK);
        let main = ours.globals.main_country() as u8;
        for t in tournaments(&ours).iter().filter(|t| t.day() < LOWER_UNTIL && t.host().0 == main) {
            assert!(ours.team(main.into(), t.host().1 as u16).unwrap().level() >= LOWER);
        }
        assert_eq!(tournaments(&ours).len(), tournaments(&theirs).len());
    }

    /// Probe1's Turniere of days 35 to 39 played through (days 35 to 40):
    /// each filled to its four, its four games played, the final between
    /// the semifinals' winners and the game for third between their losers;
    /// every club's money moved by what its Turniere owe it — the fees,
    /// the host's costs and gates, the prize money.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_preseason_turniere_are_played_through() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let data = crate::data::Data::load(&dir).unwrap();
        let mut records = read("Probe1.an3");
        let money = |r: &Records, c: (u8, i16), at: usize| {
            r.team(c.0.into(), c.1 as u16).map_or(0, |t| t.get::<i32>(at + crate::finances::CLUB_TO_TEAM))
        };
        let standing = |r: &Records, c: (u8, i16)| r.team(c.0.into(), c.1 as u16).map_or(0, |t| t.respect());
        let clubs: Vec<(u8, i16)> = (0..records.countries.team_count(14)).map(|n| (14, n)).collect();
        let before: Vec<_> = clubs
            .iter()
            .map(|&c| (money(&records, c, OTHER_COSTS), money(&records, c, OTHER_INCOME), money(&records, c, GATE), money(&records, c, PLAN_PREMIUMS), standing(&records, c)))
            .collect();
        let mut random = Random::new(5);
        for day in 35..=40 {
            crate::exhibition::play_day(&mut records, &data.calendar, &data.presale, day, &mut random);
        }
        let played: Vec<Tournament> =
            tournaments(&records).into_iter().filter(|t| t.kind() == TURNIER && (35..=39).contains(&t.day())).collect();
        assert_eq!(played.len(), 7);
        // What each club is owed: costs, income, gates, premiums, respect.
        type Owed = (i32, i32, i32, i32, i16);
        let mut owed: std::collections::BTreeMap<(u8, i16), Owed> = Default::default();
        for t in &played {
            assert!(t.full(), "{:08x}", t.id());
            assert!((0..4).all(|g| t.result(g).is_some()));
            let (w0, l0) = t.winner_loser(SEMI_ONE);
            let (w1, l1) = t.winner_loser(SEMI_TWO);
            let set = |g: usize| { let mut s = t.sides(g); s.sort(); s };
            let mut finalists = [club_of(w0), club_of(w1)];
            finalists.sort();
            let mut thirds = [club_of(l0), club_of(l1)];
            thirds.sort();
            assert_eq!((set(FINAL), set(THIRD)), (finalists, thirds));
            let (crowds, gates, fees, costs) = t.takings();
            assert!(crowds > 0 && costs == HOST_COSTS);
            let host = t.host();
            owed.entry(host).or_default().0 += fees + HOST_COSTS;
            owed.entry(host).or_default().2 += gates;
            for k in 1..4 {
                owed.entry(club_of(t.slot(k))).or_default().1 += fee(&records, host, club_of(t.slot(k)));
            }
            let (first, second) = t.winner_loser(FINAL);
            let (third, fourth) = t.winner_loser(THIRD);
            for (club, prize, by) in [(first, 40_000, 5), (second, 20_000, 0), (third, 10_000, -3), (fourth, 0, -8)] {
                let o = owed.entry(club_of(club)).or_default();
                o.3 += prize;
                o.4 += by;
            }
        }
        for (k, &club) in clubs.iter().enumerate() {
            let o = owed.get(&club).copied().unwrap_or_default();
            let (costs, income, gate, premiums, held) = before[k];
            assert_eq!(money(&records, club, OTHER_COSTS) - costs, o.0, "{club:?} costs");
            assert_eq!(money(&records, club, OTHER_INCOME) - income, o.1, "{club:?} income");
            assert_eq!(money(&records, club, GATE) - gate, o.2, "{club:?} gate");
            assert_eq!(money(&records, club, PLAN_PREMIUMS) - premiums, o.3, "{club:?} premiums");
            // Respect moves by every result as well (`FUN_00714236`).
            let _ = (held, o.4);
        }
    }

    /// The winter's halls, Germany's at half week 48 and France's at 52,
    /// fall on the days the original's season S2c holds them, one or two a
    /// day.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_winter_holds_the_originals_halls() {
        let (mut ours, theirs) = (read("A.an3"), read("S2c.an3"));
        let mut random = Random::new(3);
        open_winter(&mut ours, GERMANY, GERMANY_HALLS, &mut random);
        open_winter(&mut ours, FRANCE, FRANCE_HALLS, &mut random);
        assert_eq!(by_day(&ours, HALLE), by_day(&theirs, HALLE));
        hosts_keep_the_rules(&ours, HALLE, HOSTS_HALL, HALL_STACK);
        hosts_keep_the_rules(&theirs, HALLE, HOSTS_HALL, HALL_STACK);
    }
}
