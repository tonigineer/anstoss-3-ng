//! A poaching talk on its day (dialog 319, `CVerhandlungAbwerbeversuch`,
//! `IDD_VERTRAGSVERLAENGERUNG_SPIELER_STANDARD`): another club's player and
//! the manager's club, his contract as it stands and the manager's offer
//! beside it, which starts as the same terms (`FUN_0076ad3b`). Only
//! "Abbrechen" works yet; the offer's controls and "Verhandeln" come with
//! the negotiation — among them the offer's "ab sofort" (6004), which the
//! fill disables when he may not come at once (L+0x98 0).

use a3_core::cycle::dialogs::POACHING_TALK;
use a3_core::transfers::TERMS;

use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
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
pub struct PoachingTalk;

impl Screen for PoachingTalk {
    fn ids(&self) -> &[u16] {
        &[POACHING_TALK]
    }

    /// `FUN_0076ad3b` and the dialog's fill `FUN_005b7add`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let Args::Record(id) = args else { return };
        let (game, dialog) = (cx.game, cx.dialog);
        mark_inert(view, dialog, |t| matches!(t, Target::Button(c) if dialog.controls[c].id == Some(CANCEL)));
        let (Some(records), Some(season)) = (cx.state.records.as_ref(), cx.state.season.as_ref()) else {
            return;
        };
        let now = a3_core::transfers::terms(records, id);
        let offer = offer(now, records.globals.day() as i16);
        let flags = |t: &[u8; TERMS]| u16::from_le_bytes([t[14], t[15]]);
        // The manager's club: its name, league and crest.
        let club = records
            .human_slot(season.current)
            .and_then(|slot| records.managers.get(slot))
            .map(|m| (m.country() as u8, m.team() as i16));
        if let Some((country, number)) = club {
            set(view, dialog, CLUB, game.club_name(Some(records), country, number));
            if let Some(team) = records.team(country.into(), number as u16) {
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
        let yes_no = |on: bool| game.text(if on { YES } else { NO }).to_owned();
        for (ids, t) in [(NOW, &now), (OFFER, &offer)] {
            let word = |at: usize| i64::from(u16::from_le_bytes([t[at], t[at + 1]]));
            let f = flags(t);
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

    fn input(&mut self, cx: &mut Cx, _view: &mut View, target: Target, _double: bool) -> Outcome {
        match target {
            Target::Button(c) if cx.dialog.controls[c].id == Some(CANCEL) => Outcome::Close(Reply::Cancel),
            _ => Outcome::Stay,
        }
    }
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
