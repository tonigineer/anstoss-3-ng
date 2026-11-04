//! The league matchdays of a running season: every game the league blocks
//! still hold open on a calendar day is scored here and written back where
//! the original's played matches put them. The 3D match of the human's club
//! is out of the replica's scope, so its games are scored like the others.

use crate::calendar::Calendar;
use crate::cards;
use crate::finances::CLUB_TO_TEAM;
use crate::opponent::{CUPS_AT, League, PAIRING, PAIRINGS, cup_pairings};
use crate::random::Random;
use crate::records::{PLANNER_DAYS, PLANNER_ENTRY, Records, Team};
use crate::season::Season;
use crate::settlement;
use crate::statistics::{self, ALL};
use crate::tickets;
use crate::world::{self, World};

/// The stadiums (`0x1a48138[index]`), and the capacity within.
const STADIUM_AT: u32 = 0x1a48138;
const CAPACITY: usize = 0x3c;
/// The plan's attendance and its season-ticket holders (club +0x120,
/// +0x124), and the gate money booked match by match — the league's in
/// club +0x230, the cup's in +0x234, against the cup's planned gate at
/// +0x12c, a money and not a crowd.
const PLAN_ATTENDANCE: usize = 0x120;
const PLAN_SEASON_TICKETS: usize = 0x124;
const BOOKED_LEAGUE: usize = 0x230;
const BOOKED_CUP: usize = 0x234;

/// Strengths to reckon by when a team's record (G+0xb8) carries none: the
/// goalkeeper, defenders, midfield and attack in tenths, attack and hold
/// even so the luck alone makes the goals.
const EVEN: [i16; 4] = [340; 4];
/// The home side's edge, in the strengths' tenths.
const HOME_EDGE: i32 = 50;
/// How much the difference of the strengths is worth a goal.
const GOAL_UNITS: i32 = 200;

/// Play the games of the season's day: for every country the game follows,
/// every league whose calendar row holds this day as one of its matchdays
/// and has not played that matchday yet gets its pairs scored, and so does
/// the country's cup when the day is one of its rounds.
pub fn play_day(records: &mut Records, world: &World, calendar: &Calendar, season: &mut Season) {
    let day = season.day;
    for slot in 0..world::SLOTS {
        let Some(nation) = world.country(slot).map(|country| country.nation) else {
            continue;
        };
        if !records.globals.played(i16::try_from(nation).unwrap_or(0)) {
            continue;
        }
        cup_day(records, nation, calendar, day, &mut season.random);
        supercup_day(records, nation, calendar, day, &mut season.random);
        league_cup_day(records, nation, calendar, day, &mut season.random);
        for level in 0..records.countries.leagues(usize::from(nation)) {
            let level = level as u8;
            let Some(row) = calendar.row(nation, usize::from(level)) else {
                continue;
            };
            let Some(matchday) = calendar.matchday(row, day) else {
                continue;
            };
            let due = League::of(records, nation, level).is_some_and(|league| {
                league.played() + 1 == matchday && !league.open_games().is_empty()
            });
            if due {
                settle(records, nation, level, day, &mut season.random);
            }
        }
    }
}

/// The cup's days sit on the country's first league row, coded 0x40 in the
/// top bits with the round in the low ones (`FUN_004cea5a`); Germany's row
/// names its six rounds at days 59, 115, 157, 234, 262 and 328. The round's
/// pairings stand where the mode's row of the table at 0x8a5e10 says, the
/// final one pairing behind the last of them.
const CUP_CODE: u32 = 0x4000_0000;
/// A pairing's flags: the result stands, the tie went to extra time (text
/// 0x761) or to penalties (text 0x762), whose score stays as it was.
const CUP_PLAYED: u8 = 0x08;
const CUP_RESULT: u8 = 0x20;
const CUP_EXTRA: u8 = 0x80;
const CUP_SHOOT: u8 = 0x40;
/// The cup's last round, its final: the sixth, whatever the mode, one
/// pairing behind the mode's last row. The original tests the same with
/// `FUN_006d12a8`, which asks the round against the rounds table's sixth.
const FINAL_ROUND: usize = 6;

/// The year calendar's entry of a fixture: kind 6, its code at +4, the
/// clubs at +8 and +0xc, the score at +0x10, the flags at +0x14 — 0x20
/// for a result that stands, 0x08 on a cup tie — and the hosting club
/// again at +0x16 (`FUN_0062a0ad`). The season's booking keeps every
/// cup day from the start, a not-yet-drawn round naming no club; when
/// a club leaves the cup its day's entry falls away (`FUN_0072ded2`).
const YEAR_MATCH: u8 = 6;
const YEAR_HOME: usize = 8;
const YEAR_AWAY: usize = 0xc;
const YEAR_GOALS: usize = 0x10;
const YEAR_FLAGS: usize = 0x14;
const YEAR_VENUE: usize = 0x16;
const YEAR_RESULT: u8 = 0x20;
const YEAR_CUP: u8 = 0x08;

