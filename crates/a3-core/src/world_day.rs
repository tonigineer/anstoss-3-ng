//! The world's day (`FUN_006504b6` running `FUN_007848fa` with the day
//! of the week): on the first weekday every club of the countries
//! played besides the main one lives its day, on the second the main
//! country's clubs do; the game day counter stands in `DAT_00b5cbd4`.
//!
//! One club's day (`FUN_00784a6d`) draws over every player of its
//! squad: a roll of a hundred for the player's mood (`FUN_007849f3`,
//! whose two weight tables only decide what follows the roll), and,
//! for the computer clubs, a roll of ten thousand for the player's
//! play (`FUN_00786622`), which rolls a seven more when it comes out
//! 0 or 1. A club with humans at its record's +0xa8 — a trainer and a
//! manager by number — instead rolls a three after its squad, and the
//! roll of a hundred on its run-time word +0xa7 stands for every club
//! whose word is set.
//!
//! The driver then rounds the human clubs of the globals' list once a
//! day, seven rolls of ten per player ([`luck_round_of_list`]); which
//! player state a 0-roll feeds is not ported. Behind the counter's
//! increment the day walks its schedule table once ([`schedule_walk`]):
//! a roll of a hundred steps the walk index and the index rolls over
//! the table's rows ([`schedule_day`]).
//!
//! Not ported yet, and drawing in the original when they run: that
//! hundred-roll on +0xa7, whose table no save hands the replica, and
//! what the rolls answer to — the moods, the play and the club's
//! standing; the weekly parts of the
//! other weekdays (`FUN_00417a8a` with its two-rolls on the second,
//! `FUN_00648e0d` with hundreds on the fourth, `FUN_00786931` on the
//! second, `FUN_00786080` on the sixth).

use crate::random::Random;
use crate::records::{Records, Team};

/// The day of the world: `weekday` 0 sends the clubs of the more
/// countries through [`club`], 1 those of the main country; the other
/// weekdays' steps are not here.
pub fn day(records: &Records, weekday: i32, random: &mut Random) {
    match weekday {
        0 => {
            let countries = records.globals.more_countries();
            let more = records.globals.more_count().max(0) as usize;
            for k in 0..more.min(countries.len()) {
                clubs_of(records, countries[k], random);
            }
        }
        1 => clubs_of(records, records.globals.main_country(), random),
        _ => {}
    }
    luck_round_of_list(records, random);
    schedule_walk(records, random);
}

/// The day's step behind the counter's increment (`FUN_00675a73(1)`):
/// one roll of a hundred nudges the walk index at +0xb5df6a one step
/// towards its phase's target ([`walk_next`], `FUN_00783dc3`), and the
/// index then picks the day's rolls over the schedule table
/// ([`schedule_day`], `FUN_00783ea3`). The rotation `FUN_007842a1`
/// draws nothing, and the phase machine steps no phase boundary here —
/// its table day counts at +0xb5cbda are read but not written back.
fn schedule_walk(records: &Records, random: &mut Random) {
    /// The walk's target per phase (`DAT_008acfec`, `FUN_00675a73`'s
    /// `_DAT_00b5cbdc` index).
    const TARGETS: [i16; 13] = [0, 5, 4, 3, 3, 2, 1, 0, 0, 1, 2, 3, 4];
    let phase = records.globals.get::<i16>(0xb5cbdc).max(0) as usize;
    let target = TARGETS.get(phase).copied().unwrap_or(0);
    let walk = records.globals.get::<i16>(0xb5df6a);
    let roll = i16::try_from(random.below(100)).unwrap();
    schedule_day(walk_next(walk, target, roll), random);
}

/// A day's nudge of the walk index (`FUN_00783dc3`): below eighty it
/// climbs towards the target, between eighty and ninety it only climbs
/// out of the two lowest rows, and from ninety it sinks.
fn walk_next(walk: i16, target: i16, roll: i16) -> i16 {
    if walk == target && roll < 0x50 {
        return walk;
    }
    if walk < target && roll < 0x50 {
        walk + 1
    } else if target < walk && roll < 0x50 {
        walk - 1
    } else if roll < 0x5a {
        if walk < 2 {
            walk + 1
        } else {
            walk
        }
    } else if 0 < walk {
        walk - 1
    } else {
        walk
    }
}

