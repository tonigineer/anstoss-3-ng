//! The talk over a transfer's fee (`FUN_00768638` asm 0x7694cc–0x769b4b):
//! once the bidders' rounds left a bidder whose contract the player took,
//! his club and that bidder bargain the fee — the seller asking, the
//! bidder offering, in thousands, round after round until the asking
//! comes down to the offer on the fee slider's steps, or one of them runs
//! out of patience. A human seller answers in dialog 308
//! (`CVerhandlungUeberAbloese`); a computer seller brings his asking down
//! by `FUN_0076e018`; a computer bidder raises its offer by
//! `FUN_0076e379`.

use crate::records::{Player, Records, Team};
use crate::transfers::Side;

/// The fee slider's steps, in thousands (`DAT_0088cb50`, `FUN_005c690b`,
/// `FUN_005c69c9`).
pub const FEE_STEPS: [(i64, i64); 8] = [
    (50, 1),
    (100, 2),
    (500, 5),
    (1000, 10),
    (5000, 50),
    (10000, 100),
    (50000, 500),
    (crate::finances::ON, 1000),
];

/// An amount's place on the fee slider, rounded down.
pub fn step(v: i32) -> i32 {
    crate::finances::step_of(&FEE_STEPS, i64::from(v))
}

/// The amount at a place of the fee slider.
pub fn amount(p: i32) -> i32 {
    crate::finances::amount_at(&FEE_STEPS, p) as i32
}

/// Whether the asking comes down to the offer (`FUN_0076f98f`): on the
/// slider's steps, no higher.
pub fn agrees(asking: i32, offer: i32) -> bool {
    step(asking) <= step(offer)
}

/// The fee's unit (`DAT_00a3b294`) and the release fee's (`DAT_00a3b3bc`).
const UNIT: i64 = 1000;
const RELEASE_UNIT: i64 = 50_000;
/// The value's cap (`DAT_00a3b29c`), the value above which the asking
/// stands at that cap (`DAT_00a3b2b0`), the asking for a man of no value
/// (`DAT_00a3b330`).
const MOST_VALUE: i64 = 250_000_000;
const HIGH_VALUE: i64 = 50_000_000;
const NO_VALUE_ASKING: i64 = 1_000_000;
/// The asking: five times the value.
const ASKING_TIMES: i64 = 5;
/// The money figure's bounds (`DAT_00a3b2e8`, `DAT_00a3b314`) and the
/// seller's depth (`DAT_00a3b308`).
const RICH: i64 = 5_000_000;
const WELL: i64 = 2_000_000;
const DEEP: i64 = 3_000_000;
/// The factors by a club's money figure, from rich to poor (0x835104,
/// 0x832010, 0x835100, 0x8350fc, 0x8311e8); the poorest seller's
/// (0x8350f8) and buyer's (0x81c088).
const BY_MONEY: [f32; 5] = [1.4, 1.2, 1.05, 0.95, 0.9];
const POOREST_SELLER: f32 = 0.85;
const POOREST_BUYER: f32 = 0.8;
/// More for a man the pick values above 120 (0x8350f4), and for a hard
/// seller; the factor beyond which a seller is hard (0x833ff8).
const WANTED_MORE: f32 = 0.1;
const HARD_FROM: f64 = 1.3;
/// A hard seller's flags (+0x2fa): hard, and pressing.
const HARD: u16 = 3;
const PRESSING: u16 = 1;
/// The bidder's start by the seller's money: deep in debt, in debt, else
/// (0x3f733333, 0x3f866666, 0x3f8ccccd).
const BIDDER_DEEP: f32 = 0.95;
const BIDDER_DEBT: f32 = 1.05;
const BIDDER_START: f32 = 1.1;
/// More for a position the buyer lacks (0x8328d4); less for each man of a
/// squad beyond 18 (0x8328d8), from a marked country (0x8328dc), out a
/// week or five (0x8350f0, 0x8350ec), with an injury of over a hundred
/// days (0x81564c), banned (0x833d94); never under a fifth (0x833f30).
const LACKS: f32 = 0.2;
const SQUAD_EASE: i32 = 0x12;
const PER_MAN: f32 = 0.02;
const MARKED: f32 = 0.4;
const OUT_WEEK: f32 = 0.05;
const OUT_LONG: f32 = 0.15;
const LONG_INJURY: f32 = 0.3;
const LONG_INJURY_DAYS: u8 = 100;
const BANNED: f32 = 0.6;
const LEAST_FACTOR: f32 = 0.2;
/// The bidder's most over the value: for a man listed for a fee
/// (0x82e5c0), else (0x832010).
const MOST_LISTED: f32 = 1.1;
const MOST_OTHER: f32 = 1.2;
/// The opening offers, in hundredths of the bounds: the seller 120 and a
/// draw in 21 above his least, the bidder 80 less such a draw below its
/// most, each with its manager's skill.
const SELLER_OPENS: i64 = 120;
const BIDDER_OPENS: i64 = 80;
const OPENING_DRAW: u32 = 21;
/// Each side's patience (+0x40) as the rounds start.
const PATIENCE: i16 = 9;
/// `FUN_0076e379`'s raise: under 40 in a hundred 5 per cent (0x80e740),
/// else 1 to 10 per cent; the patience a cut-down offer brings back.
const RAISE_ODDS: u32 = 40;
const RAISE: f64 = 1.05;
const MET_PATIENCE: i16 = 6;
/// `FUN_0076e018`'s cuts of the asking: three per cent (0x835108) when
/// the offer has barely come up, five (0x833f80) in forty; eight to
/// twelve and one to eleven per cent with a draw (92 and 99 less one),
/// and ten (0x833fa0) in forty between a tenth of the way and its odds.
const OFF_LITTLE: f64 = 0.97;
const OFF_SOME: f64 = 0.95;
const OFF_FAR: f64 = 0.90;

