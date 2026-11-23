//! The last information before the match (dialog 96, `CInfoseiteHaupt`):
//! for each of the day's matches a human's club plays, up to four, a page
//! (dialog 98, `CInfoseiteSpiel`, filled by `FUN_004ccc26`) where the table
//! at 0x8825b8 puts it for their number, on that number's background
//! (0x882658, `FUN_004cb9e5`), the match display's combo hidden: the
//! competition, both clubs with their crests, last results and places, the
//! first leg, last season's game, the home side's record against the other,
//! the strengths, the ground, the crowd, the visiting fans, the pitch and
//! the referee. OK closes it (`OnClicked_IDOK`); the celebrity's tip (2018,
//! `CInfoseiteProminententip`) is not built.
//!
//! The pages are taken before the replica plays the day's matches
//! ([`a3_core::prematch`]); the crowd is the played match's.

use std::collections::HashMap;

use a3_core::matches::Fixture;
use a3_core::prematch::Page;
use a3_files::rch::{Align, Control, Dialog, Kind};
use bevy_math::{Rect, Vec2};

use super::announcement::{MATCHDAY_TEXT, ROUNDS, SEASON_TEXT, crest, goals, ordinal};
use super::finances::count;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert};
use crate::controls::bitmap::Picture;
use crate::pointer::Target;
use crate::view::View;

pub const PREMATCH: u16 = a3_core::cycle::dialogs::PREMATCH;
const PAGE: u16 = 98;
const OK: u16 = 1;
const TIP: u16 = 2018;
/// The match display's combo, hidden once there are pages.
const DISPLAY: u16 = 6479;
const CAPTION_ID: u16 = 9947;
/// The background for the number of pages, which the dialog sets itself
/// (`FUN_007a12fc`): a picture of the game's over the whole screen, the
/// dialog file's own (3100) under four.
const BACKDROP: u16 = 9948;
const BACKGROUNDS: [u16; 5] = [3097, 3097, 3098, 3099, 3100];
/// Where each page goes for their number (`DAT_008825b8`), 300 × 210.
const PLACES: [&[(f32, f32)]; 5] = [
    &[],
    &[(170.0, 124.0)],
    &[(10.0, 139.0), (334.0, 139.0)],
    &[(10.0, 22.0), (334.0, 22.0), (171.0, 229.0)],
    &[(10.0, 22.0), (334.0, 22.0), (10.0, 229.0), (334.0, 229.0)],
];
/// The page's controls (`FUN_004cc9f8`).
const CRESTS: [u16; 2] = [2025, 2026];
const NAMES: [u16; 2] = [2027, 2032];
const FORMS: [u16; 2] = [2028, 2031];
const TABLE_PLACES: [u16; 2] = [2029, 2030];
const FIRST_LEG: u16 = 2033;
const LAST: u16 = 2034;
const TENDENCY: u16 = 2035;
const STRENGTHS: u16 = 2036;
const CROWD: u16 = 2037;
const FANS: u16 = 2038;
const PITCH: u16 = 2040;
const REFEREE: u16 = 2041;
const HEADLINE: u16 = 6424;
const GROUND: u16 = 6449;
/// "%s (A)": a sold-out crowd, with points between the thousands.
const SOLD_OUT: u16 = 2600;
/// "%s (Ko. %d, Hä. %d)": the referee, his competence and hardness.
const REFEREE_TEXT: u16 = 2457;
/// The pitch by its wear: up to 1 "Optimal", below 5 "Ordentlich", below
/// 10 "Mittelmäßig", below 20 "Holprig", below 30 "Kartoffelacker", else
/// "Spielfeld???".
const PITCHES: [(u8, u16); 5] = [(2, 2203), (5, 2205), (10, 2204), (20, 2206), (30, 2207)];
const WORST_PITCH: u16 = 2208;
const NONE: &str = "-";

/// The pages of every number of them, each in its place, follow the
/// dialog's own controls; the background's picture and the caption go
/// first.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(page) = dialogs.get(&PAGE).cloned() else { return };
    let Some(dialog) = dialogs.get_mut(&PREMATCH) else { return };
    let origin = page.frame.map_or(Vec2::ZERO, |frame| frame.min);
    for places in PLACES {
        for &(x, y) in places {
            let shift = Vec2::new(x, y) - origin;
            dialog.controls.extend(page.controls.iter().cloned().map(|mut c| {
                c.rect = Rect::from_corners(c.rect.min + shift, c.rect.max + shift);
                c
            }));
        }
    }
    let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
    let backdrop = Control {
        kind: Kind::Bitmap,
        rect: Rect::new(0.0, 0.0, 640.0, 480.0),
        align: Align::Left,
        text: String::new(),
        id: Some(BACKDROP),
        font: a3_files::rch::PLAIN_FONT,
        ink: [0; 3],
    };
    dialog.controls.insert(0, backdrop);
}

#[derive(Default)]
pub struct Prematch;

impl Prematch {
    /// The page `index` of `count` pages: where its first control is, the
    /// pages standing one after the other in their order.
    fn first(cx: &Cx, count: usize, index: usize) -> Option<usize> {
        let anchor = cx.game.dialogs.get(&PAGE)?.controls.first()?.id?;
        let before: usize = PLACES[..count].iter().map(|p| p.len()).sum();
        cx.dialog.controls.iter().enumerate().filter(|(_, c)| c.id == Some(anchor)).nth(before + index).map(|(i, _)| i)
    }

