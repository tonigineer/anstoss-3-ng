//! Friendlies (JAHRESKALENDER.md §4.3 in the RE repo): the opponents the
//! game offers a club for a day of its year calendar, and the booking of
//! one.
//!
//! The offer is drawn at random (`FUN_004343fd`) and kept for the turn in
//! a cache of thirty lists, by club and day (`CDatenGegnerliste`, which no
//! save holds), so the same day shows the same clubs until the next turn.
//! A list takes three clubs from nearby, six by division with a chance of
//! one abroad among them, and fills up by division to ten. A booking goes
//! into the friendly list (`CDatenFreundschaftsspielliste`, `0x1c51940`)
//! and the pair into the last friendlies (`0x8eabe8`), which keep two
//! clubs apart for six weeks.

use crate::calendar::{Calendar, DAYS};
use crate::random::Random;
use crate::records::{PLANNER_ENTRY, Records};
use crate::year::FRIENDLY;

/// How many lists the cache keeps, and clubs a list offers.
const SLOTS: usize = 30;
pub const CANDIDATES: usize = 10;
/// The tries from nearby, the rounds by division with their divisions
/// (`DAT_008786f0`'s first row, the only one read), the rounds to fill up.
const NEARBY_TRIES: usize = 3;
const ROUNDS: [i16; 6] = [1, 1, 2, 2, 3, 3];
const FILL_ROUNDS: usize = 10;
/// The nearby clubs: at most twenty, less than fifty apart on the map.
const NEARBY: usize = 20;
const NEAR: i32 = 50;
/// The chance of a club abroad in a round, by the club's division (the
/// first, the second, below), and what each one taken costs it.
const ABROAD_CHANCE: [i16; 3] = [100, 50, 30];
const ABROAD_STEP: i16 = 40;
/// The eleven countries clubs abroad may come from besides the played
/// ones' first divisions (`DAT_00ae5d81`).
const ABROAD_LIST: usize = 0xae5d81;
const ABROAD_COUNTRIES: usize = 11;
/// The level of a club outside the leagues.
const OUTSIDE: i16 = 10;

/// The saved lists: the friendlies booked (three hundred of 0x1c bytes
/// after the count) and the last friendlies (fifty pairs of ten bytes),
/// which keep a pair apart for this many days.
const LIST_AT: &str = "0x1c51940";
const LIST_ENTRIES: usize = 4;
const LIST_ENTRY: usize = 0x1c;
const LIST_MOST: usize = 300;
const LAST_AT: &str = "0x8eabe8";
const LAST_ENTRIES: usize = 2;
const LAST_ENTRY: usize = 10;
const LAST_MOST: usize = 50;
const LAST_KEPT: i32 = 42;
/// The tournaments (`0x1c4be78`): their day at +0xa, their clubs counted
/// at +8, the clubs four bytes each from +0xc.
const TOURNAMENTS_AT: &str = "0x1c4be78";
const TOURNAMENT_ENTRIES: usize = 4;
const TOURNAMENT: usize = 0x74;
const TOURNAMENTS_MOST: usize = 200;
/// The European Supercup's pairing (`0x8e83a2`).
const SUPERCUP_AT: &str = "0x8e7c88";
const SUPERCUP_PAIRING: usize = 0x6fa;
/// The clubs of Germany's league cup (`DAT_00b5dfba`), six numbers.
const LEAGUE_CUP_CLUBS: usize = 0xb5dfba;
const GERMANY: u8 = 14;
/// The human clubs (`DAT_00b5cbe2`, eight bytes each, the manager's number
/// at +6) and their count.
const HUMANS: usize = 0xb5cbe2;
const HUMAN_COUNT: usize = 0xb5cc02;

/// The day codes' kinds in their top bits: the cup, a supercup or league
/// cup, the European cups (UEFA and Champions League) and the European
/// Supercup.
const KIND: u32 = 0x7f00_0000;
const CUP: u32 = 0x4000_0000;
const SUPERCUP: u32 = 0x3000_0000;
const UEFA: u32 = 0x2000_0000;
const CHAMPIONS: u32 = 0x5000_0000;
const EUROPEAN_SUPERCUP: u32 = 0x6000_0000;
const LAST_MATCHDAY: u32 = 59;
/// The team's flags (+0x110): champion, in the cup, in the UEFA Cup, in
/// the Champions League, the supercups' others.
const FLAGS: usize = 0x110;
const CHAMPION: u16 = 0x1;
const IN_CUP: u16 = 0x8000;
const IN_UEFA: u16 = 0x20;
const IN_CHAMPIONS: u16 = 0x4000;
const SUPERCUP_FLAGS: u16 = 0x1 | 0x1000 | 0x2000;

