//! The transfer market as the transfer list shows it (TRANSFERLISTE.md in
//! the RE repo): the players listed for sale or loan, the negotiation
//! appointments of the week, and a club's watch list.

use crate::records::Records;

/// The transfer list (`0x1c49278`, its data from +0x20): an i16 count,
/// then up to 400 entries of 0x1c bytes from +8.
const LIST_AT: u32 = 0x1c49278;
const LIST_FIRST: usize = 8;
const LIST_ENTRY: usize = 0x1c;
const LIST_SIZE: usize = 400;
/// The negotiation appointments (`0x1c54000`): an i16 count, entries of
/// 0x4c bytes from +4, the day at +2, the player at +6.
const TALKS_AT: u32 = 0x1c54000;
const TALKS_FIRST: usize = 4;
const TALK: usize = 0x4c;
/// The human clubs' watch lists (`0xa4d0f8[slot]`): an i16 count, then 50
/// players of 16 bytes from +4, the id first.
const WATCH_AT: u32 = 0xa4d0f8;
const WATCH_FIRST: usize = 4;
const WATCHED: usize = 16;
pub const WATCH_SIZE: usize = 50;

/// What a listing offers: a sale, a loan, a loan with an option to buy.
pub const SALE: u8 = 1;
pub const LOAN: u8 = 2;
pub const LOAN_WITH_OPTION: u8 = 3;
/// A listing's terms: 2 "ablösefrei".
pub const FREE_TRANSFER: u8 = 2;

/// A player on the transfer list, as listed: his club, his main position
/// and strength then, the day, the kind and terms, the loan's fee and the
/// option's price.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Listing {
    pub id: u16,
    pub country: u8,
    pub number: u16,
    pub position: u8,
    pub strength: u8,
    pub day: i16,
    pub kind: u8,
    pub terms: u8,
    pub fee: i32,
    pub option: i32,
}

fn word(b: &[u8], at: usize) -> i16 {
    b.get(at..at + 2).map_or(0, |b| i16::from_le_bytes([b[0], b[1]]))
}

fn long(b: &[u8], at: usize) -> i32 {
    b.get(at..at + 4).map_or(0, |b| i32::from_le_bytes(b.try_into().unwrap()))
}

fn list_name() -> String {
    format!("{LIST_AT:#x}")
}

/// The players on the transfer list, in the list's order.
pub fn listings(records: &Records) -> Vec<Listing> {
    let Some(b) = records.block(&list_name()) else { return Vec::new() };
    let count = usize::try_from(word(b, 0)).unwrap_or(0).min(LIST_SIZE);
    (0..count)
        .map(|k| {
            let e = LIST_FIRST + k * LIST_ENTRY;
            let byte = |o: usize| b.get(e + o).copied().unwrap_or(0);
            Listing {
                id: word(b, e) as u16,
                country: byte(2),
                number: word(b, e + 4) as u16,
                position: byte(6),
                strength: byte(7),
                day: word(b, e + 0xa),
                kind: byte(0xc) & 3,
                terms: byte(0xd),
                fee: long(b, e + 0x10),
                option: long(b, e + 0x14),
            }
        })
        .collect()
}

/// The list as the screen sorts it (`FUN_00759d61`): by main position,
/// then the strongest first, by selection and swap — not stably.
pub fn sorted(mut list: Vec<Listing>) -> Vec<Listing> {
    let n = list.len();
    for i in 0..n.saturating_sub(1) {
        let mut m = i;
        for j in i + 1..n {
            if list[j].position < list[m].position
                || (list[m].position == list[j].position && list[m].strength < list[j].strength)
            {
                m = j;
            }
        }
        if m != i {
            list.swap(i, m);
        }
    }
    list
}

/// A player taken off the transfer list: the entries after his move up
/// (`FUN_00758f42`), and his record's mark goes (`FUN_0075a636`).
pub fn take_off(records: &mut Records, id: u16) {
    if let Some(b) = records.block_mut(&list_name()) {
        let count = usize::try_from(word(b, 0)).unwrap_or(0).min(LIST_SIZE);
        let at = (0..count).find(|&k| word(b, LIST_FIRST + k * LIST_ENTRY) as u16 == id);
        if let Some(k) = at {
            let from = LIST_FIRST + (k + 1) * LIST_ENTRY;
            let end = (LIST_FIRST + LIST_SIZE * LIST_ENTRY).min(b.len());
            if from < end {
                b.copy_within(from..end, from - LIST_ENTRY);
            }
            b[0..2].copy_from_slice(&((count - 1) as i16).to_le_bytes());
        }
    }
    if let Some(r) = records.players.get_mut(usize::from(id)) {
        r.set_retiring(r.retiring() & !LISTED);
    }
}

/// A record's mark of a player on the transfer list (R+0x26f bit 0).
const LISTED: u8 = 1;

/// The day of the week's negotiation appointment a player has, if any
/// club has one with him (`FUN_00762a93`).
pub fn talk_day(records: &Records, id: u16) -> Option<i16> {
    let b = records.block(&format!("{TALKS_AT:#x}"))?;
    let count = usize::try_from(word(b, 0)).unwrap_or(0);
    (0..count)
        .map(|k| TALKS_FIRST + k * TALK)
        .find(|&e| word(b, e + 6) as u16 == id)
        .map(|e| word(b, e + 2))
}

fn watch_name(slot: usize) -> String {
    format!("{WATCH_AT:#x}[{slot}]")
}

/// The players a human club watches, by its slot: all fifty places, as
/// the list's colours read them (`FUN_0069904c`).
pub fn watched(records: &Records, slot: usize) -> Vec<u16> {
    let Some(b) = records.block(&watch_name(slot)) else { return Vec::new() };
    (0..WATCH_SIZE)
        .map(|k| word(b, WATCH_FIRST + k * WATCHED) as u16)
        .filter(|&id| id != 0)
        .collect()
}

/// What putting a player on the watch list came to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Watch {
    Added,
    Already,
    Full,
}

