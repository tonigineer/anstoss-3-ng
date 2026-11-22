//! A new game's start settings (`CStartvorbereitung`, dialog 6) acting on
//! its records, as `FUN_007875b1` takes them after the dialog: the order
//! the managers take their turns in, the fixture lists, the dismissals,
//! the human clubs' start, the initial conditions and the preparation.
//!
//! The calculation that follows (the cups, their draws, friendlies, the
//! clubs' goals; game cycle case 99/0, `FUN_006da3a9`) is not here.

use crate::finances::CLUB_TO_TEAM;
use crate::land_managers::{draw_order, reorder};
use crate::random::Random;
use crate::records::Records;
use crate::stadium;

/// The dialog's choices, at its defaults (`CStartvorbereitung`'s
/// `OnInitDialog`: +0x23c8 0, +0x23cc 1, +0x23c4 1, +0x23be 0, +0x23d0
/// 0, +0x23d4 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// +0x23c8: the managers take their turns as listed, not drawn.
    pub order_as_listed: bool,
    /// +0x23cc: the fixture lists as the league files have them, not
    /// shuffled (`FUN_0071c67a`).
    pub fixtures_as_listed: bool,
    /// +0x23c4: managers can be dismissed (`DAT_00b5d158`).
    pub dismissals: bool,
    /// +0x23be on, +0x23c0 which: the human clubs start with a board and
    /// a stadium of three kinds, 0 the biggest (`FUN_0071c759`).
    pub start: Option<u8>,
    /// +0x23d0: the human clubs' country tops the European ranking and
    /// they head its first league's list (`FUN_0071ca31`).
    pub initial_conditions: bool,
    /// +0x23d4: the preparation, 0 normal, 1 fast, 2 competitive games
    /// only.
    pub preparation: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            order_as_listed: false,
            fixtures_as_listed: true,
            dismissals: true,
            start: None,
            initial_conditions: false,
            preparation: 0,
        }
    }
}

/// The league mode from which a new game plays without leagues of its
/// own (`DAT_00b5cbc6` ≥ 9): dismissals always on, nothing else taken.
const NO_LEAGUE_MODE: i16 = 9;
/// `DAT_00b5d158`, `DAT_00b5d152`, `DAT_00b5d19a` (bytes, a word).
const DISMISSALS: usize = 0xb5d158;
const PREPARATIONS: [u8; 3] = [5, 7, 12];
const QUIET_OPTION: usize = 0xb5d19a;
/// The table of the human clubs (`DAT_00b5cbe2`, eight bytes each: the
/// country's byte, the club's number at +2) and their count.
const HUMAN_CLUBS: usize = 0xb5cbe2;
const HUMAN_CLUBS_COUNT: usize = 0xb5cc02;
/// The leagues' objects (`0x1a09d48[li]`, the save holds each from
/// +0x20): teams +0x50, matchdays +0x52, the teams' list +0x72 (their
/// numbers in the country), the fixture list +0x55a (a row of 0x18 a
/// matchday, each slot's place in the league).
const LEAGUE_AT: usize = 0x1a09d48;
const LEAGUE_BASE: usize = 0x20;
const LEAGUE_TEAMS: usize = 0x50;
const LEAGUE_DAYS: usize = 0x52;
const LEAGUE_LIST: usize = 0x72;
const FIXTURES: usize = 0x55a;
const FIXTURE_ROW: usize = 0x18;
/// The European ranking (`DAT_00b5d1a8`): 0x25 rows of 0x24, the country
/// (+0), its total (+4) and six seasons' points (+8).
const RANKING_AT: usize = 0xb5d1a8;
const RANKING_ROWS: usize = 0x25;
const RANKING_ROW: usize = 0x24;
const SEASONS: usize = 6;
/// The board type (team +0x123, the club object's +0xf5) and the cash
/// (club +0x210) of a human club that starts with a stadium.
const BOARDS: [u8; 3] = [5, 3, 0];
const CASH: usize = 0x210 + CLUB_TO_TEAM;
/// The least a club starting with the biggest stadium holds
/// (`DAT_00a3b314`).
const LEAST_CASH: i32 = 2_000_000;
/// The stands' seats and standing places by kind (`0x8a7c88`, words):
/// seats of the biggest at +0, its standing at +8, the middle one's at
/// +0x10 and +0x18, the smallest's at +0x20 and +0x28 — of which the
/// game takes twelve, not four, running on over what follows.
const STANDS_TABLE: [u16; 32] = [
    12000, 12000, 0, 0, 0, 0, 18000, 18000, 9000, 9000, 0, 0, 0, 0, 13500, 13500, 4000, 4000, 0, 0, 0, 0, 6000,
    6000, 48, 54, 48, 56, 56, 0, 21325, 24391,
];
/// The stadium's stands (sub-record +0x40 VIP, +0x48 standing, +0x50
/// seats, +0x60 the roofs' bits).
const VIP: usize = 0x40;
const STANDING: usize = 0x48;
const SEATS: usize = 0x50;
const STAND_BITS: usize = 0x60;

