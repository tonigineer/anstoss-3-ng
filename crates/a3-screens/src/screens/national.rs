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
const BLACK: [u8; 3] = [0, 0, 0];
const RED: [u8; 3] = [255, 0, 0];

/// The coach's squad, held in the picker until OK writes it back.
#[derive(Default)]
pub struct NationalSquad {
    /// The national club's nation byte (its `+0x104`): the coach's own
    /// land, what the squad and both lists filter on and OK writes back
    /// to — unchanged by the Land combo.
    nation: u16,
    /// The Land combo's country (`+0x4344`), the one whose clubs and
    /// players the search offers.
    land: u16,
    /// The countries the Land combo's items stand for, in order — the
    /// fill gives every item its country as its data (`FUN_0051bf6e`).
    lands: Vec<u16>,
    /// The Verein combo's number in that country (`+0x4346`).
    club: u16,
    /// The squad being put together, seeded from the team block.
    squad: Vec<u16>,
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
        // The Land combo starts at the national club's own land
        // (`FUN_0051bf6e`'s `+0x4344`), the Verein at its first club.
        self.land = self.nation;
        self.club = 0;
        self.squad = cx
            .state
            .records
            .as_ref()
            .map_or_else(Vec::new, |records| national::squad_ids(records, self.nation as i16));
        set(view, dialog, CAPTION_ID, picker_title(cx, self.nation));
        for id in [POSITIONS, INFO] {
            if let Some(options) = control(dialog, id).and_then(|c| view.options_mut(c)) {
                // Both groups are `CMyHighButton`s: they hold their
                // chosen item — the class starts it at 0, `+0x2e8` —
                // and a click on the chosen one lets it go again.  The
                // dialog's own fields start at 1 and −1 (`+0x4348`,
                // `+0x4350`) while both groups show item 0 chosen, so
                // the info picture needs two clicks for the other view.
                options.actions = false;
                options.toggle = true;
                options.chosen = 0;
            }
        }
        self.fill_land(cx, view);
        self.fill_verein(cx, view);
        self.fill_bests(cx, view);
        self.fill_squad(cx, view);
        self.fill_search(cx, view);
        mark_inert(view, dialog, |target| match target {
            Target::Button(c) => crate::controls::button::is_ok(&dialog.controls[c]),
            Target::Option { control: c, .. } => {
                matches!(dialog.controls[c].id, Some(POSITIONS) | Some(INFO))
            }
            _ => true,
        });
    }

    /// The picking (`CNationalkaderZusammenstellen`'s handlers): rows
    /// call up and send home, the combos move the search, the two groups
    /// re-fill, and OK writes the squad into the team's block — the copy
    /// the menu runs over the closed dialog (`FUN_0063df96`).  What the
    /// menu runs besides, the shirt numbers of `FUN_006d9a04`, burns the
    /// game's random and writes the national shirt byte at a record's
    /// +0x224 the replica keeps nowhere; it stays unported with the
    /// national match itself.
    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id;
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                if let Some(records) = cx.state.records.as_mut() {
                    national::set_squad(records, self.nation as i16, &self.squad);
                }
                return Outcome::Close(Reply::Ok);
            }
            // A squad row sends its player home only (`FUN_0051d16c`);
            // a bests or club row lets him in or out (`FUN_0051c981`,
            // `FUN_0051d6d4`); any change refills squad, bests and club
            // list, as all three handlers do.
            Target::Row { control: c, row }
                if matches!(id(c), Some(SQUAD_LIST) | Some(BESTS_LIST) | Some(CLUB_LIST)) =>
            {
                let Some(player) = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied())
                else {
                    return Outcome::Stay;
                };
                let player = player as u16;
                if player == 0 {
                    return Outcome::Stay;
                }
                let changed = if id(c) == Some(SQUAD_LIST) || self.squad.contains(&player) {
                    national::send_home(&mut self.squad, player)
                } else {
                    cx.state
                        .records
                        .as_ref()
                        .is_some_and(|records| national::call_up(&mut self.squad, records, player))
                };
                if changed {
                    self.fill_squad(cx, view);
                    self.fill_bests(cx, view);
                    self.fill_search(cx, view);
                }
                Outcome::Stay
            }
            // The Land combo (`FUN_0051d1fc`): its item's country, the
            // club back to the first, Verein and club list refilled —
            // squad and bests stay, and MFC fires only on a change.
            Target::Row { control: c, .. } if id(c) == Some(LAND) => {
                let chosen = view.combo(c).and_then(|combo| combo.selected);
                match chosen.and_then(|item| self.lands.get(item).copied()) {
                    Some(country) if country != self.land => {
                        self.land = country;
                        self.club = 0;
                        self.fill_verein(cx, view);
                        self.fill_search(cx, view);
                    }
                    _ => {}
                }
                Outcome::Stay
            }
            // The Verein combo (`FUN_0051d374`): its item's number, the
            // club list refilled.
            Target::Row { control: c, .. } if id(c) == Some(VEREIN) => {
                let chosen = view.combo(c).and_then(|combo| combo.selected);
                match chosen {
                    Some(item) if item as u16 != self.club => {
                        self.club = item as u16;
                        self.fill_search(cx, view);
                    }
                    _ => {}
                }
                Outcome::Stay
            }
            // The position group (`FUN_0051c948`): its chosen item plus
            // one, nothing chosen read as 0, is what the bests re-fill
            // takes — the group stands on the control, and the list is
            // empty whichever it is; only its table is unported.
            Target::Option { control: c, .. } if id(c) == Some(POSITIONS) => {
                self.fill_bests(cx, view);
                Outcome::Stay
            }
            // The info picture (`FUN_0051d7ca`): its chosen item is the
            // squad list's column mode and nothing chosen its first
            // view; the list re-fills whichever the click left.
            Target::Option { control: c, .. } if id(c) == Some(INFO) => {
                self.fill_squad(cx, view);
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}

