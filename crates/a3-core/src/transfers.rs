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
    /// Each club's participant status (+0x10 + 6k, meaning not traced).
    pub statuses: Vec<u8>,
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
                statuses: (0..n)
                    .map(|i| b.get(e + CLUB_AT + i * CLUB + 4).copied().unwrap_or(0))
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

/// A human club out of a loan or purchase talk on its day, told so at
/// once (`FUN_00768497` when it cannot pay, `FUN_0076851f` when its squad
/// is full; both only for a human's club, `FUN_00677428`). They leave the
/// reason (0 / 1) at `0xa321f4` and the club at `0xa321f6`/`0xa321f8` and
/// post message 0x11f to its manager, whose alert (`FUN_0043ddb9`) reads
/// "ANSTOSS 3 - Achtung <club>!" (2917) over text 2918 or 2919.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Dropout {
    pub player: u16,
    pub club: Side,
    pub squad_full: bool,
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
pub fn loan_day(records: &mut Records, day: i16, told: &mut Vec<Dropout>) -> Vec<LoanTalk> {
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
        let mut bidders = Vec::new();
        for &club in t.clubs.iter().filter(|&&c| c != lender) {
            let Some(team) = records.team(u16::from(club.0), club.1 as u16) else { continue };
            let investable = crate::finances::Balance::of(team, prices, &agreed).investable();
            let squad_full = team.player_count() >= FULL_SQUAD;
            if investable < i64::from(listing.fee as u32) || squad_full {
                if team.human() != 0 {
                    told.push(Dropout {
                        player: t.player,
                        club,
                        squad_full: investable >= i64::from(listing.fee as u32),
                    });
                }
            } else {
                bidders.push(club);
            }
        }
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
    pub opening: Opening,
    /// The seller's frame and each bidder's, in the bidders' order.
    pub asking: Asking,
    pub offers: Vec<Asking>,
    /// The frame whose terms the player took last (`DAT_01c5c1ec`: 0 the
    /// seller's, k the k-th bidder's), and their worth to him with the
    /// frame's mood (`DAT_01c5c1e8`, 1 before any).
    pub winner: Option<usize>,
    pub best: i32,
    /// The talk over the fee with the bidder he took (from asm 0x7694cc),
    /// while a human seller still answers it.
    pub fee: Option<crate::fee_talk::FeeTalk>,
    /// What computer clubs alone settled on (`FUN_0076d021`).
    pub settled: Option<Settled>,
}

/// A purchase talk settled between computer clubs: who takes the player,
/// for what fee, on what contract, and whether at once or for the
/// season's end.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Settled {
    pub buyer: Side,
    pub fee: u32,
    pub terms: [u8; TERMS],
    pub at_once: bool,
}

/// A club's frame in a purchase talk (`0x1c5bf60` + 0x5c per club, the
/// seller's first, filled by `FUN_00767acd`, `FUN_0076e55c` and
/// `FUN_0076d44d` from asm 0x768af8 and, for each bidder, 0x768c65): the
/// salary the player would take there, in thousands, and the terms the
/// club opens on.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Asking {
    /// The club's human manager (+0x00, club +0x8b), 0 for the computer.
    pub manager: u8,
    /// Its league level as the talk weighs it (+0x50, the seller's at most
    /// 2 and `DAT_01c5c250`; a bidder's by `FUN_006755b8`).
    pub level: u8,
    /// The least and the most salary he would talk about (+0x48, +0x4c).
    pub least: i32,
    pub most: i32,
    /// His mood's weight (+0x52: the seller's from table 0x8819b4 by
    /// `FUN_00770877`, a bidder's 0).
    pub mood: i16,
    /// A bidder's copy of the opening's remark flags (+0x58, -0x3c).
    pub remarks: u8,
    /// A bidder's participant status (+0x44, the appointment's +0x10 + 6k,
    /// meaning not traced); 0 for the seller.
    pub status: u8,
    /// The terms the club opens on (+0x2c … +0x3f, laid out as
    /// [`terms`]: salary, appearance bonus, …, years at 12, flags at 14).
    pub terms: [u8; TERMS],
    /// Out of the talk (+0x02): a bid beyond what the club would pay.
    pub out: bool,
    /// A bidder's remark to the player (+0x5a, a text id, 0 none).
    pub remark: u16,
}

impl Asking {
    pub fn salary(&self) -> u16 {
        u16::from_le_bytes([self.terms[0], self.terms[1]])
    }
    pub fn appearance(&self) -> u16 {
        u16::from_le_bytes([self.terms[2], self.terms[3]])
    }
    pub fn years(&self) -> i8 {
        self.terms[12] as i8
    }
    pub fn flags(&self) -> u16 {
        u16::from_le_bytes([self.terms[14], self.terms[15]])
    }
}

/// +0x3a's bits: drawn at even odds (meaning not traced), and "ab
/// sofort" (the season young and a human club or a bidder of status, and
/// R+0x161 set).
pub const ASKING_DRAWN: u16 = 0x2000;
pub const ASKING_AT_ONCE: u16 = 0x20;
/// `FUN_00770877`'s levels to +0x52 (table 0x8819b4).
const MOOD_WEIGHTS: [i16; 4] = [15, 0, -15, -35];
/// The salary's unit (`DAT_00a3b3e4`).
const SALARY_UNIT: f64 = 1000.0;
/// What `FUN_0076e55c` and `FUN_0076d44d` take of his demand, and the
/// most by the club's level (0x80e748; 0x835110 first league, 0x835118
/// below).
const LEAST_SHARE: f64 = 0.8;
const MOST_SHARE: [f64; 2] = [1.75, 1.65];
/// `FUN_0076e55c`'s bounds on the seller's money (`DAT_00a3b308`, 3 Mio)
/// and its steps on the bidder's (`DAT_00a3b2c0` … `DAT_00a3b348`, then
/// the negative ones), from the most down, in the game's money.
const SELLER_DEEP: i64 = 3_000_000;
const BIDDER_STEPS: [(i64, f32); 10] = [
    (20_000_000, 1.8),
    (16_000_000, 1.7),
    (12_000_000, 1.6),
    (8_000_000, 1.5),
    (5_000_000, 1.4),
    (2_000_000, 1.2),
    (500_000, 1.05),
    (-2_000_000, 0.95),
    (-5_000_000, 0.9),
    (-10_000_000, 0.8),
];
const BIDDER_POOR: f32 = 0.7;
/// `FUN_00708b80`'s pick value above which a buyer pays more.
const WANTED: i32 = 0x78;
/// The squad beyond which each man takes a fiftieth off.
const SQUAD_EASE: i32 = 0x12;

/// Whether the pick values him above 120 (`FUN_00708b80`: the pick's
/// percent weighed by his form), for which a club pays more.
pub(crate) fn wanted(values: &crate::value::ValueTables, p: &crate::records::Player) -> bool {
    values.pick_value(p) > WANTED
}

