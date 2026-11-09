//! The national team, the menu bar's sixteenth symbol: a menu of six buttons
//! on a picture (dialog 355, `CNationalmannschaft`; NATIONALMANNSCHAFT.md in
//! the RE repo).

use std::collections::HashMap;

use a3_core::cycle::dialogs::NATIONAL_MENU;
use a3_core::season::Rules;
use a3_files::rch::Dialog;

use super::hub;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
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

/// The bar takes the OK's place; the caption.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    let Some(dialog) = dialogs.get_mut(&NATIONAL_MENU) else { return };
    dialog.controls.retain(|c| c.id != Some(OK));
    dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, String::new()));
    hub::add_bar(dialog, rules);
}

/// The menu (`FUN_0051dd53`).
#[derive(Default)]
pub struct NationalMenu {
    /// Whether the whistle asks.
    whistle: bool,
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
        mark_inert(view, dialog, |target| hub::on_bar(dialog, target).is_some_and(hub::bar_works));
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let Some(item) = hub::on_bar(cx.dialog, target) else { return Outcome::Stay };
        let outcome = hub::bar_click(cx, view, item, hub::NATIONAL_SYMBOL);
        self.whistle = matches!(outcome, Outcome::Popup(_));
        outcome
    }

    /// The whistle's question: a yes ends the turn.
    fn answer(&mut self, _: &mut Cx, _: &mut View, reply: Reply) -> Outcome {
        if std::mem::take(&mut self.whistle) && reply == Reply::Ok {
            return Outcome::Close(Reply::Ok);
        }
        Outcome::Stay
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
