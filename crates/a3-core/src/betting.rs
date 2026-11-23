//! The betting office (ERFOLGE.md §D): each human's bets, a page for each
//! country played — on the matchday (dialog 411) and on the season (410)
//! —, and what the game offers to bet on. Nothing is paid as a bet is
//! placed; the stakes only must not come to more than the private
//! fortune, and the bets are settled with the matches.

use std::io;

use a3_files::exe::Exe;

use crate::discipline;
use crate::random::Random;
use crate::records::Records;

/// A human's bets (`0x1c67ce0[human − 1]`, the record from +0x20): five
/// season pages of 0x1c, then five matchday pages of 0x2c.
const BETS_AT: &str = "0x1c67ce0";
const MATCHDAY_FIRST: usize = 0x8c;
const MATCHDAY: usize = 0x2c;
/// A matchday page's fields: the first scorer's row, the goals, the
/// trainer to go with his odds and club, the stakes, how many times the
/// full tip is played and its ticks, whether the trainer bet runs.
const SCORER: usize = 0;
const GOALS: usize = 2;
const TRAINER: usize = 4;
const TRAINER_ODDS: usize = 6;
const TRAINER_COUNTRY: usize = 8;
const TRAINER_NUMBER: usize = 0xa;
const HIGH_STAKE: usize = 0xc;
const SCORER_STAKE: usize = 0x10;
const GOALS_STAKE: usize = 0x14;
const TRAINER_STAKE: usize = 0x18;
const FULL: usize = 0x1c;
const TICKS: usize = 0x1e;
const RUNNING: usize = 0x29;
pub const MATCHES: usize = 11;
/// The pages: one a country played.
pub const PAGES: usize = 5;

/// What the game offers on the matchday (`0x1c645d8`, its data from
/// 0x1c645f8): eleven matches a country, the first scorers and the
/// trainers to be sacked. Its first byte, the first match's home country,
/// is 0 while there is none.
const OFFER_AT: &str = "0x1c645d8";
const MATCHES_AT: usize = 2;
const MATCH_PAGE: usize = 0xfa;
const MATCH: usize = 0x16;
const SCORERS_AT: usize = 0x4e4;
const SCORER_PAGE: usize = 0x25c;
const TRAINERS_AT: usize = 0x10b0;
const TRAINER_PAGE: usize = 0x192;

/// The ticks of a match: home win, draw, away win, and the highest win.
pub const HOME: u8 = 4;
pub const DRAW: u8 = 2;
pub const AWAY: u8 = 1;
pub const HIGHEST: u8 = 8;
const OUTCOMES: u8 = HOME | DRAW | AWAY;
/// The most a stake of the season or a single bet of the matchday comes
/// to (`DAT_00a3b3ac`), and the stake on the highest win.
pub const MOST_STAKE: i32 = 20_000;
pub const MOST_HIGH_STAKE: i32 = 10_000;
/// How many times a full tip is played at most.
pub const MOST_FULL: i16 = 10;

/// The stakes' spinners' scale (`0x88d100`, `0x88d348`).
const SCALE_AT: u32 = 0x88d100;
const SCALE_STEPS: usize = 4;

pub struct BettingTables {
    scale: Vec<(i32, i32)>,
}

impl BettingTables {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        let values: Vec<i32> = exe.u32s(SCALE_AT, 2 * SCALE_STEPS)?.into_iter().map(|v| v as i32).collect();
        let scale = values.chunks(2).map(|c| (c[0], c[1])).take_while(|&(_, step)| step != 0).collect();
        Ok(Self { scale })
    }

    /// The stake at a spinner's position (`FUN_005c69c9`).
    pub fn stake_at(&self, position: i32) -> i32 {
        discipline::amount(&self.scale, position)
    }

    /// A stake's position (`FUN_005c690b`).
    pub fn position_of(&self, stake: i32) -> i32 {
        discipline::position(&self.scale, stake)
    }
}

/// A human's bets on a country's matchday.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MatchdayBet {
    /// The first scorer's row in the offer, −1 none.
    pub scorer: i16,
    /// The matchday's goals, −1 none.
    pub goals: i16,
    /// The trainer to be sacked: a person by id, a human manager by his
    /// number negated, 0 none; his odds in tenths and his club.
    pub trainer: i16,
    pub trainer_odds: i16,
    pub trainer_club: (i16, i16),
    /// The stake on each highest win ticked.
    pub high_stake: i32,
    pub scorer_stake: i32,
    pub goals_stake: i32,
    pub trainer_stake: i32,
    /// How many times the full tip is played, 0 to 10.
    pub full: i16,
    pub ticks: [u8; MATCHES],
    /// Whether the trainer bet runs: then it stands.
    pub running: bool,
}

fn int(b: &[u8], at: usize) -> i32 {
    b.get(at..at + 4).map_or(0, |f| i32::from_le_bytes(f.try_into().unwrap()))
}

fn short(b: &[u8], at: usize) -> i16 {
    b.get(at..at + 2).map_or(0, |f| i16::from_le_bytes(f.try_into().unwrap()))
}

fn bets_block(human: usize) -> String {
    format!("{BETS_AT}[{human}]")
}

impl MatchdayBet {
    /// A full tip's stake (`FUN_0077f9fe`): a euro for every way the
    /// ticks combine — the outcomes ticked of each match multiplied — times
    /// how often it is played; nothing while a match has none ticked.
    pub fn full_stake(&self) -> i32 {
        self.ticks.iter().map(|t| (t & OUTCOMES).count_ones() as i32).product::<i32>() * i32::from(self.full)
    }

