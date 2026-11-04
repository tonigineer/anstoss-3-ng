//! The supercups of the countries played, set up as the new season's cups
//! are drawn (`FUN_006cc3ff`'s `FUN_006ccc80`, after each country's cup;
//! the object `0x1ba5148` + slot × 0x74, its data from +0x20).
//!
//! The object is cleared (`FUN_007cff4b`) and takes its country. Its
//! two are the closed season's champion and cup winner — the runner-up
//! when the champion won the cup too — (the European table's places by
//! country, `DAT_008e83be`). A country of the cup system 0 (Germany's,
//! `DAT_008a6110`) plays a small tournament of six (`FUN_006cdd42`): the
//! two, then the best of its first league's final places not among them
//! (the country block's +0x26, `FUN_006810c8`); the first two wait for
//! the later rounds, the third plays the fourth and the fifth the sixth.
//! Any other country's supercup is the one tie of the two, the sides
//! changed one time in two (`FUN_006cce25`).
//!
//! Left out: the first season's tournament (from the league's list,
//! `FUN_006cdcbe`), the ties' days played, their premiums and the
//! calendar's supercup days.

use crate::random::Random;
use crate::records::Records;

pub(crate) const OBJECT: u32 = 0x1ba5148;
/// The cup systems that play a tournament (`DAT_008a6110`).
const TOURNAMENT_SYSTEMS: [u8; 1] = [0];
/// The object's fields from its data: the country, the six of a
/// tournament, its ties of 0xe bytes, a single tie's sides.
const COUNTRY: usize = 0;
pub(crate) const SIX: usize = 2;
pub(crate) const TIES: usize = 0xe;
pub(crate) const TIE: usize = 0xe;
/// The tournament's final, or the one tie of any other country.
pub(crate) const FINAL_TIE: usize = 0x46;
const SINGLE_HOME: usize = FINAL_TIE;
const SINGLE_AWAY: usize = 0x4a;
/// The country block's final places of its first league, and the
/// European table's champion, cup winner and runner-up by country.
const FINAL_PLACES: usize = 0x26;
const PLACES_TAKEN: usize = 18;
const EUROPE: &str = "0x8e7c88";
const EUROPE_PLACES: usize = 0x716;
const EUROPE_ROW: usize = 12;
const CHAMPION: usize = 0;
const CUP_WINNER: usize = 8;
const CUP_RUNNER_UP: usize = 9;

/// Whether a country's supercup is a tournament (its cup system's mark
/// in `DAT_008a6110`).
pub(crate) fn is_tournament(records: &Records, country: u16) -> bool {
    TOURNAMENT_SYSTEMS.contains(&records.countries.cup_system(usize::from(country)))
}

fn put16(b: &mut [u8], at: usize, v: i16) {
    if let Some(w) = b.get_mut(at..at + 2) {
        w.copy_from_slice(&v.to_le_bytes());
    }
}

/// A country's supercup for the new season (`FUN_006ccc80`).
pub fn set_up(records: &mut Records, country: u16, random: &mut Random) {
    if records.globals.seasons() <= 1 {
        return;
    }
    let slot = usize::from(records.countries.slot(usize::from(country)));
    let system = records.countries.cup_system(usize::from(country));
    let name = format!("{OBJECT:#x}[{slot}]");
    let place = |k: usize| -> i16 {
        records
            .block(EUROPE)
            .and_then(|b| b.get(EUROPE_PLACES + usize::from(country) * EUROPE_ROW + k))
            .map_or(-1, |&v| i16::from(v as i8))
    };
    let (champion, winner, runner_up) = (place(CHAMPION), place(CUP_WINNER), place(CUP_RUNNER_UP));
    let other = if champion == winner { runner_up } else { winner };
    if TOURNAMENT_SYSTEMS.contains(&system) {
        let places: Vec<u8> = records
            .block(&format!("0x19f7fd0[{slot}]"))
            .map_or_else(Vec::new, |b| (0..PLACES_TAKEN).filter_map(|k| b.get(FINAL_PLACES + k).copied()).collect());
        let mut six: Vec<i16> = vec![champion, other];
        for &club in places.iter().take(6) {
            if six.len() < 6 && !six.contains(&i16::from(club)) {
                six.push(i16::from(club));
            }
        }
        let Some(b) = records.block_mut(&name) else { return };
        b.fill(0);
        put16(b, COUNTRY, country as i16);
        for (k, &club) in six.iter().enumerate() {
            put16(b, SIX + 2 * k, club);
        }
        let side = |k: usize| six.get(k).copied().unwrap_or(0);
        for (tie, (home, away)) in [(2, 3), (4, 5)].into_iter().enumerate() {
            let at = TIES + tie * TIE;
            b[at] = country as u8;
            put16(b, at + 2, side(home));
            b[at + 4] = country as u8;
            put16(b, at + 6, side(away));
        }
    } else {
        let turned = random.below(2) != 0;
        let Some(b) = records.block_mut(&name) else { return };
        b.fill(0);
        put16(b, COUNTRY, country as i16);
        let (home, away) = if turned { (other, champion) } else { (champion, other) };
        b[SINGLE_HOME] = country as u8;
        put16(b, SINGLE_HOME + 2, home);
        b[SINGLE_AWAY] = country as u8;
        put16(b, SINGLE_AWAY + 2, away);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The original's new season (NewSeason000) set up again from its own
    /// tables: Germany's six come out as the original's — its champion and
    /// cup winner, then the league's best four besides — and every other
    /// country's tie has the original's two sides.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_supercups_set_up_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut b, n) = (load("NewSeason000.an3"), load("NewSeason000.an3"));
        let mut r = Random::new(5);
        let mut tournaments = 0;
        for country in crate::opponent::played_nations(&b) {
            let slot = usize::from(b.countries.slot(usize::from(country)));
            let name = format!("{OBJECT:#x}[{slot}]");
            b.block_mut(&name).unwrap().fill(0xaa);
            set_up(&mut b, country, &mut r);
            let (x, y) = (b.block(&name).unwrap(), n.block(&name).unwrap());
            if TOURNAMENT_SYSTEMS.contains(&b.countries.cup_system(usize::from(country))) {
                tournaments += 1;
                assert_eq!(x, y, "{name}");
            } else {
                let sides = |b: &[u8]| {
                    let mut v = [&b[SINGLE_HOME..SINGLE_HOME + 4], &b[SINGLE_AWAY..SINGLE_AWAY + 4]];
                    v.sort();
                    v.concat()
                };
                assert_eq!(sides(x), sides(y), "{name}");
                assert_eq!(x[..SINGLE_HOME], y[..SINGLE_HOME], "{name}");
                assert_eq!(x[SINGLE_AWAY + 4..], y[SINGLE_AWAY + 4..], "{name}");
            }
        }
        assert_eq!(tournaments, 1);
    }
}
