//! The day's news of the human clubs: the pick of the category
//! (`FUN_0060be4d` over `DAT_0088e128`, inside `FUN_005ef8cc` as the
//! advance `FUN_0064f981` runs it for the clubs whose slot its gates
//! drew today, after the training).
//!
//! What is not here yet: the club's waiting-event gate `FUN_006a700a`,
//! which draws once only in one of its late cases and finds nothing in
//! the replica; the group news `FUN_005e3885` rolls 400 off the club's
//! fan byte (+0x9e), which the replica has no part of — on the compared
//! days of save A it stands at 100 or above, so no roll runs before the
//! pick; the rest gate `FUN_005ef56c` reads two run-time flags beside
//! the day counter; the player-manager blocks behind the club's bytes
//! +0x8a and +0x8b; and the handlers of the other categories but the
//! first — nearest category 18 (`FUN_005fd4ae`), which rolls over the
//! club's squad as 19 does and then picks one more weighted table when
//! the talked-about player has no news flag yet.
//!
//! Category 1, a player's children ([`children`]), and category 2, a
//! player's military or civil service ([`service`]), are here.

use std::ops::RangeInclusive;

use crate::event_text::{Fill, Posted};
use crate::mood::LAST_REAL_ID;
use crate::random::Random;
use crate::records::Records;

/// The code the news of a player's private life is posted with
/// (`FUN_00600f2f`'s second argument, 0x60).
const PRIVATE_NEWS: u16 = 0x60;
/// The player's bits at +0x1b4: he and his partner wish for a child
/// (`FUN_006ffda8`, set by `FUN_006ffdc8`), he has been a father before
/// (`FUN_007019d6` reads it clear), a child is on the way
/// (`FUN_00701ac4`).
const WISH: u32 = 1;
const FATHERED: u32 = 0x80_0000;
const EXPECTING: u32 = 0x100_0000;
/// The partner's kind, the low two bits of +0x27e (`FUN_00702b8c`).
const GIRLFRIEND: u8 = 1;
const WIFE: u8 = 3;
/// The most men each of category 1's two lists takes.
const PICKS_MOST: usize = 0x23;
/// The news "a child on the way" stands among the recent events as this
/// (`FUN_005e5e90(27000)` before it, `FUN_005e5ee0` adding it).
const EXPECTING_KEY: i32 = 27_000;
/// The recent events kept (`FUN_005e5ee0`'s twenty).
const RECENT: usize = 20;
/// The child is due this many days on, and up to `DUE_SPREAD` less one
/// more.
const DUE_AFTER: i32 = 0xdf;
const DUE_SPREAD: u32 = 0x2d;
/// A man has at most five children; their days of birth stand four bytes
/// each from +0x174 (`FUN_007019fd`).
const MOST_CHILDREN: i16 = 5;
const BIRTH_DATES: usize = 0x174;
/// One time in five the news is told.
const TOLD_ONE_IN: u32 = 5;
/// The man wishes aloud for a child only while the trainer pleases him
/// more than this (`FUN_007008b9`).
const WISH_CONTENT: i32 = 0x68;
/// A first child surprises a man less pleased with his private life
/// than this (`FUN_007010d9`).
const GLAD_PRIVATE: i32 = 100;
/// A man with this many children or more is told one more with a groan.
const MANY_CHILDREN: i16 = 4;
/// Category 1's texts, wishes and children on the way.
pub const CHILDREN_TEXTS: RangeInclusive<u16> = 0x410..=0x41e;

