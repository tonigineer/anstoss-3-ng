//! The crowd and the gate of a friendly or a tournament game
//! (`FUN_0078b26c`'s branch for the day's own matches, `FUN_0078d3e0`).
//!
//! The demand is the international tie's ([`crate::presale`]'s
//! `FUN_0078f369`) by the game's round index — 5 a friendly, a Turnier's
//! 3, 2 and 5 —, then a tenth off and halved, cut by how the two clubs'
//! classes stand to each other and by how many such games a human club has
//! played this season (its extra block's +0x2564, which each one counts).
//! The ground's stadium takes it: the visitors first into their own stand,
//! then everybody by four price shares over the stands in their orders —
//! the shares paired the wrong way round, an original quirk —; the gate at
//! fixed prices (10, 15, 20, 150; roofed places a third more), 0.86 of it,
//! 3 % off what pays. Afterwards the pitch wears, by a chance at the home
//! club's own ground.

use crate::presale::PresaleTables;
use crate::random::Random;
use crate::records::Records;
use crate::stadium::Sub;
use crate::tickets::weight;

/// The demand's factor by the round index (`0x8ad354`), the cut by the
/// season's such games so far (`0x8ad334`, seven at most).
const ROUND_FACTOR: [f32; 8] = [0.5, 0.55, 0.6, 0.7, 0.8, 0.9, 1.0, 1.0];
const PLAYED_CUT: [i32; 8] = [0, 5, 10, 20, 35, 50, 70, 80];
const PLAYED_MOST: i16 = 7;
/// The club's league prices (club +0x49c standing, +0x4a6 the seats of
/// the side stands), at least 5; the prices the gate is reckoned at:
/// standing, side seats, main seats, boxes; its class of the shares; the
/// part kept of what pays.
const PRICES: [usize; 2] = [0x49c, 0x4a6];
const LEAST_PRICE: i16 = 5;
const GATE_PRICES: [i16; 4] = [10, 15, 20, 150];
const REFERENCES: [i32; 4] = [15, 20, 40, 150];
const CLASS: i32 = 6;
const DEDUCTION: f64 = 3.0;
const GATE_PART: f32 = 0.86;
/// What a stand of a condition holds of its places (`0x8a7b50`).
const USABLE: [i32; 8] = [100, 100, 100, 100, 80, 50, 20, 0];
/// The sub-record's fields: the visitors' cap (a tenth of it), the boxes,
/// standing and seats of each stand, their age and building weeks, the
/// roofs, the pitch's wear, the home and the away fans' stands.
const AWAY_CAP: usize = 0x38;
const VIP: usize = 0x40;
const STANDING: usize = 0x48;
const SEATS: usize = 0x50;
const AGE: usize = 0x58;
const WEEKS: usize = 0x5c;
const ROOFS: usize = 0x60;
const WEAR: usize = 0x68;
const HOME_STAND: usize = 0x6a;
const AWAY_STAND: usize = 0x6b;
/// The stands' orders (`0x8ad388`, `0x8ad3b4`, `0x8ad400`): the visitors'
/// seat seekers, their boxes and standing, everybody's by their stand.
const AWAY_SEATS: [[usize; 4]; 4] = [[2, 3, 1, 0], [2, 3, 0, 1], [2, 1, 0, 3], [3, 1, 0, 2]];
const AWAY_OTHERS: [[usize; 4]; 4] = [[2, 0, 1, 3], [2, 0, 1, 3], [2, 0, 1, 3], [3, 0, 1, 2]];
const BY_STAND: [[usize; 4]; 4] = [[0, 1, 3, 2], [0, 1, 3, 2], [2, 0, 1, 3], [3, 0, 1, 2]];
const MAIN: [usize; 2] = [0, 1];
const SIDES: [usize; 2] = [2, 3];
/// The pitch's chance to wear by the home club's index (club +0x301);
/// past the table always.
const WEAR_INDEX: usize = 0x301;
const WEAR_CHANCES: [i32; 3] = [72, 59, 50];
/// The sub-record the stadium is stored back to (team +0x879).
const STORED_SUB: usize = 0x879;

