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
//! The countries played close their players' seasons the same way at the
//! season's end, club by club with its reserve team (`FUN_006b4727`), and
//! open them at the new season's first day by a pass of their own
//! (`FUN_0077655c`, `FUN_007762cc`): the season's counters to nought
//! again — the league matches, assists and kinds of goals among them —,
//! a weak condition or freshness lifted, the loan's terms and some
//! reasons ended, and the form of the season and the last form set to
//! the form today.
//!
//! Players end their careers at the season's end — those of the played
//! countries who said they would (`FUN_0066d435`, a human club's in its
//! history) — and at the new season's start the kept countries' old men,
//! above 33 and a draw below 3 (`FUN_00429182`): out of their squads and
//! lists, their records into the pool new players are made from
//! (`FUN_0070de6c`, `FUN_0066d975`).
//!
//! The thin clubs' new players come from [`crate::newplayer`].
//!
//! Left out: the national legends (`FUN_0066df9e`), the stadium byte the season's
//! end clears (`FUN_00638335`'s object +0x68); for the countries
//! played the contracts that run out (`FUN_006b3f92`), the amateurs'
//! retirements, the staff's contracts and the computer clubs' new youth
//! and strength. The original draws with its own generator, so the men
//! who grow are others than the original's.

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

/// Before the season's deals every club of the countries played counts
/// a first season for its players who have none (`FUN_006b4459` per club,
/// `FUN_006b3f92`; the contracts that run out there are
/// [`crate::free_agents::release`]).
pub fn count_first_seasons(records: &mut Records) {
    for country in crate::opponent::played_nations(records) {
        for number in 0..records.countries.team_count(usize::from(country)).max(0) as u16 {
            for id in squad(records, country, number) {
                if let Some(player) = records.player_mut(id).filter(|p| p.get::<u8>(SEASONS) == 0) {
                    player.put::<u8>(SEASONS, 1);
                }
            }
        }
    }
}

/// The played countries' clubs close their players' season at the
/// season's end (`FUN_006b44cb` per club, `FUN_006b4727`): a club that is
/// no reserve team clears its two season words and closes every player's
/// season (`FUN_0070665a`), and its reserve team, if it has one
/// (team +0x873), does the same. (The retirements from a club's
/// amateurs, `FUN_00407ecd`, and its staff's contracts, `FUN_006b4d51`,
/// are not built.)
pub fn close_played(records: &mut Records, random: &mut Random) {
    let year = i32::from(records.globals.start_year());
    for country in crate::opponent::played_nations(records) {
        for number in 0..records.countries.team_count(usize::from(country)).max(0) as u16 {
            let Some(team) = records.team(country, number) else { continue };
            let [first, reserve] = team.linked();
            if first >= 0 {
                continue;
            }
            let clubs = std::iter::once(number).chain(u16::try_from(reserve).ok());
            for club in clubs.collect::<Vec<_>>() {
                if let Some(team) = records.team_mut(country, club) {
                    for at in TEAM_SEASON_WORDS {
                        team.put::<u16>(at, 0);
                    }
                }
                for id in squad(records, country, club) {
                    if let Some(player) = records.player_mut(id) {
                        close_player(player, year, random);
                    }
                }
            }
        }
        // The country's youth leagues move their teams up and down
        // after its clubs (`FUN_006b44cb`'s `FUN_00670ac5`).
        crate::youth_league::close_season(records, country);
    }
}

/// What the new season puts to nought of a played country's player
/// (`FUN_007762cc`), byte by byte: R+0x274 to +0x279, the goals, the
/// cards' first bytes, the goals' kinds, own goals, assists and joker
/// goals, the last match's, penalties, free kicks and the keeper's pair,
/// chances and home goals, the league matches and the two counts behind
/// them, the bans served and the team of the day, R+0x25e.
const OPENED_BYTES: [usize; 38] = [
    0x274, 0x275, 0x23c, 0x23d, 0x23e, 0x277, 0x278, 0x279, 0x252, 0x22c, 0x22d, 0x22e, 0x231,
    0x235, 0x241, 0x242, 0x243, 0x244, 0x246, 0x245, 0x247, 0x248, 0x267, 0x268, 0x24c, 0x24b,
    0x24e, 0x24d, 0x250, 0x24f, 0x253, 0x254, 0x255, 0x25e, 0x276, 0x249, 0x24a, 0x251,
];
/// A condition (in hundredths) or freshness this low is lifted.
const LOW: i32 = 30;
const LOWISH: i32 = 50;
const CONDITION_LIFT: [i16; 2] = [40, 20];
const FRESHNESS_LIFT: [i16; 2] = [50, 30];
/// Bit 11 of R+0x1b4: he smokes, his condition counts five less.
const SMOKES: u32 = 0x800;
/// The reasons the season takes from him (R+0x1b0's bits 0x40, 0x100
/// and 0x800), the terms' bits it ends (the loan's 0x100, 0x200, 0x400),
/// R+0x26f's bit 6.
const REASONS_ENDED: u32 = 0x940;
const TERMS_ENDED: u16 = 0x0700;
const RETIRING_KEPT: u8 = 0xbf;
/// R+0x23a, put to nought in every even year.
const EVEN_YEAR_BYTE: usize = 0x23a;

