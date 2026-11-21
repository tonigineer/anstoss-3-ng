//! A new game's clubs, read from the countries' `Land*.sav` into the team
//! records (`FUN_00687e7c`'s team part, from `FUN_00686684`).
//!
//! Each `VEREIN` is read into a scratch club of 0x1b4 bytes and its
//! trainer and manager into two of 0xc (`FUN_0068cdfa`, `FUN_0068c06f`);
//! the team takes the club's values (`FUN_0068d411`), the trainer and the
//! manager the next two persons, and its players their ids
//! (`FUN_0068a38c`). The main country's clubs come first, every one of its
//! file; then eight of each country not played, in the order of the
//! eleven (`DAT_008a3820`).
//!
//! The names go into the pool as the clubs are filled, the president's,
//! the staff's, the players'; in which order the original takes them, with
//! the other sections of the files, is the pool's part.

use std::io;
use std::path::Path;

use a3_files::bitmap_font::to_cp1252;
use a3_files::sav::Section;

use crate::land_players::{foreign_clubs, squad, uefa_fixed, Pool};
use crate::random::Random;
use crate::records::{Person, Records, Team};
use crate::world::{find, LAND_FILES};

/// A date as `FUN_00402586` reads it: `atoi` at the day's, the month's and
/// the year's place of "dd.mm.yyyy".
fn date(text: &str) -> (u8, u8, u16) {
    let at = |i: usize| atoi(text.get(i..).unwrap_or(""));
    (at(0) as u8, at(3) as u8, at(6) as u16)
}

/// C's `atoi`: blanks, a sign, the digits up to the first other character.
fn atoi(text: &str) -> i32 {
    let text = text.trim_start();
    let (sign, digits) = match text.as_bytes().first() {
        Some(b'-') => (-1, &text[1..]),
        Some(b'+') => (1, &text[1..]),
        _ => (1, text),
    };
    let value = digits.bytes().take_while(u8::is_ascii_digit).fold(0i32, |n, d| n.wrapping_mul(10).wrapping_add(i32::from(d - b'0')));
    sign * value
}

/// Which of a club's people `FUN_0068c06f` reads (its third argument).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Trainer = 0,
    Manager = 1,
    CoTrainer = 2,
}

/// A trainer or manager as `FUN_0068c06f` reads him (0xc bytes).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Staff {
    /// +0, +2: first and last name.
    pub names: [String; 2],
    /// +4, +5: what he can as a trainer and as a manager; the one the
    /// file does not give is five less than the other, at least 1.
    pub trainer: i8,
    pub manager: i8,
    /// +6: a trainer's kind of training.
    pub kind: u8,
    /// +8: day, month, year.
    pub birthday: (u8, u8, u16),
}

impl Staff {
    /// `FUN_0068c06f`: the names, the ability the role gives (a trainer
    /// also his kind), a number the game drops, the birthday.
    pub fn read(section: &Section, role: Role) -> Self {
        let long = |i: usize| section.number(i).unwrap_or(0);
        let other = |own: i8| if own < 6 { 1 } else { own - 5 };
        let own = long(2) as i8;
        let (trainer, manager) = match role {
            Role::Manager => (other(own), own),
            _ => (own, other(own)),
        };
        let kind = if role == Role::Trainer { long(3) as u8 } else { 0 };
        let birthday = date(section.text(if role == Role::Trainer { 5 } else { 4 }).unwrap_or(""));
        let name = |i: usize| section.text(i).unwrap_or_default().to_owned();
        Self { names: [name(0), name(1)], trainer, manager, kind, birthday }
    }

    /// `FUN_0068f9a0`: the person of his record (0x54 bytes at
    /// `DAT_00be3920` + n × 0x54) takes his values.
    fn fill(&self, person: &mut Person, pool: &mut Pool) {
        person.set_names([pool.index(&self.names[0]), pool.index(&self.names[1])]);
        person.put(0x24, self.trainer);
        person.put(0x25, self.manager);
        person.set_training_kind(self.kind);
        let (day, month, year) = self.birthday;
        person.put(0x28, day);
        person.put(0x29, month);
        person.put(0x2a, year);
    }
}

/// The values of a `VEREIN`, by line, as `FUN_0068cdfa` reads them after
/// its `TRAINER` and `MANAGER`.
const NAME: usize = 1;
const SHORT: usize = 2;
const NICK: usize = 3;
const PEOPLE: usize = 4;
/// Lines 5 to 12 go to the scratch club's +0x7e, +0x81, +0x7f, +0x80,
/// +0x82, +0x85, +0x83, +0x84: the team's +0x114 and +0x118.
const BYTES_7E: usize = 5;
const BYTES_7E_AT: [usize; 8] = [0, 3, 1, 2, 4, 7, 5, 6];
const MONEY: usize = 13;
const BYTE_90: usize = 14;
const BYTE_8C: usize = 15;
const BYTE_91: usize = 17;
const BOARD: usize = 19;
const BYTE_94: usize = 20;
const BYTE_96: usize = 21;
const FLAGS_98: usize = 22;
const MAP_AT: usize = 23;
const BYTES_FE: usize = 25;
const INTS_108: usize = 33;
const PRESIDENT_LAST: usize = 37;
const PRESIDENT_FIRST: usize = 38;
const PRESIDENT_BIRTHDAY: usize = 39;
const BYTE_1B0: usize = 40;
const SHORTS_1A4: usize = 41;
const BYTE_1B1: usize = 47;
const FOUNDED: usize = 48;
/// Lines 49 to 54 the game drops; then how many players.
const PLAYER_COUNT: usize = 55;

/// The money the files give, in 10,000s, kept between these
/// (`FUN_0068cdfa`), and its unit (`DAT_00a3b3bc`).
const MONEY_RANGE: (i64, i64) = (-2000, 1500);
const MONEY_UNIT: i64 = 10_000;
/// `DAT_008a3d98`: the file's line 14 into the team's +0x120.
const BYTE_90_TABLE: [u8; 16] = [2, 0, 1, 0, 1, 2, 3, 4, 5, 8, 7, 6, 9, 10, 0, 0];
/// The files' bits of line 22 the game keeps.
const FLAGS_98_KEPT: u32 = 0x1c7;
/// The texts' room in the scratch club, the NUL counted.
const NAME_ROOM: usize = 0x13;
const SHORT_ROOM: usize = 4;

/// A club as `FUN_0068cdfa` reads its `VEREIN`, by the scratch club's
/// offsets.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Club {
    /// +0 (19 characters), +0x14 (4), +0x18, +0x4a: the name, "1.FSV
    /// Mainz 05", the short name, "FSV", and two more, "FSV", "Mainzer".
    pub name: String,
    pub short: String,
    pub nick: String,
    pub people: String,
    /// The trainer and the manager.
    pub staff: [Staff; 2],
    /// +0x7e … +0x85 in the scratch's order.
    pub bytes_7e: [u8; 8],
    /// +0x88: in 10,000s.
    pub money: i64,
    /// +0x90 (through `DAT_008a3d98`), +0x8c, +0x8d.
    pub byte_90: u8,
    pub bytes_8c: [u8; 2],
    /// +0x91, +0x92: the file's less one.
    pub bytes_91: [i8; 2],
    /// +0x93: how the board is, 0 to 5.
    pub board: u8,
    pub byte_94: u8,
    pub byte_96: u8,
    /// +0x98: the last season's title (low three bits) and more.
    pub flags_98: u32,
    /// +0xfa, +0xfc: where it lies on the country's map.
    pub map_at: [i16; 2],
    /// +0xfe … +0x105.
    pub bytes_fe: [u8; 8],
    /// +0x108 … +0x114.
    pub ints_108: [u32; 4],
    /// +0x174, +0x188: the president's first and last name; +0x19c his
    /// birthday.
    pub president: [String; 2],
    pub president_birthday: (u8, u8, u16),
    pub byte_1b0: u8,
    /// +0x1a4 … +0x1ae.
    pub shorts_1a4: [i16; 6],
    /// +0x1b1: 2 a registered club.
    pub byte_1b1: u8,
    /// +0x1a0: when it was founded.
    pub founded: (u8, u8, u16),
    /// +0x7c: how many `SPIELER` follow.
    pub player_count: u16,
}

impl Club {
    pub fn read(verein: &Section) -> Self {
        let long = |i: usize| verein.number(i).unwrap_or(0);
        let byte = |i: usize| long(i) as u8;
        let text = |i: usize| verein.text(i).unwrap_or_default().to_owned();
        let room = |i: usize, room: usize| text(i).chars().take(room).collect::<String>();
        let staff = |name: &str, role| verein.child(name).map(|s| Staff::read(s, role)).unwrap_or_default();
        let mut bytes_7e = [0; 8];
        for (k, &at) in BYTES_7E_AT.iter().enumerate() {
            bytes_7e[at] = byte(BYTES_7E + k);
        }
        Self {
            name: room(NAME, NAME_ROOM),
            short: room(SHORT, SHORT_ROOM),
            nick: text(NICK),
            people: text(PEOPLE),
            staff: [staff("TRAINER", Role::Trainer), staff("MANAGER", Role::Manager)],
            bytes_7e,
            money: long(MONEY).clamp(MONEY_RANGE.0, MONEY_RANGE.1),
            byte_90: BYTE_90_TABLE[usize::from(byte(BYTE_90) & 0xf)],
            bytes_8c: [byte(BYTE_8C), byte(BYTE_8C + 1)],
            bytes_91: [(long(BYTE_91) as i8).wrapping_sub(1), (long(BYTE_91 + 1) as i8).wrapping_sub(1)],
            board: byte(BOARD),
            byte_94: byte(BYTE_94),
            byte_96: byte(BYTE_96),
            flags_98: long(FLAGS_98) as u32 & FLAGS_98_KEPT,
            map_at: [long(MAP_AT) as i16, long(MAP_AT + 1) as i16],
            bytes_fe: std::array::from_fn(|k| byte(BYTES_FE + k)),
            ints_108: std::array::from_fn(|k| long(INTS_108 + k) as u32),
            president: [room(PRESIDENT_FIRST, NAME_ROOM), room(PRESIDENT_LAST, NAME_ROOM)],
            president_birthday: date(&text(PRESIDENT_BIRTHDAY)),
            byte_1b0: byte(BYTE_1B0),
            shorts_1a4: std::array::from_fn(|k| long(SHORTS_1A4 + k) as i16),
            byte_1b1: byte(BYTE_1B1),
            founded: date(&text(FOUNDED)),
            player_count: long(PLAYER_COUNT) as u16,
        }
    }

