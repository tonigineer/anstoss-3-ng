//! A new game's played country opens its clubs' first season
//! (`FUN_00774088` for a country played, in `FUN_0065e659` while the season
//! count stands at one): every club of its leagues in the leagues' order,
//! then every club in none (its level 10), runs the club's season opening
//! (`FUN_00772bee`). In the first season that draws the club's will to win
//! (41, less 20, `FUN_0069efe8`), opens the men of its squad
//! (`FUN_00772799`: their private lives, hobbies, first stations,
//! release fees, characters and languages), names its president
//! (`FUN_0066d30f`, `FUN_0069b9a2`), sets a league club's goals by its
//! squad's rank (`FUN_00775cb7`, a draw in 102), the mood (80 and a draw
//! of 41), a draw in 100 at club +0xdf0 and plans its search
//! (`FUN_006a03fb`).
//!
//! The club's other first-season fields (its class, tactics, respect,
//! the reserves and youth leagues of `FUN_00774088`'s middle) draw nothing
//! and stand in [`crate::settlement`].

use crate::random::Random;
use crate::records::{Player, Records};

/// The hobbies a man is drawn (`DAT_00a325c8`, `FUN_005fd496`: 39 of a
/// weight and the hobby at `0x88e828`); four in five draw one
/// (`FUN_005fed6b`).
const HOBBIES_AT: u32 = 0x88e828;
const HOBBIES: usize = 39;
/// The game text of each hobby (`FUN_007335e0` over `0x8a8c7e`, by the
/// hobby's number; 0 none).
const HOBBY_TEXTS_AT: u32 = 0x8a8c7e;
const HOBBY_TEXTS: usize = 40;
const HOBBY_ODDS: u32 = 80;
/// By his strength, the chance in 100 of the bit at L+0x1c (R+0x1d4,
/// `FUN_006fe877`) and of a release fee at a top league's club, in 200 at
/// the second's (`DAT_008acab0`, `DAT_008acae8`); of a language problem
/// of the first and of the second kind (`DAT_008acb20`, `DAT_008acb58`).
const FRESH_BIT_AT: u32 = 0x8acab0;
const FEE_AT: u32 = 0x8acae8;
const PROBLEM_AT: [u32; 2] = [0x8acb20, 0x8acb58];
const STRENGTHS: usize = 14;
/// The row of first names a nation's men's partners and children take
/// (`DAT_008a7470`), and whether the nation has its own
/// (`DAT_008a74e8`); else his club's country's row.
const NAME_ROWS_AT: u32 = 0x8a7470;
const OWN_NAMES_AT: u32 = 0x8a74e8;
const NATIONS: usize = 0x78;
/// The days of the months a partner's birthday is drawn in
/// (`DAT_008a3088`, `FUN_006759f0`).
const MONTH_DAYS_AT: u32 = 0x8a3088;

/// The tables the opening reads from the program.
pub struct Tables {
    hobbies: Vec<(i16, i16)>,
    hobby_total: u32,
    hobby_texts: Vec<u16>,
    fresh_bit: Vec<u32>,
    fee: Vec<u32>,
    problems: [Vec<u32>; 2],
    name_rows: Vec<u8>,
    own_names: Vec<u8>,
    month_days: Vec<u16>,
}

impl Tables {
    pub fn load(exe: &a3_files::exe::Exe) -> std::io::Result<Self> {
        let pairs = exe.u16s(HOBBIES_AT, 2 * HOBBIES)?;
        let hobbies: Vec<(i16, i16)> = pairs.chunks(2).map(|c| (c[0] as i16, c[1] as i16)).collect();
        // The total is the weights' sum (`FUN_0060bdb7`, its least -1).
        let hobby_total = hobbies.iter().map(|h| i32::from(h.0)).sum::<i32>().max(0) as u32;
        Ok(Self {
            hobbies,
            hobby_total,
            hobby_texts: exe.u16s(HOBBY_TEXTS_AT, HOBBY_TEXTS)?,
            fresh_bit: exe.u32s(FRESH_BIT_AT, STRENGTHS)?,
            fee: exe.u32s(FEE_AT, STRENGTHS)?,
            problems: [exe.u32s(PROBLEM_AT[0], STRENGTHS)?, exe.u32s(PROBLEM_AT[1], STRENGTHS)?],
            name_rows: exe.bytes(NAME_ROWS_AT, NATIONS)?.to_vec(),
            own_names: exe.bytes(OWN_NAMES_AT, 0x80)?.to_vec(),
            month_days: exe.u16s(MONTH_DAYS_AT, 13)?,
        })
    }

    /// A weighted hobby (`FUN_0060be4d` on `DAT_00a325c8`): a draw below
    /// the total walks the weights; past them none.
    pub(crate) fn hobby(&self, random: &mut Random) -> u8 {
        let mut r = random.below(self.hobby_total) as i32;
        for &(weight, hobby) in &self.hobbies {
            if i32::from(weight) > r {
                return hobby as u8;
            }
            r -= i32::from(weight);
        }
        0
    }

    /// A hobby's game text (`FUN_007335e0`): 0, none, for hobby 0.
    pub(crate) fn hobby_text(&self, hobby: u8) -> u16 {
        if hobby == 0 { 0 } else { self.hobby_texts.get(usize::from(hobby)).copied().unwrap_or(0) }
    }

    fn by_strength(table: &[u32], p: &Player) -> u32 {
        table.get(usize::from(p.strength())).copied().unwrap_or(0)
    }
}

