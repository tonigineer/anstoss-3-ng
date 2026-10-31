//! The club's own merchandise (dialog 56, `CFanartikelKaufen`), behind
//! the merchandise calculation's "Details" (115's 2204; KALKULATION.md
//! §5.1 in the RE repo): the range in a list with its prices, stock and
//! orders, the buyer and the marketing man, the order size with its
//! rebate, the fans' discount, the last weeks' sales as bars, and the
//! buying of one article or all. The dialog works on a copy of the range
//! that its OK writes back ([`a3_core::merchandise`]).

use std::collections::HashMap;

use a3_core::merchandise::{
    self, BUYER_WAGE, FIRST_ORDER_SIZE, MARKETING_WAGE, ORDER_SIZES, Range,
};
use a3_files::rch::Dialog;

use super::discipline::CAPTION_ID;
use super::honours::market::cents;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::pointer::Target;
use crate::view::View;

pub(crate) const MERCHANDISE_RANGE: u16 = 56;

const LIST: u16 = 1495;
const BUYER: u16 = 1496;
const MARKETING: u16 = 1497;
const ORDER_SIZE: u16 = 1498;
const ORDER_SPIN: u16 = 1499;
const REBATE: u16 = 1500;
const DISCOUNT_LABEL: u16 = 1501;
const DISCOUNT: u16 = 1502;
const DISCOUNT_SPIN: u16 = 1503;
const BUY_ALL: u16 = 1504;
const BUY_ONE: u16 = 1505;
const WEEKS_TITLE: u16 = 1507;
const CHART: u16 = 1508;
const OK: u16 = 1;
/// The fans' discount sits on its spinner fifty up, from a surcharge of
/// 50% to a discount of 50%; its label tells which (`0x880c34`,
/// `0x880c48`).
const DISCOUNT_ZERO: i32 = 50;
const SURCHARGE_TEXT: &str = "Zuschlag für Fans";
const DISCOUNT_TEXT: &str = "Rabatt für Fans";
/// The chart's last eight weeks: where each bar stands, how wide, its
/// colour (`0x880bf0`, `FUN_0049b354`).
const CHART_BARS: [i32; 8] = [6, 22, 37, 52, 68, 83, 98, 114];
const CHART_WEEKS: usize = 8;
const BAR_WIDTH: i32 = 10;
const BAR_TOP_GAP: i32 = 10;
const BAR_COLOUR: u32 = 0x6666ff;
/// The kinds' pictures in the list's rows, at its left edge, black and
/// all (`FUN_007a95f6`, `FUN_007a96c5`).
const ICON_SLOT: usize = 0;

#[derive(Default)]
pub struct MerchandiseRange {
    range: Option<Range>,
    /// The order size's place on its spinner, from 0.
    size: usize,
    /// The article last chosen in the list, by its place: the first at
    /// first; refilling the list drops its selection but not this (+0x2f64).
    chosen: usize,
    /// Hired when the dialog opened: those cannot be sent away again.
    hired: (bool, bool),
}

impl Screen for MerchandiseRange {
    fn ids(&self) -> &[u16] {
        &[MERCHANDISE_RANGE]
    }

