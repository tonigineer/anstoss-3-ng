//! A new game's players, read from a country's `Land*.sav` into the
//! records (`FUN_00687e7c` for the main country and each chosen one, from
//! `FUN_00686684`).
//!
//! Each club's `SPIELER` sections are read into a scratch player of 0x7e
//! bytes (`FUN_0068c257`), the club lists them by main position, keeper
//! first, the file's order within one (`FUN_0068a38c`), and each takes the
//! next id and its record (`FUN_0068c903`). The names go into the pool,
//! each once (`FUN_0068b4de`).
//!
//! Of a country not played (`FUN_00676c03` 0) only eight clubs are read,
//! for the European cups: those `Kleinig.sav` names for it first, then the
//! first league's by their players' average strength.

use std::collections::HashMap;
use std::io;
use std::path::Path;

use a3_files::sav::Section;

use crate::newplayer::COUNTRY_CLASS;
use crate::random::Random;
use crate::records::{Player, Records};
use crate::world::{abilities, find, GOALKEEPER, LAND_FILES, POSITION_CODES, STRIKER};

/// The values of a `SPIELER`, by line, as `FUN_0068c257` reads them; lines
/// 2 and 5 it reads and drops.
const LAST_NAME: usize = 0;
const FIRST_NAME: usize = 1;
const FACE: [usize; 2] = [3, 4];
const STRENGTH: usize = 6;
const NATION: usize = 7;
const POSITIONS: usize = 8;
const GOOD: usize = 11;
const BAD: usize = 12;
const TRAITS: usize = 13;
const NICKNAME_SHOWN: usize = 15;
const NICKNAME: usize = 16;
const FOOT: usize = 17;
const TALENT: usize = 18;
const KNOCK: usize = 19;
const FANS: usize = 20;
const BIRTHDAY: usize = 21;
const BYTE_27D: usize = 22;
const SECOND_NATION: usize = 23;
const MARKS: usize = 24;
const QUALITIES: usize = 25;
const BIT_20: usize = 26;
const NO_CAPTAIN: usize = 27;
const SHIRT: usize = 28;
/// The face's other two parts: the low nibble of the first byte and the
/// third byte.
const FACE_MORE: usize = 29;
/// The scratch player's texts: last name and nickname 0x1d bytes, the
/// first name 0x13 (`strncpy`).
const LONG_TEXT: usize = 0x1d;
const SHORT_TEXT: usize = 0x13;
/// The highest nation code; above, the file's nation is not one.
const LAST_NATION: u8 = 0x74;
const HOME_GROWN: u8 = 0x80;
/// Condition and freshness start drawn: 35 to 51, 60 to 81 (in hundredths),
/// ten more for the fit, ten less for the unfit.
const CONDITION: (u32, i16) = (0x640, 0xdac);
const FRESHNESS: (u32, i16) = (0x834, 6000);
const FIT_BY: i16 = 1000;
/// Contentment, scruples and morale start middling.
const CONTENT_AT: usize = 0x28f;
const MIDDLING: u8 = 100;

/// A player as `FUN_0068c257` reads him from his `SPIELER`, by the scratch
/// player's offsets.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scratch {
    /// +0x16, +0x2, +0x34.
    pub last_name: String,
    pub first_name: String,
    pub nickname: String,
    /// +0x53, +0x54, +0x56, +0x55.
    pub face: [u8; 4],
    /// +0x57: 1 to 12.
    pub strength: u8,
    /// +0x68 (bit 7: home-grown), +0x69.
    pub nation: u8,
    pub second_nation: u8,
    /// +0x58 to +0x5a, in the game's numbering.
    pub positions: [u8; 3],
    /// +0x5c, +0x5e, +0x60, +0x62, +0x70.
    pub good: u16,
    pub bad: u16,
    pub traits: [u16; 2],
    pub qualities: u16,
    /// +0x52, +0x6b, +0x67, +0x6c, +0x6d, +0x6e, +0x6f, +0x72, +0x73, +0x74.
    pub nickname_shown: u8,
    pub foot: u8,
    pub talent: i8,
    pub knock: u8,
    pub fans: u8,
    pub byte_27d: u8,
    pub marks: u8,
    pub bit_20: u8,
    pub no_captain: u8,
    pub shirt: u8,
    /// +0x7a: day, month, year.
    pub birthday: (u8, u8, u16),
}

