//! The game cycle: the game as a sequential program, on a thread of its
//! own, that asks for a screen whenever it needs the player and waits for
//! the reply — as the game's logic thread asks its main thread for dialogs.
//! What comes next is plain control flow here: the start menu, a new game,
//! the job offers, and the weeks of the season up to its first match.
//!
//! The screens share the [`GameState`] with the cycle; while it waits for a
//! reply, the open screen may read and change the state.

use std::path::Path;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;

use a3_files::an3::Save;
use tracing::{info, warn};

use crate::data::Data;
use crate::load;
use crate::medical;
use crate::national;
use crate::newgame::{self, Mode, Setup};
use crate::random::Random;
use crate::records::Records;
use crate::season::Season;
use crate::state::GameState;
use crate::youth;

/// The dialogs the cycle asks for, by their ids in the dialog file; the
/// week screen under ids of its own for none to four club panels.
pub mod dialogs {
    pub const START: u16 = 9;
    pub const LOAD: u16 = 424;
    pub const HIGH_SCORES: u16 = 95;
    pub const NET_NAME: u16 = 474;
    pub const NET_DETAILS: u16 = 473;
    pub const NET_GAMES: u16 = 416;
    pub const COUNTRIES: u16 = 140;
    pub const PLAYERS: u16 = 209;
    pub const NAMES: u16 = 212;
    pub const CLUB: u16 = 304;
    pub const NATIONAL_TEAM: u16 = 352;
    pub const NATIONAL_MENU: u16 = 355;
    /// The coach putting together his national squad
    /// (`CNationalkaderZusammenstellen`).
    pub const NATIONAL_SQUAD: u16 = 357;
    /// The coach looking for a foreign player to talk into changing his
    /// nation (`CNationalSpielerSuchen`).
    pub const NATIONAL_SEARCH: u16 = 351;
    /// The overview of his naturalization talks (`CEinbuergerungsgespraeche`).
    pub const NATIONAL_TALKS: u16 = 471;
    /// The coach arranging his team's friendlies in the international
    /// windows (`CNationalFreundschaftsspiel`).
    pub const NATIONAL_FRIENDLY: u16 = 354;
    /// The successes of every nation's teams at the big tournaments
    /// (`CInfoNationalteam`).
    pub const NATIONAL_INFO: u16 = 472;
    pub const ABILITIES: u16 = 374;
    pub const BIRTHDAY: u16 = 491;
    pub const SETTINGS: u16 = 256;
    pub const OFFERS: u16 = 99;
    pub const TALKS: u16 = 103;
    pub const FAILED: u16 = 102;
    pub const SIGNED: u16 = 104;
    pub const NOTICE: u16 = 380;
    /// A Turnier's draw and its results, for a human taking part.
    pub const TOURNAMENT_DRAW: u16 = 298;
    pub const TOURNAMENT_RESULT: u16 = 299;
    /// A hall's results, for a human taking part.
    pub const HALL_RESULT: u16 = 94;
    /// The betting shop's report of the matchday just paid
    /// (`CWettauswertung`); it comes up for every human the settlement
    /// left a section marked for (`FUN_0077f1b8` kind 1).
    pub const BET_REPORT: u16 = 409;
    pub const QUESTION: u16 = 384;
    pub const WAIT: u16 = 381;
    pub const OFFICE: u16 = 483;
    /// The national manager's office.  Its objects keep the club office's
    /// ids at other places and drop squad, money, trainings, nation and
    /// honours.  SEASON.md: it is the national coach's office under the
    /// same start-view bits as 483 and 504, as 502 is its text menu; the
    /// club office's nation object 6315 (code 1600) answers only for
    /// national coaches.
    pub const NATIONAL_OFFICE: u16 = 484;
    /// The grand office of the grandest clubs.
    pub const GRAND_OFFICE: u16 = 504;
    /// The manager's page of notes (`CDiNotizen`); a turn opens it over
    /// its screen when the manager left a note on his page.
    pub const NOTEPAD: u16 = 397;
    /// The next match's announcements, a league match's
    /// (`CSpielankuendigungLiga`) and any other's (`CSpielAnkuendigung`),
    /// which a turn opens first when the match is within a week and the
    /// game's options ask for it.
    pub const LEAGUE_ANNOUNCEMENT: u16 = 358;
    pub const MATCH_ANNOUNCEMENT: u16 = 201;
    /// The text menu of a manager who chose it.  Dialog 502, the national
    /// team's smaller variant, is not built yet.
    pub const TEXT_MENU: u16 = 501;
    pub const LINEUP: u16 = 147;
    /// The line-up as a list, for a manager who switched to it.
    pub const LINEUP_LIST: u16 = 429;
    pub const DRESSING_ROOM: u16 = 302;
    /// "Spielstand speichern" of the dressing room's game options
    /// (`CDiSave`), asking over its own question (`CAlertOkAbbruch`)
    /// before it takes a name that is saved already.
    pub const SAVE: u16 = 489;
    /// The comparison with the next opponent, man for man, the bar's
    /// second symbol.
    pub const COMPARISON: u16 = 78;
    /// The club's tactics, the bar's third symbol.
    pub const TACTICS: u16 = 271;
    /// The week's training, the bar's fifth symbol: as text, or as
    /// pictures for a manager who switched to them; the popup of its
    /// units.
    pub const SHORT_TRAINING: u16 = 346;
    /// The season's training weeks and the camp, the bar's fourth symbol;
    /// each player's own training, its sixth.
    pub const LONG_TRAINING: u16 = 345;
    pub const INDIVIDUAL_TRAINING: u16 = 347;
    /// The abilities gained and lost over the week (`CFaehigkeitenWoche`),
    /// which the turns show for a human club whose book of the week has
    /// entries.
    pub const WEEK_ABILITIES: u16 = 364;
    /// The ability a player is to work on, chosen from those he lacks.
    pub const ABILITY_CHOICE: u16 = 365;
    /// The year calendar, the bar's seventh symbol; the week planner, its
    /// eighth.
    pub const YEAR_CALENDAR: u16 = 348;
    pub const WEEK_PLANNER: u16 = 413;
    /// The balance sheet, the bar's ninth symbol; the finances, its tenth.
    pub const BALANCE: u16 = 349;
    pub const FINANCES: u16 = 417;
    /// The stadium, the bar's twelfth symbol.
    pub const STADIUM: u16 = 464;
    pub const EXPANSION: u16 = 463;
    pub const GROUNDS: u16 = 466;
    /// The squad and the transfer market, the bar's eleventh symbol, and
    /// its current squad.
    pub const SQUAD_MENU: u16 = 218;
    /// The telephone ("Kontakte", bar symbol 12).
    pub const CONTACTS: u16 = 404;
    /// The board ("Vereinsführung", bar symbol 13).
    pub const BOARD: u16 = 439;
    /// The statistics archive ("Statistiken", bar symbol 14).
    pub const STATISTICS: u16 = 18;
    /// The manager's own page ("Persönliche Erfolge", bar symbol 16).
    pub const HONOURS: u16 = 175;
    pub const CURRENT_SQUAD: u16 = 216;
    pub const MARKET_VALUES: u16 = 217;
    pub const NEXT_SQUAD: u16 = 219;
    pub const LOANED_OUT: u16 = 300;
    /// The squad's friendships and enmities, from the current squad.
    pub const RELATIONS: u16 = 500;
    /// The semi-automatic contract extension, and its question whether
    /// to draw an option.
    pub const EXTENSIONS: u16 = 92;
    pub const DRAW_OPTION: u16 = 305;
    /// "Kein Termin mehr frei": a player with no free appointment this week.
    pub const NO_DATE: u16 = 262;
    /// A player under contract asked onto the transfer list: what he
    /// wants for it.
    pub const LISTING_DEMAND: u16 = 205;
    /// A player's loan conditions, to put him on the list for a loan.
    pub const LOAN_CONDITIONS: u16 = 292;
    /// Other clubs' youths to poach, and the pre-contract offered.
    pub const YOUTH_POACHING: u16 = 107;
    pub const PRE_CONTRACT: u16 = 325;
    /// The club's youth and its amateurs, and a trial with the
    /// professionals offered to one of them.
    pub const YOUTH_TEAM: u16 = 428;
    /// Any club's squad to look through.
    pub const CLUB_SEARCH: u16 = 306;
    /// NORACSA's search, and what it found.
    pub const NORACSA_SEARCH: u16 = 154;
    pub const NORACSA_FOUND: u16 = 155;
    /// NORACSA's request for a player, kept for a week.
    pub const NORACSA_REQUEST: u16 = 156;
    pub const AMATEUR_PLAYERS: u16 = 427;
    pub const TRIAL: u16 = 11;
    /// The transfer market's list, the world's best players.
    pub const TRANSFER_LIST: u16 = 287;
    pub const WORLDS_BEST: u16 = 462;
    /// The week's appointments, the watch list and a watched player's
    /// choices.
    pub const APPOINTMENTS: u16 = 25;
    pub const WATCH_LIST: u16 = 27;
    pub const WATCHED_PLAYER: u16 = 28;
    /// Which human clubs join a talk from the week-start form of 25
    /// (`CAnVerhandlungenTeilnehmen`).
    pub const JOIN_TALKS: u16 = 376;
    /// The balance sheet's deposits, its loans, a loan paid back, the
    /// bank's refusal.
    pub const INVESTMENTS: u16 = 16;
    pub const LOANS: u16 = 335;
    pub const REPAYMENT: u16 = 138;
    pub const NO_CREDIT: u16 = 24;
    /// The alert of a human club out of a talk on its day: "ANSTOSS 3 -
    /// Achtung %s!" over the bank's refusal or the full squad.
    pub const ALERT_FOR: u16 = 2917;
    pub const BANK_NO_CREDIT: u16 = 2918;
    pub const SQUAD_FULL: u16 = 2919;
    pub const TRAINING_PICTURES: u16 = 513;
    pub const UNIT_CHOICE: u16 = 362;
    /// "Sind Sie sicher?" before the training strip writes, reads or
    /// restores its weeks and its plan; and the file select it opens.
    pub const SAFE_SAVE: u16 = 241;
    pub const SAFE_LOAD: u16 = 242;
    pub const SAFE_ORIGINAL: u16 = 243;
    pub const PLAN_FILES: u16 = 506;
    pub const WEEKS: [u16; 5] = [1000, 1001, 1002, 1003, 1004];
    /// A talk with a player, on the day it was booked for.
    pub const TALK: u16 = 461;
    pub const VISIT: u16 = 210;
    /// A poaching talk with another club's player on its day,
    /// `CVerhandlungAbwerbeversuch` (`IDD_VERTRAGSVERLAENGERUNG_SPIELER_STANDARD`).
    pub const POACHING_TALK: u16 = 319;
    /// A transfer's fee talked over with its human seller,
    /// `CVerhandlungUeberAbloese` (`IDD_VERHANDLUNG_UEBER_ABLOESE`).
    pub const FEE_TALK: u16 = 308;
    /// The year calendar's open day, `IDD_TAG_DER_OFFENEN_TUER`.
    pub const OPEN_DAY: u16 = 270;
    /// The tables after the matchday, `CTabelleNachDemSpiel`
    /// (`IDD_TABELLE_NACH_DEM_SPIEL`).
    pub const TABLES_AFTER: u16 = 269;
    /// The interview after the match, `CInterviewDialog`
    /// (`IDD_INTERVIEW`).
    pub const INTERVIEW: u16 = 383;
    /// The match report after the match, `CSpielberichtStandard`
    /// (`IDD_SPIELBERICHT_STANDARD`).
    pub const MATCH_REPORT: u16 = 332;
    /// The last information before the match, `CInfoseiteHaupt`
    /// (`IDD_INFOSEITE_HAUPT`), with a page `CInfoseiteSpiel` a match.
    pub const PREMATCH: u16 = 96;
    /// Half time of a human club's match, `CHalbzeitpause`
    /// (`IDD_HALBZEIT`), and the talk behind it, `CHalbzeitAnsprache`
    /// (`IDD_HALBZEIT_ANSPRACHE`).
    pub const HALFTIME: u16 = 93;
    pub const HALFTIME_TALK: u16 = 499;
    /// The doping control after the match, `CDopingAusgabe`
    /// (`IDD_DOPING_AUSGABE`).
    pub const DOPING_CONTROL: u16 = 41;
    /// The cup results after the matchday, `CPokalNachSpieltag`
    /// (`IDD_POKAL_NACH_DEM_SPIEL`).
    pub const CUP_RESULTS: u16 = 161;
    /// The association's court on its planner day, `CGerichtDialog`
    /// (`IDD_GERICHT`).
    pub const COURT: u16 = 89;
}

