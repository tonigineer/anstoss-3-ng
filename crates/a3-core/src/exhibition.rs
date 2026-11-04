//! The day's matches outside the competitions (`FUN_007615bb`, from the
//! human clubs' day `FUN_0064f981`): the friendlies booked for the day
//! (`FUN_00625212`, `FUN_00625310`) and the tournaments' games
//! ([`crate::tournament::day_games`]) go into the day's match list; the
//! list is played (`FUN_006f3b7a`) and its results booked; then the
//! tournaments' evening.
//!
//! The crowd and the gate are the original's ([`crate::crowd`]). The
//! goals come from the match engine there; until it is ported, the replica
//! scores these games as it scores the leagues' ([`crate::matches`]): by
//! the teams' strengths and a luck, a tie in a tournament decided by extra
//! time and then penalties.

use crate::calendar::Calendar;
use crate::finances::CLUB_TO_TEAM;
use crate::matches::{EVEN, HOME_EDGE, goals};
use crate::presale::PresaleTables;
use crate::random::Random;
use crate::records::Records;
use crate::tournament::{self, Fixture, Outcome};

/// Extra time is a third of a game.
const EXTRA_PART: u8 = 3;
/// A friendly's round index for its crowd; a Turnier's by its game.
const FRIENDLY_ROUND: i16 = 5;
const TURNIER_ROUNDS: [i16; 4] = [3, 3, 2, 5];
/// The referees a game draws from (`FUN_006f3b7a`'s first loop).
const REFEREES: u32 = 40;
/// The fee's unit (`DAT_00a3b3e4`).
const THOUSAND: i32 = 1000;
/// The club's money the friendly books (club offsets): the other costs
/// and incomes, the gate of friendlies; a human's extra block keeps the
/// last friendly's share (club +0xdfc, extra +0x2560).
const OTHER_COSTS: usize = 0x2c0;
const OTHER_INCOME: usize = 0x2c4;
const GATE: usize = 0x23c;
const LAST_SHARE: usize = 0x2560;
/// Mood, respect and will within their bounds.
const MOST: i32 = 200;

/// What the human should see of the day's tournaments: the Turniere drawn
/// today (their pairings) and those whose day ended (their results).
#[derive(Default, Debug)]
pub struct Shown {
    pub drawn: Vec<u32>,
    pub results: Vec<u32>,
}

/// A game of the day's list: the sides and the ground, a friendly's id or
/// a tournament's fixture.
enum Game {
    Friendly { id: u32, home: (u8, i16), away: (u8, i16), fee: i32 },
    Tournament(Fixture),
}

impl Game {
    fn sides(&self) -> ((u8, i16), (u8, i16), (u8, i16)) {
        match self {
            Game::Friendly { home, away, .. } => (*home, *away, *home),
            Game::Tournament(f) => (f.home, f.away, f.ground),
        }
    }
}

/// The day's matches (`FUN_007615bb`) on day `today`.
pub fn play_day(records: &mut Records, calendar: &Calendar, tables: &PresaleTables, today: i16, random: &mut Random) -> Shown {
    let mut games = friendlies(records, today);
    let fixtures = tournament::day_games(records, calendar, today, random);
    let mut shown = Shown::default();
    for fixture in &fixtures {
        // A human in its four sees its pairings (`FUN_0062622f`).
        if fixture.game == tournament::SEMI_ONE && humans_in(records, fixture.id) {
            shown.drawn.push(fixture.id);
        }
    }
    games.extend(fixtures.into_iter().map(Game::Tournament));
    // The first loop: each game's referee drawn, its crowd and gate.
    let gates: Vec<crate::crowd::Gate> = games
        .iter()
        .map(|game| {
            random.below(REFEREES);
            let round = match game {
                Game::Friendly { .. } => FRIENDLY_ROUND,
                Game::Tournament(f) => TURNIER_ROUNDS[f.game],
            };
            crate::crowd::gate(records, calendar, tables, game.sides(), round, random)
        })
        .collect();
    // The games scored; their results move the clubs (`FUN_00714236`).
    let outcomes: Vec<Outcome> = games
        .iter()
        .zip(&gates)
        .map(|(game, gate)| {
            let decided = matches!(game, Game::Tournament(_));
            let (home, away, ground) = game.sides();
            let mut outcome = score(records, home, away, ground, decided, random);
            outcome.crowd = gate.crowd;
            outcome.gate = gate.gate;
            outcome
        })
        .collect();
    for (game, outcome) in games.iter().zip(&outcomes) {
        let (home, away, _) = game.sides();
        after_result(records, home, away, i32::from(outcome.goals.0) - i32::from(outcome.goals.1));
    }
    for (game, outcome) in games.iter().zip(&outcomes) {
        match game {
            Game::Friendly { id, home, away, fee } => played(records, *id, *home, *away, *fee, outcome),
            Game::Tournament(fixture) => tournament::record(records, fixture, outcome),
        }
    }
    shown.results = tournament::evening(records, today);
    shown
}