/// A running fee talk (`0x1c5bf38`): the seller's frame and the bidder's
/// whose contract the player took (+0x2b4), their asking and offer (+0x54
/// each, in thousands), their patience (+0x40) and whether they are out
/// (+0x02), and the bounds `FUN_0076d60e` set.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FeeTalk {
    pub player: u16,
    pub seller: Side,
    pub buyer: Side,
    /// Each side's human manager, 0 for the computer (frame +0x00).
    pub managers: [u8; 2],
    /// What the man is worth to the talk, in the game's money (-0x34).
    pub value: i64,
    /// The first asking (+0x30c), what the buyer can pay of it (+0x308),
    /// the least the seller takes (+0x2fc), the most the bidder pays
    /// (+0x300), in thousands.
    pub asking: i32,
    pub affordable: i32,
    pub least: i32,
    pub most: i32,
    /// The last offer the seller has seen (+0x304), in thousands; the
    /// block is zeroed when the talk ends, so it starts at nothing.
    pub seen: i32,
    /// A hard seller's flags (+0x2fa); the release fee as a cap (+0x310,
    /// +0x314, in the game's money).
    pub hard: u16,
    pub fixed: Option<i64>,
    /// The seller's asking and the bidder's offer (frames' +0x54).
    pub demand: i32,
    pub offer: i32,
    pub patience: [i16; 2],
    pub out: [bool; 2],
    /// Whether he would come at once: the bidder's terms "ab sofort"
    /// (+0x3a 0x20), which the dialog's header tells.
    pub at_once: bool,
    /// The rounds gone (+0x324, `DAT_01c5c25c`).
    pub round: i16,
}

/// How a fee talk ended.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ending {
    /// Agreed on the fee, in the game's money.
    Agreed(i64),
    /// One side broke the talk off.
    Failed,
}

