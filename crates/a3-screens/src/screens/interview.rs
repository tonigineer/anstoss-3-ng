//! The interview after the match (dialog 383, `CInterviewDialog`,
//! `IDD_INTERVIEW`): the reporter's words at the top, the three answers
//! below to click (`OnClicked_IDC_ANTWORT1` … 3, `FUN_004ce64c`). A turn
//! to another topic stands two seconds with the answers empty
//! (`FUN_004ce49c(2000)`); the thanks bring the OK button up, hidden
//! till then (`FUN_004ce326`). A click on an answer's place ends either
//! early, as the original's handlers release the wait whatever shows.
//! The questions and their effects are [`a3_core::interview`]'s.

use a3_core::cycle::dialogs::INTERVIEW;
use a3_core::interview::Page;

use super::{Args, Cx, Outcome, Reply, Screen, control, set};
use crate::pointer::Target;
use crate::view::View;

/// The reporter's words, the answers, OK.
const WORDS: u16 = 3610;
const ANSWERS: [u16; 3] = [4735, 4736, 4737];
const OK: u16 = 1;
/// How long a turn to another topic stands.
const REMARK_SECONDS: f32 = 2.0;

#[derive(Default)]
pub struct InterviewScreen {
    /// How long the page has stood.
    shown: f32,
}

impl InterviewScreen {
    fn show(&mut self, cx: &mut Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        self.shown = 0.0;
        let Some(interview) = cx.state.season.as_ref().and_then(|s| s.interview.as_ref()) else { return };
        let said = |id: u16| game.text(id).replacen("%s", &interview.manager_name, 1);
        let (words, answers, done) = match &interview.page {
            Page::Question { text, answers } => (text.clone(), answers.clone(), false),
            Page::Remark(id) => (said(*id), Default::default(), false),
            Page::Closing(id) => (said(*id), Default::default(), true),
        };
        set(view, dialog, WORDS, words);
        for (id, answer) in ANSWERS.into_iter().zip(answers) {
            set(view, dialog, id, answer);
        }
        if let Some(c) = control(dialog, OK) {
            view.controls[c].hidden = !done;
        }
    }

    /// The page goes on: an answer given, a remark or the thanks clicked
    /// away.
    fn go_on(&mut self, cx: &mut Cx, view: &mut View, choice: usize) -> Outcome {
        let state = &mut *cx.state;
        let (Some(season), Some(records)) = (state.season.as_mut(), state.records.as_mut()) else {
            return Outcome::Close(Reply::Ok);
        };
        let Some(interview) = season.interview.as_mut() else { return Outcome::Close(Reply::Ok) };
        match interview.page {
            Page::Question { .. } => interview.answer(
                choice,
                records,
                &cx.game.data.rules.discipline.mood,
                &mut season.interview_recent,
                &mut season.random,
            ),
            Page::Remark(_) => interview.go_on(),
            Page::Closing(_) => return Outcome::Close(Reply::Ok),
        }
        self.show(cx, view);
        Outcome::Stay
    }
}

impl Screen for InterviewScreen {
    fn ids(&self) -> &[u16] {
        &[INTERVIEW]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        for id in ANSWERS {
            if let Some(c) = control(dialog, id) {
                view.set_clickable(c, true);
            }
        }
        self.show(cx, view);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            Target::Control(c) => match ANSWERS.iter().position(|&id| dialog.controls[c].id == Some(id)) {
                Some(choice) => self.go_on(cx, view, choice),
                None => Outcome::Stay,
            },
            Target::Button(c) if dialog.controls[c].id == Some(OK) => {
                let closing = cx.state.season.as_ref().and_then(|s| s.interview.as_ref()).is_some_and(|i| matches!(i.page, Page::Closing(_)));
                if closing { Outcome::Close(Reply::Ok) } else { Outcome::Stay }
            }
            _ => Outcome::Stay,
        }
    }

    fn tick(&mut self, cx: &mut Cx, view: &mut View, seconds: f32) -> Outcome {
        let remark = cx.state.season.as_ref().and_then(|s| s.interview.as_ref()).is_some_and(|i| matches!(i.page, Page::Remark(_)));
        if !remark {
            return Outcome::Stay;
        }
        self.shown += seconds;
        if self.shown < REMARK_SECONDS {
            return Outcome::Stay;
        }
        self.go_on(cx, view, 0)
    }
}
