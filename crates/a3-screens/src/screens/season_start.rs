//! The season start's screens at the season's turn: the event 0x82 each
//! human club's manager gets (`FUN_00671c60`, run by
//! `CMyStartDialog::FUN_00671d57`) — the negotiations he leads himself
//! (dialog 422, `CSaisonstartVerhandlungen`), the European television's
//! talks behind them (dialog 475, `CDiVerhandlungenUefa`) and the season's
//! preparation (dialog 419, `CVorbereitung`) — and the event 0x130 behind them
//! (`FUN_0062def0`, run by `CMyStartDialog::FUN_0062de93`): the overviews
//! of the season's start (dialog 420, `CSaisonstartAuslosungen`).
//!
//! The negotiations' boxes show the talks the manager may lead, ticked or
//! off as the original sets them ([`a3_core::finances::season_talks`]),
//! and tick and untick; OK hands their ticks to the cycle, which has the
//! computer sign the talks left unticked
//! ([`a3_core::settlement::sign_left_talks`]) and opens the European
//! television's talks for a kept box. There three of the country's
//! stations bid ([`a3_core::settlement::tv_bids`]); "Verhandeln" raises a
//! station's bid until its patience runs out and it breaks off
//! ([`a3_core::settlement::tv_bargain`]), "Unterschreiben" signs it and
//! ends the talks. The preparation's figures and choices and the
//! overviews are not built, so the value fields the original fills stay
//! empty and every other control is inert.

use std::collections::HashMap;

use a3_core::cycle::dialogs;
use a3_core::finances::season_talks;
use a3_core::settlement::{TV_PATIENCE, TvBid, sign_tv_bid, tv_bargain, tv_bids, tv_terms};
use a3_files::rch::Dialog;

use super::discipline::CAPTION_ID;
use super::squad::{own_team, own_team_mut};
use super::{Args, Cx, Outcome, Reply, Request, Screen, control, mark_inert, set};
use crate::controls::ControlState;
use crate::controls::bitmap::{BitmapState, Picture, PictureSource};
use crate::controls::options::NONE;
use crate::pointer::Target;
use crate::view::View;

pub const TALKS: u16 = dialogs::SEASON_START_TALKS;
pub const PREPARATION: u16 = dialogs::SEASON_PREPARATION;
pub const OVERVIEWS: u16 = dialogs::SEASON_OVERVIEWS;
pub const EURO_TV: u16 = dialogs::EURO_TV_TALKS;
const OK: u16 = 1;
/// The European television's three offers, a station's name and its sum
/// each, which the file holds samples in ("RTL plus", "6.000.000 DM
/// fest").
const EURO_TV_OFFERS: [u16; 6] = [6051, 6052, 6053, 6054, 6055, 6056];
/// The question the European television's OK asks.
const EURO_TV_LEAVE: u16 = 0x1251;
/// The three stations, left to right: their names, their offers, their
/// faces and their groups of "Verhandeln" and "Unterschreiben"
/// (`CDiVerhandlungenUefa::FUN_005ba38d`, `FUN_005ba500`).
const TV_NAMES: [u16; 3] = [6051, 6055, 6053];
const TV_OFFERS: [u16; 3] = [6052, 6056, 6054];
const TV_FACES: [u16; 3] = [5780, 5781, 6057];
const TV_GROUPS: [u16; 3] = [6058, 6059, 6060];
const BARGAIN: usize = 0;
/// An offer's words (`FUN_005b9db0`): a fixed sum alone, both, a sum per
/// home match alone; "---" for a station that broke off (`DAT_0088c644`),
/// which its group's labels turn to as well (`DAT_0088c648`, `0x88c64c`).
const FIXED_ALONE: u16 = 0x11df;
const FIXED_AND_PER_HOME: u16 = 0x11e0;
const PER_HOME_ALONE: u16 = 0x11e1;
const BROKEN_OFF: &str = "---";
/// The notice before a signed bid closes the talks (`FUN_005ba88c`).
const SIGNED: u16 = 0x11e2;
/// The faces' frame by the station's strain (`DAT_0088c600`), the last one
/// for a station that broke off (`DAT_0088c61e`).
const FACE_BY_STRAIN: [usize; 16] = [0, 3, 7, 11, 15, 19, 23, 24, 31, 35, 39, 43, 47, 51, 55, 59];
/// The left station's face is the graphics' sixty frames from 1620
/// (`DAT_0089f6d8`); the others' are the morph pictures of the
/// television and of the career (`FUN_00795ba7` with kinds 2 and 6:
/// "morph/%s%02d.jpg", counted from one).
const FIRST_FACE: u16 = 1620;
const FACE_COUNT: u16 = 60;
const MORPH_FACES: [&str; 3] = ["", "Karriere/Krfr", "Fernsehen/Tvrt"];