/// How much more or less a club pays him than his own (`FUN_0076e55c`'s
/// factor): 1 at his own club; else by the seller's money (0.85 deep in
/// debt, 0.95 in debt), more for a man the pick values above 120, for a
/// position the buyer lacks (0.2) or has once (0.1), less for each man of
/// a squad beyond 18, then times the buyer's money's step, less 0.4 from a
/// seller of a marked country (0x89dfc8), 0.15 out five weeks or more
/// (0.05 a week), 0.6 banned, and never under 0.2.
fn bargain_factor(
    records: &Records,
    tables: (&crate::contract::ContractTables, &crate::value::ValueTables),
    p: &crate::records::Player,
    seller: &crate::records::Team,
    club: &crate::records::Team,
) -> f32 {
    if (seller.country(), seller.number()) == (club.country(), club.number()) {
        return 1.0;
    }
    let prices = records.globals.fund_prices();
    let money = crate::finances::figure(seller, prices);
    let mut f: f32 = if money < -SELLER_DEEP {
        0.85
    } else if money < 0 {
        0.95
    } else {
        1.0
    };
    let position = p.positions()[0];
    if wanted(tables.1, p) {
        f += 0.1;
    }
    // Club +0xbf0[position]: his position's men in the buyer's squad.
    let same = club
        .players()
        .into_iter()
        .filter(|&id| records.player(id).is_some_and(|q| q.positions()[0] == position))
        .count();
    match same {
        0 => f += 0.2,
        1 => f += 0.1,
        _ => {}
    }
    let squad = i32::from(club.player_count() as i16);
    if squad > SQUAD_EASE {
        f = (f64::from(f) - f64::from(squad - SQUAD_EASE) / 50.0) as f32;
    }
    let money = crate::finances::figure(club, prices);
    f *= BIDDER_STEPS.iter().find(|&&(above, _)| money > above).map_or(BIDDER_POOR, |&(_, s)| s);
    if tables.0.marked_country(seller.country()) {
        f -= 0.4;
    }
    let out = crate::load::days_to(&records.globals, p.injured_to());
    if out >= 0x23 {
        f -= 0.15;
    } else if out >= 7 {
        f -= 0.05;
    }
    if crate::load::days_to(&records.globals, p.long_to()) != 0 {
        f -= 0.6;
    }
    f.max(0.2)
}

/// One club's frame (`FUN_00768638` asm 0x768af8–0x768b9f for the
/// seller, 0x768c80–0x768da1 for a bidder): the bounds of
/// `FUN_0076e55c` — the least 0.8 of his demand (`FUN_00708012`), in
/// thousands, times the factor only when that is under 1, the most 1.75
/// or 1.65 of it by the club's league — and the terms of
/// `FUN_0076d44d`: a salary of 0.8 of his demand and an appearance bonus
/// of five of it per matchday, each spread 90 to 110 per cent, the years
/// (`FUN_0076d3ac`), the even draw, "ab sofort". The first bidder of
/// the appointment opens on the terms booked with it, unless their
/// salary is 0. (A human bidder also counts at the seller's club
/// +0xe04[manager], whose place in the save is not traced.)
fn club_asking(
    records: &Records,
    random: &mut crate::random::Random,
    tables: (&crate::contract::ContractTables, &crate::value::ValueTables),
    t: &Appointment,
    k: usize,
    remarks: u8,
    day: i16,
) -> Option<Asking> {
    let seller = t.clubs[0];
    let side = *t.clubs.get(k)?;
    let seller_team = records.team(u16::from(seller.0), seller.1 as u16)?;
    let team = records.team(u16::from(side.0), side.1 as u16)?;
    let p = records.player(t.player)?;
    let manager = team.humans()[1];
    let level = if k == 0 {
        team.level().min(2)
    } else {
        // FUN_006755b8: the main country's leagues, the rest as its first.
        let main = records.globals.main_country();
        match team.level() {
            l if i16::from(side.0) != main => if l > 0 { 0 } else { l },
            l if main == 0x10 => l.min(3),
            l => l.min(2),
        }
    };
    let age = crate::free_agents::age(records, p);
    let star = records.globals.stars().get(usize::from(p.positions()[0])) == Some(&t.player);
    let signee = crate::contract::Signee::of(
        p,
        age,
        crate::statistics::form_today(p),
        tables.1,
        star,
        records.globals.start_year(),
    );
    let demand = f64::from(tables.0.demand(&signee));
    let factor = bargain_factor(records, tables, p, seller_team, team);
    let v = f64::from((demand / SALARY_UNIT) as f32);
    let least = if factor < 1.0 { v * f64::from(factor) * LEAST_SHARE } else { v * LEAST_SHARE } as i32;
    let most = (v * MOST_SHARE[usize::from(team.level() > 0)]) as i32;
    let mood = if k == 0 {
        let content = if t.player < 0x4269 {
            crate::mood::content_level(crate::mood::parts(p))
        } else {
            1
        };
        MOOD_WEIGHTS[content]
    } else {
        0
    };
    let status = if k == 0 { 0 } else { t.statuses.get(k).copied().unwrap_or(0) };
    let mut asking = Asking {
        manager,
        level,
        least,
        most,
        mood,
        remarks: if k == 0 { 0 } else { remarks },
        status,
        terms: [0; TERMS],
        out: false,
        remark: 0,
    };
    if k == 1 {
        asking.terms = t.terms;
    }
    if k != 1 || asking.salary() == 0 {
        asking.terms = opening_terms(records, random, p, age, demand, seller_team, &asking, day);
    }
    Some(asking)
}

/// `FUN_0076d44d`'s terms.
#[allow(clippy::too_many_arguments)]
fn opening_terms(
    records: &Records,
    random: &mut crate::random::Random,
    p: &crate::records::Player,
    age: i32,
    demand: f64,
    seller: &crate::records::Team,
    asking: &Asking,
    day: i16,
) -> [u8; TERMS] {
    let a = f64::from((demand * LEAST_SHARE) as f32);
    let salary = (a / SALARY_UNIT) as i32 as u16;
    let matchdays = match crate::statistics::league_length(records, seller) {
        ..1 => 0x22,
        n => i32::from(n),
    };
    let appearance = (i32::from(salary) * 5 / matchdays) as u16;
    let salary = ((random.below(21) + 90) * u32::from(salary) / 100) as u16;
    let appearance = ((random.below(21) + 90) * u32::from(appearance) / 100) as u16;
    let years = crate::free_agents::fresh_years(p, age, random);
    let mut flags = 0;
    if random.below(2) != 0 {
        flags |= ASKING_DRAWN;
    }
    if day <= LATE_SEASON && (asking.manager != 0 || asking.status != 0) {
        flags |= ASKING_AT_ONCE;
    }
    // `FUN_007588bc`: L+0x98, which is R+0x161 here — Nsoki's 3 keeps
    // the bit at the original's day 64 where his R+0x162 is 0.
    if p.get::<u8>(0x161) == 0 {
        flags &= !ASKING_AT_ONCE;
    }
    let mut t = [0; TERMS];
    t[0..2].copy_from_slice(&salary.to_le_bytes());
    t[2..4].copy_from_slice(&appearance.to_le_bytes());
    t[12] = years as u8;
    t[14..16].copy_from_slice(&flags.to_le_bytes());
    t
}

/// How a purchase talk opens (`FUN_00768638` from asm 0x7687af to
/// 0x768af8), before its bidders are looked at.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Opening {
    /// His partner's stance on the move (`DAT_01c5c1f8`, 0 to 6): 0 none
    /// or content, 2 against it, 3 wants money, 4 and 5 two moods of the
    /// wife, by his id's parity, 6 his nation's kind 3.
    pub stance: u8,
    /// His remark's flags (-0x3c: 2 his wife came along, 4 too young to
    /// leave) and its text (-0x1a, 0x5c7 to 0x5ca, 0 none).
    pub flags: u8,
    pub remark: u16,
    /// The share the talk opens on (`DAT_01c5c254`): 50 with his contract
    /// run out, else 100.
    pub share: u8,
    /// Whether the seller lets him go (-0x28).
    pub released: bool,
}

