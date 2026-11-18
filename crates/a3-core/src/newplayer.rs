//! New players for the clubs, made from the records of ended careers
//! (`FUN_0066b6c6`, its id from the regeneration table by `FUN_0066ce4d`).
//!
//! An id comes from the regeneration table's range of the club's country's
//! class (`DAT_0089de50`): drawn first among those who began at that club,
//! then any of the position, the table refilled from the pool once
//! (`FUN_0066dc8b`), then any at all, then another class drawn; its record
//! is wiped but for the names and the face (`FUN_00637be2`). The new
//! player is content in every part, of middling condition and freshness
//! (`FUN_0070745a`), of the position asked and the country's nation, his
//! talent drawn by the club's worth (`FUN_0066b128`), his side positions,
//! abilities and character drawn (`FUN_0066b1ea`, `FUN_0066b456`,
//! `FUN_0066b329`), his languages the country's and a second
//! (`FUN_00705d4c`), his foot (`FUN_0070638f`), a smoker one time in seven,
//! overweight one in twelve.
//!
//! A club of a country kept in short takes him at 19 to 22 on fresh terms
//! of three or four years (`FUN_0066ca21`, `FUN_00709e88`); a computer club
//! of the countries played fills its thin positions with its youngest
//! amateurs or with new youngsters of 18 or 19 (`FUN_0077655c`).

use chrono::NaiveDate;

use crate::contract::{ContractTables, Signee};
use crate::random::Random;
use crate::records::{Player, Records};
use crate::value::ValueTables;

/// The regeneration table (`DAT_00b68ea8`): 9000 entries of the id, the
/// class and the main position; each class's range (`DAT_00b5cc08`,
/// `DAT_00b5cc44`).
const REGENERATION: &str = "0xb68ea8";
const ENTRIES: usize = 9000;
const CLASS_FROM: usize = 0xb5cc08;
const CLASS_TO: usize = 0xb5cc44;
const CLASSES: u32 = 30;
/// Ids from this one on are not made anew (the player-managers).
const REMADE_BELOW: u16 = 0x4269;
/// Where a pooled player began: his country and club (R+0x164, +0x166).
const BEGAN_COUNTRY: usize = 0x164;
const BEGAN_TEAM: usize = 0x166;

/// Each country's class (`DAT_0089de50`).
const COUNTRY_CLASS: [u8; 117] = [
    0, 11, 25, 11, 3, 1, 14, 4, 5, 15, 4, 13, 4, 16, 0, 4, 1, 23, 16, 17, 2, 23, 11, 18, 6, 1, 17, 19, 3, 1,
    13, 11, 1, 4, 25, 23, 11, 23, 2, 3, 11, 4, 22, 1, 11, 1, 20, 7, 4, 4, 21, 5, 4, 22, 23, 3, 12, 8, 24, 9,
    11, 26, 26, 4, 13, 26, 11, 10, 23, 27, 4, 1, 5, 1, 11, 18, 25, 25, 11, 25, 25, 11, 4, 11, 11, 11, 2, 4,
    13, 13, 12, 12, 11, 12, 11, 11, 13, 12, 12, 2, 12, 13, 1, 11, 1, 25, 12, 11, 1, 11, 11, 12, 13, 13, 11,
    11, 23,
];
/// Each country's language (`DAT_0089dec8`), and the countries a German
/// speaker's second language is drawn from (`DAT_0089df40`).
const LANGUAGE: [u8; 117] = [
    1, 10, 9, 10, 4, 1, 0, 4, 5, 9, 4, 11, 4, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 1, 1, 4,
    1, 8, 1, 8, 0, 1, 2, 4, 1, 1, 1, 1, 1, 0, 4, 4, 0, 5, 4, 1, 8, 3, 0, 1, 1, 1, 1, 1, 1, 4, 1, 1, 1, 1, 1,
    1, 1, 4, 1, 4, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
];
const GERMAN: u8 = 1;
const OTHER_LANGUAGES: [u8; 10] = [14, 16, 20, 28, 63, 51, 67, 54, 24, 23];
const FRANCE: u8 = 0x14;
const ENGLAND: u8 = 0x10;
/// The positions a club's new player is drawn from (`DAT_0089ffc0`), the
/// side position by the main one (`DAT_0089ffcc`).
const DRAWN_POSITIONS: [u8; 12] = [1, 2, 3, 3, 4, 5, 6, 7, 8, 9, 10, 10];
const SIDE: [u8; 11] = [0, 0, 3, 2, 3, 3, 9, 8, 2, 10, 9];
/// The chances of the character bits by talent, sixteen a row
/// (`DAT_008a0010`, `DAT_008a0060`).
const TRAITS_A: [u8; 80] = [
    3, 3, 4, 3, 3, 5, 7, 7, 6, 6, 7, 4, 5, 0, 5, 7, 4, 4, 4, 4, 4, 5, 5, 5, 5, 5, 6, 3, 5, 2, 4, 5, 5, 5,
    3, 5, 3, 4, 3, 3, 4, 4, 5, 2, 4, 5, 3, 3, 6, 6, 3, 6, 2, 3, 1, 2, 4, 3, 4, 1, 3, 6, 3, 1, 7, 7, 2, 7,
    2, 3, 0, 1, 3, 3, 3, 1, 2, 7, 3, 0,
];
const TRAITS_B: [u8; 80] = [
    7, 5, 3, 5, 5, 3, 7, 2, 4, 7, 7, 7, 7, 7, 7, 4, 6, 5, 4, 6, 4, 3, 6, 4, 5, 5, 5, 5, 4, 5, 5, 4, 5, 5,
    5, 7, 3, 3, 5, 5, 6, 3, 3, 3, 2, 4, 4, 5, 4, 5, 4, 7, 2, 3, 4, 6, 6, 1, 1, 1, 1, 3, 3, 5, 3, 5, 3, 7,
    2, 3, 3, 7, 6, 0, 0, 0, 0, 2, 2, 5,
];
/// The days of the months.
const MONTH_DAYS: [u32; 13] = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
/// The money units the worth divides by (`DAT_00a3b36c`, `DAT_00a3b370`,
/// `DAT_00a3b37c`), and a kept club's worth per class (`DAT_00a3b348`).
const UNIT_200K: i32 = 200_000;
const UNIT_150K: i32 = 150_000;
const UNIT_100K: i32 = 100_000;
pub(crate) const WORTH_PER_CLASS: i32 = 500_000;
/// A kept club's class (team +0x875, the club object's +0x11e0).
const CLUB_CLASS: usize = 0x875;
/// The seasons count the rollover keeps (R+0x161), the retiring byte and
/// the picks.
const SEASONS: usize = 0x161;

