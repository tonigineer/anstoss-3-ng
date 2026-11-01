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
//! day ([`luck_round_of_list`]): every squad player shifts his
//! freshness, loses fifty of his condition, and rolls seven of the
//! tens — a 0-roll nudges the contentment part it stands for a step
//! nearer a hundred ([`nudge`], [`crate::mood::change`]). Behind the
//! counter's
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

use crate::mood;
use crate::random::Random;
use crate::records::{Player, Records, Team};

/// The day of the world: `weekday` 0 sends the clubs of the more
/// countries through [`club`], 1 those of the main country; the other
/// weekdays' steps are not here.
pub fn day(records: &mut Records, weekday: i32, random: &mut Random) {
    match weekday {
        0 => {
            let countries = records.globals.more_countries();
            let more = records.globals.more_count().max(0) as usize;
            for k in 0..more.min(countries.len()) {
                clubs_of(&*records, countries[k], random);
            }
        }
        1 => clubs_of(&*records, records.globals.main_country(), random),
        _ => {}
    }
    luck_round_of_list(records, random);
    schedule_walk(&*records, random);
}

/// The day's step behind the counter's increment (`FUN_00675a73(1)`):
/// one roll of a hundred nudges the walk index at +0xb5df6a one step
/// towards its phase's target ([`walk_next`], `FUN_00783dc3`), and the
/// index then picks the day's rolls over the schedule table
/// ([`schedule_day`], `FUN_00783ea3`), which [`rotate`] has just
/// shifted by the walk's pair (the rotation `FUN_007842a1` draws
/// nothing); the day then re-rolls the table's fives ([`reroll_fives`])
/// and in the late phases fades some of its threes ([`fade_threes`],
/// `FUN_0078418b`). The phase machine steps no phase boundary at
/// A.an3 — its table day counts at +0xb5cbda are read but not written
/// back, and neither is the shifted table.
fn schedule_walk(records: &Records, random: &mut Random) {
    /// The walk's target per phase (`DAT_008acfec`, `FUN_00675a73`'s
    /// `_DAT_00b5cbdc` index).
    const TARGETS: [i16; 13] = [0, 5, 4, 3, 3, 2, 1, 0, 0, 1, 2, 3, 4];
    /// The table's shift per walk index (`DAT_008ad008`, handed to
    /// `FUN_007842a1` beside the index).
    const SHIFTS: [(i16, i16); 6] = [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1)];
    let phase = records.globals.get::<i16>(0xb5cbdc);
    let target = TARGETS.get(phase.max(0) as usize).copied().unwrap_or(0);
    let walk = records.globals.get::<i16>(0xb5df6a);
    let roll = i16::try_from(random.below(100)).unwrap();
    let walk = walk_next(walk, target, roll);
    let mut table: [u8; TABLE] = records.globals.get(0xb5d92a);
    if let Some(&(dx, dy)) = usize::try_from(walk).ok().and_then(|w| SHIFTS.get(w)) {
        rotate(&mut table, dx, dy);
    }
    schedule_day(walk, &mut table, random);
    reroll_fives(&mut table, random);
    if 5 < phase && phase < 9 {
        fade_threes(&mut table, random);
    }
}

/// The schedule table: forty rows of forty cells of small numbers,
/// at +0xb5d92a in the globals.
const TABLE: usize = 1600;

/// The table's day shift (`FUN_007842a1`): its cells step by the walk's
/// shift pair, and the rows and columns they shift out of keep their
/// old numbers — the table is copied first, and only the shifted cells
/// are written again.
fn rotate(table: &mut [u8; TABLE], dx: i16, dy: i16) {
    let source = *table;
    let (x0, x1): (i32, i32) = if dx < 0 { (1, 40) } else if dx == 0 { (0, 40) } else { (0, 39) };
    let (y0, y1): (i32, i32) = if dy < 0 { (1, 40) } else if dy == 0 { (0, 40) } else { (0, 39) };
    for x in x0..x1 {
        for y in y0..y1 {
            let at = usize::try_from(y + x * 40).unwrap();
            let to = usize::try_from(y + i32::from(dy) + (x + i32::from(dx)) * 40).unwrap();
            table[to] = source[at];
        }
    }
}

