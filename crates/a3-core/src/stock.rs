//! The stock market and what the managers hold of their own club
//! (VEREINSFUEHRUNG.md §2.6): the shares a club has out, the portfolios'
//! places, and the majority which carries a manager's confidence to 100
//! (`FUN_00405e95`, `FUN_0040689a`, `FUN_00406b4f`, `FUN_007701b2`).

use crate::random::Random;
use crate::records::{Records, Team};

/// The stock market object (`0x8be408`, its data from +0x20): an i32
/// count and entries of 0x108 from +0x14.
const MARKET_AT: u32 = 0x8be408;
const MARKET_COUNT: usize = 0;
const MARKET_FIRST: usize = 0x14;
const MARKET_ENTRY: usize = 0x108;
const CLUB_COUNTRY: usize = 0x1c;
const CLUB_NUMBER: usize = 0x1e;
const CLUB_SHARES: usize = 0x20;
/// The managers' portfolios (`0x8dc6e0[slot − 1]`): fifty places each,
/// a used flag, the market entry held and how many shares.
const PORT_AT: u32 = 0x8dc6e0;
const PORT_SLOTS: usize = 50;
const PORT_USED: usize = 0;
const PORT_ENTRY: usize = 0x34;
const PORT_SHARES: usize = 0x3c;
const PORT_STEP: usize = 0x10;
/// The weeks of prices an entry keeps: the current one stands at
/// `0x38 + week % 52 * 4`, the week itself at `0x24` (`FUN_00407520`).
const PRICES: usize = 52;
const PRICES_AT: usize = 0x38;
const CLUB_WEEK: usize = 0x24;

fn word(b: &[u8], at: usize) -> i16 {
    match (b.get(at), b.get(at + 1)) {
        (Some(lo), Some(hi)) => i16::from_le_bytes([*lo, *hi]),
        _ => 0,
    }
}

fn long(b: &[u8], at: usize) -> i32 {
    match b.get(at..at + 4) {
        Some(four) => i32::from_le_bytes(four.try_into().unwrap()),
        _ => 0,
    }
}

/// The club's place on the market and its shares in all
/// (`FUN_00405e95`).
fn listing(records: &Records, country: u8, number: i16) -> Option<(usize, i32)> {
    let b = records.block(&format!("{MARKET_AT:#x}"))?;
    let count = word(b, MARKET_COUNT).max(0) as usize;
    (0..count).find_map(|k| {
        let e = MARKET_FIRST + k * MARKET_ENTRY;
        if word(b, e + CLUB_COUNTRY) != i16::from(country) || word(b, e + CLUB_NUMBER) != number {
            return None;
        }
        Some((k, long(b, e + CLUB_SHARES)))
    })
}

/// What the manager in this slot holds of that market place
/// (`FUN_0040689a`).
fn holding(records: &Records, slot: i16, place: usize) -> i32 {
    let Some(b) = records.block(&format!("{PORT_AT:#x}[{}]", slot - 1)) else {
        return 0;
    };
    (0..PORT_SLOTS).fold(0, |sum, k| {
        let used = b.get(PORT_USED + k).is_some_and(|&u| u != 0);
        let at = PORT_ENTRY + k * PORT_STEP;
        if !used || usize::try_from(long(b, at)).unwrap_or(usize::MAX) != place {
            return sum;
        }
        sum + long(b, at + (PORT_SHARES - PORT_ENTRY))
    })
}

/// The shares as the board screen shows them (§2.6): the shares in all,
/// the manager's own, and the other humans' by their place among the
/// managers. Nothing when the club is not on the market.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Split {
    pub total: i32,
    pub own: i32,
    pub others: Vec<(usize, i32)>,
}

/// The manager's place among the humans, 1 to 4, from the slots the
/// globals keep; 0 for a manager not in play. His portfolio of shares
/// stands at `0x8dc6e0[place − 1]`.
pub fn place(records: &Records, index: usize) -> i16 {
    records.globals.slots().get(index).copied().unwrap_or(0)
}

/// What all of this human's holdings come to (`FUN_006ddc73`): his
/// shares in every club still on the market, each at that club's current
/// price, all summed.
pub fn portfolio_value(records: &Records, human: i16) -> i64 {
    let Some(port) = records.block(&format!("{PORT_AT:#x}[{}]", human - 1)) else {
        return 0;
    };
    let Some(market) = records.block(&format!("{MARKET_AT:#x}")) else {
        return 0;
    };
    (0..PORT_SLOTS).fold(0, |sum, k| {
        if port.get(PORT_USED + k).is_none_or(|&used| used == 0) {
            return sum;
        }
        let at = PORT_ENTRY + k * PORT_STEP;
        let Ok(place) = usize::try_from(long(port, at)) else { return sum };
        let Some(entry) = market.get(MARKET_FIRST + place * MARKET_ENTRY..) else {
            return sum;
        };
        if word(entry, CLUB_COUNTRY) == 0 {
            return sum;
        }
        let Ok(ring) = usize::try_from(word(entry, CLUB_WEEK)) else { return sum };
        let Some(four) = entry.get(PRICES_AT + ring % PRICES * 4..PRICES_AT + ring % PRICES * 4 + 4)
        else {
            return sum;
        };
        let price = f32::from_le_bytes(four.try_into().unwrap());
        let shares = long(port, at + (PORT_SHARES - PORT_ENTRY)) as f32;
        sum + (shares * price).round() as i64
    })
}

