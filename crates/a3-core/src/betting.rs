//! The betting office (ERFOLGE.md §D): each human's bets, a page for each
//! country played — on the matchday (dialog 411) and on the season (410)
//! —, and what the game offers to bet on. Nothing is paid as a bet is
//! placed; the stakes only must not come to more than the private
//! fortune, and the bets are settled with the matches.

use std::io;

use a3_files::exe::Exe;

use crate::discipline;
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
}
