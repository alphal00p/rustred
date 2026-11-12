use super::{Integral, PowerError, SolverError};

/// Optional finite-search seed schedule; it does not change the integral order
/// or turn an exhausted search into a statement of coverage.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub enum CaseSeedSchedule {
    /// The historical zero-first, increasing-L1-shell sequence, unchanged.
    #[default]
    Reference,
    /// Search only these inclusive L1 displacement shells. In particular,
    /// `first_depth == last_depth == 2` actually omits the depth-0/1 sources;
    /// it is not a deeper cap on an otherwise identical first-hit search.
    ShiftShells {
        first_depth: u32,
        last_depth: u32,
        /// Full stored-coordinate permutation; frozen axes are skipped.
        axis_priority: Vec<usize>,
        positive_first: bool,
    },
    /// seed-pose (p-integral R3a): first every seed that moves ONLY symbolic (free) coordinates, in increasing L1
    /// shells up to `symbolic_depth` (numeric coordinates, i.e. dots of lines and numerators of ISPs fixed by the
    /// case, stay at their case value); then, when `then_full`, the complete Reference stream up to `last_depth`
    /// without the seeds already offered. Axis priority / sign order as in `ShiftShells`. Rationale: the rows of
    /// numeric-moving seeds (dots on boundary lines, numerators on zero ISPs) carry the hardest keys; offered first
    /// they become the GPLU pivots that every later row is reduced against, so the first matching pivot's trace
    /// closure drags them in (T162: 1,268 rows / degree 18 vs 39 rows / degree 2 without them).
    SymbolicShells {
        symbolic_depth: u32,
        then_full: bool,
        last_depth: u32,
        axis_priority: Vec<usize>,
        positive_first: bool,
    },
}

impl CaseSeedSchedule {
    /// The last shell this schedule searches (None = unbounded Reference).
    pub fn last_depth(&self) -> Option<u32> {
        match self {
            Self::Reference => None,
            Self::ShiftShells { last_depth, .. } => Some(*last_depth),
            Self::SymbolicShells {
                symbolic_depth,
                then_full,
                last_depth,
                ..
            } => Some(if *then_full {
                (*last_depth).max(*symbolic_depth)
            } else {
                *symbolic_depth
            }),
        }
    }

    pub fn validate(&self, arity: usize) -> Result<(), SolverError> {
        let (first_depth, last_depth, axis_priority) = match self {
            Self::Reference => return Ok(()),
            Self::ShiftShells {
                first_depth,
                last_depth,
                axis_priority,
                ..
            } => (first_depth, last_depth, axis_priority),
            Self::SymbolicShells { axis_priority, .. } => (&0, &0, axis_priority),
        };
        if first_depth > last_depth {
            return Err(SolverError::InvalidInput(
                "seed first depth must not exceed its last depth".into(),
            ));
        }
        if axis_priority.len() != arity {
            return Err(SolverError::InvalidInput(
                "seed axis priority must contain every stored coordinate exactly once".into(),
            ));
        }
        let mut seen = vec![false; arity];
        for &axis in axis_priority {
            let Some(visited) = seen.get_mut(axis) else {
                return Err(SolverError::InvalidInput(
                    "seed axis priority contains an out-of-range coordinate".into(),
                ));
            };
            if std::mem::replace(visited, true) {
                return Err(SolverError::InvalidInput(
                    "seed axis priority contains a repeated coordinate".into(),
                ));
            }
        }
        Ok(())
    }
}

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
    /// Two-sided eps axes: a numeric power may cross between `n >= 1` and
    /// `n <= 0` there (one support, see `SourceSystem::two_sided_axes`).
    two_sided: [bool; N],
    active: [usize; N],
    active_count: usize,
    absolute: [i16; N],
    positive: [bool; N],
    /// False retains the historical boolRunner interpretation exactly.
    positive_first: bool,
    depth: i16,
    first: bool,
    done: bool,
}

