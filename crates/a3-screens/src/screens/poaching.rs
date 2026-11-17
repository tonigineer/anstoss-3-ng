//! A poaching talk on its day (dialog 319, `CVerhandlungAbwerbeversuch`,
//! `IDD_VERTRAGSVERLAENGERUNG_SPIELER_STANDARD`): another club's player and
//! the manager's club, his contract as it stands and the manager's offer
//! beside it, which starts as the same terms (`FUN_0076ad3b`). The offer's
//! spins and toggles change it; "Verhandeln" comes with the negotiation —
//! as does the fill disabling "ab sofort" (6004) when he may not come at
//! once (L+0x98 0).

use a3_core::cycle::dialogs::POACHING_TALK;
use a3_core::transfers::TERMS;

use super::popups::NOTICE;
use super::{Args, Cx, Outcome, Reply, Request, Screen, control, mark_inert, set};
use crate::controls::ControlState;
use crate::controls::bitmap::Picture;
use crate::pointer::Target;
use crate::view::View;

/// The header (`0x1037`), the club, its league, the crest, the label the
/// dialog writes "Laufzeit" into (`0x102b`).
const HEADER: u16 = 4151;
const CLUB: u16 = 4149;
const LEAGUE: u16 = 4150;
const CREST: u16 = 4125;
const YEARS_LABEL: u16 = 4139;
/// "Abbrechen" (`OnClicked_IDC_VVERL_STD_ABBRECHEN`).
const CANCEL: u16 = 4153;
/// The offer's spins (`OnVScroll_005b8fb4`): the signing fee, salary,
/// appearance and goal bonus, release clause, with the step tables
/// (`DAT_0088ca10` …) and maxima (`DAT_008913a8` …) they run on; and the
/// years, 1 to 10.
const FEE_SPIN: u16 = 6008;
const SALARY_SPIN: u16 = 4133;
const APPEARANCE_SPIN: u16 = 4134;
const GOAL_SPIN: u16 = 4135;
const CLAUSE_SPIN: u16 = 4137;
const YEARS_SPIN: u16 = 4142;
const MONEY_STEPS: [(i32, i32); 8] =
    [(50, 1), (100, 2), (300, 5), (1000, 10), (3000, 20), (6000, 50), (10000, 100), (-1, 200)];
const BONUS_STEPS: [(i32, i32); 4] = [(10, 1), (100, 5), (400, 10), (-1, 50)];
const CLAUSE_STEPS: [(i32, i32); 8] =
    [(50, 1), (100, 2), (500, 5), (1000, 10), (5000, 50), (10000, 100), (50000, 500), (-1, 1000)];
/// Each money spin: its id, its table, its most, where in the terms.
const MONEY_SPINS: [(u16, &[(i32, i32)], i32, usize); 5] = [
    (FEE_SPIN, &MONEY_STEPS, 25000, 8),
    (SALARY_SPIN, &MONEY_STEPS, 20000, 0),
    (APPEARANCE_SPIN, &BONUS_STEPS, 2000, 2),
    (GOAL_SPIN, &BONUS_STEPS, 2000, 4),
    (CLAUSE_SPIN, &CLAUSE_STEPS, 20000, 6),
];
/// The offer's toggles and the flag each flips (`OnClicked_IDC_VVERL_STD_
/// AN_STAMM`, `_AN_OPTVEREIN`, `_OPTSPIELER`, `_AN_ERGAENZUNG`, `_AN_20`);
/// "ab wann" (6004) and the league (6010) have their own rules.
const TOGGLES: [(u16, u16); 5] =
    [(4144, REGULAR), (4147, CLUB_OPTION), (4148, PLAYER_OPTION), (6013, SQUAD_PLAYER), (6014, TWENTY_GAMES)];
const WHEN: u16 = 6004;
const LEAGUE_TOGGLE: u16 = 6010;
/// "ab sofort" refused: too late in the season, past day 0xd8 (`0x10d0`),
/// else not before day 0xd2 (`0x10cf`).
const TOO_LATE: u16 = 0x10d0;
const NOT_AT_ONCE: u16 = 0x10cf;
const LAST_DAY: u16 = 0xd8;
/// Each column's thirteen controls in the order the fill walks them
/// (`DAT_0088cba0` the contract, `DAT_0088cbd4` the offer).
const NOW: [u16; 13] = [4126, 4127, 4128, 4136, 4140, 4143, 4145, 4146, 6005, 6006, 6009, 6011, 6012];
const OFFER: [u16; 13] = [4130, 4131, 4132, 4138, 4141, 4144, 4147, 4148, 6004, 6007, 6010, 6013, 6014];