use self::dialogs::*;

/// The start menu's entries, in order.
/// The season tickets sell and the boards are let while the half weeks
/// since the season's start stand below this (`DAT_00b5cbd2 < 0xc`).
const PRESEASON_HALF_WEEKS: u16 = 12;
const NEW_GAME: usize = 0;
const LOAD_GAME: usize = 1;
const HOST_NEW: usize = 2;
const HOST_SAVED: usize = 3;
const JOIN: usize = 4;
const SCORES: usize = 5;
/// "Niemand hat Sie mehr im Angebot": every offer is gone.
const NO_ONE_LEFT: u16 = 5053;
/// "Sie haben für das anstehende Länderspiel nicht genügend Spieler in
/// ihrem Nationalkader!": the week hub's alert when its coach's squad is
/// too small for his team's match this week.
const NOT_ENOUGH_PLAYERS: u16 = 4966;
/// Clubs from this class on have the grand office.
const GRAND_CLASS: i64 = 5;
/// The manager's choice of the office over the menu bar.
const OFFICE_MENU: u8 = 1;
/// The manager's choice of the text menu over the menu bar.
const TEXT_MENU_CHOICE: u8 = 2;
const WEEK_DAYS: usize = 7;
/// Whether the game shows the tournaments' screens (`DAT_00b5d14f`).
const TOURNAMENT_SCREENS: usize = 0xb5d14f;
/// The game's switch for the Spielbericht after the match (byte 0x07 of
/// the switches at `0xb5d13c`, `DAT_00b5d143`), on at 1.
const REPORT_SWITCH: usize = 0x07;
/// From this day of the season the report follows every match; before it
/// only with the preparation normal (5) or "nur Pflichtspiele" (12)
/// (`FUN_006ef262`, `DAT_00b5d152`).
const REPORTS_FROM: usize = 42;
const FULL_PREPARATIONS: [u8; 2] = [5, 12];

/// Whether the day's match reports open their dialog (`FUN_006ef262`
/// posting event 0xce, `FUN_00551818` opening it).
fn reports_shown(records: &Records, day: usize) -> bool {
    (day >= REPORTS_FROM || FULL_PREPARATIONS.contains(&records.globals.preparation()))
        && records.globals.game_options()[REPORT_SWITCH] == 1
}
/// The game option "Spielankündigung vor jedem Spiel" (its byte among the
/// switches at `0xb5d13c`), on; how many days ahead it announces a match.
const ANNOUNCE_OPTION: usize = 2;
const ON: u8 = 1;
const ANNOUNCE_AHEAD: i16 = 7;
/// The game options' switch to keep the rolling autosaves and the weeks
/// between them — bytes 0x0f and 0x1f of the switches at `0xb5d13c`, on
/// and every week since `FUN_006c6290`. The dressing room's spinner 6348
/// sets the weeks; zero saves every week.
const AUTOSAVE: usize = 0x0f;
const AUTOSAVE_WEEKS: usize = 0x1f;
/// The kind of a league match (club +0xb4c).
const LEAGUE_MATCH: u8 = 2;
/// A talk booked in the week planner, and the talk itself.
const TALK: u8 = 0x19;
const TALK_DIALOG: u16 = dialogs::TALK;
/// An event entry of the year calendars, and the open day's number in it.
const CALENDAR_EVENT: u8 = 8;
const OPEN_DAY_EVENT: u16 = 6;
/// A sick visit booked in the week planner; what the week calls each.
const VISIT: u8 = 0x0b;
const TALK_TEXT: u16 = 4230;
const VISIT_TEXT: u16 = 2221;
/// The professional-contract talks "Profivertrag geben" books, for the
/// youth and the amateurs; on their day the week calls them a negotiation
/// (`FUN_0076cb3e`, text 5048 via `FUN_0064f456`).
const CONTRACT_TALKS: [u8; 2] = [youth::YOUTH_CONTRACT_TALK, youth::AMATEUR_CONTRACT_TALK];
const CONTRACT_TALK_TEXT: u16 = 5048;
/// The visit tells how he took it with the preparation at "normal", or
/// from this day of the season on.
const NORMAL_PREPARATION: u8 = 5;
const TOLD_FROM: usize = 42;

/// A screen to open, with what it is to show.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Request {
    pub screen: u16,
    pub args: Args,
}

impl Request {
    pub fn new(screen: u16) -> Self {
        Self { screen, args: Args::None }
    }

    pub fn with(screen: u16, args: Args) -> Self {
        Self { screen, args }
    }
}

/// A negotiation the week planner books (VERHANDLUNG-BUCHUNG.md): the
/// player; the appointment's kind (2 poaching, 3 a purchase, 4 a loan) and
/// the planner's (0x0d, 0x0e); whether the contract's terms go along;
/// whether the prompt asks for "den gewünschten Termin" (287's purchase,
/// 462) rather than the file's; the day of the player's appointment when
/// the club joins it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Negotiation {
    pub player: u16,
    pub kind: i16,
    pub planner: u8,
    pub terms: bool,
    pub chosen: bool,
    pub join: Option<i16>,
}

/// What a `%` of a text stands for: nothing, a number, a count with
/// points between the thousands (`FUN_004dacb9`), an amount of money as
/// the game writes a 64-bit one (`FUN_004db27e`) or a 32-bit one
/// (`FUN_004daf8f`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fill {
    None,
    Number(i32),
    Count(i32),
    Money64(i64),
    Money(i64),
}

/// What a screen is asked to show.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Args {
    None,
    /// The finances' yearly statement at the season's end: dialog 417
    /// opened without the bar and with its OK, as the event 0x12d the
    /// season end posts to every human club opens it (`FUN_006742e0`,
    /// `FUN_004a141c`).
    Statement,
    /// The world map of the training camps' places, for a camp from a day.
    CampDay(usize),
    /// The choice of a friendly's opponent, for a day.
    FriendlyDay(usize),
    /// The choice of a tournament of a kind (0 Turnier, 1 hall, 2 beach)
    /// for a day: one's own or another's to join.
    TournamentDay { day: usize, kind: u8 },
    /// One's own tournament to invite clubs to, by its id.
    OwnTournament { id: u32, day: usize, kind: u8 },
    /// A tournament to show, by its id.
    Tournament(u32),
    /// A friendly offered to another human's club: the club, whether the
    /// asking club plays away, the day.
    FriendlyAsk { country: u8, number: i16, away: bool, day: usize },
    /// A human's club invited to a tournament of one's own.
    TournamentInvite { id: u32, country: u8, number: i16 },
    /// A camp's place to confirm, from 0, and its first day.
    Camp { place: usize, day: usize },
    /// The choice among a group's other places, from 0: up to three.
    CampGroup { places: [u8; 3], count: u8 },
    /// The text of a question or notice, by its id.
    Text(u16),
    /// A job offer, by its tab.
    Offer(usize),
    /// A player of the current club, by his place in its squad.
    Player(usize),
    /// A player of the game, by his record's id.
    Record(u16),
    /// A player of the game, and the club (country, number) a popup about
    /// him is for: its caption ends in the club's name.
    RecordAt(u16, u8, i16),
    /// A notice's text for a club, its caption "ANSTOSS 3 - Achtung! -
    /// <club>" (`FUN_0043e49f`).
    TextAtClub(u16, u8, i16),
    /// The week's appointments as the week start shows them, for all
    /// humans at once (25 with +0xd94 set).
    WeekStart,
    /// The human clubs that may join a talk (376): up to four, by country
    /// and number.
    JoinClubs { clubs: [(u8, i16); 4], count: u8 },
    /// The leagues the table after the matchday steps through, by country
    /// and level (the list at 0x1c2efd0 the dialog copies, `FUN_007291e4`).
    Tables { leagues: [(u16, u8); crate::matches::TABLES_MOST], count: u8 },
    /// The human clubs whose match reports the Spielbericht after the
    /// match shows, the first opened, the others by their crests
    /// (`FUN_00551818`'s list, the dialog's +0x3328).
    Reports { clubs: [(u8, i16); 4], count: u8 },
    /// The doping control of a human's club after the match
    /// (`DAT_00a2e0b8`).
    Doping(crate::doping_control::Control),
    /// A case the association's court heard (`DAT_00a3b44c`).
    Court(crate::court::Case),
    /// The day's national cups for the cup results (the block at
    /// `0xa3b488`).
    CupResults(crate::cup_results::CupDay),
    /// A day's event for its window (the block at `0xa2f6d8`).
    Event(crate::event_text::Posted),
    /// A record the current manager broke, for its window over the 3D
    /// picture (`DAT_00a321fc`, kind 5).
    Render(crate::render::Broken),
    /// Which player to watch a week: dialog 281 asking for one of the
    /// club's own, none excluded, "keiner" among them.
    WeekPlayer,
    /// An alert the game writes itself, by where it is in the program.
    Message(u32),
    /// Two texts, the first and a space before the second, both as they
    /// are; a first of 0 is none.
    Texts(u16, u16),
    /// A text by its id, with a player's name for its `%s`.
    TextFor(u16, u16),
    /// A notice with its own title, its text with a player's name for its
    /// `%s` and his club's phrases (`v1` … `V20`), by ids.
    PlayerMove {
        title: u16,
        text: u16,
        player: u16,
    },
    /// A notice whose title (by id) takes a club's name for its `%s`, over
    /// a text by its id (`FUN_0043ddb9`).
    ClubAlert {
        title: u16,
        text: u16,
        country: u8,
        number: i16,
    },
    /// A text by its id, with the date of a day of the season for its
    /// `%s`.
    TextForDate(u16, usize),
    /// A text by its id, with an amount of money for its `%s`.
    TextForMoney(u16, i64),
    /// A text with a number for its "%d".
    TextForNumber(u16, i32),
    /// A text by its id, with a club's name for its `%s`: the club by its
    /// country and number.
    TextForClub(u16, u8, i16),
    /// A text by its id with a club's phrases (`v1` … `V20`) and a number
    /// for its "%d": the club by its country and number.
    ForClub {
        text: u16,
        country: u16,
        number: u16,
        value: i32,
    },
    /// The trainers' market: the goalkeepers' coaches, or the others'.
    Trainers {
        keepers: bool,
    },
    /// The market's offer dialog, opening talks with one candidate: the
    /// goalkeepers' coach or not, and his record's id.
    TrainerOffer {
        keepers: bool,
        id: i16,
    },
    /// What came of such talks (dialogs 31, 34, 37 and 39): which of them,
    /// the trainer by kind and record id, and the money and years the
    /// dialog names — the old coach's severance, or the compromise.
    TrainerNews {
        dialog: u16,
        keepers: bool,
        id: i16,
        amount: i64,
        years: i32,
    },
    /// The answer to "Vertrag verlängern" (dialogs 36 and 38): which of
    /// them, the coach by kind and record id, and — of the one that agrees
    /// — the year his longer contract now runs to.
    TrainerExtend {
        dialog: u16,
        keepers: bool,
        id: i16,
        until: i32,
    },
    /// A cooperation contract still running (text 4456): the partner's
    /// number in the main country, the year it ends.
    Cooperation {
        partner: u16,
        year: i32,
    },
    /// A text by its id with what its `%d`s and `%s`s stand for, in order.
    Printf(u16, [Fill; 2]),
    /// A loan's terms (dialogs 290, 289): the player, the fee asked and
    /// its fair value, the option's price asked and its fair value.
    LoanTerms {
        player: u16,
        fee: i32,
        fair_fee: i32,
        option: i32,
        fair_option: i32,
        /// The club the terms are shown for from the week-start form,
        /// whose name the caption ends in.
        club: Option<(u8, i16)>,
    },
    /// The builder's offer (text 4812): its price, the weeks and the weeks
    /// it comes to, and the cost in all.
    BuilderOffer {
        price: i32,
        weeks: i16,
        total: i32,
    },
    /// A text by its id, with another text for its `%s`.
    TextForText(u16, u16),
    /// A loan of the current club, by its slot.
    Loan(usize),
    /// How a player took a sick visit (`medical::Reaction`).
    Visit {
        player: u16,
        kind: u8,
        draw: u8,
    },
    /// An appointment of a kind to book with a player, by his record's id.
    Book(u8, u16),
    /// A negotiation to book in the week planner, or to join.
    Negotiation(Negotiation),
    /// A contract talk with one's own board to book in the week planner
    /// (the board screen's first item): the club by country and number.
    WithClub(u8, i16),
    /// A club, by its country's slot and its place there.
    Club(usize, usize),
    /// A youth or an amateur to try with the professionals, by his record's
    /// id.
    Trial {
        player: u16,
        youth: bool,
    },
    /// A text by its id, with the name of one of the current manager's
    /// formations for its `%s`.
    TextForFormation(u16, usize),
    /// The list view's question whether one of the current manager's
    /// formations is to keep the eleven's spots, in its own words.
    KeepFormation(usize),
    /// The training strip's "Sind Sie sicher?": whether it speaks of the
    /// season's plan, which changes dialog 242's words, or of the
    /// manager's own training weeks.
    PlanSafe {
        plan: bool,
    },
    /// The training strip's file select, saving or reading a file — of the
    /// manager's own training weeks (`plan\*.Woc`) or of the season's plan
    /// (`plan\*.pla`), which changes its extension; the select writes and
    /// reads the file itself, as the game's `CFileSelect` does.
    PlanFiles {
        save: bool,
        plan: bool,
    },
    /// What changed of a player's contentment with the trainer, told with a
    /// text with his name.
    Change {
        text: u16,
        player: u16,
        before: i16,
        after: i16,
    },
    /// A youth age group: 0 the A, 1 the B, 2 the C youth.
    AgeGroup(usize),
    /// One of the manager's week templates, by its place.
    Template(usize),
    /// A text by its id with a player's name for its `%s`, and the alert's
    /// style, which picks its ball (`FUN_0043e4c2`: 3 and 4 the doping's).
    StyledFor(u16, u16, u8),
    /// A share of the market by its place, and the manager's holding of it
    /// by its place, whose price paid the chart marks.
    Share(usize, Option<usize>),
}

