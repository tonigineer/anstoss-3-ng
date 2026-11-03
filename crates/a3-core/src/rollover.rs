//! The players over the season's rollover, in the countries whose clubs
//! the game keeps without playing their leagues (the countries of the
//! second kind, `DAT_00ae61ae` = 2: the clubs abroad the market, the
//! scouts and the European cups draw on).
//!
//! The season's end closes every player's season there (`FUN_006b45c4`,
//! from `FUN_006b3ee0` between the humans' pay and the settlement): his
//! season's counters fall to nought — cards, goals, penalties and free
//! kicks, the keeper's saves, the bans served, the team of the day — a
//! suspension dated in the old season runs out, the contract's options
//! lapse in its last year, and a count of his seasons goes up
//! (`FUN_006b465f`, `FUN_0070665a`).
//!
//! The new season's first day then walks the same clubs again
//! (`FUN_00429418` behind `FUN_0077650c`, in `FUN_0065e659`): a contract
//! in its last year or run out is extended — three or four years, or two
//! and up to a third of the man's strength more — and from the second
//! season on every player grows or declines by his age and his talent
//! (`FUN_00706fea`), his new strength going into his half-season history.
//! Beside it every player's form of the season starts again from one
//! week of 10 (`FUN_0068b753`).
//!
//! Left out: the old men retiring from the second season on (age above
//! 33 plus a draw below 3, `FUN_0070de6c`), the young players the clubs
//! draw to fill thin positions (`FUN_0066ca21`), the stadium byte the
//! season's end clears (`FUN_00638335`'s object +0x68), and the clubs of
//! the countries played, whose rollover runs other routines
//! (`FUN_006b4727`, `FUN_0077655c`). The original draws with its own
//! generator, so the men who grow are others than the original's.

use chrono::NaiveDate;

use crate::random::Random;
use crate::records::{Player, Records};

/// The countries the passes walk, from 1 (`DAT_00ae61ae`'s 0x75 bytes).
const COUNTRIES: usize = 0x75;
/// A country whose clubs are kept without their leagues being played.
const KEPT: u8 = 2;
/// Two words of the team record the season's end puts to nought (the
/// club object's +0xc06 and +0xc08, `FUN_0063b75a`).
const TEAM_SEASON_WORDS: [usize; 2] = [0x24c, 0x24e];

/// The day a suspension is dated on (R+0x1f2): the old season's days
/// run out, later ones move back by its 365.
const SEASON_DAYS: i16 = 365;
/// What the close puts to nought, one byte each: R+0x274 and +0x275, the
/// three goals, the league cards, the first cup and the first European
/// card byte with the cup's ban, the bans served and the team of the day,
/// penalties, free kicks and the keeper's pair, the next club's league.
const CLOSED_BYTES: [usize; 20] = [
    0x274, 0x275, 0x23c, 0x23d, 0x23e, 0x22c, 0x22d, 0x22e, 0x231, 0x234, 0x235, 0x267, 0x268,
    0x24b, 0x24c, 0x24d, 0x24e, 0x24f, 0x250, 0x14c,
];
/// The words it puts to nought: the form's sum, the next club's word at
/// R+0x14e and the knock's day (R+0x1de, the object's +0x2f).
const CLOSED_WORDS: [usize; 3] = [0x1e0, 0x14e, 0x1de];
/// A count of the seasons the rollover passes him through (the object's
/// +0x98): the close and the development raise it by one each; a man at
/// nought does not develop yet. The transfer list reads it too.
const SEASONS: usize = 0x161;
/// A byte the close raises one time in ten, up to 10 (the object's +0x09;
/// meaning not traced).
const STEP: usize = 0x27d;
const STEP_MOST: i8 = 10;
/// The picks' low four bits and two of the partner's bits go with the
/// season.
const PICKS_KEPT: u8 = 0xf0;
const PARTNER_KEPT: u8 = 0x9f;
/// The terms that lapse: the options in the contract's last year, the
/// European-cup clause and 0x800 always.
const OPTIONS: u16 = 0x3000;
const LAPSING: u16 = 0x0810;
/// His strength's history, a nibble each half-season.
const HISTORY: usize = 0x2bd;
const HISTORY_LEN: usize = 20;
/// The form the new season starts every player at, over one week
/// (`FUN_0068b753` walks ids 0 to 0x426c).
const FORM_START: (i16, u8) = (10, 1);
const FORM_PLAYERS: u16 = 0x426d;

/// The clubs of the kept countries, by country and number.
fn kept_clubs(records: &Records) -> Vec<(u16, u16)> {
    (1..COUNTRIES)
        .filter(|&c| records.countries.kind(c) == KEPT)
        .flat_map(|c| (0..records.countries.team_count(c).max(0)).map(move |n| (c as u16, n as u16)))
        .collect()
}

