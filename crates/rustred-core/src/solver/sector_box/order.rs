//! Column-order presets for the global row space.
//!
//! The pivot of a row is its leftmost, i.e. hardest, column, so masters land
//! at the easy end. Every preset forces the caller's preferred masters to
//! the easy end, keeping their relative preset order among themselves.

use super::super::discovery::ColumnComparator;
use super::super::{Integral, IntegralOrder, SolverError};
use std::cmp::Ordering;

/// Column-order presets (hardest first).
#[derive(Clone, Debug)]
pub enum ColumnOrder {
    /// RustRed's [`IntegralOrder`] (as used by `NumericOrder`): more
    /// denominators, then inverse sector lexicographic order, then total
    /// absolute power, then numerator total, then coordinate ties.
    Numeric,
    /// `(lines, dots, numerator rank, lexicographic powers)`, larger is
    /// harder: the key that reproduced Kira's per-unique-sector counts in
    /// the LiteRed-route prototype.
    LinesDotsRank,
    /// `(lines, numerator rank, dots, lexicographic powers)`, larger is
    /// harder: numerators are harder than any number of dots in the same
    /// sector, so master bases stay numerator-free where possible and do
    /// not change when a box gains numerator seeds (Kira-like).
    LinesRankDots,
    /// A caller-supplied programmable order.
    Compiled(rustred_order::CompiledOrder),
}

/// Slot priority of the lexicographic tie-break of the `Lines*` presets,
/// the analogue of Kira's internal propagator permutation (scheme 1 of
/// arXiv:2505.20197 §4.1, following the Kira 1.2 guidelines: auxiliary
/// propagators last). Pivots and masters per class do not depend on it in
/// exact arithmetic beyond the choice of representatives; fill-in does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SlotPriority {
    /// Slots in their natural order `0, 1, ...`.
    #[default]
    Natural,
    /// The active lines of the given top sector (Kira id `sum 2^i`) first,
    /// in ascending slot order, then every other slot (the ISPs of that
    /// sector) in ascending order.
    TopSectorFirst(u64),
    /// An explicit permutation: `slots[k]` is compared `k`-th.
    Permutation(Vec<u8>),
}

impl SlotPriority {
    /// The comparison order of the slots, or `None` for the natural order.
    pub(crate) fn resolve(&self, arity: usize) -> Result<Option<Vec<usize>>, SolverError> {
        let slots: Vec<usize> = match self {
            Self::Natural => return Ok(None),
            Self::TopSectorFirst(top) => {
                if arity < 64 && top >> arity != 0 {
                    return Err(SolverError::InvalidInput(format!(
                        "slot priority top sector {top} has slots beyond arity {arity}"
                    )));
                }
                let active = (0..arity).filter(|&slot| top >> slot & 1 == 1);
                let inactive = (0..arity).filter(|&slot| top >> slot & 1 == 0);
                active.chain(inactive).collect()
            }
            Self::Permutation(slots) => slots.iter().map(|&slot| usize::from(slot)).collect(),
        };
        let mut seen = vec![false; arity];
        for &slot in &slots {
            if slot >= arity || std::mem::replace(&mut seen[slot], true) {
                return Err(SolverError::InvalidInput(format!(
                    "slot priority {slots:?} is not a permutation of 0..{arity}"
                )));
            }
        }
        if slots.len() != arity {
            return Err(SolverError::InvalidInput(format!(
                "slot priority {slots:?} is not a permutation of 0..{arity}"
            )));
        }
        Ok((!slots.iter().copied().eq(0..arity)).then_some(slots))
    }
}

impl ColumnOrder {
    pub(crate) fn tag(&self) -> &'static str {
        match self {
            Self::Numeric => "numeric",
            Self::LinesDotsRank => "lines-dots-rank",
            Self::LinesRankDots => "lines-rank-dots",
            Self::Compiled(_) => "compiled",
        }
    }
}

#[derive(Clone, Debug)]
enum Base<const N: usize> {
    Integral(IntegralOrder<N>),
    LinesDotsRank,
    LinesRankDots,
}

/// A resolved column order for one session: preset plus preferred masters.
#[derive(Clone, Debug)]
pub(crate) struct BoxOrder<const N: usize> {
    base: Base<N>,
    /// Comparison order of the slots in the lexicographic tie-break; the
    /// natural order when `None`.
    slots: Option<Vec<usize>>,
    /// Sorted by the derived key order, for allocation-free lookups.
    preferred: Vec<Integral<N>>,
    fingerprint: Vec<u8>,
}

