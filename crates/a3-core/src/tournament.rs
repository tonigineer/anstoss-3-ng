//! Tournaments (JAHRESKALENDER.md §4.3 in the RE repo): the list of the
//! season's tournaments (`CDatenTurnierliste`, `0x1c4be78`) — a Turnier of
//! four clubs, a Hallenturnier or a Strandturnier of eight — which a human
//! club joins or hosts from its year calendar.
//!
//! The computer's clubs host Turniere in the pre-season and Hallenturniere
//! in the winter break; a human hosts his own by inviting the clubs the
//! turn offers him for the day, the friendly's ([`crate::friendly`]).

use crate::records::{PLANNER_ENTRY, Planner, Records};

/// The kinds of tournament; the calendar's kind is two more.
pub const TURNIER: u8 = 0;
pub const HALLE: u8 = 1;
pub const STRAND: u8 = 2;
const CALENDAR_KIND: u8 = 2;
/// The clubs each kind needs (`DAT_008abe90`).
const NEEDED: [usize; 3] = [4, 8, 8];
/// At most this many of a day and kind are offered to join
/// (`FUN_0076090e`).
const OFFERED: usize = 10;

/// The list: its count, then two hundred entries of 0x74 bytes.
const LIST_AT: &str = "0x1c4be78";
const ENTRIES: usize = 4;
const ENTRY: usize = 0x74;
const MOST: usize = 200;
/// An entry's fields: the kind, its id, the clubs in it, its first day,
/// the clubs' slots of four bytes (country, a byte, number), the host's
/// first.
const KIND: usize = 0;
const ID: usize = 4;
const COUNT: usize = 8;
const DAY: usize = 0xa;
const SLOTS: usize = 0xc;
const SLOT: usize = 4;

/// A tournament as the list keeps it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Tournament([u8; ENTRY]);

impl Tournament {
    fn word(&self, at: usize) -> i16 {
        i16::from_le_bytes([self.0[at], self.0[at + 1]])
    }

