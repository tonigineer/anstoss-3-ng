//! The association's court ("Verbandsgericht"): the week start books a
//! day for it in a human's planner when one of his club's men has a case
//! pending (`FUN_0072d850`), and on that day it hears them one by one
//! (`FUN_00628309`, run from the planner's pump `FUN_0072fd53`), each
//! verdict or appeal told in dialog 89 (event 0xa3, `CGerichtDialog`).
//!
//! A case stands on the player's live object, which `FUN_00636459` builds
//! from his record and `FUN_006364cf` stores back:
//!
//! - the league ban, L+0x35 (R+0x22f): the matches in the low five bits,
//!   0x80 a verdict pending, 0x40 an appeal filed; L+0x36 (R+0x230) what
//!   for (0x80 a red card), and L+0xc6 (R+0x1f2) the day it ends;
//! - the European cups' ban, L+0x3a (R+0x238), the same bits, L+0x3b
//!   (R+0x239) what for;
//! - a doping ban, the long absence's days L+0x6e (to R+0x1ec/+0x1ee) with
//!   L+0x68 (R+0x26e): 0x80 a verdict pending, 0x40 an appeal filed; and
//!   the player's earlier cases, L+0x31 (R+0x22b).
//!
//! Each hearing books the lawyer's tariff (0x89e45c, by the club's lawyer
//! +0x5e0: a red card's, or the other's for doping) times 1,000 into the
//! club's lawyer costs (+0x2b4).

use crate::finances::CLUB_TO_TEAM;
use crate::random::Random;
use crate::records::{Appointment, Records};

/// The planner's day of the court (2228 "Verbandsgericht"), its +4 the
/// bans to hear: 1 (3, 5, 6) the league's and doping, 2 (4) the European
/// cups'.
pub const COURT: u8 = 0x13;
pub const LEAGUE_HEARING: u16 = 1;
pub const EUROPE_HEARING: u16 = 2;
/// The week start's other booking: a negotiation the club sits at.
const NEGOTIATION: u8 = 0x0e;
/// The week's days the court may sit on: Tuesday, Wednesday, Friday and
/// Saturday of the week from Monday.
const COURT_DAYS: [i16; 4] = [1, 2, 4, 5];

const LEAGUE_BAN: usize = 0x22f;
const LEAGUE_KIND: usize = 0x230;
const BAN_ENDS: usize = 0x1f2;
const EUROPE_BAN: usize = 0x238;
const EUROPE_KIND: usize = 0x239;
const LONG: usize = 0x26e;
const EARLIER: usize = 0x22b;
const PENDING: u8 = 0x80;
const APPEALED: u8 = 0x40;
const MATCHES: u8 = 0x1f;
const RED_CARD: u8 = 0x80;
/// The earlier cases' marks (`FUN_0062965d`): 0x80 once, 0x40 twice.
const ONCE: u8 = 0x80;
const TWICE: u8 = 0x40;
/// What a doping verdict's `FUN_00628fb6` is told for the kind.
const DOPING: i32 = 100;

/// The club's lawyer (club +0x5e0) and its lawyer costs (+0x2b4).
const LAWYER_AT: usize = 0x5e0 + CLUB_TO_TEAM;
const COSTS_AT: usize = 0x2b4 + CLUB_TO_TEAM;
/// The tariffs a hearing, in thousands, by lawyer: a red card's, the
/// other's (0x89e45c); `DAT_00a3b3e4`.
const TARIFFS: [(i32, i32); 5] = [(10, 30), (15, 50), (20, 80), (30, 120), (50, 200)];
const TARIFF_UNIT: i32 = 1000;
/// How good each lawyer is (`DAT_00b217b4`, in the block from
/// `0xb1f860`): his place, with one pair of neighbours swapped at the new
/// game (`FUN_0078a810(4)`).
const QUALITY_BLOCK: &str = "0xb1f860";
const QUALITY_AT: usize = 0xb217b4 - 0xb1f860;
const LAWYERS: usize = 5;