/// The start settings taken (`FUN_007875b1` after dialog 6).
pub fn apply(records: &mut Records, settings: &Settings, random: &mut Random) {
    let humans = usize::from(records.globals.humans());
    if humans > 1 {
        let order = if settings.order_as_listed { (1..=humans).collect() } else { draw_order(humans, random) };
        reorder(records, &order);
    }
    if !settings.fixtures_as_listed {
        shuffle_fixtures(records, random);
    }
    if records.globals.mode() < NO_LEAGUE_MODE {
        records.globals.put::<u8>(DISMISSALS, u8::from(settings.dismissals));
        if let Some(kind) = settings.start {
            start_clubs(records, kind);
        }
        if settings.initial_conditions {
            initial_conditions(records);
        }
    } else {
        records.globals.put::<u8>(DISMISSALS, 1);
    }
    let preparation = PREPARATIONS[usize::from(settings.preparation).min(PREPARATIONS.len() - 1)];
    records.globals.set_preparation(preparation);
    records.globals.put::<i16>(QUIET_OPTION, 0);
}

fn league_name(li: usize) -> String {
    format!("{LEAGUE_AT:#x}[{li}]")
}

fn word(block: &[u8], at: usize) -> i16 {
    i16::from_le_bytes([block[at - LEAGUE_BASE], block[at - LEAGUE_BASE + 1]])
}

/// `FUN_0071c67a`: each league's teams take new places in a shuffled
/// order (`FUN_00675630`), the fixture list following them — the leagues
/// from the first up to the count of further countries (`DAT_00b5cbae`),
/// as the game counts them, not each country's leagues.
fn shuffle_fixtures(records: &mut Records, random: &mut Random) {
    let last = usize::try_from(records.globals.more_count()).unwrap_or(0);
    for li in 0..=last {
        let Some(block) = records.block_mut(&league_name(li)) else { continue };
        let teams = usize::try_from(word(block, LEAGUE_TEAMS)).unwrap_or(0);
        let days = usize::try_from(word(block, LEAGUE_DAYS)).unwrap_or(0);
        let place = random.shuffle(teams);
        for day in 0..days {
            for slot in 0..teams {
                let at = FIXTURES - LEAGUE_BASE + slot + day * FIXTURE_ROW;
                let Some(&old) = block.get(at) else { continue };
                if let Some(&new) = place.get(usize::from(old)) {
                    block[at] = new as u8;
                }
            }
        }
    }
}

/// `FUN_0071c759`: every human at a club, slot by slot, has its board
/// set to the kind's (the club object's +0xf5) and its stadium played in
/// (`FUN_00638335`, `FUN_0063865c`) rebuilt: stand by stand the VIP boxes
/// cleared, standing and seats from the table, the roof on — off for
/// the smallest, whose twelve rounds write the table on over the stands'
/// age, building weeks, bits and comfort. The biggest's club holds at
/// least [`LEAST_CASH`].
fn start_clubs(records: &mut Records, kind: u8) {
    let Some(&board) = BOARDS.get(usize::from(kind)) else { return };
    let humans = usize::from(records.globals.humans());
    for slot in 0..humans.min(records.managers.len()) {
        let (country, number) = (records.managers[slot].country(), records.managers[slot].team());
        if country == 0 {
            continue;
        }
        let Some(mut subs) = stadium::subs(records, country as u8, number) else { continue };
        let Some(team) = records.team_mut(country, number) else { continue };
        team.set_board(board);
        let sub = &mut subs[0].0;
        let put = |sub: &mut Vec<u8>, at: usize, v: u16| sub[at..at + 2].copy_from_slice(&v.to_le_bytes());
        let (seats, standing, rounds, roof) = match kind {
            0 => (0, 4, 4, true),
            1 => (8, 12, 4, true),
            _ => (16, 20, 12, false),
        };
        for i in 0..rounds {
            put(sub, STANDING + 2 * i, STANDS_TABLE[standing + i]);
            put(sub, SEATS + 2 * i, STANDS_TABLE[seats + i]);
            put(sub, VIP + 2 * i, 0);
            // FUN_00718196: a bit of the four stands' only.
            if i < 4 {
                let bits = u16::from_le_bytes([sub[STAND_BITS], sub[STAND_BITS + 1]]);
                let bit = 1 << i;
                put(sub, STAND_BITS, if roof { bits | bit } else { bits & !bit });
            }
        }
        if kind == 0 {
            let cash = team.get::<i64>(CASH);
            if cash < i64::from(LEAST_CASH) {
                team.put::<i64>(CASH, i64::from(LEAST_CASH));
            }
        }
        stadium::store(records, country as u8, number, &subs);
    }
}