    /// `FUN_0068d411`: the team takes the club's values; its country is
    /// the file's, its number the club's place there, its league level 0.
    pub fn fill(&self, team: &mut Team, country: u8, number: u8, pool: &mut Pool) {
        let put_text = |team: &mut Team, at: usize, text: &str, room: usize| {
            let bytes: Vec<u8> = text.chars().map(|c| to_cp1252(c).unwrap_or(b'?')).collect();
            for k in 0..room {
                team.put::<u8>(at + k, bytes.get(k).copied().unwrap_or(0));
            }
        };
        put_text(team, 0x20, &self.name, NAME_ROOM + 1);
        // The short name has four bytes and no NUL of its own when it fills
        // them: the next text follows it.
        put_text(team, 0x34, &self.short, SHORT_ROOM);
        put_text(team, 0x38, &self.nick, 0x6a - 0x38);
        put_text(team, 0x6a, &self.people, 0x9c - 0x6a);
        team.set_country(country);
        team.set_number(number);
        team.set_level(0);
        for (k, &b) in self.bytes_7e.iter().enumerate() {
            team.put(0x114 + k, b);
        }
        team.put(0x488, self.money * MONEY_UNIT);
        team.put(0x120, self.byte_90);
        team.put(0x11c, self.bytes_8c[0]);
        team.put(0x11d, self.bytes_8c[1]);
        team.put(0x121, self.bytes_91[0]);
        team.put(0x122, self.bytes_91[1]);
        team.set_board(self.board);
        let mut flags_124 = if self.byte_94 != 0 { 0x10 } else { 0 };
        if self.flags_98 & 0x100 != 0 {
            flags_124 |= 0x80;
        }
        team.put::<u8>(0x124, flags_124);
        team.put(0x126, self.byte_96);
        // The last season's title.
        let mut honours: u16 = team.get(0x110);
        honours |= match self.flags_98 & 7 {
            1 => 1,
            2 => 0x10,
            3 => 8,
            4 => 4,
            6 => 2,
            _ => 0,
        };
        if self.flags_98 & 0x40 != 0 {
            honours |= 0x1000;
        }
        if self.flags_98 & 0x80 != 0 {
            honours |= 0x2000;
        }
        team.put(0x110, honours);
        team.set_map_at(self.map_at);
        for (k, &b) in self.bytes_fe.iter().enumerate() {
            // +0x872, +0x873: the club it is one with, from 1 in the file.
            let b = if k == 2 || k == 3 { b.wrapping_sub(1) } else { b };
            team.put(0x870 + k + usize::from(k >= 4), b);
        }
        for (k, &v) in self.ints_108.iter().enumerate() {
            team.put(0x87c + 4 * k, v);
        }
        team.put(0x8f0, pool.index(&self.president[0]));
        team.put(0x8f2, pool.index(&self.president[1]));
        let put_date = |team: &mut Team, at: usize, (day, month, year): (u8, u8, u16)| {
            team.put(at, day);
            team.put(at + 1, month);
            team.put(at + 2, year);
        };
        put_date(team, 0x8f4, self.president_birthday);
        put_date(team, 0x920, self.founded);
        let mut legal = 0u32;
        if self.byte_1b0 != 0 {
            legal |= 1;
        }
        if self.byte_1b1 != 2 {
            legal |= 2;
        }
        team.put(0x924, legal);
        team.set_player_count(self.player_count);
    }
}

/// The persons of a new game, counted from 1 (`DAT_00bde198`), its
/// players, from `players` on (`DAT_00bde19c`), and the next entry of
/// the free players' table (`DAT_00ba0b20`).
pub struct Counters {
    pub persons: usize,
    pub players: usize,
    pub free: u16,
}

/// What a trainer's or manager's person draws besides the file's: 1 to 3
/// (`FUN_0068fa30`, at +0x2c).
const PERSON_DRAW: u32 = 3;

/// `FUN_0068a38c`: a club of a country's file into a team, its trainer and
/// manager into the next two persons, each drawing at +0x2c, its players
/// into the next records by position ([`squad`]); the team lists them and
/// adds up their strength.
pub fn fill_team(
    records: &mut Records,
    team: usize,
    verein: &Section,
    country: u8,
    number: u8,
    counters: &mut Counters,
    random: &mut Random,
) {
    let club = Club::read(verein);
    let mut pool = Pool::new(&mut records.names);
    club.fill(&mut records.teams[team], country, number, &mut pool);
    records.teams[team].set_squad_strength(0);
    let mut staff = records.teams[team].staff();
    for (k, man) in club.staff.iter().enumerate() {
        staff[1 + k] = counters.persons as u16;
        let person = &mut records.persons[counters.persons];
        man.fill(person, &mut pool);
        person.put(0x2c, random.below(PERSON_DRAW) as u8 + 1);
        counters.persons += 1;
    }
    records.teams[team].set_staff(staff);
    let mut ids = Vec::new();
    let mut strength: i16 = 0;
    for scratch in squad(verein) {
        let id = counters.players;
        let player = &mut records.players[id];
        scratch.fill(player, &mut pool, random);
        player.set_id(id as u16);
        strength = strength.wrapping_add(i16::from(player.strength()));
        ids.push(id as u16);
        counters.players += 1;
    }
    let team = &mut records.teams[team];
    for (i, &id) in ids.iter().enumerate() {
        team.put(0x1f2 + 2 * i, id);
    }
    team.set_squad_strength(strength);
}

/// The nations of the eleven countries with files, in their order
/// (`DAT_008a3820`): each one's row of the countries' table, its system
/// (`DAT_00ae5d0c`).
pub const NATIONS: [u8; 11] = [14, 16, 20, 28, 63, 51, 24, 47, 57, 59, 67];
const GERMANY: u8 = 14;
/// From this year Germany plays four leagues, not six (`FUN_006876b4`).
const FOUR_LEAGUES_FROM: u16 = 2000;
const GERMAN_LEAGUES: i16 = 4;
/// How many leagues the main country plays at most, and another.
const MAIN_LEAGUES: usize = 6;
/// The nations whose teams count in the tables of teams kept
/// (`DAT_0089e0b8`).
const KEEPS_TEAMS: [u8; 0x78] = [
    0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 1, 0, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 1, 1, 0, 0, 1, 1,
    0, 0, 1, 0, 0, 1, 1, 1, 0, 0, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 0, 1, 1, 1, 0, 0, 0, 1, 0, 1, 1, 1, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0,
];
/// How many free trainers and managers of a country played its file lists
/// (`DAT_008a3a8e`, `DAT_008a3a92`: the same for every system).
const FREE_TRAINERS: usize = 60;
const FREE_MANAGERS: usize = 40;
/// What the national trainer's person holds at +0x2c.
const NATIONAL_TRAINER: u8 = 0x28;
/// How many teams a country not played keeps.
const FOREIGN_TEAMS: usize = 8;

/// The countries' tables (`DAT_00ae5b38` …) the clubs fill.
const HAS_TEAMS: usize = 0xae5b38;
const SLOT: usize = 0xae5c97;
const SYSTEM: usize = 0xae5d0c;
const SLOT_COUNTRY: usize = 0xae5d8c;
const LEAGUE_COUNTRY: usize = 0xae5d98;
const LEAGUE_LEVEL: usize = 0xae5dc4;
const TEAM_COUNT: usize = 0xae5e06;
const FIRST_TEAM: usize = 0xae5ef0;
const FIRST_LEAGUE: usize = 0xae5fda;
const LEAGUES: usize = 0xae60c4;
const KIND: usize = 0xae61ae;
const PERSONS_FROM: usize = 0xae6306;
const PERSONS_TO: usize = 0xae6320;
const FOREIGN_CREST: usize = 0xae670e;
const CREST: usize = 0xae6bb6;
const CREST_ROW: usize = 0x6c;
/// The kinds of country (`DAT_00ae61ae`): its clubs all teams, or eight.
const KIND_PLAYED: u8 = 1;
const KIND_FOREIGN: u8 = 2;

/// The `VEREIN`s of a country's file.
fn vereine(land: &Section) -> Vec<&Section> {
    let clubs = land.child("LAND").or_else(|| land.child("BONUSLAND")).unwrap_or(land);
    clubs.all("VEREIN").collect()
}

/// The `LAND` section, which holds the cup's name, how many leagues and
/// clubs, and the clubs.
fn land_section(land: &Section) -> &Section {
    land.child("LAND").or_else(|| land.child("BONUSLAND")).unwrap_or(land)
}