/// An offered club as the cache keeps it (eight bytes): its country and
/// number, whether the asking club plays away (`+4`), its class from 1
/// (`+6`), and the fee in thousands (`+7`, 0 sharing the gate). A try that
/// fails leaves what it wrote, and the fee is not always written, so a
/// candidate can carry a fee an earlier try left at its place.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Candidate {
    pub country: u8,
    pub number: i16,
    pub away: i16,
    pub class: u8,
    pub fee: u8,
}

impl Candidate {
    pub fn club(&self) -> (u8, i16) {
        (self.country, self.number)
    }
}

#[derive(Clone, Copy, Default, PartialEq)]
struct Slot {
    used: bool,
    day: i16,
    club: (u8, i16),
    count: usize,
    candidates: [Candidate; CANDIDATES],
}

/// The cache of the turn's offers (`CDatenGegnerliste`, 0x8eae00).
#[derive(Clone, Default, PartialEq)]
pub struct Cache {
    slots: [Slot; SLOTS],
}

impl Cache {
    /// Empties it, as each turn start does (`FUN_0064e40f`) unless a game
    /// was just loaded.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// The clubs offered to `club` for day `day` (`FUN_00434722`,
    /// `FUN_0043477e`): the turn's list if there is one, else a new one
    /// drawn into the last free slot, or the last slot when none is free.
    pub fn offer(
        &mut self,
        records: &Records,
        calendar: &Calendar,
        club: (u8, i16),
        day: i16,
        random: &mut Random,
    ) -> Vec<Candidate> {
        let mut free = None;
        for (s, slot) in self.slots.iter().enumerate() {
            if !slot.used {
                free = Some(s);
            } else if slot.club == club && slot.day == day {
                return slot.candidates[..slot.count].to_vec();
            }
        }
        let slot = &mut self.slots[free.unwrap_or(SLOTS - 1)];
        let world = World { records, calendar, week: records.globals.get(0xb5cbd6) };
        fill(slot, &world, club, day, random);
        slot.candidates[..slot.count].to_vec()
    }
}

/// What the draws read.
struct World<'a> {
    records: &'a Records,
    calendar: &'a Calendar,
    /// The week's first day (`DAT_00b5cbd6`).
    week: i16,
}

