//! Half time of a human club's match: the break (dialog 93,
//! `CHalbzeitpause`) and the talk to the team behind it (dialog 499,
//! `CHalbzeitAnsprache`), one pair for each human side
//! ([`a3_core::halftime`]).
//!
//! The break is the line-up as the match stands at the 45th minute
//! (`FUN_004b9244`, `FUN_004b98ad`, `FUN_004bace2`): the caption with both
//! clubs, the half's score and chances, the referee and the minute; the
//! eleven on their spots and the bench (the boxes 1916 and 1927 on, placed
//! as the line-up's), the squad below, the comparison with the opponent;
//! how many may still come on (`FUN_00659053`); and the bar of the
//! manager's breaks, a club and its score each (6269). OK (1943) is done
//! (`OnClicked_IDC_HALBZEIT_OK`); the substitutions, the statistics and
//! duties of its bar (6267), the tactics and the talks to single men wait.
//!
//! The talk lists twelve lines (`FUN_004b7e8c`, `DAT_00881a14`); OK, or a
//! double click, keeps the chosen one (`OnClicked_IDOK` at 0x4b8125), its
//! sound left out; Cancel keeps none.

use std::collections::HashMap;

use a3_core::halftime::{Break, TALKS};
use a3_core::season::ClubState;
use a3_files::rch::Dialog;

use super::announcement::ordinal;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::game::Game;
use crate::pointer::Target;
use crate::view::View;

pub const HALFTIME: u16 = a3_core::cycle::dialogs::HALFTIME;
pub const TALK: u16 = a3_core::cycle::dialogs::HALFTIME_TALK;
const CAPTION_ID: u16 = 9947;
/// The break's controls (`FUN_004b8e1e`).
const FIELD_BOXES: u16 = 1916;
const BENCH_BOXES: u16 = 1927;
const STILL_ON: u16 = 1942;
const DONE: u16 = 1943;
const MATCHES: u16 = 6269;
const TITLES: u16 = 5514;
const SECOND_TITLES: u16 = 5515;
/// The picture under the second row of titles (`FUN_007a743a`).
const SECOND_TITLES_PICTURE: u16 = 2259;
/// "%s - %s %d : %d Chancen (%d:%d)", ", %s Minute" and the referee's
/// "%s (K. %d, H. %d)" (`FUN_004b98ad`).
const CAPTION_TEXT: u16 = 2627;
const MINUTE_TEXT: u16 = 2626;
const REFEREE_TEXT: u16 = 2625;
/// The break comes at the 45th minute (`DAT_01bad052`).
const MINUTE: u8 = 45;
/// A league or cup match lets three come on (`FUN_006ef296`: the side's
/// +0xb2 at 3 for a record whose +0x24 is nought).
const SUBSTITUTIONS: usize = 3;
/// The talk's list.
const LINES: u16 = 4767;
/// The eleven's average age beside the comparison's average form, the
/// club's and the opponent's ("DA", `FUN_004bb7f1` over `FUN_0067582a`).
const AGES: [u16; 2] = [2554, 2566];

/// The break's boxes are the line-up's, and its second row of titles lies
/// over the first (`FUN_004b9244`); both dialogs get a caption.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &a3_core::season::Rules) {
    if let Some(dialog) = dialogs.get_mut(&HALFTIME) {
        super::hub::place_boxes(dialog, rules, FIELD_BOXES, BENCH_BOXES);
        let first = dialog.controls.iter().find(|c| c.id == Some(TITLES)).map(|c| c.rect);
        if let Some(rect) = first
            && let Some(second) = dialog.controls.iter_mut().find(|c| c.id == Some(SECOND_TITLES))
        {
            second.rect = rect;
        }
    }
    for id in [HALFTIME, TALK] {
        if let Some(dialog) = dialogs.get_mut(&id) {
            let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
            dialog.controls.insert(0, caption);
        }
    }
}

/// The average age of a club's eleven, "%2.1f"; blank without one.
fn average_age(state: &ClubState, today: chrono::NaiveDate) -> String {
    let ages: Vec<u32> = state.lineup.on_field().map(|(_, p)| state.players[p].age(today)).collect();
    if ages.is_empty() {
        return String::new();
    }
    format!("{:.1}", f64::from(ages.iter().sum::<u32>()) / ages.len() as f64)
}

/// The break the dialogs show.
fn current(cx: &Cx) -> Option<Break> {
    let season = cx.state.season.as_ref()?;
    season.halftime.get(season.halftime_at).cloned()
}

/// A club's name by its country and number.
fn club_name(cx: &Cx, (country, number): (u8, i16)) -> String {
    cx.game.club_name(cx.state.records.as_ref(), country, number)
}

