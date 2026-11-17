//! A club's search plan (team +0x34c, 0x28 bytes; `FUN_0069d0ca` reads
//! it, `FUN_0069d125` writes it back): two budgets (+0, +4, both moved by
//! `FUN_006a018f`), the count of wanted positions (+0x12) and up to three
//! of them from +0x14, six bytes each — the position, the strength wished
//! for, how badly (or how far the weakest falls short), two marks.
//!
//! After a transfer (`FUN_0070ca69`'s tail, either way it went) the buyer
//! pays the fee and a thousand times the salary out of both budgets, the
//! seller gains half the fee, and each plans its search anew
//! (`FUN_006a03fb` with its second argument nought).

use crate::random::Random;
use crate::records::{Player, Records};

/// The plan in the team record, its count and its wanted positions.
const PLAN: usize = 0x34c;
const PLAN_SIZE: usize = 0x28;
const COUNT: usize = 0x12;
const WANTED: usize = 0x14;
const WANTED_SIZE: usize = 6;
const WANTED_MOST: i16 = 3;
/// The club's level in hundredths of a strength (`FUN_0069d16e`).
const LEVEL: usize = 0x348;
/// How many a squad holds of each position before it wants one
/// (`0x8a4e84`).
const ENOUGH: [i16; 11] = [0, 1, 1, 3, 1, 1, 1, 1, 1, 1, 3];
/// A man out this many days or more is not counted at his position.
const OUT: i32 = 0x2a;
/// A list of this many or more plans nothing.
const LIST_MOST: usize = 0x1f;
/// The months in which the squad counts as it stands.
const SUMMER: std::ops::RangeInclusive<u16> = 7..=9;
/// The salary's unit (`DAT_00a3b3e4`).
const THOUSAND: i32 = 1000;
/// R+0x26f: he retires; R+0x15e: on loan.
const RETIRING: u8 = 0x80;
const ON_LOAN: u16 = 0x100;

/// The search plans after a transfer of `fee` on a contract of `salary`
/// thousands (`FUN_0070ca69` asm 0x70d13a–0x70d1da).
pub(crate) fn after_transfer(
    records: &mut Records,
    random: &mut Random,
    seller: (u8, i16),
    buyer: (u8, i16),
    fee: u32,
    salary: u16,
) {
    let mut plan = read(records, buyer);
    spend(&mut plan, (fee as i32).wrapping_neg());
    spend(&mut plan, i32::from(salary).wrapping_neg().wrapping_mul(THOUSAND));
    plan_anew(records, random, buyer, &mut plan);
    write(records, buyer, &plan);
    let mut plan = read(records, seller);
    spend(&mut plan, fee as i32 / 2);
    plan_anew(records, random, seller, &mut plan);
    write(records, seller, &plan);
}

fn read(records: &Records, club: (u8, i16)) -> [u8; PLAN_SIZE] {
    team_of(records, club).map_or([0; PLAN_SIZE], |t| t.get(PLAN))
}

fn write(records: &mut Records, club: (u8, i16), plan: &[u8; PLAN_SIZE]) {
    let Ok(number) = u16::try_from(club.1) else { return };
    if let Some(team) = records.team_mut(u16::from(club.0), number) {
        team.put(PLAN, *plan);
    }
}

fn team_of(records: &Records, club: (u8, i16)) -> Option<&crate::records::Team> {
    records.team(u16::from(club.0), u16::try_from(club.1).ok()?)
}

