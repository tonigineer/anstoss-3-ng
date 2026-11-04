//! The youth leagues (`0xb77522` + age group × 0xd88 + league × 0x2898 +
//! slot × 0x7efc, `FUN_0066fa2c`): per country slot, league and age group
//! the teams, the schedule and the results, as the senior leagues keep
//! them; and per club and age group how far its youth stands
//! (`0xb7eeea`, `FUN_0066fae0`). Their tables are reckoned from the
//! results as the game reckons them (`FUN_00729f8b`) and sorted as it
//! sorts them (`FUN_0072b9b9` with `FUN_0072bc49`).

use crate::records::Records;

/// The objects of the youth leagues, one per country slot; their saved
/// bytes start 0x20 into the object.
const OBJECT: u32 = 0xb77500;
const SAVED_FROM: usize = 0x20;
const LEAGUES_AT: usize = 0x22 - SAVED_FROM;
const AGE_GROUP: usize = 0xd88;
const LEAGUE: usize = 0x2898;
const STAGES_AT: usize = 0xb7eeea - 0xb77500 - SAVED_FROM;
const STAGES: usize = 0x510;
const CLUB_STAGES: usize = 0xc;

const TEAMS: usize = 2;
const PLAYED: usize = 4;
const FOUR_ROUNDS: usize = 6;
const NUMBERS: usize = 8;
const SCHEDULE: usize = 0x38;
const ROW: usize = 0x18;
const RESULTS: usize = 0x488;
const RESULT_ROW: usize = 0x60;
const UNPLAYED: u8 = 0xff;

/// A youth league of an age group (0 A, 1 B, 2 C).
pub struct YouthLeague<'a> {
    data: &'a [u8],
}

/// A match's goals, home first, and the half-time's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Score {
    pub home: u8,
    pub away: u8,
    pub half_home: u8,
    pub half_away: u8,
}

impl<'a> YouthLeague<'a> {
    pub fn of(records: &'a Records, country: u16, league: usize, age: usize) -> Option<Self> {
        let slot = records.countries.slot(country.into());
        let block = records.block(&format!("{OBJECT:#x}[{slot}]"))?;
        let at = LEAGUES_AT + age * AGE_GROUP + league * LEAGUE;
        Some(Self { data: block.get(at..at + AGE_GROUP)? })
    }

    fn i16_at(&self, at: usize) -> i16 {
        i16::from_le_bytes([self.data[at], self.data[at + 1]])
    }

    pub fn teams(&self) -> usize {
        usize::try_from(self.i16_at(TEAMS)).unwrap_or(0).min(24)
    }

    /// The matchdays played.
    pub fn played(&self) -> usize {
        usize::try_from(self.i16_at(PLAYED)).unwrap_or(0)
    }

    fn four_rounds(&self) -> bool {
        self.i16_at(FOUR_ROUNDS) != 0
    }

    /// The team number at a slot.
    pub fn number(&self, slot: usize) -> i16 {
        self.i16_at(NUMBERS + 2 * slot)
    }

    /// The pairs of a matchday's row of the schedule, by slots.
    pub fn pairs(&self, row: usize) -> Vec<(usize, usize)> {
        let at = SCHEDULE + row * ROW;
        (0..self.teams() / 2)
            .map(|k| (usize::from(self.data[at + 2 * k]), usize::from(self.data[at + 2 * k + 1])))
            .collect()
    }

    /// The result of a slot's match on a matchday from 1, if played
    /// (`FUN_00670978`): a league of four rounds plays its second two on
    /// the first two's rows, its results beside theirs.
    pub fn result(&self, matchday: usize, slot: usize) -> Option<Score> {
        let n = self.teams();
        let (mut row, mut offset) = (matchday, 0);
        if self.four_rounds() && matchday + 2 > 2 * n {
            row -= 2 * n - 2;
            offset = n;
        }
        let (home, away) =
            self.pairs(row.checked_sub(1)?).into_iter().find(|&(h, a)| h == slot || a == slot)?;
        let at = RESULTS + home * RESULT_ROW + (away + offset) * 4;
        let b = self.data.get(at..at + 4)?;
        (b[0] != UNPLAYED).then_some(Score {
            home: b[0],
            away: b[1],
            half_home: b[2],
            half_away: b[3],
        })
    }