    /// How many highest wins are ticked.
    pub fn highest(&self) -> i32 {
        self.ticks.iter().filter(|&&t| t & HIGHEST != 0).count() as i32
    }

    /// What the page stakes as the matchday's total counts it: the goals'
    /// stake is left out (`FUN_0077fb60`).
    fn counted(&self) -> i32 {
        self.full_stake() + self.high_stake * self.highest() + self.scorer_stake + self.trainer_stake
    }
}

/// The human's bets on the page's matchday.
pub fn matchday_bet(records: &Records, human: usize, page: usize) -> MatchdayBet {
    let Some(b) = records.block(&bets_block(human)) else { return MatchdayBet::default() };
    let m = MATCHDAY_FIRST + page * MATCHDAY;
    MatchdayBet {
        scorer: short(b, m + SCORER),
        goals: short(b, m + GOALS),
        trainer: short(b, m + TRAINER),
        trainer_odds: short(b, m + TRAINER_ODDS),
        trainer_club: (short(b, m + TRAINER_COUNTRY), short(b, m + TRAINER_NUMBER)),
        high_stake: int(b, m + HIGH_STAKE),
        scorer_stake: int(b, m + SCORER_STAKE),
        goals_stake: int(b, m + GOALS_STAKE),
        trainer_stake: int(b, m + TRAINER_STAKE),
        full: short(b, m + FULL),
        ticks: std::array::from_fn(|k| b.get(m + TICKS + k).copied().unwrap_or(0)),
        running: b.get(m + RUNNING).is_some_and(|&r| r != 0),
    }
}

/// Writes the page's matchday bets back.
pub fn set_matchday_bet(records: &mut Records, human: usize, page: usize, bet: &MatchdayBet) {
    let Some(b) = records.block_mut(&bets_block(human)) else { return };
    let m = MATCHDAY_FIRST + page * MATCHDAY;
    let mut put = |at: usize, bytes: &[u8]| b[m + at..m + at + bytes.len()].copy_from_slice(bytes);
    put(SCORER, &bet.scorer.to_le_bytes());
    put(GOALS, &bet.goals.to_le_bytes());
    put(TRAINER, &bet.trainer.to_le_bytes());
    put(TRAINER_ODDS, &bet.trainer_odds.to_le_bytes());
    put(TRAINER_COUNTRY, &bet.trainer_club.0.to_le_bytes());
    put(TRAINER_NUMBER, &bet.trainer_club.1.to_le_bytes());
    put(HIGH_STAKE, &bet.high_stake.to_le_bytes());
    put(SCORER_STAKE, &bet.scorer_stake.to_le_bytes());
    put(GOALS_STAKE, &bet.goals_stake.to_le_bytes());
    put(TRAINER_STAKE, &bet.trainer_stake.to_le_bytes());
    put(FULL, &bet.full.to_le_bytes());
    put(TICKS, &bet.ticks);
    put(RUNNING, &[u8::from(bet.running)]);
}

/// All the human's matchday stakes as they count against his fortune
/// (`FUN_0077fb60`).
pub fn matchday_total(records: &Records, human: usize) -> i32 {
    (0..PAGES).map(|page| matchday_bet(records, human, page).counted()).sum()
}

/// What a single matchday bet may still take (`FUN_0077fbfe`): the
/// fortune less the matchday's stakes, at most 20.000, none below.
pub fn matchday_room(records: &Records, human: usize, fortune: i32) -> i32 {
    (fortune - matchday_total(records, human)).min(MOST_STAKE).max(0)
}

/// A match of the matchday's offer: the home team's country and number,
/// the away team's, and the quota of a win.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Match {
    pub home: (u8, u16),
    pub away: (u8, u16),
    pub quota: i16,
}

/// A first scorer to bet on: his names in the pool and his odds in tenths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scorer {
    pub names: (u16, u16),
    pub odds: i32,
}

/// A trainer to bet will be sacked: a person, or a human manager negated;
/// his club and his odds in tenths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trainer {
    pub id: i16,
    pub club: (i16, i16),
    pub odds: i16,
}

/// The matchday's offer on a country.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchdayOffer {
    pub matches: Vec<Match>,
    pub scorers: Vec<Scorer>,
    pub trainers: Vec<Trainer>,
}

/// Whether the game has made a matchday's offer at all.
pub fn matchday_open(records: &Records) -> bool {
    records.block(OFFER_AT).and_then(|b| b.get(MATCHES_AT)).is_some_and(|&c| c != 0)
}

/// The matchday's offer on the page's country.
pub fn matchday_offer(records: &Records, page: usize) -> Option<MatchdayOffer> {
    let b = records.block(OFFER_AT)?;
    let matches = (0..MATCHES)
        .map(|k| {
            let m = MATCHES_AT + page * MATCH_PAGE + k * MATCH;
            Match {
                home: (b.get(m).copied().unwrap_or(0), short(b, m + 2) as u16),
                away: (b.get(m + 4).copied().unwrap_or(0), short(b, m + 6) as u16),
                quota: short(b, m + 0xe),
            }
        })
        .collect();
    let s = SCORERS_AT + page * SCORER_PAGE;
    let scorers = (0..usize::try_from(int(b, s)).unwrap_or(0))
        .map(|i| Scorer {
            names: (short(b, s + 0xcc + 2 * i) as u16, short(b, s + 0x130 + 2 * i) as u16),
            odds: int(b, s + 0x194 + 4 * i),
        })
        .collect();
    let t = TRAINERS_AT + page * TRAINER_PAGE;
    let trainers = (0..usize::try_from(short(b, t)).unwrap_or(0))
        .map(|i| Trainer {
            id: short(b, t + 2 + 2 * i),
            club: (short(b, t + 0x66 + 2 * i), short(b, t + 0xca + 2 * i)),
            odds: short(b, t + 0x12e + 2 * i),
        })
        .collect();
    Some(MatchdayOffer { matches, scorers, trainers })
}

