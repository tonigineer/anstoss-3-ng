//! The season's settlement (`FUN_00621909`): at the season's end, before
//! the yearly statement, the human managers first get what their season's
//! place and gates owe them (`FUN_0062292a`), then every club of the
//! countries played closes its books (`FUN_00621998`, `FUN_006219d2`). The
//! season's achievements pay the team bonuses, the main sponsor's extras
//! and, where a human manages the club, his own premium for a title or a
//! promotion (`FUN_006228c0`); the season's result is taxed and, for a
//! club gone public, a fifth of what is left goes out as dividends; the
//! result joins the last seasons' and the cash.
//!
//! Left out here, all of it for the humans or outside the club's books: the
//! trainer's premium for the cup rounds he reaches (`FUN_006cebcd`), the
//! manager's premium for a cup final won (`FUN_0042190a`, `FUN_00421bfa`,
//! `FUN_006cec7f`), the fans' mood after a title (`FUN_007144e4`), what the
//! sponsors weigh for a human club (`FUN_006d5a85`), the manager's score,
//! the season's balance row and
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
use crate::league::{self, LeagueTables};
use crate::records::{Manager, Records, Team};

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
/// The main sponsor's attendance contract: the average home gate it asks
/// the season to reach, and the bonus it pays into its money once the
/// club's own gates do (`FUN_0062292a`, dialog 246).
const ATTENDANCE: usize = 0x52c;
const ATTENDANCE_BONUS: usize = 0x528;
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
/// one first, then the others in their order. Before the books close, the
/// humans in their slots get what their club's place and gates owe them.
pub fn settle_season(records: &mut Records, tables: &LeagueTables) {
    let rate = records.globals.get::<u8>(TAX_RATE);
    let globals = &records.globals;
    let more = usize::try_from(globals.more_count()).unwrap_or(0).min(globals.more_countries().len());
    let countries: Vec<i16> =
        std::iter::once(globals.main_country()).chain(globals.more_countries()[..more].iter().copied()).collect();
    let humans = usize::from(records.globals.humans());
    for n in 0..humans {
        let Some(manager) = records.human_slot(n) else { continue };
        let (country, number) = (records.managers[manager].country(), records.managers[manager].team());
        if country == 0 {
            continue;
        }
        let Some(i) = records.team_index(country, number) else { continue };
        let class = league::class(records, tables, &records.teams[i]);
        let size = league::teams(records, records.teams[i].country(), records.teams[i].level());
        let played = usize::from(records.teams[i].home_games()).min(records.teams[i].gates().len());
        let gates: i32 = records.teams[i].gates()[..played].iter().sum();
        let (manager, team) = (&mut records.managers[manager], &mut records.teams[i]);
        reach_the_goal(manager, team, class);
        attendance_bonus(team, gates, size);
    }
    for country in countries {
        let Ok(country) = usize::try_from(country) else { continue };
        let first = usize::try_from(records.countries.first_team(country)).unwrap_or(0);
        let count = usize::try_from(records.countries.team_count(country)).unwrap_or(0);
        for i in first..first.saturating_add(count).min(records.teams.len()) {
            let manager = manager_of(records, i).map(|m| &mut records.managers[m]);
            settle(&mut records.teams[i], manager, country, rate);
        }
    }
}

/// The human manager of the club at `i`, the one the club names in its
/// [second human byte](Team::humans) — none for a club the game runs.
fn manager_of(records: &Records, i: usize) -> Option<usize> {
    let team = &records.teams[i];
    if team.humans()[1] == 0 {
        return None;
    }
    let (country, number) = (u16::from(team.country()), u16::from(team.number()));
    records.managers.iter().position(|m| m.country() == country && m.team() == number)
}

/// What reaching the class he set pays the manager (`FUN_0062292a`): his
/// premium joins his own money and the club's bonuses, but only once the
/// place reaches the class his goal asks for.
fn reach_the_goal(manager: &mut Manager, team: &mut Team, class: i32) {
    if i32::from(manager.goal_class()) <= class {
        let v = manager.goal_premium();
        manager.set_cash(manager.cash().wrapping_add(v));
        add(team, BONUSES_PAID, v);
    }
}

/// The bonus the main sponsor's attendance contract pays (`FUN_0062292a`,
/// `FUN_006210c4`): what the season's home games brought in, counted as an
/// average over the league's other teams, has to reach what the contract
/// asks for.
fn attendance_bonus(team: &mut Team, gates: i32, size: i32) {
    // A league of none asks 100000; of one the original divides by nought.
    let limit = if size < 2 { 100_000 } else { gates / (size - 1) };
    let extras = club::<u8>(team, SPONSOR) != 0 && team.get::<u8>(SPONSOR_FLAGS) & NO_EXTRAS == 0;
    if extras && club::<i32>(team, ATTENDANCE) <= limit {
        add(team, SPONSOR_MONEY, club::<i32>(team, ATTENDANCE_BONUS));
    }
}

/// What a title or a promotion pays the human running the club, on top of
/// what his players get (`FUN_006228c0`): his premium joins his own money
/// and the bonuses the club has paid. The club's second human byte names
/// him, which is why the caller hands him over only for a human club.
fn pay_premium(team: &mut Team, manager: Option<&mut Manager>) {
    let Some(manager) = manager else { return };
    let v = manager.title_premium();
    manager.set_cash(manager.cash().wrapping_add(v));
    add(team, BONUSES_PAID, v);
}

