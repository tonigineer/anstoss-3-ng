//! The transfers agreed for the season's end carried out (`FUN_0070f7cf`,
//! in the season-end chain `FUN_006b3ee0` between the clubs' contract pass
//! and their players' close).
//!
//! The deals stand in the agreed list (`0x1be0b40`, 0x28 bytes each: the
//! player, the selling and the buying club, the fee, the 0x14 bytes of the
//! contract in the player object's layout, a loan's flag). A first pass
//! takes every dealt player from the club that sells him or lends him
//! (`FUN_0070e3f5`): out of its squad and line-up, his career's station
//! closed today, his season's counters and matchdays cleared, the club's
//! will to win moved by what he was to it (`FUN_0071473c`), the fee booked
//! as its sale; a sale to a league below what his contract holds him for is
//! called off (`FUN_00427183`). A second pass brings him to the buyer
//! (`FUN_0070e4c1`) while it has fewer than 35: into its squad by his
//! position, his club's bytes, a new station, the fee booked as its
//! purchase, its will to win moved by him (`FUN_00714688`, twice as the
//! original does); the deal's contract becomes his — a sale's years one
//! more, counted from the closing season, its fee in thousands and his
//! seasons at nought —, his next club cleared, his strength stepping by
//! chance (`FUN_0070e588`) and his old ties at the old club ended
//! (`FUN_0070c99e`). Each human club keeps the deal in its history, the
//! seller as sold, the buyer as come (`FUN_0070ebfb`). The list then stands
//! empty (`FUN_0070f564`).
//!
//! A player whose sale is called off with his contract running out goes
//! to the free agents (`FUN_0040e8cc`, [`execute_with`]).
//!
//! Left out: the human clubs' training plans of a player between them (`FUN_00746ddf`),
//! the news (`FUN_0070eccd`), the record and the all-time lists
//! (`FUN_005dbeb7`, `FUN_005dbefc`, `FUN_006d5931`) and the share market's
//! note (`FUN_00405bf3`).

use chrono::NaiveDate;

use crate::finances::CLUB_TO_TEAM;
use crate::random::Random;
use crate::records::{Player, Records, Team};

const AGREED: &str = "0x1be0b40";
const PAGE: usize = 4000;
const PAGE_ENTRIES: usize = 100;
const PAGE_HEAD: usize = 4;
const ENTRY: usize = 0x28;
/// A deal's fields.
const PLAYER: usize = 2;
const SELLER: usize = 4;
const BUYER: usize = 8;
const FEE: usize = 0xc;
const TERMS: usize = 0x10;
const LOAN: usize = 0x25;
/// The club's books of transfers (club offsets): what it sold, what it
/// bought, the net.
const SALES: usize = 0x470;
const PURCHASES: usize = 0x478;
const NET: usize = 0x258;
/// The retiring byte's bit: he ends his career.
const RETIRING: u8 = 0x80;
/// A club takes no more than this many.
const SQUAD_MOST: usize = 35;
/// Fees count in thousands where they are kept.
const THOUSAND: u32 = 1000;
/// R+0x282's bit for a player counted as home-grown.
const HOME_GROWN: u8 = 0x10;
/// The career's stations.
const STATIONS: usize = 0x20;
const STATION: usize = 0x14;
const STATIONS_MOST: i16 = 15;
/// The history's kinds.
const CAME: u8 = 1;
const SOLD: u8 = 2;

/// A deal as the list keeps it.
#[derive(Clone, Copy)]
struct Deal {
    bytes: [u8; ENTRY],
    open: bool,
}

impl Deal {
    fn word(&self, at: usize) -> i16 {
        i16::from_le_bytes([self.bytes[at], self.bytes[at + 1]])
    }

    fn player(&self) -> u16 {
        self.word(PLAYER) as u16
    }

    fn seller(&self) -> (u8, i16) {
        (self.bytes[SELLER], self.word(SELLER + 2))
    }