/// A human's season bets on a country (the bets record's first pages,
/// 0x1c a country): the champion by team number, the cup winner and the
/// top scorer by their rows in the offer, the trainer sacked first with
/// the odds taken, and the four stakes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SeasonPage {
    pub champion: i16,
    pub cup: i16,
    pub scorer: i16,
    pub trainer: i16,
    pub trainer_odds: i16,
    pub stakes: [i32; 4],
}

/// The season bets over all countries: the own team's place, with the
/// club it was bet for, and the European cups' and the tournaments'
/// winners by their rows, with the five stakes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SeasonBets {
    pub pages: [SeasonPage; PAGES],
    pub place: i16,
    pub place_club: (i16, i16),
    /// The Champions League, the UEFA cup, the European and the World
    /// championship.
    pub cups: [i16; 4],
    /// The place's, then the four cups' stakes.
    pub stakes: [i32; 5],
}

const SEASON_PAGE: usize = 0x1c;
const SEASON_STAKES: usize = 0xc;
const PLACE_CLUB: usize = 0x19a;
const PLACE: usize = 0x19e;
const CUPS: usize = 0x1a0;
const STAKES: usize = 0x1a8;
/// The own league's size and the odds of each place.
const LEAGUE_SIZE: usize = 0x168;
const PLACE_ODDS: usize = 0x16a;

/// The human's season bets.
pub fn season_bets(records: &Records, human: usize) -> SeasonBets {
    let Some(b) = records.block(&bets_block(human)) else { return SeasonBets::default() };
    SeasonBets {
        pages: std::array::from_fn(|c| {
            let p = c * SEASON_PAGE;
            SeasonPage {
                champion: short(b, p),
                cup: short(b, p + 2),
                scorer: short(b, p + 4),
                trainer: short(b, p + 6),
                trainer_odds: short(b, p + 8),
                stakes: std::array::from_fn(|k| int(b, p + SEASON_STAKES + 4 * k)),
            }
        }),
        place: short(b, PLACE),
        place_club: (short(b, PLACE_CLUB), short(b, PLACE_CLUB + 2)),
        cups: std::array::from_fn(|k| short(b, CUPS + 2 * k)),
        stakes: std::array::from_fn(|k| int(b, STAKES + 4 * k)),
    }
}

/// Writes the season bets back.
pub fn set_season_bets(records: &mut Records, human: usize, bets: &SeasonBets) {
    let Some(b) = records.block_mut(&bets_block(human)) else { return };
    let mut put = |at: usize, bytes: &[u8]| b[at..at + bytes.len()].copy_from_slice(bytes);
    for (c, page) in bets.pages.iter().enumerate() {
        let p = c * SEASON_PAGE;
        put(p, &page.champion.to_le_bytes());
        put(p + 2, &page.cup.to_le_bytes());
        put(p + 4, &page.scorer.to_le_bytes());
        put(p + 6, &page.trainer.to_le_bytes());
        put(p + 8, &page.trainer_odds.to_le_bytes());
        for (k, stake) in page.stakes.iter().enumerate() {
            put(p + SEASON_STAKES + 4 * k, &stake.to_le_bytes());
        }
    }
    put(PLACE, &bets.place.to_le_bytes());
    put(PLACE_CLUB, &bets.place_club.0.to_le_bytes());
    put(PLACE_CLUB + 2, &bets.place_club.1.to_le_bytes());
    for (k, cup) in bets.cups.iter().enumerate() {
        put(CUPS + 2 * k, &cup.to_le_bytes());
    }
    for (k, stake) in bets.stakes.iter().enumerate() {
        put(STAKES + 4 * k, &stake.to_le_bytes());
    }
}

/// The odds of each place of the own league, as many as it has teams.
pub fn place_odds(records: &Records, human: usize) -> Vec<i16> {
    let Some(b) = records.block(&bets_block(human)) else { return Vec::new() };
    let size = usize::try_from(short(b, LEAGUE_SIZE)).unwrap_or(0).min(24);
    (0..size).map(|i| short(b, PLACE_ODDS + 2 * i)).collect()
}

impl SeasonBets {
    /// All the season's stakes (what `FUN_0077fc41` sums; the matchday's
    /// are not counted).
    pub fn total(&self) -> i32 {
        self.pages.iter().flat_map(|p| p.stakes).sum::<i32>() + self.stakes.iter().sum::<i32>()
    }
}

/// What a season stake may still take (`FUN_0077fc41`): the fortune less
/// the season's stakes, at most 20.000, none below.
pub fn season_room(bets: &SeasonBets, fortune: i32) -> i32 {
    (fortune - bets.total()).min(MOST_STAKE).max(0)
}

/// What the game offers on the season (`0x1c65e88`, its data from
/// 0x1c65ea8): a country's top scorers, trainers and cup teams, and over
/// all the European cups' clubs and the tournaments' nations.
const SEASON_OFFER_AT: &str = "0x1c65e88";
const SEASON_SCORERS: usize = 0;
const SEASON_TRAINERS: usize = 0xbcc;
const CUP_TEAMS: usize = 0x13a6;
const CUP_PAGE: usize = 0x102;
const CHAMPIONS_LEAGUE: usize = 0x18b0;
const UEFA_CUP: usize = 0x19a2;
const EUROPEAN: usize = 0x1a94;
const WORLD: usize = 0x1c66;
/// The champions' odds by place, in the globals block (`0xb21820`).
const GLOBALS_AT: &str = "0xb1f860";
const CHAMPION_ODDS: usize = 0x1fc0;
const CHAMPION_PAGE: usize = 0x90;

