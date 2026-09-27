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
use rustred::solver::DomainPowerBounds;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Cell {
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
}
