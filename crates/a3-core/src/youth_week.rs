//! A club's youth through the season (`FUN_006c41b1`): a computer club
//! of the main and the more countries with no human walks it in the
//! clubs' week on the market (`FUN_004297e1`, before its fresh terms), a
//! human club in its day's news (`FUN_005ef8cc`), told what came of it.
//! Outside June and July, a club that is no reserve team draws one in ten
//! thousand each week, and by the draw a youth of its youth team grows
//! (9000 and over), one is hurt for months (2500 to 2504), one out hurt
//! loses a point and may give up (2600 to 2609), or, the richer the club
//! the likelier, it takes a new youth — three a season at most; a club
//! too poor for one whose manager is a human's may have its talent scout
//! find one. On 1000,
//! with no new youth taken, a club of the first two leagues poaches one
//! of the youth list from his club, the human's told. A week that comes
//! to nothing else, a club with no amateur team works its pool of
//! amateurs: a new one, a status up or down, an amateur hurt or slipping.

use crate::event_text::{Fill, Posted};
use crate::random::Random;
use crate::records::{Player, Records};

/// The months the youth rests (`_DAT_00b5cbdc`).
const SUMMER: [u16; 2] = [6, 7];
/// The week's draw and its bands.
const DRAW: u32 = 10_000;
const GROWS_FROM: u32 = 9000;
const HURT: std::ops::RangeInclusive<u32> = 2500..=2504;
const POACHED: std::ops::RangeInclusive<u32> = 2600..=2609;
/// A club takes a new youth while its support in these (`DAT_00a3b37c`)
/// is no less than the draw in fifties; a fifth more with the team flag
/// +0x95c's bit 1 (`FUN_0069f177`).
const SUPPORT_UNIT: i32 = 100_000;
const DRAW_UNIT: i32 = 50;
const FLAG_AT: usize = 0x95c;
const FLAG: u8 = 2;

/// The draw on which a club poaches, and the leagues it does so in
/// (club +0x105 under 2).
const POACHES: u32 = 1000;
const POACHING_LEVELS: u8 = 2;

/// The talent scout's sum the plan carries (club +0x26c, KALKULATION.md),
/// in [`SUPPORT_UNIT`]s: a club too poor for a new youth, its manager a
/// human, finds one on a draw of a hundred under two fifths of it.
const SCOUT: usize = 0x26c + crate::finances::CLUB_TO_TEAM;
const SCOUT_ODDS: u32 = 100;
/// The event of the youth news a human club is told, and its texts in
/// `ereignis.saq`: a new youth (`FUN_005f04c2`), one grown to a strength
/// (`FUN_005f04e0`), one hurt (`FUN_005f0574`), one set back a point
/// (`FUN_005f0592`) or giving up (`FUN_005f0538`); a new amateur
/// (`FUN_005f05b0`), one hurt (`FUN_005f060a`) or giving up
/// (`FUN_005f05ec`).
const YOUTH_EVENT: u16 = 0x60;
const TAKEN_TEXT: u16 = 0x35c;
const GROWN_TEXT: u16 = 0x35d;
const GAVE_UP_TEXT: u16 = 0x35f;
const HURT_TEXT: u16 = 0x361;
const SET_BACK_TEXT: u16 = 0x362;
const AMATEUR_TEXT: u16 = 0x363;
const AMATEUR_HURT_TEXT: u16 = 0x368;
const AMATEUR_GONE_TEXT: u16 = 0x36a;

/// What a human club is told of its youth's week, with `news`
/// (`FUN_006c41b1`'s second word).
struct Told<'a> {
    club: (u16, u16),
    news: bool,
    posted: &'a mut Vec<Posted>,
}

impl Told<'_> {
    fn post(&mut self, text: u16, fill: Fill) {
        if self.news {
            self.posted.push(Posted { club: self.club, code: YOUTH_EVENT, text, fill });
        }
    }
}

/// One club's week for its youth (`FUN_006c41b1`); with `news` the club
/// is a human's and is told what came of it, its trainer's and manager's
/// standing moving with it. A youth poached from a human club is told to
/// that one in `posted` either way.
pub(crate) fn walk(records: &mut Records, (country, number): (u8, u16), news: bool, random: &mut Random, posted: &mut Vec<Posted>) {
    let [_, month, _] = records.globals.date();
    if SUMMER.contains(&month) {
        return;
    }
    let Some(team) = records.team(u16::from(country), number) else { return };
    if team.linked()[0] >= 0 {
        return;
    }
    let draw = random.below(DRAW);
    let support = team.get::<i32>(crate::newplayer::YOUTH_SUPPORT);
    let (manager, scout) = (team.humans()[1], team.get::<i32>(SCOUT));
    let mut can = (support / SUPPORT_UNIT) as i16;
    if team.get::<u8>(FLAG_AT) & FLAG != 0 {
        can += can / 5;
    }
    let standing = (country, number as i16);
    let mut told = Told { club: (u16::from(country), number), news, posted };
    if draw >= GROWS_FROM {
        if let Some(id) = grow(records, (country, number), support, random) {
            // The trainer a point up with the public and the fans
            // (`FUN_0041828e`), the youth's new strength told.
            if news {
                crate::training_day::move_likings(records, told.club, [1, 1, 0]);
            }
            let strength = records.player(id).map_or(0, |p| p.strength());
            told.post(GROWN_TEXT, Fill::NamedNumber(id, i32::from(strength)));
            return;
        }
    } else {
        if HURT.contains(&draw) {
            if let Some(id) = hurt(records, (country, number), random) {
                told.post(HURT_TEXT, Fill::Named(id));
            }
            return;
        }
        if POACHED.contains(&draw) {
            slip(records, (country, number), &mut told, random);
            return;
        }
        if i32::from(can) < draw as i32 / DRAW_UNIT {
            // Too poor for a new youth: a human manager's talent scout
            // may find one (`FUN_006c35fc`), the trainer and the manager
            // two up with the public and the fans (`FUN_00418065`).
            if manager != 0 && (random.below(SCOUT_ODDS) as i32) < scout / SUPPORT_UNIT * 2 / 5 {
                crate::exhibition::move_standing(records, standing, [2, 2, 0], 0);
                scouted(records, (country, number), random);
            }
        } else if let Some(id) = take(records, (country, number), random) {
            if news {
                crate::exhibition::move_standing(records, standing, [1, 1, 0], 0);
            }
            told.post(TAKEN_TEXT, Fill::Named(id));
            return;
        }
    }
    if draw != POACHES {
        amateurs(records, (country, number), &mut told, random);
        return;
    }
    let level = records.team(u16::from(country), number).map_or(POACHING_LEVELS, |t| t.level());
    if level >= POACHING_LEVELS {
        return;
    }
    // The club he came from told, if a human's (`FUN_00677428`,
    // `FUN_005f0556`).
    if let Some((id, (from, from_number))) = poach(records, (country, number), random)
        && from != 0
        && crate::statistics::is_human(records, from, i16::from(from_number))
    {
        told.posted.push(Posted { club: (u16::from(from), u16::from(from_number)), code: POACHED_EVENT, text: POACHED_TEXT, fill: Fill::Named(id) });
    }
}

