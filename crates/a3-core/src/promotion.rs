//! The season's promotions and relegations (`FUN_006b24f2`, per country
//! played `FUN_006af7a5`), the clubs' season flags they give
//! (`FUN_006b2543`) and the leagues the new season opens with
//! (`FUN_00774088`).
//!
//! At the season's end every played country draws next season's clubs of
//! its leagues from the closed tables and keeps them beside the current
//! ones (the league block's +0x52; `FUN_006926f7`); its country block
//! books who went down, who came up and who was sent down whatever his
//! place, at most four of each a level. A country with its first league
//! played alone (`FUN_006b2212`) sends the bottom of the table out of the
//! league and draws as many clubs from those outside it; Germany, played
//! as the main country (`FUN_006ae26f`), moves its four leagues — the
//! Bundesliga, the 2. Bundesliga and two Regionalligen — in one: the
//! Bundesliga's bottom three down, the 2. Bundesliga's top three up and
//! its bottom four into the Regionalligen, whose best two each go up; the
//! Regionalligen take, beside their own staying clubs and the clubs from
//! above, four clubs from outside the leagues each, drawn by region, and
//! are split again by region and by how far south a club lies, their
//! bottom four each falling out. A club's reserve team plays no league
//! with its first team's.
//!
//! The flags of the season follow for every club of the country (its
//! team +0x110, the letters of the tables): the champion, the runner-up,
//! the cup's winner and finalist, the promoted and the relegated — the
//! settlement pays the season by them — and the leagues take their new
//! clubs. At the new season's first init each club then stands at its
//! league's level and its place in the list; a club in no league at 10.
//!
//! The promoted and relegated clubs' squads count it, every player one
//! promotion or relegation more, the main country's players offered to
//! the record lists of the most of them (`FUN_007104dc`).
//!
//! Left out: the other countries' own rules (`FUN_006af8fd`,
//! `FUN_006b07f0`, `FUN_006b1c01`, `FUN_006b0d18`, `FUN_006b1920`,
//! `FUN_006b122e`), which the replica's own leagues fall back to the plain
//! rule for, and Germany's rules before 2000 (`FUN_006915f1`); what the
//! booked moves set off besides — the news (`FUN_006143d2`); the
//! European places' flags. Each club's record of moves (`FUN_00726382`,
//! `FUN_007264bf`, by slot `FUN_00634d9c`, `FUN_00634eff`, and its
//! history, `FUN_00640b2d`, `FUN_00640b9f`) counts every booked move. The clubs from outside
//! are drawn with the replica's own generator.

use crate::opponent::{LEAGUE_FIELDS, League};
use crate::random::Random;
use crate::records::Records;
use crate::statistics::{self, ALL, Row};
use crate::tickets::{self, TicketTables};

const GERMANY: u16 = 0xe;
/// The country whose single league skips reserve teams among its risers
/// and books its relegated a level down (`FUN_006b1f12`).
const FAR: u16 = 0x3f;

/// A league block's words: its clubs, how many go up and down, the list
/// of its clubs and next season's list.
const TEAMS: usize = 0;
const UP: usize = 0x1e;
const DOWN: usize = 0x20;
const CURRENT: usize = 0x22;
const NEXT: usize = 0x52;
const LIST: usize = 24;
/// From the bottom, so many places at most go down.
const BOTTOM: usize = 5;

/// The country block's books (the block holds the object from its +0x20):
/// per level how many went down and came up and per first two levels how
/// many were sent down, then the lists, four clubs a level.
const DOWN_COUNTS: usize = 0x1d2;
const UP_COUNTS: usize = 0x1da;
const FORCED_COUNTS: usize = 0x1e2;
const DOWN_LISTS: usize = 0x1e6;
const UP_LISTS: usize = 0x206;
const FORCED_LISTS: usize = 0x226;
const BOOKED: usize = 4;

/// A club outside the leagues stands at this level.
const OUTSIDE: u8 = 10;
/// Team +0x924's bit for the southern region.
const SOUTH: u8 = 2;
/// The Regionalligen's sort key for a club from outside.
const FROM_OUTSIDE: i16 = 0x18;
const DRAWN_PER_REGION: usize = 4;
const REGIONAL_CLUBS: usize = 36;
/// The clubs in the first league every season (`DAT_00b1710e`, four bytes
/// a club, 0x1b0 a league slot, in the lists block).
const TOP_SEASONS: usize = 0xb1710e - 0xb0b6b8;
const TOP_SEASONS_SLOT: usize = 0x1b0;
/// The season flags the books set, and those of team +0x110 they leave.
const CHAMPION: u16 = 0x1;
const RUNNER_UP: u16 = 0x2;
const PROMOTED: u16 = 0x4;
const RELEGATED: u16 = 0x8;
const CUP_WINNER: u16 = 0x1000;
const CUP_FINALIST: u16 = 0x2000;
const FLAGS_SET: u16 = 0xf87f;
const NO_CLUB: usize = 200;
/// The count of leagues the main country's flags walk (its block's +0x24,
/// the object's +0x44).
const MAIN_LEAGUES: usize = 0x24;

/// A country's books of the season's moves (the object's +0x110 …); two
/// books are the same when their lists are.
#[derive(Clone, Debug, Default)]
pub struct Moves {
    pub down: [Vec<i16>; 4],
    pub up: [Vec<i16>; 4],
    pub forced: [Vec<i16>; 2],
    /// Every move as it was booked, up or down, with its level — the
    /// lists above keep four a level only (the record of moves counts
    /// them all).
    log: Vec<(bool, usize, i16)>,
}

impl PartialEq for Moves {
    fn eq(&self, other: &Self) -> bool {
        (&self.down, &self.up, &self.forced) == (&other.down, &other.up, &other.forced)
    }
}

impl Eq for Moves {}

fn book(list: &mut Vec<i16>, team: i16) {
    if list.len() < BOOKED {
        list.push(team);
    }
}

impl Moves {
    fn relegate(&mut self, level: usize, team: i16) {
        book(&mut self.down[level], team);
        self.log.push((false, level, team));
    }

    fn promote(&mut self, level: usize, team: i16) {
        book(&mut self.up[level], team);
        self.log.push((true, level, team));
    }

    fn force(&mut self, level: usize, team: i16) {
        book(&mut self.forced[level], team);
    }

    /// Whether a club came up from a level (`FUN_00680569`), was sent
    /// down from one (`FUN_00680623`), went down from one (`FUN_006805c6`).
    fn promoted(&self, level: usize, team: i16) -> bool {
        self.up[level].contains(&team)
    }

