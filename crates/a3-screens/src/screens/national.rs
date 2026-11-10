//! The national team, the menu bar's sixteenth symbol: a menu of six buttons
//! on a picture (dialog 355, `CNationalmannschaft`), and its first button's
//! squad picker (dialog 357, `CNationalkaderZusammenstellen`; NATIONALMANNSCHAFT.md
//! in the RE repo).

use std::collections::HashMap;

use a3_core::cycle::dialogs::{LINEUP, NATIONAL_MENU, NATIONAL_SQUAD, NOTICE, QUESTION};
use a3_core::national;
use a3_core::season::Rules;
use a3_core::{lineup, load};
use a3_files::rch::{Align, Dialog};

use super::hub;
use super::{Args, Cx, Outcome, Reply, Request, Screen, control, mark_inert, set};
use crate::controls::options::NONE;
use crate::pointer::Target;
use crate::view::View;

/// The OK, hidden under the bar; the caption and its text, "ANSTOSS 3 -
/// Nationalmannschaft %s %s".
const OK: u16 = 1;
const CAPTION_ID: u16 = 9935;
const CAPTION: u16 = 3608;
/// The six buttons, one group (`CMyHighButton`).
const BUTTONS: u16 = 4538;
/// The first button, "Nationalkader zusammenstellen", opening the picker.
const PICK: usize = 0;
/// The last button, "Als Nationaltrainer zurücktreten", and its three
/// answers: refused before a match, sure?, accepted.
const RESIGN: usize = 5;
const NOT_BEFORE_THE_MATCH: u16 = 3615;
const REALLY_RESIGN: u16 = 3471;
const RESIGNATION_ACCEPTED: u16 = 3472;

/// The bar takes the OK's place; the caption.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    let Some(dialog) = dialogs.get_mut(&NATIONAL_MENU) else { return };
    dialog.controls.retain(|c| c.id != Some(OK));
    dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, String::new()));
    hub::add_bar(dialog, rules);
}

/// The picker's caption, its own across the top (`FUN_0051bf6e`'s caption
/// fill).
pub(crate) fn compose_picker(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&NATIONAL_SQUAD) else { return };
    dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, String::new()));
}

/// What the menu has opened over itself.
#[derive(Default, PartialEq)]
enum Open {
    #[default]
    Nothing,
    /// The whistle's question.
    Whistle,
    /// "Your chairman refuses", or a resignation asked after.
    Refused,
    Question,
    /// The resignation's last word; the line-up comes after it.
    Accepted,
}

/// The menu (`FUN_0051dd53`).
#[derive(Default)]
pub struct NationalMenu {
    open: Open,
}

impl Screen for NationalMenu {
    fn ids(&self) -> &[u16] {
        &[NATIONAL_MENU]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        hub::open_national_bar(cx, view);
        set(view, dialog, CAPTION_ID, title(cx));
        if let Some(buttons) = control(dialog, BUTTONS).and_then(|c| view.options_mut(c)) {
            buttons.actions = true;
            buttons.chosen = NONE;
        }
        let has_team = cx
            .state
            .season
            .as_ref()
            .and_then(|s| s.manager())
            .and_then(|m| m.national_team)
            .is_some();
        let works = |target: Target| match target {
            Target::Option { control: c, item: RESIGN } => dialog.controls[c].id == Some(BUTTONS),
            Target::Option { control: c, item: PICK } => {
                dialog.controls[c].id == Some(BUTTONS) && has_team
            }
            _ => hub::on_bar(dialog, target).is_some_and(hub::bar_works),
        };
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        if let Some(item) = hub::on_bar(cx.dialog, target) {
            let outcome = hub::bar_click(cx, view, item, hub::NATIONAL_SYMBOL);
            self.open = if matches!(outcome, Outcome::Popup(_)) { Open::Whistle } else { Open::Nothing };
            return outcome;
        }
        match target {
            Target::Option { control: c, item: RESIGN } if cx.dialog.controls[c].id == Some(BUTTONS) => {
                self.resign(cx, view)
            }
            Target::Option { control: c, item: PICK } if cx.dialog.controls[c].id == Some(BUTTONS) => {
                self.choose(cx, view)
            }
            _ => Outcome::Stay,
        }
    }

    /// What the menu opened has closed; the chosen button lets go only now
    /// (asm 0x51e3f9).
    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        match std::mem::take(&mut self.open) {
            Open::Whistle if reply == Reply::Ok => return Outcome::Close(Reply::Ok),
            Open::Question if reply == Reply::Ok => {
                resigned(cx);
                self.open = Open::Accepted;
                return Outcome::Popup(Request::with(NOTICE, Args::Text(RESIGNATION_ACCEPTED)));
            }
            Open::Accepted => return Outcome::Close(Reply::Goto(LINEUP)),
            _ => {}
        }
        hold(cx, view, None);
        Outcome::Stay
    }
}

