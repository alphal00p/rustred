//! Authenticated integer-cylinder starts for generated residual work items.
//!
//! This is the first symbolic `startp` proof layer.  It resolves one exact
//! live-leaf queue item, replays its coordinate-equality extraction, preserves
//! every source predicate through the owned queue, and schedules exact
//! cylindrical prepare-point shells without filling free indices by a corner.
//!
//! The certificate deliberately proves no rule, coverage, zero, or master.
//! An unresolved equality outside the literal-coordinate language is retained
//! as [`GeneratedCylindricalStartCompleteness::DependentSymbolicStartPending`]
//! rather than being sampled away or mistaken for full LiteRed parity.

use std::fmt;
use std::sync::Arc;

use crate::{
    CoordinateEqualityLeafStatus, CylindricalOrderingError, CylindricalOrderingLimits,
    CylindricalParametricEliminationOrdering, CylindricalPreparePointScheduleCertificate,
    CylindricalPreparePointScheduleError, CylindricalPreparePointScheduleLimits,
    GeneratedSectorLiveLeafQueueCertificate, GeneratedSectorLiveLeafQueueError,
    GeneratedSymbolicRowSpanCertificate, IntegralFamily, ParametricCoefficientContext,
    PartialIndexAssignment, SectorMask, SymbolicPolynomialPredicateKind, SymbolicSectorCaseId,
};

pub const GENERATED_CYLINDRICAL_RESIDUAL_START_V1_SCHEMA: &str =
    "rustred-generated-cylindrical-residual-start-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedCylindricalResidualStartLimits {
    pub ordering: CylindricalOrderingLimits,
    pub schedule: CylindricalPreparePointScheduleLimits,
    pub max_work_item_ordinal: usize,
    pub max_assignment_entries: usize,
    pub max_pending_dependent_equalities: usize,
}

impl Default for GeneratedCylindricalResidualStartLimits {
    fn default() -> Self {
        Self {
            ordering: CylindricalOrderingLimits::default(),
            schedule: CylindricalPreparePointScheduleLimits::default(),
            max_work_item_ordinal: 1_000_000,
            max_assignment_entries: 1_000_000,
            max_pending_dependent_equalities: 1_000_000,
        }
    }
}

/// Whether the selected source case is fully represented by literal fixed
/// coordinates or still contains equality predicates requiring the dependent
/// affine/general start layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedCylindricalStartCompleteness {
    IndependentIntegerCylinder,
    DependentSymbolicStartPending {
        unresolved_equality_predicate_ordinals: Box<[usize]>,
    },
}

impl GeneratedCylindricalStartCompleteness {
    pub fn pending_equality_predicate_ordinals(&self) -> &[usize] {
        match self {
            Self::IndependentIntegerCylinder => &[],
            Self::DependentSymbolicStartPending {
                unresolved_equality_predicate_ordinals,
            } => unresolved_equality_predicate_ordinals,
        }
    }

