//! One training day at the human clubs: the day's planned units take
//! their sessions, pickers send an injury or an event ahead, and the
//! day looks for one trained player whose luck might turn
//! (`FUN_00749508` with `FUN_0074a1ec`, which the day's advance
//! `FUN_0064f981` runs after the ticket sales).
//!
//! Here besides: the session's injury (`FUN_00767225`), the unit's team
//! skills (`FUN_0069851b` and its kin), the events 100 to 110 of
//! `FUN_0060c1c0` with 27 of event 102's 31 player events, the secret
//! training's toll (`FUN_0041828e`), and the day's news pick after the
//! friendlies ([`news`]).
//!
//! What is not here yet: the day's snapshot of the squad
//! (`FUN_00746c48`) — and with it the block which decays the positions
//! at the week's start and the units' position development with their
//! rolls (`FUN_00747246`; nothing of it rolled in the day-63
//! comparison, where draws 8 to 10 stand); the training gains
//! (`FUN_0074ae97`); what the day's news posts once its category is
//! picked (`FUN_005ee5c0` with `FUN_005ef430` and `FUN_005f1eaf`; the
//! pick itself stands in [`crate::news`]), and the events' news; the
//! player events 8, 20, 26 and 34; the camp's own happenings
//! (`FUN_0060ff69`); the player-manager's block behind the club's byte
//! +0x8a; and the third case of `FUN_00745d3c`, which weighs two
//! abilities.

use std::collections::BTreeMap;

use chrono::NaiveDate;

