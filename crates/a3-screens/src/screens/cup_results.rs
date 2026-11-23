//! The cup results after the matchday (dialog 161, `CPokalNachSpieltag`):
//! the day's national cup rounds, then on OK the rounds drawn behind them
//! (`OnClicked_IDC_POKAL_NACH_OK`; it opens on the draw where no result
//! stands, `FUN_0052c679`). The page lists its cups (`FUN_0052c84e`), "Nat.
//! Pokal" with the country's letters each, and the bar below steps
//! through them (`OnHScroll`, `FUN_00531b69`), the cup's name beside it.
//! A cup's round (`FUN_0052eb17`) names itself in the caption — the
//! page's title, the round, "Nationaler Pokal" and the country, the
//! final's title cut to "ANSTOSS 3 -" — and lists its pairings: a blue
//! line "%s gegen %s" of the two sides' leagues wherever their classes
//! change (the table at `0x876ed0`), an empty one between, and each tie
//! with both clubs, their leagues ("%s Liga", "Amat."), the legs played
//! and "n.V." or "n.E." (`FUN_0052e430`), red where a human's club plays
//! (`FUN_0052c252`). The day's cups are [`a3_core::cup_results`].
//!
//! Not built: the draw's animation the draw page opens for each cup once
//! with the graphic option on (dialog 338, `CDiAuslosung`, `DAT_00b5d149`),
//! and the other competitions the original lists before and after the
//! national cups.

use std::collections::HashMap;

use a3_core::cup_results::{CupDay, FINAL_ROUND, SLOTS};
use a3_core::records::Records;
use a3_core::statistics::{self, CupPairing};
use a3_files::rch::Dialog;

use super::discipline::CAPTION_ID;
use super::youth_tables::set_columns;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::controls::slider;
use crate::game::Game;
use crate::pointer::Target;
use crate::view::View;

pub const CUP_RESULTS: u16 = a3_core::cycle::dialogs::CUP_RESULTS;
const OK: u16 = 2762;
const LIST: u16 = 2763;
const BAR: u16 = 4778;
const CUP_NAME: u16 = 5656;
/// IDS_POKAL_NACH_ERGEBNISSE, _AUSLOSUNG, and the final's
/// IDS_POKAL_NACH_ANSTOSS2.
const RESULTS_TITLE: u16 = 0xb4c;
const DRAW_TITLE: u16 = 0xb43;
const FINAL_TITLE: u16 = 0xe52;
/// IDS_POKAL_NACH_NATPOKAL in the caption, IDS_POKALNACH_NATPOKAL beside
/// the bar with the country's letters ("%s %s", the table at `0xae5768`).
const NATIONAL_CUP: u16 = 0xb52;
const CUP_LABEL: u16 = 0x616;
/// The rounds' names by their number (`0x886198`): Runde 1 for 0 and 1,
/// Runde 2, Achtelfinale, Viertelfinale, Halbfinale, Finale.
const ROUND_NAMES: [u16; 8] = [0xb45, 0xb45, 0xb46, 0xb47, 0xb48, 0xb49, 0xb4a, 0xb4a];
/// "%s Liga" with the class's ordinal, "Amat." from class 10 on.
const WHICH_LEAGUE: u16 = 0xc45;
const AMATEURS: u16 = 0xb41;
const AMATEUR_CLASS: i32 = 10;
/// "%s gegen %s", the lower league's name first (`0xae636e`).
const LEAGUE_AGAINST: u16 = 0x1048;
const EXTRA_TIME: u16 = 0x761;
const PENALTIES: u16 = 0x762;
/// Which pair of classes a line of ties belongs to, by the two sides'
/// classes up to 4 (`0x876ed0`).
const PAIRS: [[u8; 5]; 5] =
    [[0, 1, 2, 3, 4], [1, 5, 6, 7, 8], [2, 6, 9, 10, 11], [3, 7, 10, 12, 13], [4, 8, 11, 13, 14]];
const CLASSES: i32 = 4;
/// The list's eight columns (`FUN_007a941f` with `0x886230` and
/// `0x886250`): the home side, the away side, each leg's goals right in
/// twenty pixels before their ":", the note.
const COLUMNS: [(u16, u16); 8] =
    [(50, 0), (220, 0), (0x8190, 20), (420, 0), (0x81c2, 20), (470, 0), (490, 0), (550, 0)];