/// A player put on a club's watch list (`FUN_00418741`): after the others,
/// unless he is there already or the list is full.
pub fn watch(records: &mut Records, slot: usize, id: u16) -> Watch {
    let Some(b) = records.block_mut(&watch_name(slot)) else { return Watch::Full };
    let count = usize::try_from(word(b, 0)).unwrap_or(0);
    if count >= WATCH_SIZE {
        return Watch::Full;
    }
    if (0..count).any(|k| word(b, WATCH_FIRST + k * WATCHED) as u16 == id) {
        return Watch::Already;
    }
    let at = WATCH_FIRST + count * WATCHED;
    b[at..at + 2].copy_from_slice(&id.to_le_bytes());
    b[0..2].copy_from_slice(&((count + 1) as i16).to_le_bytes());
    Watch::Added
}

/// A negotiation appointment (TERMINE-BEOBACHTUNG.md §1.1): its kind, the
/// day, the player, and the clubs taking part — his own first.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Appointment {
    pub kind: i16,
    pub day: i16,
    pub player: u16,
    pub clubs: Vec<(u8, i16)>,
    /// Each club's weight in the talk (+0x11 + 6k).
    pub weights: Vec<u8>,
    /// The contract the talk is on (+0x38).
    pub terms: [u8; TERMS],
}

/// The appointments' kinds: poaching a player not on the transfer list
/// (not shown to others), a purchase, a loan; the first is never booked.
pub const RELEASE_FEE_TALK: i16 = 1;
pub const POACHING: i16 = 2;
pub const PURCHASE: i16 = 3;
pub const LOAN_TALK: i16 = 4;
const MOST_CLUBS: usize = 10;
const CLUB_AT: usize = 0xc;
const CLUB: usize = 6;

/// The week's appointments, in the order kept.
pub fn appointments(records: &Records) -> Vec<Appointment> {
    let Some(b) = records.block(&format!("{TALKS_AT:#x}")) else { return Vec::new() };
    let count = usize::try_from(word(b, 0)).unwrap_or(0);
    (0..count)
        .map(|k| {
            let e = TALKS_FIRST + k * TALK;
            let n = usize::try_from(word(b, e + 4)).unwrap_or(0).min(MOST_CLUBS);
            Appointment {
                kind: word(b, e),
                day: word(b, e + 2),
                player: word(b, e + 6) as u16,
                clubs: (0..n)
                    .map(|i| {
                        let at = e + CLUB_AT + i * CLUB;
                        (b.get(at).copied().unwrap_or(0), word(b, at + 2))
                    })
                    .collect(),
                weights: (0..n)
                    .map(|i| b.get(e + CLUB_AT + i * CLUB + 5).copied().unwrap_or(0))
                    .collect(),
                terms: b
                    .get(e + TALK_TERMS..e + TALK_TERMS + TERMS)
                    .and_then(|t| t.try_into().ok())
                    .unwrap_or([0; TERMS]),
            }
        })
        .collect()
}

/// A watched player (16 bytes): his id, the marks of his last five
/// matches watched, newest first, the matches watched and his form's sum
/// over them (TERMINE-BEOBACHTUNG.md §3.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Watched {
    pub id: u16,
    pub marks: [u8; 5],
    pub matches: i16,
    pub form_sum: i32,
}

/// The watch list's places with a player, all fifty, in place order.
pub fn watch_list(records: &Records, slot: usize) -> Vec<Watched> {
    let Some(b) = records.block(&watch_name(slot)) else { return Vec::new() };
    (0..WATCH_SIZE)
        .map(|k| WATCH_FIRST + k * WATCHED)
        .filter_map(|e| {
            let id = word(b, e) as u16;
            (id != 0).then(|| Watched {
                id,
                marks: std::array::from_fn(|i| b.get(e + 2 + i).copied().unwrap_or(0)),
                matches: word(b, e + 8),
                form_sum: long(b, e + 0xc),
            })
        })
        .collect()
}

/// A player taken off the watch list: the places after his move up, the
/// last used one is cleared (`FUN_00418831`).
pub fn unwatch(records: &mut Records, slot: usize, id: u16) {
    let Some(b) = records.block_mut(&watch_name(slot)) else { return };
    let count = usize::try_from(word(b, 0)).unwrap_or(0).min(WATCH_SIZE);
    let Some(k) = (0..count).find(|&k| word(b, WATCH_FIRST + k * WATCHED) as u16 == id) else {
        return;
    };
    let end = WATCH_FIRST + count * WATCHED;
    b.copy_within(WATCH_FIRST + (k + 1) * WATCHED..end, WATCH_FIRST + k * WATCHED);
    b[end - WATCHED..end].fill(0);
    b[0..2].copy_from_slice(&((count - 1) as i16).to_le_bytes());
}

/// The scouts out by region: the day each comes back, 0 none
/// (TERMINE-BEOBACHTUNG.md §3.4).
const SCOUTS_AT: usize = 0x32e;
pub const REGIONS: usize = 5;
const SCOUT_DAYS: i16 = 28;

pub fn scouts(records: &Records, slot: usize) -> [i16; REGIONS] {
    let b = records.block(&watch_name(slot)).unwrap_or(&[]);
    std::array::from_fn(|i| word(b, SCOUTS_AT + 2 * i))
}

/// A scout sent to a region for four weeks.
pub fn send_scout(records: &mut Records, slot: usize, region: usize, today: i16) {
    if let Some(b) = records.block_mut(&watch_name(slot)) {
        let at = SCOUTS_AT + 2 * region;
        b[at..at + 2].copy_from_slice(&(today + SCOUT_DAYS).to_le_bytes());
    }
}

/// Two more lists a player can have a talk pending in: the national
/// team's search (6-byte entries, the id first) and another (8-byte
/// entries, the id at +2), each after an i16 count (`FUN_007647b4`).
const PENDING_LISTS: [(u32, usize, usize); 2] = [(0x1c53d98, 6, 0), (0x1c53ee8, 8, 2)];

