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
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
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
/// "Schiedsrichter %s bestechen", "Gegner %s bestechen", "Bitte
/// auswählen".
const REFEREE_TEXT: u16 = 4514;
const OPPONENT_TEXT: u16 = 4515;
const CHOOSE: u16 = 4945;
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
        let others: Vec<String> = if closed & illegal::ATTACKS_DONE == 0 {
            records
                .humans()
                .filter(|h| h.human() != m.human())
                .map(|h| format!("{} {}", h.first_name(), h.last_name()))
                .collect()
        } else {
            Vec::new()
        };
        if others.is_empty() {
            hide(view, &[HUMANS]);
        } else if let Some(rows) = control(dialog, HUMANS).and_then(|c| view.options_mut(c)) {
            for (row, label) in rows.labels.iter_mut().enumerate() {
                *label = others.get(row).cloned().unwrap_or_default();
            }
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
                    view.controls[b].hidden = v < 1;
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
                    set(view, dialog, GATE_KEPT, format!("{:.1}%", v * 100.0));
                } else {
                    m.set_cup_share_kept(v);
                    set(view, dialog, CUP_SHARE, format!("{:.1}%", 50.0 - v * 100.0));
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
            Target::Button(c) if id(c) == 1 => return Outcome::Close(Reply::Ok),
            _ => {}
        }
        Outcome::Stay
    }
}

impl Illegal {
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

/// None of the uses is built yet: their OKs are crossed out as they show.
fn cross(view: &mut View, dialog: &Dialog) {
    mark_inert(view, dialog, |t| match t {
        Target::Button(c) => dialog.controls[c].id == Some(1),
        _ => true,
    });
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
