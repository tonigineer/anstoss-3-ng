//! The share market (dialog 431 `CDiAktienmarkt`; ERFOLGE.md, "The share
//! market"), behind the honours page's "A": the shares of football clubs
//! or of companies with their prices, the manager's holdings of them, and
//! his totals; a share chosen shows its last ten weeks and can be bought,
//! a holding can be sold, each with a slider for how many.

use std::collections::HashMap;

use a3_core::market::{self, Holding, Market, Share};
use a3_files::rch::Dialog;

use crate::controls::ControlState;
use crate::controls::options::NONE;
use crate::controls::paint::PaintState;
use crate::game::Game;
use crate::pointer::Target;
use crate::screens::finances::{count, printf};
use crate::screens::playerinfo::money;
use crate::screens::{Args, Cx, Outcome, Reply, Request, Screen, caption, control, mark_inert, set};
use crate::view::View;
use a3_core::cycle::dialogs::NOTICE;

pub(crate) const MARKET_DIALOG: u16 = 431;
const CAPTION_ID: u16 = 9947;
/// The filter, football clubs or companies; the index and its tendency;
/// the private fortune.
const FILTER: u16 = 5653;
const INDEX: u16 = 5139;
const INDEX_TENDENCY: u16 = 5286;
const WEALTH: u16 = 5282;
/// The two lists.
const SHARES: u16 = 5278;
const HOLDINGS: u16 = 5276;
/// Buying and selling: the chart with its top and bottom price, the count
/// and its slider, what it comes to, and the group of chart and trade.
struct Side {
    chart: u16,
    top: u16,
    bottom: u16,
    count: u16,
    slider: u16,
    amount: u16,
    trade: u16,
}
const BUY: Side =
    Side { chart: 5279, top: 5294, bottom: 5295, count: 5281, slider: 5280, amount: 5287, trade: 5654 };
const SELL: Side =
    Side { chart: 5283, top: 5296, bottom: 5297, count: 5285, slider: 5284, amount: 5288, trade: 5655 };
/// The trade groups' items: the big chart (432), and the trade itself.
const TRADE: usize = 1;
/// The week's winners and losers (433).
const WINNERS: u16 = 5292;
/// The holdings' totals: their tendency, profit and value.
const TOTAL_TENDENCY: u16 = 5326;
const PROFIT: u16 = 5334;
const VOLUME: u16 = 5327;

/// Texts of the game.
const SUSPENDED: u16 = 4513;
const NO_MORE: u16 = 4598;
const NOT_ALL: u16 = 4487;
const VALUES: [u16; 3] = [2907, 2906, 2905];
const CAPTION: &str = "%s %s %s";
const NOTHING: &str = " ";
const NO_TENDENCY: &str = "---";
/// Red for a loss, blue for the rest (`0x0000ff`, `0xff0000`).
const RED: [u8; 3] = [255, 0, 0];
const BLUE: [u8; 3] = [0, 0, 255];
/// A holding's row shows its value as money above this much.
const AS_MONEY: i32 = 100_000;
const AS_VALUE: f64 = 1000.0;

/// The chart (`FUN_0043aeaa`): ten weeks here, all 52 on the big chart,
/// white bars a pixel apart; blue through the last two peaks, red through
/// the last two troughs; black at the price a holding was bought at.
const CHART_WEEKS: usize = 10;
const WHITE: u32 = 0xffffff;
const PEAKS: u32 = 0xff0000;
const TROUGHS: u32 = 0x0000ff;
const BOUGHT: u32 = 0;

/// The caption bar takes the title with the manager's names; the big
/// chart's and the winners' keep their own.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    if let Some(dialog) = dialogs.get_mut(&MARKET_DIALOG) {
        dialog.controls.insert(0, caption(dialog.frame, CAPTION_ID, String::new()));
    }
    for id in [CHART_DIALOG, WINNERS_DIALOG] {
        if let Some(dialog) = dialogs.get_mut(&id) {
            let title = dialog.title.clone();
            dialog.controls.insert(0, caption(dialog.frame, CAPTION_ID, title));
        }
    }
}

/// A price, its cents as `ftol(p × 100)` cuts them, worked out wider than
/// the price is kept — 168.8199… is "168,81": "151,31", "12,--".
pub(crate) fn price(p: f32) -> String {
    cents((f64::from(p) * 100.0) as i32)
}

