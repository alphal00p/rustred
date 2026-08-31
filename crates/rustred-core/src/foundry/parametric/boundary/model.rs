use crate::foundry::completion::frame::exact::ExactCircuitLoweringSeal;
use crate::identity::IndexShift;
use crate::sector::{
    ComplexityComponent, Mask, SectorMonotoneDomain, SectorMonotonePointClass,
    SectorMonotoneShiftDescentWitness, StrictDescentWitness,
};

use super::super::error::ParametricRuleError;
use super::super::prepare::try_vec;

/// Classification of one universal RHS dependency at a concrete point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SectorMonotoneDependencyKind {
    SameSector,
    ProperSubsector,
}

/// A universal, term-local piecewise proof for one parametric RHS shift.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SectorMonotoneDependency {
    right_hand_side_ordinal: usize,
    pivot_shift: IndexShift,
    shift: IndexShift,
    descent: SectorMonotoneShiftDescentWitness,
}

impl SectorMonotoneDependency {
    pub fn right_hand_side_ordinal(&self) -> usize {
        self.right_hand_side_ordinal
    }

    pub fn pivot_shift(&self) -> &IndexShift {
        &self.pivot_shift
    }

    pub fn shift(&self) -> &IndexShift {
        &self.shift
    }

    /// Universal same-sector/pinch proof. Proper-subsector cells are explicit
    /// dependencies only; the witness makes no lower-rule or closure claim.
    pub fn descent(&self) -> &SectorMonotoneShiftDescentWitness {
        &self.descent
    }

    pub fn verify(&self) -> bool {
        self.descent.verify()
            && key_matches_shift(self.descent.pivot(), &self.pivot_shift)
            && key_matches_shift(self.descent.target(), &self.shift)
    }

    /// Materialize the unique proof-bearing classification at one point of
    /// the universal parent-sector box.
    pub fn classify(
        &self,
        indices: &[i64],
    ) -> Result<SectorMonotoneDependencyAtPoint, ParametricRuleError> {
        let class = self
            .descent
            .classify(indices)?
            .ok_or(ParametricRuleError::PointOutsideSectorMonotoneDomain)?;
        let pivot = self
            .descent
            .domain()
            .checked_translate(indices, self.pivot_shift.values())?
            .ok_or(ParametricRuleError::ReducerInvariant {
                detail: "sector-monotone pivot is not representable at a contained point",
            })?;
        let target = self
            .descent
            .domain()
            .checked_translate(indices, self.shift.values())?
            .ok_or(ParametricRuleError::ReducerInvariant {
                detail: "sector-monotone RHS is not representable at a contained point",
            })?;
        let concrete_descent = self
            .descent
            .policy()
            .prove_strict_descent(&pivot, &target)?;
        let kind = match class {
            SectorMonotonePointClass::SameSector => {
                if concrete_descent.source().sector() != concrete_descent.target().sector()
                    || concrete_descent.decisive_component() == ComplexityComponent::PropagatorCount
                {
                    return Err(ParametricRuleError::ReducerInvariant {
                        detail: "same-sector monotone cell produced a cross-sector descent",
                    });
                }
                SectorMonotoneDependencyKind::SameSector
            }
            SectorMonotonePointClass::ProperSubsector { .. } => {
                if !concrete_descent
                    .target()
                    .sector()
                    .is_strict_subsector_of(concrete_descent.source().sector())?
                    || concrete_descent.decisive_component() != ComplexityComponent::PropagatorCount
                {
                    return Err(ParametricRuleError::ReducerInvariant {
                        detail: "pinch cylinder did not produce propagator-count descent",
                    });
                }
                SectorMonotoneDependencyKind::ProperSubsector
            }
        };
        Ok(SectorMonotoneDependencyAtPoint {
            right_hand_side_ordinal: self.right_hand_side_ordinal,
            shift: self.shift.clone(),
            kind,
            partition_class: class,
            concrete_descent,
        })
    }

    pub(super) fn new(
        right_hand_side_ordinal: usize,
        pivot_shift: IndexShift,
        shift: IndexShift,
        descent: SectorMonotoneShiftDescentWitness,
    ) -> Self {
        Self {
            right_hand_side_ordinal,
            pivot_shift,
            shift,
            descent,
        }
    }

    pub(crate) fn from_exact_lowering(
        _seal: &ExactCircuitLoweringSeal,
        right_hand_side_ordinal: usize,
        pivot_shift: IndexShift,
        shift: IndexShift,
        descent: SectorMonotoneShiftDescentWitness,
    ) -> Self {
        Self::new(right_hand_side_ordinal, pivot_shift, shift, descent)
    }
}

/// Concrete specialization of one universal sector-monotone dependency.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SectorMonotoneDependencyAtPoint {
    right_hand_side_ordinal: usize,
    shift: IndexShift,
    kind: SectorMonotoneDependencyKind,
    partition_class: SectorMonotonePointClass,
    concrete_descent: StrictDescentWitness,
}