/// The lawyers' ranks at a new game (`FUN_0065e659`'s first-season
/// branch at 0x65ea7a): 0 to 4 laid down in order, then one draw below 4
/// swapping it with the rank behind it.
pub(crate) fn open_ranks(records: &mut Records, random: &mut Random) {
    let swap = random.below(LAWYERS as u32 - 1) as usize;
    if let Some(b) = records.block_mut(QUALITY_BLOCK)
        && let Some(rows) = b.get_mut(QUALITY_AT..QUALITY_AT + LAWYERS * 2)
    {
        for (k, row) in rows.chunks_exact_mut(2).enumerate() {
            let rank = if k == swap {
                swap + 1
            } else if k == swap + 1 {
                swap
            } else {
                k
            };
            row.copy_from_slice(&(rank as i16).to_le_bytes());
        }
    }
}

/// The verdict texts, by `DAT_00a3b454` (0x8818cc): 0 "wird neu auf %d
/// Jahre festgelegt." … 9 "ist aufgehoben. Er ist ab sofort wieder
/// spielberechtigt.", 10 to 12 the one-match forms of 2, 5 and 8.
pub const VERDICT_TEXTS: [u16; 13] =
    [2094, 2095, 2096, 2097, 2098, 2099, 2100, 2101, 2102, 2103, 2793, 2794, 2795];

/// A player's bans as the court sees them, his live object's part
/// (`FUN_00636459`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Live {
    league: u8,
    league_kind: u8,
    ends: i16,
    europe: u8,
    europe_kind: u8,
    long: u8,
    days: i32,
    earlier: u8,
}

impl Live {
    /// The league ban stands while its day has not passed; a red card's
    /// is dropped once it has, or without a day (`FUN_00636459`).
    fn of(records: &Records, id: u16) -> Option<Live> {
        let r = records.player(id)?;
        let today = records.globals.day() as i16;
        let (ban, kind) = (r.get::<u8>(LEAGUE_BAN), r.get::<u8>(LEAGUE_KIND));
        let mut ends = r.get::<i16>(BAN_ENDS);
        let red_dropped = kind & RED_CARD != 0;
        let (league, league_kind) = if ends == 0 || ends < today {
            if ends != 0 {
                ends = 0;
            }
            if red_dropped { (0, 0) } else { (ban, kind) }
        } else {
            if ban == 0 {
                ends = 0;
            }
            (ban, kind)
        };
        Some(Live {
            league,
            league_kind,
            ends,
            europe: r.get(EUROPE_BAN),
            europe_kind: r.get(EUROPE_KIND),
            long: r.get(LONG),
            days: crate::load::days_to(&records.globals, r.long_to()),
            earlier: r.get(EARLIER),
        })
    }

    /// Back into the record (`FUN_006364cf`): the long absence's end from
    /// its days, none once they are spent.
    fn store(&self, records: &mut Records, id: u16) {
        let before = Live::of(records, id);
        let to = if self.days < 1 {
            [0, 0]
        } else {
            crate::medical::date_in(&records.globals, self.days)
        };
        let Some(r) = records.player_mut(id) else { return };
        r.put(LEAGUE_BAN, self.league);
        r.put(LEAGUE_KIND, self.league_kind);
        r.put(BAN_ENDS, self.ends);
        r.put(EUROPE_BAN, self.europe);
        r.put(EUROPE_KIND, self.europe_kind);
        r.put(LONG, self.long);
        r.put(EARLIER, self.earlier);
        if before.is_none_or(|b| b.days != self.days) {
            r.set_long_to(to);
        }
    }

    fn doping_ban(&self) -> bool {
        self.days != 0
    }
}

/// The squad's first man whose bans `case` picks (`FUN_00628da4` and its
/// kin over club +0xb50).
fn first(records: &Records, club: (u8, i16), case: impl Fn(&Live) -> bool) -> Option<u16> {
    let team = records.team(u16::from(club.0), club.1 as u16)?;
    team.players().into_iter().find(|&id| Live::of(records, id).is_some_and(|l| case(&l)))
}

fn league_appeal(l: &Live) -> bool {
    l.league & APPEALED != 0
}
fn league_verdict(l: &Live) -> bool {
    l.league & PENDING != 0
}
fn doping_appeal(l: &Live) -> bool {
    l.doping_ban() && l.long & APPEALED != 0
}
fn doping_verdict(l: &Live) -> bool {
    l.doping_ban() && l.long & PENDING != 0
}
fn europe_appeal(l: &Live) -> bool {
    l.europe & APPEALED != 0
}
fn europe_verdict(l: &Live) -> bool {
    l.europe & PENDING != 0
}

