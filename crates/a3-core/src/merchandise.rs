//! A human club's own merchandise (dialog 56, `CFanartikelKaufen`): the
//! range of up to forty articles with their stock and orders, the fans'
//! discount, the buyer and the marketing man, and the buying itself.
//!
//! The range lives at `0xa32880 + (slot − 1) · 0xd54` (save block
//! `0xa32860[slot − 1]`, which holds that memory from +0); the dialog
//! copies it (`FUN_00614089`), works on the copy and writes it back on OK
//! (`FUN_0061412d`). The kinds of article are the program's table at
//! `0x890180`, 0x45 records of 0x1c bytes.

use std::io;

use a3_files::exe::Exe;

use crate::records::{Records, Team};

/// The save block of a human slot's range, by its slot from 0.
pub fn block_name(slot: usize) -> String {
    format!("{RANGE_AT:#x}[{slot}]")
}

const RANGE_AT: u32 = 0xa32860;
pub const ARTICLES: usize = 40;
const ARTICLE: usize = 0x50;
/// An article: in the range, the kind, the markup on the kind's price in
/// percent, the pieces in stock, those ordered by the weeks they still
/// take (1 to 4), the pieces sold and the money taken last week.
const ACTIVE: usize = 0;
const KIND: usize = 2;
const MARKUP: usize = 6;
const STOCK: usize = 0xc;
const ORDERED: usize = 0x10;
const SOLD: usize = 0x20;
/// Left out of buying them all; the nine weeks' sales before the last.
const EXCLUDED: usize = 1;
const PAST_SALES: usize = 0x28;
const PAST_WEEKS: usize = 9;
/// Where a range marks the kinds it has given up, a byte each.
const GIVEN_UP: usize = 0xce2;
const TAKEN: usize = 0x4c;
/// The buyer and the marketing man, hired; the fans' discount in percent,
/// below nought a surcharge; the goalkeeper's and the other name some
/// articles carry (`%1`, `%5`), each with its flag.
const BUYER: usize = 0xc84;
const MARKETING: usize = 0xc88;
const FANS_DISCOUNT: usize = 0xc8c;
const STOCK_VALUE_AT_START: usize = 0xc90;
const KEEPER_SET: usize = 0xc98;
const OTHER_SET: usize = 0xc9a;
const KEEPER_NAME: (usize, usize) = (0xc9c, 0x1e);
const OTHER_NAME: (usize, usize) = (0xcba, 0x1e);
/// No article holds more pieces, in stock and ordered (`FUN_0061157e`).
const MOST_PIECES: i32 = 999_999;

/// The kinds' records: the name's text, the picture, what a piece is
/// bought and sold at in cents.
const KINDS_AT: u32 = 0x890180;
pub const KINDS: usize = 0x45;
const KIND_SIZE: usize = 0x1c;

/// The order sizes on the dialog's spinner (`0x880bc4`), the one it
/// starts at, and the rebate each size earns (`FUN_006109d5`).
pub const ORDER_SIZES: [i32; 11] = [50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000, 50000, 100000];
pub const FIRST_ORDER_SIZE: usize = 3;

/// The yearly wages of the buyer and the marketing man
/// (`DAT_00a3b378` = 120 · 1000, `DAT_00a3b368` = 240 · 1000), of which a
/// newly hired one costs the season's months still to come.
pub const BUYER_WAGE: i32 = 120_000;
pub const MARKETING_WAGE: i32 = 240_000;

/// A kind of article; one of the first four, which every range keeps,
/// is fixed (+0x10 is 1).
#[derive(Clone, Copy, Debug, Default)]
pub struct Kind {
    pub name: u16,
    pub picture: u16,
    pub bought_at: i32,
    pub sold_at: i32,
    pub fixed: u16,
}

pub struct MerchandiseTables {
    pub kinds: Vec<Kind>,
}

impl MerchandiseTables {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        let words = exe.u32s(KINDS_AT, KINDS * KIND_SIZE / 4)?;
        let kinds = words
            .chunks(KIND_SIZE / 4)
            .map(|k| Kind {
                name: k[0] as u16,
                picture: k[1] as u16,
                bought_at: k[2] as i32,
                sold_at: k[3] as i32,
                fixed: k[4] as u16,
            })
            .collect();
        Ok(Self { kinds })
    }
}

