//! The transfers agreed for the season's end carried out (`FUN_0070f7cf`,
//! in the season-end chain `FUN_006b3ee0` between the clubs' contract pass
//! and their players' close).
//!
//! The deals stand in the agreed list (`0x1be0b40`, 0x28 bytes each: the
//! player, the selling and the buying club, the fee, the 0x14 bytes of the
//! contract in the player object's layout, a loan's flag). A first pass
//! takes every dealt player from the club that sells him or lends him
//! (`FUN_0070e3f5`): out of its squad and line-up, his career's station
//! closed today, his season's counters and matchdays cleared, the club's
//! will to win moved by what he was to it (`FUN_0071473c`), the fee booked
//! as its sale; a sale to a league below what his contract holds him for is
//! called off (`FUN_00427183`). A second pass brings him to the buyer
//! (`FUN_0070e4c1`) while it has fewer than 35: into its squad by his
//! position, his club's bytes, a new station, the fee booked as its
//! purchase, its will to win moved by him (`FUN_00714688`, twice as the
//! original does); the deal's contract becomes his — a sale's years one
//! more, counted from the closing season, its fee in thousands and his
//! seasons at nought —, his next club cleared, his strength stepping by
//! chance (`FUN_0070e588`) and his old ties at the old club ended
//! (`FUN_0070c99e`). Each human club keeps the deal in its history, the
//! seller as sold, the buyer as come (`FUN_0070ebfb`). The list then stands
//! empty (`FUN_0070f564`).
//!
//! A player whose sale is called off with his contract running out goes
//! to the free agents (`FUN_0040e8cc`, [`execute_with`]).
//!
//! A deal of two millions or more enters the Supertransfers
//! (`FUN_0070eccd`, [`supertransfer`]).
//!
//! Left out: the human clubs' training plans of a player between them (`FUN_00746ddf`),
//! the human clubs' record fees and all-time lists
//! (`FUN_005dbeb7`, `FUN_005dbefc`, `FUN_006d5931`) and the share market's
//! note (`FUN_00405bf3`).

use chrono::NaiveDate;

use crate::finances::CLUB_TO_TEAM;
use crate::random::Random;
use crate::records::{Player, Records, Team};

const AGREED: &str = "0x1be0b40";
const PAGE: usize = 4000;
const PAGE_ENTRIES: usize = 100;
const PAGE_HEAD: usize = 4;
const ENTRY: usize = 0x28;
/// A deal's fields.
const PLAYER: usize = 2;
const SELLER: usize = 4;
const BUYER: usize = 8;
const FEE: usize = 0xc;
const TERMS: usize = 0x10;
const LOAN: usize = 0x25;
/// The club's books of transfers (club offsets): what it sold, what it
/// bought, the net.
const SALES: usize = 0x470;
const PURCHASES: usize = 0x478;
const NET: usize = 0x258;
/// The retiring byte's bit: he ends his career.
const RETIRING: u8 = 0x80;
/// A club takes no more than this many.
const SQUAD_MOST: usize = 35;
/// Fees count in thousands where they are kept.
const THOUSAND: u32 = 1000;
/// R+0x282's bit for a player counted as home-grown.
const HOME_GROWN: u8 = 0x10;
/// The career's stations.
const STATIONS: usize = 0x20;
const STATION: usize = 0x14;
const STATIONS_MOST: i16 = 15;
/// The history's kinds.
const CAME: u8 = 1;
const SOLD: u8 = 2;
/// What his club still owes him, in thousands (R+0x1f0), and where the
/// club books it paid (club +0x284).
const OWED: usize = 0x1f0;
const OWED_PAID: usize = 0x284;
/// The contract's bits: on loan, moved in the season, not to play in the
/// European cups.
const ON_LOAN: u16 = 0x100;
const MOVED: u16 = 0x800;
const NO_EUROPE: u16 = 0x10;
/// The club insures its players itself (club +0x303).
const INSURES: usize = 0x303;
/// A sale above this (`DAT_00a3b308`) a human seller keeps in mind: the
/// player and his strength, in his block `0xa58078` at +0x293a.
const BIG_SALE: i32 = 3_000_000;
const BIG_SALE_AT: usize = 0x293a - 0x20;

/// A deal as the list keeps it.
#[derive(Clone, Copy)]
struct Deal {
    bytes: [u8; ENTRY],
    open: bool,
}

impl Deal {
    fn word(&self, at: usize) -> i16 {
        i16::from_le_bytes([self.bytes[at], self.bytes[at + 1]])
    }

    fn player(&self) -> u16 {
        self.word(PLAYER) as u16
    }

    fn seller(&self) -> (u8, i16) {
        (self.bytes[SELLER], self.word(SELLER + 2))
    }

    fn buyer(&self) -> (u8, i16) {
        (self.bytes[BUYER], self.word(BUYER + 2))
    }

    fn fee(&self) -> u32 {
        u32::from_le_bytes(self.bytes[FEE..FEE + 4].try_into().unwrap())
    }

    fn loan(&self) -> bool {
        self.bytes[LOAN] != 0
    }
}

fn read(records: &Records) -> Vec<Deal> {
    let Some(block) = records.block(AGREED) else { return Vec::new() };
    let count = usize::from(i16::from_le_bytes([block[0], block[1]]) as u16);
    (0..count)
        .filter_map(|k| {
            let at = 4 + k / PAGE_ENTRIES * PAGE + PAGE_HEAD + k % PAGE_ENTRIES * ENTRY;
            let bytes: [u8; ENTRY] = block.get(at..at + ENTRY)?.try_into().ok()?;
            Some(Deal { bytes, open: i16::from_le_bytes([bytes[0], bytes[1]]) != 0 })
        })
        .collect()
}

fn add_money(team: &mut Team, at: usize, by: i64) {
    let at = at + CLUB_TO_TEAM;
    let v = team.get::<i64>(at).wrapping_add(by);
    team.put::<i64>(at, v);
}

fn add_net(team: &mut Team, by: i32) {
    let at = NET + CLUB_TO_TEAM;
    let v = team.get::<i32>(at).wrapping_add(by);
    team.put::<i32>(at, v);
}

