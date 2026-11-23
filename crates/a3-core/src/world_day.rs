//! The world's day (`FUN_006504b6` running `FUN_007848fa` with the day
//! of the week): on the first weekday every club of the countries
//! played besides the main one lives its day, on the second the main
//! country's clubs do, on the third the clubs of the countries not
//! played rest ([`rest_of_world`]); the game day counter stands in
//! `DAT_00b5cbd4`, and the day's close steps it and the date
//! (`step_date`), which every reckoning of days out and the autosaves
//! go by.
//!
//! One club's day (`FUN_00784a6d`) goes over every player of its
//! squad: a roll of a hundred for the player's mood (`FUN_007849f3`,
//! whose two weight tables only decide what follows the roll), a dose
//! wearing off ([`wear_off`], `FUN_006ff7ef`), and, for the computer
//! clubs, the day off (`FUN_00786622`, [`day_off`]): three points of
//! condition go, five of freshness come, every part of his contentment
//! steps nearer a hundred, and a roll of ten thousand that comes out 0
//! or 1 rolls a seven more. A club with humans at its record's +0xa8 —
//! a trainer and a manager by number — instead rolls a three after its
//! squad, and the roll of a hundred on its run-time word +0xa7 stands
//! for every club whose word is set. Its will to win steps nearer
//! nought (`FUN_0069f0b5`).
//!
//! The driver then rounds the human clubs of the globals' list once a
//! day ([`luck_round_of_list`]): every squad player shifts his
//! freshness, loses fifty of his condition, and rolls seven of the
//! tens — a 0-roll nudges the contentment part it stands for a step
//! nearer a hundred ([`nudge`], [`crate::mood::change`]). Behind the
//! counter's
//! increment the day walks its schedule table once ([`schedule_walk`]):
//! a roll of a hundred steps the walk index and the index rolls over
//! the table's rows ([`schedule_day`]).
//!
//! Not ported yet, and drawing in the original when they run: that
//! hundred-roll on +0xa7, whose table no save hands the replica, and
//! what the rolls answer to — the moods, the play and the club's
//! standing, and the club day's form and duty bytes (`FUN_006ff037`,
//! `FUN_0070528c`, `FUN_00702ebc`, `FUN_007061c4`); the weekly parts of
//! the other weekdays (`FUN_00417a8a` with its two-rolls on the second,
//! `FUN_00648e0d` with hundreds on the fourth, `FUN_00786080` on the
//! sixth).

use crate::computer_training::{cond_percent, fresh_percent, train};
use crate::mood;
use crate::random::Random;
use crate::records::{Player, Records, Team};

/// The day of the world: `weekday` 0 sends the clubs of the more
/// countries through [`club`], 1 those of the main country; the other
/// weekdays' steps are not here.
pub fn day(records: &mut Records, weekday: i32, random: &mut Random) {
    match weekday {
        0 => {
            let countries = records.globals.more_countries();
            let more = records.globals.more_count().max(0) as usize;
            for k in 0..more.min(countries.len()) {
                clubs_of(records, countries[k], random);
            }
        }
        1 => clubs_of(records, records.globals.main_country(), random),
        2 => rest_of_world(records, random),
        _ => {}
    }
    luck_round_of_list(records, random);
    // The game's day counter steps on (`DAT_00b5cbd4`), then the day's
    // step (`FUN_00675a73(1)`): the schedule walk, the date after it.
    records.globals.set_day(records.globals.day().wrapping_add(1));
    schedule_walk(&*records, random);
    if step_date(&mut records.globals) {
        fill_day_table(records);
    }
}

/// The days of the months (`DAT_008a3088`, by the month; February's
/// stand apart, by the leap rule).
const MONTH_DAYS: [u16; 13] = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

/// Whether a year has 366 days (`FUN_00675e65`).
fn leap(year: u16) -> bool {
    year.is_multiple_of(400) || (!year.is_multiple_of(100) && year.is_multiple_of(4))
}

/// The date one day on (`FUN_00675a73(1)` behind its schedule walk): the
/// day of the month and the day of the year count up; past the month's
/// last day — February's 28th, or its 29th in a leap year — the month
/// steps, and past December the year: January then, its day of the year
/// counted from nought and the year's length set anew. Returns whether
/// the year rolled — [`day`] then lays the day table down anew
/// ([`fill_day_table`]). What a new month runs (`FUN_007853a7`) is not
/// here.
fn step_date(globals: &mut crate::records::Globals) -> bool {
    let [mut day, mut month, mut year] = globals.date();
    let mut of_year = globals.day_of_year().wrapping_add(1);
    day = day.wrapping_add(1);
    let mut rolled = false;
    if month == 2 {
        let last = if leap(year) { 29 } else { 28 };
        if last < day {
            day -= last;
            month = 3;
        }
    } else if let Some(&last) = MONTH_DAYS.get(usize::from(month))
        && last < day
    {
        day -= last;
        month += 1;
        if 12 < month {
            of_year = day.wrapping_sub(1);
            month = 1;
            year = year.wrapping_add(1);
            globals.set_year_days(if leap(year) { 366 } else { 365 });
            rolled = true;
        }
    }
    globals.set_date([day, month, year]);
    globals.set_day_of_year(of_year);
    rolled
}

