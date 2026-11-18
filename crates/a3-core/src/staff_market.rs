//! The staff markets as the new season opens (`FUN_007719a6`): the
//! co-trainers' market of each played country made anew from the free men
//! of its band of the pool (`FUN_00409a3f`), the goalkeeper trainers'
//! topped up (`FUN_00409be0`), and the human clubs without a co-trainer
//! or a goalkeeper trainer given one from them (`FUN_004092d0`).
//!
//! A pool record is a save block (`0xbe3920[id]`, ids 1 to 1499, and
//! `0xbe17f8[id]`, ids 1 to 100: the 0x54-byte record from its +0x20). A
//! market is a save block per country slot (`0x8e27a0[b]`, `0x8e2490[b]`:
//! the 0x9c-byte object from its +0x20), a count and 30 entries of an id
//! and his competence.

use crate::records::{Person, Records};
use crate::random::Random;

const CO_POOL: &str = "0xbe3920";
const CO_LAST: i16 = 1499;
const KEEPER_POOL: &str = "0xbe17f8";
const KEEPER_LAST: i16 = 100;
const CO_MARKET: &str = "0x8e27a0";
const KEEPER_MARKET: &str = "0x8e2490";

/// A pool block's fields (the 0x24-byte buffer's +6, +7, +0xe, +0x10,
/// +0x15 of `FUN_006317c6`).
const COMPETENCE: usize = 0x04;
const SECOND_RATING: usize = 0x05;
const YEARS: usize = 0x0c;
const SALARY: usize = 0x10;
const YEARS_ASKED: usize = 0x15;

/// A market holds 30; a full one takes a newcomer by a draw of 60, onto
/// the entry drawn if it is one of them (`FUN_0040ae73`).
const MARKET_MOST: usize = 30;
const MARKET_DRAW: u32 = 60;

/// The bands of the pool a country's market draws from, by its cup
/// system: the co-trainers' first and past-last ids (`DAT_00ae6306`,
/// `DAT_00ae6320`), and by country slot the goalkeeper trainers'
/// (`DAT_00ae633a`, `DAT_00ae6354`).
const CO_FIRST: usize = 0xae6306;
const CO_PAST: usize = 0xae6320;
const KEEPER_FIRST: usize = 0xae633a;
const KEEPER_PAST: usize = 0xae6354;
/// The countries played, by slot (`DAT_00ae5d8e`).
const SLOT_COUNTRY: usize = 0xae5d8e;

/// The salary a free man asks, in hundreds by competence, times 90 to
/// 110 per cent (`FUN_00409015` from `DAT_00876a50`; the goalkeeper
/// trainers' `FUN_0040906b` from `DAT_00876a90`, its sixteen further on).
const PAY: [i32; 32] = [
    4, 5, 7, 9, 11, 13, 16, 19, 23, 27, 36, 48, 60, 75, 110, 150, //
    2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 15, 20, 25, 30, 35, 50,
];
const KEEPER_PAY: usize = 16;
const PAY_DIE: u32 = 21;
const PAY_LEAST: i32 = 90;
/// `DAT_00a3b3fc`.
const HUNDRED: i32 = 100;
/// The years a free man asks for when he asks none: one to three.
const YEARS_DIE: u32 = 3;

/// The competences a human club looks for, by its league's level (the
/// third for every lower one): the least and the most (`DAT_00876ad0`),
/// each held to its trainer's level (10 without one).
const WANTED: [(i16, i16); 3] = [(7, 15), (3, 9), (0, 5)];
const NO_TRAINER_CAP: i16 = 10;
/// The tries a club makes for each post (`FUN_004092d0`).
const TRIES: usize = 5;
/// A human club's co-trainer and goalkeeper trainer as its extra data
/// holds them (`DAT_00a5a900`, `DAT_00a5a902` + slot × 0x2940, set by
/// `FUN_0069ce99` and `FUN_0069cf54`).
const EXTRA_CO: usize = 0x2888;
const EXTRA_KEEPER: usize = 0x288a;
/// The team's club fields the fill reads (the club object's +0x105, +0x107).
const LEVEL: usize = 0x12e;
const RANK: usize = 0x130;