impl Scratch {
    /// `FUN_0068c257`. A nation above the last is the scratch player's
    /// +0x65, which nothing sets before: none.
    pub fn read(spieler: &Section) -> Self {
        let long = |i: usize| spieler.number(i).unwrap_or(0);
        let byte = |i: usize| long(i) as u8;
        let text = |i: usize, len: usize| -> String {
            spieler.text(i).unwrap_or_default().chars().take(len).collect()
        };
        let mut strength = byte(STRENGTH);
        if strength == 0 {
            strength = 1;
        }
        let mut nation = byte(NATION);
        if nation & 0x7f > LAST_NATION {
            nation = 0;
        }
        // The files number positions otherwise than the game (`DAT_008a3d9b`).
        let code = |p: u8| POSITION_CODES[usize::from(p)];
        let mut main = byte(POSITIONS);
        if main == 0 || main > STRIKER {
            main = GOALKEEPER;
        }
        let side = |p: u8| if (2..=STRIKER).contains(&p) { code(p) } else { 0 };
        let (main, mut second, mut third) =
            (code(main), side(byte(POSITIONS + 1)), side(byte(POSITIONS + 2)));
        if second == third || main == third {
            third = 0;
        }
        if main == second {
            second = 0;
        }
        let [good, bad] = abilities(long(GOOD), long(BAD));
        let face_more = long(FACE_MORE) as u32;
        let mut knock = byte(KNOCK);
        if knock == 0 {
            knock = 1;
        }
        let fans = byte(FANS);
        let mut birthday = (0, 0, 0);
        let date = text(BIRTHDAY, usize::MAX);
        let mut parts = date.trim().split('.').map(|p| p.trim().parse::<u32>().unwrap_or(0));
        if let (Some(d), Some(m), Some(y)) = (parts.next(), parts.next(), parts.next()) {
            birthday = (d as u8, m as u8, y as u16);
        }
        Self {
            last_name: text(LAST_NAME, LONG_TEXT),
            first_name: text(FIRST_NAME, SHORT_TEXT),
            nickname: text(NICKNAME, LONG_TEXT),
            face: [byte(FACE[0]), byte(FACE[1]), (face_more & 0xf) as u8, (face_more >> 16) as u8],
            strength: strength.min(12),
            nation,
            second_nation: byte(SECOND_NATION).min(LAST_NATION),
            positions: [main, second, third],
            good,
            bad,
            traits: [(long(TRAITS) >> 1) as u16, (long(TRAITS + 1) >> 1) as u16],
            qualities: (long(QUALITIES) >> 1) as u16,
            nickname_shown: byte(NICKNAME_SHOWN),
            foot: byte(FOOT),
            talent: byte(TALENT).wrapping_sub(1) as i8,
            knock: knock.min(7),
            fans: if fans > 3 { 0 } else { fans },
            byte_27d: byte(BYTE_27D),
            marks: byte(MARKS),
            bit_20: byte(BIT_20),
            no_captain: byte(NO_CAPTAIN),
            shirt: byte(SHIRT),
            birthday,
        }
    }