/// Play the country's cup round when the day is one: every drawn and still
/// open pairing is scored single-leg, the home side with its edge; a tie
/// runs to extra time, whose goals join the score, and from there to a
/// shootout, which only names the winner in the byte at +0xd, 0 the home
/// side. Once the round's last game is played its winners are shuffled
/// and paired anew — the home side drawn too — where the next round's row
/// says, and the block's last drawn round (the byte at +0x28) follows.
/// Each home club books its gate: a crowd as in the league, the money on
/// its season's cup takings (club +0x234). The human's year calendar keeps
/// its entries: the played round fills its score into the day's entry, the
/// new drawing names his next tie there — and drops it once he is out.
fn cup_day(records: &mut Records, nation: u16, calendar: &Calendar, day: usize, random: &mut Random) {
    let Some(row) = calendar.row(nation, 0) else {
        return;
    };
    let entry = calendar.entry(row, day);
    if entry & 0x7f00_0000 != CUP_CODE {
        return;
    }
    let round = usize::try_from(entry & 0xffff).unwrap_or(0);
    let slot = records.countries.slot(usize::from(nation));
    let name = format!("{CUPS_AT:#x}[{slot}]");
    let Some(mode) = records.block(&name).and_then(|cup| cup.get(0x24 - 0x20)).copied() else {
        return;
    };
    let mode = usize::from(mode).min(5);
    let Some((start, count)) = cup_pairings(mode, round) else {
        return;
    };
    let u16_at = |p: &[u8], at: usize| u16::from_le_bytes([p[at], p[at + 1]]);
    // The scores drawn before the block is written, as in the leagues, the
    // sides they mark, and the gate money the home clubs book with them.
    let mut scored: Vec<(usize, [u8; 4])> = Vec::new();
    let mut marks: Vec<(u8, u16, u8, u16, u8, u8)> = Vec::new();
    let mut gates: Vec<(u8, u16, u8, u16, i32)> = Vec::new();
    let mut through: Vec<(u8, u16)> = Vec::new();
    for q in start..start + count {
        let Some(p) = records
            .block(&name)
            .and_then(|cup| cup.get(PAIRINGS + q * PAIRING..PAIRINGS + (q + 1) * PAIRING))
            .filter(|p| p[0] != 0 && p[0xc] & CUP_RESULT == 0)
        else {
            continue;
        };
        let held = |country: u8, at: usize| {
            records
                .team(u16::from(country), u16_at(p, at))
                .map(|team| team.strengths())
                .unwrap_or(EVEN)
        };
        let home = held(p[0], 2);
        let home = if home == [0; 4] { EVEN } else { home };
        let away = held(p[4], 6);
        let away = if away == [0; 4] { EVEN } else { away };
        let (mut hg, _) = goals(home, away, HOME_EDGE, random);
        let (mut ag, _) = goals(away, home, -HOME_EDGE, random);
        let mut flags = CUP_PLAYED | CUP_RESULT;
        let mut won = hg.cmp(&ag);
        if hg == ag {
            flags |= CUP_EXTRA;
            let (et, _) = goals(home, away, 0, random);
            let (at, _) = goals(away, home, 0, random);
            hg = hg.saturating_add(et / 2 + u8::from(et % 2 != 0));
            ag = ag.saturating_add(at / 2 + u8::from(at % 2 != 0));
            won = hg.cmp(&ag);
            if hg == ag {
                flags |= CUP_SHOOT;
                let sum = |s: &[i16; 4]| s.iter().map(|&v| i32::from(v)).sum::<i32>().max(1);
                let (hs, as_) = (sum(&home), sum(&away));
                won = if i32::try_from(random.below((hs + as_) as u32)).unwrap_or(0) < hs {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Less
                };
            }
        }
        let winner = u8::from(won == std::cmp::Ordering::Less);
        scored.push((q, [hg, ag, flags, winner]));
        marks.push((p[0], u16_at(p, 2), p[4], u16_at(p, 6), hg, ag));
        let at = usize::from(winner != 0);
        through.push((p[at * 4], u16_at(p, at * 4 + 2)));
        // The home club's gate: a crowd as in the league, and the money it
        // brings — the played match's reckoning (FUN_0078b26c) at a mark
        // no cup round holds, which leaves it as it is.
        let (country, number) = (p[0], u16_at(p, 2));
        if let Some(t) = records.team(u16::from(country), number) {
            let cap = capacity(records, u16::from(country), number);
            let att = attendance(t, cap, random);
            let money = tickets::booked_gate(records, t, att, 0);
            if money != 0 {
                gates.push((country, number, p[4], u16_at(p, 6), money));
            }
        }
    }
    if let Some(cup) = records.block_mut(&name) {
        for (q, [hg, ag, flags, winner]) in scored {
            let at = PAIRINGS + q * PAIRING;
            if let Some(p) = cup.get_mut(at + 8..at + 10) {
                p.copy_from_slice(&[hg, ag]);
            }
            if let Some(p) = cup.get_mut(at + 0xc..at + 0xe) {
                p.copy_from_slice(&[flags, winner]);
            }
        }
    }
    // The final decided goes into the country's cup winners and its
    // winner's count of cups ([`crate::honours::cup_final`]).
    if round >= FINAL_ROUND {
        let tie = records
            .block(&name)
            .and_then(|b| b.get(PAIRINGS + start * PAIRING..PAIRINGS + start * PAIRING + crate::honours::FINAL_PAIRING))
            .map(<[u8]>::to_vec);
        if let Some(tie) = tie.filter(|t| t[0xc] & CUP_PLAYED != 0) {
            crate::honours::cup_final(records, nation, &tie);
        }
    }
    // The cup's gate belongs to both clubs: each books half of the played
    // match's money at +0x234 (`FUN_0078b26c`, the branch whose record kind
    // answers to the cup). Where the home club has a human's manager and the
    // visitor has none, the manager keeps his cup share (the getter
    // `FUN_0069c962` reads the manager's +0x1a4) of the visitor's half back
    // for the black cash box (`FUN_0069c869` adds it), at most all of it and
    // never below nought; the visitor books the rest of the half.
    for (hc, hn, ac, an, money) in gates {
        let half = money / 2;
        let kept = match (human_slot(records, hc, hn), human_slot(records, ac, an)) {
            (Some(i), None) => records.managers.get(i).map_or(0, |m| {
                ((half as f32 as f64 * f64::from(m.cup_share_kept())) as i32).max(0)
            }),
            _ => 0,
        };
        if kept > 0
            && let Some(m) = human_slot(records, hc, hn).and_then(|i| records.managers.get_mut(i))
        {
            m.set_black_cash(m.black_cash().wrapping_add(kept));
        }
        for (country, number, money) in [
            (hc, hn, half),
            (ac, an, half - kept),
        ] {
            if let Some(t) = records.team_mut(u16::from(country), number) {
                let old = t.get::<i32>(BOOKED_CUP + CLUB_TO_TEAM);
                t.put::<i32>(BOOKED_CUP + CLUB_TO_TEAM, old.wrapping_add(money));
            }
        }
    }
    // What the ties leave on the men who played them.
    for &(hc, hn, ac, an, hg, ag) in &marks {
        mark_cup(records, hc, hn, ac, an, hg, ag, random);
    }
    // Every club that came through its tie gets what the round pays: the
    // trainer's premium for reaching the round and the main sponsor's cup
    // extra (`FUN_006cebcd`; its counter of cup rounds `FUN_006cdb29` the
    // replica reads off the block instead). The final won adds the
    // manager's premium for a title and the sponsor's second extra
    // (`FUN_006cec7f`, the round `FUN_006d12a8` names the cup's last).
    for (country, number) in through {
        settlement::cup_premium(records, u16::from(country), number, round >= FINAL_ROUND);
        // The final's winner counts the cup in its trainer's career
        // (`FUN_006b50a4` with kind 4).
        if round >= FINAL_ROUND {
            crate::honours::title(records, u16::from(country), number, crate::honours::CUP);
        }
    }
    // On the human's year calendar the round's day fills its score, the
    // flags saying the result stands (0x20) and the tie a cup one (0x08);
    // the calendar slots its human clubs hold are kept for the drawing.
    let mut slots: Vec<usize> = Vec::new();
    for (hc, hn, ac, an, hg, ag) in &marks {
        for (country, number) in [(*hc, *hn), (*ac, *an)] {
            let Some(slot) = human_slot(records, country, number) else {
                continue;
            };
            slots.push(slot);
            if let Some(cal) = records.calendars.get_mut(slot) {
                let mut e = cal.entry(day);
                if e[0] == YEAR_MATCH
                    && u32::from_le_bytes([e[4], e[5], e[6], e[7]]) & 0x7f00_0000 == CUP_CODE
                {
                    e[YEAR_GOALS] = *hg;
                    e[YEAR_GOALS + 1] = *ag;
                    e[YEAR_FLAGS] |= YEAR_RESULT | YEAR_CUP;
                    cal.set_entry(day, e);
                }
            }
        }
    }
    slots.sort();
    slots.dedup();
    // The round is through when its every drawn pairing stands played; the
    // final's winner needs no drawing on.
    if round >= 6 {
        return;
    }
    let mut winners: Vec<(u8, u16)> = Vec::new();
    let drawn = records.block(&name).is_some_and(|cup| {
        for q in start..start + count {
            let Some(p) = cup.get(PAIRINGS + q * PAIRING..PAIRINGS + (q + 1) * PAIRING) else {
                continue;
            };
            if p[0] == 0 {
                continue;
            }
            if p[0xc] & CUP_RESULT == 0 {
                return false;
            }
            let at = usize::from(p[0xd] != 0);
            winners.push((p[at * 4], u16_at(p, at * 4 + 2)));
        }
        true
    });
    let Some((next, pairs)) = drawn.then(|| cup_pairings(mode, round + 1)).flatten() else {
        return;
    };
    if winners.len() != 2 * pairs {
        return;
    }
    for i in (1..winners.len()).rev() {
        let j = usize::try_from(random.below(i as u32 + 1)).unwrap_or(i);
        winners.swap(i, j);
    }
    if let Some(cup) = records.block_mut(&name) {
        for (j, pair) in winners.chunks_exact(2).enumerate() {
            let (a, b) = if random.below(2) == 1 {
                (pair[1], pair[0])
            } else {
                (pair[0], pair[1])
            };
            let [alo, ahi] = a.1.to_le_bytes();
            let [blo, bhi] = b.1.to_le_bytes();
            let row = [a.0, 0, alo, ahi, b.0, 0, blo, bhi, 0, 0, 0, 0, 0, 0];
            if let Some(p) = cup.get_mut(
                PAIRINGS + (next + j) * PAIRING..PAIRINGS + (next + j + 1) * PAIRING,
            ) {
                p.copy_from_slice(&row);
            }
        }
        if let Some(p) = cup.get_mut(0x28 - 0x20) {
            *p = u8::try_from(round + 1).unwrap_or(u8::MAX);
        }
    }
    // The next round's day on the country's first row carries the human's
    // new tie — both clubs, the home one as venue again — while the club
    // gone from the cup sees its entry fall away to nothing.
    let Some(next_day) = (0..PLANNER_DAYS).find(|&d| {
        let entry = calendar.entry(row, d);
        entry & 0x7f00_0000 == CUP_CODE && usize::try_from(entry & 0xffff).unwrap_or(0) == round + 1
    }) else {
        return;
    };
    let mut ties: Vec<(usize, (u8, u16, u8, u16))> = Vec::new();
    for j in 0..pairs {
        let Some(p) = records.block(&name).and_then(|cup| {
            cup.get(PAIRINGS + (next + j) * PAIRING..PAIRINGS + (next + j + 1) * PAIRING)
        }) else {
            continue;
        };
        if p[0] == 0 {
            continue;
        }
        let tie = (p[0], u16_at(p, 2), p[4], u16_at(p, 6));
        for (country, number) in [(tie.0, tie.1), (tie.2, tie.3)] {
            if let Some(slot) = human_slot(records, country, number) {
                ties.push((slot, tie));
            }
        }
    }
    for slot in slots {
        let Some(cal) = records.calendars.get_mut(slot) else {
            continue;
        };
        let Some((_, tie)) = ties.iter().find(|(s, _)| *s == slot) else {
            cal.set_entry(next_day, [0; PLANNER_ENTRY]);
            continue;
        };
        let (hc, hn, ac, an) = *tie;
        let mut e = cal.entry(next_day);
        if e[0] != YEAR_MATCH {
            e = [0; PLANNER_ENTRY];
            e[0] = YEAR_MATCH;
            e[1] = 1;
            let code = CUP_CODE | u32::try_from(round + 1).unwrap_or(0);
            e[4..8].copy_from_slice(&code.to_le_bytes());
        }
        e[YEAR_HOME] = hc;
        e[YEAR_HOME + 2..YEAR_HOME + 4].copy_from_slice(&hn.to_le_bytes());
        e[YEAR_AWAY] = ac;
        e[YEAR_AWAY + 2..YEAR_AWAY + 4].copy_from_slice(&an.to_le_bytes());
        e[YEAR_VENUE] = hc;
        e[YEAR_VENUE + 2..YEAR_VENUE + 4].copy_from_slice(&hn.to_le_bytes());
        e[YEAR_GOALS] = 0;
        e[YEAR_GOALS + 1] = 0;
        e[YEAR_FLAGS] = 0;
        cal.set_entry(next_day, e);
    }
}