/// The new season opens a played country's player (`FUN_007762cc`).
pub fn open_player(player: &mut Player, year: i32) {
    for at in OPENED_BYTES {
        player.put::<u8>(at, 0);
    }
    player.put::<u16>(0x1de, 0);
    let condition = {
        let c = i32::from(player.condition()) / 100;
        if player.shown() & SMOKES == 0 { c } else if c < 5 { 0 } else { c - 5 }
    };
    let lift = if condition < LOW { Some(CONDITION_LIFT[0]) } else if condition < LOWISH { Some(CONDITION_LIFT[1]) } else { None };
    if let Some(by) = lift {
        player.set_condition((player.condition().saturating_add(by * 100)).clamp(0, 10000));
    }
    let freshness = {
        let f = player.freshness();
        let sum = f[0].wrapping_add(f[1]).wrapping_add(f[2]);
        if sum < 0 { 0 } else if sum <= 10000 { i32::from(sum) / 100 } else { 100 }
    };
    let lift = if freshness < LOW { Some(FRESHNESS_LIFT[0]) } else if freshness < LOWISH { Some(FRESHNESS_LIFT[1]) } else { None };
    if let Some(by) = lift {
        let mut f = player.freshness();
        f[0] = f[0].saturating_add(by * 100).clamp(0, 10000);
        player.set_freshness(f);
    }
    player.set_terms(player.terms() & !TERMS_ENDED);
    player.put::<u8>(SEASONS, player.get::<u8>(SEASONS).wrapping_add(1));
    crate::mood::clear_reasons(player, REASONS_ENDED);
    player.set_retiring(player.retiring() & RETIRING_KEPT);
    player.set_fined(0);
    if year % 2 == 0 {
        player.put::<u8>(EVEN_YEAR_BYTE, 0);
    }
}

/// The new season's first day for the played countries' players
/// (`FUN_0077655c` per club): a thin computer squad filled with youngsters
/// when the tables are at hand ([`crate::newplayer::fill_played_club`]), a
/// weak one lifted ([`crate::development::lift_weak`]), then each player
/// opened, his last form and his form of the season — one week of it —
/// his form today; and with the tables at hand the overdraft the bank
/// allows by the squad's worth (club +0x4f0, `FUN_00622e08`). (The
/// computer clubs' line-ups and the rest of `FUN_0077655c` are not
/// built.)
pub fn open_played(
    records: &mut Records,
    tables: Option<(&crate::contract::ContractTables, &crate::value::ValueTables)>,
    random: &mut Random,
) {
    let year = i32::from(records.globals.start_year());
    let [day, month, y] = records.globals.date();
    let today = NaiveDate::from_ymd_opt(i32::from(y), u32::from(month), u32::from(day));
    for country in crate::opponent::played_nations(records) {
        for number in 0..records.countries.team_count(usize::from(country)).max(0) as u16 {
            if let Some(tables) = tables {
                crate::newplayer::fill_played_club(records, (country, number), tables, random);
            }
            crate::development::lift_weak(records, country, number, random);
            for id in squad(records, country, number) {
                let Some(player) = records.player_mut(id) else { continue };
                open_player(player, year);
                let form = crate::statistics::form_today(player);
                player.set_last_form(form as u8);
                player.set_season_form(i16::from(form as u8));
                player.set_season_form_weeks(1);
            }
            // The overdraft the bank allows by the squad's worth, a tenth
            // of it in 100.000s — the stars of last season gone by then
            // (`FUN_0065e659` clears them first; their new ones are not
            // drawn here).
            if let (Some((_, values)), Some(today)) = (tables, today) {
                let Some(team) = records.team(country, number) else { continue };
                let worth = crate::value::squad_worth(records, team, values, today, [0; 11]);
                if let Some(team) = records.team_mut(country, number) {
                    team.set_overdraft((worth / 10 / OVERDRAFT_UNIT) as i16);
                }
            }
        }
    }
}

