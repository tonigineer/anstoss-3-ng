//! The table after the matchday (dialog 269, `CTabelleNachDemSpiel`): at
//! the matchday's end, for the leagues the game's switch names, one after
//! the other on OK — the day's results two to a line, and the table with
//! its home and away figures (`FUN_00587c75`, `FUN_00587d38`). Its other
//! views on the tabs below (`OnClicked_IDC_TAB_NACH_TAB`: `FUN_00589279`,
//! `FUN_005896ae`, `FUN_00588dc9`, `FUN_00589b1b`) are not built.

use std::collections::HashMap;

use a3_core::opponent::League;
use a3_core::records::Records;
use a3_core::statistics::{self, ALL, AWAY, HOME, Row};
use a3_files::rch::Dialog;
use image::Rgba;

use super::statistics::{MARK_COLOURS, lettered};
use super::youth_tables::set_columns;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert};
use crate::controls::list::Bar;
use crate::game::Game;
use crate::pointer::Target;
use crate::view::View;

pub const TABLES_AFTER: u16 = a3_core::cycle::dialogs::TABLES_AFTER;
const OK: u16 = 3726;
const HEADLINE: u16 = 3727;
const RESULTS: u16 = 3728;
const TABLE: u16 = 3729;
const TABLE_HEADER: u16 = 3730;
const VIEWS: u16 = 3723;
/// The view the dialog opens on: the results and the table.
const TABLE_VIEW: usize = 0;
const CAPTION_ID: u16 = 9947;
/// "%s: Der %s Spieltag im Überblick", of the league and the matchday.
const HEADLINE_TEXT: u16 = 1640;
/// "Pos\tVerein\tSpiele\tGesamt\tHeimbilanz\tAuswärtsbilanz".
const HEADER_TEXT: u16 = 2425;
/// The header's columns (`0x88a758`, `0x88a774`): 0x4000 centred and
/// 0x8000 right in the width beside them.
const HEADER_COLUMNS: [(u16, u16); 7] =
    [(0x8000, 40), (0x4032, 110), (0x40b9, 40), (0x40e1, 90), (0x4154, 130), (0x41e0, 100), (600, 0)];
/// The table's nineteen columns (`FUN_007a941f` with `0x88a790`,
/// `0x88a7dc`): place, the place before, the club, its games, then points,
/// difference and goals of all, the home and the away games.
const TABLE_COLUMNS: [(u16, u16); 19] = [
    (0x8002, 18),
    (0x8016, 25),
    (50, 0),
    (0x80be, 20),
    (0x80d2, 30),
    (0x80f0, 30),
    (0x8111, 25),
    (300, 0),
    (305, 0),
    (0x814f, 30),
    (0x816d, 30),
    (0x818e, 25),
    (425, 0),
    (430, 0),
    (0x81d6, 30),
    (0x81f4, 30),
    (0x8213, 25),
    (558, 0),
    (563, 0),
];
/// A human's club stands red (`FUN_00677428`, colour 0xff).
const OWN_INK: [u8; 3] = [255, 0, 0];
/// A place's box (`FUN_007a9370` with the rectangle 5, 1, 20, 12 and
/// `FUN_007a93e7` at 100).
const MARK_LEFT: i32 = 5;
const MARK_WIDTH: i32 = 15;

/// The dialog keeps its title to itself; the caption bar takes it.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&TABLES_AFTER) else { return };
    let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}

#[derive(Default)]
pub struct TablesAfter {
    /// The leagues to show, by country and level, and the one shown.
    leagues: Vec<(u16, u8)>,
    shown: usize,
}

impl TablesAfter {
    /// The league's results and table (`FUN_00587c75`, `FUN_00587d38`).
    fn show(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return };
        let Some(&(country, level)) = self.leagues.get(self.shown) else { return };
        let Some(league) = League::of(records, country, level) else { return };
        // "%s: Der %s Spieltag im Überblick", the matchday with the
        // ordinal's "." (`FUN_00730af1`) — the last one played.
        let headline = game
            .text(HEADLINE_TEXT)
            .replacen("%s", &league.name(), 1)
            .replacen("%s", &format!("{}.", league.played()), 1);
        if let Some(header) = control(dialog, HEADLINE).and_then(|c| view.header_mut(c)) {
            header.text = headline;
        }
        if let Some(header) = control(dialog, TABLE_HEADER).and_then(|c| view.header_mut(c)) {
            header.text = game.text(HEADER_TEXT).to_owned();
            header.tabs = HEADER_COLUMNS.iter().map(|&(x, _)| f32::from(x & 0x3fff)).collect();
            header.aligns = HEADER_COLUMNS
                .iter()
                .map(|&(x, w)| match x & 0xc000 {
                    0x4000 => Some((a3_files::rch::Align::Centre, f32::from(w))),
                    0x8000 => Some((a3_files::rch::Align::Right, f32::from(w))),
                    _ => None,
                })
                .collect();
        }
        if let Some(c) = control(dialog, RESULTS)
            && let Some(list) = view.list_mut(c)
        {
            list.rows = result_rows(game, records, &league);
            list.inks.clear();
            list.scroll = 0;
        }
        if let Some(c) = control(dialog, TABLE) {
            set_columns(view, c, &TABLE_COLUMNS);
            let (rows, inks, bars) = table_rows(game, records, &league);
            if let Some(list) = view.list_mut(c) {
                list.rows = rows;
                list.inks = inks;
                list.bars = bars;
                list.scroll = 0;
            }
        }
    }
}

