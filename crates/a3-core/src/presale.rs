//! The ticket office's advance sales (`0x1c5c468`, thirty records of 0x24
//! bytes; KALKULATION.md in the RE repo): at the week's end each human club
//! opens records for its league matches of the next four weeks, at home and
//! away (`FUN_0077ca2b`), a club's next cup tie opens its own record where
//! the club notes that match (`FUN_0062bfc9`'s `FUN_0077c4ee`), and every
//! day the records of the next week sell on (`FUN_0077c8b5`); a record whose
//! day has passed is let go. How many sell comes from the attendance model
//! (`FUN_0078e5e6`) and the luck of the game's generator, so the sales draw
//! in the original's order.
//!
//! The model reckons in single precision as the original's x87 does with
//! MSVC's 53-bit precision: every statement in double, rounded to a float
//! where the code stores one.

use std::io;

use a3_files::exe::Exe;

use crate::calendar::Calendar;
use crate::finances::CLUB_TO_TEAM;
use crate::random::Random;
use crate::records::{Records, Team};
use crate::tickets::{REFERENCE_PRICES, ftol, weight};

/// The sales records (`0x1c5c468`): home and away club, the day, the kind
/// (2 a league match, 3 and 4 a cup tie, 5 to 10 an international one),
/// the tickets sold, whether either order of the clubs matches, the four
/// prices (standing, behind the goal, side, boxes).
const SALES_AT: &str = "0x1c5c468";
const SALES: usize = 30;
const SALE: usize = 0x24;
const SALE_ISSUER: usize = 8;
const SALE_DAY: usize = 0xc;
const SALE_KIND: usize = 0xe;
const SALE_EXTRA: usize = 0x10;
const SALE_SOLD: usize = 0x14;
const SALE_OPEN: usize = 0x18;
const SALE_PRICES: usize = 0x1a;
const LEAGUE: i16 = 2;
/// A cup tie's kind (`club +0xb4c`'s 3, the record's `FUN_0077c4ee` writes).
const CUP_TIE: i16 = 3;
/// A supercup tie's kind (`club +0xb4c`'s 4, the same record writer with the
/// cup's).
const SUPERCUP_TIE: i16 = 4;
/// The kinds an international tie's record takes — `club +0xb4c`'s 5 for a
/// champion's cup tie, 7 for a UEFA cup one and 8 for an Intertoto one,
/// written by the same `FUN_0077c379`. The callers hand the kind along
/// from the club's word, so only the tests name them.
#[cfg(test)]
const INTERNATIONAL: i16 = 5;
#[cfg(test)]
const UEFA: i16 = 7;
#[cfg(test)]
const INTERTOTO: i16 = 8;
/// Sales start after the season's sixth week, and look four weeks ahead,
/// a week for the daily sales.
const FIRST_SALES_DAY: i32 = 41;
const AHEAD: i32 = 28;
const WEEK: i32 = 7;
/// The days of a season the home matches are looked for in, and how many
/// are kept (`FUN_0062af24`).
const FIRST_DAY: usize = 0xc;
const LAST_DAY: usize = 0x16c;
const MOST_MATCHES: usize = 46;
const LAST_MATCHDAY_CODE: u32 = 0x3c;

/// The exe's tables of the sales and their model.
pub struct PresaleTables {
    /// A match's mark makes its prices (`DAT_008acc2c`).
    mark_factors: [f32; 4],
    /// The percent of the demand sold a day ahead, by the days left
    /// (`DAT_008ad304`).
    by_days: Vec<i16>,
    /// A country's sales in percent, by its index (`DAT_008ad314`).
    by_country: Vec<i16>,
    /// The demand by the fans' class: home, away above one, away below one
    /// and its base (`DAT_008ad420`, rows of four).
    demand: Vec<[i32; 4]>,
    /// What a place among the first five makes of the demand
    /// (`DAT_008ad370`).
    places: [f32; 6],
    /// The places a league's size expects of a target (`DAT_008ac920`,
    /// eight rows of 0x18).
    targets: Vec<u8>,
    /// A league's pull in percent (`DAT_008ad2d4`), where its country's
    /// leagues start in it (`DAT_0089dd50`), and whether a country has them
    /// (`DAT_0089ddd8`).
    pull: Vec<i16>,
    pull_start: Vec<i16>,
    pull_own: Vec<u8>,
    /// Whether a country has a sales index, and where in the globals' grid
    /// it stands (`DAT_0089e0b8`, `DAT_008ace90`).
    indexed: Vec<u8>,
    index_at: Vec<u8>,
    /// The part of a league match's demand that comes, in percent, by the
    /// country's index (`0x8ad324`).
    on_the_day: Vec<i16>,
}

impl PresaleTables {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        let f32s = |at: u32, n: usize| -> io::Result<Vec<f32>> {
            Ok(exe.u32s(at, n)?.into_iter().map(f32::from_bits).collect())
        };
        let i16s = |at: u32, n: usize| -> io::Result<Vec<i16>> {
            Ok(exe.u16s(at, n)?.into_iter().map(|v| v as i16).collect())
        };
        let marks = f32s(0x8acc2c, 4)?;
        let places = f32s(0x8ad370, 6)?;
        let rows = exe.u32s(0x8ad420, 9 * 4)?;
        Ok(Self {
            mark_factors: std::array::from_fn(|i| marks[i]),
            by_days: i16s(0x8ad304, 7)?,
            by_country: i16s(0x8ad314, 64)?,
            demand: rows.chunks(4).map(|r| std::array::from_fn(|i| r[i] as i32)).collect(),
            places: std::array::from_fn(|i| places[i]),
            targets: exe.bytes(0x8ac920, 8 * 0x18)?.to_vec(),
            pull: i16s(0x8ad2d4, 48)?,
            pull_start: i16s(0x89dd50, 16)?,
            pull_own: exe.bytes(0x89ddd8, 128)?.to_vec(),
            indexed: exe.bytes(0x89e0b8, 128)?.to_vec(),
            index_at: exe.bytes(0x8ace90, 256)?.to_vec(),
            on_the_day: i16s(0x8ad324, 8)?,
        })
    }

    /// The places expected of a target in a league of `teams`
    /// (`DAT_008ac920`), from place 1 at index 0.
    fn target(&self, teams: i16, index: i16) -> i16 {
        let row = ((teams - 10) / 2).clamp(0, 7) as usize;
        usize::try_from(index).ok().and_then(|i| self.targets.get(row * 0x18 + i)).map_or(0, |&b| i16::from(b))
    }
}

/// A club as the sales see it: its team record and the club object's
/// fields the model reads, by where the object takes them from
/// (`FUN_0063b75a`).
struct Side<'a> {
    records: &'a Records,
    team: &'a Team,
    country: u8,
    number: u8,
}

impl<'a> Side<'a> {
    fn of(records: &'a Records, country: u8, number: i16) -> Option<Self> {
        let index = records.team_index(country.into(), u16::try_from(number).ok()?)?;
        let team = records.teams.get(index)?;
        Some(Self { records, team, country, number: number as u8 })
    }

    fn level(&self) -> u8 {
        self.team.level()
    }

    /// Its place in the league's fixtures (club +0x106, team +0x12f).
    fn slot(&self) -> u8 {
        self.team.get::<u8>(0x12f)
    }

    /// Its place in the table (club +0x109, team +0x132), and the place it
    /// was set as a target (club +0x10c, team +0x135).
    fn place(&self) -> i16 {
        i16::from(self.team.get::<u8>(0x132))
    }

    fn target(&self) -> i16 {
        i16::from(self.team.get::<i8>(0x135))
    }

    /// The fans' class and kind (club +0xec, +0xed: team +0x11c, +0x11d).
    fn fans(&self) -> (u8, u8) {
        (self.team.get::<u8>(0x11c), self.team.get::<u8>(0x11d))
    }

    /// The team's strength (club +0x9c, team +0xc0).
    fn strength(&self) -> i16 {
        self.team.get::<i16>(0xc0)
    }

    /// Where it lies (club +0x11da, +0x11dc: team +0x30e, +0x310).
    fn at(&self) -> (i32, i32) {
        (i32::from(self.team.get::<i16>(0x30e)), i32::from(self.team.get::<i16>(0x310)))
    }

    /// Its class, which shifts the demand to the dearer seats (club
    /// +0x11e0, team +0x875).
    fn class(&self) -> i32 {
        i32::from(self.team.get::<u8>(0x875))
    }

    /// Its loyal fans, who come whatever (club +0x9ea, team +0x25e).
    fn loyal(&self) -> i32 {
        i32::from(self.team.get::<i16>(0x25e))
    }

    /// The season's achievements (team +0x110: `FUN_0069a161` and kin).
    fn achieved(&self, bit: u16) -> bool {
        self.team.get::<u16>(0x110) & bit != 0
    }

    /// The club it shares a derby with (club +0xf4, team +0x122).
    fn derby(&self) -> i32 {
        i32::from(self.team.get::<i8>(0x122))
    }