/// What the ground took: everybody it holds, the paying gate, the gate
/// before the deduction, the home fans and the visitors, by stand.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Gate {
    pub crowd: i32,
    pub gate: i32,
    pub gross: i32,
    pub home: i32,
    pub away: i32,
    pub stands: [i32; 4],
}

fn word(sub: &Sub, at: usize) -> i32 {
    i32::from(u16::from_le_bytes([sub.0[at], sub.0[at + 1]]))
}

/// The ground's stands as the fill reads them.
struct Ground<'a> {
    sub: &'a Sub,
    m: [[i32; 4]; 8],
}

impl Ground<'_> {
    fn open(&self, s: usize) -> bool {
        self.sub.0[WEEKS + s] == 0
    }

    fn usable(&self, at: usize, s: usize) -> i32 {
        let condition = usize::from(self.sub.0[AGE + s] / 5).min(6);
        word(self.sub, at + 2 * s) * USABLE[condition] / 100
    }

    fn roofed(&self, s: usize) -> bool {
        word(self.sub, ROOFS) & (1 << s) != 0
    }

    /// A cell filled from `p` up to its places (`FUN_0078cb50` and kin);
    /// the paying rows counted with it.
    fn fill(&mut self, s: usize, col: usize, cap: i32, p: &mut i32) {
        if !self.open(s) {
            return;
        }
        let cell = self.m[s][col];
        if cell + *p < cap {
            self.m[s][col] = cell + *p;
            self.m[s + 4][col] += *p;
            *p = 0;
        } else {
            self.m[s + 4][col] += cap - cell;
            *p -= cap - cell;
            self.m[s][col] = cap;
        }
    }

    fn boxes(&mut self, s: usize, p: &mut i32) {
        let cap = self.usable(VIP, s);
        self.fill(s, 3, cap, p);
    }

    fn seats(&mut self, s: usize, p: &mut i32) {
        let cap = self.usable(SEATS, s);
        self.fill(s, seat_column(s), cap, p);
    }

    fn standing(&mut self, s: usize, p: &mut i32) {
        let cap = self.usable(STANDING, s);
        self.fill(s, 0, cap, p);
    }

    /// A side's fans over the stands from their own (`FUN_0078ceae`):
    /// `p[3]` to the boxes, `p[2]` to the main seats, `p[1]` to the side
    /// seats, `p[0]` to the standing, the rest wherever there is room.
    fn place(&mut self, p: &mut [i32; 4], side: usize) {
        let order = BY_STAND[side];
        for &s in &order {
            self.boxes(s, &mut p[3]);
        }
        for s in MAIN {
            self.seats(s, &mut p[2]);
        }
        for s in SIDES {
            self.seats(s, &mut p[1]);
        }
        for &s in &order {
            self.standing(s, &mut p[0]);
        }
        for j in (0..4).rev() {
            if p[j] != 0 {
                for &s in &order {
                    self.standing(s, &mut p[j]);
                }
                for s in MAIN {
                    self.seats(s, &mut p[j]);
                }
                for s in SIDES {
                    self.seats(s, &mut p[j]);
                }
                for &s in &order {
                    self.boxes(s, &mut p[j]);
                }
            }
        }
    }

    /// The gate of four rows (`FUN_00719c4c`): each cell by its price, a
    /// roofed place not in a box a third more, summed in f32, 0.86 of it.
    fn gate(&self, rows: &[[i32; 4]]) -> i32 {
        let mut acc = 0f32;
        for (s, row) in rows.iter().enumerate() {
            for (c, &n) in row.iter().enumerate() {
                let roof = if self.roofed(s) && c < 3 { 4.0 } else { 3.0 };
                let x = f64::from(n) * 1.0 * f64::from(GATE_PRICES[c]) / 3.0 * f64::from(roof as f32);
                acc = (x + f64::from(acc)) as f32;
            }
        }
        (f64::from(acc) * f64::from(GATE_PART)) as i32
    }
}

