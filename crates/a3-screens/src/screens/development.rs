//! The players' development (dialog 22, `CAufwertung`, event 0xca of
//! `FUN_0040f99c`): a page for each human club, copied from the report at
//! `DAT_008e2cb4` ([`a3_core::development::Report`]). The caption bar is
//! text 1877 with the club's name (`FUN_00479873`), the header over the
//! list text 5202, or 5203 when the trainer's difficulty is 0
//! (`FUN_00479afe`).
//!
//! Every row starts "%s\t%s\t%d\t%d\t%2.2f\t": the name (`FUN_006feb4d`),
//! the position's name (`FUN_007312e4`), age, form today and the season's
//! form. At difficulty 0 it ends in his old strength, "hat sich
//! verbessert" (5173) or "verschlechtert" (5174) and his new one. Else a
//! man with no reason shows old and new; one with reasons a line each,
//! low bit first, rises before falls: the strength so far, the reason
//! (texts 1878 to 1888, 4250 twice), its chance (5128 "%d%% auf +1", 5129
//! "%d%% auf -1") and the strength after, counted on only where the draw
//! held; the lines after the first leave the man's columns blank. Where
//! the count runs past his new strength a bound follows, in blue: the 13
//! at most (5121), two a round (5123, 5124), the league's ceiling (5134),
//! his talent (5122), 1 at least (5120).
//!
//! The original rolls the lines in one by one from a thread, a drawn
//! chance first on its own line; the replica shows the list rolled. OK
//! (`OnClicked_IDC_AUFWERTUNG_OK`) turns to the next club's page, after
//! the last it closes.

use std::collections::HashMap;

use a3_core::development::{REASONS, ReportPage, ReportRow};
use a3_files::rch::Dialog;

use super::discipline::CAPTION_ID;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::pointer::Target;
use crate::view::View;

pub const DEVELOPMENT: u16 = a3_core::cycle::dialogs::DEVELOPMENT;
const OK: u16 = 1306;
const LIST: u16 = 1307;
const HEADER: u16 = 1308;
/// IDS_AUFWERTUNGEN "ANSTOSS 3 - Auf- und Abwertungen %s".
const CAPTION_TEXT: u16 = 1877;
/// IDS_AUFWERTUNG_NORMAL and IDS_AUFWERTUNG_EINFACH.
const HEADER_FULL: u16 = 5202;
const HEADER_PLAIN: u16 = 5203;
/// IDS_HAT_SICH_VERBESSERT, IDS_HAT_SICH_VERSCHLECHTERT.
const BETTER: u16 = 5173;
const WORSE: u16 = 5174;
/// IDS_AUFWERTUNG_CHANCE "%d%% auf +1", _1 "%d%% auf -1".
const CHANCE_UP: u16 = 5128;
const CHANCE_DOWN: u16 = 5129;
/// The reasons by bit (`DAT_0087f548`).
const REASON_TEXTS: [u16; REASONS] = [1878, 1879, 1880, 1881, 1882, 1883, 1884, 1885, 1886, 1887, 1888, 4250, 4250];
/// The bounds: 1 at least, 13 at most, talent, two up, league, two down.
const AT_LEAST_ONE: u16 = 5120;
const AT_MOST_13: u16 = 5121;
const BY_TALENT: u16 = 5122;
const TWO_UP: u16 = 5123;
const TWO_DOWN: u16 = 5124;
const BY_LEAGUE: u16 = 5134;
/// The bounds' lines (`FUN_007a90c1` with 0xff0000).
const BOUND_INK: [u8; 3] = [0, 0, 255];
/// The blank columns of a man's lines after his first (0x87f60c).
const BLANK: &str = " \t \t \t \t \t";

/// The caption bar takes the page's club.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&DEVELOPMENT) else { return };
    let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}

/// A page's lines as `FUN_00479afe` writes them, each with its ink.
pub fn lines(texts: &dyn Fn(u16) -> String, name: &dyn Fn(u16) -> String, position: &dyn Fn(u8) -> String, page: &ReportPage) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    for row in &page.rows {
        row_lines(texts, page.insight, &first_columns(row, name, position), row, &mut out);
    }
    out
}

fn first_columns(row: &ReportRow, name: &dyn Fn(u16) -> String, position: &dyn Fn(u8) -> String) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{:2.2}\t",
        name(row.player),
        position(row.position),
        row.age,
        row.form,
        f32::from(row.season_form) / 10.0
    )
}

