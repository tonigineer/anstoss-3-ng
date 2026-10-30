//! A player-manager's own career (dialog 403, `CDiSpielerkarriere`;
//! TRAINING-INDIV.md §4), behind the individual training's
//! "Spieler-karriere": how hard he trains, how hard he goes in a match and
//! how rough he plays, on three sliders; a team-mate to foul and one to
//! make friends with. OK keeps them (`FUN_00568f5c`), "Abbruch" nothing.

use std::collections::HashMap;

use a3_files::rch::Dialog;

use super::super::duties::short_name;
use super::super::{Args, Cx, Outcome, Reply, Screen, caption, control, mark_inert, ok_or_cancel, set};
use crate::pointer::Target;
use crate::view::View;

pub(crate) const PLAYER_CAREER: u16 = 403;
const CAPTION_ID: u16 = 9929;
/// The sliders — training, the match, hardness — and how far each goes
/// (`FUN_00568a27`).
const SLIDERS: [u16; 3] = [6133, 6134, 6135];
const MOST: [i32; 3] = [4, 4, 2];
/// The team-mate to foul, the one to make friends with.
const RIVAL: u16 = 6137;
const FRIEND: u16 = 6138;
/// "Bitte auswählen", both combo boxes' first row.
const CHOOSE: u16 = 4945;
/// A player-manager plays as 17000 and his slot: the lists end before the
/// managers' own players.
const MANAGER_PLAYERS: u16 = 17001;
const MANAGER_PLAYER: u16 = 17000;

/// The caption bar takes the dialog's title.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&PLAYER_CAREER) else { return };
    dialog.controls.insert(0, caption(dialog.frame, CAPTION_ID, String::new()));
}

/// The settings as the dialog has them, and the team-mates its combo
/// boxes list after their first row, by id.
#[derive(Default)]
pub struct PlayerCareer {
    play: [u8; 3],
    rival: i16,
    friend: i16,
    rows: [Vec<u16>; 2],
}

impl Screen for PlayerCareer {
    fn ids(&self) -> &[u16] {
        &[PLAYER_CAREER]
    }

    /// `FUN_00568a27`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        mark_inert(view, dialog, |_| true);
        let Some(season) = cx.state.season.as_ref() else { return };
        let Some(manager) = season.manager() else { return };
        let club = manager.club.and_then(|(slot, c)| game.world.country(slot)?.clubs.get(c));
        // "%s %s %s (%s)": the title, his names, his club.
        let title = format!(
            "{} {} {} ({})",
            dialog.title,
            manager.first_name,
            manager.last_name,
            club.map_or("", |c| c.name.as_str())
        );
        set(view, dialog, CAPTION_ID, title);
        let (Some(records), Some(key)) = (cx.state.records.as_ref(), manager.club) else { return };
        let Some(slot) = records.human_slot(season.current) else { return };
        let record = &records.managers[slot];
        (self.play, self.rival, self.friend) = (record.own_play(), record.rival(), record.friend());
        for ((id, most), value) in SLIDERS.into_iter().zip(MOST).zip(self.play) {
            if let Some(slider) = control(dialog, id).and_then(|c| view.value_mut(c)) {
                slider.range = (0, most);
                slider.value = i32::from(value);
            }
        }
        // The squad in its order, but for the managers' own players; a
        // friend only among those without a best friend or whose best
        // friend he is himself (`FUN_0070162e`).
        let own = MANAGER_PLAYER + slot as u16 + 1;
        let squad: Vec<(String, u16)> = season.clubs.get(&key).map_or_else(Vec::new, |club| {
            club.players
                .iter()
                .zip(&club.status)
                .filter(|(_, s)| s.id < MANAGER_PLAYERS)
                .map(|(p, s)| (short_name(cx, p, s.id), s.id))
                .collect()
        });
        let free = |id: u16| {
            records.player(id).is_none_or(|r| (r.best_friend() as i16) < 1 || r.best_friend() == own)
        };
        let lists = [
            squad.iter().collect::<Vec<_>>(),
            squad.iter().filter(|(_, id)| free(*id)).collect(),
        ];
        let chosen = [self.rival, self.friend];
        for (i, (id, list)) in [RIVAL, FRIEND].into_iter().zip(lists).enumerate() {
            self.rows[i] = list.iter().map(|(_, id)| *id).collect();
            let mut items = vec![game.text(CHOOSE).to_owned()];
            items.extend(list.iter().map(|(name, _)| name.clone()));
            // None chosen shows the first row; a rival no longer listed
            // leaves the box empty, a friend its first row.
            let found = self.rows[i].iter().position(|&p| i32::from(p) == i32::from(chosen[i]));
            let row = match (i, chosen[i] > 0) {
                (_, false) => Some(0),
                (0, true) => found.map(|r| r + 1),
                _ => Some(found.map_or(0, |r| r + 1)),
            };
            if let Some(combo) = control(dialog, id).and_then(|c| view.combo_mut(c)) {
                combo.items = items;
                combo.selected = row;
            }
        }
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        match target {
            Target::SliderStep { control: c, .. } | Target::SliderAt { control: c, .. } => {
                if let Some(i) = SLIDERS.iter().position(|&s| s == id(c)) {
                    self.play[i] = view.value(c).map_or(0, |v| v.value) as u8;
                }
                return Outcome::Stay;
            }
            // A row chosen takes its player's id; the first row's is −1
            // (`OnSelChange_IDC_UMTRETEN`, `…_FREUNDSCHAFT`).
            Target::Row { control: c, .. } => {
                let Some(i) = [RIVAL, FRIEND].iter().position(|&b| b == id(c)) else {
                    return Outcome::Stay;
                };
                let row = view.combo(c).and_then(|combo| combo.selected).unwrap_or(0);
                let player = row.checked_sub(1).and_then(|r| self.rows[i].get(r));
                let picked = player.map_or(-1, |&p| p as i16);
                if i == 0 { self.rival = picked } else { self.friend = picked }
                return Outcome::Stay;
            }
            _ => {}
        }
        let (ok, cancel) = ok_or_cancel(dialog, target);
        if cancel {
            return Outcome::Close(Reply::Cancel);
        }
        if !ok {
            return Outcome::Stay;
        }
        // The sliders and the rival as they stand; a friend only when one
        // is chosen.
        let current = cx.state.season.as_ref().map(|s| s.current);
        if let Some(records) = cx.state.records.as_mut()
            && let Some(slot) = current.and_then(|n| records.human_slot(n))
        {
            let record = &mut records.managers[slot];
            record.set_own_play(self.play);
            record.set_rival(self.rival);
            if self.friend > 0 {
                record.set_friend(self.friend);
            }
        }
        Outcome::Close(Reply::Ok)
    }
}
