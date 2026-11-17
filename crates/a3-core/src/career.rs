//! The human manager's own honours (`FUN_006b5497` and `FUN_006b583a`,
//! behind [`crate::honours::honour`]'s human branch): a club with a human
//! manager (club +0x8b, the team's second [`crate::records::Team::humans`]
//! byte) credits
//! no person at all but his own career record, the block
//! `0xa4de58[<slot>]` the personal page at dialog 405 shows
//! (ERFOLGE.md §A.1). A kind the game marks ([`crate::honours::marked`],
//! `DAT_008a56b8`) goes into the record's window of the last fifty
//! titles — the kind, the season (the game's own count,
//! [`crate::records::Globals::seasons`], 1 at the start), the club's
//! country and its
//! number there — the oldest of the fifty giving way as the fiftieth
//! comes in, and one more on the mark of the manager's current station:
//! championships at +0x18, the European cups at +0x19, the cups at +0x1a
//! and every other marked kind at +0x1b. A national team's title — kinds
//! 0xb and 0xc, of the international tournaments the replica plays no
//! final of — marks his last national team's station instead, the world's
//! title at +0x1a and the European one at +0x18. Every kind counts in its
//! own counter at the head of the record: the championship at +0, the cup
//! at +2, the cup's finalist at +4, a promotion at +0x16, and so on to
//! the international tournaments' at +0x1c to +0x20. A championship
//! lifts the manager's own count of titles as well (his record's +0x210,
//! the manager object's +0xdc), which the highscore lists add up.
//!
//! What the same two functions touch on the manager *object* and what
//! never reaches the save with it is not kept: the level's marks behind
//! `FUN_006a6805`, `FUN_006a6821`, `FUN_006a683d` and `FUN_006a6859`,
//! the object's own count at +0x17c6, and the club's season object
//! (`FUN_0061436f`, see [`crate::honours`]).

use crate::records::Records;

/// The window's count (`+0x26`) and the count of all marked titles
/// (`+0x28`), both fifteen at the window's fiftieth entry.
const WINDOW_COUNT: usize = 0x26;
const TOTAL: usize = 0x28;
/// The window's entries, eight bytes each — the kind, the season, the
/// country and the number. The first of them stands where the count of
/// them is one, which is why the original writes at `+0x22 + count * 8`.
const ENTRIES: usize = 0x2a;
const ENTRY: usize = 8;
const WINDOW: usize = 50;
/// The stations: their counts, where their rows stand, and what a title
/// adds to the last row's mark.
const CLUB_STATIONS: usize = 0x1c2;
const NATIONAL_STATIONS: usize = 0x1c3;
const CLUBS_AT: usize = 0x1c4;
const NATIONALS_AT: usize = 0x624;
const STATION: usize = 0x1c;
const WON_CHAMPIONSHIPS: usize = 0x18;
const WON_EUROPEAN: usize = 0x19;
const WON_CUPS: usize = 0x1a;
const WON_OTHER: usize = 0x1b;
/// A national team's station: the European title at +0x18, the world's at
/// +0x1a.
const WON_CONTINENTAL: usize = 0x18;
const WON_WORLD: usize = 0x1a;

/// Where a kind of title counts in the career record (`FUN_006b583a`).
fn counter(kind: u8) -> Option<usize> {
    match kind {
        1 => Some(0x0),
        2 => Some(0x16),
        3 => Some(0x18),
        4 => Some(0x2),
        5 => Some(0x6),
        6 => Some(0x8),
        7 | 8 => Some(0xc),
        9 => Some(0x12),
        10 => Some(0x14),
        0xb => Some(0x1e),
        0xc => Some(0x1a),
        0xd => Some(0x4),
        0xe => Some(0xa),
        0xf => Some(0xe),
        0x10 => Some(0x1c),
        0x11 => Some(0x20),
        _ => None,
    }
}

/// The title of `kind` won by the club of `country` and `number` in the
/// career of the human in slot `slot` ([`fun@FUN_006b5497`] writing the
/// record, [`fun@FUN_006b583a`] its counters): a marked kind into the
/// window of titles and onto the current station's mark, and every kind
/// once more in the kind's own counter. A championship counts in the
/// manager record as well.
pub fn title(records: &mut Records, slot: usize, country: u16, number: u16, kind: u8) {
    let season = records.globals.seasons();
    let club = records.team(country, number).map(|t| (t.country(), t.number()));
    let (country, number) = club.unwrap_or((0, 0));
    if let Some(car) = records.career_mut(slot) {
        if crate::honours::marked(kind) {
            bump(car, TOTAL);
            let mut count = word(car, WINDOW_COUNT).wrapping_add(1);
            if count == i16::try_from(WINDOW).unwrap() {
                // The window gives up its oldest entry.
                car.copy_within(ENTRIES + ENTRY..ENTRIES + ENTRY + (WINDOW - 1) * ENTRY, ENTRIES);
                count -= 1;
            }
            store(car, WINDOW_COUNT, count);
            if let Ok(n) = usize::try_from(count) {
                if let Some(e) = car.get_mut(ENTRIES + (n - 1) * ENTRY..ENTRIES + n * ENTRY) {
                    e[0] = kind;
                    e[2..4].copy_from_slice(&season.to_le_bytes());
                    e[4] = country;
                    e[6..8].copy_from_slice(&u16::from(number).to_le_bytes());
                }
            }
            mark_station(car, kind);
        }
        if let Some(at) = counter(kind) {
            bump(car, at);
        }
    }
    if kind == crate::honours::CHAMPIONSHIP
        && let Some(m) = records.managers.get_mut(slot)
    {
        let mut titles = m.titles();
        titles[0] = titles[0].wrapping_add(1);
        m.set_titles(titles);
    }
}

