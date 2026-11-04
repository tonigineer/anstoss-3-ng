//! The national cups' first rounds, drawn at the new season's start
//! (`FUN_006cc3ff` from the new-season block of `FUN_006da3a9`, per
//! played country `FUN_006cf189`).
//!
//! Each cup first takes its entrants into its table (the cup object's
//! +0x526, four bytes each: country, class, club; at most as many as the
//! cup holds, a club once — `FUN_006d0ad4`). Germany as the main country
//! (`FUN_006cf5be`, the rules from 2000 on) enters the Bundesliga and the
//! 2. Bundesliga as they now stand (classes 0 and 1), the clubs of last
//! season's first two leagues not among them (class 2, its final places
//! in the country block, `FUN_006810c8`), each Regionalliga's first six in
//! its list (class 2), and fills the 64 with amateur clubs, a random
//! thirty of the forty after its teams (class 10, `FUN_00675698`). Every
//! other country (`FUN_006d08e8`) enters all its clubs and fills with the
//! forty amateurs in a random order; it plays for 64 with more than 19
//! clubs, else for 32.
//!
//! The round's ties then pair the entrants in their order (`FUN_006cf2c9`),
//! every side is shuffled to a random place (`FUN_00411935`,
//! `FUN_00411fc0`) — in the German cup a club of the top two leagues
//! drawn at home against one below them changes sides —, and the ties are
//! ordered by the two clubs' classes (`FUN_00411272`, the table at
//! 0x876ed0: the top clubs' ties first). The entrants' season flags gain
//! 0x8800 (`FUN_006d0b8a`).
//!
//! Left out: the cups' first season (`FUN_006cf031`), Germany's rules
//! before 2000, the other main countries' own entrants
//! (`FUN_006cf964`, `FUN_006cfc09`, `FUN_006d057c`, …, which fall back to
//! the plain rule here). England's league cup is drawn the same way
//! beside its cup (`FUN_006cfaab`): the clubs of its four leagues by their
//! level, its amateurs while there is room. The original draws with its
//! own generator.

use crate::opponent::{CUPS_AT, League, PAIRING, PAIRINGS};
use crate::random::Random;
use crate::records::Records;

const GERMANY: u16 = 0xe;
/// The cup block's fields (the object from its +0x20): its country, the
/// home rule, its type (the row of the round table), the round drawn,
/// the last round drawn, how many it takes.
const COUNTRY: usize = 0;
const HOME_RULE: usize = 2;
const KIND: usize = 4;
const ROUND: usize = 6;
const DRAWN: usize = 8;
const CAPACITY: usize = 0xa;
/// The entrants' table.
const ENTRANTS: usize = 0x526 - 0x20;
const ENTRANT: usize = 4;
/// The amateur clubs after a country's teams, and how many a cup draws
/// from.
const AMATEURS: usize = 40;
const GERMAN_AMATEURS: usize = 30;
/// The entrants' classes.
const AMATEUR: u8 = 10;
const BELOW: u8 = 2;
/// The flags a cup's entrant takes (team +0x110).
const ENTERED: u16 = 0x8800;
/// The order of a tie by its two clubs' classes, capped at 4 (0x876ed0).
const ORDER: [[i32; 5]; 5] =
    [[0, 1, 2, 3, 4], [1, 5, 6, 7, 8], [2, 6, 9, 10, 11], [3, 7, 10, 12, 13], [4, 8, 11, 13, 14]];
/// The round table (0x8a5e10): by the cup's type and its round, the first
/// tie, how many ties, the first entrant and how many entrants.
const ROUNDS: [[(usize, usize, usize, usize); 3]; 6] = [
    [(0, 0, 0, 0), (0, 32, 0, 64), (32, 16, 0, 0)],
    [(0, 0, 0, 0), (0, 0, 0, 0), (32, 16, 0, 32)],
    [(0, 20, 44, 40), (20, 32, 0, 44), (52, 16, 0, 0)],
    [(0, 0, 0, 0), (20, 32, 0, 64), (52, 16, 0, 0)],
    [(0, 35, 22, 70), (35, 25, 7, 15), (60, 16, 0, 7)],
    [(0, 0, 0, 0), (35, 25, 7, 50), (60, 16, 0, 7)],
];
/// The entrants a round of a cup's type takes in beside the winners
/// before it: the first and how many (the round table's third and fourth
/// columns).
pub(crate) fn joining(kind: usize, round: usize) -> (usize, usize) {
    ROUNDS.get(kind).and_then(|r| r.get(round)).map_or((0, 0), |&(_, _, from, count)| (from, count))
}