/// The block the day table lives in (`DAT_00b1f860`, shared with the
/// archive and the betting lists).
pub(crate) const DAY_TABLE: &str = "0xb1f860";
/// Where the table's pairs of days begin in the block (`DAT_00b1f874`).
const DAY_PAIRS: usize = 0x14;

/// The table of the coming days (`FUN_00675eb3`), laid down when the
/// calendar rolls: at a year's end within [`day`] and at the season's
/// opening ([`crate::settlement`]). A booking `k` days out takes its date
/// from entry `day_of_year + k` there (`FUN_00637177`).
pub(crate) fn fill_day_table(records: &mut Records) {
    let (year, of_year) = (records.globals.date()[2], records.globals.day_of_year());
    let block = records
        .block_mut(DAY_TABLE)
        .expect("the block the day table lives in");
    fill_day_pairs(block, year, of_year);
}

/// Both fills of the table: from the day of the year on, every of the
/// 2000 entries names its year and its day within that year, the entries
/// before the starting day left as they stand; the ten u16s at the head
/// count up the days of the ten years after the date's year.
fn fill_day_pairs(block: &mut [u8], base: u16, of_year: u16) {
    let (mut year, mut of_year, mut len) =
        (base, u32::from(of_year), if leap(base) { 366 } else { 365 });
    for k in of_year..2000 {
        let at = DAY_PAIRS + k as usize * 4;
        block[at..at + 2].copy_from_slice(&year.to_le_bytes());
        block[at + 2..at + 4].copy_from_slice(&(of_year as u16).to_le_bytes());
        of_year += 1;
        if len <= of_year {
            of_year = 0;
            year += 1;
            len = if leap(year) { 366 } else { 365 };
        }
    }
    let mut acc: u16 = 0;
    for j in 0..10 {
        block[j * 2..j * 2 + 2].copy_from_slice(&acc.to_le_bytes());
        acc += if leap(base.wrapping_add(j as u16 + 1)) { 366 } else { 365 };
    }
}

/// The day's step behind the counter's increment (`FUN_00675a73(1)`):
/// one roll of a hundred nudges the walk index at +0xb5df6a one step
/// towards its phase's target ([`walk_next`], `FUN_00783dc3`), and the
/// index then picks the day's rolls over the schedule table
/// ([`schedule_day`], `FUN_00783ea3`), which [`rotate`] has just
/// shifted by the walk's pair (the rotation `FUN_007842a1` draws
/// nothing); the day then re-rolls the table's fives ([`reroll_fives`])
/// and in the late phases fades some of its threes ([`fade_threes`],
/// `FUN_0078418b`). The phase is the date's month (+0xb5cbdc), which
/// [`step_date`] moves on after the walk; the shifted table is not
/// written back.
fn schedule_walk(records: &Records, random: &mut Random) {
    /// The walk's target per phase (`DAT_008acfec`, `FUN_00675a73`'s
    /// `_DAT_00b5cbdc` index).
    const TARGETS: [i16; 13] = [0, 5, 4, 3, 3, 2, 1, 0, 0, 1, 2, 3, 4];
    /// The table's shift per walk index (`DAT_008ad008`, handed to
    /// `FUN_007842a1` beside the index).
    const SHIFTS: [(i16, i16); 6] = [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1)];
    let phase = records.globals.get::<i16>(0xb5cbdc);
    let target = TARGETS.get(phase.max(0) as usize).copied().unwrap_or(0);
    let walk = records.globals.get::<i16>(0xb5df6a);
    let roll = i16::try_from(random.below(100)).unwrap();
    let walk = walk_next(walk, target, roll);
    let mut table: [u8; TABLE] = records.globals.get(0xb5d92a);
    if let Some(&(dx, dy)) = usize::try_from(walk).ok().and_then(|w| SHIFTS.get(w)) {
        rotate(&mut table, dx, dy);
    }
    schedule_day(walk, &mut table, random);
    reroll_fives(&mut table, random);
    if 5 < phase && phase < 9 {
        fade_threes(&mut table, random);
    }
}

/// The schedule table: forty rows of forty cells of small numbers,
/// at +0xb5d92a in the globals.
const TABLE: usize = 1600;

