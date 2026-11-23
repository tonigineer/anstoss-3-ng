//! The original's match engine: the simulation behind a match, ported from
//! the decompile. It walks the day's matches minute by minute — the root is
//! `FUN_006f352e`, each minute runs `FUN_006f2d4f`, and `FUN_006f4744`
//! recomputes both sides' rated strengths from their sides' tactics, mood
//! and fatigue every minute. The match itself is a 0x498-byte record; its
//! two sides' blocks sit at +0x5c and +0x250, and this module's `Match` and
//! `Side` name the record's fields, keeping the original's offsets in the
//! comments.
//!
//! This is the engine's skeleton: the day's pre-match walk (`pre_match`,
//! a plain fixture at chance slot and added byte 5, no referee), the
//! minute walk, the added time, full time's extra-time and penalty
//! decisions, and the rated strengths. A
//! club minute then runs its chance frame (`FUN_006f6007`): the chooser of
//! the minute (`FUN_006f6857`), the two sides' chance rolls out of their
//! rated attack pair, the shot's quality (`FUN_0040b8c9`) and the trailing
//! side's morale talk (`FUN_0041321d`). From the frame's kinds the set
//! piece and the big chance already land: the taker is picked out of the
//! side's men (`FUN_005ded54`, `FUN_005def60`), and a goal moves the
//! score and the minute's mood as the original writes it (`FUN_007410ca`,
//! `FUN_007411ea`). A tie that extra time left level settles on penalties
//! too: the walk's penalty minute calls `penalty_shootout` (`FUN_005ddea7`
//! — the coin, the five takers a side draws out of its men, the sudden
//! death and the kick's roll) over the sides' men as the records keep
//! them, and draws nothing without the save. When the walk is over the
//! day's plain wear pass (`FUN_006f8f44` with the fifth parameter nought)
//! lands on the played men: `play_matches` runs `post_match_wear` over
//! each match the walk ran — and with it `post_match_books`, which writes
//! the assists and own goals the events left on the sides' waiting lists
//! — and neither draws a number. The chances'
//! rest lands as seams — the quality-one shot's own goal
//! (`FUN_00659d2b` and its eleven siblings, the scorer pick
//! `FUN_0077db3a`), the national-side path's set pieces (`FUN_006f543f`)
//! and open play (`FUN_006f562b`) and the talk's staff question
//! (`FUN_004134d0`) — called from the places the original calls them. The
//! original's presentation (the 3D match, its commentary and clock
//! displays: `FUN_0073f1a6` and the `DAT_01bad050`/`01bad060` displays) has
//! no replica and is left out.
//!
//! Before the minutes the kick-off (`kick_off_strengths`, the pre-match
//! walk's `FUN_006f6fc7`) reckons both sides' strengths from their
//! elevens; the clubs' tactics laid behind it wait. `matches::play_day`
//! walks the day's listed league matches through it.

use chrono::NaiveDate;

use crate::duties::{self, Duty};
use crate::random::Random;
use crate::individual::IndividualTables;
use crate::records::{Player, Records};
use crate::training_day;
use crate::{statistics, tactics, world};

/// The record's flag byte at +0x1d.
/// The match is over; its score stands and the minute walk passes it by.
pub const FINISHED: u8 = 1;
/// The minute has reached this match's end of half (`ends_at`).
pub const MINUTE_UP: u8 = 0x10;
/// Full time; no more play.
pub const OVER: u8 = 0x20;
/// The match goes to penalties.
pub const PENS: u8 = 0x40;
/// The match goes to (or is in) extra time.
pub const EXTRA: u8 = 0x80;

/// The record's kind at +0x20: this fixture can go to extra time.
pub const MAY_EXTRA: u32 = 0x80_0000;
/// Two legs home and away; the tie is decided over both.
pub const TWO_LEGS: u32 = 0x8000_0000;

/// What the minute's strength is taken of, per the side's stance byte at
/// +0x8e/+0x282 (`DAT_008a6f90`) — also added to fatigue every minute, see
/// `minute_play`.
const MINUTE_PCT: [i32; 8] = [110, 107, 105, 103, 100, 95, 90, 105];
/// The side's tactic byte at +0x8c/+0x280 into the tilt of the rated
/// strengths (`DAT_008a6f58`): the share of strength 5 that comes forward,
/// and its mirror (`DAT_008a6f74`) on strength 2. A flat 0 sits at index 3.
const TILT_ON: [i32; 7] = [22, 14, 8, 0, -21, -36, -60];
const TILT_BACK: [i32; 7] = [-60, -36, -21, 0, 8, 14, 22];
/// The side's mental byte at +0x7a/+0x26e, a percent on attack and on
/// rated strength 5 (`DAT_008a6ee8`).
const MENTAL: [i32; 5] = [0, 2, 1, -2, -3];
/// The style bit at +0x8a/+0x27e whose minute window lifts attack and
/// support by a fifth (`FUN_006f9788`); its name stays tentative until the
/// screen that sets it is ported.
const PRESS_BIT: u16 = 0x10;
/// The good word's (record 0x1c4) bit worth a hundred to the big chance's
/// taker (`FUN_005ded54`); its meaning waits with the screen that hands
/// out abilities.
const BIG_WORD: u16 = 0x10;
/// The free-kick ability (`duties::FREE_KICKS`) worth the same to the set
/// piece's taker (`FUN_005def60`).
const FREE_KICKS: u16 = 0x20;
/// The words' bit of holding the line, read off the shootout keeper
/// (`FUN_005ddcc5`): a keeper whose bad word marks it lets in fifteen
/// more from the spot, whose good word fifteen less; its name waits with
/// the screen that hands out abilities, as `BIG_WORD`'s does.
const KEEPER_GIFT: u16 = 1;
/// The own-goal danger by position kind (`DAT_008acc6c`, the .data table
/// read from the exe): the keeper stands at nought, the full backs and
/// half-backs most at risk, the forward least (`FUN_0077e3e8`).
const OWN_GOAL_W: [i32; 11] = [0, 0, 2, 5, 5, 4, 4, 2, 2, 3, 1];
/// The record's ledger slots per side — the eighteen men it enters, eleven
/// on the pitch and seven at the reserve's place.
const LEDGER_SLOTS: usize = 0x12;
/// The ledger bits `book_event` (`FUN_0065847f`) keeps per entered man: a
/// shot (`FUN_00740996`) and an own goal (`FUN_00740d68`). The halftime
/// talk (`FUN_0065d044`) adds bit 8 for a man unhurt in the record's mark
/// at +0xc3; bits 0 and 1 its `FUN_00658583` reads as "entered, no shot
/// taken" have no writer found in the corpus.
const LEDGER_SHOT: u8 = 2;
const LEDGER_OWN: u8 = 4;
/// The post-match wear multipliers (`DAT_008a6f4c`, peeked from the .bss
/// table block beside `MENTAL`), indexed by the side's pressure band.
const WEAR_MULT: [i32; 5] = [8, 9, 10, 11, 12];
/// What a played match writes per minute band, held times ten and scaled
/// by the multiplier over ten (`DAT_008a6efc`): the condition, then the
/// three freshness values — a full match lifts the man's condition a
/// little and drains his freshness well below it, the shorter bands lift
/// and drain less. `FUN_006f8b3f` takes the sibling
/// rows [`FRIENDLY_WEAR_ROWS`] when the record's short at +0x24 is not
/// nought, as the friendlies' and tournaments' walk (`FUN_006f3b7a`) has
/// it; the competitions' walks answer with these.
const WEAR_ROWS: [[i32; 4]; 5] = [
    [10, -30, -60, -25],
    [40, -90, -270, -135],
    [70, -162, -450, -225],
    [100, -225, -630, -315],
    [120, -340, -810, -405],
];
/// The rows a friendly or a tournament's game writes (`DAT_008a6f24`),
/// as [`WEAR_ROWS`]: the condition barely lifted, the freshness drained
/// about half as much.
const FRIENDLY_WEAR_ROWS: [[i32; 4]; 5] = [
    [0, -18, -36, -18],
    [20, -45, -135, -72],
    [40, -80, -225, -108],
    [50, -125, -315, -153],
    [60, -160, -670, -315],
];
/// A shootout's regulation round lists five takers a side
/// (`FUN_005ddea7`'s `FUN_005de669(club, list, 0, 5)` on each side).
const SHOOTOUT_TAKERS: usize = 5;
/// Its taker lists hold a slot per man on the pitch — eleven — the
/// sudden death adding one per round at the round's place wrapped round
/// the side's men (`Side::men`).
const SHOOTOUT_SLOTS: usize = 0xb;
/// Past twelve goals the sudden death stops rolling the dice — weighed
/// over the record's goals plus the side's shootout score, which counts
/// the shootout's own goals twice over, since its goals land in both
/// tallies; the original's own arithmetic, kept as it stands.
const SHOOTOUT_PAST: i32 = 0xc;

/// One side of a match: the record's side block at +0x5c (first side) or
/// +0x250 (second). The offsets in the comments are from the block's start.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Side {
    /// +0x00,+0x02,+0x04,+0x06,+0x08: the five rated strengths, in tenths.
    /// The engine names them only by these offsets; strength 4 feeds attack
    /// and strength 3 support, and 1 and 5 trade under a tilted tactic.
    pub strengths: [i16; 5],
    /// +0x0a (+0x66/+0x25a): the standing a goal counts against the foe's,
    /// a sixth of the two apart added to the minute's mood (`FUN_007410ca`);
    /// the kick-off lays the club's will to win there, tenfold
    /// (`FUN_006f7b67`, [`kick_off_strengths`]).
    pub nerve: i16,
    /// +0x0c: the mood of the minute, `minute_play` sinks it when the side
    /// trails through the half.
    pub morale: i16,
    /// +0x0e..+0x16: recomputed every minute from strengths, morale,
    /// stance and tactic; the chances read these.
    pub rated: [i16; 5],
    /// +0x1c: what the stance's percentage is taken of — a hundred from
    /// the kick-off, less for a favourite taking the match lightly
    /// ([`kick_off_strengths`]).
    pub strength: i16,
    /// +0x1e: index into `MENTAL`.
    pub mental: i16,
    /// +0x2c: fatigue, the sum of the minutes' percentages.
    pub fatigue: i16,
    /// +0x2e: the side's style bits; bits 9+8 (0x300) and 7+9+8 (0x380)
    /// single out play against the foe's attack, `PRESS_BIT` opens a minute
    /// window.
    pub style: u16,
    /// +0x30: index into `TILT_ON`/`TILT_BACK`.
    pub tilt: u8,
    /// +0x32: index into `MINUTE_PCT`.
    pub stance: u8,
    /// +0x31 (record +0x8d/+0x281): the side's chance tier; its five cases
    /// trade class against chance in `chance_chooser`, and a byte past 4
    /// leaves the side no chance roll at all.
    pub tier: u8,
    /// +0x33 (+0x8f/+0x283): the side's class, a signed tally of the
    /// squad's carrying players the pre-match walk sums up; the tier's
    /// multiplier shrinks by it.
    pub aggr: i8,
    /// +0x34 (+0x90/+0x284): how often this side's trailing talk has run.
    pub talks: u8,
    /// +0x35 (+0x91/+0x285): the men standing on the side's pitch. The
    /// pre-match squad walk — its third loop, summing over the eleven
    /// formation slots — sets eleven, and the sending-off events take one
    /// away with the man's slot in the club's formation; those wait, so
    /// the count stands whole. The shootout wraps its sudden-death takers
    /// round it.
    pub men: u8,
    /// +0x39 (+0x95/+0x289): the count of the side's plain chances.
    pub chances: u8,
    /// +0x3d (+0x99/+0x28d): the straight reds whose kind came from the
    /// other table (`FUN_006748ea`, see `straight_red`).
    pub other_reds: u8,
    /// +0x43 (+0x9f/+0x293): the side's bookings of the foul kinds 4 and
    /// 12, a first card lifting it (`FUN_0065a6b0`, `FUN_0065ab3a`).
    pub hard_fouls: u8,
    /// +0x4e (+0xaa/+0x29e): bit 0x10 stands once a man went off for a
    /// foul of the kinds 4 and 12.
    pub foul_marks: u16,
    /// +0xd4,+0xd6 (+0x130,+0x132/+0x324,+0x326): the side's first cards,
    /// both counters lifted together.
    pub cautions: [i16; 2],
    /// +0xd8 (+0x134/+0x328): bit 1 raised by the side's first card.
    pub caution_bits: u16,
    /// +0xdc (+0x138/+0x32c): the side's sendings-off.
    pub reds: i16,
    /// +0xde (+0x13a/+0x32e): the side's straight reds, the chance
    /// frame's roll 699 (`straight_red`); the second card counts at +0xdc.
    pub straight_reds: i16,
    /// The side's cautioned men in order — the live player's +0x57 bit 8
    /// (record +0x25b) the booking raises at the moment; the replica
    /// keeps no live objects, so the names wait here.
    pub booked: [u16; LEDGER_SLOTS],
    /// The side's men sent off in order — the live +0x57 bit 0x10 (0x20
    /// for a straight red), and
    /// the man's place in the club's formation +0xa9a cleared; the
    /// eleven the events read (`pitch_eleven`) leaves these places
    /// empty.
    pub sent_off: [u16; LEDGER_SLOTS],
    /// +0xb2 (+0x10e/+0x302): how many talks the side has left.
    pub talk_max: i16,
    /// +0xb4 (+0x110/+0x304): the side's shooting strength, `shot_quality`.
    pub shot_pwr: i16,
    /// +0xf2 (+0x14e/+0x342): the side's attitude into the match; the
    /// distance of the two sides' attitudes sets `shot_quality`'s odds.
    pub attitude: i16,
    /// +0xf4 (+0x150/+0x344): whether the side's staff can answer a talk;
    /// the pre-match walk zeroes it when the club has no assistant.
    pub assistant: i16,
    /// +0x3b,+0x3c: the press's last and first minute, only read under
    /// `PRESS_BIT`.
    pub press_to: u8,
    pub press_from: u8,
    /// +0xf6 (record +0x152/+0x346): set while the press window is open.
    pub pressed: u16,
    /// +0xfa (record +0x156/+0x34a): the side's entered men, short ids.
    /// The original copies these from its line-up (runtime +0x8e, the
    /// manager's or the co-trainer's eleven and bench, `line_up_row`);
    /// the walk enters the saved formation when the team record lists
    /// one and its own eleven otherwise (`fill_entries`), the reserve
    /// places nought with the stand-in.
    pub entries: [u16; LEDGER_SLOTS],
    /// +0x1c0 (record +0x21c/+0x410): one ledger byte per entered man,
    /// read together with `entries` by `book_event`, `own_goal_mark` and
    /// the halftime talk's dialog.
    pub ledger: [u8; LEDGER_SLOTS],
    /// The side's goal assisters in order — the men the original hands to
    /// `FUN_00740dc4`, which lifts their live player objects' two assist
    /// bytes +0x48 and +0x4b at the goal's moment. The replica keeps no
    /// live objects, so the ids wait in this list until
    /// `post_match_books` writes the records at the walk's end.
    pub assist_book: [u16; LEDGER_SLOTS],
    /// The side's men blamed for an own goal — `FUN_00740d68`'s man,
    /// whose live byte +0x47 takes the goal; the same men the ledger's
    /// own-goal bit marks.
    pub own_book: [u16; LEDGER_SLOTS],
    /// The side's takers of big chances and set pieces in order — every
    /// shot book (`FUN_00740996`, `FUN_00740a07`) the original follows
    /// with `FUN_00740a66`, which lifts the taker's live byte +0x52 at
    /// the chance's moment, the save's chances counter at +0x251; the
    /// replica waits these names for `post_match_books` as well.
    pub chance_book: [u16; LEDGER_SLOTS],
    /// The side's takers whose big chances went wide — the original's
    /// miss branch calls `FUN_00740f33` on the taker, lifting his byte
    /// +0x4c, the record's first penalties byte at +0x24b (the season
    /// screen reads the pair at +0x24b/+0x24c as "scored of taken");
    /// the replica waits these names for `post_match_books` as well —
    /// a second lift on top of the shot book below, so the wide
    /// penalty's taken byte stands moved twice.
    pub penalty_book: [u16; LEDGER_SLOTS],
    /// The side's shot takers in order, each with the shot's name:
    /// true the big chance's penalty, false the set piece's free kick.
    /// The original's shot books themselves lift counters:
    /// `FUN_00740996` at the big chance's open, before the outcome's
    /// roll, lifts the taker's taken penalty byte at live +0x4c — the
    /// record's first byte of the pair at +0x24b — and the attacking
    /// club's counter at live +0xdea; `FUN_00740a07` at the set
    /// piece's open his free-kick byte at live +0x4e, the record's
    /// +0x24d, and the club's +0xdec. The replica keeps no live man
    /// or club at that moment, so the pairs wait here for
    /// `post_match_books`; the big chance's ledger mark, the same
    /// book's `FUN_0065847f`, stands booked already.
    pub shot_book: [(u16, bool); LEDGER_SLOTS],
    /// The side's named scorers in order, each with his shot's kind as
    /// `FUN_00740aad`'s first place names it: 0 the big chance's penalty,
    /// 1 the set piece, 2 a headed goal, 3 a struck one, and -1 the
    /// quality-one shot that went wide or saved yet still named its man.
    /// The original lifts the man's goal bytes, kinds and marks at the
    /// moment; the replica keeps no live objects, so the pairs wait here
    /// for `post_match_books`.
    pub goal_book: [(u16, i16); LEDGER_SLOTS],
    /// The foe keepers of this side's big chances in order, each with
    /// the shot's outcome: true saved, false conceded. The original
    /// reads the defending club's line-up block — its first short, the
    /// keeper — and lifts his live saved byte +0x50 in the miss branch
    /// (`FUN_00741038`) or his conceded byte +0x51 in the goal branch
    /// (`FUN_00740fb6`); the replica keeps no live objects, so the
    /// pairs wait here for `post_match_books`.
    pub keeper_book: [(u16, bool); LEDGER_SLOTS],
    /// +0x20..+0x2a: the six line shorts the scorer pick's factors read
    /// (`FUN_0077e590`): the defensive and the attacking midfield's parts
    /// on the three wings, pairwise (`FUN_006a1bcf`).
    pub lines: [i16; 6],
    /// +0xc6..+0xcc (+0x122../+0x316..): the side's strength on the day —
    /// the five strengths' sum, the club's +0xdf2 —, its attack, its
    /// midfield and its defence, each a tenth of the strengths it sums
    /// (`FUN_006f6fc7`).
    pub day: [i16; 4],
}

/// A match as the original keeps it: the 0x498-byte record, one of the
/// day's row. Offsets are from the record's start.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Match {
    /// +0x00,+0x02 and +0x04,+0x06: the two sides' pair of ids;
    /// `FUN_00677428` reads them to tell national sides from clubs.
    pub ids: [u16; 4],
    /// +0x0e,+0x10: the goals so far.
    pub goals: [i16; 2],
    /// +0x08,+0x09: the first leg's goals — +0x09 counts with +0x0e, +0x08
    /// with +0x10.
    pub leg: [u8; 2],
    /// +0x12,+0x13: the score at half time, which the break copies from
    /// the goals (`FUN_0064987a`) and the settlement reads as the half's.
    pub half: [u8; 2],
    /// +0x14..+0x1b: the score at the clock's marks — 90, 10, 75, 85 —
    /// each kept in two bytes, one per side.
    pub score90: [u8; 2],
    pub score10: [u8; 2],
    pub score75: [u8; 2],
    pub score85: [u8; 2],
    /// +0x1c: the half's score packed, first side in the high nibble.
    pub half_score: i8,
    /// +0x1d: the flags above.
    pub flags: u8,
    /// +0x20: the kind bits above.
    pub kind: u32,
    /// +0x28,+0x2a: the ground's nation byte and owning id; the chance
    /// frame gives the first side's attack an eighth more when they match
    /// its own pair (`FUN_006782e4`), or its nation byte alone when the
    /// side's second id sits at 200.
    pub home_nation: u8,
    pub home_id: u16,
    /// The two sides, +0x5c and +0x250.
    pub sides: [Side; 2],
    /// +0x240,+0x434: what the last minute's booking left for the frame to
    /// pass on (`FUN_00413115`); both cleared after.
    pub cards: [u16; 2],
    /// +0x444: what side one leads by — at the kick-off twice its
    /// eleven's lead in condition, within 45 (`FUN_006f6c35`); its sign
    /// decides which side's mood sinks as the minutes of a lost half run
    /// on.
    pub advantage: i16,
    /// +0x446: a percent on the leading side's attack and hold, see
    /// `minute_play` — the crowd's lean the walk's third loop leaves
    /// ([`crowd_lean`]), below nought for the home side.
    pub form: i16,
    /// +0x448: the minute this match's half ends at; the walk's threshold,
    /// floored at 45, 90 and 120 by the half.
    pub ends_at: i16,
    /// +0x450: the chance slot the minute's chooser draws its step out of
    /// (`chance_chooser`); the pre-match walk leaves a plain fixture at 5.
    pub chance_slot: u8,
    /// +0x451: byte 6 down by two places selects the added time's draw.
    pub added_draw: u8,
    /// +0x458: the referee, a record index; the pre-match walk draws one
    /// only for a national tie and leaves a plain fixture at 0xffff.
    pub referee: i16,
    /// +0x45a: the bookings both sides took in the match (`FUN_0065a6b0`).
    pub bookings: i16,
    /// +0x468: this match's place in the day's row, set by the pre-match
    /// walk.
    pub index: u16,
    /// +0x474: the national marker — a side standing in the saved national
    /// teams raises bit 1 or 2 (`FUN_00677428`), in the regional
    /// selections bit 4 or 8 (`FUN_0067764c`); the two sides' places in
    /// the national list sit in the short's upper bytes (`FUN_00677574`,
    /// read three bits each), and the walk's third loop numbers the day's
    /// national ties from bit 4 up. A plain club fixture stays at nought;
    /// a tie carrying a national side is simulated in bulk at pre-match
    /// (`FUN_0070c334`) and joins the minute walk only when the whole day
    /// runs watched.
    pub nat_bits: u16,
    /// +0x244,+0x438: the bonus each side plays for, the club's +0xb0,
    /// which the pre-match walk lays only after the kick-off strengths
    /// (its sixth loop); the row is cleared before (`FUN_0062a325`), so
    /// the kick-off reads nought here.
    pub bonus: [i32; 2],
    /// +0x460: the match the day's listing marked for the doping control
    /// (`FUN_00690a9e`'s throw of a league's pairs).
    pub doping: bool,
    /// The men the control picked on each side before the match
    /// (`FUN_005dad80`, which marks the live man's +0x57 bit 0x40; the
    /// replica keeps no live men, so their ids wait here for the
    /// settlement's test), none 0.
    pub picked: [[u16; 2]; 2],
    /// The crowd the walk's third loop reckons for a league fixture
    /// (`FUN_0078b26c`'s locals, before the stadium takes them): the home
    /// fans and the visitors.
    pub fans: [i32; 2],
    /// +0x24: a game of the day's own list — a friendly (`FUN_00625310`
    /// lays 1) or a tournament's. The plain pass then wears its men by
    /// the friendly rows ([`FRIENDLY_WEAR_ROWS`]), no graded pass follows
    /// and the scorers' books stand off ([`post_match_books`]).
    pub exhibition: bool,
}

/// The day's clock, `DAT_01bad044` and its state globals: minute (+0xe),
/// extra-time state (+0x10: none, extra time, penalties) and second-half
/// flag (+0x12).
#[derive(Clone, Copy, Default, PartialEq)]
struct Clock {
    minute: i16,
    extra: i16,
    half2: u16,
}

/// Play the day's matches minute by minute, as `FUN_006f352e` does: both
/// halves, added time, extra time and penalties when the tie needs them.
/// `live` is the original's flag for a day whose matches are watched; the
/// replica plays all its matches to themselves, so it passes false with
/// every `Match::nat_bits` zero — a tie carrying a national side is
/// simulated in bulk at pre-match (`FUN_0070c334`) and joins the walk
/// only on a watched day.
///
/// All the day's matches walk together, one minute at a time, and the
/// added time is drawn for every match of the row — the original draws
/// from the one generator, so the order of these draws is part of the
/// port. `records` is the save's, so the set pieces and big chances can
/// find their takers among the sides' men and the penalty minute can
/// stand its kicks on them; without it those events come to nothing and
/// neither the pickers nor the kicks draw a number. With them the walk
/// also enters both sides' men in the record's slots before its first
/// minute, so the events' books find their places. `today` is the day
/// the fixtures stand on; the shootout reads the men's years against it.
/// The records stand mutable for the wear pass the day loop runs over
/// each match of the row once the minutes have walked
/// (`FUN_006f8f44` with the fifth parameter nought); the graded pass and
/// the clubs' ledgers in that loop (`FUN_00714236`, `FUN_00744d02`) wait
/// with the club objects. `lessons`, the individual training's tables,
/// lets the goals, assists and saved penalties teach the human clubs'
/// men their abilities as well ([`post_match_lessons`]).
pub fn play_matches(
    matches: &mut [Match],
    live: bool,
    mut records: Option<&mut Records>,
    lessons: Option<&IndividualTables>,
    today: NaiveDate,
    random: &mut Random,
) {
    let mut clock = Clock::default();
    if let Some(records) = records.as_deref() {
        for m in matches.iter_mut() {
            for which in 0..2 {
                fill_entries(m, which, records);
            }
        }
    }
    let mut minute = 1;
    while minute < 0x31 && tick(matches, live, minute, &mut clock, records.as_deref_mut(), today, random)
    {
        minute += 1;
    }
    clock.minute = 45;
    // Half time also packs the half's events (FUN_006f85b7 for the live
    // match) and records the scorers so far (FUN_0064987a) — not ported
    // but for the half's score, copied over the whole row (+0x12/+0x13;
    // the original skips the copy on a day `FUN_006ef262` shows no
    // break, whose half nothing reads).
    for m in matches.iter_mut() {
        m.half = [m.goals[0] as u8, m.goals[1] as u8];
    }
    for m in matches.iter_mut() {
        m.ends_at = added_time(m, random) + 90;
        m.flags &= !MINUTE_UP;
    }
    clock.half2 = 1;
    let mut minute = 46;
    while minute < 99 && tick(matches, live, minute, &mut clock, records.as_deref_mut(), today, random) {
        minute += 1;
    }
    clock.minute = 90;
    if time_out(matches, live) {
        for m in matches.iter_mut() {
            m.ends_at = added_time(m, random) + 120;
        }
        clock.half2 = 0;
        clock.extra = 1;
        let mut minute = 90;
        while minute < 0x7e && tick(matches, live, minute, &mut clock, records.as_deref_mut(), today, random) {
            minute += 1;
        }
        if after_extra_time(matches, live) {
            clock.extra = 2;
            tick(matches, live, 0x78, &mut clock, records.as_deref_mut(), today, random);
        }
    }
    // The day loop's per-match passes: what the ninety minutes leave on
    // the men who stood the pitch, and what their goals leave in the
    // assist and own-goal counters. They run on the walk's own matches,
    // the way the loop walks the row after `FUN_006f352e` ended it; the
    // national ties bulk-simulated at pre-match wait with their lists.
    // Only the grades draw, one number a graded man.
    if let Some(records) = records.as_deref_mut() {
        for m in matches.iter() {
            if m.exhibition {
                // The friendlies' walk ends on the plain pass alone
                // (`FUN_006f3b7a`'s last loop, `FUN_006f8f44` with the
                // fifth parameter nought).
                exhibition_wear(m, records);
            } else if live || m.nat_bits == 0 {
                // The men's form as the kick-off took it (runtime +0xb7).
                let kickoff: Vec<Vec<(u16, i32)>> = (0..2)
                    .map(|w| {
                        saved_eleven(m, w, records)
                            .into_iter()
                            .filter_map(|id| records.player(id).map(|p| (id, statistics::form_today(p))))
                            .collect()
                    })
                    .collect();
                post_match_wear(m, records);
                for (which, form) in kickoff.iter().enumerate() {
                    post_match_grades(m, which, form, records, random);
                }
                post_match_books(m, records);
                if let Some(tables) = lessons {
                    post_match_lessons(m, records, tables);
                }
            }
        }
    }
}

/// The day's pre-match walk (`FUN_006ef296`), run once over the row
/// before the minutes. Its first loop numbers the row and tests each
/// side's id pair against the saved national teams (`FUN_00677428`) and
/// the regional selections (`FUN_0067764c`), keeping the sides' places in
/// the national list in the marker's upper bytes (`FUN_00677574`); the
/// replica has no national lists, so its ids stand for clubs and every
/// fixture stays plain. The second loop hands both sides' staff the
/// talk (+0x150/+0x344) and lays the plain fixture's defaults: chance
/// slot and added-time byte at 5 — two places down that byte is
/// `added_time`'s selector one, so a plain fixture's stoppage sits a
/// touch before the hour — and no referee, the record's 0xffff. Nothing here
/// draws a random number: the league day's draw stream starts at its
/// first minute, as the week's trace shows. The walk's third loop — the
/// national call-ups, the crowd `FUN_0078b26c` reckons ([`crowd_row`]: the
/// lean; the gate, its home and away fans at +0x50/+0x54 settling the
/// crowd byte at +0x58, wait), and the referee pair `FUN_006cdbb9`/`FUN_004244da` unpacked by `FUN_0063161e`
/// into +0x44a..+0x458 for a national tie — works on the clubs and waits
/// there; its fourth, the sides' line-ups, is [`line_up_row`].
pub fn pre_match(row: &mut [Match]) {
    for (index, m) in row.iter_mut().enumerate() {
        m.index = index as u16;
        let national = [is_national(m.ids[1]), is_national(m.ids[3])];
        m.nat_bits = 0;
        if national[0] || national[1] {
            // the regional bits 4 and 8, the list places in bytes two
            // and three, and the day's national-tie number from bit 4
            // up wait with the lists
            m.nat_bits = national[0] as u16 + 2 * national[1] as u16;
        }
    }
    for m in row.iter_mut() {
        m.sides[0].assistant = 1;
        m.sides[1].assistant = 1;
        // the squad walk stood the eleven: each side's pitch holds eleven
        // men — the sending-offs that take one away with the formation
        // slot wait with those events
        m.sides[0].men = 11;
        m.sides[1].men = 11;
        if m.nat_bits == 0 {
            m.added_draw = 5;
            m.chance_slot = 5;
            m.referee = -1;
        } else {
            // the assistants drop where the club has none (its f_08a and
            // f_084 flags), and the referee drawn out of the two clubs
            // fills the slot, the added byte and +0x44a..+0x454
        }
    }
}

/// The walk's third loop's crowd for each league fixture of the row
/// (`FUN_0078b26c` with the match, its home and its away club: a kind
/// whose top bits are nought), [`crate::presale::league_crowd`] on the
/// day, and the lean it leaves at +0x446 ([`crowd_lean`]). The cups' and
/// the friendlies' branches wait with their rows.
pub fn crowd_row(row: &mut [Match], records: &Records, data: &crate::data::Data) {
    let today = records.globals.day() as i16;
    for m in row.iter_mut() {
        if m.kind & 0x7f00_0000 != 0 || is_national(m.ids[1]) || is_national(m.ids[3]) {
            continue;
        }
        let side = |which: usize| (m.ids[which * 2] as u8, m.ids[which * 2 + 1] as i16);
        let matchday = (m.kind & 0x3f) as i32 + 1;
        if let Some((home, away)) =
            crate::presale::league_crowd(records, side(0), side(1), matchday, today, &data.presale)
        {
            m.fans = [home, away];
            m.form = crowd_lean(home, away);
        }
    }
}

/// The crowd's lean (`FUN_0078b26c`'s tail): the more fans' side against
/// the fewer, twice the more over the fewer under a root, times twice the
/// whole crowd, 40000 on, a sixty-thousandth of it — below nought for
/// the home side, at most three either way; an even crowd leans to the
/// visitors.
pub fn crowd_lean(home: i32, away: i32) -> i16 {
    let ahead = |more: i32, fewer: i32| {
        let more = f64::from(more) + f64::from(more);
        if fewer < 1 { more.sqrt() } else { (more / f64::from(fewer)).sqrt() }
    };
    let whole = f64::from(home.wrapping_add(away));
    let lean = |root: f64, sign: f64| ((whole + whole) * root + 40000.0) * sign / 6_000_000.0;
    if away < home {
        (lean(ahead(home, away), -100.0) as i32 as i16).max(-3)
    } else {
        (lean(ahead(away, home), 100.0) as i32 as i16).min(3)
    }
}

/// The pre-match walk's fourth loop (`FUN_006ef296` on a row it does not
/// resume, its third parameter nought), match by match, the first side
/// before the second: the camp booked before the match (`FUN_0059a2ea`,
/// [`training_day::camp_before_match`], which the day runs on the human
/// fixtures), then `FUN_0042a702` — a side whose club has no human
/// trainer (club +0x8a nought) is lined up by its co-trainer in a system
/// of his choosing with three foreigners at most (`FUN_0042c802` with 6,
/// its parameter block from `FUN_004300b0`; the competition flag
/// `FUN_0042a702` lays beside it at +0xc is read by nobody), the bench
/// "Auto." fills (`FUN_0042a828`) and the parts the eleven count for
/// (`FUN_006a18c2`): [`crate::autolineup::line_up_computer`], into the
/// team record the club object is loaded from. So every computer side —
/// a foreign club of a European tie, whose record the week's line-up
/// (`FUN_0042a76e`, the played nations only) never touches, or a club
/// whose listed man has left since — stands the co-trainer's eleven the
/// walk copies into the side's +0x8e. A national side takes its call-ups
/// first (`FUN_0062fdc9`, `FUN_0062f5d9` on club 200), which wait with
/// the national lists; and a human side, before day 42 unless the start
/// option `DAT_00b5d152` is 5 or 12 (`FUN_006ef262`), is lined up the
/// same way without his turn (`FUN_0042a7da`), else his turn opens
/// (events 0xb7, 0xc0) — the replica stands his saved eleven. The
/// co-trainer draws nothing.
pub fn line_up_row(row: &[Match], records: &mut Records, data: &crate::data::Data) {
    for m in row {
        for which in 0..2 {
            let (country, number) = (m.ids[which * 2], m.ids[which * 2 + 1]);
            if is_national(number) {
                continue;
            }
            crate::autolineup::line_up_computer(records, data, country, number);
        }
    }
}

/// The pay a side plays for against a man's salary (live +0x88, the
/// record's +0x150), in the money unit `DAT_00a3b420` (ten:
/// `FUN_00627bd0`'s floats times one): a bonus past fifty units of his
/// salary lifts his part to 112 %, past twenty to 110, twelve 108, ten
/// 106, six 104, three 103; one under a unit leaves him at 96 %, under
/// two at 98, else whole (`FUN_006a1bcf`, `DAT_00a3b3ec` …
/// `DAT_00a3b418`).
const BONUS_STEPS: [(i32, i32); 6] = [(500, 112), (200, 110), (120, 108), (100, 106), (60, 104), (30, 103)];
const BONUS_SHORT: [(i32, i32); 2] = [(10, 96), (20, 98)];
/// The position whose ability word bit 0x800 lifts or sinks a man's part
/// by a point (`FUN_006a1bcf`'s `FUN_00696c00(…) == 0xe`).
const WORD_POSITION: u8 = 14;
const WORD_BIT: u16 = 0x800;
/// The live +0x57 bit a caution raises — the record's +0x25b keeps it
/// from the man's last match — and the trait bits that spare him its
/// toll: the second word's bit 1, the first's 1 and 8.
const CAUTIONED: u8 = 8;
/// The favourite's slack (`FUN_006f6fc7`): a side whose strength on the
/// day lies past the foe's by more than these may take the match lightly,
/// each step a throw under `(200 - will) * 3 / 80` of a hundred,
/// its base percent at +0x1c falling to the step's.
const FAVOURITE: [(i16, &[i16]); 2] = [(40, &[70, 90, 98]), (20, &[90, 98])];

