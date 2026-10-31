//! The black cash box (dialog 442, `CDiIllegales`), behind the Finanzen
//! screen's picture button 2224 (`0x4a3864` → `FUN_004c74f8`): what the
//! box holds and the club's money, the share of the gates kept back for
//! it and of a cup tie's gate kept from the opponent, and its seven uses,
//! each with its amount on a slider ([`a3_core::illegal`]).

use std::collections::HashMap;

use a3_core::finances;
use a3_core::illegal::{self, CUP_SHARE_MOST, STEPS};
use a3_files::rch::Dialog;

use super::discipline::CAPTION_ID;
use super::finances::money64;
use super::playerinfo::money;
use super::{Args, Cx, Outcome, Reply, Request, Screen, control, mark_inert, set};
use crate::controls::bitmap::Picture;
use crate::controls::options::NONE;
use crate::pointer::Target;
use crate::view::View;

pub(crate) const ILLEGAL: u16 = 442;

const CASH: u16 = 5528;
const FORTUNE: u16 = 5282;
const GATE_KEPT: u16 = 5529;
const CUP_SHARE: u16 = 5530;
const GATE_SPIN: u16 = 5544;
const CUP_SPIN: u16 = 5545;
/// The referee: title, slider, amount, OK.
const REFEREE_TITLE: u16 = 5546;
const REFEREE_SLIDER: u16 = 5539;
const REFEREE_AMOUNT: u16 = 5540;
const REFEREE_OK: u16 = 5551;
/// Another team's opponents: the team, slider, amount, OK.
const INCITE_TEAMS: u16 = 5542;
const INCITE_SLIDER: u16 = 5531;
const INCITE_AMOUNT: u16 = 5548;
const INCITE_OK: u16 = 5552;
/// The next opponent: title, slider, amount, OK.
const OPPONENT_TITLE: u16 = 5547;
const OPPONENT_SLIDER: u16 = 5532;
const OPPONENT_AMOUNT: u16 = 5537;
const OPPONENT_OK: u16 = 5553;
/// One of his players: the player, slider, amount, OK.
const PLAYERS: u16 = 5541;
const PLAYER_SLIDER: u16 = 5538;
const PLAYER_AMOUNT: u16 = 5549;
const PLAYER_OK: u16 = 5554;
/// Into the books, into the pocket: slider, amount, OK.
const BOOKS_SLIDER: u16 = 5533;
const BOOKS_AMOUNT: u16 = 5535;
const BOOKS_OK: u16 = 5555;
const POCKET_SLIDER: u16 = 5534;
const POCKET_AMOUNT: u16 = 5536;
const POCKET_OK: u16 = 5556;
/// The attack: the other humans, the kinds, the choice, its OK, its
/// picture.
const HUMANS: u16 = 5558;
const ATTACK_KINDS: u16 = 5550;
const ATTACK_TARGETS: u16 = 5543;
const ATTACK_CHOICE: u16 = 6414;
const ATTACK_OK: u16 = 5557;
const PORTRAIT: u16 = 6415;
/// The face shown when a portrait cannot be read (`FUN_004253a3(0xa48)`).
const NO_PORTRAIT: u16 = 2632;
/// The drawn portraits' ids, one for each human.
const PORTRAIT_DRAWN: u64 = 0x442_0000;
/// "Schiedsrichter %s bestechen", "Gegner %s bestechen", "Bitte
/// auswählen".
const REFEREE_TEXT: u16 = 4514;
const OPPONENT_TEXT: u16 = 4515;
const CHOOSE: u16 = 4945;
/// The gates' takings as booked (club +0x230), which the books take.
const TICKETS_BOOKED: usize = 0x230;
/// The league's next match is of this kind (club +0xb4c).
const LEAGUE_MATCH: u8 = 2;
/// The club's league, 0 the first (club +0x105 from team +0x12e).
const LEAGUE: usize = 0x12e;

/// The six amounts, in the original's order (+0x337c …): the referee, the
/// player, the opponent, the incited team, the books, the pocket. The
/// player's slider sets the incited team's amount and the other way
/// round (`FUN_004c975d`, `FUN_004c8d65`).
#[derive(Clone, Copy)]
enum Amount {
    Referee,
    Player,
    Opponent,
    Incite,
    Books,
    Pocket,
}

/// Each slider, the amount it sets and the text and OK it shows.
const SLIDERS: [(u16, Amount, u16, u16); 6] = [
    (REFEREE_SLIDER, Amount::Referee, REFEREE_AMOUNT, REFEREE_OK),
    (INCITE_SLIDER, Amount::Incite, INCITE_AMOUNT, INCITE_OK),
    (OPPONENT_SLIDER, Amount::Opponent, OPPONENT_AMOUNT, OPPONENT_OK),
    (PLAYER_SLIDER, Amount::Player, PLAYER_AMOUNT, PLAYER_OK),
    (BOOKS_SLIDER, Amount::Books, BOOKS_AMOUNT, BOOKS_OK),
    (POCKET_SLIDER, Amount::Pocket, POCKET_AMOUNT, POCKET_OK),
];

