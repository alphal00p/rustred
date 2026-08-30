use std::fmt;

use crate::algebra::IndexedCoefficient;
use crate::foundry::parametric::{
    ParametricNonZeroGuard, ParametricRule, SectorMonotoneDependency, SectorMonotoneTargetAdmission,
};
use crate::sector::{SectorMonotoneTargetCell, SectorMonotoneTargetCellKind};

use super::error::ParametricDependencyError;
use super::plan::ParametricProperSubsectorPlan;

/// Stable resume point in a term-major, target-cell-minor discovery stream.
///
/// The fields are intentionally read-only. A plan validates all three values
/// and the borrowed rule allocation before resuming, so a cursor from another
/// rule cannot silently skip work. This process-local cursor is deliberately
/// not a durable semantic or artifact identity.
#[derive(Clone, Copy)]
pub struct ParametricDependencyCursor<'rule> {
    pub(super) rule: &'rule ParametricRule,
    pub(super) right_hand_side_ordinal: usize,
    pub(super) target_cell_ordinal: usize,
    pub(super) obligation_ordinal: usize,
}

impl fmt::Debug for ParametricDependencyCursor<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ParametricDependencyCursor")
            .field("right_hand_side_ordinal", &self.right_hand_side_ordinal)
            .field("target_cell_ordinal", &self.target_cell_ordinal)
            .field("obligation_ordinal", &self.obligation_ordinal)
            .finish_non_exhaustive()
    }
}

impl PartialEq for ParametricDependencyCursor<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.rule, other.rule)
            && self.right_hand_side_ordinal == other.right_hand_side_ordinal
            && self.target_cell_ordinal == other.target_cell_ordinal
            && self.obligation_ordinal == other.obligation_ordinal
    }
}

impl Eq for ParametricDependencyCursor<'_> {}

impl ParametricDependencyCursor<'_> {
    pub fn right_hand_side_ordinal(self) -> usize {
        self.right_hand_side_ordinal
    }

    pub fn target_cell_ordinal(self) -> usize {
        self.target_cell_ordinal
    }

    pub fn obligation_ordinal(self) -> usize {
        self.obligation_ordinal
    }
}

/// One exact proper-subsector image of one parametric RHS term.
///
/// This value is an unresolved obligation. Its applicability is the
/// conjunction of the parent rule's retained guards and the nonvanishing of
/// the RHS coefficient numerator on the explicitly materialized cell's base
/// domain. That Boolean locus is deliberately not approximated here.
#[derive(Debug)]
pub struct ParametricProperSubsectorObligation<'plan, 'rule> {
    pub(super) ordinal: usize,
    pub(super) right_hand_side_ordinal: usize,
    pub(super) target_cell_ordinal: usize,
    pub(super) plan: &'plan ParametricProperSubsectorPlan<'rule>,
    pub(super) coefficient: &'rule IndexedCoefficient,
    pub(super) nonzero_guards: &'rule [ParametricNonZeroGuard],
    pub(super) dependency: &'rule SectorMonotoneDependency,
    pub(super) admission: &'rule SectorMonotoneTargetAdmission,
    pub(super) rule: &'rule ParametricRule,
}

impl<'plan, 'rule> ParametricProperSubsectorObligation<'plan, 'rule> {
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub fn right_hand_side_ordinal(&self) -> usize {
        self.right_hand_side_ordinal
    }

    pub fn target_cell_ordinal(&self) -> usize {
        self.target_cell_ordinal
    }

    /// Materialize the exact base/pivot/target domains on explicit demand.
    /// Streaming or retaining descriptors performs no O(K) cell allocation.
    pub fn try_materialize_cell(
        &self,
    ) -> Result<SectorMonotoneTargetCell, ParametricDependencyError> {
        let partition = self
            .plan
            .partitions()
            .get(self.right_hand_side_ordinal)
            .ok_or(ParametricDependencyError::Invariant {
                detail: "obligation RHS ordinal is outside its discovery plan",
            })?;
        Ok(partition.cell(self.target_cell_ordinal)?)
    }