    fn buyer(&self) -> (u8, i16) {
        (self.bytes[BUYER], self.word(BUYER + 2))
    }

    fn fee(&self) -> u32 {
        u32::from_le_bytes(self.bytes[FEE..FEE + 4].try_into().unwrap())
    }

    fn loan(&self) -> bool {
        self.bytes[LOAN] != 0
    }
}

fn read(records: &Records) -> Vec<Deal> {
    let Some(block) = records.block(AGREED) else { return Vec::new() };
    let count = usize::from(i16::from_le_bytes([block[0], block[1]]) as u16);
    (0..count)
        .filter_map(|k| {
            let at = 4 + k / PAGE_ENTRIES * PAGE + PAGE_HEAD + k % PAGE_ENTRIES * ENTRY;
            let bytes: [u8; ENTRY] = block.get(at..at + ENTRY)?.try_into().ok()?;
            Some(Deal { bytes, open: i16::from_le_bytes([bytes[0], bytes[1]]) != 0 })
        })
        .collect()
}

fn add_money(team: &mut Team, at: usize, by: i64) {
    let at = at + CLUB_TO_TEAM;
    let v = team.get::<i64>(at).wrapping_add(by);
    team.put::<i64>(at, v);
}

fn add_net(team: &mut Team, by: i32) {
    let at = NET + CLUB_TO_TEAM;
    let v = team.get::<i32>(at).wrapping_add(by);
    team.put::<i32>(at, v);
}

/// The will to win a club takes (`FUN_0069efe8`), within ±200.
fn will(team: &mut Team, by: i16) {
    team.set_will_to_win(team.will_to_win().saturating_add(by).clamp(-200, 200));
}

fn today(records: &Records) -> Option<NaiveDate> {
    let [day, month, year] = records.globals.date();
    NaiveDate::from_ymd_opt(i32::from(year), u32::from(month), u32::from(day))
}

/// The selling club's will to win at losing him (`FUN_0071473c`).
fn will_at_leaving(team: &mut Team, p: &Player, age: i32) {
    let [mark, more] = p.traits();
    if more & 0x60 != 0 || mark & 4 != 0 {
        will(team, 5);
    }
    if more & 1 != 0 || mark & 1 != 0 || (age < 23 && p.strength() > 7) {
        will(team, -10);
    }
}

/// The buying club's will to win at his coming (`FUN_00714688`).
fn will_at_coming(team: &mut Team, p: &Player, age: i32) {
    let [mark, more] = p.traits();
    if more & 1 != 0 {
        will(team, 2);
    }
    if mark & 1 != 0 || (age < 23 && p.strength() > 7) {
        will(team, 2);
    } else if more & 0x260 != 0 || mark & 0x804 != 0 {
        will(team, -2);
    } else if team.will() < 10 {
        will(team, 10);
    }
}

/// The level a club plays next season (`FUN_00692bf9` over the drawn
/// lists), as a class of the leagues (`FUN_006755b8`); 10 none.
pub(crate) fn next_class(records: &Records, country: u8, team: i16) -> i16 {
    let main = records.globals.main_country();
    let next = crate::promotion::next_level(records, u16::from(country), team);
    if next >= 10 {
        return next;
    }
    if i16::from(country) == main {
        if country == 0x10 { next.min(3) } else { next.min(2) }
    } else if next > 0 {
        0
    } else {
        next
    }
}

/// Whether he plays for the buyer: the deal's contract holds him for no
/// league below its clause (`FUN_00427183` with `FUN_006fe85e` on the
/// deal's terms: their word's low two bits).
fn accepts(records: &Records, deal: &Deal) -> bool {
    let clause = deal.word(TERMS + 0xe) & 3;
    let buyer = deal.buyer();
    clause == 0 || next_class(records, buyer.0, buyer.1) < clause
}

