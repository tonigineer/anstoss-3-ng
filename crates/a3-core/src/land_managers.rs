//! A new game's human managers as records. The new game (`FUN_007875b1`)
//! gives each name the setup took a slot and a fresh record
//! (`FUN_006a3f6a`, `FUN_006a3b61`) before the country files are read, the
//! abilities dialog (374) writes what each chose, and the start settings
//! draw the order the managers take their turns in and move the records
//! into it (`FUN_006a4fc5`).

use crate::joining::free_slot;
use crate::newgame::Trainer;
use crate::random::Random;
use crate::records::{MANAGERS, Manager, Records};
use crate::season::Rules;

/// The level a manager starts at before the clubs dialog sets it by the
/// league (`FUN_006a3b61`).
const START_LEVEL: u8 = 2;
/// His money: the game's unit of ten thousand (`DAT_00a3b3bc`).
const START_CASH: i32 = 10_000;
/// His nerves at their best, and the word beside them (+0x11c, meaning
/// not traced).
const NERVES: i16 = 100;
const BESIDE_NERVES: i16 = 0x50;
/// The public, the fans and the media like him 40 to 60 each.
const POPULARITY_LEAST: u8 = 40;
const POPULARITY_SPREAD: u32 = 21;
/// The position a setup record without one plays, and the last there is.
const NO_POSITION: u8 = 1;
const LAST_POSITION: u8 = 10;
/// The player standing for a manager: id 17000 + slot.
const MANAGER_PLAYER: usize = 17000;
/// His strong foot, by a draw below 100: under 15 left, under 85 right,
/// else both (`FUN_0070632a` into bits 2 and 3 of the player's +0x282).
const FOOT_DRAW: u32 = 100;
const FOOT_LEFT_BELOW: u32 = 15;
const FOOT_RIGHT_BELOW: u32 = 85;
const FOOT: [u8; 3] = [2, 1, 3];
const FOOT_BITS: u8 = 0xc;
/// The rows of a manager's abilities, one a level, seven abilities each.
const ABILITY_ROWS: usize = 16;
const ABILITIES: usize = 7;

/// `FUN_006a3f6a` and `FUN_006a3b61`: the setup's record of a manager
/// takes the first slot not standing in its own place, and the slot a
/// fresh record; his slot, 1 to 4, or `None` with all four taken.
///
/// The order moves up behind him, he stands in his own place and counts
/// among the humans (`DAT_00b5cbce`, the caller's count). The record holds
/// his names, level 2, playing strength 1, birthday, difficulty and
/// position, how the public, the fans and the media like him (drawn 40 to
/// 60 each, the fans first), nerves 100, the game's mode, season and day,
/// a trainer's year, money 10,000, the game's own formations
/// (`FUN_00696d87`) and week templates (`FUN_0074bfa8`), no week planned,
/// no national team and at every level abilities of that level. The
/// player standing for him gets his strong foot drawn (`FUN_0070632a`);
/// his names and birthday the country files' reading writes over
/// ([`crate::land_clubs::managers_as_players`]). Not followed: the
/// network's seats (`DAT_00b5cb86`, setup +0x24) and `FUN_006d6245`.
pub fn create(records: &mut Records, rules: &Rules, trainer: &Trainer, random: &mut Random) -> Option<usize> {
    let mut order = records.globals.order();
    let slot = free_slot(order)?;
    order.copy_within(slot - 1..MANAGERS - 1, slot);
    order[slot - 1] = slot as i16;
    records.globals.set_order(order);
    let mut slots = records.globals.slots();
    slots[slot - 1] = 1;
    records.globals.set_slots(slots);
    records.globals.set_humans(records.globals.humans() + 1);
    let (mode, seasons, day) = (records.globals.mode(), records.globals.seasons(), records.globals.day());
    let manager = &mut records.managers[slot - 1];
    fill(manager, rules, trainer, random);
    quirks(random);
    manager.set_level(START_LEVEL);
    manager.set_mode(mode);
    manager.put(0x12c, seasons);
    manager.put(0x12e, day);
    let foot = match random.below(FOOT_DRAW) {
        d if d < FOOT_LEFT_BELOW => FOOT[0],
        d if d < FOOT_RIGHT_BELOW => FOOT[1],
        _ => FOOT[2],
    };
    let player = &mut records.players[MANAGER_PLAYER + slot];
    player.set_manner(player.manner() & !FOOT_BITS | foot << 2);
    Some(slot)
}

