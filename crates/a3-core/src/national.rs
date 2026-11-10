//! Being a national coach (NATIONALMANNSCHAFT.md in the RE repo): whether
//! the national team plays this week, giving the job up, looking for
//! foreign players to talk into changing their nation, and arranging his
//! team's friendlies in the international windows.

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

/// The talks as the overview reads them (`CEinbuergerungsgespraeche`'s
/// fill, `FUN_0048dad5`, over all twenty of `FUN_00634f89`'s copy): the
/// entries that hold a player, in the save's order — the id 0 of the
/// free ones stands for no talk — with each one's flags.
pub fn talks(records: &Records, slot: usize) -> Vec<(u16, u8)> {
    records.block(&format!("{TALKS_AT:#x}[{slot}]")).map_or_else(Vec::new, |b| {
        (0..TALKS_MOST)
            .filter_map(|k| {
                let id = word(b, TALKS_FIRST + k * TALK) as u16;
                (id != 0).then(|| (id, b[TALKS_FIRST + k * TALK + 2]))
            })
            .collect()
    })
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

/// The friendlies the coach arranges (`CNationalFreundschaftsspiel`):
/// three slots, one per friendly day of the year.
pub const FRIENDLY_SLOTS: usize = 3;
/// The national coaches' friendly records: the `(coach - 1)`-th of the
/// four blocks at `0xa55978`, the coach's number read off his team's
/// block (`FUN_0063b3c2`, `FUN_0063b488` and `FUN_0063b598` all stop at a
/// 0 there).  The block's data holds three slots of three six-byte
/// candidates from +0, a slot every `0x12` (what `FUN_0063b3c2` copies);
/// the entry booked per slot at +0x36, which `FUN_0063b510` writes back
/// on OK; and the played scores as u32 at +0x44, four past the slot.
const FRIENDLY_RECORDS: u32 = 0xa55978;
const FRIENDLY_CANDIDATES: usize = 0x0;
const FRIENDLY_CAND_SLOT: usize = 0x12;
const FRIENDLY_BOOKED: usize = 0x36;
const FRIENDLY_SCORES: usize = 0x44;
const FRIENDLY_PICK: usize = 6;
/// What a slot's score holds while it was not played (`FUN_0063b598`):
/// 0xff in its low byte.
const NO_SCORE: u8 = 0xff;

/// One candidate or booked friendly, the six bytes the record keeps
/// (`FUN_0063b3c2` and `FUN_0063b488` copy them whole): the land at byte
/// 0 — 0 for no opponent —, the team behind the name at +2, 200 for the
/// national team, and two bytes at +4 the dialog never reads.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct FriendlyPick(pub [u8; FRIENDLY_PICK]);

impl FriendlyPick {
    /// The land the opponent plays for, 0 for no one.
    pub fn land(&self) -> u8 {
        self.0[0]
    }

    /// His team's number behind the name, 200 the national team.
    pub fn team(&self) -> i16 {
        i16::from_le_bytes([self.0[2], self.0[3]])
    }
}

fn friendly_picks(b: &[u8], at: usize) -> [FriendlyPick; FRIENDLY_SLOTS] {
    let mut picks = [FriendlyPick::default(); FRIENDLY_SLOTS];
    for (k, pick) in picks.iter_mut().enumerate() {
        let at = at + k * FRIENDLY_PICK;
        if let Some(bytes) = b.get(at..at + FRIENDLY_PICK) {
            pick.0.copy_from_slice(bytes);
        }
    }
    picks
}

/// The coach's number in his team's block (`COACH`, read by the record
/// copy's gates): 1 to 4, 0 where the save names no coach.
pub fn coach_number(records: &Records, nation: i16) -> u8 {
    team_block(records, nation).map_or(0, |team| team[COACH])
}

/// The coach's friendly record, if a coach is named and the save keeps
/// the block.
fn friendly_record(records: &Records, nation: i16) -> Option<&[u8]> {
    let coach = usize::from(coach_number(records, nation));
    (coach != 0)
        .then(|| records.block(&format!("{FRIENDLY_RECORDS:#x}[{}]", coach - 1)))
        .flatten()
}

/// The candidates the dialog offers its three slots (`FUN_0063b3c2`):
/// three six-byte picks per slot out of the record — none for a coach
/// the record does not name.
pub fn friendly_candidates(records: &Records, nation: i16) -> [[FriendlyPick; FRIENDLY_SLOTS]; FRIENDLY_SLOTS] {
    let mut slots = [[FriendlyPick::default(); FRIENDLY_SLOTS]; FRIENDLY_SLOTS];
    if let Some(b) = friendly_record(records, nation) {
        for (k, slot) in slots.iter_mut().enumerate() {
            *slot = friendly_picks(b, FRIENDLY_CANDIDATES + k * FRIENDLY_CAND_SLOT);
        }
    }
    slots
}

/// What stands booked per slot today (`FUN_0063b488`'s copy).
pub fn friendly_booked(records: &Records, nation: i16) -> [FriendlyPick; FRIENDLY_SLOTS] {
    friendly_record(records, nation).map_or([FriendlyPick::default(); FRIENDLY_SLOTS], |b| friendly_picks(b, FRIENDLY_BOOKED))
}

/// OK's write-back (`FUN_0063b510`): the three entries as the dialog
/// holds them, into the record's +0x36 — even one the coach spun away
/// from, which the dialog only cleared on screen.
pub fn set_friendly_booked(records: &mut Records, nation: i16, booked: &[FriendlyPick; FRIENDLY_SLOTS]) {
    let coach = usize::from(coach_number(records, nation));
    if coach == 0 {
        return;
    }
    let Some(b) = records.block_mut(&format!("{FRIENDLY_RECORDS:#x}[{}]", coach - 1)) else {
        return;
    };
    for (k, pick) in booked.iter().enumerate() {
        let at = FRIENDLY_BOOKED + k * FRIENDLY_PICK;
        if at + FRIENDLY_PICK <= b.len() {
            b[at..at + FRIENDLY_PICK].copy_from_slice(&pick.0);
        }
    }
}

/// The slot's score once played (`FUN_0063b598`): none while its low
/// byte stands 0xff or the record names no coach; else the two bytes.
pub fn friendly_score(records: &Records, nation: i16, slot: usize) -> Option<(u8, u8)> {
    let at = FRIENDLY_SCORES + 4 * (slot + 1);
    let raw = friendly_record(records, nation)?.get(at..at + 4).map(|w| u32::from_le_bytes(w.try_into().unwrap()))?;
    (raw as u8 != NO_SCORE).then(|| (raw as u8, (raw >> 8) as u8))
}

/// The three friendly dates of the year (`FUN_0062ab93`): the first
/// three days of the calendar's international row whose flag is the
/// friendly flag, in their order.
pub fn friendly_dates(calendar: &Calendar) -> [usize; FRIENDLY_SLOTS] {
    let mut days = [0; FRIENDLY_SLOTS];
    let mut found = 0;
    for day in 0..crate::calendar::DAYS {
        if found == FRIENDLY_SLOTS {
            break;
        }
        if calendar.entry(INTERNATIONAL_ROW, day) & 0x7f00_0000 == 0x0400_0000 {
            days[found] = day;
            found += 1;
        }
    }
    days
}

/// The days still to come holding an international window
/// (`FUN_0062abfe`): from the day after today to the year's end, those
/// whose flag is one of the window flags — at most eight — kept as the
/// row has them, its low six bits the friendly slot or the match.
pub fn upcoming_days(calendar: &Calendar, today: usize) -> Vec<(i16, u32)> {
    ((today + 1)..crate::calendar::DAYS)
        .filter_map(|day| {
            let flag = calendar.entry(INTERNATIONAL_ROW, day);
            WINDOW_FLAGS.contains(&(flag & 0x7f00_0000)).then(|| (day as i16, flag))
        })
        .take(8)
        .collect()
}

/// The international dates the save keeps (`0x1ba07c0`): six entries of
/// `0xda` bytes each, a count of nations at +0 (at most 0x40) and those
/// nations as u16 from +2; the played and scheduled pairings from +0x9e,
/// ten six-byte columns of three pairs — every byte an index into the
/// entry's nations (`FUN_006b8149`).
const DATE_ENTRIES: u32 = 0x1ba07c0;
const DATE_ENTRY: usize = 0xda;
const DATE_NATIONS: usize = 2;
const DATE_PAIRS: usize = 0x9e;
const DATE_ENTRY_MOST: usize = 6;
const DATE_NATIONS_MOST: usize = 0x40;

/// The index of the date entry naming `nation` (`OnInit`'s scan of the
/// six), none when no entry holds it.
pub fn date_entry(records: &Records, nation: i16) -> Option<usize> {
    records.block(&format!("{DATE_ENTRIES:#x}")).and_then(|b| {
        (0..DATE_ENTRY_MOST).find(|&idx| {
            let Some(e) = b.get(idx * DATE_ENTRY..idx * DATE_ENTRY + DATE_ENTRY) else {
                return false;
            };
            let count = usize::try_from(word(e, 0)).unwrap_or(0).min(DATE_NATIONS_MOST);
            (0..count).any(|k| word(e, DATE_NATIONS + 2 * k) == nation)
        })
    })
}

/// The pairings in one column of the entry's pair rows (`FUN_006b8149`):
/// its three pairs of nations, a pair dropped when an index points past
/// the entry's count.
pub fn international_pairs(records: &Records, entry: usize, column: usize) -> Vec<(i16, i16)> {
    let Some(all) = records.block(&format!("{DATE_ENTRIES:#x}")) else {
        return Vec::new();
    };
    let Some(e) = all.get(entry * DATE_ENTRY..entry * DATE_ENTRY + DATE_ENTRY) else {
        return Vec::new();
    };
    let count = usize::try_from(word(e, 0)).unwrap_or(0).min(DATE_NATIONS_MOST);
    (0..3)
        .filter_map(|k| {
            let at = DATE_PAIRS + column * 6 + k * 2;
            let (a, b) = (usize::from(*e.get(at)?), usize::from(*e.get(at + 1)?));
            (a < count && b < count).then(|| (word(e, DATE_NATIONS + 2 * a), word(e, DATE_NATIONS + 2 * b)))
        })
        .collect()
}

/// The successes at the big tournaments (`CInfoNationalteam`'s table,
/// which its `OnInit` copies out with a 0x5418-byte `memcpy`): 117
/// records of 0xb8 bytes at `0xa7c260` — the row count, the year
/// counter at +2 (shown as its +1, stepping by 2 for a country with
/// national teams and by 4 without), ten result ids at +4 and ten
/// coach names of sixteen bytes at +0x18.  Nothing in the exe writes
/// the table, and no save block reaches it — save A's last two blocks
/// end at 0xa7c21c, just short — so it stands zero and the dialog's
/// list stays empty for every land (screen c1-item4-erfolge); the walk
/// is ported for the day the tournaments themselves are simulated.
const RESULTS_AT: u32 = 0xa7c260;
const RESULTS_MOST: usize = 117;
const RESULT_RECORD: usize = 0xb8;
const RESULT_ROWS: usize = 10;
/// The two contests' texts ("Europameisterschaft", "Weltmeisterschaft")
/// and the first of the seven result texts, "nicht qualifiziert" to
/// "Finale gewonnen": the record's ids count up from it (4190..4196).
pub const EUROPEAN_CUP: u16 = 4188;
pub const WORLD_CUP: u16 = 4189;
pub const FIRST_RESULT: u16 = 4190;

/// One tournament a land's team took part in, as the info dialog reads
/// it into a row.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Success {
    pub year: i32,
    pub contest: u16,
    pub result: u16,
    pub coach: String,
}

