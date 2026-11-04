//! The season's end enters next season's European cups (`FUN_0041ac69(0)`,
//! the entrants' object `CDatenEuropapokal` at 0x8e7c88).
//!
//! Every country of Europe gives its places first (`FUN_00422b88`): a
//! country kept in short its table's eight clubs, a cup winner drawn of its
//! first six and its seventh as the finalist; a played country its closed
//! first league's eight (`FUN_00423559` from its archive), its cup's winner
//! and finalist by their season flags, the gaps filled from its league's
//! list. By the five seasons of the European ranking (`FUN_00423dc1`) the
//! countries then fill the lists in turn: the Champions League's sixteen
//! seeds — its holder, the six best countries' runners-up, the champions
//! as far as they go —, its 32 qualifiers — the six best's thirds, the
//! three best's fourths, the next nine's runners-up, champions again —, the
//! UEFA cup's direct entrants up to 36 (39 in the game's first season) —
//! its holder, fifths and sixths, fourths and thirds by rank, the cup
//! winners or their finalists — and its 82 qualifiers, and the Intertoto
//! cup's two lists of 12 and 24 from the places left. A club stands in one
//! list at most; the Intertoto takes no club of a played country that has
//! not entered it (team +0x13d). The world cup's opponent is drawn among
//! South America's clubs (`FUN_0063f804`). Each played country's books
//! keep its clubs by cup (`FUN_006b3b92`).

use crate::random::Random;
use crate::records::Records;

/// The entrants' block (from the object's +0x20): the lists' counts, the
/// UEFA cup's limit, the lists, the holders, the world cup's opponent and
/// every country's places.
const ENTRANTS: &str = "0x8e7c88";
const SEEDS: List = List { count: 0x00, at: 0x12, most: 16 };
const QUALIFIERS: List = List { count: 0x02, at: 0x92, most: 32 };
const UEFA: List = List { count: 0x04, at: 0x112, most: i16::MAX };
const UEFA_QUALIFIERS: List = List { count: 0x06, at: 0x2b2, most: 82 };
const INTERTOTO_A: List = List { count: 0x0e, at: 0x43a, most: 24 };
const INTERTOTO_B: List = List { count: 0x10, at: 0x49a, most: 20 };
const LISTS: [List; 6] = [SEEDS, QUALIFIERS, UEFA, UEFA_QUALIFIERS, INTERTOTO_A, INTERTOTO_B];
const UEFA_LIMIT: usize = 0x08;
const COUNTS: [usize; 9] = [0x00, 0x02, 0x04, 0x06, 0x08, 0x0a, 0x0c, 0x0e, 0x10];
const HOLDERS_KEPT: usize = 0x6fa;
const CLEARED: [(usize, usize); 3] = [(0x6fa, 0xe), (0x4ca, 0x230), (0x708, 0xe)];
const WORLD_OPPONENT: usize = 0x708 + 4;
const PLACES: usize = 0x716;
const PLACE_ROW: usize = 12;
const PLACE_COUNTRIES: usize = 0x75;
/// A place's slots: the first eight, the cup's winner and finalist.
const WINNER: usize = 8;
const FINALIST: usize = 9;

/// The holders of the European cups (`DAT_00ae6dd4`): the Champions
/// League's, a second (read in 1999 only) and the UEFA cup's, a country
/// byte and a club word each.
const HOLDERS: &str = "0xae6dd4";
/// The default holders' countries when none is known.
const UEFA_DEFAULT: u8 = 6;
const UEFA_DIRECT: i16 = 0x24;
const UEFA_DIRECT_FIRST: i16 = 0x27;
const INTERTOTO_B_FILLED: i16 = 12;
const INTERTOTO_A_FILLED: i16 = 24;
/// The European ranking (`DAT_00b5d1a8`): 37 rows of 0x24 — the country,
/// its total, its seasons' points.
const RANKING_AT: usize = 0xb5d1a8;
const RANKING_ROW: usize = 0x24;
const RANKED: usize = 0x25;
/// A club of a played country has entered the Intertoto cup (team +0x13d).
const INTERTOTO_ENTERED: usize = 0x13d;
/// The season flags a cup's winner and finalist take (team +0x110).
const CUP_WINNER: u16 = 0x1000;
const CUP_FINALIST: u16 = 0x2000;
/// The world cup's country whose champion meets Europe's, if it keeps
/// teams (`DAT_00ae62f4`, the countries by `DAT_0089e1a8`), and else the
/// clubs abroad (`0x1a28dd8[k]`, forty of them, their country and number at
/// +0x14) of these countries.
const WORLD_COUNTRY: usize = 0xae62f4;
const WORLD_COUNTRIES_WITH_TEAMS: [i16; 5] = [4, 7, 8, 0x21, 0x45];
const ABROAD: usize = 0x28;
const ABROAD_COUNTRY: usize = 0x14;
const SOUTH_AMERICA: [u8; 5] = [4, 7, 8, 0x21, 0x46];
const WORLD_DEFAULT: u8 = 8;
/// The title flag of a club's season (team +0x110 bit 1).
const TITLE: u16 = 1;
/// The country block's European books (from the object's +0x20): the
/// counts of the Champions League's, a fourth unused, the UEFA cup's and the
/// Intertoto's clubs, then their lists.
const BOOK_COUNTS: [usize; 4] = [0x1ac, 0x1ae, 0x1b0, 0x1b2];
const BOOK_CHAMPIONS: usize = 0x1b4;
const BOOK_UEFA: usize = 0x1c0;
const BOOK_INTERTOTO: usize = 0x1ca;
const COUNTRY_CLUBS: usize = 0x22;
const COUNTRY_PLACES: usize = 0x26;

