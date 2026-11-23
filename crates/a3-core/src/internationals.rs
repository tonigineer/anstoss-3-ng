//! The nations' international dates: the national teams play their
//! booked matches on the weekday's 3 and 6 (`FUN_006bd980`).
//!
//! The original walks the day of the hub at state 4: after the youth
//! leagues (`FUN_0062a959`) and just before the day of the world
//! (`FUN_006504b6`). The replica plays its youth leagues after the
//! day's matches, so this lands where the world's day is.
//!
//! What a nation owes is booked every Monday from the international
//! calendar (`FUN_006bd31f` writes 1 or 2 per nation into the table at
//! `DAT_001ba06d0`); the booking stands unported, so a new game of the
//! replica owes nothing while a loaded save brings its counters along —
//! and here they run down, one per date played.
//!
//! The original plays over the nation's cached player handles; the
//! fields it reads were copied off the player's record when the cache
//! loaded it (`FUN_00636638`): strength at handle +0x2, the main
//! position at +0x3, and the cap counter at +0x61 — which the cache
//! writes back to the record's +0x266 when the player is released,
//! caps and all. What it decides per player: one under his nation's
//! strength byte (`DAT_00ae6223`) is skipped a third of the times, the
//! rest earn a cap, and their position rolls for a goal — the goals go
//! straight into the record (`FUN_00704a1c`, +0x214). The original
//! keeps the squad tables of its roster beside (`FUN_005dc461`): no
//! draw, no copy into the save, so the replica books none of it.

use crate::random::Random;
use crate::records::Records;
use crate::season_odds::{
    NATIONAL_COUNT, NATIONAL_SQUAD, NATIONAL_TEAMS, NATION_STRENGTH, NATIONS,
};

/// How many international dates each nation still owes this week
/// (`DAT_001ba06d0`, an i16 per nation).
const GATES: &str = "0x1ba06d0";

/// The day's internationals, if the weekday is a 3 or a 6
/// (`FUN_006bd980` over the nations 1..=0x74).
pub fn play_day(records: &mut Records, today: i32, random: &mut Random) {
    let weekday = today.rem_euclid(7);
    if weekday != 3 && weekday != 6 {
        return;
    }
    let Some(gates) = records.block(GATES) else { return };
    let booked: Vec<usize> = (1..NATIONS)
        .filter(|&n| i16::from_le_bytes([gates[2 * n], gates[2 * n + 1]]) > 0)
        .collect();
    for nation in booked {
        let standard = records.countries.get::<u8>(NATION_STRENGTH + nation);
        // The squad the national side travels with: the ids its record
        // lists, in order (`FUN_00635f76` loads them for the original).
        let squad: Vec<(u16, u8, u8)> = records
            .block(&format!("{NATIONAL_TEAMS:#x}[{nation}]"))
            .map_or(Vec::new(), |b| {
                let count =
                    i16::from_le_bytes([b[NATIONAL_COUNT], b[NATIONAL_COUNT + 1]]).max(0) as usize;
                (0..count)
                    .filter_map(|k| b.get(NATIONAL_SQUAD + 2 * k..NATIONAL_SQUAD + 2 * k + 2))
                    .map(|w| u16::from_le_bytes([w[0], w[1]]))
                    .filter_map(|id| {
                        // A handle the cache would not have for a player
                        // the records do not keep.
                        records.player(id).map(|p| (id, p.strength(), p.positions()[0]))
                    })
                    .collect()
            });
        let turns: Vec<(u16, u16, bool)> = squad
            .iter()
            .map(|&(id, strength, position)| {
                let (goals, played) = turn(standard, strength, position, random);
                (id, goals, played)
            })
            .collect();
        for (id, goals, played) in turns {
            let Some(p) = records.player_mut(id) else { continue };
            if !played {
                // The skipped stay whole: no cap, no goal, no table.
                continue;
            }
            // The cap of anyone who stood on the pitch, the goals of
            // those who found the net; both wrap as their byte and word.
            p.set_kind_12_matches(p.kind_12_matches().wrapping_add(1));
            if goals > 0 {
                p.set_international_goals(p.international_goals().wrapping_add(goals));
            }
        }
        // One date less, played or not — the original decrements after
        // its player loop and only for a nation that owed one.
        if let Some(gates) = records.block_mut(GATES) {
            let owed = i16::from_le_bytes([gates[2 * nation], gates[2 * nation + 1]]);
            gates[2 * nation..2 * nation + 2].copy_from_slice(&(owed - 1).to_le_bytes());
        }
    }
}

