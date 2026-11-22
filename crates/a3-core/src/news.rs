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
//! +0x8a and +0x8b; the handler of category 25, and what category 24's
//! tale (`far_from_home`) does to the man it draws and the text it is
//! told with; and three of the hobbies of category 19's hobby news
//! (`FUN_005fd59d`).
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
//! ([`habits`]); 18, a man's new hobby ([`new_hobby`]), 19, a man's
//! hobby's day ([`about_a_player`]), 20, the press on a man
//! ([`press_on_a_player`]), 21, a man against his trainer
//! ([`revolt`]), 22, a new man's first months ([`first_months`]), and
//! 23, a schoolboy's days ([`school_days`]), and 24, the men far from
//! home ([`far_from_home`]) — its walk, its lists and its drawn man, not
//! the tale told on him.

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

/// Category 18's text of a man's new hobby, and the hobby only a
/// midfielder takes up (`FUN_005fd4ae`).
const NEW_HOBBY_TEXT: u16 = 0x4d1;
const MIDFIELD_HOBBY: u8 = 0x25;
const MIDFIELD: u8 = 3;

/// The hobbies whose day `FUN_005fd59d` has built here (+0x28d): fishing,
/// the football manager game, the aquarium, stocks, stamps, books,
/// computer games, American football, looks, gambling, golf, the garden,
/// bees, art, travel, skiing, diving, food, motorbikes and sports cars.
const FISHING: u8 = 2;
const GAME: u8 = 3;
const AQUARIUM: u8 = 4;
const STOCKS: u8 = 6;
const STAMPS: u8 = 7;
const BOOKS: u8 = 8;
const COMPUTER_GAMES: u8 = 9;
const FOOTBALL: u8 = 0xb;
const LOOKS: u8 = 0xc;
const GAMBLING: u8 = 0x11;
const GOLF: u8 = 0x12;
const GARDEN: u8 = 0xf;
const BEES: u8 = 0x14;
const ART: u8 = 0x18;
const TRAVEL: u8 = 0x1f;
const SKIING: u8 = 0x22;
const DIVING: u8 = 0x23;
const FOOD: u8 = 0x13;
const MOTORBIKES: u8 = 0x1c;
const SPORTS_CARS: u8 = 0x26;
/// Food tells only of a man whose condition as a list shows it
/// (`FUN_00457e00`) is under this; the bit of +0x1b4 that makes him
/// overweight (`FUN_006fff76`); the offence a gluttony earns him
/// (`FUN_00702d95(1)`).
const FOOD_UNDER: i32 = 0x3c;
const OVERWEIGHT: u32 = 0x20;
const GLUTTONY_OFFENCE: u8 = 1;
/// Motorbikes and sports cars tell of a man older than this
/// (`FUN_006ff01b`); the months without his licence a motorbike and a
/// sports car cost him (+0x28e, `FUN_00702d31`), and what they take off
/// his contentment with the press.
const RIDER_OVER: i32 = 0x11;
const BIKE_BAN: u8 = 2;
const CAR_BAN: u8 = 1;
const BIKE_PRESS: i32 = -5;
const CAR_PRESS: i32 = -3;
const WIFE_PRESS: i32 = -2;
/// The days fishing knocks him about (`FUN_00766af9(5)`), what the fresh
/// fish lift the team's mood by and what a night's game takes off his
/// freshness.
const FISHING_KNOCK: u16 = 5;
const FRESH_FISH: i32 = 10;
const SLEEPLESS: i32 = -2;
/// The injuries of the hobbies (`FUN_00766b92`): the kind and the days,
/// those given none the kind's days of the table at `DAT_008ac341`.
const SPRAINED_FOOT: (u8, i32) = (0xd, 7);
const SPADE: (u8, i32) = (0x25, 112);
const MALARIA: (u8, i32) = (0x52, 42);
const BROKEN_LEG: (u8, i32) = (0x2a, 42);
const JELLYFISH: (u8, i32) = (0x5b, 14);
const NITROGEN: (u8, i32) = (0x5c, 0);
const STOMACH: (u8, i32) = (0x54, 2);
const BIKE_FALL: (u8, i32) = (0x5a, 7);
const BIKE_CRASH: (u8, i32) = (0x31, 21);
const CAR_CRASH: (u8, i32) = (0x5d, 0);
/// A data set pleases a man less content with his distractions than this;
/// stocks break a man younger than this; a dear stamp troubles a salary
/// under this (the handle's +0x88); travel news comes before this day;
/// skiing pleases a man less pleased with his private life than this.
const DATA_SET_UNDER: i32 = 0x62;
const BROKE_UNDER: i32 = 25;
const DEAR_STAMP_SALARY: u16 = 500;
const TRAVEL_BEFORE: i32 = 3;
const SKIING_UNDER: i32 = 0x50;
/// A diary tells from after this day (`DAT_00b5cbd4` > 0xbd): the team's
/// mood twenty down and an offence of this level noted on him.
const DIARY_AFTER: i32 = 0xbd;
const DIARY_MOOD: i32 = -20;
const DIARY_OFFENCE: u8 = 2;
/// A man older than this writes his life's story (`FUN_006ff01b`) unless
/// the bit 0x40 of +0x1b4 bars him (`FUN_00702cdf`; only `FUN_00702cff`
/// sets it, and nothing calls that).
const BOOK_OVER: i32 = 0x1e;
const BOOK_BARRED: u32 = 0x40;
/// A German man with a child on the way names it: the marks of the names
/// at +0x1b4 (`FUN_007023db`, `FUN_00702410`), either of which counts as
/// named (`FUN_007022d7`, `FUN_00702359`), and the first names a child
/// of his may already have taken (the names at +0x288): "Lara", "Rolf".
const NAMED_LARA: u32 = 0x1000_0000;
const NAMED_ROLF: u32 = 0x800_0000;
const NAMED: u32 = NAMED_LARA | NAMED_ROLF;
const LARA: u8 = b'O';
const ROLF: u8 = b')';
/// The days the sunbed and the tattoo knock him about (`FUN_00766af9`);
/// the sunbed burns only the skin of this kind (+0x21a's low three bits,
/// `FUN_00707725`).
const SUNBURN_KNOCK: u16 = 5;
const TATTOO_KNOCK: u16 = 2;
const FAIR_SKIN: u8 = 0;
/// The press part of his contentment (+0x292, `FUN_00700fa2`) and what a
/// new woman seen with him takes off it.
const PRESS: usize = 3;
const NEW_WOMAN: i32 = -2;
/// The nose put right only for a man without a partner, after more than
/// ten matchdays, his club more than two places short of its season's
/// goal and its two bytes at +0x11e4 (`FUN_0069947b`: the human club's
/// extra 0x2568 under 0x2569; meaning not traced) rising.
const NOSE_AFTER: usize = 10;
const NOSE_SHORT: i32 = 2;
const RISING: usize = 0x2568;
/// What gambling moves his distractions by: the casino, roulette or
/// poker, and giving it up.
const CASINO: i32 = -10;
const ROULETTE: i32 = -5;
const GIVEN_UP: i32 = 6;

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

/// The picked category 18 (`case 0x12` with `FUN_005fd4ae`): one man is
/// drawn over the club's squad (+0xe2, the team record's `player_count`,
/// over its list); a player-manager (his id above 17 000) or a man with
/// a hobby (+0x28d, `FUN_00702c9e`) has nothing. Else he is drawn one of
/// the weighted hobbies (`FUN_0060be4d` on `DAT_00a325c8`) — hobby 0x25
/// only for a midfielder (the squad entry's +3, his main position at
/// +0x21d, 3) — takes it (`FUN_00702cbd`) and the club is told (0x4d1,
/// `sprintf` with his name and the hobby's game text, `FUN_007335e0`;
/// code 0x60 through `FUN_005f1eaf`). The news ends all the same.
pub fn new_hobby(
    records: &mut Records,
    club: (u16, u16),
    tables: &crate::first_season::Tables,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) {
    let Some(id) = drawn_man(records, club, random) else { return };
    let Some(p) = records.player_mut(id) else { return };
    if id > LAST_REAL_ID || p.hobby() != 0 {
        return;
    }
    let hobby = tables.hobby(random);
    if hobby == MIDFIELD_HOBBY && p.positions()[0] != MIDFIELD {
        return;
    }
    p.set_hobby(hobby);
    posted.push(Posted { club, code: PRIVATE_NEWS, text: NEW_HOBBY_TEXT, fill: Fill::NamedGame(id, tables.hobby_text(hobby)) });
}

/// The picked category 19 (`case 0x13` with `FUN_005fed08`): one man is
/// drawn over the club's squad, as for category 18; a player-manager or
/// a man without a hobby has nothing (the low half of its return 0), and
/// the day's news goes on to its later rolls. A man with one meets his
/// hobby's day ([`hobby_day`]), which returns 1 in its low half on every
/// way out, so the news took place whatever it told. `planned` whether
/// the club's plan holds anything but free today. Whether the news took
/// place.
pub fn about_a_player(
    records: &mut Records,
    club: (u16, u16),
    today: i32,
    planned: bool,
    league: &crate::league::LeagueTables,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    let Some(id) = drawn_man(records, club, random) else { return false };
    if id > LAST_REAL_ID || records.player(id).is_none_or(|p| p.hobby() == 0) {
        return false;
    }
    hobby_day(records, club, id, today, planned, league, posted, random);
    true
}

