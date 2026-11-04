//! A Hallenturnier or Strandturnier played (`FUN_0064ce47`), all on the
//! day after its first, when one of its eight is a human's: filled first
//! by its computer host, then two groups of four play each other — goals
//! drawn by the clubs' hall strengths —, their first two meet crosswise
//! in the semifinals, the losers shoot it out for third place, the winners
//! play the final. The host takes the gate and the television's money and
//! pays the guests' fees; the first four take prize money and the winner a
//! better mood; every club's players tire and a few get hurt. A human's
//! hall leaves its result for the screen (dialog 94).
//!
//! Original slips kept: a drawn knockout game goes to the away side; the
//! best scorer's tally counts the second semifinal with the first's goals
//! and the game for third with the second semifinal's strengths; the crowd
//! is reckoned from a running total.

use crate::random::Random;
use crate::records::Records;
use crate::tournament;

/// The clubs a hall needs; the fill's tries.
const CLUBS: usize = 8;
const FILL_TRIES: usize = 30;
/// The fill takes a club this near the host in three of ten.
const NEAR_TRIES: u32 = 3;
const NEAR: i32 = 100;
/// The hall strengths by division (`0x89ef64`), Germany's and France's
/// only; their last four run into the string after the table; outside the
/// leagues 2, other countries 5. A fit outfield player with technique
/// adds one, a training hall three.
const BASE: [i16; 10] = [6, 4, 3, 3, 3, 3, 21325, 24391, 18757, 16974];
const HALL_COUNTRIES: [u8; 2] = [14, 20];
const OUTSIDE_LEAGUES: i16 = 2;
const OTHER_COUNTRIES: i16 = 5;
const TECHNIQUE: u16 = 0x200;
const KEEPER: u8 = 1;
const FACILITIES: usize = 0x95c;
const TRAINING_HALL: u8 = 0x10;
const HALL_BONUS: i16 = 3;
/// The group games' pairs (`0x89ef4c`).
const PAIRS: [(usize, usize); 6] = [(0, 1), (2, 3), (2, 0), (3, 1), (0, 3), (1, 2)];
/// The crowd: a running total over the guests, a fourteenth of it, six
/// thousand at most; 60 a head; the television's 200 000; the costs shown.
const CROWD_PARTS: i32 = 14;
const CROWD_MOST: i32 = 6000;
const PER_HEAD: i32 = 60;
const TELEVISION: i32 = 200_000;
const SHOWN_COSTS: i32 = 100_000;
/// A hall's round index for the forecast (`R+0x26`).
const HALL_ROUND: i16 = 7;
/// The prize money of the first four, the winner's mood.
const PRIZE_STEP: i32 = 10_000;
const WINNER_MOOD: u8 = 10;
const MOOD_MOST: u8 = 200;
/// The club's money (club offsets): the gate of friendlies and
/// tournaments, the television's, the premiums received, other costs; the
/// medical department's budget.
const GATE: usize = 0x23c;
const TELEVISION_INCOME: usize = 0x244;
const PREMIUMS: usize = 0x248;
const OTHER_COSTS: usize = 0x2c0;
const MEDICAL: usize = 0x2ac;
/// The best scorer: an attacking midfielder or a forward, by tickets of
/// half his strength, two hundred at most.
const ATTACK_FROM: u8 = 9;
const TICKETS_MOST: usize = 200;

/// A game of the hall: its sides by their places among the eight, the
/// goals, and the copy of the regular time's the tallies read.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct HallGame {
    pub home: usize,
    pub away: usize,
    pub goals: (i16, i16),
    regular: (i16, i16),
}

/// A group's table row: the club's place among the eight, its games,
/// points, goals scored and conceded.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Row {
    pub club: usize,
    pub played: i16,
    pub points: i16,
    pub scored: i16,
    pub conceded: i16,
}

/// A hall's result as its screen shows it (`FUN_0064ce47`'s S).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Hall {
    pub id: u32,
    pub kind: u8,
    pub clubs: [(u8, i16); CLUBS],
    pub strengths: [i16; CLUBS],
    pub groups: [[HallGame; 6]; 2],
    pub tables: [[Row; 4]; 2],
    pub semis: [HallGame; 2],
    pub third: HallGame,
    pub final_game: HallGame,
    /// The first four by their places among the eight.
    pub ranks: [usize; 4],
    pub crowd: i32,
    pub takings: i32,
    pub fees: i32,
    pub costs: i32,
    /// The best scorer and his goals, the best keeper, by id.
    pub scorer: Option<(u16, i16)>,
    pub keeper: Option<u16>,
}

