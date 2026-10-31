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
/// follows. Each match leaves its mark on its players: appearances and
/// scorers' goals in their records, and in the league's top-scorers list.
/// Both clubs keep their gate figures: the home club its crowd, income and
/// the visiting fans, the away club the travelling gate of the game.
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
    let aways: Vec<u16> = games.iter().map(|&(_, away, _)| league.team(away) as u16).collect();
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
    for (((home, _, _), (&team, (att, money, cap))), &away) in
        games.iter().zip(homes.iter().zip(booked)).zip(&aways)
    {
        // The visiting fans: a part of the away club's own plan (save A
        // reads 3926 with the away club planning 34000), never more than
        // came to the ground. The original draws them inside the match
        // (the match record's +0x54); the replica draws them over the plan.
        let plan = records
            .team(nation, away)
            .map_or(0, |a| a.get::<i32>(PLAN_ATTENDANCE + CLUB_TO_TEAM));
        let luck = 8 + i32::try_from(random.below(7)).unwrap_or(8);
        let fans = plan.max(0).wrapping_mul(luck) / 100;
        let fans = fans.clamp(0, att.max(0));
        // The home club: its gate held among its gates, the money booked,
        // and — where it has a human's extra block — the visiting fans and
        // the gate's income of this home game, as the statistics pages
        // read them (the match record's +0x54 and +0x3c into extra +0x3f4
        // and +0x450, `FUN_007108d3`).
        let mut host = (0usize, 0u8);
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
            host = (held, t.human());
        }
        if host.1 > 0
            && let Some(x) = records.extras.get_mut(usize::from(host.1) - 1)
        {
            let mut fan_gates = x.fan_gates();
            let mut incomes = x.home_incomes();
            if let (Some(f), Some(m)) = (fan_gates.get_mut(host.0), incomes.get_mut(host.0)) {
                *f = fans;
                *m = money;
                x.set_fan_gates(fan_gates);
                x.set_home_incomes(incomes);
            }
        }
        // The away club: its count of away games up, and the gate of the
        // travelling — the whole gate at the opponent's ground, the match
        // record's +0x34 into extra +0x398 (`FUN_0063f09b`).
        let mut guest = (0usize, 0u8);
        if let Some(a) = records.team_mut(nation, away) {
            let held = usize::from(a.away_games());
            a.set_away_games(u8::try_from(held + 1).unwrap_or(u8::MAX));
            guest = (held, a.human());
        }
        if guest.1 > 0
            && let Some(x) = records.extras.get_mut(usize::from(guest.1) - 1)
        {
            let mut away_gates = x.away_gates();
            if let Some(entry) = away_gates.get_mut(guest.0) {
                *entry = att;
                x.set_away_gates(away_gates);
            }
        }
        if let Some(slot) = att_slot {
            statistics::book_attendance(records, slot, *home, att, cap);
        }
    }
    // What the goals leave on the players of both sides.
    for ((&home, &away), result) in homes.iter().zip(&aways).zip(&scored) {
        mark_match(records, nation, level, home, away, result[0], result[1], random);
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

/// Weights a goal is drawn by, over the position: nought for the keeper,
/// little for the defence, most for the attack — times the player's
/// strength. The positions run 1 to 10, keeper to striker; the special
/// kinds from 11 on (the exe's table at 0x89e23c) count among the attack.
const KIND_WEIGHTS: [u32; 11] = [0, 0, 1, 1, 1, 2, 2, 3, 4, 5, 6];

/// What a scored match leaves on its players: the eleven each side puts
/// on the pitch stand for their league match (+0x253) and their run of
/// matches in a row (+0x34e), broken for those listed but not picked;
/// every goal goes to one of the eleven, drawn over their weights — the
/// scorer's weight is his strength times his position's kind — and stands
/// in his season's league goals (+0x23c), his goals in the last match
/// (+0x249), his home goals when his side played at home (+0x252), and in
/// the league's top-scorer list. Grades, cards and the fitness wear
/// follow another step.
fn mark_match(
    records: &mut Records,
    nation: u16,
    level: u8,
    home: u16,
    away: u16,
    full: u8,
    a_full: u8,
    random: &mut Random,
) {
    for (team, goals, at_home) in [(home, full, true), (away, a_full, false)] {
        let squad = records
            .team(nation, team)
            .map(|t| t.players())
            .unwrap_or_default();
        let eleven = eleven(&squad, records);
        let weights: Vec<u32> = eleven
            .iter()
            .map(|&id| {
                records.player(id).map_or(0, |p| {
                    u32::from(p.strength()) * KIND_WEIGHTS[usize::from(p.positions()[0]).min(10)]
                })
            })
            .collect();
        let mut scored = vec![0u8; eleven.len()];
        for _ in 0..u32::from(goals) {
            let Some(k) = draw(&weights, random) else {
                break;
            };
            scored[k] = scored[k].saturating_add(1);
            let id = eleven[k];
            let names = records.player(id).map(|p| (p.first_name(), p.last_name()));
            if let Some((first, last)) = names {
                statistics::book_goal(
                    records,
                    nation,
                    level,
                    id,
                    first,
                    last,
                    u8::try_from(team).unwrap_or(0),
                );
            }
        }
        for (&id, &n) in eleven.iter().zip(&scored) {
            if let Some(p) = records.player_mut(id) {
                p.set_league_matches(p.league_matches().saturating_add(1));
                p.set_matches_in_a_row(p.matches_in_a_row().saturating_add(1));
                p.set_last_goals(n);
                if n > 0 {
                    let mut goals = p.goals();
                    goals[0] = goals[0].saturating_add(n);
                    p.set_goals(goals);
                    if at_home {
                        p.set_home_goals(p.home_goals().saturating_add(n));
                    }
                }
            }
        }
        for &id in squad.iter().filter(|&id| !eleven.contains(&id)) {
            if let Some(p) = records.player_mut(id) {
                p.set_matches_in_a_row(0);
            }
        }
    }
}

/// The eleven a side puts on the pitch: of the players it lists, those
/// fit and free — no injury (+0x228), no match suspension (+0x229) or
/// weeks of a ban (+0x22a), and not marked suspended for the match
/// (+0x26b bit 4) — the keeper first, the strongest who names him one,
/// and the ten strongest field players beside him. The original follows
/// the manager's formation; the replica has none of his yet and takes
/// the strongest.
fn eleven(squad: &[u16], records: &Records) -> Vec<u16> {
    let strength = |id: u16| u32::from(records.player(id).map_or(0, |p| p.strength()));
    let mut fit: Vec<u16> = squad
        .iter()
        .copied()
        .filter(|&id| {
            records.player(id).is_some_and(|p| {
                p.injury_kind() == 0
                    && p.suspension() & 0x3f == 0
                    && p.banned_weeks() == 0
                    && p.away_kind() & 0x10 == 0
            })
        })
        .collect();
    fit.sort_by_key(|&id| std::cmp::Reverse(strength(id)));
    let keeper = fit
        .iter()
        .copied()
        .find(|&id| records.player(id).is_some_and(|p| p.positions()[0] == world::GOALKEEPER));
    let mut eleven: Vec<u16> = keeper.into_iter().collect();
    eleven.extend(fit.iter().copied().filter(|&id| {
        id != keeper.unwrap_or(0)
            && records.player(id).is_some_and(|p| p.positions()[0] != world::GOALKEEPER)
    }));
    eleven.truncate(11);
    eleven
}

/// Which of the eleven scored a goal: drawn over their weights, or of
/// all of them alike when none of them weighs aught.
fn draw(weights: &[u32], random: &mut Random) -> Option<usize> {
    if weights.is_empty() {
        return None;
    }
    let total: u32 = weights.iter().sum();
    let cap = if total == 0 {
        u32::try_from(weights.len()).unwrap_or(1)
    } else {
        total
    };
    let mut draw = random.below(cap.max(1));
    if total == 0 {
        return Some((draw as usize).min(weights.len() - 1));
    }
    for (k, &w) in weights.iter().enumerate() {
        if draw < w {
            return Some(k);
        }
        draw -= w;
    }
    Some(weights.len() - 1)
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