/// An amount from its cents, the euros with points between the thousands
/// (`FUN_004dbb95`).
pub(crate) fn cents(c: i32) -> String {
    match c % 100 {
        0 => format!("{},--", count(c / 100)),
        part => format!("{},{:02}", count(c / 100), part.abs()),
    }
}

/// Money without its currency: "21.115", "1.36 Mio" (`FUN_004db87c`).
pub(crate) fn value(game: &Game, v: i64) -> String {
    let sign = if v < 0 { '-' } else { ' ' };
    let a = v.unsigned_abs();
    let text = |i: usize| game.text(VALUES[i]);
    let written = match a {
        0..1_000 => printf(text(0), &[&sign, &a]),
        1_000..1_000_000 => printf(text(1), &[&sign, &(a / 1000), &(a % 1000)]),
        _ => printf(text(2), &[&sign, &(a / 1_000_000), &(a % 1_000_000 / 10_000)]),
    };
    if sign == ' ' { written.chars().skip(2).collect() } else { written }
}

fn rgb(colorref: u32) -> [u8; 3] {
    let [r, g, b, _] = colorref.to_le_bytes();
    [r, g, b]
}

/// Rows in the order the game's sorted list box puts them: each before
/// the first that compares greater, without regard to case (`_stricoll`).
fn sorted<T>(mut rows: Vec<(String, T)>) -> Vec<(String, T)> {
    rows.sort_by(|a, b| {
        let key = |s: &str| s.chars().map(|c| c.to_ascii_lowercase()).collect::<String>();
        key(&a.0).cmp(&key(&b.0))
    });
    rows
}

/// What a share's chart shows: its bars and lines, and the top and bottom
/// of its scale (`FUN_0043aeaa`).
fn chart(share: &Share, n: usize, width: i32, height: i32, bought: f32) -> (PaintState, f32, f32) {
    let p: Vec<f32> = (0..n).map(|k| share.price(n - 1 - k)).collect();
    let mut low = p.iter().copied().fold(50_000.0f32, f32::min);
    let mut high = p.iter().copied().fold(-10_000.0f32, f32::max);
    high += 1.0;
    low -= 1.0;
    if low < 0.0 {
        low = 0.0;
    }
    let range = high - low;
    let mut paint = PaintState::default();
    // A step of the width less the gaps over the weeks, as a fraction:
    // the bars start where its sum truncates, the lines take it whole.
    let step = (width - n as i32 + 1) as f32 / n as f32;
    let (mut xs, mut hs) = (vec![0.0f32; n], vec![0i32; n]);
    if range != 0.0 {
        let mut x = 0.0f32;
        for k in 0..n {
            let h = (f64::from(p[k] - low) / f64::from(range) * f64::from(height)) as i32;
            let next = (f64::from(x) + f64::from(step) + 1.0) as f32;
            let w = next as i32 - x as i32 - 1;
            paint.bar(x as i32, height - h, w, h, WHITE);
            (xs[k], hs[k]) = (x, h);
            x = next;
        }
    }
    // Through the last two peaks and the last two troughs.
    let through = |paint: &mut PaintState, picks: &[usize], ink: u32| {
        let [.., a, b] = picks else { return };
        let slope = f64::from(hs[*b] - hs[*a]) / f64::from(xs[*b] - xs[*a]);
        let y0 = (f64::from(hs[*a]) - slope * f64::from(xs[*a])) as i32;
        let y1 = ((f64::from(width) - f64::from(xs[*b])) * slope + f64::from(hs[*b])) as i32;
        paint.line((0, height - y0), (width, height - y1), ink);
    };
    let peaks: Vec<usize> = (1..n - 1).filter(|&k| p[k - 1] < p[k] && p[k] > p[k + 1]).collect();
    let troughs: Vec<usize> = (1..n - 1).filter(|&k| p[k - 1] > p[k] && p[k] < p[k + 1]).collect();
    through(&mut paint, &peaks, PEAKS);
    through(&mut paint, &troughs, TROUGHS);
    if bought > 0.0 {
        let y = (f64::from(bought - low) / f64::from(range) * f64::from(height)) as i32;
        paint.line((0, height - y), (width, height - y), BOUGHT);
    }
    (paint, low, high)
}

