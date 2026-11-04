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
//! The top scorers' odds (in the offer `0x1c65e88`, by slot) go to the
//! first league's strikers by their strength, a scorer's trait and their
//! abilities, the trainers' to the first league's trainers by their
//! ability or standing, the board's confidence and its byte.
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

/// The season's offer (`0x1c65e88`, its data from 0x1c65ea8): for each
/// slot the top scorers — an i32 count, 50 ids as i32 from +4, their first
/// and last names from +0xcc and +0x130, their odds as i32 from +0x194 —
/// every 0x25c bytes, then from +0xbcc the trainers every 0x192 bytes — an
/// i16 count, 50 i16 ids (a human's his number negated), the clubs'
/// countries and numbers from +0x66 and +0xca, the odds from +0x12e.
const OFFER: &str = "0x1c65e88";
const SCORERS: usize = 0;
const SCORER_PAGE: usize = 0x25c;
const SCORER_NAMES: [usize; 2] = [0xcc, 0x130];
const SCORER_ODDS: usize = 0x194;
const SCORERS_MOST: usize = 50;
const TRAINERS: usize = 0xbcc;
const TRAINER_PAGE: usize = 0x192;
const TRAINER_CLUBS: [usize; 2] = [0x66, 0xca];
const TRAINER_ODDS: usize = 0x12e;
const TRAINERS_MOST: usize = 49;
/// A striker's main position; how a scorer's score is reckoned and its
/// odds by it (`FUN_00777544`, `FUN_007776c9`).
const STRIKER: u8 = 10;
const ABILITIES_COUNTED: u16 = 0x7ff;
const SCORER_TRAIT: [u16; 2] = [0x101, 1];
const SCORE_TOP: i32 = 130;
const SCORE_STEEP: i32 = 120;
const SCORER_SPREAD: f32 = 1.4;
const LEAST_ODDS: i32 = 11;
/// What a computer trainer's odds read: his person's ability (+0x24).
const TRAINER_ABILITY: usize = 0x24;
const LEAST_TRAINER_ODDS: i32 = 11;
const TRAINER_ODDS_FLOOR: i16 = 10;

/// The new season's odds of the championship, once the squads stand.
pub fn open_season(records: &mut Records, data: &Data) {
    for country in crate::opponent::played_nations(records) {
        keep_strengths(records, country, data);
        champion_odds(records, country);
    }
}