/// Today into a station's date.
fn date_bytes(records: &Records) -> [u8; 4] {
    let [day, month, year] = records.globals.date();
    let y = year.to_le_bytes();
    [day as u8, month as u8, y[0], y[1]]
}

/// His last station ends today (`FUN_0070429c`).
pub(crate) fn close_station(records: &mut Records, id: u16) {
    let date = date_bytes(records);
    let Some(p) = records.player_mut(id) else { return };
    let stations = p.career_count();
    if stations > 0 {
        let at = STATIONS + usize::try_from(stations - 1).unwrap_or(0) * STATION + 4;
        for (k, b) in date.iter().enumerate() {
            p.put::<u8>(at + k, *b);
        }
    }
}

/// His season left behind at the club (`FUN_00707bbf`: the station's end
/// date, `FUN_0070429c`; the counters and matchdays, `FUN_00706821`).
fn leave_season(records: &mut Records, id: u16) {
    close_station(records, id);
    let Some(p) = records.player_mut(id) else { return };
    for at in [0x244, 0x243, 0x242, 0x241, 0x248, 0x247, 0x245, 0x255, 0x254, 0x253, 0x25e, 0x352] {
        p.put::<u8>(at, 0);
    }
    p.set_matchday_goals([0; 46]);
    p.set_matchdays([0; 46]);
}

/// A new station at the club (`FUN_00704109`): today, no end, the fee in
/// thousands, the club, his strength; fifteen kept, the oldest gone.
pub(crate) fn open_station(records: &mut Records, id: u16, club: (u8, i16), fee: u32) {
    let date = date_bytes(records);
    let Some(p) = records.player_mut(id) else { return };
    let mut count = p.career_count().clamp(0, STATIONS_MOST);
    if count == STATIONS_MOST {
        for k in 0..usize::from(STATIONS_MOST as u16) - 1 {
            for b in 0..STATION {
                let v = p.get::<u8>(STATIONS + (k + 1) * STATION + b);
                p.put::<u8>(STATIONS + k * STATION + b, v);
            }
        }
        count -= 1;
    }
    let at = STATIONS + usize::try_from(count).unwrap_or(0) * STATION;
    let mut s = [0u8; STATION];
    s[0..4].copy_from_slice(&date);
    s[8..12].copy_from_slice(&(fee / THOUSAND).to_le_bytes());
    s[0x10] = club.0;
    s[0x11] = club.1 as u8;
    s[0x13] = p.strength();
    for (k, b) in s.iter().enumerate() {
        p.put::<u8>(at + k, *b);
    }
    p.set_career_count(count + 1);
}

/// A human club's history of a deal (`FUN_0070ebfb`, `FUN_0070eb02`): the
/// year, week 0xff, the kind, his names, the other club, his strength and
/// position, the fee in thousands.
fn note(records: &mut Records, club: (u8, i16), id: u16, kind: u8, other: [u8; 4], fee: u32) {
    let Some(slot) = u16::try_from(club.1)
        .ok()
        .and_then(|n| records.team(u16::from(club.0), n))
        .and_then(|t| usize::from(t.human()).checked_sub(1))
    else {
        return;
    };
    let Some(p) = records.player(id) else { return };
    let mut e = [0u8; 0x14];
    e[0..2].copy_from_slice(&records.globals.start_year().to_le_bytes());
    e[2] = 0xff;
    e[3] = kind;
    e[4..6].copy_from_slice(&p.first_name().to_le_bytes());
    e[6..8].copy_from_slice(&p.last_name().to_le_bytes());
    e[8..12].copy_from_slice(&other);
    e[0xc] = p.strength();
    e[0xd] = p.positions()[0];
    e[0x10..0x14].copy_from_slice(&(fee / THOUSAND).to_le_bytes());
    if let Some(extra) = records.extras.get_mut(slot) {
        crate::rollover::push_history(extra, e);
    }
}