    /// The season tickets sold (club +0x22c) and the prices of the four
    /// classes, standing first (club +0x49c, +0x4a6, +0x4b0, +0x4ba).
    fn season_tickets(&self) -> i32 {
        self.team.get::<i32>(0x22c + CLUB_TO_TEAM)
    }

    fn prices(&self) -> [i16; 4] {
        [0x49c, 0x4a6, 0x4b0, 0x4ba].map(|c| self.team.get::<i16>(c + CLUB_TO_TEAM))
    }

    /// The prices a cup tie sells at, the cup's own column of the same
    /// four stands (club +0x49e, +0x4a8, +0x4b2, +0x4bc — `FUN_0077c4ee`).
    fn cup_prices(&self) -> [i16; 4] {
        [0x49e, 0x4a8, 0x4b2, 0x4bc].map(|c| self.team.get::<i16>(c + CLUB_TO_TEAM))
    }

    /// The prices an international tie sells at, the international column
    /// of the same four stands (club +0x4a0, +0x4aa, +0x4b4, +0x4be —
    /// `FUN_0077c379`).
    fn international_prices(&self) -> [i16; 4] {
        [0x4a0, 0x4aa, 0x4b4, 0x4be].map(|c| self.team.get::<i16>(c + CLUB_TO_TEAM))
    }

    /// The league block it plays in (0x1a09d48[first league + level]).
    fn league(&self) -> Option<&'a [u8]> {
        let li = self.records.countries.first_league(usize::from(self.country)) + i16::from(self.level());
        self.records.block(&format!("0x1a09d48[{li}]"))
    }

    fn league_word(&self, at: usize) -> i16 {
        self.league().and_then(|b| b.get(at..at + 2)).map_or(0, |w| i16::from_le_bytes([w[0], w[1]]))
    }

    /// Its league's teams, eighteen below the sixth league
    /// (`FUN_00692a88`), and its matchdays played, ten below the tenth
    /// (`FUN_00635ba3`).
    fn teams(&self) -> i16 {
        if self.level() < 6 { self.league_word(0x30) } else { 18 }
    }

    fn played(&self) -> i16 {
        if self.level() < 10 { self.league_word(0x46) } else { 10 }
    }

    /// Whether the league stands one matchday before its last
    /// (`FUN_00635be1`).
    fn second_last(&self) -> bool {
        self.level() < 10 && i32::from(self.league_word(0x46)) == i32::from(self.league_word(0x30)) * 2 - 3
    }

    /// How far it stands above its target (`FUN_00775ed8` by
    /// `FUN_00775e0a`), else below as a negative (`FUN_00775fa0` by
    /// `FUN_00775d3c`).
    fn above(&self, t: &PresaleTables) -> i32 {
        let (teams, place, target) = (self.teams(), self.place(), self.target());
        if place < 1 || teams < place {
            return 0;
        }
        if t.target(teams, place - 1) < target {
            let mut k = place;
            while k < teams && t.target(teams, k) < target {
                k += 1;
            }
            i32::from(k - place)
        } else {
            0
        }
    }

    fn below(&self, t: &PresaleTables) -> i32 {
        let (teams, place, target) = (self.teams(), self.place(), self.target());
        if place < 1 || teams < place {
            return 0;
        }
        if target < t.target(teams, place - 1) {
            let mut k = place - 2;
            while k >= 0 && target < t.target(teams, k) {
                k -= 1;
            }
            i32::from(place - k - 1)
        } else {
            0
        }
    }

    fn form(&self, t: &PresaleTables) -> i32 {
        let above = self.above(t);
        if (above as i16) < 1 { -self.below(t) } else { above }
    }

    /// Its cup tie as the team record keeps it (team +0x312..+0x320):
    /// out of it (-1), through (1), or neither — not in one, or level
    /// (`FUN_0063fab8`, `FUN_0063fcb8`, `FUN_0063fb9c`).
    fn cup(&self) -> i8 {
        let t = self.team;
        if t.get::<u8>(0x312) == 0xff {
            return 0;
        }
        let home = t.get::<u8>(0x314) == self.country && t.get::<u16>(0x316) == u16::from(self.number);
        let (a, b) = if t.get::<u8>(0x320) & 0x30 == 0 { (0x31c, 0x31d) } else { (0x31e, 0x31f) };
        let (own, other) = if home { (t.get::<u8>(a), t.get::<u8>(b)) } else { (t.get::<u8>(b), t.get::<u8>(a)) };
        match own.cmp(&other) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Greater => 1,
            std::cmp::Ordering::Equal => 0,
        }
    }
}

/// The pull of a club's league in percent (`FUN_0078e893`).
fn pull(side: &Side, t: &PresaleTables) -> i32 {
    let (records, country, level) = (side.records, usize::from(side.country), side.level());
    if t.pull_own.get(country).copied().unwrap_or(0) == 0 {
        if side.country == records.countries.get::<u8>(0xae62f4) {
            if level < 10 { 60 } else { 15 }
        } else if level < 10 {
            // Where the country stands in the five-year ranking
            // (`FUN_00424050`).
            let rank = (0..15)
                .position(|k| records.globals.get::<i16>(0xb5d1a8 + k * 0x24) == i16::from(side.country))
                .unwrap_or(15);
            match rank {
                0..3 => 70,
                3..9 => 40,
                _ => 15,
            }
        } else {
            15
        }
    } else if level < 10 {
        let system = usize::from(records.countries.get::<u8>(0xae5d0c + country));
        let start = t.pull_start.get(system).copied().unwrap_or(0);
        let at = usize::try_from(i32::from(start) + i32::from(level)).unwrap_or(0);
        i32::from(t.pull.get(at).copied().unwrap_or(15))
    } else {
        15
    }
}

/// A country's sales index (`FUN_00783a13`): its place in the globals'
/// grid at `0xb5d92a`, forty to a row.
fn country_index(records: &Records, country: u8, t: &PresaleTables) -> usize {
    let c = usize::from(country);
    if t.indexed.get(c).copied().unwrap_or(0) == 0 {
        return 0;
    }
    let row = usize::from(t.index_at.get(c * 2).copied().unwrap_or(0));
    let col = usize::from(t.index_at.get(c * 2 + 1).copied().unwrap_or(0));
    usize::from(records.globals.get::<u8>(0xb5d92a + row * 0x28 + col))
}

/// How the distance between the clubs carries the away fans
/// (`FUN_0078e97c`): more the nearer, little from abroad.
fn distance(home: &Side, away: &Side) -> f32 {
    if home.country != away.country {
        return 0.125;
    }
    let ((x1, y1), (x2, y2)) = (home.at(), away.at());
    let d = ftol(f64::from((x1 - x2) * (x1 - x2) + (y1 - y2) * (y1 - y2)).sqrt() * 1.0);
    match d {
        ..3 => 3.0,
        3..10 => 2.5,
        10..25 => 2.0,
        25..50 => 0.8,
        50..100 => 0.4,
        100..250 => 0.2,
        _ => 0.125,
    }
}

/// The fans' class a club's demand is read by (`FUN_0078fa0d`): its
/// class, moved up by its kind when it does badly, plays a weaker side or
/// always.
fn fan_class(side: &Side, other: &Side, t: &PresaleTables) -> usize {
    let (class, kind) = side.fans();
    let c = u32::from(class);
    let v = match kind {
        2 if (side.form(t) as i16) < -2 => {
            if class < 6 { c + 2 } else { 7 }
        }
        3 if i32::from(other.strength()) + 0x14 < i32::from(side.strength()) => {
            if class < 5 { c + 3 } else { 7 }
        }
        6 => {
            if class < 6 { c + 2 } else { 7 }
        }
        7 => c,
        _ => c + 1,
    };
    v as usize
}

/// A club's own weight on its demand and the other's (`FUN_0078ebe8`):
/// its form, its cup tie, a derby, the title's last matchday, its place.
fn boost(side: &Side, other: &Side, own: &mut f32, theirs: &mut f32, form: i32, last: bool, t: &PresaleTables) {
    let f: f32 = match form as i16 {
        ..-3 => 0.6,
        -3 => 0.85,
        -2 => 0.9,
        -1 => 0.95,
        0 => 1.0,
        1 => 1.05,
        2 => 1.1,
        3 => 1.15,
        4 => 1.3,
        _ => 1.5,
    };
    *own *= f;
    match side.cup() {
        -1 => *own *= 0.85,
        1 => *own *= 1.1,
        _ => {}
    }
    if side.country == other.country && side.derby() == i32::from(other.number) {
        *own *= 1.25;
    }
    let place = side.place();
    if place == 1 && last {
        *own *= 4.0;
        *theirs *= 1.2;
    }
    if place != 0 && place < 6 {
        let p = t.places[place as usize];
        *own *= p;
        *theirs *= p;
    }
}