impl FeeTalk {
    /// The man is worth to the talk (`FUN_00709e0b` for a man listed for
    /// a fee: a month's worth for each month his contract runs, nine at
    /// most; else `FUN_00709cba`: his market value by the hundredth, times
    /// 100 — 120 when the seller has no other man at his position, 10
    /// more with one (not a keeper's or a forward's place 3 or 10), two
    /// more for each man his squad lacks of 20); 250 Mio at most.
    fn value(records: &Records, values: &crate::value::ValueTables, p: &Player, seller: &Team, listed: bool) -> i64 {
        let position = p.positions()[0];
        let star = records.globals.stars().get(usize::from(position)) == Some(&p.id());
        let worth = crate::value::Worth {
            strength: p.strength(),
            position,
            talent: p.talent(),
            age: crate::free_agents::age(records, p),
            good: p.good(),
            character: p.traits(),
            qualities: p.qualities(),
            star,
            kind_12_matches: p.kind_12_matches(),
        };
        let years = i32::from(p.contract() & 0xfff) - i32::from(records.globals.start_year());
        let month = u32::from(records.globals.date()[1]);
        if listed {
            let months = values.months_left(years, month);
            if months < 1 {
                return 0;
            }
            return (values.monthly(&worth) * i64::from(months.min(9))).min(MOST_VALUE);
        }
        let mut same = 0;
        for id in seller.players() {
            if id != p.id() && records.player(id).is_some_and(|q| q.positions()[0] == position) {
                same += 1;
            }
        }
        let mut weight: i64 = if same == 0 { 120 } else { 100 };
        if same == 1 && position != 3 && position != 10 {
            weight += 10;
        }
        let squad = i64::from(seller.player_count() as i16);
        if squad < 20 {
            weight += (20 - squad) * 2;
        }
        (values.market_value(&worth, years, month) / 100 * weight).min(MOST_VALUE)
    }

