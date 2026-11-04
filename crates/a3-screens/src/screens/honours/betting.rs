//! The betting office (ERFOLGE.md §D), behind the honours page's "W": in
//! the matchday's weeks its bets on the matchday (dialog 411,
//! `CWettbueroSpieltag`), a page for each country played under its flag —
//! the full tip of the eleven matches, played up to ten times; a stake on
//! each highest win ticked; the first scorer; the matchday's goals; the
//! trainer to be sacked within a fortnight. Every change stands at once;
//! the stakes may not come to more than the private fortune.

use std::collections::HashMap;

use a3_core::betting::{self, MatchdayBet, MatchdayOffer};
use a3_files::rch::Dialog;

use crate::controls::ControlState;
use crate::pointer::Target;
use crate::screens::playerinfo::money;
use crate::screens::statistics::spelled;
use crate::screens::{Args, Cx, Outcome, Reply, Screen, caption, control, mark_inert, set};
use crate::view::View;

pub(crate) const MATCHDAY_DIALOG: u16 = 411;
const CAPTION_ID: u16 = 9947;
/// The countries' flags, the fortune.
const FLAGS: [u16; betting::PAGES] = [5151, 5152, 5153, 5154, 5155];
const FORTUNE: u16 = 5282;
/// The full tip: how often, its spinner, its stake.
const FULL_COUNT: u16 = 5233;
const FULL_SPIN: u16 = 5170;
const FULL_STAKE: u16 = 5168;
/// The stake on each highest win, its spinner, what the ticks come to.
const HIGH: u16 = 5235;
const HIGH_SPIN: u16 = 5171;
const HIGH_STAKE: u16 = 5169;
/// The matches, their quotas, and the ticks of home win, draw, away win
/// and the highest win (`0x88d268`, `0x88d294`, `0x88d2c0`, `0x88d2ec`).
const MATCHES: [u16; betting::MATCHES] = [5172, 5173, 5174, 5175, 5176, 5177, 5178, 5179, 5180, 5181, 5182];
const QUOTAS: [u16; betting::MATCHES] = [5249, 5250, 5251, 5252, 5253, 5254, 5255, 5256, 5257, 5258, 5259];
const TICKS: [([u16; betting::MATCHES], u8); 4] = [
    ([5189, 5190, 5232, 5191, 5192, 5193, 5194, 5195, 5196, 5197, 5198], betting::HOME),
    ([5199, 5200, 5201, 5202, 5203, 5204, 5205, 5206, 5207, 5208, 5209], betting::DRAW),
    ([5210, 5211, 5212, 5213, 5214, 5215, 5216, 5217, 5218, 5219, 5220], betting::AWAY),
    ([5221, 5222, 5223, 5224, 5225, 5226, 5227, 5228, 5229, 5230, 5231], betting::HIGHEST),
];
/// The first scorer, the goals and the trainer: each a stake, its spinner
/// and a list.
const SCORER: (u16, u16, u16) = (5183, 5184, 5187);
const GOALS: (u16, u16, u16) = (5185, 5186, 5188);
const TRAINER: (u16, u16, u16) = (5163, 5162, 5164);
/// "<Läuft noch>", while the trainer bet runs.
const RUNNING: u16 = 5335;
/// The goals one can bet on, 0 to 50.
const MOST_GOALS: i16 = 50;

/// Texts of the game.
const NOT_CHOSEN: u16 = 4488;
const NOTHING: &str = " ";
const CAPTION: &str = "%s %s %s";
/// The flags' first frames by a country's kind (`0x89fce0`): Germany,
/// England, France, Italy, Spain; each ten frames.
const FLAG_FRAMES: [u16; 5] = [1526, 1546, 1536, 1556, 1566];
const FLAG_COUNT: u16 = 10;

/// The caption bars take the titles with the manager's names.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    for id in [MATCHDAY_DIALOG, SEASON_DIALOG] {
        if let Some(dialog) = dialogs.get_mut(&id) {
            dialog.controls.insert(0, caption(dialog.frame, CAPTION_ID, String::new()));
        }
    }
}

