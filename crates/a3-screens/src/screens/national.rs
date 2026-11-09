//! The national team, the menu bar's sixteenth symbol: a menu of six buttons
//! on a picture (dialog 355, `CNationalmannschaft`; NATIONALMANNSCHAFT.md in
//! the RE repo).

use std::collections::HashMap;

use a3_core::cycle::dialogs::{LINEUP, NATIONAL_MENU, NOTICE, QUESTION};
use a3_core::national;
use a3_core::season::Rules;
use a3_files::rch::Dialog;

use super::hub;
use super::{Args, Cx, Outcome, Reply, Request, Screen, control, mark_inert, set};
use crate::controls::options::NONE;
use crate::pointer::Target;
use crate::view::View;

/// The OK, hidden under the bar; the caption and its text, "ANSTOSS 3 -
/// Nationalmannschaft %s %s".
const OK: u16 = 1;
const CAPTION_ID: u16 = 9935;
const CAPTION: u16 = 3608;
/// The six buttons, one group (`CMyHighButton`).
const BUTTONS: u16 = 4538;
/// The last button, "Als Nationaltrainer zurücktreten", and its three
/// answers: refused before a match, sure?, accepted.
const RESIGN: usize = 5;
const NOT_BEFORE_THE_MATCH: u16 = 3615;
const REALLY_RESIGN: u16 = 3471;
const RESIGNATION_ACCEPTED: u16 = 3472;

/// The bar takes the OK's place; the caption.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    let Some(dialog) = dialogs.get_mut(&NATIONAL_MENU) else { return };
    dialog.controls.retain(|c| c.id != Some(OK));
    dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, String::new()));
    hub::add_bar(dialog, rules);
}

/// What the menu has opened over itself.
#[derive(Default, PartialEq)]
enum Open {
    #[default]
    Nothing,
    /// The whistle's question.
    Whistle,
    /// "Your chairman refuses", or a resignation asked after.
    Refused,
    Question,
    /// The resignation's last word; the line-up comes after it.
    Accepted,
}

/// The menu (`FUN_0051dd53`).
#[derive(Default)]
pub struct NationalMenu {
    open: Open,
}

impl Screen for NationalMenu {
    fn ids(&self) -> &[u16] {
        &[NATIONAL_MENU]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        hub::open_national_bar(cx, view);
        set(view, dialog, CAPTION_ID, title(cx));
        if let Some(buttons) = control(dialog, BUTTONS).and_then(|c| view.options_mut(c)) {
            buttons.actions = true;
            buttons.chosen = NONE;
        }
        let works = |target: Target| match target {
            Target::Option { control: c, item: RESIGN } => dialog.controls[c].id == Some(BUTTONS),
            _ => hub::on_bar(dialog, target).is_some_and(hub::bar_works),
        };
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        if let Some(item) = hub::on_bar(cx.dialog, target) {
            let outcome = hub::bar_click(cx, view, item, hub::NATIONAL_SYMBOL);
            self.open = if matches!(outcome, Outcome::Popup(_)) { Open::Whistle } else { Open::Nothing };
            return outcome;
        }
        match target {
            Target::Option { control: c, item: RESIGN } if cx.dialog.controls[c].id == Some(BUTTONS) => {
                self.resign(cx, view)
            }
            _ => Outcome::Stay,
        }
    }

    /// What the menu opened has closed; the chosen button lets go only now
    /// (asm 0x51e3f9).
    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        match std::mem::take(&mut self.open) {
            Open::Whistle if reply == Reply::Ok => return Outcome::Close(Reply::Ok),
            Open::Question if reply == Reply::Ok => {
                resigned(cx);
                self.open = Open::Accepted;
                return Outcome::Popup(Request::with(NOTICE, Args::Text(RESIGNATION_ACCEPTED)));
            }
            Open::Accepted => return Outcome::Close(Reply::Goto(LINEUP)),
            _ => {}
        }
        hold(cx, view, None);
        Outcome::Stay
    }
}

impl NationalMenu {
    /// The resignation (asm 0x51e2b2): only a manager with a nation has
    /// one to give, not in a week his team plays.
    fn resign(&mut self, cx: &mut Cx, view: &mut View) -> Outcome {
        let season = cx.state.season.as_ref();
        let Some(nation) = season.and_then(|s| s.manager()).and_then(|m| m.national_team) else {
            return Outcome::Stay;
        };
        let refused = cx.state.records.as_ref().is_some_and(|r| {
            national::plays_this_week(r, nation as i16)
        });
        hold(cx, view, Some(RESIGN));
        if refused {
            self.open = Open::Refused;
            return Outcome::Popup(Request::with(NOTICE, Args::Text(NOT_BEFORE_THE_MATCH)));
        }
        self.open = Open::Question;
        Outcome::Popup(Request::with(QUESTION, Args::Text(REALLY_RESIGN)))
    }
}

/// Holds a button down, as the group draws its chosen item, or lets it go:
/// the menu's buttons stay chosen while what they opened is up.
fn hold(cx: &Cx, view: &mut View, item: Option<usize>) {
    if let Some(buttons) = control(cx.dialog, BUTTONS).and_then(|c| view.options_mut(c)) {
        buttons.actions = item.is_none();
        buttons.chosen = item.unwrap_or(NONE);
    }
}

/// The resignation accepted: the job goes, in the records and the season
/// (`FUN_006b9872`, reason 4).
fn resigned(cx: &mut Cx) {
    let Some(season) = cx.state.season.as_mut() else { return };
    if let Some(records) = cx.state.records.as_mut()
        && let Some(slot) = records.human_slot(season.current)
    {
        national::leave(records, slot, national::RESIGNED);
    }
    if let Some(manager) = season.managers.get_mut(season.current) {
        manager.national_team = None;
    }
}

/// The caption: the manager, his nation in brackets, the date; without a
/// nation no brackets and no comma ("%s (%s), %s" at 0x8858b4, "%s %s" at
/// 0x8858c0).
fn title(cx: &Cx) -> String {
    let game = cx.game;
    let Some(season) = cx.state.season.as_ref() else { return String::new() };
    let Some(manager) = season.manager() else { return String::new() };
    let who = game
        .text(CAPTION)
        .replacen("%s", &manager.first_name, 1)
        .replacen("%s", &manager.last_name, 1);
    let date = hub::date_text(game, season.date());
    match manager.national_team {
        Some(nation) => format!("{who} ({}), {date}", game.world.nation(nation)),
        None => format!("{who} {date}"),
    }
}