/// The calendar slot of a cup club's human manager, if it has one.
/// A supercup's days sit on the country's first league row, coded 0x30
/// with the round in the low bits: Germany's tournament plays its rounds
/// 0 to 3 and its final (4) on days 33 to 45, the others one final, Italy
/// and Spain over two legs — the second turned (the code's high bit) —,
/// the code's 0x80000 deciding the tie in that game.
const SUPERCUP_CODE: u32 = 0x3000_0000;
const SUPERCUP_DECIDES: u32 = 0x0008_0000;
const SUPERCUP_TURNED: u32 = 0x8000_0000;
const SUPERCUP_FINAL: usize = 4;
/// The countries' lists of finals (`DAT_00b19150` in the lists block, by
/// slot): twenty entries of the year and the final tie; the day of the
/// last one (`DAT_00b5df90`).
const FINALS_BLOCK: &str = "0xb0b6b8";
const FINALS_AT: usize = 0xb19150 - 0xb0b6b8;
const FINALS_PER_SLOT: usize = 0x140;
const FINAL_ENTRY: usize = 0x10;
const LAST_FINAL_DAY: usize = 0xb5df90;

/// Play the country's supercup round when the day is one (the round's
/// game scored as a cup tie is, `FUN_006cd4cf`): its tie's result stored
/// as `FUN_006cc255` stores it — the first leg's score at +8, the second's
/// at +0xa, the flags at +0xc (0x20 played, 0x08 decided, 0x80 extra time,
/// 0x40 penalties), the winner's side at +0xd, two legs by their sum and
/// the away goals —, and in a tournament the winner moved on: from the
/// first two ties to the third and fourth against the first and second
/// of the six, from those to the final's home and away side. The human's
/// year calendar takes the score and his next tie, and the final decided
/// goes into the country's list of finals. (The gates and the players'
/// marks are not built.)
fn supercup_day(records: &mut Records, nation: u16, calendar: &Calendar, day: usize, random: &mut Random) {
    use crate::supercup::{FINAL_TIE, SIX, TIE, TIES};
    let Some(row) = calendar.row(nation, 0) else { return };
    let code = calendar.entry(row, day);
    if code & 0x7f00_0000 != SUPERCUP_CODE {
        return;
    }
    let round = (code & 0x3f) as usize;
    let tournament = crate::supercup::is_tournament(records, nation);
    let tie_at = if tournament && round < SUPERCUP_FINAL { TIES + round * TIE } else { FINAL_TIE };
    if !tournament && round != SUPERCUP_FINAL {
        return;
    }
    let slot = records.countries.slot(usize::from(nation));
    let name = format!("{:#x}[{slot}]", crate::supercup::OBJECT);
    let Some(tie) = records.block(&name).and_then(|b| b.get(tie_at..tie_at + TIE)).map(<[u8]>::to_vec) else { return };
    let u16_at = |p: &[u8], at: usize| u16::from_le_bytes([p[at], p[at + 1]]);
    if tie[0] == 0 || tie[4] == 0 {
        return;
    }
    let turned = code & SUPERCUP_TURNED != 0;
    if turned && tie[0xc] & CUP_RESULT == 0 || !turned && tie[0xc] & CUP_RESULT != 0 {
        return;
    }
    let held = |country: u8, number: u16| {
        records.team(u16::from(country), number).map(|team| team.strengths()).filter(|s| *s != [0; 4]).unwrap_or(EVEN)
    };
    let (sides, home_first) = if turned { ([(tie[4], u16_at(&tie, 6)), (tie[0], u16_at(&tie, 2))], false) } else { ([(tie[0], u16_at(&tie, 2)), (tie[4], u16_at(&tie, 6))], true) };
    let (home, away) = (held(sides[0].0, sides[0].1), held(sides[1].0, sides[1].1));
    let (mut hg, _) = goals(home, away, HOME_EDGE, random);
    let (mut ag, _) = goals(away, home, -HOME_EDGE, random);
    // The tie's own sides' goals: the first leg's as played, the
    // second's turned back.
    let (own_first, own_second) = if home_first { (hg, ag) } else { (ag, hg) };
    let (total_home, total_away) = if turned {
        (u16::from(tie[8]) + u16::from(own_first), u16::from(tie[9]) + u16::from(own_second))
    } else {
        (u16::from(own_first), u16::from(own_second))
    };
    let mut flags = CUP_RESULT;
    let mut winner = None;
    if code & SUPERCUP_DECIDES != 0 || turned {
        flags |= CUP_PLAYED;
        let mut won = total_home.cmp(&total_away);
        if won == std::cmp::Ordering::Equal && turned {
            // Away goals (`FUN_00677ed4`): the tie's home side's in the
            // second leg against its away side's in the first.
            won = u16::from(own_first).cmp(&u16::from(tie[9]));
        }
        if won == std::cmp::Ordering::Equal {
            flags |= CUP_EXTRA;
            let (et, _) = goals(home, away, 0, random);
            let (at, _) = goals(away, home, 0, random);
            hg = hg.saturating_add(et / 2 + u8::from(et % 2 != 0));
            ag = ag.saturating_add(at / 2 + u8::from(at % 2 != 0));
            let (f, s2) = if home_first { (hg, ag) } else { (ag, hg) };
            won = (u16::from(f) + if turned { u16::from(tie[8]) } else { 0 }).cmp(&(u16::from(s2) + if turned { u16::from(tie[9]) } else { 0 }));
            if won == std::cmp::Ordering::Equal {
                flags |= CUP_SHOOT;
                let sum = |s: &[i16; 4]| s.iter().map(|&v| i32::from(v)).sum::<i32>().max(1);
                let (hs, as_) = (sum(&home), sum(&away));
                let home_wins = i32::try_from(random.below((hs + as_) as u32)).unwrap_or(0) < hs;
                won = if home_wins == home_first { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Less };
            }
        }
        winner = Some(u8::from(won == std::cmp::Ordering::Less));
    }
    let (own_first, own_second) = if home_first { (hg, ag) } else { (ag, hg) };
    if let Some(b) = records.block_mut(&name) {
        if let Some(t) = b.get_mut(tie_at..tie_at + TIE) {
            if turned {
                t[0xa] = own_first;
                t[0xb] = own_second;
                t[0xc] |= flags | 0x10;
            } else {
                t[8] = own_first;
                t[9] = own_second;
                t[0xc] = flags;
            }
            t[0xd] = winner.unwrap_or(0);
        }
        // The winner moves on in a tournament.
        if let (true, Some(w)) = (tournament, winner) {
            let won_side = [tie[w as usize * 4], tie[w as usize * 4 + 1], tie[w as usize * 4 + 2], tie[w as usize * 4 + 3]];
            let country = b[0];
            match round {
                0 | 1 => {
                    let next = TIES + (round + 2) * TIE;
                    let seed = [b[SIX + 2 * round], b[SIX + 2 * round + 1]];
                    b[next..next + 4].copy_from_slice(&won_side);
                    b[next + 4] = country;
                    b[next + 6..next + 8].copy_from_slice(&seed);
                }
                2 => b[FINAL_TIE..FINAL_TIE + 4].copy_from_slice(&won_side),
                3 => b[FINAL_TIE + 4..FINAL_TIE + 8].copy_from_slice(&won_side),
                _ => {}
            }
        }
    }
    // The final decided goes into the country's list of finals, the
    // oldest of twenty dropped (`FUN_006ccc2a`, `FUN_006cd999`), and the
    // day is noted.
    if round == SUPERCUP_FINAL && winner.is_some() {
        // Its winner counts the supercup in its trainer's career
        // (`FUN_006cc61b`: `FUN_006b50a4` with kind 5).
        let w = usize::from(winner.unwrap_or(0) != 0) * 4;
        crate::honours::title(records, u16::from(tie[w]), u16_at(&tie, w + 2), crate::honours::SUPERCUP);
        let final_tie = records.block(&name).and_then(|b| b.get(FINAL_TIE..FINAL_TIE + FINAL_ENTRY - 2)).map(<[u8]>::to_vec);
        let year = records.globals.start_year();
        if let (Some(t), Some(b)) = (final_tie, records.block_mut(FINALS_BLOCK)) {
            let at = FINALS_AT + usize::from(slot) * FINALS_PER_SLOT;
            if let Some(list) = b.get_mut(at..at + FINALS_PER_SLOT) {
                list.copy_within(FINAL_ENTRY.., 0);
                let last = FINALS_PER_SLOT - FINAL_ENTRY;
                list[last..last + 2].copy_from_slice(&year.to_le_bytes());
                list[last + 2..].copy_from_slice(&t);
            }
        }
        records.globals.put::<u16>(LAST_FINAL_DAY, day as u16);
    }
    // The human's calendar: the day's score, and his supercup days anew.
    for (country, number) in [(tie[0], u16_at(&tie, 2)), (tie[4], u16_at(&tie, 6))] {
        let Some(slot) = human_slot(records, country, number) else { continue };
        if let Some(cal) = records.calendars.get_mut(slot) {
            let mut e = cal.entry(day);
            if e[0] == YEAR_MATCH && u32::from_le_bytes([e[4], e[5], e[6], e[7]]) & 0x7f00_0000 == SUPERCUP_CODE {
                e[YEAR_GOALS] = hg;
                e[YEAR_GOALS + 1] = ag;
                e[YEAR_FLAGS] |= YEAR_RESULT | YEAR_CUP;
                cal.set_entry(day, e);
            }
        }
        crate::year::rebook_supercup(records, calendar, slot, (u16::from(country), number), day + 1);
    }
}