fn team(records: &Records, (country, number): (u8, i16)) -> Option<&crate::records::Team> {
    u16::try_from(number).ok().and_then(|n| records.team(country.into(), n))
}

fn book(records: &mut Records, (country, number): (u8, i16), at: usize, amount: i32) {
    if let Some(t) = u16::try_from(number).ok().and_then(|n| records.team_mut(country.into(), n)) {
        let at = at + crate::finances::CLUB_TO_TEAM;
        t.put::<i32>(at, t.get::<i32>(at).wrapping_add(amount));
    }
}

/// How many more days a player is out (the club object's +0x2c).
fn days_out(records: &Records, id: u16) -> i32 {
    records.player(id).map_or(0, |p| crate::load::days_to(&records.globals, p.injured_to()))
}

/// One more club for a computer host's hall short of its eight
/// (`FUN_0064cba7`): in three of ten the first club of the host's country,
/// by number, a hundred near, no human's and free on the hall's first
/// day; else one of its first division in a random order.
fn fill_one(records: &mut Records, calendar: &crate::calendar::Calendar, id: u32, random: &mut Random) {
    let Some(t) = tournament::find(records, id) else { return };
    let (country, number) = t.host();
    let day = t.day();
    let free = |records: &Records, club: (u8, i16)| {
        !crate::statistics::is_human(records, club.0, club.1) && !crate::friendly::busy(records, calendar, club, day)
    };
    let at = |records: &Records, club: (u8, i16)| team(records, club).map_or([0, 0], |t| t.map_at());
    let picked = if random.below(10) < NEAR_TRIES {
        let [hx, hy] = at(records, (country, number));
        (0..records.countries.team_count(country.into())).map(|c| (country, c)).find(|&club| {
            let [x, y] = at(records, club);
            let (dx, dy) = (i32::from(x) - i32::from(hx), i32::from(y) - i32::from(hy));
            dx * dx + dy * dy < NEAR * NEAR && free(records, club)
        })
    } else {
        let first: Vec<i16> = crate::opponent::League::of(records, country.into(), 0)
            .map(|league| (0..league.teams()).map(|place| league.team(place)).collect())
            .unwrap_or_default();
        let order = crate::promotion::permutation(first.len(), random);
        order.iter().map(|&k| (country, first[k])).find(|&club| free(records, club))
    };
    if let Some(club) = picked {
        tournament::take(records, id, club);
    }
}

/// A club's hall strength (`FUN_0064c588`).
fn strength(records: &Records, club: (u8, i16)) -> i16 {
    let Some(t) = team(records, club) else { return OTHER_COUNTRIES };
    let mut s = if !HALL_COUNTRIES.contains(&club.0) {
        OTHER_COUNTRIES
    } else if t.level() >= 10 {
        OUTSIDE_LEAGUES
    } else {
        BASE[usize::from(t.level())]
    };
    for id in t.players() {
        let Some(p) = records.player(id) else { continue };
        if days_out(records, id) == 0 && p.positions()[0] != KEEPER && p.good() & TECHNIQUE != 0 {
            s = s.wrapping_add(1);
        }
    }
    if t.get::<u8>(FACILITIES) & TRAINING_HALL != 0 {
        s = s.wrapping_add(HALL_BONUS);
    }
    s
}

/// A game by the two strengths (`FUN_0064c3f6`): each side's goals drawn
/// below its strength; no extra time in a hall.
fn game(home: usize, away: usize, strengths: &[i16; CLUBS], random: &mut Random) -> HallGame {
    let hg = random.below(strengths[home].max(0) as u32) as i16;
    let ag = random.below(strengths[away].max(0) as u32) as i16;
    HallGame { home, away, goals: (hg, ag), regular: (hg, ag) }
}