/// Whether the club has a league or doping case pending (`FUN_006296cc`),
/// and one of the European cups (`FUN_0062973b`).
pub fn pending(records: &Records, club: (u8, i16)) -> (bool, bool) {
    let any = |case: fn(&Live) -> bool| first(records, club, case).is_some();
    (
        any(league_appeal) || any(league_verdict) || any(doping_appeal) || any(doping_verdict),
        any(europe_appeal) || any(europe_verdict),
    )
}

/// The week start's bookings in each human club's planner
/// (`FUN_0072d850`, from `FUN_0064e25b`): a day the club sits at a
/// negotiation (`FUN_0076208a`) takes that, whatever stood there; else on
/// the week's second, third, fifth or sixth day, kept free, the court —
/// the league's first, then the European cups', on the same day, the
/// second over the first.
pub fn book_week(records: &mut Records) {
    let week_start = records.globals.week_start();
    let talks = crate::transfers::appointments(records);
    for club in crate::statistics::human_clubs(records) {
        let Some(slot) = records
            .team(u16::from(club.0), club.1 as u16)
            .and_then(|t| usize::from(t.human()).checked_sub(1))
        else {
            continue;
        };
        let (mut league, mut europe) = pending(records, club);
        let Some(planner) = records.planners.get_mut(slot) else { continue };
        for i in 0..7 {
            let day = week_start + i;
            let Ok(at) = usize::try_from(day) else { continue };
            let talk = talks.iter().find(|t| t.day == day && t.clubs.contains(&club));
            if let Some(t) = talk.filter(|t| t.player != 0) {
                planner.book(at, Appointment { kind: NEGOTIATION, flag: 1, player: t.player, club: 0 });
                continue;
            }
            if !COURT_DAYS.contains(&i) || planner.day(at).kind != 0 {
                continue;
            }
            if league {
                planner.book(at, Appointment { kind: COURT, flag: 1, player: LEAGUE_HEARING, club: 0 });
                league = false;
            }
            if europe {
                planner.book(at, Appointment { kind: COURT, flag: 1, player: EUROPE_HEARING, club: 0 });
                europe = false;
            }
        }
    }
}

/// What the court told about one man (`DAT_00a3b44c`): the club, the
/// player, the verdict's text (`VERDICT_TEXTS`) and its number; whether it
/// asks to appeal and the lawyers advise to; whether the club's manager
/// is shown it (club +0x8b), else the lawyers' advice decides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Case {
    pub club: (u8, i16),
    pub player: u16,
    pub verdict: u8,
    pub value: i16,
    pub question: bool,
    pub advised: bool,
    pub shown: bool,
    appeal: Option<Appeal>,
    /// A verdict's tariff, booked once it is answered: whether the
    /// other's (doping) rather than a red card's.
    billed: Option<bool>,
}

/// Which ban an answer appeals, and on which answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Appeal {
    of: Ban,
    on: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ban {
    League,
    Doping,
    Europe,
}

impl Case {
    fn new(club: (u8, i16), player: u16, (verdict, value): (u8, i16), shown: bool) -> Self {
        Case { club, player, verdict, value, question: false, advised: false, shown, appeal: None, billed: None }
    }

    /// The verdict's text id.
    pub fn text(&self) -> u16 {
        VERDICT_TEXTS[usize::from(self.verdict).min(VERDICT_TEXTS.len() - 1)]
    }
}

/// The verdict on a ban (`FUN_00628fb6`): none — "aufgehoben"; a doping
/// ban's months (to 500 days, `(d + 10) / 30`) or years (`/ 360`); one
/// match or more.
fn verdict(ban: i32, kind: i32) -> (u8, i16) {
    if ban < 1 {
        (9, 0)
    } else if kind == DOPING {
        if ban < 501 { (7, ((ban + 10) / 30) as i16) } else { (6, ((ban + 10) / 360) as i16) }
    } else if ban == 1 {
        (12, 1)
    } else {
        (8, ban as i16)
    }
}

/// An appeal's outcome (`FUN_00629106`): nothing taken off — the ban
/// "bleibt bestehen", the matches still to sit out; all of it —
/// "aufgehoben"; a doping ban's new months or years; matches taken off.
fn outcome(cut: i32, left: i32, kind: i32) -> (u8, i16) {
    if cut == 0 {
        if kind == DOPING {
            (4, 0)
        } else if left == 1 {
            (11, 1)
        } else {
            (5, left as i16)
        }
    } else if left == 0 {
        (3, 0)
    } else if kind == DOPING {
        if left < 501 { (1, ((left + 10) / 30) as i16) } else { (0, ((left + 10) / 360) as i16) }
    } else if cut == 1 {
        (10, 1)
    } else {
        (2, cut as i16)
    }
}