#[derive(Default)]
pub struct Illegal {
    amounts: [i64; 6],
    /// The incite combo's rows past the first: the match and its side.
    incite_rows: Vec<(usize, bool)>,
    /// The bribe combo's rows past the first: the players' ids.
    players: Vec<u16>,
    /// The other humans by their places among the managers, in the rows.
    humans: Vec<usize>,
    /// The attack's cost, and the chosen human's players in its list.
    attack_cost: i64,
    targets: Vec<u16>,
    /// What a use's answer, once read, hides, and the list it disables.
    closing: Vec<u16>,
    disabling: Option<u16>,
}

impl Screen for Illegal {
    fn ids(&self) -> &[u16] {
        &[ILLEGAL]
    }

    /// `FUN_004c75b0`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        *self = Self::default();
        let Some(season) = cx.state.season.as_ref() else { return };
        let Some(manager) = season.manager() else { return };
        let club_name = manager
            .club
            .and_then(|(slot, c)| game.world.country(slot)?.clubs.get(c))
            .map_or_else(String::new, |c| c.name.clone());
        let caption = format!(
            "{} {} {} ({club_name})",
            dialog.title, manager.first_name, manager.last_name
        );
        set(view, dialog, CAPTION_ID, caption);
        let Some(records) = cx.state.records.as_ref() else { return };
        let Some(team) = super::squad::own_team(cx) else { return };
        let Some(m) = own_manager(cx) else { return };
        let closed = m.illegal_closed();
        let hide = |view: &mut View, ids: &[u16]| {
            for &id in ids {
                if let Some(c) = control(dialog, id) {
                    view.controls[c].hidden = true;
                }
            }
        };
        // The referee of the next match.
        let referee = records.referee_name(team.referee()).unwrap_or_default();
        set(view, dialog, REFEREE_TITLE, printf(game.text(REFEREE_TEXT), &referee));
        if closed & illegal::REFEREE_DONE != 0 {
            hide(view, &[REFEREE_OK, REFEREE_SLIDER, REFEREE_AMOUNT]);
        } else {
            set(view, dialog, REFEREE_AMOUNT, money(game, 0));
        }
        // Another team's opponents: the sides of the day's matches in the
        // club's country that may be incited; the one chosen last.
        let country = team.country();
        let mut items = vec![game.text(CHOOSE).to_owned()];
        let mut chosen = 0;
        let incited = m.incited();
        for (i, p) in illegal::pairings(records).iter().enumerate() {
            if p.home.0 == 0 || p.home.0 != country || p.away.0 != country {
                continue;
            }
            for (away, side, bit) in [(false, p.home, 1), (true, p.away, 2)] {
                if p.closed & bit != 0 {
                    continue;
                }
                items.push(game.club_name(Some(records), side.0, side.1));
                self.incite_rows.push((i, away));
                // The original keeps the row of the home side whichever
                // side matches (its +0x44 takes the home row's index).
                if [u16::from(side.0), side.1 as u16] == incited {
                    chosen = items.len() - 1 - usize::from(away);
                }
            }
        }
        let incite = !self.incite_rows.is_empty() && closed & illegal::INCITED == 0;
        if let Some(combo) = control(dialog, INCITE_TEAMS).and_then(|c| view.combo_mut(c)) {
            combo.items = items;
            combo.selected = Some(chosen);
            combo.disabled = !incite;
        }
        if incite {
            set(view, dialog, INCITE_AMOUNT, money(game, 0));
        } else {
            hide(view, &[INCITE_OK, INCITE_SLIDER, INCITE_AMOUNT]);
        }
        // The next opponent, if a league match is next.
        let opponent = game.club_name(Some(records), team.next_country(), team.next_number());
        set(view, dialog, OPPONENT_TITLE, printf(game.text(OPPONENT_TEXT), &opponent));
        let league_next = team.next_kind() == LEAGUE_MATCH;
        if league_next && closed & illegal::OPPONENT_DONE == 0 {
            set(view, dialog, OPPONENT_AMOUNT, money(game, 0));
        } else {
            hide(view, &[OPPONENT_OK, OPPONENT_SLIDER, OPPONENT_AMOUNT]);
        }
        // One of his players.
        let mut bribable = false;
        if league_next && closed & illegal::PLAYER_DONE == 0 {
            let squad = u16::try_from(team.next_number())
                .ok()
                .and_then(|n| records.team(team.next_country().into(), n))
                .map(|t| t.players())
                .unwrap_or_default();
            self.players = squad.into_iter().filter(|&id| id != 0).collect();
            let mut items = vec![game.text(CHOOSE).to_owned()];
            items.extend(self.players.iter().map(|&id| records.known_as(id).unwrap_or_default()));
            if let Some(combo) = control(dialog, PLAYERS).and_then(|c| view.combo_mut(c)) {
                combo.items = items;
                combo.selected = Some(0);
            }
            bribable = !self.players.is_empty();
            if bribable {
                set(view, dialog, PLAYER_AMOUNT, money(game, 0));
            }
        }
        if !bribable {
            hide(view, &[PLAYER_OK, PLAYER_SLIDER, PLAYERS, PLAYER_AMOUNT]);
        }
        // The attack: the other humans by name.
        // The managers in play by their numbers (`FUN_006a421a`), but him.
        let own = records.human_slot(season.current);
        self.humans = if closed & illegal::ATTACKS_DONE == 0 {
            let slots = records.globals.slots();
            (0..records.managers.len())
                .filter(|&i| slots.get(i).copied().unwrap_or(0) > 0 && Some(i) != own)
                .collect()
        } else {
            Vec::new()
        };
        let others: Vec<String> = self
            .humans
            .iter()
            .map(|&i| format!("{} {}", records.managers[i].first_name(), records.managers[i].last_name()))
            .collect();
        if others.is_empty() {
            hide(view, &[HUMANS]);
        } else if let Some(rows) = control(dialog, HUMANS).and_then(|c| view.options_mut(c)) {
            for (row, label) in rows.labels.iter_mut().enumerate() {
                *label = others.get(row).cloned().unwrap_or_default();
            }
            rows.off = (0..rows.labels.len()).map(|row| row >= others.len()).collect();
            rows.chosen = NONE;
        }
        set(view, dialog, BOOKS_AMOUNT, money(game, 0));
        set(view, dialog, POCKET_AMOUNT, money(game, 0));
        set(view, dialog, CASH, money(game, m.black_cash().into()));
        set(view, dialog, FORTUNE, money64(game, finances::cash_and_result(team)));
        // The two shares on their spinners, in tenths of a percent.
        let (gate, cup) = (m.gate_kept(), m.cup_share_kept());
        set(view, dialog, GATE_KEPT, format!("{:.1}%", gate * 100.0));
        set(view, dialog, CUP_SHARE, format!("{:.1}%", 50.0 - cup * 100.0));
        let gate_most = illegal::gate_most(team.get::<u8>(LEAGUE));
        for (id, most, v) in [(GATE_SPIN, gate_most, gate), (CUP_SPIN, CUP_SHARE_MOST, cup)] {
            if let Some(spin) = control(dialog, id).and_then(|c| view.value_mut(c)) {
                spin.range = (0, most);
                spin.set((v * 1000.0) as i32);
            }
        }
        // Every OK waits for its amount; the attack for its target.
        hide(view, &[
            REFEREE_OK, INCITE_OK, OPPONENT_OK, PLAYER_OK, BOOKS_OK, POCKET_OK, ATTACK_OK,
            ATTACK_KINDS, ATTACK_CHOICE, ATTACK_TARGETS,
        ]);
        self.ranges(cx, view);
        cross(view, dialog);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let (game, dialog) = (cx.game, cx.dialog);
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        // A button pressed paints itself.
        if let Target::Button(c) = target
            && let Some(b) = view.controls[c].state.as_button_mut()
        {
            b.unpainted = false;
        }
        match target {
            Target::SliderStep { control: c, .. }
            | Target::SliderAt { control: c, .. }
            | Target::SliderTrack(c) => {
                let Some(&(_, amount, text, ok)) = SLIDERS.iter().find(|s| s.0 == id(c)) else {
                    return Outcome::Stay;
                };
                let position = view.value(c).map_or(0, |v| v.value);
                let v = finances::amount_at(&STEPS, position);
                self.amounts[amount as usize] = v;
                self.ranges(cx, view);
                set(view, dialog, text, money(game, v));
                if let Some(b) = control(dialog, ok) {
                    if v < 1 {
                        view.controls[b].hidden = true;
                    } else {
                        show_unpainted(view, b);
                    }
                }
                cross(view, dialog);
            }
            // The gate's share kept, stepping up with the upper arrow
            // (`FUN_004ca68b`); the cup opponents', the other way round
            // (`FUN_004ca751` reads the arrows as a slider's). Both reach
            // the manager at once.
            Target::Spin { control: c, up } if id(c) == GATE_SPIN || id(c) == CUP_SPIN => {
                let gate = id(c) == GATE_SPIN;
                if !gate {
                    let was = own_manager(cx).map_or(0, |m| (m.cup_share_kept() * 1000.0) as i32);
                    if let Some(spin) = view.value_mut(c) {
                        spin.set(was + if up { -1 } else { 1 });
                    }
                }
                let v = view.value(c).map_or(0, |s| s.value) as f32 / 1000.0;
                let Some(m) = own_manager_mut(cx) else { return Outcome::Stay };
                if gate {
                    m.set_gate_kept(v);
                    set(view, dialog, GATE_KEPT, chopped(f64::from(v) * 100.0));
                } else {
                    m.set_cup_share_kept(v);
                    set(view, dialog, CUP_SHARE, chopped(50.0 - f64::from(v) * 100.0));
                }
            }
            // Another team chosen (`OnSelChange_IDC_AUFSTACHELN_COMBO`):
            // kept with the manager at once.
            Target::Row { control: c, .. } if id(c) == INCITE_TEAMS => {
                let row = view.combo(c).and_then(|combo| combo.selected).unwrap_or(0);
                let side = row.checked_sub(1).and_then(|r| self.incite_rows.get(r)).copied();
                let pairings = cx.state.records.as_ref().map(illegal::pairings).unwrap_or_default();
                if let Some((i, away)) = side
                    && let Some(p) = pairings.get(i)
                {
                    let (country, number) = if away { p.away } else { p.home };
                    if let Some(m) = own_manager_mut(cx) {
                        m.set_incited([country.into(), number as u16]);
                    }
                }
            }
            Target::Button(c) if id(c) == REFEREE_OK => return self.bribe_referee(cx, view),
            Target::Button(c) if id(c) == OPPONENT_OK => return self.bribe_opponent(cx, view),
            Target::Button(c) if id(c) == PLAYER_OK => return self.bribe_player(cx, view),
            // A player chosen (`OnSelChange_IDC_SPIELER_COMBO`): kept with
            // the manager at once.
            Target::Row { control: c, .. } if id(c) == PLAYERS => {
                let row = view.combo(c).and_then(|combo| combo.selected).unwrap_or(0);
                if let Some(&player) = row.checked_sub(1).and_then(|r| self.players.get(r))
                    && let Some(m) = own_manager_mut(cx)
                {
                    m.set_bribe_target(player);
                }
            }
            Target::Button(c) if id(c) == INCITE_OK => return self.incite(cx, view),
            Target::Option { control: c, .. } if id(c) == HUMANS => self.choose_human(cx, view),
            // A kind of attack: the other group cleared
            // (`OnClicked_IDC_ANSCHLAG_TYP`, `_TYP1`; `FUN_004ca256`).
            Target::Option { control: c, .. } if id(c) == ATTACK_KINDS || id(c) == ATTACK_CHOICE => {
                let other = if id(c) == ATTACK_KINDS { ATTACK_CHOICE } else { ATTACK_KINDS };
                if let Some(o) = control(dialog, other).and_then(|o| view.options_mut(o)) {
                    o.chosen = NONE;
                }
                match attack_kind(dialog, view) {
                    Some(kind) => self.attack_cost = illegal::ATTACKS[kind].0,
                    None => {
                        self.attack_cost = 0;
                        if let Some(b) = control(dialog, ATTACK_OK) {
                            view.controls[b].hidden = true;
                        }
                    }
                }
            }
            // His player chosen (`OnSelChange_IDC_ANSCHLAG_SPIELER_COMBO`).
            Target::Row { control: c, .. } if id(c) == ATTACK_TARGETS => {
                let row = view.combo(c).and_then(|combo| combo.selected).unwrap_or(0);
                if let Some(&player) = row.checked_sub(1).and_then(|r| self.targets.get(r))
                    && let Some(m) = own_manager_mut(cx)
                {
                    m.set_attacked_player(player);
                }
            }
            Target::Button(c) if id(c) == ATTACK_OK => return self.attack(cx, view),
            Target::Button(c) if id(c) == BOOKS_OK => self.into_books(cx, view),
            Target::Button(c) if id(c) == POCKET_OK => self.into_pocket(cx, view),
            Target::Button(c) if id(c) == 1 => return Outcome::Close(Reply::Ok),
            _ => {}
        }
        Outcome::Stay
    }

    /// A use's answer read: its section goes, the sliders reach anew.
    fn answer(&mut self, cx: &mut Cx, view: &mut View, _: Reply) -> Outcome {
        if let Some(combo) =
            self.disabling.take().and_then(|id| control(cx.dialog, id)).and_then(|c| view.combo_mut(c))
        {
            combo.disabled = true;
        }
        for id in std::mem::take(&mut self.closing) {
            if let Some(c) = control(cx.dialog, id) {
                view.controls[c].hidden = true;
            }
        }
        self.ranges(cx, view);
        cross(view, cx.dialog);
        Outcome::Stay
    }
}