/// Texts of the game.
const HEADER_TEXT: u16 = 3221;
const YES: u16 = 1028;
const NO: u16 = 1029;
const AT_ONCE: u16 = 1030;
const NEXT_SEASON: u16 = 1031;
const LEAGUES: u16 = 4086;
const RUNS: u16 = 2434;
const YEARS: u16 = 1081;
const MANY_YEARS: u16 = 1587;
const NO_CONTRACT: u16 = 1591;
const ONE_YEAR: u16 = 1592;
const EXPIRING: u16 = 1593;
const YEARS_LEFT: u16 = 1594;
const UNTIL: u16 = 1596;
const NEXT_SEASON_END: u16 = 3266;
/// The positions' names by main position (`DAT_008a8490`, `FUN_007312e4`).
const POSITIONS: [u16; 6] = [1090, 1134, 1135, 1138, 1136, 1137];

/// The terms' units (`DAT_00a3b3e4`, `DAT_00a3b3fc`, `DAT_00a3b3bc`).
const THOUSAND: i64 = 1000;
const HUNDRED: i64 = 100;
const RELEASE_UNIT: i64 = 50_000;
/// The terms' flags (+0xe).
const REGULAR: u16 = 0x4000;
const CLUB_OPTION: u16 = 0x2000;
const PLAYER_OPTION: u16 = 0x1000;
const AT_ONCE_FLAG: u16 = 0x20;
const SQUAD_PLAYER: u16 = 0x8000;
const TWENTY_GAMES: u16 = 0x80;
/// Before this day of the season a player may still come at once
/// (`FUN_00758296`).
const LAST_DAY_AT_ONCE: i16 = 0xd2;

#[derive(Default)]
pub struct PoachingTalk {
    /// The manager's offer as the player changes it (+0x1dc0).
    pub offer: [u8; TERMS],
    /// The manager's club's league (+0x1db8 → +0x105).
    level: u8,
}

impl Screen for PoachingTalk {
    fn ids(&self) -> &[u16] {
        &[POACHING_TALK]
    }

