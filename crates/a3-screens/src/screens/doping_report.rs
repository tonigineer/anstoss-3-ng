//! Reporting other humans' players to the association (dialog 509,
//! `CDopingMitspieler`), from the doping screen's button 5485: one box
//! for each other human, by his manager's name, ticked when he stands
//! reported (`FUN_004886f9`); a tick reports him, taking it off takes the
//! report back (`OnClicked_IDC_DOPING_MITSPIELER1..3`). OK closes. The
//! reports stand in [`a3_core::season::Season::reported`] until a doping
//! test of the reported club reads them.

use std::collections::HashMap;

use a3_files::rch::Dialog;

use super::discipline::CAPTION_ID;
use super::{Args, Cx, Outcome, Reply, Screen, control, ok_or_cancel};
use crate::controls::options::NONE;
use crate::pointer::Target;
use crate::view::View;

pub(crate) const DOPING_REPORT: u16 = 509;
/// The three boxes (MITSPIELER1..3).
const BOXES: [u16; 3] = [6421, 6422, 6423];

#[derive(Default)]
pub struct DopingReport {
    /// The reporter's number less one, and the humans' by box.
    reporter: usize,
    targets: Vec<usize>,
}

impl Screen for DopingReport {
    fn ids(&self) -> &[u16] {
        &[DOPING_REPORT]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        *self = Self::default();
        super::set(view, dialog, CAPTION_ID, dialog.title.clone());
        let (Some(records), Some(season)) = (cx.state.records.as_ref(), cx.state.season.as_ref())
        else {
            return;
        };
        let Some(own) = records.human_slot(season.current) else { return };
        self.reporter = usize::from(records.managers[own].human()).saturating_sub(1);
        // The humans in their order, all but the reporter.
        let slots = records.globals.slots();
        let mut names = Vec::new();
        for (slot, m) in records.managers.iter().enumerate() {
            if slots.get(slot).copied().unwrap_or(0) == 0 || slot == own {
                continue;
            }
            self.targets.push(usize::from(m.human()).saturating_sub(1));
            names.push(format!("{} {}", m.first_name(), m.last_name()));
        }
        for (k, id) in BOXES.into_iter().enumerate() {
            let Some(o) = control(dialog, id).and_then(|c| view.options_mut(c)) else { continue };
            let target = self.targets.get(k).copied();
            let reported = target.and_then(|t| season.reported.get(self.reporter)?.get(t).copied());
            o.chosen = if reported == Some(true) { 0 } else { NONE };
            match names.get(k) {
                Some(name) => {
                    if let Some(label) = o.labels.get_mut(0) {
                        label.clone_from(name);
                    }
                }
                // Unused boxes disabled (`FUN_007a72c2` with
                // `WS_DISABLED`) and clear.
                None => o.dimmed = vec![true],
            }
        }
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            Target::Option { control: c, .. } => {
                let Some(k) = BOXES.iter().position(|&b| dialog.controls[c].id == Some(b)) else {
                    return Outcome::Stay;
                };
                let Some(&t) = self.targets.get(k) else { return Outcome::Stay };
                let ticked = view.options(c).is_some_and(|o| o.selected().is_some());
                // A tick also looks up the human's portrait by name
                // (`FUN_004d242a`), which shows nothing here.
                if let Some(row) =
                    cx.state.season.as_mut().and_then(|s| s.reported.get_mut(self.reporter))
                    && let Some(flag) = row.get_mut(t)
                {
                    *flag = ticked;
                }
                Outcome::Stay
            }
            _ => {
                // The boxes act at once; either button only closes.
                match ok_or_cancel(dialog, target) {
                    (true, _) => Outcome::Close(Reply::Ok),
                    (_, true) => Outcome::Close(Reply::Cancel),
                    _ => Outcome::Stay,
                }
            }
        }
    }
}

/// Puts the dialog's caption in its frame.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&DOPING_REPORT) else { return };
    let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}