/// The negotiations' boxes (`FUN_0053af88`): main sponsor, boards, kit,
/// player bonuses, souvenirs, European television.
const TALK_BOXES: [u16; 6] = [0x889, 0x88a, 0x88b, 0x88c, 0x179b, 0x179c];
/// The television's box, its "%s" filled with nothing (`DAT_00ae66ce`).
const EURO_TV_BOX: u16 = 0x179c;

/// The preparation's value fields, which the file holds sample figures in
/// (`CVorbereitung::FUN_005c79c5`'s): the two coaches proposed, the
/// fitness coaches' count, the four departments' investments, the
/// proposals for the trips and the shirts, the club's money, its
/// reckoned profit and its credit limit.
const PREPARATION_VALUES: [u16; 12] = [5120, 5121, 5111, 5106, 5107, 5108, 5109, 5816, 5815, 5048, 5049, 5050];

/// The sample figures go, and the labels the original completes with a
/// name or a sum keep their words without the gap; the European
/// television's talks get their caption.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    if let Some(dialog) = dialogs.get_mut(&EURO_TV) {
        dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, dialog.title.clone()));
    }
    for id in [TALKS, PREPARATION, OVERVIEWS, EURO_TV] {
        let Some(dialog) = dialogs.get_mut(&id) else { continue };
        for c in &mut dialog.controls {
            let samples: &[u16] = match id {
                PREPARATION => &PREPARATION_VALUES,
                EURO_TV => &EURO_TV_OFFERS,
                _ => &[],
            };
            if c.id.is_some_and(|i| samples.contains(&i)) {
                c.text.clear();
            } else if c.id == Some(EURO_TV_BOX) {
                c.text = c.text.replace("%s", "");
            } else if c.id.is_some() && c.text.contains("%s") {
                c.text = c.text.replace(" %s", "").replace("%s", "");
            }
        }
    }
}