fn entry(table: &[u8], k: usize) -> (i16, u8) {
    let at = 4 * k;
    table.get(at..at + 4).map_or((0, 0), |e| (i16::from_le_bytes([e[0], e[1]]), e[3]))
}

/// An id for a new player (`FUN_0066ce4d`), the table's entry taken and
/// the record wiped; `None` when the table has none at all.
fn pick_id(records: &mut Records, country: u8, team: i16, position: Option<u8>, random: &mut Random) -> Option<u16> {
    let mut class = COUNTRY_CLASS.get(usize::from(country)).copied().unwrap_or(0) as usize;
    let mut position = position;
    let range = |records: &Records, class: usize| {
        let g = &records.globals;
        (g.get::<i16>(CLASS_FROM + 2 * class), g.get::<i16>(CLASS_TO + 2 * class))
    };
    let fits = |position: Option<u8>, p: u8| position.is_none_or(|q| q == p);
    let take = |records: &mut Records, k: usize| -> Option<u16> {
        let table = records.block_mut(REGENERATION)?;
        let (id, _) = entry(table, k);
        table[4 * k..4 * k + 4].fill(0);
        let id = u16::try_from(id).ok()?;
        wipe(records, id);
        Some(id)
    };
    let table = records.block(REGENERATION)?.to_vec();
    let (from, to) = range(records, class);
    if country > 0 && team >= 0 {
        for _ in from..to {
            let k = usize::try_from(from + random.below((to - from) as u32) as i16).unwrap_or(0);
            let (id, p) = entry(&table, k);
            if id > 0 && fits(position, p) {
                let began = records.player(id as u16).map(|r| (r.get::<i16>(BEGAN_COUNTRY), r.get::<i16>(BEGAN_TEAM)));
                if began == Some((i16::from(country), team)) {
                    return take(records, k);
                }
            }
        }
        for k in from.max(0) as usize..to.max(0) as usize {
            let (id, p) = entry(&table, k);
            if id > 0 && fits(position, p) {
                // The original compares the club's word twice here.
                let began = records.player(id as u16).map(|r| r.get::<i16>(BEGAN_TEAM));
                if began == Some(i16::from(country)) && began == Some(team) {
                    return take(records, k);
                }
            }
        }
    }
    let mut tries = 0;
    loop {
        for pass in 0..2 {
            let (from, to) = range(records, class);
            let table = records.block(REGENERATION)?.to_vec();
            for _ in from..to {
                let k = usize::try_from(from + random.below((to - from) as u32) as i16).unwrap_or(0);
                let (id, p) = entry(&table, k);
                if id > 0 && fits(position, p) {
                    return take(records, k);
                }
            }
            if pass == 0 {
                crate::rollover::regenerate_class(records, class, random);
            }
        }
        let (from, to) = range(records, class);
        let table = records.block(REGENERATION)?.to_vec();
        if let Some(k) = (from.max(0) as usize..to.max(0) as usize).find(|&k| entry(&table, k).0 > 0) {
            return take(records, k);
        }
        class = random.below(CLASSES) as usize;
        tries += 1;
        if tries == 1 {
            class = 1;
        }
        if tries == 20 {
            if position.is_none() {
                let k = (0..ENTRIES).find(|&k| entry(&table, k).0 > 0)?;
                return take(records, k);
            }
            position = None;
            tries = 0;
        }
    }
}

/// A player's record wiped for his new career (`FUN_00637be2`): all but his
/// names and face, born in 1970, strength 1, a keeper.
fn wipe(records: &mut Records, id: u16) {
    if id >= REMADE_BELOW {
        return;
    }
    let Some(p) = records.player_mut(id) else { return };
    let (first, last, face) = (p.first_name(), p.last_name(), p.face());
    *p = Player::new(&vec![0; Player::SIZE]).expect("a player's size");
    p.set_first_name(first);
    p.set_last_name(last);
    p.set_face(face);
    p.set_id(id);
    p.set_birth_year(1970);
    p.set_strength(1);
    p.set_positions([1, 0, 0]);
}