impl<const N: usize> Seeds<N> {
    /// Construct the seed stream, including the initial seed consumed by the
    /// reference solver's `do`/`while` loop. `sector` describes symbolic signs;
    /// those signs do not constrain symbolic seeding in the reference algorithm.
    /// Numeric signs are taken from `initial`, as in C++ `makeCurrent`.
    pub fn new(initial: Integral<N>, sector: [bool; N], removed_deltas: [bool; N]) -> Self {
        Self::new_two_sided(initial, sector, removed_deltas, [false; N])
    }

    /// As [`Self::new`], except that a numeric power on a two-sided axis may
    /// leave the sign of its initial value: every integer power of such an
    /// axis lies in one support (plan M2 sec.3.4), so seeds on both sides of
    /// zero instantiate identities of the same sector. All false is `new`.
    pub fn new_two_sided(
        initial: Integral<N>,
        _sector: [bool; N],
        removed_deltas: [bool; N],
        two_sided: [bool; N],
    ) -> Self {
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
            two_sided,
            active,
            active_count,
            absolute: [0; N],
            positive: [false; N],
            positive_first: false,
            depth: 0,
            first: true,
            done: false,
        }
    }

    /// Two-sided axes of this stream (all false for [`Self::new`]).
    pub fn two_sided(&self) -> [bool; N] {
        self.two_sided
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
            if !initial.is_symbolic() && !self.two_sided[index] {
                let delta = if self.positive[active] != self.positive_first {
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
            let delta = if self.positive[active] != self.positive_first {
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

/// Seed visiting schedule of the symbolic case search. The seed BALL is the same; only the order inside each L1 shell
/// changes. Opt-in; the default is the SpIRed chronology.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SeedSchedule {
    #[default]
    Spired,
    /// Within each L1 shell: seeds that move only symbolic coordinates first, then by the L1 of the moves on numeric
    /// coordinates with a positive initial value (dots), then by the L1 on numeric coordinates with a non-positive initial
    /// value (numerator powers): (dots, numerators) ascending, stable in the SpIRed chronology. p5l-lift-trace (T159, T244):
    /// the corner pivot is reached with a 10-20x smaller trace whose exact lift is instantaneous.
    SymbolicFirst,
    /// seed-pose: within each L1 shell, symbolic-only seeds first, then by (numerators, dots) ascending: dot moves
    /// BEFORE numerator moves (the mirror of `SymbolicFirst` for cases whose free axes are lines).
    DotsFirst,
}

/// A seed stream that reports the L1 shell of its most recently emitted seed.
pub trait ShellSeedSource<const N: usize>: Iterator<Item = Result<Seed<N>, PowerError>> {
    fn shell_depth(&self) -> u32;
}

impl<const N: usize> ShellSeedSource<N> for Seeds<N> {
    fn shell_depth(&self) -> u32 {
        self.depth()
    }
}

/// [`Seeds`] (or any [`ShellSeedSource`], e.g. the search's shifted-shell stream) under a [`SeedSchedule`]; buffers one
/// shell at a time for the non-default schedules.
#[derive(Clone, Debug)]
pub struct ScheduledSeeds<const N: usize, I = Seeds<N>> {
    inner: I,
    schedule: SeedSchedule,
    initial: Integral<N>,
    shell: std::collections::VecDeque<Seed<N>>,
    shell_depth: u32,
    carry: Option<(Seed<N>, u32)>,
    error: Option<PowerError>,
}

impl<const N: usize> ScheduledSeeds<N> {
    pub fn new(inner: Seeds<N>, schedule: SeedSchedule) -> Self {
        let initial = inner.initial;
        Self::over(inner, initial, schedule)
    }
}

impl<const N: usize, I: ShellSeedSource<N>> ScheduledSeeds<N, I> {
    /// Schedule any shell source whose seeds start from `initial`.
    pub fn over(inner: I, initial: Integral<N>, schedule: SeedSchedule) -> Self {
        Self {
            inner,
            schedule,
            initial,
            shell: Default::default(),
            shell_depth: 0,
            carry: None,
            error: None,
        }
    }

    /// L1 shell of the most recently emitted seed.
    pub fn depth(&self) -> u32 {
        match self.schedule {
            SeedSchedule::Spired => self.inner.shell_depth(),
            _ => self.shell_depth,
        }
    }

    fn key(&self, seed: &Seed<N>) -> (u32, u32) {
        let (mut dots, mut numerators) = (0u32, 0u32);
        for i in 0..N {
            let p = self.initial[i];
            if p.is_symbolic() {
                continue;
            }
            let delta = (i32::from(seed.integral[i].value()) - i32::from(p.value())).unsigned_abs();
            if p.value() > 0 {
                dots += delta;
            } else {
                numerators += delta;
            }
        }
        (dots, numerators)
    }

    fn fill(&mut self) {
        let (first, depth) = match self.carry.take() {
            Some(c) => c,
            None => match self.inner.next() {
                Some(Ok(seed)) => (seed, self.inner.shell_depth()),
                Some(Err(e)) => {
                    self.error = Some(e);
                    return;
                }
                None => return,
            },
        };
        let mut shell = vec![first];
        loop {
            match self.inner.next() {
                Some(Ok(seed)) => {
                    let d = self.inner.shell_depth();
                    if d != depth {
                        self.carry = Some((seed, d));
                        break;
                    }
                    shell.push(seed);
                }
                Some(Err(e)) => {
                    self.error = Some(e);
                    break;
                }
                None => break,
            }
        }
        if self.schedule == SeedSchedule::DotsFirst {
            shell.sort_by_key(|s| {
                let (dots, numerators) = self.key(s);
                (dots + numerators > 0, numerators, dots)
            });
        } else {
            shell.sort_by_key(|s| self.key(s));
        }
        self.shell_depth = depth;
        self.shell = shell.into();
    }
}

impl<const N: usize, I: ShellSeedSource<N>> Iterator for ScheduledSeeds<N, I> {
    type Item = Result<Seed<N>, PowerError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.schedule == SeedSchedule::Spired {
            return self.inner.next();
        }
        if self.shell.is_empty() && self.error.is_none() {
            self.fill();
        }
        if let Some(seed) = self.shell.pop_front() {
            return Some(Ok(seed));
        }
        self.error.take().map(Err)
    }
}

/// Search-local wrapper. Reference uses exactly the historical Seeds stream;
/// a shifted schedule filters lower shells before any source is instantiated.
/// Its last shell is imposed by SearchOptions in the caller so the existing
/// depth-exhaustion accounting remains the single upper-bound authority.
pub(super) struct CaseSeedStream<const N: usize> {
    inner: Seeds<N>,
    first_depth: u32,
    search_max_depth: Option<u32>,
    /// SymbolicShells: Some((symbolic_depth, then_full, fresh full stream)) while in the symbolic-only phase.
    symbolic: Option<(u32, bool, Seeds<N>)>,
    /// SymbolicShells phase 2: skip symbolic-only seeds of depth <= this (already offered).
    skip_symbolic_upto: Option<u32>,
}

impl<const N: usize> Seeds<N> {
    /// True when the current seed moves no numeric coordinate.
    fn symbolic_only(&self, seed: &Seed<N>) -> bool {
        (0..N).all(|i| {
            let p = self.initial[i];
            p.is_symbolic() || seed.integral[i].value() == p.value()
        })
    }
}

impl<const N: usize> CaseSeedStream<N> {
    pub(super) fn new(
        initial: Integral<N>,
        sector: [bool; N],
        frozen: [bool; N],
        two_sided: [bool; N],
        schedule: &CaseSeedSchedule,
    ) -> Result<Self, SolverError> {
        schedule.validate(N)?;
        let mut inner = Seeds::new_two_sided(initial, sector, frozen, two_sided);
        let mut first_depth = 0;
        let mut symbolic = None;
        let arrange = |inner: &mut Seeds<N>, axis_priority: &[usize], positive_first: bool| {
            inner.positive_first = positive_first;
            let mut priorities = [0; N];
            for (priority, &axis) in axis_priority.iter().enumerate() {
                priorities[axis] = priority;
            }
            inner.active[..inner.active_count].sort_by_key(|&axis| priorities[axis]);
        };
        match schedule {
            CaseSeedSchedule::Reference => {}
            CaseSeedSchedule::ShiftShells {
                first_depth: first,
                axis_priority,
                positive_first,
                ..
            } => {
                first_depth = *first;
                arrange(&mut inner, axis_priority, *positive_first);
            }
            CaseSeedSchedule::SymbolicShells {
                symbolic_depth,
                then_full,
                axis_priority,
                positive_first,
                ..
            } => {
                arrange(&mut inner, axis_priority, *positive_first);
                symbolic = Some((*symbolic_depth, *then_full, inner.clone()));
            }
        }
        Ok(Self {
            inner,
            first_depth,
            search_max_depth: None,
            symbolic,
            skip_symbolic_upto: None,
        })
    }

    pub(super) fn with_search_max_depth(mut self, limit: Option<u32>) -> Self {
        self.search_max_depth = limit;
        self
    }

    pub(super) fn depth(&self) -> u32 {
        self.inner.depth()
    }
}

impl<const N: usize> Iterator for CaseSeedStream<N> {
    type Item = Result<Seed<N>, PowerError>;

    fn next(&mut self) -> Option<Self::Item> {
        // SymbolicShells phase 1: symbolic-only seeds up to the symbolic depth; then the full stream (phase 2)
        // without them. Depths reported by `depth()` are those of the current phase's stream.
        while let Some((depth_cap, then_full, _)) = self.symbolic.as_ref().map(|s| (s.0, s.1, ())) {
            match self.inner.next() {
                Some(Ok(seed)) if self.inner.depth() <= depth_cap => {
                    if self.inner.symbolic_only(&seed) {
                        return Some(Ok(seed));
                    }
                }
                Some(Err(e)) => return Some(Err(e)),
                _ => {
                    let (_, _, fresh) = self.symbolic.take().expect("phase 1");
                    if !then_full {
                        return None;
                    }
                    self.inner = fresh;
                    self.skip_symbolic_upto = Some(depth_cap);
                }
            }
        }
        loop {
            let seed = self.inner.next()?;
            if let (Some(cap), Ok(s)) = (self.skip_symbolic_upto, &seed) {
                if self.inner.depth() <= cap && self.inner.symbolic_only(s) {
                    continue;
                }
            }
            // Expose the first over-budget shell to the caller's existing
            // depth check, rather than filtering up to a larger first_depth.
            if self
                .search_max_depth
                .is_some_and(|limit| self.inner.depth() > limit)
                || self.inner.depth() >= self.first_depth
                || seed.is_err()
            {
                return Some(seed);
            }
        }
    }
}

impl<const N: usize> ShellSeedSource<N> for CaseSeedStream<N> {
    fn shell_depth(&self) -> u32 {
        self.depth()
    }
}

#[cfg(test)]
mod tests {
    use super::super::Power;
    use super::*;

    #[test]
    fn reference_schedule_preserves_the_exact_legacy_seed_sequence() {
        let cases = [
            (Integral::symbolic([0; 3]).unwrap(), [false; 3], [false; 3]),
            (
                Integral::numeric([1, -1, 0]).unwrap(),
                [false; 3],
                [false; 3],
            ),
            (
                Integral::numeric([1, -1, 0]).unwrap(),
                [false, true, false],
                [true; 3],
            ),
        ];
        for (initial, frozen, two_sided) in cases {
            let expected: Vec<_> = Seeds::new_two_sided(initial, [true; 3], frozen, two_sided)
                .take(377)
                .collect();
            let actual: Vec<_> = CaseSeedStream::new(
                initial,
                [true; 3],
                frozen,
                two_sided,
                &CaseSeedSchedule::Reference,
            )
            .unwrap()
            .take(377)
            .collect();
            // Includes the entire integral (numeric replacements) and shifts;
            // comparing only shifts would miss changed numeric seeds.
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn shifted_shells_change_actual_seed_availability_and_priority() {
        let initial = Integral::symbolic([0; 2]).unwrap();
        for (depth, expected) in [
            (1, vec![[-1, 0], [1, 0], [0, -1], [0, 1]]),
            (
                2,
                vec![
                    [-2, 0],
                    [2, 0],
                    [-1, -1],
                    [1, -1],
                    [-1, 1],
                    [1, 1],
                    [0, -2],
                    [0, 2],
                ],
            ),
        ] {
            let schedule = CaseSeedSchedule::ShiftShells {
                first_depth: depth,
                last_depth: depth,
                axis_priority: vec![0, 1],
                positive_first: false,
            };
            let mut seeds =
                CaseSeedStream::new(initial, [true; 2], [false; 2], [false; 2], &schedule).unwrap();
            let mut actual = Vec::new();
            while let Some(seed) = seeds.next() {
                if seeds.depth() > depth {
                    break;
                }
                let seed = seed.unwrap();
                assert_eq!(
                    seed.shifts
                        .iter()
                        .map(|x| u32::from(x.unsigned_abs()))
                        .sum::<u32>(),
                    depth
                );
                actual.push(seed.shifts);
            }
            assert_eq!(actual, expected);
        }
        let reversed = CaseSeedSchedule::ShiftShells {
            first_depth: 1,
            last_depth: 1,
            axis_priority: vec![1, 0],
            positive_first: true,
        };
        let actual: Vec<_> =
            CaseSeedStream::new(initial, [true; 2], [false; 2], [false; 2], &reversed)
                .unwrap()
                .take(4)
                .map(|s| s.unwrap().shifts)
                .collect();
        assert_eq!(actual, [[0, 1], [0, -1], [1, 0], [-1, 0]]);
    }

    #[test]
    fn numeric_schedule_records_changed_integrals_even_when_shifts_are_zero() {
        let schedule = CaseSeedSchedule::ShiftShells {
            first_depth: 1,
            last_depth: 1,
            axis_priority: vec![1, 0],
            positive_first: true,
        };
        let actual: Vec<_> = CaseSeedStream::new(
            Integral::numeric([1, -1]).unwrap(),
            [true, false],
            [false; 2],
            [false; 2],
            &schedule,
        )
        .unwrap()
        .take(3)
        .map(|s| s.unwrap())
        .collect();
        assert!(actual.iter().all(|s| s.shifts == [0; 2]));
        assert_eq!(
            actual
                .iter()
                .map(|s| s.integral.powers().map(Power::value))
                .collect::<Vec<_>>(),
            [[1, 0], [1, -2], [2, -1]]
        );
    }

    #[test]
    fn scheduled_seed_axes_and_depths_fail_closed_on_invalid_descriptors() {
        for schedule in [
            CaseSeedSchedule::ShiftShells {
                first_depth: 2,
                last_depth: 1,
                axis_priority: vec![0, 1],
                positive_first: false,
            },
            CaseSeedSchedule::ShiftShells {
                first_depth: 1,
                last_depth: 2,
                axis_priority: vec![0, 0],
                positive_first: false,
            },
            CaseSeedSchedule::ShiftShells {
                first_depth: 1,
                last_depth: 2,
                axis_priority: vec![0, 2],
                positive_first: false,
            },
            CaseSeedSchedule::ShiftShells {
                first_depth: 1,
                last_depth: 2,
                axis_priority: vec![0],
                positive_first: false,
            },
        ] {
            assert!(schedule.validate(2).is_err());
        }
    }

    #[test]
    fn shell_filter_does_not_hide_a_tighter_search_limit() {
        let schedule = CaseSeedSchedule::ShiftShells {
            first_depth: 2,
            last_depth: 2,
            axis_priority: vec![0, 1],
            positive_first: false,
        };
        let mut seeds = CaseSeedStream::new(
            Integral::symbolic([0; 2]).unwrap(),
            [true; 2],
            [false; 2],
            [false; 2],
            &schedule,
        )
        .unwrap()
        .with_search_max_depth(Some(0));
        // The caller sees depth1 and refuses before source instantiation.
        // It must not silently enumerate until the requested first depth2.
        assert!(seeds.next().unwrap().is_ok());
        assert_eq!(seeds.depth(), 1);
    }

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
    fn two_sided_axes_cross_zero_and_all_false_is_new() {
        let initial = Integral::numeric([1, 1]).unwrap();
        let powers = |seeds: Seeds<2>| -> Vec<[i16; 2]> {
            seeds
                .take(60)
                .map(|seed| seed.unwrap().integral.powers().map(|p| p.value()))
                .collect()
        };
        let plain = powers(Seeds::new(initial, [true; 2], [false; 2]));
        assert_eq!(
            plain,
            powers(Seeds::new_two_sided(
                initial, [true; 2], [false; 2], [false; 2]
            ))
        );
        // A numeric power keeps its sign unless its axis is two-sided.
        assert!(plain.iter().all(|power| power.iter().all(|&n| n >= 1)));
        let crossing = powers(Seeds::new_two_sided(
            initial,
            [true; 2],
            [false; 2],
            [true, false],
        ));
        assert!(crossing.iter().any(|power| power[0] <= 0));
        assert!(crossing.iter().all(|power| power[1] >= 1));
    }

    #[test]
    fn symbolic_first_schedule_permutes_each_shell_only() {
        // axis 0 symbolic, axis 1 numeric dot (value 1), axis 2 numeric numerator (value 0)
        let initial = Integral::new([
            Power::new(true, 0).unwrap(),
            Power::new(false, 1).unwrap(),
            Power::new(false, 0).unwrap(),
        ]);
        let plain: Vec<_> = Seeds::new(initial, [true, true, false], [false; 3])
            .take_while(|s| s.is_ok())
            .map(|s| s.unwrap())
            .take(60)
            .collect();
        let mut sched = ScheduledSeeds::new(
            Seeds::new(initial, [true, true, false], [false; 3]),
            SeedSchedule::SymbolicFirst,
        );
        let mut got = Vec::new();
        let mut depths = Vec::new();
        for _ in 0..plain.len() {
            got.push(sched.next().unwrap().unwrap());
            depths.push(sched.depth());
        }
        // same multiset per shell, symbolic-only first inside every shell, shells in order
        let l1 = |s: &Seed<3>| {
            (0..3)
                .map(|i| (s.integral[i].value() - initial[i].value()).unsigned_abs() as u32)
                .sum::<u32>()
        };
        assert!(got.windows(2).all(|w| l1(&w[0]) <= l1(&w[1])));
        assert!(depths.windows(2).all(|w| w[0] <= w[1]));
        let shell2: Vec<_> = got.iter().filter(|s| l1(s) == 2).collect();
        assert_eq!(shell2[0].integral[1].value(), 1);
        assert_eq!(shell2[0].integral[2].value(), 0);
        let mut a: Vec<_> = plain
            .iter()
            .filter(|s| l1(s) <= 2)
            .map(|s| s.integral)
            .collect();
        let mut b: Vec<_> = got
            .iter()
            .filter(|s| l1(s) <= 2)
            .map(|s| s.integral)
            .collect();
        a.sort_by_key(|i| i.powers().map(|p| p.value()));
        b.sort_by_key(|i| i.powers().map(|p| p.value()));
        assert_eq!(a, b);
        let spired: Vec<_> = ScheduledSeeds::new(
            Seeds::new(initial, [true; 3], [false; 3]),
            SeedSchedule::Spired,
        )
        .take(20)
        .map(|s| s.unwrap().shifts)
        .collect();
        let raw: Vec<_> = Seeds::new(initial, [true; 3], [false; 3])
            .take(20)
            .map(|s| s.unwrap().shifts)
            .collect();
        assert_eq!(spired, raw);
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
