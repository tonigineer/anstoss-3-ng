//! The game's data as every new game starts from it: the football world of
//! the data files, and the calendar and rules the executable keeps in
//! tables of its own.

use std::io;
use std::path::{Path, PathBuf};

use a3_files::exe::Exe;

use crate::calendar::Calendar;
use crate::presale::PresaleTables;
use crate::season::Rules;
use crate::world::World;

pub struct Data {
    /// The installation, with its saves in `saves/`.
    pub dir: PathBuf,
    /// Where the rolling autosaves go: the installation's `saves/`, or
    /// elsewhere for runs that must leave the saves there as they are —
    /// the script tests, many walking at once.
    pub saves: PathBuf,
    pub world: World,
    pub calendar: Calendar,
    /// The rules the game keeps in tables of its own.
    pub rules: Rules,
    /// The advance sales' tables.
    pub presale: PresaleTables,
    /// The big flags' names by land (`FUN_0069608b`'s table at
    /// `0x8a4890`, 117 of them): the info dialog on the national teams
    /// shows `gross/<name>.bmp` from the flags' archive; lands with no
    /// flag of their own carry the catch-all "Wel", land 0 and 76
    /// among them.
    pub nation_flags: Vec<String>,
}

impl Data {
    /// Reads the data of the installation in `dir`.
    pub fn load(dir: &Path) -> io::Result<Self> {
        let world = World::load(&dir.join("Data.a3"))?;
        let exe = Exe::open(&dir.join("anstoss3.exe"))?;
        let mut rules = Rules::load(&exe)?;
        if let Ok(texts) = a3_files::rch::load_texts(&dir.join("deb/ListeDerTexte.rh")) {
            rules.training.name_originals(&texts);
        }
        Ok(Self {
            dir: dir.to_owned(),
            saves: dir.join("saves"),
            calendar: Calendar::load(&exe, &world)?,
            rules,
            presale: PresaleTables::load(&exe)?,
            nation_flags: crate::national::flag_names(&exe)?,
            world,
        })
    }
}