#[derive(Clone, Copy)]
struct List {
    count: usize,
    at: usize,
    most: i16,
}

fn word(b: &[u8], at: usize) -> i16 {
    b.get(at..at + 2).map_or(0, |w| i16::from_le_bytes([w[0], w[1]]))
}

fn put_word(b: &mut [u8], at: usize, v: i16) {
    if let Some(w) = b.get_mut(at..at + 2) {
        w.copy_from_slice(&v.to_le_bytes());
    }
}

/// The entrants as the season's end builds them.
struct Entrants<'a> {
    block: Vec<u8>,
    records: &'a Records,
}

impl Entrants<'_> {
    fn count(&self, list: List) -> i16 {
        word(&self.block, list.count)
    }

    fn entries(&self, list: List) -> impl Iterator<Item = (u8, i16)> + '_ {
        (0..self.count(list).max(0) as usize)
            .map(move |k| (self.block[list.at + 4 * k], word(&self.block, list.at + 4 * k + 2)))
    }

    /// Whether a club cannot enter: none, out of its country's clubs, or in
    /// a list already (`FUN_00419a20`).
    fn barred(&self, country: u8, team: i16) -> bool {
        if country == 0 || team < 0 {
            return true;
        }
        if i32::from(team) >= self.records.countries.team_count(usize::from(country)) as i32 {
            return true;
        }
        LISTS.iter().any(|&l| self.entries(l).any(|e| e == (country, team)))
    }

    fn push(&mut self, list: List, country: u8, team: i16) {
        let n = self.count(list).max(0) as usize;
        let at = list.at + 4 * n;
        if at + 4 <= self.block.len() {
            self.block[at] = country;
            put_word(&mut self.block, at + 2, team);
        }
        put_word(&mut self.block, list.count, n as i16 + 1);
    }

    /// Adds a club to a list: 0 taken, 1 the list full (or an Intertoto
    /// club not entered), -1 barred (`FUN_00419c9b` and its kin).
    fn add(&mut self, list: List, country: u8, team: i16) -> i32 {
        if self.barred(country, team) {
            return -1;
        }
        let full = if list.at == UEFA.at {
            self.count(UEFA_QUALIFIERS) >= word(&self.block, UEFA_LIMIT)
        } else {
            self.count(list) >= list.most
        };
        if full {
            return 1;
        }
        if (list.at == INTERTOTO_A.at || list.at == INTERTOTO_B.at)
            && self.records.globals.played(i16::from(country))
            && !entered_intertoto(self.records, country, team)
        {
            return 1;
        }
        self.push(list, country, team);
        0
    }

    fn place(&self, country: u8, slot: usize) -> i16 {
        let at = PLACES + usize::from(country) * PLACE_ROW + slot;
        i16::from(self.block.get(at).copied().unwrap_or(0xff) as i8)
    }

    fn set_place(&mut self, country: usize, slot: usize, team: i16) {
        let at = PLACES + country * PLACE_ROW + slot;
        if let Some(b) = self.block.get_mut(at) {
            *b = team as u8;
        }
    }
}

/// Whether a played country's club has entered the Intertoto cup
/// (`FUN_006414ff`).
fn entered_intertoto(records: &Records, country: u8, team: i16) -> bool {
    u16::try_from(team)
        .ok()
        .and_then(|t| records.team(u16::from(country), t))
        .is_some_and(|t| t.get::<u8>(INTERTOTO_ENTERED) != 0)
}

