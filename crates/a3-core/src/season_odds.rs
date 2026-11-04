//! The season's odds, set as the new season opens (`FUN_0065e659`'s
//! `FUN_0077650c`, `FUN_0077655c` for each played country).
//!
//! Every club of a played country keeps its squad's strength anew
//! (`FUN_006a10a0`, team +0xc0, after its squad is filled, lifted and
//! lined up), and the first league's clubs take the odds of the
//! championship (the table at `DAT_00b217c0` + slot × 0x90): their
//! strengths divided by 1.7 and ordered from the strongest down, the
//! favourite at 1.5 to 3.0 by its lead over the second (more than 15, 10
//! or 5 apart), every other by how near the favourite it comes — less
//! than half its strength 200, less than six tenths 100, less than seven
//! 50, else its place and a tenth of the favourite's odds over the cube of
//! the share, 5.0 at most.
//!
//! Left out: the squads' market value at club +0x4f0, which the club
//! keeps for itself.

use crate::data::Data;
use crate::records::Records;

/// The block of the championship's odds (`DAT_00b217c0`): for each slot
/// 24 entries of four bytes — the country, the club's number at +2 —
/// then their odds in tenths at +0x60.
const ODDS_BLOCK: &str = "0xb1f860";
const ODDS_AT: usize = 0xb217c0 - 0xb1f860;
const ODDS_PAGE: usize = 0x90;
const ODDS_ENTRY: usize = 4;
const ODDS_OF: usize = 0x60;
const ODDS_MOST: usize = 24;
/// A strength's share of the odds (`DAT_00835120`).
const STRENGTH_SHARE: f32 = 1.7;
/// The favourite's odds by its lead over the second.
const FAVOURITE: [(i16, i32); 3] = [(15, 15), (10, 20), (5, 25)];
const FAVOURITE_CLOSE: i32 = 30;
/// The odds of a club far behind the favourite, by its share of the
/// favourite's strength.
const BEHIND: [(f64, i16); 3] = [(0.5, 2000), (0.6, 1000), (0.7, 500)];
const MOST_ODDS: f32 = 50.0;

/// The new season's odds of the played countries.
pub fn open_season(records: &mut Records, data: &Data) {
    for country in crate::opponent::played_nations(records) {
        keep_strengths(records, country, data);
        champion_odds(records, country);
    }
}

/// Every club's squad strength stored anew (`FUN_006a10a0`).
fn keep_strengths(records: &mut Records, country: u16, data: &Data) {
    for number in 0..records.countries.team_count(usize::from(country)).max(0) as u16 {
        let Some(team) = records.team(country, number) else { continue };
        let club = crate::load::club_state_of(records, team, data);
        let strength = crate::statistics::squad_strength(&data.rules.pitch, &club);
        if let Some(team) = records.team_mut(country, number) {
            team.set_squad_strength(strength);
        }
    }
}

