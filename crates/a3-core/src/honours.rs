//! The titles a club's trainer counts in his career (`FUN_006b50a4` →
//! `FUN_006b5190`): a club with no human manager (club +0x8b) credits its
//! trainer (team +0x9e), with none its manager-trainer (+0x9c), one title
//! of the kind in his person record (`FUN_006b53ad`): the championship at
//! +0x3c, the cup at +0x3e, the European cups at +0x40 (kinds 6 to 8),
//! the supercups and the like at +0x42 (5, 9 to 12) and a promotion at
//! +0x44 (2, counted for the whole staff by [`crate::promotion::count`]).
//! A kind without a counter (the cup's finalist, 0xd) credits nothing. A
//! human manager's own honours (`FUN_006b5497`, `FUN_006b583a`), the
//! trainers' all-time list (`FUN_00725f33`) and the club's trophies
//! beside (`FUN_0061436f`) are not built.
//!
//! The championship counts on the champions' day, half week 0x5c
//! (`FUN_006da3a9` → `FUN_0071d351`): every played country's first league
//! honours its leader by the table then (`FUN_007293ea`) — its season
//! played out by then in the kept one. The cup counts its final's winner
//! (`FUN_006ce00e`), as do the
//! supercups' (`FUN_006cc61b`, kind 5) and England's league cup's
//! (`FUN_0067e911`, kind 0xb or 0xc).

use crate::records::Records;

pub const CHAMPIONSHIP: u8 = 1;
pub const PROMOTION: u8 = 2;
pub const CUP: u8 = 4;
pub const SUPERCUP: u8 = 5;
pub const LEAGUE_CUP: u8 = 0xb;
pub const CUP_FINALIST: u8 = 0xd;

/// The half week of the champions' day (`DAT_00b5cbd2 == 0x5c`).
pub const CHAMPIONS_DAY: u16 = 0x5c;

const LAST_PERSON: u16 = 0x5db;

/// Where a kind of title counts in the person record, if anywhere.
fn counter(kind: u8) -> Option<usize> {
    match kind {
        1 => Some(0x3c),
        2 => Some(0x44),
        4 => Some(0x3e),
        5 | 9..=12 => Some(0x42),
        6..=8 => Some(0x40),
        _ => None,
    }
}

/// The kinds a club marks in its season's titles (`DAT_008a56b8`, the
/// kinds of a promotion and below not among them).
const MARKED: [bool; 14] = [false, true, false, false, true, true, true, true, true, true, true, true, true, false];
/// The season's titles in the past seasons' byte (team +0x11e, the club
/// object's +0xee): its low two bits, three at most.
const SEASON_TITLES: usize = 0x11e;

/// A club's title of `kind` (`FUN_006b50a4`): its trainer counts it
/// ([`honour`]), the club marks it among the season's titles (+0x11e's
/// low two bits one more, three at most; `FUN_00698e00`), and for the
/// cup every man of its squad counts it in his second honour (player
/// +0x29a, `FUN_006b4fdb`). (A human club's season object, `FUN_0061436f`,
/// and the European cups' squads, `FUN_006b501e`, `FUN_006b5061`, are not
/// built.)
pub fn title(records: &mut Records, country: u16, number: u16, kind: u8) {
    honour(records, country, number, kind);
    if MARKED.get(usize::from(kind)).copied().unwrap_or(false)
        && let Some(t) = records.team_mut(country, number)
    {
        let v = t.get::<u8>(SEASON_TITLES);
        let titles = ((v & 3) + 1).min(3);
        t.put::<u8>(SEASON_TITLES, v & !3 | titles);
    }
    if kind == CUP {
        let squad = records.team(country, number).map_or_else(Vec::new, |t| t.players());
        for id in squad {
            if let Some(p) = records.player_mut(id) {
                let mut h = p.honours();
                h[1] = h[1].wrapping_add(1);
                p.set_honours(h);
            }
        }
    }
}

/// The bytes of a cup's pairing the cup winners list keeps.
pub const FINAL_PAIRING: usize = 0xe;
/// The day of the last cup final (`_DAT_00b5df8e`).
const LAST_CUP_FINAL_DAY: usize = 0xb5df8e;