/// The weights of both clubs' standing on the demand (`FUN_0078ea23`).
fn standing(home: &Side, away: &Side, h: &mut f32, a: &mut f32, t: &PresaleTables) {
    let played = away.played();
    let last = away.second_last();
    let (mut away_form, mut home_form) = (1, 1);
    if 3 < played {
        away_form = away.above(t);
        home_form = home.above(t);
    }
    boost(away, home, a, h, away_form, last, t);
    boost(home, away, h, a, home_form, last, t);
    if home.achieved(0x1) {
        *h *= 1.3;
        *a *= 1.1;
    } else if home.achieved(0x2) {
        *h *= 1.1;
    } else if home.achieved(0x10) || home.achieved(0x4000) {
        *h *= 1.05;
    }
    let loyal = away.loyal();
    if *a < loyal as f32 {
        *a = if ((loyal / 2) as f32) <= *a {
            (f64::from(loyal / 2) + f64::from(*a)) as f32
        } else {
            (f64::from(loyal) + f64::from(*a)) as f32
        };
    }
}

/// The demand for a match (`FUN_0078e5e6`, summed by `FUN_0078ada8`): the
/// home fans over the four classes by their prices, the away fans on the
/// side and the standing places.
fn demand(home: &Side, away: &Side, prices: [i16; 4], t: &PresaleTables) -> i32 {
    let (own, guests) = demand_halves(home, away, prices, t);
    own + guests
}

/// [`demand`]'s two halves apart, as `FUN_0078e5e6` hands them back: the
/// home fans, the visitors.
fn demand_halves(home: &Side, away: &Side, prices: [i16; 4], t: &PresaleTables) -> (i32, i32) {
    let f = distance(home, away);
    let row = |class: usize| t.demand.get(class).copied().unwrap_or([0; 4]);
    let (lh, la) = (fan_class(home, away, t), fan_class(away, home, t));
    let mut h = if f <= 1.0 { row(lh)[0] as f32 } else { (f64::from(row(lh)[0]) * f64::from(f)) as f32 };
    let mut a = if la <= 3 && f < 1.0 {
        (f64::from(row(la)[2]) * f64::from(f) + f64::from(row(la)[3])) as f32
    } else {
        (f64::from(row(la)[1]) * f64::from(f)) as f32
    };
    h = (f64::from(pull(home, t)) * f64::from(h) / 100.0) as f32;
    a = (f64::from(pull(away, t)) * f64::from(a) / 100.0) as f32;
    standing(home, away, &mut h, &mut a, t);
    let loyal = away.loyal();
    if loyal != 0 && loyal as f64 > f64::from(a) {
        a = (f64::from(loyal / 2) + f64::from(a)) as f32;
    }
    let class = f64::from(home.class() * 2);
    let boxes = (class + f64::from(3.3f32)) as f32;
    let side = (class + 33.0) as f32;
    let behind = (class + 11.0) as f32;
    let standing = (110.0 - f64::from(boxes) - f64::from(behind) - f64::from(side)) as f32;
    let [stand_price, behind_price, side_price, box_price] = prices;
    let [box_ref, side_ref, behind_ref, stand_ref] = REFERENCE_PRICES;
    let w_box = weight(box_price, box_ref);
    let w_side = weight(side_price, side_ref);
    let w_behind = weight(behind_price, behind_ref);
    let w_stand = weight(stand_price, stand_ref);
    let boxes = boxes * w_box;
    let side = side * w_side;
    let behind = behind * w_behind;
    let standing = standing * w_stand;
    let hh = f64::from(h);
    let own = ftol(hh * f64::from(boxes) + hh * f64::from(side) + hh * f64::from(behind) + hh * f64::from(standing)) / 100;
    let aa = f64::from(a);
    let guests = ftol(aa * 4.0 * f64::from(w_side) / 5.0 + aa * f64::from(w_stand) / 5.0);
    (own, guests)
}

/// The crowd of a league match on its day, its home fans and its
/// visitors (`FUN_0078b26c`'s branch for a fixture whose kind's top bits
/// and +0x24 are nought). The record the pair sells on today
/// (`FUN_0077c9c2`/`FUN_0077bd10`: an unopened one in its order, an open
/// one either way round) lends its prices and its sold tickets; without
/// one the home club's league prices go by the matchday's mark
/// (`FUN_0069e3d1`, `DAT_008acc2c`) and nothing is sold. The demand's
/// halves (`FUN_0078e5e6`) are cut by the day's part of the home
/// country's index (`0x8ad324`); falling short of the sold tickets and
/// the season tickets (club +0x22c), the sold are shared by the halves'
/// weights and the season tickets join the home fans. Nothing is drawn.
pub(crate) fn league_crowd(
    records: &Records,
    home: (u8, i16),
    away: (u8, i16),
    matchday: i32,
    today: i16,
    t: &PresaleTables,
) -> Option<(i32, i32)> {
    let (h, a) = (Side::of(records, home.0, home.1)?, Side::of(records, away.0, away.1)?);
    let sale = records.block(SALES_AT).and_then(|block| {
        (0..SALES).find_map(|k| {
            let r = block.get(k * SALE..(k + 1) * SALE)?;
            let (first, second, day) = ((r[0], word(r, 2)), (r[4], word(r, 6)), word(r, SALE_DAY));
            let found = if word(r, SALE_OPEN) == 0 {
                first == home && second == away && day == today
            } else {
                day == today && ((first == home && second == away) || (first == away && second == home))
            };
            if found { read(block, k) } else { None }
        })
    });
    let (prices, sold) = match &sale {
        Some(sale) => (sale.prices, sale.sold),
        None => {
            let mark = crate::tickets::matchday_mark(h.team, matchday);
            let factor = t.mark_factors.get(mark).copied().unwrap_or(1.0);
            (h.prices().map(|p| ftol(f64::from(p) * f64::from(factor)) as i16), 0)
        }
    };
    let (mut fans, mut guests) = demand_halves(&h, &a, prices, t);
    let index = country_index(records, h.country, t);
    let part = i32::from(t.on_the_day.get(index).copied().unwrap_or(0));
    fans = fans.wrapping_mul(part) / 100;
    guests = guests.wrapping_mul(part) / 100;
    let season = h.season_tickets();
    if fans.wrapping_add(guests) < sold.wrapping_add(season) {
        if guests == 0 {
            fans = sold.wrapping_add(season);
        } else if fans == 0 {
            fans = season;
            guests = sold;
        } else {
            let weight = fans as f32 / (guests.wrapping_add(fans)) as f32;
            fans = ftol(f64::from(sold) * f64::from(weight));
            guests = sold.wrapping_sub(fans);
            fans = fans.wrapping_add(season);
        }
    }
    Some((fans, guests))
}

/// A side's demand at the ground: the home row when the side's club
/// issued the tickets, else the away rows (`FUN_0078ee04`,
/// `FUN_0078f369`).
fn side_demand(t: &PresaleTables, class: usize, f: f32, issuer: bool) -> f32 {
    let row = t.demand.get(class).copied().unwrap_or([0; 4]);
    if issuer {
        if f <= 1.0 { row[0] as f32 } else { (f64::from(row[0]) * f64::from(f)) as f32 }
    } else if class <= 3 && f < 1.0 {
        (f64::from(row[2]) * f64::from(f) + f64::from(row[3])) as f32
    } else {
        (f64::from(row[1]) * f64::from(f)) as f32
    }
}

/// The four classes' shares of a crowd by the club's class — boxes,
/// side, behind the goal and the standing rest of `total` — weighed by
/// their prices against the reference ones.
fn shares(class: i32, extra: [f32; 3], total: f32, prices: [i16; 4]) -> ([f32; 4], f32, f32) {
    let c = f64::from(class * 2);
    let boxes = (c + f64::from(extra[0])) as f32;
    let side = (c + f64::from(extra[1])) as f32;
    let behind = (c + f64::from(extra[2])) as f32;
    let standing = (f64::from(total) - f64::from(boxes) - f64::from(behind) - f64::from(side)) as f32;
    let [stand_price, behind_price, side_price, box_price] = prices;
    let [box_ref, side_ref, behind_ref, stand_ref] = REFERENCE_PRICES;
    let (w_box, w_side) = (weight(box_price, box_ref), weight(side_price, side_ref));
    let (w_behind, w_stand) = (weight(behind_price, behind_ref), weight(stand_price, stand_ref));
    ([boxes * w_box, side * w_side, behind * w_behind, standing * w_stand], w_side, w_stand)
}

/// A crowd over the four classes' weighted shares, in hundreds.
fn over(crowd: f32, s: [f32; 4]) -> i32 {
    let c = f64::from(crowd);
    ftol(c * f64::from(s[0]) + c * f64::from(s[1]) + c * f64::from(s[2]) + c * f64::from(s[3])) / 100
}

/// The away fans on the side and the standing places.
fn guests(crowd: f32, w_side: f32, w_stand: f32) -> i32 {
    let c = f64::from(crowd);
    ftol(c * 4.0 * f64::from(w_side) / 5.0 + c * f64::from(w_stand) / 5.0)
}

/// The cup rounds' pull (`DAT_008ad4c0`), by the record's round.
const ROUND_FACTORS: [f32; 8] = [0.8, 1.0, 1.2, 1.6, 2.4, 3.0, 3.5, 4.0];
const FINAL: i16 = 7;

