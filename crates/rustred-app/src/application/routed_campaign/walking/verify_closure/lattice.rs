//! Exact integer-set predicates for the offline closure verifier, written
//! independently of the walker's containment code (`DomainPowerSummary`,
//! `CompactDomain::contains`, the bucket index). Integer interval sums only;
//! this is neither a polyhedral nor a computer-algebra solver.
//!
//! A cell is `{x in N^n : lower <= x <= upper, R <= rank, A <= a_max,
//! d_min <= D <= d_max}` with physical indices `n_i = x_i + 1` on owner axes
//! and `n_i = -x_i` elsewhere, `A = sum(max(n_i, 0)) = sum_owner(x_i + 1)`,
//! `R = sum_other(x_i)` and `D = A - R`. A and R are sums of integer
//! intervals over disjoint axis groups, so each takes every integer between
//! its box extremes and (A, R) ranges over a full integer rectangle; D then
//! takes every integer between the rectangle's extremes. Emptiness is an
//! interval test; `inner <= outer` holds iff inner meets the negation of no
//! outer constraint. `for_each_point` enumerates small cells as a brute-force
//! cross-check of both.
//!
//! `covered_by_union` decides `Q <= T_1 u ... u T_k` exactly (the G2'
//! "anchor scopes plus residual cover Q" question). It splits Q minus T_1 into
//! disjoint regions `Q ^ c_1 ^ ... ^ c_{j-1} ^ not c_j` over T_1's
//! constraints `c_j` and recurses on T_2..T_k. Every region is a box with
//! interval bounds on A, R and D; the same interval argument decides its
//! emptiness exactly, so no region is approximated. Only the budget on the
//! number of regions makes an answer undecided (None), never wrong.
use rustred::solver::DomainPowerBounds;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct Cell {
    pub owner: Vec<bool>,
    pub lower: Vec<u64>,
    pub upper: Vec<Option<u64>>,
    pub rank: Option<u32>,
    pub powers: DomainPowerBounds,
}

/// Additional constraints on a cell: raised coordinate lowers, lowered
/// coordinate uppers and A/R/D intervals (None: no extra bound).
#[derive(Clone, Copy, Default)]
struct Extra {
    raise: Option<(usize, u64)>,
    cut: Option<(usize, u64)>,
    a_low: Option<i128>,
    r_low: Option<i128>,
    d_low: Option<i128>,
    d_high: Option<i128>,
}

fn cap(value: Option<i128>, bound: Option<i128>) -> Option<i128> {
    match (value, bound) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}
fn floor(value: Option<i128>, bound: Option<i128>) -> Option<i128> {
    match (value, bound) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
}

/// A box with interval bounds on A, R and D (None: unbounded on that side):
/// one disjoint piece of `Q minus (T_1 u ... u T_j)` in `covered_by_union`.
#[derive(Clone, Debug)]
struct Region {
    owner: Vec<bool>,
    lower: Vec<u64>,
    upper: Vec<Option<u64>>,
    a: (Option<i128>, Option<i128>),
    r: (Option<i128>, Option<i128>),
    d: (Option<i128>, Option<i128>),
}

/// One constraint of a cell, in the order `Region::minus` splits on them.
#[derive(Clone, Copy)]
enum Bound {
    AxisLow(usize, u64),
    AxisHigh(usize, u64),
    RHigh(i128),
    AHigh(i128),
    DLow(i128),
    DHigh(i128),
}

impl Region {
    fn of(cell: &Cell) -> Self {
        Self {
            owner: cell.owner.clone(),
            lower: cell.lower.clone(),
            upper: cell.upper.clone(),
            a: (None, cell.powers.max_positive_power.map(i128::from)),
            r: (None, cell.rank.map(i128::from)),
            d: (
                cell.powers.min_power_difference.map(i128::from),
                cell.powers.max_power_difference.map(i128::from),
            ),
        }
    }