/// The season's offer on a country, and over all.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SeasonOffer {
    /// The odds of the champion by the league's places.
    pub champion_odds: Vec<i16>,
    /// The cup's teams by number with their odds.
    pub cup: Vec<(i16, i16)>,
    pub scorers: Vec<Scorer>,
    pub trainers: Vec<Trainer>,
    /// The European cups' clubs, by country and number, with their odds.
    pub european_cups: [Vec<((u8, u16), i16)>; 2],
    /// The tournaments' nations with their odds.
    pub tournaments: [Vec<(i16, i16)>; 2],
}

fn scorers(b: &[u8], s: usize) -> Vec<Scorer> {
    (0..usize::try_from(int(b, s)).unwrap_or(0).min(50))
        .map(|i| Scorer {
            names: (short(b, s + 0xcc + 2 * i) as u16, short(b, s + 0x130 + 2 * i) as u16),
            odds: int(b, s + 0x194 + 4 * i),
        })
        .collect()
}

fn trainers(b: &[u8], t: usize) -> Vec<Trainer> {
    (0..usize::try_from(short(b, t)).unwrap_or(0).min(50))
        .map(|i| Trainer {
            id: short(b, t + 2 + 2 * i),
            club: (short(b, t + 0x66 + 2 * i), short(b, t + 0xca + 2 * i)),
            odds: short(b, t + 0x12e + 2 * i),
        })
        .collect()
}

/// The season's offer on the page's country.
pub fn season_offer(records: &Records, page: usize) -> Option<SeasonOffer> {
    let b = records.block(SEASON_OFFER_AT)?;
    let odds = records.block(GLOBALS_AT);
    let champion_odds = (0..24)
        .map(|i| odds.map_or(0, |g| short(g, CHAMPION_ODDS + page * CHAMPION_PAGE + 2 * i)))
        .collect();
    let c = CUP_TEAMS + page * CUP_PAGE;
    let cup = (0..usize::try_from(short(b, c)).unwrap_or(0).min(64))
        .map(|i| (short(b, c + 2 + 2 * i), short(b, c + 0x82 + 2 * i)))
        .collect();
    let clubs = |at: usize| {
        (0..usize::try_from(short(b, at)).unwrap_or(0).min(40))
            .map(|i| {
                let e = at + 2 + 4 * i;
                ((b.get(e).copied().unwrap_or(0), short(b, e + 2) as u16), short(b, at + 0xa2 + 2 * i))
            })
            .collect()
    };
    let nations = |at: usize| {
        (0..usize::try_from(short(b, at)).unwrap_or(0).min(116))
            .map(|i| (short(b, at + 2 + 2 * i), short(b, at + 0xea + 2 * i)))
            .collect()
    };
    Some(SeasonOffer {
        champion_odds,
        cup,
        scorers: scorers(b, SEASON_SCORERS + page * SCORER_PAGE),
        trainers: trainers(b, SEASON_TRAINERS + page * TRAINER_PAGE),
        european_cups: [clubs(CHAMPIONS_LEAGUE), clubs(UEFA_CUP)],
        tournaments: [nations(EUROPEAN), nations(WORLD)],
    })
}

/// Odds in tenths as the lists write them: one place below ten, the whole
/// number from ten on.
pub fn odds(tenths: i32) -> String {
    if tenths < 100 { format!("{:.1}", f64::from(tenths) / 10.0) } else { (tenths / 10).to_string() }
}

/// The settlement's report for dialog 409 (`CWettbueroAuswertung`, the
/// block `0x1c68450`, its data from 0x1c68470): what the last matchday
/// paid every human, a slot for each country page and human.
const REPORT_AT: &str = "0x1c68450";
/// A section's flag, `u16` at `base + page * 2 + human * 10` with the
/// human numbered from 1 (`0x1c68490` & friends); set when the section
/// has lines, never cleared.
const FLAG_FULL: usize = 0x70c;
const FLAG_GOALS: usize = 0x73e;
const FLAG_SCORER: usize = 0x770;
const FLAG_H: usize = 0x7a2;
const FLAG_TRAINER: usize = 0x7d4;
/// The matchday's goals shown per page — −1 where none was played — and
/// the leagues' top scorer.
const SHOWN_GOALS: usize = 0x806;
const TOP_SCORERS: usize = 0x810;
/// The full tip: how many the ticks got, its stake and payout, at
/// `page * 0xc + human * 0x3c`; the goals, first scorer and trainer bets
/// follow in the same shape.
const FULL_REC: usize = 0x81c;
const FULL_HUMAN: usize = 0x3c;
const GOALS_REC: usize = 0x948;
const SCORER_REC: usize = 0xa74;
const TRAINER_REC: usize = 0xba0;
/// The highest win: its two clubs, stake and payout, the winner's goals
/// at +0x10; then the rows of every match ticked for it — two clubs and
/// the goals a row of 0xc — with their count a page and human before.
const WINNER: usize = 0xccc;
const WINNER_HUMAN: usize = 0x64;
const DETAILS_COUNT: usize = 0xec0;
const DETAILS: usize = 0xef2;
const DETAILS_PAGE: usize = 0xf0;
const DETAILS_HUMAN: usize = 0x4b0;
const DETAIL: usize = 0xc;

/// A club as the report's raw pair has it: the country's number, the
/// team's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TeamPair {
    pub country: u8,
    pub number: u16,
}

impl TeamPair {
    fn of_raw(v: u32) -> Self {
        Self { country: v as u8, number: (v >> 16) as u16 }
    }
}