/// The matches an upheld appeal takes off a ban (`FUN_00628309`): one
/// match all, four and five down to three, seven to twelve down to six
/// (the European cups' from seven), else none.
fn matches_off(ban: u8, europe: bool) -> u8 {
    let from = if europe { 7 } else { 6 };
    match ban {
        0..=1 => ban,
        4..=5 => ban - 3,
        _ if ban >= from && ban <= 12 => ban - 6,
        _ => 0,
    }
}

/// The association's word on the club's manners (`FUN_00695073`): 5
/// "ausgezeichnet" down to 1, one lower for bits 0x20 and 0x40.
pub fn standing(manners: u8) -> i32 {
    let mut word = match manners & 0x1f {
        0..=1 => 5,
        2..=4 => 4,
        5..=8 => 3,
        9..=22 => 2,
        _ => 1,
    };
    if manners & 0x60 != 0 && word > 1 {
        word -= 1;
    }
    word
}

/// The club's side of a hearing: its lawyer's quality, the association's
/// word, whether its country's matches are played, and its manager shown.
struct Club {
    key: (u8, i16),
    lawyer: usize,
    quality: i32,
    standing: i32,
    played: bool,
    shown: bool,
}

impl Club {
    fn of(records: &Records, key: (u8, i16)) -> Option<Club> {
        let team = records.team(u16::from(key.0), key.1 as u16)?;
        let lawyer = usize::try_from(team.get::<i8>(LAWYER_AT)).unwrap_or(0).min(LAWYERS - 1);
        let quality = records
            .block(QUALITY_BLOCK)
            .and_then(|b| b.get(QUALITY_AT + 2 * lawyer..QUALITY_AT + 2 * lawyer + 2))
            .map_or(lawyer as i32, |w| i32::from(i16::from_le_bytes([w[0], w[1]])));
        Some(Club {
            key,
            lawyer,
            quality,
            standing: standing(team.association()),
            played: records.globals.played(i16::from(key.0)),
            shown: team.humans()[1] != 0,
        })
    }

    /// An appeal upheld in the league or on doping (`FUN_006295da`): a
    /// roll of 100 below 23 and twice the lawyer's quality and the
    /// association's word.
    fn upholds(&self, random: &mut Random) -> bool {
        let roll = random.below(100) as i32;
        roll < self.quality * 2 + 23 + self.standing * 2
    }

    /// One in the European cups (`FUN_00629628`): the lawyer's alone.
    fn upholds_europe(&self, random: &mut Random) -> bool {
        (random.below(100) as i32) < self.quality * 2 + 23
    }

    /// What the court adds to a red card's matches (`FUN_00629279`), by
    /// the mean of the lawyer's quality and the association's word: at
    /// the worst one match even odds — and of that even odds again — at
    /// the next one match even odds; at 3 and 4 one off, even odds and
    /// three to one, for a ban standing.
    fn red_card(&self, ban: u8, random: &mut Random) -> i32 {
        match (self.quality + self.standing) / 2 {
            0 if random.below(100) < 50 => random.below(2) as i32,
            1 if random.below(100) < 50 => 1,
            3 if ban != 0 && random.below(100) < 50 => -1,
            4 if ban != 0 && random.below(100) < 75 => -1,
            _ => 0,
        }
    }

    /// What the court takes off a doping ban's days (`FUN_00629435`), in
    /// a country played only: with the lawyer at 0 three in four times,
    /// at 1 even odds, ninety days — 360 from a year on; at 3 even odds,
    /// at 4 three in four, as much added — 360 over a year, ninety over
    /// ninety days. The original's sign gives the worst lawyers the
    /// shorter bans.
    fn doping(&self, days: i32, random: &mut Random) -> i32 {
        if !self.played {
            return 0;
        }
        let off = if days < 360 { 90 } else { 360 };
        let on = if days > 360 {
            -360
        } else if days > 90 {
            -90
        } else {
            0
        };
        match self.quality {
            0 if random.below(100) < 75 => off,
            1 if random.below(100) < 50 => off,
            3 if random.below(100) < 50 => on,
            4 if random.below(100) < 75 => on,
            _ => 0,
        }
    }