    /// `FUN_0068c903`: the record of a player read from the files, his
    /// names put into the pool; his club the team's part sets.
    pub fn fill(&self, player: &mut Player, pool: &mut Pool, random: &mut Random) {
        player.set_last_name(pool.index(&self.last_name));
        player.set_first_name(pool.index(&self.first_name));
        let [a, b, c, d] = self.face;
        player.set_face([a.wrapping_add(c << 4), b.wrapping_add(d << 4)]);
        player.set_strength(self.strength);
        player.set_nation(self.nation);
        player.put::<u8>(0x227, COUNTRY_CLASS[usize::from(self.nation & 0x7f)]);
        player.set_positions(self.positions);
        player.set_good(self.good);
        player.set_bad(self.bad);
        player.set_traits(self.traits);
        for at in CONTENT_AT..CONTENT_AT + 8 {
            player.put(at, MIDDLING);
        }
        let mut condition = random.below(CONDITION.0) as i16 + CONDITION.1;
        let mut freshness = random.below(FRESHNESS.0) as i16 + FRESHNESS.1;
        let [character, more] = self.traits;
        if character & 4 != 0 || more & 0x20 != 0 {
            condition -= FIT_BY;
            freshness -= FIT_BY;
        } else if character & 2 != 0 || more & 0x101 != 0 {
            condition += FIT_BY;
            freshness += FIT_BY;
        }
        player.set_condition(condition);
        let mut fresh = player.freshness();
        fresh[0] = freshness;
        player.set_freshness(fresh);
        player.set_club([0, 0]);
        if self.nickname_shown != 0 {
            player.set_shown(player.shown() | 0x8000_0000);
        }
        player.set_nickname(pool.index(&self.nickname));
        player.set_talent(self.talent);
        player.set_knocks(1 << (self.knock - 1));
        let (day, month, year) = self.birthday;
        player.set_birth_day(day);
        player.set_birth_month(month);
        player.set_birth_year(year);
        player.put(0x27d, self.byte_27d);
        player.set_second_nation(self.second_nation);
        let mut manner = self.fans;
        match self.fans {
            2 => player.set_flags(player.flags() | 0x10000),
            3 => player.set_flags(player.flags() | 0x400),
            _ => {}
        }
        manner |= self.foot << 2;
        if self.marks & 1 != 0 {
            manner |= 0x80;
        }
        if self.marks & 2 != 0 {
            player.set_retiring(player.retiring() | 2);
        }
        if self.no_captain != 0 {
            manner |= 0x40;
        }
        if self.bit_20 != 0 {
            manner |= 0x20;
        }
        if self.nation & HOME_GROWN != 0 {
            manner |= 0x10;
        }
        player.set_manner(manner);
        player.set_nation(self.nation & 0x7f);
        player.set_qualities(self.qualities);
        player.put::<u8>(0x164, 0);
        player.put::<u16>(0x166, 0xffff);
        player.set_shirt(self.shirt);
    }
}

/// The name pool as `FUN_0068b4de` fills it: a name trimmed
/// (`FUN_006857a1`), none 0, one there already its index, else the next.
pub struct Pool<'a> {
    names: &'a mut Vec<String>,
    index: HashMap<String, u16>,
}

impl<'a> Pool<'a> {
    /// The pool as the save holds it: the empty name after the last NUL is
    /// none.
    pub fn new(names: &'a mut Vec<String>) -> Self {
        if names.last().is_some_and(String::is_empty) {
            names.pop();
        }
        let mut index = HashMap::new();
        for (i, name) in names.iter().enumerate() {
            index.entry(name.clone()).or_insert(i as u16);
        }
        Self { names, index }
    }

    pub fn index(&mut self, name: &str) -> u16 {
        let name = trim(name);
        if name.is_empty() {
            return 0;
        }
        if let Some(&i) = self.index.get(&name) {
            return i;
        }
        let i = self.names.len() as u16;
        self.names.push(name.clone());
        self.index.insert(name, i);
        i
    }
}

/// `FUN_006857a1`: from the first character above a blank up to the first
/// control character, without the blanks at the end.
fn trim(name: &str) -> String {
    let start = name.trim_start_matches(|c: char| c <= ' ');
    let text: String = start.chars().take_while(|&c| c >= ' ').collect();
    text.trim_end_matches(' ').to_owned()
}

/// How many clubs of a country not played are read.
const FOREIGN_CLUBS: usize = 8;
/// `Kleinig.sav`'s `MISC` / `UEFA`: 37 rows of seven values, then three
/// clubs the European cups take of a country not played, each its nation
/// (a byte) and its number from 1 (`DAT_00ae6dd4`, read in
/// `FUN_00686684`).
const UEFA_FIXED_AT: usize = 37 * 7;
const UEFA_FIXED: usize = 3;

/// The clubs of a country's file: in `LAND`, or `BONUSLAND` in the bonus
/// file.
fn clubs(land: &Section) -> Vec<&Section> {
    let clubs = land.child("LAND").or_else(|| land.child("BONUSLAND")).unwrap_or(land);
    clubs.all("VEREIN").collect()
}

