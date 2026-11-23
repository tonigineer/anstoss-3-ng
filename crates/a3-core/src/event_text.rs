//! The day's event texts: what a happening at a human club tells its
//! manager in a window of its own (`FUN_005f1eaf` and its helpers fill the
//! event block at `0xa2f6d8`, `FUN_005ef5cb` posts it as event 0xfc and
//! `FUN_005f2234` opens the window the text fits). The block of a plain
//! text event holds the code (+0x4, `DAT_00a2f6dc`), the club (+0xc and
//! +0xe, its bytes +0x104 and +0x103), the club's human manager (+0x10,
//! its byte +0x8a, `FUN_005ef826`) and the text (`DAT_00a2f700`). The
//! window is "Ereignis" up to 200 characters (dialog 51, `CEreignistext`),
//! the larger one beyond (dialog 435, `CEreignisTextMittel` up to 400,
//! `CEreignisTextGross` after) — and it is the first part alone that
//! sizes it, `FUN_005f2234` reading `strlen` of `DAT_00a2f700`. The post
//! waits until the window is closed, so the events come up in the middle
//! of the day, as they happen.
//!
//! The two-part events (`FUN_005f2114`, player events 24, 28, 39 and 40)
//! stand the second text in a second buffer (`DAT_00a2f8f4`) and put 2000
//! into the pause word `DAT_00a2f6ea`, which `FUN_005ef45d` clears again
//! with every reset. The window's thread (`FUN_00498139`) waits out the
//! pause and, unless OK closed the window before it (`+0xd18`), puts
//! "%s\n%s" of both buffers into the text (`OnUserC000_004981e9`).
//!
//! Posted here: the training day's events ([`crate::training_day`]) whose
//! texts the original builds of its texts alone or with the names of the
//! men concerned, the two-part ones with their second part in tow
//! ([`Fill::Headbutt`], [`Fill::Bomb`], [`Fill::NamedThen`]). Not here:
//! the events with values beside the text (`FUN_005f1f42`, window
//! `CEreignis1Wert`: player events 10 to 14 and 16), the training's
//! ability and trait events (105 to 107) and the events of the day's news
//! and the rest of the game.

use a3_files::saq::EventTexts;

use crate::records::Records;

/// The event window up to 200 characters (`IDD_EREIGNIS_TEXT`).
pub const EVENT_TEXT: u16 = 51;
/// The larger one (`IDD_EREIGNIS_TEXT_GROSS`).
pub const EVENT_TEXT_LARGE: u16 = 435;
/// The longest text the small window takes (`strlen < 0xc9`).
const SMALL_MOST: usize = 0xc8;

/// The first season's first days without events while the option at
/// `DAT_00b5d19a` is set (`FUN_005ef56c`).
const QUIET_DAYS: u16 = 0x2a;
const QUIET_OPTION: usize = 0xb5d19a;