/// The human in `slot` leaves his national team for `reason`
/// (`FUN_006fdf90`): his last national station gets today's date and the
/// reason, its three figures nought. Without a station nothing happens.
pub fn left_national_team(records: &mut Records, slot: usize, reason: u8) {
    let [day, month, year] = records.globals.date();
    let Some(car) = records.career_mut(slot) else { return };
    let Some(last) = usize::from(car[NATIONAL_STATIONS]).checked_sub(1) else { return };
    let row = &mut car[NATIONALS_AT + last * STATION..][..STATION];
    row[8] = day as u8;
    row[9] = month as u8;
    row[0xa..0xc].copy_from_slice(&year.to_le_bytes());
    row[0xc] = reason;
    row[0xe..0x12].fill(0);
    row[0x14..0x18].fill(0);
}

/// The career's counts of endings: by dismissal (reasons 0 and 3, the
/// personal page's "Entlassen") and by stepping down (reason 4, "Selbst
/// gegangen").
const DISMISSED: usize = 0x1ba;
const STEPPED_DOWN: usize = 0x1bc;

/// The human in `slot` leaves his club for `reason` (`FUN_006fddfe`, from
/// `FUN_006da0fe` at a career's end with reason 4). First the season's
/// youth and service marks go into his honour marks (`FUN_006dd39a`:
/// object +0xc8 and +0xcc into +0xd4). Then his last club station gets
/// today's date, the reason, the manager's two running figures (object
/// +0xe0, +0xe2) and the sum of his nine marks (`FUN_006dd9f1`); a
/// dismissal (0 or 3) or a step down (4) counts at the record's head.
/// Without a station the record stays as it is.
pub fn left_club(records: &mut Records, slot: usize, reason: u8) {
    let Some(m) = records.managers.get_mut(slot) else { return };
    m.set_honour_marks(m.honour_marks().wrapping_add(m.youth_marks()).wrapping_add(m.service_marks()));
    m.set_youth_marks(0);
    m.set_service_marks(0);
    let figures = m.station_figures();
    let marks: i32 = {
        let m = &records.managers[slot];
        crate::evaluation::marks(records, m, slot + 1).iter().fold(0i32, |a, &b| a.wrapping_add(b))
    };
    let [day, month, year] = records.globals.date();
    let Some(car) = records.career_mut(slot) else { return };
    let Some(last) = usize::from(car[CLUB_STATIONS]).checked_sub(1) else { return };
    let Some(row) = car.get_mut(CLUBS_AT + last * STATION..CLUBS_AT + (last + 1) * STATION) else { return };
    row[8] = day as u8;
    row[9] = month as u8;
    row[0xa..0xc].copy_from_slice(&year.to_le_bytes());
    row[0xc] = reason;
    row[0xe..0x10].copy_from_slice(&figures[0].to_le_bytes());
    row[0x10..0x12].copy_from_slice(&figures[1].to_le_bytes());
    row[0x14..0x18].copy_from_slice(&marks.to_le_bytes());
    match reason {
        0 | 3 => bump(car, DISMISSED),
        4 => bump(car, STEPPED_DOWN),
        _ => {}
    }
}

/// Whether a club stands in its league's relegation places
/// (`FUN_00677d72`): a club of a followed country (not the list's end,
/// `;`) in one of its leagues, placed below the league's size less its
/// relegations (the league's G+0 and G+0x20).
fn going_down(records: &Records, club: (u8, u16)) -> bool {
    const NO_COUNTRY: u8 = b';';
    let Some(team) = records.team(club.0.into(), club.1) else { return false };
    let (country, level) = (team.country(), team.level());
    if !records.globals.played(country.into()) || country == NO_COUNTRY {
        return false;
    }
    if i16::from(level) >= records.countries.leagues(country.into()) {
        return false;
    }
    let Some(league) = crate::opponent::League::of(records, country.into(), level) else { return false };
    (league.teams() as i16).wrapping_sub(league.relegations()) < i16::from(team.table_place())
}