/// The season's opening of the staff markets (`FUN_007719a6`'s first three).
pub fn open_season(records: &mut Records, random: &mut Random) {
    build_co_markets(records, random);
    top_up_keeper_markets(records, random);
    fill_human_posts(records, random);
}

fn slots(records: &Records) -> Vec<(usize, u16)> {
    let count = usize::try_from(i32::from(records.globals.more_count()) + 1).unwrap_or(0);
    (0..count).map(|k| (k, records.countries.get::<i16>(SLOT_COUNTRY + 2 * k) as u16)).collect()
}

fn market_name(market: &str, records: &Records, country: u16) -> String {
    format!("{market}[{}]", records.countries.slot(usize::from(country)))
}

fn read_market(records: &Records, name: &str) -> Vec<(i16, i16)> {
    let Some(b) = records.block(name) else { return Vec::new() };
    let i16_at = |at: usize| b.get(at..at + 2).map_or(0, |x| i16::from_le_bytes([x[0], x[1]]));
    let count = usize::try_from(i16_at(0)).unwrap_or(0).min(MARKET_MOST);
    (0..count).map(|k| (i16_at(2 + 4 * k), i16_at(4 + 4 * k))).collect()
}

fn write_market(records: &mut Records, name: &str, entries: &[(i16, i16)]) {
    let Some(b) = records.block_mut(name) else { return };
    if b.len() < 2 + 4 * MARKET_MOST {
        return;
    }
    b[..2 + 4 * MARKET_MOST].fill(0);
    b[..2].copy_from_slice(&(entries.len() as i16).to_le_bytes());
    for (k, (id, comp)) in entries.iter().take(MARKET_MOST).enumerate() {
        b[2 + 4 * k..4 + 4 * k].copy_from_slice(&id.to_le_bytes());
        b[4 + 4 * k..6 + 4 * k].copy_from_slice(&comp.to_le_bytes());
    }
}

/// A newcomer to a market (`FUN_0040ae73`, the keepers' `FUN_0040b35f`):
/// none twice; a full one draws an entry of 60 for him.
fn enter(market: &mut Vec<(i16, i16)>, id: i16, comp: i16, random: &mut Random) {
    if market.iter().any(|&(k, _)| k == id) {
        return;
    }
    if market.len() < MARKET_MOST {
        market.push((id, comp));
    } else {
        let k = random.below(MARKET_DRAW) as usize;
        if k < MARKET_MOST {
            market[k] = (id, comp);
        }
    }
}

fn pay(table: usize, comp: i8, random: &mut Random) -> i32 {
    let base = usize::try_from(comp).ok().and_then(|c| PAY.get(table + c)).copied().unwrap_or(0);
    base * (random.below(PAY_DIE) as i32 + PAY_LEAST) * HUNDRED
}

/// Writes a co-trainers' person as the records keep him into his pool
/// block, which the save holds.
pub(crate) fn store_person(records: &mut Records, id: u16) {
    let Some(bytes) = records.persons.get(usize::from(id)).map(|p| p.bytes().to_vec()) else { return };
    if let Some(b) = records.block_mut(&format!("{CO_POOL}[{id}]")) {
        let n = b.len().min(bytes.len());
        b[..n].copy_from_slice(&bytes[..n]);
    }
}

/// Edits a pool block, and the co-trainers' person as the records keep it.
fn edit(records: &mut Records, pool: &str, id: i16, f: impl FnOnce(&mut [u8])) {
    let Some(b) = records.block_mut(&format!("{pool}[{id}]")) else { return };
    f(b);
    if pool == CO_POOL {
        let bytes = b.to_vec();
        if let (Some(p), Ok(new)) = (records.persons.get_mut(id as usize), Person::new(&bytes)) {
            *p = new;
        }
    }
}

fn ask_terms(b: &mut [u8], random: &mut Random) {
    if b[YEARS_ASKED] == 0 {
        b[YEARS_ASKED] = random.below(YEARS_DIE) as u8 + 1;
    }
    if i32::from_le_bytes([b[SALARY], b[SALARY + 1], b[SALARY + 2], b[SALARY + 3]]) == 0 {
        let salary = pay(0, b[COMPETENCE] as i8, random);
        b[SALARY..SALARY + 4].copy_from_slice(&salary.to_le_bytes());
    }
}