    /// Whether the lawyers advise to appeal (`FUN_00629566`): the worst
    /// at even odds, the next two times in three, the others when the
    /// court added to the ban.
    fn advises(&self, added: i32, random: &mut Random) -> bool {
        match self.quality {
            0 => random.below(2) != 0,
            1 => random.below(3) != 0,
            2..=4 => added > 0,
            _ => false,
        }
    }

    /// The hearing's tariff into the club's lawyer costs.
    fn bill(&self, records: &mut Records, doping: bool) {
        let (red, other) = TARIFFS[self.lawyer];
        let fee = if doping { other } else { red } * TARIFF_UNIT;
        if let Some(team) = records.team_mut(u16::from(self.key.0), self.key.1 as u16) {
            team.put(COSTS_AT, team.get::<i32>(COSTS_AT).wrapping_add(fee));
        }
    }
}

/// The manager's popularity with the public one down after an appeal in
/// the league or on doping (`FUN_00418394(-1, 0, 0)`): the club's human
/// manager (+0x8b); a computer club's staff person (+0x7e, else +0x82).
fn public_down(records: &mut Records, club: (u8, i16)) {
    let Some(team) = records.team(u16::from(club.0), club.1 as u16) else { return };
    let manager = team.humans()[1];
    let staff = team.staff();
    if manager != 0 {
        if let Some(m) = usize::from(manager).checked_sub(1).and_then(|i| records.managers.get_mut(i)) {
            let [public, fans, media] = m.popularity();
            m.set_popularity([public.saturating_sub(1), fans, media]);
        }
        return;
    }
    let person = if staff[0] != 0 { staff[0] } else { staff[2] };
    if (1..=1499).contains(&person)
        && let Some(p) = records.persons.get_mut(usize::from(person))
    {
        p.put::<u8>(0x4a, p.get::<u8>(0x4a).saturating_sub(1));
    }
}

/// The groups of cases a hearing walks, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Group {
    LeagueAppeal,
    LeagueVerdict,
    DopingAppeal,
    DopingVerdict,
    EuropeAppeal,
    EuropeVerdict,
}

const LEAGUE_GROUPS: [Group; 4] =
    [Group::LeagueAppeal, Group::LeagueVerdict, Group::DopingAppeal, Group::DopingVerdict];
const EUROPE_GROUPS: [Group; 2] = [Group::EuropeAppeal, Group::EuropeVerdict];

/// One sitting of the court for a club (`FUN_00628309`): the planner's
/// +4 picks the groups; each group takes the squad's first man it holds
/// until none is left, every case cleared as it is heard.
pub struct Hearing {
    club: (u8, i16),
    groups: &'static [Group],
    at: usize,
}

impl Hearing {
    pub fn new(club: (u8, i16), kind: u16) -> Self {
        let groups: &'static [Group] = match kind {
            1 | 3 | 5 | 6 => &LEAGUE_GROUPS,
            2 | 4 => &EUROPE_GROUPS,
            _ => &[],
        };
        Hearing { club, groups, at: 0 }
    }

    /// The next case told, its man's bans and the club's costs already
    /// changed; `None` once the sitting is over.
    pub fn next(&mut self, records: &mut Records, random: &mut Random) -> Option<Case> {
        let club = Club::of(records, self.club)?;
        while let Some(&group) = self.groups.get(self.at) {
            let pick = match group {
                Group::LeagueAppeal => league_appeal,
                Group::LeagueVerdict => league_verdict,
                Group::DopingAppeal => doping_appeal,
                Group::DopingVerdict => doping_verdict,
                Group::EuropeAppeal => europe_appeal,
                Group::EuropeVerdict => europe_verdict,
            };
            let Some(id) = first(records, self.club, pick) else {
                self.at += 1;
                continue;
            };
            let Some(mut l) = Live::of(records, id) else {
                self.at += 1;
                continue;
            };
            if let Some(case) = hear(group, &club, records, id, &mut l, random) {
                return Some(case);
            }
        }
        None
    }
}