/// Category 2's texts, a man's military service (0x430 to 0x437) and his
/// civil service (0x438 to 0x43a).
pub const SERVICE_TEXTS: RangeInclusive<u16> = 0x430..=0x43a;
/// Only German men serve: the handle's nation byte (+0x24, the record's
/// +0x225) compared whole.
const GERMANY: u8 = 0x0e;
/// The service of a man, bits 5 and 6 of the partner byte +0x27e
/// (`FUN_00700581`): military, civil.
const MILITARY: u8 = 1;
const CIVIL: u8 = 2;
/// The happenings of each service, weighed alike (`FUN_0060be4d` over
/// `DAT_00a3267c`, registered by `FUN_005f9595` from `DAT_0088e6b0`:
/// eight of weight 1; and over `DAT_00a32670`, `FUN_005f9899` from
/// `DAT_0088e6d0`: three).
const MILITARY_HAPPENINGS: u32 = 8;
const CIVIL_HAPPENINGS: u32 = 3;
/// A happening of the military service stands among the recent events as
/// its number plus this.
const MILITARY_KEY: i32 = 0x57e4;
/// The injury the obstacle course leaves (`FUN_00766b92` with kind 10 and
/// no days: the kind's days of the table at `DAT_008ac341`, no knock).
const OBSTACLE_INJURY: (u8, i32) = (10, 35);
/// The second trait word's star airs (the handle's +0x12, the record's
/// +0x1ca), which the work in a care home takes away.
const STAR_AIRS: u16 = 0x800;
/// The state of a girlfriend (bits 2 to 4 of +0x27e, `FUN_00702c12`)
/// that leaves him free for a nurse.
const PARTING_STATE: u8 = 3;

/// The category table (`DAT_0088e128`, 24 pairs of weight and code
/// registered at a cap of 10 000): a total weight of 2100, the codes 1
/// to 12, 18 to 25, 50 to 52 and 55 — codes 13 to 17 stand no more.
const NEWS: [(u32, u16); 24] = [
    (100, 1),
    (100, 2),
    (100, 3),
    (100, 4),
    (100, 5),
    (100, 6),
    (100, 7),
    (100, 8),
    (100, 9),
    (55, 10),
    (55, 11),
    (11, 12),
    (110, 18),
    (110, 19),
    (30, 20),
    (11, 21),
    (22, 22),
    (11, 23),
    (55, 24),
    (100, 25),
    (66, 50),
    (44, 51),
    (500, 52),
    (20, 55),
];

/// The day's pick of the news category: one roll over the cap of 10 000;
/// the walk subtracts each weight not standing above the rest, and the
/// first entry whose weight stands above it gives its code — the rest
/// gone stands for no news, 0.
pub fn pick(random: &mut Random) -> u16 {
    let mut rest = random.below(10_000);
    for (weight, code) in NEWS {
        if weight > rest {
            return code;
        }
        rest -= weight;
    }
    0
}

/// The picked category 19 (`case 0x13` with `FUN_005fed08`): one of the
/// club's own players is talked about, drawn over the club's squad —
/// the club's +0xe2, the team record's `player_count` at +0x112, over
/// its squad list (the club's +0xb50, built from the record at +0x1f2).
///
/// Whether the news is then posted rests on two run-time parts the
/// replica holds no part of: the player must not be a player-manager
/// (its id below 17 001), and the posting `FUN_005fd59d` runs only for
/// a player whose news flag (its +0x28d) an earlier news already set —
/// no save keeps that flag, so out of a loaded save the pick of the
/// player stands as the day's whole draw here.
pub fn about_a_player(squad: u16, random: &mut Random) {
    random.below(u32::from(squad));
}