/// The top scorers' and the trainers' odds of the played countries, as
/// their players open the season — before the managers' year moves the
/// boards' confidence (`FUN_00775219` comes after).
pub fn open_lists(records: &mut Records, random: &mut crate::random::Random) {
    for country in crate::opponent::played_nations(records) {
        scorer_odds(records, country);
        trainer_odds(records, country, random);
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

/// A striker's score (`FUN_00777544`): his strength ten times, five more
/// for a scorer's trait, one for each ability of the first eleven.
fn score(player: &crate::records::Player) -> i32 {
    let traits = player.traits();
    let mut score = i32::from(player.strength()) * 10;
    if traits[1] & SCORER_TRAIT[1] != 0 || traits[0] & SCORER_TRAIT[0] != 0 {
        score += 5;
    }
    score + (player.good() & ABILITIES_COUNTED).count_ones() as i32
}

/// The top scorers' odds (`FUN_007776c9`): the strikers of the first
/// league's clubs, the fifty best by their score (`FUN_00429cb0`), their
/// odds in tenths by how far below 130 they score — over its cube when
/// they average 120 at most, straight beyond — 1.1 at least.
fn scorer_odds(records: &mut Records, country: u16) {
    let slot = usize::from(records.countries.slot(usize::from(country)));
    let clubs: Vec<i16> = crate::opponent::League::of(records, country, 0)
        .map_or_else(Vec::new, |l| (0..l.teams()).map(|k| l.team(k)).collect());
    let mut ids = [0u16; SCORERS_MOST];
    let mut scores = [0i32; SCORERS_MOST];
    let mut count = 0usize;
    for number in clubs {
        let Some(team) = u16::try_from(number).ok().and_then(|n| records.team(country, n)) else { continue };
        for id in team.players() {
            let Some(player) = records.player(id) else { continue };
            if player.positions()[0] != STRIKER {
                continue;
            }
            let value = score(player);
            let mut k = SCORERS_MOST as i32 - 2;
            while k >= 0 && scores[k as usize] < value {
                scores[k as usize + 1] = scores[k as usize];
                ids[k as usize + 1] = ids[k as usize];
                k -= 1;
            }
            scores[(k + 1) as usize] = value;
            ids[(k + 1) as usize] = id;
            count = (count + 1).min(SCORERS_MOST);
        }
    }
    let names: Vec<[u16; 2]> =
        ids[..count].iter().map(|&id| records.player(id).map_or([0; 2], |p| [p.first_name(), p.last_name()])).collect();
    let average = if count == 0 { 0 } else { scores[..count].iter().sum::<i32>() / count as i32 };
    let Some(b) = records.block_mut(OFFER) else { return };
    let page = SCORERS + slot * SCORER_PAGE;
    let Some(p) = b.get_mut(page..page + SCORER_PAGE) else { return };
    p.fill(0);
    p[..4].copy_from_slice(&(count as i32).to_le_bytes());
    for k in 0..count {
        p[4 + 4 * k..8 + 4 * k].copy_from_slice(&i32::from(ids[k]).to_le_bytes());
        for (at, name) in SCORER_NAMES.iter().zip(names[k]) {
            p[at + 2 * k..at + 2 * k + 2].copy_from_slice(&name.to_le_bytes());
        }
        let far = f64::from(((SCORE_TOP - scores[k]) as f64 / 10.0) as f32);
        let spread = f64::from(SCORER_SPREAD);
        let reach = if average <= SCORE_STEEP { far * far * far } else { far };
        let odds = ((reach * spread + 1.0) * 10.0).trunc() as i32;
        let odds = if odds < LEAST_ODDS { LEAST_ODDS } else { odds };
        p[SCORER_ODDS + 4 * k..SCORER_ODDS + 4 * k + 4].copy_from_slice(&odds.to_le_bytes());
    }
}

/// The trainers' odds (`FUN_007771ae`): every first-league club's
/// trainer — a computer club's manager-trainer or else its trainer, by his
/// ability, a human's by his standing and the board's confidence in him —
/// with the board's byte (team +0x123) three times over, four times the
/// sum; ten or less goes to 11 to 13 by chance; ordered from the lowest
/// odds, 49 at most (`FUN_0077f3bf`, 1.0 at least).
fn trainer_odds(records: &mut Records, country: u16, random: &mut crate::random::Random) {
    let slot = usize::from(records.countries.slot(usize::from(country)));
    // A club without a trainer leaves the last odds as they were.
    let mut odds = LEAST_TRAINER_ODDS;
    let mut entries = [(0i16, 0i16, 0i16, 0i16); TRAINERS_MOST + 1];
    let mut count = 0usize;
    for number in 0..records.countries.team_count(usize::from(country)).max(0) as u16 {
        let Some(team) = records.team(country, number) else { continue };
        if team.level() != 0 {
            continue;
        }
        let board = i32::from(team.board());
        let human = team.humans()[0];
        let [both, trainer, _] = team.staff();
        let id = if human == 0 {
            let id = if both as i16 >= 1 { both as i16 } else { trainer as i16 };
            if id > 0 {
                let ability = records.persons.get(id as usize).map_or(0, |p| i32::from(p.get::<i8>(TRAINER_ABILITY)));
                odds = (ability + 5 + board * 3) * 4;
            }
            id.max(0)
        } else {
            let index = usize::from(human) - 1;
            let standing = records.managers.get(index).map_or(0, |m| m.standing());
            let confidence = crate::stock::confidence(records, index);
            odds = (standing + 5 + board * 3 + (confidence - 65) / 5) * 4;
            -i16::from(human)
        };
        if odds < LEAST_TRAINER_ODDS {
            odds = random.below(3) as i32 + LEAST_TRAINER_ODDS;
        }
        if id == 0 {
            continue;
        }
        let odds = (odds as i16).max(TRAINER_ODDS_FLOOR);
        let mut k = count as i32 - 1;
        while k >= 0 && odds < entries[k as usize].3 {
            entries[k as usize + 1] = entries[k as usize];
            k -= 1;
        }
        if k < TRAINERS_MOST as i32 {
            entries[(k + 1) as usize] = (id, country as i16, number as i16, odds);
            if count < TRAINERS_MOST {
                count += 1;
            }
        }
    }
    let Some(b) = records.block_mut(OFFER) else { return };
    let page = TRAINERS + slot * TRAINER_PAGE;
    let Some(p) = b.get_mut(page..page + TRAINER_PAGE) else { return };
    p.fill(0);
    p[..2].copy_from_slice(&(count as i16).to_le_bytes());
    for (k, &(id, c, n, odds)) in entries.iter().enumerate().take(count) {
        p[2 + 2 * k..4 + 2 * k].copy_from_slice(&id.to_le_bytes());
        p[TRAINER_CLUBS[0] + 2 * k..TRAINER_CLUBS[0] + 2 * k + 2].copy_from_slice(&c.to_le_bytes());
        p[TRAINER_CLUBS[1] + 2 * k..TRAINER_CLUBS[1] + 2 * k + 2].copy_from_slice(&n.to_le_bytes());
        p[TRAINER_ODDS + 2 * k..TRAINER_ODDS + 2 * k + 2].copy_from_slice(&odds.to_le_bytes());
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

    /// Over the same save the top scorers' and the trainers' lists come
    /// out as the original's, page by page.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_scorers_and_trainers_odds_come_out_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut b, n, e) = (load("NewSeason000.an3"), load("NewSeason000.an3"), load("SeasonEnd364.an3"));
        let mut random = crate::random::Random::new(4);
        b.block_mut(OFFER).unwrap().fill(0);
        // The boards' confidence as it stood before the managers' year.
        for k in 0..usize::from(n.globals.humans()) {
            let m = e.managers.get(k).unwrap();
            let (c, u, w) = (m.confidence(), m.confidence_ups(), m.confidence_swings());
            let m = b.managers.get_mut(k).unwrap();
            m.set_confidence(c);
            m.set_confidence_ups(u);
            m.set_confidence_swings(w);
        }
        for country in crate::opponent::played_nations(&n) {
            scorer_odds(&mut b, country);
            trainer_odds(&mut b, country, &mut random);
            let slot = usize::from(n.countries.slot(usize::from(country)));
            let (x, y) = (b.block(OFFER).unwrap(), n.block(OFFER).unwrap());
            let at = SCORERS + slot * SCORER_PAGE;
            let count = i32::from_le_bytes(y[at..at + 4].try_into().unwrap()) as usize;
            assert_eq!(x[at..at + 4], y[at..at + 4], "slot {slot} scorers");
            for (from, width) in [(4, 4), (SCORER_NAMES[0], 2), (SCORER_NAMES[1], 2), (SCORER_ODDS, 4)] {
                let r = at + from..at + from + width * count;
                assert_eq!(x[r.clone()], y[r], "slot {slot} scorers from {from:#x}");
            }
            let at = TRAINERS + slot * TRAINER_PAGE;
            let count = i16::from_le_bytes(y[at..at + 2].try_into().unwrap()) as usize;
            for from in [0, 2, TRAINER_CLUBS[0], TRAINER_CLUBS[1], TRAINER_ODDS] {
                let r = at + from..at + from + if from == 0 { 2 } else { 2 * count };
                assert_eq!(x[r.clone()], y[r], "slot {slot} trainers from {from:#x}");
            }
        }
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