/// The overdraft's unit (`DAT_00a3b37c`).
const OVERDRAFT_UNIT: i64 = 100_000;

/// The transfer list (its block from the object's +0x20): an i16 count,
/// 400 entries of 0x1c bytes from +8, the player's id first.
const TRANSFER_LIST: &str = "0x1c49278";
const LISTED_FIRST: usize = 8;
const LISTED: usize = 0x1c;
const LISTED_MOST: usize = 400;
/// The human slots' watch lists: an i16 count, fifty entries of 16 bytes
/// from +4, the id first.
const WATCH_LISTS: usize = 4;
const WATCHED_FIRST: usize = 4;
const WATCHED: usize = 0x10;
/// The pool of ended careers new players are made from (`DAT_00b628d8`):
/// thirty counts by class, then 0x1964 entries of four bytes — the id,
/// the class (R+0x227), the main position.
const POOL: &str = "0xb628d8";
const POOL_ENTRIES: usize = 0x3c;
const POOL_SIZE: usize = 0x1964;
const CLASSES: usize = 30;
/// A class is sent on into the regeneration table once it holds this
/// many (the table at 0x8a00bc).
const POOL_FULL: [i16; CLASSES] = {
    let mut t = [20; CLASSES];
    let mut k = 0;
    while k < 10 {
        t[k] = 50;
        k += 1;
    }
    t
};
/// The regeneration table (`DAT_00b68ea8`, 9000 entries of four bytes)
/// and each class's range in it (`DAT_00b5cc08`, `DAT_00b5cc44`).
const REGENERATION: &str = "0xb68ea8";
const CLASS_FROM: usize = 0xb5cc08;
const CLASS_TO: usize = 0xb5cc44;
/// The option to swap a new pool entry's first name with one of his
/// class's (`DAT_00b5d159`).
const SWAP_NAMES: usize = 0xb5d159;
/// The first id the pool takes no more (the player-managers).
const POOLED_BELOW: u16 = 0x4269;
/// The human club's history kind of a career's end.
const CAREER_END: u8 = 7;
const RETIRES: u8 = 0x80;

/// A player out of the lists he stands in: the transfer list
/// (`FUN_00758f42`) and the human slots' watch lists (`FUN_004189c9`).
fn delist(records: &mut Records, id: u16) {
    let unlist = |block: &mut [u8], first: usize, size: usize, most: usize| {
        let count = usize::try_from(i16::from_le_bytes([block[0], block[1]])).unwrap_or(0).min(most);
        let id_at = |k: usize| u16::from_le_bytes([block[first + k * size], block[first + k * size + 1]]);
        if let Some(k) = (0..count).find(|&k| id_at(k) == id) {
            block.copy_within(first + (k + 1) * size..first + count * size, first + k * size);
            block[0..2].copy_from_slice(&((count - 1) as i16).to_le_bytes());
        }
    };
    delist_transfer(records, id);
    for slot in 0..WATCH_LISTS {
        if let Some(list) = records.block_mut(&format!("0xa4d0f8[{slot}]")).filter(|b| b.len() >= WATCHED_FIRST + 50 * WATCHED) {
            unlist(list, WATCHED_FIRST, WATCHED, 50);
        }
    }
}

/// A player off the transfer list only (`FUN_0075a636` → `FUN_00758f42`):
/// the entries behind his move up, all four hundred places through, the
/// last staying as it was.
pub(crate) fn delist_transfer(records: &mut Records, id: u16) {
    if let Some(list) = records.block_mut(TRANSFER_LIST).filter(|b| b.len() >= LISTED_FIRST + LISTED_MOST * LISTED) {
        let count = usize::try_from(i16::from_le_bytes([list[0], list[1]])).unwrap_or(0).min(LISTED_MOST);
        let id_at = |list: &[u8], k: usize| u16::from_le_bytes([list[LISTED_FIRST + k * LISTED], list[LISTED_FIRST + k * LISTED + 1]]);
        if let Some(k) = (0..count).find(|&k| id_at(list, k) == id) {
            list.copy_within(LISTED_FIRST + (k + 1) * LISTED..LISTED_FIRST + LISTED_MOST * LISTED, LISTED_FIRST + k * LISTED);
            list[0..2].copy_from_slice(&((count - 1) as i16).to_le_bytes());
        }
    }
}

