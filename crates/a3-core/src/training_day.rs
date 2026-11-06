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

/// Event 102's player events (`DAT_0088fe88`, registered at `0xa32794`
/// by `FUN_0060bdb7`): 31 pairs of weight and code, the weights summing
/// to the cap of 2011, so every roll meets one.
const PLAYER_EVENTS: [(u32, u32); 31] = [
    (90, 4),
    (90, 5),
    (60, 8),
    (30, 9),
    (120, 10),
    (30, 11),
    (150, 12),
    (30, 13),
    (60, 14),
    (60, 15),
    (90, 16),
    (30, 20),
    (30, 23),
    (60, 24),
    (30, 26),
    (90, 28),
    (30, 29),
    (30, 30),
    (30, 31),
    (120, 32),
    (60, 33),
    (60, 34),
    (120, 35),
    (60, 36),
    (90, 37),
    (120, 38),
    (90, 39),
    (90, 40),
    (30, 41),
    (30, 42),
    (1, 43),
];
/// A player event stands among the recent ones as its code plus this.
const PLAYER_EVENT_BASE: i32 = 13_000;
/// The recent events kept (`FUN_005e5ee0`'s twenty).
const RECENT: usize = 20;

/// The injuries a player-manager takes (`FUN_00766b92` with the table
/// at `DAT_008ac341`): the kind and the days out; no table flag is set
/// for these six.
const MANAGER_INJURIES: [(u8, i16); 6] = [(25, 7), (28, 7), (31, 3), (32, 7), (44, 7), (47, 3)];

/// A training day at each human club. `today` is the day counter,
/// `date` the day it stands on and `recent` the events lately met
/// ([`crate::season::Season::recent_events`]); the day's news follows
/// apart, in [`news`].
pub fn day(
    records: &mut Records,
    clubs: &mut BTreeMap<(usize, usize), ClubState>,
    world: &world::World,
    today: i32,
    date: NaiveDate,
    recent: &mut Vec<i32>,
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
                recent,
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
}

