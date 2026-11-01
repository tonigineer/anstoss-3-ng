//! One training day at the human clubs: the day's planned units take
//! their sessions, pickers send an injury or an event ahead, and the
//! day looks for one trained player whose luck might turn
//! (`FUN_00749508` with `FUN_0074a1ec`, which the day's advance
//! `FUN_0064f981` runs after the ticket sales).
//!
//! What is not here yet: the day's snapshot of the squad
//! (`FUN_00746c48`) — and with it the block which decays the positions
//! at the week's start and the units' position development with their
//! rolls (`FUN_00747246`; nothing of it rolled in the day-63
//! comparison, where draws 8 to 10 stand); the training gains
//! (`FUN_0074ae97`, `FUN_00767225`); what the day's news posts once its
//! category is picked (`FUN_005ee5c0` with `FUN_005ef430` and
//! `FUN_005f1eaf`; the pick itself stands in [`crate::news`]); the
//! events 100 to 110
//! of `FUN_0060c1c0`; the camp's own happenings (`FUN_0060ff69`); the
//! writes of `FUN_0041828e` behind the secret flag; the flag helpers
//! `FUN_0069851b` and friends; the player-manager's block behind the
//! club's byte +0x8a; and the third case of `FUN_00745d3c`, which
//! weighs two abilities.

use std::collections::BTreeMap;

use chrono::NaiveDate;

use crate::lineup;
use crate::random::Random;
use crate::records::{Player, Records, Team};
use crate::season::ClubState;
use crate::statistics;
use crate::training::{self, FREE};
use crate::world;

/// The unit rows (`DAT_008aa590`, 0x34 bytes each): the flags of a
/// unit's side effects at +0x14 and the talent for individual training
/// at +0xc.
const FLAGS: [u32; 39] = [
    0x00_0000,
    0x00_0001,
    0x03_0020,
    0x00_0000,
    0x00_0002,
    0x00_0000,
    0x00_0000,
    0x00_0004,
    0x00_0008,
    0x00_0000,
    0x00_0010,
    0x00_0000,
    0x00_0000,
    0x00_0000,
    0x00_0000,
    0x00_0000,
    0x00_0000,
    0x00_0000,
    0x00_0000,
    0x00_0700,
    0x00_0000,
    0x00_0040,
    0x00_0000,
    0x00_0020,
    0x00_0020,
    0x00_0000,
    0x00_0000,
    0x00_0000,
    0x04_0000,
    0x04_0000,
    0x00_0000,
    0x00_0000,
    0x00_0000,
    0x08_0000,
    0x00_0000,
    0x00_0000,
    0x00_0020,
    0x00_0000,
    0x08_0000,
];
const TALENT: [i16; 39] = [
    150, 60, 150, 30, 60, 100, 60, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 80, 30, 50, 150, 50, -1,
    50, 50, 50, -1, 500, 50, 50, 50, 30, 50, 50, 30, 50, 50, 80, 50,
];

/// The pickers' tables (`FUN_0060c015` and `FUN_0060c1c0`): weight and
/// code, a total of 10000 each.
const INJURIES: [(u32, u32); 2] = [(4, 1), (4, 2)];
const EVENTS: [(u32, u32); 11] = [
    (10, 100),
    (4, 101),
    (280, 102),
    (3, 103),
    (3, 104),
    (5, 105),
    (3, 106),
    (3, 107),
    (800, 108),
    (300, 109),
    (100, 110),
];

/// The injuries a player-manager takes (`FUN_00766b92` with the table
/// at `DAT_008ac341`): the kind and the days out; no table flag is set
/// for these six.
const MANAGER_INJURIES: [(u8, i16); 6] = [(25, 7), (28, 7), (31, 3), (32, 7), (44, 7), (47, 3)];