#[derive(Default)]
pub struct SeasonStart {
    /// The European television's three bids.
    bids: [TvBid; 3],
    /// What the open popup's answer leads to.
    pending: Pending,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Pending {
    /// The question before the talks close.
    #[default]
    Leave,
    /// A station broke off.
    BrokenOff(usize),
    /// A station's bid is signed.
    Signed(usize),
}

impl Screen for SeasonStart {
    fn ids(&self) -> &[u16] {
        &[TALKS, PREPARATION, OVERVIEWS, EURO_TV]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        // The European television's talks name the manager and his club
        // after their title.
        if view.id == EURO_TV {
            let manager = cx.state.season.as_ref().and_then(|s| s.manager());
            let who = manager.map_or_else(String::new, |m| format!("{} {}", m.first_name, m.last_name));
            let club = manager
                .and_then(|m| m.club)
                .and_then(|(slot, c)| cx.game.world.country(slot)?.clubs.get(c))
                .map_or_else(String::new, |c| c.name.clone());
            set(view, dialog, CAPTION_ID, format!("{} {who} ({club})", dialog.title));
            self.open_tv_talks(cx, view);
        }
        // Only the negotiations hold the boxes.
        let talks = control(dialog, TALK_BOXES[0])
            .and(own_team(cx).zip(cx.state.records.as_ref()))
            .map(|(team, records)| season_talks(team, records.globals.preparation()));
        if let Some(talks) = talks {
            for (id, talk) in TALK_BOXES.into_iter().zip(talks) {
                let Some(c) = control(dialog, id) else { continue };
                if let Some(check) = view.check_mut(c) {
                    (check.checked, check.disabled) = (talk.ticked, !talk.open);
                }
            }
        }
        mark_inert(view, dialog, |t| match t {
            Target::Button(c) => dialog.controls[c].id == Some(OK),
            Target::Check(c) => dialog.controls[c].id.is_some_and(|i| TALK_BOXES.contains(&i)),
            Target::Option { control: c, .. } => dialog.controls[c].id.is_some_and(|i| TV_GROUPS.contains(&i)),
            _ => false,
        });
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            // The negotiations' ticks go back one bit each, in the boxes'
            // order (`FUN_0053b3ed` keeps them at +0xa08 … +0xa1c).
            Target::Button(c) if control(cx.dialog, OK) == Some(c) && view.id == TALKS => {
                let ticks = TALK_BOXES.iter().enumerate().fold(0, |mask, (k, &id)| {
                    let ticked = control(cx.dialog, id).and_then(|b| view.check(b)).is_some_and(|b| b.checked);
                    mask | usize::from(ticked) << k
                });
                Outcome::Close(Reply::Choice(ticks))
            }
            // The European television's talks ask before they close
            // (`OnClicked_IDOK_005ba6d2`: `CAlertOkAbbruch` with text 0x1251).
            Target::Button(c) if control(cx.dialog, OK) == Some(c) && view.id == EURO_TV => {
                self.pending = Pending::Leave;
                Outcome::Popup(Request::with(dialogs::QUESTION, Args::Text(EURO_TV_LEAVE)))
            }
            Target::Button(c) if control(cx.dialog, OK) == Some(c) => Outcome::Close(Reply::Ok),
            Target::Option { control: c, item } if view.id == EURO_TV => {
                let Some(k) = TV_GROUPS.iter().position(|&id| control(cx.dialog, id) == Some(c)) else {
                    return Outcome::Stay;
                };
                self.tv_choice(cx, view, k, item)
            }
            _ => Outcome::Stay,
        }
    }

    /// The question's OK closes the European television's talks, its
    /// cancel keeps them; a station's breaking off taken in, its offer and
    /// group go dead (`FUN_005ba04d`); the signing's notice taken in, the
    /// bid is the club's and the talks close with OK (`FUN_005ba88c`).
    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        match std::mem::take(&mut self.pending) {
            Pending::Leave if reply == Reply::Ok => Outcome::Close(Reply::Ok),
            Pending::Leave => Outcome::Stay,
            Pending::BrokenOff(k) => {
                if let Some(group) = control(cx.dialog, TV_GROUPS[k]).and_then(|c| view.options_mut(c)) {
                    group.labels.iter_mut().for_each(|l| *l = BROKEN_OFF.to_owned());
                    group.dimmed = vec![true; group.labels.len()];
                    group.chosen = NONE;
                }
                self.show_bid(cx, view, k);
                Outcome::Stay
            }
            Pending::Signed(k) => {
                if let Some(team) = own_team_mut(cx) {
                    sign_tv_bid(team, &self.bids[k], k as u8);
                }
                Outcome::Close(Reply::Ok)
            }
        }
    }
}