/// A man's hobby's day (`FUN_005fd59d`, "EreignisHobby %d"), by his hobby
/// (+0x28d, `FUN_00702c9e`); "out" is the handle's days out (+0x2c).
/// Fishing, one of three: knocked about five days by a bladder infection
/// (0x482) or a poisoned fish (0x484), or fresh fish for the team, its
/// mood up ten (0x483). The football manager game: two less fresh after a
/// night's season (0x485), a sprained foot (0x486), or a data set found,
/// three more content with his distractions while under 98 (0x487). The
/// aquarium, one of two: three less (0x489), or for a married man the
/// tank broken, five less (0x48a). Stocks, one of three: one time in ten
/// a man under 25 of strength 2 or more broke, one point of strength off
/// (0x48b); rich, his hobby stamps from now on (0x48c); or the shares
/// gone, fifteen less (0x48d). Stamps, one of two: a dear stamp bought on
/// a salary under 500, ten less (0x48e), or the collection stolen,
/// fifteen less (0x48f). Books, one of two: a diary after day 189, the
/// team's mood twenty down and an offence of level 2 noted (0x490, told
/// with values); or, for a man over 30, one of three — doping confessed
/// (0x491), a harmless life's story (0x493), or two others of the squad
/// drawn ([`other_man`]) who fall out with each other, told with both
/// names (0x494; the handbook 0x495 asks for a fourth draw of three).
/// Computer games, one of two (the original's 0x49a and 0x49b ask for a
/// third and fourth): a German man with a child on the way, not named
/// yet, names it "Lara" if a girl (0x496, 0x497 with children) or is
/// talked out of "Kurt" for "Rolf" (0x498, 0x499). Looks, one of six: a
/// sunburn for fair skin, knocked about five days (0x49e); a tattoo, two
/// days (0x49f); new hair (0x4a0); a new woman, two less content with the
/// press (0x4a1, told with values); a nose put right ([`NOSE_AFTER`],
/// 0x4a3). Gambling, one of four: the casino, ten less (0x4ac); roulette
/// or poker, five less (0x4ae, 0x4af); or given up, his hobby gone, six
/// more (0x4b0) — all told with values. Golf only makes a headline. The
/// headlines (0x492, 0x4a2, 0x4a4, 0x4ad, 0x4b1; `FUN_00789c32`) are not
/// posted here. American football, one of two: up late (0x49c,
/// nothing taken), or not out, a bruised rib, knocked about seven days
/// (0x49d). The garden, one of three, not out: a spade in his foot
/// (0x4a9) or knocked about a fortnight by hornets (0x4aa); or a
/// bootlicker names a rose after his manager (0x4ab). The bees: the
/// hobby forbidden (0x4ba, the wife's name left empty). Art, one of four:
/// five less (0x4bb), or five and a draw of six less (0x4bc to 0x4be).
/// Travel, before the third day: malaria (0x4c3), or seven days away
/// (0x4c4, 0x4c5). Skiing, not out and from December to March, on a day
/// without training: a few hours, three more pleased with his private
/// life while under 80 (0x4c6), or a broken leg (0x4c7). Diving: a
/// jellyfish (0x4c8) or nitrogen (0x4c9). The changes of his
/// contentment are told with values (`FUN_00601043`) and not posted
/// here; the others are told by his name, `%2` empty (`FUN_00600f2f`).
/// Food, for a man whose condition shows under 60: from 28 December to
/// 13 January overweight (0x4b4); else one of six — overweight (0x4b2,
/// 0x4b6, 0x4b7), slim again (0x4b3), overweight with an offence of
/// level 1 and the manager asked over a fine (0x4b8, `FUN_005f29bc`'s
/// question, not built: nothing posted), or a stomach upset, two days
/// (0x4b9). Motorbikes, for a man over 17, one of six: a fall (0x4bf) or
/// a crash (0x4c0), out the kind's days; or, while he has his licence,
/// two months without it and five less content with the press (0x4c1,
/// told with values; headline 0x4c2). Sports cars, one of six: while he
/// has his licence and is not out, a crash (0x4cb); for a married man
/// two less content with the press (0x4cc, 0x4ce, told with values;
/// headlines 0x4cd, 0x4cf); for one over 17 with his licence and not
/// out, a month without it and three less (0x4d0; headline 0x4d2); else
/// at most a headline (0x4d3), nothing taken.
/// The other hobby of the original (0x20) is not built: it tells
/// nothing and draws nothing here.
#[allow(clippy::too_many_arguments)]
fn hobby_day(
    records: &mut Records,
    club: (u16, u16),
    id: u16,
    today: i32,
    planned: bool,
    league: &crate::league::LeagueTables,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) {
    let Some(p) = records.player(id) else { return };
    let out = crate::load::days_to(&records.globals, p.injured_to()) != 0;
    let parts = p.contentment();
    let married = p.partner() & 3 == WIFE;
    let month = i32::from(records.globals.date()[1] as i16);
    let distract = |records: &mut Records, by: i32| change(records, id, DISTRACTION, by);
    let knock = |records: &mut Records, days: u16| crate::training_day::knock_for(records, id, days, today);
    let injure = |records: &mut Records, (kind, days): (u8, i32)| crate::training_day::injure(records, id, kind, days, today);
    let text = match p.hobby() {
        FISHING => match random.below(3) {
            0 => {
                knock(records, FISHING_KNOCK);
                0x482
            }
            1 => {
                team_mood(records, club, FRESH_FISH);
                0x483
            }
            _ => {
                knock(records, FISHING_KNOCK);
                0x484
            }
        },
        GAME => match random.below(3) {
            0 => {
                tire(records, id, SLEEPLESS);
                0x485
            }
            1 => {
                injure(records, SPRAINED_FOOT);
                0x486
            }
            _ => {
                if i32::from(parts[DISTRACTION]) < DATA_SET_UNDER {
                    distract(records, 3);
                }
                return;
            }
        },
        AQUARIUM => {
            if random.below(2) == 0 {
                distract(records, -3);
            } else if married {
                distract(records, -5);
            }
            return;
        }
        STOCKS => match random.below(3) {
            0 => {
                let broke = random.below(10) == 0;
                let Some(p) = records.player(id) else { return };
                let age = years_since(records, (p.birth_day(), p.birth_month(), p.birth_year() as i16));
                if !(broke && age < BROKE_UNDER && p.strength() > 1) {
                    return;
                }
                let Some(p) = records.player_mut(id) else { return };
                p.set_strength(p.strength() - 1);
                0x48b
            }
            1 => {
                if let Some(p) = records.player_mut(id) {
                    p.set_hobby(STAMPS);
                }
                0x48c
            }
            _ => {
                distract(records, -15);
                return;
            }
        },
        STAMPS => {
            if random.below(2) != 0 {
                distract(records, -15);
            } else if records.player(id).is_some_and(|p| p.salary() < DEAR_STAMP_SALARY) {
                distract(records, -10);
            }
            return;
        }
        BOOKS => {
            if random.below(2) == 0 {
                if today > DIARY_AFTER {
                    team_mood(records, club, DIARY_MOOD);
                    if let Some(p) = records.player_mut(id) {
                        crate::discipline::note_offence(p, DIARY_OFFENCE);
                    }
                }
                return;
            }
            let age = years_since(records, (p.birth_day(), p.birth_month(), p.birth_year() as i16));
            if age <= BOOK_OVER || p.shown() & BOOK_BARRED != 0 {
                return;
            }
            match random.below(3) {
                0 => 0x491,
                1 => 0x493,
                _ => {
                    let first = other_man(records, club, id, random);
                    let second = other_man(records, club, id, random);
                    let (Some(first), Some(second)) = (first, second) else { return };
                    if first == second {
                        return;
                    }
                    crate::training_day::fall_out(records, first, second);
                    posted.push(Posted { club, code: PRIVATE_NEWS, text: 0x494, fill: Fill::NumberedThree(id, first, second) });
                    return;
                }
            }
        }
        COMPUTER_GAMES => {
            let (mark, name, texts) = if random.below(2) == 0 { (NAMED_LARA, LARA, [0x496, 0x497]) } else { (NAMED_ROLF, ROLF, [0x498, 0x499]) };
            let children = p.children();
            let named = p.shown() & NAMED != 0 || p.children_names().iter().take(children.max(0) as usize).any(|&n| n == name);
            if p.nation() != GERMANY || p.shown() & EXPECTING == 0 || named {
                return;
            }
            let Some(p) = records.player_mut(id) else { return };
            p.set_shown(p.shown() | mark);
            texts[usize::from(children != 0)]
        }
        LOOKS => match random.below(6) {
            0 if p.face()[0] & 7 == FAIR_SKIN => {
                knock(records, SUNBURN_KNOCK);
                0x49e
            }
            1 => {
                knock(records, TATTOO_KNOCK);
                0x49f
            }
            2 => 0x4a0,
            3 => {
                change(records, id, PRESS, NEW_WOMAN);
                return;
            }
            4 if p.partner() & 3 == 0 && nose_due(records, club, league) => 0x4a3,
            _ => return,
        },
        GAMBLING => {
            match random.below(4) {
                0 => distract(records, CASINO),
                3 => {
                    if let Some(p) = records.player_mut(id) {
                        p.set_hobby(0);
                    }
                    distract(records, GIVEN_UP);
                }
                _ => distract(records, ROULETTE),
            }
            return;
        }
        GOLF => return,
        FOOTBALL => match random.below(2) {
            0 => 0x49c,
            _ if !out => {
                knock(records, 7);
                0x49d
            }
            _ => return,
        },
        GARDEN => match random.below(3) {
            0 if !out => {
                injure(records, SPADE);
                0x4a9
            }
            1 if !out => {
                knock(records, 14);
                0x4aa
            }
            2 if records.player(id).is_some_and(|p| p.shown() & BOOTLICKER != 0) => 0x4ab,
            _ => return,
        },
        BEES => {
            if let Some(p) = records.player_mut(id) {
                p.set_hobby(0);
            }
            0x4ba
        }
        ART => {
            let by = if random.below(4) == 0 { -5 } else { -5 - random.below(6) as i32 };
            distract(records, by);
            return;
        }
        TRAVEL if today < TRAVEL_BEFORE => match random.below(3) {
            0 => {
                injure(records, MALARIA);
                0x4c3
            }
            k => {
                send_away(records, id, 7, 4);
                0x4c3 + k as u16
            }
        },
        SKIING if !out && !(4..=11).contains(&month) => {
            if planned {
                return;
            }
            if random.below(2) != 0 {
                injure(records, BROKEN_LEG);
                0x4c7
            } else {
                if i32::from(parts[PRIVATE]) < SKIING_UNDER {
                    change(records, id, PRIVATE, 3);
                }
                return;
            }
        }
        DIVING => {
            if random.below(2) == 0 {
                injure(records, JELLYFISH);
                0x4c8
            } else {
                injure(records, NITROGEN);
                0x4c9
            }
        }
        FOOD => {
            if shown_condition(p) >= FOOD_UNDER {
                return;
            }
            let [day, month, _] = records.globals.date().map(|w| w as i16);
            let christmas = (month == 12 && day >= 28) || (month == 1 && day <= 13);
            let fat = |records: &mut Records, on: bool| {
                if let Some(p) = records.player_mut(id) {
                    p.set_shown(if on { p.shown() | OVERWEIGHT } else { p.shown() & !OVERWEIGHT });
                }
            };
            if christmas {
                fat(records, true);
                0x4b4
            } else {
                match random.below(6) {
                    0 => {
                        fat(records, true);
                        0x4b2
                    }
                    1 => {
                        fat(records, false);
                        0x4b3
                    }
                    k @ (2 | 3) => {
                        fat(records, true);
                        0x4b4 + k as u16
                    }
                    4 => {
                        fat(records, true);
                        if let Some(p) = records.player_mut(id) {
                            crate::discipline::note_offence(p, GLUTTONY_OFFENCE);
                        }
                        return;
                    }
                    _ => {
                        injure(records, STOMACH);
                        0x4b9
                    }
                }
            }
        }
        MOTORBIKES => {
            let age = years_since(records, (p.birth_day(), p.birth_month(), p.birth_year() as i16));
            let licensed = p.get::<u8>(NO_LICENCE) == 0;
            if age <= RIDER_OVER {
                return;
            }
            match random.below(6) {
                0 => {
                    injure(records, BIKE_FALL);
                    0x4bf
                }
                1 => {
                    injure(records, BIKE_CRASH);
                    0x4c0
                }
                2 if licensed => {
                    if let Some(p) = records.player_mut(id) {
                        p.put::<u8>(NO_LICENCE, BIKE_BAN);
                    }
                    change(records, id, PRESS, BIKE_PRESS);
                    return;
                }
                _ => return,
            }
        }
        SPORTS_CARS => {
            let age = years_since(records, (p.birth_day(), p.birth_month(), p.birth_year() as i16));
            let licensed = p.get::<u8>(NO_LICENCE) == 0;
            match random.below(6) {
                0 if licensed && !out => {
                    injure(records, CAR_CRASH);
                    0x4cb
                }
                1 | 2 if married => {
                    change(records, id, PRESS, WIFE_PRESS);
                    return;
                }
                3 if licensed && age > RIDER_OVER && !out => {
                    if let Some(p) = records.player_mut(id) {
                        p.put::<u8>(NO_LICENCE, CAR_BAN);
                    }
                    change(records, id, PRESS, CAR_PRESS);
                    return;
                }
                _ => return,
            }
        }
        _ => return,
    };
    posted.push(Posted { club, code: PRIVATE_NEWS, text, fill: Fill::NumberedOne(id) });
}

/// Category 20's press on a man (`FUN_005e56a5`): his match bits (+0x25b,
/// the handle's +0x57) that he stood the last match, his grade in it
/// twice over (+0x273's low four bits, the handle's +0xb8,
/// `FUN_00705e9b`) of a flop at least, his season's grades (`FUN_00705eb6`)
/// above, his league matches (+0x253, the handle's +0x54) above.
const STOOD_THE_MATCH: u8 = 1;
const FLOP_GRADE: u8 = 10;
const FLOP_SEASON: i32 = 40;
const FLOP_MATCHES: u8 = 10;
const FLOP_PRESS: i32 = -6;
/// The sensitive man (the second trait word's 0x400, the handle's +0x12)
/// under this morale (+0x296, the handle's +0xca) and of this grade.
const SENSITIVE: u16 = 0x400;
const SENSITIVE_MORALE: u8 = 80;
const SENSITIVE_GRADE: u8 = 8;
/// The man in his contract's last year (the handle's +0x94) above this
/// strength.
const ABROAD_STRENGTH: u8 = 9;
/// The millionaire under this content with his private life (+0x293,
/// `FUN_007011f1`), above this strength and age (`FUN_006ff01b`).
const MILLIONAIRE_PRIVATE: u8 = 90;
const MILLIONAIRE_STRENGTH: u8 = 8;
const MILLIONAIRE_AGE: i32 = 24;
const PRESS_KNOCK: i32 = -8;
const MILLIONAIRE_PRESS: i32 = -10;

/// The picked category 20 (`case 0x14` with `FUN_005e56a5`,
/// "EreignisProbPresse"): one man drawn over the club's squad, as for
/// category 18; a player-manager ends the day's news untold. The press
/// takes up the first that fits him, his contentment with the press
/// (`FUN_00700fa2`) down: a flop who stood the last match with a grade of
/// 5 or worse, his season's grades above 40, more than ten league
/// matches, his club short of its season's goal (`FUN_00775f0a`), six
/// down (headline 0x5ef, his name and the club's manager's); a sensitive
/// man under 80 of morale with a grade of 4 or worse, eight (0x5f1); a
/// man in his contract's last year above strength 9, off abroad, eight
/// (0x5f2); a millionaire under 90 content with his private life, above
/// strength 8 and older than 24, ten (0x5f4). The others never come:
/// the flop's season's grades come to 35 at most, the man of the crisis
/// (0x5f0) asks for more than three of the manager's last three matches
/// (`FUN_005ee8c6` counts three at most), the national call (0x5f3)
/// stands behind a constant false. The
/// headlines (`FUN_00789c32`) are not posted here. With none of them the
/// news goes on.
pub fn press_on_a_player(
    records: &mut Records,
    club: (u16, u16),
    league: &crate::league::LeagueTables,
    random: &mut Random,
) -> bool {
    let Some(id) = drawn_man(records, club, random) else { return false };
    if id > LAST_REAL_ID {
        return true;
    }
    let (Some(team), Some(p)) = (records.team(club.0, club.1), records.player(id)) else { return false };
    let missing = crate::league::goal_missing(records, league, team);
    let grade = p.grade() & 0xf;
    let age = years_since(records, (p.birth_day(), p.birth_month(), p.birth_year() as i16));
    let by = if p.match_bits() & STOOD_THE_MATCH != 0
        && grade >= FLOP_GRADE
        && season_grades(p) > FLOP_SEASON
        && p.league_matches() > FLOP_MATCHES
        && missing > 0
    {
        FLOP_PRESS
    } else if p.morale() < SENSITIVE_MORALE && p.traits()[1] & SENSITIVE != 0 && grade >= SENSITIVE_GRADE {
        PRESS_KNOCK
    } else if crate::transfer_week::years_left(records, p) == 1 && p.strength() > ABROAD_STRENGTH {
        PRESS_KNOCK
    } else if p.contentment()[PRIVATE] < MILLIONAIRE_PRIVATE && p.strength() > MILLIONAIRE_STRENGTH && age > MILLIONAIRE_AGE {
        MILLIONAIRE_PRESS
    } else {
        return false;
    };
    change(records, id, PRESS, by);
    true
}

