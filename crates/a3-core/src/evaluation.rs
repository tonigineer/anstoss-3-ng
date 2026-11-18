//! What the managers are worth, as the comparison of the players and
//! their clubs asks it (dialog 211, ERFOLGE.md §A.6): nine marks apiece,
//! each weighed by the game's difficulty (`FUN_006dda52`).

use chrono::NaiveDate;

use crate::{
    autolineup, finances, league,
    records::{self, Records, Team},
    stock,
};

/// The unit the marks are scaled by (`DAT_00a3b3bc`).
const UNIT: i32 = 10_000;
/// The age a player counts old at in the squad's marks (`FUN_006dd041`).
const OLD: i32 = 33;
/// More than three of them and every further old one costs the squad.
const OLD_MOST: i32 = 3;
/// A player young for his strength adds this.
const YOUNG_BONUS: i32 = 20;
/// How far his age may stay above his strength and still count young.
const YOUNG_WITHIN: u32 = 16;

/// How much the game counts the abilities it asks of a player (0x8a6718)
/// and the traits it asks of his character (0x8a6738), a bit each, and
/// how much a manager stands for by where he ranks (0x8a6758).
const ABILITY_POINTS: [i16; 16] = [2, 2, 0, 0, 0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 0];
const TRAIT_POINTS: [i16; 16] = [-2, 0, 2, -2, -2, -2, 2, -2, 1, -2, -2, 0, 0, 0, 0, 0];
const STANDING_POINTS: [i16; 16] = [
    -200, -100, 0, 100, 200, 400, 600, 800, 1000, 1250, 1500, 1750, 2000, 2500, 3000, 4000,
];

/// The nine marks of one manager (`FUN_006dda52`), in the order dialog
/// 211 asks them: his money, the contract he is on, his shares, how his
/// team has gone since he came, what the youth brought, what he did for
/// the club, how the season runs, what he has won, and what he stands
/// for. The last of them is his standing; the screen shows it beside the
/// mark in brackets.
/// `human` is his number among the humans, 1 to 4: his career record and
/// his portfolio stand one block after the last human's (`FUN_006a426b`,
/// which takes the manager with that number).
pub fn marks(records: &Records, manager: &records::Manager, human: usize) -> [i32; 9] {
    let mut out = [0; 9];
    out[0] = adjust(manager, manager.cash() / UNIT);
    let shares = stock::portfolio_value(records, i16::try_from(human).unwrap_or(0));
    out[2] = adjust(manager, (shares / i64::from(UNIT)) as i32);
    if manager.country() != 0 {
        let team = records.team(manager.country(), manager.team());
        if let Some(team) = team {
            let (base, per_team) = finances::TARGET_POINTS[target_at(team)];
            let size = league::teams(records, team.country(), team.level());
            let pay = manager.pay_point() * (base + per_team * (size - 10)) + manager.pay_fixed();
            out[1] = adjust(manager, pay / 2 / UNIT);
            out[3] = adjust(manager, team_marks(records, team) - manager.team_start());
        }
        out[4] = adjust(manager, manager.youth_marks());
        out[5] = adjust(manager, manager.service_marks());
        out[6] = adjust(manager, manager.season_marks());
    }
    let trainer = records
        .career(human.wrapping_sub(1))
        .and_then(|c| c.get(0x42..0x44))
        .map_or(0, |two| i16::from_le_bytes([two[0], two[1]]));
    out[7] = adjust(manager, manager.honour_marks()) + i32::from(trainer) * 5;
    let standing = usize::try_from(manager.standing()).unwrap_or(0).min(STANDING_POINTS.len() - 1);
    out[8] = adjust(manager, i32::from(STANDING_POINTS[standing]))
        + adjust(manager, manager.standing_marks());
    out
}

/// How the game weighs a mark by its difficulty (`FUN_006ddc45`): four
/// times the mark, less a quarter of `difficulty − 2` times it — so the
/// plainest difficulty leaves four times the mark, and an easier one
/// more.
fn adjust(manager: &records::Manager, x: i32) -> i32 {
    let difficulty = i32::from(manager.difficulty() & 0x7f);
    x.wrapping_mul(4)
        .wrapping_sub((difficulty - 2).wrapping_mul(x).wrapping_div(4))
}

