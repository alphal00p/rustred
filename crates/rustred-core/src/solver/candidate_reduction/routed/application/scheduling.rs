//! Operational visitation priorities, independent of persisted formula descent.
use std::cmp::Ordering;

use super::CandidateRoutedKeyDegrees;
use crate::algebra::Coefficient;
use crate::family::IntegralKey;

/// Experimental ordering of already admitted roots or already evaluated child
/// dependencies. These priorities do not choose rules or prove descent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CandidateRoutedWorkOrder {
    /// Existing ascending key order (phase, owner, key for dependencies).
    #[default]
    Canonical,
    NumeratorRankDescending,
    PositivePowerSumDescending,
    PropagatorCountDescending,
    CornerDistanceDescending,
    /// Numerator plus denominator monomial count of the incoming coefficient.
    /// This uses existing sparse sizes, without expanding or cloning algebra.
    CoefficientTermsDescending,
}

/// Independent experimental worklist priorities. The default preserves the
/// current root order and depth-first child order without sorting allocations.
/// Each root is still fully reduced before its contribution is collected;
/// this does not combine nonterminal descendants across different roots.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CandidateRoutedScheduling {
    pub roots: CandidateRoutedWorkOrder,
    pub dependencies: CandidateRoutedWorkOrder,
}

impl CandidateRoutedWorkOrder {
    /// Equal priorities must be resolved using the original full key order.
    pub(super) fn compare_priority(
        self,
        left: &IntegralKey,
        left_coefficient: &Coefficient,
        right: &IntegralKey,
        right_coefficient: &Coefficient,
    ) -> Ordering {
        self.priority(right, right_coefficient)
            .cmp(&self.priority(left, left_coefficient))
    }

    fn priority(self, key: &IntegralKey, coefficient: &Coefficient) -> [u128; 4] {
        if self == Self::Canonical {
            return [0; 4];
        }
        if self == Self::CoefficientTermsDescending {
            return [
                (coefficient.numerator.nterms() as u128)
                    .saturating_add(coefficient.denominator.nterms() as u128),
                0,
                0,
                0,
            ];
        }
        let degree = CandidateRoutedKeyDegrees::from_key(key);
        let rank = degree.numerator_rank;
        let dots = degree.dot_excess;
        let support = degree.support as u128;
        let powers = degree.positive_power_sum;
        match self {
            Self::NumeratorRankDescending => [rank, dots, support, powers],
            Self::PositivePowerSumDescending => [powers, rank, support, dots],
            Self::PropagatorCountDescending => [support, dots, rank, powers],
            Self::CornerDistanceDescending => [rank.saturating_add(dots), support, dots, rank],
            Self::Canonical | Self::CoefficientTermsDescending => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::CoefficientContext;

    #[test]
    fn priorities_distinguish_rank_dots_support_and_coefficient_size() {
        let c = CoefficientContext::try_new(["d"]).unwrap();
        let rank = IntegralKey::try_new([1, 0, -4]).unwrap();
        let dots = IntegralKey::try_new([4, 0, -1]).unwrap();
        let support = IntegralKey::try_new([1, 1, 1]).unwrap();
        let extreme = IntegralKey::try_new([i64::MAX, 0, i64::MIN]).unwrap();
        let one = c.one();
        for (order, first, second) in [
            (
                CandidateRoutedWorkOrder::NumeratorRankDescending,
                &rank,
                &dots,
            ),
            (
                CandidateRoutedWorkOrder::PositivePowerSumDescending,
                &dots,
                &rank,
            ),
            (
                CandidateRoutedWorkOrder::PropagatorCountDescending,
                &support,
                &dots,
            ),
            (
                CandidateRoutedWorkOrder::CornerDistanceDescending,
                &rank,
                &support,
            ),
            (
                CandidateRoutedWorkOrder::NumeratorRankDescending,
                &extreme,
                &rank,
            ),
        ] {
            assert_eq!(
                order.compare_priority(first, &one, second, &one),
                Ordering::Less
            );
            assert_eq!(
                order.compare_priority(second, &one, first, &one),
                Ordering::Greater
            );
        }
        let large = c.coefficient_fixture("(d+1)/(d-4)");
        assert_eq!(
            CandidateRoutedWorkOrder::CoefficientTermsDescending
                .compare_priority(&rank, &large, &dots, &one),
            Ordering::Less,
        );
        assert_eq!(
            CandidateRoutedWorkOrder::Canonical.compare_priority(&extreme, &large, &support, &one),
            Ordering::Equal
        );
    }
}