impl World<'_> {
    fn kind(&self, country: u8) -> u8 {
        self.records.countries.kind(country.into())
    }

    fn clubs(&self, country: u8) -> i16 {
        self.records.countries.team_count(country.into())
    }

    /// The team of a club of the countries that keep teams.
    fn team(&self, (country, number): (u8, i16)) -> Option<&crate::records::Team> {
        let n = u16::try_from(number).ok().filter(|_| number < self.clubs(country))?;
        matches!(self.kind(country), 1 | 2).then(|| self.records.team(country.into(), n)).flatten()
    }

    /// A club's division (`FUN_0063e2df`): 10 outside the leagues, 0 for
    /// the countries kept in short.
    fn level(&self, club: (u8, i16)) -> i16 {
        match self.kind(club.0) {
            1 | 2 => self.team(club).map_or(OUTSIDE, |t| i16::from(t.level())),
            _ => 0,
        }
    }

    /// The team's flags (`FUN_0063e033`); none for a club without a team.
    fn flags(&self, club: (u8, i16)) -> u16 {
        self.team(club).map_or(0, |t| t.get::<u16>(FLAGS))
    }

    /// Whether the club won its league (`FUN_0069a161`).
    fn champion(&self, club: (u8, i16)) -> bool {
        self.flags(club) & CHAMPION != 0
    }

    /// A human club (`FUN_00677428`).
    fn human(&self, (country, number): (u8, i16)) -> bool {
        crate::statistics::is_human(self.records, country, number)
    }

    /// Where a played country's club lies on the map (`FUN_0063e837`).
    fn at(&self, (country, number): (u8, i16)) -> (i32, i32) {
        if !self.records.globals.played(country.into()) || number >= self.clubs(country) {
            return (0, 0);
        }
        self.team((country, number)).map_or((0, 0), |t| {
            let [x, y] = t.map_at();
            (i32::from(x), i32::from(y))
        })
    }

    /// Whether two clubs lie less than fifty apart (`FUN_0067792f`, whose
    /// truncated root is below 50 just when the square is below 2500).
    fn near(&self, a: (u8, i16), b: (u8, i16)) -> bool {
        let ((x1, y1), (x2, y2)) = (self.at(a), self.at(b));
        (x1 - x2).pow(2) + (y1 - y2).pow(2) < NEAR * NEAR
    }

    /// Whether the pair played or booked a friendly lately
    /// (`FUN_0043563e`).
    fn lately(&self, a: (u8, i16), b: (u8, i16)) -> bool {
        let (lo, hi) = ordered(a, b);
        last_friendlies(self.records).iter().any(|&(_, x, y)| x == lo && y == hi)
    }

    /// Whether the club has a match near the day (`FUN_006777a8`): the
    /// league the day before, on it or after, the cup or a supercup on
    /// them, the European cups, a friendly on the day, a tournament the day
    /// before, on it or after.
    fn busy(&self, club: (u8, i16), day: i16) -> bool {
        if day >= 42 && ((day - 1..=day + 1).any(|t| self.league_on(club, t)) || self.cup_near(club, day)) {
            return true;
        }
        self.european_near(club, day) || friendly_on(self.records, club, day) || self.tournament_near(club, day)
    }

    /// A league match (`FUN_0062d73b`).
    fn league_on(&self, club: (u8, i16), t: i16) -> bool {
        let level = self.level(club);
        if !(40..DAYS as i16).contains(&t) || level >= OUTSIDE || self.kind(club.0) != 1 {
            return false;
        }
        let Some(row) = self.calendar.row(club.0.into(), level as usize) else { return false };
        (1..=LAST_MATCHDAY).contains(&self.calendar.entry(row, t as usize))
    }

    /// A cup tie or a supercup the day before, on it or after
    /// (`FUN_0062d86b`); past the season's last day nothing.
    fn cup_near(&self, club: (u8, i16), day: i16) -> bool {
        for t in day - 1..=day + 1 {
            if t > 11 {
                if t >= DAYS as i16 {
                    return false;
                }
                if self.cup_on(club, t) {
                    return true;
                }
            }
        }
        false
    }

    /// A cup tie or a supercup on a day (`FUN_0062d9b3`): the cup's while
    /// the club is in it; Germany's league cup for its six clubs, elsewhere
    /// the supercups for the champions and the cups' winners.
    fn cup_on(&self, club: (u8, i16), t: i16) -> bool {
        if self.kind(club.0) != 1 || !(12..DAYS as i16).contains(&t) {
            return false;
        }
        let Some(row) = self.calendar.row(club.0.into(), 0) else { return false };
        let flags = self.flags(club);
        match self.calendar.entry(row, t as usize) & KIND {
            CUP => flags & IN_CUP != 0,
            SUPERCUP if club.0 == GERMANY => {
                let number = club.1 as u8;
                (0..6).any(|k| self.records.globals.get::<u8>(LEAGUE_CUP_CLUBS + k) == number)
            }
            SUPERCUP => flags & SUPERCUP_FLAGS != 0,
            _ => false,
        }
    }

    /// A European match the day before, on it or after (`FUN_0062dac7`);
    /// past the season's last day nothing.
    fn european_near(&self, (country, number): (u8, i16), day: i16) -> bool {
        let club = (country, i16::from(number as u8));
        for t in day - 1..=day + 1 {
            if t < 0 {
                continue;
            }
            if t >= DAYS as i16 {
                return false;
            }
            if self.european_on(club, t) {
                return true;
            }
        }
        false
    }

    /// A European match on a day (`FUN_0062db4a`): the UEFA Cup's and the
    /// Champions League's for their clubs, the European Supercup's two.
    fn european_on(&self, club: (u8, i16), t: i16) -> bool {
        match self.calendar.entry(0, t as usize) & KIND {
            UEFA => self.flags(club) & IN_UEFA != 0,
            CHAMPIONS => self.flags(club) & IN_CHAMPIONS != 0,
            EUROPEAN_SUPERCUP => self.in_european_supercup(club),
            _ => false,
        }
    }

    /// One of the European Supercup's two clubs (`FUN_0041988c`).
    fn in_european_supercup(&self, club: (u8, i16)) -> bool {
        let Some(p) = self.records.block(SUPERCUP_AT).and_then(|b| b.get(SUPERCUP_PAIRING..SUPERCUP_PAIRING + 8))
        else {
            return false;
        };
        let word = |at: usize| i16::from_le_bytes([p[at], p[at + 1]]);
        (p[0], word(2)) == club || (p[4], word(6)) == club
    }

    /// A tournament the club plays in the day before, on it or after
    /// (`FUN_0075ef68`).
    fn tournament_near(&self, club: (u8, i16), day: i16) -> bool {
        let Some(block) = self.records.block(TOURNAMENTS_AT) else { return false };
        let word = |b: &[u8], at: usize| i16::from_le_bytes([b[at], b[at + 1]]);
        let count = usize::try_from(word(block, 0)).unwrap_or(0).min(TOURNAMENTS_MOST);
        (0..count)
            .filter_map(|k| block.get(TOURNAMENT_ENTRIES + k * TOURNAMENT..TOURNAMENT_ENTRIES + (k + 1) * TOURNAMENT))
            .filter(|e| (i32::from(word(e, 0xa)) - i32::from(day)).abs() <= 1)
            .any(|e| {
                let clubs = usize::try_from(word(e, 8)).unwrap_or(0).min(8);
                (0..clubs).any(|j| (e[0xc + 4 * j], word(e, 0xe + 4 * j)) == club)
            })
    }
}