    /// Exact: A and R are sums of integer intervals over disjoint axis
    /// groups, so (A, R) fills an integer rectangle and D = A - R takes
    /// every integer between its extremes.
    fn nonempty(&self) -> bool {
        let (mut a_low, mut a_high, mut r_low, mut r_high) =
            (0i128, Some(0i128), 0i128, Some(0i128));
        for axis in 0..self.owner.len() {
            let (low, high) = (
                i128::from(self.lower[axis]),
                self.upper[axis].map(i128::from),
            );
            if high.is_some_and(|high| high < low) {
                return false;
            }
            let shift = i128::from(self.owner[axis]);
            let (sum_low, sum_high) = if self.owner[axis] {
                (&mut a_low, &mut a_high)
            } else {
                (&mut r_low, &mut r_high)
            };
            *sum_low += low + shift;
            *sum_high = sum_high.zip(high).map(|(s, h)| s + h + shift);
        }
        let a_low = floor(Some(a_low), self.a.0).expect("finite");
        let a_high = cap(a_high, self.a.1);
        let r_low = floor(Some(r_low), self.r.0).expect("finite");
        let r_high = cap(r_high, self.r.1);
        if a_high.is_some_and(|high| high < a_low) || r_high.is_some_and(|high| high < r_low) {
            return false;
        }
        let d_low = floor(r_high.map(|r| a_low - r), self.d.0);
        let d_high = cap(a_high.map(|a| a - r_low), self.d.1);
        !matches!((d_low, d_high), (Some(low), Some(high)) if low > high)
    }

    fn with(&self, bound: Bound, negated: bool) -> Option<Self> {
        let mut region = self.clone();
        match (bound, negated) {
            (Bound::AxisLow(axis, low), false) => {
                region.lower[axis] = region.lower[axis].max(low);
            }
            (Bound::AxisLow(axis, low), true) => {
                let high = low.checked_sub(1)?;
                region.upper[axis] = Some(region.upper[axis].map_or(high, |u| u.min(high)));
            }
            (Bound::AxisHigh(axis, high), false) => {
                region.upper[axis] = Some(region.upper[axis].map_or(high, |u| u.min(high)));
            }
            (Bound::AxisHigh(axis, high), true) => {
                region.lower[axis] = region.lower[axis].max(high.checked_add(1)?);
            }
            (Bound::RHigh(v), false) => region.r.1 = cap(region.r.1, Some(v)),
            (Bound::RHigh(v), true) => region.r.0 = floor(region.r.0, Some(v + 1)),
            (Bound::AHigh(v), false) => region.a.1 = cap(region.a.1, Some(v)),
            (Bound::AHigh(v), true) => region.a.0 = floor(region.a.0, Some(v + 1)),
            (Bound::DLow(v), false) => region.d.0 = floor(region.d.0, Some(v)),
            (Bound::DLow(v), true) => region.d.1 = cap(region.d.1, Some(v - 1)),
            (Bound::DHigh(v), false) => region.d.1 = cap(region.d.1, Some(v)),
            (Bound::DHigh(v), true) => region.d.0 = floor(region.d.0, Some(v + 1)),
        }
        region.nonempty().then_some(region)
    }

    /// The nonempty disjoint pieces of `self minus target` (same owner).
    fn minus(&self, target: &Cell) -> Vec<Self> {
        let mut bounds = Vec::new();
        for axis in 0..target.owner.len() {
            bounds.push(Bound::AxisLow(axis, target.lower[axis]));
            if let Some(high) = target.upper[axis] {
                bounds.push(Bound::AxisHigh(axis, high));
            }
        }
        bounds.extend(target.rank.map(|r| Bound::RHigh(i128::from(r))));
        let powers = &target.powers;
        bounds.extend(
            powers
                .max_positive_power
                .map(|a| Bound::AHigh(i128::from(a))),
        );
        bounds.extend(
            powers
                .min_power_difference
                .map(|d| Bound::DLow(i128::from(d))),
        );
        bounds.extend(
            powers
                .max_power_difference
                .map(|d| Bound::DHigh(i128::from(d))),
        );
        let mut pieces = Vec::new();
        let mut rest = self.clone();
        for bound in bounds {
            pieces.extend(rest.with(bound, true));
            match rest.with(bound, false) {
                Some(next) => rest = next,
                None => break,
            }
        }
        pieces
    }

    fn covered(&self, targets: &[&Cell], budget: &mut u64) -> Option<bool> {
        let Some((first, others)) = targets.split_first() else {
            return Some(false);
        };
        if *budget == 0 {
            return None;
        }
        *budget -= 1;
        if first.owner != self.owner {
            // Nonempty cells of different owners are disjoint.
            return self.covered(others, budget);
        }
        for piece in self.minus(first) {
            if !piece.covered(others, budget)? {
                return Some(false);
            }
        }
        Some(true)
    }
}

