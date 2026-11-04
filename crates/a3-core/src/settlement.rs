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
//! The cup's rounds pay from here too: every tie's winner gets
//! [`cup_premium`] where the round is played (`FUN_006cebcd`, and for the
//! final `FUN_006cec7f`); the counter of cup rounds the original raises
//! beside it (`FUN_006cdb29` into the manager object's +0xa0) the replica
//! reads from the cup block instead.
//!
//! Each human club also keeps its season in the archive: the balance row,
//! the transfer net and the sponsor's money of the season that just closed
//! go into its manager's Extra block behind the past seasons
//! ([`archive`] — `FUN_00633539`, `FUN_006337b9` and `FUN_00633a30`), as
//! does the past season of gates and sales the development pages draw
//! behind their curves (`FUN_0063410d`), where the Saisonbilanzen and the
//! stadium pages read them out again.
//!
//! Left out here, all of it for the humans or outside the club's books: the
//! fans' mood after a title (`FUN_007144e4`), what the
//! sponsors weigh for a human club (`FUN_006d5a85`), the manager's score,
//! and the
//! dividends paid into the managers' own shares (`FUN_00406fea`). The
//! season's bookings stay until the new season clears them
//! ([`clear_bookings`] — `FUN_0042b2fc` per club of the countries
//! played, `FUN_00672839` in each), which the replica's new season calls
//! along with the counters and the leagues ([`open_season`]); what the
//! original sets back up beside the clearing — the ticket target
//! (`FUN_0067324c`), the class template's rows and the sponsor, TV and
//! national-team money — it has not built.
//!
//! Checked against the original's last day before its season end and its
//! first day after: with the season's end before it as the cycle runs it —
//! the flags ([`crate::promotion::flag`]), the retirements, the agreed
//! transfers ([`crate::deals`]) and the free agents
//! ([`crate::free_agents`], the youths leaving the youth among them) — 183
//! of the 212 clubs settle to the euro (86 with none of them, 109 with the
//! flags alone, 165 without the free agents, 175 without the youths). The
//! others pay their bonuses on a squad a man or two off the original's,
//! or rose in a more country by the draw: both follow the random draws
//! (the development, the free agents' leagues, the risers), which the
//! replica makes with its own generator.

use chrono::{Datelike, NaiveDate};

use crate::finances::{CLUB_TO_TEAM, booked_result, result, transfer_net};
use crate::league::{self, LeagueTables};
use crate::random::Random;
use crate::records::{Globals, Manager, Records, SeasonBalance, SeasonHistory, Team};

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
/// The sponsor's money stands in three fields: what it pays weekly, what
/// the contracts' extras add, and what the season's events left; the
/// archive's row of the season keeps all three summed
/// ([`archive`], `FUN_00633a30`).
const SPONSOR_EXTRA: usize = 0x2f8;
const SPONSOR_EVENTS: usize = 0x2fc;
const TAX: usize = 0x310;
const DIVIDENDS: usize = 0x2d4;
const SHARES_SOLD: usize = 0x2d0;
/// The season's bookings in the club's fields that the new season's
/// start clears: the two blocks of season fields, the eight dwords
/// behind them and the ticket sales plan — five rows of two bytes at
/// each of four places (`FUN_00672839`); the cash at +0x210 between the
/// blocks keeps what the season made.
const BOOKED_BLOCKS: [usize; 2] = [0x120, 0x228];
const BOOKED_LEN: usize = 0xf0;
const CLEARED_DWORDS: usize = 0x470;
const PLAN_ROWS: [usize; 4] = [0x49c, 0x4a6, 0x4b0, 0x4ba];
/// Beside them it puts +0x498 to nought, +0x492 to fifty, and +0x9ec —
/// which the team record holds at +0x374 — to nought.
const CLEARED_498: usize = 0x498;
const FIFTY_AT: usize = 0x492;
const TEAM_9EC: usize = 0x374;
/// The attendance plan the new season stands up again: the season's home
/// games, kept at the team record's +0x92e for `FUN_0069e34e` to read
/// back as the divisor of the expected crowd (`FUN_00621143`); the
/// money trouble's cap — its byte of thousands at +0x851, its flag bit
/// at +0x379 — and the plan itself at the club's +0x120 (`FUN_0042b589`).
const DIVISOR_AT: usize = 0x92e;
const NO_GATES_PLAN: i32 = 5000;
const CAP_AT: usize = 0x851;
const CAP_FLAG: usize = 0x379;
const CAPPED: u8 = 4;
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
/// The main sponsor's cup extras, both into its money: for every round the
/// club reaches, and for the final won (`FUN_006cebcd`, `FUN_006cec7f`).
const CUP_ROUND_EXTRA: usize = 0x520;
const CUP_FINAL_EXTRA: usize = 0x51c;
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
/// What the archive's row keeps the result within: the original writes the
/// row's four bytes from what overflows 0x77359401 up to two thousand
/// million and what lies below its negative back to minus two thousand
/// million (`FUN_006219d2` before `FUN_00633539`).
const MOST_RESULT: i64 = 2_000_000_000;

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
            archive(records, i);
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

/// What one cup round leaves with the club that came through it
/// (`FUN_006cebcd`, which the winner of every tie gets): the contract's
/// premium for the trainer's cup rounds joins the manager's own money and
/// the bonuses the club has paid, guarded by the club's first human byte,
/// its trainer — the original hands it to that man, and where a human runs
/// the club he is the club's manager anyway. The main sponsor pays its
/// extra for a round reached into its money. The final won brings the
/// sponsor's second extra and the premium for a title (`FUN_006cec7f`
/// with `FUN_006228c0`, the same pay a league title brings).
fn cup_round(team: &mut Team, mut manager: Option<&mut Manager>, won_the_final: bool) {
    if team.humans()[0] != 0
        && let Some(manager) = manager.as_deref_mut()
    {
        let v = manager.cup_premium();
        manager.set_cash(manager.cash().wrapping_add(v));
        add(team, BONUSES_PAID, v);
    }
    let extras = club::<u8>(team, SPONSOR) != 0 && team.get::<u8>(SPONSOR_FLAGS) & NO_EXTRAS == 0;
    if extras {
        add(team, SPONSOR_MONEY, club::<i32>(team, CUP_ROUND_EXTRA));
        if won_the_final {
            add(team, SPONSOR_MONEY, club::<i32>(team, CUP_FINAL_EXTRA));
        }
    }
    if won_the_final {
        pay_premium(team, manager.as_deref_mut());
    }
}

