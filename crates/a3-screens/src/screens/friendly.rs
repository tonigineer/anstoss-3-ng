//! A friendly booked from the year calendar (JAHRESKALENDER.md §4.3 in the
//! RE repo): the choice of its opponent (dialog 72, `CFreundAuswahl`,
//! "Gegnerauswahl für Freundschaftsspiel"). The clubs the turn offers for
//! the day ([`a3_core::friendly`]) stand in the list under its header
//! "Verein\tLiga\tOrt\tBedingung" — the name, the division, "H" when the
//! club plays at home or "A" away, the fee or "Einnahmenteilung"; a click
//! on one books the friendly (`0x4aa42a`). Below, the other human clubs
//! free that day, each with a home and an away button; asking them is not
//! built yet.

use std::collections::HashMap;

use a3_core::friendly::{self, Candidate};
use a3_files::rch::Dialog;

use super::discipline::CAPTION_ID;
use super::playerinfo::money;
use super::squad::own_team;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::pointer::Target;
use crate::view::View;

/// The dialog, and its two buttons that close it.
pub(crate) const CHOICE: u16 = 72;
const CLOSERS: [u16; 2] = [1, 1679];
/// The offered clubs, the other humans' names and their buttons (home and
/// away for each, two to a row).
const OFFER: u16 = 1666;
const HUMANS: [u16; 3] = [1667, 1668, 1669];
const ASK: u16 = 1675;
/// "A", "H", "Einnahmenteilung".
const AWAY: u16 = 1786;
const HOME: u16 = 1787;
const GATE_SHARED: u16 = 1772;
/// The fee's unit (`DAT_00a3b3e4`).
const THOUSAND: i64 = 1000;
/// The lowest division the column shows, from 0.
const LOWEST_SHOWN: i16 = 2;

#[derive(Default)]
pub struct FriendlyChoice {
    /// The day the friendly is for.
    day: usize,
    /// The clubs offered, row by row.
    offer: Vec<Candidate>,
}

impl Screen for FriendlyChoice {
    fn ids(&self) -> &[u16] {
        &[CHOICE]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let dialog = cx.dialog;
        *self = Self::default();
        if let Args::FriendlyDay(day) = args {
            self.day = day;
        }
        let day = self.day as i16;
        let club = own_team(cx).map(|t| (t.country(), i16::from(t.number())));
        let calendar = &cx.game.calendar;
        let state = &mut *cx.state;
        if let (Some(club), Some(records), Some(season)) = (club, state.records.as_ref(), state.season.as_mut()) {
            self.offer = season.opponents.offer(records, calendar, club, day, &mut season.random);
        }
        let Some(records) = cx.state.records.as_ref() else { return };
        let rows: Vec<String> = self
            .offer
            .iter()
            .map(|c| {
                let division = friendly::division(records, calendar, c.club()).min(LOWEST_SHOWN) + 1;
                let ground = cx.game.text(if c.away != 0 { AWAY } else { HOME });
                let fee = if c.fee != 0 {
                    money(cx.game, i64::from(c.fee) * THOUSAND)
                } else {
                    cx.game.text(GATE_SHARED).to_owned()
                };
                format!("{}\t{division}\t{ground}\t{fee}", cx.game.club_name(Some(records), c.country, c.number))
            })
            .collect();
        if let Some(list) = control(dialog, OFFER).and_then(|c| view.list_mut(c)) {
            list.keys = (0..rows.len()).collect();
            list.rows = rows;
            list.selected = None;
        }
        // The other humans free that day (`FUN_006355f4`, `FUN_0062ae6a`):
        // their names, and their buttons; those of rows nobody fills are
        // disabled (`FUN_007a72c2`).
        let others: Vec<(u8, i16)> = a3_core::statistics::human_clubs(records)
            .into_iter()
            .filter(|&h| Some(h) != club && !friendly::taken(records, calendar, h, day))
            .take(HUMANS.len())
            .collect();
        let names: Vec<String> =
            others.iter().map(|&(country, number)| cx.game.club_name(Some(records), country, number)).collect();
        for (k, &id) in HUMANS.iter().enumerate() {
            set(view, dialog, id, names.get(k).cloned().unwrap_or_default());
        }
        if let Some(options) = control(dialog, ASK).and_then(|c| view.options_mut(c)) {
            options.dimmed = (0..options.labels.len()).map(|item| item / 2 >= others.len()).collect();
            options.chosen = crate::controls::options::NONE;
        }
        // A disabled button answers nothing as it should; only the
        // humans' own are crossed out.
        let works = |target: Target| match target {
            Target::Button(c) => dialog.controls[c].id.is_some_and(|id| CLOSERS.contains(&id)),
            Target::Option { item, .. } => item / 2 >= others.len(),
            _ => false,
        };
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            Target::Button(c) if dialog.controls[c].id.is_some_and(|id| CLOSERS.contains(&id)) => {
                Outcome::Close(Reply::Cancel)
            }
            Target::Row { control: c, row } | Target::Cell { control: c, row, .. }
                if dialog.controls[c].id == Some(OFFER) =>
            {
                let Some(i) = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied()) else {
                    return Outcome::Stay;
                };
                if self.book(cx, i) { Outcome::Close(Reply::Ok) } else { Outcome::Stay }
            }
            _ => Outcome::Stay,
        }
    }
}

impl FriendlyChoice {
    /// Books the friendly with the row's club (`FUN_0075f812`); the year
    /// calendar then writes the opponent into its day (`FUN_0072ede4`).
    fn book(&self, cx: &mut Cx, i: usize) -> bool {
        let Some(candidate) = self.offer.get(i) else { return false };
        let Some(team) = own_team(cx) else { return false };
        let club = (team.country(), i16::from(team.number()));
        let (manager, slot) = (team.humans()[0], usize::from(team.human()).checked_sub(1));
        let Some(records) = cx.state.records.as_mut() else { return false };
        if !friendly::book(records, club, manager, candidate, self.day as i16) {
            return false;
        }
        if let Some(calendar) = slot.and_then(|s| records.calendars.get_mut(s)) {
            a3_core::year::book_friendly(calendar, self.day, candidate.club());
        }
        true
    }
}

/// The caption.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&CHOICE) else { return };
    let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}
