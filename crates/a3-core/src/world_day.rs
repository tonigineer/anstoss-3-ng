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
}
