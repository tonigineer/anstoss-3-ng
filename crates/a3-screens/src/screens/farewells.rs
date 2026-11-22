//! The careers the season's end closes (dialog 152, `CMitTraenen`, "mit
//! Tränen", event 0xcd of `FUN_0066d435`, handled by `FUN_00518e4f`): a
//! page for each human club losing men, copied from the block at
//! `0xa321f4` ([`a3_core::rollover::Farewells`]). The season above
//! (`FUN_00518fb8`): text 2113 "Saison %s/%s" with the season's year and
//! the next one's last two digits — the whole year when they are 00 — by
//! "%d". Each page (`FUN_00519123`) lists up to ten men,
//! "%s\t%s\t%d\t%d": "Name, F." or the nickname (`FUN_006fec36`), his
//! position's abbreviation (`FUN_00731356`), his strength and his age
//! (`FUN_006ff01b`). OK (`OnClicked_IDC_MIT_TRAENEN_OK`) turns to the next
//! club's page, after the last it closes. The caption bar is the page's
//! club (text 2114 with its phrases, `FUN_00733401`).

use std::collections::HashMap;

use a3_core::rollover::Farewells as Notice;
use a3_files::rch::Dialog;

use super::discipline::CAPTION_ID;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::pointer::Target;
use crate::view::View;

pub const FAREWELLS: u16 = a3_core::cycle::dialogs::FAREWELLS;
const SEASON: u16 = 2593;
const OK: u16 = 2594;
const LIST: u16 = 2595;
/// IDS "Saison %s/%s".
const SEASON_TEXT: u16 = 2113;
/// IDS "ANSTOSS 3 - Spieler v4, die ihre Karriere beenden...".
const CAPTION_TEXT: u16 = 2114;

/// The caption bar takes the page's club.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&FAREWELLS) else { return };
    let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}

#[derive(Default)]
pub struct Farewells {
    notice: Notice,
    /// The page shown, by club (+0xec0).
    page: usize,
}

impl Farewells {
    /// The page's club's men (`FUN_00519123`).
    fn fill(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return };
        if let Some(&(country, number)) = self.notice.clubs.get(self.page) {
            let slot = game.world.slot_of(country.into()).unwrap_or(0);
            let caption = game.with_club(game.text(CAPTION_TEXT), (slot, usize::try_from(number).unwrap_or(0)));
            set(view, dialog, CAPTION_ID, caption);
        }
        let [day, month, year] = records.globals.date();
        let today = chrono::NaiveDate::from_ymd_opt(i32::from(year), u32::from(month), u32::from(day));
        let mut rows = Vec::new();
        for &id in self.notice.players.get(self.page).into_iter().flatten().filter(|&&id| id != 0) {
            let (Some(r), Some((player, _, _))) =
                (records.player(id), a3_core::load::any_player(records, &game.data, id))
            else {
                continue;
            };
            let age = today.map_or(0, |d| a3_core::statistics::age_on(r, d));
            rows.push(format!(
                "{}\t{}\t{}\t{age}",
                super::duties::short_name(cx, &player, id),
                super::hub::abbreviation(game, r.positions()[0]),
                r.strength(),
            ));
        }
        if let Some(list) = control(dialog, LIST).and_then(|c| view.list_mut(c)) {
            list.keys = (0..rows.len()).collect();
            list.rows = rows;
            list.inks.clear();
            list.row_icons.clear();
            list.selected = None;
            list.scroll = 0;
        }
    }
}

impl Screen for Farewells {
    fn ids(&self) -> &[u16] {
        &[FAREWELLS]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let dialog = cx.dialog;
        let Args::Farewells(notice) = args else { return };
        (self.notice, self.page) = (notice, 0);
        let year = cx.state.records.as_ref().map_or(0, |r| i32::from(r.globals.start_year()));
        let next = if (year + 1) % 100 == 0 { year + 1 } else { (year + 1) % 100 };
        let title = cx.game.text(SEASON_TEXT).replacen("%s", &year.to_string(), 1);
        set(view, dialog, SEASON, title.replacen("%s", &next.to_string(), 1));
        self.fill(cx, view);
        mark_inert(view, dialog, |t| match t {
            Target::Button(c) => dialog.controls[c].id == Some(OK),
            _ => false,
        });
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(c) if control(cx.dialog, OK) == Some(c) => {
                self.page += 1;
                if self.page < usize::from(self.notice.count) {
                    self.fill(cx, view);
                    Outcome::Stay
                } else {
                    Outcome::Close(Reply::Ok)
                }
            }
            _ => Outcome::Stay,
        }
    }
}