/// Both budgets moved by `by` (`FUN_006a018f`).
fn spend(plan: &mut [u8; PLAN_SIZE], by: i32) {
    for at in [4, 0] {
        let v = i32::from_le_bytes(plan[at..at + 4].try_into().unwrap()).wrapping_add(by);
        plan[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
}

fn count(plan: &[u8; PLAN_SIZE]) -> i16 {
    i16::from_le_bytes([plan[COUNT], plan[COUNT + 1]])
}

fn set_count(plan: &mut [u8; PLAN_SIZE], n: i16) {
    plan[COUNT..COUNT + 2].copy_from_slice(&n.to_le_bytes());
}

/// A wanted position onto the plan, its count up by one.
fn want(plan: &mut [u8; PLAN_SIZE], position: usize, strength: u8, need: u16, marks: [u8; 2]) {
    let n = count(plan);
    let at = WANTED + usize::try_from(n).unwrap_or(0) * WANTED_SIZE;
    if let Some(e) = plan.get_mut(at..at + WANTED_SIZE) {
        e[0] = position as u8;
        e[1] = strength;
        e[2..4].copy_from_slice(&need.to_le_bytes());
        e[4..6].copy_from_slice(&marks);
    }
    set_count(plan, n.wrapping_add(1));
}

/// A position's five most promising men, the best first (the 0x1a bytes
/// `FUN_006a0f20` clears): their count, their ids and promises, a sixth
/// place for the one pushed out.
#[derive(Clone, Copy, Default)]
struct Best {
    count: i16,
    ids: [u16; 6],
    values: [i16; 6],
}

impl Best {
    /// Him in by his promise, above the fifth's (`FUN_006a0f40`).
    fn take(&mut self, id: u16, value: i16) {
        if self.values[4] >= value {
            return;
        }
        let mut at = usize::try_from(self.count).unwrap_or(0).min(5);
        while at > 0 && self.values[at - 1] < value {
            self.values[at] = self.values[at - 1];
            self.ids[at] = self.ids[at - 1];
            at -= 1;
        }
        self.values[at] = value;
        self.ids[at] = id;
        if self.count < 5 {
            self.count += 1;
        }
    }

    /// The list cut at him (`FUN_006a1000`): he and those behind him off,
    /// the places up to the fourth cleared.
    fn cut(&mut self, id: u16) {
        let mut k = i32::from(self.count).min(5);
        while k >= 0 {
            if self.ids[k as usize] == id {
                self.count = k as i16;
                while k < 4 {
                    self.ids[k as usize] = 0;
                    self.values[k as usize] = 0;
                    k += 1;
                }
            }
            k -= 1;
        }
    }
}

/// Whether he stays for next season (`FUN_00707350`): not passed over,
/// no next club, not retiring, not on loan.
fn stays(p: &Player) -> bool {
    p.picks() >> 6 != 1 && p.next_club()[0] == 0 && p.retiring() & RETIRING == 0 && p.terms() & ON_LOAN == 0
}

/// The club's wanted positions planned anew (`FUN_006a03fb`, its second
/// argument nought). A plan over budget, or a club with a human manager
/// (club +0x8b, team +0xa9), wants nothing. The men weighed are the squad
/// from July to September, else those of it who stay and those it has
/// bought or will have back from loan for next season (`FUN_0070f041`'s
/// lists 1 and 2); thirty-one or more want nothing. Each goes to his
/// position's five best by his promise (`FUN_006ff516`); the men at a
/// position not out six weeks are counted.
///
/// A position from the second to the tenth where no more than enough
/// (`0x8a4e84`) are fit is wanted at the club's level plus one, need 5 —
/// at once when more than enough are among the best, else on a draw of
/// two in five, which is drawn even with the plan full. Then three rounds
/// (each position once at most): a position with nobody is wanted, need
/// 3; else the weakest that counts (the one past enough, or the last) of
/// all positions is found — none, and the plan is done; at or above the
/// level he is struck off with those behind him, below it his position is
/// wanted with the gap for need.
fn plan_anew(records: &Records, random: &mut Random, club: (u8, i16), plan: &mut [u8; PLAN_SIZE]) {
    if i32::from_le_bytes(plan[0..4].try_into().unwrap()) < 0 {
        set_count(plan, 0);
        return;
    }
    let Some(team) = team_of(records, club) else { return };
    if team.humans()[1] != 0 {
        set_count(plan, 0);
        return;
    }
    let summer = SUMMER.contains(&records.globals.date()[1]);
    let mut fit = [0i16; 11];
    let mut men = Vec::new();
    for id in team.players() {
        let Some(p) = records.player(id) else { continue };
        if summer || stays(p) {
            men.push(id);
        }
        let out = |to| crate::load::days_to(&records.globals, to);
        if out(p.injured_to()) < OUT && out(p.long_to()) < OUT {
            let at = usize::from(p.positions()[0]).min(10);
            fit[at] = fit[at].wrapping_add(1);
        }
    }
    if !summer {
        let agreed = records.agreed();
        let coming = agreed.iter().filter(|a| a.seller != club && a.buyer == club);
        men.extend(coming.clone().filter(|a| !a.flagged).map(|a| a.player));
        men.extend(coming.filter(|a| a.flagged).map(|a| a.player));
    }
    if men.len() >= LIST_MOST {
        set_count(plan, 0);
        return;
    }
    let mut best = [Best::default(); 11];
    for &id in &men {
        let Some(p) = records.player(id) else { continue };
        let value = crate::free_agents::promise(p, crate::free_agents::age(records, p));
        best[usize::from(p.positions()[0]).min(10)].take(id, value);
    }
    let level = f32::from(team.get::<i16>(LEVEL)) / 100.0;
    let wish = (f64::from(level) + 1.0) as i32 as u8;
    set_count(plan, 0);
    for at in 2..=10 {
        if fit[at] > ENOUGH[at] {
            continue;
        }
        if best[at].count <= ENOUGH[at] && random.below(5) > 1 {
            continue;
        }
        if count(plan) < WANTED_MOST {
            want(plan, at, wish, 5, [1, 2]);
        }
    }
    let mut taken = [false; 11];
    let mut round = 0;
    while round < 3 && count(plan) < WANTED_MOST {
        let (mut least, mut who, mut at) = (99i16, 0i32, 0usize);
        for k in 1..=10 {
            if taken[k] {
                continue;
            }
            let (b, enough) = (&best[k], ENOUGH[k] as usize);
            if b.count > ENOUGH[k] {
                if b.values[enough] <= least {
                    (least, who, at) = (b.values[enough], i32::from(b.ids[enough]), k);
                }
            } else if b.count < 1 {
                want(plan, k, wish, 3, [1, 1]);
                taken[k] = true;
                who = -1;
                break;
            } else {
                let last = b.count as usize - 1;
                if b.values[last] < least {
                    (least, who, at) = (b.values[last], i32::from(b.ids[last]), k);
                }
            }
        }
        if who == 0 {
            return;
        }
        if who != -1 {
            let gap = f64::from(level) - f64::from(least);
            // The original strikes him off too when the list held -1 men
            // (asm 0x6a0cbb), which it never does.
            if gap <= 0.0 {
                best[at].cut(who as u16);
            } else {
                want(plan, at, wish, gap as i32 as u16, [u8::from(summer), 1]);
                taken[at] = true;
            }
        }
        round += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_best_keep_five_and_cut_at_a_man() {
        let mut b = Best::default();
        for (id, v) in [(1, 4), (2, 6), (3, 5), (4, 4), (5, 7), (6, 5), (7, 4)] {
            b.take(id, v);
        }
        assert_eq!((b.count, b.ids, b.values), (5, [5, 2, 3, 6, 1, 4], [7, 6, 5, 5, 4, 4]));
        b.cut(3);
        assert_eq!((b.count, b.ids, b.values), (2, [5, 2, 0, 0, 1, 4], [7, 6, 0, 0, 4, 4]));
    }

    #[test]
    fn a_transfer_moves_both_budgets() {
        let mut plan = [0u8; PLAN_SIZE];
        plan[0..4].copy_from_slice(&2_500_000i32.to_le_bytes());
        spend(&mut plan, -548_000);
        spend(&mut plan, -60 * THOUSAND);
        let word = |at: usize| i32::from_le_bytes(plan[at..at + 4].try_into().unwrap());
        assert_eq!((word(0), word(4)), (1_892_000, -608_000));
    }
}