impl Cell {
    fn axis_bounds(&self, axis: usize, extra: &Extra) -> (i128, Option<i128>) {
        let mut low = i128::from(self.lower[axis]);
        let mut high = self.upper[axis].map(i128::from);
        if let Some((raised, value)) = extra.raise
            && raised == axis
        {
            low = low.max(i128::from(value));
        }
        if let Some((cut, value)) = extra.cut
            && cut == axis
        {
            high = cap(high, Some(i128::from(value)));
        }
        (low, high)
    }

    fn nonempty_with(&self, extra: &Extra) -> bool {
        let (mut a_low, mut a_high, mut r_low, mut r_high) =
            (0i128, Some(0i128), 0i128, Some(0i128));
        for axis in 0..self.owner.len() {
            let (low, high) = self.axis_bounds(axis, extra);
            if high.is_some_and(|high| high < low) {
                return false;
            }
            let (sum_low, sum_high) = if self.owner[axis] {
                (&mut a_low, &mut a_high)
            } else {
                (&mut r_low, &mut r_high)
            };
            *sum_low += low + i128::from(self.owner[axis]);
            *sum_high = sum_high
                .zip(high)
                .map(|(s, h)| s + h + i128::from(self.owner[axis]));
        }
        let a_low = a_low.max(extra.a_low.unwrap_or(i128::MIN));
        let a_high = cap(a_high, self.powers.max_positive_power.map(i128::from));
        let r_low = r_low.max(extra.r_low.unwrap_or(i128::MIN));
        let r_high = cap(r_high, self.rank.map(i128::from));
        if a_high.is_some_and(|high| high < a_low) || r_high.is_some_and(|high| high < r_low) {
            return false;
        }
        // D over the (A, R) rectangle: every integer in [a_low - r_high, a_high - r_low].
        let d_low = floor(
            floor(
                r_high.map(|r| a_low - r),
                self.powers.min_power_difference.map(i128::from),
            ),
            extra.d_low,
        );
        let d_high = cap(
            cap(
                a_high.map(|a| a - r_low),
                self.powers.max_power_difference.map(i128::from),
            ),
            extra.d_high,
        );
        !matches!((d_low, d_high), (Some(low), Some(high)) if low > high)
    }

    pub fn is_empty(&self) -> bool {
        !self.nonempty_with(&Extra::default())
    }

    /// Exact integer-set inclusion `inner <= self`. An empty inner is
    /// contained in every cell; nonempty cells of different owners are
    /// disjoint (their A/R axis roles differ), so they never contain each other.
    pub fn contains(&self, inner: &Cell) -> bool {
        if inner.is_empty() {
            return true;
        }
        if self.owner != inner.owner {
            return false;
        }
        // Each outer constraint is tested only when the inner cell's own
        // bound does not already imply it (then its violation set is empty).
        for axis in 0..self.owner.len() {
            // An explicit upper at u64::MAX excludes no u64 coordinate.
            if let Some(high) = self.upper[axis]
                && high < u64::MAX
                && inner.upper[axis].is_none_or(|own| own > high)
                && inner.nonempty_with(&Extra {
                    raise: Some((axis, high + 1)),
                    ..Extra::default()
                })
            {
                return false;
            }
            let low = self.lower[axis];
            if low > inner.lower[axis]
                && inner.nonempty_with(&Extra {
                    cut: Some((axis, low - 1)),
                    ..Extra::default()
                })
            {
                return false;
            }
        }
        let violated = |extra: Extra| inner.nonempty_with(&extra);
        let implied = |outer: Option<i128>, own: Option<i128>, upper: bool| match (outer, own) {
            (None, _) => true,
            (Some(_), None) => false,
            (Some(o), Some(i)) => {
                if upper {
                    i <= o
                } else {
                    i >= o
                }
            }
        };
        let wide = |v: Option<u64>| v.map(i128::from);
        let signed = |v: Option<i64>| v.map(i128::from);
        let (outer, own) = (&self.powers, &inner.powers);
        !((!implied(self.rank.map(i128::from), inner.rank.map(i128::from), true)
            && violated(Extra {
                r_low: self.rank.map(|r| i128::from(r) + 1),
                ..Extra::default()
            }))
            || (!implied(
                wide(outer.max_positive_power),
                wide(own.max_positive_power),
                true,
            ) && violated(Extra {
                a_low: wide(outer.max_positive_power).map(|a| a + 1),
                ..Extra::default()
            }))
            || (!implied(
                signed(outer.min_power_difference),
                signed(own.min_power_difference),
                false,
            ) && violated(Extra {
                d_high: signed(outer.min_power_difference).map(|d| d - 1),
                ..Extra::default()
            }))
            || (!implied(
                signed(outer.max_power_difference),
                signed(own.max_power_difference),
                true,
            ) && violated(Extra {
                d_low: signed(outer.max_power_difference).map(|d| d + 1),
                ..Extra::default()
            })))
    }