    /// One match's page (`FUN_004ccc26`).
    fn fill(cx: &Cx, view: &mut View, first: usize, page: &Page) {
        let game = cx.game;
        let Some(records) = cx.state.records.as_ref() else { return };
        let Some(template) = game.dialogs.get(&PAGE) else { return };
        let at = |id: u16| template.controls.iter().position(|c| c.id == Some(id)).map(|j| first + j);
        let set = |view: &mut View, id: u16, text: String| {
            if let Some(c) = at(id) {
                view.set_text(c, text);
            }
        };
        let team = |(country, number): (u8, i16)| records.team(country.into(), u16::try_from(number).ok()?);
        let sides = page.sides.map(team);
        for (k, side) in sides.iter().enumerate() {
            if let Some(c) = at(CRESTS[k]) {
                view.set_picture(c, side.and_then(|t| crest(game, records, t)));
            }
            set(view, NAMES[k], side.map(|t| t.name()).unwrap_or_default());
            set(view, FORMS[k], page.forms[k].clone());
            let place = page.places[k];
            set(view, TABLE_PLACES[k], if place > 0 { format!("({})", ordinal(place)) } else { String::new() });
        }
        let headline = match page.fixture {
            // "1. Bundesliga 2025/26 - 4. Spieltag".
            Fixture::League { matchday, .. } => {
                let name = a3_core::prematch::league(records, page).map(|l| l.name()).unwrap_or_default();
                let year = i32::from(records.globals.start_year());
                let next = if (year + 1) % 100 == 0 { (year + 1).to_string() } else { format!("{:02}", (year + 1) % 100) };
                let season = game.text(SEASON_TEXT).replacen("%s", &name, 1).replacen("%s", &year.to_string(), 1).replacen("%s", &next, 1);
                let day = game.text(MATCHDAY_TEXT).replacen("%d", &matchday.to_string(), 1).replacen("%s", ".", 1);
                format!("{season} - {day}")
            }
            // "DFB-Pokal 1. Runde" (`FUN_0054d7fc`, code 0x40000000).
            Fixture::Cup { code } => {
                let cup = records.national_cup_name(page.sides[0].0).unwrap_or_default();
                let round = ROUNDS.get((code & 0x3f) as usize).map(|&id| game.text(id)).unwrap_or_default();
                format!("{cup} {round}")
            }
        };
        set(view, HEADLINE, headline);
        set(view, FIRST_LEG, goals(page.first_leg));
        let league = matches!(page.fixture, Fixture::League { .. });
        set(view, LAST, if league { goals(page.last) } else { NONE.to_owned() });
        let tendency = page.tendency.map(|[w, d, l]| format!("{w}:{d}:{l}"));
        set(view, TENDENCY, tendency.unwrap_or_else(|| NONE.to_owned()));
        set(view, STRENGTHS, format!("{} : {}", page.strengths[0], page.strengths[1]));
        let (ground, capacity, wear) = page.ground.clone().unwrap_or_default();
        set(view, GROUND, ground);
        let crowd = if page.attendance >= capacity {
            game.text(SOLD_OUT).replacen("%s", &count(page.attendance), 1)
        } else {
            page.attendance.to_string()
        };
        set(view, CROWD, crowd);
        set(view, FANS, page.fans.map_or_else(|| NONE.to_owned(), |f| f.to_string()));
        let pitch = PITCHES.iter().find(|&&(below, _)| wear < below).map_or(WORST_PITCH, |&(_, id)| id);
        set(view, PITCH, game.text(pitch).to_owned());
        let referee = records.referee(page.referee).map(|(name, competence, hardness)| {
            game.text(REFEREE_TEXT)
                .replacen("%s", name, 1)
                .replacen("%d", &competence.to_string(), 1)
                .replacen("%d", &hardness.to_string(), 1)
        });
        set(view, REFEREE, referee.unwrap_or_default());
    }
}

impl Screen for Prematch {
    fn ids(&self) -> &[u16] {
        &[PREMATCH]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        let pages = cx.state.season.as_ref().map(|s| s.prematch.clone()).unwrap_or_default();
        let count = pages.len().min(a3_core::prematch::PAGES);
        // Every page hidden but the ones of this number.
        if let (Some(first), Some(per)) = (Self::first(cx, 1, 0), cx.game.dialogs.get(&PAGE).map(|d| d.controls.len())) {
            let slots: usize = PLACES.iter().map(|p| p.len()).sum();
            for shown in view.controls.iter_mut().skip(first).take(slots * per) {
                shown.hidden = true;
            }
            for (k, page) in pages.iter().take(count).enumerate() {
                let Some(start) = Self::first(cx, count, k) else { continue };
                for shown in &mut view.controls[start..start + per] {
                    shown.hidden = false;
                }
                Self::fill(cx, view, start, page);
            }
        }
        if let Some(c) = control(dialog, BACKDROP) {
            let own = BACKGROUNDS[count] == dialog.background;
            view.controls[c].hidden = own;
            view.set_picture(c, (!own).then(|| Picture::graphic(BACKGROUNDS[count])));
        }
        if count > 0
            && let Some(c) = control(dialog, DISPLAY)
        {
            view.controls[c].hidden = true;
        }
        mark_inert(view, dialog, |t| match t {
            Target::Button(c) => dialog.controls[c].id == Some(OK),
            _ => false,
        });
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(c) if cx.dialog.controls[c].id == Some(OK) => Outcome::Close(Reply::Ok),
            // The celebrity's tip (`CInfoseiteProminententip`) is not built.
            Target::Button(c) if cx.dialog.controls[c].id == Some(TIP) => Outcome::Stay,
            _ => Outcome::Stay,
        }
    }
}