/// The day's numbers over the schedule table (`FUN_00783ea3`): the walk
/// index says which rows and columns get new numbers out of its rolls.
fn schedule_day(walk: i16, table: &mut [u8; TABLE], random: &mut Random) {
    let mut roll = |range: u32| u8::try_from(random.below(range)).unwrap();
    match walk {
        0 => {
            for row in 0..40 {
                table[39 + row * 40] = roll(2);
            }
        }
        1 => {
            for row in 0..40 {
                table[39 + row * 40] = roll(2);
            }
            for column in 0..20 {
                table[column] = roll(5) + 1;
            }
            for column in 20..40 {
                table[column] = roll(3) + 1;
            }
        }
        2 => {
            for column in 0..20 {
                table[column] = roll(5) + 1;
            }
            for column in 20..40 {
                table[column] = roll(3) + 1;
            }
        }
        3 => {
            for column in 0..20 {
                table[column] = roll(4);
            }
            for column in 20..40 {
                table[column] = roll(3) + 1;
            }
            for row in 0..40 {
                table[row * 40] = roll(4) + 1;
            }
        }
        4 => {
            for row in 0..40 {
                table[row * 40] = roll(4) + 1;
            }
        }
        5 => {
            for row in 0..40 {
                table[row * 40] = roll(3) + 1;
            }
            for column in 0..40 {
                table[1560 + column] = roll(4);
            }
        }
        _ => {}
    }
}

/// The day's re-roll of the table's fives (`FUN_0078418b`'s first
/// loop): every five over the whole table rolls a two and stands
/// again as a two or a three.
fn reroll_fives(table: &mut [u8; TABLE], random: &mut Random) {
    for cell in table.iter_mut() {
        if *cell == 5 {
            *cell = 2 + u8::try_from(random.below(2)).unwrap();
        }
    }
}

