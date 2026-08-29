use crate::foundry::parametric::{ParametricRule, SectorMonotoneTargetAdmission};
use crate::sector::SectorMonotoneTargetPartition;

use super::error::ParametricDependencyError;
use super::limits::ParametricDependencyLimits;
use super::model::{ParametricDependencyCursor, ParametricProperSubsectorObligations};

/// Preflighted O(R*K) owner for one lazy proper-subsector obligation stream.
///
/// The plan borrows its authenticated rule instead of cloning Symbolica
/// coefficients or guard payloads. It retains only compact target partitions.
/// Iteration yields O(1) descriptors; exact O(K) domains are built only when a
/// caller explicitly materializes a descriptor.
#[derive(Debug)]
pub struct ParametricProperSubsectorPlan<'rule> {
    rule: &'rule ParametricRule,
    admission: &'rule SectorMonotoneTargetAdmission,
    partitions: Vec<SectorMonotoneTargetPartition>,
    described_target_sector_cell_count: usize,
    proper_subsector_obligation_count: usize,
}

impl<'rule> ParametricProperSubsectorPlan<'rule> {
    pub fn try_new(
        rule: &'rule ParametricRule,
        limits: ParametricDependencyLimits,
    ) -> Result<Self, ParametricDependencyError> {
        let admission = rule
            .sector_monotone_admission()
            .ok_or(ParametricDependencyError::RuleHasNoSectorMonotoneAdmission)?;
        if !rule_and_admission_are_bound(rule, admission) {
            return Err(ParametricDependencyError::Invariant {
                detail: "the rule and sector-monotone admission are not bound term for term",
            });
        }
        check_limit(
            "parametric dependency rule terms",
            admission.dependencies().len(),
            limits.max_rule_terms,
        )?;
        let partition_coordinate_cells = checked_mul(
            "parametric dependency partition coordinate cells",
            checked_mul(
                "parametric dependency partition coordinate cells",
                admission.dependencies().len(),
                admission.parent_sector().arity(),
            )?,
            3,
        )?;
        check_limit(
            "parametric dependency partition coordinate cells",
            partition_coordinate_cells,
            limits.max_partition_coordinate_cells,
        )?;
        let per_obligation_materialization_coordinate_cells = checked_mul(
            "parametric dependency per-obligation materialization coordinate cells",
            admission.parent_sector().arity(),
            4,
        )?;
        check_limit(
            "parametric dependency per-obligation materialization coordinate cells",
            per_obligation_materialization_coordinate_cells,
            limits.max_per_obligation_materialization_coordinate_cells,
        )?;

        let mut described_target_sector_cell_count = 0usize;
        let mut proper_subsector_obligation_count = 0usize;
        for dependency in admission.dependencies() {
            let census = dependency.descent().target_sector_partition_census()?;
            described_target_sector_cell_count = checked_add(
                "described parametric target-sector cells",
                described_target_sector_cell_count,
                census.cell_count(),
            )?;
            proper_subsector_obligation_count = checked_add(
                "proper-subsector obligations",
                proper_subsector_obligation_count,
                census.proper_subsector_cell_count(),
            )?;
        }
        check_limit(
            "described parametric target-sector cells",
            described_target_sector_cell_count,
            limits.max_described_target_sector_cells,
        )?;
        check_limit(
            "proper-subsector obligations",
            proper_subsector_obligation_count,
            limits.max_proper_subsector_obligations,
        )?;

        let mut partitions = Vec::new();
        partitions
            .try_reserve_exact(admission.dependencies().len())
            .map_err(|_| ParametricDependencyError::AllocationFailure {
                resource: "parametric dependency target partitions",
                requested: admission.dependencies().len(),
            })?;
        for dependency in admission.dependencies() {
            let partition = dependency.descent().try_target_sector_partition()?;
            partitions.push(partition);
        }
        Ok(Self {
            rule,
            admission,
            partitions,
            described_target_sector_cell_count,
            proper_subsector_obligation_count,
        })
    }