impl Illegal {
    /// The referee asked (`OnClicked_IDC_SCHIEDSRICHTER_OK`): once a
    /// season, whatever he says. Taking it, he costs the amount and
    /// promises one of three favours, kept with the manager for the
    /// match; refusing, he costs nothing. His section goes.
    fn bribe_referee(&mut self, cx: &mut Cx, view: &mut View) -> Outcome {
        let amount = std::mem::take(&mut self.amounts[Amount::Referee as usize]);
        let Some(team) = super::squad::own_team(cx) else { return Outcome::Stay };
        let factor = illegal::league_factor(team.country(), team.get::<u8>(LEAGUE));
        let corruptible =
            cx.state.records.as_ref().is_some_and(|r| r.referee_corruptible(team.referee()));
        let Some(season) = cx.state.season.as_mut() else { return Outcome::Stay };
        let mut below = |n| season.random.below(n);
        let (taken, sure) = illegal::bribe_referee(amount, corruptible, factor, &mut below);
        let pick = if taken { below(3) } else { below(5) } as usize;
        let mut closed = illegal::REFEREE_DONE;
        let text = if taken {
            let (promise, bit) = illegal::REFEREE_PROMISES[pick];
            closed |= illegal::REFEREE_BRIBED | bit;
            Args::Texts(if sure { illegal::REFEREE_GREETING } else { 0 }, promise)
        } else {
            Args::Text(illegal::REFEREE_REFUSALS[pick])
        };
        if let Some(m) = own_manager_mut(cx) {
            m.set_illegal_closed(m.illegal_closed() | closed);
            if taken {
                m.set_black_cash(m.black_cash().wrapping_sub(amount as i32));
            }
        }
        if taken {
            let cash = own_manager(cx).map_or(0, |m| m.black_cash().max(0));
            set(view, cx.dialog, CASH, money(cx.game, cash.into()));
        }
        // The section goes once his answer is read.
        self.closing = vec![REFEREE_OK, REFEREE_SLIDER, REFEREE_AMOUNT];
        Outcome::Popup(Request::with(super::popups::NOTICE, text))
    }