/// The table's day shift (`FUN_007842a1`): its cells step by the walk's
/// shift pair, and the rows and columns they shift out of keep their
/// old numbers — the table is copied first, and only the shifted cells
/// are written again.
fn rotate(table: &mut [u8; TABLE], dx: i16, dy: i16) {
    let source = *table;
    let (x0, x1): (i32, i32) = if dx < 0 { (1, 40) } else if dx == 0 { (0, 40) } else { (0, 39) };
    let (y0, y1): (i32, i32) = if dy < 0 { (1, 40) } else if dy == 0 { (0, 40) } else { (0, 39) };
    for x in x0..x1 {
        for y in y0..y1 {
            let at = usize::try_from(y + x * 40).unwrap();
            let to = usize::try_from(y + i32::from(dy) + (x + i32::from(dx)) * 40).unwrap();
            table[to] = source[at];
        }
    }
}

/// The day's numbers over the schedule table (`FUN_00783ea3`): the walk
/// index says which rows and columns get new numbers out of its rolls.
fn schedule_day(walk: i16, table: &mut [u8; TABLE], random: &mut Random) {
    let mut roll = |range: u32| u8::try_from(random.below(range)).unwrap();
    match walk {
        0 => {
            for row in 0..40 {
                table[39 + row * 40] = roll(2);
            }
        }
        1 => {
            for row in 0..40 {
                table[39 + row * 40] = roll(2);
            }
            for column in 0..20 {
                table[column] = roll(5) + 1;
            }
            for column in 20..40 {
                table[column] = roll(3) + 1;
            }
        }
        2 => {
            for column in 0..20 {
                table[column] = roll(5) + 1;
            }
            for column in 20..40 {
                table[column] = roll(3) + 1;
            }
        }
        3 => {
            for column in 0..20 {
                table[column] = roll(4);
            }
            for column in 20..40 {
                table[column] = roll(3) + 1;
            }
            for row in 0..40 {
                table[row * 40] = roll(4) + 1;
            }
        }
        4 => {
            for row in 0..40 {
                table[row * 40] = roll(4) + 1;
            }
        }
        5 => {
            for row in 0..40 {
                table[row * 40] = roll(3) + 1;
            }
            for column in 0..40 {
                table[1560 + column] = roll(4);
            }
        }
        _ => {}
    }
}

/// The day's re-roll of the table's fives (`FUN_0078418b`'s first
/// loop): every five over the whole table rolls a two and stands
/// again as a two or a three.
fn reroll_fives(table: &mut [u8; TABLE], random: &mut Random) {
    for cell in table.iter_mut() {
        if *cell == 5 {
            *cell = 2 + u8::try_from(random.below(2)).unwrap();
        }
    }
}

/// The late season's fading of the table's threes (`FUN_0078418b`'s
/// second loop, only in phases six to eight): every three rolls a
/// hundred, and a ten of them turn the cell into a six.
fn fade_threes(table: &mut [u8; TABLE], random: &mut Random) {
    for cell in table.iter_mut() {
        if *cell == 3 && random.below(100) < 10 {
            *cell = 6;
        }
    }
}

/// A day's nudge of the walk index (`FUN_00783dc3`): below eighty it
/// climbs towards the target, between eighty and ninety it only climbs
/// out of the two lowest rows, and from ninety it sinks.
fn walk_next(walk: i16, target: i16, roll: i16) -> i16 {
    if walk == target && roll < 0x50 {
        return walk;
    }
    if walk < target && roll < 0x50 {
        walk + 1
    } else if target < walk && roll < 0x50 {
        walk - 1
    } else if roll < 0x5a {
        if walk < 2 {
            walk + 1
        } else {
            walk
        }
    } else if 0 < walk {
        walk - 1
    } else {
        walk
    }
}

/// The round the day's driver runs after the weekday switch, over the
/// globals' list of human clubs (`FUN_007845aa` behind the wrapper
/// `FUN_007848f0`): the list stands at +0xb5cbe2 — one entry eight bytes
/// apart, the country's byte and the club's number two further — and its
/// count at +0xb5cc02; on every club every squad player rolls seven of
/// the tens, [`luck_round`].
fn luck_round_of_list(records: &mut Records, random: &mut Random) {
    let list: [u8; 32] = records.globals.get(0xb5cbe2);
    let count = records.globals.get::<i16>(0xb5cc02).max(0) as usize;
    for entry in list.chunks(8).take(count) {
        let country = u16::from(entry[0]);
        let number = u16::from_le_bytes([entry[2], entry[3]]);
        let Some(team) = records.team(country, number) else { continue };
        let ids = team.players();
        let human = team.humans() != [0, 0];
        let manager = human
            .then(|| records.managers.iter().find(|m| m.country() == country && m.team() == number))
            .flatten();
        let rank = manager.map_or(3, |m| (m.difficulty() & 0x7f).min(4) as usize);
        for id in ids {
            if let Some(player) = records.player_mut(id) {
                luck_player(player, FRESHENED[rank], random);
            }
        }
    }
}