impl NationalMenu {
    /// "Nationalkader zusammenstellen" (asm 0x51e25f): the picker over the
    /// menu; only a coach with a nation has a squad to put together.
    fn choose(&mut self, cx: &mut Cx, view: &mut View) -> Outcome {
        let has_team = cx
            .state
            .season
            .as_ref()
            .and_then(|s| s.manager())
            .and_then(|m| m.national_team)
            .is_some();
        if !has_team {
            return Outcome::Stay;
        }
        hold(cx, view, Some(PICK));
        Outcome::Popup(Request::new(NATIONAL_SQUAD))
    }

    /// The resignation (asm 0x51e2b2): only a manager with a nation has
    /// one to give, not in a week his team plays.
    fn resign(&mut self, cx: &mut Cx, view: &mut View) -> Outcome {
        let season = cx.state.season.as_ref();
        let Some(nation) = season.and_then(|s| s.manager()).and_then(|m| m.national_team) else {
            return Outcome::Stay;
        };
        let refused = cx.state.records.as_ref().is_some_and(|r| {
            national::plays_this_week(r, nation as i16)
        });
        hold(cx, view, Some(RESIGN));
        if refused {
            self.open = Open::Refused;
            return Outcome::Popup(Request::with(NOTICE, Args::Text(NOT_BEFORE_THE_MATCH)));
        }
        self.open = Open::Question;
        Outcome::Popup(Request::with(QUESTION, Args::Text(REALLY_RESIGN)))
    }
}

/// Holds a button down, as the group draws its chosen item, or lets it go:
/// the menu's buttons stay chosen while what they opened is up.
fn hold(cx: &Cx, view: &mut View, item: Option<usize>) {
    if let Some(buttons) = control(cx.dialog, BUTTONS).and_then(|c| view.options_mut(c)) {
        buttons.actions = item.is_none();
        buttons.chosen = item.unwrap_or(NONE);
    }
}

/// The resignation accepted: the job goes, in the records and the season
/// (`FUN_006b9872`, reason 4).
fn resigned(cx: &mut Cx) {
    let Some(season) = cx.state.season.as_mut() else { return };
    if let Some(records) = cx.state.records.as_mut()
        && let Some(slot) = records.human_slot(season.current)
    {
        national::leave(records, slot, national::RESIGNED);
    }
    if let Some(manager) = season.managers.get_mut(season.current) {
        manager.national_team = None;
    }
}

/// The caption: the manager, his nation in brackets, the date; without a
/// nation no brackets and no comma ("%s (%s), %s" at 0x8858b4, "%s %s" at
/// 0x8858c0).
fn title(cx: &Cx) -> String {
    let game = cx.game;
    let Some(season) = cx.state.season.as_ref() else { return String::new() };
    let Some(manager) = season.manager() else { return String::new() };
    let who = game
        .text(CAPTION)
        .replacen("%s", &manager.first_name, 1)
        .replacen("%s", &manager.last_name, 1);
    let date = hub::date_text(game, season.date());
    match manager.national_team {
        Some(nation) => format!("{who} ({}), {date}", game.world.nation(nation)),
        None => format!("{who} {date}"),
    }
}

/// The picker's controls (`FUN_0051bf6e`): the coach's own squad, the
/// bests on their positions, the country's clubs to search, and the free
/// slots under the squad.
const LAND: u16 = 4554;
const VEREIN: u16 = 4555;
const SQUAD_LIST: u16 = 4556;
const BESTS_LIST: u16 = 4558;
const CLUB_LIST: u16 = 4553;
const FREE_SLOTS: u16 = 4559;
/// The positions row and the info picture, a click on either not wired
/// yet.
const POSITIONS: u16 = 4775;
const INFO: u16 = 6358;
/// What the squad's count leaves in the static under it (`FUN_0051ca6c`):
/// text 3493 for one slot left, 3494 for a full squad, otherwise its own
/// "Sie haben noch Platz für %s Spieler." filled with the free slots.
const SQUAD_ONE_LEFT: u16 = 3493;
const SQUAD_FULL: u16 = 3494;
const SQUAD_MOST: usize = 0x16;
const BLACK: [u8; 3] = [0, 0, 0];
const RED: [u8; 3] = [255, 0, 0];

/// The coach's squad, held in the picker until OK writes it back.
#[derive(Default)]
pub struct NationalSquad {
    /// The Land combo's nation (`+0x4344`), the club's nation byte.
    nation: u16,
    /// The Verein combo's number in that country (`+0x4346`).
    club: u16,
}