    /// Direct evaluation of the defining inequalities at one point.
    pub fn member(&self, point: &[u64]) -> bool {
        let (mut a, mut r) = (0i128, 0i128);
        for (axis, &x) in point.iter().enumerate() {
            if x < self.lower[axis] || self.upper[axis].is_some_and(|high| x > high) {
                return false;
            }
            if self.owner[axis] {
                a += i128::from(x) + 1;
            } else {
                r += i128::from(x);
            }
        }
        self.rank.is_none_or(|rank| r <= i128::from(rank))
            && self
                .powers
                .max_positive_power
                .is_none_or(|m| a <= i128::from(m))
            && self
                .powers
                .min_power_difference
                .is_none_or(|d| a - r >= i128::from(d))
            && self
                .powers
                .max_power_difference
                .is_none_or(|d| a - r <= i128::from(d))
    }

    /// Calls `visit` for every lattice point if the cell's A/R-capped
    /// rectangle holds at most `limit` points; returns the number of points
    /// visited, or None (nothing visited) when the rectangle is larger or
    /// unbounded. `visit` returning false stops early.
    pub fn for_each_point(&self, limit: u64, mut visit: impl FnMut(&[u64]) -> bool) -> Option<u64> {
        let n = self.owner.len();
        let mut caps: Vec<Option<u64>> = self.upper.clone();
        for (active, total) in [
            (
                true,
                self.powers
                    .max_positive_power
                    .map(|a| i128::from(a) - self.owner.iter().filter(|&&b| b).count() as i128),
            ),
            (false, self.rank.map(i128::from)),
        ] {
            let Some(total) = total else { continue };
            let floor: i128 = (0..n)
                .filter(|&axis| self.owner[axis] == active)
                .map(|axis| i128::from(self.lower[axis]))
                .sum();
            if total < floor {
                return Some(0);
            }
            for axis in (0..n).filter(|&axis| self.owner[axis] == active) {
                // Every other axis of the group sits at least at its lower.
                let bound = u64::try_from(total - floor + i128::from(self.lower[axis])).ok();
                caps[axis] = match (caps[axis], bound) {
                    (Some(c), Some(b)) => Some(c.min(b)),
                    (c, b) => c.or(b),
                };
            }
        }
        let mut size = 1u128;
        for axis in 0..n {
            let high = caps[axis]?;
            if high < self.lower[axis] {
                return Some(0);
            }
            size = size.saturating_mul(u128::from(high - self.lower[axis]) + 1);
            if size > u128::from(limit) {
                return None;
            }
        }
        let mut point = self.lower.clone();
        let mut visited = 0u64;
        loop {
            if self.member(&point) {
                visited += 1;
                if !visit(&point) {
                    return Some(visited);
                }
            }
            let mut axis = 0;
            loop {
                if axis == n {
                    return Some(visited);
                }
                if point[axis] < caps[axis].expect("finite rectangle") {
                    point[axis] += 1;
                    break;
                }
                point[axis] = self.lower[axis];
                axis += 1;
            }
        }
    }

    /// Exact `self <= t_1 u ... u t_k` (see the module note); None when more
    /// than `max_regions` regions would be examined (undecided, never wrong).
    pub fn covered_by_union(&self, targets: &[&Cell], max_regions: u64) -> Option<bool> {
        let region = Region::of(self);
        if !region.nonempty() {
            return Some(true);
        }
        let mut budget = max_regions;
        region.covered(targets, &mut budget)
    }