/// Into the buyer's squad by his main position, after those who play it
/// or before (`FUN_0070df0b`), his club's bytes the buyer's.
fn join_squad(team: &mut Team, records_positions: &dyn Fn(u16) -> u8, id: u16, position: u8) {
    let mut squad = team.players();
    let mut at = squad.len();
    while at > 0 && position < records_positions(squad[at - 1]) {
        at -= 1;
    }
    squad.insert(at, id);
    team.set_players(&squad);
}

/// His strength's step at the new club (`FUN_0070e588`): one in a thousand
/// two up, else one in fifteen one up, else one in fifteen one down (above
/// 1), else one in a thousand two down (above 2); within his talent, a 13
/// keeps 13.
fn step_strength(p: &mut Player, random: &mut Random) {
    let old = p.strength();
    let _ = random.below(100);
    let mut s = old;
    if random.below(1000) == 0 {
        s += 2;
    } else if random.below(15) == 0 {
        s += 1;
    } else if random.below(15) == 0 {
        if s > 1 {
            s -= 1;
        }
    } else if random.below(1000) == 0 && s > 2 {
        s -= 2;
    }
    if s == 0 {
        s = 1;
    }
    let top = crate::rollover::top_strength(p.talent());
    if s < 13 || old != 13 {
        if top <= s {
            s = top;
        }
    } else {
        s = 13;
    }
    p.set_strength(s);
}

/// His old ties ended at the new club (`FUN_0070c99e`): the list bits,
/// his absence's mark, suspension, reasons, R+0x1f4, the fines, the sick
/// visits, the ban's weeks, R+0x27a; his contentment with the trainer, the
/// team and the club at 100; the problems and several bits of R+0x1b4,
/// R+0x27f, R+0x280 and R+0x297 cleared.
pub(crate) fn new_ties(p: &mut Player, absent: bool) {
    p.set_retiring(p.retiring() & 0x3a);
    if !absent {
        p.set_away_kind(0);
    } else {
        p.set_away_kind(p.away_kind() & 0xef);
    }
    p.set_suspension(0);
    p.set_flags(0);
    p.put::<u16>(0x1f4, 0);
    p.set_fined(0);
    p.set_visits(0);
    p.set_banned_weeks(0);
    p.put::<u8>(0x27a, 0);
    let mut parts = p.contentment();
    parts[0..3].copy_from_slice(&[100; 3]);
    p.set_contentment(parts);
    p.set_morale(crate::mood::morale(p));
    p.set_problems(p.problems() & 0xfc);
    p.set_shown(p.shown() & !(0x4000_0000 | 0x8000 | 0x1_0000 | 0x2_0000 | 0x4_0000));
    p.put::<u8>(0x280, p.get::<u8>(0x280) & 0xec & 0xef);
    p.set_offences(0);
    p.set_problems(p.problems() & 0xf3);
    p.set_morale(crate::mood::morale(p));
}

/// A club lets a player go (`FUN_0070e3f5`): its will to win moved by
/// him, his station closed and his season left behind, the fee into its
/// sales and its net, out of its squad, line-up and bench; to a club of
/// another country no more home-grown.
pub(crate) fn let_go(records: &mut Records, club: (u8, i16), id: u16, fee: u32, buyer: Option<u8>) {
    let Ok(number) = u16::try_from(club.1) else { return };
    let age = today(records).and_then(|d| records.player(id).map(|p| crate::statistics::age_on(p, d))).unwrap_or(0);
    let p = records.player(id).cloned();
    if let (Some(team), Some(p)) = (records.team_mut(u16::from(club.0), number), p.as_ref()) {
        will_at_leaving(team, p, age);
    }
    leave_season(records, id);
    if let Some(team) = records.team_mut(u16::from(club.0), number) {
        add_money(team, SALES, i64::from(fee));
        add_net(team, fee as i32);
        let squad: Vec<u16> = team.players().into_iter().filter(|&p| p != id).collect();
        team.set_players(&squad);
        let mut lineup = team.lineup();
        lineup.iter_mut().filter(|s| **s == id).for_each(|s| *s = 0);
        team.set_lineup(lineup);
        let mut bench = team.bench();
        bench.iter_mut().filter(|s| **s == id).for_each(|s| *s = 0);
        team.set_bench(bench);
    }
    if let Some(country) = buyer.filter(|&c| c != 0 && c != club.0) {
        let _ = country;
        if let Some(p) = records.player_mut(id) {
            p.set_manner(p.manner() & !HOME_GROWN);
        }
    }
}