/// A training day at each human club. `today` is the day counter,
/// `date` the day it stands on and `slots` the day's event gates of
/// [`crate::human_day::event_slots`], one per human club in the order of
/// [`crate::statistics::human_clubs`].
pub fn day(
    records: &mut Records,
    clubs: &mut BTreeMap<(usize, usize), ClubState>,
    world: &world::World,
    today: i32,
    date: NaiveDate,
    slots: &[i16],
    random: &mut Random,
) {
    let weekday = today.rem_euclid(7) as usize;
    for (country, number) in statistics::human_clubs(records) {
        let Ok(place) = usize::try_from(number) else { continue };
        let (country, number) = (u16::from(country), number as u16);
        // The season's clubs stand under their country's slot; the club's
        // place in that list is its number in the records.
        let Some(slot) = world
            .countries
            .iter()
            .position(|c| c.as_ref().is_some_and(|c| c.nation == country))
        else {
            continue;
        };
        let Some(state) = clubs.get_mut(&(slot, place)) else { continue };
        let Some(team) = records.team(country, number) else { continue };
        let human = team.human();
        if human == 0 {
            continue;
        }
        let slot = usize::from(human) - 1;
        let ids = team.players();
        let gates = team.get::<u16>(0x95c);
        let secret = team.passing() & training::SECRET != 0;
        let flags = state.training.flags;
        let mut plan = state.training.plan();
        if weekday == 0 {
            // The week's start also decays the snapshot's positions —
            // no rolls, and the snapshot is not here. Whose condition
            // the two rolls below train is lost in the decompile; the
            // replica trains the squad member whose turn it is.
            for roll in [10, 100] {
                let on = gates & if roll == 10 { 8 } else { 0x80 } != 0;
                if !on {
                    continue;
                }
                for i in 0..ids.len() {
                    if random.below(roll) == 0 {
                        if let Some(player) = records.player_mut(ids[i]) {
                            train_condition(player, 1);
                        }
                    }
                }
            }
        } else if weekday == 1 && flags & 4 != 0 {
            // The team meeting trains nobody that day.
            state.training.set(4, false);
            for k in 0..training::SESSIONS {
                state.training.set_unit(1, k, FREE);
            }
            plan = state.training.plan();
        }
        let mut counter = 0;
        for k in 0..training::SESSIONS {
            let unit = usize::from(plan[weekday * training::SESSIONS + k]);
            if unit < TALENT.len() {
                if let Some(extra) = records.extras.get_mut(slot) {
                    let mut units = extra.season_units();
                    units[unit] += 1;
                    extra.set_season_units(units);
                }
            }
            if unit == usize::from(FREE) {
                continue;
            }
            if secret {
                // `FUN_0041828e`'s happenings are not here; the rolls
                // still go.
                if random.below(6) == 0 {}
                if random.below(14) < 2 {}
            }
            counter += 1;
            session(
                records,
                state,
                country,
                number,
                &ids,
                gates,
                unit,
                counter,
                today,
                date,
                slot,
                random,
            );
        }
        if flags & training::CAMP != 0 {
            // `FUN_0060ff69`'s camp happenings are not here.
            if random.below(7) == 0 {
                let mut rooms = [0; 0x15];
                for &id in &ids {
                    if let Some(player) = records.player(id) {
                        let room = i32::from(player.room());
                        if room > 0 && room < 0x15 {
                            rooms[room as usize] += 1;
                        }
                    }
                }
                let alone = rooms[1..0x15].iter().filter(|&&c| c == 1).count() as i32;
                if alone > 0 {
                    // The original gates this on a run-time team word
                    // (+0x0, `% 0xe000`); the save keeps no such word,
                    // so the write stands here on its own.
                    if let Some(team) = records.team_mut(country, number) {
                        let will = (i32::from(team.will_to_win()) - alone).clamp(-200, 200);
                        team.set_will_to_win(will as i16);
                    }
                }
            }
        }
    }
    // The day's news (`FUN_005ef842`, run by `FUN_0064f981` in a second
    // loop over the human clubs after the training): a club meets it when
    // the day's gates drew today for its slot. The category pick, and
    // category 19's pick of one of the club's own players over its squad,
    // stand in `crate::news`.
    for (k, (country, number)) in statistics::human_clubs(records).into_iter().enumerate() {
        if slots.get(k) == Some(&(today.rem_euclid(7) as i16)) {
            let code = crate::news::pick(random);
            if code == 19 {
                let (country, number) = (u16::from(country), number as u16);
                let squad = records
                    .team(country, number)
                    .map_or(0, |team| team.player_count());
                crate::news::about_a_player(squad, random);
            }
        }
    }
}

