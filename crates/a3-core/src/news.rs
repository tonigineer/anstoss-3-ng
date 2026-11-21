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
//! first six — nearest category 18 (`FUN_005fd4ae`), which rolls over the
//! club's squad as 19 does and then picks one more weighted table when
//! the talked-about player has no news flag yet.
//!
//! Category 1, a player's children ([`children`]), category 2, a
//! player's military or civil service ([`service`]), and categories 3 to
//! 6, a single man's days ([`single`]), a girlfriend's
//! ([`with_girlfriend`]), a married man's ([`married`]) and his wife's
//! suspicion ([`suspicion`]), are here.

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

/// The parts of his contentment the news of his private life moves:
/// his private life (+0x293, `FUN_007010d9`) and his distractions
/// (+0x294, `FUN_00701210`).
const PRIVATE: usize = 4;
const DISTRACTION: usize = 5;
/// Heartsore, bit 0x10 of +0x1b4 (`FUN_006ffdfa`): no single man's news
/// and no partner to take; healed once more pleased with his private
/// life than this.
const HEARTSORE: u32 = 0x10;
const HEALED: i32 = 0x59;
/// The happenings of a single man, a girlfriend's man, a married man and
/// a suspecting wife, pairs of weight and number (`FUN_0060be4d` over
/// `DAT_00a32634`, `DAT_00a32628`, `DAT_00a3261c` and `DAT_00a32610`,
/// registered by `FUN_005fa6b6`, `FUN_005faa5e`, `FUN_005fb24c` and
/// `FUN_005fb739` from `DAT_0088e70c`, `DAT_0088e71c` — twelve of
/// weight 1 — `DAT_0088e74c` and `DAT_0088e778`).
const SINGLE_HAPPENINGS: [(u32, u16); 4] = [(5, 1), (5, 2), (1, 3), (5, 4)];
const GIRLFRIEND_HAPPENINGS: u32 = 12;
const MARRIED_HAPPENINGS: [(u32, u16); 11] =
    [(3, 1), (3, 2), (3, 3), (3, 4), (3, 5), (3, 6), (3, 7), (3, 8), (3, 9), (1, 10), (1, 11)];
const SUSPICION_HAPPENINGS: [(u32, u16); 4] = [(3, 1), (3, 2), (1, 3), (3, 4)];
/// The happenings stand among the recent events as their number plus
/// these.
const SINGLE_KEY: i32 = 24_000;
const GIRLFRIEND_KEY: i32 = 22_000;
const MARRIED_KEY: i32 = 0x6b6c;
/// A grandmother's trolley: an injury of kind 4 (no knock), a week out
/// (`FUN_00766b92`).
const TROLLEY_INJURY: (u8, i32) = (4, 7);
/// His flat lies over a disco (bit 0x100 of +0x1b4, `FUN_006ffe7d`).
const DISCO_FLAT: u32 = 0x100;
/// His ex: her first name (+0x285), row (+0x287) and birthday (+0x18c);
/// his partner's birthday (+0x188); the date they came together (+0x16c,
/// day, month, year); the byte of their engagement (+0x170,
/// `FUN_00702a9b`).
const EX_NAME: usize = 0x285;
const EX_ROW: usize = 0x287;
const EX_BIRTHDAY: usize = 0x18c;
const PARTNER_BIRTHDAY: usize = 0x188;
const TOGETHER_SINCE: usize = 0x16c;
const ENGAGED: usize = 0x170;
/// Within this many days of their start his ex still matters.
const YEAR_DAYS: i32 = 0x16d;
/// A team-mate takes his girlfriend while he is less pleased with his
/// private life than this.
const LEAVES_PRIVATE: i32 = 0x5f;
/// Bit 0x2000000 of +0x1b4 (`FUN_007028d7`), cleared when he parts, keeps
/// him from going back to his ex.
const BACK_GATE: u32 = 0x200_0000;
/// His mother-in-law lives with him (bit 0x80000 of +0x1b4,
/// `FUN_00700413`).
const MOTHER_IN_LAW: u32 = 0x8_0000;
/// An affair (bit 4 of +0x1b4, `FUN_00702a5b`), found out (bit 8,
/// `FUN_00702a02`).
const AFFAIR: u32 = 4;
const SUSPECTED: u32 = 8;
/// The date (`DAT_00b5cbda`, day and month) of no present.
const CHRISTMAS_EVE: (u16, u16) = (24, 12);

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