/// The partner's kind (R+0x27e's low two bits): a girlfriend, a wife; the
/// state of the two (bits 2 to 4) and the bit a married man's draw of 30
/// in 100 sets (`FUN_006ffd23`).
const PARTNER: usize = 0x27e;
const GIRLFRIEND: u8 = 1;
const WIFE: u8 = 3;
const PARTNER_BIT: u8 = 0x80;
/// The partner's first name (from the row's women, 50 on), her row, her
/// birthday (day and month), the day they came together or married
/// (R+0x16c), the dates the stations of private life pass through
/// (R+0x170), and the children's: four bytes each from R+0x174, their
/// names from R+0x288.
const PARTNER_NAME: usize = 0x283;
const NAMES_ROW: usize = 0x286;
const PARTNER_BIRTHDAY: usize = 0x188;
const TOGETHER_SINCE: usize = 0x16c;
const PASSING: usize = 0x170;
const CHILD_DATES: usize = 0x174;
const CHILD_NAMES: usize = 0x288;
const CHILDREN_MOST: i16 = 5;
/// The bits a man's children leave (`FUN_00701ae6`) and the two that name
/// a German's first son or daughter (Germany's real players).
const CHILD_CLEARS: u32 = 0x0100_0001;
const CHILD_SETS: u32 = 0x0080_0000;
const NAMED_CHILDREN: u32 = 0x1800_0000;
const GERMANY: u8 = 0x0e;
const REAL_PLAYERS: u16 = 0x4269;
/// R+0x1b4's bits the opening may set: overweight, a smoker, a bit with
/// age (0x400), and one with the trait 0x200.
const OVERWEIGHT: u32 = 0x20;
const SMOKES: u32 = 0x800;
const AGE_BIT: u32 = 0x400;
const TRAIT_BIT: u32 = 0x0080_0200;
/// The release fee's unit (`DAT_00a3b3bc`), and the word it stands in
/// (R+0x156, L+0x8e).
const FEE_UNIT: i64 = 10_000;
/// The seasons count (R+0x161, L+0x98), one to three.
const SEASONS: usize = 0x161;
/// The word L+0x1c stands in (R+0x1d4).
const FRESH_WORD: usize = 0x1d4;
/// The languages a man speaks besides, each by its draw in 100: English,
/// French, and two more.
const MORE_LANGUAGES: [(u8, u32); 4] = [(0x10, 95), (0x14, 20), (0x3f, 10), (0x1c, 10)];
/// The global lists of a man by position and by quality (`DAT_00b5dfd0`,
/// `DAT_00b5dff8` for keepers, `DAT_00b5dfe6`).
const HOME_GROWN: usize = 0xb5dfd0;
const KEEPER_QUALITIES: usize = 0xb5dff8;
const QUALITIES: usize = 0xb5dfe6;

/// The club's level a man's release fee is drawn at (club +0x105).
const TOP: u8 = 0;
const SECOND: u8 = 1;
/// A club in no league (`FUN_00774088`'s last loop).
const NO_LEAGUE: u8 = 10;

/// The team fields: the will to win, the board's goals, the mood, the
/// draw at club +0xdf0, the president's names and birthday.
const WILL: usize = 0x95a;
const WILL_DRAW: u32 = 41;
const WILL_LESS: i16 = 20;
const WILL_MOST: i16 = 200;
const GOALS_AT: [usize; 2] = [0x135, 0x136];
const RANK_AT: usize = 0x130;
const FLAGS_AT: usize = 0x110;
const PROMOTED_FLAG: u16 = 4;
const RELEGATED_FLAG: u16 = 8;
const MOOD_BASE: u8 = 80;
const MOOD_DRAW: u32 = 41;
const SEASON_DRAW: usize = 0x302;
const PRESIDENT: usize = 0x8f8;
const PRESIDENT_TRIES: usize = 5;
const PRESIDENT_FROM: u32 = 35;
const PRESIDENT_AGES: u32 = 20;
/// The season's marks the mood takes (`FUN_0069a161` … `FUN_0069a261`):
/// the champion, the runner-up, a UEFA cup entrant, the promoted, the
/// relegated, the first that holds; the cup winner besides.
const STANDINGS: [(u16, i8); 5] = [(1, 30), (2, 20), (0x10, 10), (4, 20), (8, -30)];
const CUP_WON: u16 = 0x1000;

/// The played `country`'s clubs open their first season (`FUN_00774088`).
pub fn open_country(records: &mut Records, rules: &crate::season::Rules, country: u16, random: &mut Random) {
    let tables = &rules.first_season;
    let leagues = records.countries.leagues(usize::from(country)).max(0) as u8;
    let mut in_league = Vec::new();
    for level in 0..leagues {
        let Some(league) = crate::opponent::League::of(records, country, level) else { continue };
        let (size, clubs): (usize, Vec<i16>) = (league.teams(), (0..league.teams()).map(|k| league.team(k)).collect());
        for number in clubs {
            let Ok(number) = u16::try_from(number) else { continue };
            in_league.push(number);
            open_club(records, rules, tables, (country, number), level, Some(size), random);
        }
    }
    for number in 0..records.countries.team_count(usize::from(country)).max(0) as u16 {
        if !in_league.contains(&number) {
            open_club(records, rules, tables, (country, number), NO_LEAGUE, None, random);
        }
    }
}

/// The countries not played (1 to 0x74, `FUN_00676c03` false) open their
/// clubs' first season (`FUN_00774963`): each club, at the first level
/// (club +0x105 nought), by the same opening with no league
/// (`FUN_00772bee(club, 0, 0)`: goals one and one, no president), then
/// its ticket prices' five rows at the start's (`DAT_00a3b41c` 15,
/// `DAT_00a3b414` 25, `DAT_00a3b408` 50, `DAT_00a3b3fc` 100).
const KEPT_COUNTRIES: u16 = 0x75;
const KEPT_PRICES: [(usize, i16); 4] = [(0x49c, 15), (0x4a6, 25), (0x4b0, 50), (0x4ba, 100)];

pub fn open_kept(records: &mut Records, rules: &crate::season::Rules, random: &mut Random) {
    for country in 1..KEPT_COUNTRIES {
        if records.globals.played(country as i16) {
            continue;
        }
        for number in 0..records.countries.team_count(usize::from(country)).max(0) as u16 {
            let Some(team) = records.team_mut(country, number) else { continue };
            team.set_level(TOP);
            open_club(records, rules, &rules.first_season, (country, number), TOP, None, random);
            let Some(team) = records.team_mut(country, number) else { continue };
            for (at, price) in KEPT_PRICES {
                for k in 0..5 {
                    team.put::<i16>(at + crate::finances::CLUB_TO_TEAM + 2 * k, price);
                }
            }
        }
    }
}

