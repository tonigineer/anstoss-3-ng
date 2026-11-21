//! The day's news of the human clubs: the pick of the category
//! (`FUN_0060be4d` over `DAT_0088e128`, inside `FUN_005ef8cc` as the
//! advance `FUN_0064f981` runs it for the clubs whose slot its gates
//! drew today, after the training).
//!
//! Before the pick stand the association's points for repeated doping
//! ([`doping_penalty`]) and the doping reports against the club's trainer
//! ([`doping_reports`]). What is not here yet: the appointments due that
//! the waiting-event gate `FUN_006a700a` runs (it never ends the news);
//! the group news `FUN_005e3885` rolls 400 off the club's
//! fan byte (+0x9e), which the replica has no part of — on the compared
//! days of save A it stands at 100 or above, so no roll runs before the
//! pick; the rest gate `FUN_005ef56c` reads two run-time flags beside
//! the day counter; the player-manager blocks behind the club's bytes
//! +0x8a and +0x8b; and the handlers of the later categories —
//! nearest category 18 (`FUN_005fd4ae`), which rolls over the
//! club's squad as 19 does and then picks one more weighted table when
//! the talked-about player has no news flag yet.
//!
//! Category 1, a player's children ([`children`]), category 2, a
//! player's military or civil service ([`service`]), and categories 3 to
//! 6, a single man's days ([`single`]), a girlfriend's
//! ([`with_girlfriend`]), a married man's ([`married`]) and his wife's
//! suspicion ([`suspicion`]), are here, and categories 7, a wife who
//! knows of the affair ([`affair_known`]), 8, a father's day with his
//! family ([`family`]), 9, a wedding near ([`engaged`]), 10, a man in
//! low spirits ([`low_spirits`]), 11, two at odds ([`at_odds`]), 12,
//! a man's new affair ([`new_affair`]), and the squad's own, 50, a
//! bootlicker ([`bootlicker`]), 51, an informer ([`informer`]), 52, the
//! mole who talks to the press ([`moles`]), and 55, a man's habits
//! ([`habits`]).

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
/// The happenings of a wife who knows of the affair, two weighed 3
/// (`FUN_0060be4d` over `DAT_00a32604`, registered by `FUN_005fba02`
/// from `DAT_0088e788`).
const KNOWN_HAPPENINGS: [(u32, u16); 2] = [(3, 1), (3, 2)];
/// How the two stand (bits 2 to 4 of +0x27e) once she files for divorce.
const DIVORCE: u8 = 4;
/// The second trait word's bit (the handle's +0x12, the record's +0x1ca)
/// that has a man in firm hands take up with a nude model.
const MODEL_TRAIT: u16 = 0x200;
/// A man so pleased with his private life (+0x293, `FUN_007011f1`, the
/// raw byte) takes up no new affair.
const NO_AFFAIR_FROM: u8 = 100;
/// A new affair's first name (`FUN_007029a7`): one of the fifty past the
/// first fifty of his nation's row, not his partner's.
const AFFAIR_NAMES: (u32, u8) = (50, 50);

/// The happenings of a married father's family, eighteen weighed 3
/// (`FUN_0060be4d` over `DAT_00a325f8`, registered by `FUN_005fbbe5`
/// from `DAT_0088e790`).
const FAMILY_HAPPENINGS: [(u32, u16); 18] = [
    (3, 1), (3, 2), (3, 3), (3, 4), (3, 5), (3, 6), (3, 7), (3, 8), (3, 9),
    (3, 10), (3, 11), (3, 12), (3, 13), (3, 14), (3, 15), (3, 16), (3, 17), (3, 18),
];
/// A son bears one of the first fifty names of his row (+0x288 on,
/// `FUN_00701ffa`), a daughter one of the fifty after.
const SONS_NAMES: u8 = 0x32;
/// The youngest child born less than this many days ago keeps his father
/// awake (`FUN_00702084`).
const NEWBORN_DAYS: i32 = 0xb4;
/// A father older than this goes roller-skating with his son.
const SKATING_FROM: i32 = 0x19;
/// The knock the skating leaves (the handle's +0x2f).
const GRAZED_DAYS: u16 = 7;
/// The months a school's parents' evening falls in (`DAT_00b5cbdc`):
/// November, and January to April.
const PARENTS_EVENING: [u16; 5] = [11, 1, 2, 3, 4];
/// From this day (`DAT_00b5cbd4`) a son is mocked at school for his
/// father's club missing its goal.
const MOCKED_FROM: i32 = 100;
/// The children's toys: an injury of kind 15 with the kind's own days
/// (`FUN_00766b92(15, -1)`, the table at `DAT_008ac341`).
const TOYS_INJURY: (u8, i32) = (15, 21);

/// Category 9's happenings, three alike (`FUN_0060be4d` over
/// `DAT_00a32640`, registered by `FUN_005fa37c` from `DAT_0088e700`) —
/// the fourth case of `FUN_005fa394` no entry reaches; they stand among
/// the recent events as their number plus 23000.
const WEDDING_HAPPENINGS: [(u32, u16); 3] = [(1, 1), (1, 2), (1, 3)];
const WEDDING_KEY: i32 = 23_000;
/// A man is low while his private life and distractions together please
/// him less than this.
const LOW_SPIRITS: i32 = 0xa0;
/// Category 10's happenings (`DAT_00a325ec`, registered by
/// `FUN_005fc6a4` from `DAT_0088e7d8`) — no 5.
const LOW_HAPPENINGS: [(u32, u16); 6] = [(30, 1), (3, 2), (3, 3), (3, 4), (30, 6), (30, 7)];
/// The months he is still without his driving licence (+0x28e,
/// `FUN_00702d31`).
const NO_LICENCE: usize = 0x28e;
/// The countries whose clubs' men drive on the left already (the club's
/// +0x104).
const LEFT_DRIVING: [u16; 2] = [0x10, 0x39];
/// The second trait word's bits (the handle's +0x12) of a man who takes
/// to drink, the bits of +0x1b4 he then has (`FUN_00703090(1)`), and the
/// bit of a spy among the squad (`FUN_007001ed`).
const DRINKER: u16 = 0x2610;
const DRINKS: u32 = 0x80_0200;
const SPY: u32 = 0x8000;
/// A club of this number (its +0x103) has no spy to tell.
const NO_SPIES: u16 = 200;
/// Category 11: the state of two at odds (`FUN_00702c12`), the roll of a
/// hundred under which they make up, how much his private life then
/// pleases him (`FUN_0070117e(100)`), and how the row ends otherwise
/// (`DAT_00a325d4`, registered by `FUN_005fcd85` from `DAT_0088e80c`).
const AT_ODDS: u8 = 1;
const MAKE_UP_IN: u32 = 100;
const MAKE_UP_UNDER: u32 = 0x46;
const MADE_UP: i32 = 100;
const ROW_HAPPENINGS: [(u32, u16); 7] = [(30, 1), (3, 2), (3, 3), (3, 4), (3, 5), (30, 6), (30, 7)];

/// Category 50: a bootlicker (+0x1b4 bit 0x4000, `FUN_00700168`), the
/// twenty happenings of his day (`DAT_00a2eb80`, registered by
/// `FUN_005e4a26` from `DAT_0088d898`), standing among the recent events
/// as their number plus 26000, and the team's mood his first takes.
const BOOTLICKER: u32 = 0x4000;
const BOOTLICKER_HAPPENINGS: [(u32, u16); 20] = [
    (3, 1), (200, 2), (200, 3), (200, 4), (200, 5), (200, 6), (200, 7), (200, 8), (200, 9), (200, 10),
    (200, 11), (200, 12), (200, 13), (200, 14), (200, 15), (200, 16), (200, 17), (200, 18), (200, 19), (200, 20),
];
const BOOTLICKER_KEY: i32 = 26_000;
const BOOTLICKED_MOOD: i32 = -10;
/// The day counter under which happening 20 is told.
const FIRST_DAYS: i32 = 7;
/// Category 51: an informer's two happenings (`DAT_00a2eb74`, registered
/// by `FUN_005e4eaa` from `DAT_0088d8e8`), what the squad's trust in the
/// trainer loses once he is found out, and the bits of the man shut out
/// (`FUN_0070031f`) and of the mole (`FUN_00700272`).
const INFORMER_HAPPENINGS: [(u32, u16); 2] = [(10, 1), (50, 2)];
const FOUND_OUT: i32 = -10;
const SHUT_OUT: u32 = 0x20000;
const MOLE: u32 = 0x10000;
const TEAM: usize = 1;
/// Category 52 (`FUN_005e50ba`): a man pleased with the trainer less
/// than 0x5a counts low; one not a mole of morale under 0x46, none of
/// the trait bits 0x120, with two of trainer, team and club under 0x58
/// may turn mole; a mole of morale 0x56 or more stops.
const LOW_TRAINER: i32 = 0x5a;
const MOLE_MORALE: u8 = 0x46;
const MOLE_TRAITS: u16 = 0x120;
const MOLE_PARTS: i32 = 0x58;
const MOLE_STOPS: u8 = 0x56;
/// How many men low with the trainer, and how many wanting to leave
/// (`FUN_00770877` at 3), the press hears of.
const LOW_MANY: usize = 7;
const LEAVING_MANY: usize = 5;
const LEAVING: usize = 3;
/// The club object's flags at +0xdf8: the press told of the low men, of
/// those wanting to leave, of the trainer this round, and the round's
/// three bits at 0x12 (`FUN_0069fe57`, stepped by `FUN_0069fe73` up to
/// 15, the bit past them cleared).
const TOLD_LOW: u32 = 0x10000;
const TOLD_LEAVING: u32 = 0x20000;
const TOLD_TRAINER: u32 = 0x20_0000;
const PRESS_ROUND: u32 = 0x1c_0000;
const PRESS_SHIFT: u32 = 0x12;
const PRESS_MOST: u32 = 0xf;
/// Category 55 (`FUN_005fff0f`): four habits over one man drawn of the
/// whole squad, a smoker's (+0x1b4 bit 0x800, `FUN_006fffd9`, a
/// handicap of 5 in condition) and a private fitness trainer's (bit
/// 0x2000, `FUN_007000e3`, a bonus of 5).
const HABITS: u32 = 4;
const SMOKES: u32 = 0x800;
const OWN_TRAINER: u32 = 0x2000;

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

