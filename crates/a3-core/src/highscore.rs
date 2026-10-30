//! The installation's highscore lists (`Highscore/high%02d`, ERFOLGE.md
//! §A.5): a career that ends goes into the list of the game's mode
//! (`FUN_00656e9b`) — the five best careers of the career mode in
//! `high09`, the best game of each fixed number of seasons in `high00` to
//! `high08`. A game played on past its end keeps none.

use crate::records::Manager;

/// The game's mode for a career, and for a game played on without an end
/// (`FUN_006571c5`, `FUN_006fceff`).
pub const CAREER: i16 = 9;
pub const ENDLESS: i16 = 10;
/// An entry's bytes.
const ENTRY: usize = 0x30;
/// How many entries each mode's list holds (`0x89f25c`).
const COUNTS: [usize; 10] = [1, 1, 1, 1, 1, 1, 1, 1, 1, 5];
/// The seasons each fixed mode lasts, which its entries count by
/// (`0x884420`).
const SEASONS: [i32; 9] = [1, 2, 3, 4, 5, 10, 20, 50, 100];
/// The lists' files in the game's folder, by mode.
const FILES: [&str; 10] = [
    "Highscore/high00",
    "Highscore/high01",
    "Highscore/high02",
    "Highscore/high03",
    "Highscore/high04",
    "Highscore/high05",
    "Highscore/high06",
    "Highscore/high07",
    "Highscore/high08",
    "Highscore/high09",
];
/// Where an entry keeps its fields.
const FIRST_NAME: usize = 0;
const LAST_NAME: usize = 0xa;
const AGE: usize = 0x1a;
const YEARS: usize = 0x1c;
const POINTS: usize = 0x20;
const LEVEL: usize = 0x24;
const TITLES: usize = 0x26;
const DATE: usize = 0x2a;

/// What an ending career brings to its list.
pub struct Career<'a> {
    pub manager: &'a Manager,
    /// The sum of his nine marks (`FUN_006dda52`).
    pub points: i32,
    /// His age today, which a career's entry shows.
    pub age: i32,
    /// The day it ends, as the computer's clock has it: day, month, year.
    pub date: (u8, u8, u16),
}

/// The file of the list for the game's `mode`, if it keeps one.
pub fn file(mode: i16) -> Option<&'static str> {
    FILES.get(usize::try_from(mode).ok()?).copied()
}

/// The list of the game's `mode` with `career` entered, from `list` as its
/// file holds it (`FUN_00656e9b`). The career takes the place of the first
/// entry with fewer points, or none; the list comes back whole either way,
/// as the game writes it back unchanged when the career is not good
/// enough. The entries below move down a place, all but the one the
/// career lands on: the game's loop stops a place short, so that one is
/// lost and the one under it stands twice. The names are copied up to
/// their ends only, what the place held after them stays. A manager whose
/// game is played on past its end, or a game in no mode with a list,
/// enters nothing.
pub fn enter(mode: i16, list: &[u8], career: &Career) -> Option<Vec<u8>> {
    let manager = career.manager;
    if manager.mode() == ENDLESS {
        return None;
    }
    let count = *COUNTS.get(usize::try_from(mode).ok()?)?;
    let mut table = vec![0; count * ENTRY];
    let read = list.len().min(table.len());
    table[..read].copy_from_slice(&list[..read]);
    let points = |table: &[u8], i: usize| -> i32 {
        i32::from_le_bytes(table[i * ENTRY + POINTS..][..4].try_into().unwrap())
    };
    let Some(at) = (0..count).find(|&i| points(&table, i) < career.points) else {
        return Some(table);
    };
    for i in (at + 1..count.saturating_sub(1)).rev() {
        table.copy_within(i * ENTRY..(i + 1) * ENTRY, (i + 1) * ENTRY);
    }
    let entry = &mut table[at * ENTRY..(at + 1) * ENTRY];
    let raw = manager.bytes();
    copy_name(&mut entry[FIRST_NAME..], &raw[0..0xa]);
    copy_name(&mut entry[LAST_NAME..], &raw[0xa..0x1c]);
    // A career counts the manager's age and his years as a trainer; a
    // fixed game its seasons in both.
    let (age, years) = if manager.mode() == CAREER {
        (career.age, i32::from(manager.trainer_years()))
    } else {
        let seasons = usize::try_from(manager.mode()).ok().and_then(|m| SEASONS.get(m));
        (seasons.copied().unwrap_or(0), seasons.copied().unwrap_or(0))
    };
    entry[AGE..AGE + 2].copy_from_slice(&(age as i16).to_le_bytes());
    entry[YEARS..YEARS + 4].copy_from_slice(&years.to_le_bytes());
    entry[POINTS..POINTS + 4].copy_from_slice(&career.points.to_le_bytes());
    // The level twice over, from the same call (`FUN_006a61e5`).
    let level = manager.standing() as u8;
    entry[LEVEL] = level;
    entry[LEVEL + 1] = level;
    let [titles, more] = manager.titles();
    entry[TITLES..TITLES + 2].copy_from_slice(&titles.to_le_bytes());
    entry[TITLES + 2..TITLES + 4].copy_from_slice(&more.to_le_bytes());
    let (day, month, year) = career.date;
    entry[DATE] = day;
    entry[DATE + 1] = month;
    entry[DATE + 2..DATE + 4].copy_from_slice(&year.to_le_bytes());
    Some(table)
}