/// The countries by their last five seasons of the ranking, the best first
/// (`FUN_00423dc1`): the rows' second to sixth season summed, an exchange
/// sort; the unranked rows behind, country 0.
fn ranked(records: &Records) -> Vec<u8> {
    let g = &records.globals;
    let mut rows: Vec<(u8, f32)> = (0..RANKED)
        .map(|k| RANKING_AT + k * RANKING_ROW)
        .filter(|&at| g.get::<i16>(at) != 0)
        .map(|at| {
            let sum = (1..6).fold(0.0f32, |s, k| s + f32::from_bits(g.get::<u32>(at + 8 + 4 * k)));
            (g.get::<i16>(at) as u8, sum)
        })
        .collect();
    for i in 0..rows.len() {
        for j in i + 1..rows.len() {
            if rows[i].1 < rows[j].1 {
                rows.swap(i, j);
            }
        }
    }
    let mut countries: Vec<u8> = rows.into_iter().map(|r| r.0).collect();
    countries.resize(RANKED, 0);
    countries
}

/// The season's end enters next season's European cups (`FUN_0041ac69(0)`).
pub fn qualify(records: &mut Records, random: &mut Random) {
    let Some(block) = records.block(ENTRANTS).map(<[u8]>::to_vec) else { return };
    let mut e = Entrants { block, records };
    // The lists start empty, the holders' and the opponent's fields, the
    // Intertoto's tail and every country's places cleared
    // (`FUN_00419956`).
    for (at, len) in CLEARED {
        if let Some(b) = e.block.get_mut(at..at + len) {
            b.fill(0);
        }
    }
    world_opponent(&mut e, random);
    for at in COUNTS {
        put_word(&mut e.block, at, 0);
    }
    if let Some(b) = e.block.get_mut(PLACES..PLACES + PLACE_COUNTRIES * PLACE_ROW) {
        b.fill(0xff);
    }
    places(&mut e, random);
    fill_lists(&mut e);
    let block = e.block;
    if let Some(b) = records.block_mut(ENTRANTS) {
        b.copy_from_slice(&block);
    }
    books(records);
}

/// The lists filled from the countries' places by the ranking, with the
/// holders kept beside them.
fn fill_lists(e: &mut Entrants) {
    let holders = e.records.block(HOLDERS).map(<[u8]>::to_vec).unwrap_or_else(|| vec![0; 12]);
    let main = e.records.globals.main_country() as u8;
    // The holders kept beside the lists: a missing one's country stands
    // in as the main country's (the Champions League's) or 6 (the UEFA
    // cup's), its club 0.
    let mut kept = [holders[0], holders[1], holders[2], holders[3], holders[8], holders[9], holders[10], holders[11]];
    if kept[0] == 0 {
        kept[..4].copy_from_slice(&[main, holders[1], 0, 0]);
    }
    if kept[4] == 0 {
        kept[4..].copy_from_slice(&[UEFA_DEFAULT, holders[9], 0, 0]);
    }
    if let Some(b) = e.block.get_mut(HOLDERS_KEPT..HOLDERS_KEPT + 8) {
        b.copy_from_slice(&kept);
    }
    let rank = ranked(e.records);
    let c = |k: usize| rank.get(k).copied().unwrap_or(0);
    let holder = (holders[0], i16::from_le_bytes([holders[2], holders[3]]));
    if holder.0 != 0 {
        e.add(SEEDS, holder.0, holder.1);
    }
    for k in 0..6 {
        e.add(SEEDS, c(k), e.place(c(k), 1));
    }
    let mut champions = 0;
    while champions < RANKED && e.count(SEEDS) != SEEDS.most {
        e.add(SEEDS, c(champions), e.place(c(champions), 0));
        champions += 1;
    }
    let mut thirds = 0;
    while thirds < 6 {
        e.add(QUALIFIERS, c(thirds), e.place(c(thirds), 2));
        thirds += 1;
    }
    let mut fourths = 0;
    while fourths < 3 {
        e.add(QUALIFIERS, c(fourths), e.place(c(fourths), 3));
        fourths += 1;
    }
    let mut runners_up = 6;
    while runners_up < 15 {
        e.add(QUALIFIERS, c(runners_up), e.place(c(runners_up), 1));
        runners_up += 1;
    }
    while champions < RANKED && e.count(QUALIFIERS) != QUALIFIERS.most {
        e.add(QUALIFIERS, c(champions), e.place(c(champions), 0));
        champions += 1;
    }
    let limit = if e.records.globals.seasons() == 1 { UEFA_DIRECT_FIRST } else { UEFA_DIRECT };
    put_word(&mut e.block, UEFA_LIMIT, limit);
    e.add(UEFA, holders[8], i16::from_le_bytes([holders[10], holders[11]]));
    let mut sixths = 0;
    while sixths < 3 {
        e.add(UEFA, c(sixths), e.place(c(sixths), 5));
        sixths += 1;
    }
    let mut fifths = 0;
    while fifths < 8 {
        e.add(UEFA, c(fifths), e.place(c(fifths), 4));
        fifths += 1;
    }
    for _ in 0..4 {
        e.add(UEFA, c(fourths), e.place(c(fourths), 3));
        fourths += 1;
    }
    for _ in 0..3 {
        e.add(UEFA, c(thirds), e.place(c(thirds), 2));
        thirds += 1;
    }
    let mut cups = 0;
    while cups < RANKED && e.count(UEFA) != limit {
        if e.add(UEFA, c(cups), e.place(c(cups), WINNER)) != 0 {
            e.add(UEFA, c(cups), e.place(c(cups), FINALIST));
        }
        cups += 1;
    }
    while cups < RANKED {
        if e.add(UEFA_QUALIFIERS, c(cups), e.place(c(cups), WINNER)) != 0 {
            e.add(UEFA_QUALIFIERS, c(cups), e.place(c(cups), FINALIST));
        }
        cups += 1;
    }
    while champions < RANKED {
        e.add(UEFA_QUALIFIERS, c(champions), e.place(c(champions), 0));
        champions += 1;
    }
    while runners_up < RANKED {
        e.add(UEFA_QUALIFIERS, c(runners_up), e.place(c(runners_up), 1));
        runners_up += 1;
    }
    while thirds < RANKED && e.add(UEFA_QUALIFIERS, c(thirds), e.place(c(thirds), 2)) != 1 {
        thirds += 1;
    }
    while fourths < RANKED && e.add(UEFA_QUALIFIERS, c(fourths), e.place(c(fourths), 3)) != 1 {
        fourths += 1;
    }
    // The Intertoto takes the places left, country by country, each list
    // from the highest place it has not reached there.
    let (mut sevenths, mut eighths) = (0usize, 0usize);
    let mut k = 0usize;
    let mut steps = 0;
    while e.count(INTERTOTO_B) < INTERTOTO_B_FILLED && steps < 4 * RANKED {
        intertoto_step(e, &c, k, INTERTOTO_B, [&mut fourths, &mut fifths, &mut sixths, &mut sevenths], None);
        k += 1;
        steps += 1;
    }
    k = 0;
    while e.count(INTERTOTO_A) < INTERTOTO_A_FILLED && k < RANKED {
        intertoto_step(e, &c, k, INTERTOTO_A, [&mut fourths, &mut fifths, &mut sixths, &mut sevenths], Some(&mut eighths));
        k += 1;
    }
    k = 0;
    steps = 0;
    while e.count(INTERTOTO_A) < INTERTOTO_A_FILLED && steps < 4 * RANKED {
        intertoto_step(e, &c, k, INTERTOTO_A, [&mut fourths, &mut fifths, &mut sixths, &mut sevenths], Some(&mut eighths));
        k += 1;
        steps += 1;
    }
}