/// The weeks each kind takes to come, which the save keeps beside the
/// program's table (0x45 u16 the loader copies to `0x890198 + i · 0x1c`).
pub fn delivery_weeks(records: &Records) -> Vec<i16> {
    records
        .block("local_a8")
        .map(|b| b.chunks_exact(2).map(|w| i16::from_le_bytes([w[0], w[1]])).collect())
        .unwrap_or_default()
}

/// A copy of a human club's range.
#[derive(Clone, Debug)]
pub struct Range(Vec<u8>);

/// An article of the range, by its place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Article {
    pub place: usize,
    pub kind: usize,
    pub markup: i16,
    pub stock: i32,
    /// Ordered, in all the weeks to come.
    pub ordered: i32,
    pub sold: i32,
    pub taken: i32,
    pub excluded: bool,
}

impl Range {
    /// The range of the club's human slot (`FUN_00614089`); none for a
    /// club without one.
    pub fn of(records: &Records, team: &Team) -> Option<Self> {
        let slot = usize::from(team.human()).checked_sub(1).filter(|&s| s < 4)?;
        records.block(&block_name(slot)).map(|b| Self(b.to_vec()))
    }

    /// Writes it back to the club's slot (`FUN_0061412d`).
    pub fn write(&self, records: &mut Records, slot: usize) {
        if let Some(block) = records.block_mut(&block_name(slot)) {
            let n = block.len().min(self.0.len());
            block[..n].copy_from_slice(&self.0[..n]);
        }
    }

    fn i32_(&self, at: usize) -> i32 {
        self.0.get(at..at + 4).map_or(0, |b| i32::from_le_bytes(b.try_into().unwrap()))
    }

    fn i16_(&self, at: usize) -> i16 {
        self.0.get(at..at + 2).map_or(0, |b| i16::from_le_bytes([b[0], b[1]]))
    }

    fn put_i32(&mut self, at: usize, v: i32) {
        if let Some(b) = self.0.get_mut(at..at + 4) {
            b.copy_from_slice(&v.to_le_bytes());
        }
    }

    fn text(&self, (at, len): (usize, usize)) -> String {
        let bytes = self.0.get(at..at + len).unwrap_or_default();
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        bytes[..end].iter().map(|&b| char::from(b)).collect()
    }

    pub fn article(&self, place: usize) -> Option<Article> {
        let at = place * ARTICLE;
        if place >= ARTICLES || self.0.get(at + ACTIVE).copied().unwrap_or(0) == 0 {
            return None;
        }
        let ordered = (0..4).fold(0i32, |s, w| s.wrapping_add(self.i32_(at + ORDERED + 4 * w)));
        Some(Article {
            place,
            kind: usize::try_from(self.i16_(at + KIND)).unwrap_or(0),
            markup: self.i16_(at + MARKUP),
            stock: self.i32_(at + STOCK),
            ordered,
            sold: self.i32_(at + SOLD),
            taken: self.i32_(at + TAKEN),
            excluded: self.0[at + EXCLUDED] != 0,
        })
    }

    /// The sales of the ten weeks before, the oldest first and last week's
    /// at the end, as the article's chart draws them.
    pub fn sales(&self, place: usize) -> [i32; 10] {
        let at = place * ARTICLE;
        let mut weeks = [0; 10];
        for (w, v) in weeks.iter_mut().take(PAST_WEEKS).enumerate() {
            *v = self.i32_(at + PAST_SALES + 4 * w);
        }
        weeks[PAST_WEEKS] = self.i32_(at + SOLD);
        weeks
    }

    /// The article's dialog's OK (`0x49b9c9`): its markup and whether it is
    /// left out of buying them all.
    pub fn set_article(&mut self, place: usize, markup: i16, excluded: bool) {
        let at = place * ARTICLE;
        if let Some(b) = self.0.get_mut(at + MARKUP..at + MARKUP + 2) {
            b.copy_from_slice(&markup.to_le_bytes());
        }
        if let Some(b) = self.0.get_mut(at + EXCLUDED) {
            *b = excluded.into();
        }
    }

    /// Gives an article of a kind up (`FUN_006112a9`): a fixed kind stays;
    /// otherwise the first article of the kind leaves the range, which
    /// marks the kind as given up. Whether one left.
    pub fn give_up(&mut self, tables: &MerchandiseTables, kind: usize) -> bool {
        if tables.kinds.get(kind).is_some_and(|k| k.fixed == 1) {
            return false;
        }
        let Some(a) = self.articles().into_iter().find(|a| a.kind == kind) else { return false };
        if let Some(b) = self.0.get_mut(GIVEN_UP + kind) {
            *b = 2;
        }
        self.0[a.place * ARTICLE + ACTIVE] = 0;
        true
    }