/// The screen and the country it shows.
#[derive(Default)]
pub struct MatchdayBets {
    page: usize,
}

fn human(cx: &Cx) -> Option<usize> {
    cx.state.records.as_ref()?.human_slot(cx.state.season.as_ref()?.current)
}

fn fortune(cx: &Cx) -> i32 {
    let records = cx.state.records.as_ref();
    human(cx).and_then(|h| records?.managers.get(h)).map_or(0, |m| m.cash())
}

/// A spinner's range and position.
fn spin(dialog: &Dialog, view: &mut View, id: u16, most: i32, position: i32) {
    if let Some(v) = control(dialog, id).and_then(|c| view.value_mut(c)) {
        v.range = (0, most);
        v.set(position);
    }
}

fn shown(dialog: &Dialog, view: &mut View, id: u16, on: bool) {
    if let Some(c) = control(dialog, id) {
        view.controls[c].hidden = !on;
    }
}

impl MatchdayBets {
    fn bet(&self, cx: &Cx) -> MatchdayBet {
        match (cx.state.records.as_ref(), human(cx)) {
            (Some(records), Some(h)) => betting::matchday_bet(records, h, self.page),
            _ => MatchdayBet::default(),
        }
    }

    fn keep(&self, cx: &mut Cx, bet: &MatchdayBet) {
        let h = human(cx);
        if let (Some(records), Some(h)) = (cx.state.records.as_mut(), h) {
            betting::set_matchday_bet(records, h, self.page, bet);
        }
    }

    /// The matchday's stakes against the fortune (`FUN_0077fb60`).
    fn over(cx: &Cx) -> bool {
        let total = match (cx.state.records.as_ref(), human(cx)) {
            (Some(records), Some(h)) => betting::matchday_total(records, h),
            _ => 0,
        };
        total > fortune(cx)
    }

    /// The page of a country (`FUN_005d53e0`).
    fn page(&mut self, cx: &Cx, view: &mut View, page: usize) {
        let (game, dialog) = (cx.game, cx.dialog);
        self.page = page;
        wave(dialog, view, page);
        let bet = self.bet(cx);
        let tables = &game.rules.betting;
        let Some(records) = cx.state.records.as_ref() else { return };
        let offer = betting::matchday_offer(records, page)
            .unwrap_or(MatchdayOffer { matches: Vec::new(), scorers: Vec::new(), trainers: Vec::new() });
        spin(dialog, view, FULL_SPIN, i32::from(betting::MOST_FULL), i32::from(bet.full));
        let most = tables.position_of(betting::MOST_HIGH_STAKE);
        spin(dialog, view, HIGH_SPIN, most, tables.position_of(bet.high_stake));
        let most = tables.position_of(betting::MOST_STAKE);
        for ((_, s, _), stake) in [(SCORER, bet.scorer_stake), (GOALS, bet.goals_stake), (TRAINER, bet.trainer_stake)] {
            spin(dialog, view, s, most, tables.position_of(stake));
        }
        self.show_stakes(cx, view, &bet);
        let not_chosen = game.text(NOT_CHOSEN).to_owned();
        let scorers = offer.scorers.iter().map(|s| {
            format!("{}\t({})", spelled(records, s.names.0, s.names.1), betting::odds(s.odds))
        });
        let rows: Vec<String> = std::iter::once(not_chosen.clone()).chain(scorers).collect();
        combo(dialog, view, SCORER.2, rows, Some(usize::try_from(bet.scorer + 1).unwrap_or(0)), false);
        let goals = (0..=MOST_GOALS).map(|g| format!("{g:2}"));
        let rows: Vec<String> = std::iter::once(not_chosen.clone()).chain(goals).collect();
        combo(dialog, view, GOALS.2, rows, Some(usize::try_from(bet.goals + 1).unwrap_or(0)), false);
        let name = |id: i16| trainer_name(cx, id);
        if bet.running {
            // The running bet stands, its trainer alone in the list.
            combo(dialog, view, TRAINER.2, vec![name(bet.trainer)], Some(0), true);
        } else {
            let trainers = offer.trainers.iter().map(|t| format!("{} ({})", name(t.id), betting::odds(t.odds.into())));
            let rows: Vec<String> = std::iter::once(not_chosen).chain(trainers).collect();
            let chosen = offer.trainers.iter().position(|t| t.id == bet.trainer && bet.trainer != 0);
            combo(dialog, view, TRAINER.2, rows, Some(chosen.map_or(0, |i| i + 1)), false);
        }
        shown(dialog, view, RUNNING, bet.running);
        shown(dialog, view, TRAINER.1, !bet.running);
        // The matches with their quotas (the table places the game puts
        // after a club's name stay out: none was ever seen).
        let club = |(country, number): (u8, u16)| {
            records.team(u16::from(country), number).map(|t| t.name()).unwrap_or_default()
        };
        for (k, m) in offer.matches.iter().enumerate().take(betting::MATCHES) {
            set(view, dialog, MATCHES[k], format!("{} - {}", club(m.home), club(m.away)));
            set(view, dialog, QUOTAS[k], (m.quota / 10).to_string());
        }
        self.show_ticks(dialog, view, &bet);
    }