/// `FUN_0071ca31`: the first human club's country takes the ranking's
/// first row's points, one more each season, and the ranking is
/// reckoned again (`FUN_00423dc1`); in that country's first league the
/// human clubs standing from the third place on move up, one by one the
/// highest first, to the third, fourth … place, the teams between
/// moving down.
fn initial_conditions(records: &mut Records) {
    let count = usize::try_from(records.globals.get::<i16>(HUMAN_CLUBS_COUNT)).unwrap_or(0).min(4);
    if count == 0 {
        return;
    }
    let list: [u8; 32] = records.globals.get(HUMAN_CLUBS);
    let country = list[0];
    let clubs: Vec<i16> = (0..count).map(|k| i16::from_le_bytes([list[k * 8 + 2], list[k * 8 + 3]])).collect();
    let mut ranking: Vec<u8> = records.globals.get::<[u8; RANKING_ROWS * RANKING_ROW]>(RANKING_AT).to_vec();
    let f32_at = |r: &[u8], at: usize| f32::from_le_bytes(r[at..at + 4].try_into().unwrap());
    for row in 0..RANKING_ROWS {
        if i16::from_le_bytes([ranking[row * RANKING_ROW], ranking[row * RANKING_ROW + 1]]) == i16::from(country) {
            if row != 0 {
                for s in 0..SEASONS {
                    let points = f32_at(&ranking, 8 + 4 * s) + 1.0;
                    let at = row * RANKING_ROW + 8 + 4 * s;
                    ranking[at..at + 4].copy_from_slice(&points.to_le_bytes());
                }
            }
            break;
        }
    }
    reckon_ranking(&mut ranking);
    records.globals.put::<[u8; RANKING_ROWS * RANKING_ROW]>(RANKING_AT, ranking.try_into().unwrap());

    let li = usize::try_from(records.countries.first_league(usize::from(country))).unwrap_or(0);
    let Some(block) = records.block_mut(&league_name(li)) else { return };
    let teams = usize::try_from(word(block, LEAGUE_TEAMS)).unwrap_or(0).min(24);
    let at = |p: usize| LEAGUE_LIST - LEAGUE_BASE + 2 * p;
    let mut list: Vec<i16> = (0..teams).map(|p| i16::from_le_bytes([block[at(p)], block[at(p) + 1]])).collect();
    const NOWHERE: usize = 99;
    let mut place = [NOWHERE; 4];
    for p in 2..teams {
        for (k, &club) in clubs.iter().enumerate() {
            if list[p] == club {
                place[k] = p;
            }
        }
    }
    let mut next = 2;
    for k in 0..count {
        let mut first = k;
        for j in k + 1..count {
            if place[j] < place[first] {
                first = j;
            }
        }
        if place[first] < NOWHERE {
            for p in (next + 1..=place[first]).rev() {
                list[p] = list[p - 1];
            }
            list[next] = clubs[first];
            place[first] = NOWHERE;
            next += 1;
        }
    }
    for (p, team) in list.iter().enumerate() {
        block[at(p)..at(p) + 2].copy_from_slice(&team.to_le_bytes());
    }
}