    /// The articles in the range, by their places.
    pub fn articles(&self) -> Vec<Article> {
        (0..ARTICLES).filter_map(|p| self.article(p)).collect()
    }

    pub fn buyer(&self) -> bool {
        self.i32_(BUYER) != 0
    }

    pub fn marketing(&self) -> bool {
        self.i32_(MARKETING) != 0
    }

    pub fn set_buyer(&mut self, on: bool) {
        self.put_i32(BUYER, on.into());
    }

    pub fn set_marketing(&mut self, on: bool) {
        self.put_i32(MARKETING, on.into());
    }

    /// What the stock was worth when the season began (+0xc90).
    pub fn stock_value_at_start(&self) -> i32 {
        self.i32_(STOCK_VALUE_AT_START)
    }

    pub fn fans_discount(&self) -> i16 {
        self.i16_(FANS_DISCOUNT)
    }

    pub fn set_fans_discount(&mut self, discount: i16) {
        if let Some(b) = self.0.get_mut(FANS_DISCOUNT..FANS_DISCOUNT + 2) {
            b.copy_from_slice(&discount.to_le_bytes());
        }
    }

    /// What a piece sells at, in cents (`FUN_00610a99`): the kind's price
    /// with the article's markup and the fans' discount, cut down to
    /// 50 cents, below 100 € to a euro, and from there to ten euros.
    pub fn price(&self, tables: &MerchandiseTables, article: &Article) -> i32 {
        let sold_at = tables.kinds.get(article.kind).map_or(0, |k| k.sold_at);
        let mut v = f64::from(sold_at);
        v *= f64::from(i32::from(article.markup) + 100) / 100.0;
        v *= f64::from(100 - i32::from(self.fans_discount())) / 100.0;
        let unit = if v < 1000.0 {
            50
        } else if v < 10000.0 {
            100
        } else {
            1000
        };
        v as i32 / unit * unit
    }

    /// An article's name (`FUN_0061069a`): the kind's text, its `%1` …
    /// `%7` filled — the goalkeeper's name, the club's short name, the
    /// rival's, the other name, and half the years since 1993, from 1 to 5,
    /// by the season's year; `%3` and `%6` stay empty.
    pub fn name(&self, text: &str, club: &str, rival: &str, year: u16) -> String {
        if !text.contains('%') {
            return text.to_owned();
        }
        let named = |set: usize, name| if self.i16_(set) != 0 { self.text(name) } else { String::new() };
        let mut out = String::new();
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            let Some(n) = chars.peek().filter(|_| c == '%').and_then(|d| d.to_digit(10)) else {
                out.push(c);
                continue;
            };
            chars.next();
            match n {
                1 => out.push_str(&named(KEEPER_SET, KEEPER_NAME)),
                2 => out.push_str(club),
                4 => out.push_str(rival),
                5 => out.push_str(&named(OTHER_SET, OTHER_NAME)),
                7 => out.push_str(&((i32::from(year) - 1993) / 2).clamp(1, 5).to_string()),
                _ => {}
            }
        }
        out
    }

    /// Orders `pieces` of an article (`FUN_0061157e`): no more than the
    /// article may hold, at the kind's price less the order's rebate,
    /// booked as merchandise bought; the pieces come in the weeks the kind
    /// takes. Returns what it cost.
    pub fn order(
        &mut self,
        tables: &MerchandiseTables,
        weeks: &[i16],
        place: usize,
        pieces: i32,
    ) -> Option<i32> {
        let article = self.article(place)?;
        let held = article.stock.wrapping_add(article.ordered);
        let pieces = if held.wrapping_add(pieces) > MOST_PIECES { MOST_PIECES - held } else { pieces };
        if pieces < 0 {
            return None;
        }
        let bought_at = tables.kinds.get(article.kind).map_or(0, |k| k.bought_at);
        let cost = f64::from(bought_at) * f64::from(pieces) / 100.0;
        let cost = f64::from(100 - rebate(pieces)) / 100.0 * cost;
        let week = usize::try_from(weeks.get(article.kind).copied().unwrap_or(1)).unwrap_or(0);
        let at = place * ARTICLE + STOCK + 4 * week;
        self.put_i32(at, self.i32_(at).wrapping_add(pieces));
        Some(cost as i32)
    }

    /// Orders `pieces` of every article not given up (`FUN_00611510`);
    /// returns what it all cost.
    pub fn order_all(&mut self, tables: &MerchandiseTables, weeks: &[i16], pieces: i32) -> i32 {
        let open: Vec<usize> =
            (0..ARTICLES).filter(|&p| self.0.get(p * ARTICLE + 1) == Some(&0)).collect();
        open.into_iter()
            .filter_map(|p| self.order(tables, weeks, p, pieces))
            .fold(0, i32::wrapping_add)
    }
}

