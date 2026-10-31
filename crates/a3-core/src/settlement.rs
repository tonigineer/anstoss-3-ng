//! The season's settlement (`FUN_00621909`): at the season's end, before
//! the yearly statement, every club of the countries played closes its
//! books (`FUN_00621998`, `FUN_006219d2`). The season's achievements pay
//! the team bonuses and the main sponsor's extras, the season's result is
//! taxed and, for a club gone public, a fifth of what is left goes out as
//! dividends; the result joins the last seasons' and the cash.
//!
//! Left out here, all of it for the humans or outside the club's books: the
//! manager's own title and staying-up bonus (`FUN_006228c0`), the fans'
//! mood after a title (`FUN_007144e4`), what the sponsors weigh for a human
//! club (`FUN_006d5a85`), the manager's score, the season's balance row and
//! transfer balance in the archive (`FUN_00633539`, `FUN_006337b9`), and the
//! dividends paid into the managers' own shares (`FUN_00406fea`). The
//! season's bookings stay until the new season clears them
//! (`FUN_00673131`), which the replica has not built yet.
//!
//! Checked against the original's last day before its season end and its
//! first day after: 85 of the 212 clubs settle to the euro, 43 of them
//! taxed. Nearly all clubs that won a title, went up or down or reached
//! Europe differ — the season-end steps before the settlement, not built
//! here, move their money first.

use crate::finances::{CLUB_TO_TEAM, booked_result, result};
use crate::records::{Records, Team};

/// The tax rate in percent (`DAT_00b5dfa9`); the money events move it by
/// two at a time, within eight of its base (`DAT_00b5dfaa`).
const TAX_RATE: usize = 0xb5dfa9;
/// The season's achievements in the team record (`f_110`): bit 1 the
/// title, 4 promotion, 8 relegation, 0x10 a European place
/// (`FUN_0069a161`, `FUN_0069a1e1`, `FUN_0069a221`, `FUN_0069a0e1`).
const ACHIEVEMENTS: usize = 0x110;
const TITLE: u16 = 0x1;
const PROMOTED: u16 = 0x4;
const RELEGATED: u16 = 0x8;
const EUROPE: u16 = 0x10;
/// A public company's flag in the team record (`f_924` bit 1,
/// `FUN_0069d71d`).
const PUBLIC: usize = 0x924;
/// The club's flags copied from the team's +0x874 (`f_0fd`): with bit 8
/// the main sponsor pays no extras.
const SPONSOR_FLAGS: usize = 0x874;
const NO_EXTRAS: u8 = 0x8;
/// The club's fields (club object offsets, the team's sit
/// [`CLUB_TO_TEAM`] further on): its cash, the prize money, the team
/// bonuses paid, the main sponsor's money and the tax, the dividends paid
/// and the shares sold.
const CASH: usize = 0x210;
const PRIZE_MONEY: usize = 0x248;
const BONUSES_PAID: usize = 0x288;
const SPONSOR_MONEY: usize = 0x2f4;
const TAX: usize = 0x310;
const DIVIDENDS: usize = 0x2d4;
const SHARES_SOLD: usize = 0x2d0;
/// The main sponsor: whether there is one, the percent its money shrinks
/// by on relegation, its extras for a title or a promotion and for a
/// European place.
const SPONSOR: usize = 0x509;
const RELEGATION_CUT: usize = 0x50a;
const TITLE_EXTRA: usize = 0x514;
const EUROPE_EXTRA: usize = 0x518;
/// The team bonuses' mode (0 paid, 1 halved, others none; `FUN_0062266a`)
/// and the bonuses each player gets for the title, a European place, the
/// promotion and the last one, paid unless the club goes down.
const BONUS_MODE: usize = 0x5b8;
const TITLE_BONUS: usize = 0x5c8;
const EUROPE_BONUS: usize = 0x5cc;
const PROMOTED_BONUS: usize = 0x5d0;
const LAST_BONUS: usize = 0x5d4;
/// The prize money by place the English first division pays
/// (`FUN_0062286e`, `DAT_00a3b314`, `DAT_00a3b37c`): the club's country
/// number and class as the club object holds them.
const ENGLAND: usize = 16;
const PRIZE_CLASS: u8 = 1;
const PRIZE_BASE: u32 = 2_000_000;
const PRIZE_STEP: u32 = 100_000;
/// The last three seasons' results, newest first (club +0xe10, kept at
/// the team's +0x908).
const PAST_RESULTS: [usize; 3] = [0x908, 0x910, 0x918];
/// The most a dividend comes to.
const MOST_DIVIDEND: i64 = 2_000_000_000;

