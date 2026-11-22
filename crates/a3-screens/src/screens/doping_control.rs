//! The doping control after the match (dialog 41, `CDopingAusgabe`,
//! filled by `FUN_00487d65` from `DAT_00a2e0b8`): the club in the caption
//! ("Dopingkontrolle %s", `FUN_0063e52f`), and for each man tested, two
//! at most, his name (`FUN_006feb4d`), the A sample "positiv" or
//! "negativ", and behind a positive one the B sample — gone, lost or no
//! agent found, the player "nicht gedopt", or positive too, the player
//! "gedopt". A negative A sample leaves the B sample and the verdict
//! empty, a man not tested his whole row. OK closes (`OnClicked_IDOK`).
//! The control itself is [`a3_core::doping_control`].

use std::collections::HashMap;

use a3_core::doping_control::{B_CLEARS, TESTED_MOST};
use a3_files::rch::Dialog;

use super::discipline::CAPTION_ID;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::pointer::Target;
use crate::view::View;

pub const DOPING_CONTROL: u16 = a3_core::cycle::dialogs::DOPING_CONTROL;
const OK: u16 = 1;
/// The rows' controls (`FUN_00487c42`): the name, the A sample, the B
/// sample, the verdict.
const NAMES: [u16; TESTED_MOST] = [1380, 1381];
const A_SAMPLES: [u16; TESTED_MOST] = [1382, 1386];
const B_SAMPLES: [u16; TESTED_MOST] = [1383, 1387];
const VERDICTS: [u16; TESTED_MOST] = [1384, 1388];
/// IDS_DOPING_POSITIV, _NEGATIV; the B sample below three:
/// _VERSCHWUNDEN, _VERSCHLAMPT, _KEIN_MITTEL; IDS_SPIELER_NICHT_GEDOPT,
/// IDS_SPIELER_GEDOPT.
const POSITIVE: u16 = 1945;
const NEGATIVE: u16 = 1946;
const CLEARED: [u16; B_CLEARS as usize] = [1947, 1948, 1949];
const NOT_DOPED: u16 = 1950;
const DOPED: u16 = 1951;

/// The dialog keeps its title to itself; the caption bar takes it.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&DOPING_CONTROL) else { return };
    let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}

#[derive(Default)]
pub struct DopingControl;

impl Screen for DopingControl {
    fn ids(&self) -> &[u16] {
        &[DOPING_CONTROL]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let dialog = cx.dialog;
        let Args::Doping(c) = args else { return };
        let records = cx.state.records.as_ref();
        let club = cx.game.club_name(records, c.club.0, c.club.1);
        set(view, dialog, CAPTION_ID, dialog.title.replacen("%s", &club, 1));
        let text = |id: u16| cx.game.text(id).to_owned();
        for k in 0..TESTED_MOST {
            let row = if k < usize::from(c.count) {
                let name = records.and_then(|r| r.known_as(c.players[k])).unwrap_or_default();
                if !c.positive[k] {
                    [name, text(NEGATIVE), String::new(), String::new()]
                } else if c.banned(k) {
                    // The B sample's line keeps the A sample's text.
                    [name, text(POSITIVE), text(POSITIVE), text(DOPED)]
                } else {
                    [name, text(POSITIVE), text(CLEARED[usize::from(c.b_sample[k])]), text(NOT_DOPED)]
                }
            } else {
                Default::default()
            };
            for (id, line) in [NAMES[k], A_SAMPLES[k], B_SAMPLES[k], VERDICTS[k]].into_iter().zip(row) {
                set(view, dialog, id, line);
            }
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