/// A club's players in the order its team lists them (`FUN_0068a38c`): by
/// main position from the keeper, the file's order within one.
pub fn squad(verein: &Section) -> Vec<Scratch> {
    let read: Vec<Scratch> = verein.all("SPIELER").map(Scratch::read).collect();
    (GOALKEEPER..=STRIKER)
        .flat_map(|position| read.iter().filter(move |p| p.positions[0] == position).cloned())
        .collect()
}

/// Every club's players of a country's file into the records from `next`
/// on (`DAT_00bde19c`), in the file's order of the clubs: each club's ids.
pub fn read_country(records: &mut Records, land: &Section, next: &mut usize, random: &mut Random) -> Vec<Vec<u16>> {
    let all: Vec<usize> = (0..clubs(land).len()).collect();
    read_clubs(records, land, &all, next, random)
}

/// The clubs `Kleinig.sav` names for the European cups of a country not
/// played: nation and club index (`DAT_00ae6dd4`).
pub fn uefa_fixed(kleinig: &Section) -> Vec<(u8, usize)> {
    let Some(uefa) = kleinig.child("MISC").and_then(|misc| misc.child("UEFA")) else { return Vec::new() };
    (0..UEFA_FIXED)
        .map(|i| {
            let value = |k: usize| uefa.number(UEFA_FIXED_AT + 2 * i + k).unwrap_or(0);
            let club = value(1) as i16;
            (value(0) as u8, if club > 0 { club - 1 } else { club } as usize)
        })
        .collect()
}

/// The eight clubs of a country not played whose players are read, in
/// their order (`FUN_00687e7c`): those `fixed` names for its nation, then
/// the first league's others by their players' average strength in
/// hundredths, the best first, the league's order among equals; a club
/// without players counts nought.
pub fn foreign_clubs(land: &Section, nation: u8, first_league: &[usize], fixed: &[(u8, usize)]) -> Vec<usize> {
    let clubs = clubs(land);
    let mut taken: Vec<usize> = fixed.iter().filter(|&&(n, _)| n == nation).map(|&(_, club)| club).collect();
    let fixed_count = taken.len();
    let mut ranked: Vec<(i16, usize)> = Vec::new();
    for &club in first_league {
        if taken.contains(&club) {
            continue;
        }
        taken.push(club);
        let strengths: Vec<u8> = clubs.get(club).map_or_else(Vec::new, |verein| {
            verein.all("SPIELER").map(|p| Scratch::read(p).strength).collect()
        });
        let sum: i32 = strengths.iter().map(|&s| i32::from(s)).sum();
        if sum < 1 {
            ranked.push((0, club));
            continue;
        }
        let average = (sum * 100 / strengths.len() as i32) as i16;
        let at = ranked.iter().rposition(|&(v, _)| v >= average).map_or(0, |i| i + 1);
        ranked.insert(at, (average, club));
    }
    let mut chosen: Vec<usize> = taken[..fixed_count].to_vec();
    chosen.extend(ranked.iter().take(FOREIGN_CLUBS.saturating_sub(fixed_count)).map(|&(_, club)| club));
    chosen
}

/// The players of some clubs of a country's file into the records from
/// `next` on, club by club in the order given: each club's ids.
pub fn read_clubs(
    records: &mut Records,
    land: &Section,
    chosen: &[usize],
    next: &mut usize,
    random: &mut Random,
) -> Vec<Vec<u16>> {
    let clubs = clubs(land);
    let mut pool = Pool::new(&mut records.names);
    chosen
        .iter()
        .map(|&club| {
            squad(clubs[club])
                .iter()
                .map(|scratch| {
                    let id = *next;
                    let player = &mut records.players[id];
                    scratch.fill(player, &mut pool, random);
                    player.set_id(id as u16);
                    *next += 1;
                    id as u16
                })
                .collect()
        })
        .collect()
}