    fn show_ticks(&self, dialog: &Dialog, view: &mut View, bet: &MatchdayBet) {
        for (ids, bit) in TICKS {
            for (k, id) in ids.into_iter().enumerate() {
                if let Some(ControlState::Check(c)) = control(dialog, id).map(|c| &mut view.controls[c].state) {
                    c.checked = bet.ticks[k] & bit != 0;
                }
            }
        }
    }

    fn show_stakes(&self, cx: &Cx, view: &mut View, bet: &MatchdayBet) {
        let (game, dialog) = (cx.game, cx.dialog);
        set(view, dialog, FULL_COUNT, format!("{} x", bet.full));
        set(view, dialog, FULL_STAKE, money(game, bet.full_stake().into()));
        set(view, dialog, HIGH, money(game, bet.high_stake.into()));
        set(view, dialog, HIGH_STAKE, money(game, (bet.high_stake * bet.highest()).into()));
        for ((s, _, _), stake) in [(SCORER, bet.scorer_stake), (GOALS, bet.goals_stake), (TRAINER, bet.trainer_stake)] {
            set(view, dialog, s, money(game, stake.into()));
        }
    }

    /// A tick of match k (the dialog's `OnCommand`): an outcome on, unless
    /// the stakes then come to more than the fortune; the highest win on
    /// one match only.
    fn tick(&mut self, cx: &mut Cx, view: &mut View, k: usize, bit: u8, on: bool) {
        let mut bet = self.bet(cx);
        if on {
            if bit == betting::HIGHEST {
                for t in &mut bet.ticks {
                    *t &= !betting::HIGHEST;
                }
            }
            bet.ticks[k] |= bit;
            self.keep(cx, &bet);
            if Self::over(cx) {
                bet.ticks[k] &= !bit;
                self.keep(cx, &bet);
            }
        } else {
            bet.ticks[k] &= !bit;
            self.keep(cx, &bet);
        }
        self.show_ticks(cx.dialog, view, &bet);
        self.show_stakes(cx, view, &bet);
    }

