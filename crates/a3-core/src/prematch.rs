//! The last information before the match (dialog 96, `CInfoseiteHaupt`):
//! a page (dialog 98, `CInfoseiteSpiel`, filled by `FUN_004ccc26`) for
//! each of the day's matches a human's club plays, as they stand before a
//! ball is kicked — both clubs, their last results and places, the first
//! leg, last season's game, the home side's record against the other, the
//! strengths, the ground, the crowd and the visiting fans.
//!
//! The day's live flag (`DAT_01bad05c`, set in `FUN_006ef296`) is up when
//! a match of the day has a human's club in it (+0x474) and the day is the
//! season's 42nd or later, or the preparation is the full one
//! (`FUN_006ef262`); then `FUN_006f38b0` posts event 0x74 and
//! `FUN_004cb878` opens 96, which closes again at once without a page
//! (`FUN_004cb9e5`). The switches "Infoscreen für Länderspiele" (byte 0 of
//! the options at `0xb5d13c`) and "Reine Computerspieltage anzeigen"
//! (byte 1) raise the flag too, the one for a national team's day, the
//! other for a day of computer clubs only — neither brings a page.

use crate::autolineup;
use crate::data::Data;
use crate::matches::{Faced, Fixture};
use crate::opponent::{self, League};
use crate::records::{Records, Team};
use crate::season::Season;
use crate::stadium;

/// The most pages: the dialog's four slots (+0xb70), the 0x8e0 bytes of
/// four 0x238 entries at `0x1bdd7d8` (`FUN_006fa448`).
pub const PAGES: usize = 4;
/// From this day of the season the flag follows every human's match;
/// before it only with the preparation normal (5) or "nur Pflichtspiele"
/// (12) (`FUN_006ef262`, `DAT_00b5d152`).
const LIVE_FROM: usize = 42;
const FULL_PREPARATIONS: [u8; 2] = [5, 12];

/// Whether a day's human matches raise the live flag (`FUN_006ef262`).
pub fn live(records: &Records, day: usize) -> bool {
    day >= LIVE_FROM || FULL_PREPARATIONS.contains(&records.globals.preparation())
}

/// A match's page, as it stands before the match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    /// Home side first, by country and number.
    pub sides: [(u8, i16); 2],
    pub fixture: Fixture,
    /// Each side's last results (`FUN_0072c2bb`) and its place in the
    /// table (club +0x109), for a league match.
    pub forms: [String; 2],
    pub places: [u8; 2],
    /// "Hinspiel": the league's meeting this season (`FUN_0040d972`), or
    /// a national cup's first leg as it was played (`FUN_0040dac1`, its
    /// two goals shown the other way round), home side's goals first.
    pub first_leg: Option<(u8, u8)>,
    /// "Letztes Spiel": last season's home game (`FUN_0040d7eb`) — with
    /// the first leg's goals where both stand, as the original reads the
    /// two into the same words.
    pub last: Option<(u8, u8)>,
    /// "Tendenz": the home side's record against the other, won, drawn
    /// and lost (`FUN_0040d3a0`), for a league match.
    pub tendency: Option<[u32; 3]>,
    pub strengths: [i32; 2],
    /// The home side's ground (`FUN_00638335` of the record's +0x28, the
    /// home club): its name, how many it holds (+0x3c) and its pitch's
    /// wear (+0x68).
    pub ground: Option<(String, i32, u8)>,
    /// The referee (+0x44e, `FUN_006cdbb9` finding the one the schedule
    /// gave the match): the one the human side's club holds for its next
    /// match (the club object's +0xb50), 0 none.
    pub referee: i16,
    /// The crowd (+0x34) and the visiting fans (+0x54) of the match.
    pub attendance: i32,
    pub fans: Option<i32>,
}

/// The pages of the day's matches with a human's club, the first four,
/// before they are played ([`crate::matches::human_fixtures`]); the crowd
/// follows with [`crowds`] once the replica has played them.
pub fn pages(records: &Records, data: &Data, season: &Season, fixtures: &[([(u8, i16); 2], Fixture)]) -> Vec<Page> {
    fixtures.iter().take(PAGES).filter_map(|&(sides, fixture)| page(records, data, season, sides, fixture)).collect()
}