/// How a screen was left.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reply {
    /// OK, yes, or done.
    Ok,
    /// Cancel or no.
    Cancel,
    /// An entry chosen: the start menu's, or the job offer to talk about.
    Choice(usize),
    /// On to another of the main screens, by its dialog id.
    Goto(u16),
    /// The game is to end: the program closes.
    Quit,
    /// The game is given up for a new one: back to the start menu.
    NewGame,
}

/// The screens' end of the line to the cycle: its requests, and where the
/// replies go. Dropping it ends the cycle at its next request.
pub struct Link {
    pub requests: Receiver<Request>,
    pub replies: Sender<Reply>,
}

/// Starts the game cycle on its thread. `seed` gives each new game's random
/// numbers their start; the app takes them from the clock, as the game
/// seeds its generator from the time.
pub fn spawn(
    data: Arc<Data>,
    state: Arc<Mutex<GameState>>,
    seed: Box<dyn FnMut() -> u64 + Send>,
) -> Link {
    let (requests, from_cycle) = channel();
    let (to_cycle, replies) = channel();
    let mut cycle = Cycle { data, state, seed, requests, replies };
    thread::Builder::new()
        .name("game cycle".into())
        .spawn(move || cycle.run())
        .expect("a thread for the game cycle");
    Link { requests: from_cycle, replies: to_cycle }
}

/// The game is over: quit, or nobody is there to answer the cycle's
/// requests any more.
struct Gone;

struct Cycle {
    data: Arc<Data>,
    state: Arc<Mutex<GameState>>,
    seed: Box<dyn FnMut() -> u64 + Send>,
    requests: Sender<Request>,
    replies: Receiver<Reply>,
}

impl Cycle {
    fn run(&mut self) {
        // Ends when the game is quit or nobody answers any more.
        let _ = self.start_menu();
    }

    /// Asks for a screen and waits for its reply.
    fn ask(&self, request: Request) -> Result<Reply, Gone> {
        self.requests.send(request).map_err(|_| Gone)?;
        self.replies.recv().map_err(|_| Gone)
    }

    /// Asks for a screen that only needs OK; whether it got it.
    fn ok(&self, screen: u16) -> Result<bool, Gone> {
        Ok(self.ask(Request::new(screen))? == Reply::Ok)
    }

    fn state(&self) -> MutexGuard<'_, GameState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Works on the season, if there is one.
    fn season<T>(&self, work: impl FnOnce(&mut Season) -> T) -> Option<T> {
        self.state().season.as_mut().map(work)
    }

    /// The start menu, again and again, until the game is quit.
    fn start_menu(&mut self) -> Result<(), Gone> {
        loop {
            let Reply::Choice(entry) = self.ask(Request::new(START))? else { continue };
            let mode = match entry {
                NEW_GAME => Mode::Single,
                LOAD_GAME => Mode::Load,
                HOST_NEW => Mode::ServerNew,
                HOST_SAVED => Mode::ServerLoad,
                JOIN => Mode::Client,
                SCORES => {
                    self.ask(Request::new(HIGH_SCORES))?;
                    continue;
                }
                _ => return Ok(()),
            };
            self.state().mode = mode;
            match mode {
                Mode::Single => {
                    self.state().setup = Setup::default();
                    self.new_game()?;
                }
                Mode::Load | Mode::ServerLoad => self.load()?,
                Mode::ServerNew | Mode::Client => self.network()?,
            }
        }
    }

    /// A new game, through the game's own sequence of screens: countries,
    /// number of players, a name per player, then for every player a club
    /// and national team, abilities and birthday, and the start settings.
    /// Cancel on any of them goes back to the start menu.
    fn new_game(&mut self) -> Result<(), Gone> {
        if !self.ok(COUNTRIES)? || !self.ok(PLAYERS)? {
            return Ok(());
        }
        let players = self.state().setup.trainers.len();
        for player in 0..players {
            self.state().setup.current = player;
            if !self.ok(NAMES)? {
                return Ok(());
            }
        }
        if self.state().setup.chooses_club() {
            for player in 0..players {
                self.state().setup.current = player;
                if !self.ok(CLUB)? {
                    return Ok(());
                }
                // Without a club, no national team either.
                let club = self.state().setup.trainer().is_some_and(|t| t.club.is_some());
                if club && !self.ok(NATIONAL_TEAM)? {
                    return Ok(());
                }
            }
        }
        for player in 0..players {
            self.state().setup.current = player;
            if !self.ok(ABILITIES)? || !self.ok(BIRTHDAY)? {
                return Ok(());
            }
        }
        if !self.ok(SETTINGS)? {
            return Ok(());
        }
        let seed = (self.seed)();
        {
            let mut state = self.state();
            newgame::log(&self.data.world, &state.setup);
            state.season = Some(newgame::calculate(&self.data, &state.setup, seed));
            state.play.game_loaded(0);
        }
        self.job_offers()?;
        self.weeks(true)
    }