/// The entrants' table of a cup object, four bytes each.
pub(crate) const ENTRANTS_AT: usize = ENTRANTS;

/// England's league cup (the object `0x1ba7488`, a cup's layout).
const ENGLAND: u16 = 0x10;
pub(crate) const LEAGUE_CUP: &str = "0x1ba7488";
const LEAGUE_CUP_LEVELS: u8 = 4;

/// A cup's setup by `FUN_006cef0b`'s case: its type, the round it draws
/// and how many it takes.
fn setup(case: u8, main: bool) -> (u16, u16, u16) {
    match case {
        0 if main => (2, 0, 0x54),
        0 => (3, 1, 0x40),
        1 if main => (4, 0, 0x5c),
        1 => (5, 1, 0x39),
        2 => (1, 2, 0x20),
        _ => (0, 1, 0x40),
    }
}

/// Picks `k` distinct numbers below `n` in the order drawn
/// (`FUN_00675698`).
pub(crate) fn choose(k: usize, n: usize, random: &mut Random) -> Vec<usize> {
    let n = if n < k || k > 200 { k.min(200) } else { n };
    let mut taken = vec![false; n];
    (0..k.min(n))
        .map(|_| loop {
            let at = random.below(n as u32) as usize;
            if !taken[at] {
                taken[at] = true;
                break at;
            }
        })
        .collect()
}

/// A random order of `n` (`FUN_00675630`).
pub(crate) fn order(n: usize, random: &mut Random) -> Vec<usize> {
    let mut slots: Vec<Option<usize>> = vec![None; n];
    for k in 0..n {
        loop {
            let at = random.below(n as u32) as usize;
            if slots[at].is_none() {
                slots[at] = Some(k);
                break;
            }
        }
    }
    slots.into_iter().map(|s| s.unwrap_or(0)).collect()
}

/// A cup's entrants as its table takes them.
struct Entrants {
    country: u8,
    capacity: usize,
    list: Vec<[u8; ENTRANT]>,
}

impl Entrants {
    /// Takes a club unless full or already in (`FUN_006d0ad4`).
    fn enter(&mut self, team: i16, class: u8) {
        if self.list.len() >= self.capacity || self.list.iter().any(|e| i16::from_le_bytes([e[2], e[3]]) == team) {
            return;
        }
        let t = team.to_le_bytes();
        self.list.push([self.country, class, t[0], t[1]]);
    }

    fn full(&self) -> bool {
        self.list.len() >= self.capacity
    }
}

fn list(records: &Records, country: u16, level: u8) -> Vec<i16> {
    League::of(records, country, level).map_or_else(Vec::new, |l| (0..l.teams()).map(|i| l.team(i)).collect())
}

/// A club's class for the ties' order (`FUN_0063e388`, capped at 4):
/// by its level through `FUN_006755b8` — the main country's up to 2, a
/// club outside the leagues one more —, an amateur club 10.
fn class(records: &Records, country: u16, team: i16, main: u16) -> usize {
    let clubs = records.countries.team_count(usize::from(country));
    if team < 0 || team >= clubs {
        return 4;
    }
    let by_level = |level: u8| -> i32 {
        if country == main {
            if country == 0x10 { i32::from(level.min(3)) } else { i32::from(level.min(2)) }
        } else if level > 0 {
            0
        } else {
            i32::from(level)
        }
    };
    let level = records.team(country, team as u16).map_or(10, |t| t.level());
    let c = if level == 10 { by_level(10) + 1 } else { by_level(level) };
    usize::try_from(c).unwrap_or(0).min(4)
}

/// Draws the national cups' first rounds of every played country.
pub fn draw(records: &mut Records, random: &mut Random) {
    let main = u16::try_from(records.globals.main_country()).unwrap_or(0);
    for country in crate::opponent::played_nations(records) {
        draw_cup(records, country, main, random);
        // Its supercup beside (`FUN_006ccc80`).
        crate::supercup::set_up(records, country, random);
        // England's league cup last (`FUN_006cfaab`).
        if country == ENGLAND {
            draw_league_cup(records, main, random);
        }
    }
}