/// The target the club plays for this season, as an index into the
/// target tables (the team record's +0x10c).
fn target_at(team: &Team) -> usize {
    usize::try_from(team.get::<i8>(finances::TARGET))
        .unwrap_or(0)
        .min(finances::TARGET_POINTS.len() - 1)
}

/// What a team is worth to its manager (`FUN_006dd041`): what the squad
/// can do, what its respect, will and mood come to, and how fit and how
/// fresh its best players are.
fn team_marks(records: &Records, team: &Team) -> i32 {
    let [day, month, year] = records.globals.date();
    let today = NaiveDate::from_ymd_opt(i32::from(year), u32::from(month), u32::from(day));
    let mut sum = 0;
    let mut old = 0;
    for id in team.players() {
        let Some(player) = records.player(id) else { continue };
        sum += player_marks(player);
        let Some(today) = today else { continue };
        let years = autolineup::age(player.birthday(), today);
        if years >= OLD {
            old += 1;
            if old > OLD_MOST {
                sum -= 10;
            }
        } else {
            let strength = i32::from(player.strength());
            if ((years - strength) as u32) < YOUNG_WITHIN {
                sum += YOUNG_BONUS;
            }
        }
    }
    i32::from(team.respect()) / 10
        + sum
        - 60
        + i32::from(team.will()) / 10
        + i32::from(team.mood()) / 10
        + average(records, team, freshness_points) / 5
        + average(records, team, condition_points) / 5
}

/// What one player can do (`FUN_006dd197`): his strength ten times over,
/// plus a point for every ability he is good at and what his traits are
/// each worth.
fn player_marks(player: &records::Player) -> i32 {
    let mut sum = i32::from(player.strength()) * 10;
    let traits = player.traits();
    for bit in 0..16 {
        if player.good() & (1 << bit) != 0 {
            sum += 1;
        }
        if traits[0] & (1 << bit) != 0 {
            sum += i32::from(ABILITY_POINTS[bit]);
        }
        if traits[1] & (1 << bit) != 0 {
            sum += i32::from(TRAIT_POINTS[bit]);
        }
    }
    sum
}

/// How fit a player looks to the game (`FUN_00457e00`): his condition
/// in hundreds; an injured one with five or more points loses five, and
/// a badly injured one counts as nothing.
fn condition_points(player: &records::Player) -> i32 {
    let points = i32::from(player.condition()) / 100;
    if player.shown() & 0x800 == 0 {
        points
    } else if points < 5 {
        0
    } else {
        points - 5
    }
}

/// How fresh a player looks (`FUN_006ff649`): his three freshness
/// figures in hundreds, so at most 100; no figure at all counts as
/// none, which the game turns into a wild negative.
fn freshness_points(player: &records::Player) -> i32 {
    let [one, two, three] = player.freshness();
    let sum = i32::from(one) + i32::from(two) + i32::from(three);
    if sum < 0 {
        -0x1_0000
    } else if sum >= 10_001 {
        100
    } else {
        sum / 100
    }
}

/// The team's average of a player's figure, from the records: over the
/// eleven and the bench, or the squad if neither has anyone, or 50
/// (`FUN_006771b8`, `FUN_00676f16` — averages over the club's players).
fn average(records: &Records, team: &Team, of: fn(&records::Player) -> i32) -> i32 {
    let value = |id: u16| records.player(id).map_or(0, of);
    let picked: Vec<i32> =
        team.lineup().into_iter().chain(team.bench()).filter(|&id| id != 0).map(value).collect();
    if !picked.is_empty() {
        return picked.iter().sum::<i32>() / picked.len() as i32;
    }
    let squad = team.players();
    if squad.is_empty() {
        return 50;
    }
    squad.iter().map(|&id| value(id)).sum::<i32>() / squad.len() as i32
}


/// The season records' table (block `0x1ba9f38`, ERFOLGE.md §A.4): ten
/// entries of 0x40 — the name (0x34 bytes), the points (i32), the club's
/// country (u8) and number (u16), the date (day, month, year u16) — and
/// behind them, at +0x280, each human's sum of last season's end by his
/// number.
const SEASON_RECORDS: &str = "0x1ba9f38";
const ENTRY: usize = 0x40;
const ENTRIES: usize = 10;
const ENTRY_POINTS: usize = 0x34;
const ENTRY_COUNTRY: usize = 0x38;
const ENTRY_NUMBER: usize = 0x3a;
const ENTRY_DATE: usize = 0x3c;
const LAST_SUMS: usize = 0x280;

