//! The national team, the menu bar's sixteenth symbol: a menu of six buttons
//! on a picture (dialog 355, `CNationalmannschaft`), its first button's squad
//! picker (dialog 357, `CNationalkaderZusammenstellen`), its second button's
//! search for a foreign player to talk into changing his nation (dialog 351,
//! `CNationalSpielerSuchen`), its third button's overview of the talks so far
//! (dialog 471, `CEinbuergerungsgespraeche`), its fourth button's arranging
//! of friendlies on the international days (dialog 354,
//! `CNationalFreundschaftsspiel`) and its fifth button's infos on the
//! nations' tournament successes (dialog 472, `CInfoNationalteam`;
//! NATIONALMANNSCHAFT.md in the RE repo).

use std::collections::HashMap;

use a3_core::cycle::dialogs::{
    LINEUP, NATIONAL_FRIENDLY, NATIONAL_INFO, NATIONAL_MENU, NATIONAL_SEARCH, NATIONAL_SQUAD,
    NATIONAL_TALKS, NOTICE, QUESTION,
};
use a3_core::national;
use a3_core::season::Rules;
use a3_core::transfers;
use a3_core::{lineup, load};
use a3_files::rch::{Align, Dialog};

use super::hub;
use super::planner;
use super::{Args, Cx, Outcome, Reply, Request, Screen, control, mark_inert, set};
use crate::controls::bitmap::{Picture, PictureSource};
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
/// The second, "Spieler für ein Gespräch aussuchen", opening the search.
const SEARCH: usize = 1;
/// The third, "Infos über Einbürgerungsgespräche", opening the talks'
/// overview.
const TALKS: usize = 2;
/// The fourth, "Freundschaftsspiele", opening the arranger.
const FRIENDLY: usize = 3;
/// The fifth, "Infos über Nationalteams", opening the successes.
const INFOS: usize = 4;
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
    // The search's, the overview's, the arranger's and the infos' captions
    // are the dialogs' own titles, "ANSTOSS 3 - Spieler für Gespräch
    // aussuchen", "ANSTOSS 3 - Übersicht der Einbürgerungsgespräche",
    // "ANSTOSS 3 - Freundschaftsspiel vereinbaren" and "ANSTOSS 3 - Infos
    // über Nationalteams"; nothing changes in them either.
    for id in [NATIONAL_SEARCH, NATIONAL_TALKS, NATIONAL_FRIENDLY, NATIONAL_INFO] {
        if let Some(dialog) = dialogs.get_mut(&id) {
            let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
            dialog.controls.insert(0, caption);
        }
    }
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
        let has_team = coach_has_a_team(cx);
        let works = |target: Target| match target {
            Target::Option { control: c, item: RESIGN | TALKS | INFOS } => {
                dialog.controls[c].id == Some(BUTTONS)
            }
            Target::Option { control: c, item: PICK | SEARCH | FRIENDLY } => {
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
            Target::Option { control: c, item: SEARCH }
                if cx.dialog.controls[c].id == Some(BUTTONS) =>
            {
                self.find(cx, view)
            }
            Target::Option { control: c, item: TALKS }
                if cx.dialog.controls[c].id == Some(BUTTONS) =>
            {
                self.overview(cx, view)
            }
            Target::Option { control: c, item: FRIENDLY }
                if cx.dialog.controls[c].id == Some(BUTTONS) =>
            {
                self.arrange(cx, view)
            }
            Target::Option { control: c, item: INFOS }
                if cx.dialog.controls[c].id == Some(BUTTONS) =>
            {
                self.successes(cx, view)
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
        if !coach_has_a_team(cx) {
            return Outcome::Stay;
        }
        hold(cx, view, Some(PICK));
        Outcome::Popup(Request::new(NATIONAL_SQUAD))
    }

    /// "Spieler für ein Gespräch aussuchen": the search over the menu, for
    /// nothing else than the nation behind his team (`+0x17e6` of the
    /// manager, the same gate the dialog's item answers to).
    fn find(&mut self, cx: &mut Cx, view: &mut View) -> Outcome {
        if !coach_has_a_team(cx) {
            return Outcome::Stay;
        }
        hold(cx, view, Some(SEARCH));
        Outcome::Popup(Request::new(NATIONAL_SEARCH))
    }

    /// "Freundschaftsspiele": the arranger over the menu, for nothing
    /// else than the nation behind his team — like the picker and the
    /// search, only a coach with a nation has friendlies to arrange.
    fn arrange(&mut self, cx: &mut Cx, view: &mut View) -> Outcome {
        if !coach_has_a_team(cx) {
            return Outcome::Stay;
        }
        hold(cx, view, Some(FRIENDLY));
        Outcome::Popup(Request::new(NATIONAL_FRIENDLY))
    }

    /// "Infos über Einbürgerungsgespräche": the overview of the talks on
    /// his slate.
    /// Along with the successes the one button that opens for any coach,
    /// with a nation or without (asm 0x51e2c0; the dialog takes his id
    /// from the manager's +0x1a).
    fn overview(&mut self, cx: &mut Cx, view: &mut View) -> Outcome {
        hold(cx, view, Some(TALKS));
        Outcome::Popup(Request::new(NATIONAL_TALKS))
    }

    /// "Infos über Nationalteams": the nations' tournament successes.
    /// With the talks' overview the one button that opens for any coach,
    /// with a nation behind him or without (the chain at asm 0x51e02b);
    /// the dialog reads his own land from the manager's `+0x17e6`.
    fn successes(&mut self, cx: &mut Cx, view: &mut View) -> Outcome {
        hold(cx, view, Some(INFOS));
        Outcome::Popup(Request::new(NATIONAL_INFO))
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

/// Whether the manager is a national coach at all, his team behind him.
fn coach_has_a_team(cx: &Cx) -> bool {
    cx.state
        .season
        .as_ref()
        .and_then(|s| s.manager())
        .and_then(|m| m.national_team)
        .is_some()
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
    fn fill_land(&mut self, cx: &Cx, view: &mut View) {
        fill_land_combo(cx, view, LAND, &mut self.lands, self.land);
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

/// The Land combo of either national dialog (`FUN_0051bf6e`'s and
/// `FUN_0051f097`'s fill): every country the table of kinds keeps
/// (`DAT_00ae61ae`), named by its nation, chosen `land` (`FUN_007997be`
/// selects by the item's country); the items' countries stand in `lands`
/// for the handlers.
fn fill_land_combo(cx: &Cx, view: &mut View, id: u16, lands: &mut Vec<u16>, land: u16) {
    let (game, dialog) = (cx.game, cx.dialog);
    let (Some(records), Some(c)) = (cx.state.records.as_ref(), control(dialog, id)) else {
        return;
    };
    *lands = (1..0x75usize)
        .filter(|&country| records.countries.kind(country) != 0)
        .map(|country| country as u16)
        .collect();
    let items: Vec<String> = lands
        .iter()
        .map(|&country| game.world.nation(country).to_owned())
        .collect();
    let selected = lands.iter().position(|&country| country == land);
    if let Some(combo) = view.combo_mut(c) {
        combo.items = items;
        combo.selected = selected;
        combo.inks.clear();
        combo.open = false;
        combo.scroll = 0;
    }
}

/// The search's controls (`FUN_0051ee79`): the Land combo, the Verein
/// list, the player list and Abbruch — the last by id, its picture is
/// not the cancel one.
const SEARCH_LAND: u16 = 4510;
const SEARCH_CLUB: u16 = 4508;
const SEARCH_PLAYERS: u16 = 4507;
const ABBRUCH: u16 = 4509;
/// What the player list's last column says of the talks so far
/// (`FUN_006b8f5f`'s bits and the texts they name): the first word, the
/// offer, his answer.
const FIRST_WORD: u16 = 3475;
const THE_OFFER: u16 = 3476;
const HIS_ANSWER: u16 = 3477;
/// The click's alerts in the original's order: taken for a talk by this
/// coach already, he said no, the association allows no more talks.
const PICKED_ALREADY: u16 = 3478;
const SAID_NO: u16 = 3474;
const NO_MORE_TALKS: u16 = 3479;
/// The planner kind of a talk about his nation: the mode 0x11
/// `CSummeTerminplTerminAuswahl` is called with (`FUN_0051f715`), kept
/// as kind 0x11.
const NATION_TALK: u8 = 0x11;

/// What the search has open over itself: one of its alerts, or the
/// planner for the talk's day.
#[derive(Default, PartialEq)]
enum SearchOpen {
    #[default]
    Nothing,
    Notice,
    Planner,
}

/// The coach looking for a foreign player to talk into changing his
/// nation (`FUN_0051ee79`; NATIONALMANNSCHAFT.md §4): the lists offer a
/// chosen club's players who are not of his team's nation — the
/// foreigners — who have never worn a national shirt (the handle's
/// `+0x61`, the cap count off the record's +0x266 group), are not set in
/// their ways (`FUN_00707562`: his +0x282 manner without its 0x80 bit)
/// and stand below id 0x4269; a row asks the planner for a day and then
/// writes the search list.  What the talk itself says — dialog 353,
/// `CNationalGespraech`, and the flags `FUN_006b8c87` sets — waits for
/// the end-of-turn appointment, which the replica does not run yet.
#[derive(Default)]
pub struct NationalPlayerSearch {
    /// The nation behind his team (the manager's `+0x17e6`, the dialog's
    /// `+0x1edc`): the filter the player list runs against.
    nation: u16,
    /// The Land combo's country, the one whose clubs and players are
    /// offered; the combo opens on his own team's nation.
    land: u16,
    /// The countries the Land combo's items stand for, in order.
    lands: Vec<u16>,
    /// The Verein list's number in that country (`+0x1ede`, 0 from
    /// OnInit).
    club: u16,
    /// The player whose planner is open, so its answer knows whose
    /// search entry to write.
    picked: u16,
    open: SearchOpen,
}

impl Screen for NationalPlayerSearch {
    fn ids(&self) -> &[u16] {
        &[NATIONAL_SEARCH]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        // OnInit (`FUN_0051f097`): the land at his own team's — the
        // manager's +0x17e6, which the menu gate proved nonzero —, the
        // club at 0, the talk table copied out for the click's gates,
        // the Land combo filled and his country chosen in it, then the
        // players and the Verein list.  The caption keeps its composed
        // title; the static "Bitte wählen Sie einen Spieler aus!" stands
        // alone.
        self.nation = cx
            .state
            .season
            .as_ref()
            .and_then(|s| s.manager())
            .and_then(|m| m.national_team)
            .unwrap_or(0);
        self.land = self.nation;
        self.club = 0;
        self.picked = 0;
        self.open = SearchOpen::Nothing;
        fill_land_combo(cx, view, SEARCH_LAND, &mut self.lands, self.land);
        self.fill_club(cx, view);
        self.fill_players(cx, view);
        // Everything here answers a click; only the statics stand still.
        mark_inert(view, cx.dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id;
        match target {
            Target::Button(c) if id(c) == Some(ABBRUCH) => Outcome::Close(Reply::Cancel),
            // The Land combo (`FUN_0051f4a1`): its item's country, the
            // club back to 0, Verein list and players refilled — MFC
            // fires only on a change.
            Target::Row { control: c, .. } if id(c) == Some(SEARCH_LAND) => {
                let chosen = view.combo(c).and_then(|combo| combo.selected);
                match chosen.and_then(|item| self.lands.get(item).copied()) {
                    Some(country) if country != self.land => {
                        self.land = country;
                        self.club = 0;
                        self.fill_club(cx, view);
                        self.fill_players(cx, view);
                    }
                    _ => {}
                }
                Outcome::Stay
            }
            // The Verein list (`FUN_0051f591`): the row's number, the
            // players refilled for it.
            Target::Row { control: c, row } if id(c) == Some(SEARCH_CLUB) => {
                let number = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied());
                match number {
                    Some(n) if n as u16 != self.club => {
                        self.club = n as u16;
                        self.fill_players(cx, view);
                    }
                    _ => {}
                }
                Outcome::Stay
            }
            // The player list (`FUN_0051f715`).
            Target::Row { control: c, row } if id(c) == Some(SEARCH_PLAYERS) => {
                let player = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied());
                self.pick(cx, player.unwrap_or(0) as u16)
            }
            _ => Outcome::Stay,
        }
    }

    /// The planner has chosen the day (`FUN_007656f6`): the planner
    /// entry it booked names it, and the search list takes the player,
    /// that day and this coach's id.  An alert closing answers nothing.
    /// The MFC DAO waits — `FUN_0076330b`, `FUN_006ad59e` — stand in no
    /// way in the single-threaded replica.  The list stands as it is:
    /// the status column follows the talk's flags, which only the talk
    /// itself sets, and that waits with the turn.
    fn answer(&mut self, cx: &mut Cx, _: &mut View, reply: Reply) -> Outcome {
        if std::mem::take(&mut self.open) != SearchOpen::Planner || reply != Reply::Ok {
            return Outcome::Stay;
        }
        let (Some(records), Some(season)) = (cx.state.records.as_mut(), cx.state.season.as_ref())
        else {
            return Outcome::Stay;
        };
        let Some(slot) = records.human_slot(season.current) else {
            return Outcome::Stay;
        };
        // The planner was asked for this week; the talk sits in one of
        // its seven days.
        let start = usize::try_from(records.globals.week_start()).unwrap_or(0);
        let booked = records.planners.get(slot).and_then(|p| {
            (start..start + 7).find(|&d| {
                let a = p.day(d);
                a.kind == NATION_TALK && a.player == self.picked
            })
        });
        let Some(day) = booked else { return Outcome::Stay };
        let coach = records.managers.get(slot).map_or(0, |m| u16::from(m.human()));
        national::book_search(records, self.picked, day as i16, coach);
        Outcome::Stay
    }
}

impl NationalPlayerSearch {
    /// The Verein list (`FUN_0051f66d`): the Land's clubs in the list's
    /// sorted order (`FUN_007a90c1` keeps each before the first that
    /// compares greater), one name's clubs in their number.  Then
    /// `FUN_007a9fb0` selects `+0x1ede` as a *row* — 0 at OnInit and on
    /// a Land change — but the list paints that row plain, no chosen bar,
    /// until the coach clicks it himself; the players keep coming from
    /// the number in `+0x1ede`.
    fn fill_club(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let (Some(records), Some(c)) = (cx.state.records.as_ref(), control(dialog, SEARCH_CLUB))
        else {
            return;
        };
        let count = records.countries.team_count(usize::from(self.land)).max(0) as usize;
        let mut clubs: Vec<(String, usize)> = (0..count as u16)
            .map(|number| {
                (game.club_name(Some(records), self.land as u8, number as i16), usize::from(number))
            })
            .collect();
        clubs.sort_by_key(|(name, _)| super::search::stricmp(name));
        if let Some(list) = view.list_mut(c) {
            list.rows = clubs.iter().map(|(name, _)| name.clone()).collect();
            list.inks.clear();
            list.row_icons.clear();
            list.keys = clubs.into_iter().map(|(_, number)| number).collect();
            list.selected = None;
            list.scroll = 0;
        }
    }

    /// The player list (`FUN_0051f1be`): the chosen club's foreigners,
    /// uncapped, not set in their ways, below id 0x4269 (see the
    /// screen).  The row, the format at 0x8859e0: name, position,
    /// strength, the club's league — the byte at its `+0x105`, the
    /// record's +0x12e level, plus one and capped at three —, his age, his
    /// nation's three letters (the string table at 0xae5768, the nations'
    /// second name in `Laender.sav`) and the state of his talks so far.
    fn fill_players(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let (Some(records), Some(season), Some(c)) = (
            cx.state.records.as_ref(),
            cx.state.season.as_ref(),
            control(dialog, SEARCH_PLAYERS),
        ) else {
            return;
        };
        let slot = records.human_slot(season.current).unwrap_or(0);
        let (players, league) = match records.team(self.land, self.club) {
            Some(team) => (team.players(), (u16::from(team.level()) + 1).min(3)),
            None => (Vec::new(), 0),
        };
        let today = season.date();
        let (mut rows, mut keys) = (Vec::new(), Vec::new());
        for id in players {
            let Some(record) = records.player(id) else { continue };
            if id >= 0x4269
                || u16::from(record.nation()) == self.nation
                || record.kind_12_matches() != 0
                || record.manner() & 0x80 != 0
            {
                continue;
            }
            let Some((player, _, _)) = load::any_player(records, &game.data, id) else {
                continue;
            };
            let status = match national::talk_flags(records, slot, id) {
                f if f & 4 != 0 => game.text(HIS_ANSWER).to_owned(),
                f if f & 2 != 0 => game.text(THE_OFFER).to_owned(),
                f if f & 1 != 0 => game.text(FIRST_WORD).to_owned(),
                _ => String::new(),
            };
            rows.push(
                [
                    hub::name(&player),
                    hub::abbreviation(game, player.positions[0]),
                    player.strength.to_string(),
                    league.to_string(),
                    player.age(today).to_string(),
                    game
                        .world
                        .nation_codes
                        .get(usize::from(record.nation()))
                        .map_or("", String::as_str)
                        .to_owned(),
                    status,
                ]
                .join("\t"),
            );
            keys.push(usize::from(id));
        }
        if let Some(list) = view.list_mut(c) {
            list.rows = rows;
            list.inks.clear();
            list.row_icons.clear();
            list.keys = keys;
            list.selected = None;
            list.scroll = 0;
        }
    }

    /// A row of the player list (`FUN_0051f715`), in the original's
    /// gate order; then the planner for a talk with him, mode 0x11 of
    /// `CSummeTerminplTerminAuswahl`, booked as kind 0x11.
    fn pick(&mut self, cx: &mut Cx, player: u16) -> Outcome {
        if player == 0 {
            return Outcome::Stay;
        }
        let (Some(records), Some(season)) = (cx.state.records.as_ref(), cx.state.season.as_ref())
        else {
            return Outcome::Stay;
        };
        let Some(slot) = records.human_slot(season.current) else {
            return Outcome::Stay;
        };
        let coach = records.managers.get(slot).map_or(0, |m| u16::from(m.human()));
        // Taken for a talk by this coach already, the alert answers
        // (`FUN_00764ca8` over the search list).
        if national::searched(records, coach, player) {
            self.open = SearchOpen::Notice;
            return Outcome::Popup(Request::with(NOTICE, Args::Text(PICKED_ALREADY)));
        }
        // Anywhere pending — under another coach, an extension, a
        // negotiation (`FUN_007647b4` says free only when none of the
        // three lists hold him): nothing happens.
        if transfers::pending(records, player) {
            return Outcome::Stay;
        }
        // He has already said no to changing his nation, his flags' 4
        // (`FUN_006b8f5f` over the copied talk table).
        if national::talk_flags(records, slot, player) & national::ANSWERED != 0 {
            self.open = SearchOpen::Notice;
            return Outcome::Popup(Request::with(NOTICE, Args::Text(SAID_NO)));
        }
        // The association allows no more talks: the copied count, read
        // at the head of `FUN_00634f89`'s copy, may not reach 11.
        if national::talk_count(records, slot) > national::TALKS_ALLOWED {
            self.open = SearchOpen::Notice;
            return Outcome::Popup(Request::with(NOTICE, Args::Text(NO_MORE_TALKS)));
        }
        self.picked = player;
        self.open = SearchOpen::Planner;
        Outcome::Popup(Request::with(planner::PLANNER, Args::Book(NATION_TALK, player)))
    }
}

/// The talks' list, control 0x178f (`CListBoxSpielerInfo`).
const TALKS_LIST: u16 = 6031;

/// The overview of the talks so far (`CEinbuergerungsgespraeche`, dialog
/// 471): rows over the coach's talk entries, each the player's name
/// (`FUN_006feb4d`), his nation's three letters (the table at 0xae5768),
/// his club's name (`FUN_0063e52f` from the club's league and number, 200
/// standing for the national team) and how far the talk came: the first
/// word, the offer, his answer (texts 3475, 3476, 3477).  The class has
/// no handler but OK's: a row stands chosen and nothing answers it.
#[derive(Default)]
pub struct NationalTalks;

impl Screen for NationalTalks {
    fn ids(&self) -> &[u16] {
        &[NATIONAL_TALKS]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        fill_talks(cx, view);
        mark_inert(view, cx.dialog, |_| true);
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

/// The rows, in the save's order — the entries whose player stands 0 are
/// no talk and stand in no list (`FUN_0048dad5`; `FUN_007a90c1` appends,
/// the box not sorted, and its item keeps the player's id).
fn fill_talks(cx: &Cx, view: &mut View) {
    let (game, dialog) = (cx.game, cx.dialog);
    let (Some(records), Some(season), Some(c)) = (
        cx.state.records.as_ref(),
        cx.state.season.as_ref(),
        control(dialog, TALKS_LIST),
    ) else {
        return;
    };
    let slot = records.human_slot(season.current).unwrap_or(0);
    let (mut rows, mut keys) = (Vec::new(), Vec::new());
    for (id, flags) in national::talks(records, slot) {
        let Some(record) = records.player(id) else { continue };
        let name = records.known_as(id).unwrap_or_default();
        let nation = game
            .world
            .nation_codes
            .get(usize::from(record.nation()))
            .map_or("", String::as_str)
            .to_owned();
        let [country, number] = record.club();
        let status = match flags {
            f if f & 4 != 0 => HIS_ANSWER,
            f if f & 2 != 0 => THE_OFFER,
            _ => FIRST_WORD,
        };
        rows.push(
            [
                name,
                nation,
                game.club_name(Some(records), country, i16::from(number)),
                game.text(status).to_owned(),
            ]
            .join("\t"),
        );
        keys.push(usize::from(id));
    }
    if let Some(list) = view.list_mut(c) {
        list.rows = rows;
        list.inks.clear();
        list.row_icons.clear();
        list.keys = keys;
        list.selected = None;
        list.scroll = 0;
    }
}

/// The infos dialog's Land combo, its flag and its list of rows
/// (controls 6028, 6029 and 6030).
const INFO_LAND: u16 = 6028;
const INFO_FLAG: u16 = 6029;
const INFO_LIST: u16 = 6030;

/// The infos on the national teams (`CInfoNationalteam`, dialog 472):
/// a combo of the lands 1 to 115, chosen to the coach's own nation when
/// he has one — without, nothing is chosen and the rows come from land
/// 0 (`DAT_00b5cbb0`, a global never written) — the land's big flag
/// `gross/<name>.bmp` under the header "Wettbewerb / Erfolg / Trainer"
/// and rows of its tournaments, "%s %d\t%s\t%s" over contest and year,
/// result text and coach, newest first (`FUN_004cb3b2`).  The results
/// table stands zero — no code writes it and no save block covers it —
/// so the list is empty for every land (screen c1-item4-erfolge); the
/// combo still moves flag and rows together, and no row answers a click.
#[derive(Default)]
pub struct NationalTeamInfo {
    /// The lands the combo's items stand for, in its order.
    lands: Vec<u16>,
}

impl Screen for NationalTeamInfo {
    fn ids(&self) -> &[u16] {
        &[NATIONAL_INFO]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        // OnInit (`FUN_004cb1c8`): the results table copied out (it
        // stands zero), the texts 4188..4196 fetched and the combo
        // filled with the lands 1 to 115 — chosen to the coach's
        // nation when it is one of them, and its rows and flag shown;
        // else nothing is chosen and land 0's empty rows stand.
        let (game, dialog) = (cx.game, cx.dialog);
        self.lands = (1..0x74u16).collect();
        let nation = cx
            .state
            .season
            .as_ref()
            .and_then(|s| s.manager())
            .and_then(|m| m.national_team);
        let chosen = nation.and_then(|n| self.lands.iter().position(|&l| l == n));
        let items: Vec<String> = self.lands.iter().map(|&l| game.world.nation(l).to_owned()).collect();
        if let Some(c) = control(dialog, INFO_LAND)
            && let Some(combo) = view.combo_mut(c)
        {
            combo.items = items;
            combo.selected = chosen;
            combo.inks.clear();
            combo.open = false;
            combo.scroll = 0;
        }
        self.show(cx, view, chosen.map_or(0, |item| self.lands[item]));
        // Everything here answers a click; only the header's statics
        // stand still.
        mark_inert(view, cx.dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            // The Land combo (`OnSelChange_IDC_INFO_NAT_COMBO_LAND`):
            // its item's land refills rows and flag at once; MFC fires
            // only on a change.
            Target::Row { control: c, .. } if dialog.controls[c].id == Some(INFO_LAND) => {
                if let Some(land) = view
                    .combo(c)
                    .and_then(|combo| combo.selected)
                    .and_then(|item| self.lands.get(item).copied())
                {
                    self.show(cx, view, land);
                }
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}

impl NationalTeamInfo {
    /// The rows of `land` and its big flag: `gross/<name>.bmp` by the
    /// table at 0x8a4890, falling back to the 76th name, the catch-all
    /// "Wel", below land 1 and above 116; a land whose big flag the
    /// archive does not hold shows no picture.
    fn show(&self, cx: &Cx, view: &mut View, land: u16) {
        let (game, dialog) = (cx.game, cx.dialog);
        if let Some(c) = control(dialog, INFO_LIST)
            && let Some(list) = view.list_mut(c)
        {
            list.rows = cx
                .state
                .records
                .as_ref()
                .map_or_else(Vec::new, |records| national::successes(records, land as i16))
                .iter()
                .map(|row| {
                    [
                        format!("{} {}", game.text(row.contest), row.year),
                        game.text(row.result).to_owned(),
                        row.coach.clone(),
                    ]
                    .join("\t")
                })
                .collect();
            list.inks.clear();
            list.row_icons.clear();
            list.keys.clear();
            list.selected = None;
            list.scroll = 0;
        }
        let flag = (1..=0x74).contains(&land).then_some(land).unwrap_or(0x4c);
        let name = game.data.nation_flags.get(usize::from(flag)).cloned().unwrap_or_default();
        if let Some(c) = control(dialog, INFO_FLAG) {
            view.set_picture(
                c,
                Some(Picture {
                    source: PictureSource::Archive(format!("gross/{name}.bmp")),
                    part: None,
                    black_clear: false,
                }),
            );
        }
    }
}

/// The arranger's three slots: the opponent's name and flag, the date and
/// the spinner to pick between the candidates, and the score once the day
/// went by (dialog 354).
const FRIENDLY_NAMES: [u16; national::FRIENDLY_SLOTS] = [4524, 4525, 4526];
const FRIENDLY_FLAGS: [u16; national::FRIENDLY_SLOTS] = [4521, 4522, 4523];
const FRIENDLY_DATES: [u16; national::FRIENDLY_SLOTS] = [4527, 4528, 4529];
const FRIENDLY_SPINS: [u16; national::FRIENDLY_SLOTS] = [4530, 4531, 4532];
const FRIENDLY_RESULTS: [u16; national::FRIENDLY_SLOTS] = [4533, 4534, 4535];
/// The list of the games still to come, date, opponent and kind.
const FRIENDLY_LIST: u16 = 4537;
/// What the list's last column says of a window's match and of the
/// friendly the coach arranged (texts 3604 and 3605).
const WINDOW_MATCH: u16 = 3604;
const ARRANGED_FRIENDLY: u16 = 3605;
/// The national team behind the number 200 in a land, what the list names
/// for a window's opponent.
const NATIONAL_OPPONENT: i16 = 200;

/// The coach arranging his team's friendlies (`CNationalFreundschaftsspiel`,
/// dialog 354): three dates, the first three friendly days of the
/// international row, each with three candidates from the coach's record
/// to spin between — the free choice at position 3 the dialog starts on
/// while the day is more than four days off.  What stands booked shows its
/// name and flag; a played day shows its score and answers no spinner any
/// more.  The list below names the window's matches from the save's date
/// entry and, below each friendly day still open, the candidate the
/// spinner holds.  OK writes the three booked entries into the record
/// (`FUN_0051a2c4`); no row answers a click.
#[derive(Default)]
pub struct NationalFriendlies {
    nation: u16,
    dates: [i16; national::FRIENDLY_SLOTS],
    upcoming: Vec<(i16, u32)>,
    entry: Option<usize>,
    booked: [national::FriendlyPick; national::FRIENDLY_SLOTS],
    candidates: [[national::FriendlyPick; national::FRIENDLY_SLOTS]; national::FRIENDLY_SLOTS],
    /// Per slot: the spinner's position, or −1 once the slot is frozen.
    state: [i8; national::FRIENDLY_SLOTS],
}

impl Screen for NationalFriendlies {
    fn ids(&self) -> &[u16] {
        &[NATIONAL_FRIENDLY]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        self.nation = cx
            .state
            .season
            .as_ref()
            .and_then(|s| s.manager())
            .and_then(|m| m.national_team)
            .unwrap_or(0);
        self.state = [-1; national::FRIENDLY_SLOTS];
        let nation = self.nation as i16;
        let year = cx.state.season.as_ref().map_or(0, |s| s.year);
        let records = cx.state.records.as_ref();
        let today = records.map_or(0, |r| usize::from(r.globals.day()));
        self.dates = national::friendly_dates(&game.calendar).map(|d| d as i16);
        self.upcoming = records.map_or_else(Vec::new, |_| national::upcoming_days(&game.calendar, today));
        self.entry = records.and_then(|r| national::date_entry(r, nation));
        if self.entry.is_none() {
            // Without a date entry for his nation the dialog keeps only
            // the friendly days of its copy of the window rows.
            self.upcoming.retain(|&(_, flag)| flag & 0x7f00_0000 == 0x0400_0000);
        }
        self.candidates =
            records.map_or(<[[national::FriendlyPick; national::FRIENDLY_SLOTS]; national::FRIENDLY_SLOTS]>::default(), |r| national::friendly_candidates(r, nation));
        self.booked = records.map_or(<[national::FriendlyPick; national::FRIENDLY_SLOTS]>::default(), |r| national::friendly_booked(r, nation));
        // `OnInit`: the results start blank; each slot's spinner gets its
        // range 0 to 3, the free choice included.
        for k in 0..national::FRIENDLY_SLOTS {
            let day = usize::try_from(self.dates[k]).unwrap_or(0);
            set(view, dialog, FRIENDLY_DATES[k], hub::date_text(game, a3_core::calendar::date(year, day)));
            set(view, dialog, FRIENDLY_RESULTS[k], String::new());
            if let Some(c) = control(dialog, FRIENDLY_SPINS[k])
                && let Some(v) = view.value_mut(c)
            {
                v.range = (0, 3);
            }
            let mut state: i8 = -1;
            if self.booked[k].land() == 0 {
                set(view, dialog, FRIENDLY_NAMES[k], String::new());
                if let Some(c) = control(dialog, FRIENDLY_FLAGS[k]) {
                    view.set_picture(c, None);
                    view.controls[c].hidden = true;
                }
                if day > today + 4 {
                    state = 3;
                    if let Some(c) = control(dialog, FRIENDLY_SPINS[k]) {
                        view.controls[c].hidden = false;
                        if let Some(v) = view.value_mut(c) {
                            v.set(3);
                        }
                    }
                } else if let Some(c) = control(dialog, FRIENDLY_SPINS[k]) {
                    view.controls[c].hidden = true;
                }
            } else {
                self.show_slot(cx, view, k);
                state = 0;
                for p in 0..national::FRIENDLY_SLOTS {
                    if self.booked[k].land() == self.candidates[k][p].land() {
                        state = i8::try_from(p).unwrap_or(3);
                        if let Some(c) = control(dialog, FRIENDLY_SPINS[k])
                            && let Some(v) = view.value_mut(c)
                        {
                            v.set(p as i32);
                        }
                    }
                }
                match records.and_then(|r| national::friendly_score(r, nation, k)) {
                    Some((a, b)) => {
                        set(view, dialog, FRIENDLY_RESULTS[k], format!("{a}:{b}"));
                        if let Some(c) = control(dialog, FRIENDLY_SPINS[k]) {
                            view.controls[c].hidden = true;
                        }
                        state = -1;
                    }
                    None => {
                        if day <= today + 4
                            && let Some(c) = control(dialog, FRIENDLY_SPINS[k])
                        {
                            view.controls[c].hidden = true;
                            state = -1;
                        }
                    }
                }
            }
            self.state[k] = state;
        }
        self.fill_list(cx, view);
        mark_inert(view, dialog, |target| match target {
            Target::Button(c) => crate::controls::button::is_ok(&dialog.controls[c]),
            Target::Spin { control: c, .. } => {
                dialog.controls[c].id.is_some_and(|id| FRIENDLY_SPINS.contains(&id))
            }
            _ => false,
        });
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            // OK (`FUN_0051a2c4`): the three entries the dialog holds go
            // into the record — even a slot spun to the free choice,
            // whose display went blank while what stood booked stayed.
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                if let Some(records) = cx.state.records.as_mut() {
                    national::set_friendly_booked(records, self.nation as i16, &self.booked);
                }
                Outcome::Close(Reply::Ok)
            }
            // A spinner (`FUN_0051a9e8`), its value moved already: a
            // frozen slot answers nothing; below the free choice the
            // candidate under the position stands booked and shows; at
            // it the slot clears its display.  Either way the list comes
            // up anew.
            Target::Spin { control: c, .. } => {
                let Some(k) = dialog.controls[c]
                    .id
                    .and_then(|id| FRIENDLY_SPINS.iter().position(|&s| s == id))
                else {
                    return Outcome::Stay;
                };
                if self.state[k] < 0 {
                    return Outcome::Stay;
                }
                let pos = view.value(c).map_or(3, |v| v.value);
                self.state[k] = i8::try_from(pos).unwrap_or(3);
                if (0..national::FRIENDLY_SLOTS as i32).contains(&pos) {
                    self.booked[k] = self.candidates[k][pos as usize];
                    self.show_slot(cx, view, k);
                } else {
                    self.clear_slot(cx, view, k);
                }
                self.fill_list(cx, view);
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}

impl NationalFriendlies {
    /// The booked entry of slot `k` on show (`FUN_0051a40b`): its name
    /// behind land and number, its flag picture, the bitmap visible.
    fn show_slot(&self, cx: &Cx, view: &mut View, k: usize) {
        let pick = self.booked[k];
        let name = cx.state.records.as_ref().map_or_else(String::new, |r| {
            cx.game.club_name(Some(r), pick.land(), pick.team())
        });
        set(view, cx.dialog, FRIENDLY_NAMES[k], name);
        if let Some(c) = control(cx.dialog, FRIENDLY_FLAGS[k]) {
            view.set_picture(c, Some(flag_picture(pick.land())));
            view.controls[c].hidden = false;
        }
    }

    /// The slot's display cleared (`FUN_0051a2f9`): no name, the image
    /// emptied, the bitmap hidden — the booked entry itself stays, as in
    /// the original, and OK still writes it.
    fn clear_slot(&self, cx: &Cx, view: &mut View, k: usize) {
        set(view, cx.dialog, FRIENDLY_NAMES[k], String::new());
        if let Some(c) = control(cx.dialog, FRIENDLY_FLAGS[k]) {
            view.set_picture(c, None);
            view.controls[c].hidden = true;
        }
    }

    /// The list of games still to come (`FUN_0051a508`) over the window
    /// days: a match row for each of the entry's pairings holding his
    /// nation, and under a friendly day a row only while the slot's
    /// spinner holds a candidate — naming that candidate, not what was
    /// booked before.  Without a date entry for his nation the list ends
    /// at the first window's match.
    fn fill_list(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(c) = control(dialog, FRIENDLY_LIST) else { return };
        let year = cx.state.season.as_ref().map_or(0, |s| s.year);
        let (records, parity) =
            cx.state.records.as_ref().map_or((None, 0), |r| (Some(r), usize::from(r.globals.start_year()) & 1));
        let nation = self.nation as i16;
        let mut rows: Vec<String> = Vec::new();
        for &(day, flag) in &self.upcoming {
            let date = hub::date_text(game, a3_core::calendar::date(year, usize::try_from(day).unwrap_or(0)));
            if flag & 0x7f00_0000 == 0x0800_0000 {
                let Some(entry) = self.entry else { break };
                let column = (flag & 0x3f) as usize + 5 * parity;
                let opponent = records
                    .map_or_else(Vec::new, |r| national::international_pairs(r, entry, column))
                    .into_iter()
                    .find(|&(a, b)| a == nation || b == nation)
                    .map(|(a, b)| if a == nation { b } else { a });
                if let Some(opponent) = opponent {
                    rows.push(
                        [
                            date,
                            game.club_name(records, opponent as u8, NATIONAL_OPPONENT),
                            game.text(WINDOW_MATCH).to_owned(),
                        ]
                        .join("\t"),
                    );
                }
            } else {
                let slot = (flag & 0x3f) as usize;
                let state = self.state.get(slot).copied().unwrap_or(-1);
                if (0..national::FRIENDLY_SLOTS as i8).contains(&state) {
                    let pick = self.candidates[slot][state as usize];
                    rows.push(
                        [
                            date,
                            game.club_name(records, pick.land(), pick.team()),
                            game.text(ARRANGED_FRIENDLY).to_owned(),
                        ]
                        .join("\t"),
                    );
                }
            }
        }
        if let Some(list) = view.list_mut(c) {
            // Three columns, all left, starting at 0, 100 and 210, no
            // widths (`FUN_007a941f` over `DAT_008855c0` and 0x8855cc).
            list.tabs = vec![0.0, 100.0, 210.0];
            list.aligns = vec![Some((Align::Left, 0.0)); 3];
            list.rows = rows;
            list.inks.clear();
            list.row_icons.clear();
            list.keys.clear();
            list.selected = None;
            list.scroll = 0;
        }
    }
}

/// A flag's picture (`FUN_00696030`).
fn flag_picture(nation: u8) -> Picture {
    let n = nation.clamp(1, 0x74);
    Picture {
        source: PictureSource::Archive(format!("klein/Flag{n:02}.bmp")),
        part: None,
        black_clear: false,
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