    /// A table of the matchdays played (`FUN_00729f8b`): all matches, or
    /// only the home or the away ones, sorted.
    pub fn table(&self, games: Games) -> Vec<Row> {
        let mut rows: Vec<Row> = (0..self.teams())
            .map(|slot| Row { number: self.number(slot), ..Row::default() })
            .collect();
        // The pairs from the matchday's own row; only the result's lookup
        // folds the later rounds onto the first.
        for matchday in 1..=self.played() {
            for (home, away) in self.pairs(matchday - 1) {
                let Some(score) = self.result(matchday, home) else { continue };
                if games != Games::Away {
                    rows[home].count(score.home, score.away);
                }
                if games != Games::Home {
                    rows[away].count(score.away, score.home);
                }
            }
        }
        sort(&mut rows);
        rows
    }
}

/// Which matches a table counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Games {
    All,
    Home,
    Away,
}

/// A team's line of a table (0x14 bytes in the game).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Row {
    pub number: i16,
    pub games: i16,
    pub points: i16,
    pub goals_for: i16,
    pub goals_against: i16,
    pub won: i16,
    pub drawn: i16,
    pub lost: i16,
}

impl Row {
    fn count(&mut self, own: u8, theirs: u8) {
        self.games += 1;
        match own.cmp(&theirs) {
            std::cmp::Ordering::Greater => {
                self.points += 3;
                self.won += 1;
            }
            std::cmp::Ordering::Equal => {
                self.points += 1;
                self.drawn += 1;
            }
            std::cmp::Ordering::Less => self.lost += 1,
        }
        self.goals_for += i16::from(own);
        self.goals_against += i16::from(theirs);
    }

    pub fn difference(&self) -> i16 {
        self.goals_for - self.goals_against
    }

    /// Whether it stands below `other` (`FUN_0072bc49`): fewer points, a
    /// worse difference, fewer goals, fewer games.
    fn below(&self, other: &Row) -> bool {
        (self.points, self.difference(), self.goals_for, self.games)
            < (other.points, other.difference(), other.goals_for, other.games)
    }
}

/// The game's bubble sort: neighbours change places while one stands
/// below the next (`FUN_0072b9b9`).
fn sort(rows: &mut [Row]) {
    let mut swapped = true;
    while swapped {
        swapped = false;
        for i in 1..rows.len() {
            if rows[i - 1].below(&rows[i]) {
                rows.swap(i - 1, i);
                swapped = true;
            }
        }
    }
}

/// How far a club's youth of an age group has come (`FUN_0066fae0`): the
/// stage it is in (3 the final round), its place there, whether it has
/// played in it.
pub struct Stage {
    pub stage: u8,
    pub place: u8,
    pub played: bool,
}

pub fn stage(records: &Records, country: u16, club: usize, age: usize) -> Option<Stage> {
    let slot = records.countries.slot(country.into());
    let block = records.block(&format!("{OBJECT:#x}[{slot}]"))?;
    let stages = block.get(STAGES_AT..STAGES_AT + STAGES)?;
    let at = club * CLUB_STAGES + age;
    Some(Stage {
        stage: *stages.get(at)?,
        place: *stages.get(at + 3)?,
        played: *stages.get(at + 9)? != 0,
    })
}

/// The stage byte of the youth's final round.
pub const FINAL_ROUND: u8 = 3;
/// The stage of a reserve team's youth, which no final round has.
const RESERVE: u8 = 4;
/// The clubs a country's stages hold, and the places of a final round.
const STAGE_CLUBS: usize = 0x6c;
const FINAL_PLACES: u8 = 5;
/// The age groups, the leagues each may have, and the teams a season
/// moves between two of them.
const AGES: usize = 3;
const LEAGUES: usize = 3;
const MOVED: usize = 3;
/// Main countries whose youth plays in three leagues, and in two.
const THREE_LEAGUES: [i16; 2] = [14, 16];
const TWO_LEAGUES: [i16; 3] = [20, 28, 63];