/// A club's first season opened (`FUN_00772bee`'s draws and their fields).
fn open_club(
    records: &mut Records,
    rules: &crate::season::Rules,
    tables: &Tables,
    (country, number): (u16, u16),
    level: u8,
    size: Option<usize>,
    random: &mut Random,
) {
    let will = random.below(WILL_DRAW) as i16 - WILL_LESS;
    let Some(team) = records.team_mut(country, number) else { return };
    let w = team.get::<i16>(WILL).saturating_add(will).clamp(-WILL_MOST, WILL_MOST);
    team.put::<i16>(WILL, w);
    let squad = team.players();
    for id in squad {
        open_man(records, rules, tables, (country as u8, number), level, id, random);
    }
    if records.globals.played(country as i16) {
        let names = president_names(records, country, random);
        let date = date_years_ago(records, (random.below(PRESIDENT_AGES) + PRESIDENT_FROM) as i32, random);
        if let Some(team) = records.team_mut(country, number) {
            team.put::<i16>(PRESIDENT, names.0);
            team.put::<i16>(PRESIDENT + 2, names.1);
            for (k, b) in date.iter().enumerate() {
                team.put::<u8>(PRESIDENT + 4 + k, *b);
            }
        }
    }
    let Some(team) = records.team(country, number) else { return };
    let goals = match size {
        Some(size) => rules.season_goals(
            size,
            usize::from(team.get::<u8>(RANK_AT)),
            team.get::<u16>(FLAGS_AT) & PROMOTED_FLAG != 0,
            i32::from(team.board()),
            level == 0,
            team.linked()[0] >= 0,
            random,
        ),
        None => (1, 1),
    };
    let human = team.humans()[0];
    let flags = team.get::<u16>(FLAGS_AT);
    let mut mood = (random.below(MOOD_DRAW) as u8).wrapping_add(MOOD_BASE);
    let mut will = 0;
    if let Some(&(bit, m)) = STANDINGS.iter().find(|s| flags & s.0 != 0) {
        mood = mood.wrapping_add(m as u8);
        // The promoted take ten of will more, the relegated ten less
        // (`FUN_0069efe8(10)`, `FUN_0069efe8(-10)`).
        will = match bit {
            PROMOTED_FLAG => 10,
            RELEGATED_FLAG => -10,
            _ => 0,
        };
    }
    if flags & CUP_WON != 0 {
        mood = mood.wrapping_add(20);
    }
    let draw = random.below(100) as u8;
    let Some(team) = records.team_mut(country, number) else { return };
    team.put::<u8>(GOALS_AT[0], goals.0);
    team.put::<u8>(GOALS_AT[1], goals.1);
    team.set_mood(mood);
    team.put::<i16>(WILL, team.get::<i16>(WILL).saturating_add(will).clamp(-WILL_MOST, WILL_MOST));
    team.put::<u8>(SEASON_DRAW, draw);
    if let Some(m) = usize::from(human).checked_sub(1).and_then(|k| records.managers.get_mut(k)) {
        m.set_goal_class(i16::from(goals.1 as i8));
    }
    // The search planned anew (`FUN_006a01b8`'s budget, then
    // `FUN_006a03fb(club, 0)`).
    crate::search_plan::plan_season(records, random, (country as u8, number as i16));
}

/// The president's first and last name (`FUN_0066d30f`): of two men of
/// the regeneration table's range of the country's class, five tries at
/// most while the first name stays 1 or 2.
fn president_names(records: &Records, country: u16, random: &mut Random) -> (i16, i16) {
    let class = crate::newplayer::COUNTRY_CLASS.get(usize::from(country)).copied().unwrap_or(0) as usize;
    let (from, to) = crate::newplayer::class_range(records, class);
    let table = records.block(crate::newplayer::REGENERATION);
    let id_at = |k: i32| -> i16 {
        table.and_then(|t| t.get(4 * k as usize..4 * k as usize + 2)).map_or(0, |e| i16::from_le_bytes([e[0], e[1]]))
    };
    let (mut first, mut last) = (1, 2);
    for _ in 0..PRESIDENT_TRIES {
        let k = random.below((to - from) as u32) as i32 + i32::from(from);
        let id = id_at(k);
        if id > 0 && let Some(p) = records.player(id as u16) {
            first = p.first_name() as i16;
        }
        let k = random.below((to - from) as u32) as i32 + i32::from(from);
        let id = id_at(k);
        if id > 0 && let Some(p) = records.player(id as u16) {
            last = p.last_name() as i16;
        }
        if first != 1 && first != 2 {
            break;
        }
    }
    (first, last)
}

/// A date `years` ago from today (`FUN_00675923`): day, month, year.
fn date_years_ago(records: &Records, years: i32, random: &mut Random) -> [u8; 4] {
    let (d, m, y) = crate::newplayer::birth_date(records, years, random);
    let y = y.to_le_bytes();
    [d, m, y[0], y[1]]
}

fn put_date(p: &mut Player, at: usize, date: [u8; 4]) {
    for (k, b) in date.iter().enumerate() {
        p.put::<u8>(at + k, *b);
    }
}

fn today(records: &Records) -> [u8; 4] {
    let [day, month, year] = records.globals.date();
    let y = year.to_le_bytes();
    [day as u8, month as u8, y[0], y[1]]
}

/// A squad man opened (`FUN_00772799`'s body).
fn open_man(
    records: &mut Records,
    rules: &crate::season::Rules,
    tables: &Tables,
    club: (u8, u16),
    level: u8,
    id: u16,
    random: &mut Random,
) {
    let Some(p) = records.player(id) else { return };
    let age = crate::free_agents::age(records, p);
    private_life(records, tables, club.0, id, age, random);
    let hobby = if random.below(100) < HOBBY_ODDS { tables.hobby(random) } else { 0 };
    let Some(p) = records.player_mut(id) else { return };
    p.set_hobby(hobby);
    crate::deals::open_station(records, id, (club.0, club.1 as i16), 0);
    let Some(p) = records.player(id) else { return };
    let (position, qualities) = (p.positions()[0], p.qualities());
    if p.manner() & 0x20 != 0 {
        records.globals.put::<u16>(HOME_GROWN + 2 * usize::from(position), id);
    }
    let (list, count) = if position == 1 { (KEEPER_QUALITIES, 9) } else { (QUALITIES, 16) };
    for k in 0..count {
        if qualities & 1 << k != 0 {
            records.globals.put::<u16>(list + 2 * k, id);
        }
    }
    let Some(p) = records.player(id) else { return };
    let fee_chance = Tables::by_strength(&tables.fee, p);
    let fee = match level {
        TOP => {
            let fresh = random.below(100) < Tables::by_strength(&tables.fresh_bit, p);
            let value = market_value(records, rules, id);
            if fresh && let Some(p) = records.player_mut(id) {
                p.put::<u16>(FRESH_WORD, p.get::<u16>(FRESH_WORD) & !3 | 1);
            }
            (random.below(100) < fee_chance).then_some(value)
        }
        SECOND => {
            let value = market_value(records, rules, id);
            (random.below(200) < fee_chance).then_some(value)
        }
        _ => None,
    };
    if let Some(value) = fee {
        let units = value / FEE_UNIT;
        let fee = (i64::from(random.below(21) + 90) * units / 100) as u16;
        if let Some(p) = records.player_mut(id) {
            p.set_fixed_fee(fee);
        }
    }
    let seasons = random.below(3) as u8 + 1;
    let Some(p) = records.player(id) else { return };
    let problem = if random.below(100) < Tables::by_strength(&tables.problems[0], p) {
        Some(1)
    } else if random.below(100) < Tables::by_strength(&tables.problems[1], p) {
        Some(2)
    } else {
        None
    };
    let trait_word = p.traits()[1];
    let mut bits = 0u32;
    if random.below(100) < 2 {
        bits |= OVERWEIGHT;
    }
    // Two draws whose calls only read (`FUN_00701696`, `FUN_0070005e`).
    let _ = random.below(100);
    let _ = random.below(100);
    let smoke_odds = if trait_word & 0x100 != 0 { 15 } else { 3 };
    if random.below(100) < smoke_odds {
        bits |= SMOKES;
    }
    if trait_word & 0x200 != 0 && random.below(100) < 3 {
        bits |= TRAIT_BIT;
    }
    if (random.below(100) as i32) < age / 2 + 10 {
        bits |= AGE_BIT;
    }
    let nation = p.nation();
    let mut languages = vec![club.0, nation];
    for (language, odds) in MORE_LANGUAGES {
        if random.below(100) < odds {
            languages.push(language);
        }
    }
    let mood = random.below(11) as i8 + 5;
    let Some(p) = records.player_mut(id) else { return };
    p.put::<u8>(SEASONS, seasons);
    if let Some(problem) = problem {
        p.set_problems(p.problems() & 0xcf | problem << 4);
    }
    p.set_shown(p.shown() | bits);
    for language in languages {
        crate::newplayer::speaks(p, language);
    }
    p.set_mood(mood);
}

