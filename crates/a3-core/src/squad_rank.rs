//! The clubs' rank by their squads within each league, as the new season
//! opens (`FUN_0065e659`'s `FUN_00778394`, `FUN_007782dc` for every league
//! of the main country and the first of each more one): the clubs ordered
//! by their squad's score (`FUN_0077811c`, the strongest first, an equal
//! one behind), each taking its rank at club +0x107 (team +0x130), the
//! league keeping the order (`FUN_00691148`, its +0x4d2).
//!
//! A squad's score (`FUN_007779e4`) counts its best two keepers, six
//! defenders, five midfielders and three strikers by their main position
//! and score (`FUN_00777544`: strength ten times, five for a scorer's
//! trait, one for each ability of the first eleven), ten times their
//! average.

use crate::records::Records;

const RANK_AT: usize = 0x130;
const ORDER_AT: usize = crate::opponent::LEAGUE_FIELDS + 0x4d2;
/// The positions' groups and how many of each count.
const GROUPS: [(u8, u8, usize); 4] = [(1, 1, 2), (2, 5, 6), (6, 9, 5), (10, 10, 3)];

/// A squad's score (`FUN_007779e4`).
pub fn squad_score(records: &Records, team: &crate::records::Team) -> i32 {
    let mut groups: [Vec<i32>; 4] = Default::default();
    for id in team.players() {
        let Some(p) = records.player(id) else { continue };
        let position = p.positions()[0];
        if let Some(g) = GROUPS.iter().position(|&(lo, hi, _)| (lo..=hi).contains(&position)) {
            groups[g].push(crate::season_odds::score(p));
        }
    }
    let (mut sum, mut count) = (0i16, 0i32);
    for (g, scores) in groups.iter_mut().enumerate() {
        // Highest first, an equal one behind the earlier (`FUN_00429cb0`).
        scores.sort_by(|a, b| b.cmp(a));
        for &v in scores.iter().take(GROUPS[g].2) {
            sum = sum.wrapping_add(v as i16);
            count += 1;
        }
    }
    if count < 1 { 1 } else { i32::from(sum) * 10 / count }
}

/// Every ranked league's clubs by their squads.
pub fn rank(records: &mut Records) {
    let main = u16::try_from(records.globals.main_country()).unwrap_or(0);
    let mut leagues: Vec<(u16, u8)> =
        (0..records.countries.leagues(usize::from(main)).max(0) as u8).map(|l| (main, l)).collect();
    for country in crate::opponent::played_nations(records) {
        if country != main {
            leagues.push((country, 0));
        }
    }
    for (country, level) in leagues {
        let Some(league) = crate::opponent::League::of(records, country, level) else { continue };
        let numbers: Vec<i16> = (0..league.teams()).map(|k| league.team(k)).collect();
        let mut scored: Vec<(i16, i32)> = numbers
            .iter()
            .map(|&n| (n, u16::try_from(n).ok().and_then(|n| records.team(country, n)).map_or(0, |t| squad_score(records, t))))
            .collect();
        // The exchange sort of `FUN_0077811c`.
        for i in 0..scored.len() {
            for j in i + 1..scored.len() {
                if scored[i].1 < scored[j].1 {
                    scored.swap(i, j);
                }
            }
        }
        let index = i32::from(records.countries.first_league(usize::from(country))) + i32::from(level);
        if let Some(b) = records.block_mut(&format!("0x1a09d48[{index}]")) {
            for (k, &(n, _)) in scored.iter().enumerate() {
                if let Some(e) = b.get_mut(ORDER_AT + k) {
                    *e = n as u8;
                }
            }
        }
        for (k, &(n, _)) in scored.iter().enumerate() {
            if let Some(team) = u16::try_from(n).ok().and_then(|n| records.team_mut(country, n)) {
                team.put::<u8>(RANK_AT, k as u8);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The original's NewSeason000 ranked again from its own squads: the
    /// ranks hold at two clubs in three — the original ranks before the new
    /// season fills and lifts its squads — England's first league, the
    /// least changed, at all but two;
    /// and every ranked league keeps its order beside the ranks.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_clubs_rank_by_their_squads_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut b, n) = (load("NewSeason000.an3"), load("NewSeason000.an3"));
        rank(&mut b);
        let (mut same, mut all) = (0, 0);
        for country in crate::opponent::played_nations(&n) {
            for t in 0..n.countries.team_count(usize::from(country)).max(0) as u16 {
                let (x, y) = (b.team(country, t).unwrap(), n.team(country, t).unwrap());
                if x.level() >= 10 {
                    continue;
                }
                all += 1;
                same += i32::from(x.get::<u8>(RANK_AT) == y.get::<u8>(RANK_AT));
            }
        }
        assert!(all > 140 && same * 3 >= all * 2 - 6, "{same} of {all}");
        let england = crate::opponent::League::of(&n, 16, 0).unwrap();
        let apart = (0..england.teams())
            .map(|k| england.team(k) as u16)
            .filter(|&t| b.team(16, t).unwrap().get::<u8>(RANK_AT) != n.team(16, t).unwrap().get::<u8>(RANK_AT))
            .count();
        assert!(apart <= 2, "{apart}");
        let index = n.countries.first_league(16);
        let order = b.block(&format!("0x1a09d48[{index}]")).unwrap();
        for k in 0..england.teams() {
            let t = u16::from(order[ORDER_AT + k]);
            assert_eq!(usize::from(b.team(16, t).unwrap().get::<u8>(RANK_AT)), k);
        }
    }
}