/// A group's table in its order (`FUN_0072b9b9` with `FUN_0072bc49`):
/// points, goal difference, goals scored, games played, a full tie in the
/// slots' order.
fn sort(rows: &mut [Row; 4]) {
    let less = |a: &Row, b: &Row| {
        let key = |r: &Row| (r.points, r.scored - r.conceded, r.scored, r.played);
        key(a) < key(b)
    };
    loop {
        let mut swapped = false;
        for i in 0..3 {
            if less(&rows[i], &rows[i + 1]) {
                rows.swap(i, i + 1);
                swapped = true;
            }
        }
        if !swapped {
            break;
        }
    }
}

/// The hall of `id` on the day after its first (`FUN_0064ce47`).
pub fn play(data: &crate::data::Data, records: &mut Records, id: u32, random: &mut Random) -> Option<Hall> {
    let t = tournament::find(records, id)?;
    if t.count() != CLUBS && t.clubs().iter().any(|&(c, n)| crate::statistics::is_human(records, c, n)) {
        let (hc, hn) = t.host();
        if !crate::statistics::is_human(records, hc, hn) {
            for _ in 0..FILL_TRIES {
                if tournament::find(records, id).is_none_or(|t| t.count() >= CLUBS) {
                    break;
                }
                fill_one(records, &data.calendar, id, random);
            }
        }
    }
    let t = tournament::find(records, id)?;
    if t.count() != CLUBS {
        return None;
    }
    let clubs: [(u8, i16); CLUBS] = std::array::from_fn(|i| t.clubs()[i]);
    let human = clubs.iter().any(|&c| team(records, c).is_some_and(|t| t.humans() != [0, 0]));
    let strengths: [i16; CLUBS] = std::array::from_fn(|i| strength(records, clubs[i]));
    let (crowd, takings, fees) = money(data, records, &clubs, random);

    // The groups: slots 0 to 3 and 4 to 7.
    let mut groups = [[HallGame::default(); 6]; 2];
    let mut tables = [[Row::default(); 4]; 2];
    for g in 0..2 {
        let rows = &mut tables[g];
        for (r, row) in rows.iter_mut().enumerate() {
            row.club = 4 * g + r;
        }
        for (n, &(x, y)) in PAIRS.iter().enumerate() {
            let played = game(4 * g + x, 4 * g + y, &strengths, random);
            let (hg, ag) = played.goals;
            let points = match hg.cmp(&ag) {
                std::cmp::Ordering::Greater => (3, 0),
                std::cmp::Ordering::Equal => (1, 1),
                std::cmp::Ordering::Less => (0, 3),
            };
            for (k, goals, against, p) in [(x, hg, ag, points.0), (y, ag, hg, points.1)] {
                rows[k].points += p;
                rows[k].played += 1;
                rows[k].scored += goals;
                rows[k].conceded += against;
            }
            groups[g][n] = played;
        }
        sort(rows);
    }
    let (a1, a2, b1, b2) = (tables[0][0].club, tables[0][1].club, tables[1][0].club, tables[1][1].club);

    // The semifinals crosswise, a draw going to the away side.
    let semis = [game(a1, b2, &strengths, random), game(b1, a2, &strengths, random)];
    let decide = |g: &HallGame| if g.goals.1 < g.goals.0 { (g.home, g.away) } else { (g.away, g.home) };
    let (w1, l1) = decide(&semis[0]);
    let (w2, l2) = decide(&semis[1]);
    // Third place on penalties (`FUN_0064c4ff`), its regular copy what the
    // second semifinal left on the stack: its two strengths.
    let mut th = 1 + random.below(5) as i16;
    let mut ta = 1 + random.below(5) as i16;
    if th == ta {
        if random.below(2) == 0 {
            ta += 1;
        } else {
            th += 1;
        }
    }
    let third = HallGame { home: l1, away: l2, goals: (th, ta), regular: (strengths[b1], strengths[a2]) };
    let final_game = game(w1, w2, &strengths, random);
    let (first, second) = decide(&final_game);
    let (third_place, fourth) = decide(&third);
    let ranks = [first, second, third_place, fourth];

    // The tallies for the best scorer and keeper.
    let (mut scored, mut conceded) = ([0i16; CLUBS], [0i16; CLUBS]);
    for row in tables.iter().flatten() {
        scored[row.club] += row.scored;
        conceded[row.club] += row.conceded;
    }
    let s0 = semis[0].regular;
    for (home, away) in [(a1, b2), (b1, a2)] {
        scored[home] += s0.0;
        conceded[home] += s0.1;
        scored[away] += s0.1;
        conceded[away] += s0.0;
    }
    for g in [third, final_game] {
        scored[g.home] += g.regular.0;
        conceded[g.home] += g.regular.1;
        scored[g.away] += g.regular.1;
        conceded[g.away] += g.regular.0;
    }
    let best = (0..CLUBS).fold(0, |b, i| if scored[i] > scored[b] { i } else { b });
    let keep = (0..CLUBS).fold(0, |b, i| if conceded[i] < conceded[b] { i } else { b });
    let (mut scorer, mut keeper) = (None, None);
    if human {
        let tickets = |club: (u8, i16), keepers: bool, most: usize| {
            let mut list = Vec::new();
            for id in team(records, club).map(|t| t.players()).unwrap_or_default() {
                let Some(p) = records.player(id) else { continue };
                let fits = if keepers { p.positions()[0] == KEEPER } else { p.positions()[0] >= ATTACK_FROM };
                if fits && days_out(records, id) <= 0 {
                    for _ in 0..(p.strength() >> 1) {
                        if list.len() < most {
                            list.push(id);
                        }
                    }
                }
            }
            list
        };
        let list = tickets(clubs[best], false, TICKETS_MOST);
        if !list.is_empty() {
            let id = list[random.below(list.len() as u32) as usize];
            let g = scored[best];
            let goals = (g + 2) / 3 + random.below((g / 6).max(0) as u32) as i16;
            scorer = Some((id, goals));
        }
        let list = tickets(clubs[keep], true, usize::MAX);
        if !list.is_empty() {
            keeper = Some(list[random.below(list.len() as u32) as usize]);
        }
    }

    // The prize money and the winner's mood.
    for (i, &rank) in ranks.iter().enumerate() {
        book(records, clubs[rank], PREMIUMS, PRIZE_STEP * (4 - i as i32));
        if i == 0
            && let Some(t) = u16::try_from(clubs[rank].1).ok().and_then(|n| records.team_mut(clubs[rank].0.into(), n))
        {
            t.set_mood(t.mood().wrapping_add(WINNER_MOOD).min(MOOD_MOST));
        }
    }
    tournament::set_takings(records, id, (crowd, takings, fees, SHOWN_COSTS));

    // Every club's players tire, and a few get hurt.
    let today = records.globals.get::<i16>(0xb5cbd4);
    for &club in &clubs {
        wear(data, records, club, today, random);
    }
    human.then(|| Hall {
        id,
        kind: t.kind(),
        clubs,
        strengths,
        groups,
        tables,
        semis,
        third,
        final_game,
        ranks,
        crowd,
        takings,
        fees,
        costs: SHOWN_COSTS,
        scorer,
        keeper,
    })
}