/// `FUN_00409a3f`: each played country's co-trainers' market emptied, and
/// filled from its band with the free men (+0x0c nought) worth a post —
/// a competence over his second rating and four, or a second rating
/// below two (`FUN_00408d19`); every man of the band asks years (one to
/// three) and a salary if he asks none.
fn build_co_markets(records: &mut Records, random: &mut Random) {
    for (_, country) in slots(records) {
        let band = usize::from(records.countries.cup_system(usize::from(country)));
        let first = records.countries.get::<i16>(CO_FIRST + 2 * band);
        let past = records.countries.get::<i16>(CO_PAST + 2 * band);
        let name = market_name(CO_MARKET, records, country);
        let mut market = Vec::new();
        for id in first.max(1)..past.min(CO_LAST + 1) {
            let Some(b) = records.block(&format!("{CO_POOL}[{id}]")) else { continue };
            let (comp, second) = (b[COMPETENCE] as i8, b[SECOND_RATING] as i8);
            if b[YEARS] == 0 && (i32::from(second) + 4 < i32::from(comp) || second < 2) {
                enter(&mut market, id, i16::from(comp), random);
            }
            edit(records, CO_POOL, id, |b| ask_terms(b, random));
        }
        write_market(records, &name, &market);
    }
}

/// `FUN_00409be0`: each played country's goalkeeper trainers' market
/// keeps its men and takes the free ones of its band anew, at a salary
/// drawn afresh; the band's men ask terms as the co-trainers do (the
/// co-trainers' salary table, as the original has it).
fn top_up_keeper_markets(records: &mut Records, random: &mut Random) {
    for (k, country) in slots(records) {
        let first = records.countries.get::<i16>(KEEPER_FIRST + 2 * k);
        let past = records.countries.get::<i16>(KEEPER_PAST + 2 * k);
        let name = market_name(KEEPER_MARKET, records, country);
        let mut market = read_market(records, &name);
        for id in first.max(1)..past.min(KEEPER_LAST + 1) {
            let Some(b) = records.block(&format!("{KEEPER_POOL}[{id}]")) else { continue };
            let free = b[YEARS] == 0;
            let comp = b[COMPETENCE] as i8;
            if free {
                enter(&mut market, id, i16::from(comp), random);
            }
            edit(records, KEEPER_POOL, id, |b| {
                if free {
                    let salary = pay(KEEPER_PAY, comp, random);
                    b[SALARY..SALARY + 4].copy_from_slice(&salary.to_le_bytes());
                }
                ask_terms(b, random)
            });
        }
        write_market(records, &name, &market);
    }
}

/// A man of a market by competence (`FUN_0040af0c`, the keepers'
/// `FUN_0040b3f8`): one of those between the least and the most, else of
/// those below, else of those above, drawn evenly; 0 none.
fn pick(market: &[(i16, i16)], least: i16, most: i16, random: &mut Random) -> i16 {
    let (mut below, mut above, mut between) = (Vec::new(), Vec::new(), Vec::new());
    for &(id, comp) in market {
        if id == 0 {
            continue;
        }
        let bucket = if comp < least {
            &mut below
        } else if most < comp {
            &mut above
        } else {
            &mut between
        };
        if bucket.len() < 100 {
            bucket.push(id);
        }
    }
    for bucket in [between, below, above] {
        if !bucket.is_empty() {
            return bucket[random.below(bucket.len() as u32) as usize];
        }
    }
    0
}

