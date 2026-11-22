//! The association's court (dialog 89, `CGerichtDialog`, filled by
//! `FUN_004b5f82` from `DAT_00a3b44c`): the club behind the title in the
//! caption ("%s %s"), the case — 2093 "Die Sperre Ihres Spielers %s %s"
//! with the player's name (`FUN_006feb4d`) and nothing, the verdict's
//! text (0x8818cc) with its number behind — and, where the court lets the
//! club appeal, the question 2104 with the lawyers' advice, 2105 for, 2106
//! against; without one the question stays empty and Nein is hidden. OK
//! answers yes (`OnClicked_IDC_GERICHT_OK`), Nein no. The original plays
//! `videos/justitia.sks` beside it when videos are on; the replica shows
//! none. The court itself is [`a3_core::court`].

use std::collections::HashMap;

use a3_files::rch::Dialog;

use super::discipline::CAPTION_ID;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::pointer::Target;
use crate::view::View;

pub const COURT: u16 = a3_core::cycle::dialogs::COURT;
const OK: u16 = 1875;
const TEXT: u16 = 1876;
const NO: u16 = 1877;
const QUESTION: u16 = 1878;
/// IDS_DIE_SPERRE, IDS_FRAGE_EINSPRUCH, IDS_RECHTSANWALT_SAGT_JA, _NEIN.
const THE_BAN: u16 = 2093;
const APPEAL: u16 = 2104;
const ADVISED: u16 = 2105;
const ADVISED_AGAINST: u16 = 2106;

/// The dialog keeps its title to itself; the caption bar takes it.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&COURT) else { return };
    let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}

#[derive(Default)]
pub struct Court;

impl Screen for Court {
    fn ids(&self) -> &[u16] {
        &[COURT]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let dialog = cx.dialog;
        let Args::Court(case) = args else { return };
        let records = cx.state.records.as_ref();
        let club = cx.game.club_name(records, case.club.0, case.club.1);
        set(view, dialog, CAPTION_ID, format!("{} {}", dialog.title, club));
        let name = records.and_then(|r| r.known_as(case.player)).unwrap_or_default();
        let ban = cx.game.text(THE_BAN).replacen("%s", &name, 1).replacen("%s", "", 1);
        let verdict = cx.game.text(case.text()).replacen("%d", &case.value.to_string(), 1);
        set(view, dialog, TEXT, ban + &verdict);
        let question = if case.question {
            let advice = if case.advised { ADVISED } else { ADVISED_AGAINST };
            format!("{} {}", cx.game.text(APPEAL), cx.game.text(advice))
        } else {
            String::new()
        };
        set(view, dialog, QUESTION, question);
        if let Some(c) = control(dialog, NO) {
            view.controls[c].hidden = !case.question;
        }
        mark_inert(view, dialog, |t| match t {
            Target::Button(c) => [OK, NO].iter().any(|&id| dialog.controls[c].id == Some(id)),
            _ => false,
        });
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(c) if control(cx.dialog, OK) == Some(c) => Outcome::Close(Reply::Ok),
            Target::Button(c) if control(cx.dialog, NO) == Some(c) => Outcome::Close(Reply::Cancel),
            _ => Outcome::Stay,
        }
    }
}