/// The picked category 3 (`case 3` with `FUN_005fa890`): the club's own
/// real players with no partner go into one list (0x23 at most) unless
/// heartsore (+0x1b4 bit 0x10, `FUN_006ffdfa`) — and a heartsore man
/// pleased enough with his private life (past 0x59) is healed of it
/// on the way (`FUN_006ffe1a(0)`). One of the list is drawn and meets
/// his happening ([`single_day`]). Whether the news took place — with no
/// one in the list it did not.
pub fn single(
    records: &mut Records,
    club: (u16, u16),
    today: i32,
    recent: &mut Vec<i32>,
    tables: &crate::first_season::Tables,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    let squad = records.team(club.0, club.1).map(|t| t.players()).unwrap_or_default();
    let mut free = Vec::new();
    for &id in &squad {
        if id > LAST_REAL_ID {
            continue;
        }
        let Some(p) = records.player_mut(id) else { continue };
        if p.partner() & 3 != 0 {
            continue;
        }
        if p.shown() & HEARTSORE == 0 {
            if free.len() < PICKS_MOST {
                free.push(id);
            }
        } else if i32::from(p.contentment()[PRIVATE]) > HEALED {
            p.set_shown(p.shown() & !HEARTSORE);
        }
    }
    if free.is_empty() {
        return false;
    }
    let id = free[random.below(free.len() as u32) as usize];
    single_day(records, club, id, today, recent, tables, posted, random)
}

/// A single man's day (`FUN_005fa6ce`, "EreignisSolo"): one of four
/// happenings weighed 5, 5, 1, 5, and one met lately not met again
/// (24000 and its number). He falls in love — a new girlfriend, five
/// less content with his distractions, told with values beside the text
/// (0x44b, `FUN_00601043`), which the replica does not post; a new
/// girlfriend who wears him out, five points of freshness and two of
/// condition gone (0x44c); a grandmother's trolley in his heels, out a
/// week with an injury of kind 4, unless out already (0x44d); or, for a
/// man whose flat lies over a disco (+0x1b4 bit 0x100, `FUN_006ffe7d`),
/// five points of freshness gone (0x44e).
fn single_day(
    records: &mut Records,
    club: (u16, u16),
    id: u16,
    today: i32,
    recent: &mut Vec<i32>,
    tables: &crate::first_season::Tables,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    let happening = weighed(&SINGLE_HAPPENINGS, random);
    if !meet(recent, SINGLE_KEY + i32::from(happening)) {
        return true;
    }
    let Some(p) = records.player(id) else { return true };
    let country = p.club()[0];
    let (text, fill) = match happening {
        1 => {
            crate::first_season::new_partner(records, tables, country, id, random);
            change(records, id, DISTRACTION, -5);
            return true;
        }
        2 => {
            crate::first_season::new_partner(records, tables, country, id, random);
            if let Some(p) = records.player_mut(id) {
                // `FUN_005e5f40(-5)` and `FUN_00411130(-2)` on the handle's
                // freshness (+0x18) and condition (+0x16).
                let mut fresh = p.freshness();
                fresh[0] = (i32::from(fresh[0]) - 5 * 100).clamp(0, 10_000) as i16;
                p.set_freshness(fresh);
                p.set_condition((i32::from(p.condition()) - 2 * 100).clamp(0, 10_000) as i16);
            }
            (0x44c, Fill::Partnered(id))
        }
        3 => {
            if crate::load::days_to(&records.globals, p.injured_to()) != 0 {
                return true;
            }
            crate::training_day::injure(records, id, TROLLEY_INJURY.0, TROLLEY_INJURY.1, today);
            (0x44d, Fill::Partnered(id))
        }
        _ => {
            if p.shown() & DISCO_FLAT == 0 {
                return true;
            }
            if let Some(p) = records.player_mut(id) {
                let mut fresh = p.freshness();
                fresh[0] = (i32::from(fresh[0]) - 5 * 100).clamp(0, 10_000) as i16;
                p.set_freshness(fresh);
            }
            (0x44e, Fill::NumberedOne(id))
        }
    };
    posted.push(Posted { club, code: PRIVATE_NEWS, text, fill });
    true
}