fn row_lines(texts: &dyn Fn(u16) -> String, insight: u16, first: &str, row: &ReportRow, out: &mut Vec<(String, bool)>) {
    let (old, new) = (row.old, row.new);
    if insight == 0 {
        out.push((
            match new.cmp(&old) {
                std::cmp::Ordering::Greater => format!("{first}{old}\t{}\t\t{new}", texts(BETTER)),
                std::cmp::Ordering::Less => format!("{first}{old}\t{}\t\t{new}", texts(WORSE)),
                std::cmp::Ordering::Equal => format!("{first}{old}\t\t\t{new}"),
            },
            false,
        ));
        return;
    }
    if row.up == 0 && row.down == 0 {
        out.push((format!("{first}{old}\t\t\t{new}"), false));
        return;
    }
    let mut first = first.to_owned();
    let mut count = i16::from(old);
    for k in 0..REASONS {
        let (rise, text) = if row.up & 1 << k != 0 {
            (1, CHANCE_UP)
        } else if row.down & 1 << k != 0 {
            (-1, CHANCE_DOWN)
        } else {
            continue;
        };
        let chance = row.chances[k];
        let odds = texts(text).replacen("%d", &chance.unsigned_abs().to_string(), 1).replace("%%", "%");
        let reason = texts(REASON_TEXTS[k]);
        let after = if chance < 0 { count } else { count + rise };
        out.push((format!("{first}{count}\t{reason}\t{odds}\t{after}"), false));
        count = after;
        first = BLANK.to_owned();
    }
    let new = i16::from(new);
    let bound = if new < count {
        Some(if new == 13 {
            texts(AT_MOST_13)
        } else if new + 2 < count {
            texts(TWO_UP)
        } else if i16::from(row.ceiling) < count {
            texts(BY_LEAGUE).replacen("%d", &new.to_string(), 1)
        } else {
            texts(BY_TALENT).replacen("%d", &new.to_string(), 1)
        })
    } else if count < 1 {
        Some(texts(AT_LEAST_ONE))
    } else if count < new - 2 {
        Some(texts(TWO_DOWN))
    } else {
        None
    };
    if let Some(bound) = bound {
        out.push((bound, true));
    }
}

#[derive(Default)]
pub struct Development {
    /// The page shown, by club (+0x60bc).
    page: usize,
}

impl Development {
    fn pages<'a>(cx: &'a Cx) -> &'a [ReportPage] {
        cx.state.season.as_ref().map_or(&[], |s| s.development.as_slice())
    }

    /// The page's club's lines (`FUN_00479873`, `FUN_00479afe`).
    fn fill(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(page) = Self::pages(cx).get(self.page) else { return };
        let records = cx.state.records.as_ref();
        let slot = game.world.slot_of(page.country.into()).unwrap_or(0);
        let club = game.world.country(slot).and_then(|c| c.clubs.get(usize::try_from(page.number).unwrap_or(0)));
        let caption = game.text(CAPTION_TEXT).replacen("%s", club.map_or("", |c| c.name.as_str()), 1);
        set(view, dialog, CAPTION_ID, caption);
        let header = if page.insight != 0 { HEADER_FULL } else { HEADER_PLAIN };
        // The header splits its titles where the text writes "\t".
        if let Some(header_text) = control(dialog, HEADER).and_then(|c| view.header_mut(c)) {
            header_text.text = game.text(header).to_owned();
        }
        let name = |id: u16| {
            const NICKNAME: u32 = 0x8000_0000;
            let (Some(records), Some(r)) = (records, records.and_then(|r| r.player(id))) else { return String::new() };
            if r.shown() & NICKNAME != 0 {
                records.name(r.nickname()).to_owned()
            } else {
                format!("{} {}", records.name(r.first_name()), records.name(r.last_name()))
            }
        };
        let position = |kind: u8| super::hub::abbreviation(game, kind);
        let texts = |id: u16| game.text(id).to_owned();
        let lines = lines(&texts, &name, &position, page);
        if let Some(list) = control(dialog, LIST).and_then(|c| view.list_mut(c)) {
            list.keys = (0..lines.len()).collect();
            list.inks = lines.iter().map(|&(_, bound)| bound.then_some(BOUND_INK)).collect();
            list.rows = lines.into_iter().map(|(text, _)| text).collect();
            list.row_icons.clear();
            list.selected = None;
            list.scroll = 0;
        }
    }
}

impl Screen for Development {
    fn ids(&self) -> &[u16] {
        &[DEVELOPMENT]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        self.page = 0;
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
                if self.page < Self::pages(cx).len() {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// At a difficulty above 0 a man's reasons come a line each, the
    /// strength counted on where a draw held, and a bound behind in blue.
    #[test]
    fn reasons_count_on_to_the_bound() {
        let texts = |id: u16| match id {
            CHANCE_UP => "%d%% auf +1".to_owned(),
            CHANCE_DOWN => "%d%% auf -1".to_owned(),
            BY_LEAGUE => "Stärke durch Liga auf %d begrenzt".to_owned(),
            1878 => "einsatzbedingt".to_owned(),
            1880 => "Form".to_owned(),
            1881 => "Alter/Talent".to_owned(),
            _ => String::new(),
        };
        let mut chances = CHANCES_SHOWN;
        chances[2] = -12;
        chances[3] = 40;
        let row = ReportRow { player: 1, position: 10, age: 20, form: 11, old: 9, new: 9, ceiling: 9, up: 1 | 4 | 8, down: 0, season_form: 112, chances };
        let page = ReportPage { country: 0, number: 0, insight: 2, rows: vec![row] };
        let lines = lines(&texts, &|_| "Max Muster".to_owned(), &|_| "ST".to_owned(), &page);
        let lines: Vec<(&str, bool)> = lines.iter().map(|(t, b)| (t.as_str(), *b)).collect();
        assert_eq!(
            lines,
            [
                ("Max Muster\tST\t20\t11\t11.20\t9\teinsatzbedingt\t100% auf +1\t10", false),
                (" \t \t \t \t \t10\tForm\t12% auf +1\t10", false),
                (" \t \t \t \t \t10\tAlter/Talent\t40% auf +1\t11", false),
                ("Stärke durch Liga auf 9 begrenzt", true),
            ]
        );
    }

    const CHANCES_SHOWN: [i16; REASONS] = [100; REASONS];
}