/// A country's cup final decided (`FUN_006ced69`, and `FUN_00634ba3` in
/// `FUN_006ce00e`): the country's cup winners list drops its oldest of
/// twenty and keeps the final last — the start year and the pairing with
/// its score —, the winner counts one cup more in the second titles list
/// (the lists block +0x105e0, 0xd8 a slot), and the day is noted.
pub fn cup_final(records: &mut Records, country: u16, tie: &[u8]) {
    use crate::statistics::{CUP_WINNERS, LISTS, TITLES, TITLES_SLOT};
    let Some(tie) = tie.get(..FINAL_PAIRING) else { return };
    let year = records.globals.start_year();
    let day = records.globals.day();
    let slot = usize::from(records.countries.slot(usize::from(country)));
    let w = usize::from(tie[0xd] != 0) * 4;
    let winner = u16::from_le_bytes([tie[w + 2], tie[w + 3]]);
    if let Some(b) = records.block_mut(LISTS) {
        let (at, per) = CUP_WINNERS;
        let from = at + slot * per;
        if let Some(list) = b.get_mut(from..from + per) {
            list.copy_within(0x10.., 0);
            let e = &mut list[per - 0x10..];
            e[0..2].copy_from_slice(&year.to_le_bytes());
            e[2..2 + FINAL_PAIRING].copy_from_slice(tie);
        }
        let titles = TITLES[1] + slot * TITLES_SLOT + usize::from(winner) * 2;
        if let Some(t) = b.get_mut(titles..titles + 2) {
            let v = i16::from_le_bytes([t[0], t[1]]).wrapping_add(1);
            t.copy_from_slice(&v.to_le_bytes());
        }
    }
    records.globals.put::<u16>(LAST_CUP_FINAL_DAY, day);
}

/// A club's title of `kind` in its trainer's career (`FUN_006b5190`'s
/// computer branch).
pub fn honour(records: &mut Records, country: u16, number: u16, kind: u8) {
    let Some(team) = records.team(country, number) else { return };
    if team.humans()[1] != 0 {
        return;
    }
    let [manager_trainer, trainer, _] = team.staff();
    let id = if trainer != 0 { trainer } else { manager_trainer };
    let (Some(at), true) = (counter(kind), id != 0 && id <= LAST_PERSON) else { return };
    if let Some(p) = records.persons.get_mut(usize::from(id)) {
        p.put::<i16>(at, p.get::<i16>(at).wrapping_add(1));
    }
}

/// The champions' day (`FUN_0071d351`): every played country's first
/// league's leader by the table then is its champion —
/// - the country's champions list drops its oldest of twenty and keeps
///   the new one last (`FUN_0071d54b`, `FUN_0071d76d`): the start year,
///   the club's country and number, its points and goals;
/// - the club counts one title more in the first of its titles lists
///   (`FUN_00634a1d`: the lists block +0x101a8, 0xd8 a slot);
/// - it marks the title in its honours byte (team +0x8ea, 0x80;
///   `FUN_00699fe1`) and every man of its squad counts it in his own
///   first honour (player +0x299, `FUN_007045c9` in `FUN_0071d2d0`);
/// - its history (the object `0xa6d348`, 0x80 a club) counts one title
///   more at +0 and keeps the calendar year among the last five after it
///   (`FUN_0064089e`, `FUN_00640d8b`);
/// - and it takes the title ([`title`]).
///
/// (The human's event 0x6a and country 0x3b's play-off, `FUN_006d8bb0`,
/// are not built.)
const CHAMPION_MARK: u8 = 0x80;
const HISTORY: &str = "0xa6d348";
const HISTORY_CLUB: usize = 0x80;
const HISTORY_YEARS_END: usize = 0xc;
const TEAM_HONOURS: usize = 0x8ea;
const CHAMPION_ENTRY: usize = 0xc;