    /// The next opponent asked to lose (`OnClicked_IDC_GEGNER_OK`): once a
    /// season; his captain takes an amount above his price
    /// (`FUN_004ca876`), which leaves the box for the opponent's human
    /// manager's box, if he has one (`FUN_004ca902`, `FUN_0069c869`).
    fn bribe_opponent(&mut self, cx: &mut Cx, view: &mut View) -> Outcome {
        let amount = std::mem::take(&mut self.amounts[Amount::Opponent as usize]);
        let Some(records) = cx.state.records.as_ref() else { return Outcome::Stay };
        let Some(team) = super::squad::own_team(cx) else { return Outcome::Stay };
        let factor = illegal::league_factor(team.country(), team.get::<u8>(LEAGUE));
        let (day, country, number) = (team.next_day(), team.next_country(), team.next_number());
        let opponent = u16::try_from(number).ok().and_then(|n| records.team(country.into(), n));
        let place = opponent
            .and_then(|o| a3_core::opponent::League::of(records, country.into(), o.get::<u8>(LEAGUE)))
            .map_or(0, |l| l.place_of(number));
        let human = opponent.map_or(0, |o| o.humans()[0]);
        let Some(season) = cx.state.season.as_mut() else { return Outcome::Stay };
        let mut below = |n| season.random.below(n);
        let price = illegal::opponent_price(day, place, factor, &mut below);
        let taken = price > 0 && price < amount;
        let text = if taken {
            illegal::OPPONENT_YES[below(2) as usize]
        } else {
            illegal::OPPONENT_NO[below(3) as usize]
        };
        if let Some(records) = cx.state.records.as_mut()
            && taken
            && let Some(m) = usize::from(human).checked_sub(1).and_then(|i| records.managers.get_mut(i))
        {
            m.set_black_cash(m.black_cash().wrapping_add(amount as i32));
        }
        if let Some(m) = own_manager_mut(cx) {
            let mut closed = m.illegal_closed() | illegal::OPPONENT_DONE;
            if taken {
                closed |= illegal::OPPONENT_BRIBED;
                m.set_black_cash(m.black_cash().wrapping_sub(amount as i32));
            }
            m.set_illegal_closed(closed);
        }
        if taken {
            let cash = own_manager(cx).map_or(0, |m| m.black_cash().max(0));
            set(view, cx.dialog, CASH, money(cx.game, cash.into()));
        }
        self.closing = vec![OPPONENT_OK, OPPONENT_SLIDER, OPPONENT_AMOUNT];
        Outcome::Popup(Request::with(super::popups::NOTICE, Args::Text(text)))
    }

