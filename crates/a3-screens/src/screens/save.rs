//! "Spielstand speichern" of the dressing room's game options (dialog 489,
//! `CDiSave`): the saved games listed by name, date and time, and a field
//! for the name to save under. A click on a row puts that row's name in the
//! field (`OnSelChange`); OK completes the name with the ".an3" of text
//! 4310 when it does not end in one already, asks — over the dialog, with
//! text 4593 — before it takes a name that is saved already, and writes the
//! records to `saves\<name>` (`FUN_0053c497`, `FUN_006ed31e`). Cancel gives
//! the save up. Both leave the dressing room standing: `FUN_007a0d38` is a
//! window primitive, not a way back to the start menu, as a run of the
//! original confirms for every answer.

use std::collections::HashMap;

pub use a3_core::cycle::dialogs::SAVE;
use a3_core::cycle::dialogs::QUESTION;
use a3_files::rch::Dialog;
use chrono::{DateTime, Local};

use super::{Args, Cx, Outcome, Reply, Request, Screen, control, mark_inert, ok_or_cancel};
use crate::pointer::Target;
use crate::view::View;

/// Controls by their ids in the dialog file.
const SAVE_LIST: u16 = 5248;
const SAVE_NAME: u16 = 1866;
/// The rows' columns: the name with its extension, the date, the time.
const LIST_TABS: [f32; 3] = [0.0, 270.0, 325.0];
/// ".an3", the dialog's extension (`FUN_0053c013`, text 4310).
const EXTENSION: u16 = 4310;
/// "Es existiert bereits ein Spielstand mit diesem Namen! Überschreiben?"
/// (`FUN_0043e93f`, text 4593).
const OVERWRITE: u16 = 4593;

/// The dialog keeps its title to itself; the caption bar takes it.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&SAVE) else { return };
    let caption = super::caption(dialog.frame, super::discipline::CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}

/// The saved games named on, and the name held while its question waits.
#[derive(Default)]
pub struct SaveGame {
    pending: Option<String>,
}

/// The records into `saves\{name}` (`FUN_006ed31e`). The dressing room
/// flushes the club's and the manager object's records first
/// (`FUN_0063dd1c`, `FUN_006a426b`); the replica keeps its records in one
/// place, so there is nothing to flush.
fn write(cx: &mut Cx, name: &str) {
    let Some(bytes) =
        cx.state.records.as_ref().and_then(|records| records.to_save().to_bytes_stored().ok())
    else {
        return;
    };
    let at = cx.game.dir.join("saves").join(name);
    if let Err(err) = std::fs::write(&at, bytes) {
        tracing::info!("saving {at:?}: {err}");
    }
}

impl Screen for SaveGame {
    fn ids(&self) -> &[u16] {
        &[SAVE]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        self.pending = None;
        let ext = cx.game.text(EXTENSION).to_owned();
        let rows = cx
            .game
            .save_games()
            .iter()
            .map(|save| {
                let saved = DateTime::<Local>::from(save.saved);
                format!(
                    "{}{ext}\t{}\t{}",
                    save.name,
                    saved.format("%d.%m.%Y"),
                    saved.format("%H:%M")
                )
            })
            .collect();
        if let Some(list) = control(cx.dialog, SAVE_LIST).and_then(|c| view.list_mut(c)) {
            // It opens with no row chosen: the field asks for a new name.
            list.selected = None;
            list.scroll = 0;
            list.rows = rows;
            list.tabs = LIST_TABS.to_vec();
        }
        if let Some(field) = control(cx.dialog, SAVE_NAME) {
            view.set_text(field, "");
            view.focus = Some(field);
        }
        mark_inert(view, cx.dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        if let Target::Cell { control: c, row, .. } | Target::Row { control: c, row } = target {
            if dialog.controls[c].id == Some(SAVE_LIST) {
                // The row's name, extension and all, into the field.
                let name = view
                    .list(c)
                    .and_then(|list| list.rows.get(row))
                    .and_then(|row| row.split('\t').next())
                    .map(str::to_owned);
                if let (Some(name), Some(field)) = (name, control(dialog, SAVE_NAME)) {
                    view.set_text(field, name);
                }
            }
            return Outcome::Stay;
        }
        let (ok, cancel) = ok_or_cancel(dialog, target);
        if cancel {
            return Outcome::Close(Reply::Cancel);
        }
        if !ok {
            return Outcome::Stay;
        }
        let Some(field) = control(dialog, SAVE_NAME) else { return Outcome::Stay };
        let mut name = view.text(field).to_owned();
        if name.is_empty() {
            return Outcome::Stay;
        }
        // Too short to hold an extension at all, or not ".an3" at its end,
        // lower-case alike (`FUN_0053c497`).
        let ext = cx.game.text(EXTENSION);
        if name.len() < ext.len() + 1 || !name.to_lowercase().ends_with(ext) {
            name.push_str(ext);
        }
        let at = cx.game.dir.join("saves").join(&name);
        if std::fs::exists(&at).unwrap_or(false) {
            self.pending = Some(name);
            return Outcome::Popup(Request::with(QUESTION, Args::Text(OVERWRITE)));
        }
        write(cx, &name);
        Outcome::Close(Reply::Ok)
    }

    fn answer(&mut self, cx: &mut Cx, _: &mut View, reply: Reply) -> Outcome {
        if reply != Reply::Ok {
            // "Abbruch": the question goes away, the dialog stands with
            // the name still in its field.
            return Outcome::Stay;
        }
        let Some(name) = self.pending.take() else { return Outcome::Stay };
        write(cx, &name);
        Outcome::Close(Reply::Ok)
    }
}