impl NationalSquad {
    /// The Land combo (`FUN_0051bf6e`): every country the table of kinds
    /// keeps (`DAT_00ae61ae`), named by its nation, chosen this team's;
    /// its items' countries stand in `self.lands` for the handler.
    fn fill_land(&mut self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let (Some(records), Some(c)) = (cx.state.records.as_ref(), control(dialog, LAND)) else {
            return;
        };
        self.lands = (1..0x75usize)
            .filter(|&country| records.countries.kind(country) != 0)
            .map(|country| country as u16)
            .collect();
        let items: Vec<String> = self
            .lands
            .iter()
            .map(|&country| game.world.nation(country).to_owned())
            .collect();
        let selected = self.lands.iter().position(|&country| country == self.land);
        if let Some(combo) = view.combo_mut(c) {
            combo.items = items;
            combo.selected = selected;
            combo.inks.clear();
            combo.open = false;
            combo.scroll = 0;
        }
    }

    /// The Verein combo (`FUN_0051d25a`): the Land's clubs, its first one
    /// chosen; a club's row is red while it plays someone the national
    /// club may call up (`FUN_0051d5fa`: the national club's own nation,
    /// not retiring, not already booked — the Land only picks the clubs).
    fn fill_verein(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let (Some(records), Some(c)) = (cx.state.records.as_ref(), control(dialog, VEREIN)) else {
            return;
        };
        let count = records.countries.team_count(usize::from(self.land)).max(0) as usize;
        let mut items = Vec::new();
        let mut inks = Vec::new();
        for number in 0..count as u16 {
            items.push(game.club_name(Some(records), self.land as u8, number as i16));
            let calls = records
                .team(self.land, number)
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

    /// The bests list (`FUN_0051c33f`) stays empty: the group's number —
    /// `+0x4348`, the position control's chosen item plus one, what the
    /// handlers pass every re-fill — picks the block from a copy of the
    /// coach's own `0x9c0` bytes at `DAT_00a55e76`, 1 to 4 any other
    /// value zeroing that copy (`FUN_0069d359`); the table itself is
    /// filled by a process the replica does not have, so every value
    /// leaves the list empty and the click only moves the highlight.
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

    /// The squad list (`FUN_0051ca6c`), in whichever of its two views the
    /// info picture left it — its chosen item is the original's `+0x4350`
    /// and nothing chosen the first view.  The first: his name, position,
    /// strength, form, expected strength, age and weeks out — the days he
    /// is out by, plus six, in whole weeks; the abilities in its three
    /// icon columns.  A player of the unknown and guessed strength classes
    /// shows "?" for his strength, and one of the unknown class shows an
    /// expected strength of 1.0; the others take `FUN_006a144e`.  The
    /// second, the format at 0x88585c: name, position, strength, the club
    /// he plays for — the country and number at his +0x220 and +0x221 —
    /// and the grade of his last match, twice it in the low four bits of
    /// his +0x273 over 2.0, a dash from 0x88585a where there is none.
    fn fill_squad(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let (Some(records), Some(season), Some(c)) = (
            cx.state.records.as_ref(),
            cx.state.season.as_ref(),
            control(dialog, SQUAD_LIST),
        ) else {
            return;
        };
        // The info picture's chosen item, `FUN_007a71f7`'s GetCurSel of
        // the group behind `+0x4350`; −1, nothing chosen, the first view.
        let mode = control(dialog, INFO).map_or(-1, |c| match view.chosen(c) {
            NONE => -1,
            item => item as i32,
        });
        let slots = crate::controls::list::icon_slots(&dialog.controls[c]);
        let pitch = &game.rules.pitch;
        let today = season.date();
        let (mut rows, mut inks, mut icons, mut keys) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for &id in &self.squad {
            let (Some(record), Some((player, fitness, status))) =
                (records.player(id), load::any_player(records, &game.data, id))
            else {
                continue;
            };
            let class = record.picks() >> 6;
            let strength = if matches!(class, 1 | 3) { "?".to_owned() } else { player.strength.to_string() };
            inks.push(Some(BLACK));
            keys.push(usize::from(id));
            if mode < 0 {
                let expected = if class == 1 {
                    10
                } else {
                    pitch.strength_at(&player, fitness, player.positions[0])
                };
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
                icons.push(slots.iter().copied().zip(pitch.ability_icons(&player)).collect());
            } else {
                // The second view draws no icons and wants the club and
                // the grade in place of form, age and weeks.
                let [country, number] = record.club();
                let grade = record.grade() & 0xf;
                rows.push(
                    [
                        hub::name(&player),
                        hub::abbreviation(game, player.positions[0]),
                        strength,
                        game.club_name(Some(records), country, number.into()),
                        if grade == 0 {
                            "-".to_owned()
                        } else {
                            hub::tenths(5 * i32::from(grade))
                        },
                    ]
                    .join("\t"),
                );
            }
        }
        let filled = keys.len();
        if let Some(list) = view.list_mut(c) {
            if mode < 0 {
                // The screen hands the list new columns as it fills it
                // (`FUN_0051ca6c` calling `FUN_007a941f` with eight
                // columns from `DAT_008856d0` and `DAT_008856f0`): the
                // position starts at 105, not the dialog's 100, and the
                // strength stands right-aligned at 130 in a width of 20,
                // not centred in 30.
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
            } else {
                // The second view's five columns: the widths stand as
                // `DAT_00885724` reads them — 0, 25, 20, 20 and open for
                // the grade — but where they start, the original reads
                // from a runtime table whose entries are not in the file
                // (`DAT_00885710`); they stand where the first view's do.
                list.tabs = vec![0.0, 105.0, 130.0, 165.0, 195.0];
                list.aligns = vec![
                    Some((Align::Left, 0.0)),
                    Some((Align::Centre, 25.0)),
                    Some((Align::Right, 20.0)),
                    Some((Align::Left, 20.0)),
                    Some((Align::Left, 0.0)),
                ];
            }
            list.rows = rows;
            list.inks = inks;
            list.row_icons = icons;
            list.keys = keys;
            list.selected = None;
            list.scroll = 0;
        }
        let free = if filled == national::SQUAD_MOST {
            game.text(SQUAD_FULL).to_owned()
        } else if filled == national::SQUAD_MOST - 1 {
            game.text(SQUAD_ONE_LEFT).to_owned()
        } else {
            let template = control(dialog, FREE_SLOTS)
                .map_or(String::new(), |c| dialog.controls[c].text.clone());
            template.replacen("%s", &(national::SQUAD_MOST - filled).to_string(), 1)
        };
        set(view, dialog, FREE_SLOTS, free);
    }

    /// The list under the two combos (`FUN_0051d3be`): the chosen club's
    /// players of the nation the coach may call up, red while they stand
    /// in the squad he is putting together; name, position and expected
    /// strength, no icons.  The club follows the Land combo (`+0x4344`);
    /// the filter — the nation byte at the player's `+0x24`, equal to the
    /// national club's own — follows the coach, not the combo.
    fn fill_search(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let (Some(records), Some(c)) = (cx.state.records.as_ref(), control(dialog, CLUB_LIST)) else {
            return;
        };
        let pitch = &game.rules.pitch;
        let (mut rows, mut inks, mut keys) = (Vec::new(), Vec::new(), Vec::new());
        if let Some(team) = records.team(self.land, self.club) {
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
                inks.push(Some(if self.squad.contains(&id) { RED } else { BLACK }));
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
