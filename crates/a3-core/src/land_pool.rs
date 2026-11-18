//! A new game's referees and free players (`FUN_0068716b`, from
//! `FUN_00686684`).
//!
//! `ISchiris.sav`'s `ISCHIRI` holds forty referees, six lines each
//! (`FUN_0068be0f`): last and first name, competence, hardness, the byte at
//! +0x27 and the number at +0x30. They become referees 201 to 240.
//!
//! Each `Jugend<n>.sav` (n 1 to 17) holds in its `ASPIELER` the names of
//! free players of the country class n + 10, a hundred of the first three,
//! sixty of the others (`FUN_0068b266`): each takes the next player id, a
//! main position drawn from 1 to 10 and a face of his class
//! (`FUN_0066e5fc`), and an entry of the free players' table
//! (`DAT_00b68ea8`, four bytes: the id, then the position at +3), whose
//! first and end index by class stand at `DAT_00b5cc08` / `DAT_00b5cc44`.

use std::io;
use std::path::Path;

use a3_files::sav::Section;

use crate::land_players::Pool;
use crate::random::Random;
use crate::records::Records;
use crate::world::find;

/// The referees' records (0x34 bytes, the save's blocks from +0x20).
const REFEREE_AT: u32 = 0x19f4ed8;
/// The first of the forty referees read (`FUN_0068716b`'s 0xc9).
const FIRST_REFEREE: usize = 0xc9;
const REFEREES: usize = 0x28;
/// The lines of a referee.
const REFEREE_LINES: usize = 6;

/// The free players' table: an entry of four bytes each.
const FREE_AT: &str = "0xb68ea8";
/// The first and end entries of each country class (`DAT_00b5cc08`,
/// `DAT_00b5cc44`, i16 by class).
const CLASS_FIRST: usize = 0xb5cc08;
const CLASS_END: usize = 0xb5cc44;
/// Where the free players' entries begin and fifty after
/// (`DAT_00b5cc42`, `DAT_00b5cc7e`).
const FREE_FIRST: usize = 0xb5cc42;
const FREE_LATER: usize = 0xb5cc7e;
/// The `Jugend` files and how many each holds.
const FILES: u16 = 17;
const MANY: usize = 100;
const FEW: usize = 60;

/// `FUN_0068be0f` for the forty referees of `ISCHIRI`.
pub fn read_referees(records: &mut Records, ischiri: &Section) {
    let mut names = std::mem::take(&mut records.names);
    let mut pool = Pool::new(&mut names);
    for (i, lines) in ischiri.values.chunks(REFEREE_LINES).take(REFEREES).enumerate() {
        let line = |n: usize| lines.get(n).map_or("", String::as_str);
        let number = |n: usize| line(n).trim().parse::<i64>().unwrap_or(0);
        let first = pool.index(line(1));
        let last = pool.index(line(0));
        let Some(bytes) = records.block_mut(&format!("{REFEREE_AT:#x}[{}]", FIRST_REFEREE + i)) else {
            continue;
        };
        bytes[0..2].copy_from_slice(&first.to_le_bytes());
        bytes[2..4].copy_from_slice(&last.to_le_bytes());
        bytes[5] = number(2) as u8;
        bytes[6] = number(3) as u8;
        bytes[7] = number(4) as u8;
        bytes[0x10..0x14].copy_from_slice(&(number(5) as i32).to_le_bytes());
    }
    drop(pool);
    records.names = names;
}

/// `FUN_0066e5fc`: a face of a country class, the byte at +0x21a and the
/// two halves of +0x21b.
pub fn face(class: u16, random: &mut Random) -> (u8, u8, u8) {
    let mut drawn = |kind: u8, colours: u32, rare: u32| {
        let mut colour = random.below(colours) as u8;
        if rare != 0 && colour == 3 && random.below(rare) != 0 {
            colour = 0;
        }
        let hair = if random.below(100) < 99 { random.below(4) as u8 + 1 } else { 0 };
        (kind, colour, hair)
    };
    match class {
        0 | 1 | 2 | 7 | 8 | 9 => drawn(0, 5, 0x19),
        3 | 4 | 5 | 10 | 0x12 | 0x13 | 0x16 => (1, 4, 2),
        6 | 0xe | 0xf | 0x15 | 0x17 | 0x18 | 0x1a | 0x1b => drawn(0, 5, 0x32),
        0xb => (2, 4, 1),
        0xc => (1, 4, 1),
        0xd => drawn(3, 5, 0x32),
        0x10 | 0x11 | 0x14 | 0x19 => drawn(0, 2, 0),
        _ => (0, 4, 2),
    }
}

