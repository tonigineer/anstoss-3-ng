//! The share market as the managers trade on it (dialog 431
//! `CDiAktienmarkt`; ERFOLGE.md, "The share market"): the shares on offer,
//! football clubs' and companies', with their prices of the last 52 weeks,
//! and each human's holdings — fifty places, each a share, the price it
//! was bought at on average, how many, and whether they vote to sack its
//! club's manager. Buying takes from the shares on offer and from the
//! private fortune, with a fee; selling gives back.

use std::io;

use a3_files::exe::Exe;

use crate::discipline;
use crate::records::Records;

/// The market object (`0x8be408`, its data from 0x8be428): how many
/// shares, the index and its tendency, the shares a record of 0x108 each.
const MARKET_AT: &str = "0x8be408";
const COUNT: usize = 0;
const INDEX: usize = 4;
const TENDENCY: usize = 0xc;
const FIRST: usize = 0x14;
const ENTRY: usize = 0x108;
/// A share's record: its name, a company or a club, the club's country
/// and number, the shares its company has out, this week's place in the
/// ring of prices, the shares on offer, the change on the week before in
/// percent, the highest and lowest price of the year, the prices.
const NAME: usize = 0;
const NAME_LEN: usize = 0x14;
const KIND: usize = 0x14;
const COUNTRY: usize = 0x1c;
const NUMBER: usize = 0x1e;
const ISSUED: usize = 0x20;
const WEEK: usize = 0x24;
const OFFER: usize = 0x28;
const CHANGE: usize = 0x2c;
const HIGH: usize = 0x30;
const LOW: usize = 0x34;
const PRICES: usize = 0x38;
pub const WEEKS: usize = 52;
const COMPANY: i32 = 1;
/// A human's holdings (`0x8dc6e0[human − 1]`, their record from +0x20):
/// whether each of the fifty places is used, then the places of 0x10: the
/// share, the price paid on average, how many, the sacking vote.
const HOLDINGS_AT: &str = "0x8dc6e0";
pub const PLACES: usize = 50;
const HOLDING_FIRST: usize = 0x34;
const HOLDING: usize = 0x10;
const SHARE: usize = 0;
const BOUGHT_AT: usize = 4;
const HELD: usize = 8;
const SACK: usize = 0xc;
/// What a trade costs on top: half a percent and a euro, the cents cut
/// off (`0x80e950`, `0x80e948`).
const FEE_RATE: f64 = 0.005;
const FEE_BASE: f64 = 1.0;
/// The most shares a trade takes.
pub const MOST: i32 = 500_000;

/// Where the market's tables stand in the exe: the slider's scale of
/// (limit, step) pairs, the change's bounds between the rows' colours,
/// and those colours.
const SCALE_AT: u32 = 0x878c68;
const SCALE_STEPS: usize = 8;
const BOUNDS_AT: u32 = 0x8768c8;
const BOUNDS: usize = 4;
const INKS_AT: u32 = 0x878c50;

pub struct MarketTables {
    scale: Vec<(i32, i32)>,
    bounds: Vec<f32>,
    /// Windows colours, `0x00bbggrr`: red, orange, black, light blue, blue.
    pub inks: Vec<u32>,
}

impl MarketTables {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        let values: Vec<i32> = exe.u32s(SCALE_AT, 2 * SCALE_STEPS)?.into_iter().map(|v| v as i32).collect();
        let scale = values.chunks(2).map(|c| (c[0], c[1])).take_while(|&(_, step)| step != 0).collect();
        let bounds = exe.u32s(BOUNDS_AT, BOUNDS)?.into_iter().map(f32::from_bits).collect();
        Ok(Self { scale, bounds, inks: exe.u32s(INKS_AT, BOUNDS + 1)? })
    }

    /// How many shares a slider's position stands for (`FUN_005c69c9`).
    pub fn count_at(&self, position: i32) -> i32 {
        discipline::amount(&self.scale, position)
    }

    /// The position of a count on the slider (`FUN_005c690b`).
    pub fn position_of(&self, count: i32) -> i32 {
        discipline::position(&self.scale, count)
    }

    /// The colour a change in percent gives its row: the first bound it
    /// does not pass, or the last colour (`0x405aee`).
    pub fn ink(&self, change: f32) -> u32 {
        let class = self.bounds.iter().position(|&b| change <= b).unwrap_or(self.bounds.len());
        self.inks.get(class).copied().unwrap_or(0)
    }
}

