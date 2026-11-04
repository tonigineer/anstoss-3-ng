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
use a3_core::cycle::Request;
use crate::pointer::Target;
use crate::view::View;

pub(crate) const MERCHANDISE_RANGE: u16 = 56;
/// An article of it (`CFanartKaufen`), and its question before it is
/// given up (text 3836).
const ARTICLE: u16 = 57;
const GIVE_UP_QUESTION: u16 = 3836;
/// The range's figures (`CFanartikelInfo`), behind 56's "Details".
const INFO: u16 = 372;
const INFO_BUTTON: u16 = 1506;

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
    /// The order size's place on its spinner, from 0.
    size: usize,
    /// The article last chosen in the list, by its place: the first at
    /// first; refilling the list drops its selection but not this (+0x2f64).
    chosen: usize,
    /// Hired when the dialog opened: those cannot be sent away again.
    hired: (bool, bool),
    /// An article's dialog is open, whose answer refills the rows.
    article: bool,
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
        cx.state.merchandise = range;
        self.show_discount(cx, view);
        self.show_size(cx, view);
        self.rows(cx, view);
        self.chart(cx, view);
        let count = cx.state.merchandise.as_ref().map_or(0, |r| r.articles().len());
        if let Some(c) = control(dialog, WEEKS_TITLE) {
            let text = super::finances::printf(&dialog.controls[c].text, &[&count]);
            view.set_text(c, text);
        }
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, double: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        match target {
            // The fans' discount (`FUN_0049bb71`): the prices anew.
            Target::Spin { control: c, .. } if id(c) == DISCOUNT_SPIN => {
                let v = view.value(c).map_or(DISCOUNT_ZERO, |v| v.value);
                if let Some(r) = cx.state.merchandise.as_mut() {
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
                    // A double click opens the article (`OnDblClk_…_LISTE`).
                    if double {
                        let place = u16::try_from(place).unwrap_or(0);
                        self.article = true;
                        return Outcome::Popup(Request::with(ARTICLE, Args::Record(place)));
                    }
                }
            }
            Target::Button(c) if id(c) == BUY_ONE => self.buy(cx, view, Some(self.chosen)),
            Target::Button(c) if id(c) == BUY_ALL => self.buy(cx, view, None),
            // `OnClicked_IDC_FANARTIKEL_KAUFEN_INFO`.
            Target::Button(c) if id(c) == INFO_BUTTON => return Outcome::Popup(Request::new(INFO)),
            Target::Button(c) if id(c) == OK => {
                self.write(cx, view);
                return Outcome::Close(Reply::Ok);
            }
            _ => {}
        }
        Outcome::Stay
    }

    /// Back from the article: the rows anew, whatever it answered.
    fn answer(&mut self, cx: &mut Cx, view: &mut View, _: Reply) -> Outcome {
        if std::mem::take(&mut self.article) {
            self.rows(cx, view);
        }
        Outcome::Stay
    }
}