    /// Whether the two cells share a lattice point (same owner, nonempty
    /// intersection; the intersection of two cells is a cell).
    pub fn meets(&self, other: &Cell) -> bool {
        fn min<T: Ord + Copy>(a: Option<T>, b: Option<T>) -> Option<T> {
            match (a, b) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            }
        }
        if self.owner != other.owner {
            return false;
        }
        let mut both = self.clone();
        for axis in 0..self.owner.len() {
            both.lower[axis] = self.lower[axis].max(other.lower[axis]);
            both.upper[axis] = min(self.upper[axis], other.upper[axis]);
        }
        both.rank = min(self.rank, other.rank);
        let (mine, theirs) = (&self.powers, &other.powers);
        both.powers.max_positive_power = min(mine.max_positive_power, theirs.max_positive_power);
        both.powers.max_power_difference =
            min(mine.max_power_difference, theirs.max_power_difference);
        both.powers.min_power_difference =
            match (mine.min_power_difference, theirs.min_power_difference) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (a, b) => a.or(b),
            };
        !both.is_empty()
    }

    /// Brute-force union cover over this cell's points, when enumerable.
    pub fn brute_force_covered_by_union(&self, targets: &[&Cell], limit: u64) -> Option<bool> {
        let mut covered = true;
        self.for_each_point(limit, |point| {
            covered = targets
                .iter()
                .any(|t| t.owner == self.owner && t.member(point));
            covered
        })?;
        Some(covered)
    }

    /// Brute-force inclusion over the inner cell's points, when enumerable.
    pub fn brute_force_contains(&self, inner: &Cell, limit: u64) -> Option<(bool, u64)> {
        let mut contained = true;
        let owners_differ = self.owner != inner.owner;
        let visited = inner.for_each_point(limit, |point| {
            contained = !owners_differ && self.member(point);
            contained
        })?;
        Some((contained, visited))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
        fn maybe(&mut self, n: u64) -> Option<u64> {
            (self.below(4) != 0).then(|| self.below(n))
        }
    }

    fn random_cell(rng: &mut Rng, owner: &[bool]) -> Cell {
        let n = owner.len();
        let mut lower = Vec::new();
        let mut upper = Vec::new();
        for _ in 0..n {
            let low = rng.below(3);
            lower.push(low);
            upper.push(rng.maybe(4).map(|extra| low + extra));
        }
        let d = |rng: &mut Rng| rng.maybe(9).map(|v| v as i64 - 3);
        let (mut d_min, mut d_max) = (d(rng), d(rng));
        if let (Some(a), Some(b)) = (d_min, d_max)
            && a > b
        {
            (d_min, d_max) = (Some(b), Some(a));
        }
        Cell {
            owner: owner.to_vec(),
            lower,
            upper,
            rank: rng.maybe(6).map(|r| r as u32),
            powers: DomainPowerBounds {
                max_positive_power: rng.maybe(9),
                min_power_difference: d_min,
                max_power_difference: d_max,
            },
        }
    }

    /// Ground truth over a window large enough to hold every point that
    /// matters: all bounds in the generator stay below 12.
    fn window_points(owner_len: usize) -> Vec<Vec<u64>> {
        let mut points = vec![vec![]];
        for _ in 0..owner_len {
            points = points
                .into_iter()
                .flat_map(|p: Vec<u64>| {
                    (0..12u64).map(move |x| {
                        let mut q = p.clone();
                        q.push(x);
                        q
                    })
                })
                .collect();
        }
        points
    }

    #[test]
    fn exact_predicates_match_point_enumeration() {
        let mut rng = Rng(7);
        for n in 1..=3usize {
            let window = window_points(n);
            for _ in 0..1500 {
                let owner: Vec<bool> = (0..n).map(|_| rng.below(2) == 1).collect();
                let outer = random_cell(&mut rng, &owner);
                let inner_owner = if rng.below(8) == 0 {
                    (0..n).map(|_| rng.below(2) == 1).collect()
                } else {
                    owner.clone()
                };
                let inner = random_cell(&mut rng, &inner_owner);
                // Points beyond the window can matter only for unbounded
                // cells; restrict the comparison to cells bounded inside it.
                let bounded = |c: &Cell| {
                    c.for_each_point(u64::MAX, |_| true).is_some()
                        && c.upper.iter().all(|u| u.is_some_and(|u| u < 12))
                };
                let inside: Vec<&Vec<u64>> = window.iter().filter(|p| inner.member(p)).collect();
                assert_eq!(inner.is_empty(), inside.is_empty(), "{inner:?}");
                if bounded(&inner) {
                    let truth = inside
                        .iter()
                        .all(|p| outer.member(p) && outer.owner == inner.owner);
                    assert_eq!(outer.contains(&inner), truth, "{outer:?} {inner:?}");
                    let (brute, count) = outer.brute_force_contains(&inner, 1 << 20).unwrap();
                    assert_eq!(brute, truth);
                    if truth {
                        assert_eq!(count as usize, inside.len());
                    }
                }
            }
        }
    }

    #[test]
    fn exact_inclusion_agrees_with_the_walker_summary_on_same_owner_cells() {
        // Cross-validation only: the walker's `DomainPowerSummary::contains`
        // claims exact inclusion for this vocabulary. Disagreement would
        // indicate a defect in one of two independent implementations.
        use rustred::solver::DomainPowerSummary;
        let mut rng = Rng(11);
        for _ in 0..20000 {
            let owner = [rng.below(2) == 1, rng.below(2) == 1, rng.below(2) == 1];
            let (outer, inner) = (random_cell(&mut rng, &owner), random_cell(&mut rng, &owner));
            let summary = |c: &Cell| {
                DomainPowerSummary::<3>::try_new(owner, &c.lower, &c.upper, c.rank, c.powers)
                    .unwrap()
            };
            assert_eq!(
                outer.contains(&inner),
                summary(&outer).contains(&summary(&inner)),
                "{outer:?} {inner:?}"
            );
        }
    }

    #[test]
    fn unbounded_cells_and_empty_cells() {
        let open = Cell {
            owner: vec![true, false],
            lower: vec![0, 0],
            upper: vec![None, None],
            rank: None,
            powers: DomainPowerBounds::default(),
        };
        let mut capped = open.clone();
        capped.rank = Some(3);
        assert!(open.contains(&capped));
        assert!(!capped.contains(&open));
        assert!(open.for_each_point(1000, |_| true).is_none());
        let mut empty = capped.clone();
        empty.powers.min_power_difference = Some(5);
        empty.powers.max_power_difference = Some(4);
        assert!(empty.is_empty());
        let mut other_owner = empty.clone();
        other_owner.owner = vec![false, true];
        assert!(capped.contains(&other_owner));
        // D >= 7 with A <= 3 on one owner axis is empty (A = x + 1 <= 3 -> D <= 3).
        let mut high_d = capped.clone();
        high_d.powers = DomainPowerBounds {
            max_positive_power: Some(3),
            min_power_difference: Some(7),
            max_power_difference: None,
        };
        assert!(high_d.is_empty());
        assert_eq!(high_d.for_each_point(1000, |_| true), Some(0));
    }

    #[test]
    fn union_cover_matches_point_enumeration() {
        let mut rng = Rng(23);
        let (mut covered, mut union_only, mut uncovered) = (0, 0, 0);
        for n in 1..=3usize {
            let window = window_points(n);
            for _ in 0..5000 {
                let owner: Vec<bool> = (0..n).map(|_| rng.below(2) == 1).collect();
                let q = random_cell(&mut rng, &owner);
                if !(q.for_each_point(u64::MAX, |_| true).is_some()
                    && q.upper.iter().all(|u| u.is_some_and(|u| u < 12)))
                {
                    continue;
                }
                // Targets: random cells, plus pieces of q itself so that
                // covers needing several targets occur often.
                let mut targets = Vec::new();
                for _ in 0..1 + rng.below(4) {
                    let target = match rng.below(3) {
                        0 => random_cell(&mut rng, &owner),
                        1 => {
                            let mut piece = q.clone();
                            let axis = rng.below(n as u64) as usize;
                            let cut = q.lower[axis] + rng.below(3);
                            if rng.below(2) == 0 {
                                piece.upper[axis] = Some(cut);
                            } else {
                                piece.lower[axis] = cut + 1;
                            }
                            piece
                        }
                        _ => {
                            let mut piece = q.clone();
                            let cut = rng.below(9) as i64 - 3;
                            if rng.below(2) == 0 {
                                piece.powers.max_power_difference = Some(cut);
                            } else {
                                piece.powers.min_power_difference = Some(cut + 1);
                            }
                            if rng.below(4) == 0 {
                                piece.owner = (0..n).map(|_| rng.below(2) == 1).collect();
                            }
                            piece
                        }
                    };
                    targets.push(target);
                }
                if rng.below(3) != 0 {
                    // A complementary pair split on an axis or in D: together
                    // they cover q (or leave a one-layer gap), and in general
                    // neither does alone.
                    let gap = rng.below(3) == 0;
                    let (mut low, mut high) = (q.clone(), q.clone());
                    // Split strictly inside q's range on an axis where q
                    // spans several values, so both halves are proper.
                    let splittable: Vec<usize> = (0..n)
                        .filter(|&a| q.upper[a].is_some_and(|u| u > q.lower[a]))
                        .collect();
                    if !splittable.is_empty() && rng.below(3) != 0 {
                        let axis = splittable[rng.below(splittable.len() as u64) as usize];
                        let span = q.upper[axis].expect("splittable") - q.lower[axis];
                        let cut = q.lower[axis] + rng.below(span);
                        low.upper[axis] = Some(cut);
                        high.lower[axis] = cut + 1 + u64::from(gap);
                    } else {
                        let cut = rng.below(9) as i64 - 3;
                        low.powers.max_power_difference = Some(cut);
                        high.powers.min_power_difference = Some(cut + 1 + i64::from(gap));
                    }
                    targets.push(low);
                    targets.push(high);
                }
                let refs: Vec<&Cell> = targets.iter().collect();
                let truth = window
                    .iter()
                    .filter(|p| q.member(p))
                    .all(|p| targets.iter().any(|t| t.owner == q.owner && t.member(p)));
                assert_eq!(
                    q.covered_by_union(&refs, 1 << 20),
                    Some(truth),
                    "{q:?} {targets:?}"
                );
                assert_eq!(q.brute_force_covered_by_union(&refs, 1 << 20), Some(truth));
                if truth {
                    covered += 1;
                    union_only += usize::from(!targets.iter().any(|t| t.contains(&q)));
                } else {
                    uncovered += 1;
                }
                // A single target reduces to exact inclusion.
                assert_eq!(
                    q.covered_by_union(&refs[..1], 1 << 20),
                    Some(targets[0].contains(&q))
                );
            }
        }
        // The draw must exercise genuine multi-target covers and misses.
        assert!(
            union_only > 200 && uncovered > 200,
            "{covered} {union_only} {uncovered}"
        );
    }

    /// Four to six axes with at least two axes in each owner group (the
    /// A and R sums each span several coordinates, as in real 10- and
    /// 15-axis cells), covers made of 2-4 pieces of Q (splits along axes and
    /// D, optionally with a one-layer gap) plus random distractors. Ground
    /// truth: every point of the window (which bounds Q) evaluated directly.
    #[test]
    fn union_cover_matches_point_enumeration_on_four_to_six_axes() {
        let mut rng = Rng(41);
        let (mut union_only, mut needs_three, mut uncovered, mut meets_checked) = (0, 0, 0, 0);
        for n in 4..=6usize {
            let window: Vec<Vec<u64>> = {
                let mut points = vec![vec![]];
                for _ in 0..n {
                    points = points
                        .into_iter()
                        .flat_map(|p: Vec<u64>| {
                            (0..5u64).map(move |x| {
                                let mut q = p.clone();
                                q.push(x);
                                q
                            })
                        })
                        .collect();
                }
                points
            };
            for _ in 0..1500 {
                // At least two owner and two non-owner axes.
                let mut owner: Vec<bool> = (0..n).map(|axis| axis % 2 == 0).collect();
                for axis in 4..n {
                    owner[axis] = rng.below(2) == 1;
                }
                let mut q = random_cell(&mut rng, &owner);
                let (mut own_low, mut other_low) = (0u64, 0u64);
                for axis in 0..n {
                    // Bounded inside the window: every point of q is in it.
                    q.lower[axis] = rng.below(2);
                    q.upper[axis] = Some(q.lower[axis] + rng.below(3));
                    if owner[axis] {
                        own_low += q.lower[axis] + 1;
                    } else {
                        other_low += q.lower[axis];
                    }
                }
                // R and A caps that leave q nonempty most of the time.
                q.rank = rng.maybe(5).map(|r| (other_low + r) as u32);
                q.powers.max_positive_power = rng.maybe(5).map(|a| own_low + a);
                if q.is_empty() {
                    continue;
                }
                let splits = 1 + rng.below(3) as usize;
                let mut pieces = vec![q.clone()];
                let gap = rng.below(2) == 0;
                for split in 0..splits {
                    // Split the last piece strictly inside an axis range, or
                    // in D; the last split of a gap draw leaves one layer out.
                    let piece = pieces.pop().expect("piece");
                    let (mut low, mut high) = (piece.clone(), piece.clone());
                    let last = gap && split + 1 == splits;
                    let splittable: Vec<usize> = (0..n)
                        .filter(|&a| piece.upper[a].is_some_and(|u| u > piece.lower[a]))
                        .collect();
                    if !splittable.is_empty() && rng.below(4) != 0 {
                        let axis = splittable[rng.below(splittable.len() as u64) as usize];
                        let span = piece.upper[axis].expect("bounded") - piece.lower[axis];
                        let cut = piece.lower[axis] + rng.below(span);
                        low.upper[axis] = Some(cut);
                        high.lower[axis] = cut + 1 + u64::from(last);
                    } else {
                        let cut = rng.below(11) as i64 - 5;
                        low.powers.max_power_difference = Some(
                            piece
                                .powers
                                .max_power_difference
                                .map_or(cut, |d| d.min(cut)),
                        );
                        high.powers.min_power_difference = Some(
                            piece
                                .powers
                                .min_power_difference
                                .map_or(cut + 1 + i64::from(last), |d| {
                                    d.max(cut + 1 + i64::from(last))
                                }),
                        );
                    }
                    pieces.push(low);
                    pieces.push(high);
                }
                let mut targets = pieces;
                for _ in 0..rng.below(3) {
                    let mut distractor = random_cell(&mut rng, &owner);
                    if rng.below(5) == 0 {
                        distractor.owner = (0..n).map(|_| rng.below(2) == 1).collect();
                    }
                    targets.push(distractor);
                }
                // Shuffle so that the pieces are not always tried first.
                for index in (1..targets.len()).rev() {
                    targets.swap(index, rng.below(index as u64 + 1) as usize);
                }
                let refs: Vec<&Cell> = targets.iter().collect();
                let inside: Vec<&Vec<u64>> = window.iter().filter(|p| q.member(p)).collect();
                let truth = inside
                    .iter()
                    .all(|p| targets.iter().any(|t| t.owner == q.owner && t.member(p)));
                assert_eq!(
                    q.covered_by_union(&refs, 1 << 22),
                    Some(truth),
                    "{q:?} {targets:?}"
                );
                assert_eq!(q.brute_force_covered_by_union(&refs, 1 << 20), Some(truth));
                for target in &targets {
                    let shared = inside
                        .iter()
                        .any(|p| target.owner == q.owner && target.member(p));
                    assert_eq!(q.meets(target), shared, "{q:?} {target:?}");
                    meets_checked += 1;
                }
                if !truth {
                    uncovered += 1;
                } else if !targets.iter().any(|t| t.contains(&q)) {
                    union_only += 1;
                    let pair = (0..refs.len()).any(|i| {
                        (i + 1..refs.len())
                            .any(|j| q.covered_by_union(&[refs[i], refs[j]], 1 << 22) == Some(true))
                    });
                    needs_three += usize::from(!pair);
                }
            }
        }
        // Genuine multi-target covers (some needing three or more targets)
        // and misses must both be frequent.
        // Python port of this generator (same RNG): 3347 draws, 1357
        // union-only covers, 508 needing >= 3 targets, 766 uncovered.
        assert!(
            union_only > 800 && needs_three > 250 && uncovered > 400,
            "union-only {union_only}, needing >= 3 targets {needs_three}, uncovered {uncovered}"
        );
        assert!(meets_checked > 10_000);
    }

    #[test]
    fn union_cover_of_a_d_cut_residual_and_its_anchor_and_the_budget() {
        // Q = one owner axis and one other axis, unbounded; D = A - R.
        let q = Cell {
            owner: vec![true, false],
            lower: vec![0, 0],
            upper: vec![None, None],
            rank: Some(4),
            powers: DomainPowerBounds {
                max_positive_power: Some(9),
                min_power_difference: None,
                max_power_difference: None,
            },
        };
        let mut residual = q.clone();
        residual.powers.max_power_difference = Some(2);
        let mut anchor = q.clone();
        anchor.powers.min_power_difference = Some(3);
        assert_eq!(
            q.covered_by_union(&[&residual, &anchor], 1 << 10),
            Some(true)
        );
        assert!(!anchor.contains(&q) && !residual.contains(&q));
        // Shrinking the residual by one D value leaves the D = 2 layer open.
        let mut short = residual.clone();
        short.powers.max_power_difference = Some(1);
        assert_eq!(q.covered_by_union(&[&short, &anchor], 1 << 10), Some(false));
        // An anchor of another owner covers nothing.
        let mut foreign = anchor.clone();
        foreign.owner = vec![false, true];
        assert_eq!(
            q.covered_by_union(&[&residual, &foreign], 1 << 10),
            Some(false)
        );
        // The region budget makes the answer undecided, never wrong.
        assert_eq!(q.covered_by_union(&[&residual, &anchor], 1), None);
        assert_eq!(q.covered_by_union(&[], 1 << 10), Some(false));
        let mut empty = q.clone();
        empty.powers.min_power_difference = Some(20);
        assert_eq!(empty.covered_by_union(&[], 1 << 10), Some(true));
    }
}