/// What leaving `club` costs the human in `slot` (`FUN_006d9f70`, from
/// `FUN_006da0fe` after his last station is closed): 40 of his standing
/// marks (the manager object's +0xd8, `FUN_006dd8f3`) when the club stands
/// in the relegation places, else 10, and 8 of his nerves (`FUN_006be82b`,
/// clamped to 0..100 by `FUN_006be903`). `FUN_0070ddbf` then takes a
/// player-manager out of the club's squad and clears his +0x26; the
/// replica keeps no player-manager in a squad, so that part is not built.
pub fn leaving_costs(records: &mut Records, slot: usize, club: (u8, u16)) {
    let by = if going_down(records, club) { 40 } else { 10 };
    let Some(m) = records.managers.get_mut(slot) else { return };
    m.set_standing_marks(m.standing_marks().wrapping_sub(by));
    m.set_nerves((m.nerves() - 8).clamp(0, 100));
}

/// The club and its players let go when the human in `slot` ends his
/// career (`FUN_006d9fa1`, from `FUN_006da0fe` after [`leaving_costs`]):
/// the club's trainer and manager numbers (club +0x8a, +0x8b; team +0xa8)
/// go to nought (`FUN_00409fe7`, `FUN_0040a05d`), and with both gone its
/// extra data's first byte and its human slot (team +0xb5, `FUN_0063a513`
/// → `FUN_0063a548`). Its week's book of abilities ends (`FUN_006436b3`).
/// Each squad player loses his suspension (L+0x66 bit 4 = R+0x26b,
/// `FUN_0074d7e6`), the line-ups' always and never (L+0xb6 = R+0x271 bits
/// 4 and 5, `FUN_00706ddb(0)`, `FUN_00706d66(0)`), R+0x26f bit 6 and the
/// trainer's grievances 0x100, 0x1000 and 0x4000 (`FUN_007717eb`,
/// `FUN_00770942`), his contentment with the trainer raised to a hundred
/// when it stands below (`FUN_007008b9`, `FUN_00700846`). Last, everyone
/// on trial goes back at once (`FUN_0074d27f(club, 1)`).
pub fn release_club(
    records: &mut Records,
    clubs: &mut std::collections::BTreeMap<(usize, usize), crate::season::ClubState>,
    world: &crate::world::World,
    club: (u8, u16),
) {
    const SUSPENDED: u8 = 0x10;
    const PICKS: u8 = 0x30;
    const RETIRING_BIT: u8 = 0x40;
    const GRIEVANCES: u32 = 0x100 | 0x1000 | 0x4000;
    let (country, number) = club;
    let Some(team) = records.team_mut(u16::from(country), number) else { return };
    let human = team.human();
    team.set_humans([0, 0]);
    if human != 0 {
        team.set_human(0);
    }
    let squad = team.players();
    if let Some(extra) = usize::from(human).checked_sub(1).and_then(|i| records.extras.get_mut(i)) {
        extra.put(0x20, 0u8);
    }
    records.clear_abilities_week(human);
    for id in squad {
        let Some(p) = records.player_mut(id) else { continue };
        p.set_away_kind(p.away_kind() & !SUSPENDED);
        p.set_picks(p.picks() & !PICKS);
        p.set_retiring(p.retiring() & !RETIRING_BIT);
        crate::mood::clear_reasons(p, GRIEVANCES);
        if crate::mood::parts(p)[0] < 100 {
            let mut parts = p.contentment();
            parts[0] = 100;
            p.set_contentment(parts);
            p.set_morale(crate::mood::morale(p));
        }
    }
    crate::youth::end_club_trials(records, clubs, world, country, number, true);
}

