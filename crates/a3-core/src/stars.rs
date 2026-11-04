//! The game's best players, picked anew as the new season opens
//! (`FUN_0065e659`'s last `FUN_0077841a`, once the squads stand): over every
//! club of every country with clubs, the best player of each position — the
//! star whose market value counts a fifth more (`DAT_00b5dfd0`) — and the
//! best of each ability, the field players' sixteen and the keepers' nine
//! (`DAT_00b5dfe6`, `DAT_00b5e006`).
//!
//! A player counts his strength on his main position a hundred times
//! (`FUN_006a144e`) and his abilities' and character's percent with a
//! hundred, by his form today — 0.85 at nought to 1.0 at ten
//! (`FUN_00708b80`); each list keeps its two best (`FUN_007787a0`), the
//! first found ahead of an equal one.

use crate::data::Data;
use crate::records::Records;

/// Positions 1 to 10; abilities of the field players, then the keepers'.
const POSITIONS: usize = 11;
const ABILITIES: usize = 25;
const FIELD_ABILITIES: usize = 16;
const KEEPER_ABILITIES: usize = 9;
const KEEPER: u8 = 1;
const COUNTRIES: usize = 0x75;

/// The two best of a list (`FUN_007787a0`): a new one goes in once it
/// beats the second.
#[derive(Clone, Copy, Default)]
struct Best {
    count: usize,
    ids: [u16; 3],
    scores: [i16; 3],
}

impl Best {
    fn offer(&mut self, id: u16, score: i16) {
        if self.scores[1] >= score {
            return;
        }
        let mut k = self.count;
        while k > 0 && self.scores[k - 1] < score {
            self.scores[k] = self.scores[k - 1];
            self.ids[k] = self.ids[k - 1];
            k -= 1;
        }
        self.scores[k] = score;
        self.ids[k] = id;
        if self.count < 2 {
            self.count += 1;
        }
    }

    fn first(&self) -> u16 {
        if self.count >= 1 { self.ids[0] } else { 0 }
    }
}

/// The stars and the best of each ability, written to the globals.
pub fn pick(records: &mut Records, data: &Data) {
    let mut positions = [Best::default(); POSITIONS];
    let mut abilities = [Best::default(); ABILITIES];
    for country in 0..COUNTRIES {
        if records.countries.kind(country) == 0 {
            continue;
        }
        for number in 0..records.countries.team_count(country).max(0) as u16 {
            let Some(team) = records.team(country as u16, number) else { continue };
            let club = crate::load::club_state_of(records, team, data);
            for (p, (player, id)) in club.players.iter().zip(team.players()).enumerate() {
                let Some(record) = records.player(id) else { continue };
                let position = player.positions[0];
                let fitness = crate::lineup::fitness_of(&club.fitness, p);
                let strength = data.rules.pitch.strength_at(player, fitness, position);
                let form = f64::from(crate::statistics::form_today(record));
                let percent = data.rules.value.pick_percent(record.good(), record.traits(), position == KEEPER);
                let share = (form * 1.5 + 85.0) / 100.0;
                let score = (strength * 100 + (f64::from(percent + 100) * share).trunc() as i32) as i16;
                if let Some(best) = positions.get_mut(usize::from(position)) {
                    best.offer(id, score);
                }
                let (first, count) = if position == KEEPER { (FIELD_ABILITIES, KEEPER_ABILITIES) } else { (0, FIELD_ABILITIES) };
                for b in 0..count {
                    if record.good() & (1 << b) != 0 {
                        abilities[first + b].offer(id, score);
                    }
                }
            }
        }
    }
    let mut stars = records.globals.stars();
    for (k, best) in positions.iter().enumerate().skip(1) {
        stars[k] = best.first();
    }
    records.globals.set_stars(stars);
    let mut field = records.globals.best_by_ability();
    let mut keepers = records.globals.best_keepers_by_ability();
    for (k, best) in abilities.iter().enumerate() {
        let id = best.first() as i16;
        if k < FIELD_ABILITIES {
            field[k] = id;
        } else {
            keepers[k - FIELD_ABILITIES] = id;
        }
    }
    records.globals.set_best_by_ability(field);
    records.globals.set_best_keepers_by_ability(keepers);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The original's NewSeason000 picked again from its own squads: its
    /// stars and its best of every ability come out as the original's.
    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_stars_are_picked_as_the_original() {
        use a3_files::an3::Save;
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let keeps = dir.parent().unwrap().join("saves-kept");
        let load = |name: &str| Records::from_save(Save::read(&keeps.join(name)).unwrap()).unwrap();
        let (mut b, n) = (load("NewSeason000.an3"), load("NewSeason000.an3"));
        let data = Data::load(&dir).unwrap();
        b.globals.set_stars([0; 11]);
        b.globals.set_best_by_ability([0; 16]);
        b.globals.set_best_keepers_by_ability([0; 9]);
        pick(&mut b, &data);
        assert_eq!(b.globals.stars(), n.globals.stars());
        assert_eq!(b.globals.best_by_ability(), n.globals.best_by_ability());
        assert_eq!(b.globals.best_keepers_by_ability(), n.globals.best_keepers_by_ability());
    }
}