/// The will to win a club takes (`FUN_0069efe8`), within ±200.
fn will(team: &mut Team, by: i16) {
    team.set_will_to_win(team.will_to_win().saturating_add(by).clamp(-200, 200));
}

fn today(records: &Records) -> Option<NaiveDate> {
    let [day, month, year] = records.globals.date();
    NaiveDate::from_ymd_opt(i32::from(year), u32::from(month), u32::from(day))
}

/// The selling club's will to win at losing him (`FUN_0071473c`).
fn will_at_leaving(team: &mut Team, p: &Player, age: i32) {
    let [mark, more] = p.traits();
    if more & 0x60 != 0 || mark & 4 != 0 {
        will(team, 5);
    }
    if more & 1 != 0 || mark & 1 != 0 || (age < 23 && p.strength() > 7) {
        will(team, -10);
    }
}

/// The buying club's will to win at his coming (`FUN_00714688`).
fn will_at_coming(team: &mut Team, p: &Player, age: i32) {
    let [mark, more] = p.traits();
    if more & 1 != 0 {
        will(team, 2);
    }
    if mark & 1 != 0 || (age < 23 && p.strength() > 7) {
        will(team, 2);
    } else if more & 0x260 != 0 || mark & 0x804 != 0 {
        will(team, -2);
    } else if team.will() < 10 {
        will(team, 10);
    }
}

/// The level a club plays next season (`FUN_00692bf9` over the drawn
/// lists), as a class of the leagues (`FUN_006755b8`); 10 none.
pub(crate) fn next_class(records: &Records, country: u8, team: i16) -> i16 {
    let main = records.globals.main_country();
    let next = crate::promotion::next_level(records, u16::from(country), team);
    if next >= 10 {
        return next;
    }
    if i16::from(country) == main {
        if country == 0x10 { next.min(3) } else { next.min(2) }
    } else if next > 0 {
        0
    } else {
        next
    }
}

/// Whether he plays for the buyer: the deal's contract holds him for no
/// league below its clause (`FUN_00427183` with `FUN_006fe85e` on the
/// deal's terms: their word's low two bits).
fn accepts(records: &Records, deal: &Deal) -> bool {
    let clause = deal.word(TERMS + 0xe) & 3;
    let buyer = deal.buyer();
    clause == 0 || next_class(records, buyer.0, buyer.1) < clause
}

/// Today into a station's date.
fn date_bytes(records: &Records) -> [u8; 4] {
    let [day, month, year] = records.globals.date();
    let y = year.to_le_bytes();
    [day as u8, month as u8, y[0], y[1]]
}

/// His last station ends today (`FUN_0070429c`).
pub(crate) fn close_station(records: &mut Records, id: u16) {
    let date = date_bytes(records);
    let Some(p) = records.player_mut(id) else { return };
    let stations = p.career_count();
    if stations > 0 {
        let at = STATIONS + usize::try_from(stations - 1).unwrap_or(0) * STATION + 4;
        for (k, b) in date.iter().enumerate() {
            p.put::<u8>(at + k, *b);
        }
    }
}

/// His season left behind at the club (`FUN_00707bbf`: the station's end
/// date, `FUN_0070429c`; the counters and matchdays, `FUN_00706821`).
fn leave_season(records: &mut Records, id: u16) {
    close_station(records, id);
    let Some(p) = records.player_mut(id) else { return };
    for at in [0x244, 0x243, 0x242, 0x241, 0x248, 0x247, 0x245, 0x255, 0x254, 0x253, 0x25e, 0x352] {
        p.put::<u8>(at, 0);
    }
    p.set_matchday_goals([0; 46]);
    p.set_matchdays([0; 46]);
}

/// A new station at the club (`FUN_00704109`): today, no end, the fee in
/// thousands, the club, his strength; fifteen kept, the oldest gone.
pub(crate) fn open_station(records: &mut Records, id: u16, club: (u8, i16), fee: u32) {
    open_station_as(records, id, club, fee, 0);
}

/// The same with the station's kind at +0x12 (`FUN_00704109`'s fourth
/// argument: 1 for a transfer at once).
fn open_station_as(records: &mut Records, id: u16, club: (u8, i16), fee: u32, kind: u8) {
    let date = date_bytes(records);
    let Some(p) = records.player_mut(id) else { return };
    let mut count = p.career_count().clamp(0, STATIONS_MOST);
    if count == STATIONS_MOST {
        for k in 0..usize::from(STATIONS_MOST as u16) - 1 {
            for b in 0..STATION {
                let v = p.get::<u8>(STATIONS + (k + 1) * STATION + b);
                p.put::<u8>(STATIONS + k * STATION + b, v);
            }
        }
        count -= 1;
    }
    let at = STATIONS + usize::try_from(count).unwrap_or(0) * STATION;
    let mut s = [0u8; STATION];
    s[0..4].copy_from_slice(&date);
    s[8..12].copy_from_slice(&(fee / THOUSAND).to_le_bytes());
    s[0x10] = club.0;
    s[0x11] = club.1 as u8;
    s[0x12] = kind;
    s[0x13] = p.strength();
    for (k, b) in s.iter().enumerate() {
        p.put::<u8>(at + k, *b);
    }
    p.set_career_count(count + 1);
}

/// A human club's history of a deal (`FUN_0070ebfb`, `FUN_0070eb02`): the
/// year, week 0xff, the kind, his names, the other club, his strength and
/// position, the fee in thousands.
fn note(records: &mut Records, club: (u8, i16), id: u16, kind: u8, other: [u8; 4], fee: u32) {
    note_in(records, club, id, 0xff, kind, other, fee);
}

/// The same in a week of the season (`FUN_0070e8c4`, the day's seventh).
fn note_in(records: &mut Records, club: (u8, i16), id: u16, week: u8, kind: u8, other: [u8; 4], fee: u32) {
    let Some(slot) = u16::try_from(club.1)
        .ok()
        .and_then(|n| records.team(u16::from(club.0), n))
        .and_then(|t| usize::from(t.human()).checked_sub(1))
    else {
        return;
    };
    let Some(p) = records.player(id) else { return };
    let mut e = [0u8; 0x14];
    e[0..2].copy_from_slice(&records.globals.start_year().to_le_bytes());
    e[2] = week;
    e[3] = kind;
    e[4..6].copy_from_slice(&p.first_name().to_le_bytes());
    e[6..8].copy_from_slice(&p.last_name().to_le_bytes());
    e[8..12].copy_from_slice(&other);
    e[0xc] = p.strength();
    e[0xd] = p.positions()[0];
    e[0x10..0x14].copy_from_slice(&(fee / THOUSAND).to_le_bytes());
    if let Some(extra) = records.extras.get_mut(slot) {
        crate::rollover::push_history(extra, e);
    }
}

