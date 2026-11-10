//! The betting office's report of the settled matchday (ERFOLGE.md §D,
//! dialog 409 `CWettbueroAuswertung`, `FUN_005cf263`): the Toto rows, the
//! highest wins — every ticked match in three lines, the first also
//! carrying stake, win and the winner's result in the Ergebnis column —,
//! the matchday's goals, the first scorer and the trainer's sack; a
//! blank after every section with lines, the dashes, and the Bilanz over
//! all. What a row paid less than it cost stands in red in the original;
//! the replica's list knows one ink and leaves them black.

use std::collections::HashMap;

use a3_core::betting::{self, PageReport, TeamPair};
use a3_files::rch::{Column, Dialog, Kind};

use crate::pointer::Target;
use crate::screens::statistics::spelled;
use crate::screens::{Args, Cx, Outcome, Reply, Screen, control, mark_inert};
use crate::view::View;

pub(crate) const REPORT_DIALOG: u16 = 409;
/// The list of rows (`CWettbueroErgebnis`'s listbox, filled by
/// `FUN_005cf263`).
const LIST: u16 = 5248;
/// The section headers and their line texts (`0x1171` to `0x117a`,
/// 0x1186).
const TOTO: u16 = 0x1171;
const HÖCHST: u16 = 0x1173;
const SEVERAL: u16 = 0x1174;
const GOALS: u16 = 0x1175;
const SCORER: u16 = 0x1177;
const TRAINER_HEAD: u16 = 0x1178;
const SACKED: u16 = 0x1179;
const STILL: u16 = 0x117a;
const BILANZ: u16 = 0x1186;
/// The blank after a section with lines, and the dashes over the Bilanz.
const BLANK: &str = " ";
const SEPARATOR: &str = "----------\t----------\t----------\t----------\t----------";
/// The columns under the dialog's statics "Tip", "Land", "Ergebnis",
/// "Einsatz" and "Gewinn".
const TABS: [f32; 5] = [3.0, 133.0, 207.0, 374.0, 441.0];

/// The list as the report wants it: five columns, twelve pixels a row.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&REPORT_DIALOG) else { return };
    if let Some(c) = dialog.controls.iter_mut().find(|c| c.id == Some(LIST)) {
        c.kind = Kind::ListBox {
            row_height: 12,
            style: 0,
            columns: TABS.iter().map(|&at| Column { at, align: None }).collect(),
            icons: Vec::new(),
        };
    }
}

#[derive(Default)]
pub struct Report;

/// The human's report slot (`this + 0xd38`'s 0x1a).
fn human(cx: &Cx) -> Option<usize> {
    cx.state.records.as_ref()?.human_slot(cx.state.season.as_ref()?.current)
}

/// A club's name as the report's raw pair has it.
fn club(cx: &Cx, pair: TeamPair) -> String {
    let Some(records) = cx.state.records.as_ref() else { return String::new() };
    records.team(u16::from(pair.country), pair.number).map(|t| t.name()).unwrap_or_default()
}

/// The page's country, its matches' first non-zero one.
fn country(cx: &Cx, report: &PageReport) -> String {
    cx.game.world.nation(u16::from(report.nation)).to_owned()
}