/// One country's turn at an Intertoto list: the fourth if its fourths are
/// not past it, else the fifth, the sixth, the seventh (and for the second
/// list the eighth), the mark of the place taken moving past it.
fn intertoto_step(
    e: &mut Entrants,
    c: &dyn Fn(usize) -> u8,
    k: usize,
    list: List,
    marks: [&mut usize; 4],
    eighths: Option<&mut usize>,
) {
    let [fourths, fifths, sixths, sevenths] = marks;
    let country = c(k);
    if k < *fourths {
        if k < *fifths {
            if k < *sixths {
                match eighths {
                    None => {
                        if *sevenths <= k {
                            e.add(list, country, e.place(country, 6));
                            *sevenths = k + 1;
                        }
                    }
                    Some(eighths) => {
                        if k < *sevenths {
                            if *eighths <= k {
                                e.add(list, country, e.place(country, 7));
                                *eighths = k + 1;
                            }
                        } else {
                            e.add(list, country, e.place(country, 6));
                            *sevenths = k + 1;
                        }
                    }
                }
            } else {
                e.add(list, country, e.place(country, 5));
                *sixths = k + 1;
            }
        } else {
            e.add(list, country, e.place(country, 4));
            *fifths = k + 1;
        }
    } else {
        e.add(list, country, e.place(country, 3));
        *fourths = k + 1;
    }
}

/// The world cup's opponent (`FUN_0063f804`): the champion of its country
/// if that keeps teams, else one of South America's clubs abroad, drawn.
fn world_opponent(e: &mut Entrants, random: &mut Random) {
    let records = e.records;
    let country = records.countries.get::<i16>(WORLD_COUNTRY);
    if country > 0 && WORLD_COUNTRIES_WITH_TEAMS.contains(&country) {
        let n = records.countries.team_count(country as usize).max(0) as u16;
        if let Some(t) = (0..n).find(|&t| records.team(country as u16, t).is_some_and(|t| t.get::<u16>(0x110) & TITLE != 0)) {
            e.block[WORLD_OPPONENT] = country as u8;
            put_word(&mut e.block, WORLD_OPPONENT + 2, t as i16);
            return;
        }
    }
    let clubs: Vec<(u8, u8)> = (0..ABROAD)
        .filter_map(|k| records.block(&format!("0x1a28dd8[{k}]")))
        .map(|b| (b.get(ABROAD_COUNTRY).copied().unwrap_or(0), b.get(ABROAD_COUNTRY + 1).copied().unwrap_or(0)))
        .filter(|(c, _)| SOUTH_AMERICA.contains(c))
        .collect();
    if clubs.is_empty() {
        e.block[WORLD_OPPONENT] = WORLD_DEFAULT;
        put_word(&mut e.block, WORLD_OPPONENT + 2, 0);
    } else {
        let (country, team) = clubs[random.below(clubs.len() as u32) as usize];
        e.block[WORLD_OPPONENT] = country;
        put_word(&mut e.block, WORLD_OPPONENT + 2, i16::from(team));
    }
}