    /// A stake's spinner moved (`FUN_005d663b` for the highest win, the
    /// 410 rule for the three single bets).
    fn stake(&mut self, cx: &mut Cx, view: &mut View, id: u16, position: i32) {
        let tables = &cx.game.rules.betting;
        let mut bet = self.bet(cx);
        let mut value = tables.stake_at(position);
        if id == HIGH_SPIN {
            let old = bet.high_stake;
            bet.high_stake = value;
            self.keep(cx, &bet);
            if value > old && Self::over(cx) {
                bet.high_stake = old;
                self.keep(cx, &bet);
                spin(cx.dialog, view, id, tables.position_of(betting::MOST_HIGH_STAKE), tables.position_of(old));
                return;
            }
        } else {
            let current = match id {
                _ if id == SCORER.1 => bet.scorer_stake,
                _ if id == GOALS.1 => bet.goals_stake,
                _ => bet.trainer_stake,
            };
            let room = match (cx.state.records.as_ref(), human(cx)) {
                (Some(records), Some(h)) => betting::matchday_room(records, h, fortune(cx)),
                _ => 0,
            };
            let mut position = position;
            if value > room + current {
                position = (position - 1).max(0);
                value = tables.stake_at(position);
                spin(cx.dialog, view, id, tables.position_of(betting::MOST_STAKE), position);
            }
            match id {
                _ if id == SCORER.1 => bet.scorer_stake = value,
                _ if id == GOALS.1 => bet.goals_stake = value,
                _ => bet.trainer_stake = value,
            }
            self.keep(cx, &bet);
        }
        self.show_stakes(cx, view, &bet);
    }
}

/// A flag for each country played, the rest hidden (`0x89fce0`, by the
/// country's kind).
fn show_flags(cx: &Cx, view: &mut View) {
    let dialog = cx.dialog;
    let countries = cx.state.records.as_ref().map(|r| {
        let count = usize::try_from(r.globals.more_count() + 1).unwrap_or(1);
        (0..betting::PAGES)
            .map(|c| {
                (c < count).then(|| {
                    let key = usize::try_from(r.countries.ranking_key(c)).unwrap_or(0);
                    usize::from(r.countries.cup_system(key))
                })
            })
            .collect::<Vec<_>>()
    });
    for (i, id) in FLAGS.into_iter().enumerate() {
        let Some(c) = control(dialog, id) else { continue };
        let kind = countries.as_ref().and_then(|k| k[i]);
        view.controls[c].hidden = kind.is_none();
        if let (Some(kind), ControlState::Anim(a)) = (kind, &mut view.controls[c].state) {
            let first = FLAG_FRAMES.get(kind).copied().unwrap_or(FLAG_FRAMES[0]);
            a.frames = (first..first + FLAG_COUNT).collect();
            a.frame = 0;
            a.clickable = true;
        }
    }
}

/// The chosen country's flag waves, the others stand where they stopped.
fn wave(dialog: &Dialog, view: &mut View, page: usize) {
    for (i, id) in FLAGS.into_iter().enumerate() {
        if let Some(ControlState::Anim(a)) = control(dialog, id).map(|c| &mut view.controls[c].state) {
            a.playing = i == page;
        }
    }
}

/// The caption: the dialog's title with the manager's names.
fn title(cx: &Cx, view: &mut View) {
    let dialog = cx.dialog;
    if let Some(manager) = cx.state.season.as_ref().and_then(|s| s.manager()) {
        let title = CAPTION
            .replacen("%s", &dialog.title, 1)
            .replacen("%s", &manager.first_name, 1)
            .replacen("%s", &manager.last_name, 1);
        set(view, dialog, CAPTION_ID, title);
    }
}

/// A trainer's name: a person's, or a human manager's "%s %s".
fn trainer_name(cx: &Cx, id: i16) -> String {
    let Some(records) = cx.state.records.as_ref() else { return String::new() };
    if id > 0 {
        return records.person_name(id as u16);
    }
    let manager = usize::try_from(-i32::from(id) - 1).ok().and_then(|m| records.managers.get(m));
    manager.map_or_else(String::new, |m| format!("{} {}", m.first_name(), m.last_name()))
}

fn combo(dialog: &Dialog, view: &mut View, id: u16, items: Vec<String>, selected: Option<usize>, disabled: bool) {
    if let Some(c) = control(dialog, id).and_then(|c| view.combo_mut(c)) {
        c.items = items;
        c.selected = selected;
        c.scroll = 0;
        c.disabled = disabled;
    }
}

impl Screen for MatchdayBets {
    fn ids(&self) -> &[u16] {
        &[MATCHDAY_DIALOG]
    }