fn put(b: &mut [u8], at: usize, bytes: &[u8]) {
    if let Some(slot) = b.get_mut(at..at + bytes.len()) {
        slot.copy_from_slice(bytes);
    }
}

/// A report flag stands where the section has lines.
fn flag_at(writes: &mut Vec<(usize, Vec<u8>)>, at: usize, page: usize, human: usize) {
    writes.push((at + page * 2 + human * 10, 1u16.to_le_bytes().to_vec()));
}

fn word(b: &[u8], at: usize) -> u32 {
    b.get(at..at + 4).map_or(0, |f| u32::from_le_bytes(f.try_into().unwrap()))
}

/// Where the page's match k stands in the offer.
fn entry(page: usize, k: usize) -> usize {
    MATCHES_AT + page * MATCH_PAGE + k * MATCH
}

/// How correct a tip of the matchday was (`FUN_0077ff36`): nought when
/// the tick and the result agree, two when a home win was tipped away and
/// the other way round, one else; the tip's high nibble is cleared on
/// the way.
fn corrected(expect: u8, actual: u8) -> (u8, i16) {
    let expect = expect & 0xf;
    let tip = expect & OUTCOMES;
    let points = if tip == actual {
        0
    } else if matches!((tip, actual), (AWAY, HOME) | (HOME, AWAY)) {
        2
    } else {
        1
    };
    (expect, points)
}

/// The three quotas a country's page pays for eleven, ten and nine of
/// the full tip (`FUN_0078107b`): how many points its eleven tips missed
/// — two points for a home win tipped away and the other way — squared,
/// two where they missed less than twice eleven; the eleven's quota from
/// that times eleven to twenty rolls over ten, the ten's from it again
/// times eight to twelve and the nine's times five to seven, each with a
/// fresh hundred above its mark when it grows over 45.00 and 650.00.
/// Every page draws, one played or not.
pub fn redraw_quotas(records: &mut Records, random: &mut Random) {
    let Some(b) = records.block_mut(OFFER_AT) else { return };
    for page in 0..PAGES {
        let mut missed = 0i16;
        for k in 0..MATCHES {
            let m = entry(page, k);
            let (Some(expect), Some(actual)) = (b.get(m + 8).copied(), b.get(m + 9).copied()) else {
                break;
            };
            let (expect, points) = corrected(expect, actual);
            if let Some(x) = b.get_mut(m + 8) {
                *x = expect;
            }
            missed += points;
        }
        let short_by = i32::from(missed) - 11;
        let short_by = (short_by * short_by).max(2);
        let eleven = (random.below(0xb) as i32 + 10) * short_by / 10;
        let mut ten = eleven * (random.below(5) as i32 + 8);
        if ten > 0x1194 {
            ten = random.below(100) as i32 + 0x1194;
        }
        let mut nine = ten * (random.below(3) as i32 + 5);
        if nine > 0xfde8 {
            nine = random.below(100) as i32 + 0xfde8;
        }
        for (at, quota) in [(0xf4, eleven), (0xf6, ten), (0xf8, nine)] {
            put(b, page * MATCH_PAGE + at, &(quota as i16).to_le_bytes());
        }
    }
}

