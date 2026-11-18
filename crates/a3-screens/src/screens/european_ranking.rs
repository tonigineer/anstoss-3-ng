//! The European ranking the season's end rolled on (dialog 301,
//! `CUefa5Jahreswertung`, "5-Jahres-Wertung", event 0xcf of
//! `FUN_00423cad`, handled by `FUN_005b0432`). Its header (`FUN_005b0578`)
//! is the dialog's own titles, "%s\t%s\t%s\t%s\t%s\t%s", with the five
//! seasons up to the year's (`DAT_00b5cbca`), the oldest first, each
//! "%02d/%02d" (`FUN_005b03ff`); a row a country of the ranking's 37 that
//! has one (`DAT_00b5d1a8`, 0x24 a row), "%d.\t%s\t%3.2f…": its row's
//! place, its name (`DAT_00ae5940`), its total and the points of those
//! five seasons (+0xc to +0x1c). OK (`OnClicked_IDC_UEFA5_OK`) closes.

use std::collections::HashMap;

use a3_files::rch::Dialog;

use super::discipline::CAPTION_ID;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert};
use crate::pointer::Target;
use crate::view::View;

pub const EUROPEAN_RANKING: u16 = a3_core::cycle::dialogs::EUROPEAN_RANKING;
const OK: u16 = 4032;
const LIST: u16 = 4031;
const HEADER: u16 = 4034;

/// The dialog keeps its title to itself; the caption bar takes it.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&EUROPEAN_RANKING) else { return };
    let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}

#[derive(Default)]
pub struct EuropeanRanking;

impl Screen for EuropeanRanking {
    fn ids(&self) -> &[u16] {
        &[EUROPEAN_RANKING]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return };
        let year = i32::from(records.globals.start_year());
        if let Some(c) = control(dialog, HEADER) {
            let seasons: Vec<String> =
                (0..5).rev().map(|k| year - k).map(|y| format!("{:02}/{:02}", y % 100, (y + 1) % 100)).collect();
            // The header splits its titles where the text writes "\t".
            let titles = dialog.controls[c].text.clone() + "\\t" + &seasons.join("\\t");
            if let Some(header) = view.header_mut(c) {
                header.text = titles;
            }
        }
        let rows: Vec<String> = a3_core::statistics::five_years(records)
            .iter()
            .enumerate()
            .filter(|(_, (country, _))| *country != 0)
            .map(|(i, (country, points))| {
                let figures: Vec<String> =
                    [0, 1, 2, 3, 4, 5].map(|k| super::statistics::fixed(f64::from(points[k]), 2)).to_vec();
                format!("{}.\t{}\t{}", i + 1, game.world.nation(*country), figures.join("\t"))
            })
            .collect();
        if let Some(list) = control(dialog, LIST).and_then(|c| view.list_mut(c)) {
            list.keys = (0..rows.len()).collect();
            list.rows = rows;
            list.inks.clear();
            list.row_icons.clear();
            list.selected = None;
            list.scroll = 0;
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