impl Screen for NationalSquad {
    fn ids(&self) -> &[u16] {
        &[NATIONAL_SQUAD]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        self.nation = cx
            .state
            .season
            .as_ref()
            .and_then(|s| s.manager())
            .and_then(|m| m.national_team)
            .unwrap_or(0);
        self.club = 0;
        set(view, dialog, CAPTION_ID, picker_title(cx, self.nation));
        for id in [POSITIONS, INFO] {
            if let Some(options) = control(dialog, id).and_then(|c| view.options_mut(c)) {
                options.actions = true;
                options.chosen = NONE;
            }
        }
        self.fill_land(cx, view);
        self.fill_verein(cx, view);
        self.fill_bests(cx, view);
        self.fill_squad(cx, view);
        self.fill_search(cx, view);
        mark_inert(view, dialog, |_| false);
    }

    /// Nothing clicks yet: OK writes the squad back and the combos search
    /// only once the picking lands.
    fn input(&mut self, _: &mut Cx, _: &mut View, _: Target, _: bool) -> Outcome {
        Outcome::Stay
    }
}

impl NationalSquad {
    /// The Land combo (`FUN_0051bf6e`): every country the table of kinds
    /// keeps (`DAT_00ae61ae`), named by its nation, chosen this team's.
    fn fill_land(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let (Some(records), Some(c)) = (cx.state.records.as_ref(), control(dialog, LAND)) else {
            return;
        };
        let mut items = Vec::new();
        let mut selected = None;
        for country in 1..0x75usize {
            if records.countries.kind(country) == 0 {
                continue;
            }
            if country as u16 == self.nation {
                selected = Some(items.len());
            }
            items.push(game.world.nation(country as u16).to_owned());
        }
        if let Some(combo) = view.combo_mut(c) {
            combo.items = items;
            combo.selected = selected;
            combo.inks.clear();
            combo.open = false;
            combo.scroll = 0;
        }
    }

    /// The Verein combo (`FUN_0051d25a`): the country's clubs; a club's row
    /// is red while it plays someone of the nation it may call up
    /// (`FUN_0051d5fa`: his own country, not retiring, not already booked).
    fn fill_verein(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let (Some(records), Some(c)) = (cx.state.records.as_ref(), control(dialog, VEREIN)) else {
            return;
        };
        let count = records.countries.team_count(usize::from(self.nation)).max(0) as usize;
        let mut items = Vec::new();
        let mut inks = Vec::new();
        for number in 0..count as u16 {
            items.push(game.club_name(Some(records), self.nation as u8, number as i16));
            let calls = records
                .team(self.nation, number)
                .is_some_and(|team| team.players().iter().any(|&id| self.pickable(records, id)));
            inks.push(Some(if calls { RED } else { BLACK }));
        }
        if let Some(combo) = view.combo_mut(c) {
            combo.items = items;
            combo.inks = inks;
            combo.selected = (count != 0).then_some(0);
            combo.open = false;
            combo.scroll = 0;
        }
    }

    /// The bests list (`FUN_0051c33f`) stays empty: its table, `DAT_00a55e76`
    /// at the coach's own `0x9c0`, is filled by a process the replica does
    /// not have.
    fn fill_bests(&self, cx: &Cx, view: &mut View) {
        let Some(c) = control(cx.dialog, BESTS_LIST) else { return };
        if let Some(list) = view.list_mut(c) {
            list.rows.clear();
            list.inks.clear();
            list.row_icons.clear();
            list.keys.clear();
            list.selected = None;
            list.scroll = 0;
        }
    }