/// A share on the market.
#[derive(Clone, Debug, PartialEq)]
pub struct Share {
    pub name: String,
    pub company: bool,
    /// The club's country and number; −1 and its own number for a company.
    pub country: i16,
    pub number: i16,
    pub issued: i32,
    week: i16,
    /// The shares to be had.
    pub offer: i32,
    /// The change on the week before, in percent.
    pub change: f32,
    pub high: f32,
    pub low: f32,
    prices: [f32; WEEKS],
}

impl Share {
    /// Its price `ago` weeks back, 0 this week's (`FUN_00407520`).
    pub fn price(&self, ago: usize) -> f32 {
        self.prices[(i32::from(self.week) + ago as i32).rem_euclid(WEEKS as i32) as usize]
    }
}

/// The market as it stands.
#[derive(Clone, Debug, PartialEq)]
pub struct Market {
    pub index: f32,
    pub tendency: f32,
    pub shares: Vec<Share>,
}

/// A place of a human's holdings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Holding {
    pub share: usize,
    pub bought_at: f32,
    pub count: i32,
    /// Whether these shares vote to sack the club's manager.
    pub sack: bool,
}

fn int(b: &[u8], at: usize) -> i32 {
    b.get(at..at + 4).map_or(0, |f| i32::from_le_bytes(f.try_into().unwrap()))
}

fn short(b: &[u8], at: usize) -> i16 {
    b.get(at..at + 2).map_or(0, |f| i16::from_le_bytes(f.try_into().unwrap()))
}

fn float(b: &[u8], at: usize) -> f32 {
    f32::from_bits(int(b, at) as u32)
}

fn holdings_block(human: usize) -> String {
    format!("{HOLDINGS_AT}[{human}]")
}

/// The market as the save keeps it.
pub fn market(records: &Records) -> Option<Market> {
    let b = records.block(MARKET_AT)?;
    let count = usize::try_from(int(b, COUNT)).unwrap_or(0);
    let shares = (0..count)
        .map_while(|i| {
            let e = b.get(FIRST + i * ENTRY..FIRST + (i + 1) * ENTRY)?;
            let name = &e[NAME..NAME + NAME_LEN];
            let end = name.iter().position(|&c| c == 0).unwrap_or(NAME_LEN);
            Some(Share {
                name: name[..end].iter().map(|&c| char::from(c)).collect(),
                company: int(e, KIND) == COMPANY,
                country: short(e, COUNTRY),
                number: short(e, NUMBER),
                issued: int(e, ISSUED),
                week: short(e, WEEK),
                offer: int(e, OFFER),
                change: float(e, CHANGE),
                high: float(e, HIGH),
                low: float(e, LOW),
                prices: std::array::from_fn(|k| float(e, PRICES + 4 * k)),
            })
        })
        .collect();
    Some(Market { index: float(b, INDEX), tendency: float(b, TENDENCY), shares })
}

/// The holdings of the human in this records slot (the manager object's
/// number less one), place by place.
pub fn holdings(records: &Records, human: usize) -> [Option<Holding>; PLACES] {
    let Some(b) = records.block(&holdings_block(human)) else { return [None; PLACES] };
    std::array::from_fn(|j| {
        if b.get(j).is_none_or(|&used| used == 0) {
            return None;
        }
        let at = HOLDING_FIRST + j * HOLDING;
        Some(Holding {
            share: usize::try_from(int(b, at + SHARE)).unwrap_or(usize::MAX),
            bought_at: float(b, at + BOUGHT_AT),
            count: int(b, at + HELD),
            sack: b.get(at + SACK).is_some_and(|&s| s != 0),
        })
    })
}

/// Whether the human may buy the share: he holds it already, or a place
/// is free (`FUN_004069d0`).
pub fn can_buy(records: &Records, human: usize, share: usize) -> bool {
    holdings(records, human).iter().any(|h| h.is_none_or(|h| h.share == share))
}

/// What a trade of so much costs in fees.
pub fn fee(value: i32) -> i32 {
    (f64::from(value) * FEE_RATE + FEE_BASE) as i32
}

/// What `count` shares come to at `price`, the cents cut off.
pub fn value(price: f32, count: i32) -> i32 {
    (f64::from(price) * f64::from(count)) as i32
}