/// Whether a talk with a player is pending anywhere (`FUN_007647b4`).
pub fn pending(records: &Records, id: u16) -> bool {
    let in_list = |(at, size, field): (u32, usize, usize)| {
        records.block(&format!("{at:#x}")).is_some_and(|b| {
            let count = usize::try_from(word(b, 0)).unwrap_or(0);
            (0..count).any(|k| word(b, 2 + k * size + field) as u16 == id)
        })
    };
    PENDING_LISTS.into_iter().any(in_list) || appointments(records).iter().any(|t| t.player == id)
}

/// A club as the appointments name it: its country and number.
pub type Side = (u8, i16);

/// At most this many appointments; this many clubs at one table.
const MOST_TALKS: usize = 200;
const MOST_JOINED: usize = 7;
/// A human club's weight at the table (`FUN_0077ee79` answers 10).
const HUMAN_WEIGHT: u8 = 10;
/// An appointment's fields: kind, day, how many clubs, the player, his
/// club, the clubs at +0xc (country, number, status, weight), the terms.
const TALK_KIND: usize = 0;
const TALK_DAY: usize = 2;
const TALK_CLUBS: usize = 4;
const TALK_PLAYER: usize = 6;
const TALK_HIS_CLUB: usize = 8;
const TALK_TERMS: usize = 0x38;
pub const TERMS: usize = 20;

