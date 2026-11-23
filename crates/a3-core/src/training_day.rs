//! One training day at the human clubs: the day's planned units take
//! their sessions, pickers send an injury or an event ahead, and the
//! day looks for one trained player whose luck might turn
//! (`FUN_00749508` with `FUN_0074a1ec`, which the day's advance
//! `FUN_0064f981` runs after the ticket sales).
//!
//! Here besides: the session's injury (`FUN_00767225`), the unit's team
//! skills (`FUN_0069851b` and its kin), the events 100 to 110 of
//! `FUN_0060c1c0` with all 31 of event 102's player events, the secret
//! training's toll (`FUN_0041828e`), the day's news pick after the
//! friendlies ([`news`]), what a session takes off each trained man's
//! freshness and brings his condition (`FUN_0074ae97`), and the men's
//! own training: the day's snapshot of their choices (`FUN_00746c48`),
//! the week's start fading what they do not work on, each session's
//! rolls for the ability one works on and his learning it
//! (`FUN_00747246`), the snapshot kept again (`FUN_00746d33`); and what
//! a match's goals, assists and saved penalties teach (`FUN_00747855`).
//! And the training camp: the year calendar's camp sets the share of
//! each session by its place's ball (`FUN_006504e9`), brings its room
//! happenings (`FUN_0060ff69`) and the lonely rooms' toll; the camp
//! ticked before a match works on the squad as the match day begins
//! (`FUN_0059a2ea`, [`camp_before_match`]).
//!
//! What is not here yet: the national check of the trained
//! (`FUN_006feff8`); what the day's news posts once its category is
//! picked (`FUN_005ee5c0` with `FUN_005ef430` and `FUN_005f1eaf`; the
//! pick itself stands in [`crate::news`]), and the events' news; the
//! camp's family happening (`FUN_0060f7b9`'s 0x44a, a child's age
//! behind it), which waits on a team count no code raises; the player-manager's block
//! behind the club's byte +0x8a; and the third case of `FUN_00745d3c`,
//! which weighs two standing places — reached only in `FUN_00745281`'s
//! own side block, itself not here, since the day's site stands with
//! the first place free and event 102's picks draw men holding at most
//! one.

use std::collections::BTreeMap;

use chrono::NaiveDate;

use crate::computer_training::{cond_percent, fresh_percent};
use crate::event_text::{Fill, Lead, Posted, Value};
use crate::individual::{self, Entry, IndividualTables, FIELD_ABILITIES, LEARNT, NOTHING};
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
use crate::year::{self, YearTables};

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

/// The rest of a unit's row that a session works with (`FUN_0074ae97`):
/// what it takes off the three freshness words (+0, +2, +4) and brings
/// the condition (+6, while he stands below +8), all at a hundred
/// percent; and +0xa, the fitness units, which a club's team byte
/// +0x470 lifts by five or seven more.
const GAINS: [[i16; 6]; 39] = [
    [-20, -20, -10, 25, 6500, 0],
    [-30, -50, -20, 20, 6500, 0],
    [-10, -10, 0, 10, 5500, 0],
    [-40, -60, -30, 30, 8500, 0],
    [-30, -50, -20, 20, 6500, 0],
    [-10, -10, 0, 20, 5500, 0],
    [-150, -400, -200, 120, 10000, 2],
    [-30, -50, -20, 20, 6500, 0],
    [-30, -50, -20, 20, 6500, 0],
    [-10, -10, 0, 10, 5500, 0],
    [-20, -20, -10, 20, 6500, 0],
    [0, 0, 0, 0, 0, 0],
    [-20, -20, -10, 20, 6500, 0],
    [-50, -150, -70, 30, 8500, 0],
    [-10, -10, 0, 10, 5500, 0],
    [-10, -10, 0, 10, 5500, 0],
    [-70, -250, -120, 60, 10000, 1],
    [-110, -320, -180, 90, 10000, 1],
    [-50, -150, -70, 20, 6500, 0],
    [-70, -200, -100, 60, 9500, 1],
    [-50, -150, -70, 30, 8000, 0],
    [-70, -200, -100, 40, 10000, 1],
    [-60, -200, -100, 30, 9500, 0],
    [0, 0, 0, 0, 0, 0],
    [-20, -20, -10, 10, 6500, 0],
    [-30, -50, -20, 20, 7500, 0],
    [0, 0, 0, 0, 0, 0],
    [-130, -300, -150, 80, 10000, 1],
    [-110, -300, -150, 80, 10000, 1],
    [-200, -500, -300, 120, 10000, 2],
    [-40, -60, -30, 30, 8500, 0],
    [-30, -50, -20, 20, 6500, 0],
    [-80, -150, -80, 30, 8500, 0],
    [-40, -100, -50, 40, 7500, 0],
    [-80, -120, -60, 40, 8500, 0],
    [-20, -20, -10, 10, 5500, 0],
    [-60, -200, -100, 40, 7500, 0],
    [-110, -250, -120, 80, 10000, 1],
    [-40, -60, -30, 30, 8500, 0],
];