/// What one man of a side counts for before its match, a hundredfold
/// (`FUN_006a1bcf`'s loop): his strength on the spot's kind as the
/// line-up reckons it (`FUN_006a144e`) and twenty more, a point either
/// way on position 14 by his good and bad word's bit 0x800, a cautioned
/// man less — four fifths for a defender (`FUN_00677c5f`), nine tenths
/// for any other — and the side's bonus against his salary.
fn man_part(pitch: &crate::lineup::Tables, spot: (i16, i16), strength: i32, p: &Player, bonus: i32) -> i32 {
    let mut v = strength + 20;
    if pitch.position(spot) == WORD_POSITION {
        if p.good() & WORD_BIT != 0 {
            v += 10;
        } else if p.bad() & WORD_BIT != 0 {
            v = (v - 10).max(0);
        }
    }
    let traits = p.traits();
    if p.match_bits() & CAUTIONED != 0 && traits[1] & 1 == 0 && traits[0] & 9 == 0 {
        v = if (2..=5).contains(&p.positions()[0]) { (v << 2) / 5 } else { v * 9 / 10 };
    }
    v * bonus_percent(bonus, p.salary())
}

/// What the side's bonus makes of a man's part against his salary, in
/// percent ([`BONUS_STEPS`], [`BONUS_SHORT`]).
fn bonus_percent(bonus: i32, salary: u16) -> i32 {
    let salary = i32::from(salary);
    BONUS_STEPS
        .iter()
        .find(|&&(by, _)| bonus > salary * by)
        .or_else(|| BONUS_SHORT.iter().find(|&&(by, _)| bonus < salary * by))
        .map_or(100, |&(_, pct)| pct)
}

/// A side's five strengths and six line shorts before its match
/// (`FUN_006a1bcf`, which `FUN_006f7b67` reaches through `FUN_006f6c20`):
/// a squad under eleven takes the defaults at 0x8a52a8 — the second row
/// for a club of a country marked at 0x89dfc8, the third for one below
/// the tenth level, else the first —; any other sums its eleven, each
/// man's part ([`man_part`]) spread by the band down the pitch his spot
/// lies in over goal, defence, defensive and attacking midfield and
/// attack (the row's bytes 0, 1, 4, 5 and 3 at 0x8a5258), the two
/// midfields further over the wings by the spot across
/// ([`crate::lineup::Tables::wing`]); every sum a hundredth. The offside
/// byte the side keeps at +0x3e (`FUN_006f981c`) reads the men's kinds
/// at their live +0x28, which the record keeps nowhere: a seam.
fn side_parts(data: &crate::data::Data, records: &Records, team: &crate::records::Team, bonus: i32) -> ([i16; 5], [i16; 6]) {
    let pitch = &data.rules.pitch;
    if team.players().len() < 11 {
        let row = if data.rules.contract.marked_country(team.country()) {
            1
        } else if team.level() > 9 {
            2
        } else {
            0
        };
        let r = pitch.short_side(row);
        return ([r[0], r[1], r[2], r[3], r[4]], [r[5], r[6], r[7], r[8], r[9], r[10]]);
    }
    let club = crate::load::club_state_of(records, team, data);
    let squad = team.players();
    let (mut s, mut l) = ([0i32; 5], [0i32; 6]);
    for (spot, k) in club.lineup.on_field() {
        let (Some(man), Some(p)) = (club.players.get(k), squad.get(k).and_then(|&id| records.player(id))) else {
            continue;
        };
        let strength = pitch.strength_at(man, crate::lineup::fitness_of(&club.fitness, k), pitch.kind_under(spot));
        let v = man_part(pitch, spot, strength, p, bonus);
        let row = pitch.side_row(pitch.down_band(spot.1));
        let part = |b: u8| (u32::from(b).wrapping_mul(v as u32)) as i32 / 100;
        s[0] += part(row[0]);
        s[1] += part(row[1]);
        s[4] += part(row[3]);
        let (a, b) = (part(row[4]), part(row[5]));
        s[2] += a;
        s[3] += b;
        let wing = pitch.wing(spot.0).map(i32::from);
        for (w, &share) in wing.iter().enumerate() {
            l[w * 2] += a * share / 100;
            l[w * 2 + 1] += b * share / 100;
        }
    }
    (s.map(|v| (v / 100) as i16), l.map(|v| (v / 100) as i16))
}

/// The class a man adds to his side (`FUN_006f6b7d`): the first trait
/// word's bit 8 one up, a field player's bit 0x10 one down.
fn class_of(p: &Player) -> i8 {
    let bits = p.traits()[0];
    if bits & 8 != 0 {
        1
    } else if p.positions()[0] != crate::world::GOALKEEPER && bits & 0x10 != 0 {
        -1
    } else {
        0
    }
}

/// The condition a side's eleven stand in on average, as the lists show
/// it (`FUN_006770a4` over `FUN_00457e00`): a side listing nobody takes
/// its squad's, an empty squad fifty.
fn eleven_condition(records: &Records, team: &crate::records::Team) -> i32 {
    let points = |ids: &[u16]| -> Vec<i32> {
        ids.iter()
            .filter(|&&id| id != 0)
            .map(|&id| records.player(id).map_or(0, |p| i32::from(crate::computer_training::cond_percent(p))))
            .collect()
    };
    let eleven = points(&team.lineup());
    if !eleven.is_empty() {
        return eleven.iter().sum::<i32>() / eleven.len() as i32;
    }
    let squad = points(&team.players());
    if squad.is_empty() { 50 } else { squad.iter().sum::<i32>() / squad.len() as i32 }
}

/// The kick-off of one match of the row (`FUN_006f6fc7`, the first call
/// of the pre-match walk's fifth loop), over the sides' records as
/// [`line_up_row`] left them: both sides' strengths and line shorts
/// ([`side_parts`]), each club's tactics copied in — its style word to
/// +0x2e, its tilt, stance and tier bytes (club +0xa8, +0xa9, +0xaa) —
/// and its will to win tenfold to +0x0a (`FUN_006f7b67`); the class the
/// eleven's men carry summed into +0x8f/+0x283 (`FUN_006f6b7d`); eleven
/// men a side; what the first side's eleven lead the second's by in
/// condition, twice over and within 45, at +0x444 (`FUN_006f6c35`); the
/// strength on the day and its parts at +0x122../+0x316.. (the club's
/// +0xdf2); both base percents at a hundred, and a side stronger on the
/// day by more than forty or twenty throws for its slack ([`FAVOURITE`]),
/// the first side first. The man marks at +0x40 (`FUN_006f794a`), the
/// forwards' percent at +0xb4 (`FUN_006fad6c`), the league table's
/// bits at +0x13c/+0x330 and the per-man match bytes the walk resets
/// (`FUN_006f7af1`) wait; none of them draws.
pub fn kick_off_strengths(m: &mut Match, records: &Records, data: &crate::data::Data, random: &mut Random) {
    let mut wills = [0i16; 2];
    let mut conditions = [0i32; 2];
    for which in 0..2 {
        let Some(team) = records.team(m.ids[which * 2], m.ids[which * 2 + 1]) else { continue };
        let (strengths, lines) = side_parts(data, records, team, m.bonus[which]);
        let side = &mut m.sides[which];
        side.strengths = strengths;
        side.lines = lines;
        side.style = team.tactic_flags();
        wills[which] = team.will();
        side.nerve = wills[which].wrapping_mul(10);
        side.tilt = team.style();
        side.stance = team.effort();
        side.tier = team.hardness();
        for id in team.lineup().into_iter().filter(|&id| id != 0) {
            if let Some(p) = records.player(id) {
                side.aggr = side.aggr.wrapping_add(class_of(p));
            }
        }
        side.men = 11;
        side.shot_pwr = shot_strength(data, records, team);
        conditions[which] = eleven_condition(records, team);
    }
    m.advantage = ((conditions[0] - conditions[1]) * 2).clamp(-45, 45) as i16;
    for side in m.sides.iter_mut() {
        let s = side.strengths.map(i32::from);
        side.day = [
            (s.iter().sum::<i32>() / 10) as i16,
            (s[4] / 10) as i16,
            ((s[2] + s[3]) / 10) as i16,
            (s[1] / 10) as i16,
        ];
        side.strength = 100;
    }
    for (us, them) in [(0usize, 1usize), (1, 0)] {
        let (ours, theirs) = (m.sides[us].day[0], m.sides[them].day[0]);
        let Some(&(_, steps)) = FAVOURITE.iter().find(|&&(by, _)| theirs.wrapping_add(by) < ours) else {
            continue;
        };
        let odds = (200 - i32::from(wills[us])) * 3 / 80;
        for &step in steps {
            if (random.below(100) as i32) < odds {
                m.sides[us].strength = step;
                break;
            }
        }
    }
}

/// The side's shooting strength the kick-off ends with (`FUN_006fad6c`
/// on +0x5c and +0x250, +0xb4): over the club's formation (+0xa9a, each
/// man's spot at +0xae6/+0xae8), a man in the striker's area (position
/// 10, `FUN_00696c00`) counts two and one on the wings or behind the
/// striker (9, 11, 12) one; under six the side shoots whole, up to eight
/// ten points less each from 150, and past that at 60.
fn shot_strength(data: &crate::data::Data, records: &Records, team: &crate::records::Team) -> i16 {
    let pitch = &data.rules.pitch;
    let club = crate::load::club_state_of(records, team, data);
    let forward: i16 = club
        .lineup
        .on_field()
        .map(|(spot, _)| match pitch.position(spot) {
            10 => 2,
            9 | 11 | 12 => 1,
            _ => 0,
        })
        .sum();
    if forward < 6 {
        100
    } else if forward < 9 {
        forward * -10 + 0x96
    } else {
        0x3c
    }
}

/// The pre-match walk's fifth loop (`FUN_006ef296`), match by match: the
/// kick-off ([`kick_off_strengths`]); each side's press window, the
/// first side first ([`press_window`], `FUN_006f9656`); the computer
/// clubs' tactics against the other side by their strengths on the day
/// as the kick-off left them (club +0xdf2), the first side at home
/// (`FUN_0042c1aa` both ways, [`crate::exhibition::computer_tactics`],
/// three throws a computer side), into the team records the club objects
/// go back to; then the clubs' tactics laid over the sides — style word,
/// tilt, stance, tier (`FUN_006f7d4d`) — and three talks a side (+0x10e,
/// +0x302; a match of the kind at +0x24 takes `DAT_00b5d168`, which no
/// league match is). `FUN_0071511f` (a human club's ledger),
/// `FUN_006f94e4`, `FUN_00698006` (club +0xb40) and the club-object
/// halves of `FUN_006f7d4d` — the formation copies the replica reads from
/// the record, the listed men's freshness and condition averaged into
/// +0xc0/+0xc2, a human manager's tactics at `DAT_01bdd6d8` — draw nothing
/// and wait; so do the events 0xc5/0xc6 a human's turn opens.
pub fn kick_off_row(row: &mut [Match], records: &mut Records, data: &crate::data::Data, random: &mut Random) {
    for m in row.iter_mut() {
        kick_off_strengths(m, records, data, random);
        for which in 0..2 {
            press_window(m, which, records, random);
        }
        let clubs = [0, 1].map(|w| (m.ids[w * 2] as u8, m.ids[w * 2 + 1] as i16));
        let ahead = i32::from(m.sides[0].day[0].wrapping_sub(m.sides[1].day[0]));
        let behind = i32::from(m.sides[1].day[0].wrapping_sub(m.sides[0].day[0]));
        crate::exhibition::computer_tactics(records, clubs[0], clubs[1], true, ahead, random);
        crate::exhibition::computer_tactics(records, clubs[1], clubs[0], false, behind, random);
        for which in 0..2 {
            let side = &mut m.sides[which];
            if let Some(team) = records.team(m.ids[which * 2], m.ids[which * 2 + 1]) {
                side.style = team.tactic_flags();
                side.tilt = team.style();
                side.stance = team.effort();
                side.tier = team.hardness();
            }
            side.talk_max = TALKS;
        }
    }
}

/// The talks a side may hold in a match (+0x10e/+0x302).
const TALKS: i16 = 3;
/// The places of the formation whose men can carry the press
/// (`FUN_006f9656`: +0xa9a's shorts 5 to 10), the bits of the man's good
/// word that make him one (+0xc & 0x102) and the condition and freshness
/// he must stand past.
const PRESS_PLACES: std::ops::Range<usize> = 5..11;
const PRESS_WORD: u16 = 0x102;
const PRESS_FIT: i16 = 40;

/// A side's press window (`FUN_006f9656`, the clock nought from the
/// walk's start): both bytes cleared; a club pressing (its style bit
/// 0x10) counts its pressing byte (club +0x9f, `FUN_00698b3d(-1)`) and
/// each man on the places 5 to 10 with the good word's 0x102 who stands
/// past forty in condition (`FUN_00457e00`) and in freshness
/// (`FUN_006ff649`); past two, +0x3b takes a throw of twenty from the
/// fifth minute (the 45th once the clock runs) and +0x3c five minutes
/// past it for each beyond two. The minute's test (`FUN_006f9788`) asks
/// for a minute at most +0x3b and at least +0x3c, so the window the walk
/// lays never opens.
fn press_window(m: &mut Match, which: usize, records: &Records, random: &mut Random) {
    let side = &mut m.sides[which];
    side.press_to = 0;
    side.press_from = 0;
    let Some(team) = records.team(m.ids[which * 2], m.ids[which * 2 + 1]) else { return };
    if team.tactic_flags() & PRESS_BIT == 0 {
        return;
    }
    let lineup = team.lineup();
    let mut count = i16::from(team.pressing());
    for &id in &lineup[PRESS_PLACES] {
        let Some(p) = (id != 0).then(|| records.player(id)).flatten() else { continue };
        if p.good() & PRESS_WORD != 0
            && crate::computer_training::cond_percent(p) > PRESS_FIT
            && crate::computer_training::fresh_percent(p) > PRESS_FIT
        {
            count += 1;
        }
    }
    if count > 2 {
        side.press_to = random.below(20) as u8 + 5;
        side.press_from = side.press_to.wrapping_add(((count - 2) * 5) as u8);
    }
}

/// The building kind of an airfield, its record's first dword
/// (`DAT_01c8b498`, the 57th of the grounds' types).
const AIRFIELD: i16 = 57;

/// Whether a club's cached grounds hold a finished airfield of its own
/// (`FUN_007be73d` over the ten keys at `DAT_008ef358`, then
/// `FUN_007be6bd`: an entry in use, on its land, no week of building
/// left).
fn has_airfield(records: &Records, country: u16, number: u16) -> bool {
    let Some(grounds) = crate::grounds::Grounds::of(records, country as u8, number as i16) else {
        return false;
    };
    (0..crate::grounds::ENTRIES).rev().map(|k| grounds.entry(k)).any(|e| {
        e.used && e.owned && e.weeks == 0 && e.kind == AIRFIELD
    })
}

/// Whether the match is played on the side's own ground (`FUN_006782aa`
/// on the row's +0x28: nation byte and number).
fn at_home(m: &Match, which: usize) -> bool {
    m.home_nation == m.ids[which * 2] as u8 && m.home_id == m.ids[which * 2 + 1] & 0xff
}

/// Both sides' men entered for the match (`FUN_00658cfb`, the sixth
/// loop's first call): the formation's eighteen places — eleven and the
/// bench, club +0xa9a — copied into +0x156/+0x34a as listed, the first
/// side's place before the second's, and for each listed man a throw of
/// a hundred whether he has an off day (past his live +0x09 five times
/// and fifty his part at +0x17a is seventy of a hundred, a second side's
/// man with the good word's 0x800 at eighty). The parts, his condition
/// at +0x1f8 and the ledger bytes at 0x23 wait with the live man.
fn enter_sides(m: &mut Match, records: &Records, random: &mut Random) {
    let listed = [0, 1].map(|which| {
        let mut ids = [0u16; LEDGER_SLOTS];
        if let Some(team) = records.team(m.ids[which * 2], m.ids[which * 2 + 1]) {
            for (slot, id) in ids.iter_mut().zip(team.lineup().into_iter().chain(team.bench())) {
                *slot = id;
            }
        }
        ids
    });
    for slot in 0..LEDGER_SLOTS {
        for which in 0..2 {
            let id = listed[which][slot];
            m.sides[which].entries[slot] = id;
            if id != 0 && records.player(id).is_some() {
                random.below(100);
            }
        }
    }
}

/// The base percent a side loses on a foe's record against it: a tally
/// from eight on takes the second side's to 96 of a hundred, from five
/// to 98; under minus five the first side's to 96, under minus two 98
/// (`FUN_006ef296` over `FUN_0040c5e0`).
const TALLY_STEPS: [(i16, usize, i16); 4] = [(8, 1, 96), (5, 1, 98), (-6, 0, 96), (-3, 0, 98)];
/// A side whose foe flew in on its own airfield plays at 95 of a hundred.
const FLOWN_IN: i16 = 95;

/// The pre-match walk's sixth loop (`FUN_006ef296`), match by match:
/// the men entered ([`enter_sides`]); the base percents at +0x78/+0x26c
/// bent by the first club's record against the second
/// ([`crate::opponent::home_tally`], [`TALLY_STEPS`]) and by a visitor's
/// airfield ([`has_airfield`]: a side away from its ground that has one
/// takes the other side's to [`FLOWN_IN`]); then the first half's added
/// time (`FUN_0040bc1b`) on the 45th minute as the half's end, the
/// minute-up flag cleared. On the match the listing marked for the
/// doping control (+0x460) both sides have their men picked between the
/// tally and the added time, the first side first
/// ([`crate::doping_control::pick`], `FUN_005dad80`). Before the tally, a club a human manager's
/// flag +0xf8 bit 1 names starts at 120 (`FUN_006782aa` over
/// `FUN_006a421a`'s list), which waits with the manager objects; the national
/// tie's bulk run (`FUN_0070c334`), the home club's gate
/// (`FUN_00678021`), the human manager's man at +0x74/+0x268
/// (`FUN_0069c1a2`), the weighted lists' counts at +0x462/+0x463
/// (`FUN_00767686`), `FUN_006fb1d1` and the bonus at +0x244/+0x438 —
/// club +0xb0, a word the team record does not keep — draw nothing and
/// wait.
pub fn ready_row(row: &mut [Match], records: &mut Records, random: &mut Random) {
    for m in row.iter_mut() {
        enter_sides(m, records, random);
        let teams = [0, 1].map(|w| records.team(m.ids[w * 2], m.ids[w * 2 + 1]));
        let tally = match teams {
            [Some(a), Some(b)] => crate::opponent::home_tally(records, a, b),
            _ => 0,
        };
        let step = TALLY_STEPS
            .iter()
            .find(|&&(by, _, _)| if by > 0 { tally >= by } else { tally <= by });
        if let Some(&(_, which, pct)) = step {
            let s = &mut m.sides[which].strength;
            *s = (i32::from(*s) * i32::from(pct) / 100) as i16;
        }
        for (us, them) in [(0usize, 1usize), (1, 0)] {
            if !at_home(m, us) && has_airfield(records, m.ids[us * 2], m.ids[us * 2 + 1]) {
                let s = &mut m.sides[them].strength;
                *s = (i32::from(*s) * i32::from(FLOWN_IN) / 100) as i16;
            }
        }
        if m.doping {
            for which in 0..2 {
                let men = crate::doping_control::pick(records, m.ids[which * 2], m.ids[which * 2 + 1], random);
                for (slot, id) in m.picked[which].iter_mut().zip(men) {
                    *slot = id;
                }
            }
        }
        m.ends_at = added_time(m, random) + 45;
        m.flags &= !MINUTE_UP;
    }
}

/// The day's whole pre-match walk (`FUN_006ef296`) over a row the day
/// lists: [`pre_match`]'s loops, the line-ups ([`line_up_row`]), the
/// kick-off and tactics ([`kick_off_row`]) and the men and added time
/// ([`ready_row`]).
pub fn pre_match_walk(row: &mut [Match], records: &mut Records, data: &crate::data::Data, random: &mut Random) {
    pre_match(row);
    crowd_row(row, records, data);
    line_up_row(row, records, data);
    kick_off_row(row, records, data, random);
    ready_row(row, records, random);
    // the info page's tip reads it; the replica's page waits
    let _tip = celebrity_tip(row, records, random);
}

/// The celebrity's tip on the humans' matches, drawn as the walk's last
/// step once every match stands ready (`FUN_004cbf59` from
/// `FUN_006ef296` at 0x6f0ea8; `CInfoseiteProminententip` shows it).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CelebrityTip {
    /// The celebrity's country (`DAT_008ec6ea`) and which of its twenty
    /// (`DAT_008ec6ec`).
    pub country: i16,
    pub celebrity: u32,
    /// Each tipped match's ids (`DAT_008ec6f0..`) and the goals tipped
    /// for its sides (`DAT_008ec710`, `DAT_008ec718`).
    pub tips: Vec<([u16; 4], [i16; 2])>,
}

/// Seven times in ten, or always without further countries, the main
/// country's celebrity (`DAT_00b5cbb0`), else one of a further country,
/// picked among `DAT_00b5cbae` by the countries' key (`DAT_00ae5d8e`);
/// then each match marked at +0x474 (a human's club, a national side)
/// gets ninety throws below a thousand: under 24 and twice the levels'
/// gap (club +0x105, at most 2, the away side's less the home side's) a
/// home goal, above 972 less that an away goal, and a one-leg tie that
/// must be won (+0x20 bit 0x80000 without the second leg) breaks a draw
/// for the home side.
pub fn celebrity_tip(row: &[Match], records: &Records, random: &mut Random) -> CelebrityTip {
    let globals = &records.globals;
    let more = globals.more_count();
    let country = if random.below(10) < 7 || more == 0 {
        globals.main_country()
    } else {
        records.countries.ranking_key(random.below(u32::try_from(more).unwrap_or(0)) as usize)
    };
    let celebrity = random.below(0x14);
    let level = |nation: u16, id: u16| records.team(nation, id).map_or(0, |t| i16::from(t.level().min(2)));
    let mut tips = Vec::new();
    for m in row.iter().filter(|m| m.nat_bits != 0) {
        let gap = level(m.ids[2], m.ids[3]) - level(m.ids[0], m.ids[1]);
        let (mut home, mut away) = (0, 0);
        for _ in 0..0x5a {
            let roll = random.below(1000) as i16;
            if roll < gap * 2 + 0x18 {
                home += 1;
            }
            if gap * -2 + 0x3cc < roll {
                away += 1;
            }
        }
        if m.kind & (TWO_LEGS | 0x8_0000) == 0x8_0000 && home == away {
            home += 1;
        }
        tips.push((m.ids, [home, away]));
    }
    CelebrityTip { country, celebrity, tips }
}

/// One minute of the day's matches, `FUN_006f2d4f` without its
/// presentation; true while some open match still runs before its end of
/// half. The display clocks (+0x1c, +0x0c) are left out.
fn tick(
    matches: &mut [Match],
    live: bool,
    minute: i16,
    clock: &mut Clock,
    mut records: Option<&mut Records>,
    today: NaiveDate,
    random: &mut Random,
) -> bool {
    if clock.extra == 0 && clock.half2 == 0 && minute < 0x78 {
        clock.minute = minute;
        let mut run = false;
        for m in matches.iter_mut() {
            if m.flags & FINISHED == 0 && (live || m.nat_bits == 0) {
                if minute == 10 {
                    m.score10 = [m.goals[0] as u8, m.goals[1] as u8];
                }
                minute_play(m, clock, records.as_deref_mut(), random);
                if minute < m.ends_at.max(45) {
                    run = true;
                } else {
                    m.flags |= MINUTE_UP;
                }
            }
        }
        run
    } else if clock.extra == 0 && clock.half2 == 1 {
        clock.minute = minute;
        let mut run = false;
        for m in matches.iter_mut() {
            if live || m.nat_bits == 0 {
                if m.flags & FINISHED == 0 {
                    minute_play(m, clock, records.as_deref_mut(), random);
                    if minute < m.ends_at.max(90) {
                        run = true;
                    } else {
                        m.flags |= MINUTE_UP;
                    }
                }
                if minute == 75 {
                    m.score75 = [m.goals[0] as u8, m.goals[1] as u8];
                } else if minute == 85 {
                    m.score85 = [m.goals[0] as u8, m.goals[1] as u8];
                } else if minute == 90 {
                    m.score90 = [m.goals[0] as u8, m.goals[1] as u8];
                }
            }
        }
        run
    } else if clock.extra == 1 {
        clock.minute = minute;
        if minute == 106 {
            clock.half2 = 1;
        }
        let mut run = false;
        for m in matches.iter_mut() {
            if (live || m.nat_bits == 0) && m.flags & FINISHED == 0 && m.flags & EXTRA != 0 {
                minute_play(m, clock, records.as_deref_mut(), random);
                if minute < m.ends_at.max(120) {
                    run = true;
                } else {
                    m.flags |= MINUTE_UP;
                }
            }
        }
        run
    } else {
        for m in matches.iter_mut() {
            if (live || m.nat_bits == 0) && m.flags & FINISHED == 0 && m.flags & PENS != 0 {
                penalties(m, records.as_deref(), today, random);
            }
        }
        false
    }
}

/// One match's minute, `FUN_006f4744`: the moods sink on a lost cause,
/// both sides' rated strengths come out of their strengths, stance, tactic,
/// mood and the foe's form, fatigue accumulates — and what the minute is
/// worth in chances follows.
fn minute_play(m: &mut Match, clock: &Clock, mut records: Option<&mut Records>, random: &mut Random) {
    if m.flags & (MINUTE_UP | OVER) != 0 {
        return;
    }
    national_tactics(m, records.as_deref());
    let extra = m.flags & EXTRA != 0;
    if m.advantage < 0 {
        if m.advantage as i32 + 90 < clock.minute as i32 {
            if extra {
                m.sides[0].morale += (m.advantage * 15) / 90;
            } else {
                m.sides[0].morale -= 15;
            }
        }
    } else if m.advantage > 0 && (90 - m.advantage as i32) < clock.minute as i32 {
        if extra {
            m.sides[1].morale -= (m.advantage * 15) / 90;
        } else {
            m.sides[1].morale -= 15;
        }
    }
    rate_side(m, 0, extra);
    rate_side(m, 1, extra);
    // The special styles play against the foe's attack: 0x380 keeps one's
    // own back and lets the foe's attack through, 0x300 opens both.
    for (us, them) in [(0usize, 1usize), (1, 0)] {
        match m.sides[us].style & 0x380 {
            0x380 => {
                m.sides[us].rated[3] = (m.sides[us].rated[3] as i32 * 8 / 10) as i16;
                m.sides[them].rated[3] = (m.sides[them].rated[3] as i32 * 85 / 100) as i16;
            }
            0x300 => {
                m.sides[us].rated[3] = (m.sides[us].rated[3] as i32 * 115 / 100) as i16;
                m.sides[them].rated[3] = (m.sides[them].rated[3] as i32 * 120 / 100) as i16;
            }
            _ => {}
        }
    }
    // The press's minute window lifts attack and support a fifth, and marks
    // the side as pressing (`FUN_006f9788`): in the first half any window
    // counts, in the second its end must lie past the 44th minute.
    for side in m.sides.iter_mut() {
        let open = if clock.half2 == 0 {
            side.press_to != 0
        } else {
            side.press_to > 44
        };
        if side.style & PRESS_BIT != 0
            && open
            && side.press_to as i16 >= clock.minute
            && side.press_from as i16 <= clock.minute
        {
            side.rated[3] = (side.rated[3] as i32 * 12 / 10) as i16;
            side.rated[2] = (side.rated[2] as i32 * 12 / 10) as i16;
            side.pressed = 1;
        }
    }
    for side in m.sides.iter_mut() {
        if side.rated[3] < 10 {
            side.rated[3] = 10;
        }
    }
    // Fatigue: the minute's percentage, until the half's minute runs out —
    // 46 up to half time, 91 through the second half, 121 in extra time.
    let accrue = if !extra {
        if clock.half2 == 0 {
            clock.minute < 46
        } else {
            clock.minute < 91
        }
    } else {
        clock.minute < 121
    };
    if accrue {
        for side in m.sides.iter_mut() {
            side.fatigue += MINUTE_PCT[usize::from(side.stance)] as i16;
        }
    }
    for side in m.sides.iter_mut() {
        if side.rated[4] < 0 {
            side.rated[4] = 3;
        }
    }
    // The chances. When neither side is national — the league's path, as
    // the week's draw trace shows it under `FUN_006f4744`'s returns
    // +0x0c4/+0x0d4 — the minute runs the plain chance frame and the
    // trailing side's talk (`FUN_006f6007`, `FUN_0041321d`). A tie with a
    // national side instead runs through the clubs' match-day state
    // (`FUN_00658b30` twice, their state at +0x474 set by the pre-match
    // walk), the set pieces (`FUN_006f543f`) and, when those come to
    // nothing, open play (`FUN_006f562b`) — that path is a later piece.
    if !is_national(m.ids[0]) && !is_national(m.ids[2]) {
        chance_minute(m, clock, records.as_deref_mut(), random);
        morale_talk(m, clock, random);
    } else {
        // FUN_0063dd1c()/FUN_0063df96() pair the clubs, FUN_00658b30(club,
        // side, clock) twice, FUN_006fbc5e(m), set pieces by
        // FUN_006f543f(m, club, club, clock), FUN_0041321d(m, clock) —
        // and FUN_006f562b(m, clock) open play when no set piece scored.
    }
}

/// One side's rated strengths of the minute, the middle of `FUN_006f4744`.
/// The stance scales the minute's base; the tilt trades strength 5 forward
/// into rated 2 and strength 2 back into rated 5; mood lifts rated 3 and 4;
/// the foe's form and the side's mental lift the attack pair; a side's own
/// form never feeds both sides, the original reads `form` once with its
/// sign for one side and the other.
fn rate_side(m: &mut Match, which: usize, _extra: bool) {
    let side = &mut m.sides[which];
    let foe_form = if which == 0 {
        if m.form < 0 { m.form } else { 0 }
    } else if m.form > 0 {
        m.form
    } else {
        0
    };
    let base = (side.strength as i32 * MINUTE_PCT[usize::from(side.stance)]) / 100;
    let s = side.strengths;
    let tilt_on = TILT_ON[usize::from(side.tilt)];
    let tilt_back = TILT_BACK[usize::from(side.tilt)];
    let mental = MENTAL[side.mental as usize];
    let r = &mut side.rated;
    r[0] = (s[0] as i32 * base / 100) as i16;
    r[1] = ((s[0] as i32 + s[1] as i32 + (s[4] as i32 * tilt_on) / 100) * base / 100) as i16;
    r[2] = ((s[2] as i32 + side.morale as i32 / 20) * base / 100) as i16;
    r[3] = ((s[3] as i32 + side.morale as i32 / 20) * base / 100) as i16;
    r[4] = ((s[4] as i32 + (s[1] as i32 * tilt_back) / 100) * base / 100) as i16;
    if foe_form != 0 {
        // The original subtracts the form product on the first side (whose
        // form is then negative, `+0x70 -= +0x70*form/100`) and adds it on
        // the second (`+0x264 += +0x264*form/100`); both lift the attack.
        let d = if which == 0 { -(foe_form as i32) } else { foe_form as i32 };
        r[3] = (r[3] as i32 + r[3] as i32 * d / 100) as i16;
        r[4] = (r[4] as i32 + r[4] as i32 * d / 100) as i16;
    }
    r[3] = (r[3] as i32 + r[3] as i32 * mental / 100) as i16;
    r[4] = (r[4] as i32 + r[4] as i32 * mental / 100) as i16;
}

/// A national side's own tactic, which the association's table sets for
/// the minute from the side's place in the list: the first side's place
/// in the marker's bits 8..10, the second's in bits 12..14.
fn national_tactics(m: &mut Match, records: Option<&Records>) {
    if m.nat_bits & 1 != 0 {
        national_tactic(&mut m.sides[0], (m.nat_bits as i16 >> 8) & 7, records);
    }
    if m.nat_bits & 2 != 0 {
        national_tactic(&mut m.sides[1], (m.nat_bits as i16 >> 12) & 7, records);
    }
}

/// The association's national-side table (`FUN_006fb8f5`) overwriting a
/// national side's tactic byte at +0x30 with the entry at its list place
/// less one: `DAT_01bdd6d8 + (place-1)·0x38`, the first data byte (+0x20)
/// of the four saved raw objects at 0x1bdd6b8, 24 bytes each. Places
/// outside 1..4 leave the tactic standing; it only ever runs for a tie
/// carrying that side. Without the save the tactic stands as well.
fn national_tactic(side: &mut Side, place: i16, records: Option<&Records>) {
    if !(1..=4).contains(&place) {
        return;
    }
    let Some(block) = records.and_then(|r| r.block(&format!("{NATIONAL_TABLE}[{}]", place - 1))) else {
        return;
    };
    if let Some(&tilt) = block.first() {
        side.tilt = tilt;
    }
}

/// The association's four national-side entries, saved raw (`0x1bdd6b8`
/// + i·0x38, the loader's 24-byte objects).
const NATIONAL_TABLE: &str = "0x1bdd6b8";

/// Which side has the minute's set moment, `FUN_006f6857`: the record's
/// chance slot at +0x450 (the pre-match walk leaves a plain fixture at 5)
/// gives the minute's step, `below(slot+1)·5+7`; each side then rolls
/// `below(multiplier·step) < 10` to claim it, its tier's multiplier
/// shrinking with its class. A tier byte past 4 skips the side's roll.
/// 1 is the first side, 2 the second, 0 nobody.
fn chance_chooser(m: &Match, random: &mut Random) -> u8 {
    let step = random.below(m.chance_slot as u32 + 1) as i32 * 5 + 7;
    for (which, side) in m.sides.iter().enumerate() {
        let mult = match side.tier {
            0 => 24 - 2 * side.aggr as i32,
            1 => 38 - 2 * side.aggr as i32,
            2 => 56 - 3 * side.aggr as i32,
            3 => 100 - 4 * side.aggr as i32,
            4 => 180 - 5 * side.aggr as i32,
            _ => continue,
        };
        if random.below((mult * step) as u32) < 10 {
            return which as u8 + 1;
        }
    }
    0
}