    fn set_word(&mut self, at: usize, value: i16) {
        self.0[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }

    pub fn kind(&self) -> u8 {
        self.0[KIND]
    }

    pub fn id(&self) -> u32 {
        u32::from_le_bytes(self.0[ID..ID + 4].try_into().unwrap_or_default())
    }

    /// Its first day; it takes the next as well.
    pub fn day(&self) -> i16 {
        self.word(DAY)
    }

    pub fn count(&self) -> usize {
        usize::try_from(self.word(COUNT)).unwrap_or(0)
    }

    /// The clubs it may take (`FUN_0076006a`): four for a Turnier, eight
    /// for the others.
    fn room(&self) -> usize {
        if self.kind() == TURNIER { NEEDED[0] } else { NEEDED[1] }
    }

    /// The clubs in it, the host first.
    pub fn clubs(&self) -> Vec<(u8, i16)> {
        (0..self.count().min(8)).map(|k| self.club(k)).collect()
    }

    fn club(&self, k: usize) -> (u8, i16) {
        let at = SLOTS + k * SLOT;
        (self.0[at], self.word(at + 2))
    }

    pub fn host(&self) -> (u8, i16) {
        self.club(0)
    }

    /// The host's slot as it stands, its second byte with it: what the
    /// calendar's days of it carry.
    fn host_bytes(&self) -> [u8; 4] {
        self.0[SLOTS..SLOTS + SLOT].try_into().unwrap_or_default()
    }

    /// Whether the club is in it (`FUN_0075ffdd`).
    pub fn has(&self, club: (u8, i16)) -> bool {
        self.clubs().contains(&club)
    }

    /// Takes a club into the next slot while there is room
    /// (`FUN_0076006a`); it does not look whether the club is in already.
    fn add(&mut self, (country, number): (u8, i16)) -> bool {
        let count = self.count();
        if count >= self.room() {
            return false;
        }
        let at = SLOTS + count * SLOT;
        self.0[at] = country;
        self.set_word(at + 2, number);
        self.set_word(COUNT, count as i16 + 1);
        true
    }

    /// Whether it has the clubs its kind needs.
    pub fn full(&self) -> bool {
        self.count() >= self.room()
    }
}

/// The season's tournaments.
pub fn tournaments(records: &Records) -> Vec<Tournament> {
    let Some(block) = records.block(LIST_AT) else { return Vec::new() };
    let count = usize::try_from(i16::from_le_bytes([block[0], block[1]])).unwrap_or(0).min(MOST);
    (0..count)
        .filter_map(|k| block.get(ENTRIES + k * ENTRY..ENTRIES + (k + 1) * ENTRY))
        .filter_map(|e| e.try_into().ok().map(Tournament))
        .collect()
}

/// Writes the list back (`FUN_00760759`): the entries past it cleared.
fn store(records: &mut Records, list: &[Tournament]) {
    let Some(block) = records.block_mut(LIST_AT) else { return };
    let old = usize::try_from(i16::from_le_bytes([block[0], block[1]])).unwrap_or(0).min(MOST);
    for (k, t) in list.iter().enumerate().take(MOST) {
        block[ENTRIES + k * ENTRY..ENTRIES + (k + 1) * ENTRY].copy_from_slice(&t.0);
    }
    for k in list.len()..old {
        block[ENTRIES + k * ENTRY..ENTRIES + (k + 1) * ENTRY].fill(0);
    }
    block[0..2].copy_from_slice(&(list.len().min(MOST) as i16).to_le_bytes());
}

/// A tournament's id (`FUN_00760e8d`): the host's country and number in
/// the top bytes, its first day below.
pub fn id(host: (u8, i16), day: i16) -> u32 {
    crate::friendly::id(host, day)
}

/// The tournament with an id (`FUN_00761444`).
pub fn find(records: &Records, id: u32) -> Option<Tournament> {
    tournaments(records).into_iter().find(|t| t.id() == id)
}

/// The tournaments of a kind starting on a day, ten at most
/// (`FUN_0076090e`).
pub fn on_day(records: &Records, day: i16, kind: u8) -> Vec<Tournament> {
    tournaments(records).into_iter().filter(|t| t.day() == day && t.kind() == kind).take(OFFERED).collect()
}

/// A tournament of the club's own on a day (`FUN_00760d7a`): the club its
/// host and only club so far, in the list unless it is full. Its id.
pub fn open_own(records: &mut Records, club: (u8, i16), day: i16, kind: u8) -> Option<u32> {
    let mut list = tournaments(records);
    if list.len() >= MOST {
        return None;
    }
    let mut own = Tournament([0; ENTRY]);
    own.0[KIND] = kind;
    own.0[ID..ID + 4].copy_from_slice(&id(club, day).to_le_bytes());
    own.set_word(DAY, day);
    own.add(club);
    list.push(own);
    store(records, &list);
    Some(own.id())
}

/// Takes a tournament out of the list (`FUN_00760de0`).
pub fn remove(records: &mut Records, id: u32) {
    let mut list = tournaments(records);
    if let Some(k) = list.iter().position(|t| t.id() == id) {
        list.remove(k);
        store(records, &list);
    }
}

/// The calendar of a human club's manager, by his number from 1
/// (`FUN_0067764c`); none for a computer club.
fn calendar_of(records: &Records, club: (u8, i16)) -> Option<usize> {
    let manager = crate::friendly::manager_of(records, club);
    (1..=4).contains(&manager).then(|| usize::from(manager) - 1).filter(|&m| m < records.calendars.len())
}

/// Whether a calendar's two days of the tournament are free.
fn free(calendar: &Planner, day: i16) -> bool {
    let d = usize::try_from(day).unwrap_or(usize::MAX);
    calendar.entry(d)[0] == 0 && calendar.entry(d + 1)[0] == 0
}

/// The tournament's two days in a calendar (`FUN_0072ede4` with kinds 2 to
/// 4): its kind, the first marked 1 and the second 2, the host's slot.
pub fn mark(calendar: &mut Planner, t: &Tournament) {
    let d = usize::try_from(t.day()).unwrap_or(usize::MAX);
    for (k, mark) in [(d, 1), (d + 1, 2)] {
        let mut entry = [0; PLANNER_ENTRY];
        entry[0] = t.kind() + CALENDAR_KIND;
        entry[1] = mark;
        entry[4..8].copy_from_slice(&t.host_bytes());
        calendar.set_entry(k, entry);
    }
}

/// A club joins a tournament (`FUN_00760f5d`): a human club only with both
/// days free in his calendar, which then shows it; any club while there is
/// room.
pub fn join(records: &mut Records, club: (u8, i16), id: u32) -> bool {
    let mut list = tournaments(records);
    let Some(t) = list.iter_mut().find(|t| t.id() == id) else { return false };
    let calendar = calendar_of(records, club);
    if calendar.is_some_and(|m| !free(&records.calendars[m], t.day())) || !t.add(club) {
        return false;
    }
    let t = *t;
    if let Some(m) = calendar {
        mark(&mut records.calendars[m], &t);
    }
    store(records, &list);
    true
}

/// Why a tournament of one's own is not to be: a guest's calendar taken
/// on its days, no room left, or not the clubs its kind needs
/// (`FUN_0076029f`'s 2, 3 and 4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Short {
    Taken,
    NoRoom,
    TooFew,
}