/// The picked category 1 (`case 1` with `FUN_005f8ca8`): each man of
/// the club's squad with a partner and no child on the way is drawn for
/// a child coming one time in seven with a wife, in twelve with a
/// girlfriend or fiancée — every time and one in seven once their wish
/// is spoken — and, while it is not, for the wish one time in five and
/// in fifteen, into one list each. A man of the first list is
/// drawn and told his child is on the way ([`expecting`]); failing any,
/// one of the second speaks his wish ([`wish`]). Whether the news took
/// place — when not, the day's news goes on to its later rolls.
/// `recent` are the events lately met
/// ([`crate::season::Season::recent_events`]).
pub fn children(
    records: &mut Records,
    club: (u16, u16),
    recent: &mut Vec<i32>,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    let squad = records.team(club.0, club.1).map(|t| t.players()).unwrap_or_default();
    let mut coming = Vec::new();
    let mut wishing = Vec::new();
    for &id in &squad {
        let Some(p) = records.player(id) else { continue };
        if p.shown() & EXPECTING != 0 {
            continue;
        }
        let wished = p.shown() & WISH != 0;
        let (child_one_in, wish_one_in) = match p.partner() & 3 {
            0 => continue,
            WIFE if wished => (1, 0),
            WIFE => (7, 5),
            _ if wished => (7, 0),
            _ => (12, 15),
        };
        if random.below(child_one_in) == 0 && coming.len() < PICKS_MOST {
            coming.push(id);
        }
        if wish_one_in > 0 && random.below(wish_one_in) == 0 && wishing.len() < PICKS_MOST {
            wishing.push(id);
        }
    }
    if !coming.is_empty() {
        let id = coming[random.below(coming.len() as u32) as usize];
        return expecting(records, club, id, recent, posted, random);
    }
    if !wishing.is_empty() {
        let id = wishing[random.below(wishing.len() as u32) as usize];
        return wish(records, club, id, posted);
    }
    false
}

/// He and his partner wish for a child (`FUN_005f87d4`): the wish is
/// marked in any case; a player, not a playing manager, says so to his
/// trainer while the trainer pleases him enough — his wife or girlfriend
/// named by kind, a first child or one more. The original hands no
/// partner's name to these texts, which take none.
fn wish(records: &mut Records, club: (u16, u16), id: u16, posted: &mut Vec<Posted>) -> bool {
    let Some(p) = records.player_mut(id) else { return true };
    p.set_shown(p.shown() | WISH);
    if id > LAST_REAL_ID || crate::mood::parts(p)[0] <= WISH_CONTENT {
        return true;
    }
    let girlfriend = p.partner() & 3 == GIRLFRIEND;
    let text = match (p.children() == 0, girlfriend) {
        (true, false) => 0x410,
        (true, true) => 0x411,
        (false, false) => 0x412,
        (false, true) => 0x413,
    };
    posted.push(Posted { club, code: PRIVATE_NEWS, text, fill: Fill::Partnered(id) });
    true
}

/// A child on the way (`FUN_005f88b5`): not twice among the recent
/// events; then the child is due some 223 to 267 days on
/// ([`expect`]) — none past five children. A player, not a playing
/// manager, is told so one time in five, how by whether he has been a
/// father before, how pleased with his private life he is, and how many
/// children he has. The two texts of a girlfriend's proposal (0x419,
/// with the wedding date `FUN_00702ac7` sets) and her plain news (0x41a)
/// stand behind a gate no man passes — a first father's branch within the
/// others' — and so do 0x417 and 0x418, which want the wish spoken while
/// [`expect`] has just cleared it.
fn expecting(
    records: &mut Records,
    club: (u16, u16),
    id: u16,
    recent: &mut Vec<i32>,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    let Some(p) = records.player(id) else { return false };
    let first = p.shown() & FATHERED == 0;
    let partner = p.partner() & 3;
    if partner == 0 {
        return false;
    }
    if !meet(recent, EXPECTING_KEY) {
        return true;
    }
    let due = i32::from(records.globals.day()) + DUE_AFTER + random.below(DUE_SPREAD) as i32;
    if !expect(records, id, due) {
        return false;
    }
    if id > LAST_REAL_ID {
        return true;
    }
    let Some(p) = records.player(id) else { return true };
    let wished = p.shown() & WISH != 0;
    let children = p.children();
    // The private part read through its setter by nought, which reckons
    // his morale anew (`FUN_007010d9(0)`).
    let glum = |records: &mut Records| {
        records.player_mut(id).is_some_and(|p| {
            crate::mood::change_private(p, 0);
            i32::from(p.contentment()[4]) < GLAD_PRIVATE
        })
    };
    // The text, or the first of two the told news draws between.
    let (text, either) = if first && !wished && glum(records) {
        (0x414, false)
    } else if first && !wished {
        (0x415, true)
    } else if first && random.below(3) == 0 {
        (0x417, false)
    } else if first {
        (0x418, false)
    } else if random.below(10) != 0 {
        (0x41b, false)
    } else if children >= MANY_CHILDREN {
        (0x41c, false)
    } else if partner == WIFE {
        (0x41d, false)
    } else {
        (0x41e, false)
    };
    if random.below(TOLD_ONE_IN) == 0 {
        let text = if either { text + random.below(2) as u16 } else { text };
        posted.push(Posted { club, code: PRIVATE_NEWS, text, fill: Fill::Partnered(id) });
    }
    true
}