/// Into the buyer's squad by his main position, after those who play it
/// or before (`FUN_0070df0b`), his club's bytes the buyer's.
fn join_squad(team: &mut Team, records_positions: &dyn Fn(u16) -> u8, id: u16, position: u8) {
    let mut squad = team.players();
    let mut at = squad.len();
    while at > 0 && position < records_positions(squad[at - 1]) {
        at -= 1;
    }
    squad.insert(at, id);
    team.set_players(&squad);
}

/// His strength's step at the new club (`FUN_0070e588`): one in a thousand
/// two up, else one in fifteen one up, else one in fifteen one down (above
/// 1), else one in a thousand two down (above 2); within his talent, a 13
/// keeps 13.
fn step_strength(p: &mut Player, random: &mut Random) {
    let old = p.strength();
    let _ = random.below(100);
    let mut s = old;
    if random.below(1000) == 0 {
        s += 2;
    } else if random.below(15) == 0 {
        s += 1;
    } else if random.below(15) == 0 {
        if s > 1 {
            s -= 1;
        }
    } else if random.below(1000) == 0 && s > 2 {
        s -= 2;
    }
    if s == 0 {
        s = 1;
    }
    let top = crate::rollover::top_strength(p.talent());
    if s < 13 || old != 13 {
        if top <= s {
            s = top;
        }
    } else {
        s = 13;
    }
    p.set_strength(s);
}

/// His old ties ended at the new club (`FUN_0070c99e`): the list bits,
/// his absence's mark, suspension, reasons, R+0x1f4, the fines, the sick
/// visits, the ban's weeks, R+0x27a; his contentment with the trainer, the
/// team and the club at 100; the problems and several bits of R+0x1b4,
/// R+0x27f, R+0x280 and R+0x297 cleared.
pub(crate) fn new_ties(p: &mut Player, absent: bool) {
    p.set_retiring(p.retiring() & 0x3a);
    if !absent {
        p.set_away_kind(0);
    } else {
        p.set_away_kind(p.away_kind() & 0xef);
    }
    p.set_suspension(0);
    p.set_flags(0);
    p.put::<u16>(0x1f4, 0);
    p.set_fined(0);
    p.set_visits(0);
    p.set_banned_weeks(0);
    p.put::<u8>(0x27a, 0);
    let mut parts = p.contentment();
    parts[0..3].copy_from_slice(&[100; 3]);
    p.set_contentment(parts);
    p.set_morale(crate::mood::morale(p));
    p.set_problems(p.problems() & 0xfc);
    p.set_shown(p.shown() & !(0x4000_0000 | 0x8000 | 0x1_0000 | 0x2_0000 | 0x4_0000));
    p.put::<u8>(0x280, p.get::<u8>(0x280) & 0xec & 0xef);
    p.set_offences(0);
    p.set_problems(p.problems() & 0xf3);
    p.set_morale(crate::mood::morale(p));
}

/// A club lets a player go (`FUN_0070e3f5`): its will to win moved by
/// him, his station closed and his season left behind, the fee into its
/// sales and its net, out of its squad, line-up and bench; to a club of
/// another country no more home-grown.
pub(crate) fn let_go(records: &mut Records, club: (u8, i16), id: u16, fee: u32, buyer: Option<u8>) {
    let Ok(number) = u16::try_from(club.1) else { return };
    let age = today(records).and_then(|d| records.player(id).map(|p| crate::statistics::age_on(p, d))).unwrap_or(0);
    let p = records.player(id).cloned();
    if let (Some(team), Some(p)) = (records.team_mut(u16::from(club.0), number), p.as_ref()) {
        will_at_leaving(team, p, age);
    }
    leave_season(records, id);
    if let Some(team) = records.team_mut(u16::from(club.0), number) {
        add_money(team, SALES, i64::from(fee));
        add_net(team, fee as i32);
        let squad: Vec<u16> = team.players().into_iter().filter(|&p| p != id).collect();
        team.set_players(&squad);
        let mut lineup = team.lineup();
        lineup.iter_mut().filter(|s| **s == id).for_each(|s| *s = 0);
        team.set_lineup(lineup);
        let mut bench = team.bench();
        bench.iter_mut().filter(|s| **s == id).for_each(|s| *s = 0);
        team.set_bench(bench);
    }
    if let Some(country) = buyer.filter(|&c| c != 0 && c != club.0) {
        let _ = country;
        if let Some(p) = records.player_mut(id) {
            p.set_manner(p.manner() & !HOME_GROWN);
        }
    }
}

/// A club takes a player in (`FUN_0070e4c1`): its will to win moved by
/// him, off its watch list, the fee out of its net into its purchases,
/// into its squad by his position with its bytes, a new station.
pub(crate) fn bring_in(records: &mut Records, club: (u8, i16), id: u16, fee: u32) {
    let Ok(number) = u16::try_from(club.1) else { return };
    let age = today(records).and_then(|d| records.player(id).map(|p| crate::statistics::age_on(p, d))).unwrap_or(0);
    let Some(p) = records.player(id).cloned() else { return };
    let position = p.positions()[0];
    let positions: Vec<(u16, u8)> = records
        .team(u16::from(club.0), number)
        .map(|t| t.players().into_iter().map(|q| (q, records.player(q).map_or(0, |r| r.positions()[0]))).collect())
        .unwrap_or_default();
    let position_of = |q: u16| positions.iter().find(|e| e.0 == q).map_or(0, |e| e.1);
    if let Some(team) = records.team_mut(u16::from(club.0), number) {
        will_at_coming(team, &p, age);
        add_net(team, -(fee as i32));
        add_money(team, PURCHASES, i64::from(fee));
        join_squad(team, &position_of, id, position);
    }
    crate::transfers::unwatch_at_club(records, club, id);
    if let Some(p) = records.player_mut(id) {
        p.set_club([club.0, club.1 as u8]);
    }
    open_station(records, id, club, fee);
}

