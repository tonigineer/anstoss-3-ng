//! The Spielbericht after the match (dialog 332, `CSpielberichtStandard`,
//! `IDD_SPIELBERICHT_STANDARD`): event 0xce opens it behind the matchday
//! for the human clubs that played (`FUN_00551818`), the first one's report
//! shown (`FUN_0055214c`, `FUN_005524a6`): both crests and names, the
//! competition and the places, the score with the half's, the first leg,
//! the goals, the other events, the crowd, the weather, the gate and the
//! money, the comparison of the sides, the referee with his verdict, the
//! best man, the playing time, the rating and the TV's share. The crests
//! below stand for the other humans' clubs and show their reports
//! (`OnClicked_IDC_SPIELB_STD_WAPPEN1` … 3, `FUN_005543d6`); the line-ups
//! behind `IDC_SPB_STD_AUFSTELLUNGEN` (dialog 346) are not built. The
//! reports are [`a3_core::report`]'s.

use a3_core::cycle::dialogs::MATCH_REPORT;
use a3_core::opponent::League;
use a3_core::records::Records;
use a3_core::statistics::{self, Report};

use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::controls::bitmap::Picture;
use crate::game::Game;
use crate::pointer::Target;
use crate::view::View;

/// The controls (`FUN_00551cee`).
const CRESTS: [u16; 2] = [4305, 4306];
const COMPETITION: u16 = 4307;
const MATCHDAY: u16 = 4308;
const NAMES: [u16; 2] = [4309, 4311];
const PLACES: [u16; 2] = [4312, 4314];
const SCORE: u16 = 4313;
const FIRST_LEG: u16 = 4315;
const ATTENDANCE: u16 = 4316;
const WEATHER: u16 = 4317;
const INCOME: u16 = 4318;
/// The TV money, the bonuses received and paid (+0x140, +0x144, +0x148).
const MONEY: [u16; 3] = [4319, 4320, 4321];
/// Each side's chances, yellows, second yellows and corners.
const COMPARISON: [[u16; 4]; 2] = [[4322, 4323, 4324, 4325], [4326, 4327, 4328, 4329]];
const REFEREE: u16 = 4330;
const BEST: u16 = 4331;
const MINUTES: u16 = 4332;
const RATING: u16 = 4333;
const TV_SHARE: u16 = 4334;
const GOALS: u16 = 4335;
const EVENTS: u16 = 4337;
const OTHER_CRESTS: [u16; 3] = [4339, 4340, 4341];
const OTHER_NAMES: [u16; 3] = [4342, 4343, 4344];
const LINE_UPS: u16 = 4345;
const OK: u16 = 1;

/// The goals' columns (`FUN_007a941f` with 0x888374 and 0x888388): the
/// score's two sides about the ":", the scorer, the minute; the 0x8000
/// right in the width beside.
const GOAL_COLUMNS: [(u16, u16); 5] = [(0x8000, 17), (19, 0), (23, 0), (40, 0), (0x80ce, 30)];
/// The texts: no goal, no event; an own goal's and a penalty's rows; a
/// penalty missed; an injury's diagnosis; the score's ending after extra
/// time and penalties (`FUN_00554583`); a matchday; a sold-out crowd; the
/// playing time; a place ("%1%2 Platz", `FUN_00730a25`).
const NO_GOALS: u16 = 3186;
const NO_EVENTS: u16 = 3187;
const OWN_GOAL: u16 = 2607;
const PENALTY: u16 = 2608;
const MISSED: u16 = 2981;
const DIAGNOSIS: u16 = 4990;
const EXTRA_TIME: u16 = 0x761;
const PENALTIES: u16 = 0x762;
const MATCHDAY_TEXT: u16 = 2880;
const SOLD_OUT: u16 = 2600;
const MINUTES_TEXT: u16 = 2601;
const PLACE_TEXT: u16 = 1058;
/// The other events' formats by kind (0x8a835c), a weather's (0x8acfb4,
/// the first two alike), a rating's (0x8a8298), a verdict's (0x8a82d8).
const EVENT_TEXTS: [u16; 6] = [2581, 2602, 2603, 2604, 2605, 2606];
const WEATHERS: u16 = 2537;
const RATINGS: [u16; 15] =
    [2585, 2586, 2587, 2588, 2589, 2590, 2599, 2591, 2592, 2593, 2594, 2595, 2596, 2597, 2598];