/// The kinds a foul is drawn from (`FUN_006746aa`): one table by the
/// fouling man, walked with a throw of the table's width, each kind as
/// wide as its entry — `DAT_008a04e8` the plain man's (width 0x21),
/// `DAT_008a0530` a man of a club in the countries 0x10 and 0x39 (live
/// +6, record +0x220; 0x1b), `DAT_008a05c0` the second trait bit 0x20
/// (live +0x12, record +0x1ca; 0x1c), `DAT_008a0578` its bit 2 (0x24) —
/// and `DAT_008a0608` the other draw's (`FUN_00674794`, width 6). A walk
/// past 0x21 entries names 1 (0x20 for `FUN_00674794`).
const FOULS: [i16; 36] = [
    0, 8, 3, 0, 4, 0, 0, 1, 0, 1, 1, 1, 3, 0, 3, 3, 1, 0, 0, 0, 0, 1, 0, 1, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
const FOULS_COUNTRY: [i16; 36] = [
    0, 8, 2, 0, 2, 0, 0, 1, 0, 0, 0, 0, 3, 0, 3, 3, 1, 0, 0, 0, 0, 1, 0, 1, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
const FOULS_HARD: [i16; 36] = [
    0, 12, 3, 0, 6, 0, 0, 3, 0, 2, 1, 2, 3, 0, 3, 3, 2, 0, 0, 0, 0, 1, 0, 1, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
const FOULS_TRAIT: [i16; 36] = [
    0, 6, 3, 0, 2, 0, 0, 0, 0, 1, 1, 1, 3, 0, 3, 3, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
const FOULS_OTHER: [i16; 36] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0,
];

/// The kind out of a table: the throw walks the entries, less each one
/// it passes, and stops at the first it falls under.
fn foul_kind(table: &[i16; 36], roll: u32, past: i16) -> i16 {
    let mut left = roll as i16;
    for (kind, &width) in table.iter().take(0x22).enumerate() {
        if left < width {
            return kind as i16;
        }
        left -= width;
    }
    past
}

/// The eleven the events read: the club's formation +0xa9a, a man sent
/// off leaving his place empty (`FUN_0065a6b0` clears it).
fn pitch_eleven(m: &Match, which: usize, records: &Records) -> Vec<u16> {
    let off = &m.sides[which].sent_off;
    saved_eleven(m, which, records)
        .into_iter()
        .map(|id| if off.contains(&id) { 0 } else { id })
        .collect()
}

/// Whether the man stands cautioned: the live +0x57 bit 8 this match's
/// first card raised. The record's +0x25b keeps the bit from the man's
/// last match — every kept save shows hundreds of men with bit 8 or 0x10
/// standing — and its clearing before the next one is not traced, so
/// the replica starts every man of the walk uncautioned.
fn cautioned(side: &Side, p: &Player) -> bool {
    side.booked.contains(&p.id())
}

/// The man the manager's tactics name at +0x390 of his club's extra data
/// (`FUN_0069c313` on the club object: a club's human slot +0x11de one to
/// four reads `DAT_00a58408`); a computer club names nobody, and a
/// national side's own block (`DAT_00a559f2`) the replica does not read.
fn foul_pick(m: &Match, which: usize, records: &Records) -> u16 {
    if m.nat_bits != 0 {
        return 0;
    }
    let at = which * 2;
    records
        .team(m.ids[at], m.ids[at + 1])
        .and_then(|t| records.extras.get(usize::from(t.human()).checked_sub(1)?))
        .map_or(0, |extra| extra.picks()[tactics::PROVOKE])
}

/// The foul's candidates (`FUN_00674c12`): every man of the eleven by his
/// place, a keeper (type byte live +3 at one) once and always at place
/// nought; a back (2..5) weighs 140, a midfielder (6..9) 100, any other
/// 58, a cautioned man 60, 40 and 20; the first trait's bit 0x10 halves
/// it, else its bit 8 or the second's bit 2 add a half, and the named
/// man uncautioned another half; the man stands weight/10 + 1 times.
fn foul_candidates(m: &Match, which: usize, eleven: &[u16], records: &Records) -> Vec<u8> {
    let marked = foul_pick(m, which, records);
    let mut list = Vec::new();
    for (slot, &id) in eleven.iter().enumerate().take(11) {
        let Some(p) = (id != 0).then(|| records.player(id)).flatten() else {
            continue;
        };
        let position = p.positions()[0];
        if position == world::GOALKEEPER {
            list.push(0);
            continue;
        }
        let booked = cautioned(&m.sides[which], p);
        let mut weight: u16 = match (position, booked) {
            (2..=5, false) => 140,
            (2..=5, true) => 60,
            (6..=9, false) => 100,
            (6..=9, true) => 40,
            (_, false) => 58,
            (_, true) => 20,
        };
        let [first, second] = p.traits();
        if first & 0x10 == 0 {
            if first & 8 != 0 || second & 2 != 0 {
                weight += weight / 2;
            }
            if id == marked && !booked {
                weight += weight / 2;
            }
        } else {
            weight /= 2;
        }
        for _ in 0..=weight / 10 {
            list.push(slot as u8);
        }
    }
    list
}

/// The chooser's foul (`FUN_0065b0b5`, side two's `FUN_0065bd27`): only a
/// side with more than eight men on the pitch; one throw over the
/// candidates names the man, an empty place nobody; under
/// `35 - 2·added` of a hundred the kind comes from `FUN_00674794`'s
/// table, else from the man's own (`FUN_006746aa`), and the card books.
fn booking(m: &mut Match, which: usize, records: &Records, random: &mut Random) {
    if m.sides[which].men <= 8 {
        return;
    }
    let eleven = pitch_eleven(m, which, records);
    let list = foul_candidates(m, which, &eleven, records);
    if list.is_empty() {
        return;
    }
    let slot = list[random.below(list.len() as u32) as usize] as usize;
    let Some(p) = eleven.get(slot).filter(|&&id| id != 0).and_then(|&id| records.player(id)) else {
        return;
    };
    let kind = if (random.below(100) as i32) < 0x23 - 2 * m.added_draw as i32 {
        foul_kind(&FOULS_OTHER, random.below(6), 0x20)
    } else {
        let second = p.traits()[1];
        let (table, width) = if second & 2 != 0 {
            (&FOULS_HARD, 0x24)
        } else if second & 0x20 != 0 {
            (&FOULS_TRAIT, 0x1c)
        } else if matches!(p.club()[0], 0x10 | 0x39) {
            (&FOULS_COUNTRY, 0x1b)
        } else {
            (&FOULS, 0x21)
        };
        foul_kind(table, random.below(width), 1)
    };
    let booked = cautioned(&m.sides[which], p);
    book_card(m, which, p.id(), &eleven, kind, booked);
}

/// The kinds a straight red is drawn from (`FUN_00674825`): the plain
/// man's `DAT_008a0728` (width 0x66), a man of the places 9 and 10 (live
/// +3) `DAT_008a0770` (0x52), the second trait bit 2 (live +0x12)
/// `DAT_008a07b8` (0x79); and `FUN_006748ea`'s `DAT_008a0800` (width 7).
/// A walk past 0x21 entries names 1.
const REDS: [i16; 36] = [
    0, 0, 2, 0, 4, 0, 0, 0, 20, 4, 6, 2, 1, 10, 8, 2, 1, 1, 1, 2, 6, 2, 1, 10, 10, 2, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0,
];
const REDS_FRONT: [i16; 36] = [
    0, 0, 2, 0, 4, 0, 0, 0, 0, 4, 6, 2, 1, 10, 8, 2, 1, 1, 1, 2, 6, 2, 1, 10, 10, 2, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0,
];
const REDS_HARD: [i16; 36] = [
    0, 0, 2, 0, 8, 0, 0, 0, 20, 10, 8, 6, 4, 10, 8, 2, 1, 1, 1, 2, 6, 2, 1, 10, 10, 2, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0,
];
const REDS_OTHER: [i16; 36] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0,
];

/// The chance frame's straight red, roll 699 (`FUN_0065b894`, side two's
/// `FUN_0065bf36`): only a side with more than nine men; the foul's
/// candidates (`FUN_00674c12`) and one throw name the man, an empty
/// place nobody. He lifts the match's bookings, and under
/// `100 - 6·added` of four hundred his kind comes from the other table
/// (`FUN_006748ea`, counted at +0x3d), else from his own
/// (`FUN_00674825`); his live +0x57 takes 0x20, his place empties, the
/// side's men drop by one and its straight reds rise, a mood above
/// nought takes 105 of a hundred, and before the 31st minute the foul
/// mark 0x20 stands. The kind goes only to the news and the books
/// (`FUN_00674e74`, `FUN_0073ebb6`); seams as at `book_card`'s
/// sending-off (`FUN_0070497d`, `FUN_00414b25`, `FUN_004131b0`,
/// `FUN_004fc3c8`, the strengths' `FUN_006f794a` pair, `FUN_006f6c20`,
/// `FUN_006f6c35`) and the national tie's +0x474 block with its keeper's
/// stand-in at +0x112.
fn straight_red(m: &mut Match, which: usize, records: &Records, minute: i16, random: &mut Random) -> Option<i16> {
    if m.sides[which].men <= 9 {
        return None;
    }
    let eleven = pitch_eleven(m, which, records);
    let list = foul_candidates(m, which, &eleven, records);
    if list.is_empty() {
        return None;
    }
    let slot = list[random.below(list.len() as u32) as usize] as usize;
    let p = eleven.get(slot).filter(|&&id| id != 0).and_then(|&id| records.player(id))?;
    m.bookings += 1;
    let other = (random.below(400) as i32) < 100 - 6 * m.added_draw as i32;
    let kind = if other {
        foul_kind(&REDS_OTHER, random.below(7), 1)
    } else if p.traits()[1] & 2 != 0 {
        foul_kind(&REDS_HARD, random.below(0x79), 1)
    } else if matches!(p.positions()[0], 9 | 10) {
        foul_kind(&REDS_FRONT, random.below(0x52), 1)
    } else {
        foul_kind(&REDS, random.below(0x66), 1)
    };
    let side = &mut m.sides[which];
    book_in(&mut side.sent_off, p.id());
    side.men -= 1;
    side.straight_reds += 1;
    if 0 < side.morale {
        side.morale = (side.morale as i32 * 0x69 / 100) as i16;
    }
    if other {
        side.other_reds += 1;
    }
    if minute < 31 {
        side.foul_marks |= 0x20;
    }
    Some(kind)
}

/// The injury kinds' rows as the match's injury reads them
/// (`DAT_008ac340`, ten bytes a kind, kinds 0 to 0x43): byte 0 whether a
/// lingering knock of the kind comes back — 1 as itself, 2 as the worse
/// kind `FUN_00766c1b` names —, byte 1 the base days, byte 5 the weight
/// of the draw.
const INJURY_RECURS: [u8; 0x44] = [
    0, 0, 0, 1, 1, 1, 1, 0, 1, 2, 0, 1, 1, 0, 2, 1, 0, 0, 1, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 1, 1, 1, 0, 0, 0,
    1, 1, 1, 1, 0, 1, 1, 1, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
const INJURY_DAYS: [u8; 0x44] = [
    56, 56, 182, 28, 14, 31, 21, 56, 21, 21, 35, 21, 7, 28, 28, 21, 56, 42, 10, 10, 28, 28, 140, 14, 84, 7, 3, 182, 7,
    49, 140, 3, 7, 21, 21, 42, 35, 112, 28, 21, 5, 7, 49, 7, 7, 14, 14, 3, 49, 21, 10, 84, 7, 28, 42, 49, 14, 14, 14,
    42, 42, 2, 28, 14, 21, 56, 42, 112,
];
const INJURY_WEIGHTS: [u8; 0x44] = [
    1, 1, 1, 2, 3, 2, 3, 1, 2, 2, 2, 3, 2, 2, 2, 2, 2, 1, 2, 2, 1, 1, 1, 1, 1, 3, 2, 0, 2, 1, 2, 1, 4, 4, 2, 1, 3, 1, 2,
    4, 3, 4, 1, 1, 3, 4, 1, 1, 1, 1, 0, 2, 1, 3, 0, 1, 2, 3, 1, 1, 1, 1, 2, 3, 2, 2, 2, 1,
];
/// The kinds a club with its flag bit 2 (club +0xfd, team +0x874) draws
/// only above 74 of a hundred.
const SPARED_KINDS: [u8; 6] = [6, 0xc, 0x21, 0x27, 0x2d, 0x39];
/// The ids of men (`FUN_00767686` skips the places above).
const MEN_BELOW: i16 = 0x4269;

/// A man's lingering knock as the live object holds it: his days out
/// (+0x2c, reckoned from the record's end) one to six and a kind
/// (+0x2e, the record's +0x228) under 0x3e — its row's byte 0, else
/// nought.
fn lingering(records: &Records, p: &Player) -> (u8, u8) {
    let days = crate::load::days_to(&records.globals, p.injured_to());
    let kind = p.injury_kind();
    if (1..7).contains(&days) && kind != 0 && kind < 0x3e {
        (kind, INJURY_RECURS[usize::from(kind)])
    } else {
        (kind, 0)
    }
}

/// The injury's candidates (`FUN_00767686`): each place of the eleven
/// with a man stands ten times, seventeen with a lingering knock; the
/// knock bit 2 (record +0x218, the live +0x14) halves that, else the
/// bit 4 adds two. The man at the side's +0x18 (+0x74 for the first,
/// +0x268 for the second — `FUN_006782aa` says which the club is) adds
/// twenty, the second side's fifty; no ported walk fills that field, so
/// nobody draws the extra.
fn injury_candidates(eleven: &[u16], records: &Records) -> Vec<u8> {
    let mut list = Vec::new();
    for (slot, &id) in eleven.iter().enumerate().take(11) {
        if id == 0 || MEN_BELOW <= id as i16 {
            continue;
        }
        let Some(p) = records.player(id) else { continue };
        let mut weight: u16 = if lingering(records, p).1 != 0 { 0x11 } else { 10 };
        if p.knocks() & 2 != 0 {
            weight /= 2;
        } else if p.knocks() & 4 != 0 {
            weight += 2;
        }
        list.extend(std::iter::repeat_n(slot as u8, usize::from(weight)));
    }
    list
}

/// The injury's kind (`FUN_00766c1b`): a lingering knock comes back on
/// one throw of two — its own kind, or for a row's byte 2 the kinds 9,
/// 14 and 29 as 10, 16 and 30 —, else a draw over the kinds 1 to 61 by
/// their weights, a club with its flag bit 2 throwing a hundred for each
/// of `SPARED_KINDS` and keeping it only above 74; the walk stops one
/// band late as `training_day`'s does. A torn muscle (0x25) of a man
/// under 35 fresh is the worse 0x43.
fn injury_kind(records: &Records, p: &Player, flags: u8, random: &mut Random) -> u8 {
    let (kind, recurs) = lingering(records, p);
    let mut drawn = 0;
    if recurs != 0 && random.below(2) != 0 {
        drawn = if recurs == 2 {
            match kind {
                9 => 10,
                0xe => 0x10,
                0x1d => 0x1e,
                _ => 0,
            }
        } else {
            kind
        };
    }
    if drawn == 0 {
        let mut kinds = Vec::new();
        let mut starts = Vec::new();
        let mut total = 0i32;
        for k in 1..0x3e_u8 {
            if flags & 2 != 0 && SPARED_KINDS.contains(&k) && random.below(100) <= 0x4a {
                continue;
            }
            kinds.push(k);
            starts.push(total);
            total += i32::from(INJURY_WEIGHTS[usize::from(k)]);
        }
        let roll = random.below(total as u32) as i32;
        let mut at = 0;
        while at + 1 < kinds.len() && starts[at] <= roll {
            at += 1;
        }
        drawn = kinds[at];
    }
    if drawn == 0x25 && crate::computer_training::fresh_percent(p) < 0x23 {
        drawn = 0x43;
    }
    drawn
}

/// The chance frame's injury, rolls 900 to 903 (`FUN_0065cd8a`, side
/// two's `FUN_0065ce98`): a throw over the candidates names the place,
/// and its man takes the kind `injury_kind` draws (`FUN_00766eb9`) for
/// the kind's base days as the club's doctors shorten them
/// (`training_day::cared_days`), from today (+0x2a, `DAT_00b5cbd4`);
/// kinds 0x1b and 0x1e may leave the knee knock (`FUN_007675c6`), and
/// the live object goes back to the record (`FUN_006364cf`). The
/// place's spot on the pitch (`FUN_00696cc5`, +0xae8/+0xae6) and the
/// match's own injury event (`FUN_0065c8b1` — the man's way off, the
/// news) are seams. Returns the man and his kind.
fn injury(m: &mut Match, which: usize, records: &mut Records, random: &mut Random) -> Option<(u16, u8)> {
    let at = which * 2;
    let team = records.team(m.ids[at], m.ids[at + 1])?;
    let (flags, spent) = (team.get::<u8>(training_day::CLUB_FLAGS), team.get::<i32>(MEDICAL_SPENT));
    let eleven = pitch_eleven(m, which, records);
    let list = injury_candidates(&eleven, records);
    if list.is_empty() {
        return None;
    }
    let slot = usize::from(list[random.below(list.len() as u32) as usize]);
    let id = eleven[slot];
    let p = records.player(id)?;
    let kind = injury_kind(records, p, flags, random);
    if kind == 0 {
        return None;
    }
    let days = training_day::cared_days(spent, INJURY_DAYS[usize::from(kind)], random);
    let today = records.globals.day();
    training_day::injure(records, id, kind, days, i32::from(today));
    if let Some(date) = records.globals.date_of(today as i16) {
        training_day::knee_knock(records, id, kind, date, random);
    }
    Some((id, kind))
}
/// The club's medical spending (club +0x2ac, team +0x314).
const MEDICAL_SPENT: usize = 0x314;

/// The card (`FUN_0065a6b0`, side two's `FUN_0065ab3a`, here with the
/// sixth parameter nought and the seventh one): a man standing in the
/// eleven lifts the match's bookings at +0x45a. His first card cautions
/// him and lifts the side's counters, the kinds 4 and 12 its hard
/// fouls; a cautioned man goes off: his place empties, the side's men
/// drop by one, its reds rise, and a mood above nought takes 105 of a
/// hundred (an x87 division the integer one matches). Seams: the
/// national tie's news and books (`FUN_0073e712`, `FUN_0073e963`,
/// `FUN_006fcbba`, `FUN_006fabf0`, `FUN_00702eee`) and its keeper's
/// stand-in at +0x112, the sending-off's own calls (`FUN_0070492e`,
/// `FUN_00414b25`, `FUN_004131b0`, `FUN_006fad22`, `FUN_004fc3c8`) and
/// the two sides' strengths recomputed (`FUN_006f794a` twice,
/// `FUN_006f6c20`, `FUN_006f6c35`), which the replica's walk takes as
/// the pre-match walk left them.
fn book_card(m: &mut Match, which: usize, id: u16, eleven: &[u16], kind: i16, booked: bool) {
    if !eleven.iter().take(11).any(|&x| x == id) {
        return;
    }
    m.bookings += 1;
    let side = &mut m.sides[which];
    if !booked {
        book_in(&mut side.booked, id);
        side.cautions[0] += 1;
        side.caution_bits |= 1;
        side.cautions[1] += 1;
        if kind == 4 || kind == 12 {
            side.hard_fouls += 1;
        }
    } else {
        book_in(&mut side.sent_off, id);
        side.men -= 1;
        side.reds += 1;
        if 0 < side.morale {
            side.morale = (side.morale as i32 * 0x69 / 100) as i16;
        }
        if kind == 4 || kind == 12 {
            side.foul_marks |= 0x10;
        }
    }
}

/// The minute's chance frame, `FUN_006f6007`: what the chooser picked runs
/// as a foul through the match's club (`FUN_0065b0b5`, `FUN_0065bd27`,
/// see `booking`); then both sides' chance comes out of their rated support and
/// attack, scaled to a common 92 together, the ground's owner lifting the
/// first side's attack an eighth. A roll under the side's share opens the
/// chance — its shape by the minute kind and the shot's quality — and the
/// frame closes with the bookings the last minute left.
fn chance_minute(m: &mut Match, clock: &Clock, mut records: Option<&mut Records>, random: &mut Random) {
    // The chooser's set moment becomes a foul at the match's club:
    // FUN_0065b0b5 when it picked the first side, FUN_0065bd27 the second
    // (each through the club slot FUN_0063dd1c); without the save there
    // is no club to book.
    let picked = chance_chooser(m, random);
    if let (1 | 2, Some(records)) = (picked, records.as_deref()) {
        booking(m, picked as usize - 1, records, random);
    }
    // The four chance weights: support and attack per side, as floats.
    let mut w = [
        [m.sides[0].rated[3] as f32, m.sides[0].rated[2] as f32],
        [m.sides[1].rated[3] as f32, m.sides[1].rated[2] as f32],
    ];
    let total = w[0][0] + w[0][1] + w[1][0] + w[1][1];
    if 2.0 < total {
        let f = 92.0 / total;
        for pair in w.iter_mut() {
            for x in pair.iter_mut() {
                *x *= f;
            }
        }
    }
    let mut attack = w[0][0] + w[0][1];
    if m.ids[1] == 200 {
        if m.ids[0] as u8 == m.home_nation {
            attack *= 1.08;
        }
    } else if m.home_nation == m.ids[0] as u8 && m.home_id == m.ids[1] {
        attack *= 1.08;
    }
    let defend = w[1][0] + w[1][1];
    chance_for(m, 0, attack, clock.minute, records.as_deref_mut(), random);
    chance_for(m, 1, defend, clock.minute, records, random);
    // What the last minute's booking left (`FUN_00413115`): the match-day
    // state takes it (`FUN_00412cf2` per side, its bit +0x80 saying
    // red), then both marks clear; `FUN_0070c1a9` keeps the live clocks —
    // not ported.
    m.cards = [0, 0];
    let _ = clock;
}

/// One side's half of the frame: a thousand-wide roll against its share;
/// past it (or past fifteen goals for the side) no chance, though the
/// roll's own marks set events off: 699 the straight red (`straight_red`),
/// 900..903 the injury (`injury`). Inside, the
/// minute kind under 90 is a plain chance — the quality-one shot runs the
/// `FUN_00659d2b` event (side two's `FUN_0065a3e1`) and counts twice, once
/// in the event and once in the frame; every other imperfect shot counts
/// once — 90..97 sends the `FUN_00659b85` set piece and from 98 the big
/// chance `FUN_00659945` runs, each with its taker picked out of the
/// saved records.
fn chance_for(
    m: &mut Match,
    which: usize,
    weight: f32,
    minute: i16,
    records: Option<&mut Records>,
    random: &mut Random,
) {
    let roll = random.below(1000);
    if weight <= roll as f32 || 15 < m.goals[which] {
        // roll 699: FUN_0065b894 (side two's FUN_0065bf36), each through
        // the club slot FUN_0063dd1c — without the save no club to book;
        // 899 < roll < 904: FUN_0065cd8a / FUN_0065ce98, the injury.
        match (roll, records) {
            (699, Some(records)) => {
                straight_red(m, which, records, minute, random);
            }
            (900..=903, Some(records)) => {
                injury(m, which, records, random);
            }
            _ => {}
        }
        return;
    }
    let kind = random.below(100);
    let quality = shot_quality(m, which, random);
    if kind < 90 {
        if quality == 1 {
            // The shot event raises the byte itself; the frame raises once
            // more, so a quality-one chance costs the side two counts.
            let men = records.as_deref().map(|r| (side_men(m, which, r), side_men(m, 1 - which, r)));
            shot_event(
                m,
                which,
                men.as_ref().map(|(a, d)| (a.as_slice(), d.as_slice())),
                random,
            );
            m.sides[which].chances += 1;
        } else if quality < 4 {
            m.sides[which].chances += 1;
        }
    } else if kind < 98 {
        set_piece(m, which, quality, records.and_then(|r| set_taker(m, which, r)));
    } else {
        big_chance(m, which, records.and_then(|r| big_taker(m, which, r)), random);
    }
}

/// The set piece of a kind 90..97 chance (`FUN_00659b85`, and side two's
/// `FUN_0065a23a`): with its taker found the side's chance is counted, and
/// a perfect shot — quality 1, drawn back in the frame — is a goal; the
/// event itself draws nothing. The taker's chances counter follows his
/// shot book as `FUN_00740a66` follows `FUN_00740a07`, and a perfect
/// shot his own counters as `FUN_00740aad` behind it, under kind one —
/// the free kick — both waiting in the side's book lists; at the event's
/// open `FUN_00740a07` lifts the taker's free-kick byte at live +0x4e,
/// the record's +0x24d first byte, and the club's taken pair at live
/// +0xdec, the record's +0x334 — waiting in the side's shot list; the
/// post-goal talk
/// (`FUN_0065e118`) and the crowd's shout (`FUN_0069e224`, `FUN_0069e2b9`,
/// and `FUN_004fc297` for a national side) are seams; none draws either. A human side's
/// taker is the manager's designated man: his club's saved direct free
/// kick list, read by `FUN_0069ac7d` and put in every reader's order by
/// `FUN_006dc332` — see `set_taker` below.
fn set_piece(m: &mut Match, which: usize, quality: u8, taker: Option<u16>) {
    let Some(taker) = taker else {
        return;
    };
    // `FUN_00740a07` stands at the event's very open: the attempt lifts
    // the taker's taken free-kick byte and the club's taken pair, the
    // perfect shot's goal only the scored one beside them
    book_shot(m, which, taker, false);
    // `FUN_00740a66` follows the shot book before the shot is judged:
    // every found taker takes the chance, the goal or miss aside
    book_in(&mut m.sides[which].chance_book, taker);
    if quality == 1 {
        // `FUN_00740aad` kind one behind it: the perfect set-piece shot
        // is the scorer's free kick goal
        book_goal(m, which, taker, 1);
        score(m, which);
    }
    m.sides[which].chances += 1;
}

/// The big chance from kind 98 (`FUN_00659945`, side two's `FUN_00659ff9`):
/// with its taker found, one hundred-wide roll; under 0x50 the first
/// side's chance is a goal, under 0x46 only the second's — the visitor's
/// big chance lies more often. A chance is counted either way, and the
/// taker's shot stands in the record's ledger before the roll —
/// `FUN_00740996` marks him whichever way the ball goes; its counters —
/// the man's taken penalty byte at live +0x4c, the record's +0x24b first
/// byte, and the club's awarded pair at live +0xdea, the record's +0x332
/// — wait in the side's shot list;
/// the taker's chances counter (`FUN_00740a66` right behind the book)
/// waits in the side's book list; the goal is his own counters
/// (`FUN_00740aad`) under kind nought — the penalty — waiting in the
/// side's goal list, while the miss's book (`FUN_00740f33`,
/// lifting the taken penalty byte where the scored one would stand in the
/// scorer's own books) waits its name in the side's penalty list; the
/// defending line-up's first man — its keeper — takes the chance
/// himself: the miss branch lifts his saved byte, the goal branch the
/// conceded one (`FUN_00741038`, `FUN_00740fb6`), waiting in the side's
/// keeper list; his mood byte shifts with both, a live byte with no
/// save copy — as with `FUN_00740f33`. As in the set piece a human side's man comes from his
/// club's list — for the big chance the penalty list, read by
/// `FUN_0069b17b`; see `big_taker` below.
fn big_chance(m: &mut Match, which: usize, taker: Option<u16>, random: &mut Random) {
    let Some(taker) = taker else {
        return;
    };
    book_event(m, taker, LEDGER_SHOT);
    // `FUN_00740996`'s counters ride with that ledger mark, before the
    // roll: the attempt lifts the taker's taken penalty byte and the
    // club's awarded pair — a wide chance lifts the taken byte once
    // more in the miss branch below, so the wide penalty's byte moves
    // twice, the scored one's once
    book_shot(m, which, taker, true);
    book_in(&mut m.sides[which].chance_book, taker);
    if (random.below(100) as i32) < if which == 0 { 0x50 } else { 0x46 } {
        // `FUN_00740aad` kind nought at the goal: the big chance stands
        // as the man's penalty in his own books
        book_goal(m, which, taker, 0);
        score(m, which);
        // right after the goal write, `FUN_00740fb6` gives the foe
        // keeper the conceded byte
        book_keeper(m, which, false);
    } else {
        // the original's miss branch lifts the taker's taken counter
        // (`FUN_00740f33`) and the foe keeper's saved one
        // (`FUN_00741038`); the scored one would stand in the scorer's
        // own books (`FUN_00740aad` kind nought) in the goal list
        book_in(&mut m.sides[which].penalty_book, taker);
        book_keeper(m, which, true);
    }
    m.sides[which].chances += 1;
}

/// One more name in a side's waiting book list, first free slot; ids
/// never stand at nought. Eighteen places serve one side's goals for a
/// whole walk, as the ledger's slots serve its men.
fn book_in(list: &mut [u16; LEDGER_SLOTS], id: u16) {
    if let Some(slot) = list.iter_mut().find(|s| **s == 0) {
        *slot = id;
    }
}

/// One more scorer in a side's waiting goal list, with his shot's kind —
/// first free slot; as in `book_in` the id never stands at nought.
fn book_goal(m: &mut Match, which: usize, id: u16, kind: i16) {
    if let Some(slot) = m.sides[which].goal_book.iter_mut().find(|s| s.0 == 0) {
        *slot = (id, kind);
    }
}

/// The foe keeper of a big chance: the original reads the defending
/// club's line-up block, its first short naming the keeper, and lifts
/// him in both of the event's branches — `FUN_00741038` his saved byte
/// at the miss, `FUN_00740fb6` the conceded one at the goal, both
/// behind the usual kind gate and asking a club man by the player
/// object's type byte +0x3. The replica's walk enters the side's
/// formation in the entry slots before its first minute
/// (`fill_entries`), the keeper first — the saved eleven's first place,
/// `matches::eleven` standing when none is saved — so the defending
/// side's first entered man stands for the line-up's first short, and
/// its nought matches the line-up's; a national tie's keeper fails the
/// type byte, and the replica stands national ties off the books
/// through `nat_bits` besides.
fn book_keeper(m: &mut Match, which: usize, saved: bool) {
    let id = m.sides[1 - which].entries[0];
    if id == 0 {
        return;
    }
    if let Some(slot) = m.sides[which].keeper_book.iter_mut().find(|s| s.0 == 0) {
        *slot = (id, saved);
    }
}

/// One more shot taker in a side's waiting shot list, first free slot,
/// with the shot's name — true the big chance's penalty, false the set
/// piece's free kick. `FUN_00740996` at the big chance's open and
/// `FUN_00740a07` at the set piece's both lift the taker's taken byte —
/// live +0x4c and +0x4e, the records' +0x24b and +0x24d first bytes —
/// and the attacking club's own counters at live +0xdea and +0xdec,
/// behind the usual kind gate. The club's scored byte beside them lifts
/// not with the attempt but at the goal — `FUN_007410ca`'s and
/// `FUN_007411ea`'s `+0xdeb` and `+0xdee`, standing in the side's goal
/// list behind kind nought and kind one — and a wide penalty lifts its
/// taken byte once more in the miss branch (`FUN_00740f33`).
fn book_shot(m: &mut Match, which: usize, id: u16, penalty: bool) {
    if let Some(slot) = m.sides[which].shot_book.iter_mut().find(|s| s.0 == 0) {
        *slot = (id, penalty);
    }
}

/// The quality-one shot's event (`FUN_00659d2b`, side two's `FUN_0065a3e1`):
/// one two-hundred-wide roll; on its very first throw the foe heads it into
/// his own keeper's net — one own man drawn out of the defending eleven
/// (`FUN_0077e3e8`) and the goal counts for the attacking side. Otherwise
/// the scorer and his assist come out of the attacking eleven
/// (`FUN_0077db3a`), the shot's outcome is drawn (`FUN_00740899`: under 30
/// a headed goal, under 50 a struck one, else wide or saved — the books'
/// concern alone) and the goal write (`FUN_007410ca`/`FUN_007411ea`) lands
/// unconditionally: a quality-one shot not turned into an own goal is a
/// goal, and only a scorer-less eleven keeps the net clean. The event's own
/// chance count closes it. A found own man is marked in the record's
/// ledger (`FUN_00740d68` before the goal write) — the first event to book
/// one; this event marks no shot: the shot's ledger bit comes from the big
/// chances and the plain event of qualities two and three (`FUN_00659311`,
/// not ported). The assist's and own-goal man's counters (`FUN_00740dc4`,
/// and `FUN_00740d68` beyond its ledger bit) stand booked through the
/// side's lists; a found scorer takes the chance counter too, as
/// `FUN_00740a66` gives the set pieces' and big chances' takers, and the
/// scorer's own counters (`FUN_00740aad`) follow right behind under the
/// outcome's kind — headed, struck, or the fifth when the draw names no
/// kind, the goal write standing either way — both waiting in the side's
/// book lists; the post-goal talk
/// (`FUN_0065e118`) and the crowd's shout (`FUN_0069e224`, `FUN_0069e2b9`,
/// `FUN_004fc297` for a national tie) draw nothing.
fn shot_event(
    m: &mut Match,
    which: usize,
    men: Option<(&[&Player], &[&Player])>,
    random: &mut Random,
) {
    if let Some((att, def)) = men {
        if random.below(200) == 0 {
            let own = own_goal_pick(def, random);
            if own != 0 {
                book_event(m, own, LEDGER_OWN);
                // `FUN_00740d68`'s other half: the blamed man, a man of
                // the defending side, takes the own goal in his byte.
                book_in(&mut m.sides[1 - which].own_book, own);
                score(m, which);
            }
        } else {
            let (scorer, assist) = scorer_pick(att, random);
            // FUN_00740899's outcome: under 30 headed, under 50 struck,
            // else no kind — the scorer's books and the goal write stand
            // either way
            let outcome = random.below(100) as i16;
            if scorer != 0 {
                // `FUN_00740a66` first, then `FUN_00740aad`: the scorer
                // takes his chance count, and his books learn the kind
                book_in(&mut m.sides[which].chance_book, scorer);
                book_goal(
                    m,
                    which,
                    scorer,
                    if outcome < 0x1e { 2 } else if outcome < 0x32 { 3 } else { -1 },
                );
                // `FUN_00740dc4` runs beside the scorer's books whenever
                // the pick named an assist.
                if assist != 0 {
                    book_in(&mut m.sides[which].assist_book, assist);
                }
                score(m, which);
            }
        }
    }
    m.sides[which].chances += 1;
}

/// The man's rating in the pick (`FUN_006a144e`): his class in tens bent by
/// the position he's stood at and his age's curve, capped at 1400 in tenths
/// — the original reads the runtime object's bytes +2, +3..+5 and +0xca,
/// whose save-side age curve is not mapped, and the man's runtime marks
/// (`f_003`, `f_00c`: worth two apiece, the 0x80 a fifth more) sit in no
/// record. `strength` ten times over plus his form today stands in, the
/// taker pick's own measure.
fn shot_rating(p: &Player) -> i32 {
    (i32::from(p.strength()) * 10 + statistics::form_today(p)).clamp(0, 1_400)
}

/// The shot's band of the man's position kind (`FUN_00696cc5`'s field
/// classifier, then `FUN_0077db3a`'s bands): the keeper takes no shot at
/// goal at all, the back pair stands at one, the half pair at one or two
/// (two when the slot sits on the half line — the walk plays the
/// manager's eleven but the band still reads his main position, not the
/// formation's spot at the record's +0x194), the half-forward pair at three,
/// the runner at two, the forward at four, the spearhead at six; the
/// original drops a marked man on the side's bit at +0x40 one band down.
fn shot_band(kind: u8) -> i32 {
    match kind {
        0 => 2, // unclassified — the classifier's fallback band
        1 => 0,
        2 | 3 | 4 | 5 => 1,
        6 | 7 => 3,
        8 => 2,
        9 => 4,
        10 => 6,
        _ => 2,
    }
}

/// Scorer and assist of the shot (`FUN_0077db3a`): every slot of the
/// attacking club's eleven at +0xa9a — its keeper bandless and out of the
/// list — weighs `band × (rating + 5)`, the fitness trainer's own man at
/// seven-tenths, each line's strength a factor (`FUN_0077e590`: the two
/// sides' six line shorts at +0x7c.. and +0x270.. apart, three base values
/// 30/40/30 — or 35/30/35 for the attacking style — bent a third of the way
/// by each line's deviation, thence `pow(x/100, 0.33)` for the scorer's
/// list and `sqrt(x/100)` for the assist's). Those line shorts wait with
/// their writer — not `FUN_0078b26c`, which reckons the crowd — and with
/// them the factors fall away: both
/// lists carry the same whole-number weights. The scorer is drawn below the
/// first list's total — first cumulative above the draw, the head when
/// none; the assist below the second list's, the scorer lifted out with the
/// list rebased below his weight and the total with it. With no listed man
/// nothing is drawn at all.
fn scorer_pick(men: &[&Player], random: &mut Random) -> (u16, u16) {
    let mut ids: Vec<u16> = Vec::new();
    let mut cums: Vec<i32> = Vec::new();
    for &p in men {
        let band = shot_band(p.positions()[0]);
        if band == 0 {
            continue;
        }
        let weight = band * (shot_rating(p) + 50) / 10;
        cums.push(cums.last().unwrap_or(&0) + weight);
        ids.push(p.id());
    }
    if ids.is_empty() {
        return (0, 0);
    }
    let total = cums.last().copied().unwrap_or(0).max(0);
    let draw = random.below(total as u32);
    let mut j = 0;
    while j < ids.len() && cums[j] <= draw as i32 {
        j += 1;
    }
    if j == ids.len() {
        j = 0;
    }
    let scorer = ids[j];
    if ids.len() == 1 {
        return (scorer, 0);
    }
    // lift the scorer out: the list above him shifts down by his weight
    let own = if j == 0 { cums[0] } else { cums[j] - cums[j - 1] };
    for i in j + 1..ids.len() {
        cums[i] -= own;
    }
    ids.remove(j);
    cums.remove(j);
    let total = cums.last().copied().unwrap_or(0).max(0);
    let draw = random.below(total as u32);
    let mut k = 0;
    while k < ids.len() && cums[k] <= draw as i32 {
        k += 1;
    }
    (scorer, ids[if k == ids.len() { 0 } else { k }])
}

/// The own-goal man out of the defending eleven (`FUN_0077e3e8`): each
/// slot weighed by its kind's danger (`OWN_GOAL_W`, the keeper and the
/// unclassified at nought) times hundred against the man's rating; one draw
/// below the total, first cumulative above it, the head when none; nothing
/// drawn when the weighted list stands empty.
fn own_goal_pick(men: &[&Player], random: &mut Random) -> u16 {
    let mut ids: Vec<u16> = Vec::new();
    let mut cums: Vec<i32> = Vec::new();
    for &p in men {
        let kind = p.positions()[0] as usize;
        let danger = OWN_GOAL_W.get(kind).copied().unwrap_or(0);
        if danger == 0 {
            continue;
        }
        let weight = danger * (1_000 - shot_rating(p)) / 10;
        cums.push(cums.last().unwrap_or(&0) + weight);
        ids.push(p.id());
    }
    if ids.is_empty() {
        return 0;
    }
    // a better man than hundred weighs pulls his own-goal danger below
    // nought; the draw then simply has no choice left
    let total = cums.last().copied().unwrap_or(0).max(0);
    let draw = random.below(total as u32);
    let mut j = 0;
    while j < ids.len() && cums[j] <= draw as i32 {
        j += 1;
    }
    ids[if j == ids.len() { 0 } else { j }]
}

/// The goal as the match record takes it (`FUN_007410ca`, mirrored by
/// `FUN_007411ea`): the side's count at +0x0e/+0x10, and a sixth of the
/// two sides' standing at +0x66/+0x25a apart added to the minute's mood.
/// Who fills that standing is not ported, so it holds at nought and the
/// mood stands; the scorer's books and the clubs' counters wait.
fn score(m: &mut Match, which: usize) {
    let drift = (m.sides[which].nerve - m.sides[1 - which].nerve) / 6;
    m.sides[which].morale += drift as i16;
    m.goals[which] += 1;
}

/// What a man did in the match, kept on the record beside his entry slot
/// (`FUN_0065847f`): the id is looked for in both sides' entered lists,
/// and the bits are set in the ledger byte of every slot that names him;
/// a man the record does not name books nothing. The events book him as
/// they happen — the big chances their taker's shot before the roll
/// (`FUN_00740996`, in both sides' events), the quality-one event the
/// own-goal man (`FUN_00740d68`); the plain event of qualities two and
/// three (`FUN_00659311`, not ported) books shots the same way.
fn book_event(m: &mut Match, id: u16, bits: u8) {
    for i in 0..LEDGER_SLOTS {
        if m.sides[0].entries[i] == id {
            m.sides[0].ledger[i] |= bits;
        }
        if m.sides[1].entries[i] == id {
            m.sides[1].ledger[i] |= bits;
        }
    }
}

/// Whether the record names this man as the one who put the ball in his
/// own net (`FUN_0065850d`, what the halftime talk asks to pick its
/// subject): the two sides' lists are walked slot by slot together, and
/// the first list naming the man answers with bit 4 of its ledger byte.
pub fn own_goal_mark(m: &Match, id: u16) -> bool {
    for i in 0..LEDGER_SLOTS {
        if m.sides[0].entries[i] == id {
            return m.sides[0].ledger[i] & LEDGER_OWN != 0;
        }
        if m.sides[1].entries[i] == id {
            return m.sides[1].ledger[i] & LEDGER_OWN != 0;
        }
    }
    false
}

/// The side's team record's saved formation, when it lists anybody: the
/// manager's — or the co-trainer's, [`line_up_row`] — eleven at the
/// record's +0x148 and the bench at +0x15e
/// (`Player ids of the eleven and the bench, 0 an empty place`). The
/// save's club block carries the same eleven at +0xa9a: the loader
/// `FUN_0063b75a` copies the 0xaa bytes there from the record
/// (134811ff), the save-back copies them home (134954), and the walk
/// copies the eleven into the match's side block at +0x8e
/// (`memcpy(side + 0x8e, club + 0xa9a, 0x16)`, _global.c 221842), an
/// empty place copied as nought — a squad too short for the co-trainer's
/// system, or a human's listed man gone since, leaves the side a man
/// down there. A record listing nobody — a club the row's line-up did
/// not reach — stands none.
fn saved_formation(team: &crate::records::Team) -> Option<([u16; 11], [u16; 7])> {
    (team.lineup().iter().any(|&id| id != 0)).then(|| (team.lineup(), team.bench()))
}

/// The side's eleven on the pitch: the eleven its record lists, the
/// manager's or the co-trainer's of [`line_up_row`], an empty place
/// nought; the replica's strongest when it lists nobody (the stand-in
/// in `crate::matches::eleven`, for a walk run without the row's
/// line-up). The original keeps the formation's place order; the
/// list's first short is the keeper, and the taker fallbacks read
/// exactly that (`+0xa9a`'s first place). The line-up screen's
/// substitution helpers — they replace an injured or suspended listed
/// man — run when the manager visits, not in the walk; the replica
/// stands the listed man as saved.
pub(crate) fn saved_eleven(m: &Match, which: usize, records: &Records) -> Vec<u16> {
    let at = which * 2;
    let team = match records.team(m.ids[at], m.ids[at + 1]) {
        Some(team) => team,
        None => return Vec::new(),
    };
    match saved_formation(team) {
        Some((lineup, _)) => lineup.to_vec(),
        None => crate::matches::eleven(&team.players(), records),
    }
}

/// The side's men on the pitch, as records: its eleven in the
/// formation's place order (or the stand-in's strongest), each looked
/// up in its record, an empty place left out.
fn side_men<'a>(m: &Match, which: usize, records: &'a Records) -> Vec<&'a Player> {
    pitch_eleven(m, which, records)
        .into_iter()
        .filter_map(|id| records.player(id))
        .collect()
}

/// Enter the side's men in the record's entered slots (the original's
/// +0xfa, filled from the line-up at runtime +0x8e — the saved eleven,
/// the manager's or the co-trainer's of [`line_up_row`]). The original
/// enters eighteen men, eleven on the pitch and seven at the reserve's
/// place: the saved formation's eleven and bench, the record's +0x148
/// and +0x15e, an empty place nought. A record listing nobody has the
/// replica's stand-in enter its eleven and the rest nought. The slots are
/// where `book_event` hangs its ledger bytes, and the halftime dialog
/// walks them. The walk's sixth loop enters them first, with its throws
/// ([`enter_sides`]); this fill, for a row walked without it, draws
/// nothing.
fn fill_entries(m: &mut Match, which: usize, records: &Records) {
    let at = which * 2;
    let ids = records
        .team(m.ids[at], m.ids[at + 1])
        .and_then(|t| saved_formation(&t))
        .map(|(lineup, bench)| {
            let mut ids: Vec<u16> = lineup.to_vec();
            ids.extend_from_slice(&bench);
            ids
        })
        .unwrap_or_else(|| saved_eleven(m, which, records));
    for (slot, id) in m.sides[which].entries.iter_mut().zip(ids) {
        *slot = id;
    }
}

/// The designated man for a duty, as the original picks him at a human
/// club: the manager's saved list (`FUN_0069ac7d`/`FUN_0069b17b`), put in
/// every reader's order (`FUN_006dc332`, here [`duties::from_ids`] and
/// [`duties::normalise`]), and the first listed man standing in the
/// eleven — the set piece scans only behind the keeper's slot, the big
/// chance from that slot itself. With no listed man on the pitch the
/// eleven's first man takes it, as the original's fallback at +0xa9a. The
/// ordered list's copy back into the club object (`FUN_0069ad46`/
/// `FUN_0069b268`) touches the runtime record alone, and a national
/// side's lists sit in the national block the replica does not read —
/// those ties and the computer clubs, which list nothing, fall to the
/// argmax. The pick draws no number.
fn designated_taker(m: &Match, which: usize, records: &Records, duty: Duty, from: usize) -> Option<u16> {
    if m.nat_bits != 0 {
        return None;
    }
    let at = which * 2;
    let team = records.team(m.ids[at], m.ids[at + 1])?;
    let extra = records.extras.get(usize::from(team.human()).checked_sub(1)?)?;
    let squad = team.players();
    let list = duties::from_ids(&squad, &extra.duty_lists()[duty.stored()]);
    // normalise orders a squad by strength and main position; it reads
    // nothing else of the men, whose names the save's record keeps.
    let men: Vec<world::Player> = squad
        .iter()
        .map(|&id| {
            let p = records.player(id);
            world::Player {
                last_name: String::new(),
                first_name: String::new(),
                strength: p.map_or(0, |p| p.strength() as u8),
                positions: p.map_or([0; 3], |p| p.positions()),
                abilities: [0, 0],
                traits: [0, 0],
                birthday: None,
                shirt: 0,
                feet: 0,
            }
        })
        .collect();
    let eleven = pitch_eleven(m, which, records);
    let pitch = eleven.get(from..).unwrap_or(&[]);
    duties::normalise(&men, &list)
        .into_iter()
        .filter_map(|p| squad.get(p).copied())
        .find(|id| pitch.contains(id))
        .or_else(|| eleven.first().copied())
}

/// The big chance's man: at a human club the manager's penalty list, at
/// a computer club the argmax below — the original branches on the
/// manager's number at the club's +0x8a (`FUN_005ded54`).
fn big_taker(m: &Match, which: usize, records: &Records) -> Option<u16> {
    designated_taker(m, which, records, Duty::Penalty, 0)
        .or_else(|| ai_big_taker(&side_men(m, which, records)))
}

/// The big chance's man at a computer club (`FUN_005ded54`, +0x8a
/// nought): the highest rated of the eleven — strength ten times over
/// plus his form today
/// (`statistics::form_today`, the very `FUN_006ff037` the lists show) —
/// the keeper half so fit, the good word's bit 0x10 a hundred better, and
/// a marked weakness in the bad word leaving a man only his doubled
/// strength. Nobody above nought found means no taker at all; the
/// original scans the club's saved eleven at +0xa9a and its keeper byte.
fn ai_big_taker(men: &[&Player]) -> Option<u16> {
    let mut best = -1i32;
    let mut pick = 0;
    for &p in men {
        let worth = i32::from(p.strength()) * 10 + statistics::form_today(p);
        let rating = if p.positions()[0] == world::GOALKEEPER {
            worth / 2
        } else if p.good() & BIG_WORD != 0 {
            worth + 100
        } else if p.bad() != 0 {
            i32::from(p.strength()) << 1
        } else {
            worth
        };
        if best < rating {
            best = rating;
            pick = p.id();
        }
    }
    (pick != 0).then_some(pick)
}

/// The set piece's man: at a human club the manager's direct free kick
/// list, at a computer club the argmax below (`FUN_005def60`).
fn set_taker(m: &Match, which: usize, records: &Records) -> Option<u16> {
    designated_taker(m, which, records, Duty::ShotFreeKick, 1)
        .or_else(|| ai_set_taker(&side_men(m, which, records)))
}

/// The set piece's man at a computer club (`FUN_005def60`, +0x8a nought):
/// the same rating — free kicks, the good word's bit 0x20, a hundred
/// better — but the keeper stands unrated and is chosen only when nothing
/// else beats him. No weakness is marked down here.
fn ai_set_taker(men: &[&Player]) -> Option<u16> {
    let mut best = -1i32;
    let mut pick = 0;
    for &p in men {
        let rating = if p.positions()[0] == world::GOALKEEPER {
            0
        } else {
            let worth = i32::from(p.strength()) * 10 + statistics::form_today(p);
            if p.good() & FREE_KICKS != 0 {
                worth + 100
            } else {
                worth
            }
        };
        if best < rating {
            best = rating;
            pick = p.id();
        }
    }
    (pick != 0).then_some(pick)
}

/// How good the chance's shot is (`FUN_0040b8c9`): the sides' attitudes
/// apart set the odds, an attacker far behind the defender's hold shoots
/// worse. First the attacker's strength of the shot against the defender's
/// hold — under it, quality 1; then two hundred-wide rolls bent by the
/// margin give 2 or 3, else 4.
fn shot_quality(m: &Match, attacker: usize, random: &mut Random) -> u8 {
    let atk_side = &m.sides[attacker];
    let def_side = &m.sides[1 - attacker];
    let d = (atk_side.attitude as i32 - def_side.attitude as i32) * -3;
    let odds = if def_side.attitude == 4 && atk_side.attitude != 4 {
        d + 88
    } else if def_side.attitude == 0 && atk_side.attitude != 0 {
        d + 82
    } else {
        d + 85
    };
    let r4 = atk_side.rated[4] as i32;
    let r1 = def_side.rated[1] as i32;
    let margin = if r4 + 0xa0 < r1 {
        -8
    } else if r4 + 0x50 < r1 {
        -4
    } else if r4 - 0x50 < r1 {
        0
    } else if r4 - 0xa0 < r1 {
        4
    } else {
        8
    };
    let shot = (r4 * ((odds * atk_side.shot_pwr as i32) / 100)) / 100;
    let struck = random.below(shot.max(0) as u32);
    let hold = random.below(r1.max(0) as u32);
    if hold < struck {
        return 1;
    }
    if (random.below(100) as i32) < margin + 40 {
        return 2;
    }
    if (random.below(100) as i32) < margin / 2 + 40 {
        return 3;
    }
    4
}

/// Which side sits behind on rated strength (`FUN_00414f4e`), asked by the
/// trailing talk: the second side when its five rated numbers sum under the
/// first's, else the first.
fn trailing(m: &Match) -> usize {
    let sum = |s: &Side| -> i32 { s.rated.iter().map(|x| *x as i32).sum() };
    if sum(&m.sides[1]) < sum(&m.sides[0]) {
        1
    } else {
        0
    }
}

/// The trailing side's morale talk (`FUN_0041321d`): only a lead past one
/// goal or a minute past 49 lets the minute ask at all, and then one roll
/// in three. Each side with talks left and a staff answer runs the talk;
/// a successful one lifts the side (its staff question `FUN_004134d0`
/// needs the club's squad and assistant, `FUN_0040a1ec` — the staff
/// piece), and the lift is what adds to the talk count and recomputes the
/// side's +0x444 (`FUN_006f6c35`).
fn morale_talk(m: &mut Match, clock: &Clock, random: &mut Random) {
    let l = lead(m);
    if !(1 < l.abs() || 0x31 < clock.minute) {
        return;
    }
    if random.below(3) != 0 {
        return;
    }
    let behind = trailing(m);
    for which in 0..2 {
        let side = &mut m.sides[which];
        if side.talks as i16 >= side.talk_max {
            continue;
        }
        if side.assistant != 0 {
            // FUN_004134d0(side block, club, ±lead, `behind` mirrored as
            // 1 - behind on the second side, clock): the staff question
            // whose answer lifts the side — until it lands no talk lifts,
            // so the count stays put. On a lift the original adds here,
            // runs FUN_006f6c20 and FUN_006fae69, re-draws the +0x444
            // strength difference (FUN_006f6c35) and the live news.
            let _ = (l, behind, which);
        } else if m.nat_bits & (4 << which) == 0 {
            // FUN_0042c574 seam: the club takes the talk itself for a
            // side standing in no regional selection.
        }
    }
}

/// The walk's penalty minute for a match flagged `PENS` (`FUN_005ddea7`
/// run right after the minute walk in the original's day loop): the tie
/// settles on penalties over the sides' men as the save's records keep
/// them — the kicks read the men's years, so the minute needs the day's
/// date. Without the records the minute comes to nothing and draws no
/// number, as the pickers' events do.
fn penalties(m: &mut Match, records: Option<&Records>, today: NaiveDate, random: &mut Random) {
    let Some(records) = records else {
        return;
    };
    let att = side_men(m, 0, records);
    let def = side_men(m, 1, records);
    penalty_shootout(m, (&att, &def), today, random);
}

/// Whether an id is a national side (`FUN_00677428`) — not ported; every
/// fixture runs the club path until it is.
fn is_national(_id: u16) -> bool {
    false
}

/// The stoppage the half gets (`FUN_0040bc1b`): the record's byte at +0x451
/// down two places picks the draw — a tight one of −1..1, one centred at 0,
/// at −1, at −2 or at −3.
fn added_time(m: &Match, random: &mut Random) -> i16 {
    match m.added_draw >> 2 {
        0 => random.below(3) as i16 - 1,
        1 => random.below(3) as i16 - 2,
        2 => random.below(4) as i16 - 3,
        _ => random.below(5) as i16 - 4,
    }
}

/// Whether the tie is level and so needs more play — `FUN_00414e2e` at nought:
/// no extra time allowed ends the match, a single leg is level on goals,
/// and over two legs the first leg's goals carry in (`FUN_00677ed4`).
fn level(m: &Match) -> bool {
    if m.kind & MAY_EXTRA == 0 {
        return false;
    }
    if m.kind & TWO_LEGS == 0 {
        return m.goals[0] == m.goals[1];
    }
    let a = m.leg[0] as i16 + m.goals[1];
    let c = m.leg[1] as i16 + m.goals[0];
    a == c && m.goals[1] == m.leg[1] as i16
}

/// What side one leads by over the tie, `FUN_00414e9f`.
pub fn lead(m: &Match) -> i32 {
    if m.kind & TWO_LEGS == 0 {
        m.goals[0] as i32 - m.goals[1] as i32
    } else {
        (m.leg[1] as i32 + m.goals[0] as i32) - m.leg[0] as i32 - m.goals[1] as i32
    }
}

/// Full time, `FUN_006f3343`: pack the half's score, end what may not go
/// on, mark what must; true when any match goes to extra time.
fn time_out(matches: &mut [Match], live: bool) -> bool {
    let mut extra = false;
    for m in matches.iter_mut() {
        if live || m.nat_bits == 0 {
            m.half_score = (((m.goals[0] as i32) << 4) as i8).wrapping_add(m.goals[1] as u16 as i8);
            if m.kind & MAY_EXTRA == 0 {
                m.flags |= OVER;
            } else if level(m) {
                m.flags &= !MINUTE_UP;
                m.flags |= EXTRA;
                extra = true;
            } else {
                m.flags |= OVER;
            }
        }
    }
    extra
}

/// After extra time, `FUN_006f345f`: end what the legs have settled, and
/// send the rest to penalties; true when any match goes there.
fn after_extra_time(matches: &mut [Match], live: bool) -> bool {
    let mut pens = false;
    for m in matches.iter_mut() {
        if (live || m.nat_bits == 0) && m.flags & OVER == 0 {
            if level(m) {
                m.flags |= PENS;
                pens = true;
            } else {
                m.flags |= OVER;
            }
        }
    }
    pens
}

/// The side's pressure band (`FUN_006f8e73`): its fatigue at +0x2c divided
/// by the ninety minutes — a hundred and twenty once the match went to
/// extra time — and whatever stands past ninety to the minute is banded in
/// fives, never past the top band.
fn wear_band(fatigue: i16, extra: bool) -> usize {
    let minutes = if extra { 120 } else { 90 };
    (((i32::from(fatigue) / minutes - 90) / 5).clamp(0, 4)) as usize
}

/// What the match leaves on one played man (`FUN_006f8b3f`): the row of
/// his minutes band scaled by ten through the side's multiplier lifts his
/// condition, kept at 0..10000 by `FUN_00654ff0`, and moves the three
/// freshness values, kept by `FUN_006fc330` at 0..10000 on the first and
/// −10000..10000 on the other two. The original halves the multiplier for
/// the men at player byte +0x28 == 1, and takes it down to eight tenths
/// when the club's fitness trainer — the runtime staff array `DAT_00a58402`
/// behind the club's slot at +0x11de, asked by `FUN_0069bec0` — is exactly
/// this man; both wait with the club objects and their staff, seam here.
fn wear_man(p: &mut Player, minutes: i32, mult: i32) {
    wear_by_rows(p, minutes, mult, &WEAR_ROWS);
}

/// [`wear_man`] by the `rows` the match's kind takes.
fn wear_by_rows(p: &mut Player, minutes: i32, mult: i32, rows: &[[i32; 4]; 5]) {
    let band = if minutes < 30 {
        0
    } else if minutes < 60 {
        1
    } else if minutes < 80 {
        2
    } else if minutes < 100 {
        3
    } else {
        4
    };
    let row = rows[band];
    let scaled = |d: i32| d * mult / 10;
    let cond = i32::from(p.condition()) + scaled(row[0]);
    p.set_condition(cond.clamp(0, 10_000) as i16);
    let mut fresh = p.freshness();
    for i in 0..3 {
        let (low, high) = if i == 0 { (0, 10_000) } else { (-10_000, 10_000) };
        let moved = i32::from(fresh[i]) + scaled(row[i + 1]);
        fresh[i] = moved.clamp(low, high) as i16;
    }
    p.set_freshness(fresh);
}

/// What a played match leaves on the men who stood on its pitch: the plain
/// pass the day loop runs over every finished match before the graded one
/// (`FUN_006f8f44` with the fifth parameter nought, its `FUN_006f8e73`
/// walking each club's squad at +0xb50). Nothing is drawn here — the wear
/// is arithmetic on the save. The original weighs each man's minutes byte
/// at +0x58, which `FUN_006f8dc7` fills from the played flags at +0x57:
/// the eleven stand ninety minutes, a hundred and twenty with extra time,
/// and a substitute at minute m plays ninety minus m; the replica knows no
/// substitutions yet, so its formation's eleven stands whole and its
/// bench — the saved seven at the record's +0x15e — stays untouched.
pub fn post_match_wear(m: &Match, records: &mut Records) {
    let extra = m.flags & EXTRA != 0;
    let minutes = if extra { 120 } else { 90 };
    for which in 0..2 {
        let mult = WEAR_MULT[wear_band(m.sides[which].fatigue, extra)];
        for id in saved_eleven(m, which, records) {
            if let Some(p) = records.player_mut(id) {
                wear_man(p, minutes, mult);
            }
        }
    }
}

/// What the plain pass leaves on a walked game of the day's own list
/// (`FUN_006f8f44` with the fifth parameter nought on a record whose
/// +0x24 is set, `FUN_006f8e73` over both sides): each side's eleven by
/// the friendly rows ([`FRIENDLY_WEAR_ROWS`]) for the whole game, the
/// multiplier by the side's pressure band as its minutes summed it
/// (+0x2c), halved for the man at the keeper's spot. The scorers' marks
/// the pass reads beside (`FUN_006244c3`, `FUN_0062445b` over the men's
/// +0xc3 with both sides' strengths on the day) wait with the live men.
fn exhibition_wear(m: &Match, records: &mut Records) {
    let extra = m.flags & EXTRA != 0;
    let minutes = if extra { 120 } else { 90 };
    for which in 0..2 {
        let mult = WEAR_MULT[wear_band(m.sides[which].fatigue, extra)];
        for id in saved_eleven(m, which, records) {
            if let Some(p) = records.player_mut(id).filter(|_| id != 0) {
                let mult = if p.positions()[0] == world::GOALKEEPER { mult / 2 } else { mult };
                wear_by_rows(p, minutes, mult, &FRIENDLY_WEAR_ROWS);
            }
        }
    }
}

/// The talks a side may hold in a game of the day's own list
/// (`DAT_00b5d168`, laid on both sides by `FUN_006f3b7a`).
const EXHIBITION_TALKS: usize = 0xb5d168;

/// The friendlies' walk's second loop (`FUN_006f3b7a`), one game of the
/// day's own list: both sides' men entered with their throws
/// ([`enter_sides`], `FUN_00658cfb`), the kick-off
/// ([`kick_off_strengths`], `FUN_006f6fc7`) whose strength on the day
/// each club keeps (club +0xdf2, stored back to the team record's +0x304
/// by `FUN_0063df96`), each computer side's tactics against the other by
/// it (`FUN_0042c1aa`, the home side first) and the clubs' tactics laid
/// over the sides (`FUN_006f7d4d`); the talks are the option's. Unlike
/// the competitions' walk (`FUN_006ef296`) it lays no chance slot,
/// added-time byte, staff or referee slot (+0x450, +0x451, +0x150,
/// +0x458 keep the cleared record's nought), opens no press window,
/// weighs neither the record against the foe nor an airfield, and draws
/// no first half's added time: the half ends on the 45th minute.
pub fn exhibition_kick_off(m: &mut Match, records: &mut Records, data: &crate::data::Data, random: &mut Random) {
    enter_sides(m, records, random);
    kick_off_strengths(m, records, data, random);
    for which in 0..2 {
        if let Some(team) = records.team_mut(m.ids[which * 2], m.ids[which * 2 + 1]) {
            team.set_day_strength(m.sides[which].day[0]);
        }
    }
    let clubs = [0, 1].map(|w| (m.ids[w * 2] as u8, m.ids[w * 2 + 1] as i16));
    let ahead = i32::from(m.sides[0].day[0].wrapping_sub(m.sides[1].day[0]));
    let behind = i32::from(m.sides[1].day[0].wrapping_sub(m.sides[0].day[0]));
    crate::exhibition::computer_tactics(records, clubs[0], clubs[1], true, ahead, random);
    crate::exhibition::computer_tactics(records, clubs[1], clubs[0], false, behind, random);
    let talks = records.globals.get::<i16>(EXHIBITION_TALKS);
    for which in 0..2 {
        let side = &mut m.sides[which];
        if let Some(team) = records.team(m.ids[which * 2], m.ids[which * 2 + 1]) {
            side.style = team.tactic_flags();
            side.tilt = team.style();
            side.stance = team.effort();
            side.tier = team.hardness();
        }
        side.talk_max = talks;
    }
}

/// What a friendly or a tournament's game leaves on a side's men
/// (`FUN_006f3b7a`'s last loop, `FUN_006f8f44` with the fifth parameter
/// nought): the walk takes the side's stance from the club's effort
/// (`FUN_006f7d4d`, club +0xa9 into side +0x32), the fatigue
/// sums the stance's percentage over the ninety minutes — the hundred and
/// twenty of extra time —, and its band names the multiplier, halved for
/// the man at the keeper's spot (his +0x28, `FUN_006f6fc7`); the friendly
/// rows (the record's +0x24 set) by each man's minutes. The eleven the
/// team keeps stand the whole game, the replica's games knowing no
/// substitutions yet.
pub fn friendly_wear(records: &mut Records, (country, number): (u8, i16), extra: bool) {
    let Some(team) = u16::try_from(number).ok().and_then(|n| records.team(country.into(), n)) else { return };
    let minutes = if extra { 120 } else { 90 };
    let pct = MINUTE_PCT.get(usize::from(team.effort())).copied().unwrap_or(100);
    let mult = WEAR_MULT[wear_band((pct * minutes) as i16, extra)];
    for id in team.lineup() {
        if let Some(p) = records.player_mut(id).filter(|_| id != 0) {
            // The man byte +0x28 at one halves it: the keeper's.
            let mult = if p.positions()[0] == world::GOALKEEPER { mult / 2 } else { mult };
            wear_by_rows(p, minutes, mult, &FRIENDLY_WEAR_ROWS);
        }
    }
}

/// A man's grade for a club match, in half steps 2 (a one) to 12 (a
/// six) — `FUN_006f7f6f` with the match's national marker +0x474 nought:
/// six and a draw below four, twice what his form fell from the kick-off
/// (the runtime +0xb7 that `FUN_006f7af1`'s reset took from
/// `FUN_006ff037`) to now, one or two better for a win by one or two or
/// by more than two — worse by as much for such a loss —, one better at
/// the top form of twenty. The man's runtime mark at +0xc3 above two
/// (grade one) and the high nibble of his +0xb8 (the grade held to three
/// of it) have no writer in the replica's walk and stand nought.
pub fn match_grade(kickoff_form: i32, form_now: i32, own: i16, against: i16, random: &mut Random) -> u8 {
    let mut g = random.below(4) as i32 + 6 + (kickoff_form - form_now) * 2;
    g += if own > against + 2 {
        -2
    } else if own > against {
        -1
    } else if own < against - 2 {
        2
    } else if own < against {
        1
    } else {
        0
    };
    if form_now == 20 {
        g -= 1;
    }
    g.clamp(2, 12) as u8
}

/// The grading over a side's squad (`FUN_006f86e1` in the graded pass,
/// `FUN_006f8f44` with the fifth parameter one), in the club's squad
/// order at +0xb50: a man of twenty minutes and more takes his grade
/// ([`match_grade`], kept at the record's +0x273), one of fewer a
/// nought; the side's mean in tenths goes to the match side's +0x3a, and
/// the best grade among the men of more than forty-four minutes names
/// the match's best men, all its ties. The replica's eleven stand the
/// whole match. The best man's talks (`FUN_0069e008` and its kin) and the
/// clubs' ledgers (`FUN_00714236`, `FUN_00744d02`) wait with the club
/// objects. Returns the mean and the best men.
pub fn post_match_grades(
    m: &Match,
    which: usize,
    kickoff: &[(u16, i32)],
    records: &mut Records,
    random: &mut Random,
) -> (u8, Vec<u16>) {
    let at = which * 2;
    let Some(team) = records.team(m.ids[at], m.ids[at + 1]) else { return (0, Vec::new()) };
    let minutes = if m.flags & EXTRA != 0 { 120 } else { 90 };
    let (own, against) = (m.goals[which], m.goals[1 - which]);
    let (mut sum, mut count, mut best, mut men) = (0i32, 0i32, 0xffu8, Vec::new());
    for id in team.players() {
        let Some(&(_, form)) = kickoff.iter().find(|(man, _)| *man == id && id != 0) else { continue };
        let Some(p) = records.player_mut(id) else { continue };
        if minutes < 0x14 {
            p.set_grade(0);
            continue;
        }
        let g = match_grade(form, statistics::form_today(p), own, against, random);
        p.set_grade(g);
        sum += i32::from(g);
        count += 1;
        if minutes > 0x2c {
            if g < best {
                best = g;
                men.clear();
                men.push(id);
            } else if g == best {
                men.push(id);
            }
        }
    }
    let mean = if count < 1 { 0 } else { (sum * 10 / count) as u8 };
    (mean, men)
}

/// The record bit of a smoker (player +0x1b4 bit 11): his condition
/// counts five steps less (`FUN_006fffd9` in `FUN_00457e00`).
const SMOKER: u32 = 0x800;

/// What a league or cup match leaves on a side's squad: the graded pass
/// (`FUN_006f8f44` with the fifth parameter one, as the day's matches
/// walk `FUN_006f0efd` calls it; `FUN_006f86e1` over the club's squad
/// at +0xb50, in its order). A man who stood on the pitch wears by the
/// competition rows ([`WEAR_ROWS`], the record's +0x24 nought) for the
/// minutes `played` names him, the multiplier by the side's pressure band
/// — its stance the club's effort as in [`friendly_wear`], held the whole
/// match — halved at the keeper's spot; a man who did not
/// (`position == 0`) loses one condition step ([`sat_out`]). The original's
/// eight tenths for the man its human club's fitness trainer looks after
/// (`FUN_0069bec0`, the human slot's runtime staff at `DAT_00a58402`)
/// waits with those staff objects; the computer clubs answer nought.
pub fn competition_wear(records: &mut Records, (country, number): (u8, u16), played: &[(u16, i32)], extra: bool) {
    let Some(team) = records.team(country.into(), number) else { return };
    let squad = team.players();
    let full = if extra { 120 } else { 90 };
    let pct = MINUTE_PCT.get(usize::from(team.effort())).copied().unwrap_or(100);
    let mult = WEAR_MULT[wear_band((pct * full) as i16, extra)];
    for id in squad {
        let Some(p) = records.player_mut(id).filter(|_| id != 0) else { continue };
        match played.iter().find(|(man, _)| *man == id) {
            Some(&(_, minutes)) => {
                let mult = if p.positions()[0] == world::GOALKEEPER { mult / 2 } else { mult };
                wear_by_rows(p, minutes, mult, &WEAR_ROWS);
            }
            None => sat_out(p),
        }
    }
}

/// The squad's men a walked match left off its pitch, each a sat-out
/// step ([`sat_out`]), the walk having worn those who stood
/// ([`post_match_wear`]).
pub fn sat_out_rest(records: &mut Records, (country, number): (u8, u16), stood: &[u16]) {
    let Some(team) = records.team(country.into(), number) else { return };
    for id in team.players() {
        if id == 0 || stood.contains(&id) {
            continue;
        }
        if let Some(p) = records.player_mut(id) {
            sat_out(p);
        }
    }
}

/// A competition match's man who sat it out (`FUN_006f8d88`, the record's
/// +0x24 nought): one condition step down (`FUN_00411130(-1)`, kept at
/// 0..10000) while his steps (`FUN_00457e00`, the condition by the
/// hundred, a smoker's five less) stand above nought. The original asks
/// `FUN_00747998` first: no step for a man whose runtime byte +0xb5 is 4,
/// and at a human club none for one its manager's individual plan
/// (`FUN_00746ddf`, `DAT_00a50028`) holds with a task; neither has a save
/// copy found, so every man takes the step.
fn sat_out(p: &mut Player) {
    let steps = i32::from(p.condition()) / 100;
    let steps = if p.shown() & SMOKER != 0 { if steps < 5 { 0 } else { steps - 5 } } else { steps };
    if steps > 0 {
        p.set_condition((i32::from(p.condition()) - 100).clamp(0, 10_000) as i16);
    }
}

/// What a walked match leaves in the players' counters — chances,
/// penalties taken, assists, own goals and the scorer's own books: the
/// names the shot event booked on the side's waiting lists, written into
/// the records once the walk is done — the replica keeps no live player
/// objects for the goal's moment, where the original lifts the man's
/// bytes. `FUN_00740dc4` sets an assist man's two bytes +0x48 and +0x4b
/// one higher, which the save keeps as the record's assist and
/// last-assist counters; `FUN_00740d68` beyond its ledger bit sets the
/// blamed man's byte +0x47, the record's own-goal counter; and
/// `FUN_00740a66`, right behind every set-piece and big-chance shot
/// book, sets the taker's byte +0x52, the record's chances counter at
/// +0x251; and `FUN_00740f33` in the big chance's miss branch sets the
/// taker's byte +0x4c, the record's penalties-taken byte at +0x24b —
/// its own lowering of the man's runtime mood byte +0x65 has no save
/// copy and stays a seam; and `FUN_00740aad`, right behind the found
/// scorer in the shot event and behind the goal of the set piece and the
/// big chance, lifts the scorer's own books: first the goal mark, the
/// record's byte at +0x277, before every gate — its meaning beyond the
/// counters not traced — then within the kind gate below the last match's
/// goal at +0x249, the season's league goal at +0x23c, the count the
/// strength upgrade weighs at +0x274, the home goal at +0x252 for a man
/// of the first side (the match's second short against the scoring
/// club's number byte at runtime +0x103), and the substitute's goal at
/// +0x248 when his match bits carry bit 2 — raised by the line-up's
/// substitution helpers, not ported, so the byte stands as the save has
/// it. The kind names his goal bucket at +0x241: penalty with the scored
/// penalty byte at +0x24c, free kick with the scored free kick at
/// +0x24e, headed, struck, and the fifth the lists never name for a shot
/// whose outcome draw named no kind. Beside the scorers the keeper list
/// books the big chances' foes: the defending side's first entered man
/// takes their chances, `FUN_00741038` lifting his saved byte at +0x24f
/// in the miss branch, `FUN_00740fb6` the conceded one at +0x250 in the
/// goal branch; both sit behind the same kind gate and ask a club man,
/// which every entered man of a club fixture is. The mood byte both
/// branches shift, live +0x65, has no save copy, and `FUN_006a784b`
/// beside the miss's mood lift teaches in [`post_match_lessons`]. Beside the keeper list
/// the shot list books both events' takers: the shot books' own counters
/// lift the taker's taken penalty or free-kick byte — the +0x24b and
/// +0x24d first bytes — and the club at the side's own id pair adds
/// every attempt to its season pair at +0x332 and +0x334, the awarded
/// penalty and the taken free kick, while the goal list's kind nought
/// and kind one add the scored byte standing beside them,
/// `FUN_007410ca`'s `+0xdeb` and `FUN_007411ea`'s `+0xdee` lifts at the
/// goal; a wide penalty's taken byte stands lifted twice, once by the
/// shot book and once by the miss branch above. The lessons behind those
/// buckets (`FUN_006a76df`, `FUN_006a7737`, `FUN_006a778f`) teach in
/// [`post_match_lessons`]; the club counters (`FUN_006facd8`,
/// `FUN_007408d1`) and the talks `FUN_00677c87` and
/// `FUN_00740959` stay seams; the tail that reads which side the man
/// stands in and marks him there (`FUN_006782aa`, `FUN_006fac42`) works
/// only for national sides — the id pair returns nought for clubs — so a
/// plain fixture passes it by. All these lifts run
/// only while the match's kind code sits below 0x3f or stands at 0x10 —
/// a national tie carries its counters in its own block instead — and
/// the match's short at +0x24 stands nought — [`play_matches`] passes a
/// game of the day's own list by (`Match::exhibition`); as with the
/// wear, the replica stands national ties off the books through the
/// tie's `nat_bits`. Nothing here draws a number.
pub fn post_match_books(m: &Match, records: &mut Records) {
    for which in 0..2 {
        // the club's season pair counts the side's attempts and its
        // scored kinds; the club lift stands at the shot and at the
        // goal whether or not the man's own record answers
        let (mut awarded, mut pens_scored) = (0u8, 0u8);
        let (mut fks_taken, mut fks_scored) = (0u16, 0u16);
        for &id in m.sides[which].chance_book.iter().take_while(|&&id| id != 0) {
            if let Some(p) = records.player_mut(id) {
                p.set_chances(p.chances().wrapping_add(1));
            }
        }
        for &id in m.sides[which].penalty_book.iter().take_while(|&&id| id != 0) {
            if let Some(p) = records.player_mut(id) {
                let [taken, scored] = p.penalties();
                p.set_penalties([taken.wrapping_add(1), scored]);
            }
        }
        for &id in m.sides[which].assist_book.iter().take_while(|&&id| id != 0) {
            if let Some(p) = records.player_mut(id) {
                p.set_assists(p.assists().wrapping_add(1));
                p.set_last_assists(p.last_assists().wrapping_add(1));
            }
        }
        for &id in m.sides[which].own_book.iter().take_while(|&&id| id != 0) {
            if let Some(p) = records.player_mut(id) {
                p.set_own_goals(p.own_goals().wrapping_add(1));
            }
        }
        for &(id, kind) in m.sides[which].goal_book.iter().take_while(|&&(id, _)| id != 0) {
            // the club's scored byte counts the kind at the goal write,
            // `FUN_007410ca` and `FUN_007411ea`, whoever scored it
            if kind == 0 {
                pens_scored += 1;
            } else if kind == 1 {
                fks_scored += 1;
            }
            if let Some(p) = records.player_mut(id) {
                // `FUN_00740aad` lifts this mark before every gate, wide
                // or saved outcome aside
                p.set_scorer_marks(p.scorer_marks().wrapping_add(1));
                p.set_last_goals(p.last_goals().wrapping_add(1));
                let mut goals = p.goals();
                goals[0] = goals[0].wrapping_add(1);
                p.set_goals(goals);
                p.set_upgrade_count(p.upgrade_count().wrapping_add(1));
                // The first side's club counts its goals at home; the
                // original weighs the match's second short against the
                // scoring club's number byte — the replica reads the
                // man's own club pair, record +0x220
                if u16::from(p.club()[1]) == m.ids[1] {
                    p.set_home_goals(p.home_goals().wrapping_add(1));
                }
                if p.match_bits() & 2 != 0 {
                    p.set_joker_goals(p.joker_goals().wrapping_add(1));
                }
                // the kind's goal bucket, and the scored penalty or free
                // kick standing beside their kind
                let mut kinds = p.goal_kinds();
                match kind {
                    0 => {
                        kinds[0] = kinds[0].wrapping_add(1);
                        let [taken, scored] = p.penalties();
                        p.set_penalties([taken, scored.wrapping_add(1)]);
                    }
                    1 => {
                        kinds[1] = kinds[1].wrapping_add(1);
                        let [taken, scored] = p.free_kicks();
                        p.set_free_kicks([taken, scored.wrapping_add(1)]);
                    }
                    2 => kinds[3] = kinds[3].wrapping_add(1),
                    3 => kinds[2] = kinds[2].wrapping_add(1),
                    _ => kinds[4] = kinds[4].wrapping_add(1),
                }
                p.set_goal_kinds(kinds);
            }
        }
        for &(id, saved) in
            m.sides[which].keeper_book.iter().take_while(|&&(id, _)| id != 0)
        {
            if let Some(p) = records.player_mut(id) {
                if saved {
                    p.set_saves(p.saves().wrapping_add(1));
                } else {
                    p.set_conceded(p.conceded().wrapping_add(1));
                }
            }
        }
        for &(id, penalty) in
            m.sides[which].shot_book.iter().take_while(|&&(id, _)| id != 0)
        {
            if penalty {
                awarded += 1;
            } else {
                fks_taken += 1;
            }
            if let Some(p) = records.player_mut(id) {
                if penalty {
                    let [taken, scored] = p.penalties();
                    p.set_penalties([taken.wrapping_add(1), scored]);
                } else {
                    let [taken, scored] = p.free_kicks();
                    p.set_free_kicks([taken.wrapping_add(1), scored]);
                }
            }
        }
        // the attacking club's own season counters, the club at the
        // side's own id pair: attempts from the shot list, scored kinds
        // from the goal list
        if let Some(t) = records.team_mut(m.ids[which * 2], m.ids[which * 2 + 1]) {
            if awarded != 0 || pens_scored != 0 {
                let [att, scored] = t.penalties();
                t.set_penalties([att.wrapping_add(awarded), scored.wrapping_add(pens_scored)]);
            }
            if fks_taken != 0 || fks_scored != 0 {
                let [taken, scored] = t.free_kicks();
                t.set_free_kicks([
                    taken.wrapping_add(fks_taken),
                    scored.wrapping_add(fks_scored),
                ]);
            }
        }
    }
}

/// What a walked match teaches the human clubs' men, out of the same
/// waiting lists as [`post_match_books`]: the original teaches at the
/// event's moment, behind the same kind gate — the scorer by his goal's
/// kind (`FUN_00740aad`), the assist man (`FUN_00740dc4`) and the foe
/// keeper who saved the big chance (`FUN_00741038`), the club at the
/// keeper's own side. The lessons touch only the individual training's
/// store and the abilities, which the books leave alone, so their order
/// against the books does not matter. Nothing here draws a number.
pub fn post_match_lessons(m: &Match, records: &mut Records, tables: &IndividualTables) {
    for which in 0..2 {
        let club = (m.ids[which * 2], m.ids[which * 2 + 1]);
        let foe = (m.ids[(1 - which) * 2], m.ids[(1 - which) * 2 + 1]);
        let side = &m.sides[which];
        for &(id, kind) in side.goal_book.iter().take_while(|&&(id, _)| id != 0) {
            training_day::goal_lessons(records, tables, club, id, kind);
        }
        for &id in side.assist_book.iter().take_while(|&&id| id != 0) {
            training_day::assist_lesson(records, tables, club, id);
        }
        for &(id, saved) in side.keeper_book.iter().take_while(|&&(id, _)| id != 0) {
            if saved {
                training_day::save_lesson(records, tables, foe, id);
            }
        }
    }
}

/// Settle a tie that extra time left level, the way the day loop does for
/// every match its penalty decision leaves over (`FUN_005ddea7`, run with
/// the record and the day's clock right after the minute walk). Fewer than
/// seven men standing on either pitch and the tie is handed over whole:
/// three goals for the fuller one, one for the other — an equal count
/// gives them to the first side — and no kick is taken, no number drawn.
/// Else a coin drawn below two says which side steps up first, both sides
/// list their five takers out of their own men (the first side's list
/// filled first, `FUN_005de669`), and the kicks alternate: every goal
/// lands in the record's count and in the shootout's own, and the round
/// is given up the moment the trailing side can no longer reach the
/// other's lead. Level after those ten, the sudden death adds a round at
/// a time, its taker listed at the round's place wrapped round the side's
/// men on the pitch, until one kick lands and the other does not; once a
/// side's goals — the record's plus its shootout score, so the shootout's
/// goals count twice over — stand past `SHOOTOUT_PAST`, the dice fall
/// away and the round's first kicker misses while its second scores, the
/// original's arithmetic kept as it stands. The shootout writes nothing
/// else: the morale stays, the ledger sees no shot, and the original's
/// two book calls (`FUN_006fca60`, `FUN_006fcada`) answer for national
/// clubs alone. A national side's kicks run instead on its designated
/// takers (`FUN_005de952`) and a detailed roll (`FUN_0073e017`); both
/// wait, the replica's ties stand on clubs.
pub fn penalty_shootout(
    m: &mut Match,
    men: (&[&Player], &[&Player]),
    today: NaiveDate,
    random: &mut Random,
) {
    let men = [men.0, men.1];
    if men[0].len() < 7 || men[1].len() < 7 {
        // the original counts each club's eleven formation slots; the
        // replica counts its men lists, the same eleven once the line-up
        // has stood them
        if men[0].len() < men[1].len() {
            m.goals[0] += 1;
            m.goals[1] += 3;
        } else {
            m.goals[0] += 3;
            m.goals[1] += 1;
        }
        return;
    }
    let mut first = usize::from(random.below(2) != 0);
    let mut list = [[0u16; SHOOTOUT_SLOTS]; 2];
    for side in 0..2 {
        fill_takers(men[side], &mut list[side], 0, SHOOTOUT_TAKERS, random);
    }
    let mut kicks = [0usize; 2];
    let mut scored = [0i32; 2];
    let mut winner: Option<usize> = None;
    while winner.is_none() && kicks[0] + kicks[1] < 2 * SHOOTOUT_TAKERS {
        let taker = listed(men[first], list[first][kicks[first]]);
        if penalty_kick(taker, men[1 - first].first().copied(), 0, today, random) {
            m.goals[first] += 1;
            scored[first] += 1;
        }
        kicks[first] += 1;
        for side in 0..2 {
            let left = scored[side] + SHOOTOUT_TAKERS as i32 - kicks[side] as i32;
            if left < scored[1 - side] {
                winner = Some(1 - side);
            }
        }
        first = 1 - first;
    }
    while winner.is_none() {
        let mut slot = [0usize; 2];
        for side in 0..2 {
            slot[side] = kicks[side] % usize::from(m.sides[side].men.max(1));
            fill_takers(men[side], &mut list[side], slot[side], 1, random);
        }
        let past = i32::from(m.goals[0]) + scored[0] > SHOOTOUT_PAST
            || i32::from(m.goals[1]) + scored[1] > SHOOTOUT_PAST;
        let certain = i16::from(past);
        for _ in 0..2 {
            let taker = listed(men[first], list[first][slot[first]]);
            // the weight sits on each side by its place, not on the round
            // as a whole: the first kicker of the pair is lifted, its
            // second pressed down — over twelve the first side therefore
            // takes the round whichever way the coin fell
            let pressure = if first == 0 { certain } else { -certain };
            if penalty_kick(taker, men[1 - first].first().copied(), pressure, today, random) {
                m.goals[first] += 1;
                scored[first] += 1;
            }
            kicks[first] += 1;
            first = 1 - first;
        }
        if scored[0] != scored[1] {
            winner = Some(if scored[0] > scored[1] { 0 } else { 1 });
        }
    }
}

/// The shootout's taker picker (`FUN_005de669`'s club path): `need` more
/// men listed from `start` on, drawn one below the pool's shrinking size
/// out of the side's men not already listed ahead — five from eleven
/// gives below eleven, ten, nine, eight, seven. A pool too thin for the
/// order takes the first standing man for every slot instead, and an
/// empty pitch leaves the slots as they were. A national side hands the
/// order to its designated takers (`FUN_005de952`), which waits.
fn fill_takers(
    men: &[&Player],
    list: &mut [u16],
    start: usize,
    need: usize,
    random: &mut Random,
) {
    let mut pool: Vec<u16> =
        men.iter().map(|p| p.id()).filter(|id| !list[..start].contains(id)).collect();
    if pool.len() < need {
        if let Some(first) = men.first() {
            for slot in list[start..start + need].iter_mut() {
                *slot = first.id();
            }
        }
        return;
    }
    for i in 0..need {
        let pick = random.below(pool.len() as u32) as usize;
        list[start + i] = pool.remove(pick);
    }
}

/// The listed man by his short id; nought names nobody, and a national
/// side's designated list can name a man off the pitch — his kick then
/// finds no taker.
fn listed<'a>(men: &[&'a Player], id: u16) -> Option<&'a Player> {
    men.iter().copied().find(|p| id != 0 && p.id() == id)
}