/// The screen: the filter, the share chosen and the holding chosen.
#[derive(Default)]
pub struct ShareMarket {
    companies: bool,
    share: Option<usize>,
    place: Option<usize>,
}

impl ShareMarket {
    fn human(cx: &Cx) -> Option<usize> {
        cx.state.records.as_ref()?.human_slot(cx.state.season.as_ref()?.current)
    }

    fn market(cx: &Cx) -> Option<Market> {
        market::market(cx.state.records.as_ref()?)
    }

    fn holdings(cx: &Cx) -> [Option<Holding>; market::PLACES] {
        match (cx.state.records.as_ref(), Self::human(cx)) {
            (Some(records), Some(human)) => market::holdings(records, human),
            _ => [None; market::PLACES],
        }
    }

    fn cash(cx: &Cx) -> i32 {
        let records = cx.state.records.as_ref();
        Self::human(cx).and_then(|h| records?.managers.get(h)).map_or(0, |m| m.cash())
    }

    /// A share's row in the market list (`0x43c997`).
    fn share_row(share: &Share) -> String {
        format!(
            "{}\t{}\t{}\t{:.2}%\t{}\t{}\t{}",
            share.name,
            price(share.price(0)),
            price(share.price(1)),
            share.change,
            count(share.offer),
            price(share.high),
            price(share.low)
        )
    }

    /// The market list for the filter, sorted, the chosen share chosen
    /// again; without one buying is off.
    fn fill_shares(&mut self, cx: &Cx, view: &mut View) {
        let market = Self::market(cx);
        let tables = &cx.game.rules.market;
        let rows: Vec<(String, (usize, u32))> = market
            .iter()
            .flat_map(|m| m.shares.iter().enumerate())
            .filter(|(_, s)| s.company == self.companies)
            .map(|(i, s)| (Self::share_row(s), (i, tables.ink(s.change))))
            .collect();
        let rows = sorted(rows);
        let Some(list) = control(cx.dialog, SHARES).and_then(|c| view.list_mut(c)) else { return };
        list.keys = rows.iter().map(|r| r.1.0).collect();
        list.inks = rows.iter().map(|r| Some(rgb(r.1.1))).collect();
        list.rows = rows.into_iter().map(|r| r.0).collect();
        list.selected = self.share.and_then(|s| list.keys.iter().position(|&k| k == s));
        list.scroll = 0;
        if list.selected.is_none() {
            self.share = None;
            off(cx.dialog, view, &BUY);
        }
    }

    /// The holdings that pass the filter, sorted (`0x43cc36`); the chosen
    /// one chosen again while it is held, else selling is off; then the
    /// totals over all of them.
    fn fill_holdings(&mut self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(market) = Self::market(cx) else { return };
        let tables = &game.rules.market;
        let held = Self::holdings(cx);
        let mut rows = Vec::new();
        for (j, h) in held.iter().enumerate() {
            let Some(h) = h else { continue };
            let Some(share) = market.shares.get(h.share) else { continue };
            if share.company != self.companies {
                continue;
            }
            let p0 = share.price(0);
            let gain = (p0 - h.bought_at) * 100.0 / h.bought_at;
            let v = market::value(p0, h.count);
            let worth = if v > AS_MONEY {
                money(game, v.into())
            } else if f64::from(h.count) * f64::from(p0) > AS_VALUE {
                value(game, v.into())
            } else {
                cents((100.0 * f64::from(p0) * f64::from(h.count)) as i32)
            };
            let row = format!(
                "{}\t{}\t{}\t{gain:.2}%\t{}\t{worth}",
                share.name,
                price(p0),
                price(h.bought_at),
                count(h.count)
            );
            let ink = if p0 <= 0.0 { tables.inks[2] } else { tables.ink(gain) };
            rows.push((row, (j, ink)));
        }
        let rows = sorted(rows);
        if let Some(list) = control(dialog, HOLDINGS).and_then(|c| view.list_mut(c)) {
            list.keys = rows.iter().map(|r| r.1.0).collect();
            list.inks = rows.iter().map(|r| Some(rgb(r.1.1))).collect();
            list.rows = rows.into_iter().map(|r| r.0).collect();
            list.selected = self.place.and_then(|p| list.keys.iter().position(|&k| k == p));
            if list.selected.is_none() {
                self.place = None;
            }
        }
        if self.place.is_none() {
            off(dialog, view, &SELL);
        }
        // The totals, of all holdings (`0x43d48f`).
        let (mut n, mut a, mut worth, mut gain) = (0f64, 0f64, 0f64, 0f64);
        for h in held.iter().flatten() {
            let Some(share) = market.shares.get(h.share) else { continue };
            let (p0, c) = (f64::from(share.price(0)), f64::from(h.count));
            let bought = f64::from(h.bought_at);
            n += c;
            a += (p0 - bought) * 100.0 / bought * c;
            worth += p0 * c;
            gain += (p0 - bought) * c;
        }
        if n > 0.0 {
            set(view, dialog, TOTAL_TENDENCY, format!("{:.2}", a / n));
            ink(dialog, view, TOTAL_TENDENCY, if a < 0.0 { RED } else { BLUE });
        } else {
            set(view, dialog, TOTAL_TENDENCY, NO_TENDENCY.to_owned());
        }
        set(view, dialog, VOLUME, money(game, worth as i64));
        set(view, dialog, PROFIT, money(game, gain as i64));
        ink(dialog, view, PROFIT, if gain < 0.0 { RED } else { BLUE });
    }

