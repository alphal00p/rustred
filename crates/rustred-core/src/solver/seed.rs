use super::{Integral, PowerError};

/// One seed for a simple coordinate case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seed<const N: usize> {
    /// Symbolic powers include their displacement; numeric powers contain their
    /// replacement values. Removed delta coordinates retain their initial value.
    pub integral: Integral<N>,
    /// The C++ `currentShifts` array: numeric and removed coordinates are zero.
    pub shifts: [i16; N],
}

/// Streaming equivalent of SpIRed's `seedRunner` for simple coordinate cases.
///
/// Seeds begin at zero displacement and proceed through increasing L1 shells.
/// Within a shell, absolute shifts follow `integerPartitionPermRunner` and signs
/// follow `boolRunner`: negative first, with the earliest nonzero coordinate
/// changing sign fastest. Numeric coordinates stay in their initial sector;
/// symbolic coordinates may shift in either direction. Storage is O(N), with no
/// shell collection. The first unrepresentable valid seed yields an error and
/// terminates the iterator instead of wrapping a compact power.
#[derive(Clone, Debug)]
pub struct Seeds<const N: usize> {
    initial: Integral<N>,
    active: [usize; N],
    active_count: usize,
    absolute: [i16; N],
    positive: [bool; N],
    depth: i16,
    first: bool,
    done: bool,
}

impl<const N: usize> Seeds<N> {
    /// Construct the seed stream, including the initial seed consumed by the
    /// reference solver's `do`/`while` loop. `sector` describes symbolic signs;
    /// those signs do not constrain symbolic seeding in the reference algorithm.
    /// Numeric signs are taken from `initial`, as in C++ `makeCurrent`.
    pub fn new(initial: Integral<N>, _sector: [bool; N], removed_deltas: [bool; N]) -> Self {
        let mut active = [0; N];
        let mut active_count = 0;
        for (index, removed) in removed_deltas.into_iter().enumerate() {
            if !removed {
                active[active_count] = index;
                active_count += 1;
            }
        }
        Self {
            initial,
            active,
            active_count,
            absolute: [0; N],
            positive: [false; N],
            depth: 0,
            first: true,
            done: false,
        }
    }

    /// The L1 displacement of the most recently emitted seed.
    pub fn depth(&self) -> u32 {
        self.depth as u32
    }

    fn advance(&mut self) {
        // boolRunner increments its earliest entry first. Zero components have
        // no sign bit and therefore must be skipped.
        for index in 0..self.active_count {
            if self.absolute[index] != 0 {
                self.positive[index] = !self.positive[index];
                if self.positive[index] {
                    return;
                }
            }
        }

        // Exact integerPartitionPermRunner::next transition, using only the
        // current composition. Start each new composition with negative signs.
        if let Some(last) = (0..self.active_count).rfind(|&i| self.absolute[i] != 0) {
            if last + 1 < self.active_count {
                self.absolute[last] -= 1;
                self.absolute[last + 1] = 1;
                return;
            }
            let deficiency = self.absolute[last];
            self.absolute[last] = 0;
            if let Some(previous) = (0..last).rfind(|&i| self.absolute[i] != 0) {
                self.absolute[previous] -= 1;
                self.absolute[previous + 1] = deficiency + 1;
                return;
            }
        }

        // Compact initial powers guarantee an overflow seed by depth 128, long
        // before this i16 shell counter can overflow.
        self.depth += 1;
        self.absolute[0] = self.depth;
    }

    fn current(&self) -> Result<Option<Seed<N>>, PowerError> {
        // Test every numeric sector constraint before constructing powers: an
        // inadmissible seed must not become an artificial compact-range error.
        for active in 0..self.active_count {
            let index = self.active[active];
            let initial = self.initial[index];
            if !initial.is_symbolic() {
                let delta = if self.positive[active] {
                    self.absolute[active]
                } else {
                    -self.absolute[active]
                };
                if (i32::from(initial.value()) + i32::from(delta) > 0) != (initial.value() > 0) {
                    return Ok(None);
                }
            }
        }

        let mut powers = *self.initial.powers();
        let mut shifts = [0; N];
        for active in 0..self.active_count {
            let index = self.active[active];
            let delta = if self.positive[active] {
                self.absolute[active]
            } else {
                -self.absolute[active]
            };
            powers[index] = powers[index].shifted(delta)?;
            if powers[index].is_symbolic() {
                shifts[index] = delta;
            }
        }
        Ok(Some(Seed {
            integral: Integral::new(powers),
            shifts,
        }))
    }
}

impl<const N: usize> Iterator for Seeds<N> {
    type Item = Result<Seed<N>, PowerError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        if self.first {
            self.first = false;
            return Some(Ok(Seed {
                integral: self.initial,
                shifts: [0; N],
            }));
        }
        if self.active_count == 0 {
            self.done = true;
            return None;
        }
        loop {
            self.advance();
            match self.current() {
                Ok(Some(seed)) => return Some(Ok(seed)),
                Ok(None) => continue,
                Err(error) => {
                    self.done = true;
                    return Some(Err(error));
                }
            }
        }
    }
}

impl<const N: usize> std::iter::FusedIterator for Seeds<N> {}

#[cfg(test)]
mod tests {
    use super::super::Power;
    use super::*;