/// Every country's places (`FUN_00422b88(0)`).
fn places(e: &mut Entrants, random: &mut Random) {
    for country in 1..PLACE_COUNTRIES {
        let records = e.records;
        if records.countries.kind(country) != 2 || !records.countries.has_teams(country) {
            continue;
        }
        let table = crate::statistics::other_table(records, country as u16);
        if table.len() < 8 {
            continue;
        }
        for (slot, row) in table.iter().enumerate() {
            e.set_place(country, slot, row.0);
        }
        let drawn = random.below(6) as usize;
        e.set_place(country, WINNER, table[drawn].0);
        e.set_place(country, FINALIST, table[6].0);
    }
    for country in crate::opponent::played_nations(e.records) {
        played_places(e, country);
    }
}

/// A played country's places (`FUN_00423559(country, 0)`): its closed first
/// league's eight from its archive, its cup's winner and finalist by their
/// flags, the gaps from its first league's list.
fn played_places(e: &mut Entrants, country: u16) {
    let records = e.records;
    let c = usize::from(country);
    let slot = records.countries.slot(c);
    let Some(block) = records.block(&format!("0x19f7fd0[{slot}]")) else { return };
    let archived: Vec<u8> = block.get(COUNTRY_PLACES..COUNTRY_PLACES + 8).map_or_else(|| vec![0xff; 8], <[u8]>::to_vec);
    let clubs = word(block, COUNTRY_CLUBS).max(0) as u16;
    for (k, &t) in archived.iter().enumerate() {
        e.set_place(c, k, i16::from(t as i8));
    }
    for t in 0..clubs {
        let flags = records.team(country, t).map_or(0, |t| t.get::<u16>(0x110));
        if flags & CUP_WINNER != 0 {
            e.set_place(c, WINNER, t as i16);
        }
        if flags & CUP_FINALIST != 0 {
            e.set_place(c, FINALIST, t as i16);
        }
    }
    let list = crate::opponent::League::of(records, country, 0)
        .map(|l| (0..l.teams()).map(|p| l.team(p)).collect::<Vec<i16>>())
        .unwrap_or_default();
    let place = |e: &Entrants, k: usize| e.place(country as u8, k);
    if (0..6).any(|k| place(e, k) == -1) {
        for &t in &list {
            if (0..6).any(|k| place(e, k) == t) {
                continue;
            }
            if let Some(k) = (0..6).find(|&k| place(e, k) == -1) {
                e.set_place(c, k, t);
            }
        }
    }
    if place(e, WINNER) == -1 || place(e, FINALIST) == -1 {
        for &t in &list {
            if place(e, WINNER) == t || place(e, FINALIST) == t {
                continue;
            }
            if place(e, WINNER) == -1 {
                e.set_place(c, WINNER, t);
            } else if place(e, FINALIST) == -1 {
                e.set_place(c, FINALIST, t);
            }
        }
    }
}

/// Each played country's books of its European clubs (`FUN_006b3ca4`,
/// `FUN_006b3b92`): every club in the Champions League's lists, then those
/// of the UEFA cup's not booked already, and the Intertoto's.
fn books(records: &mut Records) {
    let Some(entrants) = records.block(ENTRANTS).map(<[u8]>::to_vec) else { return };
    let listed = |list: List, country: u8, team: i16| {
        (0..word(&entrants, list.count).max(0) as usize)
            .any(|k| entrants[list.at + 4 * k] == country && word(&entrants, list.at + 4 * k + 2) == team)
    };
    for country in crate::opponent::played_nations(records) {
        let slot = records.countries.slot(usize::from(country));
        let Some(block) = records.block_mut(&format!("0x19f7fd0[{slot}]")) else { continue };
        for at in BOOK_COUNTS {
            put_word(block, at, 0);
        }
        let clubs = word(block, COUNTRY_CLUBS).max(0);
        let c = country as u8;
        let booked = |b: &[u8], count: usize, at: usize, t: i16| (0..word(b, count).max(0) as usize).any(|k| word(b, at + 2 * k) == t);
        for t in 0..clubs {
            if (listed(SEEDS, c, t) || listed(QUALIFIERS, c, t))
                && !booked(block, BOOK_COUNTS[0], BOOK_CHAMPIONS, t)
            {
                let n = word(block, BOOK_COUNTS[0]);
                put_word(block, BOOK_CHAMPIONS + 2 * n as usize, t);
                put_word(block, BOOK_COUNTS[0], n + 1);
            }
            if (listed(UEFA, c, t) || listed(UEFA_QUALIFIERS, c, t))
                && !booked(block, BOOK_COUNTS[0], BOOK_CHAMPIONS, t)
                && !booked(block, BOOK_COUNTS[2], BOOK_UEFA, t)
            {
                let n = word(block, BOOK_COUNTS[2]);
                put_word(block, BOOK_UEFA + 2 * n as usize, t);
                put_word(block, BOOK_COUNTS[2], n + 1);
            }
            if listed(INTERTOTO_A, c, t) || listed(INTERTOTO_B, c, t) {
                let n = word(block, BOOK_COUNTS[3]);
                put_word(block, BOOK_INTERTOTO + 2 * n as usize, t);
                put_word(block, BOOK_COUNTS[3], n + 1);
            }
        }
    }
}