fn put16(b: &mut [u8], at: usize, v: i16) {
    b[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

fn put_side(b: &mut [u8], at: usize, (country, number): Side, status: u8, weight: u8) {
    b[at] = country;
    b[at + 1] = 0;
    put16(b, at + 2, number);
    b[at + 4] = status;
    b[at + 5] = weight;
}

/// An appointment's clubs, by entry (participant k at +0xc + 6k).
fn side(b: &[u8], e: usize, k: usize) -> Side {
    let at = e + CLUB_AT + k * CLUB;
    (b.get(at).copied().unwrap_or(0), word(b, at + 2))
}

/// A new appointment (`FUN_0076254d`): the player, his club at the table
/// and ours after it, a human's weight; only while there is room and the
/// day has not gone by. Whether it was written.
pub fn book(
    records: &mut Records,
    kind: i16,
    day: i16,
    player: u16,
    seller: Side,
    buyer: Side,
    terms: [u8; TERMS],
) -> bool {
    let today = records.globals.day() as i16;
    let Some(b) = records.block_mut(&format!("{TALKS_AT:#x}")) else { return false };
    let count = usize::try_from(word(b, 0)).unwrap_or(0);
    let e = TALKS_FIRST + count * TALK;
    if count >= MOST_TALKS || day < today || b.len() < e + TALK {
        return false;
    }
    b[e..e + TALK].fill(0);
    put16(b, e + TALK_KIND, kind);
    put16(b, e + TALK_DAY, day);
    put16(b, e + TALK_CLUBS, 2);
    put16(b, e + TALK_PLAYER, player as i16);
    b[e + TALK_HIS_CLUB] = seller.0;
    put16(b, e + TALK_HIS_CLUB + 2, seller.1);
    put_side(b, e + CLUB_AT, seller, 0, 0);
    put_side(b, e + CLUB_AT + CLUB, buyer, 0, HUMAN_WEIGHT);
    b[e + TALK_TERMS..e + TALK_TERMS + TERMS].copy_from_slice(&terms);
    put16(b, 0, count as i16 + 1);
    true
}

/// Our club joins the player's appointment (`FUN_00762691`): not a full
/// table, not twice; after the second club, before those weighing less
/// than a human. Whether it joined.
pub fn join(records: &mut Records, player: u16, buyer: Side) -> bool {
    let Some(b) = records.block_mut(&format!("{TALKS_AT:#x}")) else { return false };
    let count = usize::try_from(word(b, 0)).unwrap_or(0);
    let Some(e) = (0..count)
        .map(|k| TALKS_FIRST + k * TALK)
        .find(|&e| word(b, e + TALK_PLAYER) as u16 == player)
    else {
        return false;
    };
    let n = usize::try_from(word(b, e + TALK_CLUBS)).unwrap_or(0);
    if n >= MOST_JOINED || (0..n).any(|k| side(b, e, k) == buyer) {
        return false;
    }
    let mut k = if n == 1 { 0 } else { n - 1 };
    if n != 1 {
        while k > 1 && b[e + CLUB_AT + k * CLUB + 5] < HUMAN_WEIGHT {
            let at = e + CLUB_AT + k * CLUB;
            b.copy_within(at..at + CLUB, at + CLUB);
            k -= 1;
        }
    }
    put_side(b, e + CLUB_AT + (k + 1) * CLUB, buyer, 0, HUMAN_WEIGHT);
    put16(b, e + TALK_CLUBS, n as i16 + 1);
    true
}

/// A human club joins the player's appointment from the week-start form
/// of 25 (`FUN_007651de(L, club, day − WS, kind, 0x0e)`): its planner
/// busy that day → nothing; else the join (`FUN_00762691`, its result
/// ignored) and the planner's day as a negotiation of the transfer list.
/// The kind is not read. Whether the planner was free.
pub fn join_from_week_start(records: &mut Records, player: u16, club: Side, day: i16) -> bool {
    let Some(slot) = records
        .team(u16::from(club.0), club.1 as u16)
        .and_then(|t| usize::from(t.human()).checked_sub(1))
    else {
        return false;
    };
    let Ok(at) = usize::try_from(day) else { return false };
    if records.planners.get(slot).is_none_or(|p| p.day(at).kind != 0) {
        return false;
    }
    join(records, player, club);
    if let Some(planner) = records.planners.get_mut(slot) {
        planner.book(at, crate::records::Appointment { kind: LIST_TALK_DAY, flag: 1, player, club: 0 });
    }
    true
}

/// The planner's kind of a negotiation booked from the transfer list.
pub const LIST_TALK_DAY: u8 = 0x0e;

/// How many of this week's appointments a club sits at (`FUN_00761f7a`):
/// from the week's first day to its last.
pub fn talks_in_week(records: &Records, club: Side) -> usize {
    let start = records.globals.week_start();
    appointments(records)
        .iter()
        .filter(|t| (start..=start + 6).contains(&t.day))
        .map(|t| t.clubs.iter().filter(|&&c| c == club).count())
        .sum()
}

/// Whether a club sits at an appointment on a day (`FUN_00761c43`).
pub fn talks_on(records: &Records, club: Side, day: i16) -> bool {
    appointments(records).iter().any(|t| t.day == day && t.clubs.contains(&club))
}

/// The contract as the player's object holds it, which a poaching talk
/// takes along (L+0x88 … +0x9b, `FUN_00636638`): salary, bonuses, the
/// release fee and +0x158, the years left, the terms, +0x162 and +0x160.
/// (The original's L+0x98 matched R+0x162 in both talks seen, not the
/// R+0x161 it is said to copy; VERHANDLUNG-BUCHUNG.md keeps it open.)
pub fn terms(records: &Records, player: u16) -> [u8; TERMS] {
    let mut t = [0; TERMS];
    let Some(r) = records.player(player) else { return t };
    let years = (i32::from(r.contract() & 0xfff) - i32::from(records.globals.start_year())).max(0);
    t[0..2].copy_from_slice(&r.salary().to_le_bytes());
    t[2..4].copy_from_slice(&r.appearance_bonus().to_le_bytes());
    t[4..6].copy_from_slice(&r.goal_bonus().to_le_bytes());
    t[6..8].copy_from_slice(&r.fixed_fee().to_le_bytes());
    t[8..12].copy_from_slice(&r.get::<u32>(0x158).to_le_bytes());
    t[12] = years as u8;
    t[14..16].copy_from_slice(&r.terms().to_le_bytes());
    t[16] = r.get::<u8>(0x162);
    t[17] = r.get::<u8>(0x160);
    t
}

/// A listing's terms as the club sets them: 1 for a fee, 2 "ablösefrei".
pub const FOR_A_FEE: u8 = 1;

/// The terms of his listing, if he is on the list (`FUN_0075b308`, the
/// entry's +0xd): the club may take him off only when it is 1.
pub fn listed_terms(records: &Records, id: u16) -> Option<u8> {
    let b = records.block(&list_name())?;
    let count = usize::try_from(word(b, 0)).unwrap_or(0).min(LIST_SIZE);
    (0..count)
        .map(|k| LIST_FIRST + k * LIST_ENTRY)
        .find(|&e| word(b, e) as u16 == id)
        .and_then(|e| b.get(e + 0xd).copied())
}

/// Him onto the transfer list for sale (`FUN_0075a129` → `FUN_007592fa`
/// with kind 1): his club, main position and strength, today, the terms
/// — `free` "ablösefrei" —, and his record marked; false when the list is
/// full or holds him. The entry's +8 (a strength outlook,
/// `FUN_006ff516`) and +0xe (a value factor, `FUN_00708b80`), which the
/// list's screen does not show, stay 0.
pub fn list_for_sale(records: &mut Records, id: u16, free: bool) -> bool {
    list_for_terms(records, id, if free { FREE_TRANSFER } else { FOR_A_FEE })
}

/// Him onto the transfer list for sale on `terms` (`FUN_00758b34` with
/// kind 1): as `list_for_sale`; a manager who plays is never listed.
pub(crate) fn list_for_terms(records: &mut Records, id: u16, terms: u8) -> bool {
    if id > crate::mood::LAST_REAL_ID {
        return false;
    }
    let Some(r) = records.player(id) else { return false };
    let [country, number] = r.club();
    let (position, strength) = (r.positions()[0], r.strength());
    let day = records.globals.day() as i16;
    let Some(b) = records.block_mut(&list_name()) else { return false };
    let count = usize::try_from(word(b, 0)).unwrap_or(0).min(LIST_SIZE);
    if count >= LIST_SIZE || (0..count).any(|k| word(b, LIST_FIRST + k * LIST_ENTRY) as u16 == id)
    {
        return false;
    }
    let e = LIST_FIRST + count * LIST_ENTRY;
    if b.len() < e + LIST_ENTRY {
        return false;
    }
    b[e..e + LIST_ENTRY].fill(0);
    put16(b, e, id as i16);
    b[e + 2] = country;
    put16(b, e + 4, i16::from(number));
    b[e + 6] = position;
    b[e + 7] = strength;
    put16(b, e + 0xa, day);
    b[e + 0xc] = SALE;
    b[e + 0xd] = terms;
    put16(b, 0, count as i16 + 1);
    if let Some(r) = records.players.get_mut(usize::from(id)) {
        r.set_retiring(r.retiring() | LISTED);
    }
    true
}

/// Every listing's day moved back by `days` (`FUN_00759d0c`).
pub(crate) fn shift_days(records: &mut Records, days: i16) {
    let Some(b) = records.block_mut(&list_name()) else { return };
    let count = usize::try_from(word(b, 0)).unwrap_or(0).min(LIST_SIZE);
    for k in 0..count {
        let at = LIST_FIRST + k * LIST_ENTRY + 0xa;
        let day = word(b, at).wrapping_sub(days);
        put16(b, at, day);
    }
}

/// Late in the season (`FUN_0075926f`, at each week's market after day
/// 216): a listing whose player has under two years left loses its terms,
/// so his club cannot take him off any more.
pub(crate) fn fix_late_terms(records: &mut Records) {
    let start = i32::from(records.globals.start_year());
    let short: Vec<bool> = listings(records)
        .iter()
        .map(|l| records.player(l.id).is_some_and(|p| i32::from(p.contract() & 0xfff) - start < 2))
        .collect();
    let Some(b) = records.block_mut(&list_name()) else { return };
    for (k, short) in short.into_iter().enumerate() {
        let at = LIST_FIRST + k * LIST_ENTRY + 0xd;
        if short && b.get(at).is_some_and(|&t| t != 0) {
            b[at] = 0;
        }
    }
}

/// Him onto the transfer list for a loan (`FUN_0075b0da` →
/// `FUN_00758b34`): kind 2 for a fee, 3 with an option to buy at
/// `option`, terms 1; as `list_for_sale` otherwise. The entry's +0x18,
/// which the original reckons against the fair prices and no screen
/// built reads, stays 0.
pub fn list_for_loan(
    records: &mut Records,
    id: u16,
    with_option: bool,
    fee: i32,
    option: i32,
) -> bool {
    if !list_for_sale(records, id, false) {
        return false;
    }
    let Some(b) = records.block_mut(&list_name()) else { return false };
    let count = usize::try_from(word(b, 0)).unwrap_or(0);
    let e = LIST_FIRST + (count - 1) * LIST_ENTRY;
    b[e + 0xc] = if with_option { LOAN_WITH_OPTION } else { LOAN };
    b[e + 0xd] = FOR_A_FEE;
    b[e + 0x10..e + 0x14].copy_from_slice(&fee.to_le_bytes());
    let option = if with_option { option } else { 0 };
    b[e + 0x14..e + 0x18].copy_from_slice(&option.to_le_bytes());
    true
}

/// A club's players listed for a loan (`FUN_0075ab13`).
pub fn loan_listings(records: &Records, (country, number): Side) -> usize {
    listings(records)
        .iter()
        .filter(|l| matches!(l.kind, LOAN | LOAN_WITH_OPTION))
        .filter(|l| l.country == country && l.number as i16 == number)
        .count()
}

/// A player off a human club's watch list (`FUN_00418ca1`: the list of
/// the club's human slot).
pub fn unwatch_at_club(records: &mut Records, club: (u8, i16), id: u16) {
    let slot = u16::try_from(club.1)
        .ok()
        .and_then(|n| records.team(u16::from(club.0), n))
        .and_then(|t| usize::from(t.human()).checked_sub(1));
    if let Some(slot) = slot {
        unwatch(records, slot, id);
    }
}

/// An appointment taken off the list, the player's first (`FUN_00761aa6`):
/// the entries after it move up.
pub fn cancel_talk(records: &mut Records, player: u16) {
    let Some(b) = records.block_mut(&format!("{TALKS_AT:#x}")) else { return };
    let count = usize::try_from(word(b, 0)).unwrap_or(0).min(MOST_TALKS);
    let Some(k) = (0..count).find(|&k| word(b, TALKS_FIRST + k * TALK + TALK_PLAYER) as u16 == player)
    else {
        return;
    };
    let from = TALKS_FIRST + (k + 1) * TALK;
    let end = (TALKS_FIRST + count * TALK).min(b.len());
    if from < end {
        b.copy_within(from..end, from - TALK);
    }
    put16(b, 0, count as i16 - 1);
}

/// The day's poaching talks (`FUN_0076cb3e`'s first loop): every kind-2
/// appointment of the day (`FUN_00761d61`) leaves the list; unless the
/// first pre-season stands still (`FUN_005ef56c`), one whose second club
/// (+0x12/+0x14) has a human manager (`FUN_0067764c`) and names a player
/// (0 < id < 0x426e) is his talk of the day (`FUN_0076aca0`). The talks,
/// as player and the manager's number from 1.
pub fn poaching_day(records: &mut Records, day: i16) -> Vec<(u16, u8)> {
    let still = crate::tournament::still(records, day);
    let mut talks = Vec::new();
    while let Some(t) =
        appointments(records).into_iter().find(|t| t.day == day && t.kind == POACHING)
    {
        let human = t
            .clubs
            .get(1)
            .and_then(|&(country, number)| records.team(u16::from(country), number as u16))
            .map_or(0, |team| team.human());
        if !still && human != 0 && (1..0x426e).contains(&t.player) {
            talks.push((t.player, human));
        }
        cancel_talk(records, t.player);
    }
    talks
}

/// A human manager's talk of the day (`0x1c5b9c8 + (n − 1)·0x44`, from
/// +0x20; `FUN_0076abc4` reads it, `FUN_0076ac2e` writes it, and the day
/// run zeroes all of them first, `FUN_0076fac0`): the player, how the
/// talk ended (+0x28) and the contract it ended on (+0x30, 20 bytes).
/// Not saved.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct DayTalk {
    pub player: u16,
    pub kind: i32,
    pub terms: [u8; TERMS],
}

/// A poaching talk the player agreed to (`FUN_0076ad3b` writes it on the
/// dialog's OK).
pub const AGREED: i32 = 3;

/// Purchase talks with the player's club go on one of these days of the
/// next week, a row drawn (`0x8abbdc`, 3 rows of 5, + 7 from the week's
/// start).
const PURCHASE_DAYS: [[i16; 5]; 3] = [[5, 4, 2, 1, 3], [2, 5, 4, 1, 3], [4, 2, 5, 1, 3]];

/// How many appointments a club sits at from `first` to six days on
/// (`FUN_00761f7a`).
fn talks_from(records: &Records, club: Side, first: i16) -> usize {
    appointments(records)
        .iter()
        .filter(|t| (first..=first + 6).contains(&t.day))
        .map(|t| t.clubs.iter().filter(|&&c| c == club).count())
        .sum()
}

/// Whether a club has three talks in the next week already
/// (`FUN_00764a80`).
fn next_week_full(records: &Records, club: Side) -> bool {
    talks_from(records, club, records.globals.week_start() + 7) >= 3
}

/// Whether a club is busy on a day (`FUN_00764ad0(club, day − WS)`): a
/// human club's planner holds something then, or it sits at a talk.
fn busy(records: &Records, club: Side, day: i16) -> bool {
    let slot = records
        .team(u16::from(club.0), club.1 as u16)
        .and_then(|t| usize::from(t.human()).checked_sub(1));
    if let Some(slot) = slot
        && records.planners.get(slot).is_some_and(|p| p.day(usize::try_from(day).unwrap_or(0)).kind != 0)
    {
        return true;
    }
    talks_on(records, club, day)
}

/// The day of the next week, from its first, for a purchase talk between
/// the buyer and the player's club (`FUN_007586d3`): 0 when either has
/// three talks that week; else the first of a drawn row free for both,
/// 0 if none is.
fn purchase_day(records: &Records, random: &mut crate::random::Random, his: Side, buyer: Side) -> i16 {
    if next_week_full(records, buyer) || next_week_full(records, his) {
        return 0;
    }
    let start = records.globals.week_start();
    let row = random.below(3) as usize;
    PURCHASE_DAYS[row]
        .into_iter()
        .find(|&d| !busy(records, his, start + d + 7) && !busy(records, buyer, start + d + 7))
        .unwrap_or(0)
}

/// After the day's talks, a manager's agreed poaching talk
/// (`FUN_0076aeb8`): the player goes on the transfer list for sale with
/// terms 3 (`FUN_0075a3cc(L, 0)` → `FUN_00758e88` → `FUN_00758b34(L, 1,
/// 3, 0, 0)`); if he was not there already, the buyer meets his club for
/// the fee in the next week (`FUN_0076254d`, kind 3, on the contract
/// agreed). The day booked, if one was.
pub fn after_poaching(
    records: &mut Records,
    random: &mut crate::random::Random,
    talk: &DayTalk,
    buyer: Side,
) -> Option<i16> {
    if talk.player == 0 || talk.kind != AGREED {
        return None;
    }
    let his = records.player(talk.player).map(|r| r.club())?;
    let his = (his[0], i16::from(his[1]));
    if !list_for_terms(records, talk.player, POACHED) {
        return None;
    }
    let day = records.globals.week_start() + 7 + purchase_day(records, random, his, buyer);
    book(records, PURCHASE, day, talk.player, his, buyer, talk.terms).then_some(day)
}

/// A listing's terms after a poaching talk the player agreed to.
pub const POACHED: u8 = 3;

/// A loan talk on its day: the player, the lending club, the clubs still
/// bidding, in the appointment's order, and, when none of them has a
/// human manager, the computer club that gets him.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LoanTalk {
    pub player: u16,
    pub lender: Side,
    pub bidders: Vec<Side>,
    pub winner: Option<Side>,
}

