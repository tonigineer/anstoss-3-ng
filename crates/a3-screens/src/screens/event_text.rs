//! A day's event (dialog 51, `CEreignistext`, and for a text past 200
//! characters dialog 435, `CEreignisTextMittel` and `CEreignisTextGross`,
//! which do the same; set up by `FUN_00497ea3`): the caption the window's
//! title with the club's human manager ("%s %s", first and last name)
//! and the club ("%s (%s)", or alone without a manager) behind it, the
//! event's text (control 1451, `DAT_00a2f700`), and the ball pointing at
//! its "i" (control 1453, the frames listed at `0x89f2f4`, every 200 ms).
//! OK closes (`OnClicked_IDOK`). What posts the events is
//! [`a3_core::event_text`].

use std::collections::HashMap;

use a3_core::event_text::{self, EVENT_TEXT, EVENT_TEXT_LARGE};
use a3_files::rch::Dialog;
use a3_files::saq::EventTexts;

use super::discipline::CAPTION_ID;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::pointer::Target;
use crate::view::View;

const OK: u16 = 1;
const TEXT: u16 = 1451;
const BALL: u16 = 1453;
/// The ball's frames (`0x89f2f4`, up to its 0).
const BALL_FRAMES: std::ops::RangeInclusive<u16> = 1762..=1773;

/// The windows keep their title to themselves; the caption bar takes it.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    for id in [EVENT_TEXT, EVENT_TEXT_LARGE] {
        let Some(dialog) = dialogs.get_mut(&id) else { continue };
        let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
        dialog.controls.insert(0, caption);
    }
}

#[derive(Default)]
pub struct EventText;

impl Screen for EventText {
    fn ids(&self) -> &[u16] {
        &[EVENT_TEXT, EVENT_TEXT_LARGE]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let dialog = cx.dialog;
        let Args::Event(posted) = args else { return };
        let Some(records) = cx.state.records.as_ref() else { return };
        let Some(texts) = EventTexts::load(&cx.game.dir) else { return };
        let mut event = event_text::make(records, &texts, &|id| cx.game.text(id).to_owned(), &cx.game.rules.words.first_names, &posted);
        // What only the game's world and money writing fill in.
        match posted.fill {
            event_text::Fill::Club => {
                let slot = cx.game.world.slot_of(posted.club.0).unwrap_or(0);
                event.text = cx.game.with_club(&event.text, (slot, usize::from(posted.club.1)));
            }
            event_text::Fill::FalseReport(_, amount) => {
                event.text = event.text.replacen("%s", &super::playerinfo::money(cx.game, i64::from(amount)), 1);
            }
            _ => {}
        }
        let mut who = String::new();
        if let Some(m) = usize::from(event.manager).checked_sub(1).and_then(|m| records.managers.get(m)) {
            who = format!("{} {}", m.first_name(), m.last_name());
        }
        if event.club.0 != 0 {
            let club = cx.game.club_name(Some(records), event.club.0, event.club.1 as i16);
            who = if who.is_empty() { club } else { format!("{who} ({club})") };
        }
        set(view, dialog, CAPTION_ID, format!("{} {who}", dialog.title));
        set(view, dialog, TEXT, event.text);
        if let Some(ball) = control(dialog, BALL).and_then(|b| view.anim_mut(b)) {
            ball.frames = BALL_FRAMES.collect();
            ball.playing = true;
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