/// The Champions League's object (`0x8e91e0`, from +0x20): its pairings
/// and groups cleared in five stretches, its round's fields and its
/// rounds' first pairings, the main country's first league's points for a
/// win at home and away and the word behind them.
const CHAMPIONS: &str = "0x8e91e0";
const CHAMPIONS_CLEARED: [(usize, usize); 5] = [(0, 0x922), (0x922, 0x180), (0xaa2, 0xc0), (0xb62, 0x280), (0xde2, 0x140)];
const CHAMPIONS_FIELDS: [(usize, u32); 8] =
    [(0xf28, 0x10), (0xf2c, 0x70), (0xf30, 0xa0), (0xf34, 0xa4), (0xf38, 0xa6), (0xf24, 1), (0xf3c, 4), (0xf40, 4)];
const CHAMPIONS_POINTS: usize = 0xf44;
const CHAMPIONS_QUALIFIER_TIES: usize = 16;
const PAIRING: usize = 0xe;
/// The UEFA cup's object (`0x8e8940`): its rounds' first pairings.
const UEFA_CUP: &str = "0x8e8940";
const UEFA_STARTS: [u32; 8] = [0, 50, 98, 122, 138, 146, 150, 152];
/// Each country's count of clubs in the two cups (`DAT_00b5cca6`).
const COUNTRY_ENTRIES: usize = 0xb5cca6;
/// The Intertoto's first list paired in twelve ties (the entrants' +0x4ea).
const INTERTOTO_TIES: usize = 0x4ca;
/// The season flags the cups give a club (team +0x110): in Europe, and
/// the Champions League's seed, the UEFA cup, the Intertoto.
const IN_EUROPE: u16 = 0x4000;
const SEEDED: u16 = 0x40;
const IN_UEFA: u16 = 0x10;
const IN_INTERTOTO: u16 = 0x20;