/// The column a stand's seats count in: the main stands', the sides'.
fn seat_column(s: usize) -> usize {
    if s <= 1 { 2 } else { 1 }
}

/// The ground takes the home fans `h` and the visitors `a` (`FUN_0078d3e0`
/// with no season tickets and no free tickets, the day's mark plain).
pub(crate) fn fill(sub: &Sub, h: i32, a: i32) -> Gate {
    let mut g = Ground { sub, m: [[0; 4]; 8] };
    let home_stand = usize::from(sub.0[HOME_STAND]).min(3);
    let mut away_stand = usize::from(sub.0[AWAY_STAND]).min(3);
    if home_stand == away_stand {
        away_stand = if home_stand & 2 == 0 { 1 - home_stand } else { 5 - home_stand };
    }
    let pct: f32 = if h == 0 {
        0.0
    } else if a == 0 {
        100.0
    } else {
        (f64::from(h.wrapping_mul(100)) / f64::from(h + a)) as f32
    };
    // The four shares' weights by the gate's prices.
    let c = f64::from(2 * CLASS);
    let (wv, wm, ws) = ((c + 3.0) as f32, (c + 30.0) as f32, (c + 10.0) as f32);
    let wt = (((100.0 - f64::from(wv)) - f64::from(ws)) - f64::from(wm)) as f32;
    let w = |price: usize, base: f32| (f64::from(weight(GATE_PRICES[price], REFERENCES[price])) * f64::from(base)) as f32;
    let (wv, wm, ws, wt) = (w(3, wv), w(2, wm), w(1, ws), w(0, wt));
    // The visitors first, into their own stand, then on.
    let cap = i32::from_le_bytes(sub.0[AWAY_CAP..AWAY_CAP + 4].try_into().unwrap_or_default());
    let c8 = a.min(cap / 10);
    let mut seekers = (c8 << 1) / 5;
    let mut standers = c8 - seekers;
    let mut away = c8;
    let e = away_stand;
    if g.open(e) {
        let col = seat_column(e);
        let seats = g.usable(SEATS, e);
        let n = seekers.min(seats);
        g.m[e][col] = n;
        g.m[e + 4][col] = n;
        seekers -= n;
        let stand = g.usable(STANDING, e);
        let n = standers.min(stand);
        g.m[e][0] = n;
        g.m[e + 4][0] = n;
        standers -= n;
    }
    for s in AWAY_SEATS[e] {
        g.seats(s, &mut seekers);
    }
    for s in MAIN {
        g.standing(s, &mut seekers);
    }
    for s in AWAY_OTHERS[e] {
        g.boxes(s, &mut seekers);
    }
    for s in AWAY_OTHERS[e] {
        g.standing(s, &mut standers);
    }
    for s in MAIN {
        g.seats(s, &mut standers);
    }
    for s in AWAY_OTHERS[e] {
        g.boxes(s, &mut standers);
    }
    away -= standers + seekers;
    // The places left, shared between the two sides.
    let free: i32 = (0..4)
        .filter(|&s| g.open(s))
        .map(|s| g.usable(VIP, s) + g.usable(SEATS, s) + g.usable(STANDING, s) - g.m[s].iter().sum::<i32>())
        .sum();
    let (rh, ra) = (h.max(0), (a - away).max(0));
    let mut ec = (f64::from(free) * f64::from(pct) / 100.0) as i32;
    let mut aw = free - ec;
    if ec > rh {
        aw += ec - rh;
        ec = rh;
    }
    if aw > ra {
        ec += aw - ra;
        aw = ra;
    }
    if ec > rh {
        ec = rh;
    }
    let home = ec;
    away += aw;
    let sum = (((f64::from(wv) + f64::from(wm)) + f64::from(ws)) + f64::from(wt)) as f32;
    let split = |n: i32| {
        let part = |x: f32| (f64::from(n) * f64::from(x) / f64::from(sum)) as i32;
        let (p0, p1, p2) = (part(wv), part(wm), part(ws));
        [p0, p1, p2, n - p0 - p1 - p2]
    };
    let (mut hp, mut ap) = (split(ec), split(aw));
    g.place(&mut hp, home_stand);
    g.place(&mut ap, away_stand);
    let gross = g.gate(&g.m[0..4]);
    let paying = g.gate(&g.m[4..8]);
    let stands: [i32; 4] = std::array::from_fn(|s| g.m[s].iter().sum());
    Gate {
        crowd: stands.iter().sum(),
        gate: (f64::from(paying) * (100.0 - DEDUCTION) / 100.0) as i32,
        gross,
        home,
        away,
        stands,
    }
}