/// The prices an article may be sold at (`FUN_0049d381`): from the
/// kind's price less half, a percent at a time to half as much again,
/// each price cut to its unit (`FUN_00610a29`), with the first markup
/// that gives it.
pub fn price_steps(sold_at: i32) -> Vec<(i32, i16)> {
    let price = |m: i16| {
        let v = (f64::from(sold_at) * (f64::from(m) + 100.0) / 100.0) as i32;
        let unit = if v < 1000 {
            50
        } else if v < 10000 {
            100
        } else {
            1000
        };
        v / unit * unit
    };
    let mut steps = vec![(price(-50), -50)];
    for m in -49..=50 {
        let p = price(m);
        if steps.last().is_some_and(|&(last, _)| last != p) {
            steps.push((p, m));
        }
    }
    steps
}

/// The scale of an article's chart (`FUN_0049d9b8`): the power of ten
/// above the best week (`ftol(log10(best) + 1.0)`), or a fifth, quarter
/// or half of it where the best is no more.
pub fn chart_scale(best: i32) -> i32 {
    let d = 10f64.powi((f64::from(best).log10() + 1.0) as i32) as i32;
    if best <= d / 5 {
        d / 5
    } else if best <= d / 4 {
        d / 4
    } else if best <= d / 2 {
        d / 2
    } else {
        d
    }
}

/// The rebate on an order of so many pieces, in percent; a small order
/// costs more (`FUN_006109d5`).
pub fn rebate(pieces: i32) -> i32 {
    match pieces {
        ..100 => -20,
        100..500 => -10,
        500..1000 => 0,
        1000..10000 => 10,
        10000..100000 => 20,
        _ => 30,
    }
}

/// What a man newly hired costs for the rest of the season
/// (`FUN_00620d09`): his yearly wage by the months still to come.
pub fn rest_of_season(wage: i32, months_left: u8) -> i32 {
    if months_left == 0 { 0 } else { (f64::from(months_left) * (f64::from(wage) / 12.0)) as i32 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prices_step_by_their_units() {
        let steps = price_steps(8000);
        assert_eq!(steps.first(), Some(&(4000, -50)));
        assert_eq!(steps.last(), Some(&(12000, 50)));
        assert!(steps.contains(&(8000, 0)));
        // Below ten euros the price goes by 50 cents, so markups share.
        let pens = price_steps(500);
        assert_eq!(&pens[..3], &[(250, -50), (300, -40), (350, -30)]);
    }

    #[test]
    fn the_chart_scales_to_a_step_above_the_best() {
        assert_eq!(chart_scale(92), 100);
        assert_eq!(chart_scale(45), 50);
        assert_eq!(chart_scale(20), 20);
        assert_eq!(chart_scale(270), 500);
    }

    #[test]
    fn rebates_follow_the_order_size() {
        let r: Vec<i32> = ORDER_SIZES.iter().map(|&n| rebate(n)).collect();
        assert_eq!(r, [-20, -10, -10, 0, 10, 10, 10, 20, 20, 20, 30]);
    }

    #[test]
    fn prices_are_cut_to_their_units() {
        let tables = MerchandiseTables {
            kinds: vec![Kind { sold_at: 8000, ..Kind::default() }, Kind { sold_at: 999, ..Kind::default() }],
        };
        let mut range = Range(vec![0; 0xd34]);
        range.set_fans_discount(10);
        let shirt = Article {
            place: 0, kind: 0, markup: 0, stock: 0, ordered: 0, sold: 0, taken: 0, excluded: false,
        };
        assert_eq!(range.price(&tables, &shirt), 7200);
        let pen = Article { kind: 1, markup: 5, ..shirt };
        // 999 · 1.05 · 0.9 = 944.055 → 900 cents.
        assert_eq!(range.price(&tables, &pen), 900);
    }
}