/// `land`'s tournament rows, newest first (`FUN_004cb3b2`): the slots
/// walked down from the counted one.  With national teams behind the
/// land (`DAT_0089e0b8[land]`), a row is a Weltmeisterschaft when the
/// counter leaves 0 or 1 mod 4 (`FUN_004cb372`) and an
/// Europameisterschaft else, and the counter steps by 2; without,
/// every row is a Weltmeisterschaft and it steps by 4.
pub fn successes(records: &Records, land: i16) -> Vec<Success> {
    let (Some(all), Ok(n)) = (records.block(&format!("{RESULTS_AT:#x}")), usize::try_from(land))
    else {
        return Vec::new();
    };
    if n >= RESULTS_MOST {
        return Vec::new();
    }
    all.get(n * RESULT_RECORD..n * RESULT_RECORD + RESULT_RECORD)
        .map_or_else(Vec::new, |r| rows(r, records.countries.has_teams(n)))
}

/// The big flags' names by land (`FUN_0069608b`'s pointer table at
/// `0x8a4890`, 117 of them): the info dialog loads
/// `gross/<name>.bmp` from the flags' archive, and the land it falls
/// back to outside 1..=116 is the 76th, the catch-all "Wel".
pub fn flag_names(exe: &a3_files::exe::Exe) -> std::io::Result<Vec<String>> {
    (0..117u32)
        .map(|n| {
            let at = a3_files::cpr::u32_at(exe.bytes(0x8a4890 + 4 * n, 4)?, 0)?;
            let raw = exe.bytes(at, 40)?;
            let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
            Ok(a3_files::sav::cp1252(&raw[..end]))
        })
        .collect()
}