/// A club's fields as the deals name them: country, 0, number.
/// The Supertransfers (fifty of 0x1c bytes at `DAT_00b1b2e8`, the lists
/// block's +0xfc30, the dearest first).
const SUPERTRANSFERS: usize = 0xfc30;
const SUPERTRANSFER: usize = 0x1c;
const SUPERTRANSFERS_KEPT: usize = 50;
/// A fee below this never enters them (`DAT_00a3b314`).
const SUPERTRANSFER_LEAST: i32 = 2_000_000;
/// They keep fees in ten thousands (`DAT_00a3b3bc`).
const TEN_THOUSAND: i32 = 10_000;

/// A transfer entered in the Supertransfers (`FUN_0070eccd`, after the
/// buyer's history, mid-season through `FUN_0070e82e`/`FUN_0070ee57`): a
/// fee of two millions or more, in ten thousands no less than the
/// fiftieth's, goes in above every entry it is not below, those moving one
/// down and the fiftieth falling out. The entry: the player, his first
/// and last name — none and his nickname when he is known by it (+0x1b4
/// bit 31) —, the fee, the club he came from (`from`, a free player's
/// nation and 200), the buyer's country byte and number, his position and
/// strength; the bytes between (+6, +0x15, +0x1a) stay as the slot held
/// them.
pub(crate) fn supertransfer(records: &mut Records, id: u16, from: [u8; 4], buyer: (u8, i16), fee: u32) {
    let fee = fee as i32;
    if fee < SUPERTRANSFER_LEAST {
        return;
    }
    let fee = fee / TEN_THOUSAND;
    let Some(p) = records.player(id) else { return };
    let names = if p.shown() & 0x8000_0000 != 0 { [0, p.nickname()] } else { [p.first_name(), p.last_name()] };
    let (position, strength) = (p.positions()[0], p.strength());
    let Some(b) = records
        .block_mut(crate::statistics::LISTS)
        .and_then(|b| b.get_mut(SUPERTRANSFERS..SUPERTRANSFERS + SUPERTRANSFERS_KEPT * SUPERTRANSFER))
    else {
        return;
    };
    let fee_at = |b: &[u8], k: usize| i32::from_le_bytes(b[k * SUPERTRANSFER + 0xc..k * SUPERTRANSFER + 0x10].try_into().unwrap());
    if fee_at(b, SUPERTRANSFERS_KEPT - 1) > fee {
        return;
    }
    let mut k = SUPERTRANSFERS_KEPT - 1;
    while k > 0 && fee_at(b, k - 1) <= fee {
        b.copy_within((k - 1) * SUPERTRANSFER..k * SUPERTRANSFER, k * SUPERTRANSFER);
        k -= 1;
    }
    let e = &mut b[k * SUPERTRANSFER..(k + 1) * SUPERTRANSFER];
    e[0..2].copy_from_slice(&id.to_le_bytes());
    e[2..4].copy_from_slice(&names[0].to_le_bytes());
    e[4..6].copy_from_slice(&names[1].to_le_bytes());
    e[0xc..0x10].copy_from_slice(&fee.to_le_bytes());
    e[0x10..0x14].copy_from_slice(&from);
    e[0x14] = buyer.0;
    e[0x16..0x18].copy_from_slice(&u16::from(buyer.1 as u8).to_le_bytes());
    e[0x18] = position;
    e[0x19] = strength;
}

fn side_bytes(club: (u8, i16)) -> [u8; 4] {
    let mut s = [0u8; 4];
    s[0] = club.0;
    s[2..4].copy_from_slice(&club.1.to_le_bytes());
    s
}

/// What the selling club still owes him goes with him (`FUN_0070ca69`:
/// R+0x1f0, in thousands, into the club's +0x284 and his earnings).
fn settle_owed(records: &mut Records, seller: (u8, i16), id: u16) {
    let Some(p) = records.player_mut(id) else { return };
    let owed = i32::from(p.get::<u16>(OWED)) * THOUSAND as i32;
    p.set_earned(p.earned().wrapping_add(owed));
    p.put::<u16>(OWED, 0);
    if let Some(team) = u16::try_from(seller.1).ok().and_then(|n| records.team_mut(u16::from(seller.0), n)) {
        let at = OWED_PAID + CLUB_TO_TEAM;
        let v = team.get::<i32>(at).wrapping_add(owed);
        team.put::<i32>(at, v);
    }
}

/// The number a computer club gives him (`FUN_006d98a0`): the first free
/// of his position's three, else one to forty drawn and the next free
/// from there.
fn shirt_at(records: &Records, club: (u8, i16), id: u16, random: &mut Random) -> Option<u8> {
    let team = records.team(u16::from(club.0), u16::try_from(club.1).ok()?)?;
    let position = usize::from(records.player(id)?.positions()[0]);
    let mut worn = [false; 256];
    for q in team.players() {
        if let Some(r) = records.player(q) {
            worn[usize::from(r.shirt())] = true;
        }
    }
    let choices = crate::settlement::SHIRT_CHOICES.get(position.checked_sub(1)? * 3..position * 3)?;
    if let Some(&n) = choices.iter().find(|&&n| !worn[usize::from(n)]) {
        return Some(n);
    }
    let mut n = random.below(40) as usize + 1;
    while n < 255 && worn[n] {
        n += 1;
    }
    Some(n as u8)
}