/// Whether a human club has a match on the day itself, or a tournament
/// the day before or after (`FUN_0062ae6a`): the dialog offers asking only
/// the other humans free that day.
pub fn taken(records: &Records, calendar: &Calendar, club: (u8, i16), day: i16) -> bool {
    let world = World { records, calendar, week: 0 };
    world.league_on(club, day)
        || world.cup_on(club, day)
        || (0..DAYS as i16).contains(&day) && world.european_on(club, day)
        || world.tournament_near(club, day)
        || friendly_on(records, club, day)
}

/// A club's division as the offer shows it (`FUN_0063e2df`): 10 outside
/// the leagues, 0 for the countries kept in short.
pub fn division(records: &Records, calendar: &Calendar, club: (u8, i16)) -> i16 {
    World { records, calendar, week: 0 }.level(club)
}

/// The asking club, as its club object has it.
struct Own {
    club: (u8, i16),
    level: i16,
    champion: bool,
}

/// Draws a new list into a slot (`FUN_004343fd`).
fn fill(slot: &mut Slot, world: &World, club: (u8, i16), day: i16, random: &mut Random) {
    slot.used = true;
    slot.day = day;
    slot.club = club;
    slot.count = 0;
    let own = Own { club, level: world.level(club), champion: world.champion(club) };
    let mut chance = ABROAD_CHANCE[own.level.clamp(0, 2) as usize];
    let add = |slot: &mut Slot, found: bool| {
        let new = slot.candidates[slot.count];
        if found && !slot.candidates[..slot.count].iter().any(|c| c.club() == new.club()) {
            slot.count += 1;
        }
    };
    for _ in 0..NEARBY_TRIES {
        let found = nearby(world, day, &mut slot.candidates[slot.count], &own, random);
        add(slot, found);
    }
    for k in ROUNDS {
        let found = if random.below(10) == 0 && (random.below(100) as i16) < chance {
            chance -= ABROAD_STEP;
            abroad(world, day, &mut slot.candidates[slot.count], &own, random)
        } else {
            by_division(world, day, &mut slot.candidates[slot.count], &own, k, random)
        };
        add(slot, found);
    }
    for _ in 0..FILL_ROUNDS {
        if slot.count >= CANDIDATES {
            break;
        }
        let k = random.below(3) as i16;
        let found = by_division(world, day, &mut slot.candidates[slot.count], &own, k, random);
        add(slot, found);
    }
}

/// A club of the own country lying near (`FUN_004349f5`): among the first
/// twenty near that have not met the club lately, one at random — which
/// may be the club itself or another human's, and then nothing is found.
/// It checks the day the week's first day later than asked, as all three
/// kinds of draws do.
fn nearby(world: &World, day: i16, out: &mut Candidate, own: &Own, random: &mut Random) -> bool {
    let country = own.club.0;
    let mut near = Vec::with_capacity(NEARBY);
    for i in 0..world.clubs(country) {
        let other = (country, i);
        if world.lately(own.club, other) || !world.near(own.club, other) {
            continue;
        }
        near.push(i);
        if near.len() >= NEARBY {
            break;
        }
    }
    if near.is_empty() {
        return false;
    }
    let picked = (country, near[random.below(near.len() as u32) as usize]);
    if world.human(picked) || world.busy(picked, world.week.wrapping_add(day)) {
        return false;
    }
    (out.country, out.number) = picked;
    out.class = world.level(picked).clamp(0, 2) as u8 + 1;
    terms(world, out, own, random);
    true
}