/// `FUN_00423dc1` on the ranking itself: each row with a country, in
/// turn, writes its country and the sum of its last five seasons (in
/// floats) into the next row from the first; those rows sorted by it,
/// highest first, a row swapping with each later one above it.
fn reckon_ranking(ranking: &mut [u8]) {
    let f32_at = |r: &[u8], at: usize| f32::from_le_bytes(r[at..at + 4].try_into().unwrap());
    let mut rows = 0;
    for row in 0..RANKING_ROWS {
        let at = row * RANKING_ROW;
        let country = [ranking[at], ranking[at + 1]];
        if i16::from_le_bytes(country) == 0 {
            continue;
        }
        let mut total = 0.0f32;
        for s in 1..SEASONS {
            total += f32_at(ranking, at + 8 + 4 * s);
        }
        let to = rows * RANKING_ROW;
        ranking[to..to + 2].copy_from_slice(&country);
        ranking[to + 4..to + 8].copy_from_slice(&total.to_le_bytes());
        rows += 1;
    }
    for i in 0..rows {
        for j in i + 1..rows {
            if f32_at(ranking, i * RANKING_ROW + 4) < f32_at(ranking, j * RANKING_ROW + 4) {
                for b in 0..RANKING_ROW {
                    ranking.swap(i * RANKING_ROW + b, j * RANKING_ROW + b);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn league_word(records: &Records, li: usize, at: usize) -> i16 {
        word(records.block(&league_name(li)).unwrap(), at)
    }

    fn put_league_word(records: &mut Records, li: usize, at: usize, v: i16) {
        let block = records.block_mut(&league_name(li)).unwrap();
        block[at - LEAGUE_BASE..at - LEAGUE_BASE + 2].copy_from_slice(&v.to_le_bytes());
    }

    /// Shuffled, a fixture list keeps every pairing, its teams under new
    /// places, one place for each; leagues past the count stay.
    #[test]
    fn a_shuffled_fixture_list_keeps_its_pairings() {
        let mut records = Records::blank();
        let teams = 6usize;
        let days = 2 * teams - 2;
        for li in 0..2 {
            put_league_word(&mut records, li, LEAGUE_TEAMS, teams as i16);
            put_league_word(&mut records, li, LEAGUE_DAYS, days as i16);
            let block = records.block_mut(&league_name(li)).unwrap();
            for day in 0..days {
                for slot in 0..teams {
                    block[FIXTURES - LEAGUE_BASE + slot + day * FIXTURE_ROW] = ((slot + day) % teams) as u8;
                }
            }
        }
        let before = |r: &Records, li: usize| r.block(&league_name(li)).unwrap().to_vec();
        let (first, second) = (before(&records, 0), before(&records, 1));
        let mut random = Random::new(3);
        let place = random.clone().shuffle(teams);
        let settings = Settings { fixtures_as_listed: false, ..Settings::default() };
        apply(&mut records, &settings, &mut random);
        let after = before(&records, 0);
        for day in 0..days {
            for slot in 0..teams {
                let at = FIXTURES - LEAGUE_BASE + slot + day * FIXTURE_ROW;
                assert_eq!(i16::from(after[at]), place[usize::from(first[at])]);
            }
        }
        assert_ne!(after, first, "the order drawn moves some");
        assert_eq!(before(&records, 1), second, "no further country: the second league stays");
        assert_eq!(records.globals.preparation(), 5);
        assert_eq!(records.globals.get::<u8>(DISMISSALS), 1);
    }

    /// The initial conditions: the country second in the ranking takes the
    /// first's points plus one and the top; the human club seventh in its
    /// league moves to the third place, the third to seventh moving down.
    #[test]
    fn the_initial_conditions_put_the_human_club_up() {
        let mut records = Records::blank();
        let country = 14u8;
        let li = usize::try_from(records.countries.first_league(usize::from(country))).unwrap();
        put_league_word(&mut records, li, LEAGUE_TEAMS, 10);
        for p in 0..10 {
            put_league_word(&mut records, li, LEAGUE_LIST + 2 * p, p as i16);
        }
        let mut list = [0u8; 32];
        list[0] = country;
        list[2] = 6;
        records.globals.put(HUMAN_CLUBS, list);
        records.globals.put::<i16>(HUMAN_CLUBS_COUNT, 1);
        let mut ranking = [0u8; RANKING_ROWS * RANKING_ROW];
        for (row, (c, points)) in [(20i16, 10.0f32), (i16::from(country), 5.0), (28, 1.0)].into_iter().enumerate() {
            ranking[row * RANKING_ROW..row * RANKING_ROW + 2].copy_from_slice(&c.to_le_bytes());
            for s in 0..SEASONS {
                let at = row * RANKING_ROW + 8 + 4 * s;
                ranking[at..at + 4].copy_from_slice(&points.to_le_bytes());
            }
        }
        records.globals.put(RANKING_AT, ranking);
        apply(&mut records, &Settings { initial_conditions: true, ..Settings::default() }, &mut Random::new(1));
        let list: Vec<i16> = (0..10).map(|p| league_word(&records, li, LEAGUE_LIST + 2 * p)).collect();
        assert_eq!(list, [0, 1, 6, 2, 3, 4, 5, 7, 8, 9]);
        let ranking: [u8; RANKING_ROWS * RANKING_ROW] = records.globals.get(RANKING_AT);
        let row = |r: usize| {
            let at = r * RANKING_ROW;
            (i16::from_le_bytes([ranking[at], ranking[at + 1]]), f32::from_le_bytes(ranking[at + 4..at + 8].try_into().unwrap()))
        };
        assert_eq!([row(0), row(1), row(2)], [(i16::from(country), 55.0), (20, 50.0), (28, 5.0)]);
    }

    /// A kept save's human club started with the biggest stadium: the board
    /// of kind 5, the stands from the table, roofed, no boxes, two million
    /// at least; the smallest's twelve rounds write over the bits.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_human_club_starts_with_the_stadium_chosen() {
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let save = a3_files::an3::Save::read(&keeps.join("A.an3")).unwrap();
        let mut records = Records::from_save(save.clone()).unwrap();
        let (country, number) = (records.managers[0].country(), records.managers[0].team());
        assert_ne!(country, 0, "A: a human at a club");
        records.team_mut(country, number).unwrap().put::<i64>(CASH, 5);
        start_clubs(&mut records, 0);
        let team = records.team(country, number).unwrap();
        assert_eq!((team.board(), team.get::<i64>(CASH)), (5, i64::from(LEAST_CASH)));
        let sub = &stadium::subs(&records, country as u8, number).unwrap()[0];
        for s in 0..4 {
            assert_eq!((sub.vip(s), sub.seats(s), sub.standing(s), sub.roof(s)), (0, STANDS_TABLE[s], STANDS_TABLE[4 + s], true));
        }
        let mut records = Records::from_save(save).unwrap();
        start_clubs(&mut records, 2);
        let sub = &stadium::subs(&records, country as u8, number).unwrap()[0].0;
        assert_eq!(&sub[STAND_BITS..STAND_BITS + 2], &48u16.to_le_bytes(), "the seats' table run on to +0x60");
        assert_eq!(records.team(country, number).unwrap().board(), 0);
    }

    /// The oracle's new game takes the defaults: applied to its dump of the
    /// first turn, the order as it stands, they leave every block as the
    /// original has it, its fixture lists as listed.
    #[test]
    #[ignore = "reads the oracle's dumps from $ANSTOSS3_ORACLE"]
    fn the_defaults_leave_a_new_game_as_the_originals_first_turn() {
        let Some(oracle) = std::env::var_os("ANSTOSS3_ORACLE") else {
            eprintln!("no oracle dump of a new game's first turn among the saves: ANSTOSS3_ORACLE is not set");
            return;
        };
        let path = std::path::PathBuf::from(oracle).join("runs/new-game/first-turn.an3");
        if !path.exists() {
            eprintln!("no oracle dump of a new game's first turn among the saves");
            return;
        }
        let dump = a3_files::an3::Save::read(&path).unwrap();
        let mut records = Records::from_save(dump.clone()).unwrap();
        assert_eq!(records.globals.preparation(), 5);
        assert_eq!(records.globals.get::<u8>(DISMISSALS), 1);
        records.globals.set_preparation(0);
        records.globals.put::<u8>(DISMISSALS, 0);
        apply(&mut records, &Settings { order_as_listed: true, ..Settings::default() }, &mut Random::new(1));
        let apart: Vec<String> = a3_files::an3::compare(&records.to_save(), &dump).blocks.iter().map(|d| d.to_string()).collect();
        assert!(apart.is_empty(), "{apart:#?}");
    }
}