fn league_at(age: usize, league: usize) -> usize {
    LEAGUES_AT + age * AGE_GROUP + league * LEAGUE
}

/// The youth matchdays fall on the senior leagues' (`FUN_0062a959`): a
/// calendar code below this is a matchday, counted from 1.
const LAST_CODE: u32 = 0x3c;
/// The schedule's rows.
const ROWS: usize = (RESULTS - SCHEDULE) / ROW;
/// What a youth match's sides start from (`FUN_0066fdb8`): the planned
/// youth support (club +0x1a8, team +0x420) in hundred thousands, three
/// hundred and fifty at home, three hundred away, five more for a club of
/// team flag +0x95c's bit 2; then scaled so both together come to some 56
/// chances in a thousand a minute.
const SUPPORT_AT: usize = 0x420;
const SUPPORT_UNIT: i32 = 100_000;
const HOME_START: i32 = 350;
const AWAY_START: i32 = 300;
const FLAG_AT: usize = 0x95c;
const FLAG: u8 = 2;
const FLAG_BONUS: i32 = 5;
const SCALE: i32 = 56;
const MINUTES: usize = 90;
const HALF_TIME: usize = 46;
/// R+0x352: a youth's goals; a station's goals at +0xc.
const YOUTH_GOALS: usize = 0x352;
const STATIONS: usize = 0x20;
const STATION: usize = 0x14;
const STATION_GOALS: usize = 0xc;

/// The day's youth matchdays (`FUN_0062a959`): the main country's three
/// youth leagues on the days its first three senior leagues play, the
/// more countries' first on their first league's.
pub fn play_day(records: &mut Records, calendar: &crate::calendar::Calendar, day: usize, random: &mut crate::random::Random) {
    let globals = &records.globals;
    let Ok(main) = u16::try_from(globals.main_country()) else { return };
    let more = usize::try_from(globals.more_count()).unwrap_or(0).min(globals.more_countries().len());
    let mut due = Vec::new();
    for league in 0..LEAGUES {
        due.push((main, league));
    }
    for &country in &globals.more_countries()[..more] {
        if let Ok(country) = u16::try_from(country) {
            due.push((country, 0));
        }
    }
    for (country, league) in due {
        let Some(row) = calendar.row(country, league) else { continue };
        let code = calendar.entry(row, day);
        if code != 0 && code < LAST_CODE {
            play_matchday(records, country, league, code as usize - 1, random);
        }
    }
}

/// A youth league's matchday (from 0) for each age group in turn
/// (`FUN_0066fdb8`), its first league's then moving the final round.
fn play_matchday(records: &mut Records, country: u16, league: usize, matchday: usize, random: &mut crate::random::Random) {
    let slot = records.countries.slot(country.into());
    let name = format!("{OBJECT:#x}[{slot}]");
    for age in 0..AGES {
        let at = league_at(age, league);
        let Some(block) = records.block(&name) else { return };
        let Some(data) = block.get(at..at + AGE_GROUP) else { return };
        let youth = YouthLeague { data };
        let teams = youth.teams();
        if matchday >= ROWS {
            continue;
        }
        let row = SCHEDULE + matchday * ROW;
        let pairs: Vec<(usize, usize)> =
            (0..teams / 2).map(|k| (usize::from(data[row + 2 * k]), usize::from(data[row + 2 * k + 1]))).collect();
        let numbers: Vec<i16> = (0..24).map(|k| youth.number(k)).collect();
        let mut results = Vec::new();
        for &(home, away) in &pairs {
            let (Some(&h), Some(&a)) = (numbers.get(home), numbers.get(away)) else { continue };
            let score = play_match(records, country, (h, a), age, random);
            results.push((home, score));
        }
        let Some(block) = records.block_mut(&name) else { return };
        let data = &mut block[at..at + AGE_GROUP];
        data[PLAYED..PLAYED + 2].copy_from_slice(&((matchday + 1) as i16).to_le_bytes());
        for (home, score) in results {
            store(data, matchday + 1, home, score);
        }
    }
    if league == 0 {
        move_final_round(records, &name, random);
    }
}