/// A club of the own country of division `k` (0 the first, 1 the second,
/// 2 and 3 any below, as classes 3 and 4) that has not met the club lately
/// (`FUN_00434bd3`).
fn by_division(world: &World, day: i16, out: &mut Candidate, own: &Own, k: i16, random: &mut Random) -> bool {
    out.country = 0;
    out.number = 0;
    let country = own.club.0;
    let fits = |level: i16| if k < 2 { level == k } else { level >= 2 };
    let clubs: Vec<i16> = (0..world.clubs(country))
        .filter(|&i| fits(world.level((country, i))) && !world.lately(own.club, (country, i)))
        .collect();
    if clubs.is_empty() {
        return false;
    }
    let picked = (country, clubs[random.below(clubs.len() as u32) as usize]);
    if world.human(picked) || world.busy(picked, world.week.wrapping_add(day)) {
        return false;
    }
    (out.country, out.number) = picked;
    out.class = (k + 1) as u8;
    terms(world, out, own, random);
    true
}

/// A club abroad (`FUN_00434e6b`), always of class 3: home or away at
/// random, for 80 to 120 thousand, 25 more when the club itself is a
/// champion and plays away, 50 more when the one abroad is a champion and
/// comes.
fn abroad(world: &World, day: i16, out: &mut Candidate, own: &Own, random: &mut Random) -> bool {
    out.country = 0;
    out.number = 0;
    let Some(picked) = abroad_pick(world, own.club.0, random) else { return false };
    if world.busy(picked, world.week.wrapping_add(day)) {
        return false;
    }
    (out.country, out.number) = picked;
    out.class = 3;
    out.away = if random.below(2) != 0 { 0 } else { 1 };
    out.fee = random.below(5) as u8 * 10 + 80;
    if out.away != 0 {
        if own.champion {
            out.fee += 25;
        }
    } else if world.champion(picked) {
        out.fee += 50;
    }
    true
}

/// The clubs abroad (`FUN_00434ff9`): the other played countries' first
/// divisions but the humans', then every club of the eleven countries of
/// `DAT_00ae5d81` but the own country's, unsifted — so a club of both
/// lists stands in it twice. The original sifts the first part also by
/// the last friendlies, but of a pair it never sets; that sifts nothing.
fn abroad_pick(world: &World, own: u8, random: &mut Random) -> Option<(u8, i16)> {
    let globals = &world.records.globals;
    let mut clubs = Vec::new();
    let tops = |country: u8, clubs: &mut Vec<(u8, i16)>| {
        for i in 0..world.clubs(country) {
            if world.level((country, i)) == 0 && !world.human((country, i)) {
                clubs.push((country, i));
            }
        }
    };
    let main = globals.main_country() as u8;
    if main != own {
        tops(main, &mut clubs);
    }
    let more = usize::try_from(globals.more_count()).unwrap_or(0);
    for &country in globals.more_countries().iter().take(more) {
        if country as u8 != own {
            tops(country as u8, &mut clubs);
        }
    }
    for j in 0..ABROAD_COUNTRIES {
        let country = world.records.countries.get::<u8>(ABROAD_LIST + j);
        if country != own {
            clubs.extend((0..world.clubs(country)).map(|i| (country, i)));
        }
    }
    (!clubs.is_empty()).then(|| clubs[random.below(clubs.len() as u32) as usize])
}

