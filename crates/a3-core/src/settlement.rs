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

/// The club's contracts that run by the season, each its years left in a
/// byte (club offsets): the main sponsor (+0x509), the boards' package
/// (+0x5a1), the fourteen boards one by one (+0x531, eight apart), the
/// shirt's (+0x5a9) and the television's (+0x5b1).
const CONTRACT_YEARS: [usize; 4] = [0x509, 0x5a1, 0x5a9, 0x5b1];
const BOARD_YEARS: usize = 0x531;
const BOARDS: usize = 14;
const BOARD_STRIDE: usize = 8;

/// The new season counts every club's contracts down a year, those of the
/// countries played (`FUN_0042b2fc` per club, `FUN_00716522` in each): a
/// contract with years left keeps one less, one with none stays at none. A
/// club that moves league — promoted or relegated (`FUN_0069a1e1`,
/// `FUN_0069a221`: the season flags 4 and 8) — loses them all. The walk
/// comes before the human's closing season is kept as its last
/// ([`crate::finances::keep_last_season`]), whose copy holds the years
/// already counted down. The moved human's message (0x10c) and what the
/// original does then with its own league's clubs (`FUN_0069d930`,
/// `FUN_0069d9b5`) are not built; neither are the new contracts the
/// computer clubs sign afterwards nor the fallback sponsor a club without
/// one gets (`FUN_00673566`, `FUN_0071684e`).
pub fn count_down_contracts(records: &mut Records) {
    for i in 0..records.teams.len() {
        let country = i16::from(records.teams[i].country());
        if country == 0 || !records.globals.played(country) {
            continue;
        }
        let team = &mut records.teams[i];
        let moved = team.get::<u16>(SEASON_FLAGS) & MOVED != 0;
        let fields = CONTRACT_YEARS.into_iter().chain((0..BOARDS).map(|k| BOARD_YEARS + k * BOARD_STRIDE));
        for at in fields {
            let years = club::<u8>(team, at);
            put::<u8>(team, at, if moved { 0 } else { years.saturating_sub(1) });
        }
    }
}

/// The new season's shirt numbers, for every club of the countries
/// played (`FUN_0042b424`'s `FUN_006d924a(club, 0, 0)`): of two players
/// sharing a number one loses it — the original's code takes it from the
/// stronger —; then (`FUN_006d947c`) the squad is ordered by position,
/// one to ten, and within one by strength, and each player without a
/// number takes the first free of his position's three
/// (`DAT_008a66ac`), the numbers others hold marked as the order reaches
/// them; whoever is left draws one to forty and takes the next free from
/// there. (Player +0x223, the object's +0x26; his position +0x21d.)
const SHIRT_CHOICES: [u8; 30] = [1, 21, 30, 5, 22, 23, 4, 6, 14, 2, 13, 15, 3, 12, 16, 8, 18, 25, 7, 17, 24, 19, 28, 27, 10, 26, 20, 9, 11, 29];
const SHIRT_DRAWN: u32 = 40;

pub fn number_shirts(records: &mut Records, random: &mut Random) {
    for i in 0..records.teams.len() {
        let country = i16::from(records.teams[i].country());
        if country == 0 || !records.globals.played(country) {
            continue;
        }
        let squad: Vec<usize> = records.teams[i]
            .players()
            .into_iter()
            .filter(|&id| id != 0 && usize::from(id) < records.players.len())
            .map(usize::from)
            .collect();
        number_squad(records, &squad, random);
    }
}

fn number_squad(records: &mut Records, squad: &[usize], random: &mut Random) {
    let shirt = |r: &Records, id: usize| r.players[id].get::<u8>(0x223);
    let strength = |r: &Records, id: usize| r.players[id].strength();
    let position = |r: &Records, id: usize| r.players[id].positions()[0];
    for &a in squad {
        for &b in squad {
            if a != b && shirt(records, a) == shirt(records, b) {
                let loser = if strength(records, b) < strength(records, a) { a } else { b };
                records.players[loser].put::<u8>(0x223, 0);
            }
        }
    }
    let mut order: Vec<usize> = Vec::new();
    for pos in 1..=10u8 {
        let start = order.len();
        order.extend(squad.iter().copied().filter(|&id| position(records, id) == pos));
        for a in start..order.len() {
            for b in a + 1..order.len() {
                if strength(records, order[a]) < strength(records, order[b]) {
                    order.swap(a, b);
                }
            }
        }
    }
    let mut used = [false; 256];
    let (mut current, mut next) = (0u8, 0usize);
    for &id in &order {
        let pos = position(records, id);
        if pos != current {
            current = pos;
            next = 0;
        }
        let own = shirt(records, id);
        if own != 0 {
            used[usize::from(own)] = true;
            continue;
        }
        let choice = |k: usize| SHIRT_CHOICES[(usize::from(pos) - 1) * 3 + k];
        while next < 3 && used[usize::from(choice(next))] {
            next += 1;
        }
        if next < 3 {
            let n = choice(next);
            records.players[id].put::<u8>(0x223, n);
            used[usize::from(n)] = true;
            next += 1;
        }
    }
    for &id in squad {
        if shirt(records, id) != 0 {
            continue;
        }
        let mut n = random.below(SHIRT_DRAWN) as usize + 1;
        while n < used.len() - 1 && used[n] {
            n += 1;
        }
        records.players[id].put::<u8>(0x223, n as u8);
        used[n] = true;
    }
}

/// What the new season's first day clears of the season's bookings:
/// every club of the countries played — the main one and the more ones,
/// in every league and the human club alike — stands with its two
/// blocks of season fields zeroed and with the first row of its ticket
/// sales plan copied down over the rest; the cash between the blocks
/// keeps what the season made (`FUN_0042b2fc` walks the countries'
/// clubs, `FUN_0042b424` reaches `FUN_0042b589`, and there
/// `FUN_00672839` clears), and the clubs the walk plans get their
/// season's sums and grades again (`plan_season`). What the original sets
/// back up beside — the ticket sales target (`FUN_0067324c`), the drawn
/// sums and the class template's money — the replica has not built, so
/// its clubs stand at nought there. The cycle reaches this from
/// [`open_season`].
pub fn clear_bookings(records: &mut Records, random: &mut Random) {
    let second = records.globals.seasons() > 1;
    let holders = records.block(HOLDERS).map(<[u8]>::to_vec).unwrap_or_default();
    for i in 0..records.teams.len() {
        let country = i16::from(records.teams[i].country());
        if country == 0 || !records.globals.played(country) {
            continue;
        }
        let system = usize::from(records.countries.cup_system(usize::from(records.teams[i].country())));
        clear_club(&mut records.teams[i]);
        plan_club(&mut records.teams[i], system);
        plan_staff(records, i);
        if records.teams[i].humans()[1] == 0 {
            let carried = players_carried_on(records, &records.teams[i]);
            put::<i32>(&mut records.teams[i], 0x17c, carried);
            if records.teams[i].level() >= 10 {
                let skill = manager_skill(records, &records.teams[i]);
                plan_amateur_stadium(&mut records.teams[i], skill, random);
            }
            plan_bonuses(&mut records.teams[i], system);
            let bonuses = expected_bonuses(records, &records.teams[i]);
            put::<i32>(&mut records.teams[i], EXPECTED_BONUSES, bonuses);
        }
        let first = usize::try_from(records.countries.first_team(usize::from(records.teams[i].country()))).unwrap_or(0);
        let held = holder(&holders, records.teams[i].country(), i.wrapping_sub(first));
        if planned(&records.teams[i]) {
            insure_squad(records, i);
        }
        let cover = insurance(records, &records.teams[i]);
        let skill = manager_skill(records, &records.teams[i]);
        plan_season(&mut records.teams[i], second, held, cover);
        if planned(&records.teams[i]) {
            plan_drawn(&mut records.teams[i], skill, random);
            let capacity = records
                .block(&format!("{STADIUMS}[{i}]"))
                .and_then(|b| b.get(STADIUM_CAPACITY..STADIUM_CAPACITY + 4))
                .map_or(0, |b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]));
            sign_main_sponsor(&mut records.teams[i], system, skill, held, capacity, random);
            number_sponsor(&mut records.teams[i], system, random);
            plan_price(&mut records.teams[i], system, random);
            sign_boards(&mut records.teams[i], system, random);
            sign_shirt(&mut records.teams[i], system, random);
            sign_television(&mut records.teams[i], system, random);
        }
    }
}

/// What the original's walk sets up beside the clearing for each club it
/// plans (`FUN_0042b589`: a club below the tenth level whose +0x8b is
/// clear, so not the human's): the two sums of 25 000 for every band step
/// below the top (+0x19c, +0x1a0: 75 000 in the first league, 50 000 in
/// the second, 25 000 lower), 100 000 at +0x1b8, fifty at +0x5c4, the
/// grade at +0x5e0 by the league and the place (the first league four
/// less a tenth of the place, the second three less, lower two in the
/// top ten and one below), +0x5e1 to nought and +0x5e2 one for a top-ten
/// club of the first league (a club with no manager at +0x8a), +0x1b0 by
/// the league a season after the first (800 000, 450 000, 200 000) and
/// +0x1f8 one in the second league and in the first, two there for a
/// club of class five or more (team +0x875, the object's +0x11e0).
/// Its two sums from the season's flags: +0x12c 400 000, 200 000 or
/// 100 000 by the band (`DAT_008774c4` thousands), a million more for the
/// cup's winner (flag 0x1000); +0x130 five millions for the champion
/// (flag 1) or the Champions League's holder, else two millions for a
/// club in Europe (flag 0x4000), 1 200 000 more for the holder of the
/// first or second European cup (`FUN_0042419d` over `DAT_00ae6dd4`).
/// The squad's salaries stand at +0x17c (`FUN_0062023c`), +0x1fc at
/// twenty or +0x304's figure and +0x504 at 20 000 in the top three
/// leagues, 10 000 below (`FUN_00716a87`'s tail).
/// Measured on the pair: every one of the 147 clubs planned agrees. The
/// walk's drawn parts (the TV's and the boards' money, +0x18c, +0x1a8,
/// +0x1a4), the sponsors it signs (`FUN_00716a87`) and the first
/// season's +0x1b0 are not built.
const PLAN_STEP: i32 = 25_000;
const PLAN_1B8: i32 = 100_000;
const PLAN_5C4: u16 = 50;
const PLAN_1FC: u16 = 20;
const PLAN_RATE_TOP: i32 = 20_000;
const PLAN_RATE_LOW: i32 = 10_000;
const PLAN_1B0: [i32; 3] = [800_000, 450_000, 200_000];
const CLUB_CLASS: usize = 0x875;
const HIGH_CLASS: u8 = 5;

const PLAN_12C: [i32; 3] = [400_000, 200_000, 100_000];
const PLAN_CUP_WINNER: i32 = 1_000_000;
const PLAN_CHAMPION: i32 = 5_000_000;
const PLAN_EUROPE: i32 = 2_000_000;
const PLAN_HOLDER: i32 = 1_200_000;
const CHAMPION_FLAG: u16 = 1;
const EUROPE_FLAG: u16 = 0x4000;
const CUP_WINNER_FLAG: u16 = 0x1000;
/// The holders of the European cups, a country byte and a club word in
/// each four (`DAT_00ae6dd4`).
const HOLDERS: &str = "0xae6dd4";

/// Which European cup's holder a club is, one to three, or nought
/// (`FUN_0042419d`).
fn holder(holders: &[u8], country: u8, number: usize) -> u8 {
    (0..3u8)
        .find(|&k| {
            let at = usize::from(k) * 4;
            holders.get(at) == Some(&country)
                && holders.get(at + 2..at + 4).map(|w| usize::from(u16::from_le_bytes([w[0], w[1]]))) == Some(number)
        })
        .map_or(0, |k| k + 1)
}