/// What `FUN_00630e6c` and `FUN_0070745a` draw for the player standing
/// for the manager, between the popularity and his strong foot. The
/// values they pick (+0x32, +0x14, +0x12 and the foot's +0x28) the fresh
/// record leaves: the first turn's does not hold them, so only the draws
/// are ported. `FUN_00630e6c`: a stand below five wants a position —
/// two when the draw was nought, else one (`0x630eaf`); of the record's
/// ten chances `0x630f0a` a fresh one takes at once, each drawn by the
/// setup's ten. Then two rows below four draw another value
/// (`0x630f89`/`0x630f98`, `0x630fb2`/`0x630fc1`) and two talents below
/// forty (`0x630fdb`, `0x630ff4`); `FUN_0070745a` two more below twenty
/// (`0x7074b6`, `0x7074cf`).
fn quirks(random: &mut Random) {
    let stand = random.below(10);
    if stand < 5 {
        for _ in 0..if stand == 0 { 2 } else { 1 } {
            random.below(10);
        }
    }
    if random.below(10) < 4 {
        random.below(16);
    }
    if random.below(10) < 4 {
        random.below(8);
    }
    random.below(40);
    random.below(40);
    random.below(20);
    random.below(20);
}

/// The record's part of `FUN_006a3b61` but the level, the mode, the
/// season and the day.
fn fill(manager: &mut Manager, rules: &Rules, trainer: &Trainer, random: &mut Random) {
    manager.clear_with_names(&trainer.first_name, &trainer.last_name);
    // +0x33: the playing strength as player-manager.
    manager.put(0x33, 1u8);
    let [day_of, month, year] = trainer.birthday;
    manager.set_birth_day(day_of as u8);
    manager.set_birth_month(month as u8);
    manager.set_birth_year(year as u16);
    manager.set_difficulty(trainer.difficulty as u16);
    manager.set_position(match setup_position(trainer) {
        0 => NO_POSITION,
        p => p.min(LAST_POSITION),
    });
    let fans = POPULARITY_LEAST + random.below(POPULARITY_SPREAD) as u8;
    let public = POPULARITY_LEAST + random.below(POPULARITY_SPREAD) as u8;
    let media = POPULARITY_LEAST + random.below(POPULARITY_SPREAD) as u8;
    manager.set_popularity([public, fans, media]);
    manager.set_nerves(NERVES);
    manager.put(0x11c, BESIDE_NERVES);
    manager.set_trainer_years(1);
    manager.put(0x1f68, 5i16);
    manager.put(0x1f6a, 1i16);
    manager.set_cash(START_CASH);
    manager.put(0x19bf, 1u8);
    for (f, (name, spots)) in rules.pitch.new_formations().iter().enumerate() {
        manager.set_formation(f, name, *spots);
    }
    for (t, template) in rules.training.originals.iter().enumerate() {
        manager.set_template(t, &template.name, template.week);
    }
    manager.set_national_team(0);
    manager.set_abilities(std::array::from_fn(|row| [row as i16; ABILITIES]));
}

/// The position the setup's record holds (+0x20): the birthday dialog's
/// item counted from one, so its default striker (item 9) is 10.
fn setup_position(trainer: &Trainer) -> u8 {
    u8::try_from(trainer.position + 1).unwrap_or(u8::MAX)
}

/// `CEigenschaftenAuswaehlen::OnClicked_IDOK_0048b868`: the abilities
/// chosen stand in the row of his standing (`FUN_006a61e5`), and each row
/// below is the one above it with the highest ability — the last of equals
/// — lowered seven times, never below nought; his playing strength, 1 to 7,
/// at +0x33.
pub fn choose_abilities(manager: &mut Manager, chosen: [i16; ABILITIES], strength: u8) {
    let mut rows = manager.abilities();
    let standing = (manager.standing().max(0) as usize).min(ABILITY_ROWS - 1);
    rows[standing] = chosen;
    for row in (0..standing).rev() {
        rows[row] = rows[row + 1];
        for _ in 0..ABILITIES {
            // `max_by_key` keeps the last of equals, as the original's `<=`.
            let highest = (0..ABILITIES).max_by_key(|&a| rows[row][a]);
            if let Some(a) = highest.filter(|&a| rows[row][a] > 0) {
                rows[row][a] -= 1;
            }
        }
    }
    manager.set_abilities(rows);
    manager.put(0x33, strength.clamp(1, 7));
}

/// The start settings' draw of the order (`FUN_007875b1` after dialog 6,
/// unless "as listed" was chosen): each place from the first swaps with
/// one drawn among all; the slot each place takes its manager from.
pub fn draw_order(humans: usize, random: &mut Random) -> Vec<usize> {
    let mut order: Vec<usize> = (1..=humans).collect();
    for k in 0..humans {
        let j = random.below(humans as u32) as usize;
        order.swap(k, j);
    }
    order
}