/// One kick (`FUN_005ddcc5`'s dice for a club tie; the national path's
/// detailed roll waits): the pressure the sudden death puts on settles it
/// before anything rolls — the side it weighs down misses, the side it
/// lifts scores — and an unlisted taker misses while an empty keeper's
/// place on the foe's side buries it. Else the roll goes a hundred wide
/// under a threshold: four times the taker's age past sixty, a man unafraid
/// of the spot (`BIG_WORD` in his good word) striking from seventy past
/// four times his, one marked down for it from fifty; every year of the
/// keeper's age takes two off, and a keeper who holds his line fifteen
/// more or less by `KEEPER_GIFT` in his bad or good word. Below the
/// threshold, the ball is in. The original reads the ages off runtime
/// bytes; the replica weighs them from the men's birthdays.
fn penalty_kick(
    taker: Option<&Player>,
    keeper: Option<&Player>,
    pressure: i16,
    today: NaiveDate,
    random: &mut Random,
) -> bool {
    if pressure < 0 {
        return false;
    }
    if pressure > 0 {
        return true;
    }
    let Some(taker) = taker else {
        return false;
    };
    let Some(keeper) = keeper else {
        return true;
    };
    let years = crate::autolineup::age(taker.birthday(), today);
    let mut thr = years * 4 + 0x3c;
    if taker.positions()[0] != world::GOALKEEPER {
        if taker.good() & BIG_WORD != 0 {
            thr = years * 4 + 0x46;
        } else if taker.bad() & BIG_WORD != 0 {
            thr = years * 4 + 0x32;
        }
    }
    thr -= crate::autolineup::age(keeper.birthday(), today) * 2;
    if keeper.positions()[0] == world::GOALKEEPER {
        if keeper.good() & KEEPER_GIFT == 0 && keeper.bad() & KEEPER_GIFT != 0 {
            thr += 0xf;
        } else if keeper.good() & KEEPER_GIFT != 0 {
            thr -= 0xf;
        }
    }
    (random.below(100) as i32) < thr
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plain fixture: both sides alike, one leg, the stance that changes
    /// nothing (index 4 sits at 100 %), the tier a league fixture stands
    /// at in the original's week (tier 2, class 0), and everything the
    /// pre-match walk lays: slot 5, added byte 5, staff on, no referee.
    fn fixture() -> Match {
        let side = Side {
            strengths: [100, 100, 100, 100, 100],
            strength: 100,
            stance: 4,
            tilt: 3,
            tier: 2,
            ..Side::default()
        };
        let mut m = Match { sides: [side, side], ends_at: 0, ..Match::default() };
        pre_match(std::slice::from_mut(&mut m));
        m
    }

    /// The day the fixtures stand on; the shootout reads the men's years
    /// against it, and the men the tests give them are years on it.
    fn test_day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2000, 6, 15).unwrap()
    }

    #[test]
    fn a_goalless_afternoon_ends_by_the_book() {
        let mut m = fixture();
        let mut r = Random::new(12345);
        play_matches(std::slice::from_mut(&mut m), false, None, None, test_day(), &mut r);
        // the added time now sits behind the minutes' chance draws in the
        // stream, so it is checked in its band, not by pre-drawing it:
        // the plain fixture's byte 5 gives selector one, 88 up
        assert!((88..=90).contains(&m.ends_at), "{}", m.ends_at);
        assert_eq!(m.flags, MINUTE_UP | OVER);
        assert_eq!(m.goals, [0, 0]);
        assert_eq!(m.half_score, 0);
    }

    #[test]
    fn added_time_stays_in_its_band() {
        let mut m = fixture();
        for (sel, low, high) in [(0u8, 89, 91), (1, 88, 90), (2, 87, 90), (3, 86, 90)] {
            m.added_draw = sel << 2;
            let mut r = Random::new(7);
            for _ in 0..12 {
                let added = added_time(&m, &mut r);
                assert!(
                    (low - 90..=high - 90).contains(&(added as i32)),
                    "{sel}: {added}"
                );
            }
        }
    }

    #[test]
    fn the_walk_packs_the_half_and_draws_added_time_for_every_match() {
        let mut b = fixture();
        let mut a = fixture();
        a.goals = [3, 2];
        a.kind = MAY_EXTRA | TWO_LEGS;
        a.leg = [2, 3];
        b.added_draw = 2 << 2; // its band is the wider one, 87 up
        let mut ms = [b, a];
        let mut r = Random::new(99);
        play_matches(&mut ms, false, None, None, test_day(), &mut r);
        assert!((87..=90).contains(&ms[0].ends_at), "{}", ms[0].ends_at);
        assert!((88..=90).contains(&ms[1].ends_at), "{}", ms[1].ends_at);
        assert_eq!(ms[1].half_score, 0x32);
        assert_eq!(ms[1].score10, [3, 2]); // the goals stood before the walk
        assert_eq!(ms[1].score90, [3, 2]);
    }

    #[test]
    fn a_level_single_leg_goes_the_whole_way() {
        let mut m = fixture();
        m.kind = MAY_EXTRA;
        m.goals = [2, 2];
        let mut r = Random::new(5);
        play_matches(std::slice::from_mut(&mut m), false, None, None, test_day(), &mut r);
        assert_eq!(m.flags, MINUTE_UP | EXTRA | PENS);
        // the extra time's fresh added-time draw, behind the chances,
        // adds the plain fixture's selector one to 120
        assert!((118..=120).contains(&m.ends_at), "{}", m.ends_at);
    }

    #[test]
    fn a_decided_single_leg_never_goes_on() {
        let mut m = fixture();
        m.kind = MAY_EXTRA;
        m.goals = [2, 1];
        let mut r = Random::new(5);
        play_matches(std::slice::from_mut(&mut m), false, None, None, test_day(), &mut r);
        assert_eq!(m.flags, MINUTE_UP | OVER);
    }

    #[test]
    fn the_legs_carry_over_a_level_night() {
        let mut m = fixture();
        m.kind = MAY_EXTRA | TWO_LEGS;
        // level on the legs' totals yet decided by the first leg's marks
        m.leg = [1, 1];
        m.goals = [0, 0];
        assert!(!level(&m));
        assert_eq!(lead(&m), 0);
        m.goals = [1, 1];
        assert!(level(&m));
        m.leg = [0, 0];
        assert!(!level(&m));
        m.goals = [0, 0];
        assert!(level(&m));
    }

    #[test]
    fn the_finished_match_keeps_its_minute_out() {
        let mut m = fixture();
        m.flags = FINISHED;
        m.goals = [3, 2];
        let mut clock = Clock::default();
        let mut r = Random::new(1);
        tick(std::slice::from_mut(&mut m), false, 10, &mut clock, None, test_day(), &mut r);
        assert_eq!(m.score10, [0, 0]);
        // a half-2 mark is taken whatever the match's own state
        clock = Clock { half2: 1, minute: 90, ..Clock::default() };
        tick(std::slice::from_mut(&mut m), false, 90, &mut clock, None, test_day(), &mut r);
        assert_eq!(m.score90, [3, 2]);
    }

    #[test]
    fn strengths_follow_the_stance_and_tilt() {
        let mut m = fixture();
        m.sides[0].strengths = [100, 200, 300, 400, 500];
        let mut clock = Clock::default();
        let mut r = Random::new(1);
        minute_play(&mut m, &clock, None, &mut r);
        assert_eq!(m.sides[0].rated, [100, 300, 300, 400, 500]);
        m.sides[0].stance = 0; // 110 %
        m.sides[0].tilt = 0; // all the way forward
        minute_play(&mut m, &clock, None, &mut r);
        assert_eq!(m.sides[0].rated, [110, 451, 330, 440, 418]);
    }

    #[test]
    fn the_bigger_crowd_leans_the_match_its_way() {
        // home fans ahead: below nought, at most three
        assert_eq!(crowd_lean(20000, 2000), -3);
        assert_eq!(crowd_lean(25477, 21018), -3);
        assert_eq!(crowd_lean(12000, 5000), -1);
        assert_eq!(crowd_lean(10000, 3000), -1);
        // a small crowd leans nowhere, an even one to the visitors
        assert_eq!(crowd_lean(2000, 1000), 0);
        assert_eq!(crowd_lean(0, 0), 0);
        assert_eq!(crowd_lean(10000, 10000), 1);
        assert_eq!(crowd_lean(1000, 20000), 3);
    }

    #[test]
    fn mood_and_form_work_the_attack_pair() {
        let mut m = fixture();
        m.sides[0].morale = 200;
        m.form = -20;
        let mut clock = Clock::default();
        let mut r = Random::new(1);
        minute_play(&mut m, &clock, None, &mut r);
        assert_eq!(m.sides[0].rated[3], 132); // (100 + 200/20) = 110, plus 20 % form
        assert_eq!(m.sides[0].rated[4], 120); // 100 plus the same 20 %
        assert_eq!(m.sides[1].rated[3], 100); // side two feels no negative form
        m.form = 20;
        minute_play(&mut m, &clock, None, &mut r);
        assert_eq!(m.sides[0].rated[3], 110); // and no positive form either
        assert_eq!(m.sides[1].rated[3], 120);
        // a mental of −3 cuts the attack pair back a further 3 %
        m.sides[1].mental = 4;
        minute_play(&mut m, &clock, None, &mut r);
        assert_eq!(m.sides[1].rated[3], 120 - 3);
        assert_eq!(m.sides[1].rated[4], 117); // its hold, form-lifted too
    }

    #[test]
    fn a_lost_half_sinks_the_mood() {
        let mut m = fixture();
        m.advantage = -50;
        let mut r = Random::new(1);
        minute_play(&mut m, &Clock { minute: 30, ..Clock::default() }, None, &mut r);
        assert_eq!(m.sides[0].morale, 0);
        minute_play(&mut m, &Clock { minute: 100, ..Clock::default() }, None, &mut r);
        assert_eq!(m.sides[0].morale, -15);
        m.flags = EXTRA;
        minute_play(&mut m, &Clock { minute: 100, ..Clock::default() }, None, &mut r);
        assert_eq!(m.sides[0].morale, -15 - 8); // (−50 · 15) / 90
        // and the other side when it lies behind
        let mut m = fixture();
        m.advantage = 50;
        minute_play(&mut m, &Clock { minute: 100, ..Clock::default() }, None, &mut r);
        assert_eq!(m.sides[1].morale, -15);
    }

    #[test]
    fn fatigue_accrues_until_the_half_runs_out() {
        let mut m = fixture();
        let mut r = Random::new(1);
        minute_play(&mut m, &Clock { minute: 45, ..Clock::default() }, None, &mut r);
        assert_eq!([m.sides[0].fatigue, m.sides[1].fatigue], [100, 100]);
        minute_play(&mut m, &Clock { minute: 46, ..Clock::default() }, None, &mut r);
        assert_eq!(m.sides[0].fatigue, 100);
        minute_play(
            &mut m,
            &Clock { minute: 90, half2: 1, ..Clock::default() },
            None,
            &mut r,
        );
        assert_eq!(m.sides[0].fatigue, 200);
        minute_play(&mut m, &Clock { minute: 91, half2: 1, ..Clock::default() }, None, &mut r);
        assert_eq!(m.sides[0].fatigue, 200);
        m.flags = EXTRA;
        minute_play(&mut m, &Clock { minute: 120, half2: 1, ..Clock::default() }, None, &mut r);
        assert_eq!(m.sides[0].fatigue, 300);
    }

    #[test]
    fn the_press_window_lifts_attack_and_support() {
        let mut m = fixture();
        m.sides[0].style = PRESS_BIT;
        m.sides[0].press_to = 30;
        m.sides[0].press_from = 10;
        let mut r = Random::new(1);
        minute_play(&mut m, &Clock { minute: 20, ..Clock::default() }, None, &mut r);
        assert_eq!(m.sides[0].rated[3], 120);
        assert_eq!(m.sides[0].rated[2], 120);
        assert_eq!(m.sides[0].pressed, 1);
        // past the window's end minute, no side presses
        let mut m2 = fixture();
        m2.sides[0].style = PRESS_BIT;
        m2.sides[0].press_to = 30;
        m2.sides[0].press_from = 10;
        minute_play(&mut m2, &Clock { minute: 40, ..Clock::default() }, None, &mut r);
        assert_eq!(m2.sides[0].rated[3], 100);
        assert_eq!(m2.sides[0].pressed, 0);
    }

    #[test]
    fn the_special_styles_trade_the_attacks() {
        let mut m = fixture();
        m.sides[0].style = 0x380;
        let mut r = Random::new(1);
        minute_play(&mut m, &Clock::default(), None, &mut r);
        assert_eq!(m.sides[0].rated[3], 80);
        assert_eq!(m.sides[1].rated[3], 85);
        let mut m = fixture();
        m.sides[1].style = 0x300;
        minute_play(&mut m, &Clock::default(), None, &mut r);
        assert_eq!(m.sides[0].rated[3], 120);
        assert_eq!(m.sides[1].rated[3], 115);
    }

    #[test]
    fn the_floors_hold() {
        let mut m = fixture();
        m.sides[0].strengths = [0, 1000, 0, 0, 0];
        m.sides[0].tilt = 0;
        let mut r = Random::new(1);
        minute_play(&mut m, &Clock::default(), None, &mut r);
        assert_eq!(m.sides[0].rated[3], 10);
        assert_eq!(m.sides[0].rated[4], 3);
    }

    #[test]
    fn the_chooser_rolls_its_step_and_both_sides() {
        let m = fixture();
        let mut took = false;
        for seed in 0u64..500 {
            let mut r = Random::new(seed);
            let step = r.below(6) as i32 * 5 + 7; // slot 5
            let first = r.below((56 * step) as u32); // tier 2, class 0
            let second = r.below((56 * step) as u32);
            let want = if first < 10 {
                1
            } else if second < 10 {
                2
            } else {
                0
            };
            assert_eq!(chance_chooser(&m, &mut Random::new(seed)), want, "seed {seed}");
            took |= want != 0;
        }
        assert!(took, "no seed under 500 let a side take the moment");
    }

    #[test]
    fn a_tier_past_four_rolls_nothing_for_its_side() {
        let mut m = fixture();
        m.sides[0].tier = 5;
        let seed = 3;
        let mut r = Random::new(seed);
        let step = r.below(6) as i32 * 5 + 7;
        let second = r.below((56 * step) as u32); // only side two still rolls
        let want = if second < 10 { 2 } else { 0 };
        assert_eq!(chance_chooser(&m, &mut Random::new(seed)), want);
    }

    #[test]
    fn the_frame_asks_the_chooser_then_both_sides() {
        let mut m = fixture(); // rated still zero: every share misses
        let mut expect = Random::new(771);
        let step = expect.below(6) as i32 * 5 + 7;
        expect.below((56 * step) as u32);
        expect.below((56 * step) as u32);
        expect.below(1000);
        expect.below(1000);
        let mut r = Random::new(771);
        chance_minute(&mut m, &Clock::default(), None, &mut r);
        assert_eq!(r, expect, "the chooser's three draws, then one roll each");
        assert_eq!(m.goals, [0, 0]);
        assert_eq!(m.sides[0].chances, 0);
    }

    #[test]
    fn a_chance_counts_without_scoring_until_the_events_land() {
        let mut m = fixture();
        m.sides[0].rated[3] = 92;
        m.sides[1].rated[3] = 92; // the four sum to 184, scaled onto 92
        let mut r = Random::new(7);
        for _ in 0..60 {
            chance_minute(&mut m, &Clock { minute: 30, ..Clock::default() }, None, &mut r);
        }
        assert_eq!(m.goals, [0, 0]); // FUN_00659d2b and siblings turn counts into goals
        assert!(0 < m.sides[0].chances + m.sides[1].chances);
    }

    #[test]
    fn the_ground_lifts_its_owner_an_eighth() {
        // Side one alone at a share of 92: at its own ground that reaches
        // 99.36, so rolls of 92..99 stay a miss without the ground and
        // open a chance — extra draws — with it.
        let mut found = None;
        for seed in 0u64..20_000 {
            let mut r = Random::new(seed);
            let step = r.below(6) as i32 * 5 + 7;
            r.below((56 * step) as u32);
            r.below((56 * step) as u32);
            let roll = r.below(1000);
            r.below(1000); // side two, whose zero share always misses
            if 92 <= roll && roll < 100 {
                found = Some((seed, r));
                break;
            }
        }
        let (seed, expect) = found.expect("no seed under 20000 rolls the home gap");
        let base = Match {
            ids: [3, 5, 8, 9],
            home_nation: 255,
            home_id: 999,
            ..fixture()
        };
        let mut plain = base;
        plain.sides[0].rated[3] = 92;
        let mut host = base;
        host.ids = [7, 200, 8, 9];
        host.home_nation = 7;
        host.sides[0].rated[3] = 92;
        let mut rp = Random::new(seed);
        chance_minute(&mut plain, &Clock::default(), None, &mut rp);
        assert_eq!(rp, expect, "without the ground the frame is five draws");
        let mut rh = Random::new(seed);
        chance_minute(&mut host, &Clock::default(), None, &mut rh);
        assert_ne!(rh, expect, "the host's roll opened a chance");
    }

    #[test]
    fn a_strong_shot_beats_a_weak_hold() {
        let mut m = fixture();
        m.sides[0].rated[4] = 100;
        m.sides[0].shot_pwr = 100;
        m.sides[1].rated[1] = 1; // below(1) holds nothing
        for seed in 0..50 {
            let mut r = Random::new(seed);
            let struck = r.below(85); // odds 85 at level attitudes
            if 0 < struck {
                assert_eq!(shot_quality(&m, 0, &mut Random::new(seed)), 1, "seed {seed}");
                return;
            }
        }
    }

    #[test]
    fn a_blown_shot_falls_to_the_margins() {
        let mut m = fixture();
        m.sides[0].rated[4] = 0; // nothing behind the shot
        m.sides[1].rated[1] = 100; // a full hold, and a far-off attacker
        for seed in 0..30 {
            let mut r = Random::new(seed);
            r.below(100); // the hold never sits under zero
            let a = r.below(100);
            let want = if a < 36 {
                2
            } else {
                let b = r.below(100);
                if b < 38 {
                    3
                } else {
                    4
                }
            };
            assert_eq!(shot_quality(&m, 0, &mut Random::new(seed)), want, "seed {seed}");
        }
    }

    #[test]
    fn the_talk_sleeps_through_a_narrow_early_lead() {
        let mut m = fixture();
        m.goals = [2, 1];
        let mut r = Random::new(11);
        morale_talk(&mut m, &Clock { minute: 45, ..Clock::default() }, &mut r);
        assert_eq!(r, Random::new(11), "one goal up before the 50th minute asks nothing");
    }

    #[test]
    fn the_talk_asks_once_and_the_staff_seam_draws_nothing_yet() {
        let mut m = fixture();
        m.goals = [2, 0]; // two goals apart opens the gate at any minute
        m.sides[0].talk_max = 2;
        m.sides[0].assistant = 1;
        let mut expect = Random::new(11);
        expect.below(3);
        let mut r = Random::new(11);
        morale_talk(&mut m, &Clock::default(), &mut r);
        assert_eq!(r, expect, "one roll in three, then FUN_004134d0 draws nothing");
        assert_eq!(m.sides[0].talks, 0);
    }

    #[test]
    fn the_talk_follows_the_rated_sums() {
        let mut m = fixture();
        m.sides[0].rated[0] = 50; // the first side ahead on rated sums
        assert_eq!(trailing(&m), 1);
        m.sides[1].rated[1] = 51; // now the sums carry the first side back
        assert_eq!(trailing(&m), 0);
    }

    #[test]
    fn the_pre_match_walk_lays_the_plain_fixture() {
        let mut row = [Match::default(), Match::default()];
        pre_match(&mut row);
        for (index, m) in row.iter().enumerate() {
            assert_eq!(m.index, index as u16); // +0x468, the place in the row
            assert_eq!(m.nat_bits, 0); // +0x474: no national side stands in
            assert_eq!([m.sides[0].assistant, m.sides[1].assistant], [1, 1]);
            assert_eq!(m.chance_slot, 5);
            assert_eq!(m.added_draw, 5); // selector one for `added_time`
            assert_eq!(m.referee, -1); // 0xffff: no referee was drawn
        }
    }

    #[test]
    fn the_pre_match_walk_draws_nothing() {
        // the league day's stream of draws starts at its first minute
        let mut row = [Match::default(), Match::default()];
        let mut r = Random::new(9);
        pre_match(&mut row);
        assert_eq!(r, Random::new(9));
    }

    /// The walk's sixth loop throws once for each man both sides list,
    /// eleven and bench, and then the first half's added time: a plain
    /// fixture's half ends between the 43rd and the 45th minute. Its
    /// fifth gives a computer club the co-trainer's tactics, three throws,
    /// and lays them over the side with its three talks.
    #[test]
    fn the_walk_enters_the_men_and_ends_the_first_half() {
        use a3_files::an3::Save;
        let game = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let data = crate::data::Data::load(&game).unwrap();
        let mut records = Records::from_save(Save::read(&game.join("saves/A.an3")).unwrap()).unwrap();
        let mut m = Match::default();
        m.ids = [14, 2, 14, 3];
        (m.home_nation, m.home_id) = (14, 2);
        let mut row = [m];
        pre_match(&mut row);
        line_up_row(&row, &mut records, &data);
        let listed: usize = [2, 3]
            .iter()
            .map(|&n| {
                let t = records.team(14, n).unwrap();
                t.lineup().into_iter().chain(t.bench()).filter(|&id| id != 0 && records.player(id).is_some()).count()
            })
            .sum();
        assert!(listed > 22, "both sides list a bench");
        let mut random = Random::new(3);
        kick_off_row(&mut row, &mut records, &data, &mut random);
        for (side, n) in row[0].sides.iter().zip([2, 3]) {
            let t = records.team(14, n).unwrap();
            assert_eq!((side.style, side.stance, side.tier, side.talk_max), (0, t.effort(), t.hardness(), 3));
            assert_eq!(side.tilt, t.style());
        }
        let before = random.clone();
        ready_row(&mut row, &mut records, &mut random);
        let mut step = before;
        for _ in 0..=listed {
            step.below(2);
        }
        assert_eq!(step, random, "a throw a listed man and the added time");
        assert!((43..=45).contains(&row[0].ends_at), "{}", row[0].ends_at);
        assert_eq!(row[0].sides[0].entries[0], records.team(14, 2).unwrap().lineup()[0]);
    }

    #[test]
    fn a_quiet_day_leaves_the_national_tie_to_its_sums() {
        // a tie carrying a national side is bulk-simulated at pre-match;
        // an unwatched day's minute walk passes it by
        let mut tie = fixture();
        tie.nat_bits = 3;
        let mut plain = fixture();
        let mut ms = [tie, plain];
        let mut r = Random::new(3);
        play_matches(&mut ms, false, None, None, test_day(), &mut r);
        assert_eq!(ms[0].sides[0].fatigue, 0);
        assert!(ms[1].sides[0].fatigue > 0);
    }

    #[test]
    fn a_watched_day_runs_the_national_tie_by_the_minute() {
        let mut tie = fixture();
        tie.nat_bits = 1;
        let mut r = Random::new(3);
        play_matches(std::slice::from_mut(&mut tie), true, None, None, test_day(), &mut r);
        assert!(tie.sides[0].fatigue > 0);
    }

    /// A man of the eleven as the record keeps him, with only what the
    /// taker's rating reads: strength (1 to 12), main position, the good
    /// and bad words, and his mood — the rest naught, which stands for
    /// fit and fresh, form then half his mood.
    fn squad_player(id: u16, strength: u8, position: u8, good: u16, bad: u16, mood: i8) -> Player {
        let mut p = Player::new(&vec![0; Player::SIZE]).unwrap();
        p.set_id(id);
        p.set_strength(strength);
        p.set_positions([position, 0, 0]);
        p.set_good(good);
        p.set_bad(bad);
        p.set_mood(mood);
        p
    }

    /// The same man with his birthday set, so he stands `age` on `today`
    /// — the shootout's roll reads his years where the original reads a
    /// runtime byte.
    fn aged_player(
        id: u16,
        strength: u8,
        position: u8,
        age: i32,
        good: u16,
        bad: u16,
        today: NaiveDate,
    ) -> Player {
        use chrono::Datelike;
        let mut p = squad_player(id, strength, position, good, bad, 10);
        p.set_birth_year((today.year() - age) as u16);
        p.set_birth_month(today.month() as u8);
        p.set_birth_day(today.day() as u8);
        p
    }

    /// The eleven as the shootout sees them: the keeper's place first, the
    /// ten after him, all aged and the field men carrying `bad`.
    fn aged_eleven(
        base: u16,
        keeper_age: i32,
        field_age: i32,
        bad: u16,
        today: NaiveDate,
    ) -> Vec<Player> {
        let mut men = vec![aged_player(base, 10, world::GOALKEEPER, keeper_age, 0, 0, today)];
        men.extend((1..11u16).map(|i| aged_player(base + i, 5, 4, field_age, 0, bad, today)));
        men
    }

    #[test]
    fn the_big_chance_takes_its_man_from_the_eleven() {
        // strength ten times over plus the form today (mood 10 stands at
        // form 5): 75 over 55, the keeper's 105 halved to 52
        let strong = squad_player(1, 7, 4, 0, 0, 10);
        let small = squad_player(2, 5, 6, 0, 0, 10);
        let keeper = squad_player(3, 10, world::GOALKEEPER, 0, 0, 10);
        assert_eq!(ai_big_taker(&[&strong, &small, &keeper]), Some(1));
        // the good word's bit 0x10 lifts a lesser man over all of them
        let willing = squad_player(4, 4, 7, BIG_WORD, 0, 10);
        assert_eq!(ai_big_taker(&[&strong, &small, &keeper, &willing]), Some(4));
        // a marked weakness leaves the strong man his doubled strength
        let shaky = squad_player(5, 9, 8, 0, 0x20, 10);
        assert_eq!(ai_big_taker(&[&shaky, &small]), Some(2));
        // nobody above nought rated means no taker at all
        let spent = squad_player(6, 0, 4, 0, 0, -100);
        let keeper2 = squad_player(7, 0, world::GOALKEEPER, 0, 0, -100);
        assert_eq!(ai_big_taker(&[&spent, &keeper2]), None);
    }

    #[test]
    fn the_set_piece_takes_its_free_kick_man() {
        let keeper = squad_player(1, 10, world::GOALKEEPER, 0, 0, 10);
        let field = squad_player(2, 1, 4, 0, 0, 10);
        assert_eq!(ai_set_taker(&[&keeper, &field]), Some(2));
        // with nothing else standing, the keeper takes it after all
        assert_eq!(ai_set_taker(std::slice::from_ref(&&keeper)), Some(1));
        // free kicks lift the weaker man, and no weakness is marked down
        let kicker = squad_player(3, 5, 8, FREE_KICKS, 0, 10);
        let shaky = squad_player(4, 9, 6, 0, 0x20, 10);
        assert_eq!(ai_set_taker(&[&kicker, &shaky]), Some(3));
        assert_eq!(ai_set_taker(&[&shaky, &field]), Some(4));
    }

    #[test]
    fn the_big_chance_lies_to_the_second_side_more() {
        // the first side converts below 0x50, the second only below 0x46:
        // a seed whose first hundred-wide draw sits between the two
        let seed = (1u64..)
            .find(|&s| {
                let d = Random::new(s).below(100);
                0x46 <= d && d < 0x50
            })
            .unwrap();
        let mut m = fixture();
        let mut r = Random::new(seed);
        big_chance(&mut m, 0, Some(3), &mut r);
        assert_eq!([m.goals[0], m.sides[0].chances.into()], [1, 1]);
        let mut m = fixture();
        let mut r = Random::new(seed);
        big_chance(&mut m, 1, Some(3), &mut r);
        assert_eq!([m.goals[1], m.sides[1].chances.into()], [0, 1]);
        // a chance whose taker is missing comes to nothing — and draws
        // nothing, so the day's stream is untouched
        let mut m = fixture();
        let mut r = Random::new(seed);
        big_chance(&mut m, 0, None, &mut r);
        assert_eq!([m.goals[0], m.sides[0].chances.into()], [0, 0]);
        assert_eq!(r, Random::new(seed));
    }

    #[test]
    fn the_set_piece_scores_only_on_a_perfect_shot() {
        let mut m = fixture();
        set_piece(&mut m, 1, 1, Some(4));
        assert_eq!([m.goals[1], m.sides[1].chances.into()], [1, 1]);
        assert_eq!(m.sides[1].chance_book[0], 4, "the taker takes the chance in his byte too");
        let mut m = fixture();
        set_piece(&mut m, 0, 2, Some(4));
        assert_eq!([m.goals[0], m.sides[0].chances.into()], [0, 1]);
        assert_eq!(m.sides[0].chance_book[0], 4, "a wide set piece still books the chance");
        let mut m = fixture();
        set_piece(&mut m, 0, 1, None);
        assert_eq!([m.goals[0], m.sides[0].chances.into()], [0, 0]);
        assert_eq!(m.sides[0].chance_book, [0; LEDGER_SLOTS], "no taker, no chance");
    }

    #[test]
    fn a_goal_bends_the_mood_by_the_sides_apart() {
        let mut m = fixture();
        m.sides[0].nerve = 30;
        m.sides[1].nerve = 12;
        score(&mut m, 0);
        assert_eq!([m.goals[0], m.sides[0].morale], [1, 3]);
        score(&mut m, 1);
        assert_eq!([m.goals[1], m.sides[1].morale], [1, -3]);
    }

    #[test]
    fn the_pressure_of_the_minutes_bands_the_wear() {
        // fatigue is the minutes' percentages summed: an even hundred a
        // minute over ninety stands at 9000, two bands past the calm
        // ninety; easy walking stays at the smallest multiplier of eight
        // and pressing a hundred and ten a minute reaches the greatest 12
        assert_eq!(WEAR_MULT[wear_band(8100, false)], 8);
        assert_eq!(WEAR_MULT[wear_band(9000, false)], 10);
        assert_eq!(WEAR_MULT[wear_band(9900, false)], 12);
        // extra time divides its fuller fatigue by a hundred and twenty:
        // a hundred and five a minute stands three bands up at 11
        assert_eq!(WEAR_MULT[wear_band(12600, true)], 11);
    }

    #[test]
    fn the_minutes_lift_the_man_and_drain_his_freshness() {
        // the man stands the whole ninety: the fourth band, its row scaled
        // by ten through the multiplier ten lifts his condition a hundred
        // and sinks the freshness by 225, 630 and 315 — the first value
        // only down to nought, where `FUN_006fc330` holds it
        let mut p = squad_player(1, 7, 4, 0, 0, 10);
        p.set_freshness([5000, 0, 0]);
        wear_man(&mut p, 90, 10);
        assert_eq!(p.condition(), 100);
        assert_eq!(p.freshness(), [4775, -630, -315]);
        // a hundred and twenty minutes past a hundred take the last row
        let mut p = squad_player(1, 7, 4, 0, 0, 10);
        p.set_freshness([5000, 0, 0]);
        wear_man(&mut p, 120, 10);
        assert_eq!(p.condition(), 120);
        assert_eq!(p.freshness(), [4660, -810, -405]);
        // an hour stands in the band below; the division truncates toward
        // nought, as the twenty minutes' −25 · 11 / 10 shows at −27
        let mut p = squad_player(1, 7, 4, 0, 0, 10);
        wear_man(&mut p, 70, 8);
        assert_eq!(p.condition(), 56);
        assert_eq!(p.freshness(), [0, -360, -180]);
        let mut p = squad_player(1, 7, 4, 0, 0, 10);
        wear_man(&mut p, 20, 11);
        assert_eq!(p.condition(), 11);
        assert_eq!(p.freshness(), [0, -66, -27]);
    }

    #[test]
    fn a_friendly_wears_by_its_own_rows() {
        // as the original's dumps of Probe1's pre-season tournaments have
        // it: ninety minutes at the normal effort's hundred a minute, the
        // multiplier ten, lift a man fifty and drain 125, 315 and 153
        let mut p = squad_player(1, 7, 4, 0, 0, 10);
        p.set_condition(5850);
        p.set_freshness([5807, -985, -468]);
        wear_by_rows(&mut p, 90, WEAR_MULT[wear_band(90 * 100, false)], &FRIENDLY_WEAR_ROWS);
        assert_eq!(p.condition(), 5900);
        assert_eq!(p.freshness(), [5682, -1300, -621]);
        // extra time at the harder effort's 105 stands in the band of
        // eleven; the keeper's halved five takes 30, 80, 335 and 157
        assert_eq!(WEAR_MULT[wear_band(120 * 105, true)], 11);
        let mut p = squad_player(1, 7, 4, 0, 0, 10);
        p.set_freshness([6335, 0, 0]);
        wear_by_rows(&mut p, 120, 10 / 2, &FRIENDLY_WEAR_ROWS);
        assert_eq!(p.condition(), 30);
        assert_eq!(p.freshness(), [6255, -335, -157]);
    }

    #[test]
    fn a_competition_match_wears_who_stood_and_steps_down_who_sat() {
        // as the original's dumps of Probe1's supercup final have it (the
        // visitors 14/85 the day after): ninety minutes at the harder
        // effort's 105 a minute stand in the band of eleven, lifting a man
        // 110 and draining 247, 693 and 346 by the competition rows
        assert_eq!(WEAR_MULT[wear_band(90 * 105, false)], 11);
        let mut p = squad_player(3289, 7, 4, 0, 0, 10);
        p.set_condition(5747);
        p.set_freshness([5762, 0, 0]);
        wear_by_rows(&mut p, 90, 11, &WEAR_ROWS);
        assert_eq!(p.condition(), 5857);
        assert_eq!(p.freshness(), [5515, -693, -346]);
        // the keeper's halved five: 50, 112, 315 and 157
        let mut p = squad_player(3285, 7, 1, 0, 0, 10);
        p.set_condition(6897);
        p.set_freshness([6676, 0, 0]);
        wear_by_rows(&mut p, 90, 11 / 2, &WEAR_ROWS);
        assert_eq!(p.condition(), 6947);
        assert_eq!(p.freshness(), [6564, -315, -157]);
        // a man who sat it out loses a step, his freshness untouched
        let mut p = squad_player(3286, 7, 4, 0, 0, 10);
        p.set_condition(5652);
        sat_out(&mut p);
        assert_eq!(p.condition(), 5552);
        assert_eq!(p.freshness(), [0, 0, 0]);
        // none below a step, and a smoker's five fewer: none below six
        p.set_condition(99);
        sat_out(&mut p);
        assert_eq!(p.condition(), 99);
        p.set_shown(SMOKER);
        p.set_condition(599);
        sat_out(&mut p);
        assert_eq!(p.condition(), 599);
        p.set_condition(600);
        sat_out(&mut p);
        assert_eq!(p.condition(), 500);
    }

    #[test]
    fn the_wear_holds_its_three_numbers_within_band() {
        // `FUN_00654ff0` keeps the condition at 0..10000 and `FUN_006fc330`
        // the freshness at 0..10000 on the first value, −10000..10000 on
        // the other two: an overplayed man sinks no lower and a fresh one
        // lifts no higher, and the condition is lifted even from nought
        let mut p = squad_player(1, 7, 4, 0, 0, 10);
        p.set_condition(9950);
        p.set_freshness([50, -9900, 9900]);
        wear_man(&mut p, 120, 12); // +144, −408, −972, −486
        assert_eq!(p.condition(), 10_000);
        assert_eq!(p.freshness(), [0, -10_000, 9414]);
        let mut p = squad_player(1, 7, 4, 0, 0, 10);
        p.set_condition(3000);
        wear_man(&mut p, 90, 8);
        assert_eq!(p.condition(), 3080);
    }

    #[test]
    fn a_grade_follows_the_result_and_the_form_lost() {
        // the draw below four, replayed from the same seed
        let draw = Random::new(7).below(4) as u8;
        // a level game at unchanged form: six and the draw
        assert_eq!(match_grade(9, 9, 1, 1, &mut Random::new(7)), 6 + draw);
        // a win by more than two, two half steps better
        assert_eq!(match_grade(9, 9, 4, 0, &mut Random::new(7)), 4 + draw);
        // a loss by one with two form points lost: one and four worse
        assert_eq!(match_grade(9, 7, 0, 1, &mut Random::new(7)), (11 + draw).min(12));
        // the top form of twenty and a big win cannot pass a one
        assert_eq!(match_grade(0, 20, 9, 0, &mut Random::new(7)), 2);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_match_grades_its_eleven_on_the_save() {
        use a3_files::an3::Save;
        let game = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let mut records = Records::from_save(Save::read(&game.join("saves/A.an3")).unwrap()).unwrap();
        let mut m = Match::default();
        m.ids = [0, 0, 0, 500];
        m.goals = [2, 0];
        let eleven = saved_eleven(&m, 0, &records);
        let kickoff: Vec<(u16, i32)> =
            eleven.iter().map(|&id| (id, statistics::form_today(records.player(id).unwrap()))).collect();
        let (mean, best) = post_match_grades(&m, 0, &kickoff, &mut records, &mut Random::new(3));
        let grades: Vec<u8> = eleven.iter().map(|&id| records.player(id).unwrap().grade()).collect();
        assert!(grades.iter().all(|g| (2..=12).contains(g)), "{grades:?}");
        let low = *grades.iter().min().unwrap();
        assert!(!best.is_empty() && best.iter().all(|id| records.player(*id).unwrap().grade() == low));
        let sum: i32 = grades.iter().map(|&g| i32::from(g)).sum();
        assert_eq!(i32::from(mean), sum * 10 / 11);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_match_wears_its_eleven_on_the_save() {
        use a3_files::an3::Save;
        let game = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let mut records = Records::from_save(Save::read(&game.join("saves/A.an3")).unwrap()).unwrap();
        // the first German club's saved eleven, on a side that walked its
        // minutes at an even hundred a minute — the multiplier ten, the
        // ninety's row; the save's manager stands his full formation
        let ids: Vec<u16> = records.team(0, 0).unwrap().lineup().into_iter().collect();
        assert_eq!(ids.iter().filter(|&&id| id != 0).count(), 11);
        let before: Vec<(i16, [i16; 3])> = ids
            .iter()
            .map(|id| {
                let p = records.player(*id).unwrap();
                (p.condition(), p.freshness())
            })
            .collect();
        let mut m = Match::default();
        m.ids = [0, 0, 0, 500]; // the second side stands in no club here
        m.sides[0].fatigue = 9000;
        post_match_wear(&m, &mut records);
        for (id, (c, f)) in ids.iter().zip(&before) {
            let p = records.player(*id).unwrap();
            assert_eq!(p.condition(), (i32::from(*c) + 100).clamp(0, 10_000) as i16);
            assert_eq!(
                p.freshness().map(i32::from),
                [
                    (i32::from(f[0]) - 225).clamp(0, 10_000),
                    (i32::from(f[1]) - 630).clamp(-10_000, 10_000),
                    (i32::from(f[2]) - 315).clamp(-10_000, 10_000),
                ]
            );
        }
        // and the men really moved — one at full condition may stand, but
        // nobody on a fresh save stays put
        assert!(ids
            .iter()
            .zip(&before)
            .any(|(id, (c, _))| records.player(*id).unwrap().condition() != *c));
    }

    #[test]
    fn the_keeper_takes_no_shot_at_goal_at_all() {
        let keeper = squad_player(1, 10, world::GOALKEEPER, 0, 0, 10);
        let mut r = Random::new(5);
        assert_eq!(scorer_pick(&[&keeper], &mut r), (0, 0));
        assert_eq!(r, Random::new(5), "an unweighed list draws nothing");
    }

    #[test]
    fn the_spearhead_outweighs_the_back() {
        // the back at band one weighs 45, the spearhead at band six 153
        // (strength ten times over plus form 5, mood 10 again): one seed
        // below the first cumulative, one above it
        let back = squad_player(1, 40, 4, 0, 0, 10);
        let head = squad_player(2, 20, world::STRIKER, 0, 0, 10);
        let men = [&back as &Player, &head];
        let low = (1u64..).find(|&s| Random::new(s).below(198) < 45).unwrap();
        let high = (1u64..).find(|&s| Random::new(s).below(198) >= 45).unwrap();
        assert_eq!(scorer_pick(&men, &mut Random::new(low)).0, 1);
        assert_eq!(scorer_pick(&men, &mut Random::new(high)).0, 2);
    }

    #[test]
    fn the_assist_list_loses_the_scorer() {
        // 55 and 75 weigh the two half backs; a scorer from the head of
        // the list leaves the other alone under 75 — rebased by his weight
        let first = squad_player(1, 50, 2, 0, 0, 10);
        let second = squad_player(2, 70, 2, 0, 0, 10);
        let seed = (1u64..)
            .find(|&s| Random::new(s).below(130) < 55)
            .unwrap();
        let mut expect = Random::new(seed);
        expect.below(130);
        expect.below(75);
        let mut r = Random::new(seed);
        assert_eq!(scorer_pick(&[&first, &second], &mut r), (1, 2));
        assert_eq!(r, expect, "the scorer draw and the assist draw");
        // a lone listed man gets no assist and costs the one draw
        let mut r = Random::new(seed);
        expect = Random::new(seed);
        expect.below(55);
        assert_eq!(scorer_pick(&[&first], &mut r), (1, 0));
        assert_eq!(r, expect);
    }

    #[test]
    fn the_keeper_heads_nothing_into_his_own_net() {
        let keeper = squad_player(1, 10, world::GOALKEEPER, 0, 0, 10);
        let mut r = Random::new(9);
        assert_eq!(own_goal_pick(&[&keeper], &mut r), 0);
        assert_eq!(r, Random::new(9));
        // a back at five danger weighs 247, the spearhead at one 49 —
        // the form 5 comes off the thousand as well
        let back = squad_player(2, 50, 3, 0, 0, 10);
        let head = squad_player(3, 50, world::STRIKER, 0, 0, 10);
        let men = [&back as &Player, &head];
        let low = (1u64..).find(|&s| Random::new(s).below(296) < 247).unwrap();
        let high = (1u64..).find(|&s| Random::new(s).below(296) >= 247).unwrap();
        assert_eq!(own_goal_pick(&men, &mut Random::new(low)), 2);
        assert_eq!(own_goal_pick(&men, &mut Random::new(high)), 3);
    }

    #[test]
    fn the_shot_event_counts_the_chance_without_an_eleven() {
        let mut m = fixture();
        let mut r = Random::new(3);
        shot_event(&mut m, 0, None, &mut r);
        assert_eq!([m.goals[0], m.sides[0].chances.into()], [0, 1]);
        assert_eq!(r, Random::new(3), "the event draws nothing it cannot weigh");
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_quality_one_shot_is_a_goal_on_the_save() {
        use a3_files::an3::Save;
        let game = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let records = Records::from_save(Save::read(&game.join("saves/A.an3")).unwrap()).unwrap();
        let mut m = Match::default();
        m.ids = [0, 0, 0, 500]; // the second side stands in no club here
        let att = side_men(&m, 0, &records);
        let def = side_men(&m, 1, &records);
        assert_eq!(att.len(), 11);
        assert!(def.is_empty());
        // the plain throw: the pick's draws, the outcome's, and the goal —
        // a seed anywhere, the eleven all weighed
        let seed = (1u64..).find(|&s| Random::new(s).below(200) != 0).unwrap();
        let mut m = Match::default();
        m.ids = [0, 0, 0, 500];
        shot_event(&mut m, 0, Some((&att, &def)), &mut Random::new(seed));
        assert_eq!([m.goals[0], m.sides[0].chances.into()], [1, 1]);
        // and the rarest throw of all: an own goal none of the empty
        // defending side can be blamed for, so no goal stands
        let seed = (1u64..).find(|&s| Random::new(s).below(200) == 0).unwrap();
        let mut m = Match::default();
        m.ids = [0, 0, 0, 500];
        shot_event(&mut m, 0, Some((&att, &def)), &mut Random::new(seed));
        assert_eq!([m.goals[0], m.sides[0].chances.into()], [0, 1]);
    }

    #[test]
    fn the_big_chance_marks_its_taker_whichever_way_the_ball_goes() {
        // the record's entered lists arrive with the line-up; the slots
        // stand in for that fill here
        let mut m = fixture();
        m.sides[0].entries[3] = 7;
        // the book runs before the hundred-wide roll, so a miss still
        // leaves the taker's shot in the ledger
        let miss = (1u64..).find(|&s| Random::new(s).below(100) >= 0x50).unwrap();
        big_chance(&mut m, 0, Some(7), &mut Random::new(miss));
        assert_eq!([m.goals[0], m.sides[0].chances.into()], [0, 1]);
        assert_eq!(m.sides[0].ledger[3], LEDGER_SHOT);
        assert_eq!(m.sides[0].chance_book[0], 7, "the taker's chance byte stands booked alike");
        assert_eq!(m.sides[0].penalty_book[0], 7, "the wide chance lifts his taken byte");
        // a taker the record does not name books nothing, and the mark
        // reaches the second side's entered slot as well
        let mut m = fixture();
        big_chance(&mut m, 0, Some(7), &mut Random::new(miss));
        assert_eq!(m.sides[0].ledger, [0; LEDGER_SLOTS]);
        // the chance byte needs no entered slot: the man himself stands
        // booked even where the ledger finds no place for him
        assert_eq!(m.sides[0].chance_book[0], 7);
        m.sides[1].entries[17] = 7;
        big_chance(&mut m, 1, Some(7), &mut Random::new(miss));
        assert_eq!(m.sides[1].ledger[17], LEDGER_SHOT);
        assert_eq!(m.sides[1].chance_book[0], 7);
        assert_eq!(m.sides[1].penalty_book[0], 7);
        // the converted chance lifts no taken byte: only the miss
        // branch calls `FUN_00740f33`, the scored one stands in the
        // scorer's own books
        let hit = (1u64..).find(|&s| Random::new(s).below(100) < 0x50).unwrap();
        let mut m = fixture();
        big_chance(&mut m, 0, Some(7), &mut Random::new(hit));
        assert_eq!(m.goals[0], 1);
        assert_eq!(m.sides[0].penalty_book, [0; LEDGER_SLOTS]);
    }

    #[test]
    fn the_own_goal_marks_its_man_and_the_mark_is_read_back() {
        let back = squad_player(9, 5, 3, 0, 0, 10);
        let def = [&back as &Player];
        let throw = (1u64..).find(|&s| Random::new(s).below(200) == 0).unwrap();
        let mut m = fixture();
        m.sides[1].entries[5] = 9;
        shot_event(&mut m, 0, Some((&[], def.as_slice())), &mut Random::new(throw));
        assert_eq!(m.goals[0], 1);
        assert_eq!(m.sides[1].ledger[5], LEDGER_OWN);
        assert!(own_goal_mark(&m, 9));
        // the quality-one event marks no shot at all: the scorer's slot
        // stays clean; only the big chances and the plain event book them
        let struck = (1u64..).find(|&s| Random::new(s).below(200) != 0).unwrap();
        let mut m = fixture();
        m.sides[0].entries[0] = 9;
        shot_event(&mut m, 0, Some((def.as_slice(), &[])), &mut Random::new(struck));
        assert_eq!(m.goals[0], 1);
        assert_eq!(m.sides[0].ledger, [0; LEDGER_SLOTS]);
        // a man the record names nowhere reads back unmarked
        assert!(!own_goal_mark(&m, 42));
    }

    #[test]
    fn the_ledger_gathers_its_bits_in_the_naming_slot() {
        let mut m = fixture();
        book_event(&mut m, 11, LEDGER_SHOT);
        assert_eq!(m.sides[0].ledger, [0; LEDGER_SLOTS]);
        m.sides[1].entries[4] = 11;
        book_event(&mut m, 11, LEDGER_SHOT);
        assert_eq!(m.sides[1].ledger[4], LEDGER_SHOT);
        book_event(&mut m, 11, LEDGER_OWN);
        assert_eq!(m.sides[1].ledger[4], LEDGER_SHOT | LEDGER_OWN);
        assert!(own_goal_mark(&m, 11));
        assert!(!own_goal_mark(&m, 12));
    }

    #[test]
    fn a_thin_pitch_hands_the_tie_over_whole() {
        let today = NaiveDate::from_ymd_opt(2000, 6, 15).unwrap();
        let many = aged_eleven(1, 25, 25, 0, today);
        let few: Vec<Player> =
            (30..36).map(|id| aged_player(id, 5, 4, 25, 0, 0, today)).collect();
        let also: Vec<Player> =
            (40..46).map(|id| aged_player(id, 5, 4, 25, 0, 0, today)).collect();
        let many: Vec<&Player> = many.iter().collect();
        let few: Vec<&Player> = few.iter().collect();
        let also: Vec<&Player> = also.iter().collect();
        let mut m = fixture();
        assert_eq!([m.sides[0].men, m.sides[1].men], [11, 11]);
        let mut r = Random::new(11);
        penalty_shootout(&mut m, (&few, &many), today, &mut r);
        assert_eq!([m.goals[0], m.goals[1]], [1, 3]);
        // an equal pair of thin pitches gives the three to the first side,
        // and neither walk draws a number
        let mut m = fixture();
        penalty_shootout(&mut m, (&few, &also), today, &mut r);
        assert_eq!([m.goals[0], m.goals[1]], [3, 1]);
        assert_eq!(r, Random::new(11), "the away-hand walk draws nothing");
    }

    #[test]
    fn the_shootout_draws_its_takers_without_replacement() {
        let today = NaiveDate::from_ymd_opt(2000, 6, 15).unwrap();
        let men = aged_eleven(1, 25, 25, 0, today);
        let men: Vec<&Player> = men.iter().collect();
        let mut list = [0u16; SHOOTOUT_SLOTS];
        let mut r = Random::new(5);
        fill_takers(&men, &mut list, 0, SHOOTOUT_TAKERS, &mut r);
        assert!(list[..SHOOTOUT_TAKERS].iter().all(|id| *id != 0));
        for i in 0..SHOOTOUT_TAKERS {
            for j in i + 1..SHOOTOUT_TAKERS {
                assert_ne!(list[i], list[j], "no man steps up twice");
            }
        }
        let mut expect = Random::new(5);
        for n in [11u32, 10, 9, 8, 7] {
            expect.below(n);
        }
        assert_eq!(r, expect, "one draw below the pool's shrinking size");
        // the sudden death's one taker skips the five already listed
        fill_takers(&men, &mut list, SHOOTOUT_TAKERS, 1, &mut r);
        expect.below(6);
        assert_ne!(list[SHOOTOUT_TAKERS], 0);
        assert!(!list[..SHOOTOUT_TAKERS].contains(&list[SHOOTOUT_TAKERS]));
        assert_eq!(r, expect);
        // a pitch thinner than the order takes its first standing man for
        // every slot, and rolls nothing
        let few: Vec<Player> =
            (30..33).map(|id| aged_player(id, 5, 4, 25, 0, 0, today)).collect();
        let few: Vec<&Player> = few.iter().collect();
        let mut list = [0u16; SHOOTOUT_SLOTS];
        fill_takers(&few, &mut list, 0, SHOOTOUT_TAKERS, &mut r);
        assert_eq!(&list[..SHOOTOUT_TAKERS], &[30u16; SHOOTOUT_TAKERS]);
        assert_eq!(r, expect, "the fallback draws nothing");
    }

    #[test]
    fn the_shootout_kick_weighs_its_pressure_before_the_dice() {
        let today = NaiveDate::from_ymd_opt(2000, 6, 15).unwrap();
        let taker = aged_player(1, 5, 4, 25, 0, 0, today);
        let keeper = aged_player(2, 8, world::GOALKEEPER, 25, 0, 0, today);
        let mut r = Random::new(4);
        // the sudden death's weight settles the kick before anything else,
        // and draws nothing
        assert!(!penalty_kick(Some(&taker), Some(&keeper), -1, today, &mut r));
        assert!(penalty_kick(Some(&taker), Some(&keeper), 1, today, &mut r));
        // nobody listed misses, and an empty keeper's place buries it
        assert!(!penalty_kick(None, Some(&keeper), 0, today, &mut r));
        assert!(penalty_kick(Some(&taker), None, 0, today, &mut r));
        assert_eq!(r, Random::new(4), "none of these reaches the dice");
        // the dice themselves, each on its own stream
        let mut r = Random::new(4);
        assert!(penalty_kick(Some(&taker), Some(&keeper), 0, today, &mut r));
        let mut expect = Random::new(4);
        // at twenty-five the strike stands at a hundred and sixty under a
        // twenty-five-year-old keeper's fifty: below a hundred, always in
        expect.below(100);
        assert_eq!(r, expect);
        let mut r = Random::new(4);
        let shaky = aged_player(3, 5, 4, 17, 0, BIG_WORD, today);
        let old = aged_player(4, 8, world::GOALKEEPER, 64, 0, 0, today);
        assert!(!penalty_kick(Some(&shaky), Some(&old), 0, today, &mut r));
        // a seventeen-year-old marked down for the spot before a
        // sixty-four-year-old keeper falls ten below the reach: out under
        // any throw
        let mut expect = Random::new(4);
        expect.below(100);
        assert_eq!(r, expect);
        // the keeper's line: the same young man unafraid of the spot
        // strikes from a hundred and thirty-eight under the old keeper's
        // a hundred and twenty-eight — out under nought, but a keeper
        // who cannot hold his line lets fifteen through
        let brave = aged_player(5, 5, 4, 17, BIG_WORD, 0, today);
        let gift = aged_player(6, 8, world::GOALKEEPER, 64, 0, KEEPER_GIFT, today);
        let narrow = (1u64..).find(|&s| (10..15).contains(&Random::new(s).below(100))).unwrap();
        assert!(!penalty_kick(Some(&brave), Some(&old), 0, today, &mut Random::new(narrow)));
        assert!(penalty_kick(Some(&brave), Some(&gift), 0, today, &mut Random::new(narrow)));
        // and one who holds it shuts fifteen out again
        let stout = aged_player(7, 8, world::GOALKEEPER, 64, KEEPER_GIFT, 0, today);
        assert!(!penalty_kick(Some(&brave), Some(&stout), 0, today, &mut Random::new(narrow)));
    }

    #[test]
    fn a_shootout_of_certain_kicks_needs_three_sudden_rounds() {
        let today = NaiveDate::from_ymd_opt(2000, 6, 15).unwrap();
        // men of twenty-five before keepers of twenty-five strike from a
        // hundred and sixty under fifty: every dice kick lands
        let a = aged_eleven(1, 25, 25, 0, today);
        let b = aged_eleven(20, 25, 25, 0, today);
        let a: Vec<&Player> = a.iter().collect();
        let b: Vec<&Player> = b.iter().collect();
        let mut m = fixture();
        let mut r = Random::new(7);
        penalty_shootout(&mut m, (&a, &b), today, &mut r);
        // five apiece in the round, then the sudden death decides in its
        // third one: at seven and seven the past-twelve weight lifts the
        // first side's kick and weighs the second's down, so the first
        // side takes it whichever way the coin fell
        assert_eq!([m.goals[0], m.goals[1]], [8, 7]);
        // the stream: the coin, the five takers a side out of eleven, the
        // round's ten rolls, and only then every sudden round's one taker
        // at the pool's shrinking size before its two rolls — the third
        // round rolls no dice
        let mut expect = Random::new(7);
        expect.below(2);
        for n in [11u32, 10, 9, 8, 7] {
            expect.below(n);
        }
        for n in [11, 10, 9, 8, 7] {
            expect.below(n);
        }
        for _ in 0..10 {
            expect.below(100);
        }
        expect.below(6);
        expect.below(6);
        expect.below(100);
        expect.below(100);
        expect.below(5);
        expect.below(5);
        expect.below(100);
        expect.below(100);
        expect.below(4);
        expect.below(4);
        assert_eq!(r, expect, "the shootout's whole draw stream");
    }

    #[test]
    fn the_shootout_gives_up_while_the_trailing_side_can_no_longer_reach() {
        let today = NaiveDate::from_ymd_opt(2000, 6, 15).unwrap();
        // the first side's men of twenty-five reach a hundred and twenty-six
        // over the second side's young keeper, and their keeper's own sixty-
        // four years reach further still; the second side's men of seventeen
        // marked down for the spot strike from fifty past sixty-eight under
        // the first side's old keeper's a hundred and twenty-eight — out,
        // and their own young keeper no better off; every kick is settled
        let a = aged_eleven(1, 64, 25, 0, today);
        let b = aged_eleven(20, 17, 17, BIG_WORD, today);
        let a: Vec<&Player> = a.iter().collect();
        let b: Vec<&Player> = b.iter().collect();
        // whichever side steps up first, the walk gives up on the sixth
        // kick: the first side has three, the second none, and two kicks
        // left reach no more than two — the check after the sixth spare the
        // rest, whoever took it
        let seeds = [
            (1u64..).find(|&s| Random::new(s).below(2) == 0).unwrap(),
            (1u64..).find(|&s| Random::new(s).below(2) != 0).unwrap(),
        ];
        for seed in seeds {
            let mut m = fixture();
            let mut r = Random::new(seed);
            penalty_shootout(&mut m, (&a, &b), today, &mut r);
            assert_eq!([m.goals[0], m.goals[1]], [3, 0], "seed {seed}");
            let mut expect = Random::new(seed);
            expect.below(2);
            for n in [11u32, 10, 9, 8, 7] {
                expect.below(n);
            }
            for n in [11, 10, 9, 8, 7] {
                expect.below(n);
            }
            for _ in 0..6 {
                expect.below(100);
            }
            assert_eq!(r, expect, "six kicks and their rolls, seed {seed}");
        }
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_saved_eleven_takes_its_shootout_to_a_decision() {
        use a3_files::an3::Save;
        let game = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let records = Records::from_save(Save::read(&game.join("saves/A.an3")).unwrap()).unwrap();
        // the first German club's eleven standing on both pitches: real
        // ages, so the kicks are dice from the first — and the walk
        // reaches a decision, the past-twelve weight ending any run of
        // equal kicks
        let mut m = fixture();
        m.ids = [0, 0, 0, 0];
        let att = side_men(&m, 0, &records);
        let def = side_men(&m, 1, &records);
        assert_eq!([att.len(), def.len()], [11, 11]);
        let today = NaiveDate::from_ymd_opt(2000, 6, 15).unwrap();
        penalty_shootout(&mut m, (&att, &def), today, &mut Random::new(7));
        assert!(m.goals[0] != m.goals[1], "{:?}", m.goals);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_walks_penalty_minute_settles_the_saved_tie() {
        use a3_files::an3::Save;
        let game = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let mut records = Records::from_save(Save::read(&game.join("saves/A.an3")).unwrap()).unwrap();
        // the first German club's eleven on both pitches, a level tie
        // after extra time: the walk's penalty minute now calls the
        // shootout itself, and the saved men's ages make the kicks dice
        // from the first — the tie comes out decided
        let mut m = fixture();
        m.ids = [0, 0, 0, 0];
        m.goals = [2, 2];
        m.flags = PENS;
        let mut clock = Clock { extra: 2, minute: 0x78, ..Clock::default() };
        let mut r = Random::new(7);
        let run = tick(
            std::slice::from_mut(&mut m),
            false,
            0x78,
            &mut clock,
            Some(&mut records),
            test_day(),
            &mut r,
        );
        assert!(!run, "the penalty minute is the walk's last word");
        assert_ne!(m.goals[0], m.goals[1], "{:?}", m.goals);
        assert_ne!(r, Random::new(7), "the minute rolled the kicks' dice");
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_row_lines_up_its_computer_sides_before_the_minutes() {
        use a3_files::an3::Save;
        let game = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let mut records = Records::from_save(Save::read(&game.join("saves/A.an3")).unwrap()).unwrap();
        let data = crate::data::Data::load(&game).unwrap();
        // a German computer club whose listed second man has gone since
        // the week's line-up, against a foreign club whose record lists
        // nobody — the week's line-up reaches the played nations only
        let foreign = (0..8u16)
            .find(|&t| records.team(13, t).is_some_and(|x| x.lineup() == [0; 11] && x.players().len() >= 11))
            .expect("a foreign club listing nobody");
        let mut m = fixture();
        m.ids = [14, 16, 13, foreign];
        assert_eq!(records.team(14, 16).unwrap().humans()[0], 0);
        assert_eq!(saved_eleven(&m, 0, &records)[1], 0, "the hole stands in the record");
        let mut human = fixture();
        human.ids = [0, 0, 0, 0];
        let saved = records.team(0, 0).unwrap().lineup();
        line_up_row(&[m, human], &mut records, &data);
        // both stand the co-trainer's full eleven out of their squads,
        // the walk enters it, and the human's saved eleven stays his
        for which in 0..2 {
            let team = records.team(m.ids[which * 2], m.ids[which * 2 + 1]).unwrap();
            let eleven = saved_eleven(&m, which, &records);
            assert_eq!(eleven, team.lineup().to_vec());
            assert!(eleven.iter().all(|&id| id != 0 && team.players().contains(&id)), "{eleven:?}");
        }
        assert_eq!(records.team(0, 0).unwrap().lineup(), saved);
        let lineup = records.team(13, foreign).unwrap().lineup();
        play_matches(std::slice::from_mut(&mut m), false, Some(&mut records), None, test_day(), &mut Random::new(5));
        assert_eq!(m.sides[1].entries[..11], lineup[..]);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_walk_enters_its_men_before_the_minutes() {
        use a3_files::an3::Save;
        let game = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let mut records = Records::from_save(Save::read(&game.join("saves/A.an3")).unwrap()).unwrap();
        // the first German club's manager on both pitches: the walk's
        // first act with the records enters his saved eighteen — the
        // eleven at the record's +0x148 and the bench at +0x15e — so
        // from the first minute the books find their men
        let mut m = fixture();
        m.ids = [0, 0, 0, 0];
        play_matches(std::slice::from_mut(&mut m), false, Some(&mut records), None, test_day(), &mut Random::new(5));
        for which in 0..2 {
            let team = records
                .team(m.ids[which * 2], m.ids[which * 2 + 1])
                .expect("the save's first club");
            let saved: Vec<u16> = team.lineup().into_iter().chain(team.bench()).collect();
            assert_eq!(
                &m.sides[which].entries[..],
                saved.as_slice(),
                "side {which} stands its manager's eighteen entered"
            );
            let keeper = records.player(m.sides[which].entries[0]);
            assert!(
                keeper.is_some_and(|p| p.positions()[0] == world::GOALKEEPER),
                "the line-up's first short names the keeper"
            );
            let pitch: Vec<u16> = side_men(&m, which, &records).iter().map(|p| p.id()).collect();
            // a man sent off leaves his place empty
            let standing: Vec<u16> =
                team.lineup().into_iter().filter(|id| !m.sides[which].sent_off.contains(id)).collect();
            assert_eq!(pitch, standing, "side {which} plays its manager's eleven");
        }
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_entered_man_takes_his_ledgers_mark() {
        use a3_files::an3::Save;
        let game = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let records = Records::from_save(Save::read(&game.join("saves/A.an3")).unwrap()).unwrap();
        // with the eleven entered, the big chance's marking call before
        // the roll (`FUN_00740996`) finds the taker's own slot at
        // +0x21c: the best-rated man of the saved club stands there, and
        // whichever way the hundred-wide roll falls his shot is booked
        let mut m = fixture();
        m.ids = [0, 0, 0, 0];
        fill_entries(&mut m, 0, &records);
        let men = side_men(&m, 0, &records);
        let taker = ai_big_taker(&men).expect("a man rated above nought");
        let slot = men.iter().position(|p| p.id() == taker).unwrap();
        big_chance(&mut m, 0, Some(taker), &mut Random::new(3));
        assert_eq!(m.sides[0].ledger[slot], LEDGER_SHOT);
        assert!(!own_goal_mark(&m, taker), "the mark is the shot's bit alone");
        assert_eq!(m.sides[1].ledger, [0; LEDGER_SLOTS], "side one stands unentered");
    }

    #[test]
    fn the_penalty_minute_draws_nothing_without_the_save() {
        // the same minute over a row the records cannot name: as the
        // pickers' events come to nothing, so do the kicks — no number
        // drawn, the tie left level for the day's own settling
        let mut m = fixture();
        m.goals = [2, 2];
        m.flags = PENS;
        let mut clock = Clock { extra: 2, minute: 0x78, ..Clock::default() };
        let mut r = Random::new(4);
        let run =
            tick(std::slice::from_mut(&mut m), false, 0x78, &mut clock, None, test_day(), &mut r);
        assert!(!run);
        assert_eq!(m.goals, [2, 2]);
        assert_eq!(r, Random::new(4), "the empty minute draws no coin");
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_national_side_plays_the_associations_saved_tactic() {
        use a3_files::an3::Save;
        let game = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let records = Records::from_save(Save::read(&game.join("saves/A.an3")).unwrap()).unwrap();
        let saved: Vec<u8> =
            (0..4).map(|i| records.block(&format!("0x1bdd6b8[{i}]")).unwrap()[0]).collect();
        eprintln!("national tactics {saved:?}");
        // the side at list place p takes the first byte of the saved entry
        // p-1 into its tactic (+0x30); places 0 and 5..7 keep the tactic
        for place in 1..=4i16 {
            let mut side = fixture().sides[0];
            side.tilt = 0x7f;
            national_tactic(&mut side, place, Some(&records));
            assert_eq!(side.tilt, saved[place as usize - 1]);
        }
        for place in [0i16, 5, 7] {
            let mut side = fixture().sides[0];
            national_tactic(&mut side, place, Some(&records));
            assert_eq!(side.tilt, 3);
        }
        // the minute reads the second side's place from the marker's top
        // nibble, the first side's from the byte above the bits
        let mut m = fixture();
        m.sides[0].tilt = 0x7f;
        m.sides[1].tilt = 0x7f;
        m.nat_bits = 2 | (3 << 8) | (2 << 12);
        national_tactics(&mut m, Some(&records));
        assert_eq!([m.sides[0].tilt, m.sides[1].tilt], [0x7f, saved[1]]);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_walked_match_wears_its_men_on_the_save() {
        use a3_files::an3::Save;
        let game = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let mut records = Records::from_save(Save::read(&game.join("saves/A.an3")).unwrap()).unwrap();
        // the first German club's eleven stands on both pitches of a
        // plain fixture the walk runs to full time, its sides weary past
        // the top band: the day loop's plain wear pass lands on every
        // played man per pitch — ninety minutes of the fourth row at the
        // twelvefold, arithmetic on the save with no number drawn — so
        // here each man takes it twice; the national tie in the same
        // row, which the walk leaves to the bulk pre-match simulation,
        // adds nothing, and the club's reserve stands unworn
        let team = records.team(0, 0).unwrap();
        let men: Vec<u16> = team.lineup().into_iter().collect();
        let bench: Vec<u16> = team.bench()[..3].to_vec();
        let before: Vec<(i16, [i16; 3])> = men
            .iter()
            .chain(&bench)
            .map(|id| {
                let p = records.player(*id).unwrap();
                (p.condition(), p.freshness())
            })
            .collect();
        let mut m = fixture();
        m.ids = [0, 0, 0, 0];
        m.sides[0].fatigue = 12600;
        m.sides[1].fatigue = 12600;
        let mut tie = fixture();
        tie.ids = [0, 0, 0, 0];
        tie.nat_bits = 1;
        let mut r = Random::new(6);
        play_matches(&mut [m, tie], false, Some(&mut records), None, test_day(), &mut r);
        // top wear band through the side's fatigue, ninety minutes on
        // the men: the fourth row scaled twelve tenths, twice over for
        // the eleven that stands on both of this fixture's pitches
        let moved = |d: i32| 2 * (d * 12 / 10);
        for (id, (cond, fresh)) in men.iter().zip(&before) {
            let p = records.player(*id).unwrap();
            assert_eq!(
                (p.condition(), p.freshness()),
                (
                    (i32::from(*cond) + moved(100)).clamp(0, 10_000) as i16,
                    [
                        (i32::from(fresh[0]) + moved(-225)).clamp(0, 10_000) as i16,
                        (i32::from(fresh[1]) + moved(-630)).clamp(-10_000, 10_000) as i16,
                        (i32::from(fresh[2]) + moved(-315)).clamp(-10_000, 10_000) as i16,
                    ],
                ),
                "the eleven of club one stands worn by its two pitches"
            );
        }
        for (id, (cond, fresh)) in bench.iter().zip(&before[men.len()..]) {
            let p = records.player(*id).unwrap();
            assert_eq!((p.condition(), p.freshness()), (*cond, *fresh), "the reserve {id} stands untouched");
        }
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_manager_names_his_men_for_their_set_pieces() {
        use a3_files::an3::Save;
        let game = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let records = Records::from_save(Save::read(&game.join("saves/Toni.an3")).unwrap()).unwrap();
        // the manager in play stands his club on both pitches: the big
        // chance goes to the first man of his saved penalty list who
        // stands in the eleven, the set piece to the first of the direct
        // free kick list behind the keeper's slot — and on this save his
        // named man outranks the strongest, where a computer club's
        // argmax would take the kick himself
        let manager = records.humans().next().expect("Toni.an3 has a manager");
        let mut m = fixture();
        m.ids = [manager.country(), manager.team(), manager.country(), manager.team()];
        let team = records.team(m.ids[0], m.ids[1]).expect("the manager's club");
        assert_ne!(team.human(), 0, "the manager's club names him at +0xb5");
        let eleven = saved_eleven(&m, 0, &records);
        let extra = &records.extras[usize::from(team.human()) - 1];
        for (from, duty) in [(0, Duty::Penalty), (1, Duty::ShotFreeKick)] {
            let want = extra.duty_lists()[duty.stored()]
                .iter()
                .copied()
                .take_while(|&id| id != 0)
                .find(|id| eleven.iter().skip(from).any(|&e| e == *id))
                .or_else(|| eleven.first().copied());
            let got =
                if from == 0 { big_taker(&m, 0, &records) } else { set_taker(&m, 0, &records) };
            assert_eq!(got, want, "the {duty:?} list names its standing man");
            assert!(got.is_some_and(|t| eleven.contains(&t)), "{duty:?} on the pitch");
            let argmax = if from == 0 {
                ai_big_taker(&side_men(&m, 0, &records))
            } else {
                ai_set_taker(&side_men(&m, 0, &records))
            };
            assert_ne!(got, argmax, "the list outranks the strongest here");
        }
        // a national side keeps its lists in its own block, which the
        // replica does not read: the very club now falls to the argmax
        m.nat_bits = 1;
        assert_eq!(
            big_taker(&m, 0, &records),
            ai_big_taker(&side_men(&m, 0, &records)),
            "the national tie lists nobody"
        );
    }

    #[test]
    fn the_shot_event_books_its_assist_and_its_own_goal() {
        let back = squad_player(9, 5, 3, 0, 0, 10);
        let spear = squad_player(7, 50, world::STRIKER, 0, 0, 10);
        let mate = squad_player(8, 50, world::STRIKER, 0, 0, 10);
        let att = [&spear as &Player, &mate];
        let def = [&back as &Player];
        // the rarest throw: the foe heads it into his own net — the
        // blamed man waits in the defending side's own-goal list, and
        // that branch names no assist man at all
        let throw = (1u64..).find(|&s| Random::new(s).below(200) == 0).unwrap();
        let mut m = fixture();
        shot_event(&mut m, 0, Some((&att, def.as_slice())), &mut Random::new(throw));
        assert_eq!(m.goals[0], 1);
        assert_eq!(m.sides[1].own_book[0], 9);
        assert_eq!(&m.sides[1].own_book[1..], &[0u16; LEDGER_SLOTS - 1]);
        assert_eq!(m.sides[0].assist_book, [0; LEDGER_SLOTS]);
        // the plain throw names a scorer, and with two men weighed his
        // assist as well: the assist waits in the attacking side's list
        let struck = (1u64..).find(|&s| Random::new(s).below(200) != 0).unwrap();
        let mut m = fixture();
        shot_event(&mut m, 0, Some((&att, def.as_slice())), &mut Random::new(struck));
        assert_eq!(m.goals[0], 1);
        let named = m.sides[0].assist_book[0];
        assert!(named == 7 || named == 8, "the pick named {named}");
        assert_eq!(&m.sides[0].assist_book[1..], &[0u16; LEDGER_SLOTS - 1]);
        assert_eq!(m.sides[1].own_book, [0; LEDGER_SLOTS]);
        // an eleven none of the weighing lists — one keeper — books no
        // name, and the net stays clean
        let keeper = squad_player(1, 10, world::GOALKEEPER, 0, 0, 10);
        let mut m = fixture();
        let att = [&keeper as &Player];
        shot_event(&mut m, 0, Some((att.as_slice(), def.as_slice())), &mut Random::new(struck));
        assert_eq!(m.goals[0], 0);
        assert_eq!(m.sides[0].assist_book, [0; LEDGER_SLOTS]);
    }

    #[test]
    fn the_named_shots_book_their_goal_kinds() {
        let back = squad_player(9, 5, 3, 0, 0, 10);
        let spear = squad_player(7, 50, world::STRIKER, 0, 0, 10);
        let mate = squad_player(8, 50, world::STRIKER, 0, 0, 10);
        let att = [&spear as &Player, &mate];
        let def = [&back as &Player];
        // the perfect set piece names its taker as kind one — the free
        // kick; an imperfect one names no scorer at all, chance aside
        let mut m = fixture();
        set_piece(&mut m, 0, 1, Some(7));
        assert_eq!(m.sides[0].goal_book[0], (7, 1));
        assert_eq!(m.sides[0].chance_book[0], 7);
        let mut m = fixture();
        set_piece(&mut m, 0, 2, Some(7));
        assert_eq!(m.sides[0].goal_book, [(0, 0); LEDGER_SLOTS]);
        assert_eq!(m.sides[0].chance_book[0], 7, "the taker takes the chance either way");
        // the big chance's goal is his penalty (kind nought), its miss
        // books no name in the goal list
        let goal = (1u64..).find(|&s| Random::new(s).below(100) < 0x50).unwrap();
        let mut m = fixture();
        big_chance(&mut m, 0, Some(7), &mut Random::new(goal));
        assert_eq!(m.sides[0].goal_book[0], (7, 0));
        assert_eq!(m.sides[0].penalty_book, [0; LEDGER_SLOTS]);
        let wide = (1u64..).find(|&s| Random::new(s).below(100) >= 0x50).unwrap();
        let mut m = fixture();
        big_chance(&mut m, 0, Some(7), &mut Random::new(wide));
        assert_eq!(m.sides[0].goal_book, [(0, 0); LEDGER_SLOTS]);
        assert_eq!(m.sides[0].penalty_book[0], 7);
        // the quality-one shot's outcome kind, as FUN_00740899 draws it:
        // headed under 30, struck under 50, nameless fifth else — and
        // the scorer takes his chance count right behind (`FUN_00740a66`)
        for want in [2i16, 3, -1] {
            let seed = (1u64..)
                .find(|&s| {
                    let mut r = Random::new(s);
                    if r.below(200) == 0 {
                        return false;
                    }
                    if scorer_pick(&att, &mut r).0 == 0 {
                        return false;
                    }
                    let o = r.below(100) as i16;
                    let kind = if o < 0x1e { 2 } else if o < 0x32 { 3 } else { -1 };
                    kind == want
                })
                .unwrap();
            let mut m = fixture();
            shot_event(&mut m, 0, Some((&att, def.as_slice())), &mut Random::new(seed));
            assert_eq!(m.goals[0], 1, "the goal write stands under every kind");
            assert_eq!(m.sides[0].goal_book[0].1, want, "kind {want}");
            assert_ne!(m.sides[0].goal_book[0].0, 0);
            assert_eq!(
                m.sides[0].chance_book[0],
                m.sides[0].goal_book[0].0,
                "the scorer takes the chance count too"
            );
        }
    }

    #[test]
    fn the_big_chance_books_the_foe_keeper() {
        // the defending side's first entered man — the walk's keeper —
        // takes the big chance himself: saved in its miss branch,
        // conceded in the goal one; an empty entry list names nobody
        let goal = (1u64..).find(|&s| Random::new(s).below(100) < 0x50).unwrap();
        let wide = (1u64..).find(|&s| Random::new(s).below(100) >= 0x50).unwrap();
        let mut m = fixture();
        m.sides[1].entries[0] = 61;
        big_chance(&mut m, 0, Some(7), &mut Random::new(goal));
        assert_eq!(m.sides[0].keeper_book[0], (61, false));
        assert_eq!(&m.sides[0].keeper_book[1..], &[(0, false); LEDGER_SLOTS - 1]);
        assert_eq!(
            m.sides[1].keeper_book,
            [(0, false); LEDGER_SLOTS],
            "the foe's own list stays free"
        );
        let mut m = fixture();
        m.sides[1].entries[0] = 61;
        big_chance(&mut m, 0, Some(7), &mut Random::new(wide));
        assert_eq!(m.sides[0].keeper_book[0], (61, true));
        // side two's foe keeper stands in the first side's entries; the
        // wide throw misses past its lower mark at 0x46 as well
        let mut m = fixture();
        m.sides[0].entries[0] = 41;
        big_chance(&mut m, 1, Some(7), &mut Random::new(wide));
        assert_eq!(m.sides[1].keeper_book[0], (41, true));
        // the empty line-up — nought at the entries' first slot — lifts
        // nobody's byte
        let mut m = fixture();
        big_chance(&mut m, 0, Some(7), &mut Random::new(goal));
        assert_eq!(m.sides[0].keeper_book, [(0, false); LEDGER_SLOTS]);
    }

    #[test]
    fn the_shots_book_their_takers() {
        // both events write the taker into the shot list before the
        // outcome is drawn; the wide chance stands on the penalty list
        // as well, so his taken byte moves twice, the scored one's once
        let hit = (1u64..).find(|&s| Random::new(s).below(100) < 0x50).unwrap();
        let wide = (1u64..).find(|&s| Random::new(s).below(100) >= 0x50).unwrap();
        let mut m = fixture();
        big_chance(&mut m, 0, Some(7), &mut Random::new(hit));
        assert_eq!(m.sides[0].shot_book[0], (7, true));
        assert_eq!(m.sides[0].penalty_book, [0; LEDGER_SLOTS], "the scored penalty meets no miss list");
        big_chance(&mut m, 0, Some(7), &mut Random::new(wide));
        assert_eq!(m.sides[0].shot_book[1], (7, true), "the attempt stands whichever way the roll goes");
        assert_eq!(m.sides[0].penalty_book[0], 7, "and the wide one meets the miss list besides");
        // the set piece books its taker the same way, free kick by name,
        // the perfect shot and the imperfect one alike
        let mut m = fixture();
        set_piece(&mut m, 0, 1, Some(7));
        assert_eq!(m.sides[0].shot_book[0], (7, false));
        set_piece(&mut m, 0, 2, Some(7));
        assert_eq!(m.sides[0].shot_book[1], (7, false), "the goal's kind aside");
        // no taker, no shot
        let mut m = fixture();
        set_piece(&mut m, 0, 1, None);
        big_chance(&mut m, 0, None, &mut Random::new(hit));
        assert_eq!(m.sides[0].shot_book, [(0, false); LEDGER_SLOTS]);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_walk_books_its_scorers_on_the_save() {
        use a3_files::an3::Save;
        let game = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let read = || Records::from_save(Save::read(&game.join("saves/A.an3")).unwrap()).unwrap();
        let records = read();
        let squad = records.team(0, 0).unwrap().players();
        let men = crate::matches::eleven(&squad, &records);
        let (a, b, c) = (men[0], men[1], men[2]);
        // one seeded scorer per kind on each side; the walk's own
        // events fill the same lists with their picks beside these
        let mut club = fixture();
        club.ids = [0, 0, 0, 0];
        club.sides[0].goal_book[0] = (a, 0);
        club.sides[0].goal_book[1] = (a, 1);
        club.sides[0].goal_book[2] = (a, -1);
        club.sides[1].goal_book[0] = (b, 2);
        club.sides[1].goal_book[1] = (b, 3);
        let mut tie = fixture();
        tie.ids = [0, 0, 0, 0];
        tie.nat_bits = 1;
        tie.sides[0].goal_book[0] = (c, 0);
        let mut row = [club, tie];
        let counters = |r: &Records, id: u16| {
            let p = r.player(id).unwrap();
            (
                p.scorer_marks(),
                p.last_goals(),
                p.goals()[0],
                p.upgrade_count(),
                p.home_goals(),
                p.joker_goals(),
                p.penalties()[1],
                p.free_kicks()[1],
                p.goal_kinds(),
            )
        };
        let (before_a, before_b, before_c) =
            (counters(&records, a), counters(&records, b), counters(&records, c));
        let joker = |id: u16| (read().player(id).unwrap().match_bits() >> 1) & 1;
        let at_home = |id: u16| read().player(id).unwrap().club()[1] as u16 == 0;
        let mut records = read();
        play_matches(&mut row, false, Some(&mut records), None, test_day(), &mut Random::new(6));
        let seen_club = row[0];
        // how often each name stands in the walked club's goal list,
        // wholly and per kind
        let marks = |id: u16| -> u8 {
            seen_club
                .sides
                .iter()
                .flat_map(|s| s.goal_book.iter())
                .filter(|&&(x, _)| x == id)
                .count() as u8
        };
        let of_kind = |id: u16, kind: i16| -> u8 {
            seen_club
                .sides
                .iter()
                .flat_map(|s| s.goal_book.iter())
                .filter(|&&(x, k)| x == id && k == kind)
                .count() as u8
        };
        // every booked man lifts the mark, the league goal, the last
        // match's and the upgrade count alike; the home byte only with a
        // man of the first side's club, the substitute byte only with
        // those whose match bits carry bit 2 — then once per entry
        let expect = |before: (u8, u8, u8, u8, u8, u8, u8, u8, [u8; 5]), id: u16| {
            let mut kinds = before.8;
            kinds[0] = kinds[0].wrapping_add(of_kind(id, 0));
            kinds[1] = kinds[1].wrapping_add(of_kind(id, 1));
            kinds[2] = kinds[2].wrapping_add(of_kind(id, 3));
            kinds[3] = kinds[3].wrapping_add(of_kind(id, 2));
            kinds[4] = kinds[4].wrapping_add(
                marks(id)
                    .wrapping_sub(of_kind(id, 0))
                    .wrapping_sub(of_kind(id, 1))
                    .wrapping_sub(of_kind(id, 2))
                    .wrapping_sub(of_kind(id, 3)),
            );
            (
                before.0.wrapping_add(marks(id)),
                before.1.wrapping_add(marks(id)),
                before.2.wrapping_add(marks(id)),
                before.3.wrapping_add(marks(id)),
                before.4.wrapping_add(if at_home(id) { marks(id) } else { 0 }),
                before.5.wrapping_add(if joker(id) != 0 { marks(id) } else { 0 }),
                before.6.wrapping_add(of_kind(id, 0)),
                before.7.wrapping_add(of_kind(id, 1)),
                kinds,
            )
        };
        assert_eq!(counters(&records, a), expect(before_a, a), "the seeded names meet the records");
        assert_eq!(counters(&records, b), expect(before_b, b), "the walk's own picks likewise");
        // the national tie sits out the scorer's books as the rest: its
        // list stays filled, its man's counters stand as they were
        assert_eq!(seen_tie_goal(&row[1]), Some((c, 0)));
        assert_eq!(counters(&records, c), before_c, "the national tie's man is not booked");
        fn seen_tie_goal(m: &Match) -> Option<(u16, i16)> {
            m.sides[0].goal_book.iter().find(|s| s.0 != 0).copied()
        }
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_walk_books_the_foe_keepers_on_the_save() {
        use a3_files::an3::Save;
        let game = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let read = || Records::from_save(Save::read(&game.join("saves/A.an3")).unwrap()).unwrap();
        let records = read();
        let squad = records.team(0, 0).unwrap().players();
        let men = crate::matches::eleven(&squad, &records);
        let (gk, mate) = (men[0], men[1]);
        // seeded foe keepers on the club's lists, saved and conceded
        // alike — the walk's own big chances fill the same lists with
        // their picks beside these
        let mut club = fixture();
        club.ids = [0, 0, 0, 0];
        club.sides[0].keeper_book[0] = (gk, true);
        club.sides[0].keeper_book[1] = (mate, false);
        club.sides[1].keeper_book[0] = (gk, false);
        let mut tie = fixture();
        tie.ids = [0, 0, 0, 0];
        tie.nat_bits = 1;
        tie.sides[0].keeper_book[0] = (mate, true);
        let mut row = [club, tie];
        let pair = |r: &Records, id: u16| {
            let p = r.player(id).unwrap();
            (p.saves(), p.conceded())
        };
        let (before_gk, before_mate) = (pair(&records, gk), pair(&records, mate));
        let mut records = read();
        play_matches(&mut row, false, Some(&mut records), None, test_day(), &mut Random::new(6));
        let seen_club = row[0];
        // how often each name stands in the walked club's keeper list,
        // saved and conceded apart
        let of_outcome = |id: u16, saved: bool| -> u8 {
            seen_club
                .sides
                .iter()
                .flat_map(|s| s.keeper_book.iter())
                .filter(|&&(x, s)| x == id && s == saved)
                .count() as u8
        };
        let expect = |before: (u8, u8), id: u16| {
            (
                before.0.wrapping_add(of_outcome(id, true)),
                before.1.wrapping_add(of_outcome(id, false)),
            )
        };
        assert_eq!(pair(&records, gk), expect(before_gk, gk), "the seeded names meet the records");
        assert_eq!(pair(&records, mate), expect(before_mate, mate), "the walk's own picks likewise");
        // the national tie sits out the keeper's books as the rest: its
        // list stays filled, and its men's bytes stand by the club's
        // counts alone
        assert_eq!(seen_tie_keeper(&row[1]), Some((mate, true)));
        fn seen_tie_keeper(m: &Match) -> Option<(u16, bool)> {
            m.sides[0].keeper_book.iter().find(|s| s.0 != 0).copied()
        }
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_walk_books_its_shots_on_the_save() {
        use a3_files::an3::Save;
        let game = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let read = || Records::from_save(Save::read(&game.join("saves/A.an3")).unwrap()).unwrap();
        let records = read();
        let squad = records.team(0, 0).unwrap().players();
        let men = crate::matches::eleven(&squad, &records);
        let (a, b, c) = (men[0], men[1], men[2]);
        // seeded takers on the club's lists, penalty and free kick
        // alike, and one scored kind per list for the club's own pair;
        // the walk's own events fill the same lists with their picks
        // beside these
        let mut club = fixture();
        club.ids = [0, 0, 0, 0];
        club.sides[0].shot_book[0] = (a, true);
        club.sides[0].shot_book[1] = (b, false);
        club.sides[1].shot_book[0] = (b, true);
        club.sides[0].goal_book[0] = (c, 0);
        club.sides[1].goal_book[0] = (c, 1);
        let mut tie = fixture();
        tie.ids = [0, 0, 0, 0];
        tie.nat_bits = 1;
        tie.sides[0].shot_book[0] = (c, true);
        let mut row = [club, tie];
        let taken = |r: &Records, id: u16| {
            let p = r.player(id).unwrap();
            (p.penalties()[0], p.free_kicks()[0])
        };
        let club_pair = |r: &Records| {
            let t = r.team(0, 0).unwrap();
            (t.penalties(), t.free_kicks())
        };
        let (before_a, before_b, before_c) =
            (taken(&records, a), taken(&records, b), taken(&records, c));
        let (before_pens, before_fks) = club_pair(&records);
        let mut records = read();
        play_matches(&mut row, false, Some(&mut records), None, test_day(), &mut Random::new(6));
        let seen_club = row[0];
        // how often each name meets the walked club's shot list, by the
        // shot's name — and how often its wide chances meet the miss
        // list once more, and its scored kinds stand in the goal list
        let shots = |id: u16, penalty: bool| -> u8 {
            seen_club
                .sides
                .iter()
                .flat_map(|s| s.shot_book.iter())
                .filter(|&&(x, p)| x == id && p == penalty)
                .count() as u8
        };
        let wide = |id: u16| -> u8 {
            seen_club
                .sides
                .iter()
                .flat_map(|s| s.penalty_book.iter())
                .filter(|&&x| x == id)
                .count() as u8
        };
        let scored = |kind: i16| -> u8 {
            seen_club
                .sides
                .iter()
                .flat_map(|s| s.goal_book.iter())
                .filter(|&&(x, k)| x != 0 && k == kind)
                .count() as u8
        };
        // the taken bytes stand as many higher as the name meets the
        // shot list, the wide chance's taker as many more again from
        // the miss branch's own lift
        let expect = |before: (u8, u8), id: u16| {
            (
                before.0.wrapping_add(shots(id, true)).wrapping_add(wide(id)),
                before.1.wrapping_add(shots(id, false)),
            )
        };
        assert_eq!(taken(&records, a), expect(before_a, a), "the seeded names meet the records");
        assert_eq!(taken(&records, b), expect(before_b, b), "the walk's own picks likewise");
        // the club at the fixtures' ids counts every shot its sides
        // fired, and its own pair's scored byte every kind the goal
        // lists name
        let all_shots = |penalty: bool| -> u8 {
            seen_club
                .sides
                .iter()
                .flat_map(|s| s.shot_book.iter())
                .filter(|s| s.0 != 0 && s.1 == penalty)
                .count() as u8
        };
        let (pens, fks) = club_pair(&records);
        assert_eq!(
            pens,
            [
                before_pens[0].wrapping_add(all_shots(true)),
                before_pens[1].wrapping_add(scored(0))
            ],
            "the awarded and the scored penalty byte"
        );
        assert_eq!(
            fks,
            [
                before_fks[0].wrapping_add(all_shots(false) as u16),
                before_fks[1].wrapping_add(scored(1) as u16)
            ],
            "the taken and the scored free kick short"
        );
        // the national tie sits out the shot's books as the rest: its
        // list stays filled, and its man's bytes stand by the club's
        // counts alone
        assert_eq!(row[1].sides[0].shot_book[0], (c, true));
        assert_eq!(taken(&records, c), expect(before_c, c), "the national tie's man is not booked");
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_walk_books_its_counters_on_the_save() {
        use a3_files::an3::Save;
        let game = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let read = || Records::from_save(Save::read(&game.join("saves/A.an3")).unwrap()).unwrap();
        let records = read();
        let squad = records.team(0, 0).unwrap().players();
        let men = crate::matches::eleven(&squad, &records);
        let (a, b, c, d) = (men[0], men[1], men[2], men[3]);
        // one seeded name per book on each tie; the walk's own events
        // fill the same lists with their picks beside these
        let mut club = fixture();
        club.ids = [0, 0, 0, 0];
        club.sides[0].chance_book[0] = a;
        club.sides[0].assist_book[0] = a;
        club.sides[0].penalty_book[0] = a;
        club.sides[1].own_book[0] = b;
        let mut tie = fixture();
        tie.ids = [0, 0, 0, 0];
        tie.nat_bits = 1;
        tie.sides[0].chance_book[0] = c;
        tie.sides[0].assist_book[0] = c;
        tie.sides[0].penalty_book[0] = c;
        tie.sides[1].own_book[0] = d;
        let mut row = [club, tie];
        let mut records = read();
        let counters = |r: &Records, id: u16| {
            let p = r.player(id).unwrap();
            (p.chances(), p.assists(), p.last_assists(), p.penalties()[0], p.own_goals())
        };
        let (before_a, before_b) = (counters(&records, a), counters(&records, b));
        let (before_c, before_d) = (counters(&records, c), counters(&records, d));
        play_matches(&mut row, false, Some(&mut records), None, test_day(), &mut Random::new(6));
        let (seen_club, seen_tie) = (row[0], row[1]);
        // every name standing in the walked club's lists meets the
        // records once — the seeded one and the walk's own picks alike
        let listed = |m: &Match, id: u16, which: u8| -> u8 {
            m.sides
                .iter()
                .flat_map(|s| match which {
                    0 => s.chance_book.iter(),
                    1 => s.assist_book.iter(),
                    2 => s.own_book.iter(),
                    _ => s.penalty_book.iter(),
                })
                .filter(|&&x| x == id)
                .count() as u8
        };
        // the taken penalty byte meets the shot list too, the penalty
        // shots only — a wide chance's taker stands on both lists and
        // moves twice
        let shot_taken = |m: &Match, id: u16| -> u8 {
            m.sides
                .iter()
                .flat_map(|s| s.shot_book.iter())
                .filter(|&&(x, penalty)| x == id && penalty)
                .count() as u8
        };
        // each counter stands exactly as many higher as its name meets
        // the walked lists — chance, assist, and own-goal books alike
        let expect = |before: (u8, u8, u8, u8, u8), id: u16| {
            (
                before.0.wrapping_add(listed(&seen_club, id, 0)),
                before.1.wrapping_add(listed(&seen_club, id, 1)),
                before.2.wrapping_add(listed(&seen_club, id, 1)),
                before.3
                    .wrapping_add(listed(&seen_club, id, 3))
                    .wrapping_add(shot_taken(&seen_club, id)),
                before.4.wrapping_add(listed(&seen_club, id, 2)),
            )
        };
        assert_eq!(counters(&records, a), expect(before_a, a), "the seeded names meet the records once");
        assert_eq!(counters(&records, b), expect(before_b, b), "the walk's own picks likewise");
        // the national tie sits out the books as it sits out the wear:
        // its lists stay full, its men's counters stand as they were
        assert_eq!(seen_tie.sides[0].assist_book[0], c);
        assert_eq!(counters(&records, c), before_c, "the national tie's man is not booked");
        assert_eq!(counters(&records, d), before_d, "the national tie's own-goal man is not booked");
    }

    #[test]
    fn fouls_book_yellow_and_red_cards() {
        use a3_files::an3::Save;
        // the kind tables walk their throw: entry nought is empty, so the
        // plain table's first eight throws name kind one; the other draw's
        // six land on 26..31
        assert_eq!(foul_kind(&FOULS, 0, 1), 1);
        assert_eq!(foul_kind(&FOULS, 8, 1), 2);
        assert_eq!(foul_kind(&FOULS, 11, 1), 4);
        assert_eq!(foul_kind(&FOULS, 15, 1), 7);
        assert_eq!(foul_kind(&FOULS_OTHER, 0, 0x20), 26);
        assert_eq!(foul_kind(&FOULS_OTHER, 5, 0x20), 31);
        assert_eq!(foul_kind(&FOULS_TRAIT, 27, 1), 1, "a throw past the table names one");
        let game = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let mut records = Records::from_save(Save::read(&game.join("saves/A.an3")).unwrap()).unwrap();
        let mut m = fixture();
        m.ids = [0, 0, 0, 1];
        m.sides[0].men = 11;
        m.sides[0].morale = 100;
        let eleven = pitch_eleven(&m, 0, &records);
        assert_eq!(eleven.len(), 11);
        // every man of the eleven stands among the candidates, the keeper
        // once and at place nought
        let list = foul_candidates(&m, 0, &eleven, &records);
        for (slot, &id) in eleven.iter().enumerate() {
            let p = records.player(id).unwrap();
            let n = list.iter().filter(|&&s| s as usize == slot).count();
            if p.positions()[0] == world::GOALKEEPER {
                assert_eq!(slot, 0);
                assert_eq!(n, 1, "the keeper stands once");
            } else {
                assert!(1 < n, "a field man stands his weight's tenth and one");
            }
        }
        // a first card cautions the man, a second sends him off
        let man = eleven[3];
        book_card(&mut m, 0, man, &eleven, 4, false);
        assert_eq!(m.bookings, 1);
        assert_eq!(m.sides[0].booked[0], man);
        assert_eq!(m.sides[0].cautions, [1, 1]);
        assert_eq!(m.sides[0].caution_bits, 1);
        assert_eq!(m.sides[0].hard_fouls, 1, "kind 4 is a hard foul");
        assert_eq!(m.sides[0].men, 11);
        assert!(cautioned(&m.sides[0], records.player(man).unwrap()));
        let weighed = foul_candidates(&m, 0, &eleven, &records);
        assert!(
            weighed.iter().filter(|&&s| s == 3).count() < list.iter().filter(|&&s| s == 3).count(),
            "a cautioned man weighs less"
        );
        book_card(&mut m, 0, man, &eleven, 2, true);
        assert_eq!(m.bookings, 2);
        assert_eq!(m.sides[0].sent_off[0], man);
        assert_eq!(m.sides[0].reds, 1);
        assert_eq!(m.sides[0].men, 10);
        assert_eq!(m.sides[0].morale, 105, "the side's mood takes 105 of a hundred");
        assert_eq!(m.sides[0].foul_marks, 0, "kind 2 leaves the mark");
        assert_eq!(m.sides[0].cautions, [1, 1], "the red card is no caution");
        let pitch = pitch_eleven(&m, 0, &records);
        assert_eq!(pitch[3], 0, "his place stands empty");
        assert!(!side_men(&m, 0, &records).iter().any(|p| p.id() == man));
        // a man off the pitch books nothing
        book_card(&mut m, 0, man, &pitch, 4, true);
        assert_eq!(m.bookings, 2);
        // a side down to eight men draws no foul at all
        let mut few = fixture();
        few.ids = [0, 0, 0, 1];
        few.sides[0].men = 8;
        let mut random = Random::new(3);
        let before = random.clone();
        booking(&mut few, 0, &records, &mut random);
        assert_eq!(random, before);
        assert_eq!(few.bookings, 0);
        // a full side draws its man out of the eleven and books him
        let mut full = fixture();
        full.ids = [0, 0, 0, 1];
        full.sides[0].men = 11;
        booking(&mut full, 0, &records, &mut Random::new(3));
        assert_eq!(full.bookings, 1);
        let named = full.sides[0].booked[0];
        assert!(named != 0 && eleven.contains(&named));
        // the walk's chooser hands the minute's foul to its side
        let booked = (1u64..200).any(|seed| {
            let mut m = fixture();
            m.ids = [0, 0, 0, 1];
            m.sides[0].men = 11;
            m.sides[1].men = 11;
            let mut random = Random::new(seed);
            for _ in 0..90 {
                chance_minute(&mut m, &Clock::default(), Some(&mut records), &mut random);
            }
            0 < m.bookings
        });
        assert!(booked, "ninety minutes book a card now and then");
    }

    #[test]
    fn roll_699_sends_a_man_straight_off() {
        use a3_files::an3::Save;
        // the tables' widths are their throws'; entries nought and one
        // are empty, the other table names 26..32
        for (table, width) in [(&REDS, 0x66), (&REDS_FRONT, 0x52), (&REDS_HARD, 0x79), (&REDS_OTHER, 7)] {
            assert_eq!(table.iter().take(0x22).map(|&x| x as i32).sum::<i32>(), width);
        }
        assert_eq!(foul_kind(&REDS, 0, 1), 2);
        assert_eq!(foul_kind(&REDS, 6, 1), 8);
        assert_eq!(foul_kind(&REDS_FRONT, 6, 1), 9, "a forward's table has no kind 8");
        assert_eq!(foul_kind(&REDS_OTHER, 0, 1), 26);
        assert_eq!(foul_kind(&REDS_OTHER, 6, 1), 32);
        let game = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let mut records = Records::from_save(Save::read(&game.join("saves/A.an3")).unwrap()).unwrap();
        let side = |men| {
            let mut m = fixture();
            m.ids = [0, 0, 0, 1];
            m.sides[0].men = men;
            m.sides[0].morale = 100;
            m
        };
        // a side of nine draws nothing
        let mut nine = side(9);
        let mut random = Random::new(5);
        let before = random.clone();
        assert_eq!(straight_red(&mut nine, 0, &records, 10, &mut random), None);
        assert_eq!(random, before);
        // a full side's man goes off, early in the match with the mark
        let mut early = side(11);
        let kind = straight_red(&mut early, 0, &records, 10, &mut Random::new(5));
        assert!(matches!(kind, Some(1..=0x21)));
        let s = &early.sides[0];
        let man = s.sent_off[0];
        assert!(man != 0 && saved_eleven(&early, 0, &records).contains(&man));
        assert_eq!((early.bookings, s.men, s.straight_reds, s.reds), (1, 10, 1, 0));
        assert_eq!(s.morale, 105);
        assert_eq!(s.foul_marks, 0x20);
        assert_eq!(s.booked, [0; LEDGER_SLOTS], "no caution");
        assert_eq!(pitch_eleven(&early, 0, &records).iter().filter(|&&id| id == 0).count(), 1);
        let mut late = side(11);
        straight_red(&mut late, 0, &records, 31, &mut Random::new(5));
        assert_eq!(late.sides[0].foul_marks, 0, "from the 31st minute no mark");
        assert_eq!(late.sides[0].sent_off, early.sides[0].sent_off);
        // the frame's no-chance roll 699 sends off until nine are left,
        // and without the save nothing happens
        let mut m = side(11);
        let mut bare = side(11);
        let mut random = Random::new(7);
        let mut other = Random::new(7);
        for _ in 0..20_000 {
            chance_for(&mut m, 0, 0.0, 60, Some(&mut records), &mut random);
            chance_for(&mut bare, 0, 0.0, 60, None, &mut other);
        }
        assert_eq!((m.sides[0].men, m.sides[0].straight_reds, m.bookings), (9, 2, 2));
        assert_eq!((bare.sides[0].men, bare.bookings), (11, 0));
        assert_eq!(m.sides[1], side(11).sides[1], "the other side stands untouched");
    }

    #[test]
    fn rolls_900_to_903_injure_a_man() {
        use a3_files::an3::Save;
        let game = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        // the kinds' rows stand as the exe holds them
        let exe = a3_files::exe::Exe::open(&game.join("anstoss3.exe")).unwrap();
        let rows = exe.bytes(0x8ac340, 10 * 0x44).unwrap();
        for (k, row) in rows.chunks(10).enumerate() {
            assert_eq!((row[0], row[1], row[5]), (INJURY_RECURS[k], INJURY_DAYS[k], INJURY_WEIGHTS[k]), "kind {k}");
        }
        let mut records = Records::from_save(Save::read(&game.join("saves/A.an3")).unwrap()).unwrap();
        let mut m = fixture();
        m.ids = [0, 0, 0, 1];
        m.sides[0].men = 11;
        let eleven = pitch_eleven(&m, 0, &records);
        let list = injury_candidates(&eleven, &records);
        for (slot, &id) in eleven.iter().enumerate() {
            let n = list.iter().filter(|&&s| usize::from(s) == slot).count();
            assert!(id == 0 || (5..=0x13).contains(&n), "place {slot}: {n}");
        }
        // a lingering knock comes back on about half the throws, a row's
        // byte 2 as the worse kind
        let id = eleven[0];
        crate::medical::injure_for(&mut records, id, 3);
        for (kind, back) in [(3u8, 3u8), (9, 10), (0x1d, 0x1e)] {
            records.player_mut(id).unwrap().set_injury_kind(kind);
            let p = records.player(id).unwrap();
            assert_eq!(lingering(&records, p).1, INJURY_RECURS[usize::from(kind)]);
            let again = (0..400).filter(|&s| injury_kind(&records, p, 0, &mut Random::new(s)) == back).count();
            assert!((150..260).contains(&again), "kind {kind}: {again}");
        }
        // the spared kinds come up less for a club with the flag bit 2
        let p = records.player(eleven[1]).unwrap();
        let spared = |flags| (0..3000).filter(|&s| SPARED_KINDS.contains(&injury_kind(&records, p, flags, &mut Random::new(s)))).count();
        assert!(spared(2) * 2 < spared(0), "{} {}", spared(2), spared(0));
        // the man takes the kind from today for a day at least
        let (man, kind) = injury(&mut m, 0, &mut records, &mut Random::new(5)).unwrap();
        assert!(eleven.contains(&man));
        let p = records.player(man).unwrap();
        assert!(matches!(kind, 1..=0x3d | 0x43));
        assert_eq!((p.injury_kind(), p.injured_on()), (kind, records.globals.day()));
        assert!(1 <= crate::load::days_to(&records.globals, p.injured_to()));
        // the frame's rolls 900..903 injure a man now and then; without
        // the save nothing happens
        let mut fresh = Records::from_save(Save::read(&game.join("saves/A.an3")).unwrap()).unwrap();
        let hurt = |records: &Records| {
            eleven.iter().filter(|&&id| id != 0 && records.player(id).unwrap().injured_on() == records.globals.day()).count()
        };
        assert_eq!(hurt(&fresh), 0);
        let mut random = Random::new(9);
        for _ in 0..3000 {
            chance_for(&mut m, 0, 0.0, 60, Some(&mut fresh), &mut random);
        }
        assert!(0 < hurt(&fresh));
    }

    /// The kick-off's bonus against a man's salary: the row's nought
    /// leaves every paid man at 96 % and an unpaid one whole, a bonus of
    /// one and a half units at 98, two whole, and past fifty his part at
    /// 112.
    #[test]
    fn the_bonus_bends_a_mans_part_by_his_salary() {
        assert_eq!(bonus_percent(0, 300), 96);
        assert_eq!(bonus_percent(0, 0), 100);
        assert_eq!(bonus_percent(15 * 300, 300), 98);
        assert_eq!(bonus_percent(20 * 300, 300), 100);
        assert_eq!(bonus_percent(31 * 300, 300), 103);
        assert_eq!(bonus_percent(501 * 300, 300), 112);
    }

    /// The original's trace of a season's days (the oracle's
    /// `runs/season`, its dumps a day apart and `draws.bin` from the
    /// start): between one dump's generator and the next lies the day's
    /// pre-match walk, and in its fifth loop the kick-off's slack throws
    /// (`FUN_006f6fc7`, return addresses 0x6f7469.. for the first side,
    /// 0x6f757a.. for the second) name each favourite by more than forty
    /// or twenty, the clubs' tactics (`FUN_0042c1aa`, 0x6f007e and
    /// 0x6f009d) parting the matches. On every league day whose row holds
    /// only the leagues the day lists, the replica's kick-off over the
    /// dump's records, the row lined up first, names the original's
    /// favourite — or none — match by match: 611 of 612 over ten days,
    /// the one apart a second side 22 ahead on the day.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR and the oracle's season run from $ANSTOSS3_ORACLE"]
    fn the_kick_off_names_the_originals_favourites() {
        let Some((data, draws, traced)) = traced_days() else { return };
        let kick = |c: u32| (0x6f6fc7..0x6f791a).contains(&c);
        let tactic = |above: u32| above == 0x6f007e || above == 0x6f009d;
        let (mut days, mut matches, mut agreed) = (0, 0, 0);
        for TracedDay { day, start, end, records, row, .. } in traced {
            let (mut records, mut row) = (records, row);
            // the day's fifth loop: from its first throw or tactic on,
            // as long as only those come
            let Some(first) = (start..end).find(|&i| kick(draws[i].0) || tactic(draws[i].3)) else {
                continue;
            };
            let span: Vec<_> = draws[first..].iter().take_while(|d| kick(d.0) || tactic(d.3)).collect();
            // the original's matches in order, each its favourite — the
            // side and whether by forty — or none: a match closes with its
            // clubs' tactics, three throws a computer side
            let mut theirs: Vec<Option<(usize, bool)>> = Vec::new();
            let (mut open, mut last, mut run) = (false, 0u32, 0);
            for d in &span {
                let fresh = if kick(d.0) {
                    last != 0 && last != 1
                } else {
                    (d.3 == 0x6f007e && (last == 0x6f009d || (last == 0x6f007e && run == 3)))
                        || (d.3 == 0x6f009d && last == 0x6f009d && run == 3)
                };
                if fresh || !open {
                    theirs.push(None);
                    open = true;
                }
                let cat = match d.0 {
                    0x6f7469 => Some((0, true)),
                    0x6f7500 => Some((0, false)),
                    0x6f757a => Some((1, true)),
                    0x6f761a => Some((1, false)),
                    _ => None,
                };
                if let Some(cat) = cat {
                    *theirs.last_mut().unwrap() = Some(cat);
                }
                let now = if kick(d.0) { 1 } else { d.3 };
                run = if now == last { run + 1 } else { 1 };
                last = now;
            }
            // the replica's, over the day's league matches in the
            // listing's order
            if theirs.len() != row.len() {
                eprintln!("day {day}: {} matches listed, the original's row {}", row.len(), theirs.len());
                continue;
            }
            line_up_row(&row, &mut records, &data);
            let mut ours = Vec::new();
            for m in row.iter_mut() {
                kick_off_strengths(m, &records, &data, &mut Random::new(1));
                let (a, b) = (m.sides[0].day[0], m.sides[1].day[0]);
                ours.push(if b + 40 < a {
                    Some((0, true))
                } else if b + 20 < a {
                    Some((0, false))
                } else if a + 40 < b {
                    Some((1, true))
                } else if a + 20 < b {
                    Some((1, false))
                } else {
                    None
                });
            }
            days += 1;
            matches += row.len();
            for (k, (t, o)) in theirs.iter().zip(&ours).enumerate() {
                if t == o {
                    agreed += 1;
                } else {
                    let m = &row[k];
                    eprintln!("day {day} match {k} {:?}: theirs {t:?}, ours {o:?} at {} {}", m.ids, m.sides[0].day[0], m.sides[1].day[0]);
                }
            }
        }
        eprintln!("{days} days, {agreed} of {matches} matches agree");
        assert!(days >= 8, "only {days} league days found in the trace");
        assert!(agreed * 100 >= matches * 99, "{agreed} of {matches} matches name the original's favourite");
    }

    /// A league day of the oracle's season run: where its dump's
    /// generator stands in the trace and the next dump's, that state,
    /// the dump's records and the day's league matches in the listing's
    /// order.
    struct TracedDay {
        day: u16,
        start: usize,
        end: usize,
        state: (i64, i64),
        records: Records,
        row: Vec<Match>,
        /// Where each match of the row writes its result: the league's
        /// block and the four bytes' offset in it.
        slots: Vec<(String, usize)>,
    }

    /// The oracle's season run (`runs/season`, its dumps a day apart, and
    /// `draws.bin` from the start): the game data, the main generator's
    /// moving draws — caller, range, result, the first return address
    /// above — and the league days whose dumps align in the trace.
    fn traced_days() -> Option<(crate::data::Data, Vec<(u32, u32, u32, u32)>, Vec<TracedDay>)> {
        use a3_files::an3::Save;
        let Some(oracle) = std::env::var_os("ANSTOSS3_ORACLE") else {
            eprintln!("no oracle season run among the saves: ANSTOSS3_ORACLE is not set");
            return None;
        };
        let runs = std::path::PathBuf::from(oracle).join("runs");
        let Ok(trace) = std::fs::read(runs.join("draws.bin")) else {
            eprintln!("no oracle season trace among the saves");
            return None;
        };
        if !runs.join("season/day-064.an3").exists() {
            eprintln!("no oracle season dumps among the saves");
            return None;
        }
        let game = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let data = crate::data::Data::load(&game).unwrap();
        let u32_at = |r: &[u8], at: usize| u32::from_le_bytes(r[at..at + 4].try_into().unwrap());
        let draws: Vec<(u32, u32, u32, u32)> = trace
            .chunks_exact(48)
            .filter(|r| r[12] == 0 && u32_at(r, 4) >= 2)
            .map(|r| (u32_at(r, 0), u32_at(r, 4), u32_at(r, 8), u32_at(r, 16)))
            .collect();
        // where each dump's generator stands in the trace
        let mut starts = Vec::new();
        let mut from = 0;
        for day in 63..=139u16 {
            let dump = Save::read(&runs.join(format!("season/day-{day:03}.an3"))).unwrap();
            let word = |name: &str| i64::from(i32::from_le_bytes(dump.block(name).unwrap().data[..4].try_into().unwrap()));
            let state = (word("0x89dcc0"), word("0x89dcc4"));
            let start = (from..draws.len().saturating_sub(8)).find(|&i| {
                let mut r = Random::from_state(state.0, state.1);
                draws[i..i + 8].iter().all(|d| r.below(d.1) == d.2)
            });
            if let Some(start) = start {
                from = start;
            }
            starts.push((day, start, state, dump));
        }
        let mut days = Vec::new();
        for pair in starts.windows(2) {
            let ((day, Some(start), state, dump), (_, Some(end), _, _)) = (&pair[0], &pair[1]) else { continue };
            let records = Records::from_save(dump.clone()).unwrap();
            let mut row = Vec::new();
            let mut slots = Vec::new();
            let main = u16::try_from(records.globals.main_country()).unwrap_or(0);
            let more = usize::try_from(records.globals.more_count()).unwrap_or(0);
            let firsts: Vec<(u16, u8)> =
                records.globals.more_countries()[..more].iter().map(|&c| (u16::try_from(c).unwrap_or(0), 0)).collect();
            let leagues: Vec<(u16, u8)> =
                (0..records.countries.leagues(usize::from(main)).max(0)).map(|l| (main, l as u8)).chain(firsts).collect();
            for (country, level) in leagues {
                let Some(r) = data.calendar.row(country, usize::from(level)) else { continue };
                let Some(matchday) = data.calendar.matchday(r, usize::from(*day)) else { continue };
                let Some(league) = crate::opponent::League::of(&records, country, level) else { continue };
                if league.played() + 1 != matchday {
                    continue;
                }
                let name = crate::opponent::League::block_name(&records, country, level).unwrap_or_default();
                for (home, away, at) in league.open_games() {
                    let mut m = Match::default();
                    m.ids = [country, league.team(home) as u16, country, league.team(away) as u16];
                    row.push(m);
                    slots.push((name.clone(), at));
                }
            }
            if !row.is_empty() {
                days.push(TracedDay { day: *day, start: *start, end: *end, state: *state, records, row, slots });
            }
        }
        Some((data, draws, days))
    }

    /// The whole pre-match walk against the original's trace: from the
    /// dump's generator replayed up to the walk's first throw, the
    /// replica's fifth loop (the kick-off's slack, the press windows and
    /// the computer clubs' tactics) leaves the generator where the
    /// original's does on every traced league day but the one whose
    /// favourite it misnames; its sixth loop, from the original's state
    /// there, throws once for every listed man of both sides and then
    /// the first half's added time (`FUN_00658cfb`, 0x658cfb..; then
    /// `FUN_0040bc1b`), the doping control's picks between on the match
    /// the listing marked, match by match as the original does, and ends
    /// each half where the original's throw puts it: 585 of 585 matches
    /// over ten days. The human's own match takes the national branch
    /// there (`FUN_00677428` names the human managers' clubs, +0x474;
    /// `FUN_006fc8c6`, the bulk run `FUN_0070c334`), which waits; the
    /// test steps over it on the original's throws.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR and the oracle's season run from $ANSTOSS3_ORACLE"]
    fn the_walk_throws_as_the_originals_league_days() {
        let Some((data, draws, traced)) = traced_days() else { return };
        let kick = |c: u32| (0x6f6fc7..0x6f791a).contains(&c);
        let tactic = |above: u32| above == 0x6f007e || above == 0x6f009d;
        let press = |c: u32| (0x6f9656..0x6f9788).contains(&c);
        let man = |c: u32| (0x658cfb..0x658fb2).contains(&c);
        let added = |c: u32| (0x40bc1b..0x40bc96).contains(&c);
        let fifth = |d: &(u32, u32, u32, u32)| kick(d.0) || tactic(d.3) || press(d.0);
        let (mut days, mut wholes, mut fifths, mut matches, mut sixths, mut pressed) = (0, 0, 0, 0, 0, 0);
        for TracedDay { day, start, end, state, records, row, .. } in traced {
            let Some(first) = (start..end).find(|&i| fifth(&draws[i])) else { continue };
            // the generator at the walk's first throw, the day's draws
            // before it replayed; a day whose trace the replay leaves
            // (the generator set anew on the way) is passed
            let mut original = Random::from_state(state.0, state.1);
            if !draws[start..first].iter().all(|d| original.below(d.1) == d.2) {
                eprintln!("day {day}: the trace does not replay to the walk");
                continue;
            }
            let six = first + draws[first..].iter().take_while(|d| fifth(d)).count();
            pressed += draws[first..six].iter().filter(|d| press(d.0)).count();
            // the original's sixth loop, match by match: a human's
            // club's side first draws for its national branch
            // (`FUN_006fc8c6`, 0x6fc8c6..), then each listed man throws,
            // the doping control picks (0x5dac94..0x5dae5f), a human's
            // match runs in bulk (`FUN_0070c334`), and the added time
            let mut theirs = Vec::new();
            let mut i = six;
            loop {
                let from = i;
                let national = draws[i..].iter().take_while(|d| (0x6fc8c6..0x6fc9dc).contains(&d.0)).count();
                i += national;
                let n = draws[i..].iter().take_while(|d| man(d.0)).count();
                i += n;
                let picks = draws[i..].iter().take_while(|d| (0x5dac94..0x5dae5f).contains(&d.0)).count();
                i += picks;
                let bulk = if national > 0 { draws[i..].iter().take_while(|d| !added(d.0)).count() } else { 0 };
                i += bulk;
                if !added(draws[i].0) {
                    break;
                }
                i += 1;
                theirs.push((national > 0, n, picks > 0, &draws[from..i]));
            }
            if theirs.len() < row.len() {
                let at = row.get(theirs.len()).map(|m| m.ids);
                eprintln!("day {day}: {} matches listed, the original's sixth loop {} up to {at:?}", row.len(), theirs.len());
                continue;
            }
            // a row the original holds more matches in than the day's
            // leagues list (its cups) leaves the fifth loop's end apart
            let whole = theirs.len() == row.len();
            days += 1;
            wholes += usize::from(whole);
            let (mut records, mut row) = (records, row);
            for m in row.iter_mut() {
                m.home_nation = m.ids[0] as u8;
                m.home_id = m.ids[1];
            }
            pre_match(&mut row);
            line_up_row(&row, &mut records, &data);
            let mut ours = original.clone();
            kick_off_row(&mut row, &mut records, &data, &mut ours);
            for d in &draws[first..six] {
                original.below(d.1);
            }
            if ours == original {
                fifths += 1;
            } else if whole {
                eprintln!("day {day}: the fifth loop parts from the original's");
            }
            for (k, m) in row.iter_mut().enumerate() {
                let (human, n, picked, span) = theirs[k];
                // the listing's mark, which the day's list draws before
                // the walk, as the original's picks show it
                m.doping = picked;
                let mut ours = original.clone();
                ready_row(std::slice::from_mut(m), &mut records, &mut ours);
                for d in span {
                    assert_eq!(original.below(d.1), d.2);
                }
                if human {
                    // the human's match, the national branch, waits
                    continue;
                }
                let throw = span[span.len() - 1];
                matches += 1;
                if ours == original && throw.1 == 3 && m.ends_at == throw.2 as i16 - 2 + 45 {
                    sixths += 1;
                } else {
                    eprintln!("day {day} match {k} {:?}: the original throws {n} for its men, then {throw:?}", m.ids);
                }
            }
        }
        eprintln!("{days} days, {fifths} of {wholes} fifth loops agree, {sixths} of {matches} matches in the sixth, {pressed} press throws");
        assert!(days >= 8, "only {days} league days found in the trace");
        assert!(fifths + 1 >= wholes, "{fifths} of {wholes} fifth loops leave the original's generator");
        assert_eq!(sixths, matches, "the sixth loop's throws");
    }

    /// The original's sixth loop from its first throw, match by match:
    /// whether a human's club's side drew for its national branch
    /// (`FUN_006fc8c6`, 0x6fc8c6..) and ran in bulk (`FUN_0070c334`),
    /// the throws for the listed men, whether the doping control picked
    /// (0x5dac94..0x5dae5f), and the match's draws up to its added time
    /// (`FUN_0040bc1b`).
    fn sixth_loop(draws: &[(u32, u32, u32, u32)], six: usize) -> Vec<(bool, usize, bool, std::ops::Range<usize>)> {
        let man = |c: u32| (0x658cfb..0x658fb2).contains(&c);
        let added = |c: u32| (0x40bc1b..0x40bc96).contains(&c);
        let mut theirs = Vec::new();
        let mut i = six;
        loop {
            let from = i;
            let national = draws[i..].iter().take_while(|d| (0x6fc8c6..0x6fc9dc).contains(&d.0)).count();
            i += national;
            let n = draws[i..].iter().take_while(|d| man(d.0)).count();
            i += n;
            let picks = draws[i..].iter().take_while(|d| (0x5dac94..0x5dae5f).contains(&d.0)).count();
            i += picks;
            let bulk = if national > 0 { draws[i..].iter().take_while(|d| !added(d.0)).count() } else { 0 };
            i += bulk;
            if i >= draws.len() || !added(draws[i].0) {
                return theirs;
            }
            i += 1;
            theirs.push((national > 0, n, picks > 0, from..i));
        }
    }

    /// The walk's last step against the original's trace: on every traced
    /// league day whose row holds only the listed leagues, from the
    /// original's state once its sixth loop is through (the human's
    /// match, bulk-run in the national branch, stepped over on the
    /// original's throws and marked at +0x474), the celebrity's tip
    /// (`FUN_004cbf59`, 0x4cbf59..) throws as the original's — two or
    /// three for the celebrity, ninety for each marked match — and leaves
    /// the generator where the minutes' first throw (`FUN_006f6857`, or
    /// a marked match's open play first) finds it. How far the replica's minutes then follow and
    /// how many of the next dump's league results they meet is printed:
    /// the shot's range (`FUN_0040b8c9`, the attack's rated +0x16 a few
    /// hundredths low), the sides' class sum (+0x33, one apart on some
    /// sides) and the human's match, which the watched walk plays through
    /// the national path (`FUN_006f562b`'s `FUN_006f529d`, 0x6f5308), part
    /// them still.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR and the oracle's season run from $ANSTOSS3_ORACLE"]
    fn the_walk_ends_where_the_originals_minutes_start() {
        use a3_files::an3::Save;
        let Some((data, draws, traced)) = traced_days() else { return };
        let season = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_ORACLE").unwrap()).join("runs/season");
        let kick = |c: u32| (0x6f6fc7..0x6f791a).contains(&c);
        let tactic = |above: u32| above == 0x6f007e || above == 0x6f009d;
        let press = |c: u32| (0x6f9656..0x6f9788).contains(&c);
        let fifth = |d: &(u32, u32, u32, u32)| kick(d.0) || tactic(d.3) || press(d.0);
        let (mut days, mut tipped, mut followed, mut matches, mut agreed) = (0, 0, 0, 0, 0);
        for TracedDay { day, start, end, state, records, row, slots } in traced {
            let Some(first) = (start..end).find(|&i| fifth(&draws[i])) else { continue };
            let mut original = Random::from_state(state.0, state.1);
            if !draws[start..first].iter().all(|d| original.below(d.1) == d.2) {
                continue;
            }
            let six = first + draws[first..].iter().take_while(|d| fifth(d)).count();
            let theirs = sixth_loop(&draws, six);
            if theirs.len() != row.len() {
                continue;
            }
            let (mut records, mut row) = (records, row);
            for m in row.iter_mut() {
                m.home_nation = m.ids[0] as u8;
                m.home_id = m.ids[1];
            }
            pre_match(&mut row);
            crowd_row(&mut row, &records, &data);
            line_up_row(&row, &mut records, &data);
            let mut ours = original.clone();
            kick_off_row(&mut row, &mut records, &data, &mut ours);
            for d in &draws[first..six] {
                original.below(d.1);
            }
            if ours != original {
                eprintln!("day {day}: the fifth loop parts from the original's");
                continue;
            }
            for (m, (human, _, picked, span)) in row.iter_mut().zip(&theirs) {
                m.doping = *picked;
                let mut ours = original.clone();
                ready_row(std::slice::from_mut(m), &mut records, &mut ours);
                for d in &draws[span.clone()] {
                    original.below(d.1);
                }
                if *human {
                    // run in bulk by the national branch: the minutes pass it
                    m.nat_bits |= 1;
                }
            }
            let walk = theirs.last().map_or(six, |t| t.3.end);
            days += 1;
            // the tip, by the generator's state, then the minutes' first
            // throw: the chance's side (`FUN_006f6857`) or, a marked match
            // first, its open play (`FUN_006f529d`..)
            let tip = draws[walk..].iter().take_while(|d| (0x4cbf59..0x4cc22e).contains(&d.0)).count();
            let mut ours = original.clone();
            let marked = row.iter().filter(|m| m.nat_bits != 0).count();
            let drawn = celebrity_tip(&row, &records, &mut ours);
            for d in &draws[walk..walk + tip] {
                original.below(d.1);
            }
            let minutes = walk + tip;
            if ours == original && (0x6f529d..0x6f6af7).contains(&draws[minutes].0) && drawn.tips.len() == marked {
                tipped += 1;
            } else {
                eprintln!("day {day}: the tip throws {tip}, then {:#x}", draws[minutes].0);
                continue;
            }
            let today = records.globals.date_of(records.globals.day() as i16).unwrap();
            crate::random::record(true);
            play_matches(&mut row, false, Some(&mut records), None, today, &mut ours);
            let mine = crate::random::recorded();
            crate::random::record(false);
            let part = mine.iter().zip(&draws[minutes..]).position(|(a, d)| (a.range, a.result) != (d.1, d.2));
            match part {
                None => followed += 1,
                Some(k) => {
                    let (d, a) = (draws[minutes + k], mine[k]);
                    eprintln!(
                        "day {day}: the minutes part at throw {k} of {}: theirs {:#x} below {} {}, ours below {} {} at {}:{}",
                        mine.len(), d.0, d.1, d.2, a.range, a.result, a.at.file(), a.at.line()
                    );
                }
            }
            let next = Records::from_save(Save::read(&season.join(format!("day-{:03}.an3", day + 1))).unwrap()).unwrap();
            for ((m, (human, ..)), (name, at)) in row.iter().zip(&theirs).zip(&slots) {
                if *human {
                    continue;
                }
                matches += 1;
                let written = next.block(name).unwrap()[*at..*at + 4].to_vec();
                agreed += usize::from(written == [m.goals[0] as u8, m.goals[1] as u8, m.half[0], m.half[1]]);
            }
        }
        eprintln!("{days} days, {tipped} tips as the original's; {followed} follow its minutes, {agreed} of {matches} results agree");
        assert!(days >= 8, "only {days} league days found in the trace");
        assert_eq!(tipped, days, "the celebrity's tip");
    }
}
