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
