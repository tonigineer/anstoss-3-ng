//! The dated list of the private lives: the `CPtrList` at `0x8eb8b0`
//! where a wedding day is booked (`FUN_0043779f`) and a child's birth is
//! booked ahead (`FUN_004377cf`), and which the day walk runs off every
//! day (`FUN_005f0428`, right before the headline).
//!
//! The list is saved as pages of `0x324` bytes — the blocks `0x8eb8b0[n]`:
//! a u16 of entries already read at +0, the u16 count of entries at +2,
//! and a hundred entries from +4, each the booked day as four bytes
//! (day, month, year) plus the man's id and the kind, two bytes each.
//! The day counts up as a number, so the entries sort by it.

use crate::records::Records;

/// The list's address, whose pages the save names `0x8eb8b0[n]`.
const PAGES_AT: u32 = 0x8eb8b0;
/// The block the layout puts before the list, where a first page goes.
const FOLLOWS: &str = "0x1b9ffc8";
/// A page: the two u16s of head and a hundred entries of eight bytes.
const PAGE: usize = 0x324;
/// Entries one page holds before it passes one on (`FUN_0043741e`).
const ENTRIES: usize = 100;

/// A booked wedding day, still untold (`FUN_0043779f`'s kind).
pub const WEDDING: u8 = 0;
/// A child's booked birth (`FUN_004377cf`'s kind).
pub const BIRTH: u8 = 1;

/// A day as the list counts it: the date's four bytes in one number,
/// which counts up with the days (`FUN_00676992`).
pub fn stamp(day: u8, month: u8, year: u16) -> u32 {
    u32::from(day) | u32::from(month) << 8 | u32::from(year) << 16
}

/// Today, stamped.
pub fn today(records: &Records) -> u32 {
    use chrono::Datelike;
    records
        .globals
        .date_of(i16::try_from(records.globals.day()).unwrap_or(0))
        .map_or(0, |d| stamp(d.day() as u8, d.month() as u8, d.year() as u16))
}

fn page_name(page: usize) -> String {
    format!("{PAGES_AT:#x}[{page}]")
}

/// How many pages the records carry.
fn pages(records: &Records) -> usize {
    (0..).find(|&n| records.block(&page_name(n)).is_none()).unwrap()
}

/// The entry at `at` of a page: the booked day, the man, the kind.
fn entry(page: &[u8], at: usize) -> (u32, u16, u16) {
    let e = &page[4 + at * 8..][..8];
    (
        u32::from_le_bytes(e[..4].try_into().unwrap()),
        u16::from_le_bytes(e[4..6].try_into().unwrap()),
        u16::from_le_bytes(e[6..8].try_into().unwrap()),
    )
}

fn put_entry(page: &mut [u8], at: usize, (date, id, kind): (u32, u16, u16)) {
    page[4 + at * 8..][..4].copy_from_slice(&date.to_le_bytes());
    page[4 + at * 8 + 4..][..2].copy_from_slice(&id.to_le_bytes());
    page[4 + at * 8 + 6..][..2].copy_from_slice(&kind.to_le_bytes());
}

fn count(page: &[u8]) -> usize {
    usize::from(u16::from_le_bytes([page[2], page[3]]))
}

fn set_count(page: &mut [u8], count: usize) {
    page[2..4].copy_from_slice(&(count as u16).to_le_bytes());
}

/// Book the day `when` of kind `kind` for the man `id`
/// (`FUN_004377cf` and `FUN_0043779f` through `FUN_0043772c`): the entry
/// takes the place before the first entry of a later day
/// (`FUN_00437850`), and a page already at its hundredth entry passes
/// its last one on to the next (`FUN_0043741e`) — at the chain's end one
/// more page is cut (`FUN_0043736a`).
pub fn book(records: &mut Records, id: u16, kind: u8, when: u32) {
    let mut carry = (when, id, u16::from(kind));
    let mut page = 0;
    loop {
        let passed_on = match records.block_mut(&page_name(page)) {
            Some(data) => {
                let n = count(data);
                // the first entry of a later day than the booked one
                let at = (0..n).find(|&i| entry(data, i).0 > carry.0).unwrap_or(n);
                if at == n && n == ENTRIES {
                    // full, and the entry belongs past all of it
                    (carry, true)
                } else {
                    // a full page sheds its last entry into the shift
                    let evicted = (n == ENTRIES).then(|| entry(data, n - 1));
                    let top = if evicted.is_some() { n - 1 } else { n };
                    for i in (at + 1..=top).rev() {
                        let e = entry(data, i - 1);
                        put_entry(data, i, e);
                    }
                    put_entry(data, at, carry);
                    set_count(data, top + 1);
                    match evicted {
                        Some(e) => (e, true),
                        None => return,
                    }
                }
            }
            None => {
                // the chain's end: one more page, the entry first in it
                records.add_page(PAGES_AT, FOLLOWS, PAGE);
                let data = records.block_mut(&page_name(page)).expect("the page just cut");
                put_entry(data, 0, carry);
                set_count(data, 1);
                return;
            }
        };
        carry = passed_on.0;
        page += 1;
    }
}