/// The squad of a club, by id.
fn squad(records: &Records, country: u16, number: u16) -> Vec<u16> {
    records.team(country, number).map_or_else(Vec::new, |t| t.players())
}

/// The years his contract has left in `year`, as his object holds them:
/// none once run out.
fn years_left(player: &Player, year: i32) -> i8 {
    let left = (i32::from(player.contract() & 0xfff) - year) as i8;
    left.max(0)
}

/// The contract's year written back from the years left (`FUN_006364e7`'s
/// copy: the whole word).
fn set_years_left(player: &mut Player, year: i32, left: i8) {
    player.set_contract((i32::from(left) + year) as u16);
}

/// The season's end for the kept countries (`FUN_006b45c4`): every club
/// clears its two season words, every player of it closes his season
/// (`FUN_006b465f`), his suspension's day put to nought after.
pub fn close_kept(records: &mut Records, random: &mut Random) {
    let year = i32::from(records.globals.start_year());
    for (country, number) in kept_clubs(records) {
        if let Some(team) = records.team_mut(country, number) {
            for at in TEAM_SEASON_WORDS {
                team.put::<u16>(at, 0);
            }
        }
        for id in squad(records, country, number) {
            let Some(player) = records.player_mut(id) else { continue };
            close_player(player, year, random);
            player.set_sent_off_on(0);
        }
    }
}

/// A player's season closed (`FUN_0070665a`), in the season of `year`.
pub fn close_player(player: &mut Player, year: i32, random: &mut Random) {
    let left = years_left(player, year);
    let day = player.sent_off_on();
    if day <= SEASON_DAYS {
        player.set_sent_off_on(0);
        player.set_league_ban([0, 0]);
    } else {
        player.set_sent_off_on(day - SEASON_DAYS);
    }
    for at in CLOSED_BYTES {
        player.put::<u8>(at, 0);
    }
    for at in CLOSED_WORDS {
        player.put::<u16>(at, 0);
    }
    player.set_picks(player.picks() & PICKS_KEPT);
    let mut terms = player.terms();
    if left < 2 {
        terms &= !OPTIONS;
    }
    player.set_terms(terms & !LAPSING);
    player.put::<u8>(SEASONS, player.get::<u8>(SEASONS).wrapping_add(1));
    if random.below(10) == 0 && player.get::<i8>(STEP) < STEP_MOST {
        player.put::<i8>(STEP, player.get::<i8>(STEP) + 1);
    }
    player.set_partner(player.partner() & PARTNER_KEPT);
    set_years_left(player, year, left);
}

/// Every player's form of the season back at one week of 10
/// (`FUN_0068b753`, at the new season's first init).
pub fn restart_form(records: &mut Records) {
    for id in 0..FORM_PLAYERS {
        let Some(player) = records.players.get_mut(usize::from(id)) else { continue };
        player.set_season_form(FORM_START.0);
        player.set_season_form_weeks(FORM_START.1);
    }
}

/// The new season's first day for the kept countries (`FUN_00429418`):
/// per club the contracts in their last year or run out extended, and
/// from the second season on every player developed (`FUN_00429182`).
pub fn open_kept(records: &mut Records, random: &mut Random) {
    let year = i32::from(records.globals.start_year());
    let today = {
        let [day, month, y] = records.globals.date();
        NaiveDate::from_ymd_opt(i32::from(y), u32::from(month), u32::from(day))
    };
    let developing = records.globals.seasons() > 1;
    for (country, number) in kept_clubs(records) {
        let ids = squad(records, country, number);
        for &id in &ids {
            let Some(player) = records.player_mut(id) else { continue };
            let mut left = years_left(player, year);
            match left {
                0 => left += 3 + random.below(2) as i8,
                1 => left += 2 + random.below(u32::from(player.strength()) / 3) as i8,
                _ => {}
            }
            set_years_left(player, year, left);
        }
        let Some(today) = today.filter(|_| developing) else { continue };
        for &id in &ids {
            if let Some(player) = records.player_mut(id) {
                develop(player, today, random);
            }
        }
    }
}

/// The strength a talent reaches at most (`FUN_006ff4c0`): 12 for the
/// greatest, 11, 10, 9, 8 for none.
fn top_strength(talent: i8) -> u8 {
    match talent {
        0 => 12,
        1 => 11,
        3 => 9,
        4 => 8,
        _ => 10,
    }
}