/// A player of no club ends his career (`FUN_0066d93b` → `FUN_0066d975`):
/// off the lists, his retiring bits gone, into the pool.
pub(crate) fn end_career(records: &mut Records, id: u16, random: &mut Random) {
    delist(records, id);
    if let Some(p) = records.player_mut(id) {
        p.set_retiring(0);
    }
    if id < POOLED_BELOW {
        pool(records, id, random);
    }
}

/// A move into a human club's history (`FUN_0070e8c4` of kind 1 came or
/// 2 left): the year and week, his names, the other club, his strength
/// and position, no fee.
pub(crate) fn note_move(records: &mut Records, club: (u8, i16), id: u16, kind: u8, other: (u8, i16)) {
    let Some(slot) = u16::try_from(club.1)
        .ok()
        .and_then(|n| records.team(u16::from(club.0), n))
        .and_then(|t| usize::from(t.human()).checked_sub(1))
    else {
        return;
    };
    let Some(p) = records.player(id) else { return };
    let mut entry = career_end_entry(p, records.globals.start_year(), (records.globals.day() / 7) as u8);
    entry[3] = kind;
    entry[8] = other.0;
    entry[0xa..0xc].copy_from_slice(&other.1.to_le_bytes());
    if let Some(extra) = records.extras.get_mut(slot) {
        push_history(extra, entry);
    }
}

/// A class of the pool, or all of them (`None`), sent on into the
/// regeneration table (`FUN_0066dc8b`): from a random end of the pool,
/// each entry into the first free place of its class's range.
fn regenerate(records: &mut Records, class: Option<usize>, random: &mut Random) {
    let marked = |c: usize| class.is_none_or(|k| k == c);
    let mut start: [i16; CLASSES] = std::array::from_fn(|c| records.globals.get::<i16>(CLASS_FROM + 2 * c));
    let end: [i16; CLASSES] = std::array::from_fn(|c| records.globals.get::<i16>(CLASS_TO + 2 * c));
    let backwards = random.below(2) == 0;
    let (Some(mut pool), Some(mut table)) =
        (records.block(POOL).map(<[u8]>::to_vec), records.block(REGENERATION).map(<[u8]>::to_vec))
    else {
        return;
    };
    let order: Vec<usize> = if backwards { (0..POOL_SIZE).rev().collect() } else { (0..POOL_SIZE).collect() };
    for k in order {
        let at = POOL_ENTRIES + 4 * k;
        if pool.len() < at + 4 || u16::from_le_bytes([pool[at], pool[at + 1]]) == 0 {
            continue;
        }
        let c = usize::from(pool[at + 2]);
        if c >= CLASSES || !marked(c) {
            continue;
        }
        let mut j = start[c];
        while j < end[c] {
            let to = 4 * usize::try_from(j).unwrap_or(0);
            if table.len() >= to + 4 && u16::from_le_bytes([table[to], table[to + 1]]) == 0 {
                let n = i16::from_le_bytes([pool[2 * c], pool[2 * c + 1]]) - 1;
                pool[2 * c..2 * c + 2].copy_from_slice(&n.to_le_bytes());
                table[to..to + 4].copy_from_slice(&pool[at..at + 4]);
                pool[at..at + 4].fill(0);
                break;
            }
            j += 1;
        }
        start[c] = j + 1;
    }
    if let Some(b) = records.block_mut(POOL) {
        b.copy_from_slice(&pool);
    }
    if let Some(b) = records.block_mut(REGENERATION) {
        b.copy_from_slice(&table);
    }
}