/// `FUN_004092d0`: the human clubs, by country slot, the higher league's
/// and the better squad's first (its level plus one, a thousandfold, less
/// its squad's rank, the club object's +0x105 and +0x107), each without a
/// co-trainer (team +0xa2) or a goalkeeper trainer (+0xa4) given one of
/// its market whose competence its league's level asks, held to its
/// trainer's level — five tries for one no club took before it this
/// opening. The club's extra data names him (`FUN_0069ce99`,
/// `FUN_0069cf54`; the original's arrays of those taken hold five and
/// four, past which it writes over its own loop). As NewSeason000 has it
/// he stands at the club with a year's contract if he had none, and a
/// co-trainer leaves his market, by a step not traced.
fn fill_human_posts(records: &mut Records, random: &mut Random) {
    let mut buckets: Vec<Vec<(i32, (u8, i16))>> = vec![Vec::new(); 5];
    for club in crate::statistics::human_clubs(records) {
        let Some(team) = records.team(u16::from(club.0), club.1 as u16) else { continue };
        let b = usize::from(records.countries.slot(usize::from(club.0)));
        let key = (i32::from(team.get::<u8>(LEVEL)) + 1) * 1000 - i32::from(team.get::<u8>(RANK));
        let Some(bucket) = buckets.get_mut(b) else { continue };
        let at = bucket.iter().position(|&(k, _)| key < k).unwrap_or(bucket.len());
        bucket.insert(at, (key, club));
    }
    let mut taken: [Vec<i16>; 2] = [Vec::new(), Vec::new()];
    for (k, country) in slots(records) {
        let names = [market_name(CO_MARKET, records, country), market_name(KEEPER_MARKET, records, country)];
        let mut markets = [read_market(records, &names[0]), read_market(records, &names[1])];
        for &(_, (nation, number)) in buckets.get(k).map_or(&[][..], Vec::as_slice) {
            let (nation, number) = (u16::from(nation), number as u16);
            let Some(team) = records.team(nation, number) else { continue };
            let cap = match team.humans()[0] {
                0 => NO_TRAINER_CAP,
                n => records.managers.get(usize::from(n) - 1).map_or(NO_TRAINER_CAP, |m| i16::from(m.level())),
            };
            let (least, most) = WANTED[usize::from(team.get::<u8>(LEVEL).min(2))];
            let (least, most) = (least.min(cap), most.min(cap));
            let slot = usize::from(team.human());
            let held = [team.co_trainer(), team.second_coach()];
            for post in 0..2 {
                if held[post] != 0 {
                    continue;
                }
                let mut hired = 0;
                for _ in 0..TRIES {
                    let id = pick(&markets[post], least, most, random);
                    if id < 1 {
                        break;
                    }
                    if !taken[post].contains(&id) {
                        taken[post].push(id);
                        hired = id;
                        break;
                    }
                }
                if hired == 0 {
                    continue;
                }
                if let Some(extra) = slot.checked_sub(1).filter(|&s| s < 4).and_then(|s| records.extras.get_mut(s)) {
                    extra.put::<i16>([EXTRA_CO, EXTRA_KEEPER][post], hired);
                }
                if let Some(team) = records.team_mut(nation, number) {
                    match post {
                        0 => team.set_co_trainer(hired),
                        _ => team.set_second_coach(hired),
                    }
                }
                edit(records, [CO_POOL, KEEPER_POOL][post], hired, |b| b[YEARS] = b[YEARS].max(1));
                if post == 0 {
                    markets[0].retain(|&(id, _)| id != hired);
                    write_market(records, &names[0], &markets[0]);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(name: &str) -> Records {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap()
    }

    fn worth_a_post(r: &Records, id: i16) -> bool {
        let b = r.block(&format!("{CO_POOL}[{id}]")).unwrap();
        let (comp, second) = (b[COMPETENCE] as i8, b[SECOND_RATING] as i8);
        i32::from(second) + 4 < i32::from(comp) || second < 2
    }

    /// The original's markets in NewSeason000 hold the men of the rule:
    /// each co-trainer of his country's band, worth a post, at the
    /// competence his record has, and none twice; the replica's from
    /// SeasonEnd364 hold the same kind, thirty to a market where so many
    /// stand free, every man of the bands asking terms. The keepers'
    /// markets stand as they were, full.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_markets_hold_the_free_men_worth_a_post() {
        let theirs = read("NewSeason000.an3");
        let mut ours = read("SeasonEnd364.an3");
        let before = read("SeasonEnd364.an3");
        crate::rollover::close_played(&mut ours, &mut Random::new(7));
        open_season(&mut ours, &mut Random::new(7));
        for r in [&theirs, &ours] {
            for (_, country) in slots(r) {
                let band = usize::from(r.countries.cup_system(usize::from(country)));
                let range = r.countries.get::<i16>(CO_FIRST + 2 * band)..r.countries.get::<i16>(CO_PAST + 2 * band);
                let market = read_market(r, &market_name(CO_MARKET, r, country));
                assert!(market.len() >= 29, "{country}: {}", market.len());
                for (n, &(id, comp)) in market.iter().enumerate() {
                    assert!(range.contains(&id), "{country}: {id} outside {range:?}");
                    assert!(worth_a_post(r, id), "{country}: {id}");
                    let b = r.block(&format!("{CO_POOL}[{id}]")).unwrap();
                    assert_eq!(i16::from(b[COMPETENCE] as i8), comp, "{country}: {id}'s competence");
                    assert!(!market[..n].iter().any(|&(k, _)| k == id), "{country}: {id} twice");
                }
                let keepers = read_market(r, &market_name(KEEPER_MARKET, r, country));
                assert_eq!(keepers, read_market(&before, &market_name(KEEPER_MARKET, &before, country)));
            }
        }
        for (_, country) in slots(&ours) {
            let band = usize::from(ours.countries.cup_system(usize::from(country)));
            for id in ours.countries.get::<i16>(CO_FIRST + 2 * band)..ours.countries.get::<i16>(CO_PAST + 2 * band) {
                let b = ours.block(&format!("{CO_POOL}[{id}]")).unwrap();
                assert!((1..=3).contains(&b[YEARS_ASKED]), "{id} asks years");
                assert_ne!(&b[SALARY..SALARY + 4], &[0; 4], "{id} asks a salary");
            }
        }
    }

    /// SeasonEnd364's human club lets its co-trainer and its goalkeeper
    /// trainer go at the season's end; the opening gives it a man of each
    /// market, as the original gives it 261 and 16: within the competence
    /// its first division asks if the market has one, named in its extra
    /// data, under a contract, the co-trainer gone from his market.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_human_club_takes_its_posts_from_the_markets() {
        for (seed, r) in [(0, read("NewSeason000.an3")), (7, read("SeasonEnd364.an3")), (11, read("SeasonEnd364.an3"))] {
            let mut r = r;
            if seed != 0 {
                crate::rollover::close_played(&mut r, &mut Random::new(seed));
                open_season(&mut r, &mut Random::new(seed));
            }
            let (nation, number) = crate::statistics::human_clubs(&r)[0];
            let team = r.team(u16::from(nation), number as u16).unwrap();
            let extra = &r.extras[usize::from(team.human()) - 1];
            let (co, keeper) = (team.co_trainer(), team.second_coach());
            assert!(co > 0 && keeper > 0, "seed {seed}: {co} {keeper}");
            assert_eq!((extra.get::<i16>(EXTRA_CO), extra.get::<i16>(EXTRA_KEEPER)), (co, keeper), "seed {seed}");
            let market = read_market(&r, &market_name(CO_MARKET, &r, u16::from(nation)));
            assert!(!market.iter().any(|&(id, _)| id == co), "seed {seed}: {co} still listed");
            let keepers = read_market(&r, &market_name(KEEPER_MARKET, &r, u16::from(nation)));
            let &(_, comp) = keepers.iter().find(|&&(id, _)| id == keeper).expect("the keeper of his market");
            assert!((7..=15).contains(&comp), "seed {seed}: keeper {keeper} at {comp}");
            for (pool, id) in [(CO_POOL, co), (KEEPER_POOL, keeper)] {
                assert_ne!(r.block(&format!("{pool}[{id}]")).unwrap()[YEARS], 0, "seed {seed}: {id} under contract");
            }
        }
    }

    /// A market's newcomer: none twice, the thirty-first by a draw of 60
    /// onto the entry drawn; the pick prefers the men between, then below,
    /// then above.
    #[test]
    fn a_market_takes_and_gives_as_the_original() {
        let mut random = Random::new(3);
        let mut market = Vec::new();
        for id in 1..=30 {
            enter(&mut market, id, id, &mut random);
        }
        enter(&mut market, 5, 99, &mut random);
        assert_eq!(market.len(), 30);
        assert_eq!(market[4], (5, 5));
        for id in 31..200 {
            enter(&mut market, id, 0, &mut random);
        }
        assert_eq!(market.len(), 30);
        assert!(market.iter().any(|&(id, _)| id > 30));
        let market = [(1, 2), (2, 12), (3, 8), (0, 8)];
        assert_eq!(pick(&market, 7, 9, &mut random), 3);
        assert_eq!(pick(&market[..2], 7, 9, &mut random), 1);
        assert_eq!(pick(&market[1..2], 7, 9, &mut random), 2);
        assert_eq!(pick(&[], 7, 9, &mut random), 0);
    }
}