/// Today's friendlies into the list, in the list's order
/// (`FUN_00625212`, `FUN_00625310`): the home side the booking club unless
/// it plays away; the fee paid by the home club to the visitors as the
/// match begins, both ways the same. The first pre-season's still days
/// play none.
fn friendlies(records: &mut Records, today: i16) -> Vec<Game> {
    if tournament::still(records, today) {
        return Vec::new();
    }
    let mut games = Vec::new();
    for f in crate::friendly::friendlies(records).into_iter().filter(|f| f.day == today) {
        let (home, away) = if f.away { (f.opponent, f.club) } else { (f.club, f.opponent) };
        let fee = i32::from(f.fee as i16) * THOUSAND;
        book(records, home, OTHER_COSTS, fee);
        book(records, away, OTHER_INCOME, fee);
        games.push(Game::Friendly { id: f.id, home, away, fee });
    }
    games
}

fn humans_in(records: &Records, id: u32) -> bool {
    tournament::find(records, id)
        .is_some_and(|t| t.clubs().iter().take(4).any(|&(c, n)| crate::statistics::is_human(records, c, n)))
}

fn team(records: &Records, (country, number): (u8, i16)) -> Option<&crate::records::Team> {
    u16::try_from(number).ok().and_then(|n| records.team(country.into(), n))
}

fn team_mut(records: &mut Records, (country, number): (u8, i16)) -> Option<&mut crate::records::Team> {
    u16::try_from(number).ok().and_then(|n| records.team_mut(country.into(), n))
}

/// Books money onto a club's record by its club-object offset.
fn book(records: &mut Records, club: (u8, i16), at: usize, amount: i32) {
    if let Some(t) = team_mut(records, club) {
        let at = at + CLUB_TO_TEAM;
        t.put::<i32>(at, t.get::<i32>(at).wrapping_add(amount));
    }
}

/// A club's strengths in goal, defence, midfield and attack, in tenths;
/// none held, even ones.
fn strengths(records: &Records, club: (u8, i16)) -> [i16; 4] {
    let held = team(records, club).map_or(EVEN, |t| t.strengths());
    if held == [0; 4] { EVEN } else { held }
}

/// The side's strength on the day (club +0xdf2, which `FUN_006f6fc7` sets
/// at the kick-off from the side's parts): here their sum in points.
fn strength(records: &Records, club: (u8, i16)) -> i32 {
    strengths(records, club).iter().map(|&s| i32::from(s)).sum::<i32>() / 10
}

/// A game scored as the replica scores the leagues': the side at its
/// ground with its edge; a tie in a tournament goes to extra time, then to
/// penalties, which only name the winner.
fn score(records: &Records, home: (u8, i16), away: (u8, i16), ground: (u8, i16), decided: bool, random: &mut Random) -> Outcome {
    let edge = if home == ground {
        HOME_EDGE
    } else if away == ground {
        -HOME_EDGE
    } else {
        0
    };
    let (hs, as_) = (strengths(records, home), strengths(records, away));
    let (h, _) = goals(hs, as_, edge, random);
    let (a, _) = goals(as_, hs, -edge, random);
    let mut outcome = Outcome { goals: (h, a), ..Outcome::default() };
    if decided && h == a {
        outcome.extra = true;
        let (eh, _) = goals(hs, as_, edge, random);
        let (ea, _) = goals(as_, hs, -edge, random);
        let (eh, ea) = (eh / EXTRA_PART, ea / EXTRA_PART);
        outcome.goals = (h + eh, a + ea);
        if eh == ea {
            // The shootout's goals count with the score, as the original's
            // Turniere keep them ("6:5" after penalties).
            outcome.penalties = true;
            let won = 3 + random.below(3) as u8;
            let lost = won - 1 - random.below(2) as u8;
            outcome.goals = if random.below(2) == 0 {
                (outcome.goals.0 + won, outcome.goals.1 + lost)
            } else {
                (outcome.goals.0 + lost, outcome.goals.1 + won)
            };
        }
    }
    // The winning side by the goals, the away side on a draw
    // (`FUN_006f3b7a`'s third loop).
    outcome.winner = u8::from(outcome.goals.0 <= outcome.goals.1);
    outcome
}