/// The club's doping cases (club +0x115, team +0x13e): their count in the
/// low three bits, the top three kept when the association acts.
const DOPING_CASES: usize = 0x13e;
const CASES_COUNT: u8 = 7;
const CASES_KEPT: u8 = 0xe0;
/// More cases than this, and the association acts.
const CASES_TOLERATED: u8 = 2;
/// Only before this half week of the season (`DAT_00b5cbd2 < 0x50`).
const PENALTY_BEFORE: u16 = 0x50;
/// The club's penalty points (club +0x100, team +0x12a, three points
/// each in the table): the association takes two, six points.
const PENALTY: u8 = 3;
const PENALTY_TAKEN: u8 = 2;
/// The text "Dopingfälle v5 … 6 Punkte abgezogen" and its code.
pub const PENALTY_TEXT: u16 = 0x6f;
const PENALTY_CODE: u16 = 99;
/// What it costs the club's public image (`FUN_00698d01(-10)`, kept to
/// 10..50).
const PENALTY_IMAGE: i32 = -10;
const IMAGE_LEAST: i32 = 10;
const IMAGE_MOST: i32 = 50;

/// The association's points for repeated doping (`FUN_005ef8cc` after
/// its waiting events and rest gate): before the season's 80th half week,
/// a club with more than two doping cases and no penalty points yet is
/// told so with its phrases (text 0x6f, event 99 through `FUN_00733486`),
/// its cases go back to one, it loses six points and ten of its public
/// image. The day's news goes on.
pub fn doping_penalty(records: &mut Records, club: (u16, u16), posted: &mut Vec<Posted>) {
    if records.globals.half_weeks() >= PENALTY_BEFORE {
        return;
    }
    let Some(team) = records.team_mut(club.0, club.1) else { return };
    let cases = team.get::<u8>(DOPING_CASES);
    let flags = team.get::<u8>(crate::statistics::TEAM_FLAGS);
    if cases & CASES_COUNT <= CASES_TOLERATED || flags & PENALTY != 0 {
        return;
    }
    posted.push(Posted { club, code: PENALTY_CODE, text: PENALTY_TEXT, fill: Fill::Club });
    team.put::<u8>(DOPING_CASES, (cases & CASES_KEPT) + 1);
    team.put::<u8>(crate::statistics::TEAM_FLAGS, flags | PENALTY_TAKEN);
    let image = (i32::from(team.public_image()) + PENALTY_IMAGE).clamp(IMAGE_LEAST, IMAGE_MOST);
    team.set_public_image(image as u8);
}

/// A false report costs the reporter's club this at its other costs
/// (`DAT_00a3b38c`, club +0x2c0), told with text 0x703, code 0x60.
pub const FALSE_REPORT: i32 = 50_000;
pub const FALSE_REPORT_TEXT: u16 = 0x703;
const OTHER_COSTS: usize = 0x2c0 + crate::finances::CLUB_TO_TEAM;
/// A manager's reports followed up count to two (`FUN_006a662d(1)`).
const REPORTS_COUNTED: i16 = 2;
/// The control tests two men of the squad.
const TESTED: u32 = 2;