/// Content in every part, of middling condition and freshness
/// (`FUN_0070745a`).
fn freshen(p: &mut Player, random: &mut Random) {
    p.set_contentment([100; 7]);
    p.set_morale(crate::mood::morale(p));
    let condition = (random.below(20) as i16 + 40) * 100;
    p.set_condition(condition.clamp(0, 10000));
    let mut fresh = p.freshness();
    fresh[0] = ((random.below(20) as i16 + 40) * 100).clamp(0, 10000);
    p.set_freshness(fresh);
}

/// His talent by his club's worth (`FUN_0066b128`): 0 the greatest to 4.
fn talent_for(worth: i32, random: &mut Random) -> i8 {
    let r = random.below(100) as i32;
    let a = worth / UNIT_200K;
    let first = a + 15 + worth / UNIT_150K;
    let second = first + 15 - a;
    if r < a + 5 {
        0
    } else if r < first {
        1
    } else if r < second {
        4
    } else if r < second + 25 - worth / UNIT_100K {
        3
    } else {
        2
    }
}

/// His side positions (`FUN_0066b1ea`).
fn sides(p: &mut Player, random: &mut Random) {
    let mut positions = p.positions();
    let main = positions[0];
    if main == 1 {
        positions[1] = 0;
        positions[2] = 0;
    } else if random.below(100) < 40 {
        positions[1] = SIDE.get(usize::from(main)).copied().unwrap_or(0);
        let r = random.below(100);
        if r < 20 {
            positions[2] = match main {
                2 | 3 | 8 | 9 | 10 => random.below(9) as u8 + 2,
                4 => 7,
                5 => 6,
                6 => 5,
                7 => 4,
                _ => positions[2],
            };
        } else if r < 30 {
            positions[2] = random.below(9) as u8 + 2;
        }
        if positions[2] == main || positions[2] == positions[1] {
            positions[2] = 0;
        }
    }
    p.set_positions(positions);
}

/// His abilities, good and bad, and his character (`FUN_0066b456`,
/// `FUN_0066b329`); `plain` when made for a computer club.
fn abilities(p: &mut Player, plain: bool, random: &mut Random) {
    let n = if p.positions()[0] != 1 { 16 } else { 9 };
    let (mut good, mut bad) = (0u16, p.bad());
    p.set_traits([0, 0]);
    p.set_knocks(0);
    let mut r = random.below(100);
    if plain && (60..80).contains(&r) {
        r = 50;
    }
    if r < 60 {
        good |= 1 << random.below(n);
    } else if r < 80 {
        good |= 1 << random.below(n);
        if random.below(2) == 0 {
            bad |= 1 << random.below(n);
        } else {
            good |= 1 << random.below(n);
        }
    }
    if r % 33 < 2 {
        bad |= 1 << random.below(n);
    }
    bad &= !good;
    p.set_good(good);
    p.set_bad(bad);
    let row = usize::try_from(p.talent()).unwrap_or(0).min(4) * 16;
    let mut traits = [0u16; 2];
    for (k, table) in [&TRAITS_A, &TRAITS_B].into_iter().enumerate() {
        let mut r = random.below(100);
        for bit in 0..16 {
            let chance = u32::from(table[row + bit]);
            if r < chance {
                traits[k] |= 1 << bit;
                break;
            }
            r -= chance;
        }
    }
    p.set_traits(traits);
    if random.below(150) == 0 {
        p.set_shown(p.shown() | 0x4000);
    }
    let r = random.below(100);
    if r < 10 {
        p.set_knocks(p.knocks() | 2);
    } else if r < 20 {
        p.set_knocks(p.knocks() | 4);
    }
}

/// A language more (`FUN_00705d4c`): the country's bit in R+0x216, his
/// language problems gone.
fn speaks(p: &mut Player, country: u8) {
    let bit = LANGUAGE.get(usize::from(country)).copied().unwrap_or(0);
    p.put::<u16>(0x216, p.get::<u16>(0x216) | (1 << (bit & 0x1f)));
    p.set_problems(p.problems() & 0xf3);
}

/// His strong foot (`FUN_0070638f`): left one time in seven, right most of
/// the time, both else; a left-sided man left-footed takes both, a
/// right-sided one right-footed too.
fn foot(p: &mut Player, random: &mut Random) {
    let r = random.below(100);
    let mut f: u8 = if r < 15 { 2 } else if r < 85 { 1 } else { 3 };
    let set = |p: &mut Player, f: u8| p.set_manner((p.manner() & 0xf3) | (f << 2));
    set(p, f);
    let main = p.positions()[0];
    if matches!(main, 5 | 6) && f == 2 {
        f = 3;
        set(p, f);
    }
    if matches!(main, 4 | 7) && f == 1 {
        set(p, 3);
    }
}