/// The contract's end when the human in `slot` leaves his club
/// (`FUN_006da008(manager, club, flag, 3)`, from `FUN_006da0fe` after
/// [`release_club`]). With `counted` (the career's end passes 1): a club
/// in one of the countries played besides the main one (manager +0x1e
/// among `DAT_00b5cbb4`, `FUN_00676c5d`) takes one off the object's
/// +0x17c9 while it stands above nought, and +0x17ce counts one more
/// (the object's block +0x17bc is the record's +0x19ac). His flat
/// (the private block's +0xc, rec+0x1d0, levels 1 to 7) is sold for 70 %
/// of its price (`DAT_00886584[level]` × `0.7f` at `0x83113c`, cut by
/// `ftol`) into his fortune (+0x98, rec+0x118), its level back to nought
/// (`FUN_006a5d6e`, `FUN_006a5d9f`). Then the club goes: +0x1c to +0x22
/// nought, the record's +0x26 and the object's +0x17dc bits 6 to 8
/// (rec+0x1f1c, `FUN_006a6370(0)`), and +0xa4 (rec+0x19a).
pub fn end_contract(records: &mut Records, slot: usize, counted: bool) {
    const ABROAD_AT: usize = 0x19ac + 0xd;
    const ENDS_AT: usize = 0x19ac + 0x12;
    const FLAT_AT: usize = 0x1c4 + 0xc;
    const FLAT_PRICES: [i32; 8] = [4531, 200_000, 400_000, 600_000, 1_000_000, 1_500_000, 3_000_000, 10_000_000];
    const SOLD_FOR: f32 = 0.7;
    const CLUB_AT: usize = 0x1c;
    const PLAYING_AT: usize = 0x26;
    const STATE_BITS_AT: usize = 0x1f1c;
    const STATE_BITS: u32 = 0x1c0;
    const STATION_AT: usize = 0x19a;
    let more: Vec<i16> = {
        let g = &records.globals;
        let n = usize::try_from(g.more_count()).unwrap_or(0);
        g.more_countries().iter().take(n).copied().collect()
    };
    let Some(m) = records.managers.get_mut(slot) else { return };
    if counted {
        if more.contains(&(m.country() as i16)) && m.get::<u8>(ABROAD_AT) != 0 {
            m.put(ABROAD_AT, m.get::<u8>(ABROAD_AT) - 1);
        }
        m.put(ENDS_AT, m.get::<u8>(ENDS_AT).wrapping_add(1));
    }
    let flat = m.get::<u8>(FLAT_AT);
    if (1..8).contains(&flat) {
        let sold = (f64::from(FLAT_PRICES[usize::from(flat)]) * f64::from(SOLD_FOR)).trunc() as i32;
        m.set_cash(m.cash().wrapping_add(sold));
        m.put(FLAT_AT, 0u8);
    }
    m.put(CLUB_AT, [0u8; 7]);
    m.put(PLAYING_AT, 0u8);
    m.put(STATE_BITS_AT, m.get::<u32>(STATE_BITS_AT) & !STATE_BITS);
    m.put(STATION_AT, 0u8);
}

/// The club's deposits dissolved when the human leaves it at his career's
/// end (`FUN_005e0147(club, 3)`, from `FUN_006da0fe` after
/// [`end_contract`]): bit 2 of the flags sends it to `FUN_00648847`,
/// [`crate::finances::dissolve_all`] at the funds' prices of the day.
pub fn dissolve_deposits(records: &mut Records, club: (u8, u16)) {
    let prices = records.globals.fund_prices();
    let Some(team) = records.team_mut(u16::from(club.0), club.1) else { return };
    crate::finances::dissolve_all(team, prices);
}

/// The table of the human clubs rebuilt (`FUN_00635503`, from
/// `FUN_006da0fe` after [`dissolve_deposits`]): the 32 bytes at
/// `DAT_00b5cbe2` cleared and, walking the humans in their order
/// (`FUN_006a421a`, "GetMitspielerNachReihenfolge", up to `DAT_00b5cbce`),
/// one eight-byte entry for each who stands at a club (+0x1e not nought):
/// the country's byte, the club's number (+0x20) and the manager's number
/// (+0x1a) twice; their count at `DAT_00b5cc02`.
pub fn rebuild_human_clubs(records: &mut Records) {
    const LIST: usize = 0xb5cbe2;
    const COUNT: usize = 0xb5cc02;
    let order = records.globals.order();
    let humans = usize::from(records.globals.humans()).min(order.len());
    let mut list = [0u8; 32];
    let mut count = 0usize;
    for &human in &order[..humans] {
        let Some(m) = usize::try_from(human - 1).ok().and_then(|i| records.managers.get(i)) else { continue };
        if m.country() == 0 || count >= 4 {
            continue;
        }
        let e = &mut list[count * 8..count * 8 + 8];
        e[0] = m.country() as u8;
        e[2..4].copy_from_slice(&m.team().to_le_bytes());
        // The manager's number: the record read from a save leaves +0x1a
        // at nought, so his number in the order stands in for it.
        e[4..6].copy_from_slice(&human.to_le_bytes());
        e[6..8].copy_from_slice(&human.to_le_bytes());
        count += 1;
    }
    records.globals.put(LIST, list);
    records.globals.put::<i16>(COUNT, count as i16);
}

/// One entry of the ticker of jobs (`DAT_00a2e55c`, 0x14 bytes each from
/// +4 behind the count word): its two names (+0, +2 — a manager's number
/// negated), its kind (+4: 3 a career's end, 1, else another text of
/// `CGefaehrdeteJobs` +0x1d68/+0x1d6c/+0x1d70), the club (+6 country
/// byte, +8 number) and the flags at +0xc (told, `FUN_005dfa97`) and
/// +0x10 (the dialog `FUN_004aacc1` lists only entries with it set).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JobNote {
    pub first: i16,
    pub second: i16,
    pub kind: i16,
    pub club: (u8, u16),
    pub told: bool,
    pub live: bool,
}

/// The most entries the ticker of jobs takes (`FUN_005df799`'s 0x32).
pub const MOST_JOB_NOTES: usize = 50;