/// England's league cup days on its first league row, coded 0x01 with the
/// round low: rounds over two legs (the second turned, the code's high
/// bit) and single games that decide (0x80000).
const LEAGUE_CUP_CODE: u32 = 0x0100_0000;
const ENGLAND: u16 = 0x10;

/// Play England's league cup round when the day is one: every tie of the
/// round scored as a cup tie is and stored as the supercup's are (the
/// first leg at +8, the second at +0xa, the flags, the winner, two legs
/// by their sum and the away goals); once the round stands decided its
/// winners and the entrants the next round takes in beside them are
/// shuffled and paired anew there, the home side drawn too, and the
/// block's last drawn round follows. (The gates, the players' marks and
/// the premiums are not built.)
fn league_cup_day(records: &mut Records, nation: u16, calendar: &Calendar, day: usize, random: &mut Random) {
    use crate::cup_draw::{ENTRANTS_AT, LEAGUE_CUP, joining};
    if nation != ENGLAND {
        return;
    }
    let Some(row) = calendar.row(nation, 0) else { return };
    let code = calendar.entry(row, day);
    if code & 0x7f00_0000 != LEAGUE_CUP_CODE {
        return;
    }
    let round = (code & 0x3f) as usize;
    let turned = code & SUPERCUP_TURNED != 0;
    let decides = code & SUPERCUP_DECIDES != 0 || turned;
    let Some(block) = records.block(LEAGUE_CUP).map(<[u8]>::to_vec) else { return };
    let mode = usize::from(u16::from_le_bytes([block[4], block[5]]));
    let Some((start, count)) = cup_pairings(mode, round) else { return };
    let u16_at = |p: &[u8], at: usize| u16::from_le_bytes([p[at], p[at + 1]]);
    let held = |country: u8, number: u16| {
        records.team(u16::from(country), number).map(|team| team.strengths()).filter(|s| *s != [0; 4]).unwrap_or(EVEN)
    };
    let mut results: Vec<(usize, [u8; PAIRING])> = Vec::new();
    for q in start..start + count {
        let at = PAIRINGS + q * PAIRING;
        let Some(tie) = block.get(at..at + PAIRING) else { continue };
        let mut tie: [u8; PAIRING] = tie.try_into().unwrap_or([0; PAIRING]);
        if tie[0] == 0 || tie[4] == 0 || turned && tie[0xc] & CUP_RESULT == 0 || !turned && tie[0xc] & CUP_RESULT != 0 {
            continue;
        }
        let (a, b) = ((tie[0], u16_at(&tie, 2)), (tie[4], u16_at(&tie, 6)));
        let (home, away) = if turned { (held(b.0, b.1), held(a.0, a.1)) } else { (held(a.0, a.1), held(b.0, b.1)) };
        let (hg, _) = goals(home, away, HOME_EDGE, random);
        let (ag, _) = goals(away, home, -HOME_EDGE, random);
        // The tie's own sides' goals.
        let (mut first, mut second) = if turned { (ag, hg) } else { (hg, ag) };
        let mut flags = CUP_RESULT;
        let mut winner = 0;
        if decides {
            flags |= CUP_PLAYED;
            let sum = |f: u8, s2: u8| if turned { (u16::from(tie[8]) + u16::from(f), u16::from(tie[9]) + u16::from(s2)) } else { (u16::from(f), u16::from(s2)) };
            let (ta, tb) = sum(first, second);
            let mut won = ta.cmp(&tb);
            if won == std::cmp::Ordering::Equal && turned {
                won = u16::from(first).cmp(&u16::from(tie[9]));
            }
            if won == std::cmp::Ordering::Equal {
                flags |= CUP_EXTRA;
                let (et, _) = goals(home, away, 0, random);
                let (at2, _) = goals(away, home, 0, random);
                let (eh, ea) = (et / 2 + u8::from(et % 2 != 0), at2 / 2 + u8::from(at2 % 2 != 0));
                let (ef, es) = if turned { (ea, eh) } else { (eh, ea) };
                first = first.saturating_add(ef);
                second = second.saturating_add(es);
                let (ta, tb) = sum(first, second);
                won = ta.cmp(&tb);
                if won == std::cmp::Ordering::Equal {
                    flags |= CUP_SHOOT;
                    let total = |s: &[i16; 4]| s.iter().map(|&v| i32::from(v)).sum::<i32>().max(1);
                    let (hs, as_) = (total(&home), total(&away));
                    let home_wins = i32::try_from(random.below((hs + as_) as u32)).unwrap_or(0) < hs;
                    won = if home_wins != turned { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Less };
                }
            }
            winner = u8::from(won == std::cmp::Ordering::Less);
        }
        if turned {
            tie[0xa] = first;
            tie[0xb] = second;
            tie[0xc] |= flags | 0x10;
        } else {
            tie[8] = first;
            tie[9] = second;
            tie[0xc] = flags;
        }
        tie[0xd] = winner;
        results.push((q, tie));
    }
    if results.is_empty() {
        return;
    }
    let Some(b) = records.block_mut(LEAGUE_CUP) else { return };
    for (q, tie) in &results {
        let at = PAIRINGS + q * PAIRING;
        b[at..at + PAIRING].copy_from_slice(tie);
    }
    // The round decided: its winners and the next round's entrants on.
    let decided: Option<Vec<[u8; 4]>> = (start..start + count)
        .map(|q| {
            let at = PAIRINGS + q * PAIRING;
            let p = &b[at..at + PAIRING];
            if p[0] == 0 {
                Some(None)
            } else if p[0xc] & CUP_PLAYED == 0 {
                None
            } else {
                let w = usize::from(p[0xd] != 0) * 4;
                Some(Some([p[w], p[w + 1], p[w + 2], p[w + 3]]))
            }
        })
        .collect::<Option<Vec<_>>>()
        .map(|v| v.into_iter().flatten().collect());
    let Some(mut sides) = decided else { return };
    if round >= 6 {
        // The final's winner counts the league cup in its trainer's career
        // (`FUN_0067e911`: `FUN_006b50a4` with kind 0xb or 0xc).
        for side in sides {
            crate::honours::title(records, u16::from(side[0]), u16::from_le_bytes([side[2], side[3]]), crate::honours::LEAGUE_CUP);
        }
        return;
    }
    let Some((next, pairs)) = cup_pairings(mode, round + 1) else { return };
    let (from, joined) = joining(mode, round + 1);
    for k in from..from + joined {
        let at = ENTRANTS_AT + k * 4;
        if let Some(e) = b.get(at..at + 4) {
            sides.push([e[0], e[1], e[2], e[3]]);
        }
    }
    if sides.len() != 2 * pairs {
        return;
    }
    for i in (1..sides.len()).rev() {
        let j = usize::try_from(random.below(i as u32 + 1)).unwrap_or(i);
        sides.swap(i, j);
    }
    for (j, pair) in sides.chunks_exact(2).enumerate() {
        let (x, y) = if random.below(2) == 1 { (pair[1], pair[0]) } else { (pair[0], pair[1]) };
        let at = PAIRINGS + (next + j) * PAIRING;
        if let Some(p) = b.get_mut(at..at + PAIRING) {
            p.fill(0);
            p[0] = x[0];
            p[2..4].copy_from_slice(&x[2..4]);
            p[4] = y[0];
            p[6..8].copy_from_slice(&y[2..4]);
        }
    }
    if let Some(d) = b.get_mut(0x28 - 0x20) {
        *d = u8::try_from(round + 1).unwrap_or(u8::MAX);
    }
}