    /// The chosen share: its chart, then the slider up to what the
    /// fortune and the offer allow (`0x43bd02`, `0x43c57f`).
    fn choose_share(&mut self, cx: &mut Cx, view: &mut View) -> Outcome {
        let dialog = cx.dialog;
        let Some(share) = self.share.and_then(|s| Self::market(cx)?.shares.get(s).cloned()) else {
            off(dialog, view, &BUY);
            clear_chart(dialog, view, &BUY);
            return Outcome::Stay;
        };
        show_chart(dialog, view, &BUY, &share, -1.0);
        self.buying(cx, view)
    }

    /// The buying slider for the chosen share, at one.
    fn buying(&mut self, cx: &mut Cx, view: &mut View) -> Outcome {
        let dialog = cx.dialog;
        let (Some(i), Some(human)) = (self.share, Self::human(cx)) else { return Outcome::Stay };
        let Some(share) = Self::market(cx).and_then(|m| m.shares.get(i).cloned()) else {
            return Outcome::Stay;
        };
        let records = cx.state.records.as_ref();
        if !records.is_some_and(|r| market::can_buy(r, human, i)) {
            off(dialog, view, &BUY);
            return Outcome::Popup(Request::with(NOTICE, Args::Text(NO_MORE)));
        }
        let p0 = share.price(0);
        let most = if p0 > 0.0 { ((f64::from(Self::cash(cx)) / f64::from(p0)) as i32).min(share.offer) } else { 0 };
        if most <= 0 {
            off(dialog, view, &BUY);
            return Outcome::Stay;
        }
        on(cx, view, &BUY, most.min(market::MOST), p0);
        Outcome::Stay
    }

    /// The chosen holding: its chart with the price paid, the selling
    /// slider (`0x43be89`, `0x43c806`).
    fn choose_place(&mut self, cx: &mut Cx, view: &mut View) {
        let dialog = cx.dialog;
        let held = self.place.and_then(|p| Self::holdings(cx)[p]);
        let share = held.and_then(|h| Self::market(cx)?.shares.get(h.share).cloned());
        let (Some(h), Some(share)) = (held, share) else {
            off(dialog, view, &SELL);
            clear_chart(dialog, view, &SELL);
            return;
        };
        show_chart(dialog, view, &SELL, &share, h.bought_at);
        self.selling(cx, view);
    }

    fn selling(&mut self, cx: &mut Cx, view: &mut View) {
        let held = self.place.and_then(|p| Self::holdings(cx)[p]);
        let share = held.and_then(|h| Self::market(cx)?.shares.get(h.share).cloned());
        match (held, share) {
            (Some(h), Some(share)) if h.count > 0 => {
                on(cx, view, &SELL, h.count.min(market::MOST), share.price(0));
            }
            _ => off(cx.dialog, view, &SELL),
        }
    }