/// A new player for a club (`FUN_0066b6c6`), of `position` and drawn by
/// `worth`; `plain` for a computer club's.
pub(crate) fn make(
    records: &mut Records,
    (country, team): (u8, i16),
    position: u8,
    worth: i32,
    plain: bool,
    random: &mut Random,
) -> Option<u16> {
    let id = pick_id(records, country, team, Some(position), random)?;
    let p = records.player_mut(id)?;
    freshen(p, random);
    let mut positions = p.positions();
    positions[0] = position;
    p.set_positions(positions);
    p.set_nation(country);
    p.set_manner(p.manner() & !0x10);
    p.set_talent(talent_for(worth, random));
    p.put::<u8>(0x27d, 5);
    p.put::<u8>(0x353, 10);
    sides(p, random);
    abilities(p, plain, random);
    speaks(p, country);
    if LANGUAGE.get(usize::from(country)) == Some(&GERMAN) {
        if random.below(10) < 8 {
            speaks(p, FRANCE);
        } else {
            let other = OTHER_LANGUAGES[random.below(10) as usize];
            speaks(p, other);
        }
    } else if random.below(10) < 8 {
        speaks(p, ENGLAND);
    } else {
        speaks(p, FRANCE);
    }
    foot(p, random);
    if random.below(100) < 15 {
        p.set_shown(p.shown() | 0x800);
    }
    if random.below(100) < 8 {
        p.set_shown(p.shown() | 0x20);
    }
    Some(id)
}

/// His birthday for an age of `age` on the game's day (`FUN_00675923`).
fn born(records: &Records, p: &mut Player, age: i32, random: &mut Random) {
    let (d, m, y) = birth_date(records, age, random);
    p.set_birth_day(d);
    p.set_birth_month(m);
    p.set_birth_year(y);
}

/// The day, month and year of such a birthday.
fn birth_date(records: &Records, age: i32, random: &mut Random) -> (u8, u8, u16) {
    let [day, month, year] = records.globals.date();
    let m = random.below(12) + 1;
    let d = random.below(MONTH_DAYS[m as usize]) + 1;
    let base = i32::from(year) - age;
    let y = if m < u32::from(month) {
        base
    } else if m > u32::from(month) || d > u32::from(day) {
        base - 1
    } else {
        base
    };
    (d as u8, m as u8, y as u16)
}

/// A new youth's age: 13 and four more drawn; one under this is of
/// strength 1 to 3, an older one of 3 to 5.
const YOUTH_FROM: u32 = 13;
const YOUTH_AGES: u32 = 5;
const YOUNG_YOUTH: u32 = 15;
/// One in five is born abroad, in any of the 116 countries.
const ABROAD_ODDS: u32 = 5;
const NATIONS: u32 = 0x74;
/// Born abroad (R+0x282's bit, `FUN_00705861`).
const BORN_ABROAD: u8 = 0x10;
/// The planned youth support (club +0x2b0) a youth's talent is drawn by.
pub(crate) const YOUTH_SUPPORT: usize = 0x2b0 + crate::finances::CLUB_TO_TEAM;
/// R+0x1b4's bits a new youth may come with (`FUN_0070018a`,
/// `FUN_006ffffb`, `FUN_006fff76`), and R+0x161.
const RARE_BIT: u32 = 0x4000;
const SMOKES: u32 = 0x800;
const OVERWEIGHT: u32 = 0x20;
/// The youth stations' kinds by class (C, B, A).
const YOUTH_STATION: u8 = 9;

/// A new youth for a computer club's youth team (`FUN_0066c0c2`; a human
/// club's draws more bits, not built), while it holds under fifteen with
/// those moved up (`FUN_004d7940`): 13 to 17 years, one in five of a
/// nation drawn, of a position drawn, made by the club's youth support
/// (`FUN_0066b6c6`) and fresh, with no contract, salary or seasons; of
/// strength 1 to 3 under fifteen, else 3 to 5; R+0x26f at 8, one in 150
/// with R+0x1b4's 0x4000; his nation's language; his station as a youth
/// of his class, into the youth team; his foot; over fifteen a smoker
/// one in five and overweight one in twelve and a half; one in twenty
/// onto the youth list.
pub(crate) fn youth_for(records: &mut Records, (country, number): (u8, u16), random: &mut Random) -> Option<u16> {
    let (youths, moved_up) = crate::youth::youths(records, country, number);
    if youths.len() + moved_up >= crate::youth::FULL {
        return None;
    }
    let age = random.below(YOUTH_AGES) + YOUTH_FROM;
    let (day, month, year) = birth_date(records, age as i32, random);
    let nation = if random.below(ABROAD_ODDS) == 0 { random.below(NATIONS) as u8 + 1 } else { country };
    let position = DRAWN_POSITIONS[random.below(12) as usize];
    let worth = records.team(u16::from(country), number)?.get::<i32>(YOUTH_SUPPORT);
    let id = make(records, (country, number as i16), position, worth, true, random)?;
    let start = records.globals.start_year();
    {
        let p = records.player_mut(id)?;
        freshen(p, random);
        p.set_nation(nation);
        if nation != country {
            p.set_manner(p.manner() | BORN_ABROAD);
        }
        p.set_birth_day(day);
        p.set_birth_month(month);
        p.set_birth_year(year);
        p.set_contract(start);
        p.set_salary(0);
        p.put::<u8>(SEASONS, 0);
        let strength = random.below(3) as u8 + if age < YOUNG_YOUTH { 1 } else { 3 };
        p.set_strength(strength);
        p.set_retiring(8);
        if random.below(150) == 0 {
            p.set_shown(p.shown() | RARE_BIT);
        }
        speaks(p, nation);
    }
    let class = crate::youth::class(records, id) as u8;
    crate::deals::open_youth_station(records, id, (country, number as i16), YOUTH_STATION + class);
    crate::youth::join_youth(records, (country, number), id);
    let p = records.player_mut(id)?;
    p.set_picks(0);
    foot(p, random);
    if age > YOUNG_YOUTH {
        if random.below(100) < 20 {
            p.set_shown(p.shown() | SMOKES);
        }
        if random.below(100) < 8 {
            p.set_shown(p.shown() | OVERWEIGHT);
        }
    }
    if random.below(20) == 0 {
        crate::youth::add_prospect(records, id, (country, number));
    }
    Some(id)
}

