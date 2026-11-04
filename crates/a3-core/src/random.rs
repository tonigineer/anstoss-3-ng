//! The game's random numbers: L'Ecuyer's combined generator, of whose
//! result the game takes the high bits.
//!
//! For comparing a run with the original's, the draws can be recorded —
//! each draw's range, result and the line that asked for it — and set
//! against the probe's trace of the original's draws (`tools/probe/
//! draws.py --replica`): where the two first part is where the replica's
//! order of draws leaves the original's.

use std::panic::Location;
use std::sync::Mutex;

const M1: i64 = 2_147_483_563;
const M2: i64 = 2_147_483_399;

#[derive(Clone, Debug, PartialEq)]
pub struct Random {
    s1: i64,
    s2: i64,
}

impl Random {
    pub fn new(seed: u64) -> Self {
        let s1 = 1 + (seed % (M1 as u64 - 1)) as i64;
        let s2 = 1 + (seed.rotate_left(29) % (M2 as u64 - 1)) as i64;
        Self { s1, s2 }
    }

    /// A generator whose state the game sets itself, both halves. A
    /// recording starts afresh here: the run to compare starts where both
    /// generators were set alike.
    pub fn from_state(s1: i64, s2: i64) -> Self {
        if let Ok(mut draws) = DRAWS.lock()
            && let Some(draws) = draws.as_mut()
        {
            draws.clear();
        }
        Self { s1, s2 }
    }

    /// Original: FUN_0078a8f4, on the state at `DAT_0089dcc0`/`DAT_0089dcc4`.
    fn next(&mut self) -> i64 {
        self.s1 = 40_014 * self.s1 % M1;
        self.s2 = 40_692 * self.s2 % M2;
        let z = self.s1 - self.s2;
        if z < 1 { z + M1 - 1 } else { z }
    }

    /// A number below `n`; 0 when there is no choice.
    ///
    /// Original: FUN_0078a810
    #[track_caller]
    pub fn below(&mut self, n: u32) -> u32 {
        if n < 2 {
            return 0;
        }
        let result = (self.next() >> 16) as u32 % n;
        if let Ok(mut draws) = DRAWS.lock()
            && let Some(draws) = draws.as_mut()
        {
            draws.push(Draw { range: n, result, at: Location::caller() });
        }
        result
    }
}

/// A draw as recorded: its range, its result and where it was asked for.
#[derive(Clone, Copy, Debug)]
pub struct Draw {
    pub range: u32,
    pub result: u32,
    pub at: &'static Location<'static>,
}

/// The draws recorded, while a recording runs.
static DRAWS: Mutex<Option<Vec<Draw>>> = Mutex::new(None);

/// Starts recording every draw, or stops and forgets them.
pub fn record(on: bool) {
    if let Ok(mut draws) = DRAWS.lock() {
        *draws = on.then(Vec::new);
    }
}

/// The draws recorded so far, the recording going on.
pub fn recorded() -> Vec<Draw> {
    DRAWS.lock().ok().and_then(|d| d.clone()).unwrap_or_default()
}

/// The recorded draws as text, a line each: range, result and the line
/// that drew, tab-separated.
pub fn draws_text(draws: &[Draw]) -> String {
    draws.iter().map(|d| format!("{}\t{}\t{}:{}\n", d.range, d.result, d.at.file(), d.at.line())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_stay_below_their_bound() {
        let mut random = Random::new(12345);
        assert!((0..10_000).all(|_| random.below(7) < 7));
        assert_eq!(random.below(1), 0);
        let mut seen = [false; 7];
        (0..1000).for_each(|_| seen[random.below(7) as usize] = true);
        assert!(seen.iter().all(|&s| s));
    }

    /// The generator steps as the original's FUN_0078a8f4 does on its
    /// state: Schrage's 40014 and 40692 on 0x7fffffab and 0x7fffff07, the
    /// difference wrapped below one, its high bits taken.
    #[test]
    fn the_generator_steps_as_the_original() {
        let original = |s1: &mut i32, s2: &mut i32| -> i32 {
            *s1 = s1.wrapping_mul(0x9c4e).wrapping_add((*s1 / 0xd1a4).wrapping_mul(-0x7fffffab));
            if *s1 < 0 {
                *s1 = s1.wrapping_add(0x7fffffab);
            }
            *s2 = s2.wrapping_mul(0x9ef4).wrapping_add((*s2 / 0xce26).wrapping_mul(-0x7fffff07));
            if *s2 < 0 {
                *s2 = s2.wrapping_add(0x7fffff07);
            }
            let mut z = s1.wrapping_sub(*s2);
            if z < 1 {
                z = z.wrapping_add(0x7fffffaa);
            }
            z >> 16
        };
        let (mut s1, mut s2) = (123_456_789, 987_654_321);
        let mut replica = Random::from_state(i64::from(s1), i64::from(s2));
        for n in [2u32, 3, 7, 10, 100, 1000, 32768] {
            for _ in 0..1000 {
                assert_eq!(replica.below(n), original(&mut s1, &mut s2) as u32 % n);
            }
        }
    }
}
