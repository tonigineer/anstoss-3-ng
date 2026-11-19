//! "Neueinstieg": a new human joins the running game. The dressing room's
//! first option (`CUmkleide::FUN_005b1156`, closing with 3000) asks his
//! name (212) and birthday (491) and queues him (`CMyStartDialog::
//! FUN_006b7255` → `FUN_006c4f81`); the week's start takes the queue in
//! (`FUN_006c5005` from `FUN_006da3a9`'s state 1): a slot for each
//! (`FUN_006a3f6a`, `FUN_006a3b61`), and the level the others leave him
//! (`FUN_006a5400`).

use crate::newgame::Trainer;
use crate::random::Random;
use crate::records::{MANAGERS, Records};
use crate::season::{NewManager, Rules, Season};

/// At most four humans play; a fifth is told so (text 3260).
pub const MOST: usize = MANAGERS;
/// The level a newcomer gets at most, and at least (`FUN_006a5400`).
const LEVEL_CEILING: i32 = 15;
const LEVEL_FLOOR: i32 = 2;
/// He starts that many levels below the lowest of the others.
const LEVEL_BELOW: i32 = 3;

/// The level a newcomer starts at: three below the lowest of the others,
/// no more than 15 and no less than 2 (`FUN_006a5400`).
pub fn level(others: impl IntoIterator<Item = i32>) -> i32 {
    others.into_iter().map(|l| l - LEVEL_BELOW).fold(LEVEL_CEILING, i32::min).max(LEVEL_FLOOR)
}

/// The slot a newcomer takes, 1 to 4, by the humans' order: the first
/// number not standing in its own place (`FUN_006a3f6a`); `None` when all
/// four are in play.
pub fn free_slot(order: [i16; MANAGERS]) -> Option<usize> {
    (1..=MANAGERS).find(|&n| order[n - 1] != n as i16)
}

/// Takes a newcomer into the game; his place among the season's managers,
/// or `None` with no slot left.
pub fn join(
    season: &mut Season,
    records: Option<&mut Records>,
    rules: &Rules,
    trainer: &Trainer,
) -> Option<usize> {
    let level = level(season.managers.iter().map(|m| m.level));
    let at = match records {
        Some(records) => enter(records, rules, &mut season.random, trainer, level)?,
        None if season.managers.len() < MOST => season.managers.len(),
        None => return None,
    };
    let newcomer = NewManager {
        first_name: trainer.first_name.clone(),
        last_name: trainer.last_name.clone(),
        club: None,
        national_team: None,
        level,
        difficulty: trainer.difficulty,
        negotiating: 0,
    };
    season.newcomer(rules, at, newcomer);
    Some(at)
}

/// The newcomer's record in his slot (`FUN_006a3b61`, as
/// [`crate::land_managers::create`] writes it), then his number and the
/// level of `FUN_006a5400`; his place among the humans in play.
fn enter(records: &mut Records, rules: &Rules, random: &mut Random, trainer: &Trainer, level: i32) -> Option<usize> {
    let slot = crate::land_managers::create(records, rules, trainer, random)?;
    let manager = &mut records.managers[slot - 1];
    manager.set_human(slot as u16);
    manager.set_level(level as u8);
    Some(records.globals.slots()[..slot - 1].iter().filter(|&&s| s > 0).count())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_newcomer_starts_three_levels_below_the_lowest() {
        assert_eq!(level([9, 6]), 3);
        assert_eq!(level([4]), 2, "never below 2");
        assert_eq!(level([20]), 15, "never above 15");
    }

    #[test]
    fn the_first_gap_in_the_order_is_his_slot() {
        assert_eq!(free_slot([1, 2, -1, -1]), Some(3));
        assert_eq!(free_slot([2, -1, -1, -1]), Some(1));
        assert_eq!(free_slot([1, 3, -1, -1]), Some(2));
        assert_eq!(free_slot([1, 2, 3, 4]), None);
    }
}
