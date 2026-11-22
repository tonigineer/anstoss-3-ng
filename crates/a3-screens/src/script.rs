//! Scripts that play the game without a window: for tests, and for running
//! the original under the probe with the same script.
//!
//! A script is a list of lines:
//!
//! - `# …` — a comment;
//! - `seed <n>` — where a new game's random numbers start;
//! - `on <dialog> <actions>` — what to do every time the dialog waits for
//!   the player;
//! - `once <dialog> <actions>` — what to do the next time only, ahead of the
//!   dialog's `on`; several queue up;
//! - `until dialog <id>`, `until day <n>`, `until end` — plays until that
//!   dialog waits, the season reaches that day, or the game ends;
//! - `repeat <n> days` — plays until the season has moved on that far.
//!
//! Actions, separated by `;`, act on the dialog's controls by their ids in
//! the dialog file: `ok`, `cancel`, `press <button>`, `click <static or
//! animation>`, `click bar <symbol>`, `choose <options> <item>`,
//! `row <list> <row>`, `cell <list> <row> <column>`, `type <field> <text>`,
//! `spin <spinner> up|down` (a slider's arrows too), `slide <slider>
//! <value>`, `rclick <control>`, `drag <box or row> <box or row>|off` (a
//! box or row is `<control>` or `<list> row <row>`), and `wait` until the
//! screen moves on by itself; or on the dialog's pixels: `click at <x>
//! <y>`, `drag at <x> <y> to <x> <y>`; and `random <s1> <s2>`, which sets
//! the game's random numbers to that state. A dialog the script has no actions
//! for stops it, as does an action for a control the dialog does not have.
//!
//! Dialogs go by their ids in the game's dialog file, as the original knows
//! them: the week is dialog 2, though the replica puts it together under ids
//! of its own.

use std::collections::{HashMap, VecDeque};
use std::fmt;

use a3_files::rch::{Dialog, Kind};
use bevy_math::Vec2;

use crate::controls::button::{cancel_button, ok_button};
use crate::game::Game;
use crate::host::{Change, Host};
use crate::pointer::{Grip, Pixels, Target};
use crate::screens::calendar::{WEEK, is_week};
use crate::screens::control;

/// Time passes in steps this long while a script waits, for at most this
/// long before it gives up.
const STEP_SECS: f32 = 0.05;
const MOST_SECS: f32 = 60.0;
const DEFAULT_SEED: u64 = 1;

/// A parsed script.
pub struct Script {
    pub(crate) seed: u64,
    pub(crate) lines: Vec<(usize, Line)>,
}

pub(crate) enum Line {
    On(u16, Vec<Action>),
    Once(u16, Vec<Action>),
    /// Actions on the dialog on top, at once, whether or not it moves on.
    Now(Vec<Action>),
    Until(Until),
    Repeat(usize),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Until {
    Dialog(u16),
    Day(usize),
    End,
}

#[derive(Clone, Debug)]
pub(crate) enum Action {
    Ok,
    Cancel,
    Press(u16),
    Click(u16),
    Bar(usize),
    Choose(u16, usize),
    Row(u16, usize),
    /// A row's column, in a list that answers by column.
    Cell(u16, usize, usize),
    Type(u16, String),
    Spin(u16, bool),
    Slide(u16, i32),
    /// Drags a control (a player's box) or a list's row onto another, or off
    /// them all.
    Drag(Spot, Option<Spot>),
    /// Drags whatever is at one point of the dialog to another.
    DragAt((i16, i16), (i16, i16)),
    /// Clicks whatever is at a point of the dialog.
    ClickAt((i16, i16)),
    /// The pointer moved to a point of the dialog, over whatever is there.
    PointAt((i16, i16)),
    /// A right click on a control or a list's row.
    RightClick(Spot),
    /// A second click on what another action clicked: the double click of
    /// a list's row, which the game takes for a choice.
    Dbl(Box<Action>),
    Wait,
    /// The game's random numbers set to a state, both halves.
    Random(i64, i64),
}

/// A control, or a row of a list, by its id.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Spot {
    Control(u16),
    Row(u16, usize),
    Item(u16, usize),
}

/// A spot at the start of `words`: `ID` or `ID row N`; and what follows it.
fn spot<'a>(words: &'a [&'a str]) -> Option<(Spot, &'a [&'a str])> {
    let id = words.first()?.parse().ok()?;
    match words.get(1..3) {
        Some(["row", row]) => Some((Spot::Row(id, row.parse().ok()?), &words[3..])),
        Some(["item", item]) => Some((Spot::Item(id, item.parse().ok()?), &words[3..])),
        _ => Some((Spot::Control(id), &words[1..])),
    }
}

/// Why a script stopped.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// A line it cannot read: its number and what is wrong.
    Line(usize, String),
    /// A dialog it has no actions for.
    NoActions(u16),
    /// A control the dialog does not have.
    NoControl { dialog: u16, control: u16 },
    /// A dialog that did not move on while it waited.
    Stuck(u16),
    /// Actions for a dialog that was gone before they were done.
    LeftOver(u16),
    /// The game ended before the condition was met.
    Ended,
    /// Days to repeat before there is a season.
    NoSeason,
    /// What the probe in the original said went wrong, or that it is gone.
    Probe(String),
    /// An action the original cannot be driven to do yet.
    Unsupported(u16, String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Line(line, what) => write!(f, "line {line}: {what}"),
            Self::NoActions(dialog) => {
                write!(f, "dialog {dialog} waits, and the script has no actions for it")
            }
            Self::NoControl { dialog, control } => {
                write!(f, "dialog {dialog} has no control {control}")
            }
            Self::Stuck(dialog) => write!(f, "dialog {dialog} did not move on"),
            Self::LeftOver(dialog) => {
                write!(f, "dialog {dialog} was gone before its actions were done")
            }
            Self::Ended => write!(f, "the game ended first"),
            Self::NoSeason => write!(f, "there is no season to count days in"),
            Self::Probe(what) => write!(f, "the probe: {what}"),
            Self::Unsupported(dialog, action) => {
                write!(f, "dialog {dialog}: the original cannot be driven to {action} yet")
            }
        }
    }
}

impl std::error::Error for Error {}

impl Script {
    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut seed = DEFAULT_SEED;
        let mut lines = Vec::new();
        for (index, raw) in text.lines().enumerate() {
            let number = index + 1;
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let bad = |what: &str| Error::Line(number, format!("{what}: {line}"));
            let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
            let rest = rest.trim();
            match word {
                "seed" => seed = rest.parse().map_err(|_| bad("not a seed"))?,
                "on" | "once" => {
                    let (dialog, actions) =
                        rest.split_once(char::is_whitespace).ok_or_else(|| bad("no actions"))?;
                    let dialog = dialog.parse().map_err(|_| bad("not a dialog"))?;
                    let actions = actions
                        .split(';')
                        .map(|a| parse_action(a.trim()).ok_or_else(|| bad("not an action")))
                        .collect::<Result<_, _>>()?;
                    lines.push((
                        number,
                        if word == "on" {
                            Line::On(dialog, actions)
                        } else {
                            Line::Once(dialog, actions)
                        },
                    ));
                }
                "now" => {
                    let actions = rest
                        .split(';')
                        .map(|a| parse_action(a.trim()).ok_or_else(|| bad("not an action")))
                        .collect::<Result<_, _>>()?;
                    lines.push((number, Line::Now(actions)));
                }
                "until" => {
                    let words: Vec<&str> = rest.split_whitespace().collect();
                    let until = match words[..] {
                        ["dialog", id] => {
                            Until::Dialog(id.parse().map_err(|_| bad("not a dialog"))?)
                        }
                        ["day", day] => Until::Day(day.parse().map_err(|_| bad("not a day"))?),
                        ["end"] => Until::End,
                        _ => return Err(bad("until what")),
                    };
                    lines.push((number, Line::Until(until)));
                }
                "repeat" => {
                    let words: Vec<&str> = rest.split_whitespace().collect();
                    let ["days" | "day", ..] = words.get(1..).unwrap_or_default() else {
                        return Err(bad("repeat how many days"));
                    };
                    let days = words[0].parse().map_err(|_| bad("not a number of days"))?;
                    lines.push((number, Line::Repeat(days)));
                }
                _ => return Err(bad("no such line")),
            }
        }
        Ok(Self { seed, lines })
    }
}

fn parse_action(action: &str) -> Option<Action> {
    let words: Vec<&str> = action.split_whitespace().collect();
    let id = |word: &str| word.parse::<u16>().ok();
    Some(match words[..] {
        ["ok"] => Action::Ok,
        ["cancel"] => Action::Cancel,
        ["wait"] => Action::Wait,
        ["random", s1, s2] => Action::Random(s1.parse().ok()?, s2.parse().ok()?),
        ["press", button] => Action::Press(id(button)?),
        ["click", "bar", symbol] => Action::Bar(symbol.parse().ok()?),
        ["click", control] => Action::Click(id(control)?),
        ["choose", options, item] => Action::Choose(id(options)?, item.parse().ok()?),
        ["row", list, row] => Action::Row(id(list)?, row.parse().ok()?),
        ["cell", list, row, column] => {
            Action::Cell(id(list)?, row.parse().ok()?, column.parse().ok()?)
        }
        ["spin", spinner, "up"] => Action::Spin(id(spinner)?, true),
        ["spin", spinner, "down"] => Action::Spin(id(spinner)?, false),
        ["slide", slider, value] => Action::Slide(id(slider)?, value.parse().ok()?),
        ["rclick", ref rest @ ..] => match spot(rest)? {
            (spot, []) => Action::RightClick(spot),
            _ => return None,
        },
        ["dbl", ref rest @ ..] => Action::Dbl(Box::new(parse_action(&rest.join(" "))?)),
        ["click", "at", x, y] => Action::ClickAt((x.parse().ok()?, y.parse().ok()?)),
        ["point", "at", x, y] => Action::PointAt((x.parse().ok()?, y.parse().ok()?)),
        ["drag", "at", x0, y0, "to", x1, y1] => {
            let at = |x: &str, y: &str| Some((x.parse().ok()?, y.parse().ok()?));
            Action::DragAt(at(x0, y0)?, at(x1, y1)?)
        }
        ["drag", ref rest @ ..] => {
            let (from, rest) = spot(rest)?;
            match rest {
                ["off"] => Action::Drag(from, None),
                _ => match spot(rest)? {
                    (to, []) => Action::Drag(from, Some(to)),
                    _ => return None,
                },
            }
        }
        ["type", field, ..] => {
            let text = action.splitn(3, char::is_whitespace).nth(2).unwrap_or_default();
            Action::Type(id(field)?, text.to_owned())
        }
        _ => return None,
    })
}

/// Plays scripts on a game of its own.
pub struct Runner<'a> {
    game: &'a Game,
    pub host: Host,
    on: HashMap<u16, Vec<Action>>,
    once: HashMap<u16, VecDeque<Vec<Action>>>,
    /// Every screen that came up and every action, in order.
    pub log: Vec<String>,
}

impl<'a> Runner<'a> {
    /// Starts a game whose new games take their random numbers from `seed`.
    pub fn new(game: &'a Game, seed: u64) -> Self {
        let host = Host::start(game, Box::new(move || seed));
        Self { game, host, on: HashMap::new(), once: HashMap::new(), log: Vec::new() }
    }

    /// Plays a script on a game of its own.
    pub fn play(game: &'a Game, script: &Script) -> (Self, Result<(), Error>) {
        let mut runner = Self::new(game, script.seed);
        let result = runner.run(script);
        (runner, result)
    }

    pub fn run(&mut self, script: &Script) -> Result<(), Error> {
        for (_, line) in &script.lines {
            match line {
                Line::On(dialog, actions) => _ = self.on.insert(*dialog, actions.clone()),
                Line::Once(dialog, actions) => {
                    self.once.entry(*dialog).or_default().push_back(actions.clone())
                }
                Line::Until(until) => self.until(*until)?,
                Line::Now(actions) => {
                    self.settle()?;
                    let shown = self.host.view().id;
                    for action in actions {
                        self.act(shown, action)?;
                    }
                }
                Line::Repeat(days) => {
                    self.settle()?;
                    let today = self.host.state().season.as_ref().map(|s| s.day);
                    let today = today.ok_or(Error::NoSeason)?;
                    self.until(Until::Day(today + days))?;
                }
            }
        }
        Ok(())
    }

    /// The dialog on top, by its id in the game's dialog file.
    pub fn dialog(&self) -> Option<u16> {
        self.host.is_open().then(|| named(self.host.view().id))
    }

    /// Plays until the condition is met.
    fn until(&mut self, until: Until) -> Result<(), Error> {
        loop {
            match self.settle() {
                Err(Error::Ended) if until == Until::End => return Ok(()),
                result => result?,
            }
            let shown = self.host.view().id;
            let id = named(shown);
            let met = match until {
                Until::Dialog(dialog) => id == dialog,
                Until::Day(day) => self.host.state().season.as_ref().is_some_and(|s| s.day >= day),
                Until::End => false,
            };
            if met {
                return Ok(());
            }
            let actions = match self.once.get_mut(&id).and_then(VecDeque::pop_front) {
                Some(actions) => actions,
                None => self.on.get(&id).cloned().ok_or(Error::NoActions(id))?,
            };
            for (n, action) in actions.iter().enumerate() {
                self.act(shown, action)?;
                let gone = !self.host.waiting() || self.host.view().id != shown;
                if gone && n + 1 < actions.len() {
                    return Err(Error::LeftOver(id));
                }
                if gone {
                    break;
                }
            }
        }
    }

    /// Waits for a screen that waits for the player.
    fn settle(&mut self) -> Result<(), Error> {
        while !self.host.waiting() {
            match self.host.next(self.game) {
                Change::Quit => return Err(Error::Ended),
                Change::Screen => self.log.push(format!("screen {}", named(self.host.view().id))),
                Change::Same => {}
            }
        }
        Ok(())
    }

    /// Acts on the controls of the dialog `shown`; `dbl` clicks the action
    /// it holds a second time, which is what Windows takes for a double
    /// click.
    fn act(&mut self, shown: u16, action: &Action) -> Result<(), Error> {
        if let Action::Dbl(inner) = action {
            self.act(shown, inner)?;
            return self.act_on(shown, inner, true);
        }
        self.act_on(shown, action, false)
    }

    /// Acts on the controls of the dialog `shown`, `double` for the second
    /// click of a double click.
    fn act_on(&mut self, shown: u16, action: &Action, double: bool) -> Result<(), Error> {
        self.log.push(format!("  {action:?}"));
        let game = self.game;
        let controls = &game.dialogs[&shown];
        let target = match *action {
            Action::Type(_, ref text) => {
                let Target::Edit(field) = target_of(controls, named(shown), action)? else {
                    unreachable!("a field to type into");
                };
                let change = self.host.input(game, Target::Edit(field), false);
                self.change(change);
                self.host.view_mut().set_text(field, "");
                self.host.view_mut().type_text(controls, text);
                // A screen that takes text as it is typed sees it at once.
                let change = self.host.tick(game, 0.0);
                self.change(change);
                return Ok(());
            }
            Action::Wait => return self.wait(shown),
            Action::Random(s1, s2) => {
                if let Some(season) = self.host.state().season.as_mut() {
                    season.random = a3_core::random::Random::from_state(s1, s2);
                }
                return Ok(());
            }
            Action::PointAt(at) => {
                self.host.view_mut().hovered = self.click_at(shown, at).ok();
                self.host.view_mut().pointer = Some(Vec2::new(at.0.into(), at.1.into()));
                let change = self.host.tick(game, 0.0);
                self.change(change);
                return Ok(());
            }
            // A drag from where nothing can be taken hold of does nothing.
            Action::DragAt(from, to) => match self.drag_at(shown, from, to) {
                // A map's tile is pressed, then let go of where the pointer
                // stays.
                Some(drop @ Target::Drop { from: grip @ Grip::Tile { .. }, to: end, .. }) => {
                    self.host.view_mut().pointer = Some(Vec2::new(from.0.into(), from.1.into()));
                    self.host.view_mut().hovered = Some(grip.target());
                    self.change_by(game, grip.target());
                    self.host.view_mut().pointer = Some(Vec2::new(to.0.into(), to.1.into()));
                    self.host.view_mut().hovered = end.map(Grip::target);
                    drop
                }
                Some(target) => target,
                None => return Ok(()),
            },
            // The pointer goes where it clicks.
            Action::ClickAt(at) => {
                let target = self.click_at(shown, at)?;
                self.host.view_mut().pointer = Some(Vec2::new(at.0.into(), at.1.into()));
                // A map's tile is pressed and let go of again, the pointer
                // staying on it.
                // A slider's track pages it towards the click.
                if let Target::SliderTrack(control) = target
                    && let Some(&value) = self.host.view().value(control)
                    && let Some(paged) =
                        crate::controls::slider::paged(&controls.controls[control], &value, f32::from(at.0))
                {
                    Target::SliderAt { control, value: paged }
                } else {
                match Grip::of(target) {
                    Some(grip @ Grip::Tile { .. }) => {
                        self.host.view_mut().hovered = Some(target);
                        self.change_by(game, target);
                        Target::Drop { from: grip, to: Some(grip), at, moved: (0, 0) }
                    }
                    _ => target,
                }
                }
            }
            _ => match target_of(controls, named(shown), action)? {
                // The pointer goes from the middle of one to the other's.
                Target::Drop { from, to, .. } => {
                    let view = self.host.view();
                    let middle = |grip: Grip| {
                        let rects = crate::pointer::targets(controls, view, &Pixels);
                        let rect = rects.into_iter().find(|&(t, _)| Grip::of(t) == Some(grip));
                        rect.map_or(Vec2::ZERO, |(_, r)| r.center())
                    };
                    let start = middle(from);
                    let d = to.map_or(Vec2::ZERO, |to| middle(to) - start);
                    let (at, moved) = ((start.x as i16, start.y as i16), (d.x as i16, d.y as i16));
                    Target::Drop { from, to, at, moved }
                }
                target => target,
            },
        };
        let change = self.host.input(game, target, double);
        self.change(change);
        Ok(())
    }

    /// What a drag from one point of the dialog `shown` to another takes
    /// hold of and lets go of on, and how far it goes; nothing if there is
    /// nothing to take hold of.
    fn drag_at(&self, shown: u16, from: (i16, i16), to: (i16, i16)) -> Option<Target> {
        let controls = &self.game.dialogs[&shown];
        let rects = crate::pointer::targets(controls, self.host.view(), &Pixels);
        let under = |(x, y): (i16, i16)| {
            let at = Vec2::new(f32::from(x), f32::from(y));
            rects.iter().rev().filter(|(_, r)| r.contains(at)).find_map(|&(t, _)| Grip::of(t))
        };
        let grip = under(from)?;
        let moved = (to.0 - from.0, to.1 - from.1);
        // A map's tile counts let go of where it was taken too.
        let to = under(to).filter(|&g| g != grip || matches!(g, Grip::Tile { .. }));
        Some(Target::Drop { from: grip, to, at: from, moved })
    }

    /// What a click at a point of the dialog `shown` acts on: the topmost
    /// target there.
    fn click_at(&self, shown: u16, (x, y): (i16, i16)) -> Result<Target, Error> {
        let controls = &self.game.dialogs[&shown];
        let at = Vec2::new(f32::from(x), f32::from(y));
        let rects = crate::pointer::targets(controls, self.host.view(), &Pixels);
        let found = rects.iter().rev().find(|(_, r)| r.contains(at)).map(|&(t, r)| match t {
            // A drawn picture is told the spot, from its top left.
            Target::Point { control, .. } => {
                Target::Point { control, x: x - r.min.x as i16, y: y - r.min.y as i16 }
            }
            t => t,
        });
        found.ok_or_else(|| Error::Unsupported(named(shown), format!("a click at {x} {y}")))
    }

    /// Lets time pass until the screen `shown` moves on by itself.
    fn wait(&mut self, shown: u16) -> Result<(), Error> {
        let mut waited = 0.0;
        while self.host.waiting() && self.host.view().id == shown {
            if waited > MOST_SECS {
                return Err(Error::Stuck(named(shown)));
            }
            let change = self.host.tick(self.game, STEP_SECS);
            self.change(change);
            waited += STEP_SECS;
        }
        Ok(())
    }

    /// The screen given a target.
    fn change_by(&mut self, game: &Game, target: Target) {
        let change = self.host.input(game, target, false);
        self.change(change);
    }

    fn change(&mut self, change: Change) {
        if change == Change::Screen {
            self.log.push(format!("screen {}", named(self.host.view().id)));
        }
    }
}

/// What an action acts on in `controls`, the dialog `dialog`; `wait` acts on
/// nothing.
pub(crate) fn target_of(controls: &Dialog, dialog: u16, action: &Action) -> Result<Target, Error> {
    let find = |id: u16| control(controls, id).ok_or(Error::NoControl { dialog, control: id });
    let button = |found: Option<usize>| found.ok_or(Error::NoControl { dialog, control: 0 });
    Ok(match *action {
        Action::Ok => Target::Button(button(ok_button(controls))?),
        Action::Cancel => Target::Button(button(cancel_button(controls))?),
        Action::Press(id) => Target::Button(find(id)?),
        Action::Click(id) => {
            let control = find(id)?;
            match controls.controls[control].kind {
                Kind::Check { .. } => Target::Check(control),
                Kind::Combo { .. } => Target::Combo(control),
                _ => Target::Control(control),
            }
        }
        Action::Bar(item) => {
            let bar = controls.controls.iter().position(|c| matches!(c.kind, Kind::Bar { .. }));
            Target::Option { control: bar.ok_or(Error::NoControl { dialog, control: 0 })?, item }
        }
        Action::Choose(id, item) => Target::Option { control: find(id)?, item },
        Action::Row(id, row) => Target::Row { control: find(id)?, row },
        Action::Cell(id, row, column) => Target::Cell { control: find(id)?, row, column },
        Action::Spin(id, up) => {
            let control = find(id)?;
            match controls.controls[control].kind {
                Kind::Slider { .. } => Target::SliderStep { control, up },
                _ => Target::Spin { control, up },
            }
        }
        Action::Slide(id, value) => Target::SliderAt { control: find(id)?, value },
        Action::Drag(from, to) => {
            let grip = |spot: Spot| grip(controls, dialog, spot);
            Target::Drop {
                from: grip(from)?,
                to: to.map(grip).transpose()?,
                at: (0, 0),
                moved: (0, 0),
            }
        }
        Action::RightClick(spot) => Target::Menu(grip(controls, dialog, spot)?),
        Action::Type(id, _) => Target::Edit(find(id)?),
        Action::Wait => return Err(Error::Unsupported(dialog, "wait on a control".into())),
        Action::Random(..) => return Err(Error::Unsupported(dialog, "random numbers".into())),
        // A double click acts twice; the runner does that, not this.
        Action::Dbl(_) => return Err(Error::Unsupported(dialog, "a double click".into())),
        Action::DragAt(..) | Action::ClickAt(_) | Action::PointAt(_) => {
            return Err(Error::Unsupported(dialog, "an action by pixels".into()));
        }
    })
}

/// What a spot of a script takes hold of in `controls`.
fn grip(controls: &Dialog, dialog: u16, spot: Spot) -> Result<Grip, Error> {
    let find = |id: u16| control(controls, id).ok_or(Error::NoControl { dialog, control: id });
    Ok(match spot {
        Spot::Control(id) => Grip::Control(find(id)?),
        Spot::Row(id, row) => Grip::Row { control: find(id)?, row },
        Spot::Item(id, item) => Grip::Option { control: find(id)?, item },
    })
}

/// A dialog the replica shows by its id in the game's dialog file.
fn named(shown: u16) -> u16 {
    if is_week(shown) { WEEK } else { shown }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::screens::compose;

    fn game() -> Game {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let mut game = Game::load(&dir).unwrap();
        // The walks' rolling autosaves stay out of the shared saves: one
        // landing there mid-run would move every row of the load list
        // under a parallel test's `on_save`.
        let autosaves = std::env::temp_dir().join("a3-script-autosaves");
        std::fs::create_dir_all(&autosaves).unwrap();
        std::sync::Arc::get_mut(&mut game.data).expect("the game's data, just loaded").saves = autosaves;
        compose(&mut game);
        game
    }

    fn seen(runner: &Runner, dialog: u16) -> usize {
        let screen = format!("screen {dialog}");
        runner.log.iter().filter(|line| **line == screen).count()
    }

    /// A save of the game patched and written anew under `out`, in a copy
    /// of the game's directory of symlinks with its own `saves`; the
    /// directory to load the game from again.
    fn patched(
        dir: &std::path::Path,
        name: &str,
        out: &str,
        patch: impl FnOnce(&mut a3_core::records::Records),
    ) -> PathBuf {
        use a3_core::records::Records;
        let keeps = dir.parent().unwrap().join("saves-kept");
        let bytes = [dir.join(format!("saves/{name}.an3")), keeps.join(format!("{name}.an3"))]
            .into_iter()
            .find_map(|path| std::fs::read(path).ok())
            .unwrap();
        let mut records = Records::from_save(a3_files::an3::Save::parse(&bytes).unwrap()).unwrap();
        patch(&mut records);
        let root = std::env::temp_dir().join(format!("a3-script-{out}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("saves")).unwrap();
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            if entry.file_name() != "saves" {
                std::os::unix::fs::symlink(entry.path(), root.join(entry.file_name())).unwrap();
            }
        }
        let bytes = records.to_save().to_bytes_stored().unwrap();
        std::fs::write(root.join(format!("saves/{out}.an3")), bytes).unwrap();
        root
    }

    /// A new game for two, played to the first matchday.
    const NEW_GAME: &str = include_str!("../scripts/new-game.txt");

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_new_game_plays_to_the_first_matchday() {
        let game = game();
        let script = Script::parse(NEW_GAME).unwrap();
        let (runner, result) = Runner::play(&game, &script);
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let state = runner.host.state();
        let season = state.season.as_ref().unwrap();
        // The Bundesliga starts on day 48, in the seventh week; the script
        // stops at the first turn of day 42, before it.
        assert_eq!(season.day, 42);
        assert!(season.managers[0].club.is_some() && season.managers[1].club.is_none());
        assert_eq!(seen(&runner, WEEK), 6, "{}", runner.log.join("\n"));
        // Every turn ends with the whistle, one before each week: the
        // manager without a club plays none (`FUN_0064e40f`); the script
        // stops at the first turn of day 42.
        let whistles = runner.log.iter().filter(|line| line.trim() == "Bar(18)").count();
        assert_eq!(whistles, 6);
    }

    /// Loads the newest save and stops on the line-up.
    const LOAD: &str = include_str!("../scripts/load.txt");

    /// The start view a save chose for its manager opens at the turn:
    /// the office 483 and the text menu 501, each in place of the
    /// line-up.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_chosen_start_view_opens() {
        use a3_core::cycle::dialogs;
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        for (view, screen) in [(1u8, dialogs::OFFICE), (2, dialogs::TEXT_MENU)] {
            let name = format!("view{view}");
            let root = patched(&dir, "Toni", &name, |records| {
                for m in &mut records.managers {
                    m.set_menu_view(view);
                }
            });
            let mut game = Game::load(&root).unwrap();
            compose(&mut game);
            let load = LOAD.replace("until dialog 147", &format!("until dialog {screen}"));
            let (runner, result) = Runner::play(&game, &Script::parse(&load).unwrap());
            result.unwrap_or_else(|err| panic!("{name}: {err}\n{}", runner.log.join("\n")));
            assert_eq!(runner.host.view().id, screen, "{name}");
        }
    }

    /// A game loaded mid-week turns on its saved day and then walks only
    /// the rest of its week: the traced original of save "Toni" (day 62,
    /// its week from 56, 7 Nov) showed dialog 147 again at day 63, with
    /// the week's wait 381 just before it — not seven days on at 69 — and
    /// between them the window of the record the save holds, which the
    /// turn before the match leaves alone. Behind that turn the saved day's
    /// own matches are played (A, the same game a day on, holds Wolfsburg
    /// against Union), their dialogs before the day moves on.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_load_mid_week_turns_again_at_its_weeks_boundary() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "Toni", "midweek", |_| {});
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let script = Script::parse(
            "once 9 choose 4771 1; ok
             on 424 row 5144 0; ok
             until dialog 147
             on 147 click bar 18
             on 2 wait
             on 461 ok
             on 270 ok
             on 381 wait
             on 477 ok
             on 96 ok
             on 93 press 1943
             on 499 ok
             on 41 ok
             on 332 ok
             on 269 ok
             on 383 click at 460 210
             until day 64",
        )
        .unwrap();
        let (runner, result) = Runner::play(&game, &script);
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let state = runner.host.state();
        let (season, records) = (state.season.as_ref().unwrap(), state.records.as_ref().unwrap());
        // The turns came at 62 and at 63, and the walk stands in the new
        // week at 64; the turns of 69 would mean the week ran wrong.
        assert_eq!(season.day, 64, "{}", runner.log.join("\n"));
        assert_eq!(seen(&runner, 147), 2, "{}", runner.log.join("\n"));
        assert_eq!(seen(&runner, 381), 1, "{}", runner.log.join("\n"));
        assert_eq!(seen(&runner, 477), 1, "{}", runner.log.join("\n"));
        // The saved week's start (56) stood through the saved day's turn;
        // the planner counts from 63 only since the new week opened.
        assert_eq!(records.globals.week_start(), 63);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn every_save_opens_on_its_line_up() {
        let game = game();
        let saves = game.save_games();
        assert!(!saves.is_empty(), "no saves");
        for (row, save) in saves.iter().enumerate() {
            let script =
                Script::parse(&LOAD.replace("row 5144 0", &format!("row 5144 {row}"))).unwrap();
            let (runner, result) = Runner::play(&game, &script);
            // A first manager without a club opens his turn in the
            // dressing room (S2), one who chose it in his office or text
            // menu (Office), not on a line-up.
            use a3_core::cycle::dialogs::{DRESSING_ROOM, GRAND_OFFICE, OFFICE, TEXT_MENU};
            if [DRESSING_ROOM, OFFICE, GRAND_OFFICE, TEXT_MENU].contains(&runner.host.view().id) {
                continue;
            }
            result.unwrap_or_else(|err| panic!("{}: {err}\n{}", save.name, runner.log.join("\n")));
            let view = runner.host.view();
            assert_eq!(view.id, 147, "{}", save.name);
            let state = runner.host.state();
            let (season, records) =
                (state.season.as_ref().unwrap(), state.records.as_ref().unwrap());
            assert_eq!(season.day, usize::from(records.globals.day()));
            let [day, month, year] = records.globals.date();
            let dated = (0..view.controls.len())
                .any(|c| view.text(c).ends_with(&format!(", {day}.{month}.{year}")));
            assert!(dated, "{}: no caption with the date {day}.{month}.{year}", save.name);

            // The squad as the save has it: the eleven, the bench, the rest
            // backwards from the squad by position, as the original's list
            // opens — unless the save goes on with a matchday's matches,
            // before which the co-trainer may line the team up anew (see
            // `the_co_trainer_lines_up_a_resumed_matchday`).
            let manager = records.humans().next().unwrap();
            let team = records.team(manager.country(), manager.team()).unwrap();
            let squad = team.players();
            let position = |id: u16| records.player(id).unwrap().positions()[0];
            let by_position: Vec<u16> = (1..=10)
                .flat_map(|p| squad.iter().copied().filter(move |&id| position(id) == p))
                .collect();
            let mut order: Vec<u16> =
                team.lineup().into_iter().chain(team.bench()).filter(|&id| id != 0).collect();
            if records.globals.resume() == MATCHES_RESUMED {
                let club = &season.clubs[&season.manager().unwrap().club.unwrap()];
                order = club.lineup.squad().map(|p| club.status[p].id).collect();
            }
            let rest: Vec<u16> =
                by_position.iter().rev().copied().filter(|id| !order.contains(id)).collect();
            order.extend(rest);
            let list = view.controls.iter().find_map(|c| match &c.state {
                crate::controls::ControlState::List(list) if list.rows.len() == squad.len() => {
                    Some(list)
                }
                _ => None,
            });
            let list =
                list.unwrap_or_else(|| panic!("{}: no list of {} players", save.name, squad.len()));
            for (row, id) in list.rows.iter().zip(order) {
                let player = records.player(id).unwrap();
                let columns: Vec<&str> = row.split('\t').collect();
                let initial = records
                    .name(player.first_name())
                    .chars()
                    .next()
                    .map_or(String::new(), |c| format!(", {c}."));
                assert_eq!(
                    columns[0],
                    format!("{}{initial}", records.name(player.last_name())),
                    "{}",
                    save.name
                );
                assert_eq!(columns[3], player.strength().to_string());
                assert_eq!(columns[6], player.morale().to_string());
                assert_eq!(columns[8], player.shirt().to_string());
                let age = season.date().years_since(player.birthday().unwrap()).unwrap();
                assert_eq!(columns[9], age.to_string());
            }
        }
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn dragged_players_move_as_the_game_moves_them() {
        let game = game();
        let (mut runner, result) = Runner::play(&game, &Script::parse(LOAD).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let dialog = &game.dialogs[&147];
        let at = |id: u16| crate::screens::control(dialog, id).unwrap();
        let lineup = |runner: &Runner| {
            let state = runner.host.state();
            let season = state.season.as_ref().unwrap();
            season.clubs[&season.manager().unwrap().club.unwrap()].lineup.clone()
        };
        let drop = |runner: &mut Runner, from: Grip, to: Option<Grip>| {
            runner.host.input(&game, Target::Drop { from, to, at: (0, 0), moved: (0, 0) }, false);
        };
        let before = lineup(&runner);
        // Two boxes on the pitch swap their players …
        drop(&mut runner, Grip::Control(at(2510)), Some(Grip::Control(at(2511))));
        drop(&mut runner, Grip::Control(at(2512)), Some(Grip::Control(at(2521))));
        // … one let go of off the boxes leaves the line-up …
        drop(&mut runner, Grip::Control(at(2520)), None);
        // … and one of the list's rest takes a box: the 19th row, after the
        // eleven and the bench (scrolled out of sight here).
        drop(&mut runner, Grip::Row { control: at(2509), row: 17 }, Some(Grip::Control(at(2527))));
        let after = lineup(&runner);
        assert_eq!((after.field[0], after.field[1]), (before.field[1], before.field[0]));
        assert_eq!((after.field[2], after.bench[0]), (before.bench[0], before.field[2]));
        assert_eq!(after.field[10], None);
        let rest: Vec<usize> =
            (0..).take_while(|&p| p < 40).filter(|p| !before.squad().any(|q| q == *p)).collect();
        assert!(after.bench[6].is_some() && after.bench[6] != before.bench[6]);
        assert!(rest.contains(&after.bench[6].unwrap()), "the box took one of the rest");
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_formation_chosen_stands_the_line_up_in_it() {
        let game = game();
        let script = format!(
            "{}\nnow choose 5266 1\nuntil dialog 244\nnow choose 3386 3; press 1\nuntil dialog 147",
            LOAD.trim_end()
        );
        let (runner, result) = Runner::play(&game, &Script::parse(&script).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let state = runner.host.state();
        let season = state.season.as_ref().unwrap();
        let manager = season.manager().unwrap();
        let club = &season.clubs[&manager.club.unwrap()];
        assert_eq!(manager.formations[3].0, "4-4-2");
        assert_eq!(club.formation, 3);
        assert_eq!(club.lineup.spots, manager.formations[3].1);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_special_variant_puts_up_players_on_their_own_positions() {
        let game = game();
        let script = format!(
            "{}\nnow choose 2572 0\nuntil dialog 386\nnow choose 5330 7\nuntil dialog 147",
            LOAD.trim_end()
        );
        let (runner, result) = Runner::play(&game, &Script::parse(&script).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let state = runner.host.state();
        let season = state.season.as_ref().unwrap();
        let club = &season.clubs[&season.manager().unwrap().club.unwrap()];
        let pitch = &game.rules.pitch;
        let lineup = &club.lineup;
        for (spot, player) in lineup.on_field() {
            assert_eq!(club.players[player].positions[0], pitch.kind_under(spot));
        }
        let squad: Vec<usize> = lineup.squad().collect();
        let mut unique = squad.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), squad.len(), "nobody twice");
        let first = lineup.bench[0].unwrap();
        assert_eq!(club.players[first].positions[0], a3_core::world::GOALKEEPER);
    }

    /// The cohesion the original showed for the gate's save (P0's check.sh)
    /// after each of these, on the line-up (`tools/oracle`, 13.10.2026).
    const COHESION_OF_THE_ORIGINAL: [(&str, i32); 5] = [
        ("", 14),
        ("now drag 2510 2511; drag 2512 2521; drag 2520 off; drag 2509 row 9 2527", 2),
        ("now choose 5266 1\nuntil dialog 244\nnow choose 3386 3; press 1\nuntil dialog 147", 6),
        ("now choose 2572 0\nuntil dialog 386\nnow choose 5330 1\nuntil dialog 147", 21),
        ("now choose 2572 0\nuntil dialog 386\nnow choose 5330 7\nuntil dialog 147", 21),
    ];

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_line_ups_cohesion_is_the_originals() {
        let game = game();
        let Some(row) = game.save_games().iter().position(|s| s.name == "Probe1") else {
            eprintln!("no Probe1 among the saves; the oracle's copy has it");
            return;
        };
        for (actions, expected) in COHESION_OF_THE_ORIGINAL {
            let load = LOAD.replace("row 5144 0", &format!("row 5144 {row}"));
            let script = format!("{}\n{actions}", load.trim_end());
            let (runner, result) = Runner::play(&game, &Script::parse(&script).unwrap());
            result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
            let state = runner.host.state();
            let season = state.season.as_ref().unwrap();
            let club = &season.clubs[&season.manager().unwrap().club.unwrap()];
            let cohesion = a3_core::cohesion::of(&game.rules.pitch, club, season.date());
            assert_eq!(cohesion, expected, "after {actions:?}");
        }
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_medical_department_lists_the_injured_and_the_knocked() {
        let game = game();
        let script = format!("{}\nnow choose 5266 6\nuntil dialog 151", LOAD.trim_end());
        let (runner, result) = Runner::play(&game, &Script::parse(&script).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let state = runner.host.state();
        let season = state.season.as_ref().unwrap();
        let club = &season.clubs[&season.manager().unwrap().club.unwrap()];
        let records = state.records.as_ref().unwrap();
        let knocks = |p: usize| records.player(club.status[p].id).map_or(0, |r| r.knocks());
        let sick = a3_core::medical::sick_list(club, knocks);
        let view = runner.host.view();
        let list = view.list(control(&game.dialogs[&151], 2586).unwrap()).unwrap();
        if sick.is_empty() {
            assert_eq!(list.rows, ["Alle Spieler sind derzeit einsetzbar."]);
        } else {
            assert_eq!(list.keys, sick);
            assert_eq!(list.rows.len(), sick.len());
        }
        // OK goes back to the line-up.
        let script = format!("{script}\nnow ok\nuntil dialog 147");
        let (runner, result) = Runner::play(&game, &Script::parse(&script).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_formation_keeps_where_the_boxes_stand() {
        let game = game();
        let script = format!(
            "{}\nnow choose 5266 2\nuntil dialog 384\nnow ok\nuntil dialog 147",
            LOAD.trim_end()
        );
        let (runner, result) = Runner::play(&game, &Script::parse(&script).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let state = runner.host.state();
        let season = state.season.as_ref().unwrap();
        let manager = season.manager().unwrap();
        let club = &season.clubs[&manager.club.unwrap()];
        assert_eq!(manager.formations[club.formation].1, club.lineup.spots);
        let records = state.records.as_ref().unwrap();
        let record = &records.managers[records.human_slot(season.current).unwrap()];
        assert_eq!(record.formations()[club.formation].1, club.lineup.spots);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_penalty_taker_picked_takes_the_penalties() {
        use a3_core::duties::Duty;
        let game = game();
        let open =
            format!("{}\nnow choose 5266 3\nuntil dialog 418\nnow choose 5340 2", LOAD.trim_end());
        let (runner, result) = Runner::play(&game, &Script::parse(&open).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let view = runner.host.view();
        let list = control(&game.dialogs[&418], 5339).unwrap();
        let rows = &view.list(list).unwrap().rows;
        let name = rows[3].split('\t').nth(1).unwrap().to_owned();
        assert_eq!(rows[3].split('\t').next(), Some("4"));

        let script = format!("{open}\nnow row 5339 3; ok\nuntil dialog 147");
        let (runner, result) = Runner::play(&game, &Script::parse(&script).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let state = runner.host.state();
        let season = state.season.as_ref().unwrap();
        let club = &season.clubs[&season.manager().unwrap().club.unwrap()];
        // He goes first, the three above him one down; the record has it.
        let taker = club.duties[Duty::Penalty.stored()][0];
        let p = &club.players[taker];
        let initial = p.first_name.chars().next().map_or(String::new(), |c| format!(", {c}."));
        assert_eq!(format!("{}{initial}", p.last_name), name);
        assert_eq!(club.duties[Duty::Penalty.stored()].len(), club.players.len().min(35));
        let records = state.records.as_ref().unwrap();
        let human = records.humans().next().unwrap();
        let team = records.team(human.country(), human.team()).unwrap();
        let extra = &records.extras[usize::from(team.human()) - 1];
        assert_eq!(extra.duty_lists()[Duty::Penalty.stored()][0], club.status[taker].id);
        // On the pitch his box shows the penalty, if he plays.
        let view = runner.host.view();
        if let Some(slot) = club.lineup.field.iter().position(|&f| f == Some(taker)) {
            let c = control(&game.dialogs[&147], 2510 + slot as u16).unwrap();
            let shown = view.controls[c].state.as_player().unwrap().duty;
            assert_eq!(shown, game.rules.pitch.duty_icon(Duty::Penalty));
        }
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_player_info_shows_what_the_originals_does() {
        let game = game();
        let Some(row) = game.save_games().iter().position(|s| s.name == "Probe1") else {
            eprintln!("no Probe1 among the saves; the oracle's copy has it");
            return;
        };
        let load = LOAD.replace("row 5144 0", &format!("row 5144 {row}"));
        let script =
            format!("{}\nnow rclick 2510; choose 9950 0\nuntil dialog 399", load.trim_end());
        let (runner, result) = Runner::play(&game, &Script::parse(&script).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let view = runner.host.view();
        let dialog = &game.dialogs[&399];
        let text = |id: u16| view.text(crate::screens::control(dialog, id).unwrap()).to_owned();
        // As the original showed the gate's keeper (P0's check.sh).
        assert_eq!(text(4825), "Max Schmitt");
        assert_eq!(text(4826), "19 Jahre (18.1.2006)");
        assert_eq!(text(4834), "144.000 €");
        assert_eq!(text(4843), "1.36 Mio €");
        assert_eq!(text(4844), "Vertrag bis 30.6.2028 (noch drei Jahre).");
        assert_eq!(text(4878), "Er ist gesund.");
        assert_eq!([text(4971), text(4972)], ["Geldgeier", "Skandalnudel"]);
    }

    /// Plays the save `save` to its line-up, then `then`; the runner, or
    /// none if the installation has no such save.
    fn on_save<'a>(game: &'a Game, save: &str, then: &str) -> Option<Runner<'a>> {
        let Some(row) = game.save_games().iter().position(|s| s.name == save) else {
            eprintln!("no {save} among the saves; the oracle's copy has it");
            return None;
        };
        let load = LOAD.replace("row 5144 0", &format!("row 5144 {row}"));
        let script = format!("{}\n{then}", load.trim_end());
        let (runner, result) = Runner::play(game, &Script::parse(&script).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        Some(runner)
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_bonus_for_the_team_costs_and_takes_the_edge_off() {
        let game = game();
        // The keeper's menu: "Spielerinfo", a blank line, "Prämie" (no talk
        // on the week's last day), the bonus for the team at its start,
        // 100.000 €.
        let then = "now rclick 2510; choose 9950 2\nuntil dialog 195\nnow choose 2947 0; ok\n\
                    until dialog 380\nnow press 4731\nuntil dialog 147";
        let Some(before) = on_save(&game, "Toni", "") else { return };
        let runner = on_save(&game, "Toni", then).unwrap();
        let team = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let manager = records.humans().next().unwrap();
            let team = records.team(manager.country(), manager.team()).unwrap();
            (team.bonuses_paid(), team.will_to_win())
        };
        let ((paid, will), (paid_now, will_now)) = (team(&before), team(&runner));
        assert_eq!(paid_now, paid + 100_000);
        // Two tries in the first league, two off the will to win each.
        assert_eq!(will_now, will - 4);
        assert!(runner.log.iter().any(|l| l == "screen 380"));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_fine_for_nothing_is_refused() {
        let game = game();
        let then =
            "now rclick 2510; choose 9950 3\nuntil dialog 85\nnow press 1867\nuntil dialog 88";
        let Some(runner) = on_save(&game, "Toni", then) else { return };
        let view = runner.host.view();
        let dialog = &game.dialogs[&88];
        let text = view.text(crate::screens::control(dialog, 1873).unwrap());
        assert_eq!(
            text,
            "Frederik Rönnow hat absolut nichts gemacht, daß eine Geldstrafe rechtfertigen würde!"
        );
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_opponent_info_tells_what_the_originals_does() {
        let game = game();
        let then = "now choose 5266 7\nuntil dialog 83";
        let Some(runner) = on_save(&game, "Toni", then) else { return };
        // As the original shows Union's next opponent on day 62.
        let view = runner.host.view();
        let dialog = &game.dialogs[&83];
        let text = |id: u16| view.text(crate::screens::control(dialog, id).unwrap());
        assert_eq!(text(1828), "VfL Wolfsburg");
        assert_eq!(text(1829), "9. Platz (3 Punkte,2 Spiele) (zuletzt: sN)");
        assert_eq!(text(1830), "Runde 2 (RB Leipzig)");
        assert_eq!(text(1831), "nicht dabei");
        assert_eq!(text(1848), "gesegnet");
        assert_eq!(text(1825), "Volkswagen Arena, 30000 Zuschauer");
        // Clubs further in Europe, as EUROPAPOKAL-STAND.md reads the save:
        // a seed, a qualifier winner, a UEFA qualifier, a direct entrant
        // before the first round's draw.
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let stand = |name: &str| {
            let team = records.teams.iter().find(|t| t.name() == name).unwrap();
            a3_core::opponent::europe_stand(records, team)
        };
        use a3_core::opponent::EuropeStand::{In, NotIn};
        assert_eq!(stand("FC Bayern München"), In(0, 1871, None));
        let silkeborg = records.teams.iter().find(|t| t.name() == "Silkeborg IF").unwrap();
        let silkeborg = (silkeborg.country(), i16::from(silkeborg.number()));
        assert_eq!(stand("Eintracht Frankfurt"), In(0, 1871, Some(silkeborg)));
        assert!(matches!(stand("Paksi FC"), In(1, 1864, Some(_))));
        assert_eq!(stand("Borussia Dortmund"), NotIn);
        assert_eq!(stand("Silkeborg IF"), NotIn);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_list_view_takes_players_in_and_out_as_the_originals_does() {
        let game = game();
        let dialog = &game.dialogs[&429];
        let text = |runner: &Runner, id: u16| {
            runner.host.view().text(crate::screens::control(dialog, id).unwrap()).to_owned()
        };
        // As the original shows Union's line-up as a list on day 62: the
        // parts the pitch view reckoned, all eleven in.
        let then = "now choose 5266 0\nuntil dialog 429";
        let Some(runner) = on_save(&game, "Toni", then) else { return };
        assert_eq!(text(&runner, 5277), "Derzeit sind 11 Spieler aufgestellt!");
        let parts = [2549, 2550, 2551, 2552, 2555].map(|id| text(&runner, id));
        assert_eq!(parts, ["9.5", "31.1", "34.4", "22.9", "97.9"]);
        assert_eq!(text(&runner, 2556), "3.8");
        // The keeper out (the bench is full), then the spare keeper in: he
        // takes the empty slot, the parts stay as they were.
        let then = format!("{then}\nnow row 2509 0\nuntil dialog 429");
        let runner = on_save(&game, "Toni", &then).unwrap();
        assert_eq!(text(&runner, 5277), "Derzeit sind 10 Spieler aufgestellt!");
        assert_eq!(text(&runner, 2556), "3.6");
        let then = format!("{then}\nnow row 2509 24\nuntil dialog 429");
        let runner = on_save(&game, "Toni", &then).unwrap();
        assert_eq!(text(&runner, 5277), "Derzeit sind 11 Spieler aufgestellt!");
        assert_eq!(text(&runner, 2556), "3.7");
        assert_eq!(text(&runner, 2549), "9.5");
        let state = runner.host.state();
        let season = state.season.as_ref().unwrap();
        let club = &season.clubs[&season.manager().unwrap().club.unwrap()];
        let keeper = club.lineup.field[0].map(|p| club.players[p].last_name.clone());
        assert_eq!(keeper.as_deref(), Some("Klaus"));
        // The view is kept with the manager, in his record too: the switch
        // goes back to the pitch.
        assert!(season.manager().unwrap().list_view);
        assert!(state.records.as_ref().unwrap().managers[0].list_view());
        drop(state);
        let then = format!("{then}\nnow choose 5266 0\nuntil dialog 147");
        let runner = on_save(&game, "Toni", &then).unwrap();
        let state = runner.host.state();
        assert!(!state.season.as_ref().unwrap().manager().unwrap().list_view);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_tactics_show_and_keep_what_the_originals_do() {
        let game = game();
        let dialog = &game.dialogs[&271];
        let text = |runner: &Runner, id: u16| {
            runner.host.view().text(crate::screens::control(dialog, id).unwrap()).to_owned()
        };
        // As the original shows Union's tactics before Wolfsburg away.
        let then = "now choose 9930 2\nuntil dialog 271";
        let Some(runner) = on_save(&game, "Toni", then) else { return };
        assert_eq!(text(&runner, 3735), "1.FC Union Berlin (A, 6.) - VfL Wolfsburg (H, 9.)");
        assert_eq!(text(&runner, 3736), "Schiedsrichter: Schmidt (Komp. 8, H. 5)");
        let labels = [6278, 6277, 6276, 3754].map(|id| text(&runner, id));
        assert_eq!(
            labels,
            ["Spielweise: Vorsichtig", "Härte: Normal", "Einsatz: Normal", "Zeitspiel: nie"]
        );
        assert_eq!([text(&runner, 3741), text(&runner, 5518)], ["Stärke: 3", " 0 €"]);
        // Every kind of control once, as the original took them; then away
        // to the line-up and back.
        let then = format!(
            "{then}\nnow press 3737; rclick 3738; press 3739; press 3756; press 3739; press 3757\n\
             now choose 3762 5; choose 3742 0; choose 5517 0; choose 6293 0; choose 5517 0\n\
             now spin 5519 up; spin 5519 up; spin 5519 up; spin 5520 up; spin 5522 down; \
             spin 5521 down\nnow choose 9930 0\nuntil dialog 147\nnow choose 9930 2\n\
             until dialog 271"
        );
        let runner = on_save(&game, "Toni", &then).unwrap();
        let labels = [6278, 6277, 6276, 3754].map(|id| text(&runner, id));
        assert_eq!(
            labels,
            ["Spielweise: Normal", "Härte: Rauh", "Einsatz: 150%", "Zeitspiel: immer"]
        );
        assert_eq!(text(&runner, 5518), " 30 €");
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        // Union's team record.
        let team = &records.teams[84];
        assert_eq!([team.style(), team.hardness(), team.effort()], [3, 1, 0]);
        // "Zeitspiel" immer, tricks, the offside trap, "Langsamer
        // Spielaufbau", the middle; passing as the original's dump has it.
        assert_eq!(team.tactic_flags(), 0x1 | 0x4 | 0x8 | 5 << 7 | 0x800);
        assert_eq!(team.passing(), 0x6054);
        assert_eq!(records.extras[0].special_bonus(), 30);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_comparison_marks_man_for_man_as_the_original_does() {
        let game = game();
        let dialog = &game.dialogs[&78];
        let text = |runner: &Runner, id: u16| {
            runner.host.view().text(crate::screens::control(dialog, id).unwrap()).to_owned()
        };
        let texts = |runner: &Runner, ids: &[u16]| -> Vec<String> {
            ids.iter().map(|&id| text(runner, id)).collect()
        };
        // As the original shows Union against Wolfsburg.
        let then = "now choose 9930 1\nuntil dialog 78";
        let Some(runner) = on_save(&game, "Toni", then) else { return };
        assert_eq!(
            text(&runner, 9920),
            "ANSTOSS 3 - Gegenüberstellung 1.FC Union Berlin (unten) - VfL Wolfsburg (oben)"
        );
        let own = texts(&runner, &[5612, 5613, 5614, 5615, 5616, 5617]);
        assert_eq!(own, ["9", "20", "84", "84", "3.8", "6"]);
        let theirs = texts(&runner, &[1744, 1745, 1746, 1747, 1748, 1749]);
        assert_eq!(theirs, ["8", "11", "45", "68", "5.7", "8"]);
        let parts = texts(&runner, &[1730, 5661, 1732, 1733, 1734, 1735, 1736, 1737]);
        assert_eq!(parts, ["9.5", "31.1", "34.4", "22.9", "9.2", "27.4", "38.2", "23.4"]);
        // Of the save's three pairs one marker has left the eleven.
        assert_eq!(runner.host.view().lines.len(), 2);
        // The original's moves: a defender onto a striker, a defender onto
        // a midfielder, two midfielders onto midfielders, a click on the
        // first defender; a defender forward at set pieces, the strikers
        // no longer marked; away to the line-up and back.
        let then = format!(
            "{then}\nnow drag at 143 404 to 379 214\nnow drag at 379 404 to 63 158\n\
             now drag at 261 351 to 203 179\nnow drag at 319 299 to 319 179\n\
             now click at 143 404\nnow click 5619\nnow click at 560 376\nnow click 5662\n\
             now choose 9930 0\nuntil dialog 147\nnow choose 9930 1\nuntil dialog 78"
        );
        let runner = on_save(&game, "Toni", &then).unwrap();
        assert_eq!(runner.host.view().lines.len(), 2);
        let state = runner.host.state();
        let team = &state.records.as_ref().unwrap().teams[84];
        // Moscardo marks Majer, Haberer Svanberg.
        assert_eq!(team.markers()[..3], [8378, 0, 3275]);
        assert_eq!(team.marked()[..3], [1616, 0, 1617]);
        assert_eq!(team.set_piece(), 3262);
        assert_eq!(team.passing(), 0x609b & !0x4000);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_action_areas_follow_their_handles() {
        let game = game();
        // As the original took them: the areas, Skov picked, his bottom
        // right handle out, his top handle up; away and back.
        let then = "now choose 9930 1\nuntil dialog 78\nnow choose 5618 1\nnow click at 93 248\n\
                    now drag at 166 284 to 200 320\nnow drag at 107 211 to 107 150\n\
                    now choose 9930 0\nuntil dialog 147\nnow choose 9930 1\nuntil dialog 78";
        let Some(runner) = on_save(&game, "Toni", then) else { return };
        let view = runner.host.view();
        let area = crate::screens::control(&game.dialogs[&78], 9921).unwrap();
        // The man-marking again, as every opening.
        assert!(view.controls[area].hidden && view.shades.is_empty());
        let state = runner.host.state();
        let team = &state.records.as_ref().unwrap().teams[84];
        assert_eq!(team.areas()[5], [79, 98, 107, 72]);
        assert_eq!(team.areas()[1], [79, 42, 79, 42]);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_short_training_keeps_what_is_set() {
        let game = game();
        // On the week's last day: a template, a player in individual
        // training, the camp; a past day refused; away and back.
        let then = "now choose 9930 4\nuntil dialog 346\nnow choose 5035 4\nnow click 4427\n\
                    now row 4427 3\nnow click 4425\nnow click 3831\nuntil dialog 380\nnow press 4731\n\
                    until dialog 346\nnow choose 9930 0\nuntil dialog 147\nnow choose 9930 4\n\
                    until dialog 346";
        let Some(runner) = on_save(&game, "Toni", then) else { return };
        assert!(runner.log.iter().any(|line| line == "screen 380"));
        let dialog = &game.dialogs[&346];
        let view = runner.host.view();
        let text = |id: u16| view.text(crate::screens::control(dialog, id).unwrap()).to_owned();
        assert_eq!(text(3831), "Spaziergang");
        assert_eq!((text(3840), text(3842)), (String::new(), "Spiel".to_owned()));
        let combo = crate::screens::control(dialog, 4427).unwrap();
        assert_eq!(view.combo(combo).and_then(|c| c.selected), Some(3));
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        // "ES/Fähigkeiten VORB": training matches all week, from today.
        let plan = records.extras[records.human_slot(0).unwrap()].plan();
        assert_eq!((&plan[..3], &plan[18..21]), (&[26, 26, 26][..], &[33, 33, 33][..]));
        let week = records.humans().next().unwrap().week(62 / 7);
        assert!(week.template == 5 && week.player != 0);
        assert_ne!(records.teams[84].training_flags() & a3_core::training::CAMP, 0);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_long_and_individual_training_keep_what_is_set() {
        let game = game();
        // On a week's first day: an intensity stepped, the button of those
        // who did not play; a template for the week, the camp's food a
        // notch down, the training secret; away.
        let then = "now click bar 5\nuntil dialog 347\nnow cell 3298 1 8\nnow press 4785\n\
                    now click bar 3\nuntil dialog 345\nnow choose 5035 2\n\
                    now click at 134 130\nnow press 5344\nnow click bar 0\nuntil dialog 147";
        let Some(runner) = on_save(&game, "Probe1", then) else { return };
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let manager = records.humans().next().unwrap();
        let team = records.team(manager.country(), manager.team()).unwrap();
        // 0x94 is 0, 1, 1, 2; the last one stepped round to 0.
        assert_eq!(team.intensity_states(), 0x14);
        assert_ne!(team.passing() & a3_core::training::SECRET, 0);
        let slot = usize::from(team.human()) - 1;
        let kept = records.individual[slot].entries().iter().filter(|e| e.id != 0).count();
        assert_eq!(kept, usize::from(team.player_count()).min(a3_core::individual::ENTRIES));
        let week = usize::from(records.globals.day()) / 7;
        assert_eq!(manager.week(week).template, 3);
        assert_eq!(manager.camp_rules()[0], 3);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_double_click_on_a_week_asks_which_player_to_watch() {
        let game = game();
        // Week 30 of the season, mid-week: on the page of weeks the double
        // click of week 36 asks whom to watch that week (281) — the list the
        // short training's combo holds. A row names the player, "keiner" and
        // the OK store nobody, a cancellation keeps him, and a double click
        // on a week already over asks nothing.
        let now = on_save(&game, "Probe1", "").unwrap();
        let week = now.host.state().season.as_ref().unwrap().day / 7;
        let (open, past) = (week + 6, week - 3);
        let page = format!("now click bar 3\nuntil dialog 345\nnow choose 4403 {}", open / 25);
        let dbl = |row: usize| format!("now dbl row 4407 {row}\nuntil dialog 281");
        let asked = format!("{page}\n{}", dbl(open % 25));
        let picked = format!("{asked}\nnow row 3827 2\nuntil dialog 345");
        let watch = |answer: &str| format!("{picked}\n{}\n{answer}", dbl(open % 25));
        let player = |runner: &Runner| {
            runner.host.state().records.as_ref().unwrap().humans().next().unwrap().week(open).player
        };
        let before = player(&now);
        let rows = |runner: &Runner, dialog: u16, id: u16| {
            let c = crate::screens::control(&game.dialogs[&dialog], id).unwrap();
            runner.host.view().list(c).unwrap().rows.clone()
        };
        let runner = on_save(&game, "Probe1", &asked).unwrap();
        assert_eq!(runner.host.view().id, 281, "{}", runner.log.join("\n"));
        let asked_rows = rows(&runner, 281, 3827);
        assert_eq!(asked_rows[0], game.text(1091));
        let shown = on_save(&game, "Probe1", "now click bar 4\nuntil dialog 346").unwrap();
        let combo = crate::screens::control(&game.dialogs[&346], 4427).unwrap();
        let items = shown.host.view().combo(combo).unwrap().items.clone();
        assert_eq!(asked_rows, items);
        let named = asked_rows.get(2).map_or(String::new(), |r| r.split('\t').next().unwrap().into());
        let runner = on_save(&game, "Probe1", &picked).unwrap();
        let chosen = player(&runner);
        assert_ne!(chosen, 0);
        let listed = rows(&runner, 345, 4407).remove(open % 25);
        assert_eq!(listed.split('\t').nth(2), Some(named.as_str()));
        let runner = on_save(&game, "Probe1", &watch("now cancel")).unwrap();
        assert_eq!((player(&runner), seen(&runner, 281)), (chosen, 2), "{}", runner.log.join("\n"));
        let runner = on_save(&game, "Probe1", &watch("now row 3827 0")).unwrap();
        assert_eq!(player(&runner), 0);
        let runner = on_save(&game, "Probe1", &watch("now ok")).unwrap();
        assert_eq!(player(&runner), 0);
        let over = format!("now dbl row 4407 {}", past % 25);
        let runner = on_save(&game, "Probe1", &format!("{page}\n{over}")).unwrap();
        assert_eq!((player(&runner), seen(&runner, 281)), (before, 0), "{}", runner.log.join("\n"));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_cleared_plan_leaves_the_current_week_and_the_players() {
        let game = game();
        // Probe1 mid-week: the current week's and the next two templates
        // chosen, the next week's player watched. The strip's last item asks
        // whether the plan may go (384); cancelled all stays, and with OK the
        // weeks ahead of the current one stand empty — that week's template
        // and every week's player among them.
        let now = on_save(&game, "Probe1", "").unwrap();
        let cur = now.host.state().season.as_ref().unwrap().day / 7;
        let weeks = |runner: &Runner| {
            let state = runner.host.state();
            let manager = state.records.as_ref().unwrap().humans().next().unwrap();
            [cur, cur + 1, cur + 2].map(|w| (manager.week(w).template, manager.week(w).player))
        };
        let row = (cur + 1) % 25;
        let fill = "now click bar 3\nuntil dialog 345\nnow choose 5035 4\nnow choose 5035 7";
        let asked = format!(
            "{fill}\nnow dbl row 4407 {row}\nuntil dialog 281\nnow row 3827 2\nuntil dialog 345\n\
             now choose 5652 5\nuntil dialog 384"
        );
        let before = weeks(&now);
        let kept = on_save(&game, "Probe1", &format!("{asked}\nnow cancel")).unwrap();
        let filled = weeks(&kept);
        assert_eq!((filled[0].0, filled[1].0), (5, 8), "{}", kept.log.join("\n"));
        assert_ne!(filled[1].1, 0, "{}", kept.log.join("\n"));
        assert_eq!(filled[2], before[2], "{}", kept.log.join("\n"));
        let runner = on_save(&game, "Probe1", &format!("{asked}\nnow ok")).unwrap();
        assert_eq!(
            weeks(&runner),
            [(5, filled[0].1), (0, filled[1].1), (0, filled[2].1)],
            "{}",
            runner.log.join("\n")
        );
        let list = crate::screens::control(&game.dialogs[&345], 4407).unwrap();
        let rows = runner.host.view().list(list).unwrap().rows.clone();
        let blank = |week: usize| rows[week % 25].split('\t').nth(1).unwrap_or_default().is_empty();
        assert!(!blank(cur) && blank(cur + 1) && blank(cur + 2), "{}", runner.log.join("\n"));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_strip_writes_reads_and_puts_back_the_training_weeks() {
        let game = game();
        // Probe1: the strip's first item asks (241) and then writes the
        // manager's 18 week templates to `plan\*.Woc`, the second asks (242)
        // and reads them back from a file the select lists, the third asks
        // (243) and puts the game's own weeks in without a select. Cancelled,
        // nothing happens at all.
        let opened = "now click bar 3\nuntil dialog 345";
        let file = game.dir.join("plan/Nachtwache.Woc");
        let kept = on_save(
            &game,
            "Probe1",
            &format!("{opened}\nnow choose 5652 0\nuntil dialog 241\nnow cancel"),
        )
        .unwrap();
        assert_eq!((seen(&kept, 241), seen(&kept, 506)), (1, 0), "{}", kept.log.join("\n"));
        assert!(!file.exists(), "{}", kept.log.join("\n"));

        // What the file holds is the block of templates as the manager keeps
        // it; it is deleted again before the asserts, so the install stays
        // clean either way.
        let saved = format!(
            "{opened}\nnow choose 5652 0\nuntil dialog 241\nnow ok\nuntil dialog 506\n\
             now type 6361 Nachtwache\nnow ok\nuntil dialog 345"
        );
        let runner = on_save(&game, "Probe1", &saved).unwrap();
        let block = std::fs::read(&file);
        std::fs::remove_file(&file).unwrap();
        assert_eq!((seen(&runner, 241), seen(&runner, 506)), (1, 1), "{}", runner.log.join("\n"));
        let templates = a3_core::training::week_templates(&block.unwrap());
        let state = runner.host.state();
        assert_eq!(templates, state.season.as_ref().unwrap().manager().unwrap().templates);

        // Reading: the name comes from the list's row, and after the select
        // closed the names stand in the strip again. The install's plan
        // folder holds one file of weeks, "Stimmspaz V1.2.Woc".
        let listed = crate::screens::popups::plan_files(&game.dir, game.text(3470));
        if listed.first().is_none_or(|(name, _)| name != "Stimmspaz V1.2.Woc") {
            eprintln!("no Stimmspaz V1.2.Woc in the plan folder; the oracle's copy has it");
            return;
        }
        let load = format!(
            "{opened}\nnow choose 5652 1\nuntil dialog 242\nnow ok\nuntil dialog 506\n\
             now row 5261 0\nnow ok\nuntil dialog 345"
        );
        let runner = on_save(&game, "Probe1", &load).unwrap();
        let want = a3_core::training::week_templates(
            &std::fs::read(game.dir.join("plan/Stimmspaz V1.2.Woc")).unwrap(),
        );
        let names = want.iter().map(|t| t.name.as_str()).collect::<Vec<_>>();
        let loaded = runner.host.state();
        let manager = loaded.records.as_ref().unwrap().humans().next().unwrap();
        let kept = manager.templates();
        assert_eq!(kept.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), names);
        let combo = crate::screens::control(&game.dialogs[&345], 5035).unwrap();
        let labels = runner.host.view().options(combo).unwrap().labels.clone();
        assert_eq!(labels.iter().map(String::as_str).collect::<Vec<_>>(), names);

        // The game's own weeks, straight out of its program, with no select
        // between the asking and the weeks.
        let restore =
            format!("{opened}\nnow choose 5652 2\nuntil dialog 243\nnow ok\nuntil dialog 345");
        let runner = on_save(&game, "Probe1", &restore).unwrap();
        let asked = (seen(&runner, 243), seen(&runner, 506));
        assert_eq!(asked, (1, 0), "{}", runner.log.join("\n"));
        let back = runner.host.state();
        assert_eq!(
            back.season.as_ref().unwrap().manager().unwrap().templates,
            game.rules.training.originals
        );
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_strip_writes_and_reads_the_season_plan() {
        let game = game();
        // Probe1: the strip's fourth item asks (241) and writes the season's
        // fifty plan entries as one 2900-byte block to `plan\*.pla`, the
        // fifth asks in the plan's own words (242) and reads them back from
        // a file the select lists — from the current week on, and without
        // their done flags and players. Cancelled, nothing happens.
        let now = on_save(&game, "Probe1", "").unwrap();
        let day = now.host.state().season.as_ref().unwrap().day;
        let cur = day / 7;
        if !crate::screens::popups::plan_files(&game.dir, game.text(3468)).is_empty() {
            eprintln!("a .pla already lies in the plan folder; the oracle's copy has none");
            return;
        }
        let opened = "now click bar 3\nuntil dialog 345";
        let file = game.dir.join("plan/Nachtwache.pla");
        let kept = on_save(
            &game,
            "Probe1",
            &format!("{opened}\nnow choose 5652 3\nuntil dialog 241\nnow cancel"),
        )
        .unwrap();
        assert_eq!((seen(&kept, 241), seen(&kept, 506)), (1, 0), "{}", kept.log.join("\n"));
        assert!(!file.exists(), "{}", kept.log.join("\n"));

        // The next three weeks take a template, so the file has an entry to
        // find again, and then the save runs; the file goes before the
        // asserts, so the install stays clean either way.
        let saved = format!(
            "{opened}\nnow choose 5035 2\nnow choose 5035 2\nnow choose 5035 2\n\
             now choose 5652 3\nuntil dialog 241\nnow ok\nuntil dialog 506\n\
             now type 6361 Nachtwache\nnow ok\nuntil dialog 345"
        );
        let runner = on_save(&game, "Probe1", &saved).unwrap();
        let bytes = std::fs::read(&file).expect("the save wrote no .pla");
        std::fs::remove_file(&file).unwrap();
        assert_eq!((seen(&runner, 241), seen(&runner, 506)), (1, 1), "{}", runner.log.join("\n"));
        let entries = a3_core::training::read_plan_file(&bytes).unwrap();
        assert_eq!((bytes.len(), entries.len()), (2900, 50));
        // The block is the manager's weeks as they stand, entry for entry —
        // a chosen week brings its number only, not the template's name, to
        // its entry, which is what the original's choose does as well.
        let state = runner.host.state();
        let weeks = &state.season.as_ref().unwrap().manager().unwrap().weeks;
        assert_eq!(&a3_core::training::plan_file(weeks), &bytes);
        assert_eq!((entries[cur + 2].template, entries[cur + 1].template), (3, 3));

        // Reading it back: the week took another template and a watched
        // player first; the file's entry comes over both, minus the player.
        let (page, row) = ((cur + 2) / 25, (cur + 2) % 25);
        let load = format!(
            "{opened}\nnow choose 4403 {page}\nnow row 4407 {row}\nnow choose 5035 5\n\
             now dbl row 4407 {row}\nuntil dialog 281\nnow row 3827 2\nuntil dialog 345\n\
             now choose 5652 4\nuntil dialog 242\nnow ok\nuntil dialog 506\n\
             now row 5261 0\nnow ok\nuntil dialog 345"
        );
        std::fs::write(&file, &bytes).unwrap();
        let runner = on_save(&game, "Probe1", &load).unwrap();
        std::fs::remove_file(&file).unwrap();
        assert_eq!((seen(&runner, 242), seen(&runner, 506)), (1, 1), "{}", runner.log.join("\n"));
        let state = runner.host.state();
        let entry = state.records.as_ref().unwrap().humans().next().unwrap().week(cur + 2);
        assert_eq!((entry.template, entry.player, entry.done), (3, 0, false), "{}", runner.log.join("\n"));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_co_trainer_settings_stand_and_go() {
        let game = game();
        // Probe1 has a co-trainer: the corner button 4417 in the long-term
        // training opens his settings (391).  Abbruch leaves everything as
        // it was; OK takes the box, the days and a Schwerunkt off — and
        // with the current week still unplanned, the co-trainer plans it
        // himself.
        let opened = "now click bar 3\nuntil dialog 345";
        let now = on_save(&game, "Probe1", "").unwrap();
        let state = now.host.state();
        let season = state.season.as_ref().unwrap();
        let key = season.manager().unwrap().club.unwrap();
        let club = season.clubs.get(&key).unwrap();
        let (was_days, was_flags) = (club.co_training.days, club.line_up_flags);
        let cur = season.day / 7;
        let was_plan = season.manager().unwrap().weeks.get(cur).map_or(0, |w| w.template);
        assert_eq!((was_flags & 1, was_days), (0, 5), "{}", now.log.join("\n"));

        let kept = on_save(
            &game,
            "Probe1",
            &format!("{opened}\nnow press 4417\nuntil dialog 391\nnow spin 4796 up\nnow cancel\nuntil dialog 345"),
        )
        .unwrap();
        assert_eq!(seen(&kept, 391), 1, "{}", kept.log.join("\n"));
        let state = kept.host.state();
        let club = state.season.as_ref().unwrap().clubs.get(&key).unwrap();
        assert_eq!(club.co_training.days, was_days, "{}", kept.log.join("\n"));
        assert_eq!((club.line_up_flags, was_flags), (was_flags, was_flags));

        let took = on_save(
            &game,
            "Probe1",
            &format!(
                "{opened}\nnow press 4417\nuntil dialog 391\nnow click 4925\n\
                 now spin 4796 up\nnow slide 4786 0\nnow ok\nuntil dialog 345"
            ),
        )
        .unwrap();
        assert_eq!(seen(&took, 391), 1, "{}", took.log.join("\n"));
        let state = took.host.state();
        let season = state.season.as_ref().unwrap();
        let club = season.clubs.get(&key).unwrap();
        assert_eq!(club.line_up_flags & 1, 1, "{}", took.log.join("\n"));
        assert_eq!(club.co_training.days, 6, "{}", took.log.join("\n"));
        assert_eq!(club.co_training.focus[0], 0, "{}", took.log.join("\n"));
        let records = state.records.as_ref().unwrap();
        let extra = records
            .extras
            .get(records.human_slot(season.current).expect("a human manager"))
            .expect("the human's extra data");
        assert_eq!((extra.line_up_flags() & 1, extra.co_training_days()), (1, 6));
        if was_plan <= 0 {
            let after = season.manager().unwrap().weeks.get(cur).map_or(0, |w| w.template);
            assert_ne!(after, 0, "{}", took.log.join("\n"));
        }
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_opponents_squad_stands_over_the_comparison() {
        let game = game();
        // The comparison's "Gegner" (button 5664 of 78) puts up the
        // opponent's own squad (476); the OK brings it down again.
        let Some(run) = on_save(
            &game,
            "Toni",
            "now click bar 1\nuntil dialog 78\nnow press 5664\nuntil dialog 476\n\
             now ok\nuntil dialog 78",
        ) else {
            return;
        };
        assert_eq!(seen(&run, 476), 1, "{}", run.log.join("\n"));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_right_click_on_the_opponent_shows_his_info() {
        let game = game();
        // A right click on the opponent's keeper on the comparison (box
        // 5631 of 78) puts up his info (399), as on one's own eleven.
        let Some(run) = on_save(
            &game,
            "Toni",
            "now click bar 1\nuntil dialog 78\nnow rclick 5631\nuntil dialog 399",
        ) else {
            return;
        };
        assert_eq!(seen(&run, 399), 1, "{}", run.log.join("\n"));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn an_opponents_squad_row_shows_his_info() {
        let game = game();
        // A right click on a row of the opponent's squad (list 5261 of
        // 476) puts up his one-item menu and his info (399), as the
        // shared `CListBoxSpielerInfo` has it.
        let Some(run) = on_save(
            &game,
            "Toni",
            "now click bar 1\nuntil dialog 78\nnow press 5664\nuntil dialog 476\n\
             now rclick 5261 row 0\nnow choose 9950 0\nuntil dialog 399",
        ) else {
            return;
        };
        assert_eq!(seen(&run, 399), 1, "{}", run.log.join("\n"));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_calendar_books_and_the_planner_shows() {
        let game = game();
        // September: the family day dragged onto the 10th, the club party
        // of the 15th clicked away; then the week planner.
        let then = "now click bar 6\nuntil dialog 348\nnow drag at 544 355 to 244 180\n\
                    now cell 4459 14 4\nnow click bar 7\nuntil dialog 413";
        let Some(runner) = on_save(&game, "Toni", then) else { return };
        let state = runner.host.state();
        let calendar = &state.records.as_ref().unwrap().calendars[0];
        let booked = a3_core::year::day(calendar, 65);
        assert_eq!((booked.kind(), booked.value()), (a3_core::year::EVENT, 0));
        assert_eq!(a3_core::year::day(calendar, 70).kind(), 0);
        let dialog = &game.dialogs[&413];
        let view = runner.host.view();
        let rows = crate::screens::control(dialog, 5034).unwrap();
        let labels = &view.options(rows).unwrap().labels;
        assert_eq!(labels[1], "Vertragsverhandlung\nTom Rothe");
        assert_eq!(labels[6], "Einzelgespräch\nRobert Skov");
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_balance_sheet_shows_the_clubs_money() {
        let game = game();
        // As the original showed it; a club of the third league may not go
        // to the stock market.
        let then = "now click bar 8\nuntil dialog 349";
        let Some(runner) = on_save(&game, "Probe1", then) else { return };
        let view = runner.host.view();
        let dialog = &game.dialogs[&349];
        let text =
            |id: u16| view.text(crate::screens::control(dialog, id).unwrap()).trim().to_owned();
        assert_eq!([text(4463), text(4464), text(4465)], ["663.765 €", "0 €", "0 €"]);
        assert_eq!([text(4472), text(4473), text(4474)], ["663.765 €", "2.50 Mio €", "3.16 Mio €"]);
        assert_eq!([text(4469), text(4470), text(4471)], ["15%", "12.50%", "(1.70 Mio €)"]);
        assert_eq!([text(4466), text(4467)], ["0 €", "663.765 €"]);
        // The forecasts, the tickets by the attendance model among them.
        assert_eq!([text(4468), text(4475)], ["1.42 Mio €", "- 2.49 Mio €"]);
        assert_eq!(
            [text(5510), text(5511), text(5512), text(5513)],
            ["Verein", "---", "---", "---"]
        );
        let then = "now click bar 8\nuntil dialog 349\nnow press 6152\nuntil dialog 380";
        let runner = on_save(&game, "Probe1", then).unwrap();
        let view = runner.host.view();
        let notice = view.text(crate::screens::control(&game.dialogs[&380], 4730).unwrap());
        assert_eq!(notice, game.text(4582));
    }

    /// Going public, on A with a profit in each of the last three seasons
    /// patched in: 645.000 shares offered for 25.80 Mio €, as the original
    /// offers them; taken, at the random numbers poked into the original
    /// before its OK, the club stands on the market as the original put it
    /// there (peeked: drift 1, 1610 shares to be had, 219.45 € high, low
    /// and current), an AG, the proceeds in the bank.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_bilanz_takes_the_club_public() {
        let Some(dir) = std::env::var_os("ANSTOSS3_DIR").map(PathBuf::from) else { return };
        if !dir.join("saves/A.an3").exists() && !dir.join("../saves-kept/A.an3").exists() {
            return;
        }
        let root = patched(&dir, "A", "going-public", |records| {
            let team = usize::from(records.managers[0].team());
            for at in [0x908, 0x910, 0x918] {
                records.teams[team].put::<i64>(at, 0);
            }
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let ask = "now click bar 8\nuntil dialog 349\nnow press 6152\nuntil dialog 384";
        let runner = on_save(&game, "going-public", ask).unwrap();
        let view = runner.host.view();
        let question = view.text(control(&game.dialogs[&384], 4738).unwrap()).to_owned();
        let offered = question.contains("645.000 Aktien") && question.contains("25.80 Mio €");
        assert!(offered, "{question}");

        let taken = format!("{ask}\nnow random 12345 67890\nnow press 4741\nuntil dialog 349");
        let runner = on_save(&game, "going-public", &taken).unwrap();
        let dialog = &game.dialogs[&349];
        let view = runner.host.view();
        let text = |id: u16| view.text(control(dialog, id).unwrap()).trim().to_owned();
        assert_eq!([text(5510), text(5511), text(5512), text(5513)], [
            "AG",
            "645.000",
            "219,44 €",
            "141.54 Mio €"
        ]);
        assert_eq!(text(4463), "31.72 Mio €");
        assert!(view.disabled(control(dialog, 6152).unwrap()));
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let market = records.block("0x8be408").unwrap();
        assert_eq!(i32::from_le_bytes(market[..4].try_into().unwrap()), 204);
        let entry = &market[0x14 + 203 * 0x108..0x14 + 204 * 0x108];
        let price = 0x435b7333u32.to_le_bytes();
        let mut expected = vec![0u8; 0x108];
        expected[..17].copy_from_slice(b"1.FC Union Berlin");
        for (at, bytes) in [
            (0x1c, &14i16.to_le_bytes()[..]),
            (0x1e, &84i16.to_le_bytes()),
            (0x20, &645_000i32.to_le_bytes()),
            (0x24, &50i16.to_le_bytes()),
            (0x26, &1i16.to_le_bytes()),
            (0x28, &1610i32.to_le_bytes()),
            (0x30, &price),
            (0x34, &price),
            (0x38 + 50 * 4, &price),
        ] {
            expected[at..at + bytes.len()].copy_from_slice(bytes);
        }
        assert_eq!(entry, &expected[..]);
        let team = &records.teams[usize::from(records.managers[0].team())];
        assert_eq!(team.legal_form() & 1, 1);
        assert_eq!(team.get::<u32>(0x308) & 0x8000, 0x8000);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_deposit_is_made_and_dissolved() {
        let game = game();
        let deposits = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let manager = records.humans().next().unwrap();
            let team = records.team(manager.country(), manager.team()).unwrap();
            a3_core::finances::deposits_of(team, records.globals.fund_prices())
        };
        // 120.000 € with the third partner: a step up from 100.000 €.
        let open = "now click bar 8\nuntil dialog 349\nnow press 4461\nuntil dialog 16";
        let then = format!("{open}\nnow spin 1278 up\nnow choose 6050 2\nnow press 1276");
        let Some(runner) = on_save(&game, "Probe1", &then) else { return };
        let made = deposits(&runner);
        assert_eq!(made.len(), 1);
        assert_eq!((made[0].slot, made[0].partner, made[0].cost), (0, 2, 120_000));
        let then = format!("{then}\nnow row 1266 0\nnow press 1277\nnow press 1\nuntil dialog 349");
        let runner = on_save(&game, "Probe1", &then).unwrap();
        assert!(deposits(&runner).is_empty());
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_loan_is_taken_and_paid_back() {
        let game = game();
        let loans = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let manager = records.humans().next().unwrap();
            let team = records.team(manager.country(), manager.team()).unwrap();
            a3_core::finances::loans_of(team)
        };
        // 30.000 € for two years, not to be paid back early; then 10.000 €
        // for a year, which may.
        let then = "now click bar 8\nuntil dialog 349\nnow press 4462\nuntil dialog 335\n\
                    now spin 4363 up\nnow spin 4363 up\nnow choose 6061 4\nnow press 4361\n\
                    now choose 6061 0\nnow slide 4363 1\nnow press 4361";
        let Some(runner) = on_save(&game, "Probe1", then) else { return };
        let taken = loans(&runner);
        assert_eq!(taken.len(), 2);
        assert_eq!((taken[0].principal, taken[0].term, taken[0].repayable), (30_000, 1, false));
        assert_eq!((taken[1].principal, taken[1].term, taken[1].repayable), (10_000, 0, true));
        assert_eq!(taken[0].rates[0] - taken[1].rates[0], -150);
        // The fixed one cannot be paid back; the other all at once.
        let fixed = format!("{then}\nnow row 4299 0\nnow press 4362\nuntil dialog 380");
        assert_eq!(loans(&on_save(&game, "Probe1", &fixed).unwrap()).len(), 2);
        let then = format!(
            "{then}\nnow row 4299 1\nnow press 4362\nuntil dialog 138\n\
                            now press 2457\nuntil dialog 335"
        );
        let left = loans(&on_save(&game, "Probe1", &then).unwrap());
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].principal, 30_000);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_squad_is_listed_by_position() {
        let game = game();
        let open = "now click bar 10\nuntil dialog 218\nnow choose 5042 0\nuntil dialog 216";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let view = runner.host.view();
        let dialog = &game.dialogs[&216];
        let list = view.list(crate::screens::control(dialog, 3298).unwrap()).unwrap();
        // By main position, the keepers first; fifteen columns a row.
        let positions: Vec<&str> =
            list.rows.iter().map(|r| r.split('\t').nth(1).unwrap()).collect();
        assert_eq!(positions[0], "TO");
        assert!(list.rows.iter().all(|r| r.split('\t').count() == 15));
        let line = view.text(crate::screens::control(dialog, 3299).unwrap());
        assert!(
            line.starts_with(&format!("Derzeit verfügen Sie über {} Spieler", list.rows.len()))
        );
        // The fine's mode on the first row opens its dialog.
        let then = format!("{open}\nnow choose 5657 4\nnow row 3298 0\nuntil dialog 85");
        on_save(&game, "Probe1", &then).unwrap();
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_transfer_list_filters_and_watches() {
        let game = game();
        let open = "now click bar 10\nuntil dialog 218\nnow choose 6371 0\nuntil dialog 287";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let dialog = &game.dialogs[&287];
        let list = |runner: &Runner| {
            let view = runner.host.view();
            view.list(crate::screens::control(dialog, 3937).unwrap()).unwrap().clone()
        };
        let all = list(&runner);
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let listed = a3_core::transfers::listings(records);
        assert_eq!(all.rows.len(), listed.iter().filter(|e| e.id != 0).count());
        let count =
            runner.host.view().text(crate::screens::control(dialog, 3936).unwrap()).to_owned();
        assert!(count.contains(&listed.len().to_string()));
        drop(state);
        // Strength 13 and up: nobody; the least strength is kept on OK.
        let strong = format!("{open}\nnow spin 5660 up{}", "\nnow spin 5660 up".repeat(12));
        assert!(list(&on_save(&game, "Probe1", &strong).unwrap()).rows.is_empty());
        let kept = format!("{strong}\nnow press 3938\nuntil dialog 218");
        let runner = on_save(&game, "Probe1", &kept).unwrap();
        let state = runner.host.state();
        let manager = state.records.as_ref().unwrap().managers[0].least_strength();
        assert_eq!(manager, 13);
        drop(state);
        // A player watched turns green after the next refill.
        let id = all.keys[0] as u16;
        let watch = format!(
            "{open}\nnow rclick 3937 row 0\nnow choose 9950 1\nnow choose 5658 1\nnow choose 5658 0"
        );
        let runner = on_save(&game, "Probe1", &watch).unwrap();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        assert!(a3_core::transfers::watched(records, 0).contains(&id));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_agreed_transfers_are_read_by_their_pages() {
        let game = game();
        let Some(runner) = on_save(&game, "Toni", "") else { return };
        let state = runner.host.state();
        let agreed = state.records.as_ref().unwrap().agreed();
        // Union buys Reis from Girona; Ilic and Markgraf come back from
        // their loans (KADER-NAECHSTE-VERLIEHEN.md §1.2).
        let union = (14, 84);
        let ids: Vec<(u16, bool)> = agreed.iter().map(|a| (a.player, a.flagged)).collect();
        assert_eq!(ids, [(7897, false), (3281, true), (3267, true)]);
        assert!(agreed.iter().all(|a| a.buyer == union && a.fee == 0));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn next_seasons_squad_marks_and_refills() {
        let game = game();
        let open = "now click bar 10\nuntil dialog 218\nnow choose 5042 1\nuntil dialog 219";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let dialog = &game.dialogs[&219];
        let list = runner
            .host
            .view()
            .list(crate::screens::control(dialog, 3318).unwrap())
            .unwrap()
            .clone();
        assert!(list.rows.iter().all(|r| r.split('\t').count() == 14));
        // By main position: the keepers first.
        assert_eq!(list.rows[0].split('\t').nth(1), Some("TO"));
        // A click marks a player and fills the list anew: nothing selected.
        let then = format!("{open}\nnow row 3318 2");
        let runner = on_save(&game, "Probe1", &then).unwrap();
        let list = runner
            .host
            .view()
            .list(crate::screens::control(dialog, 3318).unwrap())
            .unwrap()
            .clone();
        assert_eq!(list.selected, None);
        // The players out on loan: as many rows as loans coming home.
        let then = "now click bar 10\nuntil dialog 218\nnow choose 5042 5\nuntil dialog 300";
        on_save(&game, "Probe1", then).unwrap();
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_contract_is_offered_a_year_more() {
        let game = game();
        let open = "now click bar 10\nuntil dialog 218\nnow choose 5042 4\nuntil dialog 92";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let dialog = &game.dialogs[&92];
        let list = |runner: &Runner| {
            let view = runner.host.view();
            view.list(crate::screens::control(dialog, 1883).unwrap()).unwrap().clone()
        };
        // The players in their contract's last year, in the squad's order.
        let rows = list(&runner);
        assert_eq!(rows.rows.len(), 5);
        assert!(rows.rows.iter().all(|r| r.split('\t').count() == 13));
        assert!(rows.rows.iter().all(|r| r.ends_with("\t30.6.2026\tNein")));
        let id = rows.keys[0] as u16;
        // A click: he takes a year more, or declines; either way he is off
        // the list once the alert is gone.
        let offer =
            format!("{open}\nnow row 1883 0\nuntil dialog 380\nnow press 4731\nuntil dialog 92");
        let runner = on_save(&game, "Probe1", &offer).unwrap();
        assert_eq!(list(&runner).rows.len(), 4);
        let state = runner.host.state();
        let r = state.records.as_ref().unwrap().player(id).unwrap();
        assert!(r.contract() & 0xfff == 2027 || r.retiring() & 0x40 != 0);
        // Nobody the club holds an option on.
        let options = format!("{open}\nnow choose 1884 1");
        assert!(list(&on_save(&game, "Probe1", &options).unwrap()).rows.is_empty());
        // No youth of another club without a pre-contract.
        let poaching = "now click bar 10\nuntil dialog 218\nnow choose 6371 3\nuntil dialog 107";
        on_save(&game, "Probe1", poaching).unwrap();
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_youth_is_supported_and_signs_a_pre_contract() {
        let game = game();
        let open = "now click bar 10\nuntil dialog 218\nnow choose 5042 2\nuntil dialog 428";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let dialog = &game.dialogs[&428];
        let rows = |runner: &Runner, id: u16| {
            let view = runner.host.view();
            view.list(crate::screens::control(dialog, id).unwrap()).unwrap().rows.clone()
        };
        // Two B youths, no A or C youth.
        assert!(rows(&runner, 5264).is_empty() && rows(&runner, 5262).is_empty());
        let youths = rows(&runner, 5263);
        assert_eq!(youths.len(), 2);
        assert!(youths.iter().all(|r| r.split('\t').count() == 10));
        assert!(youths.iter().all(|r| r.ends_with("\tkein Vorvertrag")));
        // A click in "Förd." gives the first the group's support.
        let support = format!("{open}\nnow click at 397 209");
        let runner = on_save(&game, "Probe1", &support).unwrap();
        assert_eq!(rows(&runner, 5263)[0].split('\t').nth(7), Some("X"));
        // Anywhere else: the parents' pre-contract, then too young for more.
        let sign = format!(
            "{open}\nnow click at 100 209\nuntil dialog 325\nnow press 4190\nuntil dialog 428\n\
             now click at 100 209\nuntil dialog 380\nnow press 4731\nuntil dialog 428"
        );
        let runner = on_save(&game, "Probe1", &sign).unwrap();
        assert!(rows(&runner, 5263)[0].ends_with(" €"));
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let first =
            runner.host.view().list(crate::screens::control(dialog, 5263).unwrap()).unwrap().keys
                [0];
        let r = records.player(first as u16).unwrap();
        assert_eq!(r.contract() & 0xfff, records.globals.start_year() + 10);
        // The amateurs open from the same menu.
        let amateurs = "now click bar 10\nuntil dialog 218\nnow choose 5042 3\nuntil dialog 427";
        on_save(&game, "Probe1", amateurs).unwrap();
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_squads_friends_and_enemies_are_listed() {
        let game = game();
        let open = "now click bar 10\nuntil dialog 218\nnow choose 5042 0\nuntil dialog 216\n\
                    now press 6280\nuntil dialog 500";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let dialog = &game.dialogs[&500];
        let shown = |runner: &Runner| {
            let view = runner.host.view();
            let list = view.list(crate::screens::control(dialog, 3298).unwrap()).unwrap().clone();
            let titles = view.text(crate::screens::control(dialog, 6281).unwrap()).to_owned();
            (list, titles)
        };
        // Every player of the squad, his group, best friend and friends.
        let (list, friends) = shown(&runner);
        let squad =
            runner.host.state().season.as_ref().map_or(0, |s| {
                s.clubs.get(&s.manager().unwrap().club.unwrap()).unwrap().players.len()
            });
        assert_eq!(list.rows.len(), squad);
        assert!(list.rows.iter().all(|r| r.split('\t').count() == 4));
        // The enmities, under titles of their own.
        let enemies = format!("{open}\nnow choose 1884 1");
        let (list, titles) = shown(&on_save(&game, "Probe1", &enemies).unwrap());
        assert_eq!(list.rows.len(), squad);
        assert_ne!(titles, friends);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_stadium_is_expanded() {
        use a3_core::stadium;
        let game = game();
        let crane = "now click bar 11\nuntil dialog 464\nnow click at 311 145\nuntil dialog 463";
        let Some(runner) = on_save(&game, "Probe1", crane) else { return };
        let club = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let manager = records.humans().next().unwrap();
            let team = records.team(manager.country(), manager.team()).unwrap();
            let subs = stadium::subs(records, team.country(), u16::from(team.number())).unwrap();
            (team.get::<i64>(0x2d8 + 0x278), subs[0].clone())
        };
        let (costs, sub) = club(&runner);
        // Ten more standing places on the Gegentribüne are far beyond the
        // money: refused.
        let dear = format!("{crane}\nnow spin 5875 up\nnow press 1\nuntil dialog 380");
        on_save(&game, "Probe1", &dear).unwrap();
        // The right stand renovated a step: its cost booked, the stadium
        // carried, once the screen is left.
        let mut plan = sub.clone();
        plan.set_condition(3, sub.condition(3) - 1);
        let renovation = stadium::stand_cost(&plan, &sub, 3).renovation;
        assert!(renovation > 0);
        let renovate = format!(
            "{crane}\nnow choose 5915 3\nnow press 1\nuntil dialog 464\nnow click bar 0\nuntil dialog 147"
        );
        let (after, carried) = club(&on_save(&game, "Probe1", &renovate).unwrap());
        assert_eq!(after - costs, i64::from(renovation));
        assert_eq!(carried.condition(3), sub.condition(3) - 1);
        // The stadium seen from the screen and from the plan.
        let views = format!(
            "{crane}\nnow choose 5916 0\nuntil dialog 249\nnow press 1\nuntil dialog 463\n\
             now press 2\nuntil dialog 464\nnow choose 5852 0\nuntil dialog 249\nnow press 1\n\
             until dialog 464"
        );
        on_save(&game, "Probe1", &views).unwrap();
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_grounds_are_shown_and_their_view_kept() {
        let game = game();
        let open = "now click bar 11\nuntil dialog 464\nnow press 5861\nuntil dialog 466";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let dialog = &game.dialogs[&466];
        let text =
            |runner: &Runner, id| runner.host.view().text(control(dialog, id).unwrap()).to_owned();
        assert_eq!(text(&runner, 6291), "Kaufkosten:");
        assert!(text(&runner, 5966).starts_with("Apfelbaum\n"), "{}", text(&runner, 5966));
        // The club's objects, sorted, the first chosen, with its weeks.
        let club = format!("{open}\nnow choose 6471 1");
        let runner = on_save(&game, "Probe1", &club).unwrap();
        assert!(text(&runner, 5966).starts_with("Doping-Labor\n"), "{}", text(&runner, 5966));
        // Zoomed in and left, the grounds keep the zoom.
        let zoomed = format!("{open}\nnow press 5969\nnow press 1\nuntil dialog 464");
        let runner = on_save(&game, "Probe1", &zoomed).unwrap();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let manager = records.humans().next().unwrap();
        let team = records.team(manager.country(), manager.team()).unwrap();
        let grounds =
            a3_core::grounds::Grounds::of(records, team.country(), i16::from(team.number()))
                .unwrap();
        assert_eq!(grounds.view().zoom, 2);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_telephone_hires_a_lawyer_and_refuses_a_referee() {
        let game = game();
        let open = "now click bar 12\nuntil dialog 404";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        assert_eq!(runner.host.view().id, 404);
        let team = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let manager = records.humans().next().unwrap();
            let team = records.team(manager.country(), manager.team()).unwrap();
            (team.get::<i8>(0x5e0 + 0x278), team.get::<i16>(0x928))
        };
        assert_eq!(team(&runner), (0, 0));
        // The lawyer "Dr. Rentz", OK; the first referee of the country, OK.
        let then = format!(
            "{open}\nnow click at 442 210\nnow choose 2317 0\nnow ok\n\
             now click at 442 240\nnow row 6211 1\nnow ok"
        );
        let runner = on_save(&game, "Probe1", &then).unwrap();
        let (lawyer, referee) = team(&runner);
        assert_eq!(lawyer, 3);
        assert!(referee > 0, "{referee}");
        // The refusal waits as a headline for the next day's paper, which
        // puts it first in the list under today's date.
        {
            let mut guard = runner.host.state();
            let state = &mut *guard;
            let season = state.season.as_mut().unwrap();
            let item = season.headlines.items()[0].clone();
            assert_eq!(item.priority, 5);
            assert!(
                item.headline.contains("hat die Schnauze voll") || item.headline.contains("sauer auf"),
                "{item:?}"
            );
            let records = state.records.as_mut().unwrap();
            let before = a3_core::statistics::newspaper(records).len();
            season.headlines.publish(records);
            let paper = a3_core::statistics::newspaper(records);
            assert_eq!(paper.len(), (before + 1).min(20));
            assert_eq!(paper[0].lines, [item.headline, item.body]);
            assert!(season.headlines.items().is_empty());
        }
        // The aliens' office lists the foreigners.
        let runner = on_save(&game, "Probe1", &format!("{open}\nnow click at 442 302")).unwrap();
        let list = control(&game.dialogs[&492], 6212).unwrap();
        assert!(!runner.host.view().list(list).unwrap().rows.is_empty());
    }

    /// A co-trainer's record block.
    fn co_block<'a>(records: &'a a3_core::records::Records, id: i16) -> &'a [u8] {
        records.block(&format!("0xbe3920[{id}]")).unwrap()
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_telephone_hires_a_co_trainer_and_pays_off_the_old_one() {
        let game = game();
        let open = "now click bar 12\nuntil dialog 404";
        let market = |records: &a3_core::records::Records, b: u8| -> Vec<(i16, i16)> {
            let m = records.block(&format!("0x8e27a0[{b}]")).unwrap();
            let count = i16::from_le_bytes(m[0..2].try_into().unwrap()).max(0) as usize;
            (0..count.min(30))
                .map(|k| {
                    (
                        i16::from_le_bytes(m[2 + k * 4..4 + k * 4].try_into().unwrap()),
                        i16::from_le_bytes(m[4 + k * 4..6 + k * 4].try_into().unwrap()),
                    )
                })
                .collect()
        };
        // The list as it stands on the save's day, and the club's own coach.
        let list = control(&game.dialogs[&35], 1359).unwrap();
        let seen = format!("{open}\nnow choose 4975 0\nuntil dialog 35");
        let Some(before) = on_save(&game, "Probe1", &seen) else { return };
        let state = before.host.state();
        let records = state.records.as_ref().unwrap();
        let manager = records.humans().next().unwrap();
        let (country, number) = (manager.country(), manager.team());
        let team = records.team(country, number).unwrap();
        let old = team.get::<i16>(0xa2);
        let cand = i16::try_from(before.host.view().list(list).unwrap().keys[0]).unwrap();
        let b = records.countries.slot(usize::from(country));
        let before_market = market(records, b);
        assert!(before_market.iter().any(|&(id, _)| id == cand));
        assert_eq!(co_block(records, cand)[0xe], 0);
        let pay = i32::from_le_bytes(co_block(records, cand)[0x10..0x14].try_into().unwrap());
        let years = i32::from(co_block(records, cand)[0x15]).clamp(1, 5);
        let (old_pay, old_left, old_competence) = if old == 0 {
            (0, 0, 0)
        } else {
            let r = co_block(records, old);
            (
                i32::from_le_bytes(r[0x10..0x14].try_into().unwrap()),
                i32::from(r[0xe]),
                i16::from(r[4]),
            )
        };
        let staff = (team.get::<i32>(0x4f8), team.get::<i32>(0x4dc));
        use chrono::Datelike;
        let month = state.season.as_ref().unwrap().date().month();
        let severance = a3_core::trainers::severance(i64::from(old_pay), old_left, month);
        // An offer one step over what he asks: he takes it at once.
        let pos = a3_core::trainers::position(i64::from(pay) / 1000) + 1;
        let salary = a3_core::trainers::at_position(pos) * 1000;
        assert!(salary >= i64::from(pay), "{salary} against {pay}");
        let settle = if old == 0 {
            "until dialog 39".to_owned()
        } else {
            "until dialog 31\nnow ok\nuntil dialog 39".to_owned()
        };
        let then = format!(
            "{seen}\nnow row 1359 0\nuntil dialog 32\nnow slide 1350 {pos}\nnow press 1351\n\
             {settle}\nnow ok\nuntil dialog 404"
        );
        let runner = on_save(&game, "Probe1", &then).unwrap();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let team = records.team(country, number).unwrap();
        assert_eq!(team.get::<i16>(0xa2), cand);
        let r = co_block(records, cand);
        assert_eq!(i32::from_le_bytes(r[0x10..0x14].try_into().unwrap()), i32::try_from(salary).unwrap());
        assert_eq!(r[0xe], u8::try_from(years).unwrap());
        if old != 0 {
            // The old coach stands without a contract again, and the
            // severance sits on his two accounts.
            assert_eq!(co_block(records, old)[0xe], 0);
            assert_eq!(team.get::<i32>(0x4f8), staff.0.wrapping_add(severance as i32));
            assert_eq!(team.get::<i32>(0x4dc), staff.1.wrapping_add(severance as i32));
            let after = market(records, b);
            assert!(after.iter().any(|&(id, competence)| {
                id == old && competence == old_competence
            }));
        }
        // He himself left the market.
        let after = market(records, b);
        assert!(!after.iter().any(|&(id, _)| id == cand));
        assert_eq!(after.len(), before_market.len().saturating_sub(if old == 0 { 1 } else { 0 }));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_own_coach_extends_his_contract_or_refuses() {
        let game = game();
        let open = "now click bar 12\nuntil dialog 404";
        let shown = format!("{open}\nnow choose 4975 0\nuntil dialog 35");
        let own_of = |state: &a3_core::state::GameState| -> i16 {
            let records = state.records.as_ref().unwrap();
            let manager = records.humans().next().unwrap();
            records.team(manager.country(), manager.team()).unwrap().get::<i16>(0xa2)
        };
        // Probe1's coach of 1953: past the age, and more a co-trainer than
        // the playing manager is a head — he won't, whatever the money.
        let ask = format!("{shown}\nnow click 1361\nuntil dialog 36");
        let Some(runner) = on_save(&game, "Probe1", &ask) else { return };
        assert_eq!(seen(&runner, 36), 1);
        assert_eq!(seen(&runner, 38), 0);
        let notice = &game.dialogs[&36];
        let filled = runner.host.view().text(control(notice, 1364).unwrap()).to_owned();
        assert!(filled.contains("möchte seinen Vertrag derzeit nicht verlängern"), "{filled}");
        // Closing it leaves the market open and the contract untouched.
        let runner = on_save(&game, "Probe1", &format!("{ask}\nnow ok\nuntil dialog 35")).unwrap();
        assert_eq!(runner.host.view().id, 35);
        let state = runner.host.state();
        assert_eq!(co_block(state.records.as_ref().unwrap(), own_of(&state))[0xe], 0);
        // A younger one of modest competence agrees, and his contract runs
        // a year longer; the salary stays (§5.8).
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "Probe1", "co-extend", |records| {
            let index = records.human_slot(0).unwrap();
            let (country, number) =
                (records.managers[index].country(), records.managers[index].team());
            let own = records.team(country, number).unwrap().get::<i16>(0xa2);
            let r = records.block_mut(&format!("0xbe3920[{own}]")).unwrap();
            r[4] = 2;
            r[0xa..0xc].copy_from_slice(&1967i16.to_le_bytes());
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let before = on_save(&game, "co-extend", &shown).unwrap();
        let (own, left) = {
            let state = before.host.state();
            let records = state.records.as_ref().unwrap();
            let own = own_of(&state);
            (own, co_block(records, own)[0xe])
        };
        let year = before.host.state().season.as_ref().unwrap().year;
        let runner = on_save(&game, "co-extend", &format!("{shown}\nnow click 1361\nuntil dialog 38"))
            .unwrap();
        assert_eq!(seen(&runner, 38), 1);
        let agreed = &game.dialogs[&38];
        let filled = runner.host.view().text(control(agreed, 1371).unwrap()).to_owned();
        // The original names the year the season ends in plus the new years
        // before him: 2025/26 with one year added reads "bis 2027".
        let until = year + i32::from(left) + 2;
        assert!(filled.contains(&format!("bis {until}")), "{filled}");
        let runner =
            on_save(
                &game,
                "co-extend",
                // 38's one button is drawn with the cancel face.
                &format!("{shown}\nnow click 1361\nuntil dialog 38\nnow press 1372\nuntil dialog 35"),
            )
                .unwrap();
        assert_eq!(runner.host.view().id, 35);
        let state = runner.host.state();
        let r = co_block(state.records.as_ref().unwrap(), own);
        assert_eq!(r[0xe], left + 1);
        assert_eq!(i32::from_le_bytes(r[0x10..0x14].try_into().unwrap()), 182_400);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_reporter_makes_the_release_clause_public() {
        let game = game();
        let ask = "now click bar 12\nuntil dialog 404\nnow choose 4975 7\nuntil dialog 493";
        let items = control(&game.dialogs[&493], 6214).unwrap();
        // Probe1 has no clause: the item is off, and a click on it stays.
        let Some(runner) = on_save(&game, "Probe1", &format!("{ask}\nnow choose 6214 0")) else {
            return;
        };
        assert_eq!(runner.host.view().id, 493);
        assert!(runner.host.view().options(items).unwrap().dimmed[0]);
        // One with a clause at 420.000 € (§7.3).
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "Probe1", "reporter", |records| {
            let index = records.human_slot(0).unwrap();
            let manager = &mut records.managers[index];
            manager.put(0x178, 42u8);
            manager.set_popularity([30, 60, 38]);
            manager.set_confidence(75);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let runner = on_save(&game, "reporter", &ask).unwrap();
        assert!(!runner.host.view().options(items).unwrap().dimmed[0]);
        // The interview: its reporter turns through his sixteen pictures,
        // and the sentence of the interview stands in the file.
        let seen = format!("{ask}\nnow choose 6214 0\nuntil dialog 66");
        let runner = on_save(&game, "reporter", &seen).unwrap();
        let ball = runner.host.view().anim(control(&game.dialogs[&66], 1336).unwrap()).unwrap();
        assert_eq!(ball.frames.len(), 16);
        assert!(ball.playing);
        assert!(
            game.dialogs[&66]
                .controls
                .iter()
                .any(|c| c.text.contains("Freigabeklausel in meinem Vertrag"))
        );
        // Closing it makes the clause public and costs five points with
        // the fans and ten of the board's confidence; and since nothing
        // sets the "already known" bit, the item stays ready (§7.3).
        let twice = format!(
            "{seen}\nnow press 2\nuntil dialog 493\nnow choose 6214 0\n\
             until dialog 66\nnow press 2\nuntil dialog 493"
        );
        let runner = on_save(&game, "reporter", &twice).unwrap();
        assert_eq!(runner.host.view().id, 493);
        let state = runner.host.state();
        let manager = &state.records.as_ref().unwrap().managers[usize::from(
            state.records.as_ref().unwrap().human_slot(0).unwrap(),
        )];
        assert_ne!(manager.get::<u8>(0x179) & 1, 0);
        assert_eq!(manager.popularity(), [30, 50, 38]);
        assert_eq!(manager.confidence(), 55);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_other_clubs_answer_the_enquiry() {
        let game = game();
        let ask = "now click bar 12\nuntil dialog 404\nnow choose 4975 8\nuntil dialog 15";
        // Probe1's manager is under contract past the season with neither
        // clause nor release: the enquiry only brings that up (§8.2).
        let runner = on_save(&game, "Probe1", &format!("{ask}\nnow row 1261 0\nuntil dialog 380"))
            .unwrap();
        assert_eq!(runner.host.view().id, 380);
        // With a clause he may ask, and there he stands no chance: his
        // standing of two is below what Probe1's candidates expect.
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "Probe1", "enquiry", |records| {
            let index = records.human_slot(0).unwrap();
            records.managers[index].put(0x178, 42u8);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let runner = on_save(
            &game,
            "enquiry",
            &format!("{ask}\nnow row 1261 0\nuntil dialog 13\nnow press 1257\nuntil dialog 15"),
        )
        .unwrap();
        assert_eq!(runner.host.view().id, 15);
        // In A.an3 his standing fits the candidates' class, and the club
        // turns him down kindly whichever way the dice fall: its luck of
        // nine answers the enquiry at home in any case.
        let root = patched(&dir, "A", "enquiry", |records| {
            let index = records.human_slot(0).unwrap();
            records.managers[index].put(0x178, 42u8);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let runner = on_save(
            &game,
            "enquiry",
            &format!("{ask}\nnow row 1261 0\nuntil dialog 12\nnow press 1\nuntil dialog 15"),
        )
        .unwrap();
        assert_eq!(runner.host.view().id, 15);
        // With a standing of fifteen a next-season candidate at last
        // welcomes him: dialog 14, then the week planner, and the talk's
        // day stands booked (`FUN_006a8d67`) — asking there again only
        // brings up the running talks.
        let root = patched(&dir, "Probe1", "enquiry-warm", |records| {
            let index = records.human_slot(0).unwrap();
            records.managers[index].put(0x178, 42u8);
            records.managers[index].put(0x23, 15u8);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let runner = on_save(
            &game,
            "enquiry-warm",
            &format!(
                "{ask}\nnow row 1261 0\nuntil dialog 14\nnow press 1\nuntil dialog 412\n\
                 now choose 5034 6\nuntil dialog 15\nnow row 1261 0\nuntil dialog 380"
            ),
        )
        .unwrap();
        assert_eq!(runner.host.view().id, 380);
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let slot = records.human_slot(0).unwrap();
        let day = (records.globals.week_start() + 6) as usize;
        let booked = records.planners.get(slot).unwrap().day(day);
        assert_eq!((booked.kind, booked.player), (0x14, records.globals.main_country() as u16));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_refused_co_trainer_leaves_the_market_list() {
        let game = game();
        let open = "now click bar 12\nuntil dialog 404";
        let list = control(&game.dialogs[&35], 1359).unwrap();
        let seen = format!("{open}\nnow choose 4975 0\nuntil dialog 35");
        let Some(before) = on_save(&game, "Probe1", &seen) else { return };
        let state = before.host.state();
        let records = state.records.as_ref().unwrap();
        let keys = before.host.view().list(list).unwrap().keys.clone();
        // One who asks for enough that nothing at all really refuses him.
        let (row, cand) = keys
            .iter()
            .enumerate()
            .map(|(row, &key)| {
                let id = i16::try_from(key).unwrap();
                let r = records.block(&format!("0xbe3920[{id}]")).unwrap();
                (row, id, i32::from_le_bytes(r[0x10..0x14].try_into().unwrap()))
            })
            .find(|&(_, _, pay)| pay >= 20_000)
            .map(|(row, id, _)| (row, id))
            .unwrap();
        let before_co = records
            .team(records.humans().next().unwrap().country(), records.humans().next().unwrap().team())
            .unwrap()
            .get::<i16>(0xa2);
        let then = format!(
            "{seen}\nnow row 1359 {row}\nuntil dialog 32\nnow slide 1350 0\nnow press 1351\n\
             until dialog 34\nnow ok\nuntil dialog 35"
        );
        let runner = on_save(&game, "Probe1", &then).unwrap();
        assert_eq!(runner.host.view().id, 35);
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        // Turned down for good with this manager, and gone from the list;
        // the market block keeps him, the club its old coach.
        let number =
            u32::try_from(records.human_slot(state.season.as_ref().unwrap().current).unwrap())
                .unwrap()
                + 1;
        let r = records.block(&format!("0xbe3920[{cand}]")).unwrap();
        let turned_down = u16::from_le_bytes([r[0x18], r[0x19]]);
        assert_ne!(turned_down & (1 << (number & 31)), 0);
        let keys = runner.host.view().list(list).unwrap().keys.clone();
        assert!(!keys.iter().any(|&key| key == usize::try_from(cand).unwrap()));
        let b = records.countries.slot(usize::from(records.humans().next().unwrap().country()));
        let m = records.block(&format!("0x8e27a0[{b}]")).unwrap();
        let listed = (0..30).any(|k| {
            m.get(2 + k * 4..6 + k * 4)
                .is_some_and(|e| i16::from_le_bytes([e[0], e[1]]) == cand)
        });
        assert!(listed);
        let team = records
            .team(records.humans().next().unwrap().country(), records.humans().next().unwrap().team())
            .unwrap();
        assert_eq!(team.get::<i16>(0xa2), before_co);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_board_books_a_talk_about_the_manager_s_own_contract() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        // Probe1's manager has two years of contract left, enough for the
        // board to see no need; with none it is ready to talk and the
        // planner opens. The patched save sits in a copy of the game.
        let (mut row, mut booked) = (0, 0);
        let root = patched(&dir, "Probe1", "board-talk", |records| {
            let slot = records.human_slot(0).unwrap();
            records.managers[slot].put(0x172, 0i16);
            let (today, start) =
                (i32::from(records.globals.day()), i32::from(records.globals.week_start()));
            row = (0..7)
                .find(|&i| {
                    let day = (start + i) as usize;
                    records.planners[slot].day(day).kind == 0 && start + i >= today
                })
                .unwrap();
            booked = start + row;
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);

        let then = format!(
            "now click bar 13\nuntil dialog 439\nnow choose 5387 0\nuntil dialog 412\n\
             now choose 5034 {row}"
        );
        let Some(runner) = on_save(&game, "board-talk", &then) else { return };
        assert_eq!(seen(&runner, 412), 1);
        assert_eq!(runner.host.view().id, 439);
        // The line now says the talk is booked.
        let view = runner.host.view();
        let text = view.text(control(&game.dialogs[&439], 5360).unwrap()).to_owned();
        assert_eq!(text, game.text(3823));
        // The talk stands in the shared list and the planner's day names
        // the club.
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let slot = records.human_slot(0).unwrap();
        let manager = records.humans().next().unwrap();
        let team = records.team(manager.country(), manager.team()).unwrap();
        let b = records.block("0x1ba0438").unwrap();
        assert_eq!(i16::from_le_bytes([b[0], b[1]]), 1);
        let e = 2;
        assert_eq!(i16::from_le_bytes([b[e], b[e + 1]]), i16::try_from(slot + 1).unwrap());
        assert_eq!(i16::from_le_bytes([b[e + 2], b[e + 3]]), booked as i16);
        assert_eq!(b[e + 4], team.country());
        assert_eq!(i16::from_le_bytes([b[e + 5], b[e + 6]]), i16::from(team.number()));
        let day = records.planners[slot].day(booked as usize);
        assert_eq!([day.kind, day.flag], [0x14, 1]);
        assert_eq!(day.player, u16::from(team.country()));
        assert_eq!(day.club, i16::from(team.number()));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_board_shows_the_club_s_shares_and_their_majority() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        // No club of Probe1's is on the stock market; put the manager's
        // own there with a thousand shares and give his portfolio six
        // hundred of them — a majority, which carries the confidence to
        // its full.
        let root = patched(&dir, "Probe1", "board-shares", |records| {
            let index = records.human_slot(0).unwrap();
            let manager = &records.managers[index];
            let (country, number, slot) =
                (manager.country(), manager.team(), a3_core::stock::place(records, index));
            let b = records.block_mut("0x8be408").unwrap();
            b[0..4].copy_from_slice(&1i32.to_le_bytes());
            let e = 0x14;
            b[e + 0x1c..e + 0x1e].copy_from_slice(&[country as u8, 0]);
            b[e + 0x1e..e + 0x20].copy_from_slice(&(number as i16).to_le_bytes());
            b[e + 0x20..e + 0x24].copy_from_slice(&1000i32.to_le_bytes());
            let place = usize::try_from(slot - 1).unwrap_or(0);
            let p = records.block_mut(&format!("0x8dc6e0[{place}]")).unwrap();
            p[0] = 1;
            p[0x34..0x38].copy_from_slice(&0i32.to_le_bytes());
            p[0x3c..0x40].copy_from_slice(&600i32.to_le_bytes());
        });
        {
            let bytes = std::fs::read(root.join("saves/board-shares.an3")).unwrap();
            let save = a3_files::an3::Save::parse(&bytes).unwrap();
            let b = save.block("0x8dc6e0[0]").unwrap();
            eprintln!("written form {:?} head {:?}", b.form, &b.data[..0x40]);
            let records = a3_core::records::Records::from_save(save).unwrap();
            let b = records.block("0x8dc6e0[0]").unwrap();
            eprintln!("loaded head {:?}", &b[..0x40]);
        }
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let open = "now click bar 13\nuntil dialog 439";
        let Some(runner) = on_save(&game, "board-shares", open) else { return };
        let dialog = &game.dialogs[&439];
        let text = |id| runner.host.view().text(control(dialog, id).unwrap()).to_owned();
        let hidden = |id| runner.host.view().controls[control(dialog, id).unwrap()].hidden;
        // The majority: all the confidence, and the shares' rows.
        {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let index = records.human_slot(0).unwrap();
            eprintln!(
                "slot {} split {:?} majority {}",
                a3_core::stock::place(records, index),
                records.humans().next().and_then(|m| a3_core::stock::split(
                    records,
                    a3_core::stock::place(records, index),
                    m.country() as u8,
                    m.team() as i16
                )),
                a3_core::stock::majority(records, index),
            );
        }
        assert_eq!(text(2774), "100%");
        assert_eq!(text(5371), "60.000%");
        assert_eq!(text(5375), "40.000%");
        assert_eq!(text(5361), dialog.controls[control(dialog, 5361).unwrap()].text);
        assert!(!hidden(5371) && !hidden(5375) && !hidden(5369));
        // The rows of the other humans: none, for Probe1 plays alone.
        let alone = runner.host.state().records.as_ref().unwrap().humans().count() == 1;
        assert_eq!(hidden(5372), !alone);
        assert_eq!(hidden(5380), !alone);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_board_agrees_to_a_resignation_its_flags_stand_with() {
        let game = game();
        // The question comes first, and the board's own flags (§0's
        // +0xdf1) answer it with agreement (§4.5's first step).
        let ask = "now click bar 13\nuntil dialog 439\nnow choose 5387 4";
        let Some(runner) = on_save(&game, "Probe1", &format!("{ask}\nuntil dialog 384")) else {
            return;
        };
        let question = &game.dialogs[&384];
        let asked = runner.host.view().text(control(question, 4738).unwrap()).to_owned();
        assert_eq!(asked, game.text(3888));
        // No to it: the board says nothing, fills nothing anew.
        let no =
            on_save(&game, "Probe1", &format!("{ask}\nuntil dialog 384\nnow press 4739")).unwrap();
        assert_eq!(seen(&no, 380), 0);
        assert_eq!(no.host.view().id, 439);
        // Yes: the flags say the board has agreed, whatever the season.
        let agreed = patched(
            &PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap()),
            "Probe1",
            "board-agreed",
            |records| {
                let index = records.human_slot(0).unwrap();
                let (country, number) =
                    (records.managers[index].country(), records.managers[index].team());
                records.team_mut(country, number).unwrap().put(0x303, 4u8);
            },
        );
        let mut game = Game::load(&agreed).unwrap();
        compose(&mut game);
        let yes = on_save(
            &game,
            "board-agreed",
            &format!("{ask}\nuntil dialog 384\nnow press 4741\nuntil dialog 380"),
        )
        .unwrap();
        let notice = &game.dialogs[&380];
        assert_eq!(
            yes.host.view().text(control(notice, 4730).unwrap()).to_owned(),
            game.text(1803)
        );
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_board_refuses_a_resignation_at_the_seasons_start() {
        let game = game();
        // Ulm's nerves about him are at full hundred and its board has
        // no flags up: the season has played no matchday, so the request
        // is refused (§4.5's third step, as on screen v16 and v17).
        let ask = "now click bar 13\nuntil dialog 439\nnow choose 5387 4\nuntil dialog 384\n\
                   now press 4741\nuntil dialog 380";
        let Some(runner) = on_save(&game, "Probe1", ask) else { return };
        let notice = &game.dialogs[&380];
        let text = runner.host.view().text(control(notice, 4730).unwrap()).to_owned();
        assert_eq!(text, game.text(1805));
        // Refused: the board fills itself anew and stays.
        assert_eq!(seen(&runner, 439), 1);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_board_holds_to_its_manager_through_the_season() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        // Twelve matchdays played, a place in the table and its goal: the
        // season's middle. The board holds to its manager whatever the
        // goal stands at, for `FUN_00775e0a` answers nought or more.
        let root = patched(&dir, "Probe1", "board-holds", |records| {
            let index = records.human_slot(0).unwrap();
            let (country, number) =
                (records.managers[index].country(), records.managers[index].team());
            let level = records.team(country, number).unwrap().get::<u8>(0x12e);
            let league =
                i32::from(records.countries.first_league(usize::from(country))) + i32::from(level);
            let played = 12i16.to_le_bytes();
            records.block_mut(&format!("{:#x}[{league}]", 0x1a09d48)).unwrap()[0x46..0x48]
                .copy_from_slice(&played);
            let team = records.team_mut(country, number).unwrap();
            team.put(0x132, 6u8);
            team.put(0x136, 2i8);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let ask = "now click bar 13\nuntil dialog 439\nnow choose 5387 4\nuntil dialog 384\n\
                   now press 4741\nuntil dialog 380";
        let Some(runner) = on_save(&game, "board-holds", ask) else { return };
        let notice = &game.dialogs[&380];
        let text = runner.host.view().text(control(notice, 4730).unwrap()).to_owned();
        assert_eq!(text, game.text(1808));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_board_weighs_a_resignation_late_in_the_season() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        // Forty matchdays played and the board not thinking well of him:
        // what the club's content with him (28) is against his standing
        // and twenty decides.
        let late = |out: &str, level: u8| {
            patched(&dir, "Probe1", out, |records| {
                let index = records.human_slot(0).unwrap();
                let (country, number) =
                    (records.managers[index].country(), records.managers[index].team());
                let level_of = records.team(country, number).unwrap().get::<u8>(0x12e);
                let league = i32::from(records.countries.first_league(usize::from(country)))
                    + i32::from(level_of);
                let played = 40i16.to_le_bytes();
                records.block_mut(&format!("{:#x}[{league}]", 0x1a09d48)).unwrap()[0x46..0x48]
                    .copy_from_slice(&played);
                let manager = &mut records.managers[index];
                manager.set_confidence(50);
                manager.set_level(level);
            })
        };
        let ask = "now click bar 13\nuntil dialog 439\nnow choose 5387 4\nuntil dialog 384\n\
                   now press 4741\nuntil dialog 380";
        let notice = |game: &Game, runner: &Runner| {
            let dialog = &game.dialogs[&380];
            runner.host.view().text(control(dialog, 4730).unwrap()).to_owned()
        };
        // Two of a standing and twenty short of the club's content with
        // him: the board thinks it may be better so.
        let game = {
            let root = late("board-better", 2);
            let mut game = Game::load(&root).unwrap();
            compose(&mut game);
            game
        };
        let runner = on_save(&game, "board-better", ask).unwrap();
        assert_eq!(notice(&game, &runner), game.text(1806));
        // At a standing of twelve the content is not past what the board
        // weighs against it, and it will hear of nothing.
        let root = late("board-no-way", 12);
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let runner = on_save(&game, "board-no-way", ask).unwrap();
        assert_eq!(notice(&game, &runner), game.text(1807));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_board_lets_a_manager_without_nerves_go() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        // Nerves well under sixty: the board has understanding for him
        // and takes the resignation, which ends his turn (§1's -3).
        let root = patched(&dir, "Probe1", "board-resigned", |records| {
            let index = records.human_slot(0).unwrap();
            records.managers[index].put(0x11e, 40i16);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let ask = "now click bar 13\nuntil dialog 439\nnow choose 5387 4\nuntil dialog 384\n\
                   now press 4741\nuntil dialog 380";
        let Some(runner) = on_save(&game, "board-resigned", ask) else { return };
        let notice = &game.dialogs[&380];
        let text = runner.host.view().text(control(notice, 4730).unwrap()).to_owned();
        assert_eq!(text, game.text(1804));
        // The yes to its alert ends the turn as EndDialog(-3) does: the
        // board is closed, and with Anna Alpha, who has no club, playing
        // no turn the week goes on.
        let over =
            on_save(&game, "board-resigned", &format!("{ask}\nnow press 4731\nuntil dialog 2"))
                .unwrap();
        assert!(a3_core::cycle::dialogs::WEEKS.contains(&over.host.view().id));
        assert_eq!(seen(&over, 380), 1);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_board_answers_the_release_asked() {
        let game = game();
        let ask = "now click bar 13\nuntil dialog 439\nnow choose 5387 3\nuntil dialog 380";
        let Some(runner) = on_save(&game, "Probe1", ask) else { return };
        let notice = &game.dialogs[&380];
        let text =
            |runner: &Runner| runner.host.view().text(control(notice, 4730).unwrap()).to_owned();
        // Ulm stands under its goal, but one year of contract left is not
        // more than the season: the club will not hear of it, and the ask
        // costs five with the board.
        assert_eq!(text(&runner), game.text(1814));
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let index = records.human_slot(0).unwrap();
        assert_ne!(records.managers[index].get::<u32>(0x1f1c) & 0x1000, 0);
        assert_eq!(records.managers[index].shown_confidence(), 77);
        // Once a season only: the second ask is answered without asking.
        let twice = format!("{ask}\nnow press 4731\nnow choose 5387 3\nuntil dialog 380");
        let runner = on_save(&game, "Probe1", &twice).unwrap();
        assert_eq!(text(&runner), game.text(5006));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_board_grants_a_release_when_it_must() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        // Three years of contract left, and a standing worse than the
        // season goal: with the confidence under eighty the board lets
        // the manager go, though it does not like it.
        let root = patched(&dir, "Probe1", "board-freedom", |records| {
            let index = records.human_slot(0).unwrap();
            records.managers[index].put(0x180, 3i16);
            let manager = &records.managers[index];
            let (country, number) = (manager.country(), manager.team());
            records.team_mut(country, number).unwrap().put(a3_core::league::NO_PLACE_AT, 5i8);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let ask = "now click bar 13\nuntil dialog 439\nnow choose 5387 3\nuntil dialog 380";
        let Some(runner) = on_save(&game, "board-freedom", ask) else { return };
        let text = runner.host.view().text(control(&game.dialogs[&380], 4730).unwrap()).to_owned();
        assert_eq!(text, game.text(1813));
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let manager = &records.managers[records.human_slot(0).unwrap()];
        assert_eq!(manager.get::<u8>(0x179) & 1, 1);
        assert_ne!(manager.get::<u16>(0x1f0) & 8, 0);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_grounds_scroll_by_the_arrows_and_keep_their_view() {
        let game = game();
        let open = "now click bar 11\nuntil dialog 464\nnow press 5861\nuntil dialog 466";
        let Some(mut runner) = on_save(&game, "Probe1", open) else { return };
        // Right and down held for a second, then let go.
        runner.host.view_mut().arrows = [false, true, false, true];
        (0..33).for_each(|_| _ = runner.host.tick(&game, 0.03));
        runner.host.view_mut().arrows = [false; 4];
        (0..10).for_each(|_| _ = runner.host.tick(&game, 0.03));
        // The ambience plays, every sound at most at 50.
        let ambience = runner.host.view().ambience.clone();
        assert_eq!(ambience.len(), 8);
        assert!(ambience.iter().all(|&(_, v)| v <= 50));
        let ok = control(&game.dialogs[&466], 1).unwrap();
        runner.host.input(&game, crate::pointer::Target::Button(ok), false);
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let manager = records.humans().next().unwrap();
        let team = records.team(manager.country(), manager.team()).unwrap();
        let grounds =
            a3_core::grounds::Grounds::of(records, team.country(), i16::from(team.number()))
                .unwrap();
        let (x, y) = grounds.view().scroll;
        assert!(x > 0 && y > 0 && x == y, "{x} {y}");
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_grounds_sub_dialogs_report_on_them() {
        let game = game();
        let open = "now click bar 11\nuntil dialog 464\nnow press 5861\nuntil dialog 466";
        let Some(runner) = on_save(&game, "Probe1", &format!("{open}\nnow press 5971")) else {
            return;
        };
        // The info: the club's name, its land in hectares.
        let text = |runner: &Runner, dialog: u16, id: u16| {
            let control = control(&game.dialogs[&dialog], id).unwrap();
            runner.host.view().text(control).to_owned()
        };
        assert!(text(&runner, 467, 6250).starts_with("Vereinsgelände "));
        assert!(text(&runner, 467, 6251).ends_with(" ha"), "{}", text(&runner, 467, 6251));
        // The staff, sorted; the fans' complaints or their content; what
        // is being built or that nothing is — each closed by its OK.
        let rows = |runner: &Runner, dialog: u16, id: u16| {
            let control = control(&game.dialogs[&dialog], id).unwrap();
            runner.host.view().list(control).unwrap().rows.clone()
        };
        let then = |button: u16| format!("{open}\nnow press 5971\nnow press 1\nnow press {button}");
        let runner = on_save(&game, "Probe1", &then(5774)).unwrap();
        let staff = rows(&runner, 465, 5261);
        assert!(!staff.is_empty() && staff.is_sorted_by_key(|r| r.to_lowercase()), "{staff:?}");
        let runner = on_save(&game, "Probe1", &then(5972)).unwrap();
        assert!(!rows(&runner, 468, 5996).is_empty());
        let runner = on_save(&game, "Probe1", &format!("{}\nnow press 1", then(5979))).unwrap();
        assert_eq!(runner.host.view().id, 466);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn land_is_bought_built_on_cleared_and_sold() {
        let game = game();
        let open = "now click bar 11\nuntil dialog 464\nnow press 5861\nuntil dialog 466";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let grounds = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let manager = records.humans().next().unwrap();
            let team = records.team(manager.country(), manager.team()).unwrap();
            let money = (team.get::<i64>(0x2e8 + 0x278), team.get::<u32>(0x2e0 + 0x278));
            let grounds =
                a3_core::grounds::Grounds::of(records, team.country(), team.number().into());
            (grounds.unwrap(), money)
        };
        // A tile in the first view that is not the club's, with nothing on it.
        let (before, money) = grounds(&runner);
        let view = before.view().zoomed(1);
        let ((ox, oy), (offx, offy)) = view.origin();
        let (c, r) = (0..27)
            .flat_map(|r| (0..27).map(move |c| (c, r)))
            .find(|&(c, r)| {
                let (x, y) = ((ox + c) as usize, (oy + r) as usize);
                !before.owns(x, y) && before.at(x, y).is_none()
            })
            .expect("land to buy");
        let (x, y) = ((ox + c) as usize, (oy + r) as usize);
        let at = format!("now click at {} {}", 2 + c * 16 - offx + 8, 25 + r * 16 - offy + 8);
        // Kaufen asks, at 5.000 € a tile.
        let runner = on_save(&game, "Probe1", &format!("{open}\n{at}")).unwrap();
        let question = runner.host.view().text(control(&game.dialogs[&384], 4738).unwrap());
        assert!(question.contains("5.000"), "{question}");
        // Bought, an Apfelbaum planted, pulled down, the land sold: the
        // extensions take 5.000 + 1.000 + 50 €, the stadium's income 3.000 €.
        let then = format!(
            "{open}\n{at}\nnow press 4741\nnow click at 521 187\n{at}\nnow click at 611 187\n\
             {at}\nnow press 4741\nnow click at 566 187\n{at}\nnow press 4741\n\
             now press 1\nuntil dialog 464"
        );
        let runner = on_save(&game, "Probe1", &then).unwrap();
        let (after, (extensions, income)) = grounds(&runner);
        assert_eq!(extensions - money.0, 6050);
        assert_eq!(income.wrapping_sub(money.1), 3000);
        assert!(!after.owns(x, y) && after.at(x, y).is_none());
        assert_eq!(after.count(), before.count());
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_stadium_is_shown_and_its_comfort_bought() {
        let game = game();
        let open = "now click bar 11\nuntil dialog 464";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let dialog = &game.dialogs[&464];
        let text = |runner: &Runner, id: u16| {
            runner.host.view().text(crate::screens::control(dialog, id).unwrap()).to_owned()
        };
        // The places add up to the capacity.
        let places: Vec<i32> = (5846..=5850).map(|id| text(&runner, id).parse().unwrap()).collect();
        assert_eq!(places.iter().sum::<i32>(), text(&runner, 5851).parse::<i32>().unwrap());
        let stands = runner
            .host
            .view()
            .list(crate::screens::control(dialog, 5853).unwrap())
            .unwrap()
            .rows
            .len();
        assert_eq!(stands, 4);
        // Box seats ticked: they cost, are carried out, and stay once left.
        let state = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let manager = records.humans().next().unwrap();
            let team = records.team(manager.country(), manager.team()).unwrap();
            let subs =
                a3_core::stadium::subs(records, team.country(), team.number().into()).unwrap();
            (team.get::<i64>(0x2d8 + 0x278), subs[0].has(a3_core::stadium::Comfort::Boxes))
        };
        let (costs, boxes) = state(&runner);
        assert!(!boxes);
        let buy = format!(
            "{open}\nnow click at 150 427\nnow press 5917\nuntil dialog 384\nnow press 4741\n\
             until dialog 464\nnow click bar 10\nuntil dialog 218"
        );
        let runner = on_save(&game, "Probe1", &buy).unwrap();
        assert_eq!(state(&runner), (costs + 250_000, true));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn any_clubs_squad_is_looked_through() {
        let game = game();
        let open = "now click bar 10\nuntil dialog 218\nnow choose 6371 4\nuntil dialog 306";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let dialog = &game.dialogs[&306];
        let list = |runner: &Runner, id: u16| {
            runner.host.view().list(crate::screens::control(dialog, id).unwrap()).unwrap().clone()
        };
        // The main country's clubs but our own, by name; no players yet.
        let clubs = list(&runner, 4060);
        assert!(!clubs.rows.is_empty());
        let key = |name: &str| name.to_lowercase();
        assert!(clubs.rows.windows(2).all(|w| key(&w[0]) <= key(&w[1]) || !w[0].is_ascii()));
        assert!(list(&runner, 4059).rows.is_empty());
        // A club: its players, by position, and the heading.
        let club = format!("{open}\nnow row 4060 0");
        let runner = on_save(&game, "Probe1", &club).unwrap();
        let players = list(&runner, 4059);
        assert!(!players.rows.is_empty());
        assert!(players.rows.iter().all(|r| r.split('\t').count() == 7));
        let heading =
            runner.host.view().text(crate::screens::control(dialog, 6286).unwrap()).to_owned();
        assert!(heading.contains("D-Stärke"), "{heading}");
        // NORACSA needs a computer department, which Ulm lacks.
        let noracsa = "now click bar 10\nuntil dialog 218\nnow choose 6371 6\nuntil dialog 380";
        on_save(&game, "Probe1", noracsa).unwrap();
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_calculations_page_and_book_what_they_may() {
        let game = game();
        let open = "now click bar 9\nuntil dialog 417";
        let staff = format!("{open}\nnow click at 88 225\nuntil dialog 447");
        let Some(runner) = on_save(&game, "Probe1", &staff) else { return };
        let booked = |runner: &Runner, c: usize| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let manager = records.humans().next().unwrap();
            let team = records.team(manager.country(), manager.team()).unwrap();
            (team.get::<u8>(c + 0x278), team.get::<i32>(0x228 + 0x50 + 0x278))
        };
        // Ulm's groundsman, level 0: raised by his spinner, booked on OK.
        assert_eq!(booked(&runner, 0x301), (0, 12_000));
        let raise = format!("{staff}\nnow spin 5704 up\nnow click at 606 449\nuntil dialog 417");
        assert_eq!(booked(&on_save(&game, "Probe1", &raise).unwrap(), 0x301), (1, 24_000));
        // The arrow bar pages on to the players' calculation and back.
        let paging = format!(
            "{staff}\nnow click at 479 439\nuntil dialog 454\nnow click at 440 439\n\
             until dialog 447\nnow click at 547 449\nuntil dialog 417"
        );
        on_save(&game, "Probe1", &paging).unwrap();
        // A better statistics department, booked at its yearly price.
        let statistics = format!(
            "{open}\nnow click at 88 318\nuntil dialog 122\nnow click at 273 274\n\
             now click at 606 449\nuntil dialog 417"
        );
        let runner = on_save(&game, "Probe1", &statistics).unwrap();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let manager = records.humans().next().unwrap();
        let team = records.team(manager.country(), manager.team()).unwrap();
        assert_eq!(team.get::<u8>(0x5e1 + 0x278), 1);
        // The ticket office lists the home matches ahead; its standing
        // price, raised, is written at once and kept though it is
        // cancelled.
        let tickets = format!("{open}\nnow click at 88 64\nuntil dialog 137");
        let runner = on_save(&game, "Probe1", &tickets).unwrap();
        let list = control(&game.dialogs[&137], 2437).unwrap();
        let first = runner.host.view().list(list).unwrap().rows[0].clone();
        assert_eq!(first, "31.8.2025\tLigaspiel\tFC Ingolstadt 04\t0\tnormal");
        let raised = format!("{tickets}\nnow spin 2423 up\nnow click at 547 449\nuntil dialog 417");
        let price = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let manager = records.humans().next().unwrap();
            records.team(manager.country(), manager.team()).unwrap().get::<i16>(0x49c + 0x278)
        };
        assert_eq!(price(&runner), 15);
        assert_eq!(price(&on_save(&game, "Probe1", &raised).unwrap()), 16);
        // Back from the tickets to the shares, round on to the TV money;
        // from the sponsors on to the merchandise, whose OK closes it.
        let round = format!(
            "{tickets}\nnow click at 440 439\nuntil dialog 446\nnow click at 479 439\n\
             until dialog 137\nnow click at 479 439\nuntil dialog 448"
        );
        on_save(&game, "Probe1", &round).unwrap();
        let merchandise = format!(
            "{open}\nnow click at 88 133\nuntil dialog 125\nnow click at 479 439\n\
             until dialog 115\nnow click at 606 449\nuntil dialog 417"
        );
        on_save(&game, "Probe1", &merchandise).unwrap();
        // The main sponsor's contract, from the sponsors' page and back.
        let contract = format!(
            "{open}\nnow click at 88 133\nuntil dialog 125\nnow press 2351\nuntil dialog 246"
        );
        let runner = on_save(&game, "Probe1", &contract).unwrap();
        let text =
            |id| runner.host.view().text(control(&game.dialogs[&246], id).unwrap()).to_owned();
        assert!(text(3424).starts_with("Sponsorenpool ("), "{}", text(3424));
        assert!(text(3416).ends_with(" %"), "{}", text(3416));
        on_save(&game, "Probe1", &format!("{contract}\nnow press 3422\nuntil dialog 125")).unwrap();
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_books_compare_with_the_season_before() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let open = "now click bar 9\nuntil dialog 417";
        let game = game();
        let combo = |runner: &Runner| {
            runner
                .host
                .view()
                .combo(control(&game.dialogs[&417], 2227).unwrap())
                .unwrap()
                .clone()
        };
        // Without a last season the combo has nothing to compare with.
        let runner = on_save(&game, "Probe1", open).unwrap();
        assert_eq!(
            combo(&runner).items,
            [game.text(2363).to_owned(), game.text(2365).to_owned()]
        );
        // Probe1's extra data can keep one: its club's booked accounts
        // stand in for last season's (with one word raised to tell the
        // columns apart), and what its transfers came to — figures no
        // account of this season shows.
        let root = patched(&dir, "Probe1", "vorjahr", |records| {
            let index = records.human_slot(0).unwrap();
            let (country, number) =
                (records.managers[index].country(), records.managers[index].team());
            let team = records.team_index(country, number).unwrap();
            let slot = usize::from(records.teams[team].human()) - 1;
            let booked: Vec<u32> = (0..60).map(|w| records.teams[team].get(0x4a0 + 4 * w)).collect();
            let extra = &mut records.extras[slot];
            for (w, word) in booked.into_iter().enumerate() {
                let word = if w == 22 { word.wrapping_add(7000) } else { word };
                extra.put(0x4c8 + 4 * w, word);
            }
            extra.put(0x710, 12345678i64);
            extra.put(0x718, 23456789i64);
            extra.put(0x898, 1i16);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let runner = on_save(&game, "vorjahr", &format!("{open}\nnow row 2227 1")).unwrap();
        let view = runner.host.view();
        let combo = view.combo(control(&game.dialogs[&417], 2227).unwrap()).unwrap();
        assert_eq!(combo.items[1], game.text(2364));
        assert_eq!(combo.selected, Some(1));
        // The transfers row shows last season's sales and purchases, and
        // both comparison columns are headed "Vorjahr".
        let text = |id| view.text(control(&game.dialogs[&417], id).unwrap()).to_owned();
        assert_eq!(text(5059), "23.456.789");
        assert_eq!(text(5073), "12.345.678");
        assert_eq!(text(5780), game.text(2396));
        assert_eq!(text(5782), game.text(2396));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn players_are_watched_and_dropped() {
        let game = game();
        let menu = "now click bar 10\nuntil dialog 218";
        // Two players from the transfer list onto the watch list.
        let watch = format!(
            "{menu}\nnow choose 6371 0\nuntil dialog 287\nnow rclick 3937 row 0\nnow choose 9950 1\n\
             now rclick 3937 row 1\nnow choose 9950 1\nnow press 3938\nuntil dialog 218\n\
             now choose 6371 2\nuntil dialog 27"
        );
        let Some(runner) = on_save(&game, "Probe1", &watch) else { return };
        let dialog = &game.dialogs[&27];
        let rows = |runner: &Runner| {
            let view = runner.host.view();
            view.list(crate::screens::control(dialog, 1330).unwrap()).unwrap().rows.len()
        };
        assert_eq!(rows(&runner), 2);
        let line =
            runner.host.view().text(crate::screens::control(dialog, 1329).unwrap()).to_owned();
        assert_eq!(line, "Derzeit stehen 2 Spieler unter Beobachtung.");
        // The first dropped again through his choices.
        let drop =
            format!("{watch}\nnow row 1330 0\nuntil dialog 28\nnow choose 1333 1\nuntil dialog 27");
        assert_eq!(rows(&on_save(&game, "Probe1", &drop).unwrap()), 1);
        // The week's appointments open, as public as they are.
        let talks = format!("{menu}\nnow choose 6371 1\nuntil dialog 25");
        on_save(&game, "Probe1", &talks).unwrap();
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_purchase_is_booked_and_a_talk_joined() {
        use a3_core::transfers;
        let game = game();
        let list = "now click bar 10\nuntil dialog 218\nnow choose 6371 0\nuntil dialog 287";
        let Some(runner) = on_save(&game, "Probe1", list) else { return };
        let rows = control(&game.dialogs[&287], 3937).unwrap();
        let keys: Vec<u16> =
            runner.host.view().list(rows).unwrap().keys.iter().map(|&k| k as u16).collect();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let manager = records.humans().next().unwrap();
        let own = (manager.country() as u8, manager.team() as i16);
        let start = records.globals.week_start();
        let listed = transfers::listings(records);
        let sale = |id: u16| listed.iter().any(|e| e.id == id && e.kind == transfers::SALE);
        let theirs = |id: u16| records.player(id).is_some_and(|r| r.club()[0] != 0);
        // A player of another club with no appointment, and one with an
        // appointment this week on a day the club has free.
        let free = keys
            .iter()
            .position(|&id| sale(id) && theirs(id) && transfers::talk_day(records, id).is_none())
            .unwrap();
        let fixed = keys
            .iter()
            .position(|&id| {
                transfers::talk_day(records, id).is_some_and(|d| {
                    (start..start + 7).contains(&d)
                        && records.planners[0].day(d as usize).kind == 0
                        && !transfers::talks_on(records, own, d)
                }) && sale(id)
                    && theirs(id)
            })
            .unwrap();
        let fixed_day = transfers::talk_day(records, keys[fixed]).unwrap();
        drop(state);
        // The first is booked on Saturday, in the planner as a negotiation
        // of the transfer list.
        let buy = format!(
            "{list}\nnow row 3937 {free}\nuntil dialog 412\nnow choose 5034 6\nuntil dialog 287"
        );
        let runner = on_save(&game, "Probe1", &buy).unwrap();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let talks = transfers::appointments(records);
        let talk = talks.iter().find(|t| t.player == keys[free]).unwrap();
        assert_eq!((talk.kind, talk.day, talk.clubs[1]), (3, start + 6, own));
        let booked = records.planners[0].day(start as usize + 6);
        assert_eq!((booked.kind, booked.player), (0x0e, keys[free]));
        drop(state);
        // The club joins the second's, on its day.
        let join = format!(
            "{list}\nnow row 3937 {fixed}\nuntil dialog 412\nnow press 1\nuntil dialog 287"
        );
        let runner = on_save(&game, "Probe1", &join).unwrap();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let talks = transfers::appointments(records);
        assert!(talks.iter().find(|t| t.player == keys[fixed]).unwrap().clubs.contains(&own));
        let booked = records.planners[0].day(fixed_day as usize);
        assert_eq!((booked.kind, booked.player), (0x0e, keys[fixed]));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_finances_show_the_season_row_by_row() {
        let game = game();
        let texts = |then: &str, ids: &[u16]| -> Vec<String> {
            let then = format!("now click bar 9\nuntil dialog 417\n{then}");
            let runner = on_save(&game, "Probe1", &then).unwrap();
            let view = runner.host.view();
            let dialog = &game.dialogs[&417];
            ids.iter()
                .map(|&id| view.text(crate::screens::control(dialog, id).unwrap()).to_owned())
                .collect()
        };
        if game.save_games().iter().all(|s| s.name != "Probe1") {
            return;
        }
        // As the original showed them: the tickets, the TV money, the
        // sponsors, the other income, the staff, the players, the matches
        // and training, the stadium, the other costs, the result.
        let rows = [5093, 5094, 5096, 5098, 5086, 5087, 5088, 5089, 5090, 5092, 1616];
        assert_eq!(
            texts("", &rows),
            [
                "85.487", "900.000", "266.000", "19.583", "34.566", "272.833", "170.740",
                "121.666", "7.500", "663.765", "Gewinn"
            ]
        );
        // This week's: the tickets and sponsors since the turn began.
        assert_eq!(texts("now row 2226 3", &[5093, 5096, 5092]), ["8.171", "45.000", "53.171"]);
        // The season carried on: the tickets at the plan's attendance.
        assert_eq!(texts("now row 2226 0", &[5093]), ["1.245.752"]);
        // The season extrapolated.
        let rows = [5096, 5098, 5086, 5087, 5088, 5090];
        assert_eq!(
            texts("now row 2226 1", &rows),
            ["280.000", "230.000", "410.000", "208.431", "397.890", "90.000"]
        );
        assert_eq!(texts("now row 2226 1", &[5093, 5092]), ["1.240.000", "1.422.013"]);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_talk_is_booked_in_the_planner() {
        let game = game();
        // The keeper's menu on the first day of a week: the talk is its
        // second entry; the first free day is today.
        let then =
            "now rclick 2510; choose 9950 1\nuntil dialog 412\nnow choose 5034 0\nuntil dialog 147";
        let Some(runner) = on_save(&game, "Probe1", then) else { return };
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let day = usize::from(records.globals.day());
        let booked = records.planners[0].day(day);
        assert_eq!((booked.kind, booked.flag, booked.player), (0x19, 1, 3204));
    }

    /// "Profivertrag geben" for Probe1's amateur Wilson (dialog 11): the
    /// week planner books the talk on Monday, kind 0x10, into the
    /// amateurs' list too; asked again, alert 2921 refuses — as the
    /// original does.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_professional_contract_talk_is_booked() {
        let game = game();
        let wilson = "now click at 80 91\nuntil dialog 11";
        let book = format!(
            "now click bar 10\nuntil dialog 218\nnow choose 5042 3\nuntil dialog 427\n\
             {wilson}\nnow choose 1254 1\nuntil dialog 412\nnow choose 5034 1\nuntil dialog 427"
        );
        let Some(runner) = on_save(&game, "Probe1", &book) else { return };
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let monday = usize::from(records.globals.day()) + 1;
        let booked = records.planners[0].day(monday);
        assert_eq!((booked.kind, booked.flag), (0x10, 1));
        assert!(a3_core::youth::amateur_talk_booked(records, booked.player));
        drop(state);
        let again = format!("{book}\n{wilson}\nnow choose 1254 1\nuntil dialog 380");
        let Some(runner) = on_save(&game, "Probe1", &again) else { return };
        let view = runner.host.view();
        let text = view.text(control(&game.dialogs[&380], 4730).unwrap());
        assert_eq!(text, game.text(2921));
    }

    /// The booked professional contract's talk speaks up on its day: the
    /// original's day run walks the talks list the planner booking wrote
    /// and, while he is still with the club, posts its message (text 5048,
    /// `FUN_0076cb3e`, `FUN_0076bb38`, `FUN_0076c613`). Wilson's talk from
    /// `a_professional_contract_talk_is_booked` is booked for Monday, and
    /// the week plays on to Tuesday: the week message then carries it.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_contract_talk_is_announced_on_its_day() {
        let game = game();
        let wilson = "now click at 80 91\nuntil dialog 11";
        let book = format!(
            "now click bar 10\nuntil dialog 218\nnow choose 5042 3\nuntil dialog 427\n\
             {wilson}\nnow choose 1254 1\nuntil dialog 412\nnow choose 5034 1\nuntil dialog 427"
        );
        // The week on: the planner's yes, the line-up and the office
        // whistled off, the no-training question answered, and over the
        // days the week reports and waits for.
        let play = "on 427 ok\non 218 click bar 0\non 147 click bar 18\non 302 click bar 18\n\
                    on 384 press 4741\non 2 wait\non 381 wait\nrepeat 2 days";
        let Some(runner) = on_save(&game, "Probe1", &format!("{book}\n{play}")) else { return };
        let state = runner.host.state();
        let season = state.season.as_ref().expect("the season is playing");
        let records = state.records.as_ref().unwrap();
        // Two days on: the Monday the talk stands in the planner for.
        let talk = records.planners[0].day(usize::try_from(season.day).unwrap() - 1);
        assert_eq!((talk.kind, talk.flag), (0x10, 1), "no talk booked on Monday");
        let name = records.name(records.player(talk.player).unwrap().last_name()).to_owned();
        assert_eq!(season.planner_message, Some((5048, name)), "the talk is not announced");
    }

    /// The league's matchdays play on as the days pass: the save's first
    /// league has played three matchdays, and ten days carry at least one
    /// more — every game with a result, half time within full time, the
    /// clubs' records agreeing with the table the results reckon, the
    /// gates: a crowd to every matchday, money booked and a gate held for
    /// every home game, the attendance rows keeping step, the away games
    /// counted and the human club's extra rows filled — visiting fans,
    /// gate incomes, travelling gates matching the home club's own — and
    /// the players': new appearances and goals over the save, the league's
    /// top-scorers list grown by exactly the new goals, and the rows of
    /// the matchdays in the players' forty-six — the starters, the
    /// substitutions, the grades, the cards and the goal nibbles all
    /// answering to the counters — and the human's year calendar filling
    /// its booked fixture days with the new scores and the standing
    /// result flags. Played on
    /// a copy: ten days cross week ends, and their autosaves roll into the
    /// game's own `saves` — which would shuffle the list the other tests
    /// pick their save from.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_league_matchdays_play_on_as_the_days_pass() {
        use a3_core::{opponent, statistics};
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "A", "league-days", |_| {});
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let play = "on 89 press 1875\non 427 ok\non 218 click bar 0\non 147 click bar 18\non 302 click bar 18\n\
                    on 384 press 4741\non 308 choose 4095 0\non 96 ok\non 93 press 1943\non 499 ok\non 41 ok\non 332 ok\non 269 ok\non 2 wait\non 381 wait\non 25 ok\non 270 ok\non 383 click at 460 210\nrepeat 10 days";
        // The save as copied, to reckon the players' figures against.
        let bytes = std::fs::read(root.join("saves/league-days.an3")).unwrap();
        let before = a3_core::records::Records::from_save(a3_files::an3::Save::parse(&bytes).unwrap())
            .expect("the save's records");
        let home_nation = u16::try_from(before.globals.main_country()).unwrap();
        let league0 = opponent::League::of(&before, home_nation, 0).expect("the first league");
        let squad: Vec<u16> = (0..league0.teams())
            .flat_map(|place| {
                before
                    .team(home_nation, league0.team(place) as u16)
                    .map(|t| t.players())
                    .unwrap_or_default()
            })
            .collect();
        let apps = |recs: &a3_core::records::Records, squad: &[u16]| {
            squad
                .iter()
                .map(|&id| usize::from(recs.player(id).map_or(0, |p| p.league_matches())))
                .sum::<usize>()
        };
        let goals = |recs: &a3_core::records::Records, squad: &[u16]| {
            squad
                .iter()
                .map(|&id| usize::from(recs.player(id).map_or(0, |p| p.goals()[0])))
                .sum::<usize>()
        };
        let lesss = |recs: &a3_core::records::Records, squad: &[u16]| {
            squad
                .iter()
                .map(|&id| usize::from(recs.player(id).map_or(0, |p| p.league_less()[0])))
                .sum::<usize>()
        };
        let carded = |recs: &a3_core::records::Records, squad: &[u16]| {
            squad
                .iter()
                .map(|&id| {
                    recs.player(id).map_or(0, |p| {
                        usize::from(p.league_cards()[0])
                            + usize::from(p.league_cards()[1])
                            + usize::from(p.league_cards()[2])
                    })
                })
                .sum::<usize>()
        };
        let tor_before: usize = statistics::torjaeger(&before, home_nation, 0)
            .iter()
            .map(|r| usize::from(r.values[0]))
            .sum();
        let Some(runner) = on_save(&game, "league-days", play) else { return };
        let state = runner.host.state();
        let (season, records) = (state.season.as_ref().unwrap(), state.records.as_ref().unwrap());
        let nation = u16::try_from(records.globals.main_country()).unwrap();
        let league = opponent::League::of(records, nation, 0).expect("the first league");
        // A man sold on in these days leaves his season's counts and rows
        // behind (`FUN_00707bbf`): the players' figures are reckoned over
        // those still at their club.
        let squad: Vec<u16> = squad
            .into_iter()
            .filter(|&id| before.player(id).map(|p| p.club()) == records.player(id).map(|p| p.club()))
            .collect();
        let (apps_before, goals_before, less_before, carded_before) =
            (apps(&before, &squad), goals(&before, &squad), lesss(&before, &squad), carded(&before, &squad));
        // The save opened on three played matchdays, and the days since
        // each pair meet once a week at least.
        assert!(league.played() > 3, "still only {} matchdays played", league.played());
        for md in 3..league.played() {
            for (home, away, result) in league.games(md) {
                let [full, away_full, half, away_half] =
                    result.unwrap_or_else(|| panic!("md {md}: {home}-{away} unwritten"));
                assert!(
                    half <= full && away_half <= away_full,
                    "md {md}: {home}-{away} {full}:{away_full} ht {half}:{away_half}"
                );
            }
        }
        // The clubs carry what the reckoned table shows them.
        let rows = statistics::table(records, &league, statistics::ALL);
        for (rank, row) in rows.iter().enumerate() {
            let team = records.team(nation, row.team as u16).expect("a league club");
            assert_eq!(team.table_place(), rank as u8 + 1, "team {}", row.team);
            assert_eq!(
                team.record(),
                [
                    u8::try_from(row.won).unwrap(),
                    u8::try_from(row.drawn).unwrap(),
                    u8::try_from(row.lost).unwrap(),
                ],
                "team {}",
                row.team
            );
            assert_eq!(
                team.goals(),
                [
                    u8::try_from(row.goals_for).unwrap(),
                    u8::try_from(row.goals_against).unwrap(),
                ],
                "team {}",
                row.team
            );
        }
        // The gates come in with the games: every matchday the days played
        // has its crowd, every club booked money, holds a gate per home
        // game, and its attendance row keeps step with them.
        for md in 3..league.played() {
            assert!(league.matchday_crowds()[md] > 0, "md {md}: no crowd");
        }
        let att_slot = statistics::attendance_slot(records, nation, 0).expect("the league's row");
        let att_rows = statistics::league_attendance(records, att_slot).expect("the attendance rows");
        for place in 0..league.teams() {
            let number = league.team(place) as u16;
            let team = records.team(nation, number).expect("a league club");
            let booked = team.get::<i32>(0x230 + a3_core::finances::CLUB_TO_TEAM);
            assert!(booked > 0, "team {number}: no gate money booked");
            let homes = (0..league.played())
                .flat_map(|md| league.games(md))
                .filter(|&(home, _, _)| home == place)
                .count();
            let held = usize::from(team.home_games());
            assert_eq!(held, homes, "team {number}: {} gates for {homes} home games", held);
            for idx in 0..held {
                assert!(team.gates()[idx] > 0, "team {number}: gate {idx} nought");
            }
            let row = att_rows[place];
            assert_eq!(usize::try_from(row.games).unwrap(), held, "team {number}: row out of step");
            assert!(row.total > 0 && row.percent > 0 && row.percent <= 100, "team {number}: {row:?}");
            // The away games stand counted too: home games plus away games
            // is what the league has played against the club.
            let aways = (0..league.played())
                .flat_map(|md| league.games(md))
                .filter(|&(_, away, _)| away == place)
                .count();
            assert_eq!(
                usize::from(team.away_games()),
                aways,
                "team {number}: {} away games counted for {aways}",
                team.away_games()
            );
            // The human club's extra block keeps its rows: visiting fans
            // within the gate and incomes at every home game, and every
            // travelling gate equal to the home club's own gate of that
            // very game.
            if team.human() > 0
                && let Some(x) = records.extras.get(usize::from(team.human()) - 1)
            {
                for k in 0..held {
                    let fans = x.fan_gates()[k];
                    assert!(
                        fans > 0 && fans <= team.gates()[k],
                        "team {number}: {fans} visiting fans of a {}",
                        team.gates()[k]
                    );
                    assert!(x.home_incomes()[k] > 0, "team {number}: income {k} nought");
                }
                let mut host_homes = vec![0usize; league.teams()];
                let mut j = 0usize;
                for md in 0..league.played() {
                    for (home, away, _) in league.games(md) {
                        if away == place {
                            if md >= 3 {
                                let host = records
                                    .team(nation, league.team(usize::from(home)) as u16)
                                    .expect("a league club");
                                assert_eq!(
                                    x.away_gates()[j],
                                    host.gates()[host_homes[usize::from(home)]],
                                    "team {number}: travelling gate {j}"
                                );
                            }
                            j += 1;
                        }
                        host_homes[usize::from(home)] += 1;
                    }
                }
            }
        }
        // The matches left their mark on the players: at least twenty of
        // the twenty-two stand per game even with injuries and suspensions,
        // the goals of their season's league column grew by just the new
        // ones, and the top-scorers list took each of them once.
        let new_games: usize = (3..league.played()).map(|md| league.games(md).len()).sum();
        let new_goals: usize = (3..league.played())
            .flat_map(|md| league.games(md))
            .map(|(_, _, r)| {
                let r = r.expect("written by the days");
                usize::from(r[0]) + usize::from(r[1])
            })
            .sum();
        assert!(
            apps(records, &squad) >= apps_before + 20 * new_games,
            "apps {apps_before} → {}, {new_games} games",
            apps(records, &squad)
        );
        assert_eq!(goals(records, &squad), goals_before + new_goals, "scorers' goals");
        let tor_after: usize = statistics::torjaeger(records, nation, 0)
            .iter()
            .map(|r| usize::from(r.values[0]))
            .sum();
        assert_eq!(tor_after, tor_before + new_goals, "the top-scorers list");
        // And the matchday rows in the players' forty-six: twenty to
        // twenty-two starters to every game, one to three men changed per
        // side, grades within the half steps the saves read, and every row
        // in agreement with the counters — appearances, coming-ons, cards
        // and the goal nibbles all adding up to what the games gave.
        for md in 3..league.played() {
            for (home, away, _) in league.games(md) {
                let men: Vec<u16> = [home, away]
                    .into_iter()
                    .flat_map(|place| {
                        records
                            .team(nation, league.team(place) as u16)
                            .map(|t| t.players())
                            .unwrap_or_default()
                    })
                    .collect();
                let started = men
                    .iter()
                    .filter(|&&id| records.player(id).is_some_and(|p| p.matchdays()[md] & 0x40 != 0))
                    .count();
                let touched = men
                    .iter()
                    .filter(|&&id| records.player(id).is_some_and(|p| p.matchdays()[md] & 0x80 != 0))
                    .count();
                assert!(
                    (20..=22).contains(&started) && (4..=12).contains(&touched),
                    "md {md}: {started} starters, {touched} changes of men"
                );
            }
        }
        let mut row_apps = 0usize;
        let mut came_ons = 0usize;
        let mut row_cards = 0usize;
        let mut nibbled = 0usize;
        for &id in &squad {
            let Some(p) = records.player(id) else { continue };
            let bytes = p.matchdays();
            let nibbles = p.matchday_goals();
            for md in 3..league.played() {
                let b = bytes[md];
                if b & 0xc0 != 0 {
                    row_apps += 1;
                    let grade = (b >> 2) & 0xf;
                    assert!((5..=11).contains(&grade), "p{id} md {md}: grade {grade}");
                }
                if b & 0xc0 == 0x80 {
                    came_ons += 1;
                }
                if b & 3 != 0 {
                    row_cards += 1;
                }
                nibbled += usize::from(nibbles[md] >> 4);
            }
        }
        assert_eq!(apps(records, &squad), apps_before + row_apps, "the rows and the appearance counts");
        assert_eq!(lesss(records, &squad), less_before + came_ons, "the coming-ons at +0x254");
        assert_eq!(carded(records, &squad), carded_before + row_cards, "the rows' cards and the counters");
        assert_eq!(nibbled, new_goals, "the matchday goal nibbles");
        // The grade of the last match stands at +0x273 as the row shows it.
        let last = league.played() - 1;
        for &id in &squad {
            if let Some(p) = records.player(id) {
                let b = p.matchdays()[last];
                if b & 0xc0 != 0 {
                    assert_eq!(
                        usize::from(p.grade()),
                        usize::from((b >> 2) & 0xf),
                        "p{id}: the grade of matchday {last}"
                    );
                }
            }
        }
        // The year calendar had the whole season booked: each of the
        // human's fixtures stands on its day — kind 6, the matchday
        // plain as its code, both clubs named — with no score and no
        // flag. The played matchdays fill their day's entry: the new
        // score, and the flag saying the result stands (0x20).
        let human = (0..league.teams())
            .map(|place| league.team(place) as u16)
            .find(|&number| records.team(nation, number).is_some_and(|t| t.human() > 0))
            .expect("the human club");
        let hum = usize::from(records.team(nation, human).unwrap().human()) - 1;
        let cal = &records.calendars[hum];
        for md in 3..league.played() {
            let code = u32::try_from(md + 1).unwrap();
            let day = (0..a3_core::records::PLANNER_DAYS)
                .find(|&d| {
                    let e = cal.entry(d);
                    e[0] == 6 && u32::from_le_bytes([e[4], e[5], e[6], e[7]]) == code
                })
                .expect("the calendar's day of the matchday");
            let (home, away, result) = league
                .games(md)
                .into_iter()
                .find(|&(h, a, _)| {
                    [league.team(h) as u16, league.team(a) as u16].contains(&human)
                })
                .expect("the human's fixture");
            let [hg, ag, _, _] = result.expect("the fixture's score");
            let e = cal.entry(day);
            assert_eq!(e[8], nation as u8, "md {md} day {day}: the entry's country");
            assert_eq!(
                u16::from_le_bytes([e[10], e[11]]),
                league.team(home) as u16,
                "md {md}: the entry's home club"
            );
            assert_eq!(
                u16::from_le_bytes([e[0xe], e[0xf]]),
                league.team(away) as u16,
                "md {md}: the entry's away club"
            );
            assert_eq!(
                &e[0x10..0x12],
                &[hg, ag],
                "md {md} day {day}: the calendar's score"
            );
            assert_eq!(e[0x14] & 0x20, 0x20, "md {md}: the result flag");
        }
        assert!(season.day > 63);
    }

    /// The country's cup round plays on when its day comes. The cup's days
    /// stand coded on the country's first league row — for Germany round 1
    /// at day 59 — and on save A, at day 63, the first round stands played
    /// and the second already drawn; the patch sets the day back before
    /// the round's, wipes the first round's scores, empties the second's
    /// rows and sends the block back to round 1. Ten days cross the day:
    /// every first-round pairing holds a result with its flags answering
    /// to the score — a shootout's score staying even — the second round's
    /// rows hold the winners, each once and unplayed, the block names
    /// round 2 as drawn, and the human club's tie played with the rest —
    /// its home tie booking its gate on the club's cup takings and its
    /// men carrying the tie's marks: appearances in the career's cup
    /// bytes, goals in the season's and the career's; its year calendar
    /// fills the round's day with the new score and names the next
    /// round's tie on its day — or drops it once the club is out.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_cup_round_plays_on_when_its_day_comes() {
        use a3_core::opponent::{CUPS_AT, PAIRING, PAIRINGS};
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let mut cup_gate = (0u16, 0i32);
        let mut cup_marks: Vec<(u16, (i64, i64, i64))> = Vec::new();
        // The visitor of the human's home tie, and the box and visitor's
        // cup takings as the patch leaves them, to measure the split.
        let mut cup_away = (0u8, 0u16);
        let mut cup_box: Option<(usize, i32, i32)> = None;
        let root = patched(&dir, "A", "cup-days", |records| {
            records.globals.set_day(56);
            // The save's planner still holds the talks and visits booked in
            // the days it lived through; rewound, they open their dialogs
            // again — the talk's kind 0x19 and the visit's 0x0b — so the
            // patch clears them for the days ahead.
            if let Some(slot) = records.human_slot(0) {
                if let Some(planner) = records.planners.get_mut(slot) {
                    for day in 56..=66 {
                        if matches!(planner.entry(day)[0], 0x19 | 0x0b) {
                            planner.set_entry(day, [0; a3_core::records::PLANNER_ENTRY]);
                        }
                    }
                }
            }
            let nation = usize::try_from(records.globals.main_country()).unwrap();
            let cup = format!("{CUPS_AT:#x}[{}]", records.countries.slot(nation));
            // The human club plays its first-round tie at home on this
            // save; keep its number and the cup takings the save holds
            // from the round it lived through, to see the gate come on,
            // and the men's cup marks — season goals, career goals and
            // the career's five appearance bytes — to see the tie leave
            // its marks on them.
            let totals = |records: &a3_core::records::Records, num: u16| -> (i64, i64, i64) {
                let mut marks = (0i64, 0i64, 0i64);
                if let Some(t) = records.team(nation as u16, num) {
                    for id in t.players() {
                        if let Some(p) = records.player(id) {
                            marks.0 += i64::from(p.goals()[1]);
                            marks.1 += i64::from(p.career_cup_goals());
                            marks.2 += p
                                .career_cup_appearances()
                                .iter()
                                .map(|&a| i64::from(a))
                                .sum::<i64>();
                        }
                    }
                }
                marks
            };
            if let Some(b) = records.block(&cup) {
                for q in 0..32 {
                    let p = &b[PAIRINGS + q * PAIRING..PAIRINGS + (q + 1) * PAIRING];
                    let num = u16::from_le_bytes([p[2], p[3]]);
                    if usize::from(p[0]) == nation
                        && records
                            .team(nation as u16, num)
                            .is_some_and(|t| t.human() > 0)
                    {
                        if let Some(t) = records.team(nation as u16, num) {
                            cup_gate = (
                                num,
                                t.get::<i32>(
                                    0x234 + a3_core::finances::CLUB_TO_TEAM,
                                ),
                            );
                        }
                        let away = u16::from_le_bytes([p[6], p[7]]);
                        cup_away = (p[4], away);
                        for &n in &[num, away] {
                            cup_marks.push((n, totals(records, n)));
                        }
                    }
                }
            }
            // The manager keeps his cup share of the visitor's half of
            // the gate in the black cash box: with the share at 0.3, and
            // the league's share at nought so the league games of these
            // days leave the box alone, the run's black cash gain is the
            // cup's share alone. Note the box and the visitor's cup
            // takings as the patch leaves them.
            if cup_away.1 != 0
                && let Some(h) = records
                    .team(nation as u16, cup_gate.0)
                    .filter(|t| usize::from(t.human()) > 0)
                    .map(|t| usize::from(t.human()) - 1)
                && records
                    .team(u16::from(cup_away.0), cup_away.1)
                    .is_some_and(|t| usize::from(t.human()) == 0)
            {
                records.managers[h].set_cup_share_kept(0.3);
                records.managers[h].set_gate_kept(0.0);
                cup_box = Some((
                    h,
                    records.managers[h].black_cash(),
                    records
                        .team(u16::from(cup_away.0), cup_away.1)
                        .unwrap()
                        .get::<i32>(0x234 + a3_core::finances::CLUB_TO_TEAM),
                ));
            }
            let Some(block) = records.block_mut(&cup) else {
                panic!("the save's cup block");
            };
            for q in 0..32 {
                block[PAIRINGS + q * PAIRING + 8..PAIRINGS + q * PAIRING + 14].fill(0);
            }
            for q in 32..48 {
                block[PAIRINGS + q * PAIRING] = 0;
            }
            block[0x28 - 0x20] = 1;
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let play = "on 427 ok\non 218 click bar 0\non 147 click bar 18\non 302 click bar 18\n\
                    on 384 press 4741\non 308 choose 4095 0\non 96 ok\non 93 press 1943\non 499 ok\non 332 ok\non 2 wait\non 381 wait\non 25 ok\non 161 ok\non 270 ok\nrepeat 10 days";
        let Some(runner) = on_save(&game, "cup-days", play) else { return };
        let state = runner.host.state();
        let (season, records) = (state.season.as_ref().unwrap(), state.records.as_ref().unwrap());
        assert!(season.day > 63, "the days did not play on");
        let nation = u16::try_from(records.globals.main_country()).unwrap();
        let cup = records
            .block(&format!("{CUPS_AT:#x}[{}]", records.countries.slot(usize::from(nation))))
            .expect("the cup block");
        let u16_at = |p: &[u8], at: usize| u16::from_le_bytes([p[at], p[at + 1]]);
        let pairing = |q: usize| &cup[PAIRINGS + q * PAIRING..PAIRINGS + (q + 1) * PAIRING];
        // The first round played out, the flags answering to the score.
        let mut winners: Vec<(u8, u16)> = Vec::new();
        let mut ties = 0;
        for q in 0..32 {
            let p = pairing(q);
            assert_ne!(p[0], 0, "pairing {q} lost its clubs");
            let (hg, ag, flags, winner) = (p[8], p[9], p[0xc], p[0xd]);
            assert_eq!(flags & 0x28, 0x28, "pairing {q}: flags {flags:#02x}");
            assert!(winner < 2, "pairing {q}: winner {winner}");
            if hg == ag {
                assert_eq!(flags & 0xc0, 0xc0, "pairing {q}: {hg}:{ag} flags {flags:#02x}");
            } else {
                assert_eq!(
                    usize::from(winner),
                    usize::from(hg < ag),
                    "pairing {q}: {hg}:{ag} winner {winner}"
                );
            }
            let at = usize::from(winner != 0) * 4;
            winners.push((p[at], u16_at(p, at + 2)));
            ties += usize::from(flags & 0xc0 != 0);
        }
        assert!(ties > 0, "not one of the thirty-two ties went past ninety minutes");
        // The second round: the first's winners, drawn anew, none played.
        let mut drawn: Vec<(u8, u16)> = Vec::new();
        for q in 32..48 {
            let p = pairing(q);
            assert_ne!(p[0], 0, "pairing {q} stayed empty");
            assert_eq!(p[0xc] & 0x20, 0, "pairing {q} played before its day");
            drawn.push((p[0], u16_at(p, 2)));
            drawn.push((p[4], u16_at(p, 6)));
        }
        winners.sort();
        drawn.sort();
        assert_eq!(drawn, winners, "the second round is not the first's winners");
        assert_eq!(cup[0x28 - 0x20], 2, "the block's last drawn round");
        // The human club's tie played with the rest.
        let human = (0..32)
            .filter(|&q| {
                let p = pairing(q);
                [p[0], p[4]].iter().zip([2, 6]).any(|(&c, at)| {
                    records
                        .team(u16::from(c), u16_at(p, at))
                        .is_some_and(|t| t.human() > 0)
                })
            })
            .count();
        assert_eq!(human, 1, "the human club in the first round's pairings");
        // The home tie booked its gate on the club's cup takings (+0x234);
        // the save lived through this round once, so the run books once
        // more atop what it held.
        assert_ne!(cup_gate.0, 0, "the human club played away in round one");
        let booked = records
            .team(nation, cup_gate.0)
            .unwrap()
            .get::<i32>(0x234 + a3_core::finances::CLUB_TO_TEAM);
        assert!(
            booked > cup_gate.1,
            "the cup tie booked no gate: {} -> {booked}",
            cup_gate.1
        );
        // The gate split between the clubs: the home club booked half of
        // it, the visitor the other half less the manager's cup share,
        // and the black cash box gained that share — ftol of the home
        // club's half times the 0.3 the patch set.
        let Some((h, box0, away0)) = cup_box else {
            panic!("no human home tie against a computer's to measure");
        };
        let away_booked = records
            .team(nation, cup_away.1)
            .unwrap()
            .get::<i32>(0x234 + a3_core::finances::CLUB_TO_TEAM);
        let (home, away, kept) = (
            i64::from(booked - cup_gate.1),
            i64::from(away_booked - away0),
            i64::from(records.managers[h].black_cash() - box0),
        );
        assert_eq!(
            away + kept,
            home,
            "the visitor booked {away} and the box kept {kept}, against the home club's half {home}"
        );
        assert_eq!(
            kept,
            i64::from((f64::from(home as i32) * f64::from(0.3f32)) as i32),
            "the manager kept {kept} of the half {home}, not his three tenths"
        );
        // And the tie marked its men: the eleven each side sent out stand
        // in the career's cup appearances (+0x25f), and its goals — all
        // of the tie's, drawn over the eleven — in the season's cup
        // goals (+0x23d) and the career's (+0x23f).
        let q = (0..32)
            .find(|&q| {
                let p = pairing(q);
                usize::from(p[0]) == usize::from(nation) && u16_at(p, 2) == cup_gate.0
            })
            .expect("the human club's home tie");
        let (hg, ag) = (pairing(q)[8], pairing(q)[9]);
        for (num, before) in cup_marks {
            let mut after = (0i64, 0i64, 0i64);
            for id in records.team(nation, num).unwrap().players() {
                if let Some(p) = records.player(id) {
                    after.0 += i64::from(p.goals()[1]);
                    after.1 += i64::from(p.career_cup_goals());
                    after.2 += p
                        .career_cup_appearances()
                        .iter()
                        .map(|&a| i64::from(a))
                        .sum::<i64>();
                }
            }
            let score = i64::from(if num == cup_gate.0 { hg } else { ag });
            assert_eq!(
                after.0 - before.0,
                score,
                "club {num}'s season cup goals: {} -> {}, tie {hg}:{ag}",
                before.0,
                after.0
            );
            assert_eq!(
                after.1 - before.1,
                score,
                "club {num}'s career cup goals: {} -> {}, tie {hg}:{ag}",
                before.1,
                after.1
            );
            assert!(
                after.2 - before.2 >= 11,
                "club {num}'s cup appearances: {} -> {}",
                before.2,
                after.2
            );
        }
        // The human's year calendar kept the cup's days booked from the
        // season's start; the played round fills its day (59) with the
        // new score, its flags saying the result stands (0x20) and the
        // tie a cup one (0x08), and the drawing names the human's next
        // tie on the second round's day (115) — or drops the entry when
        // he lost, as the original zeroes it for a club out of the cup.
        let hum = usize::from(records.team(nation, cup_gate.0).unwrap().human()) - 1;
        let cal = &records.calendars[hum];
        let e = cal.entry(59);
        assert_eq!(e[0], 6, "the round's day left the calendar");
        assert_eq!(
            u32::from_le_bytes([e[4], e[5], e[6], e[7]]),
            0x4008_0001,
            "the round's day lost its cup code"
        );
        let p = pairing(q);
        assert_eq!(e[8], p[0], "the entry's home country");
        assert_eq!(&e[10..12], &p[2..4], "the entry's home club");
        assert_eq!(e[12], p[4], "the entry's away country");
        assert_eq!(&e[14..16], &p[6..8], "the entry's away club");
        assert_eq!(&e[0x10..0x12], &[hg, ag], "the entry's score");
        assert_eq!(e[0x14] & 0x28, 0x28, "the entry's flags {:#02x}", e[0x14]);
        assert_eq!(
            &e[0x16..0x1a],
            &[p[0], 0, p[2], p[3]],
            "the entry's venue"
        );
        let won = p[0xd] == 0;
        let e2 = cal.entry(115);
        match (32..48).find(|&r| {
            let p = pairing(r);
            u16_at(p, 2) == cup_gate.0 || u16_at(p, 6) == cup_gate.0
        }) {
            Some(r) => {
                let p = pairing(r);
                assert!(won, "club {} plays round two without having won", cup_gate.0);
                assert_eq!(e2[0], 6, "the second round's day left the calendar");
                assert_eq!(
                    u32::from_le_bytes([e2[4], e2[5], e2[6], e2[7]]),
                    0x4008_0002,
                    "the second round's day lost its cup code"
                );
                assert_eq!(e2[8], p[0], "the entry's home country");
                assert_eq!(&e2[10..12], &p[2..4], "the entry's home club");
                assert_eq!(e2[12], p[4], "the entry's away country");
                assert_eq!(&e2[14..16], &p[6..8], "the entry's away club");
                assert_eq!(&e2[0x10..0x14], &[0, 0, 0, 0], "the entry already holds a result");
                assert_eq!(
                    &e2[0x16..0x1a],
                    &[p[0], 0, p[2], p[3]],
                    "the entry's venue"
                );
            }
            None => {
                assert!(!won, "club {} won and vanished from round two", cup_gate.0);
                assert_eq!(e2[0], 0, "the out club kept its cup day");
            }
        }
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn spiel_beenden_ends_the_game() {
        let game = game();
        // The dressing room's game options: "Spiel beenden", and yes.
        let then = "now click bar 17\nuntil dialog 302\nnow choose 4035 4\nuntil dialog 384\n\
                    now press 4741\nuntil end";
        // `until end` holds only once the game cycle has ended: after the
        // question, the dressing room it closes with.
        let Some(runner) = on_save(&game, "Probe1", then) else { return };
        let screens: Vec<&str> =
            runner.log.iter().filter_map(|l| l.strip_prefix("screen ")).collect();
        assert_eq!(screens[screens.len() - 3..], ["302", "384", "302"]);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn neues_spiel_goes_back_to_the_start_menu() {
        let game = game();
        // The dressing room's game options: "Neues Spiel", and yes.
        let then = "now click bar 17\nuntil dialog 302\nnow choose 4035 3\nuntil dialog 384\n\
                    now press 4741\nuntil dialog 9";
        let Some(runner) = on_save(&game, "Probe1", then) else { return };
        let screens: Vec<&str> =
            runner.log.iter().filter_map(|l| l.strip_prefix("screen ")).collect();
        // The dressing room closes after its question, and the game cycle
        // with it: the start menu is next.
        assert_eq!(screens[screens.len() - 4..], ["302", "384", "302", "9"]);
        // The game was loaded by the second entry; the menu opens on the
        // first again, as the original's does.
        let entries = crate::screens::control(&game.dialogs[&9], 4771).unwrap();
        assert_eq!(runner.host.view().chosen(entries), 0);
    }

    /// "Musik & Soundeffekte" (dialog 259): opened on the oracle's
    /// `musik.cfg` (all in turn, sounds on, 0x3ffb: every listed piece but
    /// "Go West" red, as on the original); a click on a piece puts it out,
    /// OK keeps that, cancel keeps nothing.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_music_keeps_its_pieces() {
        let game = game();
        let dialog = &game.dialogs[&259];
        let shown = |runner: &Runner| {
            let view = runner.host.view();
            let list = view.list(control(dialog, 6453).unwrap()).unwrap();
            let on: Vec<bool> = list.inks.iter().map(Option::is_some).collect();
            (list.rows.len(), view.chosen(control(dialog, 6451).unwrap()), on)
        };
        let mut from_file = vec![true; 14];
        from_file[13] = false;
        let open = "now click bar 17\nuntil dialog 302\nnow choose 4035 1\nuntil dialog 259";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        assert_eq!(shown(&runner), (14, 2, from_file.clone()));
        let changed = format!("{open}\nnow row 6453 0\nnow choose 6451 3");
        let Some(runner) = on_save(&game, "Probe1", &changed) else { return };
        assert!(!shown(&runner).2[0]);
        let again = "until dialog 302\nnow choose 4035 1\nuntil dialog 259";
        let kept = format!("{changed}\nnow press 1\n{again}");
        let Some(runner) = on_save(&game, "Probe1", &kept) else { return };
        let mut on = from_file.clone();
        on[0] = false;
        assert_eq!(shown(&runner), (14, 3, on));
        // What OK set waits there for the music: "1987" out, "Go West" too.
        assert_eq!(runner.host.state().music, Some((3, 1, 0x3ffb & !1)));
        // And musik.cfg's bytes, for the end.
        let kept = vec![("musik.cfg", vec![3, 0, 1, 0, 0xfa, 0x3f, 0, 0])];
        assert_eq!(runner.host.state().settings_files, kept);
        let dropped = format!("{changed}\nnow press 2\n{again}");
        let Some(runner) = on_save(&game, "Probe1", &dropped) else { return };
        assert_eq!(shown(&runner), (14, 2, from_file.clone()));
    }

    /// "Musikstücke anspielen": pressed, with the chosen pieces' mode, a
    /// piece switched on is to be heard at once; not pressed, it is not.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_stereo_plays_a_piece_switched_on() {
        let game = game();
        let open = "now click bar 17\nuntil dialog 302\nnow choose 4035 1\nuntil dialog 259";
        // "Go West", off in the oracle's musik.cfg, is the last row.
        let heard = format!("{open}\nnow choose 6452 0\nnow choose 6451 3\nnow row 6453 13");
        let Some(runner) = on_save(&game, "Probe1", &heard) else { return };
        assert_eq!(runner.host.state().listen, Some(15));
        let quiet = format!("{open}\nnow choose 6451 3\nnow row 6453 13");
        let Some(runner) = on_save(&game, "Probe1", &quiet) else { return };
        assert_eq!(runner.host.state().listen, None);
    }

    /// The week planner's announcement (dialog 358) on Toni.an3, whose
    /// league match at Wolfsburg is within the week: every line as the
    /// original shows it.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_planner_announces_the_next_league_match() {
        let game = game();
        let then = "now click bar 7\nuntil dialog 413\nnow press 6215\nuntil dialog 358";
        let Some(runner) = on_save(&game, "Toni", then) else { return };
        let view = runner.host.view();
        let dialog = &game.dialogs[&358];
        let text = |id| view.text(control(dialog, id).unwrap()).to_owned();
        let shown: Vec<String> =
            [2973, 2974, 2975, 2976, 2978, 2977, 2980, 2979, 2981, 2982, 2983, 2984, 2985]
                .into_iter()
                .map(text)
                .collect();
        assert_eq!(
            shown,
            [
                "1. Bundesliga 2025/26",
                "3. Spieltag",
                "VfL Wolfsburg",
                "sN",
                "(9.)",
                "1.FC Union Berlin",
                "Sn",
                "(6.)",
                "-",
                "-",
                "0:0:0",
                "98 : 97",
                "24.524",
            ]
        );
        let rows = &view.list(control(dialog, 4570).unwrap()).unwrap().rows;
        assert_eq!(rows.len(), 9);
        assert_eq!(rows[0], "Bayer 04 Leverkusen (7.)\t-\t1.FSV Mainz 05 (3.)");
    }

    /// With the next match more than a week away (Probe1) the button does
    /// nothing, as the original's.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_announcement_waits_for_the_week_of_the_match() {
        let game = game();
        let then = "now click bar 7\nuntil dialog 413\nnow press 6215";
        let Some(runner) = on_save(&game, "Probe1", then) else { return };
        assert_eq!(runner.host.view().id, 413);
    }

    fn notice_text(game: &Game, runner: &Runner) -> String {
        runner.host.view().text(control(&game.dialogs[&380], 4730).unwrap()).to_owned()
    }

    /// "Vertrag verlängern" in the current squad (216): on Probe1 a talk
    /// is booked in the week planner (kind 0x0c, into the extension
    /// talks' list too), as the original books Rönnow's on A.an3.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn an_extension_talk_is_booked() {
        let game = game();
        let open = "now click bar 10\nuntil dialog 218\nnow choose 5042 0\nuntil dialog 216\n\
                    now choose 5657 0";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let list = control(&game.dialogs[&216], 3298).unwrap();
        let keys = runner.host.view().list(list).unwrap().keys.clone();
        drop(runner);
        // The first row that leads to the planner.
        let row = (0..keys.len()).find(|&row| {
            let then = format!("{open}\nnow row 3298 {row}");
            on_save(&game, "Probe1", &then).is_some_and(|r| r.host.view().id == 412)
        });
        let Some(row) = row else { panic!("nobody to talk to") };
        let then = format!(
            "{open}\nnow row 3298 {row}\nuntil dialog 412\nnow choose 5034 1\nuntil dialog 216"
        );
        let Some(runner) = on_save(&game, "Probe1", &then) else { return };
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let day = usize::from(records.globals.day()) + 1;
        let booked = records.planners[0].day(day);
        let season = state.season.as_ref().unwrap();
        let club = &season.clubs[&season.manager().unwrap().club.unwrap()];
        assert_eq!((booked.kind, booked.player), (0x0c, club.status[keys[row]].id));
        assert!(a3_core::transfers::pending(records, booked.player));
    }

    /// "Auf Transferliste setzen" on Probe1: the first player who asks
    /// for something (205) goes onto the list on "Einverstanden", his
    /// demand kept; clicked again he is asked about (4072) and taken off.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_player_goes_onto_the_transfer_list_and_off() {
        let game = game();
        let open = "now click bar 10\nuntil dialog 218\nnow choose 5042 0\nuntil dialog 216\n\
                    now choose 5657 1";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let list = control(&game.dialogs[&216], 3298).unwrap();
        let rows = runner.host.view().list(list).unwrap().rows.len();
        drop(runner);
        let asks = |row: usize| {
            let then = format!("{open}\nnow row 3298 {row}");
            on_save(&game, "Probe1", &then).is_some_and(|r| r.host.view().id == 205)
        };
        let Some(row) = (0..rows).find(|&row| asks(row)) else { panic!("nobody asks") };
        let agreed = format!(
            "{open}\nnow row 3298 {row}\nuntil dialog 205\nnow choose 3022 0\nuntil dialog 216"
        );
        let Some(runner) = on_save(&game, "Probe1", &agreed) else { return };
        let id = {
            let state = runner.host.state();
            let season = state.season.as_ref().unwrap();
            let club = &season.clubs[&season.manager().unwrap().club.unwrap()];
            let key = runner.host.view().list(list).unwrap().keys[row];
            let id = club.status[key].id;
            let records = state.records.as_ref().unwrap();
            assert_eq!(a3_core::transfers::listed_terms(records, id), Some(1));
            assert!(records.player(id).unwrap().get::<u16>(0x1f0) > 0);
            id
        };
        drop(runner);
        let off = format!(
            "{agreed}\nnow row 3298 {row}\nuntil dialog 384\nnow press 4741\nuntil dialog 216"
        );
        let Some(runner) = on_save(&game, "Probe1", &off) else { return };
        let state = runner.host.state();
        assert_eq!(a3_core::transfers::listed_terms(state.records.as_ref().unwrap(), id), None);
    }

    /// "Verleihen" on Probe1, as the original: Schmitt gets the loan's
    /// conditions (292) at the fair prices — 340.000 € with an option of
    /// 1.80 Mio €, 230.000 € without — and OK lists him with the option;
    /// Ortag will not go.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_player_is_listed_for_a_loan() {
        let game = game();
        let open = "now click bar 10\nuntil dialog 218\nnow choose 5042 0\nuntil dialog 216\n\
                    now choose 5657 2";
        let ortag = format!("{open}\nnow row 3298 1\nuntil dialog 380");
        let Some(runner) = on_save(&game, "Probe1", &ortag) else { return };
        assert!(notice_text(&game, &runner).contains("nicht bereit, sich verleihen"));
        drop(runner);
        let schmitt = format!("{open}\nnow row 3298 0\nuntil dialog 292");
        let Some(runner) = on_save(&game, "Probe1", &schmitt) else { return };
        let dialog = &game.dialogs[&292];
        let text = |id| runner.host.view().text(control(dialog, id).unwrap()).to_owned();
        let shown = [text(3963), text(3967), text(3968)];
        assert_eq!(shown, ["340.000 €", "1.80 Mio €", "230.000 €"]);
        drop(runner);
        let ok = format!("{schmitt}\nnow press 3969\nuntil dialog 216");
        let Some(runner) = on_save(&game, "Probe1", &ok) else { return };
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let season = state.season.as_ref().unwrap();
        let club = &season.clubs[&season.manager().unwrap().club.unwrap()];
        let list = control(&game.dialogs[&216], 3298).unwrap();
        let id = club.status[runner.host.view().list(list).unwrap().keys[0]].id;
        let listings = a3_core::transfers::listings(records);
        let listing = listings.into_iter().find(|l| l.id == id).unwrap();
        assert_eq!((listing.kind, listing.fee, listing.option), (3, 340_000, 1_800_000));
    }

    /// Doping (dialog 394) on Probe1, as the original shows it: Ortag
    /// ready, Schmitt not; "Kondition steigern" doses Ortag at once — doped,
    /// his condition ten up, the dose on the medical bill — and a second
    /// dose is refused; Schmitt will not even go on the list.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn doping_doses_who_is_ready() {
        let game = game();
        let open = "now choose 5266 6\nuntil dialog 151\nnow press 4929\nuntil dialog 394";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let dialog = &game.dialogs[&394];
        let list = control(dialog, 4926).unwrap();
        let rows = runner.host.view().list(list).unwrap().rows.clone();
        assert!(rows[0].starts_with("Schmitt, M.") && rows[0].ends_with("\tNein"), "{}", rows[0]);
        assert!(rows[1].starts_with("Ortag, C.") && rows[1].ends_with("\tJa"), "{}", rows[1]);
        let ortag = runner.host.view().list(list).unwrap().keys[1] as u16;
        let before = {
            let state = runner.host.state();
            let r = state.records.as_ref().unwrap().player(ortag).unwrap();
            (r.condition(), r.doping())
        };
        drop(runner);
        let dose = format!("{open}\nnow choose 4928 1\nnow cell 4926 1 0\nuntil dialog 380");
        let Some(runner) = on_save(&game, "Probe1", &dose) else { return };
        let notice = notice_text(&game, &runner);
        assert_eq!(notice, game.text(4071).replacen("%s", "Christian Ortag", 1));
        let state = runner.host.state();
        let r = state.records.as_ref().unwrap().player(ortag).unwrap();
        assert_eq!(r.condition(), before.0 + 1000);
        assert!(a3_core::doping::doped(r));
        assert_eq!(r.doping() >> 2 & 3, 2);
        assert_eq!(before.1 & 3, 0);
        drop(state);
        let again = format!(
            "{dose}\nnow press 4731\nuntil dialog 394\n\
             now cell 4926 1 0\nuntil dialog 380"
        );
        let Some(runner) = on_save(&game, "Probe1", &again) else { return };
        let notice = notice_text(&game, &runner);
        assert_eq!(notice, game.text(4070).replacen("%s", "Christian Ortag", 1));
        let list_schmitt = format!("{open}\nnow cell 4926 0 6\nuntil dialog 380");
        let Some(runner) = on_save(&game, "Probe1", &list_schmitt) else { return };
        let notice = notice_text(&game, &runner);
        assert!(notice.starts_with("Ihr Spieler") && notice.contains("Dopingliste"), "{notice}");
    }

    /// The template editor (dialog 350) from a right click on the short
    /// training's first template: a unit clicked fills Sunday's first
    /// session and moves on; switching to the second template on the
    /// combo keeps the first in the manager and his record, the session
    /// to set staying where it was (Sunday's second), and OK keeps the
    /// second's change.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_template_editor_keeps_the_weeks() {
        let game = game();
        let categories = &game.data.rules.training.categories;
        let (first, second) = (categories[0].units[3].1, categories[1].units[0].1);
        let then = "now click bar 4\nuntil dialog 346\nnow rclick 5035 item 0\nuntil dialog 350\n\
                    now click 35003\nnow row 5651 1\nnow click 35100\n\
                    now press 1\nuntil dialog 346";
        let Some(runner) = on_save(&game, "Probe1", then) else { return };
        let state = runner.host.state();
        let season = state.season.as_ref().unwrap();
        let templates = &season.managers[season.current].templates;
        assert_eq!((templates[0].week[0], templates[1].week[1]), (first, second));
        let records = state.records.as_ref().unwrap();
        let slot = records.human_slot(season.current).unwrap();
        let kept = records.managers[slot].templates();
        assert_eq!((kept[0].1[0], kept[1].1[1]), (first, second));
        assert_eq!(kept[1].0, templates[1].name);
    }

    /// The A youth's tables (dialog 430) on Probe1, before its first
    /// matchday: the first row's pairings without results, every team at
    /// nought in the order of its slot, the first yellow and the last three
    /// purple, three league tabs and no fourth — as the original shows it.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_youth_tables_show_the_leagues() {
        let game = game();
        let then = "now click bar 10\nuntil dialog 218\nnow choose 5042 2\nuntil dialog 428\n\
                    now choose 5271 0\nuntil dialog 430";
        let Some(runner) = on_save(&game, "Probe1", then) else { return };
        let view = runner.host.view();
        let dialog = &game.dialogs[&430];
        let results = view.list(control(dialog, 3728).unwrap()).unwrap();
        assert_eq!(
            results.rows[0],
            "FC Bayern München\t- 1.FC Heidenheim\t\t\t\t\tSC Freiburg\t- TSG 1899 Hoffenheim"
        );
        let table = view.list(control(dialog, 3729).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 18);
        let zero = "0\t0\t0\t:\t0";
        assert_eq!(table.rows[0], format!("1.\tFC Bayern München\t{zero}\t{zero}\t{zero}"));
        assert_eq!(table.inks[0], Some([200, 200, 0]));
        assert_eq!(table.inks[15..], [Some([128, 0, 128]); 3]);
        let tabs = view.options(control(dialog, 3723).unwrap()).unwrap();
        assert_eq!(tabs.off, [false, false, false, true]);
    }

    /// "Wie oft & Wie lange" (dialog 158): the counts of the oracle's
    /// zeit.saq with this start counted in, its longest session, and the
    /// game's play time as Probe1 was saved with it, counting on.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_play_time_counts_from_the_files() {
        let game = game();
        let words = std::fs::read(game.dir.join("zeit.saq")).ok();
        let word = |i: usize| {
            words.as_ref().map(|w| u32::from_le_bytes(w[4 * i..4 * i + 4].try_into().unwrap()))
        };
        let then = "now click bar 17\nuntil dialog 302\nnow choose 4035 8\nuntil dialog 158";
        let Some(runner) = on_save(&game, "Probe1", then) else { return };
        let view = runner.host.view();
        let dialog = &game.dialogs[&158];
        let text = |id| view.text(control(dialog, id).unwrap()).to_owned();
        assert_eq!(text(2687), word(0).map_or(1, |w| w + 1).to_string());
        assert_eq!(text(2689), word(1).unwrap_or(1).to_string());
        let longest = word(3).unwrap_or(0);
        assert_eq!(text(6412), a3_core::playtime::split(longest).1);
        // Probe1 was saved after 184 seconds of play: the original's own
        // clock, so another oracle run of the same game stores another.
        assert!(text(2691).starts_with("00:03:0"), "{}", text(2691));
        assert_eq!(text(2686), "");
    }

    /// "Optionen" (dialog 503): OK writes the switches into the globals'
    /// block, the start screen into the manager and the difficulty once,
    /// locked from then on; cancel writes nothing.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_options_write_what_ok_keeps() {
        let game = game();
        let open = "now click bar 17\nuntil dialog 302\nnow choose 4035 6\nuntil dialog 503";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let before = records.globals.game_options();
        let slot = records.human_slot(state.season.as_ref().unwrap().current).unwrap();
        let difficulty = records.managers[slot].difficulty();
        assert_eq!(difficulty & 0x80, 0);
        drop(state);
        // Tabellen twice (an, alle, aus), Spieldarstellung, the weeks up,
        // the start screen and the difficulty one on.
        let clicks = "now press 6328\nnow press 6328\nnow press 6336\nnow spin 6348 up\n\
                      now press 6346\nnow press 6352";
        let kept = format!("{open}\n{clicks}\nnow press 1\nuntil dialog 302");
        let Some(runner) = on_save(&game, "Probe1", &kept) else { return };
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let mut after = before;
        after[0x08] = 0;
        after[0x10] = 0x21;
        after[0x1f] = before[0x1f] + 1;
        assert_eq!(records.globals.game_options(), after);
        assert_eq!(records.globals.last_display(), 0x21);
        assert_eq!(records.managers[slot].menu_view(), 1);
        assert_eq!(records.managers[slot].difficulty(), (difficulty & 0x7f) + 1 | 0x80);
        let season = state.season.as_ref().unwrap();
        assert_eq!(season.managers[season.current].menu, 1);
        drop(state);
        // Opened again, the difficulty's button no longer answers.
        let again = format!("{kept}\nnow choose 4035 6\nuntil dialog 503");
        let Some(runner) = on_save(&game, "Probe1", &again) else { return };
        let button = control(&game.dialogs[&503], 6352).unwrap();
        assert!(runner.host.view().disabled(button));
        drop(runner);
        let dropped = format!("{open}\n{clicks}\nnow press 2\nuntil dialog 302");
        let Some(runner) = on_save(&game, "Probe1", &dropped) else { return };
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        assert_eq!(records.globals.game_options(), before);
        assert_eq!(records.managers[slot].difficulty(), difficulty);
    }

    /// "3D - Optionen" (dialog 511): open on the original's values, OK
    /// keeps a change for the next opening, cancel drops one.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_3d_options_keep_what_ok_keeps() {
        let game = game();
        let dialog = &game.dialogs[&511];
        let shown = |runner: &Runner| {
            let view = runner.host.view();
            let at = |id| control(dialog, id).unwrap();
            let checks: Vec<bool> = [6472, 6473, 6474, 6467, 6468]
                .into_iter()
                .map(|id| view.check(at(id)).unwrap().checked)
                .collect();
            (view.chosen(at(6466)), view.chosen(at(6475)), checks)
        };
        let open = "now click bar 17\nuntil dialog 302\nnow choose 4035 7\nuntil dialog 511";
        let Some(runner) = on_save(&game, "Probe1", open) else { return };
        assert_eq!(shown(&runner), (2, 2, vec![true; 5]));
        let changed = format!("{open}\nnow choose 6466 0\nnow click 6473");
        let again = "until dialog 302\nnow choose 4035 7\nuntil dialog 511";
        let kept = format!("{changed}\nnow press 1\n{again}");
        let Some(runner) = on_save(&game, "Probe1", &kept) else { return };
        assert_eq!(shown(&runner), (0, 2, vec![true, false, true, true, true]));
        // Konfig3D.cfg's bytes for the end: the figures' quality (word 3)
        // and Schatten (word 8) changed, the rest as the file has it.
        let mut file = std::fs::read(game.dir.join("Konfig3D.cfg")).unwrap();
        file[12..16].copy_from_slice(&0u32.to_le_bytes());
        file[32..36].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(runner.host.state().settings_files, [("Konfig3D.cfg", file)]);
        let dropped = format!("{changed}\nnow press 2\n{again}");
        let Some(runner) = on_save(&game, "Probe1", &dropped) else { return };
        assert_eq!(shown(&runner), (2, 2, vec![true; 5]));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn neues_spiel_lights_its_entry_until_answered() {
        let game = game();
        // The dressing room's view, under the question or alone.
        let lit = |runner: &Runner| {
            let entries = crate::screens::control(&game.dialogs[&302], 4035).unwrap();
            let room = runner.host.views().find(|v| v.id == 302).unwrap();
            room.chosen(entries)
        };
        // Lit under its question, as the original's pressed entry is; so
        // is "Spiel beenden" under its own.
        let ask = "now click bar 17\nuntil dialog 302\nnow choose 4035 3\nuntil dialog 384";
        let Some(runner) = on_save(&game, "Probe1", ask) else { return };
        assert_eq!(lit(&runner), 3);
        let Some(runner) = on_save(&game, "Probe1", &ask.replace("4035 3", "4035 4")) else {
            return;
        };
        assert_eq!(lit(&runner), 4);
        // No keeps the dressing room, the entry dark again.
        let no = format!("{ask}\nnow press 4739\nuntil dialog 302");
        let Some(runner) = on_save(&game, "Probe1", &no) else { return };
        assert_eq!(lit(&runner), crate::controls::options::NONE);
    }

    /// "Spielstand speichern" (dialog 489): the saved games stand in its
    /// list with their extension, a row names itself in the field, a fresh
    /// name writes at once (".an3" completed), and a taken one asks —
    /// text 4593 — before yes writes it, "Abbruch" leaving the old file
    /// and the dialog standing.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_save_dialog_writes_what_ok_names() {
        let game = game();
        // A copy of the installation with a save of its own, so the
        // dialog may write without touching the original's saves.
        let root = patched(&game.dir, "Probe1", "Nachtprobe", |_| {});
        let mut copy = Game::load(&root).unwrap();
        compose(&mut copy);
        let dialog = &copy.dialogs[&489];
        let field = control(dialog, 1866).unwrap();
        let open = "now click bar 17\nuntil dialog 302\nnow choose 4035 2\nuntil dialog 489";
        // It opens with no row chosen and the field empty, focused for a
        // new name; the rows carry their extension.
        let Some(runner) = on_save(&copy, "Nachtprobe", open) else { return };
        let view = runner.host.view();
        let list = view.list(control(dialog, 5248).unwrap()).unwrap();
        assert_eq!(list.rows.len(), 1);
        assert!(list.rows[0].starts_with("Nachtprobe.an3\t"), "{:?}", list.rows);
        assert_eq!(list.tabs, [0.0, 270.0, 325.0]);
        assert_eq!(list.selected, None);
        assert_eq!(view.focus, Some(field));
        assert_eq!(view.text(field), "");
        // A click on the row puts its name, extension and all, into the
        // field. (Before the write runs below, while it is still the only
        // save and the first row.)
        let named = format!("{open}\nnow row 5248 0\nuntil dialog 489");
        let Some(runner) = on_save(&copy, "Nachtprobe", &named) else { return };
        assert_eq!(runner.host.view().text(field), "Nachtprobe.an3");
        // OK saves it under a fresh name — the ".an3" completed — and
        // lands back on 302.
        let fresh = format!("{open}\nnow type 1866 Nachtzwei\nnow ok\nuntil dialog 302");
        let Some(runner) = on_save(&copy, "Nachtprobe", &fresh) else { return };
        let _ = runner;
        assert!(std::fs::metadata(root.join("saves/Nachtzwei.an3")).is_ok());
        // A taken name asks first; "Abbruch" answers the question and
        // leaves 489 standing, the old file untouched, and cancel then
        // gives the save up.
        let old = root.join("saves/Nachtprobe.an3");
        let kept = std::fs::metadata(&old).unwrap().modified().unwrap();
        let asked = format!("{open}\nnow type 1866 Nachtprobe\nnow ok\nuntil dialog 384");
        let Some(runner) = on_save(&copy, "Nachtprobe", &asked) else { return };
        let question = &copy.dialogs[&384];
        assert_eq!(
            runner.host.view().text(control(question, 4738).unwrap()),
            copy.text(4593)
        );
        let dropped = format!("{asked}\nnow press 4739\nnow cancel\nuntil dialog 302");
        let Some(runner) = on_save(&copy, "Nachtprobe", &dropped) else { return };
        assert_eq!(runner.host.view().id, 302);
        assert_eq!(std::fs::metadata(&old).unwrap().modified().unwrap(), kept);
        // Yes takes the name over and writes.
        let taken = format!("{asked}\nnow press 4741\nuntil dialog 302");
        let Some(runner) = on_save(&copy, "Nachtprobe", &taken) else { return };
        assert_eq!(runner.host.view().id, 302);
        assert!(std::fs::metadata(&old).unwrap().modified().unwrap() > kept);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_dragged_player_goes_with_the_pointer() {
        let game = game();
        let Some(runner) = on_save(&game, "Probe1", "") else { return };
        let dialog = &game.dialogs[&147];
        let mut view = runner.host.view().clone();
        let list = crate::screens::control(dialog, 2509).unwrap();
        let at = bevy_math::Vec2::new(300.0, 120.0);
        let draw = |view: &crate::view::View| {
            let background = crate::classic::picture(&game, dialog);
            crate::classic::Classic::new(&game, dialog, view, background)
                .draw(dialog, view, false, 0.0)
        };
        let before = draw(&view);
        view.dragging = Some(crate::view::Dragging {
            grip: crate::pointer::Grip::Row { control: list, row: 3 },
            at,
            from: at,
        });
        let after = draw(&view);
        // The box's picture lies under the pointer, and nowhere else changed.
        let changed = |x: u32, y: u32| before.get_pixel(x, y) != after.get_pixel(x, y);
        assert!(changed(at.x as u32, at.y as u32));
        assert!(!changed(at.x as u32 + 80, at.y as u32));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_booked_talk_comes_on_its_day() {
        let game = game();
        // The talk booked for today, the turn ended with the whistle and
        // yes to no training: the week's first day brings the talk; the
        // keeper is praised.
        // The other manager's turn, in the dressing room, ends the same.
        let then = "now rclick 2510; choose 9950 1\nuntil dialog 412\nnow choose 5034 0\n\
                    until dialog 147\non 302 click bar 18\non 384 press 4741\non 2 wait\n\
                    now click bar 18\nuntil dialog 461\nnow choose 5839 0";
        let Some(runner) = on_save(&game, "Probe1", then) else { return };
        let view = runner.host.view();
        let dialog = &game.dialogs[&461];
        let text = |id| view.text(crate::screens::control(dialog, id).unwrap()).to_owned();
        assert_eq!(text(5833), "Max Schmitt");
        assert!(!text(1416).is_empty(), "his reaction");
        // One topic a talk.
        let topics = view.options(crate::screens::control(dialog, 5839).unwrap()).unwrap();
        assert!(topics.off.iter().all(|&off| off));
    }

    /// Where the cycle goes on after loading a game saved during a
    /// matchday's matches.
    const MATCHES_RESUMED: u16 = 0x3e6;

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_co_trainer_lines_up_a_resumed_matchday() {
        let game = game();
        // Saved during a matchday, with "Automatische Aufstellung freies
        // System" ticked: the original opened it on this eleven and bench
        // (`tools/oracle`, 13.10.2026).
        let Some(runner) = on_save(&game, "Toni", "") else { return };
        let state = runner.host.state();
        let season = state.season.as_ref().unwrap();
        let records = state.records.as_ref().unwrap();
        let club = &season.clubs[&season.manager().unwrap().club.unwrap()];
        let name = |p: Option<usize>| {
            p.map_or_else(String::new, |p| {
                records.name(records.player(club.status[p].id).unwrap().last_name()).to_owned()
            })
        };
        let eleven: Vec<String> = club.lineup.field.iter().map(|&p| name(p)).collect();
        let bench: Vec<String> = club.lineup.bench.iter().map(|&p| name(p)).collect();
        assert_eq!(
            eleven,
            [
                "Rönnow",
                "Leite",
                "Nsoki",
                "Juranovic",
                "Moore",
                "Skov",
                "Moscardo",
                "Lubiniecki",
                "Haberer",
                "Burke",
                "Ljubicic"
            ]
        );
        assert_eq!(bench, ["Raab", "Král", "Schäfer", "Querfeld", "Kemlein", "Khedira", "Köhn"]);
    }

    /// A page, a control of it, and the text it shows.
    type Shown = (u16, u16, &'static str);

    /// What the original's other pages showed of the first player of the
    /// line-up, in the gate's save and in one a few matchdays on
    /// (`tools/oracle`, 13.10.2026).
    const PAGES_OF_THE_ORIGINAL: [(&str, &[Shown]); 2] = [
        (
            "Probe1",
            &[
                (402, 4879, "9.0"),
                (402, 4880, "9"),
                (402, 4882, "-"),
                (402, 4875, "-"),
                (400, 5019, "1"),
                (400, 5029, "Im Verein seit 2025."),
                (401, 5648, "Freundin"),
                (401, 4961, "Karin"),
                (401, 4962, "zusammen"),
                (401, 4977, "Familie"),
                (401, 6003, "Er ist zufrieden. Positiv: toller Vertrag"),
            ],
        ),
        (
            "Toni",
            &[
                (402, 4862, "2"),
                (402, 4879, "15.0"),
                (402, 4880, "16"),
                (402, 4881, "12"),
                (402, 4882, "4.0"),
                (400, 5009, "2"),
                (400, 5017, "1"),
                (400, 5021, "Tore England"),
                (401, 5648, "Ehefrau"),
                (401, 4961, "Tatjana"),
                (401, 4969, "101"),
                (401, 4964, "98"),
                (401, 6003, "Er ist zufrieden."),
            ],
        ),
    ];

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_player_infos_pages_show_what_the_originals_do() {
        let game = game();
        for (save, expected) in PAGES_OF_THE_ORIGINAL {
            let Some(row) = game.save_games().iter().position(|s| s.name == save) else {
                eprintln!("no {save} among the saves; the oracle's copy has it");
                continue;
            };
            let load = LOAD.replace("row 5144 0", &format!("row 5144 {row}"));
            let mut script =
                format!("{}\nnow rclick 2510; choose 9950 0\nuntil dialog 399", load.trim_end());
            for page in [402, 400, 401] {
                script += &format!("\nnow choose 4889 1\nuntil dialog {page}");
                let (runner, result) = Runner::play(&game, &Script::parse(&script).unwrap());
                result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
                let view = runner.host.view();
                let dialog = &game.dialogs[&page];
                for &(_, id, text) in expected.iter().filter(|(p, ..)| *p == page) {
                    // Some of the game's texts end in spaces ("Ehefrau        ").
                    let shown = view.text(crate::screens::control(dialog, id).unwrap());
                    assert_eq!(shown.trim_end(), text, "{save}, page {page}, control {id}");
                }
            }
        }
    }

    /// The honours' three small personal pages on the save "A", as the
    /// original showed them (ERFOLGE.md §A.7).
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_honours_pages_show_pay_records_and_scores() {
        use crate::screens::playerinfo::money;
        let game = game();
        let to_pages = "now click bar 16\nuntil dialog 175";
        // The pay: one running season at 88.500 €, its two lines below.
        let then = format!("{to_pages}\nnow choose 5643 4\nuntil dialog 84");
        let Some(runner) = on_save(&game, "A", &then) else { return };
        let view = runner.host.view();
        let dialog = &game.dialogs[&84];
        let list = view.list(control(dialog, 5248).unwrap()).unwrap();
        assert_eq!(list.rows, [format!("2025/2026\t{}", money(&game, 88_500))]);
        assert_eq!(view.text(control(dialog, 1860).unwrap()), money(&game, 88_500));
        // Half a year had passed since July, hence half the yearly sum.
        assert_eq!(view.text(control(dialog, 5591).unwrap()), money(&game, 531_000));
        // The season records: no save yet at hand has earned one.
        let then = format!("{to_pages}\nnow choose 5643 7\nuntil dialog 174");
        let runner = on_save(&game, "A", &then).unwrap();
        let view = runner.host.view();
        let dialog = &game.dialogs[&174];
        assert!(view.list(control(dialog, 2830).unwrap()).unwrap().rows.is_empty());
        // The highscores: the careers of the installation, five on the
        // oracle's copy as setup.sh makes it, none of the fixed games kept
        // there (an oracle run that ends a career adds its own).
        let then = format!("{to_pages}\nnow choose 5643 8\nuntil dialog 95");
        let runner = on_save(&game, "A", &then).unwrap();
        let view = runner.host.view();
        let dialog = &game.dialogs[&95];
        if game.high_entries(9).is_empty() {
            eprintln!("no high09 in this installation; the oracle's copy has it");
        } else {
            let rows = &view.list(control(dialog, 2010).unwrap()).unwrap().rows;
            assert_eq!(rows.len(), 5);
            let expected = [
                "1.\tToni Lubiniecki\t18\t5880\t5880\t9\t0\t24.8.2026",
                "2.\tToni Lubiniecki\t18\t5880\t5880\t9\t0\t26.9.2026",
                "3.\tToni Lubiniecki\t18\t5880\t5880\t9\t0\t26.9.2026",
                "4.\tToni Lubiniecki\t18\t5880\t5880\t9\t0\t26.9.2026",
                "5.\tToni Lubiniecki\t18\t5880\t5880\t9\t0\t26.9.2026",
            ];
            assert_eq!(rows.join("\n"), expected.join("\n"));
        }
        assert!(view.list(control(dialog, 2011).unwrap()).unwrap().rows.is_empty());
    }

    /// Item 3 of the honours' pages: on the save "A" the manager is a
    /// player-manager already — stepping down takes him out of the squad
    /// and out of the label, and becoming one again puts him back where
    /// his position sorts him (`FUN_0053f3be`).
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_manager_steps_down_and_becomes_a_player_again() {
        let game = game();
        let open = "now click bar 16\nuntil dialog 175";
        let pages = control(&game.dialogs[&175], 5643).unwrap();
        let manager = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            records.humans().next().unwrap().playing()
        };
        let squad = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let m = records.humans().next().unwrap();
            records.team(u16::from(m.country()), m.team()).unwrap().players()
        };
        let label = |runner: &Runner| runner.host.view().options(pages).unwrap().labels[3].clone();
        // As he stands: player-manager, in the squad, and so labelled.
        let Some(before) = on_save(&game, "A", open) else { return };
        let standing = squad(&before);
        assert!(standing.contains(&17001));
        assert_eq!(manager(&before), 1);
        assert_eq!(label(&before), game.text(4246));
        // He steps down: out of the squad, the flag off, the label asks
        // him to become one.
        let down = format!("{open}\nnow choose 5643 3\nuntil dialog 384\nnow ok");
        let runner = on_save(&game, "A", &down).unwrap();
        let ids = squad(&runner);
        assert!(!ids.contains(&17001));
        assert_eq!(ids.len(), standing.len() - 1);
        assert_eq!(manager(&runner), 0);
        assert_eq!(label(&runner), game.text(4290));
        // And he becomes one again: the very squad as before.
        let up = format!("{down}\nnow choose 5643 3\nuntil dialog 384\nnow ok");
        let runner = on_save(&game, "A", &up).unwrap();
        assert_eq!(squad(&runner), standing);
        assert_eq!(manager(&runner), 1);
        assert_eq!(label(&runner), game.text(4246));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_trainer_sees_his_titles_and_stations() {
        let game = game();
        let then = "now click bar 16\nuntil dialog 175\nnow choose 5643 0\nuntil dialog 405";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        let dialog = &game.dialogs[&405];
        assert_eq!(view.text(control(dialog, 5928).unwrap()), "Toni Lubiniecki");
        assert_eq!(view.text(control(dialog, 5929).unwrap()), "18 Jahre (26.3.2007)");
        assert_eq!(view.text(control(dialog, 5930).unwrap()), "Managerlevel 9");
        // His games, the wins of them, the seasons behind and at the club.
        for (id, shown) in [(5955, "3"), (5956, "1"), (5957, "0"), (5958, "0")] {
            assert_eq!(view.text(control(dialog, id).unwrap()), shown);
        }
        assert_eq!(view.text(control(dialog, 5952).unwrap()), "50%");
        assert_eq!(view.text(control(dialog, 5949).unwrap()), "1");
        assert_eq!(view.text(control(dialog, 5950).unwrap()), "0");
        assert_eq!(view.text(control(dialog, 5951).unwrap()), "0");
        assert_eq!(view.text(control(dialog, 5953).unwrap()), "1");
        assert_eq!(view.text(control(dialog, 5954).unwrap()), "1");
        // A new career has no titles; the file text shows the CL's name.
        for id in [
            5931, 5932, 5933, 5934, 5935, 5936, 5937, 5938, 5939, 5940, 5941, 5942, 5943, 5944,
            5945, 5946,
        ] {
            assert_eq!(view.text(control(dialog, id).unwrap()), "0");
        }
        assert_eq!(
            view.text(control(dialog, 6300).unwrap()),
            "Champions League-Siege"
        );
        assert!(view.list(control(dialog, 6002).unwrap()).unwrap().rows.is_empty());
        assert_eq!(
            view.list(control(dialog, 5948).unwrap()).unwrap().rows,
            ["1.FC Union Berlin\t7.7.25-\t\t0\t0\t0"]
        );
        // The one graph point: a blue line across at level 9 of fifteen.
        assert!(
            view.lines
                .contains(&([26, 170, 253, 170], [0, 0, 255])),
            "lines: {:?}",
            view.lines
        );
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_privates_page_sells_the_manager_a_better_life() {
        let game = game();
        let then = "now click bar 16\nuntil dialog 175\nnow choose 5643 1\nuntil dialog 443";
        let Some(mut runner) = on_save(&game, "A", then) else { return };
        let dialog = &game.dialogs[&443];
        let id = |id: u16| control(dialog, id).unwrap();
        // The caption names him; a single man keeps the file's "ledig"
        // and "---", and the five children's rows are empty.
        {
            let view = runner.host.view();
            assert_eq!(view.text(id(9946)), "ANSTOSS 3 - Privates Toni Lubiniecki");
            assert_eq!(view.text(id(5599)), "ledig");
            assert_eq!(view.text(id(5607)), "---");
            for (name, years) in [(5593, 5596), (5594, 5597), (5595, 5598), (5603, 5605), (5604, 5606)] {
                assert_eq!(view.text(id(name)), "");
                assert_eq!(view.text(id(years)), "");
            }
            // The three rows: what he has, and what the next step costs.
            assert_eq!(view.text(id(5600)), "Miete");
            assert_eq!(view.text(id(5601)), "Rostlaube");
            assert_eq!(view.text(id(5602)), "Tischtennis");
            assert_eq!(view.text(id(5609)), "Kleine Eigentumswohnung (Preis 200.000 €)");
            assert_eq!(view.text(id(5608)), "Kleinwagen (Preis 20.000 €)");
            assert_eq!(view.text(id(5610)), "Golfausrüstung kaufen (20.000 €)");
            assert_eq!(view.text(id(6487)), "4.000 €");
            let wealth = {
                let state = runner.host.state();
                let slot = state.records.as_ref().unwrap()
                    .human_slot(state.season.as_ref().unwrap().current).unwrap();
                state.records.as_ref().unwrap().managers[slot].get::<i32>(0x118)
            };
            assert_eq!(view.text(id(5282)), crate::screens::playerinfo::money(&game, wealth.into()));
        }
        // A wife of the fifth name pool born on the 15th of June 1986,
        // two children, and wealth enough for the last step of every row.
        // (Pool 14, as an old RE note poked in, is out of range: the
        // original keeps eight pools, and past that it reads a name out
        // of unrelated memory — nothing to match.)
        let (nation, month, year, day) = {
            let mut state = runner.host.state();
            let current = state.season.as_ref().unwrap().current;
            let slot = state.records.as_ref().unwrap().human_slot(current).unwrap();
            let records = state.records.as_mut().unwrap();
            let [day, month, year] = records.globals.date();
            let manager = records.managers.get_mut(slot).unwrap();
            manager.put(0x118, 50_000_000i32);
            manager.put(0x1c4, [3u8, 0, 5, 5, 15, 6, 0xc2, 0x07]);
            let person = records.players.get_mut(17_001 + slot).unwrap();
            let nation = usize::from(person.names_nation());
            person.set_children(2);
            person.set_children_names([3, 7, 0, 0, 0]);
            person.put(0x174, [1u8, 1, (year % 256) as u8, (year / 256) as u8]);
            person.put(0x178, [1u8, 1, (2005 % 256) as u8, (2005 / 256) as u8]);
            (nation, month, year, day)
        };
        assert!(month >= 2, "the months' text wants a date from March on");
        let ok = control(&game.dialogs[&443], 1).unwrap();
        runner.host.input(&game, crate::pointer::Target::Button(ok), false);
        let pages = control(&game.dialogs[&175], 5643).unwrap();
        runner.host.input(&game, crate::pointer::Target::Option { control: pages, item: 1 }, false);
        let dialog = &game.dialogs[&443];
        let id = |id: u16| control(dialog, id).unwrap();
        {
            let view = runner.host.view();
            // The wife: her name's number 4 of the fifth pool, as old
            // as that June day.
            assert_eq!(view.text(id(5599)), game.rules.words.first_names[5 * 100 + 4]);
            let grown = year as i32 - 1986 - ((month, day) < (6, 15)) as i32;
            assert_eq!(view.text(id(5607)), grown.to_string());
            // Two children named from his own pool: the one of months,
            // the other of years.
            assert_eq!(
                view.text(id(5593)),
                game.rules.words.first_names[nation * 100 + 3]
            );
            assert_eq!(
                view.text(id(5594)),
                game.rules.words.first_names[nation * 100 + 7]
            );
            assert_eq!(
                view.text(id(5596)),
                game.rules.words.month_many.replace("%d", &i32::from(month).to_string())
            );
            assert_eq!(view.text(id(5597)), (year - 2005).to_string());
            assert_eq!(view.text(id(5603)), "");
            assert_eq!(view.text(id(5605)), "");
            // A wife makes the household half again as dear.
            assert_eq!(view.text(id(6487)), "6.000 €");
            assert_eq!(view.text(id(5282)), "50.00 Mio €");
        }
        // The flat, step by step to the top — and the button there does
        // nothing at all.
        let buy = |runner: &mut Runner, id: u16| {
            let c = control(&game.dialogs[&443], id).unwrap();
            runner.host.input(&game, crate::pointer::Target::Button(c), false);
        };
        (0..8).for_each(|_| buy(&mut runner, 5609));
        {
            let view = runner.host.view();
            assert_eq!(view.text(id(5600)), game.text(4531));
            assert_eq!(view.text(id(5609)), "Alles erreicht!");
            assert_eq!(view.text(id(5282)), "33.30 Mio €");
        }
        // The car's first step and the hobby's.
        buy(&mut runner, 5608);
        buy(&mut runner, 5610);
        {
            let view = runner.host.view();
            assert_eq!(view.text(id(5601)), game.text(4518));
            assert_eq!(view.text(id(5602)), game.text(4532));
            assert_eq!(view.text(id(5282)), "33.26 Mio €");
        }
        let slot = {
            let state = runner.host.state();
            let current = state.season.as_ref().unwrap().current;
            state.records.as_ref().unwrap().human_slot(current).unwrap()
        };
        {
            let state = runner.host.state();
            let manager = &state.records.as_ref().unwrap().managers[slot];
            assert_eq!(manager.get::<i32>(0x118), 33_260_000);
            assert_eq!(&manager.get::<[u8; 3]>(0x1d0), &[7, 1, 1]);
        }
        // Closed again, the page's own wealth row has the new figure.
        let ok = control(&game.dialogs[&443], 1).unwrap();
        runner.host.input(&game, crate::pointer::Target::Button(ok), false);
        let view = runner.host.view();
        let dialog = &game.dialogs[&175];
        assert_eq!(view.text(control(dialog, 5282).unwrap()), "33.26 Mio €");
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_popularity_page_rates_the_manager_four_ways() {
        let game = game();
        let then = "now click bar 16\nuntil dialog 175\nnow choose 5643 5\nuntil dialog 26";
        let Some(mut runner) = on_save(&game, "A", then) else { return };
        let dialog = &game.dialogs[&26];
        let id = |id: u16| control(dialog, id).unwrap();
        {
            let view = runner.host.view();
            assert_eq!(
                view.text(id(9947)),
                "ANSTOSS 3 - Beliebtheit Toni Lubiniecki (1.FC Union Berlin), 8.9.2025"
            );
            assert_eq!(view.text(id(1320)), "Toni Lubiniecki");
            // A.an3's four rows, of the public, the fans, the media and
            // his players: the percent, the rank's word and its picture.
            for (picture, percent, shown, rank, word, graphic) in [
                (1321, 5644, "49%", 1316, 1497, 1382),
                (1322, 5645, "56%", 1317, 1496, 1381),
                (1323, 5646, "36%", 1318, 1498, 1383),
                (5589, 5647, "84%", 5590, 1493, 1378),
            ] {
                assert_eq!(view.text(id(percent)), shown);
                assert_eq!(view.text(id(rank)), game.text(word));
                let bitmap = view.controls[id(picture)].state.as_bitmap().unwrap();
                assert_eq!(bitmap.picture, Some(crate::controls::bitmap::Picture::graphic(graphic)));
            }
            // What the club's own look and the association's say; A.an3
            // is neutral to the public and earned the best word there.
            for (ctl, word) in [(1327, 1502), (1328, 1509)] {
                let shown = view.text(id(ctl));
                assert!(shown.ends_with(&format!("\\t{}", game.text(word))), "{shown}");
            }
        }
        let ok = control(dialog, 1319).unwrap();
        runner.host.input(&game, crate::pointer::Target::Button(ok), false);
        assert_eq!(runner.host.view().id, 175);
    }

    /// The comparison of the humans (dialog 211) on "Probe1": both of
    /// them marked nine ways, and only the sums move when a question is
    /// clicked out.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_comparison_marks_the_humans_nine_ways() {
        let game = game();
        let then = "now click bar 16\nuntil dialog 175\nnow choose 5643 6\nuntil dialog 211";
        let Some(mut runner) = on_save(&game, "Probe1", then) else { return };
        let dialog = &game.dialogs[&211];
        let id = |id: u16| control(dialog, id).unwrap();
        let column = |view: &crate::view::View, name: &str| {
            (0..4usize).find(|&c| view.text(id(3070 + 12 * c as u16)) == name).unwrap()
        };
        let row = |view: &crate::view::View, c: usize| -> Vec<String> {
            (0..9).map(|k| view.text(id(3070 + (12 * c + 2 + k) as u16)).to_owned()).collect()
        };
        // The two humans among the managers; the spare columns stay
        // blank. Probe1's first has no club, so only his standing (2)
        // marks him; the second, at difficulty 2 and with SSV Ulm, gets
        // his money, his pay, his team, the youth and the season.
        let (anna, bert);
        {
            let view = runner.host.view();
            anna = column(view, "Anna");
            bert = column(view, "Bert");
            assert_eq!(row(view, anna), ["0", "0", "0", "0", "0", "0", "0", "0", "-20 (2)"]);
            assert_eq!(row(view, bert), ["4", "12", "0", "20", "4", "0", "0", "0", "0 (2)"]);
            assert_eq!(view.text(id(3070 + (12 * anna + 11) as u16)), "-20");
            assert_eq!(view.text(id(3070 + (12 * bert + 11) as u16)), "40");
            assert_eq!(view.text(id(3070 + 24)), "");
            assert_eq!(view.text(id(3070 + 36 + 11)), "");
        }
        // Seven questions out: what stays lit is the money and the
        // standing; the marks themselves keep their figures.
        for q in 3062..=3068 {
            let target = crate::pointer::Target::Option { control: id(q), item: 0 };
            runner.host.input(&game, target, false);
        }
        {
            let view = runner.host.view();
            assert_eq!(row(view, anna), ["0", "0", "0", "0", "0", "0", "0", "0", "-20 (2)"]);
            assert_eq!(view.text(id(3070 + (12 * anna + 11) as u16)), "-20");
            assert_eq!(view.text(id(3070 + (12 * bert + 11) as u16)), "4");
        }
        // OK goes back to the honours page.
        let ok = control(dialog, 1).unwrap();
        runner.host.input(&game, crate::pointer::Target::Button(ok), false);
        assert_eq!(runner.host.view().id, 175);
    }

    /// The three cases of ill repute the records tell (`FUN_0047efdd`),
    /// on copies of A.an3: its twelve matchdays played — the saves kept
    /// hold only three — its third millionaire, and its second place.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_public_words_ill_of_the_own_club() {
        /// Open the popularity page on the patched copy and read the
        /// public's word for it.
        fn word(dir: &std::path::Path, out: &str, patch: impl FnOnce(&mut a3_core::records::Records)) {
            let open = "now click bar 16\nuntil dialog 175\nnow choose 5643 5\nuntil dialog 26";
            let root = patched(dir, "A", out, patch);
            let mut game = Game::load(&root).unwrap();
            compose(&mut game);
            let Some(runner) = on_save(&game, out, open) else {
                panic!("the popularity page never opened on {out}");
            };
            let dialog = &game.dialogs[&26];
            let shown = runner.host.view().text(control(dialog, 1327).unwrap()).to_owned();
            let want = format!("\\t{}", game.text(match out {
                // Seven places short of the season's goal and three
                // squad players paid over 600 (`FUN_0047eb1f`).
                "public-millionaires" => 3072,
                // Second of its league three matchdays in (`FUN_0047ebde`).
                "public-treterpack" => 3073,
                // Twelve matchdays played, its last defeat back of five
                // of them (`FUN_0047ec75`, `FUN_00660cda`).
                _ => 3074,
            }));
            assert!(shown.ends_with(&want), "{out}: {shown}");
        }

        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        // Mark twelve of the own league's matchdays played.
        let twelve = |records: &mut a3_core::records::Records| {
            let index = records.human_slot(0).unwrap();
            let (country, number) =
                (records.managers[index].country(), records.managers[index].team());
            let level = records.team(country, number).unwrap().level();
            let league =
                i32::from(records.countries.first_league(usize::from(country))) + i32::from(level);
            records.block_mut(&format!("{:#x}[{league}]", 0x1a09d48)).unwrap()[0x46..0x48]
                .copy_from_slice(&12i16.to_le_bytes());
        };
        let own = |records: &mut a3_core::records::Records| {
            let index = records.human_slot(0).unwrap();
            (records.managers[index].country(), records.managers[index].team())
        };
        word(&dir, "public-millionaires", |records| {
            twelve(records);
            let (country, number) = own(records);
            let squad = records.team(country, number).unwrap().players();
            let id = squad
                .iter()
                .copied()
                .find(|&id| usize::from(records.players[usize::from(id)].salary()) <= 600)
                .unwrap();
            records.players[usize::from(id)].set_salary(650);
        });
        word(&dir, "public-treterpack", |records| {
            let (country, number) = own(records);
            records.team_mut(country, number).unwrap().set_table_place(2);
        });
        word(&dir, "public-pity", |records| twelve(records));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_start_menu_leads_where_the_game_does() {
        let game = game();
        let script = Script::parse(
            "
            once 9 choose 4771 5; ok
            on 95 ok
            once 9 choose 4771 1; ok
            on 424 cancel
            once 9 choose 4771 4; ok
            on 474 cancel
            once 9 choose 4771 6; ok
            until end
            ",
        )
        .unwrap();
        let (runner, result) = Runner::play(&game, &script);
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let screens: Vec<&str> =
            runner.log.iter().filter_map(|l| l.strip_prefix("screen ")).collect();
        assert_eq!(screens, ["9", "95", "9", "424", "9", "474", "9"]);
    }

    /// What the high scores behind the start menu look like: the same
    /// captioned list as the honours page's, read off the installation
    /// (a shot of the original: "ANSTOSS 3 - Highscoreliste" over the
    /// careers).
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_start_menu_shows_the_captioned_score_list() {
        let game = game();
        let script = Script::parse("now choose 4771 5; ok\nuntil dialog 95").unwrap();
        let (runner, result) = Runner::play(&game, &script);
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let view = runner.host.view();
        let dialog = &game.dialogs[&95];
        let caption = view.text(control(dialog, crate::screens::honours::scores::CAPTION_ID).unwrap());
        assert_eq!(caption, dialog.title);
        assert_eq!(caption, "ANSTOSS 3 - Highscoreliste");
        let careers = game.high_entries(9).iter().filter(|e| e.points != 0).count();
        if careers == 0 {
            eprintln!("no high09 in this installation; the oracle's copy has it");
            return;
        }
        let rows = &view.list(control(dialog, 2010).unwrap()).unwrap().rows;
        assert_eq!(rows.len(), careers);
        assert!(view.list(control(dialog, 2011).unwrap()).unwrap().rows.is_empty());
    }

    /// Back from the scores the start menu shows its first entry again,
    /// as the original's does, not the scores' it was left by.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_start_menu_forgets_the_scores_entry() {
        let game = game();
        let script =
            Script::parse("now choose 4771 5; ok\nuntil dialog 95\nnow press 1\nuntil dialog 9")
                .unwrap();
        let (runner, result) = Runner::play(&game, &script);
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let entries = control(&game.dialogs[&9], 4771).unwrap();
        assert_eq!(runner.host.view().chosen(entries), 0);
    }

    /// The notepad of the week planner (`CDiNotizen`, dialog 397): what
    /// the manager types stays on his own page, which he sees again when
    /// he opens it; löschen empties the edit, and OK keeps it that way.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_notepad_keeps_what_the_manager_types() {
        let game = game();
        let open = "now click bar 7\nuntil dialog 413\nnow press 5036\nuntil dialog 397";
        let Some(runner) = on_save(&game, "Toni", open) else { return };
        let edit = control(&game.dialogs[&397], 5414).unwrap();
        let page = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            records.notes[records.human_slot(0).unwrap()].text()
        };
        assert_eq!(runner.host.view().text(edit), "");
        assert_eq!(page(&runner), "");
        // Typed and kept: reopening the notepad shows the page again.
        let then =
            format!("{open}\nnow type 5414 Abstieg knapp; ok\nuntil dialog 413\nnow press 5036");
        let runner = on_save(&game, "Toni", &then).unwrap();
        assert_eq!(runner.host.view().text(edit), "Abstieg knapp");
        assert_eq!(page(&runner), "Abstieg knapp");
        // The edit of the dialog itself takes twenty keys at most, as the
        // original's resource says (`edit 20`).
        let then = format!("{open}\nnow type 5414 {}; ok\nuntil dialog 413", "x".repeat(1100));
        let runner = on_save(&game, "Toni", &then).unwrap();
        assert_eq!(page(&runner).len(), 20);
        // löschen clears the edit only; without OK the page keeps its text.
        let then = format!("{open}\nnow press 5416");
        let runner = on_save(&game, "Toni", &then).unwrap();
        assert_eq!(runner.host.view().text(edit), "");
        let then = format!("{open}\nnow press 5416; ok\nuntil dialog 413");
        let runner = on_save(&game, "Toni", &then).unwrap();
        assert_eq!(page(&runner), "");
    }

    /// A turn opens the note left on it: the turn of a manager with text
    /// on his page starts on the notepad over his turn screen — the office
    /// or the line-up — which follows once the note is read away; a
    /// manager with an empty page walks straight into his office.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_turn_opens_the_note_left_on_it() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let noted = patched(&dir, "Toni", "office-noted", |records| {
            let page = records.human_slot(0).unwrap();
            records.managers[page].set_menu_view(1);
            records.notes[page].set_text("Abstieg knapp");
        });
        let mut game = Game::load(&noted).unwrap();
        compose(&mut game);
        let load = LOAD.replace("until dialog 147", "until dialog 397");
        {
            let (runner, result) = Runner::play(&game, &Script::parse(&load).unwrap());
            result.unwrap_or_else(|err| panic!("text: {err}\n{}", runner.log.join("\n")));
            let view = runner.host.view();
            let edit = crate::screens::control(&game.dialogs[&397], 5414).unwrap();
            assert_eq!(view.text(edit), "Abstieg knapp", "the page opened bare");
        }
        let script = format!("{load}now press 1\nuntil dialog 483");
        let (runner, result) = Runner::play(&game, &Script::parse(&script).unwrap());
        result.unwrap_or_else(|err| panic!("noted: {err}\n{}", runner.log.join("\n")));
        let at = |what: &str| runner.log.iter().position(|l| l == what);
        let note = at("screen 397").expect("no notepad at the office turn");
        let office = at("screen 483").expect("no office after the note");
        assert!(note < office, "the notepad came after the office: {}", runner.log.join("\n"));
        let bare = patched(&dir, "Toni", "office-bare", |records| {
            let page = records.human_slot(0).unwrap();
            records.managers[page].set_menu_view(1);
        });
        let mut game = Game::load(&bare).unwrap();
        compose(&mut game);
        let load = LOAD.replace("until dialog 147", "until dialog 483");
        let (runner, result) = Runner::play(&game, &Script::parse(&load).unwrap());
        result.unwrap_or_else(|err| panic!("bare: {err}\n{}", runner.log.join("\n")));
        assert_eq!(seen(&runner, 397), 0, "a page of none popped: {}", runner.log.join("\n"));
        let line = patched(&dir, "Toni", "line-noted", |records| {
            let page = records.human_slot(0).unwrap();
            records.notes[page].set_text("Abstieg knapp");
        });
        let mut game = Game::load(&line).unwrap();
        compose(&mut game);
        let load = LOAD.replace("until dialog 147", "until dialog 397");
        let script = format!("{load}now press 1\nuntil dialog 147");
        let (runner, result) = Runner::play(&game, &Script::parse(&script).unwrap());
        result.unwrap_or_else(|err| panic!("line: {err}\n{}", runner.log.join("\n")));
        let at = |what: &str| runner.log.iter().position(|l| l == what);
        let note = at("screen 397").expect("no notepad at the line-up turn");
        let up = at("screen 147").expect("no line-up after the note");
        assert!(note < up, "the notepad came after the line-up: {}", runner.log.join("\n"));
    }

    #[test]
    fn scripts_say_what_is_wrong_with_them() {
        assert_eq!(
            Script::parse("on 9 dance").err(),
            Some(Error::Line(1, "not an action: on 9 dance".into()))
        );
        assert!(matches!(Script::parse("\n# fine\nuntil soon").err(), Some(Error::Line(3, _))));
        let script = Script::parse("seed 3\non 9 type 3121 Anna Alpha; ok\nrepeat 7 days").unwrap();
        assert_eq!(script.seed, 3);
        assert!(matches!(&script.lines[0].1, Line::On(9, actions)
            if matches!(&actions[0], Action::Type(3121, text) if text == "Anna Alpha")));
        assert!(matches!(script.lines[1].1, Line::Repeat(7)));
    }

    /// Probe1 with its own club's next match moved within the week, and
    /// changed as `patch` says; Probe1 has "Spielankündigung vor jedem
    /// Spiel" on.
    fn match_ahead(out: &str, patch: impl FnOnce(&mut a3_core::records::Team, u8)) -> Game {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "Probe1", out, |records| {
            let index = records.human_slot(0).unwrap();
            let (country, number) =
                (records.managers[index].country(), records.managers[index].team());
            let today = records.globals.day() as i16;
            let team = records.team_mut(country, number).unwrap();
            team.set_next_day(today + 2);
            patch(team, country as u8);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        game
    }

    /// A save's turn up to the announcement it opens with, `dialog`.
    fn announced<'a>(game: &'a Game, save: &str, dialog: u16, then: &str) -> Runner<'a> {
        let row = game.save_games().iter().position(|s| s.name == save).unwrap();
        let load = LOAD.replace("row 5144 0", &format!("row 5144 {row}"));
        let load = load.replace("until dialog 147", &format!("until dialog {dialog}"));
        let script = format!("{}\n{then}", load.trim_end());
        let (runner, result) = Runner::play(game, &Script::parse(&script).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        runner
    }

    fn announcement_lines(game: &Game, runner: &Runner, id: u16) -> Vec<String> {
        let view = runner.host.view();
        assert_eq!(view.id, id);
        let dialog = &game.dialogs[&id];
        (2973..=2985).map(|id| view.text(control(dialog, id).unwrap()).to_owned()).collect()
    }

    /// A cup tie two days ahead (Probe1 patched: the first round at home
    /// against the 20th club of the country): the turn opens with dialog
    /// 201, and the planner's button opens it again, every line as the
    /// original shows it — the computer's club counted with its
    /// co-trainer's line-up.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_planner_announces_a_cup_match() {
        let game = match_ahead("cup-tie", |team, country| {
            team.set_next_country(country);
            team.set_next_number(20);
            team.set_next_venue(0);
            team.set_next_kind(3);
            team.set_next_round(1);
        });
        let lines = [
            "DFB-Pokal - 1. Runde",
            "",
            "SSV Ulm 1846",
            " ",
            "SV Waldhof Mannheim",
            " ",
            " ",
            " ",
            "-",
            "-",
            "-",
            "61 : 64",
            "-",
        ];
        let runner = announced(&game, "cup-tie", 201, "");
        assert_eq!(announcement_lines(&game, &runner, 201), lines);
        let then = "now press 1\nuntil dialog 147\nnow click bar 7\nuntil dialog 413\n\
                    now press 6215\nuntil dialog 201";
        let runner = announced(&game, "cup-tie", 201, then);
        assert_eq!(announcement_lines(&game, &runner, 201), lines);
    }

    /// Probe1's league match at Freiberg moved to two days ahead: the turn
    /// opens with the league's announcement 358, as the original's does;
    /// its OK leads on to the line-up. Toni.an3, whose match is as near,
    /// has the option off and opens on the line-up.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_turn_opens_with_its_match_announced() {
        let game = match_ahead("league-ahead", |_, _| {});
        let runner = announced(&game, "league-ahead", 358, "");
        assert_eq!(
            announcement_lines(&game, &runner, 358),
            [
                "3. Liga Süd 2025/26",
                "1. Spieltag",
                "SGV Freiberg",
                "",
                "SSV Ulm 1846",
                "",
                "",
                "",
                "-",
                "-",
                "0:0:0",
                "48 : 61",
                "-",
            ]
        );
        let runner = announced(&game, "league-ahead", 358, "now press 1\nuntil dialog 147");
        assert_eq!(runner.host.view().id, 147);
    }

    /// The statistics archive on A.an3 (bar symbol 14), as the original
    /// shows it: eight categories, the tables' eighteen pages, the
    /// Bundesliga's table with its boxes and letters, the own club red;
    /// the 2. Bundesliga's champion forecast one step along the strip.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_statistics_show_the_league_tables() {
        let game = game();
        let open = "now click bar 14\nuntil dialog 18";
        let Some(runner) = on_save(&game, "A", open) else { return };
        let view = runner.host.view();
        let dialog = &game.dialogs[&18];
        let combo = view.combo(control(dialog, 1283).unwrap()).unwrap();
        assert_eq!(combo.items.len(), 8);
        let pages = &view.list(control(dialog, 1284).unwrap()).unwrap().rows;
        assert_eq!(pages.len(), 18);
        let named = (pages[0].as_str(), pages[8].as_str());
        assert_eq!(named, ("Aktuelle Tabellen", "Pokal Deutschland"));
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let row = |i: usize| (table.rows[i].text.clone(), table.rows[i].ink, table.rows[i].mark);
        let first = "1.\t(2.)\tVfB Stuttgart (E,P)\t3\t3\t0\t0\t9\t3\t4\t:\t1";
        assert_eq!(row(0), (first.to_owned(), [0; 3], Some([255, 255, 0])));
        let own = "11.\t(6.)\t1.FC Union Berlin\t3\t1\t0\t2\t3\t0\t4\t:\t4";
        assert_eq!(row(10), (own.to_owned(), [255, 0, 0], None));
        assert_eq!(view.text(control(dialog, 4780).unwrap()), "1. Bundesliga");
        let forecast = format!("{open}\nnow click at 545 436\nnow row 1284 6");
        let Some(runner) = on_save(&game, "A", &forecast) else { return };
        let view = runner.host.view();
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows[0].text, "1.\tFortuna Düsseldorf\t24\t7\t3\t79\t46\t61\t:\t15");
        assert_eq!(view.text(control(dialog, 4780).unwrap()), "2. Bundesliga");
    }

    /// The archive's cup pages on A.an3 (bar symbol 14): Deutschland's
    /// Runde 1 with its results, "n.V." and "n.E." notes and the own
    /// club's tie red; the strip one step on to the drawn but unplayed
    /// Runde 2; England's still open first round without results; and the
    /// yellow bar at its edge, whose down arrow scrolls a long round.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_statistics_show_the_cup_rounds() {
        let game = game();
        let open = "now click bar 14\nuntil dialog 18";
        let de = format!("{open}\nnow row 1284 8");
        let Some(runner) = on_save(&game, "A", &de) else { return };
        let view = runner.host.view();
        let dialog = &game.dialogs[&18];
        assert_eq!(view.text(control(dialog, 4780).unwrap()), "Runde 1");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 32);
        let row = |i: usize| (table.rows[i].text.clone(), table.rows[i].ink, table.rows[i].mark);
        let aug = "FC Augsburg\t-\tVfB Stuttgart\t1\t:\t0\t ";
        assert_eq!(row(0), (aug.to_owned(), [0; 3], None));
        let own = "1.FC Union Berlin\t-\t1.FC Köln\t4\t:\t1\t ";
        assert_eq!(row(1), (own.to_owned(), [255, 0, 0], None));
        let dortmund = "Borussia Dortmund\t-\tSV 07 Elversberg\t0\t:\t1\tn.V.";
        assert_eq!(table.rows[4].text, dortmund);
        let frankfurt = "Eintracht Frankfurt\t-\tKarlsruher SC\t5\t:\t3\tn.E.";
        assert_eq!(table.rows[5].text, frankfurt);
        let on = format!("{de}\nnow click at 545 436");
        let Some(runner) = on_save(&game, "A", &on) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 4780).unwrap()), "Runde 2");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 16);
        assert_eq!(table.rows[0].text, "1.FC Union Berlin\t-\tHamburger SV\t\t\t\t ");
        assert_eq!(table.rows[0].ink, [255, 0, 0]);
        let en = format!("{open}\nnow row 1284 9");
        let Some(runner) = on_save(&game, "A", &en) else { return };
        let view = runner.host.view();
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows[0].text, "FC Burnley\t-\tTottenham Hotspur\t\t\t\t ");
        // The down arrow of the bar that stands over 32 pairs: three
        // clicks and the eighth pair stands at the top.
        let down = format!("{de}\nnow click at 608 394\nnow click at 608 394\nnow click at 608 394");
        let Some(runner) = on_save(&game, "A", &down) else { return };
        let view = runner.host.view();
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.scroll, 3);
        // A league cup's page (`FUN_00446c74`): whichever of the five
        // pages opens, the pairings come from the one cup object at
        // 0x1ba7488, whose own rounds the strip steps through — here
        // round 1 only, played among the English clubs.
        let lg = format!("{open}\nnow row 1284 13");
        let Some(runner) = on_save(&game, "A", &lg) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 4780).unwrap()), "Runde 1");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 25);
        assert_eq!(
            table.rows[0].text,
            "Wolverhampton Wand.\t-\tAFC Sunderland\t\t\t\t "
        );
        assert_eq!(
            table.rows[24].text,
            "FC Dagenham & Redb.\t-\tAFC Bradford\t\t\t\t "
        );
    }

    /// The archive's season results on A.an3 (bar symbol 14): the
    /// Bundesliga cross-table in the table's order, the own club red,
    /// every cell the game its row's club had at home against the
    /// column's, and the bar's arrows stepping its ten columns over.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_statistics_show_the_season_results() {
        let game = game();
        let open = "now click bar 14\nuntil dialog 18";
        let se = format!("{open}\nnow row 1284 7");
        let Some(runner) = on_save(&game, "A", &se) else { return };
        let view = runner.host.view();
        let dialog = &game.dialogs[&18];
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Saisonergebnisse");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!((table.rows.len(), table.cross), (18, Some(0)));
        // Rows and columns stand in the league's own places, the order
        // the season started in: today's first VfB Stuttgart sits at
        // nine, the own club Union at thirteen.
        assert!(table.rows[0].text.starts_with("1 FC Bayern"));
        assert_eq!(table.rows[0].ink, [0; 3]);
        assert!(table.rows[8].text.starts_with("9 VfB Stuttgart"));
        assert!(table.rows[12].text.starts_with("13 1.FC Union Berlin"));
        assert_eq!(table.rows[12].ink, [255, 0, 0]);
        let wide = |t: &str| t.split('\t').count();
        assert_eq!(wide(&table.rows[0].text), 11);
        let first = table.rows[12].text.clone();
        // The right arrow of the bar at the table's head: the window one
        // column further, the same labels, other cells.
        let on = format!("{se}\nnow click at 290 64");
        let Some(runner) = on_save(&game, "A", &on) else { return };
        let view = runner.host.view();
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.cross, Some(1));
        assert!(table.rows[12].text.starts_with("13 1.FC Union Berlin"));
        assert_eq!(wide(&table.rows[12].text), 11);
        assert_ne!(table.rows[12].text, first);
    }

    /// The archive's European pages on A.an3 (bar symbol 15): the Europa
    /// League's strip with its one played round, its qualification's 41
    /// first legs under the cup's own name; and the European Supercup's
    /// single unplayed pairing, Paris against Tottenham, centred.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_statistics_show_the_european_cups() {
        let game = game();
        let open = "now click bar 14\nuntil dialog 18";
        let el = format!("{open}\nnow row 1284 16");
        let Some(runner) = on_save(&game, "A", &el) else { return };
        let view = runner.host.view();
        let dialog = &game.dialogs[&18];
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Europa League");
        assert_eq!(view.text(control(dialog, 4780).unwrap()), "Qualifikation");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 41);
        assert_eq!(
            table.rows[0].text,
            "Caernarfon Town (WAL)\t-\tBohemian FC Dublin (IRL)\t0\t:\t1\t "
        );
        assert_eq!(
            table.rows[40].text,
            "FK Radnik Bijeljina (BIH)\t-\tBreidablik Kópavog. (ISL)\t2\t:\t1\t "
        );
        let sc = format!("{open}\nnow row 1284 17");
        let Some(runner) = on_save(&game, "A", &sc) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Europäischer Supercup");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 1);
        assert_eq!(
            table.rows[0].text,
            "Paris Saint-Germain (FRA)\t-\tTottenham Hotspur (ENG)\t\t\t\t "
        );
    }

    /// The archive's Champions League page on A.an3: the strip with its
    /// three rounds so far, the qualification's 16 played pairs (two of
    /// them after extra time, and the one against Eintracht Frankfurt in
    /// red), and the groups' pages as `FUN_00447c15` fills them: each
    /// group its header, its first matchday and its fresh table.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_statistics_show_the_champions_league() {
        let game = game();
        let open = "now click bar 14\nuntil dialog 18";
        let cl = format!("{open}\nnow row 1284 15");
        let Some(runner) = on_save(&game, "A", &cl) else { return };
        let view = runner.host.view();
        let dialog = &game.dialogs[&18];
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Champions League");
        assert_eq!(view.text(control(dialog, 4780).unwrap()), "Qualifikation");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 16);
        assert_eq!(
            table.rows[0].text,
            "Glasgow Rangers (SCO)\t-\tManchester City (ENG)\t0\t:\t0\t1\t:\t0\t "
        );
        assert_eq!(
            table.rows[12].text,
            "Atromitos Athen (GRE)\t-\tMFK Karvina (CZE)\t0\t:\t2\t4\t:\t0\tn.V."
        );
        assert_eq!(
            table.rows[3].text,
            "Silkeborg IF (DEN)\t-\tEintracht Frankfurt (DEU)\t1\t:\t0\t1\t:\t3\t "
        );
        assert_eq!(table.rows[3].ink, [255, 0, 0]);
        let groups = format!("{cl}\nnow click at 545 436");
        let Some(runner) = on_save(&game, "A", &groups) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 4780).unwrap()), "Gruppen 1-4");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 36);
        assert_eq!(table.rows[0].text, "Gruppe 1:");
        assert_eq!(
            table.rows[1].text,
            "FC Barcelona (ESP)\t-\tFC Basel (SUI)\t0\t:\t1\t "
        );
        assert_eq!(
            table.rows[2].text,
            "Bayer 04 Leverkusen (DEU)\t-\tAjax Amsterdam (NED)\t1\t:\t1\t "
        );
        assert_eq!(table.rows[2].ink, [255, 0, 0]);
        assert_eq!(table.rows[4].text, "1.\tFC Basel (SUI)\t1\t3\t1\t1\t:0\t");
        assert_eq!(
            table.rows[5].text,
            "2.\tBayer 04 Leverkusen (DEU)\t1\t1\t0\t1\t:1\t"
        );
        assert_eq!(table.rows[5].ink, [255, 0, 0]);
        let second = format!("{groups}\nnow click at 545 436");
        let Some(runner) = on_save(&game, "A", &second) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 4780).unwrap()), "Gruppen 5-8");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows[0].text, "Gruppe 5:");
    }

    /// The archive's goal pages on A2.an3, where every club's statistics
    /// department stands at level two: the four graphs of the Bundesliga
    /// league — goals scored, conceded, the difference and both together
    /// — each headed "Trefferübersicht", its clubs biggest first with
    /// "name (value)" over a bar against an axis ending at 25, a
    /// negative bar green, the own club Union red.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_statistics_show_the_goal_graphs() {
        let game = game();
        let open = "now click bar 14\nuntil dialog 18";
        let dialog = &game.dialogs[&18];
        let page = |row: usize| format!("{open}\nnow row 1284 {row}");
        let tref = format!("{open}\nnow row 1284 9");
        let Some(runner) = on_save(&game, "A2", &tref) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Trefferübersicht");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.cross, None);
        let graph = table.graph.as_ref().expect("the goals' bars");
        assert_eq!((table.rows.len(), graph.bars.len()), (18, 18));
        assert!(graph.bars.iter().all(|&(_, ink)| ink == [0, 0, 0]));
        // Bayern and Mainz level at five, in the league's own places;
        // five of twenty-five on a plot of 376 is a bar of 75.
        assert!(table.rows[0].text.ends_with("(5)"));
        assert!(table.rows[0].text.starts_with("FC Bayern"));
        assert!(table.rows[1].text.ends_with("(5)"));
        assert!(table.rows[2].text.ends_with("(4)"));
        assert_eq!(graph.bars[0].0, 75);
        let union = table.rows.iter().position(|r| r.text.contains("Union")).unwrap();
        assert_eq!(table.rows[union].ink, [255, 0, 0]);
        // The conceded goals: Werder and M'gladbach level at six.
        let cass = page(10);
        let Some(runner) = on_save(&game, "A2", &cass) else { return };
        let view = runner.host.view();
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert!(table.rows[0].text.ends_with("(6)"));
        assert!(table.rows[1].text.ends_with("(6)"));
        // The difference runs down to Hamburg's minus two — a green bar
        // of thirty, its value two of twenty-five on 376.
        let diff = page(11);
        let Some(runner) = on_save(&game, "A2", &diff) else { return };
        let view = runner.host.view();
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert!(table.rows[0].text.ends_with("(4)"));
        assert_eq!(table.rows[17].text, "Hamburger SV (-2)");
        let graph = table.graph.as_ref().unwrap();
        assert_eq!(graph.bars[17], (30, [0, 0xff, 0]));
        // Both together: Werder and M'gladbach at ten, their bars 150.
        let total = page(12);
        let Some(runner) = on_save(&game, "A2", &total) else { return };
        let view = runner.host.view();
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert!(table.rows[0].text.ends_with("(10)"));
        let graph = table.graph.as_ref().unwrap();
        assert_eq!(graph.bars[0].0, 150);
    }

    /// With the statistics department raised the Mannschaft category's
    /// Torjägerliste plus adds the goals by kind, the chances and the
    /// share converted.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_ranks_the_squads_scorers_in_full() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 1\nnow row 1284 2";
        let Some(runner) = on_save(&game, "A2", then) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Nr\\tName\\tTore\\tLS\\tFr\\tEl\\tKo\\tDi\\tCh\\tVerwertet"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows[0].text, "1.\tJosip Juranovic\t1\t0\t0\t0\t0\t1\t1\t100.0");
        assert_eq!(table.rows[3].text, "4.\tToni Lubiniecki\t1\t0\t0\t0\t1\t0\t1\t100.0");
        assert_eq!(table.rows[4].text, "5.\tDiogo Leite\t0\t0\t0\t0\t0\t0\t0\t0.0");
    }

    /// Once the squad has a joker goal and an own goal, the Mannschaft
    /// category lists its Joker and its Eigentore pages (texts 2083 and
    /// 2041) and both name the players by their counts: the Joker by the
    /// joker goals, the fewer came-ons first on a tie, and the goals per
    /// came-on as "%3.1f" (`FUN_0046bd01`), the Eigentore by the own
    /// goals (`FUN_00468c74`).
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_names_the_jokers_and_the_own_goals() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let mut ids = [0; 2];
        let root = patched(&dir, "A", "squad-goals", |records| {
            let index = records.human_slot(0).unwrap();
            let (country, number) =
                (records.managers[index].country(), records.managers[index].team());
            let players = records.team(country, number).unwrap().players();
            ids = [players[0], players[1]];
            // Two joker goals each: the one came on less often stands
            // first. The second also put one past his own keeper.
            let joker = &mut records.players[usize::from(players[0])];
            joker.set_joker_goals(2);
            joker.set_league_less([3, 0]);
            let own = &mut records.players[usize::from(players[1])];
            own.set_joker_goals(2);
            own.set_league_less([5, 0]);
            own.set_own_goals(1);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let open = "now click bar 14\nuntil dialog 18\nnow row 1283 1";
        let Some(mut runner) = on_save(&game, "squad-goals", open) else { return };
        let dialog = &game.dialogs[&18];
        let list = control(dialog, 1284).unwrap();
        let entries = runner.host.view().list(list).unwrap().keys.clone();
        // The list's keys name the pages by their place in the category,
        // so the two codes find theirs there first.
        let pages = &game.rules.statistics.categories[1];
        let at = |code: i16| pages.iter().position(|p| p.code == code).unwrap();
        let joker = entries.iter().position(|&e| e == at(3)).unwrap();
        let own = entries.iter().position(|&e| e == at(0xd)).unwrap();
        runner.run(&Script::parse(&format!("now row 1284 {joker}")).unwrap()).unwrap();
        let names = runner
            .host
            .state()
            .records
            .as_ref()
            .map(|records| [ids[0], ids[1]].map(|id| records.known_as(id).unwrap_or_default()))
            .unwrap_or_default();
        let name = |k: usize| names[k].clone();
        {
            let view = runner.host.view();
            assert_eq!(view.text(control(dialog, 1292).unwrap()), game.text(3038));
            let table = view.table(control(dialog, 29999).unwrap()).unwrap();
            assert_eq!(
                table.rows.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
                [
                    format!("1.\t{}\t2\t3\t0.7", name(0)).as_str(),
                    format!("2.\t{}\t2\t5\t0.4", name(1)).as_str()
                ]
            );
        }
        runner.run(&Script::parse(&format!("now row 1284 {own}")).unwrap()).unwrap();
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), game.text(2409));
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 1);
        assert_eq!(table.rows[0].text, format!("1.\t{}\t1", name(1)));
    }

    /// With the department raised the Mannschaft category's Notenschnitt
    /// Mannschaft draws the club's mean grade and its opponents' by
    /// matchday.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_draws_the_teams_grades() {
        use crate::controls::table::Shape;
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 1\nnow row 1284 18";
        let Some(runner) = on_save(&game, "A2", then) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Notenschnitt");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let shapes = &table.graph.as_ref().unwrap().shapes;
        let segments = |colour: [u8; 3]| {
            shapes.iter().filter(|s| matches!(s, Shape::Pen { colour: c, .. } if *c == colour)).count()
        };
        // Three matchdays, the first handed in twice: three segments each.
        assert_eq!(segments([255, 0, 0]), 3);
        assert_eq!(segments([255, 255, 255]), 3);
    }

    /// The Formverlauf Mannschaft draws the form of the team's three lines
    /// week by week, the defence red, the midfield blue, the attack green.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_draws_the_lines_form() {
        use crate::controls::table::Shape;
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 1\nnow row 1284 19";
        let Some(runner) = on_save(&game, "A2", then) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Formverlauf Mannschaft");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let shapes = &table.graph.as_ref().unwrap().shapes;
        let texts: Vec<&str> = shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, ["Abwehr", "Mittelfeld", "Sturm", "Woche", "Formpunkte/20"]);
    }

    /// With the department raised the Mannschaft category's
    /// Fairneßvergleich weighs each match's fair-play points, both sides.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_compares_the_matches_fairness() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 1\nnow row 1284 17";
        let Some(runner) = on_save(&game, "A2", then) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Fairneßvergleich\\tPunkte\\tErgebnis"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let texts: Vec<&str> = table.rows.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "Hamburger SV\t-\t1.FC Union Berlin\t2\t:1\t0\t:3",
                "1.FC Union Berlin\t-\tBorussia Dortmund\t3\t:0\t1\t:2",
                "VfL Wolfsburg\t-\t1.FC Union Berlin\t0\t:6\t2\t:0",
                "Punkte total :\t10\tAuswärts:\t7",
                "Punkte Gäste:\t2\tG. Auswärts:\t0"
            ]
        );
        assert_eq!(table.rows[1].ink, [255, 0, 0]);
    }

    /// With the department raised the Mannschaft category's Chancen lists
    /// each match's chances and goals and the two sides' rates.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_weighs_the_clubs_chances() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 1\nnow row 1284 15";
        let Some(runner) = on_save(&game, "A2", then) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Nr\\tName\\tGegner\\tChancen\\tTore"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let texts: Vec<&str> = table.rows.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "1.\tHamburger SV\t-\t1.FC Union Berlin\t2\t: 3\t0\t: 3",
                "2.\t1.FC Union Berlin\t-\tBorussia Dortmund\t1\t: 3\t1\t: 2",
                "3.\tVfL Wolfsburg\t-\t1.FC Union Berlin\t5\t: 1\t2\t: 0",
                "",
                "Eigene Chancen:\t5\tEigener Schnitt:\t80.0%",
                "Gegnerische Chancen:\t10\tGegnerischer Schnitt:\t40.0%"
            ]
        );
        // The own home match red.
        assert_eq!(table.rows[1].ink, [0xfa, 0, 0]);
        assert_eq!(table.centred, 6);
    }

    /// With the department raised the Mannschaft category's Heimtorjäger
    /// ranks the squad by its goals at home, and its Einsätze
    /// hintereinander by its league matches in a row.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_ranks_the_squads_home_scorers_and_runs() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let page = |row: u32| {
            format!("now click bar 14\nuntil dialog 18\nnow row 1283 1\nnow row 1284 {row}")
        };
        let Some(runner) = on_save(&game, "A2", &page(14)) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Nr\\tName\\tHeim\\tAusw\\tGesamt"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows[0].text, "1.\tMikey Moore\t1\t0\t(1)");
        assert_eq!(table.rows[1].text, "2.\tJosip Juranovic\t0\t1\t(1)");
        let Some(runner) = on_save(&game, "A2", &page(16)) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Nr\\tName\\tEinsätze");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows[0].text, "1.\tFrederik Rönnow\t3");
        assert_eq!(table.rows[10].text, "11.\tStanley Nsoki\t1");
    }

    /// The Aktuelles category counts the league's goals per matchday, and
    /// names the country's best and worst referees.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_counts_goals_per_matchday_and_rates_the_referees() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let page = |row: u32| {
            format!("now click bar 14\nuntil dialog 18\nnow row 1283 3\nnow row 1284 {row}")
        };
        let texts = |row: u32| -> Option<Vec<String>> {
            let runner = on_save(&game, "A", &page(row))?;
            let view = runner.host.view();
            let table = view.table(control(dialog, 29999).unwrap()).unwrap();
            Some(table.rows.iter().map(|r| r.text.clone()).collect())
        };
        let Some(rows) = texts(5) else { return };
        assert_eq!(
            rows,
            ["1.\t20\t2.2", "2.\t13\t1.4", "3.\t16\t1.8", "Schnitt/Spieltag 16.3"]
        );
        let Some(rows) = texts(9) else { return };
        assert_eq!(rows[0], "Beste Schiedsrichter");
        assert_eq!(rows[1], "1.\tFlorian Heft\t3\t1.5");
        assert_eq!(rows[4], "");
        assert_eq!(rows[5], "Schlechtester Schiedsrichter");
        assert_eq!(rows[6], "1.\tLasse Koslowski\t1\t5.0");
        assert_eq!(rows.len(), 9);
    }

    /// The Aktuelles category's Qualifikationsgruppen shows a group's last
    /// pairs and its table, the strip stepping through the six groups.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_shows_the_qualifying_groups() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 3\nnow row 1284 4";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "WM-Qualifikationsgruppen");
        assert_eq!(view.text(control(dialog, 4780).unwrap()), "Gruppe 1");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 9);
        assert_eq!(table.rows[0].text, "Niederlande\t- Malta\t4\t:1");
        assert_eq!(table.rows[3].text, "1.\tKroatien\t5\t10\t7\t13\t:\t6");
        let next = format!("{then}\nnow click at 545 436");
        let Some(runner) = on_save(&game, "A", &next) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 4780).unwrap()), "Gruppe 2");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows[3].text, "1.\tSpanien\t5\t13\t9\t14\t:\t5");
    }

    /// The Aktuelles category's Elf des Tages puts the league's team of
    /// the day on the canvas: a crest left of each player, his name over
    /// the club he played for, the keeper at the bottom.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_shows_the_eleven_of_the_day() {
        use crate::controls::table::Shape;
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 3\nnow row 1284 0";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Elf des Tages");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let shapes = &table.graph.as_ref().unwrap().shapes;
        let texts: Vec<_> = shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Text { at, text, .. } => Some((*at, text.as_str())),
                _ => None,
            })
            .collect();
        assert_eq!(texts.len(), 22);
        assert_eq!(texts[0], ((196, 307), "Alexander Nübel"));
        assert_eq!(texts[1], ((196, 322), "VfB Stuttgart"));
        assert_eq!(texts[21], ((290, 17), "VfB Stuttgart"));
        let crests = shapes.iter().filter(|s| matches!(s, Shape::Image { .. })).count();
        assert_eq!(crests, 11);
        assert!(matches!(shapes[0], Shape::Image { at: (162, 305), .. }));
    }

    /// With the department raised, the Historisches category adds the
    /// Supertransfers, the dearest transfers, and the matches with the
    /// most spectators in the league and in the international cups.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_lists_transfers_and_crowds() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 4\nnow row 1284 9";
        let rows = |then: &str| -> Vec<String> {
            let Some(runner) = on_save(&game, "A2", then) else { return Vec::new() };
            let view = runner.host.view();
            let table = view.table(control(dialog, 29999).unwrap()).unwrap();
            table.rows.iter().map(|r| r.text.clone()).collect()
        };
        let transfers = rows(then);
        if transfers.is_empty() {
            return;
        }
        let first = "1.\tWeah, T.\tRM\t10\t14.47 Mio\tOlympique Marseille\tFC Bayern München";
        assert_eq!(transfers[0], first);
        let national = rows(&format!("{then}\nnow row 1284 10"));
        assert_eq!(national.len(), 20);
        assert_eq!(national[0], "1\t75000\tFC Bayern München\t-\tBorussia Dortmund\t1\t:\t1");
        let international = rows(&format!("{then}\nnow row 1284 11"));
        assert_eq!(international[0], "1\t100000\tFC Liverpool\t-\tCrystal Palace\t0\t:\t2");
    }

    /// The Historisches category's ewige Tabellen rank the clubs of the
    /// country's first league over all its seasons, this one's added: by
    /// points, with Germany's two notes under them, and by points a game.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_shows_the_all_time_tables() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 4\nnow row 1284 7";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let texts: Vec<&str> = table.rows.iter().map(|r| r.text.as_str()).collect();
        assert!(texts[0].starts_with("1.\tFC Bayern München\t2047\t4156\t"));
        let last = texts.len() - 1;
        assert_eq!(texts[last], "1993/94 wurden Dresden 4 Punkte abgezogen.");
        assert_eq!(table.rows[last].columns, Some(vec![(0, 0)]));
        let next = format!("{then}\nnow row 1284 8");
        let Some(runner) = on_save(&game, "A", &next) else { return };
        let view = runner.host.view();
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert!(table.rows[1].text.starts_with("2.\tRB Leipzig\t309\t1.83\t"));
        assert!(table.rows[12].text.starts_with("13.\t1.FC Union Berlin\t207\t1.38\t"));
        assert_eq!(table.rows[12].ink, [255, 0, 0]);
    }

    /// The Historisches category's Häufigstes Ergebnis ranks the league's
    /// scores by how often they came out; its Trainerrauswürfe stay empty
    /// before a first sacking.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_ranks_the_scores() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 4\nnow row 1284 10";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let texts: Vec<&str> = table.rows.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts.len(), 10);
        assert_eq!(texts[0], "1.\t1\t:\t0\t7\t7");
        assert_eq!(texts[9], "10.\t2\t:\t4\t1\t1");
        let sacked = format!("{then}\nnow row 1284 9");
        let Some(runner) = on_save(&game, "A", &sacked) else { return };
        let view = runner.host.view();
        assert!(view.table(control(dialog, 29999).unwrap()).unwrap().rows.is_empty());
    }

    /// The Historisches category's Ligapokal/Supercup lists the country's
    /// finals with year and result, and its Tabellenführungen the clubs
    /// that led the first league, the own club red.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_shows_finals_and_leaders() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 4\nnow row 1284 5";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let texts: Vec<&str> = table.rows.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts, ["2025\tEintracht Frankfurt\t-\tVfB Stuttgart\t1\t:\t2\t "]);
        let next = format!("{then}\nnow row 1284 6");
        let Some(runner) = on_save(&game, "A", &next) else { return };
        let view = runner.host.view();
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let texts: Vec<&str> = table.rows.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(
            texts,
            ["1.\tFC Bayern München\t1\t1", "2.\t1.FC Union Berlin\t1\t1", "3.\tVfB Stuttgart\t1\t1"]
        );
        assert_eq!(table.rows[1].ink, [255, 0, 0]);
    }

    /// The Historisches category's 5-Jahreswertung lists the European
    /// ranking's 37 countries with their total and the six seasons' points,
    /// the seasons' labels counting back from the year's.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_shows_the_five_year_ranking() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 4\nnow row 1284 3";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        let header = "Nr.\\tLand\\tgesamt\\t20/21\\t21/22\\t22/23\\t23/24\\t24/25\\t25/26";
        assert_eq!(view.text(control(dialog, 1292).unwrap()), header);
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 37);
        let first = "1.\tEngland\t104.00\t24.00\t21.00\t23.00\t17.00\t29.00\t1.00";
        assert_eq!(table.rows[0].text, first);
    }

    /// The Historisches category's Rekorde show the records the game has
    /// set — its seeded ones and those the manager broke — each title in
    /// red over who holds it, since when and the figure; Für die Ewigkeit
    /// those of the installation's file.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_shows_the_records() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 4\nnow row 1284 2";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let texts: Vec<&str> = table.rows.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts.len(), 34);
        assert_eq!(texts[0], "Höchster Heimsieg");
        assert_eq!(texts[1], "R. Langenberg\t8.2.2026\t6:0 gegen Rot-Weiß Erfurt");
        assert_eq!(texts[21], "T. Lubiniecki\t18.7.2025\t7.00 Mio € (Mikey Moore)");
        assert_eq!(texts[33], "R. Langenberg\t8.2.2026\t12");
        assert_eq!(table.rows[0].ink, [0xaa, 0, 0]);
        let next = format!("{then}\nnow row 1284 4");
        let Some(runner) = on_save(&game, "A", &next) else { return };
        let view = runner.host.view();
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows[0].text, "Höchster Heimsieg");
        assert!(table.rows[1].text.starts_with("R. Langenberg\t24.8.2026\t6:0 gegen "));
    }

    /// The Historisches category's Ewige Torjägerliste and Ewige Einsätze
    /// list the country's first league's twenty best scorers, with their
    /// clubs, and its most fielded players.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_lists_the_countrys_all_time_best() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 4\nnow row 1284 0";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 20);
        assert_eq!(table.rows[0].text, "1.\tHarry Kane\tFC Bayern München\t*\t3\t3\t1");
        let next = format!("{then}\nnow row 1284 1");
        let Some(runner) = on_save(&game, "A", &next) else { return };
        let view = runner.host.view();
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert!(table.rows[0].text.starts_with("1.\t"));
        assert!(table.rows[0].text.ends_with("\t3\t1\t0\t"));
    }

    /// With the department raised, the Aktuelles category's Elf des
    /// Jahres puts the country's team of the season on the canvas, each
    /// name with how often he made the day's eleven.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_shows_the_eleven_of_the_year() {
        use crate::controls::table::Shape;
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 3\nnow row 1284 1";
        let Some(runner) = on_save(&game, "A2", then) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Elf des Jahres (Aktueller Stand)");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let shapes = &table.graph.as_ref().unwrap().shapes;
        let texts: Vec<_> = shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Text { at, text, .. } => Some((*at, text.as_str())),
                _ => None,
            })
            .collect();
        assert_eq!(texts.len(), 22);
        assert_eq!(texts[0], ((196, 307), "Alexander Nübel (1)"));
        assert_eq!(texts[1], ((196, 322), "VfB Stuttgart"));
    }

    /// With the department raised, the Aktuelles category's
    /// Mannschaftsvergleich sets out the own club's figures — alone, with
    /// no other human club to compare it with.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_compares_the_squads() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 3\nnow row 1284 2";
        let Some(runner) = on_save(&game, "A2", then) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Mannschaftsvergleich");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let texts: Vec<&str> = table.rows.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts.len(), 17);
        assert_eq!(texts[0], "Verein\t1.FC Union Berlin\t");
        assert_eq!(texts[1], "Manager\tToni Lubiniecki (9)\t");
        assert_eq!(texts[2], "DAlter\t25.3\t");
        assert_eq!(texts[11], "Torhüter\tA:3\tDF:13.1\tTo:0\t");
        assert_eq!(texts[15], "Gesamt\tA:26\tDF:14.1\tTo:4\t");
        assert_eq!(texts[16], "Marktwert\t101.23 Mio €\t");
    }

    /// With the department raised, the Aktuelles category's Stars der
    /// Ligen lists the league's best player at each position; a position
    /// no one holds keeps an empty name.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_lists_the_leagues_stars() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 3\nnow row 1284 7";
        let Some(runner) = on_save(&game, "A2", then) else { return };
        let view = runner.host.view();
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let texts: Vec<&str> = table.rows.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts.len(), 10);
        assert_eq!(texts[0], "Torwart\tGregor Kobel\tBorussia Dortmund\t10.6\t27");
        assert_eq!(texts[1], "Libero\t\tohne Verein\t0.0\t0");
        assert_eq!(texts[9], "Sturm\tHarry Kane\tFC Bayern München\t11.8\t32");
    }

    /// The Aktuelles category's Spielbericht tells the club's last match:
    /// the league and matchday, the pairing with the places, the goals,
    /// the red card, the gate, the weather, the comparison and the money,
    /// rating, referee, playing time and best player.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_reports_the_last_match() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 3\nnow row 1284 2";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Spielbericht");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let texts: Vec<&str> = table.rows.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts.len(), 18);
        assert_eq!(texts[0], "\t1. Bundesliga, 3. Spieltag");
        assert_eq!(
            texts[1],
            "Begegnung:\tVfL Wolfsburg (4. Platz) - 1.FC Union Berlin (11. Platz)   2:0 "
        );
        assert_eq!(texts[2], "Torschützen:\t1\t:\t0\tKilian Fischer\t(31.)");
        assert_eq!(texts[3], "\t2\t:\t0\tMohamed Amoura\t(64.)");
        assert_eq!(texts[4], "besondere Ereignisse:\tRote Karte für Diogo Leite (89.)");
        assert_eq!(texts[5], "Zuschauerzahl:\t24589");
        assert_eq!(texts[12], "Bruttoeinnahme:\t719.682 €");
        assert_eq!(texts[13], "Prämienauszahlung:\t136.025 € (93.200 €)");
        let referee = "Schiedsrichterleistung:\tSchiedsrichter Schmidt hatte keine Probleme";
        assert_eq!(texts[15], referee);
        assert_eq!(texts[17], "Bester Spieler:\tStanley Nsoki ");
        assert_eq!(table.rows[2].columns.as_ref().map(Vec::len), Some(6));
        assert_eq!(table.rows[7].columns.as_ref().map(Vec::len), Some(3));
    }

    /// The Aktuelles category's Spielplan shows the league's pairs two
    /// matchdays at a time, from the two the last one played falls in;
    /// its bar steps two on.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_shows_the_fixture_plan() {
        use crate::controls::table::Shape;
        let game = game();
        let dialog = &game.dialogs[&18];
        let texts = |runner: &Runner| -> Vec<((i32, i32), String)> {
            let view = runner.host.view();
            let table = view.table(control(dialog, 29999).unwrap()).unwrap();
            let shapes = &table.graph.as_ref().unwrap().shapes;
            shapes
                .iter()
                .filter_map(|s| match s {
                    Shape::Text { at, text, .. } => Some((*at, text.clone())),
                    _ => None,
                })
                .collect()
        };
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 3\nnow row 1284 6";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let plan = texts(&runner);
        assert_eq!(plan.len(), 2 + 2 * 9 * 3);
        assert_eq!(plan[0], ((0, 0), "3. Spieltag".to_owned()));
        assert_eq!(plan[1], ((0, 20), "Bayer 04 Leverkusen".to_owned()));
        assert_eq!(plan[3], ((150, 20), "1.FSV Mainz 05".to_owned()));
        assert_eq!(plan[28], ((0, 180), "4. Spieltag".to_owned()));
        let next = format!("{then}\nnow click at 629 412");
        let Some(runner) = on_save(&game, "A", &next) else { return };
        let plan = texts(&runner);
        assert_eq!(plan[0].1, "5. Spieltag");
        assert_eq!(plan[1].1, "VfL Wolfsburg");
        assert_eq!(runner.host.view().table(control(dialog, 29999).unwrap()).unwrap().scroll, 2);
    }

    /// The Aktuelles category's Serien states the club's runs that reached
    /// their marks and its biggest wins and defeats that scored.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_states_the_clubs_runs() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 3\nnow row 1284 1";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Aktuelles Mannschaft");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let texts: Vec<&str> = table.rows.iter().map(|r| r.text.as_str()).collect();
        // The 0:2 at Wolfsburg scored nothing, so it stands out of the list.
        assert_eq!(
            texts,
            [
                "Höchste Heimniederlage gegen: Borussia Dortmund 1:2",
                "Höchster Auswärtssieg bei: Hamburger SV 3:0"
            ]
        );
        assert!(table.columns.is_empty());
    }

    /// The Aktuelles category's Letzter Spieltag shows the league's last
    /// matchday: places before it, results, half time and gates.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_shows_the_last_matchday() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 3\nnow row 1284 3";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Paarungen\\tZuschauer");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 9);
        assert_eq!(
            table.rows[3].text,
            "VfL Wolfsburg (9.)\t- 1.FC Union Berlin (6.)\t2\t:0\t(1\t:0)\t24.589"
        );
        assert_eq!(table.rows[3].ink, [255, 0, 0]);
        assert_eq!(
            table.rows[6].text,
            "FC Bayern München (1.)\t- Borussia Dortmund (10.)\t1\t:1\t(1\t:0)\t75.000"
        );
    }

    /// The Aktuelles category's Tab. sonstige Länder names the top three of
    /// each country the game does not play in full.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_tops_the_other_countries_tables() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 3\nnow row 1284 8";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Pl.\\tVerein\\tPunkte");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let texts: Vec<&str> = table.rows.iter().take(6).map(|r| r.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "Portugal",
                "",
                "1.\tBenfica Lissabon\t9",
                "2.\tGD Estoril Praia\t9",
                "3.\tFC Rio Ave\t9",
                ""
            ]
        );
        assert_eq!(table.rows[2].ink, [255, 0, 0]);
    }

    /// The Aktuelles category shows the European club ranking the game
    /// keeps, and the newspaper's headlines — none yet on this save.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_ranks_europes_clubs_and_quotes_the_paper() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let page = |row: u32| {
            format!("now click bar 14\nuntil dialog 18\nnow row 1283 3\nnow row 1284 {row}")
        };
        let Some(runner) = on_save(&game, "A", &page(10)) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Pl.\\tVerein\\tPunkte\\tLiga\\tEP\\tPokal"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 25);
        assert_eq!(table.rows[0].text, "1.\tInter Mailand\t345.00\t270.00\t75.00\t0.00");
        assert_eq!(
            table.rows[13].text,
            "14.\tFC Bayern München\t275.67\t200.67\t75.00\t0.00"
        );
        let Some(runner) = on_save(&game, "A", &page(7)) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Letzte Zeitungsmeldungen");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 1);
        assert!(table.columns.is_empty());
    }

    /// The Mannschaft category's Torjägerliste ranks the own squad by its
    /// league goals, with the cup's and the European cups' beside.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_ranks_the_squads_scorers() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 1\nnow row 1284 1";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Nr\\tName\\tTore\\tPokal\\tEuropapokal"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let state = runner.host.state();
        // 1.FC Union Berlin, the own club.
        let squad = state.records.as_ref().unwrap().team(14, 84).unwrap().players().len();
        assert_eq!(table.rows.len(), squad);
        assert_eq!(table.centred, squad as i32);
        assert_eq!(table.rows[0].text, "1.\tJosip Juranovic\t1\t0\t0");
        assert_eq!(table.rows[3].text, "4.\tToni Lubiniecki\t1\t1\t0");
        assert!(table.rows.iter().all(|r| r.ink == [0, 0, 0]));
    }

    /// The Mannschaft category's Vereinsresultate lists the club's league
    /// fixtures with their dates, the results of those played.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_lists_the_clubs_fixtures() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 1\nnow row 1284 0";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Datum\\tBegegnung\\tErgebnis");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        // Thirty-four matchdays and a blank row between the halves.
        assert_eq!(table.rows.len(), 35);
        assert_eq!(table.centred, 35);
        assert_eq!(
            table.rows[0].text,
            "Sam,\t24.8.2025\tHamburger SV\t-\t1.FC Union Berlin\t0\t:\t3"
        );
        assert_eq!(
            table.rows[3].text,
            "Sam,\t14.9.2025\t1.FC Union Berlin\t-\tRB Leipzig\t"
        );
        assert_eq!(table.rows[17].text, "");
    }

    /// The Mannschaft category's Torjäger ewig lists the club's scorers
    /// under its manager as the club keeps them.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_lists_the_clubs_all_time_scorers() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 1\nnow row 1284 2";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Nr\\tName\\tTore\\tEins.\\tSchnitt\\t1.L\\t2.L\\t3.L\\tLetzte 3 Jahre"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 12);
        assert_eq!(table.centred, 13);
        assert_eq!(table.rows[0].text, "1.\tToni Lubiniecki\t1\t3\t0.3\t1\t0\t0\t1\t0\t0");
        assert_eq!(table.rows[4].text, "5.\tMarin Ljubicic\t0\t3\t0.0\t0\t0\t0\t0\t0\t0");
    }

    /// The Mannschaft category's Einsätze ewig lists the club's most
    /// fielded players under its manager as the club keeps them.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_lists_the_clubs_all_time_appearances() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 1\nnow row 1284 5";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Nr\\tName\\tEinsätze\\t1.Liga\\t2.Liga\\t3.Liga\\tPokal\\tLändersp."
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 13);
        assert_eq!(table.centred, 13);
        assert_eq!(table.rows[0].text, "1.\tMarin Ljubicic\t3\t3\t0\t0\t1\t0");
        assert_eq!(table.rows[12].text, "13.\tLeopold Querfeld\t0\t0\t0\t0\t1\t0");
    }

    /// The Mannschaft category's Formverlauf and Mannschaftswerte draw
    /// the club's weeks as lines in the graph window, with their legends.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_draws_the_clubs_weeks() {
        use crate::controls::table::Shape;
        let game = game();
        let dialog = &game.dialogs[&18];
        let page = |row: u32| {
            format!("now click bar 14\nuntil dialog 18\nnow row 1283 1\nnow row 1284 {row}")
        };
        let texts = |shapes: &[Shape]| -> Vec<String> {
            shapes
                .iter()
                .filter_map(|s| match s {
                    Shape::Text { text, .. } => Some(text.clone()),
                    _ => None,
                })
                .collect()
        };
        let Some(runner) = on_save(&game, "A", &page(12)) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Formverlauf");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let shapes = &table.graph.as_ref().unwrap().shapes;
        assert_eq!(texts(shapes), ["Mannschaftsform", "Woche", "Formpunkte/20"]);
        // Nine weeks of form: eight red segments.
        let red = shapes
            .iter()
            .filter(|s| matches!(s, Shape::Pen { colour: [255, 0, 0], .. }))
            .count();
        assert_eq!(red, 8);
        let Some(runner) = on_save(&game, "A", &page(11)) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Verlauf Mannschaftswerte");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let shapes = &table.graph.as_ref().unwrap().shapes;
        assert_eq!(
            texts(shapes)[..6],
            ["Siegeswille", "Stimmung", "Eingespieltheit", "Kondition", "Frische", "Respekt"]
        );
        // The respect's line starts a week later: seven segments.
        let yellow = shapes
            .iter()
            .filter(|s| matches!(s, Shape::Pen { colour: [255, 255, 0], .. }))
            .count();
        assert_eq!(yellow, 7);
    }

    /// The Mannschaft category's Tabellenplatzentwicklung draws each human
    /// club's league places matchday by matchday.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_draws_the_clubs_places() {
        use crate::controls::table::Shape;
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 1\nnow row 1284 13";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Tabellenplatzentwicklung");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let shapes = &table.graph.as_ref().unwrap().shapes;
        let texts: Vec<&str> = shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, ["1.FC Union Berlin", "Spieltag"]);
        // Places 1, 6 and 11 over the Bundesliga's 17 steps of 310 / 17
        // pixels: two red segments down from the top.
        let red: Vec<_> = shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Pen { from, to, colour: [255, 0, 0] } => Some((*from, *to)),
                _ => None,
            })
            .collect();
        assert_eq!(red.len(), 2);
        assert!(red[0].1.1 > red[0].0.1 && red[1].1.1 > red[1].0.1);
    }

    /// The Mannschaft category's Trefferverteilung draws the club's goals
    /// and those against it minute by minute, and counts them by half.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_draws_the_goals_by_minute() {
        use crate::controls::table::Shape;
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 1\nnow row 1284 17";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Trefferverteilung");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let shapes = &table.graph.as_ref().unwrap().shapes;
        let texts: Vec<&str> = shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        for half in [
            "Tore 1.Halbzeit :2",
            "Tore 2.Halbzeit :2",
            "Gegentore 1.Halbzeit :3",
            "Gegentore 2.Halbzeit :1",
        ] {
            assert!(texts.contains(&half), "{half}");
        }
        // Four goals each way, a bar each, and the key's two boxes.
        let fills = shapes.iter().filter(|s| matches!(s, Shape::Fill { .. })).count();
        assert_eq!(fills, 4 + 4 + 2);
    }

    /// The Mannschaft category's Angstgegner and Lieblingsgegner name the
    /// clubs the own club lost and won against, at home, away and in all.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_names_the_clubs_foes_and_favourites() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let page = |row: u32| {
            format!("now click bar 14\nuntil dialog 18\nnow row 1283 1\nnow row 1284 {row}")
        };
        let Some(runner) = on_save(&game, "A", &page(15)) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Angstgegner\\tSpiele\\tPkt\\tDiff\\tTore"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let texts: Vec<&str> = table.rows.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "Heimbegegnungen",
                "Borussia Dortmund\t1\t0\t-1\t1\t:\t2",
                "",
                "Auswärtsbegegnungen",
                "VfL Wolfsburg\t1\t0\t-2\t0\t:\t2",
                "",
                "Alle Ligaspiele",
                "VfL Wolfsburg\t1\t0\t-2\t0\t:\t2",
                "Borussia Dortmund\t1\t0\t-1\t1\t:\t2",
                ""
            ]
        );
        assert_eq!(table.rows[0].ink, [0xaa, 0, 0]);
        assert_eq!(table.rows[0].columns, Some(vec![(0, 0)]));
        let Some(runner) = on_save(&game, "A", &page(16)) else { return };
        let view = runner.host.view();
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let texts: Vec<&str> = table.rows.iter().map(|r| r.text.as_str()).collect();
        // No win at home yet: that section is left out.
        assert_eq!(
            texts,
            [
                "Auswärtsbegegnungen",
                "Hamburger SV\t1\t3\t3\t3\t:\t0",
                "",
                "Alle Ligaspiele",
                "Hamburger SV\t1\t3\t3\t3\t:\t0",
                ""
            ]
        );
    }

    // The same two pages on the kept second-season save, shot at the original
    // there (S2late3, day 300 of Union's second season, the department raised
    // to three): the comparison counts the archived games of the season
    // before as well, the away rows over both seasons — two games against FC
    // Bayern München away, none of them won. All three of the page's sections
    // are shot at the original; its category is again the second one, and its
    // list holds the two pages at its rows 24 and 25. The third section sums
    // the two above: Bayern's four games, the point of the one home win, and
    // 5 goals scored against 11 conceded over both meetings at home and away.
    #[test]
    #[ignore = "needs the kept save S2late3"]
    fn the_second_season_names_the_foes_of_the_seasons_before() {
        let Some((root, game)) = kept_game("S2late3", "foes") else { return };
        let dialog = &game.dialogs[&18];
        let red = [170, 0, 0];
        let foe = [
            ("Angstgegner", 24, [
                "Borussia Dortmund\t2\t0\t-4\t1\t:\t5",
                "Borussia M'gladbach\t1\t0\t-2\t0\t:\t2",
                "SC Freiburg\t1\t0\t-1\t0\t:\t1",
                "FC Bayern München\t2\t1\t-3\t3\t:\t6",
                "Eintracht Frankfurt\t2\t1\t-1\t0\t:\t1",
            ].as_slice(), [
                "VfL Wolfsburg\t2\t0\t-4\t0\t:\t4",
                "FC Bayern München\t2\t0\t-3\t2\t:\t5",
                "SC Freiburg\t1\t0\t-2\t0\t:\t2",
                "Hannover 96\t1\t0\t-1\t0\t:\t1",
                "1.FSV Mainz 05\t2\t1\t-1\t1\t:\t2",
            ].as_slice(), [
                "FC Bayern München\t4\t1\t-6\t5\t:\t11",
                "SC Freiburg\t2\t0\t-3\t0\t:\t3",
                "Borussia Dortmund\t4\t3\t-5\t4\t:\t9",
                "VfL Wolfsburg\t4\t3\t-4\t4\t:\t8",
                "Hannover 96\t2\t1\t-1\t1\t:\t2",
            ].as_slice()),
            ("Lieblingsgegner", 25, [
                "VfB Stuttgart\t2\t6\t3\t4\t:\t1",
                "TSG 1899 Hoffenheim\t2\t6\t2\t5\t:\t3",
                "SV Werder Bremen\t2\t4\t2\t4\t:\t2",
                "FC St. Pauli\t2\t4\t1\t1\t:\t0",
                "1.FC Köln\t1\t3\t1\t2\t:\t1",
            ].as_slice(), [
                "RB Leipzig\t2\t6\t2\t3\t:\t1",
                "SV Werder Bremen\t1\t3\t2\t2\t:\t0",
                "1.FC Köln\t1\t3\t1\t2\t:\t1",
                "FC St. Pauli\t1\t3\t1\t2\t:\t1",
                "1.FC Heidenheim\t1\t3\t1\t1\t:\t0",
            ].as_slice(), [
                "TSG 1899 Hoffenheim\t4\t8\t2\t7\t:\t5",
                "SV Werder Bremen\t3\t7\t4\t6\t:\t2",
                "FC St. Pauli\t3\t7\t2\t3\t:\t1",
                "1.FC Köln\t2\t6\t2\t4\t:\t2",
                "1.FC Heidenheim\t2\t6\t2\t2\t:\t0",
            ].as_slice()),
        ];
        for (page, row, heim, auswaerts, alle) in foe {
            let then = format!("now click bar 14\nuntil dialog 18\nnow row 1283 1\nnow row 1284 {row}");
            let Some(runner) = on_save(&game, "foes", &then) else { return };
            let view = runner.host.view();
            let head = format!("{page}\\tSpiele\\tPkt\\tDiff\\tTore");
            assert_eq!(view.text(control(dialog, 1292).unwrap()), head);
            let table = view.table(control(dialog, 29999).unwrap()).unwrap();
            let lines = |name: &str, rows: &[&str]| {
                assert!(
                    table.rows.iter().any(|r| r.text == name && r.ink == red),
                    "{page}: no {name} head"
                );
                for line in rows {
                    assert!(
                        table.rows.iter().any(|r| r.text == *line && r.ink == [0, 0, 0]),
                        "{page}: no {line}"
                    );
                }
            };
            lines("Heimbegegnungen", &heim);
            lines("Auswärtsbegegnungen", &auswaerts);
            lines("Alle Ligaspiele", alle);
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The Mannschaft category's Spielertrends groups the squad by its
    /// chances of a downgrade or an upgrade, the goals still needed beside.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_groups_the_squads_trends() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 1\nnow row 1284 7";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Spielertrends\\tAlter\\tPos\\tStärke\\tForm\\tD-Form\\tTore bis Aufwertung"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        // Three groups' headings and the 26 players.
        assert_eq!(table.rows.len(), 29);
        assert_eq!(table.rows[0].text, "Möglicher Abwertungskandidat");
        assert_eq!(table.rows[0].ink, [0xaa, 0, 0]);
        assert_eq!(table.rows[1].text, "Christopher Trimmel\t38\tRV\t7\t14\t11.1\t12");
        // A keeper needs no goals.
        assert_eq!(table.rows[3].text, "Frederik Rönnow\t33\tTO\t9\t15\t14.7\t-");
        assert_eq!(table.rows[21].text, "Möglicher Aufwertungskandidat");
        assert_eq!(table.rows[22].text, "David Preu\t20\tLM\t5\t15\t14.3\t8");
    }

    /// The Mannschaft category's Wer kam? Wer ging? lists the club's
    /// transfers under its manager and their total.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_lists_the_clubs_transfers() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 1\nnow row 1284 14";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Jahr\\tW\\tArt\\tName\\tVerein\\tSt\\tPos\\tAblöse"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 8);
        assert_eq!(
            table.rows[0].text,
            "2025\t2\tZug\tGabriel Moscardo\tSporting Braga\t7\tDM\t4.35 Mio €"
        );
        assert_eq!(
            table.rows[4].text,
            "2025\t9\tVerl\tAndrej Ilic\tSV Werder Bremen\t8\tST\t850.000 €"
        );
        // Sales and loans out bring 5.568 thousand, the two who came cost
        // 11.350 thousand.
        assert_eq!(table.rows[7].text, "Transfersumme total: - 5.78 Mio €");
        assert_eq!(table.rows[7].columns, Some(vec![(0, 0)]));
    }

    /// The Mannschaft category's Notenschnitt Spieler ranks the squad by
    /// its mean grade, at home and away beside, those without a grade
    /// last.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_grades_the_squad() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 1\nnow row 1284 10";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Name\\tGesamt\\tHeim\\tEins.\\tAusw.\\tEins."
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 26);
        assert_eq!(table.rows[0].text, "Stanley Nsoki\t2.8\t-\t0\t2.8\t2");
        assert_eq!(table.rows[1].text, "Toni Lubiniecki\t2.8\t3.0\t1\t2.8\t2");
        assert_eq!(table.rows[11].text, "Gabriel Moscardo\t4.0\t3.5\t1\t4.3\t2");
        // Without a grade the mean counts as 101, so they come last.
        assert_eq!(table.rows[12].text, "David Preu\t-\t-\t0\t-\t0");
    }

    /// The Mannschaft category's Spielereigenschaften names the squad's
    /// traits, the players by name.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_names_the_squads_traits() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 1\nnow row 1284 18";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Name\\tPos\\tStk\\tSpielereigenschaften"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 26);
        assert_eq!(table.rows[0].text, "Alex Král\tOM\t9\tAllrounder");
        assert_eq!(table.rows[2].text, "András Schäfer\tOM\t9\tEgoist");
        assert_eq!(table.rows[3].text, "Carl Klaus\tTO\t6\t");
    }

    /// The Mannschaft category's Einsätze counts the squad's league
    /// matches and weighs its form this season against today's.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_counts_the_squads_matches() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 1\nnow row 1284 4";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Nr\\tName\\tEinsätze\\tEinw.\\tAusw.\\tD-Form\\tTendenz"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 26);
        assert_eq!(table.rows[0].text, "1.\tFrederik Rönnow\t3\t0\t0\t14.7\t+");
        // Of equal matches, the fewer come on stand first.
        assert_eq!(table.rows[11].text, "12.\tStanley Nsoki\t2\t1\t0\t14.2\t+");
    }

    /// More of the Mannschaft category's lists over the own squad: the
    /// Scorer and the Vorlagen keep only the players with a figure, the
    /// Fairneß and the Gelbe Karten all of them.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_lists_the_squads_figures() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let page = |row: u32| {
            format!("now click bar 14\nuntil dialog 18\nnow row 1283 1\nnow row 1284 {row}")
        };
        let texts = |row: u32| -> Option<(String, Vec<String>, i32)> {
            let runner = on_save(&game, "A", &page(row))?;
            let view = runner.host.view();
            let header = view.text(control(dialog, 1292).unwrap()).to_owned();
            let table = view.table(control(dialog, 29999).unwrap()).unwrap();
            Some((header, table.rows.iter().map(|r| r.text.clone()).collect(), table.centred))
        };
        let Some((header, rows, centred)) = texts(3) else { return };
        assert_eq!(header, "Nr.\\tName\\tGesamt\\tlSp\\tTore\\tlSp\\tVorl.\\tlSp");
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[0], "1.\tJosip Juranovic\t2\t(0)\t1\t(0)\t1\t(0)");
        assert_eq!(centred, -1);
        let Some((header, rows, centred)) = texts(6) else { return };
        assert_eq!(header, "Nr\\tName\\tPunkte\\tRot\\tGelb-Rot\\tGelb\\tSptg. gesp");
        assert_eq!(rows.len(), 26);
        assert_eq!(rows[0], "1.\tDiogo Leite\t5\t1\t0\t0\t0");
        assert_eq!(centred, 26);
        let Some((_, rows, _)) = texts(8) else { return };
        assert_eq!(rows, ["1.\tDanilho Doekhi\t1\t(0)", "2.\tJosip Juranovic\t1\t(0)"]);
        let Some((header, rows, _)) = texts(9) else { return };
        assert_eq!(header, "Nr\\tName\\tLiga\\tPokal\\tEuropapokal");
        assert_eq!(rows[1], "2.\tOliver Burke\t1\t1\t0");
    }

    /// The Liga category names its players and counts their goals,
    /// assists, free kicks and penalties.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_lists_the_leagues_players() {
        let game = game();
        let open = "now click bar 14\nuntil dialog 18";
        let liga = format!("{open}\nnow row 1283 2");
        let dialog = &game.dialogs[&18];
        let Some(runner) = on_save(&game, "A", &liga) else { return };
        let view = runner.host.view();
        // The Torjägerliste stands first, on the own league's strip.
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Nr.\\tName\\tVerein\\tTore\\tFr\\tEl\\tKo\\tDi\\tPo"
        );
        assert_eq!(view.text(control(dialog, 4780).unwrap()), "1. Bundesliga");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        // The saved list's order, the page's figures its entry's bytes.
        assert_eq!(table.rows[0].text, "1.\tKane, H\tFC Bayern München\t3\t1\t0\t0\t1\t1");
        assert_eq!(table.rows[1].text, "2.\tBeier, M\tBorussia Dortmund\t2\t0\t0\t0\t0\t0");
        assert_eq!(
            table.rows[3].text,
            "4.\tSchick, P\tBayer 04 Leverkusen\t2\t0\t0\t0\t2\t0"
        );
        // The own club's row blue.
        let union = table.rows.iter().position(|r| r.ink == [0, 0, 255]).unwrap();
        assert!(table.rows[union].text.contains("\tMoore, M\t1.FC Union Berlin\t"));
        // The scorer page: goals plus assists, twenty at most, the own
        // row red.
        let sc = format!("{liga}\nnow row 1284 1");
        let Some(runner) = on_save(&game, "A", &sc) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Nr.\\tName\\tVerein\\tPkt\\tTore\\tlSp\\tVorl.\\tlSp"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 20);
        assert_eq!(table.rows[0].text, "1.\tKane, H\tFC Bayern München\t5\t3\t0\t2\t1");
        let red = table.rows.iter().position(|r| r.ink == [255, 0, 0]).unwrap();
        assert!(table.rows[red].text.starts_with("6.\tJuranovic, J\t1.FC Union Berlin"));
        // The free-kick page (`FUN_0044ab7a`): taken, converted and
        // missed, both names spelled out, rows from the top.
        let fr = format!("{liga}\nnow row 1284 2");
        let Some(runner) = on_save(&game, "A", &fr) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Nr.\\tName\\tVerein\\tges.\\tverw.\\tversch."
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.centred, -1);
        assert_eq!(
            table.rows[0].text,
            "1.\tFranck Honorat\tBorussia M'gladbach\t2\t1\t1"
        );
        assert_eq!(
            table.rows[4].text,
            "5.\tErmedin Demirovic\tVfB Stuttgart\t3\t1\t2"
        );
        // The penalty page (`FUN_0044af3b`): the own club's row blue.
        let el = format!("{liga}\nnow row 1284 3");
        let Some(runner) = on_save(&game, "A", &el) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Nr.\\tName\\tVerein\\tElfer\\tverw.\\tversch."
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(
            table.rows[1].text,
            "2.\tJanik Haberer\t1.FC Union Berlin\t1\t1\t0"
        );
        assert_eq!(table.rows[1].ink, [0, 0, 255]);
        // The assistor page (`FUN_0044a02e`) stands centred like the two
        // first; its figures are the entry's after the club's number.
        let vo = format!("{liga}\nnow row 1284 5");
        let Some(runner) = on_save(&game, "A", &vo) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Nr.\\tName\\tVerein\\tVorl.\\tlSp"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 20);
        assert_eq!(table.centred, 20);
        assert_eq!(table.rows[0].text, "1.\tHarry Kane\tFC Bayern München\t2\t0");
        // The keeper page (`FUN_0044b2fc`) lists only keepers with one of
        // their two figures; the own club's keeper stands blue.
        let ek = format!("{liga}\nnow row 1284 8");
        let Some(runner) = on_save(&game, "A", &ek) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Nr.\\tName\\tVerein\\tpariert\\tkassiert"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.centred, -1);
        assert_eq!(
            table.rows[0].text,
            "1.\tFrederik Rönnow\t1.FC Union Berlin\t1\t0"
        );
        assert_eq!(table.rows[0].ink, [0, 0, 255]);
        // The Eigen goal page (`FUN_0044a810`): this save has none yet, so
        // the table stays empty under its one-figure header.
        let et = format!("{liga}\nnow row 1284 7");
        let Some(runner) = on_save(&game, "A", &et) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Nr.\\tName\\tVerein\\tEigentore"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert!(table.rows.is_empty());
    }

    /// The Treter, open with the statistics department raised, weighs each
    /// player's cards as the Fairneß does a club's.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_weighs_the_leagues_players_cards() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 2\nnow row 1284 9";
        let Some(runner) = on_save(&game, "A2", then) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Nr.\\tName\\tVerein\\tPkt\\tRot\\tGeR\\tGelb"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 20);
        assert_eq!(table.centred, -1);
        assert_eq!(table.rows[0].text, "1.\tMuheim, M\tHamburger SV\t7\t1\t0\t2");
        assert_eq!(table.rows[2].text, "3.\tLeite, D\t1.FC Union Berlin\t5\t1\t0\t0");
        assert_eq!(table.rows[2].ink, [0, 0, 255]);
    }

    /// The Schirilieblinge, open with the department raised, ranks the
    /// clubs by how often the referees favoured them, below 0 as well.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_ranks_the_referees_favourites() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 2\nnow row 1284 10";
        let Some(runner) = on_save(&game, "A2", then) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Nr\\tVerein\\twie oft bevorzugt"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 18);
        assert_eq!(table.rows[0].text, "1.\tFC Bayern München\t2");
        assert_eq!(table.rows[10].text, "11.\t1.FC Union Berlin\t-1");
        assert_eq!(table.rows[10].ink, [255, 0, 0]);
        assert_eq!(table.rows[17].text, "18.\tEintracht Frankfurt\t-3");
    }

    /// The Fairneß ranks the league's clubs by their cards' points and
    /// closes with the season's record for one match.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_weighs_the_leagues_cards() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 2\nnow row 1284 4";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Nr.\\tName\\tPkt\\tRot\\tGeR\\tGelb\\tPkt-Schnitt"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        // Eighteen clubs, a blank and the record.
        assert_eq!(table.rows.len(), 20);
        assert_eq!(table.centred, -1);
        // Five points a red card, three a yellow-red, one a yellow, over
        // the three matchdays played.
        assert_eq!(table.rows[0].text, "1.\t1.FC Köln\t17\t2\t0\t7\t5.67");
        assert_eq!(table.rows[4].text, "5.\t1.FC Union Berlin\t10\t1\t0\t5\t3.33");
        assert_eq!(table.rows[4].ink, [255, 0, 0]);
        assert_eq!(table.rows[18].text, "");
        assert_eq!(table.rows[19].text, "Saisonrekord in einem Spiel:\t1.FC Köln (8)");
        assert_eq!(table.rows[19].columns, Some(vec![(0, 0x8c), (0xc8, 0)]));
    }

    /// The Mannschaftsübersicht ranks the league's teams by their
    /// strength, the mean of their best players by position.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_ranks_the_leagues_strengths() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 2\nnow row 1284 9";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Nr.\\tVerein\\tStärke"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 18);
        assert_eq!(table.rows[0].text, "1.\tFC Bayern München\t108.0");
        assert_eq!(table.rows[8].text, "9.\t1.FC Union Berlin\t88.6");
        assert_eq!(table.rows[8].ink, [255, 0, 0]);
        assert_eq!(table.rows[17].text, "18.\tFC St. Pauli\t83.2");
    }

    /// The Eingesetzte Spieler, open with the department raised, counts
    /// the players each club fielded and those of them still in its squad.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_counts_the_leagues_fielded_players() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 2\nnow row 1284 14";
        let Some(runner) = on_save(&game, "A2", then) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Nr.\\tVerein\\tTotal\\takt. Kader\\taus akt. Kader\\tAnteil"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 18);
        assert_eq!(table.rows[0].text, "1.\tEintracht Frankfurt\t23\t26\t22\t84.6");
        assert_eq!(table.rows[17].text, "18.\t1.FC Union Berlin\t13\t26\t13\t50.0");
        assert_eq!(table.rows[17].ink, [255, 0, 0]);
    }

    /// The Durchschnittsalter ranks the league's squads by their mean
    /// age on the day, the youngest first.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_ranks_the_leagues_ages() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 2\nnow row 1284 10";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Pl.\\tVerein\\tSchnitt\\tUnter 22\\tÜber 30"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 18);
        assert_eq!(table.centred, -1);
        assert_eq!(table.rows[0].text, "1.\tSV Werder Bremen\t24.5\t8\t3");
        assert_eq!(table.rows[9].text, "10.\t1.FC Union Berlin\t25.3\t9\t5");
        assert_eq!(table.rows[9].ink, [255, 0, 0]);
        assert_eq!(table.rows[17].text, "18.\tFC Bayern München\t26.7\t3\t4");
    }

    /// The Ausländer counts each squad's foreigners and their nations.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_counts_the_leagues_foreigners() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 2\nnow row 1284 11";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Nr.\\tVerein\\tAnzahl\\tNationen"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 18);
        assert_eq!(table.rows[0].text, "1.\tBayer 04 Leverkusen\t22\t12");
        // Equal foreigners part by their nations.
        assert_eq!(table.rows[1].text, "2.\tSV Werder Bremen\t21\t15");
        assert_eq!(table.rows[7].text, "8.\t1.FC Union Berlin\t18\t15");
        assert_eq!(table.rows[7].ink, [255, 0, 0]);
        assert_eq!(table.rows[17].text, "18.\t1.FC Heidenheim\t4\t3");
    }

    /// The Plätze über Saisonziel lists the clubs by the places they stand
    /// above their season's target, below it negative.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_measures_the_leagues_targets() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 2\nnow row 1284 12";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Pl.\\tVerein\\tPlätze über Saisonziel"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 18);
        assert_eq!(table.rows[0].text, "1.\tTSG 1899 Hoffenheim\t4\t");
        assert_eq!(table.rows[14].text, "15.\t1.FC Union Berlin\t-7\t");
        assert_eq!(table.rows[14].ink, [255, 0, 0]);
        assert_eq!(table.rows[17].text, "18.\tBorussia M'gladbach\t-11\t");
    }

    /// The Häufigste Einw. and Ausw. Spieler list the players most often
    /// brought on and taken off, by their last names.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_lists_the_leagues_substitutes() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let liga = "now click bar 14\nuntil dialog 18\nnow row 1283 2";
        let Some(runner) = on_save(&game, "A", &format!("{liga}\nnow row 1284 13")) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Pl.\\tSpieler\\tVerein\\tEinwechslungen"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 20);
        // The window is handed the three matchdays played and one more.
        assert_eq!(table.centred, 4);
        assert_eq!(table.rows[0].text, "1.\tBitshiabu\tRB Leipzig\t3");
        assert_eq!(table.rows[19].text, "20.\tKumbedi\tVfL Wolfsburg\t1");
        let Some(runner) = on_save(&game, "A", &format!("{liga}\nnow row 1284 14")) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Pl.\\tSpieler\\tVerein\\tAuswechslungen"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows[0].text, "1.\tMatsima\tFC Augsburg\t3");
    }

    /// The Tore nach Einwechsl. lists the clubs whose substitutes scored,
    /// and says so where none did.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_lists_the_leagues_substitute_goals() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let page = "now click bar 14\nuntil dialog 18\nnow row 1283 2\nnow row 1284 16";
        let Some(runner) = on_save(&game, "A", page) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Pl.\\tVerein\\tTore\\tLetzter Spieltag"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let texts: Vec<&str> = table.rows.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts, ["1.\tBorussia Dortmund\t1\t1", "2.\tHamburger SV\t1\t1"]);
        // Five steps along the strip, Ligue 1 has none yet.
        let ligue = format!("{page}{}", "\nnow click at 545 436".repeat(5));
        let Some(runner) = on_save(&game, "A", &ligue) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 4780).unwrap()), "Ligue 1");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let texts: Vec<&str> = table.rows.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts, ["Es fielen noch keine Tore durch Einwechselspieler!"]);
    }

    /// The Erhaltene Elfmeter lists the clubs awarded a penalty as the
    /// game keeps them, with those they scored.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_lists_the_leagues_penalties() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 2\nnow row 1284 15";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Pl.\\tVerein\\tElfmeter\\tVerwandelt"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let texts: Vec<&str> = table.rows.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "1.\tBorussia M'gladbach\t1\t1",
                "2.\tVfL Wolfsburg\t1\t0",
                "3.\t1.FC Union Berlin\t1\t1"
            ]
        );
        assert_eq!(table.rows[2].ink, [255, 0, 0]);
    }

    /// The Häufigste Einwechsl. and Auswechsl. list the clubs by their
    /// substitutions, as the game keeps them.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_lists_the_leagues_substitutions() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let liga = "now click bar 14\nuntil dialog 18\nnow row 1283 2";
        let Some(runner) = on_save(&game, "A", &format!("{liga}\nnow row 1284 17")) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Pl.\\tVerein\\tEinw.\\tTore");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 18);
        // The substitutes' page ends its rows with a tab.
        assert_eq!(table.rows[0].text, "1.\tFC Augsburg\t9\t0\t");
        assert_eq!(table.rows[17].text, "18.\t1.FC Union Berlin\t1\t0\t");
        assert_eq!(table.rows[17].ink, [255, 0, 0]);
        let Some(runner) = on_save(&game, "A", &format!("{liga}\nnow row 1284 18")) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Pl.\\tVerein\\tAusw.\\tTore");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows[0].text, "1.\tFC Augsburg\t9\t0");
        assert_eq!(table.rows[17].text, "18.\t1.FC Union Berlin\t1\t0");
    }

    /// The Chancenverwertung reckons each club's quote of goals from its
    /// chances and ranks the league by it.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_ranks_the_leagues_chances() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 2\nnow row 1284 6";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 1292).unwrap()),
            "Nr.\\tVerein\\tQuote\\tChancen\\tTore"
        );
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 18);
        assert_eq!(table.centred, -1);
        // The own club leads with four goals from five chances, in red;
        // no dot after the rank.
        assert_eq!(table.rows[0].text, "1\t1.FC Union Berlin\t80%\t5\t4");
        assert_eq!(table.rows[0].ink, [255, 0, 0]);
        assert_eq!(table.rows[1].text, "2\tBorussia Dortmund\t44%\t9\t4");
        assert_eq!(table.rows[17].text, "18\t1.FC Heidenheim\t0%\t4\t0");
    }

    /// The four pages a third department opens in the Liga list all show
    /// the league's list the game saved: the mean form, the goal slices
    /// of either half, and the clean sheets, the own club red. They run
    /// on Forl, A2 with its statistics department raised to the third
    /// level - the saved lists are there in A2 either way.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_archive_shows_the_leagues_form_and_goal_times() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let liga = "now click bar 14\nuntil dialog 18\nnow row 1283 2";
        let Some(runner) = on_save(&game, "Forl", &format!("{liga}\nnow row 1284 19")) else {
            return
        };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Pl.\\tVerein\\tForm");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 18);
        assert_eq!(table.rows[0].text, "1.\t1.FSV Mainz 05\t87");
        assert_eq!(table.rows[15].text, "16.\t1.FC Union Berlin\t61");
        assert_eq!(table.rows[15].ink, [255, 0, 0]);
        assert_eq!(table.rows[17].text, "18.\tVfB Stuttgart\t48");
        // The two Wann trafen pages rank by the goals of their own half
        // and count them over the six quarter-hour slices.
        let minutes = "Pl.\\tVerein\\tTore\\t1-15\\t16-30\\t31-45\\t46-60\\t61-75\\t75-90";
        let Some(runner) = on_save(&game, "Forl", &format!("{liga}\nnow row 1284 20")) else {
            return
        };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), minutes);
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows[0].text, "1.\tFC Bayern München\t5\t1\t2\t0\t1\t1\t0");
        assert_eq!(table.rows[6].text, "7.\t1.FC Union Berlin\t4\t0\t1\t1\t0\t1\t1");
        assert_eq!(table.rows[6].ink, [255, 0, 0]);
        let Some(runner) = on_save(&game, "Forl", &format!("{liga}\nnow row 1284 21")) else {
            return
        };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), minutes);
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows[0].text, "1.\tSV Werder Bremen\t6\t2\t0\t0\t2\t2\t0");
        assert_eq!(table.rows[4].text, "5.\t1.FC Union Berlin\t4\t2\t0\t1\t0\t1\t0");
        assert_eq!(table.rows[4].ink, [255, 0, 0]);
        let Some(runner) = on_save(&game, "Forl", &format!("{liga}\nnow row 1284 22")) else {
            return
        };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Pl.\\tVerein\\tSpieltage\\tAnzahl");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 12);
        assert_eq!(table.rows[0].text, "1.\tRB Leipzig\t3\t3");
        assert_eq!(table.rows[8].text, "9.\t1.FC Union Berlin\t3\t1");
        assert_eq!(table.rows[8].ink, [255, 0, 0]);
    }

    /// The Stadion category's tables weigh the gates the league kept: the
    /// crowd per club sorted by its average, the own club's home take per
    /// fixture, the league's totals per matchday with the fullest red, the
    /// season tickets against the capacities, and the biggest stadiums of
    /// the country. Two of the category's ten pages stay hidden on A.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_stadion_weighs_gates_tickets_and_stadiums() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let stad = "now click bar 14\nuntil dialog 18\nnow row 1283 5";
        // Zuschauer Liga: the crowds sorted by their average, the own
        // club red, the league named on the strip.
        let Some(runner) = on_save(&game, "A", &format!("{stad}\nnow row 1284 3")) else {
            return
        };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 4780).unwrap()), "1. Bundesliga");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 18);
        assert_eq!(table.rows[0].text, "FC Bayern München\t2\t120.737\t60.368\t80%");
        assert_eq!(table.rows[17].text, "1.FC Heidenheim\t1\t11.481\t11.481\t76%");
        let union = table.rows.iter().position(|r| r.ink == [255, 0, 0]).unwrap();
        assert_eq!(table.rows[union].text, "1.FC Union Berlin\t1\t22.012\t22.012\t100%");
        // Einnahmen: every home fixture, the played one with gate and take.
        let Some(runner) = on_save(&game, "A", &format!("{stad}\nnow row 1284 4")) else {
            return
        };
        let table = runner.host.view().table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 17);
        assert_eq!(table.rows[0].text, "Borussia Dortmund\t22.012\t539.073 €");
        assert_eq!(table.rows[1].text, "RB Leipzig\t0\t 0 €");
        // Zuschauer Spieltag: the totals per matchday, the fullest red.
        let Some(runner) = on_save(&game, "A", &format!("{stad}\nnow row 1284 5")) else {
            return
        };
        let table = runner.host.view().table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows[0].text, "1.\t238.556");
        assert_eq!(table.rows[0].ink, [0, 0, 0]);
        assert_eq!(table.rows[2].text, "3.\t255.910");
        assert_eq!(table.rows[2].ink, [255, 0, 0]);
        // Dauerkarten: the own club's share of its capacity.
        let Some(runner) = on_save(&game, "A", &format!("{stad}\nnow row 1284 6")) else {
            return
        };
        let table = runner.host.view().table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 18);
        let union = table.rows.iter().position(|r| r.ink == [255, 0, 0]).unwrap();
        assert_eq!(table.rows[union].text, "14.\t1.FC Union Berlin\t9570\t22012\t43%");
        // Größte Stadien: the country's biggest first, the strip named.
        let Some(runner) = on_save(&game, "A", &format!("{stad}\nnow row 1284 7")) else {
            return
        };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 4780).unwrap()), "Deutschland");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows[0].text, "Signal Iduna Park\tDortmund\t81365\tBorussia Dortmund");
    }

    /// The Stadion category's three bar pages stand their bars on an
    /// axis: the own club's home games at their gates with the plan of
    /// attendance behind, the guest clubs' fans at those games, and the
    /// gates the away games drew — each labelled "name ( value )", the
    /// steps of a thousand and the unit by the axis's end.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_stadion_stands_bars_on_the_axis() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let stad = "now click bar 14\nuntil dialog 18\nnow row 1283 5";
        // Auslastung: the played home game's gate and the plan's bar.
        let Some(runner) = on_save(&game, "A", &format!("{stad}\nnow row 1284 0")) else {
            return
        };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 4780).unwrap()), "");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let graph = table.graph.as_ref().unwrap();
        assert_eq!(
            table.rows.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
            ["Borussia Dortmund ( 22012 )", "Kalkuliert ( 22012 )"]
        );
        assert_eq!(graph.steps, 23);
        assert_eq!(graph.bars, vec![(354, [0, 0, 0]), (354, [0, 0, 0])]);
        assert_eq!(graph.top, 10);
        assert_eq!(graph.unit, "Zuschauer/1000");
        // Gegnerische Fans: the fans the guest brought, its own steps.
        let Some(runner) = on_save(&game, "A", &format!("{stad}\nnow row 1284 1")) else {
            return
        };
        let table = runner.host.view().table(control(dialog, 29999).unwrap()).unwrap();
        let graph = table.graph.as_ref().unwrap();
        assert_eq!(table.rows.len(), 1);
        assert_eq!(table.rows[0].text, "Borussia Dortmund ( 3926 )");
        assert_eq!(graph.steps, 4);
        assert_eq!(graph.bars, vec![(363, [0, 0, 0])]);
        assert_eq!(graph.unit, "Zuschauer/1000");
        // Auswärtsauslastung: the gates the two away games drew.
        let Some(runner) = on_save(&game, "A", &format!("{stad}\nnow row 1284 2")) else {
            return
        };
        let table = runner.host.view().table(control(dialog, 29999).unwrap()).unwrap();
        let graph = table.graph.as_ref().unwrap();
        assert_eq!(
            table.rows.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
            ["Hamburger SV ( 57000 )", "VfL Wolfsburg ( 24589 )"]
        );
        assert_eq!(graph.steps, 6);
        assert_eq!(graph.bars, vec![(352, [0, 0, 0]), (152, [0, 0, 0])]);
    }

    /// The Stadion category's two development pages join the list once
    /// the manager has seen a season end: on a copy of A with his season
    /// count raised, the gates and the plan of attendance stand as two
    /// curves over the years, the sold season tickets and the target the
    /// preseason sells towards as bars.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_stadion_develops_its_crowds_and_tickets() {
        use crate::controls::table::Shape;
        let game = game();
        let dialog = &game.dialogs[&18];
        let stad = "now click bar 14\nuntil dialog 18\nnow row 1283 5";
        // The plain A's manager stands in his first season: eight pages.
        let Some(runner) = on_save(&game, "A", stad) else { return };
        let list = runner.host.view().list(control(dialog, 1284).unwrap()).unwrap();
        assert_eq!(list.rows.len(), 8);
        // On the copy whose manager reached a second season, the two
        // development pages come between the matchday and the season
        // ticket pages.
        let Some(runner) = on_save(&game, "Ap", stad) else { return };
        let list = runner.host.view().list(control(dialog, 1284).unwrap()).unwrap();
        assert_eq!(list.rows.len(), 10);
        // Zuschauerentwicklung: the one season's gate sum and planned
        // attendance, the first point handed in twice, the unit risen to
        // hundred-thousands — the plan times the league's eighteen clubs
        // near the top, the gates at the axis.
        let then = format!("{stad}\nnow row 1284 6");
        let Some(runner) = on_save(&game, "Ap", &then) else { return };
        let table = runner.host.view().table(control(dialog, 29999).unwrap()).unwrap();
        assert!(table.rows.is_empty());
        let graph = table.graph.as_ref().unwrap();
        assert!(graph.bars.is_empty());
        let curves = graph
            .shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Pen { from, to, colour } if *colour != [0, 0, 0] => {
                    Some((*from, *to, *colour))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            curves,
            [
                ((6, 301), (232, 301), [255, 255, 255]),
                ((6, 11), (232, 11), [255, 0, 0])
            ]
        );
        // Dauerkartenentwicklung: the league the season tickets were sold
        // in, counted from one, and the target as the "Kalkuliert" bar,
        // both on steps of a thousand without the goal pages' offset.
        let then = format!("{stad}\nnow row 1284 7");
        let Some(runner) = on_save(&game, "Ap", &then) else { return };
        let table = runner.host.view().table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(
            table.rows.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
            ["1. Liga (9570)", "Kalkuliert (9184)"]
        );
        let graph = table.graph.as_ref().unwrap();
        assert_eq!(graph.steps, 10);
        assert_eq!(graph.bars, vec![(354, [0, 0, 0]), (340, [0, 0, 0])]);
        assert_eq!(graph.top, 0);
        assert_eq!(graph.unit, "Dauerkarten/1000");
    }

    /// In his second season the manager's Finanzen list the Werbeeinnahmen
    /// as their eighth page (condition 0x11): S2c, a day of Union's second
    /// season the original played on to from A, shows last season's
    /// sponsor money from the human's extra record and the running
    /// season's, newest first, as the original does on the same save.
    /// S2c, a day of Union's second season the original played on to
    /// from A, as the only save of a game of its own; `None` where it is
    /// not kept.
    fn second_season(out: &str) -> Option<(PathBuf, Game)> {
        kept_game("S2c", out)
    }

    /// A kept save as the only save of a game of its own; `None` where it
    /// is not kept.
    fn kept_game(save: &str, out: &str) -> Option<(PathBuf, Game)> {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        if !dir.parent().unwrap().join(format!("saves-kept/{save}.an3")).exists() {
            eprintln!("no {save} among the kept saves");
            return None;
        }
        let root = patched(&dir, save, out, |_| {});
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        Some((root, game))
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_second_season_lists_the_sponsor_money() {
        let Some((root, game)) = second_season("second-season") else { return };
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 6\nnow row 1284 7";
        let Some(runner) = on_save(&game, "second-season", then) else { return };
        let view = runner.host.view();
        assert_eq!(view.list(control(dialog, 1284).unwrap()).unwrap().rows.len(), 8);
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(
            table.rows.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
            ["2026 (1.31 Mio €)", "2025 (8.87 Mio €)"]
        );
        let graph = table.graph.as_ref().unwrap();
        assert_eq!(graph.bars.len(), 2);
        assert!(graph.bars[1].0 > graph.bars[0].0, "last season's bar is the longer");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The Historisches' Endtabelle of the season gone by: the Bundesliga
    /// 2025/26 as the archive keeps it on S2c, reckoned from the stored
    /// results — St. Pauli champions with 54 points, Union third in red —,
    /// the places' boxes cyan, teal and green above and lavender for the
    /// three going down, as the original draws them.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_endtabelle_shows_the_season_gone_by() {
        let Some((root, game)) = second_season("endtabelle") else { return };
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 4\nnow row 1284 0";
        let Some(runner) = on_save(&game, "endtabelle", then) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), r"Nr.\tVerein\tPkt\tDiff\tTore");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 18);
        assert_eq!(table.rows[0].text, "1.\tFC St. Pauli\t54\t14\t41\t:\t27");
        assert_eq!(table.rows[2].text, "3.\t1.FC Union Berlin\t53\t8\t41\t:\t33");
        assert_eq!(table.rows[2].ink, [255, 0, 0]);
        assert_eq!(table.rows[17].text, "18.\t1.FC Heidenheim\t33\t-18\t32\t:\t50");
        assert_eq!(table.rows[0].mark, Some([0, 255, 255]));
        assert_eq!(table.rows[1].mark, Some([0, 128, 128]));
        assert_eq!(table.rows[15].mark, Some([160, 160, 255]));
        assert_eq!(table.rows[6].mark, None);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The Historisches' Resultate of the season gone by: the archive's
    /// Bundesliga 2025/26 on S2c laid crosswise in its own order under
    /// "Resultate letzte Saison", each cell the row's home game, the own
    /// club's name red but its cells black, as the original paints them.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_resultate_show_the_season_gone_by_crosswise() {
        let Some((root, game)) = second_season("resultate") else { return };
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 4\nnow row 1284 1";
        let Some(runner) = on_save(&game, "resultate", then) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Resultate letzte Saison");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.cross, Some(0));
        assert_eq!(table.rows.len(), 18);
        assert!(table.rows[0].text.starts_with("1 FC Bayern München\t\t1:2\t2:0\t1:1"), "{}", table.rows[0].text);
        assert!(table.rows[1].text.starts_with("2 Bayer 04 Leverkusen\t0:1\t\t3:0"), "{}", table.rows[1].text);
        assert_eq!(table.rows[12].ink, [255, 0, 0], "{}", table.rows[12].text);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The Historisches' kept lists on S2c, as the original shows them:
    /// the Champions League's final of 2025/26 (Int. Wettbewerbe, the
    /// newest at the top), the UEFA cup's decided on penalties, the
    /// Bundesliga's top scorers of the season gone by, its best tallies and
    /// Germany's most capped players.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_historisches_show_the_kept_lists() {
        let Some((root, game)) = second_season("kept-lists") else { return };
        let dialog = &game.dialogs[&18];
        let rows = |page: usize, steps: usize| {
            let then = format!(
                "now click bar 14\nuntil dialog 18\nnow row 1283 4\nnow row 1284 {page}{}",
                "\nnow click at 546 436".repeat(steps)
            );
            let runner = on_save(&game, "kept-lists", &then).unwrap();
            let view = runner.host.view();
            let table = view.table(control(dialog, 29999).unwrap()).unwrap();
            table.rows.iter().map(|r| r.text.clone()).collect::<Vec<_>>()
        };
        assert_eq!(rows(2, 0), ["2025/26\tJuventus Turin\t-\tFC Arsenal\t1\t:\t0\t "]);
        assert_eq!(rows(2, 1), ["2025/26\tReal Betis Sevilla\t-\tFeyenoord Rotterdam\t3\t:\t4\tn.E."]);
        assert_eq!(rows(7, 0), ["2025/26\tTim Kleindienst\tBorussia M'gladbach\t17"]);
        assert_eq!(
            rows(8, 0)[..2],
            ["2025/26\tTim Kleindienst\tBorussia M'gladbach\t17", "2025/26\tMarvin Pieringer\t1.FC Heidenheim\t16"]
        );
        assert_eq!(rows(9, 0)[..2], ["1.\tNico Schlotterbeck\t18\t*", "2.\tLeroy Sané\t17\t*"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The Historisches' honours on S2c, as the original shows them: the
    /// Bundesliga's champions and the cup's winners, the world cup with
    /// its top scorer, the trainer and the player of the year, Europe's
    /// footballer, the most successful trainers and the season's best
    /// squad, each page's first row.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_historisches_show_the_honours() {
        let Some((root, game)) = second_season("honours") else { return };
        let dialog = &game.dialogs[&18];
        let first = |page: usize| {
            let then = format!("now click bar 14\nuntil dialog 18\nnow row 1283 4\nnow row 1284 {page}");
            let runner = on_save(&game, "honours", &then).unwrap();
            let view = runner.host.view();
            let table = view.table(control(dialog, 29999).unwrap()).unwrap();
            table.rows.first().map(|r| r.text.clone()).unwrap_or_default()
        };
        assert_eq!(
            [11, 12, 17, 18, 19, 20, 21, 22, 24].map(first),
            [
                "2025/26\tFC St. Pauli\t54\t14\t41\t:\t27",
                "2025\tFC Viktoria Köln\t-\t1.FSV Mainz 05\t0\t:\t2\t ",
                "2026\tBelgien\tFrankreich\t-\tMexiko\t2\t:1\t ",
                "2026\tBelgien\tMaeda (Japan)\t3",
                "2025\tAlexander Blessin\tFC St. Pauli",
                "2025\tSilvan Widmer\t1.FSV Mainz 05",
                "2025\tKylian Mbappé\tReal Madrid",
                "1.\tBlessin, A.\t1\t0\t0\t1\t0",
                "2025/26\tFC Chelsea\tEngland\t314\t31\t10.1",
            ]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The Historisches a statistics department of level 3 adds, on S2c
    /// with Union's department raised (S2c3), each page's first row as the
    /// original shows it: the eternal home and away tables, the
    /// Herbstmeister, the Rekordmeister, the clubs most often promoted and
    /// relegated, and the players.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_third_department_shows_more_history() {
        let Some((root, game)) = kept_game("S2c3", "third-department") else { return };
        let dialog = &game.dialogs[&18];
        let first = |page: usize| {
            let then = format!("now click bar 14\nuntil dialog 18\nnow row 1283 4\nnow row 1284 {page}");
            let runner = on_save(&game, "third-department", &then).unwrap();
            let view = runner.host.view();
            let table = view.table(control(dialog, 29999).unwrap()).unwrap();
            table.rows.first().map(|r| r.text.clone()).unwrap_or_default()
        };
        assert_eq!(
            [17, 18, 19, 20, 31, 32, 35, 36].map(first),
            [
                "1.\tBorussia Dortmund\t26\t52\t24\t54\t:\t30",
                "1.\t1.FSV Mainz 05\t26\t41\t4\t35\t:\t31",
                "2026/27\tBorussia Dortmund\t33\t5\t32\t:\t27",
                "1.\tFC Bayern München\t34\t20\t8",
                "1\tFC Hansa Rostock\t2\t0\t1\t1",
                "1.\tSC Freiburg\t1\t1\t0\t0\t0",
                "1\tBenjamin Uphoff\t2\tFC Hansa Rostock",
                "1\tDiant Ramaj\t1\t1.FC Heidenheim",
            ]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The Mannschaft's Restprogramm in the last matchdays (S2late3: day
    /// 300 of Union's second season, the original played on from S2c, the
    /// department raised to 3): the six clubs from three above Union to two
    /// below, each with its fixtures to come and an empty row; Union red,
    /// and Werder's home game against it, as the original shows them.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_restprogramm_shows_the_last_fixtures() {
        let Some((root, game)) = kept_game("S2late3", "restprogramm") else { return };
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 1\nnow row 1284 26";
        let runner = on_save(&game, "restprogramm", then).unwrap();
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), r"Verein\tPlatz\tPunkte\tTore");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let texts: Vec<&str> = table.rows.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts[..7], ["FC Augsburg\t10\t39\t38\t:\t45", "A: Eintracht Frankfurt", "H: VfL Wolfsburg",
            "A: TSG 1899 Hoffenheim", "H: FC Bayern München", "A: Bayer 04 Leverkusen", ""]);
        assert_eq!(texts.len(), 6 * 7);
        assert_eq!(texts[14], "SV Werder Bremen\t12\t36\t45\t:\t47");
        assert_eq!((texts[15], table.rows[15].ink), ("H: 1.FC Union Berlin", [255, 0, 0]));
        assert_eq!((texts[21], table.rows[21].ink), ("1.FC Union Berlin\t13\t35\t28\t:\t37", [255, 0, 0]));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The ticket office's advance sales, as the original makes them on A
    /// with its generator set to 12345/67890 and played to the next day:
    /// the week's end opens a record for Union's away match at club 10 on
    /// day 90 and sells 188 at once (one percent of a demand of 18.8
    /// thousand, drawn), and the day sells on Union's home match on day
    /// 69 from 3,588 to 4,605 (six percent of 16,955) — the records, the
    /// prices and the draws as the original's, watched by the probe
    /// (`FUN_0077ca2b`, `FUN_0077c8b5`, `FUN_0078ab55`).
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_ticket_office_sells_ahead_as_the_original() {
        let game = game();
        let then = "now random 12345 67890\non 422 ok\non 420 ok\non 421 ok\non 287 ok\non 321 ok\non 25 ok\non 341 ok\non 120 ok\non 51 ok\non 392 ok\non 435 ok\non 96 ok\non 161 ok\non 422 ok\non 419 ok\non 419 ok\non 477 ok\non 380 press 4731\non 381 wait\non 2 wait\non 147 click bar 18\non 302 click bar 18\non 384 press 4741\non 308 choose 4095 0\non 93 press 1943\non 383 click at 460 210\non 99 press 2\non 307 click at 320 240\non 310 click at 320 240\non 331 press 4303\non 65 click at 165 330\non 423 ok\non 1 click at 320 357\non 375 click at 617 182; press 4715\non 270 ok\nuntil day 64";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let home = a3_core::tickets::sale(records, (14, 84), (14, 2), 69).unwrap();
        let away = a3_core::tickets::sale(records, (14, 10), (14, 84), 90).unwrap();
        assert_eq!((home.sold, home.prices), (4605, [17, 23, 43, 150]));
        assert_eq!((away.sold, away.prices), (188, [15, 20, 40, 150]));
    }

    /// The walk after the whistle draws as the original's, draw for draw:
    /// A with its generator set to 12345/67890, the turn ended and walked
    /// to the open day's dialog. The probe saw eleven draws on the way —
    /// the week's end opening a far match's sale (below 2), the human
    /// clubs' event gates (20, 100 and 3 a slot), the day's sale (2), the
    /// training's event pick (10000) and its luck (26 and 450) — and the
    /// dialog itself drawing none (`FUN_0064e40f`, `FUN_0064f981`,
    /// `FUN_00749508`, `FUN_0074a1ec`). The rest of the day follows as
    /// the original's, draw for draw by the probe's trace: the news after
    /// the friendlies, the world's day — 5314 draws to the next day's
    /// walk, whose computer clubs' training week parts 1801 draws on.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_day_draws_as_the_original_to_its_open_day() {
        use a3_core::random::Random;
        let game = game();
        let then = "now random 12345 67890\non 2 wait\non 381 wait\non 147 click bar 18\nuntil dialog 270";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let mut original = Random::from_state(12345, 67890);
        let drawn: Vec<u32> =
            [2, 20, 100, 3, 3, 3, 3, 2, 10_000, 26, 450].iter().map(|&n| original.below(n)).collect();
        assert_eq!(drawn, [1, 3, 6, 2, 1, 0, 1, 0, 2619, 12, 51]);
        assert_eq!(runner.host.state().season.as_ref().map(|s| s.random.clone()), Some(original));
        drop(runner);
        let day = format!("{then}\non 270 ok\nuntil day 64");
        let Some(runner) = on_save(&game, "A", &day) else { return };
        let mut original = Random::from_state(12345, 67890);
        for _ in 0..5314 {
            original.below(2);
        }
        let state = runner.host.state();
        assert_eq!(state.season.as_ref().map(|s| s.random.clone()), Some(original));
        // The day's close stepped the game's own counter and date, as
        // the next day's computer clubs read them for their men's days
        // out (`FUN_006504b6`, `FUN_00675a73`).
        let globals = &state.records.as_ref().unwrap().globals;
        assert_eq!((globals.day(), globals.date()), (64, [9, 9, 2025]));
    }

    /// The dial the running week answers to: its orange circle lives in
    /// the screen's own background art, and the replica only gives its
    /// click area an id of its own, 9932 — no bar of the menu stands under
    /// the week — while the week walks its days on its own and the dial
    /// walks one on at once (the original's click id breaking into the
    /// week, `FUN_00710839`, and `FUN_0062a959` moving the day). Watched
    /// on A at its first week's walk, two dials deep.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_week_walks_to_its_dial() {
        let game = game();
        let load = "now random 12345 67890\non 422 ok\non 420 ok\non 421 ok\non 287 ok\non 321 ok\non 25 ok\non 341 ok\non 120 ok\non 51 ok\non 392 ok\non 435 ok\non 96 ok\non 93 press 1943\non 499 ok\non 161 ok\non 422 ok\non 419 ok\non 419 ok\non 477 ok\non 380 press 4731\non 381 wait\non 2 wait\non 147 click bar 18\non 302 click bar 18\non 384 press 4741\n";
        let walk = format!("{load}until dialog 2");
        let dialled = format!("{walk}\nnow press 9932\nnow press 9932\nuntil dialog 2");
        let Some(plain) = on_save(&game, "A", &walk) else { return };
        let Some(runner) = on_save(&game, "A", &dialled) else { return };
        // The two presses of the week's own dial were taken, and the walk
        // stood where it stood before them.
        let presses = |runner: &Runner| {
            runner.log.iter().filter(|l| l.trim() == "Press(9932)").count()
        };
        assert_eq!(presses(&runner), presses(&plain) + 2);
        assert_eq!(
            runner.host.state().season.as_ref().map(|s| s.day),
            plain.host.state().season.as_ref().map(|s| s.day)
        );
        let view = runner.host.view();
        assert_eq!(view.id, a3_core::cycle::dialogs::WEEKS[1]);
        assert!(is_week(view.id));
        let dialog = &game.dialogs[&view.id];
        assert!(
            !dialog.controls.iter().any(|c| matches!(c.kind, Kind::Bar { .. })),
            "the week shows no bar of the menu"
        );
        let dial = dialog
            .controls
            .iter()
            .position(|c| c.id == Some(9932))
            .expect("the week's dial");
        assert_eq!(
            dialog.controls[dial].rect,
            bevy_math::Rect::new(600.0, 448.0, 640.0, 478.0)
        );
        assert!(view.controls[dial].state.as_bitmap().unwrap().clickable);
    }

    #[test]
    #[ignore = "needs the game"]
    fn the_finanzen_pays_salaries_and_estimates_values() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let fin = "now click bar 14\nuntil dialog 18\nnow row 1283 6";
        // The plain A's manager stands in his first season: seven pages,
        // the Werbeeinnahmen hidden.
        let Some(runner) = on_save(&game, "A", fin) else { return };
        let list = runner.host.view().list(control(dialog, 1284).unwrap()).unwrap();
        assert_eq!(list.rows.len(), 7);
        // Gehälter: the whole squad of 26 by salary, biggest first; the
        // best paid and the last one shown, with D-Form and trend.
        let then = format!("{fin}\nnow row 1284 2");
        let Some(runner) = on_save(&game, "A", &then) else { return };
        let table = runner.host.view().table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(table.rows.len(), 26);
        assert_eq!(
            table.rows[0].text,
            "Danilho Doekhi\tIV\t10\t14\t793.000 €\t9.900 €\t12.7\t+"
        );
        assert_eq!(
            table.rows[24].text,
            "Carl Klaus\tTO\t6\t15\t64.000 €\t 800 €\t13.9\t+"
        );
        // Marktwerte: the same squad by market value, the top player at
        // 10.06 million, none of them with a release fee.
        let then = format!("{fin}\nnow row 1284 3");
        let Some(runner) = on_save(&game, "A", &then) else { return };
        let table = runner.host.view().table(control(dialog, 29999).unwrap()).unwrap();
        assert_eq!(
            table.rows[0].text,
            "Danilho Doekhi\tIV\t10\t14\t10.06 Mio €\tNein\t12.7\t+"
        );
    }

    /// The Finanzen category's two line graphs draw the own club's past
    /// weeks: the cash curve in blue, the merchandise turnover in black.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_finanzen_draws_the_weeks_of_cash_and_merchandise() {
        use crate::controls::table::Shape;
        let game = game();
        let dialog = &game.dialogs[&18];
        let fin = "now click bar 14\nuntil dialog 18\nnow row 1283 6";
        let texts = |shapes: &[Shape]| -> Vec<String> {
            shapes
                .iter()
                .filter_map(|s| match s {
                    Shape::Text { text, .. } => Some(text.clone()),
                    _ => None,
                })
                .collect()
        };
        // Kassenstand: ten weeks leave nine blue segments; the axis
        // steps a million apiece, its label at the zero line 17 under.
        let then = format!("{fin}\nnow row 1284 1");
        let Some(runner) = on_save(&game, "A", &then) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Entwicklung Kassenstand");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        assert!(table.rows.is_empty());
        let shapes = &table.graph.as_ref().unwrap().shapes;
        assert_eq!(texts(shapes), ["Kassenstand", "Woche", "1.00 Mio €"]);
        let blue = shapes
            .iter()
            .filter(|s| matches!(s, Shape::Pen { colour: [0, 0, 255], .. }))
            .count();
        assert_eq!(blue, 9);
        // Fanartikelumsatz: eighteen weeks on 318 pixels of height, the
        // zero line on the bottom - the line starts flat off the axis.
        let then = format!("{fin}\nnow row 1284 5");
        let Some(runner) = on_save(&game, "A", &then) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Fanartikelumsatz");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let shapes = &table.graph.as_ref().unwrap().shapes;
        assert_eq!(texts(shapes), ["Fanartikelumsatz"]);
        assert!(shapes.iter().any(|s| matches!(
            s,
            Shape::Pen { from: (6, 326), to: (31, 326), colour: [0, 0, 0] }
        )));
    }

    /// The Trikots page names the own club's best-selling shirts, most
    /// first and twenty at most — former players and the manager himself
    /// among them, ties following the swap sort's order.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_finanzen_names_the_best_selling_shirts() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 6\nnow row 1284 6";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        // The text itself names the two columns with a literal "\t".
        assert_eq!(view.text(control(dialog, 1292).unwrap()), r"Spieler\tTrikots");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        // Twenty of the 31 sellers shown, centred for all 31.
        assert_eq!(table.rows.len(), 20);
        assert_eq!(table.centred, 31);
        assert_eq!(table.rows[0].text, "Diogo Leite\t343");
        // 28 twice, then 25 twice: the exchange sort keeps the slot's
        // order where the counts are equal.
        assert_eq!(table.rows[8].text, "Andrej Ilic\t28");
        assert_eq!(table.rows[9].text, "David Preu\t28");
        assert_eq!(table.rows[13].text, "Toni Lubiniecki\t22");
    }

    /// Saisonbilanzen sums up the running season in one row: the class
    /// from 1, place, points and goals, the squad's strength as the club
    /// keeps it anew (207, where the save still holds 210) and the result
    /// booked so far; a first season has no kept ones below it.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_finanzen_sums_up_the_season() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 6\nnow row 1284 0";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert!(view.text(control(dialog, 1292).unwrap()).starts_with("Saison"));
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let rows: Vec<&str> = table.rows.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(rows, ["25/26\t1\t11\t\t3\t4\t:\t4\t207\t3.67 Mio €"]);
    }

    /// Transferbilanz draws the season's net transfers as a bar: A's
    /// loss of 5.78 million green over the whole 384 pixels of its six
    /// million-steps, standing five lower than the goal pages' bars.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_finanzen_draws_the_transfer_balance() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let then = "now click bar 14\nuntil dialog 18\nnow row 1283 6\nnow row 1284 4";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 1292).unwrap()), "Entwicklung Transferbilanz");
        let table = view.table(control(dialog, 29999).unwrap()).unwrap();
        let rows: Vec<&str> = table.rows.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(rows, ["2025 ( - 5.78 Mio € )"]);
        let graph = table.graph.as_ref().unwrap();
        assert_eq!(graph.bars, vec![(384, [0, 0xff, 0])]);
        assert_eq!((graph.steps, graph.top), (6, 5));
    }

    /// The Spiel category lists the last match as the engine stored it:
    /// Zweikämpfe by the duel quote, the keeper first where all are
    /// level; Härte by the shots, as the original slipped, which puts
    /// Moscardo first; the opponent's side is empty on A.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_spiel_category_lists_the_last_match() {
        let game = game();
        let dialog = &game.dialogs[&18];
        let spiel = "now click bar 14\nuntil dialog 18\nnow row 1283 7";
        let rows = |then: &str| -> Option<Vec<String>> {
            let runner = on_save(&game, "A", then)?;
            let table = runner.host.view().table(control(dialog, 29999).unwrap()).unwrap().clone();
            Some(table.rows.iter().map(|r| r.text.clone()).collect())
        };
        let Some(duels) = rows(&format!("{spiel}\nnow row 1284 0")) else { return };
        assert_eq!(duels.len(), 11);
        assert_eq!(duels[0], "Frederik Rönnow\t45\tTO\t3.0\t0\t0\t0\t0.0\t0\t0");
        let Some(fouls) = rows(&format!("{spiel}\nnow row 1284 5")) else { return };
        assert_eq!(fouls[0], "Gabriel Moscardo\t45\tDM\t0\t0\t0\t0\t0");
        let Some(theirs) = rows(&format!("{spiel}\nnow row 1284 5\nnow click at 545 436")) else {
            return;
        };
        assert!(theirs.is_empty());
    }

    /// The long-term training's 4418 opens the training statistics: on
    /// A twenty players train well for their super condition, none badly;
    /// the units list all but the eleventh, the season's and last week's;
    /// the graph draws nine weeks in three curves. Patched, a tired
    /// keeper trains badly, one with morale too low and tired as well
    /// names only the last reason, and one with reasons both ways shows
    /// in neither list.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_training_statistics_list_who_trains_well_and_badly() {
        let open = "now click bar 3\nuntil dialog 345\nnow press 4418\nuntil dialog 406";
        let rows = |game: &Game, save: &str| -> Option<[Vec<String>; 3]> {
            let runner = on_save(game, save, open)?;
            let dialog = &game.dialogs[&406];
            let list = |id: u16| runner.host.view().list(control(dialog, id).unwrap()).unwrap().rows.clone();
            Some([list(4821), list(4822), list(4823)])
        };
        let game = game();
        let Some([good, bad, units]) = rows(&game, "A") else { return };
        assert_eq!(good.len(), 20);
        assert_eq!(good[0], "Rönnow, F.\tTO\t9\tSuper Kondition");
        assert!(bad.is_empty());
        assert_eq!(units.len(), 38);
        assert_eq!(units[6], "brutale Kondition\t36\t0");
        assert_eq!(units[7], "Ecken\t6\t2");
        let runner = on_save(&game, "A", open).unwrap();
        let graph = control(&game.dialogs[&406], 5526).unwrap();
        assert_eq!(runner.host.view().paint(graph).unwrap().lines.len(), 27);

        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "A", "training-moods", |records| {
            let (fit, fresh) = (records.players[3257].condition(), records.players[3257].freshness());
            let mut set = |id: usize, morale: u8, condition: i16, freshness: [i16; 3]| {
                let p = &mut records.players[id];
                p.set_morale(morale);
                p.set_condition(condition);
                p.set_freshness(freshness);
            };
            set(3257, 100, fit, [1000; 3]);
            set(3258, 120, 3000, fresh);
            set(3259, 80, fit, [1000; 3]);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let [good, bad, _] = rows(&game, "training-moods").unwrap();
        assert_eq!(good.len(), 17);
        assert_eq!(bad, ["Rönnow, F.\tTO\t9\tmüde", "Klaus, C.\tTO\t6\tmüde"]);
    }

    /// The camp's rooms seat the squad as their records keep them, a pair
    /// of best friends blue. A name dragged onto a free bed moves there;
    /// one dragged onto Bianca's picture swaps with her, and she shows in
    /// his room. OK keeps the rooms, Bianca's place aside: opened again,
    /// the one who stood there is back in his room and Bianca at hers.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_camp_rooms_swap_names_and_bianca() {
        use crate::controls::ControlState;
        let game = game();
        let dialog = &game.dialogs[&407];
        let open = "now click bar 3\nuntil dialog 345\nnow press 4890\nuntil dialog 407";
        let names = |runner: &Runner, ids: &[u16]| -> Vec<String> {
            ids.iter().map(|&id| runner.host.view().text(control(dialog, id).unwrap()).to_owned()).collect()
        };
        let Some(runner) = on_save(&game, "A", open) else { return };
        assert_eq!(names(&runner, &[4810, 4811, 4893]), ["Leite, D.", "Moore, M.", "Frei"]);
        let nsoki = control(dialog, 4812).unwrap();
        let ink = match &runner.host.view().controls[nsoki].state {
            ControlState::Static(s) => s.ink,
            _ => None,
        };
        assert_eq!(ink, Some([0, 0, 255]));
        let then = format!("{open}\nnow drag 4810 4893\nnow drag 4811 4824");
        let runner = on_save(&game, "A", &then).unwrap();
        assert_eq!(names(&runner, &[4810, 4811, 4893]), ["Frei", "Bianca", "Leite, D."]);
        let again = format!("{then}\nnow press 1\nuntil dialog 345\nnow press 4890\nuntil dialog 407");
        let runner = on_save(&game, "A", &again).unwrap();
        assert_eq!(names(&runner, &[4810, 4811, 4893]), ["Moore, M.", "Frei", "Leite, D."]);
    }

    /// The year calendar's camp, dragged onto the 17th of September, asks
    /// for its place on the world map; the Dutch sports school confirmed,
    /// it starts there that day and ends the next. Cape Canaveral's point
    /// offers its group's three places, the Mars base not yet. Atlantis
    /// shows only in the lottery's lucky season.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_calendar_books_a_training_camp_from_the_world_map() {
        let game = game();
        let map = "now click bar 6\nuntil dialog 348\nnow drag at 480 210 to 180 260\nuntil dialog 488";
        let booked = format!("{map}\nnow click at 297 150\nuntil dialog 285\nnow press 1\nuntil dialog 348");
        let Some(runner) = on_save(&game, "A", &booked) else { return };
        let state = runner.host.state();
        let calendar = &state.records.as_ref().unwrap().calendars[0];
        let (start, end) = (a3_core::year::day(calendar, 72), a3_core::year::day(calendar, 73));
        assert!(start.camp_start() && start.place() == 2);
        assert!(end.kind() == a3_core::year::CAMP && !end.camp_start());

        let group = format!("{map}\nnow click at 132 200\nuntil dialog 50");
        let runner = on_save(&game, "A", &group).unwrap();
        let rows = control(&game.dialogs[&50], 1447).unwrap();
        let labels = &runner.host.view().options(rows).unwrap().labels;
        assert_eq!(labels[..3], ["Raumstation", "Mondbasis Alpha", "Marsbasis 51"]);
        let mars = format!("{group}\nnow click at 320 366\nuntil dialog 380");
        let runner = on_save(&game, "A", &mars).unwrap();
        assert_eq!(runner.host.view().id, 380);

        let atlantis = |runner: &Runner| {
            let dialog = &game.dialogs[&488];
            let shown = |id: u16| !runner.host.view().controls[control(dialog, id).unwrap()].hidden;
            (shown(6156), shown(100 + 20))
        };
        let runner = on_save(&game, "A", map).unwrap();
        assert_eq!(atlantis(&runner), (false, false));
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "A", "lucky-season", |records| records.globals.set_lottery(0));
        let mut lucky = Game::load(&root).unwrap();
        compose(&mut lucky);
        let runner = on_save(&lucky, "lucky-season", map).unwrap();
        assert_eq!(atlantis(&runner), (true, true));
    }

    /// On Probe1 (day 35) the Turnier dragged onto the 13th of August offers
    /// what the original offered there: hosting one's own, the invitations
    /// of SC Paderborn 07 and Chemnitzer FC, two rows empty. Paderborn's
    /// joined, its entry takes Ulm and both days show it. One's own, at
    /// random numbers poked into the original before the click, invites
    /// the three clubs the original's list held first and takes them as
    /// its entry 43; with one guest only, OK warns instead.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_calendar_joins_and_hosts_a_tournament() {
        let game = game();
        let offer = "now click bar 6\nuntil dialog 348\nnow drag at 540 253 to 180 222\nuntil dialog 296";
        let Some(runner) = on_save(&game, "Probe1", offer) else { return };
        let rows = control(&game.dialogs[&296], 3984).unwrap();
        let labels = runner.host.view().options(rows).unwrap().labels.clone();
        assert_eq!(labels, [
            "eigenes Turnier veranstalten",
            "Einladung SC Paderborn 07 annehmen",
            "Einladung Chemnitzer FC annehmen",
            "",
            ""
        ]);
        let calendar_day = |runner: &Runner, d: usize| {
            let state = runner.host.state();
            let day = a3_core::year::day(&state.records.as_ref().unwrap().calendars[0], d);
            (day.kind(), day.club(), day.value())
        };
        let tournament = |runner: &Runner, id: u32| {
            let state = runner.host.state();
            a3_core::tournament::find(state.records.as_ref().unwrap(), id).map(|t| t.clubs())
        };

        let runner = on_save(&game, "Probe1", &format!("{offer}\nnow click at 347 209\nuntil dialog 348")).unwrap();
        assert_eq!(tournament(&runner, 0x0e38_0025), Some(vec![(14, 56), (14, 82)]));
        assert_eq!(calendar_day(&runner, 37).0, a3_core::year::TOURNAMENT);
        assert_eq!(calendar_day(&runner, 37).1, (14, 56));
        assert_eq!((calendar_day(&runner, 38).0, calendar_day(&runner, 38).1), (a3_core::year::TOURNAMENT, (14, 56)));

        let own = format!("{offer}\nnow random 12345 67890\nnow click at 347 173\nuntil dialog 45");
        let hosted = format!("{own}\nnow row 1401 0\nnow row 1401 1\nnow row 1401 2\nnow press 1\nuntil dialog 348");
        let runner = on_save(&game, "Probe1", &hosted).unwrap();
        assert_eq!(tournament(&runner, 0x0e52_0025), Some(vec![(14, 82), (14, 103), (14, 11), (14, 27)]));
        let count = a3_core::tournament::tournaments(runner.host.state().records.as_ref().unwrap()).len();
        assert_eq!(count, 44);
        assert_eq!((calendar_day(&runner, 37).0, calendar_day(&runner, 37).1), (a3_core::year::TOURNAMENT, (14, 82)));

        let runner = on_save(&game, "Probe1", &format!("{own}\nnow row 1401 0\nnow press 1\nuntil dialog 380")).unwrap();
        assert_eq!(runner.host.view().id, 380);
    }

    /// Paderborn's Turnier joined on Probe1 and the turn ended: on its day
    /// its draw comes up over the week — the dates, the host, Ulm in the
    /// first semifinal as the second slot, the host against the fourth —,
    /// in its evening the results with the semifinals only, the game for
    /// third and the final with their headers empty, and after the second
    /// day all four with the fees, prizes, gate and crowd.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_joined_turnier_shows_its_draw_and_results() {
        let game = game();
        let ways = "on 422 ok\non 420 ok\non 421 ok\non 287 ok\non 321 ok\non 25 ok\non 341 ok\non 120 ok\n\
                    on 51 ok\non 392 ok\non 435 ok\non 96 ok\non 93 press 1943\non 499 ok\non 161 ok\non 419 ok\non 477 ok\n\
                    on 380 press 4731\non 381 wait\non 2 wait\non 384 press 4741\non 99 press 2\non 423 ok";
        let draw = format!(
            "now click bar 6\nuntil dialog 348\nnow drag at 540 253 to 180 222\nuntil dialog 296\n\
             now click at 347 209\nuntil dialog 348\nnow click bar 0\nuntil dialog 147\n{ways}\n\
             now click bar 18\nuntil dialog 298"
        );
        let Some(runner) = on_save(&game, "Probe1", &draw) else { return };
        let text = |runner: &Runner, dialog: u16, id: u16| {
            runner.host.view().text(control(&game.dialogs[&dialog], id).unwrap()).to_owned()
        };
        assert_eq!(text(&runner, 298, 4003), "Auslosung für das Turnier vom 13.8 - 14.8.2025");
        assert_eq!(text(&runner, 298, 3997), "SC Paderborn 07");
        assert_eq!(text(&runner, 298, 3998), "SSV Ulm 1846 -");
        assert_eq!(text(&runner, 298, 4000), "SC Paderborn 07 -");

        let first = format!("{draw}\nnow ok\nuntil dialog 299");
        let runner = on_save(&game, "Probe1", &first).unwrap();
        assert_eq!(text(&runner, 299, 4018), "SSV Ulm 1846");
        assert!(text(&runner, 299, 4019).starts_with("- "));
        assert!(text(&runner, 299, 4023).contains(':'));
        assert_eq!([text(&runner, 299, 4014), text(&runner, 299, 4017), text(&runner, 299, 4020)], ["", "", ""]);

        let second = format!("{first}\nnow press 4012\nuntil dialog 299");
        let runner = on_save(&game, "Probe1", &second).unwrap();
        assert_eq!([text(&runner, 299, 4014), text(&runner, 299, 4017)], ["Spiel um Platz 3", "Finale"]);
        assert!(text(&runner, 299, 4020).contains(':') && text(&runner, 299, 4021).contains(':'));
        assert_eq!(text(&runner, 299, 4026), "70.000 €");
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let t = a3_core::tournament::find(records, 0x0e38_0025).unwrap();
        assert!((0..4).all(|g| t.result(g).is_some()));
        let fees = crate::screens::playerinfo::money(&game, i64::from(t.takings().2));
        assert_eq!(text(&runner, 299, 4025), fees);
    }

    /// With a second human patched in at Arminia Bielefeld (manager 2,
    /// Lena Lux), the friendly of the 17th of September offers her below
    /// the list; her home button asks her (dialog 69) — Union's manager and
    /// club, "in seinem Stadion", the day — and her yes books the friendly
    /// into both calendars. A Turnier of one's own on that day offers her
    /// too; her button invites her (297) with both days and the host, and
    /// her yes takes her among those in.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn another_human_is_asked_to_a_friendly_and_a_turnier() {
        let Some(dir) = std::env::var_os("ANSTOSS3_DIR").map(PathBuf::from) else { return };
        if !dir.join("saves/A.an3").exists() && !dir.join("../saves-kept/A.an3").exists() {
            return;
        }
        let root = patched(&dir, "A", "friendly-human", |records| {
            const LIST: usize = 0xb5cbe2;
            let globals = &mut records.globals;
            globals.put::<u8>(LIST + 8, 14);
            globals.put::<i16>(LIST + 10, 43);
            globals.put::<i16>(LIST + 14, 2);
            globals.put::<i16>(0xb5cc02, 2);
            let team = records.team_mut(14, 43).unwrap();
            team.set_humans([2, 2]);
            team.set_human(2);
            let mut name = |at: usize, len: usize, text: &str| {
                let mut bytes = vec![0u8; len];
                bytes[..text.len()].copy_from_slice(text.as_bytes());
                for (k, &b) in bytes.iter().enumerate() {
                    records.managers[1].put::<u8>(at + k, b);
                }
            };
            name(0, 10, "Lena");
            name(0xa, 0x12, "Lux");
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let text = |runner: &Runner, dialog: u16, id: u16| {
            runner.host.view().text(control(&game.dialogs[&dialog], id).unwrap()).to_owned()
        };

        let offer = "now click bar 6\nuntil dialog 348\nnow drag at 540 403 to 180 260\nuntil dialog 72";
        let runner = on_save(&game, "friendly-human", offer).unwrap();
        assert_eq!(text(&runner, 72, 1667), "Arminia Bielefeld");
        let ask = format!("{offer}\nnow choose 1675 0\nuntil dialog 69");
        let runner = on_save(&game, "friendly-human", &ask).unwrap();
        let union = {
            let state = runner.host.state();
            let m = &state.records.as_ref().unwrap().managers[0];
            format!("{} {}", m.first_name(), m.last_name())
        };
        assert_eq!(
            text(&runner, 69, 1660),
            format!("Lena Lux, Ihr Kollege {union} (1.FC Union Berlin) bietet Ihnen ein Freundschaftsspiel in seinem Stadion an.")
        );
        assert_eq!(text(&runner, 69, 1661), "Der Termin wäre der 17.9");
        let runner = on_save(&game, "friendly-human", &format!("{ask}\nnow press 1662\nuntil dialog 348")).unwrap();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let day = |m: usize| {
            let d = a3_core::year::day(&records.calendars[m], 72);
            (d.kind(), d.club())
        };
        assert_eq!(day(0), (a3_core::year::FRIENDLY, (14, 43)));
        assert_eq!(day(1), (a3_core::year::FRIENDLY, (14, 84)));
        let booked = a3_core::friendly::friendlies(records);
        assert_eq!((booked.len(), booked[0].opponent, booked[0].away), (1, (14, 43), false));
        drop(state);

        let own = "now click bar 6\nuntil dialog 348\nnow drag at 540 253 to 180 260\nuntil dialog 296\n\
                   now click at 347 173\nuntil dialog 45";
        let runner = on_save(&game, "friendly-human", &format!("{own}\nnow choose 1405 0\nuntil dialog 297")).unwrap();
        let invitation = text(&runner, 297, 3992);
        assert!(invitation.starts_with("Lena Lux, sie erhalten eine Einladung für das Turnier"), "{invitation}");
        assert!(invitation.contains(" und ") && invitation.ends_with("Wollen Sie teilnehmen?"), "{invitation}");
        let taken = format!("{own}\nnow choose 1405 0\nuntil dialog 297\nnow press 1\nuntil dialog 45");
        let runner = on_save(&game, "friendly-human", &taken).unwrap();
        let rows = runner.host.view().list(control(&game.dialogs[&45], 1400).unwrap()).unwrap().rows.clone();
        assert!(rows[0].starts_with("Arminia Bielefeld\t"), "{rows:?}");
    }

    /// A friendly booked on A for the 9th of September — the first row at
    /// random numbers poked into the original before the drop, club 14/24
    /// at its own ground, no fee — and the days played on: the original
    /// shared a gate of 132 072 in halves of 66 036, kept Union's half as
    /// its manager's last share and counted the game with its season.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_friendly_takes_the_originals_gate() {
        let game = game();
        let ways = "on 422 ok\non 420 ok\non 421 ok\non 287 ok\non 321 ok\non 25 ok\non 341 ok\non 120 ok\n\
                    on 51 ok\non 392 ok\non 435 ok\non 96 ok\non 161 ok\non 419 ok\non 477 ok\n\
                    on 380 press 4731\non 270 press 3734\non 499 ok\non 381 wait\non 2 wait\n\
                    on 147 click bar 18\non 302 click bar 18\non 384 press 4741\non 308 choose 4095 0\n\
                    on 93 press 1943\non 383 click at 460 210\non 99 press 2\non 307 click at 320 240\n\
                    on 310 click at 320 240\non 331 press 4303\non 65 click at 165 330\non 423 ok\n\
                    on 1 click at 320 357\non 375 click at 617 182; press 4715";
        let book = "now click bar 6\nuntil dialog 348\nnow random 12345 67890\nnow drag at 540 403 to 180 166\n\
                    until dialog 72\nnow row 1666 0\nuntil dialog 348\nnow click bar 0\nuntil dialog 147";
        let Some(runner) = on_save(&game, "A", book) else { return };
        let gate = |runner: &Runner, club: (u8, u16)| {
            let state = runner.host.state();
            state.records.as_ref().unwrap().team(club.0.into(), club.1).unwrap().get::<i32>(0x4b4)
        };
        let booked = {
            let state = runner.host.state();
            a3_core::friendly::friendlies(state.records.as_ref().unwrap())
        };
        assert_eq!((booked[0].opponent, booked[0].away, booked[0].fee), ((14, 24), true, 0));
        let before = (gate(&runner, (14, 84)), gate(&runner, (14, 24)));
        let runner = on_save(&game, "A", &format!("{book}\n{ways}\nuntil day 65")).unwrap();
        assert_eq!(gate(&runner, (14, 84)) - before.0, 66_036);
        assert_eq!(gate(&runner, (14, 24)) - before.1, 66_036);
        let state = runner.host.state();
        let extra = &state.records.as_ref().unwrap().extras[0];
        assert_eq!((extra.get::<i32>(0x2560), extra.together()), (66_036, 1));
    }

    /// Union's own Hallenturnier on A for the 16th of September, seven of
    /// the offered clubs invited: after its second day its results come up
    /// over the week (dialog 94) — the host and its six thousand, the
    /// groups' games and tables, the knockout games, the money with the
    /// gain or the loss.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_hosted_hall_shows_its_results() {
        let game = game();
        let ways = "on 89 press 1875\non 422 ok\non 420 ok\non 421 ok\non 287 ok\non 321 ok\non 25 ok\non 341 ok\non 120 ok\n\
                    on 51 ok\non 392 ok\non 435 ok\non 96 ok\non 161 ok\non 419 ok\non 477 ok\n\
                    on 380 press 4731\non 270 press 3734\non 499 ok\non 381 wait\non 2 wait\n\
                    on 147 click bar 18\non 302 click bar 18\non 384 press 4741\non 308 choose 4095 0\non 332 ok\non 269 ok\n\
                    on 93 press 1943\non 383 click at 460 210\non 99 press 2\non 307 click at 320 240\n\
                    on 310 click at 320 240\non 331 press 4303\non 65 click at 165 330\non 423 ok\n\
                    on 1 click at 320 357\non 375 click at 617 182; press 4715";
        let invite = "now row 1401 0\n".repeat(7);
        let then = format!(
            "now click bar 6\nuntil dialog 348\nnow drag at 540 289 to 180 257\nuntil dialog 296\n\
             now click at 347 173\nuntil dialog 45\n{invite}now press 1\nuntil dialog 348\n\
             now click bar 0\nuntil dialog 147\n{ways}\nuntil dialog 94"
        );
        let Some(runner) = on_save(&game, "A", &then) else { return };
        let text = |id: u16| runner.host.view().text(control(&game.dialogs[&94], id).unwrap()).to_owned();
        assert_eq!(text(crate::screens::discipline::CAPTION_ID), "ANSTOSS 3 - Ergebnisse Hallenturnier");
        assert_eq!(text(1957), "Veranstalter: 1.FC Union Berlin (6000 Zuschauer)");
        assert!(text(1960).starts_with("1.FC Union Berlin - "), "{}", text(1960));
        assert!(text(1966).contains(" : ") && text(1990).ends_with(" Punkte"));
        assert_eq!(text(1953), "560.000 €");
        assert_eq!(text(1955), "100.000 €");
    }

    /// The friendly dragged onto the 17th of September offers, at random
    /// numbers poked into the original before the drop, the ten clubs the
    /// original offered (its trace drew the same thirty numbers): Sporting
    /// abroad at Union's ground for 140.000 €, the near ones for the gate.
    /// Sporting taken, the friendly list, the last friendlies and the day
    /// hold what the original's memory and calendar held after the click.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_calendar_books_a_friendly_with_a_club_offered() {
        let game = game();
        let offer = "now click bar 6\nuntil dialog 348\nnow random 12345 67890\n\
                     now drag at 540 403 to 180 260\nuntil dialog 72";
        let Some(runner) = on_save(&game, "A", offer) else { return };
        let list = control(&game.dialogs[&72], 1666).unwrap();
        let rows = runner.host.view().list(list).unwrap().rows.clone();
        assert_eq!(rows.len(), 10);
        assert_eq!(rows[0], "SV Babelsberg 03\t3\tA\tEinnahmenteilung");
        assert_eq!(rows[1], "Hertha BSC\t2\tA\tEinnahmenteilung");
        assert_eq!(rows[3], "Sporting Lissabon\t1\tH\t140.000 €");
        assert_eq!(rows[5], "SV Drochtersen\t3\tA\t50.000 €");
        assert_eq!(rows[9], "DJK Vilzing\t3\tA\t40.000 €");

        let runner = on_save(&game, "A", &format!("{offer}\nnow row 1666 3\nuntil dialog 348")).unwrap();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let booked = a3_core::friendly::friendlies(records);
        assert_eq!(booked, [a3_core::friendly::Friendly {
            id: 0x0e54_0048,
            day: 72,
            club: (14, 84),
            opponent: (51, 0),
            goals: (0, 0),
            away: false,
            fee: 140,
        }]);
        assert_eq!(a3_core::friendly::last_friendlies(records), [(72, (14, 84), (51, 0))]);
        let day = a3_core::year::day(&records.calendars[0], 72);
        assert_eq!((day.kind(), day.club()), (a3_core::year::FRIENDLY, (51, 0)));
    }

    /// The Finanzen screen's donations offer what the bank holds up to
    /// 100.000, in hundreds on the slider's steps; on A only good causes,
    /// no other human. Given, the amount joins the club's costs, the
    /// manager's popularity with the public and in the media rises by one,
    /// the board's confidence falls by one, the club's image rises by one.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_finanzen_screen_gives_donations() {
        let game = game();
        let dialog = &game.dialogs[&63];
        let open = "now click bar 9\nuntil dialog 417\nnow press 2222\nuntil dialog 63";
        let Some(runner) = on_save(&game, "A", open) else { return };
        let text = |runner: &Runner, id: u16| runner.host.view().text(control(dialog, id).unwrap()).to_owned();
        assert_eq!([text(&runner, 1628), text(&runner, 1629), text(&runner, 1630)], [
            "100.000 €",
            "5.92 Mio €",
            "17.42 Mio €"
        ]);
        let rows = runner.host.view().options(control(dialog, 6022).unwrap()).unwrap().labels.clone();
        assert_eq!(rows[1..], ["---", "---", "---"]);
        let figures = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let manager = &records.managers[0];
            let team = &records.teams[usize::from(manager.team())];
            let costs = team.get::<i32>(0x2c0 + a3_core::finances::CLUB_TO_TEAM);
            (manager.popularity(), manager.confidence(), team.public_image(), costs)
        };
        let before = figures(&runner);
        let slid = format!("{open}\nnow slide 1631 60");
        let runner = on_save(&game, "A", &slid).unwrap();
        assert_eq!(text(&runner, 1628), "10.000 €");
        let given = format!("{open}\nnow press 1\nuntil dialog 417");
        let runner = on_save(&game, "A", &given).unwrap();
        let ([public, fans, media], confidence, image, costs) = before;
        assert_eq!(
            figures(&runner),
            ([public + 1, fans, media + 1], confidence - 1, image + 1, costs + 100_000)
        );
    }

    /// The association's conditions: none on A. Patched with a debt limit,
    /// no building and no camps, they stand a line each and the outlook
    /// grants the licence; with 31 Mio more costs booked the chance is gone
    /// and the Finanzen screen refuses (text 3108) — not so for a club
    /// whose cash itself is in the red by more than 10 Mio in the first
    /// season, which gets it after all.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_finanzen_screen_shows_the_licence_conditions() {
        let open = "now click bar 9\nuntil dialog 417\nnow press 1619\nuntil dialog 64";
        let texts = |game: &Game, save: &str| -> Option<(String, String)> {
            let runner = on_save(game, save, open)?;
            let dialog = &game.dialogs[&64];
            let text = |id: u16| runner.host.view().text(control(dialog, id).unwrap()).to_owned();
            Some((text(1637), text(1636)))
        };
        let game = game();
        let Some((conditions, outlook)) = texts(&game, "A") else { return };
        assert_eq!(conditions, format!("{}\n", game.text(2444)));
        assert!(outlook.is_empty());

        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        fn own(records: &mut a3_core::records::Records) -> &mut a3_core::records::Team {
            let team = usize::from(records.managers[0].team());
            &mut records.teams[team]
        }
        let root = patched(&dir, "A", "licence-conditions", |records| {
            let team = own(records);
            team.put(0x12b, 0x08u8 | 0x01 | 0x02);
            team.put(0x5d8 + a3_core::finances::CLUB_TO_TEAM, 5u8);
        });
        let mut patched_game = Game::load(&root).unwrap();
        compose(&mut patched_game);
        let (conditions, outlook) = texts(&patched_game, "licence-conditions").unwrap();
        let debt = game.text(2811).replacen("%s", "5.00 Mio €", 1);
        assert_eq!(conditions, format!("{debt}\n{}\n{}\n", game.text(2441), game.text(2442)));
        assert!(outlook.starts_with(game.text(2819)), "{outlook}");

        let root = patched(&dir, "A", "licence-gone", |records| {
            let at = 0x2c0 + a3_core::finances::CLUB_TO_TEAM;
            let team = own(records);
            team.put(at, team.get::<i32>(at) + 31_000_000);
        });
        let mut broke = Game::load(&root).unwrap();
        compose(&mut broke);
        let refused = "now click bar 9\nuntil dialog 417\nnow press 1619\nuntil dialog 380";
        let runner = on_save(&broke, "licence-gone", refused).unwrap();
        assert_eq!(runner.host.view().id, 380);
    }

    /// A second human: A patched with his slot, without a club and one of
    /// the next team. The Finanzen screen offers him among the recipients;
    /// the offered 100.000 given to him, the screen refuses another
    /// donation this week (text 4602). Both managers whistle their turn
    /// off, and the week's end (`FUN_006eeda7`) books the amount with the
    /// recipient's other income (club +0x2c4) and empties the store.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_donation_to_another_human_is_booked_at_the_weeks_end() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "A", "second-human", |records| {
            records.globals.set_slots([1, 1, 0, 0]);
            let team = records.managers[0].team();
            records.managers[1].set_team(team + 1);
            records.managers[1].set_status(0);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let received = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let team = usize::from(records.managers[1].team());
            records.teams[team].get::<i32>(0x2c4 + a3_core::finances::CLUB_TO_TEAM)
        };
        let Some(before) = on_save(&game, "second-human", "") else { return };
        let before = received(&before);
        // Give the second human's club the 100.000 offered at first; the
        // refusal; then the manager ends his turn with the whistle — the
        // second, not under contract, plays none. The week runs on past
        // the whistles now that the season does not stop at a match, so
        // its waiting screens are swept; the script ends a day into the
        // new week, the store long since booked.
        let then = "on 384 press 4741\non 380 press 4731\non 218 click bar 0\n\
                    on 147 click bar 18\non 2 wait\non 381 wait\non 270 ok\n\
                    now click bar 9\nuntil dialog 417\nnow press 2222\nuntil dialog 63\n\
                    now choose 6022 1\nnow press 1\nuntil dialog 417\nnow press 2222\n\
                    until dialog 380\nuntil dialog 417\nnow click bar 18\n\
                    until dialog 2\nrepeat 1 day";
        let runner = on_save(&game, "second-human", then).unwrap();
        assert_eq!(received(&runner), before + 100_000, "{}", runner.log.join("\n"));
        assert!(runner.host.state().season.as_ref().unwrap().donations.is_empty());
    }

    /// The world map's points name their place: the pointer resting on the
    /// Dutch sports school's point, as long as one waits at a menu bar's
    /// symbol, shows its name as a tooltip (the points' tooltip
    /// "= its name").
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_world_map_names_the_place_under_the_pointer() {
        let game = game();
        let map = "now click bar 6\nuntil dialog 348\nnow drag at 480 210 to 180 260\nuntil dialog 488";
        let Some(mut runner) = on_save(&game, "A", map) else { return };
        let point = control(&game.dialogs[&488], 100 + 2).unwrap();
        let name = game.text(game.rules.year.places[2].name).to_owned();
        assert!(!name.is_empty());
        runner.host.view_mut().hovered = Some(Target::Button(point));
        // The rest only counts from the hover, not from the map's opening.
        runner.host.tick(&game, 0.5);
        assert_eq!(runner.host.view().tip, None, "{}", runner.log.join("\n"));
        runner.host.tick(&game, 0.5);
        assert_eq!(runner.host.view().tip, None);
        runner.host.tick(&game, 0.5);
        assert_eq!(runner.host.view().tip, Some((point, 0)));
        let tip = runner.host.view().controls[point].state.as_button().unwrap().tip.clone();
        assert_eq!(tip.as_deref(), Some(name.as_str()));
    }

    /// "Karriereende" (the dressing room's sixth game option,
    /// `FUN_005b13da`) asks with text 2691. Yes ends the turn, and the
    /// week's end the career (`FUN_006c5190`): Bert Beta, the first of
    /// Probe1's two humans, goes into the career list of the game's mode
    /// (`FUN_00656e9b`) at the first place with fewer points — the place
    /// it takes lost, the one under it standing twice, as the game's shift
    /// stops a place short —, the highscores show him, and he leaves the
    /// game to Anna Alpha. "Spiel beenden" enters his career the same, at
    /// once, and ends the game before the highscores show.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_career_ends_into_the_highscore_list() {
        use chrono::Datelike;
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "Probe1", "Karriereende", |_| {});
        // A list of the copy's own: the installation's is only linked.
        let scores = root.join("Highscore");
        std::fs::remove_file(&scores).unwrap();
        std::fs::create_dir(&scores).unwrap();
        let list: Vec<u8> = [100, 30, 20, 10, 5]
            .iter()
            .flat_map(|&points: &i32| {
                let mut entry = [0; 0x30];
                entry[..4].copy_from_slice(b"Old\0");
                entry[0x20..0x24].copy_from_slice(&points.to_le_bytes());
                entry
            })
            .collect();
        std::fs::write(scores.join("high09"), &list).unwrap();
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let today = || {
            let d = chrono::Utc::now().date_naive();
            [d.day() as u8, d.month() as u8, d.year() as u8, (d.year() >> 8) as u8]
        };
        let entered = |runner: &Runner| {
            let state = runner.host.state();
            let (_, list) = state.settings_files.iter().find(|(f, _)| *f == "Highscore/high09")?;
            Some(list.clone())
        };
        let ask = "now click bar 17\nuntil dialog 302\nnow choose 4035 5\nuntil dialog 384";
        let Some(runner) = on_save(&game, "Karriereende", ask) else { return };
        let question = &game.dialogs[&384];
        assert_eq!(runner.host.view().text(control(question, 4738).unwrap()), game.text(2691));
        // No: the dressing room stands, his career whole.
        let no = format!("{ask}\nnow press 4739\nuntil dialog 302");
        let runner = on_save(&game, "Karriereende", &no).unwrap();
        assert_eq!(runner.host.view().id, 302);
        assert!(runner.host.state().leaving.is_empty());
        // Yes: Anna, without a club, plays no turn, so the turns are over
        // and the highscores show him at once (event 0xb0), as on the
        // original — from the list kept for the end.
        let before = today();
        let yes = format!("{ask}\nnow press 4741\nuntil dialog 95");
        let runner = on_save(&game, "Karriereende", &yes).unwrap();
        let rows = &runner.host.view().list(control(&game.dialogs[&95], 2010).unwrap()).unwrap().rows;
        assert!(rows[1].starts_with("2.\tBert Beta\t55\t40\t40\t2\t0\t"), "{rows:?}");
        let list = entered(&runner).expect("the career list kept for the end");
        let points: Vec<i32> = list
            .chunks(0x30)
            .map(|e| i32::from_le_bytes(e[0x20..0x24].try_into().unwrap()))
            .collect();
        assert_eq!(points, [100, 40, 20, 20, 10]);
        let bert = &list[0x30..0x60];
        assert_eq!(&bert[..5], b"Bert\0");
        assert_eq!(&bert[0xa..0xf], b"Beta\0");
        // 55 years old, a year a trainer, 40 points, level 2 twice, no
        // titles, today.
        assert_eq!(&bert[0x1a..0x2a], [55, 0, 1, 0, 0, 0, 40, 0, 0, 0, 2, 2, 0, 0, 0, 0]);
        assert!([before, today()].contains(&bert[0x2a..0x2e].try_into().unwrap()));
        // Its OK, and the week played: Anna alone is left, in the slots,
        // the order and the count — and with no one else to play, the next
        // week's turn is hers, in the dressing room (`FUN_0064e40f`).
        let week = format!("{yes}\nnow ok\non 2 wait\non 381 wait\non 25 ok\nuntil dialog 302");
        let runner = on_save(&game, "Karriereende", &week).unwrap();
        let screens: Vec<&str> =
            runner.log.iter().filter_map(|l| l.strip_prefix("screen ")).collect();
        let shown = screens.iter().position(|&s| s == "95").unwrap();
        assert_eq!(screens[shown + 1], "2", "the week after the highscores");
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let season = state.season.as_ref().unwrap();
        let names: Vec<_> = season.managers.iter().map(|m| m.first_name.as_str()).collect();
        assert_eq!(names, ["Anna"]);
        assert_eq!(records.globals.humans(), 1);
        assert_eq!(records.globals.slots(), [0, 1, 0, 0]);
        assert_eq!(records.globals.order(), [2, -1, -1, -1]);
        // The national team let him go too (`FUN_006b9872`).
        assert_eq!(records.managers[0].national_team(), 0);
        // His club noted the departure for the dismissal dialog
        // (`FUN_005dfec7`): number 1, kind 3, reason 4, to be shown.
        let due = season.notices.clone().due(i16::MAX);
        let noted: Vec<_> = due.iter().map(|n| (n.manager, n.kind, n.reason, n.shown)).collect();
        assert_eq!(noted, [(1, 3, 4, 0x80)]);
        assert_eq!(season.day % 7, 0, "a week's first day");
        drop(state);
        let caption = control(&game.dialogs[&302], 9920).unwrap();
        assert!(runner.host.view().text(caption).contains("Anna Alpha"));
        // "Spiel beenden" enters him at once, as the program ends — the
        // original's extro follows, not the highscores.
        let quit = "now click bar 17\nuntil dialog 302\nnow choose 4035 4\nuntil dialog 384\n\
                    now press 4741\nuntil end";
        let runner = on_save(&game, "Karriereende", quit).unwrap();
        assert_eq!(entered(&runner).as_ref().map(|l| &l[0x30..0x35]), Some(&b"Bert\0"[..]));
        assert_eq!(seen(&runner, 95), 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The individual training's "Spieler-karriere" (dialog 403,
    /// `CDiSpielerkarriere`), as the original shows it on A: titled with
    /// the player-manager and his club, the sliders at his 4, 4 and 1, both
    /// lists opening on "Bitte auswählen" and then the squad in its order
    /// but for himself — the friends' only with players who have no best
    /// friend or have him. OK keeps the sliders, the rival and a friend;
    /// the first row clears the rival (−1); "Abbruch" keeps nothing.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_player_manager_sets_his_own_career() {
        let game = game();
        let dialog = &game.dialogs[&403];
        let open = "now click bar 5\nuntil dialog 347\nnow press 6151\nuntil dialog 403";
        let Some(runner) = on_save(&game, "A", open) else { return };
        let view = runner.host.view();
        assert_eq!(
            view.text(control(dialog, 9929).unwrap()),
            "ANSTOSS 3 - Spielerkarriere Toni Lubiniecki (1.FC Union Berlin)"
        );
        let sliders = [6133, 6134, 6135].map(|id| {
            let value = view.value(control(dialog, id).unwrap()).unwrap();
            (value.value, value.range)
        });
        assert_eq!(sliders, [(4, (0, 4)), (4, (0, 4)), (1, (0, 2))]);
        let rival = view.combo(control(dialog, 6137).unwrap()).unwrap();
        let friend = view.combo(control(dialog, 6138).unwrap()).unwrap();
        assert_eq!((rival.selected, friend.selected), (Some(0), Some(0)));
        assert_eq!(rival.items[..5], [game.text(4945), "Rönnow, F.", "Raab, M.", "Klaus, C.", "Doekhi, D."]);
        assert!(!rival.items.iter().any(|i| i.starts_with("Lubiniecki")));
        {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let season = state.season.as_ref().unwrap();
            let club = &season.clubs[&season.managers[0].club.unwrap()];
            let free = club
                .status
                .iter()
                .filter(|s| s.id < 17001)
                .filter(|s| {
                    let best = records.player(s.id).unwrap().best_friend();
                    (best as i16) < 1 || best == 17001
                })
                .count();
            assert_eq!(friend.items.len(), free + 1);
        }
        // The button stands crossed out no more.
        let training = on_save(&game, "A", "now click bar 5\nuntil dialog 347").unwrap();
        let career = control(&game.dialogs[&347], 6151).unwrap();
        assert!(!training.host.view().inert.contains(&Target::Button(career)));
        // OK with the sliders moved and a rival and a friend chosen.
        let kept = format!("{open}\nnow slide 6133 2; slide 6135 0; row 6137 3; row 6138 2; ok\nuntil dialog 347");
        let runner = on_save(&game, "A", &kept).unwrap();
        {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let manager = &records.managers[0];
            assert_eq!(manager.own_play(), [2, 4, 0]);
            let last = |id: i16| {
                let player = records.player(id as u16).unwrap();
                records.name(player.last_name()).to_owned()
            };
            assert_eq!(last(manager.rival()), "Klaus");
            assert_eq!(last(manager.friend()), "Raab");
        }
        // The first row clears the rival; the friend stays as it was.
        let cleared = format!("{open}\nnow row 6137 0; row 6138 0; ok\nuntil dialog 347");
        let runner = on_save(&game, "A", &cleared).unwrap();
        let state = runner.host.state();
        let manager = &state.records.as_ref().unwrap().managers[0];
        assert_eq!((manager.rival(), manager.friend()), (-1, 0));
        drop(state);
        // "Abbruch" keeps nothing.
        let dropped = format!("{open}\nnow slide 6133 0; row 6137 3; cancel\nuntil dialog 347");
        let runner = on_save(&game, "A", &dropped).unwrap();
        let state = runner.host.state();
        let manager = &state.records.as_ref().unwrap().managers[0];
        assert_eq!((manager.own_play(), manager.rival()), ([4, 4, 1], 0));
    }

    /// The honours page's "Vertrauen Vorstand" (dialog 320,
    /// `CVertrauenVorstand`), as the original draws it: on A the confidence
    /// of the last periods from the second on, 85 with an 88, then today's
    /// 82 — a red curve from (218,172) to (344,176) over the 50 % line, each
    /// with its last pixel — and text 2472; on Probe1 a flat 82 from the
    /// sixth period. OK leaves the page's button unpressed.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_board_confidence_shows_its_curve() {
        let game = game();
        let dialog = &game.dialogs[&320];
        let open = "now click bar 16\nuntil dialog 175\nnow choose 5571 0\nuntil dialog 320";
        let Some(runner) = on_save(&game, "A", open) else { return };
        let view = runner.host.view();
        assert_eq!(view.text(control(dialog, 4157).unwrap()), game.text(2472));
        let red = [255, 0, 0];
        let ends: Vec<[i32; 4]> = view.lines.iter().map(|&(l, ink)| {
            assert_eq!(ink, red);
            l
        }).collect();
        assert_eq!(ends[..2], [[213, 222, 366, 222], [366, 222, 366, 222]]);
        let points: Vec<(i32, i32)> = ends[2..].iter().map(|l| (l[2], l[3])).collect();
        assert_eq!(
            points,
            [(218, 172), (232, 172), (246, 172), (260, 172), (274, 172), (288, 172), (302, 172),
             (316, 168), (330, 172), (344, 176), (344, 176)]
        );
        let runner = on_save(&game, "Probe1", open).unwrap();
        let view = runner.host.view();
        let points: Vec<(i32, i32)> = view.lines[2..].iter().map(|&(l, _)| (l[2], l[3])).collect();
        assert_eq!(points, [(218, 176), (232, 176), (246, 176), (260, 176), (274, 176), (288, 176), (288, 176)]);
        let back = on_save(&game, "A", &format!("{open}\nnow ok\nuntil dialog 175")).unwrap();
        let button = control(&game.dialogs[&175], 5571).unwrap();
        assert_eq!(back.host.view().chosen(button), crate::controls::options::NONE);
    }

    /// The share market (dialog 431, `CDiAktienmarkt`) behind the honours
    /// page's "A", as the original trades on A: the nine clubs' shares by
    /// name, AFC Sunderland chosen and six bought for 907 € and a fee of
    /// 5, three sold back for 453 € less 3 — the offer down and up again,
    /// the holding kept in the last place at the price paid —; back on the
    /// honours page the holding in its list, a click voting to sack its
    /// club's manager.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_share_market_buys_and_sells() {
        let game = game();
        let dialog = &game.dialogs[&431];
        let text = |runner: &Runner, id: u16| runner.host.view().text(control(dialog, id).unwrap()).to_owned();
        let open = "now click bar 16\nuntil dialog 175\nnow choose 5572 0\nuntil dialog 431";
        let Some(runner) = on_save(&game, "A", open) else { return };
        assert_eq!(text(&runner, 9947), "ANSTOSS 3 - Aktienmarkt Toni Lubiniecki");
        assert_eq!(
            [5282, 5139, 5286, 5326, 5334, 5327].map(|id| text(&runner, id)),
            ["99.300 €", "6958.92", "0.50%", "---", " 0 €", " 0 €"]
        );
        let shares = runner.host.view().list(control(dialog, 5278).unwrap()).unwrap().clone();
        assert_eq!(shares.rows.len(), 9);
        assert_eq!(shares.rows[0], "AFC Sunderland\t151,31\t168,81\t-10.37%\t2.150\t168,81\t138,88");
        assert_eq!(shares.inks[0], Some([255, 0, 0]));
        assert!(shares.rows[8].starts_with("Tottenham Hotspur\t"));
        assert!(runner.host.view().controls[control(dialog, 5280).unwrap()].hidden);
        // Six of the first.
        let six = format!("{open}\nnow row 5278 0\n{}", "now spin 5280 up\n".repeat(5));
        let runner = on_save(&game, "A", &six).unwrap();
        assert_eq!([5294, 5295, 5281, 5287].map(|id| text(&runner, id)), ["169,81", "138,08", "6", " 907 €"]);
        let bought = format!("{six}now choose 5654 1");
        let runner = on_save(&game, "A", &bought).unwrap();
        assert_eq!([5282, 5326, 5327].map(|id| text(&runner, id)), ["98.388 €", "0.00", " 907 €"]);
        let held = runner.host.view().list(control(dialog, 5276).unwrap()).unwrap().rows.clone();
        assert_eq!(held, ["AFC Sunderland\t151,31\t151,31\t0.00%\t6\t907,91"]);
        {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let holdings = a3_core::market::holdings(records, 0);
            assert!(holdings[..49].iter().all(Option::is_none), "the last place first");
            let h = holdings[49].unwrap();
            assert_eq!((h.count, h.sack), (6, false));
            let market = a3_core::market::market(records).unwrap();
            assert_eq!(market.shares[h.share].offer, 2144);
        }
        // Three sold back.
        let sold = format!("{bought}\nnow row 5276 0\nnow spin 5284 up\nnow spin 5284 up\nnow choose 5655 1");
        let runner = on_save(&game, "A", &sold).unwrap();
        assert_eq!([5282, 5327].map(|id| text(&runner, id)), ["98.838 €", " 453 €"]);
        let row = &runner.host.view().list(control(dialog, 5278).unwrap()).unwrap().rows[0];
        assert!(row.contains("\t2.147\t"), "{row}");
        // Back on the honours page: the holding, and a click on it.
        let back = format!("{bought}\nnow ok\nuntil dialog 175");
        let runner = on_save(&game, "A", &back).unwrap();
        let list = control(&game.dialogs[&175], 5261).unwrap();
        let rows = runner.host.view().list(list).unwrap().rows.clone();
        assert_eq!(rows, ["AFC Sunderland\t6\t0.0\t151,31\t151,31\t907,91\t"]);
        let runner = on_save(&game, "A", &format!("{back}\nnow row 5261 0")).unwrap();
        let rows = runner.host.view().list(list).unwrap().rows.clone();
        assert_eq!(rows, [format!("AFC Sunderland\t6\t0.0\t151,31\t151,31\t907,91\t{}", game.text(3163))]);
    }

    /// The market's big chart (432) and the week's winners and losers
    /// (433), as the original shows them on A: AFC Sunderland's year in 52
    /// bars on a scale from 137,88 to 169,81, its figures above, the
    /// offer without points; from a holding the same with the black line
    /// at the price paid. The winners: AS Rom first at 4.94 %; the losers:
    /// SpVgg Unterhaching first at −22.65 %, the prices rounded with a dot.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_share_market_shows_its_chart_and_the_weeks_winners() {
        let game = game();
        let text = |runner: &Runner, dialog: u16, id: u16| {
            runner.host.view().text(control(&game.dialogs[&dialog], id).unwrap()).to_owned()
        };
        let open = "now click bar 16\nuntil dialog 175\nnow choose 5572 0\nuntil dialog 431\nnow row 5278 0";
        let chart = format!("{open}\nnow choose 5654 0\nuntil dialog 432");
        let Some(runner) = on_save(&game, "A", &chart) else { return };
        assert_eq!(
            [9947, 1866, 5301, 5300, 5304, 5303, 5302, 5305, 5328].map(|id| text(&runner, 432, id)),
            ["ANSTOSS 3 - Chart", "AFC Sunderland", "151,31", "168,81", "2150", "168,81", "138,88", "169,81", "137,88"]
        );
        let paint = control(&game.dialogs[&432], 5299).unwrap();
        let shown = runner.host.view().paint(paint).unwrap().clone();
        assert_eq!(shown.bars.len(), 52);
        assert!(shown.lines.iter().all(|l| l.1 != image::Rgba([0, 0, 0, 255])), "no line at a price paid");
        let held = format!("{open}\nnow choose 5654 1\nnow row 5276 0\nnow choose 5655 0\nuntil dialog 432");
        let runner = on_save(&game, "A", &held).unwrap();
        let shown = runner.host.view().paint(paint).unwrap().clone();
        assert!(shown.lines.iter().any(|l| l.1 == image::Rgba([0, 0, 0, 255])), "the price paid");
        let winners = format!("{open}\nnow press 5292\nuntil dialog 433");
        let runner = on_save(&game, "A", &winners).unwrap();
        let rows = |id| runner.host.view().list(control(&game.dialogs[&433], id).unwrap()).unwrap().rows.clone();
        let (up, down) = (rows(5308), rows(5307));
        assert_eq!((up.len(), down.len()), (8, 8));
        assert_eq!(up[0], "AS Rom\t271.67\t258.88\t4.94%\t2.400\t12.79");
        assert_eq!(down[0], "SpVgg Unterhaching\t64.07\t82.83\t-22.65%\t1.620\t-18.76");
        assert!(down[2].starts_with("AFC Sunderland\t151.32\t"), "{}", down[2]);
    }

    /// The betting office's matchday bets (dialog 411) behind the honours
    /// page's "W", as the original takes them on A: the German matchday's
    /// eleven matches; the full tip twice with match one's three outcomes
    /// (six euros), a highest win on one match only at 30 € each; Harry
    /// Kane first in the scorers' list at 3.1; England's page with a German
    /// match in its eleventh row; everything kept to the next opening.
    /// Probe1, in its fifth week, finds the office closed (text 4592).
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_betting_office_takes_matchday_bets() {
        let game = game();
        let dialog = &game.dialogs[&411];
        let text = |runner: &Runner, id: u16| runner.host.view().text(control(dialog, id).unwrap()).to_owned();
        let open = "now click bar 16\nuntil dialog 175\nnow choose 5572 1\nuntil dialog 411";
        let Some(runner) = on_save(&game, "A", open) else { return };
        assert_eq!(text(&runner, 9947), "ANSTOSS 3 - Wettbüro Spieltag Toni Lubiniecki");
        assert_eq!(
            [5282, 5233, 5172, 5249, 5182].map(|id| text(&runner, id)),
            ["99.300 €", "0 x", "Eintracht Frankfurt - 1.FC Heidenheim", "7", "SV Darmstadt 98 - Karlsruher SC"]
        );
        let scorers = runner.host.view().combo(control(dialog, 5187).unwrap()).unwrap().items.clone();
        assert_eq!(scorers[..3], [game.text(4488), "Harry Kane\t(3.1)", "Serhou Guirassy\t(10)"]);
        let ticks = [5189, 5190, 5232, 5191, 5192, 5193, 5194, 5195, 5196, 5197, 5198, 5199, 5210]
            .map(|id| format!("now click {id}"))
            .join("\n");
        let tips = format!(
            "{open}\nnow spin 5170 up\n{ticks}\nnow spin 5170 up\nnow click 5223\n\
             now spin 5171 up\nnow spin 5171 up\nnow spin 5171 up\nnow click 5224\nnow row 5187 1"
        );
        let runner = on_save(&game, "A", &tips).unwrap();
        assert_eq!([5233, 5168, 5235, 5169].map(|id| text(&runner, id)), ["2 x", " 6 €", " 30 €", " 30 €"]);
        {
            let state = runner.host.state();
            let bet = a3_core::betting::matchday_bet(state.records.as_ref().unwrap(), 0, 0);
            assert_eq!((bet.full, bet.ticks[0], bet.ticks[2], bet.ticks[3], bet.ticks[4]), (2, 7, 4, 12, 4));
            assert_eq!((bet.high_stake, bet.scorer), (30, 0));
        }
        let england = format!("{open}\nnow click 5152");
        let runner = on_save(&game, "A", &england).unwrap();
        assert_eq!(text(&runner, 5172), "AFC Bournemouth - Manchester City");
        assert_eq!(text(&runner, 5182), "Eintracht Frankfurt - 1.FC Heidenheim");
        let again = format!("{tips}\nnow ok\nuntil dialog 175\nnow choose 5572 1\nuntil dialog 411");
        let runner = on_save(&game, "A", &again).unwrap();
        assert_eq!(text(&runner, 5233), "2 x");
        let closed = "now click bar 16\nuntil dialog 175\nnow choose 5572 1\nuntil dialog 380";
        let runner = on_save(&game, "Probe1", closed).unwrap();
        let notice = control(&game.dialogs[&380], 4730).unwrap();
        assert_eq!(runner.host.view().text(notice), game.text(4592));
    }

    /// The betting office's season bets (dialog 410), in a game's first
    /// eight half weeks — here A with its counter set to 5, as the
    /// original was poked —: the first league's table with the
    /// champions' odds by place ("FC Bayern München 3.0"), the cups'
    /// names as the new game set them, the European championship left out
    /// in 2025 and the world's shown; the trainer chosen takes the odds of
    /// the row below his, as the game does; the stakes stop where the
    /// season's leave no room: with 150 € and four bets of 10 €, the
    /// champion's at 100 €.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_betting_office_takes_season_bets() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "A", "season-bets", |records| records.globals.set_half_weeks(5));
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let dialog = &game.dialogs[&410];
        let text = |runner: &Runner, id: u16| runner.host.view().text(control(dialog, id).unwrap()).to_owned();
        let items = |runner: &Runner, id: u16| runner.host.view().combo(control(dialog, id).unwrap()).unwrap().items.clone();
        let open = "now click bar 16\nuntil dialog 175\nnow choose 5572 1\nuntil dialog 410";
        let Some(runner) = on_save(&game, "season-bets", open) else { return };
        assert_eq!(text(&runner, 9947), "ANSTOSS 3 - Wettbüro Saison Toni Lubiniecki");
        assert_eq!([6284, 6285, 5282].map(|id| text(&runner, id)), ["Champions League", "Europa League", "99.300 €"]);
        assert_eq!(items(&runner, 5149)[..3], [game.text(4945), "FC Bayern München\t3.0", "Bayer 04 Leverkusen\t5.4"]);
        assert_eq!(items(&runner, 5167)[..3], [game.text(4945), " 1\t2.1", " 2\t1.5"]);
        assert_eq!(items(&runner, 5246)[1], "Brasilien\t1.7");
        let hidden = |runner: &Runner, id: u16| runner.host.view().controls[control(dialog, id).unwrap()].hidden;
        assert!(hidden(&runner, 5243) && hidden(&runner, 5242) && !hidden(&runner, 5246));
        let chosen = format!(
            "{open}\nnow spin 5148 up\nnow spin 5148 up\nnow spin 5148 up\nnow row 5149 1\nnow row 5164 1\n\
             now row 5167 1\nnow row 5246 1"
        );
        let runner = on_save(&game, "season-bets", &chosen).unwrap();
        assert_eq!(text(&runner, 5150), " 30 €");
        {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let bets = a3_core::betting::season_bets(records, 0);
            let bayern = records.teams.iter().find(|t| t.name() == "FC Bayern München").unwrap().number();
            assert_eq!(bets.pages[0].champion, bayern as i16);
            assert_eq!(bets.pages[0].stakes[0], 30);
            let offer = a3_core::betting::season_offer(records, 0).unwrap();
            assert_eq!((bets.pages[0].trainer, bets.pages[0].trainer_odds), (offer.trainers[0].id, offer.trainers[1].odds));
            let manager = &records.managers[0];
            assert_eq!((bets.place, bets.place_club), (1, (manager.team() as i16, manager.country() as i16)));
            assert_eq!(bets.cups[3], 0);
        }
        let root = patched(&dir, "A", "season-cap", |records| {
            records.globals.set_half_weeks(5);
            records.managers[0].set_cash(150);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let capped = format!(
            "{open}\nnow spin 5165 up\nnow spin 5237 up\nnow spin 5239 up\nnow spin 5245 up\n{}",
            "now spin 5148 up\n".repeat(12)
        );
        let runner = on_save(&game, "season-cap", &capped).unwrap();
        assert_eq!(text(&runner, 5150), " 100 €");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The help (dialog 423) behind the honours page's question mark, as
    /// the original shows it on A: the window's topic, "Persönliche
    /// Erfolge", fifteen steps of the bar long; thirteen steps down the
    /// button before "Siehe auch" leads to "Rechtsformen der Vereine",
    /// the black text beside it nowhere; "Historie" returns to where the
    /// bar stood; the contents and the index hold no history.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_help_shows_a_windows_topic_and_follows_its_links() {
        let game = game();
        let dialog = &game.dialogs[&423];
        let title = |runner: &Runner| runner.host.view().text(control(dialog, 6476).unwrap()).to_owned();
        let bar = |runner: &Runner| {
            let value = runner.host.view().value(control(dialog, 5142).unwrap()).copied().unwrap();
            (value.value, value.range)
        };
        let open = "now click bar 16\nuntil dialog 175\nnow click at 623 11\nuntil dialog 423";
        let Some(runner) = on_save(&game, "A", open) else { return };
        assert_eq!(runner.host.view().text(control(dialog, 9947).unwrap()), "ANSTOSS 3 - Hilfe Version 1.40/1.30");
        assert_eq!((title(&runner).as_str(), bar(&runner)), ("Persönliche Erfolge", (0, (0, 15))));
        let down = format!("{open}\n{}", "now click at 469 454\n".repeat(13));
        let runner = on_save(&game, "A", &format!("{down}now click at 100 349")).unwrap();
        assert_eq!((title(&runner).as_str(), bar(&runner).0), ("Persönliche Erfolge", 13));
        let button = format!("{down}now click at 29 349");
        let runner = on_save(&game, "A", &button).unwrap();
        assert_eq!((title(&runner).as_str(), bar(&runner)), ("Rechtsformen der Vereine", (0, (0, 0))));
        let runner = on_save(&game, "A", &format!("{button}\nnow choose 6164 0")).unwrap();
        assert_eq!((title(&runner).as_str(), bar(&runner).0), ("Persönliche Erfolge", 13));
        let pages = format!("{button}\nnow press 5143\nnow press 5139\nnow choose 6164 0");
        let runner = on_save(&game, "A", &pages).unwrap();
        assert_eq!(title(&runner), "Persönliche Erfolge");
        let runner = on_save(&game, "A", &format!("{open}\nnow press 5139")).unwrap();
        assert_eq!((title(&runner).as_str(), bar(&runner).1), ("Index", (0, 126)));
    }

    /// A loaded manager keeps the national team the save names: his
    /// record's +0x1f72 (the manager object's +0x17e6, whose name the
    /// honours page writes with text 5566, `FUN_006a426b`) turns into the
    /// season's `national_team`; a save naming none leaves him without
    /// one. No kept save names a team, so the test pokes Germany (14)
    /// into a copy of Toni.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_loaded_manager_keeps_his_national_team() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let game = game();
        let Some(runner) = on_save(&game, "Toni", "") else { return };
        assert_eq!(
            runner.host.state().season.as_ref().unwrap().managers[0].national_team,
            None,
            "Toni names no national team"
        );
        let root = patched(&dir, "Toni", "national", |records| {
            let slot = records.human_slot(0).unwrap();
            records.managers[slot].put(0x1f72, 14i16);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let runner = on_save(&game, "national", "").unwrap();
        let state = runner.host.state();
        let season = state.season.as_ref().unwrap();
        assert_eq!(season.managers[0].national_team, Some(14), "the season's national team");
        assert!(season.managers[0].club.is_some(), "he still stands at his club");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The season's end: A set one day before its top league's last
    /// matchday. Past the last matchday every club settles its season
    /// (`FUN_006219d2`) — Union's result taxed at the save's 29 %, the rest
    /// its newest past result — and the game brings its managers with a
    /// club the yearly statement — the event 0x12d (`FUN_006742e0`, opened
    /// by `FUN_004a141c`): dialog 417 without the bar, with its OK instead,
    /// hiding the Spenden and black cash box buttons, keeping the
    /// Ermittlung combo on "Aktuell", and below the rows the tax booked,
    /// in words and figures.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_season_end_statements_the_tax() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "A", "season-statement", |records| {
            records.globals.set_day(320);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        // The week's finances, the whistle, the week run out past the
        // last matchday, and the statement comes before the new week.
        let then = "on 384 press 4741\non 2 wait\non 381 wait\n\
                    now click bar 9\nuntil dialog 417\nnow click bar 18\n\
                    until dialog 417";
        let Some(runner) = on_save(&game, "season-statement", then) else { return };
        let log = runner.log.join("\n");
        assert_eq!(runner.host.view().id, 417, "{log}");
        let dialog = &game.dialogs[&417];
        let view = runner.host.view();
        // The statement mode: OK up, bar and picture buttons off, the
        // finding combo disabled, and the tax told below the rows.
        let shown = |id: u16| !view.controls[control(dialog, id).unwrap()].hidden;
        assert!(shown(1), "the statement keeps its OK");
        assert!(!shown(9930), "the statement comes without the bar");
        assert!(!shown(2222), "the statement hides the Spenden button");
        assert!(!shown(2224), "the statement hides the black cash box button");
        assert!(view.combo(control(dialog, 2226).unwrap()).unwrap().disabled, "{log}");
        // The settlement's tax: 29 % of the result before it, which is
        // what is left of it plus the tax.
        let (tax, left, rate) = {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let team = &records.teams[usize::from(records.managers[0].team())];
            (a3_core::finances::tax_paid(team), team.get::<i64>(0x908), records.globals.get::<u8>(0xb5dfa9))
        };
        assert_eq!(rate, 29);
        assert!(tax > 0, "Union's season is taxed");
        assert_eq!(i64::from(tax), (f64::from(rate) * (left + i64::from(tax)) as f64 / 100.0) as i64);
        let line = view.text(control(dialog, 1621).unwrap());
        let amount = crate::screens::finances::money32(&game, tax);
        assert!(line.contains(&amount), "the tax line reads {line}, not {amount}");
        // The statement has shown itself once in this game.
        assert!(runner.host.state().season.as_ref().unwrap().statement, "{log}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// OK past the statement opens the new season: the game walks on from
    /// day 0 of the year that follows, the statement stands free to come
    /// again at the next season's end, and the top league of the country
    /// played stands with no matchday played. The statement itself the
    /// test above shows.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_statement_opens_the_new_season() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "A", "season-open", |records| {
            records.globals.set_day(320);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        // As in the tax test up to the statement, then OK.
        let then = "on 384 press 4741\non 2 wait\non 381 wait\non 356 press 4542\n\
                    now click bar 9\nuntil dialog 417\nnow click bar 18\n\
                    until dialog 417\nnow press 1\nuntil dialog 356\nnow press 4542";
        let Some(mut runner) = on_save(&game, "season-open", then) else { return };
        // The cycle opens the new season behind the press: wait for its
        // next screen before reading the state.
        let _ = runner.settle();
        let log = runner.log.join("\n");
        let state = runner.host.state();
        let (season, records) = (state.season.as_ref().unwrap(), state.records.as_ref().unwrap());
        assert_eq!(season.year, 2026, "{log}");
        assert!(!season.statement, "the statement stands free again");
        assert!(season.day < 7, "the game walks on from day {}", season.day);
        assert_eq!(records.globals.start_year(), 2026);
        let day = usize::try_from(records.globals.day()).ok();
        assert_eq!(day, Some(season.day), "the day counted from the new season's 0");
        let nation = u16::try_from(records.globals.main_country()).unwrap();
        let top = a3_core::opponent::League::of(records, nation, 0).unwrap();
        assert_eq!(top.played(), 0, "the league opens with none played");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The first player of the first club abroad the game keeps without
    /// its league (a country of the second kind), by id.
    fn kept_player(records: &a3_core::records::Records) -> Option<u16> {
        let country = (1..0x75usize).find(|&c| records.countries.kind(c) == 2 && records.countries.team_count(c) > 0)?;
        records.team(country as u16, 0)?.players().first().copied()
    }

    /// Past the season's statement a kept country's player has closed his
    /// season and opened the next: his season's goals and cards at
    /// nought, his form of the season one week of 10, his contract in its
    /// last year run on by three or four, the count of his seasons up by
    /// two (the close and the development) and his strength at most a
    /// point from where it stood. The closed season stays in the
    /// all-time table of the first league and is the archive's newest; the
    /// leagues have taken their promoted and relegated clubs, and the cup
    /// its first round.
    /// The game plays on season after season: from A's turn (its day set
    /// to 320) the replica plays five more seasons to the end, each turn's
    /// steps on its own state, and stands in the sixth with its counters
    /// rolled — the year on, the half weeks counted, the league open, the
    /// European cups entered, every played club's squad within its bounds,
    /// the squads of both kinds of countries as full as they were, and the
    /// human club's squad in the season as in its records.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_game_plays_on_season_after_season() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "A", "seasons-on", |records| records.globals.set_day(320));
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        // The staff's standing runs down at each season's end, so from one
        // season to the next the club's training shifts and the week's
        // book of gained and lost abilities can stand at the turn.
        let handlers = "on 422 ok\non 420 ok\non 421 ok\non 287 ok\non 321 ok\non 25 ok\non 341 ok\n\
                        on 364 ok\n\
                        on 120 ok\non 51 ok\non 392 ok\non 435 ok\non 96 ok\non 161 ok\non 419 ok\n\
                        on 477 ok\non 380 press 4731\non 147 click bar 18\non 302 click bar 18\n\
                        on 308 choose 4095 0\non 332 ok\non 269 ok\non 93 press 1943\non 499 ok\non 41 ok\non 383 click at 460 210\non 99 press 2\n\
                        on 307 click at 320 240\non 310 click at 320 240\non 331 press 4303\n\
                        on 65 click at 165 330\non 423 ok\non 1 click at 320 357\n\
                        on 375 click at 617 182; press 4715\non 461 ok\non 384 press 4741\non 2 wait\n\
                        on 381 wait\non 356 press 4542\non 22 press 1306\non 152 press 2594\n\
                        on 379 ok\non 314 ok\n";
        let turns = "until dialog 417\nnow press 1\n".repeat(5);
        let then = format!(
            "{handlers}now click bar 9\nuntil dialog 417\nnow click bar 18\nuntil dialog 417\n\
             now press 1\n{turns}until dialog 301\nnow ok\nuntil day 30"
        );
        let Some(mut runner) = on_save(&game, "seasons-on", &then) else { return };
        let _ = runner.settle();
        let log = runner.log.join("\n");
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let season = state.season.as_ref().unwrap();
        assert_eq!(records.globals.seasons(), 7, "{log}");
        assert_eq!(records.globals.start_year(), 2031);
        assert_eq!(season.day, 30);
        assert_eq!(records.globals.half_weeks(), 8);
        let league = a3_core::opponent::League::of(records, 14, 0).unwrap();
        assert_eq!(league.teams(), 18);
        let entrants = records.block("0x8e7c88").unwrap();
        let count = |at: usize| i16::from_le_bytes([entrants[at], entrants[at + 1]]);
        assert_eq!((count(0), count(2), count(6)), (16, 32, 82), "the European cups entered");
        for c in [14u16, 16, 20, 28, 63] {
            for t in 0..records.countries.team_count(usize::from(c)).max(0) as u16 {
                let n = records.team(c, t).unwrap().players().len();
                assert!(n <= 40, "club {c}/{t} with {n} players");
            }
        }
        // The squads hold their size: the thin ones take new players.
        let mut sizes = [(0usize, 0usize); 3];
        for c in 1..0x75u16 {
            let kind = usize::from(records.countries.kind(usize::from(c)) as u8).min(2);
            for t in 0..records.countries.team_count(usize::from(c)).max(0) as u16 {
                if let Some(team) = records.team(c, t) {
                    sizes[kind].0 += 1;
                    sizes[kind].1 += team.players().len();
                }
            }
        }
        let average = |(n, sum): (usize, usize)| sum as f64 / n.max(1) as f64;
        assert!(average(sizes[1]) > 20.0 && average(sizes[2]) > 15.0, "squads of {sizes:?}");
        // Each human club stands as its records have it after the turns.
        for (&(slot, index), club) in &season.clubs {
            let nation = game.data.world.country(slot).unwrap().nation;
            let team = records.team(nation, index as u16).unwrap();
            let names: Vec<String> =
                team.players().iter().map(|&id| records.name(records.player(id).unwrap().last_name()).to_owned()).collect();
            let state: Vec<String> = club.players.iter().map(|p| p.last_name.clone()).collect();
            assert_eq!(state, names, "club {slot}/{index}'s squad");
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_new_season_rolls_the_kept_countries_players_over() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let mut before = None;
        let (mut eternal, mut running, mut closed) = (Vec::new(), 0, Vec::new());
        let mut moved: (Vec<i16>, Vec<i16>) = (Vec::new(), Vec::new());
        let root = patched(&dir, "A", "season-kept", |records| {
            records.globals.set_day(320);
            let nation = u16::try_from(records.globals.main_country()).unwrap();
            eternal = a3_core::statistics::eternal(records, nation);
            let league = a3_core::opponent::League::of(records, nation, 0).unwrap();
            let table = a3_core::statistics::table(records, &league, a3_core::statistics::ALL);
            running = table.iter().map(|row| row.games).sum::<i32>();
            closed = table
                .iter()
                .map(|r| (r.team as u8, r.games as i16, r.points as i16, r.goals_for as i16, r.goals_against as i16))
                .collect();
            closed.sort();
            let level1 = a3_core::opponent::League::of(records, nation, 1).unwrap();
            let second = a3_core::statistics::table(records, &level1, a3_core::statistics::ALL);
            moved = (
                table.iter().rev().take(3).map(|r| r.team).collect(),
                second.iter().take(3).map(|r| r.team).collect(),
            );
            let id = kept_player(records).expect("a kept country's player");
            let year = records.globals.start_year();
            let p = records.player_mut(id).unwrap();
            p.set_goals([5, 1, 2]);
            p.set_league_cards([3, 1, 0]);
            p.set_season_form(130);
            p.set_season_form_weeks(12);
            p.set_contract(year + 1);
            p.put::<u8>(0x161, 2);
            before = Some((id, p.strength(), year));
        });
        let (id, strength, year) = before.unwrap();
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let then = "on 384 press 4741\non 2 wait\non 381 wait\n\
                    now click bar 9\nuntil dialog 417\nnow click bar 18\n\
                    until dialog 417\nnow press 1\nuntil dialog 301\nnow ok";
        let Some(mut runner) = on_save(&game, "season-kept", then) else { return };
        let _ = runner.settle();
        let log = runner.log.join("\n");
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        assert_eq!(records.globals.start_year(), year + 1, "{log}");
        let p = records.player(id).unwrap();
        assert_eq!((p.goals(), p.league_cards()), ([0; 3], [0; 3]), "player {id}");
        assert_eq!((p.season_form(), p.season_form_weeks()), (10, 1));
        let left = i32::from(p.contract() & 0xfff) - i32::from(year + 1);
        assert!((3..=3 + i32::from(strength) / 3).contains(&left), "{left} years left");
        assert_eq!(p.get::<u8>(0x161), 4);
        assert!(p.strength().abs_diff(strength) <= 1, "{} from {strength}", p.strength());
        // The closed season stays in the all-time table, which stands as
        // it stood before the turn (the patched day skips the matchdays
        // between): its season's games now kept, none running.
        let nation = u16::try_from(records.globals.main_country()).unwrap();
        assert_eq!(a3_core::statistics::eternal(records, nation), eternal);
        assert!(running > 0, "the closed season had games to keep");
        // The Historisches' newest season is the one just closed.
        let archived = a3_core::statistics::final_table(records, nation, 0, 9).unwrap();
        let mut rows: Vec<_> =
            archived.rows.iter().map(|r| (r.team, r.games, r.points, r.goals_for, r.goals_against)).collect();
        rows.sort();
        assert_eq!(rows, closed, "the closed season archived");
        // The Bundesliga's bottom three play the 2. Bundesliga now, its
        // top three the Bundesliga, and every club stands at its list's
        // level and place.
        let level = |team: i16| records.team(nation, team as u16).unwrap().level();
        assert!(moved.0.iter().all(|&t| level(t) == 1), "relegated {:?}", moved.0);
        assert!(moved.1.iter().all(|&t| level(t) == 0), "promoted {:?}", moved.1);
        for l in 0..records.countries.leagues(usize::from(nation)) as u8 {
            let league = a3_core::opponent::League::of(records, nation, l).unwrap();
            for place in 0..league.teams() {
                let team = records.team(nation, league.team(place) as u16).unwrap();
                assert_eq!((team.level(), usize::from(team.league_place())), (l, place));
            }
        }
        // The cup's first round stands drawn: 32 ties of the 64 entrants.
        let slot = records.countries.slot(usize::from(nation));
        let cup = records.block(&format!("{:#x}[{slot}]", a3_core::opponent::CUPS_AT)).unwrap();
        let ties = (0..32)
            .filter(|q| {
                let at = a3_core::opponent::PAIRINGS + q * a3_core::opponent::PAIRING;
                cup[at] != 0 && cup[at + 4] != 0
            })
            .count();
        assert_eq!(ties, 32, "the cup's first round drawn");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The merchandise calculation's "Details" opens the club's own range
    /// (56): one article bought at the order size, then every one at the
    /// next size with its rebate, each booked as merchandise bought at once;
    /// OK writes the orders back into the human slot's range, in the weeks
    /// each kind takes.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_merchandise_range_orders_its_articles() {
        use a3_core::merchandise::{Range, delivery_weeks};
        let game = game();
        let open = "now click bar 9\nuntil dialog 417\nnow click at 88 133\nuntil dialog 125\n\
                    now click at 479 439\nuntil dialog 115\nnow press 2204\nuntil dialog 56";
        let Some(before) = on_save(&game, "A", open) else { return };
        let dialog = &game.dialogs[&56];
        let view = before.host.view();
        // Both men hired on A: ticked and locked.
        for id in [1496, 1497] {
            let check = view.check(control(dialog, id).unwrap()).unwrap();
            assert!(check.checked && check.disabled, "{id}");
        }
        let rows = &view.list(control(dialog, 1495).unwrap()).unwrap().rows;
        assert_eq!(rows.len(), 13);
        assert_eq!(rows[0], "Trikots\t40,--\t72,--\t8437\t0\t270\t19440\t2");
        let state = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let manager = records.humans().next().unwrap();
            let team = records.team(manager.country(), manager.team()).unwrap();
            let bought = team.get::<i32>(0x308 + a3_core::finances::CLUB_TO_TEAM);
            (bought, Range::of(records, team).unwrap(), delivery_weeks(records))
        };
        let (bought, range, weeks) = state(&before);
        // Trikots: 500 at 40 € with no rebate, due in two weeks.
        let one = format!("{open}\nnow row 1495 0\nnow press 1505\nnow press 1\nuntil dialog 115");
        let (after, ordered, _) = state(&on_save(&game, "A", &one).unwrap());
        assert_eq!(after - bought, 20_000);
        assert_eq!(weeks[0], 2);
        assert_eq!(ordered.article(0).unwrap().ordered, 500);
        // All at 1.000 pieces, 10% off each.
        let all = format!("{open}\nnow spin 1499 up\nnow press 1504\nnow press 1\nuntil dialog 115");
        let (after, ordered, _) = state(&on_save(&game, "A", &all).unwrap());
        let tables = &game.rules.merchandise;
        let cost: i32 = range
            .articles()
            .iter()
            .map(|a| (f64::from(tables.kinds[a.kind].bought_at) * 1000.0 / 100.0 * 0.9) as i32)
            .sum();
        assert_eq!(after - bought, cost);
        for a in range.articles() {
            assert_eq!(ordered.article(a.place).unwrap().ordered, a.ordered + 1000, "{}", a.place);
        }
    }

    /// A double click on an article of the range opens it (57): its price
    /// stepped up twice and the box ticked, OK keeps the markup and leaves
    /// it out of buying all; given up, after the question, it leaves the
    /// range, and 56's OK writes both back.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn an_article_takes_its_price_or_leaves_the_range() {
        use a3_core::merchandise::Range;
        let game = game();
        let open = "now click bar 9\nuntil dialog 417\nnow click at 88 133\nuntil dialog 125\n\
                    now click at 479 439\nuntil dialog 115\nnow press 2204\nuntil dialog 56\n\
                    now dbl row 1495 6\nuntil dialog 57";
        let Some(runner) = on_save(&game, "A", open) else { return };
        let dialog = &game.dialogs[&57];
        let text = |id| runner.host.view().text(control(dialog, id).unwrap()).to_owned();
        assert_eq!(text(1509), "Schlafanzug");
        let figures = [1511, 1512, 1513, 1514].map(text);
        assert_eq!(figures, ["60,--", "60,--", "25,--", "886"]);
        assert_eq!(text(1519), "Max. Verkauf: 92");
        let range = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let manager = records.humans().next().unwrap();
            Range::of(records, records.team(manager.country(), manager.team()).unwrap()).unwrap()
        };
        let place = range(&runner).articles()[6].place;
        let kept = format!(
            "{open}\nnow spin 1516 up\nnow spin 1516 up\nnow click 1515\nnow ok\nuntil dialog 56\n\
             now press 1\nuntil dialog 115"
        );
        let after = range(&on_save(&game, "A", &kept).unwrap());
        let a = after.article(place).unwrap();
        // 60 € is the kind's price; a euro a step, 61 € first at +2%,
        // 62 € at +4%.
        assert_eq!((a.markup, a.excluded), (4, true));
        let gone = format!(
            "{open}\nnow press 1517\nuntil dialog 384\nnow ok\nuntil dialog 56\n\
             now press 1\nuntil dialog 115"
        );
        let after = range(&on_save(&game, "A", &gone).unwrap());
        assert!(after.article(place).is_none());
        assert_eq!(after.articles().len(), 12);
    }

    /// 56's "Details" tells the range's season (372): takings, costs and
    /// the profit, a loss with its sign, after 500 scarves were bought.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_merchandise_info_tells_the_season() {
        let game = game();
        let then = "now click bar 9\nuntil dialog 417\nnow click at 88 133\nuntil dialog 125\n\
                    now click at 479 439\nuntil dialog 115\nnow press 2204\nuntil dialog 56\n\
                    now row 1495 2\nnow press 1505\nnow press 1506\nuntil dialog 372";
        let Some(runner) = on_save(&game, "A", then) else { return };
        let dialog = &game.dialogs[&372];
        let text = |id| runner.host.view().text(control(dialog, id).unwrap()).to_owned();
        let figures = [4675, 4676, 4677, 4678, 4679].map(text);
        assert_eq!(figures, ["314.572 €", " 0 €", "986.535 €", "- 671.963 €", " 0 €"]);
        // Back on 56 its list keeps going as it was.
        let back = format!("{then}\nnow ok\nuntil dialog 56");
        on_save(&game, "A", &back).unwrap();
    }

    /// The Finanzen screen's black cash box (442): the next match's
    /// referee and opponent named, the day's teams of the country to
    /// incite, what the box holds; a slider's step sets its amount and
    /// shows its OK, and the two shares reach the manager at once — the
    /// cup opponents' the other way round.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_black_cash_box_shows_its_uses() {
        let game = game();
        let open = "now click bar 9\nuntil dialog 417\nnow press 2224\nuntil dialog 442";
        let Some(runner) = on_save(&game, "A", open) else { return };
        let dialog = &game.dialogs[&442];
        let view = runner.host.view();
        let text = |id| view.text(control(dialog, id).unwrap()).to_owned();
        assert_eq!(text(5546), "Schiedsrichter Robert Kempter bestechen");
        assert_eq!(text(5547), "Gegner RB Leipzig bestechen");
        assert_eq!(text(5528), "220.757 €");
        assert_eq!((text(5529), text(5530)), ("3.0%".to_owned(), "45.0%".to_owned()));
        let teams = &view.combo(control(dialog, 5542).unwrap()).unwrap().items;
        assert_eq!(&teams[..3], ["Bitte auswählen", "Eintracht Frankfurt", "1.FC Heidenheim"]);
        let then = format!("{open}\nnow slide 5539 1\nnow spin 5545 up\nnow spin 5544 down");
        let runner = on_save(&game, "A", &then).unwrap();
        let view = runner.host.view();
        let text = |id| view.text(control(dialog, id).unwrap()).to_owned();
        assert_eq!(text(5540), " 100 €");
        assert!(!view.controls[control(dialog, 5551).unwrap()].hidden);
        // One step down shows 2.8% as the original does: 29 thousandths
        // as a float times a hundred, chopped.
        assert_eq!((text(5529), text(5530)), ("2.8%".to_owned(), "45.1%".to_owned()));
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let m = records.humans().next().unwrap();
        assert_eq!(((m.gate_kept() * 1000.0) as i32, (m.cup_share_kept() * 1000.0) as i32), (29, 49));
    }

    /// The black cash box's money into the books — the gates' takings as
    /// booked — and into the manager's own pocket.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_black_cash_goes_into_the_books_or_the_pocket() {
        let game = game();
        let open = "now click bar 9\nuntil dialog 417\nnow press 2224\nuntil dialog 442";
        let figures = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let m = records.humans().next().unwrap();
            let team = records.team(m.country(), m.team()).unwrap();
            (m.black_cash(), m.cash(), team.get::<i32>(0x230 + a3_core::finances::CLUB_TO_TEAM))
        };
        let Some(before) = on_save(&game, "A", open) else { return };
        let (cash, own, gates) = figures(&before);
        let then = format!("{open}\nnow slide 5533 3\nnow press 5555\nnow slide 5534 12\nnow press 5556");
        let after = figures(&on_save(&game, "A", &then).unwrap());
        // Three steps of a hundred to the books, twelve to the pocket.
        assert_eq!(after, (cash - 300 - 1200, own + 1200, gates + 300));
    }

    /// The referee asked: A's does not take money, and only says so —
    /// his section closed for the season, the box untouched. One the news
    /// made corruptible (his +8) takes 300.000 in a first league for sure:
    /// the box pays and he promises a favour.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_referee_takes_the_black_money_or_not() {
        let game = game();
        let open = "now click bar 9\nuntil dialog 417\nnow press 2224\nuntil dialog 442";
        let state = |runner: &Runner| {
            let state = runner.host.state();
            let m = state.records.as_ref().unwrap().humans().next().unwrap().clone();
            (m.black_cash(), m.illegal_closed())
        };
        let ask = format!("{open}\nnow slide 5539 1\nnow press 5551\nuntil dialog 380");
        let Some(refused) = on_save(&game, "A", &ask) else { return };
        assert_eq!(state(&refused), (220_757, 0x80));
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "A", "corrupt-referee", |records| {
            let m = records.humans().next().unwrap();
            let team = records.team(m.country(), m.team()).unwrap();
            let referee = team.referee();
            records.block_mut(&format!("0x19f4ed8[{referee}]")).unwrap()[8] = 0x80;
            records.managers[0].set_black_cash(400_000);
        });
        let mut patched_game = Game::load(&root).unwrap();
        compose(&mut patched_game);
        // 300.000: a hundred steps of hundreds, ninety of thousands, twenty
        // of ten thousands: step 210.
        let ask = format!("{open}\nnow slide 5539 210\nnow press 5551\nuntil dialog 380");
        let Some(taken) = on_save(&patched_game, "corrupt-referee", &ask) else { return };
        let (cash, closed) = state(&taken);
        assert_eq!(cash, 100_000);
        assert_eq!(closed & 0xc0, 0xc0);
        assert_eq!((closed & 0x1c00_0000).count_ones(), 1, "{closed:#x}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The next opponent's captain asks 500.000 on A's day: a hundred is
    /// refused and costs nothing; 600.000 from a fuller box is taken.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_opponent_takes_the_black_money_above_his_price() {
        let game = game();
        let open = "now click bar 9\nuntil dialog 417\nnow press 2224\nuntil dialog 442";
        let state = |runner: &Runner| {
            let state = runner.host.state();
            let m = state.records.as_ref().unwrap().humans().next().unwrap().clone();
            (m.black_cash(), m.illegal_closed())
        };
        let ask = format!("{open}\nnow slide 5532 1\nnow press 5553\nuntil dialog 380");
        let Some(refused) = on_save(&game, "A", &ask) else { return };
        assert_eq!(state(&refused), (220_757, 0x20));
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "A", "rich-box", |records| {
            records.managers[0].set_black_cash(700_000);
        });
        let mut patched_game = Game::load(&root).unwrap();
        compose(&mut patched_game);
        // 600.000: step 100 + 90 + 50.
        let ask = format!("{open}\nnow slide 5532 240\nnow press 5553\nuntil dialog 380");
        let Some(taken) = on_save(&patched_game, "rich-box", &ask) else { return };
        assert_eq!(state(&taken), (100_000, 0x30));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A player of the next opponent: Gulácsi refuses a hundred on A —
    /// his scruples hold, and a hundred is less than his price — and,
    /// his scruples patched away, takes it.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_player_of_the_opponent_takes_the_black_money_or_not() {
        let game = game();
        let ask = "now click bar 9\nuntil dialog 417\nnow press 2224\nuntil dialog 442\n\
                   now row 5541 1\nnow slide 5538 1\nnow press 5554\nuntil dialog 380";
        let state = |runner: &Runner| {
            let state = runner.host.state();
            let m = state.records.as_ref().unwrap().humans().next().unwrap().clone();
            (m.black_cash(), m.illegal_closed(), m.bribe_target())
        };
        let Some(refused) = on_save(&game, "A", ask) else { return };
        let (cash, closed, target) = state(&refused);
        assert_eq!((cash, closed), (220_757, 0x8));
        let name = refused.host.state().records.as_ref().unwrap().known_as(target);
        assert_eq!(name.as_deref(), Some("Péter Gulácsi"));
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "A", "no-scruples", |records| {
            records.player_mut(target).unwrap().set_scruples(0);
        });
        let mut patched_game = Game::load(&root).unwrap();
        compose(&mut patched_game);
        let Some(taken) = on_save(&patched_game, "no-scruples", ask) else { return };
        assert_eq!(state(&taken), (220_657, 0xc, target));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Another team's opponents incited: nothing before a team is chosen;
    /// a hundred falls short of a hundred times the next opponent's average
    /// salary on A, and is enough once their salaries are patched to one.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_black_money_incites_another_teams_opponents_or_not() {
        let game = game();
        let open = "now click bar 9\nuntil dialog 417\nnow press 2224\nuntil dialog 442";
        let state = |runner: &Runner| {
            let state = runner.host.state();
            let m = state.records.as_ref().unwrap().humans().next().unwrap().clone();
            (m.black_cash(), m.illegal_closed())
        };
        let unchosen = format!("{open}\nnow slide 5531 1\nnow press 5552");
        let Some(runner) = on_save(&game, "A", &unchosen) else { return };
        assert_eq!(state(&runner), (220_757, 0));
        let ask = format!("{open}\nnow row 5542 1\nnow slide 5531 1\nnow press 5552\nuntil dialog 380");
        assert_eq!(state(&on_save(&game, "A", &ask).unwrap()), (220_757, 0x2));
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "A", "cheap-opponents", |records| {
            let m = records.humans().next().unwrap();
            let team = records.team(m.country(), m.team()).unwrap();
            let next = records.team(team.next_country().into(), team.next_number() as u16).unwrap();
            for id in next.players() {
                records.player_mut(id).unwrap().set_salary(1);
            }
        });
        let mut patched_game = Game::load(&root).unwrap();
        compose(&mut patched_game);
        let Some(taken) = on_save(&patched_game, "cheap-opponents", &ask) else { return };
        assert_eq!(state(&taken), (220_657, 0x3));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// An attack on another human (Probe1, two managers): Anna Alpha
    /// chosen, a player put out for 400.000 — too dear for an empty box,
    /// ordered from a patched one, and closed for the season.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_black_money_orders_an_attack_on_another_human() {
        let game = game();
        let ask = "now click bar 9\nuntil dialog 417\nnow press 2224\nuntil dialog 442\n\
                   now choose 5558 0\nnow choose 5550 0\nnow press 5557\nuntil dialog 380";
        let state = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let current = state.season.as_ref().unwrap().current;
            let m = records.managers[records.human_slot(current).unwrap()].clone();
            (m.black_cash(), m.illegal_closed(), m.attacked())
        };
        let Some(dear) = on_save(&game, "Probe1", ask) else { return };
        let dialog = &game.dialogs[&380];
        let text = dear.host.view().text(control(dialog, 4730).unwrap()).to_owned();
        assert_eq!(text, "Für diesen Anschlag brauchen Sie mehr Geld!");
        let (cash, closed, anna) = state(&dear);
        assert_eq!((cash, closed), (0, 0));
        assert!(anna > 0, "the human chosen is kept at once");
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "Probe1", "rich-attack", |records| {
            for m in &mut records.managers {
                m.set_black_cash(500_000);
            }
        });
        let mut patched_game = Game::load(&root).unwrap();
        compose(&mut patched_game);
        let Some(ordered) = on_save(&patched_game, "rich-attack", ask) else { return };
        assert_eq!(state(&ordered), (100_000, 0x8000_0000, anna));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The black cash box fills from the human's home league gates: with
    /// all of the gate kept back, the takings stay where they were and the
    /// box gains it all; with A's 3% the box gains three hundredths of the
    /// gates, the rest booked. Played on copies, a fortnight on.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn home_gates_fill_the_black_cash_box() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let play = "on 89 press 1875\non 41 ok\non 427 ok\non 218 click bar 0\non 147 click bar 18\non 302 click bar 18\n\
                    on 384 press 4741\non 308 choose 4095 0\non 96 ok\non 93 press 1943\non 499 ok\non 332 ok\non 269 ok\non 383 click at 460 210\non 2 wait\non 381 wait\non 25 ok\non 161 ok\non 270 ok\nrepeat 14 days";
        let figures = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let m = records.humans().next().unwrap();
            let t = records.team(m.country(), m.team()).unwrap();
            (m.black_cash(), t.get::<i32>(0x230 + a3_core::finances::CLUB_TO_TEAM), t.home_games())
        };
        for (name, share) in [("gate-all", 1.0f32), ("gate-some", 0.03)] {
            let root = patched(&dir, "A", name, |records| records.managers[0].set_gate_kept(share));
            let mut game = Game::load(&root).unwrap();
            compose(&mut game);
            let Some(before) = on_save(&game, name, "") else { return };
            let (box0, booked0, home0) = figures(&before);
            let after = on_save(&game, name, play).unwrap();
            let (box1, booked1, home1) = figures(&after);
            assert!(home1 > home0, "a home game in a fortnight");
            let (kept, rest) = (i64::from(box1 - box0), i64::from(booked1 - booked0));
            if share == 1.0 {
                assert!(kept > 0 && rest == 0, "{kept} {rest}");
            } else {
                let gates = kept + rest;
                assert!((kept - gates * 3 / 100).abs() <= i64::from(home1 - home0), "{kept} of {gates}");
            }
            let _ = std::fs::remove_dir_all(&root);
        }
    }

    /// The week's book of gained and lost abilities opens the next turn
    /// and then stands empty: what the days booked for the human club
    /// shows on 364 at its manager's turn — the man's name, his main
    /// position, the first ability he gained and the first he lost by
    /// name, each with its icon in the list's two icon slots, the club's
    /// name behind the caption (`FUN_00499fc3`'s rows) — and OK closes
    /// it; the week's end clears the book (`FUN_006436b3`), so the turns
    /// after this one show no 364 again.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_weeks_book_of_abilities_opens_the_turn_and_blanks() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "A", "ability-book", |_| {});
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let walk = "on 89 press 1875\non 41 ok\non 427 ok\non 218 click bar 0\non 147 click bar 18\non 302 click bar 18\n\
                    on 384 press 4741\non 308 choose 4095 0\non 96 ok\non 93 press 1943\non 499 ok\non 332 ok\non 269 ok\non 2 wait\non 381 wait\non 25 ok\non 161 ok\non 270 ok";
        let row = game.save_games().iter().position(|s| s.name == "ability-book").unwrap();
        let load = LOAD.replace("row 5144 0", &format!("row 5144 {row}"));
        let icon = |keeper: bool, good: bool, at: usize| {
            game.rules.pitch.ability_icon(keeper, good, at).unwrap()
        };
        let mut runner = Runner::new(&game, 0);
        runner.run(&Script::parse(&load).unwrap()).unwrap();
        // Two days on, the turn's week has ended and cleared its book;
        // what the days between the weeks book shows at the next turn.
        let day = runner.host.state().season.as_ref().unwrap().day;
        let into = format!("{walk}\nuntil day {}", day + 2);
        runner.run(&Script::parse(&into).unwrap()).unwrap_or_else(|err| {
            panic!("{err}\n{}", runner.log.join("\n"));
        });
        // Book a gained and a lost field ability on one man and a lost
        // keeper ability on another; an entry per man, as the writers do.
        let (want, club) = {
            let mut state = runner.host.state();
            let records = state.records.as_mut().unwrap();
            let manager = records.humans().next().unwrap();
            let (country, number) = (manager.country(), manager.team());
            let team = records.team(country, number).unwrap();
            let (slot, players, club) = (team.human(), team.players(), team.name().to_owned());
            let keeper = players
                .iter()
                .copied()
                .find(|&id| records.player(id).is_some_and(|p| p.positions()[0] == 1))
                .unwrap();
            let field = players
                .iter()
                .copied()
                .find(|&id| id != keeper && records.player(id).is_some_and(|p| p.positions()[0] != 1))
                .unwrap();
            records.ability_gained(slot, field, 2);
            records.ability_lost(slot, field, 5);
            records.ability_lost(slot, keeper, 3);
            let line = |id: u16, gained: Option<usize>, lost: Option<usize>| {
                let p = records.player(id).unwrap();
                let is_keeper = p.positions()[0] == 1;
                let names =
                    if is_keeper { &game.rules.words.keeper_abilities } else { &game.rules.words.field_abilities };
                let word = |at: Option<usize>| {
                    at.map_or(String::new(), |b| game.text(names[b]).to_owned())
                };
                [
                    records.known_as(id).unwrap(),
                    crate::screens::hub::abbreviation(&game, p.positions()[0]),
                    word(gained),
                    word(lost),
                ]
                .join("\t")
            };
            (vec![line(field, Some(2), Some(5)), line(keeper, None, Some(3))], club)
        };
        // The walk to the turn: 364 comes before it and the script, which
        // has no action for it, breaks there with the dialog standing.
        let reached = runner.run(&Script::parse(&format!("{walk}\nrepeat 20 days")).unwrap());
        assert!(matches!(reached, Err(Error::NoActions(364))), "no 364 at the turn: {:?}", reached);
        let dialog = &game.dialogs[&364];
        let list = crate::screens::control(dialog, 4598).unwrap();
        let crate::controls::ControlState::List(rows) = &runner.host.view().controls[list].state
        else {
            panic!("{}: no list on 364", runner.log.join("\n"));
        };
        assert_eq!(rows.rows[..want.len()], want[..], "364: {}", runner.log.join("\n"));
        let slots = crate::controls::list::icon_slots(&dialog.controls[list]);
        assert_eq!(
            rows.row_icons[0],
            [(slots[0], icon(false, true, 2)), (slots[1], icon(false, false, 5))]
        );
        assert_eq!(rows.row_icons[1], [(slots[1], icon(true, false, 3))]);
        let caption = crate::screens::control(dialog, 9946).unwrap();
        assert_eq!(runner.host.view().text(caption), format!("{} {club}", dialog.title));
        // OK closes it; four more days of turns show no 364 a second time.
        let day = runner.host.state().season.as_ref().unwrap().day;
        let again = format!("{walk}\non 364 ok\nuntil day {}", day + 4);
        runner.run(&Script::parse(&again).unwrap()).unwrap_or_else(|err| {
            panic!("{err}\n{}", runner.log.join("\n"));
        });
        assert_eq!(seen(&runner, 364), 1, "the book stood past the week's end:\n{}", runner.log.join("\n"));
        let _ = std::fs::remove_dir_all(&root);
    }
    /// The national menu, with a job poked into a copy of A (no kept save
    /// has one): the bar's sixteenth symbol opens it, its symbol chosen and
    /// on, the caption names the nation, the six buttons carry the file's
    /// labels and none is chosen; the bar leaves it again.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_national_menu_opens_from_the_bar() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        if on_save(&game(), "A", "").is_none() {
            return;
        }
        let root = patched(&dir, "A", "nationalmenu", |records| {
            let slot = records.human_slot(0).unwrap();
            records.managers[slot].put(0x1f72, 14i16);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let runner = on_save(&game, "nationalmenu", "now click bar 15\nuntil dialog 355").unwrap();
        let dialog = &game.dialogs[&355];
        let view = runner.host.view();
        let caption = crate::screens::control(dialog, 9935).unwrap();
        assert_eq!(view.text(caption), "ANSTOSS 3 - Nationalmannschaft Toni Lubiniecki (Deutschland), 8.9.2025");
        let bar = view.bar(crate::screens::control(dialog, 9930).unwrap()).unwrap();
        assert_eq!(bar.chosen, 15);
        assert!(!bar.off[15] && !bar.off[3] && !bar.off[5]);
        let buttons = view.options(crate::screens::control(dialog, 4538).unwrap()).unwrap();
        assert_eq!(buttons.labels.len(), 6);
        assert_eq!(buttons.labels[0], "Kader zusammenstellen");
        assert_eq!(buttons.labels[5], "Als Nationaltrainer zurücktreten");
        assert_eq!(buttons.chosen, crate::controls::options::NONE);
        // Item 5 does nothing yet; the bar's line-up symbol leaves.
        let runner = on_save(&game, "nationalmenu", "now click bar 15\nuntil dialog 355\nnow click bar 0\nuntil dialog 147").unwrap();
        assert_eq!(seen(&runner, 355), 1);
        let _ = std::fs::remove_dir_all(&root);
    }
    /// The national menu's last button, with the job poked into a copy of
    /// A: the question over the menu, the button held down under it, a no
    /// that lets it go; a yes that takes the job off the records, closes
    /// the station and leads to the line-up; and the chairman's refusal in
    /// a week the team plays.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_national_coach_resigns_unless_his_team_plays_this_week() {
        const OPEN: &str = "now click bar 15\nuntil dialog 355\nnow choose 4538 5";
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        if on_save(&game(), "A", "").is_none() {
            return;
        }
        let job = |records: &mut a3_core::records::Records| {
            let slot = records.human_slot(0).unwrap();
            records.managers[slot].put(0x1f72, 14i16);
            records.globals.put::<u8>(0xb5d172 + (slot + 1) * 8, 14);
            records.globals.put::<u16>(0xb5d174 + (slot + 1) * 8, 200);
            records.block_mut("0xa9d660[14]").unwrap()[0x88..0x8a].fill(1);
            let career = records.career_mut(slot).unwrap();
            career[0x1c3] = 2;
            career[0x624 + 0x1c + 0xc] = 9;
            career[0x624 + 0x1c + 0x10] = 7;
        };
        let root = patched(&dir, "A", "nationalresign", job);
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let question = on_save(&game, "nationalresign", &format!("{OPEN}\nuntil dialog 384")).unwrap();
        let text = crate::screens::control(&game.dialogs[&384], 4738).unwrap();
        assert_eq!(question.host.view().text(text), game.text(3471));
        let menu = question.host.views().find(|v| v.id == 355).unwrap();
        let buttons = crate::screens::control(&game.dialogs[&355], 4538).unwrap();
        assert_eq!(menu.options(buttons).unwrap().chosen, 5, "held down under the question");
        let no = on_save(&game, "nationalresign", &format!("{OPEN}\nuntil dialog 384\nnow cancel\nuntil dialog 355")).unwrap();
        assert_eq!(no.host.view().options(buttons).unwrap().chosen, crate::controls::options::NONE);
        assert_eq!(no.host.state().season.as_ref().unwrap().managers[0].national_team, Some(14));
        let yes = on_save(&game, "nationalresign", &format!("{OPEN}\nuntil dialog 384\nnow ok\nuntil dialog 380")).unwrap();
        let notice = crate::screens::control(&game.dialogs[&380], 4730).unwrap();
        assert_eq!(yes.host.view().text(notice), game.text(3472));
        let done = on_save(&game, "nationalresign", &format!("{OPEN}\nuntil dialog 384\nnow ok\nuntil dialog 380\nnow press 4731\nuntil dialog 147")).unwrap();
        let state = done.host.state();
        assert_eq!(state.season.as_ref().unwrap().managers[0].national_team, None);
        let records = state.records.as_ref().unwrap();
        let slot = records.human_slot(0).unwrap();
        assert_eq!(records.managers[slot].national_team(), 0);
        assert_eq!(records.globals.get::<u8>(0xb5d172 + (slot + 1) * 8), 0);
        assert_eq!(records.globals.get::<u16>(0xb5d174 + (slot + 1) * 8), 0);
        assert_eq!(records.block("0xa9d660[14]").unwrap()[0x88..0x8a], [0, 0]);
        let row = &records.career(slot).unwrap()[0x624 + 0x1c..0x624 + 0x38];
        let [day, month, year] = records.globals.date();
        assert_eq!((row[8], row[9]), (day as u8, month as u8));
        assert_eq!(u16::from_le_bytes([row[0xa], row[0xb]]), year);
        assert_eq!((row[0xc], row[0x10]), (4, 0));
        let _ = std::fs::remove_dir_all(&root);
        let root = patched(&dir, "A", "nationalrefuses", |records| {
            job(records);
            let soon = records.globals.week_start() + 1;
            records.block_mut("0xa9d660[14]").unwrap()[0x230..0x232].copy_from_slice(&soon.to_le_bytes());
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let refused = on_save(&game, "nationalrefuses", &format!("{OPEN}\nuntil dialog 380")).unwrap();
        assert_eq!(refused.host.view().text(notice), game.text(3615));
        let stays = on_save(&game, "nationalrefuses", &format!("{OPEN}\nuntil dialog 380\nnow press 4731\nuntil dialog 355")).unwrap();
        assert_eq!(stays.host.state().season.as_ref().unwrap().managers[0].national_team, Some(14));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The week hub does not end the turn of a national coach whose squad is
    /// too small for the international match in his week (`FUN_00650b84` asm
    /// 0x6523d3): with the week moved onto one whose Wednesday the calendar's
    /// international row flags and the team's next match pulled into it, the
    /// whistle ends the turn only while 18 of the squad stand fit; short of
    /// that the hub shows alert 4966 and its code −1600 opens the menu again.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_national_coach_turn_waits_for_eighteen_fit_in_his_squad() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        if on_save(&game(), "A", "").is_none() {
            return;
        }
        let year = game();
        // The first week of the year whose Wednesday — `week_start + 3`, as
        // `FUN_0067dc44` asks `FUN_00629d80` — stands in an international
        // window of row 0: "Nat.-Freund." 0x04000000, "WM-Quali" 0x08000000.
        let week = (0..52)
            .map(|w| w * 7)
            .find(|&w| {
                let wed = year.data.calendar.entry(0, w + 3) & 0x7f00_0000;
                wed == 0x0400_0000 || wed == 0x0800_0000
            })
            .expect("no international window in the calendar's first row") as i16;
        let job = |records: &mut a3_core::records::Records| {
            let slot = records.human_slot(0).unwrap();
            records.managers[slot].put(0x1f72, 14i16);
            records.globals.put::<u8>(0xb5d172 + (slot + 1) * 8, 14);
            records.globals.put::<u16>(0xb5d174 + (slot + 1) * 8, 200);
            records.block_mut("0xa9d660[14]").unwrap()[0x88..0x8a].fill(1);
            let career = records.career_mut(slot).unwrap();
            career[0x1c3] = 2;
            career[0x624 + 0x1c + 0xc] = 9;
            career[0x624 + 0x1c + 0x10] = 7;
        };
        let window = |records: &mut a3_core::records::Records| {
            records.globals.set_week_start(week);
            let next = (week + 1).to_le_bytes();
            records.block_mut("0xa9d660[14]").unwrap()[0x230..0x232].copy_from_slice(&next);
        };
        let root = patched(&dir, "A", "nationalsmall", |records| {
            job(records);
            window(records);
            // The squad list down to 17: `FUN_00650b84` counts every player
            // of the list at record 0x112/0x1f2 whose days of injury at his
            // +0x2c stand under a week — 17 is the first number short.
            records.block_mut("0xa9d660[14]").unwrap()[0xf2..0xf4].copy_from_slice(&17u16.to_le_bytes());
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let before = on_save(&game, "nationalsmall", "").unwrap();
        let day = before.host.state().season.as_ref().unwrap().day;
        drop(before);
        let notice = crate::screens::control(&game.dialogs[&380], 4730).unwrap();
        let alert = on_save(&game, "nationalsmall", "now click bar 18\nuntil dialog 380").unwrap();
        assert_eq!(alert.host.view().text(notice), game.text(4966));
        let runner = on_save(
            &game,
            "nationalsmall",
            "now click bar 18\nuntil dialog 380\nnow press 4731\nuntil dialog 355",
        )
        .unwrap();
        assert_eq!(seen(&runner, 355), 1, "the menu did not come back:\n{}", runner.log.join("\n"));
        let state = runner.host.state();
        assert_eq!(state.season.as_ref().unwrap().day, day, "the turn ended anyway");
        assert_eq!(state.season.as_ref().unwrap().managers[0].national_team, Some(14));
        let _ = std::fs::remove_dir_all(&root);
        // The saved squad of 23, all of them fit: the turn ends as ever,
        // alert and menu out of the way to the next day.
        let root = patched(&dir, "A", "nationalbig", |records| {
            job(records);
            let (ids, count) = {
                let team = records.block("0xa9d660[14]").unwrap();
                let n = i16::from_le_bytes([team[0xf2], team[0xf3]]) as usize;
                let ids: Vec<u16> = (0..n)
                    .map(|k| u16::from_le_bytes([team[0x1d2 + 2 * k], team[0x1d2 + 2 * k + 1]]))
                    .collect();
                (ids, n)
            };
            assert!(count > 17, "the saved squad of nation 14 holds only {count}");
            window(records);
            for id in ids {
                records.player_mut(id).unwrap().set_injured_to([0, 0]);
            }
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        // Day 63 of save A is a week boundary (`day % 7 == 0`): after the
        // whistle its week end opens, then the new week's wait and turn,
        // clicked again, until the walk stands in the next day.
        let runner = on_save(
            &game,
            "nationalbig",
            "on 147 click bar 18\non 2 wait\non 461 ok\non 270 ok\non 381 wait\nuntil day 64",
        )
        .unwrap();
        assert_eq!(
            seen(&runner, 380) + seen(&runner, 355),
            0,
            "the turn was held back:\n{}",
            runner.log.join("\n")
        );
        let _ = std::fs::remove_dir_all(&root);
    }
    /// The picker on a job poked into a copy of A: its squad list holds
    /// the block's players, a squad row sends one home and OK writes the
    /// list back; the Verein combo opens a club's list — its players of
    /// the coach's nation, red while the squad holds them — and a row
    /// there calls him up, inserted by main position; the Land combo
    /// moves the search to the other country's clubs, and the position
    /// group and the info picture stand on their controls — the picture
    /// needs its two clicks for the list's second view (name, position,
    /// strength, club and the grade of the last match).
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_national_coach_calls_up_and_sends_home_in_the_picker() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        if on_save(&game(), "A", "").is_none() {
            return;
        }
        let pickable = |records: &a3_core::records::Records, id: u16| {
            records.player(id).is_some_and(|p| {
                p.nation() == 14 && p.retiring() & 2 == 0 && p.picks() & 2 == 0
            })
        };
        let position = |records: &a3_core::records::Records, id: u16| {
            records.player(id).map_or(0, |p| p.positions()[0])
        };
        let mut seeds = [0u16; 2];
        let mut add = 0u16;
        let mut club_no = 0u16;
        let mut add_row = 0usize;
        let mut expect: Vec<u16> = Vec::new();
        let mut shown: Vec<u16> = Vec::new();
        let root = patched(&dir, "A", "nationalpick", |records| {
            let slot = records.human_slot(0).unwrap();
            records.managers[slot].put(0x1f72, 14i16);
            // A club of the coach's land with three players he may call
            // up: two seeded into the block in position order, one to
            // call from the club's list.
            club_no = (0..records.countries.team_count(14).max(0) as u16)
                .find(|&n| {
                    records.team(14, n).is_some_and(|t| {
                        t.players().iter().filter(|&&id| pickable(records, id)).count() >= 3
                    })
                })
                .unwrap();
            let team = records.team(14, club_no).unwrap().players();
            shown = team.iter().copied().filter(|&id| pickable(records, id)).collect();
            let mut ranked = shown.clone();
            ranked.sort_by_key(|&id| (usize::from(position(records, id)), usize::from(id)));
            seeds = [ranked[0], ranked[1]];
            add = shown.iter().copied().find(|&id| id != seeds[0] && id != seeds[1]).unwrap();
            add_row = shown.iter().position(|&id| id == add).unwrap();
            expect = seeds.to_vec();
            a3_core::national::call_up(&mut expect, records, add);
            let team = records.block_mut("0xa9d660[14]").unwrap();
            team[0xf2..0xf4].copy_from_slice(&2u16.to_le_bytes());
            for (k, &id) in seeds.iter().enumerate() {
                team[0x1d2 + 2 * k..0x1d4 + 2 * k].copy_from_slice(&id.to_le_bytes());
            }
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let open = "now click bar 15\nuntil dialog 355\nnow choose 4538 0\nuntil dialog 357";
        let dialog = &game.dialogs[&357];
        let squad_list = crate::screens::control(dialog, 4556).unwrap();
        let club_list = crate::screens::control(dialog, 4553).unwrap();
        // Open: the block's two stand in the list, the free slots count
        // from 22.
        let runner = on_save(&game, "nationalpick", open).unwrap();
        let view = runner.host.view();
        let keys: Vec<usize> = seeds.iter().map(|&id| usize::from(id)).collect();
        assert_eq!(view.list(squad_list).unwrap().keys, keys, "{}", runner.log.join("\n"));
        let free = crate::screens::control(dialog, 4559).unwrap();
        assert!(view.text(free).contains("20"), "{}", view.text(free));
        // The club's list: the pickable players in the club's own order,
        // the seeded ones red.
        let runner = on_save(&game, "nationalpick", &format!("{open}\nnow row 4555 {club_no}")).unwrap();
        let list = runner.host.view().list(club_list).unwrap();
        let shown_keys: Vec<usize> = shown.iter().map(|&id| usize::from(id)).collect();
        assert_eq!(list.keys, shown_keys, "the club list: {}", list.rows.join(" / "));
        let inks: Vec<Option<[u8; 3]>> = shown
            .iter()
            .map(|&id| Some(if seeds.contains(&id) { [255, 0, 0] } else { [0, 0, 0] }))
            .collect();
        assert_eq!(list.inks, inks);
        // A club row calls him up at the place the insert by main
        // position gives him.
        let pick = &format!("{open}\nnow row 4555 {club_no}\nnow row 4553 {add_row}");
        let runner = on_save(&game, "nationalpick", pick).unwrap();
        let expect_keys: Vec<usize> = expect.iter().map(|&id| usize::from(id)).collect();
        assert_eq!(runner.host.view().list(squad_list).unwrap().keys, expect_keys);
        // OK writes the squad into the team's block.
        let runner = on_save(&game, "nationalpick", &format!("{pick}\nnow ok\nuntil dialog 355")).unwrap();
        let state = runner.host.state();
        let team = state.records.as_ref().unwrap().block("0xa9d660[14]").unwrap();
        assert_eq!(i16::from_le_bytes([team[0xf2], team[0xf3]]), 3);
        let ids: Vec<u16> = (0..3)
            .map(|k| u16::from_le_bytes([team[0x1d2 + 2 * k], team[0x1d2 + 2 * k + 1]]))
            .collect();
        assert_eq!(ids, expect);
        // A squad row sends its player home; OK writes the one left.
        let runner = on_save(
            &game,
            "nationalpick",
            &format!("{open}\nnow row 4556 0\nnow ok\nuntil dialog 355"),
        )
        .unwrap();
        let state = runner.host.state();
        let team = state.records.as_ref().unwrap().block("0xa9d660[14]").unwrap();
        assert_eq!(i16::from_le_bytes([team[0xf2], team[0xf3]]), 1);
        assert_eq!(u16::from_le_bytes([team[0x1d2], team[0x1d3]]), seeds[1]);
        // The info picture: its first click lets the group go (the class
        // starts it chosen while the dialog's mode is not), the second
        // draws the club and the grade in place of form, age and weeks.
        let info = crate::screens::control(dialog, 6358).unwrap();
        let runner = on_save(&game, "nationalpick", &format!("{open}\nnow choose 6358 0")).unwrap();
        assert_eq!(
            runner.host.view().options(info).unwrap().chosen,
            crate::controls::options::NONE,
            "the first click did not let the picture go"
        );
        assert_eq!(runner.host.view().list(squad_list).unwrap().rows[0].split('\t').count(), 7);
        let runner = on_save(
            &game,
            "nationalpick",
            &format!("{open}\nnow choose 6358 0\nnow choose 6358 0"),
        )
        .unwrap();
        let view = runner.host.view();
        assert_eq!(view.options(info).unwrap().chosen, 0);
        let list = view.list(squad_list).unwrap();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        for (row, &key) in list.rows.iter().zip(&list.keys) {
            let cols: Vec<&str> = row.split('\t').collect();
            assert_eq!(cols.len(), 5, "{row}");
            let player = records.player(key as u16).unwrap();
            let [country, number] = player.club();
            assert_eq!(cols[3], game.club_name(Some(records), country, i16::from(number)));
            let grade = player.grade() & 0xf;
            assert_eq!(
                cols[4],
                if grade == 0 { "-".to_owned() } else { format!("{:.1}", (5 * grade) as f32 / 10.0) }
            );
        }
        // The position group stands chosen while the bests list stays
        // empty: the team block names no coach whose record it reads.
        let positions = crate::screens::control(dialog, 4775).unwrap();
        let bests = crate::screens::control(dialog, 4558).unwrap();
        let runner = on_save(&game, "nationalpick", &format!("{open}\nnow choose 4775 1")).unwrap();
        let view = runner.host.view();
        assert_eq!(view.options(positions).unwrap().chosen, 1);
        assert!(view.list(bests).unwrap().rows.is_empty());
        // The Land combo moves the Verein list and the club list to the
        // other country's first club.
        let lands: Vec<u16> = (1..0x75u16)
            .filter(|&c| {
                runner.host.state().records.as_ref().unwrap().countries.kind(usize::from(c)) != 0
            })
            .collect();
        let land = crate::screens::control(dialog, 4554).unwrap();
        let verein = crate::screens::control(dialog, 4555).unwrap();
        let runner = on_save(&game, "nationalpick", &format!("{open}\nnow row 4554 1")).unwrap();
        let view = runner.host.view();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        assert_eq!(view.combo(land).unwrap().selected, Some(1));
        let items: Vec<String> = (0..records.countries.team_count(usize::from(lands[1])).max(0) as u16)
            .map(|n| game.club_name(Some(records), lands[1] as u8, n as i16))
            .collect();
        assert_eq!(view.combo(verein).unwrap().items, items, "the Verein list did not follow the Land");
        let want: Vec<usize> = records
            .team(lands[1], 0)
            .map_or_else(Vec::new, |t| t.players())
            .iter()
            .copied()
            .filter(|&id| pickable(records, id))
            .map(|id| usize::from(id))
            .collect();
        assert_eq!(view.list(club_list).unwrap().keys, want);
        let _ = std::fs::remove_dir_all(&root);
    }
    /// The search on a job poked into a copy of A (dialog 351): the Land
    /// combo opens on the coach's nation, the Verein list on the first
    /// club; the player list holds the club's foreigners — not of his
    /// team's nation, uncapped, not set in their ways — and a row opens
    /// the planner, whose booked day lands in the search list with the
    /// coach's id; a second click on him answers with the alert.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_national_coach_books_a_talk_with_a_foreign_player() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        if on_save(&game(), "A", "").is_none() {
            return;
        }
        let foreign = |records: &a3_core::records::Records, id: u16| {
            records.player(id).is_some_and(|p| {
                id < 0x4269
                    && p.nation() != 14
                    && p.kind_12_matches() == 0
                    && p.manner() & 0x80 == 0
            })
        };
        let mut land_pos = 0usize;
        let mut club_no = 0u16;
        let mut init_liga = 0u16;
        let mut init: Vec<u16> = Vec::new();
        let mut want: Vec<u16> = Vec::new();
        let mut picked = 0u16;
        let mut pick_row = 0usize;
        let mut free_day = 0usize;
        let mut booked_on = 0i16;
        let mut coach_id = 0u16;
        let mut entries = 0usize;
        let root = patched(&dir, "A", "nationalsearch", |records| {
            assert_ne!(records.countries.kind(14), 0, "save A's land 14 is no country");
            let slot = records.human_slot(0).unwrap();
            records.managers[slot].put(0x1f72, 14i16);
            land_pos = (1..0x75u16)
                .filter(|&c| records.countries.kind(usize::from(c)) != 0)
                .position(|c| c == 14)
                .unwrap();
            init_liga = records.team(14, 0).map_or(0, |t| (u16::from(t.level()) + 1).min(3));
            init = records
                .team(14, 0)
                .map_or_else(Vec::new, |t| t.players())
                .iter()
                .copied()
                .filter(|&id| foreign(records, id))
                .collect();
            club_no = (0..records.countries.team_count(14).max(0) as u16)
                .find(|&n| {
                    records
                        .team(14, n)
                        .is_some_and(|t| t.players().iter().any(|&id| foreign(records, id)))
                })
                .unwrap();
            want = records
                .team(14, club_no)
                .unwrap()
                .players()
                .iter()
                .copied()
                .filter(|&id| foreign(records, id))
                .collect();
            pick_row = want
                .iter()
                .position(|&id| !a3_core::transfers::pending(records, id))
                .unwrap();
            picked = want[pick_row];
            let (start, today) =
                (records.globals.week_start(), records.globals.day());
            free_day = (0..7)
                .find(|&k| {
                    records.planners.get(slot).is_some_and(|p| {
                        p.day(start as usize + k).kind == 0 && start as usize + k >= today as usize
                    })
                })
                .expect("no free day in the week");
            booked_on = start + free_day as i16;
            coach_id = u16::from(records.managers[slot].human());
            entries =
                usize::try_from(i16::from_le_bytes(records.block("0x1c53d98").unwrap()[0..2].try_into().unwrap()))
                    .unwrap_or(0);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let dialog = &game.dialogs[&351];
        let land = crate::screens::control(dialog, 4510).unwrap();
        let clubs = crate::screens::control(dialog, 4508).unwrap();
        let players = crate::screens::control(dialog, 4507).unwrap();
        let notice = crate::screens::control(&game.dialogs[&380], 4730).unwrap();
        let open = "now click bar 15\nuntil dialog 355\nnow choose 4538 1\nuntil dialog 351";
        let keys = |ids: &[u16]| ids.iter().map(|&id| usize::from(id)).collect::<Vec<usize>>();
        // Open: the caption keeps the file's title; the combo on his own
        // nation; the Verein list sorted by name, no row painted chosen —
        // `FUN_007a9fb0` selects row 0 but the list draws it plain —
        // while the players stand at the country's first club.
        let runner = on_save(&game, "nationalsearch", open).unwrap();
        let view = runner.host.view();
        let caption = crate::screens::control(dialog, 9935).unwrap();
        assert_eq!(view.text(caption), "ANSTOSS 3 - Spieler für Gespräch aussuchen");
        assert_eq!(view.combo(land).unwrap().selected, Some(land_pos), "{}", runner.log.join("\n"));
        let list = view.list(clubs).unwrap();
        assert_eq!(list.selected, None);
        let order = crate::screens::search::stricmp;
        assert!(
            list.rows.windows(2).all(|w| order(&w[0]) <= order(&w[1])),
            "the Verein list sorted: {}",
            list.rows.join(" / ")
        );
        let club_row = list.keys.iter().position(|&k| k == usize::from(club_no)).unwrap();
        let plist = view.list(players).unwrap();
        assert_eq!(plist.keys, keys(&init));
        assert_eq!(plist.rows[0].split('\t').nth(3).unwrap(), init_liga.to_string());
        // The Verein row moves the player list; the row keeps the file's
        // seven columns.
        let pick = &format!("{open}\nnow row 4508 {club_row}\n");
        let runner = on_save(&game, "nationalsearch", pick).unwrap();
        let list = runner.host.view().list(players).unwrap();
        assert_eq!(list.keys, keys(&want), "the player list: {}", list.rows.join(" / "));
        assert_eq!(list.rows[0].split('\t').count(), 7);
        // His row opens the planner; the day booked there — and the
        // coach — land in the search list behind the entries standing.
        let booked =
            &format!("{pick}now row 4507 {pick_row}\nuntil dialog 412\nnow choose 5034 {free_day}\nuntil dialog 351");
        let runner = on_save(&game, "nationalsearch", booked).unwrap();
        let state = runner.host.state();
        let block = state.records.as_ref().unwrap().block("0x1c53d98").unwrap();
        let e = 2 + entries * 6;
        assert_eq!(i16::from_le_bytes(block[0..2].try_into().unwrap()), entries as i16 + 1);
        assert_eq!(u16::from_le_bytes(block[e..e + 2].try_into().unwrap()), picked);
        assert_eq!(i16::from_le_bytes(block[e + 2..e + 4].try_into().unwrap()), booked_on);
        assert_eq!(u16::from_le_bytes(block[e + 4..e + 6].try_into().unwrap()), coach_id);
        // The same row again answers with the week's alert, and the
        // planner stays closed.
        let runner = on_save(
            &game,
            "nationalsearch",
            &format!("{booked}\nnow row 4507 {pick_row}\nuntil dialog 380"),
        )
        .unwrap();
        assert_eq!(runner.host.view().text(notice), game.text(3478));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The talks' overview (471) over a copy of A with the job poked and
    /// two talks written into the coach's table: its button held down
    /// under the dialog, one row per talk in the table's order — the
    /// player's name, his nation's three letters, his club and how far
    /// the talk came — and OK letting the button go.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_talks_overview_lists_the_coach_s_talks() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        if on_save(&game(), "A", "").is_none() {
            return;
        }
        let root = patched(&dir, "A", "nationaltalks", |records| {
            let slot = records.human_slot(0).unwrap();
            records.managers[slot].put(0x1f72, 14i16);
            let team = records.team(14, 0).expect("A has a first German club");
            let (first, second) = (team.players()[0], team.players()[1]);
            let b = records.block_mut("0xa55708[0]").unwrap();
            b[0..2].copy_from_slice(&2i16.to_le_bytes());
            b[2..4].copy_from_slice(&first.to_le_bytes());
            b[4] = 1;
            b[8..10].copy_from_slice(&second.to_le_bytes());
            b[10] = 6;
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let open = "now click bar 15\nuntil dialog 355\nnow choose 4538 2\nuntil dialog 471";
        let runner = on_save(&game, "nationaltalks", open).unwrap();
        let dialog = &game.dialogs[&471];
        let view = runner.host.view();
        let caption = crate::screens::control(dialog, 9935).unwrap();
        assert_eq!(view.text(caption), "ANSTOSS 3 - Übersicht der Einbürgerungsgespräche");
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let team = records.team(14, 0).unwrap();
        let (first, second) = (team.players()[0], team.players()[1]);
        let row = |id: u16, status: u16| {
            let r = records.player(id).unwrap();
            let [country, number] = r.club();
            [
                records.known_as(id).unwrap(),
                game.world.nation_codes[usize::from(r.nation())].clone(),
                game.club_name(Some(records), country, i16::from(number)),
                game.text(status).to_owned(),
            ]
            .join("\t")
        };
        let list = view.list(crate::screens::control(dialog, 6031).unwrap()).unwrap();
        assert_eq!(
            list.rows,
            [row(first, 3475), row(second, 3477)],
            "the talks' rows: {}",
            list.rows.join(" / ")
        );
        assert_eq!(list.keys, [usize::from(first), usize::from(second)]);
        // The third button stays held down while the overview stands over
        // the menu, as the menu clears it only when the dialog closes.
        let buttons = crate::screens::control(&game.dialogs[&355], 4538).unwrap();
        let menu = runner.host.views().find(|v| v.id == 355).unwrap();
        assert_eq!(menu.options(buttons).unwrap().chosen, 2, "held down under the overview");
        let closed =
            on_save(&game, "nationaltalks", &format!("{open}\nnow ok\nuntil dialog 355")).unwrap();
        assert_eq!(
            closed.host.view().options(buttons).unwrap().chosen,
            crate::controls::options::NONE
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The friendlies arranger (dialog 354) over a copy of A with the job
    /// poked, a coach number of 1 on nation 14 and the first coach's
    /// record filled: three candidates per slot with land 1, 2 and 3 and
    /// the number 200 behind them, the third booked at the first slot and
    /// played 2:1.  The dates stand on the row's three friendly days
    /// (the exe's fixed international row at 0x8914e0); the booked slot
    /// shows name, flag and score with its spinner hidden at the
    /// candidate under it, the free ones stand at the free choice 3.  The
    /// list holds the window matches whose pairing from A's date entry
    /// names nation 14, and no friendly row for a slot without a
    /// candidate under its spinner.  A spinner books the candidate under
    /// it and brings that day's row; spun back to the free choice the
    /// display clears, yet OK still writes him — the original's quirk.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_national_coach_arranges_his_friendlies() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        if on_save(&game(), "A", "").is_none() {
            return;
        }
        let root = patched(&dir, "A", "nationalfriendly", |records| {
            let slot = records.human_slot(0).unwrap();
            records.managers[slot].put(0x1f72, 14i16);
            records.block_mut("0xa9d660[14]").unwrap()[0x88] = 1;
            let rec = records.block_mut("0xa55978[0]").unwrap();
            for k in 0..3usize {
                for p in 0..3usize {
                    let at = k * 0x12 + p * 6;
                    rec[at] = (p + 1) as u8;
                    rec[at + 2] = 200;
                }
                rec[0x36 + k * 6..0x3c + k * 6].fill(0);
            }
            rec[0x36] = 3;
            rec[0x38] = 200;
            rec[0x48] = 2;
            rec[0x49] = 1;
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let dialog = &game.dialogs[&354];
        let st = |id: u16| crate::screens::control(dialog, id).unwrap();
        let open = "now click bar 15\nuntil dialog 355\nnow choose 4538 3\nuntil dialog 354";
        let base = on_save(&game, "nationalfriendly", open).unwrap();
        let view = base.host.view();
        let state = base.host.state();
        let records = state.records.as_ref().unwrap();
        let year = state.season.as_ref().unwrap().year;
        let land = |n: u8| game.club_name(Some(records), n, 200);
        let day_text = |day: usize| {
            crate::screens::hub::date_text(&game, a3_core::calendar::date(year, day))
        };
        for (k, &day) in a3_core::national::friendly_dates(&game.calendar).iter().enumerate() {
            assert_eq!(view.text(st(4527 + k as u16)), day_text(day), "date {k}");
        }
        assert_eq!(view.text(st(4524)), land(3), "the booked slot shows its candidate");
        assert_eq!(view.text(st(4533)), "2:1");
        for id in [4525, 4526, 4534, 4535] {
            assert_eq!(view.text(st(id)), "");
        }
        let hidden = |id: u16| view.controls[st(id)].hidden;
        assert!(hidden(4522) && hidden(4523) && hidden(4530), "the played day answers no more");
        assert!(!hidden(4521) && !hidden(4531) && !hidden(4532));
        assert_eq!(view.value(st(4530)).unwrap().value, 2, "the candidate the booking stands under");
        assert_eq!((view.value(st(4531)).unwrap().value, view.value(st(4532)).unwrap().value), (3, 3));
        assert_eq!(view.value(st(4531)).unwrap().range, (0, 3));
        // The rows as the original's list function draws them, straight
        // from the raw save bytes: the eight window days after today from
        // the international row, the matches whose pairing in A's date
        // entry names nation 14, and under a friendly day the candidate
        // its spinner holds (land and number as poked, candidate `p` of
        // any slot standing for land `p + 1`).
        let b = records.block("0x1ba07c0").unwrap();
        let w16 = |b: &[u8], at: usize| u16::from_le_bytes([b[at], b[at + 1]]);
        let entry = (0..6).find(|&idx| {
            let e = idx * 0xda;
            let count = usize::from(w16(b, e).min(0x40));
            (0..count).any(|k| w16(b, e + 2 + 2 * k) == 14)
        });
        assert!(entry.is_some(), "A's date entries name nation 14");
        let entry = entry.unwrap() * 0xda;
        let count = usize::from(w16(b, entry).min(0x40));
        let parity = usize::from(records.globals.start_year() & 1);
        let today = usize::from(records.globals.day());
        let mut window: Vec<(usize, u32)> = Vec::new();
        for day in today + 1..a3_core::calendar::DAYS {
            let flag = game.calendar.entry(0, day);
            if matches!(flag & 0x7f00_0000, 0x0400_0000 | 0x0800_0000) {
                window.push((day, flag));
                if window.len() == 8 {
                    break;
                }
            }
        }
        assert_eq!(window.len(), 8, "A's year holds eight window days");
        let rows_with = |states: [i8; 3]| -> Vec<String> {
            window
                .iter()
                .filter_map(|&(day, flag)| {
                    if flag & 0x7f00_0000 == 0x0800_0000 {
                        let column = (flag & 0x3f) as usize + 5 * parity;
                        let opponent = (0..3).find_map(|k| {
                            let at = entry + 0x9e + column * 6 + k * 2;
                            let (x, y) = (usize::from(b[at]), usize::from(b[at + 1]));
                            if x >= count || y >= count {
                                return None;
                            }
                            let (na, nb) =
                                (w16(b, entry + 2 + 2 * x) as i16, w16(b, entry + 2 + 2 * y) as i16);
                            if na == 14 {
                                Some(nb)
                            } else if nb == 14 {
                                Some(na)
                            } else {
                                None
                            }
                        })?;
                        Some(
                            [
                                day_text(day),
                                game.club_name(Some(records), opponent as u8, 200),
                                game.text(3604).to_owned(),
                            ]
                            .join("\t"),
                        )
                    } else {
                        let state = *states.get((flag & 0x3f) as usize)?;
                        if !(0..3).contains(&state) {
                            return None;
                        }
                        Some(
                            [
                                day_text(day),
                                game.club_name(Some(records), (state + 1) as u8, 200),
                                game.text(3605).to_owned(),
                            ]
                            .join("\t"),
                        )
                    }
                })
                .collect()
        };
        let list = view.list(st(4537)).unwrap();
        assert_eq!(list.rows, rows_with([-1, 3, 3]), "the list: {}", list.rows.join(" / "));
        assert_eq!(list.tabs, [0.0, 100.0, 210.0]);
        // The fourth button stands held while the arranger is up.
        let buttons = crate::screens::control(&game.dialogs[&355], 4538).unwrap();
        assert_eq!(
            base.host.views().find(|v| v.id == 355).unwrap().options(buttons).unwrap().chosen,
            3,
            "held down under the arranger"
        );
        // The second slot's spinner books the candidate under it and the
        // day's friendly row comes in.
        let runner =
            on_save(&game, "nationalfriendly", &format!("{open}\nnow spin 4531 down")).unwrap();
        let view = runner.host.view();
        assert_eq!(view.value(st(4531)).unwrap().value, 2);
        assert_eq!(view.text(st(4525)), land(3));
        assert!(!view.controls[st(4522)].hidden);
        assert_eq!(view.list(st(4537)).unwrap().rows, rows_with([-1, 2, 3]));
        // Three steps down at the third slot reach the first candidate,
        // up again to the free choice the display clears.
        let down = "now spin 4532 down\nnow spin 4532 down\nnow spin 4532 down";
        let runner = on_save(&game, "nationalfriendly", &format!("{open}\n{down}")).unwrap();
        let view = runner.host.view();
        assert_eq!(view.value(st(4532)).unwrap().value, 0);
        assert_eq!(view.text(st(4526)), land(1));
        assert!(!view.controls[st(4523)].hidden);
        assert_eq!(view.list(st(4537)).unwrap().rows, rows_with([-1, 3, 0]));
        let up = &format!("{down}\nnow spin 4532 up\nnow spin 4532 up\nnow spin 4532 up");
        let runner = on_save(&game, "nationalfriendly", &format!("{open}\n{up}")).unwrap();
        let view = runner.host.view();
        assert_eq!(view.value(st(4532)).unwrap().value, 3);
        assert_eq!(view.text(st(4526)), "");
        assert!(view.controls[st(4523)].hidden);
        assert_eq!(
            view.list(st(4537)).unwrap().rows,
            rows_with([-1, 3, 3]),
            "the free choice shows no friendly row"
        );
        // OK writes all three as the dialog holds them: a slot spun back
        // to the free choice keeps the candidate it last stood under (the
        // original only cleared the display), and the menu button lets go
        // with the dialog.
        let runner =
            on_save(&game, "nationalfriendly", &format!("{open}\n{up}\nnow ok\nuntil dialog 355"))
                .unwrap();
        let state = runner.host.state();
        let rec = state.records.as_ref().unwrap().block("0xa55978[0]").unwrap();
        assert_eq!(&rec[0x36..0x3c], &[3, 0, 200, 0, 0, 0], "the played slot stays as it was");
        assert_eq!(&rec[0x3c..0x42], [0, 0, 0, 0, 0, 0], "an untouched slot keeps its empty entry");
        assert_eq!(
            &rec[0x36 + 2 * 6..0x3c + 2 * 6],
            &[3, 0, 200, 0, 0, 0],
            "the last candidate under the spun-away spinner still books"
        );
        assert_eq!(
            runner.host.view().options(buttons).unwrap().chosen,
            crate::controls::options::NONE
        );
        // A slot spun to a candidate and confirmed stands booked there.
        let runner = on_save(
            &game,
            "nationalfriendly",
            &format!("{open}\nnow spin 4531 down\nnow ok\nuntil dialog 355"),
        )
        .unwrap();
        let state = runner.host.state();
        let rec = state.records.as_ref().unwrap().block("0xa55978[0]").unwrap();
        assert_eq!(&rec[0x3c..0x42], &[3, 0, 200, 0, 0, 0], "the picked candidate stands booked");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The infos on the national teams (472) over a copy of A with the job
    /// poked: the combo holds the lands 1 to 115 and stands on the
    /// coach's own nation, that land's big flag stands under the header,
    /// and the list is empty — the results table `0xa7c260` no save block
    /// covers and nothing writes stands zero, so no land has a tournament
    /// booked (the original screen c1-item4-erfolge looks the same).  A
    /// combo row moves flag and rows together; the fifth button is held
    /// under the menu until OK lets it go.  A coach whose nation number
    /// stands outside the combo's 1 to 115 still opens the infos with
    /// nothing chosen: land 0's empty rows and the 76th name's catch-all
    /// flag stand, the original's `DAT_00b5cbb0`.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_infos_show_the_nations_teams() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let installed = game();
        if on_save(&installed, "A", "").is_none() {
            return;
        }
        let root = patched(&dir, "A", "nationalinfo", |records| {
            let slot = records.human_slot(0).unwrap();
            records.managers[slot].put(0x1f72, 14i16);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let flag = |name: &str| crate::controls::bitmap::Picture {
            source: crate::controls::bitmap::PictureSource::Archive(format!("gross/{name}.bmp")),
            part: None,
            black_clear: false,
        };
        let dialog = &game.dialogs[&472];
        let st = |id: u16| crate::screens::control(dialog, id).unwrap();
        let open = "now click bar 15\nuntil dialog 355\nnow choose 4538 4\nuntil dialog 472";
        let base = on_save(&game, "nationalinfo", open).unwrap();
        let view = base.host.view();
        let caption = crate::screens::control(dialog, 9935).unwrap();
        assert_eq!(view.text(caption), "ANSTOSS 3 - Infos über Nationalteams");
        // The combo over the lands 1 to 115, chosen to his own nation 14.
        let combo = view.combo(st(6028)).unwrap();
        assert_eq!(combo.items.len(), 115, "the lands 1 to 115");
        assert_eq!(combo.items[13], game.world.nation(14));
        assert_eq!(combo.selected, Some(13), "{}", base.log.join("\n"));
        // The land's big flag, and no row anywhere: the results table
        // stands zero in every save.
        assert_eq!(
            view.controls[st(6029)].state.as_bitmap().unwrap().picture,
            Some(flag(&game.data.nation_flags[14]))
        );
        let list = view.list(st(6030)).unwrap();
        assert!(list.rows.is_empty() && list.keys.is_empty(), "the list stands empty");
        // The fifth button stands held while the infos are up.
        let buttons = crate::screens::control(&game.dialogs[&355], 4538).unwrap();
        assert_eq!(
            base.host.views().find(|v| v.id == 355).unwrap().options(buttons).unwrap().chosen,
            4,
            "held down under the infos"
        );
        // A combo row moves flag and rows together.
        let runner = on_save(&game, "nationalinfo", &format!("{open}\nnow row 6028 2")).unwrap();
        let view = runner.host.view();
        assert_eq!(view.combo(st(6028)).unwrap().selected, Some(2));
        assert_eq!(
            view.controls[st(6029)].state.as_bitmap().unwrap().picture,
            Some(flag(&game.data.nation_flags[3]))
        );
        assert!(view.list(st(6030)).unwrap().rows.is_empty());
        // OK lets the button go.
        let closed =
            on_save(&game, "nationalinfo", &format!("{open}\nnow ok\nuntil dialog 355")).unwrap();
        assert_eq!(
            closed.host.view().options(buttons).unwrap().chosen,
            crate::controls::options::NONE
        );
        // A coach whose nation number stands outside the combo's 1 to
        // 115 sees nothing chosen; land 0's empty rows and the 76th
        // name's catch-all flag stand, as the original with land 0 from
        // `DAT_00b5cbb0`.
        let far = patched(&dir, "A", "nationalinfo2", |records| {
            let slot = records.human_slot(0).unwrap();
            records.managers[slot].put(0x1f72, 200i16);
        });
        let mut other = Game::load(&far).unwrap();
        compose(&mut other);
        let runner = on_save(&other, "nationalinfo2", open).unwrap();
        let view = runner.host.view();
        let os = |id: u16| crate::screens::control(&other.dialogs[&472], id).unwrap();
        assert_eq!(view.combo(os(6028)).unwrap().selected, None, "{}", runner.log.join("\n"));
        assert_eq!(
            view.controls[os(6029)].state.as_bitmap().unwrap().picture,
            Some(flag(&other.data.nation_flags[76]))
        );
        assert!(view.list(os(6030)).unwrap().rows.is_empty());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&far);
    }

    /// The week start of day 70 after save A's week: 25 for all humans
    /// by itself, before the turn; a click on a listed player's talk on a
    /// free day → 376 with Union alone, ticked; its OK joins Union after
    /// the first participant and the planner's day, and 25 fills again
    /// with the row red (TERMINE-BEOBACHTUNG.md §2.6, e0–e2x1).
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_week_start_shows_the_talks_and_lets_the_humans_join() {
        use a3_core::transfers;
        // The replica's week books no talks of the computer clubs and runs
        // none: Zetterer's role goes to Enríquez (7610, Alavés, day 68),
        // moved to the Thursday of the new week.
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "A", "week-start-talks", |records| {
            let b = records.block_mut("0x1c54000").unwrap();
            let e = (0..9)
                .map(|k| 4 + k * 0x4c)
                .find(|&e| b[e + 6..e + 8] == 7610u16.to_le_bytes())
                .unwrap();
            b[e + 2..e + 4].copy_from_slice(&74i16.to_le_bytes());
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let week = "on 89 press 1875\non 427 ok\non 218 click bar 0\non 147 click bar 18\non 302 click bar 18\n\
                    on 384 press 4741\non 308 choose 4095 0\non 96 ok\non 93 press 1943\non 499 ok\non 332 ok\non 269 ok\non 2 wait\non 381 wait\non 270 ok\non 417 ok\non 41 ok\n\
                    until dialog 25";
        let runner = on_save(&game, "week-start-talks", week).unwrap();
        let list = control(&game.dialogs[&25], 1313).unwrap();
        let keys: Vec<u16> =
            runner.host.view().list(list).unwrap().keys.iter().map(|&k| k as u16).collect();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        assert_eq!(records.globals.day(), 70, "{}", runner.log.join("\n"));
        assert_eq!(records.globals.week_start(), 70);
        let union = a3_core::statistics::human_clubs(records)[0];
        let listed = transfers::listings(records);
        let talks = transfers::appointments(records);
        let row = keys
            .iter()
            .position(|&id| {
                let talk = talks.iter().find(|t| t.player == id).unwrap();
                listed.iter().any(|e| e.id == id && e.kind == transfers::SALE)
                    && (70..84).contains(&talk.day)
                    && !talk.clubs.contains(&union)
                    && records.planners[0].day(talk.day as usize).kind == 0
            })
            .unwrap_or_else(|| panic!("{talks:?} {keys:?}"));
        let id = keys[row];
        let day = transfers::talk_day(records, id).unwrap();
        drop(state);
        let join = format!("{week}\nnow row 1313 {row}\nuntil dialog 376");
        let runner = on_save(&game, "week-start-talks", &join).unwrap();
        let boxes = |id| control(&game.dialogs[&376], id).unwrap();
        let view = runner.host.view();
        let first = view.check(boxes(4716)).unwrap();
        assert_eq!((first.text.as_str(), first.checked), ("1.FC Union Berlin", true));
        assert!(view.controls[boxes(4717)].hidden);
        let ok = format!("{join}\nnow press 4721\nuntil dialog 25");
        let runner = on_save(&game, "week-start-talks", &ok).unwrap();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let talk = transfers::appointments(records).into_iter().find(|t| t.player == id).unwrap();
        assert_eq!(talk.clubs[2], union, "{talk:?}");
        let booked = records.planners[0].day(day as usize);
        assert_eq!((booked.kind, booked.player), (0x0e, id));
        let view = runner.host.view();
        let rows = view.list(list).unwrap();
        let shown = rows.keys.iter().position(|&k| k as u16 == id).unwrap();
        assert_eq!(rows.inks[shown], Some([255, 0, 0]));
        assert_eq!(rows.selected, None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_poaching_talk_opens_on_its_day() {
        use a3_core::transfers;
        let game = game();
        let Some(mut runner) = on_save(&game, "Probe1", "") else { return };
        let player = {
            let mut state = runner.host.state();
            let records = state.records.as_mut().unwrap();
            let manager = records.humans().next().unwrap();
            let own = (manager.country() as u8, manager.team() as i16);
            let player = (1..0x4269)
                .find(|&p| {
                    records.player(p).is_some_and(|r| {
                        let c = r.club();
                        c[1] != 0 && (c[0], i16::from(c[1])) != own && r.salary() > 0
                    }) && transfers::talk_day(records, p).is_none()
                })
                .unwrap();
            let his = records.player(player).map(|r| r.club()).unwrap();
            let day = records.globals.day() as i16 + 1;
            assert!(transfers::book(
                records,
                transfers::POACHING,
                day,
                player,
                (his[0], i16::from(his[1])),
                own,
                [0; transfers::TERMS],
            ));
            player
        };
        let then = "on 302 click bar 18\non 384 press 4741\non 2 wait\nnow click bar 18\n\
                    until dialog 319";
        runner.run(&Script::parse(then).unwrap()).unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let view = runner.host.view();
        let dialog = &game.dialogs[&319];
        let text = |id| view.text(crate::screens::control(dialog, id).unwrap()).to_owned();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let r = records.player(player).unwrap();
        // His contract, and the offer with it but the signing fee.
        let money = |v: i64| crate::screens::playerinfo::money(&game, v);
        assert_eq!(text(4126), money(i64::from(r.salary()) * 1000));
        assert_eq!(text(4130), text(4126));
        assert_eq!(text(4131), text(4127));
        assert_eq!(text(6007), money(0));
        assert_eq!(text(4139), "Laufzeit");
        assert!(text(4151).contains("Alter: "), "{}", text(4151));
        assert!(!text(4149).is_empty());
        let salary = r.salary();
        drop(state);
        // The offer's spins step on the game's tables, its toggles flip:
        // salary one step up, the signing fee to 2 (1.000s), "Stammspieler"
        // and the league.
        let edit = "now spin 4133 up\nnow spin 6008 up\nnow spin 6008 up\nnow press 4144\nnow press 6010\n\
                    now spin 4142 up";
        runner.run(&Script::parse(edit).unwrap()).unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let view = runner.host.view();
        let text = |id| view.text(crate::screens::control(dialog, id).unwrap()).to_owned();
        let step = match salary { ..50 => 1, 50..100 => 2, 100..300 => 5, 300..1000 => 10, 1000..3000 => 20, 3000..6000 => 50, 6000..10000 => 100, _ => 200 };
        let up = (i64::from(salary) / step * step + step).min(20000);
        assert_eq!(text(4130), money(up * 1000));
        assert_eq!(text(6007), money(2000));
        assert_ne!(text(4144), text(4143));
        assert_ne!(text(6010), game.text(4086));
        // "Abbrechen" ends the talk without an agreement.
        let cancel = "now press 4153\nuntil dialog 381";
        runner.run(&Script::parse(cancel).unwrap()).unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_poaching_talk_is_haggled_till_he_agrees_or_breaks_off() {
        use a3_core::transfers;
        let game = game();
        let dialog = &game.dialogs[&319];
        // Once with the salary and bonuses cut to nothing, once on the
        // offer as it stands, which a well paid player takes at once.
        for cut in [true, false] {
            let Some(mut runner) = on_save(&game, "Probe1", "") else { return };
            let player = {
                let mut state = runner.host.state();
                let records = state.records.as_mut().unwrap();
                let manager = records.humans().next().unwrap();
                let own = (manager.country() as u8, manager.team() as i16);
                let player = (1..0x4269)
                    .find(|&p| {
                        records.player(p).is_some_and(|r| {
                            let c = r.club();
                            c[1] != 0
                                && (c[0], i16::from(c[1])) != own
                                && r.salary() > 0
                                && r.strength() >= 8
                                && game.world.slot_of(c[0].into()).is_some()
                        }) && transfers::talk_day(records, p).is_none()
                    })
                    .unwrap();
                let his = records.player(player).map(|r| r.club()).unwrap();
                let day = records.globals.day() as i16 + 1;
                assert!(transfers::book(records, transfers::POACHING, day, player, (his[0], i16::from(his[1])), own, [0; transfers::TERMS]));
                player
            };
            let mut then = "on 302 click bar 18\non 384 press 4741\non 2 wait\nnow click bar 18\nuntil dialog 319".to_owned();
            for _ in 0..if cut { 300 } else { 0 } {
                then.push_str("\nnow spin 4133 down\nnow spin 4134 down\nnow spin 4135 down");
            }
            runner.run(&Script::parse(&then).unwrap()).unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
            let face = |runner: &Runner| {
                let view = runner.host.view();
                match &view.controls[crate::screens::control(dialog, 4152).unwrap()].state {
                    crate::controls::ControlState::Anim(a) => (a.frames.first().copied(), a.frame),
                    _ => (None, 0),
                }
            };
            assert_eq!(face(&runner), (Some(1680), 0));
            let expected = {
                let state = runner.host.state();
                let records = state.records.as_ref().unwrap();
                crate::screens::poaching::offer(transfers::terms(records, player), records.globals.day() as i16)
            };
            let mut presses = 0;
            while runner.dialog() == Some(319) {
                runner.run(&Script::parse("now press 4154").unwrap()).unwrap();
                presses += 1;
                assert!(presses <= 7);
                if runner.dialog() == Some(319) {
                    assert!(face(&runner).1 > 0);
                }
            }
            assert_eq!(runner.dialog(), Some(380));
            let name = runner.host.state().records.as_ref().unwrap().known_as(player).unwrap();
            let text = runner.host.view().text(crate::screens::control(&game.dialogs[&380], 4730).unwrap()).to_owned();
            assert!(text.contains(&name), "{text}");
            let agreed = !text.contains(game.text(0x104a).split("%s").nth(1).unwrap().trim());
            assert_eq!(agreed, !cut, "{text} after {presses}");
            assert!(!text.contains("v2"), "{text}");
            // The talk ends; the day goes on.
            let on = "now press 4731\nuntil dialog 381";
            runner.run(&Script::parse(on).unwrap()).unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
            if agreed {
                // The offer agreed is the talk's contract for the fee talk.
                let state = runner.host.state();
                let all = transfers::appointments(state.records.as_ref().unwrap());
                let fee = all.iter().find(|t| t.player == player && t.kind == transfers::PURCHASE).expect("no fee talk");
                assert_eq!(fee.terms, expected);
                assert_ne!(fee.terms, [0; transfers::TERMS]);
            }
            if !agreed {
                let state = runner.host.state();
                let records = state.records.as_ref().unwrap();
                let r = records.player(player).unwrap();
                let left = i32::from(r.contract() & 0xfff) > i32::from(records.globals.start_year());
                assert_eq!(r.flags() & 0x2000 != 0, left);
            }
        }
    }

    /// Hot seat: A with a second human under contract at the next club
    /// plays its whole season and on into the next, each manager ending
    /// his turns with the whistle and pressing his season-end statement
    /// away; the same turns on the same save replay identically — the
    /// records and the season alike (PLAN P8's gate).
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_two_manager_season_replays_identically() {
        // The season end's statement, one for each human (the first is
        // pressed away above).
        const TURNS: usize = 1;
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        if !dir.parent().unwrap().join("saves-kept/A.an3").exists() && !dir.join("saves/A.an3").exists() {
            eprintln!("no A among the saves");
            return;
        }
        let root = patched(&dir, "A", "hot-seat", |records| {
            records.globals.set_slots([1, 1, 0, 0]);
            let team = records.managers[0].team();
            records.managers[1].set_team(team + 1);
            let (status, country) = (records.managers[0].status(), records.managers[0].country());
            records.managers[1].set_status(status);
            records.managers[1].set_country(country);
            records.managers[1].set_human(2);
            let club = records.team_mut(country, team + 1).unwrap();
            club.set_humans([2, 2]);
            club.set_human(2);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let handlers = "on 89 press 1875\non 422 ok\non 420 ok\non 421 ok\non 287 ok\non 321 ok\non 25 ok\non 341 ok\n\
                        on 364 ok\non 120 ok\non 51 ok\non 392 ok\non 435 ok\non 96 ok\non 161 ok\non 419 ok\n\
                        on 477 ok\non 380 press 4731\non 384 press 4741\non 218 click bar 0\non 147 click bar 18\n\
                        on 302 click bar 18\non 308 choose 4095 0\non 332 ok\non 269 ok\non 93 press 1943\non 499 ok\non 41 ok\non 383 click at 460 210\n\
                        on 99 press 2\non 307 click at 320 240\non 310 click at 320 240\non 331 press 4303\n\
                        on 65 click at 165 330\non 423 ok\non 1 click at 320 357\n\
                        on 375 click at 617 182; press 4715\non 461 ok\non 2 wait\non 381 wait\non 270 ok\n\
                        on 356 press 4542\non 379 ok\non 314 ok\n";
        let turns = "until dialog 417\nnow press 1\n".repeat(TURNS);
        let turns = "until dialog 417\nnow press 1\n".repeat(TURNS);
        let then = format!(
            "{handlers}now click bar 9\nuntil dialog 417\nnow click bar 18\nuntil dialog 417\n\
             now press 1\n{turns}until dialog 422\nuntil day 30"
        );
        let play = || {
            let mut runner = on_save(&game, "hot-seat", &then).unwrap();
            let _ = runner.settle();
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let bytes = records.to_save().to_bytes_stored().unwrap();
            let season = state.season.as_ref().unwrap();
            let summary =
                (season.year, season.day, season.managers.len(), records.globals.seasons(), season.clubs.len());
            (bytes, summary, runner.log.clone())
        };
        let (first, summary, log) = play();
        eprintln!("{summary:?}");
        let (second, again, _) = play();
        assert_eq!(summary.2, 2, "{}", log.join("\n"));
        assert_eq!(summary.1, 30, "{}", log.join("\n"));
        assert_eq!(summary.4, 2, "both humans' clubs followed");
        assert_eq!(summary, again);
        assert!(first == second, "the replay's records differ");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Probe1 with a second human at the next club: the doping screen's
    /// report button opens dialog 509 with one box, his name on it, the
    /// other two disabled; ticking it reports him, ticking again takes it
    /// back. With three reports followed up the button only alerts (5002).
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn doping_reports_another_human() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        if !dir.parent().unwrap().join("saves-kept/Probe1.an3").exists()
            && !dir.join("saves/Probe1.an3").exists()
        {
            eprintln!("no Probe1 among the saves");
            return;
        }
        let second = |reports: i16| {
            move |records: &mut a3_core::records::Records| {
                records.globals.set_slots([1, 1, 0, 0]);
                let team = records.managers[0].team();
                records.managers[1].set_team(team + 1);
                let (status, country) = (records.managers[0].status(), records.managers[0].country());
                records.managers[1].set_status(status);
                records.managers[1].set_country(country);
                records.managers[1].set_human(2);
                records.managers[0].set_doping_reports(reports);
                let club = records.team_mut(country, team + 1).unwrap();
                club.set_humans([2, 2]);
                club.set_human(2);
            }
        };
        let root = patched(&dir, "Probe1", "doping-two", second(0));
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let open = "now choose 5266 6\nuntil dialog 151\nnow press 4929\nuntil dialog 394\n\
                    now press 5485\nuntil dialog 509";
        let runner = on_save(&game, "doping-two", open).unwrap();
        let dialog = &game.dialogs[&509];
        let view = runner.host.view();
        let name = {
            let state = runner.host.state();
            let m = &state.records.as_ref().unwrap().managers[1];
            format!("{} {}", m.first_name(), m.last_name())
        };
        let first = view.options(control(dialog, 6421).unwrap()).unwrap();
        assert_eq!(first.labels[0], name);
        assert_eq!(first.selected(), None);
        for id in [6422, 6423] {
            assert_eq!(view.options(control(dialog, id).unwrap()).unwrap().dimmed, vec![true]);
        }
        drop(runner);
        let tick = format!("{open}\nnow choose 6421 0");
        let runner = on_save(&game, "doping-two", &tick).unwrap();
        assert!(runner.host.state().season.as_ref().unwrap().reported[0][1]);
        drop(runner);
        let again = format!("{tick}\nnow choose 6421 0\nnow press 1\nuntil dialog 394\nnow press 5485\nuntil dialog 509");
        let runner = on_save(&game, "doping-two", &again).unwrap();
        assert!(!runner.host.state().season.as_ref().unwrap().reported[0][1]);
        drop(runner);
        let _ = std::fs::remove_dir_all(&root);
        let root = patched(&dir, "Probe1", "doping-three", second(3));
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let refused = "now choose 5266 6\nuntil dialog 151\nnow press 4929\nuntil dialog 394\n\
                       now press 5485\nuntil dialog 380";
        let runner = on_save(&game, "doping-three", refused).unwrap();
        assert_eq!(notice_text(&game, &runner), game.text(5002));
        drop(runner);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// "Neueinstieg": a second human gives his name and
    /// birthday in the dressing room, and the week's start takes him in,
    /// three levels below the first, with offers to sign; his own turn
    /// follows the first's.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_new_human_joins_at_the_weeks_start() {
        let game = game();
        let ask = "now click bar 17\nuntil dialog 302\nnow choose 4035 0\nuntil dialog 212\n\
                   now type 3121 Carl; type 3122 Gamma; ok\nuntil dialog 491\nnow ok\nuntil dialog 302";
        let Some(runner) = on_save(&game, "Toni", ask) else { return };
        {
            let state = runner.host.state();
            assert_eq!(state.joining.len(), 1);
            assert_eq!(state.joining[0].last_name, "Gamma");
            assert_eq!(state.season.as_ref().unwrap().managers.len(), 1, "not before the week");
        }
        drop(runner);
        let week = format!(
            "{ask}\non 147 click bar 18\non 302 click bar 18\non 384 press 4741\non 2 wait\n\
             on 461 ok\non 270 ok\non 381 wait\non 96 ok\non 93 press 1943\non 499 ok\non 41 ok\n\
             on 332 ok\non 269 ok\non 383 click at 460 210\nuntil dialog 99"
        );
        let runner = on_save(&game, "Toni", &week).unwrap();
        let first_level = {
            let state = runner.host.state();
            let season = state.season.as_ref().unwrap();
            assert_eq!(season.managers.len(), 2, "{}", runner.log.join("\n"));
            let newcomer = &season.managers[season.current];
            assert_eq!(newcomer.last_name, "Gamma");
            assert!(!newcomer.offers.is_empty());
            let records = state.records.as_ref().unwrap();
            assert_eq!(records.globals.humans(), 2);
            let slot = records.human_slot(season.current).unwrap();
            assert_eq!(records.managers[slot].last_name(), "Gamma");
            assert_eq!(i32::from(records.managers[slot].level()), newcomer.level);
            season.managers[1 - season.current].level
        };
        assert_eq!(runner.host.state().season.as_ref().unwrap().managers[1].level, (first_level - 3).clamp(2, 15));
        drop(runner);
        // He signs with the first club: his turn comes after the first's.
        let signed = format!(
            "{week}\nnow choose 2063 0\non 103 choose 2107 1\non 104 press 2115\nuntil dialog 147\n\
             now click bar 18\non 302 click bar 18\non 384 press 4741\nuntil dialog 147"
        );
        let runner = on_save(&game, "Toni", &signed).unwrap();
        let state = runner.host.state();
        let season = state.season.as_ref().unwrap();
        assert_eq!(season.current, 1, "{}", runner.log.join("\n"));
        let club = season.managers[1].club.expect("he signed");
        assert!(season.clubs.contains_key(&club));
        let records = state.records.as_ref().unwrap();
        assert_eq!(records.managers[records.human_slot(1).unwrap()].status(), 3);
    }

    /// Nsoki's fee talk on A, day 64, as the original holds it at its
    /// first dialog 308 (seeds poked before the week): 59/3 offers 1 868
    /// thousands, shown at its step (1.85 Mio), Union asks 10 350, shown
    /// at 10 Mio; holding to it five rounds, the bidder runs out of
    /// patience and Nsoki stays.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_seller_talks_over_the_fee_in_dialog_308() {
        let game = game();
        let ways = "now random 12345 67890\non 422 ok\non 420 ok\non 421 ok\non 287 ok\non 321 ok\non 25 ok\n\
                    on 341 ok\non 120 ok\non 51 ok\non 392 ok\non 435 ok\non 96 ok\non 93 press 1943\non 499 ok\non 161 ok\non 422 ok\n\
                    on 419 ok\non 419 ok\non 477 ok\non 380 press 4731\non 381 wait\non 2 wait\n\
                    on 147 click bar 18\non 302 click bar 18\non 384 press 4741\non 270 ok";
        let Some(runner) = on_save(&game, "A", &format!("{ways}\nuntil dialog 308")) else { return };
        let text = |id: u16| runner.host.view().text(control(&game.dialogs[&308], id).unwrap()).to_owned();
        assert_eq!(text(4070), "Stanley Nsoki");
        assert_eq!(text(4078), "1.FC Union Berlin");
        assert_eq!(text(4086), "1.85 Mio €");
        assert_eq!(text(4088), "10.00 Mio €");
        {
            let state = runner.host.state();
            let fee = state.season.as_ref().unwrap().fee_talk.clone().unwrap();
            assert_eq!((fee.buyer, fee.offer, fee.demand, fee.most), ((59, 3), 1868, 10_350, 2459));
        }
        drop(runner);
        let runner = on_save(&game, "A", &format!("{ways}\non 308 choose 4095 0\nuntil day 65")).unwrap();
        assert_eq!(runner.log.iter().filter(|l| l.as_str() == "screen 308").count(), 5, "{}", runner.log.join("\n"));
        let state = runner.host.state();
        assert!(state.season.as_ref().unwrap().fee_talk.is_none());
        let nsoki = state.records.as_ref().unwrap().player(3262).unwrap();
        assert_eq!((nsoki.club()[0], nsoki.club()[1]), (14, 84));
    }

    /// Young's purchase talk on A, day 64, computer clubs only (seeds
    /// poked before the week, the fee talk before it held to its asking):
    /// as the original's records stand after it, 14/20 has taken him from
    /// 14/43 at once for 548 000 — a weaker man by the step, on fresh
    /// terms of 60 and 3 till 2029 with his option, moved in the season
    /// and out of the European cups, with the number 25, a second station
    /// from 8 September, off the transfer list; the fee stands in both
    /// clubs' books.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn computer_clubs_settle_a_purchase_talk_and_the_player_moves() {
        let game = game();
        let ways = "now random 12345 67890\non 422 ok\non 420 ok\non 421 ok\non 287 ok\non 321 ok\non 25 ok\n\
                    on 341 ok\non 120 ok\non 51 ok\non 392 ok\non 435 ok\non 96 ok\non 93 press 1943\non 499 ok\non 161 ok\non 422 ok\n\
                    on 419 ok\non 419 ok\non 477 ok\non 380 press 4731\non 381 wait\non 2 wait\n\
                    on 147 click bar 18\non 302 click bar 18\non 384 press 4741\non 270 ok";
        let Some(before) = on_save(&game, "A", "") else { return };
        let runner = on_save(&game, "A", &format!("{ways}\non 308 choose 4095 0\nuntil day 65")).unwrap();
        const YOUNG: u16 = 2168;
        let books = |runner: &Runner, number: u16| {
            let state = runner.host.state();
            let team = state.records.as_ref().unwrap().team(14, number).unwrap();
            // Net, sales and purchases (club +0x258, +0x470, +0x478).
            (team.players().len(), team.get::<i32>(0x4d0), team.get::<i64>(0x6e8), team.get::<i64>(0x6f0))
        };
        let (old, new) = (books(&before, 43), books(&before, 20));
        assert_eq!(books(&runner, 43), (old.0 - 1, old.1 + 548_000, old.2 + 548_000, old.3));
        assert_eq!(books(&runner, 20), (new.0 + 1, new.1 - 548_000, new.2, new.3 + 548_000));
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let p = records.player(YOUNG).unwrap();
        assert_eq!((p.club(), p.strength(), p.shirt()), ([14, 20], 4, 25));
        assert_eq!((p.salary(), p.appearance_bonus(), p.goal_bonus(), p.fixed_fee()), (60, 3, 0, 0));
        assert_eq!((p.contract(), p.terms(), p.get::<u32>(0x158), p.get::<u8>(0x161)), (2029, 0x1810, 548, 0));
        assert_eq!((p.retiring(), p.intensity(), p.get::<i16>(0x1f4)), (0, 3, 0));
        assert_eq!(p.career_count(), 2);
        let station: [u8; 0x14] = p.get(0x34);
        assert_eq!(station, [9, 9, 0xe9, 7, 0, 0, 0, 0, 0x24, 2, 0, 0, 0, 0, 0, 0, 14, 20, 1, 4]);
        assert!(records.team(14, 20).unwrap().players().contains(&YOUNG));
        assert!(!records.team(14, 43).unwrap().players().contains(&YOUNG));
        assert!(!a3_core::transfers::listings(records).iter().any(|l| l.id == YOUNG));
        // Both squads' best by position, their best sixteen's mean and
        // the sixteenth (team +0x322), as the original left them.
        let best = |number: u16| records.team(14, number).unwrap().get::<[u8; 16]>(0x322);
        assert_eq!(best(43)[12..], [67, 0, 62, 0]);
        assert_eq!(best(20)[12..], [58, 0, 50, 0]);
    }

    /// The same move on A, day 64: the search plans (team +0x34c) as the
    /// original's records hold them after it — 14/20 has paid the fee and
    /// a thousand times the salary out of both budgets and wants a libero
    /// and a stopper at once and a libero more, all at 6; 14/43 has half
    /// the fee more and wants a libero for nobody, a keeper short by 2 and
    /// a forward short by 1, at 7.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn computer_clubs_plan_their_search_anew_after_a_transfer() {
        let game = game();
        let ways = "now random 12345 67890\non 422 ok\non 420 ok\non 421 ok\non 287 ok\non 321 ok\non 25 ok\n\
                    on 341 ok\non 120 ok\non 51 ok\non 392 ok\non 435 ok\non 96 ok\non 93 press 1943\non 499 ok\non 161 ok\non 422 ok\n\
                    on 419 ok\non 419 ok\non 477 ok\non 380 press 4731\non 381 wait\non 2 wait\n\
                    on 147 click bar 18\non 302 click bar 18\non 384 press 4741\non 270 ok";
        let Some(runner) = on_save(&game, "A", &format!("{ways}\non 308 choose 4095 0\nuntil day 65")) else { return };
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let plan = |number: u16| records.team(14, number).unwrap().get::<[u8; 0x28]>(0x34c);
        let budgets = |b: [u8; 0x28]| (i32::from_le_bytes(b[0..4].try_into().unwrap()), i32::from_le_bytes(b[4..8].try_into().unwrap()));
        assert_eq!(budgets(plan(20)), (1_892_000, -608_000));
        assert_eq!(plan(20)[0x12..0x26], [3, 0, 2, 6, 5, 0, 1, 2, 3, 6, 5, 0, 1, 2, 2, 6, 3, 0, 1, 1]);
        assert_eq!(budgets(plan(43)), (1_922_000, -578_000));
        assert_eq!(plan(43)[0x12..0x26], [3, 0, 2, 7, 3, 0, 1, 1, 1, 7, 2, 0, 1, 1, 10, 7, 1, 0, 1, 1]);
    }

    /// A's switch for the tables stands at 1 ("an"): the matchday of day
    /// 69 played, dialog 269 opens on the one league with a human's club,
    /// the 1. Bundesliga — its headline, the day's nine games two to a
    /// line, the eighteen clubs with their home and away figures, Union
    /// red, each place's box in its colour — and its OK closes it after
    /// that league; the walk goes on to the next turn.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_table_after_the_matchday_shows_the_human_league() {
        let game = game();
        let ways = "on 89 press 1875\non 422 ok\non 420 ok\non 421 ok\non 287 ok\non 321 ok\non 25 ok\non 341 ok\non 120 ok\n\
                    on 51 ok\non 392 ok\non 435 ok\non 419 ok\non 380 press 4731\non 381 wait\non 2 wait\n\
                    on 147 click bar 18\non 302 click bar 18\non 384 press 4741\non 270 ok\non 461 ok\n\
                    on 308 choose 4095 0\non 96 ok\non 93 press 1943\non 499 ok\non 41 ok\non 332 ok\non 383 click at 460 210";
        let Some(runner) = on_save(&game, "A", &format!("{ways}\nuntil dialog 269")) else { return };
        let dialog = &game.dialogs[&269];
        let view = runner.host.view();
        let id = |id: u16| control(dialog, id).unwrap();
        assert_eq!(view.header(id(3727)).unwrap().text, "1. Bundesliga: Der 4. Spieltag im Überblick");
        assert!(view.header(id(3730)).unwrap().text.starts_with(r"Pos\tVerein\tSpiele\tGesamt\t"));
        let results = view.list(id(3728)).unwrap();
        assert_eq!(results.rows.len(), 5);
        let first = results.rows[0].split('\t').collect::<Vec<_>>();
        assert_eq!(first.len(), 12, "{:?}", results.rows[0]);
        assert!(first[1].starts_with("- ") && first[3].starts_with(':') && first[4].starts_with('('));
        assert!(results.rows[4].ends_with('\t'), "the ninth game stands alone: {:?}", results.rows[4]);
        let table = view.list(id(3729)).unwrap();
        assert_eq!(table.rows.len(), 18);
        assert_eq!(table.tabs.len(), 19);
        assert!(table.rows.iter().all(|r| r.split('\t').count() == 19));
        let union = table.rows.iter().position(|r| r.contains("Union Berlin")).unwrap();
        assert_eq!(table.inks[union], Some([255, 0, 0]));
        assert_eq!(table.inks.iter().filter(|i| i.is_some()).count(), 1);
        assert!(table.rows[0].starts_with("1.\t("));
        let bar = |row: usize| table.bars[row].map(|b| (b.left, b.width, b.fill.0));
        assert_eq!(bar(0), Some((5, 15, [255, 255, 0, 255])));
        assert_eq!(bar(17).map(|b| b.2), Some([255, 160, 160, 255]));
        drop(runner);
        let runner = on_save(&game, "A", &format!("{ways}\non 332 ok\non 269 ok\nuntil day 70")).unwrap();
        assert_eq!(seen(&runner, 269), 1, "{}", runner.log.join("\n"));
    }

    /// A's interviews are on: walking on from A, on the matchday the
    /// roll draws one for Union, dialog 383 opens behind the table on
    /// its first question — the reporter's words above, three answers to
    /// click, no OK. Clicking the first answer's place through it, as
    /// Toni's walk does, plays it to the reporter's thanks and closes it;
    /// the walk goes on, the reporter's mood kept in the station's record.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_interview_after_the_match_opens_and_is_answered() {
        let game = game();
        if !game.save_games().iter().any(|s| s.name == "A") {
            eprintln!("no A among the saves; the oracle's copy has it");
            return;
        }
        let ways = "on 89 press 1875\non 422 ok\non 420 ok\non 421 ok\non 287 ok\non 321 ok\non 25 ok\non 341 ok\non 120 ok\n\
                    on 51 ok\non 392 ok\non 435 ok\non 419 ok\non 380 press 4731\non 381 wait\non 2 wait\n\
                    on 147 click bar 18\non 302 click bar 18\non 384 press 4741\non 270 ok\non 461 ok\n\
                    on 308 choose 4095 0\non 96 ok\non 93 press 1943\non 499 ok\non 332 ok\non 269 ok\non 161 ok";
        let row = game.save_games().iter().position(|s| s.name == "A").unwrap();
        let load = LOAD.replace("row 5144 0", &format!("row 5144 {row}"));
        let play = |then: &str| Runner::play(&game, &Script::parse(&format!("{}\n{then}", load.trim_end())).unwrap());
        // The roll is the game's: the first of a few seeds whose walk
        // comes to an interview before the season's end.
        let (seed, runner) = (1..20)
            .find_map(|s| {
                let (runner, result) = play(&format!("now random {s} {}\n{ways}\nuntil dialog 383", s * 7 + 3));
                result.is_ok().then_some((s, runner))
            })
            .expect("an interview within the seeds");
        let dialog = &game.dialogs[&383];
        let id = |id: u16| control(dialog, id).unwrap();
        let day = {
            let state = runner.host.state();
            let season = state.season.as_ref().unwrap();
            let interview = season.interview.as_ref().expect("the interview runs");
            let a3_core::interview::Page::Question { text, answers } = &interview.page else {
                panic!("{:?}", interview.page)
            };
            let view = runner.host.view();
            assert_eq!(view.text(id(3610)), text);
            assert!(!text.is_empty());
            for (k, answer) in answers.iter().enumerate() {
                assert_eq!(view.text(id([4735, 4736, 4737][k])), answer);
                assert!(!answer.is_empty());
            }
            assert!(view.controls[id(1)].hidden, "OK waits for the thanks");
            season.day
        };
        drop(runner);
        let (runner, result) = play(&format!("now random {seed} {}\n{ways}\non 383 click at 460 210\nuntil day {}", seed * 7 + 3, day + 1));
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        assert_eq!(seen(&runner, 383), 1, "{}", runner.log.join("\n"));
        let state = runner.host.state();
        assert!(state.season.as_ref().unwrap().interview.is_none());
        let recent = state.season.as_ref().unwrap().interview_recent;
        assert!((1..=4).contains(&recent.iter().filter(|&&q| q != 0).count()), "{recent:?}");
    }

    /// A's Spielbericht switch is on: walking on from A, Union's 4th
    /// matchday writes its report and dialog 332 opens on it behind the
    /// match, before the table: both clubs, the league's matchday, the
    /// score the league block holds with the half's, one goal row for
    /// every goal with the running score, the crowd and the gate as the
    /// home club booked them, the sixty minutes of a match without
    /// stoppages; no other human club, so no crest below. OK goes on to
    /// the table.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_match_report_opens_after_the_match() {
        let game = game();
        let ways = "on 89 press 1875\non 422 ok\non 420 ok\non 421 ok\non 287 ok\non 321 ok\non 25 ok\non 341 ok\non 120 ok\n\
                    on 51 ok\non 392 ok\non 435 ok\non 419 ok\non 380 press 4731\non 381 wait\non 2 wait\n\
                    on 147 click bar 18\non 302 click bar 18\non 384 press 4741\non 270 ok\non 461 ok\n\
                    on 308 choose 4095 0\non 96 ok\non 93 press 1943\non 499 ok\non 41 ok";
        let Some(runner) = on_save(&game, "A", &format!("{ways}\nuntil dialog 332")) else { return };
        assert_eq!(seen(&runner, 269), 0, "the report comes before the table");
        let dialog = &game.dialogs[&332];
        let view = runner.host.view();
        let id = |id: u16| control(dialog, id).unwrap();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let union = records.team(14, 84).unwrap();
        let report = a3_core::statistics::match_report(records, union).expect("Union's report");
        let [(hc, hn), (ac, an)] = report.sides();
        assert!((hc, hn) == (14, 84) || (ac, an) == (14, 84), "{:?}", report.sides());
        assert_eq!(report.competition(), 3, "the 4th matchday, from 0");
        let league = a3_core::opponent::League::of(records, 14, union.level()).unwrap();
        let game_of = league.games(3).into_iter().find(|&(h, a, _)| {
            [league.team(h), league.team(a)].contains(&84)
        });
        let [gh, ga, hh, ha] = game_of.and_then(|g| g.2).expect("the match played");
        assert_eq!(report.goals(), [i16::from(gh), i16::from(ga)]);
        assert_eq!(view.text(id(4313)), format!("{gh}:{ga} ({hh}:{ha})"));
        assert_eq!(view.text(id(4309)), game.club_name(Some(records), hc, hn));
        assert_eq!(view.text(id(4311)), game.club_name(Some(records), ac, an));
        assert_eq!(view.text(id(4307)), "1. Bundesliga - 4. Spieltag");
        assert!(view.text(id(4312)).ends_with(". Platz"), "{:?}", view.text(id(4312)));
        let goals = view.list(id(4335)).unwrap();
        if gh + ga == 0 {
            assert_eq!(goals.rows, ["Fehlanzeige"]);
        } else {
            assert_eq!(goals.rows.len(), usize::from(gh + ga), "{:?}", goals.rows);
            let last = goals.rows.last().unwrap().split('\t').collect::<Vec<_>>();
            assert_eq!((last[0], last[2]), (gh.to_string().as_str(), ga.to_string().as_str()), "{last:?}");
        }
        assert_eq!(view.list(id(4337)).unwrap().rows, ["keine"]);
        assert!(report.attendance() > 0 && report.income() > 0);
        assert_eq!(view.text(id(4332)), "60 Minuten");
        assert!(view.text(id(4330)).starts_with("SR "));
        for k in [4339, 4340, 4341, 4342, 4343, 4344] {
            assert!(view.controls[id(k)].hidden, "{k}");
        }
        drop(state);
        drop(runner);
        let runner = on_save(&game, "A", &format!("{ways}\non 332 ok\nuntil dialog 269")).unwrap();
        assert_eq!(seen(&runner, 332), 1, "{}", runner.log.join("\n"));
    }

    /// Walking on from A, Union's 4th matchday is a human's match after
    /// the season's 42nd day: before its report dialog 96 opens on one
    /// page in the middle of the one-page background — the league and
    /// matchday, both clubs, their places, the strengths, Union's ground
    /// with the played match's crowd, its visiting fans and pitch — the
    /// other places' pages hidden with the display's combo. OK goes on to
    /// the report.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_last_information_shows_before_the_match() {
        let game = game();
        let ways = "on 89 press 1875\non 422 ok\non 420 ok\non 421 ok\non 287 ok\non 321 ok\non 25 ok\non 341 ok\non 120 ok\n\
                    on 51 ok\non 392 ok\non 435 ok\non 419 ok\non 380 press 4731\non 381 wait\non 2 wait\n\
                    on 147 click bar 18\non 302 click bar 18\non 384 press 4741\non 270 ok\non 461 ok\n\
                    on 308 choose 4095 0\non 41 ok";
        let Some(runner) = on_save(&game, "A", &format!("{ways}\nuntil dialog 96")) else { return };
        assert_eq!(seen(&runner, 332), 0, "the information comes before the report");
        let dialog = &game.dialogs[&96];
        let page = &game.dialogs[&98];
        let view = runner.host.view();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let season = state.season.as_ref().unwrap();
        assert_eq!(season.prematch.len(), 1, "{:?}", season.prematch);
        let shown = &season.prematch[0];
        assert!(shown.sides.contains(&(14, 84)), "{:?}", shown.sides);
        // The one-page slot is the first of the ten.
        let per = page.controls.len();
        let first = dialog.controls.iter().position(|c| c.id == Some(2025)).unwrap();
        let at = |id: u16| first + page.controls.iter().position(|c| c.id == Some(id)).unwrap();
        assert!(view.controls[first..first + per].iter().all(|c| !c.hidden));
        assert!(view.controls[first + per..first + 10 * per].iter().all(|c| c.hidden));
        assert_eq!(dialog.controls[at(2025)].rect.min, bevy_math::Vec2::new(174.0, 144.0));
        assert!(view.text(at(6424)).starts_with("1. Bundesliga "), "{:?}", view.text(at(6424)));
        assert!(view.text(at(6424)).ends_with(" - 4. Spieltag"), "{:?}", view.text(at(6424)));
        let [(hc, hn), (ac, an)] = shown.sides;
        assert_eq!(view.text(at(2027)), game.club_name(Some(records), hc, hn));
        assert_eq!(view.text(at(2032)), game.club_name(Some(records), ac, an));
        assert!(view.text(at(2029)).starts_with('(') && view.text(at(2029)).ends_with(".)"));
        assert_eq!(view.text(at(2028)).chars().count(), 3, "three matchdays played");
        let strengths = view.text(at(2036)).split(" : ").map(|s| s.parse::<i32>().unwrap()).collect::<Vec<_>>();
        assert_eq!(strengths.len(), 2);
        assert!(strengths.iter().all(|&s| s > 0), "{strengths:?}");
        let report = a3_core::statistics::match_report(records, records.team(14, 84).unwrap()).unwrap();
        assert_eq!(shown.attendance, report.attendance());
        let crowd = view.text(at(2037));
        assert!(crowd == report.attendance().to_string() || crowd.ends_with(" (A)"), "{crowd:?}");
        assert!(view.text(at(2038)).parse::<i32>().is_ok_and(|f| (0..=shown.attendance).contains(&f)));
        assert!(!view.text(at(6449)).is_empty());
        let pitches = [2203, 2204, 2205, 2206, 2207, 2208].map(|id| game.text(id).to_owned());
        assert!(pitches.contains(&view.text(at(2040)).to_owned()), "{:?}", view.text(at(2040)));
        let backdrop = dialog.controls.iter().position(|c| c.id == Some(9948)).unwrap();
        let picture = view.controls[backdrop].state.as_bitmap().unwrap().picture.clone();
        assert_eq!(picture, Some(crate::controls::bitmap::Picture::graphic(3097)));
        let combo = dialog.controls.iter().position(|c| c.id == Some(6479)).unwrap();
        assert!(view.controls[combo].hidden);
        drop(state);
        drop(runner);
        let runner = on_save(&game, "A", &format!("{ways}\non 96 ok\non 93 press 1943\non 499 ok\nuntil dialog 332")).unwrap();
        assert_eq!(seen(&runner, 96), 1, "{}", runner.log.join("\n"));
    }

    /// Walking on from A, Union's 4th matchday stops at half time behind
    /// the last information: dialog 93 opens on Union's match with both
    /// clubs, the half's score as the report keeps it and the 45th minute
    /// in its caption, the eleven in their boxes, the bench, three to come
    /// on and Union's break on its bar. Its OK opens the talk, its twelve
    /// lines under the club and the half's score; OK keeps the chosen
    /// line, and the report follows.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_match_stops_at_half_time_for_the_talk() {
        let game = game();
        let ways = "on 89 press 1875\non 422 ok\non 420 ok\non 421 ok\non 287 ok\non 321 ok\non 25 ok\non 341 ok\non 120 ok\n\
                    on 51 ok\non 392 ok\non 435 ok\non 419 ok\non 380 press 4731\non 381 wait\non 2 wait\n\
                    on 147 click bar 18\non 302 click bar 18\non 384 press 4741\non 270 ok\non 461 ok\n\
                    on 308 choose 4095 0\non 96 ok\non 41 ok";
        let Some(runner) = on_save(&game, "A", &format!("{ways}\nuntil dialog 93")) else { return };
        assert_eq!(seen(&runner, 96), 1, "the break comes behind the last information");
        assert_eq!(seen(&runner, 332), 0, "and before the report");
        let dialog = &game.dialogs[&93];
        let view = runner.host.view();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let season = state.season.as_ref().unwrap();
        assert_eq!(season.halftime.len(), 1, "{:?}", season.halftime);
        let shown = &season.halftime[0];
        assert_eq!(shown.club, (14, 84));
        let report = a3_core::statistics::match_report(records, records.team(14, 84).unwrap()).unwrap();
        assert_eq!(shown.goals, report.half());
        let at = |id: u16| dialog.controls.iter().position(|c| c.id == Some(id)).unwrap();
        let [(hc, hn), (ac, an)] = shown.sides;
        let (home, away) = (game.club_name(Some(records), hc, hn), game.club_name(Some(records), ac, an));
        let caption = view.text(at(9947));
        let score = format!("{home} - {away} {} : {} Chancen (0:0)", shown.goals[0], shown.goals[1]);
        assert!(caption.starts_with(&score), "{caption:?} {score:?}");
        assert!(caption.ends_with(", 45. Minute"), "{caption:?}");
        let boxes = (1916..1927).filter_map(|id| view.controls[at(id)].state.as_player()).filter(|b| b.filled).count();
        assert_eq!(boxes, 11, "the eleven on the pitch");
        assert!(view.controls[at(1927)].state.as_player().is_some_and(|b| b.filled), "the bench");
        assert_eq!(view.text(at(1942)), "Ausgewechselt: 3");
        for id in [2554, 2566] {
            let age = view.text(at(id)).parse::<f32>().unwrap();
            assert!((17.0..36.0).contains(&age), "the eleven's average age {age}");
        }
        let bar = view.controls[at(6269)].state.as_options().unwrap();
        let own = shown.own_goals();
        assert_eq!(bar.labels[0], format!("{} {}:{}", game.club_name(Some(records), 14, 84), own[0], own[1]));
        assert!(bar.off[1..].iter().all(|&off| off));
        drop(state);
        drop(runner);
        let Some(runner) = on_save(&game, "A", &format!("{ways}\non 93 press 1943\nuntil dialog 499")) else { return };
        let talk = &game.dialogs[&499];
        let view = runner.host.view();
        let at = |id: u16| talk.controls.iter().position(|c| c.id == Some(id)).unwrap();
        let union = game.club_name(runner.host.state().records.as_ref(), 14, 84);
        let caption = view.text(at(9947));
        assert!(caption.starts_with(&format!("ANSTOSS 3 - Halbzeitansprache - {union} (")), "{caption:?}");
        let lines = view.controls[at(4767)].state.as_list().unwrap();
        assert_eq!(lines.rows.len(), 12);
        assert_eq!(lines.rows[0], game.text(4413));
        assert_eq!(lines.rows[11], game.text(4423));
        drop(runner);
        let then = format!("{ways}\non 93 press 1943\nonce 499 row 4767 2; ok\nuntil dialog 332");
        let Some(runner) = on_save(&game, "A", &then) else { return };
        assert_eq!(seen(&runner, 499), 1, "{}", runner.log.join("\n"));
        let state = runner.host.state();
        assert_eq!(state.season.as_ref().unwrap().halftime[0].talk, 3, "the third line, 'Konzentriert weiter so!'");
    }


    /// Walking on from A, a league matchday's doping control falls on
    /// Union's match and dialog 41 opens behind the half-time talk, before
    /// the match report: the club in the caption, two of Union's men by
    /// name, their A samples negative — nobody in A took anything — and
    /// nothing behind. With every man of Union's given a dose that can be
    /// traced in the match's week, the same control finds them out: a
    /// positive A sample, the B sample's verdict behind it, and a banned
    /// man's long absence a doping ban, his club's case counted.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_doping_control_reports_after_the_match() {
        let handlers = "on 89 press 1875\non 422 ok\non 420 ok\non 421 ok\non 287 ok\non 321 ok\non 25 ok\non 341 ok\n\
                        on 364 ok\non 120 ok\non 51 ok\non 392 ok\non 435 ok\non 96 ok\non 161 ok\non 419 ok\n\
                        on 477 ok\non 380 press 4731\non 384 press 4741\non 218 click bar 0\non 147 click bar 18\n\
                        on 302 click bar 18\non 308 choose 4095 0\non 332 ok\non 269 ok\non 93 press 1943\non 499 ok\n\
                        on 383 click at 460 210\non 99 press 2\non 307 click at 320 240\non 310 click at 320 240\n\
                        on 331 press 4303\non 65 click at 165 330\non 423 ok\non 1 click at 320 357\n\
                        on 375 click at 617 182; press 4715\non 461 ok\non 2 wait\non 381 wait\non 270 ok\non 417 press 1\n\
                        until dialog 41";
        let game = game();
        let Some(runner) = on_save(&game, "A", handlers) else { return };
        let dialog = &game.dialogs[&41];
        let text = |runner: &Runner, id: u16| runner.host.view().text(control(dialog, id).unwrap()).to_owned();
        let union = {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            records.team(14, 84).unwrap().players()
        };
        let last = |screen: &str| runner.log.iter().rposition(|l| l == screen);
        assert!(last("screen 332") < last("screen 499"), "the control comes before the report");
        let club = game.club_name(runner.host.state().records.as_ref(), 14, 84);
        assert_eq!(text(&runner, 9924), dialog.title.replacen("%s", &club, 1));
        let names: Vec<String> = {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            union.iter().filter_map(|&id| records.known_as(id)).collect()
        };
        for (name, a, b, verdict) in [(1380, 1382, 1383, 1384), (1381, 1386, 1387, 1388)] {
            assert!(names.contains(&text(&runner, name)), "{:?}", text(&runner, name));
            assert_eq!(text(&runner, a), game.text(1946), "negativ");
            assert_eq!((text(&runner, b), text(&runner, verdict)), (String::new(), String::new()));
        }
        let day = runner.host.state().season.as_ref().unwrap().day;
        drop(runner);
        // A dose wears off at the main country's club day, the second
        // weekday's (`FUN_006ff7ef`): the men take theirs behind the last
        // one before the match.
        let dosed = day - (day - 2).rem_euclid(7);
        let week = handlers.replace("until dialog 41", &format!("until day {dosed}"));
        let Some(mut runner) = on_save(&game, "A", &week) else { return };
        {
            let mut state = runner.host.state();
            let records = state.records.as_mut().unwrap();
            for &id in &union {
                records.player_mut(id).unwrap().set_doping(0x80 | 1 << 2 | 2);
            }
        }
        let result = runner.run(&Script::parse("until dialog 41").unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let state = runner.host.state();
        assert_eq!(state.season.as_ref().unwrap().day, day, "the same control");
        let records = state.records.as_ref().unwrap();
        let rows = [(1380, 1382, 1383, 1384), (1381, 1386, 1387, 1388)].map(|(n, a, b, v)| {
            [n, a, b, v].map(|id| runner.host.view().text(control(dialog, id).unwrap()).to_owned())
        });
        assert!(rows.iter().any(|r| r[1] == game.text(1945)), "{rows:?}");
        let mut banned = 0;
        for [name, a, b, verdict] in &rows {
            let id = union.iter().copied().find(|&id| records.known_as(id).as_ref() == Some(name)).unwrap();
            let r = records.player(id).unwrap();
            if *a == game.text(1946) {
                assert_eq!((b.as_str(), verdict.as_str()), ("", ""));
            } else if *verdict == game.text(1951) {
                assert_eq!(*b, game.text(1945), "the B sample positive too");
                assert_eq!(r.long_unknown(), 0x80, "a doping ban");
                assert!(r.long_to()[0] >= records.globals.date()[2], "{:?}", r.long_to());
                banned += 1;
            } else {
                assert_eq!(*verdict, game.text(1950));
                assert!([1947, 1948, 1949].iter().any(|&t| *b == game.text(t)), "{b:?}");
                assert_eq!(r.long_unknown() & 0x80, 0);
            }
        }
        let cases = records.team(14, 84).unwrap().get::<u8>(0x13e);
        assert_eq!(cases & 0x80 != 0, banned > 0, "{cases:#x}");
    }

    /// The cup results open after a cup day, behind the table after the
    /// matchday: A set back before day 59, the first round of Germany's
    /// cup and of France's, the first legs of Italy's and Spain's, all of
    /// which the save holds played, Germany's and France's second rounds
    /// drawn. The results page steps through the four cups on its bar,
    /// "Nat. Pokal" with the country's letters beside it, the round in the
    /// caption and its ties in the list: a blue line of the two sides'
    /// leagues before each new pair of them, an empty one between, each
    /// tie with both clubs, their leagues — "Amat." for the amateurs — the
    /// score and how it was decided, Union's red, a first leg's second
    /// empty. OK turns to the draw of the two second rounds, their legs
    /// empty; OK again closes it and the day walks on.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_cup_results_open_after_a_cup_day() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "A", "cup-results", |records| {
            records.globals.set_day(58);
            if let Some(slot) = records.human_slot(0)
                && let Some(planner) = records.planners.get_mut(slot)
            {
                for day in 58..=60 {
                    if matches!(planner.entry(day)[0], 0x19 | 0x0b) {
                        planner.set_entry(day, [0; a3_core::records::PLANNER_ENTRY]);
                    }
                }
            }
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let play = "on 427 ok\non 218 click bar 0\non 147 click bar 18\non 302 click bar 18\n\
                    on 384 press 4741\non 308 choose 4095 0\non 96 ok\non 93 press 1943\non 499 ok\n\
                    on 41 ok\non 332 ok\non 269 ok\non 2 wait\non 381 wait\non 25 ok\non 270 ok\n\
                    until dialog 161";
        let dialog = &game.dialogs[&161];
        let list = control(dialog, 2763).unwrap();
        let bar = control(dialog, 4778).unwrap();
        type Row = (String, Option<[u8; 3]>, bool);
        let shown = |more: &str| -> Option<(String, String, Vec<Row>, (i32, i32))> {
            let runner = on_save(&game, "cup-results", &format!("{play}\n{more}"))?;
            let view = runner.host.view();
            let text = |id: u16| view.text(control(dialog, id).unwrap()).to_owned();
            let l = view.list(list).unwrap();
            let rows = (0..l.rows.len()).map(|i| (l.rows[i].clone(), l.inks[i], l.spans[i])).collect();
            let range = view.value(bar).unwrap().range;
            Some((text(9924), text(5656), rows, range))
        };
        let Some((caption, cup, rows, range)) = shown("") else { return };
        let title = |page: u16, round: u16, country: u16| {
            format!("{} {} {} {}", game.text(page), game.text(round), game.text(0xb52), game.world.nation(country))
        };
        let label = |letters: &str| format!("{} {letters}", game.text(0x616));
        assert_eq!(caption, title(0xb4c, 0xb45, 14), "Ergebnisse Runde 1 Nationaler Pokal Deutschland");
        assert_eq!((cup, range), (label("DEU"), (0, 3)), "four cups");
        let blue = Some([0, 0, 255]);
        assert_eq!(rows[0], ("1. Bundesliga gegen 1. Bundesliga".into(), blue, true));
        let union = "1.FC Union Berlin (1. Liga)\t- 1.FC Köln (1. Liga)\t4\t:1\t\t\t";
        assert!(rows.contains(&(union.into(), Some([255, 0, 0]), false)));
        let at = rows.iter().position(|r| r.0 == "1. Bundesliga gegen 2. Bundesliga").unwrap();
        assert_eq!(rows[at - 1], (String::new(), None, false), "an empty line between");
        for tie in [
            "Borussia Dortmund (1. Liga)\t- SV 07 Elversberg (2. Liga)\t0\t:1\t\t\tn.V.",
            "Bonner SC (Amat.)\t- RB Leipzig (1. Liga)\t0\t:6\t\t\t",
            "TSV 1860 München (3. Liga)\t- VfL Wolfsburg (1. Liga)\t3\t:5\t\t\tn.E.",
        ] {
            assert!(rows.iter().any(|r| r.0 == tie), "{tie:?}");
        }
        // The bar's next cup, and its last: a first leg.
        let Some((caption, cup, ..)) = shown("now slide 4778 1") else { return };
        assert_eq!((caption, cup), (title(0xb4c, 0xb45, 20), label("FRA")));
        let Some((caption, _, rows, _)) = shown("now slide 4778 3") else { return };
        assert_eq!(caption, title(0xb4c, 0xb45, 63));
        assert!(rows.iter().any(|r| r.0 == "FC Barcelona (1. Liga)\t- CA Osasuna (1. Liga)\t0\t:1\t\t\t"));
        // OK: the draw of the two second rounds.
        let Some((caption, cup, rows, range)) = shown("now press 2762") else { return };
        assert_eq!((caption, cup, range), (title(0xb43, 0xb46, 14), label("DEU"), (0, 1)));
        let drawn = "Borussia M'gladbach (1. Liga)\t- Hamburger SV (1. Liga)\t\t\t\t\t";
        assert!(rows.contains(&(drawn.into(), None, false)));
        let Some((caption, ..)) = shown("now press 2762\nnow slide 4778 1") else { return };
        assert_eq!(caption, title(0xb43, 0xb46, 20));
        let on = format!("{play}\nnow press 2762\nnow press 2762\nuntil day 60");
        let Some(runner) = on_save(&game, "cup-results", &on) else { return };
        assert_eq!(runner.host.state().season.as_ref().unwrap().day, 60);
        assert_eq!(runner.log.iter().filter(|l| *l == "screen 161").count(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Walking on from A with the generator at 12345/67890, the original
    /// opens its first event window (dialog 51) on day 64, for a youth
    /// player grown out of the day's news (text 861) — the replica's
    /// draws part before it. With the generator at 12344/68173
    /// (the court's draw of day 67, the men's own training rolls, the
    /// week end's look at its last day's matches, each matchday's own
    /// two lists, the resting world's rolls, the condition the
    /// competition matches leave and the youth's weeks and the players'
    /// private lives, moods, the squad's own news and the men's hobbies move the replica's later ones) its first
    /// comes from a
    /// training: player event 4, the brawl of text 7, in the same window, its caption "Ereignis", Union's manager and the
    /// club as the original's (the caption bar matched pixel for pixel),
    /// the ball beside it; OK closes it and the day goes on.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_training_brawl_comes_up_in_the_event_window() {
        let game = game();
        let handlers = "on 89 press 1875\non 422 ok\non 420 ok\non 421 ok\non 287 ok\non 321 ok\non 25 ok\non 341 ok\n\
                        on 364 ok\non 120 ok\non 392 ok\non 96 ok\non 161 ok\non 419 ok\n\
                        on 477 ok\non 380 press 4731\non 384 press 4741\non 218 click bar 0\non 147 click bar 18\n\
                        on 302 click bar 18\non 308 choose 4095 0\non 332 ok\non 269 ok\non 93 press 1943\non 499 ok\n\
                        on 41 ok\non 383 click at 460 210\non 99 press 2\non 307 click at 320 240\non 310 click at 320 240\n\
                        on 331 press 4303\non 65 click at 165 330\non 423 ok\non 1 click at 320 357\n\
                        on 375 click at 617 182; press 4715\non 461 ok\non 2 wait\non 381 wait\non 270 ok\non 338 ok\n";
        let then = format!("now random 12344 68173\n{handlers}until dialog 51");
        let Some(runner) = on_save(&game, "A", &then) else { return };
        let dialog = &game.dialogs[&51];
        let text = |id: u16| runner.host.view().text(control(dialog, id).unwrap()).to_owned();
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let union = records.team(14, 84).unwrap();
        let manager = &records.managers[usize::from(union.human()) - 1];
        let club = game.club_name(Some(records), 14, 84);
        let caption = format!("{} {} {} ({club})", dialog.title, manager.first_name(), manager.last_name());
        assert_eq!(caption, "ANSTOSS 3 - Ereignis Toni Lubiniecki (1.FC Union Berlin)");
        assert_eq!(text(crate::screens::discipline::CAPTION_ID), caption);
        let texts = a3_files::saq::EventTexts::load(&game.dir).unwrap();
        assert_eq!(text(1451), texts.get(7).unwrap(), "the brawl");
        assert!(runner.log.iter().all(|l| l != "screen 435"), "nothing larger before");
        let day = state.season.as_ref().unwrap().day;
        assert!(day > 64, "{day}");
        drop(state);
        drop(runner);
        let on = format!("{then}\nnow ok\nuntil dialog 147");
        let Some(runner) = on_save(&game, "A", &on) else { return };
        assert!(runner.host.state().season.as_ref().unwrap().day >= day);
    }

    /// Walking on from A, the court Union's planner booked for day 67
    /// sits: Diogo Leite's red card waits for its verdict, and dialog 89
    /// opens on it — the club behind the title, "Die Sperre Ihres
    /// Spielers Diogo Leite" and the verdict, with Nein hidden and no
    /// question where the court leaves him under two matches; OK closes
    /// it, the case cleared and the lawyer's red-card tariff (R4, 30
    /// thousand) booked. With four matches on his card the court lets the
    /// club appeal: the question with the lawyers' advice, Nein shown; OK
    /// files the appeal for the next sitting, Nein does not.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_court_hears_a_red_card_and_asks_to_appeal() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let handlers = "on 422 ok\non 420 ok\non 421 ok\non 287 ok\non 321 ok\non 25 ok\non 341 ok\n\
                        on 364 ok\non 120 ok\non 51 ok\non 392 ok\non 435 ok\non 96 ok\non 161 ok\non 419 ok\n\
                        on 477 ok\non 380 press 4731\non 384 press 4741\non 218 click bar 0\non 147 click bar 18\n\
                        on 302 click bar 18\non 308 choose 4095 0\non 332 ok\non 269 ok\non 93 press 1943\non 499 ok\n\
                        on 41 ok\non 383 click at 460 210\non 99 press 2\non 307 click at 320 240\non 310 click at 320 240\n\
                        on 331 press 4303\non 65 click at 165 330\non 423 ok\non 1 click at 320 357\n\
                        on 375 click at 617 182; press 4715\non 461 ok\non 2 wait\non 381 wait\non 270 ok\non 417 press 1\n\
                        until dialog 89";
        const LEITE: u16 = 3261;
        let game = game();
        let Some(runner) = on_save(&game, "A", handlers) else { return };
        let dialog = &game.dialogs[&89];
        let text = |runner: &Runner, id: u16| runner.host.view().text(control(dialog, id).unwrap()).to_owned();
        let hidden = |runner: &Runner, id: u16| runner.host.view().controls[control(dialog, id).unwrap()].hidden;
        let costs = |runner: &Runner| {
            let state = runner.host.state();
            state.records.as_ref().unwrap().team(14, 84).unwrap().get::<i32>(0x2b4 + 0x278)
        };
        let club = game.club_name(runner.host.state().records.as_ref(), 14, 84);
        assert_eq!(text(&runner, 9924), format!("{} {club}", dialog.title));
        let told = text(&runner, 1876);
        assert!(told.starts_with("Die Sperre Ihres Spielers Diogo Leite "), "{told:?}");
        let verdicts = [game.text(2103).to_owned(), game.text(2795).to_owned()];
        assert!(verdicts.iter().any(|v| told.ends_with(v.as_str())), "{told:?}");
        assert_eq!(text(&runner, 1878), "");
        assert!(hidden(&runner, 1877) && !hidden(&runner, 1875));
        assert_eq!(runner.host.state().season.as_ref().unwrap().day, 67);
        let before = costs(&runner);
        drop(runner);
        let Some(runner) = on_save(&game, "A", &handlers.replace("until dialog 89", "on 89 press 1875\nuntil day 68"))
        else {
            return;
        };
        assert_eq!(seen(&runner, 89), 1, "{}", runner.log.join("\n"));
        {
            let state = runner.host.state();
            let r = state.records.as_ref().unwrap().player(LEITE).unwrap();
            assert_eq!(r.get::<u8>(0x22f) & 0xc0, 0, "the verdict is in");
        }
        assert_eq!(costs(&runner), before + 30_000);
        drop(runner);
        let root = patched(&dir, "A", "court-four", |records| {
            let r = records.player_mut(LEITE).unwrap();
            r.put::<u8>(0x22f, 0x84);
            r.put::<i16>(0x1f2, 90);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let Some(runner) = on_save(&game, "court-four", handlers) else { return };
        let question = text(&runner, 1878);
        let advice = [2105, 2106].map(|t| format!("{} {}", game.text(2104), game.text(t)));
        assert!(advice.contains(&question), "{question:?}");
        assert!(!hidden(&runner, 1877));
        let told = text(&runner, 1876);
        assert!(told.contains(" wird auf ") && told.ends_with(" Spiele festgelegt."), "{told:?}");
        drop(runner);
        for (button, appealed) in [(1875, 0x40), (1877, 0)] {
            let then = handlers.replace("until dialog 89", &format!("on 89 press {button}\nuntil day 68"));
            let Some(runner) = on_save(&game, "court-four", &then) else { return };
            let state = runner.host.state();
            let r = state.records.as_ref().unwrap().player(LEITE).unwrap();
            assert_eq!(r.get::<u8>(0x22f) & 0xc0, appealed, "{button}");
            assert!((3..=5).contains(&(r.get::<u8>(0x22f) & 0x1f)));
        }
    }

    /// A's turn opens, as the original's, on the window of the record its
    /// manager broke, the dearest sale: his name in the caption,
    /// "Herzlichen Glückwunsch" and the record's three lines, cancel gone;
    /// OK leads on to the line-up, and the record is not shown again
    /// (`FUN_00650b84`, `CRender`).
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_broken_record_opens_its_window_before_the_line_up() {
        let game = game();
        let Some(row) = game.save_games().iter().position(|s| s.name == "A") else {
            eprintln!("no A among the saves; the oracle's copy has it");
            return;
        };
        let script = format!("once 9 choose 4771 1; ok\non 424 row 5144 {row}; ok\nuntil dialog 477");
        let (runner, result) = Runner::play(&game, &Script::parse(&script).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let dialog = &game.dialogs[&477];
        let view = runner.host.view();
        let shown = |id: u16| {
            let c = control(dialog, id).unwrap();
            (!view.controls[c].hidden).then(|| view.text(c).to_owned())
        };
        assert_eq!(shown(9924).as_deref(), Some("ANSTOSS 3 - Ereignis für Toni Lubiniecki"));
        let lines: Vec<String> =
            (0..5).filter_map(|i| shown(9941 + i).or_else(|| shown(9951 + i))).filter(|l| !l.is_empty()).collect();
        assert_eq!(
            lines,
            [
                "Herzlichen Glückwunsch",
                "Sie haben damit den Rekord",
                "für den teuersten Verkauf",
                "eines Spielers verbessert!"
            ]
        );
        assert!(control(dialog, 2).is_none() && control(dialog, 1).is_some());
        drop(view);
        drop(runner);
        let Some(runner) = on_save(&game, "A", "") else { return };
        assert_eq!(seen(&runner, 477), 1, "{}", runner.log.join("\n"));
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        assert_eq!(records.managers[records.human_slot(0).unwrap()].render_events(), 0);
    }

    /// Where a day of the replica's records parts from the original's
    /// dump of the same day, by what: every player's condition, freshness
    /// and injury (the game's day it came, the date it lasts to, its kind,
    /// the knock), the human clubs' squads apart from the others, and the
    /// human clubs' money as the cash box tells it.
    fn apart(
        replica: &a3_core::records::Records,
        oracle: &a3_core::records::Records,
    ) -> std::collections::BTreeMap<&'static str, Vec<String>> {
        let mut apart = std::collections::BTreeMap::<&'static str, Vec<String>>::new();
        let human: Vec<_> = oracle
            .humans()
            .filter_map(|m| Some((m.country(), m.team(), oracle.team(m.country(), m.team())?)))
            .filter(|(.., team)| team.humans() != [0, 0])
            .collect();
        let squads: std::collections::BTreeSet<u16> = human.iter().flat_map(|(.., team)| team.players()).collect();
        for (id, (r, o)) in replica.players.iter().zip(&oracle.players).enumerate().skip(1) {
            let id = id as u16;
            let human = squads.contains(&id);
            let mut differ = |what: [&'static str; 2], a: String, b: String| {
                if a != b {
                    apart.entry(what[usize::from(human)]).or_default().push(format!("{id}: {a}, not {b}"));
                }
            };
            differ(["condition", "human condition"], r.condition().to_string(), o.condition().to_string());
            differ(["freshness", "human freshness"], format!("{:?}", r.freshness()), format!("{:?}", o.freshness()));
            let injury = |p: &a3_core::records::Player| {
                format!("{} {:?} {} {}", p.injured_on(), p.injured_to(), p.injury_kind(), p.knocked())
            };
            differ(["injury", "human injury"], injury(r), injury(o));
            differ(["abilities", "human abilities"], format!("{:x} {:x}", r.good(), r.bad()), format!("{:x} {:x}", o.good(), o.bad()));
        }
        for (country, number, team) in human {
            let money = a3_core::finances::cash_and_result(team);
            let ours = replica.team(country, number).map(a3_core::finances::cash_and_result);
            if ours != Some(money) {
                apart.entry("money").or_default().push(format!("{country}/{number}: {ours:?}, not {money}"));
            }
            // The men's own training as the club's store keeps it, entry
            // by entry: who, what he works on, how far he got, how hard.
            let slot = usize::from(team.human()).saturating_sub(1);
            let own = |records: &a3_core::records::Records| -> Vec<String> {
                records.individual.get(slot).map_or_else(Vec::new, |store| {
                    store
                        .entries()
                        .iter()
                        .map(|e| format!("{} {} {:?} {}", e.id, e.ability, e.progress, e.intensity))
                        .collect()
                })
            };
            for (k, (a, b)) in own(replica).iter().zip(own(oracle)).enumerate() {
                if *a != b {
                    apart.entry("human own training").or_default().push(format!("entry {k}: {a}, not {b}"));
                }
            }
        }
        apart
    }

    /// What the replica still reckons otherwise than the original over
    /// Probe1's week, each the matter of a part of its own: the computer
    /// clubs' injuries breaking out and healing (daily-reckoning-injuries)
    /// and the condition and freshness their men's matches, friendlies
    /// and tournament games leave (daily-reckoning-match-wear). The human
    /// club's money agrees, its season tickets selling day by day.
    const STILL_APART: [&str; 3] = ["injury", "condition", "freshness"];

    /// What of [`STILL_APART`] parts only where a match did: the men of a
    /// club that played one that day by the original's dumps, and the men
    /// whose injury parts. Everywhere else the computer clubs' week, their
    /// days off and the rest of the world's agree.
    const MATCH_WEAR: [&str; 2] = ["condition", "freshness"];

    /// The oracle's week from Probe1 (`tools/oracle/check.sh` step 1,
    /// `tools/oracle/scripts/from-save.txt`, a dump after each day's close)
    /// against the replica's days from the same save, its random numbers
    /// set at the line-up to where the original's stood: the loader
    /// overwrites the state's first half with `rand()` (SAVEGAME.md), and
    /// the dump of day 35, taken as the turn opens, holds it. Each day
    /// after starts from the generator the original's dump of the day
    /// before closed with, so a day's draws are held against the
    /// original's even where an earlier day's unported ones would have
    /// moved them, and from its men's condition, freshness and injuries.
    /// The load agrees in everything compared, and the human club's squad
    /// and money keep agreeing day by day; what still parts is
    /// [`STILL_APART`], its fitness only at the clubs that played
    /// ([`MATCH_WEAR`]).
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR and the oracle's dumps from $ANSTOSS3_ORACLE"]
    fn probe1s_week_against_the_oracles_dumps() {
        let script = include_str!("../../../tools/oracle/scripts/from-save.txt");
        // The supercup's final on day 41 (14/85 at 14/3) wears both squads
        // by the competition rows; the visitors, who field their saved
        // eleven the whole match, agree man by man the day after.
        let agree = [(42, (14, 85))];
        let Some(seen) = week_against_dumps(script, "runs/check/save-a", "tools/oracle/check.sh", &agree) else { return };
        // A part that brings its matter into line takes it off the list.
        for what in STILL_APART {
            assert!(seen.contains_key(what), "{what} agrees all week now: take it off STILL_APART");
        }
    }

    /// Probe1's week again with the manager's first template planned at
    /// the line-up ("gemischte Woche", `tools/oracle/scripts/training-week.txt`):
    /// the sessions take the human squad's freshness and bring its
    /// condition by the units' rows, its men's own training rolls for
    /// what they work on and learn it, all as the original's dumps have
    /// it day by day.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR and the oracle's dumps from $ANSTOSS3_ORACLE"]
    fn a_training_week_against_the_oracles_dumps() {
        let script = include_str!("../../../tools/oracle/scripts/training-week.txt");
        let how = "the oracle's play.sh of tools/oracle/scripts/training-week.txt on Probe1";
        let Some(seen) = week_against_dumps(script, "runs/training-week", how, &[]) else { return };
        assert!(seen.contains_key("condition"), "the computer clubs' condition still parts: {seen:?}");
        // The week did train: the original's own days part in the human
        // squad's condition and its men's own training.
        let dumps = PathBuf::from(std::env::var_os("ANSTOSS3_ORACLE").unwrap()).join("runs/training-week");
        let dump = |day: u16| {
            let save = a3_files::an3::Save::read(&dumps.join(format!("day-{day:03}.an3"))).unwrap();
            a3_core::records::Records::from_save(save).unwrap()
        };
        let moved = apart(&dump(42), &dump(36));
        assert!(moved.contains_key("human condition") && moved.contains_key("human own training"), "{moved:?}");
    }

    /// Plays `script` on Probe1 against the oracle's dumps of the same
    /// script in `run` (of `$ANSTOSS3_ORACLE`), day by day to day 42;
    /// whatever parts must be among [`STILL_APART`], and what did comes
    /// back with the number of days it did. The clubs `agree` names by
    /// day keep no man apart in [`MATCH_WEAR`] that day. None when the
    /// dumps or the save are missing (`how` writes them).
    fn week_against_dumps(
        script: &str,
        run: &str,
        how: &str,
        agree: &[(u16, (u16, u16))],
    ) -> Option<std::collections::BTreeMap<&'static str, usize>> {
        use a3_core::records::Records;
        use a3_files::an3::Save;
        let Some(oracle) = std::env::var_os("ANSTOSS3_ORACLE") else {
            eprintln!("no oracle dumps of Probe1's week among the saves: ANSTOSS3_ORACLE is not set");
            return None;
        };
        let dumps = PathBuf::from(oracle).join(run);
        if !dumps.join("day-042.an3").exists() {
            eprintln!("no oracle dumps of Probe1's week in {run} among the saves; {how} writes them");
            return None;
        }
        let dump = |day: u16| Save::read(&dumps.join(format!("day-{day:03}.an3"))).unwrap();
        let word = |save: &Save, name: &str| {
            i64::from(i32::from_le_bytes(save.block(name).unwrap().data[..4].try_into().unwrap()))
        };
        let first = dump(35);
        let (s1, s2) = (word(&first, "0x89dcc0"), word(&first, "0x89dcc4"));
        let game = game();
        let Some(row) = game.save_games().iter().position(|s| s.name == "Probe1") else {
            eprintln!("no Probe1 among the saves; the oracle's copy has it");
            return None;
        };
        let script: String = script
            .lines()
            .filter(|line| !line.starts_with("until"))
            .map(|line| line.replace("row 5144 0", &format!("row 5144 {row}")) + "\n")
            .collect();
        let script = format!("{script}until dialog 147\nnow random {s1} {s2}");
        let (mut runner, result) = Runner::play(&game, &Script::parse(&script).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        {
            let state = runner.host.state();
            let loaded = apart(state.records.as_ref().unwrap(), &Records::from_save(first).unwrap());
            assert!(loaded.is_empty(), "the load parts from the original's: {loaded:?}");
        }
        let mut seen = std::collections::BTreeMap::<&str, usize>::new();
        let id_of = |entry: &String| entry.split(':').next().and_then(|id| id.parse::<u16>().ok()).unwrap();
        for day in 36..=42u16 {
            let before = dump(day - 1);
            let (s1, s2) = (word(&before, "0x89dcc0"), word(&before, "0x89dcc4"));
            let before = Records::from_save(before).unwrap();
            // The matters still apart start the day as the original's:
            // the men's condition, freshness and injuries, so a day's
            // draws behind a man who is out in one game and not in the
            // other stay in line, and what parts is the day's own.
            {
                let mut state = runner.host.state();
                let records = state.records.as_mut().unwrap();
                for (r, o) in records.players.iter_mut().zip(&before.players) {
                    r.set_condition(o.condition());
                    r.set_freshness(o.freshness());
                    r.set_injured_on(o.injured_on());
                    r.set_injured_to(o.injured_to());
                    r.set_injury_kind(o.injury_kind());
                    r.set_knocked(o.knocked());
                }
            }
            let result = runner.run(&Script::parse(&format!("now random {s1} {s2}\nuntil day {day}")).unwrap());
            result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
            let oracle = Records::from_save(dump(day)).unwrap();
            let state = runner.host.state();
            let replica = state.records.as_ref().unwrap();
            assert_eq!(replica.globals.day(), oracle.globals.day());
            assert!(oracle.humans().any(|m| oracle.team(m.country(), m.team()).is_some_and(|t| t.humans() != [0, 0])));
            // The clubs that played the day: under a match's wear
            // (`FUN_006f8b3f`) a man's third freshness word moves to
            // something not nought, which nothing else of a computer
            // club's day does — the week's bulk sets it to nought, the
            // days off move the first word only.
            let mut played = std::collections::BTreeSet::<(u16, u16)>::new();
            let mut club_of = std::collections::BTreeMap::<u16, (u16, u16)>::new();
            for country in 1..0x75u16 {
                for number in 0..oracle.countries.team_count(usize::from(country)).max(0) as u16 {
                    let Some(team) = oracle.team(country, number) else { continue };
                    let worn = |id: &u16| match (before.player(*id), oracle.player(*id)) {
                        (Some(b), Some(a)) => a.freshness()[2] != 0 && a.freshness()[2] != b.freshness()[2],
                        _ => false,
                    };
                    if team.players().iter().any(worn) {
                        played.insert((country, number));
                    }
                    club_of.extend(team.players().into_iter().map(|id| (id, (country, number))));
                }
            }
            // On the third weekday the clubs of the countries not played
            // rest up to bounds that each club moves by a roll of ten and
            // of five (`FUN_0078680a`); those rolls come behind the day's
            // matches, whose engine draws are not the replica's yet, so
            // where a man stands near his bound his rest may part.
            if (day - 1) % 7 == 2 {
                played.extend(club_of.values().filter(|(country, _)| !oracle.globals.played(*country as i16)));
            }
            let parted = apart(replica, &oracle);
            for &(_, club) in agree.iter().filter(|(on, _)| *on == day) {
                let off: Vec<&String> = parted
                    .iter()
                    .filter(|(what, _)| MATCH_WEAR.contains(what))
                    .flat_map(|(_, list)| list)
                    .filter(|entry| club_of.get(&id_of(entry)) == Some(&club))
                    .collect();
                assert!(off.is_empty(), "day {day}: the men of {club:?} part, {off:?}");
            }
            let injured: std::collections::BTreeSet<u16> = parted.get("injury").into_iter().flatten().map(id_of).collect();
            for (what, list) in parted {
                assert!(STILL_APART.contains(&what), "day {day}, {what}: {} apart, {:?}", list.len(), &list[..list.len().min(5)]);
                if MATCH_WEAR.contains(&what) {
                    let unplayed: Vec<&String> = list
                        .iter()
                        .filter(|entry| {
                            let id = id_of(entry);
                            !injured.contains(&id) && !club_of.get(&id).is_some_and(|club| played.contains(club))
                        })
                        .collect();
                    assert!(unplayed.is_empty(), "day {day}, {what}: {} apart whose club played nothing, {:?}", unplayed.len(), &unplayed[..unplayed.len().min(5)]);
                }
                eprintln!("day {day}, {what}: {} apart, {:?}", list.len(), &list[..list.len().min(3)]);
                *seen.entry(what).or_default() += 1;
            }
        }
        Some(seen)
    }

    /// From the office the tactics come without the bar and with their
    /// OK, as every hub opens its screens from the office or the text
    /// menu (request byte 0 nought, `FUN_00652a86` in the turn before a
    /// match): the OK keeps the stepping buttons for the match — Einsatz
    /// "Normal" one step on is "120%", the record's value 2 the match's
    /// wear reads — and leads back to the office.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_tactics_from_the_office_keep_with_their_ok() {
        use a3_core::cycle::dialogs;
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "Toni", "tactics-office", |records| {
            for m in &mut records.managers {
                m.set_menu_view(1);
            }
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let dialog = &game.dialogs[&dialogs::TACTICS];
        let to_tactics = LOAD.replace("until dialog 147", "until dialog 483\nnow press 6124\nuntil dialog 271");
        let (runner, result) = Runner::play(&game, &Script::parse(&to_tactics).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        assert!(runner.host.state().season.as_ref().unwrap().before_match);
        let hidden = |runner: &Runner, id: u16| runner.host.view().controls[crate::screens::control(dialog, id).unwrap()].hidden;
        assert!(!hidden(&runner, 1), "the OK shows");
        assert!(hidden(&runner, 9930), "no bar");
        let back = to_tactics.replace("until dialog 271", "until dialog 271\nnow press 3739; press 1\nuntil dialog 483");
        let (runner, result) = Runner::play(&game, &Script::parse(&back).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        assert_eq!(runner.host.view().id, dialogs::OFFICE);
        let state = runner.host.state();
        assert_eq!(state.records.as_ref().unwrap().teams[84].effort(), 2);
    }
    /// A human whose count at record +0x180 stands above one hears of his
    /// contract at the season's turn: past A's statement his club stayed
    /// in its league, so the season's new goal (dialog 379) opens, and OK
    /// lets the game walk on into the new season.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_turn_tells_a_human_his_seasons_goal() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "A", "season-goal", |records| {
            records.globals.set_day(320);
            for slot in 0..records.globals.slots().len() {
                if records.globals.slots()[slot] > 0 {
                    records.managers[slot].set_unrest_count(2);
                }
            }
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let then = "on 384 press 4741\non 2 wait\non 381 wait\non 356 press 4542\n\
                    now click bar 9\nuntil dialog 417\nnow click bar 18\n\
                    until dialog 417\nnow press 1\nuntil dialog 379";
        let Some(mut runner) = on_save(&game, "season-goal", then) else { return };
        let log = runner.log.join("\n");
        assert_eq!(runner.host.view().id, a3_core::cycle::dialogs::SEASON_GOAL, "{log}");
        assert_eq!(runner.host.state().season.as_ref().unwrap().year, 2026);
        let _ = runner.settle();
        let _ = std::fs::remove_dir_all(&root);
    }
    /// Past the season's statement each human club's manager has the
    /// season start's event 0x82 — the negotiations he leads himself
    /// (dialog 422), then the season's preparation (419) — and the
    /// overviews of the season's start follow (event 0x130, dialog 420);
    /// OK closes each, and the game walks on into the new season.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_turn_brings_the_season_starts_screens() {
        use crate::screens::season_start::{OVERVIEWS, PREPARATION, TALKS};
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "A", "season-start", |records| {
            records.globals.set_day(320);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let to = |last: u16| {
            format!(
                "on 384 press 4741\non 2 wait\non 381 wait\non 379 ok\non 314 ok\non 356 press 4542\n\
                 now click bar 9\nuntil dialog 417\nnow click bar 18\n\
                 until dialog 417\nnow press 1\nuntil dialog {TALKS}{}",
                match last {
                    TALKS => String::new(),
                    PREPARATION => format!("\nnow press 1\nuntil dialog {PREPARATION}"),
                    _ => format!("\nnow press 1\nuntil dialog {PREPARATION}\nnow press 1\nuntil dialog {OVERVIEWS}"),
                }
            )
        };
        let Some(runner) = on_save(&game, "season-start", &to(TALKS)) else { return };
        let log = runner.log.join("\n");
        assert_eq!(runner.host.view().id, TALKS, "{log}");
        assert_eq!(runner.host.state().season.as_ref().unwrap().day, 0, "the new season stands");
        let dialog = &game.dialogs[&TALKS];
        assert_eq!(dialog.controls[control(dialog, 6044).unwrap()].text, " Fernsehgelder  (bei Teilnahme)");
        drop(runner);
        let Some(runner) = on_save(&game, "season-start", &to(PREPARATION)) else { return };
        assert_eq!(runner.host.view().id, PREPARATION, "{}", runner.log.join("\n"));
        let money = control(&game.dialogs[&PREPARATION], 5048).unwrap();
        assert_eq!(runner.host.view().text(money), "", "no sample figure");
        drop(runner);
        let Some(mut runner) = on_save(&game, "season-start", &to(OVERVIEWS)) else { return };
        assert_eq!(runner.host.view().id, OVERVIEWS, "{}", runner.log.join("\n"));
        let _ = runner.settle();
        let _ = std::fs::remove_dir_all(&root);
    }
    /// An even year's turn offers a human standing 9 with the season's
    /// points past 400 national teams (`FUN_006b9c81`): past A's
    /// statement into season 2, 2026, dialog 356 lists the nations, the
    /// caption names him, the first stands chosen with its terms, and OK
    /// makes him its coach on them before the season start's screens.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn an_even_years_turn_offers_a_human_a_national_team() {
        use crate::screens::national_offer::NATIONAL_OFFER;
        use crate::screens::season_start::TALKS;
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let root = patched(&dir, "A", "national-offer", |records| {
            records.globals.set_day(320);
            for slot in 0..records.globals.slots().len() {
                if records.globals.slots()[slot] > 0 {
                    let m = &mut records.managers[slot];
                    m.set_level(9);
                    m.set_level_bits(0);
                    m.put::<i16>(0x1f74, 6009);
                }
            }
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let open = "on 384 press 4741\non 2 wait\non 381 wait\non 379 ok\non 314 ok\n\
                    now click bar 9\nuntil dialog 417\nnow click bar 18\n\
                    until dialog 417\nnow press 1\nuntil dialog 356";
        let Some(runner) = on_save(&game, "national-offer", open) else { return };
        let log = runner.log.join("\n");
        assert_eq!(runner.host.view().id, NATIONAL_OFFER, "{log}");
        let dialog = &game.dialogs[&NATIONAL_OFFER];
        let view = runner.host.view();
        let list = view.list(control(dialog, 4543).unwrap()).unwrap();
        assert!(!list.rows.is_empty() && list.rows.len() <= 10, "{:?}", list.rows);
        assert_eq!(list.selected, Some(0));
        assert_eq!(view.text(control(dialog, 4551).unwrap()), list.rows[0]);
        assert!(view.text(control(dialog, 4547).unwrap()).contains('€'));
        let first = list.rows[0].clone();
        drop(runner);
        let Some(mut runner) = on_save(&game, "national-offer", &format!("{open}\nnow press 4546\nuntil dialog {TALKS}"))
        else {
            return;
        };
        assert_eq!(runner.host.view().id, TALKS, "{}", runner.log.join("\n"));
        {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let slot = records.human_slot(0).unwrap();
            let nation = records.managers[slot].national_team();
            assert!(nation > 0, "he coaches a nation");
            assert_eq!(game.club_name(Some(records), nation as u8, 200), first);
            assert_eq!(a3_core::national::coach_number(records, nation), slot as u8 + 1);
            assert!(records.managers[slot].get::<i32>(0x1f7c) >= 250_000);
        }
        let _ = runner.settle();
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A youth on trial at Union in his last week (A patched, his weeks at
    /// 1): the loaded week's end holds its block, the next, at day 70,
    /// sends him back to the youth and tells the club in the event window,
    /// "Ihr Jugendspieler %s trainiert ab jetzt wieder in Ihrer A-Jugend!"
    /// (text 4156, `FUN_0074d2b5`), his name in it.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_trials_end_is_told_to_the_club() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        let mut youth = 0;
        let root = patched(&dir, "A", "trial-end", |records| {
            youth = *records.team(14, 84).unwrap().players().last().unwrap();
            let p = records.player_mut(youth).unwrap();
            p.set_retiring(p.retiring() | 0x10);
            p.set_picks(p.picks() & 0x30 | 1);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let handlers = "on 89 press 1875\non 422 ok\non 420 ok\non 421 ok\non 287 ok\non 321 ok\non 25 ok\non 341 ok\n\
                        on 364 ok\non 120 ok\non 392 ok\non 96 ok\non 161 ok\non 419 ok\non 435 ok\n\
                        on 477 ok\non 380 press 4731\non 384 press 4741\non 218 click bar 0\non 147 click bar 18\n\
                        on 302 click bar 18\non 308 choose 4095 0\non 332 ok\non 269 ok\non 93 press 1943\non 499 ok\n\
                        on 41 ok\non 383 click at 460 210\non 99 press 2\non 307 click at 320 240\non 310 click at 320 240\n\
                        on 331 press 4303\non 65 click at 165 330\non 423 ok\non 1 click at 320 357\n\
                        on 375 click at 617 182; press 4715\non 461 ok\non 2 wait\non 381 wait\non 270 ok\non 338 ok\n";
        let Some(mut runner) = on_save(&game, "trial-end", &format!("{handlers}until dialog 51")) else { return };
        let dialog = &game.dialogs[&51];
        let name = runner.host.state().records.as_ref().unwrap().known_as(youth).unwrap();
        let told = game.text(4156).replacen("%s", &name, 1);
        assert_eq!(told, format!("Ihr Jugendspieler {name} trainiert ab jetzt wieder in Ihrer A-Jugend!"));
        for _ in 0..40 {
            if runner.host.view().text(control(dialog, 1451).unwrap()) == told {
                break;
            }
            assert!(runner.host.state().season.as_ref().unwrap().day <= 70, "{}", runner.log.join("\n"));
            runner.run(&Script::parse("now ok\nuntil dialog 51").unwrap()).unwrap();
        }
        assert_eq!(runner.host.view().text(control(dialog, 1451).unwrap()), told);
        {
            let state = runner.host.state();
            assert_eq!(state.season.as_ref().unwrap().day, 70);
            let records = state.records.as_ref().unwrap();
            assert!(!records.team(14, 84).unwrap().players().contains(&youth));
            assert_eq!(records.player(youth).unwrap().retiring() & 0x10, 0);
        }
        runner.run(&Script::parse("now ok\nuntil dialog 147").unwrap()).unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }

    /// SeasonEnd364 patched so that twelve of Union's men end their
    /// careers: the season's end tells them first (dialog 152, the
    /// original's shot of the same save matching but for strengths its
    /// own development drew) — the club in the caption, "Saison 2025/26"
    /// over the first ten, "Name, F.", position, strength, age — and OK,
    /// the last page, goes on to the statement while they are gone from
    /// the squad.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_season_end_says_goodbye_to_the_leaving_men() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        if !dir.join("saves/SeasonEnd364.an3").exists() && !dir.parent().unwrap().join("saves-kept/SeasonEnd364.an3").exists() {
            eprintln!("no SeasonEnd364 among the saves; the oracle's copy has it");
            return;
        }
        let mut leaving = Vec::new();
        let root = patched(&dir, "SeasonEnd364", "farewells", |records| {
            let (country, number) = a3_core::statistics::human_clubs(records)[0];
            let squad = records.team(country.into(), number as u16).unwrap().players();
            leaving = squad[squad.len() - 12..].to_vec();
            for &id in &leaving {
                let p = records.player_mut(id).unwrap();
                p.set_retiring(p.retiring() | 0x80);
            }
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        // The loaded game's turn reaches the season's end at once.
        let row = game.save_games().iter().position(|s| s.name == "farewells").unwrap();
        let load = LOAD.replace("row 5144 0", &format!("row 5144 {row}")).replace("until dialog 147", "until dialog 152");
        let (mut runner, result) = Runner::play(&game, &Script::parse(&load).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let dialog = &game.dialogs[&152];
        assert_eq!(runner.host.view().text(control(dialog, 2593).unwrap()), "Saison 2025/26");
        assert_eq!(
            runner.host.view().text(control(dialog, 9924).unwrap()),
            "ANSTOSS 3 - Spieler des 1.FC Union Berlin, die ihre Karriere beenden..."
        );
        let rows = runner.host.view().list(control(dialog, 2595).unwrap()).unwrap().rows.clone();
        assert_eq!(rows.len(), 10, "ten of twelve");
        {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let r = records.player(leaving[0]).unwrap();
            let [day, month, year] = records.globals.date();
            let today = chrono::NaiveDate::from_ymd_opt(year.into(), month.into(), day.into()).unwrap();
            let fields: Vec<&str> = rows[0].split('\t').collect();
            assert_eq!(fields.len(), 4, "{}", rows[0]);
            assert!(fields[0].contains(", ") || r.shown() & 0x8000_0000 != 0, "{}", rows[0]);
            assert_eq!(fields[2], r.strength().to_string());
            assert_eq!(fields[3], a3_core::statistics::age_on(r, today).to_string());
        }
        runner.run(&Script::parse("now press 2594\nuntil dialog 417").unwrap()).unwrap();
        {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let (country, number) = a3_core::statistics::human_clubs(records)[0];
            let squad = records.team(country.into(), number as u16).unwrap().players();
            assert!(leaving.iter().all(|id| !squad.contains(id)), "the men are gone");
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// SeasonEnd364's turn rolls the European ranking on and tells it
    /// (dialog 301, event 0xcf) behind the statement, as the original's
    /// shot of the same save shows it pixel for pixel: the dialog's
    /// "Pl.", "Land", "Punkte gesamt" and the five seasons up to 2025/26;
    /// the countries in their new order, each with its place, its total
    /// and those seasons' points, the one just played the last and the
    /// new one, at naught, not shown. OK goes on into the new season.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_season_end_tells_the_rolled_european_ranking() {
        let game = game();
        let Some(row) = game.save_games().iter().position(|s| s.name == "SeasonEnd364") else {
            eprintln!("no SeasonEnd364 among the saves; the oracle's copy has it");
            return;
        };
        let load = LOAD
            .replace("row 5144 0", &format!("row 5144 {row}"))
            .replace("on 301 ok\n", "")
            .replace("until dialog 147", "on 152 press 2594\non 417 ok\nuntil dialog 301");
        let (mut runner, result) = Runner::play(&game, &Script::parse(&load).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let dialog = &game.dialogs[&301];
        let header = runner.host.view().header(control(dialog, 4034).unwrap()).unwrap().text.clone();
        assert_eq!(header, "Pl.\\tLand\\tPunkte gesamt\\t21/22\\t22/23\\t23/24\\t24/25\\t25/26");
        let rows = runner.host.view().list(control(dialog, 4031).unwrap()).unwrap().rows.clone();
        {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let ranked = a3_core::statistics::five_years(records);
            let shown: Vec<_> = ranked.iter().filter(|(country, _)| *country != 0).collect();
            assert!(shown.len() > 20, "{}", shown.len());
            assert_eq!(rows.len(), shown.len());
            assert!(shown.iter().all(|(_, points)| points[6] == 0.0), "the new season at naught");
            let fields: Vec<&str> = rows[0].split('\t').collect();
            assert_eq!(fields.len(), 8, "{}", rows[0]);
            assert_eq!(fields[0], "1.");
            assert_eq!(fields[1], game.world.nation(shown[0].0));
            assert_eq!(fields[2], format!("{:.2}", shown[0].1[0]));
            assert_eq!(fields[7], format!("{:.2}", shown[0].1[5]));
            let totals: Vec<f64> = rows.iter().map(|r| r.split('\t').nth(2).unwrap().parse().unwrap()).collect();
            assert!(totals.windows(2).all(|w| w[0] >= w[1]), "{totals:?}");
        }
        runner.run(&Script::parse("now press 4032").unwrap()).unwrap();
        let _ = runner.settle();
        assert_ne!(runner.host.view().id, 301, "OK closes it");
        assert_eq!(runner.host.state().records.as_ref().unwrap().globals.start_year(), 2026);
    }

    /// SeasonEnd364's season end shows the players' development first
    /// (dialog 22, event 0xca): Union's name in the caption, the plain
    /// header as its trainer plays at difficulty 0, and a line a man —
    /// name, position, age, form, season's form, old strength, whether he
    /// got better or worse, and his new strength as the records now have
    /// it. OK turns the page, the last goes on.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_season_end_shows_the_players_development() {
        let game = game();
        let Some(row) = game.save_games().iter().position(|s| s.name == "SeasonEnd364") else {
            eprintln!("no SeasonEnd364 among the saves; the oracle's copy has it");
            return;
        };
        let load = LOAD
            .replace("row 5144 0", &format!("row 5144 {row}"))
            .replace("on 22 press 1306\n", "")
            .replace("until dialog 147", "until dialog 22");
        let (mut runner, result) = Runner::play(&game, &Script::parse(&load).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let dialog = &game.dialogs[&22];
        assert_eq!(
            runner.host.view().text(control(dialog, 9924).unwrap()),
            "ANSTOSS 3 - Auf- und Abwertungen 1.FC Union Berlin"
        );
        let header = runner.host.view().header(control(dialog, 1308).unwrap()).unwrap().text.clone();
        assert_eq!(header, "Name\\tPos\\tAlter\\tForm\\tD-Form\\tAlte Stärke\\tGrund\\t\\tNeue Stärke");
        let list = runner.host.view().list(control(dialog, 1307).unwrap()).unwrap().clone();
        let pages = runner.host.state().season.as_ref().unwrap().development.clone();
        assert!(!pages.is_empty() && pages[0].insight == 0 && !pages[0].rows.is_empty());
        assert_eq!(list.rows.len(), pages[0].rows.len());
        {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            for (line, r) in list.rows.iter().zip(&pages[0].rows) {
                let fields: Vec<&str> = line.split('\t').collect();
                assert_eq!(fields.len(), 9, "{line}");
                assert_eq!(fields[2], r.age.to_string());
                assert_eq!(fields[4], format!("{:.2}", f32::from(r.season_form) / 10.0));
                assert_eq!(fields[5], r.old.to_string());
                let said = match r.new.cmp(&r.old) {
                    std::cmp::Ordering::Greater => "hat sich verbessert",
                    std::cmp::Ordering::Less => "hat sich verschlechtert",
                    std::cmp::Ordering::Equal => "",
                };
                assert_eq!(fields[6], said, "{line}");
                assert_eq!(fields[8], r.new.to_string());
                assert_eq!(records.player(r.player).unwrap().strength(), r.new, "{r:?}");
            }
        }
        for _ in 0..pages.len() {
            runner.run(&Script::parse("now press 1306").unwrap()).unwrap();
            let _ = runner.settle();
        }
        assert_ne!(runner.host.view().id, 22, "the last page's OK closes it");
    }

    /// A's human made England's coach, his lists of the best by position
    /// built from the squads as the week's market builds them: the
    /// picker's bests list shows the first ten of the chosen position's,
    /// the strongest today first, each with his full name and strength.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_picker_lists_the_coachs_best_by_position() {
        let dir = PathBuf::from(std::env::var_os("ANSTOSS3_DIR").unwrap());
        if on_save(&game(), "A", "").is_none() {
            return;
        }
        let source = game();
        let pitch = &source.rules.pitch;
        let root = patched(&dir, "A", "nationalbests", |records| {
            let slot = records.human_slot(0).unwrap();
            a3_core::national::take(records, slot, a3_core::national::Terms { nation: 14, salary: 0, bonus: 0 });
            let mut bests = a3_core::national::Bests::new(records);
            for country in (1..0x75u16).filter(|&c| records.countries.kind(usize::from(c)) != 0) {
                for number in 0..records.countries.team_count(usize::from(country)).max(0) as u16 {
                    bests.offer(records, pitch, (country, number));
                }
            }
            bests.store(records);
        });
        let mut game = Game::load(&root).unwrap();
        compose(&mut game);
        let open = "now click bar 15\nuntil dialog 355\nnow choose 4538 0\nuntil dialog 357";
        let dialog = &game.dialogs[&357];
        let bests = crate::screens::control(dialog, 4558).unwrap();
        // On opening the first position's; the third's once its item
        // is chosen.
        for (group, script) in [(1, open.to_owned()), (3, format!("{open}\nnow choose 4775 2"))] {
            let runner = on_save(&game, "nationalbests", &script).unwrap();
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let list = runner.host.view().list(bests).unwrap();
            let mut want: Vec<(u16, i32)> = a3_core::national::bests(records, 14, group)
                .iter()
                .take(10)
                .filter(|b| b.id != 0)
                .map(|b| {
                    let (p, f, _) = a3_core::load::any_player(records, &game.data, b.id).unwrap();
                    (b.id, game.rules.pitch.strength_at(&p, f, p.positions[0]))
                })
                .collect();
            for i in 0..want.len() {
                for j in i + 1..want.len() {
                    if want[i].1 < want[j].1 {
                        want.swap(i, j);
                    }
                }
            }
            assert_eq!(want.len(), 10, "{}", runner.log.join("\n"));
            assert_eq!(list.keys, want.iter().map(|w| usize::from(w.0)).collect::<Vec<_>>(), "position {group}");
            for (row, &(id, strength)) in list.rows.iter().zip(&want) {
                assert_eq!(*row, format!("{}\t{:.1}", records.known_as(id).unwrap(), strength as f32 / 10.0));
            }
            assert!(list.row_icons.iter().any(|icons| !icons.is_empty()));
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// SeasonEnd364's turn reaches the season start's negotiations (dialog
    /// 422): each box ticked or off as the club's contracts and its place
    /// in Europe leave it (`CSaisonstartVerhandlungen::FUN_0053b05b`), the
    /// bonuses' always open, the television's label without its "%s".
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_season_start_talks_tick_the_open_negotiations() {
        let game = game();
        let Some(row) = game.save_games().iter().position(|s| s.name == "SeasonEnd364") else {
            eprintln!("no SeasonEnd364 among the saves; the oracle's copy has it");
            return;
        };
        let load = LOAD.replace("row 5144 0", &format!("row 5144 {row}")).replace(
            "until dialog 147",
            "on 152 press 2594\non 417 ok\non 301 ok\non 22 ok\non 89 press 1875\non 287 ok\non 321 ok\non 25 ok\non 341 ok\non 364 ok\non 120 ok\non 51 ok\non 392 ok\non 435 ok\non 96 ok\non 161 ok\nuntil dialog 422",
        );
        let (runner, result) = Runner::play(&game, &Script::parse(&load).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        assert_eq!(runner.host.view().id, 422, "{}", runner.log.join("\n"));
        let dialog = &game.dialogs[&422];
        let state = runner.host.state();
        let records = state.records.as_ref().unwrap();
        let (slot, number) = state.season.as_ref().unwrap().manager().unwrap().club.unwrap();
        let nation = game.world.country(slot).unwrap().nation;
        let team = records.team(nation, number as u16).unwrap();
        let talks = a3_core::finances::season_talks(team, records.globals.preparation());
        assert!(talks[3].open, "the bonuses' box is always open");
        for (id, talk) in [0x889u16, 0x88a, 0x88b, 0x88c, 0x179b, 0x179c].into_iter().zip(talks) {
            let c = control(dialog, id).unwrap();
            let crate::controls::ControlState::Check(check) = &runner.host.view().controls[c].state else { panic!("{id}") };
            assert_eq!((check.checked, check.disabled), (talk.ticked, !talk.open), "box {id:#x}");
        }
        let c = control(dialog, 0x179c).unwrap();
        let crate::controls::ControlState::Check(check) = &runner.host.view().controls[c].state else { panic!() };
        assert_eq!(check.text, " Fernsehgelder  (bei Teilnahme)");
    }

    /// SeasonEnd364's season start with the main sponsor's box unticked:
    /// the computer signs what the manager left it (`FUN_006741be`) — the
    /// main sponsor, and the boards with it, whose own tick the original
    /// never reads — while the talks kept stay as they were; the
    /// European television's talks (dialog 475) come up for the club in
    /// Europe, and the preparation follows.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_season_start_talks_left_unticked_are_signed() {
        let game = game();
        let Some(row) = game.save_games().iter().position(|s| s.name == "SeasonEnd364") else {
            eprintln!("no SeasonEnd364 among the saves; the oracle's copy has it");
            return;
        };
        let turn = "on 152 press 2594\non 417 ok\non 301 ok\non 22 ok\non 89 press 1875\non 287 ok\non 321 ok\non 25 ok\non 341 ok\non 364 ok\non 120 ok\non 51 ok\non 392 ok\non 435 ok\non 96 ok\non 161 ok\nuntil dialog 422";
        let load = LOAD.replace("row 5144 0", &format!("row 5144 {row}")).replace("until dialog 147", turn);
        let team_of = |runner: &Runner| {
            let state = runner.host.state();
            let records = state.records.as_ref().unwrap();
            let (slot, number) = state.season.as_ref().unwrap().manager().unwrap().club.unwrap();
            let nation = game.world.country(slot).unwrap().nation;
            records.team(nation, number as u16).unwrap().clone()
        };
        let (runner, result) = Runner::play(&game, &Script::parse(&load).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let before = team_of(&runner);
        let talks = a3_core::finances::season_talks(&before, runner.host.state().records.as_ref().unwrap().globals.preparation());
        drop(runner);
        let years = |t: &a3_core::records::Team, at: usize| t.get::<u8>(at + a3_core::finances::CLUB_TO_TEAM);
        let then = format!("{turn}\nnow click 2185\nnow ok\nuntil dialog 419");
        let load = LOAD.replace("row 5144 0", &format!("row 5144 {row}")).replace("until dialog 147", &then);
        let (runner, result) = Runner::play(&game, &Script::parse(&load).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        assert_eq!(runner.host.view().id, 419, "{}", runner.log.join("\n"));
        // The television's box kept for a club in Europe opens its talks,
        // as the original does on this save.
        let tv = talks[5].ticked && a3_core::finances::in_europe(&before);
        assert_eq!(runner.log.iter().any(|l| l == "screen 475"), tv, "{}", runner.log.join("\n"));
        assert_eq!(runner.log.iter().any(|l| l == "screen 384"), tv, "its OK asks first");
        let after = team_of(&runner);
        for (k, at) in [(0, 0x509), (1, 0x5a1)] {
            if talks[0].open && talks[k].open {
                assert_ne!(years(&after, at), 0, "the computer signed {at:#x}");
            } else {
                assert_eq!(years(&after, at), years(&before, at), "{at:#x} as it was");
            }
        }
        if talks[0].open {
            assert_ne!(after.get::<u16>(0x4ea + a3_core::finances::CLUB_TO_TEAM), 0, "the sponsor's number");
        }
        for (k, at) in [(2, 0x5b1), (4, 0x5a9)] {
            if talks[k].ticked {
                assert_eq!(years(&after, at), years(&before, at), "{at:#x} kept for the manager");
            }
        }
    }

    /// SeasonEnd364's European television talks (dialog 475): the
    /// country's three stations bid as the original draws them after the
    /// talks left to the computer (`FUN_005ba500`), a "Verhandeln" raises
    /// the left one's bid (`FUN_005ba04d`), and "Unterschreiben" on the
    /// middle one signs its sums into the club after the notice
    /// (`FUN_005ba88c`), the cycle booking the fixed sum where it does.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_european_tv_talks_bargain_and_sign() {
        use a3_core::settlement::{TvBid, tv_bargain, tv_bids, tv_terms};
        let game = game();
        let Some(row) = game.save_games().iter().position(|s| s.name == "SeasonEnd364") else {
            eprintln!("no SeasonEnd364 among the saves; the oracle's copy has it");
            return;
        };
        let turn = "on 152 press 2594\non 417 ok\non 301 ok\non 22 ok\non 89 press 1875\non 287 ok\non 321 ok\non 25 ok\non 341 ok\non 364 ok\non 120 ok\non 51 ok\non 392 ok\non 435 ok\non 96 ok\non 161 ok\nuntil dialog 422";
        let play = |then: &str| {
            let load = LOAD.replace("row 5144 0", &format!("row 5144 {row}")).replace("until dialog 147", &format!("{turn}{then}"));
            let (runner, result) = Runner::play(&game, &Script::parse(&load).unwrap());
            result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
            runner
        };
        let club = |runner: &Runner| {
            let state = runner.host.state();
            let (slot, number) = state.season.as_ref().unwrap().manager().unwrap().club.unwrap();
            let nation = game.world.country(slot).unwrap().nation;
            state.records.as_ref().unwrap().team_index(nation, number as u16).unwrap()
        };
        // What the talks draw: the computer's signing of the talks left
        // unticked first, then the bids, on the season's random.
        let runner = play("");
        let i = club(&runner);
        let (mut random, country, names, before, tv) = {
            let mut state = runner.host.state();
            let state = &mut *state;
            let records = state.records.as_mut().unwrap();
            let talks = a3_core::finances::season_talks(&records.teams[i], records.globals.preparation());
            let mut random = state.season.as_ref().unwrap().random.clone();
            a3_core::settlement::sign_left_talks(records, i, talks.map(|t| t.ticked), &mut random);
            let tv = talks[5].ticked && a3_core::finances::in_europe(&records.teams[i]);
            let country = records.teams[i].country();
            let names: Vec<String> =
                (0..3).map(|k| a3_core::report::country_station(records, country, k).unwrap()).collect();
            let at = a3_core::finances::CLUB_TO_TEAM;
            let before = [0x13c, 0x244].map(|o| records.teams[i].get::<i32>(o + at));
            (random, country, names, before, tv)
        };
        drop(runner);
        assert!(tv, "SeasonEnd364's club keeps the television's talks in Europe");
        let mut bids = tv_bids(&mut random);
        let mut kinds: Vec<u8> = bids.iter().map(|b| u8::from(b.fixed != 0) + 2 * u8::from(b.per_home != 0)).collect();
        kinds.sort_unstable();
        assert_eq!(kinds, [1, 2, 3], "one fixed sum, one per home match, one both");
        let money = |v: i32| crate::screens::playerinfo::money(&game, i64::from(v));
        let words = |b: &TvBid| {
            let (fixed, per_home) = tv_terms(b);
            match (fixed, per_home) {
                (f, 0) => format!("Fixbetrag {}", money(f)),
                (0, p) => format!("Pro Heimspiel {}", money(p)),
                (f, p) => format!("Fixbetrag {}, pro Heimspiel {}", money(f), money(p)),
            }
        };
        let dialog = &game.dialogs[&475];
        let at = |id: u16| crate::screens::control(dialog, id).unwrap();
        let runner = play("\nnow ok\nuntil dialog 475");
        let view = runner.host.view();
        assert_eq!(view.id, 475, "{}", runner.log.join("\n"));
        for (k, (name, offer)) in [(6051, 6052), (6055, 6056), (6053, 6054)].into_iter().enumerate() {
            assert_eq!(view.text(at(name)), names[k], "station {k} of {country}");
            assert_eq!(view.text(at(offer)), words(&bids[k]), "station {k}'s offer");
        }
        drop(runner);
        // One round with the left station: never its last.
        let was = bids[0];
        assert!(tv_bargain(&mut bids[0], &mut random));
        assert!(bids[0].fixed >= was.fixed && bids[0].per_home >= was.per_home);
        let runner = play("\nnow ok\nuntil dialog 475\nnow choose 6058 0");
        let view = runner.host.view();
        assert_eq!(view.text(at(6052)), words(&bids[0]), "the raised offer");
        assert_eq!(view.chosen(at(6058)), crate::controls::options::NONE, "the item let go");
        drop(runner);
        // Signed with the middle one: its notice, then its sums the club's.
        let runner = play("\nnow ok\nuntil dialog 475\nnow choose 6058 0\nnow choose 6059 1\nuntil dialog 419");
        assert!(runner.log.iter().any(|l| l == "screen 380"), "{}", runner.log.join("\n"));
        let state = runner.host.state();
        let team = &state.records.as_ref().unwrap().teams[i];
        let at = a3_core::finances::CLUB_TO_TEAM;
        let (fixed, per_home) = tv_terms(&bids[1]);
        assert_eq!(team.get::<i32>(0x5ec + at), per_home);
        assert_eq!(team.get::<u8>(0x5f0 + at), 1);
        let booked = [0x13c, 0x244].map(|o| team.get::<i32>(o + at));
        let kept = team.get::<i32>(0x5e8 + at);
        if kept == 0 {
            assert_eq!(booked, before.map(|b| b + fixed), "the fixed sum booked");
        } else {
            assert_eq!((kept, booked), (fixed, before), "the fixed sum kept");
        }
    }

    /// What the replica's new game still sets up otherwise than the
    /// original's, each the matter of a part of new-game-exact. The
    /// original's calculation builds the first season's job offers its
    /// own way; the replica's job market stands in, so Bert signs with
    /// another club than SSV Ulm and has its squad, its board's
    /// confidence and its training weeks in place of the original's
    /// (`new-game-exact-start-calc-offers`, parked on the grounds map's
    /// generator); in the records, the fields the calculation fills and
    /// the generator words it leaves behind part as far as
    /// `new-game-exact-start-calc-preseason-bookings-fields` is parked.
    /// The managers, their order, views and records stand as the
    /// original's (`a3_core::land_managers`, `a3_core::land_start`).
    const NEW_GAME_APART: [&str; 5] =
        ["blocks", "clubs", "confidence", "squads", "training weeks"];

    /// Where the replica's game at a new game's first turn parts from the
    /// original's dump of the same turn, by what: the date, the managers
    /// as a loaded game reads them (`a3_core::load::season`) — names,
    /// clubs, national teams, levels, confidence, views, formations, week
    /// templates and plans —, the human clubs' squads by name, and the
    /// records block by block (`an3::compare`, without the blocks of real
    /// time and of what the writer left on the stack).
    fn new_game_apart(
        state: &a3_core::state::GameState,
        dump: &a3_files::an3::Save,
        data: &a3_core::data::Data,
    ) -> std::collections::BTreeMap<&'static str, Vec<String>> {
        use a3_files::an3;
        let mut apart = std::collections::BTreeMap::<&'static str, Vec<String>>::new();
        let records = a3_core::records::Records::from_save(dump.clone()).unwrap();
        let oracle = a3_core::load::season(&records, data, a3_core::random::Random::new(0));
        let replica = state.season.as_ref().expect("the new game's season");
        let mut differ = |what: &'static str, a: String, b: String| {
            if a != b {
                apart.entry(what).or_default().push(format!("{a}, not {b}"));
            }
        };
        differ("date", replica.date().to_string(), oracle.date().to_string());
        // The managers by name; the order they take their turns in apart.
        let order = |season: &a3_core::season::Season| -> Vec<String> {
            season.managers.iter().map(|m| format!("{} {}", m.first_name, m.last_name)).collect()
        };
        differ("order", order(replica).join(", "), order(&oracle).join(", "));
        let club = |club: Option<(usize, usize)>| {
            club.and_then(|(slot, c)| Some(data.world.country(slot)?.clubs.get(c)?.name.clone()))
                .unwrap_or_else(|| "no club".into())
        };
        // A list's entries one by one: how many part, and the first.
        let entries = |a: Vec<String>, b: Vec<String>| -> (String, String) {
            let parting: Vec<usize> = (0..a.len().max(b.len())).filter(|&i| a.get(i) != b.get(i)).collect();
            let Some(&i) = parting.first() else { return (String::new(), String::new()) };
            let at = |list: &[String]| list.get(i).cloned().unwrap_or_else(|| "nothing".into());
            (format!("{} of {} entries apart, #{i} {}", parting.len(), a.len(), at(&a)), at(&b))
        };
        for r in &replica.managers {
            let name = format!("{} {}", r.first_name, r.last_name);
            let Some(o) = oracle.managers.iter().find(|o| format!("{} {}", o.first_name, o.last_name) == name) else {
                differ("names", name, "no such manager".into());
                continue;
            };
            let mut differ = |what, a: String, b: String| {
                if a != b {
                    differ(what, format!("{name}: {a}"), b);
                }
            };
            differ("clubs", club(r.club), club(o.club));
            differ("national teams", format!("{:?}", r.national_team), format!("{:?}", o.national_team));
            differ("levels", r.level.to_string(), o.level.to_string());
            differ("confidence", r.confidence.to_string(), o.confidence.to_string());
            let views = |m: &a3_core::season::Manager| {
                format!("{} {} {} {} {:?}", m.menu, m.list_view, m.training_pictures, m.individual_rule, m.camp_rules)
            };
            differ("views", views(r), views(o));
            let formations = |m: &a3_core::season::Manager| m.formations.iter().map(|f| format!("{f:?}")).collect();
            let (a, b) = entries(formations(r), formations(o));
            differ("formations", a, b);
            let templates = |m: &a3_core::season::Manager| m.templates.iter().map(|t| format!("{t:?}")).collect();
            let (a, b) = entries(templates(r), templates(o));
            differ("templates", a, b);
            let weeks = |m: &a3_core::season::Manager| m.weeks.iter().map(|w| format!("{w:?}")).collect();
            let (a, b) = entries(weeks(r), weeks(o));
            differ("training weeks", a, b);
        }
        let names = |players: &[a3_core::world::Player]| -> Vec<String> {
            players.iter().map(|p| format!("{} {}", p.first_name, p.last_name)).collect()
        };
        for (&at, o) in &oracle.clubs {
            let r = replica.clubs.get(&at).map_or_else(Vec::new, |r| names(&r.players));
            let (a, b) = entries(r, names(&o.players));
            differ("squads", if a.is_empty() { a } else { format!("{}: {a}", club(Some(at))) }, b);
        }
        match &state.records {
            None => differ("records", "none".into(), "the dump's".into()),
            Some(records) => {
                for d in an3::compare(&records.to_save(), dump).blocks {
                    if !an3::REAL_TIME.contains(&d.name.as_str()) && !an3::LEFTOVERS.contains(&d.name.as_str()) {
                        differ("blocks", d.to_string(), String::new());
                    }
                }
            }
        }
        apart
    }

    /// The new game of `scripts/new-game.txt` at its first turn (the
    /// line-up, dialog 147) against the original's dump of that turn
    /// (`tools/oracle/new-game.sh`, rand() seeded 1): whatever parts must
    /// be among [`NEW_GAME_APART`], and each of those must still part, so
    /// a part that brings its matter into line takes it off the list.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR and the oracle's dumps from $ANSTOSS3_ORACLE"]
    fn a_new_game_against_the_oracles_first_turn() {
        let Some(oracle) = std::env::var_os("ANSTOSS3_ORACLE") else {
            eprintln!("no oracle dump of a new game's first turn among the saves: ANSTOSS3_ORACLE is not set");
            return;
        };
        let path = PathBuf::from(oracle).join("runs/new-game/first-turn.an3");
        if !path.exists() {
            eprintln!("no oracle dump of a new game's first turn among the saves; tools/oracle/new-game.sh writes it");
            return;
        }
        let dump = a3_files::an3::Save::read(&path).unwrap();
        let game = game();
        let script: String = NEW_GAME.lines().filter(|l| !l.starts_with("until")).map(|l| format!("{l}\n")).collect();
        let (runner, result) = Runner::play(&game, &Script::parse(&format!("{script}until dialog 147")).unwrap());
        result.unwrap_or_else(|err| panic!("{err}\n{}", runner.log.join("\n")));
        let state = runner.host.state();
        let apart = new_game_apart(&state, &dump, &game.data);
        for (what, list) in &apart {
            eprintln!("{what}: {} apart, {:?}", list.len(), &list[..list.len().min(5)]);
        }
        for what in apart.keys() {
            assert!(NEW_GAME_APART.contains(what), "{what} parts from the original's new game: {:?}", apart[what]);
        }
        for what in NEW_GAME_APART {
            assert!(apart.contains_key(what), "{what} agrees with the original's new game now: take it off NEW_GAME_APART");
        }
    }
}