/// R+0x1ca's traits (L+0x12) the opening reads: wants money
/// ("Geldgeier"), and a family man (meaning by its use).
const GREEDY: u16 = 0x40;
const FAMILY: u16 = 0x200;
/// R+0x15e's bit 0x40 (L+0x96), which keeps a computer seller's player
/// unreleased (meaning not traced).
const KEPT: u16 = 0x40;
/// The listing's terms (E+0xd) that release him: 1 and 2.
const RELEASING_TERMS: [u8; 2] = [1, 2];

/// The partner's stance bits R+0x27f bits 4 and 5 (`FUN_00705dbe`,
/// written by `FUN_00705de5`, clamped to 0..2).
fn stance_bits(p: &crate::records::Player) -> u8 {
    p.problems() >> 4 & 3
}

fn set_stance_bits(p: &mut crate::records::Player, value: u8) {
    p.set_problems(p.problems() & 0xcf | value.min(2) << 4);
}

/// `FUN_00768638`'s opening, asm 0x7687af–0x768af8: the partner's stance
/// (a wife at ease, R+0x27e kind 3 and standing 0, by his traits; else a
/// strong man's partner turns against it three times in ten), a nation
/// of kind 3 (`DAT_00ae5678`) overruling; then the remark — a wife who
/// came along at even odds, a man under 25 who would rather stay, any
/// other one in a hundred; the share; whether the seller lets him go.
pub fn purchase_opening(
    records: &mut Records,
    random: &mut crate::random::Random,
    t: &Appointment,
    nation_kinds: &[u8],
) -> Option<Opening> {
    let &seller = t.clubs.first()?;
    let today = crate::free_agents::today(records);
    let listing = listings(records).into_iter().find(|l| l.id == t.player)?;
    let start_year = records.globals.start_year();
    let seller_human = records
        .team(u16::from(seller.0), seller.1 as u16)
        .is_some_and(|team| team.human() != 0);
    let p = records.player_mut(t.player)?;
    let mut stance = stance_bits(p);
    let married = p.partner() & 3 == 3;
    let strong = p.strength() > 10;
    let traits = p.traits()[1];
    if married && p.partner() >> 2 & 7 == 0 {
        set_stance_bits(p, 1);
        stance = if strong && traits & GREEDY != 0 {
            3
        } else if traits & FAMILY != 0 {
            if t.player % 2 == 0 { 5 } else { 4 }
        } else {
            0
        };
    } else if strong && random.below(100) < 30 {
        set_stance_bits(p, 2);
        stance = 2;
    } else if stance == 1 {
        set_stance_bits(p, 0);
        stance = 0;
    }
    if nation_kinds.get(usize::from(p.nation())) == Some(&3) {
        set_stance_bits(p, 0);
        stance = 6;
    }
    let (mut flags, mut remark) = (0, 0);
    let bits = stance_bits(p);
    if married && bits == 1 && random.below(2) != 0 {
        flags |= 2;
        remark = 0x5c7;
        stance = 3;
    } else if bits == 2 {
        // The original's third case (under 20, ten league matches, a
        // talent) lies under the first and is never reached.
        if today.is_some_and(|d| crate::statistics::age_on(p, d) < 25) {
            flags |= 4;
            remark = 0x5c8;
        } else if random.below(100) == 0 {
            remark = 0x5ca;
        }
    }
    let p = records.player(t.player)?;
    let years = (i32::from(p.contract() & 0xfff) - i32::from(start_year)).max(0);
    let share = if years == 0 { 50 } else { 100 };
    // Every way through asm 0x7689f5–0x768a82 lands on 0 but a contract
    // run out.
    let mut released = RELEASING_TERMS.contains(&listing.terms) || years == 0;
    if !released && !seller_human && p.terms() & KEPT == 0 {
        released = true;
    }
    if t.clubs.iter().skip(1).any(|&c| c == seller) {
        released = false;
    }
    Some(Opening { stance, flags, remark, share, released })
}

/// The release fee's unit (`DAT_00a3b3bc`).
const RELEASE_UNIT: i64 = 50_000;
/// Past this day of the season a release fee bars no bidder
/// (`FUN_0075827a`: `DAT_00b5cbd4` > 0xd8).
const LATE_SEASON: i16 = 0xd8;

/// The next of the day's purchase talks (`FUN_0076cb3e`'s last loop:
/// kind 3 → `FUN_00768638`), `None` when none is left; the player and the
/// talk as far as it got, its fee talk still to answer by a human seller
/// ([`PurchaseTalk::fee`]), and [`close_purchase_talk`] after it. The
/// talk ends at once when the player has left the selling club
/// (`FUN_006968c4`) or the transfer list (`FUN_0075974f`); else each other
/// club of the appointment drops out when the player's release fee
/// (L+0x8e, R+0x156, in 50 000s) is set, the season is not past day 216
/// and the club can invest less (`FUN_00767b6a`, a human told by
/// `FUN_00768497`), or when its squad is full (`FUN_00767c78`, told by
/// `FUN_0076851f`); the seller is skipped (`FUN_006782aa`); before them
/// the seller's frame, and each bidder left gets his own ([`Asking`]) with
/// his remark and mood; then the bidders bargain his contract
/// ([`PurchaseTalk::winner`]) and, a bidder's taken, the clubs his fee.
#[allow(clippy::too_many_arguments)]
pub fn next_purchase_talk(
    records: &mut Records,
    random: &mut crate::random::Random,
    tables: (&crate::contract::ContractTables, &crate::value::ValueTables),
    medical: &crate::medical::MedicalTables,
    nation_kinds: &[u8],
    day: i16,
    told: &mut Vec<Dropout>,
) -> Option<(u16, Option<PurchaseTalk>)> {
    let t = appointments(records).into_iter().find(|t| t.day == day && t.kind == PURCHASE)?;
    let talk = purchase_bidders(records, random, tables, medical, nation_kinds, &t, day, told);
    Some((t.player, talk))
}

/// A purchase talk's end: a listing whose terms read [`POACHED`] (E+0xd =
/// 3) leaves the transfer list (`FUN_0075a71f`), and the appointment goes
/// (`FUN_0076cb3e`). (The move after a fee agreed with a human club —
/// `FUN_0070ca69` — and the talk's messages 0x96 are not built.)
pub fn close_purchase_talk(records: &mut Records, player: u16) {
    if listings(records).iter().any(|l| l.id == player && l.terms == POACHED) {
        take_off(records, player);
    }
    cancel_talk(records, player);
}