/// The walk of one record, as the dialog's filler runs it.
fn rows(record: &[u8], has_team: bool) -> Vec<Success> {
    let count = usize::try_from(word(record, 0)).unwrap_or(0).min(RESULT_ROWS);
    let mut counter = word(record, 2);
    let mut out = Vec::new();
    for slot in (0..count).rev() {
        let contest = if !has_team || counter.rem_euclid(4) <= 1 { WORLD_CUP } else { EUROPEAN_CUP };
        let result = FIRST_RESULT + u16::try_from(word(record, 4 + slot * 2)).unwrap_or(0);
        let coach = a3_files::sav::cp1252(
            record[0x18 + slot * 0x10..].split(|&b| b == 0).next().unwrap_or_default(),
        );
        out.push(Success { year: i32::from(counter) + 1, contest, result, coach });
        counter -= if has_team { 2 } else { 4 };
    }
    out
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

    /// The friendly record of a coach poked into the save
    /// (`FUN_0063b3c2`, `FUN_0063b488`, `FUN_0063b510` and
    /// `FUN_0063b598`): three candidates per slot and what stands
    /// booked read from their offsets; the score only past its 0xff; the
    /// write-back lands where `FUN_0063b510` puts it; a nation without a
    /// coach reads nothing anywhere.  The date entries' pairs come out of
    /// the raw bytes of `0x1ba07c0` as `FUN_006b8149` reads them.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_friendly_record_holds_candidates_booked_and_scores() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let path = dir.parent().unwrap().join("saves-kept/NewSeason000.an3");
        if !path.exists() {
            eprintln!("no NewSeason000 among the kept saves; the oracle's copy has it");
            return;
        }
        let mut records = Records::from_save(Save::read(&path).unwrap()).unwrap();
        let nation = (1..0x75i16)
            .find(|&n| team_block(&records, n).is_some())
            .unwrap();
        records.block_mut(&format!("{NATIONAL_TEAMS:#x}[{nation}]")).unwrap()[COACH] = 2;
        {
            let b = records.block_mut(&format!("{FRIENDLY_RECORDS:#x}[1]")).unwrap();
            for k in 0..FRIENDLY_SLOTS {
                for p in 0..FRIENDLY_SLOTS {
                    let at = FRIENDLY_CANDIDATES + k * FRIENDLY_CAND_SLOT + p * FRIENDLY_PICK;
                    b[at] = (p + 1) as u8;
                    b[at + 2] = 200;
                }
                let at = FRIENDLY_BOOKED + k * FRIENDLY_PICK;
                b[at..at + FRIENDLY_PICK].fill(0);
            }
            b[FRIENDLY_BOOKED] = 3;
            b[FRIENDLY_BOOKED + 2] = 200;
            b[FRIENDLY_SCORES + 4] = 0xff;
            b[FRIENDLY_SCORES + 8] = 3;
            b[FRIENDLY_SCORES + 9] = 1;
            b[FRIENDLY_SCORES + 12] = 0xff;
        }
        let cands = friendly_candidates(&records, nation);
        assert_eq!(cands[0][2].land(), 3);
        assert_eq!(cands[2][0].team(), 200);
        let booked = friendly_booked(&records, nation);
        assert_eq!((booked[0].land(), booked[0].team()), (3, 200));
        assert_eq!(booked[1], FriendlyPick::default(), "the free slots stand at 0");
        assert_eq!(friendly_score(&records, nation, 0), None, "0xff is no score");
        assert_eq!(friendly_score(&records, nation, 1), Some((3, 1)));
        assert_eq!(friendly_score(&records, nation, 2), None);
        // The write-back of a spun-away pick, which the original still
        // books: the second slot takes the third candidate.
        let mut booked = friendly_booked(&records, nation);
        booked[1] = cands[1][2];
        set_friendly_booked(&mut records, nation, &booked);
        assert_eq!(&records.block(&format!("{FRIENDLY_RECORDS:#x}[1]")).unwrap()[FRIENDLY_BOOKED + FRIENDLY_PICK..FRIENDLY_BOOKED + 2 * FRIENDLY_PICK], &cands[1][2].0);
        assert_eq!(friendly_booked(&records, nation)[1], cands[1][2]);
        // No coach, no record.
        records.block_mut(&format!("{NATIONAL_TEAMS:#x}[{nation}]")).unwrap()[COACH] = 0;
        assert_eq!(friendly_booked(&records, nation), [FriendlyPick::default(); FRIENDLY_SLOTS]);
        assert_eq!(friendly_candidates(&records, nation)[0], [FriendlyPick::default(); FRIENDLY_SLOTS]);
        assert_eq!(friendly_score(&records, nation, 1), None);
        // The date entries: an entry naming a nation, and its pairs by
        // their bytes.
        if let Some(b) = records.block(&format!("{DATE_ENTRIES:#x}")) {
            let entry = (0..DATE_ENTRY_MOST).find(|&idx| {
                let e = &b[idx * DATE_ENTRY..idx * DATE_ENTRY + DATE_ENTRY];
                let count = usize::try_from(word(e, 0)).unwrap_or(0).min(DATE_NATIONS_MOST);
                (0..count).any(|k| word(e, DATE_NATIONS + 2 * k) == nation)
            });
            assert_eq!(date_entry(&records, nation), entry, "the scan finds the same entry");
            if let Some(entry) = entry {
                let e = &b[entry * DATE_ENTRY..entry * DATE_ENTRY + DATE_ENTRY];
                let count = usize::try_from(word(e, 0)).unwrap_or(0).min(DATE_NATIONS_MOST);
                for column in 0..10 {
                    let want: Vec<(i16, i16)> = (0..3)
                        .filter_map(|k| {
                            let at = DATE_PAIRS + column * 6 + k * 2;
                            let (a, c) = (usize::from(e[at]), usize::from(e[at + 1]));
                            (a < count && c < count).then(|| (word(e, DATE_NATIONS + 2 * a), word(e, DATE_NATIONS + 2 * c)))
                        })
                        .collect();
                    assert_eq!(international_pairs(&records, entry, column), want, "column {column}");
                }
            }
        }
    }

    /// The walk of `FUN_004cb3b2` over a crafted record: rows newest
    /// first, the contest by the counter's remainder mod 4 and its step,
    /// the result ids counting up from 4190 and the coaches cut at their
    /// NUL — and, without national teams behind the land, every row a
    /// Weltmeisterschaft with the counter leaping by 4.
    #[test]
    fn the_walk_reads_the_rows_newest_first() {
        let mut record = [0u8; RESULT_RECORD];
        record[0..2].copy_from_slice(&3i16.to_le_bytes());
        record[2..4].copy_from_slice(&2004i16.to_le_bytes());
        for (slot, id) in [(2, 0i16), (1, 5), (0, 6)] {
            record[4 + slot * 2..6 + slot * 2].copy_from_slice(&id.to_le_bytes());
        }
        for (slot, name) in [(2, "Völler"), (1, "Löw"), (0, "Klinsmann")] {
            let at = 0x18 + slot * 0x10;
            let bytes: Vec<u8> = name
                .chars()
                .map(|c| a3_files::bitmap_font::to_cp1252(c).unwrap_or(b'?'))
                .collect();
            record[at..at + bytes.len()].copy_from_slice(&bytes);
        }
        let named = rows(&record, true);
        assert_eq!(
            named,
            [
                Success { year: 2005, contest: WORLD_CUP, result: FIRST_RESULT, coach: "Völler".into() },
                Success { year: 2003, contest: EUROPEAN_CUP, result: FIRST_RESULT + 5, coach: "Löw".into() },
                Success { year: 2001, contest: WORLD_CUP, result: FIRST_RESULT + 6, coach: "Klinsmann".into() },
            ]
        );
        let once = {
            let mut r = [0u8; RESULT_RECORD];
            r[0..2].copy_from_slice(&2i16.to_le_bytes());
            r[2..4].copy_from_slice(&2006i16.to_le_bytes());
            rows(&r, false)
        };
        assert_eq!(
            once,
            [
                Success { year: 2007, contest: WORLD_CUP, result: FIRST_RESULT, coach: String::new() },
                Success { year: 2003, contest: WORLD_CUP, result: FIRST_RESULT, coach: String::new() },
            ]
        );
    }
}
