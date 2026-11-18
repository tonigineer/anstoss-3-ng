//! The doping control after a league match. Building a league's matchday
//! (`FUN_00690a9e`) the original marks one of its matches for the control
//! (the match record's +0x460, `FUN_0078a810(teams / 2)`); before it is
//! played (`FUN_006ef296`) each side has two of its men picked
//! (`FUN_005dad80`, the live player's +0x57 bit 0x40); after it the
//! settlement (`FUN_007108d3` → `FUN_00741aef`) tests the picked men of a
//! human's club (`FUN_005dae5f`) and posts event 0xcb, which opens dialog
//! 41 on the result it leaves at `DAT_00a2e0b8`.
//!
//! A sample is found positive only on a dose that can be traced (the
//! doping byte's 0x80, set by the dose when its roll of 100 lay at or
//! above the dialog's risk, `FUN_006ff6bc`), and then only when a roll of
//! 100 lies at or above twenty and the medical department's shield. The
//! B sample then decides: a roll of twenty below three clears him — gone,
//! lost, no agent found — else he is banned and his club counts the case.
//!
//! The replica scores its league matches without the original's live
//! players, so the pick is not kept on the players' match bits: it is
//! made and tested at once, once the match has been scored.

use crate::finances::CLUB_TO_TEAM;
use crate::random::Random;
use crate::records::Records;

/// The medical budget spent and planned (club +0x2ac and +0x1a4): spent
/// above 1,000,000 the plan's every 50,000 above it shields two more in a
/// hundred, at most forty (`DAT_00a3b330`, `DAT_00a3b38c`).
const MEDICAL_SPENT: usize = 0x2ac + CLUB_TO_TEAM;
const MEDICAL_PLAN: usize = 0x1a4 + CLUB_TO_TEAM;
const BUDGET_FREE: i32 = 1_000_000;
const BUDGET_STEP: i32 = 50_000;
const SHIELD_MOST: i16 = 40;
/// A sample that can be traced is still missed below this (`+ 0x14`).
const MISSED: i16 = 20;
/// The B sample's roll of twenty; below three it clears him.
const B_ROLL: u32 = 20;
pub const B_CLEARS: u8 = 3;
/// The doping byte (R+0x26d): doped in bits 0–1, 0x80 traceable.
const DOPED: u8 = 0x03;
const TRACEABLE: u8 = 0x80;
/// The club's flags (club +0xfd, team +0x874): bit 0 spares the club the
/// next control, and goes.
const FLAGS: usize = 0x874;
const SPARED: u8 = 0x01;
/// The club's doping cases (club +0x115, team +0x13e): a count to five
/// in the low three bits, 0x80 once there was one.
const CASES: usize = 0x13e;
const CASES_COUNT: u8 = 0x07;
const CASES_MOST: u8 = 5;
const CASES_SEEN: u8 = 0x80;
/// The player's earlier cases (L+0x31, R+0x22b, `FUN_0062965d`): 0x80
/// once, 0x40 twice; a ban's days by them (`FUN_005db561`): five years
/// for a third, a draw of the table at `0x88d5a0` for a second, else
/// ninety, 180 or 270 days.
const EARLIER: usize = 0x22b;
const ONCE: u8 = 0x80;
const TWICE: u8 = 0x40;
const THIRD_BAN: i32 = 1800;
const SECOND_BANS: [i32; 10] = [720, 720, 1080, 1080, 1080, 1440, 1440, 1440, 1800, 1800];
const FIRST_BAN: i32 = 90;
/// A ban's mark on the long absence (L+0x68 = R+0x26e, `FUN_005db517`).
const BANNED: u8 = 0x80;
/// The squad slots the pick draws from: the eleven, and seven on the bench.
const BENCH: u32 = 7;
const BENCH_TRIES: u32 = 3;
/// At most two men are tested.
pub const TESTED_MOST: usize = 2;

/// What the control of a human's club leaves for dialog 41
/// (`DAT_00a2e0b8`): the club (+2, +4), how many were tested (+0), their
/// ids (+6), whether the A sample was positive (+0xa) and the B sample's
/// roll (+0xe).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Control {
    pub club: (u8, i16),
    pub count: u8,
    pub players: [u16; TESTED_MOST],
    pub positive: [bool; TESTED_MOST],
    pub b_sample: [u8; TESTED_MOST],
}

impl Control {
    /// Whether the `k`th man stands banned: positive, and the B sample
    /// with him.
    pub fn banned(&self, k: usize) -> bool {
        self.positive[k] && self.b_sample[k] >= B_CLEARS
    }
}

/// Which of a matchday's matches is controlled: one of the league's
/// pairs, by its place among them (`FUN_00690a9e`).
pub fn controlled(teams: usize, random: &mut Random) -> usize {
    random.below(u32::try_from(teams / 2).unwrap_or(0)) as usize
}