/// Who gets a loan no human bids for (`FUN_00682814`): of the bidders
/// with squad room (`FUN_00767c78`) the one with the highest weight in
/// the appointment (+0x11 + 6k), the first on a tie. The move itself
/// (`FUN_0070d5ef` on the entry's terms +0x44, fee E+0x10, option E+0x14,
/// kind E+0xc) and the lender's news (`FUN_00681a52`) are not built.
fn loan_winner(records: &Records, t: &Appointment, bidders: &[Side]) -> Option<Side> {
    let human = |&(country, number): &Side| {
        records.team(u16::from(country), number as u16).is_some_and(|team| team.human() != 0)
    };
    if bidders.iter().any(human) {
        return None;
    }
    let mut best: Option<(u8, Side)> = None;
    for (k, &club) in t.clubs.iter().enumerate().skip(1) {
        let weight = t.weights.get(k).copied().unwrap_or(0);
        if bidders.contains(&club) && best.is_none_or(|(w, _)| weight > w) {
            best = Some((weight, club));
        }
    }
    best.map(|(_, club)| club)
}

/// A squad this big takes no one more on loan (`FUN_00767c78`: club
/// +0xe2 < 0x22).
const FULL_SQUAD: u16 = 34;

/// The day's loan talks (`FUN_0076cb3e`'s last loop: kind 4 →
/// `FUN_00681ae6`, the entry removed either way). The talk ends at once
/// when the player has left the lending club (`FUN_006968c4`) or the
/// transfer list (`FUN_007597a4`); else each other club of the
/// appointment (`FUN_006782aa` skips the lender) drops out when it can
/// invest less than the loan's fee E+0x10 (`FUN_00681a9f`, a human told
/// by `FUN_00768497`) or its squad is full (`FUN_00767c78`, told by
/// `FUN_0076851f`), and with nobody left (asm 0x681e64) nothing more
/// happens. Past day 216 the original counts the squad with next season's
/// comings and goings (`FUN_0070ffef`), not ported: the squad as it
/// stands.
pub fn loan_day(records: &mut Records, day: i16) -> Vec<LoanTalk> {
    let mut talks = Vec::new();
    while let Some(t) =
        appointments(records).into_iter().find(|t| t.day == day && t.kind == LOAN_TALK)
    {
        cancel_talk(records, t.player);
        let Some(&lender) = t.clubs.first() else { continue };
        let with_lender = records
            .player(t.player)
            .is_some_and(|r| t.player != 0 && (r.club()[0], i16::from(r.club()[1])) == lender);
        let Some(listing) = listings(records).into_iter().find(|l| l.id == t.player) else {
            continue;
        };
        if !with_lender {
            continue;
        }
        let prices = records.globals.fund_prices();
        let agreed = records.agreed();
        let bidders: Vec<Side> = t
            .clubs
            .iter()
            .copied()
            .filter(|&c| c != lender)
            .filter(|&(country, number)| {
                records.team(u16::from(country), number as u16).is_some_and(|team| {
                    let investable =
                        crate::finances::Balance::of(team, prices, &agreed).investable();
                    investable >= i64::from(listing.fee as u32) && team.player_count() < FULL_SQUAD
                })
            })
            .collect();
        if !bidders.is_empty() {
            let winner = loan_winner(records, &t, &bidders);
            talks.push(LoanTalk { player: t.player, lender, bidders, winner });
        }
    }
    talks
}