fn human_slot(records: &Records, country: u8, number: u16) -> Option<usize> {
    records
        .team(u16::from(country), number)
        .filter(|team| team.human() > 0)
        .map(|team| usize::from(team.human()) - 1)
}

/// What a cup tie leaves on its men: the eleven each side puts on the
/// pitch — its own suspended men (+0x234) held off the round — stand for
/// their season's cup appearances in the career's five bytes (+0x25f),
/// and every goal, drawn over the eleven as in the league, stands in the
/// season's cup goals (+0x23d) and the career's (+0x23f). Cards follow
/// the league's odds into the cup's counts (+0x231: yellow, red, yellow-
/// red) and the country's cup rule (cards::cup); a suspending card marks
/// the cup's suspension byte (+0x234: 0x80 red, 0x40 yellow-red, else a
/// yellow accumulation) and holds its man off the next round, where the
/// tie clears it. The league's 46 matchday rows hold its games alone, so
/// a cup tie leaves no row there, and the season's cup grade (+0x233)
/// and the substitutes' shares are the original's match engine, still
/// unportrayed.
fn mark_cup(
    records: &mut Records,
    hc: u8,
    hn: u16,
    ac: u8,
    an: u16,
    full: u8,
    a_full: u8,
    random: &mut Random,
) {
    for (country, team, goals) in [(hc, hn, full), (ac, an, a_full)] {
        let nation = u16::from(country);
        let squad = records
            .team(nation, team)
            .map(|t| t.players())
            .unwrap_or_default();
        let fit: Vec<u16> = squad
            .iter()
            .copied()
            .filter(|&id| {
                records.player(id).is_some_and(|p| available(records, id) && p.cup_ban() == 0)
            })
            .collect();
        let eleven = eleven(&fit, records);
        let weights: Vec<u32> = eleven
            .iter()
            .map(|&id| {
                records.player(id).map_or(0, |p| {
                    u32::from(p.strength()) * KIND_WEIGHTS[usize::from(p.positions()[0]).min(10)]
                })
            })
            .collect();
        let mut scored = vec![0u8; eleven.len()];
        for _ in 0..u32::from(goals) {
            let Some(k) = draw(&weights, random) else {
                break;
            };
            scored[k] = scored[k].saturating_add(1);
        }
        // A fresh card's suspension of this round's marking stands over
        // the serving of the last; the men who wore one into the tie sat
        // it out and stand free of it now.
        let mut carded: Vec<u16> = Vec::new();
        for (&id, &n) in eleven.iter().zip(&scored) {
            let card = if random.below(110) == 0 {
                3
            } else if random.below(340) == 0 {
                2
            } else if random.below(9) == 0 {
                1
            } else {
                0
            };
            let suspends_yellow = card == 1
                && records.player(id).is_some_and(|p| {
                    let mut q = p.clone();
                    let mut counted = q.cup_cards();
                    counted[0] = counted[0].saturating_add(1);
                    q.set_cup_cards(counted);
                    cards::cup(records, nation, &q)
                });
            if let Some(p) = records.player_mut(id) {
                let mut apps = p.career_cup_appearances();
                apps[0] = apps[0].saturating_add(1);
                p.set_career_cup_appearances(apps);
                if n > 0 {
                    let mut goals = p.goals();
                    goals[1] = goals[1].saturating_add(n);
                    p.set_goals(goals);
                    p.set_career_cup_goals(p.career_cup_goals().saturating_add(n));
                }
                if card != 0 {
                    let mut cards = p.cup_cards();
                    match card {
                        1 => cards[0] = cards[0].saturating_add(1),
                        2 => cards[2] = cards[2].saturating_add(1),
                        _ => cards[1] = cards[1].saturating_add(1),
                    }
                    p.set_cup_cards(cards);
                    if card >= 2 || suspends_yellow {
                        p.set_cup_ban(match card {
                            3 => 0x80,
                            2 => 0x40,
                            _ => 1,
                        });
                        carded.push(id);
                    }
                }
            }
        }
        for id in squad {
            if carded.contains(&id) {
                continue;
            }
            if let Some(p) = records.player_mut(id) {
                p.set_cup_ban(0);
            }
        }
    }
}