/// The picked category 4 (`case 4` with `FUN_005fb17a`): one of the
/// club's own real players with a girlfriend is drawn (0x23 at most in
/// the list) and meets his happening ([`girlfriend_day`]); with none the
/// news did not take place.
pub fn with_girlfriend(
    records: &mut Records,
    club: (u16, u16),
    recent: &mut Vec<i32>,
    tables: &crate::first_season::Tables,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    let squad = records.team(club.0, club.1).map(|t| t.players()).unwrap_or_default();
    let men: Vec<u16> = squad
        .into_iter()
        .filter(|&id| id <= LAST_REAL_ID && records.player(id).is_some_and(|p| p.partner() & 3 == GIRLFRIEND))
        .take(PICKS_MOST)
        .collect();
    if men.is_empty() {
        return false;
    }
    let id = men[random.below(men.len() as u32) as usize];
    girlfriend_day(records, club, id, recent, tables, posted, random)
}

/// A day with his girlfriend (`FUN_005faa76`, "EreignisHatFreundin"): one
/// of twelve happenings alike, one met lately not met again (22000 and
/// its number), most only while the two stand calm (state 0 of
/// `FUN_00702c12`). Rows, a mocked compliment, a refused proposal, one
/// before the whole stadium, her pressing to marry, her visit home: less
/// pleased with his private life by 5 and a draw of 6 (1 and 3), 10 and
/// of 10, 6 and of 8, 15 and of 5, or 5 — told with values (0x44f to
/// 0x454, 0x45a), not posted. A team-mate takes her: one of the squad's
/// free men drawn first (`FUN_005fa981`), then, while he is less pleased
/// with his private life than 0x5f and no child is coming, she goes to
/// the mate (`FUN_00702668`), both three more pleased (0x450). Calling
/// her by his ex's name within a year of their start (0x455): back with
/// the ex (`FUN_00702762`), unless bit 0x2000000 of +0x1b4 stands; the
/// ex calling him (0x456). A man not engaged (+0x170) and with no
/// children: a new girlfriend from the disco (0x457); she leaves him,
/// 15 less (0x458, not posted); or she leaves him for a free team-mate,
/// the two each other's rival (`FUN_00703ce5`), 15 less (0x459, not
/// posted).
fn girlfriend_day(
    records: &mut Records,
    club: (u16, u16),
    id: u16,
    recent: &mut Vec<i32>,
    tables: &crate::first_season::Tables,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    let happening = random.below(GIRLFRIEND_HAPPENINGS) as u16 + 1;
    if !meet(recent, GIRLFRIEND_KEY + i32::from(happening)) {
        return true;
    }
    let Some(p) = records.player(id) else { return true };
    let has_girlfriend = p.partner() & 3 == GIRLFRIEND;
    let calm = state(p) == 0;
    let private = i32::from(p.contentment()[PRIVATE]);
    let expecting = p.shown() & EXPECTING != 0;
    let fresh_start = days_together(records, id) < YEAR_DAYS && p.get::<u8>(EX_NAME) != 0;
    let unbound = p.get::<u8>(ENGAGED) == 0 && p.children() == 0 && calm;
    let country = p.club()[0];
    let text = match happening {
        1 | 3 | 4 | 5 | 6 => {
            if has_girlfriend && calm {
                let (less, spread) = match happening {
                    1 | 3 => (5, 6),
                    4 => (10, 10),
                    5 => (6, 8),
                    _ => (15, 5),
                };
                let by = -less - random.below(spread) as i32;
                change(records, id, PRIVATE, by);
            }
            return true;
        }
        2 => {
            let Some(mate) = free_mate(records, club, random) else { return true };
            if has_girlfriend && private < LEAVES_PRIVATE && calm && !expecting {
                crate::first_season::part(records, mate);
                take_partner(records, mate, id);
                crate::first_season::part(records, id);
                change(records, mate, PRIVATE, 3);
                change(records, id, PRIVATE, 3);
            }
            return true;
        }
        7 => {
            if !has_girlfriend || !fresh_start || !calm || p.shown() & BACK_GATE != 0 {
                return true;
            }
            back_to_ex(records, id);
            0x455
        }
        8 => {
            if !has_girlfriend || !fresh_start || !calm {
                return true;
            }
            0x456
        }
        9 => {
            if !unbound {
                return true;
            }
            crate::first_season::new_partner(records, tables, country, id, random);
            0x457
        }
        10 => {
            if unbound {
                crate::first_season::part(records, id);
                change(records, id, PRIVATE, -15);
            }
            return true;
        }
        11 => {
            if !unbound || expecting {
                return true;
            }
            let Some(mate) = free_mate(records, club, random) else { return true };
            crate::first_season::part(records, mate);
            take_partner(records, mate, id);
            crate::first_season::part(records, id);
            rivals(records, id, mate);
            change(records, id, PRIVATE, -15);
            return true;
        }
        _ => {
            if calm {
                change(records, id, PRIVATE, -5);
            }
            return true;
        }
    };
    posted.push(Posted { club, code: PRIVATE_NEWS, text, fill: Fill::PartnerAndEx(id) });
    true
}