impl<const N: usize> BoxOrder<N> {
    #[cfg(test)]
    pub(crate) fn new(
        preset: &ColumnOrder,
        arity: usize,
        preferred: &[Integral<N>],
    ) -> Result<Self, SolverError> {
        Self::with_slot_priority(preset, arity, preferred, &SlotPriority::Natural)
    }

    pub(crate) fn with_slot_priority(
        preset: &ColumnOrder,
        arity: usize,
        preferred: &[Integral<N>],
        priority: &SlotPriority,
    ) -> Result<Self, SolverError> {
        let slots = priority.resolve(arity)?;
        if slots.is_some() && matches!(preset, ColumnOrder::Numeric | ColumnOrder::Compiled(_)) {
            return Err(SolverError::InvalidInput(format!(
                "slot priority applies to the lines-* column orders, not to {}",
                preset.tag()
            )));
        }
        let base = match preset {
            ColumnOrder::Numeric => Base::Integral(
                IntegralOrder::new([false; N], [false; N]).with_physical_arity(arity)?,
            ),
            ColumnOrder::LinesDotsRank => Base::LinesDotsRank,
            ColumnOrder::LinesRankDots => Base::LinesRankDots,
            ColumnOrder::Compiled(program) => {
                if program.arity() != arity {
                    return Err(SolverError::InvalidInput(format!(
                        "compiled column order has arity {}, the family {arity}",
                        program.arity()
                    )));
                }
                Base::Integral(
                    IntegralOrder::new([false; N], [false; N]).with_program(program.clone())?,
                )
            }
        };
        let mut fingerprint = preset.tag().as_bytes().to_vec();
        if let ColumnOrder::Compiled(program) = preset {
            fingerprint.extend_from_slice(program.canonical_bytes());
        }
        if let Some(slots) = &slots {
            fingerprint.push(0xfe);
            fingerprint.extend(slots.iter().map(|&slot| slot as u8));
        }
        let mut sorted = preferred.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        for integral in &sorted {
            fingerprint.push(0xff);
            fingerprint.extend(integral.powers().iter().map(|power| power.value() as u8));
        }
        Ok(Self {
            base,
            slots,
            preferred: sorted,
            fingerprint,
        })
    }

    /// Canonical bytes binding the preset and the preferred set.
    pub(crate) fn fingerprint(&self) -> &[u8] {
        &self.fingerprint
    }

    fn compare_base(&self, left: &Integral<N>, right: &Integral<N>) -> Ordering {
        match &self.base {
            Base::Integral(order) => order.compare(left, right),
            Base::LinesDotsRank => lines_dots_rank(left, self.slots.as_deref())
                .cmp(&lines_dots_rank(right, self.slots.as_deref()))
                .reverse(),
            Base::LinesRankDots => {
                let (left_lines, left_dots, left_rank, left_powers) =
                    lines_dots_rank(left, self.slots.as_deref());
                let (right_lines, right_dots, right_rank, right_powers) =
                    lines_dots_rank(right, self.slots.as_deref());
                (left_lines, left_rank, left_dots, left_powers)
                    .cmp(&(right_lines, right_rank, right_dots, right_powers))
                    .reverse()
            }
        }
    }
}

/// `(lines, dots, rank, powers)`; a larger key is harder. `powers` lists
/// the slots in priority order (natural order without a priority).
fn lines_dots_rank<const N: usize>(
    integral: &Integral<N>,
    slots: Option<&[usize]>,
) -> (u32, i32, i32, [i16; N]) {
    let mut lines = 0;
    let mut dots = 0;
    let mut rank = 0;
    let mut powers = [0; N];
    let values = integral.powers();
    if let Some(slots) = slots {
        for (position, &slot) in slots.iter().enumerate() {
            powers[position] = values[slot].value();
        }
    }
    for (slot, power) in values.iter().enumerate() {
        let value = power.value();
        if slots.is_none() {
            powers[slot] = value;
        }
        if value > 0 {
            lines += 1;
            dots += i32::from(value) - 1;
        } else {
            rank -= i32::from(value);
        }
    }
    (lines, dots, rank, powers)
}