    /// A player of the next opponent asked (`OnClicked_IDC_SPIELER_OK`):
    /// nothing until one is chosen; then once a season, the amount leaving
    /// the box if he takes it. The section goes.
    fn bribe_player(&mut self, cx: &mut Cx, view: &mut View) -> Outcome {
        let row = control(cx.dialog, PLAYERS).and_then(|c| view.combo(c)).and_then(|c| c.selected);
        if row.unwrap_or(0) == 0 {
            return Outcome::Stay;
        }
        let amount = std::mem::take(&mut self.amounts[Amount::Player as usize]);
        let Some(records) = cx.state.records.as_ref() else { return Outcome::Stay };
        let target = own_manager(cx).map_or(0, |m| m.bribe_target());
        let (scruples, strength) =
            records.player(target).map_or((0, 0), |p| (p.scruples(), p.strength()));
        let taken = illegal::player_takes(amount, scruples, strength);
        let Some(season) = cx.state.season.as_mut() else { return Outcome::Stay };
        let text = if taken {
            illegal::PLAYER_YES[season.random.below(2) as usize]
        } else {
            illegal::PLAYER_NO[season.random.below(3) as usize]
        };
        if let Some(m) = own_manager_mut(cx) {
            let mut closed = m.illegal_closed() | illegal::PLAYER_DONE;
            if taken {
                closed |= illegal::PLAYER_BRIBED;
                m.set_black_cash(m.black_cash().wrapping_sub(amount as i32));
            }
            m.set_illegal_closed(closed);
        }
        if taken {
            let cash = own_manager(cx).map_or(0, |m| m.black_cash().max(0));
            set(view, cx.dialog, CASH, money(cx.game, cash.into()));
        }
        self.closing = vec![PLAYER_OK, PLAYER_SLIDER, PLAYERS, PLAYER_AMOUNT];
        Outcome::Popup(Request::with(super::popups::NOTICE, Args::Text(text)))
    }