/// A club takes a player in (`FUN_0070e4c1`): its will to win moved by
/// him, off its watch list, the fee out of its net into its purchases,
/// into its squad by his position with its bytes, a new station.
pub(crate) fn bring_in(records: &mut Records, club: (u8, i16), id: u16, fee: u32) {
    let Ok(number) = u16::try_from(club.1) else { return };
    let age = today(records).and_then(|d| records.player(id).map(|p| crate::statistics::age_on(p, d))).unwrap_or(0);
    let Some(p) = records.player(id).cloned() else { return };
    let position = p.positions()[0];
    let positions: Vec<(u16, u8)> = records
        .team(u16::from(club.0), number)
        .map(|t| t.players().into_iter().map(|q| (q, records.player(q).map_or(0, |r| r.positions()[0]))).collect())
        .unwrap_or_default();
    let position_of = |q: u16| positions.iter().find(|e| e.0 == q).map_or(0, |e| e.1);
    if let Some(team) = records.team_mut(u16::from(club.0), number) {
        will_at_coming(team, &p, age);
        add_net(team, -(fee as i32));
        add_money(team, PURCHASES, i64::from(fee));
        join_squad(team, &position_of, id, position);
    }
    crate::transfers::unwatch_at_club(records, club, id);
    if let Some(p) = records.player_mut(id) {
        p.set_club([club.0, club.1 as u8]);
    }
    open_station(records, id, club, fee);
}

/// Carries out the season's agreed transfers (`FUN_0070f7cf`).
pub fn execute(records: &mut Records, random: &mut Random) {
    let mut agents = Vec::new();
    execute_with(records, random, &mut agents);
}