/// Where the match is played and what it costs (`FUN_0075f0ca`): a club
/// of a better class comes to a lower one, for 20 or 30 thousand from the
/// second class (or the fee left before), 40 to 60 from the others; equal
/// classes toss for the ground and share the gate; near clubs play for the
/// gate only; a champion asks 25 more away and 50 more at home.
fn terms(world: &World, out: &mut Candidate, own: &Own, random: &mut Random) {
    let class = i16::from(out.class) - 1;
    let level = own.level.min(2);
    let fee = |out: &mut Candidate, random: &mut Random| {
        if class == 1 {
            if random.below(2) != 0 {
                out.fee = random.below(2) as u8 * 10 + 20;
            }
        } else {
            out.fee = random.below(3) as u8 * 10 + 40;
        }
    };
    if level < class {
        out.away = 1;
        fee(out, random);
    } else if level == class {
        out.away = i16::from(random.below(2) as u8);
        out.fee = 0;
    } else {
        out.away = 0;
        fee(out, random);
    }
    if world.near(own.club, out.club()) {
        out.fee = 0;
    }
    if out.fee != 0 && own.champion {
        out.fee += if out.away != 0 { 25 } else { 50 };
    }
}

/// A pair in the order the last friendlies keep it, the lower country
/// and number first (`FUN_004354cd`).
fn ordered(a: (u8, i16), b: (u8, i16)) -> ((u8, i16), (u8, i16)) {
    if b.0 < a.0 || (b.0 == a.0 && b.1 < a.1) { (b, a) } else { (a, b) }
}

/// A last friendly: its day and its pair of clubs, each by country and
/// number, in their order.
pub type LastFriendly = (i16, (u8, i16), (u8, i16));

/// The last friendlies.
pub fn last_friendlies(records: &Records) -> Vec<LastFriendly> {
    let Some(block) = records.block(LAST_AT) else { return Vec::new() };
    let word = |at: usize| i16::from_le_bytes([block[at], block[at + 1]]);
    let count = usize::try_from(word(0)).unwrap_or(0).min(LAST_MOST);
    (0..count)
        .map(|k| LAST_ENTRIES + k * LAST_ENTRY)
        .filter(|&at| at + LAST_ENTRY <= block.len())
        .map(|at| (word(at), (block[at + 2], word(at + 4)), (block[at + 6], word(at + 8))))
        .collect()
}

fn set_last_friendlies(records: &mut Records, entries: &[LastFriendly]) {
    let Some(block) = records.block_mut(LAST_AT) else { return };
    let end = (LAST_ENTRIES + entries.len() * LAST_ENTRY).min(block.len());
    block[LAST_ENTRIES..end].fill(0);
    for (k, &(day, lo, hi)) in entries.iter().enumerate() {
        let at = LAST_ENTRIES + k * LAST_ENTRY;
        let Some(e) = block.get_mut(at..at + LAST_ENTRY) else { break };
        e[0..2].copy_from_slice(&day.to_le_bytes());
        e[2] = lo.0;
        e[4..6].copy_from_slice(&lo.1.to_le_bytes());
        e[6] = hi.0;
        e[8..10].copy_from_slice(&hi.1.to_le_bytes());
    }
    block[0..2].copy_from_slice(&(entries.len() as i16).to_le_bytes());
}

/// The game cycle forgets the last friendlies more than six weeks gone
/// (`FUN_0043539b`).
pub fn forget_last_friendlies(records: &mut Records, today: i16) {
    let mut entries = last_friendlies(records);
    let before = entries.len();
    entries.retain(|&(day, _, _)| i32::from(today) <= i32::from(day) + LAST_KEPT);
    if entries.len() != before {
        set_last_friendlies(records, &entries);
    }
}

/// A friendly booked: its id, the day, the booking club, its opponent,
/// the goals once played, whether the booking club plays away, the fee in
/// thousands.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Friendly {
    pub id: u32,
    pub day: i16,
    pub club: (u8, i16),
    pub opponent: (u8, i16),
    pub goals: (u8, u8),
    pub away: bool,
    pub fee: u32,
}

impl Friendly {
    fn read(e: &[u8]) -> Self {
        let word = |at: usize| i16::from_le_bytes([e[at], e[at + 1]]);
        Self {
            id: u32::from_le_bytes([e[0], e[1], e[2], e[3]]),
            day: word(4),
            club: (e[6], word(8)),
            opponent: (e[0xa], word(0xc)),
            goals: (e[0xe], e[0xf]),
            away: e[0x14] != 0,
            fee: u32::from_le_bytes([e[0x18], e[0x19], e[0x1a], e[0x1b]]),
        }
    }
}

/// The friendlies booked this season; played ones stay until the season
/// ends.
pub fn friendlies(records: &Records) -> Vec<Friendly> {
    let Some(block) = records.block(LIST_AT) else { return Vec::new() };
    let count = usize::try_from(i16::from_le_bytes([block[0], block[1]])).unwrap_or(0).min(LIST_MOST);
    (0..count)
        .filter_map(|k| block.get(LIST_ENTRIES + k * LIST_ENTRY..LIST_ENTRIES + (k + 1) * LIST_ENTRY))
        .map(Friendly::read)
        .collect()
}