/// The club's shares among the humans (`FUN_005b548f`'s reading): the
/// manager's own holding and every other human's, in the order the
/// board screen's rows take them.
pub fn split(records: &Records, slot: i16, country: u8, number: i16) -> Option<Split> {
    let (place, total) = listing(records, country, number)?;
    if total <= 0 {
        return None;
    }
    let others = records
        .globals
        .slots()
        .iter()
        .enumerate()
        .filter(|(_, s)| **s > 0 && **s != slot)
        .map(|(i, &s)| (i, holding(records, s, place)))
        .collect();
    Some(Split { total, own: holding(records, slot, place), others })
}

/// Whether the manager at this place among the managers holds the
/// majority of his own club's shares (`FUN_00406b4f`): the club is on
/// the market and he holds more than half of it.
pub fn majority(records: &Records, index: usize) -> bool {
    let Some(manager) = records.managers.get(index) else { return false };
    let slot = place(records, index);
    if slot == 0 || manager.country() == 0 {
        return false;
    }
    listing(records, manager.country() as u8, manager.team() as i16)
        .is_some_and(|(place, total)| total != 0 && holding(records, slot, place) > total / 2)
}

/// Where in a portfolio's place the share's vote waits: a byte set when
/// the holding is to be counted at the next vote.
const PORT_VOTE: usize = 0x40;
/// A club is voted on only with more shares than this out.
const VOTE_LEAST: i32 = 1000;

/// The shareholders' vote (`FUN_00406baf`, first in `FUN_00751d76` as the
/// turns begin): every human's places with a vote waiting are counted
/// per club, in the order the clubs first come, and the wait cleared.
/// A club on the market with more than a thousand shares takes the count
/// as its percent for (team +0xb2, club +0x90) — 100 once it is more than
/// half — and 0 against (+0xb3): the original's second sum is never
/// added to.
pub fn vote(records: &mut Records) {
    let Some(market) = records.block(&format!("{MARKET_AT:#x}")).map(<[u8]>::to_vec) else {
        return;
    };
    // (country, number, for, against) as `local_964`'s twelve bytes.
    let mut clubs: Vec<(u8, i16, i32, i32)> = Vec::new();
    let slots: Vec<i16> = records.globals.slots().iter().copied().filter(|&s| s > 0).collect();
    for slot in slots {
        let Some(port) = records.block_mut(&format!("{PORT_AT:#x}[{}]", slot - 1)) else {
            continue;
        };
        for k in 0..PORT_SLOTS {
            if port.get(PORT_USED + k).is_none_or(|&u| u == 0) {
                continue;
            }
            let at = PORT_ENTRY + k * PORT_STEP;
            let e = MARKET_FIRST + long(port, at).wrapping_mul(MARKET_ENTRY as i32) as usize;
            let country = word(&market, e.wrapping_add(CLUB_COUNTRY));
            if country <= 0 || port.get(at + (PORT_VOTE - PORT_ENTRY)).is_none_or(|&v| v == 0) {
                continue;
            }
            let number = word(&market, e.wrapping_add(CLUB_NUMBER));
            let shares = long(port, at + (PORT_SHARES - PORT_ENTRY));
            // The club is found by its country byte, as the list keeps it.
            match clubs.iter_mut().find(|c| i16::from(c.0) == country && c.1 == number) {
                Some(club) => club.2 = club.2.wrapping_add(shares),
                None => clubs.push((country as u8, number, shares, 0)),
            }
            port[at + (PORT_VOTE - PORT_ENTRY)] = 0;
        }
    }
    for (country, number, pro, contra) in clubs {
        let Some((_, total)) = listing(records, country, number) else { continue };
        if total <= VOTE_LEAST {
            continue;
        }
        let share = |n: i32| if n >= total / 2 + 1 { 100 } else { (n.wrapping_mul(100) / total) as u8 };
        if let Some(team) = records.team_mut(u16::from(country), number as u16) {
            team.set_votes([share(pro), share(contra)]);
        }
    }
}

/// The board's confidence as the screens show it: 100 for a manager
/// holding the majority of his own club, else as he stands
/// (`FUN_007701b2`).
pub fn confidence(records: &Records, index: usize) -> i32 {
    match records.managers.get(index) {
        Some(_) if majority(records, index) => 100,
        Some(manager) => manager.shown_confidence(),
        None => 0,
    }
}