/// The demand for a cup tie (`FUN_0078ee04`, summed by `FUN_0078f32f`):
/// the club that issued the tickets reads the home row and lends its
/// class; both leagues' pulls averaged; the round's pull, a hundredfold
/// for a final no side of which issued; a small club at home to a first
/// league side with few fans doubled; the classes apart weigh on the home
/// side.
fn cup_demand(home: &Side, away: &Side, sale: &Sale, issuer: (u8, i16), t: &PresaleTables) -> i32 {
    let f = distance(home, away);
    let (lh, la) = (fan_class(home, away, t), fan_class(away, home, t));
    let issued = |side: &Side| issuer == (side.country, i16::from(side.number));
    let (mut neutral, mut class) = (true, 6);
    let mut h = side_demand(t, lh, f, issued(home));
    if issued(home) {
        class = home.class();
        neutral = false;
    }
    let mut a = side_demand(t, la, f, issued(away));
    if issued(away) {
        class = away.class();
        neutral = false;
    }
    standing(home, away, &mut h, &mut a, t);
    let pull = (pull(home, t) + pull(away, t)) / 2;
    h = (f64::from(pull) * f64::from(h) / 100.0) as f32;
    a = (f64::from(pull) * f64::from(a) / 100.0) as f32;
    if sale.extra == FINAL && neutral {
        h *= 100.0;
        a *= 100.0;
    } else if 0 < sale.extra {
        let k = ROUND_FACTORS.get(sale.extra as usize).copied().unwrap_or(1.0);
        h *= k;
        a *= k;
    }
    if home.country == away.country && home.level() >= 2 && away.level() == 0 && away.fans().0 < 2 {
        h = h + h;
    }
    let (lv_home, lv_away) = (i32::from(home.level().min(2)), i32::from(away.level().min(2)));
    if lv_away < lv_home {
        h = ((f64::from(lv_home - lv_away) * 0.2 + 1.0) * f64::from(h)) as f32;
    } else if lv_home < lv_away {
        h = ((f64::from(lv_home - lv_away) * 0.35 + 1.0) * f64::from(h)) as f32;
    }
    let (s, w_side, w_stand) = shares(class, [3.0, 30.0, 10.0], 100.0, sale.prices);
    let own = over(h, s);
    if neutral { own + over(a, s) } else { own + guests(a, w_side, w_stand) }
}

/// How the distance to the ground carries a side's fans
/// (`FUN_0078f76b` by `FUN_0067792f`): as between clubs, from the
/// coordinates of a played country's clubs, nought elsewhere.
fn distance_to(records: &Records, side: &Side, ground: (u8, i16)) -> f32 {
    if side.country != ground.0 {
        return 0.125;
    }
    let at = Side::of(records, ground.0, ground.1)
        .filter(|g| records.globals.played(i16::from(g.country)))
        .map_or((0, 0), |g| g.at());
    let (x1, y1) = side.at();
    let d = ftol(f64::from((x1 - at.0) * (x1 - at.0) + (y1 - at.1) * (y1 - at.1)).sqrt() * 1.0);
    match d {
        ..3 => 3.0,
        3..10 => 2.5,
        10..25 => 2.0,
        25..50 => 0.8,
        50..100 => 0.4,
        100..250 => 0.2,
        _ => 0.125,
    }
}

/// The demand for an international tie (`FUN_0078f369`, summed by
/// `FUN_0078f803`): a final sells a quarter of a million on each side;
/// else both sides' fans by their distances to the ground, the issuing
/// club reading the home row, their leagues' pulls, a small club at home
/// to a first league side doubled, the round's pull, and Germany at home
/// to country 0x43 five times its guests.
fn international_demand(records: &Records, home: &Side, away: &Side, sale: &Sale, issuer: (u8, i16), t: &PresaleTables) -> i32 {
    let (h, a) = international_halves(records, home, away, sale, issuer, t);
    h + a
}

/// The demand of a friendly's or a tournament game's two sides
/// (`FUN_0078b26c`'s branch for them: `FUN_0078f369` with the game's
/// round index — 5 a friendly, 3, 2 and 5 a Turnier's semifinals, third
/// place and final —, the ground as the issuer, the home club's league
/// prices for its standing and seats and 15 and 20 above), its halves
/// apart: the home fans, the visitors.
pub(crate) fn match_demand(
    records: &Records,
    home: (u8, i16),
    away: (u8, i16),
    ground: (u8, i16),
    prices: [i16; 2],
    round: i16,
    t: &PresaleTables,
) -> Option<(i32, i32)> {
    let (h, a) = (Side::of(records, home.0, home.1)?, Side::of(records, away.0, away.1)?);
    let sale = Sale {
        at: 0,
        home,
        away,
        issuer: ground,
        day: 0,
        kind: 0,
        extra: round,
        sold: 0,
        prices: [prices[0], prices[1], 15, 20],
    };
    Some(international_halves(records, &h, &a, &sale, ground, t))
}

/// `FUN_0078f369`'s two sides: the home fans over the four classes'
/// shares, the visitors over the side seats and the standing.
fn international_halves(records: &Records, home: &Side, away: &Side, sale: &Sale, issuer: (u8, i16), t: &PresaleTables) -> (i32, i32) {
    let (lh, la) = (fan_class(home, away, t), fan_class(away, home, t));
    if sale.extra == FINAL {
        return (250_000, 250_000);
    }
    let fa = distance_to(records, away, issuer);
    let fh = distance_to(records, home, issuer);
    let mut a = side_demand(t, la, fa, false);
    let mut h = side_demand(t, lh, fh, issuer == (home.country, i16::from(home.number)));
    h = (f64::from(pull(home, t)) * f64::from(h) / 100.0) as f32;
    a = (f64::from(pull(away, t)) * f64::from(a) / 100.0) as f32;
    if home.country == away.country && home.level() >= 2 && away.level() == 0 && away.fans().0 < 2 {
        h = h + h;
    }
    if (0..=FINAL).contains(&sale.extra) {
        let k = ROUND_FACTORS[sale.extra as usize];
        h *= k;
        a *= k;
    }
    if away.country == 0x43 && home.country == 0xe {
        a *= 5.0;
    }
    let (s, w_side, w_stand) = shares(home.class(), [3.0, 30.0, 10.0], 100.0, sale.prices);
    (over(h, s), guests(a, w_side, w_stand))
}

/// How many a cup or international tie's record has sold by now
/// (`FUN_0078ade2`, `FUN_0078b026`): within the week as a league match's
/// but without the cap of fifteen thousand; further ahead per mille,
/// drawn by how far. The issuing club's stadium bounds it, and a record
/// never sells less than it had.
fn sell_tie(records: &Records, sale: &Sale, today: i32, random: &mut Random, t: &PresaleTables) -> i32 {
    let (Some(home), Some(away)) =
        (Side::of(records, sale.home.0, sale.home.1), Side::of(records, sale.away.0, sale.away.1))
    else {
        return sale.sold;
    };
    let want = if (3..=4).contains(&sale.kind) {
        cup_demand(&home, &away, sale, sale.issuer, t)
    } else {
        international_demand(records, &home, &away, sale, sale.issuer, t)
    };
    let room = Side::of(records, sale.issuer.0, sale.issuer.1).map_or(0, |g| capacity(records, &g));
    let index = country_index(records, sale.issuer.0, t);
    let mut sold = sale.sold;
    let days = i32::from(sale.day) - today;
    if i32::from(sale.day) < today + WEEK {
        let by_days = i32::from(t.by_days.get(days.max(0) as usize).copied().unwrap_or(0));
        let luck = random.below(2) as i32;
        let by_country = i32::from(t.by_country.get(index).copied().unwrap_or(0));
        sold = sold.wrapping_add(want.wrapping_mul(by_days + luck * 3) / 100 * by_country / 100);
        if room < sold {
            sold = room;
        }
    } else {
        let per_mille = if days < 15 {
            random.below(150) as i32 + 50
        } else if days < 25 {
            random.below(80) as i32 + 40
        } else {
            random.below(20) as i32 + 1
        };
        let mut ahead = (want.wrapping_mul(per_mille) as u32 / 1000) as i32;
        if room < ahead {
            ahead = room;
        }
        if sold < ahead {
            sold = ahead;
        }
    }
    sold.max(sale.sold)
}

/// A record of the sales.
#[derive(Clone, Copy, Debug)]
struct Sale {
    at: usize,
    home: (u8, i16),
    away: (u8, i16),
    issuer: (u8, i16),
    day: i16,
    kind: i16,
    extra: i16,
    sold: i32,
    prices: [i16; 4],
}

fn word(b: &[u8], at: usize) -> i16 {
    i16::from_le_bytes([b[at], b[at + 1]])
}

