//! A tournament booked from the year calendar (JAHRESKALENDER.md §4.3 in
//! the RE repo; [`a3_core::tournament`]).
//!
//! Dialog 296 ("Turnier auswählen", `CTurnierArtAuswaehlen` and for the
//! hall and the beach `CHallenturnierArtWaehlen`) offers to host one's own
//! — for a hall only while the club hosts none yet — and the invitations
//! of the computer's clubs whose tournament of that kind starts on the day
//! and has room, five rows at most, each with the host's crest. Hosting
//! opens dialog 45 ("Eigenes Turnier veranstalten",
//! `CEigenesTurnierVeranstalten`): the clubs the turn offers for the day,
//! the friendly's, are invited one click each into the list of those in,
//! with what each costs; its OK takes the tournament once it has its four
//! or eight. Inviting another human (dialog 297) is not built.

use std::collections::HashMap;

use a3_core::friendly::Candidate;
use a3_core::records::Records;
use a3_core::tournament::{self, Short, TURNIER};
use a3_files::rch::Dialog;

use super::discipline::CAPTION_ID;
use super::playerinfo::money;
use super::squad::own_team;
use super::{Args, Cx, Outcome, Reply, Request, Screen, control, mark_inert};
use crate::controls::bitmap::Picture;
use crate::game::Game;
use crate::pointer::Target;
use crate::view::View;

/// Dialog 296: its rows, their crests, its cancel buttons.
pub(crate) const CHOICE: u16 = 296;
const ROWS: u16 = 3984;
const CRESTS: [u16; 5] = [3979, 3980, 3981, 3982, 3983];
const CHOICE_CLOSERS: [u16; 2] = [2, 3989];
/// "Einladung %s annehmen".
const INVITATION: u16 = 1625;
/// The row that hosts one's own.
const OWN: i16 = -10;
/// A club hosts a Hallenturnier (club +0xf7, team +0x125).
const HOSTS_HALL: u8 = 2;

/// Dialog 45: those in, those offered, the other humans' buttons, OK and
/// its cancels.
pub(crate) const OWN_TOURNAMENT: u16 = 45;
const IN: u16 = 1400;
const OFFERED: u16 = 1401;
const HUMANS: u16 = 1405;
const OK: u16 = 1;
const OWN_CLOSERS: [u16; 2] = [2, 1410];
/// "Sie haben zu wenig Vereine eingeladen!", "%d. Liga".
const TOO_FEW: u16 = 3082;
const NTH_LEAGUE: u16 = 4167;

/// The manager's club: its country and number, and his number from 1.
fn own_club(cx: &Cx) -> Option<((u8, i16), u8)> {
    let team = own_team(cx)?;
    Some(((team.country(), i16::from(team.number())), team.humans()[0]))
}

/// A club's crest as the dialog's bitmaps show it (`FUN_00696161`).
fn crest(game: &Game, records: &Records, (country, number): (u8, i16)) -> Option<Picture> {
    let slot = game.world.slot_of(country.into())?;
    let file = game.rules.crest(&game.world, Some(records), (slot, usize::try_from(number).ok()?))?;
    Some(Picture::file(file))
}

#[derive(Default)]
pub struct TournamentChoice {
    day: usize,
    kind: u8,
    /// What each row stands for: hosting one's own, or the tournament to
    /// join by its id.
    rows: Vec<(i16, u32)>,
    /// The own tournament while dialog 45 is open.
    hosting: Option<u32>,
}