    /// `0x5d521e`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        title(cx, view);
        show_flags(cx, view);
        set(view, dialog, FORTUNE, money(game, fortune(cx).into()));
        self.page(cx, view, 0);
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        match target {
            Target::Control(c) if FLAGS.contains(&id(c)) => {
                let page = FLAGS.iter().position(|&f| f == id(c)).unwrap_or(0);
                self.page(cx, view, page);
            }
            // How often the full tip is played (`FUN_005d6511`).
            Target::Spin { control: c, .. } if id(c) == FULL_SPIN => {
                let n = view.value(c).map_or(0, |v| v.value) as i16;
                let mut bet = self.bet(cx);
                let old = bet.full;
                bet.full = n;
                self.keep(cx, &bet);
                if n > old && Self::over(cx) {
                    bet.full = old;
                    self.keep(cx, &bet);
                    spin(dialog, view, FULL_SPIN, i32::from(betting::MOST_FULL), i32::from(old));
                    return Outcome::Stay;
                }
                self.show_stakes(cx, view, &bet);
            }
            Target::Spin { control: c, .. } if [HIGH_SPIN, SCORER.1, GOALS.1, TRAINER.1].contains(&id(c)) => {
                let position = view.value(c).map_or(0, |v| v.value);
                self.stake(cx, view, id(c), position);
            }
            Target::Check(c) => {
                let on = matches!(&view.controls[c].state, ControlState::Check(b) if b.checked);
                for (ids, bit) in TICKS {
                    if let Some(k) = ids.iter().position(|&t| t == id(c)) {
                        self.tick(cx, view, k, bit, on);
                    }
                }
            }
            // The lists (`0x5d68bc`, `0x5d6c15`, `0x5d6a02`).
            Target::Row { control: c, .. } if [SCORER.2, GOALS.2, TRAINER.2].contains(&id(c)) => {
                let row = view.combo(c).and_then(|combo| combo.selected).unwrap_or(0) as i16;
                let mut bet = self.bet(cx);
                if id(c) == SCORER.2 {
                    bet.scorer = row - 1;
                } else if id(c) == GOALS.2 {
                    bet.goals = row - 1;
                } else if !bet.running {
                    let offer = cx.state.records.as_ref().and_then(|r| betting::matchday_offer(r, self.page));
                    match offer.and_then(|o| o.trainers.get(usize::try_from(row - 1).ok()?).copied()) {
                        Some(t) => {
                            bet.trainer = t.id;
                            bet.trainer_odds = t.odds;
                            bet.trainer_club = t.club;
                        }
                        None => bet.trainer = 0,
                    }
                }
                self.keep(cx, &bet);
            }
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                return Outcome::Close(Reply::Ok);
            }
            _ => {}
        }
        Outcome::Stay
    }
}

pub(crate) const SEASON_DIALOG: u16 = 410;
/// A bet of the season: its stake's spinner, the stake, its list.
struct Bet {
    spin: u16,
    stake: u16,
    list: u16,
}
/// A country's: the champion, the cup winner, the top scorer, the trainer
/// sacked first.
const CHAMPION: Bet = Bet { spin: 5148, stake: 5150, list: 5149 };
const CUP: Bet = Bet { spin: 5157, stake: 5156, list: 5158 };
const TOP_SCORER: Bet = Bet { spin: 5160, stake: 5159, list: 5161 };
const SACKED_FIRST: Bet = Bet { spin: 5162, stake: 5163, list: 5164 };
/// Over all: the own team's place, the Champions League's and the UEFA
/// cup's winners, the European and the world champion.
const PLACE: Bet = Bet { spin: 5165, stake: 5166, list: 5167 };
const CUPS: [Bet; 4] = [
    Bet { spin: 5237, stake: 5234, list: 5236 },
    Bet { spin: 5239, stake: 5238, list: 5240 },
    Bet { spin: 5242, stake: 5241, list: 5243 },
    Bet { spin: 5245, stake: 5244, list: 5246 },
];
/// The European cups' names, as the new game set them.
const CUP_NAMES: [u16; 2] = [6284, 6285];
/// "Bitte auswählen".
const CHOOSE: u16 = 4945;
/// The tournaments come by the year: the European championship's when
/// the start year leaves 3 over four, the world's when it leaves 1.
const EUROPEAN_YEAR: u16 = 3;
const WORLD_YEAR: u16 = 1;