/// The freshness a day's round adds to a squad player, by the class of
/// his club's manager (`FUN_007845aa`'s table at `DAT_008ad040`); a
/// club the game runs itself, with no manager of its own, sits at the
/// fourth.
const FRESHENED: [i16; 5] = [89, 85, 81, 70, 64];

/// One player's turn in the round (`FUN_007845aa`'s player body): the
/// last session's freshness stands in for the open one, that one drops
/// to nought, the season's gains a manager's step and stop at ten
/// thousand, and fifty of condition goes; then seven rolls of ten, a
/// 0-roll nudging one contentment part ([`nudge`]) in the loop's order:
/// trainer, team, private life, distractions, press, club, offers.
fn luck_player(player: &mut Player, freshened: i16, random: &mut Random) {
    let mut freshness = player.freshness();
    freshness[1] = freshness[2];
    freshness[2] = 0;
    freshness[0] = freshness[0].wrapping_add(freshened);
    if 10_000 < freshness[0] {
        freshness[0] = 10_000;
    }
    player.set_freshness(freshness);
    player.set_condition((i32::from(player.condition()) - 50).clamp(0, 10_000) as i16);
    for slot in [0, 1, 4, 5, 3, 2, 6] {
        if random.below(10) == 0 {
            nudge(player, slot);
        }
    }
}

/// A contentment part a step nearer a hundred, nought and the parts
/// above it staying put; his morale moves with it (`FUN_007007a1` and
/// its six twins, each called behind its slot's 0-roll).
fn nudge(player: &mut Player, slot: usize) {
    let value = i32::from(player.contentment()[slot]);
    if value < 100 {
        mood::change(player, slot, 1);
    } else if 100 < value {
        mood::change(player, slot, -1);
    }
}

/// The clubs of one country, the records' count of them in the order
/// they stand in their country (`FUN_00784f27` and `FUN_00784ee8`).
fn clubs_of(records: &mut Records, country: i16, random: &mut Random) {
    let Ok(id) = u16::try_from(country) else {
        return;
    };
    for number in 0..records.countries.team_count(id as usize).max(0) {
        let Ok(number) = u16::try_from(number) else {
            continue;
        };
        club(records, (id, number), random);
    }
}

/// One club's day (`FUN_00784a6d`): every squad player gets the roll
/// of a hundred, his dose wears off, and — unless the club is a human
/// one at its record's +0xa8 — he has his day off with the roll of ten
/// thousand in it; the human club rolls a three once after its squad
/// instead. The club's will to win steps nearer nought between.
fn club(records: &mut Records, (country, number): (u16, u16), random: &mut Random) {
    let Some(team) = records.team(country, number) else { return };
    let human = team.humans() != [0, 0];
    for id in team.players() {
        random.below(100);
        let mut player = records.player_mut(id);
        if let Some(player) = player.as_deref_mut() {
            wear_off(player);
        }
        if !human {
            day_off(player, random);
        }
    }
    if let Some(team) = records.team_mut(country, number) {
        calm(team);
    }
    if human {
        random.below(3);
    }
}

/// A dose wearing off (`FUN_006ff7ef`): the two bits of his doping
/// byte that count it down shift once, and while it still holds, the
/// condition kind (`doping::CONDITION`) takes five points of it again.
fn wear_off(player: &mut Player) {
    let byte = player.doping();
    let byte = (byte & DOSE) >> 1 | byte & !DOSE;
    player.set_doping(byte);
    if byte & DOSE != 0 && (byte >> 2) & 3 == crate::doping::CONDITION {
        player.set_condition(train(player.condition(), -5));
    }
}

/// The bits of the doping byte that count a dose down.
const DOSE: u8 = 0x03;

/// A computer club player's day off (`FUN_00786622`): three points of
/// condition go and five of freshness come, each part of his
/// contentment steps nearer a hundred in the round's order, and a roll
/// of ten thousand that comes out 0 or 1 moves his contentment with
/// the trainer or the team by a seven's roll less three.
fn day_off(mut player: Option<&mut Player>, random: &mut Random) {
    if let Some(player) = player.as_deref_mut() {
        player.set_condition(train(player.condition(), -3));
        let mut fresh = player.freshness();
        fresh[0] = train(fresh[0], 5);
        player.set_freshness(fresh);
        for slot in [0, 1, 4, 5, 3, 2, 6] {
            nudge(player, slot);
        }
    }
    let slot = random.below(10_000) as usize;
    if slot < 2 {
        let by = random.below(7) as i32 - 3;
        if let Some(player) = player {
            mood::change(player, slot, by);
        }
    }
}

/// A club's will to win a step nearer nought (`FUN_0069f0b5`).
fn calm(team: &mut Team) {
    let will = team.will_to_win();
    team.set_will_to_win(will - will.signum());
}