/// His season's grades (`FUN_00705eb6`): over the matchdays he played
/// (+0x319, bit 0x80), five times the mean of the grade in bits 2 to 4;
/// none played, nought.
fn season_grades(p: &crate::records::Player) -> i32 {
    let played: Vec<i32> = p.matchdays().iter().filter(|&&d| d & 0x80 != 0).map(|&d| i32::from(d & 0x1c) >> 2).collect();
    if played.is_empty() {
        return 0;
    }
    played.iter().sum::<i32>() * 10 / played.len() as i32 / 2
}

/// Category 21's happenings (`FUN_0060be4d` over `DAT_00a2eb5c`,
/// registered by `FUN_005e59e6` from `DAT_0088d8f8` at a cap of 10 000):
/// six weighed 10, so the rest of the roll is none; one met lately is
/// not met again (26 500 and its number).
const REVOLTS: [(u32, u16); 6] = [(10, 1), (10, 2), (10, 3), (10, 4), (10, 5), (10, 6)];
const REVOLTS_CAP: u32 = 10_000;
const REVOLT_MET: i32 = 0x6784;
/// The second trait word's bits that keep a man out of it.
const LOYAL: u16 = 0x120;
/// Under this content with the trainer (`FUN_007008b9`) a man rebels,
/// under the other he kicks; under this the board's confidence
/// (`FUN_007701b2`).
const REBEL_CONTENT: i32 = 0x50;
const KICK_CONTENT: i32 = 0x5a;
const REBEL_CONFIDENCE: i32 = 0x50;
/// The conditioning units of a plan (`FUN_005ea83f` asks for each).
const CONDITIONING: [u8; 3] = [0x11, 0x10, 6];
/// More matchdays than the first and fewer than the second, and the
/// condition as a list shows it under this.
const UNFIT_ROUNDS: (usize, usize) = (5, 15);
const UNFIT_CONDITION: i32 = 0x32;
/// The trainer's grievances of his flags (+0x1b0, the handle's +0x20).
const GRIEVED: u32 = 0x5000;
/// The offence a foul on the trainer notes (`FUN_00702d95(1)`).
const FOUL_OFFENCE: u8 = 1;

/// The picked category 21 (`case 0x15` with `FUN_005e5de6`): nothing for
/// a club without its trainer's number (+0x8a); else one man drawn over
/// the squad, and a real one without the loyal traits (0x120) has his
/// day ([`revolt_day`]). Otherwise the news goes on.
/// `units` are today's three sessions of the club's plan.
pub fn revolt(
    records: &mut Records,
    club: (u16, u16),
    today: i32,
    units: [u8; 3],
    league: &crate::league::LeagueTables,
    (recent, posted): (&mut Vec<i32>, &mut Vec<Posted>),
    random: &mut Random,
) -> bool {
    let trainer = records.team(club.0, club.1).map_or(0, |t| t.humans()[0]);
    if trainer == 0 {
        return false;
    }
    let Some(id) = drawn_man(records, club, random) else { return false };
    if id > LAST_REAL_ID || records.player(id).is_none_or(|p| p.traits()[1] & LOYAL != 0) {
        return false;
    }
    revolt_day(records, club, id, usize::from(trainer - 1), today, units, league, recent, posted, random)
}

/// A man against his trainer (`FUN_005e5a01`, "EreignisAufstand %d"):
/// one of six happenings, none in most rolls, which goes on with the
/// news; one met lately ends it untold. Every other ends it, told or not.
/// 1: under 80 content with the trainer, the manager without the
/// majority of his club's shares (`FUN_00406b4f`) and a plan for today
/// (`FUN_005ea8cc`), the man refuses him — his contentment with the
/// trainer (+0x28f, `FUN_00700846`) to nought, told with values beside
/// the text (0x5f5, `FUN_00601043`), which the replica does not post.
/// 2: under 90, a kick from behind in a practice match (0x5f6, whose
/// `%s` the numbered fill leaves standing). 3: under 80, a conditioning
/// unit today and the match tomorrow (the club's +0xb44), the club short
/// of its season's goal and the board's confidence under 80: he
/// complains to a reporter (0x5f7; headline 0x5f8). 4: under 80, not
/// all three conditioning units today and no match tomorrow, the human
/// club's two bytes rising (`FUN_0069947b`), past the fifth matchday and
/// before the fifteenth, not out unvisited (+0x270's low six bits,
/// `FUN_007061a0`), a condition under 50 and the board's confidence
/// under 80: he finds the team unfit (0x5f9; headline 0x5fa). 5: one with
/// the trainer's grievances 0x1000 or 0x4000 elbows him (0x5fb). 6: one
/// with them knees him, an offence of level 1 noted, and the manager is
/// asked what he does (0x5fc, `FUN_005f29bc`'s question, not built:
/// nothing posted). The posts take his name and, as `%2`, the manager's
/// last name (the manager's +0xa), which neither text asks for.
#[allow(clippy::too_many_arguments)]
fn revolt_day(
    records: &mut Records,
    club: (u16, u16),
    id: u16,
    manager: usize,
    today: i32,
    units: [u8; 3],
    league: &crate::league::LeagueTables,
    recent: &mut Vec<i32>,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    let happening = weighed_within(&REVOLTS, REVOLTS_CAP, random);
    if happening == 0 {
        return false;
    }
    if !meet(recent, REVOLT_MET + i32::from(happening)) {
        return true;
    }
    let (Some(team), Some(p)) = (records.team(club.0, club.1), records.player(id)) else { return true };
    let content = crate::mood::parts(p)[0];
    let confidence = crate::stock::confidence(records, manager);
    let conditioning = CONDITIONING.map(|unit| units.contains(&unit));
    let next = i32::from(team.next_day());
    let text = match happening {
        1 => {
            let planned = units.iter().any(|&u| u != crate::training::FREE);
            if content < REBEL_CONTENT && !crate::stock::majority(records, manager) && planned {
                if let Some(p) = records.player_mut(id) {
                    let mut parts = p.contentment();
                    parts[0] = 0;
                    p.set_contentment(parts);
                    p.set_morale(crate::mood::morale(p));
                }
            }
            return true;
        }
        2 if content < KICK_CONTENT => 0x5f6,
        3 if content < REBEL_CONTENT
            && conditioning.contains(&true)
            && next == today + 1
            && crate::league::goal_missing(records, league, team) > 0
            && confidence < REBEL_CONFIDENCE =>
        {
            0x5f7
        }
        4 if content < REBEL_CONTENT
            && conditioning.contains(&false)
            && next > today + 1
            && rising(records, team)
            && (UNFIT_ROUNDS.0 + 1..UNFIT_ROUNDS.1).contains(&crate::league::matchdays(records, team))
            && p.visits() & 0x3f == 0
            && shown_condition(p) < UNFIT_CONDITION
            && confidence < REBEL_CONFIDENCE =>
        {
            0x5f9
        }
        5 if p.flags() & GRIEVED != 0 => 0x5fb,
        6 if p.flags() & GRIEVED != 0 => {
            if let Some(p) = records.player_mut(id) {
                crate::discipline::note_offence(p, FOUL_OFFENCE);
            }
            return true;
        }
        _ => return true,
    };
    posted.push(Posted { club, code: PRIVATE_NEWS, text, fill: Fill::NumberedOne(id) });
    true
}

/// The team's mood moved by `by` within 0 to 200 (`FUN_00698ef6` on the
/// club's +0x9e, the team's +0xc4), the squad's reasons of mood left as
/// they stand.
fn team_mood(records: &mut Records, club: (u16, u16), by: i32) {
    if let Some(team) = records.team_mut(club.0, club.1) {
        team.set_mood((i32::from(team.mood()) + by).clamp(0, 200) as u8);
    }
}

/// His freshness moved by `by` points (`FUN_005e5f40` on the handle's
/// +0x18, a hundredth a point), within 0 and 10 000.
fn tire(records: &mut Records, id: u16, by: i32) {
    if let Some(p) = records.player_mut(id) {
        let mut fresh = p.freshness();
        fresh[0] = (i32::from(fresh[0]) + by * 100).clamp(0, 10_000) as i16;
        p.set_freshness(fresh);
    }
}

/// Whether a man of the club may have his nose put right (`case 4` of
/// the looks): more than ten matchdays played in its league
/// (`FUN_00692744`), more than two places short of its season's goal
/// (`FUN_00775f0a`, [`crate::league::goal_missing`]) and the human club's
/// two bytes at +0x11e4 rising (`FUN_0069947b`).
fn nose_due(records: &Records, club: (u16, u16), league: &crate::league::LeagueTables) -> bool {
    let Some(team) = records.team(club.0, club.1) else { return false };
    crate::league::matchdays(records, team) > NOSE_AFTER
        && crate::league::goal_missing(records, league, team) > NOSE_SHORT
        && rising(records, team)
}

/// The human club's two bytes at +0x11e4 rising (`FUN_0069947b`).
fn rising(records: &Records, team: &crate::records::Team) -> bool {
    usize::from(team.human())
        .checked_sub(1)
        .and_then(|s| records.extras.get(s))
        .is_some_and(|e| e.get::<u8>(RISING) < e.get::<u8>(RISING + 1))
}

/// His condition as a list shows it (`FUN_00457e00`): a smoker's five
/// less, never under nought.
fn shown_condition(p: &crate::records::Player) -> i32 {
    let shown = i32::from(p.condition()) / 100;
    if p.shown() & SMOKES == 0 { shown } else if shown < 5 { 0 } else { shown - 5 }
}

/// Another man of the club's squad (`FUN_005ec9d8`): its real players but
/// him and those who hate him (+0x206), 0x23 at most, one drawn; none
/// and nothing drawn when there is no such man.
fn other_man(records: &Records, club: (u16, u16), id: u16, random: &mut Random) -> Option<u16> {
    let squad = records.team(club.0, club.1).map(|t| t.players()).unwrap_or_default();
    let such: Vec<u16> = squad
        .into_iter()
        .filter(|&o| o <= LAST_REAL_ID && o != id && records.player(o).is_some_and(|p| p.hates() != id))
        .take(PICKS_MOST)
        .collect();
    if such.is_empty() {
        return None;
    }
    Some(such[random.below(such.len() as u32) as usize])
}

/// One man drawn over the club's squad (`FUN_0078a810(+0xe2)` into its
/// list at +0xb50).
fn drawn_man(records: &Records, club: (u16, u16), random: &mut Random) -> Option<u16> {
    let team = records.team(club.0, club.1)?;
    let at = random.below(u32::from(team.player_count())) as usize;
    team.players().get(at).copied()
}

/// The two happenings of a new man's first months, weighed alike
/// (`FUN_0060be4d` over the list at `DAT_00a32658`, registered by
/// `FUN_005f9ebf` from `DAT_0088e6f0`); the second is never told.
const FIRST_MONTHS: [(u32, u16); 2] = [(1, 1), (1, 2)];
/// The five happenings of a schoolboy's day, weighed alike
/// (`FUN_0060be4d` over `DAT_00a32664`, registered by `FUN_005f9b78`
/// from `DAT_0088e6dc`).
const SCHOOL_DAYS: [(u32, u16); 5] = [(1, 1), (1, 2), (1, 3), (1, 4), (1, 5)];
/// The season from which a new man's first months are talked about
/// (`FUN_005f9fd0` with `DAT_00b5cbc8`), and the months under which he
/// still counts as new (`FUN_006ff5d5`).
const FIRST_MONTHS_SEASON: u16 = 3;
const NEW_AT_THE_CLUB: i32 = 4;
/// The men each of the two categories gathers at most.
const MEN_MOST: usize = 0x14;
/// A schoolboy is this old in the season's start year, 18 with a
/// birthday before August and 19 with one later (`FUN_005f9d6d`).
const SCHOOLBOY_AGE: (i32, i32) = (18, 19);
/// The kind of planner appointment that keeps the school news off the
/// day, the day from which it no longer does, and the June of the
/// Abitur (`FUN_005eab71`, `FUN_005f9b90`).
const TALK_KIND: u8 = 1;
const TALK_UNTIL: u16 = 0x16b;
const ABITUR_MONTH: u16 = 6;
/// Ten to nineteen off his contentment with his private life when the
/// girlfriend stays behind (`FUN_007010d9`), and four off his
/// distractions at the school's burden or exam (`FUN_00701210`).
const MOVE_KNOCK: i32 = -10;
const MOVE_SPREAD: u32 = 10;
const SCHOOL_KNOCK: i32 = -4;

