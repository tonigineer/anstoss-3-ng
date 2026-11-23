//! `texte/ereignis.saq`: the news and event texts (`FUN_005ef298`, loaded
//! once into the table at `0xa2fae8`). An 8-byte header — the data size
//! (0x42d1c) and the text count (0x9b3) — then the data, each byte XOR the
//! previous cipher byte with the low byte of the size as seed, holding the
//! texts as zero-terminated cp1252 strings, ids counted from 1.

use std::path::Path;

/// The file inside the game directory.
pub const EVENT_TEXTS: &str = "texte/ereignis.saq";

/// The size and count the game insists on; anything else is refused
/// ("Falsche Anzahl IDs").
const SIZE: u32 = 0x42d1c;
const COUNT: u32 = 0x9b3;

pub struct EventTexts {
    texts: Vec<String>,
}

/// The chained XOR: each byte with the cipher byte before it.
pub fn decipher(data: &mut [u8], seed: u8) {
    let mut key = seed;
    for byte in data {
        let cipher = *byte;
        *byte ^= key;
        key = cipher;
    }
}

impl EventTexts {
    pub fn parse(file: &[u8]) -> Option<Self> {
        let word = |i: usize| file.get(i..i + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        let (size, count) = (word(0)?, word(4)?);
        if size != SIZE || count != COUNT {
            return None;
        }
        let mut data = file.get(8..8 + size as usize)?.to_vec();
        decipher(&mut data, size as u8);
        let texts: Vec<String> =
            data.split(|&b| b == 0).take(count as usize).map(crate::sav::cp1252).collect();
        (texts.len() == count as usize).then_some(Self { texts })
    }

    pub fn load(game_dir: &Path) -> Option<Self> {
        Self::parse(&std::fs::read(game_dir.join(EVENT_TEXTS)).ok()?)
    }

    /// Text `id` (1 ..= 0x9b3).
    pub fn get(&self, id: usize) -> Option<&str> {
        self.texts.get(id.checked_sub(1)?).map(String::as_str)
    }

    /// A headline text split at its first '-' into headline and body
    /// (`FUN_00789c32`); without one, all of it is the headline.
    pub fn headline(&self, id: usize) -> Option<(&str, &str)> {
        let text = self.get(id)?;
        Some(text.split_once('-').unwrap_or((text, "")))
    }

    pub fn len(&self) -> usize {
        self.texts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.texts.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chained_xor_round_trips() {
        let plain = b"Haus\0Tor\0".to_vec();
        let mut cipher = plain.clone();
        let mut key = 0x1c;
        for byte in &mut cipher {
            *byte ^= key;
            key = *byte;
        }
        decipher(&mut cipher, 0x1c);
        assert_eq!(cipher, plain);
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_event_texts_decode() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let texts = EventTexts::load(&dir).expect("ereignis.saq");
        assert_eq!(texts.len(), 0x9b3);
        assert_eq!(texts.get(2), Some("Alles klar."));
        assert!(texts.get(308).unwrap().contains("baut gerade ein Haus"));
        assert!(texts.get(680).unwrap().contains("Autogrammstunden"));
        assert_eq!(texts.get(0x9b3), Some("\"Sieg oder Spielabbruch!\""));
        assert_eq!(texts.get(0), None);
    }
}