/// A name as `strcpy` copies it: up to its end and the zero after it.
fn copy_name(to: &mut [u8], name: &[u8]) {
    let len = name.iter().position(|&b| b == 0).unwrap_or(name.len()).min(to.len() - 1);
    to[..len].copy_from_slice(&name[..len]);
    to[len] = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A manager named Toni Lubiniecki of the career mode, with his level
    /// and one title of each kind.
    fn manager(mode: i16) -> Manager {
        let mut m = Manager::new(&vec![0; Manager::SIZE]).unwrap();
        m.put(0, *b"Toni\0");
        m.put(0xa, *b"Lubiniecki\0");
        m.set_mode(mode);
        m.set_trainer_years(3);
        m.set_level(9);
        m.set_titles([1, 2]);
        m
    }

    /// An entry of `points`, its first name followed by a leftover.
    fn entry(points: i32) -> [u8; ENTRY] {
        let mut e = [0; ENTRY];
        e[..8].copy_from_slice(b"Ann\0old\0");
        e[POINTS..POINTS + 4].copy_from_slice(&points.to_le_bytes());
        e
    }

    fn points(table: &[u8]) -> Vec<i32> {
        table.chunks(ENTRY).map(|e| i32::from_le_bytes(e[POINTS..POINTS + 4].try_into().unwrap())).collect()
    }

    #[test]
    fn a_career_takes_the_place_of_the_first_with_fewer_points() {
        let m = manager(CAREER);
        let career = Career { manager: &m, points: 450, age: 41, date: (30, 10, 2026) };
        let list: Vec<u8> = [900, 500, 400, 300, 200].iter().flat_map(|&p| entry(p)).collect();
        let table = enter(CAREER, &list, &career).unwrap();
        // 400 is lost and 300 stands twice: the game moves all but the
        // place it fills.
        assert_eq!(points(&table), [900, 500, 450, 300, 300]);
        let e = &table[2 * ENTRY..3 * ENTRY];
        assert_eq!(&e[..8], b"Toni\0ld\0");
        assert_eq!(&e[LAST_NAME..LAST_NAME + 11], b"Lubiniecki\0");
        assert_eq!(i16::from_le_bytes([e[AGE], e[AGE + 1]]), 41);
        assert_eq!(e[YEARS], 3);
        assert_eq!((e[LEVEL], e[LEVEL + 1]), (9, 9));
        assert_eq!(&e[TITLES..TITLES + 4], [1, 0, 2, 0]);
        assert_eq!(&e[DATE..DATE + 4], [30, 10, 0xea, 0x07]);
    }

    #[test]
    fn a_career_not_good_enough_leaves_the_list() {
        let m = manager(CAREER);
        let career = Career { manager: &m, points: 200, age: 41, date: (30, 10, 2026) };
        let list: Vec<u8> = [900, 500, 400, 300, 200].iter().flat_map(|&p| entry(p)).collect();
        assert_eq!(enter(CAREER, &list, &career).unwrap(), list);
        // No file yet: an empty list, which the career heads.
        let table = enter(CAREER, &[], &career).unwrap();
        assert_eq!(points(&table), [200, 0, 0, 0, 0]);
    }

    #[test]
    fn a_fixed_game_counts_its_seasons_and_a_game_played_on_nothing() {
        let m = manager(5);
        let career = Career { manager: &m, points: 10, age: 41, date: (30, 10, 2026) };
        let table = enter(5, &[], &career).unwrap();
        assert_eq!(table.len(), ENTRY);
        assert_eq!((table[AGE], table[YEARS]), (10, 10));
        assert_eq!(file(5), Some("Highscore/high05"));
        let on = manager(ENDLESS);
        let career = Career { manager: &on, ..career };
        assert_eq!(enter(5, &[], &career), None);
        assert_eq!(file(ENDLESS), None);
    }
}
