//! Being a national coach (NATIONALMANNSCHAFT.md in the RE repo): whether
//! the national team plays this week, and giving the job up.

use crate::calendar::Calendar;
use crate::load;
use crate::records::Records;
use crate::season_odds::{NATIONAL_COUNT, NATIONAL_SQUAD, NATIONAL_TEAMS, NATIONS};

/// A national team's record (`0xa9d660[nation]`, its data 0x20 in): the
/// coach's number, twice (club +0x8a and +0x8b when it is loaded), and the
/// day of its next match (the club's +0xb44, a team's +0x250).
const COACH: usize = 0xa8 - 0x20;
const NEXT_DAY: usize = 0x250 - 0x20;
/// Set, any match still to come counts as this week's (`DAT_00b5dfc4`,
/// what sets it is not known; TRAINING-KURZ.md).
const ANY_DAY_AHEAD: usize = 0xb5dfc4;
/// A human's national key, eight bytes each by his number: the nation and
/// the team's number, 200 (`FUN_006b9755`).
const KEY_NATION: usize = 0xb5d172;
const KEY_TEAM: usize = 0xb5d174;
const KEY: usize = 8;

/// Why a coach left (`FUN_006fdf90`'s second argument): he resigned.
pub const RESIGNED: u8 = 4;
/// Fit players a coach must have for his team's match this week before the
/// turn may end (`FUN_00650b84` asm 0x652467 and `FUN_0065270d` asm
/// 0x652a0d, `0x11 < count`); a player counts as hurt once he has been
/// out for a week, his +0x2c at 7 days or more.
const FIT_ENOUGH: usize = 18;
const FIT_DAYS: i32 = 7;
/// The year calendar's international row: `FUN_00629d80` reads it at
/// `DAT_008914e0 + day * 4` and keeps the flag in its top byte; of those,
/// 0x04000000 ("Nat.-Freund.") and 0x08000000 ("WM-Quali") mark an
/// international window (`FUN_0067dc44`).
const INTERNATIONAL_ROW: usize = 0;
const WINDOW_FLAGS: [u32; 2] = [0x0400_0000, 0x0800_0000];

/// A national team's saved block, if `nation` is a nation.
fn team_block(records: &Records, nation: i16) -> Option<&[u8]> {
    if !usize::try_from(nation).is_ok_and(|n| n < NATIONS) {
        return None;
    }
    records.block(&format!("{NATIONAL_TEAMS:#x}[{nation}]"))
}

/// Whether the national team of `nation` plays this week (`FUN_0067db00`):
/// its next match after the week's first day and before the next week's,
/// or, with `DAT_00b5dfc4` set, any match still to come.
pub fn plays_this_week(records: &Records, nation: i16) -> bool {
    let Some(team) = team_block(records, nation) else {
        return false;
    };
    let next = i16::from_le_bytes([team[NEXT_DAY], team[NEXT_DAY + 1]]);
    let today = records.globals.get::<i16>(0xb5cbd4);
    let week = records.globals.week_start();
    (records.globals.get::<i32>(ANY_DAY_AHEAD) != 0 && next >= today)
        || (week < next && i32::from(next) < i32::from(week) + 7)
}

/// The most a coach may name (`FUN_0051b6fe`'s cap, the count
/// `FUN_0051ca6c` calls full): 0x16.
pub const SQUAD_MOST: usize = 0x16;

/// The squad the team block keeps: its count at +0x112 and the ids at
/// +0x1f2, the loaded club's list at its +0xe2 and +0xb50.
pub fn squad_ids(records: &Records, nation: i16) -> Vec<u16> {
    let Some(team) = team_block(records, nation) else {
        return Vec::new();
    };
    let count = i16::from_le_bytes([team[NATIONAL_COUNT], team[NATIONAL_COUNT + 1]]).max(0) as usize;
    (0..count)
        .filter_map(|k| team.get(NATIONAL_SQUAD + 2 * k..NATIONAL_SQUAD + 2 * k + 2))
        .map(|w| u16::from_le_bytes([w[0], w[1]]))
        .collect()
}

/// Call `id` up into a squad the picker holds beside its dialog
/// (`FUN_0051b6fe`): nothing for id 0, nothing once 22 are named or he
/// already stands in the list; else he lands after the last player whose
/// main position does not beat his — the loop compares the handle's +3,
/// the byte the cache copies from the record's +0x21d (`FUN_00636638`),
/// the first of his positions, and shifts every bigger one along — so
/// the list runs keepers first, strikers last.  Answers whether he went
/// in.
pub fn call_up(squad: &mut Vec<u16>, records: &Records, id: u16) -> bool {
    if id == 0 || squad.len() >= SQUAD_MOST || squad.contains(&id) {
        return false;
    }
    // A record the save does not keep reads position 0, under every
    // real one; the original reads the byte off the handle it loads.
    let position = records.player(id).map_or(0, |p| p.positions()[0]);
    let position_of = |&id: &u16| records.player(id).map_or(0, |p| p.positions()[0]);
    squad.push(0);
    let mut at = squad.len() - 1;
    while at > 0 && position_of(&squad[at - 1]) > position {
        squad.swap(at, at - 1);
        at -= 1;
    }
    squad[at] = id;
    true
}