/// Buys `count` of the share for the human (`FUN_004065a9`): no more than
/// are on offer — the game then says another was quicker
/// (`FUN_00406472`) —; into the place that holds it, the price paid
/// averaged, or else into the last free place; the offer goes down. The
/// fortune pays their value and the fee. How many he got.
pub fn buy(records: &mut Records, human: usize, share: usize, count: i32) -> i32 {
    let Some(price) = market(records).and_then(|m| m.shares.get(share).map(|s| s.price(0))) else {
        return 0;
    };
    let offer = records.block(MARKET_AT).map_or(0, |b| int(b, FIRST + share * ENTRY + OFFER));
    let count = count.min(offer);
    if count <= 0 {
        return 0;
    }
    let held = holdings(records, human);
    let place = held.iter().position(|h| h.is_some_and(|h| h.share == share));
    let place = place.or_else(|| held.iter().rposition(Option::is_none));
    let Some(place) = place else { return 0 };
    let Some(b) = records.block_mut(&holdings_block(human)) else { return 0 };
    let at = HOLDING_FIRST + place * HOLDING;
    let (bought_at, before) = match held[place] {
        Some(h) => {
            // Worked out wider than it is kept, as the game's x87 does.
            let total = f64::from(h.count + count);
            let paid = f64::from(price) * f64::from(count) + f64::from(h.count) * f64::from(h.bought_at);
            ((paid / total) as f32, h.count)
        }
        None => {
            b[place] = 1;
            b[at + SACK] = 0;
            b[at..at + 4].copy_from_slice(&(share as i32).to_le_bytes());
            (price, 0)
        }
    };
    b[at + BOUGHT_AT..at + BOUGHT_AT + 4].copy_from_slice(&bought_at.to_le_bytes());
    b[at + HELD..at + HELD + 4].copy_from_slice(&(before + count).to_le_bytes());
    set_offer(records, share, offer - count);
    let paid = value(price, count);
    pay(records, human, -(paid + fee(paid)));
    count
}

/// Sells `count` of the place's shares (`FUN_004067a5`): the offer goes
/// up, the place empties with its last share, and the fortune takes their
/// value less the fee.
pub fn sell(records: &mut Records, human: usize, place: usize, count: i32) {
    let Some(h) = holdings(records, human)[place] else { return };
    let count = count.min(h.count);
    if count <= 0 {
        return;
    }
    let Some(share) = market(records).and_then(|m| m.shares.get(h.share).cloned()) else { return };
    let Some(b) = records.block_mut(&holdings_block(human)) else { return };
    let at = HOLDING_FIRST + place * HOLDING;
    b[at + HELD..at + HELD + 4].copy_from_slice(&(h.count - count).to_le_bytes());
    if h.count == count {
        b[place] = 0;
    }
    set_offer(records, h.share, share.offer + count);
    let got = value(share.price(0), count);
    pay(records, human, got - fee(got));
}

/// Whether the place's shares vote to sack their club's manager, as the
/// honours page's list switches it.
pub fn set_sack(records: &mut Records, human: usize, place: usize, sack: bool) {
    if let Some(b) = records.block_mut(&holdings_block(human)) {
        b[HOLDING_FIRST + place * HOLDING + SACK] = u8::from(sack);
    }
}

fn set_offer(records: &mut Records, share: usize, offer: i32) {
    if let Some(b) = records.block_mut(MARKET_AT) {
        let at = FIRST + share * ENTRY + OFFER;
        b[at..at + 4].copy_from_slice(&offer.to_le_bytes());
    }
}

/// The private fortune of the human (the manager object's +0x98).
fn pay(records: &mut Records, human: usize, amount: i32) {
    if let Some(manager) = records.managers.get_mut(human) {
        manager.set_cash(manager.cash().wrapping_add(amount));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trade_costs_half_a_percent_and_a_euro() {
        // Six at 151.32: 907 and 5; 31 at 681.16: 21.115 and 106
        // (peeked on the original).
        assert_eq!((value(151.32, 6), fee(value(151.32, 6))), (907, 5));
        assert_eq!((value(681.16, 31), fee(value(681.16, 31))), (21_115, 106));
        assert_eq!(fee(value(151.32, 3)), 3);
    }
}