    fn forced_at(&self, level: usize, team: i16) -> bool {
        self.forced[level].contains(&team)
    }

    fn relegated_at(&self, level: usize, team: i16) -> bool {
        self.down[level].contains(&team)
    }

    /// The books as a country block keeps them.
    pub fn read(block: &[u8]) -> Self {
        let word = |at: usize| block.get(at..at + 2).map_or(0, |w| i16::from_le_bytes([w[0], w[1]]));
        let list = |counts: usize, lists: usize, level: usize| -> Vec<i16> {
            let n = usize::try_from(word(counts + 2 * level)).unwrap_or(0).min(BOOKED);
            (0..n).map(|k| word(lists + level * 2 * BOOKED + 2 * k)).collect()
        };
        Self {
            down: std::array::from_fn(|l| list(DOWN_COUNTS, DOWN_LISTS, l)),
            up: std::array::from_fn(|l| list(UP_COUNTS, UP_LISTS, l)),
            forced: std::array::from_fn(|l| list(FORCED_COUNTS, FORCED_LISTS, l)),
            log: Vec::new(),
        }
    }

    /// Into the country block: the counts, and the lists' first entries —
    /// the others stand as they stood (`FUN_00680d37`).
    fn store(&self, block: &mut [u8]) {
        let mut put = |at: usize, v: i16| {
            if let Some(w) = block.get_mut(at..at + 2) {
                w.copy_from_slice(&v.to_le_bytes());
            }
        };
        let lists: [(&[Vec<i16>], usize, usize); 3] = [
            (&self.down, DOWN_COUNTS, DOWN_LISTS),
            (&self.up, UP_COUNTS, UP_LISTS),
            (&self.forced, FORCED_COUNTS, FORCED_LISTS),
        ];
        for (levels, counts, at) in lists {
            for (level, list) in levels.iter().enumerate() {
                put(counts + 2 * level, list.len() as i16);
                for (k, &team) in list.iter().enumerate() {
                    put(at + level * 2 * BOOKED + 2 * k, team);
                }
            }
        }
    }
}

/// A league as the draw reads it: its size, its moves and its closed table.
struct Closed {
    teams: usize,
    up: usize,
    first_down: usize,
    rows: Vec<Row>,
}

fn league_word(records: &Records, country: u16, level: u8, field: usize) -> i16 {
    League::block_name(records, country, level)
        .and_then(|name| records.block(&name))
        .and_then(|b| b.get(LEAGUE_FIELDS + field..LEAGUE_FIELDS + field + 2))
        .map_or(0, |w| i16::from_le_bytes([w[0], w[1]]))
}

fn closed(records: &Records, country: u16, level: u8) -> Option<Closed> {
    let league = League::of(records, country, level)?;
    let teams = usize::try_from(league_word(records, country, level, TEAMS)).ok()?.min(LIST);
    let down = usize::try_from(league_word(records, country, level, DOWN)).unwrap_or(0);
    let up = usize::try_from(league_word(records, country, level, UP)).unwrap_or(0);
    let rows = statistics::table(records, &league, ALL);
    (rows.len() >= teams).then_some(Closed { teams, up, first_down: teams.saturating_sub(down), rows })
}

/// A club's first team when it is a reserve team (`FUN_0063e793`).
fn first_team(records: &Records, country: u16, team: i16) -> Option<i16> {
    let parent = records.team(country, u16::try_from(team).ok()?)?.linked()[0];
    (parent >= 0).then_some(i16::from(parent))
}

/// A club's level (`FUN_0063e2df`): 10 outside the leagues.
fn level(records: &Records, country: u16, team: i16) -> u8 {
    u16::try_from(team).ok().and_then(|n| records.team(country, n)).map_or(OUTSIDE, |t| t.level())
}

/// A random order of `n` (`FUN_00675630`): each place in turn drawn
/// among those still free.
fn permutation(n: usize, random: &mut Random) -> Vec<usize> {
    let mut order: Vec<Option<usize>> = vec![None; n];
    for k in 0..n {
        loop {
            let at = random.below(n as u32) as usize;
            if order[at].is_none() {
                order[at] = Some(k);
                break;
            }
        }
    }
    order.into_iter().map(|k| k.unwrap_or(0)).collect()
}

/// Who may come up into a country's first league (`FUN_00680706`): with
/// one league the clubs outside it, in their order; else the second
/// league's clubs.
fn risers(records: &Records, country: u16) -> Vec<i16> {
    if records.countries.leagues(usize::from(country)) == 1 {
        (0..records.countries.team_count(usize::from(country)).max(0))
            .filter(|&k| level(records, country, k) > 9)
            .collect()
    } else {
        League::of(records, country, 1).map_or_else(Vec::new, |l| (0..l.teams()).map(|i| l.team(i)).collect())
    }
}

/// The plain rule for a country's first league (`FUN_006b2212`, and for
/// `FAR` `FUN_006b1f12`): from the bottom up to five places, a club sent
/// down whatever its place and every club below the safe places go out,
/// the others stay in their table's order, and the places left fill with
/// the clubs drawn from those who may come up.
fn single(records: &Records, country: u16, moves: &mut Moves, order_of: &mut Orders) -> Option<Vec<i16>> {
    let l = closed(records, country, 0)?;
    let down_level = usize::from(country == FAR);
    let mut place = l.teams as isize - 1;
    while place >= l.teams as isize - BOTTOM as isize {
        let row = l.rows[place as usize];
        if row.sent_down() {
            moves.force(0, row.team);
        } else if (place as usize) < l.first_down {
            break;
        }
        moves.relegate(down_level, row.team);
        place -= 1;
    }
    let mut next: Vec<i16> = l.rows[..(place + 1).max(0) as usize].iter().map(|r| r.team).collect();
    let pool = risers(records, country);
    let order = order_of(pool.len());
    let mut k = 0;
    while next.len() < l.teams && k < pool.len() {
        let team = pool[order[k]];
        k += 1;
        if country == FAR && first_team(records, country, team).is_some() {
            continue;
        }
        next.push(team);
        moves.promote(0, team);
    }
    Some(next)
}

/// A club of the Regionalligen's pool (`FUN_0068fb1e`): its region and
/// how far south it lies (team +0x924's bit, team +0x310).
#[derive(Clone, Copy)]
struct Placed {
    team: i16,
    region: i16,
    south: i16,
}

fn placed(records: &Records, team: i16) -> Placed {
    let t = u16::try_from(team).ok().and_then(|n| records.team(GERMANY, n));
    Placed {
        team,
        region: t.map_or(0, |t| i16::from(t.legal_form() & SOUTH != 0)),
        south: t.map_or(0, |t| t.map_at()[1]),
    }
}

