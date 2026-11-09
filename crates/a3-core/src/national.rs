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

/// How many of the national squad are fit (`FUN_00650b84` asm 0x652426
/// and `FUN_0065270d` asm 0x6529cb: the club's squad list at its +0xe2 —
/// record 0x112 and 0x1f2 — one counted while his days of injury at his
/// +0x2c stand below a week).
pub fn fit_squad(records: &Records, nation: i16) -> usize {
    let Some(team) = team_block(records, nation) else {
        return 0;
    };
    let count = i16::from_le_bytes([team[NATIONAL_COUNT], team[NATIONAL_COUNT + 1]]).max(0) as usize;
    (0..count)
        .filter_map(|k| team.get(NATIONAL_SQUAD + 2 * k..NATIONAL_SQUAD + 2 * k + 2))
        .map(|w| u16::from_le_bytes([w[0], w[1]]))
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