/// Send `id` home from a squad the picker holds (`FUN_0051b819`): he
/// must stand in it, and then every copy of him goes, the rest shifting
/// down.  Answers whether any did.
pub fn send_home(squad: &mut Vec<u16>, id: u16) -> bool {
    let was = squad.len();
    squad.retain(|&player| player != id);
    squad.len() != was
}

/// Write the picked squad back into the team's block, as the menu's
/// `FUN_0063df96` copies the loaded club's list into the record: the
/// count at +0x112 and the ids at +0x1f2, the ids past the count left
/// standing as the original leaves them.
pub fn set_squad(records: &mut Records, nation: i16, squad: &[u16]) {
    if !usize::try_from(nation).is_ok_and(|n| n < NATIONS) {
        return;
    }
    let Some(team) = records.block_mut(&format!("{NATIONAL_TEAMS:#x}[{nation}]")) else {
        return;
    };
    team[NATIONAL_COUNT..NATIONAL_COUNT + 2].copy_from_slice(&(squad.len() as i16).to_le_bytes());
    for (k, &id) in squad.iter().enumerate() {
        let at = NATIONAL_SQUAD + 2 * k;
        if at + 2 > team.len() {
            break;
        }
        team[at..at + 2].copy_from_slice(&id.to_le_bytes());
    }
}

/// How many of the national squad are fit (`FUN_00650b84` asm 0x652426
/// and `FUN_0065270d` asm 0x6529cb: the club's squad list at its +0xe2 —
/// record 0x112 and 0x1f2 — one counted while his days of injury at his
/// +0x2c stand below a week).
pub fn fit_squad(records: &Records, nation: i16) -> usize {
    squad_ids(records, nation)
        .into_iter()
        .filter(|&player| {
            records
                .player(player)
                .is_some_and(|p| load::days_to(&records.globals, p.injured_to()) < FIT_DAYS)
        })
        .count()
}

/// Whether the week's Wednesday is an international day (`FUN_0067dc44`'s
/// first gate over `FUN_00629d80`): row 0 of the calendar, its top byte
/// one of the window flags, on `DAT_00b5cbd6 + 3`.  `FUN_00629d80` also
/// blanks the flag 0x02000000 under some condition (`FUN_0067bbed`);
/// either way it is no window.
pub fn window_wednesday(calendar: &Calendar, week_start: i16) -> bool {
    let wednesday = (i32::from(week_start) + 3).max(0) as usize;
    WINDOW_FLAGS.contains(&(calendar.entry(INTERNATIONAL_ROW, wednesday) & 0x7f00_0000))
}

/// The week hub's gate when its turn ends (`CMyStartDialog::FUN_00650b84`
/// asm 0x6523d3; NATIONALMANNSCHAFT.md §4.3): a coach of a nation, a
/// window on the week's Wednesday, his team playing this week, and fewer
/// than 18 fit in his squad — the turn does not end; the hub shows its
/// alert (text 4966) and code −1600 opens his menu again (`FUN_0051db39`).
/// What the original does for a big enough squad instead, and between —
/// `FUN_006bca8c` and `FUN_0063a317` marking the squad's players for the
/// match — is club state the replica keeps nowhere, as in `internationals`.
pub fn squad_blocks_turn(records: &Records, calendar: &Calendar, nation: i16) -> bool {
    window_wednesday(calendar, records.globals.week_start())
        && plays_this_week(records, nation)
        && fit_squad(records, nation) < FIT_ENOUGH
}

/// The same gate on the hub of a coach without a club (`FUN_0065270d`
/// asm 0x65292f): it asks nothing of the calendar and alerts for nothing,
/// only code −1600 back to his menu.
pub fn squad_blocks_clubless_turn(records: &Records, nation: i16) -> bool {
    plays_this_week(records, nation) && fit_squad(records, nation) < FIT_ENOUGH
}