/// One session of the day (`FUN_0074a1ec` at intensity 0, with nothing
/// asked back).
fn session(
    records: &mut Records,
    state: &ClubState,
    country: u16,
    number: u16,
    ids: &[u16],
    gates: u16,
    unit: usize,
    counter: u32,
    today: i32,
    date: NaiveDate,
    slot: usize,
    random: &mut Random,
) {
    let mut trainable = Vec::new();
    let mut injured = Vec::new();
    for &id in ids {
        let Some(player) = records.player(id) else { continue };
        if player.get::<i16>(0x2c) < 7 {
            // The national check (`FUN_006fef73`, `FUN_006feff8`) is
            // not in the corpus and caps nothing here; a 0x20 unit only
            // trains its own kind — the record's scratch byte +0x3,
            // kept at +0x21d by the load copies. `FUN_0074ae97`'s gains
            // are not here.
            if unit != 0x20 || player.get::<u8>(0x21d) == 1 {
                trainable.push(id);
            }
        } else {
            injured.push(id);
        }
    }
    if i32::from(records.team(country, number).map_or(0, |t| t.player_count())) / 2
        < trainable.len() as i32
    {
        let flags = FLAGS[unit];
        if flags & 1 != 0 {
            if let Some(team) = records.team_mut(country, number) {
                let off = i32::from(team.offside());
                let (roll, add) = if off < 100 {
                    (random.below(3), 4)
                } else {
                    (random.below(2), 3)
                };
                let sum = (off + add + roll as i32) as u8;
                team.set_offside(sum.min(200));
            }
        }
        for bit in [2, 4, 8, 0x10, 0x40] {
            if flags & bit != 0 && random.below(100) < 0x46 {
                // The helpers `FUN_0069851b` and friends are not here.
            }
        }
        for _ in 0..2 {
            if flags & 0x20 != 0 && random.below(100) < 0x14 {
                // `FUN_0069893d` and `FUN_00698b8d` are not here.
            }
        }
        if unit == 9 && !trainable.is_empty() {
            let roll = random.below(trainable.len() as u32);
            if let Some(team) = records.team_mut(country, number) {
                let sum = (i32::from(team.get::<u8>(0xa4))
                    + trainable.len() as i32 / 3
                    + roll as i32) as u8;
                team.put::<u8>(0xa4, sum.min(200));
            }
        } else if unit == 0x1a {
            if let Some(team) = records.team_mut(country, number) {
                add_mood(team, 2);
                if team.will() < 0x32 {
                    let will = (i32::from(team.will_to_win()) + 1).clamp(-200, 200);
                    team.set_will_to_win(will as i16);
                }
            }
        } else if unit == 0x21 {
            if let Some(team) = records.team_mut(country, number) {
                add_mood(team, 1);
            }
        }
    }
    if !trainable.is_empty() && TALENT[unit] > 0 {
        let at = random.below(trainable.len() as u32) as usize;
        let id = trainable[at];
        let mut fc = i32::from(TALENT[unit]);
        if let Some(player) = records.player(id) {
            let fresh = player.freshness();
            let fe = (i32::from(fresh[0]) + i32::from(fresh[1]) + i32::from(fresh[2])) / 100;
            let fe = fe.clamp(0, 100);
            if fe < 0x3c {
                fc = fc * (fe + 0x28) / 100;
            }
            if gates & 0x20 != 0 {
                fc += fc / 10;
            }
        }
        // `FUN_00767225`'s gain and its news are not here.
        if random.below(fc.max(0) as u32) == 0 {}
    }
    if counter == 1 {
        if !injured.is_empty() {
            let mut code = pick(&INJURIES, 10_000, random);
            if gates & 1 != 0 && random.below(100) == 0 {
                code = 3;
            }
            if code != 0 {
                let at = random.below(injured.len() as u32) as usize;
                if let Some(player) = records.player_mut(injured[at]) {
                    let mut days = player.get::<i16>(0x2c);
                    if days < 0x2b {
                        if days > 0x15 {
                            if code == 1 {
                                days -= 7;
                            } else if code == 2 {
                                days += 7;
                            }
                        }
                    } else if code == 3 {
                        days -= 14;
                    } else if code == 1 {
                        days -= 14;
                    } else if code == 2 {
                        days += 14;
                    }
                    player.put::<i16>(0x2c, days);
                }
            }
        }
        // `FUN_0060c1c0`'s events 100 to 110 are not here; only the
        // pick stands, and the training luck behind it.
        let event = pick(&EVENTS, 10_000, random);
        if event == 0 && !trainable.is_empty() {
            let at = random.below(trainable.len() as u32) as usize;
            let id = trainable[at];
            luck(records, id, date, random);
            if id > 17_000 {
                let rival = records.managers.get(slot).map_or(-1, |m| m.rival());
                if rival >= 0 {
                    for &other in &trainable {
                        if other == rival as u16 {
                            if random.below(3) != 0 {
                                let at = random.below(6) as usize;
                                if let Some(player) = records.player_mut(other) {
                                    player.put::<u8>(0x2e, MANAGER_INJURIES[at].0);
                                    player.put::<i16>(0x2c, MANAGER_INJURIES[at].1);
                                    player.put::<i16>(0x2a, today as i16);
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }
    }
    // A hard unit in a line-up that stands fit tires the mood; a free
    // unit's roll never comes, those sessions being skipped above.
    if lineup::average(state, lineup::condition) > 0x4b && (unit == 0x11 || unit == 6) {
        if let Some(team) = records.team_mut(country, number) {
            let mood = i32::from(team.mood());
            if mood > 3 {
                team.set_mood((mood - 3) as u8);
            }
        }
    }
}

/// The luck of one trained player (`FUN_0074a1ec`'s chain): a young
/// one rises, a good day rests him, else his rank stands or falls. The
/// chain works on the record's scratch bytes before the save begins —
/// the load copies give their kept places: the rank byte (+0x2) at
/// +0x21c, the feet (+0x3 to +0x5) at +0x21d to +0x21f, the line-up
/// flag (+0x10) at +0x1c8 (`FUN_00636638` and friends). The news of
/// each turn (`FUN_005ee5c0` and friends) is not posted.
fn luck(records: &mut Records, id: u16, date: NaiveDate, random: &mut Random) {
    let Some(player) = records.player_mut(id) else { return };
    let age = statistics::age_on(player, date);
    let mut rank = player.get::<u8>(0x21c);
    if rank < 5 {
        if random.below(0x4b0) == 0 {
            rank += 1;
        }
    } else if statistics::form_today(player) < 0xc {
        if random.below(0x2ee) == 0 {
            train_condition(player, 5);
        }
    } else if !(statistics::form_today(player) < 0xb || random.below(0x1c2) != 0) {
        train_freshness(player, -5);
    } else {
        let played = player.get::<u8>(0x1c8);
        let willing = played & 4 == 0
            || rank < 2
            || rank > 5
            || statistics::form_today(player) > 8
            || random.below(400) != 0;
        if !willing {
            rank -= 1;
        } else {
            let going = age < 0x20
                || statistics::form_today(player) > 8
                || rank < 2
                || rank > 6
                || random.below(200) != 0;
            if !going {
                rank -= 1;
            } else {
                let mut grew = false;
                if rank < 4
                    && statistics::form_today(player) > 7
                    && age < 0x18
                    && random.below(200) == 0
                {
                    rank += 1;
                    grew = true;
                }
                if !grew && rank > 7 {
                    let side = player.get::<u8>(0x21e);
                    if side == 0 && age > 0x1e && random.below(400) == 0 {
                        let foot = player.get::<u8>(0x21d);
                        let other = player.get::<u8>(0x21f);
                        if foot != 1 && foot != 2 && other != 2 {
                            // `FUN_00745d3c(p, 2)`; the weighing of two
                            // abilities is not here.
                            if player.get::<u8>(0x21e) == 0 {
                                player.put::<u8>(0x21e, 2);
                            } else if player.get::<u8>(0x21f) == 0 {
                                player.put::<u8>(0x21f, 2);
                            }
                        }
                    }
                }
            }
        }
    }
    player.put::<u8>(0x21c, rank);
}

/// And of freshness (`FUN_005e5f40`).
fn train_freshness(player: &mut Player, steps: i32) {
    let mut fresh = player.freshness();
    fresh[0] = (i32::from(fresh[0]) + steps * 100).clamp(0, 10_000) as i16;
    player.set_freshness(fresh);
}

/// `FUN_0060be4d`: one roll over the weights; the first entry heavier
/// than what the roll left takes the day.
fn pick(table: &[(u32, u32)], total: u32, random: &mut Random) -> u32 {
    let mut rest = random.below(total);
    for &(weight, code) in table {
        if weight > rest {
            return code;
        }
        rest -= weight;
    }
    0
}

/// A hundredth of a point of condition per step (`FUN_00411130`).
fn train_condition(player: &mut Player, steps: i32) {
    let condition = i32::from(player.condition()) + steps * 100;
    player.set_condition(condition.clamp(0, 10_000) as i16);
}

/// The team's mood, within 0 to 200 (`FUN_007147ce`).
fn add_mood(team: &mut Team, steps: i32) {
    let mood = i32::from(team.mood()) + steps;
    team.set_mood(mood.clamp(0, 200) as u8);
}
