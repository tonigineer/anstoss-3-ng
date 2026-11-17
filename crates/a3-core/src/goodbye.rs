//! A manager's goodbye from his club (`FUN_006da0fe`, behind
//! `FUN_006da1ac` at a career's end, reason 4): first the club notes the
//! departure (`FUN_005dfec7`) in the list of departures waiting to be
//! shown (`DAT_00a2ea68`, ten slots of 0x18 bytes). The day's loop
//! (`FUN_006da3a9`) takes them out (`FUN_005dfe23`): one that is to be
//! shown and whose day has come goes to its manager as message 0x108,
//! the dismissal dialog `CEntlassung` (`FUN_00494339`), and every slot is
//! cleared either way. No save keeps the list.

use crate::records::Records;

/// The slots of the list (`FUN_005dfdd5` looks at ten).
pub const SLOTS: usize = 10;

/// The career record's club stations: their count and their rows, each
/// with the championships won there at +0x18 (`FUN_006fe2e3` reading
/// +0x1dc of the last).
const CLUB_STATIONS: usize = 0x1c2;
const CLUBS_AT: usize = 0x1c4;
const STATION: usize = 0x1c;
const WON_CHAMPIONSHIPS: usize = 0x18;

/// One departure (`FUN_005dfd08`'s slot; word 0, the slot's use, is 1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Notice {
    /// The manager's number (manager +0x1a, word 1).
    pub manager: i16,
    /// Word 2: 3 at the career's end.
    pub kind: i16,
    /// Why he goes (word 3): 4 at the career's end.
    pub reason: i16,
    /// The club he leaves (club +0x104 and +0x103, words 4 and 5).
    pub country: u8,
    pub number: u16,
    /// His championships at that club: the mark of his last club station
    /// in the career record (`FUN_006fe2e3(manager, -1)`, word 6).
    pub championships: u8,
    /// The days since he signed with it (`FUN_006fe28c`, word 7).
    pub days: i16,
    /// The day it is noted on (`DAT_00b5cbd4`, word 8).
    pub day: i16,
    /// Whether and how it is shown (word 9): 0 keeps it from showing.
    pub shown: u16,
    /// The dword at word 10.
    pub extra: u32,
}

/// The list of departures (`DAT_00a2ea68`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Notices {
    slots: [Option<Notice>; SLOTS],
}

impl Notices {
    /// Into the first free slot (`FUN_005dfdd5`); with all ten taken the
    /// notice is dropped.
    pub fn post(&mut self, notice: Notice) -> bool {
        match self.slots.iter_mut().find(|s| s.is_none()) {
            Some(slot) => {
                *slot = Some(notice);
                true
            }
            None => false,
        }
    }

    /// The day's turn (`FUN_005dfe23`): the notices to be shown whose day
    /// has come, in slot order; the list is empty afterwards.
    pub fn due(&mut self, today: i16) -> Vec<Notice> {
        self.slots
            .iter_mut()
            .filter_map(Option::take)
            .filter(|n| n.shown != 0 && n.day <= today)
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(Option::is_none)
    }
}

/// The day number of a date (`FUN_00675795`, called as
/// `FUN_00675e49(day, month, year)`): March is the year's first month.
fn day_number(day: i32, month: i32, year: i32) -> i32 {
    let (month, year) = if month < 3 { (month + 9, year - 1) } else { (month - 3, year) };
    (year / 100) * 146_097 / 4 + (year % 100) * 1461 / 4 + 719_469 + day + (month * 153 + 2) / 5
}

/// The notice the club of `country` and `number` gives of the human in
/// `slot` leaving it (`FUN_005dfec7(manager, club, reason, kind, shown,
/// extra)`), stamped with today's day and date.
pub fn notice(
    records: &Records,
    slot: usize,
    (country, number): (u8, u16),
    reason: i16,
    kind: i16,
    shown: u16,
    extra: u32,
) -> Notice {
    let championships = records
        .career(slot)
        .and_then(|car| {
            let last = usize::from(*car.get(CLUB_STATIONS)?).checked_sub(1)?;
            car.get(CLUBS_AT + last * STATION + WON_CHAMPIONSHIPS).copied()
        })
        .unwrap_or(0);
    let [d, m, y] = records.globals.date().map(|w| i32::from(w as i16));
    let days = records.managers.get(slot).map_or(0, |mgr| {
        day_number(d, m, y)
            - day_number(
                i32::from(mgr.signed_day() as i8),
                i32::from(mgr.signed_month() as i8),
                i32::from(mgr.signed_year() as i16),
            )
    });
    Notice {
        manager: slot as i16 + 1,
        kind,
        reason,
        country,
        number,
        championships,
        days: days as i16,
        day: records.globals.day() as i16,
        shown,
        extra,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_numbers_count_days() {
        assert_eq!(day_number(1, 3, 2025) - day_number(28, 2, 2025), 1);
        assert_eq!(day_number(1, 1, 2026) - day_number(1, 1, 2025), 365);
        assert_eq!(day_number(1, 3, 2024) - day_number(28, 2, 2024), 2);
    }

    #[test]
    fn ten_slots_and_the_day_takes_them_all() {
        let mut list = Notices::default();
        let at = |day, shown| Notice { day, shown, ..Default::default() };
        for i in 0..SLOTS {
            assert!(list.post(at(i as i16, u16::from(i % 2 == 0))));
        }
        assert!(!list.post(at(0, 1)), "an eleventh is dropped");
        let shown = list.due(4);
        assert_eq!(shown.iter().map(|n| n.day).collect::<Vec<_>>(), vec![0, 2, 4]);
        assert!(list.is_empty(), "the later and the silent are cleared as well");
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_club_notes_his_departure() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let records = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let slot = records.globals.slots().iter().position(|&s| s > 0).unwrap();
        let mgr = &records.managers[slot];
        let club = (mgr.country() as u8, mgr.team());
        assert_ne!(club.0, 0, "the human has a club");
        let n = notice(&records, slot, club, 4, 3, 0x80, 0);
        assert_eq!((n.manager, n.kind, n.reason, n.shown), (slot as i16 + 1, 3, 4, 0x80));
        assert_eq!((n.country, n.number), club);
        assert_eq!(n.day, records.globals.day() as i16);
        assert!(n.days >= 0, "signed before today: {}", n.days);
        let car = records.career(slot).unwrap();
        let count = usize::from(car[CLUB_STATIONS]);
        let expected = if count == 0 { 0 } else { car[CLUBS_AT + (count - 1) * STATION + WON_CHAMPIONSHIPS] };
        assert_eq!(n.championships, expected);
    }
}