/// The picked category 22 (`case 0x16` with `FUN_005f9fd0`, the label
/// "EreignisErsteMonate %d"): the day's news ends untold in the game's
/// first two seasons. Else its men are the club's squad in order, real
/// ids, standing at the club under four months ([`months_at_the_club`])
/// — twenty at most, one drawn; none and the news goes on. His talk
/// ([`girlfriend_stays`]).
pub fn first_months(
    records: &mut Records,
    club: (u16, u16),
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    if records.globals.seasons() < FIRST_MONTHS_SEASON {
        return true;
    }
    let Some(team) = records.team(club.0, club.1) else { return false };
    let newcomers: Vec<u16> = team
        .players()
        .into_iter()
        .filter(|&id| {
            id <= LAST_REAL_ID
                && records.player(id).is_some_and(|p| months_at_the_club(records, &p) < NEW_AT_THE_CLUB)
        })
        .take(MEN_MOST)
        .collect();
    if newcomers.is_empty() {
        return false;
    }
    let id = newcomers[random.below(newcomers.len() as u32) as usize];
    girlfriend_stays(records, club, id, posted, random)
}

/// How long a man stands at his club, in months (`FUN_006ff5d5`): with
/// stations of his career, the year and the month between today and the
/// last one's date, the month he came counted on (`FUN_006758f3`);
/// without one, `FUN_006758a6`'s thirtieth of the days between the
/// first of July 1997 and today — a count of the year's day number
/// ([`year_days`]) only, so it nears six by the year's end.
fn months_at_the_club(records: &Records, p: &crate::records::Player) -> i32 {
    let [day, month, year] = records.globals.date().map(|w| i32::from(w as i16));
    match p.joined() {
        Some((_, joined_month, joined_year)) => {
            (year - i32::from(joined_year)) * 12 + month - i32::from(joined_month) + 1
        }
        None => (year_days(day, month, year) - year_days(1, 7, 1997)) / 30,
    }
}