/// How a posted text takes the names of the men it is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fill {
    /// The text as it stands (`FUN_005f1e86`).
    Plain,
    /// Its `%s` a man's name (`sprintf` with `FUN_006feb4d`, as
    /// `FUN_005ee5c0` does).
    Named(u16),
    /// Its `%s` a man's name and its `%d` a number (`sprintf` with
    /// `FUN_006feb4d`, as `FUN_005f04e0` tells a youth's new strength).
    NamedNumber(u16, i32),
    /// `%1` and `%2` two men's names (`FUN_005ee619` through
    /// `FUN_005eee7f`).
    Numbered(u16, u16),
    /// `%1` a man's name and `%2` his partner's first name, from the
    /// first names of his nation (`FUN_00600f2f` with `FUN_0070173f`).
    Partnered(u16),
    /// `%1` a man's name and `%2` nothing (`FUN_00600f2f` handed an
    /// empty string, as the news of a man's service is).
    NumberedOne(u16),
    /// `%1` … `%3` three men's names and `%4` nothing (`FUN_00601166`
    /// with two `FUN_006feb4d`, as a man's autobiography tells of two
    /// team-mates).
    NumberedThree(u16, u16, u16),
    /// `%1` and `%2` two men's names and `%3` one of the game's texts by
    /// its id, 0 none (`FUN_00601166` with `FUN_006feb4d` and
    /// `FUN_007335e0`, as the hobby two team-mates share is), `%4`
    /// nothing.
    NamesAndGame(u16, u16, u16),
    /// `%1` and `%2` two men's names and `%3` `%4` nothing
    /// (`FUN_00601166` with one `FUN_006feb4d`, its second name buffer
    /// left unfilled as it is when the wives bring two men together).
    Names(u16, u16),
    /// `%1` a man's name, `%2` his partner's first name and `%3` his
    /// ex's — her name of his partner's row (`FUN_00601166` with
    /// `FUN_0070173f` and `FUN_0070185c`), `%4` nothing.
    PartnerAndEx(u16),
    /// `%1` a man's name, `%2` his affair's first name and `%3` his
    /// wife's (`FUN_00601166` with `FUN_007017db` and `FUN_0070173f`).
    AffairAndWife(u16),
    /// `%1` a man's name, `%2` his wife's first name and his last name
    /// (`FUN_0070177c`, "%s %s"), `%3` her first name and `%4` nothing
    /// (`FUN_00601166` with `FUN_0070177c` and `FUN_0070173f`, or
    /// `FUN_00600f2f` with the first alone).
    WifeByName(u16),
    /// `%1` a man's name and `%2` his affair's first name
    /// (`FUN_00600f2f` with `FUN_007017db`).
    Affair(u16),
    /// `%1` a man's name, `%2` his partner's first name, `%3` his
    /// affair's and `%4` nothing (`FUN_00601166` with `FUN_0070173f` and
    /// `FUN_007017db`).
    PartnerAndAffair(u16),
    /// `%1` a man's name and `%2` the first name of his child of that
    /// number (`FUN_00600f2f` with `FUN_00701818`, the names at +0x288).
    Child(u16, u8),
    /// `%1` a man's name and `%2` … the first names of his `n` newborns,
    /// the children of number `k` upwards (`FUN_006012df` with
    /// `FUN_00701818`, the names at +0x288): three for triplets, two for
    /// twins with the third buffer left empty.
    Newborns(u16, u8, u8),
    /// A lead and the text behind it (`"%s %s"`): a man's name, or
    /// another text when there is none.
    Lead(Lead),
    /// The text one of the game's own (by its id in `ListeDerTexte.rh`,
    /// `FUN_00730300`) rather than `ereignis.saq`'s, its `%s` a man's
    /// name (as `FUN_0074d2b5` builds a trial's end).
    GameNamed(u16),
    /// The text's `v1` … `v20` the club's phrases (`FUN_00733486`), put
    /// in where the window shows it, with the world's club names.
    Club,
    /// The first `%s` a human manager's name, by his number
    /// (`FUN_006a537d`), the second an amount of money (`FUN_004daf8f`),
    /// written where the window shows it.
    FalseReport(u16, i32),
    /// Its first `%s` a man's name, the second one of the game's texts by
    /// its id, 0 none (`sprintf` with `FUN_006feb4d` and `FUN_007335e0`,
    /// as `FUN_005fd4ae` tells a man's new hobby).
    NamedGame(u16, u16),
    /// The two-part head-duel (`FUN_005f2114`, player event 24): the
    /// text's `%s` `%s` two men's surnames (`FUN_006fea7c`), the second
    /// part `part`'s text (0x23 to 0x26), its `%s` one man's surname
    /// when `named` is not 0. The last roll names the second man while
    /// the first goes out, as the original does.
    Headbutt(u16, u16, u16, u16),
    /// The two-part bomb of the training (`FUN_005f2114`, player event
    /// 28): `%s` the shooter's full name (`FUN_006feb4d`) with `"\n"`
    /// (`DAT_008900d2`) and the text 0x2b behind it; the second part
    /// 0x2c's `%s` the keeper's full name with `"\n"` and the text
    /// `tail` (0x2d or 0x2e) behind.
    Bomb(u16, u16, u16),
    /// Two-part (`FUN_005f2114`, player events 39 and 40): `%s` a man's
    /// full name (`FUN_006feb4d`), the second part the text `second`
    /// (0x3b or 0x3d) as it stands.
    NamedThen(u16, u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lead {
    Player(u16),
    Text(u16),
}

/// An event as a rule posts it: the club, the event's code and the text's
/// id in `ereignis.saq` (the game's own for [`Fill::GameNamed`]) with what
/// goes into it. Its text is filled in
/// when it comes up ([`make`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Posted {
    pub club: (u16, u16),
    pub code: u16,
    pub text: u16,
    pub fill: Fill,
}

