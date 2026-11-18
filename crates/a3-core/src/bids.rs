//! A computer club's bids for the positions it wants (`FUN_004279f8`'s
//! middle, after its plan's weekly refill): on a draw of its odds under
//! the plan's chance (`FUN_006a0dcc`) it walks the transfer list for each
//! wanted position in turn (`FUN_0075905a`), takes the most promising man
//! it can pay for and wants, and books a talk with his club
//! (`FUN_00426a9c`) — and stops at the first position that found one.
//! None found anywhere, it looks among the free and foreign players
//! (`FUN_004266a6`, not built).

use crate::random::Random;
use crate::records::{Player, Records, Team};
use crate::search_plan::{Search, Wanted};
use crate::transfers::Side;

/// The days of the week each club has a talk booked on this week
/// (`0x8ea248`, a byte by the club's place among all teams — a foreign
/// club's from 900 —, a bit per day from the week's start; the week's
/// market clears it first, `FUN_004268b0`, and only the bookings here set
/// it, `FUN_004268ce`).
#[derive(Default)]
pub(crate) struct Busy(std::collections::HashMap<Side, u8>);

impl Busy {
    /// A club's days (`FUN_00426972`'s read): one of country nought has
    /// none.
    fn of(&self, club: Side) -> u8 {
        if club.0 == 0 { 0 } else { self.0.get(&club).copied().unwrap_or(0) }
    }

    fn mark(&mut self, club: Side, day: i16) {
        *self.0.entry(club).or_default() |= 1u8.checked_shl(day as u32 & 0x1f).unwrap_or(0);
    }
}

/// The deal a listing's kind serves (`DAT_008abbfc`: none, a sale, a loan,
/// a loan with an option), as the wanted position's +5 asks it, a
/// purchase for nought.
const DEAL_OF_KIND: [u8; 4] = [1, 1, 2, 2];
const PURCHASE: u8 = 1;
const LOAN: u8 = 2;
/// The age from which a reserve team bids for nobody (`FUN_006ff01b`).
const RESERVE_AGE: i32 = 24;
/// A listing's terms: free, at a quarter off.
const FREE: u8 = 2;
const QUARTER_OFF: u8 = 1;
/// After this day of the season (`FUN_0075827a`).
const LATE: i32 = 216;
/// The release fee's unit (`DAT_00a3b3bc`).
const RELEASE_UNIT: u64 = 10_000;
/// The strongest club level that still wants a man of each strength
/// (`0x8abbcc`, `FUN_00758468`).
const LEVEL_BY_STRENGTH: [u8; 24] = [10, 10, 10, 10, 3, 3, 3, 3, 1, 1, 1, 0, 0, 0, 0, 0, 5, 0, 4, 0, 2, 0, 1, 0];
const ENGLAND: u16 = 0x10;
/// The leagues' objects (`0x1a09d48[li]`, data from +0x20): the record's
/// +0x02 and +0x16 (its matchdays played) at the block's +0x32 and +0x46.
const LEAGUE_AT: u32 = 0x1a09d48;
const LEAGUE_DAYS: usize = 0x32;
const LEAGUE_PLAYED: usize = 0x46;
/// The last matchdays in which the table moves a club's level.
const LAST_MATCHDAYS: i16 = 5;
/// The places from the top and the bottom that move it.
const TOP: i32 = 4;
const BOTTOM: i32 = 3;