/// Where a result goes (`FUN_00670831`): beside its pair in the
/// matchday's row, a four rounds' later two beside the first two's.
fn store(data: &mut [u8], matchday: usize, slot: usize, score: [u8; 4]) {
    let youth = YouthLeague { data };
    let n = youth.teams();
    let (mut row, mut offset) = (matchday, 0);
    if youth.four_rounds() && matchday + 2 > 2 * n {
        row -= 2 * n - 2;
        offset = n;
    }
    let Some(row) = row.checked_sub(1) else { return };
    let Some(&(home, away)) = youth.pairs(row).iter().find(|&&(h, a)| h == slot || a == slot) else {
        return;
    };
    let at = RESULTS + home * RESULT_ROW + (away + offset) * 4;
    if let Some(b) = data.get_mut(at..at + 4) {
        b.copy_from_slice(&score);
    }
}

/// One youth match of an age group (`FUN_0066fdb8`'s pair): the sides'
/// strengths, then ninety minutes of a chance each a minute, each goal
/// given to the first of the side's scorers whose draw comes under his
/// weight. Goals, then the half-time's.
fn play_match(records: &mut Records, country: u16, (home, away): (i16, i16), age: usize, random: &mut crate::random::Random) -> [u8; 4] {
    let side = |number: i16, start: i32| -> (i32, Vec<(u16, u32)>) {
        let Some(team) = u16::try_from(number).ok().and_then(|n| records.team(country, n)) else {
            return (start, Vec::new());
        };
        let mut strength = team.get::<i32>(SUPPORT_AT) / SUPPORT_UNIT + start;
        let (youths, _) = crate::youth::youths(records, country as u8, number as u16);
        let group: Vec<u16> = youths.into_iter().filter(|&id| crate::youth::class(records, id) == age).collect();
        strength += youth_strength(records, &group);
        if team.get::<u8>(FLAG_AT) & FLAG != 0 {
            strength += FLAG_BONUS;
        }
        (strength as i16 as i32, scorers(records, &group))
    };
    let (mut home_chance, home_scorers) = side(home, HOME_START);
    let (mut away_chance, away_scorers) = side(away, AWAY_START);
    let k = (home_chance + away_chance) / SCALE;
    if k > 0 {
        home_chance /= k;
        away_chance /= k;
    }
    let (mut goals, mut half) = ([0u8; 2], [0u8; 2]);
    for minute in 0..MINUTES {
        if minute == HALF_TIME {
            half = goals;
        }
        for (k, (chance, list)) in [(home_chance, &home_scorers), (away_chance, &away_scorers)].into_iter().enumerate() {
            if random.below(1000) < chance as u32 {
                goals[k] = goals[k].wrapping_add(1);
                for &(id, weight) in list {
                    if random.below(100) < weight {
                        score(records, id);
                        break;
                    }
                }
            }
        }
    }
    [goals[0], goals[1], half[0], half[1]]
}

/// What an age group's youths add to their side (`FUN_0066fb5b`): ten for
/// each point of strength above three, twenty for a nought, ten for a one.
fn youth_strength(records: &Records, group: &[u16]) -> i32 {
    let points: i32 = group
        .iter()
        .filter_map(|&id| records.player(id))
        .map(|p| match p.strength() {
            0 => 2,
            1 => 1,
            s if s > 3 => i32::from(s) - 3,
            _ => 0,
        })
        .sum();
    points * 10
}

