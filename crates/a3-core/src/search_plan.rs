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
    plan_anew(records, random, buyer, &mut plan, false);
    write(records, buyer, &plan);
    let mut plan = read(records, seller);
    spend(&mut plan, fee as i32 / 2);
    plan_anew(records, random, seller, &mut plan, false);
    write(records, seller, &plan);
}

/// The plan's base budget (+0xc) and the mark its November planning
/// sets (+0x10, an i16; who lowers it again is not traced).
const BASE: usize = 0xc;
const NOVEMBER_MARK: usize = 0x10;
/// A mark above this keeps the November planning off; it is set to this.
const NOVEMBER_KEPT: i16 = 10;
const NOVEMBER_SET: i16 = 12;
/// November, and the days of it the planning runs in.
const NOVEMBER: u16 = 11;
const NOVEMBER_DAYS: u16 = 8;
/// The months the planning with its mark stays off (`FUN_006a03fb`).
const AUTUMN: std::ops::RangeInclusive<u16> = 7..=10;
/// A budget this high takes nothing more (`DAT_00a3b2a0`, 200 Mio).
const REFILL_BELOW: i32 = 200_000_000;
/// What a week adds by the club's rating: below 0.3 half a million, then
/// above each mark its sum (`DAT_00a3b2a4` twice … `DAT_00a3b32c`), up to
/// 0.8 nothing.
const POOR_RATING: f64 = 0.3;
const POOR_REFILL: i32 = 500_000;
const REFILLS: [(f64, i32); 9] = [
    (8.8, 240_000_000),
    (7.8, 120_000_000),
    (6.8, 60_000_000),
    (5.8, 30_000_000),
    (4.8, 16_000_000),
    (3.8, 9_000_000),
    (2.8, 5_000_000),
    (1.8, 2_500_000),
    (0.8, 1_200_000),
];
/// The licence's transfer sum, in the club object, by its unit
/// (`DAT_00a3b37c`).
const TRANSFER_SUM: usize = 0x5dc + crate::finances::CLUB_TO_TEAM;
const SUM_UNIT: i32 = 100_000;

/// A computer club's plan at its week on the market (`FUN_004279f8`,
/// after the players it scouted): the budget starts again from the base
/// (`FUN_0069ff5e`) — below 200 Mio with a sum by the club's rating —,
/// cut to the licence's transfer sum where the association set one
/// (club +0x101's 0x10, +0x5dc), and the second budget added, there only
/// when above nought. In November's first seven days the wanted
/// positions are planned anew (`FUN_006a03fb` with its second argument
/// one) unless the plan's mark stands above ten, which then goes to
/// twelve.
pub(crate) fn refill(records: &mut Records, random: &mut Random, club: (u8, i16)) {
    let Some(team) = team_of(records, club) else { return };
    let mut plan = read(records, club);
    let budget = refilled(records, team, &plan);
    plan[0..4].copy_from_slice(&budget.to_le_bytes());
    let [day, month, _] = records.globals.date();
    if month == NOVEMBER && day < NOVEMBER_DAYS {
        plan_anew(records, random, club, &mut plan, true);
    }
    write(records, club, &plan);
}

/// The budget the week's refill leaves (`FUN_0069ff5e`).
fn refilled(records: &Records, team: &crate::records::Team, plan: &[u8; PLAN_SIZE]) -> i32 {
    let mut budget = long(plan, BASE);
    if budget < REFILL_BELOW {
        let r = f64::from(rating(records, team));
        let add = if r < POOR_RATING {
            POOR_REFILL
        } else {
            REFILLS.iter().find(|&&(above, _)| r > above).map_or(0, |&(_, add)| add)
        };
        budget = budget.wrapping_add(add);
    }
    let extra = long(plan, 4);
    if crate::licence::conditions(team) & crate::licence::TRANSFER_BUDGET != 0 {
        budget = budget.min(i32::from(team.get::<i16>(TRANSFER_SUM)).wrapping_mul(SUM_UNIT));
        if extra > 0 {
            budget = budget.wrapping_add(extra);
        }
    } else {
        budget = budget.wrapping_add(extra);
    }
    budget
}

fn long(plan: &[u8; PLAN_SIZE], at: usize) -> i32 {
    i32::from_le_bytes(plan[at..at + 4].try_into().unwrap())
}

