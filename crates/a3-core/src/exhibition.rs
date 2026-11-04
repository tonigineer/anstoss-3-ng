//! The day's matches outside the competitions (`FUN_007615bb`, from the
//! human clubs' day `FUN_0064f981`): the tournaments' games — and the
//! friendlies booked for the day, still to come — go into the day's match
//! list, are played, and their results booked; then the tournaments'
//! evening.
//!
//! The original plays the list through its match engine (`FUN_006f3b7a`).
//! Until the engine is ported, the replica scores these games as it scores
//! the leagues' ([`crate::matches`]): by the teams' strengths and a luck,
//! a tie in a tournament decided by extra time and then penalties; the
//! crowd a part of the ground's planned attendance and the gate some 20 €
//! a head — what the original's played Turniere in save A come to
//! (a median of a quarter of the plan a game, from 10 to 36 € a head).

use crate::calendar::Calendar;
use crate::finances::CLUB_TO_TEAM;
use crate::matches::{EVEN, HOME_EDGE, capacity, goals};
use crate::random::Random;
use crate::records::Records;
use crate::tournament::{self, Fixture, Outcome};

/// The plan's attendance (club +0x120).
const PLAN_ATTENDANCE: usize = 0x120;
/// The crowd: ten to forty in a hundred of the ground's plan.
const CROWD_LEAST: u32 = 10;
const CROWD_SPREAD: u32 = 31;
/// What a head pays.
const PER_HEAD: i32 = 20;
/// Extra time is a third of a game.
const EXTRA_PART: u8 = 3;

/// What the human should see of the day's tournaments: the Turniere drawn
/// today (their pairings) and those whose day ended (their results).
#[derive(Default, Debug)]
pub struct Shown {
    pub drawn: Vec<u32>,
    pub results: Vec<u32>,
}

/// The day's matches (`FUN_007615bb`) on day `today`.
pub fn play_day(records: &mut Records, calendar: &Calendar, today: i16, random: &mut Random) -> Shown {
    let fixtures = tournament::day_games(records, calendar, today, random);
    let mut shown = Shown::default();
    for fixture in &fixtures {
        // A human in its four sees its pairings (`FUN_0062622f`).
        if fixture.game == tournament::SEMI_ONE && humans_in(records, fixture.id) {
            shown.drawn.push(fixture.id);
        }
    }
    let outcomes: Vec<Outcome> = fixtures.iter().map(|f| play(records, f, true, random)).collect();
    for (fixture, outcome) in fixtures.iter().zip(&outcomes) {
        tournament::record(records, fixture, outcome);
    }
    shown.results = tournament::evening(records, today);
    shown
}

fn humans_in(records: &Records, id: u32) -> bool {
    tournament::find(records, id)
        .is_some_and(|t| t.clubs().iter().take(4).any(|&(c, n)| crate::statistics::is_human(records, c, n)))
}

/// A club's strengths in goal, defence, midfield and attack, in tenths;
/// none held, even ones.
fn strengths(records: &Records, (country, number): (u8, i16)) -> [i16; 4] {
    let held = u16::try_from(number).ok().and_then(|n| records.team(country.into(), n)).map_or(EVEN, |t| t.strengths());
    if held == [0; 4] { EVEN } else { held }
}

/// A game scored as the replica scores the leagues': the side at its
/// ground with its edge; a tie in a tournament goes to extra time, then
/// to penalties, which only name the winner.
pub(crate) fn play(records: &Records, f: &Fixture, decided: bool, random: &mut Random) -> Outcome {
    let edge = if f.home == f.ground {
        HOME_EDGE
    } else if f.away == f.ground {
        -HOME_EDGE
    } else {
        0
    };
    let (home, away) = (strengths(records, f.home), strengths(records, f.away));
    let (h, _) = goals(home, away, edge, random);
    let (a, _) = goals(away, home, -edge, random);
    let mut outcome = Outcome { goals: (h, a), winner: u8::from(a > h), ..Outcome::default() };
    if decided && h == a {
        outcome.extra = true;
        let (eh, _) = goals(home, away, edge, random);
        let (ea, _) = goals(away, home, -edge, random);
        let (eh, ea) = (eh / EXTRA_PART, ea / EXTRA_PART);
        outcome.goals = (h + eh, a + ea);
        if eh == ea {
            outcome.penalties = true;
            outcome.winner = random.below(2) as u8;
        } else {
            outcome.winner = u8::from(ea > eh);
        }
    }
    let (country, number) = f.ground;
    if let Some(team) = u16::try_from(number).ok().and_then(|n| records.team(country.into(), n)) {
        let plan = team.get::<i32>(PLAN_ATTENDANCE + CLUB_TO_TEAM).max(0);
        let share = i32::try_from(CROWD_LEAST + random.below(CROWD_SPREAD)).unwrap_or(25);
        let mut crowd = plan.wrapping_mul(share) / 100;
        let cap = capacity(records, country.into(), number as u16);
        if cap > 0 {
            crowd = crowd.min(cap);
        }
        outcome.crowd = crowd;
        outcome.gate = crowd.wrapping_mul(PER_HEAD);
    }
    outcome
}
