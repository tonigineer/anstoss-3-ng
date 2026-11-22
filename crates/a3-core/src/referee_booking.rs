//! The referees booked for the human clubs' next matches (`FUN_0062bfc9`'s
//! draws). Two tables of thirty bookings each, 0xe bytes: the home club's
//! country (+0) and number (+2), the away club's country (+4) and number
//! (+6), the day (+8), the referee (+0xa), whether the booking stands
//! (+0xc) and whether it holds either way round (+0xd). The national
//! matches book at `0x1bab640` (`FUN_006cdc0f` → `FUN_006db61a`), the
//! international ones at `0x1bab498` (`FUN_00424407` → `FUN_006dba8e`).
//! A booking a week past its day is let go; one for the same day and the
//! same two clubs gives its referee again, so both human sides of a match
//! see the same man.

use crate::random::Random;
use crate::records::Records;

const NATIONAL: &str = "0x1bab640";
const INTERNATIONAL: &str = "0x1bab498";
const ENTRY: usize = 0xe;
const ENTRIES: usize = 30;
/// A country's referees, its slot's forty from `slot × 40 + 1` on.
const PER_COUNTRY: i16 = 40;
/// The referees of no league's country, with one of their own (+4).
const FOREIGN: std::ops::RangeInclusive<i16> = 0xc9..=0xf0;
/// The referee a club refused (team +0x928, the contacts' dialog).
const REFUSED_AT: usize = 0x928;
/// The national draw's tries over a booked and refused man, the
/// international's over a booked one.
const NATIONAL_TRIES: u32 = 30;
const INTERNATIONAL_TRIES: u32 = 20;

fn word(e: &[u8], at: usize) -> i16 {
    i16::from_le_bytes([e[at], e[at + 1]])
}

