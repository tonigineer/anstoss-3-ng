//! The interview after the match (dialog 383, `CInterviewDialog`): at the
//! matchday's end the first human club whose match draws one is asked
//! three or four questions by its TV station's reporter
//! (`FUN_00660a72`, `FUN_00666aea`, `FUN_00660895`). The questions come
//! from `texte/Interv.saq` ([`a3_files::interview`]); one may come when
//! the facts it needs hold — the facts of the match (`FUN_0066177d`), and
//! the questions not asked before — its topic is still open, the
//! reporter's manner and mood allow it, and it is not among the last
//! thirty of this run of the game; of those that may, the heaviest by
//! weight, one of them by chance (`FUN_0065fca8`, `FUN_0065fb7d`). Each
//! answer moves two things: the manager's popularity or the board's
//! confidence, the team's mood, a player's contentment, the reporter's
//! mood and manner, or where the talk goes next (`FUN_0065ff9e`). A
//! change of topic has the reporter say so first; the end, his thanks,
//! by his manner (`FUN_006607d0`), and his mood is kept for the next
//! time (`FUN_0065fb29`).
//!
//! The replica's matches are scored, not played: of the facts only those
//! of the score, the matchday, the table and the clubs hold here, the
//! ones of the match's events, the squad and the manager's record stay
//! off; no referee is drawn, and the match's best and worst men go
//! unnamed. Only league matches are looked at.

use std::io;

use a3_files::exe::Exe;
use a3_files::interview::Question;

use crate::mood::{self, MoodTables};
use crate::opponent::League;
use crate::random::Random;
use crate::records::{Records, Team};
use crate::season::Rules;

/// The facts' cases (`FUN_0066177d`'s switch at 0x6621d0): a byte for
/// each fact from 1, the case that decides it; 239 decides none.
const CASES_AT: u32 = 0x666924;
const CASES: usize = 0x1ae;
/// The reporter's texts (`DAT_0089fe6c`): five turns to another topic,
/// then the thanks, two by each of four manners.
const TEXTS_AT: u32 = 0x89fe6c;
const TEXTS: usize = 13;
const REMARKS: usize = 5;

/// The game's switch for the interviews (byte 0x17 of the switches at
/// `0xb5d13c`, `DAT_00b5d153`).
const SWITCH: usize = 0x17;
const ON: u8 = 1;

/// The topics a question may have (+0xc, 0x32 bytes).
pub const TOPICS: usize = 50;
/// The questions of this run of the game kept from coming again
/// (`DAT_00b621d8`, thirty).
pub type Recent = [i32; 30];
/// A question's facts, from -1000 to 999 (`+0x2f88`): a negative one is a
/// question of that id not asked yet.
const FACTS: usize = 1000;
/// The question that every answer may lead back to; asking it does not
/// mark it asked.
const REPEATABLE: i32 = 1;

/// The country's block (`0x19f7fd0[slot]`, saved from +0x20): its TV
/// stations, 0x32 bytes each from `DAT_019fab6a`: the station, the
/// reporter's first and last name, his manner, his mood with each human.
const STATIONS_AT: usize = 0x2b9a - 0x20;
const STATION: usize = 0x32;
const STATIONS: u8 = 2;
const REPORTER: (usize, usize) = (0x1c, 0x10);
const MANNER: usize = 0x2c;
const MOODS: usize = 0x2d;
/// The two experts behind them (`FUN_00681188`, 0x1c bytes each from
/// `DAT_019fac00`); the interview names the second half of one.
const EXPERTS_AT: usize = 0x2c30 - 0x20;
const EXPERT: usize = 0x1c;
const EXPERT_NAME: (usize, usize) = (0xc, 0x10);
/// The association's short name ("DFB", `FUN_00637f0c` at `DAT_019f8186`).
const ASSOCIATION: (usize, usize) = (0x1b6 - 0x20, 0x20);
/// Where the human's last match report notes the TV station
/// (`0xa53e38[slot]`, the copy's +0x60f in `FUN_0063f430`).
const REPORTS: u32 = 0xa53e38;
const REPORT_STATION: usize = 0x60f;
/// The team's place in the table and the byte behind it (club +0x109,
/// +0x10a), the class it stands at for its goal (club +0x10c), and its
/// rival (club +0xf4), by their team record's offsets (`FUN_0063b75a`).
const PLACE_AT: usize = 0x132;
const PLACE_NEXT: usize = 0x133;
const GOAL_CLASS: usize = 0x135;
const RIVAL_AT: usize = 0x122;