    /// Another team's opponents incited (`OnClicked_IDC_AUFSTACHELN_OK`):
    /// nothing until a team is chosen; then once a season, the amount
    /// leaving the box if it is enough. The section goes, its list
    /// disabled.
    fn incite(&mut self, cx: &mut Cx, view: &mut View) -> Outcome {
        let row = control(cx.dialog, INCITE_TEAMS).and_then(|c| view.combo(c)).and_then(|c| c.selected);
        if row.unwrap_or(0) == 0 {
            return Outcome::Stay;
        }
        let amount = std::mem::take(&mut self.amounts[Amount::Incite as usize]);
        let Some(records) = cx.state.records.as_ref() else { return Outcome::Stay };
        let Some(team) = super::squad::own_team(cx) else { return Outcome::Stay };
        let salaries: Vec<u16> = u16::try_from(team.next_number())
            .ok()
            .and_then(|n| records.team(team.next_country().into(), n))
            .map(|t| t.players().iter().filter_map(|&id| records.player(id)).map(|p| p.salary()).collect())
            .unwrap_or_default();
        let Some(season) = cx.state.season.as_mut() else { return Outcome::Stay };
        let mut below = |n| season.random.below(n);
        let taken = illegal::incite_takes(amount, &salaries, &mut below);
        let pick = below(2) as usize;
        let text = if taken { illegal::INCITE_YES[pick] } else { illegal::INCITE_NO[pick] };
        if let Some(m) = own_manager_mut(cx) {
            let mut closed = m.illegal_closed() | illegal::INCITED;
            if taken {
                closed |= illegal::INCITED_DONE;
                m.set_black_cash(m.black_cash().wrapping_sub(amount as i32));
            }
            m.set_illegal_closed(closed);
        }
        if taken {
            let cash = own_manager(cx).map_or(0, |m| m.black_cash().max(0));
            set(view, cx.dialog, CASH, money(cx.game, cash.into()));
        }
        self.closing = vec![INCITE_OK, INCITE_SLIDER, INCITE_AMOUNT];
        self.disabling = Some(INCITE_TEAMS);
        Outcome::Popup(Request::with(super::popups::NOTICE, Args::Text(text)))
    }

    /// A human chosen to attack, or the choice taken back
    /// (`OnClicked_IDC_MITSPIELER_WAEHLEN`): the kinds, the list of his
    /// players and the OK show, his portrait beside them.
    fn choose_human(&mut self, cx: &mut Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let row = control(dialog, HUMANS).and_then(|c| view.options(c)).map_or(NONE, |o| o.chosen);
        self.attack_cost = 0;
        for id in [ATTACK_KINDS, ATTACK_CHOICE] {
            if let Some(o) = control(dialog, id).and_then(|c| view.options_mut(c)) {
                o.chosen = NONE;
            }
        }
        let Some(&human) = self.humans.get(row) else {
            for id in [ATTACK_TARGETS, ATTACK_KINDS, ATTACK_CHOICE, ATTACK_OK] {
                if let Some(c) = control(dialog, id) {
                    view.controls[c].hidden = true;
                }
            }
            return;
        };
        for id in [ATTACK_TARGETS, ATTACK_KINDS, ATTACK_CHOICE] {
            if let Some(c) = control(dialog, id) {
                view.controls[c].hidden = false;
            }
        }
        if let Some(c) = control(dialog, ATTACK_OK) {
            show_unpainted(view, c);
        }
        let Some(records) = cx.state.records.as_ref() else { return };
        let other = &records.managers[human];
        // His club's players (`FUN_004caa33`), if he has one.
        self.targets = if other.country() > 0 {
            records.team(other.country(), other.team()).map(|t| t.players()).unwrap_or_default()
        } else {
            Vec::new()
        };
        let mut items = vec![game.text(CHOOSE).to_owned()];
        items.extend(self.targets.iter().map(|&id| records.known_as(id).unwrap_or_default()));
        if let Some(combo) = control(dialog, ATTACK_TARGETS).and_then(|c| view.combo_mut(c)) {
            combo.items = items;
            combo.selected = Some(0);
        }
        // His portrait from the game's folder, else the empty face
        // (`FUN_004d242a`).
        let file = format!("portrait/{}", other.portrait());
        let read = (!other.portrait().is_empty())
            .then(|| Picture { black_clear: false, ..Picture::file(file) }.load(game).ok())
            .flatten();
        // The control stretches it to its own size (`CMyBitmap`).
        if let Some(c) = control(dialog, PORTRAIT) {
            let r = dialog.controls[c].rect;
            let image = read.or_else(|| game.graphic(NO_PORTRAIT).ok());
            let picture = image.map(|image| {
                let (w, h) = (r.width() as u32, r.height() as u32);
                let fitted = stretch(&image, w, h);
                Picture::drawn(PORTRAIT_DRAWN + human as u64, fitted)
            });
            view.set_picture(c, picture);
        }
        if let Some(m) = own_manager_mut(cx) {
            m.set_attacked(human as u16 + 1);
        }
        cross(view, dialog);
    }