/// The day's rolls over the schedule table (`FUN_00783ea3`): the walk
/// index says which rows of the 40×40 table at +0xb5d92a get new
/// numbers — only the rolls of the rows' cells are ported, the numbers
/// they answer to and the table itself are not.
fn schedule_day(walk: i16, random: &mut Random) {
    match walk {
        0 => {
            for _ in 0..40 {
                random.below(2);
            }
        }
        1 => {
            for _ in 0..40 {
                random.below(2);
            }
            for _ in 0..20 {
                random.below(5);
            }
            for _ in 0..20 {
                random.below(3);
            }
        }
        2 => {
            for _ in 0..20 {
                random.below(5);
            }
            for _ in 0..20 {
                random.below(3);
            }
        }
        3 => {
            for _ in 0..20 {
                random.below(4);
            }
            for _ in 0..20 {
                random.below(3);
            }
            for _ in 0..40 {
                random.below(4);
            }
        }
        4 => {
            for _ in 0..40 {
                random.below(4);
            }
        }
        5 => {
            for _ in 0..40 {
                random.below(3);
            }
            for _ in 0..40 {
                random.below(4);
            }
        }
        _ => {}
    }
}

/// The round the day's driver runs after the weekday switch, over the
/// globals' list of human clubs (`FUN_007845aa` behind the wrapper
/// `FUN_007848f0`): the list stands at +0xb5cbe2 — one entry eight bytes
/// apart, the country's byte and the club's number two further — and its
/// count at +0xb5cc02; on every club every squad player rolls seven of
/// the tens, [`luck_round`].
fn luck_round_of_list(records: &Records, random: &mut Random) {
    let list: [u8; 32] = records.globals.get(0xb5cbe2);
    let count = records.globals.get::<i16>(0xb5cc02).max(0) as usize;
    for entry in list.chunks(8).take(count) {
        let country = u16::from(entry[0]);
        let number = u16::from_le_bytes([entry[2], entry[3]]);
        luck_round(records.team(country, number), random);
    }
}

/// One club's round of tens (`FUN_007845aa`'s player loop): seven rolls
/// of ten for every player of its squad. A 0-roll reads one of the
/// player's states and raises or lowers it by one — the club's manager
/// stands in the states' table, and its skill only feeds what the rolls
/// answer to, not the rolls themselves; the states stay unported.
fn luck_round(team: Option<&Team>, random: &mut Random) {
    let Some(team) = team else { return };
    for _ in 0..team.player_count() {
        for _ in 0..7 {
            random.below(10);
        }
    }
}

/// The clubs of one country, the records' count of them in the order
/// they stand in their country (`FUN_00784f27` and `FUN_00784ee8`).
fn clubs_of(records: &Records, country: i16, random: &mut Random) {
    let Ok(id) = u16::try_from(country) else {
        return;
    };
    for number in 0..records.countries.team_count(id as usize).max(0) {
        let Ok(number) = u16::try_from(number) else {
            continue;
        };
        club(records.team(id, number), random);
    }
}

