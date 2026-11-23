//! A nation's offer of its team at an even year's turn (dialog 356,
//! `CNationaltrainerWerden`, "Verhandlung Nationalmannschaft", event 0xc2
//! of `FUN_006b9c81`; NATIONALMANNSCHAFT.md §7.2). The list of
//! interested nations (4543) holds the ones not dropped; the chosen
//! one's name (4551), salary (4547) and bonus per round (4548) stand
//! beside it (`FUN_00520799`). OK (`0x520261`) takes it unless another
//! coach has it — then it drops out of the list (alert 5321 in the
//! original, not built) and the next stands chosen, none left closing
//! the dialog. Abbruch takes nothing. The bargaining ("Verhandeln"),
//! the squad's view ("Spielerkader", `0x520905`) and the morph are not
//! built.

use std::collections::HashMap;

use a3_core::national::{self, Terms};
use a3_files::rch::Dialog;

use super::discipline::CAPTION_ID;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::pointer::Target;
use crate::view::View;

pub const NATIONAL_OFFER: u16 = a3_core::cycle::dialogs::NATIONAL_OFFER;
const CANCEL: u16 = 4542;
const LIST: u16 = 4543;
const OK: u16 = 4546;
const SALARY: u16 = 4547;
const BONUS: u16 = 4548;
const NATION: u16 = 4551;
/// The national team's number of a nation (`FUN_006b9755`'s 200).
const NATIONAL_TEAM: i16 = 200;

/// The dialog's title goes to the caption bar, the manager's name
/// behind it as it opens.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&NATIONAL_OFFER) else { return };
    let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}

#[derive(Default)]
pub struct NationalOffer {
    /// The rows' nations and first terms (+0x1b98, +0x1bf4, +0x1c30).
    terms: Vec<Terms>,
    /// The nations dropped from the list (+0x1b5c).
    dropped: Vec<bool>,
    /// The chosen row among all of them (+0x1c72).
    chosen: Option<usize>,
}

impl NationalOffer {
    /// The list anew (`FUN_0052013a`): a row for each nation not dropped,
    /// its key the nation's place; with `rechoose` the first of them is
    /// chosen.
    fn fill(&mut self, cx: &Cx, view: &mut View, rechoose: bool) {
        let (game, dialog) = (cx.game, cx.dialog);
        let records = cx.state.records.as_ref();
        if rechoose {
            self.chosen = (0..self.terms.len()).find(|&k| !self.dropped[k]);
        }
        let (mut rows, mut keys) = (Vec::new(), Vec::new());
        for (k, t) in self.terms.iter().enumerate().filter(|&(k, _)| !self.dropped[k]) {
            rows.push(game.club_name(records, t.nation as u8, NATIONAL_TEAM));
            keys.push(k);
        }
        let selected = self.chosen.and_then(|k| keys.iter().position(|&r| r == k));
        if let Some(list) = control(dialog, LIST).and_then(|c| view.list_mut(c)) {
            list.rows = rows;
            list.keys = keys;
            list.inks.clear();
            list.row_icons.clear();
            list.selected = selected;
            list.scroll = 0;
        }
        self.show(cx, view);
    }

    /// The chosen nation's name and terms (`FUN_00520799`).
    fn show(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(t) = self.chosen.and_then(|k| self.terms.get(k)) else { return };
        set(view, dialog, NATION, game.club_name(cx.state.records.as_ref(), t.nation as u8, NATIONAL_TEAM));
        set(view, dialog, SALARY, super::finances::money32(game, t.salary as u32));
        set(view, dialog, BONUS, super::finances::money32(game, t.bonus as u32));
    }
}

impl Screen for NationalOffer {
    fn ids(&self) -> &[u16] {
        &[NATIONAL_OFFER]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let dialog = cx.dialog;
        self.terms = match args {
            Args::NationalOffers { terms, count } => terms[..usize::from(count)].to_vec(),
            _ => Vec::new(),
        };
        self.dropped = vec![false; self.terms.len()];
        // "%s %s %s": the title, the manager's first and last name.
        let name = cx.state.records.as_ref().zip(cx.state.season.as_ref()).and_then(|(r, s)| {
            let m = r.managers.get(r.human_slot(s.current)?)?;
            Some(format!("{} {}", m.first_name(), m.last_name()))
        });
        set(view, dialog, CAPTION_ID, format!("{} {}", dialog.title, name.unwrap_or_default()));
        self.fill(cx, view, true);
        mark_inert(view, dialog, |t| match t {
            Target::Button(c) => matches!(dialog.controls[c].id, Some(OK | CANCEL)),
            Target::Row { control: c, .. } => dialog.controls[c].id == Some(LIST),
            _ => false,
        });
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            Target::Row { control: c, row } if dialog.controls[c].id == Some(LIST) => {
                if let Some(k) = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied()) {
                    self.chosen = Some(k);
                    if let Some(list) = view.list_mut(c) {
                        list.selected = Some(list.scroll + row);
                    }
                    self.show(cx, view);
                }
                Outcome::Stay
            }
            Target::Button(c) if dialog.controls[c].id == Some(CANCEL) => Outcome::Close(Reply::Cancel),
            Target::Button(c) if dialog.controls[c].id == Some(OK) => {
                let Some(k) = self.chosen else { return Outcome::Stay };
                let taken = cx
                    .state
                    .records
                    .as_ref()
                    .is_some_and(|r| national::coach_number(r, self.terms[k].nation) != 0);
                if !taken {
                    return Outcome::Close(Reply::Choice(k));
                }
                // Another coach has it: it drops out, the next stands
                // chosen, none left closes the dialog.
                self.dropped[k] = true;
                self.fill(cx, view, true);
                if self.chosen.is_none() { Outcome::Close(Reply::Cancel) } else { Outcome::Stay }
            }
            _ => Outcome::Stay,
        }
    }
}