/// An ended career into the pool (`FUN_0066d975`'s end): the first free
/// place takes him, by his class and position — a full pool first sends
/// every class on —; with the option set he swaps first names with one
/// of his class there; his class full, it is sent on.
fn pool(records: &mut Records, id: u16, random: &mut Random) {
    let Some(player) = records.player(id) else { return };
    let (class, position) = (player.get::<u8>(0x227), player.positions()[0]);
    let scan = |records: &Records| -> (Option<usize>, Vec<u16>) {
        let Some(b) = records.block(POOL) else { return (None, Vec::new()) };
        let mut free = None;
        let mut same = Vec::new();
        for k in 0..POOL_SIZE {
            let at = POOL_ENTRIES + 4 * k;
            let Some(e) = b.get(at..at + 4) else { break };
            let entry = u16::from_le_bytes([e[0], e[1]]);
            if entry == 0 {
                free = free.or(Some(k));
            } else if e[2] == class && same.len() < 100 {
                same.push(entry);
            }
        }
        (free, same)
    };
    let (mut free, same) = scan(records);
    if free.is_none() {
        regenerate(records, None, random);
        free = scan(records).0;
    }
    let Some(free) = free else { return };
    if !same.is_empty() && records.globals.get::<u8>(SWAP_NAMES) == 1 {
        let other = same[random.below(same.len() as u32) as usize];
        let names = (records.player(other).map(|p| p.first_name()), records.player(id).map(|p| p.first_name()));
        if let (Some(a), Some(b)) = names {
            records.player_mut(other).unwrap().set_first_name(b);
            records.player_mut(id).unwrap().set_first_name(a);
        }
    }
    let count = {
        let Some(b) = records.block_mut(POOL) else { return };
        let at = POOL_ENTRIES + 4 * free;
        b[at..at + 2].copy_from_slice(&id.to_le_bytes());
        b[at + 2] = class;
        b[at + 3] = position;
        let c = 2 * usize::from(class);
        let n = i16::from_le_bytes([b[c], b[c + 1]]) + 1;
        b[c..c + 2].copy_from_slice(&n.to_le_bytes());
        n
    };
    if POOL_FULL.get(usize::from(class)).is_some_and(|&full| count >= full) {
        regenerate(records, Some(usize::from(class)), random);
    }
}

/// A player ends his career at his club (`FUN_0070de6c`): out of its
/// squad and its line-up (`FUN_0070e01a`, `FUN_0070d9c5`), off the
/// transfer list and the watch lists, his retiring bits gone, and into
/// the pool (`FUN_0066d975`). (His national legends, `FUN_0066df9e`, and
/// the other lists he is taken from are not built.)
pub fn retire(records: &mut Records, country: u16, number: u16, id: u16, random: &mut Random) {
    if let Some(team) = records.team_mut(country, number) {
        let squad: Vec<u16> = team.players().into_iter().filter(|&p| p != id).collect();
        team.set_players(&squad);
        let clear = |slots: &mut [u16]| slots.iter_mut().filter(|s| **s == id).for_each(|s| *s = 0);
        let mut lineup = team.lineup();
        clear(&mut lineup);
        team.set_lineup(lineup);
        let mut bench = team.bench();
        clear(&mut bench);
        team.set_bench(bench);
    }
    delist(records, id);
    if let Some(p) = records.player_mut(id) {
        p.set_retiring(0);
    }
    if id < POOLED_BELOW {
        pool(records, id, random);
    }
}

/// A career's end into a human club's history (`FUN_0070e8c4` of kind 7):
/// the year and week, his names, his strength and position, no club, no
/// fee; a full history drops its oldest.
fn note_career_end(records: &mut Records, country: u16, number: u16, id: u16) {
    let Some(slot) = records.team(country, number).and_then(|t| usize::from(t.human()).checked_sub(1)) else { return };
    let Some(p) = records.player(id) else { return };
    let entry = career_end_entry(p, records.globals.start_year(), (records.globals.day() / 7) as u8);
    if let Some(extra) = records.extras.get_mut(slot) {
        push_history(extra, entry);
    }
}

const HISTORY_COUNT: usize = 0xdde;
const HISTORY_LIST: usize = 0xde0;
const HISTORY_ENTRY: usize = 0x14;
const HISTORY_MOST: i16 = 100;

fn career_end_entry(p: &Player, year: u16, week: u8) -> [u8; HISTORY_ENTRY] {
    let mut e = [0u8; HISTORY_ENTRY];
    e[0..2].copy_from_slice(&year.to_le_bytes());
    e[2] = week;
    e[3] = CAREER_END;
    e[4..6].copy_from_slice(&p.first_name().to_le_bytes());
    e[6..8].copy_from_slice(&p.last_name().to_le_bytes());
    e[0xc] = p.strength();
    e[0xd] = p.positions()[0];
    e
}