/// The fee talk after the bidders' rounds (`FUN_00768638` asm
/// 0x7694cc–0x7698d1), for a bidder taken (the seller's own taken is his
/// contract's renewal, not built). No fee is talked over for a man listed
/// "ablösefrei" (E+0xd 2), one whose contract runs out — no year left, or
/// its last and the bid not "ab sofort" (+0x3a 0x20, which day 216 past
/// clears, `FUN_0075827a`) —, nor between clubs of a country (the youth's
/// and amateurs' compensation, `FUN_0069d8b2` … `FUN_00708836`), and a
/// release fee is paid as it stands; those moves are not built. Of the
/// rest a human seller's is talked over ([`crate::fee_talk::FeeTalk`]); two
/// computer clubs settle on the bidder's most, and a computer seller's
/// turns with a human bidder (`FUN_0076e018`) are not built.
fn fee_opening(
    records: &Records,
    random: &mut crate::random::Random,
    tables: (&crate::contract::ContractTables, &crate::value::ValueTables),
    medical: &crate::medical::MedicalTables,
    talk: &PurchaseTalk,
    day: i16,
) -> Option<crate::fee_talk::FeeTalk> {
    let k = talk.winner.filter(|&k| k > 0)?;
    let offer = talk.offers.get(k - 1)?;
    let buyer = *talk.bidders.get(k - 1)?;
    let p = records.player(talk.player)?;
    let years = i32::from(p.contract() & 0xfff) - i32::from(records.globals.start_year());
    let at_once = offer.flags() & ASKING_AT_ONCE != 0 && day <= LATE_SEASON;
    if listed_terms(records, talk.player) == Some(FREE_TRANSFER) || years < 1 || (years == 1 && !at_once) {
        return None;
    }
    if buyer.0 == talk.seller.0 || p.fixed_fee() > 0 {
        return None;
    }
    let managers = [talk.asking.manager, offer.manager];
    let mut fee =
        crate::fee_talk::FeeTalk::open(records, random, tables, medical, talk.player, talk.seller, buyer, managers)?;
    fee.at_once = at_once;
    (managers[0] != 0).then_some(fee)
}

/// `FUN_00768638` up to its bargaining: who still bids.
#[allow(clippy::too_many_arguments)]
fn purchase_bidders(
    records: &mut Records,
    random: &mut crate::random::Random,
    tables: (&crate::contract::ContractTables, &crate::value::ValueTables),
    medical: &crate::medical::MedicalTables,
    nation_kinds: &[u8],
    t: &Appointment,
    day: i16,
    told: &mut Vec<Dropout>,
) -> Option<PurchaseTalk> {
    let &seller = t.clubs.first()?;
    let player = records.player(t.player)?;
    if t.player == 0 || (player.club()[0], i16::from(player.club()[1])) != seller {
        return None;
    }
    let opening = purchase_opening(records, random, t, nation_kinds)?;
    let asking = club_asking(records, random, tables, t, 0, 0, day)?;
    let player = records.player(t.player)?;
    let fee = i64::from(player.fixed_fee()) * RELEASE_UNIT;
    let prices = records.globals.fund_prices();
    let agreed = records.agreed();
    let mut bidders = Vec::new();
    let mut offers = Vec::new();
    for (k, &club) in t.clubs.iter().enumerate().skip(1).filter(|&(_, &c)| c != seller) {
        let Some(team) = records.team(u16::from(club.0), club.1 as u16) else { continue };
        let affords = fee == 0
            || day > LATE_SEASON
            || crate::finances::Balance::of(team, prices, &agreed).investable() >= fee;
        let squad_full = team.player_count() >= FULL_SQUAD;
        if !affords || squad_full {
            if team.human() != 0 {
                told.push(Dropout { player: t.player, club, squad_full: affords });
            }
        } else if let Some(mut offer) = club_asking(records, random, tables, t, k, opening.flags, day) {
            bidder_mood(records, t.player, seller, club, &mut offer);
            bidders.push(club);
            offers.push(offer);
        }
    }
    if bidders.is_empty() {
        return None;
    }
    let mut talk = PurchaseTalk {
        player: t.player,
        seller,
        bidders,
        opening,
        asking,
        offers,
        winner: None,
        best: 1,
        fee: None,
        settled: None,
    };
    if talk.asking.manager == 0 && talk.offers.iter().all(|f| f.manager == 0) {
        talk.settled = computer_settlement(records, random, tables, medical, t, &talk, day);
        return Some(talk);
    }
    contract_rounds(records, random, tables, &mut talk, day);
    talk.fee = fee_opening(records, random, tables, medical, &talk, day);
    Some(talk)
}

/// What a club of the second league down pays one of the first or the
/// second for a man whose contract ran out, in Germany (`DAT_00a3b37c`,
/// `DAT_00a3b394`).
const RUN_OUT_FEES: [u32; 2] = [100_000, 40_000];
const GERMANY: u8 = 0xe;

/// A purchase talk of computer clubs alone (`FUN_0076d021`, from
/// `FUN_00768638` asm 0x76914d), unless the appointment names one club
/// only. Of its other clubs that are no human's and have squad room
/// (`FUN_00767c78`) the one of the highest weight (+0x11 + 6k), the first
/// on a tie, takes him on fresh terms (`FUN_00709fb6`) of two years at
/// least — at once when the frame at its place in the appointment bids
/// "ab sofort" (+0x3a 0x20; the frames stand closed up, so after a
/// dropout it is another club's or none) and day 216 is not past
/// (`FUN_0075827a`). With no year of his contract left, or its last and
/// not at once, there is no fee but the German one above. Else
/// `FUN_0076d60e` sets the bounds — the talk's winner (+0x2b4) still
/// nought, so the seller's own club stands in for the bidder — and the
/// fee is the bidder's most, or his release fee when the seller's least
/// reaches it. Then the move (`FUN_0070ca69`).
#[allow(clippy::too_many_arguments)]
fn computer_settlement(
    records: &mut Records,
    random: &mut crate::random::Random,
    tables: (&crate::contract::ContractTables, &crate::value::ValueTables),
    medical: &crate::medical::MedicalTables,
    t: &Appointment,
    talk: &PurchaseTalk,
    day: i16,
) -> Option<Settled> {
    if t.clubs.len() == 1 {
        return None;
    }
    let seller = talk.seller;
    let mut best: Option<(u8, usize)> = None;
    for (k, &club) in t.clubs.iter().enumerate().skip(1) {
        let Some(team) = records.team(u16::from(club.0), club.1 as u16) else { continue };
        if team.human() != 0 || team.player_count() >= FULL_SQUAD {
            continue;
        }
        let weight = t.weights.get(k).copied().unwrap_or(0);
        if best.is_none_or(|(w, _)| weight > w) {
            best = Some((weight, k));
        }
    }
    let (_, k) = best?;
    let buyer = t.clubs[k];
    let levels = [seller, buyer].map(|c| records.team(u16::from(c.0), c.1 as u16).map_or(0, |team| team.level()));
    let mut terms = crate::free_agents::fresh_contract(records, t.player, levels[1], tables, random)?;
    if (terms[12] as i8) < 2 {
        terms[12] = 2;
    }
    let frame = if k == 0 { Some(&talk.asking) } else { talk.offers.get(k - 1) };
    let at_once = frame.is_some_and(|f| f.flags() & ASKING_AT_ONCE != 0) && day <= LATE_SEASON;
    let p = records.player(t.player)?;
    let years = (i32::from(p.contract() & 0xfff) - i32::from(records.globals.start_year())) as i8;
    let fixed = i64::from(p.fixed_fee()) * RELEASE_UNIT;
    let fee = if years < 1 || (years == 1 && !at_once) {
        if seller.0 == GERMANY && levels[0] > 1 && buyer.0 == GERMANY {
            RUN_OUT_FEES.get(usize::from(levels[1])).copied().unwrap_or(0)
        } else {
            0
        }
    } else {
        let managers = [talk.asking.manager; 2];
        let bounds =
            crate::fee_talk::FeeTalk::open(records, random, tables, medical, t.player, seller, seller, managers)?;
        if fixed == 0 || i64::from(bounds.least as u32) * 1000 < fixed {
            (bounds.most as u32).wrapping_mul(1000)
        } else {
            fixed as u32
        }
    };
    if at_once {
        crate::deals::move_now(records, random, t.player, seller, buyer, fee, &terms);
    } else {
        crate::deals::agree(records, random, t.player, seller, buyer, fee, &terms);
    }
    Some(Settled { buyer, fee, terms, at_once })
}