fn club<T: crate::records::Field>(team: &Team, at: usize) -> T {
    team.get(at + CLUB_TO_TEAM)
}

fn put<T: crate::records::Field>(team: &mut Team, at: usize, value: T) {
    team.put(at + CLUB_TO_TEAM, value);
}

fn add(team: &mut Team, at: usize, v: i32) {
    let old = club::<i32>(team, at);
    put(team, at, old.wrapping_add(v));
}

/// Settles the season of every club of the countries played: the main
/// one first, then the others in their order.
pub fn settle_season(records: &mut Records) {
    let rate = records.globals.get::<u8>(TAX_RATE);
    let globals = &records.globals;
    let more = usize::try_from(globals.more_count()).unwrap_or(0).min(globals.more_countries().len());
    let countries: Vec<i16> =
        std::iter::once(globals.main_country()).chain(globals.more_countries()[..more].iter().copied()).collect();
    for country in countries {
        let Ok(country) = usize::try_from(country) else { continue };
        let first = usize::try_from(records.countries.first_team(country)).unwrap_or(0);
        let count = usize::try_from(records.countries.team_count(country)).unwrap_or(0);
        for team in records.teams.iter_mut().skip(first).take(count) {
            settle(team, country, rate);
        }
    }
}

/// Settles a club's season (`FUN_006219d2`), of the country `country`,
/// at the tax rate `rate`.
pub fn settle(team: &mut Team, country: usize, rate: u8) {
    let achieved = team.get::<u16>(ACHIEVEMENTS);
    let squad = i32::from(team.player_count() as i16);
    let extras = club::<u8>(team, SPONSOR) != 0 && team.get::<u8>(SPONSOR_FLAGS) & NO_EXTRAS == 0;
    // The bonuses go to the players by the mode (`FUN_0062266a`).
    let bonus = |team: &mut Team, at: usize| {
        let v = club::<i32>(team, at).wrapping_mul(squad);
        match club::<i16>(team, BONUS_MODE) {
            0 => add(team, BONUSES_PAID, v),
            1 => add(team, BONUSES_PAID, v / 2),
            _ => {}
        }
    };
    let extra = |team: &mut Team, at: usize| {
        if extras {
            let v = club::<i32>(team, at);
            add(team, SPONSOR_MONEY, v);
        }
    };
    if achieved & TITLE != 0 {
        bonus(team, TITLE_BONUS);
        extra(team, TITLE_EXTRA);
    }
    if achieved & EUROPE != 0 {
        bonus(team, EUROPE_BONUS);
        extra(team, EUROPE_EXTRA);
    }
    if achieved & PROMOTED != 0 {
        bonus(team, PROMOTED_BONUS);
        extra(team, TITLE_EXTRA);
    }
    if achieved & RELEGATED == 0 {
        bonus(team, LAST_BONUS);
    }
    if country == ENGLAND && team.level() == PRIZE_CLASS {
        let prize = PRIZE_BASE.wrapping_sub(PRIZE_STEP.wrapping_mul(u32::from(team.table_place())));
        let old = club::<u32>(team, PRIZE_MONEY);
        put(team, PRIZE_MONEY, old.wrapping_add(prize));
    }
    // Relegation cuts the main sponsor's money by its percent, in whole
    // hundreds.
    let cut = club::<u8>(team, RELEGATION_CUT);
    if club::<u8>(team, SPONSOR) != 0 && cut != 0 && achieved & RELEGATED != 0 {
        let money = club::<u32>(team, SPONSOR_MONEY);
        put(team, SPONSOR_MONEY, (money / 100).wrapping_mul(100u32.wrapping_sub(u32::from(cut))));
    }
    // The tax on a positive result (`FUN_006218df`).
    let mut left = booked_result(team);
    if left > 0 {
        let tax = (f64::from(rate) * left as f64 / 100.0) as i32;
        let old = club::<u32>(team, TAX);
        put(team, TAX, old.wrapping_add(tax as u32));
        left -= i64::from(tax);
    }
    // A public company pays a fifth of its result, the transfers as sales
    // less purchases, as dividends (`FUN_0061fc61`).
    if team.get::<u8>(PUBLIC) & 1 != 0 {
        let earned = result(team);
        if earned > 0 {
            let dividend = (earned / 5).min(MOST_DIVIDEND);
            add(team, DIVIDENDS, dividend as i32);
            left -= dividend;
        }
    }
    let past: Vec<i64> = PAST_RESULTS.iter().map(|&at| team.get::<i64>(at)).collect();
    team.put(PAST_RESULTS[2], past[1]);
    team.put(PAST_RESULTS[1], past[0]);
    team.put(PAST_RESULTS[0], left);
    let cash = club::<i64>(team, CASH)
        .wrapping_add(left)
        .wrapping_add(i64::from(club::<i32>(team, SHARES_SOLD)));
    put(team, CASH, cash);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn team() -> Team {
        Team::new(&[0; Team::SIZE]).unwrap()
    }

    /// A champion of 20 men in mode 0 with a sponsor: the title bonus for
    /// each man and the sponsor's extra, the last bonus as it stays up, the
    /// tax at 29 % on the result, and the result joins the past ones and
    /// the cash with the shares sold.
    #[test]
    fn a_champion_is_paid_and_taxed() {
        let mut t = team();
        t.set_player_count(20);
        t.put::<u16>(ACHIEVEMENTS, TITLE);
        put::<u8>(&mut t, SPONSOR, 1);
        put::<i32>(&mut t, TITLE_BONUS, 10_000);
        put::<i32>(&mut t, LAST_BONUS, 1_000);
        put::<i32>(&mut t, TITLE_EXTRA, 500_000);
        put::<i64>(&mut t, CASH, 1_000_000);
        put::<i32>(&mut t, SHARES_SOLD, 7);
        for (k, &at) in PAST_RESULTS.iter().enumerate() {
            t.put::<i64>(at, k as i64 + 1);
        }
        let before = booked_result(&t);
        settle(&mut t, 14, 29);
        assert_eq!(club::<i32>(&t, BONUSES_PAID), 20 * 10_000 + 20 * 1_000);
        assert_eq!(club::<i32>(&t, SPONSOR_MONEY), 500_000);
        let result = before + 500_000 - 220_000;
        let tax = (29.0 * result as f64 / 100.0) as i64;
        assert!(result > 0, "{result}");
        assert_eq!(i64::from(club::<u32>(&t, TAX)), tax);
        assert_eq!(t.get::<i64>(PAST_RESULTS[0]), result - tax);
        assert_eq!((t.get::<i64>(PAST_RESULTS[1]), t.get::<i64>(PAST_RESULTS[2])), (1, 2));
        assert_eq!(club::<i64>(&t, CASH), 1_000_000 + result - tax + 7);
    }

    /// A relegated club's sponsor pays its cut, no last bonus is paid and a
    /// loss pays no tax; the halving mode halves what bonuses are left.
    #[test]
    fn relegation_cuts_the_sponsor_and_a_loss_pays_no_tax() {
        let mut t = team();
        t.set_player_count(10);
        t.put::<u16>(ACHIEVEMENTS, RELEGATED | PROMOTED);
        put::<u8>(&mut t, SPONSOR, 1);
        put::<u8>(&mut t, RELEGATION_CUT, 40);
        put::<i16>(&mut t, BONUS_MODE, 1);
        put::<i32>(&mut t, PROMOTED_BONUS, 1_001);
        put::<i32>(&mut t, LAST_BONUS, 50_000);
        put::<i32>(&mut t, SPONSOR_MONEY, 1_000_050);
        put::<i32>(&mut t, BONUSES_PAID, 9_000_000);
        settle(&mut t, 14, 29);
        assert_eq!(club::<i32>(&t, BONUSES_PAID), 9_000_000 + 10 * 1_001 / 2);
        // The extra for the promotion, then the cut in whole hundreds.
        assert_eq!(club::<u32>(&t, SPONSOR_MONEY), (1_000_050 + 0) / 100 * 60);
        assert!(t.get::<i64>(PAST_RESULTS[0]) < 0);
        assert_eq!(club::<u32>(&t, TAX), 0);
    }

    /// England's first class gets its prize money by place; a public club
    /// pays a fifth of its result after tax as dividends.
    #[test]
    fn english_prize_money_and_dividends() {
        let mut t = team();
        t.set_level(PRIZE_CLASS);
        t.set_table_place(3);
        t.put::<u16>(ACHIEVEMENTS, RELEGATED);
        t.put::<u8>(PUBLIC, 1);
        settle(&mut t, ENGLAND, 0);
        let prize = 2_000_000 - 300_000;
        assert_eq!(club::<u32>(&t, PRIZE_MONEY), prize);
        assert_eq!(club::<i32>(&t, DIVIDENDS), prize as i32 / 5);
        assert_eq!(t.get::<i64>(PAST_RESULTS[0]), i64::from(prize - prize / 5));
    }
}