/// A computer club's bids of the week (`FUN_004279f8` asm
/// 0x427da1–0x428189): drawn against its odds, under the plan's chance.
/// For each wanted position the budget is its share (`FUN_006a0e9b`), and
/// the transfer list is walked for the men of that position whose outlook
/// at listing (E+8) lies within one of the strength wished for and whose
/// listing serves the deal: one of the club's country is taken, another
/// on a draw of two when his outlook is above the wish. A reserve team
/// takes nobody of 24 or more; his own club's man is passed by. A loan
/// is taken when its fee stays under two thirds of the budget, a
/// purchase when his price does not pass it (free, or a quarter off, as
/// listed) and his strength suits the club's level (`FUN_00758468`); of
/// those the best by promise a thousand times and the pick (an i16).
/// The first position that found one books the talk and ends the bids.
pub(crate) fn bid(
    records: &mut Records,
    rules: &crate::season::Rules,
    club: (u16, u16),
    odds: u32,
    busy: &mut Busy,
    random: &mut Random,
) {
    let drawn = random.below(odds) as i16;
    let side: Side = (club.0 as u8, club.1 as i16);
    let search = Search::of(records, side);
    let late = i32::from(records.globals.day()) > LATE;
    if drawn >= search.chance(late) {
        return;
    }
    let Some(team) = records.team(club.0, club.1) else { return };
    let reserve = team.linked()[0] >= 0;
    let entries = crate::transfers::list_entries(records);
    for (k, want) in search.wanted().iter().enumerate() {
        let budget = search.budget_for(k);
        let deal = if want.deal == 0 { PURCHASE } else { want.deal };
        let mut best: (i16, Side, i16) = (0, (0, 0), 0);
        for e in &entries {
            if !listed_as_wanted(e, want, deal, side.0, random) {
                continue;
            }
            let id = u16::from_le_bytes([e[0], e[1]]);
            let Some(p) = records.player(id) else { continue };
            let age = crate::free_agents::age(records, p);
            if reserve && age >= RESERVE_AGE {
                continue;
            }
            let price = match e[0xd] {
                FREE => 0,
                QUARTER_OFF => {
                    let p = price(records, rules, p);
                    (p >> 1) + (p >> 2)
                }
                _ => price(records, rules, p),
            };
            let seller: Side = (p.club()[0], i16::from(p.club()[1]));
            let score = (i32::from(crate::free_agents::promise(p, age)) * 1000
                + i32::from(rules.value.pick_value(p) as i16)) as i16;
            if seller == side {
                continue;
            }
            let takes = if want.deal == LOAN {
                let fee = u32::from_le_bytes([e[0x10], e[0x11], e[0x12], e[0x13]]);
                fee < budget.wrapping_sub((budget as i32 / 3) as u32)
            } else {
                price <= budget && suits(records, p, team)
            };
            if takes && best.2 < score {
                best = (id as i16, seller, score);
            }
        }
        if best.0 > 0 {
            let kind = if want.deal == LOAN { crate::transfers::LOAN_TALK } else { crate::transfers::PURCHASE };
            book(records, busy, best.0 as u16, best.1, side, want.status, kind);
            return;
        }
    }
}

/// Whether a listing is one the walk stops at (`FUN_0075905a`): the
/// position, the outlook within one, the deal; of the club's country, or
/// of another on a draw of two with the outlook above the wish.
fn listed_as_wanted(e: &[u8; 0x1c], want: &Wanted, deal: u8, country: u8, random: &mut Random) -> bool {
    let outlook = i32::from(e[8]);
    if e[6] != want.position
        || (outlook - i32::from(want.strength)).abs() >= 2
        || DEAL_OF_KIND.get(usize::from(e[0xc])) != Some(&deal)
    {
        return false;
    }
    if e[2] == country {
        return true;
    }
    random.below(2) != 0 && i32::from(want.strength) < outlook
}

/// What a listed man costs (`FUN_0070a393`): nothing with no year of his
/// contract left, or only one past day 216; else his release fee in ten
/// thousands, or his market value (`FUN_00708836`).
fn price(records: &Records, rules: &crate::season::Rules, p: &Player) -> u32 {
    let years = i32::from(p.contract() & 0xfff) - i32::from(records.globals.start_year());
    if years < 1 || (years == 1 && i32::from(records.globals.day()) > LATE) {
        return 0;
    }
    if p.fixed_fee() != 0 {
        return (u64::from(p.fixed_fee()) * RELEASE_UNIT) as u32;
    }
    let position = p.positions()[0];
    let worth = crate::value::Worth {
        strength: p.strength(),
        position,
        talent: p.talent(),
        age: crate::free_agents::age(records, p),
        good: p.good(),
        character: p.traits(),
        qualities: p.qualities(),
        star: records.globals.stars().get(usize::from(position)) == Some(&p.id()),
        kind_12_matches: p.kind_12_matches(),
    };
    rules.value.market_value(&worth, years, u32::from(records.globals.date()[1])) as u32
}

/// Whether a man's strength suits the club (`FUN_00758468`): its level
/// (`FUN_006755b8`: the main country's league, to the second — England's
/// to the third —, another country's nought), one up in the last five
/// matchdays of a played country's league (`FUN_00692923`) when it stands
/// in the last three places, one down in the first three, at most the
/// level his strength is still wanted at (`0x8abbcc`).
fn suits(records: &Records, p: &Player, team: &Team) -> bool {
    let (country, level) = (u16::from(team.country()), team.level());
    let mut lv = if i16::try_from(country).ok() == Some(records.globals.main_country()) {
        i16::from(level.min(if country == ENGLAND { 3 } else { 2 }))
    } else {
        0
    };
    if late_in_league(records, country, level) {
        let place = crate::league::place(team);
        if crate::league::teams(records, country as u8, level) - BOTTOM < place {
            lv += 1;
        } else if place < TOP && lv > 0 {
            lv -= 1;
        }
    }
    lv <= i16::from(LEVEL_BY_STRENGTH.get(usize::from(p.strength())).copied().unwrap_or(0))
}