/// An age group's scorers (`FUN_0066fc32`): the fit youths of the attack
/// (weight 20), the midfield (12) and the defence (4), the heavier first,
/// the stronger first among the same; no keeper.
fn scorers(records: &Records, group: &[u16]) -> Vec<(u16, u32)> {
    let mut list: Vec<(u16, u32, u8)> = Vec::new();
    for &id in group {
        let Some(p) = records.player(id) else { continue };
        if crate::load::days_to(&records.globals, p.injured_to()) >= 1 {
            continue;
        }
        let weight = match p.positions()[0] {
            10 => 20,
            6..=9 => 12,
            2..=5 => 4,
            _ => continue,
        };
        let strength = p.strength();
        let at = list
            .iter()
            .rposition(|&(_, w, s)| !(w < weight || w == weight && s < strength))
            .map_or(0, |k| k + 1);
        list.insert(at, (id, weight, strength));
    }
    list.into_iter().map(|(id, w, _)| (id, w)).collect()
}

/// A youth's goal (`FUN_00706c77`): into his youth goals and his last
/// station's (`FUN_0070432d`).
fn score(records: &mut Records, id: u16) {
    let Some(p) = records.player_mut(id) else { return };
    p.put::<u8>(YOUTH_GOALS, p.get::<u8>(YOUTH_GOALS).wrapping_add(1));
    if let Ok(count) = usize::try_from(p.career_count())
        && count > 0
    {
        let at = STATIONS + (count - 1) * STATION + STATION_GOALS;
        p.put::<u16>(at, p.get::<u16>(at).wrapping_add(1));
    }
}

/// The final round moves on with the first league's matchday
/// (`FUN_0066fdb8`'s end): each club in it has played, its place drifts
/// by its class — up or down by a draw of a hundred —, and of those come
/// to the first two places three drawn keep a place 0 or 1, the rest 2.
fn move_final_round(records: &mut Records, name: &str, random: &mut crate::random::Random) {
    let Some(block) = records.block_mut(name) else { return };
    let Some(stages) = block.get_mut(STAGES_AT..STAGES_AT + STAGES) else { return };
    for age in 0..AGES {
        let mut leaders = Vec::new();
        for club in 0..STAGE_CLUBS {
            let at = club * CLUB_STAGES + age;
            if stages[at] != FINAL_ROUND {
                continue;
            }
            stages[at + 9] = 1;
            let roll = random.below(100);
            let (down, up) = match stages[at + 6] {
                1 => (10, 95),
                2 => (8, 93),
                3 => (7, 91),
                _ => (0, 100),
            };
            let place = &mut stages[at + 3];
            if roll < down {
                *place = place.saturating_sub(1);
            }
            if roll > up {
                *place = if *place < 4 { *place + 1 } else { 4 };
            }
            if *place < 2 {
                leaders.push(club);
                *place = 2;
            }
        }
        if leaders.len() > 3 {
            let order = crate::cup_draw::order(leaders.len(), random);
            for k in 0..3 {
                stages[leaders[order[k]] * CLUB_STAGES + age + 3] = random.below(2) as u8;
            }
        }
    }
}