/// A stake or a win: plain figures (`FUN_004daf8f`).
fn figure(value: i32) -> String {
    value.to_string()
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

/// The match's goals, right then left in their places (`0x88cf6c`).
fn goals(home: u8, away: u8) -> String {
    format!("     {:>2} : {:<2}", home, away)
}

impl Report {
    /// Every row of the list and the Bilanz at its end.
    fn lines(&self, cx: &Cx, human: usize) -> Vec<String> {
        let Some(records) = cx.state.records.as_ref() else { return Vec::new() };
        let game = cx.game;
        let reports: Vec<Option<PageReport>> =
            (0..betting::PAGES).map(|page| betting::page_report(records, human, page)).collect();
        let filled = |has: fn(&PageReport) -> bool| {
            reports.iter().flatten().filter(move |r| has(r)).collect::<Vec<_>>()
        };
        let (mut rows, mut total) = (Vec::new(), 0i32);

        // The Toto (`0x1171`): how many right, the country, stake and win.
        if !filled(|r| r.full.is_some()).is_empty() {
            rows.push(game.text(TOTO).to_owned());
            for report in filled(|r| r.full.is_some()) {
                let (got, stake, payout) = report.full.unwrap();
                total += payout - stake;
                rows.push(format!(
                    "{got} Richtige\t{}\t-\t{}\t{}",
                    country(cx, report),
                    figure(stake),
                    figure(payout)
                ));
            }
            rows.push(BLANK.to_owned());
        }

        // The highest win (`0x1173`): every ticked match in three lines,
        // the first also carrying stake and win and the unique winner's
        // result — "mehrere" where several tie — in the Ergebnis column.
        if !filled(|r| r.high.is_some()).is_empty() {
            rows.push(game.text(HÖCHST).to_owned());
            for report in filled(|r| r.high.is_some()) {
                let high = report.high.clone().unwrap();
                total += high.payout - high.stake;
                let land = country(cx, report);
                for (k, (home, away, hg, ag)) in high.rows.iter().enumerate() {
                    if k > 0 {
                        rows.push(format!("{} -", club(cx, *home)));
                        rows.push(club(cx, *away));
                        rows.push(goals(*hg, *ag));
                        continue;
                    }
                    let tip = format!("{} -", club(cx, *home));
                    match high.winner {
                        None => rows.push(format!(
                            "{tip}\t{land}\t{}\t{}\t{}",
                            game.text(SEVERAL),
                            figure(high.stake),
                            figure(high.payout)
                        )),
                        Some((wh, wa, wg, wag)) => {
                            rows.push(format!(
                                "{tip}\t{land}\t{} -\t{}\t{}",
                                club(cx, wh),
                                figure(high.stake),
                                figure(high.payout)
                            ));
                            rows.push(format!("{}\t\t{}", club(cx, *away), club(cx, wa)));
                            rows.push(format!("{}\t\t{}", goals(*hg, *ag), goals(wg, wag)));
                        }
                    }
                    if high.winner.is_none() {
                        rows.push(club(cx, *away));
                        rows.push(goals(*hg, *ag));
                    }
                }
            }
            rows.push(BLANK.to_owned());
        }

        // The matchday's goals (`0x1175`): the tip, the country, what the
        // matchday brought, stake and win.
        if !filled(|r| r.goals.is_some()).is_empty() {
            rows.push(game.text(GOALS).to_owned());
            for report in filled(|r| r.goals.is_some()) {
                let (tip, stake, payout) = report.goals.unwrap();
                total += payout - stake;
                rows.push(format!(
                    "{tip} getippt\t{}\t{}\t{}\t{}",
                    country(cx, report),
                    report.shown,
                    figure(stake),
                    figure(payout)
                ));
            }
            rows.push(BLANK.to_owned());
        }

        // The first scorer (`0x1177`): the tipped pair of names, the
        // country, the league's first, stake and win; the first's last
        // name stands again under the result (`0x88cfd8`).
        if !filled(|r| r.scorer.is_some()).is_empty() {
            rows.push(game.text(SCORER).to_owned());
            for (page, report) in reports.iter().enumerate().filter_map(|(page, report)| Some((page, report.as_ref()?))).filter(|(_, r)| r.scorer.is_some()) {
                let (row, stake, payout) = report.scorer.unwrap();
                total += payout - stake;
                let tipped = usize::try_from(row).ok()
                    .and_then(|k| betting::matchday_offer(records, page).and_then(|o| o.scorers.get(k).copied()))
                    .map(|s| spelled(records, s.names.0, s.names.1))
                    .unwrap_or_default();
                let first = if report.top < 1 {
                    "-".to_owned()
                } else {
                    records.person_name(u16::try_from(report.top).unwrap_or(0))
                };
                rows.push(format!(
                    "{tipped}\t{}\t{first}\t{}\t{}",
                    country(cx, report),
                    figure(stake),
                    figure(payout)
                ));
                if report.top >= 1 {
                    rows.push(format!("\t\t{}", first.rsplit(' ').next().unwrap_or_default()));
                }
            }
            rows.push(BLANK.to_owned());
        }

        // The trainer's sack (`0x1178`): his name, the country, whether
        // he is gone yet, stake and win.
        if !filled(|r| r.trainer.is_some()).is_empty() {
            rows.push(game.text(TRAINER_HEAD).to_owned());
            for report in filled(|r| r.trainer.is_some()) {
                let (id, stake, payout) = report.trainer.unwrap();
                total += payout - stake;
                let status = if payout < 1 { game.text(STILL) } else { game.text(SACKED) };
                rows.push(format!(
                    "{}\t{}\t{}\t{}\t{}",
                    trainer_name(cx, id),
                    country(cx, report),
                    status,
                    figure(stake),
                    figure(payout)
                ));
            }
            rows.push(BLANK.to_owned());
        }

        // The dashes and the Bilanz over all (`0x88d094`, 0x1186).
        rows.push(SEPARATOR.to_owned());
        rows.push(game.text(BILANZ).replacen("%s", &figure(total), 1));
        rows
    }
}

impl Screen for Report {
    fn ids(&self) -> &[u16] {
        &[REPORT_DIALOG]
    }

    /// `0x5cf263`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        if let (Some(h), Some(c)) = (human(cx), control(dialog, LIST)) {
            let rows = self.lines(cx, h);
            if let Some(list) = view.list_mut(c) {
                list.rows = rows;
                list.scroll = 0;
            }
        }
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        if let Target::Button(c) = target {
            if crate::controls::button::is_ok(&dialog.controls[c]) {
                return Outcome::Close(Reply::Ok);
            }
        }
        Outcome::Stay
    }
}