    pub const fn is_complete_integer_cylinder(&self) -> bool {
        matches!(self, Self::IndependentIntegerCylinder)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedCylindricalResidualStartStats {
    assignment_entries: usize,
    free_positions: usize,
    source_predicates: usize,
    residual_unresolved_predicates: usize,
    pending_dependent_equalities: usize,
    prepare_point_layers: usize,
    prepare_points: usize,
}

impl GeneratedCylindricalResidualStartStats {
    pub const fn assignment_entries(self) -> usize {
        self.assignment_entries
    }
    pub const fn free_positions(self) -> usize {
        self.free_positions
    }
    pub const fn source_predicates(self) -> usize {
        self.source_predicates
    }
    pub const fn residual_unresolved_predicates(self) -> usize {
        self.residual_unresolved_predicates
    }
    pub const fn pending_dependent_equalities(self) -> usize {
        self.pending_dependent_equalities
    }
    pub const fn prepare_point_layers(self) -> usize {
        self.prepare_point_layers
    }
    pub const fn prepare_points(self) -> usize {
        self.prepare_points
    }
}

#[derive(Clone, Debug)]
pub struct GeneratedCylindricalResidualStartCertificate {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    source_queue: Arc<GeneratedSectorLiveLeafQueueCertificate>,
    source_work_item_ordinal: usize,
    source_case: SymbolicSectorCaseId,
    sector: SectorMask,
    assignment: PartialIndexAssignment,
    completeness: GeneratedCylindricalStartCompleteness,
    schedule: CylindricalPreparePointScheduleCertificate,
    limits: GeneratedCylindricalResidualStartLimits,
    stats: GeneratedCylindricalResidualStartStats,
}

impl GeneratedCylindricalResidualStartCertificate {
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source_queue: Arc<GeneratedSectorLiveLeafQueueCertificate>,
        source_work_item_ordinal: usize,
        through_depth: usize,
        limits: GeneratedCylindricalResidualStartLimits,
    ) -> Result<Self, GeneratedCylindricalResidualStartError> {
        source_queue.replay(family, context)?;
        let result = compile_unreplayed_with_replayed_source(
            family,
            context,
            source_queue,
            source_work_item_ordinal,
            through_depth,
            limits,
        )?;
        result.replay_with_replayed_source(family, context)?;
        Ok(result)
    }

    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }
    pub const fn source_queue(&self) -> &Arc<GeneratedSectorLiveLeafQueueCertificate> {
        &self.source_queue
    }
    pub(crate) fn row_span_arc(&self) -> &Arc<GeneratedSymbolicRowSpanCertificate> {
        self.source_queue.discovery().row_span_arc()
    }
    pub const fn source_work_item_ordinal(&self) -> usize {
        self.source_work_item_ordinal
    }
    pub const fn source_case(&self) -> SymbolicSectorCaseId {
        self.source_case
    }
    pub const fn sector(&self) -> &SectorMask {
        &self.sector
    }
    pub const fn assignment(&self) -> &PartialIndexAssignment {
        &self.assignment
    }
    pub const fn completeness(&self) -> &GeneratedCylindricalStartCompleteness {
        &self.completeness
    }
    pub const fn schedule(&self) -> &CylindricalPreparePointScheduleCertificate {
        &self.schedule
    }
    pub const fn limits(&self) -> GeneratedCylindricalResidualStartLimits {
        self.limits
    }
    pub const fn stats(&self) -> GeneratedCylindricalResidualStartStats {
        self.stats
    }

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedCylindricalResidualStartError> {
        if self.schema != GENERATED_CYLINDRICAL_RESIDUAL_START_V1_SCHEMA {
            return Err(GeneratedCylindricalResidualStartError::SchemaMismatch);
        }
        self.source_queue.replay(family, context)?;
        self.replay_with_replayed_source(family, context)
    }