/// The picked category 5 (`case 5` with `FUN_005fb634`): nothing while
/// the club stands in a camp of its trainer's calendar (`FUN_005eab71`,
/// yet the news took place); else one of the club's own real players
/// married and with an affair (+0x284, `FUN_006ffbe3`) is drawn (0x23 at
/// most in the list) and meets his happening ([`married_day`]); with none
/// the news did not take place.
pub fn married(
    records: &mut Records,
    club: (u16, u16),
    today: i32,
    recent: &mut Vec<i32>,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    let Some(team) = records.team(club.0, club.1) else { return false };
    let camp = usize::from(team.humans()[0])
        .checked_sub(1)
        .and_then(|trainer| records.calendars.get(trainer))
        .zip(usize::try_from(today).ok())
        .is_some_and(|(calendar, d)| d < crate::year::CAMP_DAYS && crate::year::in_camp(calendar, d));
    if camp {
        return true;
    }
    let men: Vec<u16> = team
        .players()
        .into_iter()
        .filter(|&id| id <= LAST_REAL_ID)
        .filter(|&id| records.player(id).is_some_and(|p| p.partner() & 3 == WIFE && p.affair_name() != 0))
        .take(PICKS_MOST)
        .collect();
    if men.is_empty() {
        return false;
    }
    let id = men[random.below(men.len() as u32) as usize];
    married_day(records, club, id, recent, posted, random)
}

/// A married man's day (`FUN_005fb264`, "EreignisSpielerIstVerheiratet"):
/// one of eleven happenings weighed 3 nine times and 1 twice, one met
/// lately not met again (27500 and its number); 4 to 7 have nothing.
/// His wife gone with their child to a new man, for a man of strength 1
/// (the handle's +0x2, the record's +0x21c) with one child: 40 and a
/// draw of 11 less pleased with his private life (0x45b); his
/// mother-in-law moving out, three more content with his distractions,
/// or in, five less (+0x1b4 bit 0x80000; 0x45c, 0x45d); his wife's hair
/// unseen, five less (0x462) — told with values, not posted. Talking of
/// his affair in his sleep, calm with his wife, the affair found out
/// (bits 4 off, 8 on, `FUN_00702a02`; 0x463). His wife caught robbing
/// a bank: five less content with his distractions and his private life
/// — the headline (0x465, `FUN_00789c32`) and the window (0x464,
/// `FUN_006015fe`) are not posted. Christmas Eve with no present, four
/// less (0x7be, not posted), and then the news did not take place.
fn married_day(
    records: &mut Records,
    club: (u16, u16),
    id: u16,
    recent: &mut Vec<i32>,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    let happening = weighed(&MARRIED_HAPPENINGS, random);
    if !meet(recent, MARRIED_KEY + i32::from(happening)) {
        return true;
    }
    let Some(p) = records.player(id) else { return true };
    let shown = p.shown();
    match happening {
        1 => {
            if p.strength() == 1 && p.partner() & 3 == WIFE && p.children() == 1 {
                let by = -40 - random.below(11) as i32;
                change(records, id, PRIVATE, by);
            }
        }
        2 | 3 => {
            let moves_in = happening == 3;
            if (shown & MOTHER_IN_LAW == 0) == moves_in {
                if let Some(p) = records.player_mut(id) {
                    p.set_shown(if moves_in { shown | MOTHER_IN_LAW } else { shown & !MOTHER_IN_LAW });
                }
                change(records, id, DISTRACTION, if moves_in { -5 } else { 3 });
            }
        }
        8 => change(records, id, PRIVATE, -5),
        9 => {
            if p.affair_name() != 0 && state(p) == 0 {
                if let Some(p) = records.player_mut(id) {
                    p.set_shown(shown & !AFFAIR | SUSPECTED);
                }
                posted.push(Posted { club, code: PRIVATE_NEWS, text: 0x463, fill: Fill::AffairAndWife(id) });
            }
        }
        10 => {
            change(records, id, DISTRACTION, -5);
            change(records, id, PRIVATE, -5);
        }
        11 => {
            let [day, month, _] = records.globals.date();
            if (day, month) != CHRISTMAS_EVE {
                return true;
            }
            change(records, id, PRIVATE, -4);
            return false;
        }
        _ => {}
    }
    true
}