/// The rest of the world's week (`FUN_00786931` on the third weekday):
/// the clubs of every country not played (`FUN_00676c03`) rest
/// ([`rest`]) up to the season's bounds ([`rest_bounds`]).
fn rest_of_world(records: &mut Records, random: &mut Random) {
    let bounds = rest_bounds(records.globals.day() as i16);
    for country in 1..0x75i16 {
        if records.globals.played(country) {
            continue;
        }
        for number in 0..records.countries.team_count(country as usize).max(0) {
            rest(records, (country as u16, number as u16), bounds, random);
        }
    }
}

/// How fit and how fresh the resting clubs get (`FUN_00786424` with
/// 0x10), in percent, by the game's day: the first six weeks' table
/// (`DAT_008ad0f8`, `DAT_008ad104`), then eighty less a fifteenth and
/// an eighth of the day, and from the day 322 on seventy and eighty.
fn rest_bounds(day: i16) -> (i16, i16) {
    if day >= 0x142 {
        (70, 80)
    } else if day < 0x2a {
        let week = (day / 7).max(0) as usize;
        (REST_CONDITION[week], REST_FRESHNESS[week])
    } else {
        (0x50 - day / 15, 0x50 - day / 8)
    }
}

/// The first six weeks' bounds of the resting clubs (`DAT_008ad0f8`,
/// `DAT_008ad104`).
const REST_CONDITION: [i16; 6] = [50, 54, 58, 62, 66, 70];
const REST_FRESHNESS: [i16; 6] = [80, 30, 30, 40, 60, 65];

/// One resting club (`FUN_0078680a`): its bounds moved by a roll of ten
/// up and five down, every player two points of condition and three of
/// freshness richer — half of what then stands above a bound going
/// again — and his mood's roll of a hundred; then its will to win steps
/// nearer nought.
fn rest(records: &mut Records, (country, number): (u16, u16), (condition, freshness): (i16, i16), random: &mut Random) {
    let condition = (condition + random.below(10) as i16).min(100);
    let freshness = (freshness - random.below(5) as i16).max(0);
    let Some(team) = records.team(country, number) else { return };
    for id in team.players() {
        if let Some(player) = records.player_mut(id) {
            rest_man(player, (condition, freshness));
        }
        random.below(100);
    }
    if let Some(team) = records.team_mut(country, number) {
        calm(team);
    }
}