/// Settles a club's season (`FUN_006219d2`), of the country `country`,
/// at the tax rate `rate`, with the human [`Manager`] running it when it
/// has one (`Some`, and only then: the club's human byte says so).
pub fn settle(team: &mut Team, mut manager: Option<&mut Manager>, country: usize, rate: u8) {
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
        pay_premium(team, manager.as_deref_mut());
    }
    if achieved & EUROPE != 0 {
        bonus(team, EUROPE_BONUS);
        extra(team, EUROPE_EXTRA);
    }
    if achieved & PROMOTED != 0 {
        bonus(team, PROMOTED_BONUS);
        extra(team, TITLE_EXTRA);
        pay_premium(team, manager.as_deref_mut());
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
        settle(&mut t, None, 14, 29);
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
        settle(&mut t, None, 14, 29);
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
        settle(&mut t, None, ENGLAND, 0);
        let prize = 2_000_000 - 300_000;
        assert_eq!(club::<u32>(&t, PRIZE_MONEY), prize);
        assert_eq!(club::<i32>(&t, DIVIDENDS), prize as i32 / 5);
        assert_eq!(t.get::<i64>(PAST_RESULTS[0]), i64::from(prize - prize / 5));
    }

    /// A champion that also went up pays its human manager his premium
    /// twice, once for the title and once for the promotion (`FUN_006228c0`),
    /// into his own money and into the bonuses the club has paid; the
    /// premium sits on top of what the players get.
    #[test]
    fn a_human_manager_gets_a_premium_for_a_title_and_for_a_promotion() {
        let mut t = team();
        t.set_player_count(2);
        t.set_humans([0, 3]);
        t.put::<u16>(ACHIEVEMENTS, TITLE | PROMOTED);
        put::<i32>(&mut t, TITLE_BONUS, 1_000);
        put::<i32>(&mut t, PROMOTED_BONUS, 500);
        let mut m = Manager::new(&vec![0; Manager::SIZE]).unwrap();
        m.set_title_premium(45_000);
        m.set_cash(100);
        let players = 2 * 1_000 + 2 * 500;
        settle(&mut t, Some(&mut m), 14, 0);
        assert_eq!(club::<i32>(&t, BONUSES_PAID), players + 2 * 45_000);
        assert_eq!(m.cash(), 100 + 2 * 45_000);
        // The same club without a human manager keeps the players' share.
        let mut t = team();
        t.set_player_count(2);
        t.put::<u16>(ACHIEVEMENTS, TITLE | PROMOTED);
        put::<i32>(&mut t, TITLE_BONUS, 1_000);
        put::<i32>(&mut t, PROMOTED_BONUS, 500);
        settle(&mut t, None, 14, 0);
        assert_eq!(club::<i32>(&t, BONUSES_PAID), players);
    }

    /// The premium for the class goal waits for the place to reach the
    /// class the manager set (`FUN_0062292a`).
    #[test]
    fn the_goal_premium_waits_for_the_class() {
        let mut t = team();
        let mut m = Manager::new(&vec![0; Manager::SIZE]).unwrap();
        m.set_goal_class(3);
        m.set_goal_premium(22_500);
        reach_the_goal(&mut m, &mut t, 2);
        assert_eq!(club::<i32>(&t, BONUSES_PAID), 0);
        assert_eq!(m.cash(), 0);
        reach_the_goal(&mut m, &mut t, 3);
        assert_eq!(club::<i32>(&t, BONUSES_PAID), 22_500);
        assert_eq!(m.cash(), 22_500);
    }

    /// The sponsor's attendance contract pays its bonus once the season's
    /// average home gate reaches what it asks for: the gates of the home
    /// games played, shared over the league's other teams. Without a
    /// sponsor, or with the flag that stops the extras, nothing goes.
    #[test]
    fn the_attendance_bonus_needs_the_average_gate() {
        let mut t = team();
        t.set_home_games(2);
        // The season's two home gates, the rows of team +0x264.
        t.put::<i32>(0x264, 30_000);
        t.put::<i32>(0x268, 20_000);
        put::<u8>(&mut t, SPONSOR, 1);
        put::<i32>(&mut t, ATTENDANCE, 4_000);
        put::<i32>(&mut t, ATTENDANCE_BONUS, 1_150_000);
        // Eleven teams: 50000 to eleven less than them, 5000 each.
        attendance_bonus(&mut t, 50_000, 11);
        assert_eq!(club::<i32>(&t, SPONSOR_MONEY), 1_150_000);
        // A bigger league asks more than the club brings.
        attendance_bonus(&mut t, 50_000, 21);
        assert_eq!(club::<i32>(&t, SPONSOR_MONEY), 1_150_000);
        // The flag stops it, the missing sponsor too.
        let mut t = team();
        put::<u8>(&mut t, SPONSOR, 1);
        t.put::<u8>(SPONSOR_FLAGS, NO_EXTRAS);
        put::<i32>(&mut t, ATTENDANCE, 0);
        put::<i32>(&mut t, ATTENDANCE_BONUS, 1_150_000);
        attendance_bonus(&mut t, 50_000, 11);
        assert_eq!(club::<i32>(&t, SPONSOR_MONEY), 0);
    }
}