pub(crate) fn push_history(extra: &mut crate::records::Extra, entry: [u8; HISTORY_ENTRY]) {
    let mut count = extra.get::<i16>(HISTORY_COUNT).clamp(0, HISTORY_MOST);
    if count == HISTORY_MOST {
        for k in 0..usize::from(HISTORY_MOST as u16) - 1 {
            let next: [u8; HISTORY_ENTRY] = extra.get(HISTORY_LIST + (k + 1) * HISTORY_ENTRY);
            extra.put(HISTORY_LIST + k * HISTORY_ENTRY, next);
        }
        count = HISTORY_MOST - 1;
    }
    extra.put(HISTORY_LIST + count as usize * HISTORY_ENTRY, entry);
    extra.put(HISTORY_COUNT, count + 1);
}

/// The season's end retires the players who said they would end their
/// careers (R+0x26f's bit 7; `FUN_0066d435`): the human clubs' first,
/// each into his club's history, then every club's of the countries
/// played (`FUN_0066d815`). (The human's notice of them, event 0xcd, is
/// not built.)
pub fn retire_announced(records: &mut Records, random: &mut Random) {
    let retiring = |records: &Records, country: u16, number: u16| -> Vec<u16> {
        squad(records, country, number)
            .into_iter()
            .filter(|&id| records.player(id).is_some_and(|p| p.retiring() & RETIRES != 0))
            .collect()
    };
    for (country, number) in crate::statistics::human_clubs(records) {
        let (country, Ok(number)) = (u16::from(country), u16::try_from(number)) else { continue };
        for id in retiring(records, country, number) {
            note_career_end(records, country, number, id);
            retire(records, country, number, id, random);
        }
    }
    for country in crate::opponent::played_nations(records) {
        for number in 0..records.countries.team_count(usize::from(country)).max(0) as u16 {
            for id in retiring(records, country, number) {
                retire(records, country, number, id, random);
            }
        }
    }
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
pub fn open_kept(
    records: &mut Records,
    tables: Option<(&crate::contract::ContractTables, &crate::value::ValueTables)>,
    random: &mut Random,
) {
    let year = i32::from(records.globals.start_year());
    let today = {
        let [day, month, y] = records.globals.date();
        NaiveDate::from_ymd_opt(i32::from(y), u32::from(month), u32::from(day))
    };
    let developing = records.globals.seasons() > 1;
    for (country, number) in kept_clubs(records) {
        // From the second season on the old men end their careers: above
        // 33 and a draw below 3 (`FUN_00429182`'s first loop).
        if developing && let Some(today) = today {
            let old: Vec<u16> = squad(records, country, number)
                .into_iter()
                .filter(|&id| {
                    let age = records.player(id).map_or(0, |p| crate::statistics::age_on(p, today));
                    random.below(3) as i32 + 33 < age
                })
                .collect();
            for id in old {
                retire(records, country, number, id, random);
            }
        }
        let ids = squad(records, country, number);
        let mut by_position = [0i16; 11];
        for &id in &ids {
            let Some(player) = records.player_mut(id) else { continue };
            let mut left = years_left(player, year);
            match left {
                0 => left += 3 + random.below(2) as i8,
                1 => left += 2 + random.below(u32::from(player.strength()) / 3) as i8,
                _ => {}
            }
            set_years_left(player, year, left);
            by_position[usize::from(player.positions()[0]).min(10)] += 1;
        }
        // A thin squad takes new players (`FUN_00429182` → `FUN_0066ca21`):
        // two keepers below 28 men with fewer than two, and below 16 one
        // for every position under its least.
        if let Some(tables) = tables {
            let club = (country, number);
            if ids.len() < KEPT_THIN && by_position[1] < 2 {
                for _ in 0..2 {
                    crate::newplayer::for_kept_club(records, club, Some(1), tables, random);
                }
            }
            if squad(records, country, number).len() < KEPT_SHORT {
                for position in 2..11u8 {
                    if by_position[usize::from(position)] < i16::from(KEPT_LEAST[usize::from(position)]) {
                        crate::newplayer::for_kept_club(records, club, Some(position), tables, random);
                    }
                }
            }
        }
        let ids = squad(records, country, number);
        let Some(today) = today.filter(|_| developing) else { continue };
        for &id in &ids {
            if let Some(player) = records.player_mut(id) {
                develop(player, today, random);
            }
        }
    }
}

/// A kept club this thin takes two keepers when it has fewer than two, this
/// short one man for each position under its least (`DAT_008773e0`).
const KEPT_THIN: usize = 28;
const KEPT_SHORT: usize = 16;
const KEPT_LEAST: [u8; 11] = [0, 2, 1, 3, 1, 1, 2, 2, 1, 2, 3];

/// The regeneration of one class (`FUN_0066dc8b`).
pub(crate) fn regenerate_class(records: &mut Records, class: usize, random: &mut Random) {
    regenerate(records, Some(class), random);
}

/// The strength a talent reaches at most (`FUN_006ff4c0`): 12 for the
/// greatest, 11, 10, 9, 8 for none.
pub(crate) fn top_strength(talent: i8) -> u8 {
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
pub(crate) fn note_strength(player: &mut Player, strength: u8) {
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

    /// On the original's NewSeason000 every played club's overdraft is a
    /// tenth of its squad's worth in 100.000s, reckoned on the season's
    /// first day without the stars, which the new season has cleared.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_overdraft_is_the_squads_worth_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let n = Records::from_save(Save::read(&keeps.join("NewSeason000.an3")).unwrap()).unwrap();
        let data = crate::data::Data::load(&dir).unwrap();
        let [day, month, year] = n.globals.date();
        let today = NaiveDate::from_ymd_opt(i32::from(year), u32::from(month), u32::from(day)).unwrap();
        let mut clubs = 0;
        for country in crate::opponent::played_nations(&n) {
            for number in 0..n.countries.team_count(usize::from(country)).max(0) as u16 {
                let team = n.team(country, number).unwrap();
                let worth = crate::value::squad_worth(&n, team, &data.rules.value, today, [0; 11]);
                assert_eq!((worth / 10 / OVERDRAFT_UNIT) as i16, team.overdraft(), "{country}/{number}");
                clubs += 1;
            }
        }
        assert!(clubs > 200);
    }

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
    /// replica against NewSeason000. Every player still in a squad in both
    /// — each game retires its old men by its own draws — stands with his season's counters as the original's, the same count
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
            let (after, ours_after) = (squad(&theirs, country, number), squad(&ours, country, number));
            // Both games' draws retire the old men their own way.
            for id in squad(&before, country, number).into_iter().filter(|id| after.contains(id) && ours_after.contains(id)) {
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

    /// The played countries' players over the original's pair: closed at
    /// the season's end and opened at the new season's start by the
    /// replica, every player of SeasonEnd364 still at his club in
    /// NewSeason000 stands with his season's counters, his season's form
    /// and last form, his condition and freshness, his reasons, the
    /// retiring bit and the fines as the original's; his count of seasons
    /// too, but for the few whose contract the original renewed
    /// (`FUN_006b3f92`, [`crate::free_agents`], not run here).
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_played_countries_players_roll_over_as_the_original() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let read = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs, before) =
            (read("SeasonEnd364.an3"), read("NewSeason000.an3"), read("SeasonEnd364.an3"));
        let mut random = Random::new(7);
        count_first_seasons(&mut ours);
        close_played(&mut ours, &mut random);
        crate::settlement::open_season(&mut ours, &mut random);
        let (mut compared, mut renewed) = (0, 0);
        for country in crate::opponent::played_nations(&before) {
            for number in 0..before.countries.team_count(usize::from(country)) as u16 {
                let after = squad(&theirs, country, number);
                for id in squad(&before, country, number).into_iter().filter(|id| after.contains(id)) {
                    let (o, x, y) = (before.player(id).unwrap(), ours.player(id).unwrap(), theirs.player(id).unwrap());
                    if o.birthday() != y.birthday() {
                        continue;
                    }
                    compared += 1;
                    for at in OPENED_BYTES.into_iter().chain([0x27b, 0x34d, 0x34f]) {
                        assert_eq!(x.get::<u8>(at), y.get::<u8>(at), "player {id} at {at:#x}");
                    }
                    for at in [0x1e0, 0x1ce, 0x1d0, 0x1d2, 0x1d4, 0x1de] {
                        assert_eq!(x.get::<u16>(at), y.get::<u16>(at), "player {id} at {at:#x}");
                    }
                    assert_eq!(x.flags() & REASONS_ENDED, y.flags() & REASONS_ENDED, "player {id}'s reasons");
                    assert_eq!(x.retiring() & !RETIRING_KEPT, y.retiring() & !RETIRING_KEPT, "player {id}");
                    if x.get::<u8>(SEASONS) != y.get::<u8>(SEASONS) {
                        assert!(y.get::<u8>(SEASONS) <= 2, "player {id}'s seasons");
                        renewed += 1;
                    }
                }
            }
        }
        assert!(compared > 4000, "{compared} players compared");
        assert!(renewed < 10, "{renewed} counts renewed");
    }

    /// The season's end retires the played clubs' players who said they
    /// would, over the original's pair: every one of SeasonEnd364's stands
    /// in no squad afterwards, his retiring bits gone, his record in the
    /// pool or sent on into the regeneration table — and in the original's
    /// NewSeason000 too, but for the records it has already made new
    /// youth players of (not built). The pool's counts keep its entries.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_season_end_retires_as_the_original() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let read = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs, before) =
            (read("SeasonEnd364.an3"), read("NewSeason000.an3"), read("SeasonEnd364.an3"));
        retire_announced(&mut ours, &mut Random::new(3));
        let ids = |r: &Records, name: &str, first: usize, n: usize| -> Vec<u16> {
            let b = r.block(name).unwrap();
            (0..n).map(|k| u16::from_le_bytes([b[first + 4 * k], b[first + 4 * k + 1]])).filter(|&id| id != 0).collect()
        };
        let kept = |r: &Records| {
            let mut v = ids(r, POOL, POOL_ENTRIES, POOL_SIZE);
            v.extend(ids(r, REGENERATION, 0, 9000));
            v
        };
        let (ours_kept, theirs_kept) = (kept(&ours), kept(&theirs));
        let in_squad = |r: &Records, id: u16| r.teams.iter().any(|t| t.players().contains(&id));
        let mut retired = 0;
        for country in crate::opponent::played_nations(&before) {
            for number in 0..before.countries.team_count(usize::from(country)) as u16 {
                for id in squad(&before, country, number) {
                    let p = before.player(id).unwrap();
                    if p.retiring() & RETIRES == 0 {
                        continue;
                    }
                    retired += 1;
                    assert!(!in_squad(&ours, id), "player {id} retired");
                    assert_eq!(ours.player(id).unwrap().retiring(), 0);
                    assert!(id >= POOLED_BELOW || ours_kept.contains(&id), "player {id} pooled");
                    let reused = theirs.player(id).unwrap().birthday() != p.birthday();
                    assert!(reused || !in_squad(&theirs, id), "the original retires {id}");
                    assert!(reused || theirs_kept.contains(&id), "the original pools {id}");
                }
            }
        }
        assert!(retired > 100, "{retired} retired");
        let b = ours.block(POOL).unwrap();
        for class in 0..CLASSES {
            let held = (0..POOL_SIZE)
                .filter(|&k| {
                    let at = POOL_ENTRIES + 4 * k;
                    u16::from_le_bytes([b[at], b[at + 1]]) != 0 && usize::from(b[at + 2]) == class
                })
                .count() as i16;
            assert_eq!(i16::from_le_bytes([b[2 * class], b[2 * class + 1]]), held, "class {class}'s count");
        }
    }

    #[test]
    fn a_career_end_goes_into_the_human_clubs_history() {
        let mut p = player();
        p.set_first_name(11);
        p.set_last_name(22);
        p.set_strength(7);
        p.set_positions([4, 0, 0]);
        let mut extra = crate::records::Extra::new(&vec![0u8; crate::records::Extra::SIZE]).unwrap();
        extra.put::<i16>(0xdde, 100);
        for k in 0..100usize {
            extra.put::<i16>(0xde0 + k * 0x14, 1900 + k as i16);
        }
        let entry = career_end_entry(&p, 2026, 7);
        push_history(&mut extra, entry);
        assert_eq!(extra.get::<i16>(0xdde), 100, "a full history drops its oldest");
        assert_eq!(extra.get::<i16>(0xde0), 1901);
        let e: [u8; 0x14] = extra.get(0xde0 + 99 * 0x14);
        assert_eq!((i16::from_le_bytes([e[0], e[1]]), e[2], e[3]), (2026, 7, CAREER_END));
        assert_eq!((u16::from_le_bytes([e[4], e[5]]), u16::from_le_bytes([e[6], e[7]]), e[0xc], e[0xd]), (11, 22, 7, 4));
    }
}