/// The effects' kinds (`FUN_0065ff9e`'s switch), and the players the
/// kinds from 18 to 34 name (+0x467c + kind × 2).
const FANS: i16 = 1;
const PUBLIC: i16 = 2;
const MEDIA: i16 = 3;
const MOOD: i16 = 4;
const MANNER_SET: i16 = 5;
const BOARD: i16 = 6;
const REFEREE_FORGIVES: i16 = 7;
const REFEREE_MINDS: i16 = 8;
const NEW_TOPIC: i16 = 9;
const TOPIC: i16 = 10;
const FOLLOW: i16 = 11;
const TEAM_MOOD: i16 = 13;
const STOP: i16 = 16;
const END: i16 = 17;
const PLAYERS: std::ops::Range<i16> = 18..35;
const KEEPER_KIND: i16 = 30;
/// A player effect's values that give him a reason instead
/// (`FUN_00770800`).
const HAPPY: (i16, u32) = (30, 0x80000);
const SNUBBED: (i16, u32) = (31, 0x1000);
/// The team's mood and what it does (`FUN_007147ce`, `FUN_00771701`).
const MOOD_MOST: i32 = 200;
const GOOD_MOOD: u32 = 0x40000;
const TEAM_GRIEVANCE: u32 = 0x80;
const CHEERFUL: u16 = 0x4;

/// The program's tables the interview reads.
pub struct Tables {
    cases: Vec<u8>,
    texts: Vec<u16>,
}

impl Tables {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        Ok(Self { cases: exe.bytes(CASES_AT, CASES)?.to_vec(), texts: exe.u16s(TEXTS_AT, TEXTS)? })
    }

}

/// A match of the day as the interview reads it: the sides, home first,
/// their goals, and its kind (`+0x20`: a league's matchday from 0, else
/// the cup's calendar code), and a second leg's first leg (`+0x08`,
/// `+0x09`, the away side's goals first).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fixture {
    pub sides: [(u8, i16); 2],
    pub goals: [i16; 2],
    pub kind: u32,
    pub leg: [i16; 2],
}

/// The league games just played, in the order their leagues played, each
/// league's in the order of its schedule.
pub fn fixtures(records: &Records, played: &[(u16, u8)]) -> Vec<Fixture> {
    let mut list = Vec::new();
    for &(country, level) in played {
        let Some(league) = League::of(records, country, level) else { continue };
        let Some(matchday) = league.played().checked_sub(1) else { continue };
        for (home, away, result) in league.games(matchday) {
            let Some(result) = result else { continue };
            let side = |place: usize| (country as u8, league.team(place));
            list.push(Fixture {
                sides: [side(home), side(away)],
                goals: [i16::from(result[0]), i16::from(result[1])],
                kind: matchday as u32,
                leg: [0; 2],
            });
        }
    }
    list
}

fn team<'a>(records: &'a Records, (country, number): (u8, i16)) -> Option<&'a Team> {
    records.team(country.into(), u16::try_from(number).ok()?)
}

/// The match of the day that draws an interview, and whether its human
/// side is the home one (`FUN_00660a72`): with the switch on, the matches
/// in their order, of each the home side if it is a human's, else the
/// away side if that is; the first that draws one is the only one.
pub fn due(records: &Records, fixtures: &[Fixture], random: &mut Random) -> Option<(Fixture, bool)> {
    if records.globals.game_options()[SWITCH] != ON {
        return None;
    }
    let human = |(country, number): (u8, i16)| crate::statistics::is_human(records, country, number);
    fixtures.iter().find_map(|f| {
        let home = if human(f.sides[0]) {
            true
        } else if human(f.sides[1]) {
            false
        } else {
            return None;
        };
        draws(records, f, home, random).then_some((*f, home))
    })
}

/// Whether a league match draws an interview for its human side
/// (`FUN_00666aea`): a match among the top six, one time in four; a
/// win or a defeat by three, one in five; a first league's club at the
/// top two or the bottom three, one in thirty-three. A national cup tie
/// rolls by [`cup_draws`]; the European cups' rolls are not built.
fn draws(records: &Records, f: &Fixture, home: bool, random: &mut Random) -> bool {
    if f.kind & 0x7f00_0000 == NATIONAL_CUP {
        return cup_draws(f, home, random);
    }
    if f.kind & 0x7f00_0000 >= 0x3f {
        return false;
    }
    let place = |side: (u8, i16)| team(records, side).map_or(0, |t| i32::from(t.get::<u8>(PLACE_AT)));
    let (home_place, away_place) = (place(f.sides[0]), place(f.sides[1]));
    let own = f.sides[usize::from(!home)];
    let Some(club) = team(records, own) else { return false };
    let size = crate::league::teams(records, club.country(), club.level());
    let sum = home_place + away_place;
    if 0 < sum && sum < 7 && random.below(100) < 25 {
        return true;
    }
    if random.below(100) < 20 && (f.goals[0] - f.goals[1]).abs() >= 3 {
        return true;
    }
    if club.level() == 0 && random.below(100) < 3 {
        let place = if home { home_place } else { away_place };
        if place < 3 || size - 3 < place {
            return true;
        }
    }
    false
}

/// The national cup's kind, and a second leg's flag in it.
const NATIONAL_CUP: u32 = 0x4000_0000;
const SECOND_LEG: u32 = 0x8000_0000;