/// The doping reports against the club's trainer (`FUN_005db227`): each
/// other human in his order (`FUN_006a421a` up to `DAT_00b5cbce`) who
/// reported him (`DAT_00a2e016`) has his report taken and counted
/// (`FUN_006a662d(1)`, to two). With any, the association tests two men
/// of the squad drawn apart (`FUN_00675698(2, squad)`, the live player's
/// +0x57 bit 0x40, then `FUN_005dae5f`): a positive sample shows its
/// control (event 0xcb, dialog 41); with none, every reporter at a club
/// pays 50 000 for the false accusation, told with the trainer's name.
/// Whether the day's news ends here comes back.
pub fn doping_reports(
    records: &mut Records,
    club: (u16, u16),
    reported: &mut [[bool; crate::records::MANAGERS]; crate::records::MANAGERS],
    controls: &mut Vec<crate::doping_control::Control>,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    let Some(trainer) = records.team(club.0, club.1).map(|t| t.humans()[0]) else { return false };
    let order = records.globals.order();
    let humans = usize::from(records.globals.humans()).min(order.len());
    let mut reporters = Vec::new();
    for &number in &order[..humans] {
        let (Ok(reporter), Some(target)) = (usize::try_from(number - 1), usize::from(trainer).checked_sub(1)) else {
            continue;
        };
        if number == i16::from(trainer) {
            continue;
        }
        let Some(flag) = reported.get_mut(reporter).and_then(|row| row.get_mut(target)) else { continue };
        if *flag {
            *flag = false;
            reporters.push(reporter);
            if let Some(m) = records.managers.get_mut(reporter) {
                if m.doping_reports() < REPORTS_COUNTED {
                    m.set_doping_reports(m.doping_reports() + 1);
                }
            }
        }
    }
    if reporters.is_empty() {
        return false;
    }
    let squad = records.team(club.0, club.1).map_or(Vec::new(), |t| t.players());
    let mut drawn = [u32::MAX; TESTED as usize];
    let over = (squad.len() as u32).max(TESTED);
    for k in 0..drawn.len() {
        drawn[k] = loop {
            let at = random.below(over);
            if !drawn[..k].contains(&at) {
                break at;
            }
        };
    }
    let picked: Vec<u16> = drawn.iter().filter_map(|&at| squad.get(at as usize).copied()).collect();
    let control = crate::doping_control::test(records, club.0, club.1, &picked, random);
    if let Some(control) = control.filter(|c| (0..usize::from(c.count)).any(|k| c.positive[k])) {
        controls.push(control);
        return true;
    }
    for reporter in reporters {
        let Some(m) = records.managers.get(reporter) else { continue };
        let at = (m.country(), m.team());
        if at.0 == 0 {
            continue;
        }
        if let Some(team) = records.team_mut(at.0, at.1) {
            let costs = team.get::<i32>(OTHER_COSTS).wrapping_add(FALSE_REPORT);
            team.put(OTHER_COSTS, costs);
        }
        posted.push(Posted {
            club: at,
            code: PRIVATE_NEWS,
            text: FALSE_REPORT_TEXT,
            fill: Fill::FalseReport(u16::from(trainer), FALSE_REPORT),
        });
    }
    true
}

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
    if in_camp(records, club, today) {
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

/// The picked category 7 (`case 7` with `FUN_005fbaf9`): one of the
/// club's own real players married and suspected of his affair (+0x1b4
/// bit 8, `FUN_00702a7b`) is drawn (0x23 at most in the list) and his
/// wife acts on it ([`affair_known_day`]); with none the news did not
/// take place.
pub fn affair_known(records: &mut Records, club: (u16, u16), posted: &mut Vec<Posted>, random: &mut Random) -> bool {
    let men = drawn_from(records, club, |p| p.partner() & 3 == WIFE && p.shown() & SUSPECTED != 0);
    if men.is_empty() {
        return false;
    }
    let id = men[random.below(men.len() as u32) as usize];
    affair_known_day(records, club, id, posted, random);
    true
}

/// His wife knows (`FUN_005fba1a`, "EreignisFrauWeissAffaere"): one of
/// two happenings weighed alike, none while the two do not stand calm.
/// She makes him jealous until he ends the affair (+0x284 cleared, bits
/// 4 and 8 off, `FUN_00702962`; 0x469), or she files for divorce (their
/// state 4, `FUN_00702c39`; 0x46a).
fn affair_known_day(records: &mut Records, club: (u16, u16), id: u16, posted: &mut Vec<Posted>, random: &mut Random) {
    let happening = weighed(&KNOWN_HAPPENINGS, random);
    let Some(p) = records.player_mut(id) else { return };
    if state(p) != 0 {
        return;
    }
    let text = if happening == 1 {
        p.set_affair_name(0);
        p.set_shown(p.shown() & !(AFFAIR | SUSPECTED));
        0x469
    } else {
        p.set_partner(p.partner() & 0xe3 | DIVORCE << 2);
        0x46a
    };
    posted.push(Posted { club, code: PRIVATE_NEWS, text, fill: Fill::WifeByName(id) });
}

/// The picked category 12 (`case 0xc` with `FUN_005fd1d1`): one of the
/// club's own real players married (`FUN_006ffb14`) and less than 100
/// pleased with his private life is drawn (0x23 at most in the list) and
/// takes up a new affair, its first name drawn (`FUN_007029a7`) — the
/// bits of an affair at +0x1b4 stay as they are. With the trait of a
/// nude model's man he is told of as in firm hands (0x4e3), else as
/// married (0x4e4); the girlfriend's text (0x4e5) the list's wives never
/// reach. With none the news did not take place.
pub fn new_affair(records: &mut Records, club: (u16, u16), posted: &mut Vec<Posted>, random: &mut Random) -> bool {
    let men = drawn_from(records, club, |p| p.partner() & 3 == WIFE && p.contentment()[PRIVATE] < NO_AFFAIR_FROM);
    if men.is_empty() {
        return false;
    }
    let id = men[random.below(men.len() as u32) as usize];
    let Some(p) = records.player_mut(id) else { return true };
    let model = p.contentment()[PRIVATE] < NO_AFFAIR_FROM && p.traits()[1] & MODEL_TRAIT != 0;
    let (text, fill) = match p.partner() & 3 {
        _ if model => (0x4e3, Fill::Affair(id)),
        WIFE => (0x4e4, Fill::PartnerAndAffair(id)),
        GIRLFRIEND => (0x4e5, Fill::PartnerAndAffair(id)),
        _ => return true,
    };
    let (spread, first) = AFFAIR_NAMES;
    loop {
        let name = random.below(spread) as u8 + first;
        p.set_affair_name(name);
        if name != p.partner_name() {
            break;
        }
    }
    posted.push(Posted { club, code: PRIVATE_NEWS, text, fill });
    true
}

/// The club's own real players who meet `keep`, in the squad's order,
/// 0x23 at most — the list categories 7 and 12 draw one man from.
fn drawn_from(records: &Records, club: (u16, u16), keep: impl Fn(&crate::records::Player) -> bool) -> Vec<u16> {
    let squad = records.team(club.0, club.1).map(|t| t.players()).unwrap_or_default();
    squad
        .into_iter()
        .filter(|&id| id <= LAST_REAL_ID)
        .filter(|&id| records.player(id).is_some_and(&keep))
        .take(PICKS_MOST)
        .collect()
}

/// The picked category 8 (`case 8` with `FUN_005fc5b5`): one of the
/// club's own real players married and with children is drawn (0x23 at
/// most in the list) and has a day with his family ([`family_day`]);
/// with none the news did not take place.
pub fn family(
    records: &mut Records,
    club: (u16, u16),
    today: i32,
    league: &crate::league::LeagueTables,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    let men = drawn_from(records, club, |p| p.partner() & 3 == WIFE && p.children() > 0);
    if men.is_empty() {
        return false;
    }
    let id = men[random.below(men.len() as u32) as usize];
    family_day(records, club, id, today, league, posted, random);
    true
}

/// A father's day (`FUN_005fbbfd`, "EreignisKinderImAlter"): one of
/// eighteen happenings weighed alike, most about one of his children of
/// an age ([`child`], drawn before the other gates are asked). Told as
/// plain texts: grazed roller-skating with a son of 8 to 14, knocked a
/// week, the father older than 25 and neither out nor knocked (0x46c); a
/// cow on a farm, a child of 6 to 14 (0x46d); the toys he falls over,
/// three weeks out, with more than two children and not out (0x47a).
/// The others move his distractions (+0x294, `FUN_00701210`) and are
/// told with values beside the text (`FUN_00601043`, `FUN_006012df`,
/// `FUN_00601467`), which the replica does not post: a baby under 180
/// days old crying at night, 5 (0x46b); at the parents' evening, from
/// November to April, a son of 6 to 14 or a daughter of 12 to 16, 4
/// (0x46e, 0x46f); a son of 14 to 17 wrecking the car, 4 and a draw of 4
/// (0x470); a daughter of 12 to 16 running away, 5 and a draw of 5
/// (0x471); a son of 13 to 16 breaking into a car, twice in the table, 4
/// and a draw of 5 (0x472); from day 100 a son of 6 to 15 mocked for the
/// club missing its goal ([`crate::league::goal_missing`]), 2 (0x474);
/// with the two calm, his wife's broken leg, two sons of 3 to 14, 2
/// (0x475), the hamster, two children of 3 to 14, 5 (0x476), two of 11 to
/// 14 smoking, 5 and his private life 3 (0x477), the children's mess
/// with more than two, 4 (0x479); a daughter of 13 to 16 kissing the
/// neighbours' boy, 3 (0x478), or of 10 to 16 flooding the flat, the
/// table's last two, 4 (0x47c). The fight of two sons (0x47b) no
/// happening tells.
fn family_day(
    records: &mut Records,
    club: (u16, u16),
    id: u16,
    today: i32,
    league: &crate::league::LeagueTables,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) {
    let happening = weighed(&FAMILY_HAPPENINGS, random);
    let Some(p) = records.player(id) else { return };
    let married = p.partner() & 3 == WIFE;
    let calm = state(p) == 0;
    let children = p.children();
    let evening = PARENTS_EVENING.contains(&records.globals.date()[1]);
    let distract = |records: &mut Records, by: i32| change(records, id, DISTRACTION, by);
    let pair = |records: &Records, kind: Kid, years: RangeInclusive<i32>, random: &mut Random| {
        let one = child(records, id, kind, years.clone(), random);
        let other = child(records, id, kind, years, random);
        married && calm && one.is_some() && other.is_some() && one != other
    };
    match happening {
        1 => {
            if children > 0 && days_since_youngest(records, id) < NEWBORN_DAYS {
                distract(records, 0);
                distract(records, -5);
            }
        }
        2 => {
            let birth = (p.birth_day(), p.birth_month(), p.birth_year() as i16);
            if years_since(records, birth) > SKATING_FROM
                && crate::load::days_to(&records.globals, p.injured_to()) == 0
                && knocked_days(p, today) == 0
            {
                if let Some(k) = child(records, id, Kid::Son, 8..=14, random) {
                    crate::training_day::knock_for(records, id, GRAZED_DAYS, today);
                    posted.push(Posted { club, code: PRIVATE_NEWS, text: 0x46c, fill: Fill::Child(id, k) });
                }
            }
        }
        3 => {
            let k = child(records, id, Kid::Any, 6..=14, random);
            if let (true, Some(k)) = (married, k) {
                posted.push(Posted { club, code: PRIVATE_NEWS, text: 0x46d, fill: Fill::Child(id, k) });
            }
        }
        4 | 5 => {
            let (kind, years) = if happening == 4 { (Kid::Son, 6..=14) } else { (Kid::Daughter, 12..=16) };
            if child(records, id, kind, years, random).is_some() && married && evening {
                distract(records, 0);
                distract(records, -4);
            }
        }
        6 => {
            if child(records, id, Kid::Son, 14..=17, random).is_some() && married {
                distract(records, 0);
                let more = random.below(4) as i32;
                distract(records, -4 - more);
            }
        }
        7..=9 => {
            let (kind, years, by) = if happening == 7 { (Kid::Daughter, 12..=16, -5) } else { (Kid::Son, 13..=16, -4) };
            if child(records, id, kind, years, random).is_some() && married {
                let more = random.below(5) as i32;
                distract(records, by - more);
            }
        }
        10 => {
            let missing = records.team(club.0, club.1).map_or(0, |t| crate::league::goal_missing(records, league, t));
            if today >= MOCKED_FROM && missing != 0 && child(records, id, Kid::Son, 6..=15, random).is_some() && married {
                distract(records, -2);
            }
        }
        11 => {
            if pair(records, Kid::Son, 3..=14, random) {
                distract(records, -2);
            }
        }
        12 => {
            if pair(records, Kid::Any, 3..=14, random) {
                distract(records, -5);
            }
        }
        13 => {
            if pair(records, Kid::Any, 11..=14, random) {
                change(records, id, PRIVATE, -3);
                distract(records, -5);
            }
        }
        14 | 17 | 18 => {
            let (years, by) = if happening == 14 { (13..=16, -3) } else { (10..=16, -4) };
            if child(records, id, Kid::Daughter, years, random).is_some() {
                distract(records, by);
            }
        }
        15 => {
            if married && calm && children > 2 {
                distract(records, -4);
            }
        }
        16 => {
            if children > 2 && crate::load::days_to(&records.globals, p.injured_to()) == 0 {
                crate::training_day::injure(records, id, TOYS_INJURY.0, TOYS_INJURY.1, today);
                posted.push(Posted { club, code: PRIVATE_NEWS, text: 0x47a, fill: Fill::Partnered(id) });
            }
        }
        _ => {}
    }
}

/// The picked category 9 (`case 9` with `FUN_005fa5f8`): the club's own
/// real men engaged (+0x170, `FUN_00702a9b`) go into a list (0x23 at
/// most), yet none of it is drawn: the day ("EreignisKurzVorHochzeit",
/// `FUN_005fa394`) is handed the squad slot past the last (the club's
/// +0xb50 at the index the walk ended on, its +0xe2), as the original
/// does. One of three happenings alike, one met lately not met again
/// (23000 and its number). The first two ask that slot's man for a
/// girlfriend, calm (she calls off the wedding: parting, his affair gone,
/// 30 and a draw of 10 less pleased with his private life, 0x445) or
/// with an affair (the other girlfriend's number found, `FUN_0070282a`,
/// 0x446), both told with values and not posted here; the third is
/// nothing. Out of a loaded save the slot past the last holds no man
/// the replica knows of, so neither gate is met and nothing more is
/// drawn. Whether the news took place — with no one engaged it did not.
pub fn engaged(records: &Records, club: (u16, u16), recent: &mut Vec<i32>, random: &mut Random) -> bool {
    if drawn_from(records, club, |p| p.get::<u8>(ENGAGED) != 0).is_empty() {
        return false;
    }
    let happening = weighed(&WEDDING_HAPPENINGS, random);
    meet(recent, WEDDING_KEY + i32::from(happening));
    true
}

/// The picked category 10 (`case 10` with `FUN_005fc8e6`): one of the
/// club's own real men whose private life and distractions (+0x293 and
/// +0x294) together please him less than 0xa0 is drawn (0x23 at most in
/// the list) and has a bad day ([`low_day`]); with none the news did not
/// take place. `planned` is whether the club's plan (its +0xc0a) has
/// anything but free for today.
pub fn low_spirits(
    records: &mut Records,
    club: (u16, u16),
    planned: bool,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    let men = drawn_from(records, club, |p| {
        let c = p.contentment();
        i32::from(c[PRIVATE]) + i32::from(c[DISTRACTION]) < LOW_SPIRITS
    });
    if men.is_empty() {
        return false;
    }
    let id = men[random.below(men.len() as u32) as usize];
    low_day(records, club, id, planned, posted, random);
    true
}

/// A bad day (`FUN_005fc6bc`, "EreignisNichtGut"): one of six
/// happenings weighed 30, 3, 3, 3, 30, 30. He is not well (0x47d). His
/// licence (+0x28e, months, `FUN_00702d31`) taken while he has it: a
/// month for speeding to a training the plan holds today (0x47e,
/// `FUN_005ea8cc`), six for a tree hit drunk (0x47f), six for driving
/// on the left, not in the clubs of the countries 0x10 and 0x39 (0x480).
/// A man of the trait bits 0x2610 starts drinking (+0x1b4 bits
/// 0x800200, `FUN_00703090`), told when a spy (bit 0x8000,
/// `FUN_006494b8`) is among the squad of a club numbered other than 200
/// (0x481 — its `%1` him, its `%2` empty, as the original fills it). A
/// man without his licence caught riding without a ticket, 3 less
/// pleased with his private life, told with values (0x4d5), not posted.
fn low_day(records: &mut Records, club: (u16, u16), id: u16, planned: bool, posted: &mut Vec<Posted>, random: &mut Random) {
    let happening = weighed(&LOW_HAPPENINGS, random);
    let squad = records.team(club.0, club.1).map(|t| t.players()).unwrap_or_default();
    let spied = club.1 != NO_SPIES && squad.iter().any(|&m| records.player(m).is_some_and(|p| p.shown() & SPY != 0));
    let Some(p) = records.player_mut(id) else { return };
    let licence = p.get::<u8>(NO_LICENCE);
    let text = match happening {
        1 => 0x47d,
        2 if licence == 0 && planned => {
            p.put::<u8>(NO_LICENCE, 1);
            0x47e
        }
        3 if licence == 0 => {
            p.put::<u8>(NO_LICENCE, 6);
            0x47f
        }
        4 if licence == 0 && !LEFT_DRIVING.contains(&club.0) => {
            p.put::<u8>(NO_LICENCE, 6);
            0x480
        }
        6 if p.traits()[1] & DRINKER != 0 => {
            p.set_shown(p.shown() | DRINKS);
            if !spied {
                return;
            }
            0x481
        }
        7 if licence > 0 => {
            crate::mood::change(p, PRIVATE, -3);
            return;
        }
        _ => return,
    };
    posted.push(Posted { club, code: PRIVATE_NEWS, text, fill: Fill::NumberedOne(id) });
}

/// The picked category 11 (`case 0xb` with `FUN_005fcfcd`): nothing while
/// the club stands in a camp of its trainer's calendar (`FUN_005eab71`,
/// yet the news took place); else of the club's own real men at odds
/// with their partner (state 1, `FUN_00702c12`), 0x23 at most, one is
/// drawn ("EreignisVersEnd"): 70 times in 100 the two make up
/// (`FUN_005fcd09`), his private life set to please him 100
/// (`FUN_0070117e`), told of his girlfriend (0x4db) or else his wife
/// (0x4dc); else it ends ([`row_end`]). With none the news did not take
/// place.
pub fn at_odds(records: &mut Records, club: (u16, u16), today: i32, posted: &mut Vec<Posted>, random: &mut Random) -> bool {
    if in_camp(records, club, today) {
        return true;
    }
    let men = drawn_from(records, club, |p| state(p) == AT_ODDS);
    if men.is_empty() {
        return false;
    }
    if random.below(MAKE_UP_IN) >= MAKE_UP_UNDER {
        let id = men[random.below(men.len() as u32) as usize];
        return row_end(records, id, random);
    }
    let id = men[random.below(men.len() as u32) as usize];
    let Some(p) = records.player_mut(id) else { return true };
    crate::mood::change(p, PRIVATE, MADE_UP - i32::from(p.contentment()[PRIVATE]));
    let text = if p.partner() & 3 == GIRLFRIEND { 0x4db } else { 0x4dc };
    posted.push(Posted { club, code: PRIVATE_NEWS, text, fill: Fill::Partnered(id) });
    true
}

/// How the row ends (`FUN_005fcd9d`): one of seven happenings weighed
/// 30, 3, 3, 3, 3, 30, 30, the first two a girlfriend's and the next two
/// a wife's, swapped to fit him. She leaves him for good, 10 less pleased
/// with his private life (0x4dd, 0x4df); he is done with her, 5 more
/// (0x4de, 0x4e0); a married man before the divorce judge, 15 less
/// (0x4e1) — all told with values, not posted here. The last two are
/// none: the news did not take place.
fn row_end(records: &mut Records, id: u16, random: &mut Random) -> bool {
    let mut happening = weighed(&ROW_HAPPENINGS, random);
    let Some(p) = records.player_mut(id) else { return true };
    let girlfriend = p.partner() & 3 == GIRLFRIEND;
    match happening {
        1 | 2 if !girlfriend => happening += 2,
        3 | 4 if girlfriend => happening -= 2,
        _ => {}
    }
    let by = match happening {
        1 | 3 => -10,
        2 | 4 => 5,
        5 if p.partner() & 3 == WIFE => -15,
        5 => 0,
        _ => return false,
    };
    if by != 0 {
        crate::mood::change(p, PRIVATE, by);
    }
    true
}

/// Whether the club stands in a camp of its trainer's calendar (the
/// club's +0x8a, `FUN_005eab71`): never from day 0x16b on.
fn in_camp(records: &Records, club: (u16, u16), today: i32) -> bool {
    records
        .team(club.0, club.1)
        .and_then(|team| usize::from(team.humans()[0]).checked_sub(1))
        .and_then(|trainer| records.calendars.get(trainer))
        .zip(usize::try_from(today).ok())
        .is_some_and(|(calendar, d)| d < crate::year::CAMP_DAYS && crate::year::in_camp(calendar, d))
}

/// The picked category 50 (`case 0x32` with `FUN_005e4dde`): one of the
/// club's own real bootlickers (+0x1b4 bit 0x4000, 0x23 at most) is
/// drawn and has his day ([`bootlicker_day`]); with none the news did
/// not take place.
pub fn bootlicker(
    records: &mut Records,
    club: (u16, u16),
    today: i32,
    planned: bool,
    (league, mood): (&crate::league::LeagueTables, &crate::mood::MoodTables),
    recent: &mut Vec<i32>,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    let men = drawn_from(records, club, |p| p.shown() & BOOTLICKER != 0);
    if men.is_empty() {
        return false;
    }
    let id = men[random.below(men.len() as u32) as usize];
    bootlicker_day(records, club, id, today, planned, (league, mood), recent, posted, random);
    true
}

/// A bootlicker's day (`FUN_005e4a3e`, "EreignisSchleimer"): one of
/// twenty happenings weighed 3 and 200 each, one met lately not met
/// again (26000 and its number), each text's `%1` him and `%2` empty
/// (0x3ba asks for a `%s` the filling leaves, 0x3c9 for his wife's
/// name, both as the original shows them). The team's mood ten down
/// (`FUN_007147ce`, `FUN_00771701`; 0x3b8); the club short of its
/// season's goal (`FUN_00775f0a`, 0x3b9) or not (0x3c1); 3, 4, 8 and 9
/// on a day the plan holds training (`FUN_005ea8cc`; 0x3ba, 0x3bb,
/// 0x3bf, 0x3c0); 5, 7 and 19 always (0x3bc, 0x3be, 0x3ca); a married
/// man (`FUN_006ffb14`) in December before the 24th (0x3c8) or any day
/// (0x3c9); the first seven days of the counter (0x3cb). The rest are
/// nothing; the news took place all the same.
fn bootlicker_day(
    records: &mut Records,
    club: (u16, u16),
    id: u16,
    today: i32,
    planned: bool,
    (league, mood): (&crate::league::LeagueTables, &crate::mood::MoodTables),
    recent: &mut Vec<i32>,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) {
    let happening = weighed(&BOOTLICKER_HAPPENINGS, random);
    if !meet(recent, BOOTLICKER_KEY + i32::from(happening)) {
        return;
    }
    let missing = records.team(club.0, club.1).map_or(0, |team| crate::league::goal_missing(records, league, team));
    let married = records.player(id).is_some_and(|p| p.partner() & 3 == WIFE);
    let [day, month, _] = records.globals.date().map(|w| i32::from(w as i16));
    let text = match happening {
        1 => {
            crate::interview::team_mood(records, mood, (club.0 as u8, club.1 as i16), BOOTLICKED_MOOD);
            0x3b8
        }
        2 if missing > 0 => 0x3b9,
        3 if planned => 0x3ba,
        4 if planned => 0x3bb,
        5 => 0x3bc,
        7 => 0x3be,
        8 if planned => 0x3bf,
        9 if planned => 0x3c0,
        10 if missing == 0 => 0x3c1,
        17 if married && month == i32::from(CHRISTMAS_EVE.1) && day < i32::from(CHRISTMAS_EVE.0) => 0x3c8,
        18 if married => 0x3c9,
        19 => 0x3ca,
        20 if today < FIRST_DAYS => 0x3cb,
        _ => return,
    };
    posted.push(Posted { club, code: PRIVATE_NEWS, text, fill: Fill::NumberedOne(id) });
}

/// The picked category 51 (`case 0x33` with `FUN_005e4fd6`): one of the
/// club's own real informers (+0x1b4 bit 0x8000, 0x23 at most) is drawn
/// ("EreignisSpitzel", `FUN_005e4ec2`), one of two happenings weighed 10
/// and 50. Found out: every man of the squad ten less pleased with the
/// trainer (`FUN_007007a1`), the informer shut out (`FUN_0070031f`) and
/// not pleased with the team at all (`FUN_00700bcf(0)`), told (0x3d7).
/// Else a mole of the squad's real men (+0x1b4 bit 0x10000, 0x23 at
/// most, `FUN_005edd19`) is drawn and, one of the informer's three
/// friends (+0x1f8) not shut out (`FUN_0070346f`), told by name (0x3d6)
/// — and then, as with none, the news did not take place.
pub fn informer(records: &mut Records, club: (u16, u16), posted: &mut Vec<Posted>, random: &mut Random) -> bool {
    let men = drawn_from(records, club, |p| p.shown() & SPY != 0);
    if men.is_empty() {
        return false;
    }
    let id = men[random.below(men.len() as u32) as usize];
    if weighed(&INFORMER_HAPPENINGS, random) == 1 {
        for man in records.team(club.0, club.1).map(|t| t.players()).unwrap_or_default() {
            change(records, man, 0, FOUND_OUT);
        }
        let Some(p) = records.player_mut(id) else { return true };
        p.set_shown(p.shown() | SHUT_OUT);
        crate::mood::change(p, TEAM, -crate::mood::MOST);
        posted.push(Posted { club, code: PRIVATE_NEWS, text: 0x3d7, fill: Fill::NumberedOne(id) });
        return true;
    }
    let moles = drawn_from(records, club, |p| p.shown() & MOLE != 0);
    if moles.is_empty() {
        return false;
    }
    let mole = moles[random.below(moles.len() as u32) as usize];
    let shut_out = records.player(mole).is_some_and(|p| p.shown() & SHUT_OUT != 0);
    if !shut_out && records.player(id).is_some_and(|p| p.friends().contains(&mole)) {
        posted.push(Posted { club, code: PRIVATE_NEWS, text: 0x3d6, fill: Fill::Numbered(id, mole) });
    }
    false
}

/// The picked category 52 (`case 0x34` with `FUN_005e50ba`): over the
/// club's own real men, those pleased with the trainer less than 0x5a
/// and those wanting to leave (`FUN_00770877`) are counted; a mole of
/// morale 0x56 or more stops (`FUN_00700294(0)`), one under it is listed;
/// a man not a mole of morale under 0x46, none of the trait bits 0x120
/// (the handle's +0x12), with two of trainer, team and club under 0x58
/// is listed to turn one (0x23 at most in each list). With no mole one
/// of the second list is drawn and turns mole, nothing told; with
/// neither the news did not take place. With moles the press hears of
/// the club (the club object's flags +0xdf8): of seven men low with the
/// trainer (0x6a5) or of five wanting to leave (0x6a6), each once, the
/// round stepped (`FUN_0069fe73(1)`); else, once a round, of the human
/// trainer by the round's state (0x6a7 to 0x6aa — 0x6ab and 0x6ac ask
/// for states three bits never hold). Those headlines (`FUN_00789c32`)
/// are not posted here; the flags are kept.
pub fn moles(records: &mut Records, club: (u16, u16), random: &mut Random) -> bool {
    let squad = records.team(club.0, club.1).map(|t| t.players()).unwrap_or_default();
    let (mut low, mut leaving, mut moles, mut turning) = (0, 0, Vec::new(), Vec::new());
    for id in squad.into_iter().filter(|&id| id <= LAST_REAL_ID) {
        let Some(p) = records.player_mut(id) else { continue };
        let parts = crate::mood::parts(p);
        low += usize::from(parts[0] < LOW_TRAINER);
        leaving += usize::from(crate::mood::content_level(parts) == LEAVING);
        if p.shown() & MOLE == 0 {
            let unhappy = parts[..3].iter().filter(|&&part| part < MOLE_PARTS).count();
            if p.morale() < MOLE_MORALE && p.traits()[1] & MOLE_TRAITS == 0 && unhappy > 1 && turning.len() < PICKS_MOST {
                turning.push(id);
            }
        } else if p.morale() < MOLE_STOPS {
            if moles.len() < PICKS_MOST {
                moles.push(id);
            }
        } else {
            p.set_shown(p.shown() & !MOLE);
        }
    }
    if moles.is_empty() {
        if turning.is_empty() {
            return false;
        }
        let id = turning[random.below(turning.len() as u32) as usize];
        if let Some(p) = records.player_mut(id) {
            p.set_shown(p.shown() | MOLE);
        }
        return true;
    }
    let Some(team) = records.team_mut(club.0, club.1) else { return true };
    let flags = team.training_flags();
    let step = |flags: u32| {
        let round = ((flags & PRESS_ROUND) >> PRESS_SHIFT) + 1;
        (flags & !PRESS_ROUND | round.min(PRESS_MOST) << PRESS_SHIFT) & !TOLD_TRAINER
    };
    if low >= LOW_MANY && flags & TOLD_LOW == 0 {
        team.set_training_flags(step(flags | TOLD_LOW));
    } else if leaving >= LEAVING_MANY && flags & TOLD_LEAVING == 0 {
        team.set_training_flags(step(flags | TOLD_LEAVING));
    } else if flags & TOLD_TRAINER == 0 && team.humans()[0] != 0 {
        team.set_training_flags(flags | TOLD_TRAINER);
    }
    true
}

/// The picked category 55 (`case 0x37` with `FUN_005fff0f`): one man is
/// drawn over the club's whole squad (+0xe2); a player-manager has
/// nothing. Of four habits one: he takes up smoking while the club is
/// short of its season's goal (`FUN_00775fa0` below nought; 0x54c) or
/// gives it up while it is not (0x54d), takes on a private fitness
/// trainer (0x54e) or parts from him (0x54f) — each only when he has it
/// not, or has it. The news took place all the same.
pub fn habits(
    records: &mut Records,
    club: (u16, u16),
    league: &crate::league::LeagueTables,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    let Some(team) = records.team(club.0, club.1) else { return true };
    let squad = team.players();
    let at = random.below(u32::from(team.player_count())) as usize;
    let Some(&id) = squad.get(at) else { return true };
    if id > LAST_REAL_ID {
        return true;
    }
    let reached = crate::league::goal_reached(records, league, team);
    let short = reached < 1 && crate::league::goal_missing(records, league, team) > 0;
    let Some(p) = records.player_mut(id) else { return true };
    let shown = p.shown();
    let (text, bits) = match random.below(HABITS) {
        0 if shown & SMOKES == 0 && short => (0x54c, shown | SMOKES),
        1 if shown & SMOKES != 0 && !short => (0x54d, shown & !SMOKES),
        2 if shown & OWN_TRAINER == 0 => (0x54e, shown | OWN_TRAINER),
        3 if shown & OWN_TRAINER != 0 => (0x54f, shown & !OWN_TRAINER),
        _ => return true,
    };
    p.set_shown(bits);
    posted.push(Posted { club, code: PRIVATE_NEWS, text, fill: Fill::NumberedOne(id) });
    true
}

/// Whether a text is one of categories 50, 51 and 55's.
pub fn squad_text(text: u16) -> bool {
    (0x3b8..=0x3cb).contains(&text) || text == 0x3d6 || text == 0x3d7 || (0x54c..=0x54f).contains(&text)
}

/// Which of his children [`child`] draws among (`FUN_007021cb`'s first
/// argument: 0, 1, any other).
#[derive(Clone, Copy)]
enum Kid {
    Son,
    Daughter,
    Any,
}

/// One of his children of `kind` whose age (`FUN_0070202a`, the birth
/// dates at +0x174) lies in `years`, drawn (`FUN_007021cb`): his number,
/// none with no draw.
fn child(records: &Records, id: u16, kind: Kid, years: RangeInclusive<i32>, random: &mut Random) -> Option<u8> {
    let p = records.player(id)?;
    let fits: Vec<u8> = (0..p.children().clamp(0, MOST_CHILDREN) as u8)
        .filter(|&k| {
            let son = p.get::<u8>(0x288 + usize::from(k)) < SONS_NAMES;
            match kind {
                Kid::Son => son,
                Kid::Daughter => !son,
                Kid::Any => true,
            }
        })
        .filter(|&k| years.contains(&years_since(records, birth_date(p, k))))
        .collect();
    (!fits.is_empty()).then(|| fits[random.below(fits.len() as u32) as usize])
}

/// The day of birth of his child `k` (day, month, year from +0x174).
fn birth_date(p: &crate::records::Player, k: u8) -> (u8, u8, i16) {
    let at = BIRTH_DATES + 4 * usize::from(k);
    (p.get::<u8>(at), p.get::<u8>(at + 1), p.get::<i16>(at + 2))
}

/// Full years since a date as the game counts them (`FUN_0067582a`):
/// one more once its day of the year is reached, none below nought
/// before.
fn years_since(records: &Records, (day, month, year): (u8, u8, i16)) -> i32 {
    let [today, this_month, this_year] = records.globals.date().map(|w| i32::from(w as i16));
    let years = this_year - i32::from(year) - 1;
    let (day, month) = (i32::from(day), i32::from(month));
    if month < this_month || month == this_month && day <= today {
        years + 1
    } else {
        years.max(0)
    }
}

/// Days since his youngest child was born (`FUN_00702084`), -1 with none.
fn days_since_youngest(records: &Records, id: u16) -> i32 {
    let Some(p) = records.player(id) else { return -1 };
    if p.children() < 1 {
        return -1;
    }
    let (day, month, year) = birth_date(p, (p.children().min(MOST_CHILDREN) - 1) as u8);
    let [d, m, y] = records.globals.date().map(|w| i32::from(w as i16));
    crate::goodbye::day_number(d, m, y) - crate::goodbye::day_number(day.into(), month.into(), year.into())
}

/// The days he is still knocked about, as the training's handle holds
/// them (+0x2f, which `FUN_00636638` reckons out of the day at +0x1de).
fn knocked_days(p: &crate::records::Player, today: i32) -> i32 {
    match p.get::<u16>(0x1de) {
        0 => 0,
        to => (i32::from(to) - today).max(0),
    }
}

/// Whether a text is one of categories 3 to 12's.
pub fn partner_text(text: u16) -> bool {
    (0x44b..=0x481).contains(&text) || (0x4db..=0x4e5).contains(&text) || text == 0x4d4 || text == 0x7be
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

    /// Categories 7 and 12 on A's human squad, one man fitting each: a
    /// wife who knows ends his affair (0x469) with her full name, or files
    /// for divorce (0x46a), not again once the two no longer stand calm;
    /// an unhappy married man takes up a new affair (0x4e4), its name
    /// not his wife's, or with the trait 0x200 a nude model (0x4e3). A
    /// list of none draws nothing.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn categories_7_and_12_tell_of_affairs() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let texts = a3_files::saq::EventTexts::load(&dir).unwrap();
        let words = crate::words::Words::load(&a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap()).unwrap();
        // Every man of the squad with a girlfriend and pleased, the man
        // `partner` and `shown` bits, calm.
        let load = |partner: u8, shown: u32| {
            let mut a = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
            let (hc, hn) = crate::statistics::human_clubs(&a)[0];
            let club = (u16::from(hc), hn as u16);
            let squad = a.team(club.0, club.1).unwrap().players();
            let man = squad[3];
            for &id in &squad {
                let p = a.player_mut(id).unwrap();
                p.set_partner(GIRLFRIEND);
                p.set_shown(p.shown() & !(AFFAIR | SUSPECTED));
                p.set_affair_name(0);
                let mut c = p.contentment();
                c[PRIVATE] = 100;
                p.set_contentment(c);
            }
            let p = a.player_mut(man).unwrap();
            p.set_partner(partner);
            p.set_shown(p.shown() | shown);
            p.set_affair_name(66);
            (a, club, man)
        };
        let seed = |n: u16| {
            (0..10_000)
                .find(|&s| {
                    let mut r = Random::new(s);
                    r.below(1);
                    weighed(&KNOWN_HAPPENINGS, &mut r) == n
                })
                .unwrap()
        };
        let told = |r: &Records, posted: &Posted, man: u16| {
            let event = crate::event_text::make(r, &texts, &|_| String::new(), &words.first_names, posted);
            assert!(!event.text.contains('%') && event.text.contains(&r.known_as(man).unwrap()), "{}", event.text);
            event.text
        };
        let first = |r: &Records, man: u16, name: u8| {
            words.first_names[usize::from(r.player(man).unwrap().names_nation()) * 100 + usize::from(name)].clone()
        };

        // She makes him end it.
        let (mut r, club, man) = load(WIFE, AFFAIR | SUSPECTED);
        let mut posted = Vec::new();
        assert!(affair_known(&mut r, club, &mut posted, &mut Random::new(seed(1))));
        assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text: 0x469, fill: Fill::WifeByName(man) }]);
        let p = r.player(man).unwrap();
        assert_eq!((p.affair_name(), p.shown() & (AFFAIR | SUSPECTED)), (0, 0));
        let wife = first(&r, man, p.partner_name());
        let full = format!("{wife} {}", r.name(p.last_name()));
        assert!(told(&r, &posted[0], man).starts_with(&full));
        // She files for divorce, and then stands so.
        let (mut r, club, man) = load(WIFE, SUSPECTED);
        let mut posted = Vec::new();
        assert!(affair_known(&mut r, club, &mut posted, &mut Random::new(seed(2))));
        assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text: 0x46a, fill: Fill::WifeByName(man) }]);
        assert_eq!(state(r.player(man).unwrap()), DIVORCE);
        told(&r, &posted[0], man);
        assert!(affair_known(&mut r, club, &mut posted, &mut Random::new(seed(1))));
        assert_eq!(posted.len(), 1);
        // Not suspected: no one, no draw.
        let (mut r, club, _) = load(WIFE, AFFAIR);
        let mut random = Random::new(3);
        assert!(!affair_known(&mut r, club, &mut Vec::new(), &mut random));
        assert!(random == Random::new(3));

        // A new affair of an unhappy married man.
        let (mut r, club, man) = load(WIFE, 0);
        let p = r.player_mut(man).unwrap();
        let mut c = p.contentment();
        c[PRIVATE] = 99;
        p.set_contentment(c);
        let mut traits = p.traits();
        traits[1] &= !MODEL_TRAIT;
        p.set_traits(traits);
        let shown = p.shown();
        for s in 0..20 {
            let mut posted = Vec::new();
            assert!(new_affair(&mut r, club, &mut posted, &mut Random::new(s)));
            assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text: 0x4e4, fill: Fill::PartnerAndAffair(man) }]);
            let p = r.player(man).unwrap();
            assert!((50..100).contains(&p.affair_name()) && p.affair_name() != p.partner_name());
            assert_eq!(p.shown(), shown);
            let text = told(&r, &posted[0], man);
            assert!(text.contains(&first(&r, man, p.affair_name())), "{text}");
        }
        // The nude model.
        let p = r.player_mut(man).unwrap();
        let mut traits = p.traits();
        traits[1] |= MODEL_TRAIT;
        p.set_traits(traits);
        let mut posted = Vec::new();
        assert!(new_affair(&mut r, club, &mut posted, &mut Random::new(5)));
        assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text: 0x4e3, fill: Fill::Affair(man) }]);
        told(&r, &posted[0], man);
        // Pleased enough: no one, no draw.
        let (mut r, club, _) = load(WIFE, 0);
        let mut random = Random::new(3);
        assert!(!new_affair(&mut r, club, &mut Vec::new(), &mut random));
        assert!(random == Random::new(3));
    }

    /// Category 8 on A's human squad, one married father of a son of ten:
    /// roller-skating grazes him a week (0x46c, the son named), not when
    /// he is knocked already; the toys of three children put him three
    /// weeks out (0x47a); a baby born today costs him 5 of his
    /// distractions, told with values and so not posted here. A squad
    /// with no married father draws nothing.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn category_8_tells_of_a_fathers_family() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let exe = a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap();
        let texts = a3_files::saq::EventTexts::load(&dir).unwrap();
        let words = crate::words::Words::load(&exe).unwrap();
        let league = crate::league::LeagueTables::load(&exe).unwrap();
        // Every man of the squad with a girlfriend, the man married, 30,
        // fit, with children born `ago` years before today, the first a
        // son (name 7), the others daughters.
        let load = |children: i16, ago: u16| {
            let mut a = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
            let (hc, hn) = crate::statistics::human_clubs(&a)[0];
            let club = (u16::from(hc), hn as u16);
            let squad = a.team(club.0, club.1).unwrap().players();
            let man = squad[3];
            let [d, m, y] = a.globals.date();
            for &id in &squad {
                a.player_mut(id).unwrap().set_partner(GIRLFRIEND);
            }
            let p = a.player_mut(man).unwrap();
            p.set_partner(WIFE);
            p.set_birth_year(y - 30);
            p.set_injured_to([0, 0]);
            p.put::<u16>(0x1de, 0);
            p.set_children(children);
            p.set_children_names([7, 60, 61, 62, 63]);
            for k in 0..children as usize {
                let at = BIRTH_DATES + 4 * k;
                p.put::<u8>(at, d as u8);
                p.put::<u8>(at + 1, m as u8);
                p.put::<u16>(at + 2, y - ago);
            }
            let mut c = p.contentment();
            c[DISTRACTION] = 80;
            p.set_contentment(c);
            (a, club, man)
        };
        let seed = |n: u16| {
            (0..10_000)
                .find(|&s| {
                    let mut r = Random::new(s);
                    r.below(1);
                    weighed(&FAMILY_HAPPENINGS, &mut r) == n
                })
                .unwrap()
        };
        let told = |r: &Records, posted: &Posted, man: u16| {
            let event = crate::event_text::make(r, &texts, &|_| String::new(), &words.first_names, posted);
            assert!(!event.text.contains('%') && event.text.contains(&r.known_as(man).unwrap()), "{}", event.text);
            event.text
        };

        // Roller-skating with his son of ten.
        let (mut r, club, man) = load(1, 10);
        let today = i32::from(r.globals.day());
        let mut posted = Vec::new();
        assert!(family(&mut r, club, today, &league, &mut posted, &mut Random::new(seed(2))));
        assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text: 0x46c, fill: Fill::Child(man, 0) }]);
        let son = &words.first_names[usize::from(r.player(man).unwrap().names_nation()) * 100 + 7];
        assert!(told(&r, &posted[0], man).contains(son.as_str()));
        assert_eq!(r.player(man).unwrap().get::<u16>(0x1de), today as u16 + 7);
        // Knocked already: nothing.
        let mut posted = Vec::new();
        assert!(family(&mut r, club, today, &league, &mut posted, &mut Random::new(seed(2))));
        assert!(posted.is_empty());
        // A son too young for it.
        let (mut r, club, _) = load(1, 5);
        let mut posted = Vec::new();
        assert!(family(&mut r, club, today, &league, &mut posted, &mut Random::new(seed(2))));
        assert!(posted.is_empty());

        // The toys of three children.
        let (mut r, club, man) = load(3, 4);
        let mut posted = Vec::new();
        assert!(family(&mut r, club, today, &league, &mut posted, &mut Random::new(seed(16))));
        assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text: 0x47a, fill: Fill::Partnered(man) }]);
        told(&r, &posted[0], man);
        let p = r.player(man).unwrap();
        assert_eq!((p.injury_kind(), crate::load::days_to(&r.globals, p.injured_to())), TOYS_INJURY);

        // A baby born today: 5 less, nothing posted.
        let (mut r, club, man) = load(1, 0);
        let mut posted = Vec::new();
        assert!(family(&mut r, club, today, &league, &mut posted, &mut Random::new(seed(1))));
        assert!(posted.is_empty());
        assert_eq!(r.player(man).unwrap().contentment()[DISTRACTION], 75);

        // No married father: no one, no draw.
        let (mut r, club, man) = load(0, 0);
        r.player_mut(man).unwrap().set_partner(GIRLFRIEND);
        let mut random = Random::new(3);
        assert!(!family(&mut r, club, today, &league, &mut Vec::new(), &mut random));
        assert!(random == Random::new(3));
    }

    /// Categories 9 to 11 on A's human squad, one man fitting each: an
    /// engaged man's day takes one draw and meets its happening once,
    /// handed no man; a low man is not well (0x47d), loses his licence
    /// for six months (0x47f) and not again, for a month only on a day
    /// with training (0x47e), rides without a ticket once without it, and
    /// takes to drink, told only with a spy in the squad (0x481); two at
    /// odds make up (0x4db, 0x4dc), his private life at 100, or their
    /// row ends in nothing. A list of none draws nothing.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn categories_9_to_11_tell_of_moods() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let texts = a3_files::saq::EventTexts::load(&dir).unwrap();
        let words = crate::words::Words::load(&a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap()).unwrap();
        // Every man of the squad pleased, calm, not engaged, licensed,
        // no spy; the man with `partner`, his private life and
        // distractions at `low`.
        let load = |partner: u8, low: u8| {
            let mut a = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
            let (hc, hn) = crate::statistics::human_clubs(&a)[0];
            let club = (u16::from(hc), hn as u16);
            let squad = a.team(club.0, club.1).unwrap().players();
            let man = squad[3];
            for &id in &squad {
                let p = a.player_mut(id).unwrap();
                p.set_partner(GIRLFRIEND);
                p.put::<u8>(ENGAGED, 0);
                p.put::<u8>(NO_LICENCE, 0);
                p.set_shown(p.shown() & !(SPY | DRINKS));
                let mut c = p.contentment();
                c[PRIVATE] = 100;
                c[DISTRACTION] = 100;
                p.set_contentment(c);
            }
            let p = a.player_mut(man).unwrap();
            p.set_partner(partner);
            let mut c = p.contentment();
            c[PRIVATE] = low;
            c[DISTRACTION] = low;
            p.set_contentment(c);
            let mut traits = p.traits();
            traits[1] &= !DRINKER;
            p.set_traits(traits);
            (a, club, man, squad)
        };
        let seed = |draws: &dyn Fn(&mut Random) -> bool| (0..10_000).find(|&s| draws(&mut Random::new(s))).unwrap();
        let low = |n: u16| {
            seed(&|r| {
                r.below(1);
                weighed(&LOW_HAPPENINGS, r) == n
            })
        };
        let told = |r: &Records, posted: &Posted, man: u16| {
            let event = crate::event_text::make(r, &texts, &|_| String::new(), &words.first_names, posted);
            assert!(!event.text.contains('%') && event.text.contains(&r.known_as(man).unwrap()), "{}", event.text);
        };

        // An engaged man: one draw, its happening met once.
        let (mut r, club, man, _) = load(GIRLFRIEND, 100);
        let mut random = Random::new(3);
        assert!(!engaged(&r, club, &mut Vec::new(), &mut random));
        assert!(random == Random::new(3));
        r.player_mut(man).unwrap().put::<u8>(ENGAGED, 1);
        let mut recent = Vec::new();
        let mut random = Random::new(3);
        assert!(engaged(&r, club, &mut recent, &mut random));
        let mut once = Random::new(3);
        let happening = weighed(&WEDDING_HAPPENINGS, &mut once);
        assert!(random == once);
        assert_eq!(recent, [WEDDING_KEY + i32::from(happening)]);
        assert!(engaged(&r, club, &mut recent, &mut Random::new(3)));
        assert_eq!(recent.len(), 1);

        // A low man.
        let (mut r, club, man, squad) = load(GIRLFRIEND, 79);
        let mut posted = Vec::new();
        assert!(low_spirits(&mut r, club, false, &mut posted, &mut Random::new(low(1))));
        assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text: 0x47d, fill: Fill::NumberedOne(man) }]);
        told(&r, &posted[0], man);
        // Six months without his licence, then nothing more to take.
        let mut posted = Vec::new();
        assert!(low_spirits(&mut r, club, false, &mut posted, &mut Random::new(low(3))));
        assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text: 0x47f, fill: Fill::NumberedOne(man) }]);
        assert_eq!(r.player(man).unwrap().get::<u8>(NO_LICENCE), 6);
        assert!(low_spirits(&mut r, club, true, &mut posted, &mut Random::new(low(2))));
        assert_eq!(posted.len(), 1);
        // Without it he rides without a ticket: 3 less, not posted.
        assert!(low_spirits(&mut r, club, false, &mut posted, &mut Random::new(low(7))));
        assert_eq!((posted.len(), r.player(man).unwrap().contentment()[PRIVATE]), (1, 76));
        // A month for speeding only to a day's training.
        let (mut r, club, man, _) = load(GIRLFRIEND, 79);
        let mut posted = Vec::new();
        assert!(low_spirits(&mut r, club, false, &mut posted, &mut Random::new(low(2))));
        assert!(posted.is_empty());
        assert!(low_spirits(&mut r, club, true, &mut posted, &mut Random::new(low(2))));
        assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text: 0x47e, fill: Fill::NumberedOne(man) }]);
        assert_eq!(r.player(man).unwrap().get::<u8>(NO_LICENCE), 1);
        // A drinker, told once a spy is in the squad.
        let p = r.player_mut(man).unwrap();
        let mut traits = p.traits();
        traits[1] |= 0x10;
        p.set_traits(traits);
        let mut posted = Vec::new();
        assert!(low_spirits(&mut r, club, false, &mut posted, &mut Random::new(low(6))));
        assert!(posted.is_empty());
        assert_eq!(r.player(man).unwrap().shown() & DRINKS, DRINKS);
        let spy = r.player_mut(squad[0]).unwrap();
        spy.set_shown(spy.shown() | SPY);
        assert!(low_spirits(&mut r, club, false, &mut posted, &mut Random::new(low(6))));
        assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text: 0x481, fill: Fill::NumberedOne(man) }]);
        // Pleased enough: no one, no draw.
        let (mut r, club, _, _) = load(GIRLFRIEND, 80);
        let mut random = Random::new(3);
        assert!(!low_spirits(&mut r, club, true, &mut Vec::new(), &mut random));
        assert!(random == Random::new(3));

        // Two at odds make up.
        let make_up = seed(&|r| r.below(MAKE_UP_IN) < MAKE_UP_UNDER);
        for (partner, text) in [(GIRLFRIEND | AT_ODDS << 2, 0x4db), (WIFE | AT_ODDS << 2, 0x4dc)] {
            let (mut r, club, man, _) = load(partner, 60);
            let today = i32::from(r.globals.day());
            let mut posted = Vec::new();
            assert!(at_odds(&mut r, club, today, &mut posted, &mut Random::new(make_up)));
            assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text, fill: Fill::Partnered(man) }]);
            told(&r, &posted[0], man);
            assert_eq!(r.player(man).unwrap().contentment()[PRIVATE], 100);
        }
        // Their row ends: a wife's leaving, a girlfriend's told; the last
        // two none.
        let ends = |n: u16| {
            seed(&|r| {
                let out = r.below(MAKE_UP_IN) >= MAKE_UP_UNDER;
                r.below(1);
                out && weighed(&ROW_HAPPENINGS, r) == n
            })
        };
        let (mut r, club, man, _) = load(GIRLFRIEND | AT_ODDS << 2, 60);
        let today = i32::from(r.globals.day());
        assert!(!in_camp(&r, club, today));
        let mut posted = Vec::new();
        assert!(at_odds(&mut r, club, today, &mut posted, &mut Random::new(ends(3))));
        assert_eq!((posted.len(), r.player(man).unwrap().contentment()[PRIVATE]), (0, 50));
        assert!(!at_odds(&mut r, club, today, &mut posted, &mut Random::new(ends(6))));
        // Calm: no one, no draw.
        let (mut r, club, _, _) = load(GIRLFRIEND, 60);
        let mut random = Random::new(3);
        assert!(!at_odds(&mut r, club, today, &mut Vec::new(), &mut random));
        assert!(random == Random::new(3));
    }

    /// Categories 50 to 52 and 55 on A's human squad: a bootlicker's
    /// happening told once (0x3bc), the team's mood ten down (0x3b8), a
    /// training's only on a planned day (0x3ba); an informer found out
    /// (0x3d7) shut out, the squad less pleased with the trainer, or
    /// telling of a mole friend (0x3d6) with the news not taken place; an
    /// unhappy man turned mole, then the press's flags stepped and a
    /// pleased mole stopped; a habit taken up and given up. Lists of none
    /// draw nothing.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn categories_50_to_55_tell_of_the_squad() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let exe = a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap();
        let texts = a3_files::saq::EventTexts::load(&dir).unwrap();
        let words = crate::words::Words::load(&exe).unwrap();
        let league = crate::league::LeagueTables::load(&exe).unwrap();
        let mood = crate::mood::MoodTables::load(&exe).unwrap();
        // Every man of the squad pleased at 100 with no reasons, none of
        // the bits; `man` the fourth.
        let load = || {
            let mut a = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
            let (hc, hn) = crate::statistics::human_clubs(&a)[0];
            let club = (u16::from(hc), hn as u16);
            let squad = a.team(club.0, club.1).unwrap().players();
            for &id in &squad {
                let p = a.player_mut(id).unwrap();
                p.set_shown(p.shown() & !(BOOTLICKER | SPY | MOLE | SHUT_OUT | SMOKES | OWN_TRAINER));
                p.set_flags(0);
                p.set_contentment([100; 7]);
                crate::mood::change(p, 0, 0);
            }
            (a, club, squad[3], squad)
        };
        let seed = |draws: &dyn Fn(&mut Random) -> bool| (0..10_000).find(|&s| draws(&mut Random::new(s))).unwrap();
        let told = |r: &Records, posted: &Posted, man: u16| {
            let event = crate::event_text::make(r, &texts, &|_| String::new(), &words.first_names, posted);
            assert!(!event.text.contains('%') && event.text.contains(&r.known_as(man).unwrap()), "{}", event.text);
        };
        let one = |table: &'static [(u32, u16)], n: u16| {
            seed(&|r| {
                r.below(1);
                weighed(table, r) == n
            })
        };
        let today = 30;

        // A bootlicker.
        let (mut r, club, man, squad) = load();
        let mut random = Random::new(3);
        assert!(!bootlicker(&mut r, club, today, true, (&league, &mood), &mut Vec::new(), &mut Vec::new(), &mut random));
        assert!(random == Random::new(3));
        let p = r.player_mut(man).unwrap();
        p.set_shown(p.shown() | BOOTLICKER);
        let (mut recent, mut posted) = (Vec::new(), Vec::new());
        let five = one(&BOOTLICKER_HAPPENINGS, 5);
        assert!(bootlicker(&mut r, club, today, false, (&league, &mood), &mut recent, &mut posted, &mut Random::new(five)));
        assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text: 0x3bc, fill: Fill::NumberedOne(man) }]);
        assert_eq!(recent, [BOOTLICKER_KEY + 5]);
        told(&r, &posted[0], man);
        assert!(bootlicker(&mut r, club, today, false, (&league, &mood), &mut recent, &mut posted, &mut Random::new(five)));
        assert_eq!(posted.len(), 1);
        let before = r.team(club.0, club.1).unwrap().mood();
        let mut posted = Vec::new();
        assert!(bootlicker(&mut r, club, today, false, (&league, &mood), &mut recent, &mut posted, &mut Random::new(one(&BOOTLICKER_HAPPENINGS, 1))));
        assert_eq!(posted[0].text, 0x3b8);
        assert_eq!(i32::from(r.team(club.0, club.1).unwrap().mood()), (i32::from(before) - 10).max(0));
        let three = one(&BOOTLICKER_HAPPENINGS, 3);
        let mut posted = Vec::new();
        assert!(bootlicker(&mut r, club, today, false, (&league, &mood), &mut Vec::new(), &mut posted, &mut Random::new(three)));
        assert!(bootlicker(&mut r, club, today, true, (&league, &mood), &mut Vec::new(), &mut posted, &mut Random::new(three)));
        assert_eq!(posted.iter().map(|p| p.text).collect::<Vec<_>>(), [0x3ba]);
        // Its text asks for `%s`, which the original's filling leaves.
        assert!(told_of(&r, &texts, &words, &posted[0]).contains("Ihr Spieler %s kommt"));

        // An informer found out.
        let mut random = Random::new(3);
        assert!(!informer(&mut r, club, &mut Vec::new(), &mut random));
        assert!(random == Random::new(3));
        let p = r.player_mut(man).unwrap();
        p.set_shown(p.shown() | SPY);
        let mut posted = Vec::new();
        assert!(informer(&mut r, club, &mut posted, &mut Random::new(one(&INFORMER_HAPPENINGS, 1))));
        assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text: 0x3d7, fill: Fill::NumberedOne(man) }]);
        told(&r, &posted[0], man);
        let p = r.player(man).unwrap();
        assert_eq!((p.shown() & SHUT_OUT, p.contentment()[TEAM], p.contentment()[0]), (SHUT_OUT, 0, 90));
        assert_eq!(r.player(squad[0]).unwrap().contentment()[0], 90);
        // Or telling of a mole among his friends: posted, yet no news.
        let (mut r, club, man, squad) = load();
        let p = r.player_mut(man).unwrap();
        p.set_shown(p.shown() | SPY);
        p.set_friends([squad[0], 0, 0]);
        let mole = r.player_mut(squad[0]).unwrap();
        mole.set_shown(mole.shown() | MOLE);
        let two = seed(&|r| {
            r.below(1);
            weighed(&INFORMER_HAPPENINGS, r) == 2 && r.below(1) == 0
        });
        let mut posted = Vec::new();
        assert!(!informer(&mut r, club, &mut posted, &mut Random::new(two)));
        assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text: 0x3d6, fill: Fill::Numbered(man, squad[0]) }]);
        told(&r, &posted[0], man);
        assert!(told_of(&r, &texts, &words, &posted[0]).contains(&r.known_as(squad[0]).unwrap()));

        // Moles: none unhappy, no draw; an unhappy man turns.
        let (mut r, club, man, squad) = load();
        let mut random = Random::new(3);
        assert!(!moles(&mut r, club, &mut random));
        assert!(random == Random::new(3));
        let p = r.player_mut(man).unwrap();
        crate::mood::change(p, 0, -50);
        crate::mood::change(p, TEAM, -50);
        let mut random = Random::new(3);
        assert!(moles(&mut r, club, &mut random));
        let mut once = Random::new(3);
        once.below(1);
        assert!(random == once);
        assert_eq!(r.player(man).unwrap().shown() & MOLE, MOLE);
        // The press hears of the trainer once a round.
        let flags = |r: &Records| r.team(club.0, club.1).unwrap().training_flags();
        assert_ne!(r.team(club.0, club.1).unwrap().humans()[0], 0);
        let start = flags(&r);
        assert!(moles(&mut r, club, &mut Random::new(3)));
        assert_eq!(flags(&r), start | TOLD_TRAINER);
        // Of seven low with the trainer: told once, the round stepped.
        for &id in &squad[..LOW_MANY] {
            crate::mood::change(r.player_mut(id).unwrap(), 0, -20);
        }
        assert!(moles(&mut r, club, &mut Random::new(3)));
        let round = (start & PRESS_ROUND) >> PRESS_SHIFT;
        assert_eq!(flags(&r) & (TOLD_LOW | TOLD_TRAINER | PRESS_ROUND), TOLD_LOW | (round + 1).min(PRESS_MOST) << PRESS_SHIFT & PRESS_ROUND);
        // A mole pleased again stops; with no one left unhappy, nothing.
        let p = r.player_mut(man).unwrap();
        p.set_contentment([100; 7]);
        crate::mood::change(p, 0, 0);
        for &id in &squad {
            let p = r.player_mut(id).unwrap();
            p.set_contentment([100; 7]);
            crate::mood::change(p, 0, 0);
        }
        assert!(!moles(&mut r, club, &mut Random::new(3)));
        assert_eq!(r.player(man).unwrap().shown() & MOLE, 0);

        // A habit taken up and given up; smoking as the goal stands.
        let (mut r, club, _, squad) = load();
        let count = u32::from(r.team(club.0, club.1).unwrap().player_count());
        let habit = |n: u32| seed(&|r| r.below(count) == 3 && r.below(HABITS) == n);
        let mut posted = Vec::new();
        assert!(habits(&mut r, club, &league, &mut posted, &mut Random::new(habit(3))));
        assert!(posted.is_empty());
        assert!(habits(&mut r, club, &league, &mut posted, &mut Random::new(habit(2))));
        assert!(habits(&mut r, club, &league, &mut posted, &mut Random::new(habit(2))));
        assert_eq!(r.player(squad[3]).unwrap().shown() & OWN_TRAINER, OWN_TRAINER);
        assert!(habits(&mut r, club, &league, &mut posted, &mut Random::new(habit(3))));
        assert_eq!(r.player(squad[3]).unwrap().shown() & OWN_TRAINER, 0);
        let team = r.team(club.0, club.1).unwrap();
        let short = crate::league::goal_reached(&r, &league, team) < 1 && crate::league::goal_missing(&r, &league, team) > 0;
        assert!(habits(&mut r, club, &league, &mut posted, &mut Random::new(habit(if short { 0 } else { 1 }))));
        assert!(habits(&mut r, club, &league, &mut posted, &mut Random::new(habit(if short { 1 } else { 0 }))));
        let expect: &[u16] = if short { &[0x54e, 0x54f, 0x54c] } else { &[0x54e, 0x54f] };
        assert_eq!(posted.iter().map(|p| p.text).collect::<Vec<_>>(), expect);
        // Their texts speak of 5%: the man named.
        for p in &posted {
            assert!(told_of(&r, &texts, &words, p).starts_with(&format!("Ihr Spieler {}", r.known_as(squad[3]).unwrap())));
        }
    }

    fn told_of(r: &Records, texts: &a3_files::saq::EventTexts, words: &crate::words::Words, posted: &Posted) -> String {
        crate::event_text::make(r, texts, &|_| String::new(), &words.first_names, posted).text
    }

    /// The gates before the pick: three doping cases cost the club six
    /// points, ten of its image and the cases but one, told once; a
    /// report against its trainer is taken, two men tested, and without
    /// a positive sample the reporter's club pays 50 000 and the news ends.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn doping_penalty_and_reports_gate_the_news() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let mut a = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let (hc, hn) = crate::statistics::human_clubs(&a)[0];
        let club = (u16::from(hc), hn as u16);
        assert!(a.globals.half_weeks() < PENALTY_BEFORE);
        let team = a.team_mut(club.0, club.1).unwrap();
        team.put::<u8>(DOPING_CASES, 0x83);
        team.put::<u8>(crate::statistics::TEAM_FLAGS, 0);
        team.set_public_image(25);
        let mut posted = Vec::new();
        doping_penalty(&mut a, club, &mut posted);
        assert_eq!(posted, [Posted { club, code: 99, text: PENALTY_TEXT, fill: Fill::Club }]);
        let team = a.team(club.0, club.1).unwrap();
        assert_eq!(team.get::<u8>(DOPING_CASES), 0x81);
        assert_eq!(team.get::<u8>(crate::statistics::TEAM_FLAGS), 2);
        assert_eq!(team.public_image(), 15);
        doping_penalty(&mut a, club, &mut posted);
        assert_eq!(posted.len(), 1, "one case left, no more");

        // A second human at another club reported the trainer.
        let trainer = a.team(club.0, club.1).unwrap().humans()[0];
        let other: u8 = if trainer == 1 { 2 } else { 1 };
        let mut order = a.globals.order();
        order[..2].copy_from_slice(&[trainer as i16, other as i16]);
        a.globals.set_order(order);
        a.globals.set_humans(2);
        let far = (club.0, if club.1 == 1 { 2 } else { 1 });
        let m = &mut a.managers[usize::from(other) - 1];
        m.set_country(far.0);
        m.set_team(far.1);
        m.set_doping_reports(0);
        let costs = a.team(far.0, far.1).unwrap().get::<i32>(OTHER_COSTS);
        let mut reported = [[false; crate::records::MANAGERS]; crate::records::MANAGERS];
        reported[usize::from(other) - 1][usize::from(trainer) - 1] = true;
        let squad = a.team(club.0, club.1).unwrap().players();
        for &id in &squad {
            a.player_mut(id).unwrap().set_doping(0);
        }
        let (mut controls, mut posted) = (Vec::new(), Vec::new());
        let mut random = Random::from_state(20261121, 0);
        let mut expected = random.clone();
        let first = expected.below(squad.len() as u32);
        while expected.below(squad.len() as u32) == first {}
        assert!(doping_reports(&mut a, club, &mut reported, &mut controls, &mut posted, &mut random));
        assert!(random == expected, "two men drawn apart, no sample positive");
        assert!(controls.is_empty());
        assert!(!reported[usize::from(other) - 1][usize::from(trainer) - 1], "the report taken");
        assert_eq!(a.managers[usize::from(other) - 1].doping_reports(), 1);
        assert_eq!(a.team(far.0, far.1).unwrap().get::<i32>(OTHER_COSTS), costs + FALSE_REPORT);
        let fill = Fill::FalseReport(u16::from(trainer), FALSE_REPORT);
        assert_eq!(posted, [Posted { club: far, code: PRIVATE_NEWS, text: FALSE_REPORT_TEXT, fill }]);
        let texts = a3_files::saq::EventTexts::load(&dir).unwrap();
        let told = crate::event_text::make(&a, &texts, &|_| String::new(), &[], &posted[0]).text;
        let m = &a.managers[usize::from(trainer) - 1];
        assert!(told.contains(&format!("{} {}", m.first_name(), m.last_name())), "{told}");
        // Nobody reported: the news goes on, nothing drawn.
        let before = random.clone();
        assert!(!doping_reports(&mut a, club, &mut reported, &mut controls, &mut posted, &mut random));
        assert!(random == before);
    }
}