/// The Herbstmeister (`FUN_0071d175` on the half weeks 0x2f to 0x38,
/// `FUN_0071d686`): a country whose half week has come — its entry of
/// `DAT_008a7cb8` by its index, the main country's at `0xb5cbb2`, the
/// more countries' at `0xb5cbbc` beside them at `0xb5cbb4` — enters its
/// first league's leader by the table then into its list of autumn
/// champions as the champions' list keeps its own.
const AUTUMN_DAYS: [u16; 5] = [48, 54, 48, 56, 56];
pub const AUTUMN_FIRST: u16 = 0x2f;
pub const AUTUMN_LAST: u16 = 0x38;
const MAIN_INDEX: usize = 0xb5cbb2;
const MORE_COUNTRIES: usize = 0xb5cbb4;
const MORE_INDEX: usize = 0xb5cbbc;
const MORE_COUNT: usize = 0xb5cbae;

pub fn autumn_day(records: &mut Records, half_week: u16) {
    let main = u16::try_from(records.globals.main_country()).unwrap_or(0);
    let mut countries = vec![(main, records.globals.get::<i16>(MAIN_INDEX))];
    for k in 0..usize::try_from(records.globals.get::<i16>(MORE_COUNT)).unwrap_or(0).min(4) {
        let country = records.globals.get::<i16>(MORE_COUNTRIES + 2 * k);
        countries.push((u16::try_from(country).unwrap_or(0), records.globals.get::<i16>(MORE_INDEX + 2 * k)));
    }
    for (country, index) in countries {
        let due = usize::try_from(index).ok().and_then(|i| AUTUMN_DAYS.get(i)).copied();
        if due == Some(half_week) {
            enter_leader(records, country, crate::statistics::AUTUMN, crate::statistics::ALL);
        }
    }
}

/// A country's first league's leader by the table of `kind` into one of
/// its lists of twenty champions' entries, the oldest dropped
/// (`FUN_0071d76d`): the start year, the club's country and number, its
/// points and goals. Gives the leader's number.
fn enter_leader(records: &mut Records, country: u16, (at, per): (usize, usize), kind: u8) -> Option<u16> {
    let league = crate::opponent::League::of(records, country, 0)?;
    let row = crate::statistics::table(records, &league, kind).first().cloned()?;
    let number = u16::try_from(row.team).ok()?;
    let year = records.globals.start_year();
    let slot = usize::from(records.countries.slot(usize::from(country)));
    let list = records.block_mut(crate::statistics::LISTS)?.get_mut(at + slot * per..at + (slot + 1) * per)?;
    list.copy_within(CHAMPION_ENTRY.., 0);
    let e = &mut list[per - CHAMPION_ENTRY..];
    e[0..2].copy_from_slice(&year.to_le_bytes());
    e[2] = country as u8;
    e[3] = 0;
    e[4..6].copy_from_slice(&row.team.to_le_bytes());
    e[6..8].copy_from_slice(&(row.points as i16).to_le_bytes());
    e[8..10].copy_from_slice(&(row.goals_for as i16).to_le_bytes());
    e[10..12].copy_from_slice(&(row.goals_against as i16).to_le_bytes());
    Some(number)
}

/// The season's top scorers the same half week (`FUN_0071cd1f` →
/// `FUN_0071cd6f` per played country, its first league's list of
/// scorers — the main country's first, a more country's by its place
/// after six, `FUN_00677b07`): the country's Torschützenkönige move on a
/// season, the oldest dropped, and the new one holds the first three of
/// those tied with the leader — each ten bytes: his names, his club's
/// number, his goals and the start year; every one so tied counts it in
/// his honours (player +0x29f, `FUN_007047a3`); and every scorer of the
/// list with goals at least the tenth best tally's goes into the ten best
/// tallies, above those he equals. (The runtime note of the country's top
/// scorer, `DAT_01c6857e`, is not kept.)
const SCORER_ENTRY: usize = 0x10;
const SCORERS: usize = 100;
const TIED: usize = 10;
const KING_SEASON: usize = 0x1e;
const TALLY: usize = 10;