/// Whether a national cup tie draws an interview (`FUN_00666aea`'s
/// 0x40000000 branch): fifteen times in a hundred; else, the human side
/// behind over the tie (`FUN_00414f22`, both legs' goals in a second leg
/// by `FUN_00414e9f`), one time in three. The record's +0x26 of 7 draws
/// one always; what it holds is not mapped yet, so that rule waits.
fn cup_draws(f: &Fixture, home: bool, random: &mut Random) -> bool {
    if random.below(100) < 15 {
        return true;
    }
    let mut diff = f.goals[0] - f.goals[1];
    if f.kind & SECOND_LEG != 0 {
        diff += f.leg[1] - f.leg[0];
    }
    let own = if home { diff } else { -diff };
    own < 0 && random.below(3) == 0
}

/// What the dialog shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Page {
    /// A question and the three answers drawn for it, waiting for one.
    Question { text: String, answers: [String; 3] },
    /// The reporter turns to another topic (a text, "%s" the manager's
    /// name); two seconds, then the question.
    Remark(u16),
    /// His thanks, the same way, and OK.
    Closing(u16),
}

/// What the facts are reckoned from (`FUN_0066177d`'s locals).
struct Match {
    /// The goals, the club's first, and their difference.
    own: i16,
    opp: i16,
    diff: i16,
    /// The league's matchday from 1, -1 for another match (`local_13c`).
    md: i16,
    /// A match between two of the six best (`local_150`).
    top: bool,
    /// The club's league: its teams and its matchdays (`FUN_00692a88`,
    /// `FUN_00692997`).
    teams: i32,
    matchdays: i32,
    kind: u32,
    place: i32,
    opp_place: i32,
    level: u8,
    country: u8,
    opp_country: u8,
    /// How far the place lies past the goal's class, short of it below
    /// nought (`FUN_00775ed8`, `FUN_00775f0a`).
    goal: i32,
    home: bool,
    rival: i8,
}

/// Whether a fact holds, by its case (the cases built; the rest wait for
/// the match's events and the records behind them).
fn holds(case: u8, m: &Match) -> bool {
    let md = i32::from(m.md);
    let league_day = md > 0 || m.kind & 0x8008_0000 != 0x8_0000;
    match case {
        0x01 => m.diff < 0,
        0x05 => true,
        0x07 => m.diff < 1,
        0x08 => m.opp_country == 0x1c && m.diff > 0,
        0x0a => m.diff == 1,
        0x0e => 0 < md && md < 10,
        0x11 => m.goal < 0,
        0x13 => m.diff < -2,
        0x14 => md > 0 && m.place < 10,
        0x1a => m.goal >= 0,
        0x1d => 0 < md && md < 6,
        0x20 => m.opp > 3,
        0x23 => md > 0 && m.level == 1,
        0x28 => m.opp > 2,
        0x29 => md > 3,
        0x2c => m.kind & 0x7f08_0000 == 0x5000_0000,
        0x2d => league_day && m.home,
        0x2f => m.diff == 1 || m.diff == 2,
        0x33 => m.own == 0 && m.opp > 0,
        0x34 => m.diff > 0,
        0x36 => m.diff == -1,
        0x37 => md > 0 && md <= m.matchdays - 5,
        0x38 => md > 0 && m.place < 6,
        0x39 => md > 0 && m.place == 1,
        0x3b => 0 < md && md < 0x14,
        0x3c => league_day && !m.home,
        0x3f => m.own == 0 && m.opp == 0,
        0x42 => m.own == 0,
        0x49 => md > 0 && m.level == 0,
        0x4a => m.diff > 2,
        0x4c => m.diff == 0,
        0x4e => m.kind & 0x7f08_0000 == 0x5000_0000 && m.diff == 0,
        0x4f => m.kind & 0xff00_0000 == 0xd000_0000,
        0x50 => md > 10,
        0x52 => m.top && m.diff < 0,
        0x54 => md > 0 && m.place > 1,
        0x5d => md > 0 && m.teams - 2 <= m.place,
        0x66 => md > 0x19,
        0x68 => m.kind & 0x8_0000 != 0,
        0x69 | 0xaf => md > 0,
        0x6d => m.opp_country == 0x2f,
        0x6e => m.opp_country == 0x18,
        0x6f => md > 0 && m.teams - 2 <= m.opp_place,
        0x73 => md > 0 && md == m.matchdays,
        0x77 => md > 0 && md == m.matchdays - 5,
        0x7d => m.top,
        0x7f | 0x97 => md == 1,
        0x80 => md > 0 && (m.place - m.teams / 2).abs() < 4,
        0x85 => m.diff < -4,
        0x86 => m.opp_country == 0x10 && m.diff > 0,
        0x91 => md == m.matchdays && m.level == 1 && m.place < 3,
        0x94 => md > 0 && 6 < m.place && m.place < 10,
        0xa1 => m.kind & 0xff08_0000 == 0x4008_0000,
        0xa6 => md > 0 && m.level != 0,
        0xab => md == 0xd,
        0xac => md > 0 && md == m.matchdays - 1 && m.place < m.teams,
        0xc0 => md == 3,
        0xcb => md > 0 && md == m.teams,
        0xd6 => md > 0 && md == m.matchdays / 2,
        0xdb => m.country == 0x0e,
        0xdc => 0 < md && md < 0x1e,
        0xdf => m.goal == 0,
        0xe2 => md > 0 && md <= m.matchdays / 2,
        0xe8 => m.country != 0x10,
        0xee => m.rival >= 0,
        _ => false,
    }
}