/// A club going public on the market (`FUN_00406201` in a game on one
/// machine): one already there takes the shares; a new one goes on at the
/// end with its first price (`FUN_00404dcd`), and the count grows.
pub fn list(records: &mut Records, index: usize, shares: i32, random: &mut Random) {
    let Some(team) = records.teams.get(index) else { return };
    let (country, number) = (i16::from(team.country()), i16::from(team.number()));
    let Some(mut market) = records.block(&format!("{MARKET_AT:#x}")).map(<[u8]>::to_vec) else {
        return;
    };
    let count = usize::try_from(long(&market, MARKET_COUNT)).unwrap_or(0);
    let found = (0..count).map(|k| MARKET_FIRST + k * MARKET_ENTRY).find(|&e| {
        word(&market, e + CLUB_COUNTRY) == country && word(&market, e + CLUB_NUMBER) == number
    });
    if let Some(e) = found {
        put_long(&mut market, e + CLUB_SHARES, shares);
    } else {
        let e = MARKET_FIRST + count * MARKET_ENTRY;
        if market.len() < e + MARKET_ENTRY {
            return;
        }
        let name: Vec<u8> = team.bytes().iter().copied().take_while(|&b| b != 0).collect();
        {
            let entry = &mut market[e..e + MARKET_ENTRY];
            put_word(entry, CLUB_WEEK, NEW_WEEK);
            put_long(entry, CLUB_SHARES, NO_SHARES);
            put_word(entry, CLUB_COUNTRY, country);
            put_word(entry, CLUB_NUMBER, number);
            put_long(entry, NAME_END, 0);
            // The name goes in as `strcpy` puts it, over what follows.
            let end = (name.len() + 1).min(MARKET_ENTRY);
            entry[..end].copy_from_slice(&[&name[..], &[0]].concat()[..end]);
            put_long(entry, CLUB_SHARES, shares);
        }
        let price = first_price(records, team, &mut market, e, random);
        let entry = &mut market[e..e + MARKET_ENTRY];
        entry[HIGH..HIGH + 4].copy_from_slice(&price.to_le_bytes());
        entry[LOW..LOW + 4].copy_from_slice(&price.to_le_bytes());
        put_long(&mut market, MARKET_COUNT, count as i32 + 1);
    }
    if let Some(block) = records.block_mut(&format!("{MARKET_AT:#x}")) {
        block.copy_from_slice(&market);
    }
}

/// A new entry's week in the ring before its first price, the shares it
/// holds until it takes the club's, and where its name ends.
const NEW_WEEK: i16 = 51;
const NO_SHARES: i32 = 99_999_999;
const NAME_END: usize = 0x14;
/// What else an entry keeps: a bonus on its price in percent, the
/// price's drift, the shares to be had, the change on the week before in
/// percent, the highest and lowest price.
const BONUS: usize = 0x18;
const DRIFT: usize = 0x26;
const TO_BE_HAD: usize = 0x28;
const CHANGE: usize = 0x2c;
const HIGH: usize = 0x30;
const LOW: usize = 0x34;
const DRIFT_MOST: i16 = 50;
/// The least a share is priced at (`DAT_00a3b424`).
const LEAST_PRICE: f32 = 5.0;
/// The team's fields the price goes by: its table place, last season's,
/// its squad's rank (club +0x109, +0x10b, +0x107); the honours it won
/// (`FUN_0069a031`), a cup run's bit (`FUN_0069a261`), its form at
/// home and away by three bits each (club +0xd8).
const PLACE: usize = 0x132;
const LAST_PLACE: usize = 0x134;
const SQUAD_RANK: usize = 0x130;
const HONOURS: usize = 0x8ea;
const RUNS: usize = 0x110;
const CUP_RUN: u16 = 0x1000;
const FORM: usize = 0xfc;
/// The European cups' holders, three of country and number
/// (`DAT_00ae6dd4`, `FUN_0042419d`).
const HOLDERS_AT: u32 = 0xae6dd4;
const HOLDERS: usize = 3;
/// The league's teams and its matchdays played, in its record.
const LEAGUE_AT: u32 = 0x1a09d48;
const LEAGUE_TEAMS: usize = 0x30;
const LEAGUE_PLAYED: usize = 0x46;
const LOWEST_LEAGUE: u8 = 6;
const DEFAULT_TEAMS: i16 = 18;