fn read(block: &[u8], k: usize) -> Option<Sale> {
    let r = block.get(k * SALE..(k + 1) * SALE)?;
    (r[0] != 0).then(|| Sale {
        at: k,
        home: (r[0], word(r, 2)),
        away: (r[4], word(r, 6)),
        issuer: (r[SALE_ISSUER], word(r, SALE_ISSUER + 2)),
        day: word(r, SALE_DAY),
        kind: word(r, SALE_KIND),
        extra: word(r, SALE_EXTRA),
        sold: i32::from_le_bytes(r[SALE_SOLD..SALE_SOLD + 4].try_into().unwrap()),
        prices: std::array::from_fn(|i| word(r, SALE_PRICES + 2 * i)),
    })
}

/// How many a league match's record has sold by now (`FUN_0078ab55`):
/// within the week, the demand by the days left and a little luck, by the
/// country's index, fifteen thousand a day at most; further ahead a few
/// percent of it, drawn by how far. Never more than the stadium holds
/// besides its season tickets.
fn sell_league(records: &Records, sale: &Sale, today: i32, random: &mut Random, t: &PresaleTables) -> i32 {
    let (Some(home), Some(away)) =
        (Side::of(records, sale.home.0, sale.home.1), Side::of(records, sale.away.0, sale.away.1))
    else {
        return sale.sold;
    };
    let mut sold = sale.sold;
    let room = capacity(records, &home) - home.season_tickets();
    let days = i32::from(sale.day) - today;
    if i32::from(sale.day) < today + WEEK {
        let want = demand(&home, &away, sale.prices, t);
        let by_days = i32::from(t.by_days.get(days.max(0) as usize).copied().unwrap_or(0));
        let luck = random.below(2) as i32;
        let index = country_index(records, home.country, t);
        let by_country = i32::from(t.by_country.get(index).copied().unwrap_or(0));
        let mut more = want.wrapping_mul(by_days + luck * 3) / 100 * by_country / 100;
        if 15000 < more {
            more = random.below(1000) as i32 + 15000;
        }
        sold = sold.wrapping_add(more);
        if room < sold {
            sold = room;
        }
    } else {
        let want = demand(&home, &away, sale.prices, t);
        let percent = if days < 15 {
            random.below(15) as i32 + 5
        } else if days < 25 {
            random.below(8) as i32 + 4
        } else {
            random.below(2) as i32
        };
        let mut ahead = (want.wrapping_mul(percent) as u32 / 100) as i32;
        if room < ahead {
            ahead = room;
        }
        if sold < ahead {
            sold = ahead;
        }
    }
    sold
}

/// What the club's stadium holds (`FUN_00638335`, the stadium's +0x3c): a
/// played or playable country's club's from its live stadium object, a
/// foreign one's from the foreign records (`DAT_01a3af38`, by
/// [`crate::opponent::stadium`]) — the ground of a final on a neutral
/// ground mostly stands abroad.
fn capacity(records: &Records, side: &Side) -> i32 {
    ground_capacity(records, side.country, side.number.into())
}

/// The same for a club by its country and number.
pub(crate) fn ground_capacity(records: &Records, country: u8, number: i16) -> i32 {
    if !records.globals.played(i16::from(country)) && !crate::opponent::PLAYABLE.contains(&u16::from(country)) {
        return u16::try_from(number).ok().and_then(|n| crate::opponent::stadium(records, country.into(), n)).map_or(0, |s| s.capacity);
    }
    let Ok(number) = u16::try_from(number) else { return 0 };
    records
        .team_index(country.into(), number)
        .and_then(|i| records.block(&format!("0x1a48138[{i}]")))
        .and_then(|b| b.get(0x3c..0x40))
        .map_or(0, |c| i32::from_le_bytes(c.try_into().unwrap()))
}

/// A record's sale of the day, by its kind (`FUN_0077bfbd`,
/// `FUN_0077c0c9`): a league match's from the seventh week on, a cup or
/// international tie's at once.
fn sell(records: &mut Records, sale: &Sale, today: i32, random: &mut Random, t: &PresaleTables) {
    let sold = match sale.kind {
        LEAGUE if today > FIRST_SALES_DAY => sell_league(records, sale, today, random, t),
        3..=10 => sell_tie(records, sale, today, random, t),
        _ => return,
    };
    if let Some(b) = records.block_mut(SALES_AT) {
        b[sale.at * SALE + SALE_SOLD..sale.at * SALE + SALE_SOLD + 4].copy_from_slice(&sold.to_le_bytes());
    }
}

/// The club's league matches of the season by day (`FUN_0062af24`): the
/// opponent, the day, and whether at home; forty-six at most.
fn matches(records: &Records, calendar: &Calendar, club: &Side) -> Vec<(i16, usize, bool)> {
    let mut found = Vec::new();
    if records.countries.kind(usize::from(club.country)) != 1 || club.level() >= 10 {
        return found;
    }
    let (Some(row), Some(league)) = (calendar.row(u16::from(club.country), usize::from(club.level())), club.league())
    else {
        return found;
    };
    let teams = usize::try_from(club.league_word(0x30)).unwrap_or(0);
    let slot = club.slot();
    let number = |place: u8| word(league, 0x52 + usize::from(place) * 2);
    for day in FIRST_DAY..LAST_DAY {
        let code = calendar.entry(row, day);
        if code == 0 || code >= LAST_MATCHDAY_CODE {
            continue;
        }
        let md = (code - 1) as usize;
        for e in (0..teams).step_by(2) {
            let at = 0x53a + md * 0x18 + e;
            let (Some(&h), Some(&a)) = (league.get(at), league.get(at + 1)) else { continue };
            let entry = if h == slot {
                (number(a), day, true)
            } else if a == slot {
                (number(h), day, false)
            } else {
                continue;
            };
            if found.len() >= MOST_MATCHES {
                return found;
            }
            found.push(entry);
        }
    }
    found
}

/// Opens a record for a match unless one is kept (`FUN_0077c1d5`), and
/// makes its first sale; whether one was opened. The home club stands first
/// and issues the record (`FUN_0077bec6`'s null fixture list), and the mark
/// at +0x10 — a cup tie's round — starts empty: the original's `FUN_0077be53`
/// gets a nought there, and only after the first sale has been made do the
/// cup paths write the round ([`set_extra`]).
fn open(
    records: &mut Records,
    home: (u8, i16),
    away: (u8, i16),
    day: i16,
    kind: i16,
    today: i32,
    random: &mut Random,
    t: &PresaleTables,
) -> bool {
    open_issued(records, home, away, home, day, kind, today, random, t)
}

/// [`open`] with the club that issues the record named: over a fixture
/// list `FUN_0077c1d5` hands `FUN_0077bec6` the list's ground — its
/// country and number — where it would hand the home club.
#[allow(clippy::too_many_arguments)]
fn open_issued(
    records: &mut Records,
    home: (u8, i16),
    away: (u8, i16),
    issuer: (u8, i16),
    day: i16,
    kind: i16,
    today: i32,
    random: &mut Random,
    t: &PresaleTables,
) -> bool {
    let Some(block) = records.block(SALES_AT) else { return false };
    let mut free = None;
    for k in 0..SALES {
        let r = &block[k * SALE..(k + 1) * SALE];
        let (a, b, d) = ((r[0], word(r, 2)), (r[4], word(r, 6)), word(r, SALE_DAY));
        let found = if word(r, SALE_OPEN) == 0 {
            a == home && b == away && d == day
        } else {
            d == day && ((a == home && b == away) || (a == away && b == home))
        };
        if found {
            return false;
        }
        if free.is_none() && r[0] == 0 {
            free = Some(k);
        }
    }
    if let Some(k) = free {
        let Some(b) = records.block_mut(SALES_AT) else { return true };
        let r = &mut b[k * SALE..(k + 1) * SALE];
        r[0] = home.0;
        r[2..4].copy_from_slice(&home.1.to_le_bytes());
        r[4] = away.0;
        r[6..8].copy_from_slice(&away.1.to_le_bytes());
        r[SALE_ISSUER] = issuer.0;
        r[SALE_ISSUER + 2..SALE_ISSUER + 4].copy_from_slice(&issuer.1.to_le_bytes());
        r[SALE_DAY..SALE_DAY + 2].copy_from_slice(&day.to_le_bytes());
        r[SALE_KIND..SALE_KIND + 2].copy_from_slice(&kind.to_le_bytes());
        r[SALE_EXTRA..SALE_EXTRA + 2].copy_from_slice(&0i16.to_le_bytes());
        r[SALE_SOLD..SALE_SOLD + 4].copy_from_slice(&0i32.to_le_bytes());
        r[SALE_OPEN..SALE_OPEN + 2].copy_from_slice(&1i16.to_le_bytes());
        let sale = records.block(SALES_AT).and_then(|b| read(b, k));
        if let Some(sale) = sale {
            sell(records, &sale, today, random, t);
        }
    }
    true
}