/// A club's class for the demand's cut (`FUN_006755b8` with
/// `FUN_0063e2df`): the main country's league, up to the third (England's
/// the fourth); every other country's club 0.
fn class(records: &Records, calendar: &crate::calendar::Calendar, (country, number): (u8, i16)) -> i32 {
    const ENGLAND: i16 = 16;
    let level = i32::from(crate::friendly::division(records, calendar, (country, number)));
    let main = records.globals.main_country();
    if i16::from(country) == main {
        level.min(if main == ENGLAND { 3 } else { 2 })
    } else {
        0
    }
}

/// A human club's games of the season outside the competitions
/// (`FUN_0069d5a7`): its extra block's +0x2564; a computer club none.
fn played(records: &Records, (country, number): (u8, i16)) -> i16 {
    let slot = u16::try_from(number).ok().and_then(|n| records.team(country.into(), n)).map_or(0, |t| t.human());
    usize::from(slot).checked_sub(1).and_then(|s| records.extras.get(s)).map_or(0, |e| e.together())
}

fn count(records: &mut Records, (country, number): (u8, i16)) {
    let slot = u16::try_from(number).ok().and_then(|n| records.team(country.into(), n)).map_or(0, |t| t.human());
    if let Some(extra) = usize::from(slot).checked_sub(1).and_then(|s| records.extras.get_mut(s)) {
        extra.set_together(extra.together().wrapping_add(1));
    }
}

