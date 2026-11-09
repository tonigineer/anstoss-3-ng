//! Being a national coach (NATIONALMANNSCHAFT.md in the RE repo): whether
//! the national team plays this week, and giving the job up.

use crate::records::Records;
use crate::season_odds::{NATIONAL_TEAMS, NATIONS};

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

/// Whether the national team of `nation` plays this week (`FUN_0067db00`):
/// its next match after the week's first day and before the next week's,
/// or, with `DAT_00b5dfc4` set, any match still to come.
pub fn plays_this_week(records: &Records, nation: i16) -> bool {
    if !usize::try_from(nation).is_ok_and(|n| n < NATIONS) {
        return false;
    }
    let Some(team) = records.block(&format!("{NATIONAL_TEAMS:#x}[{nation}]")) else {
        return false;
    };
    let next = i16::from_le_bytes([team[NEXT_DAY], team[NEXT_DAY + 1]]);
    let today = records.globals.get::<i16>(0xb5cbd4);
    let week = records.globals.week_start();
    (records.globals.get::<i32>(ANY_DAY_AHEAD) != 0 && next >= today)
        || (week < next && i32::from(next) < i32::from(week) + 7)
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