/// A transfer carried out at once (`FUN_0070ca69`, its last argument
/// set): each human club's history keeps it in the season's week
/// (`FUN_0070e8c4`), his season at the old club is left behind, his
/// strength steps by chance at a computer club (`FUN_0070e588`), the
/// contract becomes his (`FUN_0070db4b`: no next club, his line-up marks
/// and intensity fresh; his cards and bans gone with the country, his
/// season's goals with the country or the league), he changes squads, the
/// fee is booked at both clubs and kept as what he cost, what the seller
/// owed him is paid, he leaves the transfer list and is marked as moved
/// in the season (R+0x15e 0x800); both squads' best by position are
/// reckoned anew, the clubs' will to win moves, a computer club numbers
/// him, his old ties end and a new station opens. Outside July and August
/// he may not play in the European cups (0x10) — within a country, or
/// between clubs not linked (team +0x872); a club that insures its
/// players itself (+0x303) insures him by his salary, and a human seller
/// keeps a sale above three millions in mind.
///
/// Left out: the paper's lines for a man of strength 11 up from a first
/// league (ereignis 0x684, 0x685), the records
/// (`FUN_006d5931`), a human buyer's strength step and
/// message (`FUN_0070e6a3`), the human clubs' training plans
/// (`FUN_00746ddf`) and the list of the week's moves (`FUN_00759eb9`, not
/// saved). Then both clubs plan their search anew
/// ([`crate::search_plan::after_transfer`]).
pub(crate) fn move_now(
    records: &mut Records,
    random: &mut Random,
    id: u16,
    seller: (u8, i16),
    buyer: (u8, i16),
    fee: u32,
    terms: &[u8; 0x14],
) {
    carry_out(records, random, id, seller, buyer, fee, terms);
    crate::search_plan::after_transfer(records, random, seller, buyer, fee, u16::from_le_bytes([terms[0], terms[1]]));
}

/// The move itself (`FUN_0070ca69`'s branch for at once).
fn carry_out(
    records: &mut Records,
    random: &mut Random,
    id: u16,
    seller: (u8, i16),
    buyer: (u8, i16),
    fee: u32,
    terms: &[u8; 0x14],
) {
    let (Ok(seller_n), Ok(buyer_n)) = (u16::try_from(seller.1), u16::try_from(buyer.1)) else { return };
    let (seller_c, buyer_c) = (u16::from(seller.0), u16::from(buyer.0));
    let (Some(from), Some(to)) = (records.team(seller_c, seller_n), records.team(buyer_c, buyer_n)) else { return };
    let levels = [from.level(), to.level()];
    let linked = [from, to].map(|t| {
        if records.globals.played(i16::from(t.country())) { i16::from(t.linked()[0]) } else { -1 }
    });
    let (human_buyer, numbered) = (to.human() != 0, to.humans()[0] == 0);
    let insures = to.get::<u8>(INSURES + CLUB_TO_TEAM) != 0;
    let seller_slot = usize::from(from.human());
    let week = (records.globals.day() / 7) as u8;
    let year = i32::from(records.globals.start_year());
    let month = records.globals.date()[1];
    note_in(records, seller, id, week, SOLD, side_bytes(buyer), fee);
    note_in(records, buyer, id, week, CAME, side_bytes(seller), fee);
    supertransfer(records, id, side_bytes(seller), buyer, fee);
    leave_season(records, id);
    let absent = records.player(id).is_some_and(|p| {
        let [y, d] = p.away_to();
        let (now, day) = (records.globals.date()[2], records.globals.day_of_year());
        y > now || (y == now && d > day)
    });
    let word = |at: usize| u16::from_le_bytes([terms[at], terms[at + 1]]);
    let Some(p) = records.player_mut(id) else { return };
    if !human_buyer {
        step_strength(p, random);
    }
    p.put::<u8>(0x14c, 0);
    p.put::<u16>(0x14e, 0);
    p.set_salary(word(0));
    p.set_appearance_bonus(word(2));
    p.set_goal_bonus(word(4));
    p.set_contract((i32::from(terms[0xc] as i8) + year) as u16);
    p.put::<u8>(0x161, 0);
    if word(0xe) & ON_LOAN == 0 {
        p.set_fixed_fee(word(6));
    } else {
        p.set_fixed_fee(0);
    }
    p.set_terms(word(0xe));
    p.set_retiring(p.retiring() & 0x3f);
    p.set_picks(0);
    p.set_intensity(3);
    p.set_matches_in_a_row(0);
    if seller.0 != buyer.0 {
        for at in [0x22c, 0x22d, 0x22e, 0x22f, 0x230, 0x231, 0x234, 0x23d, 0x253, 0x25e] {
            p.put::<u8>(at, 0);
        }
        p.set_manner(p.manner() & !HOME_GROWN);
    }
    if seller.0 != buyer.0 || levels[0] != levels[1] {
        for at in (0x241..=0x252).chain([0x23c, 0x23d]) {
            p.put::<u8>(at, 0);
        }
    }
    p.set_flags(0);
    p.put::<u32>(0x158, fee / THOUSAND);
    let position = p.positions()[0];
    // The squads (`FUN_0070df0b`, `FUN_0070e01a`, `FUN_0070d9c5`).
    let positions: Vec<(u16, u8)> = records
        .team(buyer_c, buyer_n)
        .map(|t| t.players().into_iter().map(|q| (q, records.player(q).map_or(0, |r| r.positions()[0]))).collect())
        .unwrap_or_default();
    let position_of = |q: u16| positions.iter().find(|e| e.0 == q).map_or(0, |e| e.1);
    if let Some(team) = records.team_mut(buyer_c, buyer_n) {
        join_squad(team, &position_of, id, position);
        add_money(team, PURCHASES, i64::from(fee as i32));
        add_net(team, (fee as i32).wrapping_neg());
    }
    if let Some(p) = records.player_mut(id) {
        p.set_club([buyer.0, buyer.1 as u8]);
    }
    if let Some(team) = records.team_mut(seller_c, seller_n) {
        let squad: Vec<u16> = team.players().into_iter().filter(|&q| q != id).collect();
        team.set_players(&squad);
        let mut lineup = team.lineup();
        lineup.iter_mut().filter(|s| **s == id).for_each(|s| *s = 0);
        team.set_lineup(lineup);
        let mut bench = team.bench();
        bench.iter_mut().filter(|s| **s == id).for_each(|s| *s = 0);
        team.set_bench(bench);
        add_money(team, SALES, i64::from(fee as i32));
        add_net(team, fee as i32);
    }
    settle_owed(records, seller, id);
    crate::transfers::take_off(records, id);
    if let Some(p) = records.player_mut(id) {
        p.set_terms(p.terms() | MOVED);
    }
    crate::squad_rank::keep_best(records, seller_c, seller_n);
    crate::squad_rank::keep_best(records, buyer_c, buyer_n);
    let age = today(records).and_then(|d| records.player(id).map(|p| crate::statistics::age_on(p, d))).unwrap_or(0);
    let now = records.player(id).cloned();
    if let (Some(team), Some(p)) = (records.team_mut(seller_c, seller_n), now.as_ref()) {
        will_at_leaving(team, p, age);
    }
    if let (Some(team), Some(p)) = (records.team_mut(buyer_c, buyer_n), now.as_ref()) {
        will_at_coming(team, p, age);
    }
    crate::transfers::unwatch_at_club(records, buyer, id);
    if let Some(p) = records.player_mut(id) {
        p.set_shirt(0);
    }
    if numbered
        && let Some(n) = shirt_at(records, buyer, id, random)
        && let Some(p) = records.player_mut(id)
    {
        p.set_shirt(n);
    }
    if let Some(p) = records.player_mut(id) {
        new_ties(p, absent);
    }
    open_station_as(records, id, buyer, fee, 1);
    let Some(p) = records.player_mut(id) else { return };
    if !(7..=8).contains(&month)
        && (seller.0 == buyer.0 || (linked[1] != seller.1 && linked[0] != buyer.1))
    {
        p.set_terms(p.terms() | NO_EUROPE);
    }
    if insures {
        let salary = u32::from(p.salary());
        let premium = match p.traits()[1] {
            t if t & 8 != 0 => salary / 5,
            t if t & 4 != 0 => salary * 3 / 20,
            _ => salary / 10,
        };
        p.put::<i16>(0x1f4, premium.max(1) as i16);
    }
    if fee as i32 > BIG_SALE
        && (1..=4).contains(&seller_slot)
        && let Some(strength) = records.player(id).map(Player::strength)
        && let Some(b) = records.block_mut(&format!("0xa58078[{}]", seller_slot - 1))
        && let Some(e) = b.get_mut(BIG_SALE_AT..BIG_SALE_AT + 4)
    {
        e[0..2].copy_from_slice(&id.to_le_bytes());
        e[2..4].copy_from_slice(&u16::from(strength).to_le_bytes());
    }
}

