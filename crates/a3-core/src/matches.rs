//! The league matchdays of a running season: every game the league blocks
//! still hold open on a calendar day is scored here and written back where
//! the original's played matches put them. The 3D match of the human's club
//! is out of the replica's scope, so its games are scored like the others.

use crate::calendar::Calendar;
use crate::opponent::League;
use crate::random::Random;
use crate::records::Records;
use crate::season::Season;
use crate::statistics::{self, ALL};
use crate::world::{self, World};

/// Strengths to reckon by when a team's record (G+0xb8) carries none: the
/// goalkeeper, defenders, midfield and attack in tenths, attack and hold
/// even so the luck alone makes the goals.
const EVEN: [i16; 4] = [340; 4];
/// The home side's edge, in the strengths' tenths.
const HOME_EDGE: i32 = 50;
/// How much the difference of the strengths is worth a goal.
const GOAL_UNITS: i32 = 200;

/// Play the league games of the season's day: for every country the game
/// follows, every league whose calendar row holds this day as one of its
/// matchdays and has not played that matchday yet gets its pairs scored.
pub fn play_day(records: &mut Records, world: &World, calendar: &Calendar, season: &mut Season) {
    let day = season.day;
    for slot in 0..world::SLOTS {
        let Some(nation) = world.country(slot).map(|country| country.nation) else {
            continue;
        };
        if !records.globals.played(i16::try_from(nation).unwrap_or(0)) {
            continue;
        }
        for level in 0..records.countries.leagues(usize::from(nation)) {
            let level = level as u8;
            let Some(row) = calendar.row(nation, usize::from(level)) else {
                continue;
            };
            let Some(matchday) = calendar.matchday(row, day) else {
                continue;
            };
            let due = League::of(records, nation, level).is_some_and(|league| {
                league.played() + 1 == matchday && !league.open_games().is_empty()
            });
            if due {
                settle(records, nation, level, &mut season.random);
            }
        }
    }
}

/// Score a league's next matchday and write it to its block: the four goal
/// bytes of each pair, the count of matchdays played, and the crowd — left
/// nought, the gate and its money being the stadium's to bring (the
/// original writes totals of some hundred and fifty thousand here) — then
/// the places the table shows after the matchday, and each club's season
/// record, goals and table place.
fn settle(records: &mut Records, nation: u16, level: u8, random: &mut Random) {
    let Some(league) = League::of(records, nation, level) else {
        return;
    };
    let Some(name) = League::block_name(records, nation, level) else {
        return;
    };
    let games = league.open_games();
    // The byte where each place writes its place after this matchday, and
    // where the count and the crowd stand — all taken before the count is
    // raised, since they lie on the matchday as it was played.
    let strengths: Vec<[i16; 4]> = (0..league.teams())
        .map(|place| {
            let held = records
                .team(nation, league.team(place) as u16)
                .map(|team| team.strengths())
                .unwrap_or(EVEN);
            if held == [0; 4] { EVEN } else { held }
        })
        .collect();
    let places: Vec<Option<usize>> = (0..league.teams())
        .map(|place| league.place_slot(place))
        .collect();
    let played_at = league.played_slot();
    let crowd_at = league.crowd_slot();
    let played = (league.played() + 1) as i16;
    let scored: Vec<[u8; 4]> = games
        .iter()
        .map(|&(home, away, _)| {
            let (full, half) = goals(strengths[home], strengths[away], HOME_EDGE, random);
            let (a_full, a_half) = goals(strengths[away], strengths[home], -HOME_EDGE, random);
            [full, a_full, half, a_half]
        })
        .collect();
    if let Some(block) = records.block_mut(&name) {
        for ((_, _, at), result) in games.iter().zip(&scored) {
            if let Some(slot) = block.get_mut(*at..*at + 4) {
                slot.copy_from_slice(result);
            }
        }
        if let Some(slot) = block.get_mut(played_at..played_at + 2) {
            slot.copy_from_slice(&played.to_le_bytes());
        }
        if let Some(slot) = block.get_mut(crowd_at..crowd_at + 4) {
            slot.copy_from_slice(&0i32.to_le_bytes());
        }
    }
    // The table as it stands after the matchday, and the places its rows
    // write into the blocks' history.
    let Some(league) = League::of(records, nation, level) else {
        return;
    };
    let rows = statistics::table(records, &league, ALL);
    if let Some(block) = records.block_mut(&name) {
        for (place, rank) in rows.iter().enumerate().map(|(rank, row)| (row.place, rank + 1)) {
            if let Some(at) = places.get(place).and_then(|slot| *slot)
                && let Some(byte) = block.get_mut(at)
            {
                *byte = rank as u8;
            }
        }
    }
    for (rank, row) in rows.iter().enumerate() {
        let record = [
            u8::try_from(row.won).unwrap_or(u8::MAX),
            u8::try_from(row.drawn).unwrap_or(u8::MAX),
            u8::try_from(row.lost).unwrap_or(u8::MAX),
        ];
        let goals = [
            u8::try_from(row.goals_for).unwrap_or(u8::MAX),
            u8::try_from(row.goals_against).unwrap_or(u8::MAX),
        ];
        if let Some(team) = records.team_mut(nation, row.team as u16) {
            team.set_record(record);
            team.set_goals(goals);
            team.set_table_place(rank as u8 + 1);
        }
    }
}

/// How many goals one side scores past the other: its midfield and attack
/// over their goalkeeper and defenders — in tenths, GOAL_UNITS of the
/// difference being a goal, and the home side playing the stronger by
/// HOME_EDGE — with a luck from nought to nine lifting the count by half of
/// what it is beyond two, and sharing the goals over the halves by its
/// remainder in threes. Calibrated
/// on a save's played matchdays, which bring some two goals to a game.
fn goals(att: [i16; 4], hold: [i16; 4], edge: i32, random: &mut Random) -> (u8, u8) {
    let attack = i32::from(att[2]) + i32::from(att[3]);
    let defence = i32::from(hold[0]) + i32::from(hold[1]);
    let luck = random.below(10);
    let full =
        ((attack - defence + edge) / GOAL_UNITS + (i32::try_from(luck).unwrap_or(0) - 2) / 2)
            .clamp(0, 6) as u8;
    let half = (u32::from(full) * (luck % 3) / 2) as u8;
    (full, half)
}