/// The new season's European cups (`FUN_0041bb6c`): the Champions League
/// cleared and set for its qualifiers (`FUN_0041bd39`), the UEFA cup
/// cleared with its rounds' starts (`FUN_0041f9b1`), every entrant's club flagged and its country
/// counted (`FUN_0041a3ef`), from the second season the Intertoto's first
/// list drawn into twelve ties, and the Champions League's qualifiers
/// paired: each of the first sixteen away to one of the other sixteen in
/// a drawn order (`FUN_0041bef4`). (The favourites' lists, `FUN_0077f5d9`
/// at 0x1c67758, live outside the save and are not built.)
pub fn open_season(records: &mut Records, random: &mut Random) {
    let main = u16::try_from(records.globals.main_country()).unwrap_or(0);
    let rules: Option<[u8; 8]> = crate::opponent::League::block_name(records, main, 0)
        .and_then(|n| records.block(&n))
        .and_then(|b| b.get(crate::opponent::LEAGUE_FIELDS + 8..crate::opponent::LEAGUE_FIELDS + 0x10))
        .map(|w| w.try_into().unwrap());
    if let Some(b) = records.block_mut(CHAMPIONS) {
        // The whole object first (`FUN_007cff4b`), then its stretches.
        b.fill(0);
        for (at, len) in CHAMPIONS_CLEARED {
            if let Some(r) = b.get_mut(at..at + len) {
                r.fill(0);
            }
        }
        for (at, v) in CHAMPIONS_FIELDS {
            if let Some(r) = b.get_mut(at..at + 4) {
                r.copy_from_slice(&v.to_le_bytes());
            }
        }
        if let (Some(r), Some(rules)) = (b.get_mut(CHAMPIONS_POINTS..CHAMPIONS_POINTS + 8), rules) {
            r.copy_from_slice(&rules);
        }
    }
    if let Some(b) = records.block_mut(UEFA_CUP) {
        b.fill(0);
        for (k, v) in UEFA_STARTS.iter().enumerate() {
            if let Some(r) = b.get_mut(4 * k..4 * k + 4) {
                r.copy_from_slice(&v.to_le_bytes());
            }
        }
    }
    for k in 0..PLACE_COUNTRIES {
        records.globals.put::<u8>(COUNTRY_ENTRIES + k, 0);
    }
    let Some(entrants) = records.block(ENTRANTS).map(<[u8]>::to_vec) else { return };
    let entries = |list: List| -> Vec<(u8, i16)> {
        (0..word(&entrants, list.count).max(0) as usize)
            .map(|k| (entrants[list.at + 4 * k], word(&entrants, list.at + 4 * k + 2)))
            .collect()
    };
    for (list, flags, counted) in [
        (SEEDS, IN_EUROPE | SEEDED, true),
        (QUALIFIERS, IN_EUROPE, true),
        (UEFA, IN_EUROPE | IN_UEFA, true),
        (UEFA_QUALIFIERS, IN_EUROPE | IN_UEFA, true),
        (INTERTOTO_A, IN_EUROPE | IN_INTERTOTO, false),
        (INTERTOTO_B, IN_EUROPE | IN_INTERTOTO, false),
    ] {
        for (country, team) in entries(list) {
            flag_club(records, country, team, flags);
            if counted {
                let at = COUNTRY_ENTRIES + usize::from(country);
                records.globals.put::<u8>(at, records.globals.get::<u8>(at).wrapping_add(1));
            }
        }
    }
    let mut entrants = entrants;
    if records.globals.seasons() > 1 {
        let order = crate::cup_draw::order(INTERTOTO_A_FILLED as usize, random);
        for k in (0..order.len()).step_by(2) {
            let at = INTERTOTO_TIES + (k / 2) * PAIRING;
            let (a, b) = (INTERTOTO_A.at + 4 * order[k], INTERTOTO_A.at + 4 * order[k + 1]);
            let (first, second) = (entrants[a..a + 4].to_vec(), entrants[b..b + 4].to_vec());
            entrants[at..at + 4].copy_from_slice(&first);
            entrants[at + 4..at + 8].copy_from_slice(&second);
        }
        if let Some(b) = records.block_mut(ENTRANTS) {
            b.copy_from_slice(&entrants);
        }
    }
    let order = crate::cup_draw::order(CHAMPIONS_QUALIFIER_TIES, random);
    if let Some(b) = records.block_mut(CHAMPIONS) {
        for (k, &o) in order.iter().enumerate() {
            let at = k * PAIRING;
            let home = QUALIFIERS.at + 4 * (CHAMPIONS_QUALIFIER_TIES + o);
            let away = QUALIFIERS.at + 4 * k;
            b[at..at + 4].copy_from_slice(&entrants[home..home + 4]);
            b[at + 4..at + 8].copy_from_slice(&entrants[away..away + 4]);
        }
    }
    records.globals.set_europe_stage(0);
}

