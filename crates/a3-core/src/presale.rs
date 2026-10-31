//! The ticket office's advance sales (`0x1c5c468`, thirty records of 0x24
//! bytes; KALKULATION.md in the RE repo): at the week's end each human club
//! opens records for its league matches of the next four weeks, at home and
//! away (`FUN_0077ca2b`), and every day the records of the next week sell
//! on (`FUN_0077c8b5`); a record whose day has passed is let go. How many
//! sell comes from the attendance model (`FUN_0078e5e6`) and the luck of the
//! game's generator, so the sales draw in the original's order.
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
    own + guests
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
    let (lh, la) = (fan_class(home, away, t), fan_class(away, home, t));
    if sale.extra == FINAL {
        return 250_000 + 250_000;
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
    over(h, s) + guests(a, w_side, w_stand)
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

/// What the club's stadium holds (`FUN_00638335`, the stadium's +0x3c).
fn capacity(records: &Records, side: &Side) -> i32 {
    records
        .team_index(side.country.into(), side.number.into())
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
/// makes its first sale; whether one was opened.
fn open(
    records: &mut Records,
    home: (u8, i16),
    away: (u8, i16),
    day: i16,
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
        r[SALE_ISSUER] = home.0;
        r[SALE_ISSUER + 2..SALE_ISSUER + 4].copy_from_slice(&home.1.to_le_bytes());
        r[SALE_DAY..SALE_DAY + 2].copy_from_slice(&day.to_le_bytes());
        r[SALE_KIND..SALE_KIND + 2].copy_from_slice(&LEAGUE.to_le_bytes());
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
            if !open(records, h, a, day as i16, today, random, t) {
                continue;
            }
            let Some(own) = Side::of(records, club.0, club.1) else { continue };
            let mark = crate::tickets::matchday_mark(own.team, k as i32 + 1);
            let factor = t.mark_factors.get(mark).copied().unwrap_or(1.0);
            let Some(host) = Side::of(records, h.0, h.1) else { continue };
            let prices = host.prices().map(|p| ftol(f64::from(p) * f64::from(factor)) as i16);
            let found = records.block(SALES_AT).and_then(|b| {
                (0..SALES).find_map(|k| read(b, k).filter(|s| s.home == h && s.away == a && s.day == day as i16))
            });
            if let (Some(s), Some(b)) = (found, records.block_mut(SALES_AT)) {
                for (i, p) in prices.iter().enumerate() {
                    let at = s.at * SALE + SALE_PRICES + 2 * i;
                    b[at..at + 2].copy_from_slice(&p.to_le_bytes());
                }
            }
        }
    }
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