fn put_word(b: &mut [u8], at: usize, v: i16) {
    b[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

fn put_long(b: &mut [u8], at: usize, v: i32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn float(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

/// A club's price for the week (`FUN_00405024` for a club whose country
/// keeps its teams): from 120, 60 or 30 by its league's level, six points a
/// place its squad ranks above the league's last, four a place it stands
/// above the last, now or last season as the season has gone on, ten for
/// the top; 30, 20 and 10 for its honours, half or one and a half of
/// those again for a cup run or a European cup held; its form at home and
/// away rolled in; its drift; up to a euro more. Then the bonus, at least
/// five euros, the change on the week before, and the highest and lowest.
fn first_price(
    records: &Records,
    team: &Team,
    market: &mut [u8],
    e: usize,
    random: &mut Random,
) -> f32 {
    let mut week = word(&market[e..], CLUB_WEEK) - 1;
    if week < 0 {
        week = NEW_WEEK;
    }
    put_word(&mut market[e..], CLUB_WEEK, week);
    let mut base: f32 = match team.level() {
        0 => 120.0,
        1 => 60.0,
        _ => 30.0,
    };
    let (teams, played) = league(records, team);
    let (now, last): (f32, f32) = match played {
        0 => (0.0, 1.0),
        1 => (0.2, 0.8),
        2 => (0.4, 0.6),
        3 => (0.6, 0.4),
        4 => (0.8, 0.2),
        _ => (1.0, 0.0),
    };
    let above = |at: usize| f64::from(i32::from(teams) - i32::from(team.get::<u8>(at)));
    base = (above(PLACE) * f64::from(now) * 4.0 + f64::from(base)) as f32;
    base = (above(LAST_PLACE) * f64::from(last) * 4.0 + f64::from(base)) as f32;
    base = (above(SQUAD_RANK) * 6.0 + f64::from(base)) as f32;
    if team.get::<u8>(PLACE) == 1 {
        base += 10.0;
    }
    let honours = team.get::<u8>(HONOURS);
    let mut won = 0.0f32;
    for (bits, points) in [(1, 30.0), (2, 20.0), (3, 10.0)] {
        if honours & bits != 0 {
            base += points;
            won += points;
        }
    }
    if team.get::<u16>(RUNS) & CUP_RUN != 0 {
        base = (f64::from(won) * 0.5 + f64::from(base)) as f32;
    }
    if holds_cup(records, team) {
        base = (f64::from(won) * 1.5 + f64::from(base)) as f32;
    }
    let form = team.get::<i16>(FORM);
    for (bits, most) in [(form & 7, 20), ((form >> 3) & 7, 10)] {
        base = match bits {
            0 | 1 => base + random.below(most) as f32,
            2 => base - random.below(most) as f32,
            3 => (f64::from(random.below(10)) - 5.0 + f64::from(base)) as f32,
            _ => base,
        };
    }
    let entry = &mut market[e..e + MARKET_ENTRY];
    let drift = word(entry, DRIFT).wrapping_add(2 - random.below(5) as i16);
    let drift = drift.clamp(-DRIFT_MOST, DRIFT_MOST);
    put_word(entry, DRIFT, drift);
    base = (f64::from(drift) + f64::from(base)) as f32;
    let price = (f64::from(random.below(100)) / 100.0 + f64::from(base)) as f32;
    let to_be_had = random.below(0x97) + random.below(0x47) + 0x5a + random.below(0x1f);
    let to_be_had = to_be_had as i32 * 10;
    put_long(entry, TO_BE_HAD, to_be_had);
    priced(records, market, e, i16::from(team.country()), i16::from(team.number()), price)
}

/// What every price for a new week ends with (`FUN_00405024`'s tail): the
/// shares to be had cut down to what no portfolio holds yet, the bonus on
/// the price in percent, the floor of five euros, the price in the week's
/// place of the ring, the change on the week before, and the highest and
/// lowest kept up to date. What is missing against the original: an
/// entry pushed down to the floor sends its watchers the news of it
/// (the loop above `FUN_00406917`, events 0x7cd and 0x7ce) — drawing
/// nothing, so no parity hangs on it.
fn priced(
    records: &Records,
    market: &mut [u8],
    e: usize,
    country: i16,
    number: i16,
    mut price: f32,
) -> f32 {
    let week = word(&market[e..], CLUB_WEEK);
    let held: i32 = (0..crate::records::MANAGERS as i16)
        .map(|slot| held(records, market, slot + 1, country, number))
        .fold(0, i32::wrapping_add);
    let entry = &mut market[e..e + MARKET_ENTRY];
    let free = long(entry, CLUB_SHARES).wrapping_sub(held);
    if long(entry, TO_BE_HAD) > free {
        put_long(entry, TO_BE_HAD, free);
    }
    price = (f64::from(float(entry, BONUS)) / 100.0 * f64::from(price) + f64::from(price)) as f32;
    if price < LEAST_PRICE {
        price = LEAST_PRICE;
    }
    let ring = |w: i16| PRICES_AT + usize::try_from(w).unwrap_or(0) % PRICES * 4;
    entry[ring(week)..ring(week) + 4].copy_from_slice(&price.to_le_bytes());
    let before = float(entry, ring((week + 1) % PRICES as i16));
    let change = if before == 0.0 {
        0.0
    } else {
        let points = ((f64::from(price) - f64::from(before)) * 100.0) as f32;
        (f64::from(points) / f64::from(before)) as f32
    };
    entry[CHANGE..CHANGE + 4].copy_from_slice(&change.to_le_bytes());
    if price > float(entry, HIGH) {
        entry[HIGH..HIGH + 4].copy_from_slice(&price.to_le_bytes());
    }
    if price < float(entry, LOW) {
        entry[LOW..LOW + 4].copy_from_slice(&price.to_le_bytes());
    }
    price
}

/// An entry no country keeps a club of takes its price for the new week
/// from last week's, walked by the weekday (`FUN_00405024`'s other half):
/// the day of the week rolls its drift. A club of a country that keeps no
/// teams also re-rolls its shares to be had; what is missing: the index
/// entries' re-roll stands in a table the game only keeps at
/// `0xbdeea0`, and its draws are not taken yet.
fn index_price(market: &mut [u8], e: usize, day: i32, random: &mut Random) -> f32 {
    let mut week = word(&market[e..], CLUB_WEEK) - 1;
    if week < 0 {
        week = NEW_WEEK;
    }
    put_word(&mut market[e..], CLUB_WEEK, week);
    let (n, off) = match (day % 7 + 7) % 7 {
        0 => (0x3e9, -5.0),
        1 => (0x3e9, -4.0),
        2 => (0x4b1, -4.0),
        3 => (0x579, -4.0),
        4 => (0x3e9, -6.0),
        5 => (0x321, -4.0),
        _ => (0x259, -4.0),
    };
    let ring = |w: i16| PRICES_AT + usize::try_from(w).unwrap_or(0) % PRICES * 4;
    let before = float(&market[e..e + MARKET_ENTRY], ring((week + 1) % PRICES as i16));
    (f64::from(random.below(n)) / 100.0 + off) as f32 + before
}

/// The week's end on the market (`FUN_00405c5e`, before the advance sales
/// open): every entry takes its price for the new week, the mean of them
/// moves the market's own index — the change on the week before, its run,
/// and the index line at five thousand about the run's middle — and the
/// entries' bonuses go back to nought for the week to come.
pub fn roll_week(records: &mut Records, day: i32, random: &mut Random) {
    let Some(mut market) = records.block(&format!("{MARKET_AT:#x}")).map(<[u8]>::to_vec) else {
        return;
    };
    let count = usize::try_from(long(&market, MARKET_COUNT)).unwrap_or(0);
    if count == 0 {
        return;
    }
    let mut mean = 0.0f32;
    for k in 0..count {
        let e = MARKET_FIRST + k * MARKET_ENTRY;
        if market.len() < e + MARKET_ENTRY {
            break;
        }
        let (country, number) = (word(&market[e..], CLUB_COUNTRY), word(&market[e..], CLUB_NUMBER));
        let price = if country >= 0 && records.countries.keeps_club(usize::from(country as u8), number)
        {
            match records.team(u16::try_from(country).unwrap_or(0), u16::try_from(number).unwrap_or(0))
            {
                Some(team) => first_price(records, team, &mut market, e, random),
                None => index_price(&mut market, e, day, random),
            }
        } else {
            let price = index_price(&mut market, e, day, random);
            if country >= 0 {
                let to_be_had =
                    random.below(0x97) + random.below(0x47) + 0x5a + random.below(0x1f);
                put_long(&mut market[e..], TO_BE_HAD, to_be_had as i32 * 10);
            }
            priced(records, &mut market, e, country, number, price)
        };
        mean += price;
    }
    let mean = mean / count as f32;
    let before = float(&market, PREV_MEAN);
    let change = ((mean - before) * 100.0) / before;
    let run = float(&market, RUN) + change / 100.0;
    put_float(&mut market, INDEX, run * 5000.0 + 5000.0);
    put_float(&mut market, RUN, run);
    put_float(&mut market, CHANGE_ON, change);
    put_float(&mut market, PREV_MEAN, mean);
    for k in 0..count {
        let e = MARKET_FIRST + k * MARKET_ENTRY;
        if market.len() < e + MARKET_ENTRY {
            break;
        }
        put_long(&mut market, e + BONUS, 0);
    }
    if let Some(block) = records.block_mut(&format!("{MARKET_AT:#x}")) {
        block.copy_from_slice(&market);
    }
}

/// The market's own index in its header (`FUN_00405c5e`): the index line,
/// the mean of the prices it stands on, the change on the week before in
/// percent, and the run of those changes.
const INDEX: usize = 0x4;

/// The index names' table (`0xbdeea0`, saved in the block `0xbdee80`):
/// 0xc9 names of 0x16 each with the word at +0x14 (`DAT_00bdeeb4`) that
/// says how the entry trades — nought for every name of the original's
/// table. The market never lists the first name.
const NAMES_AT: &str = "0xbdee80";
const NAME_ENTRY: usize = 0x16;
const NAME_FLAG: usize = 0x14;
const NAMES: usize = 0xc9;
/// The shares an index entry goes on with (`DAT_00989680` at the first
/// trade) and the most rows the market ever takes.
const INDEX_SHARES: i32 = 10_000_000;
const MARKET_MOST: usize = 0x1d4;
/// The club's bit of having shares out at all (`FUN_0069d71d`, the legal
/// form at club +0x924), and the countries the market walks with their
/// clubs' count (`0xae5e06`).
const LEGAL_FORM: usize = 0x924;
const COUNTRIES: i16 = 0x75;

/// A name of the table or of a market row: up to the next nought, never
/// past its place.
fn row_name(b: &[u8], at: usize) -> Vec<u8> {
    match b.get(at..at + NAME_ENTRY) {
        Some(field) => field.iter().copied().take_while(|&c| c != 0).collect(),
        None => Vec::new(),
    }
}

/// The word that says how the table entry trades.
fn name_flag(names: &[u8], entry: usize) -> i16 {
    names
        .get(entry * NAME_ENTRY + NAME_FLAG..entry * NAME_ENTRY + NAME_FLAG + 2)
        .map_or(0, |w| i16::from_le_bytes([w[0], w[1]]))
}

/// The shares an index entry offers for the new week (`FUN_00405024`'s
/// week off the weekday, table entries only): five thousand to eight
/// thousand by the first trade, fifteen to thirty-five thousand by the
/// second, forty to a hundred thousand by the third. The entry is the
/// row's number — one past its place in the table — so the neighbour's
/// word is read, as the original reads the number and not the place.
fn index_had(names: &[u8], entry: usize, random: &mut Random) -> i32 {
    match name_flag(names, entry) {
        0 => random.below(0x33) as i32 * 100 + 5000,
        1 => random.below(0x15) as i32 * 1000 + 15000,
        _ => random.below(0x3d) as i32 * 1000 + 40000,
    }
}

/// One market row's price for the new week (`FUN_00405024` called on the
/// row): the club's own while its country keeps its clubs, the ring's
/// walk off the weekday else. An index entry re-rolls its shares by the
/// table's trade word; a club of a country that keeps no clubs re-rolls
/// them raw — without the tenfold the club's own price puts on them, as
/// the original leaves them here.
fn row_price(
    records: &Records,
    market: &mut [u8],
    e: usize,
    day: i32,
    names: &[u8],
    random: &mut Random,
) -> f32 {
    let (country, number) = (word(&market[e..], CLUB_COUNTRY), word(&market[e..], CLUB_NUMBER));
    if country >= 0 && records.countries.keeps_club(usize::from(country as u8), number) {
        match records.team(
            u16::try_from(country).unwrap_or(0),
            u16::try_from(number).unwrap_or(0),
        ) {
            Some(team) => first_price(records, team, market, e, random),
            None => {
                let price = index_price(market, e, day, random);
                priced(records, market, e, country, number, price)
            }
        }
    } else {
        let price = index_price(market, e, day, random);
        if country >= 0 {
            let to_be_had = random.below(0x97) + random.below(0x47) + 0x5a + random.below(0x1f);
            put_long(market, e + TO_BE_HAD, to_be_had as i32);
        } else {
            let entry = usize::try_from(number).unwrap_or(0);
            put_long(market, e + TO_BE_HAD, index_had(names, entry, random));
        }
        priced(records, market, e, country, number, price)
    }
}

/// Opening the market at the new game (`FUN_00405f2f`, its first half of
/// `FUN_0065e659`'s first season): every club of a played country that
/// has shares out goes on with its first price fixed at one euro
/// (`FUN_00404dcd` — `FUN_00482531` counts the club's worth into shares,
/// but its two constants live only in the game's memory, so the shares
/// stand at nought here; they are never below one at a new game, which
/// is why the three spare draws of a share-less club are not missed);
/// then every index name the market does not carry yet goes on
/// (`FUN_00404ed8`, the name looked for only among the index rows — the
/// club rows are never compared, the search starting where they end).
/// Fifty-two weekly prices then walk the mean, the change on the week
/// before and the run into the index line; the entries' bonuses are not
/// zeroed here. `FUN_007cfb99` and `FUN_007cff0c`, around the whole,
/// only lock and unlock the object. What is missing: `FUN_0063a317`'s
/// news of a club left without a row and the news of entries pushed to
/// the floor of five euros (`FUN_00406917`), neither drawing anything.
pub fn open(records: &mut Records, random: &mut Random) {
    let Some(mut market) = records.block(&format!("{MARKET_AT:#x}")).map(<[u8]>::to_vec) else {
        return;
    };
    let names = records
        .block(NAMES_AT)
        .map(<[u8]>::to_vec)
        .unwrap_or_default();
    market.fill(0);
    let day = i32::from(records.globals.start_year()) - 0x7ca;
    let mut mean = 0.0f32;
    let mut count = 0usize;
    for country in 1..COUNTRIES {
        if !records.globals.played(country) {
            continue;
        }
        let teams = records.countries.team_count(usize::from(country as u16)).max(0);
        for number in 0..teams {
            // `FUN_0063e793` finds no row of this club yet: the market
            // stands cleared.
            let Some(team) = records.team(u16::from(country as u16), u16::try_from(number).unwrap_or(0))
            else {
                continue;
            };
            if team.get::<u8>(LEGAL_FORM) & 1 == 0 {
                continue;
            }
            let e = MARKET_FIRST + count * MARKET_ENTRY;
            if market.len() < e + MARKET_ENTRY {
                break;
            }
            let name: Vec<u8> = team.bytes().iter().copied().take_while(|&b| b != 0).collect();
            {
                let entry = &mut market[e..e + MARKET_ENTRY];
                put_word(entry, CLUB_WEEK, NEW_WEEK);
                put_long(entry, CLUB_SHARES, NO_SHARES);
                put_word(entry, CLUB_COUNTRY, country);
                put_word(entry, CLUB_NUMBER, number);
                put_long(entry, NAME_END, 0);
                // The name goes in as `strcpy` puts it, over what follows.
                let end = (name.len() + 1).min(MARKET_ENTRY);
                entry[..end].copy_from_slice(&[&name[..], &[0]].concat()[..end]);
                put_long(entry, CLUB_SHARES, 0);
            }
            row_price(records, &mut market, e, day, &names, random);
            // `FUN_00404dcd` fixes the first price at one euro: it stands
            // in the week the row's price left, high and low with it,
            // and the euro it returns moves the mean.
            let week = word(&market[e..], CLUB_WEEK);
            let ring = PRICES_AT + usize::try_from(week).unwrap_or(0) % PRICES * 4;
            let entry = &mut market[e..e + MARKET_ENTRY];
            for at in [ring, HIGH, LOW] {
                entry[at..at + 4].copy_from_slice(&1.0f32.to_le_bytes());
            }
            mean += 1.0;
            count += 1;
        }
    }
    let clubs = count;
    for k in 1..NAMES {
        if count >= MARKET_MOST {
            break;
        }
        let wanted = row_name(&names, k * NAME_ENTRY);
        // The market's own clubs are not looked at (`sVar1` stands at
        // their count), only the index rows made so far.
        if (clubs..count).any(|j| row_name(&market, MARKET_FIRST + j * MARKET_ENTRY) == wanted) {
            continue;
        }
        let e = MARKET_FIRST + count * MARKET_ENTRY;
        if market.len() < e + MARKET_ENTRY {
            break;
        }
        let shares = match name_flag(&names, k) {
            0 => INDEX_SHARES + random.below(0xb) as i32 * 1_000_000,
            // The second trade names a second base (`DAT_01c9c380`, a
            // constant of the game's memory) with a draw below 31 in
            // millions — no name of the original's table carries it.
            1 => random.below(0x1f) as i32 * 1_000_000,
            _ => random.below(0x65) as i32 * 1_000_000 + 100_000_000,
        };
        {
            let entry = &mut market[e..e + MARKET_ENTRY];
            put_word(entry, CLUB_WEEK, NEW_WEEK);
            put_long(entry, CLUB_SHARES, NO_SHARES);
            put_word(entry, CLUB_COUNTRY, -1);
            put_word(entry, CLUB_NUMBER, k as i16 + 1);
            put_long(entry, NAME_END, 1);
            let end = (wanted.len() + 1).min(MARKET_ENTRY);
            entry[..end].copy_from_slice(&[&wanted[..], &[0]].concat()[..end]);
            put_long(entry, CLUB_SHARES, shares);
            // The first price goes straight into the open week's place of
            // the ring, high and low with it (`FUN_00404ed8`);
            // `_DAT_008913a4` multiplies it by one.
            let price = (f64::from(random.below(0xc351)) / 100.0 + 200.0) as f32;
            let ring = PRICES_AT + usize::from(NEW_WEEK as u16) * 4;
            for at in [ring, HIGH, LOW] {
                entry[at..at + 4].copy_from_slice(&price.to_le_bytes());
            }
        }
        mean += row_price(records, &mut market, e, day, &names, random);
        count += 1;
    }
    if count > 0 {
        put_float(&mut market, PREV_MEAN, mean / count as f32);
        for _ in 0..PRICES {
            let mut week = 0.0f32;
            for k in 0..count {
                let e = MARKET_FIRST + k * MARKET_ENTRY;
                week += row_price(records, &mut market, e, day, &names, random);
            }
            let week = week / count as f32;
            let before = float(&market, PREV_MEAN);
            let change = ((week - before) * 100.0) / before;
            let run = float(&market, RUN) + change / 100.0;
            put_float(&mut market, RUN, run);
            put_float(&mut market, CHANGE_ON, change);
            put_float(&mut market, PREV_MEAN, week);
        }
        let run = float(&market, RUN);
        put_float(&mut market, INDEX, run * 5000.0 + 5000.0);
    }
    put_long(&mut market, MARKET_COUNT, count as i32);
    if let Some(block) = records.block_mut(&format!("{MARKET_AT:#x}")) {
        block.copy_from_slice(&market);
    }
}
const PREV_MEAN: usize = 0x8;
const CHANGE_ON: usize = 0xc;
const RUN: usize = 0x10;

fn put_float(b: &mut [u8], at: usize, v: f32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

/// The club's league: how many teams it has (`FUN_00692a88`) and how many
/// matchdays it has played where the game follows it (`FUN_00692744`).
fn league(records: &Records, team: &Team) -> (i16, i16) {
    if team.level() >= LOWEST_LEAGUE {
        return (DEFAULT_TEAMS, 0);
    }
    let country = usize::from(team.country());
    let index = records.countries.first_league(country) + i16::from(team.level());
    let block = records.block(&format!("{LEAGUE_AT:#x}[{index}]"));
    let teams = block.map_or(0, |b| word(b, LEAGUE_TEAMS));
    let played = records.globals.played(country as i16);
    (teams, if played { block.map_or(0, |b| word(b, LEAGUE_PLAYED)) } else { 0 })
}

/// Whether the club holds a European cup (`FUN_0042419d`).
fn holds_cup(records: &Records, team: &Team) -> bool {
    records.block(&format!("{HOLDERS_AT:#x}")).is_some_and(|b| {
        (0..HOLDERS).any(|k| {
            b.get(k * 4) == Some(&team.country())
                && word(b, k * 4 + 2) as u16 == u16::from(team.number())
        })
    })
}

/// What the human at this place among the managers holds of a club, by
/// the first of his places that holds it (`FUN_0040689a`), reading the
/// market as it now is.
fn held(records: &Records, market: &[u8], slot: i16, country: i16, number: i16) -> i32 {
    let Some(b) = records.block(&format!("{PORT_AT:#x}[{}]", slot - 1)) else {
        return 0;
    };
    (0..PORT_SLOTS)
        .find(|&k| {
            let at = PORT_ENTRY + k * PORT_STEP;
            let e = (long(b, at) as usize).wrapping_mul(MARKET_ENTRY).wrapping_add(MARKET_FIRST);
            b.get(PORT_USED + k).is_some_and(|&u| u != 0)
                && word(market, e.wrapping_add(CLUB_COUNTRY)) == country
                && word(market, e.wrapping_add(CLUB_NUMBER)) == number
        })
        .map_or(0, |k| long(b, PORT_SHARES + k * PORT_STEP))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_index_entry_walks_from_last_weeks_price() {
        let mut market = vec![0u8; MARKET_ENTRY];
        put_word(&mut market, CLUB_WEEK, 10);
        market[PRICES_AT + 10 * 4..PRICES_AT + 10 * 4 + 4]
            .copy_from_slice(&100.0f32.to_le_bytes());
        let mut random = Random::new(7);
        let price = index_price(&mut market, 0, 0, &mut random);
        assert_eq!(word(&market, CLUB_WEEK), 9);
        assert!((95.0..105.0).contains(&price), "{price}");
        // What `priced` will store for the new week, the next day walked on.
        market[PRICES_AT + 9 * 4..PRICES_AT + 9 * 4 + 4].copy_from_slice(&price.to_le_bytes());
        let walked = index_price(&mut market, 0, 4, &mut random);
        assert!((price - 6.0..price + 4.0).contains(&walked), "{walked}");
    }

    #[test]
    #[ignore = "reads the kept saves next to $ANSTOSS3_DIR"]
    fn a_vote_counts_the_waiting_shares_of_a_club() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let save = dir.parent().unwrap().join("saves-kept").join("A.an3");
        let mut records = Records::from_save(Save::read(&save).unwrap()).unwrap();
        let index = records.human_slot(0).unwrap();
        let (country, number) = (records.managers[index].country(), records.managers[index].team());
        let slot = place(&records, index);
        let set = |records: &mut Records, total: i32, held: [(i32, u8); 2]| {
            let b = records.block_mut("0x8be408").unwrap();
            put_long(b, MARKET_COUNT, 1);
            put_word(b, MARKET_FIRST + CLUB_COUNTRY, country as i16);
            put_word(b, MARKET_FIRST + CLUB_NUMBER, number as i16);
            put_long(b, MARKET_FIRST + CLUB_SHARES, total);
            let p = records.block_mut(&format!("0x8dc6e0[{}]", slot - 1)).unwrap();
            for (k, (shares, wait)) in held.into_iter().enumerate() {
                p[PORT_USED + k] = 1;
                put_long(p, PORT_ENTRY + k * PORT_STEP, 0);
                put_long(p, PORT_SHARES + k * PORT_STEP, shares);
                p[PORT_VOTE + k * PORT_STEP] = wait;
            }
        };
        let votes = |records: &Records| records.team(country, number).unwrap().votes();
        let waits = |records: &Records| {
            let p = records.block(&format!("0x8dc6e0[{}]", slot - 1)).unwrap();
            [p[PORT_VOTE], p[PORT_VOTE + PORT_STEP]]
        };
        // Two places waiting: 300 + 200 of 2000 shares, a quarter for.
        set(&mut records, 2000, [(300, 1), (200, 1)]);
        records.team_mut(country, number).unwrap().set_votes([7, 7]);
        vote(&mut records);
        assert_eq!(votes(&records), [25, 0]);
        assert_eq!(waits(&records), [0, 0]);
        // Only the waiting place counts, and more than half is all.
        set(&mut records, 2000, [(1001, 1), (900, 0)]);
        vote(&mut records);
        assert_eq!(votes(&records), [100, 0]);
        // A club of no more than a thousand shares keeps its last vote,
        // though the wait is cleared all the same.
        set(&mut records, 1000, [(400, 1), (0, 0)]);
        vote(&mut records);
        assert_eq!(votes(&records), [100, 0]);
        assert_eq!(waits(&records), [0, 0]);
    }
}