/// A played country's club takes a cup's flags (`FUN_0063e0d1`).
fn flag_club(records: &mut Records, country: u8, team: i16, flags: u16) {
    if team == 200 || records.countries.kind(usize::from(country)) != 1 {
        return;
    }
    let Ok(number) = u16::try_from(team) else { return };
    if let Some(t) = records.team_mut(u16::from(country), number) {
        t.put::<u16>(0x110, t.get::<u16>(0x110) | flags);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair() -> (Records, Records, Records) {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        (load("SeasonEnd364.an3"), load("NewSeason000.an3"), load("SeasonEnd364.an3"))
    }

    /// Over the original's pair: the season end's steps before it run by the
    /// replica, every country's places stand as NewSeason000 keeps them but
    /// the drawn cup winners of the countries kept in short; with the
    /// original's own places the lists come out byte for byte as its —
    /// every club, its list and its order, the holders kept beside them —
    /// and so do the played countries' books.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_season_end_enters_the_european_cups_as_the_original() {
        let (mut ours, theirs, _) = pair();
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let data = crate::data::Data::load(&dir).unwrap();
        let mut random = Random::new(3);
        let _ = crate::promotion::draw(&mut ours, &mut random);
        crate::promotion::count(&mut ours);
        crate::statistics::archive_leagues(&mut ours);
        crate::promotion::flag(&mut ours, &data.rules.forecast.tickets);
        let staged = ours.block(ENTRANTS).unwrap().to_vec();
        qualify(&mut ours, &mut random);
        let (x, y) = (ours.block(ENTRANTS).unwrap(), theirs.block(ENTRANTS).unwrap());
        for country in 0..PLACE_COUNTRIES {
            let at = PLACES + country * PLACE_ROW;
            for slot in 0..PLACE_ROW {
                if slot == WINNER && ours.countries.kind(country) == 2 {
                    continue;
                }
                assert_eq!(x[at + slot], y[at + slot], "country {country}'s place {slot}");
            }
        }
        // The lists from the original's places.
        let mut block = staged;
        block[PLACES..PLACES + PLACE_COUNTRIES * PLACE_ROW]
            .copy_from_slice(&y[PLACES..PLACES + PLACE_COUNTRIES * PLACE_ROW]);
        for at in COUNTS {
            put_word(&mut block, at, 0);
        }
        let mut e = Entrants { block, records: &ours };
        fill_lists(&mut e);
        for list in LISTS {
            let (mine, theirs_list): (Vec<_>, Vec<_>) = (e.entries(list).collect(), {
                let t = Entrants { block: y.to_vec(), records: &theirs };
                t.entries(list).collect()
            });
            assert_eq!(mine, theirs_list, "the list at {:#x}", list.at);
        }
        assert_eq!(e.block[UEFA_LIMIT..UEFA_LIMIT + 2], y[UEFA_LIMIT..UEFA_LIMIT + 2]);
        assert_eq!(e.block[HOLDERS_KEPT..HOLDERS_KEPT + 8], y[HOLDERS_KEPT..HOLDERS_KEPT + 8]);
        // The books from the original's lists.
        let lists = y.to_vec();
        ours.block_mut(ENTRANTS).unwrap().copy_from_slice(&lists);
        books(&mut ours);
        for country in crate::opponent::played_nations(&ours) {
            let slot = ours.countries.slot(usize::from(country));
            let name = format!("0x19f7fd0[{slot}]");
            let (b, t) = (ours.block(&name).unwrap(), theirs.block(&name).unwrap());
            for (count, list) in [(0, BOOK_CHAMPIONS), (2, BOOK_UEFA), (3, BOOK_INTERTOTO)] {
                let n = word(t, BOOK_COUNTS[count]);
                assert_eq!(word(b, BOOK_COUNTS[count]), n, "country {country}'s book {count}");
                for k in 0..n.max(0) as usize {
                    assert_eq!(word(b, list + 2 * k), word(t, list + 2 * k), "country {country}'s book {count}");
                }
            }
        }
    }

    /// The new season's cups over the original's pair, opened by the
    /// replica from the original's lists: every played club's European
    /// flags, every country's count of entrants, the UEFA cup's object and
    /// the Champions League's as NewSeason000 keeps them — its
    /// qualifiers' ties with the first sixteen away, their opponents the
    /// other sixteen in a drawn order.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_new_season_opens_the_european_cups_as_the_original() {
        let (mut ours, theirs, _) = pair();
        let lists = theirs.block(ENTRANTS).unwrap().to_vec();
        ours.block_mut(ENTRANTS).unwrap().copy_from_slice(&lists);
        let played = crate::opponent::played_nations(&ours);
        for &c in &played {
            for t in 0..ours.countries.team_count(usize::from(c)).max(0) as u16 {
                let team = ours.team_mut(c, t).unwrap();
                team.put::<u16>(0x110, team.get::<u16>(0x110) & !(IN_EUROPE | SEEDED | IN_UEFA | IN_INTERTOTO));
            }
        }
        ours.globals.set_seasons(2);
        open_season(&mut ours, &mut Random::new(3));
        let europe = IN_EUROPE | SEEDED | IN_UEFA | IN_INTERTOTO;
        for &c in &played {
            for t in 0..ours.countries.team_count(usize::from(c)).max(0) as u16 {
                let flags = |r: &Records| r.team(c, t).unwrap().get::<u16>(0x110) & europe;
                assert_eq!(flags(&ours), flags(&theirs), "club {c}/{t}");
            }
        }
        for k in 0..PLACE_COUNTRIES {
            assert_eq!(ours.globals.get::<u8>(COUNTRY_ENTRIES + k), theirs.globals.get::<u8>(COUNTRY_ENTRIES + k), "country {k}");
        }
        assert_eq!(ours.block(UEFA_CUP), theirs.block(UEFA_CUP), "the UEFA cup's object");
        let (x, y) = (ours.block(CHAMPIONS).unwrap(), theirs.block(CHAMPIONS).unwrap());
        let home = |k: usize| k < CHAMPIONS_QUALIFIER_TIES * PAIRING && k % PAIRING < 4;
        assert!((0..x.len()).filter(|&k| !home(k)).all(|k| x[k] == y[k]), "the Champions League's object");
        let homes = |b: &[u8]| {
            let mut h: Vec<Vec<u8>> = (0..CHAMPIONS_QUALIFIER_TIES).map(|k| b[k * PAIRING..k * PAIRING + 4].to_vec()).collect();
            h.sort();
            h
        };
        assert_eq!(homes(x), homes(y), "the qualifiers' hosts");
    }
}