use crate::lineup;
use crate::league::LeagueTables;
use crate::medical::MedicalTables;
use crate::mood::MoodTables;
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
#[allow(clippy::too_many_arguments)]
pub fn day(
    records: &mut Records,
    clubs: &mut BTreeMap<(usize, usize), ClubState>,
    world: &world::World,
    today: i32,
    date: NaiveDate,
    (mood, medical, league): (&MoodTables, &MedicalTables, &LeagueTables),
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
        // The day's three planned units (the club object's +0xc0a, the
        // week's row of its plan): event 26 asks the first two of them.
        let day_units: [u8; training::SESSIONS] =
            plan[weekday * training::SESSIONS..][..training::SESSIONS].try_into().unwrap();
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
                // Training in secret costs the trainer a little of the
                // media's liking now and then, and of the fans'
                // (`FUN_00749508` → `FUN_0041828e`).
                if random.below(6) == 0 {
                    move_likings(records, (country, number), [0, 0, -1]);
                }
                if random.below(14) < 2 {
                    move_likings(records, (country, number), [0, -1, 0]);
                }
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
                day_units,
                counter,
                today,
                date,
                slot,
                (mood, medical, league),
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
    day_units: [u8; 3],
    counter: u32,
    today: i32,
    date: NaiveDate,
    slot: usize,
    (mood, medical, league): (&MoodTables, &MedicalTables, &LeagueTables),
    recent: &mut Vec<i32>,
    random: &mut Random,
) {
    let mut trainable = Vec::new();
    let mut injured = Vec::new();
    for &id in ids {
        let Some(player) = records.player(id) else { continue };
        if days_out(records, player) < 7 {
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
        // The unit's skills, each a step up within 0 to 5 on its roll, in
        // the exe's order: seventy in a hundred for the anti-offside
        // (flag 2, `FUN_00698a4b`, club +0xa1 = team +0xc7) and three
        // fields of the set pieces' word (4, 8, 0x10: its bits 3, 6 and
        // 0; `FUN_00698616`, `FUN_0069871d`, `FUN_0069851b`; club +0xa2
        // = team +0xc8); twenty twice for 0x20's two (team +0xcb,
        // `FUN_00698b8d`, club +0xa5; the word's bits 12, `FUN_0069893d`);
        // seventy for the pressing (0x40, `FUN_00698aec`, club +0x9f =
        // team +0xc5).
        for (bit, shift) in [(2u32, None), (4, Some(3)), (8, Some(6)), (0x10, Some(0))] {
            if flags & bit != 0
                && random.below(100) < 0x46
                && let Some(team) = records.team_mut(country, number)
            {
                match shift {
                    None => team.set_anti_offside(skill_step(team.anti_offside())),
                    Some(shift) => step_set_piece(team, shift),
                }
            }
        }
        if flags & 0x20 != 0 {
            if random.below(100) < 0x14
                && let Some(team) = records.team_mut(country, number)
            {
                team.set_tactic_extra(skill_step(team.tactic_extra()));
            }
            if random.below(100) < 0x14
                && let Some(team) = records.team_mut(country, number)
            {
                step_set_piece(team, 12);
            }
        }
        if flags & 0x40 != 0
            && random.below(100) < 0x46
            && let Some(team) = records.team_mut(country, number)
        {
            team.set_pressing(skill_step(team.pressing()));
        }
        if unit == 9 && !trainable.is_empty() {
            let roll = random.below(trainable.len() as u32);
            if let Some(team) = records.team_mut(country, number) {
                // The club's +0xa4 is the team's +0xca (`FUN_0063b75a`).
                let sum = (i32::from(team.get::<u8>(0xca))
                    + trainable.len() as i32 / 3
                    + roll as i32) as u8;
                team.put::<u8>(0xca, sum.min(200));
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
        // Its 0 hurts him (`FUN_00767225`).
        if random.below(fc.max(0) as u32) == 0 {
            training_injury(records, (country, number), id, today, date, medical, random);
        }
    }
    if counter == 1 {
        if !injured.is_empty() {
            let mut code = pick(&INJURIES, 10_000, random);
            if gates & 1 != 0 && random.below(100) == 0 {
                code = 3;
            }
            if code != 0 {
                let at = random.below(injured.len() as u32) as usize;
                let id = injured[at];
                if let Some(player) = records.player(id) {
                    let mut days = days_out(records, player);
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
                    set_days_out(records, id, days);
                }
            }
        }
        // `FUN_0060c1c0`'s events 100 to 110 ([`training_event`]); the
        // training luck behind comes when none was met (`FUN_0074a1ec`
        // on its answer of 0).
        let event = pick(&EVENTS, 10_000, random);
        let met = training_event(records, (country, number), ids, &trainable, (mood, league), event, unit, day_units, (today, date), recent, random);
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
                                let (kind, days) = MANAGER_INJURIES[at];
                                injure(records, other, kind, days.into(), today);
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
/// 182 for the Achilles tendon, unless out longer already; 105 to 107
/// draw one trained man ([`draw_any`]) for his abilities and traits
/// ([`ability_event`], [`trait_event`]).
#[allow(clippy::too_many_arguments)]
fn training_event(
    records: &mut Records,
    (country, number): (u16, u16),
    squad: &[u16],
    trainable: &[u16],
    tables: (&MoodTables, &LeagueTables),
    event: u32,
    unit: usize,
    day_units: [u8; 3],
    (today, date): (i32, NaiveDate),
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
        102 => player_event(records, (country, number), trainable, tables, unit, day_units, (today, date), recent, random),
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
                let Some(p) = records.player(id) else { continue };
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
                    && days_out(records, p) < 0xb4
                {
                    injure(records, id, kind, days, today);
                }
            }
            true
        }
        105 => ability_event(records, (country, number), trainable, random),
        106 | 107 => trait_event(records, (country, number), trainable, event == 106, random),
        _ => true,
    }
}

/// The cap of a medical level's spending (`FUN_006773c4` on the club's
/// +0x2ac, the team's +0x314): below each one level less, 4 at most —
/// 150, 400, 800 and 1700 thousands (`FUN_00627bd0`, a thousand being
/// `DAT_00a3b3e4`).
const MEDICAL_LEVELS: [i32; 4] = [150_000, 400_000, 800_000, 1_700_000];
/// What a day less out costs (`DAT_00a3b37c`, a hundred thousands).
const MEDICAL_DAY: i32 = 100_000;
/// The injury kinds a session draws over (`FUN_00767225`): 1 to 61.
const TRAINING_KINDS: usize = 61;

/// A session's injury (`FUN_00767225`): a kind out of the table by its
/// weights — the walk stops one band late, as the exe's does, on the
/// kind after the one the roll falls in (but at the last); a knock kind
/// adds the knock bits of the row the roll itself names (`DAT_008ac348`
/// by the roll) and clears the knock's count, else he is out: kind 0x25
/// as 0x43 when his freshness is under 35, for the kind's days scaled
/// by what the club spends on its doctors (a hundred less three per
/// medical level and a draw below a day's cost in its spending, over a
/// hundred; one day at least), and kinds 0x1b and 0x1e may leave him
/// knocked about by the knee (`FUN_007675c6`).
fn training_injury(
    records: &mut Records,
    (country, number): (u16, u16),
    id: u16,
    today: i32,
    date: NaiveDate,
    medical: &MedicalTables,
    random: &mut Random,
) {
    let mut starts = [0i32; TRAINING_KINDS];
    let mut total = 0i32;
    for (k, start) in starts.iter_mut().enumerate() {
        *start = total;
        total += i32::from(medical.weight(k as u8 + 1));
    }
    if total == 0 {
        return;
    }
    let roll = random.below(total as u32) as i32;
    let kind = band_after(&starts, roll) as u8 + 1;
    let Some(p) = records.player(id) else { return };
    if medical.knock_bits(usize::from(kind)) != 0 {
        let bits = medical.knock_bits(roll as usize);
        if let Some(p) = records.player_mut(id) {
            p.set_knocks(p.knocks() | bits);
            p.put::<u8>(0x27c, 0);
        }
        return;
    }
    let kind = if kind == 0x25 && crate::computer_training::fresh_percent(p) < 0x23 { 0x43 } else { kind };
    let spent = records.team(country, number).map_or(0, |t| t.get::<i32>(0x314));
    let level = MEDICAL_LEVELS.iter().filter(|&&cap| cap <= spent).count() as i64;
    let care = i64::from(random.below((spent / MEDICAL_DAY).max(0) as u32));
    let base = i64::from(medical.base_days(kind));
    // The factor goes to the FPU as a 64-bit integer whose high word is
    // nought — below nought it is read as 2^32 more — and the days come
    // back through `ftol` into a short (past i32, its 0x80000000).
    let factor = i64::from((100 - 3 * level - care) as i32 as u32);
    let whole = factor * base / 100;
    let whole = i32::try_from(whole).unwrap_or(i32::MIN);
    let days = i32::from((whole as i16).max(1));
    injure(records, id, kind, days, today);
    if kind == 0x1b || kind == 0x1e {
        let Some(p) = records.player(id) else { return };
        let age = statistics::age_on(p, date);
        let [first, second] = p.traits();
        let base = if second & 1 != 0 || first & 1 != 0 { 180 } else { 200 };
        let chance = base - 9 * (40 - age);
        if (random.below(200) as i32) < chance && p.knocks() & 8 == 0 {
            // Knocked a second time he would go to the club's news
            // (`FUN_006a6c20`, event 0x16), which is not posted here.
            if let Some(p) = records.player_mut(id) {
                p.set_knocks(p.knocks() | 8);
            }
        }
    }
}

/// The trainer's three likings moved, by the public, the fans and the
/// media (`FUN_0041828e` with `FUN_004184d4`), each a signed byte held to
/// 0 to 100: a human trainer's own (his record's `popularity`, the
/// manager object's +0x3a), else the computer trainer's — the club's
/// +0x7e, or its +0x80 (the team's `staff`), a person of `0xbe3920` up
/// to 1499 (`FUN_006317c6`, `FUN_006318f4`: his +0x4a to +0x4c).
fn move_likings(records: &mut Records, (country, number): (u16, u16), by: [i8; 3]) {
    let Some(team) = records.team(country, number) else { return };
    let trainer = team.humans()[0];
    let [first, second, _] = team.staff();
    let step = |value: u8, by: i8| (value as i8).wrapping_add(by).clamp(0, 100) as u8;
    if trainer != 0 {
        if let Some(m) = records.managers.get_mut(usize::from(trainer) - 1) {
            let mut likings = m.popularity();
            for (liking, by) in likings.iter_mut().zip(by) {
                *liking = step(*liking, by);
            }
            m.set_popularity(likings);
        }
        return;
    }
    let person = if first != 0 { first } else { second };
    if (1..=1499).contains(&person)
        && let Some(p) = records.persons.get_mut(usize::from(person))
    {
        for (at, by) in (0x4a..0x4d).zip(by) {
            let value = p.get::<u8>(at);
            p.put::<u8>(at, step(value, by));
        }
    }
}

/// The place `FUN_00767225`'s walk stops at for a roll over bands that
/// start at `starts`: it steps on while a band starts at or below the
/// roll, so it lands on the band after the one the roll falls in — but
/// never past the last.
fn band_after(starts: &[i32], roll: i32) -> usize {
    let mut at = 0;
    while at + 1 < starts.len() && starts[at] <= roll {
        at += 1;
    }
    at
}

/// A man's days out as the training's handle holds them (+0x2c, which
/// `FUN_00636638` reckons out of `injured_to`).
fn days_out(records: &Records, p: &Player) -> i32 {
    crate::load::days_to(&records.globals, p.injured_to())
}

/// His days out set anew, as the handle's write-back keeps them
/// (`FUN_006364cf`'s +0x2c): none clears the injury's end, its day and
/// the doctor's visits; else it lasts that many days from today.
fn set_days_out(records: &mut Records, id: u16, days: i32) {
    if days < 1 {
        if let Some(p) = records.player_mut(id) {
            p.set_visits(0);
            p.set_injured_to([0, 0]);
            p.set_injured_on(0);
        }
    } else {
        crate::medical::injure_for(records, id, days);
    }
}

/// An injury of `kind` from today, `days` long (`FUN_00766b92` for a
/// kind that is no knock): the kind (+0x228), the day (+0x1d8), the end.
fn injure(records: &mut Records, id: u16, kind: u8, days: i32, today: i32) {
    if let Some(p) = records.player_mut(id) {
        p.set_injury_kind(kind);
        p.set_injured_on(today as u16);
    }
    set_days_out(records, id, days);
}

/// Knocked about for `days` (`FUN_00766af9`, the handle's +0x2f, which
/// the record keeps as the day it lasts to, +0x1de).
fn knock_for(records: &mut Records, id: u16, days: u16, today: i32) {
    if let Some(p) = records.player_mut(id) {
        p.put::<u16>(0x1de, if days == 0 { 0 } else { (today as u16).wrapping_add(days) });
    }
}

/// The training units' gate of the player events (`DAT_008aa5c2`, the
/// unit row's byte +0x32, `FUN_00748f29`).
const UNIT_GATES: [u8; 39] = [
    1, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 1,
];

/// The ability bits event 105 draws over (`DAT_00890040`, text and bit):
/// a keeper's six first, then the others' ten.
const ABILITY_BITS: [u16; 16] = [0, 1, 2, 3, 4, 5, 0, 1, 2, 3, 5, 6, 7, 8, 9, 11];
/// The trait bits events 106 and 107 draw over in the first trait word
/// (`DAT_0088ffa4`: six words stand there, and a man who keeps no goal
/// draws ten, the last four reading the table after it, as the exe does)
/// and in the second (`DAT_0088ffe4`).
const FIRST_TRAITS: [u16; 10] = [1, 1, 2, 4, 8, 0, 0, 0, 0, 1];
const SECOND_TRAITS: [u16; 6] = [2, 4, 16, 32, 64, 512];

/// One of the listed men, drawn (`FUN_005eb193`): none from an empty
/// list, or when he is a playing manager (its +0x6a check is not here).
fn draw_any(list: &[u16], random: &mut Random) -> Option<u16> {
    if list.is_empty() {
        return None;
    }
    let id = list[random.below(list.len() as u32) as usize];
    (id <= crate::mood::LAST_REAL_ID).then_some(id)
}

/// Event 105 (`FUN_0060c1c0`'s case 5): a trained man with any first
/// ability bit (`good`, the handle's +0xc; of nine for a keeper, else
/// sixteen) has one of [`ABILITY_BITS`] drawn — a keeper's of the six,
/// the others' of the ten; with three bits of both words set, up to five
/// draws for one he has. One he has goes from both words
/// (`FUN_00747689`), one he has in neither moves into the second
/// (`bad`, +0xe; `FUN_00747770`). Both log the loss into the human
/// club's book of the week's abilities (`FUN_00643673`'s gate into
/// `Records::ability_lost`); the manager's plan of the man (the block at
/// `0xa50028`, behind the same gate) is not updated here.
fn ability_event(
    records: &mut Records,
    (country, number): (u16, u16),
    trainable: &[u16],
    random: &mut Random,
) -> bool {
    let Some(id) = draw_any(trainable, random) else { return false };
    let Some(p) = records.player(id) else { return false };
    let keeper = p.get::<u8>(0x21d) == 1;
    let (good, bad) = (p.good(), p.bad());
    let span = if keeper { 9 } else { 16 };
    let count = |word: u16| (0..span).filter(|&i| word >> i & 1 != 0).count();
    if count(good) == 0 {
        return false;
    }
    let mut draw = || if keeper { random.below(6) as usize } else { random.below(10) as usize + 6 };
    let at = if count(good) + count(bad) == 3 {
        let Some(at) = (0..5).map(|_| draw()).find(|&at| good >> ABILITY_BITS[at] & 1 != 0) else {
            return false;
        };
        at
    } else {
        draw()
    };
    let bit = 1u16 << ABILITY_BITS[at];
    let slot = records.team(country, number).map_or(0, |t| t.human());
    let Some(p) = records.player_mut(id) else { return false };
    let lost = if good & bit != 0 {
        p.set_good(good & !bit);
        p.set_bad(bad & !bit);
        true
    } else if bad & bit != 0 {
        return false;
    } else {
        p.set_bad(bad | bit);
        true
    };
    if lost {
        records.ability_lost(slot, id, ABILITY_BITS[at]);
    }
    lost
}

/// Events 106 and 107 (`FUN_0060c1c0`'s cases 6 and 7): a trained man,
/// and a roll over three for a keeper, else over the squad's size: 0
/// draws a bit of the first trait word (+0x10, the record's `traits[0]`;
/// of five for a keeper, else ten), 1 one of the second (+0x12, of
/// six), anything else is no event. 106 gives the bit to a man without
/// any of that word yet (`FUN_0074b4cf`'s sixteen, `FUN_0074b526`'s
/// eight), 107 takes it from a man who has it.
fn trait_event(records: &mut Records, (country, number): (u16, u16), trainable: &[u16], give: bool, random: &mut Random) -> bool {
    let Some(id) = draw_any(trainable, random) else { return false };
    let Some(p) = records.player(id) else { return false };
    let keeper = p.get::<u8>(0x21d) == 1;
    let squad = records.team(country, number).map_or(0, |t| t.player_count());
    let roll = if keeper { random.below(3) } else { random.below(u32::from(squad)) };
    let (word, bit, span) = match roll {
        0 => (0, FIRST_TRAITS[random.below(if keeper { 5 } else { 10 }) as usize], 16),
        1 => (1, SECOND_TRAITS[random.below(6) as usize], 8),
        _ => return false,
    };
    let Some(p) = records.player_mut(id) else { return false };
    let mut traits = p.traits();
    if give {
        if (0..span).any(|i| traits[word] >> i & 1 != 0) || traits[word] & bit != 0 {
            return false;
        }
        traits[word] |= bit;
    } else {
        if traits[word] & bit == 0 {
            return false;
        }
        traits[word] &= !bit;
    }
    p.set_traits(traits);
    true
}

/// Event 102 (`FUN_0060d119`): a player event out of [`PLAYER_EVENTS`],
/// drawn once more when it stands among the recent ones; met once one
/// is drawn that does not. Its run (`FUN_0060d1bf`) puts it among the
/// recent ones when it took place.
#[allow(clippy::too_many_arguments)]
fn player_event(
    records: &mut Records,
    club: (u16, u16),
    trainable: &[u16],
    tables: (&MoodTables, &LeagueTables),
    unit: usize,
    day_units: [u8; 3],
    day: (i32, NaiveDate),
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
        if run_player_event(records, club, trainable, tables, code, unit, day_units, day, random) {
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
/// friend (+0x204) trains too, both three more content; 23, a keeper
/// drawn and his +0x2f set to 5 (`FUN_00766af9`); and, in a unit whose
/// row allows it ([`UNIT_GATES`], `FUN_00748f29`), 24, two men who keep
/// no goal drawn and a roll of four — both +0x2f at 1, or the first
/// injured (kind 0x1c for five days, kind 0x32 for seven;
/// `FUN_00766b92`); 28, a keeper and a man drawn, and on a three-roll's
/// 2 the keeper's +0x2f at 5; 30, with a human trainer half the time a
/// man drawn, without one always, and the mood five up; 36, two men
/// drawn and the second's +0x2f at 1 unless he is out; 39 and 40, up to
/// three draws of a man over thirty (`FUN_005eb3b5`) for one with the
/// first ability bit 2 or 4, which he loses; 34, a man under 23 drawn
/// (`FUN_005eb2c0`) and one over 30, men standing away from the club
/// for none in either draw, the young one taking the reason 0x800 and
/// the two falling out (`FUN_00703df4`): a standing friendship undone,
/// each off the other's friends and onto the other's dislikes.
/// Ungated: 37, with a human
/// trainer and a co-trainer, three draws over the trained, each one more
/// content with the team; 42, with a human trainer in a unit the gate
/// shuts, the mood ten down; 43, the mood five up, answering "not met";
/// 13, any trained man drawn, at a morale of 106 or more two less
/// content with his distractions (`FUN_00701210`, the record's +0x294);
/// 15, with a human trainer, met unless the next match is tomorrow
/// (the club's +0xb44, the team's `next_day`); 32, unless the next
/// match is the league's, at its round 7 the team's +0xca (the club's
/// +0xa4) up by the trained men's count, a byte held to 200 (the
/// club's +0xc91 it asks first is never written, so always nought);
/// 33, with a human trainer and behind the gate, two men drawn who both
/// take the reason 0x800 (`FUN_00770800`); 9, a human trainer's club in
/// its country's top division (the club's +0x105, the team's +0x12e)
/// costing him ten of the media's liking (`FUN_0041828e` → `FUN_004184d4`
/// on the manager's +0x3a, his record's `popularity`), answering "not
/// met"; 38, a human trainer's hard unit (6, 0x10, 0x11) and up to three
/// draws for a man worn out — freshness 19 or less at the intensity 3
/// (the handle's +0xb5, the record's `intensity`): five less of the
/// media's liking and of the board's confidence (`FUN_0041849a`, the
/// manager's +0x9c, his record's `confidence`), the mood ten down, the
/// man knocked about a week; 29, behind the gate, a human trainer whose
/// board stands 75 or more behind him (`FUN_007701b2`,
/// [`crate::stock::confidence`]) lifting the mood ten; 35, behind the
/// gate, a team in a low mood (under 71) twenty matchdays in
/// ([`crate::league::matchdays`]) and more than four places short of its
/// season's goal (`FUN_00775fa0`): two men drawn take the reason 0x800,
/// the mood falls to nought, and a human manager (club +0x8b) loses ten
/// of his record's +0x11e (`FUN_006be8b8`, the manager object's +0x32);
/// 16, with a human trainer, a man drawn (`FUN_005eb1ff`: a five-roll's
/// 0 takes the trainer's man of the week's individual training) and a
/// three-roll — its 0 makes him one less content with his
/// distractions, the rest one more with the team, each with a roll of
/// three or five for its news's wording;
/// 41, with a human
/// trainer, every trained defender (main position 2 to 5,
/// `FUN_00677c5f`) one more content with the team (`FUN_005ee7ed`);
/// 26, with a human manager and one of the day's two first planned
/// units of the club's plan row letting it ([`UNIT_GATES`], the club's
/// +0xc0a): up to six draws over the trained defenders, the sixth
/// still of use, for one with the trait bit 4 (the record's +0x1c8),
/// 24 at most and one side place free, none of his three positions
/// standing at 10, who then takes position 10 on his free place
/// (`FUN_00745d3c`).
/// The other codes, drawing over the squad, the manager and the club
/// each in its own way, take nothing place here yet.
#[allow(clippy::too_many_arguments)]
fn run_player_event(
    records: &mut Records,
    (country, number): (u16, u16),
    trainable: &[u16],
    (tables, league): (&MoodTables, &LeagueTables),
    code: u32,
    unit: usize,
    day_units: [u8; 3],
    (today, date): (i32, NaiveDate),
    random: &mut Random,
) -> bool {
    let trainer = records.team(country, number).map_or(0, |t| t.humans()[0]);
    let mood = |records: &mut Records, by: i32| {
        if let Some(team) = records.team_mut(country, number) {
            add_mood(team, by);
        }
    };
    let gate = UNIT_GATES.get(unit).is_some_and(|&g| g != 0);
    let knock = |records: &mut Records, id: u16, days: u16| knock_for(records, id, days, today);
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
        9 => {
            let top = records.team(country, number).is_some_and(|t| t.get::<u8>(0x12e) == 0);
            if trainer != 0 && top {
                move_likings(records, (country, number), [0, 0, -10]);
            }
            false
        }
        13 => {
            let Some(id) = draw_any(trainable, random) else { return false };
            let Some(p) = records.player_mut(id) else { return false };
            if p.morale() < 0x6a {
                return false;
            }
            crate::mood::change(p, 5, -2);
            true
        }
        15 => trainer != 0 && records.team(country, number).is_some_and(|t| i32::from(t.next_day()) != today + 1),
        32 => {
            let Some(team) = records.team_mut(country, number) else { return false };
            if team.next_kind() == 2 || team.next_round() != 7 {
                return false;
            }
            let mut v = team.get::<u8>(0xca).wrapping_add(trainable.len() as u8);
            if 200 < v {
                v = 200;
            }
            team.put::<u8>(0xca, v);
            true
        }
        16 => {
            if trainer == 0 || trainable.is_empty() {
                return false;
            }
            // `FUN_005eb1ff`: on a five-roll's 0 the man the trainer has
            // in individual training this week (the manager object's
            // +0x726 + week × 0x3a, his record's 0x872 on, the week the
            // turn's day over seven), if he is in the squad; else one of
            // the trained drawn.
            let week = usize::try_from(records.globals.week_start() / 7).unwrap_or(0);
            let squad = records.team(country, number).map_or_else(Vec::new, |t| t.players());
            let mut picked = None;
            if random.below(5) == 0 {
                let weekly = records
                    .managers
                    .get(usize::from(trainer) - 1)
                    .map_or(0, |m| m.get::<i16>(0x872 + week * 0x3a));
                picked = u16::try_from(weekly).ok().filter(|id| *id != 0 && squad.contains(id));
            }
            let id = picked.unwrap_or_else(|| trainable[random.below(trainable.len() as u32) as usize]);
            if crate::mood::LAST_REAL_ID < id {
                return false;
            }
            let Some(p) = records.player_mut(id) else { return false };
            if random.below(3) == 0 {
                if 1 < statistics::form_today(p) {
                    crate::mood::change(p, 5, -1);
                    random.below(3);
                }
            } else {
                crate::mood::change(p, 1, 1);
                random.below(5);
            }
            true
        }
        23 => {
            let Some(keeper) = draw_listed(records, trainable, true, random) else { return false };
            knock(records, keeper, 5);
            true
        }
        24 if gate => {
            let Some(one) = draw_listed(records, trainable, false, random) else { return false };
            let Some(other) = draw_listed(records, trainable, false, random) else { return false };
            if one == other {
                return false;
            }
            match random.below(4) {
                0 => {}
                1 => {
                    knock(records, one, 1);
                    knock(records, other, 1);
                }
                2 => injure(records, one, 0x1c, 5, today),
                _ => injure(records, one, 0x32, 7, today),
            }
            true
        }
        26 => {
            // The human manager first (club +0x8a), then the day's two
            // first planned units (the club's +0xc0a row): one of them
            // must let the event.
            if trainer == 0
                || !day_units
                    .iter()
                    .take(2)
                    .any(|&u| UNIT_GATES.get(usize::from(u)).is_some_and(|&g| g != 0))
            {
                return false;
            }
            // Up to six draws over the trained defenders, the sixth
            // still of use: a man with the trait bit 4 (the record's
            // +0x1c8), 24 at most, and not already standing at two
            // side positions.  A draw of none — no trained defender,
            // the playing manager, a man standing away — counts as the
            // failed draw that lets the loop run on; the original
            // reads its null player here and crashes.
            let mut man = None;
            for _ in 0..6 {
                if let Some(id) = draw_defender(records, trainable, random)
                    && records.player(id).is_some_and(|p| {
                        p.traits()[0] & 4 != 0
                            && statistics::age_on(p, date) <= 0x18
                            && (p.positions()[1] == 0 || p.positions()[2] == 0)
                    })
                {
                    man = Some(id);
                    break;
                }
            }
            let Some(man) = man else { return false };
            let Some(p) = records.player(man) else { return false };
            let mut places = p.positions();
            if places.contains(&10) {
                return false;
            }
            // `FUN_00745d3c(man, 10)`: the side position lands on the
            // free place, the first before the second; the weighing of
            // two standing places is never reached, the drawn man
            // holds at most one.  The news (word 0x28) is not posted.
            if places[1] == 0 {
                places[1] = 10;
            } else {
                places[2] = 10;
            }
            if let Some(p) = records.player_mut(man) {
                p.set_positions(places);
            }
            true
        }
        28 if gate => {
            let Some(keeper) = draw_listed(records, trainable, true, random) else { return false };
            let Some(other) = draw_listed(records, trainable, false, random) else { return false };
            if keeper == other {
                return false;
            }
            if 2 <= random.below(3) {
                knock(records, keeper, 5);
            }
            true
        }
        29 if gate => {
            if trainer == 0 || crate::stock::confidence(records, usize::from(trainer) - 1) < 0x4b {
                return false;
            }
            mood(records, 10);
            true
        }
        30 if gate => {
            if (trainer == 0 || random.below(2) != 0) && draw_listed(records, trainable, false, random).is_none() {
                return false;
            }
            mood(records, 5);
            true
        }
        33 if gate && trainer != 0 => {
            // Both are drawn before either is asked after.
            let one = draw_listed(records, trainable, false, random);
            let other = draw_listed(records, trainable, false, random);
            let (Some(one), Some(other)) = (one, other) else { return false };
            if one == other {
                return false;
            }
            for id in [one, other] {
                if let Some(p) = records.player_mut(id) {
                    tables.set_reasons(p, 0x800);
                }
            }
            true
        }
        34 if gate => {
            let Some(young) = draw_aged(records, trainable, date, 0x17, true, random) else { return false };
            let Some(elder) = draw_aged(records, trainable, date, 0x1e, false, random) else { return false };
            if let Some(p) = records.player_mut(young) {
                tables.set_reasons(p, 0x800);
            }
            fall_out(records, young, elder);
            true
        }
        35 if gate => {
            let Some(team) = records.team(country, number) else { return false };
            if 0x47 <= team.mood() || crate::league::matchdays(records, team) < 0x14 {
                return false;
            }
            let reached = crate::league::goal_reached(records, league, team);
            let margin = if reached < 1 { -crate::league::goal_missing(records, league, team) } else { reached };
            if -4 <= margin {
                return false;
            }
            let one = draw_listed(records, trainable, false, random);
            let other = draw_listed(records, trainable, false, random);
            let (Some(one), Some(other)) = (one, other) else { return false };
            if one == other {
                return false;
            }
            for id in [one, other] {
                if let Some(p) = records.player_mut(id) {
                    tables.set_reasons(p, 0x800);
                }
            }
            if let Some(team) = records.team_mut(country, number) {
                team.set_mood(0);
            }
            let manager = records.team(country, number).map_or(0, |t| t.humans()[1]);
            if manager != 0
                && let Some(m) = records.managers.get_mut(usize::from(manager) - 1)
            {
                let standing = m.get::<i16>(0x11e);
                m.put::<i16>(0x11e, (standing - 10).clamp(0, 100));
            }
            true
        }
        36 if gate => {
            let one = draw_listed(records, trainable, false, random);
            let other = draw_listed(records, trainable, false, random);
            let (Some(one), Some(other)) = (one, other) else { return false };
            if one == other {
                return false;
            }
            if records.player(other).is_some_and(|p| days_out(records, p) == 0) {
                knock(records, other, 1);
            }
            true
        }
        37 => {
            let co = records.team(country, number).map_or(0, |t| t.co_trainer());
            if trainer == 0 || co == 0 || trainable.is_empty() {
                return false;
            }
            for _ in 0..3 {
                let id = trainable[random.below(trainable.len() as u32) as usize];
                if let Some(p) = records.player_mut(id) {
                    crate::mood::change(p, 1, 1);
                }
            }
            true
        }
        38 => {
            if trainer == 0 || !matches!(unit, 6 | 0x10 | 0x11) {
                return false;
            }
            let worn = |records: &Records, id: Option<u16>| {
                id.and_then(|id| records.player(id)).is_some_and(|p| {
                    crate::computer_training::fresh_percent(p) <= 0x13 && p.intensity() == 3
                })
            };
            let Some(id) = (0..3).map(|_| draw_any(trainable, random)).find(|&id| worn(records, id)).flatten() else {
                return false;
            };
            if let Some(m) = records.managers.get_mut(usize::from(trainer) - 1) {
                let mut popularity = m.popularity();
                popularity[2] = (popularity[2] as i8).wrapping_sub(5).clamp(0, 100) as u8;
                m.set_popularity(popularity);
                m.set_confidence((m.confidence() - 5).clamp(0, 100));
            }
            mood(records, -10);
            knock(records, id, 7);
            true
        }
        39 | 40 if gate => {
            let bit = if code == 39 { 2 } else { 4 };
            let elders: Vec<u16> = trainable
                .iter()
                .copied()
                .filter(|&id| records.player(id).is_some_and(|p| p.get::<u8>(0x21d) != 1 && 30 < statistics::age_on(p, date)))
                .collect();
            let Some(id) = (0..3)
                .map(|_| draw_listed(records, &elders, false, random))
                .find(|id| id.is_some_and(|id| records.player(id).is_some_and(|p| p.good() & bit != 0)))
                .flatten()
            else {
                return false;
            };
            if let Some(p) = records.player_mut(id) {
                p.set_good(p.good() & !bit);
            }
            true
        }
        41 => {
            if trainer == 0 {
                return false;
            }
            for &id in trainable {
                if let Some(p) = records.player_mut(id)
                    && (2..=5).contains(&p.get::<u8>(0x21d))
                {
                    crate::mood::change(p, 1, 1);
                }
            }
            true
        }
        42 => {
            if trainer == 0 || gate {
                return false;
            }
            mood(records, -10);
            true
        }
        43 => {
            mood(records, 5);
            false
        }
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

/// One of the trained who keep no goal and are of an age, drawn
/// (`FUN_005eb2c0` under 23, `FUN_005eb3b5` over 30, both with the
/// event's flag set): the list's men at the position byte's 1 not
/// standing, of the years the date gives, over the bound's other side;
/// one roll over them.  None when the man drawn is a playing manager,
/// or stands away from the club — the handle's +0x6a, which the load
/// copy builds from the record's `away_to`.  Their forty stack slots
/// never fill before a squad's men have all been walked.
fn draw_aged(records: &Records, list: &[u16], date: NaiveDate, bound: i32, under: bool, random: &mut Random) -> Option<u16> {
    let such: Vec<u16> = list
        .iter()
        .copied()
        .filter(|&id| {
            records.player(id).is_some_and(|p| {
                let age = statistics::age_on(p, date);
                p.get::<u8>(0x21d) != 1 && (if under { age < bound } else { bound < age })
            })
        })
        .collect();
    if such.is_empty() {
        return None;
    }
    let id = such[random.below(such.len() as u32) as usize];
    let away = records.player(id).is_some_and(|p| p.away_to()[0] != 0);
    (id <= crate::mood::LAST_REAL_ID && !away).then_some(id)
}

/// A trained defender drawn (`FUN_005eb0ab`): the listed men whose main
/// position is one of the defence's 2 to 5 (`FUN_00677c5f`), one roll
/// over those, the playing manager and a man standing away from the
/// club for none.
fn draw_defender(records: &Records, list: &[u16], random: &mut Random) -> Option<u16> {
    let such: Vec<u16> = list
        .iter()
        .copied()
        .filter(|&id| records.player(id).is_some_and(|p| (2..=5).contains(&p.get::<u8>(0x21d))))
        .collect();
    if such.is_empty() {
        return None;
    }
    let id = such[random.below(such.len() as u32) as usize];
    let away = records.player(id).is_some_and(|p| p.away_to()[0] != 0);
    (id <= crate::mood::LAST_REAL_ID && !away).then_some(id)
}

/// The young man and the elder fall out (`FUN_00703df4`, the event's
/// word 0x35): unless the young one already names the elder among the
/// men he hates, a standing best friendship between them is undone,
/// each leaves the other off his friends, and each joins the list of
/// those the other dislikes.  The original's slips stand: taking a man
/// off a dislike list zeroes the third friend in his place
/// (`FUN_00703930`, as in [`crate::talk`]'s reconciling), and when one
/// man's three dislikes all stand, the first of them leaves and the
/// new one is written past the list, onto the best friend's own place.
fn fall_out(records: &mut Records, young: u16, elder: u16) {
    if records.player(young).is_some_and(|p| p.hates() == elder) {
        return;
    }
    if records.player(young).is_some_and(|p| p.best_friend() == elder) {
        if let Some(p) = records.player_mut(young) {
            p.set_best_friend(0);
        }
        // `FUN_00703a0c`: the other side's best friend goes with it,
        // whoever that stands at, and that man's in turn, until one of
        // them names none.
        let mut at = elder;
        loop {
            let next = match records.player_mut(at) {
                Some(p) if p.best_friend() != 0 => {
                    let next = p.best_friend();
                    p.set_best_friend(0);
                    next
                }
                _ => break,
            };
            at = next;
        }
    }
    // `FUN_00703854`: each leaves the other off his friends.
    for (who, other) in [(young, elder), (elder, young)] {
        if let Some(r) = records.players.get_mut(usize::from(who)) {
            let mut friends = r.friends();
            if let Some(at) = friends.iter().take_while(|&&f| f != 0).position(|&f| f == other) {
                for i in at..2 {
                    friends[i] = friends[i + 1];
                }
                friends[2] = 0;
                r.set_friends(friends);
            }
        }
    }
    // `FUN_007033a5` twice: each joins the other's dislikes.
    for (who, other) in [(young, elder), (elder, young)] {
        let standing = match records.player(who) {
            Some(r) => r.dislikes(),
            None => continue,
        };
        if standing.iter().take_while(|&&d| d != 0).any(|&d| d == other) {
            continue;
        }
        let Some(at) = standing.iter().position(|&d| d == 0) else {
            leave_dislike_lists(records, who, standing[0]);
            if let Some(r) = records.player_mut(who) {
                r.set_best_friend(other);
            }
            continue;
        };
        let mut dislikes = standing;
        dislikes[at] = other;
        if let Some(r) = records.player_mut(who) {
            r.set_dislikes(dislikes);
        }
    }
}

/// Taking the one man off the other's list of dislikes, and him off
/// his (`FUN_00703930` both ways): each list shifts up from his place,
/// and its third friend is zeroed instead of the third dislike.
fn leave_dislike_lists(records: &mut Records, one: u16, other: u16) {
    for (who, other) in [(one, other), (other, one)] {
        let Some(r) = records.players.get_mut(usize::from(who)) else { continue };
        let mut dislikes = r.dislikes();
        let Some(at) = dislikes.iter().take_while(|&&d| d != 0).position(|&d| d == other) else {
            continue;
        };
        for i in at..2 {
            dislikes[i] = dislikes[i + 1];
        }
        r.set_dislikes(dislikes);
        let mut friends = r.friends();
        friends[2] = 0;
        r.set_friends(friends);
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

/// A team skill a step up, held within 0 to 5 (`FUN_00698a4b` and its
/// kin): a 7, "unknown", comes down to 5.
fn skill_step(value: u8) -> u8 {
    (i16::from(value) + 1).clamp(0, 5) as u8
}

/// A field of three bits of the set pieces' word a step up (`FUN_00698616`
/// and its kin).
fn step_set_piece(team: &mut Team, shift: u16) {
    let bits = team.set_pieces();
    let value = skill_step(((bits >> shift) & 7) as u8);
    team.set_set_pieces(bits & !(7 << shift) | u16::from(value) << shift);
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

    /// The injury table's walk lands one band late, as the exe's loop
    /// does: a roll in the first band names the second kind, and every
    /// roll from the last band's start on stays on the last.
    #[test]
    fn the_injury_walk_lands_one_band_late() {
        let starts = [0, 2, 5, 9];
        let picked: Vec<usize> = (0..12).map(|roll| band_after(&starts, roll)).collect();
        assert_eq!(picked, [1, 1, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3]);
    }

    /// A unit's skill step: one up within 0 to 5, "unknown" (7) down to 5,
    /// each field of the set pieces' word apart from the others.
    #[test]
    fn the_team_skills_step_within_five() {
        assert_eq!((skill_step(0), skill_step(4), skill_step(5), skill_step(7)), (1, 5, 5, 5));
        let zeros = vec![0; Team::SIZE];
        let mut team = Team::new(&zeros).unwrap();
        team.set_set_pieces(4 << 12 | 5 << 6 | 2 << 3 | 7);
        for shift in [12, 6, 3, 0] {
            step_set_piece(&mut team, shift);
        }
        assert_eq!(team.set_pieces(), 5 << 12 | 5 << 6 | 3 << 3 | 5);
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
            squad.iter().copied().filter(|&id| records.player(id).is_some_and(|p| days_out(&records, p) < 7)).collect();
        let conditions = |records: &Records| -> Vec<i16> {
            trainable.iter().map(|&id| records.player(id).unwrap().condition()).collect()
        };
        let start = Random::from_state(12345, 67890);
        let mut random = start.clone();
        let mut recent = Vec::new();
        let date = NaiveDate::from_ymd_opt(2025, 9, 9).unwrap();
        let tables = MoodTables::load(&a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap()).unwrap();
        let league_tables = LeagueTables::load(&a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap()).unwrap();
        for (event, by) in [(100, 300), (101, 500), (103, 0)] {
            let before = conditions(&records);
            assert!(training_event(&mut records, club, &squad, &trainable, (&tables, &league_tables), event, 0, [0, 0, 0], (64, date), &mut recent, &mut random));
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
        assert!(training_event(&mut records, club, &squad, &trainable, (&tables, &league_tables), 108, 0, [0, 0, 0], (64, date), &mut recent, &mut random));
        assert_eq!(random, expected);
        // The player events' plainest: the team's mood ten down and up
        // again, and code 12's one draw over the outfield, one man two
        // more content with the team.
        let mood = i32::from(records.team(club.0, club.1).unwrap().mood());
        assert!(run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 4, 0, [0, 0, 0], (64, date), &mut random));
        let down = (mood - 10).max(0);
        assert_eq!(i32::from(records.team(club.0, club.1).unwrap().mood()), down);
        assert!(run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 5, 0, [0, 0, 0], (64, date), &mut random));
        assert_eq!(i32::from(records.team(club.0, club.1).unwrap().mood()), (down + 10).min(200));
        assert_eq!(random, expected, "the mood events draw nothing");
        let outfield = trainable.iter().filter(|&&id| records.player(id).unwrap().get::<u8>(0x21d) != 1).count();
        let team_parts = |records: &Records| -> i32 {
            trainable.iter().map(|&id| i32::from(records.player(id).unwrap().contentment()[1])).sum()
        };
        let before = team_parts(&records);
        assert!(run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 12, 0, [0, 0, 0], (64, date), &mut random));
        expected.below(outfield as u32);
        assert_eq!(random, expected);
        assert!((0..=2).contains(&(team_parts(&records) - before)));
        // Code 24 in a unit whose row shuts it (unit 1) is no event and
        // draws nothing; code 23 draws a keeper and sets his +0x2f to 5.
        let held = random.clone();
        assert!(!run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 24, 1, [0, 0, 0], (64, date), &mut random));
        assert_eq!(random, held);
        let keepers: Vec<u16> =
            trainable.iter().copied().filter(|&id| records.player(id).unwrap().get::<u8>(0x21d) == 1).collect();
        if !keepers.is_empty() {
            assert!(run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 23, 0, [0, 0, 0], (64, date), &mut random));
            assert!(keepers.iter().any(|&id| records.player(id).unwrap().get::<u16>(0x1de) == 64 + 5));
        }
        // Code 9 draws nothing, answers "not met", and costs a human
        // trainer in the top division ten of the media's liking.
        let held = random.clone();
        let team = records.team(club.0, club.1).unwrap();
        let (trainer, top) = (team.humans()[0], team.get::<u8>(0x12e) == 0);
        let media = |records: &Records| records.managers[usize::from(trainer).max(1) - 1].popularity()[2];
        let before = media(&records);
        assert!(!run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 9, 0, [0, 0, 0], (64, date), &mut random));
        let lost = if trainer != 0 && top { before.min(10) } else { 0 };
        assert_eq!(media(&records), before - lost);
        assert_eq!(random, held);
        // Code 29 in an open unit (0): met as the board's confidence in a
        // human trainer stands at 75 or more, the mood then ten up.
        let backed = trainer != 0 && crate::stock::confidence(&records, usize::from(trainer).max(1) - 1) >= 0x4b;
        let mood_before = i32::from(records.team(club.0, club.1).unwrap().mood());
        assert_eq!(run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 29, 0, [0, 0, 0], (64, date), &mut random), backed);
        let lifted = if backed { (mood_before + 10).min(200) } else { mood_before };
        assert_eq!(i32::from(records.team(club.0, club.1).unwrap().mood()), lifted);
        // Code 41 lifts every trained defender's team contentment by one
        // with a human trainer, and draws nothing.
        let defenders = |records: &Records| -> i32 {
            trainable
                .iter()
                .map(|&id| records.player(id).unwrap())
                .filter(|p| (2..=5).contains(&p.get::<u8>(0x21d)))
                .map(|p| i32::from(p.contentment()[1]))
                .sum()
        };
        let (held, before) = (random.clone(), defenders(&records));
        assert_eq!(run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 41, 0, [0, 0, 0], (64, date), &mut random), trainer != 0);
        assert!(trainer == 0 || defenders(&records) >= before);
        assert_eq!(random, held);
        // Event 110 puts a man out whose knock's count passed five: the
        // Achilles tendon for 182 days (kind 0x41), the groin for 100
        // (0x42), into the record's own fields, today as the day.
        let (achilles, groin) = (squad[0], squad[1]);
        for (id, bit) in [(achilles, crate::medical::ACHILLES), (groin, crate::medical::GROIN)] {
            let p = records.player_mut(id).unwrap();
            p.set_knocks(p.knocks() & !(crate::medical::ACHILLES | crate::medical::GROIN) | bit);
            p.put::<u8>(0x27c, 6);
            p.set_injured_to([0, 0]);
        }
        let held = random.clone();
        assert!(training_event(&mut records, club, &squad, &trainable, (&tables, &league_tables), 110, 0, [0, 0, 0], (64, date), &mut recent, &mut random));
        assert_eq!(random, held, "the long knocks draw nothing");
        for (id, kind, days) in [(achilles, 0x41, 182), (groin, 0x42, 100)] {
            let p = records.player(id).unwrap();
            assert_eq!((p.injury_kind(), p.injured_on(), days_out(&records, p)), (kind, 64, days));
        }
        // Code 35 waits for twenty matchdays: on A, a few in, it neither
        // draws nor takes place.
        let team = records.team(club.0, club.1).unwrap();
        assert!(crate::league::matchdays(&records, team) < 0x14);
        let held = random.clone();
        assert!(!run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 35, 0, [0, 0, 0], (64, date), &mut random));
        assert_eq!(random, held);
        // Code 38 outside its hard units neither draws nor takes place.
        let held = random.clone();
        assert!(!run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 38, 0, [0, 0, 0], (64, date), &mut random));
        assert_eq!(random, held);
        // Code 34, behind the gate: the young draw (`FUN_005eb2c0`,
        // under 23) and the elder's (`FUN_005eb3b5`, over 30) each roll
        // once over their trained outfield men; the young one's reason
        // 0x800 and the two falling out.  A drawn man standing away
        // from the club counts for none, which ends the event already
        // at the young draw — the elder's roll never runs.
        let at_age = |records: &Records, id: u16, under: bool| {
            records.player(id).is_some_and(|p| {
                let age = statistics::age_on(p, date);
                p.get::<u8>(0x21d) != 1 && (if under { age < 23 } else { 30 < age })
            })
        };
        let youngs: Vec<u16> = trainable.iter().copied().filter(|&id| at_age(&records, id, true)).collect();
        let elders: Vec<u16> = trainable.iter().copied().filter(|&id| at_age(&records, id, false)).collect();
        assert!(!youngs.is_empty() && !elders.is_empty(), "A fields both ages of the draw");
        assert!(trainable.iter().any(|&id| id > crate::mood::LAST_REAL_ID), "A lists its playing manager among the trained");
        let mut probe = random.clone();
        let away = youngs[probe.below(youngs.len() as u32) as usize];
        records.player_mut(away).unwrap().set_away_to([1, 0]);
        let mut expected = random.clone();
        expected.below(youngs.len() as u32);
        let held = records.player(away).unwrap().dislikes();
        assert!(!run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 34, 0, [0, 0, 0], (64, date), &mut random));
        assert_eq!(random, expected, "the young draw's roll, no other");
        assert_eq!(records.player(away).unwrap().dislikes(), held);
        // A drawn playing manager is none's company as well: with the
        // manager the one man of his age in the list, the roll names
        // him at once.
        records.player_mut(away).unwrap().set_away_to([0, 0]);
        let manager = trainable.iter().copied().find(|&id| id > crate::mood::LAST_REAL_ID).unwrap();
        let one = [manager];
        let mut expected = random.clone();
        expected.below(1);
        assert!(!run_player_event(&mut records, club, &one, (&tables, &league_tables), 34, 0, [0, 0, 0], (64, date), &mut random));
        assert_eq!(random, expected);
        // Back at the club — every pool man cleared, the save has its
        // own loaned men — the pair are seeded as friends and best
        // friends; the young one's three dislikes all stand, and the
        // first of them knows no dislikes.  The rolls come from the
        // state the two standstill runs left, so the pair is drawn
        // anew, over the real men only.
        for &id in youngs.iter().chain(elders.iter()) {
            records.player_mut(id).unwrap().set_away_to([0, 0]);
        }
        let real: Vec<u16> = trainable.iter().copied().filter(|&id| id <= crate::mood::LAST_REAL_ID).collect();
        let youngs: Vec<u16> = real.iter().copied().filter(|&id| at_age(&records, id, true)).collect();
        let elders: Vec<u16> = real.iter().copied().filter(|&id| at_age(&records, id, false)).collect();
        let mut probe = random.clone();
        let young = youngs[probe.below(youngs.len() as u32) as usize];
        let elder = elders[probe.below(elders.len() as u32) as usize];
        let others: Vec<u16> = squad
            .iter()
            .copied()
            .filter(|&id| id != young && id != elder && id <= crate::mood::LAST_REAL_ID)
            .take(3)
            .collect();
        for id in [others[0], others[1], others[2]] {
            records.player_mut(id).unwrap().set_dislikes([0; 3]);
        }
        {
            let p = records.player_mut(young).unwrap();
            p.set_hates(0);
            p.set_best_friend(elder);
            p.set_friends([others[0], elder, others[1]]);
            p.set_dislikes([others[0], others[1], others[2]]);
        }
        {
            let p = records.player_mut(elder).unwrap();
            p.set_best_friend(young);
            p.set_friends([young, 0, 0]);
            p.set_dislikes([0; 3]);
        }
        let other_friends = records.player(others[0]).unwrap().friends();
        let elder_flags = records.player(elder).unwrap().flags();
        let mut expected = random.clone();
        expected.below(youngs.len() as u32);
        expected.below(elders.len() as u32);
        assert!(run_player_event(&mut records, club, &real, (&tables, &league_tables), 34, 0, [0, 0, 0], (64, date), &mut random));
        assert_eq!(random, expected, "one roll over each draw's men");
        assert_ne!(records.player(young).unwrap().flags() & 0x800, 0, "the young man's reason");
        assert_eq!(records.player(elder).unwrap().flags(), elder_flags);
        // The young one's full dislike list lost its first man, and the
        // elder was written past its end onto the best friend's place —
        // the third friend zeroed in the first dislike's place, the
        // original's slip — with the friendship undone and each gone
        // from the other's friends.
        let y = records.player(young).unwrap();
        assert_eq!((y.dislikes(), y.friends(), y.best_friend()), ([others[1], others[2], others[2]], [others[0], others[1], 0], elder));
        let e = records.player(elder).unwrap();
        assert_eq!((e.dislikes(), e.friends(), e.best_friend()), ([young, 0, 0], [0; 3], 0));
        assert_eq!(records.player(others[0]).unwrap().friends(), other_friends, "no slip where the man was not named");
        // With the hate already standing, the falling out does nothing.
        records.player_mut(young).unwrap().set_hates(elder);
        let before = (records.player(young).unwrap().dislikes(), records.player(elder).unwrap().dislikes(), records.player(young).unwrap().best_friend());
        let mut expected = random.clone();
        expected.below(youngs.len() as u32);
        expected.below(elders.len() as u32);
        assert!(run_player_event(&mut records, club, &real, (&tables, &league_tables), 34, 0, [0, 0, 0], (64, date), &mut random));
        assert_eq!(random, expected);
        let after = (records.player(young).unwrap().dislikes(), records.player(elder).unwrap().dislikes(), records.player(young).unwrap().best_friend());
        assert_eq!(before, after);
        // The secret training's toll: a human trainer one less with the
        // fans, the others as they were.
        if trainer != 0 {
            let before = records.managers[usize::from(trainer) - 1].popularity();
            move_likings(&mut records, club, [0, -1, 0]);
            let after = records.managers[usize::from(trainer) - 1].popularity();
            assert_eq!(after, [before[0], before[1].saturating_sub(1), before[2]]);
        }
        // Code 15 draws nothing and is met with a human trainer unless
        // the next match is tomorrow.
        let held = random.clone();
        let team = records.team(club.0, club.1).unwrap();
        let expected_15 = team.humans()[0] != 0 && team.next_day() != 65;
        assert_eq!(run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 15, 0, [0, 0, 0], (64, date), &mut random), expected_15);
        assert_eq!(random, held);
        // Code 43 lifts the mood five and answers "not met"; 37 draws
        // three times with a human trainer and a co-trainer, else none.
        let mood = i32::from(records.team(club.0, club.1).unwrap().mood());
        assert!(!run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 43, 0, [0, 0, 0], (64, date), &mut random));
        assert_eq!(i32::from(records.team(club.0, club.1).unwrap().mood()), (mood + 5).min(200));
        let team = records.team(club.0, club.1).unwrap();
        let staffed = team.humans()[0] != 0 && team.co_trainer() != 0;
        let mut thrice = random.clone();
        if staffed {
            for _ in 0..3 {
                thrice.below(trainable.len() as u32);
            }
        }
        assert_eq!(run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 37, 0, [0, 0, 0], (64, date), &mut random), staffed);
        assert_eq!(random, thrice);
        // The session's injury over many rolls: the man is out from today
        // a day at least, of a kind the table puts out, or only his
        // knocks grow.
        let medical = MedicalTables::load(&a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap()).unwrap();
        for k in 0..100 {
            let id = trainable[k % trainable.len()];
            let before = records.player(id).unwrap().knocks();
            training_injury(&mut records, club, id, 64, date, &medical, &mut random);
            let p = records.player(id).unwrap();
            if p.knocks() != before && medical.knock_bits(usize::from(p.injury_kind())) == 0 && p.injured_on() != 64 {
                assert_eq!(p.knocks() & !before, p.knocks() ^ before, "knocks only grow");
                continue;
            }
            if p.injured_on() == 64 {
                assert!(days_out(&records, p) >= 1, "out a day at least");
                assert_eq!(medical.knock_bits(usize::from(p.injury_kind())), 0);
            }
        }
        // The ability and trait events over many rolls: 106 only gives a
        // trait bit, 107 only takes one, 105 moves at most one ability
        // bit, each on one man.
        let words = |records: &Records| -> Vec<(u16, u16, [u16; 2])> {
            trainable
                .iter()
                .map(|&id| {
                    let p = records.player(id).unwrap();
                    (p.good(), p.bad(), p.traits())
                })
                .collect()
        };
        let words0 = words(&records);
        for _ in 0..200 {
            for event in [105, 106, 107] {
                let before = words(&records);
                training_event(&mut records, club, &squad, &trainable, (&tables, &league_tables), event, 0, [0, 0, 0], (64, date), &mut recent, &mut random);
                let after = words(&records);
                let changed: Vec<_> = before.iter().zip(&after).filter(|(b, a)| b != a).collect();
                assert!(changed.len() <= 1, "event {event} changed {} men", changed.len());
                for ((good, bad, traits), (good2, bad2, traits2)) in changed {
                    match event {
                        105 => {
                            let moved = (good ^ good2) | (bad ^ bad2);
                            assert_eq!(moved.count_ones(), 1, "105 moves one ability bit");
                            assert_eq!(traits, traits2);
                        }
                        106 => {
                            assert!((traits2[0] & traits[0]) == traits[0] && (traits2[1] & traits[1]) == traits[1]);
                            assert_eq!((good, bad), (good2, bad2));
                        }
                        _ => {
                            assert!((traits[0] & traits2[0]) == traits2[0] && (traits[1] & traits2[1]) == traits2[1]);
                            assert_eq!((good, bad), (good2, bad2));
                        }
                    }
                }
            }
        }
        // And what 105 moved it booked: the human club's book of the week
        // holds one entry per man, its lost bits exactly the ability bits
        // the rolls moved off him and its gained side empty (the loss log
        // `FUN_00747689` and `FUN_00747770` into `Records::ability_lost`).
        let slot = records.team(club.0, club.1).unwrap().human();
        let book = records.abilities_week(slot);
        assert!(!book.is_empty(), "two hundred rolls of 105 moved no ability bit");
        for (&id, (good, bad, _)) in trainable.iter().zip(&words0) {
            let p = records.player(id).unwrap();
            let moved = (good ^ p.good()) | (bad ^ p.bad());
            let entry = book.iter().find(|e| e.id == id);
            if moved == 0 {
                assert!(entry.is_none(), "a man who moved nothing holds a book entry");
            } else {
                let e = entry.unwrap_or_else(|| panic!("no book entry for {id}'s moved {moved:#06x}"));
                assert_eq!((e.gained, e.lost), (0, moved));
            }
        }
        assert!(book.iter().all(|e| trainable.contains(&e.id)), "the book holds a man the rolls could not move");
    }

    /// Code 26 on A: a human manager and one of the day's two first
    /// planned units let the event; the draw walks the trained
    /// defenders (`FUN_005eb0ab`), up to six draws over them with the
    /// sixth still of use and a draw of none letting the loop run on;
    /// the fit man — trait bit 4, 24 at most, one side place free, none
    /// of his places standing at 10 — takes place 10 on his free side
    /// place, the first before the second (`FUN_00745d3c`).
    #[test]
    #[ignore = "reads the oracle's kept saves beside $ANSTOSS3_DIR"]
    fn the_side_position_event_draws_a_young_defender() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let save = a3_files::an3::Save::read(&dir.parent().unwrap().join("saves-kept/A.an3")).unwrap();
        let mut records = Records::from_save(save).unwrap();
        let (country, number) = statistics::human_clubs(&records)[0];
        let club = (u16::from(country), number as u16);
        let squad = records.team(club.0, club.1).unwrap().players();
        let date = NaiveDate::from_ymd_opt(2025, 9, 9).unwrap();
        let exe = a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap();
        let (tables, league_tables) = (MoodTables::load(&exe).unwrap(), LeagueTables::load(&exe).unwrap());
        let mut random = Random::from_state(24681, 13579);
        assert_ne!(records.team(club.0, club.1).unwrap().humans()[0], 0, "A's club stands under its human");
        // The gate: the day's two first planned units both free (the
        // plan's 11) means no event and no roll.
        let held = random.clone();
        assert!(!run_player_event(&mut records, club, &squad, (&tables, &league_tables), 26, 0, [11, 11, 11], (64, date), &mut random));
        assert_eq!(random, held, "two free planned units shut the event");
        // A unit letting it, but no trained defender in the list: the
        // pool stands empty and the six draws roll nothing at all.
        let none: [u16; 0] = [];
        let held = random.clone();
        assert!(!run_player_event(&mut records, club, &none, (&tables, &league_tables), 26, 0, [0, 11, 11], (64, date), &mut random));
        assert_eq!(random, held, "an empty pool draws without a roll");
        // The draw's men: the trained real defenders, the playing
        // manager out of the list.
        let defs: Vec<u16> = squad
            .iter()
            .copied()
            .filter(|&id| {
                id <= crate::mood::LAST_REAL_ID
                    && days_out(&records, records.player(id).unwrap()) < 7
                    && (2..=5).contains(&records.player(id).unwrap().positions()[0])
            })
            .collect();
        assert!(!defs.is_empty(), "A trains defenders");
        let state: Vec<_> = defs
            .iter()
            .map(|&id| {
                let p = records.player(id).unwrap();
                (p.traits(), p.positions(), p.away_to(), p.birth_year())
            })
            .collect();
        let restore = |records: &mut Records| {
            for (&id, (traits, places, away, year)) in defs.iter().zip(&state) {
                let p = records.player_mut(id).unwrap();
                p.set_traits(*traits);
                p.set_positions(*places);
                p.set_away_to(*away);
                p.set_birth_year(*year);
            }
        };
        let seed_fit = |records: &mut Records, id: u16| {
            let p = records.player_mut(id).unwrap();
            let (traits, p0) = (p.traits(), p.positions()[0]);
            p.set_traits([traits[0] | 4, traits[1]]);
            p.set_birth_year(2005);
            p.set_positions([p0, 0, 0]);
        };
        // None of them fits (the trait bit out of all), the loop spends
        // its six draws over the pool and answers "not met".
        for &id in &defs {
            let p = records.player_mut(id).unwrap();
            let traits = p.traits();
            p.set_traits([traits[0] & !4, traits[1]]);
        }
        let mut six = random.clone();
        for _ in 0..6 {
            six.below(defs.len() as u32);
        }
        assert!(!run_player_event(&mut records, club, &defs, (&tables, &league_tables), 26, 0, [0, 11, 11], (64, date), &mut random));
        assert_eq!(random, six, "six failed draws, no seventh");
        // The fit man already standing at place 10 is no event at all:
        // one draw, his places as they were.
        restore(&mut records);
        let mut probe = random.clone();
        let stood = defs[probe.below(defs.len() as u32) as usize];
        {
            let p = records.player_mut(stood).unwrap();
            let (traits, p0) = (p.traits(), p.positions()[0]);
            p.set_traits([traits[0] | 4, traits[1]]);
            p.set_birth_year(2005);
            p.set_positions([p0, 10, 0]);
        }
        let mut expected = random.clone();
        expected.below(defs.len() as u32);
        assert!(!run_player_event(&mut records, club, &defs, (&tables, &league_tables), 26, 0, [0, 11, 11], (64, date), &mut random));
        assert_eq!(random, expected);
        assert_eq!(records.player(stood).unwrap().positions(), [state[defs.iter().position(|&y| y == stood).unwrap()].1[0], 10, 0]);
        // One draw names a fitting man: place 10 lands on his free
        // first side place and the event is met.
        restore(&mut records);
        let mut probe = random.clone();
        let fit = defs[probe.below(defs.len() as u32) as usize];
        seed_fit(&mut records, fit);
        let home = state[defs.iter().position(|&y| y == fit).unwrap()].1;
        let mut expected = random.clone();
        expected.below(defs.len() as u32);
        assert!(run_player_event(&mut records, club, &defs, (&tables, &league_tables), 26, 0, [0, 11, 11], (64, date), &mut random));
        assert_eq!(random, expected);
        assert_eq!(records.player(fit).unwrap().positions(), [home[0], 10, 0]);
        // The first draw's man fits in all but standing away: the draw
        // of none counts as the failed draw the loop runs on over (the
        // original reads its null player here), and the second draw's
        // fitting man takes his place.
        restore(&mut records);
        let mut probe = random.clone();
        let (a, b) = (probe.below(defs.len() as u32) as usize, probe.below(defs.len() as u32) as usize);
        assert_ne!(a, b, "the two draws must name different men");
        let (away, man) = (defs[a], defs[b]);
        records.player_mut(away).unwrap().set_away_to([1, 0]);
        seed_fit(&mut records, away);
        seed_fit(&mut records, man);
        let away_places = records.player(away).unwrap().positions();
        let mut expected = random.clone();
        expected.below(defs.len() as u32);
        expected.below(defs.len() as u32);
        assert!(run_player_event(&mut records, club, &defs, (&tables, &league_tables), 26, 0, [0, 11, 11], (64, date), &mut random));
        assert_eq!(random, expected, "the away man's draw, then the fitting one's");
        assert_eq!(records.player(man).unwrap().positions(), [state[defs.iter().position(|&y| y == man).unwrap()].1[0], 10, 0]);
        assert_eq!(records.player(away).unwrap().positions(), away_places, "the away man keeps his places");
    }
}