/// A new amateur is 20 and nine more drawn.
const AMATEUR_FROM: u32 = 20;
const AMATEUR_AGES: u32 = 10;
/// R+0x271: his status known (bits 6 and 7 at 1), the status in bits 4
/// and 5 (0 "Stammspieler" to 2 "Starspieler").
pub(crate) const STATUS_KNOWN: u8 = 0x40;
pub(crate) const STATUS_MASK: u8 = 0x30;
pub(crate) const STATUS_SHIFT: u8 = 4;
pub(crate) const STATUS_MOST: u8 = 2;
/// The strengths an amateur of a status is drawn to, by a draw of a
/// hundred (`FUN_0040767a`, `DAT_008769b4`): the first band strength 1.
const STATUS_STRENGTH: [[u32; 8]; 3] = [
    [20, 25, 25, 15, 10, 5, 0, 0],
    [10, 15, 25, 25, 15, 7, 3, 0],
    [0, 7, 13, 31, 31, 12, 6, 2],
];

/// An amateur's status (`FUN_00706ccf`).
pub(crate) fn status(p: &Player) -> u8 {
    (p.picks() & STATUS_MASK) >> STATUS_SHIFT
}

/// His status set, within 0 and 2 (`FUN_00706ced`).
pub(crate) fn set_status(p: &mut Player, status: i32) {
    let status = status.clamp(0, i32::from(STATUS_MOST)) as u8;
    p.set_picks((p.picks() & !STATUS_MASK) | (status << STATUS_SHIFT));
}

/// A strength for a status (`FUN_0040767a`).
fn status_strength(status: u8, random: &mut Random) -> u8 {
    let row = STATUS_STRENGTH[usize::from(status.min(STATUS_MOST))];
    let mut r = random.below(100);
    let mut k = 0;
    while k < row.len() && row[k] <= r {
        r -= row[k];
        k += 1;
    }
    k as u8 + 1
}

/// A new amateur for a club's pool (`FUN_006c41b1`'s tail; the new game's
/// `FUN_0066bb26` makes his like): one in five of a nation drawn, of a
/// position drawn, his record wiped (`FUN_0066ce4d`) and fresh
/// (`FUN_0070745a`), 20 to 29, with no side positions, contract or
/// salary, R+0x26f at 8, his status known at the lowest and his strength
/// drawn by it (`FUN_00706b1e`), his nation's language; among the pool's
/// amateurs (`FUN_00407c58`) with an amateur's station at the club
/// (`FUN_00707c3f`).
pub(crate) fn amateur_for(records: &mut Records, (country, number): (u8, u16), random: &mut Random) -> Option<u16> {
    let nation = if random.below(ABROAD_ODDS) == 0 { random.below(NATIONS) as u8 + 1 } else { country };
    let position = DRAWN_POSITIONS[random.below(12) as usize];
    let id = pick_id(records, nation, number as i16, Some(position), random)?;
    {
        let p = records.player_mut(id)?;
        freshen(p, random);
        p.set_nation(nation);
        if nation != country {
            p.set_manner(p.manner() | BORN_ABROAD);
        }
    }
    let age = random.below(AMATEUR_AGES) + AMATEUR_FROM;
    let (day, month, year) = birth_date(records, age as i32, random);
    let start = records.globals.start_year();
    {
        let p = records.player_mut(id)?;
        p.set_birth_day(day);
        p.set_birth_month(month);
        p.set_birth_year(year);
        p.set_positions([position, 0, 0]);
        p.set_contract(start);
        p.set_salary(0);
        p.set_retiring(8);
        p.set_picks(STATUS_KNOWN);
        p.set_strength(status_strength(0, random));
        speaks(p, nation);
    }
    let [mut up, mut amateurs] = crate::youth::pool(records, country, number);
    up.retain(|&q| q != id);
    if !amateurs.contains(&id) && amateurs.len() < crate::youth::POOL {
        amateurs.push(id);
    }
    crate::deals::open_amateur_station(records, id, (country, number as i16));
    crate::youth::store_pool(records, country, number, &[up, amateurs]);
    Some(id)
}

