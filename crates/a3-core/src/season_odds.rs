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
//! The betting office's season offer (`0x1c65e88`, cleared as the season
//! opens) takes each played country's top scorers and trainers as its
//! players open the season, before the managers' year; then the
//! tournaments' nations, the European cups' entrants and the cups' teams,
//! each list ordered from the shortest odds.
//!
//! (The squad's worth at club +0x4f0, the bank's overdraft, the players'
//! opening sets, [`crate::rollover::open_played`].)

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

/// The new season's odds of the championship, once the squads stand,
/// then the European cups' (`FUN_0041bb6c`) and the cups' as they are
/// drawn (`FUN_006cc3ff`).
pub fn open_season(records: &mut Records, data: &Data, random: &mut crate::random::Random) {
    let countries = crate::opponent::played_nations(records);
    for &country in &countries {
        keep_strengths(records, country, data);
        champion_odds(records, country);
    }
    nation_odds(records, &data.rules.pitch.european, random);
    european_odds(records);
    for &country in &countries {
        cup_odds(records, country);
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

/// The cups' teams from +0x13a6 every 0x102 bytes: an i16 count, 63
/// numbers, their odds from +0x82; the European cups' clubs at +0x18b0
/// (the Champions League) and +0x19a2 (the UEFA cup): an i16 count, 39
/// entries of four bytes — the country, the number at +2 — their odds from
/// +0xa2.
const CUP_TEAMS: usize = 0x13a6;
const CUP_PAGE: usize = 0x102;
const CUP_ODDS: usize = 0x82;
const CUP_MOST: usize = 63;
const EUROPEAN: [usize; 2] = [0x18b0, 0x19a2];
const EUROPEAN_ODDS: usize = 0xa2;
const EUROPEAN_MOST: usize = 39;
/// The season's flags at team +0x110: in the cup this season
/// (`FUN_0069a327`).
const FLAGS_AT: usize = 0x110;
const IN_CUP: u16 = 0x8000;
/// The cup's odds below the first league, by the club's class (team
/// +0x13c) in the second, and a cup holder's (team +0x124 set) two thirds.
const SECOND_LEAGUE_ODDS: [(i8, i32); 2] = [(2, 3000), (3, 2000)];
const SECOND_LEAGUE_OTHER: i32 = 2500;
const LOWER_ODDS: i32 = 5000;
const HOLDER_AT: usize = 0x124;
/// The European clubs drawn on (the entrants' block `0x8e7c88`, data
/// from +0x20): the Champions League's sixteen seeded from +0x12, the UEFA
/// cup's from +0x112 by its count at +4, four bytes each.
const ENTRANTS: &str = "0x8e7c88";
const SEEDED: usize = 0x12;
const SEEDED_COUNT: usize = 16;
const UEFA_COUNT: usize = 4;
const UEFA_CLUBS: usize = 0x112;
const ENTRANT: usize = 4;

/// An ordered list of odds, the shortest first: a new entry goes behind
/// all it does not undercut (`FUN_0077f3bf`, `FUN_0077f4fc`,
/// `FUN_0077f5d9`). A full list counts `most` but keeps one entry more
/// past its count, where the last pushed out or one too long stands.
fn insert<T: Copy>(list: &mut Vec<(T, i16)>, most: usize, entry: T, odds: i16) {
    let counted = list.len().min(most);
    let mut k = counted;
    while k > 0 && odds < list[k - 1].1 {
        k -= 1;
    }
    if k <= most {
        list.truncate(counted);
        list.insert(k, (entry, odds));
        if list.len() > most + 1 {
            list.truncate(most + 1);
        }
    }
}

/// The cup's odds (`FUN_0077738d`): every club in the cup, a first
/// league's at its odds of the championship.
fn cup_odds(records: &mut Records, country: u16) {
    let slot = usize::from(records.countries.slot(usize::from(country)));
    let champions = records.block(ODDS_BLOCK).map(|b| {
        let page = ODDS_AT + slot * ODDS_PAGE;
        (0..ODDS_MOST)
            .map(|k| {
                let e = page + k * ODDS_ENTRY;
                let o = page + ODDS_OF + 2 * k;
                (u16::from_le_bytes([b[e + 2], b[e + 3]]), i16::from_le_bytes([b[o], b[o + 1]]))
            })
            .collect::<Vec<_>>()
    });
    let mut list: Vec<(i16, i16)> = Vec::new();
    for number in 0..records.countries.team_count(usize::from(country)).max(0) as u16 {
        let Some(team) = records.team(country, number) else { continue };
        let level = team.level();
        if team.get::<u16>(FLAGS_AT) & IN_CUP == 0 || level >= 10 {
            continue;
        }
        let mut odds = match level {
            0 => champions.as_ref().and_then(|c| c.iter().find(|e| e.0 == number)).map_or(0, |e| i32::from(e.1)),
            1 => {
                let class = team.class() as i8;
                SECOND_LEAGUE_ODDS.iter().find(|e| e.0 == class).map_or(SECOND_LEAGUE_OTHER, |e| e.1)
            }
            _ => LOWER_ODDS,
        };
        if team.get::<u8>(HOLDER_AT) != 0 {
            odds = (odds * 2 / 3).max(1);
        }
        insert(&mut list, CUP_MOST, number as i16, (odds as i16).max(TRAINER_ODDS_FLOOR));
    }
    let Some(b) = records.block_mut(OFFER) else { return };
    let page = CUP_TEAMS + slot * CUP_PAGE;
    let Some(p) = b.get_mut(page..page + CUP_PAGE) else { return };
    p.fill(0);
    p[..2].copy_from_slice(&(list.len().min(CUP_MOST) as i16).to_le_bytes());
    for (k, &(number, odds)) in list.iter().enumerate() {
        p[2 + 2 * k..4 + 2 * k].copy_from_slice(&number.to_le_bytes());
        p[CUP_ODDS + 2 * k..CUP_ODDS + 2 * k + 2].copy_from_slice(&odds.to_le_bytes());
    }
}

/// The European cups' odds (`FUN_0077f5d9` over `FUN_0041bb6c`'s
/// entrants): by the club's strength a player — the squad's over its
/// count, ten tenths for none — the cube of its distance below 130 over a
/// thousand and ten more, beyond 120 140 less it, 1.1 at least.
fn european_odds(records: &mut Records) {
    let Some(entrants) = records.block(ENTRANTS).map(<[u8]>::to_vec) else { return };
    let word = |at: usize| entrants.get(at..at + 2).map_or(0, |w| i16::from_le_bytes([w[0], w[1]]));
    let uefa = usize::try_from(word(UEFA_COUNT)).unwrap_or(0);
    for (list_at, (first, count)) in EUROPEAN.into_iter().zip([(SEEDED, SEEDED_COUNT), (UEFA_CLUBS, uefa)]) {
        let mut list: Vec<((u8, i16), i16)> = Vec::new();
        for k in 0..count {
            let e = first + k * ENTRANT;
            let (country, number) = (entrants.get(e).copied().unwrap_or(0), word(e + 2));
            let level = u16::try_from(number).ok().and_then(|n| records.team(u16::from(country), n)).map_or(10, |t| {
                let players = i32::from(t.player_count());
                if players == 0 { 10 } else { i32::from(t.squad_strength()) * 10 / players }
            });
            let odds = if level < 120 { (130 - level).pow(3) / 1000 + 10 } else { 140 - level };
            insert(&mut list, EUROPEAN_MOST, (country, number), odds.max(LEAST_ODDS) as i16);
        }
        let Some(b) = records.block_mut(OFFER) else { return };
        let Some(p) = b.get_mut(list_at..list_at + EUROPEAN_ODDS + 2 * EUROPEAN_MOST) else { return };
        p.fill(0);
        p[..2].copy_from_slice(&(list.len().min(EUROPEAN_MOST) as i16).to_le_bytes());
        for (k, &((country, number), odds)) in list.iter().enumerate() {
            let e = 2 + ENTRANT * k;
            p[e] = country;
            p[e + 2..e + 4].copy_from_slice(&number.to_le_bytes());
            p[EUROPEAN_ODDS + 2 * k..EUROPEAN_ODDS + 2 * k + 2].copy_from_slice(&odds.to_le_bytes());
        }
    }
}

/// The tournaments' nations from +0x1a94 (the European championship) and
/// +0x1c66 (the world cup): an i16 count, 115 nations and one past them,
/// their odds from +0xea.
const TOURNAMENTS: [usize; 2] = [0x1a94, 0x1c66];
const NATION_ODDS: usize = 0xea;
const NATIONS_MOST: usize = 115;
const NATIONS: usize = 0x75;
/// A nation's strength byte (`DAT_00ae6223`), and the national teams'
/// records (`0xa9d660` + nation × 0x990, a team's layout).
const NATION_STRENGTH: usize = 0xae6223;
const NATIONAL_TEAMS: u32 = 0xa9d660;
const NATIONAL_COUNT: usize = 0x112 - 0x20;
const NATIONAL_SQUAD: usize = 0x1f2 - 0x20;

/// The tournaments' odds (`FUN_0077713a`, `FUN_0077f776` for each): the
/// European championship's nations, then every nation for the world cup.
/// A played country's national team counts by its squad's strength, ten
/// times its players' average, any other nation by its strength byte ten
/// times and a draw below ten; below 120 the cube of its distance below
/// 130 over a hundred and a draw below five, beyond 140 less it, under
/// 1.1 a draw from 1.1 to 1.5.
fn nation_odds(records: &mut Records, european: &[bool], random: &mut crate::random::Random) {
    let played = crate::opponent::played_nations(records);
    for (list_at, nations) in TOURNAMENTS.into_iter().zip([
        (0..NATIONS).filter(|&n| european.get(n).copied().unwrap_or(false)).collect::<Vec<_>>(),
        (1..NATIONS).collect(),
    ]) {
        let mut list: Vec<(i16, i16)> = Vec::new();
        for nation in nations {
            let squad = played.contains(&(nation as u16)).then(|| {
                records.block(&format!("{NATIONAL_TEAMS:#x}[{nation}]")).map(|b| {
                    let count = b.get(NATIONAL_COUNT..NATIONAL_COUNT + 2).map_or(0, |w| i16::from_le_bytes([w[0], w[1]]));
                    let sum: i32 = (0..usize::try_from(count).unwrap_or(0))
                        .filter_map(|k| b.get(NATIONAL_SQUAD + 2 * k..NATIONAL_SQUAD + 2 * k + 2))
                        .filter_map(|w| records.player(u16::from_le_bytes([w[0], w[1]])))
                        .map(|p| i32::from(p.strength()))
                        .sum();
                    (i32::from(count), sum)
                })
            });
            let level = match squad.flatten() {
                Some((count, sum)) if count >= 1 => sum * 10 / count,
                _ => i32::from(records.countries.get::<u8>(NATION_STRENGTH + nation)) * 10 + random.below(10) as i32,
            };
            let mut odds = if level < 120 {
                let draw = random.below(5) as i32;
                ((130 - level).pow(3) / 100) as i16 as i32 + draw
            } else {
                140 - level
            };
            if odds < LEAST_ODDS {
                odds = random.below(5) as i32 + LEAST_ODDS;
            }
            insert(&mut list, NATIONS_MOST, nation as i16, odds as i16);
        }
        let Some(b) = records.block_mut(OFFER) else { return };
        let Some(p) = b.get_mut(list_at..list_at + NATION_ODDS + 2 * (NATIONS_MOST + 1)) else { return };
        p.fill(0);
        p[..2].copy_from_slice(&(list.len().min(NATIONS_MOST) as i16).to_le_bytes());
        for (k, &(nation, odds)) in list.iter().enumerate() {
            p[2 + 2 * k..4 + 2 * k].copy_from_slice(&nation.to_le_bytes());
            p[NATION_ODDS + 2 * k..NATION_ODDS + 2 * k + 2].copy_from_slice(&odds.to_le_bytes());
        }
    }
}

/// A striker's score (`FUN_00777544`): his strength ten times, five more
/// for a scorer's trait, one for each ability of the first eleven.
pub(crate) fn score(player: &crate::records::Player) -> i32 {
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
    let mut list: Vec<((i16, i16, i16), i16)> = Vec::new();
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
        insert(&mut list, TRAINERS_MOST, (id, country as i16, number as i16), (odds as i16).max(TRAINER_ODDS_FLOOR));
    }
    let Some(b) = records.block_mut(OFFER) else { return };
    let page = TRAINERS + slot * TRAINER_PAGE;
    let Some(p) = b.get_mut(page..page + TRAINER_PAGE) else { return };
    p.fill(0);
    p[..2].copy_from_slice(&(list.len().min(TRAINERS_MOST) as i16).to_le_bytes());
    for (k, &((id, c, n), odds)) in list.iter().enumerate() {
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
            assert_eq!(x[at..at + TRAINER_PAGE], y[at..at + TRAINER_PAGE], "slot {slot} trainers page");
            let at = SCORERS + slot * SCORER_PAGE;
            assert_eq!(x[at..at + SCORER_PAGE], y[at..at + SCORER_PAGE], "slot {slot} scorers page");
        }
    }

    /// Over the same save the cups' and the European cups' lists come out
    /// as the original's.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_cups_odds_come_out_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut b, n) = (load("NewSeason000.an3"), load("NewSeason000.an3"));
        b.block_mut(OFFER).unwrap().fill(0);
        european_odds(&mut b);
        let (x, y) = (b.block(OFFER).unwrap(), n.block(OFFER).unwrap());
        for at in EUROPEAN {
            let count = i16::from_le_bytes(y[at..at + 2].try_into().unwrap()) as usize;
            assert!(count > 10, "{at:#x}: {count}");
            for (from, width) in [(0, 2), (2, ENTRANT * count), (EUROPEAN_ODDS, 2 * count)] {
                assert_eq!(x[at + from..at + from + width], y[at + from..at + from + width], "{at:#x} from {from:#x}");
            }
            assert_eq!(x[at..at + 0xf2], y[at..at + 0xf2], "{at:#x} whole");
        }
        for country in crate::opponent::played_nations(&n) {
            cup_odds(&mut b, country);
            let slot = usize::from(n.countries.slot(usize::from(country)));
            let at = CUP_TEAMS + slot * CUP_PAGE;
            let (x, y) = (b.block(OFFER).unwrap(), n.block(OFFER).unwrap());
            let count = i16::from_le_bytes(y[at..at + 2].try_into().unwrap()) as usize;
            for (from, width) in [(0, 2), (2, 2 * count), (CUP_ODDS, 2 * count)] {
                assert_eq!(x[at + from..at + from + width], y[at + from..at + from + width], "slot {slot} cup from {from:#x}");
            }
            assert_eq!(x[at..at + CUP_PAGE], y[at..at + CUP_PAGE], "slot {slot} cup page");
        }
    }

    /// Over the same save the tournaments' lists hold the original's
    /// nations, as many — a full list's last dropped by chance — and the
    /// played countries' national teams within a draw of the original's
    /// odds.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_tournaments_odds_come_out_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut b, n) = (load("NewSeason000.an3"), load("NewSeason000.an3"));
        let data = Data::load(&dir).unwrap();
        let mut random = crate::random::Random::new(6);
        nation_odds(&mut b, &data.rules.pitch.european, &mut random);
        let played = crate::opponent::played_nations(&n);
        let list = |r: &Records, at: usize| -> Vec<(i16, i16)> {
            let b = r.block(OFFER).unwrap();
            let w = |o: usize| i16::from_le_bytes([b[o], b[o + 1]]);
            (0..w(at) as usize).map(|k| (w(at + 2 + 2 * k), w(at + NATION_ODDS + 2 * k))).collect()
        };
        for at in TOURNAMENTS {
            let (x, y) = (list(&b, at), list(&n, at));
            assert_eq!(x.len(), y.len(), "{at:#x}");
            // A full list drops its weakest by chance: one nation apart
            // at most.
            let apart = x.iter().filter(|e| !y.iter().any(|f| f.0 == e.0)).count();
            assert!(apart <= 1, "{at:#x}: {apart} apart");
            for &(nation, mine) in &x {
                let Some(&(_, theirs)) = y.iter().find(|f| f.0 == nation) else { continue };
                if played.contains(&(nation as u16)) {
                    assert!((theirs - 4..=theirs + 4).contains(&mine), "{at:#x} nation {nation}: {mine} {theirs}");
                }
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