/// `FUN_006876b4`: a country played takes the leagues from `next` on, the
/// main one up to six, another one; each knows its country and level and
/// reads its file ([`read_league`]) into its object.
fn take_leagues(records: &mut Records, dir: &Path, nation: u8, row: usize, main: bool, next: &mut i16) -> io::Result<()> {
    let nation_at = 2 * usize::from(nation);
    records.countries.put(FIRST_LEAGUE + nation_at, *next);
    let most = if main { MAIN_LEAGUES } else { 1 };
    let files = crate::world::LEAGUE_FILES[row];
    let main_country = records.globals.main_country();
    let year = records.globals.start_year();
    let mut level = 0;
    while level < most
        && level < files.len()
        && (main_country != i16::from(GERMANY) || year < FOUR_LEAGUES_FROM || level < GERMAN_LEAGUES as usize)
    {
        let at = 2 * usize::try_from(*next).unwrap_or(0);
        records.countries.put(LEAGUE_COUNTRY + at, i16::from(nation));
        records.countries.put(LEAGUE_LEVEL + at, level as i16);
        let liga = Section::read(&find(dir, files[level])?)?;
        let name = format!("{LEAGUE_AT:#x}[{next}]");
        let block = records.block_mut(&name).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("no block {name}")))?;
        let league = &mut League(block);
        read_league(league, &liga, nation, level as i16, year);
        // The system's flags (`DAT_008a3aa8`), less the European cup's
        // place of a league played twice over, and the main country's
        // bit 0.
        let mut flags = LEAGUE_FLAGS;
        if league.get::<i16>(0x6a) != 0 {
            flags &= !FLAG_TWICE_OFF;
        }
        if i16::from(nation) == main_country {
            flags &= !FLAG_MAIN_OFF;
        }
        league.put(0x60, flags);
        *next += 1;
        level += 1;
    }
    records.countries.put(LEAGUES + nation_at, level as i16);
    Ok(())
}

/// The leagues' objects (`0x1a09d48[li]`, 0x1a88 bytes; the save holds
/// each from +0x20).
const LEAGUE_AT: usize = 0x1a09d48;
const LEAGUE_BASE: usize = 0x20;
/// What every system's row of the league files' table (`0x8a3a6c`, 0x44
/// bytes a row) holds at +0x3c (`DAT_008a3aa8`): 0xf0 for all twelve.
const LEAGUE_FLAGS: u32 = 0xf0;
const FLAG_TWICE_OFF: u32 = 0x10;
const FLAG_MAIN_OFF: u32 = 1;
/// The nations whose league plays its fixture list twice (+0x6a).
const PLAYS_TWICE: [u8; 2] = [0x2f, 0x39];
const ENGLAND: u8 = 16;
/// The fixture list: a row of 0x18 bytes a matchday, each team's place in
/// the league, pairs of home and away; a copy of it after it.
const FIXTURES: usize = 0x55a;
const FIXTURES_COPY: usize = 0x9aa;
const FIXTURES_LEN: usize = 0x450;
const FIXTURE_ROW: usize = 0x18;

/// A league object, by the object's offsets.
struct League<'a>(&'a mut [u8]);

impl League<'_> {
    fn get<T: crate::records::Field>(&self, at: usize) -> T {
        T::get(&self.0[at - LEAGUE_BASE..])
    }

    fn put<T: crate::records::Field>(&mut self, at: usize, value: T) {
        value.put(&mut self.0[at - LEAGUE_BASE..]);
    }
}

/// `FUN_00687897`: a league reads its `LIGA` (name, short name, how many
/// teams, each one's place in the country's file from 1, the fixture list
/// of both halves, places from 1) into its object.
fn read_league(league: &mut League, file: &Section, nation: u8, level: i16, year: u16) {
    let nation16 = i16::from(nation);
    league.put(0x54, nation16);
    league.put(0x56, level);
    let Some(liga) = file.child("LIGA") else { return };
    let mut values = liga.values.iter();
    let mut text = |at: usize, room: usize, league: &mut League| {
        let bytes: Vec<u8> = values.next().map_or_else(Vec::new, |v| v.chars().map(|c| to_cp1252(c).unwrap_or(b'?')).collect());
        for k in 0..room {
            league.put::<u8>(at + k, bytes.get(k).copied().unwrap_or(0));
        }
    };
    text(0x20, 0x20, league);
    text(0x40, 0x10, league);
    let mut numbers = liga.values.iter().skip(2).map(|v| atoi(v));
    league.put(0x58, 3i16);
    league.put(0x5a, 3i16);
    league.put(0x5c, 0u32);
    league.put(0x60, 0u32);
    league.put(0x64, i16::from(nation == ENGLAND && level > 0));
    league.put(0x6e, promoted(nation, level, year));
    league.put(0x70, relegated(nation, level));
    let teams = numbers.next().unwrap_or(0) as i16;
    league.put(0x50, teams);
    let twice = PLAYS_TWICE.contains(&nation);
    league.put(0x6a, i16::from(twice));
    let count = usize::try_from(teams).unwrap_or(0);
    for i in 0..count {
        league.put(0x72 + 2 * i, (numbers.next().unwrap_or(0) as i16).wrapping_sub(1));
    }
    let days = (2 * count).saturating_sub(2);
    league.put(0x52, days as i16);
    for day in 0..days {
        for i in 0..count {
            league.put(FIXTURES + i + day * FIXTURE_ROW, (numbers.next().unwrap_or(0) as u8).wrapping_sub(1));
        }
    }
    if twice {
        for day in 0..days {
            for i in 0..count {
                let place: u8 = league.get(FIXTURES + day * FIXTURE_ROW + i);
                league.put(FIXTURES + i + (day + days) * FIXTURE_ROW, place);
            }
        }
        league.put(0x52, (2 * days) as i16);
    }
    let fixtures = league.0[FIXTURES - LEAGUE_BASE..FIXTURES - LEAGUE_BASE + FIXTURES_LEN].to_vec();
    league.0[FIXTURES_COPY - LEAGUE_BASE..FIXTURES_COPY - LEAGUE_BASE + FIXTURES_LEN].copy_from_slice(&fixtures);
}

/// `FUN_006874bb`: how many teams of a league below the first go up.
fn promoted(nation: u8, level: i16, year: u16) -> i16 {
    if level == 0 {
        return 0;
    }
    match nation {
        GERMANY if level == 1 => 3,
        GERMANY if year < FOUR_LEAGUES_FROM => 1,
        GERMANY => 2,
        ENGLAND => 2,
        0x14 | 0x3f => 3,
        0x1c => 4,
        _ => 0,
    }
}

/// `FUN_00687586`: how many teams of a league go down.
fn relegated(nation: u8, level: i16) -> i16 {
    match nation {
        GERMANY => if level == 0 { 3 } else { 4 },
        ENGLAND => if level == 3 { 1 } else { 3 },
        0x14 => if level == 0 { 3 } else { 4 },
        0x18 => 2,
        0x1c => 4,
        0x2f | 0x39 => 1,
        0x33 | 0x43 => 3,
        0x3f => if level == 0 { 3 } else { 4 },
        _ => 0,
    }
}

/// `FUN_00687e7c` for the main country, from team `first` on: every club
/// of its file in its order, then the country's free trainers and
/// managers and its national trainer as persons. How many teams.
/// The country file's `AJUGEND` first ([`crate::land_pool::read_young`],
/// `FUN_0068db23` before the clubs), its slot `slot`.
pub fn read_main(
    records: &mut Records,
    land: &Section,
    nation: u8,
    row: usize,
    slot: usize,
    first: usize,
    counters: &mut Counters,
    random: &mut Random,
) -> usize {
    let head = land_section(land);
    let nation_at = 2 * usize::from(nation);
    records.countries.put(LEAGUES + nation_at, head.number(1).unwrap_or(0) as i16);
    if nation == GERMANY && records.globals.start_year() >= FOUR_LEAGUES_FROM {
        records.countries.put(LEAGUES + nation_at, GERMAN_LEAGUES);
    }
    records.countries.put(PERSONS_FROM + 2 * row, counters.persons as i16);
    records.countries.put(PERSONS_TO + 2 * row, counters.persons as i16);
    let crest_slot = usize::from(records.countries.slot(usize::from(nation)));
    crate::land_pool::read_young(records, land, slot, nation, &mut counters.players, &mut counters.free, random);
    let clubs = vereine(land);
    for (i, verein) in clubs.iter().enumerate() {
        fill_team(records, first + i, verein, nation, i as u8, counters, random);
        records.countries.put(CREST + crest_slot * CREST_ROW + i, i as u8);
    }
    let count = clubs.len();
    take_teams(records, nation, count, KIND_PLAYED);
    let mut pool = Pool::new(&mut records.names);
    let free = |list: &str, name: &str, role, most| -> Vec<Staff> {
        head.child(list).map_or_else(Vec::new, |l| l.all(name).take(most).map(|s| Staff::read(s, role)).collect())
    };
    let trainers = free("TRAINERP", "TRAINER", Role::Trainer, FREE_TRAINERS);
    let managers = free("MANAGERP", "MANAGER", Role::Manager, FREE_MANAGERS);
    for man in trainers.iter().chain(&managers) {
        man.fill(&mut records.persons[counters.persons], &mut pool);
        counters.persons += 1;
    }
    // `NATTRA`, which the files leave out: the reader looks for a
    // `TRAINER` from the file's start and takes the first club's.
    let national = first_named(head, "TRAINER").map(|s| Staff::read(s, Role::Trainer)).unwrap_or_default();
    let person = &mut records.persons[counters.persons];
    national.fill(person, &mut pool);
    person.put(0x2c, NATIONAL_TRAINER);
    counters.persons += 1;
    records.countries.put(PERSONS_TO + 2 * row, counters.persons as i16);
    count
}