/// The child is on the way, due on day `due` (`FUN_007019fd`): for a man
/// with fewer than five children and none coming, the date goes in as his
/// next child's day of birth and the wish gives way to the child coming.
/// The original also books the birth among the private lives' dates
/// (`FUN_004377cf`: the saved list at `0x8eb8b0`, pages of a hundred
/// dated entries), which the replica leaves out, and the birth itself.
fn expect(records: &mut Records, id: u16, due: i32) -> bool {
    let date = records.globals.date_of(due as i16).map(|d| {
        use chrono::Datelike;
        let y = (d.year() as u16).to_le_bytes();
        [d.day() as u8, d.month() as u8, y[0], y[1]]
    });
    let Some(p) = records.player_mut(id) else { return false };
    let children = p.children();
    if children >= MOST_CHILDREN || p.shown() & EXPECTING != 0 {
        return false;
    }
    let at = BIRTH_DATES + 4 * children.max(0) as usize;
    for (k, b) in date.unwrap_or_default().into_iter().enumerate() {
        p.put::<u8>(at + k, b);
    }
    p.set_shown(p.shown() & !WISH | EXPECTING);
    true
}

/// An event met anew (`FUN_005e5e90`, `FUN_005e5ee0`): not when it
/// stands among the recent events already; else it goes in among them,
/// the oldest of twenty out.
fn meet(recent: &mut Vec<i32>, key: i32) -> bool {
    if recent.contains(&key) {
        return false;
    }
    if recent.len() == RECENT {
        recent.remove(0);
    }
    recent.push(key);
    true
}

/// The picked category 2 (`case 2` with `FUN_005f9a2d`): the club's own
/// real players — no playing manager (below 0x4269) — who are German and
/// serve, in the army or in civil service, go into one list (0x23 at
/// most) with no draw; one of them is drawn and his service has its
/// happening ([`military`], [`civil`]). Whether the news took place —
/// with no one serving it did not, and the day's news goes on to its
/// later rolls. `today` is the day counter, `recent` the events lately
/// met, `tables` those a new girlfriend is drawn from.
pub fn service(
    records: &mut Records,
    club: (u16, u16),
    today: i32,
    recent: &mut Vec<i32>,
    tables: &crate::first_season::Tables,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    let squad = records.team(club.0, club.1).map(|t| t.players()).unwrap_or_default();
    let serving: Vec<u16> = squad
        .into_iter()
        .filter(|&id| id <= LAST_REAL_ID)
        .filter(|&id| records.player(id).is_some_and(|p| p.nation() == GERMANY && matches!(serves(p), MILITARY | CIVIL)))
        .take(PICKS_MOST)
        .collect();
    if serving.is_empty() {
        return false;
    }
    let id = serving[random.below(serving.len() as u32) as usize];
    if records.player(id).is_some_and(|p| serves(p) == MILITARY) {
        military(records, club, id, today, recent, posted, random)
    } else {
        civil(records, club, id, tables, posted, random)
    }
}

/// His service (`FUN_00700581`).
fn serves(p: &crate::records::Player) -> u8 {
    p.partner() >> 5 & 3
}

