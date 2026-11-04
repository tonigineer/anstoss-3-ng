//! The contracts that run out at the season's end and the free agents
//! they make (`FUN_006b3ee0`'s club pass `FUN_006b4459` → `FUN_006b3f92`,
//! and its last step `FUN_0040ec50`).
//!
//! Every club of the countries played lets go the players with no deal
//! for the season's end whose contract does not hold them on: one in its
//! last year, or one that holds him for no league as low as the club's
//! next (`FUN_004271fb`). A player not retiring joins the season's list of
//! free agents (`FUN_0040e8cc`, five thousand at most): out of the club,
//! his station closed, his season's counters cleared, the club's will to
//! win moved, off the transfer list (`FUN_0070e3f5`, `FUN_0075a636`); one
//! retiring, or with the list full, ends his career into the pool
//! (`FUN_0070e29e`).
//!
//! When the season's players are closed the free agents find clubs
//! (`FUN_0040ec50`): one over 33, or of a country not played, ends his
//! career. A young good player of the main country's top two leagues
//! stays with his club; else his league is reckoned by his age, his
//! promise (`FUN_006ff516`) and his talent — a weak talent's drawn —,
//! and of that league's computer clubs the one with the fewest at his
//! position takes him (`FUN_0040ea45`'s tables), or his old club's reserve
//! team while he is young and was paid. A club below 30 signs him on fresh
//! terms (`FUN_00709fb6`): his salary as he would ask it, bonuses by the
//! league, years by his age, a release fee for a good one at times, the
//! player's option at times; a full one sends him into the pool. The human
//! clubs keep their losses and gains in their histories.
//!
//! The club's youths too old for its youth next season join the list
//! behind its players (`FUN_006b4230`).
//!
//! Left out: the lists and notes beside (`FUN_005dbeb7`, `FUN_005dbefc`,
//! `FUN_00405bf3`).

use chrono::NaiveDate;

use crate::contract::{ContractTables, Signee};
use crate::random::Random;
use crate::records::{Player, Records, Team};
use crate::value::{ValueTables, Worth};

/// A free agent and the club he left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FreeAgent {
    pub id: u16,
    pub country: u8,
    pub team: i16,
}

const LIST_MOST: usize = 5000;
/// A youth born eighteen years before the next season's first of July
/// leaves the youth; R+0x26f's bits 2 and 3 go with him.
const YOUTH_YEARS: i32 = 18;
const YOUTH_BITS_KEPT: u8 = 0xf3;
const RETIRES: u8 = 0x80;
const LISTED: u8 = 1;
/// A free agent this old ends his career.
const TOO_OLD: i32 = 33;
/// A club this full takes no free agent.
const FULL: usize = 30;
/// The main country's leagues and the more countries' tables
/// (`FUN_0040ea45`'s ten tables, the more countries' from the seventh).
const MORE_TABLES: usize = 6;
/// What a free agent's strength may still become by his age and talent
/// (`DAT_008a7568`: ages 17 to 34, five talents a row; `DAT_008a75c2`
/// for the older).
const PROMISE: [[i8; 5]; 18] = [
    [3, 2, 2, 2, 1],
    [3, 2, 2, 1, 1],
    [2, 2, 2, 1, 1],
    [2, 2, 1, 1, 1],
    [2, 2, 1, 1, 0],
    [2, 2, 1, 0, 0],
    [2, 1, 1, 0, 0],
    [2, 1, 0, 0, 0],
    [1, 1, 0, 0, 0],
    [1, 0, 0, 0, 0],
    [1, 0, 0, 0, 0],
    [0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0],
    [0, 0, 0, 0, -1],
    [0, 0, -1, -1, -1],
    [0, -1, -1, -1, -2],
    [-1, -1, -1, -1, -2],
    [-1, -1, -2, -2, -2],
];
const PROMISE_OLD: [i8; 5] = [-2, -2, -2, -2, -2];
/// Fees and salaries count in thousands where they are kept, a release
/// fee in ten thousands.
const THOUSAND: i32 = 1000;
const FEE_UNIT: i64 = 10_000;

fn today(records: &Records) -> Option<NaiveDate> {
    let [day, month, year] = records.globals.date();
    NaiveDate::from_ymd_opt(i32::from(year), u32::from(month), u32::from(day))
}

fn age(records: &Records, p: &Player) -> i32 {
    today(records).map_or(0, |d| crate::statistics::age_on(p, d))
}

/// The years his contract has left in the closing season, as his object
/// holds them: none once run out.
fn years_left(records: &Records, p: &Player) -> i32 {
    (i32::from(p.contract() & 0xfff) - i32::from(records.globals.start_year())).max(0)
}

