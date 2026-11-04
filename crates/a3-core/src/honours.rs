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
    if MARKED.get(usize::from(kind)).copied().unwrap_or(false) {
        if let Some(t) = records.team_mut(country, number) {
            let v = t.get::<u8>(SEASON_TITLES);
            let titles = ((v & 3) + 1).min(3);
            t.put::<u8>(SEASON_TITLES, v & !3 | titles);
        }
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
/// - and it takes the title ([`title`]).
/// (The club's history, `FUN_0064089e`, the human's event 0x6a and
/// country 0x3b's play-off, `FUN_006d8bb0`, are not built.)
const CHAMPION_MARK: u8 = 0x80;
const TEAM_HONOURS: usize = 0x8ea;
const CHAMPION_ENTRY: usize = 0xc;

pub fn champions_day(records: &mut Records) {
    use crate::statistics::{CHAMPIONS, LISTS, TITLES, TITLES_SLOT};
    let year = records.globals.start_year();
    for country in crate::opponent::played_nations(records) {
        let Some(league) = crate::opponent::League::of(records, country, 0) else { continue };
        let Some(row) = crate::statistics::table(records, &league, crate::statistics::ALL).first().cloned() else {
            continue;
        };
        let Ok(number) = u16::try_from(row.team) else { continue };
        let slot = usize::from(records.countries.slot(usize::from(country)));
        if let Some(b) = records.block_mut(LISTS) {
            let (at, per) = CHAMPIONS;
            let from = at + slot * per;
            if let Some(list) = b.get_mut(from..from + per) {
                list.copy_within(CHAMPION_ENTRY.., 0);
                let e = &mut list[per - CHAMPION_ENTRY..];
                e[0..2].copy_from_slice(&year.to_le_bytes());
                e[2] = country as u8;
                e[3] = 0;
                e[4..6].copy_from_slice(&row.team.to_le_bytes());
                e[6..8].copy_from_slice(&(row.points as i16).to_le_bytes());
                e[8..10].copy_from_slice(&(row.goals_for as i16).to_le_bytes());
                e[10..12].copy_from_slice(&(row.goals_against as i16).to_le_bytes());
            }
            let titles = TITLES[0] + slot * TITLES_SLOT + usize::from(number) * 2;
            if let Some(w) = b.get_mut(titles..titles + 2) {
                let v = i16::from_le_bytes([w[0], w[1]]).wrapping_add(1);
                w.copy_from_slice(&v.to_le_bytes());
            }
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
    /// writes the champions lists, the titles counts, the honours byte and
    /// the squads' honours as the original did.
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
        for id in 1..day.players.len().min(a.players.len()) {
            let h = a.players[id].honours();
            day.players[id].set_honours(h);
        }
        for id in 1..=usize::from(LAST_PERSON) {
            let v = a.persons[id].get::<i16>(0x3c);
            day.persons[id].put::<i16>(0x3c, v);
        }
        champions_day(&mut day);
        let (x, y) = (day.block(LISTS).unwrap(), end.block(LISTS).unwrap());
        for country in crate::opponent::played_nations(&new) {
            let slot = usize::from(a.countries.slot(usize::from(country)));
            for (at, per) in [CHAMPIONS, (TITLES[0], TITLES_SLOT)] {
                let r = at + slot * per..at + (slot + 1) * per;
                assert_eq!(x[r.clone()], y[r], "{country}'s list at {at:#x}");
            }
        }
        for i in 0..day.teams.len() {
            assert_eq!(day.teams[i].get::<u8>(TEAM_HONOURS) & CHAMPION_MARK, end.teams[i].get::<u8>(TEAM_HONOURS) & CHAMPION_MARK, "team {i}'s mark");
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
}