/// The club's rating, 0 to 9 (`FUN_0069d1e4`): its class (team +0x875),
/// 0.4 for the honours' bit 0x40 and 0.7 for 0x80 (team +0x8ea), the
/// leader of a first league a fiftieth for each matchday played, 0.3 for
/// a cup run, 0.5 promoted, a whole one less relegated (team +0x110), 0.2
/// in the first league's top five.
fn rating(records: &Records, team: &crate::records::Team) -> f32 {
    const CLASS: usize = 0x875;
    const HONOURS: usize = 0x8ea;
    const RUNS: usize = 0x110;
    const CUP_RUN: u16 = 0x1000;
    let mut r = f32::from(team.get::<u8>(CLASS));
    let honours = team.get::<u8>(HONOURS);
    if honours & 0x40 != 0 {
        r += 0.4;
    }
    if honours & 0x80 != 0 {
        r += 0.7;
    }
    let (level, place) = (team.level(), crate::league::place(team));
    if place == 1 && level == 0 {
        let played = crate::league::matchdays(records, team) as i32;
        r = (f64::from(played) * f64::from(0.02f32) + f64::from(r)) as f32;
    }
    let runs = team.get::<u16>(RUNS);
    if runs & CUP_RUN != 0 {
        r += 0.3;
    }
    if runs & crate::promotion::PROMOTED != 0 {
        r += 0.5;
    }
    if runs & crate::promotion::RELEGATED != 0 {
        r -= 1.0;
    }
    if level == 0 && place < 6 {
        r += 0.2;
    }
    r.clamp(0.0, 9.0)
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

/// The club's wanted positions planned anew (`FUN_006a03fb`; with its
/// second argument, `marked`, not from July to October nor while the
/// plan's mark stands above ten, which it then sets to twelve). A plan
/// over budget, or a club with a human manager
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
fn plan_anew(records: &Records, random: &mut Random, club: (u8, i16), plan: &mut [u8; PLAN_SIZE], marked: bool) {
    if i32::from_le_bytes(plan[0..4].try_into().unwrap()) < 0 {
        set_count(plan, 0);
        return;
    }
    let Some(team) = team_of(records, club) else { return };
    if team.humans()[1] != 0 {
        set_count(plan, 0);
        return;
    }
    let month = records.globals.date()[1];
    if marked {
        let mark = i16::from_le_bytes([plan[NOVEMBER_MARK], plan[NOVEMBER_MARK + 1]]);
        if AUTUMN.contains(&month) || mark > NOVEMBER_KEPT {
            return;
        }
        plan[NOVEMBER_MARK..NOVEMBER_MARK + 2].copy_from_slice(&NOVEMBER_SET.to_le_bytes());
    }
    let summer = SUMMER.contains(&month);
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

    /// Every computer club of the played and kept countries' first six
    /// leagues holds the budget its last week's refill left, in each kept
    /// save of a running season; in November's first week a plan whose mark
    /// stands at eleven keeps its wanted positions, one at nought plans
    /// them anew and marks twelve.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_computer_clubs_budgets_refill_as_the_original_keeps_them() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(format!("{name}.an3"))).unwrap()).unwrap();
        for name in ["A", "A2", "S2c", "S2late3", "SeasonEnd364"] {
            let records = load(name);
            let mut clubs = 0;
            let kept = (1..0x75u16).filter(|&c| records.countries.kind(usize::from(c)) == 2);
            for country in crate::opponent::played_nations(&records).into_iter().chain(kept) {
                for number in 0..records.countries.team_count(usize::from(country)).max(0) as u16 {
                    let Some(team) = records.team(country, number) else { continue };
                    if team.level() >= 6 || team.humans() != [0, 0] {
                        continue;
                    }
                    let plan: [u8; PLAN_SIZE] = team.get(PLAN);
                    assert_eq!(refilled(&records, team, &plan), long(&plan, 0), "{name}: {country}/{number}");
                    clubs += 1;
                }
            }
            assert_eq!(clubs, 403, "{name}");
        }
        let mut records = load("A");
        let [_, _, year] = records.globals.date();
        records.globals.set_date([3, NOVEMBER, year]);
        let club = (14u8, 7i16);
        let mut random = Random::new(12345);
        let mark = |records: &Records| {
            let plan = read(records, club);
            (i16::from_le_bytes([plan[NOVEMBER_MARK], plan[NOVEMBER_MARK + 1]]), plan[COUNT..].to_vec())
        };
        let with_mark = |records: &mut Records, m: i16| {
            let mut plan = read(records, club);
            plan[NOVEMBER_MARK..NOVEMBER_MARK + 2].copy_from_slice(&m.to_le_bytes());
            plan[COUNT..].fill(0xee);
            write(records, club, &plan);
        };
        with_mark(&mut records, 11);
        refill(&mut records, &mut random, club);
        assert_eq!(mark(&records), (11, vec![0xee; PLAN_SIZE - COUNT]));
        with_mark(&mut records, 0);
        refill(&mut records, &mut random, club);
        let (m, wanted) = mark(&records);
        assert_eq!(m, NOVEMBER_SET);
        assert!((0..=WANTED_MOST).contains(&i16::from_le_bytes([wanted[0], wanted[1]])), "{wanted:?}");
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