/// The first section of this name in the file's order, at any depth.
fn first_named<'a>(section: &'a Section, name: &str) -> Option<&'a Section> {
    section.children.iter().find_map(|child| if child.name == name { Some(child) } else { first_named(child, name) })
}

/// How many teams a country has, and of which kind; the tables of teams
/// kept count them if its nation does.
fn take_teams(records: &mut Records, nation: u8, count: usize, kind: u8) {
    let countries = &mut records.countries;
    let nation = usize::from(nation);
    countries.put(TEAM_COUNT + 2 * nation, count as i16);
    countries.put(KIND + nation, kind);
    if KEEPS_TEAMS.get(nation).is_some_and(|&k| k != 0) {
        countries.put(HAS_TEAMS + nation, count as u8);
    }
}

/// `FUN_00687e7c` for a country not played, from team `first` on: the
/// `chosen` clubs of its file ([`foreign_clubs`]), numbered from 0; the
/// table of crests keeps their places in the file, but of a club `fixed`
/// names its new number. Eight teams, one league.
/// The country file's slot `slot`; its `AJUGEND` youths come first, as
/// in the main country.
pub fn read_foreign(
    records: &mut Records,
    land: &Section,
    nation: u8,
    slot: usize,
    chosen: &[usize],
    fixed: &[(u8, usize)],
    first: usize,
    counters: &mut Counters,
    random: &mut Random,
) -> usize {
    let head = land_section(land);
    let nation_at = 2 * usize::from(nation);
    records.countries.put(LEAGUES + nation_at, head.number(1).unwrap_or(0) as i16);
    crate::land_pool::read_young(records, land, slot, nation, &mut counters.players, &mut counters.free, random);
    let clubs = vereine(land);
    for (k, &club) in chosen.iter().enumerate().take(FOREIGN_TEAMS) {
        fill_team(records, first + k, clubs[club], nation, k as u8, counters, random);
        // A club `Kleinig.sav` names keeps its new number there, and the
        // table of crests takes that number, not the club's place.
        let fixed = fixed.iter().any(|&(n, c)| n == nation && c == club);
        let crest = if fixed { k } else { club };
        records.countries.put(FOREIGN_CREST + usize::from(nation) * FOREIGN_TEAMS + k, crest as u8);
    }
    take_teams(records, nation, FOREIGN_TEAMS, KIND_FOREIGN);
    records.countries.put(LEAGUES + nation_at, 1i16);
    FOREIGN_TEAMS
}

/// The ranking's rows and their length (`DAT_00b5d1a8`, `memset` 0x534).
const RANKING: usize = 0x25;
const RANKING_ROW: usize = 0x24;
const RANKING_AT: usize = 0xb5d1a8;
/// A row's values in `UEFA`: the country, then six numbers.
const RANKING_VALUES: usize = 7;
/// The codes behind the rows, the three fixed clubs' pairs, the three
/// others and the two numbers in `UEFA`'s values; 0x26 of them.
const CODES_AT: usize = RANKING * RANKING_VALUES + 14;
/// A country block's byte its short code stands at (`DAT_019f8186`).
const CODE: usize = 0x196;
/// The nation's slot among the codes (the exe table at `DAT_008a3da8`).
const CODE_SLOTS: [u8; 0x75] = [
    0, 0, 0, 0, 0, 0, 0, 0x0b, 0, 0x12, 0, 0, 0, 0x0f, 0, 0,
    1, 0, 0, 0x14, 2, 0, 0, 0x0e, 6, 0x1d, 0x16, 0x15, 3, 0, 0, 0,
    0, 0, 0x18, 0, 0, 0, 0x1e, 0x1c, 0, 5, 0, 0, 0, 0x1b, 0x17, 8,
    0, 0, 0, 0x11, 0, 0x0d, 0x0c, 0, 0, 9, 0x10, 0x0a, 0, 0x1f, 0x20, 4,
    0, 0x21, 0, 7, 0x1a, 0x13, 0, 0, 0, 0x22, 0, 0x19, 0x23, 0x24, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0,
];
/// A more country takes its own name, not the code, where this byte
/// stands (the exe table at `DAT_0089dcd8`).
const CODE_OWN: [u8; 0x75] = [
    0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1,
    0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0,
    1, 1, 1, 0, 1, 1, 1, 1, 1, 0, 1, 0, 1, 1, 1, 0,
    1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1,
];

/// `FUN_00686684`'s head, `Kleinig.sav` read before any club: its `UEFA`
/// rows — the country, then the six numbers of its seasons (`FUN_00401f82`,
/// `FUN_00401ed5`), the first five of them summed to the row's total — into
/// the ranking, exchanged by that total highest first (`FUN_00423bd5`); and
/// its 38 short codes into the played countries' blocks at [`CODE`], the
/// main one's at its slot and each more country's by the table at
/// [`CODE_SLOTS`], its own name instead where [`CODE_OWN`] stands. Then the
/// `MISC` tail's two names, the FIFA president's, interned as `FUN_0068b511`
/// does with the pool as it stands, their indices into `DAT_00b5dfca` and
/// `DAT_00b5dfc8`; `FUN_0067bab0`'s reset follows, and "Der" and "Teamchef"
/// (`DAT_008a3fb8`, `s_Teamchef_008a3fbc`) begin the pool anew. The first
/// turn's dump stands with them first; its 4 and 5 in the two indices count
/// the names an already open session had put into the pool before.
pub fn read_head(records: &mut Records, dir: &Path) -> io::Result<()> {
    let kleinig = Section::read(&find(dir, "Kleinig.sav")?)?;
    let Some(misc) = kleinig.child("MISC") else { return Ok(()) };
    if let Some(uefa) = misc.child("UEFA") {
        let mut rows = vec![[0u8; RANKING_ROW]; RANKING];
        for (row, values) in uefa.values.chunks(RANKING_VALUES).enumerate().take(RANKING) {
            let number = |n: usize| atoi(values.get(n).map(String::as_str).unwrap_or(""));
            let total = (1..=5).fold(0f32, |sum, n| sum + number(n) as f32);
            rows[row][0..2].copy_from_slice(&(number(0) as i16).to_le_bytes());
            rows[row][4..8].copy_from_slice(&total.to_le_bytes());
            for n in 0..6 {
                rows[row][8 + n * 4..12 + n * 4].copy_from_slice(&(number(n + 1) as f32).to_le_bytes());
            }
        }
        // `FUN_00423bd5`: every later row with the greater total exchanges
        // places with the one standing at the earlier place.
        for a in 0..RANKING {
            for c in a + 1..RANKING {
                let (x, y) = (f32::from_le_bytes(rows[a][4..8].try_into().unwrap()), f32::from_le_bytes(rows[c][4..8].try_into().unwrap()));
                if x < y {
                    rows.swap(a, c);
                }
            }
        }
        for (row, bytes) in rows.iter().enumerate() {
            records.globals.put(RANKING_AT + row * RANKING_ROW, *bytes);
        }
        let code = |nation: usize| -> &str {
            let slot = usize::from(CODE_SLOTS.get(nation).copied().unwrap_or(0));
            uefa.values.get(CODES_AT + slot).map(String::as_str).unwrap_or("")
        };
        let main = usize::try_from(records.globals.main_country()).unwrap_or(0);
        put_code(records, 0, code(main));
        let more = usize::try_from(records.globals.more_count()).unwrap_or(0);
        for (i, &other) in records.globals.more_countries().iter().take(more).enumerate() {
            let nation = usize::try_from(other).unwrap_or(0);
            // Else the original copies a local it never filled; what its
            // stack held no reading finds out.
            if CODE_OWN.get(nation).copied().unwrap_or(0) == 0 {
                put_code(records, i + 1, code(nation));
            }
        }
    }
    // The president's names stand in the `MISC` tail after the three
    // numbers; `FUN_0068b511` scans the pool by strcmp and appends.
    let name = |n: usize| misc.values.get(n).map_or("", String::as_str);
    let (last, first) = (name(3), name(4));
    let mut names = std::mem::take(&mut records.names);
    {
        let mut pool = Pool::new(&mut names);
        records.globals.put::<u16>(0xb5dfca, if last.is_empty() { 0 } else { pool.index(last) });
        records.globals.put::<u16>(0xb5dfc8, if first.is_empty() { 0 } else { pool.index(first) });
    }
    names.clear();
    {
        let mut pool = Pool::new(&mut names);
        pool.index("Der");
        pool.index("Teamchef");
    }
    records.names = names;
    Ok(())
}

/// One country block's short code, as `strcpy` leaves it: the bytes and
/// their NUL, the rest of the field as it was.
fn put_code(records: &mut Records, slot: usize, text: &str) {
    let Some(block) = records.block_mut(&format!("0x19f7fd0[{slot}]")) else { return };
    let bytes = text.as_bytes();
    let n = bytes.len().min(block.len().saturating_sub(CODE + 1));
    block[CODE..CODE + n].copy_from_slice(&bytes[..n]);
    block[CODE + n] = 0;
}