/// The guests invited to a tournament of one's own (`FUN_0076029f`): each
/// human guest needs its days free; then the tournament must have the
/// clubs its kind needs, four or eight. So it goes into the list, and the
/// human guests' calendars show it.
pub fn invite(records: &mut Records, id: u32, guests: &[(u8, i16)]) -> Result<(), Short> {
    let Some(mut t) = find(records, id) else { return Err(Short::TooFew) };
    for &guest in guests {
        if calendar_of(records, guest).is_some_and(|m| !free(&records.calendars[m], t.day())) {
            return Err(Short::Taken);
        }
        if !t.add(guest) {
            return Err(Short::NoRoom);
        }
    }
    if t.count() != NEEDED[usize::from(t.kind().min(STRAND))] {
        return Err(Short::TooFew);
    }
    for &guest in guests {
        if let Some(m) = calendar_of(records, guest) {
            mark(&mut records.calendars[m], &t);
        }
    }
    let mut list = tournaments(records);
    match list.iter().position(|e| e.id() == id) {
        Some(k) => list[k] = t,
        None if list.len() < MOST => list.push(t),
        None => {}
    }
    store(records, &list);
    Ok(())
}

/// How many clubs a kind needs.
pub fn needed(kind: u8) -> usize {
    NEEDED[usize::from(kind.min(STRAND))]
}

/// What a guest costs its host (`FUN_006263ee`): 100 000 for a club from
/// abroad or a champion, else 50 000 from the first division, 20 000 from
/// the second, 10 000 from below.
pub fn fee(records: &Records, host: (u8, i16), guest: (u8, i16)) -> i32 {
    const CHAMPION: u16 = 0x1;
    let team = u16::try_from(guest.1).ok().and_then(|n| records.team(guest.0.into(), n));
    if guest.0 != host.0 || team.is_some_and(|t| t.get::<u16>(0x110) & CHAMPION != 0) {
        return 100_000;
    }
    match team.map_or(10, |t| t.level()) {
        0 => 50_000,
        1 => 20_000,
        _ => 10_000,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: u8, host: (u8, i16), day: i16) -> Tournament {
        let mut t = Tournament([0; ENTRY]);
        t.0[KIND] = kind;
        t.0[ID..ID + 4].copy_from_slice(&id(host, day).to_le_bytes());
        t.set_word(DAY, day);
        assert!(t.add(host));
        t
    }

    /// A Turnier takes four clubs, a hall's and a beach's eight; the host
    /// stays first, the others in the order they came, and one more than
    /// the room is refused (`FUN_0076006a`).
    #[test]
    fn a_tournament_takes_its_kinds_clubs_and_no_more() {
        for (kind, room) in [(TURNIER, 4), (HALLE, 8), (STRAND, 8)] {
            let mut t = entry(kind, (14, 82), 37);
            for k in 1..room {
                assert!(!t.full());
                assert!(t.add((14, k as i16)));
            }
            assert!(t.full());
            assert!(!t.add((14, 99)));
            assert_eq!(t.count(), room);
            assert_eq!(t.host(), (14, 82));
            assert_eq!(t.clubs()[1..], (1..room).map(|k| (14, k as i16)).collect::<Vec<_>>()[..]);
            assert!(t.has((14, 1)) && !t.has((14, 99)));
            assert_eq!(needed(kind), room);
        }
    }

    /// The id holds the host and the day (`FUN_00760e8d`): Ulm's of the 37th
    /// is 0e520025, as the original wrote it.
    #[test]
    fn the_id_holds_the_host_and_the_day() {
        assert_eq!(id((14, 82), 37), 0x0e52_0025);
        let t = entry(TURNIER, (14, 82), 37);
        assert_eq!((t.id(), t.day(), t.kind()), (0x0e52_0025, 37, TURNIER));
    }

    /// The calendar shows both days, the first marked 1 and the second 2,
    /// with the host's slot — its second byte as the slot has it.
    #[test]
    fn the_calendar_shows_both_days_with_the_host() {
        let mut calendar = Planner::new(&vec![0; Planner::SIZE]).unwrap();
        let mut t = entry(HALLE, (14, 56), 172);
        t.0[SLOTS + 1] = 0xc7;
        mark(&mut calendar, &t);
        let (first, second) = (calendar.entry(172), calendar.entry(173));
        assert_eq!(first[..8], [3, 1, 0, 0, 14, 0xc7, 56, 0]);
        assert_eq!(second[..8], [3, 2, 0, 0, 14, 0xc7, 56, 0]);
        assert!(first[8..].iter().chain(&second[8..]).all(|&b| b == 0));
        assert_eq!(calendar.entry(174)[0], 0);
        assert!(!free(&calendar, 172) && !free(&calendar, 171) && free(&calendar, 174));
    }
}