/// The season's bets (dialog 410, `CWettbueroSaison`): a page for each
/// country played under its flag — the champion, the cup winner, the top
/// scorer, the trainer sacked first —, and over all the own team's place,
/// the European cups' winners and the year's tournament's.
#[derive(Default)]
pub struct SeasonBets {
    page: usize,
}

/// A row of a list: a name and its odds.
fn with_odds(name: &str, tenths: i32) -> String {
    format!("{name}\t{}", betting::odds(tenths))
}

impl SeasonBets {
    fn bets(cx: &Cx) -> betting::SeasonBets {
        match (cx.state.records.as_ref(), human(cx)) {
            (Some(records), Some(h)) => betting::season_bets(records, h),
            _ => betting::SeasonBets::default(),
        }
    }

    fn keep(cx: &mut Cx, bets: &betting::SeasonBets) {
        let h = human(cx);
        if let (Some(records), Some(h)) = (cx.state.records.as_mut(), h) {
            betting::set_season_bets(records, h, bets);
        }
    }

    fn country(cx: &Cx, page: usize) -> u16 {
        cx.state.records.as_ref().map_or(0, |r| r.countries.ranking_key(page) as u16)
    }

    /// A stake's spinner and figure.
    fn stake(cx: &Cx, view: &mut View, bet: &Bet, stake: i32) {
        let tables = &cx.game.rules.betting;
        spin(cx.dialog, view, bet.spin, tables.position_of(betting::MOST_STAKE), tables.position_of(stake));
        set(view, cx.dialog, bet.stake, money(cx.game, stake.into()));
    }

    /// A country's page (`FUN_005d349a`).
    fn page(&mut self, cx: &Cx, view: &mut View, page: usize) {
        let (game, dialog) = (cx.game, cx.dialog);
        self.page = page;
        wave(dialog, view, page);
        let bets = Self::bets(cx);
        let p = bets.pages[page];
        let Some(records) = cx.state.records.as_ref() else { return };
        let offer = betting::season_offer(records, page).unwrap_or_default();
        let choose = game.text(CHOOSE).to_owned();
        let country = Self::country(cx, page);
        let club = |number: u16| records.team(country, number).map(|t| t.name()).unwrap_or_default();
        for (bet, stake) in [&CHAMPION, &CUP, &TOP_SCORER, &SACKED_FIRST].into_iter().zip(p.stakes) {
            Self::stake(cx, view, bet, stake);
        }
        // The champion from the first league's table, its odds by place.
        let table: Vec<i16> = a3_core::opponent::League::of(records, country, 0)
            .map(|l| (0..l.teams()).map(|i| l.team(i)).collect())
            .unwrap_or_default();
        let rows = table.iter().enumerate().map(|(i, &t)| {
            with_odds(&club(t as u16), offer.champion_odds.get(i).copied().unwrap_or(0).into())
        });
        let chosen = table.iter().position(|&t| t == p.champion).map_or(0, |i| i + 1);
        combo(dialog, view, CHAMPION.list, std::iter::once(choose.clone()).chain(rows).collect(), Some(chosen), false);
        let rows = offer.cup.iter().map(|&(t, o)| with_odds(&club(t as u16), o.into()));
        let rows: Vec<String> = std::iter::once(choose.clone()).chain(rows).collect();
        let chosen = usize::try_from(p.cup + 1).ok().filter(|&r| r < rows.len()).unwrap_or(0);
        combo(dialog, view, CUP.list, rows, Some(chosen), false);
        let rows = offer.scorers.iter().map(|s| with_odds(&spelled(records, s.names.0, s.names.1), s.odds));
        let rows: Vec<String> = std::iter::once(choose.clone()).chain(rows).collect();
        let chosen = usize::try_from(p.scorer + 1).ok().filter(|&r| r < rows.len()).unwrap_or(0);
        combo(dialog, view, TOP_SCORER.list, rows, Some(chosen), false);
        let rows = offer.trainers.iter().map(|t| with_odds(&trainer_name(cx, t.id), t.odds.into()));
        let chosen = offer.trainers.iter().position(|t| t.id == p.trainer).map_or(0, |i| i + 1);
        combo(dialog, view, SACKED_FIRST.list, std::iter::once(choose).chain(rows).collect(), Some(chosen), false);
    }