/// Score a league's next matchday and write it to its block: the four goal
/// bytes of each pair, the count of matchdays played, and the crowd — the
/// matchday's gates together, as the original keeps them by the hundred
/// and fifty thousand — then the places the table shows after the
/// matchday, and each club's season record, goals and table place. Every
/// home club books its gate: the attendance on its played matches, the
/// money on its season's takings, and the league's attendance row
/// follows. Each match leaves its mark on its players: appearances and
/// scorers' goals in their records, and in the league's top-scorers list.
/// Both clubs keep their gate figures: the home club its crowd, income and
/// the visiting fans, the away club the travelling gate of the game. The
/// human's year calendar fills the day's fixture with the new score, its
/// flag saying the result stands.
fn settle(records: &mut Records, nation: u16, level: u8, day: usize, random: &mut Random) {
    let Some(league) = League::of(records, nation, level) else {
        return;
    };
    let Some(name) = League::block_name(records, nation, level) else {
        return;
    };
    let games = league.open_games();
    // The byte where each place writes its place after this matchday, and
    // where the count and the crowd stand — all taken before the count is
    // raised, since they lie on the matchday as it was played.
    let strengths: Vec<[i16; 4]> = (0..league.teams())
        .map(|place| {
            let held = records
                .team(nation, league.team(place) as u16)
                .map(|team| team.strengths())
                .unwrap_or(EVEN);
            if held == [0; 4] { EVEN } else { held }
        })
        .collect();
    let places: Vec<Option<usize>> = (0..league.teams())
        .map(|place| league.place_slot(place))
        .collect();
    let played_at = league.played_slot();
    let crowd_at = league.crowd_slot();
    let played = (league.played() + 1) as i16;
    // The row of this matchday in the players' forty-six.
    let md = usize::from(league.played());
    let matchday = i32::try_from(league.played() + 1).unwrap_or(0);
    let scored: Vec<[u8; 4]> = games
        .iter()
        .map(|&(home, away, _)| {
            let (full, half) = goals(strengths[home], strengths[away], HOME_EDGE, random);
            let (a_full, a_half) = goals(strengths[away], strengths[home], -HOME_EDGE, random);
            [full, a_full, half, a_half]
        })
        .collect();
    // The gate each home club books for its match — the attendance, the
    // money and the stadium it fills — reckoned before the block and the
    // clubs are written.
    let homes: Vec<u16> = games.iter().map(|&(home, _, _)| league.team(home) as u16).collect();
    let aways: Vec<u16> = games.iter().map(|&(_, away, _)| league.team(away) as u16).collect();
    let booked: Vec<(i32, i32, i32)> = homes
        .iter()
        .map(|&team| match records.team(nation, team) {
            Some(t) => {
                let cap = capacity(records, nation, team);
                let att = attendance(t, cap, random);
                (att, tickets::booked_gate(records, t, att, matchday), cap)
            }
            None => (0, 0, 0),
        })
        .collect();
    let crowd: i32 = booked.iter().map(|&(att, _, _)| att).sum();
    let att_slot = statistics::attendance_slot(records, nation, level);
    if let Some(block) = records.block_mut(&name) {
        for ((_, _, at), result) in games.iter().zip(&scored) {
            if let Some(slot) = block.get_mut(*at..*at + 4) {
                slot.copy_from_slice(result);
            }
        }
        if let Some(slot) = block.get_mut(played_at..played_at + 2) {
            slot.copy_from_slice(&played.to_le_bytes());
        }
        if let Some(slot) = block.get_mut(crowd_at..crowd_at + 4) {
            slot.copy_from_slice(&crowd.to_le_bytes());
        }
    }
    for (((home, _, _), (&team, (att, money, cap))), &away) in
        games.iter().zip(homes.iter().zip(booked)).zip(&aways)
    {
        // The visiting fans: a part of the away club's own plan (save A
        // reads 3926 with the away club planning 34000), never more than
        // came to the ground. The original draws them inside the match
        // (the match record's +0x54); the replica draws them over the plan.
        let plan = records
            .team(nation, away)
            .map_or(0, |a| a.get::<i32>(PLAN_ATTENDANCE + CLUB_TO_TEAM));
        let luck = 8 + i32::try_from(random.below(7)).unwrap_or(8);
        let fans = plan.max(0).wrapping_mul(luck) / 100;
        let fans = fans.clamp(0, att.max(0));
        // The home club: its gate held among its gates, the money booked,
        // and — where it has a human's extra block — the visiting fans and
        // the gate's income of this home game, as the statistics pages
        // read them (the match record's +0x54 and +0x3c into extra +0x3f4
        // and +0x450, `FUN_007108d3`).
        // A human's home club against a computer's keeps its manager's
        // share of the gate back for his black cash box, at most all of it
        // and never below nought (`FUN_0078b26c` with `FUN_0069c90c`,
        // `FUN_0069c869`): ftol of the gate as a float times the share.
        let human = |n: u16| records.team(nation, n).map_or(0, |t| t.humans()[0]);
        let kept = match (human(team), human(away)) {
            (h, 0) if h > 0 => records.managers.get(usize::from(h) - 1).map_or(0, |m| {
                ((money as f32 as f64 * f64::from(m.gate_kept())) as i32).max(0)
            }),
            _ => 0,
        };
        if kept > 0
            && let Some(m) = usize::from(human(team)).checked_sub(1).and_then(|i| records.managers.get_mut(i))
        {
            m.set_black_cash(m.black_cash().wrapping_add(kept));
        }
        let mut host = (0usize, 0u8);
        if let Some(t) = records.team_mut(nation, team) {
            let old = t.get::<i32>(BOOKED_LEAGUE + CLUB_TO_TEAM);
            t.put::<i32>(BOOKED_LEAGUE + CLUB_TO_TEAM, old.wrapping_add(money - kept));
            let held = usize::from(t.home_games());
            let mut gates = t.gates();
            if let Some(entry) = gates.get_mut(held) {
                *entry = att;
                t.set_gates(gates);
            }
            t.set_home_games(u8::try_from(held + 1).unwrap_or(u8::MAX));
            host = (held, t.human());
        }
        if host.1 > 0
            && let Some(x) = records.extras.get_mut(usize::from(host.1) - 1)
        {
            let mut fan_gates = x.fan_gates();
            let mut incomes = x.home_incomes();
            if let (Some(f), Some(m)) = (fan_gates.get_mut(host.0), incomes.get_mut(host.0)) {
                *f = fans;
                *m = money;
                x.set_fan_gates(fan_gates);
                x.set_home_incomes(incomes);
            }
        }
        // The away club: its count of away games up, and the gate of the
        // travelling — the whole gate at the opponent's ground, the match
        // record's +0x34 into extra +0x398 (`FUN_0063f09b`).
        let mut guest = (0usize, 0u8);
        if let Some(a) = records.team_mut(nation, away) {
            let held = usize::from(a.away_games());
            a.set_away_games(u8::try_from(held + 1).unwrap_or(u8::MAX));
            guest = (held, a.human());
        }
        if guest.1 > 0
            && let Some(x) = records.extras.get_mut(usize::from(guest.1) - 1)
        {
            let mut away_gates = x.away_gates();
            if let Some(entry) = away_gates.get_mut(guest.0) {
                *entry = att;
                x.set_away_gates(away_gates);
            }
        }
        if let Some(slot) = att_slot {
            statistics::book_attendance(records, slot, *home, att, cap);
        }
    }
    // What the goals leave on the players of both sides.
    for ((&home, &away), result) in homes.iter().zip(&aways).zip(&scored) {
        mark_match(records, nation, level, md, home, away, result[0], result[1], random);
    }
    // The human's year calendar keeps the matchday's result: the season's
    // booking named the fixture on the day's entry — kind 6, the matchday
    // plain as its code — and the played match fills its score in, the
    // flag saying the result stands (0x20; the cup's 0x08 stays off).
    for ((&team, &away), &[hg, ag, _, _]) in homes.iter().zip(&aways).zip(&scored) {
        let mut slots: Vec<usize> = Vec::new();
        for number in [team, away] {
            if let Some(slot) = human_slot(records, u8::try_from(nation).unwrap_or(0), number)
                && !slots.contains(&slot)
            {
                slots.push(slot);
            }
        }
        let code = u32::try_from(matchday).unwrap_or(0);
        for slot in slots {
            let Some(cal) = records.calendars.get_mut(slot) else {
                continue;
            };
            let mut e = cal.entry(day);
            if e[0] != YEAR_MATCH
                || u32::from_le_bytes([e[4], e[5], e[6], e[7]]) != code
                || e[YEAR_HOME + 2..YEAR_HOME + 4] != team.to_le_bytes()
                || e[YEAR_AWAY + 2..YEAR_AWAY + 4] != away.to_le_bytes()
            {
                continue;
            }
            e[YEAR_GOALS] = hg;
            e[YEAR_GOALS + 1] = ag;
            e[YEAR_FLAGS] |= YEAR_RESULT;
            cal.set_entry(day, e);
        }
    }
    // And the day has served: one match off every suspension of the
    // clubs that played. A card drawn this day set its count two matches
    // high — the day itself among them — so that one still stands when
    // the next matchday picks its men.
    for &team in homes.iter().chain(&aways) {
        let squad =
            records.team(nation, team).map(|t| t.players()).unwrap_or_default();
        for id in squad {
            if let Some(p) = records.player_mut(id) {
                let old = p.suspension();
                if old & 0x3f != 0 {
                    p.set_suspension((old & 0xc0) | ((old & 0x3f) - 1));
                }
            }
        }
    }
    // The table as it stands after the matchday, and the places its rows
    // write into the blocks' history.
    let Some(league) = League::of(records, nation, level) else {
        return;
    };
    let rows = statistics::table(records, &league, ALL);
    if let Some(block) = records.block_mut(&name) {
        for (place, rank) in rows.iter().enumerate().map(|(rank, row)| (row.place, rank + 1)) {
            if let Some(at) = places.get(place).and_then(|slot| *slot)
                && let Some(byte) = block.get_mut(at)
            {
                *byte = rank as u8;
            }
        }
    }
    for (rank, row) in rows.iter().enumerate() {
        let record = [
            u8::try_from(row.won).unwrap_or(u8::MAX),
            u8::try_from(row.drawn).unwrap_or(u8::MAX),
            u8::try_from(row.lost).unwrap_or(u8::MAX),
        ];
        let goals = [
            u8::try_from(row.goals_for).unwrap_or(u8::MAX),
            u8::try_from(row.goals_against).unwrap_or(u8::MAX),
        ];
        if let Some(team) = records.team_mut(nation, row.team as u16) {
            team.set_record(record);
            team.set_goals(goals);
            team.set_table_place(rank as u8 + 1);
        }
    }
}

/// Weights a goal is drawn by, over the position: nought for the keeper,
/// little for the defence, most for the attack — times the player's
/// strength. The positions run 1 to 10, keeper to striker; the special
/// kinds from 11 on (the exe's table at 0x89e23c) count among the attack.
const KIND_WEIGHTS: [u32; 11] = [0, 0, 1, 1, 1, 2, 2, 3, 4, 5, 6];