/// A soldier's day (`FUN_005f95ad`): a man out injured or away from the
/// club has none, yet the news took place. One of eight happenings is
/// drawn, and one met lately is not met again; then: a week on the
/// training ground (0x430), at manoeuvres (0x431) or in the sick bay
/// after a gas drill (0x433), away seven days; a march leaving him
/// knocked about a day and twenty points less fresh (0x432); the
/// obstacle course's torn ligament (0x434); three days in the guardroom
/// for a row with his officer (0x435); a day and night of guard duty, two
/// days away (0x436); or his girlfriend leaves him, ten and a draw of ten
/// less pleased with his private life — the original tells this with
/// values beside the text (`FUN_00601043`, window `CEreignis2Werte`),
/// which the replica does not post. A man with no girlfriend has nothing
/// of it.
fn military(
    records: &mut Records,
    club: (u16, u16),
    id: u16,
    today: i32,
    recent: &mut Vec<i32>,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    let Some(p) = records.player(id) else { return true };
    let globals = &records.globals;
    if crate::load::days_to(globals, p.injured_to()) != 0 || crate::load::days_to(globals, p.away_to()) != 0 {
        return true;
    }
    let happening = random.below(MILITARY_HAPPENINGS) as u16 + 1;
    if !meet(recent, MILITARY_KEY + i32::from(happening)) {
        return true;
    }
    let text = 0x42f + happening;
    match happening {
        1 | 2 | 4 => send_away(records, id, 7, 8),
        3 => {
            crate::training_day::knock_for(records, id, 1, today);
            if let Some(p) = records.player_mut(id) {
                // `FUN_005e5f40(-20)` on the handle's freshness (+0x18).
                let mut fresh = p.freshness();
                fresh[0] = (i32::from(fresh[0]) - 20 * 100).clamp(0, 10_000) as i16;
                p.set_freshness(fresh);
            }
        }
        5 => crate::training_day::injure(records, id, OBSTACLE_INJURY.0, OBSTACLE_INJURY.1, today),
        6 => send_away(records, id, 3, 5),
        7 => send_away(records, id, 2, 8),
        _ => {
            if records.player(id).is_none_or(|p| p.partner() & 3 != GIRLFRIEND) {
                return true;
            }
            crate::first_season::part(records, id);
            let by = -10 - random.below(10) as i32;
            if let Some(p) = records.player_mut(id) {
                crate::mood::change_private(p, by);
            }
            return true;
        }
    }
    posted.push(Posted { club, code: PRIVATE_NEWS, text, fill: Fill::NumberedOne(id) });
    true
}

/// A day of civil service (`FUN_005f98b1`): one of three happenings,
/// none kept among the recent events: a week's course, away seven days
/// (0x438); the care home taking his star airs away, told only of a man
/// who has them (0x439); or, for a man with no partner or a girlfriend in
/// the state of parting, a new girlfriend, the nurse he falls for, ten
/// and a draw of ten more pleased with his private life — told with
/// values (0x43a), not posted here.
fn civil(
    records: &mut Records,
    club: (u16, u16),
    id: u16,
    tables: &crate::first_season::Tables,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    let Some(p) = records.player(id) else { return true };
    let (partner, country) = (p.partner(), p.club()[0]);
    let text = match random.below(CIVIL_HAPPENINGS) + 1 {
        1 => {
            send_away(records, id, 7, 9);
            0x438
        }
        2 => {
            let Some(p) = records.player_mut(id) else { return true };
            let mut traits = p.traits();
            if traits[1] & STAR_AIRS == 0 {
                return true;
            }
            traits[1] &= !STAR_AIRS;
            p.set_traits(traits);
            0x439
        }
        _ => {
            if partner & 3 == 0 || partner & 3 == GIRLFRIEND && partner >> 2 & 7 == PARTING_STATE {
                crate::first_season::new_partner(records, tables, country, id, random);
                let by = 10 + random.below(10) as i32;
                if let Some(p) = records.player_mut(id) {
                    crate::mood::change_private(p, by);
                }
            }
            return true;
        }
    };
    posted.push(Posted { club, code: PRIVATE_NEWS, text, fill: Fill::NumberedOne(id) });
    true
}

