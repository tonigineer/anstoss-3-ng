//! The newspaper's headlines (`CZeitungsmeldungen` at `DAT_01c6ac60`): a
//! queue of three, ordered by priority, that the human clubs' day
//! (`FUN_0078a0f5`, called by `FUN_0064f981`) empties into the list of
//! twenty the save keeps (`FUN_0072457f`, read back by
//! [`crate::statistics::newspaper`]). No save keeps the queue.
//!
//! Not ported: the newspaper screen the day then shows
//! (`FUN_006505fb(0x100, 1, 1)`) and the value slots a headline carries
//! (the manager's, the club's and the player's changes at +0x24..).

use crate::records::Records;

/// The queue's slots (`FUN_00789895`, stride 0x474).
const SLOTS: usize = 3;
/// The saved list: a count, then twenty entries of 0xc4 — day, month,
/// year, two lines of 0x60 (`DAT_00b0a0a8`).
const LIST: &str = "0xb0a088";
const ENTRY: usize = 0xc4;
const LINE: usize = 0x60;
const MOST: usize = 20;
/// The first season's first days without the paper when the option at
/// `DAT_00b5d19a` is set (`FUN_005ef56c`).
const QUIET_DAYS: u16 = 0x2a;
const QUIET_OPTION: usize = 0xb5d19a;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Item {
    pub priority: i16,
    pub headline: String,
    pub body: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Queue {
    slots: Vec<Item>,
    /// Items put in since the last day (+0x20), dropped ones not counted.
    count: i16,
}

impl Queue {
    /// `FUN_00789895`: after every slot of at least its priority; full of
    /// those, it is dropped, else the last falls out.
    pub fn push(&mut self, item: Item) {
        let at = self
            .slots
            .iter()
            .take_while(|s| item.priority <= s.priority && s.priority != 0)
            .count();
        if at < SLOTS {
            self.slots.insert(at, item);
            self.slots.truncate(SLOTS);
            self.count += 1;
        }
    }

    pub fn items(&self) -> &[Item] {
        &self.slots
    }

    /// The day's turn (`FUN_0078a0f5`): the first item into the paper's
    /// list under today's date, then the queue cleared (`FUN_0078985a`).
    pub fn publish(&mut self, records: &mut Records) {
        let g = &records.globals;
        let quiet = g.day() < QUIET_DAYS && g.seasons() == 1 && g.get::<i16>(QUIET_OPTION) != 0;
        if quiet || self.count == 0 {
            return;
        }
        let [day, month, year] = records.globals.date();
        if let (Some(first), Some(b)) = (self.slots.first(), records.block_mut(LIST)) {
            if b.len() >= 2 + MOST * ENTRY {
                b.copy_within(2..2 + (MOST - 1) * ENTRY, 2 + ENTRY);
                let e = &mut b[2..2 + ENTRY];
                e.fill(0);
                e[0] = day as u8;
                e[1] = month as u8;
                e[2..4].copy_from_slice(&year.to_le_bytes());
                for (k, line) in [&first.headline, &first.body].into_iter().enumerate() {
                    let bytes = to_cp1252(line);
                    let n = bytes.len().min(LINE - 1);
                    e[4 + k * LINE..4 + k * LINE + n].copy_from_slice(&bytes[..n]);
                }
                let count = i16::from_le_bytes([b[0], b[1]]);
                if count < MOST as i16 {
                    b[0..2].copy_from_slice(&(count + 1).to_le_bytes());
                }
            }
        }
        self.slots.clear();
        self.count = 0;
    }
}

/// Text `id` of `ereignis.saq` as `FUN_00789c32` makes it: `%1`.. replaced
/// by `args` after the text is split at its first '-' into headline and body.
pub fn format(text: &str, args: &[&str]) -> (String, String) {
    let fill = |part: &str| {
        let mut out = part.to_owned();
        for (k, a) in args.iter().enumerate().rev() {
            out = out.replace(&format!("%{}", k + 1), a);
        }
        out
    };
    let (head, body) = text.split_once('-').unwrap_or((text, ""));
    (fill(head), fill(body))
}

fn to_cp1252(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| match u8::try_from(u32::from(c)) {
            Ok(b) => b,
            Err(_) => (0x80u8..0xa0)
                .find(|&b| a3_files::sav::cp1252(&[b]).starts_with(c))
                .unwrap_or(b'?'),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_queue_of_higher_priority_drops_the_newcomer() {
        let item = |p: i16, h: &str| Item { priority: p, headline: h.into(), body: String::new() };
        let mut q = Queue::default();
        q.push(item(5, "a"));
        q.push(item(7, "b"));
        q.push(item(5, "c"));
        q.push(item(3, "d"));
        let order: Vec<&str> = q.items().iter().map(|i| i.headline.as_str()).collect();
        assert_eq!(order, ["b", "a", "c"]);
    }

    #[test]
    fn the_text_splits_at_its_first_dash() {
        let (h, b) = format("%1 sauer auf %2 - %3 lehnt ab", &["FC", "Merk", "Ede"]);
        assert_eq!((h.as_str(), b.as_str()), ("FC sauer auf Merk ", " Ede lehnt ab"));
    }
}