/// A purchase talk on its day, as far as it gets before the clubs
/// bargain: the player, the selling club and the clubs still bidding.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PurchaseTalk {
    pub player: u16,
    pub seller: Side,
    pub bidders: Vec<Side>,
}

/// The release fee's unit (`DAT_00a3b3bc`).
const RELEASE_UNIT: i64 = 50_000;
/// Past this day of the season a release fee bars no bidder
/// (`FUN_0075827a`: `DAT_00b5cbd4` > 0xd8).
const LATE_SEASON: i16 = 0xd8;

/// The day's purchase talks (`FUN_0076cb3e`'s last loop: kind 3 →
/// `FUN_00768638`, the entry removed either way). The talk ends at once
/// when the player has left the selling club (`FUN_006968c4`) or the
/// transfer list (`FUN_0075974f`); else each other club of the
/// appointment drops out when the player's release fee (L+0x8e, R+0x156,
/// in 50 000s) is set, the season is not past day 216 and the club can
/// invest less (`FUN_00767b6a`, a human told by `FUN_00768497`), or when
/// its squad is full (`FUN_00767c78`, told by `FUN_0076851f`); the
/// seller is skipped (`FUN_006782aa`). The bargaining that follows (from
/// asm 0x768c65) is not built. Afterwards a listing whose terms read
/// [`POACHED`] (E+0xd = 3) leaves the transfer list (`FUN_0075a71f`).
pub fn purchase_talk_day(records: &mut Records, day: i16) -> Vec<PurchaseTalk> {
    let mut talks = Vec::new();
    while let Some(t) =
        appointments(records).into_iter().find(|t| t.day == day && t.kind == PURCHASE)
    {
        if let Some(talk) = purchase_bidders(records, &t, day) {
            talks.push(talk);
        }
        if listings(records).iter().any(|l| l.id == t.player && l.terms == POACHED) {
            take_off(records, t.player);
        }
        cancel_talk(records, t.player);
    }
    talks
}