/// The picked category 6 (`case 6` with `FUN_005fb916`): one of the
/// club's own real players married and in an affair (+0x1b4 bit 4,
/// `FUN_00702a5b`) is drawn (0x23 at most in the list) and his wife
/// grows suspicious ([`suspicion_day`]); with none the news did not take
/// place.
pub fn suspicion(records: &mut Records, club: (u16, u16), posted: &mut Vec<Posted>, random: &mut Random) -> bool {
    let squad = records.team(club.0, club.1).map(|t| t.players()).unwrap_or_default();
    let men: Vec<u16> = squad
        .into_iter()
        .filter(|&id| id <= LAST_REAL_ID)
        .filter(|&id| records.player(id).is_some_and(|p| p.partner() & 3 == WIFE && p.shown() & AFFAIR != 0))
        .take(PICKS_MOST)
        .collect();
    if men.is_empty() {
        return false;
    }
    let id = men[random.below(men.len() as u32) as usize];
    suspicion_day(records, club, id, posted, random);
    true
}

/// His wife suspects (`FUN_005fb751`, "EreignisFrauSchoepftVerdacht"): one
/// of four happenings weighed 3, 3, 1, 3, kept among no recent events,
/// none while the two do not stand calm. She catches him spying: the
/// affair over (+0x284 cleared, bits 4 and 8 off, `FUN_00702962`), five
/// less pleased with his private life (0x466); a detective's photos:
/// their state 1 (`FUN_00702c39`), twenty less (0x467) — both told with
/// values, not posted. His flowers and a photo in his purse (0x468); his
/// card evenings (0x4d4), the affair found out (`FUN_00702a02`).
fn suspicion_day(records: &mut Records, club: (u16, u16), id: u16, posted: &mut Vec<Posted>, random: &mut Random) {
    let happening = weighed(&SUSPICION_HAPPENINGS, random);
    let Some(p) = records.player_mut(id) else { return };
    if state(p) != 0 {
        return;
    }
    let text = match happening {
        1 => {
            p.set_affair_name(0);
            p.set_shown(p.shown() & !(AFFAIR | SUSPECTED));
            change(records, id, PRIVATE, -5);
            return;
        }
        2 => {
            p.set_partner(p.partner() & 0xe3 | 1 << 2);
            change(records, id, PRIVATE, -20);
            return;
        }
        3 => 0x468,
        _ => {
            p.set_shown(p.shown() & !AFFAIR | SUSPECTED);
            0x4d4
        }
    };
    posted.push(Posted { club, code: PRIVATE_NEWS, text, fill: Fill::Partnered(id) });
}

/// Whether a text is one of categories 3 to 6's.
pub fn partner_text(text: u16) -> bool {
    (0x44b..=0x468).contains(&text) || text == 0x4d4 || text == 0x7be
}

/// One draw over a weighed table (`FUN_0060be4d` registered with no cap
/// beyond the weights): the first entry heavier than what the roll left.
fn weighed(table: &[(u32, u16)], random: &mut Random) -> u16 {
    let mut rest = random.below(table.iter().map(|&(weight, _)| weight).sum());
    for &(weight, code) in table {
        if weight > rest {
            return code;
        }
        rest -= weight;
    }
    0
}

/// How he and his partner stand (bits 2 to 4 of +0x27e, `FUN_00702c12`).
fn state(p: &crate::records::Player) -> u8 {
    p.partner() >> 2 & 7
}

/// One part of his contentment moved, his morale with it.
fn change(records: &mut Records, id: u16, part: usize, by: i32) {
    if let Some(p) = records.player_mut(id) {
        crate::mood::change(p, part, by);
    }
}

/// Days since he and his partner came together (`FUN_006ffc0a`): today's
/// day number less that of the date at +0x16c (day, month, year).
fn days_together(records: &Records, id: u16) -> i32 {
    let Some(p) = records.player(id) else { return 0 };
    let [day, month, year] = records.globals.date().map(|w| i32::from(w as i16));
    let since = (i32::from(p.get::<u8>(TOGETHER_SINCE)), i32::from(p.get::<u8>(TOGETHER_SINCE + 1)), i32::from(p.get::<i16>(TOGETHER_SINCE + 2)));
    crate::goodbye::day_number(day, month, year) - crate::goodbye::day_number(since.0, since.1, since.2)
}