/// The break's caption (`FUN_004b98ad`): "Union - Hertha 1 : 0 Chancen
/// (0:0), Referee (K. 5, H. 7), 45. Minute".
fn caption(cx: &Cx, game: &Game, b: &Break) -> String {
    let mut text = game
        .text(CAPTION_TEXT)
        .replacen("%s", &club_name(cx, b.sides[0]), 1)
        .replacen("%s", &club_name(cx, b.sides[1]), 1);
    for n in [b.goals[0], b.goals[1], b.chances[0], b.chances[1]] {
        text = text.replacen("%d", &n.to_string(), 1);
    }
    let referee = cx.state.records.as_ref().filter(|_| b.referee > 0).and_then(|r| r.referee(b.referee));
    if let Some((name, competence, hardness)) = referee {
        text.push_str(", ");
        text.push_str(
            &game
                .text(REFEREE_TEXT)
                .replacen("%s", name, 1)
                .replacen("%d", &competence.to_string(), 1)
                .replacen("%d", &hardness.to_string(), 1),
        );
    }
    text + &game.text(MINUTE_TEXT).replacen("%s", &ordinal(MINUTE), 1)
}

#[derive(Default)]
pub struct Halftime;

impl Screen for Halftime {
    fn ids(&self) -> &[u16] {
        &[HALFTIME, TALK]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(b) = current(cx) else { return };
        if view.id == TALK {
            // "ANSTOSS 3 - Halbzeitansprache - Union (1:0)", the club's
            // goals first.
            let [own, other] = b.own_goals();
            let title = format!("{} - {} ({own}:{other})", dialog.title, club_name(cx, b.club));
            set(view, dialog, CAPTION_ID, title);
            if let Some(list) = control(dialog, LINES).and_then(|c| view.list_mut(c)) {
                list.rows = TALKS.iter().map(|&id| game.text(id).to_owned()).collect();
                list.selected = None;
                list.scroll = 0;
            }
            mark_inert(view, dialog, |t| match t {
                Target::Button(c) => matches!(dialog.controls[c].id, Some(1 | 2)),
                _ => false,
            });
            return;
        }
        set(view, dialog, CAPTION_ID, caption(cx, game, &b));
        // The bar of the manager's breaks: the club and its score, its
        // goals first ("%s %d:%d"); the places behind them off.
        if let Some(options) = control(dialog, MATCHES).and_then(|c| view.options_mut(c)) {
            let [own, other] = b.own_goals();
            let label = format!("{} {own}:{other}", club_name(cx, b.club));
            for (k, l) in options.labels.iter_mut().enumerate() {
                *l = if k == 0 { label.clone() } else { String::new() };
            }
            options.off = (0..options.labels.len()).map(|k| k > 0).collect();
            options.chosen = 0;
        }
        if let Some(second) = control(dialog, SECOND_TITLES) {
            view.controls[second].hidden = true;
            if let Some(titles) = view.options_mut(second) {
                titles.background = Some(SECOND_TITLES_PICTURE);
                titles.chosen = 0;
            }
        }
        let key = game.world.slot_of(b.club.0.into()).zip(usize::try_from(b.club.1).ok());
        let season = cx.state.season.as_ref();
        if let (Some(season), Some(state)) = (season, key.and_then(|k| season?.clubs.get(&k))) {
            super::hub::show_break_lineup(
                view,
                dialog,
                game,
                season,
                state,
                cx.state.records.as_ref(),
                FIELD_BOXES,
                BENCH_BOXES,
            );
            // Who may still come on: the bench's men, three at most
            // (`FUN_00659053` over the side's +0x8e).
            let bench = state.lineup.bench.iter().filter(|p| p.is_some()).count();
            if let Some(c) = control(dialog, STILL_ON) {
                let text = dialog.controls[c].text.replacen("%d", &bench.min(SUBSTITUTIONS).to_string(), 1);
                view.set_text(c, text);
            }
            let today = season.date();
            let theirs = b.sides.into_iter().find(|&side| side != b.club).and_then(|(country, number)| {
                let key = (game.world.slot_of(country.into())?, usize::try_from(number).ok()?);
                a3_core::load::club(cx.state.records.as_ref()?, &game.data, key)
            });
            set(view, dialog, AGES[0], average_age(state, today));
            set(view, dialog, AGES[1], theirs.map_or_else(String::new, |t| average_age(&t, today)));
        }
        mark_inert(view, dialog, |t| match t {
            Target::Button(c) => dialog.controls[c].id == Some(DONE),
            Target::Option { control: c, item } => dialog.controls[c].id == Some(MATCHES) && item == 0,
            _ => false,
        });
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, double: bool) -> Outcome {
        let dialog = cx.dialog;
        if view.id == TALK {
            let chosen = || {
                let list = control(dialog, LINES).and_then(|c| view.list(c))?;
                list.selected.map(|row| Reply::Choice(row + 1))
            };
            return match target {
                Target::Row { control: c, .. } if double && dialog.controls[c].id == Some(LINES) => {
                    Outcome::Close(chosen().unwrap_or(Reply::Ok))
                }
                Target::Button(c) if dialog.controls[c].id == Some(1) => Outcome::Close(chosen().unwrap_or(Reply::Ok)),
                Target::Button(c) if dialog.controls[c].id == Some(2) => Outcome::Close(Reply::Cancel),
                _ => Outcome::Stay,
            };
        }
        match target {
            Target::Button(c) if dialog.controls[c].id == Some(DONE) => Outcome::Close(Reply::Ok),
            _ => Outcome::Stay,
        }
    }
}