/// What a scored match leaves on its players: the eleven each side puts
/// on the pitch stand for their league match (+0x253) and their run of
/// matches in a row (+0x34e), broken for those listed but not picked;
/// every goal goes to one of the eleven, drawn over their weights — the
/// scorer's weight is his strength times his position's kind — and stands
/// in his season's league goals (+0x23c), his goals in the last match
/// (+0x249), his home goals when his side played at home (+0x252), and in
/// the league's top-scorer list. Each man on the pitch — the eleven and
/// the one to three who came on for the weakest of them — keeps his row of
/// the matchday (+0x319: starter, substitution touched, grade, card) with
/// his goals of it in +0x2eb; one to three substitutions a side, grades,
/// cards and the suspensions they bring are the replica's own, held to
/// what the saves read. The fitness wear follows another step.
fn mark_match(
    records: &mut Records,
    nation: u16,
    level: u8,
    md: usize,
    home: u16,
    away: u16,
    full: u8,
    a_full: u8,
    random: &mut Random,
) {
    for (team, goals, at_home) in [(home, full, true), (away, a_full, false)] {
        let squad = records
            .team(nation, team)
            .map(|t| t.players())
            .unwrap_or_default();
        let eleven = eleven(&squad, records);
        let weights: Vec<u32> = eleven
            .iter()
            .map(|&id| {
                records.player(id).map_or(0, |p| {
                    u32::from(p.strength()) * KIND_WEIGHTS[usize::from(p.positions()[0]).min(10)]
                })
            })
            .collect();
        let mut scored = vec![0u8; eleven.len()];
        for _ in 0..u32::from(goals) {
            let Some(k) = draw(&weights, random) else {
                break;
            };
            scored[k] = scored[k].saturating_add(1);
            let id = eleven[k];
            let names = records.player(id).map(|p| (p.first_name(), p.last_name()));
            if let Some((first, last)) = names {
                statistics::book_goal(
                    records,
                    nation,
                    level,
                    id,
                    first,
                    last,
                    u8::try_from(team).unwrap_or(0),
                );
            }
        }
        // The side's substitutions: one to three of the strongest waiting
        // men replace the weakest field men of the eleven. The original
        // substitutes at its manager's word; the replica takes the
        // strongest who wait.
        let strength = |id: u16| u32::from(records.player(id).map_or(0, |p| p.strength()));
        let mut bench: Vec<u16> = squad
            .iter()
            .copied()
            .filter(|&id| !eleven.contains(&id) && available(records, id))
            .collect();
        bench.sort_by_key(|&id| std::cmp::Reverse(strength(id)));
        let mut field: Vec<u16> = eleven
            .iter()
            .copied()
            .filter(|&id| {
                records.player(id).is_some_and(|p| p.positions()[0] != world::GOALKEEPER)
            })
            .collect();
        field.sort_by_key(|&id| strength(id));
        let subs = (1 + usize::try_from(random.below(3)).unwrap_or(1))
            .min(field.len())
            .min(bench.len());
        let outs: Vec<u16> = field.iter().copied().take(subs).collect();
        let ins: Vec<u16> = bench.iter().copied().take(subs).collect();
        // The matchday row of every man on the pitch (R+0x319): bit 6 he
        // started, bit 7 a substitution touched him — off for a starter,
        // on for one coming in —, bits 2 to 5 his grade in half steps,
        // the low two his card: 1 yellow, 2 yellow-red, 3 red. His goals
        // of the matchday stand in the high nibble of R+0x2eb, his
        // coming-ons counted at +0x254, his grade of the last match at
        // +0x273. The original writes these from its match engine; the
        // grades (four to eleven half steps about the mean, as the saves
        // read), the cards — a yellow to about a ninth of the men, the
        // reds and yellow-reds far rarer — and the substitutions are the
        // replica's own. A red or yellow-red suspends, and so does every
        // rule's worth of yellows; the suspension counts the matchday
        // just played among its matches, which settle serves off at the
        // day's end.
        let appearances: Vec<(u16, u8, bool, bool)> = eleven
            .iter()
            .zip(&scored)
            .map(|(&id, &n)| (id, n, true, outs.contains(&id)))
            .chain(ins.iter().map(|&id| (id, 0, false, false)))
            .collect();
        for (id, n, starter, went_off) in appearances {
            let grade = (5 + u32::from(random.below(5)) + u32::from(n)).min(11) as u8;
            let card = if random.below(110) == 0 {
                3
            } else if random.below(340) == 0 {
                2
            } else if random.below(9) == 0 {
                1
            } else {
                0
            };
            // Whether this yellow is the country's rule's last: drawn over
            // the cards as they would stand with it (`cards::league`).
            let suspends_yellow = card == 1
                && records.player(id).is_some_and(|p| {
                    let mut q = p.clone();
                    let mut counted = q.league_cards();
                    counted[0] = counted[0].saturating_add(1);
                    q.set_league_cards(counted);
                    cards::league(records, nation, level, &q)
                });
            if let Some(p) = records.player_mut(id) {
                p.set_league_matches(p.league_matches().saturating_add(1));
                p.set_matches_in_a_row(p.matches_in_a_row().saturating_add(1));
                p.set_last_goals(n);
                p.set_grade(grade);
                if !starter {
                    let mut less = p.league_less();
                    less[0] = less[0].saturating_add(1);
                    p.set_league_less(less);
                }
                if n > 0 {
                    let mut goals = p.goals();
                    goals[0] = goals[0].saturating_add(n);
                    p.set_goals(goals);
                    if at_home {
                        p.set_home_goals(p.home_goals().saturating_add(n));
                    }
                }
                if md < p.matchdays().len() {
                    let mut bytes = p.matchdays();
                    let mut nibbles = p.matchday_goals();
                    bytes[md] =
                        (if starter { 0x40 } else { 0x80 }) | (grade << 2) | card;
                    if went_off {
                        bytes[md] |= 0x80;
                    }
                    nibbles[md] = n << 4;
                    p.set_matchdays(bytes);
                    p.set_matchday_goals(nibbles);
                }
                if card != 0 {
                    let mut cards = p.league_cards();
                    match card {
                        1 => cards[0] = cards[0].saturating_add(1),
                        2 => cards[2] = cards[2].saturating_add(1),
                        _ => cards[1] = cards[1].saturating_add(1),
                    }
                    p.set_league_cards(cards);
                    if card >= 2 || suspends_yellow {
                        let old = p.suspension();
                        let low = ((u32::from(old & 0x3f) + 2).min(63)) as u8;
                        p.set_suspension((old & 0xc0) | low);
                    }
                }
            }
        }
        for &id in squad
            .iter()
            .filter(|&id| !eleven.contains(&id) && !ins.contains(&id))
        {
            if let Some(p) = records.player_mut(id) {
                p.set_matches_in_a_row(0);
                // The man who sat the matchday out keeps a nought row —
                // the saves read no leftovers of older seasons in it.
                if md < p.matchdays().len() {
                    let mut bytes = p.matchdays();
                    bytes[md] = 0;
                    p.set_matchdays(bytes);
                }
                if md < p.matchday_goals().len() {
                    let mut nibbles = p.matchday_goals();
                    nibbles[md] = 0;
                    p.set_matchday_goals(nibbles);
                }
            }
        }
    }
}

/// Whether the player stands fit and free for his club's match: no
/// injury (+0x228), no match still suspended (+0x229) and no known
/// number thereof (bit 7), no weeks of a ban (+0x22a), and not marked
/// suspended for the match (+0x26b bit 4).
fn available(records: &Records, id: u16) -> bool {
    records.player(id).is_some_and(|p| {
        p.injury_kind() == 0
            && p.suspension() & 0x3f == 0
            && p.banned_weeks() == 0
            && p.away_kind() & 0x10 == 0
    })
}

/// The eleven a side puts on the pitch when no formation stands: of the
/// players it lists, those fit and free — no injury (+0x228), no match
/// suspension (+0x229) or weeks of a ban (+0x22a), and not marked
/// suspended for the match (+0x26b bit 4) — the keeper first, the
/// strongest who names him one, and the ten strongest field players
/// beside him. The engine walk reads the manager's saved eleven instead
/// (`sim::saved_eleven`); this stands for the sides whose record lists
/// none, and for the fixture paths (`mark_match`, the cup ties), where
/// the original still picks from its own squads.
pub(crate) fn eleven(squad: &[u16], records: &Records) -> Vec<u16> {
    let strength = |id: u16| u32::from(records.player(id).map_or(0, |p| p.strength()));
    let mut fit: Vec<u16> = squad.iter().copied().filter(|&id| available(records, id)).collect();
    fit.sort_by_key(|&id| std::cmp::Reverse(strength(id)));
    let keeper = fit
        .iter()
        .copied()
        .find(|&id| records.player(id).is_some_and(|p| p.positions()[0] == world::GOALKEEPER));
    let mut eleven: Vec<u16> = keeper.into_iter().collect();
    eleven.extend(fit.iter().copied().filter(|&id| {
        id != keeper.unwrap_or(0)
            && records.player(id).is_some_and(|p| p.positions()[0] != world::GOALKEEPER)
    }));
    eleven.truncate(11);
    eleven
}

/// Which of the eleven scored a goal: drawn over their weights, or of
/// all of them alike when none of them weighs aught.
fn draw(weights: &[u32], random: &mut Random) -> Option<usize> {
    if weights.is_empty() {
        return None;
    }
    let total: u32 = weights.iter().sum();
    let cap = if total == 0 {
        u32::try_from(weights.len()).unwrap_or(1)
    } else {
        total
    };
    let mut draw = random.below(cap.max(1));
    if total == 0 {
        return Some((draw as usize).min(weights.len() - 1));
    }
    for (k, &w) in weights.iter().enumerate() {
        if draw < w {
            return Some(k);
        }
        draw -= w;
    }
    Some(weights.len() - 1)
}

/// How many the club's stadium holds (`0x1a48138[index]` +0x3c).
fn capacity(records: &Records, nation: u16, team: u16) -> i32 {
    records
        .team_index(nation, team)
        .and_then(|i| records.block(&format!("{STADIUM_AT:#x}[{i}]")))
        .map_or(0, |b| {
            b.get(CAPACITY..CAPACITY + 4).map_or(0, |c| i32::from_le_bytes(c.try_into().unwrap()))
        })
}

/// How many come to one of the club's home matches: the plan's attendance
/// with three in ten off to three in ten on, held up by its season-ticket
/// holders and capped by the stadium when it holds places. The original
/// draws its crowd over the plan too (`FUN_0078e5e6`, its clubs' fills a
/// little above and below the plan); the replica's luck rides a tenth to
/// two tenths about it.
fn attendance(team: &Team, capacity: i32, random: &mut Random) -> i32 {
    let plan = team.get::<i32>(PLAN_ATTENDANCE + CLUB_TO_TEAM);
    if plan <= 0 {
        return 0;
    }
    let season = team.get::<i32>(PLAN_SEASON_TICKETS + CLUB_TO_TEAM).max(0);
    let luck = 70 + i32::try_from(random.below(61)).unwrap_or(70);
    let a = plan.wrapping_mul(luck) / 100;
    let a = a.max(season);
    if capacity > 0 { a.min(capacity) } else { a }
}