    pub fn rule(&self) -> &'rule ParametricRule {
        self.rule
    }

    pub fn admission(&self) -> &'rule SectorMonotoneTargetAdmission {
        self.admission
    }

    pub fn described_target_sector_cell_count(&self) -> usize {
        self.described_target_sector_cell_count
    }

    pub fn proper_subsector_obligation_count(&self) -> usize {
        self.proper_subsector_obligation_count
    }

    pub fn obligations(&self) -> ParametricProperSubsectorObligations<'_, 'rule> {
        ParametricProperSubsectorObligations {
            plan: self,
            cursor: ParametricDependencyCursor {
                rule: self.rule,
                right_hand_side_ordinal: 0,
                target_cell_ordinal: 0,
                obligation_ordinal: 0,
            },
        }
    }

    pub fn obligations_from(
        &self,
        cursor: ParametricDependencyCursor<'rule>,
    ) -> Result<ParametricProperSubsectorObligations<'_, 'rule>, ParametricDependencyError> {
        if !self.valid_cursor(cursor)? {
            return Err(ParametricDependencyError::InvalidCursor);
        }
        Ok(ParametricProperSubsectorObligations { plan: self, cursor })
    }

    pub fn try_verify(&self) -> Result<bool, ParametricDependencyError> {
        if !rule_and_admission_are_bound(self.rule, self.admission)
            || self.partitions.len() != self.admission.dependencies().len()
        {
            return Ok(false);
        }
        let mut described = 0usize;
        let mut proper = 0usize;
        for (dependency, partition) in self.admission.dependencies().iter().zip(&self.partitions) {
            if partition.witness() != dependency.descent() || !partition.try_verify()? {
                return Ok(false);
            }
            described = checked_add(
                "described parametric target-sector cells",
                described,
                partition.cell_count(),
            )?;
            proper = checked_add(
                "proper-subsector obligations",
                proper,
                partition.proper_subsector_cell_count(),
            )?;
        }
        Ok(described == self.described_target_sector_cell_count
            && proper == self.proper_subsector_obligation_count)
    }

    pub(super) fn partitions(&self) -> &[SectorMonotoneTargetPartition] {
        &self.partitions
    }

    pub(super) fn proper_subsector_obligation_ordinal_at(
        &self,
        right_hand_side_ordinal: usize,
        target_cell_ordinal: usize,
    ) -> Result<Option<usize>, ParametricDependencyError> {
        let Some(partition) = self.partitions.get(right_hand_side_ordinal) else {
            return Ok(None);
        };
        if target_cell_ordinal >= partition.cell_count()
            || partition.cell_kind(target_cell_ordinal)?
                != crate::sector::SectorMonotoneTargetCellKind::ProperSubsector
        {
            return Ok(None);
        }
        let mut ordinal = 0usize;
        for prior in &self.partitions[..right_hand_side_ordinal] {
            ordinal = checked_add(
                "proper-subsector obligation ordinal",
                ordinal,
                prior.proper_subsector_cell_count(),
            )?;
        }
        ordinal = checked_add(
            "proper-subsector obligation ordinal",
            ordinal,
            partition.proper_subsector_cell_count_before(target_cell_ordinal)?,
        )?;
        Ok(Some(ordinal))
    }

    fn valid_cursor(
        &self,
        cursor: ParametricDependencyCursor<'rule>,
    ) -> Result<bool, ParametricDependencyError> {
        if !std::ptr::eq(cursor.rule, self.rule) {
            return Ok(false);
        }
        if cursor.right_hand_side_ordinal > self.partitions.len() {
            return Ok(false);
        }
        if cursor.right_hand_side_ordinal == self.partitions.len() {
            return Ok(cursor.target_cell_ordinal == 0
                && cursor.obligation_ordinal == self.proper_subsector_obligation_count);
        }
        let partition = &self.partitions[cursor.right_hand_side_ordinal];
        if cursor.target_cell_ordinal > partition.cell_count() {
            return Ok(false);
        }
        let mut expected_obligation_ordinal = 0usize;
        for prior in &self.partitions[..cursor.right_hand_side_ordinal] {
            expected_obligation_ordinal = checked_add(
                "proper-subsector cursor obligation ordinal",
                expected_obligation_ordinal,
                prior.proper_subsector_cell_count(),
            )?;
        }
        expected_obligation_ordinal = checked_add(
            "proper-subsector cursor obligation ordinal",
            expected_obligation_ordinal,
            partition.proper_subsector_cell_count_before(cursor.target_cell_ordinal)?,
        )?;
        Ok(cursor.obligation_ordinal == expected_obligation_ordinal)
    }
}

fn rule_and_admission_are_bound(
    rule: &ParametricRule,
    admission: &SectorMonotoneTargetAdmission,
) -> bool {
    admission.verify()
        && admission.parent_sector() == rule.sector()
        && admission.pivot() == rule.pivot()
        && admission.dependencies().len() == rule.right_hand_side().len()
        && admission
            .dependencies()
            .iter()
            .zip(rule.right_hand_side())
            .all(|(dependency, term)| {
                dependency.shift() == term.shift()
                    && dependency.pivot_shift() == rule.pivot()
                    && dependency.descent().policy() == rule.ordering()
            })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ParametricDependencyError> {
    if requested > limit {
        Err(ParametricDependencyError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ParametricDependencyError> {
    left.checked_add(right)
        .ok_or(ParametricDependencyError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ParametricDependencyError> {
    left.checked_mul(right)
        .ok_or(ParametricDependencyError::ResourceCountOverflow { resource })
}