/// An interview while it runs: the original's object of `FUN_00660895`
/// by its offsets.
#[derive(Clone, Debug, PartialEq)]
pub struct Interview {
    pub page: Page,
    /// The question a remark stands before.
    waiting: Option<Page>,
    /// The manager's first and last name (`DAT_00b62284`), for the
    /// caption and the reporter's "%s".
    pub manager_name: String,
    club: (u8, i16),
    manager: u8,
    station: usize,
    questions: Vec<Question>,
    /// The reporter's texts (`DAT_0089fe6c`).
    texts: Vec<u16>,
    /// How many questions there are to be, and the one at (from 1).
    count: usize,
    at: usize,
    /// The reporter's mood, 0 to 5 or -1 not minded (+0), and manner (+1).
    mood: i8,
    manner: i8,
    /// The topic of the last question (+8), and the topics still open (+0xc).
    last_topic: i32,
    topics: [bool; TOPICS],
    /// The end asked for (+0x70), the topic asked for (+0x74) once the
    /// topic is to change (+0x78), the question to follow (+0x7c).
    end: i32,
    topic: i32,
    change: i32,
    follow: i32,
    /// The remarks' order (+0x80, five and the short behind them) and the
    /// next one (+0x8c).
    remarks: [i16; REMARKS + 1],
    remark_at: usize,
    /// The interview's last four questions (+0x90).
    last_four: [i32; 4],
    /// The facts that hold, from 1 (+0x3370), and the questions not asked
    /// yet, by id (+0x2f88 downwards).
    facts: Vec<bool>,
    not_asked: Vec<bool>,
    /// The names "N01" … "N17" stand for (+0x3958, 200 bytes each).
    names: [String; 17],
    /// The players the effects' kinds 18 to 34 move (+0x46a0).
    players: [u16; 17],
    /// The question up and its answers' order.
    current: Option<(usize, [usize; 3])>,
}

impl Interview {
    /// Sets up the interview of a club for its match (`FUN_0065ee08`,
    /// `FUN_00660895`, `FUN_0065f390`): three questions or four; none
    /// without the questions' file.
    pub fn begin(
        records: &Records,
        rules: &Rules,
        questions: Option<Vec<Question>>,
        (fixture, home): (Fixture, bool),
        recent: &mut Recent,
        random: &mut Random,
    ) -> Option<Self> {
        let count = 4 - usize::from(random.below(10) < 6);
        let questions = questions?;
        let side = usize::from(!home);
        let (club, foe) = (fixture.sides[side], fixture.sides[1 - side]);
        let own = team(records, club)?;
        let other = team(records, foe)?;
        let manager = own.humans()[0];
        let boss = records.managers.get(usize::from(manager).checked_sub(1)?)?;
        let slot = records.countries.slot(usize::from(club.0));
        let block = records.block(&format!("0x19f7fd0[{slot}]"));
        // The station the human's report names, at most the third.
        let station = records
            .block(&format!("{REPORTS:#x}[{}]", own.human().saturating_sub(1)))
            .and_then(|b| b.get(REPORT_STATION))
            .map_or(0, |&s| s.min(STATIONS));
        let station_at = STATIONS_AT + usize::from(station) * STATION;
        let byte = |at: usize| block.and_then(|b| b.get(at)).copied().unwrap_or(0);
        let text_at = |(at, len): (usize, usize)| block.and_then(|b| b.get(at..at + len)).map_or(String::new(), |b| crate::records::text(b, len));
        let mut me = Self {
            page: Page::Closing(0),
            waiting: None,
            manager_name: format!("{} {}", boss.first_name(), boss.last_name()),
            club,
            manager,
            station: station_at,
            questions,
            texts: rules.interview.texts.clone(),
            count,
            at: 0,
            mood: byte(station_at + MOODS + usize::from(manager) - 1) as i8,
            manner: byte(station_at + MANNER) as i8,
            last_topic: -1,
            topics: [true; TOPICS],
            end: 0,
            topic: -1,
            change: 0,
            follow: -1,
            remarks: [0; REMARKS + 1],
            remark_at: 0,
            last_four: [-1; 4],
            facts: vec![false; FACTS],
            not_asked: vec![true; FACTS + 1],
            names: Default::default(),
            players: [0; 17],
            current: None,
        };
        for (k, place) in crate::cup_draw::order(REMARKS, random).into_iter().enumerate() {
            me.remarks[k] = place as i16;
        }
        me.facts[0] = true;
        me.not_asked[1] = false;
        let m = Match {
            own: fixture.goals[side],
            opp: fixture.goals[1 - side],
            diff: fixture.goals[side] - fixture.goals[1 - side],
            md: if fixture.kind & 0x7f00_0000 < 0x3f { ((fixture.kind & 0x3f) + 1) as i16 } else { -1 },
            top: false,
            teams: crate::league::teams(records, own.country(), own.level()),
            matchdays: matchdays(records, own),
            kind: fixture.kind,
            place: i32::from(own.get::<u8>(PLACE_AT)),
            opp_place: i32::from(other.get::<u8>(PLACE_AT)),
            level: own.level(),
            country: own.country(),
            opp_country: other.country(),
            goal: rules.goal_verdict(
                crate::league::teams(records, own.country(), own.level()),
                i32::from(own.get::<u8>(PLACE_AT)),
                i32::from(own.get::<i8>(GOAL_CLASS)),
            ),
            home,
            rival: own.get::<i8>(RIVAL_AT),
        };
        let m = Match { top: m.md > 1 && u32::from(own.get::<u8>(PLACE_NEXT)) + u32::from(other.get::<u8>(PLACE_NEXT)) < 7, ..m };
        for (fact, &case) in rules.interview.cases.iter().enumerate() {
            if holds(case, &m) {
                me.facts[fact + 1] = true;
            }
        }
        // The names: the manager, the association, the rival, the
        // reporter, the ground's town, an expert; the keeper of the
        // line-up. The referee and the match's men wait for the match.
        me.names[1] = me.manager_name.clone();
        let keeper = own.lineup()[0];
        me.names[4] = records.known_as(keeper).filter(|_| keeper != 0).unwrap_or_default();
        me.names[6] = text_at(ASSOCIATION);
        if m.rival >= 0 {
            me.names[9] = records.team(club.0.into(), m.rival as u16).map(Team::name).unwrap_or_default();
        }
        me.names[10] = text_at((station_at + REPORTER.0, REPORTER.1));
        let ground = fixture.sides[0];
        me.names[11] = crate::opponent::stadium(records, ground.0.into(), ground.1 as u16)
            .map(|s| s.town.clone())
            .unwrap_or_default();
        let expert = EXPERTS_AT + random.below(2) as usize * EXPERT;
        me.names[16] = text_at((expert + EXPERT_NAME.0, EXPERT_NAME.1));
        me.players[usize::from((KEEPER_KIND - PLAYERS.start) as u16)] = keeper;
        me.next(recent, random);
        Some(me)
    }