/// Whether his contract holds him on at a club playing a league of
/// `class` next season (`FUN_004271fb`): two years left at least, and its
/// clause (the terms' low two bits) no bar.
pub(crate) fn holds(records: &Records, p: &Player, class: i16) -> bool {
    let clause = (p.terms() & 3) as i16;
    years_left(records, p) >= 2 && (clause == 0 || class < clause)
}

/// Whether the free agents' list still takes one (`FUN_0040e8cc`'s room).
pub(crate) fn takes(agents: &[FreeAgent]) -> bool {
    agents.len() < LIST_MOST
}

/// The season's end lets the players go whose contracts do not hold them
/// on (`FUN_006b4459` per club of the countries played, `FUN_006b3f92`),
/// and returns the free agents in the order they were listed.
pub fn release(records: &mut Records, random: &mut Random) -> Vec<FreeAgent> {
    let mut agents = Vec::new();
    for country in crate::opponent::played_nations(records) {
        for number in 0..records.countries.team_count(usize::from(country)).max(0) as u16 {
            let class = crate::deals::next_class(records, country as u8, number as i16);
            let squad = records.team(country, number).map_or_else(Vec::new, Team::players);
            let (mut free, mut gone) = (Vec::new(), Vec::new());
            for id in squad {
                let Some(p) = records.player(id) else { continue };
                if p.next_club()[0] != 0 || holds(records, p, class) {
                    continue;
                }
                if p.retiring() & RETIRES == 0 && agents.len() + free.len() < LIST_MOST {
                    free.push(id);
                } else {
                    gone.push(id);
                }
            }
            for id in free {
                crate::deals::let_go(records, (country as u8, number as i16), id, 0, None);
                if let Some(p) = records.player_mut(id) {
                    p.set_retiring(p.retiring() & !LISTED);
                }
                crate::rollover::delist_transfer(records, id);
                agents.push(FreeAgent { id, country: country as u8, team: number as i16 });
            }
            for id in gone {
                crate::rollover::retire(records, country, number, id, random);
            }
            // The youths too old for the youth go too (`FUN_006b4230`):
            // their station closed, onto the list, off the youth list and
            // out of the club's youth.
            for id in aged_out(records, country as u8, number) {
                if let Some(p) = records.player_mut(id) {
                    p.set_retiring(p.retiring() & YOUTH_BITS_KEPT);
                }
                crate::deals::close_station(records, id);
                if agents.len() < LIST_MOST {
                    agents.push(FreeAgent { id, country: country as u8, team: number as i16 });
                }
                crate::youth::drop_prospect(records, id);
                crate::youth::leave_youth(records, (country as u8, number), id);
            }
        }
    }
    agents
}

/// A club's youths too old for its youth next season: born before the
/// first of July eighteen years before the next season's year
/// (`FUN_007068c9`, `FUN_004379d0`).
fn aged_out(records: &Records, country: u8, number: u16) -> Vec<u16> {
    let bar = (i32::from(records.globals.start_year()) + 1 - YOUTH_YEARS, 7u8, 1u8);
    crate::youth::youths(records, country, number)
        .0
        .into_iter()
        .filter(|&id| {
            records.player(id).is_some_and(|p| (i32::from(p.birth_year()), p.birth_month(), p.birth_day()) < bar)
        })
        .collect()
}

/// His promise (`FUN_006ff516`): his strength and what his age and talent
/// add, within 1 and what his talent reaches.
fn promise(p: &Player, age: i32) -> i16 {
    let talent = usize::try_from(p.talent()).unwrap_or(0).min(4);
    let add = match age {
        ..17 => PROMISE[0][talent],
        17..35 => PROMISE[(age - 17) as usize][talent],
        _ => PROMISE_OLD[talent],
    };
    let x = i16::from(p.strength()) + i16::from(add);
    let top = i16::from(crate::rollover::top_strength(p.talent()));
    if top < x { top } else { x.max(1) }
}

/// A league's computer clubs with their squads by position
/// (`FUN_0040ea45`): next season's clubs of the league.
#[derive(Clone, Default)]
struct Table {
    country: u8,
    clubs: Vec<(i16, i16, [i16; 11])>,
}

fn table(records: &Records, country: u16, level: u8) -> Table {
    let humans = crate::statistics::human_clubs(records);
    let mut t = Table { country: country as u8, clubs: Vec::new() };
    for team in crate::promotion::next_list(records, country, level) {
        if humans.contains(&(country as u8, team)) {
            continue;
        }
        let Some(club) = u16::try_from(team).ok().and_then(|n| records.team(country, n)) else { continue };
        let squad = club.players();
        let mut by = [0i16; 11];
        for &id in &squad {
            if let Some(p) = records.player(id) {
                by[usize::from(p.positions()[0]).min(10)] += 1;
            }
        }
        t.clubs.push((team, squad.len() as i16, by));
    }
    t
}