    /// `FUN_0076ad3b` and the dialog's fill `FUN_005b7add`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let Args::Record(id) = args else { return };
        let (game, dialog) = (cx.game, cx.dialog);
        let works = |id: u16| id == CANCEL || id == WHEN || id == LEAGUE_TOGGLE || TOGGLES.iter().any(|t| t.0 == id);
        mark_inert(view, dialog, |t| matches!(t, Target::Button(c) if dialog.controls[c].id.is_some_and(works)));
        let (Some(records), Some(season)) = (cx.state.records.as_ref(), cx.state.season.as_ref()) else {
            return;
        };
        let now = a3_core::transfers::terms(records, id);
        let offer = offer(now, records.globals.day() as i16);
        self.offer = offer;
        // The manager's club: its name, league and crest.
        let club = records
            .human_slot(season.current)
            .and_then(|slot| records.managers.get(slot))
            .map(|m| (m.country() as u8, m.team() as i16));
        if let Some((country, number)) = club {
            set(view, dialog, CLUB, game.club_name(Some(records), country, number));
            if let Some(team) = records.team(country.into(), number as u16) {
                self.level = team.level();
                let place = super::search::league_place(records, team);
                set(view, dialog, LEAGUE, records.countries.league_name(country.into(), place));
            }
            let crest = game.world.slot_of(country.into()).and_then(|slot| {
                game.rules.crest(&game.world, Some(records), (slot, usize::try_from(number).ok()?))
            });
            if let Some(c) = control(dialog, CREST) {
                view.set_picture(c, crest.map(Picture::file));
            }
        }
        // Both columns (`FUN_005b8492` → `FUN_005b84d5`).
        fill_column(cx, view, &NOW, &now);
        fill_column(cx, view, &OFFER, &offer);
        // The spins on the offer (`FUN_005b7add`, `FUN_005c690b`).
        for (id, steps, most, at) in MONEY_SPINS {
            let v = if at == 8 {
                u32::from_le_bytes([offer[8], offer[9], offer[10], offer[11]]) as i32
            } else {
                i32::from(u16::from_le_bytes([offer[at], offer[at + 1]]))
            };
            if let Some(spin) = control(dialog, id).and_then(|c| view.value_mut(c)) {
                spin.range = (0, step_of(steps, most));
                spin.set(step_of(steps, v));
            }
        }
        if let Some(spin) = control(dialog, YEARS_SPIN).and_then(|c| view.value_mut(c)) {
            spin.range = (1, 10);
            spin.set(i32::from((offer[12] as i8).max(1)));
        }
        set(view, dialog, YEARS_LABEL, game.text(RUNS).to_owned());
        // "Name, F., contract, Alter: n, Stärke: n, Hpt: position".
        let Some((player, _, _)) = a3_core::load::any_player(records, &game.data, id) else { return };
        let picks = records.player(id).map_or(0, |r| r.picks());
        let strength =
            if matches!((picks & 0xc0) >> 6, 1 | 3) { "?".to_owned() } else { player.strength.to_string() };
        let today = season.date();
        let position = POSITIONS.get(usize::from(player.positions[0])).map_or("", |&p| game.text(p));
        let header = super::finances::printf(
            game.text(HEADER_TEXT),
            &[
                &super::duties::short_name(cx, &player, id),
                &contract_text(cx, now[12] as i8, today),
                &player.age(today),
                &strength,
                &position,
            ],
        );
        set(view, dialog, HEADER, header);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _double: bool) -> Outcome {
        let id = |c: usize| cx.dialog.controls[c].id.unwrap_or(0);
        let flags = u16::from_le_bytes([self.offer[14], self.offer[15]]);
        let put = |offer: &mut [u8; TERMS], f: u16| offer[14..16].copy_from_slice(&f.to_le_bytes());
        match target {
            Target::Button(c) if id(c) == CANCEL => return Outcome::Close(Reply::Cancel),
            // `FUN_005b90a3` … `FUN_005b926e`: the spin's position to its
            // amount (`FUN_005c69c9`).
            Target::Spin { control: c, .. } => {
                let Some(p) = view.value(c).map(|v| v.value) else { return Outcome::Stay };
                if id(c) == YEARS_SPIN {
                    self.offer[12] = p as u8;
                } else if let Some(&(_, steps, _, at)) = MONEY_SPINS.iter().find(|s| s.0 == id(c)) {
                    let v = amount_at(steps, p);
                    if at == 8 {
                        self.offer[8..12].copy_from_slice(&((v as u32) & 0xffff).to_le_bytes());
                    } else {
                        self.offer[at..at + 2].copy_from_slice(&(v as u16).to_le_bytes());
                    }
                } else {
                    return Outcome::Stay;
                }
            }
            Target::Button(c) | Target::Check(c) | Target::Control(c) => {
                let id = id(c);
                if let Some(&(_, flag)) = TOGGLES.iter().find(|t| t.0 == id) {
                    put(&mut self.offer, flags ^ flag);
                } else if id == WHEN {
                    // `OnClicked_IDC_VVERL_STD_AN_ABWANN` (+0x1dbc 0 in a
                    // poaching talk).
                    let day = cx.state.records.as_ref().map_or(0, |r| r.globals.day());
                    if day > LAST_DAY {
                        return Outcome::Popup(Request::with(NOTICE, Args::Text(TOO_LATE)));
                    }
                    if day >= LAST_DAY_AT_ONCE as u16 {
                        return Outcome::Popup(Request::with(NOTICE, Args::Text(NOT_AT_ONCE)));
                    }
                    put(&mut self.offer, flags ^ AT_ONCE_FLAG);
                } else if id == LEAGUE_TOGGLE {
                    // `OnClicked_IDC_VVERL_STD_AN_LIGA`: none, or the club's
                    // league (at most the third) and above.
                    let league = if flags & 3 == 0 { u16::from(self.level.min(2)) + 1 } else { 0 };
                    put(&mut self.offer, flags & !3 | league);
                } else {
                    return Outcome::Stay;
                }
            }
            _ => return Outcome::Stay,
        }
        let offer = self.offer;
        fill_column(cx, view, &OFFER, &offer);
        Outcome::Stay
    }
}