    fn close_topic(&mut self) {
        if let Some(t) = usize::try_from(self.last_topic).ok().and_then(|t| self.topics.get_mut(t)) {
            *t = false;
        }
    }

    fn fact(&self, need: i16) -> bool {
        match need {
            0 => true,
            n if n > 0 => self.facts.get(n as usize).copied().unwrap_or(false),
            n => self.not_asked.get(usize::from(n.unsigned_abs())).copied().unwrap_or(false),
        }
    }

    /// Whether a question may come (`FUN_0065fb7d`).
    fn may(&self, q: &Question, open: &[bool; TOPICS], recent: &Recent) -> bool {
        q.needs.iter().all(|&n| self.fact(n))
            && usize::try_from(q.topic).ok().and_then(|t| open.get(t)).copied().unwrap_or(false)
            && (self.manner < 1 || q.manner < 1 || self.manner == q.manner)
            && !(self.mood >= 0 && q.moods[0] <= self.mood && self.mood <= q.moods[1])
            && !recent.contains(&q.id)
            && !self.last_four.contains(&q.id)
    }

    /// The questions that may come, the heaviest first, and how many share
    /// the first one's weight; `None` for the question to follow, which
    /// comes alone (`FUN_0065fca8`).
    fn candidates(&mut self, recent: &Recent) -> (Vec<usize>, Option<usize>) {
        if self.follow >= 0
            && let Some(k) = self.questions.iter().position(|q| q.id == self.follow)
        {
            self.follow = -1;
            return (vec![k], None);
        }
        if self.change > 0 && self.last_topic >= 0 {
            self.close_topic();
        }
        let (mut open, mut pass) = if self.change == 0 || self.topic < 0 {
            (self.topics, 1)
        } else {
            let mut only = [false; TOPICS];
            if let Some(t) = usize::try_from(self.topic).ok().and_then(|t| only.get_mut(t)) {
                *t = true;
            }
            (only, 0)
        };
        while pass <= 2 {
            let mut list: Vec<usize> =
                (0..self.questions.len()).filter(|&k| self.may(&self.questions[k], &open, recent)).collect();
            // The original's exchange sort, the heavier to the front.
            for i in 0..list.len() {
                for j in i + 1..list.len() {
                    if self.questions[list[i]].weight < self.questions[list[j]].weight {
                        list.swap(i, j);
                    }
                }
            }
            let top = list.first().map(|&k| self.questions[k].weight);
            let count = list.iter().take_while(|&&k| Some(self.questions[k].weight) == top).count();
            if count > 0 {
                return (list, Some(count));
            }
            match pass {
                0 => open = self.topics,
                1 => open = [true; TOPICS],
                _ => self.mood = -1,
            }
            pass += 1;
        }
        (Vec::new(), Some(0))
    }

