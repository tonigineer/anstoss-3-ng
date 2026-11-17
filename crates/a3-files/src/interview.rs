//! `texte/Interv.saq`: the questions of the interview after a match
//! (`FUN_0065f0b8` reads it whole at every interview, `FUN_0065f134` sets
//! each record's text pointers). A count, then the records one after the
//! other, each led by its own size: the question's id, the ten facts it
//! needs, its topic, the reporter's manner it fits, the mood it is not
//! asked in, its weight, the two effects of each of its four answers, and
//! six zero-terminated cp1252 texts from +0x5c — the question twice (the
//! second may be empty) and the four answers. Plain, not enciphered.

use std::path::Path;

/// The file inside the game directory; the game asks for `interv.saq`,
/// the installation spells it `Interv.saq`.
pub const DIR: &str = "texte";
pub const FILE: &str = "interv.saq";

/// The texts start this far into a record (`FUN_0065f134`: six pointers
/// at +0x44, the first text at +0x5c).
const TEXTS: usize = 0x5c;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Question {
    /// +0x04.
    pub id: i32,
    /// +0x08: ten facts that must hold: a positive one a fact of the match
    /// (`FUN_0066177d`), a negative one a question not asked before in this
    /// interview, nought always.
    pub needs: [i16; 10],
    /// +0x1c: its topic, 0 to 49.
    pub topic: i8,
    /// +0x1d: the reporter's manner it is for, 0 any.
    pub manner: i8,
    /// +0x1e, +0x1f: the reporter's moods it is not asked in.
    pub moods: [i8; 2],
    /// +0x20: the heaviest weight among the questions that may come wins.
    pub weight: i32,
    /// +0x24: for each answer two effects, a kind and a value.
    pub effects: [[(i16, i16); 2]; 4],
    /// The question, and a second way to put it (empty if none).
    pub texts: [String; 2],
    pub answers: [String; 4],
}

fn i16_at(b: &[u8], at: usize) -> Option<i16> {
    Some(i16::from_le_bytes([*b.get(at)?, *b.get(at + 1)?]))
}

fn i32_at(b: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// The records in their order; none if the file is cut short.
pub fn parse(file: &[u8]) -> Option<Vec<Question>> {
    let count = usize::try_from(i32_at(file, 0)?).ok()?;
    let mut at = 4;
    let mut questions = Vec::with_capacity(count);
    for _ in 0..count {
        let size = usize::try_from(i32_at(file, at)?).ok()?;
        let r = file.get(at..at + size)?;
        let mut texts = r.get(TEXTS..)?.split(|&b| b == 0).map(crate::sav::cp1252);
        let mut text = || texts.next().unwrap_or_default();
        let effect = |k: usize| -> Option<(i16, i16)> { Some((i16_at(r, 0x24 + 4 * k)?, i16_at(r, 0x26 + 4 * k)?)) };
        questions.push(Question {
            id: i32_at(r, 4)?,
            needs: std::array::from_fn(|k| i16_at(r, 8 + 2 * k).unwrap_or(0)),
            topic: *r.get(0x1c)? as i8,
            manner: *r.get(0x1d)? as i8,
            moods: [*r.get(0x1e)? as i8, *r.get(0x1f)? as i8],
            weight: i32_at(r, 0x20)?,
            effects: [
                [effect(0)?, effect(1)?],
                [effect(2)?, effect(3)?],
                [effect(4)?, effect(5)?],
                [effect(6)?, effect(7)?],
            ],
            texts: [text(), text()],
            answers: [text(), text(), text(), text()],
        });
        at += size;
    }
    Some(questions)
}

/// Reads the file of an installation; none where it is missing, and the
/// interview does not take place (`FUN_0065f390`).
pub fn load(game_dir: &Path) -> Option<Vec<Question>> {
    let dir = game_dir.join(DIR);
    let path = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.file_name().is_some_and(|f| f.eq_ignore_ascii_case(FILE)))?;
    parse(&std::fs::read(path).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The installation's file: 568 questions, the first on a team that
    /// has played terribly, its first answer taking five off the team's
    /// mood and turning the talk to topic fifteen.
    #[test]
    #[ignore]
    fn the_installations_questions_parse() {
        let Some(dir) = std::env::var_os("ANSTOSS3_DIR") else { return };
        let questions = load(Path::new(&dir)).expect("Interv.saq");
        assert_eq!(questions.len(), 568);
        let first = &questions[0];
        assert_eq!((first.id, first.needs[..3].to_vec(), first.topic, first.moods), (1, vec![2, 3, 0], 7, [3, 5]));
        assert_eq!(first.effects[0], [(13, -5), (10, 15)]);
        assert!(first.texts[0].starts_with("Die Mannschaft hat in den letzten Wochen"));
        assert_eq!(first.answers[3], "Was versteht einer wie Sie schon vom Fußball?");
        assert_eq!(questions[1].texts[1], "");
    }
}