/// What the club plans for its amateurs (club +0x294) counts in these
/// (`DAT_00a3b36c`), ten at most.
const AMATEUR_BUDGET: usize = 0x294 + crate::finances::CLUB_TO_TEAM;
const BUDGET_UNIT: i32 = 200_000;
const BUDGET_MOST: i16 = 10;
/// The draws: a new amateur on 400 (up to the budget), a status on 30
/// (under five less half the budget), a mishap on 500; one not hurt is
/// hurt on one in five.
const NEW_DRAW: u32 = 400;
const STATUS_DRAW: u32 = 30;
const STATUS_ODDS: i32 = 5;
const MISHAP_DRAW: u32 = 500;
const HURT_ODDS: u32 = 5;
/// A status rises unless a draw of thirteen less the budget is above
/// three a status and 28 less his age; up to strength 8.
const STATUS_RANGE: i32 = 13;
const STATUS_WEIGHT: i32 = 3;
const STATUS_AGE: i32 = 28;
const STATUS_STRONGEST: u8 = 8;
/// R+0x271's low bits: on trial with the professionals.
const ON_TRIAL: u8 = 0x0f;
/// One who slips at the lowest status below this gives up.
const GIVES_UP_BELOW: u8 = 3;

/// A club with no amateur team in the leagues (team +0x873 below 0,
/// `FUN_0063e7cf`) works its amateur pool (`FUN_006c41b1`'s tail), the
/// more the more it plans for them: on a draw of 400 up to that and
/// under five in its pool, a new amateur (and no more); else on a draw
/// of 30 under five less half of it, an amateur of the pool not hurt
/// rises or falls a status; else on one in 500, one of the pool not hurt
/// is hurt for months one time in five, or else may slip
/// (`FUN_006c3daf`).
fn amateurs(records: &mut Records, (country, number): (u8, u16), told: &mut Told, random: &mut Random) {
    let Some(team) = records.team(u16::from(country), number) else { return };
    if team.linked()[1] >= 0 {
        return;
    }
    let budget = i32::from(((team.get::<i32>(AMATEUR_BUDGET) / BUDGET_UNIT) as i16).min(BUDGET_MOST));
    if random.below(NEW_DRAW) as i32 <= budget {
        let [up, pool] = crate::youth::pool(records, country, number);
        if up.len() + pool.len() < crate::youth::POOL {
            if let Some(id) = crate::newplayer::amateur_for(records, (country, number), random) {
                told.post(AMATEUR_TEXT, Fill::Named(id));
            }
            return;
        }
    }
    if (random.below(STATUS_DRAW) as i32) < STATUS_ODDS - budget / 2 {
        rise_or_fall(records, (country, number), budget, random);
    } else if random.below(MISHAP_DRAW) == 0 {
        mishap(records, (country, number), told, random);
    }
}

/// Days he is still out hurt (L+0x2c).
fn out_hurt(records: &Records, p: &Player) -> bool {
    crate::load::days_to(&records.globals, p.injured_to()) != 0
}

/// One of the pool's amateurs not hurt drawn: the older he is the likelier
/// he falls a status and a point, else he rises one and a point, within
/// the statuses and strength 8.
fn rise_or_fall(records: &mut Records, (country, number): (u8, u16), budget: i32, random: &mut Random) {
    let [_, pool] = crate::youth::pool(records, country, number);
    let fit: Vec<u16> = pool.into_iter().filter(|&id| records.player(id).is_some_and(|p| !out_hurt(records, p))).collect();
    if fit.is_empty() {
        return;
    }
    let id = fit[random.below(fit.len() as u32) as usize];
    let Some(today) = crate::free_agents::today(records) else { return };
    let Some(p) = records.player(id) else { return };
    let mut status = crate::newplayer::status(p);
    let r = random.below((STATUS_RANGE - budget) as u32) as i32;
    let age = crate::statistics::age_on(p, today);
    let mut strength = p.strength();
    if i32::from(status) * STATUS_WEIGHT + STATUS_AGE - age < r {
        if status > 0 {
            status -= 1;
            strength = strength.saturating_sub(1);
        }
    } else if status < crate::newplayer::STATUS_MOST {
        status += 1;
        if strength < STATUS_STRONGEST {
            strength += 1;
        }
    }
    let Some(p) = records.player_mut(id) else { return };
    p.set_strength(strength);
    crate::newplayer::set_status(p, i32::from(status));
}