    #[test]
    fn shell_and_sign_chronology_matches_cpp() {
        let initial = Integral::symbolic([0; 3]).unwrap();
        let actual: Vec<_> = Seeds::new(initial, [true, false, true], [false; 3])
            .take(25)
            .map(|seed| seed.unwrap().shifts)
            .collect();
        // integerPartitionPermRunner(2,3): 200,110,101,020,011,002.
        // boolRunner starts false/negative and flips the earliest sign first.
        assert_eq!(
            actual,
            [
                [0, 0, 0],
                [-1, 0, 0],
                [1, 0, 0],
                [0, -1, 0],
                [0, 1, 0],
                [0, 0, -1],
                [0, 0, 1],
                [-2, 0, 0],
                [2, 0, 0],
                [-1, -1, 0],
                [1, -1, 0],
                [-1, 1, 0],
                [1, 1, 0],
                [-1, 0, -1],
                [1, 0, -1],
                [-1, 0, 1],
                [1, 0, 1],
                [0, -2, 0],
                [0, 2, 0],
                [0, -1, -1],
                [0, 1, -1],
                [0, -1, 1],
                [0, 1, 1],
                [0, 0, -2],
                [0, 0, 2],
            ]
        );
    }

    #[test]
    fn six_seed_shells_match_compiled_spired_fixture() {
        // FNV-1a of each signed shift cast to uint8_t, generated directly by
        // vendored integerPartitionPermRunner<int>(depth,3) and boolRunner for
        // depths 0..=6. The C++ oracle used GCC 14.4.0 and combinatorics.hpp.
        let mut seeds = Seeds::new(Integral::symbolic([0; 3]).unwrap(), [true; 3], [false; 3]);
        let mut hash = 14_695_981_039_346_656_037u64;
        let mut count = 0;
        while let Some(seed) = seeds.next() {
            let seed = seed.unwrap();
            if seeds.depth() > 6 {
                break;
            }
            count += 1;
            for shift in seed.shifts {
                hash = (hash ^ u64::from(shift as u8)).wrapping_mul(1_099_511_628_211);
            }
        }
        assert_eq!(count, 377);
        assert_eq!(hash, 14_745_755_234_591_046_419);
    }

    #[test]
    fn numeric_seeds_preserve_sector_and_are_replacements() {
        let initial = Integral::numeric([1, -1]).unwrap();
        let mut seeds = Seeds::new(initial, [true, false], [false; 2]);
        let actual: Vec<_> = seeds
            .by_ref()
            .take(8)
            .map(|seed| {
                let seed = seed.unwrap();
                assert_eq!(seed.shifts, [0; 2]);
                assert!(seed.integral[0].value() > 0);
                assert!(seed.integral[1].value() <= 0);
                seed.integral.powers().map(Power::value)
            })
            .collect();
        assert_eq!(
            actual,
            [
                [1, -1],
                [2, -1],
                [1, -2],
                [1, 0],
                [3, -1],
                [2, -2],
                [2, 0],
                [1, -3]
            ]
        );
        assert_eq!(seeds.depth(), 2);
    }

    #[test]
    fn removed_delta_coordinates_do_not_participate() {
        let initial = Integral::new([
            Power::new(true, 0).unwrap(),
            Power::new(false, 1).unwrap(),
            Power::new(true, 3).unwrap(),
        ]);
        let actual: Vec<_> = Seeds::new(initial, [true; 3], [true, false, false])
            .take(4)
            .map(|seed| seed.unwrap())
            .collect();
        assert_eq!(actual[0].integral, initial);
        assert_eq!(actual[1].integral.powers().map(Power::value), [0, 2, 3]);
        assert_eq!(actual[2].shifts, [0, 0, -1]);
        assert_eq!(actual[3].shifts, [0, 0, 1]);
        assert_eq!(actual[2].integral[2].value(), 2);
    }

    #[test]
    fn no_active_coordinates_still_emit_the_initial_seed_once() {
        let initial = Integral::numeric([1; 2]).unwrap();
        let mut removed = Seeds::new(initial, [true; 2], [true; 2]);
        assert_eq!(removed.next().unwrap().unwrap().integral, initial);
        assert!(removed.next().is_none());
        assert!(removed.next().is_none());
        assert_eq!(Seeds::new(Integral::<0>::default(), [], []).count(), 1);
    }

    #[test]
    fn compact_overflow_is_reported_once_without_wrapping() {
        let mut seeds = Seeds::new(Integral::symbolic([63]).unwrap(), [true], [false]);
        assert_eq!(seeds.next().unwrap().unwrap().integral[0].value(), 63);
        assert_eq!(seeds.next().unwrap().unwrap().integral[0].value(), 62);
        assert_eq!(
            seeds.next(),
            Some(Err(PowerError::OutOfRange { value: 64 }))
        );
        assert!(seeds.next().is_none());

        let mut seeds = Seeds::new(Integral::numeric([-64]).unwrap(), [false], [false]);
        seeds.next().unwrap().unwrap();
        assert_eq!(
            seeds.next(),
            Some(Err(PowerError::OutOfRange { value: -65 }))
        );
    }

    #[test]
    fn seed_storage_does_not_grow_with_shell_size() {
        assert!(std::mem::size_of::<Seeds<100>>() < 1500);
        let seeds = Seeds::new(
            Integral::symbolic([0; 100]).unwrap(),
            [true; 100],
            [false; 100],
        );
        assert_eq!(seeds.take(500).filter_map(Result::ok).count(), 500);
    }
}