const VERDICTS: u16 = 2551;
/// The cups' names by the kind's high bits; the supercup is Germany's
/// Ligapokal (country 14).
const CUP: u16 = 2477;
const SUPERCUP: u16 = 2478;
const LEAGUE_CUP: u16 = 3611;
const GERMANY: u8 = 14;
const RELEGATION: u16 = 2479;
const INTERTOTO: u16 = 2485;
const FRIENDLY: u16 = 3539;
const CLUB_WORLD_CUP: u16 = 2483;
const EUROPEAN_SUPERCUP: u16 = 2484;
/// A manager no slot holds (`FUN_006a537d`).
const NO_MANAGER: u16 = 1996;
/// What a blank figure shows.
const NONE: &str = "-";
/// A human's man stands red among the events (`FUN_007a90c1` with 0xff).
const OWN_INK: [u8; 3] = [255, 0, 0];
const NATIONAL: i16 = 200;

#[derive(Default)]
pub struct MatchReport {
    /// The human clubs whose reports the dialog shows, by country and
    /// number, and the one shown (+0x3328, +0x332a, +0x3348).
    clubs: Vec<(u8, i16)>,
    shown: usize,
    /// The formats the dialog keeps of its own statics before it fills
    /// them (`FUN_0055214c`: +0x333c, +0x3340, +0x3344).
    formats: [String; 3],
}

/// Each part in for the next "%s" or "%d", in their order.
fn fill(format: &str, parts: &[&str]) -> String {
    let mut out = String::new();
    let mut rest = format;
    for part in parts {
        let Some(at) = rest.find("%s").into_iter().chain(rest.find("%d")).min() else {
            break;
        };
        out.push_str(&rest[..at]);
        out.push_str(part);
        rest = &rest[at + 2..];
    }
    out + rest
}

/// A club's crest as the dialog's bitmaps show it (`FUN_00696161`).
fn crest(game: &Game, records: &Records, (country, number): (u8, i16)) -> Option<Picture> {
    let slot = game.world.slot_of(country.into())?;
    let file = game.rules.crest(&game.world, Some(records), (slot, usize::try_from(number).ok()?))?;
    Some(Picture::file(file))
}

fn report_of(records: &Records, (country, number): (u8, i16)) -> Option<Report> {
    let team = records.team(country.into(), u16::try_from(number).ok()?)?;
    statistics::match_report(records, team)
}

impl MatchReport {
    /// The report shown and the crests of the others (`FUN_005543d6`).
    fn show(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(records) = cx.state.records.as_ref() else { return };
        let others: Vec<(u8, i16)> =
            self.clubs.iter().enumerate().filter(|&(k, _)| k != self.shown).map(|(_, &c)| c).collect();
        for k in 0..OTHER_CRESTS.len() {
            let club = others.get(k).copied();
            for id in [OTHER_CRESTS[k], OTHER_NAMES[k]] {
                if let Some(c) = control(dialog, id) {
                    view.controls[c].hidden = club.is_none();
                }
            }
            if let Some(club) = club {
                if let Some(c) = control(dialog, OTHER_CRESTS[k]) {
                    view.set_picture(c, crest(game, records, club));
                    view.set_clickable(c, true);
                }
                set(view, dialog, OTHER_NAMES[k], game.club_name(Some(records), club.0, club.1));
            }
        }
        if let Some(report) = self.clubs.get(self.shown).and_then(|&club| report_of(records, club)) {
            self.fill(game, dialog, records, &report, view);
        }
    }