    /// `FUN_0076d60e`: the talk's bounds and the opening asking and offer.
    /// The asking is five times the value (250 Mio at a value over 50 Mio,
    /// 1 Mio at none), at least the release fee; the seller's least is the
    /// value times his factor — by his money, more for a man the pick
    /// values above 120, and for one he holds hard (over 1.3, not listed
    /// for a fee and no release fee) — for a man listed for a fee the
    /// value itself unless the factor lies under 1; the bidder's most the
    /// value times its own factor (`bargain` below) and 1.1 (listed) or
    /// 1.2, the release fee at most. A human seller opens on the asking;
    /// a computer one at 120 and a draw in 21 and his manager's skill
    /// hundredths of his least (with a release fee the asking, twice the
    /// least at most); a computer bidder at 80 less a draw in 21 and its
    /// manager's skill hundredths of its most, a human one at nothing.
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        records: &Records,
        random: &mut crate::random::Random,
        tables: (&crate::contract::ContractTables, &crate::value::ValueTables),
        medical: &crate::medical::MedicalTables,
        player: u16,
        seller: Side,
        buyer: Side,
        managers: [u8; 2],
    ) -> Option<Self> {
        let p = records.player(player)?;
        let seller_team = records.team(u16::from(seller.0), seller.1 as u16)?;
        let buyer_team = records.team(u16::from(buyer.0), buyer.1 as u16)?;
        let listed = crate::transfers::listed_terms(records, player) == Some(crate::transfers::FOR_A_FEE);
        let fixed = (p.fixed_fee() > 0).then(|| i64::from(p.fixed_fee()) * RELEASE_UNIT);
        let value = Self::value(records, tables.1, p, seller_team, listed);
        let mut asking = if value > HIGH_VALUE {
            MOST_VALUE / UNIT
        } else if value > 0 {
            value * ASKING_TIMES / UNIT
        } else {
            NO_VALUE_ASKING / UNIT
        };
        if let Some(fee) = fixed
            && fee > asking * UNIT
        {
            asking = fee / UNIT;
        }
        let prices = records.globals.fund_prices();
        let agreed = records.agreed();
        // FUN_00622bd9; DAT_00a3b298's cap on it lies beyond any asking.
        let investable = crate::finances::Balance::of(buyer_team, prices, &agreed).investable() / UNIT;
        let affordable = asking.min(investable);
        let by_money = |money: i64, poorest: f32| -> f32 {
            if money > RICH {
                BY_MONEY[0]
            } else if money > WELL {
                BY_MONEY[1]
            } else if money > 0 {
                BY_MONEY[2]
            } else if money > -WELL {
                BY_MONEY[3]
            } else if money > -RICH {
                BY_MONEY[4]
            } else {
                poorest
            }
        };
        let times = |f: f32, by: f32| (f64::from(f) * f64::from(by)) as f32;
        let plus = |f: f32, by: f32| (f64::from(f) + f64::from(by)) as f32;
        let seller_money = crate::finances::figure(seller_team, prices);
        let wanted = crate::transfers::wanted(tables.1, p);
        let mut f = times(1.0, by_money(seller_money, POOREST_SELLER));
        if wanted {
            f = plus(f, WANTED_MORE);
        }
        let mut hard = 0;
        if f64::from(f) > HARD_FROM && !listed && fixed.is_none() {
            hard = HARD;
            f = plus(f, WANTED_MORE);
        }
        let seller_skill = crate::settlement::manager_skill(records, seller_team);
        let mut least = if listed && f >= 1.0 {
            value / UNIT
        } else {
            (value as f64 * f64::from(f)) as i64 / UNIT
        };
        if let Some(fee) = fixed
            && least * UNIT > fee
        {
            least = fee / UNIT;
        }
        let demand = if managers[0] != 0 {
            asking
        } else if fixed.is_some() {
            asking.min(2 * least)
        } else {
            let share = i64::from(random.below(OPENING_DRAW)) + SELLER_OPENS + seller_skill;
            (share as f64 * f64::from(least as f32) / 100.0) as i64
        };
        // The bidder's factor.
        let mut g = if seller_money < -DEEP {
            BIDDER_DEEP
        } else if seller_money < 0 {
            BIDDER_DEBT
        } else {
            BIDDER_START
        };
        if wanted {
            g = plus(g, WANTED_MORE);
        }
        let position = p.positions()[0];
        let lacks = !buyer_team
            .players()
            .into_iter()
            .any(|id| records.player(id).is_some_and(|q| q.positions()[0] == position));
        if lacks {
            g = plus(g, LACKS);
        }
        let squad = i32::from(buyer_team.player_count() as i16);
        if squad > SQUAD_EASE {
            g = (f64::from(g) - f64::from(squad - SQUAD_EASE) * f64::from(PER_MAN)) as f32;
        }
        g = times(g, by_money(crate::finances::figure(buyer_team, prices), POOREST_BUYER));
        if tables.0.marked_country(seller_team.country()) {
            g = plus(g, -MARKED);
        }
        let out = crate::load::days_to(&records.globals, p.injured_to());
        if out >= 7 {
            g = plus(g, -OUT_WEEK);
        }
        if out > 0x23 {
            g = plus(g, -OUT_LONG);
        }
        if out > 0 && p.injury_kind() != 0 && medical.base_days(p.injury_kind()) > LONG_INJURY_DAYS {
            g = plus(g, -LONG_INJURY);
        }
        if crate::load::days_to(&records.globals, p.long_to()) != 0 {
            g = plus(g, -BANNED);
        }
        if f64::from(g) <= 0.2 {
            g = LEAST_FACTOR;
        }
        let buyer_skill = crate::settlement::manager_skill(records, buyer_team);
        let over = if listed { MOST_LISTED } else { MOST_OTHER };
        let mut most = (value as f64 * f64::from(g) * f64::from(over)) as i64 / UNIT;
        let mut offer = if managers[1] != 0 {
            0
        } else {
            let share = BIDDER_OPENS - i64::from(random.below(OPENING_DRAW)) + buyer_skill;
            (share as f64 * f64::from(most as f32) / 100.0) as i64
        };
        if let Some(fee) = fixed {
            if most * UNIT > fee {
                most = fee / UNIT;
            }
            offer = offer.min(most);
        }
        Some(Self {
            player,
            seller,
            buyer,
            managers,
            value,
            asking: asking as i32,
            affordable: affordable as i32,
            least: least as i32,
            most: most as i32,
            hard,
            fixed,
            demand: demand as i32,
            offer: offer as i32,
            seen: 0,
            patience: [PATIENCE; 2],
            out: [false; 2],
            at_once: false,
            round: 0,
        })
    }

    /// After the seller's turn: out, the talk fails; his asking down to
    /// the offer, agreed on the offer.
    pub fn after_seller(&mut self) -> Option<Ending> {
        if self.out[0] {
            return Some(Ending::Failed);
        }
        if agrees(self.demand, self.offer) {
            self.demand = self.offer;
            return Some(self.agreed());
        }
        None
    }

    /// The computer seller's turn (`FUN_0076e018`): holding out hard and
    /// not pressing, an offer under his least costs him three patience
    /// and brings the pressing bit on; pressing, an offer at or under it
    /// costs one and out of patience he is out, else the bit goes off. An
    /// offer not over the last he saw costs two patience. Else, the
    /// offer risen, his asking comes down by how far the offer reached
    /// towards it, and one patience less; never under his least, under
    /// the offer, or over the release fee's cap.
    pub fn computer_seller_turn(&mut self, random: &mut crate::random::Random) {
        let mut roll = -1;
        if self.hard != 0 {
            if self.hard & PRESSING == 0 {
                if self.offer < self.least {
                    self.patience[0] -= 3;
                    if self.patience[0] < 1 {
                        self.out[0] = true;
                    }
                    self.hard |= PRESSING;
                    return;
                }
            } else if self.offer <= self.least {
                self.patience[0] -= 1;
                if self.patience[0] > 0 {
                    return;
                }
                self.out[0] = true;
                return;
            } else {
                self.hard &= !PRESSING;
                roll = 0;
            }
        }
        if self.seen >= self.offer {
            self.patience[0] -= 2;
            if self.patience[0] < 1 {
                self.out[0] = true;
            }
            return;
        }
        // How far the offer came up towards the asking, in hundredths.
        let mut pct = 5.0f32;
        let gap = self.demand - self.seen;
        if gap > 0 {
            pct = ((self.offer - self.seen) as f32 * 100.0) / gap as f32;
        }
        self.seen = self.offer;
        if roll == -1 {
            roll = random.below(100) as i32;
        }
        if pct <= 5.0 {
            self.demand = (f64::from(self.demand) * OFF_LITTLE) as i32;
        } else if roll < 40 {
            self.demand = (f64::from(self.demand) * OFF_SOME) as i32;
        } else if pct <= 10.0 {
            let by = f64::from(92 - random.below(5) as i32);
            self.demand = (by * f64::from(self.demand as f32) / 100.0) as i32;
        } else if roll < 40 {
            self.demand = (f64::from(self.demand) * OFF_FAR) as i32;
        } else {
            let by = f64::from(99 - random.below(10) as i32);
            self.demand = (by * f64::from(self.demand as f32) / 100.0) as i32;
        }
        if self.demand < self.least {
            self.demand = self.least;
        }
        if self.demand < self.offer {
            self.demand = self.offer;
        }
        if let Some(cap) = self.fixed
            && (cap as u32) < self.demand as u32
        {
            self.demand = cap as i32;
        }
        self.patience[0] -= 1;
        if self.patience[0] < 1 {
            self.out[0] = true;
        }
    }

    /// The bidder's turn (a computer's by `FUN_0076e379`) and the round's
    /// end: out, the talk fails; the asking down to the offer, agreed on
    /// the asking. (A human bidder stays in on his opening terms
    /// (`FUN_0076a19b`) and never takes the man: the winner's of a
    /// talk's rounds is a computer club's frame.)
    pub fn bidder_turn(&mut self, random: &mut crate::random::Random) -> Option<Ending> {
        if self.managers[1] == 0 {
            self.raise(random);
        }
        if self.out[1] {
            return Some(Ending::Failed);
        }
        if agrees(self.demand, self.offer) {
            self.offer = self.demand;
            return Some(self.agreed());
        }
        self.round += 1;
        None
    }

    fn agreed(&self) -> Ending {
        Ending::Agreed(i64::from(self.demand) * UNIT)
    }

    /// `FUN_0076e379`: two less patience for an offer already at the most
    /// and two for one under half the asking; then under 40 in a hundred
    /// the offer 5 per cent up, else 1 to 10 per cent and a draw in two
    /// less patience; one less again. Beyond the most it stands at the
    /// most (in the first three rounds his patience at 1); else beyond
    /// nine tenths of it one less in the first three rounds, and in the
    /// first four at least 1. Beyond the asking it meets it, his patience
    /// at 6. Run out, he is out.
    fn raise(&mut self, random: &mut crate::random::Random) {
        let patience = &mut self.patience[1];
        if self.offer == self.most {
            *patience -= 2;
        }
        if self.offer < self.demand / 2 {
            *patience -= 2;
        }
        if random.below(100) < RAISE_ODDS {
            self.offer = (f64::from(self.offer) * RAISE) as i32;
        } else {
            let by = f64::from(random.below(10) + 101);
            self.offer = (by * f64::from(self.offer as f32) / 100.0) as i32;
            *patience -= random.below(2) as i16;
        }
        *patience -= 1;
        if self.offer as u32 > self.most as u32 {
            self.offer = self.most;
            if self.round < 3 {
                *patience = 1;
            }
        } else if self.offer as u32 > self.most as u32 / 10 * 9 {
            if self.round < 3 {
                *patience -= 1;
            }
            if self.round < 4 && *patience < 1 {
                *patience = 1;
            }
        }
        if self.offer > self.demand {
            self.offer = self.demand;
            *patience = MET_PATIENCE;
        }
        if *patience < 1 {
            self.out[1] = true;
        }
    }

    /// The seller's answer in dialog 308: his asking at a slider's step
    /// (`FUN_005bce4d`), at the top step beyond the first asking holding
    /// out (+0x2fa bit 0, `FUN_005bcdc3`).
    pub fn ask(&mut self, demand: i32, pressing: bool) {
        self.demand = demand;
        if pressing {
            self.hard |= PRESSING;
        } else {
            self.hard &= !PRESSING;
        }
    }

    /// Whether the seller's slider stands at its top step, past the first
    /// asking (+0x2fa bit 0).
    pub fn pressing(&self) -> bool {
        self.hard & PRESSING != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fee_slider_rounds_to_its_steps() {
        // Nsoki's asking of 10 350 thousands stands at 10 Mio on the slider.
        assert_eq!(amount(step(10_350)), 10_000);
        assert_eq!(amount(step(2_017)), 2_000);
        assert!(agrees(2_049, 2_000) && !agrees(2_050, 2_049));
    }

    /// A talk on Nsoki's bounds (least 2 070, asking 10 000) for the
    /// seller's-turn tests.
    fn seller_at(least: i32, demand: i32, offer: i32, seen: i32, hard: u16) -> FeeTalk {
        FeeTalk {
            player: 3262,
            seller: (14, 84),
            buyer: (59, 3),
            managers: [0, 0],
            value: 2_070_000,
            asking: 10_350,
            affordable: 0,
            least,
            most: 2_459,
            seen,
            hard,
            fixed: None,
            demand,
            offer,
            patience: [9, 9],
            out: [false, false],
            at_once: false,
            round: 0,
        }
    }

    #[test]
    fn the_computer_seller_asks_less_as_the_offer_rises() {
        let mut random = crate::random::Random::from_state(1, 1);
        // The offer risen from 800 to 900 of the 9 200 between — a
        // hundredth of the way — three per cent off the asking, and the
        // seller remembers the offer; one patience less.
        let mut fee = seller_at(2_070, 10_000, 900, 800, 0);
        fee.computer_seller_turn(&mut random);
        assert_eq!((fee.seen, fee.patience[0], fee.out[0]), (900, 8, false));
        assert!((9_699..=9_700).contains(&fee.demand));
        // An offer not over the last he saw costs two patience and holds
        // the asking.
        let mut fee = seller_at(2_070, 10_000, 900, 900, 0);
        fee.computer_seller_turn(&mut random);
        assert_eq!((fee.patience[0], fee.demand), (7, 10_000));
        // Holding out hard (+0x2fa 2) and not pressing, an offer under
        // his least costs three and brings the pressing bit on.
        let mut fee = seller_at(2_070, 10_000, 900, 0, 2);
        fee.computer_seller_turn(&mut random);
        assert_eq!((fee.patience[0], fee.hard, fee.demand), (6, 3, 10_000));
        // Pressing, an offer at his least costs one patience; out of
        // patience, he is out.
        let mut fee = seller_at(900, 10_000, 900, 900, 1);
        fee.patience[0] = 1;
        fee.computer_seller_turn(&mut random);
        assert!(fee.out[0]);
    }
}