/// The men whose booked day of `kind` is `when` — read page by page, as
/// `FUN_005f0428`'s loop does over `FUN_004377e7`.
pub fn todays(records: &Records, when: u32, kind: u8) -> Vec<u16> {
    let mut ids = Vec::new();
    for page in 0..pages(records) {
        let Some(data) = records.block(&page_name(page)) else {
            break;
        };
        for i in 0..count(data) {
            let (date, id, k) = entry(data, i);
            if date == when && u8::try_from(k) == Ok(kind) {
                ids.push(id);
            }
        }
    }
    ids
}

/// Take `id`'s entry of `when` and `kind` out again
/// (`FUN_0043775c`'s `FUN_00437522`): what stands behind it moves up a
/// place.
pub fn unbook(records: &mut Records, id: u16, when: u32, kind: u8) {
    for page in 0..pages(records) {
        let Some(data) = records.block_mut(&page_name(page)) else {
            break;
        };
        let n = count(data);
        let Some(at) = (0..n).find(|&i| {
            let (date, who, k) = entry(data, i);
            date == when && who == id && k == u16::from(kind)
        }) else {
            continue;
        };
        for i in at..n - 1 {
            let e = entry(data, i + 1);
            put_entry(data, i, e);
        }
        set_count(data, n - 1);
        return;
    }
}

/// The day walk's first sweep (`FUN_0043778c` through `FUN_0043758b`):
/// a page whose newest entry stands before today is spent. The original
/// frees the page (`FUN_004374d3`); the replica empties it where it
/// lies, and the save keeps counting the page.
pub fn drop_past(records: &mut Records, when: u32) {
    for page in 0..pages(records) {
        let Some(data) = records.block_mut(&page_name(page)) else {
            break;
        };
        let n = count(data);
        if n > 0 && entry(data, n - 1).0 < when {
            let top = PAGE.min(data.len());
            data[..top].fill(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Booked days come round in the order of their date; the day walk
    /// empties a page spent in the past and takes a handled entry out,
    /// leaving its neighbours (`FUN_005f0428`).
    #[test]
    fn booked_days_come_round_in_order() {
        let mut r = Records::blank();
        book(&mut r, 7, BIRTH, stamp(3, 6, 2001));
        book(&mut r, 9, BIRTH, stamp(1, 6, 2001));
        book(&mut r, 11, WEDDING, stamp(1, 6, 2001));
        assert_eq!(todays(&r, stamp(1, 6, 2001), BIRTH), [9]);
        assert_eq!(todays(&r, stamp(1, 6, 2001), WEDDING), [11]);
        assert_eq!(todays(&r, stamp(3, 6, 2001), BIRTH), [7]);
        // the birth handled today: its entry goes, the wedding stays
        unbook(&mut r, 9, stamp(1, 6, 2001), BIRTH);
        assert_eq!(todays(&r, stamp(1, 6, 2001), BIRTH), []);
        assert_eq!(todays(&r, stamp(1, 6, 2001), WEDDING), [11]);
        // and a year on, the whole page is spent
        drop_past(&mut r, stamp(1, 1, 2002));
        assert_eq!(todays(&r, stamp(3, 6, 2001), BIRTH), []);
        assert_eq!(todays(&r, stamp(1, 6, 2001), WEDDING), []);
    }

    /// A hundred bookings fill one page, and the hundredth passes over
    /// into a second (`FUN_0043741e`).
    #[test]
    fn a_full_page_passes_its_last_booking_on() {
        let mut r = Records::blank();
        for i in 0..=100u32 {
            book(&mut r, 500, BIRTH, stamp(1, 1, 2000) + i);
        }
        assert_eq!(pages(&r), 2, "one more page was cut");
        assert_eq!(todays(&r, stamp(1, 1, 2000) + 100, BIRTH), [500]);
        assert_eq!(count(r.block(&page_name(0)).unwrap()), ENTRIES);
    }
}