/// The year's day number of a date (`FUN_00675dd3` out of the month
/// table at `DAT_008a3088`): the whole months before this one,
/// February's 29 added from March on in a leap year (`FUN_00675e65`),
/// this day, less one.
fn year_days(day: i32, month: i32, year: i32) -> i32 {
    const MONTH_DAYS: [i32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let whole = usize::try_from(month - 1).unwrap_or(0).min(MONTH_DAYS.len());
    let leap = year % 400 == 0 || (year % 4 == 0 && year % 100 != 0);
    MONTH_DAYS[..whole].iter().sum::<i32>() - 1 + i32::from(leap && month > 2) + day
}

/// The new man's talk (`FUN_005f9ed7`, "EreignisErsteMonate %d"): the
/// second happening ends the news untold; the first asks a girlfriend —
/// not a wife, not none — no children and the two not far from parting
/// (her state, bits 2 to 4, nought), else it ends untold as well. She
/// will not move to the new town: ten to nineteen off his private life,
/// and the Trennung told with her first name (0x43b).
fn girlfriend_stays(
    records: &mut Records,
    club: (u16, u16),
    id: u16,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    match weighed(&FIRST_MONTHS, random) {
        1 => {}
        2 => return true,
        _ => return false,
    }
    let Some(p) = records.player(id) else { return true };
    if p.partner() & 3 != GIRLFRIEND || p.children() != 0 || state(&p) != 0 {
        return true;
    }
    change(records, id, PRIVATE, MOVE_KNOCK - random.below(MOVE_SPREAD) as i32);
    posted.push(Posted { club, code: PRIVATE_NEWS, text: 0x43b, fill: Fill::Partnered(id) });
    true
}

/// The picked category 23 (`case 0x17` with `FUN_005f9d6d`, the label
/// "EreignisSchüler %d"): a talk booked in the club's planner ends the
/// day's news untold ([`talk_booked`]). Else its men are the club's
/// squad in order, real ids, still at school (their standing, bits 3
/// and 4, not 3) and 18 or 19 in the season's start year
/// ([`SCHOOLBOY_AGE`]) — twenty at most, one drawn; none and the news
/// goes on. His day at school ([`schoolboy`]).
pub fn school_days(
    records: &mut Records,
    club: (u16, u16),
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    if talk_booked(records, club) {
        return true;
    }
    let Some(team) = records.team(club.0, club.1) else { return false };
    let start = i32::from(records.globals.start_year());
    let boys: Vec<u16> = team
        .players()
        .into_iter()
        .filter(|&id| {
            id <= LAST_REAL_ID
                && records.player(id).is_some_and(|p| {
                    (p.school() >> 3) & 3 != 3
                        && i32::from(p.birth_year())
                            == start
                                - if p.birth_month() < 8 { SCHOOLBOY_AGE.0 } else { SCHOOLBOY_AGE.1 }
                })
        })
        .take(MEN_MOST)
        .collect();
    if boys.is_empty() {
        return false;
    }
    let id = boys[random.below(boys.len() as u32) as usize];
    schoolboy(records, club, id, posted, random)
}

/// The club's planner standing against the school news (`FUN_005eab71`
/// over the planner of the club's human number, its +0x8a; out of 1 to
/// 4 zeroed and passing): from the season's day 0x16b on it is not
/// asked at all. Else an appointment of the kind 1 today ends the
/// news — or, failing that, the last such appointment before today,
/// when it names a man or a club.
fn talk_booked(records: &Records, club: (u16, u16)) -> bool {
    let today = records.globals.day();
    if today >= TALK_UNTIL {
        return false;
    }
    let Some(slot) = records
        .team(club.0, club.1)
        .and_then(|t| usize::from(t.human()).checked_sub(1))
    else {
        return false;
    };
    let Some(planner) = records.planners.get(slot) else { return false };
    let at = usize::from(today);
    planner.day(at).kind == TALK_KIND
        || (0..at)
            .rev()
            .find(|&d| planner.day(d).kind == TALK_KIND)
            .is_some_and(|d| planner.day(d).player != 0)
}

/// The schoolboy's day (`FUN_005f9b90`, "EreignisSchüler %d"), one of
/// five drawn alike: the school's burden (0x619) or an important exam
/// (0x61a), four off his distractions, unless the teacher already backs
/// him; while he stands at 0 at school, a teacher who hates football
/// (0x61b, his standing to 1) or one who supports him (0x61c, to 2);
/// and in June the Abitur done (0x61d). All told by his name; only a
/// drawn nought — which these weights never draw — lets the news go on.
fn schoolboy(
    records: &mut Records,
    club: (u16, u16),
    id: u16,
    posted: &mut Vec<Posted>,
    random: &mut Random,
) -> bool {
    let happening = weighed(&SCHOOL_DAYS, random);
    let standing = (records.player(id).map_or(0, |p| p.school()) >> 3) & 3;
    let text = match happening {
        1 | 2 if standing != 2 => {
            change(records, id, DISTRACTION, SCHOOL_KNOCK);
            if happening == 1 { 0x619 } else { 0x61a }
        }
        3 | 4 if standing == 0 => {
            if let Some(p) = records.player_mut(id) {
                p.set_school(p.school() & 0xe7 | ((happening - 2) << 3) as u8);
            }
            if happening == 3 { 0x61b } else { 0x61c }
        }
        5 if records.globals.date()[1] == ABITUR_MONTH => 0x61d,
        _ => return happening != 0,
    };
    posted.push(Posted { club, code: PRIVATE_NEWS, text, fill: Fill::NumberedOne(id) });
    true
}

/// The bit of a man's +0x216 marking his club's country as taken care of
/// language-wise (`FUN_00705c54` asks for it, `FUN_00705d4c` sets it and
/// clears the language part of +0x27f with it); the table at
/// `DAT_0089dec8` gives the bit to use per country, 117 of them.
const FOREIGN_SEEN: [u8; 117] = [
    1, 10, 9, 10, 4, 1, 0, 4, 5, 9, 4, 11, 4, 0, 0, 0,
    1, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 1,
    1, 4, 1, 8, 1, 8, 0, 1, 2, 4, 1, 1, 1, 1, 1, 0,
    4, 4, 0, 5, 4, 1, 8, 3, 0, 1, 1, 1, 1, 1, 1, 4,
    1, 1, 1, 1, 1, 1, 4, 1, 4, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1,
];
const FOREIGN_SEEN_AT: usize = 0x216;
/// The interpreter's sum the club's plan carries (club +0x270, the
/// Spielerbetreuer of KALKULATION.md): a man not marked yet needs no tale
/// at all on a roll of a hundred under the sum in ten-thousands, plus one.
const INTERPRETER: usize = 0x270 + crate::finances::CLUB_TO_TEAM;
const MONEY_UNIT: i32 = 10_000;
/// The language part of +0x27f (`FUN_00705cc0` reads it, `FUN_00705ce7`
/// writes it): nought no help, one a Sprachlehrerin, two an interpreter.
const LANG_CLEAR: u8 = 0xf3;
/// His bits at +0x1b4: he has been talked about already (`FUN_00707b34`
/// asks, `FUN_00707b56` sets) and his family has come to him
/// (`FUN_0070049f` asks, `FUN_007004c1` sets).
const TALKED: u32 = 0x40_0000;
const FAMILY_COMES: u32 = 0x10_0000;
/// His trait at +0x1ca's 0x8000: the talk of the men far from home counts
/// a man rooted only with it, from 26 years and 0x5b of morale.
const ROOTED: u16 = 0x8000;
const ROOTED_AGE: i32 = 0x1a;
const YOUNG_AGE: i32 = 0x17;
const MOTHER_AGE: i32 = 0x19;
const ROOTED_MORALE: u8 = 0x5b;
/// The tales of one day, and the men of each list at most.
const TALES_MOST: usize = 100;
const MEN_KEEP: usize = 5;
/// A tale still follows a man this many months at the club, and this few
/// months new he is worth the short ones; his family moved within three,
/// and had settled by thirteen.
const TALED_UNTIL: i32 = 0x18;
const JUST_ARRIVED: i32 = 4;
const FAMILY_NEW: i32 = 3;
const FAMILY_SETTLED: i32 = 0xd;
/// The lands of the cold (`0x32`) and of the crowd (`0x36`) tales.
const COLD_LANDS: [u8; 5] = [0x0e, 0x10, 0x39, 0x2f, 0x3b];
const CROWD_LANDS: [u8; 3] = [0x0e, 0x14, 0x10];
/// The nations a hookah and a crowd come from, and the kind the
/// language of the land before them is weighed by.
const COLD_NATION: u8 = 8;
const CROWD_NATION: u8 = 0x43;
const KIND_GROUP: u8 = 3;
/// The young man's company gives the homesick one ten of team mood; one
/// time in thirty the day pairs men instead of telling a tale.
const COMPANY: i32 = 10;
const PAIRING: u32 = 30;

/// The picked category 24 (`FUN_00600098`, 3451 bytes, the label
/// "EreignisHeimweh"): the men of the club's squad far from home. Over
/// its real ids in order, a man whose country bit at +0x216 is not set
/// yet escapes the whole business on a roll of a hundred under what the
/// club pays its interpreter; he is marked and his language part of
/// +0x27f cleared. Else, so many months at the club, the tales his
/// staying and his nation allow ([`tales_of`]) join the day's list, a
/// hundred at most, and he himself one of two lists of five
/// ([`list_men`]).
///
/// With a list of men empty, or 29 times in thirty, one of the tales is
/// drawn and its man marked as talked about; the news ends there, as the
/// original's switch ends it — only the knock the tale gives him and its
/// text are not built yet, so it stands untold. Else a homesick man and a
/// young one are drawn: the young much weaker than he and not shut out
/// nor among the homesick man's three dislikes, ten on the homesick
/// man's team mood ([`COMPANY`]), and the news goes on.
pub fn far_from_home(
    records: &mut Records,
    club: (u16, u16),
    nation_kinds: &[u8],
    random: &mut Random,
) -> bool {
    let Some(team) = records.team(club.0, club.1) else { return false };
    let land = team.country();
    let played = i32::try_from(crate::league::matchdays(records, team)).unwrap_or(i32::MAX);
    let squad = team.players();
    let helper = team.get::<i32>(INTERPRETER);
    // The original compares the roll unsigned, a nought of the plan's sum
    // still one man in a hundred.
    let paid = (helper / MONEY_UNIT + 1) as u32;
    let seen = u16::try_from(
        1u32 << (usize::from(FOREIGN_SEEN.get(usize::from(land)).copied().unwrap_or(0)) & 0x1f),
    )
    .unwrap_or(0);
    let mut tales: Vec<(u16, u16)> = Vec::new();
    let mut homesick: Vec<u16> = Vec::new();
    let mut young: Vec<u16> = Vec::new();
    for &id in squad.iter().filter(|&&id| id <= LAST_REAL_ID) {
        let (group, prev, months, unseen) = match records.player(id) {
            Some(p) => (
                nation_kinds.get(usize::from(p.nation())).copied().unwrap_or(0),
                previous_land(p),
                months_at_the_club(records, p),
                p.get::<u16>(FOREIGN_SEEN_AT) & seen == 0,
            ),
            None => continue,
        };
        if unseen {
            if random.below(100) < paid {
                if let Some(p) = records.player_mut(id) {
                    p.put::<u16>(FOREIGN_SEEN_AT, p.get::<u16>(FOREIGN_SEEN_AT) | seen);
                    // `FUN_00705d4c`'s 0xf3 and the `FUN_00705ce7(0)`
                    // after it: no language to speak of.
                    p.set_problems(p.problems() & LANG_CLEAR);
                }
                continue;
            }
            if months <= 0 {
                continue;
            }
            if let Some(p) = records.player(id) {
                for kind in tales_of(records, p, land, group, prev, months, nation_kinds, random) {
                    if tales.len() < TALES_MOST {
                        tales.push((id, kind));
                    }
                }
            }
        }
        if let Some(p) = records.player(id) {
            list_men(records, p, id, played, &mut homesick, &mut young);
        }
    }
    if homesick.is_empty() || young.is_empty() || random.below(PAIRING) != 0 {
        if tales.is_empty() {
            return false;
        }
        let (man, _kind) = tales[random.below(tales.len() as u32) as usize];
        if let Some(p) = records.player_mut(man) {
            p.set_shown(p.shown() | TALKED);
        }
        // The kind's knock on his contentment and its text (0x6e0 on) are
        // the second half of this handler, not built yet.
        true
    } else {
        let (old, lad) = (
            homesick[random.below(homesick.len() as u32) as usize],
            young[random.below(young.len() as u32) as usize],
        );
        let much_weaker = records.player(lad).is_some_and(|y| {
            records.player(old).is_some_and(|h| i32::from(y.strength()) + 1 < i32::from(h.strength()))
        });
        // `FUN_007034de` runs on the homesick man's record (`ecx`), the
        // young man's id in his three dislikes at +0x1fe, or the young
        // man's own shut-out mark.
        let welcome = records.player(lad).is_some_and(|y| {
            y.shown() & SHUT_OUT == 0
                && records
                    .player(old)
                    .is_some_and(|h| !h.dislikes().contains(&lad))
        });
        if much_weaker && welcome {
            // The young man takes the homesick one along; the hint of it
            // (0x6dc) is a window post, which is not pushed as news.
            change(records, old, TEAM, COMPANY);
        }
        false
    }
}

/// The country of his career's station before the last (`FUN_007044b1`):
/// the byte at +0x30 plus 0x14 for the station less two, by the count of
/// them at +0x212. With fewer than two stations the original reads below
/// the record; the replica keeps nought.
fn previous_land(p: &crate::records::Player) -> u8 {
    let stations = i32::from(p.career_count());
    if stations < 2 {
        return 0;
    }
    p.get::<u8>(0x30 + usize::try_from(stations - 2).unwrap_or(0) * 0x14)
}

/// The tales a man far from home allows, in the order the original tries
/// them: his own nation's kind `group` (`DAT_00ae5678`, the replica's
/// `nation_kinds`) and that of the land before (`prev`), `land` his
/// club's, and the language part of his +0x27f drawn from it. The kind
/// numbers stand as the switch's cases; 0x3a is allowed without the
/// talked-over mark the others want.
fn tales_of(
    records: &Records,
    p: &crate::records::Player,
    land: u8,
    group: u8,
    prev: u8,
    months: i32,
    nation_kinds: &[u8],
    random: &mut Random,
) -> Vec<u16> {
    let nation = p.nation();
    let talked = p.shown() & TALKED != 0;
    let lang = p.problems() >> 2 & 3;
    let settled = |land: u8| nation_kinds.get(usize::from(land)).copied().unwrap_or(0);
    let mut kinds = Vec::new();
    if !talked && months < TALED_UNTIL {
        kinds.push(0);
    }
    if state(p) == 0 && lang == 1 {
        kinds.push(1);
    }
    if state(p) == 3 && lang != 0 {
        kinds.push(2);
    }
    if lang == 0 {
        kinds.push(3);
    }
    if lang == 1 {
        kinds.push(4);
        kinds.push(5);
    }
    if lang == 2 {
        kinds.push(6);
    }
    if !talked
        && prev != 0
        && nation != land
        && crate::free_agents::age(records, p) < MOTHER_AGE
        && months < TALED_UNTIL
        && prev != land
    {
        kinds.push(7);
    }
    if !talked && nation != land && p.shown() & FAMILY_COMES == 0 {
        if months < FAMILY_NEW {
            kinds.push(8);
        } else if months < FAMILY_SETTLED {
            kinds.push(10);
        }
        kinds.push(9);
    }
    if !talked && (nation == COLD_NATION || group == KIND_GROUP) && months < TALED_UNTIL
        && COLD_LANDS.contains(&land)
    {
        kinds.push(0x32);
    }
    if !talked && prev == 0 && nation == 1 && months < JUST_ARRIVED {
        kinds.push(0x34 + random.below(2) as u16);
    }
    if !talked
        && (group == KIND_GROUP || group == 4 || nation == CROWD_NATION || nation == COLD_NATION)
        && CROWD_LANDS.contains(&land)
    {
        kinds.push(0x36);
    }
    if !talked && months < JUST_ARRIVED && settled(prev) == KIND_GROUP {
        kinds.push(0x38);
    }
    if months < JUST_ARRIVED && settled(prev) == KIND_GROUP {
        kinds.push(0x3a);
    }
    if !talked && months < JUST_ARRIVED && crate::mood::parts(p)[TEAM] < 100 {
        kinds.push(99);
    }
    kinds
}

/// Which of the two lists a man far from home joins (`LAB_006005f4`): a
/// man from 26, rooted and of 0x5b morale or better stands among the
/// homesick, five at most. The others may stand among the young — under
/// 23, not pleased with the team, short of half the league's matchdays
/// played, and only in a league of more than ten of them — five at most.
fn list_men(
    records: &Records,
    p: &crate::records::Player,
    id: u16,
    played: i32,
    homesick: &mut Vec<u16>,
    young: &mut Vec<u16>,
) {
    let age = crate::free_agents::age(records, p);
    if age < ROOTED_AGE || p.traits()[1] & ROOTED == 0 || p.morale() < ROOTED_MORALE {
        if age < YOUNG_AGE
            && crate::mood::parts(p)[TEAM] < 100
            && i32::from(p.league_matches()) < played / 2
            && played > 10
            && young.len() < MEN_KEEP
        {
            young.push(id);
        }
    } else if homesick.len() < MEN_KEEP {
        homesick.push(id);
    }
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
            tire(records, id, -20);
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

/// Whether a text is one of categories 22 and 23's: the girlfriend who
/// stays behind (0x43b), and the schoolboy's burden, exam, teachers and
/// Abitur (0x619 to 0x61d).
pub fn newcomer_text(text: u16) -> bool {
    text == 0x43b || (0x619..=0x61d).contains(&text)
}

/// Whether a text is one of categories 18, 19, 50, 51 and 55's.
pub fn squad_text(text: u16) -> bool {
    (0x3b8..=0x3cb).contains(&text)
        || (0x482..=0x4d0).contains(&text)
        || text == 0x3d6
        || text == 0x3d7
        || (0x54c..=0x54f).contains(&text)
        || text == NEW_HOBBY_TEXT
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
    weighed_within(table, 0, random)
}

/// One draw over a weighed table registered at a cap (`FUN_0060bdb7`
/// raises it to the weights' sum): a roll past the weights draws 0.
fn weighed_within(table: &[(u32, u16)], cap: u32, random: &mut Random) -> u16 {
    let mut rest = random.below(cap.max(table.iter().map(|&(weight, _)| weight).sum()));
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

    /// Categories 18 and 19 on A's human squad, one man drawn: without a
    /// hobby he takes up one of the weighted table and is told by his name
    /// and the hobby's text — the midfielder's hobby drawn for a defender
    /// leaves him without; with one, 18 has nothing more to say. Category
    /// 19 goes on to the later rolls for a man without a hobby, one draw
    /// taken, and stands for the day's news for a man with one.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn categories_18_and_19_turn_on_a_mans_hobby() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let tables =
            crate::first_season::Tables::load(&a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap()).unwrap();
        let league = crate::league::LeagueTables::load(&a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap()).unwrap();
        let mut r = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let (hc, hn) = crate::statistics::human_clubs(&r)[0];
        let club = (u16::from(hc), hn as u16);
        let squad = r.team(club.0, club.1).unwrap().players();
        let count = r.team(club.0, club.1).unwrap().player_count() as u32;
        let man = squad[3];
        assert!(man <= LAST_REAL_ID);
        let p = r.player_mut(man).unwrap();
        p.set_hobby(0);
        let mut at = p.positions();
        at[0] = 2;
        p.set_positions(at);
        let seed = |keep: &dyn Fn(&mut Random) -> bool| (0..100_000u64).find(|&s| keep(&mut Random::new(s))).unwrap();
        let drawn = |pick: &dyn Fn(u8) -> bool| seed(&|r| r.below(count) == 3 && pick(tables.hobby(r)));

        // The midfielder's hobby for a defender: nothing.
        let mut posted = Vec::new();
        new_hobby(&mut r, club, &tables, &mut posted, &mut Random::new(drawn(&|h| h == MIDFIELD_HOBBY)));
        assert!(posted.is_empty());
        assert_eq!(r.player(man).unwrap().hobby(), 0);
        // Without a hobby category 19 goes on, one draw taken.
        let s = seed(&|r| r.below(count) == 3);
        let mut random = Random::new(s);
        assert!(!about_a_player(&mut r, club, 63, false, &league, &mut posted, &mut random));
        assert!(posted.is_empty());
        let mut expected = Random::new(s);
        expected.below(count);
        assert!(random == expected);

        // Another: taken and told.
        let s = drawn(&|h| h != MIDFIELD_HOBBY && h != 0);
        let mut expected = Random::new(s);
        expected.below(count);
        let hobby = tables.hobby(&mut expected);
        let mut random = Random::new(s);
        new_hobby(&mut r, club, &tables, &mut posted, &mut random);
        assert!(random == expected, "the man and his hobby drawn");
        assert_eq!(r.player(man).unwrap().hobby(), hobby);
        let what = tables.hobby_text(hobby);
        assert_ne!(what, 0);
        assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text: NEW_HOBBY_TEXT, fill: Fill::NamedGame(man, what) }]);
        let texts = a3_files::saq::EventTexts::load(&dir).unwrap();
        let told = crate::event_text::make(&r, &texts, &|id| format!("<{id}>"), &[], &posted[0]).text;
        assert!(told.contains(&r.known_as(man).unwrap()) && told.contains(&format!("<{what}>")), "{told}");

        // With his hobby 18 tells nothing more, one draw taken; 19 stands.
        let s = seed(&|r| r.below(count) == 3);
        let mut random = Random::new(s);
        new_hobby(&mut r, club, &tables, &mut posted, &mut random);
        assert_eq!(posted.len(), 1);
        let mut expected = Random::new(s);
        expected.below(count);
        assert!(random == expected);
        assert!(about_a_player(&mut r, club, 63, false, &league, &mut Vec::new(), &mut Random::new(s)));
    }

    /// Category 19 on A's human squad, the drawn man given a hobby: the
    /// fresh fish lift the team's mood ten, fishing's infection knocks him
    /// about five days, the garden's spade lays him up for the table's 112
    /// days — not while he is out already — the bees take his hobby, stocks
    /// make him a collector of stamps, travel tells only before the third
    /// day and sends him away a week; art moves his distractions without a
    /// word. Each tells by his name and ends the day's news.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_mans_hobby_hurts_him_or_moves_his_mood() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = || Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let a = load();
        let league = crate::league::LeagueTables::load(&a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap()).unwrap();
        let (hc, hn) = crate::statistics::human_clubs(&a)[0];
        let club = (u16::from(hc), hn as u16);
        let count = a.team(club.0, club.1).unwrap().player_count() as u32;
        let man = a.team(club.0, club.1).unwrap().players()[3];
        let today = i32::from(a.globals.day());
        let seed = |n: u32, k: u32| (0..100_000u64).find(|&s| {
            let mut r = Random::new(s);
            r.below(count) == 3 && r.below(n) == k
        }).unwrap();
        // His day with `hobby` on `day`, the hobby's first draw `k` of `n`.
        let run = |hobby: u8, day: i32, out: bool, (n, k): (u32, u32)| {
            let mut r = load();
            let p = r.player_mut(man).unwrap();
            p.set_hobby(hobby);
            p.set_injured_to([0, 0]);
            if out {
                crate::training_day::injure(&mut r, man, 1, 10, day);
            }
            let mut posted = Vec::new();
            let mut random = Random::new(seed(n, k));
            assert!(about_a_player(&mut r, club, day, false, &league, &mut posted, &mut random));
            (r, posted, random)
        };
        let told = |text: u16| vec![Posted { club, code: PRIVATE_NEWS, text, fill: Fill::NumberedOne(man) }];

        let (r, posted, _) = run(FISHING, today, false, (3, 1));
        assert_eq!(posted, told(0x483));
        assert_eq!(i32::from(r.team(club.0, club.1).unwrap().mood()), (i32::from(a.team(club.0, club.1).unwrap().mood()) + 10).min(200));
        let texts = a3_files::saq::EventTexts::load(&dir).unwrap();
        let text = crate::event_text::make(&r, &texts, &|id| format!("<{id}>"), &[], &posted[0]).text;
        assert!(text.contains(&r.known_as(man).unwrap()) && text.contains("Fische"), "{text}");

        let (r, posted, _) = run(FISHING, today, false, (3, 0));
        assert_eq!(posted, told(0x482));
        assert_eq!(r.player(man).unwrap().get::<u16>(0x1de), (today + 5) as u16);

        let (r, posted, _) = run(GARDEN, today, false, (3, 0));
        assert_eq!(posted, told(0x4a9));
        let p = r.player(man).unwrap();
        assert_eq!((p.injury_kind(), crate::load::days_to(&r.globals, p.injured_to())), SPADE);
        let (r, posted, _) = run(GARDEN, today, true, (3, 0));
        assert!(posted.is_empty());
        assert_eq!(r.player(man).unwrap().injury_kind(), 1);

        let (r, posted, random) = run(BEES, today, false, (3, 0));
        assert_eq!(posted, told(0x4ba));
        assert_eq!(r.player(man).unwrap().hobby(), 0);
        let mut expected = Random::new(seed(3, 0));
        expected.below(count);
        assert!(random == expected, "the man alone drawn");

        let (r, posted, _) = run(STOCKS, today, false, (3, 1));
        assert_eq!(posted, told(0x48c));
        assert_eq!(r.player(man).unwrap().hobby(), STAMPS);

        let (_, posted, random) = run(TRAVEL, 5, false, (3, 1));
        assert!(posted.is_empty());
        let mut expected = Random::new(seed(3, 1));
        expected.below(count);
        assert!(random == expected, "no draw after the third day");
        let (r, posted, _) = run(TRAVEL, 1, false, (3, 1));
        assert_eq!(posted, told(0x4c4));
        assert_eq!(crate::load::days_to(&r.globals, r.player(man).unwrap().away_to()), 7);

        let (r, posted, _) = run(ART, today, false, (4, 0));
        assert!(posted.is_empty());
        let before = i32::from(a.player(man).unwrap().contentment()[DISTRACTION]);
        assert_eq!(i32::from(r.player(man).unwrap().contentment()[DISTRACTION]), (before - 5).max(0));
    }

    /// Category 19 on A's human squad, the drawn man's hobby reaching the
    /// press: a diary after day 189 sours the team's mood twenty and
    /// marks an offence of level 2, telling nothing; a man over 30 tells
    /// of doping, or of two team-mates who fall out with each other over
    /// his book; a German man with a child on the way names it once; a
    /// tattoo knocks him two days, a sunburn only fair skin; a new woman
    /// costs two of the press; gambling given up takes the hobby away;
    /// golf draws nothing past the man.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_mans_hobby_reaches_the_press() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = || Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let a = load();
        let league = crate::league::LeagueTables::load(&a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap()).unwrap();
        let (hc, hn) = crate::statistics::human_clubs(&a)[0];
        let club = (u16::from(hc), hn as u16);
        let count = a.team(club.0, club.1).unwrap().player_count() as u32;
        let man = a.team(club.0, club.1).unwrap().players()[3];
        let today = i32::from(a.globals.day());
        let year = a.globals.date()[2] as u16;
        // A seed drawing the man and then each `(n, k)`.
        let seed = |draws: &[(u32, u32)]| {
            (0..1_000_000u64)
                .find(|&s| {
                    let mut r = Random::new(s);
                    r.below(count) == 3 && draws.iter().all(|&(n, k)| r.below(n) == k)
                })
                .unwrap()
        };
        let run = |r: &mut Records, hobby: u8, day: i32, draws: &[(u32, u32)]| {
            r.player_mut(man).unwrap().set_hobby(hobby);
            let mut posted = Vec::new();
            let mut random = Random::new(seed(draws));
            assert!(about_a_player(r, club, day, false, &league, &mut posted, &mut random));
            (posted, random)
        };
        let told = |text: u16| vec![Posted { club, code: PRIVATE_NEWS, text, fill: Fill::NumberedOne(man) }];
        let mood = |r: &Records| i32::from(r.team(club.0, club.1).unwrap().mood());

        // The diary: nothing before day 190, then the mood and an offence.
        let mut r = load();
        let (posted, _) = run(&mut r, BOOKS, DIARY_AFTER, &[(2, 0)]);
        assert!(posted.is_empty());
        assert_eq!((mood(&r), r.player(man).unwrap().offences()), (mood(&a), a.player(man).unwrap().offences()));
        let (posted, _) = run(&mut r, BOOKS, DIARY_AFTER + 1, &[(2, 0)]);
        assert!(posted.is_empty());
        assert_eq!(mood(&r), (mood(&a) - 20).max(0));
        let p = r.player(man).unwrap();
        assert_eq!(p.offences(), a.player(man).unwrap().offences() | 0x20);
        let counted = (a.player(man).unwrap().offence_counts() >> 2 & 3).saturating_add(1).min(3);
        assert_eq!(p.offence_counts() >> 2 & 3, counted);

        // The life's story of a man over 30.
        let older = |r: &mut Records, by: u16| {
            let p = r.player_mut(man).unwrap();
            p.set_birth_year(year - by);
            p.set_shown(p.shown() & !BOOK_BARRED);
        };
        let mut r = load();
        older(&mut r, 25);
        let (posted, random) = run(&mut r, BOOKS, today, &[(2, 1), (3, 0)]);
        assert!(posted.is_empty());
        let mut expected = Random::new(seed(&[(2, 1), (3, 0)]));
        expected.below(count);
        expected.below(2);
        assert!(random == expected, "no third draw for a young man");
        older(&mut r, 35);
        assert_eq!(run(&mut r, BOOKS, today, &[(2, 1), (3, 0)]).0, told(0x491));
        assert_eq!(run(&mut r, BOOKS, today, &[(2, 1), (3, 1)]).0, told(0x493));
        // Two team-mates drawn fall out with each other.
        let (posted, _) = run(&mut r, BOOKS, today, &[(2, 1), (3, 2)]);
        let [Posted { text: 0x494, fill: Fill::NumberedThree(him, one, other), .. }] = posted[..] else { panic!("{posted:?}") };
        assert!(him == man && one != other && one != man && other != man);
        assert!(r.player(one).unwrap().dislikes().contains(&other) && r.player(other).unwrap().dislikes().contains(&one));
        let texts = a3_files::saq::EventTexts::load(&dir).unwrap();
        let text = crate::event_text::make(&r, &texts, &|id| format!("<{id}>"), &[], &posted[0]).text;
        for id in [man, one, other] {
            assert!(text.contains(&r.known_as(id).unwrap()), "{text}");
        }

        // A child on the way, named once; a child named so already.
        let mut r = load();
        let p = r.player_mut(man).unwrap();
        p.set_nation(GERMANY);
        p.set_shown(p.shown() & !NAMED | EXPECTING);
        p.set_children(0);
        assert_eq!(run(&mut r, COMPUTER_GAMES, today, &[(2, 0)]).0, told(0x496));
        assert_ne!(r.player(man).unwrap().shown() & NAMED_LARA, 0);
        assert!(run(&mut r, COMPUTER_GAMES, today, &[(2, 1)]).0.is_empty());
        let p = r.player_mut(man).unwrap();
        p.set_shown(p.shown() & !NAMED);
        p.set_children(1);
        p.set_children_names([ROLF, 0, 0, 0, 0]);
        assert!(run(&mut r, COMPUTER_GAMES, today, &[(2, 1)]).0.is_empty());
        assert_eq!(run(&mut r, COMPUTER_GAMES, today, &[(2, 0)]).0, told(0x497));

        // Looks: the tattoo, the sunburn for fair skin alone, a new woman.
        let mut r = load();
        assert_eq!(run(&mut r, LOOKS, today, &[(6, 1)]).0, told(0x49f));
        assert_eq!(i32::from(r.player(man).unwrap().get::<u16>(0x1de)), today + 2);
        let p = r.player_mut(man).unwrap();
        p.set_face([1, p.face()[1]]);
        assert!(run(&mut r, LOOKS, today, &[(6, 0)]).0.is_empty());
        let p = r.player_mut(man).unwrap();
        p.set_face([8, p.face()[1]]);
        assert_eq!(run(&mut r, LOOKS, today, &[(6, 0)]).0, told(0x49e));
        let press = i32::from(r.player(man).unwrap().contentment()[PRESS]);
        assert!(run(&mut r, LOOKS, today, &[(6, 3)]).0.is_empty());
        assert_eq!(i32::from(r.player(man).unwrap().contentment()[PRESS]), (press - 2).max(0));

        // Gambling given up; golf.
        let mut r = load();
        let before = i32::from(r.player(man).unwrap().contentment()[DISTRACTION]);
        assert!(run(&mut r, GAMBLING, today, &[(4, 3)]).0.is_empty());
        assert_eq!(r.player(man).unwrap().hobby(), 0);
        assert_eq!(i32::from(r.player(man).unwrap().contentment()[DISTRACTION]), (before + 6).min(crate::mood::MOST));
        let (posted, random) = run(&mut r, GOLF, today, &[]);
        assert!(posted.is_empty());
        let mut expected = Random::new(seed(&[]));
        expected.below(count);
        assert!(random == expected, "golf draws nothing");
    }

    /// Category 19 on A's human squad, the drawn man given food, a
    /// motorbike or a sports car: food tells only of a man in poor
    /// condition, makes him overweight or slim, at Christmas without a
    /// draw; a gluttony notes an offence and posts nothing. The motorbike
    /// lays a man over 17 up for the table's days or takes his licence two
    /// months; the sports car crashes only a licensed man not out, takes a
    /// married man's press two, and a licence a month.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_mans_hobby_of_food_and_wheels() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = || Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let a = load();
        let league = crate::league::LeagueTables::load(&a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap()).unwrap();
        let (hc, hn) = crate::statistics::human_clubs(&a)[0];
        let club = (u16::from(hc), hn as u16);
        let count = a.team(club.0, club.1).unwrap().player_count() as u32;
        let man = a.team(club.0, club.1).unwrap().players()[3];
        let today = i32::from(a.globals.day());
        let [_, _, year] = a.globals.date();
        let seed = |draws: &[(u32, u32)]| {
            (0..1_000_000u64)
                .find(|&s| {
                    let mut r = Random::new(s);
                    r.below(count) == 3 && draws.iter().all(|&(n, k)| r.below(n) == k)
                })
                .unwrap()
        };
        let run = |r: &mut Records, hobby: u8, draws: &[(u32, u32)]| {
            r.player_mut(man).unwrap().set_hobby(hobby);
            let mut posted = Vec::new();
            let mut random = Random::new(seed(draws));
            assert!(about_a_player(r, club, today, false, &league, &mut posted, &mut random));
            (posted, random)
        };
        let told = |text: u16| vec![Posted { club, code: PRIVATE_NEWS, text, fill: Fill::NumberedOne(man) }];
        let shown = |r: &Records| r.player(man).unwrap().shown();
        let set = |r: &mut Records, condition: i16, smokes: bool, date: [u16; 3]| {
            let p = r.player_mut(man).unwrap();
            p.set_condition(condition);
            p.set_shown(if smokes { p.shown() | SMOKES } else { p.shown() & !SMOKES } & !OVERWEIGHT);
            p.set_injured_to([0, 0]);
            r.globals.set_date(date);
        };

        // Food: a fit man tells nothing; a smoker's condition counts five less.
        let mut r = load();
        set(&mut r, 6_000, false, [15, 10, year]);
        let (posted, random) = run(&mut r, FOOD, &[(6, 0)]);
        assert!(posted.is_empty());
        let mut expected = Random::new(seed(&[(6, 0)]));
        expected.below(count);
        assert!(random == expected, "no draw for a fit man");
        set(&mut r, 6_400, true, [15, 10, year]);
        assert_eq!(run(&mut r, FOOD, &[(6, 0)]).0, told(0x4b2));
        assert_ne!(shown(&r) & OVERWEIGHT, 0);
        assert_eq!(run(&mut r, FOOD, &[(6, 1)]).0, told(0x4b3));
        assert_eq!(shown(&r) & OVERWEIGHT, 0);
        assert_eq!(run(&mut r, FOOD, &[(6, 3)]).0, told(0x4b7));
        // Christmas: overweight without a draw.
        for date in [[28, 12, year], [13, 1, year]] {
            set(&mut r, 5_000, false, date);
            let (posted, random) = run(&mut r, FOOD, &[(6, 1)]);
            assert_eq!(posted, told(0x4b4));
            assert_ne!(shown(&r) & OVERWEIGHT, 0);
            let mut expected = Random::new(seed(&[(6, 1)]));
            expected.below(count);
            assert!(random == expected, "no draw at Christmas");
        }
        set(&mut r, 5_000, false, [14, 1, year]);
        assert_eq!(run(&mut r, FOOD, &[(6, 1)]).0, told(0x4b3));
        // A gluttony: an offence of level 1, nothing posted.
        let mut r = load();
        set(&mut r, 5_000, false, [15, 10, year]);
        let counted = (a.player(man).unwrap().offence_counts() & 3).saturating_add(1).min(3);
        assert!(run(&mut r, FOOD, &[(6, 4)]).0.is_empty());
        assert_ne!(shown(&r) & OVERWEIGHT, 0);
        assert_eq!(r.player(man).unwrap().offence_counts() & 3, counted);
        assert_eq!(run(&mut r, FOOD, &[(6, 5)]).0, told(0x4b9));
        let p = r.player(man).unwrap();
        assert_eq!((p.injury_kind(), crate::load::days_to(&r.globals, p.injured_to())), STOMACH);

        // Motorbikes: nothing for a man of 17, else the table's days.
        let mut r = load();
        let age = |r: &mut Records, years: u16| {
            let p = r.player_mut(man).unwrap();
            p.set_birth_year(year - years - 1);
            p.set_birth_day(31);
            p.set_birth_month(12);
            p.put::<u8>(NO_LICENCE, 0);
            p.set_injured_to([0, 0]);
        };
        age(&mut r, 17);
        assert!(run(&mut r, MOTORBIKES, &[(6, 0)]).0.is_empty());
        age(&mut r, 18);
        assert_eq!(run(&mut r, MOTORBIKES, &[(6, 1)]).0, told(0x4c0));
        let p = r.player(man).unwrap();
        assert_eq!((p.injury_kind(), crate::load::days_to(&r.globals, p.injured_to())), BIKE_CRASH);
        age(&mut r, 18);
        assert_eq!(run(&mut r, MOTORBIKES, &[(6, 0)]).0, told(0x4bf));
        // The licence: two months gone, the press five less; once only.
        age(&mut r, 18);
        let press = i32::from(r.player(man).unwrap().contentment()[PRESS]);
        assert!(run(&mut r, MOTORBIKES, &[(6, 2)]).0.is_empty());
        assert_eq!(r.player(man).unwrap().get::<u8>(NO_LICENCE), BIKE_BAN);
        assert_eq!(i32::from(r.player(man).unwrap().contentment()[PRESS]), (press - 5).max(0));
        let press = r.player(man).unwrap().contentment()[PRESS];
        assert!(run(&mut r, MOTORBIKES, &[(6, 2)]).0.is_empty());
        assert_eq!(r.player(man).unwrap().contentment()[PRESS], press);

        // Sports cars: a crash for a licensed man not out.
        assert!(run(&mut r, SPORTS_CARS, &[(6, 0)]).0.is_empty());
        age(&mut r, 18);
        crate::training_day::injure(&mut r, man, 1, 10, today);
        assert!(run(&mut r, SPORTS_CARS, &[(6, 0)]).0.is_empty());
        age(&mut r, 18);
        assert_eq!(run(&mut r, SPORTS_CARS, &[(6, 0)]).0, told(0x4cb));
        assert_eq!(r.player(man).unwrap().injury_kind(), CAR_CRASH.0);
        // A licence a month and three less; a wife two less.
        age(&mut r, 18);
        let press = i32::from(r.player(man).unwrap().contentment()[PRESS]);
        assert!(run(&mut r, SPORTS_CARS, &[(6, 3)]).0.is_empty());
        assert_eq!(r.player(man).unwrap().get::<u8>(NO_LICENCE), CAR_BAN);
        assert_eq!(i32::from(r.player(man).unwrap().contentment()[PRESS]), (press - 3).max(0));
        let p = r.player_mut(man).unwrap();
        p.set_partner(p.partner() & !3);
        let press = r.player(man).unwrap().contentment()[PRESS];
        assert!(run(&mut r, SPORTS_CARS, &[(6, 1)]).0.is_empty());
        assert_eq!(r.player(man).unwrap().contentment()[PRESS], press);
        let p = r.player_mut(man).unwrap();
        p.set_partner(p.partner() & !3 | WIFE);
        assert!(run(&mut r, SPORTS_CARS, &[(6, 2)]).0.is_empty());
        assert_eq!(i32::from(r.player(man).unwrap().contentment()[PRESS]), (i32::from(press) - 2).max(0));
        let texts = a3_files::saq::EventTexts::load(&dir).unwrap();
        for text in [0x4b2, 0x4b3, 0x4b4, 0x4b6, 0x4b7, 0x4b9, 0x4bf, 0x4c0, 0x4cb] {
            let event = crate::event_text::make(&r, &texts, &|id| format!("<{id}>"), &[], &told(text)[0]).text;
            assert!(event.contains(&r.known_as(man).unwrap()), "{text:#x}: {event}");
        }
    }

    /// Categories 20 and 21 on A's human squad, the fourth man drawn: the
    /// press takes eight off a sensitive man in poor form and off one in
    /// his contract's last year, ten off a strong unhappy millionaire, and
    /// lets an ordinary man be, the news going on. Category 21's roll
    /// mostly draws nothing and goes on; a kick and an elbow are told by
    /// his name, once only while they stand among the recent events; a
    /// knee notes an offence untold; a refusal on a day of training puts
    /// his contentment with the trainer to nought; a loyal man or a club
    /// without its trainer's number has nothing of it.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn categories_20_and_21_tell_of_the_press_and_the_trainer() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let league = crate::league::LeagueTables::load(&a3_files::exe::Exe::open(&dir.join("anstoss3.exe")).unwrap()).unwrap();
        let mut r = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let (hc, hn) = crate::statistics::human_clubs(&r)[0];
        let club = (u16::from(hc), hn as u16);
        let count = r.team(club.0, club.1).unwrap().player_count() as u32;
        let man = r.team(club.0, club.1).unwrap().players()[3];
        assert!(man <= LAST_REAL_ID);
        let year = r.globals.start_year();
        let press = |r: &Records| r.player(man).unwrap().contentment()[PRESS];
        let seed = |keep: &dyn Fn(&mut Random) -> bool| (0..2_000_000u64).find(|&s| keep(&mut Random::new(s))).unwrap();
        let fourth = seed(&|r| r.below(count) == 3);

        // An ordinary man: nothing, one draw.
        let p = r.player_mut(man).unwrap();
        p.set_traits([p.traits()[0], 0]);
        p.set_strength(5);
        p.set_contract(year + 3);
        p.set_morale(100);
        p.set_grade(4);
        p.set_flags(0);
        let mut parts = p.contentment();
        parts[PRESS] = 60;
        parts[PRIVATE] = 100;
        p.set_contentment(parts);
        let mut random = Random::new(fourth);
        assert!(!press_on_a_player(&mut r, club, &league, &mut random));
        let mut expected = Random::new(fourth);
        expected.below(count);
        assert!(random == expected);
        assert_eq!(press(&r), 60);
        // A sensitive man in poor form.
        let p = r.player_mut(man).unwrap();
        p.set_traits([p.traits()[0], SENSITIVE]);
        p.set_morale(70);
        p.set_grade(SENSITIVE_GRADE);
        assert!(press_on_a_player(&mut r, club, &league, &mut Random::new(fourth)));
        assert_eq!(press(&r), 52);
        // In his contract's last year.
        let p = r.player_mut(man).unwrap();
        p.set_traits([p.traits()[0], 0]);
        p.set_strength(10);
        p.set_contract(year + 1);
        assert!(press_on_a_player(&mut r, club, &league, &mut Random::new(fourth)));
        assert_eq!(press(&r), 44);
        // A strong unhappy millionaire past 24.
        let p = r.player_mut(man).unwrap();
        p.set_contract(year + 3);
        p.set_birth_year(year - 30);
        let mut parts = p.contentment();
        parts[PRIVATE] = 80;
        p.set_contentment(parts);
        assert!(press_on_a_player(&mut r, club, &league, &mut Random::new(fourth)));
        assert_eq!(press(&r), 34);
        // His season's grades never pass 35.
        let p = r.player_mut(man).unwrap();
        p.set_matchdays([0x80 | 7 << 2; 46]);
        assert_eq!(season_grades(p), 35);

        // Category 21.
        assert_ne!(r.team(club.0, club.1).unwrap().humans()[0], 0);
        let happening = |k: u32| seed(&|r| r.below(count) == 3 && (k * 10..k * 10 + 10).contains(&r.below(REVOLTS_CAP)));
        let training = [0x11, crate::training::FREE, crate::training::FREE];
        let mut recent = Vec::new();
        let mut posted = Vec::new();
        let p = r.player_mut(man).unwrap();
        let mut parts = p.contentment();
        parts[0] = 60;
        p.set_contentment(parts);
        // Mostly nothing.
        let none = seed(&|r| r.below(count) == 3 && r.below(REVOLTS_CAP) >= 60);
        let mut random = Random::new(none);
        assert!(!revolt(&mut r, club, 63, training, &league, (&mut recent, &mut posted), &mut random));
        let mut expected = Random::new(none);
        expected.below(count);
        expected.below(REVOLTS_CAP);
        assert!(random == expected && recent.is_empty() && posted.is_empty());
        // A kick, once while it stands among the recent events.
        let s = happening(1);
        assert!(revolt(&mut r, club, 63, training, &league, (&mut recent, &mut posted), &mut Random::new(s)));
        assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text: 0x5f6, fill: Fill::NumberedOne(man) }]);
        assert_eq!(recent, [REVOLT_MET + 2]);
        assert!(revolt(&mut r, club, 63, training, &league, (&mut recent, &mut posted), &mut Random::new(s)));
        assert_eq!(posted.len(), 1);
        // An elbow and a knee from a man with the trainer's grievance.
        r.player_mut(man).unwrap().set_flags(0x1000);
        assert!(revolt(&mut r, club, 63, training, &league, (&mut recent, &mut posted), &mut Random::new(happening(4))));
        assert_eq!(posted[1], Posted { club, code: PRIVATE_NEWS, text: 0x5fb, fill: Fill::NumberedOne(man) });
        let offences = r.player(man).unwrap().offences();
        assert!(revolt(&mut r, club, 63, training, &league, (&mut recent, &mut posted), &mut Random::new(happening(5))));
        assert_eq!(posted.len(), 2);
        assert_ne!(r.player(man).unwrap().offences(), offences);
        r.player_mut(man).unwrap().set_flags(0);
        // A refusal: nothing on a free day, nought on a day of training.
        let s = happening(0);
        let free = [crate::training::FREE; 3];
        assert!(revolt(&mut r, club, 63, free, &league, (&mut recent, &mut posted), &mut Random::new(s)));
        assert_eq!(r.player(man).unwrap().contentment()[0], 60);
        recent.clear();
        assert!(revolt(&mut r, club, 63, training, &league, (&mut recent, &mut posted), &mut Random::new(s)));
        assert_eq!(r.player(man).unwrap().contentment()[0], 0);
        assert_eq!(posted.len(), 2);
        // A loyal man: nothing, one draw.
        let p = r.player_mut(man).unwrap();
        p.set_traits([p.traits()[0], 0x20]);
        let mut random = Random::new(happening(1));
        recent.clear();
        assert!(!revolt(&mut r, club, 63, training, &league, (&mut recent, &mut posted), &mut random));
        let mut expected = Random::new(happening(1));
        expected.below(count);
        assert!(random == expected && recent.is_empty());
        // A club without its trainer's number: no draw.
        let team = r.team_mut(club.0, club.1).unwrap();
        let humans = team.humans();
        team.set_humans([0, humans[1]]);
        let mut random = Random::new(happening(1));
        assert!(!revolt(&mut r, club, 63, training, &league, (&mut recent, &mut posted), &mut random));
        assert!(random == Random::new(happening(1)));
    }

    /// Categories 22 and 23 on A's human squad, the fourth man drawn:
    /// while the game writes its second season the news ends untold; in
    /// its third he, signed with no career station and under four months
    /// at the club, with a girlfriend, no children and the two at peace,
    /// hears the Trennung (0x43b, with her first name) and takes ten to
    /// nineteen off his private life — the second happening, a wife and
    /// a father end it untold. Category 23: an appointment of kind 1 in
    /// the club's planner today ends the news untold; else the one
    /// schoolboy — 18 in the start year with a spring birthday — hears
    /// the teacher who hates football (0x61b, his standing to 1, only
    /// while it stands at 0) or the school's burden (0x619, four off his
    /// distractions, but not for a teacher who backs him), and in June
    /// the Abitur (0x61d) while out of June nothing is told; a schoolboy
    /// done with school leaves the news going on.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn categories_22_and_23_talk_to_newcomers_and_schoolboys() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let mut r = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let (hc, hn) = crate::statistics::human_clubs(&r)[0];
        let club = (u16::from(hc), hn as u16);
        let squad = r.team(club.0, club.1).unwrap().players();
        let real: Vec<u16> = squad.into_iter().filter(|&id| id <= LAST_REAL_ID).collect();
        let man = real[3];
        let [day, _, year] = r.globals.date();
        let start = r.globals.start_year();
        let seed = |keep: &dyn Fn(&mut Random) -> bool| (0..2_000_000u64).find(|&s| keep(&mut Random::new(s))).unwrap();
        let away = seed(&|r| r.below(2) == 0);

        // Category 22: he stands at the club without a career station,
        // all the others since before the last season.
        for &id in real.iter().filter(|&&id| id != man) {
            let p = r.player_mut(id).unwrap();
            p.set_career_count(1);
            p.put::<u8>(0x20, 1);
            p.put::<u8>(0x21, 1);
            p.put::<u16>(0x22, year - 2);
        }
        let p = r.player_mut(man).unwrap();
        p.set_career_count(0);
        p.set_partner(GIRLFRIEND);
        p.set_children(0);
        let mut parts = p.contentment();
        parts[PRIVATE] = 100;
        p.set_contentment(parts);
        assert!(months_at_the_club(&r, &r.player(man).unwrap()) < NEW_AT_THE_CLUB);
        let mut posted = Vec::new();
        // The game's second season: untold, no draw.
        r.globals.set_seasons(2);
        let mut random = Random::new(away);
        assert!(first_months(&mut r, club, &mut posted, &mut random));
        assert!(random == Random::new(away) && posted.is_empty());
        // Its third: the talk and the knock on his private life.
        r.globals.set_seasons(FIRST_MONTHS_SEASON);
        let mut random = Random::new(away);
        assert!(first_months(&mut r, club, &mut posted, &mut random));
        assert_eq!(posted, [Posted { club, code: PRIVATE_NEWS, text: 0x43b, fill: Fill::Partnered(man) }]);
        let mut expected = Random::new(away);
        expected.below(2);
        let knock = 10 + expected.below(10);
        assert_eq!(r.player(man).unwrap().contentment()[PRIVATE], (100 - knock) as u8);
        assert!(random == expected);
        // The second happening, a wife and a father end it untold.
        let second = seed(&|r| r.below(2) == 1);
        assert!(first_months(&mut r, club, &mut posted, &mut Random::new(second)));
        r.player_mut(man).unwrap().set_partner(WIFE);
        let mut random = Random::new(away);
        assert!(first_months(&mut r, club, &mut posted, &mut random));
        let mut expected = Random::new(away);
        expected.below(2);
        assert!(random == expected && posted.len() == 1);
        r.player_mut(man).unwrap().set_partner(GIRLFRIEND);
        r.player_mut(man).unwrap().set_children(2);
        assert!(first_months(&mut r, club, &mut posted, &mut Random::new(away)));
        assert_eq!(posted.len(), 1);

        // Category 23: he is 18 in the start year with a spring
        // birthday, all the others grown men; the planner of his club's
        // human has no appointment before today.
        for &id in real.iter().filter(|&&id| id != man) {
            r.player_mut(id).unwrap().set_birth_year(start - 30);
        }
        let p = r.player_mut(man).unwrap();
        p.set_birth_month(3);
        p.set_birth_year(start - 18);
        p.set_school(0);
        let mut parts = p.contentment();
        parts[DISTRACTION] = 100;
        p.set_contentment(parts);
        let human = r.team(club.0, club.1).unwrap().human();
        assert_ne!(human, 0, "the club knows its human");
        let slot = usize::from(human) - 1;
        let today = usize::from(r.globals.day());
        for d in 0..=today {
            r.planners[slot].set_entry(d, [0; crate::records::PLANNER_ENTRY]);
        }
        let trouble = seed(&|r| r.below(5) == 2);
        let burden = seed(&|r| r.below(5) == 0);
        let abitur = seed(&|r| r.below(5) == 4);
        // A talk booked today: untold, no draw.
        let mut entry = [0u8; crate::records::PLANNER_ENTRY];
        entry[0] = TALK_KIND;
        entry[4] = 1;
        r.planners[slot].set_entry(today, entry);
        let mut random = Random::new(trouble);
        assert!(school_days(&mut r, club, &mut posted, &mut random));
        assert!(random == Random::new(trouble) && posted.len() == 1);
        r.planners[slot].set_entry(today, [0; crate::records::PLANNER_ENTRY]);
        // The teacher who hates football, his standing from 0 to 1.
        let mut random = Random::new(trouble);
        assert!(school_days(&mut r, club, &mut posted, &mut random));
        assert_eq!(posted[1], Posted { club, code: PRIVATE_NEWS, text: 0x61b, fill: Fill::NumberedOne(man) });
        let mut expected = Random::new(trouble);
        expected.below(5);
        assert!(random == expected);
        assert_eq!((r.player(man).unwrap().school() >> 3) & 3, 1);
        // The school's burden: four off his distractions.
        assert!(school_days(&mut r, club, &mut posted, &mut Random::new(burden)));
        assert_eq!(posted[2], Posted { club, code: PRIVATE_NEWS, text: 0x619, fill: Fill::NumberedOne(man) });
        assert_eq!(r.player(man).unwrap().contentment()[DISTRACTION], 96);
        // The teacher's trouble once more: not while he stands at 1.
        assert!(school_days(&mut r, club, &mut posted, &mut Random::new(trouble)));
        assert_eq!(posted.len(), 3);
        // The burden of a man the teacher backs: nothing.
        r.player_mut(man).unwrap().set_school(2 << 3);
        assert!(school_days(&mut r, club, &mut posted, &mut Random::new(burden)));
        assert_eq!(posted.len(), 3);
        // The Abitur in June, and in September nothing.
        r.globals.set_date([day, 6, year]);
        assert!(school_days(&mut r, club, &mut posted, &mut Random::new(abitur)));
        assert_eq!(posted[3], Posted { club, code: PRIVATE_NEWS, text: 0x61d, fill: Fill::NumberedOne(man) });
        r.globals.set_date([day, 9, year]);
        assert!(school_days(&mut r, club, &mut posted, &mut Random::new(abitur)));
        assert_eq!(posted.len(), 4);
        // Done with school: he is no schoolboy, the news goes on.
        r.player_mut(man).unwrap().set_school(3 << 3);
        let mut random = Random::new(abitur);
        assert!(!school_days(&mut r, club, &mut posted, &mut random));
        assert!(random == Random::new(abitur));
    }

    /// Category 24 on A's human club, whose league is made to play 20
    /// matchdays: with two million at the club's interpreter staff every
    /// man not marked yet escapes on his roll of a hundred — his land's
    /// bit at +0x216 set, the language part of +0x27f cleared — and he
    /// stands in no list, the news going on. With the staff unpaid only
    /// the nought of the hundred escapes. A man of 30, rooted and of 0x5b
    /// morale then stands among the homesick, a man of 20, no league
    /// matchday and far from pleased with the team among the young; 13
    /// months at the club, of his club's own country and no word of its
    /// language, two tales stand on the homesick man, and 29 times in
    /// thirty one of them is drawn and marks him talked about, the news
    /// ending untold. The thirtieth time the young man, much weaker, not
    /// shut out and no dislike of him, takes him along: ten on the
    /// homesick man's contentment with the team, and the news goes on.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn category_24_marks_the_far_from_home_and_pairs_two() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let mut r = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let (hc, hn) = crate::statistics::human_clubs(&r)[0];
        let club = (u16::from(hc), hn as u16);
        let land = r.team(club.0, club.1).unwrap().country();
        let real: Vec<u16> = r
            .team(club.0, club.1)
            .unwrap()
            .players()
            .into_iter()
            .filter(|&id| id <= LAST_REAL_ID)
            .collect();
        let (old, lad) = (real[0], real[1]);
        let seed = |keep: &dyn Fn(&mut Random) -> bool| (0..2_000_000u64).find(|&s| keep(&mut Random::new(s))).unwrap();
        let [_, month, year] = r.globals.date();
        let bit = 1u16 << (FOREIGN_SEEN[usize::from(land)] & 0x1f);

        // Paid staff: two million, one man in a hundred's roll for every
        // ten thousand of it, so every man not marked yet is marked.
        for &id in &real {
            let p = r.player_mut(id).unwrap();
            p.put::<u16>(FOREIGN_SEEN_AT, p.get::<u16>(FOREIGN_SEEN_AT) & !bit);
        }
        r.team_mut(club.0, club.1).unwrap().put::<i32>(INTERPRETER, 2_000_000);
        let mut random = Random::new(1);
        assert!(!far_from_home(&mut r, club, &[], &mut random));
        for &id in &real {
            let p = r.player(id).unwrap();
            assert_eq!(p.get::<u16>(FOREIGN_SEEN_AT) & bit, bit, "man {id} marked");
            assert_eq!(p.problems() >> 2 & 3, 0, "man {id} speaks the land's language");
        }
        let mut expected = Random::new(1);
        for _ in 0..real.len() {
            expected.below(100);
        }
        assert!(random == expected);

        // Nothing paid: the club's league has played 20 matchdays (the
        // block at 0x1a09d48, its word +0x16), and every other man stays
        // out of both lists, without a morale and without a matchday.
        r.team_mut(club.0, club.1).unwrap().put::<i32>(INTERPRETER, 0);
        let team = r.team(club.0, club.1).unwrap();
        let (country, level) = (team.country(), team.get::<u8>(0x12e));
        let li = r.countries.first_league(usize::from(country)) + i16::from(level);
        r.block_mut(&format!("0x1a09d48[{li}]")).unwrap()
            [crate::opponent::LEAGUE_FIELDS + 0x16..][..2]
            .copy_from_slice(&20i16.to_le_bytes());
        for &id in real.iter().filter(|&&id| id != old && id != lad) {
            let p = r.player_mut(id).unwrap();
            p.set_league_matches(0xff);
            p.set_morale(0);
        }
        // The homesick man: 30, rooted, of good morale, 13 months at the
        // club on one station, of his club's own country, no word of its
        // language and nothing he has been talked about.
        let p = r.player_mut(old).unwrap();
        p.put::<u16>(FOREIGN_SEEN_AT, p.get::<u16>(FOREIGN_SEEN_AT) & !bit);
        p.set_flags(0);
        p.set_birth_day(1);
        p.set_birth_month(1);
        p.set_birth_year(year - 30);
        p.set_career_count(1);
        p.put::<u8>(0x20, 1);
        p.put::<u8>(0x21, month as u8);
        p.put::<u16>(0x22, year - 1);
        p.set_nation(land);
        p.set_problems(p.problems() & LANG_CLEAR);
        p.set_shown(p.shown() & !(TALKED | SHUT_OUT));
        let mut traits = p.traits();
        traits[1] |= ROOTED;
        p.set_traits(traits);
        p.set_morale(ROOTED_MORALE);
        p.set_strength(100);
        p.set_dislikes([0; 3]);
        // His other parts please him, so the morale the homesick measure
        // wants still stands after the young man lifts the team part.
        p.set_contentment([120, 60, 120, 120, 120, 120, 120]);
        // The young man: 20, no league matchday, far from pleased with
        // the team, and marked already — so no tale stands on him.
        let p = r.player_mut(lad).unwrap();
        p.set_flags(0);
        p.set_birth_day(1);
        p.set_birth_month(1);
        p.set_birth_year(year - 20);
        p.put::<u16>(FOREIGN_SEEN_AT, p.get::<u16>(FOREIGN_SEEN_AT) | bit);
        p.set_league_matches(0);
        p.set_strength(50);
        p.set_shown(p.shown() & !(TALKED | SHUT_OUT));
        let mut parts = p.contentment();
        parts[TEAM] = 20;
        p.set_contentment(parts);
        assert_eq!(months_at_the_club(&r, &r.player(old).unwrap()), 13);
        assert_eq!(crate::league::matchdays(&r, &r.team(club.0, club.1).unwrap()), 20);
        assert_eq!(tales_of(&r, r.player(old).unwrap(), land, 0, 0, 13, &[], &mut Random::new(1)), [0, 3]);

        // 29 times in thirty one of the two tales is drawn and marks its
        // man talked about; the news ends, the tale itself untold.
        let drawn = seed(&|rnd| rnd.below(100) != 0 && rnd.below(30) != 0);
        let mut random = Random::new(drawn);
        assert!(far_from_home(&mut r, club, &[], &mut random));
        assert_ne!(r.player(old).unwrap().shown() & TALKED, 0);
        let mut expected = Random::new(drawn);
        expected.below(100);
        expected.below(30);
        expected.below(2);
        assert!(random == expected);

        // The thirtieth time the two lists are paired: the young man much
        // weaker, not shut out and no dislike of the homesick one.
        let pair = seed(&|rnd| rnd.below(100) != 0 && rnd.below(30) == 0);
        let before = crate::mood::parts(r.player(old).unwrap())[TEAM];
        let mut random = Random::new(pair);
        assert!(!far_from_home(&mut r, club, &[], &mut random));
        assert_eq!(crate::mood::parts(r.player(old).unwrap())[TEAM], before + COMPANY);
        let mut expected = Random::new(pair);
        expected.below(100);
        expected.below(30);
        assert!(random == expected);
        // A dislike of him, a man shut out, a young man of his own
        // strength: nothing comes of the pairing.
        r.player_mut(old).unwrap().set_dislikes([lad, 0, 0]);
        assert!(!far_from_home(&mut r, club, &[], &mut Random::new(pair)));
        r.player_mut(old).unwrap().set_dislikes([0; 3]);
        let shut = r.player(lad).unwrap().shown() | SHUT_OUT;
        r.player_mut(lad).unwrap().set_shown(shut);
        assert!(!far_from_home(&mut r, club, &[], &mut Random::new(pair)));
        let open = r.player(lad).unwrap().shown() & !SHUT_OUT;
        r.player_mut(lad).unwrap().set_shown(open);
        r.player_mut(lad).unwrap().set_strength(100);
        assert!(!far_from_home(&mut r, club, &[], &mut Random::new(pair)));
        assert_eq!(crate::mood::parts(r.player(old).unwrap())[TEAM], before + COMPANY);

        // The nought of the hundred escapes after all: marked, in no
        // list, and the news goes on.
        let escape = seed(&|rnd| rnd.below(100) == 0);
        let mut random = Random::new(escape);
        assert!(!far_from_home(&mut r, club, &[], &mut random));
        assert_eq!(r.player(old).unwrap().get::<u16>(FOREIGN_SEEN_AT) & bit, bit);
        let mut expected = Random::new(escape);
        expected.below(100);
        assert!(random == expected);
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