/// `FUN_00686684`'s clubs for a new game of one country played, the one of
/// `main` among the eleven: its leagues and teams first, then those of each
/// other country, eight of them. The persons and players counted on from
/// `counters`.
pub fn read_countries(records: &mut Records, dir: &Path, main: usize, counters: &mut Counters, random: &mut Random) -> io::Result<usize> {
    let nation = NATIONS[main];
    for (row, &n) in NATIONS.iter().enumerate() {
        records.countries.put(SYSTEM + usize::from(n), row as u8);
    }
    records.countries.put(SLOT + usize::from(nation), 0u8);
    records.countries.put(SLOT_COUNTRY + 2, i16::from(nation));
    let mut leagues = 0;
    take_leagues(records, dir, nation, main, true, &mut leagues)?;
    let land = Section::read(&find(dir, LAND_FILES[main])?)?;
    let mut next = read_main(records, &land, nation, main, main, 0, counters, random);
    let world = crate::world::World::load(dir)?;
    let fixed = uefa_fixed(&Section::read(&find(dir, "Kleinig.sav")?)?);
    for (slot, &other) in NATIONS.iter().enumerate() {
        if slot == main {
            continue;
        }
        let Some(country) = world.country(slot) else { continue };
        let land = Section::read(&find(dir, LAND_FILES[slot])?)?;
        let first_league = country.leagues.first().map_or(&[][..], |l| &l.clubs[..]);
        let chosen = foreign_clubs(&land, other, first_league, &fixed);
        records.countries.put(FIRST_TEAM + 2 * usize::from(other), next as i16);
        next += read_foreign(records, &land, other, slot, &chosen, &fixed, next, counters, random);
    }
    read_world(records, dir, &world, nation, &fixed, next, counters, random)
}

/// Where the teams of `Internat.sav` and the bonus country start
/// (`DAT_00ae6304`): their stadiums are kept apart, from there on.
const FOREIGN_START: usize = 0xae6304;
/// The bonus country's nation (`DAT_00ae62f4`, also `DAT_01b990c8`), 0
/// without one (`CLandEinstellen`).
const BONUS_NATION: usize = 0xae62f4;
/// The clubs abroad (`0x1a28dd8[k]`, forty, 0x13c each; the save holds
/// each from +0x20): the name, then its country (+0x34) and its number
/// among the country's (+0x35); and by country where its first stands,
/// from 1 (`DAT_00ae5bad`), and how many (`DAT_00ae5c22`).
const ABROAD_AT: usize = 0x1a28dd8;
const ABROAD: usize = 0x28;
const ABROAD_NAME: usize = 0x14;
const ABROAD_COUNTRY: usize = 0x34 - 0x20;
const ABROAD_NUMBER: usize = 0x35 - 0x20;
const ABROAD_FIRST: usize = 0xae5bad;
const ABROAD_COUNT: usize = 0xae5c22;
/// A `VEREIN` of `Internat.sav` with this nation stands for the bonus
/// country's.
const BONUS_PLACE: u8 = 0xff;

/// `FUN_0068a5c1`, last in `FUN_00686684`, from team `first` on: of every
/// `LAND` of `Internat.sav` whose nation is not played its eight clubs as
/// teams of the second kind, numbered by their country's count; a `LAND`
/// of the bonus country (or of nation -1) takes the bonus file's eight
/// instead ([`read_foreign`]). Then the clubs abroad of `AVereine.sav`.
/// The teams' stadiums (`FUN_0068b808`, `FUN_0068bccb`, 0xf0 each at
/// `0x1a3af58` from [`FOREIGN_START`]) and `EUROSIEGER` (into the
/// program's table `0x8ea150`, not saved) are left out.
#[allow(clippy::too_many_arguments)]
fn read_world(
    records: &mut Records,
    dir: &Path,
    world: &crate::world::World,
    main: u8,
    fixed: &[(u8, usize)],
    first: usize,
    counters: &mut Counters,
    random: &mut Random,
) -> io::Result<usize> {
    let mut next = first;
    records.countries.put(FOREIGN_START, first as i16);
    let bonus = world.country(crate::world::BONUS_SLOT).map_or(0, |c| c.nation as u8);
    records.countries.put(BONUS_NATION, i16::from(bonus));
    // The bonus file's youths before any club of these (`FUN_0068e7c2`).
    if bonus != 0 {
        let file = Section::read(&find(dir, LAND_FILES[crate::world::BONUS_SLOT])?)?;
        crate::land_pool::read_young(records, &file, crate::world::BONUS_SLOT, bonus, &mut counters.players, &mut counters.free, random);
    }
    // `FUN_00676c03`: the main country and the more ones played; a game of
    // one country plays the main one alone.
    let played = |nation: u8| nation == main;
    let internat = Section::read(&find(dir, "Internat.sav")?)?;
    let mut last = 0u8;
    for land in internat.all("LAND") {
        for verein in land.all("VEREIN").take(FOREIGN_TEAMS) {
            let nation = verein.number(0).unwrap_or(0) as u8;
            if played(nation) || (nation == BONUS_PLACE && played(bonus)) {
                break;
            }
            if nation == bonus || nation == BONUS_PLACE {
                // `FUN_0068a301` takes the first league of the nation's
                // system (`DAT_00ae5d0c`), which the bonus country has not:
                // row 0, the German one's places pick its clubs.
                let row = usize::from(records.countries.get::<u8>(SYSTEM + usize::from(bonus)));
                let file = Section::read(&find(dir, LAND_FILES[crate::world::BONUS_SLOT])?)?;
                let first_league = world.country(row).and_then(|c| c.leagues.first()).map_or(&[][..], |l| &l.clubs[..]);
                let chosen = foreign_clubs(&file, bonus, first_league, fixed);
                records.countries.put(FIRST_TEAM + 2 * usize::from(bonus), next as i16);
                next += read_foreign(records, &file, bonus, crate::world::BONUS_SLOT, &chosen, fixed, next, counters, random);
                break;
            }
            let count: i16 = records.countries.get(TEAM_COUNT + 2 * usize::from(nation));
            fill_team(records, next, verein, nation, count as u8, counters, random);
            records.countries.put(FOREIGN_CREST + usize::from(nation) * FOREIGN_TEAMS + count as usize, count as u8);
            records.countries.put(HAS_TEAMS + usize::from(nation), (count + 1) as u8);
            records.countries.put(TEAM_COUNT + 2 * usize::from(nation), count + 1);
            if nation != last {
                records.countries.put(FIRST_TEAM + 2 * usize::from(nation), next as i16);
                records.countries.put(KIND + usize::from(nation), KIND_FOREIGN);
                last = nation;
            }
            next += 1;
        }
    }
    read_abroad(records, &Section::read(&find(dir, "AVereine.sav")?)?);
    Ok(next)
}

/// The clubs abroad (`IVEREINE`, the second part of `FUN_0068a5c1`; each
/// `IVEREIN` by `FUN_0068cda1`): a name and a country each, numbered
/// within the country as they follow one another.
fn read_abroad(records: &mut Records, file: &Section) {
    let Some(list) = file.child("IVEREINE") else { return };
    let mut last = None;
    let mut number = 0u8;
    for (k, club) in list.all("IVEREIN").take(ABROAD).enumerate() {
        let name = format!("{ABROAD_AT:#x}[{k}]");
        let Some(block) = records.block_mut(&name) else { break };
        let text: Vec<u8> = club.text(0).unwrap_or_default().chars().map(|c| to_cp1252(c).unwrap_or(b'?')).collect();
        for at in 0..ABROAD_NAME {
            block[at] = if at + 1 < ABROAD_NAME { text.get(at).copied().unwrap_or(0) } else { 0 };
        }
        let country = club.number(1).unwrap_or(0) as u8;
        block[ABROAD_COUNTRY] = country;
        if last == Some(country) {
            block[ABROAD_NUMBER] = number;
            number += 1;
        } else {
            block[ABROAD_NUMBER] = 0;
            number = 1;
            last = Some(country);
            records.countries.put(ABROAD_FIRST + usize::from(country), k as u8 + 1);
        }
        let count: u8 = records.countries.get(ABROAD_COUNT + usize::from(country));
        records.countries.put(ABROAD_COUNT + usize::from(country), count + 1);
    }
}

/// The first of the four players standing for the human managers, one
/// each by his number (`Player_ARRAY_00c02550[17000 + k]`).
pub const MANAGER_PLAYERS: usize = 17001;