/// The humans' season end in the comparison (`FUN_00624ad4`'s first two
/// calls, for every human by his number, at the head of `FUN_00624e4f`):
/// his marks folded ([`fold`]), then his season's points
/// ([`season_points`]). `today` is the PC's clock date the table's entry
/// takes (`FUN_006d4d20`'s `GetSystemTime`): day, month, year.
pub fn close_season(records: &mut Records, data: &crate::data::Data, today: (u8, u8, u16)) {
    for n in 0..usize::from(records.globals.humans()) {
        fold(records, data, n);
        season_points(records, n, today);
    }
}

/// The season's marks into what he has won, and what his squad lost or
/// gained in strength since it was last kept into what he did for the
/// club, ten for a point (`FUN_006dd2fe`: +0xd4 += +0xd0, +0xd0 = 0, and
/// with a club +0xcc += (`FUN_006a10bb` − club +0x9c) · 10).
fn fold(records: &mut Records, data: &crate::data::Data, n: usize) {
    let Some(m) = records.managers.get_mut(n) else { return };
    m.set_honour_marks(m.honour_marks().wrapping_add(m.season_marks()));
    m.set_season_marks(0);
    let (country, number) = (m.country(), m.team());
    if country == 0 {
        return;
    }
    let Some(team) = records.team(country, number) else { return };
    let club = crate::load::club_state_of(records, team, data);
    let now = crate::statistics::squad_strength(&data.rules.pitch, &club);
    let by = (i32::from(now) - i32::from(team.squad_strength())).wrapping_mul(10);
    let m = &mut records.managers[n];
    m.set_service_marks(m.service_marks().wrapping_add(by));
}

/// A human's points of the season (`FUN_006d6264`): the sum of his nine
/// marks less what his standing alone stands for (0x8a6758), less that
/// same sum at last season's end (T +0x280 by his number), written to
/// his record's +0x1f74 (`FUN_006a5580`, a word). At or above the tenth
/// entry's points they go into the table, sorted, above the entries of
/// as many — "first last" (0x8a652c), his club, the date. The sum stays
/// for next season.
fn season_points(records: &mut Records, n: usize, today: (u8, u8, u16)) {
    let number = n + 1;
    let Some(m) = records.managers.get(n) else { return };
    let total = marks(records, m, number).iter().fold(0i32, |s, &x| s.wrapping_add(x));
    let standing = usize::try_from(m.standing()).unwrap_or(0).min(STANDING_POINTS.len() - 1);
    let sum = total.wrapping_sub(i32::from(STANDING_POINTS[standing]));
    let word = |at: usize| (at..at + 0x12).map(|i| m.get::<u8>(i)).take_while(|&b| b != 0);
    let mut name: Vec<u8> = word(0).take(9).collect();
    name.push(b' ');
    name.extend(word(0xa).take(17));
    let club = (m.country() as u8, m.team());
    let Some(block) = records.block(SEASON_RECORDS).filter(|b| b.len() >= LAST_SUMS + 4 * (number + 1)) else {
        return;
    };
    let at_last = LAST_SUMS + 4 * number;
    let last = i32::from_le_bytes(block[at_last..at_last + 4].try_into().unwrap());
    let points = sum.wrapping_sub(last);
    records.managers[n].put::<i16>(crate::national::SEASON_POINTS, points as i16);
    let block = records.block_mut(SEASON_RECORDS).unwrap();
    let points_of = |b: &[u8], i: usize| {
        i32::from_le_bytes(b[i * ENTRY + ENTRY_POINTS..i * ENTRY + ENTRY_POINTS + 4].try_into().unwrap())
    };
    if points_of(block, ENTRIES - 1) <= points {
        let mut i = ENTRIES - 1;
        while i > 0 && points_of(block, i - 1) <= points {
            block.copy_within((i - 1) * ENTRY..i * ENTRY, i * ENTRY);
            i -= 1;
        }
        let e = &mut block[i * ENTRY..(i + 1) * ENTRY];
        e[..name.len()].copy_from_slice(&name);
        e[name.len()] = 0;
        e[ENTRY_POINTS..ENTRY_POINTS + 4].copy_from_slice(&points.to_le_bytes());
        e[ENTRY_DATE] = today.0;
        e[ENTRY_DATE + 1] = today.1;
        e[ENTRY_DATE + 2..ENTRY_DATE + 4].copy_from_slice(&today.2.to_le_bytes());
        e[ENTRY_COUNTRY] = club.0;
        e[ENTRY_NUMBER..ENTRY_NUMBER + 2].copy_from_slice(&club.1.to_le_bytes());
    }
    block[at_last..at_last + 4].copy_from_slice(&sum.to_le_bytes());
}