    fn replay_with_replayed_source(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedCylindricalResidualStartError> {
        let replayed = compile_unreplayed_with_replayed_source(
            family,
            context,
            self.source_queue.clone(),
            self.source_work_item_ordinal,
            self.schedule.through_depth(),
            self.limits,
        )?;
        if self.payload_eq(&replayed) {
            Ok(())
        } else {
            Err(GeneratedCylindricalResidualStartError::ReplayMismatch)
        }
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.family_fingerprint == other.family_fingerprint
            && self.context_fingerprint == other.context_fingerprint
            && self.source_queue.payload_eq(&other.source_queue)
            && self.source_work_item_ordinal == other.source_work_item_ordinal
            && self.source_case == other.source_case
            && self.sector == other.sector
            && self.assignment == other.assignment
            && self.completeness == other.completeness
            && self.schedule == other.schedule
            && self.limits == other.limits
            && self.stats == other.stats
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedCylindricalResidualStartError {
    SchemaMismatch,
    ReplayMismatch,
    WrongFamily,
    WrongContext,
    WorkItemOrdinalLimit {
        requested: usize,
        limit: usize,
    },
    WorkItemNotFound {
        ordinal: usize,
    },
    SourceCaseMismatch,
    SourceLeafProvedEmpty,
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    Queue(GeneratedSectorLiveLeafQueueError),
    Ordering(CylindricalOrderingError),
    Schedule(CylindricalPreparePointScheduleError),
}

impl fmt::Display for GeneratedCylindricalResidualStartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => {
                formatter.write_str("cylindrical residual-start schema mismatch")
            }
            Self::ReplayMismatch => {
                formatter.write_str("cylindrical residual start does not replay")
            }
            Self::WrongFamily => {
                formatter.write_str("cylindrical residual start belongs to another family")
            }
            Self::WrongContext => {
                formatter.write_str("cylindrical residual start belongs to another K(n) context")
            }
            Self::WorkItemOrdinalLimit { requested, limit } => write!(
                formatter,
                "cylindrical residual work-item ordinal {requested} exceeds configured limit {limit}"
            ),
            Self::WorkItemNotFound { ordinal } => {
                write!(
                    formatter,
                    "cylindrical residual work item {ordinal} was not retained"
                )
            }
            Self::SourceCaseMismatch => formatter.write_str(
                "cylindrical residual work item is detached from its extracted source case",
            ),
            Self::SourceLeafProvedEmpty => {
                formatter.write_str("a proved-empty residual leaf has no symbolic start")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "cylindrical residual {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::Queue(error) => error.fmt(formatter),
            Self::Ordering(error) => error.fmt(formatter),
            Self::Schedule(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedCylindricalResidualStartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Queue(error) => Some(error),
            Self::Ordering(error) => Some(error),
            Self::Schedule(error) => Some(error),
            _ => None,
        }
    }
}

impl From<GeneratedSectorLiveLeafQueueError> for GeneratedCylindricalResidualStartError {
    fn from(value: GeneratedSectorLiveLeafQueueError) -> Self {
        Self::Queue(value)
    }
}
impl From<CylindricalOrderingError> for GeneratedCylindricalResidualStartError {
    fn from(value: CylindricalOrderingError) -> Self {
        Self::Ordering(value)
    }
}
impl From<CylindricalPreparePointScheduleError> for GeneratedCylindricalResidualStartError {
    fn from(value: CylindricalPreparePointScheduleError) -> Self {
        Self::Schedule(value)
    }
}

#[allow(clippy::too_many_arguments)]
fn compile_unreplayed_with_replayed_source(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    source_queue: Arc<GeneratedSectorLiveLeafQueueCertificate>,
    source_work_item_ordinal: usize,
    through_depth: usize,
    limits: GeneratedCylindricalResidualStartLimits,
) -> Result<GeneratedCylindricalResidualStartCertificate, GeneratedCylindricalResidualStartError> {
    if source_work_item_ordinal > limits.max_work_item_ordinal {
        return Err(
            GeneratedCylindricalResidualStartError::WorkItemOrdinalLimit {
                requested: source_work_item_ordinal,
                limit: limits.max_work_item_ordinal,
            },
        );
    }
    if source_queue.family_fingerprint() != family.fingerprint() {
        return Err(GeneratedCylindricalResidualStartError::WrongFamily);
    }
    if source_queue.context_fingerprint() != context.fingerprint() {
        return Err(GeneratedCylindricalResidualStartError::WrongContext);
    }
    let item = source_queue
        .work_items()
        .get(source_work_item_ordinal)
        .filter(|item| item.ordinal() == source_work_item_ordinal)
        .ok_or(GeneratedCylindricalResidualStartError::WorkItemNotFound {
            ordinal: source_work_item_ordinal,
        })?;
    let extraction = item.extraction();
    if extraction.source_case() != item.source_case() {
        return Err(GeneratedCylindricalResidualStartError::SourceCaseMismatch);
    }
    if !matches!(
        extraction.status(),
        CoordinateEqualityLeafStatus::NotProvedEmpty
    ) {
        return Err(GeneratedCylindricalResidualStartError::SourceLeafProvedEmpty);
    }
    check_limit(
        "literal assignment entries",
        extraction.assignment().entries().len(),
        limits.max_assignment_entries,
    )?;
    // Count before allocating.  In particular a zero/tight limit must reject
    // an equality-rich leaf without first materializing the rejected ordinal
    // vector, and the successful allocation is exactly the authenticated
    // number of retained references.
    let pending_equalities = bounded_pending_equality_ordinals(
        extraction
            .unresolved_predicates()
            .iter()
            .map(|predicate| (predicate.kind(), predicate.predicate_ordinal())),
        limits.max_pending_dependent_equalities,
    );
    let pending_equalities = pending_equalities?;
    let completeness = if pending_equalities.is_empty() {
        GeneratedCylindricalStartCompleteness::IndependentIntegerCylinder
    } else {
        GeneratedCylindricalStartCompleteness::DependentSymbolicStartPending {
            unresolved_equality_predicate_ordinals: pending_equalities,
        }
    };
    let ordering = CylindricalParametricEliminationOrdering::try_new(
        source_queue.ordering(),
        source_queue.sector().clone(),
        extraction.assignment().clone(),
        limits.ordering,
    )?;
    let schedule =
        CylindricalPreparePointScheduleCertificate::compile_with_replayed_shared_ordering(
            Arc::new(ordering),
            through_depth,
            limits.schedule,
        )?;
    let source_case = item.source_case();
    let source_predicates = extraction
        .source_partition()
        .case(source_case)
        .ok_or(GeneratedCylindricalResidualStartError::SourceCaseMismatch)?
        .predicates()
        .len();
    let stats = GeneratedCylindricalResidualStartStats {
        assignment_entries: extraction.assignment().entries().len(),
        free_positions: schedule.ordering().free_positions().len(),
        source_predicates,
        residual_unresolved_predicates: extraction.unresolved_predicates().len(),
        pending_dependent_equalities: completeness.pending_equality_predicate_ordinals().len(),
        prepare_point_layers: schedule.stats().layer_count(),
        prepare_points: schedule.stats().retained_points(),
    };
    Ok(GeneratedCylindricalResidualStartCertificate {
        schema: GENERATED_CYLINDRICAL_RESIDUAL_START_V1_SCHEMA,
        family_fingerprint: family.fingerprint().into(),
        context_fingerprint: context.fingerprint().into(),
        source_queue,
        source_work_item_ordinal,
        source_case,
        sector: schedule.ordering().sector().clone(),
        assignment: schedule.ordering().assignment().clone(),
        completeness,
        schedule,
        limits,
        stats,
    })
}

/// Retain equality ordinals only after a first allocation-free census has
/// proved that the complete payload fits.  Requiring a cloneable iterator
/// keeps the two-pass contract explicit and makes it independently testable.
fn bounded_pending_equality_ordinals<I>(
    predicates: I,
    limit: usize,
) -> Result<Box<[usize]>, GeneratedCylindricalResidualStartError>
where
    I: Iterator<Item = (SymbolicPolynomialPredicateKind, usize)> + Clone,
{
    let requested = predicates
        .clone()
        .filter(|(kind, _)| *kind == SymbolicPolynomialPredicateKind::EqualZero)
        .count();
    check_limit("pending dependent equality references", requested, limit)?;
    let mut ordinals = Vec::with_capacity(requested);
    ordinals.extend(
        predicates
            .filter(|(kind, _)| *kind == SymbolicPolynomialPredicateKind::EqualZero)
            .map(|(_, ordinal)| ordinal),
    );
    debug_assert_eq!(ordinals.len(), requested);
    Ok(ordinals.into_boxed_slice())
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedCylindricalResidualStartError> {
    if requested > limit {
        Err(GeneratedCylindricalResidualStartError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_equality_census_rejects_before_retaining_any_ordinal() {
        let predicates = [
            (SymbolicPolynomialPredicateKind::NonZero, 2),
            (SymbolicPolynomialPredicateKind::EqualZero, 7),
            (SymbolicPolynomialPredicateKind::EqualZero, 11),
        ];
        assert_eq!(
            bounded_pending_equality_ordinals(predicates.into_iter(), 1),
            Err(GeneratedCylindricalResidualStartError::ResourceLimit {
                resource: "pending dependent equality references",
                requested: 2,
                limit: 1,
            })
        );
        assert_eq!(
            bounded_pending_equality_ordinals(predicates.into_iter(), 2).unwrap(),
            Box::<[usize]>::from([7, 11])
        );
    }
}