    /// The attack ordered (`OnClicked_IDC_ANSCHLAG_OK`): nothing without a
    /// kind; with too little in the box a word; else it costs its price
    /// and the section goes for the season.
    fn attack(&mut self, cx: &mut Cx, view: &mut View) -> Outcome {
        let dialog = cx.dialog;
        let Some(kind) = attack_kind(dialog, view) else { return Outcome::Stay };
        let cash = own_manager(cx).map_or(0, |m| i64::from(m.black_cash()));
        if self.attack_cost > cash {
            return Outcome::Popup(Request::with(
                super::popups::NOTICE,
                Args::Text(illegal::ATTACK_TOO_DEAR),
            ));
        }
        let cost = std::mem::take(&mut self.attack_cost);
        if let Some(m) = own_manager_mut(cx) {
            m.set_illegal_closed(m.illegal_closed() | illegal::ATTACKS[kind].1);
            m.set_black_cash(m.black_cash().wrapping_sub(cost as i32));
        }
        for id in [ATTACK_OK, ATTACK_TARGETS, ATTACK_KINDS, ATTACK_CHOICE, HUMANS] {
            if let Some(c) = control(dialog, id) {
                view.controls[c].hidden = true;
            }
        }
        for id in [ATTACK_KINDS, ATTACK_CHOICE] {
            if let Some(o) = control(dialog, id).and_then(|c| view.options_mut(c)) {
                o.chosen = NONE;
            }
        }
        let cash = own_manager(cx).map_or(0, |m| m.black_cash().max(0));
        set(view, dialog, CASH, money(cx.game, cash.into()));
        Outcome::Popup(Request::with(super::popups::NOTICE, Args::Text(illegal::ATTACK_ORDERED)))
    }

    /// "Geld in die Bilanz bringen" (`OnClicked_IDC_BILANZ_OK`): the
    /// amount from the box to the gates' takings as booked (club +0x230);
    /// its OK stays up at nought. The club's money is told anew, now by
    /// `FUN_00621eac` (`FUN_004ca9f3`).
    fn into_books(&mut self, cx: &mut Cx, view: &mut View) {
        let v = std::mem::take(&mut self.amounts[Amount::Books as usize]) as i32;
        if let Some(m) = own_manager_mut(cx) {
            m.set_black_cash(m.black_cash().wrapping_sub(v));
        }
        let fund_prices = cx.state.records.as_ref().map(|r| r.globals.fund_prices());
        if let Some(team) = super::squad::own_team_mut(cx) {
            let at = TICKETS_BOOKED + finances::CLUB_TO_TEAM;
            team.put(at, team.get::<i32>(at).wrapping_add(v));
        }
        self.after_use(cx, view, BOOKS_SLIDER, BOOKS_AMOUNT);
        if let (Some(team), Some(prices)) = (super::squad::own_team(cx), fund_prices) {
            set(view, cx.dialog, FORTUNE, money64(cx.game, finances::figure(team, prices)));
        }
    }

    /// "Geld in die eigene Tasche bringen" (`OnClicked_IDC_TASCHE_OK`):
    /// the amount from the box to the manager's own money (+0x118).
    fn into_pocket(&mut self, cx: &mut Cx, view: &mut View) {
        let v = std::mem::take(&mut self.amounts[Amount::Pocket as usize]) as i32;
        if let Some(m) = own_manager_mut(cx) {
            m.set_black_cash(m.black_cash().wrapping_sub(v));
            m.set_cash(m.cash().wrapping_add(v));
        }
        self.after_use(cx, view, POCKET_SLIDER, POCKET_AMOUNT);
    }