pub fn top_scorers_day(records: &mut Records) {
    use crate::statistics::{KINGS, LISTS, TALLIES, TOR_LEAGUE, TOR_LIST};
    let main = u16::try_from(records.globals.main_country()).unwrap_or(0);
    let mut countries = vec![(main, 0usize)];
    for k in 0..usize::try_from(records.globals.get::<i16>(MORE_COUNT)).unwrap_or(0).min(4) {
        let country = records.globals.get::<i16>(MORE_COUNTRIES + 2 * k);
        countries.push((u16::try_from(country).unwrap_or(0), k + 6));
    }
    let year = records.globals.start_year();
    for (country, index) in countries {
        let slot = usize::from(records.countries.slot(usize::from(country)));
        let Some(b) = records.block_mut(LISTS) else { return };
        let from = TOR_LIST + index * TOR_LEAGUE;
        let Some(list) = b.get(from..from + TOR_LEAGUE).map(<[u8]>::to_vec) else { continue };
        let entry = |k: usize| &list[k * SCORER_ENTRY..(k + 1) * SCORER_ENTRY];
        let tally = |e: &[u8]| -> [u8; TALLY] {
            let mut t = [0u8; TALLY];
            t[0..4].copy_from_slice(&e[2..6]);
            t[4..6].copy_from_slice(&u16::from(e[7]).to_le_bytes());
            t[6..8].copy_from_slice(&u16::from(e[8]).to_le_bytes());
            t[8..10].copy_from_slice(&year.to_le_bytes());
            t
        };
        let (at, per) = KINGS;
        let kings = at + slot * per;
        let Some(k) = b.get_mut(kings..kings + per) else { continue };
        k.copy_within(KING_SEASON.., 0);
        k[per - KING_SEASON..].fill(0);
        let lead = entry(0)[8];
        let mut tied = Vec::new();
        for i in (0..TIED).take_while(|&i| entry(i)[8] == lead) {
            if i < 3 {
                let at = per - KING_SEASON + i * TALLY;
                k[at..at + TALLY].copy_from_slice(&tally(entry(i)));
            }
            let id = u16::from_le_bytes([entry(i)[0], entry(i)[1]]);
            if id != 0 {
                tied.push(id);
            }
        }
        let (at, per) = TALLIES;
        let best = at + slot * per;
        if let Some(t) = b.get_mut(best..best + per) {
            let goals = |t: &[u8], k: usize| i16::from_le_bytes([t[k * TALLY + 6], t[k * TALLY + 7]]);
            for i in (0..SCORERS).take_while(|&i| entry(i)[8] != 0) {
                let g = i16::from(entry(i)[8]);
                if goals(t, 9) > g {
                    continue;
                }
                let mut e = 8i32;
                while e >= 0 && goals(t, e as usize) <= g {
                    let k = e as usize;
                    t.copy_within(k * TALLY..(k + 1) * TALLY, (k + 1) * TALLY);
                    e -= 1;
                }
                let k = (e + 1) as usize;
                t[k * TALLY..(k + 1) * TALLY].copy_from_slice(&tally(entry(i)));
            }
        }
        for id in tied {
            if let Some(p) = records.player_mut(id) {
                let mut h = p.honours();
                h[6] = h[6].wrapping_add(1);
                p.set_honours(h);
            }
        }
    }
}