/// Fresh terms for a new player at a club (`FUN_00709e88`): his salary as he
/// would ask it, in thousands, the bonuses by the club's league, two years.
fn terms(records: &mut Records, id: u16, level: u8, tables: (&ContractTables, &ValueTables)) {
    let year = records.globals.start_year();
    let today = {
        let [d, m, y] = records.globals.date();
        NaiveDate::from_ymd_opt(i32::from(y), u32::from(m), u32::from(d))
    };
    let Some(p) = records.player(id) else { return };
    let age = today.map_or(0, |t| crate::statistics::age_on(p, t));
    let form = crate::statistics::form_today(p);
    let position = p.positions()[0];
    let signee = Signee::of(p, age, form, tables.1, false, year);
    let salary = (tables.0.demand(&signee) / 1000) as u16;
    let share = match level {
        1 => 12.0,
        2.. => 16.0,
        _ => 8.0,
    };
    let appearance = (f64::from(salary) / share) as i32 as u16;
    let goal = match position {
        10 => Some((f64::from(salary) / share / 1.25) as i32 as u16),
        9 => Some((f64::from(salary) / share / 2.5) as i32 as u16),
        _ => None,
    };
    let Some(p) = records.player_mut(id) else { return };
    p.set_salary(salary);
    p.set_appearance_bonus(appearance);
    if let Some(goal) = goal {
        p.set_goal_bonus(goal);
    }
    p.set_fixed_fee(0);
    p.set_terms(0);
    p.set_contract(year + 2);
}

/// A club of a country kept in short takes a new player (`FUN_0066ca21`):
/// of `position`, or one drawn when `None`; aged 19 to 22, on terms of
/// three or four years, into its squad by his position.
pub fn for_kept_club(
    records: &mut Records,
    (country, number): (u16, u16),
    position: Option<u8>,
    tables: (&ContractTables, &ValueTables),
    random: &mut Random,
) -> Option<u16> {
    let position = position.unwrap_or_else(|| DRAWN_POSITIONS[random.below(12) as usize]);
    let team = records.team(country, number)?;
    let (worth, level) = (i32::from(team.get::<u8>(CLUB_CLASS)) * WORTH_PER_CLASS, team.level());
    let id = make(records, (country as u8, number as i16), position, worth, true, random)?;
    {
        let p = records.player_mut(id)?;
        freshen(p, random);
    }
    let age = random.below(4) as i32 + 19;
    let mut p = records.player(id)?.clone();
    born(records, &mut p, age, random);
    *records.player_mut(id)? = p;
    terms(records, id, level, tables);
    let year = records.globals.start_year();
    let p = records.player_mut(id)?;
    p.put::<u8>(SEASONS, 0);
    p.set_contract(year + 3 + random.below(2) as u16);
    foot(p, random);
    p.set_retiring(8);
    p.set_picks(0);
    join(records, (country, number), id);
    Some(id)
}

/// The least a computer club of the countries played keeps at each
/// position (`DAT_008acba0`), the strength its new youngsters come at by
/// its league's class and how many it has there (`DAT_008acb90`), the worth
/// they are drawn by (`DAT_00a3b348`).
const LEAST: [i16; 11] = [0, 2, 0, 3, 1, 1, 1, 1, 2, 2, 3];
const YOUTH_STRENGTH: [u8; 16] = [8, 3, 2, 1, 6, 2, 1, 1, 3, 1, 1, 1, 1, 1, 1, 1];
const FULL_SQUAD: usize = 31;
const FIELD_FULL: usize = 23;
const TRIES: usize = 5;

/// A league's class for its youngsters (`FUN_006755b8`): the main
/// country's up to 2 (England's 3), the others' 0; 3 out of the leagues.
fn youth_class(records: &Records, country: u16, level: u8) -> usize {
    if level >= 10 {
        return 3;
    }
    if i16::try_from(country).ok() == Some(records.globals.main_country()) {
        usize::from(if country == u16::from(ENGLAND) { level.min(3) } else { level.min(2) })
    } else {
        0
    }
}

/// The youngest amateur of a position at a club that is no reserve team
/// and has none (`FUN_00407774`).
fn youngest_amateur(records: &Records, (country, number): (u16, u16), position: u8) -> Option<u16> {
    let team = records.team(country, number)?;
    if team.linked() != [-1, -1] {
        return None;
    }
    let today = {
        let [d, m, y] = records.globals.date();
        NaiveDate::from_ymd_opt(i32::from(y), u32::from(m), u32::from(d))?
    };
    let [_, amateurs] = crate::youth::pool(records, country as u8, number);
    let mut best: Option<(i32, u16)> = None;
    for id in amateurs {
        let Some(p) = records.player(id) else { continue };
        if p.positions()[0] != position {
            continue;
        }
        let age = crate::statistics::age_on(p, today);
        if best.is_none_or(|(a, _)| age < a) {
            best = Some((age, id));
        }
    }
    best.map(|(_, id)| id)
}