    pub fn coefficient(&self) -> &'rule IndexedCoefficient {
        self.coefficient
    }

    pub fn nonzero_guards(&self) -> &'rule [ParametricNonZeroGuard] {
        self.nonzero_guards
    }

    pub fn dependency(&self) -> &'rule SectorMonotoneDependency {
        self.dependency
    }

    /// Complete parent rule carrying family/context/order, source replay,
    /// pivot provenance, and concrete specialization replay.
    pub fn parent_rule(&self) -> &'rule ParametricRule {
        self.rule
    }

    /// Replay descriptor binding and the exact term-local partition.
    ///
    /// This cold check does not prove coefficient/guard applicability on the
    /// cell, transitive closure, terminal status, or artifact validity.
    pub fn try_verify(&self) -> Result<bool, ParametricDependencyError> {
        let Some(term) = self
            .rule
            .right_hand_side()
            .get(self.right_hand_side_ordinal)
        else {
            return Ok(false);
        };
        let Some(partition) = self.plan.partitions().get(self.right_hand_side_ordinal) else {
            return Ok(false);
        };
        if !std::ptr::eq(self.plan.rule(), self.rule)
            || !std::ptr::eq(self.plan.admission(), self.admission)
            || self.plan.proper_subsector_obligation_ordinal_at(
                self.right_hand_side_ordinal,
                self.target_cell_ordinal,
            )? != Some(self.ordinal)
            || self.dependency.right_hand_side_ordinal() != self.right_hand_side_ordinal
            || !std::ptr::eq(term.coefficient(), self.coefficient)
            || !std::ptr::eq(self.rule.nonzero_guards(), self.nonzero_guards)
            || !matches!(
                self.rule.sector_monotone_admission(),
                Some(admission) if std::ptr::eq(admission, self.admission)
            )
            || !matches!(
                self.admission.dependencies().get(self.right_hand_side_ordinal),
                Some(dependency) if std::ptr::eq(dependency, self.dependency)
            )
            || partition.witness() != self.dependency.descent()
            || partition.cell_kind(self.target_cell_ordinal)?
                != SectorMonotoneTargetCellKind::ProperSubsector
        {
            return Ok(false);
        }
        Ok(partition.try_verify()?)
    }
}

/// Lazy streaming cursor over one preflighted discovery plan.
pub struct ParametricProperSubsectorObligations<'plan, 'rule> {
    pub(super) plan: &'plan ParametricProperSubsectorPlan<'rule>,
    pub(super) cursor: ParametricDependencyCursor<'rule>,
}

impl<'plan, 'rule> ParametricProperSubsectorObligations<'plan, 'rule> {
    pub fn cursor(&self) -> ParametricDependencyCursor<'rule> {
        self.cursor
    }

    pub fn remaining_obligation_count(&self) -> usize {
        self.plan.proper_subsector_obligation_count() - self.cursor.obligation_ordinal
    }
}

impl<'plan, 'rule> Iterator for ParametricProperSubsectorObligations<'plan, 'rule> {
    type Item =
        Result<ParametricProperSubsectorObligation<'plan, 'rule>, ParametricDependencyError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let Some(partition) = self
                .plan
                .partitions()
                .get(self.cursor.right_hand_side_ordinal)
            else {
                return None;
            };
            if self.cursor.target_cell_ordinal == partition.cell_count() {
                self.cursor.right_hand_side_ordinal += 1;
                self.cursor.target_cell_ordinal = 0;
                continue;
            }
            let target_cell_ordinal = self.cursor.target_cell_ordinal;
            let cell_kind = match partition.cell_kind(target_cell_ordinal) {
                Ok(cell_kind) => cell_kind,
                Err(error) => return Some(Err(error.into())),
            };
            self.cursor.target_cell_ordinal += 1;
            if cell_kind == SectorMonotoneTargetCellKind::SameSector {
                continue;
            }
            let right_hand_side_ordinal = self.cursor.right_hand_side_ordinal;
            let Some(term) = self
                .plan
                .rule()
                .right_hand_side()
                .get(right_hand_side_ordinal)
            else {
                return Some(Err(ParametricDependencyError::Invariant {
                    detail: "partition ordinal is outside the parametric RHS",
                }));
            };
            let Some(dependency) = self
                .plan
                .admission()
                .dependencies()
                .get(right_hand_side_ordinal)
            else {
                return Some(Err(ParametricDependencyError::Invariant {
                    detail: "partition ordinal is outside the sector-monotone dependencies",
                }));
            };
            let ordinal = self.cursor.obligation_ordinal;
            self.cursor.obligation_ordinal += 1;
            return Some(Ok(ParametricProperSubsectorObligation {
                ordinal,
                right_hand_side_ordinal,
                target_cell_ordinal,
                plan: self.plan,
                coefficient: term.coefficient(),
                nonzero_guards: self.plan.rule().nonzero_guards(),
                dependency,
                admission: self.plan.admission(),
                rule: self.plan.rule(),
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::foundry::dependency::{ParametricDependencyLimits, ParametricProperSubsectorPlan};
    use crate::foundry::parametric::tests::support::sunset_sources;
    use crate::foundry::parametric::{
        ParametricRuleLimits, derive_sector_monotone_rule_for_target,
    };
    use crate::sector::OrderingPolicy;

    #[test]
    fn cold_replay_rejects_a_tampered_global_ordinal() {
        let (_, context, relations) = sunset_sources();
        let rule = derive_sector_monotone_rule_for_target(
            &context,
            &relations,
            &[1, 1, 1],
            &[1, 0, 0],
            OrderingPolicy::default(),
            ParametricRuleLimits::default(),
        )
        .unwrap();
        let plan =
            ParametricProperSubsectorPlan::try_new(&rule, ParametricDependencyLimits::default())
                .unwrap();
        let mut obligation = plan.obligations().next().unwrap().unwrap();
        assert!(obligation.try_verify().unwrap());
        obligation.ordinal = 1;
        assert!(!obligation.try_verify().unwrap());
    }
}
