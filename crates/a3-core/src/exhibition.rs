//! The day's matches outside the competitions (`FUN_007615bb`, from the
//! human clubs' day `FUN_0064f981`): the friendlies booked for the day
//! (`FUN_00625212`, `FUN_00625310`) and the tournaments' games
//! ([`crate::tournament::day_games`]) go into the day's match list; the
//! list is played (`FUN_006f3b7a`) and its results booked; then the
//! tournaments' evening.
//!
//! The crowd and the gate are the original's ([`crate::crowd`]). The
//! friendlies and the tournaments' games walk and play in the match
//! engine ([`crate::sim`]), a tournament's game one that must be won:
//! a tie goes to extra time, then to penalties.

use crate::finances::CLUB_TO_TEAM;
use crate::random::Random;
use crate::records::Records;
use crate::sim;
use crate::tournament::{self, Fixture, Outcome};

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
/// The staff person's byte at his record's +4 (+6 of the copy
/// `FUN_006317c6` makes), the kind `FUN_0040a1ec` answers with.
const PERSON_KIND: usize = 0x24;

/// What the human should see of the day's tournaments: the Turniere drawn
/// today (their pairings) and those whose day ended (their results).
#[derive(Default, Debug)]
pub struct Shown {
    pub drawn: Vec<u32>,
    pub results: Vec<u32>,
    /// The halls played that a human took part in.
    pub halls: Vec<crate::hall::Hall>,
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

/// The day's matches (`FUN_007615bb`) on day `today`: the list walked
/// as `FUN_006f3b7a` walks it. Its first loop draws each game's referee
/// (one throw of forty), reckons its crowd and gate and the crowd's lean
/// (`FUN_0078b26c`) and lines up its computer sides (`FUN_0042a702`);
/// the second enters each game's men, kicks them off and chooses their
/// tactics ([`sim::exhibition_kick_off`]), a side whose formation stands
/// fewer than seven men losing its game ([`forfeit`]); the minutes play
/// the row (`FUN_006f352e` in its plain mode) — a tournament's game, kind
/// 0x80000, into extra time and penalties when level — and its plain
/// pass wears the men; then each game's result moves the clubs
/// (`FUN_00714236`) by their strengths on the day.
pub fn play_day(data: &crate::data::Data, records: &mut Records, today: i16, random: &mut Random) -> Shown {
    let (calendar, tables) = (&data.calendar, &data.presale);
    let mut games = friendlies(records, today);
    let (fixtures, halls) = tournament::day_games(data, records, today, random);
    let mut shown = Shown { halls, ..Shown::default() };
    for fixture in &fixtures {
        // A human in its four sees its pairings (`FUN_0062622f`).
        if fixture.game == tournament::SEMI_ONE && humans_in(records, fixture.id) {
            shown.drawn.push(fixture.id);
        }
    }
    games.extend(fixtures.into_iter().map(Game::Tournament));
    // The games' records of the row: the sides, the ground, the list's
    // mark at +0x24 — a friendly's 1 (`FUN_00625310`), a tournament's 2
    // with its kind 0x80000, a game that must be won (`FUN_00625974`) —
    // and the row's place.
    let mut row: Vec<sim::Match> = games
        .iter()
        .enumerate()
        .map(|(index, game)| {
            let (home, away, ground) = game.sides();
            let mut m = sim::Match::default();
            m.ids = [home.0.into(), home.1 as u16, away.0.into(), away.1 as u16];
            m.home_nation = ground.0;
            m.home_id = ground.1 as u16;
            if let Game::Tournament(_) = game {
                m.kind = sim::MAY_EXTRA;
            }
            m.exhibition = true;
            m.index = index as u16;
            m
        })
        .collect();
    // The first loop: each game's referee drawn, its crowd and gate, and
    // its computer sides lined up (`FUN_0042a702`).
    let mut gates = Vec::with_capacity(games.len());
    for (k, game) in games.iter().enumerate() {
        random.below(REFEREES);
        let round = match game {
            Game::Friendly { .. } => FRIENDLY_ROUND,
            Game::Tournament(f) => TURNIER_ROUNDS[f.game],
        };
        let (gate, [h, a]) = crate::crowd::gate(records, calendar, tables, game.sides(), round, random);
        row[k].form = sim::crowd_lean(h, a);
        gates.push(gate);
        let (home, away, _) = game.sides();
        for (country, number) in [home, away] {
            if let Ok(number) = u16::try_from(number) {
                crate::autolineup::line_up_computer(records, data, country.into(), number);
            }
        }
    }
    // The second: each game's men entered and kicked off, each computer
    // side's tactics chosen against the other (`FUN_0042c1aa`), the home
    // side first.
    for m in &mut row {
        sim::exhibition_kick_off(m, records, data, random);
        forfeit(m, records);
    }
    if !row.is_empty() {
        let date = crate::calendar::date(records.globals.start_year().into(), today.max(0) as usize);
        sim::play_matches(&mut row, false, Some(records), None, date, random);
    }
    // The games scored by their minutes (`FUN_006f3b7a`'s last loop): the
    // goals (+0x0e, +0x10, a shootout's counted in) into the game's bytes
    // +8 and +9, the winning side by them, the away side on a draw
    // (+0x0d); a tournament keeps whether extra time and penalties were
    // played (`FUN_00625d73`, the record's +0x1d bits 0x80 and 0x40).
    let outcomes: Vec<Outcome> = row
        .iter()
        .zip(&gates)
        .map(|(m, gate)| {
            let [h, a] = m.goals.map(|g| u8::try_from(g.max(0)).unwrap_or(u8::MAX));
            Outcome {
                goals: (h, a),
                extra: m.flags & sim::EXTRA != 0,
                penalties: m.flags & sim::PENS != 0,
                winner: u8::from(h <= a),
                crowd: gate.crowd,
                gate: gate.gate,
            }
        })
        .collect();
    // Each game's result moves the clubs (`FUN_00714236`) by their
    // strengths on the day; the walk's plain pass wore the men.
    let strengths: Vec<(i32, i32)> = row.iter().map(|m| (i32::from(m.sides[0].day[0]), i32::from(m.sides[1].day[0]))).collect();
    for ((game, outcome), &on_day) in games.iter().zip(&outcomes).zip(&strengths) {
        let (home, away, _) = game.sides();
        after_result(records, home, away, i32::from(outcome.goals.0) - i32::from(outcome.goals.1), on_day);
    }
    for ((game, outcome), &on_day) in games.iter().zip(&outcomes).zip(&strengths) {
        match game {
            Game::Friendly { id, home, away, fee } => played(records, *id, (*home, *away), *fee, outcome, on_day),
            Game::Tournament(fixture) => tournament::record(records, fixture, outcome),
        }
    }
    shown.results = tournament::evening(records, today);
    shown
}

/// Fewer men than this in a side's formation lose its game
/// (`FUN_006f6bd0` over the club's +0xa9a).
const FEWEST_MEN: usize = 7;
/// The record's flags (+0x1d) a lost game sets beside its end: the home
/// side short, the away side short.
const SHORT_HOME: u8 = 4;
const SHORT_AWAY: u8 = 2;
/// The goals the full side is given.
const GIVEN: i16 = 3;

/// A side whose formation stands fewer than seven men (`FUN_006f3b7a`'s
/// second loop, `FUN_006f6bd0` counting the eleven's places): the game
/// ends before its first minute, three to nought for the other side —
/// nought to nought when both stand short —, and the short side's
/// trainer and manager lose five with the public, the fans and the
/// media and ten of the board's confidence (`FUN_00418065`).
fn forfeit(m: &mut sim::Match, records: &mut Records) {
    let sides = [0, 1].map(|w| (m.ids[w * 2] as u8, m.ids[w * 2 + 1] as i16));
    let short = sides.map(|c| team(records, c).map_or(0, |t| t.lineup().iter().filter(|&&id| id != 0).count()) < FEWEST_MEN);
    let (flags, goals) = match short {
        [true, true] => (SHORT_HOME | SHORT_AWAY, [0, 0]),
        [true, false] => (SHORT_HOME, [0, GIVEN]),
        [false, true] => (SHORT_AWAY, [GIVEN, 0]),
        [false, false] => return,
    };
    m.flags |= sim::FINISHED | flags;
    m.goals = goals;
    m.half = [0, 0];
    for (club, _) in sides.into_iter().zip(short).filter(|&(_, s)| s) {
        move_standing(records, club, [-5, -5, -5], -10);
    }
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

/// A computer club's tactics against the other side (`FUN_0042c1aa`;
/// a human club keeps its own): normal effort and hardness, no flags,
/// and its tilt at four at home, three away. Under a trainer whose kind
/// (his person's +4, `FUN_0040a1ec`; none, two) stands outside 0 to 3 it
/// plays harder — effort two, hardness one — where both clubs' table
/// places (club +0x109) sum under eight, and all out against a side more
/// than fourteen stronger on the day (club +0xdf2). Its passing is rolled
/// short-long and safe-risky at three or four, keeping position at one or
/// two; then a side more than twenty five, and again more than fifty,
/// stronger tilts one forward (up to six), one that much weaker one back.
/// `ahead` is how far its strength on the day lies past the other's
/// (club +0xdf2 less the other's, a short).
pub(crate) fn computer_tactics(
    records: &mut Records,
    club: (u8, i16),
    other: (u8, i16),
    home: bool,
    ahead: i32,
    random: &mut Random,
) {
    let Some(own) = team(records, club) else { return };
    if own.humans()[0] != 0 {
        return;
    }
    let staff = own.staff();
    let trainer = if staff[0] != 0 { staff[0] } else { staff[1] };
    let kind = match trainer {
        0 => 2,
        id => records.persons.get(usize::from(id)).map_or(2, |p| p.get::<i8>(PERSON_KIND)),
    };
    let together = u32::from(own.table_place()) + team(records, other).map_or(0, |t| u32::from(t.table_place())) < 8;
    let mut tilt: u8 = if home { 4 } else { 3 };
    let (mut effort, mut hardness) = (4, 2);
    if !(0..=3).contains(&kind) {
        if together {
            (effort, hardness) = (2, 1);
        }
        if ahead < -14 {
            effort = 0;
        }
    }
    let passing = [3 + random.below(2), 3 + random.below(2), 1 + random.below(2)];
    for (by, step) in [(50, 1i8), (25, 1), (-50, -1), (-25, -1)] {
        let moves = if step > 0 { ahead > by && tilt < 6 } else { ahead < by && tilt != 0 };
        if moves {
            tilt = tilt.wrapping_add_signed(step);
        }
    }
    let Some(own) = team_mut(records, club) else { return };
    own.set_style(tilt);
    own.set_effort(effort);
    own.set_hardness(hardness);
    own.set_tactic_flags(0);
    let word = own.passing() & !0x1ff | passing[0] as u16 | (passing[1] as u16) << 3 | (passing[2] as u16) << 6;
    own.set_passing(word);
}

/// What a result does to both clubs (`FUN_00714236`): a win lifts the
/// mood, more against a stronger side — and the will and the respect with
/// it —, a big one against a not much weaker one; a loss to a side not
/// much weaker costs will and mood, a heavy one more; a draw moves the
/// stronger side down and the weaker up when they stand more than five
/// apart.
fn after_result(records: &mut Records, home: (u8, i16), away: (u8, i16), d: i32, (sh, sa): (i32, i32)) {
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
/// and the managers' popularity by the result and the strengths on the
/// day (`FUN_006256cc`).
fn played(
    records: &mut Records,
    id: u32,
    (home, away): ((u8, i16), (u8, i16)),
    fee: i32,
    outcome: &Outcome,
    (sh, sa): (i32, i32),
) {
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
    standing(records, home, (sh, sa), d);
    standing(records, away, (sa, sh), -d);
}

/// What a friendly's result does to a club's respect and its managers'
/// popularity (`FUN_006256cc`): a loss costs a point of respect, more as
/// the stronger side by how much stronger and a point with the public,
/// the fans and the media; a win gains one, against a stronger side two
/// more and a point with the fans.
fn standing(records: &mut Records, club: (u8, i16), (s, o): (i32, i32), d: i32) {
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

/// The popularity with the public, the fans and the media of a club's
/// trainer and, where another, its manager (`FUN_00418065`), each within
/// 0 and 100: a human's own (his record's `popularity`, the manager
/// object's +0x3a), and where the place stands without one the staff
/// person of the club's `staff` row — the trainer's place taking the
/// first staff person standing, the manager's place the third
/// (`FUN_006317c6`, `FUN_006318f4`: his +0x4a to +0x4c).
fn popularity(records: &mut Records, club: (u8, i16), by: [i32; 3]) {
    move_standing(records, club, by, 0);
}

/// [`popularity`] with the board's confidence beside (`FUN_00418065`'s
/// fourth word, `FUN_00418597`: a human's record +0x184 within 0 and
/// 100; a staff person's is dropped with the copy it moves).
pub(crate) fn move_standing(records: &mut Records, club: (u8, i16), by: [i32; 3], confidence: i32) {
    let Some(team) = team(records, club) else { return };
    let [trainer, manager] = team.humans();
    let staff = team.staff();
    let mut managers = Vec::new();
    let mut persons = Vec::new();
    if trainer != 0 {
        managers.push(trainer);
    } else {
        let person = if staff[0] != 0 { staff[0] } else { staff[1] };
        if person != 0 {
            persons.push(person);
        }
    }
    if manager != 0 && manager != trainer {
        managers.push(manager);
    } else if staff[2] != 0 {
        persons.push(staff[2]);
    }
    for m in managers {
        if let Some(manager) = usize::from(m).checked_sub(1).and_then(|i| records.managers.get_mut(i)) {
            let p = manager.popularity();
            manager.set_popularity(std::array::from_fn(|k| (i32::from(p[k]) + by[k]).clamp(0, 100) as u8));
            if confidence != 0 {
                manager.set_confidence((i32::from(manager.confidence()) + confidence).clamp(0, 100) as i16);
            }
        }
    }
    for person in persons {
        if (1..=1499).contains(&person)
            && let Some(p) = records.persons.get_mut(usize::from(person))
        {
            for (at, by) in (0x4a..0x4d).zip(by) {
                let value = p.get::<u8>(at);
                p.put::<u8>(at, (i32::from(value) + by).clamp(0, 100) as u8);
            }
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
        play_day(&data, &mut records, 72, &mut random);
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
        play_day(&data, &mut records, day, &mut random);
        let (d_union, d_near) = (money(&records, union, GATE) - g_union, money(&records, (14, 107), GATE) - g_near);
        assert!(d_near > 0 && d_union == d_near, "{d_union} {d_near}");
        assert_eq!(records.extras[0].get::<i32>(LAST_SHARE), d_union);
        assert_eq!(records.extras[0].together(), together + 2);
    }

    /// The friendlies walk in the engine: on A's 17th of September Union's
    /// friendly with Sporting kicks off, and the team record keeps the
    /// strength on the day the kick-off reckoned (club +0xdf2, team
    /// +0x304); a Union standing six men in its formation loses its next
    /// friendly before the first minute, nought to three in the list,
    /// its manager ten points of the board's confidence the poorer.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn friendlies_take_their_goals_from_the_walk() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let data = crate::data::Data::load(&dir).unwrap();
        let keeps = dir.parent().unwrap().join("saves-kept");
        let mut records = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let union = (14, 84);
        let sporting = crate::friendly::Candidate { country: 51, number: 0, away: 0, class: 3, fee: 140 };
        assert!(crate::friendly::book(&mut records, union, 1, &sporting, 72));
        let mut m = sim::Match { ids: [14, 84, 51, 0], ..sim::Match::default() };
        sim::kick_off_strengths(&mut m, &records, &data, &mut Random::new(1));
        let mut random = Random::new(4);
        play_day(&data, &mut records, 72, &mut random);
        assert_eq!(team(&records, union).unwrap().day_strength(), m.sides[0].day[0]);
        assert!(m.sides[0].day[0] > 0);

        let near = crate::friendly::Candidate { country: 14, number: 107, away: 0, class: 3, fee: 0 };
        let day = (73..120).find(|&d| records.calendars[0].entry(d)[0] == 0).unwrap() as i16;
        assert!(crate::friendly::book(&mut records, union, 1, &near, day));
        let t = team_mut(&mut records, union).unwrap();
        let mut lineup = t.lineup();
        lineup[6..].fill(0);
        t.set_lineup(lineup);
        let manager = usize::from(team(&records, union).unwrap().humans()[0]) - 1;
        let confidence = records.managers[manager].confidence();
        play_day(&data, &mut records, day, &mut random);
        let f = crate::friendly::friendlies(&records).into_iter().find(|f| f.day == day).unwrap();
        assert_eq!(f.goals, (0, 3));
        assert_eq!(records.managers[manager].confidence(), (confidence - 10).max(0));
    }

    /// Probe1's Turniere of days 35 to 39 walk in the engine (days 35 to
    /// 40): every game must be won — none ends level, a tie went to extra
    /// time and some to penalties, whose goals count in —, the winner
    /// byte names the side ahead, and each club of the four keeps the
    /// strength on the day its kick-off reckoned (team +0x304).
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn tournaments_take_their_goals_from_the_walk() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let data = crate::data::Data::load(&dir).unwrap();
        let keeps = dir.parent().unwrap().join("saves-kept");
        let mut records = Records::from_save(Save::read(&keeps.join("Probe1.an3")).unwrap()).unwrap();
        let mut random = Random::new(5);
        for day in 35..=40 {
            play_day(&data, &mut records, day, &mut random);
        }
        let played: Vec<_> = tournament::tournaments(&records)
            .into_iter()
            .filter(|t| t.kind() == tournament::TURNIER && (35..=39).contains(&t.day()))
            .collect();
        assert_eq!(played.len(), 7);
        let (mut extra, mut shootouts) = (0, 0);
        for t in &played {
            for g in 0..4 {
                let ((h, a), went_extra, pens, winner) = t.result(g).unwrap();
                assert_ne!(h, a, "{:08x} game {g}", t.id());
                assert_eq!(winner, u8::from(h < a));
                assert!(!pens || went_extra);
                extra += usize::from(went_extra);
                shootouts += usize::from(pens);
            }
            for club in t.clubs() {
                assert!(team(&records, club).unwrap().day_strength() > 0, "{club:?}");
            }
        }
        assert!(extra > 0 && shootouts > 0, "{extra} {shootouts}");
    }

    /// Where a club stands without human trainer and manager, the staff
    /// persons of its `staff` row carry the likings a friendly's result
    /// moves (`FUN_00418065`): the first staff person standing — else the
    /// second — for the trainer's place, the third for the manager's,
    /// each on his +0x4a to +0x4c (`FUN_006317c6`, `FUN_006318f4`).
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_computer_clubs_staff_persons_carry_the_likings() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let mut records = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let club = (1..=200u16)
            .map(|n| (14, n))
            .filter_map(|(c, n)| records.team(c, n).map(|t| ((c, n), t.humans(), t.staff())))
            .filter(|&(_, h, s)| h == [0, 0] && s[2] != 0)
            .map(|(club, _, s)| (club, if s[0] != 0 { s[0] } else { s[1] }, s[2]))
            .find(|&(_, first, third)| {
                first != 0
                    && first != third
                    && (1..=1499).contains(&first)
                    && (1..=1499).contains(&third)
            })
            .expect("A holds a computer club with two staff persons");
        let ((country, number), first, third) = club;
        let likings = |records: &Records, p: u16| {
            [0x4a, 0x4b, 0x4c].map(|at| records.persons.get(usize::from(p)).unwrap().get::<u8>(at))
        };
        let (before_first, before_third) = (likings(&records, first), likings(&records, third));
        let step = |v: u8, by: i32| (i32::from(v) + by).clamp(0, 100) as u8;
        popularity(&mut records, (country as u8, number as i16), [0, 1, 0]);
        assert_eq!(likings(&records, first), [before_first[0], step(before_first[1], 1), before_first[2]]);
        assert_eq!(likings(&records, third), [before_third[0], step(before_third[1], 1), before_third[2]]);
    }
}