pub fn champions_day(records: &mut Records) {
    use crate::statistics::{CHAMPIONS, LISTS, TITLES, TITLES_SLOT};
    for country in crate::opponent::played_nations(records) {
        let Some(number) = enter_leader(records, country, CHAMPIONS, crate::statistics::ALL) else { continue };
        let slot = usize::from(records.countries.slot(usize::from(country)));
        if let Some(b) = records.block_mut(LISTS) {
            let titles = TITLES[0] + slot * TITLES_SLOT + usize::from(number) * 2;
            if let Some(w) = b.get_mut(titles..titles + 2) {
                let v = i16::from_le_bytes([w[0], w[1]]).wrapping_add(1);
                w.copy_from_slice(&v.to_le_bytes());
            }
        }
        // The club's history: one title more and its year among the last
        // five (`FUN_0064089e`, `FUN_00640d8b`).
        let year = records.globals.date()[2] as i16;
        if let (Some(index), Some(b)) = (records.team_index(country, number), records.block_mut(HISTORY))
            && let Some(h) = b.get_mut(index * HISTORY_CLUB..index * HISTORY_CLUB + HISTORY_YEARS_END)
        {
            let count = i16::from_le_bytes([h[0], h[1]]);
            if count < 5 {
                let at = 2 + 2 * usize::try_from(count).unwrap_or(0);
                h[at..at + 2].copy_from_slice(&year.to_le_bytes());
            } else {
                h.copy_within(4..12, 2);
                h[10..12].copy_from_slice(&year.to_le_bytes());
            }
            h[0..2].copy_from_slice(&count.wrapping_add(1).to_le_bytes());
        }
        let squad = records.team(country, number).map_or_else(Vec::new, |t| t.players());
        if let Some(t) = records.team_mut(country, number) {
            t.put::<u8>(TEAM_HONOURS, t.get::<u8>(TEAM_HONOURS) | CHAMPION_MARK);
        }
        for id in squad {
            if let Some(p) = records.player_mut(id) {
                let mut h = p.honours();
                h[0] = h[0].wrapping_add(1);
                p.set_honours(h);
            }
        }
        title(records, country, number, CHAMPIONSHIP);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The season's champions and cup winners count their titles as the
    /// original's: from the kept day-63 save to the season's end the
    /// trainer — with none the manager-trainer — of every champion (by the
    /// champions list) and cup winner (flag 0x1000) gains the title, as
    /// the replica's honour gives it; and the champions' day, run over the
    /// season's end with what it moves set back to the day-63 save's,
    /// writes the champions lists, the titles counts, the honours byte,
    /// the clubs' histories and the squads' honours as the original did.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_titles_count_in_the_trainers_careers_as_the_original() {
        use a3_files::an3::Save;
        use crate::statistics::{CHAMPIONS, LISTS, TITLES, TITLES_SLOT};
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let read = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (a, end, new) = (read("A.an3"), read("SeasonEnd364.an3"), read("NewSeason000.an3"));
        let mut ours = read("A.an3");
        let mut count = 0;
        for country in crate::opponent::played_nations(&new) {
            if let Some(champion) = crate::statistics::champions(&end, country).first() {
                title(&mut ours, country, champion.club.1 as u16, CHAMPIONSHIP);
                count += 1;
            }
            for number in 0..new.countries.team_count(usize::from(country)).max(0) as u16 {
                if new.team(country, number).unwrap().get::<u16>(0x110) & 0x1000 != 0 {
                    title(&mut ours, country, number, CUP);
                    count += 1;
                    // The cup's men count it.
                    for id in end.team(country, number).unwrap().players() {
                        let (Some(was), Some(now)) = (a.player(id), end.player(id)) else { continue };
                        if was.birthday() == now.birthday() && a.team(country, number).unwrap().players().contains(&id) {
                            assert_eq!(ours.player(id).unwrap().honours()[1], now.honours()[1], "player {id}'s cups");
                        }
                    }
                }
            }
        }
        assert_eq!(count, 10, "five champions and five cup winners");
        // The season's titles, but where a European one came beside.
        let mut marked = 0;
        for i in 0..ours.teams.len() {
            let (x, y) = (ours.teams[i].get::<u8>(SEASON_TITLES) & 3, end.teams[i].get::<u8>(SEASON_TITLES) & 3);
            if x != a.teams[i].get::<u8>(SEASON_TITLES) & 3 {
                marked += 1;
                assert!(x == y || y > x, "team {i}'s season titles {x} {y}");
            }
        }
        assert_eq!(marked, 10, "ten clubs marked");
        for id in 1..=usize::from(LAST_PERSON) {
            for at in [0x3c, 0x3e] {
                assert_eq!(ours.persons[id].get::<i16>(at), end.persons[id].get::<i16>(at), "person {id} +{at:#x}");
            }
        }
        // The champions' day over the season's end, set back.
        let mut day = read("SeasonEnd364.an3");
        let lists = a.block(LISTS).unwrap().to_vec();
        {
            let b = day.block_mut(LISTS).unwrap();
            for country in crate::opponent::played_nations(&new) {
                let slot = usize::from(a.countries.slot(usize::from(country)));
                for (at, per) in [CHAMPIONS, (TITLES[0], TITLES_SLOT)] {
                    let r = at + slot * per..at + (slot + 1) * per;
                    b[r.clone()].copy_from_slice(&lists[r]);
                }
            }
        }
        for i in 0..day.teams.len() {
            let v = a.teams[i].get::<u8>(TEAM_HONOURS);
            day.teams[i].put::<u8>(TEAM_HONOURS, v);
        }
        let history = a.block(HISTORY).unwrap().to_vec();
        day.block_mut(HISTORY).unwrap().copy_from_slice(&history);
        for id in 1..day.players.len().min(a.players.len()) {
            let h = a.players[id].honours();
            day.players[id].set_honours(h);
        }
        for id in 1..=usize::from(LAST_PERSON) {
            let v = a.persons[id].get::<i16>(0x3c);
            day.persons[id].put::<i16>(0x3c, v);
        }
        champions_day(&mut day);
        // And the cups' finals, as the season's end holds them.
        {
            use crate::statistics::CUP_WINNERS;
            let b = day.block_mut(LISTS).unwrap();
            for country in crate::opponent::played_nations(&new) {
                let slot = usize::from(a.countries.slot(usize::from(country)));
                for (at, per) in [CUP_WINNERS, (TITLES[1], TITLES_SLOT)] {
                    let r = at + slot * per..at + (slot + 1) * per;
                    b[r.clone()].copy_from_slice(&lists[r]);
                }
            }
        }
        for country in crate::opponent::played_nations(&new) {
            let slot = end.countries.slot(usize::from(country));
            let cup = end.block(&format!("{:#x}[{slot}]", crate::opponent::CUPS_AT)).unwrap();
            let mode = usize::from(cup[0x24 - 0x20]).min(5);
            let (start, _) = crate::opponent::cup_pairings(mode, 6).unwrap();
            let at = crate::opponent::PAIRINGS + start * crate::opponent::PAIRING;
            let tie = cup[at..at + FINAL_PAIRING].to_vec();
            cup_final(&mut day, country, &tie);
        }
        // The Herbstmeister by the first round's table, which on the
        // countries' autumn days stood played.
        {
            let b = day.block_mut(LISTS).unwrap();
            for country in crate::opponent::played_nations(&new) {
                let slot = usize::from(a.countries.slot(usize::from(country)));
                let (at, per) = crate::statistics::AUTUMN;
                let r = at + slot * per..at + (slot + 1) * per;
                b[r.clone()].copy_from_slice(&lists[r]);
            }
        }
        for country in crate::opponent::played_nations(&new) {
            enter_leader(&mut day, country, crate::statistics::AUTUMN, crate::statistics::FIRST_ROUND);
        }
        // The top scorers of the season the same day.
        {
            let b = day.block_mut(LISTS).unwrap();
            for country in crate::opponent::played_nations(&new) {
                let slot = usize::from(a.countries.slot(usize::from(country)));
                for (at, per) in [crate::statistics::KINGS, crate::statistics::TALLIES] {
                    let r = at + slot * per..at + (slot + 1) * per;
                    b[r.clone()].copy_from_slice(&lists[r]);
                }
            }
        }
        let kings_before: Vec<u8> = day.players.iter().map(|p| p.honours()[6]).collect();
        for id in 1..day.players.len().min(a.players.len()) {
            let mut h = day.players[id].honours();
            h[6] = a.players[id].honours()[6];
            day.players[id].set_honours(h);
        }
        top_scorers_day(&mut day);
        let mut kings = 0;
        for id in 1..day.players.len().min(a.players.len()) {
            if a.players[id].birthday() != end.players[id].birthday() {
                continue;
            }
            assert_eq!(day.players[id].honours()[6], end.players[id].honours()[6], "player {id}'s scorers' crowns");
            kings += i32::from(end.players[id].honours()[6] != a.players[id].honours()[6]);
        }
        assert!(kings >= 5 && kings_before.len() > 0, "{kings} top scorers");
        let (x, y) = (day.block(LISTS).unwrap(), end.block(LISTS).unwrap());
        for country in crate::opponent::played_nations(&new) {
            let slot = usize::from(a.countries.slot(usize::from(country)));
            for (at, per) in [CHAMPIONS, (TITLES[0], TITLES_SLOT), crate::statistics::CUP_WINNERS, (TITLES[1], TITLES_SLOT), crate::statistics::AUTUMN, crate::statistics::KINGS, crate::statistics::TALLIES] {
                let r = at + slot * per..at + (slot + 1) * per;
                assert_eq!(x[r.clone()], y[r], "{country}'s list at {at:#x}");
            }
        }
        for i in 0..day.teams.len() {
            assert_eq!(day.teams[i].get::<u8>(TEAM_HONOURS) & CHAMPION_MARK, end.teams[i].get::<u8>(TEAM_HONOURS) & CHAMPION_MARK, "team {i}'s mark");
            let r = i * HISTORY_CLUB..i * HISTORY_CLUB + HISTORY_YEARS_END;
            if r.end <= day.block(HISTORY).unwrap().len() {
                assert_eq!(day.block(HISTORY).unwrap()[r.clone()], end.block(HISTORY).unwrap()[r], "team {i}'s history");
            }
        }
        let mut squads = 0;
        for country in crate::opponent::played_nations(&new) {
            let Some(champion) = crate::statistics::champions(&end, country).first().cloned() else { continue };
            for id in end.team(country, champion.club.1 as u16).unwrap().players() {
                if id == 0 || a.players[usize::from(id)].birthday() != end.players[usize::from(id)].birthday() {
                    continue;
                }
                assert_eq!(day.player(id).unwrap().honours()[0], end.player(id).unwrap().honours()[0], "player {id}'s titles");
                squads += 1;
            }
        }
        assert!(squads > 80, "{squads} champions' men");
        for id in 1..=usize::from(LAST_PERSON) {
            assert_eq!(day.persons[id].get::<i16>(0x3c), end.persons[id].get::<i16>(0x3c), "person {id}'s championships");
        }
    }

    /// The half weeks name their champions as the replica's own season
    /// stands: walking the kept day-63 save through the autumn every
    /// country's Herbstmeister enters its list once, and walking the kept
    /// late second season past the champions' day every country's champion
    /// and top scorers enter theirs — the season's year and the table's
    /// leader newest.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_half_weeks_name_their_champions() {
        use a3_files::an3::Save;
        use crate::random::Random;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let read = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let mut random = Random::new(4);
        let mut autumn = read("A.an3");
        let before: Vec<usize> = crate::opponent::played_nations(&autumn)
            .iter()
            .map(|&c| crate::statistics::autumn_champions(&autumn, c).0.len())
            .collect();
        for day in 64..230 {
            crate::development::half_week(&mut autumn, day, &mut random);
        }
        for (k, country) in crate::opponent::played_nations(&autumn).into_iter().enumerate() {
            let (list, _) = crate::statistics::autumn_champions(&autumn, country);
            assert_eq!(list.len(), (before[k] + 1).min(20), "{country}'s Herbstmeister once");
            let league = crate::opponent::League::of(&autumn, country, 0).unwrap();
            let leader = crate::statistics::table(&autumn, &league, crate::statistics::ALL)[0].team;
            assert_eq!((list[0].year, list[0].club.1), (autumn.globals.start_year() as i16, leader), "{country}");
        }
        let mut late = read("S2late.an3");
        let kings: Vec<usize> =
            crate::opponent::played_nations(&late).iter().map(|&c| crate::statistics::top_scorers(&late, c).len()).collect();
        for day in 299..340 {
            crate::development::half_week(&mut late, day, &mut random);
        }
        for (k, country) in crate::opponent::played_nations(&late).into_iter().enumerate() {
            let champions = crate::statistics::champions(&late, country);
            let league = crate::opponent::League::of(&late, country, 0).unwrap();
            let leader = crate::statistics::table(&late, &league, crate::statistics::ALL)[0].team;
            assert_eq!((champions[0].year, champions[0].club.1), (late.globals.start_year() as i16, leader), "{country}'s champion");
            let crowned = crate::statistics::top_scorers(&late, country);
            assert!(crowned.len() > kings[k] || crowned.len() >= 3 * 19, "{country}'s kings: {} after {}", crowned.len(), kings[k]);
            assert_eq!(crowned[0].year, late.globals.start_year() as i16, "{country}'s newest kings");
        }
    }
}