    /// The fortune, and the share's row rewritten where it stands.
    fn after_trade(&mut self, cx: &Cx, view: &mut View, share: usize) {
        let dialog = cx.dialog;
        set(view, dialog, WEALTH, money(cx.game, Self::cash(cx).into()));
        if let Some(s) = Self::market(cx).and_then(|m| m.shares.get(share).cloned())
            && let Some(list) = control(dialog, SHARES).and_then(|c| view.list_mut(c))
            && let Some(row) = list.keys.iter().position(|&k| k == share)
        {
            list.rows[row] = Self::share_row(&s);
        }
        self.fill_holdings(cx, view);
    }
}

/// A static's colour.
fn ink(dialog: &Dialog, view: &mut View, id: u16, ink: [u8; 3]) {
    if let Some(ControlState::Static(s)) = control(dialog, id).map(|c| &mut view.controls[c].state) {
        s.ink = Some(ink);
    }
}

/// A side's slider hidden, its trade off, its count and amount blank
/// (`0x43d6d9`, `0x43d7a2`).
fn off(dialog: &Dialog, view: &mut View, side: &Side) {
    if let Some(c) = control(dialog, side.slider) {
        view.controls[c].hidden = true;
    }
    if let Some(options) = control(dialog, side.trade).and_then(|c| view.options_mut(c)) {
        options.dimmed = vec![false, true];
        options.chosen = NONE;
    }
    set(view, dialog, side.count, NOTHING.to_owned());
    set(view, dialog, side.amount, NOTHING.to_owned());
}

/// A side's slider from one to the most there is, at one; its trade on.
fn on(cx: &Cx, view: &mut View, side: &Side, most: i32, p0: f32) {
    let dialog = cx.dialog;
    let tables = &cx.game.rules.market;
    if let Some(c) = control(dialog, side.slider) {
        view.controls[c].hidden = false;
        if let Some(v) = view.value_mut(c) {
            v.range = (1, tables.position_of(most));
            v.set(1);
        }
    }
    if let Some(options) = control(dialog, side.trade).and_then(|c| view.options_mut(c)) {
        options.dimmed = vec![false, false];
    }
    set(view, dialog, side.count, "1".to_owned());
    set(view, dialog, side.amount, money(cx.game, market::value(p0, 1).into()));
}

fn clear_chart(dialog: &Dialog, view: &mut View, side: &Side) {
    if let Some(paint) = control(dialog, side.chart).and_then(|c| view.paint_mut(c)) {
        *paint = PaintState::default();
    }
    set(view, dialog, side.top, NOTHING.to_owned());
    set(view, dialog, side.bottom, NOTHING.to_owned());
}

fn show_chart(dialog: &Dialog, view: &mut View, side: &Side, share: &Share, bought: f32) {
    let Some(c) = control(dialog, side.chart) else { return };
    let r = dialog.controls[c].rect;
    let (shown, low, high) = chart(share, CHART_WEEKS, r.width() as i32, r.height() as i32, bought);
    if let Some(paint) = view.paint_mut(c) {
        *paint = shown;
    }
    set(view, dialog, side.top, price(high));
    set(view, dialog, side.bottom, price(low));
}

impl Screen for ShareMarket {
    fn ids(&self) -> &[u16] {
        &[MARKET_DIALOG]
    }