/// One of the pool drawn: not hurt, one time in five he is out three to six
/// months; else he may slip.
fn mishap(records: &mut Records, (country, number): (u8, u16), told: &mut Told, random: &mut Random) {
    let [_, pool] = crate::youth::pool(records, country, number);
    if pool.is_empty() {
        return;
    }
    let id = pool[random.below(pool.len() as u32) as usize];
    let Some(p) = records.player(id) else { return };
    if !out_hurt(records, p) && random.below(HURT_ODDS) == 0 {
        let days = (random.below(HURT_MONTHS) + HURT_FROM) as i32 * MONTH;
        let today = i32::from(records.globals.day());
        crate::training_day::injure(records, id, HURT_KIND, days, today);
        if let Some(p) = records.player_mut(id) {
            p.set_retiring(p.retiring() & !HURT_CLEARS);
        }
        told.post(AMATEUR_HURT_TEXT, Fill::Named(id));
    } else {
        slip_amateur(records, (country, number), id, told, random);
    }
}

/// An amateur out hurt and not on trial slips (`FUN_006c3daf`), once for
/// the injury: a status and a point down; at the lowest status and under
/// strength 3 he gives up — out of the pool, his career ended
/// (`FUN_0070e31e`, `FUN_0066d975`).
fn slip_amateur(records: &mut Records, club: (u8, u16), id: u16, told: &mut Told, random: &mut Random) {
    let Some(p) = records.player(id) else { return };
    if p.picks() & ON_TRIAL != 0 || !out_hurt(records, p) || p.retiring() & SLIPPED != 0 {
        return;
    }
    let status = i32::from(crate::newplayer::status(p)) - 1;
    let strength = p.strength().wrapping_sub(1);
    let Some(p) = records.player_mut(id) else { return };
    p.set_strength(strength);
    p.set_retiring(p.retiring() | SLIPPED);
    if status < 1 && strength < GIVES_UP_BELOW {
        told.post(AMATEUR_GONE_TEXT, Fill::Named(id));
        crate::youth::leave_pool(records, club, id);
        crate::rollover::end_career(records, id, random);
        return;
    }
    crate::newplayer::set_status(p, status);
}

/// The event that tells a human club a youth of his was poached, and its
/// text in `ereignis.saq` (`FUN_005f0556` → `FUN_005ee5c0`).
const POACHED_EVENT: u16 = 0x60;
const POACHED_TEXT: u16 = 0x360;
/// Youths at their club over two months may slip (`FUN_006ff5d5`); one
/// the club does not support weighs twice (`FUN_00706ba6`).
const SETTLED_MONTHS: i32 = 2;
const WEIGHT: u32 = 2;
const SUPPORTED_WEIGHT: u32 = 1;
/// R+0x26f's bit: he has lost his point for this injury (the youth's hurt
/// clears it, [`HURT_CLEARS`]).
const SLIPPED: u8 = 4;
/// A youth without a station counts his months from this day, in thirty
/// days (`FUN_006ff5d5`).
const NO_STATION: (i32, u32, u32) = (1997, 7, 1);

/// Calendar months at his club, the first and the last counted, by his
/// last station's start (`FUN_006ff5d5`, `FUN_006758f3`).
fn months_at_club(records: &Records, p: &Player) -> i32 {
    let [_, month, year] = records.globals.date();
    let count = p.career_count();
    if count < 1 {
        let Some(today) = crate::free_agents::today(records) else { return 0 };
        let (y, m, d) = NO_STATION;
        let since = chrono::NaiveDate::from_ymd_opt(y, m, d).map_or(0, |d| (today - d).num_days());
        return (since / 30) as i32;
    }
    let at = 0x20 + (count as usize - 1) * 0x14;
    let (begun_month, begun_year) = (p.get::<u8>(at + 1), p.get::<u16>(at + 2));
    (i32::from(year) - i32::from(begun_year)) * 12 + i32::from(month) - i32::from(begun_month) + 1
}

/// A youth out hurt loses a point (`FUN_006c38f7`): one of those at the
/// club over two months is drawn, the supported weighing half; out hurt
/// and not yet set back for it, he is a point weaker, and at nought he
/// stays at one on half the draws, else he gives up — out of the youth
/// team and off the list, his career ended (`FUN_0066aea3`,
/// `FUN_0076408d`, `FUN_0066d975`), the trainer three down with the
/// public and the fans (`FUN_0041828e`). A human club is told either.
fn slip(records: &mut Records, (country, number): (u8, u16), told: &mut Told, random: &mut Random) {
    let (youths, _) = crate::youth::youths(records, country, number);
    let mut total = 0;
    let mut drawn = Vec::new();
    for id in youths {
        let Some(p) = records.player(id) else { continue };
        if months_at_club(records, p) > SETTLED_MONTHS {
            total += if p.shown() & crate::youth::SUPPORTED == 0 { WEIGHT } else { SUPPORTED_WEIGHT };
            drawn.push((total, id));
        }
    }
    if total == 0 {
        return;
    }
    let r = random.below(total);
    let Some(&(_, id)) = drawn.iter().find(|&&(upto, _)| upto > r).or(drawn.last()) else { return };
    let Some(p) = records.player(id) else { return };
    if crate::load::days_to(&records.globals, p.injured_to()) == 0 || p.retiring() & SLIPPED != 0 {
        return;
    }
    let strength = p.strength().wrapping_sub(1);
    let gives_up = strength == 0 && random.below(2) != 0;
    if let Some(p) = records.player_mut(id) {
        p.set_strength(if strength == 0 && !gives_up { 1 } else { strength });
        p.set_retiring(p.retiring() | SLIPPED);
    }
    if strength != 0 {
        told.post(SET_BACK_TEXT, Fill::Named(id));
    }
    if gives_up {
        if told.news {
            crate::training_day::move_likings(records, told.club, [-3, -3, 0]);
        }
        told.post(GAVE_UP_TEXT, Fill::Named(id));
        crate::youth::leave_youth(records, (country, number), id);
        crate::youth::drop_prospect(records, id);
        crate::rollover::end_career(records, id, random);
    }
}