    /// "N01" … for its name (`FUN_0065f249`): the number read on, the two
    /// characters after the N passed over.
    fn fill(&self, text: &str) -> String {
        let chars: Vec<char> = text.chars().collect();
        let mut out = String::new();
        let mut k = 0;
        while k < chars.len() {
            if chars[k] == 'N' && chars.get(k + 1).is_some_and(char::is_ascii_digit) {
                let digits: String = chars[k + 1..].iter().take_while(|c| c.is_ascii_digit()).collect();
                let n: usize = digits.parse().unwrap_or(0);
                if let Some(name) = n.checked_sub(1).and_then(|i| self.names.get(i)) {
                    out.push_str(name);
                }
                k += 3;
            } else {
                out.push(chars[k]);
                k += 1;
            }
        }
        out
    }

    /// The next question that comes, up to the last one, or the thanks
    /// (`FUN_00660895`'s loop over `FUN_0065ff9e`).
    fn next(&mut self, recent: &mut Recent, random: &mut Random) {
        while self.at < self.count {
            self.at += 1;
            if self.ask(recent, random) {
                return;
            }
            if self.end != 0 {
                break;
            }
        }
        self.close(random);
    }

    /// The question at `self.at`, if one may come (`FUN_0065ff9e` up to
    /// its wait): the reporter's turn first where the topic changes.
    fn ask(&mut self, recent: &mut Recent, random: &mut Random) -> bool {
        let (list, count) = self.candidates(recent);
        let followed = count.is_none();
        let count = count.unwrap_or(1);
        if count == 0 {
            return false;
        }
        let k = list[random.below(count as u32) as usize];
        let (id, topic) = (self.questions[k].id, i32::from(self.questions[k].topic));
        recent.rotate_left(1);
        recent[recent.len() - 1] = id;
        self.last_four.rotate_left(1);
        self.last_four[3] = id;
        if id > REPEATABLE
            && let Some(flag) = usize::try_from(id).ok().and_then(|i| self.not_asked.get_mut(i))
        {
            *flag = false;
        }
        self.waiting = None;
        let mut remark = None;
        if self.at != 1 && self.last_topic != topic && !followed {
            // The index moves on twice: one remark for the picture, the
            // next for the words.
            self.remark_at += 1;
            let text = self.remarks.get(self.remark_at).copied().unwrap_or(0);
            self.remark_at += 1;
            remark = Some(Page::Remark(self.texts.get(usize::try_from(text).unwrap_or(0)).copied().unwrap_or(0)));
            self.close_topic();
        }
        self.last_topic = topic;
        let order = crate::cup_draw::choose(3, 4, random);
        let order = [order[0], order[1], order[2]];
        let q = &self.questions[k];
        let asked = if q.texts[1].is_empty() { &q.texts[0] } else { &q.texts[random.below(2) as usize] };
        let page = Page::Question {
            text: self.fill(asked),
            answers: order.map(|o| self.fill(&q.answers[o])),
        };
        self.current = Some((k, order));
        match remark {
            Some(r) => {
                self.page = r;
                self.waiting = Some(page);
            }
            None => self.page = page,
        }
        true
    }

    /// The reporter's thanks by his manner (`FUN_006607d0`, the table's
    /// 5 + 2 × class, one of two).
    fn close(&mut self, random: &mut Random) {
        const ORDER: [i8; 13] = [5, 12, 1, 0, 11, 8, 7, 6, 4, 10, 9, 2, 3];
        let rank = ORDER.iter().position(|&m| m == self.manner).unwrap_or(0);
        let class = match rank {
            0..3 => 0,
            3..6 => 1,
            6..11 => 2,
            11..13 => 3,
            _ => 0,
        };
        let k = random.below(2) as usize + REMARKS + 2 * class;
        self.current = None;
        self.waiting = None;
        self.page = Page::Closing(self.texts.get(k).copied().unwrap_or(0));
    }

    /// The remark's two seconds are up, or it was clicked away.
    pub fn go_on(&mut self) {
        if let Some(page) = self.waiting.take() {
            self.page = page;
        }
    }

    /// The answer given, of the three shown (`FUN_0065ff9e` from its
    /// wait): its two effects, then the next question or the thanks.
    pub fn answer(&mut self, choice: usize, records: &mut Records, tables: &MoodTables, recent: &mut Recent, random: &mut Random) {
        let Some((k, order)) = self.current.take() else { return };
        let Some(&answer) = order.get(choice) else { return };
        let effects = self.questions[k].effects[answer];
        for (kind, value) in effects {
            self.effect(kind, value, records, tables);
        }
        if self.end == 0 && self.at == self.count {
            self.end = 1;
        }
        if self.end != 0 {
            self.close(random);
        } else {
            self.next(recent, random);
        }
    }

