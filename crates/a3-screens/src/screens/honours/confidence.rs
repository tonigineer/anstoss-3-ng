//! How the board's confidence went (dialog 320 `CVertrauenVorstand`,
//! ERFOLGE.md §B.4), behind the honours page's "Vertrauen Vorstand": the
//! confidence at the end of each of the last ten periods and today's, a
//! red curve over the 50 % line, and a word on where the manager stands.

use std::collections::HashMap;

use a3_files::rch::Dialog;

use crate::pointer::Target;
use crate::screens::{Args, Cx, Outcome, Reply, Screen, caption, control, mark_inert, set};
use crate::view::View;

pub(crate) const CONFIDENCE_DIALOG: u16 = 320;
const CAPTION_ID: u16 = 9947;
/// The curve's paint and the verdict.
const CURVE: u16 = 4156;
const VERDICT: u16 = 4157;
/// "Sie sind der König von %s.", for a confidence of 100; else the words
/// from the most on down, each from its least (texts 2471 to 2476).
const KING: u16 = 2470;
const VERDICTS: [(i32, u16); 6] = [(90, 2471), (80, 2472), (70, 2473), (60, 2474), (55, 2475), (0, 2476)];
/// The curve: a step a period of the paint's width over eleven, from ten
/// pixels in; the middle line five pixels short of either side; red.
const PERIODS: i32 = 11;
const FIRST_X: i32 = 10;
const MIDDLE_INSET: i32 = 5;
const RED: [u8; 3] = [255, 0, 0];

/// The caption bar keeps the dialog's title.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&CONFIDENCE_DIALOG) else { return };
    let caption = caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}

/// The screen.
pub struct Confidence;

impl Screen for Confidence {
    fn ids(&self) -> &[u16] {
        &[CONFIDENCE_DIALOG]
    }

    /// `0x5c6c85`: the verdict, then the curve (`0x5c6e2d`).
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        mark_inert(view, dialog, |_| true);
        view.lines.clear();
        let current = cx.state.season.as_ref().map(|s| s.current);
        let Some(records) = cx.state.records.as_ref() else { return };
        let Some(slot) = current.and_then(|n| records.human_slot(n)) else { return };
        let confidence = a3_core::stock::confidence(records, slot);
        let verdict = if confidence == 100 {
            let manager = cx.state.season.as_ref().and_then(|s| s.manager());
            let club = manager
                .and_then(|m| m.club)
                .and_then(|(country, c)| game.world.country(country)?.clubs.get(c));
            game.text(KING).replacen("%s", club.map_or("", |c| c.name.as_str()), 1)
        } else {
            let text = VERDICTS.iter().find(|&&(least, _)| confidence >= least).map_or(0, |v| v.1);
            game.text(text).to_owned()
        };
        set(view, dialog, VERDICT, verdict);

        let Some(rect) = control(dialog, CURVE).map(|c| dialog.controls[c].rect) else { return };
        let (left, top) = (rect.min.x as i32, rect.min.y as i32);
        let (w, h) = (rect.width() as i32, rect.height() as i32);
        let y = |value: i32| top + h - value * h / 100;
        let mut line = |from: (i32, i32), to: (i32, i32)| {
            view.lines.push(([left + from.0, from.1, left + to.0, to.1], RED));
        };
        // Each line ends on its last pixel too, as the board's chart does.
        let middle = (w - MIDDLE_INSET, top + h / 2);
        line((MIDDLE_INSET, top + h / 2), middle);
        line(middle, middle);
        let history = records.managers[slot].confidence_history().map(i32::from);
        let step = w / PERIODS;
        // From the first period that has a value; with none but the last,
        // a short stroke at today's.
        match history.iter().position(|&v| v != 0).filter(|&first| first < history.len() - 1) {
            Some(first) => {
                let mut at = (FIRST_X, y(history[first]));
                let mut x = FIRST_X;
                for &value in &history[first..] {
                    line(at, (x, y(value)));
                    at = (x, y(value));
                    x += step;
                }
                line(at, (x, y(confidence)));
                line((x, y(confidence)), (x, y(confidence)));
            }
            None => {
                let end = (FIRST_X + step, y(confidence));
                line((FIRST_X, y(confidence)), end);
                line(end, end);
            }
        }
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&cx.dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            _ => Outcome::Stay,
        }
    }
}