/// A poached youth's contract runs ten years (`FUN_006c3bab`'s +0x94).
const POACHED_YEARS: u16 = 10;
const SALARY_UNIT: i32 = 1000;

/// A youth of the list poached (`FUN_006c3bab`, `FUN_007640ee`): one of
/// its entries drawn, unless he is the club's own; out of his club's youth
/// team into this one's, off the list, on what his parents agree to for
/// its league and ten years. Who and the club he came from.
fn poach(records: &mut Records, (country, number): (u8, u16), random: &mut Random) -> Option<(u16, (u8, u8))> {
    let listed: Vec<u16> = crate::youth::prospects(records).into_iter().map(|p| p.id).filter(|&id| id as i16 > 0).collect();
    if listed.is_empty() {
        return None;
    }
    let id = listed[random.below(listed.len() as u32) as usize];
    let [from, from_number] = records.player(id)?.club();
    if (from, from_number) == (country, number as u8) {
        return None;
    }
    if from != 0 {
        crate::youth::leave_youth(records, (from, u16::from(from_number)), id);
        crate::youth::join_youth(records, (country, number), id);
    }
    crate::youth::drop_prospect(records, id);
    if from == 0 {
        return None;
    }
    let level = records.team(u16::from(country), number).map_or(0, |t| t.level());
    let year = records.globals.start_year();
    let p = records.player(id)?;
    let salary = crate::youth::salary(id, crate::free_agents::age(records, p), level) / SALARY_UNIT;
    let p = records.player_mut(id)?;
    p.set_salary(salary as u16);
    p.set_contract(year + POACHED_YEARS);
    Some((id, (from, from_number)))
}

/// The support's bands a youth's growth is judged by (`FUN_006c4103`:
/// `DAT_00a3b36c`, `DAT_00a3b350`, `DAT_00a3b334`, `DAT_00a3b324`), and
/// per band the two marks a draw must come under (`DAT_008a59f8`).
const BANDS: [i32; 4] = [200_000, 400_000, 800_000, 1_400_000];
const MARKS: [[u32; 2]; 5] = [[35, 12], [50, 15], [70, 18], [90, 21], [110, 24]];
/// The draws' ranges by the team's +0x8ef (at most 3, `FUN_0069aa8b`)
/// and the youth's talent (`DAT_008a5a0c`), and the strength each talent
/// grows to (`DAT_008a7560`).
const RANGES: [[u32; 5]; 4] = [
    [400, 600, 800, 1000, 1200],
    [500, 750, 1000, 1250, 1500],
    [600, 900, 1200, 1500, 1800],
    [700, 1050, 1400, 1750, 2100],
];
const RANGES_AT: usize = 0x8ef;
const MOST_STRENGTH: [u8; 5] = [10, 9, 8, 7, 6];
/// A supported youth born from the first of July fourteen years before
/// the season's year (`FUN_00706943`) halves his draw, an older one takes
/// two thirds.
const SUPPORTED_AGE: u16 = 14;
const SEASON_MONTH: u8 = 7;

/// A youth of the club grows a point (`FUN_006c4103`, `FUN_006c3f34`):
/// each youth below his talent's most draws by his talent, and once more
/// if over the first mark; those under a mark are drawn from. The one
/// who grew.
fn grow(records: &mut Records, (country, number): (u8, u16), support: i32, random: &mut Random) -> Option<u16> {
    let band = BANDS.iter().take_while(|&&b| support >= b).count();
    let team = records.team(u16::from(country), number)?;
    let row = RANGES[usize::from(team.get::<u8>(RANGES_AT).min(3))];
    let year = records.globals.start_year();
    let (youths, _) = crate::youth::youths(records, country, number);
    let mut grown = Vec::new();
    for id in youths {
        let Some(p) = records.player(id) else { continue };
        let Some(talent) = usize::try_from(p.talent()).ok().filter(|&t| t < MOST_STRENGTH.len()) else { continue };
        if p.strength() >= MOST_STRENGTH[talent] {
            continue;
        }
        let mut r = random.below(row[talent]);
        if p.shown() & crate::youth::SUPPORTED != 0 {
            let young = (p.birth_year(), p.birth_month()) >= (year.wrapping_sub(SUPPORTED_AGE), SEASON_MONTH);
            r = if young { r >> 1 } else { (r << 1) / 3 };
        }
        if r < MARKS[band][0] || random.below(row[talent]) < MARKS[band][1] {
            grown.push(id);
        }
    }
    if grown.is_empty() {
        return None;
    }
    let id = grown[random.below(grown.len() as u32) as usize];
    if let Some(p) = records.player_mut(id) {
        p.set_strength(p.strength().wrapping_add(1));
    }
    Some(id)
}

/// A youth hurt: months out, his kind 1 (`FUN_006c3706`, `FUN_00766b92`).
const HURT_KIND: u8 = 1;
const HURT_MONTHS: u32 = 4;
const HURT_FROM: u32 = 3;
const MONTH: i32 = 30;
/// R+0x26f's bit the youth loses with it.
const HURT_CLEARS: u8 = 4;