    fn effect(&mut self, kind: i16, value: i16, records: &mut Records, tables: &MoodTables) {
        let v = i32::from(value);
        match kind {
            FANS => self.standing([0, v, 0], 0, records),
            PUBLIC => self.standing([v, 0, 0], 0, records),
            MEDIA => self.standing([0, 0, v], 0, records),
            BOARD => self.standing([0; 3], v, records),
            MOOD => self.mood = (i32::from(self.mood) + v).clamp(0, 5) as i8,
            MANNER_SET => self.manner = value as i8,
            // The referee's grudge against the club: no referee here.
            REFEREE_FORGIVES | REFEREE_MINDS => {}
            NEW_TOPIC => {
                self.change = 1;
                self.topic = -1;
            }
            TOPIC => {
                self.change = 1;
                self.topic = v;
            }
            FOLLOW => self.follow = v,
            TEAM_MOOD => team_mood(records, tables, self.club, v),
            STOP => self.end = -1,
            END => self.end = 1,
            k if PLAYERS.contains(&k) => {
                let id = self.players[usize::from((k - PLAYERS.start) as u16)];
                let own = team(records, self.club).is_some_and(|t| id != 0 && t.players().contains(&id));
                if let (true, Some(r)) = (own, records.player_mut(id)) {
                    match value {
                        v if v == HAPPY.0 => tables.set_reasons(r, HAPPY.1),
                        v if v == SNUBBED.0 => tables.set_reasons(r, SNUBBED.1),
                        _ => mood::change_trainer(r, v),
                    }
                }
            }
            _ => {}
        }
    }

    /// The manager's popularity with the public, the fans and the media,
    /// and the board's confidence (`FUN_0041849a`), each within 0 and 100.
    fn standing(&self, by: [i32; 3], confidence: i32, records: &mut Records) {
        let Some(m) = usize::from(self.manager).checked_sub(1).and_then(|i| records.managers.get_mut(i)) else {
            return;
        };
        let liked = m.popularity();
        m.set_popularity(std::array::from_fn(|k| (i32::from(liked[k]) + by[k]).clamp(0, 100) as u8));
        m.set_confidence((i32::from(m.confidence()) + confidence).clamp(0, 100) as i16);
    }

    /// The interview is over: the reporter keeps his mood with the
    /// manager for the next time, unless he stopped minding it
    /// (`FUN_0065fb29`, `FUN_00681142`).
    pub fn finish(&self, records: &mut Records) {
        if self.mood < 0 {
            return;
        }
        let slot = records.countries.slot(usize::from(self.club.0));
        let at = self.station + MOODS + usize::from(self.manager) - 1;
        if let Some(byte) = records.block_mut(&format!("0x19f7fd0[{slot}]")).and_then(|b| b.get_mut(at)) {
            *byte = self.mood as u8;
        }
    }
}

/// The club's league's matchdays (`FUN_00692997`): 18 where the level has
/// no league of its own, 34 past the country's leagues.
fn matchdays(records: &Records, team: &Team) -> i32 {
    if team.level() >= 6 || !records.globals.played(i16::from(team.country())) {
        return 0x12;
    }
    League::of(records, team.country().into(), team.level())
        .filter(|_| i32::from(team.level()) < records.countries.leagues(usize::from(team.country())) as i32)
        .map_or(0x22, |l| l.matchdays() as i32)
}