/// One international's turn for one selected player (`FUN_006bd980`'s
/// body over a handle): under his nation's standard a three skips him
/// whole; else he earns the cap, and his main position rolls the ten
/// for a goal — a centre-forward under a two for one to three, under a
/// seven for one, the other outfield above the half line under a two
/// for one, a keeper never, the rest under no other than zero.
/// Answers the goals he scored and whether he stood on the pitch at all
/// — the skipped earn neither cap nor goal.
fn turn(standard: u8, strength: u8, position: u8, random: &mut Random) -> (u16, bool) {
    if strength < standard && random.below(3) == 0 {
        return (0, false);
    }
    let goals = match position {
        10 => {
            let roll = random.below(10);
            if roll < 2 {
                random.below(3) as u16 + 1
            } else if roll < 7 {
                1
            } else {
                0
            }
        }
        p if p >= 6 => u16::from(random.below(10) < 2),
        1 => 0,
        _ => u16::from(random.below(10) == 0),
    };
    (goals, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Under his nation's standard a player is skipped a third of the
    /// times, and a skip costs the ten he would have rolled; the ones
    /// who play roll a ten for their one in ten.
    #[test]
    fn the_weak_are_skipped_a_third_of_the_times() {
        let mut random = Random::from_state(20261105, 1);
        let mut copy = random.clone();
        let mut skipped = 0;
        for _ in 0..300 {
            let (goals, played) = turn(9, 5, 4, &mut random);
            if copy.below(3) == 0 {
                skipped += 1;
                assert_eq!((goals, played), (0, false), "the skip rolls nothing else");
            } else {
                assert_eq!(goals, u16::from(copy.below(10) == 0));
            }
        }
        assert!((60..140).contains(&skipped), "a third of 300, saw {skipped}");
        assert!(random == copy, "one three, and a ten for those who play");
    }

    /// A centre-forward at full standing: the ten under a two answers
    /// a three for one to three goals, the ten under a seven one goal,
    /// and the rest none.
    #[test]
    fn a_striker_rolls_the_ten_for_one_to_three() {
        let mut random = Random::from_state(20261105, 2);
        let mut copy = random.clone();
        let mut tallied = [0; 4];
        for _ in 0..300 {
            let (goals, played) = turn(0, 0, 10, &mut random);
            assert!(played, "no standing under it, no three");
            let roll = copy.below(10);
            let scored = if roll < 2 {
                copy.below(3) as u16 + 1
            } else if roll < 7 {
                1
            } else {
                0
            };
            assert_eq!(goals, scored);
            tallied[goals as usize] += 1;
        }
        assert!(tallied.iter().all(|&t| t > 0), "one to three and none: {tallied:?}");
        assert!(random == copy, "the three comes only under a two");
    }

    /// The keeper rolls nothing all night, the wing beyond the half
    /// line one ten; both still stand and earn the cap.
    #[test]
    fn keepers_roll_nothing_and_the_wing_one_ten() {
        let mut random = Random::from_state(20261105, 3);
        let mut copy = random.clone();
        for _ in 0..50 {
            assert_eq!(turn(0, 0, 1, &mut random), (0, true), "a keeper, no ten");
            let (goals, played) = turn(0, 0, 8, &mut random);
            assert_eq!(goals, u16::from(copy.below(10) < 2));
            assert!(played);
        }
        assert!(random == copy, "fifty tens, not more");
    }

    /// A date on a saved game: its squad read from the record, walked
    /// in order — the counter runs down, the caps of all who stand and
    /// the goals of those who score land in the players, and the
    /// weekday outside 3 and 6 moves nothing.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_saved_date_walks_its_squad_in_its_order() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let mut records =
            Records::from_save(Save::read(&keeps.join("NewSeason000.an3")).unwrap()).unwrap();
        // Every saved game owes nothing; the counter is booked by hand
        // for the first nation that travels with a squad.
        let squad_of = |r: &Records, n: usize| -> Vec<(u16, u8, u8)> {
            r.block(&format!("{NATIONAL_TEAMS:#x}[{n}]")).map_or(Vec::new(), |b| {
                let count = i16::from_le_bytes([b[NATIONAL_COUNT], b[NATIONAL_COUNT + 1]]).max(0)
                    as usize;
                (0..count)
                    .filter_map(|k| b.get(NATIONAL_SQUAD + 2 * k..NATIONAL_SQUAD + 2 * k + 2))
                    .map(|w| u16::from_le_bytes([w[0], w[1]]))
                    .filter_map(|id| r.player(id).map(|p| (id, p.strength(), p.positions()[0])))
                    .collect()
            })
        };
        let nation = (1..NATIONS).find(|&n| !squad_of(&records, n).is_empty()).unwrap();
        let standard = records.countries.get::<u8>(NATION_STRENGTH + nation);
        let squad = squad_of(&records, nation);
        records.block_mut(GATES).unwrap()[2 * nation..2 * nation + 2]
            .copy_from_slice(&1i16.to_le_bytes());
        // A weekday that does not play leaves all of it alone.
        let mut idle = Random::new(7);
        let untouched = idle.clone();
        play_day(&mut records, 67, &mut idle);
        assert!(idle == untouched, "the 4th weekday rolls nothing");
        assert_eq!(i16::from_le_bytes(read_gate(&records, nation)), 1, "still owed");
        // The date itself, 66th day: draw for draw the walk's own.
        let mut random = Random::new(7);
        let mut copy = random.clone();
        let expected: Vec<(u16, u16, bool)> = squad
            .iter()
            .map(|&(id, strength, position)| {
                let (goals, played) = turn(standard, strength, position, &mut copy);
                (id, goals, played)
            })
            .collect();
        let before: Vec<(u16, u8, u16)> = squad
            .iter()
            .map(|&(id, _, _)| {
                let p = records.player(id).unwrap();
                (id, p.kind_12_matches(), p.international_goals())
            })
            .collect();
        play_day(&mut records, 66, &mut random);
        assert!(random == copy, "the squad's turns, draw for draw");
        assert_eq!(i16::from_le_bytes(read_gate(&records, nation)), 0, "the date played");
        for (b, e) in before.iter().zip(&expected) {
            let (id, caps, scored) = (b.0, b.1, b.2);
            let (played, goals) = (e.2, e.1);
            let p = records.player(id).unwrap();
            assert_eq!(p.kind_12_matches(), caps.wrapping_add(u8::from(played)), "player {id}");
            assert_eq!(p.international_goals(), scored.wrapping_add(goals), "player {id}");
        }
    }

    fn read_gate(r: &Records, n: usize) -> [u8; 2] {
        let g = r.block(GATES).unwrap();
        [g[2 * n], g[2 * n + 1]]
    }
}