/// The years of his fresh terms (`FUN_0076d3ac`): by his age, the young
/// and able drawn up to five.
fn fresh_years(p: &Player, age: i32, random: &mut Random) -> i8 {
    match age {
        ..23 if p.strength() > 3 => match random.below(3) {
            0 => 5,
            1 => 4,
            _ => 3,
        },
        ..23 => 3,
        23..30 if p.strength() > 4 && random.below(2) != 0 => 4,
        23..30 => 3,
        30..33 => 3,
        _ => 2,
    }
}

/// A free agent's fresh terms at a club playing `level` (`FUN_00709fb6`):
/// his salary as he would ask it, in thousands; the bonus for playing a
/// share of it by the league (an eighth, a twelfth, a sixteenth), for a
/// goal the same again over 1.25 for a forward, over 2.5 for a striker
/// behind; the years by his age; a release fee in ten thousands, by his
/// market value, for one of strength 7 at least one time in five; the
/// player's option one time in two.
fn fresh_terms(
    records: &mut Records,
    id: u16,
    level: u8,
    tables: (&ContractTables, &ValueTables),
    random: &mut Random,
) {
    let year = i32::from(records.globals.start_year());
    let month = u32::from(records.globals.date()[1]);
    let Some(p) = records.player(id) else { return };
    let age = age(records, p);
    let form = crate::statistics::form_today(p);
    let position = p.positions()[0];
    let star = records.globals.stars().get(usize::from(position)) == Some(&id);
    let signee = Signee::of(p, age, form, tables.1, star, records.globals.start_year());
    let salary = (tables.0.demand(&signee) / THOUSAND) as u16;
    let share = match level {
        1 => 12.0,
        2.. => 16.0,
        _ => 8.0,
    };
    let appearance = (f64::from(salary) / share) as i32 as u16;
    let goal = match position {
        10 => (f64::from(salary) / share / 1.25) as i32 as u16,
        9 => (f64::from(salary) / share / 2.5) as i32 as u16,
        _ => 0,
    };
    let p = records.player(id).cloned();
    let Some(p) = p else { return };
    let years = fresh_years(&p, age, random);
    let fee = if p.strength() >= 7 && random.below(5) == 0 {
        let worth = Worth {
            strength: p.strength(),
            position,
            talent: p.talent(),
            age,
            good: p.good(),
            character: p.traits(),
            qualities: p.qualities(),
            star,
            kind_12_matches: p.kind_12_matches(),
        };
        (tables.1.market_value(&worth, i32::from(years), month) / FEE_UNIT) as u16
    } else {
        0
    };
    let option = if random.below(2) != 0 { 0x1000 } else { 0 };
    if let Some(p) = records.player_mut(id) {
        p.set_salary(salary);
        p.set_appearance_bonus(appearance);
        p.set_goal_bonus(goal);
        p.set_fixed_fee(fee);
        p.put::<u32>(0x158, 0);
        p.set_terms(option);
        p.put::<u8>(0x161, 0);
        p.put::<u8>(0x160, 0);
        // A year more than the terms, counted from the closing season.
        p.set_contract((i32::from(years) + 1 + year) as u16);
        p.put::<u8>(0x14c, 0);
        p.put::<u16>(0x14e, 0);
    }
}