/// The team's mood moved, and what it does to the squad of a human's
/// club (`FUN_007147ce`, `FUN_00771701`).
pub(crate) fn team_mood(records: &mut Records, tables: &MoodTables, (country, number): (u8, i16), by: i32) {
    let Some(team) = records.team_mut(country.into(), number as u16) else { return };
    let mood = (i32::from(team.mood()) + by).clamp(0, MOOD_MOST) as u8;
    team.set_mood(mood);
    for id in team.players() {
        let Some(r) = records.players.get_mut(usize::from(id)) else { continue };
        if mood >= 151 {
            if r.traits()[1] & CHEERFUL != 0 {
                tables.set_reasons(r, GOOD_MOOD);
            }
        } else if mood >= 50 {
            mood::clear_reasons(r, TEAM_GRIEVANCE);
            mood::clear_reasons(r, GOOD_MOOD);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3_files::an3::Save;

    fn setup() -> Option<(std::path::PathBuf, Records, Rules)> {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR")?);
        let keeps = dir.parent().unwrap().join("saves-kept");
        let records = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let rules = Rules::load(&Exe::open(&dir.join("anstoss3.exe")).unwrap()).unwrap();
        Some((dir, records, rules))
    }

    /// Union (14, 84), A's human club, eleventh: a match it wins 4:0 at
    /// home draws an interview on some of the rolls, never with the
    /// switch off, and a match of two computer clubs never does.
    /// A national cup tie draws one fifteen times in a hundred, and a tie
    /// the human side is behind in, over both legs, a third of the rest
    /// besides.
    #[test]
    fn a_cup_tie_lost_draws_an_interview_more_often() {
        let rate = |f: Fixture, home: bool| {
            (0..3000u64).filter(|&seed| cup_draws(&f, home, &mut Random::new(seed))).count()
        };
        let won = Fixture { sides: [(14, 22), (14, 84)], goals: [2, 0], kind: 0x4000_0003, leg: [0; 2] };
        let (home_won, away_lost) = (rate(won, true), rate(won, false));
        assert!((300..600).contains(&home_won), "{home_won}");
        assert!((1100..1500).contains(&away_lost), "{away_lost}");
        // Won 1:0 at home after 0:3 away: behind over the tie.
        let second = Fixture { goals: [1, 0], kind: 0xc000_0003, leg: [3, 0], ..won };
        assert!((1100..1500).contains(&rate(second, true)));
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_big_win_draws_an_interview_now_and_then() {
        let Some((_, mut records, _)) = setup() else { return };
        let union = Fixture { sides: [(14, 84), (14, 22)], goals: [4, 0], kind: 3, leg: [0; 2] };
        let others = Fixture { sides: [(14, 22), (14, 20)], goals: [5, 0], kind: 3, leg: [0; 2] };
        let drawn = (0..200).filter(|&s| due(&records, &[others, union], &mut Random::new(s)).is_some()).count();
        assert!((10..80).contains(&drawn), "{drawn} of 200");
        assert!((0..200).all(|s| due(&records, &[others], &mut Random::new(s)).is_none()));
        let mut options = records.globals.game_options();
        options[SWITCH] = 0;
        records.globals.set_game_options(options);
        assert!((0..200).all(|s| due(&records, &[union], &mut Random::new(s)).is_none()));
    }

    /// Union lost 0:3 away on matchday 4: the questions that come need
    /// only facts that hold for it, each answer moves what its effects
    /// say, and after three or four the reporter thanks; his mood stays
    /// in the station's record.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_interview_asks_and_its_answers_count() {
        let Some((dir, mut records, rules)) = setup() else { return };
        let tables = MoodTables::load(&Exe::open(&dir.join("anstoss3.exe")).unwrap()).unwrap();
        let fixture = Fixture { sides: [(14, 22), (14, 84)], goals: [3, 0], kind: 3, leg: [0; 2] };
        let mut recent = Recent::default();
        let mut random = Random::new(11);
        let questions = a3_files::interview::load(&dir);
        let mut interview =
            Interview::begin(&records, &rules, questions, (fixture, false), &mut recent, &mut random).unwrap();
        assert!(interview.manager_name.contains(' '), "{}", interview.manager_name);
        // A defeat: fact 2 holds, 61 (a win) does not; no question asked yet.
        assert!(interview.fact(2) && interview.fact(9) && !interview.fact(61) && interview.fact(-5));
        let mut asked = 0;
        while let Page::Question { .. } | Page::Remark(_) = interview.page {
            if let Page::Remark(id) = interview.page {
                assert!((4974..=4978).contains(&id), "{id}");
                interview.go_on();
                continue;
            }
            let (k, order) = interview.current.unwrap();
            let q = interview.questions[k].clone();
            assert!(q.needs.iter().all(|&n| n <= 0 || interview.fact(n)), "{q:?}");
            let manager = &records.managers[usize::from(interview.manager) - 1];
            let before = (manager.popularity(), manager.confidence(), records.team(14, 84).unwrap().mood());
            interview.answer(0, &mut records, &tables, &mut recent, &mut random);
            asked += 1;
            assert!(!interview.fact(-(q.id as i16)) || q.id <= REPEATABLE);
            let manager = &records.managers[usize::from(interview.manager) - 1];
            let after = (manager.popularity(), manager.confidence(), records.team(14, 84).unwrap().mood());
            let mut want = (before.0.map(i32::from), i32::from(before.1), i32::from(before.2));
            for (kind, v) in q.effects[order[0]] {
                let v = i32::from(v);
                match kind {
                    PUBLIC => want.0[0] = (want.0[0] + v).clamp(0, 100),
                    FANS => want.0[1] = (want.0[1] + v).clamp(0, 100),
                    MEDIA => want.0[2] = (want.0[2] + v).clamp(0, 100),
                    BOARD => want.1 = (want.1 + v).clamp(0, 100),
                    TEAM_MOOD => want.2 = (want.2 + v).clamp(0, MOOD_MOST),
                    _ => {}
                }
            }
            assert_eq!((after.0.map(i32::from), i32::from(after.1), i32::from(after.2)), want, "{q:?}");
        }
        let Page::Closing(id) = interview.page else { unreachable!() };
        assert!((4979..=4986).contains(&id), "{id}");
        assert!((1..=interview.count).contains(&asked) && (3..=4).contains(&interview.count));
        assert!(recent.iter().filter(|&&id| id != 0).count() >= asked);
        interview.mood = 4;
        interview.finish(&mut records);
        let slot = records.countries.slot(14);
        let block = records.block(&format!("0x19f7fd0[{slot}]")).unwrap();
        assert_eq!(block[interview.station + MOODS + usize::from(interview.manager) - 1], 4);
    }
}