/// The n-th best club of a Regionalliga that is no reserve team
/// (`FUN_006b3a90` from 2000, `FUN_006b39d1`): the k-th riser of the
/// four, two from each.
fn regional_riser(records: &Records, k: usize) -> i16 {
    let Some(l) = closed(records, GERMANY, (k / 2 + 2) as u8) else { return 0 };
    let n = k & 1;
    let mut left = n as isize;
    for row in l.rows.iter().take(l.teams) {
        if first_team(records, GERMANY, row.team).is_none() {
            left -= 1;
            if left < 0 {
                return row.team;
            }
        }
    }
    l.rows.get(n).map_or(0, |r| r.team)
}

/// A Regionalliga's season (`FUN_0069128a`): its clubs keyed by their
/// place, its best two that are no reserve team booked up, its staying
/// clubs into the pool — a reserve team only while its first team stays
/// in the top two leagues, else sent down — and its bottom four booked
/// down (a level 2 for both leagues, as the original books them).
fn regional(records: &Records, level: u8, keys: &mut [i16], pool: &mut Vec<Placed>, moves: &mut Moves) {
    let Some(l) = closed(records, GERMANY, level) else { return };
    let at = usize::from(level);
    for (place, row) in l.rows.iter().take(l.teams).enumerate() {
        if let Some(key) = usize::try_from(row.team).ok().and_then(|t| keys.get_mut(t)) {
            *key = place as i16 + 1;
        }
    }
    let mut up = 0;
    for row in l.rows.iter().take(l.teams) {
        if first_team(records, GERMANY, row.team).is_none() {
            moves.promote(at, row.team);
            up += 1;
            if up == l.up {
                break;
            }
        }
    }
    let mut place = 0;
    while place < l.first_down && !l.rows[place].sent_down() {
        let team = l.rows[place].team;
        if !moves.promoted(at, team) {
            match first_team(records, GERMANY, team) {
                None => pool.push(placed(records, team)),
                Some(first)
                    if !moves.forced_at(0, first) && !moves.forced_at(1, first) && !moves.relegated_at(1, first) =>
                {
                    pool.push(placed(records, team))
                }
                Some(_) => moves.force(1, team),
            }
        }
        place += 1;
    }
    let mut place = l.teams;
    while place > 0 {
        place -= 1;
        if place + 4 < l.teams {
            return;
        }
        let row = l.rows[place];
        if row.sent_down() {
            moves.force(1, row.team);
            continue;
        }
        if place < l.first_down {
            return;
        }
        moves.relegate(2, row.team);
    }
}

/// Moves `n` clubs from one region's candidates into the other's, the
/// nearest first: by how far south, ascending or descending, ties in
/// their order.
fn borrow(records: &Records, from: &mut Vec<i16>, to: &mut Vec<i16>, northernmost: bool) {
    let mut by: Vec<(i16, i16)> = from.iter().map(|&t| (placed(records, t).south, t)).collect();
    if northernmost {
        by.sort_by_key(|&(south, _)| south);
    } else {
        by.sort_by_key(|&(south, _)| std::cmp::Reverse(south));
    }
    for &(_, team) in by.iter().take(DRAWN_PER_REGION) {
        to.push(team);
        if let Some(k) = from.iter().position(|&t| t == team) {
            from.remove(k);
        }
    }
}

/// The original's exchange sort: each place takes the first of the rest
/// that comes before it, swapping as it goes.
fn exchange_sort<T>(items: &mut [T], before: impl Fn(&T, &T) -> bool) {
    for i in 0..items.len() {
        for j in i + 1..items.len() {
            if before(&items[j], &items[i]) {
                items.swap(i, j);
            }
        }
    }
}