impl<const N: usize> ColumnComparator<N> for BoxOrder<N> {
    fn compare(&self, left: &Integral<N>, right: &Integral<N>) -> Ordering {
        if !self.preferred.is_empty() {
            match (
                self.preferred.binary_search(left).is_ok(),
                self.preferred.binary_search(right).is_ok(),
            ) {
                (true, false) => return Ordering::Greater,
                (false, true) => return Ordering::Less,
                _ => {}
            }
        }
        self.compare_base(left, right)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn integral(values: [i16; 4]) -> Integral<4> {
        Integral::numeric(values).unwrap()
    }

    #[test]
    fn numeric_preset_is_the_solver_integral_order() {
        let order = BoxOrder::<4>::new(&ColumnOrder::Numeric, 4, &[]).unwrap();
        let solver = IntegralOrder::new([false; 4], [false; 4]);
        let keys = [
            [1, 1, 1, 0],
            [2, 1, 1, 0],
            [1, 1, 0, 0],
            [1, 1, 1, -1],
            [1, 0, 1, -2],
            [3, 1, 0, 0],
        ];
        for left in keys {
            for right in keys {
                assert_eq!(
                    order.compare(&integral(left), &integral(right)),
                    solver.compare(&integral(left), &integral(right))
                );
            }
        }
    }

    #[test]
    fn lines_dots_rank_and_preferred_masters() {
        let order = BoxOrder::<4>::new(&ColumnOrder::LinesDotsRank, 4, &[]).unwrap();
        // More lines first, then dots, then numerator rank.
        assert_eq!(
            order.compare(&integral([1, 1, 1, 0]), &integral([5, 1, 0, 0])),
            Ordering::Less
        );
        assert_eq!(
            order.compare(&integral([2, 1, 0, 0]), &integral([1, 1, 0, -3])),
            Ordering::Less
        );
        assert_eq!(
            order.compare(&integral([1, 1, 0, -1]), &integral([1, 1, 0, 0])),
            Ordering::Less
        );
        let order = BoxOrder::<4>::new(&ColumnOrder::LinesRankDots, 4, &[]).unwrap();
        // Numerators are harder than dots within a sector.
        assert_eq!(
            order.compare(&integral([1, 1, 0, -1]), &integral([3, 1, 0, 0])),
            Ordering::Less
        );
        assert_eq!(
            order.compare(&integral([1, 1, 1, 0]), &integral([5, 1, 0, -2])),
            Ordering::Less
        );
        let preferred = [integral([2, 1, 1, 0])];
        let order = BoxOrder::<4>::new(&ColumnOrder::Numeric, 4, &preferred).unwrap();
        // A preferred top-sector integral is easier than every other column.
        assert_eq!(
            order.compare(&integral([2, 1, 1, 0]), &integral([1, 0, 0, 0])),
            Ordering::Greater
        );
        assert_eq!(
            order.compare(&integral([1, 0, 0, 0]), &integral([2, 1, 1, 0])),
            Ordering::Less
        );
    }

    #[test]
    fn slot_priority_reorders_ties_only() {
        // Top sector {1, 2}: slots 1, 2 are compared first, then 0, 3.
        let natural = BoxOrder::<4>::new(&ColumnOrder::LinesRankDots, 4, &[]).unwrap();
        let top_first = BoxOrder::<4>::with_slot_priority(
            &ColumnOrder::LinesRankDots,
            4,
            &[],
            &SlotPriority::TopSectorFirst(0b0110),
        )
        .unwrap();
        // Same lines, rank and dots: a pure tie-break.
        let left = integral([-1, 2, 1, 0]);
        let right = integral([0, 1, 2, -1]);
        // Natural: slot 0 decides (-1 < 0), so `left` is easier.
        assert_eq!(natural.compare(&left, &right), Ordering::Greater);
        // Top sector first: slot 1 decides (2 > 1), so `left` is harder.
        assert_eq!(top_first.compare(&left, &right), Ordering::Less);
        // Lines, rank and dots still decide first.
        assert_eq!(
            top_first.compare(&integral([0, 1, 1, -1]), &integral([0, 2, 1, 0])),
            Ordering::Less
        );
        assert_eq!(
            SlotPriority::TopSectorFirst(0b0110).resolve(4).unwrap(),
            Some(vec![1, 2, 0, 3])
        );
        assert_eq!(
            SlotPriority::TopSectorFirst(0b1111).resolve(4).unwrap(),
            None
        );
        assert!(
            SlotPriority::Permutation(vec![0, 0, 1, 2])
                .resolve(4)
                .is_err()
        );
        assert!(
            BoxOrder::<4>::with_slot_priority(
                &ColumnOrder::Numeric,
                4,
                &[],
                &SlotPriority::Permutation(vec![3, 2, 1, 0])
            )
            .is_err()
        );
        assert_ne!(natural.fingerprint(), top_first.fingerprint());
    }
}