/// One man's case of a group: his bans changed and stored, the costs
/// booked; what is told, if anything.
fn hear(group: Group, club: &Club, records: &mut Records, id: u16, l: &mut Live, random: &mut Random) -> Option<Case> {
    let case = |told| Case::new(club.key, id, told, club.shown);
    match group {
        Group::LeagueAppeal => {
            l.league &= MATCHES;
            let mut told = None;
            if l.league != 0 {
                let cut = if club.upholds(random) { matches_off(l.league, false) } else { 0 };
                l.league -= cut;
                l.ends = l.ends.wrapping_sub(i16::from(cut) * 7);
                if l.league == 0 {
                    l.league_kind = 0;
                    l.ends = 0;
                }
                told = Some(case(outcome(i32::from(cut), i32::from(l.league), i32::from(l.league_kind & MATCHES))));
                club.bill(records, false);
            }
            l.store(records, id);
            public_down(records, club.key);
            told
        }
        Group::LeagueVerdict => {
            let added = club.red_card(l.league, random);
            l.league = (l.league & MATCHES).wrapping_add(added as u8);
            l.store(records, id);
            let ban = i32::from(l.league & MATCHES);
            let mut told = case(verdict(ban, i32::from(l.league_kind & MATCHES)));
            if l.league >= 2 {
                told.question = true;
                told.advised = club.advises(added, random);
                told.appeal = Some(Appeal { of: Ban::League, on: true });
            }
            told.billed = Some(false);
            Some(told)
        }
        Group::DopingAppeal => {
            l.long &= !APPEALED;
            let mut cut = 0;
            if club.upholds(random) {
                if l.days >= 361 {
                    cut = l.days - 360;
                } else if l.days > 90 {
                    cut = l.days - 90;
                }
            }
            l.days -= cut;
            if l.days == 0 {
                l.long = 0;
                l.earlier = unmark(l.earlier);
            }
            l.store(records, id);
            let told = case(outcome(cut, l.days, DOPING));
            club.bill(records, true);
            public_down(records, club.key);
            Some(told)
        }
        Group::DopingVerdict => {
            let off = club.doping(l.days, random);
            l.days -= off;
            l.long &= !(PENDING | APPEALED);
            let mut told = if l.days < 1 {
                l.days = 0;
                l.long = 0;
                l.store(records, id);
                case(verdict(0, DOPING))
            } else {
                l.earlier = mark(l.earlier);
                l.store(records, id);
                crate::exhibition::move_standing(records, club.key, [-3, -3, -5], 0);
                // Both of the advice's answers stand at "dazu" here.
                let _ = club.advises(off, random);
                let mut told = case(verdict(l.days, DOPING));
                told.question = true;
                told.advised = true;
                told.appeal = Some(Appeal { of: Ban::Doping, on: true });
                told
            };
            told.billed = Some(true);
            Some(told)
        }
        Group::EuropeAppeal => {
            l.europe &= MATCHES;
            let mut told = None;
            if l.europe != 0 {
                let cut = if club.upholds_europe(random) { matches_off(l.europe, true) } else { 0 };
                l.europe -= cut;
                if l.europe == 0 {
                    l.europe_kind = 0;
                }
                told = Some(case(outcome(i32::from(cut), i32::from(l.europe), i32::from(l.europe_kind & MATCHES))));
                club.bill(records, false);
            }
            l.store(records, id);
            told
        }
        Group::EuropeVerdict => {
            l.europe &= !PENDING;
            let mut told = case(verdict(i32::from(l.europe & MATCHES), i32::from(l.europe_kind & MATCHES)));
            if l.europe != 0 {
                told.question = true;
                told.advised = club.advises(0, random);
                // Advised against, the original files the appeal on "Nein".
                told.appeal = Some(Appeal { of: Ban::Europe, on: told.advised });
            }
            l.store(records, id);
            told.billed = Some(false);
            Some(told)
        }
    }
}

/// A doping verdict confirmed counts among the player's earlier cases
/// (`FUN_0062965d`); a ban lifted on appeal takes it back
/// (`FUN_00629696`).
fn mark(earlier: u8) -> u8 {
    if earlier & ONCE == 0 { earlier | ONCE } else { earlier | TWICE }
}
fn unmark(earlier: u8) -> u8 {
    if earlier & TWICE == 0 { earlier & !ONCE } else { earlier & !TWICE }
}