/// The last matchday's settlement (`FUN_007811c5`), on the half week the
/// offers are redrawn: for every human and page the full tip's stake is
/// charged and — eleven, ten or nine right — paid by the page's three
/// quotas' factors; the highest win of the unique biggest result pays its
/// stake times the match's quota as a float over ten; the goals' tip pays
/// the league's goal factor, the first scorer the offered odds, the
/// trainer's sacking within his fortnight his odds. Each stake charged is
/// gone from the private fortune at once; afterwards the page's bets
/// stand reset and its offer's count done.
pub fn settle_matchday(records: &mut Records, random: &mut Random) {
    let Some(offer) = records.block(OFFER_AT).map(|b| b.to_vec()) else { return };
    let counts: Vec<i16> = (0..PAGES).map(|p| short(&offer, p * MATCH_PAGE)).collect();
    if counts.iter().all(|&c| c == 0) {
        return;
    }
    // The full tip's factors: thirty thousand chance over the quota read
    // unsigned, so a quota below nought pays nought.
    let mut factor = [0i32; PAGES * 3];
    for (page, &count) in counts.iter().enumerate() {
        if count == 0 {
            continue;
        }
        for j in 0..3 {
            let quota = short(&offer, page * MATCH_PAGE + 0xf4 + 2 * j);
            if quota == 0 {
                continue;
            }
            let roll = i32::try_from(random.below(0x1f)).unwrap_or(0) * 10_000 + 700_000;
            factor[page * 3 + j] = (roll as u32).wrapping_div(quota as u32) as i32;
        }
    }
    // The boards the game follows draw the matchday's goals from their
    // first league: its goals of the last matchday at +0x18, its top
    // scorer at +0x1a04 (`FUN_00692045`, `FUN_006919b5`); the goals'
    // factor counts from how far those stay under three a team.
    let fields = crate::opponent::LEAGUE_FIELDS;
    let mut shown = [0i16; PAGES];
    let mut goals_factor = [0i32; PAGES];
    let mut top = [0u16; PAGES];
    let more = records.globals.more_count().max(0);
    let boards: Vec<(u16, usize)> = (0..=more)
        .filter_map(|i| {
            let country = if i == 0 {
                records.globals.main_country()
            } else {
                *records.globals.more_countries().get(usize::try_from(i).unwrap_or(0) - 1)?
            };
            let page = usize::try_from(records.globals.get::<i16>(0xb5cbb2 + 2 * usize::try_from(i).unwrap_or(0))).ok()?;
            Some((u16::try_from(country).ok()?, page))
        })
        .collect();
    for (country, page) in boards {
        if page >= PAGES {
            continue;
        }
        let Some(name) = crate::opponent::League::block_name(records, country, 0) else { continue };
        let Some(b) = records.block(&name) else { continue };
        let teams = i32::from(short(b, fields));
        let goals = short(b, fields + 0x18);
        shown[page] = goals;
        goals_factor[page] =
            (i32::from(goals) - teams * 3 / 2).abs() + 5 + i32::try_from(random.below(3)).unwrap_or(0);
        top[page] = short(b, fields + 0x1a04) as u16;
    }
    let mut writes: Vec<(usize, Vec<u8>)> = Vec::new();
    for page in 0..PAGES {
        let shown = if counts[page] == 0 { -1 } else { shown[page] };
        writes.push((SHOWN_GOALS + page * 2, shown.to_le_bytes().to_vec()));
        writes.push((TOP_SCORERS + page * 2, top[page].to_le_bytes().to_vec()));
    }
    // Every human's fortune at one stroke; his bets by his number.
    let slots = records.globals.slots();
    let playing: Vec<(usize, usize)> = records
        .managers
        .iter()
        .zip(slots)
        .enumerate()
        .filter(|(_, (_, slot))| *slot > 0)
        .map(|(m, _)| (m, usize::from(records.managers[m].human()).max(m + 1)))
        .collect();
    for (manager, human) in playing {
        let mut delta = 0i32;
        for page in 0..PAGES {
            let mut bet = matchday_bet(records, human - 1, page);
            // The full tip: its stake, how many the ticks got, the
            // payout by the page's factors.
            let stake = bet.full_stake();
            if stake != 0 {
                flag_at(&mut writes, FLAG_FULL, page, human);
                let got = if counts[page] == 0 {
                    0
                } else {
                    i16::try_from(
                        (0..MATCHES)
                            .filter(|&k| {
                                (bet.ticks[k] & OUTCOMES) & offer.get(entry(page, k) + 9).copied().unwrap_or(0) != 0
                            })
                            .count(),
                    )
                    .unwrap_or(0)
                };
                let payout = match got {
                    11 => factor[page * 3] * i32::from(bet.full),
                    10 => factor[page * 3 + 1] * i32::from(bet.full),
                    9 => factor[page * 3 + 2] * i32::from(bet.full),
                    _ => 0,
                };
                let at = FULL_REC + page * 0xc + human * FULL_HUMAN;
                writes.push((at, got.to_le_bytes().to_vec()));
                writes.push((at + 4, stake.to_le_bytes().to_vec()));
                writes.push((at + 8, payout.to_le_bytes().to_vec()));
                delta = delta.wrapping_sub(stake).wrapping_add(payout);
                bet.full = 0;
            }
            // The highest win: the rows of the ticked matches, and the
            // one result the biggest goal difference stands alone with.
            if bet.high_stake > 0 && bet.highest() > 0 {
                flag_at(&mut writes, FLAG_H, page, human);
                let mut rows = 0usize;
                for k in 0..MATCHES {
                    if bet.ticks[k] & HIGHEST == 0 {
                        continue;
                    }
                    let m = entry(page, k);
                    let (home, away) = (word(&offer, m).to_le_bytes(), word(&offer, m + 4).to_le_bytes());
                    let (hg, ag) = (
                        u16::from(offer.get(m + 0xc).copied().unwrap_or(0)).to_le_bytes(),
                        u16::from(offer.get(m + 0xd).copied().unwrap_or(0)).to_le_bytes(),
                    );
                    let row = [home.as_slice(), away.as_slice(), hg.as_slice(), ag.as_slice()].concat();
                    writes.push((DETAILS + page * DETAILS_PAGE + human * DETAILS_HUMAN + rows * DETAIL, row));
                    rows += 1;
                }
                writes.push((DETAILS_COUNT + page * 2 + human * 10, (rows as i16).to_le_bytes().to_vec()));
                let mut diff = 0u8;
                let mut goals = 0u8;
                let mut alone = 0i32;
                let mut winner = -1i32;
                for k in 0..MATCHES {
                    let m = entry(page, k);
                    let (a, g) = (offer.get(m + 0xa).copied().unwrap_or(0), offer.get(m + 0xb).copied().unwrap_or(0));
                    if diff < a {
                        alone = 1;
                        (diff, goals) = (a, g);
                        winner = i32::try_from(k).unwrap_or(0);
                    } else if a == diff {
                        if goals < g {
                            alone = 1;
                            (diff, goals) = (a, g);
                            winner = i32::try_from(k).unwrap_or(0);
                        } else if g == goals {
                            alone += 1;
                        }
                    }
                }
                let at = WINNER + page * 0x14 + human * WINNER_HUMAN;
                writes.push((at + 8, bet.high_stake.to_le_bytes().to_vec()));
                let winner = usize::try_from(winner).ok();
                if alone == 1 {
                    delta = delta.wrapping_sub(bet.high_stake);
                    let m = entry(page, winner.unwrap_or(0));
                    let (home, away) = (word(&offer, m).to_le_bytes(), word(&offer, m + 4).to_le_bytes());
                    writes.push((at, [home.as_slice(), away.as_slice()].concat()));
                    let (hg, ag) = (
                        u16::from(offer.get(m + 0xc).copied().unwrap_or(0)).to_le_bytes(),
                        u16::from(offer.get(m + 0xd).copied().unwrap_or(0)).to_le_bytes(),
                    );
                    writes.push((at + 0x10, [hg.as_slice(), ag.as_slice()].concat()));
                    if winner.is_some_and(|w| bet.ticks[w] & HIGHEST != 0) {
                        let quota = i32::from(short(&offer, m + 0xe));
                        let payout = (bet.high_stake as f32 * quota as f32 / 10.0).trunc() as i32;
                        writes.push((at + 0xc, payout.to_le_bytes().to_vec()));
                        delta = delta.wrapping_add(payout);
                    }
                }
            }
            bet.high_stake = 0;
            bet.ticks = [0; MATCHES];
            // The matchday's goals, the first scorer, the trainer to go.
            if bet.goals_stake > 0 && bet.goals > 0 {
                flag_at(&mut writes, FLAG_GOALS, page, human);
                let payout = if bet.goals == shown[page] { goals_factor[page] * bet.goals_stake } else { 0 };
                let at = GOALS_REC + page * 0xc + human * FULL_HUMAN;
                writes.push((at, bet.goals.to_le_bytes().to_vec()));
                writes.push((at + 4, bet.goals_stake.to_le_bytes().to_vec()));
                writes.push((at + 8, payout.to_le_bytes().to_vec()));
                delta = delta.wrapping_sub(bet.goals_stake).wrapping_add(payout);
            }
            bet.goals_stake = 0;
            bet.goals = -1;
            if bet.scorer_stake > 0 && bet.scorer >= 0 {
                flag_at(&mut writes, FLAG_SCORER, page, human);
                let s = SCORERS_AT + page * SCORER_PAGE;
                let row = usize::try_from(bet.scorer).unwrap_or(0);
                let payout = if int(&offer, s + 4 + 4 * row) == i32::from(top[page] as i16) {
                    bet.scorer_stake * int(&offer, s + 0x194 + 4 * row) / 10
                } else {
                    0
                };
                let at = SCORER_REC + page * 0xc + human * FULL_HUMAN;
                writes.push((at, bet.scorer.to_le_bytes().to_vec()));
                writes.push((at + 4, bet.scorer_stake.to_le_bytes().to_vec()));
                writes.push((at + 8, payout.to_le_bytes().to_vec()));
                delta = delta.wrapping_sub(bet.scorer_stake).wrapping_add(payout);
            }
            bet.scorer_stake = 0;
            bet.scorer = -1;
            if bet.trainer_stake > 0 && bet.trainer != 0 {
                let stake = bet.trainer_stake;
                let at = TRAINER_REC + page * 0xc + human * FULL_HUMAN;
                writes.push((at, bet.trainer.to_le_bytes().to_vec()));
                writes.push((at + 4, stake.to_le_bytes().to_vec()));
                writes.push((at + 8, 0i32.to_le_bytes().to_vec()));
                if !bet.running {
                    bet.running = true;
                    delta = delta.wrapping_sub(stake);
                } else {
                    bet.trainer_stake = 0;
                    bet.trainer = 0;
                    bet.running = false;
                    flag_at(&mut writes, FLAG_TRAINER, page, human);
                }
                // Sacked by now: a human's manager gone from his club's
                // two, a person off his club's three staff (`FUN_0063dd1c`).
                let club = records.team(u16::try_from(bet.trainer_club.0).unwrap_or(0), u16::try_from(bet.trainer_club.1).unwrap_or(0));
                let sacked = match club {
                    Some(t) if bet.trainer < 0 => i32::from(t.humans()[0]) != -i32::from(bet.trainer),
                    Some(t) if bet.trainer > 0 => !t.staff().iter().any(|&s| s == bet.trainer as u16),
                    _ => false,
                };
                if sacked {
                    flag_at(&mut writes, FLAG_TRAINER, page, human);
                    bet.running = false;
                    let payout = i32::from(bet.trainer_odds) * stake / 10;
                    writes.push((TRAINER_REC + page * 0xc + human * FULL_HUMAN + 8, payout.to_le_bytes().to_vec()));
                    bet.trainer_stake = 0;
                    bet.trainer = 0;
                    delta = delta.wrapping_add(payout);
                }
            }
            set_matchday_bet(records, human - 1, page, &bet);
        }
        if let Some(m) = records.managers.get_mut(manager) {
            m.set_cash(m.cash().wrapping_add(delta));
        }
    }
    if let Some(b) = records.block_mut(REPORT_AT) {
        for (at, bytes) in writes {
            put(b, at, &bytes);
        }
    }
    if let Some(b) = records.block_mut(OFFER_AT) {
        for page in 0..PAGES {
            put(b, page * MATCH_PAGE, &0i16.to_le_bytes());
        }
    }
}