/// The country file of a slot (`Data.a3`'s `Land*.sav`), read into the
/// records as [`read_country`] does.
pub fn read_slot(
    records: &mut Records,
    dir: &Path,
    slot: usize,
    next: &mut usize,
    random: &mut Random,
) -> io::Result<Vec<Vec<u16>>> {
    let land = Section::read(&find(dir, LAND_FILES[slot])?)?;
    Ok(read_country(records, &land, next, random))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spieler(lines: &[&str]) -> Section {
        Section { name: "SPIELER".into(), values: lines.iter().map(|s| s.to_string()).collect(), children: vec![] }
    }

    #[test]
    fn a_spieler_reads_as_the_original() {
        // Robin Zentner of LandDeut.sav.
        let lines = [
            "Zentner ", "Robin", "0", "0", "0", "30", "8", "14", "1", "0", "0", "4", "0", "0", "4096", "0", "",
            "1", "3", "1", "1", "28.10.1994", "0", "0", "0", "0", "0", "0", "27", "196610",
        ];
        let scratch = Scratch::read(&spieler(&lines));
        assert_eq!(scratch.positions, [1, 0, 0]);
        assert_eq!((scratch.good, scratch.traits), (2, [0, 2048]));
        assert_eq!(scratch.face, [0, 0, 2, 3]);
        let mut records = Records::blank();
        let mut pool = Pool::new(&mut records.names);
        let mut player = records.players[1].clone();
        scratch.fill(&mut player, &mut pool, &mut Random::new(1));
        assert_eq!(records.names, ["Zentner", "Robin"]);
        // The pool keeps each name once.
        let mut pool = Pool::new(&mut records.names);
        assert_eq!((pool.index(" Robin  "), pool.index("Batz")), (1, 2));
        assert_eq!((player.last_name(), player.first_name(), player.nickname()), (0, 1, 0));
        assert_eq!(player.face(), [32, 48]);
        assert_eq!((player.talent(), player.knocks(), player.manner(), player.shirt()), (2, 1, 5, 27));
        assert_eq!(player.birthday(), chrono::NaiveDate::from_ymd_opt(1994, 10, 28));
        assert_eq!((player.get::<u8>(0x227), player.get::<u16>(0x166)), (0, 0xffff));
    }

    #[test]
    fn a_club_lists_its_players_by_position() {
        let player = |name: &str, position: &str| {
            let mut lines = vec![name, "X", "0", "0", "0", "0", "5", "14", position];
            lines.resize(30, "0");
            spieler(&lines)
        };
        let verein = Section {
            name: "VEREIN".into(),
            values: vec![],
            children: vec![player("A", "10"), player("B", "1"), player("C", "6"), player("D", "1")],
        };
        let names: Vec<String> = squad(&verein).into_iter().map(|p| p.last_name).collect();
        // The files' 6 is the game's 8.
        assert_eq!(names, ["B", "D", "C", "A"]);
    }

    /// What of a player the original's first turn holds otherwise than the
    /// files gave him: his club (the clubs' part), condition and freshness
    /// (drawn after the start settings' draws), and what the season start
    /// changes (new-game-exact-squads): some players' morale (90), two
    /// players' shirts.
    const APART: [&str; 4] = ["club", "condition", "morale", "shirt"];

    /// The players of the oracle's new game in Germany
    /// (`tools/oracle/new-game.sh`) as the files give them — all of the
    /// main country's, eight clubs of each other country —: the first turn
    /// holds them in the same order, a thousand and fourteen ids on (the
    /// free players stand before them by then).
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR and the oracle's dumps from $ANSTOSS3_ORACLE"]
    fn a_new_games_players_read_as_the_originals() {
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
        let mut next = 1;
        let mut random = Random::new(1);
        let dir = std::path::PathBuf::from(game).join("Data.a3");
        let clubs = read_slot(&mut records, &dir, 0, &mut next, &mut random).unwrap();
        assert_eq!(clubs.len(), 108);
        // Each country's ids from where they begin.
        let mut segments = vec![1..next];
        let world = crate::world::World::load(&dir).unwrap();
        let fixed = uefa_fixed(&Section::read(&find(&dir, "Kleinig.sav").unwrap()).unwrap());
        assert_eq!(fixed, [(20, 5), (57, 2), (16, 9)]);
        for slot in 1..11 {
            let country = world.country(slot).unwrap();
            let land = Section::read(&find(&dir, LAND_FILES[slot]).unwrap()).unwrap();
            let chosen = foreign_clubs(&land, country.nation as u8, &country.leagues[0].clubs, &fixed);
            assert_eq!(chosen.len(), FOREIGN_CLUBS);
            let from = next;
            read_clubs(&mut records, &land, &chosen, &mut next, &mut random);
            segments.push(from..next);
        }
        let name = |r: &Records, p: &Player| (r.name(p.first_name()).to_owned(), r.name(p.last_name()).to_owned());
        let mut apart = std::collections::BTreeMap::<&str, Vec<String>>::new();
        // The first turn holds a country's players in the same order, from
        // elsewhere: the main country's a thousand and fourteen ids on.
        let mut offsets = vec![];
        for segment in &segments {
            let first = &records.players[segment.start];
            let at = (1..dump.players.len())
                .find(|&id| {
                    let o = &dump.players[id];
                    name(&dump, o) == name(&records, first) && o.birthday() == first.birthday()
                })
                .expect("a country's first player in the dump");
            offsets.push(at - segment.start);
        }
        eprintln!("offsets by country {offsets:?}");
        assert_eq!(offsets[0], 1014);
        for id in 1..next {
            let offset = offsets[segments.iter().position(|s| s.contains(&id)).unwrap()];
            let (r, o) = (&records.players[id], &dump.players[id + offset]);
            let mut differ = |what: &'static str, a: String, b: String| {
                if a != b {
                    apart.entry(what).or_default().push(format!("{id} {:?}: {a}, not {b}", name(&records, r)));
                }
            };
            let nickname = |records: &Records, p: &Player| match p.nickname() {
                0 => String::new(),
                i => records.name(i).to_owned(),
            };
            differ("names", format!("{:?} {}", name(&records, r), nickname(&records, r)), format!("{:?} {}", name(&dump, o), nickname(&dump, o)));
            differ("face", format!("{:?}", r.face()), format!("{:?}", o.face()));
            differ("strength", r.strength().to_string(), o.strength().to_string());
            differ("nations", format!("{} {} {}", r.nation(), r.second_nation(), r.get::<u8>(0x227)), format!("{} {} {}", o.nation(), o.second_nation(), o.get::<u8>(0x227)));
            differ("positions", format!("{:?}", r.positions()), format!("{:?}", o.positions()));
            differ("abilities", format!("{:x} {:x}", r.good(), r.bad()), format!("{:x} {:x}", o.good(), o.bad()));
            differ("traits", format!("{:?} {:x}", r.traits(), r.qualities()), format!("{:?} {:x}", o.traits(), o.qualities()));
            differ("talent", r.talent().to_string(), o.talent().to_string());
            differ("knocks", format!("{:x}", r.knocks()), format!("{:x}", o.knocks()));
            differ("birthday", format!("{:?}", r.birthday()), format!("{:?}", o.birthday()));
            differ("shirt", r.shirt().to_string(), o.shirt().to_string());
            differ("manner", format!("{:x}", r.manner()), format!("{:x}", o.manner()));
            const FANS: u32 = 0x10400;
            differ("flags", format!("{:x} {:x}", r.flags() & FANS, r.shown() >> 31), format!("{:x} {:x}", o.flags() & FANS, o.shown() >> 31));
            differ("marks", format!("{} {:x}", r.get::<u8>(0x27d), r.retiring() & 2), format!("{} {:x}", o.get::<u8>(0x27d), o.retiring() & 2));
            differ("began", format!("{} {:x}", r.get::<u8>(0x164), r.get::<u16>(0x166)), format!("{} {:x}", o.get::<u8>(0x164), o.get::<u16>(0x166)));
            differ("contentment", format!("{:?}", r.contentment()), format!("{:?}", o.contentment()));
            differ("morale", r.morale().to_string(), o.morale().to_string());
            differ("club", format!("{:?}", r.club()), format!("{:?}", o.club()));
            differ("condition", format!("{} {:?}", r.condition(), r.freshness()), format!("{} {:?}", o.condition(), o.freshness()));
        }
        for (what, list) in &apart {
            eprintln!("{what}: {} apart, {:?}", list.len(), &list[..list.len().min(4)]);
        }
        for what in apart.keys() {
            assert!(APART.contains(what), "{what} parts from the original's first turn: {:?}", &apart[what][..apart[what].len().min(4)]);
        }
    }
}