/// The youth leagues' promotions at the season's end (`FUN_00670ac5`,
/// after the country's clubs closed, `FUN_006b44cb`): per age group the
/// tables of the season decide, three teams going up and three down
/// between two leagues; the lowest league's three last make room for the
/// final round's best (stage 3, by their place there, then by club). A
/// main country Germany or England plays in three leagues, France,
/// Italy or Spain in two, any other country in one. Each league's
/// teams then stand in the order they come: the stayers by the table.
pub(crate) fn close_season(records: &mut Records, country: u16) {
    let main = records.globals.main_country();
    let code = country as i16;
    let leagues = if code == main && THREE_LEAGUES.contains(&code) {
        3
    } else if code == main && TWO_LEAGUES.contains(&code) {
        2
    } else {
        1
    };
    let slot = records.countries.slot(country.into());
    let name = format!("{OBJECT:#x}[{slot}]");
    for age in 0..AGES {
        let mut tables = Vec::new();
        for league in 0..leagues {
            let Some(youth) = YouthLeague::of(records, country, league, age) else { return };
            let rows: Vec<i16> = youth.table(Games::All).iter().map(|r| r.number).collect();
            tables.push(rows);
        }
        let Some(block) = records.block(&name) else { return };
        let Some(stages) = block.get(STAGES_AT..STAGES_AT + STAGES) else { return };
        let mut finalists = Vec::new();
        for place in 0..FINAL_PLACES {
            for club in 0..STAGE_CLUBS {
                let at = club * CLUB_STAGES + age;
                if stages[at] == FINAL_ROUND && stages[at + 3] == place {
                    finalists.push(club as i16);
                }
            }
        }
        let mut lists = Vec::new();
        for league in 0..leagues {
            let own = &tables[league];
            let n = own.len();
            if n < 2 * MOVED {
                lists.push(own.clone());
                continue;
            }
            let mut list = Vec::with_capacity(n);
            let first = if league > 0 {
                let above = &tables[league - 1];
                list.extend_from_slice(&above[above.len().saturating_sub(MOVED)..]);
                MOVED
            } else {
                0
            };
            list.extend_from_slice(&own[first..n - MOVED]);
            if league + 1 < leagues {
                list.extend(tables[league + 1].iter().take(MOVED));
            } else {
                list.extend(finalists.iter().take(n - list.len()));
            }
            // Short of finalists, the slots keep their old teams.
            while list.len() < n {
                list.push(own[list.len()]);
            }
            lists.push(list);
        }
        let Some(block) = records.block_mut(&name) else { return };
        for (league, list) in lists.iter().enumerate() {
            let at = league_at(age, league) + NUMBERS;
            for (k, &number) in list.iter().enumerate() {
                block[at + 2 * k..at + 2 * k + 2].copy_from_slice(&number.to_le_bytes());
            }
        }
    }
}