/// Away from the club for `days` (`FUN_006fe907` on the handle's first
/// absence, +0x6a and the reason in the low half of +0x66): not when he
/// is away longer already. The write-back keeps the days as the date
/// they last to (+0x1e4) and the reason at +0x26b.
fn send_away(records: &mut Records, id: u16, days: i32, reason: u8) {
    let to = crate::medical::date_in(&records.globals, days);
    let Some(p) = records.player(id) else { return };
    if crate::load::days_to(&records.globals, p.away_to()) > days {
        return;
    }
    let Some(p) = records.player_mut(id) else { return };
    p.set_away_to(to);
    p.set_away_kind(p.away_kind() & 0xf0 | reason);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pick takes one draw and walks the table: the roll of save A
    /// on its day 63, 1191, walks nine times 100, then 55, 55, 11 and
    /// 110 away and lands on the code 19.
    #[test]
    fn the_pick_walks_the_weights_over_one_draw() {
        let mut random = Random::from_state(20261101, 19);
        let mut copy = random.clone();
        let rest = copy.below(10_000);
        let code = pick(&mut random);
        assert!(random == copy, "one draw, no more");
        let mut walked = rest;
        let mut expected = 0;
        for (weight, at) in NEWS {
            if weight > walked {
                expected = at;
                break;
            }
            walked -= weight;
        }
        assert_eq!(code, expected);
    }

    /// Category 19 takes one draw over the club's squad and no more:
    /// the posting behind it needs the player's run-time news flag,
    /// which stands open out of every save.
    #[test]
    fn category_19_rolls_the_squad_once() {
        let mut seed = 7i64;
        let mut random = Random::from_state(seed, 3);
        while pick(&mut random.clone()) != 19 {
            seed += 1;
            random = Random::from_state(seed, 3);
        }
        let mut copy = random.clone();
        copy.below(10_000);
        let code = pick(&mut random);
        assert_eq!(code, 19);
        about_a_player(26, &mut random);
        let _next = copy.below(26);
        assert!(random == copy, "the pick of the player, one draw");
    }

    /// Category 1 on A's human squad, one married man its only partner:
    /// he is drawn for a child one time in seven and for the wish one in
    /// five; the child drawn comes due 223 days on and a few more, told
    /// one time in five with his wife's first name — once only while the
    /// news stands among the recent events. The wish alone is marked and,
    /// the trainer pleasing him, told.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn category_1_tells_of_a_child_on_the_way() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = || {
            let mut a = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
            let (hc, hn) = crate::statistics::human_clubs(&a)[0];
            let club = (u16::from(hc), hn as u16);
            let squad = a.team(club.0, club.1).unwrap().players();
            let man = squad[3];
            for &id in &squad {
                let p = a.player_mut(id).unwrap();
                p.set_partner(if id == man { WIFE } else { 0 });
                p.set_shown(p.shown() & !(WISH | EXPECTING | FATHERED));
            }
            a.player_mut(man).unwrap().set_children(0);
            (a, club, man)
        };
        let (mut r, club, man) = load();
        let draws = |seed: u64| {
            let mut r = Random::new(seed);
            let (child, wish) = (r.below(7), r.below(5));
            (child, wish, r)
        };

        // The child: a seed drawing him for it and telling it.
        let seed = (0..10_000)
            .find(|&s| {
                let (child, _, mut r) = draws(s);
                child == 0 && {
                    r.below(DUE_SPREAD);
                    r.below(TOLD_ONE_IN) == 0
                }
            })
            .unwrap();
        let (_, _, mut expected) = draws(seed);
        let due = i32::from(r.globals.day()) + DUE_AFTER + expected.below(DUE_SPREAD) as i32;
        let glum = i32::from(r.player(man).unwrap().contentment()[4]) < GLAD_PRIVATE;
        expected.below(TOLD_ONE_IN);
        let text = if glum { 0x414 } else { 0x415 + expected.below(2) as u16 };
        let (mut recent, mut posted, mut random) = (Vec::new(), Vec::new(), Random::new(seed));
        assert!(children(&mut r, club, &mut recent, &mut posted, &mut random));
        assert!(random == expected, "the draws as the original's");
        assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text, fill: Fill::Partnered(man) }]);
        assert_eq!(recent, [EXPECTING_KEY]);
        let p = r.player(man).unwrap();
        assert_eq!(p.shown() & (WISH | EXPECTING), EXPECTING);
        let date = r.globals.date_of(due as i16).unwrap();
        let born = p.get::<[u8; 4]>(BIRTH_DATES);
        use chrono::Datelike;
        assert_eq!(
            (born[0], born[1], u16::from_le_bytes([born[2], born[3]])),
            (date.day() as u8, date.month() as u8, date.year() as u16)
        );
        let mut names = vec![String::new(); 0x75 * 100];
        names[usize::from(p.names_nation()) * 100 + usize::from(p.partner_name())] = "Sabine".to_owned();
        let texts = a3_files::saq::EventTexts::load(&dir).unwrap();
        let told = crate::event_text::make(&r, &texts, &|_| String::new(), &names, &posted[0]);
        assert_eq!(told.text.contains("Sabine"), text != 0x416, "{}", told.text);
        assert!(told.text.contains(&r.known_as(man).unwrap()), "{}", told.text);

        // Among the recent events, the news is met and nothing happens.
        let (mut again, ..) = load();
        let mut random = Random::new(seed);
        assert!(children(&mut again, club, &mut recent, &mut Vec::new(), &mut random));
        assert_eq!(again.player(man).unwrap().shown() & EXPECTING, 0);

        // The wish: a seed drawing him for it only.
        let seed = (0..10_000).find(|&s| matches!(draws(s), (c, 0, _) if c != 0)).unwrap();
        let (mut r, ..) = load();
        let mut posted = Vec::new();
        assert!(children(&mut r, club, &mut Vec::new(), &mut posted, &mut Random::new(seed)));
        let p = r.player(man).unwrap();
        assert_eq!(p.shown() & (WISH | EXPECTING), WISH);
        let told = crate::mood::parts(p)[0] > WISH_CONTENT;
        let wished = told.then_some(Posted { club, code: PRIVATE_NEWS, text: 0x410, fill: Fill::Partnered(man) });
        assert_eq!(posted, wished.into_iter().collect::<Vec<_>>());
    }

    /// Category 2 on A's human squad, one German man its only one
    /// serving: drawn over a list of one, his happening one of eight in
    /// the army — away a week and told (0x430), not met again while among
    /// the recent events; the obstacle course's injury of kind 10 for 35
    /// days; his girlfriend leaving him, untold — or of three in civil
    /// service: the star airs taken and told (0x439), a nurse his new
    /// girlfriend. With no one serving no draw is made.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn category_2_tells_of_a_mans_service() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let tables =
            crate::first_season::Tables::load(&a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap()).unwrap();
        let texts = a3_files::saq::EventTexts::load(&dir).unwrap();
        let load = |service: u8, partner: u8| {
            let mut a = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
            let (hc, hn) = crate::statistics::human_clubs(&a)[0];
            let club = (u16::from(hc), hn as u16);
            let squad = a.team(club.0, club.1).unwrap().players();
            let man = squad[3];
            for &id in &squad {
                let p = a.player_mut(id).unwrap();
                p.set_partner(p.partner() & !0x60);
            }
            let p = a.player_mut(man).unwrap();
            p.set_nation(GERMANY);
            p.set_partner(service << 5 | partner);
            p.set_injured_to([0, 0]);
            p.set_away_to([0, 0]);
            (a, club, man)
        };
        let today = |r: &Records| i32::from(r.globals.day());
        // The seed whose happening of `of` is `n`, and the generator after.
        let seed = |of: u32, n: u32| {
            let s = (0..10_000)
                .find(|&s| {
                    let mut r = Random::new(s);
                    r.below(1);
                    r.below(of) + 1 == n
                })
                .unwrap();
            let mut after = Random::new(s);
            after.below(1);
            after.below(of);
            (s, after)
        };
        let run = |r: &mut Records, club, recent: &mut Vec<i32>, s: u64| {
            let (mut posted, mut random) = (Vec::new(), Random::new(s));
            let day = today(r);
            assert!(service(r, club, day, recent, &tables, &mut posted, &mut random));
            (posted, random)
        };

        // A week on the training ground.
        let (mut r, club, man) = load(MILITARY, 0);
        let (s, after) = seed(MILITARY_HAPPENINGS, 1);
        let mut recent = Vec::new();
        let (posted, random) = run(&mut r, club, &mut recent, s);
        assert!(random == after, "the draws as the original's");
        assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text: 0x430, fill: Fill::NumberedOne(man) }]);
        assert_eq!(recent, [MILITARY_KEY + 1]);
        let p = r.player(man).unwrap();
        assert_eq!(p.away_to(), crate::medical::date_in(&r.globals, 7));
        assert_eq!(p.away_kind() & 0xf, 8);
        let told = crate::event_text::make(&r, &texts, &|_| String::new(), &[], &posted[0]);
        assert!(told.text.starts_with("Ihr Spieler ") && told.text.contains(&r.known_as(man).unwrap()), "{}", told.text);
        assert!(!told.text.contains('%'), "{}", told.text);
        // Met lately, it is not met again; away, he has no happening.
        let (mut again, ..) = load(MILITARY, 0);
        let (posted, _) = run(&mut again, club, &mut recent, s);
        assert!(posted.is_empty() && again.player(man).unwrap().away_to() == [0, 0]);
        let (posted, random) = run(&mut r, club, &mut Vec::new(), s);
        let mut drawn = Random::new(s);
        drawn.below(1);
        assert!(posted.is_empty() && random == drawn);

        // The obstacle course.
        let (mut r, ..) = load(MILITARY, 0);
        let (s, _) = seed(MILITARY_HAPPENINGS, 5);
        let (posted, _) = run(&mut r, club, &mut Vec::new(), s);
        assert_eq!(posted[0].text, 0x434);
        let p = r.player(man).unwrap();
        assert_eq!((p.injury_kind(), p.injured_to()), (10, crate::medical::date_in(&r.globals, 35)));

        // His girlfriend leaves him, a draw of ten more.
        let (mut r, ..) = load(MILITARY, GIRLFRIEND);
        let private = r.player(man).unwrap().contentment()[4];
        let (s, _) = seed(MILITARY_HAPPENINGS, 8);
        let (posted, _) = run(&mut r, club, &mut Vec::new(), s);
        let p = r.player(man).unwrap();
        assert!(posted.is_empty() && p.partner() & 3 == 0);
        assert!(p.contentment()[4] < private || private == 0, "{private} {:?}", p.contentment());

        // The care home takes his star airs.
        let (mut r, ..) = load(CIVIL, 0);
        let p = r.player_mut(man).unwrap();
        p.set_traits([p.traits()[0], p.traits()[1] | STAR_AIRS]);
        let (s, after) = seed(CIVIL_HAPPENINGS, 2);
        let (posted, random) = run(&mut r, club, &mut Vec::new(), s);
        assert!(random == after);
        assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text: 0x439, fill: Fill::NumberedOne(man) }]);
        assert_eq!(r.player(man).unwrap().traits()[1] & STAR_AIRS, 0);
        // Without them nothing is told.
        let (posted, _) = run(&mut r, club, &mut Vec::new(), s);
        assert!(posted.is_empty());

        // A nurse: a new girlfriend.
        let (mut r, ..) = load(CIVIL, 0);
        let (s, _) = seed(CIVIL_HAPPENINGS, 3);
        let (posted, _) = run(&mut r, club, &mut Vec::new(), s);
        assert!(posted.is_empty());
        assert_eq!(r.player(man).unwrap().partner() & 3, GIRLFRIEND);

        // No one serving: no draw, no news.
        let (mut r, ..) = load(0, 0);
        let mut random = Random::new(7);
        let day = today(&r);
        assert!(!service(&mut r, club, day, &mut Vec::new(), &tables, &mut Vec::new(), &mut random));
        assert!(random == Random::new(7));
    }
}
