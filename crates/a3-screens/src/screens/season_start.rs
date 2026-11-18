//! The season start's screens at the season's turn: the event 0x82 each
//! human club's manager gets (`FUN_00671c60`, run by
//! `CMyStartDialog::FUN_00671d57`) — the negotiations he leads himself
//! (dialog 422, `CSaisonstartVerhandlungen`), the European television's
//! talks behind them (dialog 475, `CDiVerhandlungenUefa`) and the season's
//! preparation (dialog 419, `CVorbereitung`) — and the event 0x130 behind them
//! (`FUN_0062def0`, run by `CMyStartDialog::FUN_0062de93`): the overviews
//! of the season's start (dialog 420, `CSaisonstartAuslosungen`).
//!
//! Only the dialogs come up here, OK closing each. The negotiations' boxes
//! show the talks the manager may lead, ticked or off as the original
//! sets them ([`a3_core::finances::season_talks`]), and tick and untick;
//! OK hands their ticks to the cycle, which has the computer sign the
//! talks left unticked ([`a3_core::settlement::sign_left_talks`]) and
//! opens the European television's talks for a kept box. Those talks'
//! offers and bargaining, the preparation's figures and choices and the
//! overviews are not built, so the value fields the original fills stay
//! empty and every other control is inert.

use std::collections::HashMap;

use a3_core::cycle::dialogs;
use a3_core::finances::season_talks;
use a3_files::rch::Dialog;

use super::discipline::CAPTION_ID;
use super::squad::own_team;
use super::{Args, Cx, Outcome, Reply, Request, Screen, control, mark_inert, set};
use crate::pointer::Target;
use crate::view::View;

pub const TALKS: u16 = dialogs::SEASON_START_TALKS;
pub const PREPARATION: u16 = dialogs::SEASON_PREPARATION;
pub const OVERVIEWS: u16 = dialogs::SEASON_OVERVIEWS;
pub const EURO_TV: u16 = dialogs::EURO_TV_TALKS;
const OK: u16 = 1;
/// The European television's three offers, a station's name and its sum
/// each, which the file holds samples in ("RTL plus", "6.000.000 DM
/// fest").
const EURO_TV_OFFERS: [u16; 6] = [6051, 6052, 6053, 6054, 6055, 6056];
/// The question the European television's OK asks.
const EURO_TV_LEAVE: u16 = 0x1251;

/// The negotiations' boxes (`FUN_0053af88`): main sponsor, boards, kit,
/// player bonuses, souvenirs, European television.
const TALK_BOXES: [u16; 6] = [0x889, 0x88a, 0x88b, 0x88c, 0x179b, 0x179c];
/// The television's box, its "%s" filled with nothing (`DAT_00ae66ce`).
const EURO_TV_BOX: u16 = 0x179c;

/// The preparation's value fields, which the file holds sample figures in
/// (`CVorbereitung::FUN_005c79c5`'s): the two coaches proposed, the
/// fitness coaches' count, the four departments' investments, the
/// proposals for the trips and the shirts, the club's money, its
/// reckoned profit and its credit limit.
const PREPARATION_VALUES: [u16; 12] = [5120, 5121, 5111, 5106, 5107, 5108, 5109, 5816, 5815, 5048, 5049, 5050];

/// The sample figures go, and the labels the original completes with a
/// name or a sum keep their words without the gap; the European
/// television's talks get their caption.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    if let Some(dialog) = dialogs.get_mut(&EURO_TV) {
        dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, dialog.title.clone()));
    }
    for id in [TALKS, PREPARATION, OVERVIEWS, EURO_TV] {
        let Some(dialog) = dialogs.get_mut(&id) else { continue };
        for c in &mut dialog.controls {
            let samples: &[u16] = match id {
                PREPARATION => &PREPARATION_VALUES,
                EURO_TV => &EURO_TV_OFFERS,
                _ => &[],
            };
            if c.id.is_some_and(|i| samples.contains(&i)) {
                c.text.clear();
            } else if c.id == Some(EURO_TV_BOX) {
                c.text = c.text.replace("%s", "");
            } else if c.id.is_some() && c.text.contains("%s") {
                c.text = c.text.replace(" %s", "").replace("%s", "");
            }
        }
    }
}

pub struct SeasonStart;

impl Screen for SeasonStart {
    fn ids(&self) -> &[u16] {
        &[TALKS, PREPARATION, OVERVIEWS, EURO_TV]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        // The European television's talks name the manager and his club
        // after their title.
        if view.id == EURO_TV {
            let manager = cx.state.season.as_ref().and_then(|s| s.manager());
            let who = manager.map_or_else(String::new, |m| format!("{} {}", m.first_name, m.last_name));
            let club = manager
                .and_then(|m| m.club)
                .and_then(|(slot, c)| cx.game.world.country(slot)?.clubs.get(c))
                .map_or_else(String::new, |c| c.name.clone());
            set(view, dialog, CAPTION_ID, format!("{} {who} ({club})", dialog.title));
        }
        // Only the negotiations hold the boxes.
        let talks = control(dialog, TALK_BOXES[0])
            .and(own_team(cx).zip(cx.state.records.as_ref()))
            .map(|(team, records)| season_talks(team, records.globals.preparation()));
        if let Some(talks) = talks {
            for (id, talk) in TALK_BOXES.into_iter().zip(talks) {
                let Some(c) = control(dialog, id) else { continue };
                if let Some(check) = view.check_mut(c) {
                    (check.checked, check.disabled) = (talk.ticked, !talk.open);
                }
            }
        }
        mark_inert(view, dialog, |t| match t {
            Target::Button(c) => dialog.controls[c].id == Some(OK),
            Target::Check(c) => dialog.controls[c].id.is_some_and(|i| TALK_BOXES.contains(&i)),
            _ => false,
        });
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            // The negotiations' ticks go back one bit each, in the boxes'
            // order (`FUN_0053b3ed` keeps them at +0xa08 … +0xa1c).
            Target::Button(c) if control(cx.dialog, OK) == Some(c) && view.id == TALKS => {
                let ticks = TALK_BOXES.iter().enumerate().fold(0, |mask, (k, &id)| {
                    let ticked = control(cx.dialog, id).and_then(|b| view.check(b)).is_some_and(|b| b.checked);
                    mask | usize::from(ticked) << k
                });
                Outcome::Close(Reply::Choice(ticks))
            }
            // The European television's talks ask before they close
            // (`OnClicked_IDOK_005ba6d2`: `CAlertOkAbbruch` with text 0x1251).
            Target::Button(c) if control(cx.dialog, OK) == Some(c) && view.id == EURO_TV => {
                Outcome::Popup(Request::with(dialogs::QUESTION, Args::Text(EURO_TV_LEAVE)))
            }
            Target::Button(c) if control(cx.dialog, OK) == Some(c) => Outcome::Close(Reply::Ok),
            _ => Outcome::Stay,
        }
    }

    /// The question's OK closes the European television's talks, its
    /// cancel keeps them.
    fn answer(&mut self, _: &mut Cx, _: &mut View, reply: Reply) -> Outcome {
        if reply == Reply::Ok { Outcome::Close(Reply::Ok) } else { Outcome::Stay }
    }
}
