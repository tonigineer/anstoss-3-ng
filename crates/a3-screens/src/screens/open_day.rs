//! The open day of the year calendar (dialog 270,
//! `IDD_TAG_DER_OFFENEN_TUER`): the event the day's walk opens on the
//! day it was booked, telling the club how the day went; its pictures
//! shuffle at the open, and OK closes it again.

use std::collections::HashMap;

use a3_files::rch::Dialog;

use super::{Args, Cx, Outcome, Reply, Screen, control, popups, set};
use crate::pointer::Target;
use crate::view::View;

pub const OPEN_DAY: u16 = a3_core::cycle::dialogs::OPEN_DAY;
const CAPTION_ID: u16 = 9944;
/// The report static and the OK button.
const REPORT: u16 = 3733;
const OK: u16 = 3734;

/// Puts the open day's caption in its frame.
pub(crate) fn compose_open_day(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&OPEN_DAY) else { return };
    let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}

pub struct OpenDay;

impl Screen for OpenDay {
    fn ids(&self) -> &[u16] {
        &[OPEN_DAY]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        // The dialog's ctor (`FUN_006cbbc7`) shuffles its three picture
        // rows before it shows anything: rows of 3, 7 and 4 places.
        if let Some(season) = cx.state.season.as_mut() {
            season.random.shuffle(3);
            season.random.shuffle(7);
            season.random.shuffle(4);
        }
        let club = cx.state.season.as_ref().and_then(|s| s.manager()?.club);
        if let (Some(i), Some(club)) = (control(cx.dialog, REPORT), club) {
            set(view, cx.dialog, REPORT, cx.game.with_club(&cx.dialog.controls[i].text, club));
        }
        popups::ball(view, cx);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(b) if cx.dialog.controls[b].id == Some(OK) => {
                Outcome::Close(Reply::Ok)
            }
            _ => Outcome::Stay,
        }
    }
}