impl SectorMonotoneDependencyAtPoint {
    pub fn right_hand_side_ordinal(&self) -> usize {
        self.right_hand_side_ordinal
    }

    pub fn shift(&self) -> &IndexShift {
        &self.shift
    }

    pub fn kind(&self) -> SectorMonotoneDependencyKind {
        self.kind
    }

    pub fn partition_class(&self) -> SectorMonotonePointClass {
        self.partition_class
    }

    pub fn target_sector(&self) -> &Mask {
        self.concrete_descent.target().sector()
    }

    pub fn descent(&self) -> &StrictDescentWitness {
        &self.concrete_descent
    }

    pub fn pinched_positions(&self) -> impl Iterator<Item = usize> + '_ {
        self.concrete_descent
            .source()
            .sector()
            .active_bits()
            .iter()
            .zip(self.concrete_descent.target().sector().active_bits())
            .enumerate()
            .filter_map(|(position, (&source, &target))| (source && !target).then_some(position))
    }

    pub fn verify(&self) -> bool {
        if !self.concrete_descent.verify() {
            return false;
        }
        match self.kind {
            SectorMonotoneDependencyKind::SameSector => {
                self.concrete_descent.source().sector() == self.concrete_descent.target().sector()
                    && self.concrete_descent.decisive_component()
                        != ComplexityComponent::PropagatorCount
                    && matches!(self.partition_class, SectorMonotonePointClass::SameSector)
            }
            SectorMonotoneDependencyKind::ProperSubsector => {
                self.concrete_descent
                    .target()
                    .sector()
                    .is_strict_subsector_of(self.concrete_descent.source().sector())
                    .unwrap_or(false)
                    && self.concrete_descent.decisive_component()
                        == ComplexityComponent::PropagatorCount
                    && matches!(
                        self.partition_class,
                        SectorMonotonePointClass::ProperSubsector { .. }
                    )
            }
        }
    }
}

/// Universal sector-monotone domain and one exhaustive term-local proof per
/// RHS shift of a parametric recurrence.
///
/// Proper-subsector cells are deliberately unresolved dependencies. This
/// value neither assumes a lower-sector rule nor claims sector closure.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SectorMonotoneTargetAdmission {
    domain: SectorMonotoneDomain,
    pivot: IndexShift,
    dependencies: Vec<SectorMonotoneDependency>,
}

impl SectorMonotoneTargetAdmission {
    pub fn domain(&self) -> &SectorMonotoneDomain {
        &self.domain
    }

    pub fn parent_sector(&self) -> &Mask {
        self.domain.sector()
    }

    pub fn pivot(&self) -> &IndexShift {
        &self.pivot
    }

    pub fn dependencies(&self) -> &[SectorMonotoneDependency] {
        &self.dependencies
    }

    pub fn classify(
        &self,
        indices: &[i64],
    ) -> Result<Vec<SectorMonotoneDependencyAtPoint>, ParametricRuleError> {
        if !self.domain.contains(indices)? {
            return Err(ParametricRuleError::PointOutsideSectorMonotoneDomain);
        }
        let mut classified = try_vec(
            "sector-monotone point dependencies",
            self.dependencies.len(),
        )?;
        for dependency in &self.dependencies {
            classified.push(dependency.classify(indices)?);
        }
        Ok(classified)
    }

    pub fn proper_subsector_dependency_count_at(
        &self,
        indices: &[i64],
    ) -> Result<usize, ParametricRuleError> {
        Ok(self
            .classify(indices)?
            .iter()
            .filter(|dependency| dependency.kind() == SectorMonotoneDependencyKind::ProperSubsector)
            .count())
    }

    pub fn verify(&self) -> bool {
        self.dependencies
            .iter()
            .enumerate()
            .all(|(ordinal, dependency)| {
                dependency.right_hand_side_ordinal == ordinal
                    && dependency.pivot_shift == self.pivot
                    && dependency.descent.domain() == &self.domain
                    && dependency.verify()
            })
    }

    pub(super) fn new(
        domain: SectorMonotoneDomain,
        pivot: IndexShift,
        dependencies: Vec<SectorMonotoneDependency>,
    ) -> Self {
        Self {
            domain,
            pivot,
            dependencies,
        }
    }

    pub(crate) fn from_exact_lowering(
        _seal: &ExactCircuitLoweringSeal,
        domain: SectorMonotoneDomain,
        pivot: IndexShift,
        dependencies: Vec<SectorMonotoneDependency>,
    ) -> Self {
        Self::new(domain, pivot, dependencies)
    }
}

fn key_matches_shift(key: &crate::sector::ShiftComplexityKey, shift: &IndexShift) -> bool {
    key.arity() == shift.values().len()
        && shift
            .values()
            .iter()
            .enumerate()
            .all(|(position, &coordinate)| key.shift_at(position) == Ok(coordinate))
}