impl SeasonStart {
    /// The talks' opening (`FUN_005ba500`): the stations' bids drawn, and
    /// each station's name, face and offer shown, nothing chosen in its
    /// group (`FUN_005b9b1d`).
    fn open_tv_talks(&mut self, cx: &mut Cx, view: &mut View) {
        self.pending = Pending::Leave;
        let country = own_team(cx).map(|t| t.country());
        let names: Vec<Option<String>> = (0..3)
            .map(|k| Some(a3_core::report::country_station(cx.state.records.as_ref()?, country?, k)?))
            .collect();
        self.bids = cx.state.season.as_mut().map_or_else(Default::default, |s| tv_bids(&mut s.random));
        for (k, name) in names.into_iter().enumerate() {
            set(view, cx.dialog, TV_NAMES[k], name.unwrap_or_default());
            if let Some(group) = control(cx.dialog, TV_GROUPS[k]).and_then(|c| view.options_mut(c)) {
                group.chosen = NONE;
            }
            self.show_bid(cx, view, k);
        }
    }

    /// A click in a station's group, which the original reads by its
    /// chosen item (`FUN_007a71f7`): "Verhandeln" bargains, the station
    /// raising its bid or breaking off before the notice of it
    /// (`CVerhandlGescheitert`, dialog 102), and lets the item go again
    /// (`FUN_007a71d7`); "Unterschreiben" signs after its notice.
    fn tv_choice(&mut self, cx: &mut Cx, view: &mut View, k: usize, item: usize) -> Outcome {
        if item != BARGAIN {
            self.pending = Pending::Signed(k);
            return Outcome::Popup(Request::with(dialogs::NOTICE, Args::Text(SIGNED)));
        }
        let Some(season) = cx.state.season.as_mut() else { return Outcome::Stay };
        let stays = tv_bargain(&mut self.bids[k], &mut season.random);
        face(cx.dialog, view, k, self.bids[k].strain);
        if !stays {
            self.pending = Pending::BrokenOff(k);
            return Outcome::Popup(Request::new(dialogs::FAILED));
        }
        self.show_bid(cx, view, k);
        if let Some(group) = control(cx.dialog, TV_GROUPS[k]).and_then(|c| view.options_mut(c)) {
            group.chosen = NONE;
        }
        Outcome::Stay
    }

    /// A station's face and offer as its bid stands (`FUN_005b9db0`).
    fn show_bid(&self, cx: &Cx, view: &mut View, k: usize) {
        let bid = &self.bids[k];
        face(cx.dialog, view, k, bid.strain);
        let (fixed, per_home) = tv_terms(bid);
        let money = |v: i32| super::playerinfo::money(cx.game, i64::from(v));
        let text = if bid.strain >= TV_PATIENCE {
            BROKEN_OFF.to_owned()
        } else if bid.per_home == 0 {
            cx.game.text(FIXED_ALONE).replacen("%s", &money(fixed), 1)
        } else if bid.fixed == 0 {
            cx.game.text(PER_HOME_ALONE).replacen("%s", &money(per_home), 1)
        } else {
            cx.game.text(FIXED_AND_PER_HOME).replacen("%s", &money(fixed), 1).replacen("%s", &money(per_home), 1)
        };
        set(view, cx.dialog, TV_OFFERS[k], text);
    }
}

/// A station's face at the frame its strain puts it on: the left one an
/// animation of graphics, the others a morph's picture each.
fn face(dialog: &Dialog, view: &mut View, k: usize, strain: i32) {
    let Some(c) = control(dialog, TV_FACES[k]) else { return };
    let frame = FACE_BY_STRAIN[usize::try_from(strain).unwrap_or(0).min(FACE_BY_STRAIN.len() - 1)];
    if k == 0 {
        if let Some(anim) = view.anim_mut(c) {
            anim.frames = (FIRST_FACE..FIRST_FACE + FACE_COUNT).collect();
            anim.frame = frame;
        }
        return;
    }
    let path = format!("morph/{}{:02}.jpg", MORPH_FACES[k], frame + 1);
    let picture = Picture { source: PictureSource::Archive(path), part: None, black_clear: false };
    view.controls[c].state = ControlState::Bitmap(BitmapState { picture: Some(picture), ..BitmapState::default() });
}