    /// `0x43b9eb`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        *self = Self::default();
        if let Some(manager) = cx.state.season.as_ref().and_then(|s| s.manager()) {
            let title = CAPTION
                .replacen("%s", &dialog.title, 1)
                .replacen("%s", &manager.first_name, 1)
                .replacen("%s", &manager.last_name, 1);
            set(view, dialog, CAPTION_ID, title);
        }
        if let Some(c) = control(dialog, FILTER) {
            view.set_chosen(c, 0);
        }
        for side in [&BUY, &SELL] {
            if let Some(c) = control(dialog, side.trade) {
                view.set_chosen(c, NONE);
            }
        }
        self.fill_shares(cx, view);
        self.fill_holdings(cx, view);
        set(view, dialog, WEALTH, money(game, Self::cash(cx).into()));
        if let Some(market) = Self::market(cx) {
            let index =
                if market.index <= 0.0 { game.text(SUSPENDED).to_owned() } else { format!("{:.2}", market.index) };
            set(view, dialog, INDEX, index);
            set(view, dialog, INDEX_TENDENCY, format!("{:.2}%", market.tendency));
        }
        for side in [&BUY, &SELL] {
            clear_chart(dialog, view, side);
            off(dialog, view, side);
        }
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        match target {
            Target::Option { control: c, item } if id(c) == FILTER => {
                self.companies = item == 1;
                self.share = None;
                self.fill_shares(cx, view);
                clear_chart(dialog, view, &BUY);
                self.fill_holdings(cx, view);
                if self.place.is_none() {
                    clear_chart(dialog, view, &SELL);
                }
            }
            Target::Row { control: c, .. } if id(c) == SHARES => {
                let list = view.list(c);
                self.share = list.and_then(|l| l.keys.get(l.selected?).copied());
                return self.choose_share(cx, view);
            }
            Target::Row { control: c, .. } if id(c) == HOLDINGS => {
                let list = view.list(c);
                self.place = list.and_then(|l| l.keys.get(l.selected?).copied());
                self.choose_place(cx, view);
            }
            Target::SliderStep { control: c, .. }
            | Target::SliderAt { control: c, .. }
            | Target::SliderTrack(c)
                if id(c) == BUY.slider || id(c) == SELL.slider =>
            {
                let side = if id(c) == BUY.slider { &BUY } else { &SELL };
                let share = if id(c) == BUY.slider {
                    self.share
                } else {
                    self.place.and_then(|p| Self::holdings(cx)[p]).map(|h| h.share)
                };
                let Some(share) = share.and_then(|s| Self::market(cx)?.shares.get(s).cloned()) else {
                    return Outcome::Stay;
                };
                let n = cx.game.rules.market.count_at(view.value(c).map_or(0, |v| v.value));
                set(view, dialog, side.count, count(n));
                set(view, dialog, side.amount, money(cx.game, market::value(share.price(0), n).into()));
            }
            // Kaufen (`0x43c2da`).
            Target::Option { control: c, item: TRADE } if id(c) == BUY.trade => {
                if let Some(o) = view.options_mut(c) {
                    o.chosen = NONE;
                }
                let (Some(share), Some(human)) = (self.share, Self::human(cx)) else {
                    return Outcome::Stay;
                };
                let slider = control(dialog, BUY.slider).and_then(|s| view.value(s)).map_or(0, |v| v.value);
                let n = cx.game.rules.market.count_at(slider);
                if n <= 0 {
                    return Outcome::Stay;
                }
                let Some(records) = cx.state.records.as_mut() else { return Outcome::Stay };
                let got = market::buy(records, human, share, n);
                self.after_trade(cx, view, share);
                let outcome = self.buying(cx, view);
                if got < n {
                    return Outcome::Popup(Request::with(NOTICE, Args::Text(NOT_ALL)));
                }
                return outcome;
            }
            // Verkaufen (`0x43c3e7`).
            Target::Option { control: c, item: TRADE } if id(c) == SELL.trade => {
                if let Some(o) = view.options_mut(c) {
                    o.chosen = NONE;
                }
                let (Some(place), Some(human)) = (self.place, Self::human(cx)) else {
                    return Outcome::Stay;
                };
                let Some(h) = Self::holdings(cx)[place] else { return Outcome::Stay };
                let slider = control(dialog, SELL.slider).and_then(|s| view.value(s)).map_or(0, |v| v.value);
                let n = cx.game.rules.market.count_at(slider);
                if n <= 0 {
                    return Outcome::Stay;
                }
                if let Some(records) = cx.state.records.as_mut() {
                    market::sell(records, human, place, n);
                }
                self.after_trade(cx, view, h.share);
                self.selling(cx, view);
            }
            // The big chart of the share or the holding chosen
            // (`0x43c169`, `0x43c1f9`).
            Target::Option { control: c, .. } if id(c) == BUY.trade || id(c) == SELL.trade => {
                if let Some(o) = view.options_mut(c) {
                    o.chosen = NONE;
                }
                let args = if id(c) == BUY.trade {
                    self.share.map(|s| Args::Share(s, None))
                } else {
                    self.place.and_then(|p| Some(Args::Share(Self::holdings(cx)[p]?.share, Some(p))))
                };
                if let Some(args) = args {
                    return Outcome::Popup(Request::with(CHART_DIALOG, args));
                }
            }
            Target::Button(c) if id(c) == WINNERS => {
                return Outcome::Popup(Request::new(WINNERS_DIALOG));
            }
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                return Outcome::Close(Reply::Ok);
            }
            _ => {}
        }
        Outcome::Stay
    }
}