/// The match just played stands in the offer, wherever it is (`FUN_0078017d`):
/// its result's bit, the goal difference, the bigger score and both.
pub fn matchday_result(records: &mut Records, home: (u8, u16), away: (u8, u16), goals: (u8, u8)) {
    let Some(b) = records.block_mut(OFFER_AT) else { return };
    for page in 0..PAGES {
        for k in 0..MATCHES {
            let m = entry(page, k);
            let (Some(hc), Some(ac)) = (b.get(m).copied(), b.get(m + 4).copied()) else { continue };
            if (hc, short(b, m + 2) as u16) != home || (ac, short(b, m + 6) as u16) != away {
                continue;
            }
            let result = match goals.0.cmp(&goals.1) {
                std::cmp::Ordering::Greater => HOME,
                std::cmp::Ordering::Equal => DRAW,
                std::cmp::Ordering::Less => AWAY,
            };
            put(b, m + 9, &[result]);
            put(b, m + 0xa, &[goals.0.abs_diff(goals.1)]);
            put(b, m + 0xb, &[goals.0.max(goals.1)]);
            put(b, m + 0xc, &[goals.0, goals.1]);
        }
    }
}

/// Whether the matchday paid this human anything: one of the report's
/// five flags stands (`FUN_0077f1b8` for the event's kind 1).
pub fn report_open(records: &Records, human: usize) -> bool {
    let Some(b) = records.block(REPORT_AT) else { return false };
    let n = human + 1;
    [FLAG_FULL, FLAG_GOALS, FLAG_SCORER, FLAG_H, FLAG_TRAINER]
        .iter()
        .any(|&at| (0..PAGES).any(|page| short(b, at + page * 2 + n * 10) != 0))
}