/// The odds of the championship (`FUN_0077655c`'s last part).
fn champion_odds(records: &mut Records, country: u16) {
    let slot = usize::from(records.countries.slot(usize::from(country)));
    let mut clubs: Vec<(u16, i16)> = Vec::new();
    for number in 0..records.countries.team_count(usize::from(country)).max(0) as u16 {
        let Some(team) = records.team(country, number) else { continue };
        if team.level() == 0 {
            let share = (f64::from(team.squad_strength()) / f64::from(STRENGTH_SHARE)).trunc() as i16;
            clubs.push((number, share));
        }
    }
    // The strongest first, picked one after another; a pick of none
    // stronger than nought keeps the last.
    let mut order: Vec<(u16, i16)> = Vec::new();
    let mut pick = 0;
    let mut values: Vec<i16> = clubs.iter().map(|c| c.1).collect();
    for _ in 0..clubs.len() {
        let mut best = 0;
        for (k, &v) in values.iter().enumerate() {
            if best < v {
                best = v;
                pick = k;
            }
        }
        order.push((clubs[pick].0, values[pick]));
        values[pick] = -1;
    }
    let mut odds: Vec<i16> = order.iter().map(|o| o.1).collect();
    if let (Some(&top), Some(&second)) = (odds.first(), odds.get(1)) {
        let favourite = FAVOURITE.iter().find(|&&(lead, _)| i32::from(second) + i32::from(lead) < i32::from(top)).map_or(FAVOURITE_CLOSE, |f| f.1);
        let beyond = i32::from(top) + 1;
        odds[0] = favourite as i16;
        for (k, value) in odds.iter_mut().enumerate().skip(1) {
            let share = (f64::from(*value) / f64::from(beyond)) as f32;
            *value = match BEHIND.iter().find(|&&(below, _)| f64::from(share) < below) {
                Some(&(_, far)) => far,
                None => {
                    let share = f64::from(share);
                    let place = f64::from(k as i32 + 1 + favourite / 10);
                    let mut v = (place / (share * share * share)) as f32;
                    if v > MOST_ODDS {
                        v = MOST_ODDS;
                    }
                    (f64::from(v) * 10.0).trunc() as i16
                }
            };
        }
    } else if let Some(first) = odds.first_mut() {
        // A league of one: the favourite against none.
        *first = FAVOURITE_CLOSE as i16;
    }
    let Some(b) = records.block_mut(ODDS_BLOCK) else { return };
    let page = ODDS_AT + slot * ODDS_PAGE;
    for (k, (&(number, _), &value)) in order.iter().zip(&odds).take(ODDS_MOST).enumerate() {
        let at = page + k * ODDS_ENTRY;
        if let Some(e) = b.get_mut(at..at + ODDS_ENTRY) {
            e[0] = country as u8;
            e[2..4].copy_from_slice(&number.to_le_bytes());
        }
        let at = page + ODDS_OF + 2 * k;
        if let Some(e) = b.get_mut(at..at + 2) {
            e.copy_from_slice(&value.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The original's new season (NewSeason000) with its own squad
    /// strengths: every played country's odds of the championship come out
    /// as the original's — its clubs in the same order with the same odds.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_championship_odds_come_out_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut b, n) = (load("NewSeason000.an3"), load("NewSeason000.an3"));
        b.block_mut(ODDS_BLOCK).unwrap()[ODDS_AT..ODDS_AT + 5 * ODDS_PAGE].fill(0);
        let mut pages = 0;
        for country in crate::opponent::played_nations(&n) {
            champion_odds(&mut b, country);
            let slot = usize::from(n.countries.slot(usize::from(country)));
            let at = ODDS_AT + slot * ODDS_PAGE;
            let first = (0..n.countries.team_count(usize::from(country)).max(0) as u16)
                .filter(|&t| n.team(country, t).unwrap().level() == 0)
                .count();
            let (x, y) = (b.block(ODDS_BLOCK).unwrap(), n.block(ODDS_BLOCK).unwrap());
            for k in 0..first {
                let e = at + k * ODDS_ENTRY;
                assert_eq!((x[e], &x[e + 2..e + 4]), (y[e], &y[e + 2..e + 4]), "slot {slot} entry {k}");
            }
            let o = at + ODDS_OF;
            assert_eq!(x[o..o + 2 * first], y[o..o + 2 * first], "slot {slot} odds");
            pages += 1;
        }
        assert_eq!(pages, 5);
    }

    /// Over the same save the strengths kept anew from the squads agree
    /// with the original's stored ones at more than four clubs in five
    /// (the rest moved by squads changed after its store).
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_clubs_keep_their_strengths_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut b, n) = (load("NewSeason000.an3"), load("NewSeason000.an3"));
        let data = Data::load(&dir).unwrap();
        let (mut same, mut all) = (0, 0);
        for country in crate::opponent::played_nations(&n) {
            for number in 0..n.countries.team_count(usize::from(country)).max(0) as u16 {
                b.team_mut(country, number).unwrap().set_squad_strength(0);
            }
            keep_strengths(&mut b, country, &data);
            for number in 0..n.countries.team_count(usize::from(country)).max(0) as u16 {
                all += 1;
                same += i32::from(b.team(country, number).unwrap().squad_strength() == n.team(country, number).unwrap().squad_strength());
            }
        }
        assert!(all > 200 && same * 5 > all * 4, "{same} of {all}");
    }
}