impl Posted {
    pub fn plain(club: (u16, u16), code: u16, text: u16) -> Self {
        Self { club, code, text, fill: Fill::Plain }
    }
}

/// An event ready for its window, its text filled in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub code: u16,
    /// The club, country and number.
    pub club: (u8, u16),
    /// The club's human manager, counted from 1; 0 for none.
    pub manager: u16,
    pub text: String,
    /// The second part of a two-part event (`DAT_00a2f8f4`), which the
    /// window joins behind the text after the pause; `None` for the
    /// one-part events.
    pub second: Option<String>,
}

impl Event {
    /// The window the text fits (`FUN_005f2234`); the first part alone
    /// sizes it, the second may run the window over.
    pub fn dialog(&self) -> u16 {
        if self.text.chars().count() <= SMALL_MOST {
            EVENT_TEXT
        } else {
            EVENT_TEXT_LARGE
        }
    }
}

/// Whether events stay away (`FUN_005ef56c`): the first season's first
/// 42 days with the option set.
pub fn quiet(records: &Records) -> bool {
    let g = &records.globals;
    g.day() < QUIET_DAYS && g.seasons() == 1 && g.get::<i16>(QUIET_OPTION) != 0
}

/// The event of a post, its text filled in; `game` gives the game's own
/// texts by id, `first_names` the nations' first names a hundred each
/// ([`crate::words::Words::first_names`]).
pub fn make(
    records: &Records,
    texts: &EventTexts,
    game: &dyn Fn(u16) -> String,
    first_names: &[String],
    posted: &Posted,
) -> Event {
    let saq = |id: u16| texts.get(usize::from(id)).unwrap_or_default();
    let name = |id: u16| records.known_as(id).unwrap_or_default();
    // His last name (`FUN_006fea7c`, the names at +0x1c0).
    let last = |id: u16| records.player(id).map(|p| records.name(p.last_name())).unwrap_or_default();
    // `FUN_0067503a(nation, number)` over the table `DAT_00b9f048`.
    let first_name = |id: u16, at: usize| {
        records
            .player(id)
            .and_then(|p| first_names.get(usize::from(p.names_nation()) * 100 + usize::from(p.get::<u8>(at))))
            .cloned()
            .unwrap_or_default()
    };
    // The partner's (+0x283), the affair's (+0x284), the ex's (+0x285).
    let partner = |id: u16| first_name(id, 0x283);
    let body = saq(posted.text);
    let text = match posted.fill {
        Fill::GameNamed(id) => game(posted.text).replacen("%s", &name(id), 1),
        Fill::Plain | Fill::Club => body.to_owned(),
        Fill::FalseReport(number, _) => {
            let who = usize::from(number)
                .checked_sub(1)
                .and_then(|m| records.managers.get(m))
                .map_or(String::new(), |m| format!("{} {}", m.first_name(), m.last_name()));
            body.replacen("%s", &who, 1)
        }
        Fill::Named(id) => body.replacen("%s", &name(id), 1),
        Fill::NamedGame(id, what) => {
            let what = if what == 0 { String::new() } else { game(what) };
            body.replacen("%s", &name(id), 1).replacen("%s", &what, 1)
        }
        Fill::NamedNumber(id, n) => body.replacen("%s", &name(id), 1).replacen("%d", &n.to_string(), 1),
        Fill::Numbered(one, other) => numbered(body, &[name(one), name(other)]),
        Fill::Partnered(id) => numbered(body, &[name(id), partner(id)]),
        Fill::NumberedOne(id) => numbered(body, &[name(id), String::new()]),
        Fill::NumberedThree(id, one, other) => numbered(body, &[name(id), name(one), name(other), String::new()]),
        Fill::NamesAndGame(one, other, what) => {
            let what = if what == 0 { String::new() } else { game(what) };
            numbered(body, &[name(one), name(other), what, String::new()])
        }
        Fill::Names(one, other) => numbered(body, &[name(one), name(other), String::new(), String::new()]),
        Fill::PartnerAndEx(id) => numbered(body, &[name(id), partner(id), first_name(id, 0x285), String::new()]),
        Fill::AffairAndWife(id) => numbered(body, &[name(id), first_name(id, 0x284), partner(id), String::new()]),
        Fill::WifeByName(id) => numbered(body, &[name(id), format!("{} {}", partner(id), last(id)), partner(id), String::new()]),
        Fill::Affair(id) => numbered(body, &[name(id), first_name(id, 0x284)]),
        Fill::PartnerAndAffair(id) => numbered(body, &[name(id), partner(id), first_name(id, 0x284), String::new()]),
        Fill::Child(id, k) => numbered(body, &[name(id), first_name(id, 0x288 + usize::from(k))]),
        Fill::Newborns(id, first, n) => {
            let mut args = vec![name(id)];
            args.extend((0..3).map(|k| {
                if k < usize::from(n) {
                    first_name(id, 0x288 + usize::from(first) + k)
                } else {
                    String::new()
                }
            }));
            numbered(body, &args)
        }
        Fill::Lead(lead) => {
            let lead = match lead {
                Lead::Player(id) => name(id),
                Lead::Text(id) => saq(id).to_owned(),
            };
            format!("{lead} {body}")
        }
        Fill::Headbutt(one, other, ..) => body.replacen("%s", &last(one), 1).replacen("%s", &last(other), 1),
        // The first half names the shooter, the keeper comes in the
        // second (`FUN_0060d1bf` case 0x18).
        Fill::Bomb(_, shooter, _) => format!("{}\n{}", body.replacen("%s", &name(shooter), 1), saq(0x2b)),
        Fill::NamedThen(id, _) => body.replacen("%s", &name(id), 1),
    };
    // The second part of the two-part events (`DAT_00a2f8f4`).
    let second = match posted.fill {
        Fill::Headbutt(_, _, part, named) => {
            let words = saq(part);
            Some(if named == 0 { words.to_owned() } else { words.replacen("%s", &last(named), 1) })
        }
        Fill::Bomb(keeper, _, tail) => {
            Some(format!("{}\n{}", saq(0x2c).replacen("%s", &name(keeper), 1), saq(tail)))
        }
        Fill::NamedThen(_, part) => Some(saq(part).to_owned()),
        _ => None,
    };
    let (country, number) = posted.club;
    let manager = records.team(country, number).map_or(0, |t| u16::from(t.human()));
    Event { code: posted.code, club: (country as u8, number), manager, text, second }
}