/// A free team-mate (`FUN_005fa981`): the club's own real players with
/// no partner who are not heartsore, 0x23 at most, one drawn.
fn free_mate(records: &Records, club: (u16, u16), random: &mut Random) -> Option<u16> {
    let squad = records.team(club.0, club.1).map(|t| t.players()).unwrap_or_default();
    let free: Vec<u16> = squad
        .into_iter()
        .filter(|&id| id <= LAST_REAL_ID)
        .filter(|&id| records.player(id).is_some_and(|p| p.partner() & 3 == 0 && p.shown() & HEARTSORE == 0))
        .take(PICKS_MOST)
        .collect();
    (!free.is_empty()).then(|| free[random.below(free.len() as u32) as usize])
}

/// `to` takes the partner of `from` (`FUN_00702668`): her first name and
/// row, the children's names, days of birth and count — not her
/// birthday — no longer engaged, she his girlfriend, the two calm. The
/// private lives' list (`FUN_004377b7`) is left out.
fn take_partner(records: &mut Records, to: u16, from: u16) {
    let Some(f) = records.player(from) else { return };
    let (name, row, names, children) = (f.partner_name(), f.names_nation(), f.children_names(), f.children());
    let dates: Vec<u32> = (0..5).map(|k| f.get::<u32>(BIRTH_DATES + 4 * k)).collect();
    let Some(p) = records.player_mut(to) else { return };
    p.set_partner_name(name);
    p.set_names_nation(row);
    p.set_children_names(names);
    for (k, date) in dates.into_iter().enumerate() {
        p.put::<u32>(BIRTH_DATES + 4 * k, date);
    }
    p.set_children(children);
    p.put::<u8>(ENGAGED, 0);
    p.set_partner(p.partner() & 0xe0 | GIRLFRIEND);
}

/// Back with his ex (`FUN_00702762`): he parts from his partner, who
/// becomes his ex, and the ex before takes the partner's first name,
/// row and birthday — the kind of partner stays none, as the original
/// leaves it.
fn back_to_ex(records: &mut Records, id: u16) {
    let Some(p) = records.player(id) else { return };
    let (name, row, birthday) = (p.get::<u8>(EX_NAME), p.get::<u8>(EX_ROW), p.get::<u32>(EX_BIRTHDAY));
    crate::first_season::part(records, id);
    let Some(p) = records.player_mut(id) else { return };
    p.set_partner_name(name);
    p.set_names_nation(row);
    p.put::<u32>(PARTNER_BIRTHDAY, birthday);
    p.put::<u8>(ENGAGED, 0);
}