pub(crate) const CHART_DIALOG: u16 = 432;
/// The share's name, its price and last week's, the offer, the year's
/// highest and lowest, the chart and its scale's ends.
const CHART_NAME: u16 = 1866;
const CHART_PRICE: u16 = 5301;
const CHART_LAST: u16 = 5300;
const CHART_OFFER: u16 = 5304;
const CHART_HIGH: u16 = 5303;
const CHART_LOW: u16 = 5302;
const BIG_CHART: u16 = 5299;
const CHART_TOP: u16 = 5305;
const CHART_BOTTOM: u16 = 5328;

/// The big chart (dialog 432, `CDiChart`): a share's year, its 52 weeks
/// as the market's small charts draw ten, with the price a holding was
/// bought at. Its tendency's picture is never set.
pub struct ShareChart;

impl Screen for ShareChart {
    fn ids(&self) -> &[u16] {
        &[CHART_DIALOG]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let dialog = cx.dialog;
        mark_inert(view, dialog, |_| true);
        let Args::Share(i, place) = args else { return };
        let Some(share) = ShareMarket::market(cx).and_then(|m| m.shares.get(i).cloned()) else { return };
        let bought = place.and_then(|p| ShareMarket::holdings(cx)[p]).map_or(-1.0, |h| h.bought_at);
        set(view, dialog, CHART_NAME, share.name.clone());
        set(view, dialog, CHART_PRICE, price(share.price(0)));
        set(view, dialog, CHART_LAST, price(share.price(1)));
        set(view, dialog, CHART_OFFER, share.offer.to_string());
        set(view, dialog, CHART_HIGH, price(share.high));
        set(view, dialog, CHART_LOW, price(share.low));
        let Some(c) = control(dialog, BIG_CHART) else { return };
        let r = dialog.controls[c].rect;
        let (shown, low, high) = chart(&share, market::WEEKS, r.width() as i32, r.height() as i32, bought);
        if let Some(paint) = view.paint_mut(c) {
            *paint = shown;
        }
        set(view, dialog, CHART_TOP, price(high));
        set(view, dialog, CHART_BOTTOM, price(low));
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&cx.dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            _ => Outcome::Stay,
        }
    }
}

pub(crate) const WINNERS_DIALOG: u16 = 433;
const GAINERS: u16 = 5308;
const LOSERS: u16 = 5307;
/// How many of each.
const WINNERS_SHOWN: usize = 8;

/// The week's winners and losers (dialog 433, `CDiGewinnerUndVerlierer`):
/// of all shares, the eight that rose most, the most first, and the eight
/// that fell most, the most first (`0x4b67b5`) — the prices rounded here,
/// with a dot.
pub struct Winners;

impl Screen for Winners {
    fn ids(&self) -> &[u16] {
        &[WINNERS_DIALOG]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        mark_inert(view, dialog, |_| true);
        let Some(market) = ShareMarket::market(cx) else { return };
        let row = |s: &Share| {
            format!(
                "{}\t{:.2}\t{:.2}\t{:.2}%\t{}\t{:.2}",
                s.name,
                s.price(0),
                s.price(1),
                s.change,
                count(s.offer),
                s.price(0) - s.price(1)
            )
        };
        // Each put after those before it with a change no smaller (no
        // greater), as the game inserts them.
        let mut up: Vec<&Share> = market.shares.iter().filter(|s| s.change >= 0.0).collect();
        up.sort_by(|a, b| b.change.total_cmp(&a.change));
        let mut down: Vec<&Share> = market.shares.iter().filter(|s| s.change < 0.0).collect();
        down.sort_by(|a, b| a.change.total_cmp(&b.change));
        for (id, shares) in [(GAINERS, up), (LOSERS, down)] {
            if let Some(list) = control(dialog, id).and_then(|c| view.list_mut(c)) {
                list.rows = shares.iter().take(WINNERS_SHOWN).map(|s| row(s)).collect();
                list.selected = None;
            }
        }
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&cx.dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            _ => Outcome::Stay,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_price_keeps_its_cents_cut_off() {
        assert_eq!(price(151.319_99), "151,31");
        assert_eq!(price(168.819_99), "168,81");
        assert_eq!(price(12.0), "12,--");
        assert_eq!(price(1234.5), "1.234,50");
    }
}
