//! The question over a fine (`FUN_005f29bc`, the window of
//! `CEreignisFrage`, dialog 50): a man has been seen at what the club
//! takes amiss, and the human manager of his club chooses among three
//! rows — one game barred inside the club, a fine, or letting it pass.
//! The first two sour him with his trainer and book another punishment
//! among his honours; the third grows the club's fine book by the fine
//! (`DAT_00a321fc`, 5000 for this kind). Kind 1 (`param_5`, the question
//! table's entry `1 + 0x21` at `0x88f090`) is the gluttony of the food
//! hobby ([`crate::news`]'s hobby day, food's case 4); kinds 2 and 3
//! (`FUN_0070302d`, `FUN_006fe907`) stand behind happenings the replica
//! does not draw yet.
//!
//! Not the network (`DAT_01cb8be3`): the text of event 0x4b8 is built
//! with the man's name and `%2` empty (`FUN_005eee7f`) into
//! `DAT_00a32210`, which the window shows in place of a question of its
//! own (its table entry names one with −1), and event 0xfd is posted and
//! waited for (`FUN_00650669`). On a quiet day (`FUN_005ef56c`) the
//! original answers nought without the window; the replica never gets
//! there, because its day's news — the overweight marking, the offence
//! note and this question — is not drawn on those days at all.
//!
//! The answer's rows and the fine are the window's own words; the fine
//! stands in the middle row's `%4` (`FUN_005eea5d`).

use crate::records::Records;

/// What the window waits for: the club whose manager is asked (country
/// slot and number, `FUN_005f29bc`'s club at `param_2`, its id kept at
/// `DAT_00a321e8` from the manager) and the man it is about (`param_4`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Question {
    pub club: (u16, u16),
    pub player: u16,
}

/// The question shown, built with the man's name (`FUN_005f29bc`'s
/// `FUN_005ef430`; `CEreignisFrage` puts it into its text control 1445).
pub const QUESTION_TEXT: u16 = 0x4b8;

/// The three rows' words (`CEreignisFrage`'s label table at +8, +0xc and
/// +0x10; a zero word hides its row — kind 1 has three). The middle one
/// writes the fine at its `%4` (`FUN_005eea5d`), the money writing of
/// `DAT_00a321fc`.
pub const ANSWERS: [u16; 3] = [0x61e, 0x61f, 0x620];

/// The fine of the middle row, kind 1 (`DAT_00a3b3cc` into
/// `DAT_00a321fc`).
pub const FINE: i32 = 5000;

/// What an answer does (`FUN_005f29bc` behind the window; `answer` is
/// the row pointed at, 0 to 2 — the window's OK closes with the answer
/// still at nought, so the first row is what an unanswered question
/// means).
pub fn answer(records: &mut Records, q: &Question, answer: usize) {
    match answer {
        // Barred for one game inside the club, or the second row: ten or
        // two less content with his trainer (`FUN_007007a1`), and one
        // more punishment booked among his honours (`FUN_007049cc`, the
        // byte at +0x2a6).
        0 | 1 => {
            if let Some(p) = records.player_mut(q.player) {
                crate::mood::change_trainer(p, if answer == 0 { -10 } else { -2 });
                let mut honours = p.honours();
                honours[13] = honours[13].wrapping_add(1);
                p.set_honours(honours);
            }
        }
        // Letting it pass takes the money instead (the club's +0x28c,
        // the team record's fines).
        _ => {
            if let Some(team) = records.team_mut(q.club.0, q.club.1) {
                team.set_fines(team.fines() + FINE);
            }
        }
    }
    // Then the count of his offences at this kind (`FUN_00702fec`, the
    // 2-bit counter at +0x298, noted before the asking) plus the answer
    // less one weighs the club's discipline word: kind 1 steps it one
    // down at two and two down at more (`n == 2` and `n > 2`, clamped at
    // −100; kinds 2 and 3 can also lift it, clamped at 100). That word
    // is a run-time byte of the club object at +0xa7, filled at nought
    // and handed to no save; its readers read it past five ([`crate::news`],
    // codes 21, 47 and 48) and kind 1 only ever moves it down from that
    // nought, so the step stays a seam here, as the hundred-roll on it
    // in [`crate::world_day`] does.
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Save A's fourth man asked: the first row makes him ten less
    /// content with his trainer and books one more punishment among his
    /// honours, the second two less and one more too; the third leaves
    /// him as he is and puts the 5000 onto his club's fine book.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_three_rows_punish_the_man_or_bill_the_club() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = || Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let (hc, hn) = crate::statistics::human_clubs(&load())[0];
        let club = (u16::from(hc), hn as u16);
        let man = load().team(club.0, club.1).unwrap().players()[3];
        let q = Question { club, player: man };
        for (row, by) in [(0, -10), (1, -2)] {
            let mut r = load();
            let (trainer, punished) = {
                let p = r.player(man).unwrap();
                (i32::from(p.contentment()[0]), p.honours()[13])
            };
            answer(&mut r, &q, row);
            let p = r.player(man).unwrap();
            assert_eq!(i32::from(p.contentment()[0]), (trainer + by).clamp(0, 120));
            assert_eq!(p.honours()[13], punished.wrapping_add(1));
        }
        let mut r = load();
        let fines = r.team(club.0, club.1).unwrap().fines();
        answer(&mut r, &q, 2);
        assert_eq!(r.team(club.0, club.1).unwrap().fines(), fines + FINE);
        let p = r.player(man).unwrap();
        assert_eq!(i32::from(p.contentment()[0]), i32::from(load().player(man).unwrap().contentment()[0]));
    }
}