/// One of the youth team drawn, unless already out, is out three to six
/// months (`FUN_006c3706`); the one hurt.
fn hurt(records: &mut Records, (country, number): (u8, u16), random: &mut Random) -> Option<u16> {
    let (youths, _) = crate::youth::youths(records, country, number);
    if youths.is_empty() {
        return None;
    }
    let id = youths[random.below(youths.len() as u32) as usize];
    let p = records.player(id)?;
    if crate::load::days_to(&records.globals, p.injured_to()) != 0 {
        return None;
    }
    let days = (random.below(HURT_MONTHS) + HURT_FROM) as i32 * MONTH;
    let today = i32::from(records.globals.day());
    crate::training_day::injure(records, id, HURT_KIND, days, today);
    if let Some(p) = records.player_mut(id) {
        p.set_retiring(p.retiring() & !HURT_CLEARS);
    }
    Some(id)
}

/// A new youth while the club has made under three this season
/// (`FUN_006c3500`, `FUN_0066c0c2`); the one taken.
fn take(records: &mut Records, (country, number): (u8, u16), random: &mut Random) -> Option<u16> {
    let made = crate::youth::made(records, country, number);
    if made >= crate::youth::MOST_MADE {
        return None;
    }
    let id = crate::newplayer::youth_for(records, (country, number), random)?;
    crate::youth::set_made(records, country, number, made + 1);
    Some(id)
}

/// The talents a scout's youth comes with (`FUN_0066b1bf`): a draw of a
/// hundred under 20 the greatest, under 70 the next, else the third.
const SCOUTED_TALENTS: [(u32, i8); 2] = [(20, 0), (70, 1)];
const SCOUTED_LEAST: i8 = 2;