/// The answer to a case's question (`DAT_00a3b460`: OK yes, Nein no; a
/// computer club takes the lawyers' advice): the appeal it files, for the
/// court's next sitting, on the live object just stored; then a
/// verdict's tariff, which the original books behind the dialog.
pub fn answer(records: &mut Records, case: &Case, yes: bool) {
    if let Some(appeal) = case.appeal.filter(|a| a.on == yes) {
        let at = match appeal.of {
            Ban::League => LEAGUE_BAN,
            Ban::Doping => LONG,
            Ban::Europe => EUROPE_BAN,
        };
        if let Some(r) = records.player_mut(case.player) {
            r.put(at, r.get::<u8>(at) | APPEALED);
        }
    }
    if let (Some(doping), Some(club)) = (case.billed, Club::of(records, case.club)) {
        club.bill(records, doping);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The texts by `FUN_00628fb6` and `FUN_00629106`: a doping ban in
    /// months to 500 days, then years; one match its own text.
    #[test]
    fn the_verdicts_name_months_years_and_matches() {
        assert_eq!(verdict(0, 0), (9, 0));
        assert_eq!(verdict(1, 0), (12, 1));
        assert_eq!(verdict(3, 0x10), (8, 3));
        assert_eq!(verdict(180, DOPING), (7, 6));
        assert_eq!(verdict(720, DOPING), (6, 2));
        assert_eq!(outcome(0, 1, 0), (11, 1));
        assert_eq!(outcome(0, 4, 0), (5, 4));
        assert_eq!(outcome(2, 0, 0), (3, 0));
        assert_eq!(outcome(1, 3, 0), (10, 1));
        assert_eq!(outcome(2, 3, 0), (2, 2));
        assert_eq!(outcome(0, 180, DOPING), (4, 0));
        assert_eq!(outcome(90, 90, DOPING), (1, 3));
        assert_eq!(outcome(360, 1440, DOPING), (0, 4));
        assert_eq!([0, 1, 2, 3, 4, 5, 6, 7, 12, 13].map(|b| matches_off(b, false)), [0, 1, 0, 0, 1, 2, 0, 1, 6, 0]);
        assert_eq!(matches_off(6, true), 0);
        assert_eq!(Case::new((14, 84), 1, (12, 1), true).text(), 2795);
    }

    /// Save A at the week from day 70: a doping ban waiting for its
    /// verdict books the court on the week's second day, free in Union's
    /// planner, to hear the league's and doping cases; a European cup's
    /// ban pending too takes the same day over. The sitting tells the
    /// doping verdict in months with the question, the lawyers for it;
    /// the answer yes files the appeal, the other's tariff of R4 booked.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_doping_ban_books_the_court_and_is_heard() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let mut records = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let club = (14u8, 84i16);
        let squad = records.team(14, 84).unwrap().players();
        let (leite, doped) = (3261, squad.iter().copied().find(|&id| id != 3261).unwrap());
        records.player_mut(leite).unwrap().put::<u8>(LEAGUE_BAN, 0);
        records.globals.set_week_start(70);
        records.globals.set_day(70);
        let to = crate::medical::date_in(&records.globals, 180);
        let r = records.player_mut(doped).unwrap();
        r.set_long_to(to);
        r.set_long_unknown(PENDING);
        assert_eq!(pending(&records, club), (true, false));
        book_week(&mut records);
        assert_eq!(records.planners[0].day(71), Appointment { kind: COURT, flag: 1, player: 1, club: 0 });
        records.player_mut(leite).unwrap().put::<u8>(EUROPE_BAN, PENDING | 2);
        records.planners[0].book(71, Appointment::default());
        book_week(&mut records);
        assert_eq!(records.planners[0].day(71).player, EUROPE_HEARING, "the second over the first");
        let costs = records.team(14, 84).unwrap().get::<i32>(COSTS_AT);
        let mut random = Random::new(1);
        let mut hearing = Hearing::new(club, LEAGUE_HEARING);
        let case = hearing.next(&mut records, &mut random).unwrap();
        assert_eq!((case.player, case.question, case.advised, case.shown), (doped, true, true, true));
        assert!([7, 6].contains(&case.verdict), "{case:?}");
        assert_eq!(records.player(doped).unwrap().long_unknown(), 0);
        answer(&mut records, &case, true);
        assert_eq!(records.player(doped).unwrap().long_unknown(), APPEALED);
        assert_eq!(records.team(14, 84).unwrap().get::<i32>(COSTS_AT), costs + 120_000);
        assert_eq!(hearing.next(&mut records, &mut random), None);
    }
}