/// The day's news (`FUN_005ef842`, run by `FUN_0064f981` in its second
/// loop over the human clubs, after the training, the day's booked events
/// and the friendlies (`FUN_007615bb`)): a club meets it when the day's
/// gates drew today for its slot. The category pick, and category 19's
/// pick of one of the club's own players over its squad, stand in
/// `crate::news`. `slots` are the day's event gates of
/// [`crate::human_day::event_slots`], one per human club in the order of
/// [`crate::statistics::human_clubs`].
pub fn news(records: &Records, today: i32, slots: &[i16], random: &mut Random) {
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
    recent: &mut Vec<i32>,
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
        // `FUN_0060c1c0`'s events 100 to 110 ([`training_event`]); the
        // training luck behind comes when none was met (`FUN_0074a1ec`
        // on its answer of 0).
        let event = pick(&EVENTS, 10_000, random);
        let met = training_event(records, (country, number), ids, &trainable, event, today, recent, random);
        if !met && !trainable.is_empty() {
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

/// The training's event of the day (`FUN_0060c1c0` by its pick out of
/// [`EVENTS`], 0 for none): whether one was met. Its news is not posted.
/// 100 and 101 tire the trained men by three or five of their condition
/// when their forms today sum above twelve each — forms of 20 to 80
/// always do; 102 is a player event ([`player_event`]); 103 and 104 do
/// nothing; 108 has the co-trainer (the team's +0xa2, a person of
/// `0xbe3920` up to 1499) work one trained man who keeps no goal two up
/// in condition on a hundred-roll under seven times his skill (+0x24),
/// and 109 the second coach (+0xa4, a person of `0xbe17f8` up to 100)
/// one keeper on a sixty-roll under three times his skill and sixteen;
/// 110 puts the first of the squad, as many as train, out with their
/// knock once its count (+0x27c) passes five — 100 days for the groin,
/// 182 for the Achilles tendon, unless out longer already. Events 105
/// to 107 (`FUN_005eb193`'s men and their position bits) are not here
/// and stand as met.
#[allow(clippy::too_many_arguments)]
fn training_event(
    records: &mut Records,
    (country, number): (u16, u16),
    squad: &[u16],
    trainable: &[u16],
    event: u32,
    today: i32,
    recent: &mut Vec<i32>,
    random: &mut Random,
) -> bool {
    match event {
        0 => false,
        100 | 101 => {
            let forms: i32 =
                trainable.iter().filter_map(|&id| records.player(id)).map(statistics::form_today).sum();
            if (trainable.len() as i32) * 12 < forms {
                let by = if event == 100 { -3 } else { -5 };
                for &id in trainable {
                    if let Some(p) = records.player_mut(id) {
                        train_condition(p, by);
                    }
                }
            }
            true
        }
        102 => player_event(records, (country, number), trainable, recent, random),
        108 => {
            let co = records.team(country, number).map_or(0, |t| t.co_trainer());
            if co != 0 {
                let skill = if (1..=1499).contains(&co) {
                    records.persons.get(co as usize).map_or(0, |p| p.get::<u8>(0x24))
                } else {
                    0
                };
                if (random.below(100) as i32) < i32::from(skill) * 7
                    && let Some(id) = draw_listed(records, trainable, false, random)
                    && let Some(p) = records.player_mut(id)
                {
                    train_condition(p, 2);
                }
            }
            true
        }
        109 => {
            let coach = records.team(country, number).map_or(0, |t| t.get::<i16>(0xa4));
            if coach != 0 {
                let skill = if (1..=100).contains(&coach) {
                    records.block(&format!("0xbe17f8[{coach}]")).and_then(|b| b.get(0x24 - 0x20)).copied().unwrap_or(0)
                } else {
                    0
                };
                if (random.below(60) as i32) < i32::from(skill) * 3 + 16
                    && let Some(id) = draw_listed(records, trainable, true, random)
                    && let Some(p) = records.player_mut(id)
                {
                    train_condition(p, 2);
                }
            }
            true
        }
        110 => {
            for &id in squad.iter().take(trainable.len()) {
                let Some(p) = records.player_mut(id) else { continue };
                let knocks = p.knocks();
                let out = if knocks & crate::medical::ACHILLES != 0 {
                    Some((0x41, 0xb6))
                } else if knocks & crate::medical::GROIN != 0 {
                    Some((0x42, 100))
                } else {
                    None
                };
                if let Some((kind, days)) = out
                    && 5 < p.get::<u8>(0x27c)
                    && p.get::<i16>(0x2c) < 0xb4
                {
                    p.put::<u8>(0x2e, kind);
                    p.put::<i16>(0x2c, days);
                    p.put::<i16>(0x2a, today as i16);
                }
            }
            true
        }
        _ => true,
    }
}

/// Event 102 (`FUN_0060d119`): a player event out of [`PLAYER_EVENTS`],
/// drawn once more when it stands among the recent ones; met once one
/// is drawn that does not. Its run (`FUN_0060d1bf`) puts it among the
/// recent ones when it took place.
fn player_event(
    records: &mut Records,
    club: (u16, u16),
    trainable: &[u16],
    recent: &mut Vec<i32>,
    random: &mut Random,
) -> bool {
    for _ in 0..2 {
        let code = pick(&PLAYER_EVENTS, 2011, random);
        if code == 0 {
            return false;
        }
        let key = code as i32 + PLAYER_EVENT_BASE;
        if recent.contains(&key) {
            continue;
        }
        if run_player_event(records, club, trainable, code, random) {
            if recent.len() == RECENT {
                recent.remove(0);
            }
            recent.push(key);
        }
        return true;
    }
    false
}

/// A player event's run (`FUN_0060d1bf`, by its code less 4): whether
/// it took place; its news (`FUN_005f1e86`, `FUN_00601043`,
/// `FUN_005ee619`, none of them drawing) is not posted. Here are:
/// 4 and 5, the team's mood ten down or up (`FUN_007147ce`); 10 and 11,
/// a keeper drawn and four or three less content with the team
/// (`FUN_00700a6d`) — 10 answering "not met" all the same; 12 and 14, a
/// man who keeps no goal drawn and two or one more content; 31, two
/// friends in the training (case 0x1b): one man drawn, and when his best
/// friend (+0x204) trains too, both three more content. The other codes,
/// drawing over the squad, the manager and the club each in its own
/// way, take nothing place here yet.
fn run_player_event(
    records: &mut Records,
    (country, number): (u16, u16),
    trainable: &[u16],
    code: u32,
    random: &mut Random,
) -> bool {
    let content = |records: &mut Records, keepers: bool, by: i32, random: &mut Random| {
        let Some(id) = draw_listed(records, trainable, keepers, random) else { return false };
        if let Some(p) = records.player_mut(id) {
            crate::mood::change(p, 1, by);
        }
        true
    };
    match code {
        4 | 5 => {
            if let Some(team) = records.team_mut(country, number) {
                add_mood(team, if code == 4 { -10 } else { 10 });
            }
            true
        }
        10 => {
            content(records, true, -4, random);
            false
        }
        11 => content(records, true, -3, random),
        12 => content(records, false, 2, random),
        14 => content(records, false, 1, random),
        31 => {
            let Some(one) = draw_listed(records, trainable, false, random) else { return false };
            let friend = records.player(one).map_or(0, |p| p.best_friend());
            if friend == 0 || !trainable.contains(&friend) {
                return false;
            }
            for id in [one, friend] {
                if let Some(p) = records.player_mut(id) {
                    crate::mood::change(p, 1, 3);
                }
            }
            true
        }
        _ => false,
    }
}

/// One of the listed keepers, or of those who keep no goal, drawn
/// (`FUN_005ead2f`, `FUN_005eaed7`; the handle's position byte +0x3 is
/// the record's +0x21d): none when the list has none such, or when the
/// one drawn is a playing manager. Their last check, the handle's +0x6a
/// under the flag the event hands it, is not here.
fn draw_listed(records: &Records, list: &[u16], keepers: bool, random: &mut Random) -> Option<u16> {
    let such: Vec<u16> = list
        .iter()
        .copied()
        .filter(|&id| records.player(id).is_some_and(|p| (p.get::<u8>(0x21d) == 1) == keepers))
        .collect();
    if such.is_empty() {
        return None;
    }
    let id = such[random.below(such.len() as u32) as usize];
    (id <= crate::mood::LAST_REAL_ID).then_some(id)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Event 102's table as the exe registers it: its weights fill the cap
    /// of 2011, and the roll the original drew on A's day 64, 1112, lands
    /// on code 31, the two friends.
    #[test]
    fn the_player_events_fill_their_cap() {
        assert_eq!(PLAYER_EVENTS.iter().map(|e| e.0).sum::<u32>(), 2011);
        let mut rest = 1112;
        let code = PLAYER_EVENTS.iter().find_map(|&(weight, code)| {
            if weight > rest {
                return Some(code);
            }
            rest -= weight;
            None
        });
        assert_eq!(code, Some(31));
    }

    /// The training's events on A's human squad: 100 and 101 take three
    /// and five of every trained man's condition and draw nothing, 103
    /// does nothing, and 108 rolls the co-trainer's hundred, picking a man
    /// only under seven times his skill.
    #[test]
    #[ignore = "reads the oracle's kept saves beside $ANSTOSS3_DIR"]
    fn the_training_events_work_on_the_squad() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let save = a3_files::an3::Save::read(&dir.parent().unwrap().join("saves-kept/A.an3")).unwrap();
        let mut records = Records::from_save(save).unwrap();
        let (country, number) = statistics::human_clubs(&records)[0];
        let club = (u16::from(country), number as u16);
        let squad = records.team(club.0, club.1).unwrap().players();
        let trainable: Vec<u16> =
            squad.iter().copied().filter(|&id| records.player(id).is_some_and(|p| p.get::<i16>(0x2c) < 7)).collect();
        let conditions = |records: &Records| -> Vec<i16> {
            trainable.iter().map(|&id| records.player(id).unwrap().condition()).collect()
        };
        let start = Random::from_state(12345, 67890);
        let mut random = start.clone();
        let mut recent = Vec::new();
        for (event, by) in [(100, 300), (101, 500), (103, 0)] {
            let before = conditions(&records);
            assert!(training_event(&mut records, club, &squad, &trainable, event, 64, &mut recent, &mut random));
            let after = conditions(&records);
            for (b, a) in before.iter().zip(&after) {
                assert_eq!(*a, (b - by).max(0), "event {event}");
            }
        }
        assert_eq!(random, start, "the tiring events draw nothing");
        let co = records.team(club.0, club.1).unwrap().co_trainer();
        let skill = records.persons.get(co.max(0) as usize).map_or(0, |p| p.get::<u8>(0x24));
        let mut expected = start.clone();
        if co != 0 && (expected.below(100) as i32) < i32::from(skill) * 7 {
            expected.below(trainable.iter().filter(|&&id| records.player(id).unwrap().get::<u8>(0x21d) != 1).count() as u32);
        }
        assert!(training_event(&mut records, club, &squad, &trainable, 108, 64, &mut recent, &mut random));
        assert_eq!(random, expected);
        // The player events' plainest: the team's mood ten down and up
        // again, and code 12's one draw over the outfield, one man two
        // more content with the team.
        let mood = i32::from(records.team(club.0, club.1).unwrap().mood());
        assert!(run_player_event(&mut records, club, &trainable, 4, &mut random));
        let down = (mood - 10).max(0);
        assert_eq!(i32::from(records.team(club.0, club.1).unwrap().mood()), down);
        assert!(run_player_event(&mut records, club, &trainable, 5, &mut random));
        assert_eq!(i32::from(records.team(club.0, club.1).unwrap().mood()), (down + 10).min(200));
        assert_eq!(random, expected, "the mood events draw nothing");
        let outfield = trainable.iter().filter(|&&id| records.player(id).unwrap().get::<u8>(0x21d) != 1).count();
        let team_parts = |records: &Records| -> i32 {
            trainable.iter().map(|&id| i32::from(records.player(id).unwrap().contentment()[1])).sum()
        };
        let before = team_parts(&records);
        assert!(run_player_event(&mut records, club, &trainable, 12, &mut random));
        expected.below(outfield as u32);
        assert_eq!(random, expected);
        assert!((0..=2).contains(&(team_parts(&records) - before)));
    }
}