/// A bidder's remark and mood (`FUN_00768638` asm 0x768da1–0x7690f3),
/// the first that holds: more than three better men at his position
/// there (`FUN_005ee351`) for a man over 23 — a regular place
/// (0x5d3); the seller's rival (team +0x122 its number) for a man under
/// 10, or a man of trait 0x400 — -20 (0x5d4); no place among its best
/// two keepers, six defenders, five midfielders and three forwards
/// (`FUN_00777d37`) — under 25 0x5d5, else 0x5d6 with four years and a
/// regular place; his former club (`FUN_00704537`) after fifteen
/// seasons — +30 (0x5d7); a lower league for a man over 31 above 9 —
/// -40 (0x5d8); two children at school (`FUN_007020db`) and a club near
/// the seller's in his country (`FUN_006778e3`, ten at most) — 0x5d9;
/// a team in a mood over 130 — +20 (0x5dc). Then its buildings: +5 for
/// mark 0x200, else +8 for 0x100 (team +0x95c, `FUN_0069f96d`,
/// `FUN_0069f89f`). (A human bidder's own part, his trainer's standing
/// and his watch list, asm 0x768fc5–0x7690a6, is not built.)
fn bidder_mood(records: &Records, player: u16, seller: Side, side: Side, a: &mut Asking) {
    let (Some(p), Some(seller), Some(team)) = (
        records.player(player),
        records.team(u16::from(seller.0), seller.1 as u16),
        records.team(u16::from(side.0), side.1 as u16),
    ) else {
        return;
    };
    let age = crate::free_agents::age(records, p);
    let strength = p.strength();
    let position = p.positions()[0];
    let set_flags = |a: &mut Asking, bits: u16| {
        let f = a.flags() | bits;
        a.terms[14..16].copy_from_slice(&f.to_le_bytes());
    };
    let better = team
        .players()
        .into_iter()
        .filter_map(|id| records.player(id).map(|q| (id, q)))
        .filter(|&(id, q)| id < 0x4269 && id != player && q.positions()[0] == position && strength < q.strength())
        .count();
    let rival = i32::from(seller.get::<u8>(RIVAL_AT) as i8) == i32::from(team.number());
    let near = || {
        let (a, b) = (team.map_at(), seller.map_at());
        let (dx, dy) = (i32::from(a[0]) - i32::from(b[0]), i32::from(a[1]) - i32::from(b[1]));
        (f64::from(dx * dx + dy * dy).sqrt() * 1.0) as i32 <= NEAR
    };
    if better > 3 && age > 23 {
        a.remark = 0x5d3;
        a.remarks |= 1;
        set_flags(a, crate::contract::REGULAR_PLACE);
    } else if (rival && strength < 10) || p.traits()[1] & HOMESICK != 0 {
        a.remark = 0x5d4;
        a.mood = -20;
    } else if !among_the_best(records, team, p) {
        if age < 25 {
            a.remark = 0x5d5;
            a.remarks |= 8;
        } else {
            a.remark = 0x5d6;
            a.remarks |= 0x18;
            a.terms[12] = 4;
            set_flags(a, crate::contract::REGULAR_PLACE);
        }
    } else if p.get::<i16>(CAREER_COUNT) >= 1
        && (p.get::<u8>(FIRST_CLUB), p.get::<u8>(FIRST_CLUB + 1)) == (team.country(), team.number())
        && records.globals.seasons() > 15
    {
        a.remark = 0x5d7;
        a.mood = 30;
    } else if a.level > 0 && strength > 9 && age > 31 {
        a.remark = 0x5d8;
        a.mood = -40;
    } else if school_children(records, p) >= 2 && team.country() == p.club()[0] && near() {
        a.remark = 0x5d9;
    } else if team.mood() > 0x82 {
        a.remark = 0x5dc;
        a.mood = 20;
    }
    let marks = team.get::<u16>(MARKS_AT);
    if marks & 0x200 != 0 {
        a.mood += 5;
    } else if marks & 0x100 != 0 {
        a.mood += 8;
    }
}

/// Team +0x122 (club +0xf4): the number of the club's rival, as
/// [`bidder_mood`] reads it.
const RIVAL_AT: usize = 0x122;
/// A trait (traits[1] bit 10) that keeps a weak man from moving.
const HOMESICK: u16 = 0x400;
/// R+0x212 the entries of his career, R+0x30 the first one's club.
const CAREER_COUNT: usize = 0x212;
const FIRST_CLUB: usize = 0x30;
/// The team's marks of its buildings (team +0x95c).
const MARKS_AT: usize = 0x95c;
/// How far apart on the map two clubs count as near.
const NEAR: i32 = 10;
/// R+0x174: his children's birthdays (day, month, year), R+0x1f6 of them.
const CHILDREN_AT: usize = 0x174;

/// His children aged 6 to 17 (`FUN_007020db(2, 6, 0x11)`, each by
/// `FUN_0067582a`: years since the birthday's year, one less before its
/// day, none before birth).
fn school_children(records: &Records, p: &crate::records::Player) -> usize {
    let [day, month, year] = records.globals.date();
    (0..usize::try_from(p.children()).unwrap_or(0))
        .filter(|&k| {
            let at = CHILDREN_AT + 4 * k;
            let (d, m, y) = (p.get::<u8>(at), p.get::<u8>(at + 1), p.get::<u16>(at + 2));
            let mut age = i32::from(year as i16) - i32::from(y as i16) - 1;
            if u16::from(m) < month || (u16::from(m) == month && u16::from(d) <= day) {
                age += 1;
            }
            (6..=0x11).contains(&age.max(0))
        })
        .count()
}

/// Whether a man would be among a club's best of his kind by his score
/// (`FUN_00777d37`, `FUN_00777544`): two keepers, six defenders, five
/// midfielders, three forwards, he behind its men of an equal score; a
/// squad under 16 takes anyone.
fn among_the_best(records: &Records, team: &crate::records::Team, p: &crate::records::Player) -> bool {
    const GROUPS: [(u8, u8, usize); 4] = [(1, 1, 2), (2, 5, 6), (6, 9, 5), (10, 10, 3)];
    if (team.player_count() as i16) < 0x10 {
        return true;
    }
    let group = |q: &crate::records::Player| GROUPS.iter().position(|&(lo, hi, _)| (lo..=hi).contains(&q.positions()[0]));
    let Some(g) = group(p) else { return false };
    let mut scores: Vec<(u16, i32)> = team
        .players()
        .into_iter()
        .filter_map(|id| records.player(id).map(|q| (id, q)))
        .filter(|(_, q)| group(q) == Some(g))
        .map(|(id, q)| (id, crate::season_odds::score(q)))
        .collect();
    scores.push((p.id(), crate::season_odds::score(p)));
    scores.sort_by(|a, b| b.1.cmp(&a.1));
    scores.iter().take(GROUPS[g].2).any(|&(id, _)| id == p.id())
}