    /// The squad list (`FUN_0051ca6c`): his name, position, strength, form,
    /// expected strength, age and weeks out — the days he is out by, plus
    /// six, in whole weeks; the abilities in its three icon columns.  A
    /// player of the unknown and guessed strength classes
    /// shows "?" for his strength, and one of the unknown class shows an
    /// expected strength of 1.0; the others take `FUN_006a144e`.
    fn fill_squad(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let (Some(records), Some(season), Some(c)) = (
            cx.state.records.as_ref(),
            cx.state.season.as_ref(),
            control(dialog, SQUAD_LIST),
        ) else {
            return;
        };
        let slots = crate::controls::list::icon_slots(&dialog.controls[c]);
        let pitch = &game.rules.pitch;
        let today = season.date();
        let (mut rows, mut inks, mut icons, mut keys) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for id in national::squad_ids(records, self.nation as i16) {
            let (Some(record), Some((player, fitness, status))) =
                (records.player(id), load::any_player(records, &game.data, id))
            else {
                continue;
            };
            let class = record.picks() >> 6;
            let strength = if matches!(class, 1 | 3) { "?".to_owned() } else { player.strength.to_string() };
            let expected =
                if class == 1 { 10 } else { pitch.strength_at(&player, fitness, player.positions[0]) };
            rows.push(
                [
                    hub::name(&player),
                    hub::abbreviation(game, player.positions[0]),
                    strength,
                    lineup::form(fitness).to_string(),
                    hub::tenths(expected),
                    player.age(today).to_string(),
                    if status.injured < 1 {
                        String::new()
                    } else {
                        ((status.injured + 6) / 7).to_string()
                    },
                ]
                .join("\t"),
            );
            inks.push(Some(BLACK));
            icons.push(slots.iter().copied().zip(pitch.ability_icons(&player)).collect());
            keys.push(usize::from(id));
        }
        let filled = keys.len();
        if let Some(list) = view.list_mut(c) {
            // The screen hands the list new columns as it fills it (`FUN_0051ca6c`
            // calling `FUN_007a941f` with eight columns from `DAT_008856d0` and
            // `DAT_008856f0`): the position starts at 105, not the dialog's 100,
            // and the strength stands right-aligned at 130 in a width of 20, not
            // centred in 30.  The other view would pass `DAT_00885710` and
            // `DAT_00885724`, five columns for a non-national trainer.
            list.tabs = vec![0.0, 105.0, 130.0, 165.0, 195.0, 235.0, 335.0];
            list.aligns = vec![
                Some((Align::Left, 0.0)),
                Some((Align::Centre, 25.0)),
                Some((Align::Right, 20.0)),
                Some((Align::Right, 20.0)),
                Some((Align::Right, 30.0)),
                Some((Align::Centre, 30.0)),
                Some((Align::Centre, 30.0)),
            ];
            list.rows = rows;
            list.inks = inks;
            list.row_icons = icons;
            list.keys = keys;
            list.selected = None;
            list.scroll = 0;
        }
        let free = if filled == SQUAD_MOST {
            game.text(SQUAD_FULL).to_owned()
        } else if filled == SQUAD_MOST - 1 {
            game.text(SQUAD_ONE_LEFT).to_owned()
        } else {
            let template = control(dialog, FREE_SLOTS)
                .map_or(String::new(), |c| dialog.controls[c].text.clone());
            template.replacen("%s", &(SQUAD_MOST - filled).to_string(), 1)
        };
        set(view, dialog, FREE_SLOTS, free);
    }

    /// The list under the two combos (`FUN_0051d3be`): the chosen club's
    /// players of the nation it may call up, red while they are in the
    /// squad; name, position and expected strength, no icons.
    fn fill_search(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let (Some(records), Some(c)) = (cx.state.records.as_ref(), control(dialog, CLUB_LIST)) else {
            return;
        };
        let squad = national::squad_ids(records, self.nation as i16);
        let pitch = &game.rules.pitch;
        let (mut rows, mut inks, mut keys) = (Vec::new(), Vec::new(), Vec::new());
        if let Some(team) = records.team(self.nation, self.club) {
            for id in team.players() {
                if !self.pickable(records, id) {
                    continue;
                }
                let Some((player, fitness, _)) = load::any_player(records, &game.data, id) else {
                    continue;
                };
                rows.push(
                    [
                        hub::name(&player),
                        hub::abbreviation(game, player.positions[0]),
                        hub::tenths(pitch.strength_at(&player, fitness, player.positions[0])),
                    ]
                    .join("\t"),
                );
                inks.push(Some(if squad.contains(&id) { RED } else { BLACK }));
                keys.push(usize::from(id));
            }
        }
        if let Some(list) = view.list_mut(c) {
            list.rows = rows;
            list.inks = inks;
            list.row_icons.clear();
            list.keys = keys;
            list.selected = None;
            list.scroll = 0;
        }
    }

    /// A player of `self.nation` the coach may call up: not retiring, not
    /// already booked (`FUN_0051d5fa`'s and `FUN_0051d3be`'s filter).
    fn pickable(&self, records: &a3_core::records::Records, id: u16) -> bool {
        records.player(id).is_some_and(|r| {
            u16::from(r.nation()) == self.nation && r.retiring() & 2 == 0 && r.picks() & 2 == 0
        })
    }
}

/// The picker's caption ("%s %s %s (%s)" at 0x885804): the dialog's title,
/// the manager, his land.
fn picker_title(cx: &Cx, nation: u16) -> String {
    let Some(season) = cx.state.season.as_ref() else { return String::new() };
    let Some(manager) = season.manager() else { return String::new() };
    format!(
        "{} {} {} ({})",
        cx.dialog.title,
        manager.first_name,
        manager.last_name,
        cx.game.world.nation(nation)
    )
}
