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
//! The match's record the facts read is the human's report of it, where
//! `FUN_00550f29` copied it at the match's end: the half's score, extra
//! time and penalties, the crowd and the tie hold here; the sides'
//! chances, cards and marks, the score at the clock's marks and the
//! weather only over a record the engine wrote, and the replica's matches
//! are scored by a stand-in that leaves them at nought. The manager's
//! record, the club's money, its squad (ages, freshness, condition,
//! contentment, injuries, bans, its five best and whether they stood the
//! match) and its run of league results give their facts from the save;
//! the men the record names stay off; no referee is drawn, and the
//! match's best and worst men go unnamed. League matches and the
//! national cup's ties are looked at.

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
    /// Whether its record holds what the match engine writes into it:
    /// the sides' counters, the score at the clock's marks and the
    /// weather. The replica's stand-in scores its matches and leaves
    /// them at nought, so its fixtures say false and the facts read from
    /// them stay off.
    pub engine: bool,
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
                engine: false,
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
/// rolls by [`cup_draws`], a Champions League match by
/// [`champions_draws`]; the other European cups never roll.
fn draws(records: &Records, f: &Fixture, home: bool, random: &mut Random) -> bool {
    if f.kind & 0x7f00_0000 == NATIONAL_CUP {
        return cup_draws(f, home, random);
    }
    if f.kind & 0x7f00_0000 == CHAMPIONS_LEAGUE {
        return champions_draws(f, random);
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

/// The Champions League's kind, and its one-leg flag.
const CHAMPIONS_LEAGUE: u32 = 0x5000_0000;
const ONE_LEG: u32 = 0x0008_0000;

/// Whether a Champions League match draws an interview (`FUN_00666aea`'s
/// 0x50000000 branch): ten times in a hundred; else a one-leg match,
/// twenty in a hundred, and always unless it is a second leg.
fn champions_draws(f: &Fixture, random: &mut Random) -> bool {
    if random.below(100) < 10 {
        return true;
    }
    if f.kind & ONE_LEG != 0 && random.below(100) < 20 {
        return true;
    }
    f.kind & (SECOND_LEG | ONE_LEG) == ONE_LEG
}

/// Whether a national cup tie draws an interview (`FUN_00666aea`'s
/// 0x40000000 branch): fifteen times in a hundred; else, the human side
/// behind over the tie (`FUN_00414f22`, both legs' goals in a second leg
/// by `FUN_00414e9f`), one time in three. The record's +0x26 of 7 draws
/// one always; what it holds is not mapped yet, so that rule waits.
fn cup_draws(f: &Fixture, home: bool, random: &mut Random) -> bool {
    if random.below(100) < 15 {
        return true;
    }
    tie(f, home) < 0 && random.below(3) == 0
}

/// What the human side leads the tie by (`FUN_00414f22`): the match's
/// goals, a second leg's with the first leg's (`FUN_00414e9f`).
fn tie(f: &Fixture, home: bool) -> i16 {
    let mut diff = f.goals[0] - f.goals[1];
    if f.kind & SECOND_LEG != 0 {
        diff += f.leg[1] - f.leg[0];
    }
    if home { diff } else { -diff }
}

/// The match's record (`param_2` of `FUN_0066177d`, 0x498 bytes) as the
/// human's report keeps it: `FUN_00550f29` copies it over the report's
/// head when the match is over. The sides' blocks stand at +0x5c (home)
/// and +0x250 (away); the human's is his own.
#[derive(Clone, Debug, Default)]
struct Record {
    bytes: Vec<u8>,
    own: usize,
    foe: usize,
    home: bool,
    /// The fixture's word for the engine's counters ([`Fixture::engine`]).
    engine: bool,
    /// The ground's seats (`FUN_00638335` on +0x28, +0x2a, the copy's
    /// +0x3c), none for a ground the replica keeps no stadium for.
    capacity: Option<i32>,
}

const RECORD: usize = 0x498;
const HOME_SIDE: usize = 0x5c;
const AWAY_SIDE: usize = 0x250;
/// In a side's block: how often its trailing talk ran (+0x34), its plain
/// chances (+0x39), its straight reds of the other table (+0x3d), its
/// bookings for the foul kinds 4 and 12 (+0x43), its marks (+0x4e), the
/// eleven's states (+0x78, a short each), its yellows (+0xd4), second
/// yellows (+0xdc) and straight reds (+0xde), as `sim::Side` names them.
const TALKS: usize = 0x34;
const CHANCES: usize = 0x39;
const OTHER_REDS: usize = 0x3d;
const HARD_FOULS: usize = 0x43;
const MARKS: usize = 0x4e;
const STATES: usize = 0x78;
const YELLOWS: usize = 0xd4;
const SECOND_YELLOWS: usize = 0xdc;
const STRAIGHT_REDS: usize = 0xde;
/// The record's flag byte: extra time, penalties (+0x1d); the crowd
/// (+0x34); the weather (+0x466); the score at the clock's marks, a byte
/// each side, home first: at the half (+0x12), at 90 (+0x14), 10 (+0x16),
/// 75 (+0x18) and 85 minutes (+0x1a).
const FLAGS: usize = 0x1d;
const EXTRA_TIME: u8 = 0x80;
const PENALTIES: u8 = 0x40;
const CROWD: usize = 0x34;
const WEATHER: usize = 0x466;
const AT_HALF: usize = 0x12;
const AT_90: usize = 0x14;
const AT_10: usize = 0x16;
const AT_75: usize = 0x18;
const AT_85: usize = 0x1a;

impl Record {
    /// The human's report of the fixture, where it is the fixture's.
    fn of(records: &Records, team: &Team, f: &Fixture, home: bool) -> Option<Self> {
        let slot = usize::from(team.human()).checked_sub(1)?;
        let bytes = records.block(&format!("{REPORTS:#x}[{slot}]"))?.get(..RECORD)?.to_vec();
        let word = |at: usize| i16::from_le_bytes([bytes[at], bytes[at + 1]]);
        if [(bytes[0], word(2)), (bytes[4], word(6))] != f.sides {
            return None;
        }
        let capacity = crate::opponent::stadium(records, word(0x28) as u16, word(0x2a) as u16).map(|s| s.capacity);
        let (own, foe) = if home { (HOME_SIDE, AWAY_SIDE) } else { (AWAY_SIDE, HOME_SIDE) };
        Some(Self { bytes, own, foe, home, engine: f.engine, capacity })
    }

    fn byte(&self, at: usize) -> u8 {
        self.bytes.get(at).copied().unwrap_or(0)
    }

    fn word(&self, at: usize) -> i16 {
        i16::from_le_bytes([self.byte(at), self.byte(at + 1)])
    }

    fn long(&self, at: usize) -> i32 {
        i32::from_le_bytes([self.byte(at), self.byte(at + 1), self.byte(at + 2), self.byte(at + 3)])
    }

    /// A pair of bytes kept home first, the human side's first.
    fn pair(&self, at: usize) -> (i16, i16) {
        let (home, away) = (i16::from(self.byte(at)), i16::from(self.byte(at + 1)));
        if self.home { (home, away) } else { (away, home) }
    }

    /// A byte, a short of the human side's block and of the other's.
    fn own(&self, at: usize) -> u8 {
        self.byte(self.own + at)
    }

    fn foe(&self, at: usize) -> u8 {
        self.byte(self.foe + at)
    }

    fn own_word(&self, at: usize) -> i16 {
        self.word(self.own + at)
    }

    fn foe_word(&self, at: usize) -> i16 {
        self.word(self.foe + at)
    }

    /// The human side's eleven whose state `pick` takes (+0x78 or +0x2c8
    /// over its eleven places).
    fn states(&self, pick: impl Fn(i16) -> bool) -> usize {
        (0..11).filter(|&k| pick(self.own_word(STATES + k * 2))).count()
    }
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
#[derive(Default)]
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
    /// What the club leads the tie by (`local_10a`), and the other
    /// club's level (+0x105).
    tie: i16,
    opp_level: u8,
    /// The match's record, where the human's report keeps it.
    rec: Option<Record>,
    /// The manager, the club's money, its squad and its run of results.
    club: Club,
}

/// What the facts read off the manager object (`param_5`), the club's
/// money and its squad (`param_3` +0xb50, +0xe2), by the save's fields.
#[derive(Default)]
struct Club {
    /// The manager object's +0x22, +0x23 (the record's own offsets).
    status: u8,
    level: u8,
    /// +0x94 and +0x96 (record 0x180, 0x182).
    unrest: i16,
    second_count: i16,
    /// The titles (+0xdc), the black cash box (+0xec), the first of his
    /// abilities at his standing (+0x1800 + `FUN_006a61e5` × 0xe), the
    /// fans' and the media's liking (+0x3b, +0x3c), the contract's years
    /// (+0x66).
    titles: i16,
    black_cash: i32,
    ability: i16,
    fans: u8,
    media: u8,
    contract_years: i16,
    /// The board's confidence (`FUN_007701b2`), his majority of the
    /// club's shares (`FUN_00406b4f`), his age (`FUN_006a54d0`), and the
    /// humans of the game (`DAT_00b5cbce`).
    confidence: i32,
    majority: bool,
    age: i32,
    humans: u16,
    /// The money figure (`FUN_00621eac`) and the season's booked result
    /// (`FUN_00621f50`).
    figure: i64,
    booked: i64,
    /// The squad's men, the sums of their ages (`FUN_006ff01b`), their
    /// freshness (`FUN_006ff649`) and condition (`FUN_00457e00`), each a
    /// short as the original adds them.
    squad: i16,
    ages: i16,
    freshness: i16,
    condition: i16,
    /// How many are discontent (`FUN_00770877` over 1), play up front
    /// (main position 10, handle +0x3), are out over six days (+0x2c),
    /// stand over strength 9 (+0x2), and carry the league ban's byte
    /// (+0x36) at 0x20 or 0x80.
    discontent: i16,
    forwards: i16,
    injured: i16,
    strong: i16,
    banned: i16,
    sent_off: i16,
    /// The season's title and promotion (team +0x110: `FUN_0069a161`,
    /// `FUN_0069a1e1`).
    champion: bool,
    promoted: bool,
    /// The club's league results by matchday, its own goals first, none
    /// for a day not played (`FUN_006907da`).
    results: Vec<Option<(u8, u8)>>,
}

/// The main position of a forward (handle +0x3), the days out that count,
/// the strength that counts, and the league ban's bytes (handle +0x36).
const FORWARD: u8 = 10;
const OUT_DAYS: i32 = 6;
const STRONG: u8 = 9;
const BANNED: u8 = 0x20;
const SENT_OFF: u8 = 0x80;
/// The facts the five best of the squad give before the switch: one left
/// out of the match (live +0x57 bit 1 clear), one who stood it hurt for
/// over three weeks (+0x2c over 0x15).
const LEFT_OUT: usize = 400;
const PLAYED_HURT: usize = 408;
const HURT_DAYS: i32 = 0x15;
const BEST: usize = 5;
/// The bit of the record's match bits (+0x25b, the handle's +0x57) a man
/// who stood the match carries.
const PLAYED: u8 = 1;
/// The money's bounds (`FUN_00627bd0`: `DAT_00a3b308`, `DAT_00a3b2d8`,
/// `DAT_00a3b2e8`, `DAT_00a3b314`, so many thousands of `DAT_00a3b3e4`).
const RICH: i64 = 3_000_000;
const VERY_RICH: i64 = 8_000_000;
const DEBT: i64 = 5_000_000;
const LOSS: i64 = 2_000_000;
/// The season's achievements (team +0x110).
const ACHIEVEMENTS: usize = 0x110;
const TITLE: u16 = 1;
const PROMOTION: u16 = 4;

/// The live handle's +0x36 (`FUN_00636638`): the league ban's second
/// byte (record +0x230), left at nought when it carries a red card's bit
/// and the card's day (+0x1f2) is none or gone.
fn ban_byte(p: &crate::records::Player, today: u16) -> u8 {
    let ban = p.league_ban()[1];
    let on = i32::from(p.sent_off_on());
    if ban & 0x80 != 0 && (on == 0 || on < i32::from(today)) { 0 } else { ban }
}

impl Club {
    fn of(records: &Records, rules: &Rules, team: &Team, manager: usize) -> Self {
        let mut c = Self { humans: records.globals.humans(), ..Self::default() };
        if let Some(m) = records.managers.get(manager) {
            c.status = m.status();
            c.level = m.level();
            c.unrest = m.unrest_count();
            c.second_count = m.second_count();
            c.titles = m.titles()[0];
            c.black_cash = m.black_cash();
            c.ability = m.abilities()[m.standing().clamp(0, 15) as usize][0];
            let [_, fans, media] = m.popularity();
            (c.fans, c.media) = (fans, media);
            c.contract_years = m.contract_years();
            c.confidence = crate::stock::confidence(records, manager);
            c.majority = crate::stock::majority(records, manager);
            let born = chrono::NaiveDate::from_ymd_opt(
                i32::from(m.birth_year()),
                u32::from(m.birth_month()),
                u32::from(m.birth_day()),
            );
            c.age = crate::free_agents::today(records).map_or(0, |d| crate::autolineup::age(born, d));
        }
        c.figure = crate::finances::figure(team, records.globals.fund_prices());
        c.booked = crate::finances::booked_result(team);
        let achieved = team.get::<u16>(ACHIEVEMENTS);
        (c.champion, c.promoted) = (achieved & TITLE != 0, achieved & PROMOTION != 0);
        let today = crate::free_agents::today(records);
        let day = records.globals.day();
        for id in team.players() {
            let Some(p) = records.player(id) else { continue };
            let fitness = crate::load::fitness(p);
            c.squad += 1;
            c.ages = c.ages.wrapping_add(today.map_or(0, |d| crate::statistics::age_on(p, d)) as i16);
            c.freshness = c.freshness.wrapping_add(crate::lineup::freshness(fitness) as i16);
            c.condition = c.condition.wrapping_add(crate::lineup::condition(fitness) as i16);
            let status = crate::load::status(records, &rules.medical, p);
            c.discontent += i16::from(crate::transfer_week::discontent(p) > 1);
            c.forwards += i16::from(p.positions()[0] == FORWARD);
            c.injured += i16::from(status.injured > OUT_DAYS);
            c.strong += i16::from(p.strength() > STRONG);
            let ban = ban_byte(p, day);
            c.banned += i16::from(ban == BANNED);
            c.sent_off += i16::from(ban == SENT_OFF);
        }
        if let Some(league) = crate::opponent::League::of(records, team.country().into(), team.level()) {
            let place = usize::from(team.league_place());
            c.results = (1..=league.matchdays())
                .map(|md| match league.result(md, place)? {
                    (home, _, Some((a, b))) if home == place => Some((a, b)),
                    (_, _, Some((a, b))) => Some((b, a)),
                    _ => None,
                })
                .collect();
        }
        c
    }

    /// How many matchdays back from `from` the club's run lasts: the days
    /// to the last match `ends` holds for, all of them when none does
    /// (`FUN_00660cda` and its kin, `FUN_006611be`'s kinds). The home
    /// side the original takes from the club's fixture list
    /// (`FUN_0062af24`), the replica from the day's pairing.
    fn run(&self, from: i32, ends: impl Fn(u8, u8) -> bool) -> i32 {
        for k in (1..=from).rev() {
            if let Some(Some((own, foe))) = self.results.get(k as usize - 1) {
                if ends(*own, *foe) {
                    return from - k;
                }
            }
        }
        from
    }
}

/// The facts the club's five best give (`FUN_0066177d` before its
/// switch): its men by `FUN_006a115b`, the five above the rest kept in
/// order, ties behind; each who may play (`FUN_0069692b`) gives 400 when
/// he did not stand the match, else 408 when he stood it more than three
/// weeks hurt.
fn best_facts(records: &Records, rules: &Rules, team: &Team) -> Vec<usize> {
    let mut best = [(0i32, 0u16); BEST];
    for id in team.players() {
        let Some(p) = records.player(id) else { continue };
        let status = crate::load::status(records, &rules.medical, p);
        let value = rules.pitch.strength(&crate::load::player(records, p), crate::load::fitness(p), status.doping);
        if best[BEST - 1].0 < value {
            let at = best[..BEST - 1].iter().rposition(|&(v, _)| v >= value).map_or(0, |k| k + 1);
            best.copy_within(at..BEST - 1, at + 1);
            best[at] = (value, id);
        }
    }
    let mut facts = Vec::new();
    for &(_, id) in &best {
        let Some(p) = records.player(id).filter(|_| id != 0) else { continue };
        let status = crate::load::status(records, &rules.medical, p);
        if !crate::autolineup::may_play(&status) {
            continue;
        }
        if p.match_bits() & PLAYED == 0 {
            facts.push(LEFT_OUT);
        } else if status.injured > HURT_DAYS {
            facts.push(PLAYED_HURT);
        }
    }
    facts
}

/// Whether a fact holds, by its case (the cases built; the rest wait for
/// the manager, the squad and the men the record names). The cases that
/// read the engine's counters hold only over a record the engine wrote.
fn holds(case: u8, m: &Match) -> bool {
    let md = i32::from(m.md);
    let league_day = md > 0 || m.kind & 0x8008_0000 != 0x8_0000;
    let rec = m.rec.as_ref();
    let ev = rec.filter(|r| r.engine);
    let europe = m.kind & 0xff00_0000 == 0xd000_0000;
    match case {
        // The tie (`local_10a`).
        0x09 | 0x92 => europe && m.tie > 0,
        0x9e => europe && m.tie < 0,
        0xd9 => m.kind & 0x7f00_0000 == 0x5000_0000 && m.kind & 0x300 != 0x300 && m.kind & SECOND_LEG != 0 && m.tie < 0,
        0x95 => m.kind & 0x7f08_0000 == 0x4008_0000 && m.level == 0 && m.opp_level > 1 && m.tie < 0,
        // The record as the stand-in writes it too: the half's score,
        // extra time and penalties, the crowd.
        0x9b => rec.is_some_and(|r| r.pair(AT_HALF).0 - r.pair(AT_HALF).1 < -1),
        0xa7 => rec.is_some_and(|r| r.pair(AT_HALF).1 < r.pair(AT_HALF).0),
        0xe6 => rec.is_some_and(|r| r.pair(AT_HALF) == (0, 2)),
        0xca => rec.is_some_and(|r| r.byte(FLAGS) & PENALTIES != 0) && m.tie > 0,
        0xcd => rec.is_some_and(|r| r.byte(FLAGS) & EXTRA_TIME != 0),
        0xd7 => rec.is_some_and(|r| r.byte(FLAGS) & PENALTIES == 0),
        0xea => rec.is_some_and(|r| r.byte(FLAGS) & EXTRA_TIME == 0),
        0x83 => rec.is_some_and(|r| r.capacity.is_some_and(|c| c <= r.long(CROWD))),
        0xa0 => rec.is_some_and(|r| r.long(CROWD) < 15000),
        // The engine's: the weather.
        0x44 => ev.is_some_and(|r| matches!(r.byte(WEATHER), 3 | 11)),
        0xbc => ev.is_some_and(|r| r.byte(WEATHER) == 4),
        0xbd => ev.is_some_and(|r| matches!(r.byte(WEATHER), 3 | 8 | 9 | 11)),
        // The score at the clock's marks.
        0x8c => ev.is_some_and(|r| r.pair(AT_90).0 < m.own),
        0x9f => ev.is_some_and(|r| r.pair(AT_10).1 != 0),
        0xce => ev.is_some_and(|r| r.pair(AT_10).0 == 2),
        0x7c => md > 0 && ev.is_some_and(|r| r.pair(AT_75).1 == m.opp - 2),
        0x61 => md > 0 && ev.is_some_and(|r| r.pair(AT_85).0 < r.pair(AT_85).1 && m.own == m.opp && m.opp == r.pair(AT_85).1),
        // The chances.
        0x0f => ev.is_some_and(|r| r.own(CHANCES) < 3),
        0x35 => ev.is_some_and(|r| r.foe(CHANCES) < r.own(CHANCES)),
        0x40 => ev.is_some_and(|r| i32::from(r.own(CHANCES)) + i32::from(r.foe(CHANCES)) < 5),
        0x41 => ev.is_some_and(|r| r.foe(CHANCES) > 7),
        0x60 => ev.is_some_and(|r| r.own(CHANCES) == 1),
        0x7e => ev.is_some_and(|r| r.own(CHANCES) > 7),
        0x84 => m.own + m.opp > 3 && ev.is_some_and(|r| i32::from(r.own(CHANCES)) + i32::from(r.foe(CHANCES)) > 11),
        0xad => ev.is_some_and(|r| r.own(CHANCES) == 0),
        0xb4 => ev.is_some_and(|r| i32::from(r.foe(CHANCES)) * 3 <= i32::from(r.own(CHANCES)) && r.foe(CHANCES) > 0),
        0xb5 => ev.is_some_and(|r| r.own(CHANCES) == r.foe(CHANCES)),
        0xb6 => ev.is_some_and(|r| i32::from(r.own(CHANCES)) * 2 / 5 <= i32::from(m.own)),
        // The cards.
        0x10 => ev.is_some_and(|r| r.own_word(YELLOWS) < 3 && r.own_word(STRAIGHT_REDS) + r.foe_word(STRAIGHT_REDS) == 0),
        0x22 => ev.is_some_and(|r| r.own_word(YELLOWS) > 3 && r.foe_word(YELLOWS) > 3),
        0x46 => ev.is_some_and(|r| r.own_word(YELLOWS) > 5),
        0x57 => ev.is_some_and(|r| r.foe_word(YELLOWS) > 4),
        0xc9 => ev.is_some_and(|r| r.own_word(YELLOWS) == 0),
        0x5f => ev.is_some_and(|r| r.foe_word(STRAIGHT_REDS) + r.foe_word(SECOND_YELLOWS) == 1),
        0x7a => ev.is_some_and(|r| r.own_word(STRAIGHT_REDS) + r.own_word(SECOND_YELLOWS) == 2),
        0x16 => ev.is_some_and(|r| r.own(OTHER_REDS) > 0),
        0x17 => ev.is_some_and(|r| r.own(OTHER_REDS) > 1),
        0xd5 => ev.is_some_and(|r| r.own(OTHER_REDS) == 0),
        0x18 => ev.is_some_and(|r| r.foe(OTHER_REDS) == 0),
        0xd4 => ev.is_some_and(|r| r.foe(OTHER_REDS) > 1),
        0x75 => ev.is_some_and(|r| r.own(HARD_FOULS) > 1),
        // The side's other bytes and marks, the eleven's states.
        0x8e => ev.is_some_and(|r| r.own(TALKS) == 3),
        0x98 => ev.is_some_and(|r| r.own(TALKS) == 0),
        0xb1 => ev.is_some_and(|r| r.own(0x44) == 3),
        0x8b => ev.is_some_and(|r| r.own(0x45) == 1),
        0x9c => ev.is_some_and(|r| r.own(0x46) > 2),
        0x9d | 0xe4 => ev.is_some_and(|r| r.own(0x47) == 3),
        0xa3 => ev.is_some_and(|r| r.foe(0x48) != 0),
        0xa4 => ev.is_some_and(|r| r.own(0x49) != 0),
        0xae => ev.is_some_and(|r| r.own(0x4a) != 0),
        0xc8 => ev.is_some_and(|r| r.own(0x4b) > 4),
        0xc1 => ev.is_some_and(|r| r.foe(0x4b) > 4),
        0x63 => ev.is_some_and(|r| r.own_word(MARKS) & 1 != 0),
        0x48 => ev.is_some_and(|r| r.own_word(MARKS) & 4 != 0),
        0x8d => ev.is_some_and(|r| r.own_word(MARKS) & 0x10 != 0),
        0x67 => ev.is_some_and(|r| r.own_word(MARKS) & 0x20 != 0),
        0x0b => ev.is_some_and(|r| r.own_word(MARKS) & 0x40 != 0),
        0x99 => ev.is_some_and(|r| r.foe_word(MARKS) & 8 != 0),
        0xaa => ev.is_some_and(|r| r.foe_word(MARKS) & 0x20 != 0),
        0x70 => ev.is_some_and(|r| r.own_word(0x2e) & 4 != 0),
        0x27 => ev.is_some_and(|r| r.states(|s| s == 2 || s == 13) != 0),
        0x2a => ev.is_some_and(|r| r.states(|s| s == 10) > 1),
        0x81 => ev.is_some_and(|r| r.states(|s| s == 10) == 1),
        0xc6 => ev.is_some_and(|r| r.states(|s| (6..=9).contains(&s)) < 3),
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
        _ => holds_club(case, m),
    }
}

/// The facts of the manager, the club's money, its squad and its run of
/// results (`FUN_0066177d`'s cases over `param_5`, `FUN_00621eac` and the
/// squad at +0xb50).
fn holds_club(case: u8, m: &Match) -> bool {
    let c = &m.club;
    let md = i32::from(m.md);
    let mean_over = |sum: i16, bound: f32| c.squad > 0 && bound < f32::from(sum) / f32::from(c.squad);
    let won = |own: u8, foe: u8| own > foe;
    let lost = |own: u8, foe: u8| own < foe;
    match case {
        0x00 => c.level > 10,
        0x5b => c.level < 6,
        0x25 => c.status & 2 != 0,
        0x24 => c.unrest < 2,
        0x47 => c.unrest > 2,
        0x4d => md > -1 && c.second_count < 10,
        0x5c => md > -1 && c.second_count == 0,
        0xba => c.titles > 5,
        0xb7 => c.black_cash > 0,
        0xec => c.ability > 10,
        0x30 => c.fans > 90,
        0xc7 => c.fans == 100,
        0xc2 => c.media > 80,
        0x71 => c.contract_years > 1,
        0x04 => c.confidence < 70,
        0x06 => c.confidence < 65,
        0x1b => c.confidence < 60,
        0x26 => c.confidence < 75,
        0x89 => c.confidence > 80,
        0xe0 => c.humans == 2 && c.confidence < 65,
        0xeb => !c.majority,
        0x5a => c.age > 40,
        0x64 => c.age > 55,
        0x6a => c.age < 40,
        0xb3 => c.age > 60,
        0x15 => c.figure > RICH,
        0x43 => c.figure > VERY_RICH,
        0x90 => c.figure < -DEBT,
        0xb8 => c.booked < -LOSS,
        0x12 => mean_over(c.ages, 29.0),
        0x5e => c.squad > 0 && f32::from(c.freshness) / f32::from(c.squad) < 50.0,
        0xa8 => mean_over(c.freshness, 70.0),
        0x82 => mean_over(c.condition, 70.0),
        0x3a => c.discontent > 4,
        0xe1 => c.discontent > 5,
        0xe3 => c.forwards < 4,
        0x72 => c.injured > 4,
        0xcc => c.injured == 0,
        0x76 => c.strong > 9,
        0xc3 => md > 0 && c.banned == 2,
        0xd2 => md > 0 && c.banned > 1,
        0xd3 => md > 0 && c.sent_off > 1,
        0x0c => m.diff > 0 && md > 0 && c.promoted,
        0x0d => md > 0 && !c.promoted,
        0x51 => md > 0 && c.promoted,
        0x53 => md > 0 && !c.champion,
        0xa2 => md > 0 && c.champion,
        // Unbeaten, winning, without a win, losing (`FUN_00660cda`,
        // `FUN_00660e13`, `FUN_00660f4c`, `FUN_00661085`).
        0x02 => md > 3 && c.run(md, won) == 4,
        0x31 => md > 2 && c.run(md, |o, f| !won(o, f)) == 3,
        0x74 => md > 1 && c.run(md, |o, f| !won(o, f)) == 2,
        0x3d => md > 3 && c.run(md, lost) == 4,
        0x7b => md > 7 && c.run(md, |o, f| !lost(o, f)) == 8,
        // `FUN_006611be`'s kinds: 10 wins by three, 11 without a goal, 12
        // goalless, 13 without one let in, 14 drawn; 5 lost, from the
        // matchday before (`FUN_006615a5`).
        0x56 => md > 0 && c.run(md, |o, f| i32::from(o) <= i32::from(f) + 2) == 0,
        0x8f => md > -1 && c.run(md, |o, _| o != 0) > 3,
        0x79 => md > 0 && c.run(md, |o, f| o != 0 || f != 0) == 3,
        0x88 => md > 10 && c.run(md, |_, f| f != 0) > 10,
        0xc4 => md > -1 && c.run(md, |o, f| o != f) > 4,
        0xc5 => md > 3 && c.run(md - 1, |o, f| !lost(o, f)) > 2,
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
            tie: tie(&fixture, home),
            opp_level: other.level(),
            rec: Record::of(records, own, &fixture, home),
            club: Club::of(records, rules, own, usize::from(manager) - 1),
        };
        let m = Match { top: m.md > 1 && u32::from(own.get::<u8>(PLACE_NEXT)) + u32::from(other.get::<u8>(PLACE_NEXT)) < 7, ..m };
        for fact in best_facts(records, rules, own) {
            me.facts[fact] = true;
        }
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
        let won = Fixture { sides: [(14, 22), (14, 84)], goals: [2, 0], kind: 0x4000_0003, ..Fixture::default() };
        let (home_won, away_lost) = (rate(won, true), rate(won, false));
        assert!((300..600).contains(&home_won), "{home_won}");
        assert!((1100..1500).contains(&away_lost), "{away_lost}");
        // Won 1:0 at home after 0:3 away: behind over the tie.
        let second = Fixture { goals: [1, 0], kind: 0xc000_0003, leg: [3, 0], ..won };
        assert!((1100..1500).contains(&rate(second, true)));
    }

    /// A Champions League match draws one ten times in a hundred; a
    /// one-leg match that is no second leg always.
    #[test]
    fn a_champions_league_match_draws_an_interview() {
        let rate = |kind: u32| {
            let f = Fixture { sides: [(14, 22), (14, 84)], goals: [1, 1], kind, ..Fixture::default() };
            (0..3000u64).filter(|&seed| champions_draws(&f, &mut Random::new(seed))).count()
        };
        assert!((200..400).contains(&rate(0x5000_0002)), "{}", rate(0x5000_0002));
        assert_eq!(rate(0x5008_0002), 3000);
        let second = rate(0xd008_0002);
        assert!((700..1000).contains(&second), "{second}");
    }

    /// The record's facts, the human side away: its half's score, the
    /// penalties and the crowd hold over the stand-in's record as well;
    /// the chances, the cards and the weather only over the engine's.
    #[test]
    fn the_record_decides_the_facts_of_the_match() {
        let mut bytes = vec![0u8; RECORD];
        bytes[AT_HALF] = 2;
        bytes[FLAGS] = PENALTIES;
        bytes[CROWD..CROWD + 4].copy_from_slice(&12_000i32.to_le_bytes());
        bytes[WEATHER] = 4;
        bytes[AWAY_SIDE + CHANCES] = 2;
        bytes[HOME_SIDE + CHANCES] = 9;
        bytes[AWAY_SIDE + YELLOWS] = 6;
        let rec = Record { bytes, own: AWAY_SIDE, foe: HOME_SIDE, home: false, engine: false, capacity: Some(12_000) };
        let at = |rec: Option<Record>| Match { own: 2, opp: 2, md: -1, kind: 0xd000_0005, tie: -1, rec, ..Match::default() };
        let stand_in = at(Some(rec.clone()));
        for (case, holds_now) in [(0xe6, true), (0x9b, true), (0xa7, false), (0xd7, false), (0xca, false), (0x83, true), (0xa0, true), (0x9e, true), (0x09, false)] {
            assert_eq!(holds(case, &stand_in), holds_now, "{case:#x}");
        }
        for case in [0x0f, 0xbc, 0x46, 0x41, 0x18] {
            assert!(!holds(case, &stand_in), "{case:#x}");
        }
        let engine = at(Some(Record { engine: true, ..rec }));
        for (case, holds_now) in [(0x0f, true), (0xbc, true), (0x46, true), (0x41, true), (0x35, false), (0xc9, false), (0x18, true), (0xb6, true)] {
            assert_eq!(holds(case, &engine), holds_now, "{case:#x}");
        }
        assert!(!holds(0xa0, &at(None)) && !holds(0xd7, &at(None)));
    }

    /// A's report of Union's last match, as the original's engine wrote
    /// it: the facts of its chances, yellows and crowd say what the
    /// Spielbericht's figures say.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_reports_record_gives_the_facts_of_its_match() {
        let Some((dir, records, rules)) = setup() else { return };
        let union = records.team(14, 84).unwrap();
        let report = crate::statistics::match_report(&records, union).expect("A keeps Union's report");
        let home = report.sides()[0] == (14, 84);
        let fixture = Fixture {
            sides: report.sides(),
            goals: report.goals(),
            kind: report.competition(),
            leg: report.first_leg().map_or([0; 2], |l| [i16::from(l[1]), i16::from(l[0])]),
            engine: true,
        };
        let questions = a3_files::interview::load(&dir);
        let interview = Interview::begin(&records, &rules, questions, (fixture, home), &mut Recent::default(), &mut Random::new(3)).unwrap();
        let [_, yellows, _, chances] = report.comparison();
        let (own, foe) = if home { (0, 1) } else { (1, 0) };
        let mut seen = 0;
        for (k, &case) in rules.interview.cases.iter().enumerate() {
            let want = match case {
                0x35 => chances[foe] < chances[own],
                0x0f => chances[own] < 3,
                0x40 => chances[own] + chances[foe] < 5,
                0xc9 => yellows[own] == 0,
                0x46 => yellows[own] > 5,
                0xa0 => report.attendance() < 15000,
                _ => continue,
            };
            assert_eq!(interview.fact(k as i16 + 1), want, "fact {} case {case:#x}", k + 1);
            seen += 1;
        }
        assert!(seen >= 6, "{seen}");
        assert!(chances[0] + chances[1] > 0, "{chances:?}");
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_big_win_draws_an_interview_now_and_then() {
        let Some((_, mut records, _)) = setup() else { return };
        let union = Fixture { sides: [(14, 84), (14, 22)], goals: [4, 0], kind: 3, ..Fixture::default() };
        let others = Fixture { sides: [(14, 22), (14, 20)], goals: [5, 0], kind: 3, ..Fixture::default() };
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
        let fixture = Fixture { sides: [(14, 22), (14, 84)], goals: [3, 0], kind: 3, ..Fixture::default() };
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

    /// The club's run, money and men decide their facts: four matches
    /// without a defeat after two lost, the last won by three, a manager
    /// past sixty with a board that trusts him little.
    #[test]
    fn the_club_decides_its_facts() {
        let results = [(0, 2), (1, 3), (2, 2), (1, 0), (0, 0), (4, 1)].map(Some).to_vec();
        let club = Club { results, confidence: 62, age: 61, figure: -6_000_000, squad: 2, ages: 61, level: 4, ..Club::default() };
        let m = Match { md: 6, own: 4, opp: 1, diff: 3, club, ..Match::default() };
        for (case, want) in [
            (0x3d, true), (0x02, false), (0x56, false), (0x7b, false), (0x31, false),
            (0x06, true), (0x1b, false), (0xb3, true), (0x6a, false), (0x90, true), (0x15, false),
            (0x12, true), (0x5b, true), (0x00, false), (0xeb, true), (0xc4, false),
        ] {
            assert_eq!(holds(case, &m), want, "{case:#x}");
        }
        assert_eq!(m.club.run(6, |o, f| o < f), 4);
        assert_eq!(m.club.run(6, |o, f| o <= f + 2), 1);
        assert_eq!(m.club.run(2, |o, f| o >= f), 2, "lost both");
        assert_eq!(m.club.run(0, |_, _| true), 0);
        let m = Match { md: 7, club: Club { results: vec![Some((0, 1)); 4], ..Club::default() }, ..Match::default() };
        assert!(holds(0xc5, &m), "three lost before the day");
    }

    /// A's Union: the facts of its manager, its money and its squad say
    /// what the save says of them.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_save_gives_the_facts_of_the_club() {
        let Some((dir, records, rules)) = setup() else { return };
        let union = records.team(14, 84).unwrap();
        let fixture = Fixture { sides: [(14, 22), (14, 84)], goals: [3, 0], kind: 3, ..Fixture::default() };
        let questions = a3_files::interview::load(&dir);
        let interview =
            Interview::begin(&records, &rules, questions, (fixture, false), &mut Recent::default(), &mut Random::new(3)).unwrap();
        let slot = usize::from(interview.manager) - 1;
        let boss = &records.managers[slot];
        let confidence = crate::stock::confidence(&records, slot);
        let figure = crate::finances::figure(union, records.globals.fund_prices());
        let squad: Vec<_> = union.players().into_iter().filter_map(|id| records.player(id)).collect();
        let discontent = squad.iter().filter(|p| crate::transfer_week::discontent(p) > 1).count();
        let mut seen = 0;
        for (k, &case) in rules.interview.cases.iter().enumerate() {
            let want = match case {
                0x04 => confidence < 70,
                0x89 => confidence > 80,
                0x00 => boss.level() > 10,
                0x5b => boss.level() < 6,
                0x15 => figure > 3_000_000,
                0x90 => figure < -5_000_000,
                0x30 => boss.popularity()[1] > 90,
                0x3a => discontent > 4,
                _ => continue,
            };
            assert_eq!(interview.fact(k as i16 + 1), want, "fact {} case {case:#x}", k + 1);
            seen += 1;
        }
        assert!(seen >= 6 && !squad.is_empty(), "{seen}");
        let club = Club::of(&records, &rules, union, slot);
        assert_eq!(usize::try_from(club.squad).unwrap(), squad.len());
        let played = club.results.iter().filter(|r| r.is_some()).count();
        let goals: i32 = club.results.iter().flatten().map(|&(own, _)| i32::from(own)).sum();
        assert!(played > 0 && played < club.results.len(), "{:?}", club.results);
        assert_eq!(goals, i32::from(union.goals()[0]), "{:?}", club.results);
    }
}