/// The bidders' rounds over the player's contract (`FUN_00768638` asm
/// 0x7690f8–0x7693f9). Only with a human manager among the frames
/// (-0x14; a talk of computer clubs alone is [`computer_settlement`]'s)
/// and unless the first pre-season stands still (`FUN_005ef56c`).
/// A human party hears of it (message 0x9a by `FUN_00768366`,
/// `FUN_0076a49a`, the remark by `FUN_0076a4ee`; not built). Then each
/// frame still in, from the first bidder on and from the seller's on
/// later passes unless he lets the man go (-0x28), bids in turn — a
/// computer club by `FUN_0076a307`, a human one by `FUN_0076a19b`, whose
/// dialog is not built, so he leaves the talk — until one frame or none
/// is left in.
fn contract_rounds(
    records: &Records,
    random: &mut crate::random::Random,
    tables: (&crate::contract::ContractTables, &crate::value::ValueTables),
    talk: &mut PurchaseTalk,
    day: i16,
) {
    let mut sides: Vec<Side> = vec![talk.seller];
    sides.extend(&talk.bidders);
    let mut frames: Vec<Asking> = std::iter::once(talk.asking).chain(talk.offers.iter().copied()).collect();
    if frames.iter().all(|f| f.manager == 0) || crate::tournament::still(records, day) {
        return;
    }
    let first = usize::from(talk.opening.released);
    let n = frames.len();
    let mut k = 1;
    loop {
        while k < n {
            if !frames[k].out {
                if frames[k].manager != 0 {
                    frames[k].out = true;
                } else if let Some(at) = computer_bid(records, random, tables, talk, sides[k], &mut frames[k]) {
                    talk.winner = Some(k);
                    talk.best = at;
                }
            }
            if (first..n).filter(|&j| !frames[j].out).count() <= 1 {
                talk.asking = frames[0];
                talk.offers = frames[1..].to_vec();
                return;
            }
            k += 1;
        }
        k = first;
    }
}

/// A computer club's bid (`FUN_0076a307`): it raises its terms
/// (`FUN_0076efbb`) until they cost more than it would pay (the frame's
/// most under the salary: out) or the player takes them
/// (`FUN_0076f32a`); then their worth to him, with the frame's mood.
fn computer_bid(
    records: &Records,
    random: &mut crate::random::Random,
    tables: (&crate::contract::ContractTables, &crate::value::ValueTables),
    talk: &PurchaseTalk,
    side: Side,
    frame: &mut Asking,
) -> Option<i32> {
    let p = records.player(talk.player)?;
    let team = records.team(u16::from(side.0), side.1 as u16)?;
    let years = i32::from(p.contract() & 0xfff) - i32::from(records.globals.start_year());
    let signing_too = frame.flags() & ASKING_AT_ONCE == 0 && years == 1;
    loop {
        raise_bid(records, random, tables.0, p, team, talk.asking.level, signing_too, &mut frame.terms);
        if frame.most < i32::from(frame.salary()) {
            frame.out = true;
            return None;
        }
        let worth = offer_worth(records, tables, p, team, &frame.terms)? + i32::from(frame.mood);
        // At his own club he takes the equal of the best; elsewhere only
        // more.
        let own = (p.club()[0], p.club()[1]) == (team.country(), team.number());
        let beats = if own { worth >= talk.best } else { worth > talk.best };
        if worth > i32::from(talk.opening.share) && beats {
            return Some(worth);
        }
    }
}

/// What terms are worth to the player at a club (`FUN_00709c52`).
pub(crate) fn offer_worth(
    records: &Records,
    tables: (&crate::contract::ContractTables, &crate::value::ValueTables),
    p: &crate::records::Player,
    team: &crate::records::Team,
    terms: &[u8; TERMS],
) -> Option<i32> {
    use crate::contract::{Club, Signee, Terms};
    let age = crate::free_agents::age(records, p);
    let star = records.globals.stars().get(usize::from(p.positions()[0])) == Some(&p.id());
    let form = crate::statistics::form_today(p);
    let signee = Signee::of(p, age, form, tables.1, star, records.globals.start_year());
    let squad: Vec<_> = team
        .players()
        .into_iter()
        .filter_map(|id| records.player(id).map(|q| (id, q.positions()[0], q.strength())))
        .collect();
    // The bonuses count the matchdays of its league as `FUN_00692997`
    // gives them, 18 for a country not played (`FUN_00676c03`): save A's
    // bidder 59/3 takes Nsoki's terms at the original's 23rd raise so.
    let (_, played) = crate::tickets::league_days(records, team);
    let matchdays = if records.globals.played(i16::from(team.country())) {
        i32::from(crate::statistics::league_length(records, team))
    } else {
        NOT_PLAYED_MATCHDAYS
    };
    let month = usize::from(records.globals.date()[1]);
    let club = Club { level: team.level(), squad: &squad, matchdays, played, month };
    let word = |at: usize| u16::from_le_bytes([terms[at], terms[at + 1]]);
    let t = Terms {
        salary: word(0),
        appearance: word(2),
        goal: word(4),
        fee: word(6),
        years: terms[12] as i8,
        flags: word(14),
    };
    let signing = u32::from_le_bytes([terms[8], terms[9], terms[10], terms[11]]) as i32;
    Some(tables.0.offer_value(&signee, &club, &t, signing))
}