/// The new season fills a computer club of the countries played where it
/// is thin (`FUN_0077655c`'s first loop, from the second season on): for
/// each position below its least, up to five tries, the youngest amateur
/// of it moves up (`FUN_0040770f`) or else a new youngster comes
/// (`FUN_0066b6c6`) — 18 or 19, his strength by the league's class and the
/// club's count at the position (1 out of the leagues), his talent 1 or
/// 2, a station and two years' terms; a squad of 31, or of 23 past the
/// keepers, takes no more.
pub fn fill_played_club(
    records: &mut Records,
    (country, number): (u16, u16),
    tables: (&ContractTables, &ValueTables),
    random: &mut Random,
) {
    let Some(team) = records.team(country, number) else { return };
    if team.humans() != [0, 0] || records.globals.seasons() <= 1 {
        return;
    }
    let level = team.level();
    let class = youth_class(records, country, level);
    let squad = team.players();
    let mut counts = [0i16; 11];
    for &id in &squad {
        if let Some(p) = records.player(id) {
            counts[usize::from(p.positions()[0]).min(10)] += 1;
        }
    }
    let mut size = squad.len();
    for position in 1..11u8 {
        if size >= FULL_SQUAD || (position >= 2 && size >= FIELD_FULL) {
            break;
        }
        let k = usize::from(position);
        let mut tries = 0;
        while counts[k] < LEAST[k] && tries < TRIES {
            tries += 1;
            if let Some(id) = youngest_amateur(records, (country, number), position) {
                terms(records, id, level, tables);
                crate::youth::leave_pool(records, (country as u8, number), id);
                join(records, (country, number), id);
                if let Some(p) = records.player_mut(id) {
                    p.put::<u8>(SEASONS, 0);
                    p.set_picks(0);
                }
            } else {
                let Some(id) = make(records, (country as u8, number as i16), position, WORTH_PER_CLASS, true, random) else {
                    continue;
                };
                let age = random.below(2) as i32 + 18;
                let mut p = match records.player(id) {
                    Some(p) => p.clone(),
                    None => continue,
                };
                born(records, &mut p, age, random);
                p.set_strength(if level == 10 { 1 } else { YOUTH_STRENGTH[class * 4 + (counts[k] as usize).min(3)] });
                p.set_talent(2 - random.below(2) as i8);
                if let Some(slot) = records.player_mut(id) {
                    *slot = p;
                }
                crate::deals::open_station(records, id, (country as u8, number as i16), 0);
                terms(records, id, level, tables);
                join(records, (country, number), id);
            }
            counts[k] += 1;
            size += 1;
        }
    }
}

