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
//! side's morale talk (`FUN_0041321d`). The chances land as seams — the
//! events that turn a chance into a goal or an event (`FUN_00659945` and
//! its eleven siblings), the national-side path's set pieces
//! (`FUN_006f543f`) and open play (`FUN_006f562b`), the talk's staff
//! question (`FUN_004134d0`) and the penalty shootout (`FUN_005ddea7`) are
//! the next pieces, called from the places the original calls them. The
//! original's presentation (the 3D match, its commentary and clock
//! displays: `FUN_0073f1a6` and the `DAT_01bad050`/`01bad060` displays) has
//! no replica and is left out.
//!
//! Not wired into `matches::play_day` yet: until the events land, this
//! would only play goalless games.

use crate::random::Random;

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

/// One side of a match: the record's side block at +0x5c (first side) or
/// +0x250 (second). The offsets in the comments are from the block's start.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Side {
    /// +0x00,+0x02,+0x04,+0x06,+0x08: the five rated strengths, in tenths.
    /// The engine names them only by these offsets; strength 4 feeds attack
    /// and strength 3 support, and 1 and 5 trade under a tilted tactic.
    pub strengths: [i16; 5],
    /// +0x0c: the mood of the minute, `minute_play` sinks it when the side
    /// trails through the half.
    pub morale: i16,
    /// +0x0e..+0x16: recomputed every minute from strengths, morale,
    /// stance and tactic; the chances read these.
    pub rated: [i16; 5],
    /// +0x1c: what the stance's percentage is taken of.
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
    /// +0x39 (+0x95/+0x289): the count of the side's plain chances.
    pub chances: u8,
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
    /// +0x444: what side one leads by; its sign decides which side's mood
    /// sinks as the minutes of a lost half run on.
    pub advantage: i16,
    /// +0x446: a percent on the leading side's attack and hold, see
    /// `minute_play`.
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
/// port.
pub fn play_matches(matches: &mut [Match], live: bool, random: &mut Random) {
    let mut clock = Clock::default();
    let mut minute = 1;
    while minute < 0x31 && tick(matches, live, minute, &mut clock, random) {
        minute += 1;
    }
    clock.minute = 45;
    // Half time also packs the half's events (FUN_006f85b7 for the live
    // match) and records the scorers so far (FUN_0064987a) — not ported.
    for m in matches.iter_mut() {
        m.ends_at = added_time(m, random) + 90;
        m.flags &= !MINUTE_UP;
    }
    clock.half2 = 1;
    let mut minute = 46;
    while minute < 99 && tick(matches, live, minute, &mut clock, random) {
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
        while minute < 0x7e && tick(matches, live, minute, &mut clock, random) {
            minute += 1;
        }
        if after_extra_time(matches, live) {
            clock.extra = 2;
            tick(matches, live, 0x78, &mut clock, random);
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
/// national call-ups, the squads summed into the sides by `FUN_0078b26c`
/// and their totals at +0x50/+0x54 settling the favourite byte at +0x58,
/// and the referee pair `FUN_006cdbb9`/`FUN_004244da` unpacked by
/// `FUN_0063161e` into +0x44a..+0x458 for a national tie — works on the
/// clubs and their squads and waits there.
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

/// One minute of the day's matches, `FUN_006f2d4f` without its
/// presentation; true while some open match still runs before its end of
/// half. The display clocks (+0x1c, +0x0c) are left out.
fn tick(matches: &mut [Match], live: bool, minute: i16, clock: &mut Clock, random: &mut Random) -> bool {
    if clock.extra == 0 && clock.half2 == 0 && minute < 0x78 {
        clock.minute = minute;
        let mut run = false;
        for m in matches.iter_mut() {
            if m.flags & FINISHED == 0 && (live || m.nat_bits == 0) {
                if minute == 10 {
                    m.score10 = [m.goals[0] as u8, m.goals[1] as u8];
                }
                minute_play(m, clock, random);
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
                    minute_play(m, clock, random);
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
                minute_play(m, clock, random);
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
                penalties(m, clock, random);
            }
        }
        false
    }
}

/// One match's minute, `FUN_006f4744`: the moods sink on a lost cause,
/// both sides' rated strengths come out of their strengths, stance, tactic,
/// mood and the foe's form, fatigue accumulates — and what the minute is
/// worth in chances follows.
fn minute_play(m: &mut Match, clock: &Clock, random: &mut Random) {
    if m.flags & (MINUTE_UP | OVER) != 0 {
        return;
    }
    // A national side's own tactic, which the association's table sets
    // for the minute from the side's place in the list — not ported.
    if m.nat_bits & 1 != 0 {
        national_tactic(&mut m.sides[0], (m.nat_bits as i16 >> 8) & 7);
    }
    if m.nat_bits & 2 != 0 {
        national_tactic(&mut m.sides[1], (m.nat_bits as i16 >> 12) & 7);
    }
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
        chance_minute(m, clock, random);
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

/// The association's national-side table (`FUN_006fb8f5`) overwriting a
/// national side's tactic byte at +0x30 with the entry at its list place
/// less one, from the exe table at DAT_01bdd6d8 — not ported; it only
/// ever runs for a tie carrying that side.
fn national_tactic(_side: &mut Side, _place: i16) {}

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

/// The minute's chance frame, `FUN_006f6007`: what the chooser picked runs
/// as an event through the match's club (`FUN_0065b0b5`, `FUN_0065bd27` —
/// seam); then both sides' chance comes out of their rated support and
/// attack, scaled to a common 92 together, the ground's owner lifting the
/// first side's attack an eighth. A roll under the side's share opens the
/// chance — its shape by the minute kind and the shot's quality — and the
/// frame closes with the bookings the last minute left.
fn chance_minute(m: &mut Match, clock: &Clock, random: &mut Random) {
    // The chooser's set moment becomes an event at the match's club:
    // FUN_0065b0b5 when it picked the first side, FUN_0065bd27 the second
    // (each through the club slot FUN_0063dd1c) — the events piece.
    let _ = chance_chooser(m, random);
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
    chance_for(m, 0, attack, random);
    chance_for(m, 1, defend, random);
    // What the last minute's booking left (`FUN_00413115`): the match-day
    // state takes it (`FUN_00412cf2` per side, its bit +0x80 saying
    // red), then both marks clear; `FUN_0070c1a9` keeps the live clocks —
    // not ported.
    m.cards = [0, 0];
    let _ = clock;
}

/// One side's half of the frame: a thousand-wide roll against its share;
/// past it (or past fifteen goals for the side) no chance, though the
/// roll's own marks 699 and 900..903 set events off (seams). Inside, the
/// minute kind under 90 is a plain chance — the quality-one shot through
/// `FUN_00659d2b` counts and every imperfect shot counts — 90..97 an
/// `FUN_00659b85` shape, and from 98 the full `FUN_00659945`.
fn chance_for(m: &mut Match, which: usize, weight: f32, random: &mut Random) {
    let roll = random.below(1000);
    if weight <= roll as f32 || 15 < m.goals[which] {
        // roll 699: FUN_0065b894; 899 < roll < 904: FUN_0065cd8a (side
        // one) / FUN_0065bf36, FUN_0065ce98 (side two) — event seams.
        return;
    }
    let kind = random.below(100);
    let quality = shot_quality(m, which, random);
    if kind < 90 {
        if quality == 1 {
            // FUN_00659d2b — the shot on goal: the events piece.
            m.sides[which].chances += 1;
        } else if quality < 4 {
            m.sides[which].chances += 1;
        }
    } else if kind < 98 {
        // FUN_00659b85(m, quality, clock) — seam.
    } else {
        // FUN_00659945(m, clock) — seam.
    }
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

/// The shootout of one match (`FUN_005ddea7`) — the next piece.
fn penalties(_m: &mut Match, _clock: &Clock, _random: &mut Random) {}

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

    #[test]
    fn a_goalless_afternoon_ends_by_the_book() {
        let mut m = fixture();
        let mut r = Random::new(12345);
        play_matches(std::slice::from_mut(&mut m), false, &mut r);
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
        play_matches(&mut ms, false, &mut r);
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
        play_matches(std::slice::from_mut(&mut m), false, &mut r);
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
        play_matches(std::slice::from_mut(&mut m), false, &mut r);
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
        tick(std::slice::from_mut(&mut m), false, 10, &mut clock, &mut r);
        assert_eq!(m.score10, [0, 0]);
        // a half-2 mark is taken whatever the match's own state
        clock = Clock { half2: 1, minute: 90, ..Clock::default() };
        tick(std::slice::from_mut(&mut m), false, 90, &mut clock, &mut r);
        assert_eq!(m.score90, [3, 2]);
    }

    #[test]
    fn strengths_follow_the_stance_and_tilt() {
        let mut m = fixture();
        m.sides[0].strengths = [100, 200, 300, 400, 500];
        let mut clock = Clock::default();
        let mut r = Random::new(1);
        minute_play(&mut m, &clock, &mut r);
        assert_eq!(m.sides[0].rated, [100, 300, 300, 400, 500]);
        m.sides[0].stance = 0; // 110 %
        m.sides[0].tilt = 0; // all the way forward
        minute_play(&mut m, &clock, &mut r);
        assert_eq!(m.sides[0].rated, [110, 451, 330, 440, 418]);
    }

    #[test]
    fn mood_and_form_work_the_attack_pair() {
        let mut m = fixture();
        m.sides[0].morale = 200;
        m.form = -20;
        let mut clock = Clock::default();
        let mut r = Random::new(1);
        minute_play(&mut m, &clock, &mut r);
        assert_eq!(m.sides[0].rated[3], 132); // (100 + 200/20) = 110, plus 20 % form
        assert_eq!(m.sides[0].rated[4], 120); // 100 plus the same 20 %
        assert_eq!(m.sides[1].rated[3], 100); // side two feels no negative form
        m.form = 20;
        minute_play(&mut m, &clock, &mut r);
        assert_eq!(m.sides[0].rated[3], 110); // and no positive form either
        assert_eq!(m.sides[1].rated[3], 120);
        // a mental of −3 cuts the attack pair back a further 3 %
        m.sides[1].mental = 4;
        minute_play(&mut m, &clock, &mut r);
        assert_eq!(m.sides[1].rated[3], 120 - 3);
        assert_eq!(m.sides[1].rated[4], 117); // its hold, form-lifted too
    }

    #[test]
    fn a_lost_half_sinks_the_mood() {
        let mut m = fixture();
        m.advantage = -50;
        let mut r = Random::new(1);
        minute_play(&mut m, &Clock { minute: 30, ..Clock::default() }, &mut r);
        assert_eq!(m.sides[0].morale, 0);
        minute_play(&mut m, &Clock { minute: 100, ..Clock::default() }, &mut r);
        assert_eq!(m.sides[0].morale, -15);
        m.flags = EXTRA;
        minute_play(&mut m, &Clock { minute: 100, ..Clock::default() }, &mut r);
        assert_eq!(m.sides[0].morale, -15 - 8); // (−50 · 15) / 90
        // and the other side when it lies behind
        let mut m = fixture();
        m.advantage = 50;
        minute_play(&mut m, &Clock { minute: 100, ..Clock::default() }, &mut r);
        assert_eq!(m.sides[1].morale, -15);
    }

    #[test]
    fn fatigue_accrues_until_the_half_runs_out() {
        let mut m = fixture();
        let mut r = Random::new(1);
        minute_play(&mut m, &Clock { minute: 45, ..Clock::default() }, &mut r);
        assert_eq!([m.sides[0].fatigue, m.sides[1].fatigue], [100, 100]);
        minute_play(&mut m, &Clock { minute: 46, ..Clock::default() }, &mut r);
        assert_eq!(m.sides[0].fatigue, 100);
        minute_play(
            &mut m,
            &Clock { minute: 90, half2: 1, ..Clock::default() },
            &mut r,
        );
        assert_eq!(m.sides[0].fatigue, 200);
        minute_play(&mut m, &Clock { minute: 91, half2: 1, ..Clock::default() }, &mut r);
        assert_eq!(m.sides[0].fatigue, 200);
        m.flags = EXTRA;
        minute_play(&mut m, &Clock { minute: 120, half2: 1, ..Clock::default() }, &mut r);
        assert_eq!(m.sides[0].fatigue, 300);
    }

    #[test]
    fn the_press_window_lifts_attack_and_support() {
        let mut m = fixture();
        m.sides[0].style = PRESS_BIT;
        m.sides[0].press_to = 30;
        m.sides[0].press_from = 10;
        let mut r = Random::new(1);
        minute_play(&mut m, &Clock { minute: 20, ..Clock::default() }, &mut r);
        assert_eq!(m.sides[0].rated[3], 120);
        assert_eq!(m.sides[0].rated[2], 120);
        assert_eq!(m.sides[0].pressed, 1);
        // past the window's end minute, no side presses
        let mut m2 = fixture();
        m2.sides[0].style = PRESS_BIT;
        m2.sides[0].press_to = 30;
        m2.sides[0].press_from = 10;
        minute_play(&mut m2, &Clock { minute: 40, ..Clock::default() }, &mut r);
        assert_eq!(m2.sides[0].rated[3], 100);
        assert_eq!(m2.sides[0].pressed, 0);
    }

    #[test]
    fn the_special_styles_trade_the_attacks() {
        let mut m = fixture();
        m.sides[0].style = 0x380;
        let mut r = Random::new(1);
        minute_play(&mut m, &Clock::default(), &mut r);
        assert_eq!(m.sides[0].rated[3], 80);
        assert_eq!(m.sides[1].rated[3], 85);
        let mut m = fixture();
        m.sides[1].style = 0x300;
        minute_play(&mut m, &Clock::default(), &mut r);
        assert_eq!(m.sides[0].rated[3], 120);
        assert_eq!(m.sides[1].rated[3], 115);
    }

    #[test]
    fn the_floors_hold() {
        let mut m = fixture();
        m.sides[0].strengths = [0, 1000, 0, 0, 0];
        m.sides[0].tilt = 0;
        let mut r = Random::new(1);
        minute_play(&mut m, &Clock::default(), &mut r);
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
        chance_minute(&mut m, &Clock::default(), &mut r);
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
            chance_minute(&mut m, &Clock { minute: 30, ..Clock::default() }, &mut r);
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
        chance_minute(&mut plain, &Clock::default(), &mut rp);
        assert_eq!(rp, expect, "without the ground the frame is five draws");
        let mut rh = Random::new(seed);
        chance_minute(&mut host, &Clock::default(), &mut rh);
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

    #[test]
    fn a_quiet_day_leaves_the_national_tie_to_its_sums() {
        // a tie carrying a national side is bulk-simulated at pre-match;
        // an unwatched day's minute walk passes it by
        let mut tie = fixture();
        tie.nat_bits = 3;
        let mut plain = fixture();
        let mut ms = [tie, plain];
        let mut r = Random::new(3);
        play_matches(&mut ms, false, &mut r);
        assert_eq!(ms[0].sides[0].fatigue, 0);
        assert!(ms[1].sides[0].fatigue > 0);
    }

    #[test]
    fn a_watched_day_runs_the_national_tie_by_the_minute() {
        let mut tie = fixture();
        tie.nat_bits = 1;
        let mut r = Random::new(3);
        play_matches(std::slice::from_mut(&mut tie), true, &mut r);
        assert!(tie.sides[0].fatigue > 0);
    }
}