/// `FUN_006a4fc5`: the manager records move into the order, place k
/// taking the record of slot `order[k]`, the players standing for them
/// with them, each player keeping his own id (+0x1bc). Not followed: the
/// 0x874-byte blocks moved alike, the human numbers on a club (+0x8a,
/// +0x8b) and the national teams' coaches, none of which a new game
/// without clubs has yet.
pub fn reorder(records: &mut Records, order: &[usize]) {
    let managers: Vec<Manager> = order.iter().map(|&s| records.managers[s - 1].clone()).collect();
    let players: Vec<_> = order.iter().map(|&s| records.players[MANAGER_PLAYER + s].clone()).collect();
    for (k, (manager, mut player)) in managers.into_iter().zip(players).enumerate() {
        records.managers[k] = manager;
        player.set_id((MANAGER_PLAYER + k + 1) as u16);
        records.players[MANAGER_PLAYER + k + 1] = player;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::land_clubs::managers_as_players;

    /// Below the chosen row each is the one above with its highest ability
    /// lowered seven times, the last of equals first.
    #[test]
    fn the_abilities_below_the_chosen_fall_by_seven() {
        let mut records = Records::blank();
        let manager = &mut records.managers[0];
        manager.set_level(2);
        choose_abilities(manager, [6, 2, 2, 2, 1, 1, 0], 3);
        let rows = manager.abilities();
        assert_eq!(rows[2], [6, 2, 2, 2, 1, 1, 0]);
        assert_eq!(rows[1], [2, 1, 1, 1, 1, 1, 0]);
        assert_eq!(rows[0], [0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(manager.get::<u8>(0x33), 3);
    }

    /// Two places, each swapped with one drawn: always both slots once.
    #[test]
    fn the_order_holds_every_slot_once() {
        let mut random = Random::new(7);
        for _ in 0..20 {
            let mut order = draw_order(3, &mut random);
            order.sort();
            assert_eq!(order, [1, 2, 3]);
        }
    }

    /// The managers of `scripts/new-game.txt`, Anna and Bert, entered,
    /// read as players and moved into the order the original drew (Bert
    /// first): their records as the original's first turn holds them. Of
    /// Anna's, who stays without a club, all but what the draws and the
    /// turn's week set (popularity, nerves after the offers she turned
    /// down, +0x24, the board's confidence); of Bert's,
    /// whom his club's signing rewrites, what the setup wrote.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR and the oracle's dumps from $ANSTOSS3_ORACLE"]
    fn a_new_games_managers_as_the_originals_first_turn() {
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
        let exe = a3_files::exe::Exe::open(&std::path::Path::new(&game).join("anstoss3.exe")).unwrap();
        // The game's own templates under the running game's names.
        let mut rules = Rules::load(&exe).unwrap();
        let texts = a3_files::rch::load_texts(&std::path::Path::new(&game).join("deb/ListeDerTexte.rh")).unwrap();
        rules.training.name_originals(&texts);
        let mut records = Records::blank();
        records.globals.set_order([-1; MANAGERS]);
        records.globals.set_mode(dump.globals.mode());
        records.globals.set_seasons(dump.globals.seasons());
        let mut random = Random::new(1);
        for (first, last) in [("Anna", "Alpha"), ("Bert", "Beta")] {
            let trainer = Trainer { first_name: first.into(), last_name: last.into(), ..Trainer::default() };
            create(&mut records, &rules, &trainer, &mut random).unwrap();
        }
        records.names = dump.names.clone();
        managers_as_players(&mut records, dump.globals.main_country() as u8, dump.globals.mode());
        for manager in &mut records.managers[..2] {
            choose_abilities(manager, [2; ABILITIES], 1);
        }
        reorder(&mut records, &[2, 1]);
        assert_eq!(
            (records.globals.order(), records.globals.slots(), records.globals.humans()),
            (dump.globals.order(), dump.globals.slots(), dump.globals.humans())
        );
        // Anna, whole but for what the turn and the draws set.
        let (ours, theirs) = (records.managers[1].bytes(), dump.managers[1].bytes());
        let apart: Vec<usize> = (0..ours.len()).filter(|&b| ours[b] != theirs[b]).collect();
        eprintln!("Anna apart at {apart:x?}");
        let turn = [0x24, 0x11e, 0x126, 0x127, 0x128, 0x184];
        assert!(apart.iter().all(|b| turn.contains(b)), "Anna parts at {apart:x?}");
        // Bert, in what the setup wrote.
        let (ours, theirs) = (&records.managers[0], &dump.managers[0]);
        let setup = |m: &Manager| {
            (
                m.first_name(),
                m.last_name(),
                (m.birth_day(), m.birth_month(), m.birth_year()),
                m.position(),
                m.get::<u8>(0x33),
                m.abilities(),
                m.mode(),
                m.cash(),
                m.formations(),
                m.templates(),
                m.trainer_years(),
            )
        };
        assert_eq!(setup(ours), setup(theirs));
        for k in 0..2 {
            let (ours, theirs) = (&records.players[MANAGER_PLAYER + 1 + k], &dump.players[MANAGER_PLAYER + 1 + k]);
            assert_eq!((ours.id(), ours.first_name(), ours.last_name()), (theirs.id(), theirs.first_name(), theirs.last_name()));
        }
    }
}