    /// Every manager without a club looks at the offers and talks, until a
    /// contract is signed or every offer is gone.
    fn job_offers(&mut self) -> Result<(), Gone> {
        while self.next_with_offers() {
            let mut tab = 0;
            loop {
                let Reply::Choice(talk) = self.ask(Request::with(OFFERS, Args::Offer(tab)))? else {
                    // Going without a club.
                    self.decline();
                    break;
                };
                tab = talk;
                if self.ask(Request::with(TALKS, Args::Offer(tab)))? == Reply::Ok {
                    self.sign(tab);
                    self.ask(Request::new(SIGNED))?;
                    break;
                }
                if self.offer(tab, |o| o.open) {
                    continue;
                }
                // The club broke the talks off: on to the first offer still
                // open, if there is one.
                self.ask(Request::new(FAILED))?;
                match self.first_open_offer() {
                    Some(first) => tab = first,
                    None => {
                        self.ask(Request::with(NOTICE, Args::Text(NO_ONE_LEFT)))?;
                        self.decline();
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    /// The humans queued to join take their slots at the week's start
    /// (`FUN_006c5005`), each with the offers of the clubs in want of a
    /// manager — the original lets him wait for the offers of a running
    /// season, which are not built; the first season's stand in for them.
    fn newcomers(&mut self) -> Result<(), Gone> {
        let joined = {
            let mut guard = self.state();
            let state = &mut *guard;
            let queue = std::mem::take(&mut state.joining);
            let Some(season) = state.season.as_mut() else { return Ok(()) };
            let mut joined = Vec::new();
            for trainer in &queue {
                let Some(at) =
                    crate::joining::join(season, state.records.as_mut(), &self.data.rules, trainer)
                else {
                    continue;
                };
                for j in &mut joined {
                    if *j >= at {
                        *j += 1;
                    }
                }
                joined.push(at);
            }
            if !joined.is_empty() {
                crate::jobs::offer_jobs_to(season, &self.data.world, &self.data.rules, &joined);
            }
            joined
        };
        if joined.is_empty() {
            return Ok(());
        }
        let current = self.season(|s| s.current).unwrap_or(0);
        self.job_offers()?;
        self.season(|s| s.current = current.min(s.managers.len().saturating_sub(1)));
        Ok(())
    }

    /// Makes the next manager with offers the current one; whether there is
    /// one.
    fn next_with_offers(&self) -> bool {
        self.season(|season| {
            let manager = season.managers.iter().position(|m| !m.offers.is_empty());
            if let Some(manager) = manager {
                season.current = manager;
            }
            manager.is_some()
        })
        .unwrap_or(false)
    }

    /// Something about the current manager's offer on `tab`.
    fn offer(&self, tab: usize, look: impl FnOnce(&crate::jobs::Offer) -> bool) -> bool {
        self.season(|s| s.managers[s.current].offers.get(tab).is_some_and(look)).unwrap_or(false)
    }

    fn first_open_offer(&self) -> Option<usize> {
        self.season(|s| s.managers[s.current].offers.iter().position(|o| o.open)).flatten()
    }

    /// The current manager goes without a club, for now.
    fn decline(&self) {
        self.season(|s| {
            let current = s.current;
            s.managers[current].offers.clear();
        });
    }

    /// The current manager signs the offer on `tab` as it stands.
    fn sign(&self, tab: usize) {
        let data = &self.data;
        let club = self.season(|s| s.managers[s.current].offers.get(tab).map(|o| o.club));
        if let Some(club) = club.flatten() {
            let mut guard = self.state();
            let state = &mut *guard;
            let Some(s) = state.season.as_mut() else { return };
            let current = s.current;
            s.take_club(&data.world, &data.rules, current, club);
            // A game loaded from its records: the manager's record names
            // the club he is under contract with, and the club is followed
            // as the records hold it.
            if let Some(records) = state.records.as_mut()
                && let Some(slot) = records.human_slot(current)
                && let Some(nation) = data.world.country(club.0).map(|c| c.nation)
            {
                let manager = &mut records.managers[slot];
                manager.set_status(load::HIRED);
                manager.set_country(nation);
                manager.set_team(club.1 as u16);
                if let Some(team) = records.team_mut(nation, club.1 as u16) {
                    let [_, second] = team.humans();
                    team.set_humans([slot as u8 + 1, second]);
                }
                if let Some(followed) = load::club(records, data, club) {
                    s.clubs.insert(club, followed);
                }
            }
        }
    }

    /// The managers who have a turn this week, in their order
    /// (`FUN_0064e40f`): each of a club (its turn 100), and one with a
    /// national team only (0x65); without either a manager plays no turn
    /// in a game at one computer, but over the net he does. When that
    /// leaves nobody, the first has the turn.
    fn turns(&self) -> Vec<usize> {
        let network = matches!(self.state().mode, Mode::ServerNew | Mode::ServerLoad | Mode::Client);
        self.season(|s| {
            let turns: Vec<usize> = (0..s.managers.len())
                .filter(|&m| {
                    let manager = &s.managers[m];
                    manager.club.is_some() || manager.national_team.is_some() || network
                })
                .collect();
            if turns.is_empty() && !s.managers.is_empty() { vec![0] } else { turns }
        })
        .unwrap_or_default()
    }

    /// The abilities gained and lost over the week: before a turn, a
    /// human club whose book has entries shows its dialog
    /// (`CFaehigkeitenWoche`, built and run by `CMyStartDialog`'s
    /// `FUN_00499e7f` in the week's open block).  An empty book shows
    /// nothing and waits for nothing: the dialog's `OnInit` ends it
    /// itself at once when the first entry reads 0 (`FUN_007a0d38(1)`).
    fn week_abilities(&mut self) -> Result<(), Gone> {
        let club = self.season(|s| {
            let (country, number) = s.manager()?.club?;
            let nation = self.data.world.country(country)?.nation;
            Some((nation, u16::try_from(number).ok()?))
        });
        let Some((nation, number)) = club.flatten() else { return Ok(()) };
        let guard = self.state();
        let book = guard
            .records
            .as_ref()
            .and_then(|records| {
                let slot = records.team(nation, number).map_or(0, |t| t.human());
                (!records.abilities_week(slot).is_empty()).then_some(slot)
            });
        drop(guard);
        if book.is_some() {
            self.ok(WEEK_ABILITIES)?;
        }
        Ok(())
    }

    /// The season week by week: the autosave's wait screen, every human
    /// manager's turn, and the week running day by day — until the week of
    /// the first match, which is not played yet. A loaded game starts with
    /// the turns, as the original's does (its cycle's case 2).
    fn weeks(&mut self, mut wait: bool) -> Result<(), Gone> {
        loop {
            // The turns begin on the week's first day, which the planner,
            // the market and the contract talks count from
            // (`FUN_006da3a9`'s state 1: `DAT_00b5cbd6 = DAT_00b5cbd4`).
            // A loaded game first shows its turn again on the saved day
            // itself (the load opens 147); that short week then ends at
            // the day boundary below, where the turns here keep counting
            // from — the saved week's own start stands until then.
            if wait {
                let mut guard = self.state();
                if let Some(records) = guard.records.as_mut() {
                    let today = records.globals.day();
                    records.globals.set_week_start(today as i16);
                    // Past the season's first half week the shareholders
                    // vote (`DAT_00b5cbd2 != 0` → `FUN_00751d76` →
                    // `FUN_00406baf`).
                    if records.globals.half_weeks() != 0 {
                        crate::stock::vote(records);
                    }
                }
            }
            if wait {
                self.newcomers()?;
            }
            self.statement()?;
            if wait {
                self.ask(Request::new(WAIT))?;
                // The week's appointments for all humans, before the
                // turns: the week block queues code 0x88 behind the
                // autosave's when the appointments hold any, with the
                // preparation 5 or from day 42 (`FUN_006da3a9` at
                // 0x6da86a, `FUN_00762149`); the start dialog opens 25
                // with +0xd94 set (`FUN_0047c449`). Its option byte
                // `DAT_00b5d142` is taken at its default, 1.
                let due = self.state().records.as_ref().is_some_and(|r| {
                    (r.globals.preparation() == NORMAL_PREPARATION || r.globals.day() >= 42)
                        && !crate::transfers::appointments(r).is_empty()
                });
                if due {
                    self.ask(Request::with(APPOINTMENTS, Args::WeekStart))?;
                }
            }
            wait = true;
            for manager in self.turns() {
                self.season(|s| s.current = manager);
                self.week_abilities()?;
                if !self.turn()? {
                    return Ok(());
                }
            }
            // The week's end stands at the week's boundary, not behind
            // the turn: the original's cycle gates the blocks on
            // `DAT_00b5cbd4 % 7 == 0` (`FUN_006da3a9`'s states 1, 2 and
            // the 4 below them). A game loaded in the middle of its week
            // — save "Toni", day 62 of the week from 56 — therefore runs
            // none of them on its saved day; its first week end, and its
            // next turn with the week's dialog 147 back at 63, came in
            // the trace of 7 Nov.
            let at_week_start = self.season(|s| s.day % WEEK_DAYS == 0).unwrap_or(true);
            // The week's end first rolls the market (`FUN_00405c5e`):
            // every entry takes its price for the new week and the mean
            // of them moves the market's index. A freshly loaded game
            // holds that roll back (`DAT_00b5cc04`, `CDiLoad`) until its
            // advance sales below clear the hold — the traced original
            // week end, running just after a load, rolled no market.
            if at_week_start {
                let mut guard = self.state();
                let state = &mut *guard;
                if let (Some(season), Some(records)) =
                    (state.season.as_mut(), state.records.as_mut())
                {
                    if !season.week_hold {
                        // The week's end looks first at its last day's
                        // matches (`FUN_006da3a9`'s case 2 polling
                        // `FUN_0062a325` on the day six on): each league
                        // playing then draws its pair for the doping
                        // control, the list going on to `FUN_0078046d`
                        // (unported). After a load the poll runs before
                        // the turn's screen, its draws from the seed the
                        // loader takes from `rand()`, so the replica's
                        // fresh seed stands for them.
                        crate::matches::list_day(
                            records,
                            &self.data.calendar,
                            season.day + 6,
                            &mut season.random,
                        );
                        // The clubs' week on the transfer market comes
                        // first (`FUN_0042958d`): last years weighed,
                        // retirements announced.
                        crate::transfer_week::week(records, &mut season.random);
                        // The human clubs' trials go on a week, or end
                        // (`FUN_0064e25b`).
                        crate::youth::end_trials(records, &mut season.clubs, &self.data.world, false);
                        // It forgets the last friendlies more than six
                        // weeks gone (`FUN_0043539b`).
                        crate::friendly::forget_last_friendlies(records, season.day as i16);
                        // The week's negotiations and the court's day go
                        // into the human clubs' planners (`FUN_0072d850`).
                        crate::court::book_week(records);
                        crate::stock::roll_week(records, season.day as i32, &mut season.random);
                    }
                }
            }
            // The week's end opens the advance sales of each human club's
            // league matches of the next four weeks (`FUN_0064e40f` ->
            // `FUN_0077d4ea`), and a record just opened sells at once
            // (`FUN_0077c1d5`'s `FUN_0077bfbd`). The load's hold
            // (`DAT_00b5cc04`, `CDiLoad`) covers the market block above
            // and nothing after its clear: `FUN_0064e40f` clears the hold
            // right before the club loop that keeps forms and opens these
            // sales, so the traced original's first draw after `now
            // random` was here — one far match's percent, `FUN_0078ab55`
            // drawing below(2) and giving 1, one percent of a demand of
            // 18.8 thousand sold right away.
            if at_week_start {
                let mut guard = self.state();
                let state = &mut *guard;
                if let (Some(season), Some(records)) =
                    (state.season.as_mut(), state.records.as_mut())
                {
                    let today = season.day as i32;
                    let held = season.week_hold;
                    // The rolling autosaves, after the turns: the week's
                    // end writes them right before it clears the load's
                    // hold, so the week a game was loaded in keeps none
                    // (`FUN_0064e40f` → `FUN_006ecfeb`).
                    if !held {
                        self.autosave(records, season.day / WEEK_DAYS);
                    }
                    season.week_hold = false;
                    // The human clubs' players keep their forms for the
                    // trend (`FUN_007863d0`), and each human manager a
                    // copy of his club's money (`FUN_0078605e`), as the
                    // week ends (`FUN_0064e40f`).
                    crate::season::keep_forms(records, &mut season.clubs, &self.data.world);
                    crate::settlement::keep_club_money(records);
                    for club in crate::statistics::human_clubs(records) {
                        // The week's book of gained and lost abilities
                        // ends with the week (`FUN_0064e40f`'s
                        // `FUN_006436b3`, right before the advance
                        // sales); what the days below record shows at the
                        // next boundary, before this clears it again.
                        let slot = records
                            .team(u16::from(club.0), club.1 as u16)
                            .map_or(0, |t| t.human());
                        records.clear_abilities_week(slot);
                        crate::presale::week_end(
                            records,
                            &self.data.calendar,
                            club,
                            today,
                            &mut season.random,
                            &self.data.presale,
                        );
                    }
                    // The computer clubs line up for the new week
                    // (`FUN_0064e40f`'s `FUN_0042a76e`), so the week's
                    // games are scored by their squads as they stand.
                    crate::autolineup::computer_lineups(records, &self.data);
                    // The friendlies' offers are drawn anew for the next
                    // turns, but not after a load (`FUN_0064e40f`).
                    if !held {
                        season.opponents.clear();
                    }
                }
            }
            // The careers ended in the turns end before the week does
            // (`FUN_006c5190`), and the highscores show them (its event
            // 0xb0) — unless no human is left: then the game ends as "Spiel
            // beenden" ends it, by its flag 4, before the event shows.
            let (ended, left) = if at_week_start {
                self.careers_end()
            } else {
                (false, true)
            };
            if !left {
                return Err(Gone);
            }
            if ended {
                self.ask(Request::new(HIGH_SCORES))?;
            }
            // The week's end books what the managers gave to other
            // humans' clubs with the recipients' other income, and
            // empties the week's store so its givers may give again
            // (`FUN_006eeda7`).
            if at_week_start {
                let mut guard = self.state();
                let state = &mut *guard;
                if let (Some(season), Some(records)) =
                    (state.season.as_mut(), state.records.as_mut())
                {
                    crate::finances::book_donations(records, &mut season.donations);
                }
            }
            self.season(|s| s.before_match = false);
            let Some(week) = self.season(|s| self.start_week(s)) else {
                return Ok(());
            };
            self.ask(Request::new(week))?;
            // Days until the week's boundary: seven for a week that
            // opened at one, and the rest of its own week for a loaded
            // game standing mid-week — the `start` flag below fires on
            // `day % 7 == 0`, where the turns begin again.
            loop {
                // The day walked; its booked open day's dialog comes up in
                // the walk's middle, once the day's training is done and
                // before the friendlies and the news (`open_events_on`
                // below): the traced original showed it right after the
                // training's last draw.
                let walked = self.season(|s| s.day).unwrap_or(0);
                let mut slots = None;
                let mut posted = Vec::new();
                {
                    let mut guard = self.state();
                    let state = &mut *guard;
                    if let Some(season) = state.season.as_mut() {
                        // The day's match list is drawn before the human
                        // clubs' day (`FUN_006da3a9`'s case 3,
                        // `FUN_0062a325(day)` into `0x1baf714`), each
                        // league playing drawing its doping pair; once the
                        // day's routine is done it is drawn again, and the
                        // matches take that second list.
                        if let Some(records) = state.records.as_ref() {
                            crate::matches::list_day(records, &self.data.calendar, season.day, &mut season.random);
                        }
                        // The human clubs' day starts it (`FUN_0064f981`):
                        // on the second weekday the computer clubs'
                        // own week first, then the event gates, then
                        // the day's ticket sales.
                        let today = season.day as i32;
                        if today % 7 == 1 {
                            // The bulk draws from the shared dice, so
                            // its draws come in the original's order
                            // (`FUN_0074ca91`).
                            if let Some(records) = state.records.as_mut() {
                                crate::computer_training::week(
                                    records,
                                    &self.data.rules.computer_training,
                                    &self.data.rules.training,
                                    season.date(),
                                    &mut season.random,
                                );
                            }
                        }
                        let gates =
                            crate::human_day::event_slots(&mut season.random, (today % 7) as i16);
                        slots = Some(gates);
                        if let Some(records) = state.records.as_mut() {
                            // The day's headline goes to the paper
                            // before its events (`FUN_0078a0f5`); it
                            // draws nothing.
                            season.headlines.publish(records);
                            // The half week ticks first, the winter break
                            // developing the players at its 56th
                            // (`FUN_006da3a9`'s state 4).
                            crate::development::half_week(records, today, &mut season.random);
                            crate::presale::day(records, today, &mut season.random, &self.data.presale);
                            // The week's first day starts with the men's
                            // extra shifts and youth rounds and the
                            // week's slips settled, before the day's loop
                            // (`FUN_00747144`, `FUN_00786285`).
                            if today % 7 == 0 {
                                crate::training_day::week_start(
                                    records,
                                    &self.data.rules.discipline.mood,
                                    &mut season.random,
                                );
                            }
                            // Over the first six weeks the season tickets
                            // sell and the perimeter boards are let
                            // (`FUN_006da3a9` hands `FUN_0064f981` the
                            // half weeks below 12).
                            if records.globals.half_weeks() < PRESEASON_HALF_WEEKS {
                                crate::tickets::season_ticket_day(records, &self.data.calendar, &mut season.random);
                                crate::human_day::let_boards(records, today, &mut season.random);
                            }
                            // The day's training at the human clubs
                            // follows the sales (`FUN_00749508`).
                            let date = season.date();
                            crate::training_day::day(
                                records,
                                &mut season.clubs,
                                &self.data.world,
                                today,
                                date,
                                (
                                    &self.data.rules.discipline.mood,
                                    &self.data.rules.medical,
                                    &self.data.rules.league,
                                    &self.data.rules.individual,
                                    &self.data.rules.year,
                                ),
                                &mut season.recent_events,
                                &mut posted,
                                &mut season.random,
                            );
                        }
                    }
                }
                // The training's events come up as they happen, before
                // the booked ones (`FUN_005ef5cb`, event 0xfc).
                self.day_events(posted)?;
                self.open_events_on(walked)?;
                // The day's talks and the planners' appointments follow
                // the human clubs' training and the day's events
                // (`FUN_0064f981` 0x650285 `FUN_0076cb3e`, 0x650295
                // `FUN_0072fd53`), before the friendlies.
                self.appointments()?;
                // The betting shop's report of the matchday just paid
                // opens for every human the settlement marked a section
                // for (`FUN_0077f1b8` kind 1), on the half week the
                // settlement ran.
                if walked % 7 == 0 {
                    let report = {
                        let mut guard = self.state();
                        let state = &mut *guard;
                        match (state.records.as_ref(), state.season.as_ref()) {
                            (Some(records), Some(season)) => (0..season.managers.len())
                                .filter(|&m| season.managers[m].club.is_some())
                                .filter_map(|m| {
                                    records
                                        .human_slot(m)
                                        .filter(|&h| crate::betting::report_open(records, h))
                                        .map(|_| m)
                                })
                                .collect::<Vec<usize>>(),
                            _ => Vec::new(),
                        }
                    };
                    for manager in report {
                        self.season(|s| s.current = manager);
                        self.ask(Request::with(BET_REPORT, Args::None))?;
                    }
                }
                let mut tables = Vec::new();
                let mut reported = Vec::new();
                let mut controls = Vec::new();
                let mut cups = None;
                {
                    let mut guard = self.state();
                    let state = &mut *guard;
                    if let (Some(season), Some(records)) = (state.season.as_mut(), state.records.as_mut()) {
                        let today = walked as i32;
                        // The day's friendlies and tournaments
                        // (`FUN_007615bb`): a Turnier's draw and its
                        // results come up for the humans taking part
                        // where the option shows them (`DAT_00b5d14f`,
                        // events 0x9f and 0xa0).
                        let shown = crate::exhibition::play_day(&self.data, records, today as i16, &mut season.random);
                        if records.globals.get::<u8>(TOURNAMENT_SCREENS) == 1 {
                            let drawn = shown.drawn.into_iter().map(|id| (dialogs::TOURNAMENT_DRAW, id));
                            let results = shown.results.into_iter().map(|id| (dialogs::TOURNAMENT_RESULT, id));
                            let halls = shown.halls.iter().map(|h| (dialogs::HALL_RESULT, h.id));
                            season.tournament_screens.extend(drawn.chain(results).chain(halls));
                            season.halls.extend(shown.halls);
                        }
                        // The day's news: `FUN_0064f981`'s second club
                        // loop, after the friendlies.
                        if let Some(gates) = &slots {
                            crate::training_day::news(records, today, gates, &mut season.random);
                        }
                        // The day's league games, behind its routine and
                        // on its own day: where the original lets its
                        // clubs play them on the pitch, the replica scores
                        // them into the league blocks and their clubs'
                        // records — the human's game among them, its 3D
                        // match being out of the replica's scope. The
                        // routine done, the day's list is drawn again and
                        // played (`FUN_0062a325(day)`, `FUN_00710839` when
                        // it holds any): its marks are the doping pairs
                        // the matches take.
                        let listed = crate::matches::list_day(records, &self.data.calendar, season.day, &mut season.random);
                        // The last information before the humans'
                        // matches is taken as they stand before the
                        // first is played (`FUN_006ef296`'s records,
                        // `FUN_006fa448`), the crowd once they are.
                        let fixtures = crate::matches::human_fixtures(
                            records,
                            &self.data.world,
                            &self.data.calendar,
                            season.day,
                        );
                        let mut pages = if crate::prematch::live(records, season.day) {
                            crate::prematch::pages(records, &self.data, season, &fixtures)
                        } else {
                            Vec::new()
                        };
                        // The breaks at half time likewise, one for
                        // each human side (`FUN_0064987a`).
                        let mut breaks = crate::halftime::breaks(records, season.day, &fixtures);
                        // A camp booked before the match works on
                        // each side as the day's matches begin, home
                        // first (`FUN_006ef296`'s `FUN_0059a2ea`).
                        for (sides, _) in &fixtures {
                            for &side in sides {
                                crate::training_day::camp_before_match(
                                    records,
                                    &mut season.clubs,
                                    &self.data.world,
                                    side,
                                    &self.data.rules.discipline.mood,
                                    &mut season.random,
                                );
                            }
                        }
                        let played = crate::matches::play_day(
                            records,
                            &self.data.world,
                            &self.data.calendar,
                            season,
                            &self.data.rules.individual,
                            &listed,
                        );
                        crate::prematch::crowds(&mut pages, &played.faced);
                        season.prematch = pages;
                        crate::halftime::scores(&mut breaks, &played.faced);
                        season.halftime = breaks;
                        season.halftime_at = 0;
                        // The matchday's end lists the leagues for
                        // the table after it (`FUN_007108d3`'s
                        // `FUN_00729216`), once the results stand.
                        tables = crate::matches::tables_after(records, &played.leagues);
                        // The reports of the humans' matches stand
                        // written (`FUN_00550f29`); the Spielbericht
                        // follows from the season's 42nd day, before
                        // it only with a full preparation
                        // (`FUN_006ef262`, event 0xce), and only with
                        // the game's switch for it on (`FUN_00551818`).
                        if reports_shown(records, season.day) {
                            reported = played.reported.clone();
                        }
                        // The doping controls of the humans' clubs
                        // (event 0xcb each, `FUN_005dae5f`).
                        controls = played.controls.clone();
                        // The day's cup rounds, their next drawn
                        // (`FUN_0062eac3`, event 0xc9).
                        cups = crate::cup_results::day(records, &self.data.calendar, season.day);
                        // The clubs note their next matches anew
                        // (`FUN_0062c74b`'s `FUN_0062bfc9`), a cup tie
                        // among them opening its sales record.
                        crate::year::note_next_matches(records, &self.data, season.day, &mut season.random);
                        // The interview after a human's match
                        // (`FUN_007108d3`'s tail, `FUN_00660a72`): the
                        // first match that draws one, its questions
                        // read anew from the file.
                        let fixtures = crate::interview::fixtures(records, &played.leagues);
                        if let Some(due) = crate::interview::due(records, &fixtures, &mut season.random) {
                            season.interview = crate::interview::Interview::begin(
                                records,
                                &self.data.rules,
                                a3_files::interview::load(&self.data.dir),
                                due,
                                &mut season.interview_recent,
                                &mut season.random,
                            );
                        }
                    }
                }
                // The matches' dialogs come up as `FUN_00710839` posts
                // them, on the day they are played, before the youth
                // leagues and the world's day move it on.
                // The last information before the match (event 0x74,
                // `FUN_004cb878`): a page for each human club's match.
                if self.state().season.as_ref().is_some_and(|s| !s.prematch.is_empty()) {
                    self.ask(Request::new(PREMATCH))?;
                    if let Some(season) = self.state().season.as_mut() {
                        season.prematch.clear();
                    }
                }
                // Half time of each human side's match (event 0xb6,
                // `FUN_0064a816`): the break until it is done, then the
                // talk, whose line OK keeps.
                let breaks = self.state().season.as_ref().map_or(0, |s| s.halftime.len());
                for at in 0..breaks {
                    self.season(|s| s.halftime_at = at);
                    self.ask(Request::new(HALFTIME))?;
                    if let Reply::Choice(talk) = self.ask(Request::new(HALFTIME_TALK))? {
                        self.season(|s| {
                            if let Some(b) = s.halftime.get_mut(at) {
                                b.talk = u8::try_from(talk).unwrap_or(0);
                            }
                        });
                    }
                }
                // The doping control's results (event 0xcb, dialog 41),
                // one for each human club tested.
                for control in controls {
                    self.ask(Request::with(DOPING_CONTROL, Args::Doping(control)))?;
                }
                // The match report first (event 0xce), its clubs' crests
                // to step between them.
                if !reported.is_empty() {
                    let mut clubs = [(0, 0); 4];
                    for (slot, &club) in clubs.iter_mut().zip(&reported) {
                        *slot = club;
                    }
                    let count = reported.len().min(clubs.len()) as u8;
                    self.ask(Request::with(MATCH_REPORT, Args::Reports { clubs, count }))?;
                }
                // The table after the matchday, its leagues one by one
                // (event 200, `FUN_00729370`).
                if !tables.is_empty() {
                    let mut leagues = [(0, 0); crate::matches::TABLES_MOST];
                    for (slot, &league) in leagues.iter_mut().zip(&tables) {
                        *slot = league;
                    }
                    let count = tables.len() as u8;
                    self.ask(Request::with(TABLES_AFTER, Args::Tables { leagues, count }))?;
                }
                // The cup results behind it (event 0xc9, `FUN_0052b2bf`).
                if let Some(cups) = cups {
                    self.ask(Request::with(CUP_RESULTS, Args::CupResults(cups)))?;
                }
                // The interview behind it (event 0x126, dialog 383): the
                // dialog plays it through; the reporter keeps his mood.
                if self.state().season.as_ref().is_some_and(|s| s.interview.is_some()) {
                    self.ask(Request::new(INTERVIEW))?;
                    let mut guard = self.state();
                    let state = &mut *guard;
                    if let (Some(season), Some(records)) = (state.season.as_mut(), state.records.as_mut())
                        && let Some(interview) = season.interview.take()
                    {
                        interview.finish(records);
                    }
                }
                let week_start = {
                    let mut guard = self.state();
                    let state = &mut *guard;
                    state.season.as_mut().map(|season| {
                        let today = walked as i32;
                        if let Some(records) = state.records.as_mut() {
                            // The youth leagues play behind the matches
                            // (`FUN_0062a959`), the nations their owed
                            // international dates on the weekdays 3 and
                            // 6 (`FUN_006bd980`).
                            crate::youth_league::play_day(
                                records,
                                &self.data.calendar,
                                season.day,
                                &mut season.random,
                            );
                            crate::internationals::play_day(
                                records,
                                today,
                                &mut season.random,
                            );
                            // The day of the world closes it
                            // (`FUN_006504b6` with the weekday); the
                            // original's job steps stand between,
                            // unported.
                            crate::world_day::day(
                                records,
                                today.rem_euclid(7),
                                &mut season.random,
                            );
                        }
                        // The followed clubs stand as the day left their
                        // records; only a new game, which keeps none yet,
                        // passes its day on the replica's own model.
                        match state.records.as_ref() {
                            Some(records) => season.follow_records(records, &self.data),
                            None => season.pass_day(),
                        }
                        season.day += 1;
                        // The day's match clears the load's hold
                        // (`DAT_00b5cc04 = 0` in `FUN_006da3a9`'s case
                        // 3): the boundary's block below then rolls the
                        // market even in the week a game was loaded in.
                        season.week_hold = false;
                        let start = season.day % WEEK_DAYS == 0;
                        if start {
                            season.begin_week();
                        }
                        start.then_some(season.day / WEEK_DAYS)
                    })
                };
                if let Some(Some(_)) = week_start
                    && let Some(records) = self.state().records.as_mut()
                {
                    crate::noracsa::age_requests(records);
                }
                self.ask(Request::new(week))?;
                // The week's boundary was reached: turns, week's end and
                // days come round again. (Without a season the loop ends
                // too — it cannot walk any more days.)
                if !matches!(week_start, Some(None)) {
                    break;
                }
            }
        }
    }

    /// The season's end: the yearly statement for every manager with a
    /// club, dialog 417 opened by the event 0x12d the season end posts
    /// (`FUN_006742e0` in `FUN_006da3a9`'s season-end block, `FUN_004a141c`
    /// for the dialog). It is due once the played country's top league has
    /// no matchday ahead of the game. Every club of the countries played
    /// settles its season before (`FUN_00621909`, [`crate::settlement`]),
    /// so that the statement tells the tax booked; behind the statements
    /// the new season opens ([`crate::settlement::open_season`]) and the
    /// game walks on from its day 0, the statement free to come again at
    /// the next season's end. The promotions the original draws beside
    /// the opening are not built here, and the statement shows once in a
    /// loaded game (`season.statement`).
    fn statement(&mut self) -> Result<(), Gone> {
        let ready = {
            let state = self.state();
            state.season.as_ref().is_some_and(|season| {
                !season.statement
                    && self
                        .data
                        .world
                        .country(season.main)
                        .and_then(|country| {
                            Some(self.data.calendar.row(u16::try_from(country.nation).ok()?, 0))
                        })
                        .flatten()
                        .is_some_and(|row| {
                            self.data.calendar.next_matchday(row, season.day + 1).is_none()
                        })
            })
        };
        if !ready {
            return Ok(());
        }
        self.season(|s| s.statement = true);
        {
            // The leagues draw next season's clubs (`FUN_006b24f2`), the
            // seasons go into their archive and the first leagues' into
            // the all-time tables, the clubs take their season's flags
            // (`FUN_006b2e88`, `FUN_006b3111`, `FUN_006b2543`), the kept
            // countries' players close their
            // season beside the settlement (`FUN_006b3ee0`, between the
            // humans' pay and the clubs' books in the original's
            // season-end block).
            let mut guard = self.state();
            let state = &mut *guard;
            if let (Some(season), Some(records)) = (state.season.as_mut(), state.records.as_mut()) {
                let drawn = crate::promotion::draw(records, &mut season.random);
                crate::promotion::count(records);
                // Every trial at the human clubs ends with the season
                // (`FUN_0074d52b`).
                crate::youth::end_trials(records, &mut season.clubs, &self.data.world, true);
                crate::statistics::archive_strengths(records);
                crate::development::develop_played(records, false, &mut season.random);
                crate::rollover::retire_announced(records, &mut season.random);
                crate::statistics::archive_leagues(records);
                crate::statistics::fold_eternal(records);
                crate::promotion::flag(records, &self.data.rules.forecast.tickets);
                crate::europe_entry::qualify(records, &mut season.random);
                crate::rollover::count_first_seasons(records);
                let mut agents = crate::free_agents::release(records, &mut season.random);
                crate::deals::execute_with(records, &mut season.random, &mut agents);
                // Every club's verdict on its season against its goal
                // (`FUN_006b44cb`'s `FUN_00775fd7`).
                crate::season::close_goals(records, &self.data.rules);
                crate::rollover::close_played(records, &mut season.random);
                crate::rollover::close_kept(records, &mut season.random);
                let rules = &self.data.rules;
                crate::free_agents::place(records, &agents, (&rules.contract, &rules.value), &mut season.random);
                crate::settlement::settle_season(records, &self.data.rules.league);
                // The humans whose club the season's books put up count the
                // promotion in their own career records (`FUN_006b4e2f`
                // reading the promoted flag `FUN_006b2e88` set, between the
                // settlement and the managers' season end).
                crate::career::promotions(records);
                crate::settlement::close_managers(records, &mut season.random);
                crate::statistics::roll_ranking(records);
                // The transfer list closes the season (`FUN_00428fa2`).
                crate::transfer_week::close_season(records, &mut season.random);
                // The season's flags and places move the managers' standing
                // (`FUN_0041786a`, scheduler case 5) before the leagues
                // take their new clubs.
                crate::standing::season_end(records);
                // The original's leagues take their new clubs before its
                // settlement, which reads the places the archive kept;
                // the replica's settlement reckons the closed tables
                // itself, so they take them behind it.
                crate::promotion::take(records, &drawn);
            }
        }
        let humans = self.season(|s| {
            (0..s.managers.len())
                .filter(|&m| s.managers[m].club.is_some())
                .collect::<Vec<usize>>()
        })
        .unwrap_or_default();
        for manager in humans {
            self.season(|s| s.current = manager);
            self.ask(Request::with(FINANCES, Args::Statement))?;
        }
        // The new season's first day follows the statements, where the
        // original's season-end block hands the game to its new-season
        // block (`FUN_006da3a9`): the counters roll, the July calendar
        // stands, the leagues open and the clubs' bookings clear
        // ([`crate::settlement::open_season`]) — the game walks on from
        // the new season's day 0.
        {
            let mut guard = self.state();
            let state = &mut *guard;
            if let (Some(season), Some(records)) =
                (state.season.as_mut(), state.records.as_mut())
            {
                let rules = &self.data.rules;
                crate::settlement::open_season_with(records, &mut season.random, Some(rules));
                // Its computer clubs line up for the season
                // (`FUN_0077655c`'s `FUN_0042a76e`).
                crate::autolineup::computer_lineups(records, &self.data);
                // Every club keeps its squad's strength, and the first
                // leagues take the odds of the championship (`FUN_0077655c`).
                crate::season_odds::open_season(records, &self.data, &mut season.random);
                // The game's best players of each position and ability are
                // picked anew (`FUN_0077841a`).
                crate::stars::pick(records, &self.data);
                // The humans' calendars start empty and take their clubs'
                // fixtures of the season (`FUN_00775219`, `FUN_0072f6f3`).
                crate::year::open_season(records, &self.data.calendar);
                // Every club notes its first match of the season
                // (`FUN_0062dfa3(0)`'s `FUN_0062bfc9`).
                crate::year::note_next_matches(records, &self.data, 0, &mut season.random);
                // The human clubs' squads changed with the turn — players
                // gone, new ones come — so each club the season follows
                // stands anew as its records now have it, as a load would
                // set it up.
                let keys: Vec<(usize, usize)> = season.clubs.keys().copied().collect();
                for key in keys {
                    if let Some(club) = crate::load::club(records, &self.data, key) {
                        season.clubs.insert(key, club);
                    }
                }
                season.year = i32::from(records.globals.start_year());
                season.day = 0;
                season.statement = false;
            }
        }
        Ok(())
    }

    /// The rolling autosaves of the game being played (`FUN_006ecfeb`):
    /// of the three kept under `saves` the oldest goes, its two fellows
    /// move up a place, and the game writes itself afresh into the
    /// newest. The week's end asks for them before its announcement
    /// (`FUN_0064e40f`); the original also holds them back while its load
    /// stands uncleared (`DAT_00b5cc04`, which `CDiLoad` sets and the
    /// sales clear), the same hold as `Season::week_hold`. The counter the
    /// original weighs, "half
    /// weeks since the start" at `0xb5cbd2`, ticks on the week's middle
    /// day and its end, so half of it is the whole weeks counted here.
    fn autosave(&self, records: &Records, weeks: usize) {
        if !autosave_due(&records.globals.game_options(), weeks) {
            return;
        }
        let fresh = self.data.saves.join("Autos.an3");
        match records.to_save().to_bytes_stored() {
            Ok(bytes) => {
                if let Err(err) = roll_autosaves(&self.data.saves, &bytes) {
                    warn!("{}: {err}", fresh.display());
                }
            }
            Err(err) => warn!("{}: {err}", fresh.display()),
        }
    }

    /// The careers ended (`FUN_006c5190`): each goes into the highscore
    /// list of the game's mode (`FUN_00656e9b`), kept as its file is to
    /// hold it at the end, and its manager leaves the game — his slot
    /// closed, the humans after him moving up in the order, the clubs that
    /// engaged him for next season and the other humans' +0x1b0 naming him
    /// let go of him (`FUN_006a40ce`), his club no longer followed as a
    /// human's. The national team lets him go first (`FUN_006b9872`,
    /// reason 0); of his club's own goodbye (`FUN_006da1ac`, reason 4)
    /// only its notice is built (`FUN_005dfec7`). Whether a career
    /// ended, and whether a human is left.
    fn careers_end(&self) -> (bool, bool) {
        let mut guard = self.state();
        let state = &mut *guard;
        let leaving = std::mem::take(&mut state.leaving);
        let Some(season) = state.season.as_mut() else { return (false, true) };
        if leaving.is_empty() {
            return (false, !season.managers.is_empty());
        }
        let today = season.date();
        let mut lists = Vec::new();
        if let Some(records) = state.records.as_mut() {
            let slots: Vec<_> = leaving.iter().map(|&(n, date)| (records.human_slot(n), date)).collect();
            for &(slot, date) in &slots {
                let Some(slot) = slot else { continue };
                let mode = records.globals.mode();
                let Some(file) = crate::highscore::file(mode) else { continue };
                let manager = &records.managers[slot];
                let born = chrono::NaiveDate::from_ymd_opt(
                    i32::from(manager.birth_year()),
                    u32::from(manager.birth_month()),
                    u32::from(manager.birth_day()),
                );
                let career = crate::highscore::Career {
                    manager,
                    // `FUN_006a426b(n)` takes the record of slot n − 1.
                    points: crate::evaluation::marks(records, manager, slot + 1).iter().sum(),
                    age: crate::autolineup::age(born, today),
                    date,
                };
                // A list entered before in this session stands in for its
                // file, which is written only at the end.
                let list = lists
                    .iter()
                    .rev()
                    .chain(state.settings_files.iter())
                    .find(|(f, _)| *f == file)
                    .map(|(_, bytes)| bytes.clone())
                    .or_else(|| std::fs::read(self.data.dir.join(file)).ok())
                    .unwrap_or_default();
                if let Some(bytes) = crate::highscore::enter(mode, &list, &career) {
                    lists.push((file, bytes));
                }
            }
            for &(slot, _) in &slots {
                let Some(slot) = slot else { continue };
                crate::national::leave(records, slot, 0);
                // His club's goodbye (`FUN_006da1ac(manager, club, 1, 4,
                // 0x80)`, only with a club, manager +0x1e): so far the
                // notice of it (`FUN_005dfec7`) and the end of his last
                // club station in his career (`FUN_006fddfe`, reason 4)
                // and what leaving costs him (`FUN_006d9f70`), and the
                // club and its players let go (`FUN_006d9fa1`), and his
                // contract's end with his flat sold (`FUN_006da008`),
                // and the club's deposits dissolved (`FUN_005e0147`), and
                // the table of the human clubs rebuilt (`FUN_00635503`),
                // and the club put on the ticker of jobs (`FUN_005df799`).
                let manager = &records.managers[slot];
                if manager.country() != 0 {
                    let club = (manager.country() as u8, manager.team());
                    let notice = crate::goodbye::notice(records, slot, club, 4, 3, 0x80, 0);
                    season.notices.post(notice);
                    crate::career::left_club(records, slot, 4);
                    crate::career::leaving_costs(records, slot, (club.0, club.1 as u16));
                    crate::career::release_club(records, &mut season.clubs, &self.data.world, (club.0, club.1 as u16));
                    crate::career::end_contract(records, slot, true);
                    crate::career::dissolve_deposits(records, (club.0, club.1 as u16));
                    crate::career::rebuild_human_clubs(records);
                    crate::career::post_vacancy(&mut season.job_notes, slot, (club.0, club.1 as u16), 3);
                }
                leave(records, slot);
            }
        }
        let mut gone: Vec<usize> = leaving.iter().map(|&(n, _)| n).collect();
        gone.sort_unstable();
        gone.dedup();
        for &n in gone.iter().rev() {
            if n < season.managers.len() {
                let manager = season.managers.remove(n);
                if let Some(club) = manager.club {
                    season.clubs.remove(&club);
                }
            }
        }
        season.current = season.current.min(season.managers.len().saturating_sub(1));
        let left = !season.managers.is_empty();
        for (file, bytes) in lists {
            state.keep_settings(file, bytes);
        }
        (true, left)
    }

    /// The day's poaching talks, before the planners' pump
    /// (`FUN_0076cb3e` from the day loop `FUN_0064f981`): its first step
    /// zeroes the day's messages and talks (`FUN_0076fac0`), then every
    /// poaching appointment of the day leaves the list, and a human
    /// buyer's talk puts "Verhandlung" and the manager's name in the
    /// week's message and the player in his talk of the day
    /// (`FUN_0076aca0`, text 5048 via `FUN_0064f456`). Its message 0xa6
    /// opens the talk itself in that manager's turn (`FUN_0076ad3b`,
    /// `CVerhandlungAbwerbeversuch`), whose OK writes the talk agreed
    /// (kind 3, `FUN_0076ac2e`); after the day's messages, managers 1 to 4
    /// in turn act on a talk agreed (`FUN_0076aeb8`).
    fn poaching_talks(&self) -> Result<(), Gone> {
        let mut guard = self.state();
        let state = &mut *guard;
        let Some(season) = state.season.as_mut() else { return Ok(()) };
        season.planner_message = None;
        season.day_talks = Default::default();
        let Some(records) = state.records.as_mut() else { return Ok(()) };
        let Ok(day) = i16::try_from(season.day) else { return Ok(()) };
        let mut talks = Vec::new();
        for (player, human) in crate::transfers::poaching_day(records, day) {
            let slot = usize::from(human) - 1;
            if let Some(talk) = season.day_talks.get_mut(slot) {
                talk.player = player;
            }
            let n = (0..season.managers.len()).find(|&n| records.human_slot(n) == Some(slot));
            if let Some(manager) = n.and_then(|n| season.managers.get(n)) {
                season.planner_message =
                    Some((CONTRACT_TALK_TEXT, manager.last_name.clone()));
            }
            if let Some(n) = n {
                talks.push((n, slot, player));
            }
        }
        drop(guard);
        for (n, slot, player) in talks {
            self.season(|s| s.current = n);
            let reply = self.ask(Request::with(POACHING_TALK, Args::Record(player)))?;
            if reply == Reply::Ok
                && let Some(talk) = self.season(|s| s.day_talks.get_mut(slot).cloned()).flatten()
            {
                // The dialog left its agreed offer in the talk's terms.
                let agreed = crate::transfers::DayTalk { kind: crate::transfers::AGREED, ..talk };
                self.season(|s| s.day_talks[slot] = agreed);
            }
        }
        self.season(|s| s.current = 0);
        let mut guard = self.state();
        let state = &mut *guard;
        let (Some(season), Some(records)) = (state.season.as_mut(), state.records.as_mut()) else {
            return Ok(());
        };
        for slot in 0..season.day_talks.len() {
            let buyer = records
                .teams
                .iter()
                .find(|t| usize::from(t.human()) == slot + 1)
                .map(|t| (t.country(), i16::from(t.number())));
            if let Some(buyer) = buyer {
                let talk = season.day_talks[slot].clone();
                crate::transfers::after_poaching(records, &mut season.random, &talk, buyer);
            }
        }
        // The day's last loop: the loan talks leave the list and a loan no
        // human bids for finds its computer club (`FUN_00682814`); the
        // move itself and the human's bargaining (0x9b) are not built.
        // A human club out of a talk is told why at once (message 0x11f).
        let mut told = Vec::new();
        if let Ok(day) = i16::try_from(season.day) {
            crate::transfers::loan_day(records, day, &mut told);
        }
        drop(guard);
        self.purchase_talks(&mut told)?;
        for d in told {
            let text = if d.squad_full { SQUAD_FULL } else { BANK_NO_CREDIT };
            let args = Args::ClubAlert { title: ALERT_FOR, text, country: d.club.0, number: d.club.1 };
            self.ask(Request::with(NOTICE, args))?;
        }
        Ok(())
    }

    /// The day's purchase talks one by one (`FUN_0076cb3e`'s last loop),
    /// a fee talk with a human seller in his dialog 308 a round at a time:
    /// his answer, then the bidder's (`FUN_00768638` asm 0x7698d1–0x769b0b).
    fn purchase_talks(&self, told: &mut Vec<crate::transfers::Dropout>) -> Result<(), Gone> {
        let rules = &self.data.rules;
        loop {
            let next = {
                let mut guard = self.state();
                let state = &mut *guard;
                let (Some(season), Some(records)) = (state.season.as_mut(), state.records.as_mut()) else {
                    return Ok(());
                };
                let Ok(day) = i16::try_from(season.day) else { return Ok(()) };
                crate::transfers::next_purchase_talk(
                    records,
                    &mut season.random,
                    (&rules.contract, &rules.value),
                    &rules.medical,
                    &self.data.world.nation_kinds,
                    day,
                    told,
                )
            };
            let Some((player, talk)) = next else { return Ok(()) };
            if let Some(fee) = talk.and_then(|t| t.fee) {
                self.season(|s| s.fee_talk = Some(fee));
                loop {
                    self.ask(Request::new(FEE_TALK))?;
                    let ended = self
                        .season(|s| {
                            let fee = s.fee_talk.as_mut()?;
                            fee.after_seller().or_else(|| fee.bidder_turn(&mut s.random))
                        })
                        .flatten();
                    if ended.is_some() {
                        break;
                    }
                }
                self.season(|s| s.fee_talk = None);
            }
            let mut guard = self.state();
            if let Some(records) = guard.records.as_mut() {
                crate::transfers::close_purchase_talk(records, player);
            }
        }
    }

    /// What the managers booked for today in their planners with a player
    /// still in their squads, manager by manager (`FUN_0072fd53`): a talk
    /// opens in its manager's turn (`FUN_0048ddbe`); a sick visit happens
    /// (`FUN_00557e1f`) and tells how he took it — unless the game runs its
    /// preparation fast and the season is younger than day 42. The day's
    /// message starts fresh with its first step (`FUN_0076fac0` zeroes the
    /// day's twenty slots before its managers are walked), so the last
    /// manager with one stays on the week screen.
    fn appointments(&self) -> Result<(), Gone> {
        let managers = self.season(|s| s.managers.len()).unwrap_or(0);
        self.poaching_talks()?;
        for n in 0..managers {
            // The association's court sits for the manager's club
            // (`FUN_00628309`, its planner's +4 the bans it hears).
            let court = {
                let mut state = self.state();
                let state = &mut *state;
                let (Some(season), Some(records)) = (state.season.as_mut(), state.records.as_ref())
                else {
                    return Ok(());
                };
                season.current = n;
                records.human_slot(n).and_then(|slot| {
                    let booked = records.planners.get(slot)?.day(season.day);
                    let manager = records.managers.get(slot)?;
                    (booked.kind == crate::court::COURT && manager.country() != 0)
                        .then(|| ((manager.country() as u8, manager.team() as i16), booked.player))
                })
            };
            if let Some((club, kind)) = court {
                self.court(club, kind)?;
                continue;
            }
            let booked = {
                let mut state = self.state();
                let state = &mut *state;
                let (Some(season), Some(records)) = (state.season.as_mut(), state.records.as_mut())
                else {
                    return Ok(());
                };
                season.current = n;
                let manager = &season.managers[n];
                let last_name = manager.last_name.clone();
                let club = manager.club;
                let booked = records
                    .human_slot(n)
                    .and_then(|slot| records.planners.get(slot))
                    .map(|p| p.day(season.day))
                    .filter(|b| b.player != 0);
                // The professional contract's talk on its day: the
                // original's day run walks the talks list the booking
                // wrote beside the planner and looks him up in the
                // club — the youth in its squad, the amateur in its
                // amateur pool — before it posts the week's message
                // (`FUN_0076cb3e`, `FUN_0076bb38`, `FUN_0076c613`);
                // the planner's own pump ignores these kinds. The
                // talk's own screen is not traced, so the replica
                // stops at the message.
                let talk = booked
                    .filter(|b| CONTRACT_TALKS.contains(&b.kind))
                    .filter(|b| {
                        if b.kind == youth::YOUTH_CONTRACT_TALK {
                            club.and_then(|key| season.clubs.get(&key)).is_some_and(|c| {
                                c.status.iter().any(|s| s.id == b.player)
                            })
                        } else {
                            records
                                .human_slot(n)
                                .and_then(|slot| records.managers.get(slot))
                                .is_some_and(|m| {
                                    youth::pool(records, m.country() as u8, m.team() as u16)
                                        .iter()
                                        .any(|list| list.contains(&b.player))
                                })
                        }
                    });
                if let Some(b) = talk {
                    let name = records
                        .player(b.player)
                        .map(|r| records.name(r.last_name()).to_owned());
                    season.planner_message = Some((CONTRACT_TALK_TEXT, name.unwrap_or_default()));
                }
                let status = booked
                    .filter(|b| !CONTRACT_TALKS.contains(&b.kind))
                    .and_then(|b| {
                        let club = season.clubs.get(&club?)?;
                        club.status.iter().position(|s| s.id == b.player).map(|p| (b, p))
                    });
                match status {
                    Some((b, _)) if b.kind == TALK => {
                        season.planner_message = Some((TALK_TEXT, last_name));
                        Some((TALK_DIALOG, Args::Record(b.player)))
                    }
                    Some((b, p)) if b.kind == VISIT => {
                        let Some(key) = club else { continue };
                        let days_out = season.clubs[&key].status[p].injured;
                        let Some(r) = records.players.get_mut(usize::from(b.player)) else {
                            continue;
                        };
                        let reaction = medical::visit(r, &mut season.random, days_out);
                        let morale = i32::from(r.morale());
                        if let Some(fitness) =
                            season.clubs.get_mut(&key).and_then(|c| c.fitness.get_mut(p))
                        {
                            fitness.morale = morale;
                        }
                        season.planner_message = Some((VISIT_TEXT, last_name));
                        let told = records.globals.preparation() == NORMAL_PREPARATION
                            || season.day >= TOLD_FROM;
                        told.then_some((
                            dialogs::VISIT,
                            Args::Visit {
                                player: b.player,
                                kind: reaction.kind,
                                draw: reaction.draw,
                            },
                        ))
                    }
                    _ => None,
                }
            };
            if let Some((dialog, args)) = booked {
                self.ask(Request::with(dialog, args))?;
            }
        }
        self.season(|s| s.current = 0);
        Ok(())
    }

    /// The court's sitting for a club: each case it tells opens dialog 89
    /// at once (event 0xa3 run by `FUN_006c03ba`), its OK or Nein the
    /// answer to an appeal's question; a computer club takes the lawyers'
    /// advice.
    fn court(&self, club: (u8, i16), kind: u16) -> Result<(), Gone> {
        let mut hearing = crate::court::Hearing::new(club, kind);
        loop {
            let case = {
                let mut guard = self.state();
                let state = &mut *guard;
                let (Some(season), Some(records)) = (state.season.as_mut(), state.records.as_mut())
                else {
                    return Ok(());
                };
                hearing.next(records, &mut season.random)
            };
            let Some(case) = case else { return Ok(()) };
            let yes = if case.shown {
                self.ask(Request::with(COURT, Args::Court(case)))? == Reply::Ok
            } else {
                case.advised
            };
            if let Some(records) = self.state().records.as_mut() {
                crate::court::answer(records, &case, yes);
            }
        }
    }

    /// What the year calendars booked on the day just walked, manager by
    /// manager: save A's day 63 holds the open day, entry kind 8 with the
    /// open day's number 6 where a talk's would name the player. The
    /// dialog comes up in that day's walk, after the day's training and
    /// before its games; its constructor draws nothing. The eight draws
    /// of 9 or 10 the traced original makes at the load, before the turn's
    /// screen, are not the open day's but the week end's look at its last
    /// day's matches ([`crate::matches::list_day`]), left out after a load.
    /// The day's posted events, each in the window its text fits
    /// (`FUN_005f2234`); none in the first season's quiet days
    /// (`FUN_005ef56c`).
    fn day_events(&self, posted: Vec<crate::event_text::Posted>) -> Result<(), Gone> {
        if posted.is_empty() {
            return Ok(());
        }
        let Some(texts) = a3_files::saq::EventTexts::load(&self.data.dir) else { return Ok(()) };
        for p in posted {
            let dialog = {
                let state = self.state();
                let Some(records) = state.records.as_ref() else { return Ok(()) };
                if crate::event_text::quiet(records) {
                    return Ok(());
                }
                crate::event_text::make(records, &texts, &p).dialog()
            };
            self.ask(Request::with(dialog, Args::Event(p)))?;
        }
        Ok(())
    }

    fn open_events_on(&self, day: usize) -> Result<(), Gone> {
        let managers = self.season(|s| s.managers.len()).unwrap_or(0);
        for n in 0..managers {
            let booked = {
                let mut state = self.state();
                let state = &mut *state;
                let (Some(season), Some(records)) = (state.season.as_mut(), state.records.as_ref())
                else {
                    return Ok(());
                };
                season.current = n;
                records
                    .human_slot(n)
                    .and_then(|slot| records.calendars.get(slot))
                    .is_some_and(|c| {
                        let entry = c.day(day);
                        entry.kind == CALENDAR_EVENT && entry.player == OPEN_DAY_EVENT
                    })
            };
            if booked {
                self.ask(Request::new(OPEN_DAY))?;
            }
        }
        self.season(|s| s.current = 0);
        Ok(())
    }

    /// The week after the turns: its screen. The week of a match now runs
    /// with the rest — the league matchdays are played as the days pass.
    fn start_week(&self, season: &mut Season) -> u16 {
        season.current = 0;
        season.start_week();
        let clubs = season.managers.iter().filter(|m| m.club.is_some()).count();
        WEEKS[clubs.min(WEEKS.len() - 1)]
    }

    /// The current manager's turn, on the main screens with the menu bar or
    /// in the office, until it is ended; false once the game is given up.
    fn turn(&self) -> Result<bool, Gone> {
        let Some(mut screen) = self.season(|s| self.turn_screen(s)) else { return Ok(true) };
        // The next match within the week is announced before all else, when
        // "Spielankündigung vor jedem Spiel" is on (`FUN_00650b84`).
        if let Some(announcement) = self.announcement() {
            self.ask(Request::new(announcement))?;
        }
        // Then the records he broke, a window each over the 3D picture
        // (`FUN_00650b84` asm 0x65116f).
        for broken in self.records_broken() {
            self.ask(Request::with(crate::render::RENDER, Args::Render(broken)))?;
        }
        // A manager with a note on his page reads it before his turn screen:
        // the hub opens the notepad, and only then its own dialog — office,
        // text menu or line-up, all guarded by the same `strlen`
        // (`CMyStartDialog::FUN_00650b84`; behind it a further guard, a
        // manager field +0x2128 not yet known, may skip both).
        if self.a_note_waits() {
            self.ask(Request::new(NOTEPAD))?;
        }
        loop {
            match self.ask(Request::new(screen))? {
                Reply::Goto(next) => {
                    screen = self.season(|s| line_up_view(s, next)).unwrap_or(next);
                }
                // "Spiel beenden": the career ends as "Karriereende" ends
                // it, at once, and the cycle with it — and the program,
                // before the highscores can show it, as the original's
                // goes straight to its extro.
                Reply::Quit => {
                    self.careers_end();
                    return Err(Gone);
                }
                // "Neues Spiel": its flag 8 ends the game cycle, and the
                // main loop goes back to the start menu (`FUN_00788115`).
                Reply::NewGame => return Ok(false),
                _ => {
                    // The turn does not end for a national coach whose
                    // squad is too small for his team's match this week:
                    // the week hub asks an international window on the
                    // week's Wednesday too, alerts with text 4966, and
                    // code −1600 opens his menu again (`FUN_00650b84` asm
                    // 0x6523d3); the hub of a coach without a club gates
                    // on the match alone and alerts for nothing
                    // (`FUN_0065270d` asm 0x65292f).
                    let (nation, club) = self
                        .season(|s| s.manager().map(|m| (m.national_team, m.club.is_some())))
                        .flatten()
                        .unwrap_or((None, true));
                    let blocked = nation.is_some_and(|nation| {
                        let guard = self.state();
                        guard.records.as_ref().is_some_and(|records| {
                            if club {
                                national::squad_blocks_turn(
                                    records,
                                    &self.data.calendar,
                                    nation as i16,
                                )
                            } else {
                                national::squad_blocks_clubless_turn(records, nation as i16)
                            }
                        })
                    });
                    if !blocked {
                        return Ok(true);
                    }
                    if club {
                        self.ask(Request::with(NOTICE, Args::Text(NOT_ENOUGH_PLAYERS)))?;
                    }
                    screen = NATIONAL_MENU;
                }
            }
        }
    }

    /// Where a manager's turn opens: the office for one who chose it — the
    /// grand one for the grandest clubs — the text menu for one who chose
    /// that, else the line-up, or the dressing room for a manager without
    /// a club.
    fn turn_screen(&self, season: &Season) -> u16 {
        let Some(manager) = season.manager() else { return DRESSING_ROOM };
        if manager.menu == TEXT_MENU_CHOICE {
            return TEXT_MENU;
        }
        if manager.menu == OFFICE_MENU {
            let club =
                manager.club.and_then(|(slot, c)| self.data.world.country(slot)?.clubs.get(c));
            return if club.is_some_and(|c| c.class >= GRAND_CLASS) {
                GRAND_OFFICE
            } else {
                OFFICE
            };
        }
        if manager.club.is_some() { line_up_view(season, LINEUP) } else { DRESSING_ROOM }
    }

    /// The announcement the current manager's turn opens with, if any: the
    /// game's switch for it on (`0xb5d13e`), and his club's next match
    /// within a week of today (club +0xb46, +0xb44) — 358 for a league
    /// match, else 201.
    fn announcement(&self) -> Option<u16> {
        let state = self.state();
        let (season, records) = (state.season.as_ref()?, state.records.as_ref()?);
        let (slot, number) = season.manager()?.club?;
        let nation = self.data.world.country(slot)?.nation;
        let team = records.team(nation, u16::try_from(number).ok()?)?;
        let today = records.globals.day() as i16;
        let due = records.globals.game_options()[ANNOUNCE_OPTION] == ON
            && team.next_country() != 0
            && team.next_day() < today + ANNOUNCE_AHEAD;
        let league = team.next_kind() == LEAGUE_MATCH;
        due.then_some(if league { LEAGUE_ANNOUNCEMENT } else { MATCH_ANNOUNCEMENT })
    }

    /// The records the current manager broke that open their windows, his
    /// word of them cleared: the club manager's hub alone looks
    /// (`FUN_00650b84`), not the clubless one (`FUN_0065270d`) nor the
    /// turns before the matches (`FUN_00652a86`) — save "Toni", saved in
    /// such a turn with the record set, opens on its line-up in the
    /// original and shows the record at the next turn.
    fn records_broken(&self) -> Vec<crate::render::Broken> {
        let mut guard = self.state();
        let state = &mut *guard;
        let (Some(season), Some(records)) = (state.season.as_ref(), state.records.as_mut()) else {
            return Vec::new();
        };
        if season.before_match || season.manager().and_then(|m| m.club).is_none() {
            return Vec::new();
        }
        let Some(slot) = records.human_slot(season.current) else { return Vec::new() };
        crate::render::take_broken(records, slot)
    }

    /// Whether the current manager left text on his notepad page: what
    /// the hub's `strlen` of `DAT_008ed400 + (human − 1) × 0x420` tests.
    fn a_note_waits(&self) -> bool {
        let state = self.state();
        let Some(season) = state.season.as_ref() else { return false };
        let Some(records) = state.records.as_ref() else { return false };
        records
            .human_slot(season.current)
            .and_then(|page| records.notes.get(page))
            .is_some_and(|note| !note.text().is_empty())
    }

    /// A saved game: read, and on with its weeks; for a network game, where
    /// it would be hosted.
    fn load(&mut self) -> Result<(), Gone> {
        if !self.ok(LOAD)? {
            return Ok(());
        }
        if self.state().mode == Mode::ServerLoad {
            return self.network_games();
        }
        let name = self.state().save.clone().unwrap_or_default();
        let path = self.data.dir.join("saves").join(format!("{name}.an3"));
        let records = match Save::read(&path).and_then(Records::from_save) {
            Ok(records) => records,
            Err(err) => {
                warn!("{}: {err}", path.display());
                return Ok(());
            }
        };
        let season = load::season(&records, &self.data, Random::new((self.seed)()));
        info!("Spielstand \"{name}\" geladen: Tag {}, {}", season.day, season.date());
        {
            let mut state = self.state();
            state.season = Some(season);
            // The play time it was saved with (`0xae6e34`), counting on from
            // now (`0xae6e30`).
            let stored = records.block(a3_files::an3::REAL_TIME[0]).and_then(|b| b.get(..4));
            let stored = stored.map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
            state.play.game_loaded(stored);
            state.records = Some(records);
        }
        self.weeks(false)
    }

    /// A network game: the player's name, the details for hosting one, and
    /// the list of games.
    fn network(&mut self) -> Result<(), Gone> {
        if !self.ok(NET_NAME)? {
            return Ok(());
        }
        if self.state().mode != Mode::Client && !self.ok(NET_DETAILS)? {
            return Ok(());
        }
        self.network_games()
    }

    fn network_games(&mut self) -> Result<(), Gone> {
        if self.ok(NET_GAMES)? {
            would_start("Netzwerkspiel");
        }
        Ok(())
    }
}

/// The line-up in the view the current manager has it in: on the pitch or
/// as a list (`FUN_006a5ba7`, every turn's hub); any other screen as it is.
fn line_up_view(season: &Season, screen: u16) -> u16 {
    let manager = season.manager();
    match screen {
        LINEUP if manager.is_some_and(|m| m.list_view) => LINEUP_LIST,
        SHORT_TRAINING if manager.is_some_and(|m| m.training_pictures) => TRAINING_PICTURES,
        _ => screen,
    }
}

/// Where the game itself would begin, which is not built yet; back to the
/// start menu.
fn would_start(what: &str) {
    info!("{what}: here the game itself would begin; back to the start menu");
}

/// Whether the week's end rolls the autosaves: the options' switch on byte
/// 0x0f standing on (`DAT_00b5d14b`), and byte 0x1f (`DAT_00b5d15b`, the
/// dressing room's spinner 6348) telling every how many weeks — its zero
/// saving every week (`FUN_0064e40f` weighing half of `0xb5cbd2`).
fn autosave_due(options: &[u8; 39], weeks: usize) -> bool {
    let every = usize::from(options[AUTOSAVE_WEEKS]);
    options[AUTOSAVE] == ON && (every == 0 || weeks % every == 0)
}

/// `FUN_006ecfeb` over the game's saves: `DeleteFileA` on the oldest
/// `Auto2.an3`, `Auto1.an3` renamed into its place and `Autos.an3` into
/// `Auto1`'s, and the bytes written as the fresh `Autos.an3`.
fn roll_autosaves(saves: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::remove_file(saves.join("Auto2.an3"))
        .or_else(|err| if err.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(err) })?;
    for (from, to) in [("Auto1.an3", "Auto2.an3"), ("Autos.an3", "Auto1.an3")] {
        std::fs::rename(saves.join(from), saves.join(to))
            .or_else(|err| if err.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(err) })?;
    }
    std::fs::write(saves.join("Autos.an3"), bytes)
}

/// What the other humans' records hold of a human, by his number: +0x1b0.
const NAMES_HUMAN_AT: usize = 0x1b0;
/// The field after the team's `engaged`, which goes with it.
const ENGAGED_WITH_AT: usize = 0xb0;

/// The human of `slot` leaves the records (`FUN_006a40ce` and what
/// `FUN_006c5190` does around it): the humans after him in the order move
/// up a place, which leaves −1 at its end, and his slot closes — only
/// when he stands in the order at all; the humans count one fewer; the
/// other humans' +0x1b0 and the clubs that engaged him for next season let
/// go of his number.
fn leave(records: &mut Records, slot: usize) {
    let human = slot as i16 + 1;
    let mut order = records.globals.order();
    if let Some(at) = order.iter().position(|&h| h == human) {
        order.copy_within(at + 1.., at);
        order[order.len() - 1] = -1;
        records.globals.set_order(order);
        let mut slots = records.globals.slots();
        slots[slot] = 0;
        records.globals.set_slots(slots);
    }
    records.globals.set_humans(records.globals.humans().saturating_sub(1));
    for manager in &mut records.managers {
        if manager.get::<i16>(NAMES_HUMAN_AT) == human {
            manager.put(NAMES_HUMAN_AT, 0i16);
        }
    }
    for team in &mut records.teams {
        if team.engaged() == -human {
            team.set_engaged(0);
            team.put(ENGAGED_WITH_AT, 0i16);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_week_rolls_the_autosaves_on_the_options() {
        let mut options = [0_u8; 39];
        assert!(!autosave_due(&options, 1), "the switch off keeps them away");
        options[AUTOSAVE] = ON;
        options[AUTOSAVE_WEEKS] = 1;
        for weeks in 1..=12 {
            assert!(autosave_due(&options, weeks), "week {weeks}");
        }
        options[AUTOSAVE_WEEKS] = 0;
        assert!(autosave_due(&options, 7), "no number means every week");
        options[AUTOSAVE_WEEKS] = 3;
        assert!(autosave_due(&options, 6) && autosave_due(&options, 9));
        assert!(!autosave_due(&options, 7) && !autosave_due(&options, 8));
    }

    #[test]
    fn three_rolling_autosaves() {
        let dir = std::env::temp_dir().join("a3-autosave-roll");
        let saves = dir.join("saves");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&saves).unwrap();
        std::fs::write(saves.join("Toni.an3"), b"the manager's own").unwrap();
        let kept = |name: &str| std::fs::read(saves.join(name)).unwrap();
        for week in 1..=4 {
            roll_autosaves(&saves, format!("save {week}").as_bytes()).unwrap();
            assert_eq!(kept("Autos.an3"), format!("save {week}").as_bytes());
        }
        // The fourth roll dropped the first: three saves remain around,
        // the newest first, and the manager's own file untouched.
        assert_eq!(kept("Autos.an3"), b"save 4");
        assert_eq!(kept("Auto1.an3"), b"save 3");
        assert_eq!(kept("Auto2.an3"), b"save 2");
        assert_eq!(kept("Toni.an3"), b"the manager's own");
        assert_eq!(std::fs::read_dir(&saves).unwrap().count(), 4);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_autosave_holds_the_loaded_game() {
        let dir = std::env::temp_dir().join("a3-autosave-save");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let game = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let mut records =
            Records::from_save(Save::read(&game.join("saves/A.an3")).unwrap()).unwrap();
        records.globals.set_half_weeks(2);
        let options = records.globals.game_options();
        assert!(autosave_due(&options, usize::from(records.globals.half_weeks()) / 2));
        let bytes = records.to_save().to_bytes_stored().unwrap();
        roll_autosaves(&dir, &bytes).unwrap();
        let rolled = Save::parse(&std::fs::read(dir.join("Autos.an3")).unwrap()).unwrap();
        assert!(a3_files::an3::compare(&rolled, &records.to_save()).blocks.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