fn draw_cup(records: &mut Records, country: u16, main: u16, random: &mut Random) {
    let clubs = usize::try_from(records.countries.team_count(usize::from(country))).unwrap_or(0);
    let (case, rule) = if country == GERMANY && country == main { (3, 1) } else { (u8::from(clubs > 19) + 2, u16::from(country == GERMANY)) };
    let (kind, round, capacity) = setup(case, country == main);
    let mut entrants = Entrants { country: country as u8, capacity: usize::from(capacity), list: Vec::new() };
    if country == GERMANY && country == main {
        for level in 0..2u8 {
            for team in list(records, country, level) {
                entrants.enter(team, level);
            }
        }
        let slot = records.countries.slot(usize::from(country));
        let places: Vec<[u8; 18]> = records.block(&format!("0x19f7fd0[{slot}]")).map_or_else(Vec::new, |b| {
            (0..2).map(|l| std::array::from_fn(|k| b.get(0x26 + l * 0x18 + k).copied().unwrap_or(0))).collect()
        });
        if let [first, second] = places.as_slice() {
            for k in 0..18 {
                entrants.enter(i16::from(first[k]), BELOW);
                entrants.enter(i16::from(second[k]), BELOW);
            }
        }
        for level in 2..4u8 {
            for &team in list(records, country, level).iter().take(6) {
                entrants.enter(team, BELOW);
            }
        }
        let picks = choose(GERMAN_AMATEURS, AMATEURS, random);
        for &k in &picks {
            if entrants.full() {
                break;
            }
            entrants.enter((clubs + k) as i16, AMATEUR);
        }
    } else {
        for team in 0..clubs as i16 {
            entrants.enter(team, 0);
        }
        let picks = order(AMATEURS, random);
        for &k in &picks {
            if entrants.full() {
                break;
            }
            entrants.enter((clubs + k) as i16, AMATEUR);
        }
    }
    let slot = records.countries.slot(usize::from(country));
    let name = format!("{CUPS_AT:#x}[{slot}]");
    write_draw(records, &name, (country, main), (rule, kind, round, capacity), &entrants, random);
}

/// England's league cup, drawn as its national cup is (`FUN_006cfaab`):
/// the object cleared, set up as `FUN_006cef0b`'s second case, the clubs
/// of its four leagues as they now stand by their level, then its amateurs
/// in a random order while there is room, its first round drawn and its
/// entrants marked.
fn draw_league_cup(records: &mut Records, main: u16, random: &mut Random) {
    let clubs = usize::try_from(records.countries.team_count(usize::from(ENGLAND))).unwrap_or(0);
    let (kind, round, capacity) = setup(1, ENGLAND == main);
    let mut entrants = Entrants { country: ENGLAND as u8, capacity: usize::from(capacity), list: Vec::new() };
    for level in 0..LEAGUE_CUP_LEVELS {
        for team in list(records, ENGLAND, level) {
            entrants.enter(team, level);
        }
    }
    let picks = order(AMATEURS, random);
    for &k in &picks {
        if entrants.full() {
            break;
        }
        entrants.enter((clubs + k) as i16, AMATEUR);
    }
    let Some(cup) = records.block_mut(LEAGUE_CUP) else { return };
    cup.fill(0);
    write_draw(records, LEAGUE_CUP, (ENGLAND, main), (0, kind, round, capacity), &entrants, random);
}