/// The week's end for a human club (`FUN_0077ca2b`): after the sixth week
/// its league matches of the next four weeks get a record, the home ones
/// first, then the away ones; a record just opened takes the prices of the
/// club at home, by the mark of the match's place in the list.
pub fn week_end(records: &mut Records, calendar: &Calendar, club: (u8, i16), today: i32, random: &mut Random, t: &PresaleTables) {
    if today <= FIRST_SALES_DAY {
        return;
    }
    let list = match Side::of(records, club.0, club.1) {
        Some(side) => matches(records, calendar, &side),
        None => return,
    };
    for home in [true, false] {
        for (k, &(opponent, day, at_home)) in list.iter().enumerate() {
            if at_home != home || (day as i32) < today {
                continue;
            }
            if today + AHEAD < day as i32 {
                break;
            }
            let (h, a) = if home { (club, (club.0, opponent)) } else { ((club.0, opponent), club) };
            if !open(records, h, a, day as i16, LEAGUE, today, random, t) {
                continue;
            }
            let Some(own) = Side::of(records, club.0, club.1) else { continue };
            let mark = crate::tickets::matchday_mark(own.team, k as i32 + 1);
            let factor = t.mark_factors.get(mark).copied().unwrap_or(1.0);
            let Some(host) = Side::of(records, h.0, h.1) else { continue };
            let prices = host.prices().map(|p| ftol(f64::from(p) * f64::from(factor)) as i16);
            set_prices(records, h, a, day as i16, prices);
        }
    }
}

/// The record the pair plays on that day takes these four prices
/// (`FUN_0077bf84`).
fn set_prices(records: &mut Records, home: (u8, i16), away: (u8, i16), day: i16, prices: [i16; 4]) {
    let found = records.block(SALES_AT).and_then(|b| {
        (0..SALES).find_map(|k| read(b, k).filter(|s| s.home == home && s.away == away && s.day == day))
    });
    if let (Some(s), Some(b)) = (found, records.block_mut(SALES_AT)) {
        for (i, p) in prices.iter().enumerate() {
            let at = s.at * SALE + SALE_PRICES + 2 * i;
            b[at..at + 2].copy_from_slice(&p.to_le_bytes());
        }
    }
}

/// The record the pair plays on that day takes this mark at +0x10, written
/// after the first sale (`FUN_0077c4ee`'s and `FUN_0077c379`'s last step).
fn set_extra(records: &mut Records, home: (u8, i16), away: (u8, i16), day: i16, extra: i16) {
    let found = records.block(SALES_AT).and_then(|b| {
        (0..SALES).find_map(|k| read(b, k).filter(|s| s.home == home && s.away == away && s.day == day))
    });
    if let (Some(s), Some(b)) = (found, records.block_mut(SALES_AT)) {
        let at = s.at * SALE + SALE_EXTRA;
        b[at..at + 2].copy_from_slice(&extra.to_le_bytes());
    }
}

/// A club's next cup tie's record, opened where the club notes that match
/// (`FUN_0062bfc9`'s `FUN_0077d425` → [`fun@FUN_0077c4ee`]): the day and the
/// kind come from the club's own next-match fields, the two clubs stand with
/// the home one first, the home club issues the record, and the tie's round
/// goes in at +0x10 after the first sale — the demand of the days after
/// takes it as its round's factor, the first sale does not see it. The prices
/// are the home club's cup column as it stands, with none of the matchday's
/// mark the week's path puts on its league records; the first sale follows
/// before them, as the original's does. The final on a neutral ground is
/// [`open_listed`]'s.
pub fn open_cup(records: &mut Records, home: (u8, i16), away: (u8, i16), day: i16, round: u8, today: i32, random: &mut Random, t: &PresaleTables) {
    if !open(records, home, away, day, CUP_TIE, today, random, t) {
        return;
    }
    let Some(host) = Side::of(records, home.0, home.1) else { return };
    set_prices(records, home, away, day, host.cup_prices());
    set_extra(records, home, away, day, i16::from(round));
}

/// A club's next supercup tie's record over no fixture list, opened where
/// the club notes that match (`FUN_0062bfc9`'s kind-4 branch, its
/// `FUN_0077d425(club, tie, 0, 0)` → [`fun@FUN_0077c4ee`] again): the two
/// clubs and the issuing one stand as in the cup tie's record, only the
/// kind is the supercup's own 4, the prices the home club's cup column as
/// in [`open_cup`], and +0x10 a nought — the office's 0 for the plain
/// arm, not the stage the note carries. Over a list it is
/// [`open_listed`]'s, with the mark 1.
pub fn open_supercup(records: &mut Records, home: (u8, i16), away: (u8, i16), day: i16, today: i32, random: &mut Random, t: &PresaleTables) {
    if !open(records, home, away, day, SUPERCUP_TIE, today, random, t) {
        return;
    }
    let Some(host) = Side::of(records, home.0, home.1) else { return };
    set_prices(records, home, away, day, host.cup_prices());
    set_extra(records, home, away, day, 0);
}

/// The prices of a record opened over a fixture list: `DAT_00a3b420`, then
/// thrice `DAT_00a3b418`, which `FUN_00627bd0` fills from floats times a
/// multiplier of 1.
const LISTED_PRICES: [i16; 4] = [10, 20, 20, 20];
/// The mark a final's record carries at +0x10 over the list
/// (`FUN_0062bfc9`'s `push 7`), which the demand reads as [`FINAL`].
pub const LISTED_FINAL: i16 = FINAL;
/// The mark a supercup tie's record carries over the list.
pub const LISTED_SUPERCUP: i16 = 1;

/// A tie's record opened over the day's fixture list (`FUN_0062bfc9`'s
/// arms with `local_1f8` handed on: `FUN_0077d425` → [`fun@FUN_0077c4ee`]
/// for the cup's and the supercup's, `FUN_0077d405` →
/// [`fun@FUN_0077c379`] for the champion's cup's): the clubs stand home
/// first as ever, but the record is issued by the list's ground — the
/// stadium the finder named, for a final on a neutral ground the one
/// drawn for it — so the first sale already reads that club's stadium and
/// country. The prices are the list's constants [`LISTED_PRICES`], none of
/// either club's columns, and +0x10 takes the caller's mark after the
/// first sale: 7 for a final, which the demand of the days after sells as
/// one ([`cup_demand`]'s hundredfold where no side issued,
/// [`international_halves`]' quarter million a side), 1 for a supercup
/// tie.
#[allow(clippy::too_many_arguments)]
pub fn open_listed(
    records: &mut Records,
    kind: i16,
    home: (u8, i16),
    away: (u8, i16),
    ground: (u8, i16),
    day: i16,
    mark: i16,
    today: i32,
    random: &mut Random,
    t: &PresaleTables,
) {
    if !open_issued(records, home, away, ground, day, kind, today, random, t) {
        return;
    }
    set_prices(records, home, away, day, LISTED_PRICES);
    set_extra(records, home, away, day, mark);
}

/// A club's next international tie's record — the champion's cup's, the
/// UEFA cup's or the Intertoto's — opened where the club notes that match
/// (`FUN_0062bfc9`'s `FUN_0077d405` → [`fun@FUN_0077c379`]): the same as
/// [`open_cup`] with two differences. The kind is the club's own next
/// match's, 5, 7 or 8, and the prices come from the home club's
/// international column, the third of the three columns a club keeps for
/// its stands. Here too the round goes in only after the first sale — the
/// UEFA cup's and the Intertoto's ties have none, their notes at +0x10
/// staying a nought. The champion's cup's final on a neutral ground is
/// [`open_listed`]'s. The world's ties — the European supercup's (kind 9)
/// and the world cup's (kind 10) — open no record at all: their arms of
/// `FUN_0062bfc9` call the office with neither writer, and the takeover's
/// `FUN_0077cef1` lists only the cup's and the 0x50000000 days.
pub fn open_international(records: &mut Records, kind: i16, home: (u8, i16), away: (u8, i16), day: i16, round: u8, today: i32, random: &mut Random, t: &PresaleTables) {
    if !open(records, home, away, day, kind, today, random, t) {
        return;
    }
    let Some(host) = Side::of(records, home.0, home.1) else { return };
    set_prices(records, home, away, day, host.international_prices());
    set_extra(records, home, away, day, i16::from(round));
}