/// His market value (`FUN_00708836`) by his contract's years left.
fn market_value(records: &Records, rules: &crate::season::Rules, id: u16) -> i64 {
    let Some(p) = records.player(id) else { return 0 };
    let years = (i32::from(p.contract() & 0xfff) - i32::from(records.globals.start_year())).max(0);
    let position = p.positions()[0];
    let worth = crate::value::Worth {
        strength: p.strength(),
        position,
        talent: p.talent(),
        age: crate::free_agents::age(records, p),
        good: p.good(),
        character: p.traits(),
        qualities: p.qualities(),
        star: records.globals.stars().get(usize::from(position)) == Some(&p.id()),
        kind_12_matches: p.kind_12_matches(),
    };
    rules.value.market_value(&worth, years, u32::from(records.globals.date()[1]))
}

/// A man's private life (`FUN_007078c5`): one over 20 marries on a draw
/// below six in 100 for each year past it — some years ago, with children
/// by his age, a draw of 30 in 100 for the partner's bit —; one unmarried
/// over 16 has a girlfriend one time in two, since some years.
fn private_life(records: &mut Records, tables: &Tables, club_country: u8, id: u16, age: i32, random: &mut Random) {
    if age > 20 && (random.below(100) as i32) < (age - 20) * 6 {
        new_partner(records, tables, club_country, id, random);
        marry(records, id);
        let date = date_years_ago(records, random.below((age - 20) as u32) as i32 + 1, random);
        let year = records.globals.start_year() as i16;
        let (count, mut born) = if age < 22 {
            (random.below(2), year - 2)
        } else if age < 25 {
            let n = random.below(3);
            (n, year - random.below((age - 22) as u32) as i16 - 3)
        } else {
            let n = random.below(4);
            (n, year - random.below((age - 25) as u32) as i16 - 6)
        };
        if let Some(p) = records.player_mut(id) {
            put_date(p, TOGETHER_SINCE, date);
        }
        for _ in 0..count {
            let sex = random.below(2) as u8;
            child(records, id, sex, random);
            let date = date_years_ago(records, 2, random);
            let Some(p) = records.player_mut(id) else { return };
            let at = CHILD_DATES + 4 * (p.children() - 1).clamp(0, CHILDREN_MOST - 1) as usize;
            put_date(p, at, date);
            p.put::<i16>(at + 2, born);
            born += 2;
        }
        if random.below(100) < 30
            && let Some(p) = records.player_mut(id)
        {
            p.put::<u8>(PARTNER, p.get::<u8>(PARTNER) | PARTNER_BIT);
        }
    }
    let Some(p) = records.player(id) else { return };
    if p.get::<u8>(PARTNER) & 3 != WIFE && age > 16 && random.below(100) < 50 {
        new_partner(records, tables, club_country, id, random);
        let date = date_years_ago(records, random.below((age - 16) as u32) as i32 + 1, random);
        if let Some(p) = records.player_mut(id) {
            put_date(p, TOGETHER_SINCE, date);
        }
    }
}

/// He and his partner part (`FUN_00702467`): she, if any, kept as his ex
/// (her first name at +0x285, her row at +0x287, her birthday at +0x18c)
/// and the children gone. The original also takes her dates off the
/// private lives' list (`FUN_004377b7`), which the replica leaves out.
pub(crate) fn part(records: &mut Records, id: u16) {
    let Some(p) = records.player_mut(id) else { return };
    if p.get::<u8>(PARTNER) & 3 != 0 {
        p.put::<u8>(0x285, p.get::<u8>(PARTNER_NAME));
        p.put::<u8>(0x287, p.get::<u8>(NAMES_ROW));
        p.put::<u32>(0x18c, p.get::<u32>(PARTNER_BIRTHDAY));
        p.set_shown(p.shown() & 0xfcff_fff3);
    }
    p.put::<u8>(PARTNER, p.get::<u8>(PARTNER) & 0xfc);
    p.set_children(0);
    p.put::<u8>(PASSING, 0);
}

/// A new girlfriend (`FUN_00702571`): the one before, if any, kept as his
/// ex ([`part`]) and the children gone; her first name drawn from
/// the women of his names' row, her birthday's day and month drawn.
pub(crate) fn new_partner(records: &mut Records, tables: &Tables, club_country: u8, id: u16, random: &mut Random) {
    part(records, id);
    let Some(p) = records.player_mut(id) else { return };
    let name = random.below(50) as u8 + 50;
    let nation = usize::from(p.nation() & 0x7f);
    let row_of = if tables.own_names.get(nation).copied().unwrap_or(0) == 0 { usize::from(club_country) } else { nation };
    let row = tables.name_rows.get(row_of).copied().unwrap_or(0);
    let month = random.below(12) as u8 + 1;
    let days = tables.month_days.get(usize::from(month)).copied().unwrap_or(30);
    let day = random.below(u32::from(days)) as u8 + 1;
    p.put::<u8>(PARTNER_NAME, name);
    p.put::<u8>(NAMES_ROW, row);
    p.put::<u8>(PARTNER_BIRTHDAY, day);
    p.put::<u8>(PARTNER_BIRTHDAY + 1, month);
    p.put::<u8>(PARTNER, p.get::<u8>(PARTNER) & 0xe0 | GIRLFRIEND);
    p.put::<u8>(PASSING, 0);
}

/// She becomes his wife (`FUN_00702b15`): the passing date kept as the
/// day they married. The original also takes the booked wedding day out
/// of the private list (`FUN_004377b7`); the day walk that calls this
/// unbooks the entry itself, `bookings::unbook`.
pub(crate) fn marry(records: &mut Records, id: u16) {
    let Some(p) = records.player_mut(id) else { return };
    p.put::<u8>(PARTNER, p.get::<u8>(PARTNER) & 0xe0 | WIFE);
    let passing = p.get::<u32>(PASSING);
    p.put::<u32>(TOGETHER_SINCE, passing);
    p.put::<u8>(PASSING, 0);
}

