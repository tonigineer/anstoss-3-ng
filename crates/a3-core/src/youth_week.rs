//! A computer club's youth through the season (`FUN_006c41b1`, which the
//! clubs' week on the market, `FUN_004297e1`, calls for each club of the
//! main and the more countries with no human, before its fresh terms):
//! outside June and July, a club that is no reserve team draws one in ten
//! thousand each week, and by the draw a youth of its youth team grows
//! (9000 and over), one is hurt for months (2500 to 2504), or, the richer
//! the club the likelier, it takes a new youth — three a season at most.
//!
//! Left out so far: the poaching between clubs (2600 to 2609, and 1000
//! for a club of the first two leagues: `FUN_006c38f7`, `FUN_006c3bab`)
//! and the club's amateur pool (the walker's tail from `FUN_0063e7cf`),
//! whose draws follow the first; and the human club's own walk with its
//! news (`FUN_005ef8cc`).

use crate::random::Random;
use crate::records::Records;

/// The months the youth rests (`_DAT_00b5cbdc`).
const SUMMER: [u16; 2] = [6, 7];
/// The week's draw and its bands.
const DRAW: u32 = 10_000;
const GROWS_FROM: u32 = 9000;
const HURT: std::ops::RangeInclusive<u32> = 2500..=2504;
const POACHED: std::ops::RangeInclusive<u32> = 2600..=2609;
/// A club takes a new youth while its support in these (`DAT_00a3b37c`)
/// is no less than the draw in fifties; a fifth more with the team flag
/// +0x95c's bit 1 (`FUN_0069f177`).
const SUPPORT_UNIT: i32 = 100_000;
const DRAW_UNIT: i32 = 50;
const FLAG_AT: usize = 0x95c;
const FLAG: u8 = 2;

/// One club's week for its youth (`FUN_006c41b1` with no news).
pub(crate) fn walk(records: &mut Records, (country, number): (u8, u16), random: &mut Random) {
    let [_, month, _] = records.globals.date();
    if SUMMER.contains(&month) {
        return;
    }
    let Some(team) = records.team(u16::from(country), number) else { return };
    if team.linked()[0] >= 0 {
        return;
    }
    let draw = random.below(DRAW);
    let support = team.get::<i32>(crate::newplayer::YOUTH_SUPPORT);
    let mut can = (support / SUPPORT_UNIT) as i16;
    if team.get::<u8>(FLAG_AT) & FLAG != 0 {
        can += can / 5;
    }
    if draw >= GROWS_FROM {
        grow(records, (country, number), support, random);
        return;
    }
    if HURT.contains(&draw) {
        hurt(records, (country, number), random);
        return;
    }
    if POACHED.contains(&draw) {
        return;
    }
    if i32::from(can) >= draw as i32 / DRAW_UNIT {
        take(records, (country, number), random);
    }
}

/// The support's bands a youth's growth is judged by (`FUN_006c4103`:
/// `DAT_00a3b36c`, `DAT_00a3b350`, `DAT_00a3b334`, `DAT_00a3b324`), and
/// per band the two marks a draw must come under (`DAT_008a59f8`).
const BANDS: [i32; 4] = [200_000, 400_000, 800_000, 1_400_000];
const MARKS: [[u32; 2]; 5] = [[35, 12], [50, 15], [70, 18], [90, 21], [110, 24]];
/// The draws' ranges by the team's +0x8ef (at most 3, `FUN_0069aa8b`)
/// and the youth's talent (`DAT_008a5a0c`), and the strength each talent
/// grows to (`DAT_008a7560`).
const RANGES: [[u32; 5]; 4] = [
    [400, 600, 800, 1000, 1200],
    [500, 750, 1000, 1250, 1500],
    [600, 900, 1200, 1500, 1800],
    [700, 1050, 1400, 1750, 2100],
];
const RANGES_AT: usize = 0x8ef;
const MOST_STRENGTH: [u8; 5] = [10, 9, 8, 7, 6];
/// A supported youth born from the first of July fourteen years before
/// the season's year (`FUN_00706943`) halves his draw, an older one takes
/// two thirds.
const SUPPORTED_AGE: u16 = 14;
const SEASON_MONTH: u8 = 7;

/// A youth of the club grows a point (`FUN_006c4103`, `FUN_006c3f34`):
/// each youth below his talent's most draws by his talent, and once more
/// if over the first mark; those under a mark are drawn from. Whether one
/// grew.
fn grow(records: &mut Records, (country, number): (u8, u16), support: i32, random: &mut Random) -> bool {
    let band = BANDS.iter().take_while(|&&b| support >= b).count();
    let Some(team) = records.team(u16::from(country), number) else { return false };
    let row = RANGES[usize::from(team.get::<u8>(RANGES_AT).min(3))];
    let year = records.globals.start_year();
    let (youths, _) = crate::youth::youths(records, country, number);
    let mut grown = Vec::new();
    for id in youths {
        let Some(p) = records.player(id) else { continue };
        let Some(talent) = usize::try_from(p.talent()).ok().filter(|&t| t < MOST_STRENGTH.len()) else { continue };
        if p.strength() >= MOST_STRENGTH[talent] {
            continue;
        }
        let mut r = random.below(row[talent]);
        if p.shown() & crate::youth::SUPPORTED != 0 {
            let young = (p.birth_year(), p.birth_month()) >= (year.wrapping_sub(SUPPORTED_AGE), SEASON_MONTH);
            r = if young { r >> 1 } else { (r << 1) / 3 };
        }
        if r < MARKS[band][0] || random.below(row[talent]) < MARKS[band][1] {
            grown.push(id);
        }
    }
    if grown.is_empty() {
        return false;
    }
    let id = grown[random.below(grown.len() as u32) as usize];
    if let Some(p) = records.player_mut(id) {
        p.set_strength(p.strength().wrapping_add(1));
    }
    true
}