/// The classes' line is one column of its own from the first column's
/// start (`FUN_007a94db` with one tab of `0x886270`).
const PAIR_LEFT: f32 = 50.0;
/// A human's tie red (0xff), the classes' line blue (0xff0000).
const OWN_INK: [u8; 3] = [255, 0, 0];
const PAIR_INK: [u8; 3] = [0, 0, 255];

/// The dialog keeps its title to itself; the caption bar takes it.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&CUP_RESULTS) else { return };
    let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}

#[derive(Default)]
pub struct CupResults {
    day: CupDay,
    /// The draw's page (+0xf94), else the results'.
    draw: bool,
    /// The page's cups by their slots (+0x101c, kinds 100 on).
    cups: Vec<usize>,
    /// The page's title (+0x1010), which a final overwrites.
    title: u16,
}

impl CupResults {
    /// The page (`FUN_0052c84e`): its cups by slot, the bar over them,
    /// the first shown.
    fn fill(&mut self, cx: &Cx, view: &mut View) {
        let dialog = cx.dialog;
        self.title = if self.draw { DRAW_TITLE } else { RESULTS_TITLE };
        let rounds = if self.draw { &self.day.draws } else { &self.day.results };
        self.cups = (0..SLOTS).filter(|&s| rounds[s] >= 0).collect();
        if let Some(c) = control(dialog, BAR) {
            let n = self.cups.len() as i32;
            let handle = slider::handle_for(&dialog.controls[c], n);
            if let Some(value) = view.value_mut(c) {
                value.range = if n < 1 { (5, 5) } else { (0, n - 1) };
                value.value = value.range.0;
                value.handle = Some(handle);
            }
            view.controls[c].hidden = n < 1;
        }
        self.show(cx, view, 0);
    }

    /// The bar's cup: its name beside the bar, its round in the list.
    fn show(&mut self, cx: &Cx, view: &mut View, at: usize) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return };
        let Some(&slot) = self.cups.get(at) else { return };
        // The slot's country (`DAT_00ae5d8e`, from the main one's).
        let country = usize::try_from(records.countries.slot_country(slot + 1)).unwrap_or(0);
        let letters = game.world.nation_codes.get(country).cloned().unwrap_or_default();
        set(view, dialog, CUP_NAME, format!("{} {letters}", game.text(CUP_LABEL)));
        let rounds = if self.draw { &self.day.draws } else { &self.day.results };
        let round = rounds[slot];
        if round >= FINAL_ROUND {
            self.title = FINAL_TITLE;
        }
        let name = ROUND_NAMES.get(usize::try_from(round).unwrap_or(0)).map_or("", |&t| game.text(t));
        let caption = format!(
            "{} {name} {} {}",
            game.text(self.title),
            game.text(NATIONAL_CUP),
            game.world.nation(country as u16)
        );
        set(view, dialog, CAPTION_ID, caption);
        let Some(c) = control(dialog, LIST) else { return };
        set_columns(view, c, &COLUMNS);
        let (rows, inks, spans) = rows(game, records, slot, country, round);
        if let Some(list) = view.list_mut(c) {
            list.span_left = spans.iter().map(|&s| if s { PAIR_LEFT } else { 0.0 }).collect();
            list.rows = rows;
            list.inks = inks;
            list.spans = spans;
            list.selected = None;
            list.scroll = 0;
        }
    }
}

/// The round's lines (`FUN_0052eb17`): the ties in their order, a line of
/// their classes before each new pair of them, an empty one between.
fn rows(
    game: &Game,
    records: &Records,
    slot: usize,
    country: usize,
    round: i16,
) -> (Vec<String>, Vec<Option<[u8; 3]>>, Vec<bool>) {
    let (mut rows, mut inks, mut spans) = (Vec::new(), Vec::new(), Vec::new());
    let mut pair = None;
    for p in statistics::cup_pairings(records, slot, round) {
        let [home, away] = [(p.home_country, p.home), (p.away_country, p.away)]
            .map(|(c, n)| class(records, c, n).min(CLASSES) as usize);
        let now = PAIRS[home][away];
        if pair != Some(now) {
            if pair.is_some() {
                rows.push(String::new());
                inks.push(None);
                spans.push(false);
            }
            let name = |place: usize| records.countries.league_name(country, place);
            let line = game
                .text(LEAGUE_AGAINST)
                .replacen("%s", &name(home.min(away)), 1)
                .replacen("%s", &name(home.max(away)), 1);
            rows.push(line);
            inks.push(Some(PAIR_INK));
            spans.push(true);
            pair = Some(now);
        }
        rows.push(tie(game, records, &p));
        let human = |c: u8, n: i16| {
            u16::try_from(n).ok().and_then(|n| records.team(c.into(), n)).is_some_and(|t| t.human() > 0)
        };
        let own = human(p.home_country, p.home) || human(p.away_country, p.away);
        inks.push(own.then_some(OWN_INK));
        spans.push(false);
    }
    (rows, inks, spans)
}

