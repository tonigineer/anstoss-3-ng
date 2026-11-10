//! Being a national coach (NATIONALMANNSCHAFT.md in the RE repo): whether
//! the national team plays this week, giving the job up, and looking for
//! foreign players to talk into changing their nation.

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

/// The talks about changing nation, as the coach's human slot keeps them
/// in the save (`FUN_00634f89`'s source, the block standing at
/// `DAT_00a55728` = the slot's object at `0xa55708` plus 0x20, which the
/// save leaves out): an i16 count at the block's +0 and 20 entries of
/// six bytes from +2 — the player, the flags and the day
/// (`FUN_006b8b51` writes all three, today at `DAT_00b5cbd4`).  The flags
/// say how far it came: 1 the first word ("Vorgespräch", text 3475), 2
/// the offer ("Angebot", 3476) and 4 his answer ("Entscheidung", 3477).
const TALKS_AT: u32 = 0xa55708;
const TALKS_COUNT: usize = 0;
const TALKS_FIRST: usize = 2;
const TALK: usize = 6;
const TALKS_MOST: usize = 0x14;
/// His answer, whatever it was: set, the coach is sent away when he
/// picks the player again (the click's alert 3474).
pub const ANSWERED: u8 = 4;
/// How many talks a coach may have on his slate before his association
/// refuses any more (`FUN_0051f715`'s gate on the copied count, alert
/// 3479): ten, the eleventh entry already alerts.
pub const TALKS_ALLOWED: usize = 10;

fn word(b: &[u8], at: usize) -> i16 {
    i16::from_le_bytes([b[at], b[at + 1]])
}

fn put16(b: &mut [u8], at: usize, v: i16) {
    b[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

/// What has been said to `player` in the talks of the human in `slot`
/// (`FUN_006b8f5f`): the flags of his entry over all twenty, 0 while he
/// stands in none — the id 0 the search never lists reads 0 as well.
pub fn talk_flags(records: &Records, slot: usize, player: u16) -> u8 {
    if player == 0 {
        return 0;
    }
    records.block(&format!("{TALKS_AT:#x}[{slot}]")).map_or(0, |b| {
        (0..TALKS_MOST)
            .find(|&k| word(b, TALKS_FIRST + k * TALK) as u16 == player)
            .map_or(0, |k| b[TALKS_FIRST + k * TALK + 2])
    })
}

/// How many talks the coach has on his slate (the count `FUN_00634f89`
/// copies to the front of its copy, and the search's click counts).
pub fn talk_count(records: &Records, slot: usize) -> usize {
    records
        .block(&format!("{TALKS_AT:#x}[{slot}]"))
        .map_or(0, |b| usize::try_from(word(b, TALKS_COUNT)).unwrap_or(0).min(TALKS_MOST))
}

/// The players picked for those talks, shared by the human coaches
/// (`0x1c53d98`; `FUN_007635f7`): an i16 count and up to 50 entries of
/// six bytes — the player, the day of the talk and the coach's human id
/// (his record's +0x1a).
const SEARCH_AT: u32 = 0x1c53d98;
const SEARCH: usize = 6;
const SEARCH_MOST: usize = 0x32;

/// Whether this very coach has picked `player` for a talk already
/// (`FUN_00763518`, the click's first gate, alert 3478): an entry
/// naming him and the coach's id after the day.
pub fn searched(records: &Records, coach: u16, player: u16) -> bool {
    records.block(&format!("{SEARCH_AT:#x}")).is_some_and(|b| {
        let count = usize::try_from(word(b, 0)).unwrap_or(0).min(SEARCH_MOST);
        (0..count).any(|k| {
            word(b, 2 + k * SEARCH) as u16 == player && word(b, 2 + k * SEARCH + 4) as u16 == coach
        })
    })
}

/// `player` on `day` for the coach `coach` into the search list
/// (`FUN_007635f7`): false when he stands in it already — under any
/// coach — or when the list is full at 50; else the entry goes after the
/// last and the count grows.
pub fn book_search(records: &mut Records, player: u16, day: i16, coach: u16) -> bool {
    let Some(b) = records.block_mut(&format!("{SEARCH_AT:#x}")) else {
        return false;
    };
    let count = usize::try_from(word(b, 0)).unwrap_or(0).min(SEARCH_MOST);
    if (0..count).any(|k| word(b, 2 + k * SEARCH) as u16 == player) || count >= SEARCH_MOST {
        return false;
    }
    let e = 2 + count * SEARCH;
    if b.len() < e + SEARCH {
        return false;
    }
    put16(b, e, player as i16);
    put16(b, e + 2, day);
    put16(b, e + 4, coach as i16);
    put16(b, 0, count as i16 + 1);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3_files::an3::Save;
    use std::path::PathBuf;

    /// The talks' table and the search list over the original's save
    /// (`FUN_006b8f5f`, `FUN_00763518` and `FUN_007635f7`): the flags
    /// read from all twenty entries, counted ones or not; the search
    /// takes a player once under any coach, remembers which coach for
    /// its alert, and the count grows with the entry.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_search_takes_him_once_and_the_talks_read_his_flags() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let path = dir.parent().unwrap().join("saves-kept/NewSeason000.an3");
        if !path.exists() {
            eprintln!("no NewSeason000 among the kept saves; the oracle's copy has it");
            return;
        }
        let mut records = Records::from_save(Save::read(&path).unwrap()).unwrap();
        let country = (1..0x75u16)
            .find(|&c| records.countries.team_count(usize::from(c)) > 0)
            .unwrap();
        let players = records.team(country, 0).unwrap().players();
        let (player, other) = (players[0], players[1]);
        // One entry, at the fifth, over a count left at 0.
        let today = records.globals.day() as i16;
        {
            let b = records.block_mut(&format!("{TALKS_AT:#x}[0]")).unwrap();
            b[TALKS_FIRST..TALKS_FIRST + TALKS_MOST * TALK].fill(0);
            put16(b, TALKS_COUNT, 0);
            let e = TALKS_FIRST + 5 * TALK;
            put16(b, e, player as i16);
            b[e + 2] = 3;
            put16(b, e + 4, today);
        }
        assert_eq!(talk_flags(&records, 0, player), 3, "the scan runs over all twenty");
        assert_eq!(talk_flags(&records, 0, other), 0);
        assert_eq!(talk_flags(&records, 0, 0), 0, "id 0 is asked about in neither direction");
        assert_eq!(talk_count(&records, 0), 0, "the count stands where it was");
        // The search list: one coach's pick, then another's.
        assert!(book_search(&mut records, player, 40, 1));
        assert!(searched(&records, 1, player));
        assert!(!searched(&records, 2, player), "no other coach has picked him");
        assert!(!book_search(&mut records, player, 44, 2), "he stands in the list already");
        assert!(book_search(&mut records, other, 41, 2));
        let b = records.block(&format!("{SEARCH_AT:#x}")).unwrap();
        assert_eq!(word(b, 0), 2);
        assert_eq!((word(b, 2), word(b, 4), word(b, 6)), (player as i16, 40, 1));
        assert_eq!((word(b, 8), word(b, 10), word(b, 12)), (other as i16, 41, 2));
        assert!(searched(&records, 2, other));
    }

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