/// A transfer agreed for the season's end (`FUN_0070ca69`, its last
/// argument nought): the deal joins the list (`FUN_0070f717`,
/// `FUN_0070f25a`: the player, the clubs, the fee, the contract, the
/// buyer's league less the seller's, each the third at most; the list's
/// first page of a hundred only), what the seller owed him is paid, the
/// buyer is his next club, he leaves the transfer list and plays on
/// (R+0x26f bit 7 cleared). Left out as at once, and the paper's line
/// for a free man of strength 11 up (ereignis 0x683) and the share
/// market's note of the buyer (`FUN_00405bf3`); the clubs' counts of
/// deals and sums (+0xc06, +0xc08, +0x9ec) the save does not hold. Then
/// both clubs plan their search anew, as at once.
pub(crate) fn agree(
    records: &mut Records,
    random: &mut Random,
    id: u16,
    seller: (u8, i16),
    buyer: (u8, i16),
    fee: u32,
    terms: &[u8; 0x14],
) -> bool {
    let level = |c: (u8, i16)| {
        u16::try_from(c.1).ok().and_then(|n| records.team(u16::from(c.0), n)).map_or(0, |t| t.level().min(2))
    };
    let step = level(buyer).wrapping_sub(level(seller));
    let mut booked = false;
    if let Some(b) = records.block_mut(AGREED)
        && b.len() >= 4 + PAGE_HEAD
    {
        let word = |b: &[u8], at: usize| i16::from_le_bytes([b[at], b[at + 1]]);
        let (total, page, n) = (word(b, 0), word(b, 2), word(b, 4));
        let at = 4 + PAGE_HEAD + usize::try_from(n).unwrap_or(PAGE_ENTRIES) * ENTRY;
        if total >= 0 && page == 0 && (0..PAGE_ENTRIES as i16).contains(&n) && at + ENTRY <= b.len() {
            let e = &mut b[at..at + ENTRY];
            e[0..2].copy_from_slice(&1i16.to_le_bytes());
            e[PLAYER..PLAYER + 2].copy_from_slice(&id.to_le_bytes());
            e[SELLER] = seller.0;
            e[SELLER + 2..SELLER + 4].copy_from_slice(&seller.1.to_le_bytes());
            e[BUYER] = buyer.0;
            e[BUYER + 2..BUYER + 4].copy_from_slice(&buyer.1.to_le_bytes());
            e[FEE..FEE + 4].copy_from_slice(&fee.to_le_bytes());
            e[TERMS..TERMS + 0x14].copy_from_slice(terms);
            e[0x24] = step;
            e[LOAN] = 0;
            b[4..6].copy_from_slice(&(n + 1).to_le_bytes());
            b[0..2].copy_from_slice(&(total + 1).to_le_bytes());
            booked = true;
        }
    }
    settle_owed(records, seller, id);
    crate::transfers::take_off(records, id);
    if let Some(p) = records.player_mut(id) {
        p.put::<u8>(0x14c, buyer.0);
        p.put::<i16>(0x14e, buyer.1);
        p.set_retiring(p.retiring() & !RETIRING);
    }
    crate::search_plan::after_transfer(records, random, seller, buyer, fee, u16::from_le_bytes([terms[0], terms[1]]));
    booked
}

/// Carries out the season's agreed transfers (`FUN_0070f7cf`).
pub fn execute(records: &mut Records, random: &mut Random) {
    let mut agents = Vec::new();
    execute_with(records, random, &mut agents);
}