/// One man's rest (`FUN_0078680a`'s player body): two points of
/// condition, half of what then stands above the club's bound going
/// again, and three of freshness the same way.
fn rest_man(player: &mut Player, (condition, freshness): (i16, i16)) {
    player.set_condition(train(player.condition(), 2));
    let now = cond_percent(player);
    if condition < now {
        player.set_condition(train(player.condition(), -(now - condition) / 2));
    }
    let mut fresh = player.freshness();
    fresh[0] = train(fresh[0], 3);
    player.set_freshness(fresh);
    let now = fresh_percent(player);
    if freshness < now {
        let mut fresh = player.freshness();
        fresh[0] = train(fresh[0], -(now - freshness) / 2);
        player.set_freshness(fresh);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rest of the world's week against the original's: from the
    /// oracle's dump of Probe1's day 37 to the one of day 38, the third
    /// weekday's walk, every club of the countries not played rests as
    /// one roll of ten and one of five on the season's bounds have it —
    /// the rolls themselves come behind the day's matches, whose draws
    /// are not the replica's yet. Most of them gain the whole two and
    /// three points; some stand at a bound.
    #[test]
    #[ignore = "reads the oracle's dumps from $ANSTOSS3_ORACLE"]
    fn the_world_rests_as_the_original() {
        use a3_files::an3::Save;
        let Some(oracle) = std::env::var_os("ANSTOSS3_ORACLE") else {
            eprintln!("no oracle dumps of Probe1's week among the saves: ANSTOSS3_ORACLE is not set");
            return;
        };
        let dumps = std::path::PathBuf::from(oracle).join("runs/check/save-a");
        if !dumps.join("day-038.an3").exists() {
            eprintln!("no oracle dumps of Probe1's week among the saves; tools/oracle/check.sh writes them");
            return;
        }
        let dump = |day: u16| Records::from_save(Save::read(&dumps.join(format!("day-{day:03}.an3"))).unwrap()).unwrap();
        let (before, after) = (dump(37), dump(38));
        assert_eq!(before.globals.day() % 7, 2);
        let (condition, freshness) = rest_bounds(before.globals.day() as i16);
        assert_eq!((condition, freshness), (70, 65));
        let (mut clubs, mut bound) = (0, 0);
        for country in 1..0x75i16 {
            if before.globals.played(country) {
                continue;
            }
            for number in 0..before.countries.team_count(country as usize).max(0) as u16 {
                let Some(team) = before.team(country as u16, number) else { continue };
                let squad = team.players();
                // A club abroad in a tournament here wears its match on
                // top (its men's third freshness word moves).
                let fresh = |records: &Records, id: u16| records.player(id).unwrap().freshness()[2];
                if squad.iter().any(|&id| fresh(&after, id) != 0 && fresh(&after, id) != fresh(&before, id)) {
                    continue;
                }
                let rests = |rolls: (i16, i16)| {
                    let bounds = ((condition + rolls.0).min(100), (freshness - rolls.1).max(0));
                    squad.iter().all(|&id| {
                        let mut man = before.player(id).unwrap().clone();
                        rest_man(&mut man, bounds);
                        let theirs = after.player(id).unwrap();
                        (man.condition(), man.freshness()) == (theirs.condition(), theirs.freshness())
                    })
                };
                let rolls: Vec<(i16, i16)> = (0..10).flat_map(|a| (0..5).map(move |b| (a, b))).filter(|&r| rests(r)).collect();
                assert!(!rolls.is_empty(), "{country}/{number} rests otherwise than any rolls");
                clubs += 1;
                bound += usize::from(rolls.len() < 50);
            }
        }
        assert!(100 < clubs && 0 < bound, "{clubs} clubs rested, {bound} at a bound");
    }

    /// The date's day step: within a month, over a month's end, over
    /// February's in a leap year and not, and over the year's end, the
    /// day of the year counted from nought in the new January.
    #[test]
    fn the_date_steps_over_months_and_years() {
        let zeros = vec![0; crate::records::Globals::SIZE];
        let mut globals = crate::records::Globals::new(&zeros).unwrap();
        let step = |globals: &mut crate::records::Globals, date: [u16; 3], of_year: u16| {
            globals.set_date(date);
            globals.set_day_of_year(of_year);
            step_date(globals);
            (globals.date(), globals.day_of_year())
        };
        assert_eq!(step(&mut globals, [8, 9, 2025], 250), ([9, 9, 2025], 251));
        assert_eq!(step(&mut globals, [30, 9, 2025], 272), ([1, 10, 2025], 273));
        assert_eq!(step(&mut globals, [28, 2, 2026], 58), ([1, 3, 2026], 59));
        assert_eq!(step(&mut globals, [28, 2, 2028], 58), ([29, 2, 2028], 59));
        assert_eq!(step(&mut globals, [29, 2, 2028], 59), ([1, 3, 2028], 60));
        assert_eq!(step(&mut globals, [31, 12, 2025], 364), ([1, 1, 2026], 0));
        assert_eq!(globals.year_days(), 365);
        step(&mut globals, [31, 12, 2027], 364);
        assert_eq!(globals.year_days(), 366);
    }

    /// The year's end asks for the day table; an ordinary day and a
    /// month's end do not.
    #[test]
    fn only_the_year_end_rolls_the_year() {
        let zeros = vec![0; crate::records::Globals::SIZE];
        let mut globals = crate::records::Globals::new(&zeros).unwrap();
        globals.set_date([15, 5, 2026]);
        globals.set_day_of_year(134);
        assert!(!step_date(&mut globals));
        globals.set_date([30, 9, 2025]);
        globals.set_day_of_year(272);
        assert!(!step_date(&mut globals));
        globals.set_date([31, 12, 2025]);
        globals.set_day_of_year(364);
        assert!(step_date(&mut globals), "into January the table goes down anew");
    }

    /// The day table: from the day of the year on, every of the 2000
    /// entries names its year and its day within that year — 2026 and
    /// 2027 with 365 days, 2028 with 366 —, the entries before the
    /// starting day left standing; the ten u16s at the head count up the
    /// days of the ten years after the date's year.
    #[test]
    fn the_day_table_names_the_coming_days() {
        let mut block = vec![0xff; DAY_PAIRS + 2000 * 4];
        fill_day_pairs(&mut block, 2026, 186);
        let entry = |k: usize| {
            (
                u16::from_le_bytes(block[DAY_PAIRS + k * 4..][..2].try_into().unwrap()),
                u16::from_le_bytes(block[DAY_PAIRS + k * 4 + 2..][..2].try_into().unwrap()),
            )
        };
        assert_eq!(
            (entry(0), entry(185)),
            ((0xffff, 0xffff), (0xffff, 0xffff)),
            "the days before the starting day stand"
        );
        assert_eq!(entry(186), (2026, 186), "today's entry");
        assert_eq!(entry(364), (2026, 364));
        assert_eq!(entry(365), (2027, 0), "2026 has 365 days");
        assert_eq!(entry(729), (2027, 364));
        assert_eq!(entry(730), (2028, 0));
        assert_eq!(entry(1095), (2028, 365), "2028 has 366");
        assert_eq!(entry(1096), (2029, 0));
        let offset = |j: usize| u16::from_le_bytes(block[j * 2..][..2].try_into().unwrap());
        assert_eq!(
            (offset(0), offset(1), offset(2), offset(3), offset(4)),
            (0, 365, 731, 1096, 1461),
            "the ten coming years' day offsets"
        );
    }

    /// Save A from the installation, when there is one.
    fn save_a() -> Option<Records> {
        let dir = std::env::var("ANSTOSS3_DIR").unwrap_or_default();
        let path = std::path::Path::new(&dir).join("saves/A.an3");
        if dir.is_empty() || !path.exists() {
            eprintln!("no A among the saves; the club's day goes unchecked");
            return None;
        }
        Some(Records::from_save(a3_files::an3::Save::read(&path).unwrap()).unwrap())
    }

    /// A computer club's day: every squad player gets the roll of a
    /// hundred and the roll of ten thousand, and only the ten thousand
    /// out of 0 or 1 the roll of a seven; between, his day off takes
    /// three points of condition and brings five of freshness.
    #[test]
    fn a_computer_club_rolls_over_its_squad_and_rests() {
        let Some(mut records) = save_a() else { return };
        let (country, number) = (1..0x75u16)
            .flat_map(|c| (0..records.countries.team_count(usize::from(c)).max(0) as u16).map(move |n| (c, n)))
            .find(|&(c, n)| records.team(c, n).is_some_and(|t| t.humans() == [0, 0] && t.players().len() > 10))
            .unwrap();
        let squad = records.team(country, number).unwrap().players();
        let before: Vec<Player> = squad.iter().map(|&id| records.player(id).unwrap().clone()).collect();
        let mut random = Random::from_state(20261101, 4);
        let mut copy = random.clone();
        club(&mut records, (country, number), &mut random);
        for _ in &squad {
            copy.below(100);
            if copy.below(10_000) < 2 {
                copy.below(7);
            }
        }
        assert!(random == copy, "the club's day, draw for draw");
        for (id, was) in squad.iter().zip(&before) {
            let now = records.player(*id).unwrap();
            if was.doping() & DOSE != 0 {
                continue;
            }
            assert_eq!(now.condition(), (was.condition() - 300).clamp(0, 10_000), "{id}");
            assert_eq!(now.freshness()[0], (was.freshness()[0] + 500).clamp(0, 10_000), "{id}");
            assert_eq!(now.freshness()[1..], was.freshness()[1..], "{id}");
        }
    }

    /// The human club — a trainer or a manager at its record's +0xa8 —
    /// rolls the ten thousand over no player and a three once at the
    /// end; its men have no day off.
    #[test]
    fn a_human_club_rolls_the_three_after_its_squad() {
        let Some(mut records) = save_a() else { return };
        let (country, number) = (1..0x75u16)
            .flat_map(|c| (0..records.countries.team_count(usize::from(c)).max(0) as u16).map(move |n| (c, n)))
            .find(|&(c, n)| records.team(c, n).is_some_and(|t| t.humans() != [0, 0]))
            .unwrap();
        let squad = records.team(country, number).unwrap().players();
        let fitness = |records: &Records| -> Vec<_> {
            squad.iter().map(|&id| records.player(id).map(|p| (p.condition(), p.freshness()))).collect()
        };
        let before = fitness(&records);
        let mut random = Random::from_state(20261101, 5);
        let mut copy = random.clone();
        club(&mut records, (country, number), &mut random);
        for _ in &squad {
            copy.below(100);
        }
        copy.below(3);
        assert!(random == copy, "no ten thousand, one three");
        assert_eq!(fitness(&records), before);
    }

    /// A dose counts down at the club's day, and the condition kind
    /// takes five points while it holds.
    #[test]
    fn a_dose_wears_off() {
        let mut player = Player::new(&vec![0u8; Player::SIZE]).unwrap();
        player.set_condition(6000);
        player.set_doping(0x40 | crate::doping::CONDITION << 2 | 3);
        wear_off(&mut player);
        assert_eq!((player.doping(), player.condition()), (0x40 | crate::doping::CONDITION << 2 | 1, 5500));
        wear_off(&mut player);
        assert_eq!((player.doping(), player.condition()), (0x40 | crate::doping::CONDITION << 2, 5500));
    }

    /// One player's turn: his freshness shifts, fifty of condition
    /// goes, and he rolls seven of the tens — no more, whatever the
    /// rolls answer to.
    #[test]
    fn a_players_turn_shifts_his_freshness_and_rolls() {
        let mut player = Player::new(&vec![0u8; Player::SIZE]).unwrap();
        player.set_freshness([500, 300, 700]);
        player.set_condition(10_000);
        let mut random = Random::from_state(20261101, 6);
        let mut copy = random.clone();
        luck_player(&mut player, 70, &mut random);
        for _ in 0..7 {
            copy.below(10);
        }
        assert!(random == copy, "seven tens, no more");
        assert_eq!(player.freshness(), [570, 700, 0]);
        assert_eq!(player.condition(), 9_950);
    }

    /// A 0-roll's nudge: a part under a hundred climbs a step, one
    /// above it sinks one — to the setter's ceiling at a hundred and
    /// twenty — and a hundred stays; his morale follows the sum.
    #[test]
    fn a_nudge_walks_one_step_to_a_hundred() {
        let mut player = Player::new(&vec![0u8; Player::SIZE]).unwrap();
        player.set_contentment([50, 150, 100, 100, 100, 100, 100]);
        nudge(&mut player, 0);
        nudge(&mut player, 1);
        nudge(&mut player, 2);
        assert_eq!(player.contentment(), [51, 120, 100, 100, 100, 100, 100]);
        assert_eq!(player.morale(), 71, "six hundred seventy one less six hundred");
    }

    /// A hundred and twenty of a part stays its ceiling, and nought
    /// its floor, as the setter clamps both.
    #[test]
    fn a_nudge_never_leaves_nought_or_a_hundred_twenty() {
        let mut player = Player::new(&vec![0u8; Player::SIZE]).unwrap();
        player.set_contentment([0, 120, 101, 99, 100, 100, 100]);
        nudge(&mut player, 0);
        nudge(&mut player, 1);
        nudge(&mut player, 2);
        nudge(&mut player, 3);
        assert_eq!(player.contentment(), [1, 119, 100, 100, 100, 100, 100]);
    }

    /// A day's nudge of the walk index: it climbs and sinks towards
    /// the target below eighty, climbs out of the two lowest rows
    /// between eighty and ninety, and sinks from ninety.
    #[test]
    fn the_walk_steps_one_per_roll() {
        assert_eq!(walk_next(0, 1, 50), 1, "climb");
        assert_eq!(walk_next(4, 1, 50), 3, "sink");
        assert_eq!(walk_next(1, 1, 50), 1, "at the target");
        assert_eq!(walk_next(1, 1, 84), 2, "a middling roll leaves the target");
        assert_eq!(walk_next(0, 1, 84), 1, "out of the bottom");
        assert_eq!(walk_next(3, 1, 84), 3, "only the bottom climbs");
        assert_eq!(walk_next(3, 1, 95), 2, "from ninety it sinks");
        assert_eq!(walk_next(0, 9, 95), 0, "and never below zero");
    }

    /// The walk index's day over the schedule table: which rows get
    /// new numbers, and how many rolls that is.
    #[test]
    fn the_schedule_day_rolls_its_rows() {
        let days: [(i16, &[(u32, usize)]); 7] = [
            (0, &[(2, 40)]),
            (1, &[(2, 40), (5, 20), (3, 20)]),
            (2, &[(5, 20), (3, 20)]),
            (3, &[(4, 20), (3, 20), (4, 40)]),
            (4, &[(4, 40)]),
            (5, &[(3, 40), (4, 40)]),
            (6, &[]),
        ];
        for (walk, rows) in days {
            let mut random = Random::from_state(20261101, 8);
            let mut copy = random.clone();
            let mut table = [0u8; TABLE];
            schedule_day(walk, &mut table, &mut random);
            for (range, count) in rows {
                for _ in 0..*count {
                    copy.below(*range);
                }
            }
            assert!(random == copy, "walk {walk}'s rolls");
        }
    }

    /// A shift of the table: its cells step by the pair, and the strip
    /// they shift out of keeps its old numbers.
    #[test]
    fn the_table_shifts_its_cells() {
        let mut table = [0u8; TABLE];
        for x in 0..40 {
            for y in 0..40 {
                table[y + x * 40] = u8::try_from((x * 3 + y) % 20).unwrap();
            }
        }
        let before = table;
        rotate(&mut table, 1, -1);
        for x in 1..40 {
            for y in 0..39 {
                assert_eq!(table[y + x * 40], before[y + 1 + (x - 1) * 40], "shifted cell");
            }
        }
        for y in 0..40 {
            assert_eq!(table[y], before[y], "the emptied column stays");
        }
        for x in 0..40 {
            assert_eq!(table[39 + x * 40], before[39 + x * 40], "the emptied row stays");
        }
    }

    /// The day's re-roll: every five of the table rolls a two, and no
    /// other cell rolls at all.
    #[test]
    fn the_fives_reroll_and_nothing_else() {
        let mut table = [0u8; TABLE];
        table[7] = 5;
        table[8] = 4;
        table[500] = 3;
        table[1599] = 5;
        let mut random = Random::from_state(20261101, 9);
        let mut copy = random.clone();
        reroll_fives(&mut table, &mut random);
        let first = copy.below(2);
        let second = copy.below(2);
        assert!(random == copy, "one two per five");
        assert!(table[7] == 2 + u8::try_from(first).unwrap(), "a re-rolled five");
        assert!(table[1599] == 2 + u8::try_from(second).unwrap(), "the last one too");
        assert_eq!(table[8], 4, "a four sleeps");
        assert_eq!(table[500], 3, "a three sleeps");
    }
}