/// A child more (`FUN_00701ae6`, `sex` 0 a son): a German real player's
/// first son or daughter takes the name his bits hold, any other a first
/// name of the row's men or women no brother or sister has; born today
/// for now.
fn child(records: &mut Records, id: u16, sex: u8, random: &mut Random) {
    let today = today(records);
    let Some(p) = records.player_mut(id) else { return };
    let count = p.children().clamp(0, CHILDREN_MOST - 1) as usize;
    let names: Vec<u8> = (0..count).map(|k| p.get::<u8>(CHILD_NAMES + k)).collect();
    let mut name = None;
    if p.nation() == GERMANY {
        if id < REAL_PLAYERS {
            let shown = p.shown();
            if shown & 0x0800_0000 != 0 && sex == 0 {
                name = Some(0x29);
            } else if shown & 0x1000_0000 != 0 && sex == 1 {
                name = Some(0x4f);
            }
        }
        if name.is_some_and(|n| names.contains(&n)) {
            name = None;
        }
        p.set_shown(p.shown() & !NAMED_CHILDREN);
    }
    let name = name.unwrap_or_else(|| loop {
        let n = random.below(50) as u8 + sex * 50;
        if !names.contains(&n) {
            break n;
        }
    });
    p.put::<u8>(CHILD_NAMES + count, name);
    put_date(p, CHILD_DATES + 4 * count, today);
    p.set_children(count as i16 + 1);
    p.set_shown(p.shown() & !CHILD_CLEARS | CHILD_SETS);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The played country's clubs open their first season as the
    /// original's (the oracle's new game, its first-turn dump): from the
    /// generator as it stood before `FUN_00774088` there (trace index
    /// 52703), its 108 clubs' 67,522 draws (67,278 that move the generator)
    /// lead to the state before `FUN_00774963`, and every man and club
    /// holds the dump's fields — his seasons count one less, which a later
    /// walk raises.
    #[test]
    #[ignore = "reads the oracle's dumps from $ANSTOSS3_ORACLE and an installed game from $ANSTOSS3_DIR"]
    fn a_new_game_opens_the_played_clubs_as_the_original() {
        let (Some(oracle), Some(dir)) = (std::env::var_os("ANSTOSS3_ORACLE"), std::env::var_os("ANSTOSS3_DIR")) else {
            eprintln!("no oracle dump of a new game's first turn among the saves: ANSTOSS3_ORACLE or ANSTOSS3_DIR is not set");
            return;
        };
        let path = std::path::PathBuf::from(oracle).join("runs/new-game/first-turn.an3");
        if !path.exists() {
            eprintln!("no oracle dump of a new game's first turn among the saves");
            return;
        }
        let exe = a3_files::exe::Exe::open(&std::path::PathBuf::from(dir).join("anstoss3.exe")).unwrap();
        let rules = crate::season::Rules::load(&exe).unwrap();
        let dump = a3_files::an3::Save::read(&path).unwrap();
        let (mut ours, theirs) = (Records::from_save(dump.clone()).unwrap(), Records::from_save(dump).unwrap());
        let played = crate::opponent::played_nations(&ours);
        assert_eq!(played, vec![14]);
        // The men stand as the country files gave them: no partner, no
        // children, the station at their club not opened yet.
        let men: Vec<u16> = (0..ours.countries.team_count(14).max(0) as u16)
            .flat_map(|n| ours.team(14, n).map(|t| t.players()).unwrap_or_default())
            .collect();
        for &id in &men {
            let p = ours.player_mut(id).unwrap();
            p.put::<u8>(PARTNER, 0);
            p.set_children(0);
            p.set_career_count(p.career_count() - 1);
        }
        // The regeneration table's entries of the country's class (id
        // k + 1 at k) the later walks took for new amateurs and youths
        // stand free again; a man of a squad was never in it.
        let in_squads: std::collections::HashSet<u16> = (1..0x75usize)
            .flat_map(|c| (0..ours.countries.team_count(c).max(0) as u16).map(move |n| (c as u16, n)))
            .flat_map(|(c, n)| ours.team(c, n).map(|t| t.players()).unwrap_or_default())
            .collect();
        let (from, to) = crate::newplayer::class_range(&ours, usize::from(crate::newplayer::COUNTRY_CLASS[14]));
        let table = ours.block_mut(crate::newplayer::REGENERATION).unwrap();
        for k in from as usize..to as usize {
            let id = (k + 1) as u16;
            if i16::from_le_bytes([table[4 * k], table[4 * k + 1]]) <= 0 && !in_squads.contains(&id) {
                table[4 * k..4 * k + 2].copy_from_slice(&id.to_le_bytes());
            }
        }
        // The search plans' budgets stand at their base again (the week's
        // refill after set them anew from it), and the UEFA cup's entrants
        // are not marked yet (the European cups' entry does it later).
        for n in 0..ours.countries.team_count(14).max(0) as u16 {
            let t = ours.team_mut(14, n).unwrap();
            let base = t.get::<i32>(0x34c + 0xc);
            t.put::<i32>(0x34c, base);
            t.put::<u16>(FLAGS_AT, t.get::<u16>(FLAGS_AT) & !0x10);
        }
        crate::random::record(true);
        let mut random = Random::from_state(13_345_274, 1_134_676_826);
        open_country(&mut ours, &rules, 14, &mut random);
        let draws = crate::random::recorded();
        crate::random::record(false);
        if let Some(out) = std::env::var_os("A3_DRAWS_OUT") {
            std::fs::write(out, crate::random::draws_text(&draws)).unwrap();
        }
        assert_eq!(random, Random::from_state(1_382_845_047, 1_020_738_033), "{} draws", draws.len());
        let mut off = Vec::new();
        for &id in &men {
            let (a, b) = (ours.player(id).unwrap(), theirs.player(id).unwrap());
            let fields = |p: &Player, later: u8| {
                (
                    p.hobby(),
                    p.get::<u8>(PARTNER),
                    p.get::<u8>(PARTNER_NAME),
                    p.get::<u8>(NAMES_ROW),
                    p.children(),
                    p.children_names(),
                    p.get::<u32>(TOGETHER_SINCE),
                    p.get::<u8>(SEASONS) + later,
                    p.mood(),
                    p.fixed_fee(),
                    p.career_count(),
                )
            };
            if fields(a, 1) != fields(b, 0) {
                off.push(format!("{id}: {:?} {:?}", fields(a, 1), fields(b, 0)));
            }
        }
        for number in 0..ours.countries.team_count(14).max(0) as u16 {
            let (a, b) = (ours.team(14, number).unwrap(), theirs.team(14, number).unwrap());
            let fields = |t: &crate::records::Team| {
                (t.get::<u8>(GOALS_AT[0]), t.get::<u8>(GOALS_AT[1]), t.mood(), t.get::<u8>(SEASON_DRAW), t.get::<[u8; 8]>(PRESIDENT))
            };
            if fields(a) != fields(b) {
                off.push(format!("club {number}: {:?} {:?}", fields(a), fields(b)));
            }
        }
        assert!(off.is_empty(), "{} off:\n{}", off.len(), off[..off.len().min(30)].join("\n"));
    }

    /// The countries not played open their clubs' first season as the
    /// original's: from the generator as it stood before `FUN_00774963`
    /// (trace index 120225), their clubs' 169,849 draws lead to the state
    /// before `FUN_0077650c`, and every man and club holds the dump's
    /// fields.
    #[test]
    #[ignore = "reads the oracle's dumps from $ANSTOSS3_ORACLE and an installed game from $ANSTOSS3_DIR"]
    fn a_new_game_opens_the_kept_clubs_as_the_original() {
        let (Some(oracle), Some(dir)) = (std::env::var_os("ANSTOSS3_ORACLE"), std::env::var_os("ANSTOSS3_DIR")) else {
            eprintln!("no oracle dump of a new game's first turn among the saves: ANSTOSS3_ORACLE or ANSTOSS3_DIR is not set");
            return;
        };
        let path = std::path::PathBuf::from(oracle).join("runs/new-game/first-turn.an3");
        if !path.exists() {
            eprintln!("no oracle dump of a new game's first turn among the saves");
            return;
        }
        let exe = a3_files::exe::Exe::open(&std::path::PathBuf::from(dir).join("anstoss3.exe")).unwrap();
        let rules = crate::season::Rules::load(&exe).unwrap();
        let dump = a3_files::an3::Save::read(&path).unwrap();
        let (mut ours, theirs) = (Records::from_save(dump.clone()).unwrap(), Records::from_save(dump).unwrap());
        let clubs: Vec<(u16, u16)> = (1..KEPT_COUNTRIES)
            .filter(|&c| !ours.globals.played(c as i16))
            .flat_map(|c| (0..ours.countries.team_count(usize::from(c)).max(0) as u16).map(move |n| (c, n)))
            .collect();
        let men: Vec<u16> = clubs.iter().flat_map(|&(c, n)| ours.team(c, n).map(|t| t.players()).unwrap_or_default()).collect();
        for &id in &men {
            let p = ours.player_mut(id).unwrap();
            p.put::<u8>(PARTNER, 0);
            p.set_children(0);
            p.set_career_count(p.career_count() - 1);
        }
        for &(c, n) in &clubs {
            let t = ours.team_mut(c, n).unwrap();
            let base = t.get::<i32>(0x34c + 0xc);
            t.put::<i32>(0x34c, base);
            // The will to win stood at nought before its draw.
            t.put::<i16>(WILL, 0);
        }
        crate::random::record(true);
        let mut random = Random::from_state(1_382_845_047, 1_020_738_033);
        open_kept(&mut ours, &rules, &mut random);
        let draws = crate::random::recorded();
        crate::random::record(false);
        if let Some(out) = std::env::var_os("A3_DRAWS_OUT") {
            std::fs::write(out, crate::random::draws_text(&draws)).unwrap();
        }
        assert_eq!(random, Random::from_state(1_455_217_461, 1_094_981_282), "{} clubs, {} men, {} draws", clubs.len(), men.len(), draws.len());
        let mut off = Vec::new();
        for &id in &men {
            let (a, b) = (ours.player(id).unwrap(), theirs.player(id).unwrap());
            let fields = |p: &Player| {
                (
                    p.hobby(),
                    p.get::<u8>(PARTNER),
                    p.get::<u8>(PARTNER_NAME),
                    p.get::<u8>(NAMES_ROW),
                    p.children(),
                    p.children_names(),
                    p.get::<u32>(TOGETHER_SINCE),
                    p.get::<u8>(SEASONS),
                    p.mood(),
                    p.fixed_fee(),
                    p.career_count(),
                )
            };
            if fields(a) != fields(b) {
                off.push(format!("{id}: {:?} {:?}", fields(a), fields(b)));
            }
        }
        for &(c, n) in &clubs {
            let (a, b) = (ours.team(c, n).unwrap(), theirs.team(c, n).unwrap());
            let fields = |t: &crate::records::Team| {
                (t.level(), t.get::<u8>(GOALS_AT[0]), t.get::<u8>(GOALS_AT[1]), t.mood(), t.get::<u8>(SEASON_DRAW), t.get::<i16>(WILL))
            };
            if fields(a) != fields(b) {
                off.push(format!("club {c}/{n}: {:?} {:?}", fields(a), fields(b)));
            }
        }
        assert!(off.is_empty(), "{} off:\n{}", off.len(), off[..off.len().min(30)].join("\n"));
    }

    /// The kept clubs of the played country renew their squads and fill
    /// them as the original's: from the generator as it stood before
    /// `FUN_0077650c` (trace index 290074), its 455,954 draws — every
    /// club's two new youths (`FUN_0066bb26`) and its spare amateur —
    /// lead to the state at trace index 746028, and the new men hold the
    /// dump's fields. The pools stood empty and the taken regeneration
    /// table entries stood free; an entry held its player's id — the
    /// country's own numbered, the foreign ones the file's — and a
    /// position byte, the drawn one where the pick's probes matched.
    #[test]
    #[ignore = "reads the oracle's dumps from $ANSTOSS3_ORACLE and an installed game from $ANSTOSS3_DIR"]
    fn a_new_game_renews_the_played_clubs_squads_as_the_original() {
        let (Some(oracle), Some(dir)) = (std::env::var_os("ANSTOSS3_ORACLE"), std::env::var_os("ANSTOSS3_DIR")) else {
            eprintln!("no oracle dump of a new game's first turn among the saves: ANSTOSS3_ORACLE or ANSTOSS3_DIR is not set");
            return;
        };
        let path = std::path::PathBuf::from(&oracle).join("runs/new-game/first-turn.an3");
        if !path.exists() {
            eprintln!("no oracle dump of a new game's first turn among the saves");
            return;
        }
        let exe = a3_files::exe::Exe::open(&std::path::PathBuf::from(dir).join("anstoss3.exe")).unwrap();
        let rules = crate::season::Rules::load(&exe).unwrap();
        let dump = a3_files::an3::Save::read(&path).unwrap();
        let (mut ours, theirs) = (Records::from_save(dump.clone()).unwrap(), Records::from_save(dump).unwrap());
        let early_path = std::path::PathBuf::from(&oracle).join("runs/new-game/day-000.an3");
        let early = early_path.exists().then(|| Records::from_save(a3_files::an3::Save::read(&early_path).unwrap()).unwrap());
        let days: Vec<Records> = (0..8)
            .map(|d| {
                let p = std::path::PathBuf::from(&oracle)
                    .join(format!("runs/new-game/day-{d:03}.an3"));
                if !p.exists() {
                    return None;
                }
                Records::from_save(a3_files::an3::Save::read(&p).unwrap()).ok()
            })
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default();
        assert_eq!(crate::opponent::played_nations(&ours), vec![14]);
        let clubs: Vec<u16> = (0..ours.countries.team_count(14).max(0) as u16).collect();
        let (from, to) = crate::newplayer::class_range(&ours, usize::from(crate::newplayer::COUNTRY_CLASS[14]));
        let (from, to) = (from.max(0) as usize, to.max(0) as usize);
        let table = ours.block(crate::newplayer::REGENERATION).unwrap().to_vec();
        let entry = |k: usize| i16::from_le_bytes([table[4 * k], table[4 * k + 1]]);
        // The new men: the country's class entries the pick took, and the
        // amateurs the pools hold — the renewing made all of them.
        let pool: Vec<u16> = clubs
            .iter()
            .flat_map(|&n| {
                let [up, amateurs] = crate::youth::pool(&ours, 14, n);
                up.into_iter().chain(amateurs)
            })
            .collect();
        let mut new: Vec<u16> = (from..to).filter(|&k| entry(k) <= 0).map(|k| (k + 1) as u16).collect();
        for &id in &pool {
            if !new.contains(&id) {
                new.push(id);
            }
        }
        // The taken entries stand free again, id and position byte as the
        // pick found them. Where the probes matched, the entry's position
        // is the drawn one the new man's record carries first; the places
        // the pick reached without any probe matching — after probing the
        // whole class twice, taking the first free entry in its range —
        // held a position none of its probes ever drew, so the replay of
        // the log keeps those bytes. The foreign entries hold the file's
        // own ids, not the place's number.
        const TAKEN_BLIND: [(usize, u8); 72] = [
            (0, 1), (1, 2), (2, 2), (3, 2), (4, 2), (5, 2), (6, 2), (7, 2), (8, 2), (9, 2), (10, 2), (11, 2),
            (12, 2), (13, 2), (14, 2), (15, 2), (16, 2), (17, 2), (18, 2), (19, 2), (20, 2), (21, 2), (22, 2),
            (23, 2), (24, 2), (25, 2), (26, 2), (27, 2), (28, 2), (29, 2), (30, 2), (31, 2), (32, 2), (33, 2),
            (34, 2), (35, 2), (36, 2), (37, 2), (38, 2), (39, 2), (40, 2), (41, 2), (42, 2), (43, 2), (44, 2),
            (45, 2), (46, 2), (47, 2), (48, 2), (49, 2), (50, 2), (51, 2), (52, 2), (53, 2), (54, 2), (55, 2),
            (56, 2), (57, 2), (58, 2), (59, 2), (60, 2), (61, 2), (62, 2), (63, 2), (64, 2), (65, 2), (66, 4),
            (67, 2), (68, 2), (69, 2), (70, 2), (1814, 1),
        ];
        const TAKEN_ABROAD: [(usize, u16); 23] = [
            (1023, 3904), (1099, 3980), (1164, 4045), (1814, 4902), (1891, 4979), (1971, 5257), (2417, 5911),
            (2437, 5931), (2704, 6401), (3250, 7377), (3447, 7574), (3493, 7844), (4397, 13967), (4398, 13968),
            (4418, 13988), (4432, 14002), (4455, 14025), (4460, 14030), (4484, 14054), (4791, 14361),
            (4810, 14380), (5214, 14784), (5267, 14837),
        ];
        let blind = |k: usize| TAKEN_BLIND.iter().find(|&&(place, _)| place == k).map(|&(_, p)| p);
        let free: Vec<(usize, u8)> = (from..to)
            .filter(|&k| entry(k) <= 0)
            .map(|k| {
                let p = blind(k).or_else(|| ours.player((k + 1) as u16).map(|p| p.positions()[0]));
                (k, p.expect("a new man or a kept position byte"))
            })
            .collect();
        let abroad: Vec<(usize, u8, u16, u8)> = TAKEN_ABROAD
            .iter()
            .map(|&(k, id)| {
                let p = ours.player(id).expect("the pool amateur");
                let class = usize::from(crate::newplayer::COUNTRY_CLASS[usize::from(p.nation())]);
                (k, class as u8, id, blind(k).unwrap_or(p.positions()[0]))
            })
            .collect();
        assert_eq!(free.len() + abroad.len(), new.len(), "every new man is a taken entry");
        let table = ours.block_mut(crate::newplayer::REGENERATION).unwrap();
        for &(k, position) in &free {
            let id = (k + 1) as u16;
            table[4 * k..4 * k + 2].copy_from_slice(&id.to_le_bytes());
            table[4 * k + 2] = 0;
            table[4 * k + 3] = position;
        }
        for &(k, class, id, position) in &abroad {
            assert!(entry(k) <= 0, "the entry at {k} stood taken");
            table[4 * k..4 * k + 2].copy_from_slice(&id.to_le_bytes());
            table[4 * k + 2] = class;
            table[4 * k + 3] = position;
        }
        // The pools stood empty before the renewing filled them.
        for &n in &clubs {
            crate::youth::store_pool(&mut ours, 14, n, &[Vec::new(), Vec::new()]);
        }
        // The contracts stood as the first contracts wrote them: the walks
        // before this one carry 23 men's word past the first year, and the
        // worth below is weighed off the first year the club walks left on
        // the man (`FUN_00708836` reads his object, not the renewed word).
        let mut contracts = Random::from_state(239_039_057, 876_019_106);
        crate::rollover::first_contracts(&mut ours, (&rules.contract, &rules.value), &mut contracts);
        crate::random::record(true);
        let mut random = Random::from_state(1_455_217_461, 1_094_981_282);
        crate::rollover::open_played(&mut ours, Some((&rules.contract, &rules.value)), &mut random);
        crate::season_odds::open_lists(&mut ours, &mut random);
        crate::rollover::open_kept(&mut ours, Some((&rules.contract, &rules.value)), &mut random);
        let draws = crate::random::recorded();
        crate::random::record(false);
        if let Some(out) = std::env::var_os("A3_DRAWS_OUT") {
            std::fs::write(out, crate::random::draws_text(&draws)).unwrap();
        }
        assert_eq!(random, Random::from_state(1_168_654_908, 1_284_470_449), "{} new men, {} draws", new.len(), draws.len());
        let mut off = Vec::new();
        for &id in &new {
            let (a, b) = (ours.player(id).unwrap(), theirs.player(id).unwrap());
            let made = |p: &Player| {
                (
                    p.nation(),
                    p.birth_day(),
                    p.birth_month(),
                    p.birth_year(),
                    p.strength(),
                    p.talent(),
                    p.positions(),
                    p.retiring(),
                    p.picks(),
                )
            };
            let spare = |p: &Player| {
                (
                    p.club(),
                    p.get::<u16>(0x216),
                    p.get::<u8>(0x353),
                    p.manner(),
                    p.shown(),
                    p.condition(),
                    p.freshness(),
                    p.contentment(),
                )
            };
            if (made(a), spare(a)) != (made(b), spare(b)) {
                off.push(format!("{id}: {:?} {:?} vs {:?} {:?}", made(a), spare(a), made(b), spare(b)));
            }
        }
        for &n in &clubs {
            let (a, b) = (ours.team(14, n).unwrap(), theirs.team(14, n).unwrap());
            if a.players() != b.players() {
                let (a, b) = (a.players(), b.players());
                let gone: Vec<u16> = b.iter().filter(|p| !a.contains(p)).copied().collect();
                let put: Vec<u16> = a.iter().filter(|p| !b.contains(p)).copied().collect();
                off.push(format!("club {n}: squad {a:?} vs {b:?} (missing {gone:?}, extra {put:?})"));
            }
            if a.overdraft() != b.overdraft() {
                off.push(format!("club {n}: overdraft {} vs {}", a.overdraft(), b.overdraft()));
            }
            let (a, b) = (crate::youth::pool(&ours, 14, n), crate::youth::pool(&theirs, 14, n));
            if a != b {
                off.push(format!("club {n}: pool {a:?} vs {b:?}"));
            }
        }
        assert!(off.is_empty(), "{} off:\n{}", off.len(), off.join("\n"));
    }

    /// The rest of `FUN_0065e659` in the first season: from the generator
    /// as it stood when the staff markets had their companies (trace index
    /// 746028), the two company builds (694), the national odds (323), the
    /// luck draws (30), the merchandises' weeks (62), the lawyers' swapped
    /// rank (1), the national teams (1,392) and the market's opening with
    /// its 196 rows and fifty-two weekly prices (21,694) lead to the state
    /// at 770224 — 24,196 draws — and the market's rows stand as the
    /// dump's (`FUN_00409a3f`, `FUN_0077713a`, `FUN_00613f70`,
    /// `FUN_006b93f2`, `FUN_00405f2f` with `FUN_00404dcd`, `FUN_00404ed8`
    /// and `FUN_00405024`).
    #[test]
    #[ignore = "reads the oracle's dumps from $ANSTOSS3_ORACLE and an installed game from $ANSTOSS3_DIR"]
    fn a_new_game_opens_the_market_and_the_season_as_the_originals() {
        let (Some(oracle), Some(dir)) = (std::env::var_os("ANSTOSS3_ORACLE"), std::env::var_os("ANSTOSS3_DIR")) else {
            eprintln!("no oracle dump of a new game's first turn among the saves: ANSTOSS3_ORACLE or ANSTOSS3_DIR is not set");
            return;
        };
        let path = std::path::PathBuf::from(oracle).join("runs/new-game/first-turn.an3");
        if !path.exists() {
            eprintln!("no oracle dump of a new game's first turn among the saves");
            return;
        }
        let exe = a3_files::exe::Exe::open(&std::path::PathBuf::from(dir).join("anstoss3.exe")).unwrap();
        let rules = crate::season::Rules::load(&exe).unwrap();
        let dump = a3_files::an3::Save::read(&path).unwrap();
        let (mut ours, theirs) = (Records::from_save(dump.clone()).unwrap(), Records::from_save(dump).unwrap());
        // The dump keeps what the opening already did: at 746028 no
        // co-trainer had asked years (byte 0x15) or named a salary (0x10),
        // and at 746722 no national squad had been picked yet (the count
        // at 0x112). Stand those empty again before replaying — the man's
        // own years (0x0c) stood as the dump keeps them already.
        for id in 1..1500 {
            if let Some(b) = ours.block_mut(&format!("0xbe3920[{id}]")) {
                b[0x10..0x14].fill(0);
                b[0x15] = 0;
            }
        }
        for nation in 1..crate::season_odds::NATIONS {
            let at = crate::season_odds::NATIONAL_COUNT;
            if let Some(b) = ours.block_mut(&format!("{:#x}[{nation}]", crate::season_odds::NATIONAL_TEAMS)) {
                b[at..at + 2].fill(0);
            }
        }
        crate::random::record(true);
        let mut random = Random::from_state(1_168_654_908, 1_284_470_449);
        for _ in 0..2 {
            crate::staff_market::build_co_markets(&mut ours, &mut random);
        }
        crate::season_odds::nation_odds(&mut ours, &rules.pitch.european, &mut random);
        crate::statistics::draw_luck(&mut ours, &mut random);
        crate::merchandise::open_weeks(&mut ours, &rules.merchandise, &mut random);
        crate::court::open_ranks(&mut ours, &mut random);
        crate::season_odds::open_national_teams(&mut ours, &mut random);
        crate::stock::open(&mut ours, &mut random);
        let draws = crate::random::recorded();
        crate::random::record(false);
        if let Some(out) = std::env::var_os("A3_DRAWS_OUT") {
            std::fs::write(out, crate::random::draws_text(&draws)).unwrap();
        }
        assert_eq!(random, Random::from_state(2_063_601_799, 1_799_675_265), "{} draws", draws.len());
        let rows = |b: &[u8]| -> Vec<(i16, i16, Vec<u8>)> {
            let count = i32::from_le_bytes(b[0..4].try_into().unwrap()).max(0) as usize;
            (0..count)
                .map(|k| {
                    let e = 0x14 + k * 0x108;
                    (
                        i16::from_le_bytes([b[e + 0x1c], b[e + 0x1d]]),
                        i16::from_le_bytes([b[e + 0x1e], b[e + 0x1f]]),
                        b[e..e + 0x16].iter().copied().take_while(|&c| c != 0).collect(),
                    )
                })
                .collect()
        };
        let (a, b) = (ours.block("0x8be408").unwrap(), theirs.block("0x8be408").unwrap());
        let (a, b) = (rows(a), rows(b));
        assert_eq!(a.len(), b.len(), "the market's row count");
        // In order: the clubs first, then the index names by table place,
        // the dump's rows stand in the very order the opening put them in.
        let off: Vec<String> = (0..a.len().max(b.len()))
            .filter(|&k| a.get(k) != b.get(k))
            .map(|k| format!("{k}: {:?} vs {:?}", a.get(k), b.get(k)))
            .collect();
        assert!(off.is_empty(), "{} rows off:\n{}", off.len(), off.join("\n"));
    }

}
