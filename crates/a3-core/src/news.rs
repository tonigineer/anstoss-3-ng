//! The day's news of the human clubs: the pick of the category
//! (`FUN_0060be4d` over `DAT_0088e128`, inside `FUN_005ef8cc` as the
//! advance `FUN_0064f981` runs it for the clubs whose slot its gates
//! drew today, after the training).
//!
//! What is not here yet: the club's waiting-event gate `FUN_006a700a`,
//! which draws once only in one of its late cases and finds nothing in
//! the replica; the group news `FUN_005e3885` rolls 400 off the club's
//! fan byte (+0x9e), which the replica has no part of — on the compared
//! days of save A it stands at 100 or above, so no roll runs before the
//! pick; the rest gate `FUN_005ef56c` reads two run-time flags beside
//! the day counter; the player-manager blocks behind the club's bytes
//! +0x8a and +0x8b; and the handlers of the other categories — nearest
//! category 18 (`FUN_005fd4ae`), which rolls over the club's squad as
//! 19 does and then picks one more weighted table when the talked-about
//! player has no news flag yet, and the postings themselves.

use crate::random::Random;

/// The category table (`DAT_0088e128`, 24 pairs of weight and code
/// registered at a cap of 10 000): a total weight of 2100, the codes 1
/// to 12, 18 to 25, 50 to 52 and 55 — codes 13 to 17 stand no more.
const NEWS: [(u32, u16); 24] = [
    (100, 1),
    (100, 2),
    (100, 3),
    (100, 4),
    (100, 5),
    (100, 6),
    (100, 7),
    (100, 8),
    (100, 9),
    (55, 10),
    (55, 11),
    (11, 12),
    (110, 18),
    (110, 19),
    (30, 20),
    (11, 21),
    (22, 22),
    (11, 23),
    (55, 24),
    (100, 25),
    (66, 50),
    (44, 51),
    (500, 52),
    (20, 55),
];

/// The day's pick of the news category: one roll over the cap of 10 000;
/// the walk subtracts each weight not standing above the rest, and the
/// first entry whose weight stands above it gives its code — the rest
/// gone stands for no news, 0.
pub fn pick(random: &mut Random) -> u16 {
    let mut rest = random.below(10_000);
    for (weight, code) in NEWS {
        if weight > rest {
            return code;
        }
        rest -= weight;
    }
    0
}

/// The picked category 19 (`case 0x13` with `FUN_005fed08`): one of the
/// club's own players is talked about, drawn over the club's squad —
/// the club's +0xe2, the team record's `player_count` at +0x112, over
/// its squad list (the club's +0xb50, built from the record at +0x1f2).
///
/// Whether the news is then posted rests on two run-time parts the
/// replica holds no part of: the player must not be a player-manager
/// (its id below 17 001), and the posting `FUN_005fd59d` runs only for
/// a player whose news flag (its +0x28d) an earlier news already set —
/// no save keeps that flag, so out of a loaded save the pick of the
/// player stands as the day's whole draw here.
pub fn about_a_player(squad: u16, random: &mut Random) {
    random.below(u32::from(squad));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pick takes one draw and walks the table: the roll of save A
    /// on its day 63, 1191, walks nine times 100, then 55, 55, 11 and
    /// 110 away and lands on the code 19.
    #[test]
    fn the_pick_walks_the_weights_over_one_draw() {
        let mut random = Random::from_state(20261101, 19);
        let mut copy = random.clone();
        let rest = copy.below(10_000);
        let code = pick(&mut random);
        assert!(random == copy, "one draw, no more");
        let mut walked = rest;
        let mut expected = 0;
        for (weight, at) in NEWS {
            if weight > walked {
                expected = at;
                break;
            }
            walked -= weight;
        }
        assert_eq!(code, expected);
    }

    /// Category 19 takes one draw over the club's squad and no more:
    /// the posting behind it needs the player's run-time news flag,
    /// which stands open out of every save.
    #[test]
    fn category_19_rolls_the_squad_once() {
        let mut seed = 7i64;
        let mut random = Random::from_state(seed, 3);
        while pick(&mut random.clone()) != 19 {
            seed += 1;
            random = Random::from_state(seed, 3);
        }
        let mut copy = random.clone();
        copy.below(10_000);
        let code = pick(&mut random);
        assert_eq!(code, 19);
        about_a_player(26, &mut random);
        let _next = copy.below(26);
        assert!(random == copy, "the pick of the player, one draw");
    }
}
