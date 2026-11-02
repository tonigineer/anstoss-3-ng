//! The original's match engine: the simulation behind a match, ported from
//! the decompile. It walks the day's matches minute by minute — the root is
//! `FUN_006f352e`, each minute runs `FUN_006f2d4f`, and `FUN_006f4744`
//! recomputes both sides' rated strengths from their sides' tactics, mood
//! and fatigue every minute. The match itself is a 0x498-byte record; its
//! two sides' blocks sit at +0x5c and +0x250, and this module's `Match` and
//! `Side` name the record's fields, keeping the original's offsets in the
//! comments.
//!
//! This is the engine's skeleton: the minute walk, the added time, full
//! time's extra-time and penalty decisions, and the rated strengths. The
//! chances that make the minutes produce goals are the next pieces — set
//! pieces (`FUN_006f543f`), open play (`FUN_006f562b`), the international
//! mode (`FUN_006f6007`), the trailing side's morale rolls (`FUN_0041321d`)
//! and the penalty shootout (`FUN_005ddea7`) are called from the same
//! places the original calls them and marked where they fall in. The
//! original's presentation (the 3D match, its commentary and clock
//! displays: `FUN_0073f1a6` and the `DAT_01bad050`/`01bad060` displays) has
//! no replica and is left out.
//!
//! Not wired into `matches::play_day` yet: until the chances land, this
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
    /// The two sides, +0x5c and +0x250.
    pub sides: [Side; 2],
    /// +0x444: what side one leads by; its sign decides which side's mood
    /// sinks as the minutes of a lost half run on.
    pub advantage: i16,
    /// +0x446: a percent on the leading side's attack and hold, see
    /// `minute_play`.
    pub form: i16,
    /// +0x448: the minute this match's half ends at; the walk's threshold,
    /// floored at 45, 90 and 120 by the half.
    pub ends_at: i16,
    /// +0x451: byte 6 down by two places selects the added time's draw.
    pub added_draw: u8,
    /// +0x474: the live marker — the match the viewer watches; zero for a
    /// match only scored. The original draws its live match's minutes out
    /// of its own clock and lets the viewer's club adjust the sides;
    /// everywhere below a match with this set is passed over unless the
    /// whole day runs live.
    pub live: u16,
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
/// every `Match::live` zero.
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

/// One minute of the day's matches, `FUN_006f2d4f` without its
/// presentation; true while some open match still runs before its end of
/// half. The display clocks (+0x1c, +0x0c) are left out.
fn tick(matches: &mut [Match], live: bool, minute: i16, clock: &mut Clock, random: &mut Random) -> bool {
    if clock.extra == 0 && clock.half2 == 0 && minute < 0x78 {
        clock.minute = minute;
        let mut run = false;
        for m in matches.iter_mut() {
            if m.flags & FINISHED == 0 && (live || m.live == 0) {
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
            if live || m.live == 0 {
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
            if (live || m.live == 0) && m.flags & FINISHED == 0 && m.flags & EXTRA != 0 {
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
            if (live || m.live == 0) && m.flags & FINISHED == 0 && m.flags & PENS != 0 {
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
fn minute_play(m: &mut Match, clock: &Clock, _random: &mut Random) {
    if m.flags & (MINUTE_UP | OVER) != 0 {
        return;
    }
    // The viewer's club adjusts its side as the match runs (`FUN_006fb8f5`
    // with the live marker's two three-bit fields) — not ported.
    if m.live & 1 != 0 {
        live_adjust(&mut m.sides[0], (m.live as i16 >> 8) & 7);
    }
    if m.live & 2 != 0 {
        live_adjust(&mut m.sides[1], (m.live as i16 >> 12) & 7);
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
    // The chances: two national sides play their simple minute
    // (`FUN_006f6007`, `FUN_0041321d`); a club tie runs through the clubs'
    // match-day state (`FUN_00658b30`), their set pieces (`FUN_006f543f`)
    // and, when those come to nothing, open play (`FUN_006f562b`) — none
    // of these drawn from yet.
    if is_national(m.ids[0]) && is_national(m.ids[2]) {
        // FUN_006f6007(m, clock); FUN_0041321d(m, clock);
    } else {
        // FUN_00658b30(club, side, clock) twice, FUN_006fbc5e(m),
        // set pieces by FUN_006f543f(m, club, club, clock),
        // FUN_0041321d(m, clock) — and FUN_006f562b(m, clock) open play.
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

/// What the viewer's club does to its side mid-minute (`FUN_006fb8f5`) —
/// not ported; it only ever runs for a live match.
fn live_adjust(_side: &mut Side, _what: i16) {}

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
        if live || m.live == 0 {
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
        if (live || m.live == 0) && m.flags & OVER == 0 {
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
    /// nothing (index 4 sits at 100 %).
    fn fixture() -> Match {
        let side = Side {
            strengths: [100, 100, 100, 100, 100],
            strength: 100,
            stance: 4,
            tilt: 3,
            ..Side::default()
        };
        Match {
            sides: [side, side],
            ends_at: 0,
            ..Match::default()
        }
    }

    #[test]
    fn a_goalless_afternoon_ends_by_the_book() {
        let mut m = fixture();
        let mut r = Random::new(12345);
        let added = added_time(&m, &mut r.clone()) + 90;
        play_matches(std::slice::from_mut(&mut m), false, &mut r);
        assert_eq!(m.ends_at, added);
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
        b.added_draw = 2 << 2;
        let mut ms = [b, a];
        let mut r = Random::new(99);
        let first = added_time(&ms[0], &mut r);
        let second = added_time(&ms[1], &mut r);
        let mut r = Random::new(99);
        play_matches(&mut ms, false, &mut r);
        assert_eq!(ms[0].ends_at, 90 + first);
        assert_eq!(ms[1].ends_at, 90 + second);
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
        assert_eq!(m.ends_at, 120 + {
            let mut r = Random::new(5);
            let _half = added_time(&m, &mut r);
            added_time(&m, &mut r)
        });
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
}
