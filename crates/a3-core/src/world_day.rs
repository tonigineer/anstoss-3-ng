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
//! player state a 0-roll feeds is not ported.
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
}
