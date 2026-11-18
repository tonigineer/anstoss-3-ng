//! The contract's word at the season's turn (message 0x10b,
//! `CMyStartDialog::FUN_0066aa06`, run modally by `FUN_007a0bfd`): the
//! season's new goal where the club stayed in its league (dialog 379,
//! `CVertragskonditionenSaisonwechsel`, "Neues Saisonziel") or the new
//! terms where it went up or down (dialog 314, `CVertragskonditionenNeu`,
//! "Neue Vertragskonditionen %s" with the club's name, its flag 1 on a
//! promotion and 0 on a relegation). The sums have moved before the
//! dialog opens ([`a3_core::career::season_terms`]). OK closes; the
//! demand (`IDC_VERTRAG_KONDI_FORDERN`) is not built.

use std::collections::HashMap;

use a3_files::rch::Dialog;

use super::discipline::CAPTION_ID;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::pointer::Target;
use crate::view::View;

pub const SEASON_GOAL: u16 = a3_core::cycle::dialogs::SEASON_GOAL;
pub const NEW_TERMS: u16 = a3_core::cycle::dialogs::NEW_TERMS;
const OK: u16 = 1;

/// Both dialogs keep their title to themselves; the caption bar takes it.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    for id in [SEASON_GOAL, NEW_TERMS] {
        let Some(dialog) = dialogs.get_mut(&id) else { continue };
        let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
        dialog.controls.insert(0, caption);
    }
}

#[derive(Default)]
pub struct SeasonTerms {
    /// The new terms' flag: set on a promotion, clear on a relegation.
    pub promoted: bool,
}

impl Screen for SeasonTerms {
    fn ids(&self) -> &[u16] {
        &[SEASON_GOAL, NEW_TERMS]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let dialog = cx.dialog;
        self.promoted = matches!(args, Args::NewTerms { promoted: true });
        let club = cx.state.season.as_ref().and_then(|s| {
            let (slot, index) = s.managers.get(s.current)?.club?;
            Some(cx.game.data.world.country(slot)?.clubs.get(index)?.name.clone())
        });
        set(view, dialog, CAPTION_ID, dialog.title.replacen("%s", &club.unwrap_or_default(), 1));
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