/// The staff's salaries for what is left of the season, which the
/// clearing plans again (`FUN_00672839`): the trainer's (team +0x9e) at
/// +0x158 (`FUN_00620453`), the manager-trainer's (+0x9c) — with none the
/// manager's (+0xa0) — at +0x154 (`FUN_0062053b`), the co-trainer's
/// (+0xa2) at +0x15c (`FUN_0062068a`) and the one of the second persons'
/// table (+0xa4, `FUN_00631a0e`) at +0x160 (`FUN_00620702`): the yearly
/// salary (the person's +0x30, `FUN_006317c6`) in twelfths times the
/// months left (`DAT_0089e220` by the month, `_DAT_00b5cbdc`), its rest
/// beside from July on; and the grade's sum at +0x1ac by the grade the
/// club held last season (`FUN_006231e0`). A human club the walk passes by (its +0x8b
/// stands): the original keeps its sums from the season before, as the
/// co-trainer's shows, so the replica leaves them.
const MONTHS_LEFT: [i32; 13] = [0, 6, 5, 4, 3, 2, 1, 12, 11, 10, 9, 8, 7];
const MONTH_AT: usize = 0xb5cbdc;
const PERSON_SALARY: usize = 0x30;
const FIRST_HALF: i16 = 6;
const OTHER_PERSONS: &str = "0xbe17f8";
const OTHER_PERSON_SALARY: usize = 0x10;
const LAST_OTHER_PERSON: u16 = 100;
const CO_TRAINER_AT: usize = 0xa2;
const OTHER_STAFF_AT: usize = 0xa4;

