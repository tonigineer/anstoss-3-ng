//! The season start's screens at the season's turn: the event 0x82 each
//! human club's manager gets (`FUN_00671c60`, run by
//! `CMyStartDialog::FUN_00671d57`) — the negotiations he leads himself
//! (dialog 422, `CSaisonstartVerhandlungen`) and the season's preparation
//! (dialog 419, `CVorbereitung`) — and the event 0x130 behind them
//! (`FUN_0062def0`, run by `CMyStartDialog::FUN_0062de93`): the overviews
//! of the season's start (dialog 420, `CSaisonstartAuslosungen`).
//!
//! Only the dialogs come up here, OK closing each: their figures, choices
//! and the talks, plans and overviews they open are not built, so the
//! value fields the original fills stay empty and every other control is
//! inert.

use std::collections::HashMap;

use a3_core::cycle::dialogs;
use a3_files::rch::Dialog;

use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert};
use crate::pointer::Target;
use crate::view::View;

pub const TALKS: u16 = dialogs::SEASON_START_TALKS;
pub const PREPARATION: u16 = dialogs::SEASON_PREPARATION;
pub const OVERVIEWS: u16 = dialogs::SEASON_OVERVIEWS;
const OK: u16 = 1;

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