/// The last matchday's games, two to a line: "home\t- away" and its
/// score with the half time where it stands, a tab between the two.
fn result_rows(game: &Game, records: &Records, league: &League) -> Vec<String> {
    let played = league.played();
    if played == 0 {
        return Vec::new();
    }
    let country = league.country();
    let pair = |&(h, a, result): &(usize, usize, Option<[u8; 4]>)| {
        let name = |slot: usize| game.club_name(Some(records), country as u8, league.team(slot));
        let mut text = format!("{}\t- {}", name(h), name(a));
        if let Some([gh, ga, hh, ha]) = result {
            text.push_str(&format!("\t{gh}\t:{ga}\t({hh}\t:{ha})"));
        }
        text
    };
    league
        .games(played - 1)
        .chunks(2)
        .map(|two| {
            let second = two.get(1).map(pair).unwrap_or_default();
            format!("{}\t{second}", pair(&two[0]))
        })
        .collect()
}

/// The table's lines: the place, the place before ("(%d.)", "-" before
/// the second matchday), the club with its letters, its games, then
/// points, difference and goals of all its games, and of its home and its
/// away games, those two reckoned without the penalty points
/// (`FUN_00729f35`); a human's club red, each place's box in its colour.
fn table_rows(
    game: &Game,
    records: &Records,
    league: &League,
) -> (Vec<String>, Vec<Option<[u8; 3]>>, Vec<Option<Bar>>) {
    let country = league.country();
    let all = statistics::table(records, league, ALL);
    let home = statistics::unpenalised_table(records, league, HOME);
    let away = statistics::unpenalised_table(records, league, AWAY);
    let find = |rows: &[Row], team: i16| rows.iter().find(|r| r.team == team).copied().unwrap_or_default();
    let figures = |r: &Row| {
        format!("{}\t{}\t{}\t:\t{}", r.points, r.goals_for - r.goals_against, r.goals_for, r.goals_against)
    };
    let humans = statistics::human_clubs(records);
    let marks = league.marks();
    let mut lines = (Vec::new(), Vec::new(), Vec::new());
    for (i, row) in all.iter().enumerate() {
        let before = match league.place_before(row.place) {
            0 => "-".to_owned(),
            p => format!("({p}.)"),
        };
        let text = format!(
            "{}.\t{before}\t{}\t{}\t{}\t{}\t{}",
            i + 1,
            lettered(game, records, country, row.team),
            row.games,
            figures(row),
            figures(&find(&home, row.team)),
            figures(&find(&away, row.team)),
        );
        let own = humans.iter().any(|&(c, n)| u16::from(c) == country && n == row.team);
        let bar = usize::try_from(marks.get(i).copied().unwrap_or(0) - 1)
            .ok()
            .and_then(|k| MARK_COLOURS.get(k))
            .map(|&[r, g, b]| Bar { left: MARK_LEFT, width: MARK_WIDTH, value: 100, fill: Rgba([r, g, b, 255]) });
        lines.0.push(text);
        lines.1.push(own.then_some(OWN_INK));
        lines.2.push(bar);
    }
    lines
}

impl Screen for TablesAfter {
    fn ids(&self) -> &[u16] {
        &[TABLES_AFTER]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let dialog = cx.dialog;
        let Args::Tables { leagues, count } = args else { return };
        self.leagues = leagues[..usize::from(count).min(leagues.len())].to_vec();
        self.shown = 0;
        if let Some(views) = control(dialog, VIEWS).and_then(|c| view.options_mut(c)) {
            views.chosen = TABLE_VIEW;
        }
        self.show(cx, view);
        mark_inert(view, dialog, |t| match t {
            Target::Button(c) => dialog.controls[c].id == Some(OK),
            Target::Option { control: c, item } => {
                dialog.controls[c].id == Some(VIEWS) && item == TABLE_VIEW
            }
            _ => false,
        });
    }

    /// OK steps to the next league of the list and closes after the last
    /// (`OnClicked_IDC_TAB_NACH_OK`), back on the table's view.
    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            Target::Button(c) if dialog.controls[c].id == Some(OK) => {
                self.shown += 1;
                if self.shown >= self.leagues.len() {
                    return Outcome::Close(Reply::Ok);
                }
                if let Some(views) = control(dialog, VIEWS).and_then(|c| view.options_mut(c)) {
                    views.chosen = TABLE_VIEW;
                }
                self.show(cx, view);
                Outcome::Stay
            }
            // The other views are not built: the tabs stay on the table.
            Target::Option { control: c, .. } if dialog.controls[c].id == Some(VIEWS) => {
                if let Some(views) = view.options_mut(c) {
                    views.chosen = TABLE_VIEW;
                }
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}
