//! What a club's table place and season do to its managers' popularity
//! with the public, the fans and the media, the board's confidence and
//! the trainer's season marks (ERFOLGE.md §B), by two walks over every
//! club:
//!
//! - the champions' day's (`FUN_0041773c`, half week 0x5c): the leader of
//!   a first league gains 15/20/5 and 20 of confidence and 150 marks
//!   (`FUN_006dd899`), of a lower one 5/20/5, 14 and 60 marks;
//! - the season end's (`FUN_0041786a`, the scheduler's case 5): the
//!   relegated (+0x110 bit 8) lose 20/20/5, 15 and 50 marks, bit 0x10
//!   gains 3/6/2, 5 and 50 marks, the promoted (bit 4) 50 marks; then the
//!   club's signed count at team +0x137 (club +0x10e) moves them by its
//!   bands, and the marks take four for every place above the league's
//!   size (`FUN_00635a8a`).
//!
//! The nerves the champions' day takes from a runner-up and the clubs
//! under their league's mark (`FUN_006be8b8`, `FUN_00677d72`) and the
//! trainer's season block the season end books (`FUN_0075489e`,
//! `FUN_00754954`) are not built.

use crate::exhibition::move_standing;
use crate::records::Records;

const FLAGS: usize = 0x110;
const PROMOTED: u16 = 0x4;
const RELEGATED: u16 = 0x8;
const BIT_10: u16 = 0x10;
/// The club object's +0x10e, signed.
const COUNT: usize = 0x137;

fn clubs(records: &Records) -> Vec<(u8, i16, u8, u8)> {
    records.teams.iter().map(|t| (t.country(), i16::from(t.number()), t.level(), t.table_place())).collect()
}

/// `FUN_006dd987`: the trainer's season marks (his record +0x204), unclamped.
fn marks(records: &mut Records, club: (u8, i16), by: i32) {
    let Some(t) = u16::try_from(club.1).ok().and_then(|n| records.team(club.0.into(), n)) else { return };
    let trainer = t.humans()[0];
    if let Some(m) = usize::from(trainer).checked_sub(1).and_then(|i| records.managers.get_mut(i)) {
        m.set_season_marks(m.season_marks().wrapping_add(by));
    }
}

/// The champions' day's walk (`FUN_0041773c`).
pub fn champions_day(records: &mut Records) {
    for (country, number, level, place) in clubs(records) {
        if place != 1 {
            continue;
        }
        let club = (country, number);
        if level == 0 {
            move_standing(records, club, [15, 20, 5], 20);
            marks(records, club, 150);
        } else {
            move_standing(records, club, [5, 20, 5], 14);
            marks(records, club, 60);
        }
    }
}

/// The league's size the season end's marks count against
/// (`FUN_00635a8a` → `FUN_00635a35`): eighteen for an unplayed country or
/// the sixth level and below, else the league block's word +0x50.
fn league_size(records: &Records, country: u8, level: u8) -> i32 {
    if !records.globals.played(i16::from(country)) || level >= 6 {
        return 18;
    }
    let li = records.countries.first_league(usize::from(country)) + i16::from(level);
    let at = crate::opponent::LEAGUE_FIELDS;
    records
        .block(&format!("0x1a09d48[{li}]"))
        .and_then(|b| b.get(at..at + 2))
        .map_or(18, |w| i32::from(i16::from_le_bytes([w[0], w[1]])))
}

/// The bands of the club's count at +0x137.
fn count_band(count: i8) -> [i32; 3] {
    match count {
        i8::MIN..=-4 => [-6, -10, -3],
        -3..=-2 => [-3, -6, -2],
        -1 => [-1, -2, -1],
        0 => [0, 0, 0],
        1 => [1, 2, 1],
        2..=3 => [3, 6, 2],
        _ => [6, 10, 3],
    }
}

/// The season end's walk (`FUN_0041786a`).
pub fn season_end(records: &mut Records) {
    for (country, number, level, place) in clubs(records) {
        let club = (country, number);
        let Some(t) = u16::try_from(number).ok().and_then(|n| records.team(country.into(), n)) else { continue };
        let (flags, count) = (t.get::<u16>(FLAGS), t.get::<i8>(COUNT));
        if flags & RELEGATED != 0 {
            move_standing(records, club, [-20, -20, -5], -15);
            marks(records, club, -50);
        }
        if flags & BIT_10 != 0 {
            move_standing(records, club, [3, 6, 2], 5);
            marks(records, club, 50);
        }
        if flags & PROMOTED != 0 {
            marks(records, club, 50);
        }
        let band = count_band(count);
        if band != [0, 0, 0] {
            move_standing(records, club, band, 0);
        }
        let size = league_size(records, country, level);
        marks(records, club, (size - i32::from(place)) * 4);
    }
}

#[cfg(test)]
mod saves {
    use super::*;
    use a3_files::an3::Save;

    fn read(name: &str) -> Records {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        Records::from_save(Save::read(&dir.parent().unwrap().join("saves-kept").join(name)).unwrap()).unwrap()
    }

    /// A's human club put at the top of its first league on the champions'
    /// day: its manager gains 15/20/5 with the public, the fans and the
    /// media and 20 of confidence, each within 100, and 150 season marks;
    /// relegated at the season end with a count of −2 at +0x137, he loses
    /// 23/26/7 and 15 of confidence and gains (size − place)·4 − 50 marks.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_leader_and_the_relegated_move_their_managers_standing() {
        let mut records = read("A.an3");
        let i = records.teams.iter().position(|t| t.humans()[0] != 0).expect("A holds a human club");
        let human = usize::from(records.teams[i].humans()[0]) - 1;
        for t in records.teams.iter_mut() {
            if t.table_place() == 1 {
                t.set_table_place(2);
            }
        }
        records.teams[i].set_table_place(1);
        records.teams[i].set_level(0);
        let (p, c, m) = (records.managers[human].popularity(), records.managers[human].confidence(), records.managers[human].season_marks());
        champions_day(&mut records);
        let x = &records.managers[human];
        let step = |v: u8, by: i32| (i32::from(v) + by).clamp(0, 100) as u8;
        assert_eq!(x.popularity(), [step(p[0], 15), step(p[1], 20), step(p[2], 5)]);
        assert_eq!(x.confidence(), (c + 20).clamp(0, 100));
        assert_eq!(x.season_marks(), m + 150);

        let (p, c, m) = (x.popularity(), x.confidence(), x.season_marks());
        let t = &mut records.teams[i];
        t.put::<u16>(FLAGS, RELEGATED);
        t.put::<i8>(COUNT, -2);
        t.set_table_place(17);
        let size = league_size(&records, records.teams[i].country(), 0);
        season_end(&mut records);
        let x = &records.managers[human];
        let twice = |v: u8, a: i32, b: i32| step(step(v, a), b);
        assert_eq!(x.popularity(), [twice(p[0], -20, -3), twice(p[1], -20, -6), twice(p[2], -5, -2)]);
        assert_eq!(x.confidence(), (c - 15).clamp(0, 100));
        assert_eq!(x.season_marks(), m - 50 + (size - 17) * 4);
    }
}