/// The two become rivals (`FUN_00703ce5` on `one` with `other`): each
/// lets go of the man he hated and that man of his in turn
/// (`FUN_00703b1a`); a best friendship of `one` with `other` undone, the
/// chain of best friends behind with it (`FUN_00703a80`, `FUN_00703a0c`);
/// each off the other's friends (`FUN_00703854`) and dislikes
/// (`FUN_00703930`, its slip zeroing the third friend); then each hates
/// the other.
fn rivals(records: &mut Records, one: u16, other: u16) {
    for who in [one, other] {
        let mut at = who;
        while let Some(p) = records.player_mut(at).filter(|p| p.hates() != 0) {
            at = p.hates();
            p.set_hates(0);
        }
    }
    if records.player(one).is_some_and(|p| p.best_friend() == other) {
        if let Some(p) = records.player_mut(one) {
            p.set_best_friend(0);
        }
        let mut at = other;
        while let Some(p) = records.player_mut(at).filter(|p| p.best_friend() != 0) {
            at = p.best_friend();
            p.set_best_friend(0);
        }
    }
    // Each call goes on to the other's list only when it found him, so
    // the two take turns; a dislike in the third place stays after the
    // shift and would send the original round for good — a few rounds
    // stand for it here.
    let (mut who, mut whom) = (one, other);
    while let Some(p) = records.player_mut(who) {
        let mut friends = p.friends();
        let Some(at) = friends.iter().take_while(|&&f| f != 0).position(|&f| f == whom) else { break };
        friends.copy_within(at + 1.., at);
        friends[2] = 0;
        p.set_friends(friends);
        (who, whom) = (whom, who);
    }
    let (mut who, mut whom) = (one, other);
    for _ in 0..4 {
        let Some(p) = records.player_mut(who) else { break };
        let mut dislikes = p.dislikes();
        let Some(at) = dislikes.iter().take_while(|&&d| d != 0).position(|&d| d == whom) else { break };
        dislikes.copy_within(at + 1.., at);
        p.set_dislikes(dislikes);
        let mut friends = p.friends();
        friends[2] = 0;
        p.set_friends(friends);
        (who, whom) = (whom, who);
    }
    if let Some(p) = records.player_mut(one) {
        p.set_hates(other);
    }
    if let Some(p) = records.player_mut(other) {
        p.set_hates(one);
    }
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

    /// Categories 3 to 6 on A's human squad, one man the only one who
    /// fits: a single man's new girlfriend wearing him out (0x44c), a
    /// heartsore one healed on the way; a girlfriend's man back with his
    /// ex (0x455) and one losing her to a free team-mate; a married man's
    /// affair talked of in his sleep (0x463) and Christmas Eve on another
    /// day; a suspecting wife's find (0x468) and her detective. Each over
    /// a list of one, its happening drawn over its weights.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn categories_3_to_6_tell_of_partners() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let tables =
            crate::first_season::Tables::load(&a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap()).unwrap();
        let texts = a3_files::saq::EventTexts::load(&dir).unwrap();
        let words = crate::words::Words::load(&a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap()).unwrap();
        // Every man of the squad given `others`, the man `partner` and
        // `shown` bits, calm.
        let load = |others: u8, partner: u8, shown: u32| {
            let mut a = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
            let (hc, hn) = crate::statistics::human_clubs(&a)[0];
            let club = (u16::from(hc), hn as u16);
            let squad = a.team(club.0, club.1).unwrap().players();
            let man = squad[3];
            for &id in &squad {
                let p = a.player_mut(id).unwrap();
                p.set_partner(others);
                p.set_shown(p.shown() & !(HEARTSORE | AFFAIR | EXPECTING));
                p.set_affair_name(0);
            }
            let p = a.player_mut(man).unwrap();
            p.set_partner(partner);
            p.set_shown(p.shown() | shown);
            p.set_injured_to([0, 0]);
            (a, club, man, squad)
        };
        // The seed whose list of one draws happening `n` of `table`.
        let seed = |table: &[(u32, u16)], n: u16| {
            (0..10_000)
                .find(|&s| {
                    let mut r = Random::new(s);
                    r.below(1);
                    weighed(table, &mut r) == n
                })
                .unwrap()
        };
        let alike: Vec<(u32, u16)> = (1..=12).map(|n| (1, n)).collect();
        let told = |r: &Records, posted: &Posted, man: u16| {
            let event = crate::event_text::make(r, &texts, &|_| String::new(), &words.first_names, posted);
            assert!(!event.text.contains('%') && event.text.contains(&r.known_as(man).unwrap()), "{}", event.text);
            event.text
        };

        // A single man's new girlfriend; a heartsore one beside him healed.
        let (mut r, club, man, squad) = load(GIRLFRIEND, 0, 0);
        let sore = squad[4];
        let p = r.player_mut(sore).unwrap();
        p.set_partner(0);
        p.set_shown(p.shown() | HEARTSORE);
        crate::mood::change(p, PRIVATE, 120);
        let before = r.player(man).unwrap().clone();
        let (mut posted, mut recent) = (Vec::new(), Vec::new());
        let day = i32::from(r.globals.day());
        assert!(single(&mut r, club, day, &mut recent, &tables, &mut posted, &mut Random::new(seed(&SINGLE_HAPPENINGS, 2))));
        assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text: 0x44c, fill: Fill::Partnered(man) }]);
        assert_eq!(recent, [SINGLE_KEY + 2]);
        let p = r.player(man).unwrap();
        assert_eq!(p.partner() & 3, GIRLFRIEND);
        assert_eq!(p.freshness()[0], (before.freshness()[0] - 500).max(0));
        assert_eq!(p.condition(), (before.condition() - 200).max(0));
        assert_eq!(r.player(sore).unwrap().shown() & HEARTSORE, 0);
        let first = words.first_names[usize::from(p.names_nation()) * 100 + usize::from(p.partner_name())].clone();
        assert!(told(&r, &posted[0], man).starts_with(&first));
        // Met lately: not again.
        let mut again = Vec::new();
        assert!(single(&mut r, club, day, &mut recent, &tables, &mut again, &mut Random::new(seed(&SINGLE_HAPPENINGS, 2))));
        assert!(again.is_empty());

        // Back with his ex within a year of their start.
        let (mut r, club, man, _) = load(0, GIRLFRIEND, 0);
        let [d, m, y] = r.globals.date();
        let p = r.player_mut(man).unwrap();
        p.put::<u8>(TOGETHER_SINCE, d as u8);
        p.put::<u8>(TOGETHER_SINCE + 1, m as u8);
        p.put::<u16>(TOGETHER_SINCE + 2, y);
        p.set_shown(p.shown() & !BACK_GATE);
        p.put::<u8>(EX_NAME, 77);
        let girl = p.partner_name();
        let mut posted = Vec::new();
        assert!(with_girlfriend(&mut r, club, &mut Vec::new(), &tables, &mut posted, &mut Random::new(seed(&alike, 7))));
        assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text: 0x455, fill: Fill::PartnerAndEx(man) }]);
        let p = r.player(man).unwrap();
        assert_eq!((p.partner() & 3, p.partner_name(), p.get::<u8>(EX_NAME)), (0, 77, girl));
        told(&r, &posted[0], man);
        // A team-mate takes her: the free man drawn, she his girlfriend.
        let (mut r, club, man, squad) = load(WIFE, GIRLFRIEND, 0);
        let mate = squad[5];
        r.player_mut(mate).unwrap().set_partner(0);
        crate::mood::change(r.player_mut(man).unwrap(), PRIVATE, -120);
        let girl = r.player(man).unwrap().partner_name();
        let mate_private = r.player(mate).unwrap().contentment()[PRIVATE];
        let mut posted = Vec::new();
        let s = seed(&alike, 2);
        let mut random = Random::new(s);
        assert!(with_girlfriend(&mut r, club, &mut Vec::new(), &tables, &mut posted, &mut random));
        let mut after = Random::new(s);
        after.below(1);
        after.below(12);
        after.below(1);
        assert!(posted.is_empty() && random == after);
        let (p, q) = (r.player(man).unwrap(), r.player(mate).unwrap());
        assert_eq!((p.partner() & 3, p.get::<u8>(EX_NAME)), (0, girl));
        assert_eq!((q.partner() & 3, q.partner_name()), (GIRLFRIEND, girl));
        assert_eq!((p.contentment()[PRIVATE], q.contentment()[PRIVATE]), (3, (mate_private + 3).min(120)));

        // A married man's affair talked of in his sleep.
        let (mut r, club, man, _) = load(0, WIFE, 0);
        r.player_mut(man).unwrap().set_affair_name(66);
        let day = i32::from(r.globals.day());
        let mut posted = Vec::new();
        assert!(married(&mut r, club, day, &mut Vec::new(), &mut posted, &mut Random::new(seed(&MARRIED_HAPPENINGS, 9))));
        assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text: 0x463, fill: Fill::AffairAndWife(man) }]);
        assert_eq!(r.player(man).unwrap().shown() & (AFFAIR | SUSPECTED), SUSPECTED);
        let affair = words.first_names[usize::from(r.player(man).unwrap().names_nation()) * 100 + 66].clone();
        assert!(told(&r, &posted[0], man).contains(&affair));
        // Christmas Eve is not today: the news took place, nothing more.
        assert_ne!((r.globals.date()[0], r.globals.date()[1]), CHRISTMAS_EVE);
        let private = r.player(man).unwrap().contentment()[PRIVATE];
        assert!(married(&mut r, club, day, &mut Vec::new(), &mut posted, &mut Random::new(seed(&MARRIED_HAPPENINGS, 11))));
        assert_eq!(r.player(man).unwrap().contentment()[PRIVATE], private);
        // No affair: no one, no draw.
        let (mut r, ..) = load(0, WIFE, 0);
        let mut random = Random::new(3);
        assert!(!married(&mut r, club, day, &mut Vec::new(), &mut Vec::new(), &mut random));
        assert!(random == Random::new(3));

        // A suspecting wife's find, and her detective.
        let (mut r, club, man, _) = load(0, WIFE, AFFAIR);
        let mut posted = Vec::new();
        assert!(suspicion(&mut r, club, &mut posted, &mut Random::new(seed(&SUSPICION_HAPPENINGS, 3))));
        assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text: 0x468, fill: Fill::Partnered(man) }]);
        told(&r, &posted[0], man);
        let private = i32::from(r.player(man).unwrap().contentment()[PRIVATE]);
        assert!(suspicion(&mut r, club, &mut posted, &mut Random::new(seed(&SUSPICION_HAPPENINGS, 2))));
        let p = r.player(man).unwrap();
        assert_eq!((state(p), i32::from(p.contentment()[PRIVATE])), (1, (private - 20).max(0)));
        // No longer calm: nothing more.
        assert!(suspicion(&mut r, club, &mut posted, &mut Random::new(seed(&SUSPICION_HAPPENINGS, 3))));
        assert_eq!(posted.len(), 1);
    }
}