/// The crowd and the gate of a game of the day's own list: the demand by
/// its round index cut as above, each human club counting the game, the
/// ground's first stadium filled, then its pitch worn — by the home club's
/// chance at its own ground (one draw), always at another's — and the
/// stadium stored back where the club keeps it.
pub(crate) fn gate(
    records: &mut Records,
    calendar: &crate::calendar::Calendar,
    tables: &PresaleTables,
    (home, away, ground): ((u8, i16), (u8, i16), (u8, i16)),
    round: i16,
    random: &mut Random,
) -> Gate {
    fn team(records: &Records, c: (u8, i16)) -> Option<&crate::records::Team> {
        u16::try_from(c.1).ok().and_then(|n| records.team(c.0.into(), n))
    }
    let prices = PRICES.map(|at| {
        team(records, home).map_or(0, |t| t.get::<i16>(at + crate::finances::CLUB_TO_TEAM)).max(LEAST_PRICE)
    });
    let (h, a) = crate::presale::match_demand(records, home, away, ground, prices, round, tables).unwrap_or((0, 0));
    let factor = f64::from(ROUND_FACTOR[round.clamp(0, 7) as usize]);
    let (mut h, mut a) = ((f64::from(h) * factor) as i32 / 2, (f64::from(a) * factor) as i32 / 2);
    let (lh, la) = (class(records, calendar, home), class(records, calendar, away));
    let (mh, ma) = (played(records, home).min(PLAYED_MOST), played(records, away).min(PLAYED_MOST));
    count(records, home);
    count(records, away);
    (h, a) = match lh - la {
        -3 | -2 => (h / 10, a / 10),
        -1 => (h / 7, a / 7),
        0 => (h / 4, a / 4),
        1 => (h * 3 / 10, a * 3 / 10),
        _ => (h * 4 / 10, a * 4 / 10),
    };
    h -= h * PLAYED_CUT[mh.max(0) as usize] / 100;
    if PLAYED_CUT[ma.max(0) as usize] > 0 {
        a -= a * PLAYED_CUT[ma.max(0) as usize] / 100;
    }
    let Ok(number) = u16::try_from(ground.1) else { return Gate::default() };
    let Some(mut subs) = crate::stadium::subs(records, ground.0, number) else { return Gate::default() };
    let taken = fill(&subs[0], h, a);
    let mut sub = subs[0].clone();
    let worn = if ground == home {
        let index = team(records, home).map_or(0, |t| t.get::<u8>(WEAR_INDEX + crate::finances::CLUB_TO_TEAM));
        let chance = WEAR_CHANCES.get(usize::from(index)).copied().unwrap_or(i32::MAX);
        (random.below(100) as i32) < chance
    } else {
        true
    };
    if worn {
        sub.0[WEAR] = sub.0[WEAR].wrapping_add(1);
    }
    let stored = team(records, ground).map_or(0, |t| usize::from(t.get::<u8>(STORED_SUB)));
    if let Some(at) = subs.get_mut(stored) {
        *at = sub;
        crate::stadium::store(records, ground.0, number, &subs);
    }
    taken
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(cap: i32, vip: [u16; 4], standing: [u16; 4], seats: [u16; 4], age: [u8; 4], roofs: u16, stands: (u8, u8)) -> Sub {
        let mut b = vec![0u8; 0xd0];
        b[AWAY_CAP..AWAY_CAP + 4].copy_from_slice(&cap.to_le_bytes());
        for s in 0..4 {
            b[VIP + 2 * s..VIP + 2 * s + 2].copy_from_slice(&vip[s].to_le_bytes());
            b[STANDING + 2 * s..STANDING + 2 * s + 2].copy_from_slice(&standing[s].to_le_bytes());
            b[SEATS + 2 * s..SEATS + 2 * s + 2].copy_from_slice(&seats[s].to_le_bytes());
            b[AGE + s] = age[s];
        }
        b[ROOFS..ROOFS + 2].copy_from_slice(&roofs.to_le_bytes());
        (b[HOME_STAND], b[AWAY_STAND]) = stands;
        Sub(b)
    }

    /// Save A's Alten Försterei (sub 0, every stand roofed), as the spec
    /// works it through the original's steps: 15 000 home fans and 3 000
    /// visitors all in, the gate 250 738 before and 243 215 after the
    /// deduction; over capacity, 25 000 and 4 000 fill it to 22 012.
    #[test]
    fn the_alten_foersterei_takes_a_friendly() {
        let ground = sub(22012, [27, 0, 0, 0], [0, 6133, 6131, 6131], [3590, 0, 0, 0], [5; 4], 0xf, (0, 1));
        assert_eq!(fill(&ground, 15000, 3000), Gate {
            crowd: 18000,
            gate: 243215,
            gross: 250738,
            home: 15000,
            away: 3000,
            stands: [3617, 6133, 2119, 6131],
        });
        let full = fill(&ground, 25000, 4000);
        assert_eq!((full.crowd, full.home, full.away, full.gross, full.gate), (22012, 18012, 4000, 296743, 287840));
    }

    /// The spec's second ground: side stands aged to 80 % and 20 %, only
    /// the main stands roofed, the fans in the sides; with the right stand
    /// closed for building it holds 1 200 less.
    #[test]
    fn a_worn_ground_holds_less() {
        let mut ground =
            sub(30000, [500, 100, 0, 0], [2000, 4000, 6000, 6000], [8000, 3400, 0, 0], [0, 0, 20, 30], 0b11, (2, 3));
        let g = fill(&ground, 20000, 5000);
        assert_eq!((g.crowd, g.home, g.away, g.gross, g.gate), (24000, 19000, 5000, 459240, 445462));
        ground.0[WEEKS + 3] = 5;
        let g = fill(&ground, 20000, 5000);
        assert_eq!((g.home, g.away, g.gross, g.gate), (17800, 5000, 448920, 435452));
    }
}
