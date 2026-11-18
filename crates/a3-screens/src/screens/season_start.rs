//! The season start's screens at the season's turn: the event 0x82 each
//! human club's manager gets (`FUN_00671c60`, run by
//! `CMyStartDialog::FUN_00671d57`) — the negotiations he leads himself
//! (dialog 422, `CSaisonstartVerhandlungen`) and the season's preparation
//! (dialog 419, `CVorbereitung`) — and the event 0x130 behind them
//! (`FUN_0062def0`, run by `CMyStartDialog::FUN_0062de93`): the overviews
//! of the season's start (dialog 420, `CSaisonstartAuslosungen`).
//!
//! Only the dialogs come up here, OK closing each. The negotiations' boxes
//! show the talks the manager may lead, ticked or off as the original
//! sets them ([`a3_core::finances::season_talks`]); the talks they open,
//! the preparation's figures and choices and the overviews are not built,
//! so the value fields the original fills stay empty and every other
//! control is inert.

use std::collections::HashMap;

use a3_core::cycle::dialogs;
use a3_core::finances::season_talks;
use a3_files::rch::Dialog;

use super::squad::own_team;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert};
use crate::pointer::Target;
use crate::view::View;

pub const TALKS: u16 = dialogs::SEASON_START_TALKS;
pub const PREPARATION: u16 = dialogs::SEASON_PREPARATION;
pub const OVERVIEWS: u16 = dialogs::SEASON_OVERVIEWS;
const OK: u16 = 1;

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
/// name or a sum keep their words without the gap.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    for id in [TALKS, PREPARATION, OVERVIEWS] {
        let Some(dialog) = dialogs.get_mut(&id) else { continue };
        for c in &mut dialog.controls {
            if id == PREPARATION && c.id.is_some_and(|i| PREPARATION_VALUES.contains(&i)) {
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
        &[TALKS, PREPARATION, OVERVIEWS]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
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
            _ => false,
        });
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(c) if control(cx.dialog, OK) == Some(c) => Outcome::Close(Reply::Ok),
            _ => Outcome::Stay,
        }
    }
}