#[cfg(test)]
mod saves {
    use super::*;
    use a3_files::an3::Save;

    /// The original's turn from SeasonEnd364 to NewSeason000 (one human,
    /// number 1, standing 9): the season end's marks moved first
    /// (`FUN_0041786a`, sixty for his place) and his squad's strength
    /// eleven under the kept one as the original's season end left it, the
    /// fold leaves what he has won at NewSeason000's 104, and his season's
    /// points come out at its 6009 — the sum 7259 less his standing's 1250
    /// — kept in his record and the table's sums, and entered first in the
    /// season records with his name, his club and the date given.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_season_end_counts_the_human_s_season_points_as_the_original() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        if !keeps.join("SeasonEnd364.an3").exists() || !keeps.join("NewSeason000.an3").exists() {
            eprintln!("no SeasonEnd364 or NewSeason000 among the saves");
            return;
        }
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs) = (load("SeasonEnd364.an3"), load("NewSeason000.an3"));
        let data = crate::data::Data::load(&dir).unwrap();
        crate::standing::season_end(&mut ours);
        let (country, number) = (ours.managers[0].country(), ours.managers[0].team());
        let team = ours.team(country, number).unwrap();
        let now = crate::statistics::squad_strength(&data.rules.pitch, &crate::load::club_state_of(&ours, team, &data));
        ours.team_mut(country, number).unwrap().set_squad_strength(now + 11);
        close_season(&mut ours, &data, (18, 11, 2026));
        let (x, y) = (&ours.managers[0], &theirs.managers[0]);
        assert_eq!((x.honour_marks(), x.season_marks()), (y.honour_marks(), 0));
        assert_eq!(x.get::<i16>(crate::national::SEASON_POINTS), 6009);
        assert_eq!(y.get::<i16>(crate::national::SEASON_POINTS), 6009);
        let (a, b) = (ours.block(SEASON_RECORDS).unwrap(), theirs.block(SEASON_RECORDS).unwrap());
        assert_eq!(a[LAST_SUMS..], b[LAST_SUMS..], "the sums kept by number");
        assert_eq!(a[..ENTRY_DATE], b[..ENTRY_DATE], "name, points and club as the original's");
        assert_eq!(a[ENTRY_DATE..ENTRY_DATE + 4], [18, 11, 0xea, 0x07]);
        assert_eq!(a[ENTRY..LAST_SUMS], b[ENTRY..LAST_SUMS], "the other entries empty");
        let marks = ours.season_marks();
        assert_eq!((marks[0].name.as_str(), marks[0].points, marks[0].club), ("Toni Lubiniecki", 6009, (14, 84)));
        // A second season's end counts only what the sum moved since.
        let before = i32::from_le_bytes(a[LAST_SUMS + 4..LAST_SUMS + 8].try_into().unwrap());
        let youth = ours.managers[0].youth_marks();
        ours.managers[0].set_youth_marks(youth + 100);
        let team = ours.team(country, number).unwrap();
        let now = crate::statistics::squad_strength(&data.rules.pitch, &crate::load::club_state_of(&ours, team, &data));
        ours.team_mut(country, number).unwrap().set_squad_strength(now);
        close_season(&mut ours, &data, (1, 7, 2027));
        assert_eq!(ours.managers[0].get::<i16>(crate::national::SEASON_POINTS), 450);
        let after = ours.block(SEASON_RECORDS).unwrap();
        assert_eq!(i32::from_le_bytes(after[LAST_SUMS + 4..LAST_SUMS + 8].try_into().unwrap()), before + 450);
        let marks = ours.season_marks();
        assert_eq!(marks.iter().map(|m| m.points).collect::<Vec<_>>(), [6009, 450]);
    }
}