impl Screen for TournamentChoice {
    fn ids(&self) -> &[u16] {
        &[CHOICE]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let dialog = cx.dialog;
        *self = Self::default();
        if let Args::TournamentDay { day, kind } = args {
            (self.day, self.kind) = (day, kind);
        }
        let (Some((club, _)), Some(records)) = (own_club(cx), cx.state.records.as_ref()) else { return };
        let team = own_team(cx);
        // A hall's or a beach's own only while the club hosts no hall yet
        // (`FUN_004c24dc`).
        let own = self.kind == TURNIER || team.is_some_and(|t| t.get::<u8>(0x125) & HOSTS_HALL == 0);
        let mut labels = Vec::new();
        let mut crests = Vec::new();
        if own {
            self.rows.push((OWN, 0));
            labels.push(None);
            crests.push(crest(cx.game, records, club));
        }
        for (i, t) in tournament::on_day(records, self.day as i16, self.kind).iter().enumerate() {
            if self.rows.len() >= CRESTS.len() {
                break;
            }
            let host = t.host();
            if t.full() || a3_core::statistics::is_human(records, host.0, host.1) {
                continue;
            }
            self.rows.push((i as i16, t.id()));
            let name = cx.game.club_name(Some(records), host.0, host.1);
            labels.push(Some(cx.game.text(INVITATION).replacen("%s", &name, 1)));
            crests.push(crest(cx.game, records, host));
        }
        let shown = self.rows.len();
        if let Some(options) = control(dialog, ROWS).and_then(|c| view.options_mut(c)) {
            for row in 0..options.labels.len() {
                match labels.get(row) {
                    Some(Some(label)) => options.labels[row] = label.clone(),
                    Some(None) => {}
                    None => options.labels[row] = String::new(),
                }
            }
            options.dimmed = (0..options.labels.len()).map(|row| row >= shown).collect();
            options.chosen = crate::controls::options::NONE;
        }
        for (row, &id) in CRESTS.iter().enumerate() {
            if let Some(c) = control(dialog, id) {
                view.set_picture(c, crests.get(row).cloned().flatten());
            }
        }
        let works = |target: Target| match target {
            Target::Button(c) => dialog.controls[c].id.is_some_and(|id| CHOICE_CLOSERS.contains(&id)),
            Target::Option { control: c, .. } => dialog.controls[c].id == Some(ROWS),
            _ => false,
        };
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            Target::Button(c) if dialog.controls[c].id.is_some_and(|id| CHOICE_CLOSERS.contains(&id)) => {
                Outcome::Close(Reply::Cancel)
            }
            Target::Option { control: c, item } if dialog.controls[c].id == Some(ROWS) => {
                let Some(&(row, id)) = self.rows.get(item) else { return Outcome::Stay };
                let Some((club, _)) = own_club(cx) else { return Outcome::Stay };
                let Some(records) = cx.state.records.as_mut() else { return Outcome::Stay };
                let day = self.day as i16;
                if row == OWN {
                    // The own one stands in the list from here (`FUN_00760d7a`).
                    let Some(id) = tournament::open_own(records, club, day, self.kind) else {
                        return Outcome::Stay;
                    };
                    self.hosting = Some(id);
                    let args = Args::OwnTournament { id, day: self.day, kind: self.kind };
                    return Outcome::Popup(Request::with(OWN_TOURNAMENT, args));
                }
                if tournament::join(records, club, id) {
                    self.booked(cx, id);
                    Outcome::Close(Reply::Ok)
                } else {
                    Outcome::Close(Reply::Cancel)
                }
            }
            _ => Outcome::Stay,
        }
    }

    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        let Some(id) = self.hosting.take() else { return Outcome::Stay };
        if reply == Reply::Ok {
            self.booked(cx, id);
            return Outcome::Close(Reply::Ok);
        }
        // Not taken: the own one leaves the list again (`FUN_005ae3e0`);
        // the hall's only while nobody joined it (`FUN_004c2a62`).
        if let Some(records) = cx.state.records.as_mut() {
            let alone = tournament::find(records, id).is_none_or(|t| t.count() <= 1);
            if self.kind == TURNIER || alone {
                tournament::remove(records, id);
            }
        }
        let _ = view;
        Outcome::Stay
    }
}

impl TournamentChoice {
    /// The calendar's two days of the tournament booked (the year
    /// calendar's `FUN_0072ede4` with its host).
    fn booked(&self, cx: &mut Cx, id: u32) {
        let slot = own_team(cx).and_then(|t| usize::from(t.human()).checked_sub(1));
        let Some(records) = cx.state.records.as_mut() else { return };
        let Some(t) = tournament::find(records, id) else { return };
        if let Some(calendar) = slot.and_then(|s| records.calendars.get_mut(s)) {
            tournament::mark(calendar, &t);
        }
    }
}

#[derive(Default)]
pub struct OwnTournament {
    id: u32,
    day: usize,
    kind: u8,
    /// The clubs the turn offers, and the guests invited from them.
    offered: Vec<Candidate>,
    guests: Vec<(u8, i16)>,
    /// The other humans free on both days (`FUN_00488f2a`).
    humans: Vec<(u8, i16)>,
}