/// `FUN_0068b266` with no contract (its last two arguments 0): `count`
/// free players of a class from the names of `aspieler`, from player
/// `next` and entry `entry` on.
pub fn read_free(records: &mut Records, aspieler: &Section, class: u16, count: usize, next: &mut usize, entry: &mut u16, random: &mut Random) {
    records.globals.put::<u16>(CLASS_FIRST + usize::from(class) * 2, *entry);
    let mut names = std::mem::take(&mut records.names);
    let mut pool = Pool::new(&mut names);
    let mut lines = aspieler.values.iter().map(String::as_str);
    for _ in 0..count {
        let first = pool.index(lines.next().unwrap_or(""));
        let last = pool.index(lines.next().unwrap_or(""));
        let id = *next;
        // A class above 10 draws its position.
        let position = if class < 0xb { 1 } else { random.below(10) as u8 + 1 };
        if let Some(table) = records.block_mut(FREE_AT) {
            let at = usize::from(*entry) * 4;
            if at + 4 <= table.len() {
                table[at..at + 2].copy_from_slice(&(id as u16).to_le_bytes());
                table[at + 3] = position;
            }
        }
        let player = &mut records.players[id];
        player.set_first_name(first);
        player.set_last_name(last);
        player.put::<u8>(0x164, 0);
        player.put::<u16>(0x166, 0xffff);
        player.set_id(id as u16);
        player.put::<u8>(0x227, class as u8);
        let mut positions = player.positions();
        positions[0] = position;
        player.set_positions(positions);
        let (kind, colour, hair) = face(class, random);
        player.set_face([kind, colour.wrapping_add(hair << 4)]);
        *next += 1;
        *entry += 1;
    }
    drop(pool);
    records.names = names;
    records.globals.put::<u16>(CLASS_END + usize::from(class) * 2, *entry);
}

