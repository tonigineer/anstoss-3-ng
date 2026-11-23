//! The abilities gained and lost over the week (dialog 364,
//! `CFaehigkeitenWoche`): every man the club's book of the week holds a
//! line for, his name, his main position and the names of the first
//! ability he gained and the first he lost, the two icon slots of the
//! list's columns showing which they are (`FUN_00499fc3`).  The cycle
//! shows it for a club whose book has entries; an empty book ends the
//! dialog at once in `OnInit` and shows no row.  The header's fifty icon
//! columns the original defines over the list (`FUN_007a95f6`) are not
//! drawn; the file's header line names the columns.

use std::collections::HashMap;

use a3_files::rch::Dialog;

use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, ok_or_cancel, set};
use crate::controls::list::icon_slots;
use crate::pointer::Target;
use crate::view::View;

pub const WEEK_ABILITIES: u16 = a3_core::cycle::dialogs::WEEK_ABILITIES;
const LIST: u16 = 4598;
const CAPTION_ID: u16 = 9946;

/// The dialog keeps its title to itself; the caption bar takes it, with
/// the club's name behind it (`"%s %s"` over the title and the club).
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&WEEK_ABILITIES) else { return };
    let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}

#[derive(Default)]
pub struct WeekAbilities;

impl WeekAbilities {
    fn fill(&self, cx: &Cx, view: &mut View) {
        let dialog = cx.dialog;
        let Some(records) = cx.state.records.as_ref() else { return };
        let Some(team) = super::squad::own_team(cx) else { return };
        let slot = team.human();
        let words = &cx.game.rules.words;
        let mut rows = Vec::new();
        let mut icons: Vec<Vec<(usize, u16)>> = Vec::new();
        let mut inks = Vec::new();
        let entry_icons = |word: u16, keeper: bool, good: bool, span: usize| {
            let bit = (0..span).find(|&b| word >> b & 1 != 0);
            let name = bit
                .and_then(|b| {
                    let names =
                        if keeper { &words.keeper_abilities } else { &words.field_abilities };
                    names.get(b).map(|&id| cx.game.text(id).to_owned())
                })
                .unwrap_or_default();
            let icon = bit.and_then(|b| cx.game.rules.pitch.ability_icon(keeper, good, b));
            (name, icon)
        };
        for e in records.abilities_week(slot) {
            let Some(p) = records.player(e.id) else { continue };
            let keeper = p.positions()[0] == 1;
            let span = if keeper { 9 } else { 16 };
            let (gained, gained_icon) = entry_icons(e.gained, keeper, true, span);
            let (lost, lost_icon) = entry_icons(e.lost, keeper, false, span);
            let name = records.known_as(e.id).unwrap_or_default();
            let position = super::hub::abbreviation(cx.game, p.positions()[0]);
            rows.push([name, position, gained, lost].join("\t"));
            inks.push(None);
            icons.push(
                [(0, gained_icon), (1, lost_icon)]
                    .into_iter()
                    .filter_map(|(at, icon)| Some((at, icon?)))
                    .collect(),
            );
        }
        if let Some(c) = control(dialog, LIST) {
            let slots = icon_slots(&dialog.controls[c]);
            if let Some(list) = view.list_mut(c) {
                list.rows = rows;
                list.inks = inks;
                list.selected = None;
                list.row_icons = icons
                    .into_iter()
                    .map(|row| {
                        row.into_iter()
                            .filter_map(|(at, icon)| {
                                Some((*slots.get(at)?, icon))
                            })
                            .collect()
                    })
                    .collect();
            }
        }
    }
}

impl Screen for WeekAbilities {
    fn ids(&self) -> &[u16] {
        &[WEEK_ABILITIES]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        // The caption takes the title, the club's name behind it.
        let caption = match super::squad::own_team(cx) {
            Some(team) => format!("{} {}", dialog.title, team.name()),
            None => dialog.title.clone(),
        };
        set(view, dialog, CAPTION_ID, caption);
        // Only the OK button stands here besides the list; it closes.
        mark_inert(view, dialog, |_| true);
        self.fill(cx, view);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        if ok_or_cancel(cx.dialog, target).0 {
            return Outcome::Close(Reply::Ok);
        }
        Outcome::Stay
    }
}