/// The money (`FUN_0064ce47`'s second part): each guest's forecast at the
/// host's ground — the hall's round, no class cut — adds its crowd to a
/// running total, which is summed; a fourteenth of the sum comes, six
/// thousand at most. Each guest's fee goes to its premiums and their sum
/// to the host's costs; the host takes 60 a head and the television's
/// 200 000. Its crowd, takings and fees.
fn money(data: &crate::data::Data, records: &mut Records, clubs: &[(u8, i16); CLUBS], random: &mut Random) -> (i32, i32, i32) {
    let host = clubs[0];
    let (mut running, mut sum, mut fees) = (0i32, 0i32, 0i32);
    for &guest in &clubs[1..] {
        let gate =
            crate::crowd::gate_cut(records, &data.calendar, &data.presale, (host, guest, host), HALL_ROUND, false, random);
        running = running.wrapping_add(gate.crowd);
        sum = sum.wrapping_add(running);
        let fee = tournament::fee(records, host, guest);
        book(records, guest, PREMIUMS, fee);
        fees += fee;
    }
    book(records, host, OTHER_COSTS, fees);
    let crowd = (sum / CROWD_PARTS).min(CROWD_MOST);
    book(records, host, GATE, crowd * PER_HEAD);
    book(records, host, TELEVISION_INCOME, TELEVISION);
    (crowd, crowd * PER_HEAD + TELEVISION, fees)
}