/// How a unit teaches each ability (`DAT_008aa5a8`, the row's bytes
/// from +0x18): the field players' sixteen, then the keepers' nine.
/// 1, a roll over the men working on it and a half chance, for a step;
/// 2, the roll alone; 3, a roll over half of them, for two steps.
const TEACHES: [[u8; 25]; 39] = [
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [2, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 3, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 3, 0, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [2, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 3],
    [0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0],
    [0, 0, 1, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 3, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 3, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 1, 2, 2, 3, 2, 0, 3, 3],
    [0, 2, 0, 0, 0, 1, 3, 1, 0, 0, 0, 1, 1, 1, 0, 0, 0, 0, 2, 2, 3, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 1, 1, 1, 2, 1, 0, 0, 0, 0, 0, 2, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
];

/// The keepers' abilities count from here among a unit's.
const KEEPER_COLUMNS: usize = 16;
/// The unit flags that halve the count of men working on Schußkraft and
/// on a keeper's Abschläge and Abwürfe (Krafttraining's).
const HALVES: [(u32, usize); 3] = [(0x100, 3), (0x200, 23), (0x400, 24)];

/// The share of a session the men take in a training camp, by the
/// level of its place (`DAT_008aad7c`); outside one a hundred, or the
/// first with the club's co-trainer.
const SHARES: [i32; 8] = [95, 95, 98, 101, 104, 107, 110, 110];
/// A camp's last day reads past the table (its level the byte before the
/// places', 118), where the exe holds nought (`0x8aae68`).
const PAST_SHARES: i32 = 0;
/// The club flag (team +0x874's low byte, the club's +0xfd) that halves
/// the next session's share, and goes with it.
const HALF_SHARE: u8 = 4;
pub(crate) const CLUB_FLAGS: usize = 0x874;
/// The team byte (`FUN_006994b3`) that lifts the fitness units.
const FITNESS_LIFT: usize = 0x470;

/// A player-manager trains as hard as his career's first slider says
/// (`FUN_0074ae97`'s float factors, by the slider from 1; 2 keeps it).
const OWN_PLAY: [f32; 5] = [0.4, 0.7, 1.0, 1.1, 1.2];

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
/// The codes of the training's events as they post them: the illness of
/// 100 and 101, the torn knock of 110, the player events.
const ILLNESS_EVENT: u16 = 0x61;
const INJURY_EVENT: u16 = 0x60;
const PLAYER_EVENT: u16 = 99;

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
    (mood, medical, league, individual, camps): (&MoodTables, &MedicalTables, &LeagueTables, &IndividualTables, &YearTables),
    recent: &mut Vec<i32>,
    posted: &mut Vec<Posted>,
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
        // The camp the trainer's year calendar (the club's +0x8a) has the
        // club in today, by its level (`FUN_006504e9`'s frame +0xa).
        let camp = usize::from(team.humans()[0])
            .checked_sub(1)
            .and_then(|trainer| records.calendars.get(trainer))
            .zip(usize::try_from(today).ok())
            .and_then(|(calendar, d)| year::camp_level(camps, calendar, d));
        let flags = state.training.flags;
        let mut plan = state.training.plan();
        // The men's own training as the club's store keeps it, an entry
        // for each of the squad in its order (`FUN_00746c48`).
        let mut own = snapshot(records, slot, &ids);
        if weekday == 0 {
            // Each man took the intensity his entry holds before the
            // day's loop ([`week_start`]).
            fade(records, &mut own);
            // Whose condition the two rolls below train is lost in the
            // decompile; the replica trains the squad member whose turn
            // it is.
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
                camp,
                (mood, medical, league),
                recent,
                posted,
                random,
            );
            learn(records, &mut own, unit, human, individual, random);
        }
        // The snapshot goes back into the store (`FUN_00746d33`), the
        // club's own copy with it.
        if let Some(store) = records.individual.get_mut(slot) {
            let mut entries = store.entries();
            for &entry in &own {
                individual::keep(&mut entries, entry);
            }
            store.set_entries(&entries);
            state.individual = entries;
        }
        // In a camp of the calendar (`FUN_005eab71`) its rooms bring the
        // day's happenings, then a lonely room now and then costs the
        // will to win.
        if camp.is_some() {
            camp_happenings(records, (country, number), &ids, today, posted, random);
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

/// The reason a man takes against his trainer for an extra shift he
/// did not deserve (`FUN_00770800`'s 0x1000).
const SHIFT_GRUDGE: u32 = 0x1000;
/// The reason a strong man takes against his trainer for being sent to
/// the youth round (0x8000).
const YOUTH_GRUDGE: u32 = 0x8000;
/// The strength from which the youth round is beneath a man.
const YOUTH_STRENGTH: u8 = 8;

/// The week's start at the human clubs (`FUN_0064f981` with its first day
/// the week's first: after the presale and the day's headline, before its
/// loop over the day): `FUN_00747144` hands each club's squad to
/// `FUN_00746f46`, then `FUN_00786285` settles the week's slips.
///
/// In `FUN_00746f46` each man takes the intensity his entry holds (a
/// choice made during the week reaches him only here). With the extra
/// shift ("Sonderschicht", the entry's +0x16) a man in condition (over
/// 50) who did nothing wrong this week or last (`FUN_00702e05`) holds it
/// against his trainer three times in ten (reason 0x1000); one in poor
/// condition (50 or less) gains two points of it (`FUN_00411130`); and for
/// one who slipped the team's mood (`FUN_007147ce`) and its will to win
/// (`FUN_0069efe8`) rise by two, a slip of the first level counting as
/// punished this week (+0xb4, the record's `fined`, bit 1). The youth
/// round ("Nachwuchsrunde", +0x14) is beneath a man of strength 8 or
/// more, one in ten of them taking it ill each week (0x8000), and leaves
/// a weaker one at peace; under 60 in condition he gains a point. Without
/// it the grievance goes. The entries are read as the training day reads
/// them (`FUN_00746c48`) and not written back.
///
/// `FUN_00786285` then, over each club's squad in its order
/// (`FUN_00786230`): a slip of this week or last left unpunished costs
/// the team a point of its mood and the trainer a point with the fans and
/// the media (`FUN_00418065`) — for a manager who plays, the mood ten up
/// and a point with the public too — and every man's punishments of the
/// week are forgotten.
pub fn week_start(records: &mut Records, mood: &MoodTables, random: &mut Random) {
    let clubs = statistics::human_clubs(records);
    for &club in &clubs {
        let Some(slot) = human_slot(records, club) else { continue };
        let ids = records.team(club.0.into(), club.1 as u16).map(|t| t.players()).unwrap_or_default();
        for entry in snapshot(records, slot, &ids) {
            shift_and_youth(records, mood, club, &entry, random);
        }
    }
    for &club in &clubs {
        let ids = records.team(club.0.into(), club.1 as u16).map(|t| t.players()).unwrap_or_default();
        for id in ids {
            let Some(p) = records.player_mut(id) else { continue };
            let level = crate::discipline::recent_offence(p);
            let unpunished = level >= 1 && p.fined() & (1 << level) == 0;
            p.set_fined(0);
            if !unpunished {
                continue;
            }
            let (by, likings) =
                if id <= crate::mood::LAST_REAL_ID { (-1, [0, -1, -1]) } else { (10, [-1, -1, -1]) };
            crate::interview::team_mood(records, mood, club, by);
            crate::exhibition::move_standing(records, club, likings, 0);
        }
    }
}

/// The human slot (0 to 3) of a club a human runs (the club object's
/// +0x11de less one).
fn human_slot(records: &Records, (country, number): (u8, i16)) -> Option<usize> {
    let team = records.team(country.into(), u16::try_from(number).ok()?)?;
    let human = usize::from(team.human());
    (1..=crate::human_day::HUMANS).contains(&human).then(|| human - 1)
}

/// One man's extra shift and youth round at the week's start
/// ([`week_start`], `FUN_00746f46`'s loop).
fn shift_and_youth(records: &mut Records, mood: &MoodTables, club: (u8, i16), entry: &Entry, random: &mut Random) {
    let Some(p) = records.player_mut(entry.id) else { return };
    p.set_intensity(entry.intensity as u8);
    if entry.extra {
        if cond_percent(p) > 50 && crate::discipline::recent_offence(p) == 0 && random.below(100) < 30 {
            mood.set_reasons(p, SHIFT_GRUDGE);
        }
        if cond_percent(p) <= 50 {
            train_condition(p, 2);
        }
        let slipped = crate::discipline::recent_offence(p);
        if slipped > 0 {
            crate::interview::team_mood(records, mood, club, 2);
            if let Some(team) = records.team_mut(club.0.into(), club.1 as u16) {
                team.set_will_to_win(team.will_to_win().saturating_add(2).clamp(-200, 200));
            }
            if slipped == 1
                && let Some(p) = records.player_mut(entry.id)
            {
                p.set_fined(p.fined() | 2);
            }
        }
    }
    let Some(p) = records.player_mut(entry.id) else { return };
    if !entry.youth {
        crate::mood::clear_reasons(p, YOUTH_GRUDGE);
        return;
    }
    if p.strength() < YOUTH_STRENGTH {
        crate::mood::clear_reasons(p, YOUTH_GRUDGE);
    } else if random.below(100) < 10 {
        mood.set_reasons(p, YOUTH_GRUDGE);
    }
    if cond_percent(p) < 60 {
        train_condition(p, 1);
    }
}

/// A man of a human's club who played a match leaves the youth round
/// (`FUN_007471bd`, from the day's graded pass over the played men
/// `FUN_006f86e1`): his entry — or where the store holds none, a bare
/// one of his id, working on the first ability at no intensity
/// (`FUN_00746ddf`) — kept with the round's flag down (`FUN_00746e87`).
pub fn leave_youth_round(records: &mut Records, (country, number): (u16, u16), id: u16) {
    let Some(slot) = human_slot(records, (country as u8, number as i16)) else { return };
    let Some(store) = records.individual.get_mut(slot) else { return };
    let mut entries = store.entries();
    let mut entry = entries.iter().find(|e| e.id == id).copied().unwrap_or_else(|| {
        let mut bare = Entry::default();
        bare.id = id;
        bare.ability = 0;
        bare.intensity = individual::OUT;
        bare
    });
    entry.youth = false;
    individual::keep(&mut entries, entry);
    store.set_entries(&entries);
}

/// The human clubs' own copies of their individual training, as their
/// stores hold them once the day's matches are played.
pub fn refresh_individual(records: &Records, clubs: &mut BTreeMap<(usize, usize), ClubState>, world: &world::World) {
    for (&(slot, place), state) in clubs.iter_mut() {
        let Some(nation) = world.country(slot).map(|c| c.nation) else { continue };
        let Some(human) = human_slot(records, (nation as u8, place as i16)) else { continue };
        if let Some(store) = records.individual.get(human) {
            state.individual = store.entries();
        }
    }
}

/// The day's news (`FUN_005ef842`, run by `FUN_0064f981` in its second
/// loop over the human clubs, after the training, the day's booked events
/// and the friendlies (`FUN_007615bb`)): a club meets it when the day's
/// gates drew today for its slot, and none in the first season's quiet
/// days (`FUN_005ef56c`). The category pick and its handlers stand in
/// `crate::news`.
/// `slots` are the day's event gates of
/// [`crate::human_day::event_slots`], one per human club in the order of
/// [`crate::statistics::human_clubs`]. What the clubs are told comes back.
///
/// With no category picked (the rest of the table, 0) `FUN_005ef8cc`
/// goes on over the club's squad (+0xe2): under it of a thousand the
/// squad's player news ([`crate::news::player_news`]), which ends the
/// day's news for the club either way, else under it of five thousand
/// the player-manager's own news ([`crate::news::manager_news`],
/// behind the club's human marker at +0x8a); else on every
/// third half week by the club's number the club's youth walks its
/// week, the human told
/// ([`crate::youth_week::walk`]). A picked category's handler that
/// posts nothing goes on so too: categories 1 ([`crate::news::children`])
/// to 6 ([`crate::news::suspicion`]), 7 ([`crate::news::affair_known`]),
/// 8 ([`crate::news::family`]), 9 ([`crate::news::engaged`]), 10
/// ([`crate::news::low_spirits`]), 11 ([`crate::news::at_odds`]) and 12
/// ([`crate::news::new_affair`]), 50 ([`crate::news::bootlicker`]), 51
/// ([`crate::news::informer`]), 52 ([`crate::news::moles`]) and 55
/// ([`crate::news::habits`]) do here, and so does 19 for a man without a
/// hobby ([`crate::news::about_a_player`]); 18 ([`crate::news::new_hobby`])
/// and 19 for a man with one always end the news; 20, the press on a man
/// ([`crate::news::press_on_a_player`]), 21, a man against his trainer
/// ([`crate::news::revolt`]), 22, a new man's first months
/// ([`crate::news::first_months`]) and 23, a schoolboy's days
/// ([`crate::news::school_days`]), go on when they tell nothing, and so
/// does 24, the men far from home ([`crate::news::far_from_home`]), when
/// it pairs two of them; the tale it draws instead ends the day's news,
/// told on the man it draws. 25, happenings among the club's own men
/// ([`crate::news::in_the_team`]), ends the day's news whichever way it
/// goes, telling of friendships begun and mended and of men fallen
/// out — over a form crisis, a revenge foul, a sneer, a mocked age, a
/// snatched ad contract and one sister.
/// `recent` are the events lately met
/// ([`crate::season::Season::recent_events`]), `tables` those a new
/// girlfriend's name and birthday are drawn from, `league` the goal
/// classes a father's club is weighed by, `plan` a club's three
/// sessions of today ([`units_today`]), `mood` what
/// the team's mood does to its squad, `nation_kinds` the nations' kinds
/// a far-from-home tale is weighed by
/// ([`crate::world::World::nation_kinds`]), `reported` the humans' doping
/// reports, `controls` where a reported club's positive control goes
/// (event 0xcb) and `questions` where the question over a fine goes
/// ([`crate::fine_question`], food's case 4). (The town's share in the
/// stadium's building after a youth's quiet week is not built.)
pub fn news(
    records: &mut Records,
    today: i32,
    slots: &[i16],
    recent: &mut Vec<i32>,
    tables: &crate::first_season::Tables,
    league: &LeagueTables,
    mood: &MoodTables,
    nation_kinds: &[u8],
    plan: &dyn Fn((u16, u16)) -> [u8; training::SESSIONS],
    (reported, controls, questions): (&mut Reports, &mut Vec<crate::doping_control::Control>, &mut Vec<crate::fine_question::Question>),
    random: &mut Random,
) -> Vec<Posted> {
    let mut posted = Vec::new();
    if crate::event_text::quiet(records) {
        return posted;
    }
    for (k, (country, number)) in statistics::human_clubs(records).into_iter().enumerate() {
        if slots.get(k) != Some(&(today.rem_euclid(7) as i16)) {
            continue;
        }
        let club = (u16::from(country), number as u16);
        let squad = records.team(club.0, club.1).map_or(0, |team| team.player_count());
        let units = plan(club);
        let planned = units.iter().any(|&u| u != training::FREE);
        // The gates before the pick: the waiting appointments
        // (`FUN_006a700a`, whose return keeps only the sign of its
        // count in the high half, so it never ends the news), the rest
        // gate (above), the association's points for repeated doping,
        // the reports of doping against the trainer, and the group
        // news of a club below 100 of mood — its 400-roll and
        // fan-group pick end the day told or not (`FUN_005e3885`).
        crate::news::appointments(records, club, planned, &mut posted, random);
        crate::news::doping_penalty(records, club, &mut posted);
        if crate::news::doping_reports(records, club, reported, controls, &mut posted, random) {
            continue;
        }
        if crate::news::fan_clubs(records, club, &mut posted, random) {
            continue;
        }
        let code = crate::news::pick(random);
        let told = match code {
            0 => false,
            1 => crate::news::children(records, club, recent, &mut posted, random),
            2 => crate::news::service(records, club, today, recent, tables, &mut posted, random),
            3 => crate::news::single(records, club, today, recent, tables, &mut posted, random),
            4 => crate::news::with_girlfriend(records, club, recent, tables, &mut posted, random),
            5 => crate::news::married(records, club, today, recent, &mut posted, random),
            6 => crate::news::suspicion(records, club, &mut posted, random),
            7 => crate::news::affair_known(records, club, &mut posted, random),
            8 => crate::news::family(records, club, today, league, &mut posted, random),
            9 => crate::news::engaged(records, club, recent, random),
            10 => crate::news::low_spirits(records, club, planned, &mut posted, random),
            11 => crate::news::at_odds(records, club, today, &mut posted, random),
            12 => crate::news::new_affair(records, club, &mut posted, random),
            50 => crate::news::bootlicker(records, club, today, planned, (league, mood), recent, &mut posted, random),
            51 => crate::news::informer(records, club, &mut posted, random),
            52 => crate::news::moles(records, club, random),
            55 => crate::news::habits(records, club, league, &mut posted, random),
            18 => {
                crate::news::new_hobby(records, club, tables, &mut posted, random);
                true
            }
            19 => crate::news::about_a_player(records, club, today, planned, league, (&mut posted, questions), random),
            20 => crate::news::press_on_a_player(records, club, league, random),
            21 => crate::news::revolt(records, club, today, units, league, (recent, &mut posted), random),
            22 => crate::news::first_months(records, club, &mut posted, random),
            23 => crate::news::school_days(records, club, &mut posted, random),
            24 => crate::news::far_from_home(records, club, nation_kinds, tables, &mut posted, random),
            25 => crate::news::in_the_team(records, club, today, tables, &mut posted, random),
            _ => true,
        };
        if told {
            continue;
        }
        if random.below(PLAYER_NEWS) < u32::from(squad) {
            crate::news::player_news(records, club, today, planned, league, recent, &mut posted, random);
            continue;
        }
        if random.below(OTHER_NEWS) < u32::from(squad) {
            crate::news::manager_news(records, club, planned, recent, &mut posted, random);
            continue;
        }
        if records.globals.half_weeks() % 3 == number as u16 % 3 {
            crate::youth_week::walk(records, (country, number as u16), true, random, &mut posted);
        }
    }
    posted
}

/// Which humans reported which to the association for doping
/// ([`crate::season::Season::reported`]).
pub type Reports = [[bool; crate::records::MANAGERS]; crate::records::MANAGERS];

/// A human club's three sessions of today in its plan (the club object's
/// +0xc0a row of the day, which `FUN_005ea83f` asks unit by unit, and
/// `FUN_005ea8cc` whether any is not free), as the day's training left
/// it; all free for a club the replica holds no plan of.
pub fn units_today(clubs: &BTreeMap<(usize, usize), ClubState>, world: &world::World, (country, number): (u16, u16), today: i32) -> [u8; training::SESSIONS] {
    let weekday = today.rem_euclid(7) as usize;
    world
        .countries
        .iter()
        .position(|c| c.as_ref().is_some_and(|c| c.nation == country))
        .and_then(|slot| clubs.get(&(slot, usize::from(number))))
        .map_or([training::FREE; training::SESSIONS], |state| std::array::from_fn(|k| state.training.unit(weekday, k)))
}

/// The tail's two draws of the day's news over the squad (`FUN_005ef8cc`).
const PLAYER_NEWS: u32 = 1000;
const OTHER_NEWS: u32 = 5000;

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
    camp: Option<u8>,
    (mood, medical, league): (&MoodTables, &MedicalTables, &LeagueTables),
    recent: &mut Vec<i32>,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) {
    let mut trainable = Vec::new();
    let mut injured = Vec::new();
    let share = share(records, (country, number), camp);
    let lift = records.team(country, number).map_or(0, |t| t.get::<u8>(FITNESS_LIFT));
    // What the unit took off the first freshness word of the men before
    // him, at a hundred percent (`FUN_0074a1ec`'s +0x10 of the frame).
    let mut taken: i16 = 0;
    for &id in ids {
        let Some(player) = records.player(id) else { continue };
        if days_out(records, player) < 7 {
            // The national check (`FUN_006fef73`, `FUN_006feff8`) is
            // not in the corpus and caps nothing here; a 0x20 unit only
            // trains its own kind — the record's scratch byte +0x3,
            // kept at +0x21d by the load copies.
            if unit != 0x20 || player.get::<u8>(0x21d) == 1 {
                trainable.push(id);
                let own_play = (id > crate::mood::LAST_REAL_ID)
                    .then(|| records.managers.get(usize::from(id - crate::mood::LAST_REAL_ID - 1)))
                    .flatten()
                    .map(|m| m.own_play()[0]);
                if let Some(player) = records.player_mut(id) {
                    gains(player, unit, counter, share, lift, taken, own_play);
                }
                taken = taken.wrapping_add(GAINS[unit][0]);
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
        let met = training_event(records, (country, number), ids, &trainable, (mood, league), event, unit, day_units, (today, date), recent, posted, random);
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

/// The share of a session the men take, in percent (`FUN_0074a1ec`'s
/// +0xe of the frame): in a camp [`SHARES`] by its level; outside one
/// the decompile's frame says a hundred, the original's dumps of
/// Probe1's week (no camp, a co-trainer) the first of [`SHARES`], so the
/// co-trainer takes that; half of it for the session after the club's
/// flag, which goes.
fn share(records: &mut Records, (country, number): (u16, u16), camp: Option<u8>) -> i32 {
    let Some(team) = records.team_mut(country, number) else { return 100 };
    let mut share = match camp {
        Some(level) => SHARES.get(usize::from(level)).copied().unwrap_or(PAST_SHARES),
        None if team.co_trainer() > 0 => SHARES[0],
        None => 100,
    };
    let flags = team.get::<u8>(CLUB_FLAGS);
    if flags & HALF_SHARE != 0 {
        share /= 2;
    }
    team.put::<u8>(CLUB_FLAGS, flags & !HALF_SHARE);
    share
}

/// A camp's rooms (`CZimmerbelegung`): twenty of two men each.
const ROOMS: usize = 20;
/// The rounds of happenings a camp's day brings at most (`FUN_0060ff69`).
const CAMP_ROUNDS: usize = 5;
/// The event code of the camp's room happenings, and their texts in
/// `ereignis.saq`: his best friend in another room (two), his room-mate
/// the one he hates (two), one he cannot stand (falling out, making up).
const CAMP_EVENT: u16 = 0x60;
const FRIEND_ELSEWHERE: [u16; 2] = [0x613, 0x614];
const HATED_MATE: [u16; 2] = [0x615, 0x616];
const DISLIKED_MATE: [u16; 2] = [0x617, 0x618];
/// The record's bit (+0x1b4) of a man a camp happening met, who meets no
/// other (`FUN_007003aa`; only the player's reset `FUN_007074e5` clears
/// it), and the one of a man the team shuts out.
const CAMP_MET: u32 = 0x40000;
const OSTRACISED: u32 = 0x20000;
/// From this many days out a man has no camp happening.
const CAMP_DAYS_OUT: i32 = 8;
/// The team byte (+0x8ed) the camp's wife's happening waits for at 20 —
/// raised by `FUN_0069a77c`, which nothing in the decompile calls.
const CAMP_COUNT: usize = 0x8ed;
/// The wife's happening: before day 42, one in fifteen, the wife's bits
/// (2 to 4 of the partner byte) at 1, his private contentment 15 and up
/// to nine more down (`FUN_00702c39`, `FUN_007010d9`; event 0x449 with
/// its values, not posted here).
const WIFE_DAYS: i32 = 0x2a;
const WIFE_COUNT: u8 = 20;
const WIFE_ODDS: u32 = 15;

/// The day's happenings of a camp (`FUN_0060ff69`): up to five rounds,
/// as long as one finds a pair to post about.
fn camp_happenings(
    records: &mut Records,
    club: (u16, u16),
    ids: &[u16],
    today: i32,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) {
    for _ in 0..CAMP_ROUNDS {
        if !camp_round(records, club, ids, today, posted, random) {
            break;
        }
    }
}

/// One round of a camp's happenings (`FUN_0060f7b9`). The rooms fill in
/// the squad's order, two to a room, the rest left out; then room by
/// room the first man fit for one (not met before, under eight days out,
/// no manager; `FUN_0060f774`), with a room-mate:
/// - his best friend, of the squad and fit, sleeps elsewhere: both are
///   five less content with the trainer or, on rand(2)'s 0, three
///   ([`FRIEND_ELSEWHERE`]);
/// - else his room-mate, fit, is the one he hates: both ten less or him
///   alone fifteen ([`HATED_MATE`]);
/// - else the mate is shut out by the team or one of those he cannot
///   stand, and on rand(10)'s 0, fit: both ten less on rand(3) below 2,
///   else they make up ([`reconcile`], [`DISLIKED_MATE`]).
///
/// Then the room-mate the same way for his best friend. Each pair met
/// takes [`CAMP_MET`] and ends the round. With none, one of the squad is
/// drawn for the family's happenings before the round ends empty.
fn camp_round(
    records: &mut Records,
    club: (u16, u16),
    ids: &[u16],
    today: i32,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    let mut rooms = [[0u16; 2]; ROOMS];
    let mut filled = [0usize; ROOMS];
    for &id in ids {
        let Some(player) = records.player(id) else { continue };
        let Ok(room) = usize::try_from(player.room()) else { continue };
        if room < ROOMS && filled[room] < 2 {
            rooms[room][filled[room]] = id;
            filled[room] += 1;
        }
    }
    let in_squad = |id: u16| id != 0 && ids.contains(&id);
    let fit = |records: &Records, id: u16| {
        in_squad(id)
            && id <= crate::mood::LAST_REAL_ID
            && records
                .player(id)
                .is_some_and(|p| p.shown() & CAMP_MET == 0 && days_out(records, p) < CAMP_DAYS_OUT)
    };
    let mut met = |records: &mut Records, pair: [u16; 2], by: [i32; 2], text: u16| {
        for (id, by) in pair.into_iter().zip(by) {
            if let Some(p) = records.player_mut(id) {
                p.set_shown(p.shown() | CAMP_MET);
                if by != 0 {
                    crate::mood::change_trainer(p, by);
                }
            }
        }
        posted.push(Posted { club, code: CAMP_EVENT, text, fill: Fill::Numbered(pair[0], pair[1]) });
    };
    let friend_of = |records: &Records, id: u16| records.player(id).map_or(0, |p| p.best_friend());
    for [one, mate] in rooms {
        if fit(records, one) && mate > 0 {
            let friend = friend_of(records, one);
            if in_squad(friend) && friend != mate && fit(records, friend) {
                let (by, text) = if random.below(2) != 0 { (-5, FRIEND_ELSEWHERE[0]) } else { (-3, FRIEND_ELSEWHERE[1]) };
                met(records, [one, friend], [by, by], text);
                return true;
            }
            let hates = records.player(one).map_or(0, |p| p.hates());
            if hates == mate && fit(records, mate) {
                if random.below(2) != 0 {
                    met(records, [one, mate], [-10, -10], HATED_MATE[0]);
                } else {
                    met(records, [one, mate], [-15, 0], HATED_MATE[1]);
                }
                return true;
            }
            let shunned = records.player(mate).is_some_and(|p| p.shown() & OSTRACISED != 0)
                || records.player(one).is_some_and(|p| p.dislikes().contains(&mate));
            if shunned && random.below(10) == 0 && fit(records, mate) {
                if random.below(3) < 2 {
                    met(records, [one, mate], [-10, -10], DISLIKED_MATE[0]);
                } else {
                    reconcile(records, one, mate);
                    met(records, [one, mate], [0, 0], DISLIKED_MATE[1]);
                }
                return true;
            }
        }
        if fit(records, mate) && one > 0 {
            let friend = friend_of(records, mate);
            if in_squad(friend) && friend != one && fit(records, friend) {
                let (by, text) = if random.below(2) != 0 { (-5, FRIEND_ELSEWHERE[0]) } else { (-3, FRIEND_ELSEWHERE[1]) };
                met(records, [mate, friend], [by, by], text);
                return true;
            }
        }
    }
    // One of the squad drawn, none a manager or away (`FUN_005ebbf6`
    // with its flag; the handle's +0x6a), for the family's happenings.
    if ids.is_empty() {
        return false;
    }
    let id = ids[random.below(ids.len() as u32) as usize];
    if id > crate::mood::LAST_REAL_ID || records.player(id).is_none_or(|p| p.away_to()[0] != 0) {
        return false;
    }
    let count = records.team(club.0, club.1).map_or(0, |t| t.get::<u8>(CAMP_COUNT));
    let married = |records: &Records| records.player(id).is_some_and(|p| p.partner() & 3 == 3);
    if today < WIFE_DAYS && count == WIFE_COUNT && random.below(WIFE_ODDS) == 0 && married(records) {
        let by = -15 - random.below(10) as i32;
        if let Some(p) = records.player_mut(id) {
            p.set_partner(p.partner() & 0xe3 | 1 << 2);
            crate::mood::change_private(p, by);
        }
    }
    false
}

/// Two men making up (`FUN_00703d61`): the first hates the other no more
/// and the other nobody — nor whom that one hated, on down the chain
/// (`FUN_00703b8e`, `FUN_00703b1a`); and each leaves the other's list of
/// those he cannot stand ([`forgive`]).
fn reconcile(records: &mut Records, one: u16, other: u16) {
    if records.player(one).is_some_and(|p| p.hates() == other) {
        if let Some(p) = records.player_mut(one) {
            p.set_hates(0);
        }
        let mut next = other;
        while let Some(p) = records.player_mut(next) {
            let hated = p.hates();
            if hated == 0 {
                break;
            }
            p.set_hates(0);
            next = hated;
        }
    }
    forgive(records, one, other);
}

/// `other` off `one`'s list of those he cannot stand, then `one` off
/// `other`'s, and back while either moves (`FUN_00703930`): the list
/// closes up behind him, its last entry staying where it was, and the
/// nought meant for it falls on the third of his friends (+0x1fc). Where
/// he stands last nothing moves, and the original's call back would
/// never end: the replica stops there.
fn forgive(records: &mut Records, mut one: u16, mut other: u16) {
    loop {
        let Some(p) = records.player_mut(one) else { return };
        let mut list = p.dislikes();
        let mut at = None;
        for (k, &id) in list.iter().enumerate() {
            if id == other {
                at = Some(k);
                break;
            }
            if id == 0 {
                return;
            }
        }
        let Some(at) = at else { return };
        for k in at..2 {
            list[k] = list[k + 1];
        }
        let moved = list != p.dislikes();
        p.set_dislikes(list);
        let mut friends = p.friends();
        friends[2] = 0;
        p.set_friends(friends);
        if !moved {
            return;
        }
        std::mem::swap(&mut one, &mut other);
    }
}

/// The count of camps before matches in the team's flags (+0x308).
const CAMPS_HELD: u32 = 0x300;

/// The camp a club booked before its match (`FUN_0059a2ea`, which the
/// match day's `FUN_006ef296` runs for both sides of every match before
/// it is played), where its flag [`training::CAMP`] stands: the camp's
/// conditions work on the team ([`camp_conditions`]); the first camp of
/// the season makes each of the squad three or four more content with
/// the team, the second each on rand(100) below 50, any later costs ten
/// of the team's mood; the flag goes, and the count of camps (bits 8
/// and 9) stands at one after the first, else at two. The new season
/// clears the count (`FUN_00772bee`).
pub fn camp_before_match(
    records: &mut Records,
    clubs: &mut BTreeMap<(usize, usize), ClubState>,
    world: &world::World,
    (country, number): (u8, i16),
    tables: &MoodTables,
    random: &mut Random,
) {
    let Ok(team_number) = u16::try_from(number) else { return };
    let Some(team) = records.team(country.into(), team_number) else { return };
    let flags = team.training_flags();
    if flags & training::CAMP == 0 {
        return;
    }
    let ids = team.players();
    camp_conditions(records, (country, number), tables, random);
    let count = (flags & CAMPS_HELD) >> 8;
    if count < 2 {
        let chance = if count == 0 { 100 } else { 50 };
        for &id in &ids {
            if random.below(100) < chance {
                let by = 3 + random.below(2) as i32;
                if let Some(p) = records.player_mut(id) {
                    crate::mood::change(p, 1, by);
                }
            }
        }
    } else {
        crate::interview::team_mood(records, tables, (country, number), -10);
    }
    let held = if count == 0 { 1 } else { 2 };
    let flags = flags & !(CAMPS_HELD | training::CAMP) | held << 8;
    if let Some(team) = records.team_mut(country.into(), team_number) {
        team.set_training_flags(flags);
    }
    let slot = world.countries.iter().position(|c| c.as_ref().is_some_and(|c| c.nation == u16::from(country)));
    if let Some(state) = slot.and_then(|slot| clubs.get_mut(&(slot, usize::from(team_number)))) {
        state.training.flags = flags;
    }
}

/// The camp's conditions at work (`FUN_0077225c`), at a club its trainer
/// (+0x8a) runs: m, the mean of his three sliders (food, going out,
/// control, 0 to 4; the manager's +0x135e), moves the team's mood by
/// 2 − m; then each of the first |m − 2| of the squad gains, or with m
/// below 2 loses, two points of freshness on rand(2)'s 0, else of
/// condition — a rand over the squad drawn before each and not used.
fn camp_conditions(records: &mut Records, (country, number): (u8, i16), tables: &MoodTables, random: &mut Random) {
    let Some(team) = records.team(country.into(), number as u16) else { return };
    let trainer = usize::from(team.humans()[0]);
    let ids = team.players();
    let Some(manager) = trainer.checked_sub(1).and_then(|t| records.managers.get(t)) else { return };
    let m = manager.camp_rules().iter().map(|&r| i32::from(r)).sum::<i32>() / 3;
    crate::interview::team_mood(records, tables, (country, number), 2 - m);
    let s = m - 2;
    for k in 0..s.unsigned_abs() as usize {
        random.below(ids.len() as u32);
        let freshness = random.below(2) == 0;
        let Some(p) = ids.get(k).and_then(|&id| records.player_mut(id)) else { continue };
        if freshness {
            train_freshness(p, 2 * s.signum());
        } else {
            train_condition(p, 2 * s.signum());
        }
    }
}

/// What a session does to one trained man (`FUN_0074ae97`). Out or
/// building up (his intensity below light), only the day's first unit
/// touches him: the row's freshness at a hundred percent, the first word
/// held at nought and a little back below 0.6, and a point or two of
/// condition below 40. Else his share is the session's by his morale —
/// half of it light, half again hard; a player-manager's by his career's
/// slider; doubled in the day's third unit once the men before him took
/// more than 30 of the first word — and the row's freshness and, below
/// its cap, condition go by that share: a tenth less for the first trait
/// word's bit 4, a tenth more for bit 2, half while he stands below 25
/// percent fresh, and a fitness unit five or seven more by the team's
/// byte.
fn gains(
    player: &mut Player,
    unit: usize,
    counter: u32,
    share: i32,
    lift: u8,
    taken: i16,
    own_play: Option<u8>,
) {
    let [first, second, third, condition, cap, fitness] = GAINS[unit];
    let mut fresh = player.freshness();
    let intensity = i32::from(player.intensity());
    if intensity < i32::from(individual::LIGHT) {
        if counter == 1 {
            fresh[0] = fresh[0].wrapping_add(first).max(0);
            fresh[1] = fresh[1].wrapping_add(second);
            fresh[2] = fresh[2].wrapping_add(third);
            if fresh[0] < 30 {
                fresh[0] += 2;
            } else if fresh[0] < 60 {
                fresh[0] += 1;
            }
            player.set_freshness(fresh);
            let k = cond_percent(player);
            if k < 20 {
                train_condition(player, 2);
            } else if k < 40 {
                train_condition(player, 1);
            }
        }
        return;
    }
    let mut v = i32::from(player.morale()) * share / 100;
    if intensity != i32::from(individual::NORMAL) {
        if intensity == i32::from(individual::LIGHT) {
            v /= 2;
        } else {
            v += v / 2;
        }
    }
    if let Some(play) = own_play {
        if play != 2 {
            let factor = OWN_PLAY.get(usize::from(play)).copied().unwrap_or(OWN_PLAY[0]);
            v = (f64::from(v) * f64::from(factor)).trunc() as i32;
        }
    }
    if counter == 3 && taken < -30 {
        v <<= 1;
    }
    let part = |row: i16, v: i32, of: i32| (i32::from(row) * v / of) as i16;
    fresh[0] = fresh[0].wrapping_add(part(first, v, 100)).max(0);
    fresh[1] = fresh[1].wrapping_add(part(second, v, 100));
    fresh[2] = fresh[2].wrapping_add(part(third, v, 100));
    player.set_freshness(fresh);
    if player.condition() < cap {
        let traits = player.traits()[0];
        if traits & 4 != 0 {
            v -= v / 10;
        } else if traits & 2 != 0 {
            v += v / 10;
        }
        let of = if fresh_percent(player) < 25 { 200 } else { 100 };
        let mut c = player.condition().wrapping_add(part(condition, v, of));
        if fitness != 0 && lift != 0 {
            if lift == 2 {
                c = c.wrapping_add(7);
            } else if lift == 1 {
                c = c.wrapping_add(5);
            }
        }
        player.set_condition(c);
    }
}

/// The squad's own training for the day (`FUN_00746c48`): each man's
/// entry out of the club's store, in the squad's order, or one made for
/// him (`FUN_007467de`, which also takes a good bit off where the bad one
/// is set, in his record).
fn snapshot(records: &mut Records, slot: usize, ids: &[u16]) -> Vec<Entry> {
    let store = records.individual.get(slot).map(|s| s.entries()).unwrap_or_default();
    let mut own = Vec::with_capacity(ids.len());
    for &id in ids {
        if let Some(&entry) = store.iter().find(|e| e.id == id) {
            own.push(entry);
            continue;
        }
        let Some(p) = records.player(id) else {
            let mut entry = Entry::default();
            entry.id = id;
            own.push(entry);
            continue;
        };
        let keeper = p.get::<u8>(0x21d) == 1;
        let suspended = p.away_kind() & crate::discipline::SUSPENDED != 0;
        let intensity = individual::first_intensity(days_out(records, p), suspended);
        let (good, bad) = (p.good(), p.bad());
        own.push(Entry::new(id, keeper, [good, bad], p.favourite(), intensity));
        let span = (1u32 << individual::abilities(keeper)) - 1;
        let clash = good & bad & span as u16;
        if clash != 0 {
            if let Some(p) = records.player_mut(id) {
                p.set_good(good & !clash);
            }
        }
    }
    own
}

/// The week's first day (`FUN_00749508` with its day 0): each ability a
/// man neither has nor works on loses two of what he got, down to nought.
fn fade(records: &Records, own: &mut [Entry]) {
    for entry in own {
        let Some(p) = records.player(entry.id) else { continue };
        let good = p.good();
        for b in 0..FIELD_ABILITIES {
            if b as i16 != entry.ability && good & (1 << b) == 0 {
                let got = entry.progress[b];
                entry.progress[b] = if got < 2 { 0 } else { got - 2 };
            }
        }
    }
}

/// The men's own abilities after a session (`FUN_00749508` behind
/// `FUN_0074a1ec`). Those fit today working on one count by it — a
/// keeper's after the field players' sixteen — the unit's flags halving
/// three of the counts; then each of them who lacks it rolls by what the
/// unit teaches ([`TEACHES`]) over how many work on it: the fewer, the
/// likelier. Past 99 he has learnt it ([`learnt`]) and works on nothing;
/// one who has it already stands at a hundred.
fn learn(
    records: &mut Records,
    own: &mut [Entry],
    unit: usize,
    human: u8,
    tables: &IndividualTables,
    random: &mut Random,
) {
    let Some(teaches) = TEACHES.get(unit) else { return };
    let column = |records: &Records, entry: &Entry| -> Option<usize> {
        let ability = usize::try_from(entry.ability).ok().filter(|&a| a < FIELD_ABILITIES)?;
        let p = records.player(entry.id)?;
        (days_out(records, p) == 0)
            .then(|| ability + if p.get::<u8>(0x21d) == 1 { KEEPER_COLUMNS } else { 0 })
    };
    let mut working = [0u32; 2 * KEEPER_COLUMNS];
    for entry in own.iter() {
        if let Some(k) = column(records, entry) {
            working[k] += 1;
        }
    }
    for (flag, k) in HALVES {
        if FLAGS[unit] & flag != 0 {
            working[k] /= 2;
        }
    }
    for entry in own.iter_mut() {
        let Some(k) = column(records, entry) else { continue };
        let ability = entry.ability as usize;
        let good = records.player(entry.id).map_or(0, |p| p.good());
        if good & (1 << ability) != 0 {
            entry.progress[ability] = LEARNT;
            continue;
        }
        // Past the row's 25 a stale keeper's ability reads the next
        // row's bytes; none is taught there.
        let n = working[k];
        let step = match teaches.get(k).copied().unwrap_or(0) {
            1 if random.below(n) == 0 && random.below(100) < 50 => 1,
            2 if random.below(n) == 0 => 1,
            3 if random.below(n / 2) == 0 => 2,
            _ => 0,
        };
        if step == 0 {
            continue;
        }
        entry.progress[ability] = entry.progress[ability].wrapping_add(step);
        if entry.progress[ability] > 99 {
            learnt(records, entry, ability, human, tables);
            entry.ability = NOTHING;
        }
    }
}

/// A man learnt the ability he worked on (`FUN_00747246`). A weakness
/// goes, from nought again; a new one he has at a hundred — with three
/// already, good and bad together, the good one of least difficulty
/// below 5 goes back to 80, or failing one the first such weakness goes.
/// Both are booked as gained (`FUN_00643633`); one he had already only
/// stands at a hundred. A match can bring him here working on nothing
/// ([`match_lesson`]): the original tests the sixteenth bit of the words
/// sign-extended — his fifteenth's — sets none, and writes the progress
/// into the low byte of his entry's ability, which its caller sets to
/// nothing straight after.
fn learnt(records: &mut Records, entry: &mut Entry, ability: usize, human: u8, tables: &IndividualTables) {
    let Some(p) = records.player_mut(entry.id) else { return };
    let keeper = p.get::<u8>(0x21d) == 1;
    let shift = ability.min(31);
    let bit = (1u32 << shift) as u16;
    let has = |word: u16| i32::from(word as i16) >> shift & 1 != 0;
    let (mut good, mut bad) = (p.good(), p.bad());
    let put = |entry: &mut Entry, value: i8| {
        if let Some(got) = entry.progress.get_mut(ability) {
            *got = value;
        }
    };
    if has(bad) {
        p.set_bad(bad & !bit);
        put(entry, 0);
    } else if !has(good) {
        let span = individual::abilities(keeper);
        let count = (0..span).filter(|&c| good >> c & 1 != 0).count()
            + (0..span).filter(|&c| bad >> c & 1 != 0).count();
        if count == 3 {
            let mut easiest = 5;
            let mut drop = None;
            for c in 0..span {
                let difficulty = tables.difficulty(keeper, c);
                if good >> c & 1 != 0 && difficulty < easiest {
                    easiest = difficulty;
                    drop = Some(c);
                }
            }
            match drop {
                Some(c) => {
                    entry.progress[c] = 80;
                    good &= !(1 << c);
                }
                None => {
                    if let Some(c) = (0..span).find(|&c| bad >> c & 1 != 0 && tables.difficulty(keeper, c) < easiest) {
                        bad &= !(1 << c);
                        p.set_bad(bad);
                    }
                }
            }
        }
        put(entry, LEARNT);
        p.set_good(good | bit);
    } else {
        entry.ability = NOTHING;
        put(entry, LEARNT);
        return;
    }
    records.ability_gained(human, entry.id, ability as u16);
}

/// The field abilities a match teaches — Kopfball, Schußkraft,
/// Torinstinkt, Spielmacher — and the keeper's Elfmetertöter.
const HEADER: usize = 0;
const SHOT_POWER: usize = 3;
const INSTINCT: usize = 7;
const PLAYMAKER: usize = 11;
const PENALTY_KILLER: usize = 0;
/// How far a goal or an assist brings him, and a saved penalty.
const GOAL_LESSON: i8 = 3;
const SAVE_LESSON: i8 = 10;
/// The keeper's position byte (+0x3), and the midfield's
/// (`FUN_00677ca7`: 6 to 9).
const KEEPER: u8 = 1;
const MIDFIELD: std::ops::RangeInclusive<u8> = 6..=9;
/// The kinds of a goal that teach their own ability, as the scorer's
/// books name them (`FUN_00740aad`'s first place): headed and struck.
const HEADED: i16 = 2;
const STRUCK: i16 = 3;

/// What a match teaches one man (`FUN_00747855`): at a club a human
/// trains or manages (the club object's +0x8a or +0x8b, the gate of its
/// callers) out of a slot 1 to 4 (+0x11de), a man lacking the ability
/// gets `by` more of it in his entry of the slot's store (`FUN_00746bde`;
/// a man without one gets nothing). Past 99 he has learnt — not that
/// ability but the one his entry works on (`FUN_00747246` with the
/// entry's +0x12) — and works on nothing; the match's ability stays
/// above 99 unlearnt. The entry goes back (`FUN_00746997`).
pub fn match_lesson(
    records: &mut Records,
    tables: &IndividualTables,
    (country, number): (u16, u16),
    id: u16,
    ability: usize,
    by: i8,
) {
    let Some(team) = records.team(country, number) else { return };
    let human = team.human();
    if team.humans() == [0, 0] || !(1..=crate::human_day::HUMANS).contains(&usize::from(human)) {
        return;
    }
    let slot = usize::from(human) - 1;
    if records.player(id).is_none_or(|p| p.good() >> ability & 1 != 0) {
        return;
    }
    let Some(mut entry) =
        records.individual.get(slot).and_then(|s| s.entries().into_iter().find(|e| e.id == id))
    else {
        return;
    };
    entry.progress[ability] = entry.progress[ability].wrapping_add(by);
    if entry.progress[ability] > 99 {
        let working = usize::try_from(entry.ability).unwrap_or(usize::MAX);
        learnt(records, &mut entry, working, human, tables);
        entry.ability = NOTHING;
    }
    if let Some(store) = records.individual.get_mut(slot) {
        let mut entries = store.entries();
        individual::keep(&mut entries, entry);
        store.set_entries(&entries);
    }
}

/// A goal's lessons for its scorer, a field player (`FUN_00740aad`): a
/// headed one teaches him Kopfball (`FUN_006a76df`), a struck one
/// Schußkraft (`FUN_006a7737`), and every one Torinstinkt
/// (`FUN_006a778f`), three each. `club` is the scoring side's.
pub fn goal_lessons(records: &mut Records, tables: &IndividualTables, club: (u16, u16), id: u16, kind: i16) {
    if records.player(id).is_none_or(|p| p.positions()[0] == KEEPER) {
        return;
    }
    match kind {
        HEADED => match_lesson(records, tables, club, id, HEADER, GOAL_LESSON),
        STRUCK => match_lesson(records, tables, club, id, SHOT_POWER, GOAL_LESSON),
        _ => {}
    }
    match_lesson(records, tables, club, id, INSTINCT, GOAL_LESSON);
}

/// An assist teaches a midfielder Spielmacher, three (`FUN_00740dc4` →
/// `FUN_006a77ea`).
pub fn assist_lesson(records: &mut Records, tables: &IndividualTables, club: (u16, u16), id: u16) {
    if records.player(id).is_some_and(|p| MIDFIELD.contains(&p.positions()[0])) {
        match_lesson(records, tables, club, id, PLAYMAKER, GOAL_LESSON);
    }
}

/// A saved penalty teaches the keeper Elfmetertöter, ten (`FUN_00741038`
/// → `FUN_006a784b`). The original teaches only while his match byte
/// +0x65, lifted by the save, stood from nought to 99; it has no save
/// copy, so the replica takes it to stand there.
pub fn save_lesson(records: &mut Records, tables: &IndividualTables, club: (u16, u16), id: u16) {
    if records.player(id).is_some_and(|p| p.positions()[0] == KEEPER) {
        match_lesson(records, tables, club, id, PENALTY_KILLER, SAVE_LESSON);
    }
}

/// The training's event of the day (`FUN_0060c1c0` by its pick out of
/// [`EVENTS`], 0 for none): whether one was met; the texts of 100, 101
/// and 110 go to `posted`, those of 105 to 107 not yet.
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
    posted: &mut Vec<Posted>,
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
                // A light or a heavy stomach flu (texts 718, 719).
                let text = if event == 100 { 0x2ce } else { 0x2cf };
                posted.push(Posted::plain((country, number), ILLNESS_EVENT, text));
            }
            true
        }
        102 => player_event(records, (country, number), trainable, tables, unit, day_units, (today, date), recent, posted, random),
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
            let coach = records.team(country, number).map_or(0, |t| t.second_coach());
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
                // The text goes out even for a man out longer already.
                if let Some((kind, days)) = out
                    && 5 < p.get::<u8>(0x27c)
                {
                    let text = if kind == 0x41 { 0x3b5 } else { 0x3b7 };
                    if days_out(records, p) < 0xb4 {
                        injure(records, id, kind, days, today);
                    }
                    posted.push(Posted { club: (country, number), code: INJURY_EVENT, text, fill: Fill::Named(id) });
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
    let days = cared_days(spent, medical.base_days(kind), random);
    injure(records, id, kind, days, today);
    knee_knock(records, id, kind, date, random);
}

/// An injury's days by what the club spends on its doctors (the team's
/// +0x314, the club's +0x2ac): a hundred less three per medical level
/// (`FUN_006773c4`) and a draw below a day's cost in the spending, times
/// the kind's base days, over a hundred; one day at least — as
/// `FUN_00767225` and the match's `FUN_00766eb9` reckon them.
pub(crate) fn cared_days(spent: i32, base: u8, random: &mut Random) -> i32 {
    let level = MEDICAL_LEVELS.iter().filter(|&&cap| cap <= spent).count() as i64;
    let care = i64::from(random.below((spent / MEDICAL_DAY).max(0) as u32));
    let base = i64::from(base);
    // The factor goes to the FPU as a 64-bit integer whose high word is
    // nought — below nought it is read as 2^32 more — and the days come
    // back through `ftol` into a short (past i32, its 0x80000000).
    let factor = i64::from((100 - 3 * level - care) as i32 as u32);
    let whole = factor * base / 100;
    let whole = i32::try_from(whole).unwrap_or(i32::MIN);
    i32::from((whole as i16).max(1))
}

/// Kinds 0x1b and 0x1e may leave him knocked about by the knee
/// (`FUN_007675c6`): under 180 (200 without either trait bit 1) less
/// nine a year he is under forty, of two hundred.
pub(crate) fn knee_knock(records: &mut Records, id: u16, kind: u8, date: NaiveDate, random: &mut Random) {
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
pub(crate) fn move_likings(records: &mut Records, (country, number): (u16, u16), by: [i8; 3]) {
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
pub(crate) fn injure(records: &mut Records, id: u16, kind: u8, days: i32, today: i32) {
    if let Some(p) = records.player_mut(id) {
        p.set_injury_kind(kind);
        p.set_injured_on(today as u16);
    }
    set_days_out(records, id, days);
}

/// Knocked about for `days` (`FUN_00766af9`, the handle's +0x2f, which
/// the record keeps as the day it lasts to, +0x1de).
pub(crate) fn knock_for(records: &mut Records, id: u16, days: u16, today: i32) {
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
/// list, or when he is a playing manager or, under the flag every event
/// hands it, stands away from the club ([`away`]).
fn draw_any(records: &Records, list: &[u16], random: &mut Random) -> Option<u16> {
    if list.is_empty() {
        return None;
    }
    let id = list[random.below(list.len() as u32) as usize];
    (id <= crate::mood::LAST_REAL_ID && !away(records, id)).then_some(id)
}

/// Whether a man stands away from the club: the handle's +0x6a, which
/// the load copy builds from the record's `away_to`, and which the
/// draws of a man check under their flag (`FUN_005eb193`, `FUN_005ead2f`,
/// `FUN_005eaed7`, `FUN_005eb0ab`: `psVar[0x35] != 0`).
fn away(records: &Records, id: u16) -> bool {
    records.player(id).is_some_and(|p| p.away_to()[0] != 0)
}

/// Event 105 (`FUN_0060c1c0`'s case 5): a trained man with any first
/// ability bit (`good`, the handle's +0xc; of nine for a keeper, else
/// sixteen) has one of [`ABILITY_BITS`] drawn — a keeper's of the six,
/// the others' of the ten; with three bits of both words set, up to five
/// draws for one he has. One he has goes from both words
/// (`FUN_00747689`), one he has in neither moves into the second
/// (`bad`, +0xe; `FUN_00747770`). Both log the loss into the human
/// club's book of the week's abilities (`FUN_00643673`'s gate into
/// `Records::ability_lost`), and for a human slot of one to four his
/// entry in the club's store of the men's own training (`0xa50028` +
/// (slot − 1) × 0x7a0, `FUN_00746bde`, `FUN_00746997`) has what he got
/// of that ability put to nought; a man without one there gets none.
/// The day's run (`FUN_00749508`) puts its snapshot back over the store
/// at its end (`FUN_00746d33`), so on a trained day the write stands only
/// until then; the day of a manager away (`FUN_00748f55`) puts none back.
fn ability_event(
    records: &mut Records,
    (country, number): (u16, u16),
    trainable: &[u16],
    random: &mut Random,
) -> bool {
    let Some(id) = draw_any(records, trainable, random) else { return false };
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
        forget_progress(records, slot, id, usize::from(ABILITY_BITS[at]));
    }
    lost
}

/// What a man got of an ability, put to nought in his entry of the human
/// club's store (`FUN_00747689` and `FUN_00747770` after their gate
/// `FUN_00643673`: the slot byte +0x11de from one to four).
fn forget_progress(records: &mut Records, slot: u8, id: u16, ability: usize) {
    if !(1..=4).contains(&slot) {
        return;
    }
    let Some(store) = records.individual.get_mut(usize::from(slot) - 1) else { return };
    let mut entries = store.entries();
    let Some(mut entry) = entries.iter().copied().find(|e| e.id == id) else { return };
    if let Some(got) = entry.progress.get_mut(ability) {
        *got = 0;
    }
    individual::keep(&mut entries, entry);
    store.set_entries(&entries);
}

/// Events 106 and 107 (`FUN_0060c1c0`'s cases 6 and 7): a trained man,
/// and a roll over three for a keeper, else over the squad's size: 0
/// draws a bit of the first trait word (+0x10, the record's `traits[0]`;
/// of five for a keeper, else ten), 1 one of the second (+0x12, of
/// six), anything else is no event. 106 gives the bit to a man without
/// any of that word yet (`FUN_0074b4cf`'s sixteen, `FUN_0074b526`'s
/// eight), 107 takes it from a man who has it.
fn trait_event(records: &mut Records, (country, number): (u16, u16), trainable: &[u16], give: bool, random: &mut Random) -> bool {
    let Some(id) = draw_any(records, trainable, random) else { return false };
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
    posted: &mut Vec<Posted>,
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
        if run_player_event(records, club, trainable, tables, code, unit, day_units, day, posted, random) {
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
/// it took place. Its text goes to `posted` where the original builds it
/// of its texts and names alone ([`crate::event_text`]; `FUN_005f1e86`,
/// `FUN_005ee5c0`, `FUN_005ee619`, none of them drawing), those with a
/// value post the number it changed beside the text ([`Fill::Value`],
/// `FUN_00601043`), and the four with a second part (`FUN_005f2114`,
/// [`Fill::Headbutt`], [`Fill::Bomb`] and [`Fill::NamedThen`]) post the
/// first half with the second in tow.
/// Here are:
/// 4 and 5, the team's mood ten down or up (`FUN_007147ce`); 8, with a
/// human manager standing below ten (`FUN_006a61e5`) and his record's
/// `unrest_count` (+0x180, the manager object's +0x94) at four or more:
/// over the squad in its order every man whom no injury keeps out (the
/// handle's +0x2c, [`days_out`]) is rolled at a tenth and, on its 0,
/// stands one percentage point less in condition (`FUN_005ee85a`,
/// `FUN_00411130` on the handle's +0x16, the record's condition in
/// hundredths, wrapped before it is held to its range); 10 and 11,
/// a keeper drawn and four or three less content with the team
/// (`FUN_00700a6d`), and 12 and 14, a man who keeps no goal drawn and
/// two or one more content, each telling the value in 0xd to 0xf and
/// 0x11 — 10 answering "not met" all the same; 31, two
/// friends in the training (case 0x1b): one man drawn, and when his best
/// friend (+0x204) trains too, both three more content; 20, the offside
/// trap (the club's +0xa0, the record's `offside`) twenty up where it
/// stands below 70, the human trainer's standing (`FUN_0040a1ec`,
/// `FUN_006a426b` and `FUN_006a61e5`) at eleven or more, and the human
/// slot's merch range showing itself in its marketing byte and the
/// fans' discount (+0xc89 and +0xc8c); 23, a keeper
/// drawn and his +0x2f set to 5 (`FUN_00766af9`); and, in a unit whose
/// row allows it ([`UNIT_GATES`], `FUN_00748f29`), 24, two men who keep
/// no goal drawn and a roll of four — both +0x2f at 1, or the first
/// injured (kind 0x1c for five days, kind 0x32 for seven;
/// `FUN_00766b92`), told in 0x22's two surnames with the roll's second
/// text behind it (0x26 names the second man while the first goes out);
/// 28, a keeper and a man drawn, and on a three-roll's 2 the keeper's
/// +0x2f at 5, the bomb of 0x2a with the keeper's save of 0x2c behind
/// it; 30, with a human trainer half the time a
/// man drawn, without one always, and the mood five up; 36, two men
/// drawn and the second's +0x2f at 1 unless he is out; 39 and 40, up to
/// three draws of a man over thirty (`FUN_005eb3b5`) for one with the
/// first ability bit 2 or 4, which he loses, told of in 0x3a and 0x3c
/// with 0x3b and 0x3d behind; 34, a man under 23 drawn
/// (`FUN_005eb2c0`) and one over 30, men standing away from the club
/// for none in either draw, the young one taking the reason 0x800 and
/// the two falling out (`FUN_00703df4`): a standing friendship undone,
/// each off the other's friends and onto the other's dislikes.
/// Ungated: 37, with a human
/// trainer and a co-trainer, three draws over the trained, each one more
/// content with the team; 42, with a human trainer in a unit the gate
/// shuts, the mood ten down; 43, the mood five up, answering "not met";
/// 13, any trained man drawn, at a morale of 106 or more two less
/// content with his distractions (`FUN_00701210`, the record's +0x294),
/// told in 0x10 with the value;
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
/// three or five for its news's wording (0x18 to 0x1a, 0x13 to 0x17),
/// told with the value;
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
/// With 20 the last of them, all 31 codes the table holds run here.
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
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    let trainer = records.team(country, number).map_or(0, |t| t.humans()[0]);
    let mood = |records: &mut Records, by: i32| {
        if let Some(team) = records.team_mut(country, number) {
            add_mood(team, by);
        }
    };
    let gate = UNIT_GATES.get(unit).is_some_and(|&g| g != 0);
    // Its text, `ereignis.saq`'s `text` with the men it names.
    let mut post = |text: u16, fill: Fill| {
        posted.push(Posted { club: (country, number), code: PLAYER_EVENT, text, fill });
    };
    let knock = |records: &mut Records, id: u16, days: u16| knock_for(records, id, days, today);
    // Draws a listed man, moves his contentment with the team by `by`
    // and answers with him and the value pair of `FUN_00601043`: the
    // part as the reasons leave it (`FUN_00700c42`) before, the byte as
    // the setter wrote it (`FUN_00700a6d` answers with the record's)
    // after.
    let content = |records: &mut Records, keepers: bool, by: i32, random: &mut Random| -> Option<(u16, Value)> {
        let id = draw_listed(records, trainable, keepers, random)?;
        let p = records.player_mut(id)?;
        let before = i16::try_from(crate::mood::parts(p)[1]).unwrap_or(0);
        crate::mood::change(p, 1, by);
        Some((id, Value { art: 1, before, after: i16::from(p.contentment()[1]) }))
    };
    match code {
        4 | 5 => {
            if let Some(team) = records.team_mut(country, number) {
                add_mood(team, if code == 4 { -10 } else { 10 });
            }
            post(if code == 4 { 7 } else { 8 }, Fill::Plain);
            true
        }
        8 => {
            if trainer == 0 {
                return false;
            }
            let met = records
                .managers
                .get(usize::from(trainer) - 1)
                .is_some_and(|m| m.standing() < 10 && m.unrest_count() >= 4);
            if !met {
                return false;
            }
            post(0xb, Fill::Plain);
            // FUN_005ee85a with -1 over the club's player array
            // (+0xb50, count +0xe2, the team's `players`): only the men
            // the handle sees fit (+0x2c, [days_out] below one) are
            // rolled at, and on the roll's 0 `FUN_00411130` takes a
            // hundredth off the handle's condition (+0x16, the record's
            // +0x1ce) — the word wraps first, the result then sits
            // within 0 and 10000.  Its text (0xb) went out first.
            let squad = records.team(country, number).map_or_else(Vec::new, |t| t.players());
            for id in squad {
                if !records.player(id).is_some_and(|p| days_out(records, p) < 1) {
                    continue;
                }
                if random.below(10) == 0
                    && let Some(p) = records.player_mut(id)
                {
                    let left = p.condition().wrapping_sub(100);
                    p.set_condition(left.clamp(0, 10_000));
                }
            }
            true
        }
        10 => {
            if let Some((id, value)) = content(records, true, -4, random) {
                post(0xd, Fill::Value(id, value));
            }
            false
        }
        11 => match content(records, true, -3, random) {
            Some((id, value)) => {
                post(0xe, Fill::Value(id, value));
                true
            }
            None => false,
        },
        12 => match content(records, false, 2, random) {
            Some((id, value)) => {
                post(0xf, Fill::Value(id, value));
                true
            }
            None => false,
        },
        14 => match content(records, false, 1, random) {
            Some((id, value)) => {
                post(0x11, Fill::Value(id, value));
                true
            }
            None => false,
        },
        20 => {
            // The offside trap rises: the club's byte at +0xa0 (the
            // record's `offside`) stands below 70; the merch range of
            // the human slot shows itself in the bytes +0xc89 and
            // +0xc8c (the marketing man's flag is a whole i32 at
            // 0xc88, so in practice the fans' discount's byte
            // decides); with a human trainer, `FUN_0040a1ec` reads his
            // standing (`FUN_006a426b`, `FUN_006a61e5`), and at eleven
            // or more the trap rises by twenty, with its text (0x1e).
            let Some(team) = records.team(country, number) else {
                return false;
            };
            let offside = team.offside();
            if offside >= 0x46 {
                return false;
            }
            if !crate::merchandise::Range::of(records, team)
                .is_some_and(|r| r.marketing_or_discount_set())
            {
                return false;
            }
            if trainer == 0
                || !records
                    .managers
                    .get(usize::from(trainer) - 1)
                    .is_some_and(|m| m.standing() >= 0xb)
            {
                return false;
            }
            post(0x1e, Fill::Plain);
            if let Some(team) = records.team_mut(country, number) {
                team.set_offside(offside.wrapping_add(0x14));
            }
            true
        }
        9 => {
            let top = records.team(country, number).is_some_and(|t| t.get::<u8>(0x12e) == 0);
            if trainer != 0 && top {
                post(0xc, Fill::Plain);
                move_likings(records, (country, number), [0, 0, -10]);
            }
            false
        }
        13 => {
            let Some(id) = draw_any(records, trainable, random) else { return false };
            let Some(p) = records.player_mut(id) else { return false };
            if p.morale() < 0x6a {
                return false;
            }
            // `FUN_00701328` reads the byte off the record, `FUN_00701210`
            // answers with the new one.
            let before = i16::from(p.contentment()[5]);
            crate::mood::change(p, 5, -2);
            let after = i16::from(p.contentment()[5]);
            post(0x10, Fill::Value(id, Value { art: 3, before, after }));
            true
        }
        15 => {
            let met = trainer != 0 && records.team(country, number).is_some_and(|t| i32::from(t.next_day()) != today + 1);
            if met {
                post(0x12, Fill::Plain);
            }
            met
        }
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
            post(0x33, Fill::Plain);
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
                    let before = i16::from(p.contentment()[5]);
                    crate::mood::change(p, 5, -1);
                    let after = i16::from(p.contentment()[5]);
                    let text = match random.below(3) { 0 => 0x18, 1 => 0x19, _ => 0x1a };
                    post(text, Fill::Value(id, Value { art: 3, before, after }));
                }
            } else {
                let before = i16::try_from(crate::mood::parts(p)[1]).unwrap_or(0);
                crate::mood::change(p, 1, 1);
                let after = i16::from(p.contentment()[1]);
                let roll = u16::try_from(random.below(5)).unwrap_or(4);
                post(0x13 + roll, Fill::Value(id, Value { art: 1, before, after }));
            }
            true
        }
        23 => {
            let Some(keeper) = draw_listed(records, trainable, true, random) else { return false };
            post(0x21, Fill::Named(keeper));
            knock(records, keeper, 5);
            true
        }
        24 if gate => {
            let Some(one) = draw_listed(records, trainable, false, random) else { return false };
            let Some(other) = draw_listed(records, trainable, false, random) else { return false };
            if one == other {
                return false;
            }
            let (part, named) = match random.below(4) {
                0 => (0x23, 0),
                1 => {
                    knock(records, one, 1);
                    knock(records, other, 1);
                    (0x24, 0)
                }
                2 => {
                    injure(records, one, 0x1c, 5, today);
                    (0x25, one)
                }
                // The original names the second man's concussion and
                // still sends the first out for seven days.
                _ => {
                    injure(records, one, 0x32, 7, today);
                    (0x26, other)
                }
            };
            post(0x22, Fill::Headbutt(one, other, part, named));
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
            // holds at most one.  Its text (0x28) names him.
            if places[1] == 0 {
                places[1] = 10;
            } else {
                places[2] = 10;
            }
            if let Some(p) = records.player_mut(man) {
                p.set_positions(places);
            }
            post(0x28, Fill::Named(man));
            true
        }
        28 if gate => {
            let Some(keeper) = draw_listed(records, trainable, true, random) else { return false };
            let Some(other) = draw_listed(records, trainable, false, random) else { return false };
            if keeper == other {
                return false;
            }
            let tail = if random.below(3) < 2 {
                0x2d
            } else {
                knock(records, keeper, 5);
                0x2e
            };
            // 0x2a names the shooter, the keeper comes behind in 0x2c.
            post(0x2a, Fill::Bomb(keeper, other, tail));
            true
        }
        29 if gate => {
            if trainer == 0 || crate::stock::confidence(records, usize::from(trainer) - 1) < 0x4b {
                return false;
            }
            mood(records, 10);
            post(0x2f, Fill::Plain);
            true
        }
        30 if gate => {
            // Who took the shot in the face: one of the men, or the
            // trainer himself ("Sie haben", text 0x30) on a two-roll's 0.
            let lead = if trainer == 0 || random.below(2) != 0 {
                let Some(id) = draw_listed(records, trainable, false, random) else { return false };
                Lead::Player(id)
            } else {
                Lead::Text(0x30)
            };
            post(0x31, Fill::Lead(lead));
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
            post(0x34, Fill::Numbered(one, other));
            true
        }
        34 if gate => {
            let Some(young) = draw_aged(records, trainable, date, 0x17, true, random) else { return false };
            let Some(elder) = draw_aged(records, trainable, date, 0x1e, false, random) else { return false };
            if let Some(p) = records.player_mut(young) {
                tables.set_reasons(p, 0x800);
            }
            fall_out(records, young, elder);
            post(0x35, Fill::Numbered(young, elder));
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
            post(0x36, Fill::Numbered(one, other));
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
            post(0x37, Fill::Numbered(one, other));
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
            post(0x38, Fill::Plain);
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
            let Some(id) = (0..3).map(|_| draw_any(records, trainable, random)).find(|&id| worn(records, id)).flatten() else {
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
            post(0x39, Fill::Named(id));
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
            post(
                if code == 39 { 0x3a } else { 0x3c },
                Fill::NamedThen(id, if code == 39 { 0x3b } else { 0x3d }),
            );
            true
        }
        41 => {
            if trainer == 0 {
                return false;
            }
            post(0x3e, Fill::Plain);
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
            post(0x3f, Fill::Plain);
            mood(records, -10);
            true
        }
        43 => {
            post(0x6db, Fill::Plain);
            mood(records, 5);
            false
        }
        31 => {
            let Some(one) = draw_listed(records, trainable, false, random) else { return false };
            let friend = records.player(one).map_or(0, |p| p.best_friend());
            if friend == 0 || !trainable.contains(&friend) {
                return false;
            }
            post(0x32, Fill::Numbered(one, friend));
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
/// one drawn is a playing manager or, under the flag every event hands
/// them, stands away from the club ([`away`]).
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
    (id <= crate::mood::LAST_REAL_ID && !away(records, id)).then_some(id)
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
pub(crate) fn fall_out(records: &mut Records, young: u16, elder: u16) {
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
                            // `FUN_00745d3c(p, 2)`; the first place
                            // stands free under the gate, so the
                            // weighing of two never starts here.
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

    /// Event 105 on A's human squad, one man trained: away from the club
    /// he is not drawn and keeps all; at home he loses an ability, and
    /// his entry in the club's store has nothing left of it, the rest of
    /// what he got standing.
    #[test]
    #[ignore = "reads the oracle's kept saves beside $ANSTOSS3_DIR"]
    fn the_ability_event_skips_the_away_and_clears_the_plan() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let save = a3_files::an3::Save::read(&dir.parent().unwrap().join("saves-kept/A.an3")).unwrap();
        let mut records = Records::from_save(save).unwrap();
        let (country, number) = statistics::human_clubs(&records)[0];
        let club = (u16::from(country), number as u16);
        let slot = records.team(club.0, club.1).unwrap().human();
        assert!((1..=4).contains(&slot));
        let id = records
            .team(club.0, club.1)
            .unwrap()
            .players()
            .into_iter()
            .find(|&id| records.player(id).is_some_and(|p| p.get::<u8>(0x21d) != 1))
            .unwrap();
        let p = records.player_mut(id).unwrap();
        p.set_good(0xffff);
        p.set_bad(0);
        p.set_away_to([1, 0]);
        let mut entry = Entry::default();
        entry.id = id;
        entry.progress = [individual::LEARNT; FIELD_ABILITIES];
        let store = &mut records.individual[usize::from(slot) - 1];
        let mut entries = store.entries();
        entries.iter_mut().filter(|e| e.id == id).for_each(|e| e.id = 0);
        individual::keep(&mut entries, entry);
        store.set_entries(&entries);
        let own = |records: &Records| records.individual[usize::from(slot) - 1].entries().into_iter().find(|e| e.id == id).unwrap();
        let mut random = Random::from_state(12345, 67890);
        assert!(!ability_event(&mut records, club, &[id], &mut random));
        assert_eq!(records.player(id).unwrap().good(), 0xffff);
        assert_eq!(own(&records), entry);
        records.player_mut(id).unwrap().set_away_to([0, 0]);
        assert!(ability_event(&mut records, club, &[id], &mut random));
        let lost = 0xffff & !records.player(id).unwrap().good();
        assert_eq!(lost.count_ones(), 1);
        let bit = lost.trailing_zeros() as usize;
        entry.progress[bit] = 0;
        assert_eq!(own(&records), entry);
    }

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
            assert!(training_event(&mut records, club, &squad, &trainable, (&tables, &league_tables), event, 0, [0, 0, 0], (64, date), &mut recent, &mut Vec::new(), &mut random));
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
        assert!(training_event(&mut records, club, &squad, &trainable, (&tables, &league_tables), 108, 0, [0, 0, 0], (64, date), &mut recent, &mut Vec::new(), &mut random));
        assert_eq!(random, expected);
        // The player events' plainest: the team's mood ten down and up
        // again, and code 12's one draw over the outfield, one man two
        // more content with the team.
        let mood = i32::from(records.team(club.0, club.1).unwrap().mood());
        let mut posted = Vec::new();
        assert!(run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 4, 0, [0, 0, 0], (64, date), &mut posted, &mut random));
        let down = (mood - 10).max(0);
        assert_eq!(i32::from(records.team(club.0, club.1).unwrap().mood()), down);
        assert!(run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 5, 0, [0, 0, 0], (64, date), &mut posted, &mut random));
        assert_eq!(i32::from(records.team(club.0, club.1).unwrap().mood()), (down + 10).min(200));
        assert_eq!(random, expected, "the mood events draw nothing");
        // Each tells the club with a text of its own, the brawl and the
        // 4:4; code 12's post names the man drawn and stands his team
        // contentment's value beside the text.
        assert_eq!(posted, [Posted::plain(club, PLAYER_EVENT, 7), Posted::plain(club, PLAYER_EVENT, 8)]);
        let outfield = trainable.iter().filter(|&&id| records.player(id).unwrap().get::<u8>(0x21d) != 1).count();
        let team_parts = |records: &Records| -> i32 {
            trainable.iter().map(|&id| i32::from(records.player(id).unwrap().contentment()[1])).sum()
        };
        let before = team_parts(&records);
        assert!(run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 12, 0, [0, 0, 0], (64, date), &mut posted, &mut random));
        expected.below(outfield as u32);
        assert_eq!(random, expected);
        assert!((0..=2).contains(&(team_parts(&records) - before)));
        assert_eq!(posted.len(), 3);
        let Posted { code, text, fill, .. } = posted[2];
        assert_eq!((code, text), (PLAYER_EVENT, 0xf));
        let Fill::Value(id, value) = fill else { panic!("code 12 posts the man and his value") };
        assert_eq!((value.art, value.after), (1, i16::from(records.player(id).unwrap().contentment()[1])), "the window shows what the record now holds");
        // Code 24 in a unit whose row shuts it (unit 1) is no event and
        // draws nothing; code 23 draws a keeper and sets his +0x2f to 5.
        let held = random.clone();
        assert!(!run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 24, 1, [0, 0, 0], (64, date), &mut Vec::new(), &mut random));
        assert_eq!(random, held);
        let keepers: Vec<u16> =
            trainable.iter().copied().filter(|&id| records.player(id).unwrap().get::<u8>(0x21d) == 1).collect();
        if !keepers.is_empty() {
            assert!(run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 23, 0, [0, 0, 0], (64, date), &mut Vec::new(), &mut random));
            assert!(keepers.iter().any(|&id| records.player(id).unwrap().get::<u16>(0x1de) == 64 + 5));
        }
        // Code 9 draws nothing, answers "not met", and costs a human
        // trainer in the top division ten of the media's liking.
        let held = random.clone();
        let team = records.team(club.0, club.1).unwrap();
        let (trainer, top) = (team.humans()[0], team.get::<u8>(0x12e) == 0);
        let media = |records: &Records| records.managers[usize::from(trainer).max(1) - 1].popularity()[2];
        let before = media(&records);
        assert!(!run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 9, 0, [0, 0, 0], (64, date), &mut Vec::new(), &mut random));
        let lost = if trainer != 0 && top { before.min(10) } else { 0 };
        assert_eq!(media(&records), before - lost);
        assert_eq!(random, held);
        // Code 29 in an open unit (0): met as the board's confidence in a
        // human trainer stands at 75 or more, the mood then ten up.
        let backed = trainer != 0 && crate::stock::confidence(&records, usize::from(trainer).max(1) - 1) >= 0x4b;
        let mood_before = i32::from(records.team(club.0, club.1).unwrap().mood());
        assert_eq!(run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 29, 0, [0, 0, 0], (64, date), &mut Vec::new(), &mut random), backed);
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
        assert_eq!(run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 41, 0, [0, 0, 0], (64, date), &mut Vec::new(), &mut random), trainer != 0);
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
        assert!(training_event(&mut records, club, &squad, &trainable, (&tables, &league_tables), 110, 0, [0, 0, 0], (64, date), &mut recent, &mut Vec::new(), &mut random));
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
        assert!(!run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 35, 0, [0, 0, 0], (64, date), &mut Vec::new(), &mut random));
        assert_eq!(random, held);
        // Code 38 outside its hard units neither draws nor takes place.
        let held = random.clone();
        assert!(!run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 38, 0, [0, 0, 0], (64, date), &mut Vec::new(), &mut random));
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
        assert!(!run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 34, 0, [0, 0, 0], (64, date), &mut Vec::new(), &mut random));
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
        assert!(!run_player_event(&mut records, club, &one, (&tables, &league_tables), 34, 0, [0, 0, 0], (64, date), &mut Vec::new(), &mut random));
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
        assert!(run_player_event(&mut records, club, &real, (&tables, &league_tables), 34, 0, [0, 0, 0], (64, date), &mut Vec::new(), &mut random));
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
        assert!(run_player_event(&mut records, club, &real, (&tables, &league_tables), 34, 0, [0, 0, 0], (64, date), &mut Vec::new(), &mut random));
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
        assert_eq!(run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 15, 0, [0, 0, 0], (64, date), &mut Vec::new(), &mut random), expected_15);
        assert_eq!(random, held);
        // Code 43 lifts the mood five and answers "not met"; 37 draws
        // three times with a human trainer and a co-trainer, else none.
        let mood = i32::from(records.team(club.0, club.1).unwrap().mood());
        assert!(!run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 43, 0, [0, 0, 0], (64, date), &mut Vec::new(), &mut random));
        assert_eq!(i32::from(records.team(club.0, club.1).unwrap().mood()), (mood + 5).min(200));
        let team = records.team(club.0, club.1).unwrap();
        let staffed = team.humans()[0] != 0 && team.co_trainer() != 0;
        let mut thrice = random.clone();
        if staffed {
            for _ in 0..3 {
                thrice.below(trainable.len() as u32);
            }
        }
        assert_eq!(run_player_event(&mut records, club, &trainable, (&tables, &league_tables), 37, 0, [0, 0, 0], (64, date), &mut Vec::new(), &mut random), staffed);
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
                training_event(&mut records, club, &squad, &trainable, (&tables, &league_tables), event, 0, [0, 0, 0], (64, date), &mut recent, &mut Vec::new(), &mut random);
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
        assert!(!run_player_event(&mut records, club, &squad, (&tables, &league_tables), 26, 0, [11, 11, 11], (64, date), &mut Vec::new(), &mut random));
        assert_eq!(random, held, "two free planned units shut the event");
        // A unit letting it, but no trained defender in the list: the
        // pool stands empty and the six draws roll nothing at all.
        let none: [u16; 0] = [];
        let held = random.clone();
        assert!(!run_player_event(&mut records, club, &none, (&tables, &league_tables), 26, 0, [0, 11, 11], (64, date), &mut Vec::new(), &mut random));
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
        assert!(!run_player_event(&mut records, club, &defs, (&tables, &league_tables), 26, 0, [0, 11, 11], (64, date), &mut Vec::new(), &mut random));
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
        assert!(!run_player_event(&mut records, club, &defs, (&tables, &league_tables), 26, 0, [0, 11, 11], (64, date), &mut Vec::new(), &mut random));
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
        assert!(run_player_event(&mut records, club, &defs, (&tables, &league_tables), 26, 0, [0, 11, 11], (64, date), &mut Vec::new(), &mut random));
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
        assert!(run_player_event(&mut records, club, &defs, (&tables, &league_tables), 26, 0, [0, 11, 11], (64, date), &mut Vec::new(), &mut random));
        assert_eq!(random, expected, "the away man's draw, then the fitting one's");
        assert_eq!(records.player(man).unwrap().positions(), [state[defs.iter().position(|&y| y == man).unwrap()].1[0], 10, 0]);
        assert_eq!(records.player(away).unwrap().positions(), away_places, "the away man keeps his places");
    }

    /// Code 8 on A: the event opens on a human manager standing below
    /// ten ([`Manager::standing`]) with his record's `unrest_count` at
    /// four or more, and closes without a roll below either.  Open, the
    /// walk runs over the squad in its order and rolls at a tenth every
    /// man whom no injury keeps out ([`days_out`], the handle's +0x2c),
    /// costing each of the rolls' 0 one percentage point of condition
    /// (`FUN_005ee85a`, `FUN_00411130` in the hundredths word, held at
    /// nought); the injured man stands out of the rolls.
    #[test]
    #[ignore = "reads the oracle's kept saves beside $ANSTOSS3_DIR"]
    fn the_healthy_event_rolls_over_the_fit_squad() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let save = a3_files::an3::Save::read(&dir.parent().unwrap().join("saves-kept/A.an3")).unwrap();
        let mut records = Records::from_save(save).unwrap();
        let (country, number) = statistics::human_clubs(&records)[0];
        let club = (u16::from(country), number as u16);
        let squad = records.team(club.0, club.1).unwrap().players();
        let trainer = records.team(club.0, club.1).unwrap().humans()[0];
        assert_ne!(trainer, 0, "A's club stands under its human");
        let slot = usize::from(trainer) - 1;
        let date = NaiveDate::from_ymd_opt(2025, 9, 9).unwrap();
        let exe = a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap();
        let (tables, league_tables) = (MoodTables::load(&exe).unwrap(), LeagueTables::load(&exe).unwrap());
        let mut random = Random::from_state(24681, 13579);
        // The gates: the count below four, and the manager at ten
        // standing (level fifteen, the standing bits off) with the count
        // at five — both shut the event without a roll.
        records.managers.get_mut(slot).unwrap().set_unrest_count(0);
        let held = random.clone();
        assert!(!run_player_event(&mut records, club, &squad, (&tables, &league_tables), 8, 0, [0, 0, 0], (64, date), &mut Vec::new(), &mut random));
        assert_eq!(random, held, "the count below four shuts the event");
        {
            let m = records.managers.get_mut(slot).unwrap();
            m.set_unrest_count(5);
            m.set_level(15);
            m.set_level_bits(0);
        }
        let held = random.clone();
        assert!(!run_player_event(&mut records, club, &squad, (&tables, &league_tables), 8, 0, [0, 0, 0], (64, date), &mut Vec::new(), &mut random));
        assert_eq!(random, held, "the manager at ten standing shuts it");
        // Open: standing nought, count four; the first fit man stands
        // injured to see him out of the rolls, his condition held.
        {
            let m = records.managers.get_mut(slot).unwrap();
            m.set_level(0);
            m.set_level_bits(0);
            m.set_unrest_count(4);
        }
        let hurt = squad
            .iter()
            .copied()
            .find(|&id| records.player(id).is_some_and(|p| days_out(&records, p) < 1))
            .expect("A has a fit man");
        let held_hurt = records.player(hurt).unwrap().condition();
        set_days_out(&mut records, hurt, 5);
        let men: Vec<(u16, i16, bool)> = squad
            .iter()
            .map(|&id| {
                let fit = records.player(id).is_some_and(|p| days_out(&records, p) < 1);
                (id, records.player(id).map_or(0, |p| p.condition()), fit)
            })
            .collect();
        let mut probe = random.clone();
        let hits: Vec<bool> = men.iter().map(|&(_, _, fit)| fit && probe.below(10) == 0).collect();
        let mut want: Vec<i16> = men
            .iter()
            .zip(&hits)
            .map(|(&(_, cond, _), &hit)| if hit { cond.wrapping_sub(100).clamp(0, 10_000) } else { cond })
            .collect();
        // One man stands at fifty hundredths to see the hundredths word
        // hold at nought rather than fall under it.
        let low = men.iter().zip(&hits).position(|(&(_, _, fit), &hit)| fit && hit);
        assert!(low.is_some(), "the fixture must see a roll land");
        if let Some(i) = low {
            records.player_mut(men[i].0).unwrap().set_condition(50);
            want[i] = 0;
        }
        assert!(run_player_event(&mut records, club, &squad, (&tables, &league_tables), 8, 0, [0, 0, 0], (64, date), &mut Vec::new(), &mut random));
        assert_eq!(random, probe, "one roll at a tenth over every fit man, in the squad's order");
        for ((id, _, _), &want) in men.iter().zip(&want) {
            assert_eq!(records.player(*id).map_or(0, |p| p.condition()), want, "man {id}");
        }
        assert_eq!(records.player(hurt).unwrap().condition(), held_hurt, "the injured man stands out of the rolls");
    }

    /// The two-part events (`FUN_005f2114`) post their first half with
    /// the second in tow: code 24's head-duel of 0x22 names both drawn
    /// men and sends one of 0x23 to 0x26 behind — 0x26 naming the second
    /// man while the first goes out —, code 28's bomb of 0x2a names the
    /// shooter with the keeper's save of 0x2c and 0x2d or 0x2e behind,
    /// and 39 and 40 tell of an over-thirty man in 0x3a and 0x3c with
    /// 0x3b and 0x3d behind.
    #[test]
    #[ignore = "reads the oracle's kept saves beside $ANSTOSS3_DIR"]
    fn the_two_part_events_post_their_second_half() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let save = a3_files::an3::Save::read(&dir.parent().unwrap().join("saves-kept/A.an3")).unwrap();
        let mut records = Records::from_save(save).unwrap();
        let (country, number) = statistics::human_clubs(&records)[0];
        let club = (u16::from(country), number as u16);
        let squad = records.team(club.0, club.1).unwrap().players();
        let trainable: Vec<u16> =
            squad.iter().copied().filter(|&id| records.player(id).is_some_and(|p| days_out(&records, p) < 7)).collect();
        let exe = a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap();
        let (tables, league_tables) = (MoodTables::load(&exe).unwrap(), LeagueTables::load(&exe).unwrap());
        let date = NaiveDate::from_ymd_opt(2025, 9, 9).unwrap();
        let mut posted = Vec::new();
        // A seed that lands the event (unit 0, whose row opens it) —
        // a lost draw or two men equal leave the records untouched.
        let mut seed = 6424_i64;
        let event = |records: &mut Records, code: u32, seed: i64, posted: &mut Vec<Posted>| {
            let mut random = Random::from_state(seed, seed ^ 0x5ad9);
            posted.clear();
            run_player_event(records, club, &trainable, (&tables, &league_tables), code, 0, [0, 0, 0], (64, date), posted, &mut random)
        };
        loop {
            if event(&mut records, 24, seed, &mut posted) {
                break;
            }
            seed += 1;
            assert!(seed < 6500, "code 24 never came off");
        }
        let [post] = posted.as_slice() else { panic!("{posted:?}") };
        assert_eq!((post.club, post.code, post.text), (club, PLAYER_EVENT, 0x22));
        let Fill::Headbutt(one, other, part, named) = post.fill else { panic!("{:?}", post.fill) };
        assert_ne!(one, other);
        assert!([0x23, 0x24, 0x25, 0x26].contains(&part));
        assert_eq!(named, if part == 0x25 { one } else if part == 0x26 { other } else { 0 });
        assert!(event(&mut records, 28, seed, &mut posted), "seed {seed} draws the bomb");
        let [post] = posted.as_slice() else { panic!("{posted:?}") };
        assert_eq!((post.club, post.code, post.text), (club, PLAYER_EVENT, 0x2a));
        let Fill::Bomb(keeper, shooter, tail) = post.fill else { panic!("{:?}", post.fill) };
        assert_ne!(keeper, shooter);
        assert!([0x2d, 0x2e].contains(&tail));
        // Codes 39 and 40: over-thirty men, all of them with the bits.
        let elders: Vec<u16> = trainable
            .iter()
            .copied()
            .filter(|&id| records.player(id).is_some_and(|p| p.get::<u8>(0x21d) != 1 && 30 < statistics::age_on(p, date)))
            .collect();
        assert!(!elders.is_empty(), "A trains over-thirty men");
        for &id in &elders {
            if let Some(p) = records.player_mut(id) {
                p.set_good(p.good() | 6);
            }
        }
        for (code, first, second, bit) in [(39u32, 0x3a_u16, 0x3b_u16, 2u16), (40, 0x3c, 0x3d, 4)] {
            assert!(event(&mut records, code, seed, &mut posted), "seed {seed} finds a man");
            let [post] = posted.as_slice() else { panic!("{posted:?}") };
            assert_eq!((post.club, post.code, post.text), (club, PLAYER_EVENT, first));
            let Fill::NamedThen(who, tail) = post.fill else { panic!("{:?}", post.fill) };
            assert!(elders.contains(&who));
            assert_eq!(tail, second);
            assert_eq!(records.player(who).unwrap().good() & bit, 0, "bit {bit} of man {who}");
        }
    }

    /// Event 102's code 20: the offside trap — the club's byte +0xa0,
    /// the record's `offside` — rises by twenty where it stands below
    /// 70, the human slot's merch range shows its marketing byte or the
    /// fans' discount, and the human trainer stands at eleven (`FUN_0040a1ec`,
    /// `FUN_006a61e5` → Manager::standing).  Nothing is drawn.
    #[test]
    #[ignore = "needs the game (ANSTOSS3_DIR)"]
    fn the_offside_trap_rises_at_a_running_range() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let save = a3_files::an3::Save::read(&dir.parent().unwrap().join("saves-kept/A.an3")).unwrap();
        let mut records = Records::from_save(save).unwrap();
        let (country, number) = statistics::human_clubs(&records)[0];
        let club = (u16::from(country), number as u16);
        let squad = records.team(club.0, club.1).unwrap().players();
        let (trainer, range_slot) = {
            let team = records.team(club.0, club.1).unwrap();
            (usize::from(team.humans()[0]), usize::from(team.human()))
        };
        assert_ne!(trainer, 0, "A's club stands under its human");
        assert!((1..=4).contains(&range_slot), "A's club holds a merch range");
        let range_slot = range_slot - 1;
        let date = NaiveDate::from_ymd_opt(2025, 9, 9).unwrap();
        let exe = a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap();
        let (tables, league_tables) = (MoodTables::load(&exe).unwrap(), LeagueTables::load(&exe).unwrap());
        let run = |records: &mut Records, random: &mut Random| {
            run_player_event(records, club, &squad, (&tables, &league_tables), 20, 0, [0, 0, 0], (64, date), &mut Vec::new(), random)
        };
        let mut random = Random::from_state(864213, 97531);
        // The range, shut and open: the marketing man's flag is a whole
        // i32 at 0xc88, so clearing it also clears the byte the event
        // asks at 0xc89; the fans' discount's low byte stands in for it.
        let mut range = crate::merchandise::Range::of(&records, records.team(club.0, club.1).unwrap()).expect("A's club holds a merch range");
        let open = |range: &mut crate::merchandise::Range| {
            range.set_marketing(false);
            range.set_fans_discount(10);
        };
        records.managers.get_mut(trainer - 1).unwrap().set_level(15);
        records.managers.get_mut(trainer - 1).unwrap().set_level_bits(0);
        records.team_mut(club.0, club.1).unwrap().set_offside(60);
        let held = random.clone();
        range.set_marketing(false);
        range.set_fans_discount(0);
        range.write(&mut records, range_slot);
        assert!(!run(&mut records, &mut random), "the shut range holds the trap down");
        assert_eq!(random, held, "code 20 draws nothing");
        // Open range, standing fifteen: the trap rises twenty, and a
        // trap at 70 or above never rises again.
        open(&mut range);
        range.write(&mut records, range_slot);
        assert!(run(&mut records, &mut random));
        assert_eq!(records.team(club.0, club.1).unwrap().offside(), 80, "the trap of sixty stands at eighty");
        let held = random.clone();
        assert!(!run(&mut records, &mut random), "the trap at eighty is past the event's seventy");
        assert_eq!(random, held);
        assert_eq!(records.team(club.0, club.1).unwrap().offside(), 80);
        // Standing, at the event's eleven and below it.
        records.team_mut(club.0, club.1).unwrap().set_offside(60);
        records.managers.get_mut(trainer - 1).unwrap().set_level(11);
        assert!(run(&mut records, &mut random), "eleven standing carries the trap");
        assert_eq!(records.team(club.0, club.1).unwrap().offside(), 80);
        records.team_mut(club.0, club.1).unwrap().set_offside(60);
        records.managers.get_mut(trainer - 1).unwrap().set_level(10);
        let held = random.clone();
        assert!(!run(&mut records, &mut random), "ten standing does not");
        assert_eq!(random, held);
        assert_eq!(records.team(club.0, club.1).unwrap().offside(), 60);
    }

    /// A session's gains on one man (`FUN_0074ae97`), as the original's
    /// dumps of Probe1's training week have keeper 3205 (hard, morale
    /// 100, a co-trainer's 95 percent): Gymnastik and Trainingsspiel take
    /// 14 and 56 of the first freshness word and bring 14 and 56 of
    /// condition. Building up only the day's first unit counts, at its
    /// row's hundred percent; light halves the share, a tenth goes for
    /// bit 4 of the first trait word, the condition's half below 25
    /// percent fresh, a fitness unit's seven for the team's byte 2; the
    /// third unit doubles behind the men who took over 30; a
    /// player-manager's first slider at 1 takes 0.7, cut down.
    #[test]
    fn a_session_trains_by_the_units_row() {
        let man = |condition: i16, fresh: [i16; 3], morale: u8, intensity: u8| {
            let mut p = Player::new(&vec![0; Player::SIZE]).unwrap();
            p.set_condition(condition);
            p.set_freshness(fresh);
            p.set_morale(morale);
            p.set_intensity(intensity);
            p
        };
        let mut keeper = man(3596, [8987, 0, 0], 100, 4);
        gains(&mut keeper, 15, 1, 95, 0, 0, None);
        assert_eq!((keeper.freshness(), keeper.condition()), ([8973, -14, 0], 3610));
        gains(&mut keeper, 33, 2, 95, 0, -40, None);
        assert_eq!((keeper.freshness(), keeper.condition()), ([8917, -156, -71], 3666));

        let mut building = man(1500, [8000, 0, 0], 100, 1);
        gains(&mut building, 17, 1, 95, 0, 0, None);
        assert_eq!((building.freshness(), building.condition()), ([7890, -320, -180], 1700));
        gains(&mut building, 17, 2, 95, 0, 0, None);
        assert_eq!((building.freshness(), building.condition()), ([7890, -320, -180], 1700));

        let mut light = man(5000, [2000, 0, 0], 90, 2);
        light.set_traits([4, 0]);
        gains(&mut light, 6, 1, 100, 2, 0, None);
        assert_eq!((light.freshness(), light.condition()), ([1933, -180, -90], 5031));

        let first = |counter: u32, taken: i16, own_play: Option<u8>, unit: usize| {
            let mut p = man(5000, [5000, 0, 0], 100, 4);
            gains(&mut p, unit, counter, if own_play.is_some() { 95 } else { 100 }, 0, taken, own_play);
            p.freshness()[0]
        };
        assert_eq!((first(3, -40, None, 16), first(3, -20, None, 16), first(2, -40, None, 16)), (4790, 4895, 4895));
        assert_eq!((first(1, 0, Some(1), 15), first(1, 0, Some(2), 15)), (4991, 4986));
    }

    /// Learning what he worked on (`FUN_00747246`) on one of A's men: with
    /// Kopfball, Schußkraft and Technik already, Zweikampf takes the
    /// place of Schußkraft, the easiest, which goes back to 80; the book
    /// of the week has him gain it. A weakness goes, from nought; one he
    /// has only stands at a hundred, and he works on nothing.
    #[test]
    #[ignore = "reads the oracle's kept saves beside $ANSTOSS3_DIR"]
    fn learning_drops_the_easiest_of_three() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let tables = crate::data::Data::load(&dir).unwrap().rules.individual;
        let save = a3_files::an3::Save::read(&dir.parent().unwrap().join("saves-kept/A.an3")).unwrap();
        let mut records = Records::from_save(save).unwrap();
        let squad = records.team(14, 84).unwrap().players();
        let id = squad.into_iter().find(|&id| records.player(id).unwrap().get::<u8>(0x21d) > 1).unwrap();
        records.player_mut(id).unwrap().set_good(1 | 1 << 3 | 1 << 9);
        records.player_mut(id).unwrap().set_bad(0);
        let mut entry = Entry::new(id, false, [1 | 1 << 3 | 1 << 9, 0], 0, individual::NORMAL);
        entry.ability = 1;
        entry.progress[1] = 100;
        learnt(&mut records, &mut entry, 1, 1, &tables);
        let p = records.player(id).unwrap();
        assert_eq!((p.good(), p.bad()), (1 | 1 << 1 | 1 << 9, 0));
        assert_eq!((entry.progress[1], entry.progress[3]), (LEARNT, 80));
        assert!(records.abilities_week(1).iter().any(|e| e.id == id && e.gained == 1 << 1));

        records.player_mut(id).unwrap().set_bad(1 << 6);
        entry.ability = 6;
        entry.progress[6] = 100;
        learnt(&mut records, &mut entry, 6, 1, &tables);
        let p = records.player(id).unwrap();
        assert_eq!((p.good(), p.bad(), entry.progress[6]), (1 | 1 << 1 | 1 << 9, 0, 0));

        entry.ability = 9;
        learnt(&mut records, &mut entry, 9, 1, &tables);
        assert_eq!((entry.ability, entry.progress[9]), (NOTHING, LEARNT));
    }

    /// What a match teaches A's men (`FUN_00747855` behind
    /// `FUN_00740aad`, `FUN_00740dc4`, `FUN_00741038`): keeper 3257
    /// saving a big chance takes ten of Elfmetertöter — as he did across
    /// the original's Wolfsburg match from Toni.an3 to A.an3 —, a headed
    /// goal three of Kopfball and of Torinstinkt to a field man lacking
    /// them, an assist three of Spielmacher to a midfielder alone; a
    /// computer club's goal teaches nobody. Past 99 he learns what his
    /// entry works on, not what the match taught, and works on nothing.
    #[test]
    #[ignore = "reads the oracle's kept saves beside $ANSTOSS3_DIR"]
    fn matches_teach_the_human_clubs_men() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let tables = crate::data::Data::load(&dir).unwrap().rules.individual;
        let save = a3_files::an3::Save::read(&dir.parent().unwrap().join("saves-kept/A.an3")).unwrap();
        let mut records = Records::from_save(save).unwrap();
        let team = records.team(14, 84).unwrap();
        let (squad, human) = (team.players(), team.human());
        let slot = usize::from(human) - 1;
        let foe = (1..20).find(|&t| t != 84 && records.team(14, t).is_some_and(|t| t.humans() == [0, 0])).unwrap();
        let entry = |records: &Records, id: u16| {
            records.individual[slot].entries().into_iter().find(|e| e.id == id).unwrap()
        };
        let pick = |records: &Records, at: &dyn Fn(u8) -> bool, lacks: u16| {
            squad
                .iter()
                .copied()
                .find(|&id| {
                    let p = records.player(id).unwrap();
                    at(p.positions()[0])
                        && p.good() & lacks == 0
                        && records.individual[slot].entries().iter().any(|e| e.id == id)
                })
                .unwrap()
        };
        let striker = pick(&records, &|at| at > 9, 1 | 1 << 7);
        let midfielder = pick(&records, &|at| (6..=9).contains(&at), 1 << 11 | 1 << 7);
        let defender = pick(&records, &|at| (2..6).contains(&at), 1 << 11);
        assert_eq!(records.player(3257).unwrap().good() & 1, 0);
        let before = [3257, striker, midfielder, defender].map(|id| entry(&records, id));
        let mut m = crate::sim::Match::default();
        m.ids = [14, 84, 14, foe];
        m.sides[0].goal_book[0] = (striker, 2);
        m.sides[0].assist_book[..2].copy_from_slice(&[midfielder, defender]);
        m.sides[1].keeper_book[0] = (3257, true);
        m.sides[1].goal_book[0] = (midfielder, 2);
        crate::sim::post_match_lessons(&m, &mut records, &tables);
        let moved = |records: &Records, k: usize, ability: usize| {
            entry(records, before[k].id).progress[ability] - before[k].progress[ability]
        };
        assert_eq!(moved(&records, 0, 0), 10, "Elfmetertöter");
        assert_eq!((moved(&records, 1, 0), moved(&records, 1, 3), moved(&records, 1, 7)), (3, 0, 3));
        assert_eq!((moved(&records, 2, 11), moved(&records, 2, 0), moved(&records, 2, 7)), (3, 0, 0));
        assert_eq!(entry(&records, defender), before[3], "a defender's assist");

        // past 99 on Torinstinkt he learns Zweikampf, which he works on
        let mut e = entry(&records, striker);
        (e.ability, e.progress[7], e.progress[1]) = (1, 98, 40);
        let mut entries = records.individual[slot].entries();
        individual::keep(&mut entries, e);
        records.individual[slot].set_entries(&entries);
        records.player_mut(striker).unwrap().set_good(0);
        records.player_mut(striker).unwrap().set_bad(0);
        goal_lessons(&mut records, &tables, (14, 84), striker, -1);
        let e = entry(&records, striker);
        assert_eq!((e.ability, e.progress[7], e.progress[1]), (NOTHING, 101, LEARNT));
        assert_eq!(records.player(striker).unwrap().good(), 1 << 1);
        assert!(records.abilities_week(human).iter().any(|w| w.id == striker && w.gained == 1 << 1));
    }

    /// The week's start on A's squad (`FUN_00746f46`, `FUN_00786285`): a
    /// tired man on the extra shift who slipped this week gains two points
    /// of condition, lifts the mood and the will to win by two and has his
    /// slip punished; a man who slipped last week unpunished costs a point
    /// of mood and of the fans' and the media's liking. A weak man in the
    /// youth round loses his grievance and gains a point; a strong one
    /// rolls once. Every punishment of the week is forgotten. Playing a
    /// match takes a man out of the youth round, a man without an entry
    /// taking a bare one.
    #[test]
    #[ignore = "reads the oracle's kept saves beside $ANSTOSS3_DIR"]
    fn the_week_starts_with_the_extra_shift_and_the_youth_round() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let save = a3_files::an3::Save::read(&dir.parent().unwrap().join("saves-kept/A.an3")).unwrap();
        let mut records = Records::from_save(save).unwrap();
        let exe = a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap();
        let tables = MoodTables::load(&exe).unwrap();
        let club = statistics::human_clubs(&records)[0];
        let at = (u16::from(club.0), club.1 as u16);
        let team = records.team(at.0, at.1).unwrap();
        let (squad, slot, trainer) = (team.players(), usize::from(team.human()) - 1, team.humans()[0]);
        let real: Vec<u16> = squad.iter().copied().filter(|&id| id <= crate::mood::LAST_REAL_ID).collect();
        let strong = |records: &Records, id: u16| records.player(id).unwrap().strength() >= YOUTH_STRENGTH;
        let rest: Vec<u16> = real[2..].to_vec();
        let (tired, slipped) = (real[0], real[1]);
        let fit = *rest.iter().find(|&&id| strong(&records, id)).expect("a strong man");
        let weak = *rest.iter().find(|&&id| !strong(&records, id)).expect("a weak man");
        for &id in &squad {
            let p = records.player_mut(id).unwrap();
            p.set_offences(0);
            p.set_fined(0);
        }
        let mut entries = records.individual[slot].entries();
        for e in entries.iter_mut() {
            (e.youth, e.extra) = (false, false);
        }
        for (id, youth, extra) in [(tired, false, true), (fit, true, false), (weak, true, false)] {
            let mut entry = entries.iter().find(|e| e.id == id).copied().unwrap_or_default();
            (entry.id, entry.youth, entry.extra, entry.intensity) = (id, youth, extra, individual::HARD);
            individual::keep(&mut entries, entry);
        }
        records.individual[slot].set_entries(&entries);
        let p = records.player_mut(tired).unwrap();
        p.set_condition(4000);
        p.set_offences(0x10);
        records.player_mut(slipped).unwrap().set_offences(0x01);
        let p = records.player_mut(weak).unwrap();
        p.set_condition(5000);
        p.set_flags(p.flags() | YOUTH_GRUDGE);
        let team = records.team(at.0, at.1).unwrap();
        let (mood, will) = (i32::from(team.mood()), team.will_to_win());
        let liked = records.managers[usize::from(trainer) - 1].popularity();
        let mut random = Random::from_state(24681, 13579);
        let mut expected = random.clone();
        expected.below(100);
        week_start(&mut records, &tables, &mut random);
        assert_eq!(random, expected, "only the strong man in the youth round rolls");
        let p = records.player(tired).unwrap();
        assert_eq!((p.condition(), p.intensity()), (4200, individual::HARD as u8));
        let team = records.team(at.0, at.1).unwrap();
        assert_eq!(i32::from(team.mood()), (mood + 2).clamp(0, 200) - 1);
        assert_eq!(team.will_to_win(), (will + 2).clamp(-200, 200));
        let now = records.managers[usize::from(trainer) - 1].popularity();
        assert_eq!(now, [liked[0], liked[1].saturating_sub(1), liked[2].saturating_sub(1)]);
        let p = records.player(weak).unwrap();
        assert_eq!((p.condition(), p.flags() & YOUTH_GRUDGE), (5100, 0));
        assert!(squad.iter().all(|&id| records.player(id).unwrap().fined() == 0));

        let gone = rest.iter().copied().find(|&id| id != fit && id != weak).unwrap();
        let mut entries = records.individual[slot].entries();
        individual::purge(&mut entries, &squad.iter().copied().filter(|&id| id != gone).collect::<Vec<_>>());
        records.individual[slot].set_entries(&entries);
        leave_youth_round(&mut records, at, fit);
        leave_youth_round(&mut records, at, gone);
        let entries = records.individual[slot].entries();
        assert!(entries.iter().any(|e| e.id == fit && !e.youth && e.intensity == individual::HARD));
        let bare = entries.iter().find(|e| e.id == gone).expect("a bare entry");
        assert_eq!((bare.ability, bare.intensity, bare.youth), (0, individual::OUT, false));
    }

    /// A's club in a camp at place 2 (its ball 4) from day 100: the
    /// session's share is 104, on the camp's last day nought, after it
    /// the co-trainer's 95 or a hundred again. Two men sharing room 0,
    /// the first hating the second, fall out (ten each, or fifteen for
    /// him) and meet nothing more; making up closes the list behind the
    /// one forgiven and clears the third friend. The camp before a match
    /// with the sliders at 4 costs two of the mood and brings the first
    /// two men two points; the first such camp makes every man three or
    /// four more content with the team, the third costs ten of the mood.
    #[test]
    #[ignore = "reads the oracle's kept saves beside $ANSTOSS3_DIR"]
    fn a_camp_works_on_the_squad() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let save = a3_files::an3::Save::read(&dir.parent().unwrap().join("saves-kept/A.an3")).unwrap();
        let mut records = Records::from_save(save).unwrap();
        let exe = a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap();
        let (camps, tables) = (YearTables::load(&exe).unwrap(), MoodTables::load(&exe).unwrap());
        let (country, number) = statistics::human_clubs(&records)[0];
        let club = (u16::from(country), number as u16);
        let trainer = usize::from(records.team(club.0, club.1).unwrap().humans()[0]) - 1;

        assert_eq!(camps.places[1].ball, 4);
        year::book_camp(&mut records.calendars[trainer], 100, 2);
        let level = |records: &Records, d: usize| year::camp_level(&camps, &records.calendars[trainer], d);
        assert_eq!((level(&records, 99), level(&records, 100), level(&records, 101), level(&records, 102)), (None, Some(4), Some(118), None));
        assert_eq!(share(&mut records, club, Some(4)), 104);
        assert_eq!(share(&mut records, club, Some(118)), 0);
        let co = records.team(club.0, club.1).unwrap().co_trainer() > 0;
        assert_eq!(share(&mut records, club, None), if co { 95 } else { 100 });

        let squad = records.team(club.0, club.1).unwrap().players();
        let fit: Vec<u16> = squad
            .iter()
            .copied()
            .filter(|&id| id <= crate::mood::LAST_REAL_ID && records.player(id).is_some_and(|p| days_out(&records, p) < 8))
            .collect();
        let (one, mate) = (fit[0], fit[1]);
        for &id in &squad {
            let p = records.player_mut(id).unwrap();
            p.set_room(-1);
            p.set_shown(p.shown() & !CAMP_MET);
            p.set_best_friend(0);
            p.set_hates(0);
        }
        for id in [one, mate] {
            records.player_mut(id).unwrap().set_room(0);
        }
        records.player_mut(one).unwrap().set_hates(mate);
        let trusts = |records: &Records, id: u16| records.player(id).unwrap().contentment()[0];
        let before = [trusts(&records, one), trusts(&records, mate)];
        let mut posted = Vec::new();
        let mut random = Random::from_state(1357, 2468);
        assert!(camp_round(&mut records, club, &squad, 100, &mut posted, &mut random));
        let after = [trusts(&records, one), trusts(&records, mate)];
        let fell = [i32::from(before[0]) - i32::from(after[0]), i32::from(before[1]) - i32::from(after[1])];
        assert!(fell == [10, 10] || fell == [15, 0], "{fell:?}");
        assert_eq!(posted[0].code, CAMP_EVENT);
        assert!(HATED_MATE.contains(&posted[0].text));
        assert_eq!(posted[0].fill, Fill::Numbered(one, mate));
        assert!([one, mate].iter().all(|&id| records.player(id).unwrap().shown() & CAMP_MET != 0));
        assert!(!camp_round(&mut records, club, &squad, 100, &mut posted, &mut random));
        assert_eq!(posted.len(), 1);

        let (x, y) = (fit[2], fit[3]);
        records.player_mut(one).unwrap().set_dislikes([x, mate, y]);
        records.player_mut(one).unwrap().set_friends([x, y, mate]);
        records.player_mut(mate).unwrap().set_dislikes([one, 0, 0]);
        reconcile(&mut records, one, mate);
        assert_eq!(records.player(one).unwrap().hates(), 0);
        assert_eq!(records.player(one).unwrap().dislikes(), [x, y, y]);
        assert_eq!(records.player(one).unwrap().friends(), [x, y, 0]);
        assert_eq!(records.player(mate).unwrap().dislikes(), [0, 0, 0]);

        let world = world::World::load(&dir.join("Data.a3")).unwrap();
        let mut clubs = BTreeMap::new();
        records.managers[trainer].set_camp_rules([4, 4, 4]);
        let flags = records.team(club.0, club.1).unwrap().training_flags() & !CAMPS_HELD;
        records.team_mut(club.0, club.1).unwrap().set_mood(100);
        records.team_mut(club.0, club.1).unwrap().set_training_flags(flags | training::CAMP);
        let held: Vec<(i16, i16, u8)> = squad
            .iter()
            .map(|&id| records.player(id).unwrap())
            .map(|p| (p.condition(), p.freshness()[0], p.contentment()[1]))
            .collect();
        camp_before_match(&mut records, &mut clubs, &world, (country, number), &tables, &mut random);
        let team = records.team(club.0, club.1).unwrap();
        assert_eq!(team.mood(), 98);
        assert_eq!(team.training_flags() & (training::CAMP | CAMPS_HELD), 0x100);
        for (k, (&id, &(condition, fresh, content))) in squad.iter().zip(&held).enumerate() {
            let p = records.player(id).unwrap();
            let gained = i32::from(p.condition()) - i32::from(condition) + i32::from(p.freshness()[0]) - i32::from(fresh);
            if k < 2 {
                assert!(gained == 200 || condition >= 9800 || fresh >= 9800, "{id}: {gained}");
            } else {
                assert_eq!(gained, 0);
            }
            let more = i32::from(p.contentment()[1]) - i32::from(content);
            assert!(more == 3 || more == 4 || p.contentment()[1] == 120, "{id}: {more}");
        }
        // Without the flag nothing moves; the third camp costs ten.
        let held = random.clone();
        camp_before_match(&mut records, &mut clubs, &world, (country, number), &tables, &mut random);
        assert_eq!(random, held);
        records.managers[trainer].set_camp_rules([2, 2, 2]);
        let flags = records.team(club.0, club.1).unwrap().training_flags();
        records.team_mut(club.0, club.1).unwrap().set_training_flags(flags | training::CAMP | 0x200);
        camp_before_match(&mut records, &mut clubs, &world, (country, number), &tables, &mut random);
        let team = records.team(club.0, club.1).unwrap();
        assert_eq!(team.mood(), 88);
        assert_eq!(team.training_flags() & (training::CAMP | CAMPS_HELD), 0x200);
    }
}