    /// A use's slider back at nought, the box's money told anew, never
    /// below nought (`FUN_004ca992`), and the sliders' reach.
    fn after_use(&self, cx: &mut Cx, view: &mut View, slider: u16, amount: u16) {
        let dialog = cx.dialog;
        set(view, dialog, amount, money(cx.game, 0));
        if let Some(s) = control(dialog, slider).and_then(|c| view.value_mut(c)) {
            s.set(0);
        }
        if let Some(m) = own_manager_mut(cx)
            && m.black_cash() < 0
        {
            m.set_black_cash(0);
        }
        let cash = own_manager(cx).map_or(0, |m| m.black_cash());
        set(view, dialog, CASH, money(cx.game, cash.into()));
        self.ranges(cx, view);
    }

    /// Each slider reaches as far as the box holds less what the others
    /// are set at (`FUN_004cab8e`).
    fn ranges(&self, cx: &Cx, view: &mut View) {
        let cash = own_manager(cx).map_or(0, |m| i64::from(m.black_cash()));
        let all: i64 = self.amounts.iter().sum();
        for &(id, amount, _, _) in &SLIDERS {
            let rest = cash - (all - self.amounts[amount as usize]);
            let most = finances::step_of(&STEPS, rest);
            if let Some(slider) = control(cx.dialog, id).and_then(|c| view.value_mut(c)) {
                slider.range = (0, most);
                slider.set(slider.value);
            }
        }
    }
}

/// A share as the spinners' handlers write it ("%.1f%%"): cut to its
/// tenth, not rounded — 30 tenths of a percent, kept as a float, come out
/// "2.9%" there where the dialog's opening wrote "3.0%"; the game's
/// floating point runs chopping in them, seen on the original.
fn chopped(v: f64) -> String {
    format!("{:.1}%", (v * 10.0).floor() / 10.0)
}

/// A use's OK put up as the game does it (`ModifyStyle(0, WS_VISIBLE)`):
/// the dialog's picture stays there until the button is pointed at or
/// pressed. One up already keeps its look.
fn show_unpainted(view: &mut View, c: usize) {
    if view.controls[c].hidden {
        view.controls[c].hidden = false;
        if let Some(b) = view.controls[c].state.as_button_mut() {
            b.unpainted = true;
        }
    }
}

/// The uses not built yet: their OKs are crossed out as they show.
fn cross(view: &mut View, dialog: &Dialog) {
    mark_inert(view, dialog, |t| match t {
        Target::Button(c) => {
            matches!(
                dialog.controls[c].id,
                Some(
                    1 | REFEREE_OK
                        | INCITE_OK
                        | OPPONENT_OK
                        | PLAYER_OK
                        | ATTACK_OK
                        | BOOKS_OK
                        | POCKET_OK
                )
            )
        }
        _ => true,
    });
}

/// A picture stretched as Windows' `StretchBlt` does it: a source pixel
/// for each, stepping through the source in 16.16 fixed point from half a
/// step in (fitted to the original's portrait, pixel for pixel).
fn stretch(image: &image::RgbaImage, w: u32, h: u32) -> image::RgbaImage {
    let (sw, sh) = image.dimensions();
    let at = |i: u32, from: u32, to: u32| {
        let step = (u64::from(from) << 16) / u64::from(to.max(1));
        (((u64::from(i) * step + step / 2) >> 16) as u32).min(from.saturating_sub(1))
    };
    image::RgbaImage::from_fn(w, h, |x, y| *image.get_pixel(at(x, sw, w), at(y, sh, h)))
}

/// The kind of attack chosen: one of the first group's two, or the
/// second group's, which is the third (`FUN_004ca256`).
fn attack_kind(dialog: &Dialog, view: &View) -> Option<usize> {
    let chosen = |id| control(dialog, id).and_then(|c| view.options(c)).map_or(NONE, |o| o.chosen);
    if chosen(ATTACK_CHOICE) != NONE {
        return Some(2);
    }
    Some(chosen(ATTACK_KINDS)).filter(|&k| k < 2)
}

fn printf(text: &str, s: &str) -> String {
    super::finances::printf(text, &[&s])
}

/// The manager whose turn it is, his record.
fn own_manager<'a>(cx: &'a Cx) -> Option<&'a a3_core::records::Manager> {
    let season = cx.state.season.as_ref()?;
    let records = cx.state.records.as_ref()?;
    records.managers.get(records.human_slot(season.current)?)
}

fn own_manager_mut<'a>(cx: &'a mut Cx) -> Option<&'a mut a3_core::records::Manager> {
    let current = cx.state.season.as_ref()?.current;
    let records = cx.state.records.as_mut()?;
    let slot = records.human_slot(current)?;
    records.managers.get_mut(slot)
}

/// Puts the dialog's caption in its frame.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&ILLEGAL) else { return };
    let caption = super::caption(dialog.frame, CAPTION_ID, String::new());
    dialog.controls.insert(0, caption);
}