/// One raise of a computer club's bid (`FUN_0076efbb`): the bits of the
/// club's league in the terms (1 for a country not played, else its
/// level up to 2 and one when the man's luck lies below the odds of his
/// strength, one more for a man under 23 below 13), then by a draw in a
/// hundred: under 55 the salary — or, at even odds, the signing fee where
/// his last year runs and the bid is not "ab sofort" — by 5 per cent;
/// under 90 the appearance bonus by 5 per cent, at most 2000; under 93 a
/// forward's goal bonus by 50, 20 or 5 as the seller's level; 94 to 96
/// the years a step towards what his age wants (`FUN_00708acd`); 97 a
/// regular place, 98 no drawing at even odds, 99 his option. A raise
/// that rounds to nothing adds one to three.
#[allow(clippy::too_many_arguments)]
fn raise_bid(
    records: &Records,
    random: &mut crate::random::Random,
    tables: &crate::contract::ContractTables,
    p: &crate::records::Player,
    team: &crate::records::Team,
    seller_level: u8,
    signing_too: bool,
    t: &mut [u8; TERMS],
) {
    let word = |t: &[u8; TERMS], at: usize| u16::from_le_bytes([t[at], t[at + 1]]);
    let put = |t: &mut [u8; TERMS], at: usize, v: u16| t[at..at + 2].copy_from_slice(&v.to_le_bytes());
    let old = *t;
    let r = random.below(100);
    let age = crate::free_agents::age(records, p);
    let wanted_years: i32 = match age {
        ..0x18 => 1,
        0x18..0x1f => 3,
        _ => 4,
    };
    let league_bits = |t: &mut [u8; TERMS], bits: u16| {
        let f = word(t, 14);
        put(t, 14, f & 0xfffc | bits);
    };
    if !records.globals.played(i16::from(team.country())) {
        league_bits(t, 1);
    } else {
        let mut strength = u16::from(p.strength());
        if age < 0x17 && p.strength() < 0xd {
            strength += 1;
        }
        let level = u16::from(team.level()).min(2);
        if records.luck.of(p.id()) < tables.league_odds(strength, level) {
            league_bits(t, level + 1);
        }
    }
    let raised = |v: u32| (f64::from(v) * RAISE) as i64;
    if r < 55 {
        if random.below(2) == 0 || !signing_too {
            let salary = raised(u32::from(word(t, 0))) as u16;
            put(t, 0, salary);
            if salary == word(&old, 0) {
                put(t, 0, salary.wrapping_add(1).wrapping_add(random.below(3) as u16));
            }
        } else {
            let mut signing = u32::from_le_bytes([t[8], t[9], t[10], t[11]]);
            signing = signing.max(u32::from(word(t, 0)));
            signing = (raised(signing) as u32) & 0xffff;
            if signing == u32::from_le_bytes([old[8], old[9], old[10], old[11]]) {
                signing += random.below(3) + 1;
            }
            t[8..12].copy_from_slice(&signing.to_le_bytes());
        }
    } else if r < 90 {
        let mut bonus = raised(u32::from(word(t, 2))) as u16;
        if bonus == word(&old, 2) {
            bonus = bonus.wrapping_add(1).wrapping_add(random.below(3) as u16);
        }
        put(t, 2, bonus.min(2000));
    } else if r < 93 {
        if matches!(p.positions()[0], 9 | 10) {
            let step = match seller_level {
                0 => 50,
                1 => 20,
                _ => 5,
            };
            put(t, 4, word(t, 4).wrapping_add(step));
        }
    } else if r == 93 {
    } else if r < 97 {
        let years = i32::from(t[12] as i8);
        if wanted_years < years {
            t[12] = (years - 1) as u8;
        } else if years < wanted_years {
            t[12] = (years + 1) as u8;
        }
    } else if r == 97 {
        put(t, 14, word(t, 14) | crate::contract::REGULAR_PLACE);
    } else if r == 98 {
        put(t, 14, word(t, 14) & !ASKING_DRAWN);
    } else {
        put(t, 14, word(t, 14) | crate::contract::PLAYER_OPTION);
    }
}

/// `FUN_00692997`'s matchdays of a club whose country is not played.
const NOT_PLAYED_MATCHDAYS: i32 = 0x12;
/// What a computer club's raise multiplies by (0x80e740).
const RAISE: f64 = 1.05;

#[cfg(test)]
mod tests {
    use super::*;

    /// The day's purchase talks played as the cycle plays them, a human
    /// seller holding to his asking at its step each round ("choose 4095
    /// 0"); each talk as its fee talk ended.
    fn purchase_talk_day(
        records: &mut Records,
        random: &mut crate::random::Random,
        rules: &crate::season::Rules,
        day: i16,
        told: &mut Vec<Dropout>,
    ) -> Vec<PurchaseTalk> {
        let mut talks = Vec::new();
        let tables = (&rules.contract, &rules.value);
        while let Some((player, talk)) = next_purchase_talk(records, random, tables, &rules.medical, &[], day, told) {
            if let Some(mut talk) = talk {
                if let Some(fee) = talk.fee.as_mut() {
                    loop {
                        fee.ask(crate::fee_talk::amount(crate::fee_talk::step(fee.demand)), fee.pressing());
                        if fee.after_seller().or_else(|| fee.bidder_turn(random)).is_some() {
                            break;
                        }
                    }
                }
                talks.push(talk);
            }
            close_purchase_talk(records, player);
        }
        talks
    }

