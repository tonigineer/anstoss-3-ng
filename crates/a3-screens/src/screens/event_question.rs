//! The question with up to three answers, dialog 50 — `CEreignisFrage`:
//! the caption the window's title with the manager's name after it
//! (`this + 0x10d4`, "%s %s" first and last), the question in its text
//! control 1445, the ball in the picture 1336 (its frames every 200 ms,
//! `FUN_007957a4`), the picture 1450, and the rows in the options
//! control 1447. Their labels are the question table's words (`0x88f090`,
//! stride 0x2c, this window's entry `kind + 0x21`; its +8, +0xc and
//! +0x10 are the rows, and a row whose word is 0 stands hidden,
//! `FUN_007a72c2`); the label format `FUN_005eea5d` writes money at a
//! `%4`. Pointing at a row answers it (0 to 2, `OnClicked_IDC_EREIGNIS-
//! FRAGE_ANTW1..3`), OK closes with the answer still at nought
//! (`OnClicked_IDOK`).
//!
//! Two windows wear it: the camp group's choice among places sharing one
//! point on the year calendar's world map (`CTrainingslagerSubAuswahl`,
//! asked by [`super::camp`], which hides OK), and the question over a
//! fine the day's news asks the club's manager ([`a3_core::fine_question`]
//! kind 1, food's case 4) — there the picture 1450 stands hidden, OK
//! stays and answers the first row.

use std::collections::HashMap;

use a3_core::event_text;
use a3_core::fine_question;
use a3_files::rch::Dialog;
use a3_files::saq::EventTexts;

use super::discipline::CAPTION_ID;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::pointer::Target;
use crate::view::View;

/// The window itself, [`super::camp`]'s group choice and all — and the
/// camp group's question ("Wohin wollt ihr fahren?", text 4587).
const DIALOG: u16 = 50;
const QUESTION: u16 = 1445;
const CAMP_QUESTION_TEXT: u16 = 4587;
const BALL: u16 = 1336;
const PICTURE: u16 = 1450;
const ROWS: u16 = 1447;
const OK: u16 = 1;

/// The captions: the window keeps its title to itself, the caption bar
/// takes it.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&DIALOG) else { return };
    let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}

#[derive(Default)]
pub struct EventQuestion;

impl Screen for EventQuestion {
    fn ids(&self) -> &[u16] {
        &[DIALOG]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        match args {
            Args::CampGroup { places, count } => self.camp_group(cx, view, places, count),
            Args::EventQuestion(q) => self.fine_question(cx, view, q),
            _ => {}
        }
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Option { control: c, item } if cx.dialog.controls[c].id == Some(ROWS) => {
                // A row answers its place or its punishment and ends it.
                Outcome::Close(Reply::Choice(item))
            }
            Target::Button(c) if cx.dialog.controls[c].id == Some(OK) => {
                // OK closes with the answer still at nought.
                Outcome::Close(Reply::Ok)
            }
            _ => Outcome::Stay,
        }
    }
}

impl EventQuestion {
    /// `FUN_005a2bf8`: the title with the manager's name, the question,
    /// a bouncing ball, and a row for each of the group's other places;
    /// OK stays hidden, so only a row ends it.
    fn camp_group(&mut self, cx: &mut Cx, view: &mut View, places: [u8; 3], count: u8) {
        let (game, dialog) = (cx.game, cx.dialog);
        let name = cx.state.season.as_ref().and_then(|s| s.manager()).map_or_else(String::new, |m| {
            format!("{} {}", m.first_name, m.last_name)
        });
        set(view, dialog, CAPTION_ID, format!("{} {name}", dialog.title));
        set(view, dialog, QUESTION, game.text(CAMP_QUESTION_TEXT).to_owned());
        self.ball(dialog, view);
        for id in [PICTURE, OK] {
            if let Some(c) = control(dialog, id) {
                view.controls[c].hidden = true;
            }
        }
        let tables = &game.rules.year;
        if let Some(rows) = control(dialog, ROWS).and_then(|c| view.options_mut(c)) {
            // Answers, not a choice: none stands chosen, the one pointed
            // at lights up.
            rows.actions = true;
            rows.chosen = crate::controls::options::NONE;
            let count = usize::from(count);
            for (row, &place) in places.iter().enumerate() {
                if let Some(label) = rows.labels.get_mut(row) {
                    let name = tables.places.get(usize::from(place)).map_or(0, |p| p.name);
                    *label = if row < count { game.text(name).to_owned() } else { String::new() };
                }
                if let Some(off) = rows.off.get_mut(row) {
                    *off = row >= count;
                }
            }
        }
        mark_inert(view, dialog, |_| true);
    }

    /// `FUN_005f29bc` kind 1: the question of event 0x4b8 built over the
    /// man's name with `%2` empty (`FUN_005eee7f` into `DAT_00a32210`,
    /// which the window shows in place of one of its own), and the three
    /// rows — the middle one naming the fine at its `%4`
    /// (`FUN_005eea5d`, the money writing of the fine, `DAT_00a321fc`).
    fn fine_question(&mut self, cx: &mut Cx, view: &mut View, q: fine_question::Question) {
        let (game, dialog) = (cx.game, cx.dialog);
        let name = cx.state.season.as_ref().and_then(|s| s.manager()).map_or_else(String::new, |m| {
            format!("{} {}", m.first_name, m.last_name)
        });
        set(view, dialog, CAPTION_ID, format!("{} {name}", dialog.title));
        let texts = EventTexts::load(&game.dir);
        if let (Some(records), Some(texts)) = (cx.state.records.as_ref(), texts.as_ref()) {
            let asked = event_text::Posted {
                club: q.club,
                code: 0,
                text: fine_question::QUESTION_TEXT,
                fill: event_text::Fill::NumberedOne(q.player),
            };
            let made = event_text::make(records, texts, &|id| game.text(id).to_owned(), &game.rules.words.first_names, &asked);
            set(view, dialog, QUESTION, made.text);
            if let Some(rows) = control(dialog, ROWS).and_then(|c| view.options_mut(c)) {
                rows.actions = true;
                rows.chosen = crate::controls::options::NONE;
                let fine = super::playerinfo::money(game, i64::from(fine_question::FINE));
                for (row, &word) in fine_question::ANSWERS.iter().enumerate() {
                    if let Some(label) = rows.labels.get_mut(row) {
                        *label = texts.get(usize::from(word)).map_or_else(String::new, |t| t.replace("%4", &fine));
                    }
                    if let Some(off) = rows.off.get_mut(row) {
                        *off = false;
                    }
                }
            }
        }
        self.ball(dialog, view);
        if let Some(c) = control(dialog, PICTURE) {
            view.controls[c].hidden = true;
        }
        // OK stays: it closes with the answer at nought, the first row.
        mark_inert(view, dialog, |t| match t {
            Target::Button(c) => dialog.controls[c].id == Some(OK),
            Target::Option { .. } => true,
            _ => false,
        });
    }

    /// The bouncing ball, its frames as the camp's window plays them.
    fn ball(&mut self, dialog: &Dialog, view: &mut View) {
        if let Some(ball) = control(dialog, BALL).and_then(|b| view.anim_mut(b)) {
            ball.frames = (1745u16..=1753).chain((1746..=1752).rev()).collect();
            ball.playing = true;
        }
    }
}