/// Germany's four leagues from 2000 on (`FUN_006ae26f`).
fn germany(records: &Records, moves: &mut Moves, order_of: &mut Orders) -> Option<[Vec<i16>; 4]> {
    let (l0, l1) = (closed(records, GERMANY, 0)?, closed(records, GERMANY, 1)?);
    let mut pool: Vec<Placed> = Vec::new();
    let mut next1: Vec<i16> = Vec::new();
    let mut sent0 = 0;
    // The Bundesliga: its bottom down into the 2. Bundesliga, a club sent
    // down straight into the Regionalligen; the 2. Bundesliga's best up.
    let mut place = l0.teams as isize - 1;
    while place >= l0.teams as isize - BOTTOM as isize {
        let row = l0.rows[place as usize];
        if row.sent_down() {
            moves.force(0, row.team);
            moves.relegate(0, row.team);
            pool.push(placed(records, row.team));
            sent0 += 1;
        } else {
            if (place as usize) < l0.first_down {
                break;
            }
            moves.relegate(0, row.team);
            next1.push(row.team);
        }
        place -= 1;
    }
    let mut next0: Vec<i16> = l0.rows[..(place + 1).max(0) as usize].iter().map(|r| r.team).collect();
    let mut k = 0;
    while next0.len() < l0.teams && k < l1.rows.len() {
        let team = l1.rows[k].team;
        moves.promote(0, team);
        next0.push(team);
        k += 1;
    }
    // The 2. Bundesliga: its bottom into the Regionalligen — as many
    // fewer as the Bundesliga sent down —, the rest stays, and the
    // Regionalligen's best two each come up.
    let mut place = l1.teams as isize - 1;
    while place >= l1.teams as isize - BOTTOM as isize {
        let row = l1.rows[place as usize];
        if row.sent_down() {
            moves.force(0, row.team);
        } else if (place as usize) < l1.first_down + sent0 {
            break;
        }
        moves.relegate(1, row.team);
        pool.push(placed(records, row.team));
        place -= 1;
    }
    for row in l1.rows.iter().take((place + 1).max(0) as usize).skip(k) {
        next1.push(row.team);
    }
    let risen: [i16; 4] = std::array::from_fn(|k| regional_riser(records, k));
    for &team in &risen {
        moves.promote(1, team);
    }
    next1.extend(risen);
    // The Regionalligen: their staying clubs, the clubs from above and
    // eight from outside, four a region.
    let clubs = usize::try_from(records.countries.team_count(usize::from(GERMANY))).unwrap_or(0);
    let mut keys = vec![0i16; clubs];
    regional(records, 2, &mut keys, &mut pool, moves);
    regional(records, 3, &mut keys, &mut pool, moves);
    let (mut north, mut south): (Vec<i16>, Vec<i16>) = (Vec::new(), Vec::new());
    for team in 0..clubs as i16 {
        if level(records, GERMANY, team) <= 9 {
            continue;
        }
        let may = match first_team(records, GERMANY, team) {
            None => true,
            Some(first) => {
                level(records, GERMANY, first) < 2
                    && !moves.forced_at(0, first)
                    && !moves.forced_at(1, first)
                    && !moves.relegated_at(1, first)
            }
        };
        if !may {
            continue;
        }
        if placed(records, team).region == 0 {
            north.push(team);
        } else {
            south.push(team);
        }
        keys[team as usize] = FROM_OUTSIDE;
    }
    if north.len() < DRAWN_PER_REGION {
        borrow(records, &mut south, &mut north, true);
    }
    if south.len() < DRAWN_PER_REGION {
        borrow(records, &mut north, &mut south, false);
    }
    let order_north = order_of(north.len());
    for &k in order_north.iter().take(DRAWN_PER_REGION) {
        moves.promote(2, north[k]);
        pool.push(placed(records, north[k]));
    }
    // The original draws the south's order into the same buffer, and its
    // fill below reads both regions through it.
    let order = order_of(south.len());
    for &k in order.iter().take(DRAWN_PER_REGION) {
        moves.promote(3, south[k]);
        pool.push(placed(records, south[k]));
    }
    let (mut used_north, mut used_south, mut turn) = (DRAWN_PER_REGION, DRAWN_PER_REGION, 0);
    while pool.len() < REGIONAL_CLUBS {
        let take_north = |used: usize| used < north.len() && order.get(used).is_some_and(|&k| k < north.len());
        let take_south = |used: usize| used < south.len() && order.get(used).is_some_and(|&k| k < south.len());
        let north_first = turn % 2 == 0;
        if (north_first || !take_south(used_south)) && take_north(used_north) {
            let team = north[order[used_north]];
            moves.promote(2, team);
            pool.push(placed(records, team));
            used_north += 1;
        } else if take_south(used_south) {
            let team = south[order[used_south]];
            moves.promote(3, team);
            pool.push(placed(records, team));
            used_south += 1;
        } else {
            break;
        }
        turn += 1;
    }
    exchange_sort(&mut pool, |a, b| a.region < b.region || (a.region == b.region && a.south < b.south));
    let key = |p: &Placed| usize::try_from(p.team).ok().and_then(|t| keys.get(t)).copied().unwrap_or(0);
    let teams2 = closed(records, GERMANY, 2).map_or(LIST / 2 + 6, |l| l.teams).min(pool.len());
    let (north_league, south_league) = pool.split_at_mut(teams2);
    let teams3 = closed(records, GERMANY, 3).map_or(south_league.len(), |l| l.teams).min(south_league.len());
    exchange_sort(north_league, |a, b| key(a) < key(b));
    exchange_sort(&mut south_league[..teams3], |a, b| key(a) < key(b));
    let next2 = north_league.iter().map(|p| p.team).collect();
    let next3 = south_league[..teams3].iter().map(|p| p.team).collect();
    Some([next0, next1, next2, next3])
}

/// Next season's list into a league block's +0x52.
fn put_next(records: &mut Records, country: u16, level: u8, list: &[i16]) {
    let Some(name) = League::block_name(records, country, level) else { return };
    let Some(block) = records.block_mut(&name) else { return };
    for k in 0..LIST {
        let at = LEAGUE_FIELDS + NEXT + 2 * k;
        if let Some(w) = block.get_mut(at..at + 2) {
            w.copy_from_slice(&list.get(k).copied().unwrap_or(0).to_le_bytes());
        }
    }
}

/// The leagues drawn, by country and level.
pub type Drawn = Vec<(u16, u8)>;

/// Where the draws' random orders come from: `n` places in the order
/// they are taken.
pub type Orders<'a> = dyn FnMut(usize) -> Vec<usize> + 'a;

/// The season's draw of every played country (`FUN_006b24f2`): its
/// books cleared and filled, next season's lists beside the current.
pub fn draw(records: &mut Records, random: &mut Random) -> Drawn {
    draw_with(records, &mut |n| permutation(n, random))
}

/// The same with the orders from `order_of`.
pub fn draw_with(records: &mut Records, order_of: &mut Orders) -> Drawn {
    let mut drawn = Drawn::new();
    let main = u16::try_from(records.globals.main_country()).unwrap_or(0);
    for country in crate::opponent::played_nations(records) {
        let mut moves = Moves::default();
        let lists: Vec<Vec<i16>> = if country == GERMANY && country == main {
            germany(records, &mut moves, order_of).map_or_else(Vec::new, |l| l.to_vec())
        } else {
            single(records, country, &mut moves, order_of).into_iter().collect()
        };
        for (level, list) in lists.iter().enumerate() {
            put_next(records, country, level as u8, list);
            drawn.push((country, level as u8));
        }
        let slot = records.countries.slot(usize::from(country));
        if let Some(block) = records.block_mut(&format!("0x19f7fd0[{slot}]")) {
            moves.store(block);
        }
        record_moves(records, country, &moves.log);
    }
    drawn
}

/// A player's counts of promotions and relegations (R+0x350, +0x351).
const PROMOTIONS: usize = 0x350;
const RELEGATIONS: usize = 0x351;
/// The record lists of the players promoted and relegated most often with
/// the main country's clubs: twenty entries of 0xc bytes.
const MOST_PROMOTED: &str = "0xb06bb4";
const MOST_RELEGATED: &str = "0xb06a90";
const MOST: usize = 20;
const MOST_ENTRY: usize = 0xc;
/// Bit 31 of R+0x1b4: he is known by his nickname.
const BY_NICKNAME: u32 = 0x8000_0000;

/// An entry of a record list (`FUN_00723eaa`, `FUN_00723fdc`): his id,
/// his names — none and the nickname when he goes by it —, his club, the
/// count.
fn entry(player: &crate::records::Player, count: u8) -> [u8; MOST_ENTRY] {
    let (first, last) = if player.shown() & BY_NICKNAME != 0 {
        (0, player.nickname())
    } else {
        (player.first_name(), player.last_name())
    };
    let club = player.club();
    let mut e = [0u8; MOST_ENTRY];
    e[0..2].copy_from_slice(&player.id().to_le_bytes());
    e[2..4].copy_from_slice(&first.to_le_bytes());
    e[4..6].copy_from_slice(&last.to_le_bytes());
    e[6] = club[0];
    e[8..10].copy_from_slice(&u16::from(club[1]).to_le_bytes());
    e[10..12].copy_from_slice(&i16::from(count).to_le_bytes());
    e
}