/// A youth the talent scout found (`FUN_006c35fc`): taken as any new
/// youth, his talent drawn among the three greatest. Told to no one.
fn scouted(records: &mut Records, club: (u8, u16), random: &mut Random) {
    let Some(id) = take(records, club, random) else { return };
    let r = random.below(100);
    let talent = SCOUTED_TALENTS.iter().find(|&&(under, _)| r < under).map_or(SCOUTED_LEAST, |&(_, t)| t);
    if let Some(p) = records.player_mut(id) {
        p.set_talent(talent);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// From the original's save A (8 September) through the season's
    /// weeks to its end (the 37 outside June and July): the computer clubs
    /// of the played countries take about as many youths as the
    /// original's did — 163 more made at its computer clubs, their youth
    /// teams 621 strong at the end, 33 clubs at three —, the youths kept
    /// from A grow about as much as there, and a few are hurt for months.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_youths_grow_through_the_season_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (a, theirs) = (load("A.an3"), load("SeasonEnd364.an3"));
        let countries = crate::opponent::played_nations(&a);
        let clubs: Vec<(u8, u16)> = countries
            .iter()
            .flat_map(|&c| (0..a.countries.team_count(usize::from(c)).max(0) as u16).map(move |n| (c as u8, n)))
            .filter(|&(c, n)| a.team(u16::from(c), n).is_some_and(|t| t.humans() == [0, 0]))
            .collect();
        let tally = |r: &Records| {
            let (mut youths, mut made, mut full, mut strength, mut kept) = (0, 0, 0, 0, 0);
            for &(c, n) in &clubs {
                let (ids, _) = crate::youth::youths(r, c, n);
                youths += ids.len();
                let m = crate::youth::made(r, c, n);
                made += usize::from(m);
                full += usize::from(m == crate::youth::MOST_MADE);
                let (before, _) = crate::youth::youths(&a, c, n);
                for id in ids.into_iter().filter(|id| before.contains(id)) {
                    kept += 1;
                    strength += i32::from(r.player(id).unwrap().strength()) - i32::from(a.player(id).unwrap().strength());
                }
            }
            (youths, made, full, f64::from(strength) / f64::from(kept))
        };
        let (mut ours, mut hurt) = (load("A.an3"), 0);
        let mut random = Random::new(5);
        for _ in 0..37 {
            for &club in &clubs {
                walk(&mut ours, club, false, &mut random, &mut Vec::new());
            }
        }
        for &(c, n) in &clubs {
            for id in crate::youth::youths(&ours, c, n).0 {
                let p = ours.player(id).unwrap();
                hurt += usize::from(p.injury_kind() == HURT_KIND && crate::load::days_to(&ours.globals, p.injured_to()) >= 90);
            }
        }
        let pools = |r: &Records| {
            let (mut n, mut new, mut status, mut strength) = (0, 0, [0; 3], 0);
            for &(c, n_) in &clubs {
                let [up, am] = crate::youth::pool(r, c, n_);
                let [_, before] = crate::youth::pool(&a, c, n_);
                n += am.len() + up.len();
                for id in am {
                    let p = r.player(id).unwrap();
                    new += usize::from(!before.contains(&id));
                    status[usize::from(crate::newplayer::status(p).min(2))] += 1;
                    strength += i32::from(p.strength());
                }
            }
            (n, new, status, strength)
        };
        let (youths, made, full, grown) = tally(&ours);
        let (their_youths, their_made, their_full, their_grown) = tally(&theirs);
        let (_, made_at_a, _, _) = tally(&a);
        let near = |x: f64, y: f64, by: f64| (x / y - 1.0).abs() < by;
        assert!(near((made - made_at_a) as f64, (their_made - made_at_a) as f64, 0.2), "made {made} against {their_made}");
        assert!(near(youths as f64, their_youths as f64, 0.05), "youths {youths} against {their_youths}");
        assert!(near(full as f64, their_full as f64, 0.5), "full {full} against {their_full}");
        assert!(near(grown, their_grown, 0.25), "grown {grown:.2} against {their_grown:.2}");
        assert!((1..12).contains(&hurt), "{hurt}");
        // The clubs with no amateur team fill their pools and lift their
        // amateurs about as the original's did (238 in the pools, 33 new,
        // 89 stars, strength 921 together).
        let ((n, new, status, strength), (their_n, their_new, their_status, their_strength)) = (pools(&ours), pools(&theirs));
        assert!(near(n as f64, their_n as f64, 0.1), "pools {n} against {their_n}");
        assert!(near(new as f64, their_new as f64, 0.4), "new {new} against {their_new}");
        assert!(near(status[2] as f64, their_status[2] as f64, 0.35), "stars {status:?} against {their_status:?}");
        assert!(near(f64::from(strength), f64::from(their_strength), 0.1), "strength {strength} against {their_strength}");
        // A new youth stands in his club's youth team with a youth's
        // station of his class.
        let (c, n) = clubs.iter().copied().find(|&(c, n)| crate::youth::made(&ours, c, n) > crate::youth::made(&a, c, n)).unwrap();
        let (ids, _) = crate::youth::youths(&ours, c, n);
        let id = *ids.last().unwrap();
        let p = ours.player(id).unwrap();
        assert_eq!(p.club(), [c, n as u8]);
        let at = 0x20 + (p.career_count() as usize - 1) * 0x14;
        assert_eq!(p.get::<u8>(at + 0x12), 9 + crate::youth::class(&ours, id) as u8);
        assert_eq!(p.get::<u8>(at + 0x13), p.strength() + 0x40);
        assert!((1..=5).contains(&p.strength()) && p.salary() == 0 && p.retiring() == 8);
    }

    /// On save A a first-league computer club, its three youths made, draws
    /// 1000: it poaches the one youth of the list, a human club's, who
    /// moves into its youth team on ten years and his parents' terms, and
    /// the human is told by event 0x60's text naming him. A youth out hurt
    /// a point above nought slips one, once for the injury; at one he
    /// stays or gives up.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_club_poaches_a_humans_youth_and_an_injured_youth_slips() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let mut r = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let (hc, hn) = crate::statistics::human_clubs(&r)[0];
        let id = crate::youth::youths(&r, hc, hn as u16).0[0];
        for p in crate::youth::prospects(&r) {
            crate::youth::drop_prospect(&mut r, p.id);
        }
        crate::youth::add_prospect(&mut r, id, (hc, hn as u16));
        let club = crate::opponent::played_nations(&r)
            .into_iter()
            .flat_map(|c| (0..r.countries.team_count(usize::from(c)).max(0) as u16).map(move |n| (c as u8, n)))
            .find(|&(c, n)| r.team(u16::from(c), n).is_some_and(|t| t.humans() == [0, 0] && t.level() == 0 && t.linked()[0] < 0))
            .unwrap();
        crate::youth::set_made(&mut r, club.0, club.1, crate::youth::MOST_MADE);
        let seed = (0..1_000_000).find(|&s| Random::new(s).below(DRAW) == POACHES).unwrap();
        let mut posted = Vec::new();
        walk(&mut r, club, false, &mut Random::new(seed), &mut posted);
        assert!(crate::youth::youths(&r, club.0, club.1).0.contains(&id));
        assert!(!crate::youth::youths(&r, hc, hn as u16).0.contains(&id));
        assert!(crate::youth::prospects(&r).is_empty());
        let p = r.player(id).unwrap();
        assert_eq!(p.club(), [club.0, club.1 as u8]);
        let salary = crate::youth::salary(id, crate::free_agents::age(&r, p), 0) / 1000;
        assert_eq!((p.salary(), p.contract()), (salary as u16, r.globals.start_year() + 10));
        assert_eq!(posted, [Posted { club: (u16::from(hc), hn as u16), code: 0x60, text: 0x360, fill: Fill::Named(id) }]);
        let texts = a3_files::saq::EventTexts::load(&dir).unwrap();
        let told = crate::event_text::make(&r, &texts, &|_| String::new(), &[], &posted[0]);
        assert!(told.text.contains(&r.known_as(id).unwrap()), "{}", told.text);

        // Every youth of the club out hurt: the one drawn slips a point,
        // and not again for the same injury.
        let today = i32::from(r.globals.day());
        let youths = crate::youth::youths(&r, club.0, club.1).0;
        for &y in &youths {
            crate::training_day::injure(&mut r, y, HURT_KIND, 60, today);
            let p = r.player_mut(y).unwrap();
            p.set_strength(3);
            p.set_retiring(p.retiring() & !SLIPPED);
        }
        slip(&mut r, club, &mut untold(&mut Vec::new()), &mut Random::new(1));
        let slipped: Vec<u16> = youths.iter().copied().filter(|&y| r.player(y).unwrap().retiring() & SLIPPED != 0).collect();
        assert_eq!(slipped.len(), 1);
        let y = slipped[0];
        assert_eq!(r.player(y).unwrap().strength(), 2);
        let before = r.player(y).unwrap().strength();
        for _ in 0..50 {
            slip(&mut r, club, &mut untold(&mut Vec::new()), &mut Random::new(1));
        }
        assert_eq!(r.player(y).unwrap().strength(), before);
        // At one he stays at one or gives up his career.
        let (mut stayed, mut gone, mut random) = (0, 0, Random::new(7));
        for _ in 0..40 {
            let youths = crate::youth::youths(&r, club.0, club.1).0;
            for &y in &youths {
                let p = r.player_mut(y).unwrap();
                p.set_strength(1);
                p.set_retiring(p.retiring() & !SLIPPED);
            }
            slip(&mut r, club, &mut untold(&mut Vec::new()), &mut random);
            let now = crate::youth::youths(&r, club.0, club.1).0;
            if let Some(left) = youths.iter().copied().find(|y| !now.contains(y)) {
                gone += 1;
                assert_eq!(r.player(left).unwrap().strength(), 0);
                assert!(!crate::youth::prospects(&r).iter().any(|p| p.id == left));
            } else if youths.iter().any(|&y| r.player(y).unwrap().retiring() & SLIPPED != 0) {
                stayed += 1;
                assert!(youths.iter().all(|&y| r.player(y).unwrap().strength() == 1));
            }
        }
        assert!(stayed > 0 && gone > 0, "{stayed} stayed, {gone} gone");
    }

    /// On save A a computer club with no amateur team takes a new amateur
    /// into its pool: 20 to 29, the lowest status known, his strength by
    /// it, an amateur's station at the club. Young, he rises to a star;
    /// past 34 he falls back; out hurt he slips once, and at the lowest
    /// status under strength 3 he gives up.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_club_works_its_amateur_pool() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let mut r = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let club = crate::opponent::played_nations(&r)
            .into_iter()
            .flat_map(|c| (0..r.countries.team_count(usize::from(c)).max(0) as u16).map(move |n| (c as u8, n)))
            .find(|&(c, n)| r.team(u16::from(c), n).is_some_and(|t| t.humans() == [0, 0] && t.linked() == [-1, -1]))
            .unwrap();
        crate::youth::store_pool(&mut r, club.0, club.1, &[vec![], vec![]]);
        let mut random = Random::new(3);
        let id = crate::newplayer::amateur_for(&mut r, club, &mut random).unwrap();
        assert_eq!(crate::youth::pool(&r, club.0, club.1), [vec![], vec![id]]);
        let today = crate::free_agents::today(&r).unwrap();
        let p = r.player(id).unwrap();
        assert!((20..30).contains(&crate::statistics::age_on(p, today)));
        assert_eq!((p.picks(), p.retiring(), p.salary(), p.contract()), (0x40, 8, 0, r.globals.start_year()));
        assert!((1..=6).contains(&p.strength()) && p.positions()[1..] == [0, 0]);
        let at = 0x20 + (p.career_count() as usize - 1) * 0x14;
        assert_eq!(p.get::<[u8; 4]>(at + 0x10), [club.0, club.1 as u8, 0, p.strength() + 0x80]);

        // Rich in amateurs, the club's pool fills to five and no more.
        r.team_mut(u16::from(club.0), club.1).unwrap().put::<i32>(AMATEUR_BUDGET, 2_000_000);
        for _ in 0..2000 {
            amateurs(&mut r, club, &mut untold(&mut Vec::new()), &mut random);
            let [up, pool] = crate::youth::pool(&r, club.0, club.1);
            assert!(up.len() + pool.len() <= 5);
        }
        assert_eq!(crate::youth::pool(&r, club.0, club.1)[1].len(), 5);

        // One amateur alone in the pool: at 20 he rises to a star, two
        // points stronger; at 35 he falls to the lowest, two weaker.
        crate::youth::store_pool(&mut r, club.0, club.1, &[vec![], vec![id]]);
        let set = |r: &mut Records, age: u16, status: i32, strength: u8| {
            let year = r.globals.date()[2];
            let p = r.player_mut(id).unwrap();
            p.set_birth_year(year - age - 1);
            crate::newplayer::set_status(p, status);
            p.set_strength(strength);
            p.set_injured_to([0, 0]);
        };
        set(&mut r, 20, 0, 4);
        for _ in 0..5 {
            rise_or_fall(&mut r, club, 10, &mut random);
        }
        let p = r.player(id).unwrap();
        assert_eq!((crate::newplayer::status(p), p.strength(), p.picks() & 0xc0), (2, 6, 0x40));
        set(&mut r, 35, 2, 6);
        for _ in 0..5 {
            rise_or_fall(&mut r, club, 10, &mut random);
        }
        let p = r.player(id).unwrap();
        assert_eq!((crate::newplayer::status(p), p.strength()), (0, 4));

        // Out hurt he slips a status and a point, once for the injury.
        let day = i32::from(r.globals.day());
        crate::training_day::injure(&mut r, id, HURT_KIND, 60, day);
        set_injured_status(&mut r, id, 1, 5);
        slip_amateur(&mut r, club, id, &mut untold(&mut Vec::new()), &mut random);
        slip_amateur(&mut r, club, id, &mut untold(&mut Vec::new()), &mut random);
        let p = r.player(id).unwrap();
        assert_eq!((crate::newplayer::status(p), p.strength(), p.retiring() & SLIPPED), (0, 4, SLIPPED));
        // At the lowest status and strength 3 he gives up.
        set_injured_status(&mut r, id, 0, 3);
        slip_amateur(&mut r, club, id, &mut untold(&mut Vec::new()), &mut random);
        assert!(crate::youth::pool(&r, club.0, club.1)[1].is_empty());
        assert_eq!(r.player(id).unwrap().retiring(), 0);
    }

    /// On save A the human club's youth week tells him what came of it:
    /// a youth hurt (0x361), a new one with the trainer a point up with
    /// the public and the fans (0x35c), one grown with his new strength
    /// told (0x35d); too poor for a new youth, the manager's talent scout
    /// finds one of the three greatest talents, two points up and told to
    /// no one. In the day's news the walk comes on every third half week
    /// by the club's number, and never in the other two.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_human_club_is_told_of_its_youths_week() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = || Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let a = load();
        let texts = a3_files::saq::EventTexts::load(&dir).unwrap();
        let (hc, hn) = crate::statistics::human_clubs(&a)[0];
        let club = (hc, hn as u16);
        let trainer = usize::from(a.team(u16::from(hc), hn as u16).unwrap().humans()[0]) - 1;
        let liking = |r: &Records| r.managers[trainer].popularity();
        let seed = |range: std::ops::Range<u32>, from: u64| (from..).find(|&s| range.contains(&Random::new(s).below(DRAW))).unwrap();
        let mut r = load();
        let mut posted = Vec::new();
        walk(&mut r, club, true, &mut Random::new(seed(2500..2505, 0)), &mut posted);
        let [hurt] = posted[..] else { panic!("{posted:?}") };
        assert_eq!((hurt.club, hurt.code, hurt.text), ((u16::from(hc), hn as u16), 0x60, 0x361));
        let Fill::Named(id) = hurt.fill else { panic!() };
        assert!(crate::load::days_to(&r.globals, r.player(id).unwrap().injured_to()) >= 90);
        let told = crate::event_text::make(&r, &texts, &|_| String::new(), &[], &hurt);
        assert!(told.text.contains(&r.known_as(id).unwrap()), "{}", told.text);

        let mut r = load();
        crate::youth::set_made(&mut r, hc, hn as u16, 0);
        let before = crate::youth::youths(&r, hc, hn as u16).0;
        posted.clear();
        walk(&mut r, club, true, &mut Random::new(seed(0..50, 0)), &mut posted);
        let [taken] = posted[..] else { panic!("{posted:?}") };
        let Fill::Named(id) = taken.fill else { panic!() };
        assert_eq!(taken.text, 0x35c);
        assert!(!before.contains(&id) && crate::youth::youths(&r, hc, hn as u16).0.contains(&id));
        assert_eq!(crate::youth::made(&r, hc, hn as u16), 1);
        let up = |was: [u8; 3], by: u8| [(was[0] + by).min(100), (was[1] + by).min(100), was[2]];
        assert_eq!(liking(&r), up(liking(&a), 1));

        // A youth who can grow: the one grown is told with his strength.
        let mut s = 0;
        let grown = loop {
            let mut r = load();
            for id in crate::youth::youths(&r, hc, hn as u16).0 {
                let p = r.player_mut(id).unwrap();
                p.set_talent(0);
                p.set_strength(2);
            }
            s = seed(9000..10_000, s + 1);
            let mut posted = Vec::new();
            walk(&mut r, club, true, &mut Random::new(s), &mut posted);
            if !posted.is_empty() {
                break (r, posted);
            }
        };
        let (r, grew) = grown;
        let [grew] = grew[..] else { panic!("{grew:?}") };
        let Fill::NamedNumber(id, strength) = grew.fill else { panic!("{grew:?}") };
        assert_eq!((grew.text, strength), (0x35d, 3));
        assert_eq!(r.player(id).unwrap().strength(), 3);
        let told = crate::event_text::make(&r, &texts, &|_| String::new(), &[], &grew);
        assert!(told.text.contains(&r.known_as(id).unwrap()) && told.text.contains("Stärke 3"), "{}", told.text);
        assert_eq!(liking(&r), up(liking(&a), 1));

        // No support for youth, a scout's sum of 25 million: his find.
        let mut r = load();
        crate::youth::set_made(&mut r, hc, hn as u16, 0);
        let team = r.team_mut(u16::from(hc), hn as u16).unwrap();
        team.put::<i32>(crate::newplayer::YOUTH_SUPPORT, 0);
        team.put::<i32>(SCOUT, 25_000_000);
        posted.clear();
        walk(&mut r, club, true, &mut Random::new(seed(50..2500, 0)), &mut posted);
        assert!(posted.iter().all(|p| p.text != 0x35c), "{posted:?}");
        let id = *crate::youth::youths(&r, hc, hn as u16).0.iter().find(|id| !before.contains(id)).unwrap();
        assert!((0..=2).contains(&r.player(id).unwrap().talent()));
        assert_eq!(liking(&r), up(liking(&a), 2));

        // The day's news: on the club's third half week some seed walks
        // the youth, on the others none (a player's private life aside).
        let day = i32::from(a.globals.day());
        let slots = [day.rem_euclid(7) as i16];
        let exe = a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap();
        let tables = crate::first_season::Tables::load(&exe).unwrap();
        let league = crate::league::LeagueTables::load(&exe).unwrap();
        let mut r = load();
        let mut walked = |half: u16| {
            (0..400).any(|s| {
                r.globals.set_half_weeks(half);
                crate::training_day::news(&mut r, day, &slots, &mut Vec::new(), &tables, &league, &|_| false, &mut Random::new(s))
                    .iter()
                    .any(|p| {
                        !crate::news::CHILDREN_TEXTS.contains(&p.text)
                            && !crate::news::SERVICE_TEXTS.contains(&p.text)
                            && !crate::news::partner_text(p.text)
                    })
            })
        };
        let at = hn as u16 % 3;
        assert!(walked(at + 3));
        assert!(!walked(at + 1) && !walked(at + 2));
    }

    /// A computer club's week, told to no one.
    fn untold(posted: &mut Vec<Posted>) -> Told<'_> {
        Told { club: (0, 0), news: false, posted }
    }

    /// His status and strength set, his slip for the injury not yet taken.
    fn set_injured_status(r: &mut Records, id: u16, status: i32, strength: u8) {
        let p = r.player_mut(id).unwrap();
        crate::newplayer::set_status(p, status);
        p.set_strength(strength);
        p.set_retiring(p.retiring() & !SLIPPED);
    }
}