/// What a page's settlement left for the report dialog.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PageReport {
    /// The full tip: how many right, stake and payout.
    pub full: Option<(i16, i32, i32)>,
    /// The highest win: stake and payout, the unique winner with its
    /// goals, and the rows of every match ticked for it.
    pub high: Option<HighReport>,
    /// The goals: the tip, stake and payout.
    pub goals: Option<(i16, i32, i32)>,
    /// The first scorer: his row in the offer, stake and payout.
    pub scorer: Option<(i16, i32, i32)>,
    /// The trainer: his id, stake and payout.
    pub trainer: Option<(i16, i32, i32)>,
    /// The matchday's goals shown, and the league's top scorer.
    pub shown: i16,
    pub top: i16,
    /// The country of the page's matches.
    pub nation: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HighReport {
    pub stake: i32,
    pub payout: i32,
    /// The unique biggest result, its goals; none where several tie.
    pub winner: Option<(TeamPair, TeamPair, u8, u8)>,
    pub rows: Vec<(TeamPair, TeamPair, u8, u8)>,
}

/// The settlement's report of the page's country for the human's slot.
pub fn page_report(records: &Records, human: usize, page: usize) -> Option<PageReport> {
    let b = records.block(REPORT_AT)?;
    let n = human + 1;
    let flag = |at: usize| short(b, at + page * 2 + n * 10) != 0;
    let rec = |base: usize| (short(b, base + page * 0xc + n * FULL_HUMAN), int(b, base + page * 0xc + n * FULL_HUMAN + 4), int(b, base + page * 0xc + n * FULL_HUMAN + 8));
    let mut out = PageReport {
        shown: short(b, SHOWN_GOALS + page * 2),
        top: short(b, TOP_SCORERS + page * 2),
        ..PageReport::default()
    };
    if flag(FLAG_FULL) {
        out.full = Some(rec(FULL_REC));
    }
    if flag(FLAG_GOALS) {
        out.goals = Some(rec(GOALS_REC));
    }
    if flag(FLAG_SCORER) {
        out.scorer = Some(rec(SCORER_REC));
    }
    if flag(FLAG_TRAINER) {
        out.trainer = Some(rec(TRAINER_REC));
    }
    if flag(FLAG_H) {
        let at = WINNER + page * 0x14 + n * WINNER_HUMAN;
        let mut high = HighReport { stake: int(b, at + 8), payout: int(b, at + 0xc), ..HighReport::default() };
        if short(b, at + 2) >= 1 {
            let (hg, ag) = (short(b, at + 0x10) as u8, short(b, at + 0x12) as u8);
            high.winner = Some((TeamPair::of_raw(word(b, at)), TeamPair::of_raw(word(b, at + 4)), hg, ag));
        }
        let rows = usize::try_from(short(b, DETAILS_COUNT + page * 2 + n * 10)).unwrap_or(0);
        high.rows = (0..rows)
            .map(|k| {
                let m = DETAILS + page * DETAILS_PAGE + n * DETAILS_HUMAN + k * DETAIL;
                (TeamPair::of_raw(word(b, m)), TeamPair::of_raw(word(b, m + 4)), b.get(m + 8).copied().unwrap_or(0), b.get(m + 0xa).copied().unwrap_or(0))
            })
            .collect();
        out.high = Some(high);
    }
    out.nation = (0..MATCHES)
        .map(|k| records.block(OFFER_AT).and_then(|o| o.get(entry(page, k)).copied()).unwrap_or(0))
        .find(|&c| c != 0)
        .unwrap_or(0);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_tip_costs_a_euro_a_combination() {
        // Twice, match one with all three outcomes, the rest home wins:
        // six euros, as the original showed.
        let mut bet = MatchdayBet { full: 2, ticks: [HOME; MATCHES], ..MatchdayBet::default() };
        bet.ticks[0] = HOME | DRAW | AWAY;
        assert_eq!(bet.full_stake(), 6);
        bet.ticks[3] |= HIGHEST;
        bet.high_stake = 30;
        assert_eq!(bet.counted(), 36);
        bet.ticks[5] = 0;
        assert_eq!(bet.full_stake(), 0);
        assert_eq!((odds(31), odds(100), odds(105)), ("3.1".to_owned(), "10".to_owned(), "10".to_owned()));
    }

    #[test]
    fn the_redraw_counts_a_wrong_tip_a_second_where_it_flips_the_outcome() {
        // The points the redraw takes off the missed tips (`FUN_0078107b`
        // via the corrected count): nought when the tip agrees, two when a
        // home win stands away and the other way, one otherwise.
        for (tip, actual, points) in [
            (1u8, 4u8, 2i16), // away tipped, home won
            (4, 1, 2),
            (1, 1, 0),
            (2, 2, 0),
            (1, 2, 1),
            (0, 4, 1), // untipped counts as missed
        ] {
            assert_eq!(corrected(tip, actual).1, points, "{tip} vs {actual}");
        }
        assert_eq!(corrected(1 | 0x10, 1).0, 1); // the high nibble goes off
    }
}