/// `FUN_006a3fc7(nation, mode)`, after `FUN_00686684` and `FUN_0068716b`
/// (the new game passes `DAT_00b5cbb0` and `DAT_00b5cbc6`): every manager
/// record takes the game's mode (+0x12a), and the player standing for him
/// his names into the pool (`FUN_0068b511`, which trims the record's own
/// in place), his birthday, his position as main position, strength 1 and
/// the nation.
pub fn managers_as_players(records: &mut Records, nation: u8, mode: i16) {
    let mut pool = Pool::new(&mut records.names);
    for (k, manager) in records.managers.iter_mut().enumerate() {
        manager.set_mode(mode);
        let player = &mut records.players[MANAGER_PLAYERS + k];
        player.set_first_name(pool.index(&manager.first_name()));
        player.set_last_name(pool.index(&manager.last_name()));
        player.set_birth_day(manager.birth_day());
        player.set_birth_month(manager.birth_month());
        player.set_birth_year(manager.birth_year());
        let mut positions = player.positions();
        positions[0] = manager.position();
        player.set_positions(positions);
        player.set_strength(1);
        player.set_nation(nation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A manager's player takes his names once into the pool, his birthday
    /// and position, strength 1 and the nation; his record the mode.
    #[test]
    fn a_manager_stands_as_a_player_of_strength_one() {
        let mut records = Records::blank();
        records.managers[1].clear_with_names("Anna", "Alpha");
        records.managers[1].set_birth_day(3);
        records.managers[1].set_birth_year(1970);
        records.managers[1].set_position(4);
        managers_as_players(&mut records, 14, 9);
        let player = &records.players[MANAGER_PLAYERS + 1];
        assert_eq!((records.name(player.first_name()), records.name(player.last_name())), ("Anna", "Alpha"));
        assert_eq!((player.birth_day(), player.birth_year(), player.positions()[0], player.strength(), player.nation()), (3, 1970, 4, 1, 14));
        assert_eq!((records.players[MANAGER_PLAYERS].first_name(), records.managers[3].mode()), (0, 9));
    }

    /// The managers of the oracle's new game stand as players 17001 to
    /// 17004 of its first turn, their records in its mode.
    #[test]
    #[ignore = "reads the oracle's dumps from $ANSTOSS3_ORACLE"]
    fn a_new_games_managers_stand_as_players_as_the_originals() {
        let Some(oracle) = std::env::var_os("ANSTOSS3_ORACLE") else {
            eprintln!("no oracle dump of a new game's first turn among the saves: ANSTOSS3_ORACLE is not set");
            return;
        };
        let path = std::path::PathBuf::from(oracle).join("runs/new-game/first-turn.an3");
        if !path.exists() {
            eprintln!("no oracle dump of a new game's first turn among the saves");
            return;
        }
        let dump = Records::from_save(a3_files::an3::Save::read(&path).unwrap()).unwrap();
        let mut records = Records::blank();
        records.names = dump.names.clone();
        for (ours, theirs) in records.managers.iter_mut().zip(&dump.managers) {
            *ours = theirs.clone();
            ours.set_mode(0);
        }
        let (nation, mode) = (dump.globals.main_country() as u8, dump.globals.mode());
        eprintln!("nation {nation}, mode {mode}");
        managers_as_players(&mut records, nation, mode);
        for k in 0..crate::records::MANAGERS {
            let (ours, theirs) = (&records.players[MANAGER_PLAYERS + k], &dump.players[MANAGER_PLAYERS + k]);
            let apart: Vec<usize> = (0..ours.bytes().len()).filter(|&b| ours.bytes()[b] != theirs.bytes()[b]).collect();
            eprintln!("player {}: {} {} apart at {apart:x?}", MANAGER_PLAYERS + k, dump.name(theirs.first_name()), dump.name(theirs.last_name()));
            assert_eq!(
                (ours.first_name(), ours.last_name(), ours.birthday(), ours.positions()[0], ours.strength(), ours.nation()),
                (theirs.first_name(), theirs.last_name(), theirs.birthday(), theirs.positions()[0], theirs.strength(), theirs.nation())
            );
            assert_eq!(records.managers[k].mode(), dump.managers[k].mode());
        }
        let mut names = dump.names.clone();
        Pool::new(&mut names);
        assert_eq!(records.names, names);
    }

    /// The team's fields `FUN_0068d411` and `FUN_0068a38c` write, as bytes:
    /// where, how many, what.
    const FIELDS: [(usize, usize, &str); 24] = [
        (0x20, 0x14, "name"),
        (0x34, 4, "short"),
        (0x38, 0x32, "nick"),
        (0x6a, 0x32, "people"),
        (0x112, 2, "player count"),
        (0x114, 8, "bytes 114"),
        (0x11c, 2, "bytes 11c"),
        (0x120, 1, "byte 120"),
        (0x121, 2, "bytes 121"),
        (0x123, 1, "board"),
        (0x124, 1, "flags 124"),
        (0x126, 1, "byte 126"),
        (0x129, 1, "number"),
        (0x12d, 1, "country"),
        (0x12e, 1, "level"),
        (0x30e, 4, "map"),
        (0x488, 8, "money"),
        (0x870, 4, "bytes 870"),
        (0x875, 4, "bytes 875"),
        (0x87c, 16, "ints 87c"),
        (0x8f4, 4, "president's birthday"),
        (0x920, 4, "founded"),
        (0x924, 4, "legal form"),
        (0xc0, 2, "strength"),
    ];

    /// What of a team the original's first turn holds otherwise than the
    /// files gave it: its league level (the leagues' part sets it), money
    /// (the start settings reckon it anew, a big club's at seven tenths),
    /// strength (the club object's, `FUN_006a10bb`, as the game sets the
    /// clubs up), the human manager's club's staff (none, the managers'
    /// part), and what its trainer's and manager's persons draw (the
    /// generator stands elsewhere: the draws before the clubs, the free
    /// youths' and the referees', are the pool's part).
    const APART: [&str; 5] = ["level", "money", "strength", "human club's staff", "persons' draw"];

    #[test]
    fn a_verein_reads_as_the_original() {
        let lines = |l: &[&str]| l.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let trainer = Section {
            name: "TRAINER".into(),
            values: lines(&["Bo", "Henriksen", "8", "2", "50", "07.02.1975", "0", "0"]),
            children: vec![],
        };
        let staff = Staff::read(&trainer, Role::Trainer);
        assert_eq!((staff.trainer, staff.manager, staff.kind, staff.birthday), (8, 3, 2, (7, 2, 1975)));
        let manager = Section { name: "MANAGER".into(), values: lines(&["Niko", "Bungert", "4", "38", "24.10.1986"]), children: vec![] };
        let staff = Staff::read(&manager, Role::Manager);
        assert_eq!((staff.trainer, staff.manager, staff.kind, staff.birthday), (1, 4, 0, (24, 10, 1986)));
        let mut values = lines(&["14", "1.FSV Mainz 05 und mehr", "FSV", "FSV", "Mainzer", "1", "0", "0", "1", "67", "9", "3", "9", "-3000", "1", "3", "4", "62", "93", "4", "0", "0", "321"]);
        values.resize(56, "0".into());
        values[47] = "5".into();
        let club = Club::read(&Section { name: "VEREIN".into(), values, children: vec![trainer, manager] });
        assert_eq!(club.name, "1.FSV Mainz 05 und ");
        assert_eq!((club.bytes_7e, club.money, club.byte_90, club.bytes_91), ([1, 0, 1, 0, 67, 3, 9, 9], -2000, 0, [61, 92]));
        assert_eq!(club.flags_98, 0x141);
        let mut records = Records::blank();
        let mut pool = Pool::new(&mut records.names);
        let mut team = records.teams[0].clone();
        club.fill(&mut team, 14, 3, &mut pool);
        assert_eq!((team.name(), team.short_name(), team.country(), team.number()), ("1.FSV Mainz 05 und ".into(), "FSV".into(), 14, 3));
        assert_eq!((team.get::<i64>(0x488), team.get::<u16>(0x110), team.get::<u8>(0x124), team.legal_form()), (-20_000_000, 1 | 0x1000, 0x80, 2));
    }

    /// The head of the oracle's new game: the 0x25 ranking rows of
    /// `Kleinig.sav` sorted by their total, the short code in the main
    /// country's block, and "Der" and "Teamchef" first in the pool.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR and the oracle's dumps from $ANSTOSS3_ORACLE"]
    fn a_new_games_records_head_reads_as_the_original() {
        let (Some(game), Some(oracle)) = (std::env::var_os("ANSTOSS3_DIR"), std::env::var_os("ANSTOSS3_ORACLE")) else {
            eprintln!("no oracle dump of a new game's first turn among the saves: ANSTOSS3_DIR or ANSTOSS3_ORACLE is not set");
            return;
        };
        let path = std::path::PathBuf::from(oracle).join("runs/new-game/first-turn.an3");
        if !path.exists() {
            eprintln!("no oracle dump of a new game's first turn among the saves");
            return;
        }
        let dump = Records::from_save(a3_files::an3::Save::read(&path).unwrap()).unwrap();
        let mut records = Records::blank();
        records.globals.set_main_country(dump.globals.main_country());
        let dir = std::path::PathBuf::from(game).join("Data.a3");
        read_head(&mut records, &dir).unwrap();
        for row in 0..RANKING {
            let at = RANKING_AT + row * RANKING_ROW;
            assert_eq!(records.globals.get::<[u8; RANKING_ROW]>(at), dump.globals.get::<[u8; RANKING_ROW]>(at), "ranking row {row}");
        }
        // The played countries' codes: main and each more country the
        // table gives a code to, the own-name ones left as the original
        // left their stack in them.
        for i in 0..=4 {
            let own = i > 0 && CODE_OWN.get(usize::try_from(records.globals.more_countries()[i - 1]).unwrap_or(0)).copied().unwrap_or(0) != 0;
            if own {
                continue;
            }
            let name = format!("0x19f7fd0[{i}]");
            let (mine, theirs) = (records.block(&name).unwrap(), dump.block(&name).unwrap());
            assert_eq!(mine[CODE..CODE + 6], theirs[CODE..CODE + 6], "country code {i}");
        }
        assert_eq!(&records.names[..2], ["Der", "Teamchef"]);
        // The head names the president in the pool as it stands when the
        // head runs; the first turn's 4 and 5 count the names a session
        // open before the head had put in its pool, not the fresh one.
        assert_eq!((records.globals.get::<u16>(0xb5dfca), records.globals.get::<u16>(0xb5dfc8)), (0, 1));
    }

    /// The clubs of the oracle's new game in Germany
    /// (`tools/oracle/new-game.sh`), all of the main country, eight of each
    /// other country, as the files give them: the first turn holds the
    /// teams at the same places, alike in the files' values.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR and the oracle's dumps from $ANSTOSS3_ORACLE"]
    fn a_new_games_clubs_read_as_the_originals() {
        let (Some(game), Some(oracle)) = (std::env::var_os("ANSTOSS3_DIR"), std::env::var_os("ANSTOSS3_ORACLE")) else {
            eprintln!("no oracle dump of a new game's first turn among the saves: ANSTOSS3_DIR or ANSTOSS3_ORACLE is not set");
            return;
        };
        let path = std::path::PathBuf::from(oracle).join("runs/new-game/first-turn.an3");
        if !path.exists() {
            eprintln!("no oracle dump of a new game's first turn among the saves");
            return;
        }
        let dump = Records::from_save(a3_files::an3::Save::read(&path).unwrap()).unwrap();
        let mut records = Records::blank();
        records.globals.set_main_country(dump.globals.main_country());
        records.globals.set_start_year(dump.globals.start_year());
        let dir = std::path::PathBuf::from(game).join("Data.a3");
        let mut counters = Counters { persons: 1, players: 1, free: 0 };
        let teams = read_countries(&mut records, &dir, 0, &mut counters, &mut Random::new(1)).unwrap();
        // The main country's, eight of each other of the eleven, and eight
        // of each `LAND` of `Internat.sav` (the bonus country's from its file).
        assert_eq!(teams, 108 + 10 * 8 + 26 * 8);
        let mut apart = std::collections::BTreeMap::<String, Vec<String>>::new();
        let name = |r: &Records, i: u16| r.name(i).to_owned();
        let mut others = std::collections::BTreeMap::<usize, usize>::new();
        for t in 0..teams {
            let (ours, theirs) = (&records.teams[t], &dump.teams[t]);
            let label = format!("{t} {}", ours.name());
            let mut differ = |what: &str, a: String, b: String| {
                if a != b {
                    apart.entry(what.to_owned()).or_default().push(format!("{label}: {a}, not {b}"));
                }
            };
            for &(at, len, what) in &FIELDS {
                let bytes = |team: &Team| (at..at + len).map(|k| team.get::<u8>(k)).collect::<Vec<u8>>();
                differ(what, format!("{:x?}", bytes(ours)), format!("{:x?}", bytes(theirs)));
            }
            // The bits of honours the files give, the others come later; and
            // the start settings' UEFA cup marks its clubs with 0x10.
            const HONOURS: u16 = 0x300f;
            let (a, b) = (ours.get::<u16>(0x110), theirs.get::<u16>(0x110));
            differ("honours", format!("{:x}", a & HONOURS), format!("{:x}", b & HONOURS));
            differ("honours", format!("{:x}", a & !b), "0".into());
            let president = |r: &Records, team: &Team| format!("{} {}", name(r, team.get(0x8f0)), name(r, team.get(0x8f2)));
            differ("president", president(&records, ours), president(&dump, theirs));
            let staff = |r: &Records, team: &Team, at: std::ops::Range<usize>| {
                team.staff()[1..]
                    .iter()
                    .map(|&p| {
                        let person = &r.persons[usize::from(p)];
                        let [a, b] = person.names();
                        format!("{} {} {:x?}", name(r, a), name(r, b), &person.bytes()[at.start - 0x20..at.end - 0x20])
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            // A human manager's club has no trainer and manager of its own.
            if theirs.human() == 0 {
                differ("staff", format!("{:?}", ours.staff()), format!("{:?}", theirs.staff()));
                differ("staff's persons", staff(&records, ours, 0x24..0x2c), staff(&dump, theirs, 0x24..0x2c));
                differ("persons' draw", staff(&records, ours, 0x2c..0x2e), staff(&dump, theirs, 0x2c..0x2e));
            } else {
                differ("human club's staff", format!("{:?}", ours.staff()), format!("{:?}", theirs.staff()));
            }
            let players = |r: &Records, team: &Team| {
                team.players()
                    .iter()
                    .map(|&id| {
                        let p = &r.players[usize::from(id)];
                        format!("{} {}", name(r, p.first_name()), name(r, p.last_name()))
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            differ("players", players(&records, ours), players(&dump, theirs));
            // The bytes neither writes alike, for the list of what parts.
            let other: Vec<usize> = (0x20..0x20 + Team::SIZE)
                .filter(|&k| ours.get::<u8>(k) != theirs.get::<u8>(k))
                .filter(|&k| !FIELDS.iter().any(|&(at, len, _)| (at..at + len).contains(&k)))
                .filter(|&k| !(0x1f2..0x1f2 + 2 * 50).contains(&k) && !(0x8f0..0x8f4).contains(&k))
                .collect();
            for k in other {
                *others.entry(k).or_insert(0) += 1;
            }
        }
        // The free trainers and managers and the national trainer, after
        // the main country's teams' persons.
        let free = 1 + 2 * 108..1 + 2 * 108 + FREE_TRAINERS + FREE_MANAGERS + 1;
        for n in free {
            let person = |r: &Records| {
                let person = &r.persons[n];
                let [a, b] = person.names();
                (format!("{} {} {:x?}", name(r, a), name(r, b), &person.bytes()[4..0xc]), person.get::<u8>(0x2c))
            };
            let ((a, draw), (b, theirs)) = (person(&records), person(&dump));
            if a != b {
                apart.entry("free persons".into()).or_default().push(format!("{n}: {a}, not {b}"));
            }
            if draw != theirs {
                apart.entry("persons' draw".into()).or_default().push(format!("{n} {a}: {draw}, not {theirs}"));
            }
        }
        // The countries' tables.
        let nations: Vec<usize> = NATIONS.iter().map(|&n| usize::from(n)).collect();
        let tables: [(&str, usize, usize); 9] = [
            ("has teams", HAS_TEAMS, 1),
            ("slot", SLOT, 1),
            ("system", SYSTEM, 1),
            ("team count", TEAM_COUNT, 2),
            ("first team", FIRST_TEAM, 2),
            ("first league", FIRST_LEAGUE, 2),
            ("leagues", LEAGUES, 2),
            ("kind", KIND, 1),
            ("foreign crest", FOREIGN_CREST, 8),
        ];
        for (what, at, width) in tables {
            let row = |c: &crate::records::Countries| -> Vec<String> {
                nations.iter().map(|&n| format!("{:x?}", (0..width).map(|k| c.get::<u8>(at + n * width + k)).collect::<Vec<_>>())).collect()
            };
            let (a, b) = (row(&records.countries), row(&dump.countries));
            if a != b {
                apart.entry(format!("countries' {what}")).or_default().push(format!("{a:?}, not {b:?}"));
            }
        }
        let flat = |c: &crate::records::Countries, at: usize, len: usize| (0..len).map(|k| c.get::<u8>(at + k)).collect::<Vec<u8>>();
        for (what, at, len) in [
            ("main country", SLOT_COUNTRY + 2, 2),
            ("league countries", LEAGUE_COUNTRY, 12),
            ("league levels", LEAGUE_LEVEL, 12),
            ("persons from", PERSONS_FROM, 22),
            ("persons to", PERSONS_TO, 22),
            ("crests", CREST, 108),
        ] {
            let (a, b) = (flat(&records.countries, at, len), flat(&dump.countries, at, len));
            if a != b {
                apart.entry(format!("countries' {what}")).or_default().push(format!("{a:?}, not {b:?}"));
            }
        }
        eprintln!("bytes the files do not give, by how many teams have them otherwise: {others:x?}");
        for (what, list) in &apart {
            eprintln!("{what}: {} apart, {:?}", list.len(), &list[..list.len().min(3)]);
        }
        for what in apart.keys() {
            assert!(APART.contains(&what.as_str()), "{what} parts from the original's first turn: {:?}", &apart[what][..apart[what].len().min(3)]);
        }
    }
    /// Clubs abroad of one country number on from 0, a new country starts
    /// again and notes where its first stands, from 1.
    #[test]
    fn clubs_abroad_number_within_their_country() {
        let club = |name: &str, country: &str| Section {
            name: "IVEREIN".into(),
            values: vec![name.into(), country.into()],
            children: vec![],
        };
        let list = Section {
            name: "IVEREINE".into(),
            values: vec![],
            children: vec![club("SC Al-Ahly", "1"), club("River Plate", "4"), club("Boca Juniors", "4"), club("Estudiantes", "4")],
        };
        let file = Section { name: String::new(), values: vec![], children: vec![list] };
        let mut records = Records::blank();
        read_abroad(&mut records, &file);
        let block = |k: usize| records.block(&format!("{ABROAD_AT:#x}[{k}]")).unwrap().to_vec();
        assert_eq!(&block(1)[..12], b"River Plate\0");
        assert_eq!([block(1)[ABROAD_COUNTRY], block(1)[ABROAD_NUMBER], block(3)[ABROAD_NUMBER]], [4, 0, 2]);
        let countries = &records.countries;
        assert_eq!([countries.abroad(1), countries.abroad(4), countries.get::<u8>(ABROAD_COUNT + 4)], [1, 2, 3]);
    }

    /// How many nations the countries' tables hold rows for (the table of
    /// foreign crests ends with nation 116).
    const NATION_COUNT: usize = 117;

    /// The oracle's new game's world after the eleven: the countries'
    /// tables of every nation, where the foreign teams start, the bonus
    /// country, and the forty clubs abroad, as the first turn holds them.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR and the oracle's dumps from $ANSTOSS3_ORACLE"]
    fn a_new_games_world_reads_as_the_originals() {
        let (Some(game), Some(oracle)) = (std::env::var_os("ANSTOSS3_DIR"), std::env::var_os("ANSTOSS3_ORACLE")) else {
            eprintln!("no oracle dump of a new game's first turn among the saves: ANSTOSS3_DIR or ANSTOSS3_ORACLE is not set");
            return;
        };
        let path = std::path::PathBuf::from(oracle).join("runs/new-game/first-turn.an3");
        if !path.exists() {
            eprintln!("no oracle dump of a new game's first turn among the saves");
            return;
        }
        let dump = Records::from_save(a3_files::an3::Save::read(&path).unwrap()).unwrap();
        let mut records = Records::blank();
        records.globals.set_main_country(dump.globals.main_country());
        records.globals.set_start_year(dump.globals.start_year());
        let dir = std::path::PathBuf::from(game).join("Data.a3");
        let mut counters = Counters { persons: 1, players: 1, free: 0 };
        read_countries(&mut records, &dir, 0, &mut counters, &mut Random::new(1)).unwrap();
        let mut apart = Vec::new();
        for (what, at, width) in [
            ("has teams", HAS_TEAMS, 1),
            ("team count", TEAM_COUNT, 2),
            ("first team", FIRST_TEAM, 2),
            ("leagues", LEAGUES, 2),
            ("kind", KIND, 1),
            ("foreign crest", FOREIGN_CREST, 8),
            ("abroad first", ABROAD_FIRST, 1),
            ("abroad count", ABROAD_COUNT, 1),
        ] {
            for n in 0..NATION_COUNT {
                let row = |c: &crate::records::Countries| (0..width).map(|k| c.get::<u8>(at + n * width + k)).collect::<Vec<u8>>();
                let (a, b) = (row(&records.countries), row(&dump.countries));
                if a != b {
                    apart.push(format!("{what} of {n}: {a:x?}, not {b:x?}"));
                }
            }
        }
        for (what, at) in [("foreign start", FOREIGN_START), ("bonus nation", BONUS_NATION)] {
            let (a, b) = (records.countries.get::<i16>(at), dump.countries.get::<i16>(at));
            if a != b {
                apart.push(format!("{what}: {a}, not {b}"));
            }
        }
        for k in 0..ABROAD {
            let name = format!("{ABROAD_AT:#x}[{k}]");
            let (a, b) = (records.block(&name).unwrap(), dump.block(&name).unwrap());
            if a != b {
                let at: Vec<usize> = (0..a.len()).filter(|&i| a[i] != b[i]).collect();
                apart.push(format!("abroad {k}: at {at:x?}, {:x?}, not {:x?}", &a[..0x18], &b[..0x18]));
            }
        }
        assert!(apart.is_empty(), "{apart:#?}");
    }
}

#[cfg(test)]
mod league_tests {
    use super::*;

    /// A league of four plays six matchdays; Austria's plays them twice,
    /// and the copy after the list holds them as the list does.
    #[test]
    fn a_liga_reads_into_its_league() {
        let mut values: Vec<String> = ["Bundesliga", "BL", "4", "3", "1", "4", "2"].iter().map(|s| s.to_string()).collect();
        values.extend((0..6 * 4).map(|k| (1 + k % 4).to_string()));
        let file = Section { name: String::new(), values: vec![], children: vec![Section { name: "LIGA".into(), values, children: vec![] }] };
        let mut bytes = vec![0u8; 0x1a68];
        let league = &mut League(&mut bytes);
        read_league(league, &file, 0x2f, 0, 1999);
        assert_eq!((league.get::<i16>(0x50), league.get::<i16>(0x52), league.get::<i16>(0x54), league.get::<i16>(0x6a)), (4, 12, 0x2f, 1));
        assert_eq!((league.get::<i16>(0x72), league.get::<i16>(0x78), league.get::<i16>(0x6e), league.get::<i16>(0x70)), (2, 1, 0, 1));
        assert_eq!((league.get::<u8>(FIXTURES + 6 * FIXTURE_ROW + 3), league.get::<u8>(FIXTURES_COPY + 7 * FIXTURE_ROW + 1)), (3, 1));
        assert_eq!(&league.0[..3], b"Bun");
        assert_eq!((promoted(GERMANY, 2, 1999), promoted(GERMANY, 2, 2000), relegated(ENGLAND, 3)), (1, 2, 1));
    }

    /// The main country's leagues of the oracle's new game in Germany
    /// (`tools/oracle/new-game.sh`), read from their files: the first turn
    /// holds them alike.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR and the oracle's dumps from $ANSTOSS3_ORACLE"]
    fn a_new_games_leagues_read_as_the_originals() {
        let (Some(game), Some(oracle)) = (std::env::var_os("ANSTOSS3_DIR"), std::env::var_os("ANSTOSS3_ORACLE")) else {
            eprintln!("no oracle dump of a new game's first turn among the saves: ANSTOSS3_DIR or ANSTOSS3_ORACLE is not set");
            return;
        };
        let path = std::path::PathBuf::from(oracle).join("runs/new-game/first-turn.an3");
        if !path.exists() {
            eprintln!("no oracle dump of a new game's first turn among the saves");
            return;
        }
        let dump = Records::from_save(a3_files::an3::Save::read(&path).unwrap()).unwrap();
        let mut records = Records::blank();
        records.globals.set_main_country(dump.globals.main_country());
        records.globals.set_start_year(dump.globals.start_year());
        let dir = std::path::PathBuf::from(game).join("Data.a3");
        let mut counters = Counters { persons: 1, players: 1, free: 0 };
        read_countries(&mut records, &dir, 0, &mut counters, &mut Random::new(1)).unwrap();
        let leagues = records.countries.leagues(usize::from(GERMANY));
        assert_eq!(leagues, dump.countries.leagues(usize::from(GERMANY)));
        const FIELDS: [(usize, usize, &str); 16] = [
            (0x20, 0x20, "name"),
            (0x40, 0x10, "short"),
            (0x50, 2, "teams"),
            (0x52, 2, "matchdays"),
            (0x54, 2, "country"),
            (0x56, 2, "level"),
            (0x58, 4, "threes"),
            (0x5c, 4, "int 5c"),
            (0x60, 4, "flags"),
            (0x64, 2, "england"),
            (0x6a, 2, "twice"),
            (0x6e, 2, "promoted"),
            (0x70, 2, "relegated"),
            (0x72, 0x30, "teams' places"),
            (FIXTURES, FIXTURES_LEN, "fixtures"),
            (FIXTURES_COPY, FIXTURES_LEN, "fixtures' copy"),
        ];
        let mut apart = Vec::new();
        for li in 0..usize::try_from(leagues).unwrap() {
            let name = format!("{LEAGUE_AT:#x}[{li}]");
            let (ours, theirs) = (records.block(&name).unwrap(), dump.block(&name).unwrap());
            for &(at, len, what) in &FIELDS {
                let range = at - LEAGUE_BASE..at - LEAGUE_BASE + len;
                if ours[range.clone()] != theirs[range.clone()] {
                    apart.push(format!("{li} {what}: {:x?}, not {:x?}", &ours[range.clone()][..len.min(24)], &theirs[range][..len.min(24)]));
                }
            }
            let other: Vec<usize> = (0..ours.len())
                .filter(|&k| ours[k] != theirs[k])
                .map(|k| k + LEAGUE_BASE)
                .filter(|&k| !FIELDS.iter().any(|&(at, len, _)| (at..at + len).contains(&k)))
                .collect();
            eprintln!("league {li}: bytes the files do not give: {:x?}", &other[..other.len().min(40)]);
        }
        assert!(apart.is_empty(), "{apart:#?}");
    }

    /// The whole of the original's setup between the loader's seeding
    /// and its calculation: the two managers, the countries with their
    /// youths, the world and the bonus file, the pool and the turn
    /// order — 33,530 draws, the records 32 to 33561 of the new game's
    /// trace, and the state its calculation meets them at.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR and the oracle's trace from $ANSTOSS3_ORACLE"]
    fn a_new_games_setup_draws_the_originals_to_the_calculation() {
        let (Some(game), Some(oracle)) = (std::env::var_os("ANSTOSS3_DIR"), std::env::var_os("ANSTOSS3_ORACLE")) else {
            eprintln!("no oracle trace of a new game among the saves: ANSTOSS3_DIR or ANSTOSS3_ORACLE is not set");
            return;
        };
        if !std::path::PathBuf::from(&oracle).join("runs/new-game/draws.bin").exists() {
            eprintln!("no oracle trace of a new game among the saves");
            return;
        }
        let dir = std::path::PathBuf::from(&game).join("Data.a3");
        let exe = a3_files::exe::Exe::open(&std::path::Path::new(&game).join("anstoss3.exe")).unwrap();
        let mut rules = crate::season::Rules::load(&exe).unwrap();
        let texts = a3_files::rch::load_texts(&std::path::Path::new(&game).join("deb/ListeDerTexte.rh")).unwrap();
        rules.training.name_originals(&texts);
        let mut records = Records::blank();
        records.globals.set_order([-1; crate::records::MANAGERS]);
        let mut random = Random::from_state(41, 815_730_420);
        for (first, last) in [("Anna", "Alpha"), ("Bert", "Beta")] {
            let trainer = crate::newgame::Trainer { first_name: first.into(), last_name: last.into(), ..crate::newgame::Trainer::default() };
            crate::land_managers::create(&mut records, &rules, &trainer, &mut random).unwrap();
        }
        let mut counters = Counters { persons: 1, players: 1, free: 0 };
        read_countries(&mut records, &dir, 0, &mut counters, &mut random).unwrap();
        let (mut next, mut entry) = (counters.players, counters.free);
        crate::land_pool::read_pool(&mut records, &dir, &mut next, &mut entry, &mut random).unwrap();
        crate::land_managers::draw_order(records.globals.humans() as usize, &mut random);
        assert_eq!(random, Random::from_state(1_513_196_775, 1_935_620_346));
    }
}