/// A cup's first drawn round into its object: the round's ties in the
/// entrants' order, every side shuffled, the home rule, the ties by their
/// clubs' classes, the setup and the entrants written and the entrants'
/// season flags (`FUN_006cf2c9`, `FUN_00411935`, `FUN_006d0b8a`).
fn write_draw(
    records: &mut Records,
    name: &str,
    (country, main): (u16, u16),
    (rule, kind, round, capacity): (u16, u16, u16, u16),
    entrants: &Entrants,
    random: &mut Random,
) {
    let clubs = usize::try_from(records.countries.team_count(usize::from(country))).unwrap_or(0);
    // The round's ties, in the entrants' order.
    let Some(&(first, ties, from, count)) = ROUNDS.get(usize::from(kind)).and_then(|r| r.get(usize::from(round)))
    else {
        return;
    };
    let mut sides: Vec<[u8; ENTRANT]> = vec![[0; ENTRANT]; 2 * ties];
    let start = ties.saturating_sub(count / 2);
    let odd = count % 2 == 1;
    let entrant = |k: usize| entrants.list.get(from + k).copied().unwrap_or([0; ENTRANT]);
    if odd && start > 0 {
        sides[2 * (start - 1) + 1] = entrant(0);
    }
    let mut tie = start;
    let mut k = usize::from(odd);
    while k < count && tie < ties {
        sides[2 * tie] = entrant(k);
        sides[2 * tie + 1] = entrant(k + 1);
        tie += 1;
        k += 2;
    }
    // Every side to a random place, and the home rule (`FUN_00411fc0`).
    let n = 2 * ties;
    let mut perm = choose(n, n, random);
    for e in 0..n.saturating_sub(1) {
        let p = perm[e];
        sides.swap(e, p);
        if let Some(j) = (e..n).find(|&j| perm[j] == e) {
            perm[j] = p;
        }
    }
    let mut pairs: Vec<[[u8; ENTRANT]; 2]> = sides.chunks(2).map(|c| [c[0], c[1]]).collect();
    if rule != 0 {
        for pair in &mut pairs {
            if pair[0][1] < BELOW && pair[1][1] > 1 {
                pair.swap(0, 1);
            }
        }
    }
    // The ties by their clubs' classes (`FUN_00411272`), a selection sort.
    let team = |side: &[u8; ENTRANT]| i16::from_le_bytes([side[2], side[3]]);
    let mut keys: Vec<i32> = pairs
        .iter()
        .map(|p| ORDER[class(records, u16::from(p[0][0]), team(&p[0]), main)][class(records, u16::from(p[1][0]), team(&p[1]), main)])
        .collect();
    for i in 0..pairs.len() {
        let mut best = i;
        for j in i + 1..pairs.len() {
            if keys[j] < keys[best] {
                best = j;
            }
        }
        if best != i {
            pairs.swap(i, best);
            keys.swap(i, best);
        }
    }
    let Some(cup) = records.block_mut(name) else { return };
    let mut put = |at: usize, v: u16| {
        if let Some(w) = cup.get_mut(at..at + 2) {
            w.copy_from_slice(&v.to_le_bytes());
        }
    };
    put(COUNTRY, country);
    put(HOME_RULE, rule);
    put(KIND, kind);
    put(ROUND, round);
    put(DRAWN, round);
    put(CAPACITY, capacity);
    for (k, e) in entrants.list.iter().enumerate() {
        if let Some(s) = cup.get_mut(ENTRANTS + k * ENTRANT..ENTRANTS + (k + 1) * ENTRANT) {
            s.copy_from_slice(e);
        }
    }
    for (k, pair) in pairs.iter().enumerate() {
        let at = PAIRINGS + (first + k) * PAIRING;
        if let Some(p) = cup.get_mut(at..at + 2 * ENTRANT) {
            p[..ENTRANT].copy_from_slice(&pair[0]);
            p[ENTRANT..].copy_from_slice(&pair[1]);
        }
    }
    // The entrants' flags (`FUN_006d0b8a`, through `FUN_0063e0d1`).
    if records.countries.kind(usize::from(country)) == 1 {
        for e in &entrants.list {
            let number = u16::from_le_bytes([e[2], e[3]]);
            if usize::from(number) < clubs && number != 200
                && let Some(t) = records.team_mut(country, number)
            {
                t.put::<u16>(0x110, t.get::<u16>(0x110) | ENTERED);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_choice_takes_distinct_numbers() {
        let mut random = Random::new(2);
        let picks = choose(30, 40, &mut random);
        assert_eq!(picks.len(), 30);
        let mut sorted = picks.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 30);
        assert!(picks.iter().all(|&p| p < 40));
    }

    /// The new season's first cup rounds over the original's pair
    /// (SeasonEnd364 rolled over by the replica against NewSeason000): each
    /// played country's cup stands set up as the original's, its entrants
    /// in the original's order but for the amateur clubs drawn — ten, from
    /// the forty after the country's teams, once each —, every entrant in
    /// one tie, the ties ordered by their clubs' classes, the German cup's
    /// clubs of the top two leagues away to those below, and every
    /// entrant's flags marking him.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_new_season_draws_its_cups_as_the_original() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let read = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs) = (read("SeasonEnd364.an3"), read("NewSeason000.an3"));
        let mut random = Random::new(5);
        let drawn = crate::promotion::draw(&mut ours, &mut random);
        crate::statistics::archive_leagues(&mut ours);
        crate::promotion::take(&mut ours, &drawn);
        crate::settlement::open_season(&mut ours, &mut random);
        let main = u16::try_from(ours.globals.main_country()).unwrap();
        for country in crate::opponent::played_nations(&ours) {
            let slot = ours.countries.slot(usize::from(country));
            let name = format!("{CUPS_AT:#x}[{slot}]");
            let (x, y) = (ours.block(&name).unwrap(), theirs.block(&name).unwrap());
            assert_eq!(x[..0xc], y[..0xc], "{country}'s cup setup");
            let capacity = usize::from(u16::from_le_bytes([x[CAPACITY], x[CAPACITY + 1]]));
            let entrants = |z: &[u8]| -> Vec<[u8; ENTRANT]> {
                (0..capacity).map(|k| z[ENTRANTS + k * ENTRANT..ENTRANTS + (k + 1) * ENTRANT].try_into().unwrap()).collect()
            };
            let (ex, ey) = (entrants(x), entrants(y));
            let clubs = ours.countries.team_count(usize::from(country)) as u16;
            let drawn_at = ey.iter().position(|e| e[1] == AMATEUR).unwrap();
            assert_eq!(ex[..drawn_at], ey[..drawn_at], "{country}'s entrants");
            let mut amateurs: Vec<u16> = ex[drawn_at..].iter().map(|e| u16::from_le_bytes([e[2], e[3]])).collect();
            assert!(ex[drawn_at..].iter().all(|e| e[1] == AMATEUR), "{country}'s amateurs");
            assert!(amateurs.iter().all(|&n| (clubs..clubs + AMATEURS as u16).contains(&n)));
            amateurs.sort_unstable();
            amateurs.dedup();
            assert_eq!(amateurs.len(), capacity - drawn_at, "{country}'s amateurs once each");
            let ties = capacity / 2;
            let mut sides: Vec<[u8; ENTRANT]> = Vec::new();
            let mut last = -1;
            for q in 0..ties {
                let p = &x[PAIRINGS + q * PAIRING..PAIRINGS + (q + 1) * PAIRING];
                let (home, away): ([u8; ENTRANT], [u8; ENTRANT]) = (p[..4].try_into().unwrap(), p[4..8].try_into().unwrap());
                let team = |s: &[u8; ENTRANT]| i16::from_le_bytes([s[2], s[3]]);
                let key = ORDER[class(&ours, country, team(&home), main)][class(&ours, country, team(&away), main)];
                assert!(key >= last, "{country}'s tie {q} in order");
                last = key;
                if country == GERMANY {
                    assert!(!(home[1] < BELOW && away[1] > 1), "{country}'s tie {q}: the lower club at home");
                }
                sides.extend([home, away]);
            }
            let mut want = ex.clone();
            want.sort_unstable();
            sides.sort_unstable();
            assert_eq!(sides, want, "{country}: every entrant in one tie");
            for e in &ex[..drawn_at] {
                let t = ours.team(country, u16::from_le_bytes([e[2], e[3]])).unwrap();
                assert_eq!(t.get::<u16>(0x110) & ENTERED, ENTERED, "{country}/{} entered", u16::from_le_bytes([e[2], e[3]]));
            }
        }
    }

    /// England's league cup over the same turn: set up as the original's,
    /// its 57 entrants England's 26 clubs by their leagues — the
    /// original's — and 31 of its amateurs drawn, the 50 behind the first
    /// seven paired in its first round's 25 ties as the original pairs
    /// its own, the ties by their clubs' classes, every club marked.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_new_season_draws_the_league_cup_as_the_original() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let read = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs) = (read("SeasonEnd364.an3"), read("NewSeason000.an3"));
        let mut random = Random::new(5);
        let drawn = crate::promotion::draw(&mut ours, &mut random);
        crate::statistics::archive_leagues(&mut ours);
        crate::promotion::take(&mut ours, &drawn);
        crate::settlement::open_season(&mut ours, &mut random);
        let main = u16::try_from(ours.globals.main_country()).unwrap();
        let (x, y) = (ours.block(LEAGUE_CUP).unwrap(), theirs.block(LEAGUE_CUP).unwrap());
        assert_eq!(x[..0xc], y[..0xc], "the league cup's setup");
        let capacity = usize::from(u16::from_le_bytes([x[CAPACITY], x[CAPACITY + 1]]));
        assert_eq!(capacity, 57);
        let entrants = |z: &[u8]| -> Vec<[u8; ENTRANT]> {
            (0..capacity).map(|k| z[ENTRANTS + k * ENTRANT..ENTRANTS + (k + 1) * ENTRANT].try_into().unwrap()).collect()
        };
        let (ex, ey) = (entrants(x), entrants(y));
        // England's 26 clubs first, by their leagues — the same clubs as
        // the original's, the order apart only where the amateurs'
        // promotions went another way — then its amateurs drawn, once
        // each.
        let clubs = ours.countries.team_count(usize::from(ENGLAND)) as u16;
        let numbers = |es: &[[u8; ENTRANT]]| -> Vec<u16> {
            let mut v: Vec<u16> = es.iter().filter(|e| e[1] != AMATEUR).map(|e| u16::from_le_bytes([e[2], e[3]])).collect();
            v.sort_unstable();
            v
        };
        assert_eq!(numbers(&ex), numbers(&ey), "the league cup's clubs");
        assert_eq!(numbers(&ex).len(), usize::from(clubs));
        assert_eq!(ex[..17], ey[..17], "the Premier League's clubs in order");
        let mut amateurs: Vec<u16> = ex[usize::from(clubs)..].iter().map(|e| u16::from_le_bytes([e[2], e[3]])).collect();
        assert!(ex[usize::from(clubs)..].iter().all(|e| e[1] == AMATEUR));
        assert!(amateurs.iter().all(|&n| (clubs..clubs + AMATEURS as u16).contains(&n)));
        amateurs.sort_unstable();
        amateurs.dedup();
        assert_eq!(amateurs.len(), capacity - usize::from(clubs));
        let (first, ties, from, count) = ROUNDS[5][1];
        let mut sides: Vec<[u8; ENTRANT]> = Vec::new();
        let mut last = -1;
        for q in first..first + ties {
            let p = &x[PAIRINGS + q * PAIRING..PAIRINGS + (q + 1) * PAIRING];
            let (home, away): ([u8; ENTRANT], [u8; ENTRANT]) = (p[..4].try_into().unwrap(), p[4..8].try_into().unwrap());
            let team = |s: &[u8; ENTRANT]| i16::from_le_bytes([s[2], s[3]]);
            let key = ORDER[class(&ours, ENGLAND, team(&home), main)][class(&ours, ENGLAND, team(&away), main)];
            assert!(key >= last, "tie {q} in order");
            last = key;
            sides.extend([home, away]);
        }
        let mut want = ex[from..from + count].to_vec();
        want.sort_unstable();
        sides.sort_unstable();
        assert_eq!(sides, want, "every entrant behind the first seven in one tie");
        // The original's ties hold the same entrants.
        let mut theirs_sides: Vec<[u8; ENTRANT]> = (first..first + ties)
            .flat_map(|q| {
                let p = &y[PAIRINGS + q * PAIRING..PAIRINGS + (q + 1) * PAIRING];
                [p[..4].try_into().unwrap(), p[4..8].try_into().unwrap()]
            })
            .collect();
        theirs_sides.sort_unstable();
        let mut theirs_want = ey[from..from + count].to_vec();
        theirs_want.sort_unstable();
        assert_eq!(theirs_sides, theirs_want, "the original's ties as well");
        for e in &ex {
            let Some(t) = ours.team(ENGLAND, u16::from_le_bytes([e[2], e[3]])) else { continue };
            if u16::from_le_bytes([e[2], e[3]]) < clubs {
                assert_eq!(t.get::<u16>(0x110) & ENTERED, ENTERED);
            }
        }
    }
}