fn plan_staff(records: &mut Records, team: usize) {
    let month = records.globals.get::<i16>(MONTH_AT);
    let left = MONTHS_LEFT.get(usize::try_from(month).unwrap_or(0)).copied().unwrap_or(0);
    let rest = |salary: i32| -> i32 {
        let tail = if month > FIRST_HALF { salary % 12 } else { 0 };
        (salary / 12).wrapping_mul(left).wrapping_add(tail)
    };
    let person = |id: u16| -> i32 {
        if id == 0 || id > LAST_PERSON {
            return 0;
        }
        records.persons.get(usize::from(id)).map_or(0, |p| p.get::<i32>(PERSON_SALARY))
    };
    let other = |id: u16| -> i32 {
        if id == 0 || id > LAST_OTHER_PERSON {
            return 0;
        }
        records
            .block(&format!("{OTHER_PERSONS}[{id}]"))
            .and_then(|b| b.get(OTHER_PERSON_SALARY..OTHER_PERSON_SALARY + 4))
            .map_or(0, |b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let t = &records.teams[team];
    if t.humans() != [0, 0] {
        return;
    }
    let [manager_trainer, trainer, manager] = t.staff();
    let trainers = person(trainer);
    let managers = person(if manager_trainer == 0 { manager } else { manager_trainer });
    let co = person(t.get::<i16>(CO_TRAINER_AT).max(0) as u16);
    let more = other(t.get::<i16>(OTHER_STAFF_AT).max(0) as u16);
    let t = &mut records.teams[team];
    for (at, salary) in [(0x158, trainers), (0x154, managers), (0x15c, co), (0x160, more)] {
        put::<i32>(t, at, if salary == 0 { 0 } else { rest(salary) });
    }
    // The grade's sum (`FUN_006231e0(5, 1, grade)`): five of the first
    // and one of the second figure of `DAT_0089e45c`'s pair for the
    // grade the club still holds from last season, in thousands.
    let grade = usize::from(club::<u8>(t, 0x5e0));
    let (a, b) = GRADE_PAIRS.get(grade).copied().unwrap_or((0, 0));
    put::<i32>(t, 0x1ac, (5 * a + b) * 1000);
}

/// The grades' pairs the clearing reckons +0x1ac from (`DAT_0089e45c`).
const GRADE_PAIRS: [(i32, i32); 5] = [(10, 30), (15, 50), (20, 80), (30, 120), (50, 200)];

/// The walk's drawn sums (`FUN_0042b589`): a figure from the band's row
/// of `DAT_008774b0` — its first, the manager's skill (the person's
/// +0x25 of the manager-trainer or the manager, `FUN_0040a0c5`) times
/// its second, the cash in millions times its third, in thousands
/// (`FUN_0042b489`) —, kept within 50 000 and five millions; a club with
/// a stadium of its own (team +0x872 below nought, `FUN_0063e793`) puts
/// seven or eight tenths of it drawn at +0x18c and the rest at +0x1a8,
/// each down to fifty thousands and at least 50 000, the first league's
/// +0x1a8 at least 750 000; a sharing club nought at both. +0x1a4 is the
/// figure down to fifty thousands and drawn 50 000 more, at least 50 000.
/// The plan's rest copies them to +0x294, +0x2b0 and +0x2ac, and draws
/// +0x4fc, five times below six, beside the class's (team +0x875) four
/// millions and four more at +0x500. (The
/// landlord's share of a sharing club's and `FUN_006dd937` are not
/// built.) Measured on the original's first new-season day: all 144
/// clubs with their own stadium hold one of the two draws, the three
/// sharing ones nought.
const DRAWN_ROWS: [(i64, i64, i64); 3] = [(400, 50, 50), (250, 25, 25), (100, 15, 15)];
const DRAWN_LEAST: i32 = 50_000;
const DRAWN_MOST: i32 = 5_000_000;
const DRAWN_TOP_LEAST: i32 = 750_000;
const SKILL_AT: usize = 0x25;
const SHARED_AT: usize = 0x872;
const CASH_AT: usize = 0x210;

fn manager_skill(records: &Records, team: &Team) -> i64 {
    let [manager_trainer, _, manager] = team.staff();
    let id = if manager_trainer != 0 { manager_trainer } else { manager };
    if id == 0 || id > LAST_PERSON {
        return 0;
    }
    records.persons.get(usize::from(id)).map_or(0, |p| i64::from(p.get::<i8>(SKILL_AT)))
}

fn drawn_figure(team: &Team, skill: i64) -> i32 {
    let (first, per_skill, per_million) = DRAWN_ROWS[usize::from(team.level().min(2))];
    let millions = club::<i64>(team, CASH_AT) / 1_000_000;
    let v = (millions.wrapping_mul(per_million) + first + skill * per_skill).wrapping_mul(1000) as i32;
    v.clamp(DRAWN_LEAST, DRAWN_MOST)
}

fn plan_drawn(team: &mut Team, skill: i64, random: &mut Random) {
    let figure = drawn_figure(team, skill);
    let tenths = random.below(2) as i32 + 7;
    let down = |v: i32| v / DRAWN_LEAST * DRAWN_LEAST;
    let (own, other) = if team.get::<i8>(SHARED_AT) < 0 {
        let own = down(figure * tenths / 10).max(DRAWN_LEAST);
        let mut other = down(figure * (10 - tenths) / 10).max(DRAWN_LEAST);
        if team.level() == 0 {
            other = other.max(DRAWN_TOP_LEAST);
        }
        (own, other)
    } else {
        (0, 0)
    };
    put::<i32>(team, 0x18c, own);
    put::<i32>(team, 0x1a8, other);
    let more = (down(figure) + random.below(2) as i32 * DRAWN_LEAST).max(DRAWN_LEAST);
    put::<i32>(team, 0x1a4, more);
    put::<i32>(team, 0x294, own);
    put::<i32>(team, 0x2b0, other);
    put::<i32>(team, 0x2ac, more);
    // The plan's rest: five times a draw below six at +0x4fc, and the
    // class's millions at +0x500 (`FUN_00673566`).
    put::<u16>(team, 0x4fc, random.below(6) as u16 * 5);
    let class = i32::from(team.get::<u8>(CLUB_CLASS));
    put::<i32>(team, 0x500, (class * 4 + 4) * 1_000_000);
}

/// The main sponsor's offer the walk signs every planned club with
/// (`FUN_00716be0`, taken over by `FUN_00671bce` in `FUN_00716a87`), a
/// season after the first:
/// - the cut on relegation at +0x50a: ten times one more than a fifth of
///   the place (at most two) and a draw of two;
/// - its years at +0x509: one, two or three by a draw of a hundred (below
///   20, below 80, else), one more drawn of two for a champion or a
///   promoted club (flags 1, 4), of five for flag 0x10, of two for the
///   cup's winner, a European cup's holder or home gates above 40 000;
/// - its money at +0x510: 24 less the place in 100 000s, 400 000 and
///   the gates' fiftieth in 5 000s below the eighteenth place and not
///   promoted, else the gates' fifth in 300s and 300 000, at most 15
///   millions; two millions more for the champion, else 1 500 000 for
///   flag 0x40; a promoted club one million in the first league, 500 000
///   in the second; a fifth more for a club in Europe (0x4000);
/// - the extras for a title (+0x514, 9–12 × 250 000), a European place
///   (+0x518, first league 4–6 × 250 000), the cup's final (+0x51c, 6–8
///   × 200 000), a cup round (+0x520, 3–5 × 50 000) and +0x524 (first
///   league 6–9 × 500 000), all drawn;
/// - the attendance contract: the gates two thousand up in thousands at
///   +0x52c, within nine tenths of the stadium (+0x3c of its record; 500
///   with none), and its bonus at +0x528, the money's thousandths times
///   275, 300 or 325 drawn — both nought when no gate is asked;
///
/// then the seven sums in hundredths times the club's +0x10f (team
/// +0x138) twice over fifty and ninety, by the system's row of
/// `DAT_008a7aa8`; a drawn style takes three or six tenths of some of
/// them (`DAT_008a7af0`); each rounded by its size (`FUN_007169f0`) and
/// again, in hundredths times three for each of the manager's skill and
/// 150 (`FUN_00716a87`). The plan's rest books the money at +0x2f4 and
/// reckons the plan's sponsors at +0x1ec (`FUN_0061f483`). (The
/// offer's bytes +0x50b to +0x50d the original leaves as its stack held
/// them, so they are not written.) Measured on the original's first
/// new-season day: every planned club's fields are among their draws.
const STADIUM_CAPACITY: usize = 0x3c;
const SPONSOR_STYLES: [(usize, usize, i32); 3] = [(1, 6, 3), (0, 6, 6), (0, 1, 3)];

fn round_by_size(v: i32) -> i32 {
    let unit = if v < 10_000 {
        100
    } else if v < 100_000 {
        1_000
    } else if v < 1_000_000 {
        10_000
    } else if v < 10_000_000 {
        50_000
    } else {
        100_000
    };
    v / unit * unit
}

fn sign_main_sponsor(team: &mut Team, system: usize, skill: i64, held: u8, capacity: i32, random: &mut Random) {
    let level = team.level();
    let place = i32::from(team.league_place());
    let flags = team.get::<u16>(FLAGS_AT);
    let promoted = flags & 4 != 0;
    let gates = home_average(team);
    let cut = ((place / 5).min(2) + 1 + random.below(2) as i32) * 10;
    let mut years: u8 = match random.below(100) {
        0..20 => 1,
        20..80 => 2,
        _ => 3,
    };
    if (flags & CHAMPION_FLAG != 0 || promoted) && random.below(2) != 0 {
        years += 1;
    }
    if flags & 0x10 != 0 && random.below(5) == 0 {
        years += 1;
    }
    if (flags & CUP_WINNER_FLAG != 0 || held != 0 || gates > 40_000) && random.below(2) != 0 {
        years += 1;
    }
    let mut money = if place < 18 && !promoted {
        (24 - place) * 100_000 + 400_000 + gates / 50 * 5_000
    } else {
        gates / 5 * 3_000 / 10 + 300_000
    };
    money = money.min(15_000_000);
    if flags & CHAMPION_FLAG != 0 {
        money += 2_000_000;
    } else if flags & 0x40 != 0 {
        money += 1_500_000;
    }
    if promoted {
        match level {
            0 => money += 1_000_000,
            1 => money += 500_000,
            _ => {}
        }
    }
    if flags & EUROPE_FLAG != 0 {
        money = money / 5 * 6;
    }
    let mut sums = [0i32; 7];
    sums[0] = money;
    sums[1] = (random.below(4) as i32 + 9) * 250_000;
    sums[2] = if level == 0 { (random.below(3) as i32 + 4) * 250_000 } else { 0 };
    sums[3] = (random.below(3) as i32 + 6) * 200_000;
    sums[4] = (random.below(3) as i32 + 3) * 50_000;
    sums[5] = if level == 0 { (random.below(4) as i32 + 6) * 500_000 } else { 0 };
    let mut gate = (gates + 2_000) / 1_000 * 1_000;
    sums[6] = money / 1_000 * (random.below(3) as i32 * 25 + 275);
    let held_most = ((f64::from(capacity) * 0.9) / 500.0 * 500.0) as i32;
    let held_most = if held_most == 0 { 500 } else { held_most };
    gate = gate.min(held_most);
    if gate < 1 {
        sums[6] = 0;
        gate = 0;
    }
    let figure = (i32::from(team.get::<u8>(0x138)) * 2 / 50 + 90) * BOARD_ROWS.get(system * 3 + usize::from(level.min(2))).copied().unwrap_or(0) / 100;
    for v in &mut sums {
        *v = *v / 100 * figure;
    }
    let (from, to, tenths) = SPONSOR_STYLES[random.below(3) as usize];
    for v in &mut sums[from..to] {
        *v = *v * tenths / 10;
    }
    let pull = skill * 3 + 150;
    for (k, v) in sums.iter_mut().enumerate() {
        let first = round_by_size(*v);
        let scaled = (i64::from(first) / 100 * pull) as i32;
        put::<i32>(team, 0x510 + k * 4, round_by_size(scaled));
    }
    put::<u8>(team, 0x509, years);
    put::<u8>(team, 0x50a, cut as u8);
    put::<i32>(team, 0x52c, gate);
    let booked = club::<i32>(team, 0x510);
    put::<i32>(team, 0x2f4, booked);
    // The plan's sponsors (`FUN_0061f483`): the contract's money, +0x1fc
    // times the rate, and twelve months of perimeter advertising
    // (`FUN_0061ffe4`) — nought here, a computer club having neither
    // home games nor an attendance plan at this point.
    let rate = i32::from(club::<u16>(team, 0x1fc) as i16).wrapping_mul(club::<i32>(team, 0x504));
    put::<i32>(team, 0x1ec, if years != 0 { booked } else { 0 }.wrapping_add(rate));
}

/// The main sponsor the walk signs a planned club with (`FUN_00716a87`):
/// its number at +0x4ea, drawn among the forty of the country's cup
/// system (`DAT_00ae5d0c`), from one. (The offer it signs, its years
/// and money, is not built.) Measured on the original's first new-season
/// day: all 147 planned clubs hold a number of their system's forty.
const SPONSORS_PER_SYSTEM: u32 = 40;

fn number_sponsor(team: &mut Team, system: usize, random: &mut Random) {
    let first = system as u32 * SPONSORS_PER_SYSTEM + 1;
    put::<u16>(team, 0x4ea, (first + random.below(SPONSORS_PER_SYSTEM)) as u16);
}

/// The plan's rest's figure at +0x4ec (`FUN_00673566`, read from its
/// x87 code): the league's row of `DAT_008a0278` (from the cup system's
/// start in `DAT_0089dd50`; two from the sixth level on) times eighteen
/// less club +0x10b (team +0x134) held within 5 and 18, times eight less
/// club +0xec (team +0x11c), in thousands, over the sponsor's rate
/// (+0x504) and times one less the rate's distance from 25 000 in fifty
/// thousands — cut to a whole short —, then drawn up to five more.
/// Measured on the original's first new-season day: all 147 planned
/// clubs lie within their five.
const PRICE_ROWS: [i32; 23] = [6, 4, 3, 3, 3, 3, 6, 4, 3, 2, 6, 3, 8, 3, 8, 3, 4, 5, 4, 3, 3, 5, 5];
const PRICE_ROWS_START: [usize; 8] = [0, 6, 10, 12, 14, 16, 17, 18];
const PRICE_LOW_LEAGUES: i32 = 2;
const PRICE_PLACE_AT: usize = 0x134;
const PRICE_CLASS_AT: usize = 0x11c;

fn price_base(team: &Team, system: usize) -> i32 {
    let level = usize::from(team.level());
    let row = if level >= 6 {
        PRICE_LOW_LEAGUES
    } else {
        PRICE_ROWS_START.get(system).and_then(|&s| PRICE_ROWS.get(s + level)).copied().unwrap_or(0)
    };
    let figure = (18 - i32::from(team.get::<u8>(PRICE_PLACE_AT))).clamp(5, 18);
    let class = 8 - i32::from(team.get::<u8>(PRICE_CLASS_AT));
    let rate = club::<i32>(team, 0x504);
    if rate == 0 {
        return 0;
    }
    let d = f64::from(row) * f64::from(figure) * f64::from(class) * 1000.0 / f64::from(rate);
    let off = f64::from(rate - PLAN_STEP) / f64::from(DRAWN_LEAST);
    i32::from(((1.0 - off) * d) as i32 as i16)
}

fn plan_price(team: &mut Team, system: usize, random: &mut Random) {
    let base = price_base(team, system);
    let drawn = base - 5 + random.below(11) as i32;
    put::<i32>(team, 0x4ec, drawn.max(base));
}

/// The boards' package the walk signs every planned club for a year
/// (`FUN_007176cf` → `FUN_0071774b`): its number at +0x5a0 drawn from one
/// to ten, its year at +0x5a1, its money at +0x5a4 — ten to thirteen
/// drawn, times 800 (`DAT_00a3b3e8`), times the cup system's row of
/// `DAT_008a7aa8` by the band —, which the plan's rest takes as the
/// boards' sum at +0x1f0 and +0x2f8 (`FUN_0061f4e6`). Measured on the
/// original's first new-season day: all 147 planned clubs hold a year's
/// package of their row's four sums.
const BOARD_ROWS: [i32; 24] = [140, 40, 25, 140, 40, 25, 120, 35, 15, 180, 50, 10, 160, 50, 10, 100, 35, 25, 100, 35, 25, 70, 35, 25];
const BOARD_UNIT: i32 = 800;

fn sign_boards(team: &mut Team, system: usize, random: &mut Random) {
    put::<u8>(team, 0x5a0, random.below(10) as u8 + 1);
    put::<u8>(team, 0x5a1, 1);
    let row = BOARD_ROWS.get(system * 3 + usize::from(team.level().min(2))).copied().unwrap_or(0);
    let money = (random.below(4) as i32 + 10) * BOARD_UNIT * row;
    put::<i32>(team, 0x5a4, money);
    put::<i32>(team, 0x1f0, money);
    put::<i32>(team, 0x2f8, money);
}

/// The television contract the walk signs every planned club anew
/// (`FUN_00717709` → `FUN_007174c2`): its number at +0x5b0 drawn from one
/// to ten, one or two years drawn at +0x5b1, its money at +0x5b4. In the
/// first league 32 less four for each step of club +0xec (team +0x11c)
/// and a draw below seven less it, in 70 000s, with two millions for the
/// champion (flag 1), else a million for flag 0x40, else 500 000 for
/// flag 0x10, else 100 000 for a club in Europe (0x4000), 200 000 more
/// when its home gates averaged above 30 000 (`FUN_00715b55`,
/// `FUN_00621143`), in hundredths times the cup system's first figure of
/// `DAT_008a7aa8`, down to ten thousands; in the second six and the draw
/// in 20 000s, lower two and the draw in 12 000s; and a step of 100 000,
/// 10 000 or 6 000 for each of +0xec below four. The plan's rest takes it
/// as the television's sum at +0x1f4 and +0x2fc (`FUN_0061f572`).
/// Measured on the original's first new-season day: all 147 planned
/// clubs hold one of their draws.
const TV_AVERAGE_HIGH: i32 = 30_000;

fn home_average(team: &Team) -> i32 {
    let home = i32::from(team.get::<u16>(DIVISOR_AT) as i16);
    if home < 1 { NO_GATES_PLAN } else { team.get::<i32>(crate::statistics::GATES_SUM) / home }
}

fn sign_television(team: &mut Team, system: usize, random: &mut Random) {
    put::<u8>(team, 0x5b0, random.below(10) as u8 + 1);
    let (years, money) = offer(team, system, random);
    put::<u8>(team, 0x5b1, years);
    put::<i32>(team, 0x5b4, money);
    put::<i32>(team, 0x1f4, money);
    put::<i32>(team, 0x2fc, money);
}

/// The shirt's contract, signed the same way when the club has none left
/// (`FUN_00717462`: the offer first, then its number at +0x5a8 drawn from
/// one to ten, the money at +0x5a9/+0x5ac); the plan's rest takes a
/// running contract's money as the shirt's sum at +0x204 and +0x30c
/// (`FUN_0061f593`).
fn sign_shirt(team: &mut Team, system: usize, random: &mut Random) {
    if club::<u8>(team, 0x5a9) == 0 {
        let (years, money) = offer(team, system, random);
        put::<u8>(team, 0x5a9, years);
        put::<i32>(team, 0x5ac, money);
        put::<u8>(team, 0x5a8, random.below(10) as u8 + 1);
    }
    let money = club::<i32>(team, 0x5ac);
    put::<i32>(team, 0x204, money);
    put::<i32>(team, 0x30c, money);
}

/// A contract's offer (`FUN_007174c2`): its years and its money.
fn offer(team: &Team, system: usize, random: &mut Random) -> (u8, i32) {
    let years = random.below(2) as u8 + 1;
    let class = i32::from(team.get::<u8>(PRICE_CLASS_AT));
    let span = 7 - class;
    let draw = if span > 0 { random.below(span as u32) as i32 } else { 0 };
    let flags = team.get::<u16>(FLAGS_AT);
    let (mut money, step) = match team.level() {
        0 => {
            let mut m = (32 - 4 * class + draw) * 70_000;
            m += if flags & CHAMPION_FLAG != 0 {
                2_000_000
            } else if flags & 0x40 != 0 {
                1_000_000
            } else if flags & 0x10 != 0 {
                500_000
            } else if flags & EUROPE_FLAG != 0 {
                100_000
            } else {
                0
            };
            if home_average(team) > TV_AVERAGE_HIGH {
                m += 200_000;
            }
            let row = BOARD_ROWS.get(system * 3).copied().unwrap_or(0);
            (m / 100 * row / 10_000 * 10_000, 100_000)
        }
        1 => ((draw + 6) * 20_000, 10_000),
        _ => ((draw + 2) * 12_000, 6_000),
    };
    if class < 4 {
        money += (4 - class) * step;
    }
    (years, money)
}

/// The team bonuses the clearing plans for every club the walk passes
/// (`FUN_00672611`): five rows at +0x5ba — `DAT_008a047c`'s figures
/// (20, 15, 12, 10, 8) times ten (`DAT_00a3b420`) times the cup system's
/// figure for the band (`DAT_008a0368`, six to a system), kept to a word
/// and cut to hundreds —; from the first row the title's bonus at +0x5c8
/// (a hundred times) and a European place's at +0x5cc (fifty times) in
/// the first league, the promotion's at +0x5d0 (a hundred times) below
/// it, and the last one at +0x5d4 for a club placed fifteenth or lower
/// (75 times, at most 100 000); below the first league a club sharing
/// its stadium (team +0x872 not below nought, `FUN_0063e793`) has the
/// first three at nought. Measured on the original's first new-season
/// day: all 211 computer clubs agree.
const BONUS_FIGURES: [i32; 5] = [20, 15, 12, 10, 8];
const BONUS_SYSTEMS: [[i32; 3]; 8] = [[8, 3, 1], [8, 3, 1], [8, 3, 1], [10, 3, 1], [10, 3, 1], [6, 0, 0], [7, 0, 0], [5, 0, 0]];
const BONUS_ROWS_AT: usize = 0x5ba;
const DROP_PLACE: u8 = 15;
const DROP_MOST: i32 = 100_000;

fn plan_bonuses(team: &mut Team, system: usize) {
    let band = usize::from(team.level().min(2));
    let figure = BONUS_SYSTEMS.get(system).map_or(0, |r| r[band]);
    for (k, &f) in BONUS_FIGURES.iter().enumerate() {
        let row = ((f * 10 * figure) & 0xffff) / 100 * 100;
        put::<u16>(team, BONUS_ROWS_AT + 2 * k, row as u16);
    }
    let first = i32::from(club::<u16>(team, BONUS_ROWS_AT));
    let drop = if team.league_place() < DROP_PLACE { 0 } else { (first * 75).min(DROP_MOST) };
    let (title, europe, promotion) = if band == 0 {
        (first * 100, first * 50, 0)
    } else if team.get::<i8>(SHARED_AT) >= 0 {
        (0, 0, 0)
    } else {
        (0, 0, first * 100)
    };
    put::<i32>(team, TITLE_BONUS, title);
    put::<i32>(team, EUROPE_BONUS, europe);
    put::<i32>(team, PROMOTED_BONUS, promotion);
    put::<i32>(team, LAST_BONUS, drop);
}

/// The players' pay the clearing carries into the plan at +0x17c
/// (`FUN_006202c1`, as [`crate::forecast`] reckons it): the pay booked —
/// nought, the bookings just cleared — and the squad's wage bill
/// ([`crate::finances::wage_bill`], `FUN_0062029e`, over a league of its
/// size) for the months left, at least the plan's players' sum, which
/// stands at nought then. A planned club has it overwritten by its
/// salaries; the amateurs keep it. Measured on the original's first
/// new-season day: all 64 amateur clubs agree.
fn players_carried_on(records: &Records, team: &Team) -> i32 {
    let month = records.globals.get::<i16>(MONTH_AT);
    let left = MONTHS_LEFT.get(usize::try_from(month).unwrap_or(0)).copied().unwrap_or(0);
    let bill = crate::finances::wage_bill(records, team, league_teams(records, team));
    let tail = if month > FIRST_HALF { bill % 12 } else { 0 };
    (bill / 12).wrapping_mul(left).wrapping_add(tail).max(0)
}

/// A club's league size as the original reads it (`FUN_00635a35`): its
/// league's clubs, eighteen from the sixth level on.
fn league_teams(records: &Records, team: &Team) -> i32 {
    let level = team.level();
    if level < 6 {
        crate::opponent::League::of(records, u16::from(team.country()), level).map_or(NO_LEAGUE_TEAMS, |l| l.teams() as i32)
    } else {
        NO_LEAGUE_TEAMS
    }
}

/// An amateur club's stadium sum at +0x1b0, which the clearing alone
/// sets (`FUN_00672839` a season after the first; a planned club has it
/// overwritten by its league's): 50 000 (`DAT_00a3b38c`) — its home gates'
/// share beside, nought for a club with no league games —, 5 000 less
/// (`DAT_00a3b3cc`) for each of the manager's skill (`FUN_0040a0c5`) a
/// draw of two lands, at least 25 000 (`DAT_00a3b3a4`). Measured on the
/// original's first new-season day: all 64 amateur clubs hold one of
/// their draws.
const AMATEUR_STADIUM: i32 = 50_000;
const AMATEUR_STADIUM_STEP: i32 = 5_000;
const AMATEUR_STADIUM_LEAST: i32 = 25_000;

fn plan_amateur_stadium(team: &mut Team, skill: i64, random: &mut Random) {
    let mut v = AMATEUR_STADIUM;
    for _ in 0..skill.max(0) {
        if random.below(2) != 0 {
            v -= AMATEUR_STADIUM_STEP;
        }
    }
    put::<i32>(team, 0x1b0, v.max(AMATEUR_STADIUM_LEAST));
}

/// The team bonuses the season is expected to cost, which the clearing
/// plans at +0x180 (`FUN_0061f64f(club, 0)`): the matches the board's
/// goal (team +0x135, the object's +0x10c) expects won —
/// `DAT_008a532c`'s first figure and its second for each club of the
/// league above ten (`FUN_006a2849`; a league's size by `FUN_00635a35`,
/// 18 from the sixth level on) — times fourteen times the bonus rows the
/// goal picks (`DAT_008911f8`: the first two, three in a row from the
/// next, or the last two, averaged), raised by +0x5c4 in three hundredths
/// (as it stands before the plan sets it); the title's and a European
/// place's bonus for each of the squad in the first league by the goal,
/// the promotion's below it for a club with a stadium of its own, the
/// last one's for each; and the squad's appearance bonuses (player
/// +0x152) over the season's matchdays, fourteen hundreds each. Measured
/// on the original's first new-season day: all 211 computer clubs agree.
const EXPECTED_BONUSES: usize = 0x180;
const GOAL_ROWS: [usize; 12] = [0, 1, 1, 1, 2, 2, 3, 3, 3, 3, 4, 4];
const GOAL_WINS: [(i32, i32); 12] = [(66, 5), (60, 4), (60, 4), (56, 4), (52, 4), (48, 3), (48, 3), (46, 3), (44, 3), (40, 2), (38, 2), (10, 1)];
const BOARD_GOAL_AT: usize = 0x135;
const NO_LEAGUE_TEAMS: i32 = 18;

fn expected_bonuses(records: &Records, team: &Team) -> i32 {
    let level = team.level();
    let teams = league_teams(records, team);
    let goal = usize::try_from(team.get::<i8>(BOARD_GOAL_AT)).unwrap_or(0).min(GOAL_ROWS.len() - 1);
    let rows: [i32; 5] = std::array::from_fn(|k| i32::from(club::<u16>(team, BONUS_ROWS_AT + 2 * k)));
    let row = match GOAL_ROWS[goal] {
        0 => (rows[0] + rows[1]) / 2,
        4 => (rows[3] + rows[4]) / 2,
        s => (rows[s - 1] + rows[s] + rows[s + 1]) / 3,
    };
    let (wins, per_club) = GOAL_WINS[goal];
    let won = wins + per_club * (teams - 10);
    let raise = i64::from(club::<u16>(team, 0x5c4) as i16) + 300;
    let mut v = (i64::from(won.wrapping_mul(14).wrapping_mul(row)) * raise / 300) as i32;
    let squad = i32::from(team.player_count() as i16);
    if level == 0 {
        if goal < 2 {
            v = v.wrapping_add(club::<i32>(team, TITLE_BONUS).wrapping_mul(squad));
        }
        if goal != 0 && goal < 4 {
            v = v.wrapping_add(squad.wrapping_mul(club::<i32>(team, EUROPE_BONUS)));
        }
    } else if team.get::<i8>(SHARED_AT) < 0 && goal < 4 {
        v = v.wrapping_add(squad.wrapping_mul(club::<i32>(team, PROMOTED_BONUS)));
    }
    v = v.wrapping_add(club::<i32>(team, LAST_BONUS).wrapping_mul(squad));
    if squad > 0 {
        let sum = team.players().into_iter().filter_map(|id| records.player(id)).fold(0i32, |s, p| s + i32::from(p.appearance_bonus()));
        v = v.wrapping_add(sum * (teams - 1) * 2 / squad * 14 * 100);
    }
    v
}

/// Whether the walk plans a club: below the tenth level, its +0x8b clear.
fn planned(team: &Team) -> bool {
    team.humans()[1] == 0 && team.level() < 10
}

/// The walk insures a planned club's players anew (`FUN_00767980` for
/// each of the squad): one below strength five not at all, else a fifth
/// of his salary when often hurt (the knocks' flag 8), three twentieths
/// now and then (4), a tenth else — in thousands at player +0x1f4.
const INSURED_FROM: u8 = 5;
const OFTEN_HURT: u16 = 8;
const NOW_AND_THEN: u16 = 4;

fn insure_squad(records: &mut Records, team: usize) {
    for id in records.teams[team].players() {
        let Some(p) = records.players.get_mut(usize::from(id)).filter(|_| id != 0) else { continue };
        let salary = u32::from(p.salary());
        let cover = if p.strength() < INSURED_FROM {
            0
        } else if p.knocks() & OFTEN_HURT != 0 {
            salary / 5
        } else if p.knocks() & NOW_AND_THEN != 0 {
            salary * 3 / 20
        } else {
            salary / 10
        };
        p.put::<i16>(EXTRA_INSURANCE, cover as i16);
    }
}

fn plan_season(team: &mut Team, second: bool, held: u8, cover: Cover) {
    let manager = team.humans()[0];
    let level = team.level();
    if !planned(team) {
        return;
    }
    let band = level.min(2);
    let flags = team.get::<u16>(FLAGS_AT);
    let cup = if flags & CUP_WINNER_FLAG != 0 { PLAN_CUP_WINNER } else { 0 };
    put::<i32>(team, 0x12c, PLAN_12C[usize::from(band)] + cup);
    let europe = if flags & CHAMPION_FLAG != 0 || held == 1 {
        PLAN_CHAMPION
    } else if flags & EUROPE_FLAG != 0 {
        PLAN_EUROPE
    } else {
        0
    };
    put::<i32>(team, 0x130, europe + if held == 1 || held == 2 { PLAN_HOLDER } else { 0 });
    let place = team.league_place();
    let steps = PLAN_STEP * (3 - i32::from(band));
    put::<i32>(team, 0x19c, steps);
    put::<i32>(team, 0x1a0, steps);
    let grade = match level {
        0 => 4u8.wrapping_sub(place / 10),
        1 => 3u8.wrapping_sub(place / 10),
        _ if place < 10 => 2,
        _ => 1,
    };
    put::<u16>(team, 0x5c4, PLAN_5C4);
    put::<u8>(team, 0x5e0, grade);
    if manager == 0 {
        put::<u8>(team, 0x5e2, u8::from(level == 0 && place < 10));
        put::<u8>(team, 0x5e1, 0);
    }
    if second {
        put::<i32>(team, 0x1b0, PLAN_1B0[usize::from(band)]);
    }
    put::<i32>(team, 0x1b8, PLAN_1B8);
    put::<i32>(team, 0x17c, cover.wages);
    // The sponsors' signing closes with +0x1fc: twenty, or what +0x304
    // holds once set (`FUN_00716a87`'s tail).
    let set = club::<u16>(team, 0x304);
    put::<u16>(team, 0x1fc, if set == 0 { PLAN_1FC } else { set });
    // Before it the sponsor's rate at +0x504 by the level (20 000 in the
    // top three leagues, 10 000 below), which the plan's rest keeps within
    // 5 000 and 50 000 (`DAT_00a3b3ac`, `DAT_00a3b3bc`).
    put::<i32>(team, 0x504, if level < 3 { PLAN_RATE_TOP } else { PLAN_RATE_LOW });
    match level {
        0 => put::<u8>(team, 0x1f8, if team.get::<u8>(CLUB_CLASS) < HIGH_CLASS { 1 } else { 2 }),
        1 => put::<u8>(team, 0x1f8, 1),
        _ => {}
    }
    plan_rest(team, cover);
}

/// What the squad's insurance comes to: the players' own extra
/// insurance in thousands (player +0x1f4, the object's +0xbc;
/// `FUN_0061f9fc`) and the German clubs' compulsory one
/// ([`crate::finances::compulsory_insurance`], `FUN_0061fa79`); beside it
/// the squad's salaries in thousands (`FUN_0062023c`), which the walk
/// plans at +0x17c.
#[derive(Clone, Copy)]
struct Cover {
    extra: i32,
    compulsory: i32,
    wages: i32,
}
const EXTRA_INSURANCE: usize = 0x1f4;
const INSURANCE_UNIT: i32 = 1000;

fn insurance(records: &Records, team: &Team) -> Cover {
    let squad = || team.players().into_iter().filter_map(|id| records.player(id));
    let extra = squad().fold(0i32, |s, p| s.wrapping_add(i32::from(p.get::<i16>(EXTRA_INSURANCE)) * INSURANCE_UNIT));
    let wages = squad().fold(0i32, |s, p| s.wrapping_add(i32::from(p.salary()) * INSURANCE_UNIT));
    Cover { extra, compulsory: crate::finances::compulsory_insurance(records, team), wages }
}

/// The plan's rest copies into the season's block what stands planned
/// (`FUN_00673566`, which `FUN_0042b589` closes with): the grades' sum
/// at +0x1b4 and +0x2bc — `DAT_0089e450`'s row of +0x5e1 in ten
/// thousands and `DAT_0089e458`'s of +0x5e2 in hundred thousands —, the
/// four sums +0x164 to +0x170 at +0x26c to +0x278 and the four bytes
/// +0x1f8 to +0x1fb at +0x300 to +0x303; the insurance at +0x194 — the
/// players' own, or with +0x303 standing the sum at +0x5e4 (and that at
/// +0x29c as well, else nought there), the compulsory one beside —; the
/// running costs at +0x280: the four sums, 80 000 for each hand counted
/// at +0x300 (`DAT_00a3b380`) and +0x27c. (Its sponsors' sums, the drawn parts,
/// +0x27c's own figure (`FUN_00418ce7`) and the human's own are not
/// built.)
const GRADE_TENS: [i32; 4] = [0, 2, 20, 100];
const GRADE_HUNDREDS: [i32; 4] = [0, 2, 10, 30];

const HAND_COST: i32 = 80_000;

fn plan_rest(team: &mut Team, cover: Cover) {
    let row = |table: &[i32; 4], at: usize| table.get(usize::from(club::<u8>(team, at))).copied().unwrap_or(0);
    let grades = row(&GRADE_TENS, 0x5e1) * 10_000 + row(&GRADE_HUNDREDS, 0x5e2) * 100_000;
    put::<i32>(team, 0x1b4, grades);
    put::<i32>(team, 0x2bc, grades);
    for k in 0..4 {
        let v = club::<i32>(team, 0x164 + k * 4);
        put::<i32>(team, 0x26c + k * 4, v);
        let b = club::<u8>(team, 0x1f8 + k);
        put::<u8>(team, 0x300 + k, b);
    }
    let fixed = club::<i32>(team, 0x5e4);
    let (own, kept) = if club::<u8>(team, 0x303) == 0 { (cover.extra, 0) } else { (fixed, fixed) };
    put::<i32>(team, 0x194, own.wrapping_add(cover.compulsory));
    put::<i32>(team, 0x29c, kept);
    let running = (0..4).fold(0i32, |s, k| s.wrapping_add(club::<i32>(team, 0x164 + k * 4)));
    let hands = HAND_COST * i32::from(club::<u8>(team, 0x300));
    let v = running.wrapping_add(hands).wrapping_add(club::<i32>(team, 0x27c));
    put::<i32>(team, 0x280, v);
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
/// +0x5e8 and +0x5ec. (The staff's two salaries follow in `plan_staff`;
/// the other salaries, the stadium's and the youth's sums, its drawn
/// parts and the prices are not built.)
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
/// season after the first draws its lottery below 1000, the first
/// keeps 1. The bank sets the year's rates and the funds' deposits settle at
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

/// The new season as [`open_season`] opens it, when the game's rules are
/// at hand: the kept countries' clubs taking new players where they are
/// thin ([`crate::newplayer::for_kept_club`] by the contract and value
/// tables) and the boards setting every club's goals before the walk plans
/// the clubs by them ([`crate::season::open_goals`], `FUN_00772bee` in
/// `FUN_0065e659`, which comes before `FUN_0042b2fc`).
pub fn open_season_with(records: &mut Records, random: &mut Random, rules: Option<&crate::season::Rules>) {
    let tables = rules.map(|r| (&r.contract, &r.value));
    roll_globals(&mut records.globals, random);
    clear_objects(records);
    // The ranked leagues' clubs by their squads (`FUN_00778394`).
    crate::squad_rank::rank(records);
    crate::squad_rank::keep_scores(records);
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
    open_tactics(records, random);
    set_classes(records);
    open_standings(records, random);
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
    if let Some(rules) = rules {
        crate::season::open_goals(records, rules, random);
    }
    count_down_contracts(records);
    number_shirts(records, random);
    crate::finances::keep_last_season(records);
    clear_human_season(records);
    clear_bookings(records, random);
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

/// The played clubs' staff at the season's end (`FUN_00624e4f`'s
/// `FUN_006247a0`, `FUN_00624806` for each club): its manager-trainer,
/// trainer and manager (team +0x9c, +0x9e, +0xa0), each with two
/// contract years or more, a year less (the person's +0x2c), a season
/// more at the club (+0x34) and his +0x36 halved — the new season then
/// holds every person's +0x36 at nought, by a step not traced. (One with
/// less: the original keeps him by chance — the new years it draws then
/// go unsaved — or lets him go, the worse the club fell short of its
/// goal, and the new season's staff market (`FUN_00409a3f`) fills the
/// post; neither is built, he stays.)
const STAFF_YEARS: usize = 0x2c;
const STAFF_SEASONS: usize = 0x34;
const STAFF_HALVED: usize = 0x36;
const LAST_PERSON: u16 = 0x5db;

fn close_staff(records: &mut Records) {
    for country in crate::opponent::played_nations(records) {
        for number in 0..records.countries.team_count(usize::from(country)).max(0) as u16 {
            let Some(team) = records.team(country, number) else { continue };
            for id in team.staff() {
                if id == 0 || id > LAST_PERSON {
                    continue;
                }
                let Some(p) = records.persons.get_mut(usize::from(id)) else { continue };
                let years = p.get::<u8>(STAFF_YEARS);
                if years < 2 {
                    continue;
                }
                p.put::<u8>(STAFF_YEARS, years - 1);
                p.put::<u8>(STAFF_SEASONS, p.get::<u8>(STAFF_SEASONS).wrapping_add(1));
                p.put::<i16>(STAFF_HALVED, p.get::<i16>(STAFF_HALVED) / 2);
            }
        }
    }
}

/// The season's end for the human managers (`FUN_00624e4f`'s
/// `FUN_006fd282` for each): a year more as a trainer; in the game's mode
/// 9, his standing by his marks' average over the season — down three
/// below 60, two below 70, one below 80, one even chance in two below 85,
/// up one even chance in two from 98 while it stands below 70; the
/// played clubs' staff first ([`close_staff`]). (The rest of
/// `FUN_00624e4f` — the comparison's marks, the staff's endings — is not
/// built.)
pub fn close_managers(records: &mut Records, random: &mut Random) {
    close_staff(records);
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
/// flags and his entries' flags cleared; then the humans' statistics of
/// their last match cleared. (The career's counts by the
/// titles of the last seasons, the training plan's reset and the rest of
/// `FUN_00775219` are not built.)
pub fn open_managers(records: &mut Records, random: &mut Random) {
    for n in 0..usize::from(records.globals.humans()) {
        let Some(m) = records.managers.get(n) else { continue };
        let (country, number) = (m.country(), m.team());
        let cash = Some(number)
            .filter(|_| number > 0 || country > 0)
            .and_then(|t| records.team(country, t))
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
    // Last, the four humans' statistics of their last match go
    // (`FUN_007cff4b` on `0x1c26dd8` + slot × 0x2058).
    for slot in 0..LAST_MATCH_SLOTS {
        if let Some(b) = records.block_mut(&format!("{LAST_MATCH}[{slot}]")) {
            b.fill(0);
        }
    }
}

/// The humans' statistics of their last match, by slot.
const LAST_MATCH: &str = "0x1c26dd8";
const LAST_MATCH_SLOTS: usize = 4;

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

/// What each played club's standing opens the new season at, beside its
/// class (`FUN_00772bee`, its tactics set before by [`open_tactics`]): the respect it is held in (club +0x92, team
/// +0xb6) five for each club below it in its league and thirty — a club in
/// no league counted as of 24 —, ten more by a byte at +0x134 of 16 and
/// up, ten less below 5; the mood (club +0x9e, team +0xc4) 80 and a draw
/// below 41; then by the season just played: the champion 30 more of both,
/// the runner-up 20, a UEFA cup entrant 10, the promoted 20 more mood but
/// 30 less respect and their will ten up (team +0x95a, within ±200), the
/// relegated 30 less mood, 10 more respect, the will ten down — the first
/// of these that holds —, and the cup winner 20 more mood and 10 more
/// respect besides. Last a draw below 100 at club +0xdf0 (team +0x302).
/// The club's tactics (`FUN_00698598` … `FUN_00698bde`): five fields of
/// three bits in club +0xa2 (team +0xc8) and the bytes +0x9f and +0xa5
/// (team +0xc5, +0xcb), each 0 to 5; the new season sets them all to 5
/// less a fifth of the club's place, one more two times in three.
const TACTICS_TOP: i32 = 5;
const TACTICS_PLACES: i32 = 5;
const TACTICS_AT: usize = 0xc8;
const TACTIC_FIELDS: u16 = 5;
const TACTIC_BITS: u16 = 0x7fff;
const TACTIC_BYTES: [usize; 2] = [0xc5, 0xcb];
const RESPECT_FLOOR: i32 = 30;
/// The respect stays within nought and 200, as every change of it keeps
/// it (`FUN_00714849`).
const RESPECT_MOST: i32 = 200;
const RESPECT_STEP: i32 = 5;
const NO_LEAGUE_CLUBS: i32 = 24;
const MOOD_BASE: u8 = 80;
const MOOD_DRAW: u32 = 41;
const SEASON_FLAGS: usize = 0x110;
/// The season's marks in turn: the flag, the mood's and the respect's
/// change, the will's.
const SEASON_STANDINGS: [(u16, i8, i16, i16); 5] =
    [(1, 30, 30, 0), (2, 20, 20, 0), (0x10, 10, 10, 0), (4, 20, -30, 10), (8, -30, 10, -10)];
const CUP_WON: u16 = 0x1000;
const WILL_SHIFT: usize = 0x95a;
const WILL_MOST: i16 = 200;
const SEASON_DRAW: usize = 0x302;
/// The club's worth as the season starts (club +0x218).
const SEASON_START_WORTH: usize = 0x218;
/// Two bytes of past seasons (club +0xee, +0xef), and the season's
/// honours the second takes a bit for: the title, second place, the UEFA
/// cup, the cup.
const PAST_SEASONS: [usize; 2] = [0x11e, 0x11f];
const HONOURS: u16 = 1 | 2 | 0x10 | 0x1000;
/// The club's situation for the season (club +0xed, team +0x11d): 2 after
/// three marks in the past seasons' byte, 3 after five seasons of
/// honours, 6 or 7 when its last season fell more than three places short
/// of its goal or went more than three past it (team +0x137), else 1
/// where its stadium's byte +0x67 (its block's data) stands at nought,
/// and 0.
const SITUATION_AT: usize = 0x11d;
const VERDICT_AT: usize = 0x137;
const STADIUMS: &str = "0x1a48138";
const STADIUM_OWN: usize = 0x67;

/// The tactics every played club opens the season with, set before its
/// class — a club gone down to no league still by the place it last held.
pub(crate) fn open_tactics(records: &mut Records, random: &mut Random) {
    for country in crate::opponent::played_nations(records) {
        for number in 0..records.countries.team_count(usize::from(country)).max(0) as u16 {
            let Some(team) = records.team_mut(country, number) else { continue };
            let mut setting = TACTICS_TOP - i32::from(team.league_place()) / TACTICS_PLACES;
            if random.below(3) != 0 {
                setting += 1;
            }
            let setting = setting.clamp(0, TACTICS_TOP) as u16;
            let fields = (0..TACTIC_FIELDS).fold(0u16, |v, k| v | setting << (3 * k));
            team.put::<u16>(TACTICS_AT, team.get::<u16>(TACTICS_AT) & !TACTIC_BITS | fields);
            for at in TACTIC_BYTES {
                team.put::<u8>(at, setting as u8);
            }
        }
    }
}

/// The club's situation for the season (`FUN_0078f841`), its past
/// seasons' bytes as they stand moved on.
fn situation(team: &Team, stadium: Option<u8>) -> u8 {
    let past = team.get::<u8>(PAST_SEASONS[0]);
    let marks = (past & 3) + (past >> 2 & 3) + (past >> 4 & 3);
    let verdict = team.get::<i8>(VERDICT_AT);
    if marks >= 3 {
        2
    } else if team.get::<u8>(PAST_SEASONS[1]) & 0x1f == 0x1f {
        3
    } else if verdict < -3 {
        6
    } else if verdict > 3 {
        7
    } else {
        u8::from(stadium == Some(0))
    }
}

pub(crate) fn open_standings(records: &mut Records, random: &mut Random) {
    for country in crate::opponent::played_nations(records) {
        for number in 0..records.countries.team_count(usize::from(country)).max(0) as u16 {
            let Some(team) = records.team(country, number) else { continue };
            let level = team.level();
            let clubs = if level < 10 {
                crate::opponent::League::of(records, country, level).map_or(NO_LEAGUE_CLUBS, |l| l.teams() as i32)
            } else {
                NO_LEAGUE_CLUBS
            };
            let prices = records.globals.fund_prices();
            let index = usize::try_from(records.countries.first_team(usize::from(country))).unwrap_or(0) + usize::from(number);
            let stadium = records.block(&format!("{STADIUMS}[{index}]")).and_then(|b| b.get(STADIUM_OWN).copied());
            let Some(team) = records.team_mut(country, number) else { continue };
            // What the club is worth as the season starts: its cash, less
            // its loans, with its deposits (`FUN_00621f0d`).
            let worth = club::<i64>(team, CASH)
                .wrapping_sub(crate::finances::loans(team))
                .wrapping_add(crate::finances::deposits(team, prices));
            put::<i64>(team, SEASON_START_WORTH, worth);
            let mut respect = (clubs - i32::from(team.league_place())) * RESPECT_STEP + RESPECT_FLOOR;
            let forecast = team.get::<u8>(NO_LEAGUE_FORECAST);
            if forecast >= 16 {
                respect += 10;
            } else if forecast < 5 {
                respect -= 10;
            }
            let mut mood = (random.below(MOOD_DRAW) as u8).wrapping_add(MOOD_BASE);
            let flags = team.get::<u16>(SEASON_FLAGS);
            if let Some(&(_, m, r, will)) = SEASON_STANDINGS.iter().find(|s| flags & s.0 != 0) {
                mood = mood.wrapping_add(m as u8);
                respect += i32::from(r);
                if will != 0 {
                    let w = team.get::<i16>(WILL_SHIFT).saturating_add(will).clamp(-WILL_MOST, WILL_MOST);
                    team.put::<i16>(WILL_SHIFT, w);
                }
            }
            if flags & CUP_WON != 0 {
                mood = mood.wrapping_add(20);
                respect += 10;
            }
            team.set_respect(respect.clamp(0, RESPECT_MOST) as i16);
            team.set_mood(mood);
            team.put::<u8>(SEASON_DRAW, random.below(100) as u8);
            // The seasons' marks move on: one byte two places, the other
            // one, taking a bit for a season of honours.
            team.put::<u8>(PAST_SEASONS[0], team.get::<u8>(PAST_SEASONS[0]) << 2);
            let honours = u8::from(flags & HONOURS != 0);
            team.put::<u8>(PAST_SEASONS[1], team.get::<u8>(PAST_SEASONS[1]) << 1 | honours);
            team.put::<u8>(SITUATION_AT, situation(team, stadium));
        }
    }
}

/// The training flags' team meeting before the next match.
const TEAM_MEETING: u32 = 0x1000;
/// The season's flags of a club promoted or relegated, and what
/// `FUN_0069da17` puts back for it: four words to nought, two bytes to
/// 0xff.
const MOVED: u16 = 4 | 8;
const MOVED_WORDS: [usize; 4] = [0xfe, 0x100, 0x102, 0x104];
const MOVED_BYTES: [usize; 2] = [0x106, 0x107];
/// The human clubs' shirt sales, by slot (`DAT_00a51ea8`).
const SHIRT_SALES: &str = "0xa51ea8";

fn clear_club_seasons(records: &mut Records) {
    let second_season = records.globals.seasons() > 1;
    for country in crate::opponent::played_nations(records) {
        for number in 0..records.countries.team_count(usize::from(country)).max(0) as u16 {
            let Some(team) = records.team_mut(country, number) else { continue };
            // A club gone up or down starts afresh at +0xfe … +0x107
            // (`FUN_0069da17`).
            if team.get::<u16>(SEASON_FLAGS) & MOVED != 0 {
                for at in MOVED_WORDS {
                    team.put::<u16>(at, 0);
                }
                for at in MOVED_BYTES {
                    team.put::<u8>(at, 0xff);
                }
            }
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
            // A team meeting stands planned before its first match
            // (`FUN_00515df1`: club +0xdf8's 0x1000; the original plans it
            // at every club, the computer clubs' gone by the season's
            // first day by a step not traced).
            if let Some(team) = records.team_mut(country, number) {
                team.set_training_flags(team.training_flags() | TEAM_MEETING);
            }
            // The shirts its players sold start again from none
            // (`FUN_00699351`, emptied, `FUN_006993e0`).
            if let Some(b) = records.block_mut(&format!("{SHIRT_SALES}[{}]", human - 1)) {
                b.fill(0);
            }
            let Some(extra) = records.extras.get_mut(human - 1) else { continue };
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
        // The season's flags as the original's turn set them.
        let mut moved = 0;
        for country in crate::opponent::played_nations(&theirs) {
            for number in 0..theirs.countries.team_count(usize::from(country)).max(0) as u16 {
                let flags = theirs.team(country, number).unwrap().get::<u16>(SEASON_FLAGS);
                moved += i32::from(flags & MOVED != 0);
                ours.team_mut(country, number).unwrap().put::<u16>(SEASON_FLAGS, flags);
            }
        }
        assert!(moved > 20, "{moved} clubs moved");
        clear_club_seasons(&mut ours);
        // The clubs gone up or down start afresh at +0xfe … +0x107 as the
        // original's (their flags as the original's turn set them).
        for country in crate::opponent::played_nations(&theirs) {
            for number in 0..theirs.countries.team_count(usize::from(country)).max(0) as u16 {
                let y = theirs.team(country, number).unwrap();
                if y.get::<u16>(SEASON_FLAGS) & MOVED == 0 {
                    continue;
                }
                let x = ours.team(country, number).unwrap();
                if x.get::<u16>(SEASON_FLAGS) & MOVED == 0 {
                    continue;
                }
                for at in MOVED_WORDS {
                    assert_eq!(x.get::<u16>(at), y.get::<u16>(at), "{country}/{number} +{at:#x}");
                }
                for at in MOVED_BYTES {
                    assert_eq!(x.get::<u8>(at), y.get::<u8>(at), "{country}/{number} +{at:#x}");
                }
            }
        }
        // A team meeting planned at the human club alone, as the
        // original's first day has it.
        for country in crate::opponent::played_nations(&theirs) {
            for number in 0..theirs.countries.team_count(usize::from(country)).max(0) as u16 {
                let (x, y) = (ours.team(country, number).unwrap(), theirs.team(country, number).unwrap());
                assert_eq!(x.training_flags() & TEAM_MEETING, y.training_flags() & TEAM_MEETING, "{country}/{number}");
            }
        }
        // The human club's shirt sales stand emptied as the original's.
        for slot in 0..4 {
            let name = format!("{SHIRT_SALES}[{slot}]");
            assert_eq!(ours.block(&name), theirs.block(&name), "{name}");
        }
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
        clear_bookings(&mut closed, &mut Random::new(1));
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

    /// From the original's NewSeason000 — its clubs in their new leagues
    /// and places, the UEFA cup's mark (set after) taken off — the respect
    /// every played club opens the season at comes out as the original's,
    /// the mood and the tactics within their draws, its worth at the
    /// season's start but at the four amateurs moved up, and its situation
    /// at every club.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_clubs_open_the_season_with_the_originals_respect() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let read = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs) = (read("NewSeason000.an3"), read("NewSeason000.an3"));
        let played = crate::opponent::played_nations(&theirs);
        for &c in &played {
            for t in 0..ours.countries.team_count(usize::from(c)).max(0) as u16 {
                let team = ours.team_mut(c, t).unwrap();
                team.put::<u16>(SEASON_FLAGS, team.get::<u16>(SEASON_FLAGS) & !0x10);
                team.set_respect(-1);
            }
        }
        open_tactics(&mut ours, &mut Random::new(2));
        open_standings(&mut ours, &mut Random::new(2));
        let (mut same, mut all, mut worth) = (0, 0, 0);
        for &c in &played {
            for t in 0..ours.countries.team_count(usize::from(c)).max(0) as u16 {
                let (x, y) = (ours.team(c, t).unwrap(), theirs.team(c, t).unwrap());
                all += 1;
                same += i32::from(x.respect() == y.respect());
                worth += i32::from(club::<i64>(x, SEASON_START_WORTH) == club::<i64>(y, SEASON_START_WORTH));
                // The tactics: one setting throughout, as the original's or
                // one off by the draw.
                let (mine, theirs) = (x.get::<u8>(TACTIC_BYTES[0]), y.get::<u8>(TACTIC_BYTES[0]));
                // (A club in no league had its place put to nought after.)
                assert!(y.level() == 10 || mine.abs_diff(theirs) <= 1, "{c}/{t}: {mine} {theirs}");
                let fields = (0..TACTIC_FIELDS).fold(0u16, |v, k| v | u16::from(mine) << (3 * k));
                assert_eq!((x.get::<u16>(TACTICS_AT) & TACTIC_BITS, x.get::<u8>(TACTIC_BYTES[1])), (fields, mine), "{c}/{t}");
                // The past seasons' bytes move on from where the original's
                // stand, the second taking the season's honours.
                let honours = u8::from(x.get::<u16>(SEASON_FLAGS) & HONOURS != 0);
                assert_eq!(x.get::<u8>(PAST_SEASONS[0]), y.get::<u8>(PAST_SEASONS[0]) << 2, "{c}/{t}");
                assert_eq!(x.get::<u8>(PAST_SEASONS[1]), y.get::<u8>(PAST_SEASONS[1]) << 1 | honours, "{c}/{t}");
                let mood = i32::from(x.mood());
                assert!((MOOD_BASE as i32 - 30..=MOOD_BASE as i32 + 40 + 50).contains(&mood), "{c}/{t}: mood {mood}");
            }
        }
        assert!(all > 200 && same == all, "{same} of {all}");
        // The worth at the season's start too, but at the amateurs moved up
        // whose cash the original sets after it.
        assert!(worth + 4 >= all, "worth {worth} of {all}");
        // The situation from the original's own bytes, as they stood when
        // it was set, comes out as the original's at every club.
        for &c in &played {
            for t in 0..theirs.countries.team_count(usize::from(c)).max(0) as u16 {
                let y = theirs.team(c, t).unwrap();
                let index = usize::try_from(theirs.countries.first_team(usize::from(c))).unwrap_or(0) + usize::from(t);
                let stadium = theirs.block(&format!("{STADIUMS}[{index}]")).and_then(|b| b.get(STADIUM_OWN).copied());
                assert_eq!(situation(y, stadium), y.get::<u8>(SITUATION_AT), "{c}/{t}");
            }
        }
    }

    /// From SeasonEnd364 the staff's contracts of two years and more run a
    /// year down as the original's NewSeason000 has them, their seasons at
    /// the club one more, and no other person's years move.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_staffs_contracts_run_down_as_the_original() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let read = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs, before) = (read("SeasonEnd364.an3"), read("NewSeason000.an3"), read("SeasonEnd364.an3"));
        close_staff(&mut ours);
        let mut moved = 0;
        for id in 1..usize::from(LAST_PERSON) {
            let (Some(x), Some(y), Some(z)) = (ours.persons.get(id), theirs.persons.get(id), before.persons.get(id)) else { continue };
            if x.get::<u8>(STAFF_YEARS) != z.get::<u8>(STAFF_YEARS) {
                moved += 1;
                assert_eq!(x.get::<u8>(STAFF_YEARS), y.get::<u8>(STAFF_YEARS), "person {id}'s years");
                assert_eq!(x.get::<u8>(STAFF_SEASONS), y.get::<u8>(STAFF_SEASONS), "person {id}'s seasons");
            }
        }
        assert!(moved > 250, "{moved}");
    }

    /// The new season's managers' year leaves the humans' last-match
    /// statistics empty, as the original's NewSeason000 has them.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_last_matchs_statistics_go_with_the_season() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let read = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs) = (read("SeasonEnd364.an3"), read("NewSeason000.an3"));
        let name = format!("{LAST_MATCH}[0]");
        assert!(ours.block(&name).unwrap().iter().any(|&b| b != 0));
        open_managers(&mut ours, &mut Random::new(1));
        for slot in 0..LAST_MATCH_SLOTS {
            let name = format!("{LAST_MATCH}[{slot}]");
            assert_eq!(ours.block(&name), theirs.block(&name), "{name}");
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

    /// The contracts count down a year at the new season and a moved club
    /// loses them all, measured on the human club of the pair — its boards'
    /// package and its television's run out from one year to none as the
    /// original's do (the main sponsor the original then fills again with
    /// its fallback contract, not built) — and on two clubs set up here.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_contracts_count_down_as_the_original() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let read = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs) = (read("SeasonEnd364.an3"), read("NewSeason000.an3"));
        for i in 0..ours.teams.len() {
            let flags = theirs.teams[i].get::<u16>(SEASON_FLAGS) & MOVED;
            let team = &mut ours.teams[i];
            team.put::<u16>(SEASON_FLAGS, team.get::<u16>(SEASON_FLAGS) & !MOVED | flags);
        }
        let slot = ours.human_slot(0).unwrap();
        let (c, t) = (ours.managers[slot].country() as u16, ours.managers[slot].team() as u16);
        let before = CONTRACT_YEARS.map(|at| club::<u8>(ours.team(c, t).unwrap(), at));
        count_down_contracts(&mut ours);
        let (x, y) = (ours.team(c, t).unwrap(), theirs.team(c, t).unwrap());
        for (k, &at) in CONTRACT_YEARS.iter().enumerate().skip(1) {
            assert_eq!(club::<u8>(x, at), club::<u8>(y, at), "{c}/{t} +{at:#x} was {}", before[k]);
        }
        assert_eq!(club::<u8>(x, CONTRACT_YEARS[0]), before[0].saturating_sub(1));
        assert!(before[1..].iter().any(|&v| v > 0), "a contract to count down");
        for i in 0..ours.teams.len() {
            for k in 0..BOARDS {
                let at = BOARD_YEARS + k * BOARD_STRIDE;
                assert_eq!(club::<u8>(&ours.teams[i], at), club::<u8>(&theirs.teams[i], at), "{i} board {k}");
            }
        }
        // A club staying counts down, one moved loses all.
        let i = ours.team_index(c, t).unwrap();
        for flag in [0u16, 4] {
            let mut team = ours.teams[i].clone();
            team.put::<u16>(SEASON_FLAGS, team.get::<u16>(SEASON_FLAGS) & !MOVED | flag);
            put::<u8>(&mut team, 0x509, 3);
            put::<u8>(&mut team, 0x5a1, 0);
            put::<u8>(&mut team, BOARD_YEARS + 5 * BOARD_STRIDE, 2);
            ours.teams[i] = team;
            count_down_contracts(&mut ours);
            let team = &ours.teams[i];
            let years = if flag == 0 { [2, 0, 1] } else { [0, 0, 0] };
            assert_eq!(
                [club::<u8>(team, 0x509), club::<u8>(team, 0x5a1), club::<u8>(team, BOARD_YEARS + 5 * BOARD_STRIDE)],
                years,
                "flag {flag}"
            );
        }
    }

    /// The planned clubs' new-season fields come out as the original's:
    /// on its first new-season day every club below the tenth level but
    /// the human's, with the fields put to nought, is planned back to the
    /// original's values from its league, place and class; the human's and
    /// the amateurs' stay as they were.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_planned_clubs_open_the_season_as_the_original() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let read = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs) = (read("NewSeason000.an3"), read("NewSeason000.an3"));
        let words = [0x19c, 0x1a0, 0x1b0, 0x1b8, 0x12c, 0x130, 0x1b4, 0x2bc, 0x26c, 0x270, 0x274, 0x278, 0x194, 0x29c, 0x280, 0x17c, 0x504];
        let bytes = [0x5e0, 0x5e1, 0x5e2, 0x1f8, 0x300, 0x301, 0x302, 0x303];
        for team in ours.teams.iter_mut() {
            for at in words {
                put::<i32>(team, at, 0);
            }
            for at in bytes {
                put::<u8>(team, at, 0);
            }
            put::<u16>(team, 0x5c4, 0);
            put::<u16>(team, 0x1fc, 0);
        }
        let (mut count, mut kept) = (0, 0);
        for i in 0..ours.teams.len() {
            let country = i16::from(ours.teams[i].country());
            if country == 0 || !ours.globals.played(country) {
                continue;
            }
            let holders = theirs.block(HOLDERS).unwrap().to_vec();
            let first = theirs.countries.first_team(usize::from(ours.teams[i].country())) as usize;
            let held = holder(&holders, ours.teams[i].country(), i - first);
            if planned(&ours.teams[i]) {
                for id in ours.teams[i].players() {
                    if let Some(p) = ours.players.get_mut(usize::from(id)).filter(|_| id != 0) {
                        p.put::<i16>(EXTRA_INSURANCE, -1);
                    }
                }
                insure_squad(&mut ours, i);
                for id in ours.teams[i].players().into_iter().filter(|&id| id != 0) {
                    let (x, y) = (ours.player(id).unwrap(), theirs.player(id).unwrap());
                    assert_eq!(x.get::<i16>(EXTRA_INSURANCE), y.get::<i16>(EXTRA_INSURANCE), "{i} player {id}");
                }
            }
            let cover = insurance(&ours, &ours.teams[i]);
            plan_season(&mut ours.teams[i], true, held, cover);
            let (x, y) = (&ours.teams[i], &theirs.teams[i]);
            if y.humans()[1] != 0 || y.level() >= 10 {
                assert_eq!(club::<i32>(x, 0x19c), 0, "{i} not planned");
                kept += 1;
                continue;
            }
            for at in words {
                assert_eq!(club::<i32>(x, at), club::<i32>(y, at), "{i} +{at:#x}");
            }
            for at in bytes {
                assert_eq!(club::<u8>(x, at), club::<u8>(y, at), "{i} +{at:#x}");
            }
            assert_eq!(club::<u16>(x, 0x5c4), club::<u16>(y, 0x5c4), "{i} +0x5c4");
            assert_eq!(club::<u16>(x, 0x1fc), club::<u16>(y, 0x1fc), "{i} +0x1fc");
            count += 1;
        }
        assert!(count > 140 && kept > 0, "{count} planned, {kept} kept");
    }

    /// The staff's salaries for the season come out as the original's on
    /// its first new-season day: every computer club's trainer at +0x158, its
    /// manager-trainer or manager at +0x154, its co-trainer at +0x15c and
    /// its one more at +0x160 — the whole year's in July —, and the grade's
    /// sum at +0x1ac by the grade the season's end left.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_staffs_salaries_are_planned_as_the_original() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let read = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs) = (read("NewSeason000.an3"), read("NewSeason000.an3"));
        assert_eq!(ours.globals.get::<i16>(MONTH_AT), 7, "July");
        let mut count = 0;
        for i in 0..ours.teams.len() {
            let country = i16::from(ours.teams[i].country());
            if country == 0 || !ours.globals.played(country) || ours.teams[i].humans() != [0, 0] {
                continue;
            }
            for at in [0x154, 0x158, 0x15c, 0x160] {
                put::<i32>(&mut ours.teams[i], at, -1);
            }
            plan_staff(&mut ours, i);
            for at in [0x154, 0x158, 0x15c, 0x160] {
                assert_eq!(club::<i32>(&ours.teams[i], at), club::<i32>(&theirs.teams[i], at), "{i} +{at:#x}");
            }
            count += 1;
        }
        assert!(count > 200, "{count}");
        // The grade's sum by the grade held at the season's end.
        let mut old = read("SeasonEnd364.an3");
        for i in 0..old.teams.len() {
            let country = i16::from(old.teams[i].country());
            if country == 0 || !old.globals.played(country) || old.teams[i].humans() != [0, 0] {
                continue;
            }
            plan_staff(&mut old, i);
            assert_eq!(club::<i32>(&old.teams[i], 0x1ac), club::<i32>(&theirs.teams[i], 0x1ac), "{i} +0x1ac");
        }
    }

    /// The walk's drawn sums come out as one of the original's draws: on
    /// its first new-season day every planned club's +0x18c, +0x1a8,
    /// +0x1a4, their copies, +0x4fc and +0x500 are reproduced by some draw.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_drawn_sums_are_one_of_the_originals_draws() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let theirs = Records::from_save(Save::read(&keeps.join("NewSeason000.an3")).unwrap()).unwrap();
        let fields = [0x18c, 0x1a8, 0x1a4, 0x294, 0x2b0, 0x2ac, 0x500];
        let (mut count, mut shared) = (0, 0);
        for i in 0..theirs.teams.len() {
            let country = i16::from(theirs.teams[i].country());
            if country == 0 || !theirs.globals.played(country) || !planned(&theirs.teams[i]) {
                continue;
            }
            let skill = manager_skill(&theirs, &theirs.teams[i]);
            let want = fields.map(|at| club::<i32>(&theirs.teams[i], at));
            let fifths = club::<u16>(&theirs.teams[i], 0x4fc);
            let found = (0..200).any(|seed| {
                let mut team = theirs.teams[i].clone();
                plan_drawn(&mut team, skill, &mut Random::new(seed));
                fields.map(|at| club::<i32>(&team, at)) == want && club::<u16>(&team, 0x4fc) == fifths
            });
            assert!(found, "{i}: {want:?}");
            count += 1;
            shared += i32::from(want[0] == 0);
        }
        assert!(count > 140 && shared > 0, "{count} planned, {shared} sharing");
    }

    /// The main sponsor's number is one of the original's draws: on its
    /// first new-season day every planned club's +0x4ea is reproduced by a
    /// draw among its cup system's forty, its +0x4ec by a draw over the
    /// plan's figure, its boards' package by a draw of its row, its
    /// television contract by a draw of its league's and its shirt's —
    /// signed anew where the countdown left none, else kept a year shorter.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_sponsors_number_is_one_of_the_originals_draws() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let theirs = Records::from_save(Save::read(&keeps.join("NewSeason000.an3")).unwrap()).unwrap();
        let old = Records::from_save(Save::read(&keeps.join("SeasonEnd364.an3")).unwrap()).unwrap();
        let (mut count, mut signed, mut kept) = (0, 0, 0);
        for i in 0..theirs.teams.len() {
            let country = i16::from(theirs.teams[i].country());
            if country == 0 || !theirs.globals.played(country) || !planned(&theirs.teams[i]) {
                continue;
            }
            let system = usize::from(theirs.countries.cup_system(usize::from(theirs.teams[i].country())));
            let want = club::<u16>(&theirs.teams[i], 0x4ea);
            let found = (0..400).any(|seed| {
                let mut team = theirs.teams[i].clone();
                number_sponsor(&mut team, system, &mut Random::new(seed));
                club::<u16>(&team, 0x4ea) == want
            });
            assert!(found, "{i}: {want} of system {system}");
            let figure = club::<i32>(&theirs.teams[i], 0x4ec);
            let found = (0..400).any(|seed| {
                let mut team = theirs.teams[i].clone();
                plan_price(&mut team, system, &mut Random::new(seed));
                club::<i32>(&team, 0x4ec) == figure
            });
            assert!(found, "{i}: +0x4ec {figure}, base {}", price_base(&theirs.teams[i], system));
            let boards = [0x5a4, 0x1f0, 0x2f8].map(|at| club::<i32>(&theirs.teams[i], at));
            let bytes = [0x5a0, 0x5a1].map(|at| club::<u8>(&theirs.teams[i], at));
            let found = (0..400).any(|seed| {
                let mut team = theirs.teams[i].clone();
                sign_boards(&mut team, system, &mut Random::new(seed));
                [0x5a4, 0x1f0, 0x2f8].map(|at| club::<i32>(&team, at)) == boards
                    && [0x5a0, 0x5a1].map(|at| club::<u8>(&team, at)) == bytes
            });
            assert!(found, "{i}: boards {boards:?} {bytes:?}");
            let tv = [0x5b4, 0x1f4, 0x2fc].map(|at| club::<i32>(&theirs.teams[i], at));
            let tv_bytes = [0x5b0, 0x5b1].map(|at| club::<u8>(&theirs.teams[i], at));
            let found = (0..4000).any(|seed| {
                let mut team = theirs.teams[i].clone();
                sign_television(&mut team, system, &mut Random::new(seed));
                [0x5b4, 0x1f4, 0x2fc].map(|at| club::<i32>(&team, at)) == tv
                    && [0x5b0, 0x5b1].map(|at| club::<u8>(&team, at)) == tv_bytes
            });
            assert!(found, "{i}: television {tv:?} {tv_bytes:?}");
            // The shirt: a club the countdown left none signs anew, one
            // with years left keeps its contract a year shorter.
            let was = |at: usize| club::<u8>(&old.teams[i], at);
            let moved = theirs.teams[i].get::<u16>(SEASON_FLAGS) & MOVED != 0;
            let shirt = [0x5ac, 0x204, 0x30c].map(|at| club::<i32>(&theirs.teams[i], at));
            let shirt_bytes = [0x5a8, 0x5a9].map(|at| club::<u8>(&theirs.teams[i], at));
            if moved || was(0x5a9) <= 1 {
                let found = (0..4000).any(|seed| {
                    let mut team = theirs.teams[i].clone();
                    put::<u8>(&mut team, 0x5a9, 0);
                    sign_shirt(&mut team, system, &mut Random::new(seed));
                    [0x5ac, 0x204, 0x30c].map(|at| club::<i32>(&team, at)) == shirt
                        && [0x5a8, 0x5a9].map(|at| club::<u8>(&team, at)) == shirt_bytes
                });
                assert!(found, "{i}: shirt {shirt:?} {shirt_bytes:?}");
                signed += 1;
            } else {
                assert_eq!(shirt_bytes[1], was(0x5a9) - 1, "{i} shirt's years");
                assert_eq!(shirt, [club::<i32>(&old.teams[i], 0x5ac); 3], "{i} shirt kept");
                kept += 1;
            }
            count += 1;
        }
        assert!(count > 140 && signed > 0 && kept > 0, "{count}: {signed} shirts signed, {kept} kept");
    }

    /// The new season's shirt numbers: from the season-end save every
    /// club's players come out with a number, and the
    /// original's turn agrees for every player it numbers the same club
    /// and position by, where no draw was needed.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_shirts_are_numbered_as_the_original() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let read = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs) = (read("SeasonEnd364.an3"), read("NewSeason000.an3"));
        let before: Vec<u8> = ours.players.iter().map(|p| p.get::<u8>(0x223)).collect();
        number_shirts(&mut ours, &mut Random::new(5));
        let (mut kept, mut same) = (0, 0);
        for i in 0..ours.teams.len() {
            let country = i16::from(ours.teams[i].country());
            if country == 0 || !ours.globals.played(country) {
                continue;
            }
            let squad: Vec<usize> = ours.teams[i].players().into_iter().filter(|&id| id != 0).map(usize::from).collect();
            for &id in &squad {
                let n = ours.players[id].get::<u8>(0x223);
                // (A number held further down the order is marked only when
                // the order reaches it, so the original may hand it out
                // twice; every player has one.)
                assert!(n != 0, "team {i} player {id}");
                // A player whose number stood alone keeps it, here as there
                // when the original kept him at the club.
                let alone = before[id] != 0 && squad.iter().filter(|&&o| before[o] == before[id]).count() == 1;
                if alone && theirs.teams[i].players().contains(&(id as u16)) {
                    kept += 1;
                    assert_eq!(n, before[id], "team {i} player {id}");
                    same += i32::from(theirs.players[id].get::<u8>(0x223) == n);
                }
            }
        }
        assert!(kept > 1000 && same * 100 >= kept * 95, "{same} of {kept} kept as the original");
    }

    /// The team bonuses come out as the original's: on its first
    /// new-season day every computer club, the fields put to nought, is
    /// planned back to its rows and bonuses, and its season's expected
    /// bonuses with them; an amateur club its players' pay carried on and
    /// its stadium sum by one of its draws.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_team_bonuses_are_planned_as_the_original() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let theirs = Records::from_save(Save::read(&keeps.join("NewSeason000.an3")).unwrap()).unwrap();
        let last = Records::from_save(Save::read(&keeps.join("SeasonEnd364.an3")).unwrap()).unwrap();
        let mut count = 0;
        for i in 0..theirs.teams.len() {
            let country = i16::from(theirs.teams[i].country());
            if country == 0 || !theirs.globals.played(country) || theirs.teams[i].humans()[1] != 0 {
                continue;
            }
            let system = usize::from(theirs.countries.cup_system(usize::from(theirs.teams[i].country())));
            let mut team = theirs.teams[i].clone();
            for k in 0..5 {
                put::<u16>(&mut team, BONUS_ROWS_AT + 2 * k, 0);
            }
            for at in [TITLE_BONUS, EUROPE_BONUS, PROMOTED_BONUS, LAST_BONUS] {
                put::<i32>(&mut team, at, -1);
            }
            if team.level() >= 10 {
                assert_eq!(players_carried_on(&theirs, &team), club::<i32>(&theirs.teams[i], 0x17c), "{i} players carried on");
                let skill = manager_skill(&theirs, &team);
                let want = club::<i32>(&theirs.teams[i], 0x1b0);
                let found = (0..400).any(|seed| {
                    let mut t = team.clone();
                    plan_amateur_stadium(&mut t, skill, &mut Random::new(seed));
                    club::<i32>(&t, 0x1b0) == want
                });
                assert!(found, "{i}: amateur stadium {want}, skill {skill}");
            }
            plan_bonuses(&mut team, system);
            // The expected bonuses by the +0x5c4 the season's end left.
            let old = club::<u16>(&last.teams[i], 0x5c4);
            put::<u16>(&mut team, 0x5c4, old);
            assert_eq!(expected_bonuses(&theirs, &team), club::<i32>(&theirs.teams[i], EXPECTED_BONUSES), "{i} expected bonuses");
            for k in 0..5 {
                let at = BONUS_ROWS_AT + 2 * k;
                assert_eq!(club::<u16>(&team, at), club::<u16>(&theirs.teams[i], at), "{i} +{at:#x}");
            }
            for at in [TITLE_BONUS, EUROPE_BONUS, PROMOTED_BONUS, LAST_BONUS] {
                assert_eq!(club::<i32>(&team, at), club::<i32>(&theirs.teams[i], at), "{i} +{at:#x}");
            }
            count += 1;
        }
        assert!(count > 200, "{count}");
    }

    /// The main sponsor's offer is among the original's draws: on its
    /// first new-season day every planned club's years, cut, sums and
    /// attendance contract each come out of some draw of the offer.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_main_sponsors_offer_is_among_the_originals_draws() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let theirs = Records::from_save(Save::read(&keeps.join("NewSeason000.an3")).unwrap()).unwrap();
        let holders = theirs.block(HOLDERS).unwrap().to_vec();
        let words: Vec<usize> = (0..8).map(|k| 0x510 + k * 4).chain([0x2f4, 0x1ec]).collect();
        let mut count = 0;
        for i in 0..theirs.teams.len() {
            let country = i16::from(theirs.teams[i].country());
            if country == 0 || !theirs.globals.played(country) || !planned(&theirs.teams[i]) {
                continue;
            }
            let t = &theirs.teams[i];
            let system = usize::from(theirs.countries.cup_system(usize::from(t.country())));
            let first = theirs.countries.first_team(usize::from(t.country())) as usize;
            let held = holder(&holders, t.country(), i - first);
            let skill = manager_skill(&theirs, t);
            let b = theirs.block(&format!("{STADIUMS}[{i}]")).unwrap();
            let capacity = i32::from_le_bytes(b[STADIUM_CAPACITY..STADIUM_CAPACITY + 4].try_into().unwrap());
            let mut seen_words: Vec<std::collections::HashSet<i32>> = vec![Default::default(); words.len()];
            let mut seen_bytes: [std::collections::HashSet<u8>; 2] = Default::default();
            for seed in 0..4000 {
                let mut team = t.clone();
                sign_main_sponsor(&mut team, system, skill, held, capacity, &mut Random::new(seed));
                for (k, &at) in words.iter().enumerate() {
                    seen_words[k].insert(club::<i32>(&team, at));
                }
                seen_bytes[0].insert(club::<u8>(&team, 0x509));
                seen_bytes[1].insert(club::<u8>(&team, 0x50a));
            }
            for (k, &at) in words.iter().enumerate() {
                let want = club::<i32>(t, at);
                assert!(seen_words[k].contains(&want), "{i} +{at:#x}: {want} not among {:?}", seen_words[k].iter().take(8).collect::<Vec<_>>());
            }
            assert!(seen_bytes[0].contains(&club::<u8>(t, 0x509)), "{i} years {}", club::<u8>(t, 0x509));
            assert!(seen_bytes[1].contains(&club::<u8>(t, 0x50a)), "{i} cut {}", club::<u8>(t, 0x50a));
            count += 1;
        }
        assert!(count > 140, "{count}");
    }
}