/// `FUN_0068716b`: the referees and the free players of `Data.a3` into the
/// records, the players from `next` on, the table's entries from `entry`
/// (`DAT_00ba0b20`).
pub fn read_pool(records: &mut Records, dir: &Path, next: &mut usize, entry: &mut u16, random: &mut Random) -> io::Result<()> {
    let referees = Section::read(&find(dir, "ISchiris.sav")?)?;
    if let Some(ischiri) = referees.child("ISCHIRI") {
        read_referees(records, ischiri);
    }
    for n in 1..=FILES {
        let file = Section::read(&find(dir, &format!("Jugend{n}.sav"))?)?;
        let Some(aspieler) = file.child("ASPIELER") else {
            continue;
        };
        let count = if n < 4 { MANY } else { FEW };
        read_free(records, aspieler, n + 10, count, next, entry, random);
    }
    records.globals.put::<u16>(FREE_FIRST, *entry);
    records.globals.put::<u16>(FREE_LATER, *entry + 0x32);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The oracle's new game: its first turn holds the forty referees as
    /// the file gives them and every free player under his names, class
    /// and in the table.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR and the oracle's dumps from $ANSTOSS3_ORACLE"]
    fn a_new_games_referees_and_free_players_read_as_the_originals() {
        let (Some(game), Some(oracle)) = (std::env::var_os("ANSTOSS3_DIR"), std::env::var_os("ANSTOSS3_ORACLE")) else {
            eprintln!("no oracle dump of a new game's first turn among the saves: ANSTOSS3_DIR or ANSTOSS3_ORACLE is not set");
            return;
        };
        let path = std::path::PathBuf::from(oracle).join("runs/new-game/first-turn.an3");
        if !path.exists() {
            eprintln!("no oracle dump of a new game's first turn among the saves");
            return;
        }
        let dump = Records::from_save(a3_files::an3::Save::read(&path).unwrap()).unwrap();
        let first = |c: u16| dump.globals.get::<i16>(CLASS_FIRST + usize::from(c) * 2);
        let end = |c: u16| dump.globals.get::<i16>(CLASS_END + usize::from(c) * 2);
        eprintln!(
            "classes {:?}, free {} {}",
            (11..28).map(|c| (first(c), end(c))).collect::<Vec<_>>(),
            dump.globals.get::<i16>(FREE_FIRST),
            dump.globals.get::<i16>(FREE_LATER)
        );
        let mut records = Records::blank();
        // The head of FUN_00686684 has put its two names first
        // (`DAT_00b5dfca`, `DAT_00b5dfc8`, "Der" "Teamchef"), not read yet.
        records.names = dump.names[..2].to_vec();
        let dir = std::path::PathBuf::from(game).join("Data.a3");
        let (mut next, mut entry) = (1, first(11) as u16);
        read_pool(&mut records, &dir, &mut next, &mut entry, &mut Random::new(1)).unwrap();
        for id in FIRST_REFEREE..FIRST_REFEREE + REFEREES {
            let id = id as i16;
            assert_eq!(records.referee_name(id), dump.referee_name(id), "referee {id}");
            let mine = records.block(&format!("{REFEREE_AT:#x}[{id}]")).unwrap();
            let theirs = dump.block(&format!("{REFEREE_AT:#x}[{id}]")).unwrap();
            assert_eq!(mine[5..8], theirs[5..8], "referee {id}");
            assert_eq!(mine[0x10..0x14], theirs[0x10..0x14], "referee {id}");
        }
        for c in 11..28 {
            assert_eq!((records.globals.get::<i16>(CLASS_FIRST + c * 2), records.globals.get::<i16>(CLASS_END + c * 2)), (first(c as u16), end(c as u16)), "class {c}");
        }
        assert_eq!(records.globals.get::<i16>(FREE_FIRST), dump.globals.get::<i16>(FREE_FIRST));
        assert_eq!(records.globals.get::<i16>(FREE_LATER), dump.globals.get::<i16>(FREE_LATER));
        // Each entry's player in the dump: the same names, class and
        // position as the file and the draw give, the ids from 13934 on
        // (the country files' players stand before them).
        let (mine, theirs) = (records.block(FREE_AT).unwrap(), dump.block(FREE_AT).unwrap());
        let name = |r: &Records, id: usize| {
            let p = &r.players[id];
            (r.name(p.first_name()).to_owned(), r.name(p.last_name()).to_owned())
        };
        let (mut apart, mut taken) = (vec![], 0);
        for e in usize::from(first(11) as u16)..usize::from(entry) {
            let id = |t: &[u8]| usize::from(u16::from_le_bytes([t[e * 4], t[e * 4 + 1]]));
            let (a, b) = (id(mine), id(theirs));
            if b == 0 {
                taken += 1;
                continue;
            }
            let (p, o) = (&records.players[a], &dump.players[b]);
            if b != a + 13933 || name(&records, a) != name(&dump, b) || p.get::<u8>(0x227) != o.get::<u8>(0x227) || p.get::<u16>(0x166) != o.get::<u16>(0x166) || o.positions()[0] != theirs[e * 4 + 3] || !(1..=10).contains(&o.positions()[0])
                || (matches!(p.get::<u8>(0x227), 0xb | 0xc | 0x12 | 0x13 | 0x16) && p.face() != o.face())
            {
                apart.push(format!("{e}: {a} {:?} {}, not {b} {:?} {} {:?}", name(&records, a), p.get::<u8>(0x227), name(&dump, b), o.get::<u8>(0x227), o.positions()));
            }
        }
        // The first turn's youths have taken some out of the table
        // (new-game-exact-squads).
        eprintln!("{taken} taken");
        assert!(taken < 20);
        assert!(apart.is_empty(), "{} apart: {:?}", apart.len(), &apart[..apart.len().min(6)]);
    }
}