/// The human in `slot` gives up his national team (`FUN_006b9872`): the
/// team's coach bytes and his key cleared, the nation off his record, the
/// last national station of his career closed for `reason`. The table of
/// who coaches whom at `0x1ba43b8` stays as it is, as in the original.
pub fn leave(records: &mut Records, slot: usize, reason: u8) {
    let id = slot + 1;
    let nation = records.globals.get::<u8>(KEY_NATION + id * KEY);
    if nation != 0
        && let Some(team) = records.block_mut(&format!("{NATIONAL_TEAMS:#x}[{nation}]"))
    {
        team[COACH] = 0;
        team[COACH + 1] = 0;
    }
    records.globals.put::<u8>(KEY_NATION + id * KEY, 0);
    records.globals.put::<u16>(KEY_TEAM + id * KEY, 0);
    if let Some(manager) = records.managers.get_mut(slot) {
        manager.set_national_team(0);
    }
    crate::career::left_national_team(records, slot, reason);
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3_files::an3::Save;
    use std::path::PathBuf;

    /// The picker's list over the original's save (`FUN_0051b6fe` and
    /// `FUN_0051b819`): a called-up player lands after the last one whose
    /// main position does not beat his — keepers first, not by age — the
    /// list stops at 22, takes neither id 0 nor one already in it, and
    /// sends home every copy of the player it removes; `FUN_0063df96`'s
    /// write-back comes back through `squad_ids` as written, the ids past
    /// the count standing where they were.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_picker_inserts_by_main_position_and_writes_back() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let path = dir.parent().unwrap().join("saves-kept/NewSeason000.an3");
        if !path.exists() {
            eprintln!("no NewSeason000 among the kept saves; the oracle's copy has it");
            return;
        }
        let mut records = Records::from_save(Save::read(&path).unwrap()).unwrap();
        // A first club's players, a second club's to reach past the cap;
        // their main positions, the byte the cache copies at +0x21d.
        let country = (1..0x75u16)
            .filter(|&c| records.countries.kind(usize::from(c)) != 0)
            .find(|&c| {
                records.countries.team_count(usize::from(c)) > 1
                    && records.block(&format!("{NATIONAL_TEAMS:#x}[{c}]")).is_some_and(|b| {
                        b.len() >= NATIONAL_SQUAD + 2 * (SQUAD_MOST + 1)
                    })
            })
            .unwrap();
        let name = format!("{NATIONAL_TEAMS:#x}[{country}]");
        let mut ids = records.team(country, 0).unwrap().players();
        ids.extend(records.team(country, 1).unwrap().players());
        ids.sort();
        ids.dedup();
        let position = |id: u16| records.player(id).map_or(0, |p| p.positions()[0]);
        let keeper = *ids.iter().find(|&&id| position(id) == 1).unwrap();
        let last = *ids.iter().max_by_key(|&&id| position(id)).unwrap();
        let mut squad = Vec::new();
        assert!(call_up(&mut squad, &records, last));
        assert!(call_up(&mut squad, &records, keeper));
        assert_eq!(squad, [keeper, last], "the keeper went in behind the outfielder");
        // The middle one: after the last entry not beating his position.
        let mid = *ids
            .iter()
            .find(|&&id| {
                let p = position(id);
                id != keeper && id != last && p > 1 && p <= position(last)
            })
            .unwrap();
        let before = squad.iter().filter(|&&id| position(id) <= position(mid)).count();
        assert!(call_up(&mut squad, &records, mid));
        assert_eq!(squad.iter().position(|&id| id == mid), Some(before));
        assert!(!call_up(&mut squad, &records, mid), "he stands in it already");
        assert!(!call_up(&mut squad, &records, 0), "id 0 goes nowhere");
        // The cap: 22 named, nothing more, every pair in order.
        for &id in &ids {
            call_up(&mut squad, &records, id);
        }
        assert_eq!(squad.len(), SQUAD_MOST);
        assert!(squad.windows(2).all(|w| position(w[0]) <= position(w[1])));
        assert_eq!(squad.iter().collect::<std::collections::HashSet<_>>().len(), SQUAD_MOST);
        assert!(!call_up(&mut squad, &records, *ids.last().unwrap()));
        // Home, and home again for a player listed twice.
        let gone = squad[0];
        assert!(send_home(&mut squad, gone));
        assert!(!send_home(&mut squad, gone));
        squad.extend([gone, gone]);
        assert!(send_home(&mut squad, gone));
        assert!(!squad.contains(&gone));
        // The write-back and its round trip; the id past the count stands.
        let at = NATIONAL_SQUAD + 2 * squad.len();
        let stale = records.block(&name).unwrap()[at..at + 2].to_vec();
        set_squad(&mut records, country as i16, &squad);
        let team = records.block(&name).unwrap();
        assert_eq!(i16::from_le_bytes([team[NATIONAL_COUNT], team[NATIONAL_COUNT + 1]]), squad.len() as i16);
        assert_eq!(team[at..at + 2].to_vec(), stale);
        assert_eq!(squad_ids(&records, country as i16), squad);
    }
}