/// The youth leagues' new season (`FUN_00774088`'s last part, per played
/// country): every age group's results unplayed again (`FUN_00670777`,
/// the matchdays' count kept), and each club's stage anew — the youth
/// league it plays in (the first only, in a country of a single senior
/// league), else the final round, or none for a reserve team
/// (`FUN_006707df`, `FUN_0063e793`).
pub(crate) fn open_season(records: &mut Records, country: u16) {
    let slot = records.countries.slot(country.into());
    let name = format!("{OBJECT:#x}[{slot}]");
    let checked = if records.countries.leagues(country.into()) == 1 { 1 } else { LEAGUES };
    let clubs = records.countries.team_count(country.into()).max(0) as usize;
    let linked: Vec<i8> = (0..clubs)
        .map(|n| records.team(country, n as u16).map_or(-1, |t| t.linked()[0]))
        .collect();
    let Some(block) = records.block_mut(&name) else { return };
    if block.len() < STAGES_AT + STAGES {
        return;
    }
    for age in 0..AGES {
        for league in 0..LEAGUES {
            let at = league_at(age, league);
            block[at + RESULTS..at + AGE_GROUP].fill(UNPLAYED);
        }
        for club in 0..clubs.min(STAGE_CLUBS) {
            let plays = (0..checked).find(|&league| {
                let at = league_at(age, league);
                let teams = i16::from_le_bytes([block[at + TEAMS], block[at + TEAMS + 1]]).clamp(0, 24);
                (0..teams as usize).any(|k| {
                    let n = at + NUMBERS + 2 * k;
                    i16::from_le_bytes([block[n], block[n + 1]]) == club as i16
                })
            });
            let stage = match plays {
                Some(league) => league as u8,
                None if linked[club] < 0 => FINAL_ROUND,
                None => RESERVE,
            };
            block[STAGES_AT + club * CLUB_STAGES + age] = stage;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The youth matchdays from the original's save A (day 63) to the
    /// season's end: every youth league of the five played countries has
    /// played its matchdays as the original's own season end has them, all
    /// results in, and the goals come as often as there — home, away, at
    /// half-time — and the draws.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_youth_seasons_play_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs) = (load("A.an3"), load("SeasonEnd364.an3"));
        let data = crate::data::Data::load(&dir).unwrap();
        let mut random = crate::random::Random::new(11);
        for day in usize::from(ours.globals.day()) + 1..=364 {
            play_day(&mut ours, &data.calendar, day, &mut random);
        }
        let tally = |records: &Records| {
            let (mut played, mut sums) = (Vec::new(), [0u32; 5]);
            for country in crate::opponent::played_nations(records) {
                for league in 0..LEAGUES {
                    for age in 0..AGES {
                        let youth = YouthLeague::of(records, country, league, age).unwrap();
                        if youth.teams() == 0 {
                            continue;
                        }
                        played.push(youth.played());
                        for matchday in 1..=youth.played() {
                            for slot in 0..youth.teams() {
                                let Some(s) = youth.result(matchday, slot) else { continue };
                                let draw = u32::from(s.home == s.away);
                                let half = u32::from(s.half_home + s.half_away);
                                for (k, v) in [1, s.home.into(), s.away.into(), half, draw].into_iter().enumerate() {
                                    sums[k] += v;
                                }
                            }
                        }
                    }
                }
            }
            (played, sums)
        };
        let ((played, ours), (their_played, theirs)) = (tally(&ours), tally(&theirs));
        assert_eq!(played, their_played);
        assert_eq!(ours[0], theirs[0], "results");
        for k in 1..5 {
            let off = (f64::from(ours[k]) / f64::from(theirs[k]) - 1.0).abs();
            assert!(off < 0.05, "{k}: {} against {}", ours[k], theirs[k]);
        }
    }

    /// Over the original's pair of its season's last day and the new
    /// season's first, the replica's youth season end and new season
    /// leave every played country's youth leagues as the original's: the
    /// teams of each league in their new order, the results unplayed, and
    /// each club's stage per age group. (The final round's groups and
    /// flags behind, which its matches write, are not built.)
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_youth_leagues_turn_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut ours, theirs) = (load("SeasonEnd364.an3"), load("NewSeason000.an3"));
        let before = load("SeasonEnd364.an3");
        let countries = crate::opponent::played_nations(&ours);
        assert_eq!(countries.len(), 5);
        for &country in &countries {
            close_season(&mut ours, country);
        }
        for &country in &countries {
            open_season(&mut ours, country);
        }
        let mut moved = 0;
        for &country in &countries {
            let slot = ours.countries.slot(country.into());
            let name = format!("{OBJECT:#x}[{slot}]");
            let (x, y) = (ours.block(&name).unwrap(), theirs.block(&name).unwrap());
            assert_eq!(x[..STAGES_AT], y[..STAGES_AT], "country {country}'s leagues");
            for club in 0..STAGE_CLUBS {
                let at = STAGES_AT + club * CLUB_STAGES;
                assert_eq!(x[at..at + AGES], y[at..at + AGES], "country {country} club {club}");
            }
            for age in 0..AGES {
                let (a, b) = (YouthLeague::of(&ours, country, 0, age).unwrap(), YouthLeague::of(&before, country, 0, age).unwrap());
                moved += (0..a.teams()).filter(|&k| a.number(k) != b.number(k)).count();
            }
        }
        // The season moved teams in every country and age group.
        assert!(moved > 5 * AGES * MOVED, "{moved}");
    }

    #[test]
    fn a_table_sorts_by_points_difference_goals_and_games() {
        let row = |number, points, goals_for, goals_against, games| Row {
            number,
            points,
            goals_for,
            goals_against,
            games,
            ..Row::default()
        };
        let mut rows =
            vec![row(1, 3, 2, 2, 2), row(2, 4, 1, 1, 2), row(3, 3, 3, 1, 2), row(4, 3, 3, 1, 3)];
        sort(&mut rows);
        assert_eq!(rows.iter().map(|r| r.number).collect::<Vec<_>>(), [2, 4, 3, 1]);
    }
}