/// Offers an entry to a record list (`CNormaleListe::FUN_006c58fe`, by
/// the count descending, `0x723f8f`): only one that beats the last; it
/// goes before the first it beats, over his own entry if he has one.
fn offer(list: &mut [u8], new: [u8; MOST_ENTRY]) {
    let count = |e: &[u8]| i16::from_le_bytes([e[10], e[11]]);
    let id = |e: &[u8]| u16::from_le_bytes([e[0], e[1]]);
    let at = |k: usize| k * MOST_ENTRY..(k + 1) * MOST_ENTRY;
    if list.len() < MOST * MOST_ENTRY || count(&new) <= count(&list[at(MOST - 1)]) {
        return;
    }
    let beats = (0..MOST).find(|&k| count(&new) > count(&list[at(k)])).unwrap_or(MOST);
    match (0..MOST).find(|&k| id(&list[at(k)]) == id(&new)) {
        None => {
            list.copy_within(beats * MOST_ENTRY..(MOST - 1) * MOST_ENTRY, (beats + 1) * MOST_ENTRY);
            list[at(beats)].copy_from_slice(&new);
        }
        Some(own) => {
            list[at(own)].copy_from_slice(&new);
            if beats < own {
                list.copy_within(beats * MOST_ENTRY..own * MOST_ENTRY, (beats + 1) * MOST_ENTRY);
                list[at(beats)].copy_from_slice(&new);
            }
        }
    }
}

/// The squads of the season's promoted and relegated clubs count it
/// (`FUN_007104dc`, `FUN_00710373` per played country: `FUN_0071021f`,
/// `FUN_00710309`): every player one promotion or relegation more, and
/// with a club of the main country each offered to the record lists of
/// the most promotions and relegations. A promoted club with no human
/// trainer (club +0x8a) counts the promotion in its staff's careers too:
/// its trainer's, manager's and manager-trainer's person +0x44 one more
/// (`FUN_006b53ad` with kind 2, `FUN_00408e0a`). (The trainers' all-time
/// list beside, `FUN_00725f33`, is not built.)
const STAFF_PROMOTIONS: usize = 0x44;
const LAST_PERSON: u16 = 0x5db;

pub fn count(records: &mut Records) {
    let main = u16::try_from(records.globals.main_country()).unwrap_or(0);
    for country in crate::opponent::played_nations(records) {
        let books = moves(records, country);
        for level in 0..books.up.len() {
            for (clubs, counter, list) in
                [(&books.up[level], PROMOTIONS, MOST_PROMOTED), (&books.down[level], RELEGATIONS, MOST_RELEGATED)]
            {
                for &club in clubs {
                    let Ok(number) = u16::try_from(club) else { continue };
                    let squad = records.team(country, number).map_or_else(Vec::new, |t| t.players());
                    if counter == PROMOTIONS {
                        let staff = records.team(country, number).filter(|t| t.humans()[0] == 0).map_or([0; 3], |t| t.staff());
                        for id in staff.into_iter().filter(|&id| id != 0 && id <= LAST_PERSON) {
                            if let Some(p) = records.persons.get_mut(usize::from(id)) {
                                p.put::<i16>(STAFF_PROMOTIONS, p.get::<i16>(STAFF_PROMOTIONS).wrapping_add(1));
                            }
                        }
                    }
                    for &id in &squad {
                        if let Some(p) = records.player_mut(id) {
                            p.put::<u8>(counter, p.get::<u8>(counter).wrapping_add(1));
                        }
                    }
                    if country != main {
                        continue;
                    }
                    for &id in &squad {
                        let Some(p) = records.player(id) else { continue };
                        let n = p.get::<u8>(counter);
                        if n == 0 {
                            continue;
                        }
                        let new = entry(p, n);
                        if let Some(block) = records.block_mut(list) {
                            offer(block, new);
                        }
                    }
                }
            }
        }
    }
}

/// The clubs' record of moves, a played country's club by its place among
/// all teams (`FUN_00726382`: `0xb06cd8`, four i16 a club — its
/// promotions, and those into the first, second and third league;
/// `FUN_007264bf`: `0xb07558`, five — its relegations, and those out of
/// the first four), counted as the books take each move (`FUN_006b6a33`,
/// `FUN_006b6b22`), beyond the four a level the books keep as well. The
/// same moves count by country slot in the lists block (`FUN_00634d9c`:
/// 0xb1c508, 0x288 a slot, three i16 a club; `FUN_00634eff`: 0xb1d1b0,
/// 0x360 a slot, four), and a club up into the first or the second league
/// counts it in its history (`FUN_00640b2d`, `FUN_00640b9f`: the object
/// 0xa6d348, 0x80 a club, +0xc and +0xe).
const PROMOTION_RECORD: (&str, usize, usize) = ("0xb06cd8", 4, 3);
const RELEGATION_RECORD: (&str, usize, usize) = ("0xb07558", 5, 4);
const LISTS: &str = "0xb0b6b8";
const SLOT_PROMOTIONS: (usize, usize, usize, usize) = (0xb1c508 - 0xb0b6b8, 0x288, 6, 3);
const SLOT_RELEGATIONS: (usize, usize, usize, usize) = (0xb1d1b0 - 0xb0b6b8, 0x360, 8, 4);
const HISTORY: &str = "0xa6d348";
const HISTORY_CLUB: usize = 0x80;
const HISTORY_UP: [usize; 2] = [0xc, 0xe];

fn bump_word(block: &mut [u8], at: usize) {
    if let Some(w) = block.get_mut(at..at + 2) {
        let v = i16::from_le_bytes([w[0], w[1]]).wrapping_add(1);
        w.copy_from_slice(&v.to_le_bytes());
    }
}

fn record_moves(records: &mut Records, country: u16, log: &[(bool, usize, i16)]) {
    let first = usize::try_from(records.countries.first_team(usize::from(country))).unwrap_or(0);
    let teams = records.countries.team_count(usize::from(country)).max(0);
    let slot = usize::from(records.countries.slot(usize::from(country)));
    for &(up, level, club) in log {
        if club < 0 || club >= teams {
            continue;
        }
        let (base, per_slot, per_club, kinds) = if up { SLOT_PROMOTIONS } else { SLOT_RELEGATIONS };
        if level < kinds
            && let Some(block) = records.block_mut(LISTS)
        {
            bump_word(block, base + slot * per_slot + club as usize * per_club + level * 2);
        }
        if let Some(&at) = HISTORY_UP.get(level).filter(|_| up)
            && let Some(block) = records.block_mut(HISTORY)
        {
            bump_word(block, (first + club as usize) * HISTORY_CLUB + at);
        }
        let (name, width, kinds) = if up { PROMOTION_RECORD } else { RELEGATION_RECORD };
        let Some(block) = records.block_mut(name) else { continue };
        let at = (first + club as usize) * width * 2;
        let mut add = |k: usize| {
            if let Some(w) = block.get_mut(at + 2 * k..at + 2 * k + 2) {
                let v = i16::from_le_bytes([w[0], w[1]]).wrapping_add(1);
                w.copy_from_slice(&v.to_le_bytes());
            }
        };
        add(0);
        if level < kinds {
            add(1 + level);
        }
    }
}

