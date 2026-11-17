//! Ordered immutable batches share the existing exact rule evaluator.
use super::super::model::{CandidateReductionError, PreparedRule};
use super::model::{PreparedOwner, PreparedOwnerBatch};
use crate::algebra::Coefficient;
use crate::family::IntegralKey;
use std::collections::{BTreeMap, BTreeSet};

pub(in crate::solver::candidate_reduction) enum OwnerStep {
    Terminal,
    Applied(BTreeMap<IntegralKey, Coefficient>),
    Uncovered,
}

impl<const N: usize> PreparedOwnerBatch<N> {
    pub(super) fn new(
        rules: Vec<PreparedRule<N>>,
        terminals: BTreeSet<IntegralKey>,
        overlay: Option<super::feedback::OwnerOverlayMetadata<N>>,
    ) -> Self {
        let coalescing_bound = rules
            .iter()
            .map(|rule| rule.rhs.duplicate_shifts())
            .max()
            .unwrap_or(0);
        Self {
            whole_piece_alternatives: rules
                .iter()
                .enumerate()
                .filter_map(|(i, rule)| {
                    (rule.dispatch_policy
                        == crate::solver::RuleDispatchPolicy::AfterBaselinePartitionWholePiece)
                        .then_some(i)
                })
                .collect(),
            rules,
            terminals,
            deferred_points: None,
            coalescing_bound,
            overlay,
            point_index: std::sync::OnceLock::new(),
        }
    }

    /// boxed-node: the exact-key index of this batch's point rules for keys
    /// of `arity` slots, built once on first use.
    pub(in crate::solver::candidate_reduction) fn point_index(
        &self,
        arity: usize,
    ) -> &PointRuleIndex {
        let index = self
            .point_index
            .get_or_init(|| PointRuleIndex::new(&self.rules, arity));
        debug_assert_eq!(index.arity, arity);
        index
    }
}

/// boxed-node: an exact-key index of a batch's POINT rules (every slot of the
/// key fixed), so that first-match dispatch over a boxed table (tens of
/// thousands of point rules) is a hash lookup instead of a linear scan.
///
/// [`Self::candidates`] lists, ascending, every rule position that can apply
/// to a key: the point rules fixed exactly at the key and every other rule.
/// A point rule fixed elsewhere is refused by the evaluator's first check
/// (the fixed slots) before any guard is recorded, so dispatch over the
/// candidates selects the same rule with the same conditions as the linear
/// scan over all rules.
#[derive(Debug, Default)]
pub(in crate::solver::candidate_reduction) struct PointRuleIndex {
    arity: usize,
    points: std::collections::HashMap<Box<[i16]>, Vec<usize>>,
    general: Vec<usize>,
}

impl PointRuleIndex {
    fn new<const N: usize>(rules: &[PreparedRule<N>], arity: usize) -> Self {
        let mut index = Self {
            arity,
            ..Self::default()
        };
        for (position, rule) in rules.iter().enumerate() {
            let fixed: Option<Box<[i16]>> = (arity <= N)
                .then(|| rule.fixed[..arity].iter().copied().collect())
                .flatten();
            match fixed {
                Some(key) => index.points.entry(key).or_default().push(position),
                None => index.general.push(position),
            }
        }
        index
    }

    /// Every rule position that can apply to `powers`, ascending.
    pub(in crate::solver::candidate_reduction) fn candidates(&self, powers: &[i64]) -> Vec<usize> {
        let key: Option<Box<[i16]>> = (powers.len() == self.arity)
            .then(|| powers.iter().map(|&p| i16::try_from(p).ok()).collect())
            .flatten();
        let points = key
            .and_then(|key| self.points.get(&key))
            .map_or(&[][..], Vec::as_slice);
        if self.general.is_empty() {
            return points.to_vec();
        }
        let mut merged = Vec::with_capacity(points.len() + self.general.len());
        let (mut i, mut j) = (0, 0);
        while i < points.len() || j < self.general.len() {
            if j == self.general.len() || (i < points.len() && points[i] < self.general[j]) {
                merged.push(points[i]);
                i += 1;
            } else {
                merged.push(self.general[j]);
                j += 1;
            }
        }
        merged
    }
}

impl<const N: usize> PreparedOwner<N> {
    /// A later terminal must never shadow an earlier applicable formula.
    /// The callback wraps the one existing evaluator, optionally with aggregate
    /// reservation accounting. Only exact Uncovered advances to the next batch.
    pub(in crate::solver::candidate_reduction) fn evaluate_step<E>(
        &self,
        key: &IntegralKey,
        mut evaluate: impl FnMut(
            &PreparedOwnerBatch<N>,
        ) -> Result<
            Result<BTreeMap<IntegralKey, Coefficient>, CandidateReductionError>,
            E,
        >,
    ) -> Result<OwnerStep, E>
    where
        E: From<CandidateReductionError>,
    {
        self.evaluate_step_with_ordinal(key, |_, batch| evaluate(batch))
    }

    /// Identical ordered dispatch with its actual immutable batch ordinal.
    /// Ordinary callers use `evaluate_step` and retain no census state.
    pub(in crate::solver::candidate_reduction) fn evaluate_step_with_ordinal<E>(
        &self,
        key: &IntegralKey,
        mut evaluate: impl FnMut(
            usize,
            &PreparedOwnerBatch<N>,
        ) -> Result<
            Result<BTreeMap<IntegralKey, Coefficient>, CandidateReductionError>,
            E,
        >,
    ) -> Result<OwnerStep, E>
    where
        E: From<CandidateReductionError>,
    {
        for (ordinal, batch) in self.batches.iter().enumerate() {
            if batch.terminals.contains(key) {
                return Ok(OwnerStep::Terminal);
            }
            if batch
                .deferred_points
                .as_ref()
                .is_some_and(|points| points.contains(key))
            {
                continue;
            }
            match evaluate(ordinal, batch)? {
                Ok(terms) => return Ok(OwnerStep::Applied(terms)),
                Err(CandidateReductionError::Uncovered { .. }) => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(OwnerStep::Uncovered)
    }
}

#[cfg(test)]
mod point_index_tests {
    use super::PointRuleIndex;

    /// Candidates: the point rules fixed at the key merged, ascending, with
    /// every general rule; a key outside `i16` or of another arity meets only
    /// the general rules.
    #[test]
    fn candidates_merge_point_rules_at_the_key_with_general_rules() {
        let mut index = PointRuleIndex {
            arity: 2,
            ..PointRuleIndex::default()
        };
        index.points.insert(Box::new([2, 1]), vec![1, 4]);
        index.points.insert(Box::new([3, 1]), vec![2]);
        assert_eq!(index.candidates(&[2, 1]), vec![1, 4]);
        assert_eq!(index.candidates(&[5, 1]), Vec::<usize>::new());
        index.general = vec![0, 3, 7];
        assert_eq!(index.candidates(&[2, 1]), vec![0, 1, 3, 4, 7]);
        assert_eq!(index.candidates(&[3, 1]), vec![0, 2, 3, 7]);
        assert_eq!(index.candidates(&[5, 1]), vec![0, 3, 7]);
        assert_eq!(index.candidates(&[70_000, 1]), vec![0, 3, 7]);
        assert_eq!(index.candidates(&[2, 1, 0]), vec![0, 3, 7]);
    }
}