/// A club's players after the hall: in one of thirty two of them hurt, in
/// two of thirty one — a pick that falls on an injured player hurts
/// nobody —; then every player loses freshness and condition.
fn wear(data: &crate::data::Data, records: &mut Records, club: (u8, i16), today: i16, random: &mut Random) {
    let squad = team(records, club).map(|t| t.players()).unwrap_or_default();
    let count = squad.len() as u32;
    let r = random.below(30);
    let picks = match r {
        0 => 2,
        1 | 2 => 1,
        _ => 0,
    };
    for _ in 0..picks {
        let k = random.below(count) as usize;
        if let Some(&id) = squad.get(k)
            && days_out(records, id) == 0
        {
            injure(data, records, club, id, today, random);
        }
    }
    for &id in &squad {
        let Some(p) = records.player_mut(id) else { continue };
        let tire = |v: i16, by: i32| (i32::from(v) + by * 100).clamp(0, 10_000) as i16;
        let d = random.below(2) as i32;
        let mut f = p.freshness();
        f[0] = tire(f[0], -6 - d);
        p.set_freshness(f);
        let d = random.below(2) as i32;
        p.set_condition(tire(p.condition(), -3 - d));
    }
}

/// An injury in a hall (`FUN_00767225` with 0x26): a kind drawn by the
/// kinds' weights — one past the one the draw falls in, as the original
/// steps —, a torn muscle a worse one when the player is spent; its days
/// by the club's medical department, at least one; kinds 27 and 30 may
/// end in a knock (`FUN_007675c6`).
fn injure(data: &crate::data::Data, records: &mut Records, club: (u8, i16), id: u16, today: i16, random: &mut Random) {
    const KINDS: u8 = 61;
    const TORN: u8 = 37;
    const WORSE: u8 = 67;
    const SPENT: i32 = 35;
    const CAREER: [u8; 2] = [27, 30];
    const KNOCK: u16 = 8;
    let medical = &data.rules.medical;
    let total: u32 = (1..=KINDS).map(|k| u32::from(medical.weight(k))).sum();
    let r = random.below(total);
    let mut start = 0u32;
    let mut i = 0u8;
    while i < KINDS - 1 && start <= r {
        i += 1;
        start += u32::from(medical.weight(i));
    }
    let mut kind = i + 1;
    let Some(p) = records.player(id) else { return };
    let fresh = (p.freshness().iter().map(|&f| i32::from(f)).sum::<i32>() / 100).clamp(0, 100);
    if kind == TORN && fresh < SPENT {
        kind = WORSE;
    }
    let budget = team(records, club).map_or(0, |t| t.get::<i32>(MEDICAL + crate::finances::CLUB_TO_TEAM));
    let class = match budget {
        ..150_000 => 0,
        150_000..400_000 => 1,
        400_000..800_000 => 2,
        800_000..1_700_000 => 3,
        _ => 4,
    };
    let x = 100 - 3 * class - random.below((budget / 100_000).max(0) as u32) as i32;
    let days = ((f64::from(x as u32) * f64::from(f32::from(medical.base_days(kind)))) / 100.0) as i32;
    if let Some(p) = records.player_mut(id) {
        p.set_injury_kind(kind);
        p.set_injured_on(today as u16);
    }
    crate::medical::injure_for(records, id, days.max(1));
    if CAREER.contains(&kind)
        && let Some(p) = records.player(id)
    {
        let [d, m, y] = records.globals.date();
        let on = chrono::NaiveDate::from_ymd_opt(i32::from(y), u32::from(m), u32::from(d));
        let age = on.map_or(0, |on| crate::statistics::age_on(p, on));
        let mut chance = 200 - 9 * (40 - age);
        if p.traits()[1] & 1 != 0 || p.traits()[0] & 1 != 0 {
            chance -= 20;
        }
        if (random.below(200) as i32) < chance {
            let knocks = p.knocks();
            if knocks & KNOCK == 0 {
                if let Some(p) = records.player_mut(id) {
                    p.set_knocks(knocks | KNOCK);
                }
            } else {
                // The career's end on a day ahead (event 0x16), not ported.
                random.below((366 - i32::from(today)).max(0) as u32);
            }
        }
    }
}