/// The level a club plays next season by the drawn lists (`FUN_00692bf9`:
/// the league whose next list holds it — Germany from 2000 four at most —,
/// 10 in none, 0 for a country not played).
pub fn next_level(records: &Records, country: u16, team: i16) -> i16 {
    if !records.globals.played(country as i16) {
        return 0;
    }
    let mut leagues = records.countries.leagues(usize::from(country)).max(0);
    if country == GERMANY && records.globals.start_year() >= 2000 {
        leagues = leagues.min(4);
    }
    for level in 0..leagues as u8 {
        let Some(name) = League::block_name(records, country, level) else { continue };
        let Some(block) = records.block(&name) else { continue };
        let teams = usize::try_from(league_word(records, country, level, TEAMS)).unwrap_or(0).min(LIST);
        let found = (0..teams).any(|k| {
            let at = LEAGUE_FIELDS + NEXT + 2 * k;
            block.get(at..at + 2).is_some_and(|w| i16::from_le_bytes([w[0], w[1]]) == team)
        });
        if found {
            return i16::from(level);
        }
    }
    i16::from(OUTSIDE)
}

/// Next season's clubs of a league: its drawn list, or its list as it
/// stands when none is drawn.
pub fn next_list(records: &Records, country: u16, level: u8) -> Vec<i16> {
    let Some(block) = League::block_name(records, country, level).and_then(|name| records.block(&name)) else {
        return Vec::new();
    };
    let teams = usize::try_from(league_word(records, country, level, TEAMS)).unwrap_or(0).min(LIST);
    let read = |from: usize| -> Vec<i16> {
        (0..teams)
            .map(|k| {
                let at = LEAGUE_FIELDS + from + 2 * k;
                block.get(at..at + 2).map_or(0, |w| i16::from_le_bytes([w[0], w[1]]))
            })
            .collect()
    };
    let next = read(NEXT);
    if next.iter().all(|&t| t == 0) { read(CURRENT) } else { next }
}

/// The country's books, as its block keeps them.
pub fn moves(records: &Records, country: u16) -> Moves {
    let slot = records.countries.slot(usize::from(country));
    records.block(&format!("0x19f7fd0[{slot}]")).map_or_else(Moves::default, Moves::read)
}

/// The season's flags of every played country's clubs (`FUN_006b2543`:
/// `FUN_006b258c` for the main country, `FUN_006b290c` for the more,
/// `FUN_006b38ac`, `FUN_0063e0d1`): the first league's champion and
/// runner-up, the cup's winner and finalist, the promoted and the
/// relegated of every level booked — the more countries' of their first
/// level only —, and the sent down; they take team +0x110's bits of the
/// season, the others stay.
pub fn flag(records: &mut Records, tables: &TicketTables) {
    let main = u16::try_from(records.globals.main_country()).unwrap_or(0);
    // The main country's routine takes the new clubs of as many leagues
    // from its first as its block's count at +0x24 says (`FUN_006b258c`:
    // six for Germany's four) — the more countries' first leagues among
    // them have taken theirs when their own flags are reckoned.
    let early = {
        let slot = records.countries.slot(usize::from(main));
        let first = records.countries.first_league(usize::from(main));
        let count = records
            .block(&format!("0x19f7fd0[{slot}]"))
            .and_then(|b| b.get(MAIN_LEAGUES..MAIN_LEAGUES + 2))
            .map_or(0, |w| i16::from_le_bytes([w[0], w[1]]));
        first..first.saturating_add(count)
    };
    for country in crate::opponent::played_nations(records) {
        let clubs = usize::try_from(records.countries.team_count(usize::from(country))).unwrap_or(0);
        let mut flags = vec![0u16; clubs];
        let mut mark = |team: i16, flag: u16| {
            if let Some(f) = usize::try_from(team).ok().and_then(|t| flags.get_mut(t)) {
                *f |= flag;
            }
        };
        let switched = country != main && early.contains(&records.countries.first_league(usize::from(country)));
        let kept = if switched { swap_in_next(records, country) } else { None };
        let table = closed(records, country, 0);
        if let Some(old) = kept {
            put_current(records, country, &old);
        }
        if let Some(l) = table {
            if let Some(row) = l.rows.first() {
                mark(row.team, CHAMPION);
            }
            if let Some(row) = l.rows.get(1) {
                mark(row.team, RUNNER_UP);
            }
        }
        if let Some((winner, loser)) = u8::try_from(country).ok().and_then(|c| tickets::cup_final(records, c, tables)) {
            mark(winner as i16, CUP_WINNER);
            mark(loser as i16, CUP_FINALIST);
        }
        let books = moves(records, country);
        let levels = if country == main { 4 } else { 1 };
        for level in 0..levels {
            books.up[level].iter().for_each(|&t| mark(t, PROMOTED));
            books.down[level].iter().for_each(|&t| mark(t, RELEGATED));
        }
        books.forced.iter().flatten().for_each(|&t| mark(t, RELEGATED));
        if records.countries.kind(usize::from(country)) != 1 {
            continue;
        }
        for (number, &f) in flags.iter().enumerate().filter(|&(n, _)| n != NO_CLUB) {
            if let Some(team) = records.team_mut(country, number as u16) {
                team.put::<u16>(0x110, (team.get::<u16>(0x110) & !FLAGS_SET) | f);
            }
        }
    }
}

/// A country's first league with next season's clubs in its list; the
/// list it held, to put back.
fn swap_in_next(records: &mut Records, country: u16) -> Option<Vec<u8>> {
    let name = League::block_name(records, country, 0)?;
    let block = records.block_mut(&name)?;
    let (next, current) = (LEAGUE_FIELDS + NEXT, LEAGUE_FIELDS + CURRENT);
    let old = block.get(current..current + 2 * LIST)?.to_vec();
    block.copy_within(next..next + 2 * LIST, current);
    Some(old)
}