/// What a result does to both clubs (`FUN_00714236`): a win lifts the
/// mood, more against a stronger side — and the will and the respect with
/// it —, a big one against a not much weaker one; a loss to a side not
/// much weaker costs will and mood, a heavy one more; a draw moves the
/// stronger side down and the weaker up when they stand more than five
/// apart.
fn after_result(records: &mut Records, home: (u8, i16), away: (u8, i16), d: i32) {
    let (sh, sa) = (strength(records, home), strength(records, away));
    let won = |records: &mut Records, c: (u8, i16), (s, o): (i32, i32), d: i32| {
        change(records, c, 1, 0, 0);
        if s < o - 4 {
            change(records, c, 2, 3, 2);
        }
        if d > 4 && s <= o + 6 {
            change(records, c, 1, 1, 1);
        }
    };
    let lost = |records: &mut Records, c: (u8, i16), (s, o): (i32, i32), d: i32| {
        if s > o - 10 {
            change(records, c, -4, 0, -1);
        }
        if d < -4 {
            change(records, c, -2, 0, -1);
        }
    };
    match d.signum() {
        1 => {
            won(records, home, (sh, sa), d);
            lost(records, away, (sa, sh), -d);
        }
        -1 => {
            lost(records, home, (sh, sa), d);
            won(records, away, (sa, sh), -d);
        }
        _ if sh > sa + 5 => {
            change(records, home, -1, -1, 0);
            change(records, away, 2, 2, 0);
        }
        _ if sh < sa - 5 => {
            change(records, home, 1, 1, 0);
            change(records, away, -1, -1, 0);
        }
        _ => {}
    }
}

/// A club's mood (club +0x9e) and respect (+0x92) within 0 and 200, its
/// will (team +0x95a) within ±200.
fn change(records: &mut Records, club: (u8, i16), mood: i32, respect: i32, will: i32) {
    let Some(t) = team_mut(records, club) else { return };
    if mood != 0 {
        t.set_mood((i32::from(t.mood()) + mood).clamp(0, MOST) as u8);
    }
    if respect != 0 {
        t.set_respect((i32::from(t.respect()) + respect).clamp(0, MOST) as i16);
    }
    if will != 0 {
        t.set_will_to_win((i32::from(t.will_to_win()) + will).clamp(-MOST, MOST) as i16);
    }
}

/// A friendly played (`FUN_0062528f`, `FUN_0062555e`): its goals into the
/// list; the gate shared in halves without a fee, else all the home
/// club's — a human's extra block keeping its share —; and the respect
/// and the managers' popularity by the result (`FUN_006256cc`).
fn played(records: &mut Records, id: u32, home: (u8, i16), away: (u8, i16), fee: i32, outcome: &Outcome) {
    crate::friendly::set_goals(records, id, outcome.goals);
    let g = outcome.gate;
    let share = |records: &mut Records, club: (u8, i16), amount: i32| {
        book(records, club, GATE, amount);
        let slot = team(records, club).map_or(0, |t| t.human());
        if let Some(extra) = usize::from(slot).checked_sub(1).and_then(|s| records.extras.get_mut(s)) {
            extra.put::<i32>(LAST_SHARE, amount);
        }
    };
    if fee == 0 {
        share(records, home, g / 2);
        share(records, away, g / 2);
    } else {
        share(records, home, g);
    }
    let d = i32::from(outcome.goals.0) - i32::from(outcome.goals.1);
    standing(records, home, away, d);
    standing(records, away, home, -d);
}

