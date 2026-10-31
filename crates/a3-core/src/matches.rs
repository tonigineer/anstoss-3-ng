//! The league matchdays of a running season: every game the league blocks
//! still hold open on a calendar day is scored here and written back where
//! the original's played matches put them. The 3D match of the human's club
//! is out of the replica's scope, so its games are scored like the others.

use crate::calendar::Calendar;
use crate::finances::CLUB_TO_TEAM;
use crate::opponent::League;
use crate::random::Random;
use crate::records::{Records, Team};
use crate::season::Season;
use crate::statistics::{self, ALL};
use crate::tickets;
use crate::world::{self, World};

/// The stadiums (`0x1a48138[index]`), and the capacity within.
const STADIUM_AT: u32 = 0x1a48138;
const CAPACITY: usize = 0x3c;
/// The plan's attendance and its season-ticket holders (club +0x120,
/// +0x124), and the gate money booked match by match (club +0x230).
const PLAN_ATTENDANCE: usize = 0x120;
const PLAN_SEASON_TICKETS: usize = 0x124;
const BOOKED_LEAGUE: usize = 0x230;

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
/// bytes of each pair, the count of matchdays played, and the crowd — the
/// matchday's gates together, as the original keeps them by the hundred
/// and fifty thousand — then the places the table shows after the
/// matchday, and each club's season record, goals and table place. Every
/// home club books its gate: the attendance on its played matches, the
/// money on its season's takings, and the league's attendance row
/// follows.
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
    let matchday = i32::try_from(league.played() + 1).unwrap_or(0);
    let scored: Vec<[u8; 4]> = games
        .iter()
        .map(|&(home, away, _)| {
            let (full, half) = goals(strengths[home], strengths[away], HOME_EDGE, random);
            let (a_full, a_half) = goals(strengths[away], strengths[home], -HOME_EDGE, random);
            [full, a_full, half, a_half]
        })
        .collect();
    // The gate each home club books for its match — the attendance, the
    // money and the stadium it fills — reckoned before the block and the
    // clubs are written.
    let homes: Vec<u16> = games.iter().map(|&(home, _, _)| league.team(home) as u16).collect();
    let booked: Vec<(i32, i32, i32)> = homes
        .iter()
        .map(|&team| match records.team(nation, team) {
            Some(t) => {
                let cap = capacity(records, nation, team);
                let att = attendance(t, cap, random);
                (att, tickets::booked_gate(records, t, att, matchday), cap)
            }
            None => (0, 0, 0),
        })
        .collect();
    let crowd: i32 = booked.iter().map(|&(att, _, _)| att).sum();
    let att_slot = statistics::attendance_slot(records, nation, level);
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
            slot.copy_from_slice(&crowd.to_le_bytes());
        }
    }
    for ((home, _, _), (&team, (att, money, cap))) in
        games.iter().zip(homes.iter().zip(booked))
    {
        if let Some(t) = records.team_mut(nation, team) {
            let old = t.get::<i32>(BOOKED_LEAGUE + CLUB_TO_TEAM);
            t.put::<i32>(BOOKED_LEAGUE + CLUB_TO_TEAM, old.wrapping_add(money));
            let held = usize::from(t.home_games());
            let mut gates = t.gates();
            if let Some(entry) = gates.get_mut(held) {
                *entry = att;
                t.set_gates(gates);
            }
            t.set_home_games(u8::try_from(held + 1).unwrap_or(u8::MAX));
        }
        if let Some(slot) = att_slot {
            statistics::book_attendance(records, slot, *home, att, cap);
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

/// How many the club's stadium holds (`0x1a48138[index]` +0x3c).
fn capacity(records: &Records, nation: u16, team: u16) -> i32 {
    records
        .team_index(nation, team)
        .and_then(|i| records.block(&format!("{STADIUM_AT:#x}[{i}]")))
        .map_or(0, |b| {
            b.get(CAPACITY..CAPACITY + 4).map_or(0, |c| i32::from_le_bytes(c.try_into().unwrap()))
        })
}

/// How many come to one of the club's home matches: the plan's attendance
/// with three in ten off to three in ten on, held up by its season-ticket
/// holders and capped by the stadium when it holds places. The original
/// draws its crowd over the plan too (`FUN_0078e5e6`, its clubs' fills a
/// little above and below the plan); the replica's luck rides a tenth to
/// two tenths about it.
fn attendance(team: &Team, capacity: i32, random: &mut Random) -> i32 {
    let plan = team.get::<i32>(PLAN_ATTENDANCE + CLUB_TO_TEAM);
    if plan <= 0 {
        return 0;
    }
    let season = team.get::<i32>(PLAN_SEASON_TICKETS + CLUB_TO_TEAM).max(0);
    let luck = 70 + i32::try_from(random.below(61)).unwrap_or(70);
    let a = plan.wrapping_mul(luck) / 100;
    let a = a.max(season);
    if capacity > 0 { a.min(capacity) } else { a }
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