/// The late season's fading of the table's threes (`FUN_0078418b`'s
/// second loop, only in phases six to eight): every three rolls a
/// hundred, and a ten of them turn the cell into a six.
fn fade_threes(table: &mut [u8; TABLE], random: &mut Random) {
    for cell in table.iter_mut() {
        if *cell == 3 && random.below(100) < 10 {
            *cell = 6;
        }
    }
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

/// The round the day's driver runs after the weekday switch, over the
/// globals' list of human clubs (`FUN_007845aa` behind the wrapper
/// `FUN_007848f0`): the list stands at +0xb5cbe2 — one entry eight bytes
/// apart, the country's byte and the club's number two further — and its
/// count at +0xb5cc02; on every club every squad player rolls seven of
/// the tens, [`luck_round`].
fn luck_round_of_list(records: &mut Records, random: &mut Random) {
    let list: [u8; 32] = records.globals.get(0xb5cbe2);
    let count = records.globals.get::<i16>(0xb5cc02).max(0) as usize;
    for entry in list.chunks(8).take(count) {
        let country = u16::from(entry[0]);
        let number = u16::from_le_bytes([entry[2], entry[3]]);
        let Some(team) = records.team(country, number) else { continue };
        let ids = team.players();
        let human = team.humans() != [0, 0];
        let manager = human
            .then(|| records.managers.iter().find(|m| m.country() == country && m.team() == number))
            .flatten();
        let rank = manager.map_or(3, |m| (m.difficulty() & 0x7f).min(4) as usize);
        for id in ids {
            if let Some(player) = records.player_mut(id) {
                luck_player(player, FRESHENED[rank], random);
            }
        }
    }
}

/// The freshness a day's round adds to a squad player, by the class of
/// his club's manager (`FUN_007845aa`'s table at `DAT_008ad040`); a
/// club the game runs itself, with no manager of its own, sits at the
/// fourth.
const FRESHENED: [i16; 5] = [89, 85, 81, 70, 64];

/// One player's turn in the round (`FUN_007845aa`'s player body): the
/// last session's freshness stands in for the open one, that one drops
/// to nought, the season's gains a manager's step and stop at ten
/// thousand, and fifty of condition goes; then seven rolls of ten, a
/// 0-roll nudging one contentment part ([`nudge`]) in the loop's order:
/// trainer, team, private life, distractions, press, club, offers.
fn luck_player(player: &mut Player, freshened: i16, random: &mut Random) {
    let mut freshness = player.freshness();
    freshness[1] = freshness[2];
    freshness[2] = 0;
    freshness[0] = freshness[0].wrapping_add(freshened);
    if 10_000 < freshness[0] {
        freshness[0] = 10_000;
    }
    player.set_freshness(freshness);
    player.set_condition((i32::from(player.condition()) - 50).clamp(0, 10_000) as i16);
    for slot in [0, 1, 4, 5, 3, 2, 6] {
        if random.below(10) == 0 {
            nudge(player, slot);
        }
    }
}

/// A contentment part a step nearer a hundred, nought and the parts
/// above it staying put; his morale moves with it (`FUN_007007a1` and
/// its six twins, each called behind its slot's 0-roll).
fn nudge(player: &mut Player, slot: usize) {
    let value = i32::from(player.contentment()[slot]);
    if value < 100 {
        mood::change(player, slot, 1);
    } else if 100 < value {
        mood::change(player, slot, -1);
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

    /// One player's turn: his freshness shifts, fifty of condition
    /// goes, and he rolls seven of the tens — no more, whatever the
    /// rolls answer to.
    #[test]
    fn a_players_turn_shifts_his_freshness_and_rolls() {
        let mut player = Player::new(&vec![0u8; Player::SIZE]).unwrap();
        player.set_freshness([500, 300, 700]);
        player.set_condition(10_000);
        let mut random = Random::from_state(20261101, 6);
        let mut copy = random.clone();
        luck_player(&mut player, 70, &mut random);
        for _ in 0..7 {
            copy.below(10);
        }
        assert!(random == copy, "seven tens, no more");
        assert_eq!(player.freshness(), [570, 700, 0]);
        assert_eq!(player.condition(), 9_950);
    }

    /// A 0-roll's nudge: a part under a hundred climbs a step, one
    /// above it sinks one — to the setter's ceiling at a hundred and
    /// twenty — and a hundred stays; his morale follows the sum.
    #[test]
    fn a_nudge_walks_one_step_to_a_hundred() {
        let mut player = Player::new(&vec![0u8; Player::SIZE]).unwrap();
        player.set_contentment([50, 150, 100, 100, 100, 100, 100]);
        nudge(&mut player, 0);
        nudge(&mut player, 1);
        nudge(&mut player, 2);
        assert_eq!(player.contentment(), [51, 120, 100, 100, 100, 100, 100]);
        assert_eq!(player.morale(), 71, "six hundred seventy one less six hundred");
    }

    /// A hundred and twenty of a part stays its ceiling, and nought
    /// its floor, as the setter clamps both.
    #[test]
    fn a_nudge_never_leaves_nought_or_a_hundred_twenty() {
        let mut player = Player::new(&vec![0u8; Player::SIZE]).unwrap();
        player.set_contentment([0, 120, 101, 99, 100, 100, 100]);
        nudge(&mut player, 0);
        nudge(&mut player, 1);
        nudge(&mut player, 2);
        nudge(&mut player, 3);
        assert_eq!(player.contentment(), [1, 119, 100, 100, 100, 100, 100]);
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
            let mut table = [0u8; TABLE];
            schedule_day(walk, &mut table, &mut random);
            for (range, count) in rows {
                for _ in 0..*count {
                    copy.below(*range);
                }
            }
            assert!(random == copy, "walk {walk}'s rolls");
        }
    }

    /// A shift of the table: its cells step by the pair, and the strip
    /// they shift out of keeps its old numbers.
    #[test]
    fn the_table_shifts_its_cells() {
        let mut table = [0u8; TABLE];
        for x in 0..40 {
            for y in 0..40 {
                table[y + x * 40] = u8::try_from((x * 3 + y) % 20).unwrap();
            }
        }
        let before = table;
        rotate(&mut table, 1, -1);
        for x in 1..40 {
            for y in 0..39 {
                assert_eq!(table[y + x * 40], before[y + 1 + (x - 1) * 40], "shifted cell");
            }
        }
        for y in 0..40 {
            assert_eq!(table[y], before[y], "the emptied column stays");
        }
        for x in 0..40 {
            assert_eq!(table[39 + x * 40], before[39 + x * 40], "the emptied row stays");
        }
    }

    /// The day's re-roll: every five of the table rolls a two, and no
    /// other cell rolls at all.
    #[test]
    fn the_fives_reroll_and_nothing_else() {
        let mut table = [0u8; TABLE];
        table[7] = 5;
        table[8] = 4;
        table[500] = 3;
        table[1599] = 5;
        let mut random = Random::from_state(20261101, 9);
        let mut copy = random.clone();
        reroll_fives(&mut table, &mut random);
        let first = copy.below(2);
        let second = copy.below(2);
        assert!(random == copy, "one two per five");
        assert!(table[7] == 2 + u8::try_from(first).unwrap(), "a re-rolled five");
        assert!(table[1599] == 2 + u8::try_from(second).unwrap(), "the last one too");
        assert_eq!(table[8], 4, "a four sleeps");
        assert_eq!(table[500], 3, "a three sleeps");
    }
}