    /// The bets over all countries (`0x5d26c2`'s steps 4 to 9).
    fn overall(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let bets = Self::bets(cx);
        let Some(records) = cx.state.records.as_ref() else { return };
        let offer = betting::season_offer(records, 0).unwrap_or_default();
        let choose = game.text(CHOOSE).to_owned();
        Self::stake(cx, view, &PLACE, bets.stakes[0]);
        let odds = human(cx).map(|h| betting::place_odds(records, h)).unwrap_or_default();
        let rows = odds.iter().enumerate().map(|(i, &o)| format!("{:2}\t{}", i + 1, betting::odds(o.into())));
        let rows: Vec<String> = std::iter::once(choose.clone()).chain(rows).collect();
        let chosen = usize::try_from(bets.place).ok().filter(|&r| r < rows.len()).unwrap_or(0);
        combo(dialog, view, PLACE.list, rows, Some(chosen), false);
        let year = records.globals.start_year() % 4;
        for (k, bet) in CUPS.iter().enumerate() {
            let shown = match k {
                2 => year == EUROPEAN_YEAR,
                3 => year == WORLD_YEAR,
                _ => true,
            };
            shown_bet(dialog, view, bet, shown);
            if !shown {
                set(view, dialog, bet.stake, NOTHING.to_owned());
                continue;
            }
            Self::stake(cx, view, bet, bets.stakes[k + 1]);
            let rows: Vec<String> = match k {
                0 | 1 => offer.european_cups[k]
                    .iter()
                    .map(|&((country, number), o)| {
                        let name = records.team(u16::from(country), number).map(|t| t.name()).unwrap_or_default();
                        with_odds(&name, o.into())
                    })
                    .collect(),
                _ => offer.tournaments[k - 2]
                    .iter()
                    .map(|&(nation, o)| with_odds(game.world.nation(nation as u16), o.into()))
                    .collect(),
            };
            let rows: Vec<String> = std::iter::once(choose.clone()).chain(rows).collect();
            let chosen = usize::try_from(bets.cups[k] + 1).ok().filter(|&r| r < rows.len()).unwrap_or(0);
            combo(dialog, view, bet.list, rows, Some(chosen), false);
        }
        let [champions, uefa] = records.countries.cup_names();
        set(view, dialog, CUP_NAMES[0], champions);
        set(view, dialog, CUP_NAMES[1], uefa);
    }

    /// A stake's spinner moved (`FUN_005d41bb` and the eight like it): no
    /// more than the room the season's stakes leave, else a step back.
    fn spun(&mut self, cx: &mut Cx, view: &mut View, c: usize) {
        let id = cx.dialog.controls[c].id.unwrap_or(0);
        let tables = &cx.game.rules.betting;
        let mut bets = Self::bets(cx);
        let page = self.page;
        let stake: &mut i32 = match id {
            _ if id == CHAMPION.spin => &mut bets.pages[page].stakes[0],
            _ if id == CUP.spin => &mut bets.pages[page].stakes[1],
            _ if id == TOP_SCORER.spin => &mut bets.pages[page].stakes[2],
            _ if id == SACKED_FIRST.spin => &mut bets.pages[page].stakes[3],
            _ if id == PLACE.spin => &mut bets.stakes[0],
            _ => match CUPS.iter().position(|b| b.spin == id) {
                Some(k) => &mut bets.stakes[k + 1],
                None => return,
            },
        };
        let current = *stake;
        let room = {
            let all = Self::bets(cx);
            betting::season_room(&all, fortune(cx))
        };
        let mut position = view.value(c).map_or(0, |v| v.value);
        let mut value = tables.stake_at(position);
        if value > room + current {
            if position > 0 {
                position -= 1;
            }
            value = tables.stake_at(position);
            if let Some(v) = view.value_mut(c) {
                v.set(position);
            }
        }
        *stake = value;
        Self::keep(cx, &bets);
        let bet_stake = [&CHAMPION, &CUP, &TOP_SCORER, &SACKED_FIRST, &PLACE]
            .into_iter()
            .chain(CUPS.iter())
            .find(|b| b.spin == id)
            .map(|b| b.stake);
        if let Some(stake_id) = bet_stake {
            set(view, cx.dialog, stake_id, money(cx.game, value.into()));
        }
    }