/// A youth hurt: months out, his kind 1 (`FUN_006c3706`, `FUN_00766b92`).
const HURT_KIND: u8 = 1;
const HURT_MONTHS: u32 = 4;
const HURT_FROM: u32 = 3;
const MONTH: i32 = 30;
/// R+0x26f's bit the youth loses with it.
const HURT_CLEARS: u8 = 4;

/// One of the youth team drawn, unless already out, is out three to six
/// months (`FUN_006c3706`).
fn hurt(records: &mut Records, (country, number): (u8, u16), random: &mut Random) {
    let (youths, _) = crate::youth::youths(records, country, number);
    if youths.is_empty() {
        return;
    }
    let id = youths[random.below(youths.len() as u32) as usize];
    let Some(p) = records.player(id) else { return };
    if crate::load::days_to(&records.globals, p.injured_to()) != 0 {
        return;
    }
    let days = (random.below(HURT_MONTHS) + HURT_FROM) as i32 * MONTH;
    let today = i32::from(records.globals.day());
    crate::training_day::injure(records, id, HURT_KIND, days, today);
    if let Some(p) = records.player_mut(id) {
        p.set_retiring(p.retiring() & !HURT_CLEARS);
    }
}

/// A new youth while the club has made under three this season
/// (`FUN_006c3500`, `FUN_0066c0c2`).
fn take(records: &mut Records, (country, number): (u8, u16), random: &mut Random) -> bool {
    let made = crate::youth::made(records, country, number);
    if made >= crate::youth::MOST_MADE {
        return false;
    }
    if crate::newplayer::youth_for(records, (country, number), random).is_none() {
        return false;
    }
    crate::youth::set_made(records, country, number, made + 1);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// From the original's save A (8 September) through the season's
    /// weeks to its end (the 37 outside June and July): the computer clubs
    /// of the played countries take about as many youths as the
    /// original's did — 163 more made at its computer clubs, their youth
    /// teams 621 strong at the end, 33 clubs at three —, the youths kept
    /// from A grow about as much as there, and a few are hurt for months.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_youths_grow_through_the_season_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (a, theirs) = (load("A.an3"), load("SeasonEnd364.an3"));
        let countries = crate::opponent::played_nations(&a);
        let clubs: Vec<(u8, u16)> = countries
            .iter()
            .flat_map(|&c| (0..a.countries.team_count(usize::from(c)).max(0) as u16).map(move |n| (c as u8, n)))
            .filter(|&(c, n)| a.team(u16::from(c), n).is_some_and(|t| t.humans() == [0, 0]))
            .collect();
        let tally = |r: &Records| {
            let (mut youths, mut made, mut full, mut strength, mut kept) = (0, 0, 0, 0, 0);
            for &(c, n) in &clubs {
                let (ids, _) = crate::youth::youths(r, c, n);
                youths += ids.len();
                let m = crate::youth::made(r, c, n);
                made += usize::from(m);
                full += usize::from(m == crate::youth::MOST_MADE);
                let (before, _) = crate::youth::youths(&a, c, n);
                for id in ids.into_iter().filter(|id| before.contains(id)) {
                    kept += 1;
                    strength += i32::from(r.player(id).unwrap().strength()) - i32::from(a.player(id).unwrap().strength());
                }
            }
            (youths, made, full, f64::from(strength) / f64::from(kept))
        };
        let (mut ours, mut hurt) = (load("A.an3"), 0);
        let mut random = Random::new(5);
        for _ in 0..37 {
            for &club in &clubs {
                walk(&mut ours, club, &mut random);
            }
        }
        for &(c, n) in &clubs {
            for id in crate::youth::youths(&ours, c, n).0 {
                let p = ours.player(id).unwrap();
                hurt += usize::from(p.injury_kind() == HURT_KIND && crate::load::days_to(&ours.globals, p.injured_to()) >= 90);
            }
        }
        let (youths, made, full, grown) = tally(&ours);
        let (their_youths, their_made, their_full, their_grown) = tally(&theirs);
        let (_, made_at_a, _, _) = tally(&a);
        let near = |x: f64, y: f64, by: f64| (x / y - 1.0).abs() < by;
        assert!(near((made - made_at_a) as f64, (their_made - made_at_a) as f64, 0.2), "made {made} against {their_made}");
        assert!(near(youths as f64, their_youths as f64, 0.05), "youths {youths} against {their_youths}");
        assert!(near(full as f64, their_full as f64, 0.5), "full {full} against {their_full}");
        assert!(near(grown, their_grown, 0.25), "grown {grown:.2} against {their_grown:.2}");
        assert!((1..12).contains(&hurt), "{hurt}");
        // A new youth stands in his club's youth team with a youth's
        // station of his class.
        let (c, n) = clubs.iter().copied().find(|&(c, n)| crate::youth::made(&ours, c, n) > crate::youth::made(&a, c, n)).unwrap();
        let (ids, _) = crate::youth::youths(&ours, c, n);
        let id = *ids.last().unwrap();
        let p = ours.player(id).unwrap();
        assert_eq!(p.club(), [c, n as u8]);
        let at = 0x20 + (p.career_count() as usize - 1) * 0x14;
        assert_eq!(p.get::<u8>(at + 0x12), 9 + crate::youth::class(&ours, id) as u8);
        assert_eq!(p.get::<u8>(at + 0x13), p.strength() + 0x40);
        assert!((1..=5).contains(&p.strength()) && p.salary() == 0 && p.retiring() == 8);
    }
}