    /// `FUN_0049afc0`: the range copied, the spinners set, a hired man's
    /// box ticked and locked, the rows and the chart.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        *self = Self { size: FIRST_ORDER_SIZE, ..Self::default() };
        set(view, dialog, CAPTION_ID, dialog.title.clone());
        super::seasonal_buttons(cx, view);
        let range = cx
            .state
            .records
            .as_ref()
            .zip(super::squad::own_team(cx))
            .and_then(|(records, team)| Range::of(records, team));
        if let Some(r) = &range {
            self.hired = (r.buyer(), r.marketing());
            for (id, on) in [(BUYER, r.buyer()), (MARKETING, r.marketing())] {
                if let Some(check) = control(dialog, id).and_then(|c| view.check_mut(c)) {
                    check.checked = on;
                    check.disabled = on;
                }
            }
            if let Some(spin) = control(dialog, DISCOUNT_SPIN).and_then(|c| view.value_mut(c)) {
                spin.range = (0, 2 * DISCOUNT_ZERO);
                spin.set(i32::from(r.fans_discount()) + DISCOUNT_ZERO);
            }
        }
        if let Some(spin) = control(dialog, ORDER_SPIN).and_then(|c| view.value_mut(c)) {
            spin.range = (0, ORDER_SIZES.len() as i32 - 1);
            spin.set(self.size as i32);
        }
        self.range = range;
        self.show_discount(cx, view);
        self.show_size(cx, view);
        self.rows(cx, view);
        self.chart(cx, view);
        let count = self.range.as_ref().map_or(0, |r| r.articles().len());
        if let Some(c) = control(dialog, WEEKS_TITLE) {
            let text = super::finances::printf(&dialog.controls[c].text, &[&count]);
            view.set_text(c, text);
        }
        // The article's own dialog (CFanartKaufen, a double click) and its
        // information (CFanartikelInfo) are not built.
        mark_inert(view, dialog, |t| match t {
            Target::Button(c) => matches!(dialog.controls[c].id, Some(OK | BUY_ONE | BUY_ALL)),
            _ => false,
        });
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        match target {
            // The fans' discount (`FUN_0049bb71`): the prices anew.
            Target::Spin { control: c, .. } if id(c) == DISCOUNT_SPIN => {
                let v = view.value(c).map_or(DISCOUNT_ZERO, |v| v.value);
                if let Some(r) = self.range.as_mut() {
                    r.set_fans_discount((v - DISCOUNT_ZERO) as i16);
                }
                self.show_discount(cx, view);
                self.rows(cx, view);
            }
            // The order size (`FUN_0049bc6b`).
            Target::Spin { control: c, .. } if id(c) == ORDER_SPIN => {
                self.size = view.value(c).map_or(0, |v| v.value).clamp(0, 10) as usize;
                self.show_size(cx, view);
            }
            // A row chosen (`OnSelChange_IDC_FANARTIKEL_KAUFEN_LISTE`).
            Target::Row { control: c, .. } if id(c) == LIST => {
                if let Some(place) =
                    view.list(c).and_then(|l| l.selected.and_then(|s| l.keys.get(s).copied()))
                {
                    self.chosen = place;
                }
            }
            Target::Button(c) if id(c) == BUY_ONE => self.buy(cx, view, Some(self.chosen)),
            Target::Button(c) if id(c) == BUY_ALL => self.buy(cx, view, None),
            Target::Button(c) if id(c) == OK => {
                self.write(cx, view);
                return Outcome::Close(Reply::Ok);
            }
            _ => {}
        }
        Outcome::Stay
    }
}

impl MerchandiseRange {
    fn show_discount(&self, cx: &Cx, view: &mut View) {
        let dialog = cx.dialog;
        let discount = self.range.as_ref().map_or(0, Range::fans_discount);
        set(view, dialog, DISCOUNT, format!("{}%", discount.unsigned_abs()));
        let label = if discount < 0 { SURCHARGE_TEXT } else { DISCOUNT_TEXT };
        set(view, dialog, DISCOUNT_LABEL, label.to_owned());
    }

    fn show_size(&self, cx: &Cx, view: &mut View) {
        let dialog = cx.dialog;
        let pieces = ORDER_SIZES[self.size];
        set(view, dialog, ORDER_SIZE, pieces.to_string());
        set(view, dialog, REBATE, format!("{}%", merchandise::rebate(pieces)));
    }

    /// The rows (`FUN_0049b529`): name, bought at, sold at, in stock,
    /// ordered, sold last week, taken, the weeks an order takes; each with
    /// its kind's picture.
    fn rows(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(range) = &self.range else { return };
        let Some(records) = cx.state.records.as_ref() else { return };
        let Some(team) = super::squad::own_team(cx) else { return };
        let tables = &game.rules.merchandise;
        let weeks = merchandise::delivery_weeks(records);
        let club = team.short_name();
        let rival = u8::try_from(team.get::<i8>(RIVAL))
            .ok()
            .and_then(|n| records.team(team.country().into(), n.into()));
        let rival = rival.map_or_else(String::new, |t| t.short_name());
        let year = records.globals.start_year();
        let Some(c) = control(dialog, LIST) else { return };
        let slots = crate::controls::list::icon_slots(&dialog.controls[c]);
        let at = slots.get(ICON_SLOT).copied().unwrap_or(0.0);
        let (mut rows, mut icons, mut keys) = (Vec::new(), Vec::new(), Vec::new());
        for a in range.articles() {
            let kind = tables.kinds.get(a.kind).copied().unwrap_or_default();
            let name = range.name(game.text(kind.name), &club, &rival, year);
            let lz = weeks.get(a.kind).copied().unwrap_or(0);
            rows.push(format!(
                "{name}\t{}\t{}\t{}\t{}\t{}\t{}\t{lz}",
                cents(kind.bought_at),
                cents(range.price(tables, &a)),
                a.stock,
                a.ordered,
                a.sold,
                a.taken,
            ));
            icons.push(vec![(at, kind.picture)]);
            keys.push(a.place);
        }
        if let Some(list) = view.list_mut(c) {
            list.selected = None;
            list.rows = rows;
            list.row_icons = icons;
            list.keys = keys;
        }
    }