/// The free agents find clubs (`FUN_0040ec50`).
pub fn place(
    records: &mut Records,
    agents: &[FreeAgent],
    tables: (&ContractTables, &ValueTables),
    random: &mut Random,
) {
    let main = u16::try_from(records.globals.main_country()).unwrap_or(0);
    let leagues = records.countries.leagues(usize::from(main)).max(0) as usize;
    let more = usize::try_from(records.globals.more_count()).unwrap_or(0).min(records.globals.more_countries().len());
    let more_countries: Vec<i16> = records.globals.more_countries()[..more].to_vec();
    let mut tables_by: Vec<Table> = vec![Table::default(); MORE_TABLES + more];
    for level in 0..leagues.min(MORE_TABLES) {
        tables_by[level] = table(records, main, level as u8);
    }
    for (k, &c) in more_countries.iter().enumerate() {
        tables_by[MORE_TABLES + k] = table(records, c as u16, 0);
    }
    let humans = crate::statistics::human_clubs(records);
    for agent in agents {
        let Some(p) = records.player(agent.id).cloned() else { continue };
        let old = (agent.country, agent.team);
        let age = age(records, &p);
        if !records.globals.played(i16::from(agent.country)) || age > TOO_OLD {
            crate::rollover::end_career(records, agent.id, random);
            continue;
        }
        let mut stays: Option<i16> = None;
        let mut reserve: Option<i16> = None;
        let at = if u16::from(agent.country) == main {
            let old_team = u16::try_from(agent.team).ok().and_then(|n| records.team(main, n));
            reserve = old_team.and_then(|t| (t.linked()[1] >= 0).then_some(i16::from(t.linked()[1])));
            let mut level = i32::from(old_team.map_or(10, Team::level));
            let promise = i32::from(promise(&p, age));
            let talent = p.talent();
            if level < 2 && age < 20 && promise > 3 && !humans.contains(&old) {
                stays = Some(agent.team);
            } else if talent < 3 {
                level = if age < 27 {
                    let bar = (age - 18) / 2;
                    if promise >= bar + 5 {
                        0
                    } else if promise >= bar + 4 {
                        1
                    } else if promise < 3 || talent > 1 {
                        2
                    } else {
                        1
                    }
                } else if promise >= 8 {
                    0
                } else if promise >= 5 {
                    1
                } else {
                    2
                };
            } else {
                level = 1;
                if random.below(4) != 0 {
                    level = random.below(2) as i32 + 2;
                }
            }
            let mut level = level.min(leagues as i32 - 1).max(0);
            if main == 0xe && level > 3 {
                level = 3;
            }
            level as usize
        } else {
            match more_countries.iter().position(|&c| c == i16::from(agent.country)) {
                Some(k) => MORE_TABLES + k,
                None => continue,
            }
        };
        let Some(t) = tables_by.get(at).cloned() else { continue };
        let position = usize::from(p.positions()[0]).min(10);
        let mut best = 0;
        for k in 1..t.clubs.len() {
            if t.clubs[k].2[position] < t.clubs[best].2[position] {
                best = k;
            }
        }
        let club = match (stays, reserve) {
            (Some(team), _) => Some(team),
            (None, Some(team)) if age < 21 && p.salary() > 0 => Some(team),
            _ => t.clubs.get(best).map(|c| c.0),
        };
        let Some(club) = club.filter(|&c| c >= 0) else {
            crate::rollover::end_career(records, agent.id, random);
            continue;
        };
        let new = (t.country, club);
        let size = records.team(u16::from(new.0), club as u16).map_or(FULL, |c| c.players().len());
        if size >= FULL {
            crate::rollover::end_career(records, agent.id, random);
            continue;
        }
        if club != agent.team {
            crate::rollover::note_move(records, new, agent.id, 1, old);
            crate::rollover::note_move(records, old, agent.id, 2, new);
        }
        let level = records.team(u16::from(new.0), club as u16).map_or(10, Team::level);
        crate::deals::bring_in(records, new, agent.id, 0);
        fresh_terms(records, agent.id, level, tables, random);
        if let Some(p) = records.player_mut(agent.id) {
            crate::deals::new_ties(p, false);
        }
        if let Some(c) = tables_by.get_mut(at).and_then(|t| t.clubs.get_mut(best)) {
            c.1 += 1;
            c.2[position] += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promise_adds_by_age_and_talent_within_bounds() {
        let mut p = Player::new(&vec![0; Player::SIZE]).unwrap();
        p.set_strength(5);
        p.set_talent(0);
        assert_eq!(promise(&p, 16), 8);
        assert_eq!(promise(&p, 19), 7);
        assert_eq!(promise(&p, 40), 3);
        p.set_strength(11);
        p.set_talent(4);
        assert_eq!(promise(&p, 17), 8, "never above what his talent reaches");
        p.set_strength(1);
        assert_eq!(promise(&p, 34), 1, "never below 1");
    }

    /// Over the original's pair (SeasonEnd364 → NewSeason000), the season
    /// end's steps before it run by the replica: the players it lets go
    /// are those the original let go — gone from the club, on fresh terms
    /// or their careers ended —, every one whose contract ran out among
    /// them, and the youths it sends out of the youth onto the list are the
    /// original's; the free agents placed stand in a squad of at most 30 on
    /// terms of two to five years, and where his strength did not move
    /// between the two saves (the original develops the played countries'
    /// players before, [`crate::development`], left out here) a placed
    /// man's salary is
    /// the original's, his bonuses too at the same club, and so is the
    /// league of a weak talent of the main country.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_season_end_lets_go_and_places_the_free_agents_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs, before) =
            (load("SeasonEnd364.an3"), load("NewSeason000.an3"), load("SeasonEnd364.an3"));
        let data = crate::data::Data::load(&dir).unwrap();
        let mut random = Random::new(3);
        let _ = crate::promotion::draw(&mut ours, &mut random);
        crate::promotion::count(&mut ours);
        crate::rollover::retire_announced(&mut ours, &mut random);
        crate::statistics::archive_leagues(&mut ours);
        crate::statistics::fold_eternal(&mut ours);
        crate::promotion::flag(&mut ours, &data.rules.forecast.tickets);
        crate::rollover::count_first_seasons(&mut ours);
        let agents = release(&mut ours, &mut random);
        let released: std::collections::HashSet<u16> = agents.iter().map(|a| a.id).collect();
        let year = i32::from(before.globals.start_year());
        let mut let_go = 0;
        for country in crate::opponent::played_nations(&before) {
            for number in 0..before.countries.team_count(usize::from(country)).max(0) as u16 {
                for id in before.team(country, number).unwrap().players() {
                    let (x, y) = (before.player(id).unwrap(), theirs.player(id).unwrap());
                    if x.next_club()[0] != 0 || x.retiring() & RETIRES != 0 {
                        continue;
                    }
                    let ended = x.first_name() != y.first_name();
                    let moved = x.club() != y.club() || x.contract() != y.contract() || x.salary() != y.salary();
                    let theirs_let_go = ended || moved;
                    let ran_out = i32::from(x.contract() & 0xfff) - year < 2;
                    if released.contains(&id) {
                        let_go += 1;
                        assert!(theirs_let_go, "player {id} kept on by the original");
                    } else {
                        assert!(!(theirs_let_go && ran_out), "player {id} let go by the original");
                    }
                }
            }
        }
        assert!(let_go > 200, "{let_go} let go as the original");
        // The youths leaving the youth are the original's, now free agents.
        let mut left = 0;
        for country in crate::opponent::played_nations(&before) {
            for number in 0..before.countries.team_count(usize::from(country)).max(0) as u16 {
                let (old, _) = crate::youth::youths(&before, country as u8, number);
                let (theirs_now, _) = crate::youth::youths(&theirs, country as u8, number);
                let (ours_now, _) = crate::youth::youths(&ours, country as u8, number);
                for id in old {
                    let gone = !theirs_now.contains(&id);
                    assert_eq!(!ours_now.contains(&id), gone, "youth {id}");
                    assert_eq!(released.contains(&id), gone, "youth {id} a free agent");
                    left += i32::from(gone);
                }
            }
        }
        assert!(left > 50, "{left} youths left");
        crate::deals::execute(&mut ours, &mut random);
        crate::rollover::close_played(&mut ours, &mut random);
        crate::rollover::close_kept(&mut ours, &mut random);
        place(&mut ours, &agents, (&data.rules.contract, &data.rules.value), &mut random);
        let (mut paid, mut leagues) = (0, 0);
        let main = u16::try_from(before.globals.main_country()).unwrap();
        for a in &agents {
            let (o, x, y) = (before.player(a.id).unwrap(), ours.player(a.id).unwrap(), theirs.player(a.id).unwrap());
            let club = x.club();
            let Some(team) = ours.team(u16::from(club[0]), u16::from(club[1])) else { continue };
            if !team.players().contains(&a.id) {
                continue;
            }
            assert!(team.players().len() <= FULL, "club {club:?} over full");
            let left = i32::from(x.contract() & 0xfff) - year;
            assert!((3..=6).contains(&left), "player {}: {left} years", a.id);
            let gone = y.first_name() != o.first_name();
            if gone || y.strength() != o.strength() {
                continue;
            }
            if y.club() != o.club() || y.contract() != o.contract() {
                assert_eq!(x.salary(), y.salary(), "player {}'s salary", a.id);
                if club == y.club() {
                    assert_eq!(
                        (x.appearance_bonus(), x.goal_bonus()),
                        (y.appearance_bonus(), y.goal_bonus()),
                        "player {}'s bonuses",
                        a.id
                    );
                }
                paid += 1;
            }
            if u16::from(club[0]) == main && o.talent() < 3 {
                let level = |r: &Records, c: [u8; 2]| r.team(u16::from(c[0]), u16::from(c[1])).map(Team::level);
                assert_eq!(level(&theirs, club), level(&theirs, y.club()), "player {}'s league", a.id);
                leagues += 1;
            }
        }
        assert!(paid > 100 && leagues > 0, "{paid} terms, {leagues} leagues as the original's");
    }
}