/// The same, looked up by club: nothing for a club the game runs and no
/// cup of its own.
pub fn cup_premium(records: &mut Records, country: u16, number: u16, won_the_final: bool) {
    let Some(i) = records.team_index(country, number) else {
        return;
    };
    let manager = manager_of(records, i).map(|m| &mut records.managers[m]);
    cup_round(&mut records.teams[i], manager, won_the_final);
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

/// What the season's end keeps of a human club's season in its manager's
/// archive: the season's balance row, the transfers' net and the sponsor's
/// money of the season that just closed (`FUN_00633539`, `FUN_006337b9`
/// and `FUN_00633a30`, the three at the tail of the club's settlement),
/// and the past season the development pages draw, which the original
/// appends in the same breath as it clears the club's gates and sales
/// (`FUN_0063410d` in `FUN_0071d0a4`). The row's place keeps no star: the
/// interim gap's bit, the club object's +0x100 bit 0x80, has no home in
/// the save.
fn archive(records: &mut Records, i: usize) {
    let team = &records.teams[i];
    if team.humans() == [0, 0] {
        return;
    }
    let slot = usize::from(team.human());
    if slot == 0 || slot > 4 {
        return;
    }
    let (year, win) = (records.globals.start_year() as i16, records.win_points(team.country()));
    let row = balance_row(team, i16::from(records.globals.main_country()), year, win);
    let (net, sponsor) = (
        transfer_net(team),
        club::<i32>(team, SPONSOR_MONEY)
            .wrapping_add(club::<i32>(team, SPONSOR_EXTRA))
            .wrapping_add(club::<i32>(team, SPONSOR_EVENTS)),
    );
    let Some(extra) = records.extras.get_mut(slot - 1) else { return };
    extra.add_balance(row);
    extra.add_transfer(year, net);
    extra.add_sponsor(year, sponsor);
    extra.add_history(history_row(team));
}

/// The row the season's end hands the development pages to keep
/// (`FUN_0063410d` called from `FUN_0071d0a4`): the gates of the home
/// games played, the pair the club carries at +0x120 and +0x124 — what it
/// plans its spectators at and the season tickets' target — and the
/// league's level, no class above the second.
fn history_row(team: &Team) -> SeasonHistory {
    let played = usize::from(team.home_games()).min(team.gates().len());
    SeasonHistory {
        gates: team.gates()[..played].iter().sum(),
        planned: team.planned(),
        tickets: team.ticket_target(),
        level: team.level().min(2),
    }
}

/// The row the archive keeps of the season just settled (`FUN_00633539`):
/// the year the season started, the league's class — the Saisonbilanzen
/// page reads it out of the club's own class, and only the main country's
/// leagues have classes (`FUN_006755b8`) —, the place in the table, the
/// points the wins at the country's win points and the draws bring, the
/// league's goals, the squad's strength as the save last stored it (club
/// +0x9c) and the season's result, kept within what the row can hold.
fn balance_row(team: &Team, main: i16, year: i16, win: i16) -> SeasonBalance {
    let class = if i16::from(team.country()) == main { team.level().min(2) } else { 0 };
    let [won, drawn, _] = team.record();
    SeasonBalance {
        year,
        league: class,
        place: team.table_place(),
        points: (i32::from(won) * i32::from(win) + i32::from(drawn)) as u8,
        goals: team.goals(),
        strength: team.squad_strength(),
        result: team.get::<i64>(PAST_RESULTS[0]).clamp(-MOST_RESULT, MOST_RESULT) as i32,
    }
}

/// What the new season's first day clears of the season's bookings:
/// every club of the countries played — the main one and the more ones,
/// in every league and the human club alike — stands with its two
/// blocks of season fields zeroed and with the first row of its ticket
/// sales plan copied down over the rest; the cash between the blocks
/// keeps what the season made (`FUN_0042b2fc` walks the countries'
/// clubs, `FUN_0042b424` reaches `FUN_0042b589`, and there
/// `FUN_00672839` clears). What the original sets back up beside the
/// clearing — the ticket sales target (`FUN_0067324c`) and the class
/// template's money — the replica has not built, so its clubs stand at
/// nought there. The cycle reaches this from [`open_season`].
pub fn clear_bookings(records: &mut Records) {
    for i in 0..records.teams.len() {
        let country = i16::from(records.teams[i].country());
        if country == 0 || !records.globals.played(country) {
            continue;
        }
        let system = usize::from(records.countries.cup_system(usize::from(records.teams[i].country())));
        clear_club(&mut records.teams[i]);
        plan_club(&mut records.teams[i], system);
    }
}

/// The plan's sums a season after the first sets up beside the clearing
/// (`FUN_00672839`), as far as they stand on the club alone: its money
/// band by league (+0x1f9: 2 the first, 1 the second, 0 lower; +0x1fa
/// nought) and what that band costs (+0x170, `DAT_00883558` thousands);
/// the first league's 200 000 at +0x12c and +0x134, 800 000 at +0x130
/// with flag 0x4000, a lower league's 100 000 at +0x12c with flag 0x8000,
/// 300 000 at +0x130 with 0x4000 and 50 000 at +0x134; +0x138 ten
/// thousands by the cup system's row and the league (`DAT_0089dd50`,
/// `DAT_008a0308`), +0x13c 600 000 more with flag 0x20, both standing in
/// the season's block as well (+0x240, +0x244); and two millions at
/// +0x5e8 and +0x5ec. (The plan's salaries, the stadium's and the
/// youth's sums, its drawn parts and the prices are not built.)
const BAND_COSTS: [i32; 3] = [12, 24, 72];
const PLAN_ROWS_START: [usize; 8] = [0, 6, 10, 12, 14, 16, 17, 18];
const PLAN_ROW: [i32; 23] = [800, 500, 90, 90, 90, 90, 600, 200, 50, 20, 400, 120, 1000, 120, 1000, 120, 200, 400, 100, 100, 100, 200, 500];
const FLAGS_AT: usize = 0x110;

fn plan_club(team: &mut Team, system: usize) {
    const UNIT: i32 = 1000;
    let level = team.level();
    let flags = team.get::<u16>(FLAGS_AT);
    let band: u8 = match level {
        0 => 2,
        1 => 1,
        _ => 0,
    };
    put::<u8>(team, 0x1f9, band);
    put::<u8>(team, 0x1fa, 0);
    put::<i32>(team, 0x170, BAND_COSTS[usize::from(band)] * UNIT);
    if level == 0 {
        put::<i32>(team, 0x134, 200 * UNIT);
        put::<i32>(team, 0x12c, 200 * UNIT);
        if flags & 0x4000 != 0 {
            put::<i32>(team, 0x130, 800 * UNIT);
        }
    } else {
        put::<i32>(team, 0x12c, if flags & 0x8000 != 0 { 100 * UNIT } else { 0 });
        if flags & 0x4000 != 0 {
            put::<i32>(team, 0x130, 300 * UNIT);
        }
        put::<i32>(team, 0x134, 50 * UNIT);
    }
    if level < 10 {
        let row = PLAN_ROWS_START.get(system).copied().unwrap_or(0) + usize::from(level);
        put::<i32>(team, 0x138, 10 * UNIT * PLAN_ROW.get(row).copied().unwrap_or(0));
    }
    if flags & 0x20 != 0 {
        let v = club::<i32>(team, 0x13c);
        put::<i32>(team, 0x13c, v + 600 * UNIT);
    }
    put::<i32>(team, 0x5e8, 2000 * UNIT);
    put::<i32>(team, 0x5ec, 2000 * UNIT);
    let (a, b) = (club::<i32>(team, 0x138), club::<i32>(team, 0x13c));
    put::<i32>(team, 0x240, a);
    put::<i32>(team, 0x244, b);
}

/// The clearing of one club (`FUN_00672839`, reached through
/// `FUN_0042b589`): the season fields to nought, the sales plan's rows
/// all its first. Beside it `FUN_0042b589` stands the attendance plan
/// back up for a human club: the gates of its home games over their
/// count, which the season's end kept at the record's +0x340 and +0x92e
/// (`FUN_006b2eed`), rounded down to whole thousands (`FUN_00621143` over
/// `FUN_0069e34e`, `FUN_0067575a`; 5000 with no home game), or the money
/// trouble's cap byte of thousands when its flag stands. The board's clubs stand at nought there: the
/// original's club object carries no gates for them at that moment, so
/// its round lands on nought as well. Measured on the pair: Union's
/// 374 204 gates in 17 home games give 22 012, and 22 000 stand in the
/// new season, the divisor with them at 17.
fn clear_club(team: &mut Team) {
    for block in BOOKED_BLOCKS {
        for k in 0..BOOKED_LEN / 4 {
            put::<u32>(team, block + k * 4, 0);
        }
    }
    for k in 0..8 {
        put::<u32>(team, CLEARED_DWORDS + k * 4, 0);
    }
    put::<u32>(team, CLEARED_498, 0);
    put::<u16>(team, FIFTY_AT, 50);
    team.put::<u32>(TEAM_9EC, 0);
    for &at in &PLAN_ROWS {
        let row = club::<i16>(team, at);
        for c in 1..5 {
            put::<i16>(team, at + c * 2, row);
        }
    }
    if team.humans() == [0, 0] {
        return;
    }
    // The closed season's gates over its home games, as its end kept them
    // (`FUN_00621143`: club +0xd84 over `FUN_0069e34e`'s +0x92e; 5000 with
    // no home game).
    let home = i32::from(team.get::<u16>(DIVISOR_AT) as i16);
    let avg = if home < 1 { NO_GATES_PLAN } else { team.get::<i32>(crate::statistics::GATES_SUM) / home };
    let plan = if team.get::<u8>(CAP_FLAG) & CAPPED != 0 {
        i32::from(team.get::<u8>(CAP_AT)) * 1000
    } else {
        avg / 1000 * 1000
    };
    put::<i32>(team, 0x120, plan);
}

/// The new season's first day: the original's season-end block hands the
/// game to its new-season block (`FUN_006da3a9`'s tail, then `FUN_0065e659`
/// and `FUN_00675c1c` and `FUN_0042b2fc`). One season more and the start
/// year one further; the day, the week's start and the half weeks stand at
/// nought; the club's season in Europe opens again (measured on the pair:
/// the stage at 0, the round at -1). The July calendar is set for the new
/// year — the first day, the date, the day of the year and the year's
/// length, 366 when the new year is a leap year (`FUN_00675e65`). A
/// season after the first draws its lottery below 1000, the first keeps
/// 1. The bank sets the year's rates and the funds' deposits settle at
/// their closing worth, the funds back at 1000 ([`crate::finances::new_rates`],
/// [`crate::finances::settle_funds`]). Then the leagues of the countries
/// played stand open again, every
/// player's season form starts from one week of 10, the kept countries'
/// contracts run on and their players develop ([`crate::rollover`]), the
/// season's lists clear with the fair-play and referees' tables seeded
/// afresh and the luck drawn anew ([`crate::statistics::open_lists`]),
/// and the season's bookings clear. Every club stands at its new league
/// ([`crate::promotion::place`]), the played countries' players open their
/// season ([`crate::rollover::open_played`]) and the national cups draw
/// their first rounds ([`crate::cup_draw`]) after the European cups open
/// ([`crate::europe_entry::open_season`]). Beside it the original
/// fills the played countries' computer squads with youth and sets their
/// line-ups (the rest of `FUN_0077655c`) and refills the stars, the best
/// players and much money — unwritten here. (`FUN_0065eb48`, which walks every squad
/// player of the countries played, calls an empty `FUN_0065ebe4`: no
/// ageing happens there.)
pub fn open_season(records: &mut Records, random: &mut Random) {
    open_season_with(records, random, None);
}

/// The new season as [`open_season`] opens it, the kept countries' clubs
/// taking new players where they are thin when the contract and value
/// tables are at hand ([`crate::newplayer::for_kept_club`]).
pub fn open_season_with(
    records: &mut Records,
    random: &mut Random,
    tables: Option<(&crate::contract::ContractTables, &crate::value::ValueTables)>,
) {
    roll_globals(&mut records.globals, random);
    clear_objects(records);
    crate::finances::new_rates(&mut records.globals, random);
    crate::finances::settle_funds(records);
    crate::opponent::open_leagues(records);
    clear_club_seasons(records);
    // Each played country's youth leagues start their season with the
    // senior ones (`FUN_00774088`'s last part).
    for country in crate::opponent::played_nations(records) {
        crate::youth_league::open_season(records, country);
    }
    crate::opponent::open_cups(records);
    crate::rollover::restart_form(records);
    crate::promotion::place(records);
    set_classes(records);
    crate::statistics::open_fielded(records);
    crate::rollover::open_played(records, tables, random);
    // The played countries' top scorers and trainers take their odds
    // (`FUN_0077655c`'s `FUN_007776c9`, `FUN_007771ae`).
    crate::season_odds::open_lists(records, random);
    crate::rollover::open_kept(records, tables, random);
    crate::statistics::open_lists(records, random);
    open_managers(records, random);
    crate::europe_entry::open_season(records, random);
    crate::cup_draw::draw(records, random);
    crate::finances::keep_last_season(records);
    clear_human_season(records);
    clear_bookings(records);
    keep_club_money(records);
}

/// The objects the new season clears whole (`FUN_0065e659`'s
/// `FUN_007cff4b` calls): the betting office's season offer
/// (`0x1c65e88`, filled again by [`crate::season_odds`]), three without a
/// reader in the replica, the eleven of the year (`0xa2e0d8`), the
/// extension talks (`0x1c53ee8`), the negotiation appointments
/// (`0x1c54000`) and `0x8eabe8`. The tables of the countries kept in
/// short start afresh, each its country's number only (`FUN_006815cf`).
const CLEARED_OBJECTS: [&str; 8] =
    ["0x1c65e88", "0x1c51940", "0x1c4be78", "0x1baa1e0", "0x8eabe8", "0xa2e0d8", "0x1c54000", "0x1c53ee8"];
const KEPT_TABLES: &str = "0xb9f8f8";
const KEPT_TABLE: usize = 0xc;
const KEPT_COUNTRIES: usize = 0x75;

fn clear_objects(records: &mut Records) {
    for name in CLEARED_OBJECTS {
        if let Some(b) = records.block_mut(name) {
            b.fill(0);
        }
    }
    if let Some(b) = records.block_mut(KEPT_TABLES) {
        b.fill(0);
        for country in 1..KEPT_COUNTRIES {
            if let Some(w) = b.get_mut(country * KEPT_TABLE..country * KEPT_TABLE + 2) {
                w.copy_from_slice(&(country as i16).to_le_bytes());
            }
        }
    }
}

/// A manager's record: a standing of his own (the manager object's +0x30),
/// his marks' sum and count (+0x34, +0x36), the game's mode (+0x40).
const OWN_STANDING: usize = 0x11c;
const MARKS_SUM: usize = 0x120;
const MARKS_COUNT: usize = 0x122;
const MODE_WITH_STANDING: i16 = 9;
const STANDING_MOST: i16 = 70;

/// The season's end for the human managers (`FUN_00624e4f`'s
/// `FUN_006fd282` for each): a year more as a trainer; in the game's mode
/// 9, his standing by his marks' average over the season — down three
/// below 60, two below 70, one below 80, one even chance in two below 85,
/// up one even chance in two from 98 while it stands below 70. (The rest
/// of `FUN_00624e4f` — the contracts' years, the comparison's marks, the
/// computer clubs' staff — is not built.)
pub fn close_managers(records: &mut Records, random: &mut Random) {
    for n in 0..usize::from(records.globals.humans()) {
        let Some(m) = records.managers.get_mut(n) else { continue };
        m.set_trainer_years(m.trainer_years().wrapping_add(1));
        let count = m.get::<i16>(MARKS_COUNT);
        if m.mode() != MODE_WITH_STANDING || count == 0 {
            continue;
        }
        let average = i32::from(m.get::<i16>(MARKS_SUM)) * 100 / i32::from(count);
        let standing = m.get::<i16>(OWN_STANDING);
        let by = if average < 9801 {
            match average {
                ..6000 => -3,
                6000..7000 => -2,
                7000..8000 => -1,
                8000..8500 if random.below(2) != 0 => -1,
                _ => 0,
            }
        } else if standing < STANDING_MOST && random.below(2) != 0 {
            1
        } else {
            0
        };
        m.put::<i16>(OWN_STANDING, standing.wrapping_add(by));
    }
}

/// A manager's nerves (the object's +0x32), the board's confidence and
/// its bonus (+0x9c, +0xa1), two counts the career caps (+0x9e at 18,
/// +0x9f at 9), the season's flags and fields the new season clears, his
/// club's cash as it opens (+0x114), and fifty entries of 0x3a from +0x86c
/// (the object's +0x720) with a flag, a word and a count kept below 100.
const NERVES: usize = 0x11e;
const NERVES_LOW: i16 = 20;
const NERVES_LIFT: i16 = 80;
const CONFIDENCE_BONUS: usize = 0x189;
const CAREER_CAPS: [(usize, u8); 2] = [(0x186, 18), (0x187, 9)];
const CLUB_CASH: usize = 0x2ec;
const ENTRIES_AT: usize = 0x86c;
const ENTRY: usize = 0x3a;
const ENTRIES: usize = 50;

/// The new season opens the human managers' year (`FUN_00775219`): his
/// club's cash noted, the board's confidence moved by its bonus within 0
/// and 100 and then lifted by ten below 70 or five below 90, the career's
/// two counts capped, his nerves eighty up when at 20 or below and else
/// back at a hundred less a draw below five, and the season's marks,
/// flags and his entries' flags cleared. (The career's counts by the
/// titles of the last seasons, the training plan's reset and the rest of
/// `FUN_00775219` are not built.)
pub fn open_managers(records: &mut Records, random: &mut Random) {
    for n in 0..usize::from(records.globals.humans()) {
        let Some(m) = records.managers.get(n) else { continue };
        let (country, number) = (m.country(), m.team());
        let cash = u16::try_from(number)
            .ok()
            .filter(|_| number > 0 || country > 0)
            .and_then(|t| records.team(country as u16, t))
            .map(|t| club::<i64>(t, CASH));
        let Some(m) = records.managers.get_mut(n) else { continue };
        if let Some(cash) = cash {
            m.put::<i32>(CLUB_CASH, cash.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32);
        }
        let bonus = i16::from(m.get::<i8>(CONFIDENCE_BONUS));
        let mut confidence = (m.confidence().wrapping_add(bonus)).clamp(0, 100);
        if confidence < 70 {
            confidence += 10;
        } else if confidence < 90 {
            confidence += 5;
        }
        m.set_confidence(confidence);
        for (at, most) in CAREER_CAPS {
            if m.get::<u8>(at) > most {
                m.put::<u8>(at, most);
            }
        }
        let nerves = m.get::<i16>(NERVES);
        let nerves = if nerves <= NERVES_LOW { nerves + NERVES_LIFT } else { 100 - random.below(5) as i16 };
        m.put::<i16>(NERVES, nerves);
        m.put::<u8>(0x1f18, 0);
        m.put::<u32>(0x1f1c, m.get::<u32>(0x1f1c) & 0xffff_f1ca);
        m.put::<u16>(0x1f0, 0);
        m.put::<i16>(MARKS_COUNT, 0);
        m.put::<i16>(MARKS_SUM, 0);
        m.put::<u8>(0x19a, 0);
        m.put::<u32>(0x1a8, 0);
        for k in 0..ENTRIES {
            let at = ENTRIES_AT + k * ENTRY;
            m.put::<u8>(at, 0);
            m.put::<u16>(at + 6, 0);
            if m.get::<i16>(at + 4) > 99 {
                m.put::<i16>(at + 4, 0);
            }
        }
    }
}

/// The human manager's copy of his club's money, as the new season opens
/// it (`FUN_00775219`'s `FUN_0078605e`: the club object's thousand bytes
/// from +0x210 into the manager's +0x1370, record +0x14c0) and as every
/// week ends, before the advance sales (`FUN_0064e40f`).
const MANAGER_MONEY: usize = 0x14c0;
const MONEY_LEN: usize = 1000;

pub(crate) fn keep_club_money(records: &mut Records) {
    for n in 0..usize::from(records.globals.humans()) {
        let Some(m) = records.managers.get(n) else { continue };
        let (country, number) = (m.country(), m.team());
        if country == 0 {
            continue;
        }
        let Some(team) = records.team(country, number) else { continue };
        let money: Vec<u8> = (0..MONEY_LEN).map(|k| club::<u8>(team, CASH + k)).collect();
        let Some(m) = records.managers.get_mut(n) else { continue };
        for (k, b) in money.into_iter().enumerate() {
            m.put::<u8>(MANAGER_MONEY + k, b);
        }
    }
}

/// What the new season puts to nought of a human club's extra data, as
/// the original's first day after the turn stands: last week's and the
/// season's training units. (Its training plan stands at nought there
/// too, filled again before it trains; the replica keeps the plan.)
const HUMAN_SEASON_COUNTERS: [(usize, usize); 2] = [(0x315, 39), (0x33c, 78)];

/// What every club's new season clears (`FUN_00772bee`'s `FUN_0063f5d3`,
/// with `FUN_00775219`'s fitness count of a human club):
/// team +0x312 to 0xff, the 0x18 bytes from +0x2ca; of a human club's
/// extra data the away gates, the opposing fans' gates and the home
/// incomes by game, the goals by the minute both ways (two hundred bytes
/// each), and two tables of 0x2e. (Two more it clears at +0x8b8 and
/// +0x8e6 the open club object holds as +0xd8e and +0xdbc, and writes back
/// as they were when the club closes.)
const CLUB_SEASON_MARK: usize = 0x312;
const CLUB_SEASON_CLEARED: (usize, usize) = (0x2ca, 0x18);
const HUMAN_SEASON_CLEARED: [(usize, usize); 7] = [
    (0x398, 0x5c),
    (0x3f4, 0x5c),
    (0x450, 0x5c),
    (0x914, 200),
    (0x9dc, 200),
    (0xaa4, 0x2e),
    (0xad2, 0x2e),
];

/// The club's own season fields `FUN_00772bee` puts to nought, as the
/// team record holds them (club object ← record): +0xd24/+0xd25 (+0x260,
/// +0x261), the 0x5c bytes of +0xd28 (+0x264), +0xd8c (+0x2c8), +0x112
/// (+0x13b), +0xb4 to +0xb6 (+0xd4), +0xca/+0xcb (+0xea), +0xd8 (+0xfc,
/// a word), +0xda to +0xdd (+0xf8), +0xde (+0x10c), +0xf7 (+0x125), +0x8c
/// and +0x8e (+0xae, +0xb0, words), +0xdf1 (+0x303), +0xdea/+0xdeb
/// (+0x332) and the words +0xdec/+0xdee (+0x334, +0x336).
const CLUB_SEASON_ZEROS: [(usize, usize); 15] = [
    (0x260, 2),
    (0x264, 0x5c),
    (0x2c8, 2),
    (0x13b, 1),
    (0xd4, 3),
    (0xea, 2),
    (0xfc, 2),
    (0xf8, 4),
    (0x10c, 1),
    (0x125, 1),
    (0xae, 2),
    (0xb0, 2),
    (0x303, 1),
    (0x332, 2),
    (0x334, 4),
];
/// Its flag word +0xdf8 (+0x308) keeps only these bits, its +0x100
/// (+0x12a) the two above its lowest, its +0x102 (+0x12c) its half's
/// 0xaf.
const CLUB_SEASON_FLAGS: (usize, u32) =
    (0x308, 0xffffdfeb & 0xfffff4ff & 0xffffbf1f & 0xfffc7fff & 0xffe3ffff & 0xffdfffff);
/// From the second season on its +0x124 keeps its half's top nibble
/// (`FUN_0069d528`).
const SECOND_SEASON_BYTE: usize = 0x124;

const CLUB_SEASON_SHIFTS: [(usize, u8, u8); 2] = [(0x12a, 2, 3), (0x12c, 1, 0xaf)];
/// What its `FUN_00699524` clears on the team record itself: +0x8e8,
/// +0x8e9, +0x8ec, +0x8ed, the four bytes of +0x37c, the eight of +0x380,
/// the ten of +0x388 (the players to keep, FUN_0069d7f5), the sixteen of
/// +0x930, the eight of +0x940, and +0x338 to +0x33c; +0x8ea and +0x8eb
/// it halves.
const TEAM_SEASON_ZEROS: [(usize, usize); 8] = [
    (0x8e8, 2),
    (0x8ec, 2),
    (0x37c, 4),
    (0x380, 8),
    (0x388, 10),
    (0x930, 0x10),
    (0x940, 8),
    (0x338, 5),
];
const TEAM_SEASON_HALVED: [usize; 2] = [0x8ea, 0x8eb];
/// A human club's +0xb0 and +0xff live in its extra data (+0x394, the
/// special bonus, and +0x8a4).
const HUMAN_CLUB_ZEROS: [(usize, usize); 2] = [(0x394, 4), (0x8a4, 1)];

/// Each played club's class for the new season (`FUN_00772bee`, club
/// +0x113, team +0x13c), by its league and its place in the new list: a
/// club of the main country 6, 5 or 4 in the first league (the first six
/// places, the next six, the rest), 3 or 2 in the second (the first
/// six), England's third league 1 and lower 0, any other main country's
/// lower leagues 1 for the first six places and 0; a club of a more
/// country 9, 8 or 7 by its place; one in no league 0, its place too and
/// +0x134 at five.
pub(crate) const CLASS_AT: usize = 0x13c;
const NO_LEAGUE_FORECAST: usize = 0x134;

pub(crate) fn set_classes(records: &mut Records) {
    let main = records.globals.main_country();
    for country in crate::opponent::played_nations(records) {
        for number in 0..records.countries.team_count(usize::from(country)).max(0) as u16 {
            let Some(team) = records.team_mut(country, number) else { continue };
            let (level, place) = (team.level(), team.league_place());
            let by_place = |first: u8, second: u8, rest: u8| if place < 6 { first } else if place < 12 { second } else { rest };
            let class = if country as i16 == main {
                match level {
                    0 => by_place(6, 5, 4),
                    1 => by_place(3, 2, 2),
                    _ if country == 16 => u8::from(level == 2),
                    _ => by_place(1, 0, 0),
                }
            } else {
                by_place(9, 8, 7)
            };
            team.put::<u8>(CLASS_AT, class);
            if level == 10 {
                team.put::<u8>(NO_LEAGUE_FORECAST, 5);
                team.set_league_place(0);
                team.put::<u8>(CLASS_AT, 0);
            }
        }
    }
}

fn clear_club_seasons(records: &mut Records) {
    let second_season = records.globals.seasons() > 1;
    for country in crate::opponent::played_nations(records) {
        for number in 0..records.countries.team_count(usize::from(country)).max(0) as u16 {
            let Some(team) = records.team_mut(country, number) else { continue };
            for (at, len) in CLUB_SEASON_ZEROS {
                for k in 0..len {
                    team.put::<u8>(at + k, 0);
                }
            }
            for (at, len) in TEAM_SEASON_ZEROS {
                for k in 0..len {
                    team.put::<u8>(at + k, 0);
                }
            }
            for at in TEAM_SEASON_HALVED {
                team.put::<u8>(at, team.get::<u8>(at) >> 1);
            }
            if second_season {
                team.put::<u8>(SECOND_SEASON_BYTE, (team.get::<u8>(SECOND_SEASON_BYTE) >> 1) & 0xf0);
            }
            let (at, keep) = CLUB_SEASON_FLAGS;
            team.put::<u32>(at, team.get::<u32>(at) & keep);
            for (at, shift, mask) in CLUB_SEASON_SHIFTS {
                team.put::<u8>(at, (team.get::<u8>(at) >> shift) & mask);
            }
            team.put::<u8>(CLUB_SEASON_MARK, 0xff);
            let (at, len) = CLUB_SEASON_CLEARED;
            for k in 0..len {
                team.put::<u8>(at + k, 0);
            }
            let human = usize::from(team.human());
            if !(1..=4).contains(&human) {
                continue;
            }
            let Some(extra) = records.extras.get_mut(human - 1) else { continue };
            for (at, len) in HUMAN_SEASON_CLEARED {
                for k in 0..len {
                    extra.put::<u8>(at + k, 0);
                }
            }
            for (at, len) in HUMAN_CLUB_ZEROS {
                for k in 0..len {
                    extra.put::<u8>(at + k, 0);
                }
            }
            // And its together count (`FUN_0069d684`).
            extra.set_together(0);
            // The fitness trainer's count starts anew (`FUN_00775219`'s
            // `FUN_00632ac1`).
            extra.set_fitness_count(0);
        }
    }
}

fn clear_human_season(records: &mut Records) {
    for n in 0..usize::from(records.globals.humans()) {
        let Some(slot) = records.human_slot(n) else { continue };
        let Some(extra) = records.extras.get_mut(slot) else { continue };
        for (at, len) in HUMAN_SEASON_COUNTERS {
            for k in 0..len {
                extra.put::<u8>(at + k, 0);
            }
        }
    }
}

/// The counters and the July calendar of the new season, as the original
/// writes them (`FUN_0065e659` and `FUN_00675c1c`).
fn roll_globals(globals: &mut Globals, random: &mut Random) {
    globals.set_seasons(globals.seasons() + 1);
    let (year, y) = (globals.start_year() + 1, i32::from(globals.start_year()) + 1);
    globals.set_start_year(year);
    globals.set_day(0);
    globals.set_week_start(0);
    globals.set_half_weeks(0);
    globals.set_europe_stage(0);
    globals.set_uefa_round(-1);
    globals.set_lottery(if globals.seasons() == 1 { 1 } else { random.below(1000) as u16 });
    let start = crate::calendar::season_start(y);
    let july = NaiveDate::from_ymd_opt(y, 7, 1).expect("a day early in July");
    let first_day = start.signed_duration_since(july).num_days() as i16;
    globals.set_first_day(first_day);
    globals.set_date([first_day as u16 + 1, 7, u16::try_from(y).unwrap_or(0)]);
    globals.set_day_of_year(start.ordinal0() as u16);
    let leap = y % 4 == 0 && (y % 400 == 0 || y % 100 != 0);
    globals.set_year_days(if leap { 366 } else { 365 });
    // Ten bytes at 0xb5dfb0 back to 0xff, the word at 0xb5dfc4 to nought
    // (`FUN_0065e659`), and 0xb5d1a4 to one (`FUN_0077655c`, for every
    // played country).
    for k in 0..10 {
        globals.put::<u8>(0xb5dfb0 + k, 0xff);
    }
    globals.put::<u16>(0xb5dfc4, 0);
    globals.put::<u16>(0xb5d1a4, 1);
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
mod club_season_tests {
    use super::*;

    /// The human manager's copy of his club's money: on the original's
    /// first day of the new season (NewSeason000) it stands byte for byte
    /// as the club's money block, and the replica's copy over the closed
    /// season's save does the same.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_manager_keeps_his_clubs_money_as_the_original() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let same = |r: &Records| {
            let m = &r.managers[0];
            let team = r.team(m.country(), m.team()).unwrap();
            (0..MONEY_LEN).all(|k| m.get::<u8>(MANAGER_MONEY + k) == club::<u8>(team, CASH + k))
        };
        assert!(same(&load("NewSeason000.an3")));
        let mut ours = load("SeasonEnd364.an3");
        assert!(!same(&ours));
        keep_club_money(&mut ours);
        assert!(same(&ours));
    }

    /// Over the original's turn (SeasonEnd364 → NewSeason000) the clears
    /// of every played club's new season leave its team bytes as the
    /// original's — the season's fields put to nought, the flags kept, the
    /// shifted bytes, +0x2ca to +0x2e2 and +0x312 —, and the human club's
    /// extra data where they reach: the gates and incomes by game, the
    /// goals by the minute, the two tables and the fitness count.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_clubs_clear_their_season_as_the_original() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs) = (load("SeasonEnd364.an3"), load("NewSeason000.an3"));
        ours.globals.set_seasons(2);
        clear_club_seasons(&mut ours);
        let mut clubs = 0;
        for country in crate::opponent::played_nations(&theirs) {
            for number in 0..theirs.countries.team_count(usize::from(country)).max(0) as u16 {
                let (x, y) = (ours.team(country, number).unwrap(), theirs.team(country, number).unwrap());
                assert_eq!(x.get::<u8>(CLUB_SEASON_MARK), y.get::<u8>(CLUB_SEASON_MARK), "{country}/{number}");
                // +0x125 takes the pre-season tournaments' marks after
                // (`FUN_007719a6`), and a human club's flags a bit of its
                // own: neither built.
                let (at, len) = CLUB_SEASON_CLEARED;
                let flags = if y.human() == 0 { 4 } else { 0 };
                let ranges = CLUB_SEASON_ZEROS.into_iter().filter(|&(at, _)| at != 0x125);
                let ranges = ranges.chain([(at, len), (CLUB_SEASON_FLAGS.0, flags)]);
                let ranges = ranges.chain(CLUB_SEASON_SHIFTS.map(|(at, _, _)| (at, 1)));
                let ranges = ranges.chain(TEAM_SEASON_ZEROS).chain(TEAM_SEASON_HALVED.map(|at| (at, 1)));
                for (at, len) in ranges {
                    for k in at..at + len {
                        assert_eq!(x.get::<u8>(k), y.get::<u8>(k), "{country}/{number} +{k:#x}");
                    }
                }
                assert_eq!(x.get::<u8>(SECOND_SEASON_BYTE), y.get::<u8>(SECOND_SEASON_BYTE), "{country}/{number}");
                clubs += 1;
            }
        }
        assert!(clubs > 200);
        let (x, y) = (&ours.extras[0], &theirs.extras[0]);
        for (at, len) in HUMAN_SEASON_CLEARED.into_iter().chain(HUMAN_CLUB_ZEROS).chain([(0xb90, 2), (0x2564, 2)]) {
            for k in at..at + len {
                assert_eq!(x.get::<u8>(k), y.get::<u8>(k), "extra +{k:#x}");
            }
        }
    }
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

    /// A cup round pays the contract's cup premium into the manager's own
    /// money and the club's bonuses, and the main sponsor its extra for a
    /// round reached (`FUN_006cebcd`); the final won adds the premium for a
    /// title and the sponsor's second extra (`FUN_006cec7f`). The club's
    /// trainer byte guards the premium, the sponsor's flag the extras, and
    /// a club the game runs gets neither.
    #[test]
    fn a_cup_round_pays_its_premium_and_the_sponsor_extra() {
        let mut t = team();
        t.set_humans([3, 3]);
        put::<u8>(&mut t, SPONSOR, 1);
        put::<i32>(&mut t, CUP_ROUND_EXTRA, 95_000);
        put::<i32>(&mut t, CUP_FINAL_EXTRA, 560_000);
        let mut m = Manager::new(&vec![0; Manager::SIZE]).unwrap();
        m.set_cup_premium(6_000);
        m.set_title_premium(45_000);
        m.set_cash(100);
        cup_round(&mut t, Some(&mut m), false);
        assert_eq!(club::<i32>(&t, BONUSES_PAID), 6_000);
        assert_eq!(club::<i32>(&t, SPONSOR_MONEY), 95_000);
        assert_eq!(m.cash(), 6_100);
        // The final won pays both on top: the premium for a title and the
        // second extra, the round's own share a second time as well.
        cup_round(&mut t, Some(&mut m), true);
        assert_eq!(club::<i32>(&t, BONUSES_PAID), 2 * 6_000 + 45_000);
        assert_eq!(club::<i32>(&t, SPONSOR_MONEY), 2 * 95_000 + 560_000);
        assert_eq!(m.cash(), 100 + 2 * 6_000 + 45_000);
        // A club with a manager and no trainer keeps the premium back but
        // still gets the title's premium; the sponsor's flag stops extras.
        let mut t = team();
        t.set_humans([0, 3]);
        put::<u8>(&mut t, SPONSOR, 1);
        t.put::<u8>(SPONSOR_FLAGS, NO_EXTRAS);
        put::<i32>(&mut t, CUP_ROUND_EXTRA, 95_000);
        let mut m = Manager::new(&vec![0; Manager::SIZE]).unwrap();
        m.set_cup_premium(6_000);
        m.set_title_premium(45_000);
        cup_round(&mut t, Some(&mut m), true);
        assert_eq!(club::<i32>(&t, BONUSES_PAID), 45_000);
        assert_eq!(club::<i32>(&t, SPONSOR_MONEY), 0);
        assert_eq!(m.cash(), 45_000);
        // A club the game runs has no human to premium, but its sponsor's
        // extras still go.
        let mut t = team();
        put::<u8>(&mut t, SPONSOR, 1);
        put::<i32>(&mut t, CUP_ROUND_EXTRA, 95_000);
        put::<i32>(&mut t, CUP_FINAL_EXTRA, 560_000);
        cup_round(&mut t, None, true);
        assert_eq!(club::<i32>(&t, BONUSES_PAID), 0);
        assert_eq!(club::<i32>(&t, SPONSOR_MONEY), 95_000 + 560_000);
    }

    /// The row the archive keeps of a season, measured on the original:
    /// 1.FC Union Berlin's 2025 season, its last day's save
    /// (SeasonEnd364) against the row the first day after keeps
    /// (NewSeason000) — third of the first class, 53 points of fifteen
    /// wins and eight draws at three apiece, 41 to 33 goals, a squad of
    /// 216 and a result of 443.568. What no four bytes hold the original
    /// ends at two thousand million, both ways; a season never settled
    /// stands at its sentinel.
    #[test]
    fn the_archive_keeps_the_season_measured() {
        let mut t = team();
        t.set_country(14);
        t.set_table_place(3);
        t.set_record([15, 8, 11]);
        t.set_goals([41, 33]);
        t.set_squad_strength(216);
        t.put(PAST_RESULTS[0], 443_568i64);
        assert_eq!(
            balance_row(&t, 14, 2025, 3),
            SeasonBalance {
                year: 2025,
                league: 0,
                place: 3,
                points: 53,
                goals: [41, 33],
                strength: 216,
                result: 443_568,
            }
        );
        t.put(PAST_RESULTS[0], 2_100_000_000i64);
        assert_eq!(balance_row(&t, 14, 2025, 3).result, 2_000_000_000);
        t.put(PAST_RESULTS[0], -2_100_000_000i64);
        assert_eq!(balance_row(&t, 14, 2025, 3).result, -2_000_000_000);
        t.put(PAST_RESULTS[0], -1i64);
        assert_eq!(balance_row(&t, 14, 2025, 3).result, -1);
        // The class the row keeps is the league's own, and only the main
        // country's leagues have one.
        t.set_level(2);
        assert_eq!(balance_row(&t, 14, 2025, 3).league, 2);
        t.set_level(4);
        assert_eq!(balance_row(&t, 14, 2025, 3).league, 2);
        t.set_country(21);
        assert_eq!(balance_row(&t, 14, 2025, 3).league, 0);
    }

    /// One club stands with both blocks of season fields at nought, the
    /// eight dwords behind them gone and the sales plan's rows all its
    /// first; the cash between the blocks and the fields at +0x490 keep
    /// their season's values.
    #[test]
    fn a_cleared_club_keeps_its_cash_and_copies_its_first_plan_row() {
        let mut t = team();
        put::<i64>(&mut t, CASH, 4_430_000);
        put::<i32>(&mut t, PRIZE_MONEY, 1_700_000);
        put::<i32>(&mut t, BONUSES_PAID, 3_536_800);
        put::<i32>(&mut t, 0x120, 12_000);
        put::<i32>(&mut t, 0x124, 8_900);
        put::<u32>(&mut t, CLEARED_DWORDS, 909);
        put::<u32>(&mut t, 0x48c, 707);
        t.put::<u8>(0x490 + CLUB_TO_TEAM, 20);
        t.put::<u8>(0x494 + CLUB_TO_TEAM, 3);
        for c in 0..5 {
            put::<i16>(&mut t, PLAN_ROWS[0] + c * 2, 15 + c as i16);
        }
        clear_club(&mut t);
        assert_eq!(club::<i64>(&t, CASH), 4_430_000);
        assert_eq!(club::<i32>(&t, PRIZE_MONEY), 0);
        assert_eq!(club::<i32>(&t, BONUSES_PAID), 0);
        assert_eq!((club::<i32>(&t, 0x120), club::<i32>(&t, 0x124)), (0, 0));
        assert_eq!((club::<u32>(&t, CLEARED_DWORDS), club::<u32>(&t, 0x48c)), (0, 0));
        assert_eq!(
            (t.get::<u8>(0x490 + CLUB_TO_TEAM), t.get::<u8>(0x494 + CLUB_TO_TEAM)),
            (20, 3)
        );
        for c in 0..5 {
            assert_eq!(club::<i16>(&t, PLAN_ROWS[0] + c * 2), 15, "row {c}");
        }
    }

    /// The new season clears the bookings of the countries played —
    /// measured on the original: its last day before the season end
    /// (SeasonEnd364) against its first day after (NewSeason000). Of
    /// the main country and the more ones stand the prize money and the
    /// team bonuses at nought for every club of every league, the human
    /// club among them; the dwords behind the blocks, +0x498, +0x492 at
    /// fifty, +0x9ec, the sales plan's rows and the plan's band, its cost
    /// and the sums standing on the club alone agree as well, while the clubs of all other countries
    /// keep their season's fields. The attendance plan agrees too: the
    /// home games of the season at +0x92e for every club, and the human
    /// club's plan computed back (22 000 at Union, 17 home games of
    /// 374 204 gates); the ticket target and the class template's money
    /// the replica leaves at nought.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_new_season_clears_the_bookings_of_the_countries_played() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let read = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut closed, opened) = (read("SeasonEnd364.an3"), read("NewSeason000.an3"));
        let played: Vec<usize> = closed
            .teams
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                t.country() != 0 && closed.globals.played(i16::from(t.country()))
            })
            .map(|(i, _)| i)
            .collect();
        assert_eq!(played.len(), 212, "the four countries' clubs");
        let cash: Vec<i64> = played.iter().map(|&i| club::<i64>(&closed.teams[i], CASH)).collect();
        // The season's end keeps the gates and home games first; the new
        // season clears the clubs' own fields — the gates among them —
        // before the bookings, as the opening runs them.
        crate::statistics::close_tables(&mut closed);
        clear_club_seasons(&mut closed);
        // The new season's leagues and the season's flags as the
        // original's turn set them.
        for &i in &played {
            let (level, flags) = (opened.teams[i].level(), opened.teams[i].get::<u16>(FLAGS_AT));
            closed.teams[i].set_level(level);
            closed.teams[i].put::<u16>(FLAGS_AT, flags);
        }
        clear_bookings(&mut closed);
        for (k, &i) in played.iter().enumerate() {
            let (a, b) = (&closed.teams[i], &opened.teams[i]);
            assert_eq!(club::<i64>(a, CASH), cash[k], "team {i} keeps its cash");
            for at in [0x248, 0x288] {
                assert_eq!(
                    (club::<i32>(a, at), club::<i32>(b, at)),
                    (0, 0),
                    "team {i} +{at:#x} stands cleared"
                );
            }
            if a.humans() == [0, 0] {
                assert_eq!(
                    (club::<i32>(a, 0x120), club::<i32>(b, 0x120)),
                    (0, 0),
                    "team {i} +0x120 stands cleared"
                );
            } else {
                // The human club's plan the original computes back for
                // the new season (22 000 at Union): the replica's agrees.
                assert_eq!(club::<i32>(a, 0x120), club::<i32>(b, 0x120), "team {i} +0x120");
            }
            assert_eq!(
                a.get::<u16>(DIVISOR_AT),
                b.get::<u16>(DIVISOR_AT),
                "team {i} +0x92e the home games"
            );
            for k in 0..8 {
                let at = CLEARED_DWORDS + k * 4;
                assert_eq!(club::<u32>(a, at), club::<u32>(b, at), "team {i} +{at:#x}");
            }
            assert_eq!(club::<u32>(a, CLEARED_498), club::<u32>(b, CLEARED_498), "team {i} +0x498");
            assert_eq!(club::<u16>(a, FIFTY_AT), club::<u16>(b, FIFTY_AT), "team {i} +0x492");
            assert_eq!(a.get::<u32>(TEAM_9EC), b.get::<u32>(TEAM_9EC), "team {i} +0x9ec");
            for at in PLAN_ROWS {
                for c in 0..5 {
                    let at = at + c * 2;
                    assert_eq!(club::<i16>(a, at), club::<i16>(b, at), "team {i} +{at:#x}");
                }
            }
            // The plan's band, its cost and the sums standing on the club.
            assert_eq!(club::<u16>(a, 0x1f9), club::<u16>(b, 0x1f9), "team {i} +0x1f9");
            for at in [0x170, 0x138, 0x13c, 0x240, 0x244] {
                assert_eq!(club::<i32>(a, at), club::<i32>(b, at), "team {i} +{at:#x}");
            }
        }
        for i in 0..closed.teams.len() {
            if closed.teams[i].country() == 0 || played.contains(&i) {
                continue;
            }
            for block in BOOKED_BLOCKS {
                for k in 0..BOOKED_LEN / 4 {
                    let at = block + k * 4;
                    // One field the season-end chain writes for every
                    // club, even those of the countries not played —
                    // its money the replica has not built.
                    if at == 0x258 {
                        continue;
                    }
                    assert_eq!(
                        club::<u32>(&closed.teams[i], at),
                        club::<u32>(&opened.teams[i], at),
                        "team {i} of country {} keeps +{at:#x}",
                        closed.teams[i].country()
                    );
                }
            }
        }
    }

    /// The counters and the calendar of the new season: one season more,
    /// the start year on, the day's fields at nought, the leagues' pair
    /// of Europe as the league init leaves it, and the new season's own
    /// first day of July, day of the year and length — the leap year
    /// counted on the start year itself (`FUN_0065e659`, `FUN_00675c1c`,
    /// `FUN_00675e65`); the lottery stands at 1 for the very first
    /// season and is drawn below 1000 for every one after.
    #[test]
    fn the_new_season_rolls_the_counters_and_the_calendar() {
        let mut g = Globals::new(&vec![0; Globals::SIZE]).unwrap();
        g.set_seasons(0);
        g.set_start_year(1997);
        roll_globals(&mut g, &mut Random::new(7));
        assert_eq!((g.seasons(), g.lottery()), (1, 1), "the first season keeps its 1");
        for (seasons, start, first_day, date, day_of_year, year_days) in [
            (1u16, 2025u16, 5i16, [6u16, 7, 2026], 186u16, 365u16),
            (2, 2027, 1, [2, 7, 2028], 183, 366),
        ] {
            g.set_seasons(seasons);
            g.set_start_year(start);
            g.set_day(364);
            g.set_week_start(3);
            g.set_half_weeks(2);
            g.set_europe_stage(4);
            g.set_uefa_round(7);
            roll_globals(&mut g, &mut Random::new(7));
            assert_eq!((g.seasons(), g.start_year()), (seasons + 1, start + 1));
            assert_eq!((g.day(), g.week_start(), g.half_weeks()), (0, 0, 0));
            assert_eq!(
                (g.first_day(), g.date(), g.day_of_year(), g.year_days()),
                (first_day, date, day_of_year, year_days),
                "the season of {}",
                start + 1
            );
            assert_eq!((g.europe_stage(), g.uefa_round()), (0, -1));
            assert!(g.lottery() < 1000, "a lottery drawn for the season after");
        }
    }

    /// The human club's season counters over the original's pair: SeasonEnd364
    /// cleared by the replica stands as NewSeason000 keeps them, all at
    /// nought — among them the season's training units, which would
    /// otherwise count on season after season.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_new_season_clears_the_human_clubs_counters_as_the_original() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let read = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs) = (read("SeasonEnd364.an3"), read("NewSeason000.an3"));
        let slot = ours.human_slot(0).unwrap();
        assert!(ours.extras[slot].season_units().iter().any(|&u| u != 0), "the closing season trained");
        clear_human_season(&mut ours);
        for (at, len) in HUMAN_SEASON_COUNTERS {
            for k in at..at + len {
                assert_eq!(ours.extras[slot].get::<u8>(k), theirs.extras[slot].get::<u8>(k), "extra +{k:#x}");
            }
        }
    }

    /// The human managers' season end over the original's pair: a year
    /// more as a trainer, and in mode 9 his standing two down on an
    /// average mark of 67.76, as NewSeason000 keeps them.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_season_end_closes_the_managers_season_as_the_original() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let read = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs) = (read("SeasonEnd364.an3"), read("NewSeason000.an3"));
        close_managers(&mut ours, &mut Random::new(3));
        let (x, y) = (&ours.managers[0], &theirs.managers[0]);
        assert_eq!(x.trainer_years(), y.trainer_years());
        assert_eq!(x.get::<i16>(OWN_STANDING), y.get::<i16>(OWN_STANDING));
    }

    /// The human managers' new season over the original's pair, SeasonEnd364
    /// settled first: the board's confidence five up to 88, the nerves back
    /// near a hundred, his club's cash noted, the season's marks and flags
    /// and his entries' flags at nought, as NewSeason000 keeps them.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_new_season_opens_the_managers_year_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let read = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs) = (read("SeasonEnd364.an3"), read("NewSeason000.an3"));
        let exe = a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap();
        settle_season(&mut ours, &crate::league::LeagueTables::load(&exe).unwrap());
        open_managers(&mut ours, &mut Random::new(3));
        let (x, y) = (&ours.managers[0], &theirs.managers[0]);
        assert_eq!(x.confidence(), y.confidence());
        assert!((96..=100).contains(&x.get::<i16>(NERVES)) && (96..=100).contains(&y.get::<i16>(NERVES)));
        assert_eq!(x.get::<i32>(CLUB_CASH), y.get::<i32>(CLUB_CASH), "the club's cash");
        for at in [MARKS_SUM, MARKS_COUNT, 0x1f0, 0x1a8] {
            assert_eq!(x.get::<u16>(at), y.get::<u16>(at), "manager +{at:#x}");
        }
        for k in 0..ENTRIES {
            let at = ENTRIES_AT + k * ENTRY;
            assert_eq!((x.get::<u8>(at), x.get::<u16>(at + 6)), (y.get::<u8>(at), y.get::<u16>(at + 6)), "entry {k}");
        }
    }

    /// The objects the new season clears, over the original's pair: the
    /// eleven of the year, the talks booked, the appointments and the
    /// others stand as NewSeason000 keeps them, and so do the tables of the
    /// countries kept in short, their country numbers only (`0x1c4be78`,
    /// which the original fills again, aside).
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_new_season_clears_its_objects_as_the_original() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let read = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs) = (read("SeasonEnd364.an3"), read("NewSeason000.an3"));
        clear_objects(&mut ours);
        for name in CLEARED_OBJECTS.into_iter().filter(|&n| n != "0x1c4be78" && n != "0x1c65e88").chain([KEPT_TABLES]) {
            assert_eq!(ours.block(name), theirs.block(name), "{name}");
        }
    }

    /// The new season opens the saved one — measured on the original's
    /// pair, its last day before the season end (SeasonEnd364) against
    /// its first day after (NewSeason000). The counters and the calendar
    /// roll as above, every league of the four countries played stands
    /// with no matchday played, no result written and no crowd kept, and
    /// the cups stand set up as the original's with their first round
    /// drawn and nothing played ([`crate::cup_draw`]). The original's own
    /// lottery figure (995) stands in place of the replica's drawn one.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_new_season_opens_the_saved_season() {
        use crate::opponent::{self, HIGHEST_PAIRING, League, PAIRING, PAIRINGS};
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let read = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut closed, opened) = (read("SeasonEnd364.an3"), read("NewSeason000.an3"));
        open_season(&mut closed, &mut Random::new(7));
        let (a, b) = (&closed.globals, &opened.globals);
        assert_eq!((a.seasons(), a.start_year()), (b.seasons(), b.start_year()));
        assert_eq!((a.day(), a.week_start(), a.half_weeks()), (0, 0, 0));
        assert_eq!(
            (a.first_day(), a.date(), a.day_of_year(), a.year_days()),
            (b.first_day(), b.date(), b.day_of_year(), b.year_days())
        );
        assert_eq!((a.europe_stage(), a.uefa_round()), (b.europe_stage(), b.uefa_round()));
        assert_eq!(b.lottery(), 995, "the original's drawn lottery");
        assert!(a.lottery() < 1000, "the replica draws one of its own");
        for at in (0xb5dfb0..0xb5dfba).chain([0xb5dfc4, 0xb5dfc5, 0xb5d1a4, 0xb5d1a5]) {
            assert_eq!(a.get::<u8>(at), b.get::<u8>(at), "globals {at:#x}");
        }
        for nation in opponent::played_nations(&closed) {
            for level in 0..closed.countries.leagues(usize::from(nation)).max(0) {
                let (x, y) = (
                    League::of(&closed, nation, level as u8).unwrap(),
                    League::of(&opened, nation, level as u8).unwrap(),
                );
                assert_eq!((x.played(), y.played()), (0, 0), "{nation} league {level} played");
                for day in 0..x.matchdays() {
                    assert!(x.games(day).iter().all(|g| g.2.is_none()), "{nation} {level} +{day}");
                    assert!(y.games(day).iter().all(|g| g.2.is_none()), "{nation} {level} kept");
                }
                assert!(x.matchday_crowds().iter().all(|c| *c == 0), "{nation} {level} crowds");
                assert!(y.matchday_crowds().iter().all(|c| *c == 0), "{nation} {level} kept");
            }
            let slot = closed.countries.slot(usize::from(nation));
            let name = format!("{:#x}[{slot}]", opponent::CUPS_AT);
            let (Some(cup), Some(kept)) = (closed.block(&name), opened.block(&name)) else {
                continue;
            };
            assert_eq!(cup[..0xc], kept[..0xc], "country {nation}'s cup setup");
            for q in 0..=HIGHEST_PAIRING {
                let at = PAIRINGS + q * PAIRING;
                let drawn = |c: &[u8]| c[at] != 0;
                assert_eq!(drawn(cup), drawn(kept), "{nation} pairing {q} drawn");
                assert!(cup[at + 8..at + PAIRING].iter().all(|&x| x == 0), "{nation} pairing {q} unplayed");
            }
        }
    }
}