/// `%1` … `%99` replaced by `args` (`FUN_005eee7f`): one or two digits,
/// the first not 0; a number past the arguments leaves a '?', an empty
/// argument takes the blank behind it along.
pub fn numbered(text: &str, args: &[String]) -> String {
    let chars: Vec<char> = text.chars().collect();
    let digit = |c: Option<&char>| c.and_then(|c| c.to_digit(10));
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let first = digit(chars.get(i + 1)).filter(|&d| d > 0);
        let (Some(first), '%') = (first, chars[i]) else {
            out.push(chars[i]);
            i += 1;
            continue;
        };
        let mut k = first as usize - 1;
        i += 2;
        if let Some(second) = digit(chars.get(i)) {
            k = (k + 1) * 10 + second as usize - 1;
            i += 1;
        }
        match args.get(k) {
            Some(arg) => {
                if arg.is_empty() && chars.get(i) == Some(&' ') {
                    i += 1;
                }
                out += arg;
            }
            None => out.push('?'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbered_names_go_in_by_their_number() {
        let names = ["Moore".to_owned(), "Klaus".to_owned()];
        assert_eq!(numbered("%1 trifft %2, %1 lacht.", &names), "Moore trifft Klaus, Moore lacht.");
        assert_eq!(numbered("%3 und %12", &names), "? und ?");
        assert_eq!(numbered("100% %0", &names), "100% %0");
        assert_eq!(numbered("%1 kommt", &[String::new()]), "kommt");
    }

    #[test]
    fn a_text_past_two_hundred_takes_the_large_window() {
        let event =
            |n: usize| Event { code: 99, club: (1, 1), manager: 1, text: "x".repeat(n), second: None };
        assert_eq!(event(200).dialog(), EVENT_TEXT);
        assert_eq!(event(201).dialog(), EVENT_TEXT_LARGE);
    }
}
