//! The event windows over a 3D picture (dialog 477, `IDD_RENDER_NEU`,
//! `CRender` on `CRenderNeu`), which the hub opens at the start of a club
//! manager's turn (`CMyStartDialog::FUN_00650b84`, after the match
//! announcement): `FUN_00534f88` sets the event's kind (`DAT_00a321f4`)
//! and its data (`DAT_00a321fc`) and runs the window, only while the
//! option at `DAT_00b5d149` is on. Its OnInitDialog (`FUN_005350c6`) fills
//! up to six lines by the kind and plays the kind's video in the picture
//! on the left, which the replica leaves out with the rest of the 3D.
//!
//! Here kind 5, the records he broke: for every bit of 0 to 20 set in the
//! manager object's +0xe4 (the record's +0x218) a window while the option
//! at `DAT_00b5d14a` is on too ("Sie haben damit den Rekord für den
//! teuersten Verkauf …"); then the whole word is cleared, whatever was
//! shown (asm 0x65116f to 0x651227). Not here: the other kinds the hub
//! opens (0x12 the first day, 0x14, 0x16, 0x17, 0x18, 0x1a and 0x1b), the
//! season's titles and honours (kinds 1 to 4, 7 to 10, 0xf to 0x11, 0x13,
//! 0x15, 0x19, 0x1c from elsewhere) and the rule changes
//! (`FUN_00536943`, `CRenderNeu` alone); nor what sets the record bits.

use crate::records::Records;

/// The window (`IDD_RENDER_NEU`).
pub const RENDER: u16 = 477;

/// The options among the switches at `0xb5d13c`: the 3D graphics, without
/// which no window opens (`DAT_00b5d149`), and the records
/// (`DAT_00b5d14a`).
const GRAPHICS_OPTION: usize = 0x0d;
const RECORDS_OPTION: usize = 0x0e;
const ON: u8 = 1;

/// The records, by their bits in the word (the hub's loop up to 0x15).
pub const RECORDS: u16 = 21;
/// Their texts, by bit (the table at `0x886750`): the lines below the
/// congratulation, separated by `|`.
pub const RECORD_TEXTS: [u16; RECORDS as usize] = [
    3892, 3893, 3894, 3895, 3896, 3897, 3898, 3899, 3900, 3901, 3912, 3911, 3910, 3909, 3908, 3904, 3903,
    3902, 3907, 3905, 3906,
];
/// "Herzlichen Glückwunsch", the first line of every record (text 0xc4c).
pub const CONGRATULATIONS: u16 = 3148;
/// The caption before the manager's first and last name ("%s %s %s",
/// text 0x139a, `FUN_00536655`).
pub const CAPTION: u16 = 5018;

/// A record broken, for its window: the human's slot and the record's
/// bit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Broken {
    pub slot: u8,
    pub record: u8,
}

/// The records the manager in `slot` broke that open a window, in the
/// order of their bits; his word cleared as the hub clears it.
pub fn take_broken(records: &mut Records, slot: usize) -> Vec<Broken> {
    let options = records.globals.game_options();
    let shown = options[RECORDS_OPTION] == ON && options[GRAPHICS_OPTION] == ON;
    let Some(manager) = records.managers.get_mut(slot) else { return Vec::new() };
    let word = manager.render_events();
    manager.set_render_events(0);
    if !shown {
        return Vec::new();
    }
    (0..RECORDS)
        .filter(|&bit| word & 1 << bit != 0)
        .map(|bit| Broken { slot: slot as u8, record: bit as u8 })
        .collect()
}

/// The window's lines: the congratulation, then the record's text cut at
/// its `|`, up to four lines (`FUN_005350c6`, case 5).
pub fn lines(congratulations: &str, text: &str) -> Vec<String> {
    let mut lines = vec![congratulations.to_owned()];
    lines.extend(text.splitn(4, '|').map(str::to_owned));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_text_is_cut_into_lines_below_the_congratulation() {
        let lines = lines("Herzlichen Glückwunsch", "Sie haben damit den Rekord|für den teuersten Verkauf|eines Spielers verbessert!");
        assert_eq!(lines, ["Herzlichen Glückwunsch", "Sie haben damit den Rekord", "für den teuersten Verkauf", "eines Spielers verbessert!"]);
    }

    /// A's human broke the record for the dearest sale (bit 14, text
    /// 3908): one window, his word cleared after it, and nothing at a
    /// second look; with the records option off his word is cleared all
    /// the same.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_kept_save_shows_the_dearest_sale_once() {
        use a3_files::an3::Save;
        let keeps = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"))
            .parent()
            .unwrap()
            .join("saves-kept");
        let read = || Records::from_save(Save::read(&keeps.join("A.an3")).unwrap()).unwrap();
        let mut records = read();
        let slot = records.human_slot(0).unwrap();
        assert_eq!(take_broken(&mut records, slot), [Broken { slot: slot as u8, record: 14 }]);
        assert_eq!(RECORD_TEXTS[14], 3908);
        assert_eq!(records.managers[slot].render_events(), 0);
        assert!(take_broken(&mut records, slot).is_empty());
        let mut records = read();
        let mut options = records.globals.game_options();
        options[RECORDS_OPTION] = 0;
        records.globals.set_game_options(options);
        assert!(take_broken(&mut records, slot).is_empty());
        assert_eq!(records.managers[slot].render_events(), 0);
    }
}