    /// Nsoki's fee talk on A, day 64: Union's human manager sells, 59/3
    /// bids; the bounds the original's talk held at its first dialog 308
    /// (a man listed for a fee: nine months' worth, 2 070 000), the
    /// asking at its step (10 000) each round, and the bidder's offer
    /// rising from 80 less a draw in 21 and its manager's skill hundredths
    /// of its most till its patience runs out — no fee agreed.
    #[test]
    #[ignore = "reads the kept saves next to $ANSTOSS3_DIR"]
    fn the_day_64_fee_talk_runs_out() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let save = dir.parent().unwrap().join("saves-kept").join("A.an3");
        let mut records = Records::from_save(Save::read(&save).unwrap()).unwrap();
        let rules = crate::season::Rules::load(&a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap()).unwrap();
        let mut random = crate::random::Random::from_state(12345, 67890);
        let talks = purchase_talk_day(&mut records, &mut random, &rules, 64, &mut Vec::new());
        let fee = talks.iter().find(|t| t.player == 3262).and_then(|t| t.fee.clone()).unwrap();
        assert_eq!((fee.seller, fee.buyer, fee.value), ((14, 84), (59, 3), 2_070_000));
        assert_eq!((fee.asking, fee.least, fee.most), (10_350, 2_070, 2_459));
        assert_eq!(fee.demand, 10_000);
        assert!(fee.out[1] && !fee.out[0] && fee.offer <= fee.most);
    }

    /// The two purchase talks save A books for day 64 open as
    /// `FUN_00768638`'s first block has them: Nsoki (3262) and Young
    /// (2168), each partner's stance written back to R+0x27f and the
    /// draws taken only where the rule draws.
    #[test]
    #[ignore = "reads the kept saves next to $ANSTOSS3_DIR"]
    fn the_day_64_purchase_talks_open_by_the_partners_stance() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let save = dir.parent().unwrap().join("saves-kept").join("A.an3");
        let mut records = Records::from_save(Save::read(&save).unwrap()).unwrap();
        let world = crate::world::World::load(&dir.join("Data.a3")).unwrap();
        let talks: Vec<_> =
            appointments(&records).into_iter().filter(|t| t.day == 64 && t.kind == PURCHASE).collect();
        assert_eq!(talks.iter().map(|t| t.player).collect::<Vec<_>>(), [3262, 2168]);
        // Nsoki's wife is at ease with it (R+0x27e 0x83) and came along at
        // the even draw; Young, no strong man, has his partner's old 2
        // against it and, at 25 or more, the one-in-a-hundred draw.
        let expected = [
            (Opening { stance: 3, flags: 2, remark: 0x5c7, share: 100, released: true }, 1, 2),
            (Opening { stance: 2, flags: 0, remark: 0, share: 100, released: true }, 2, 100),
        ];
        for (t, (opening, bits, draw)) in talks.iter().zip(expected) {
            let mut random = crate::random::Random::from_state(12345, 67890);
            let o = purchase_opening(&mut records, &mut random, t, &world.nation_kinds).unwrap();
            assert_eq!(o, opening);
            assert_eq!(stance_bits(records.player(t.player).unwrap()), bits);
            let mut replay = crate::random::Random::from_state(12345, 67890);
            replay.below(draw);
            assert_eq!(random, replay);
        }
    }

    /// The selling clubs' frames of save A's day-64 purchase talks: each
    /// man's salary between 0.8 of his demand and 1.65 or 1.75 of it, the
    /// club's opening terms spread from that least, the draws in
    /// `FUN_0076d44d`'s order; Union (Nsoki's) has its human manager.
    #[test]
    #[ignore = "reads the kept saves next to $ANSTOSS3_DIR"]
    fn the_day_64_sellers_ask_from_the_players_demand() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let save = dir.parent().unwrap().join("saves-kept").join("A.an3");
        let records = Records::from_save(Save::read(&save).unwrap()).unwrap();
        let rules = crate::season::Rules::load(&a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap()).unwrap();
        let tables = (&rules.contract, &rules.value);
        let talks: Vec<_> =
            appointments(&records).into_iter().filter(|t| t.day == 64 && t.kind == PURCHASE).collect();
        let mut managers = Vec::new();
        for t in &talks {
            let mut random = crate::random::Random::from_state(12345, 67890);
            let a = club_asking(&records, &mut random, tables, t, 0, 0, 64).unwrap();
            let p = records.player(t.player).unwrap();
            let age = crate::free_agents::age(&records, p);
            let star = records.globals.stars().get(usize::from(p.positions()[0])) == Some(&t.player);
            let form = crate::statistics::form_today(p);
            let signee = crate::contract::Signee::of(p, age, form, tables.1, star, records.globals.start_year());
            let demand = tables.0.demand(&signee);
            assert!(demand > 0);
            assert_eq!(a.least, demand * 4 / 5 / 1000);
            assert!(a.most > a.least);
            let base = (f64::from(demand) * 0.8 / 1000.0) as u32;
            let mut replay = crate::random::Random::from_state(12345, 67890);
            let spread = replay.below(21) + 90;
            assert_eq!(u32::from(a.salary()), spread * base / 100);
            replay.below(21);
            assert_eq!(a.years(), crate::free_agents::fresh_years(p, age, &mut replay));
            assert_eq!(a.flags() & ASKING_DRAWN != 0, replay.below(2) != 0);
            assert_eq!(random, replay);
            managers.push(a.manager);
        }
        assert!(managers[0] != 0 && managers[1] == 0, "{managers:?}");
    }

    /// Save A's day-64 purchase talks with their bidders' frames: one for
    /// each bidder left, none asking more at the least than the seller
    /// (the factor only lowers it), each most 1.75 or 1.65 of the demand,
    /// each with the opening's remark flags among its own.
    #[test]
    #[ignore = "reads the kept saves next to $ANSTOSS3_DIR"]
    fn the_day_64_bidders_get_their_own_frames() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let save = dir.parent().unwrap().join("saves-kept").join("A.an3");
        let mut records = Records::from_save(Save::read(&save).unwrap()).unwrap();
        let rules = crate::season::Rules::load(&a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap()).unwrap();
        let booked: Vec<_> =
            appointments(&records).into_iter().filter(|t| t.day == 64 && t.kind == PURCHASE).collect();
        let mut random = crate::random::Random::from_state(7, 9);
        let talks = purchase_talk_day(&mut records, &mut random, &rules, 64, &mut Vec::new());
        assert!(talks.iter().any(|t| !t.offers.is_empty()));
        let mut lowered = 0;
        for talk in &talks {
            assert_eq!(talk.offers.len(), talk.bidders.len());
            let t = booked.iter().find(|t| t.player == talk.player).unwrap();
            for (offer, club) in talk.offers.iter().zip(&talk.bidders) {
                assert!(offer.least <= talk.asking.least, "{offer:?} {:?}", talk.asking);
                assert!(offer.most > offer.least);
                assert_eq!(offer.remarks & talk.opening.flags, talk.opening.flags);
                if offer.least < talk.asking.least {
                    lowered += 1;
                }
                assert!(t.clubs.contains(club));
            }
        }
        assert!(lowered > 0);
    }

    /// Save A's day-64 talks bargained: Nsoki's, with Union's human
    /// manager, goes to its one bidder 59/3, which raises its terms until
    /// he takes them — worth more than the share to him, its salary
    /// within what it would pay and above what it opened on; a club of a
    /// country not played counts 18 matchdays for his bonuses. Young's,
    /// computer clubs only, is not bargained: its bidder 14/20 keeps the
    /// remark of a man it has no place for — four years and a regular
    /// place — and takes him at once, on fresh terms.
    #[test]
    #[ignore = "reads the kept saves next to $ANSTOSS3_DIR"]
    fn the_day_64_bidders_bargain_the_contract() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let save = dir.parent().unwrap().join("saves-kept").join("A.an3");
        let mut records = Records::from_save(Save::read(&save).unwrap()).unwrap();
        let rules = crate::season::Rules::load(&a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap()).unwrap();
        let tables = (&rules.contract, &rules.value);
        let before = Records::from_save(Save::read(&save).unwrap()).unwrap();
        let mut random = crate::random::Random::from_state(12345, 67890);
        let talks = purchase_talk_day(&mut records, &mut random, &rules, 64, &mut Vec::new());
        let nsoki = talks.iter().find(|t| t.player == 3262).unwrap();
        assert_eq!(nsoki.bidders, [(59, 3)]);
        assert_eq!(nsoki.winner, Some(1));
        let offer = &nsoki.offers[0];
        assert!(nsoki.best > i32::from(nsoki.opening.share) && !offer.out);
        assert!(i32::from(offer.salary()) <= offer.most && i32::from(offer.salary()) > offer.least);
        let p = before.player(3262).unwrap();
        let team = before.team(59, 3).unwrap();
        assert!(!before.globals.played(59));
        assert_eq!(offer_worth(&before, tables, p, team, &offer.terms).unwrap() + i32::from(offer.mood), nsoki.best);
        let young = talks.iter().find(|t| t.player == 2168).unwrap();
        assert_eq!(young.winner, None);
        let settled = young.settled.unwrap();
        assert_eq!((settled.buyer, settled.at_once), ((14, 20), true));
        assert!(settled.fee > 0 && settled.terms[12] >= 2);
        let moved = records.player(2168).unwrap();
        assert_eq!(moved.club(), [14, 20]);
        assert_eq!(moved.get::<u32>(0x158), settled.fee / 1000);
        assert_eq!(moved.salary(), u16::from_le_bytes([settled.terms[0], settled.terms[1]]));
        assert!(records.team(14, 20).unwrap().players().contains(&2168));
        assert!(!records.team(14, 43).unwrap().players().contains(&2168));
        assert_eq!(nsoki.settled, None);
        let offer = &young.offers[0];
        assert_eq!((offer.remark, offer.years()), (0x5d6, 4));
        assert_ne!(offer.flags() & crate::contract::REGULAR_PLACE, 0);
    }

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
            let mut told = Vec::new();
            let talks = loan_day(&mut records, day, &mut told);
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
        let rules = crate::season::Rules::load(&a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap()).unwrap();
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
            let mut told = Vec::new();
            let mut random = crate::random::Random::from_state(1, 1);
            let talks = purchase_talk_day(&mut records, &mut random, &rules, day, &mut told);
            let after = appointments(&records);
            assert_eq!(after.len(), before.len() - today);
            assert!(!after.iter().any(|t| t.day == day && t.kind == PURCHASE));
            assert!(listings(&records).iter().any(|x| x.id == l.id));
            let told: Vec<Dropout> = told.into_iter().filter(|d| d.player == l.id).collect();
            (talks.into_iter().find(|t| t.player == l.id), told)
        };
        let (talk, told) = run(0);
        assert_eq!(talk.expect("no purchase talk").bidders, vec![human]);
        assert!(told.is_empty());
        // The human club that cannot pay is told so (`FUN_00768497`).
        let (talk, told) = run(u16::MAX);
        assert_eq!(talk, None);
        assert_eq!(told, vec![Dropout { player: told[0].player, club: human, squad_full: false }]);
    }

}