/// The club come free at the career's end of the human in `slot`
/// (`FUN_005df799(club, manager, 3)` on the list at `DAT_00a2e55c`, last
/// in `FUN_006da0fe`): with fewer than fifty entries one more, the
/// manager's number negated at +2, the kind, the club's country (club
/// +0x104) and number (+0x103), both flags cleared. +0 stays as the slot
/// held it; a fresh list holds nought there.
pub fn post_vacancy(notes: &mut Vec<JobNote>, slot: usize, club: (u8, u16), kind: i16) {
    if notes.len() >= MOST_JOB_NOTES {
        return;
    }
    notes.push(JobNote { first: 0, second: -((slot + 1) as i16), kind, club, told: false, live: false });
}

/// The humans' promotions at the season's turn (`FUN_006b4e2f` walking
/// the humans by the count at `DAT_00b5cbce`): each one whose club carries
/// the season's promoted flag — team +0x110's bit 4, `FUN_0069a1e1` —
/// counts it in his career. A promotion is no marked kind, so this is the
/// counter at +0x16 alone: no window entry, no station's mark. What the
/// same function does beside is not kept: the manager object's own tallies
/// of promotions and relegations (the struct's +0x10 and +0x18, through
/// `FUN_0069a0e1` and `FUN_0069a221`), his last club's standing
/// (+0xb5 = `FUN_00775fa0`), the mask off +0xba and the zero at +0xc0, and
/// `FUN_006a4917`.
pub fn promotions(records: &mut Records) {
    for (slot, held) in records.globals.slots().iter().copied().enumerate() {
        if held == 0 {
            continue;
        }
        let (country, number) = {
            let m = &records.managers[slot];
            (m.country(), m.team())
        };
        if country == 0 {
            // The manager stands for no club; `*(short *)(trainer + 0x1e)
            // != 0` is the original's own gate here.
            continue;
        }
        if records.team(country, number).is_some_and(|t| t.get::<u16>(0x110) & crate::promotion::PROMOTED != 0) {
            title(records, slot, country, number, crate::honours::PROMOTION);
        }
    }
}

/// One more on the station the manager stands at — its last row, since
/// the original writes through `(byte)(count - 1)` unchecked: with no row
/// of his yet there is nothing to mark.
fn mark_station(car: &mut [u8], kind: u8) {
    let (base, count_at, mark) = match kind {
        1 => (CLUBS_AT, CLUB_STATIONS, WON_CHAMPIONSHIPS),
        4 => (CLUBS_AT, CLUB_STATIONS, WON_CUPS),
        6 | 7 | 8 => (CLUBS_AT, CLUB_STATIONS, WON_EUROPEAN),
        0xb => (NATIONALS_AT, NATIONAL_STATIONS, WON_WORLD),
        0xc => (NATIONALS_AT, NATIONAL_STATIONS, WON_CONTINENTAL),
        _ => (CLUBS_AT, CLUB_STATIONS, WON_OTHER),
    };
    let last = usize::from(car[count_at]).wrapping_sub(1);
    if let Some(row) = car.get_mut(base + last * STATION + mark) {
        *row = row.wrapping_add(1);
    }
}

fn word(car: &[u8], at: usize) -> i16 {
    car.get(at..at + 2).map_or(0, |w| i16::from_le_bytes([w[0], w[1]]))
}

fn store(car: &mut [u8], at: usize, v: i16) {
    if let Some(w) = car.get_mut(at..at + 2) {
        w.copy_from_slice(&v.to_le_bytes());
    }
}