/// Whether a played country's league is in its last five matchdays
/// (`FUN_00692923`).
fn late_in_league(records: &Records, country: u16, level: u8) -> bool {
    if level >= 6 || !records.globals.played(country as i16) {
        return false;
    }
    let index = records.countries.first_league(usize::from(country)) + i16::from(level);
    let Some(b) = records.block(&format!("{LEAGUE_AT:#x}[{index}]")) else { return false };
    let word = |at: usize| b.get(at..at + 2).map_or(0, |w| i16::from_le_bytes([w[0], w[1]]));
    word(LEAGUE_PLAYED) >= word(LEAGUE_DAYS) - LAST_MATCHDAYS
}

/// The talk booked (`FUN_00426a9c`): where the man has one, the club
/// joins it (`FUN_00762246`, no weight) and marks the talk's day; else on
/// the first day of the week from its second neither club has marked
/// (`FUN_00426972`) a new one (`FUN_0076215b`), both clubs marking it.
fn book(records: &mut Records, busy: &mut Busy, player: u16, seller: Side, buyer: Side, status: u8, kind: i16) -> bool {
    if let Some(day) = crate::transfers::talk_day(records, player).filter(|&d| d >= 0) {
        if !crate::transfers::join_bid(records, player, buyer, status, 0) {
            return false;
        }
        busy.mark(buyer, day % 7);
        return true;
    }
    let taken = busy.of(seller) | busy.of(buyer);
    let Some(d) = (1..=6).find(|&d| taken & (1 << d) == 0) else { return false };
    let day = records.globals.week_start() + d;
    if !crate::transfers::book_bid(records, kind, day, player, seller, buyer, status) {
        return false;
    }
    busy.mark(seller, d);
    busy.mark(buyer, d);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The original's week of day 63 (Office at day 62 → A) booked four
    /// seats in three talks for men on the transfer list the day before:
    /// (14,20) for 2168, (14,13) and (14,57) for the loan of 3272, (59,3)
    /// for 3262 (the rest of A's talks are for men listed that week by
    /// the clubs' other bids). The replica's week from Office on twenty
    /// draws books about as many, the original's bids of the played
    /// country among them, each on a day of the week after its first, the
    /// buyers seated with the wanted status 1 and no weight, a loan's talk
    /// for a loan listing, the man's club first.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_computer_clubs_book_talks_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (a, rules) = (load("A.an3"), crate::data::Data::load(&dir).unwrap().rules);
        let office = load("Office.an3");
        let listed = crate::transfers::listings(&office);
        let (mut seats, mut booked) = (0, std::collections::BTreeSet::new());
        for seed in 1..=20 {
            let mut r = load("Office.an3");
            r.globals.set_day(63);
            r.globals.set_date(a.globals.date());
            r.globals.set_week_start(63);
            r.globals.set_half_weeks(a.globals.half_weeks());
            crate::transfer_week::week(&mut r, &rules, &mut Random::new(seed));
            for t in crate::transfers::appointments(&r) {
                let l = listed.iter().find(|l| l.id == t.player).expect("a listed man");
                let his = office.player(t.player).unwrap().club();
                assert_eq!(t.clubs[0], (his[0], i16::from(his[1])), "{t:?}");
                assert!((64..=69).contains(&t.day), "{t:?}");
                assert_eq!(t.kind == crate::transfers::LOAN_TALK, l.kind != 1, "{t:?}");
                assert!(t.statuses[1..].iter().all(|&s| s == 1) && t.weights.iter().all(|&w| w == 0), "{t:?}");
                seats += t.clubs.len() - 1;
                booked.extend(t.clubs[1..].iter().map(|&c| (t.player, c)));
            }
        }
        assert!((50..=120).contains(&seats), "{seats} seats in twenty weeks");
        for bid in [(2168, (14, 20)), (3272, (14, 13)), (3272, (14, 57))] {
            assert!(booked.contains(&bid), "{bid:?} in {booked:?}");
        }
    }
}