/// One club's day (`FUN_00784a6d`): every squad player gets the roll
/// of a hundred and — unless the club is a human one at its record's
/// +0xa8 — the roll of ten thousand behind it; the human club rolls a
/// three once after its squad instead.
fn club(team: Option<&Team>, random: &mut Random) {
    let Some(team) = team else { return };
    let human = team.humans() != [0, 0];
    for _ in team.players() {
        random.below(100);
        if !human {
            if random.below(10_000) < 2 {
                random.below(7);
            }
        }
    }
    if human {
        random.below(3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A computer club's day: every squad player gets the roll of a
    /// hundred and the roll of ten thousand, and only the ten thousand
    /// out of 0 or 1 the roll of a seven.
    #[test]
    fn a_computer_club_rolls_over_its_squad() {
        let mut team = Team::new(&vec![0u8; Team::SIZE]).unwrap();
        team.set_players(&[5, 9, 12]);
        let mut random = Random::from_state(20261101, 4);
        let mut copy = random.clone();
        club(Some(&team), &mut random);
        for _ in 0..3 {
            copy.below(100);
            if copy.below(10_000) < 2 {
                copy.below(7);
            }
        }
        assert!(random == copy, "the club's day, draw for draw");
    }

    /// The human club — a trainer or a manager at its record's +0xa8 —
    /// rolls the ten thousand over no player and a three once at the end.
    #[test]
    fn a_human_club_rolls_the_three_after_its_squad() {
        let mut team = Team::new(&vec![0u8; Team::SIZE]).unwrap();
        team.set_players(&[5, 9]);
        team.set_humans([0, 1]);
        let mut random = Random::from_state(20261101, 5);
        let mut copy = random.clone();
        club(Some(&team), &mut random);
        for _ in 0..2 {
            copy.below(100);
        }
        copy.below(3);
        assert!(random == copy, "no ten thousand, one three");
    }

    /// The round over a club: seven rolls of ten for every squad
    /// player — the 0-rolls' states are not ported, but their rolls are.
    #[test]
    fn the_luck_round_rolls_seven_tens_per_player() {
        let mut team = Team::new(&vec![0u8; Team::SIZE]).unwrap();
        team.set_players(&[5, 9, 12]);
        let mut random = Random::from_state(20261101, 6);
        let mut copy = random.clone();
        luck_round(Some(&team), &mut random);
        for _ in 0..21 {
            copy.below(10);
        }
        assert!(random == copy, "seven tens for every player");
    }

    /// A club with no squad player — and an entry that finds no club —
    /// rolls nothing at all.
    #[test]
    fn the_luck_round_rolls_nothing_for_a_small_squad() {
        let mut team = Team::new(&vec![0u8; Team::SIZE]).unwrap();
        team.set_players(&[]);
        let mut random = Random::from_state(20261101, 7);
        let copy = random.clone();
        luck_round(Some(&team), &mut random);
        luck_round(None, &mut random);
        assert!(random == copy, "not one roll");
    }

    /// A day's nudge of the walk index: it climbs and sinks towards
    /// the target below eighty, climbs out of the two lowest rows
    /// between eighty and ninety, and sinks from ninety.
    #[test]
    fn the_walk_steps_one_per_roll() {
        assert_eq!(walk_next(0, 1, 50), 1, "climb");
        assert_eq!(walk_next(4, 1, 50), 3, "sink");
        assert_eq!(walk_next(1, 1, 50), 1, "at the target");
        assert_eq!(walk_next(1, 1, 84), 2, "a middling roll leaves the target");
        assert_eq!(walk_next(0, 1, 84), 1, "out of the bottom");
        assert_eq!(walk_next(3, 1, 84), 3, "only the bottom climbs");
        assert_eq!(walk_next(3, 1, 95), 2, "from ninety it sinks");
        assert_eq!(walk_next(0, 9, 95), 0, "and never below zero");
    }

    /// The walk index's day over the schedule table: which rows get
    /// new numbers, and how many rolls that is.
    #[test]
    fn the_schedule_day_rolls_its_rows() {
        let days: [(i16, &[(u32, usize)]); 7] = [
            (0, &[(2, 40)]),
            (1, &[(2, 40), (5, 20), (3, 20)]),
            (2, &[(5, 20), (3, 20)]),
            (3, &[(4, 20), (3, 20), (4, 40)]),
            (4, &[(4, 40)]),
            (5, &[(3, 40), (4, 40)]),
            (6, &[]),
        ];
        for (walk, rows) in days {
            let mut random = Random::from_state(20261101, 8);
            let mut copy = random.clone();
            schedule_day(walk, &mut random);
            for (range, count) in rows {
                for _ in 0..*count {
                    copy.below(*range);
                }
            }
            assert!(random == copy, "walk {walk}'s rolls");
        }
    }
}