fn bump(car: &mut [u8], at: usize) {
    store(car, at, word(car, at).wrapping_add(1));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A career's end puts the club on the ticker of jobs, the manager's
    /// number negated; a full ticker takes nothing more.
    #[test]
    fn a_career_end_puts_the_club_on_the_ticker() {
        let mut notes = Vec::new();
        post_vacancy(&mut notes, 1, (3, 7), 3);
        assert_eq!(notes, [JobNote { first: 0, second: -2, kind: 3, club: (3, 7), told: false, live: false }]);
        notes.resize(MOST_JOB_NOTES, JobNote::default());
        post_vacancy(&mut notes, 0, (1, 1), 3);
        assert_eq!(notes.len(), MOST_JOB_NOTES);
        assert_eq!(notes[MOST_JOB_NOTES - 1], JobNote::default());
    }

    /// The kept saves' own table of the human clubs comes back byte for
    /// byte when rebuilt; with the human at no club it empties.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_human_clubs_table_is_rebuilt_as_the_game_keeps_it() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        for name in ["A", "A2", "S2c", "S2late3", "SeasonEnd364", "NewSeason000"] {
            let mut records = Records::from_save(Save::read(&keeps.join(format!("{name}.an3"))).unwrap()).unwrap();
            let kept: [u8; 32] = records.globals.get(0xb5cbe2);
            let count: i16 = records.globals.get(0xb5cc02);
            assert!(count > 0, "{name}: a human at a club");
            rebuild_human_clubs(&mut records);
            assert_eq!(records.globals.get::<[u8; 32]>(0xb5cbe2), kept, "{name}: the table");
            assert_eq!(records.globals.get::<i16>(0xb5cc02), count, "{name}: the count");
            for m in &mut records.managers {
                m.set_country(0);
            }
            rebuild_human_clubs(&mut records);
            assert_eq!(records.globals.get::<[u8; 32]>(0xb5cbe2), [0; 32]);
            assert_eq!(records.globals.get::<i16>(0xb5cc02), 0);
        }
    }

    /// The kept day-63 save's human manager: a championship, a cup, the
    /// finalist's part of it and a supercup land in his own record — the
    /// marked ones in its window of titles with the kind, the season, his
    /// club's country and number, and on the mark of the one club station
    /// he stands at; every kind in its own counter; the finalist's part
    /// nowhere but its counter; the championship once more in the
    /// manager's own count of titles. Nothing else in the block moves, and
    /// with the window full a title drops the oldest entry out.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_title_lands_in_the_humans_own_record() {
        use crate::honours::{CHAMPIONSHIP, CUP, CUP_FINALIST, SUPERCUP};
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let read = || Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let records = read();
        let slot = records.globals.slots().iter().position(|&s| s > 0).unwrap();
        let human = u8::try_from(slot + 1).unwrap();
        let (country, number) = (records.managers[slot].country(), records.managers[slot].team());
        let team = records.team(country, number).expect("the human's club");
        assert_eq!(team.humans()[1], human, "the club names slot {human} as its manager");
        let (club, club_number) = (team.country(), team.number());
        let season = records.globals.seasons();
        drop(team);
        let before: Vec<u8> = records.career(slot).unwrap().to_vec();
        let titles = records.managers[slot].titles();

        let mut ours = read();
        title(&mut ours, slot, country, number, CHAMPIONSHIP);
        let car = ours.career(slot).unwrap().to_vec();
        assert_eq!(word(&car, 0x0), 1, "the championship's counter");
        assert_eq!(word(&car, TOTAL), 1);
        assert_eq!(word(&car, WINDOW_COUNT), 1);
        let entry = [CHAMPIONSHIP, 0, season as u8, (season >> 8) as u8, club, 0, club_number, 0];
        assert_eq!(&car[ENTRIES..ENTRIES + ENTRY], &entry, "the title in the window");
        assert_eq!(car[CLUBS_AT + WON_CHAMPIONSHIPS], 1, "the station's mark");
        assert_eq!(ours.managers[slot].titles()[0], titles[0] + 1, "the manager's own count");
        still_the_same(&before, &car, &[0..2, WINDOW_COUNT..WINDOW_COUNT + 2, TOTAL..TOTAL + 2, ENTRIES..ENTRIES + ENTRY, CLUBS_AT + WON_CHAMPIONSHIPS..CLUBS_AT + WON_CHAMPIONSHIPS + 1]);

        title(&mut ours, slot, country, number, CUP);
        title(&mut ours, slot, country, number, CUP_FINALIST);
        title(&mut ours, slot, country, number, SUPERCUP);
        let car = ours.career(slot).unwrap().to_vec();
        assert_eq!((word(&car, 0x2), word(&car, 0x4), word(&car, 0x6)), (1, 1, 1), "the counters of the cup, the finalist and the supercup");
        assert_eq!(word(&car, WINDOW_COUNT), 3, "the finalist counts in no window");
        assert_eq!(word(&car, TOTAL), 3);
        assert_eq!(car[ENTRIES + ENTRY], CUP);
        assert_eq!(car[ENTRIES + 2 * ENTRY], SUPERCUP);
        assert_eq!(
            (car[CLUBS_AT + WON_CHAMPIONSHIPS], car[CLUBS_AT + WON_CUPS], car[CLUBS_AT + WON_OTHER]),
            (1, 1, 1),
            "one title of each kind on his station"
        );
        assert_eq!(ours.managers[slot].titles()[0], titles[0] + 1, "only a championship lifts it");

        // The window at its fiftieth: the oldest entry gives way.
        let mut full = read();
        let second: Vec<u8> = {
            let car = full.career_mut(slot).unwrap();
            car[WINDOW_COUNT..WINDOW_COUNT + 2].copy_from_slice(&49i16.to_le_bytes());
            for k in 0..50 {
                car[ENTRIES + k * ENTRY] = u8::try_from(k + 1).unwrap();
            }
            car[ENTRIES + ENTRY..ENTRIES + 2 * ENTRY].to_vec()
        };
        title(&mut full, slot, country, number, CUP);
        let car = full.career(slot).unwrap().to_vec();
        assert_eq!(word(&car, WINDOW_COUNT), 49, "the count back below its fifty");
        assert_eq!(word(&car, TOTAL), 1);
        assert_eq!(&car[ENTRIES..ENTRIES + ENTRY], &second[..ENTRY], "the oldest gone, the rest one on");
        assert_eq!(car[ENTRIES + 48 * ENTRY], CUP, "the new title where `+0x22 + count * 8` puts it");
        assert_eq!(car[ENTRIES + 49 * ENTRY], 50, "the window's stale tail, as the original leaves it");
    }

    /// The season's books put the human's club up: he counts the promotion
    /// in his own career — its counter at +0x16 alone. A promotion is no
    /// marked kind, so the window of titles stays as it was, the station's
    /// marks stay as they were, and the manager's own count of titles stays
    /// with a championship's. Without the flag on his club nothing of his
    /// record moves at all.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn a_promotion_counts_in_the_humans_own_career() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let read = || Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let records = read();
        let slot = records.globals.slots().iter().position(|&s| s > 0).unwrap();
        let (country, number) = (records.managers[slot].country(), records.managers[slot].team());
        assert_eq!(records.team(country, number).unwrap().get::<u16>(0x110) & crate::promotion::PROMOTED, 0, "no kept save has the human's club going up");
        drop(records);

        // No flag, no credit.
        let mut ours = read();
        let before: Vec<u8> = ours.career(slot).unwrap().to_vec();
        let titles = ours.managers[slot].titles();
        promotions(&mut ours);
        still_the_same(&before, ours.career(slot).unwrap(), &[]);
        assert_eq!(ours.managers[slot].titles(), titles);

        // The season's flag on, once and then again.
        let flags = ours.team(country, number).unwrap().get::<u16>(0x110);
        ours.team_mut(country, number).unwrap().put::<u16>(0x110, flags | crate::promotion::PROMOTED);
        promotions(&mut ours);
        let car = ours.career(slot).unwrap().to_vec();
        assert_eq!(word(&car, 0x16), 1, "the promotion's counter");
        assert_eq!(word(&car, WINDOW_COUNT), word(&before, WINDOW_COUNT), "no title in the window");
        assert_eq!(word(&car, TOTAL), word(&before, TOTAL));
        still_the_same(&before, &car, &[0x16..0x18]);
        assert_eq!(ours.managers[slot].titles(), titles, "only a championship lifts it");
        promotions(&mut ours);
        assert_eq!(word(ours.career(slot).unwrap(), 0x16), 2);
    }

    /// The kept day-63 save's human steps down from his club: his one club
    /// station is closed with today's date, reason 4, his two running
    /// figures and the sum of his nine marks, taken after the season's
    /// youth and service marks went into his honour marks; the count of
    /// stepping down goes up, nothing else in the record moves. A
    /// dismissal (reason 3) counts on the other counter.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn leaving_the_club_closes_his_station() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let mut ours = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let slot = ours.globals.slots().iter().position(|&s| s > 0).unwrap();
        let before = ours.career(slot).unwrap().to_vec();
        assert_eq!(before[CLUB_STATIONS], 1, "one club station");
        let m = &mut ours.managers[slot];
        m.set_youth_marks(7);
        m.set_service_marks(11);
        let honour = m.honour_marks();
        m.set_station_figures([0x123, -5]);
        left_club(&mut ours, slot, 4);
        let m = &ours.managers[slot];
        assert_eq!((m.youth_marks(), m.service_marks(), m.honour_marks()), (0, 0, honour + 18));
        let sum: i32 = crate::evaluation::marks(&ours, m, slot + 1).iter().sum();
        let car = ours.career(slot).unwrap().to_vec();
        let row = &car[CLUBS_AT..CLUBS_AT + STATION];
        let [d, mo, y] = ours.globals.date();
        assert_eq!((row[8], row[9], u16::from_le_bytes([row[0xa], row[0xb]])), (d as u8, mo as u8, y));
        assert_eq!(row[0xc], 4);
        assert_eq!(i16::from_le_bytes([row[0xe], row[0xf]]), 0x123);
        assert_eq!(i16::from_le_bytes([row[0x10], row[0x11]]), -5);
        assert_eq!(i32::from_le_bytes(row[0x14..0x18].try_into().unwrap()), sum);
        assert_eq!(word(&car, STEPPED_DOWN), word(&before, STEPPED_DOWN) + 1);
        still_the_same(&before, &car, &[STEPPED_DOWN..STEPPED_DOWN + 2, CLUBS_AT + 8..CLUBS_AT + 0x18]);
        left_club(&mut ours, slot, 3);
        let car = ours.career(slot).unwrap();
        assert_eq!(word(car, DISMISSED), word(&before, DISMISSED) + 1);
        assert_eq!(car[CLUBS_AT + 0xc], 3);
    }

    /// The kept day-63 save's human leaves his club: 10 standing marks
    /// and 8 nerves go, 40 marks once his club stands in the relegation
    /// places; the nerves stop at nought.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn leaving_the_club_costs_standing_and_nerves() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let mut ours = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let slot = ours.globals.slots().iter().position(|&s| s > 0).unwrap();
        let m = &ours.managers[slot];
        let club = (m.country() as u8, m.team() as u16);
        let league = {
            let t = ours.team(club.0.into(), club.1).unwrap();
            crate::opponent::League::of(&ours, t.country().into(), t.level()).unwrap()
        };
        let (teams, down) = (league.teams() as u8, league.relegations() as u8);
        assert!(down > 0, "the league sends some down");
        let set_place = |r: &mut Records, place: u8| {
            let t = r.teams.iter_mut().find(|t| (t.country(), t.number() as u16) == club).unwrap();
            t.set_table_place(place);
        };
        set_place(&mut ours, teams - down);
        ours.managers[slot].set_nerves(50);
        let marks = ours.managers[slot].standing_marks();
        leaving_costs(&mut ours, slot, club);
        assert_eq!((ours.managers[slot].standing_marks(), ours.managers[slot].nerves()), (marks - 10, 42));
        set_place(&mut ours, teams - down + 1);
        ours.managers[slot].set_nerves(5);
        leaving_costs(&mut ours, slot, club);
        assert_eq!((ours.managers[slot].standing_marks(), ours.managers[slot].nerves()), (marks - 50, 0));
    }

    /// The kept save A's human ends his career: his club has no human left
    /// in it, its slot and extra data free, and a suspended, always-picked
    /// player with the trainer's grievances and his trust in him low gets
    /// his suspension, pick and grievances lifted and his trust back to a
    /// hundred.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn ending_the_career_releases_the_club_and_its_players() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let mut ours = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let data = crate::data::Data::load(&dir).unwrap();
        let slot = ours.globals.slots().iter().position(|&s| s > 0).unwrap();
        let m = &ours.managers[slot];
        let club = (m.country() as u8, m.team() as u16);
        let human = ours.team(club.0.into(), club.1).unwrap().human();
        assert_eq!(usize::from(human), slot + 1);
        let id = ours.team(club.0.into(), club.1).unwrap().players()[0];
        let p = ours.player_mut(id).unwrap();
        p.set_away_kind(p.away_kind() | 0x10);
        p.set_picks(p.picks() | 0x20);
        p.set_flags(p.flags() | 0x100 | 0x4000 | 0x4);
        let mut parts = p.contentment();
        parts[0] = 70;
        p.set_contentment(parts);
        let mut clubs = std::collections::BTreeMap::new();
        release_club(&mut ours, &mut clubs, &data.world, club);
        let t = ours.team(club.0.into(), club.1).unwrap();
        assert_eq!((t.humans(), t.human()), ([0, 0], 0));
        assert_eq!(ours.extras[slot].get::<u8>(0x20), 0);
        let p = ours.player(id).unwrap();
        assert_eq!((p.away_kind() & 0x10, p.picks() & 0x30), (0, 0));
        assert_eq!(p.flags() & (0x100 | 0x1000 | 0x4000 | 0x4), 0x4, "only the trainer's three go");
        assert_eq!(p.contentment()[0], 100);
        assert_eq!(p.morale(), crate::mood::morale(p));
    }

    /// The kept save A's human ends his career with a flat at level 1:
    /// he gets 139,999 for it (200,000 × 0.7f, cut), the flat goes, his
    /// club fields are nought and his career count of ends one more.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn ending_the_contract_sells_the_flat_and_clears_the_club() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let mut ours = Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let slot = ours.globals.slots().iter().position(|&s| s > 0).unwrap();
        let m = &mut ours.managers[slot];
        assert_ne!(m.country(), 0);
        m.put(0x1d0, 1u8);
        let (cash, ends) = (m.cash(), m.get::<u8>(0x19be));
        let before = m.bytes().to_vec();
        end_contract(&mut ours, slot, true);
        let m = &ours.managers[slot];
        assert_eq!(m.cash(), cash + 139_999);
        assert_eq!((m.get::<u8>(0x1d0), m.country(), m.team(), m.status()), (0, 0, 0, 0));
        assert_eq!(m.get::<u8>(0x19be), ends + 1);
        still_the_same(&before, m.bytes(), &[0x1c..0x23, 0x26..0x27, 0x118..0x11c, 0x19a..0x19b, 0x1d0..0x1d1, 0x19b9..0x19ba, 0x19be..0x19bf, 0x1f1c..0x1f1d]);
    }

    /// Only the bytes in `touched` differ.
    fn still_the_same(before: &[u8], after: &[u8], touched: &[std::ops::Range<usize>]) {
        for (i, (a, b)) in before.iter().zip(after).enumerate() {
            assert!(a == b || touched.iter().any(|r| r.contains(&i)), "byte {i:#x} moved, {a:#x} → {b:#x}");
        }
    }
}