    /// One report on the dialog (`FUN_005524a6`).
    fn fill(&self, game: &Game, dialog: &a3_files::rch::Dialog, records: &Records, report: &Report, view: &mut View) {
        let text = |id: u16| game.text(id).replace("\\t", "\t");
        let name = |id: u16| records.known_as(id).unwrap_or_default();
        let minute = |m: u8| format!("{m}.");
        let money = |v: i32| super::playerinfo::money(game, i64::from(v));
        let blank = report.state() & 1 != 0;
        let sides = report.sides();
        let team = |(c, n): (u8, i16)| records.team(c.into(), u16::try_from(n).ok()?);
        for (k, &side) in sides.iter().enumerate() {
            if let Some(c) = control(dialog, CRESTS[k]) {
                view.set_picture(c, crest(game, records, side));
            }
        }
        // The goals, or "Fehlanzeige".
        if let Some(c) = control(dialog, GOALS) {
            let mut rows = Vec::new();
            let mut spans = Vec::new();
            let goals = report.scorers();
            let columns: &[(u16, u16)] = if goals.is_empty() { &[(0, 0)] } else { &GOAL_COLUMNS };
            super::youth_tables::set_columns(view, c, columns);
            if goals.is_empty() {
                rows.push(text(NO_GOALS));
                spans.push(false);
            }
            for goal in goals.iter().filter(|g| g.id != 0) {
                let [h, a] = goal.score.map(|g| g.to_string());
                let (row, span) = match goal.kind {
                    5 => (fill(&text(OWN_GOAL), &[&h, &a, &name(goal.id), &minute(goal.minute)]), false),
                    0 => (fill(&text(PENALTY), &[&h, &a, &name(goal.id), &minute(goal.minute)]), false),
                    ..100 => (fill("%d\t:\t%d\t%s\t(%s)", &[&h, &a, &name(goal.id), &minute(goal.minute)]), false),
                    100 => (fill(&text(MISSED), &[&name(goal.id), ""]), true),
                    _ => (fill("%d\t:\t%d\t%s", &[&h, &a, &name(goal.id)]), false),
                };
                rows.push(row);
                spans.push(span);
            }
            if let Some(list) = view.list_mut(c) {
                list.inks = vec![None; rows.len()];
                list.spans = spans;
                list.rows = rows;
                list.scroll = 0;
            }
        }
        // The other events, a human's man in red; "keine" without any.
        if let Some(c) = control(dialog, EVENTS) {
            let own = if sides.first().is_some_and(|&s| statistics::is_human(records, s.0, s.1)) { sides[0] } else { sides[1] };
            let ours = |id: i16| {
                team(own).is_some_and(|t| u16::try_from(id).is_ok_and(|id| t.players().contains(&id)))
            };
            let mut rows: Vec<(String, Option<[u8; 3]>)> = Vec::new();
            let events = report.events();
            if events.is_empty() {
                rows.push((text(NO_EVENTS), None));
            }
            for event in events {
                let format = EVENT_TEXTS.get(usize::from(event.kind)).map(|&id| text(id)).unwrap_or_default();
                let [first, second] = event.ids;
                let at = minute(event.minute);
                let mut red = false;
                let row = match event.kind {
                    1 if first != 0 && second != 0 => {
                        red = ours(first);
                        fill(&format, &[&name(first as u16), &name(second as u16), &at])
                    }
                    2 | 3 if first != 0 => {
                        red = ours(first);
                        fill(&format, &[&name(first as u16), &at])
                    }
                    // The injury, then its diagnosis on a row of its own.
                    4 if first != 0 => {
                        red = ours(first);
                        let ink = red.then_some(OWN_INK);
                        rows.push((fill(&format, &[&name(first as u16)]), ink));
                        let kind = records.player(first as u16).map_or(0, |p| p.injury_kind());
                        let injury = game.rules.words.injuries.get(usize::from(kind)).map_or_else(String::new, |&t| text(t));
                        fill(&text(DIAGNOSIS), &[&injury, &at])
                    }
                    5 => {
                        let slot = usize::try_from(first - 1).ok();
                        let slots = records.globals.slots();
                        let held = slot.filter(|&s| slots.get(s).is_some_and(|&n| n > 0));
                        let manager = match held.and_then(|s| records.managers.get(s)) {
                            Some(m) => format!("{} {}", m.first_name(), m.last_name()),
                            None => text(NO_MANAGER),
                        };
                        fill(&format, &[&manager, &at])
                    }
                    // The foreigners' rule broken (kind 6) the replica
                    // never books.
                    _ => String::new(),
                };
                rows.push((row, red.then_some(OWN_INK)));
            }
            super::youth_tables::set_columns(view, c, &[(0, 0)]);
            if let Some(list) = view.list_mut(c) {
                list.inks = rows.iter().map(|r| r.1).collect();
                list.spans = vec![false; rows.len()];
                list.rows = rows.into_iter().map(|r| r.0).collect();
                list.scroll = 0;
            }
        }
        // The crowd, "(A)" for a full ground.
        let (country, number) = report.venue();
        let capacity = a3_core::opponent::stadium(records, country, number).map_or(0, |s| s.capacity);
        let crowd = super::finances::count(report.attendance());
        let attendance = if blank {
            String::new()
        } else if report.attendance() < capacity {
            crowd
        } else {
            fill(&text(SOLD_OUT), &[&crowd])
        };
        set(view, dialog, ATTENDANCE, attendance);
        let weather = match report.weather() {
            0 | 1 => WEATHERS + 1,
            w => WEATHERS + u16::from(w),
        };
        set(view, dialog, WEATHER, text(weather));
        // The competition: a league's name and matchday with both clubs'
        // places from the second matchday on, else the cup's name.
        let kind = report.competition();
        let mut places = [String::new(), String::new()];
        let competition = if kind < 0x3f {
            let home = team(sides[0]);
            let league = home.and_then(|t| League::of(records, t.country().into(), t.level()));
            let played = league.as_ref().map_or(0, League::played);
            let matchday = fill(&text(MATCHDAY_TEXT), &[&minute(played as u8)]);
            if played > 1 {
                places = sides.map(|s| {
                    let p = team(s).map_or(0, |t| t.table_place());
                    text(PLACE_TEXT).replacen("%1", &p.to_string(), 1).replacen("%2", ".", 1)
                });
            }
            format!("{} - {matchday}", league.map(|l| l.name()).unwrap_or_default())
        } else {
            let [champions, uefa] = records.countries.cup_names();
            match kind & 0x7f00_0000 {
                0x4000_0000 => text(CUP),
                0x3000_0000 if sides[0].0 == GERMANY => text(LEAGUE_CUP),
                0x3000_0000 => text(SUPERCUP),
                0x0100_0000 => text(LEAGUE_CUP),
                0x1000_0000 => text(RELEGATION),
                0x2000_0000 => text(INTERTOTO),
                0x0400_0000 => text(FRIENDLY),
                // The national sides' qualifiers and finals ask whether a
                // World Cup is played (`FUN_0067bb89`), which the
                // replica's reports never come to.
                0x0800_0000 | 0x0200_0000 => String::new(),
                _ => match kind & 0x7f00_0700 {
                    0x5000_0400 | 0x5000_0700 => champions,
                    0x5000_0100 => uefa,
                    0x7000_0000 => text(CLUB_WORLD_CUP),
                    0x6000_0000 => text(EUROPEAN_SUPERCUP),
                    _ => String::new(),
                },
            }
        };
        set(view, dialog, COMPETITION, competition);
        set(view, dialog, MATCHDAY, String::new());
        for (id, place) in PLACES.into_iter().zip(places) {
            set(view, dialog, id, place);
        }
        let leg = report.first_leg().map_or(String::new(), |[h, a]| fill(&self.formats[0], &[&h.to_string(), &a.to_string()]));
        set(view, dialog, FIRST_LEG, leg);
        for (id, side) in NAMES.into_iter().zip(sides) {
            set(view, dialog, id, game.club_name(Some(records), side.0, side.1));
        }
        let comparison = report.comparison();
        for (side, ids) in COMPARISON.iter().enumerate() {
            // The report's order: second yellows, yellows, corners, chances.
            for (&id, figure) in ids.iter().zip([3, 1, 0, 2]) {
                set(view, dialog, id, comparison[figure][side].to_string());
            }
        }
        // The score, after extra time or penalties, and the half's.
        let [gh, ga] = report.goals();
        let state = report.state();
        let ending = if state & 0x40 != 0 {
            text(PENALTIES)
        } else if state & 0x80 != 0 {
            text(EXTRA_TIME)
        } else {
            String::new()
        };
        let mut score = format!("{gh}:{ga}");
        if !ending.is_empty() {
            score = format!("{score} {ending}");
        }
        let [hh, ha] = report.half();
        set(view, dialog, SCORE, format!("{score} ({hh}:{ha})"));
        // The gate, and the money each side made or paid: "-" for a side
        // without.
        if sides[0].1 == NATIONAL {
            for id in [INCOME].into_iter().chain(MONEY) {
                set(view, dialog, id, NONE.to_owned());
            }
        } else {
            set(view, dialog, INCOME, money(report.income()));
            for (&id, [h, a]) in MONEY.iter().zip(report.money().into_iter().rev()) {
                let side = |v: i32| if v == 0 { NONE.to_owned() } else { money(v) };
                let line = if h == 0 && a == 0 { NONE.to_owned() } else { fill(&self.formats[2], &[&side(h), &side(a)]) };
                set(view, dialog, id, line);
            }
        }
        let rating = usize::try_from(report.rating()).ok().and_then(|r| RATINGS.get(r)).map_or(String::new(), |&id| text(id));
        set(view, dialog, RATING, if blank { NONE.to_owned() } else { rating });
        let referee = records.referee(report.referee()).map_or("", |(name, ..)| name);
        let verdict = text(VERDICTS + report.verdict().max(0) as u16);
        let judged = if blank { String::new() } else { fill(&self.formats[1], &[referee, &verdict]) };
        set(view, dialog, REFEREE, judged);
        // The TV's share, with its station where a side's country is one
        // the game follows.
        let share = report.tv_share();
        let tv = if blank || share == 0 {
            NONE.to_owned()
        } else {
            let followed = sides.iter().any(|&(c, _)| records.globals.played(i16::from(c)));
            let station = a3_core::report::station_name(records, report.station()).unwrap_or_default();
            if followed { format!("{station}  {share}%") } else { format!("{share}%") }
        };
        set(view, dialog, TV_SHARE, tv);
        let minutes = if blank { NONE.to_owned() } else { fill(&text(MINUTES_TEXT), &[&report.minutes().to_string(), ""]) };
        set(view, dialog, MINUTES, minutes);
        let best = if blank || report.best() == 0 { NONE.to_owned() } else { name(report.best()) };
        set(view, dialog, BEST, best);
    }
}

