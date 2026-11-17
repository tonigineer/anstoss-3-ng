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
        Some(records) => enter(records, &mut season.random, trainer, level)?,
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

/// The newcomer's record in his slot (`FUN_006a3b61`, then the level of
/// `FUN_006a5400`); his place among the humans in play.
fn enter(records: &mut Records, random: &mut Random, trainer: &Trainer, level: i32) -> Option<usize> {
    let mut order = records.globals.order();
    let slot = free_slot(order)?;
    // The order moves up behind him; he stands in his own place.
    order.copy_within(slot - 1..MANAGERS - 1, slot);
    order[slot - 1] = slot as i16;
    records.globals.set_order(order);
    let mut slots = records.globals.slots();
    slots[slot - 1] = 1;
    records.globals.set_slots(slots);
    records.globals.set_humans(records.globals.humans() + 1);
    let mode = records.globals.mode();
    let seasons = records.globals.seasons();
    let day = records.globals.day();
    let manager = records.managers.get_mut(slot - 1)?;
    manager.clear_with_names(&trainer.first_name, &trainer.last_name);
    manager.set_human(slot as u16);
    manager.set_level(level as u8);
    // +0x33: the playing strength as player-manager.
    manager.put(0x33, 1u8);
    let [day_of, month, year] = trainer.birthday;
    manager.set_birth_day(day_of as u8);
    manager.set_birth_month(month as u8);
    manager.set_birth_year(year as u16);
    manager.set_difficulty(trainer.difficulty as u16);
    // +0x32: the position he plays, 1 to 10.
    manager.put(0x32, (trainer.position as u8).clamp(1, 10));
    // The public, the fans and the media: 40 to 60 each, the fans first.
    let fans = 40 + random.below(21) as u8;
    let public = 40 + random.below(21) as u8;
    let media = 40 + random.below(21) as u8;
    manager.set_popularity([public, fans, media]);
    manager.put(0x11e, 100i16);
    manager.put(0x11c, 0x50i16);
    manager.set_mode(mode);
    manager.put(0x12c, seasons);
    manager.put(0x12e, day);
    manager.set_trainer_years(1);
    manager.put(0x1f68, 5i16);
    manager.put(0x1f6a, 1i16);
    manager.put(0x19bf, 1u8);
    // His abilities at every level are the level itself.
    manager.set_abilities(std::array::from_fn(|row| [row as i16; 7]));
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