#[cfg(test)]
mod saves {
    use super::*;
    use a3_files::an3::Save;

    /// Union hosts a Halle on A for a free day with seven of its country's
    /// clubs invited; played the day after: two groups of four, the eight
    /// in their tables, four ranks of distinct clubs; the crowd a
    /// fourteenth of the running total, six thousand at most, 60 a head
    /// and the television's 200 000 its takings; the first four's prize
    /// money, the winner's mood ten up; the host's television money, the
    /// guests' fees to its costs; seven forecasts counting with Union's
    /// season; every player more tired.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_hosted_hall_is_played_the_day_after() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let data = crate::data::Data::load(&dir).unwrap();
        let keeps = dir.parent().unwrap().join("saves-kept");
        let mut records = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let union = (14, 84);
        let day = (64..120).find(|&d| (d..=d + 1).all(|k| records.calendars[0].entry(k)[0] == 0)).unwrap() as i16;
        let id = tournament::open_own(&mut records, union, day, tournament::HALLE).unwrap();
        let guests: Vec<(u8, i16)> = (0..records.countries.team_count(14))
            .map(|n| (14, n))
            .filter(|&c| c != union && !crate::friendly::busy(&records, &data.calendar, c, day))
            .take(7)
            .collect();
        tournament::invite(&mut records, id, &guests).unwrap();
        let money = |r: &Records, c: (u8, i16), at: usize| team(r, c).map_or(0, |t| t.get::<i32>(at + crate::finances::CLUB_TO_TEAM));
        let clubs: Vec<(u8, i16)> = tournament::find(&records, id).unwrap().clubs();
        let premiums: Vec<i32> = clubs.iter().map(|&c| money(&records, c, PREMIUMS)).collect();
        let moods: Vec<u8> = clubs.iter().map(|&c| team(&records, c).unwrap().mood()).collect();
        let (tv, costs) = (money(&records, union, TELEVISION_INCOME), money(&records, union, OTHER_COSTS));
        let together = records.extras[0].together();
        let fresh: i32 = team(&records, union).unwrap().players().iter().map(|&p| i32::from(records.player(p).unwrap().freshness()[0])).sum();
        let mut random = Random::new(11);
        crate::exhibition::play_day(&data, &mut records, day, &mut random);
        let shown = crate::exhibition::play_day(&data, &mut records, day + 1, &mut random);
        assert_eq!(shown.halls.len(), 1);
        let hall = &shown.halls[0];
        let mut seen: Vec<usize> = hall.tables.iter().flatten().map(|r| r.club).collect();
        seen.sort();
        assert_eq!(seen, (0..8).collect::<Vec<_>>());
        let mut ranks = hall.ranks.to_vec();
        ranks.dedup();
        assert_eq!(ranks.len(), 4);
        assert!(hall.crowd > 0 && hall.crowd <= 6000);
        assert_eq!(hall.takings, hall.crowd * 60 + 200_000);
        let t = tournament::find(&records, id).unwrap();
        assert_eq!(t.takings(), (hall.crowd, hall.takings, hall.fees, 100_000));
        let fees: i32 = clubs[1..].iter().map(|&g| tournament::fee(&records, union, g)).sum();
        assert_eq!(hall.fees, fees);
        assert_eq!(money(&records, union, TELEVISION_INCOME) - tv, 200_000);
        assert_eq!(money(&records, union, OTHER_COSTS) - costs, fees);
        for (i, &c) in clubs.iter().enumerate() {
            let rank = hall.ranks.iter().position(|&r| r == i);
            let prize = rank.map_or(0, |r| 10_000 * (4 - r as i32)) + if i > 0 { tournament::fee(&records, union, c) } else { 0 };
            assert_eq!(money(&records, c, PREMIUMS) - premiums[i], prize, "{c:?}");
        }
        let winner = hall.ranks[0];
        assert_eq!(team(&records, clubs[winner]).unwrap().mood(), moods[winner].wrapping_add(10).min(200));
        assert_eq!(records.extras[0].together(), together + 7);
        let after: i32 = team(&records, union).unwrap().players().iter().map(|&p| i32::from(records.player(p).unwrap().freshness()[0])).sum();
        assert!(after < fresh);
    }
}