impl MerchandiseRange {
    fn show_discount(&self, cx: &Cx, view: &mut View) {
        let dialog = cx.dialog;
        let discount = cx.state.merchandise.as_ref().map_or(0, Range::fans_discount);
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
        let Some(range) = &cx.state.merchandise else { return };
        let Some(records) = cx.state.records.as_ref() else { return };
        let tables = &game.rules.merchandise;
        let weeks = merchandise::delivery_weeks(records);
        let Some(c) = control(dialog, LIST) else { return };
        let slots = crate::controls::list::icon_slots(&dialog.controls[c]);
        let at = slots.get(ICON_SLOT).copied().unwrap_or(0.0);
        let (mut rows, mut icons, mut keys) = (Vec::new(), Vec::new(), Vec::new());
        for a in range.articles() {
            let kind = tables.kinds.get(a.kind).copied().unwrap_or_default();
            let name = article_name(cx, range, kind);
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
        let Some(mut range) = cx.state.merchandise.take() else { return };
        let weeks = cx.state.records.as_ref().map(merchandise::delivery_weeks).unwrap_or_default();
        let tables = &cx.game.rules.merchandise;
        let cost = match place {
            Some(p) => range.order(tables, &weeks, p, pieces).unwrap_or(0),
            None => range.order_all(tables, &weeks, pieces),
        };
        cx.state.merchandise = Some(range);
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
        let Some(mut range) = cx.state.merchandise.take() else { return };
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

/// An article's name, its kind's text with the club's names filled in
/// (`FUN_0061069a`).
fn article_name(cx: &Cx, range: &Range, kind: merchandise::Kind) -> String {
    let Some(records) = cx.state.records.as_ref() else { return String::new() };
    let Some(team) = super::squad::own_team(cx) else { return String::new() };
    let rival = u8::try_from(team.get::<i8>(RIVAL))
        .ok()
        .and_then(|n| records.team(team.country().into(), n.into()))
        .map_or_else(String::new, |t| t.short_name());
    range.name(cx.game.text(kind.name), &team.short_name(), &rival, records.globals.start_year())
}

/// An article of the range (dialog 57, `CFanartKaufen`), opened by a
/// double click on its row: its price among the steps its markup may
/// take, the kind's recommended and purchase prices, the stock, whether it
/// is left out of buying them all, its last ten weeks' sales as bars, and
/// — for any but the four fixed kinds — giving it up.
#[derive(Default)]
pub struct Article {
    place: usize,
    kind: usize,
    /// The prices it may take and the markup of each; the one shown.
    steps: Vec<(i32, i16)>,
    step: usize,
    /// Giving it up asked, waiting for the answer.
    asking: bool,
}

const NAME: u16 = 1509;
const CHART_10: u16 = 1510;
const PRICE: u16 = 1511;
const RECOMMENDED: u16 = 1512;
const PURCHASE: u16 = 1513;
const STOCK: u16 = 1514;
const EXCLUDED: u16 = 1515;
const PRICE_SPIN: u16 = 1516;
const GIVE_UP: u16 = 1517;
const CANCEL: u16 = 1518;
const BEST_WEEK: u16 = 1519;
/// Where its ten bars stand (`0x880d54`).
const ARTICLE_BARS: [i32; 10] = [10, 26, 42, 58, 73, 88, 103, 119, 135, 150];

impl Screen for Article {
    fn ids(&self) -> &[u16] {
        &[ARTICLE]
    }

    /// `FUN_0049d381`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        *self = Self::default();
        // Its OK keeps its own picture: it does not ask for the day's.
        set(view, dialog, CAPTION_ID, dialog.title.clone());
        let Args::Record(place) = args else { return };
        self.place = usize::from(place);
        let Some(range) = cx.state.merchandise.as_ref() else { return };
        let Some(a) = range.article(self.place) else { return };
        self.kind = a.kind;
        let kind = game.rules.merchandise.kinds.get(a.kind).copied().unwrap_or_default();
        self.steps = merchandise::price_steps(kind.sold_at);
        // The step whose first markup is the article's; the original reads
        // a stale index where none is.
        self.step = self.steps.iter().position(|&(_, m)| m == a.markup).unwrap_or(0);
        set(view, dialog, NAME, article_name(cx, range, kind));
        set(view, dialog, STOCK, a.stock.to_string());
        set(view, dialog, RECOMMENDED, cents(kind.sold_at));
        set(view, dialog, PURCHASE, cents(kind.bought_at));
        if let Some(spin) = control(dialog, PRICE_SPIN).and_then(|c| view.value_mut(c)) {
            spin.range = (0, self.steps.len() as i32 - 1);
            spin.set(self.step as i32);
        }
        self.show_price(cx, view);
        if let Some(check) = control(dialog, EXCLUDED).and_then(|c| view.check_mut(c)) {
            check.checked = a.excluded;
        }
        if kind.fixed & 1 != 0
            && let Some(c) = control(dialog, GIVE_UP)
        {
            view.controls[c].hidden = true;
        }
        // The chart (`FUN_0049d9b8`): the best week told beside its label.
        let sales = range.sales(self.place);
        let best = sales.iter().copied().max().unwrap_or(0);
        if let Some(c) = control(dialog, BEST_WEEK) {
            if best < 1 {
                view.controls[c].hidden = true;
            } else {
                view.set_text(c, format!("{} {best}", dialog.controls[c].text));
            }
        }
        if best >= 1
            && let Some(c) = control(dialog, CHART_10)
        {
            let height = dialog.controls[c].rect.height() as i32;
            let scale = merchandise::chart_scale(best);
            if let Some(paint) = view.paint_mut(c) {
                paint.bars.clear();
                for (&x, &v) in ARTICLE_BARS.iter().zip(&sales) {
                    let h = (height - BAR_TOP_GAP).wrapping_mul(v) / scale;
                    paint.bar(x, height + 1 - h, BAR_WIDTH, h, BAR_COLOUR);
                }
            }
        }
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        match target {
            // A price step (`FUN_0049d94b`).
            Target::Spin { control: c, .. } if id(c) == PRICE_SPIN => {
                self.step = view.value(c).map_or(0, |v| v.value).max(0) as usize;
                self.show_price(cx, view);
            }
            // OK keeps the markup and the box (`OnClicked_IDOK_0049d842`,
            // then `0x49b9c9`).
            Target::Button(c) if id(c) == OK => {
                let excluded =
                    control(dialog, EXCLUDED).and_then(|e| view.check(e)).is_some_and(|e| e.checked);
                let markup = self.steps.get(self.step).map_or(0, |&(_, m)| m);
                if let Some(range) = cx.state.merchandise.as_mut() {
                    range.set_article(self.place, markup, excluded);
                }
                return Outcome::Close(Reply::Ok);
            }
            Target::Button(c) if id(c) == CANCEL => return Outcome::Close(Reply::Cancel),
            Target::Button(c) if id(c) == GIVE_UP => {
                self.asking = true;
                return Outcome::Popup(Request::with(
                    super::popups::QUESTION,
                    Args::Text(GIVE_UP_QUESTION),
                ));
            }
            _ => {}
        }
        Outcome::Stay
    }

    /// Asked to give it up (`OnClicked_IDC_FANA_K_LOESCHEN`): yes, and the
    /// range lets the kind go (`FUN_006112a9`).
    fn answer(&mut self, cx: &mut Cx, _: &mut View, reply: Reply) -> Outcome {
        if !std::mem::take(&mut self.asking) || reply != Reply::Ok {
            return Outcome::Stay;
        }
        let tables = &cx.game.rules.merchandise;
        if let Some(range) = cx.state.merchandise.as_mut() {
            range.give_up(tables, self.kind);
        }
        Outcome::Close(Reply::Ok)
    }
}

impl Article {
    fn show_price(&self, cx: &Cx, view: &mut View) {
        let price = self.steps.get(self.step).map_or(0, |&(p, _)| p);
        set(view, cx.dialog, PRICE, cents(price));
    }
}

/// The range's figures (dialog 372, `CFanartikelInfo`): the season's
/// takings, what the stock was worth at its start, its costs, the profit
/// and the profit planned.
#[derive(Default)]
pub struct Info;

const TAKINGS: u16 = 4675;
const STOCK_AT_START: u16 = 4676;
const COSTS: u16 = 4677;
const PROFIT: u16 = 4678;
const PLANNED_PROFIT: u16 = 4679;
/// The merchandise sold, as booked, and as planned (club +0x30c, +0x204).
const SOLD_BOOKED: usize = 0x30c;
const SOLD_PLANNED: usize = 0x204;

impl Screen for Info {
    fn ids(&self) -> &[u16] {
        &[INFO]
    }

    /// `FUN_0049a73e`: the plan's figure is the one 56 shows in the
    /// season (its flag +0x2f90 set).
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        set(view, dialog, CAPTION_ID, dialog.title.clone());
        let Some(team) = super::squad::own_team(cx) else { return };
        let club = |o: usize| team.get::<i32>(o + a3_core::finances::CLUB_TO_TEAM);
        let money = |v: i32| super::playerinfo::money(game, v.into());
        let (sold, bought) = (club(SOLD_BOOKED), club(BOUGHT));
        let at_start = cx.state.merchandise.as_ref().map_or(0, Range::stock_value_at_start);
        set(view, dialog, TAKINGS, money(sold));
        set(view, dialog, STOCK_AT_START, money(at_start));
        set(view, dialog, COSTS, money(bought));
        set(view, dialog, PROFIT, money(sold.wrapping_sub(bought)));
        set(view, dialog, PLANNED_PROFIT, money(club(SOLD_PLANNED)));
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(c) if cx.dialog.controls[c].id == Some(OK) => Outcome::Close(Reply::Ok),
            _ => Outcome::Stay,
        }
    }
}

/// Puts the dialogs' captions in their frames.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    for id in [MERCHANDISE_RANGE, ARTICLE, INFO] {
        let Some(dialog) = dialogs.get_mut(&id) else { continue };
        let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
        dialog.controls.insert(0, caption);
    }
}