/// Into a club's squad by his main position, his club its (`FUN_0070df0b`).
pub(crate) fn join(records: &mut Records, (country, number): (u16, u16), id: u16) {
    let position = records.player(id).map_or(0, |p| p.positions()[0]);
    let squad = records.team(country, number).map_or_else(Vec::new, |t| t.players());
    let positions: Vec<u8> = squad.iter().map(|&q| records.player(q).map_or(0, |p| p.positions()[0])).collect();
    let mut at = squad.len();
    while at > 0 && position < positions[at - 1] {
        at -= 1;
    }
    let mut squad = squad;
    squad.insert(at, id);
    if let Some(team) = records.team_mut(country, number) {
        team.set_players(&squad);
    }
    if let Some(p) = records.player_mut(id) {
        p.set_club([country as u8, number as u8]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn talent_follows_the_clubs_worth() {
        let mut random = Random::new(3);
        let count = |worth: i32, random: &mut Random| {
            let mut c = [0; 5];
            for _ in 0..2000 {
                c[usize::try_from(talent_for(worth, random)).unwrap()] += 1;
            }
            c
        };
        let poor = count(0, &mut random);
        let rich = count(5 * WORTH_PER_CLASS, &mut random);
        assert!(rich[0] > poor[0] * 2, "{rich:?} against {poor:?}");
    }

    /// Over the original's pair: SeasonEnd364 turned to the new season by the
    /// replica as the cycle turns it, each thin kept club's
    /// new players are made as NewSeason000's are — keepers of strength 1
    /// where two were missing, aged 19 to 22, of the club's nation, on three
    /// or four years, the retiring byte at 8 — and about as many as the
    /// original's recycled players at those clubs, over four turns.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_kept_clubs_take_new_players_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (theirs, before) = (load("NewSeason000.an3"), load("SeasonEnd364.an3"));
        let data = crate::data::Data::load(&dir).unwrap();
        let exe = a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap();
        let tables = crate::league::LeagueTables::load(&exe).unwrap();
        // Four turns of the season, the count of new players summed: a
        // single draw's count swings with every other draw of the turn.
        let (mut made, mut theirs_made) = (0, 0);
        for seed in [3, 4, 5, 6] {
            let mut ours = load("SeasonEnd364.an3");
            let mut random = Random::new(seed);
            let drawn = crate::promotion::draw(&mut ours, &mut random);
            crate::promotion::count(&mut ours);
            crate::development::develop_played(&mut ours, false, &mut random);
            crate::rollover::retire_announced(&mut ours, &mut random);
            crate::statistics::archive_leagues(&mut ours);
            crate::promotion::flag(&mut ours, &data.rules.forecast.tickets);
            crate::rollover::count_first_seasons(&mut ours);
            let agents = crate::free_agents::release(&mut ours, &mut random);
            crate::deals::execute(&mut ours, &mut random);
            crate::rollover::close_played(&mut ours, &mut random);
            crate::rollover::close_kept(&mut ours, &mut random);
            crate::free_agents::place(&mut ours, &agents, (&data.rules.contract, &data.rules.value), &mut random);
            crate::settlement::settle_season(&mut ours, &tables);
            crate::promotion::take(&mut ours, &drawn);
            crate::settlement::open_season_with(&mut ours, &mut random, Some(&data.rules));
            let year = ours.globals.start_year();
            let today = NaiveDate::from_ymd_opt(2026, 7, 6).unwrap();
            let renewed = |r: &Records| -> Vec<(u16, u16)> {
                let mut v = Vec::new();
                for c in 1..0x75usize {
                    if before.countries.kind(c) != 2 {
                        continue;
                    }
                    for t in 0..before.countries.team_count(c).max(0) as u16 {
                        let old = before.team(c as u16, t).unwrap().players();
                        for id in r.team(c as u16, t).unwrap().players() {
                            let same = before.player(id).zip(r.player(id)).is_some_and(|(o, x)| o.first_name() == x.first_name() && o.birthday() == x.birthday());
                            if !old.contains(&id) && !same {
                                v.push((c as u16, id));
                            }
                        }
                    }
                }
                v
            };
            let (mine, their) = (renewed(&ours), renewed(&theirs));
            made += mine.len();
            theirs_made += their.len();
            for &(country, id) in &mine {
                let p = ours.player(id).unwrap();
                assert_eq!(p.strength(), 1, "player {id}");
                assert!((19..=22).contains(&crate::statistics::age_on(p, today)), "player {id}'s age");
                assert_eq!(u16::from(p.nation()), country, "player {id}'s nation");
                assert!((3..=4).contains(&(i32::from(p.contract() & 0xfff) - i32::from(year))), "player {id}'s years");
                assert_eq!(p.retiring(), 8);
                assert_eq!(p.club()[0], country as u8);
            }
        }
        assert!(made * 10 >= theirs_made * 8 && made * 10 <= theirs_made * 12, "{made} new against {theirs_made}");
    }

    /// Over the original's turn the played countries' computer clubs fill
    /// their thin positions about as the original's do: over four turns as
    /// many new youngsters within a tenth (one turn 95 against 91 on the
    /// pair), as many of their own amateurs moved up within a fifth, every
    /// youngster 18 or 19 on two years' terms with the club's nation.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_played_clubs_fill_their_squads_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (theirs, before) = (load("NewSeason000.an3"), load("SeasonEnd364.an3"));
        let data = crate::data::Data::load(&dir).unwrap();
        let exe = a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap();
        let tables = crate::league::LeagueTables::load(&exe).unwrap();
        // Four turns of the season, the counts summed: one draw's count
        // swings with every other draw of the turn.
        let (mut fresh_sum, mut promoted_sum, mut their_fresh_sum, mut their_promoted_sum) = (0, 0, 0, 0);
        for seed in [3, 4, 5, 6] {
            let mut ours = load("SeasonEnd364.an3");
            let mut random = Random::new(seed);
            let drawn = crate::promotion::draw(&mut ours, &mut random);
            crate::promotion::count(&mut ours);
            crate::development::develop_played(&mut ours, false, &mut random);
            crate::rollover::retire_announced(&mut ours, &mut random);
            crate::statistics::archive_leagues(&mut ours);
            crate::promotion::flag(&mut ours, &data.rules.forecast.tickets);
            crate::rollover::count_first_seasons(&mut ours);
            let agents = crate::free_agents::release(&mut ours, &mut random);
            crate::deals::execute(&mut ours, &mut random);
            crate::rollover::close_played(&mut ours, &mut random);
            crate::rollover::close_kept(&mut ours, &mut random);
            crate::free_agents::place(&mut ours, &agents, (&data.rules.contract, &data.rules.value), &mut random);
            crate::settlement::settle_season(&mut ours, &tables);
            crate::promotion::take(&mut ours, &drawn);
            crate::settlement::open_season_with(&mut ours, &mut random, Some(&data.rules));
            let year = i32::from(ours.globals.start_year());
            let today = NaiveDate::from_ymd_opt(2026, 7, 6).unwrap();
            let count = |r: &Records| {
                let (mut fresh, mut promoted, mut ids) = (0, 0, Vec::new());
                for c in crate::opponent::played_nations(&before) {
                    for t in 0..before.countries.team_count(usize::from(c)).max(0) as u16 {
                        let old = before.team(c, t).unwrap().players();
                        for id in r.team(c, t).unwrap().players() {
                            if old.contains(&id) {
                                continue;
                            }
                            let (o, p) = (before.player(id).unwrap(), r.player(id).unwrap());
                            if o.first_name() != p.first_name() || o.birthday() != p.birthday() {
                                fresh += 1;
                                ids.push((c, id));
                            } else if o.club() == p.club() {
                                promoted += 1;
                            }
                        }
                    }
                }
                (fresh, promoted, ids)
            };
            let ((fresh, promoted, ids), (their_fresh, their_promoted, _)) = (count(&ours), count(&theirs));
            fresh_sum += fresh;
            promoted_sum += promoted;
            their_fresh_sum += their_fresh as i32;
            their_promoted_sum += their_promoted as i32;
            for (country, id) in ids {
                let p = ours.player(id).unwrap();
                assert!((18..=19).contains(&crate::statistics::age_on(p, today)), "player {id}'s age");
                assert_eq!(i32::from(p.contract() & 0xfff) - year, 2, "player {id}'s years");
                assert_eq!(u16::from(p.nation()), country);
            }
        }
        assert!((fresh_sum - their_fresh_sum).abs() * 10 <= their_fresh_sum, "{fresh_sum} new against {their_fresh_sum}");
        assert!((promoted_sum - their_promoted_sum).abs() * 5 <= their_promoted_sum + 8, "{promoted_sum} up against {their_promoted_sum}");
    }
}