/// The season's agreed transfers carried out, a player whose sale is
/// called off and whose contract no longer holds him at his club let go
/// onto the free agents' list `agents` — or ending his career when he
/// retires or the list is full (`FUN_004271fb`, `FUN_0040e8cc`,
/// `FUN_0070e29e`), his human club noting him gone to no club.
pub fn execute_with(records: &mut Records, random: &mut Random, agents: &mut Vec<crate::free_agents::FreeAgent>) {
    let mut deals = read(records);
    let year = i32::from(records.globals.start_year());
    let age_of = |records: &Records, id: u16| -> i32 {
        today(records).and_then(|d| records.player(id).map(|p| crate::statistics::age_on(p, d))).unwrap_or(0)
    };
    // The sellers let them go.
    for deal in deals.iter_mut().filter(|d| d.open && d.seller().0 != 0) {
        let (id, seller, buyer) = (deal.player(), deal.seller(), deal.buyer());
        let Ok(number) = u16::try_from(seller.1) else { deal.open = false; continue };
        let in_squad = records.team(u16::from(seller.0), number).is_some_and(|t| t.players().contains(&id));
        if !in_squad {
            deal.open = false;
            continue;
        }
        if !deal.loan() && !accepts(records, deal) {
            deal.open = false;
            let class = next_class(records, seller.0, seller.1);
            let holds = records.player(id).is_some_and(|p| crate::free_agents::holds(records, p, class));
            if !holds {
                let_go(records, seller, id, 0, Some(buyer.0));
                let retires = records.player(id).is_some_and(|p| p.retiring() & RETIRING != 0);
                if !retires && crate::free_agents::takes(agents) {
                    note(records, seller, id, SOLD, [0; 4], 0);
                    agents.push(crate::free_agents::FreeAgent { id, country: seller.0, team: seller.1 });
                } else {
                    crate::rollover::retire(records, u16::from(seller.0), number, id, random);
                }
            }
            continue;
        }
        let fee = deal.fee();
        let_go(records, seller, id, fee, Some(buyer.0));
        note(records, seller, id, SOLD, deal.bytes[BUYER..BUYER + 4].try_into().unwrap(), fee);
    }
    // The buyers take them.
    for deal in deals.iter_mut() {
        if !deal.open {
            continue;
        }
        let (id, buyer) = (deal.player(), deal.buyer());
        let Ok(number) = u16::try_from(buyer.1) else { continue };
        let Some(size) = records.team(u16::from(buyer.0), number).map(|t| t.players().len()) else { continue };
        if size >= SQUAD_MOST || !accepts(records, deal) {
            continue;
        }
        let (fee, loan) = (deal.fee(), deal.loan());
        let age = age_of(records, id);
        let Some(p) = records.player(id).cloned() else { continue };
        let position = p.positions()[0];
        let positions: Vec<(u16, u8)> = records
            .team(u16::from(buyer.0), number)
            .map(|t| t.players().into_iter().map(|q| (q, records.player(q).map_or(0, |r| r.positions()[0]))).collect())
            .unwrap_or_default();
        let position_of = |q: u16| positions.iter().find(|e| e.0 == q).map_or(0, |e| e.1);
        let _ = (&p, position, &position_of);
        bring_in(records, buyer, id, fee);
        let absent = records.player(id).is_some_and(|p| {
            let [y, d] = p.away_to();
            let (now, day) = (records.globals.date()[2], records.globals.day_of_year());
            y > now || (y == now && d > day)
        });
        let t = &deal.bytes[TERMS..TERMS + 0x14];
        let word = |at: usize| u16::from_le_bytes([t[at], t[at + 1]]);
        if let Some(p) = records.player_mut(id) {
            p.set_salary(word(0));
            p.set_appearance_bonus(word(2));
            p.set_goal_bonus(word(4));
            p.set_fixed_fee(word(6));
            p.put::<u32>(0x158, u32::from_le_bytes(t[8..12].try_into().unwrap()));
            p.set_terms(word(0xe));
            p.put::<u8>(0x161, t[0x10]);
            p.put::<u8>(0x160, t[0x11]);
            let mut years = t[0xc] as i8;
            if !loan {
                years = years.wrapping_add(1);
                p.put::<u32>(0x158, fee / THOUSAND);
                p.put::<u8>(0x161, 0);
            }
            p.set_contract((i32::from(years) + year) as u16);
            p.put::<u8>(0x14c, 0);
            p.put::<u16>(0x14e, 0);
            step_strength(p, random);
            new_ties(p, absent);
        }
        let now = records.player(id).cloned();
        if let (Some(team), Some(p)) = (records.team_mut(u16::from(buyer.0), number), now.as_ref()) {
            will_at_coming(team, p, age);
        }
        crate::transfers::unwatch_at_club(records, buyer, id);
        let seller = if deal.seller().0 == 0 {
            let nation = records.player(id).map_or(0, |p| p.club()[0]);
            let mut s = [0u8; 4];
            s[0] = nation;
            s[2..4].copy_from_slice(&200i16.to_le_bytes());
            s
        } else {
            deal.bytes[SELLER..SELLER + 4].try_into().unwrap()
        };
        note(records, buyer, id, CAME, seller, fee);
        supertransfer(records, id, seller, buyer, fee);
    }
    // The list stands empty (`FUN_0070f564`).
    if let Some(block) = records.block_mut(AGREED) {
        block[0..2].copy_from_slice(&0i16.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    /// Over the original's season end (SeasonEnd364): Rönnow's sale to a
    /// club whose league his contract does not hold is called off, and
    /// with his contract not holding him at Union either he goes to the
    /// free agents, Union's history noting him gone to no club, as the
    /// original's NewSeason000 has him.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_called_off_sale_lets_the_player_go_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs) = (load("SeasonEnd364.an3"), load("NewSeason000.an3"));
        let mut random = Random::new(3);
        let mut agents = crate::free_agents::release(&mut ours, &mut random);
        const RONNOW: u16 = 3257;
        assert!(ours.team(14, 84).unwrap().players().contains(&RONNOW), "the release keeps him");
        execute_with(&mut ours, &mut random, &mut agents);
        assert!(!ours.team(14, 84).unwrap().players().contains(&RONNOW));
        assert!(!theirs.team(14, 84).unwrap().players().contains(&RONNOW));
        assert!(agents.iter().any(|a| a.id == RONNOW && (a.country, a.team) == (14, 84)));
        let entry = |r: &Records| -> Option<[u8; 0x14]> {
            let e = r.extras.get(0)?;
            (0..e.get::<i16>(0xdde).max(0) as usize).map(|k| e.get::<[u8; 0x14]>(0xde0 + k * 0x14)).find(|a| {
                r.player(RONNOW).is_some_and(|p| u16::from_le_bytes([a[4], a[5]]) == p.first_name()) && a[2] == 0xff
            })
        };
        // The other club's second byte the original leaves as its stack
        // held it (`local_1c`'s unset byte).
        let clean = |e: Option<[u8; 0x14]>| e.map(|mut a| { a[9] = 0; a });
        assert_eq!(clean(entry(&ours)), clean(entry(&theirs)));
    }

    /// Over the original's season end (SeasonEnd364 to NewSeason000): the
    /// deals carried out enter the Supertransfers as the original's list
    /// stands after them, entry for entry and byte for byte.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_season_ends_deals_enter_the_supertransfers_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs, before) =
            (load("SeasonEnd364.an3"), load("NewSeason000.an3"), load("SeasonEnd364.an3"));
        let list = |r: &Records| {
            r.block(crate::statistics::LISTS).unwrap()[SUPERTRANSFERS..SUPERTRANSFERS + SUPERTRANSFERS_KEPT * SUPERTRANSFER]
                .to_vec()
        };
        assert_ne!(list(&before), list(&theirs), "the season end changes the list");
        let mut random = Random::new(3);
        let mut agents = crate::free_agents::release(&mut ours, &mut random);
        execute_with(&mut ours, &mut random, &mut agents);
        // His strength as it stepped by chance at the buyer (`FUN_0070e588`)
        // aside where the draws part.
        let mut stepped = 0;
        for (k, (a, b)) in list(&ours).chunks(SUPERTRANSFER).zip(list(&theirs).chunks(SUPERTRANSFER)).enumerate() {
            let id = u16::from_le_bytes([a[0], a[1]]);
            let (mut a, mut b) = (a.to_vec(), b.to_vec());
            if a != b && ours.player(id).map(|p| p.strength()) != theirs.player(id).map(|p| p.strength()) {
                (a[0x19], b[0x19]) = (0, 0);
                stepped += 1;
            }
            assert_eq!(a, b, "entry {k}");
        }
        assert!(stepped < 5, "{stepped} strengths drawn apart");
    }

    use super::*;

    /// The season's deals over the original's pair: SeasonEnd364 run
    /// through the season end's steps before them and its deals carried
    /// out by the replica, every deal the original carried out has its
    /// player in the buyer's squad, with the buyer's bytes, the deal's
    /// contract and years, the next club cleared, a new station at the
    /// buyer and his season's matchdays gone, as NewSeason000 keeps him;
    /// the one sale both call off and the original's free agents, placed
    /// behind it ([`crate::free_agents`]), aside.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_season_end_carries_out_its_deals_as_the_original() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs, before) =
            (load("SeasonEnd364.an3"), load("NewSeason000.an3"), load("SeasonEnd364.an3"));
        let mut random = Random::new(3);
        let _ = crate::promotion::draw(&mut ours, &mut random);
        crate::promotion::count(&mut ours);
        crate::rollover::retire_announced(&mut ours, &mut random);
        crate::rollover::count_first_seasons(&mut ours);
        execute(&mut ours, &mut random);
        assert!(read(&ours).is_empty(), "the list stands empty");
        let in_squad = |r: &Records, club: (u8, i16), id: u16| {
            r.team(u16::from(club.0), club.1 as u16).is_some_and(|t| t.players().contains(&id))
        };
        let mut done = 0;
        for d in read(&before).iter().filter(|d| d.open) {
            let (id, buyer) = (d.player(), d.buyer());
            let (o, x, y) = (before.player(id).unwrap(), ours.player(id).unwrap(), theirs.player(id).unwrap());
            if o.birthday() != y.birthday() || !in_squad(&theirs, buyer, id) {
                continue;
            }
            done += 1;
            assert!(in_squad(&ours, buyer, id), "player {id} at {buyer:?}");
            for at in (0x14c..0x15e).chain([0x160, 0x220, 0x221, 0x212]) {
                assert_eq!(x.get::<u8>(at), y.get::<u8>(at), "player {id} at {at:#x}");
            }
            let station = STATIONS + usize::try_from(x.career_count() - 1).unwrap() * STATION;
            for at in station..station + 0x13 {
                assert_eq!(x.get::<u8>(at), y.get::<u8>(at), "player {id}'s station at {at:#x}");
            }
            assert_eq!(x.matchdays(), y.matchdays(), "player {id}'s matchdays");
        }
        assert!(done > 260, "{done} deals as the original's");
    }

    /// A sale agreed for the season's end, on A: Young's from 14/43 to
    /// 14/20 joins the agreed list behind the deals it held — open, no
    /// loan, with the clubs, the fee, the contract and the buyer's league
    /// less the seller's; the buyer is his next club, what 14/43 owed him
    /// is paid, he is off the transfer list and stays in its squad.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_sale_agreed_for_the_seasons_end_joins_the_list() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let mut records = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        const YOUNG: u16 = 2168;
        assert!(crate::transfers::listings(&records).iter().any(|l| l.id == YOUNG));
        let held = read(&records).len();
        let (owed, earned) = records.player(YOUNG).map(|p| (p.get::<u16>(OWED), p.earned())).unwrap();
        let mut terms = [0u8; 0x14];
        terms[0..2].copy_from_slice(&60u16.to_le_bytes());
        terms[0xc] = 3;
        assert!(agree(&mut records, &mut Random::new(1), YOUNG, (14, 43), (14, 20), 548_000, &terms));
        let deals = read(&records);
        assert_eq!(deals.len(), held + 1);
        let d = deals[held];
        assert!(d.open && !d.loan());
        assert_eq!((d.player(), d.seller(), d.buyer(), d.fee()), (YOUNG, (14, 43), (14, 20), 548_000));
        assert_eq!(d.bytes[TERMS..TERMS + 0x14], terms);
        let levels = [43, 20].map(|n| records.team(14, n).unwrap().level().min(2));
        assert_eq!(d.bytes[0x24], levels[1].wrapping_sub(levels[0]));
        let p = records.player(YOUNG).unwrap();
        assert_eq!((p.get::<u8>(0x14c), p.get::<i16>(0x14e)), (14, 20));
        assert_eq!((p.get::<u16>(OWED), p.earned()), (0, earned.wrapping_add(i32::from(owed) * 1000)));
        assert_eq!(p.club(), [14, 43]);
        assert!(records.team(14, 43).unwrap().players().contains(&YOUNG));
        assert!(!crate::transfers::listings(&records).iter().any(|l| l.id == YOUNG));
    }
}