fn page(records: &Records, data: &Data, season: &Season, sides: [(u8, i16); 2], fixture: Fixture) -> Option<Page> {
    let team = |(country, number): (u8, i16)| records.team(country.into(), u16::try_from(number).ok()?);
    let (home, away) = (team(sides[0])?, team(sides[1])?);
    let calendar = &data.calendar;
    let mut page = Page {
        sides,
        fixture,
        forms: Default::default(),
        places: [0; 2],
        first_leg: None,
        last: None,
        tendency: None,
        strengths: [home, away].map(|t| strength(records, data, Some(season), t)),
        ground: None,
        referee: crate::report::human_side(records, sides).and_then(team).map_or(0, Team::referee),
        attendance: 0,
        fans: None,
    };
    match fixture {
        Fixture::League { .. } => {
            for (k, t) in [home, away].into_iter().enumerate() {
                page.forms[k] = opponent::stand(records, t).map(|s| s.form).unwrap_or_default();
                page.places[k] = t.table_place();
            }
            page.first_leg = opponent::first_leg(records, calendar, home, away);
            page.last = opponent::archived_result(records, home, away, 0, false).map(|last| page.first_leg.unwrap_or(last));
            page.tendency = Some(opponent::home_record(records, calendar, home, away));
        }
        Fixture::Cup { .. } => {
            // The tie's first leg as the side that is a human's club finds
            // it, the side at home now first: shown as it was played.
            let human = crate::report::human_side(records, sides)?;
            let (own, other) = if human == sides[0] { (home, away) } else { (away, home) };
            let tickets = &data.rules.forecast.tickets;
            page.first_leg = crate::tickets::cup_first_leg(records, own, other, tickets, calendar).map(|(h, a)| (a, h));
        }
    }
    let (country, number) = (home.country(), u16::from(home.number()));
    page.ground = opponent::stadium(records, country.into(), number).map(|s| {
        let wear = stadium::subs(records, country, number).and_then(|subs| subs.first().map(stadium::Sub::wear));
        (s.name.clone(), s.capacity, wear.unwrap_or(0))
    });
    Some(page)
}

/// The crowd and the visiting fans of the matches played, onto their
/// pages.
pub fn crowds(pages: &mut [Page], faced: &[Faced]) {
    for page in pages {
        if let Some(f) = faced.iter().find(|f| f.sides == page.sides) {
            page.attendance = f.attendance;
            page.fans = f.fans;
        }
    }
}

/// What a team counts for before its match, its four parts reckoned anew
/// from its line-up as the game does first (`FUN_006a18c2`) and summed, in
/// whole points: a human's club as he has set it; a computer's club with
/// the line-up its co-trainer puts up, free in its system
/// (`FUN_0042c802`), which the original has it pick afresh before the
/// match.
pub fn strength(records: &Records, data: &Data, season: Option<&Season>, team: &Team) -> i32 {
    let pitch = &data.rules.pitch;
    let key = data.world.slot_of(team.country().into()).map(|slot| (slot, usize::from(team.number())));
    let parts = key.and_then(|key| {
        let season = season?;
        if let Some(club) = season.clubs.get(&key) {
            return Some(pitch.parts(&club.lineup, &club.players, &club.fitness));
        }
        let mut club = crate::load::club(records, data, key)?;
        if !season.managers.iter().any(|m| m.club == Some(key)) {
            autolineup::co_trainer(pitch, &mut club, autolineup::FREE | autolineup::FOREIGNERS);
        }
        Some(pitch.parts(&club.lineup, &club.players, &club.fitness))
    });
    let parts = parts.unwrap_or_else(|| team.strengths().map(i32::from));
    parts.iter().sum::<i32>() / 10
}

/// The league a page's match is played in, for its headline.
pub fn league<'a>(records: &'a Records, page: &Page) -> Option<League<'a>> {
    let Fixture::League { level, .. } = page.fixture else { return None };
    League::of(records, page.sides[0].0.into(), level)
}