/// The men picked on a side of the controlled match (`FUN_005dad80`): a
/// club spared this once has its flag taken and nobody picked; else one
/// of the eleven, and one of the bench — three draws of its seven places
/// to find one — or, with none found, another of the eleven.
pub fn pick(records: &mut Records, nation: u16, number: u16, random: &mut Random) -> Vec<u16> {
    let Some(team) = records.team_mut(nation, number) else { return Vec::new() };
    let flags = team.get::<u8>(FLAGS);
    if flags & SPARED != 0 {
        team.put::<u8>(FLAGS, flags & !SPARED);
        return Vec::new();
    }
    let (eleven, bench, squad) = (team.lineup(), team.bench(), team.players());
    let mut picked = Vec::new();
    // One of the eleven who plays for the club and is not picked yet
    // (`FUN_005dac94`, `FUN_006968c4`).
    let of_eleven = |picked: &mut Vec<u16>, random: &mut Random| {
        let free: Vec<u16> = eleven
            .iter()
            .copied()
            .filter(|&id| id != 0 && squad.contains(&id) && !picked.contains(&id))
            .collect();
        if !free.is_empty() {
            picked.push(free[random.below(free.len() as u32) as usize]);
        }
    };
    of_eleven(&mut picked, random);
    let mut place;
    let mut tries = 0;
    loop {
        place = random.below(BENCH) as usize;
        tries += 1;
        if tries == BENCH_TRIES || bench[place] != 0 {
            break;
        }
    }
    match bench[place] {
        0 => of_eleven(&mut picked, random),
        id => picked.push(id),
    }
    picked
}

/// The test of a side's picked men (`FUN_005dae5f`): only a human's club
/// is tested, its picked men in the squad's order, two at most.
pub fn test(records: &mut Records, nation: u16, number: u16, picked: &[u16], random: &mut Random) -> Option<Control> {
    let team = records.team(nation, number)?;
    let humans = team.humans();
    if humans == [0, 0] {
        return None;
    }
    let tested: Vec<u16> = team.players().into_iter().filter(|id| picked.contains(id)).take(TESTED_MOST).collect();
    if tested.is_empty() {
        return None;
    }
    // The shield, a short as the original keeps it (`uStack_dc`).
    let shield = if team.get::<i32>(MEDICAL_SPENT) > BUDGET_FREE {
        let more = ((team.get::<i32>(MEDICAL_PLAN) - BUDGET_FREE) / BUDGET_STEP * 2) as i16;
        more.min(SHIELD_MOST)
    } else {
        0
    };
    let mut control = Control {
        club: (u8::try_from(nation).unwrap_or(0), number as i16),
        count: tested.len() as u8,
        ..Control::default()
    };
    for (k, &id) in tested.iter().enumerate() {
        control.players[k] = id;
        let byte = records.player(id).map_or(0, |r| r.doping());
        let traceable = byte & DOPED != 0 && byte & TRACEABLE != 0;
        if !traceable || (random.below(100) as i16) < shield + MISSED {
            continue;
        }
        control.positive[k] = true;
        // With a human trainer, one time in a hundred the question 0x3f
        // (`FUN_005f2813`) offers to make the B sample go for 50,000 at
        // club +0x2c0; it is not built, so taken as refused.
        if humans[0] != 0 {
            random.below(100);
        }
        control.b_sample[k] = random.below(B_ROLL) as u8;
        if control.banned(k) {
            ban(records, id, random);
            if let Some(team) = records.team_mut(nation, number) {
                let mut cases = team.get::<u8>(CASES);
                if cases & CASES_COUNT < CASES_MOST {
                    cases += 1;
                }
                team.put::<u8>(CASES, cases | CASES_SEEN);
            }
        }
    }
    Some(control)
}

/// A doping ban (`FUN_005db517(player, -1)`): the days by his earlier
/// cases (`FUN_005db561`) as his long absence, marked a ban.
fn ban(records: &mut Records, id: u16, random: &mut Random) {
    let Some(earlier) = records.player(id).map(|r| r.get::<u8>(EARLIER)) else { return };
    let days = if earlier & TWICE != 0 {
        THIRD_BAN
    } else if earlier & ONCE != 0 {
        SECOND_BANS[random.below(SECOND_BANS.len() as u32) as usize]
    } else {
        (random.below(3) as i32 + 1) * FIRST_BAN
    };
    let to = crate::medical::date_in(&records.globals, days);
    if let Some(r) = records.player_mut(id) {
        r.set_long_to(to);
        r.set_long_unknown(BANNED);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_b_sample_bans_from_three() {
        let mut c = Control { count: 2, positive: [true, false], b_sample: [3, 19], ..Control::default() };
        assert!(c.banned(0));
        assert!(!c.banned(1), "a negative A sample bans nobody");
        c.b_sample[0] = 2;
        assert!(!c.banned(0));
    }
}