/// A column's texts and checks from its terms (`FUN_005b84d5`).
fn fill_column(cx: &Cx, view: &mut View, ids: &[u16; 13], t: &[u8; TERMS]) {
    let (game, dialog) = (cx.game, cx.dialog);
    let yes_no = |on: bool| game.text(if on { YES } else { NO }).to_owned();
    let word = |at: usize| i64::from(u16::from_le_bytes([t[at], t[at + 1]]));
    let f = u16::from_le_bytes([t[14], t[15]]);
    let money = |v: i64| super::playerinfo::money(game, v);
    let texts = [
        money(word(0) * THOUSAND),
        money(word(2) * HUNDRED),
        money(word(4) * HUNDRED),
        if word(6) == 0 { game.text(NO).to_owned() } else { money(word(6) * RELEASE_UNIT) },
        years_text(cx, t[12] as i8),
        yes_no(f & REGULAR != 0),
        yes_no(f & CLUB_OPTION != 0),
        yes_no(f & PLAYER_OPTION != 0),
        game.text(if f & AT_ONCE_FLAG != 0 { AT_ONCE } else { NEXT_SEASON }).to_owned(),
        money(i64::from(u32::from_le_bytes([t[8], t[9], t[10], t[11]])) * THOUSAND),
        game.text(LEAGUES + (f & 3)).to_owned(),
    ];
    for (&id, text) in ids.iter().zip(texts) {
        set(view, dialog, id, text);
    }
    for (&id, on) in ids[11..].iter().zip([f & SQUAD_PLAYER != 0, f & TWENTY_GAMES != 0]) {
        if let Some(ControlState::Check(check)) = control(dialog, id).map(|c| &mut view.controls[c].state) {
            check.checked = on;
        }
    }
}

/// A position on a spin's steps, rounded down (`FUN_005c690b`); the
/// table ends at −1 with the step beyond.
fn step_of(steps: &[(i32, i32)], v: i32) -> i32 {
    let (mut position, mut from) = (0, 0);
    for &(to, step) in steps {
        if to == -1 || v <= to {
            return (v - from) / step + position;
        }
        position += (to - from) / step;
        from = to;
    }
    position
}

/// The amount at a spin's position (`FUN_005c69c9`).
fn amount_at(steps: &[(i32, i32)], mut p: i32) -> i32 {
    let mut from = 0;
    for &(to, step) in steps {
        if to == -1 {
            return p * step + from;
        }
        let n = (to - from) / step;
        if p <= n {
            return p * step + from;
        }
        from = to;
        p -= n;
    }
    from
}

/// The manager's offer as the talk opens (`FUN_0076ad3b`): his contract's
/// terms without the signing fee, "ab sofort" while the season is young
/// enough and he may come at once at all (L+0x98, `FUN_007588bc`).
pub fn offer(now: [u8; TERMS], day: i16) -> [u8; TERMS] {
    let mut t = now;
    let mut flags = u16::from_le_bytes([t[14], t[15]]);
    if day < LAST_DAY_AT_ONCE {
        flags |= AT_ONCE_FLAG;
    }
    if t[16] == 0 {
        flags &= !AT_ONCE_FLAG;
    }
    t[14..16].copy_from_slice(&flags.to_le_bytes());
    t[8..12].fill(0);
    t
}

/// A term in years: "ein Jahr" … "zehn Jahre", else "%d Jahre"
/// (`FUN_00730bdc`).
fn years_text(cx: &Cx, years: i8) -> String {
    match years {
        1..=10 => cx.game.text(YEARS + years as u16 - 1).to_owned(),
        _ => super::finances::printf(cx.game.text(MANY_YEARS), &[&years]),
    }
}

/// How long his contract runs (`FUN_00732b5c`): none, expiring before
/// July, the years left; and then the season it ends.
fn contract_text(cx: &Cx, years: i8, today: chrono::NaiveDate) -> String {
    use chrono::Datelike;
    let game = cx.game;
    let spring = today.month() < 7;
    let mut text = match years {
        ..=0 => return game.text(NO_CONTRACT).to_owned(),
        1 => game.text(if spring { EXPIRING } else { ONE_YEAR }).to_owned(),
        2 if spring => game.text(NEXT_SEASON_END).to_owned(),
        _ => super::finances::printf(game.text(YEARS_LEFT), &[&years]),
    };
    let start = cx.state.records.as_ref().map_or(0, |r| i32::from(r.globals.start_year()));
    text.push(' ');
    text.push_str(&super::finances::printf(game.text(UNTIL), &[&(start + i32::from(years))]));
    text
}