impl Screen for OwnTournament {
    fn ids(&self) -> &[u16] {
        &[OWN_TOURNAMENT]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let dialog = cx.dialog;
        *self = Self::default();
        if let Args::OwnTournament { id, day, kind } = args {
            (self.id, self.day, self.kind) = (id, day, kind);
        }
        let Some((club, _)) = own_club(cx) else { return };
        let day = self.day as i16;
        let calendar = &cx.game.calendar;
        let state = &mut *cx.state;
        if let (Some(records), Some(season)) = (state.records.as_ref(), state.season.as_mut()) {
            self.offered = season.opponents.offer(records, calendar, club, day, &mut season.random);
        }
        let Some(records) = cx.state.records.as_ref() else { return };
        let t = tournament::find(records, self.id);
        self.humans = a3_core::statistics::human_clubs(records)
            .into_iter()
            .filter(|&h| h != club)
            .filter(|&h| (day..=day + 1).all(|d| !a3_core::friendly::taken(records, calendar, h, d)))
            .filter(|&h| !t.is_some_and(|t| t.has(h)))
            .collect();
        let names: Vec<String> =
            self.humans.iter().map(|&(country, number)| cx.game.club_name(Some(records), country, number)).collect();
        if let Some(options) = control(dialog, HUMANS).and_then(|c| view.options_mut(c)) {
            // A row without a human stands empty, not with the file's
            // "Spieler" label.
            for (row, label) in options.labels.iter_mut().enumerate() {
                *label = names.get(row).cloned().unwrap_or_default();
            }
            options.dimmed = (0..options.labels.len()).map(|row| row >= names.len()).collect();
            options.chosen = crate::controls::options::NONE;
        }
        self.fill(cx, view);
        let humans = self.humans.len();
        let works = |target: Target| match target {
            Target::Button(c) => dialog.controls[c].id.is_some_and(|id| id == OK || OWN_CLOSERS.contains(&id)),
            // A disabled row answers nothing as it should; the humans'
            // own are crossed out.
            Target::Option { item, .. } => item >= humans,
            _ => false,
        };
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            Target::Button(c) if dialog.controls[c].id.is_some_and(|id| OWN_CLOSERS.contains(&id)) => {
                Outcome::Close(Reply::Cancel)
            }
            Target::Button(c) if dialog.controls[c].id == Some(OK) => {
                let Some(records) = cx.state.records.as_mut() else { return Outcome::Stay };
                if tournament::find(records, self.id).is_none() {
                    return Outcome::Close(Reply::Cancel);
                }
                match tournament::invite(records, self.id, &self.guests) {
                    Ok(()) => Outcome::Close(Reply::Ok),
                    Err(Short::Taken | Short::NoRoom | Short::TooFew) => {
                        Outcome::Popup(Request::with(a3_core::cycle::dialogs::NOTICE, Args::Text(TOO_FEW)))
                    }
                }
            }
            // A click on an offered club invites it (`LBN_SELCHANGE` 1401)
            // while the tournament lacks clubs.
            Target::Row { control: c, row } | Target::Cell { control: c, row, .. }
                if dialog.controls[c].id == Some(OFFERED) =>
            {
                let Some(i) = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied()) else {
                    return Outcome::Stay;
                };
                let Some(club) = self.offered.get(i).map(Candidate::club) else { return Outcome::Stay };
                if self.guests.len() + 1 != tournament::needed(self.kind) && !self.guests.contains(&club) {
                    self.guests.push(club);
                    self.fill(cx, view);
                }
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}

impl OwnTournament {
    /// The two lists (`FUN_0048932d`, `FUN_004894f5`): those invited, and
    /// the offered clubs not invited yet, each with its country, league
    /// and what it costs.
    fn fill(&self, cx: &Cx, view: &mut View) {
        let dialog = cx.dialog;
        let Some(records) = cx.state.records.as_ref() else { return };
        let Some((host, _)) = own_club(cx) else { return };
        let row = |club: (u8, i16)| {
            let (country, number) = club;
            let name = cx.game.club_name(Some(records), country, number);
            let letters = cx.game.world.nation_codes.get(usize::from(country)).cloned().unwrap_or_default();
            let fee = money(cx.game, i64::from(tournament::fee(records, host, club)));
            format!("{name}\t{letters}\t{}\t{fee}", league(cx.game, records, club))
        };
        let invited: Vec<String> = self.guests.iter().map(|&g| row(g)).collect();
        let offered: Vec<(usize, String)> = self
            .offered
            .iter()
            .enumerate()
            .filter(|(_, c)| !self.guests.contains(&c.club()))
            .map(|(i, c)| (i, row(c.club())))
            .collect();
        if let Some(list) = control(dialog, IN).and_then(|c| view.list_mut(c)) {
            list.keys = vec![0; invited.len()];
            list.rows = invited;
            list.selected = None;
        }
        if let Some(list) = control(dialog, OFFERED).and_then(|c| view.list_mut(c)) {
            list.keys = offered.iter().map(|&(i, _)| i).collect();
            list.rows = offered.into_iter().map(|(_, r)| r).collect();
            list.selected = None;
        }
    }
}

/// A club's league as the list writes it (`FUN_00635968` with
/// `FUN_0063e388`): its name in a played country, else "%d. Liga".
fn league(game: &Game, records: &Records, (country, number): (u8, i16)) -> String {
    let Some(team) = u16::try_from(number).ok().and_then(|n| records.team(country.into(), n)) else {
        return game.text(NTH_LEAGUE).replacen("%d", "1", 1);
    };
    let place = super::search::league_place(records, team);
    if records.globals.played(country.into()) {
        records.countries.league_name(country.into(), place)
    } else {
        game.text(NTH_LEAGUE).replacen("%d", &(place + 1).to_string(), 1)
    }
}

/// The captions.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    for id in [CHOICE, OWN_TOURNAMENT] {
        let Some(dialog) = dialogs.get_mut(&id) else { continue };
        let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
        dialog.controls.insert(0, caption);
    }
}
