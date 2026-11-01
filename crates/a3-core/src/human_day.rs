//! The human clubs' day (`FUN_0064f981`), as far as it is ported: the
//! draws that decide who may meet an event today, and the day's advance
//! ticket sales ([`crate::presale::day`]). The game draws everything from
//! one generator, so the replica draws these in the original's order even
//! where what follows them is not built yet.
//!
//! The slot a live gate draws is that day's own (`param_1` of
//! `FUN_0064f981`, the weekday it runs); a club meets the day's news
//! when its slot stands for that day, and the training runs for all
//! clubs in either case.
//!
//! Not ported yet, and drawing in the original when they run: the first
//! weekday's bulk (`FUN_0074ca91`, on day % 7 == 1), the monthly steps
//! (`FUN_00741829`, `FUN_00741672`, `FUN_007489cb`), the rare event of
//! one in a hundred (`FUN_005ea5ce`), and the events themselves and the
//! rest of the day after the day's news picked its category.

use crate::random::Random;

/// No event slot for a human today.
pub const NONE: i16 = -1;
const HUMANS: usize = 4;

/// The event gates of the day (`FUN_0064f981` from its first draw): one
/// chance in twenty that nobody gets an event, else one in a hundred for
/// the rare one, then for each of the four human slots two chances in
/// three of an event, the slot given the day of the week (`weekday`).
pub fn event_slots(random: &mut Random, weekday: i16) -> [i16; HUMANS] {
    if random.below(20) == 0 {
        return [NONE; HUMANS];
    }
    let _rare = random.below(100) == 0;
    std::array::from_fn(|_| if random.below(3) == 0 { NONE } else { weekday })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The gates take six draws when somebody may meet an event — twenty,
    /// a hundred and three for each slot — and a slot drawn 0 gets none.
    #[test]
    fn the_gates_draw_six_times_unless_nobody_meets_one() {
        let mut random = Random::from_state(12345, 67890);
        let mut copy = random.clone();
        let first = copy.below(20);
        assert_ne!(first, 0, "a seed where somebody may meet an event");
        copy.below(100);
        let thirds: Vec<u32> = (0..4).map(|_| copy.below(3)).collect();
        let slots = event_slots(&mut random, 5);
        assert!(random == copy);
        for (slot, third) in slots.iter().zip(thirds) {
            assert_eq!(*slot, if third == 0 { NONE } else { 5 });
        }
    }
}