    /// The last eight weeks' sales as bars, the best one full high
    /// (`FUN_0049b354`).
    fn chart(&self, cx: &Cx, view: &mut View) {
        let dialog = cx.dialog;
        let Some(c) = control(dialog, CHART) else { return };
        let height = dialog.controls[c].rect.height() as i32;
        let weeks = cx
            .state
            .records
            .as_ref()
            .zip(super::squad::own_team(cx))
            .and_then(|(records, team)| {
                let slot = usize::from(team.human()).checked_sub(1)?;
                records.extras.get(slot).map(|e| e.merch_weeks())
            })
            .unwrap_or_default();
        let last = &weeks[weeks.len().saturating_sub(CHART_WEEKS)..];
        let best = last.iter().copied().max().unwrap_or(0);
        let Some(paint) = view.paint_mut(c) else { return };
        paint.bars.clear();
        if best > 0 {
            for (&x, &v) in CHART_BARS.iter().zip(last) {
                let h = (height - BAR_TOP_GAP).wrapping_mul(v) / best;
                paint.bar(x, height + 1 - h, BAR_WIDTH, h, BAR_COLOUR);
            }
        }
    }

    /// "Einzeln" and "Alle" (`OnClicked_IDC_FANARTIKEL_KAUFEN_EINZELN`,
    /// `_ALLE`): the order size of the chosen article or of every one; what
    /// it costs is booked at once, the pieces wait in the copy.
    fn buy(&mut self, cx: &mut Cx, view: &mut View, place: Option<usize>) {
        let pieces = ORDER_SIZES[self.size];
        let Some(range) = self.range.as_mut() else { return };
        let weeks = cx.state.records.as_ref().map(merchandise::delivery_weeks).unwrap_or_default();
        let tables = &cx.game.rules.merchandise;
        let cost = match place {
            Some(p) => range.order(tables, &weeks, p, pieces).unwrap_or(0),
            None => range.order_all(tables, &weeks, pieces),
        };
        if let Some(team) = super::squad::own_team_mut(cx) {
            let at = BOUGHT + a3_core::finances::CLUB_TO_TEAM;
            team.put(at, team.get::<i32>(at).wrapping_add(cost));
        }
        self.rows(cx, view);
    }

    /// OK (`OnClicked_IDOK_0049b870`): a man newly hired adds his wage for
    /// the season's months to come to the staff's; the range is written
    /// back.
    fn write(&mut self, cx: &mut Cx, view: &View) {
        let dialog = cx.dialog;
        let checked = |id| control(dialog, id).and_then(|c| view.check(c)).is_some_and(|c| c.checked);
        let (buyer, marketing) = (checked(BUYER), checked(MARKETING));
        let Some(mut range) = self.range.take() else { return };
        range.set_buyer(buyer);
        range.set_marketing(marketing);
        let months = cx.state.records.as_ref().map_or(0, |r| {
            let month = usize::from(r.globals.date()[1]);
            cx.game.rules.forecast.months_left(month)
        });
        let mut wages = 0;
        if marketing && !self.hired.1 {
            wages += merchandise::rest_of_season(MARKETING_WAGE, months);
        }
        if buyer && !self.hired.0 {
            wages += merchandise::rest_of_season(BUYER_WAGE, months);
        }
        if let Some(team) = super::squad::own_team_mut(cx) {
            let at = STAFF_WAGES + a3_core::finances::CLUB_TO_TEAM;
            team.put(at, team.get::<i32>(at).wrapping_add(wages));
        }
        let slot = super::squad::own_team(cx).and_then(|t| usize::from(t.human()).checked_sub(1));
        if let (Some(records), Some(slot)) = (cx.state.records.as_mut(), slot) {
            range.write(records, slot);
        }
    }
}

/// The club's rival, a team of its country (club +0xf4 from team +0x122,
/// `FUN_0063b75a`; −1 for none).
const RIVAL: usize = 0x122;
/// The merchandise bought and the staff's wages, as booked (club +0x308,
/// +0x280).
const BOUGHT: usize = 0x308;
const STAFF_WAGES: usize = 0x280;

/// Puts the dialog's caption in its frame.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&MERCHANDISE_RANGE) else { return };
    let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
    dialog.controls.insert(0, caption);
}