/// `FUN_00768638` up to its bargaining: who still bids.
fn purchase_bidders(records: &Records, t: &Appointment, day: i16) -> Option<PurchaseTalk> {
    let &seller = t.clubs.first()?;
    let player = records.player(t.player)?;
    if t.player == 0 || (player.club()[0], i16::from(player.club()[1])) != seller {
        return None;
    }
    listings(records).into_iter().find(|l| l.id == t.player)?;
    let fee = i64::from(player.fixed_fee()) * RELEASE_UNIT;
    let prices = records.globals.fund_prices();
    let agreed = records.agreed();
    let bidders: Vec<Side> = t
        .clubs
        .iter()
        .copied()
        .filter(|&c| c != seller)
        .filter(|&(country, number)| {
            records.team(u16::from(country), number as u16).is_some_and(|team| {
                let affords = fee == 0
                    || day > LATE_SEASON
                    || crate::finances::Balance::of(team, prices, &agreed).investable() >= fee;
                affords && team.player_count() < FULL_SQUAD
            })
        })
        .collect();
    (!bidders.is_empty()).then_some(PurchaseTalk { player: t.player, seller, bidders })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A poaching talk booked on save A beside its nine appointments, for
    /// the human club and a player of no other: on its day it leaves the
    /// list and is that manager's talk, the rest stay.
    #[test]
    #[ignore = "reads the kept saves next to $ANSTOSS3_DIR"]
    fn the_poaching_talks_leave_the_list_on_their_day() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let save = dir.parent().unwrap().join("saves-kept").join("A.an3");
        let mut records = Records::from_save(Save::read(&save).unwrap()).unwrap();
        let first = appointments(&records).remove(0);
        let human = records.teams.iter().find(|t| t.human() != 0).unwrap();
        let buyer = (human.country(), i16::from(human.number()));
        let player = (1..0x426e).find(|&p| !appointments(&records).iter().any(|t| t.player == p)).unwrap();
        let day = records.globals.day() as i16 + 1;
        assert!(book(&mut records, POACHING, day, player, first.clubs[0], buyer, [0; TERMS]));
        let before = appointments(&records);
        let today: Vec<_> = before.iter().filter(|t| t.day == day && t.kind == POACHING).collect();
        let talks = poaching_day(&mut records, day);
        let after = appointments(&records);
        assert_eq!(after.len(), before.len() - today.len());
        assert!(!after.iter().any(|t| t.day == day && t.kind == POACHING));
        let humans: Vec<_> = today
            .iter()
            .filter(|t| {
                t.clubs.get(1).and_then(|&(c, n)| records.team(u16::from(c), n as u16))
                    .is_some_and(|team| team.human() != 0)
            })
            .map(|t| t.player)
            .collect();
        assert_eq!(talks.iter().map(|t| t.0).collect::<Vec<_>>(), humans);
        assert_eq!(humans, vec![player]);
    }

    /// A poaching talk the player agreed to on save A: he goes on the list
    /// with terms 3, and the human club meets his club next week on one of
    /// the table's days, on the contract agreed; a second time he is
    /// listed already, and nothing more is booked.
    #[test]
    #[ignore = "reads the kept saves next to $ANSTOSS3_DIR"]
    fn an_agreed_poaching_talk_books_the_purchase_talk() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let save = dir.parent().unwrap().join("saves-kept").join("A.an3");
        let mut records = Records::from_save(Save::read(&save).unwrap()).unwrap();
        let human = records.teams.iter().find(|t| t.human() != 0).unwrap();
        let buyer = (human.country(), i16::from(human.number()));
        let listed: Vec<u16> = listings(&records).iter().map(|l| l.id).collect();
        let player = (1..0x4269)
            .find(|&p| {
                records.player(p).is_some_and(|r| {
                    let c = r.club();
                    c[1] != 0 && (c[0], i16::from(c[1])) != buyer
                })
                    && !listed.contains(&p)
                    && !appointments(&records).iter().any(|t| t.player == p)
            })
            .unwrap();
        let his = records.player(player).unwrap().club();
        let mut talk = DayTalk { player, kind: 0, terms: terms(&records, player) };
        talk.terms[4] = 7;
        let mut random = crate::random::Random::from_state(1, 1);
        assert_eq!(after_poaching(&mut records, &mut random, &talk, buyer), None);
        talk.kind = AGREED;
        let before = appointments(&records).len();
        let day = after_poaching(&mut records, &mut random, &talk, buyer).expect("no talk booked");
        let start = records.globals.week_start();
        assert!((start + 7..=start + 12).contains(&day), "day {day} not next week");
        assert_eq!(listed_terms(&records, player), Some(POACHED));
        let all = appointments(&records);
        assert_eq!(all.len(), before + 1);
        let t = all.last().unwrap();
        assert_eq!((t.kind, t.day, t.player), (PURCHASE, day, player));
        assert_eq!(t.clubs, vec![(his[0], i16::from(his[1])), buyer]);
        let b = records.block(&format!("{TALKS_AT:#x}")).unwrap();
        let e = TALKS_FIRST + (all.len() - 1) * TALK + TALK_TERMS;
        assert_eq!(&b[e..e + TERMS], &talk.terms);
        assert_eq!(after_poaching(&mut records, &mut random, &talk, buyer), None);
        assert_eq!(appointments(&records).len(), before + 1);
    }

    /// A loan talk booked on save A for the human club with a listed
    /// player of another: on its day it leaves the list, the rest stay;
    /// the human club bids when it can pay the fee, and drops out when
    /// the fee is beyond it.
    #[test]
    #[ignore = "reads the kept saves next to $ANSTOSS3_DIR"]
    fn the_loan_talks_leave_the_list_on_their_day() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let save = dir.parent().unwrap().join("saves-kept").join("A.an3");
        let human = {
            let records = Records::from_save(Save::read(&save).unwrap()).unwrap();
            let t = records.teams.iter().find(|t| t.human() != 0).unwrap();
            (t.country(), i16::from(t.number()))
        };
        let run = |fee: i32, computer: bool| {
            let mut records = Records::from_save(Save::read(&save).unwrap()).unwrap();
            let l = listings(&records)
                .into_iter()
                .find(|l| {
                    (l.country, l.number as i16) != human
                        && !appointments(&records).iter().any(|t| t.player == l.id)
                })
                .unwrap();
            let prices = records.globals.fund_prices();
            let agreed = records.agreed();
            let bidder = if computer {
                records
                    .teams
                    .iter()
                    .find(|t| {
                        t.human() == 0
                            && (t.country(), t.number()) != (l.country, l.number as u8)
                            && t.player_count() < FULL_SQUAD
                            && crate::finances::Balance::of(t, prices, &agreed).investable()
                                >= i64::from(fee)
                    })
                    .map(|t| (t.country(), i16::from(t.number())))
                    .unwrap()
            } else {
                human
            };
            let k = listings(&records).iter().position(|x| x.id == l.id).unwrap();
            let b = records.block_mut(&list_name()).unwrap();
            b[LIST_FIRST + k * LIST_ENTRY + 0x10..][..4].copy_from_slice(&fee.to_le_bytes());
            let lender = (l.country, l.number as i16);
            let day = records.globals.day() as i16 + 1;
            assert!(book(&mut records, LOAN_TALK, day, l.id, lender, bidder, [0; TERMS]));
            let before = appointments(&records);
            let today = before.iter().filter(|t| t.day == day && t.kind == LOAN_TALK).count();
            let talks = loan_day(&mut records, day);
            let after = appointments(&records);
            assert_eq!(after.len(), before.len() - today);
            assert!(!after.iter().any(|t| t.day == day && t.kind == LOAN_TALK));
            talks.into_iter().find(|t| t.player == l.id)
        };
        let talk = run(1000, false).expect("no loan talk");
        assert_eq!(talk.bidders, vec![human]);
        assert_eq!(talk.winner, None);
        assert_eq!(run(i32::MAX, false), None);
        let talk = run(1000, true).expect("no computer loan talk");
        assert_eq!(talk.winner, Some(talk.bidders[0]));
    }
    /// A purchase booked on save A for the human club with a listed player
    /// of another: on its day it leaves the list; the human club bids with
    /// no release fee and drops out when the fee is beyond it.
    #[test]
    #[ignore = "reads the kept saves next to $ANSTOSS3_DIR"]
    fn the_purchase_talks_leave_the_list_on_their_day() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let save = dir.parent().unwrap().join("saves-kept").join("A.an3");
        let human = {
            let records = Records::from_save(Save::read(&save).unwrap()).unwrap();
            let t = records.teams.iter().find(|t| t.human() != 0).unwrap();
            (t.country(), i16::from(t.number()))
        };
        let run = |fee: u16| {
            let mut records = Records::from_save(Save::read(&save).unwrap()).unwrap();
            let l = listings(&records)
                .into_iter()
                .find(|l| {
                    (l.country, l.number as i16) != human
                        && l.terms != POACHED
                        && !appointments(&records).iter().any(|t| t.player == l.id)
                })
                .unwrap();
            records.player_mut(l.id).unwrap().set_fixed_fee(fee);
            let seller = (l.country, l.number as i16);
            let day = records.globals.day() as i16 + 1;
            assert!(book(&mut records, PURCHASE, day, l.id, seller, human, [0; TERMS]));
            let before = appointments(&records);
            let today = before.iter().filter(|t| t.day == day && t.kind == PURCHASE).count();
            let talks = purchase_talk_day(&mut records, day);
            let after = appointments(&records);
            assert_eq!(after.len(), before.len() - today);
            assert!(!after.iter().any(|t| t.day == day && t.kind == PURCHASE));
            assert!(listings(&records).iter().any(|x| x.id == l.id));
            talks.into_iter().find(|t| t.player == l.id)
        };
        let talk = run(0).expect("no purchase talk");
        assert_eq!(talk.bidders, vec![human]);
        assert_eq!(run(u16::MAX), None);
    }

}