/// How many goals one side scores past the other: its midfield and attack
/// over their goalkeeper and defenders — in tenths, GOAL_UNITS of the
/// difference being a goal, and the home side playing the stronger by
/// HOME_EDGE — with a luck from nought to nine lifting the count by half of
/// what it is beyond two, and sharing the goals over the halves by its
/// remainder in threes. Calibrated
/// on a save's played matchdays, which bring some two goals to a game.
fn goals(att: [i16; 4], hold: [i16; 4], edge: i32, random: &mut Random) -> (u8, u8) {
    let attack = i32::from(att[2]) + i32::from(att[3]);
    let defence = i32::from(hold[0]) + i32::from(hold[1]);
    let luck = random.below(10);
    let full =
        ((attack - defence + edge) / GOAL_UNITS + (i32::try_from(luck).unwrap_or(0) - 2) / 2)
            .clamp(0, 6) as u8;
    let half = (u32::from(full) * (luck % 3) / 2) as u8;
    (full, half)
}

#[cfg(test)]
mod supercup_tests {
    use super::*;

    /// The supercups played over their days from the original's new
    /// season (NewSeason000, the supercups set up): Germany's tournament
    /// moves each winner on to the waiting seed and into the final, every
    /// tie decided; England's one final decided on its day; Italy's two
    /// legs stand open after the first and decided after the second; the
    /// human's calendar takes his club's scores, and the final's winner's
    /// trainer the title.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_supercups_are_played_on_their_days() {
        use a3_files::an3::Save;
        use crate::supercup::{FINAL_TIE, OBJECT, SIX, TIE, TIES};
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let mut records = Records::from_save(Save::read(&keeps.join("NewSeason000.an3")).unwrap()).unwrap();
        let data = crate::data::Data::load(&dir).unwrap();
        crate::year::open_season(&mut records, &data.calendar);
        let mut random = Random::new(8);
        let object = |r: &Records, country: u16| -> Vec<u8> {
            let slot = r.countries.slot(usize::from(country));
            r.block(&format!("{OBJECT:#x}[{slot}]")).unwrap().to_vec()
        };
        let six = object(&records, 14);
        let titles: Vec<i16> = records.persons.iter().map(|p| p.get::<i16>(0x42)).collect();
        for day in [33, 34] {
            for country in [14u16, 16, 28] {
                supercup_day(&mut records, country, &data.calendar, day, &mut random);
            }
        }
        let b = object(&records, 14);
        for r in 0..2 {
            let (tie, next) = (TIES + r * TIE, TIES + (r + 2) * TIE);
            assert_eq!(b[tie + 0xc] & (CUP_RESULT | CUP_PLAYED), CUP_RESULT | CUP_PLAYED, "tie {r} decided");
            let w = usize::from(b[tie + 0xd]) * 4;
            assert_eq!(b[next..next + 4], b[tie + w..tie + w + 4], "tie {r}'s winner moves on");
            assert_eq!(b[next + 6..next + 8], six[SIX + 2 * r..SIX + 2 * r + 2], "against the seed {r}");
        }
        for day in [40, 41, 45] {
            for country in [14u16, 16, 28] {
                supercup_day(&mut records, country, &data.calendar, day, &mut random);
            }
        }
        let b = object(&records, 14);
        for r in 2..4 {
            let tie = TIES + r * TIE;
            assert_ne!(b[tie + 0xc] & CUP_PLAYED, 0, "tie {r} decided");
            let w = usize::from(b[tie + 0xd]) * 4;
            let side = FINAL_TIE + (r - 2) * 4;
            assert_eq!(b[side..side + 4], b[tie + w..tie + w + 4], "tie {r}'s winner in the final");
        }
        assert_ne!(b[FINAL_TIE + 0xc] & CUP_PLAYED, 0, "Germany's final decided");
        // Its winner's trainer counts the title.
        let w = FINAL_TIE + usize::from(b[FINAL_TIE + 0xd]) * 4;
        let winner = records.team(u16::from(b[w]), u16::from_le_bytes([b[w + 2], b[w + 3]])).unwrap();
        if winner.humans()[1] == 0 {
            let [mt, tr, _] = winner.staff();
            let id = usize::from(if tr != 0 { tr } else { mt });
            assert_eq!(records.persons[id].get::<i16>(0x42), titles[id] + 1, "the winner's trainer {id}");
        }
        // The final heads the list of finals, the year before it.
        let finals = statistics::league_cup_finals(&records, 14);
        assert_eq!(finals.first().map(|f| f.0), Some(records.globals.start_year() as i16));
        let lists = records.block(FINALS_BLOCK).unwrap();
        let slot = usize::from(records.countries.slot(14));
        let last = FINALS_AT + slot * FINALS_PER_SLOT + FINALS_PER_SLOT - FINAL_ENTRY;
        assert_eq!(lists[last + 2..last + FINAL_ENTRY], b[FINAL_TIE..FINAL_TIE + FINAL_ENTRY - 2]);
        let england = object(&records, 16);
        assert_ne!(england[FINAL_TIE + 0xc] & CUP_PLAYED, 0, "England's final decided");
        let italy = object(&records, 28);
        assert_eq!(italy[FINAL_TIE + 0xc] & (CUP_RESULT | CUP_PLAYED), CUP_RESULT, "Italy's first leg open");
        supercup_day(&mut records, 28, &data.calendar, 55, &mut random);
        let italy = object(&records, 28);
        assert_ne!(italy[FINAL_TIE + 0xc] & CUP_PLAYED, 0, "Italy's second leg decides");
        // Union's days: every supercup day it played holds a score.
        let cal = &records.calendars[0];
        let played: Vec<usize> = (0..60)
            .filter(|&d| {
                let e = cal.entry(d);
                e[0] == YEAR_MATCH && u32::from_le_bytes([e[4], e[5], e[6], e[7]]) & 0x7f00_0000 == SUPERCUP_CODE
            })
            .collect();
        assert!(!played.is_empty());
        assert!(played.iter().all(|&d| cal.entry(d)[YEAR_FLAGS] & YEAR_RESULT != 0), "{played:?}");
    }

    /// England's league cup played over its days from the original's
    /// NewSeason000 (its first round drawn there): every round's ties
    /// decided, two legs where the calendar plays two, the 25 winners of
    /// the first round and the seven waiting met in the second's 16 ties,
    /// on to one final with its winner, the last drawn round following,
    /// the winner's trainer counting the title.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_league_cup_is_played_on_its_days() {
        use a3_files::an3::Save;
        use crate::cup_draw::LEAGUE_CUP;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let mut records = Records::from_save(Save::read(&keeps.join("NewSeason000.an3")).unwrap()).unwrap();
        let data = crate::data::Data::load(&dir).unwrap();
        let mut random = Random::new(9);
        let titles: Vec<i16> = records.persons.iter().map(|p| p.get::<i16>(0x42)).collect();
        let row = data.calendar.row(ENGLAND, 0).unwrap();
        let days: Vec<usize> = (0..364).filter(|&d| data.calendar.entry(row, d) & 0x7f00_0000 == LEAGUE_CUP_CODE).collect();
        assert!(days.len() >= 8, "{days:?}");
        for &d in &days {
            league_cup_day(&mut records, ENGLAND, &data.calendar, d, &mut random);
        }
        let b = records.block(LEAGUE_CUP).unwrap();
        let mode = usize::from(u16::from_le_bytes([b[4], b[5]]));
        for round in 1..=6 {
            let Some((start, count)) = cup_pairings(mode, round) else { continue };
            for q in start..start + count {
                let p = &b[PAIRINGS + q * PAIRING..PAIRINGS + (q + 1) * PAIRING];
                assert!(p[0] != 0 && p[4] != 0, "round {round} tie {q} drawn");
                assert_ne!(p[0xc] & CUP_PLAYED, 0, "round {round} tie {q} decided");
            }
        }
        assert_eq!(cup_pairings(mode, 2).map(|r| r.1), Some(16));
        assert_eq!(b[0x28 - 0x20], 6, "the final the last drawn round");
        // The final's winner's trainer counts the title.
        let (start, _) = cup_pairings(mode, 6).unwrap();
        let p = &b[PAIRINGS + start * PAIRING..PAIRINGS + (start + 1) * PAIRING];
        let w = usize::from(p[0xd] != 0) * 4;
        let (c, n) = (u16::from(p[w]), u16::from_le_bytes([p[w + 2], p[w + 3]]));
        let winner = records.team(c, n).unwrap();
        assert_eq!(winner.humans()[1], 0, "a computer club wins");
        let [mt, tr, _] = winner.staff();
        let id = usize::from(if tr != 0 { tr } else { mt });
        assert_eq!(records.persons[id].get::<i16>(0x42), titles[id] + 1, "the winner's trainer {id}");
    }
}