/// What a friendly's result does to a club's respect and its managers'
/// popularity (`FUN_006256cc`): a loss costs a point of respect, more as
/// the stronger side by how much stronger and a point with the public,
/// the fans and the media; a win gains one, against a stronger side two
/// more and a point with the fans.
fn standing(records: &mut Records, club: (u8, i16), other: (u8, i16), d: i32) {
    let (s, o) = (strength(records, club), strength(records, other));
    if d < 0 {
        change(records, club, 0, -1, 0);
        if s > o {
            popularity(records, club, [-1, -1, -1]);
            let by = if s > o + 50 {
                -10
            } else if s > o + 25 {
                -5
            } else {
                -2
            };
            change(records, club, 0, by, 0);
        }
    } else if d > 0 {
        change(records, club, 0, 1, 0);
        if s < o {
            popularity(records, club, [0, 1, 0]);
            change(records, club, 0, 2, 0);
        }
    }
}

/// The popularity with the public, the fans and the media of a human
/// club's trainer and, where another, its manager (`FUN_00418065`), each
/// within 0 and 100. A computer club's staff persons are not followed.
fn popularity(records: &mut Records, club: (u8, i16), by: [i32; 3]) {
    let Some(humans) = team(records, club).map(|t| t.humans()) else { return };
    let mut managers = vec![humans[0]];
    if humans[1] != 0 && humans[1] != humans[0] {
        managers.push(humans[1]);
    }
    for m in managers {
        if let Some(manager) = usize::from(m).checked_sub(1).and_then(|i| records.managers.get_mut(i)) {
            let p = manager.popularity();
            manager.set_popularity(std::array::from_fn(|k| (i32::from(p[k]) + by[k]).clamp(0, 100) as u8));
        }
    }
}

#[cfg(test)]
mod saves {
    use super::*;
    use a3_files::an3::Save;

    /// A friendly with Sporting (fee 140 thousand, at Union's ground) and
    /// one with Altglienicke for the gate, booked on A for the 17th of
    /// September (day 72) and played that day: Union pays Sporting the fee
    /// and keeps the whole gate, which its manager's extra block keeps as
    /// the last friendly's share; Altglienicke and Union share theirs in
    /// halves; each game counts with Union's season of them; the list keeps
    /// the goals.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn booked_friendlies_are_played_on_their_day() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let data = crate::data::Data::load(&dir).unwrap();
        let keeps = dir.parent().unwrap().join("saves-kept");
        let mut records = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let union = (14, 84);
        let sporting = crate::friendly::Candidate { country: 51, number: 0, away: 0, class: 3, fee: 140 };
        assert!(crate::friendly::book(&mut records, union, 1, &sporting, 72));
        let money = |r: &Records, c: (u8, i16), at: usize| team(r, c).map_or(0, |t| t.get::<i32>(at + CLUB_TO_TEAM));
        let (costs, gate, received, together) =
            (money(&records, union, OTHER_COSTS), money(&records, union, GATE), money(&records, (51, 0), OTHER_INCOME), records.extras[0].together());
        let mut random = Random::new(4);
        play_day(&mut records, &data.calendar, &data.presale, 72, &mut random);
        assert_eq!(money(&records, union, OTHER_COSTS) - costs, 140_000);
        assert_eq!(money(&records, (51, 0), OTHER_INCOME) - received, 140_000);
        let won = money(&records, union, GATE) - gate;
        assert!(won > 0);
        assert_eq!(records.extras[0].get::<i32>(LAST_SHARE), won);
        assert_eq!(records.extras[0].together(), together + 1);
        let f = crate::friendly::friendlies(&records)[0];
        assert!(f.goals.0 < 10 && f.goals.1 < 10);

        // At the gate's halves, with a near club on the next free day.
        let near = crate::friendly::Candidate { country: 14, number: 107, away: 1, class: 3, fee: 0 };
        let day = (73..120).find(|&d| records.calendars[0].entry(d)[0] == 0).unwrap() as i16;
        assert!(crate::friendly::book(&mut records, union, 1, &near, day));
        let (g_union, g_near) = (money(&records, union, GATE), money(&records, (14, 107), GATE));
        play_day(&mut records, &data.calendar, &data.presale, day, &mut random);
        let (d_union, d_near) = (money(&records, union, GATE) - g_union, money(&records, (14, 107), GATE) - g_near);
        assert!(d_near > 0 && d_union == d_near, "{d_union} {d_near}");
        assert_eq!(records.extras[0].get::<i32>(LAST_SHARE), d_union);
        assert_eq!(records.extras[0].together(), together + 2);
    }
}
