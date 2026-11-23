//! A friendly booked from the year calendar (JAHRESKALENDER.md §4.3 in the
//! RE repo): the choice of its opponent (dialog 72, `CFreundAuswahl`,
//! "Gegnerauswahl für Freundschaftsspiel"). The clubs the turn offers for
//! the day ([`a3_core::friendly`]) stand in the list under its header
//! "Verein\tLiga\tOrt\tBedingung" — the name, the division, "H" when the
//! club plays at home or "A" away, the fee or "Einnahmenteilung"; a click
//! on one books the friendly (`0x4aa42a`). Below, the other human clubs
//! free that day, each with a home and an away button: at one computer the
//! colleague is asked (dialog 69, `CFreundAnfrage`), and his yes books the
//! friendly into both calendars (`FUN_004aa4bd`, `FUN_004aa5b9`).

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
    /// The other humans' clubs on the buttons, and the one being asked
    /// with whether the club would play away.
    humans: Vec<(u8, i16)>,
    asking: Option<((u8, i16), bool)>,
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
        self.humans = others.clone();
        for (k, &id) in HUMANS.iter().enumerate() {
            set(view, dialog, id, names.get(k).cloned().unwrap_or_default());
        }
        if let Some(options) = control(dialog, ASK).and_then(|c| view.options_mut(c)) {
            options.dimmed = (0..options.labels.len()).map(|item| item / 2 >= others.len()).collect();
            options.chosen = crate::controls::options::NONE;
        }
        let works = |target: Target| match target {
            Target::Button(c) => dialog.controls[c].id.is_some_and(|id| CLOSERS.contains(&id)),
            Target::Option { .. } => true,
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
            // A human's home or away button asks him (`0x4aa572`): two to a
            // row, the first the club at home.
            Target::Option { control: c, item } if dialog.controls[c].id == Some(ASK) => {
                let Some(&club) = self.humans.get(item / 2) else { return Outcome::Stay };
                let away = item % 2 == 1;
                self.asking = Some((club, away));
                let args = Args::FriendlyAsk { country: club.0, number: club.1, away, day: self.day };
                Outcome::Popup(super::Request::with(REQUEST, args))
            }
            _ => Outcome::Stay,
        }
    }

    /// The colleague's yes books the friendly, his calendar with it
    /// (`FUN_0075f812` with both).
    fn answer(&mut self, cx: &mut Cx, _: &mut View, reply: Reply) -> Outcome {
        let Some(((country, number), away)) = self.asking.take() else { return Outcome::Stay };
        if reply != Reply::Ok {
            return Outcome::Stay;
        }
        let candidate = Candidate { country, number, away: i16::from(away), ..Candidate::default() };
        self.offer.push(candidate);
        if self.book(cx, self.offer.len() - 1) { Outcome::Close(Reply::Ok) } else { Outcome::Stay }
    }
}

/// Dialog 69 (`CFreundAnfrage`, `FUN_004a99de`): the colleague asked —
/// text 1788 when the asking club plays at home, else 2176, with his name,
/// the asking manager's and his club's —, the day (1789), yes and no.
pub(crate) const REQUEST: u16 = 69;
const REQUEST_TEXT: u16 = 1660;
const REQUEST_DAY: u16 = 1661;
const REQUEST_YES: u16 = 1662;
const REQUEST_NO: u16 = 1663;
const AT_HIS_GROUND: u16 = 1788;
const AT_YOURS: u16 = 2176;
const ON_DAY: u16 = 1789;
const SHORT_DATE: u16 = 1778;
/// A manager slot nobody holds (`FUN_006a537d`).
const NOBODY: u16 = 0x7cc;

/// A human club's manager's name (`FUN_006a537d` with club +0x8b): "%s %s".
pub(crate) fn manager_name(cx: &Cx, records: &a3_core::records::Records, (country, number): (u8, i16)) -> String {
    let manager = u16::try_from(number).ok().and_then(|n| records.team(country.into(), n)).map_or(0, |t| t.humans()[1]);
    match usize::from(manager).checked_sub(1).and_then(|m| records.managers.get(m)) {
        Some(m) => format!("{} {}", m.first_name(), m.last_name()),
        None => cx.game.text(NOBODY).to_owned(),
    }
}

#[derive(Default)]
pub struct FriendlyRequest;

impl Screen for FriendlyRequest {
    fn ids(&self) -> &[u16] {
        &[REQUEST]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let dialog = cx.dialog;
        let Args::FriendlyAsk { country, number, away, day } = args else { return };
        let (Some(own), Some(records)) = (own_team(cx), cx.state.records.as_ref()) else { return };
        let own = (own.country(), i16::from(own.number()));
        let text = cx.game.text(if away { AT_YOURS } else { AT_HIS_GROUND })
            .replacen("%s", &manager_name(cx, records, (country, number)), 1)
            .replacen("%s", &manager_name(cx, records, own), 1)
            .replacen("%s", &cx.game.club_name(Some(records), own.0, own.1), 1);
        set(view, dialog, REQUEST_TEXT, text);
        let year = cx.state.season.as_ref().map_or(0, |s| s.year);
        let date = a3_core::calendar::date(year, day);
        let short = cx.game.text(SHORT_DATE)
            .replace("%1", &date.format("%-d").to_string())
            .replace("%2", &date.format("%-m").to_string());
        set(view, dialog, REQUEST_DAY, cx.game.text(ON_DAY).replacen("%s", &short, 1));
        mark_inert(view, dialog, |target| {
            matches!(target, Target::Button(c) if matches!(dialog.controls[c].id, Some(REQUEST_YES | REQUEST_NO)))
        });
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(c) if cx.dialog.controls[c].id == Some(REQUEST_YES) => Outcome::Close(Reply::Ok),
            Target::Button(c) if cx.dialog.controls[c].id == Some(REQUEST_NO) => Outcome::Close(Reply::Cancel),
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

/// The captions.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    for id in [CHOICE, REQUEST] {
        let Some(dialog) = dialogs.get_mut(&id) else { continue };
        let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
        dialog.controls.insert(0, caption);
    }
}