/// The season's agreed transfers carried out, a player whose sale is
/// called off and whose contract no longer holds him at his club let go
/// onto the free agents' list `agents` — or ending his career when he
/// retires or the list is full (`FUN_004271fb`, `FUN_0040e8cc`,
/// `FUN_0070e29e`), his human club noting him gone to no club.
pub fn execute_with(records: &mut Records, random: &mut Random, agents: &mut Vec<crate::free_agents::FreeAgent>) {
    let mut deals = read(records);
    let year = i32::from(records.globals.start_year());
    let age_of = |records: &Records, id: u16| -> i32 {
        today(records).and_then(|d| records.player(id).map(|p| crate::statistics::age_on(p, d))).unwrap_or(0)
    };
    // The sellers let them go.
    for deal in deals.iter_mut().filter(|d| d.open && d.seller().0 != 0) {
        let (id, seller, buyer) = (deal.player(), deal.seller(), deal.buyer());
        let Ok(number) = u16::try_from(seller.1) else { deal.open = false; continue };
        let in_squad = records.team(u16::from(seller.0), number).is_some_and(|t| t.players().contains(&id));
        if !in_squad {
            deal.open = false;
            continue;
        }
        if !deal.loan() && !accepts(records, deal) {
            deal.open = false;
            let class = next_class(records, seller.0, seller.1);
            let holds = records.player(id).is_some_and(|p| crate::free_agents::holds(records, p, class));
            if !holds {
                let_go(records, seller, id, 0, Some(buyer.0));
                let retires = records.player(id).is_some_and(|p| p.retiring() & RETIRING != 0);
                if !retires && crate::free_agents::takes(agents) {
                    note(records, seller, id, SOLD, [0; 4], 0);
                    agents.push(crate::free_agents::FreeAgent { id, country: seller.0, team: seller.1 });
                } else {
                    crate::rollover::retire(records, u16::from(seller.0), number, id, random);
                }
            }
            continue;
        }
        let fee = deal.fee();
        let_go(records, seller, id, fee, Some(buyer.0));
        note(records, seller, id, SOLD, deal.bytes[BUYER..BUYER + 4].try_into().unwrap(), fee);
    }
    // The buyers take them.
    for deal in deals.iter_mut() {
        if !deal.open {
            continue;
        }
        let (id, buyer) = (deal.player(), deal.buyer());
        let Ok(number) = u16::try_from(buyer.1) else { continue };
        let Some(size) = records.team(u16::from(buyer.0), number).map(|t| t.players().len()) else { continue };
        if size >= SQUAD_MOST || !accepts(records, deal) {
            continue;
        }
        let (fee, loan) = (deal.fee(), deal.loan());
        let age = age_of(records, id);
        let Some(p) = records.player(id).cloned() else { continue };
        let position = p.positions()[0];
        let positions: Vec<(u16, u8)> = records
            .team(u16::from(buyer.0), number)
            .map(|t| t.players().into_iter().map(|q| (q, records.player(q).map_or(0, |r| r.positions()[0]))).collect())
            .unwrap_or_default();
        let position_of = |q: u16| positions.iter().find(|e| e.0 == q).map_or(0, |e| e.1);
        let _ = (&p, position, &position_of);
        bring_in(records, buyer, id, fee);
        let absent = records.player(id).is_some_and(|p| {
            let [y, d] = p.away_to();
            let (now, day) = (records.globals.date()[2], records.globals.day_of_year());
            y > now || (y == now && d > day)
        });
        let t = &deal.bytes[TERMS..TERMS + 0x14];
        let word = |at: usize| u16::from_le_bytes([t[at], t[at + 1]]);
        if let Some(p) = records.player_mut(id) {
            p.set_salary(word(0));
            p.set_appearance_bonus(word(2));
            p.set_goal_bonus(word(4));
            p.set_fixed_fee(word(6));
            p.put::<u32>(0x158, u32::from_le_bytes(t[8..12].try_into().unwrap()));
            p.set_terms(word(0xe));
            p.put::<u8>(0x161, t[0x10]);
            p.put::<u8>(0x160, t[0x11]);
            let mut years = t[0xc] as i8;
            if !loan {
                years = years.wrapping_add(1);
                p.put::<u32>(0x158, fee / THOUSAND);
                p.put::<u8>(0x161, 0);
            }
            p.set_contract((i32::from(years) + year) as u16);
            p.put::<u8>(0x14c, 0);
            p.put::<u16>(0x14e, 0);
            step_strength(p, random);
            new_ties(p, absent);
        }
        let now = records.player(id).cloned();
        if let (Some(team), Some(p)) = (records.team_mut(u16::from(buyer.0), number), now.as_ref()) {
            will_at_coming(team, p, age);
        }
        crate::transfers::unwatch_at_club(records, buyer, id);
        let seller = if deal.seller().0 == 0 {
            let nation = records.player(id).map_or(0, |p| p.club()[0]);
            let mut s = [0u8; 4];
            s[0] = nation;
            s[2..4].copy_from_slice(&200i16.to_le_bytes());
            s
        } else {
            deal.bytes[SELLER..SELLER + 4].try_into().unwrap()
        };
        note(records, buyer, id, CAME, seller, fee);
    }
    // The list stands empty (`FUN_0070f564`).
    if let Some(block) = records.block_mut(AGREED) {
        block[0..2].copy_from_slice(&0i16.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    /// Over the original's season end (SeasonEnd364): Rönnow's sale to a
    /// club whose league his contract does not hold is called off, and
    /// with his contract not holding him at Union either he goes to the
    /// free agents, Union's history noting him gone to no club, as the
    /// original's NewSeason000 has him.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_called_off_sale_lets_the_player_go_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs) = (load("SeasonEnd364.an3"), load("NewSeason000.an3"));
        let mut random = Random::new(3);
        let mut agents = crate::free_agents::release(&mut ours, &mut random);
        const RONNOW: u16 = 3257;
        assert!(ours.team(14, 84).unwrap().players().contains(&RONNOW), "the release keeps him");
        execute_with(&mut ours, &mut random, &mut agents);
        assert!(!ours.team(14, 84).unwrap().players().contains(&RONNOW));
        assert!(!theirs.team(14, 84).unwrap().players().contains(&RONNOW));
        assert!(agents.iter().any(|a| a.id == RONNOW && (a.country, a.team) == (14, 84)));
        let entry = |r: &Records| -> Option<[u8; 0x14]> {
            let e = r.extras.get(0)?;
            (0..e.get::<i16>(0xdde).max(0) as usize).map(|k| e.get::<[u8; 0x14]>(0xde0 + k * 0x14)).find(|a| {
                r.player(RONNOW).is_some_and(|p| u16::from_le_bytes([a[4], a[5]]) == p.first_name()) && a[2] == 0xff
            })
        };
        // The other club's second byte the original leaves as its stack
        // held it (`local_1c`'s unset byte).
        let clean = |e: Option<[u8; 0x14]>| e.map(|mut a| { a[9] = 0; a });
        assert_eq!(clean(entry(&ours)), clean(entry(&theirs)));
    }

    use super::*;

    /// The season's deals over the original's pair: SeasonEnd364 run
    /// through the season end's steps before them and its deals carried
    /// out by the replica, every deal the original carried out has its
    /// player in the buyer's squad, with the buyer's bytes, the deal's
    /// contract and years, the next club cleared, a new station at the
    /// buyer and his season's matchdays gone, as NewSeason000 keeps him;
    /// the one sale both call off and the original's free agents, placed
    /// behind it ([`crate::free_agents`]), aside.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_season_end_carries_out_its_deals_as_the_original() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs, before) =
            (load("SeasonEnd364.an3"), load("NewSeason000.an3"), load("SeasonEnd364.an3"));
        let mut random = Random::new(3);
        let _ = crate::promotion::draw(&mut ours, &mut random);
        crate::promotion::count(&mut ours);
        crate::rollover::retire_announced(&mut ours, &mut random);
        crate::rollover::count_first_seasons(&mut ours);
        execute(&mut ours, &mut random);
        assert!(read(&ours).is_empty(), "the list stands empty");
        let in_squad = |r: &Records, club: (u8, i16), id: u16| {
            r.team(u16::from(club.0), club.1 as u16).is_some_and(|t| t.players().contains(&id))
        };
        let mut done = 0;
        for d in read(&before).iter().filter(|d| d.open) {
            let (id, buyer) = (d.player(), d.buyer());
            let (o, x, y) = (before.player(id).unwrap(), ours.player(id).unwrap(), theirs.player(id).unwrap());
            if o.birthday() != y.birthday() || !in_squad(&theirs, buyer, id) {
                continue;
            }
            done += 1;
            assert!(in_squad(&ours, buyer, id), "player {id} at {buyer:?}");
            for at in (0x14c..0x15e).chain([0x160, 0x220, 0x221, 0x212]) {
                assert_eq!(x.get::<u8>(at), y.get::<u8>(at), "player {id} at {at:#x}");
            }
            let station = STATIONS + usize::try_from(x.career_count() - 1).unwrap() * STATION;
            for at in station..station + 0x13 {
                assert_eq!(x.get::<u8>(at), y.get::<u8>(at), "player {id}'s station at {at:#x}");
            }
            assert_eq!(x.matchdays(), y.matchdays(), "player {id}'s matchdays");
        }
        assert!(done > 260, "{done} deals as the original's");
    }
}
