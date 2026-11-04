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
//! honours its leader by the table then (`FUN_007293ea`), which in a
//! league still playing is not yet its champion — England's in the kept
//! season. The cup counts its final's winner (`FUN_006ce00e`).

use crate::records::Records;

pub const CHAMPIONSHIP: u8 = 1;
pub const PROMOTION: u8 = 2;
pub const CUP: u8 = 4;
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
/// league honours its leader. (The champions list, the club's history and
/// the rest of that day, `FUN_0071d54b`, `FUN_0064089e`, `FUN_0071d2d0`,
/// are not built; nor country 0x3b's play-off, `FUN_006d8bb0`.)
pub fn champions_day(records: &mut Records) {
    for country in crate::opponent::played_nations(records) {
        let Some(league) = crate::opponent::League::of(records, country, 0) else { continue };
        let Some(leader) = crate::statistics::table(records, &league, crate::statistics::ALL).first().map(|r| r.team) else {
            continue;
        };
        if let Ok(number) = u16::try_from(leader) {
            honour(records, country, number, CHAMPIONSHIP);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The season's champions and cup winners count their titles as the
    /// original's: from the kept day-63 save to the season's end the
    /// trainer — with none the manager-trainer — of every club the new
    /// season flags champion (but England's, whose leader on the
    /// champions' day was another) or cup winner gains the title, and the
    /// replica's honour gives him the same.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_titles_count_in_the_trainers_careers_as_the_original() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let read = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, end, new) = (read("A.an3"), read("SeasonEnd364.an3"), read("NewSeason000.an3"));
        const ENGLAND: u16 = 16;
        let mut count = 0;
        for country in crate::opponent::played_nations(&new) {
            for number in 0..new.countries.team_count(usize::from(country)).max(0) as u16 {
                let flags = new.team(country, number).unwrap().get::<u16>(0x110);
                for (flag, kind) in [(1u16, CHAMPIONSHIP), (0x1000, CUP)] {
                    if flags & flag == 0 || (kind == CHAMPIONSHIP && country == ENGLAND) {
                        continue;
                    }
                    honour(&mut ours, country, number, kind);
                    count += 1;
                }
            }
        }
        assert_eq!(count, 9, "four champions and five cup winners");
        for id in 1..=usize::from(LAST_PERSON) {
            for at in [0x3c, 0x3e] {
                let (x, y) = (ours.persons[id].get::<i16>(at), end.persons[id].get::<i16>(at));
                if id == 356 && at == 0x3c {
                    continue; // England's leader on the champions' day.
                }
                assert_eq!(x, y, "person {id} +{at:#x}");
            }
        }
    }
}