fn put_current(records: &mut Records, country: u16, list: &[u8]) {
    let Some(name) = League::block_name(records, country, 0) else { return };
    let Some(block) = records.block_mut(&name) else { return };
    let current = LEAGUE_FIELDS + CURRENT;
    if let Some(s) = block.get_mut(current..current + list.len()) {
        s.copy_from_slice(list);
    }
}

/// The drawn leagues take their new clubs (`FUN_006926aa` with
/// `FUN_0069238b`): next season's list becomes the league's list.
pub fn take(records: &mut Records, drawn: &Drawn) {
    for &(country, level) in drawn {
        let Some(name) = League::block_name(records, country, level) else { continue };
        let Some(block) = records.block_mut(&name) else { continue };
        let (next, current) = (LEAGUE_FIELDS + NEXT, LEAGUE_FIELDS + CURRENT);
        if block.len() >= next + 2 * LIST {
            block.copy_within(next..next + 2 * LIST, current);
        }
    }
}

/// The new season's clubs at their leagues (`FUN_00774088` per country
/// played): every club of a league at the league's level and its place in
/// the list, the first league's clubs one more season up there in the
/// lists block's count, every other club of the country at 10.
pub fn place(records: &mut Records) {
    for country in crate::opponent::played_nations(records) {
        let clubs = usize::try_from(records.countries.team_count(usize::from(country))).unwrap_or(0);
        let mut listed = vec![false; clubs];
        let slot = usize::from(records.countries.slot(usize::from(country)));
        for level in 0..records.countries.leagues(usize::from(country)).max(0) as u8 {
            let Some(league) = League::of(records, country, level) else { continue };
            let teams: Vec<i16> = (0..league.teams()).map(|i| league.team(i)).collect();
            if level == 0
                && let Some(block) = records.block_mut("0xb0b6b8")
            {
                for &team in &teams {
                    let at = TOP_SEASONS + usize::try_from(team).unwrap_or(0) * 4 + slot * TOP_SEASONS_SLOT;
                    if let Some(w) = block.get_mut(at..at + 2) {
                        let n = i16::from_le_bytes([w[0], w[1]]).wrapping_add(1);
                        w.copy_from_slice(&n.to_le_bytes());
                    }
                }
            }
            for (place, &team) in teams.iter().enumerate() {
                let Ok(number) = u16::try_from(team) else { continue };
                if let Some(t) = records.team_mut(country, number) {
                    t.set_level(level);
                    t.set_league_place(place as u8);
                }
                if let Some(l) = listed.get_mut(usize::from(number)) {
                    *l = true;
                }
            }
        }
        for (number, _) in listed.iter().enumerate().filter(|&(_, &l)| !l) {
            if let Some(t) = records.team_mut(country, number as u16) {
                t.set_level(OUTSIDE);
                t.set_league_place(0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_permutation_takes_every_place_once() {
        let mut random = Random::new(4);
        for n in [0, 1, 2, 7, 20] {
            let mut order = permutation(n, &mut random);
            order.sort_unstable();
            assert_eq!(order, (0..n).collect::<Vec<_>>());
        }
    }

    #[test]
    fn the_books_keep_four_a_level() {
        let mut moves = Moves::default();
        for team in 1..=6 {
            moves.relegate(2, team);
        }
        moves.promote(0, 9);
        moves.force(1, 3);
        assert_eq!(moves.down[2], [1, 2, 3, 4]);
        assert_eq!(moves.log.len(), 7, "the record of moves takes all six and the promotion");
        let mut block = vec![0xffu8; 0x300];
        moves.store(&mut block);
        assert_eq!(Moves::read(&block), moves);
        assert_eq!(block[DOWN_LISTS], 0xff, "a level's list past its count stands");
    }

    /// The season's end over the original's pair (SeasonEnd364 →
    /// NewSeason000), the draws fed the original's own picks — the clubs
    /// its books and its new lists name as come from outside: every
    /// played league's next clubs, every country's books, every player's
    /// counts of promotions and relegations, the record lists of them and
    /// every club's season flags stand as the original's (the more countries' first
    /// leagues the main country's routine switched early reckoning their
    /// champion and runner-up through their new clubs, as there), every
    /// club's record of promotions and relegations, and
    /// after the new season's placing every club's level and place in its
    /// list, and the first leagues' count of seasons.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_season_end_promotes_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let read = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs, before) =
            (read("SeasonEnd364.an3"), read("NewSeason000.an3"), read("SeasonEnd364.an3"));
        let list = |r: &Records, c: u16, l: u8| -> Vec<i16> {
            League::of(r, c, l).map_or_else(Vec::new, |x| (0..x.teams()).map(|i| x.team(i)).collect())
        };
        // The original's picks, draw by draw in the order the draws come.
        let mut picks: Vec<Vec<i16>> = Vec::new();
        for country in crate::opponent::played_nations(&before) {
            let books = moves(&theirs, country);
            if country == GERMANY {
                for (level, booked) in [(2u8, &books.up[2]), (3, &books.up[3])] {
                    let old: Vec<i16> = [1u8, 2, 3].iter().flat_map(|&l| list(&before, GERMANY, l)).collect();
                    let mut p: Vec<i16> = booked[2..].to_vec();
                    let more: Vec<i16> =
                        list(&theirs, GERMANY, level).into_iter().filter(|t| !old.contains(t) && !p.contains(t)).collect();
                    p.extend(more);
                    picks.push(p);
                }
            } else {
                picks.push(books.up[0].clone());
            }
        }
        let mut draws = picks.into_iter();
        let pools = candidate_pools(&before);
        let mut k = 0;
        draw_with(&mut ours, &mut |n| {
            let wanted = draws.next().unwrap_or_default();
            let pool = &pools[k];
            k += 1;
            assert_eq!(pool.len(), n, "draw {k}'s candidates");
            let mut order: Vec<usize> = wanted.iter().filter_map(|w| pool.iter().position(|t| t == w)).collect();
            assert_eq!(order.len(), wanted.len(), "draw {k}: every pick among the candidates");
            let rest: Vec<usize> = (0..n).filter(|i| !order.contains(i)).collect();
            order.extend(rest);
            order
        });
        for country in crate::opponent::played_nations(&before) {
            let levels = if country == GERMANY { 4 } else { 1 };
            for level in 0..levels {
                let name = League::block_name(&ours, country, level).unwrap();
                let b = ours.block(&name).unwrap();
                let teams = list(&theirs, country, level).len();
                let next: Vec<i16> = (0..teams)
                    .map(|k| i16::from_le_bytes([b[LEAGUE_FIELDS + NEXT + 2 * k], b[LEAGUE_FIELDS + NEXT + 2 * k + 1]]))
                    .collect();
                assert_eq!(next, list(&theirs, country, level), "{country} league {level}'s next clubs");
            }
            assert_eq!(moves(&ours, country), moves(&theirs, country), "{country}'s books");
        }
        // Every club's record of moves, the moves beyond the books' four
        // a level counted too, by slot as well, and its history.
        for name in [PROMOTION_RECORD.0, RELEGATION_RECORD.0, HISTORY] {
            let (x, y) = (ours.block(name).unwrap(), theirs.block(name).unwrap());
            assert!(x == y, "{name}: {} bytes differ", (0..x.len()).filter(|&k| x[k] != y[k]).count());
            assert_ne!(x, before.block(name).unwrap(), "{name} moved");
        }
        for (base, per_slot, _, _) in [SLOT_PROMOTIONS, SLOT_RELEGATIONS] {
            let (x, y) = (ours.block(LISTS).unwrap(), theirs.block(LISTS).unwrap());
            assert_eq!(x[base..base + 5 * per_slot], y[base..base + 5 * per_slot], "the lists block at {base:#x}");
            let was = before.block(LISTS).unwrap();
            assert_ne!(was[base..base + 5 * per_slot], y[base..base + 5 * per_slot], "{base:#x} moved");
        }
        count(&mut ours);
        for country in crate::opponent::played_nations(&before) {
            for n in 0..ours.countries.team_count(usize::from(country)) as u16 {
                for id in before.team(country, n).unwrap().players() {
                    let (x, y) = (ours.player(id).unwrap(), theirs.player(id).unwrap());
                    let counts = |p: &crate::records::Player| (p.get::<u8>(PROMOTIONS), p.get::<u8>(RELEGATIONS));
                    if before.player(id).unwrap().birthday() == y.birthday() {
                        assert_eq!(counts(x), counts(y), "player {id}'s promotions and relegations");
                    }
                }
            }
        }
        // The promoted clubs' staff count it in their careers.
        let mut moved = 0;
        for id in 1..=usize::from(LAST_PERSON).min(ours.persons.len() - 1) {
            let (x, y) = (ours.persons[id].get::<i16>(STAFF_PROMOTIONS), theirs.persons[id].get::<i16>(STAFF_PROMOTIONS));
            assert_eq!(x, y, "person {id}'s promotions");
            moved += i32::from(x != before.persons[id].get::<i16>(STAFF_PROMOTIONS));
        }
        assert!(moved > 20, "{moved} staff promoted");
        for list in [MOST_PROMOTED, MOST_RELEGATED] {
            let (x, y) = (ours.block(list).unwrap(), theirs.block(list).unwrap());
            for k in 0..MOST * MOST_ENTRY {
                if k % MOST_ENTRY != 7 {
                    assert_eq!(x[k], y[k], "{list} entry {} byte {}", k / MOST_ENTRY, k % MOST_ENTRY);
                }
            }
            assert_ne!(&x[..2], &[0, 0], "{list} filled");
        }
        let data = crate::data::Data::load(&dir).unwrap();
        flag(&mut ours, &data.rules.forecast.tickets);
        for country in crate::opponent::played_nations(&before) {
            for n in 0..ours.countries.team_count(usize::from(country)) as u16 {
                let (x, y) = (ours.team(country, n).unwrap(), theirs.team(country, n).unwrap());
                let season = |t: &crate::records::Team| t.get::<u16>(0x110) & 0x378f;
                assert_eq!(season(x), season(y), "club {country}/{n}'s season flags");
            }
        }
        let drawn: Drawn = crate::opponent::played_nations(&before)
            .into_iter()
            .flat_map(|c| (0..if c == GERMANY { 4 } else { 1 }).map(move |l| (c, l)))
            .collect();
        take(&mut ours, &drawn);
        let slots: Vec<usize> =
            crate::opponent::played_nations(&before).iter().map(|&c| usize::from(before.countries.slot(c.into()))).collect();
        let seasons = |r: &Records| -> Vec<u8> {
            let b = r.block("0xb0b6b8").unwrap();
            slots
                .iter()
                .flat_map(|&s| (0..TOP_SEASONS_SLOT / 4).map(move |k| TOP_SEASONS + s * TOP_SEASONS_SLOT + 4 * k))
                .flat_map(|at| [b[at], b[at + 1]])
                .collect()
        };
        let top_before = seasons(&ours);
        place(&mut ours);
        // The clubs' classes of the new season by their new places.
        crate::settlement::set_classes(&mut ours);
        for country in crate::opponent::played_nations(&before) {
            for n in 0..ours.countries.team_count(usize::from(country)) as u16 {
                let at = crate::settlement::CLASS_AT;
                let (x, y) = (ours.team(country, n).unwrap(), theirs.team(country, n).unwrap());
                assert_eq!(x.get::<u8>(at), y.get::<u8>(at), "club {country}/{n}'s class");
            }
        }
        assert_ne!(seasons(&ours), top_before);
        assert_eq!(seasons(&ours), seasons(&theirs), "the first leagues' seasons");
        for country in crate::opponent::played_nations(&before) {
            for n in 0..ours.countries.team_count(usize::from(country)) as u16 {
                let (x, y) = (ours.team(country, n).unwrap(), theirs.team(country, n).unwrap());
                assert_eq!((x.level(), x.league_place()), (y.level(), y.league_place()), "club {country}/{n}");
            }
        }
    }

    /// Every draw's candidates in the order the draw sees them, as the
    /// rules take them from the closed season (the draws come Germany's
    /// two regions first, then the more countries one each).
    fn candidate_pools(records: &Records) -> Vec<Vec<i16>> {
        let mut pools = Vec::new();
        for country in crate::opponent::played_nations(records) {
            if country == GERMANY {
                let mut moves = Moves::default();
                let _ = germany(records, &mut moves, &mut |n| (0..n).collect());
                // The regions' candidates: clubs outside the leagues by
                // region, as `germany` collects them before its draws.
                let relegated1 = moves.down[1].clone();
                let (mut north, mut south) = (Vec::new(), Vec::new());
                for team in 0..records.countries.team_count(usize::from(GERMANY)) as i16 {
                    if level(records, GERMANY, team) <= 9 {
                        continue;
                    }
                    let may = first_team(records, GERMANY, team)
                        .is_none_or(|f| level(records, GERMANY, f) < 2 && !relegated1.contains(&f));
                    if may {
                        if placed(records, team).region == 0 { north.push(team) } else { south.push(team) }
                    }
                }
                pools.push(north);
                pools.push(south);
            } else {
                pools.push(risers(records, country));
            }
        }
        pools
    }
}