    /// A list's choice (`0x5d42d9` and the eight like it).
    fn chose(&mut self, cx: &mut Cx, view: &View, c: usize) {
        let id = cx.dialog.controls[c].id.unwrap_or(0);
        let row = view.combo(c).and_then(|combo| combo.selected).unwrap_or(0) as i16;
        let mut bets = Self::bets(cx);
        let page = self.page;
        let Some(records) = cx.state.records.as_ref() else { return };
        let offer = betting::season_offer(records, page).unwrap_or_default();
        match id {
            _ if id == CHAMPION.list => {
                let country = Self::country(cx, page);
                let league = a3_core::opponent::League::of(records, country, 0);
                bets.pages[page].champion =
                    usize::try_from(row - 1).ok().and_then(|i| Some(league.as_ref()?.team(i))).unwrap_or(-1);
            }
            _ if id == CUP.list => bets.pages[page].cup = row - 1,
            _ if id == TOP_SCORER.list => bets.pages[page].scorer = row - 1,
            // The trainer and the odds of the row below his, as the game
            // takes them; the first's for "Bitte auswählen" (`0x5d46bc`).
            _ if id == SACKED_FIRST.list => {
                let trainer = usize::try_from(row - 1).ok().and_then(|i| offer.trainers.get(i));
                bets.pages[page].trainer = trainer.map_or(-1, |t| t.id);
                if bets.pages[page].trainer != 0 {
                    let below = offer.trainers.get(usize::try_from(row).unwrap_or(0));
                    bets.pages[page].trainer_odds = below.map_or(0, |t| t.odds);
                }
            }
            // The place, and the club it is bet for.
            _ if id == PLACE.list => {
                bets.place = if row == 0 { -1 } else { row };
                let manager = human(cx).and_then(|h| records.managers.get(h));
                if let Some(m) = manager {
                    bets.place_club = (m.team() as i16, m.country() as i16);
                }
            }
            _ => match CUPS.iter().position(|b| b.list == id) {
                Some(k) => bets.cups[k] = row - 1,
                None => return,
            },
        }
        Self::keep(cx, &bets);
    }
}

/// A bet's spinner and list shown or hidden.
fn shown_bet(dialog: &Dialog, view: &mut View, bet: &Bet, on: bool) {
    shown(dialog, view, bet.spin, on);
    shown(dialog, view, bet.list, on);
}

impl Screen for SeasonBets {
    fn ids(&self) -> &[u16] {
        &[SEASON_DIALOG]
    }

    /// `0x5d26c2`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        title(cx, view);
        show_flags(cx, view);
        self.page(cx, view, 0);
        self.overall(cx, view);
        set(view, dialog, FORTUNE, money(game, fortune(cx).into()));
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        match target {
            Target::Control(c) if FLAGS.contains(&id(c)) => {
                let page = FLAGS.iter().position(|&f| f == id(c)).unwrap_or(0);
                self.page(cx, view, page);
            }
            Target::Spin { control: c, .. } => self.spun(cx, view, c),
            Target::Row { control: c, .. } => self.chose(cx, view, c),
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                return Outcome::Close(Reply::Ok);
            }
            _ => {}
        }
        Outcome::Stay
    }
}