/// The day's sales (`FUN_0077c8b5`): a record whose day has passed is let
/// go, those of the next week sell on.
pub fn day(records: &mut Records, today: i32, random: &mut Random, t: &PresaleTables) {
    for k in 0..SALES {
        let Some(sale) = records.block(SALES_AT).and_then(|b| read(b, k)) else { continue };
        if i32::from(sale.day) < today {
            if let Some(b) = records.block_mut(SALES_AT) {
                b[k * SALE] = 0;
                b[k * SALE + SALE_OPEN..k * SALE + SALE_OPEN + 2].copy_from_slice(&0i16.to_le_bytes());
            }
        } else if i32::from(sale.day) <= today + WEEK {
            sell(records, &sale, today, random, t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3_files::an3::Save;

    fn keeps() -> std::path::PathBuf {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        dir.parent().unwrap().join("saves-kept")
    }

    fn live(r: &Records) -> Vec<Sale> {
        r.block(SALES_AT).map_or(Vec::new(), |b| (0..SALES).filter_map(|k| read(b, k)).collect())
    }

    /// A cup tie's record stands with the home club first and issuing it,
    /// kind 3 and its round at +0x10, and priced by the home club's own cup
    /// column — not the league's, and with none of the matchday's mark on
    /// it. No kept save reaches this on its own (theirs all stand where the
    /// human's next match is a league one), so the tie is patched into
    /// `S2late.an3`, whose human club sells league matches at
    /// [17, 23, 43, 150] and cup ties at [15, 25, 25, 25].
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_cup_ties_record_takes_the_home_clubs_cup_column() {
        let data = crate::data::Data::load(&std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))).unwrap();
        let mut ours = Records::from_save(Save::read(&keeps().join("S2late.an3")).unwrap()).unwrap();
        let (league, cup) = match Side::of(&ours, 14, 84) {
            Some(side) => (side.prices(), side.cup_prices()),
            None => panic!("no human club 14.84"),
        };
        assert_ne!(league, cup, "{league:?} vs {cup:?}: the save must tell its two columns apart");
        let mut random = Random::new(1);
        open_cup(&mut ours, (14, 84), (14, 22), 245, 3, 240, &mut random, &data.presale);
        let tie = live(&ours).into_iter().find(|s| s.day == 245).expect("no record for the patched tie");
        assert_eq!((tie.kind, tie.extra), (CUP_TIE, 3));
        assert_eq!((tie.home, tie.away, tie.issuer), ((14, 84), (14, 22), (14, 84)));
        assert_eq!(tie.prices, cup);
        // The tie noted again — the club's next match is looked for every
        // day — leaves the one record standing (`FUN_0077bd10`).
        open_cup(&mut ours, (14, 84), (14, 22), 245, 3, 240, &mut random, &data.presale);
        assert_eq!(live(&ours).iter().filter(|s| s.day == 245).count(), 1);
    }

    /// The human club's next match as `FUN_0062bfc9` notes it, for the
    /// office's gate.
    fn noted(day: i16, opponent: (u8, i16), kind: u8, round: u8) -> crate::year::NextMatch {
        crate::year::NextMatch { day, opponent, venue: 0, kind, round }
    }

    /// The champion's cup's final goes over the day's fixture list: its day
    /// 332 carries the first row's id 0x18, which `FUN_0062cf7f` marks as a
    /// final, so `FUN_0062bfc9` hands `FUN_0077c379` the ground
    /// `FUN_0041e17c` wrote — the one drawn for the final, `DAT_00b21ab8`,
    /// 13.5 in `S2late.an3` — and the mark 7. The record stands home first,
    /// issued by that ground, priced at the list's [10, 20, 20, 20] whatever
    /// the clubs' columns say. Its later sales sell it as a final — a
    /// quarter of a million a side — up to the ground's stadium. A day of
    /// the cup's earlier rounds goes over no list. No kept save stands the
    /// human in the final, so it is noted by hand on `S2late.an3`.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_champion_cups_final_sells_over_the_list_from_its_drawn_ground() {
        let data = crate::data::Data::load(&std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))).unwrap();
        let mut ours = Records::from_save(Save::read(&keeps().join("S2late.an3")).unwrap()).unwrap();
        patch_columns(&mut ours, (14, 84), [(0x49c, [11, 12, 13, 14i16]), (0x49e, [21, 22, 23, 24]), (0x4a0, [31, 32, 33, 34])]);
        let human = ours.team(14, 84).expect("no human club 14.84").clone();
        let ground = crate::year::listed_ground(&ours, &data.calendar, &human, &noted(332, (68, 3), INTERNATIONAL as u8, 7));
        assert_eq!(ground, Some((13, 5)));
        assert_eq!(crate::year::listed_ground(&ours, &data.calendar, &human, &noted(283, (68, 3), INTERNATIONAL as u8, 6)), None);
        let mut random = Random::new(1);
        open_listed(&mut ours, INTERNATIONAL, (14, 84), (68, 3), (13, 5), 332, LISTED_FINAL, 300, &mut random, &data.presale);
        let tie = live(&ours).into_iter().find(|s| s.day == 332).expect("no record for the final");
        assert_eq!((tie.kind, tie.extra, tie.home, tie.away, tie.issuer, tie.prices), (INTERNATIONAL, 7, (14, 84), (68, 3), (13, 5), [10, 20, 20, 20]));
        let host = Side::of(&ours, 13, 5).expect("no club 13.5");
        let room = capacity(&ours, &host);
        assert!(room > 0);
        let first = tie.sold;
        for today in 320..=331 {
            day(&mut ours, today, &mut random, &data.presale);
        }
        let sold = live(&ours).into_iter().find(|s| s.day == 332).map(|s| s.sold).unwrap();
        assert!(first <= sold && sold <= room && sold > 0, "{first} then {sold} of {room}");
    }

    /// The cup's final on a neutral ground goes over the list too:
    /// `FUN_0062cd4b` sets the list's +8 word for the id 6 under the
    /// 0x80000 flag — Germany's code on day 328 is 0x40080006 — and
    /// `FUN_006d1104` names the ground drawn for the country's slot
    /// (`DAT_00b21aa4`, 14.11 in `S2late.an3`). The record is issued by it,
    /// priced at the list's constants and marked 7 — the [`FINAL`] that
    /// [`cup_demand`] sells a hundredfold where neither side issued. A
    /// round before it stays in the plain column. Noted by hand, as no kept
    /// save stands the human in a cup final.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_cups_final_sells_over_the_list_from_its_drawn_ground() {
        let data = crate::data::Data::load(&std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))).unwrap();
        let mut ours = Records::from_save(Save::read(&keeps().join("S2late.an3")).unwrap()).unwrap();
        let human = ours.team(14, 84).expect("no human club 14.84").clone();
        assert_eq!(crate::year::listed_ground(&ours, &data.calendar, &human, &noted(328, (14, 22), CUP_TIE as u8, 6)), Some((14, 11)));
        assert_eq!(crate::year::listed_ground(&ours, &data.calendar, &human, &noted(262, (14, 22), CUP_TIE as u8, 5)), None);
        let mut random = Random::new(1);
        open_listed(&mut ours, CUP_TIE, (14, 84), (14, 22), (14, 11), 328, LISTED_FINAL, 300, &mut random, &data.presale);
        let tie = live(&ours).into_iter().find(|s| s.day == 328).expect("no record for the final");
        assert_eq!((tie.kind, tie.extra, tie.home, tie.away, tie.issuer, tie.prices), (CUP_TIE, 7, (14, 84), (14, 22), (14, 11), [10, 20, 20, 20]));
    }

    /// Every German supercup tie goes over the list, its ground the
    /// Ligapokal's of its tie — `DAT_00b21acc` for the first four, 14.23
    /// for the first in `S2late.an3` — and the slot's for the final on day
    /// 45 (`DAT_00b21a90`, 14.2).
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_german_supercup_tie_takes_the_ligapokals_ground() {
        let data = crate::data::Data::load(&std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))).unwrap();
        let ours = Records::from_save(Save::read(&keeps().join("S2late.an3")).unwrap()).unwrap();
        let human = ours.team(14, 84).expect("no human club 14.84").clone();
        let ground = |day| crate::year::listed_ground(&ours, &data.calendar, &human, &noted(day, (14, 22), SUPERCUP_TIE as u8, 5));
        assert_eq!((ground(33), ground(45)), (Some((14, 23)), Some((14, 2))));
    }

    /// The three price columns of a club's stands written apart, so a
    /// record's prices say which of them was read.
    fn patch_columns(records: &mut Records, (country, number): (u16, u16), columns: [(usize, [i16; 4]); 3]) {
        if let Some(t) = records.team_mut(country, number) {
            for (base, values) in columns {
                for (k, p) in values.iter().enumerate() {
                    t.put::<i16>(base + [0, 0xa, 0x14, 0x1e][k] + CLUB_TO_TEAM, *p);
                }
            }
        }
    }

    /// Lets the office's international records go, as the day's path lets a
    /// record whose day has passed go.
    fn clear_international(records: &mut Records) {
        if let Some(b) = records.block_mut(SALES_AT) {
            for k in 0..SALES {
                let r = &mut b[k * SALE..(k + 1) * SALE];
                if i16::from_le_bytes([r[SALE_KIND], r[SALE_KIND + 1]]) == INTERNATIONAL {
                    r[0] = 0;
                    r[SALE_OPEN..SALE_OPEN + 2].copy_from_slice(&0i16.to_le_bytes());
                }
            }
        }
    }

    /// A champion's cup tie's record sells at the home club's third column
    /// of prices, the international one. `NewSeason000.an3` stands where
    /// the human club 14.84 plays away at 68.3 on day 31 — kind 5, round 0,
    /// away — and the original's office holds the record: 68.3 first and
    /// issuing it, priced [15, 25, 50, 100]. Noting the tie again leaves
    /// that one record standing, the block byte for byte as the save has
    /// it. The save's own columns cannot say which of the three was read —
    /// a new season's clubs hold all three equal — so they are patched
    /// apart and the record cleared: what comes back is priced by the
    /// international column, neither the league's nor the cup's, with the
    /// round at +0x10 written after the first sale.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_champion_cups_record_takes_the_home_clubs_international_column() {
        let data = crate::data::Data::load(&std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))).unwrap();
        let read = || Records::from_save(Save::read(&keeps().join("NewSeason000.an3")).unwrap()).unwrap();
        let ours = read();
        let human = ours.team(14, 84).expect("no human club 14.84");
        assert_eq!(
            (human.next_day(), human.next_kind(), human.next_round(), human.next_venue(), human.next_country(), human.next_number()),
            (31, INTERNATIONAL as u8, 0, 1, 68, 3)
        );
        let opened: Vec<Sale> = live(&ours).into_iter().filter(|s| s.kind == INTERNATIONAL).collect();
        assert_eq!(opened.len(), 1, "{opened:?}");
        let tie = &opened[0];
        assert_eq!((tie.home, tie.away, tie.issuer, tie.day, tie.extra), ((68, 3), (14, 84), (68, 3), 31, 0));
        let international = match Side::of(&ours, 68, 3) {
            Some(side) => side.international_prices(),
            None => panic!("no club 68.3"),
        };
        assert_eq!(tie.prices, international);
        // The tie noted again finds the record standing: nothing is opened
        // and nothing sold, the block as the save has it.
        let mut ours = read();
        let before = ours.block(SALES_AT).map(<[u8]>::to_vec).unwrap();
        let mut random = Random::new(1);
        crate::year::note_next_matches(&mut ours, &data, 0, &mut random);
        assert_eq!(ours.block(SALES_AT).map(<[u8]>::to_vec), Some(before));
        // Cleared, the record comes back of itself. Its home club's three
        // columns are patched apart first, so the prices say which was read.
        let mut ours = read();
        clear_international(&mut ours);
        let patched = [31, 32, 33, 34];
        patch_columns(&mut ours, (68, 3), [(0x49c, [11, 12, 13, 14i16]), (0x49e, [21, 22, 23, 24]), (0x4a0, patched)]);
        crate::year::note_next_matches(&mut ours, &data, 0, &mut random);
        let again: Vec<Sale> = live(&ours).into_iter().filter(|s| s.day == 31).collect();
        assert_eq!(again.len(), 1, "{again:?}");
        assert_eq!((again[0].kind, again[0].extra, again[0].home, again[0].away, again[0].issuer), (INTERNATIONAL, 0, (68, 3), (14, 84), (68, 3)));
        assert_eq!(again[0].prices, patched, "not the league's [11, 12, 13, 14] nor the cup's [21, 22, 23, 24]");
    }

    /// A UEFA cup tie goes the same way: the record carries the club's own
    /// kind — 7, read off the finder's `0x100` bit by `FUN_0062bfc9` — and
    /// sells at the home club's international column, keeping a nought at
    /// +0x10 since the tie is noted without a round. Patched into
    /// `S2late.an3` as the Intertoto's was: no kept save stands the human
    /// in a UEFA tie, and the original's notes of the kind — its 41 ties
    /// of `A.an3` — sit on clubs of unplayed countries, who keep no office.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_uefa_cups_record_sells_at_the_international_column_too() {
        let data = crate::data::Data::load(&std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))).unwrap();
        let mut ours = Records::from_save(Save::read(&keeps().join("S2late.an3")).unwrap()).unwrap();
        patch_columns(&mut ours, (14, 84), [(0x49c, [11, 12, 13, 14i16]), (0x49e, [21, 22, 23, 24]), (0x4a0, [35, 36, 37, 38])]);
        let mut random = Random::new(1);
        open_international(&mut ours, UEFA, (14, 84), (68, 3), 331, 0, 300, &mut random, &data.presale);
        let tie = live(&ours).into_iter().find(|s| s.day == 331).expect("no record for the patched tie");
        assert_eq!((tie.kind, tie.extra, tie.home, tie.away, tie.issuer, tie.prices), (UEFA, 0, (14, 84), (68, 3), (14, 84), [35, 36, 37, 38]));
    }

    /// A supercup tie's record over the list carries kind 4 and the
    /// office's 1 at +0x10, the list's ground as its issuer, and its prices
    /// stand in no column — the fixture list's constants [10, 20, 20, 20],
    /// which the exe reads from `DAT_00a3b420` and thrice `DAT_00a3b418`.
    /// No kept save has a record of the kind, and the original's supercup
    /// notes sit on computer clubs, who keep no office: patched into
    /// `S2late.an3` like the UEFA tie's, with the human's cup column written
    /// apart first to show the record does not read it.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_supercups_record_prices_from_the_fixture_list() {
        let data = crate::data::Data::load(&std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))).unwrap();
        let mut ours = Records::from_save(Save::read(&keeps().join("S2late.an3")).unwrap()).unwrap();
        patch_columns(&mut ours, (14, 84), [(0x49c, [11, 12, 13, 14i16]), (0x49e, [21, 22, 23, 24]), (0x4a0, [31, 32, 33, 34])]);
        let mut random = Random::new(1);
        open_listed(&mut ours, SUPERCUP_TIE, (14, 84), (14, 22), (14, 23), 331, LISTED_SUPERCUP, 300, &mut random, &data.presale);
        let tie = live(&ours).into_iter().find(|s| s.day == 331).expect("no record for the patched tie");
        assert_eq!((tie.kind, tie.extra, tie.home, tie.away, tie.issuer, tie.prices), (SUPERCUP_TIE, 1, (14, 84), (14, 22), (14, 23), [10, 20, 20, 20]));
    }

    /// The branch's other arm, which the exe keeps for a non-German club on
    /// a supercup day without the neutral flag: over no fixture list the
    /// record prices from the home club's cup column like the cup tie's
    /// does, and +0x10 holds a nought. Tested on its own, with the club
    /// 68.3's columns apart.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_supercups_plain_arm_reads_the_cup_column() {
        let data = crate::data::Data::load(&std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))).unwrap();
        let mut ours = Records::from_save(Save::read(&keeps().join("S2late.an3")).unwrap()).unwrap();
        patch_columns(&mut ours, (68, 3), [(0x49c, [11, 12, 13, 14i16]), (0x49e, [21, 22, 23, 24]), (0x4a0, [31, 32, 33, 34])]);
        let mut random = Random::new(1);
        open_supercup(&mut ours, (68, 3), (14, 84), 332, 300, &mut random, &data.presale);
        let tie = live(&ours).into_iter().find(|s| s.day == 332).expect("no record for the patched tie");
        assert_eq!((tie.kind, tie.extra, tie.home, tie.away, tie.issuer, tie.prices), (SUPERCUP_TIE, 0, (68, 3), (14, 84), (68, 3), [21, 22, 23, 24]));
    }

    /// An Intertoto tie goes the same way: the record carries the club's
    /// own kind, 8, sells at the home club's international column and keeps
    /// a nought at +0x10, since the tie is noted without a round. Patched
    /// into `S2late.an3`, whose human club's three columns stand [17, 23,
    /// 43, 150], [15, 25, 25, 25] and [17, 23, 43, 150] — the league's and
    /// the international one equal, so they are written apart first.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn an_intertoto_ties_record_sells_at_the_international_column_too() {
        let data = crate::data::Data::load(&std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))).unwrap();
        let mut ours = Records::from_save(Save::read(&keeps().join("S2late.an3")).unwrap()).unwrap();
        patch_columns(&mut ours, (14, 84), [(0x49c, [11, 12, 13, 14i16]), (0x49e, [21, 22, 23, 24]), (0x4a0, [31, 32, 33, 34])]);
        let mut random = Random::new(1);
        open_international(&mut ours, INTERTOTO, (14, 84), (68, 3), 330, 0, 300, &mut random, &data.presale);
        let tie = live(&ours).into_iter().find(|s| s.day == 330).expect("no record for the patched tie");
        assert_eq!((tie.kind, tie.extra, tie.home, tie.away, tie.issuer, tie.prices), (INTERTOTO, 0, (14, 84), (68, 3), (14, 84), [31, 32, 33, 34]));
    }

    /// The block is a human club's own office: a computer club's cup round
    /// opens nothing there. `S2c.an3` stands the day before a whole round
    /// of the cup two nations away — eleven ties, every club of that
    /// league noting its next match the same day — while its office holds
    /// three of the human's league matches. Noting them all leaves the
    /// block byte for byte as the save has it.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_computer_clubs_cup_round_opens_nothing_in_the_office() {
        let data = crate::data::Data::load(&std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))).unwrap();
        let mut ours = Records::from_save(Save::read(&keeps().join("S2c.an3")).unwrap()).unwrap();
        let before = ours.block(SALES_AT).map(<[u8]>::to_vec).unwrap();
        let seen = live(&ours);
        assert_eq!(seen.len(), 3, "{seen:?}");
        assert!(seen.iter().all(|s| s.kind == LEAGUE));
        let today = (ours.globals.day() as usize).saturating_sub(1);
        let mut random = Random::new(1);
        crate::year::note_next_matches(&mut ours, &data, today, &mut random);
        assert_eq!(ours.block(SALES_AT).map(<[u8]>::to_vec), Some(before));
    }
}