/// Whether the club has a friendly booked on the day (`FUN_006250b9`).
fn friendly_on(records: &Records, club: (u8, i16), day: i16) -> bool {
    friendlies(records).iter().any(|f| f.day == day && (f.club == club || f.opponent == club))
}

/// A friendly's id (`FUN_0075feb8`): the club's country and number in the
/// top bytes, the day below.
pub fn id(club: (u8, i16), day: i16) -> u32 {
    u32::from(club.0) << 24 | u32::from(club.1 as u8) << 16 | u32::from(day as u16)
}

/// The manager of a human club, by number from 1 (`FUN_00677574`); 0 for
/// a computer club.
pub(crate) fn manager_of(records: &Records, (country, number): (u8, i16)) -> u8 {
    let count = usize::try_from(records.globals.get::<i16>(HUMAN_COUNT)).unwrap_or(0).min(4);
    (0..count)
        .map(|k| HUMANS + 8 * k)
        .find(|&at| records.globals.get::<u8>(at) == country && records.globals.get::<i16>(at + 2) == number)
        .map_or(0, |at| records.globals.get::<i16>(at + 6) as u8)
}

/// Books a friendly of `club`, whose manager has number `manager`, with
/// the candidate on `day` (`FUN_0075f812`): into the friendly list — of
/// three hundred at most —, the opponent into the club's calendar (and the
/// club into a human opponent's), the pair into the last friendlies.
/// Nothing when the list is full or a calendar's day is taken.
pub fn book(records: &mut Records, club: (u8, i16), manager: u8, candidate: &Candidate, day: i16) -> bool {
    let Some(block) = records.block(LIST_AT) else { return false };
    let count = usize::try_from(i16::from_le_bytes([block[0], block[1]])).unwrap_or(0);
    if count >= LIST_MOST || manager == 0 {
        return false;
    }
    let other = manager_of(records, candidate.club());
    let d = usize::try_from(day).unwrap_or(usize::MAX);
    let free = |records: &Records, m: u8| {
        records.calendars.get(usize::from(m) - 1).is_some_and(|c| c.entry(d)[0] == 0)
    };
    if !free(records, manager) || (other != 0 && !free(records, other)) {
        return false;
    }
    mark(records, manager, d, candidate.club());
    if other != 0 {
        mark(records, other, d, club);
    }
    let mut entry = [0u8; LIST_ENTRY];
    entry[0..4].copy_from_slice(&id(club, day).to_le_bytes());
    entry[4..6].copy_from_slice(&day.to_le_bytes());
    entry[6] = club.0;
    entry[8..10].copy_from_slice(&(club.1 as u16).to_le_bytes());
    entry[0xa] = candidate.country;
    entry[0xc..0xe].copy_from_slice(&(candidate.number as u16).to_le_bytes());
    entry[0x14] = candidate.away as u8;
    entry[0x18..0x1c].copy_from_slice(&u32::from(candidate.fee).to_le_bytes());
    let mut last = last_friendlies(records);
    if last.len() < LAST_MOST {
        let (lo, hi) = ordered(club, candidate.club());
        last.push((day, lo, hi));
        set_last_friendlies(records, &last);
    }
    let Some(block) = records.block_mut(LIST_AT) else { return false };
    let at = LIST_ENTRIES + count * LIST_ENTRY;
    block[at..at + LIST_ENTRY].copy_from_slice(&entry);
    block[0..2].copy_from_slice(&((count + 1) as i16).to_le_bytes());
    true
}

/// A friendly on a day of a manager's calendar (`FUN_0072e973` with kind
/// 5): the kind, set, the other club's country and number; the rest of the
/// day as it was.
fn mark(records: &mut Records, manager: u8, d: usize, (country, number): (u8, i16)) {
    let Some(calendar) = records.calendars.get_mut(usize::from(manager) - 1) else { return };
    let mut entry: [u8; PLANNER_ENTRY] = calendar.entry(d);
    entry[0] = FRIENDLY;
    entry[1] = 1;
    entry[4] = country;
    entry[6..8].copy_from_slice(&number.to_le_bytes());
    calendar.set_entry(d, entry);
}