impl Screen for MatchReport {
    fn ids(&self) -> &[u16] {
        &[MATCH_REPORT]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let dialog = cx.dialog;
        let Args::Reports { clubs, count } = args else { return };
        self.clubs = clubs[..usize::from(count).min(clubs.len())].to_vec();
        self.shown = 0;
        // The statics' own texts are the formats: the first leg's, the
        // referee's, the money's.
        self.formats = [FIRST_LEG, REFEREE, MONEY[2]]
            .map(|id| control(dialog, id).map_or(String::new(), |c| dialog.controls[c].text.clone()));
        self.show(cx, view);
        mark_inert(view, dialog, |t| match t {
            Target::Button(c) => dialog.controls[c].id == Some(OK),
            _ => false,
        });
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        match target {
            Target::Button(c) if dialog.controls[c].id == Some(OK) => Outcome::Close(Reply::Ok),
            // The line-ups (dialog 346) are not built.
            Target::Button(c) if dialog.controls[c].id == Some(LINE_UPS) => Outcome::Stay,
            // A crest below shows its club's report, the one shown taking
            // its place among them.
            Target::Control(c) => {
                let Some(k) = OTHER_CRESTS.iter().position(|&id| dialog.controls[c].id == Some(id)) else {
                    return Outcome::Stay;
                };
                let others: Vec<usize> = (0..self.clubs.len()).filter(|&k| k != self.shown).collect();
                if let Some(&next) = others.get(k) {
                    self.shown = next;
                    self.show(cx, view);
                }
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}