fn put(e: &mut [u8], at: usize, v: i16) {
    e[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

/// The referee a club refused (`FUN_0063e4cc`): none for a country the
/// game does not follow or a number past its clubs.
fn refused(records: &Records, country: u8, number: i16) -> i16 {
    if !records.globals.played(i16::from(country)) || number < 0 || i32::from(number) >= i32::from(records.countries.team_count(usize::from(country))) {
        return 0;
    }
    records.team(u16::from(country), number as u16).map_or(0, |t| t.get::<i16>(REFUSED_AT))
}

/// Marks a referee taken.
fn take(taken: &mut [bool; 500], referee: i16) {
    if let Some(t) = usize::try_from(referee).ok().and_then(|r| taken.get_mut(r)) {
        *t = true;
    }
}

fn is_taken(taken: &[bool; 500], referee: i16) -> bool {
    usize::try_from(referee).ok().and_then(|r| taken.get(r)).copied().unwrap_or(false)
}

/// A national match's referee (`FUN_006db61a`): the country's bookings a
/// week past their day let go; the day's booking of the same two clubs —
/// either way round where it holds so — gives its man, the day's others
/// are taken. With a free place a man of the country's forty is drawn,
/// drawn again while he is both taken and refused by one of the clubs —
/// the original asks all three at once — for thirty tries at most, and
/// booked; with none the country's first man stands, unbooked. `both`
/// marks the booking of the clubs the right way round as holding either
/// way (`FUN_006db997`, a neutral ground).
pub fn national(records: &mut Records, country: u8, home: i16, away: i16, day: i16, today: i32, both: bool, random: &mut Random) -> i16 {
    let base = i16::from(records.countries.slot(usize::from(country))) * PER_COUNTRY + 1;
    let (refused_home, refused_away) = (refused(records, country, home), refused(records, country, away));
    let Some(table) = records.block_mut(NATIONAL) else { return base };
    let mut taken = [false; 500];
    let mut free = None;
    let mut found = None;
    for (i, e) in table.chunks_exact_mut(ENTRY).take(ENTRIES).enumerate() {
        if e[0xc] == 0 {
            free.get_or_insert(i);
            continue;
        }
        if e[0] != country {
            continue;
        }
        if i32::from(word(e, 8)) + 7 < today {
            e[0xc] = 0;
            free.get_or_insert(i);
            continue;
        }
        if word(e, 8) != day {
            continue;
        }
        let (h, a) = (word(e, 2), word(e, 6));
        if (h, a) == (home, away) || e[0xd] != 0 && (h, a) == (away, home) {
            found = Some(word(e, 0xa));
            break;
        }
        take(&mut taken, word(e, 0xa));
    }
    let referee = match (found, free) {
        (Some(r), _) => r,
        (None, None) => base,
        (None, Some(i)) => {
            let mut tries = 0;
            let man = loop {
                tries += 1;
                let man = random.below(PER_COUNTRY as u32) as i16 + base;
                if !is_taken(&taken, man) || tries >= NATIONAL_TRIES || man != refused_home && man != refused_away {
                    break man;
                }
            };
            let e = &mut table[i * ENTRY..(i + 1) * ENTRY];
            put(e, 0xa, man);
            e[0] = country;
            put(e, 2, home);
            e[4] = country;
            put(e, 6, away);
            put(e, 8, day);
            e[0xc] = 1;
            man
        }
    };
    if both {
        mark_both(table, (country, home), (country, away), day);
    }
    referee
}

/// An international match's referee (`FUN_006dba8e`): every booking a
/// week past its day let go, whatever its country; the day's booking of
/// the same two clubs gives its man, the day's others are taken. The men
/// drawn from are the first slot's forty — the loop over the slots steps
/// by forty, so no other slot is reached — unless one of the two
/// countries is followed and holds that slot, and the foreign men of
/// neither country (`FUN_0063161e`'s +4). With a free place one is drawn,
/// again while taken, for twenty tries at most, and booked; with none the
/// first of them stands, unbooked. `both` as [`national`]
/// (`FUN_006dc086`).
pub fn international(records: &mut Records, (home_country, home): (u8, i16), (away_country, away): (u8, i16), day: i16, today: i32, both: bool, random: &mut Random) -> i16 {
    let followed_first = |c: u8| records.globals.played(i16::from(c)) && records.countries.slot(usize::from(c)) == 0;
    let mut men: Vec<i16> = Vec::new();
    if !followed_first(home_country) && !followed_first(away_country) {
        men.extend(1..=PER_COUNTRY);
    }
    men.extend(FOREIGN.filter(|&r| records.referee_country(r).is_some_and(|c| c != home_country && c != away_country)));
    let Some(table) = records.block_mut(INTERNATIONAL) else { return men.first().copied().unwrap_or(0) };
    let mut taken = [false; 500];
    let mut free = None;
    let mut found = None;
    for (i, e) in table.chunks_exact_mut(ENTRY).take(ENTRIES).enumerate() {
        if e[0xc] == 0 {
            free.get_or_insert(i);
            continue;
        }
        if i32::from(word(e, 8)) + 7 < today {
            e[0xc] = 0;
            free.get_or_insert(i);
            continue;
        }
        if word(e, 8) != day {
            continue;
        }
        let (h, a) = ((e[0], word(e, 2)), (e[4], word(e, 6)));
        if (h, a) == ((home_country, home), (away_country, away)) || e[0xd] != 0 && (h, a) == ((away_country, away), (home_country, home)) {
            found = Some(word(e, 0xa));
            break;
        }
        take(&mut taken, word(e, 0xa));
    }
    let referee = match (found, free) {
        (Some(r), _) => r,
        (None, None) => men.first().copied().unwrap_or(0),
        (None, Some(i)) => {
            let mut tries = 0;
            let man = loop {
                let man = men.get(random.below(men.len() as u32) as usize).copied().unwrap_or(0);
                tries += 1;
                if !is_taken(&taken, man) || tries >= INTERNATIONAL_TRIES {
                    break man;
                }
            };
            let e = &mut table[i * ENTRY..(i + 1) * ENTRY];
            put(e, 0xa, man);
            e[0] = home_country;
            put(e, 2, home);
            e[4] = away_country;
            put(e, 6, away);
            put(e, 8, day);
            e[0xc] = 1;
            man
        }
    };
    if both {
        mark_both(table, (home_country, home), (away_country, away), day);
    }
    referee
}

/// The first standing booking of the day and the two clubs, the right way
/// round, made to hold either way (`FUN_006db997`, `FUN_006dc086`).
fn mark_both(table: &mut [u8], home: (u8, i16), away: (u8, i16), day: i16) {
    if let Some(e) = table
        .chunks_exact_mut(ENTRY)
        .take(ENTRIES)
        .find(|e| e[0xc] != 0 && word(e, 8) == day && (e[0], word(e, 2)) == home && (e[4], word(e, 6)) == away)
    {
        e[0xd] = 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kept(name: &str) -> Option<Records> {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let path = dir.parent().unwrap().join("saves-kept").join(name);
        if !path.exists() {
            eprintln!("no {name} among the kept saves");
            return None;
        }
        Some(Records::from_save(a3_files::an3::Save::read(&path).unwrap()).unwrap())
    }

    /// On save A with its tables emptied: a national man is one of the
    /// country's forty, the same day and clubs give him again — turned
    /// round only once marked to hold either way —, and a week past its
    /// day the country's booking lapses; an international man is of the
    /// first slot's forty or a foreigner of neither country.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_booking_gives_its_man_again_and_lapses_after_a_week() {
        let Some(mut r) = kept("A.an3") else { return };
        r.block_mut(NATIONAL).unwrap().fill(0);
        r.block_mut(INTERNATIONAL).unwrap().fill(0);
        let mut random = Random::new(7);
        let base = i16::from(r.countries.slot(14)) * 40 + 1;
        let first = national(&mut r, 14, 3, 9, 20, 10, false, &mut random);
        assert!((base..base + 40).contains(&first), "{first}");
        assert_eq!(national(&mut r, 14, 3, 9, 20, 12, false, &mut random), first);
        let table = r.block(NATIONAL).unwrap();
        assert_eq!((table[0], word(table, 2), table[4], word(table, 6), word(table, 8), word(table, 0xa), table[0xc], table[0xd]), (14, 3, 14, 9, 20, first, 1, 0));
        let _ = national(&mut r, 14, 9, 3, 20, 12, false, &mut random);
        assert_eq!(r.block(NATIONAL).unwrap()[ENTRY + 0xc], 1, "turned round, a booking of its own");
        mark_both(r.block_mut(NATIONAL).unwrap(), (14, 3), (14, 9), 20);
        assert_eq!(r.block(NATIONAL).unwrap()[0xd], 1);
        let _ = national(&mut r, 14, 1, 2, 40, 28, false, &mut random);
        let table = r.block(NATIONAL).unwrap();
        assert_eq!((table[0xc], table[ENTRY + 0xc], word(table, 2), word(table, 8)), (1, 0, 1, 40));
        for d in 0..30 {
            let man = international(&mut r, (14, 3), (68, 5), d, i32::from(d), false, &mut random);
            let foreign = FOREIGN.contains(&man) && r.referee_country(man).is_some_and(|c| c != 14 && c != 68);
            assert!(foreign || (1..=40).contains(&man) && r.countries.slot(14) != 0, "{man}");
        }
    }
}