/// A tie as `FUN_0052e430` writes it: "%s (%s)\t- %s (%s)\t%s\t%s\t%s",
/// each leg "%d\t:%d" where it stands, else a tab.
fn tie(game: &Game, records: &Records, p: &CupPairing) -> String {
    let league = |c: u8, n: i16| {
        let class = class(records, c, n);
        if class < AMATEUR_CLASS {
            game.text(WHICH_LEAGUE).replacen("%s", &format!("{}.", class + 1), 1)
        } else {
            game.text(AMATEURS).to_owned()
        }
    };
    let leg = |leg: Option<(u8, u8)>| leg.map_or_else(|| "\t".to_owned(), |(a, b)| format!("{a}\t:{b}"));
    let note = if p.penalties {
        game.text(PENALTIES)
    } else if p.extra_time {
        game.text(EXTRA_TIME)
    } else {
        ""
    };
    format!(
        "{} ({})\t- {} ({})\t{}\t{}\t{note}",
        game.club_name(Some(records), p.home_country, p.home),
        league(p.home_country, p.home),
        game.club_name(Some(records), p.away_country, p.away),
        league(p.away_country, p.away),
        leg(p.first),
        leg(p.second)
    )
}

/// A club's class (`FUN_0063e388`): in a country of the first kind by its
/// level as `FUN_006755b8` counts it — the main country's to the third
/// (England's the fourth), every other's the first, no league one deeper
/// —, an amateur club after the teams 10; in one of the second kind its
/// level; else 0.
fn class(records: &Records, country: u8, number: i16) -> i32 {
    let c = usize::from(country);
    let teams = records.countries.team_count(c);
    let level = || {
        u16::try_from(number).ok().and_then(|n| records.team(country.into(), n)).map_or(AMATEUR_CLASS, |t| i32::from(t.level()))
    };
    let by_level = |level: i32| {
        let main = records.globals.main_country();
        if i16::from(country) == main {
            level.min(if main == 0x10 { 3 } else { 2 })
        } else if level > 0 {
            0
        } else {
            level
        }
    };
    match records.countries.kind(c) {
        1 if number < 0 || number >= teams => AMATEUR_CLASS,
        1 => match level() {
            AMATEUR_CLASS => by_level(AMATEUR_CLASS) + 1,
            level => by_level(level),
        },
        2 if number < teams => level(),
        2 => AMATEUR_CLASS,
        _ => 0,
    }
}

impl Screen for CupResults {
    fn ids(&self) -> &[u16] {
        &[CUP_RESULTS]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let dialog = cx.dialog;
        let Args::CupResults(day) = args else { return };
        self.day = day;
        self.draw = !day.any(false);
        self.fill(cx, view);
        mark_inert(view, dialog, |t| match t {
            Target::Button(c) => dialog.controls[c].id == Some(OK),
            _ => false,
        });
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            // OK turns from the results to the draw where one stands.
            Target::Button(c) if control(dialog, OK) == Some(c) => {
                if !self.draw && self.day.any(true) {
                    self.draw = true;
                    self.fill(cx, view);
                    Outcome::Stay
                } else {
                    Outcome::Close(Reply::Ok)
                }
            }
            Target::SliderStep { control: c, .. } | Target::SliderAt { control: c, .. }
                if control(dialog, BAR) == Some(c) =>
            {
                let at = view.value(c).map_or(0, |v| v.value);
                self.show(cx, view, usize::try_from(at).unwrap_or(0));
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_classes_pair_up_as_the_table_has_them() {
        // Symmetric, every pair once, from 0 to 14.
        let mut seen = [false; 15];
        for a in 0..5 {
            for b in 0..5 {
                assert_eq!(PAIRS[a][b], PAIRS[b][a]);
                seen[usize::from(PAIRS[a][b])] = true;
            }
        }
        assert!(seen.iter().all(|&s| s));
    }
}