/// A season's growth or decline (`FUN_00706fea`): by his age the chances
/// to gain a point (three times the first figure in 200) or else lose one
/// (the second in 100) — 60/0 below 22, 40/5 below 26, 30/20 below 30,
/// 10 and 10 for a keeper or a defender, else 40, below 34, 0/60 below
/// 40, then 0/100 — weighed by his talent; never above what his talent
/// reaches (a 13 stays at most 13), never below 1. His first season only
/// starts the count.
pub fn develop(player: &mut Player, today: NaiveDate, random: &mut Random) {
    let seasons = player.get::<u8>(SEASONS);
    if seasons == 0 {
        player.put::<u8>(SEASONS, 1);
        return;
    }
    let old = player.strength();
    let age = crate::statistics::age_on(player, today);
    let (mut up, mut down): (u16, u16) = match age {
        ..22 => (60, 0),
        22..26 => (40, 5),
        26..30 => (30, 20),
        30..34 => (10, if matches!(player.positions()[0], 1 | 2) { 10 } else { 40 }),
        34..40 => (0, 60),
        _ => (0, 100),
    };
    let talent = player.talent();
    match talent as u8 {
        0 => {
            if age < 25 {
                up += up / 2;
            } else if age < 29 {
                up += up >> 2;
            }
        }
        1 => {
            if old < 12 {
                if age < 25 {
                    up += up >> 2;
                }
            } else {
                up = 0;
            }
        }
        3 => {
            if old > 8 {
                up = 0;
            }
            down += down >> 2;
        }
        4 => {
            if old > 7 {
                up = 0;
            }
            down += down / 2;
        }
        _ => {
            if old > 9 {
                up = 0;
            }
        }
    }
    let mut strength = old;
    if (random.below(200) as i16) < (up * 3) as i16 {
        strength += 1;
    } else if (random.below(100) as i16) < down as i16 && strength > 1 {
        strength -= 1;
    }
    if old == 13 {
        strength = strength.min(13);
    } else {
        strength = strength.min(top_strength(talent));
    }
    strength = strength.max(1);
    player.set_strength(strength);
    player.put::<u8>(SEASONS, seasons.wrapping_add(1));
    note_strength(player, strength);
}

/// His strength into the next free nibble of the history (`FUN_00705001`).
/// A full history moves on by a byte first — and the original then writes
/// the nibble one byte behind it, into the first byte of the form's ring,
/// which the replica does as well.
fn note_strength(player: &mut Player, strength: u8) {
    let byte = |player: &Player, k: usize| player.get::<u8>(HISTORY + k);
    let mut k = 0;
    while k < HISTORY_LEN && byte(player, k) & 0xf != 0 && byte(player, k) & 0xf0 != 0 {
        k += 1;
    }
    if k == HISTORY_LEN {
        for i in 0..HISTORY_LEN - 1 {
            let next = byte(player, i + 1);
            player.put::<u8>(HISTORY + i, next);
        }
        player.put::<u8>(HISTORY + HISTORY_LEN - 1, 0);
    }
    let b = byte(player, k);
    let b = if b & 0xf == 0 { b | strength } else { b | strength << 4 };
    player.put::<u8>(HISTORY + k, b);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player() -> Player {
        Player::new(&vec![0u8; Player::SIZE]).unwrap()
    }

    #[test]
    fn the_close_empties_the_season() {
        let mut p = player();
        p.set_goals([4, 1, 2]);
        p.set_league_cards([3, 1, 0]);
        p.set_penalties([2, 1]);
        p.set_of_the_day(5);
        p.set_season_form(120);
        p.set_sent_off_on(200);
        p.set_league_ban([2, 0x80]);
        p.set_picks(0x2f);
        p.set_partner(0xe3);
        p.set_contract(2026);
        p.set_terms(0x3811);
        p.put::<u8>(SEASONS, 3);
        close_player(&mut p, 2025, &mut Random::new(1));
        assert_eq!((p.goals(), p.league_cards(), p.penalties()), ([0; 3], [0; 3], [0; 2]));
        assert_eq!((p.of_the_day(), p.season_form()), (0, 0));
        assert_eq!((p.sent_off_on(), p.league_ban()), (0, [0, 0]), "the old season's ban runs out");
        assert_eq!((p.picks(), p.partner()), (0x20, 0x83));
        assert_eq!(p.terms(), 0x0001, "the options lapse in the contract's last year");
        assert_eq!((p.get::<u8>(SEASONS), p.contract()), (4, 2026));
        // A ban dated in the next season moves back by its days; the
        // options of a longer contract stay.
        p.set_sent_off_on(400);
        p.set_league_ban([1, 0]);
        p.set_contract(2028);
        p.set_terms(0x3000);
        close_player(&mut p, 2025, &mut Random::new(1));
        assert_eq!((p.sent_off_on(), p.league_ban(), p.terms()), (35, [1, 0], 0x3000));
    }

    #[test]
    fn a_player_develops_within_his_talent() {
        let today = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
        let mut random = Random::new(3);
        let born = |p: &mut Player, year: u16| {
            p.set_birth_day(1);
            p.set_birth_month(1);
            p.set_birth_year(year);
        };
        // His first season only starts the count.
        let mut p = player();
        p.set_strength(5);
        develop(&mut p, today, &mut random);
        assert_eq!((p.strength(), p.get::<u8>(SEASONS), p.get::<u8>(HISTORY)), (5, 1, 0));
        // A young talent climbs, never past twelve.
        let mut p = player();
        born(&mut p, 2007);
        p.set_strength(10);
        p.put::<u8>(SEASONS, 1);
        for _ in 0..12 {
            develop(&mut p, today, &mut random);
        }
        assert_eq!(p.strength(), 12, "nineteen with the greatest talent");
        assert_eq!(p.get::<u8>(SEASONS), 13);
        // An old man without talent sinks, never below one; a weak
        // talent's strength falls back to its top at once.
        let mut p = player();
        born(&mut p, 1980);
        p.set_talent(4);
        p.set_strength(9);
        p.put::<u8>(SEASONS, 1);
        develop(&mut p, today, &mut random);
        assert!(p.strength() <= 8, "talent 4 reaches eight");
        for _ in 0..20 {
            develop(&mut p, today, &mut random);
        }
        assert_eq!(p.strength(), 1);
        // A thirteen stays at most thirteen.
        let mut p = player();
        born(&mut p, 2008);
        p.set_strength(13);
        p.put::<u8>(SEASONS, 1);
        develop(&mut p, today, &mut random);
        assert_eq!(p.strength(), 13);
    }

    #[test]
    fn the_history_takes_a_nibble_a_season() {
        let mut p = player();
        note_strength(&mut p, 7);
        note_strength(&mut p, 8);
        note_strength(&mut p, 9);
        assert_eq!((p.get::<u8>(HISTORY), p.get::<u8>(HISTORY + 1)), (0x87, 0x09));
        // Full, it moves on by a byte and the nibble lands behind it.
        for k in 0..HISTORY_LEN {
            p.put::<u8>(HISTORY + k, 0x55);
        }
        note_strength(&mut p, 6);
        assert_eq!(p.get::<u8>(HISTORY + HISTORY_LEN - 1), 0);
        assert_eq!(p.get::<u8>(HISTORY + HISTORY_LEN), 0x06, "the form ring's first byte");
    }

    /// The kept countries' players over the original's pair: SeasonEnd364
    /// (its last day before the season end) closed and opened by the
    /// replica against NewSeason000. Every player still in a squad there
    /// — the original retires the old ones, which the replica does not —
    /// stands with his season's counters as the original's, the same count
    /// of seasons, his contract extended within the draws' reach and his
    /// strength at most a point from where it was.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_kept_countries_roll_over_as_the_original() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let read = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs, before) =
            (read("SeasonEnd364.an3"), read("NewSeason000.an3"), read("SeasonEnd364.an3"));
        let mut random = Random::new(7);
        close_kept(&mut ours, &mut random);
        crate::settlement::open_season(&mut ours, &mut random);
        let year = i32::from(theirs.globals.start_year());
        let mut compared = 0;
        for (country, number) in kept_clubs(&before) {
            let after = squad(&theirs, country, number);
            for id in squad(&before, country, number).into_iter().filter(|id| after.contains(id)) {
                let (o, x, y) = (before.player(id).unwrap(), ours.player(id).unwrap(), theirs.player(id).unwrap());
                if o.birthday() != y.birthday() {
                    continue;
                }
                compared += 1;
                for at in CLOSED_BYTES.into_iter().chain([0x22f, 0x230, 0x271, 0x27e, 0x161, 0x27b]) {
                    assert_eq!(x.get::<u8>(at), y.get::<u8>(at), "player {id} at {at:#x}");
                }
                for at in CLOSED_WORDS.into_iter().chain([0x1f2]) {
                    assert_eq!(x.get::<u16>(at), y.get::<u16>(at), "player {id} at {at:#x}");
                }
                assert_eq!(x.terms() & !0x40, y.terms() & !0x40, "player {id}'s terms");
                let left = |p: &Player| i32::from(p.contract() & 0xfff) - year;
                let reach = match left(o).max(0) {
                    0 => 3..=4,
                    1 => 3..=3 + i32::from(o.strength()) / 3,
                    l => l..=l,
                };
                assert!(reach.contains(&left(y)), "player {id}: {} in {reach:?}", left(y));
                assert!(reach.contains(&left(x)), "player {id}: {} in {reach:?}", left(x));
                for p in [x, y] {
                    assert!(i32::from(p.strength()).abs_diff(i32::from(o.strength())) <= 1, "player {id}");
                }
            }
        }
        assert!(compared > 5000, "{compared} players compared");
    }
}
