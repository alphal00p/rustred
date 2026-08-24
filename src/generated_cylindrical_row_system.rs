//! Point-major generated IBP/LI rows on an authenticated integer cylinder.
//!
//! Every source identity comes from an immutable generated row span owned by
//! either a replayed live residual or an anchor-free sector root. For each
//! scheduled prepare point the complete identity is translated first
//! (integral keys, coefficients, and guards together) and only then specialized
//! on the authenticated coordinate assignment. The rebuilt relations remain
//! private and case-bound; this module performs no elimination and exposes no
//! reduction rule.

use std::fmt;
use std::fmt::Write as _;
use std::sync::Arc;

use crate::{
    CylindricalPreparePointScheduleCertificate, GeneratedCylindricalResidualStartCertificate,
    GeneratedCylindricalResidualStartError, GeneratedCylindricalSectorRootStartCertificate,
    GeneratedCylindricalSectorRootStartError, GeneratedCylindricalStartCompleteness,
    GeneratedSymbolicRowSpanCertificate, IntegralFamily, IntegralOrderingPolicy,
    ParametricCoefficientContext, ParametricRelation, ParametricRelationError, ParametricRowId,
    PartialIndexAssignment, PartialParametricRelationSpecialization,
    PartialParametricRelationSpecializationLimits, PartialParametricRelationSpecializationStats,
    SectorMask,
};

pub const GENERATED_CYLINDRICAL_ROW_SYSTEM_V1_SCHEMA: &str =
    "rustred-generated-cylindrical-row-system-v1";
pub const GENERATED_CYLINDRICAL_ROW_SYSTEM_V2_SCHEMA: &str =
    "rustred-generated-cylindrical-row-system-v2";

/// Version-preserving source authority for cylindrical generated rows.
///
/// The residual arm retains the original V1 live-leaf proof. The sector-root
/// arm is anchor-free and owns a freshly generated global row span. Keeping
/// the arms distinct prevents an anchored residual assignment from being
/// erased when a later layer decides whether a recurrence is global.
#[derive(Clone, Debug)]
pub enum GeneratedCylindricalRowSystemStartCertificate {
    Residual(Arc<GeneratedCylindricalResidualStartCertificate>),
    SectorRoot(Arc<GeneratedCylindricalSectorRootStartCertificate>),
}

impl GeneratedCylindricalRowSystemStartCertificate {
    pub fn family_fingerprint(&self) -> &str {
        match self {
            Self::Residual(start) => start.family_fingerprint(),
            Self::SectorRoot(start) => start.family_fingerprint(),
        }
    }

    pub fn context_fingerprint(&self) -> &str {
        match self {
            Self::Residual(start) => start.context_fingerprint(),
            Self::SectorRoot(start) => start.context_fingerprint(),
        }
    }

    pub fn sector(&self) -> &SectorMask {
        match self {
            Self::Residual(start) => start.sector(),
            Self::SectorRoot(start) => start.sector(),
        }
    }

    pub fn assignment(&self) -> &PartialIndexAssignment {
        match self {
            Self::Residual(start) => start.assignment(),
            Self::SectorRoot(start) => start.assignment(),
        }
    }

    pub fn completeness(&self) -> &GeneratedCylindricalStartCompleteness {
        match self {
            Self::Residual(start) => start.completeness(),
            Self::SectorRoot(start) => start.completeness(),
        }
    }

    pub fn schedule(&self) -> &CylindricalPreparePointScheduleCertificate {
        match self {
            Self::Residual(start) => start.schedule(),
            Self::SectorRoot(start) => start.schedule(),
        }
    }

    pub fn row_span(&self) -> &GeneratedSymbolicRowSpanCertificate {
        self.row_span_arc().as_ref()
    }

    /// Retained row-span allocation shared with the authenticated start arm.
    /// This is crate-private proof composition material: public callers only
    /// need the borrowed row span exposed by [`Self::row_span`].
    pub(crate) fn row_span_arc(&self) -> &Arc<GeneratedSymbolicRowSpanCertificate> {
        match self {
            Self::Residual(start) => start.row_span_arc(),
            Self::SectorRoot(start) => start.row_span_arc(),
        }
    }

    pub fn ordering_policy(&self) -> IntegralOrderingPolicy {
        match self {
            Self::Residual(start) => start.source_queue().ordering(),
            Self::SectorRoot(start) => start.ordering_policy(),
        }
    }

    pub fn residual_start(&self) -> Option<&Arc<GeneratedCylindricalResidualStartCertificate>> {
        match self {
            Self::Residual(start) => Some(start),
            Self::SectorRoot(_) => None,
        }
    }

    pub fn sector_root_start(
        &self,
    ) -> Option<&Arc<GeneratedCylindricalSectorRootStartCertificate>> {
        match self {
            Self::Residual(_) => None,
            Self::SectorRoot(start) => Some(start),
        }
    }

    pub fn is_sector_root(&self) -> bool {
        matches!(self, Self::SectorRoot(_))
    }

    fn row_system_schema(&self) -> &'static str {
        match self {
            Self::Residual(_) => GENERATED_CYLINDRICAL_ROW_SYSTEM_V1_SCHEMA,
            Self::SectorRoot(_) => GENERATED_CYLINDRICAL_ROW_SYSTEM_V2_SCHEMA,
        }
    }

    fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedCylindricalRowSystemError> {
        match self {
            Self::Residual(start) => start.replay(family, context).map_err(Into::into),
            Self::SectorRoot(start) => start.replay(family, context).map_err(Into::into),
        }
    }

    fn payload_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Residual(left), Self::Residual(right)) => left.payload_eq(right),
            (Self::SectorRoot(left), Self::SectorRoot(right)) => left.payload_eq(right),
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedCylindricalRowSystemLimits {
    pub specialization: PartialParametricRelationSpecializationLimits,
    pub max_source_rows: usize,
    pub max_prepare_points: usize,
    pub max_expanded_rows: usize,
    pub max_total_source_term_references: usize,
    pub max_total_translated_terms: usize,
    pub max_retained_rows: usize,
    pub max_unsatisfiable_rows: usize,
    pub max_total_base_assumptions: usize,
    pub max_total_specialization_source_terms: usize,
    pub max_total_specialization_output_terms: usize,
    pub max_total_specialization_power_operations: usize,
    pub max_total_specialization_integer_bit_work: usize,
    pub max_total_specialization_retained_terms: usize,
    /// Aggregate logical retained bytes reported by conditional
    /// specialization.  This is not a peak allocator-byte cap: while one row
    /// is committed, its shared source and specialized payload coexist.
    /// Callers must leave headroom for both logical payloads plus container
    /// overhead; the specialization certificate does not clone the source.
    pub max_total_specialization_retained_bytes: usize,
    pub max_derived_row_label_bytes: usize,
}

impl Default for GeneratedCylindricalRowSystemLimits {
    fn default() -> Self {
        Self {
            specialization: PartialParametricRelationSpecializationLimits::default(),
            max_source_rows: 100_000,
            max_prepare_points: 16_000_000,
            max_expanded_rows: 100_000_000,
            max_total_source_term_references: 1_000_000_000,
            max_total_translated_terms: 1_000_000_000,
            max_retained_rows: 100_000_000,
            max_unsatisfiable_rows: 100_000_000,
            max_total_base_assumptions: 100_000_000,
            max_total_specialization_source_terms: 10_000_000_000,
            max_total_specialization_output_terms: 10_000_000_000,
            max_total_specialization_power_operations: 100_000_000_000,
            max_total_specialization_integer_bit_work: 100_000_000_000,
            max_total_specialization_retained_terms: 10_000_000_000,
            max_total_specialization_retained_bytes: 8 * 1024 * 1024 * 1024,
            max_derived_row_label_bytes: 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedCylindricalRowSystemStats {
    source_rows: usize,
    prepare_points: usize,
    expanded_rows: usize,
    source_term_references: usize,
    translated_terms: usize,
    retained_rows: usize,
    unsatisfiable_rows: usize,
    base_assumptions: usize,
    specialization_source_terms: usize,
    specialization_output_terms: usize,
    specialization_power_operations: usize,
    specialization_integer_bit_work: usize,
    specialization_retained_terms: usize,
    specialization_retained_bytes: usize,
    derived_row_label_bytes: usize,
}

macro_rules! stats_getters {
    ($($field:ident),* $(,)?) => {$ (
        pub const fn $field(self) -> usize { self.$field }
    )* };
}

impl GeneratedCylindricalRowSystemStats {
    // Work counters are exact for retained rows.  Because the lower-level
    // partial-specialization API returns no work census with an
    // unsatisfiable-domain error, such a row is charged and reported at the
    // complete per-call allowance it received.  These counters are therefore
    // conservative upper bounds whenever `unsatisfiable_rows() != 0`.
    stats_getters!(
        source_rows,
        prepare_points,
        expanded_rows,
        source_term_references,
        translated_terms,
        retained_rows,
        unsatisfiable_rows,
        base_assumptions,
        specialization_source_terms,
        specialization_output_terms,
        specialization_power_operations,
        specialization_integer_bit_work,
        specialization_retained_terms,
        specialization_retained_bytes,
        derived_row_label_bytes,
    );
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedCylindricalSourceRowOutcome {
    Retained {
        retained_row_ordinal: usize,
        specialization: PartialParametricRelationSpecializationStats,
        base_assumptions: usize,
    },
    UnsatisfiableDomain,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedCylindricalSourceRowWitness {
    expanded_ordinal: usize,
    layer_ordinal: usize,
    depth: usize,
    prepare_point_ordinal: usize,
    source_row_ordinal: usize,
    outcome: GeneratedCylindricalSourceRowOutcome,
}

impl GeneratedCylindricalSourceRowWitness {
    pub const fn expanded_ordinal(&self) -> usize {
        self.expanded_ordinal
    }
    pub const fn layer_ordinal(&self) -> usize {
        self.layer_ordinal
    }
    pub const fn depth(&self) -> usize {
        self.depth
    }
    pub const fn prepare_point_ordinal(&self) -> usize {
        self.prepare_point_ordinal
    }
    pub const fn source_row_ordinal(&self) -> usize {
        self.source_row_ordinal
    }
    pub const fn outcome(&self) -> &GeneratedCylindricalSourceRowOutcome {
        &self.outcome
    }
}

#[derive(Clone, Debug)]
struct GeneratedCylindricalRetainedRow {
    expanded_ordinal: usize,
    specialization: Arc<PartialParametricRelationSpecialization>,
}

#[derive(Clone, Debug)]
pub struct GeneratedCylindricalRowSystemCertificate {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    start: GeneratedCylindricalRowSystemStartCertificate,
    witnesses: Vec<GeneratedCylindricalSourceRowWitness>,
    retained_rows: Vec<GeneratedCylindricalRetainedRow>,
    limits: GeneratedCylindricalRowSystemLimits,
    stats: GeneratedCylindricalRowSystemStats,
}

impl GeneratedCylindricalRowSystemCertificate {
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        start: Arc<GeneratedCylindricalResidualStartCertificate>,
        limits: GeneratedCylindricalRowSystemLimits,
    ) -> Result<Self, GeneratedCylindricalRowSystemError> {
        compile_inner(
            family,
            context,
            GeneratedCylindricalRowSystemStartCertificate::Residual(start),
            limits,
            true,
        )
    }

    /// Compile from an authenticated empty sector root. This path receives a
    /// distinct V2 row-system schema and never imports an anchored discovery.
    pub fn compile_from_sector_root(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        start: Arc<GeneratedCylindricalSectorRootStartCertificate>,
        limits: GeneratedCylindricalRowSystemLimits,
    ) -> Result<Self, GeneratedCylindricalRowSystemError> {
        compile_inner(
            family,
            context,
            GeneratedCylindricalRowSystemStartCertificate::SectorRoot(start),
            limits,
            true,
        )
    }

    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub(crate) const fn family_fingerprint_arc(&self) -> &Arc<str> {
        &self.family_fingerprint
    }
    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }
    pub(crate) const fn context_fingerprint_arc(&self) -> &Arc<str> {
        &self.context_fingerprint
    }
    pub const fn start(&self) -> &GeneratedCylindricalRowSystemStartCertificate {
        &self.start
    }
    pub fn witnesses(&self) -> &[GeneratedCylindricalSourceRowWitness] {
        &self.witnesses
    }
    pub const fn limits(&self) -> GeneratedCylindricalRowSystemLimits {
        self.limits
    }
    pub const fn stats(&self) -> GeneratedCylindricalRowSystemStats {
        self.stats
    }

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedCylindricalRowSystemError> {
        if self.schema != self.start.row_system_schema() {
            return Err(GeneratedCylindricalRowSystemError::SchemaMismatch);
        }
        let replayed = compile_inner(family, context, self.start.clone(), self.limits, false)?;
        if self.payload_eq(&replayed) {
            Ok(())
        } else {
            Err(GeneratedCylindricalRowSystemError::ReplayMismatch)
        }
    }

    pub(crate) fn prevalidated_relations(&self) -> impl Iterator<Item = &ParametricRelation> {
        self.retained_rows
            .iter()
            .map(|row| row.specialization.relation_for_bound_reelimination())
    }

    /// Private retained-row lookup for the next authenticated solver layer.
    ///
    /// The expanded ordinal is returned with the borrowed relation so that a
    /// consumer cannot detach algebra from the point-major witness that
    /// produced it.  No arbitrary relation can enter through this seam.
    pub(crate) fn prevalidated_relation(
        &self,
        retained_row_ordinal: usize,
    ) -> Option<(usize, &ParametricRelation)> {
        self.retained_rows.get(retained_row_ordinal).map(|row| {
            (
                row.expanded_ordinal,
                row.specialization.relation_for_bound_reelimination(),
            )
        })
    }

    /// Inseparable specialization payload for solver layers which must retain
    /// both the bound relation and its separately stored base-field
    /// assumptions. Cloning this `Arc` performs no deep relation/GMP clone.
    pub(crate) fn prevalidated_specialization(
        &self,
        retained_row_ordinal: usize,
    ) -> Option<(usize, &Arc<PartialParametricRelationSpecialization>)> {
        self.retained_rows
            .get(retained_row_ordinal)
            .map(|row| (row.expanded_ordinal, &row.specialization))
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.family_fingerprint == other.family_fingerprint
            && self.context_fingerprint == other.context_fingerprint
            && self.start.payload_eq(&other.start)
            && self.witnesses == other.witnesses
            && retained_rows_eq(&self.retained_rows, &other.retained_rows)
            && self.limits == other.limits
            && self.stats == other.stats
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedCylindricalRowSystemError {
    SchemaMismatch,
    ReplayMismatch,
    WrongFamily,
    WrongContext,
    IncompleteDependentSymbolicStart {
        unresolved_equality_predicates: usize,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    Start(GeneratedCylindricalResidualStartError),
    SectorRootStart(GeneratedCylindricalSectorRootStartError),
    Relation(ParametricRelationError),
}

impl fmt::Display for GeneratedCylindricalRowSystemError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("cylindrical row-system schema mismatch"),
            Self::ReplayMismatch => formatter.write_str("cylindrical row system does not replay"),
            Self::WrongFamily => {
                formatter.write_str("cylindrical row system belongs to another family")
            }
            Self::WrongContext => {
                formatter.write_str("cylindrical row system belongs to another K(n) context")
            }
            Self::IncompleteDependentSymbolicStart {
                unresolved_equality_predicates,
            } => write!(
                formatter,
                "cylindrical row system cannot compile an integer-cylinder start with {unresolved_equality_predicates} unresolved equality predicates"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "cylindrical row-system {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "cylindrical row-system {resource} count overflowed usize"
                )
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "cylindrical row-system {resource} could not reserve {requested} elements"
            ),
            Self::Start(error) => error.fmt(formatter),
            Self::SectorRootStart(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedCylindricalRowSystemError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Start(error) => Some(error),
            Self::SectorRootStart(error) => Some(error),
            Self::Relation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<GeneratedCylindricalResidualStartError> for GeneratedCylindricalRowSystemError {
    fn from(value: GeneratedCylindricalResidualStartError) -> Self {
        Self::Start(value)
    }
}
impl From<GeneratedCylindricalSectorRootStartError> for GeneratedCylindricalRowSystemError {
    fn from(value: GeneratedCylindricalSectorRootStartError) -> Self {
        Self::SectorRootStart(value)
    }
}
impl From<ParametricRelationError> for GeneratedCylindricalRowSystemError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}

fn compile_inner(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    start: GeneratedCylindricalRowSystemStartCertificate,
    limits: GeneratedCylindricalRowSystemLimits,
    replay_result: bool,
) -> Result<GeneratedCylindricalRowSystemCertificate, GeneratedCylindricalRowSystemError> {
    if start.family_fingerprint() != family.fingerprint() {
        return Err(GeneratedCylindricalRowSystemError::WrongFamily);
    }
    if start.context_fingerprint() != context.fingerprint() {
        return Err(GeneratedCylindricalRowSystemError::WrongContext);
    }
    start.replay(family, context)?;
    match start.completeness() {
        GeneratedCylindricalStartCompleteness::IndependentIntegerCylinder => {}
        GeneratedCylindricalStartCompleteness::DependentSymbolicStartPending {
            unresolved_equality_predicate_ordinals,
        } => {
            return Err(
                GeneratedCylindricalRowSystemError::IncompleteDependentSymbolicStart {
                    unresolved_equality_predicates: unresolved_equality_predicate_ordinals.len(),
                },
            );
        }
    }
    let row_span = start.row_span();
    let source_rows = row_span.rows();
    check_limit("source rows", source_rows.len(), limits.max_source_rows)?;
    let prepare_points = start.schedule().stats().retained_points();
    check_limit("prepare points", prepare_points, limits.max_prepare_points)?;
    let expanded_rows = checked_mul("expanded rows", prepare_points, source_rows.len())?;
    check_limit("expanded rows", expanded_rows, limits.max_expanded_rows)?;
    let source_terms = source_rows.iter().try_fold(0usize, |total, row| {
        checked_add("source terms", total, row.terms().len())
    })?;
    let source_term_references =
        checked_mul("source term references", prepare_points, source_terms)?;
    check_limit(
        "source term references",
        source_term_references,
        limits.max_total_source_term_references,
    )?;

    let mut stats = GeneratedCylindricalRowSystemStats {
        source_rows: source_rows.len(),
        prepare_points,
        expanded_rows,
        source_term_references,
        ..Default::default()
    };
    let mut witnesses = Vec::new();
    try_reserve_exact("expanded-row witnesses", &mut witnesses, expanded_rows)?;
    let mut retained_rows = Vec::new();
    let mut specialization_budget = SpecializationAggregateBudget::default();
    let mut expanded_ordinal = 0usize;
    for (layer_ordinal, layer) in start.schedule().layers().iter().enumerate() {
        for (prepare_point_ordinal, translation) in layer.ordered_translations().iter().enumerate()
        {
            for (source_row_ordinal, source) in source_rows.iter().enumerate() {
                let label_len = derived_row_label_len(
                    layer.depth(),
                    prepare_point_ordinal,
                    source_row_ordinal,
                )?;
                stats.derived_row_label_bytes = bounded_add(
                    "derived row label bytes",
                    stats.derived_row_label_bytes,
                    label_len,
                    limits.max_derived_row_label_bytes,
                )?;
                let mut label = String::new();
                try_reserve_string("derived row label bytes", &mut label, label_len)?;
                write!(
                    &mut label,
                    "cylindrical-d{}-p{}-r{}",
                    layer.depth(),
                    prepare_point_ordinal,
                    source_row_ordinal
                )
                .expect("writing into a reserved String is infallible");
                debug_assert_eq!(label.len(), label_len);

                // A common lattice translation is injective on sparse keys,
                // so it cannot increase the source term cardinality.  Charge
                // that cardinality before constructing the Symbolica row.
                let translated_terms_preflight = bounded_add(
                    "translated terms",
                    stats.translated_terms,
                    source.terms().len(),
                    limits.max_total_translated_terms,
                )?;
                let translated = source.translated(
                    context,
                    translation,
                    ParametricRowId::Derived {
                        label: label.into(),
                    },
                    limits.specialization.arithmetic,
                )?;
                if translated.terms().len() > source.terms().len() {
                    return Err(GeneratedCylindricalRowSystemError::ReplayMismatch);
                }
                stats.translated_terms = translated_terms_preflight
                    .checked_sub(source.terms().len() - translated.terms().len())
                    .ok_or(GeneratedCylindricalRowSystemError::ReplayMismatch)?;
                let effective_specialization =
                    specialization_budget.limits_for_remaining(limits.specialization, limits)?;
                let translated = Arc::new(translated);
                match translated.partially_specialized_on(
                    context,
                    start.assignment().clone(),
                    effective_specialization,
                ) {
                    Ok(specialized) => {
                        let specialization = specialized.stats();
                        specialization_budget.consume_stats(specialization, limits)?;
                        accumulate_specialization(&mut stats, specialization, limits)?;
                        stats.base_assumptions = bounded_add(
                            "base assumptions",
                            stats.base_assumptions,
                            specialized.base_assumptions().len(),
                            limits.max_total_base_assumptions,
                        )?;
                        stats.retained_rows = bounded_add(
                            "retained rows",
                            stats.retained_rows,
                            1,
                            limits.max_retained_rows,
                        )?;
                        let retained_row_ordinal = retained_rows.len();
                        try_reserve_exact("retained rows", &mut retained_rows, 1)?;
                        witnesses.push(GeneratedCylindricalSourceRowWitness {
                            expanded_ordinal,
                            layer_ordinal,
                            depth: layer.depth(),
                            prepare_point_ordinal,
                            source_row_ordinal,
                            outcome: GeneratedCylindricalSourceRowOutcome::Retained {
                                retained_row_ordinal,
                                specialization,
                                base_assumptions: specialized.base_assumptions().len(),
                            },
                        });
                        retained_rows.push(GeneratedCylindricalRetainedRow {
                            expanded_ordinal,
                            specialization: Arc::new(specialized),
                        });
                    }
                    Err(ParametricRelationError::UnsatisfiableDomain) => {
                        // This semantic outcome does not currently carry
                        // exact work statistics.  Consume its complete
                        // per-call allowance so aggregate work remains
                        // fail-closed for every later row.
                        specialization_budget.consume_limits(effective_specialization, limits)?;
                        accumulate_specialization_limits(
                            &mut stats,
                            effective_specialization,
                            limits,
                        )?;
                        stats.base_assumptions = bounded_add(
                            "base assumptions",
                            stats.base_assumptions,
                            effective_specialization.max_base_assumptions,
                            limits.max_total_base_assumptions,
                        )?;
                        stats.unsatisfiable_rows = bounded_add(
                            "unsatisfiable rows",
                            stats.unsatisfiable_rows,
                            1,
                            limits.max_unsatisfiable_rows,
                        )?;
                        witnesses.push(GeneratedCylindricalSourceRowWitness {
                            expanded_ordinal,
                            layer_ordinal,
                            depth: layer.depth(),
                            prepare_point_ordinal,
                            source_row_ordinal,
                            outcome: GeneratedCylindricalSourceRowOutcome::UnsatisfiableDomain,
                        });
                    }
                    Err(error) => return Err(error.into()),
                }
                expanded_ordinal = checked_add("expanded row ordinals", expanded_ordinal, 1)?;
            }
        }
    }
    if expanded_ordinal != expanded_rows || witnesses.len() != expanded_rows {
        return Err(GeneratedCylindricalRowSystemError::ReplayMismatch);
    }
    let result = GeneratedCylindricalRowSystemCertificate {
        schema: start.row_system_schema(),
        family_fingerprint: family.fingerprint().into(),
        context_fingerprint: context.fingerprint().into(),
        start,
        witnesses,
        retained_rows,
        limits,
        stats,
    };
    if replay_result {
        result.replay(family, context)?;
    }
    Ok(result)
}

fn accumulate_specialization(
    stats: &mut GeneratedCylindricalRowSystemStats,
    row: PartialParametricRelationSpecializationStats,
    limits: GeneratedCylindricalRowSystemLimits,
) -> Result<(), GeneratedCylindricalRowSystemError> {
    macro_rules! add {
        ($field:ident, $getter:ident, $name:literal, $limit:ident) => {
            stats.$field = bounded_add($name, stats.$field, row.$getter(), limits.$limit)?;
        };
    }
    add!(
        specialization_source_terms,
        source_terms,
        "specialization source terms",
        max_total_specialization_source_terms
    );
    add!(
        specialization_output_terms,
        output_terms,
        "specialization output terms",
        max_total_specialization_output_terms
    );
    add!(
        specialization_power_operations,
        power_operations,
        "specialization power operations",
        max_total_specialization_power_operations
    );
    add!(
        specialization_integer_bit_work,
        integer_bit_work,
        "specialization integer bit work",
        max_total_specialization_integer_bit_work
    );
    add!(
        specialization_retained_terms,
        retained_terms,
        "specialization retained terms",
        max_total_specialization_retained_terms
    );
    add!(
        specialization_retained_bytes,
        retained_bytes,
        "specialization retained bytes",
        max_total_specialization_retained_bytes
    );
    Ok(())
}

fn accumulate_specialization_limits(
    stats: &mut GeneratedCylindricalRowSystemStats,
    row: PartialParametricRelationSpecializationLimits,
    limits: GeneratedCylindricalRowSystemLimits,
) -> Result<(), GeneratedCylindricalRowSystemError> {
    macro_rules! add {
        ($field:ident, $value:ident, $name:literal, $limit:ident) => {
            stats.$field = bounded_add($name, stats.$field, row.$value, limits.$limit)?;
        };
    }
    add!(
        specialization_source_terms,
        max_source_terms,
        "specialization source terms",
        max_total_specialization_source_terms
    );
    add!(
        specialization_output_terms,
        max_output_terms,
        "specialization output terms",
        max_total_specialization_output_terms
    );
    add!(
        specialization_power_operations,
        max_power_operations,
        "specialization power operations",
        max_total_specialization_power_operations
    );
    add!(
        specialization_integer_bit_work,
        max_integer_bit_work,
        "specialization integer bit work",
        max_total_specialization_integer_bit_work
    );
    add!(
        specialization_retained_terms,
        max_retained_terms,
        "specialization retained terms",
        max_total_specialization_retained_terms
    );
    add!(
        specialization_retained_bytes,
        max_retained_bytes,
        "specialization retained bytes",
        max_total_specialization_retained_bytes
    );
    Ok(())
}

/// Aggregate allowance actually committed by specialization calls.  A
/// successful call consumes its exact work census.  An unsatisfiable-domain
/// result has no census in the current lower-level API, so it conservatively
/// consumes the complete allowance offered to that call.
#[derive(Clone, Copy, Debug, Default)]
struct SpecializationAggregateBudget {
    base_assumptions: usize,
    source_terms: usize,
    output_terms: usize,
    power_operations: usize,
    integer_bit_work: usize,
    retained_terms: usize,
    retained_bytes: usize,
}

impl SpecializationAggregateBudget {
    fn limits_for_remaining(
        self,
        mut per_call: PartialParametricRelationSpecializationLimits,
        aggregate: GeneratedCylindricalRowSystemLimits,
    ) -> Result<PartialParametricRelationSpecializationLimits, GeneratedCylindricalRowSystemError>
    {
        macro_rules! restrict {
            ($per_call:ident, $used:ident, $aggregate:ident, $name:literal) => {
                per_call.$per_call = per_call.$per_call.min(remaining_budget(
                    $name,
                    self.$used,
                    aggregate.$aggregate,
                )?);
            };
        }
        restrict!(
            max_base_assumptions,
            base_assumptions,
            max_total_base_assumptions,
            "base assumptions"
        );
        restrict!(
            max_source_terms,
            source_terms,
            max_total_specialization_source_terms,
            "specialization source terms"
        );
        restrict!(
            max_output_terms,
            output_terms,
            max_total_specialization_output_terms,
            "specialization output terms"
        );
        restrict!(
            max_power_operations,
            power_operations,
            max_total_specialization_power_operations,
            "specialization power operations"
        );
        restrict!(
            max_integer_bit_work,
            integer_bit_work,
            max_total_specialization_integer_bit_work,
            "specialization integer bit work"
        );
        restrict!(
            max_retained_terms,
            retained_terms,
            max_total_specialization_retained_terms,
            "specialization retained terms"
        );
        restrict!(
            max_retained_bytes,
            retained_bytes,
            max_total_specialization_retained_bytes,
            "specialization retained bytes"
        );
        Ok(per_call)
    }

    fn consume_stats(
        &mut self,
        row: PartialParametricRelationSpecializationStats,
        limits: GeneratedCylindricalRowSystemLimits,
    ) -> Result<(), GeneratedCylindricalRowSystemError> {
        self.consume(
            row.base_assumptions(),
            row.source_terms(),
            row.output_terms(),
            row.power_operations(),
            row.integer_bit_work(),
            row.retained_terms(),
            row.retained_bytes(),
            limits,
        )
    }

    fn consume_limits(
        &mut self,
        row: PartialParametricRelationSpecializationLimits,
        limits: GeneratedCylindricalRowSystemLimits,
    ) -> Result<(), GeneratedCylindricalRowSystemError> {
        self.consume(
            row.max_base_assumptions,
            row.max_source_terms,
            row.max_output_terms,
            row.max_power_operations,
            row.max_integer_bit_work,
            row.max_retained_terms,
            row.max_retained_bytes,
            limits,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn consume(
        &mut self,
        base_assumptions: usize,
        source_terms: usize,
        output_terms: usize,
        power_operations: usize,
        integer_bit_work: usize,
        retained_terms: usize,
        retained_bytes: usize,
        limits: GeneratedCylindricalRowSystemLimits,
    ) -> Result<(), GeneratedCylindricalRowSystemError> {
        macro_rules! charge {
            ($field:ident, $value:ident, $name:literal, $limit:ident) => {
                self.$field = bounded_add($name, self.$field, $value, limits.$limit)?;
            };
        }
        charge!(
            base_assumptions,
            base_assumptions,
            "base assumptions",
            max_total_base_assumptions
        );
        charge!(
            source_terms,
            source_terms,
            "specialization source terms",
            max_total_specialization_source_terms
        );
        charge!(
            output_terms,
            output_terms,
            "specialization output terms",
            max_total_specialization_output_terms
        );
        charge!(
            power_operations,
            power_operations,
            "specialization power operations",
            max_total_specialization_power_operations
        );
        charge!(
            integer_bit_work,
            integer_bit_work,
            "specialization integer bit work",
            max_total_specialization_integer_bit_work
        );
        charge!(
            retained_terms,
            retained_terms,
            "specialization retained terms",
            max_total_specialization_retained_terms
        );
        charge!(
            retained_bytes,
            retained_bytes,
            "specialization retained bytes",
            max_total_specialization_retained_bytes
        );
        Ok(())
    }
}

fn remaining_budget(
    resource: &'static str,
    used: usize,
    limit: usize,
) -> Result<usize, GeneratedCylindricalRowSystemError> {
    limit
        .checked_sub(used)
        .ok_or(GeneratedCylindricalRowSystemError::ResourceLimit {
            resource,
            requested: used,
            limit,
        })
}

fn derived_row_label_len(
    depth: usize,
    prepare_point_ordinal: usize,
    source_row_ordinal: usize,
) -> Result<usize, GeneratedCylindricalRowSystemError> {
    [
        "cylindrical-d".len(),
        decimal_digits(depth),
        "-p".len(),
        decimal_digits(prepare_point_ordinal),
        "-r".len(),
        decimal_digits(source_row_ordinal),
    ]
    .into_iter()
    .try_fold(0usize, |total, component| {
        checked_add("derived row label bytes", total, component)
    })
}

fn decimal_digits(mut value: usize) -> usize {
    let mut digits = 1usize;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn try_reserve_exact<T>(
    resource: &'static str,
    values: &mut Vec<T>,
    additional: usize,
) -> Result<(), GeneratedCylindricalRowSystemError> {
    let requested = checked_add(resource, values.len(), additional)?;
    values.try_reserve_exact(additional).map_err(|_| {
        GeneratedCylindricalRowSystemError::AllocationFailure {
            resource,
            requested,
        }
    })
}

fn try_reserve_string(
    resource: &'static str,
    value: &mut String,
    additional: usize,
) -> Result<(), GeneratedCylindricalRowSystemError> {
    let requested = checked_add(resource, value.len(), additional)?;
    value.try_reserve_exact(additional).map_err(|_| {
        GeneratedCylindricalRowSystemError::AllocationFailure {
            resource,
            requested,
        }
    })
}

fn retained_rows_eq(
    left: &[GeneratedCylindricalRetainedRow],
    right: &[GeneratedCylindricalRetainedRow],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.expanded_ordinal == right.expanded_ordinal
                && left.specialization.stats() == right.specialization.stats()
                && left.specialization.base_assumptions() == right.specialization.base_assumptions()
                && left
                    .specialization
                    .relation_for_bound_reelimination()
                    .has_identical_guard_provenance(
                        right.specialization.relation_for_bound_reelimination(),
                    )
        })
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedCylindricalRowSystemError> {
    left.checked_add(right)
        .ok_or(GeneratedCylindricalRowSystemError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedCylindricalRowSystemError> {
    left.checked_mul(right)
        .ok_or(GeneratedCylindricalRowSystemError::ResourceCountOverflow { resource })
}

fn bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, GeneratedCylindricalRowSystemError> {
    let requested = checked_add(resource, left, right)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedCylindricalRowSystemError> {
    if requested > limit {
        Err(GeneratedCylindricalRowSystemError::ResourceLimit {
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
    use crate::{
        AffineDenominator, BasePolynomial, CoefficientContext, FamilySectorInventoryCompiler,
        FamilySectorInventoryLimits, GeneratedCylindricalResidualStartLimits,
        GeneratedCylindricalSectorRootStartLimits, GeneratedSectorDiscoveryCompiler,
        GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCompiler,
        GeneratedSectorLiveLeafQueueLimits, GeneratedSymbolicRowSpanConfig, IntegralOrderingPolicy,
        ParametricArithmeticLimits, ParametricIbpConfig, ParametricIbpGenerator, PowerShiftPolicy,
        SectorMask, SectorRestrictions, SymbolicPolynomialPredicateKind,
    };

    fn massive_tadpole() -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        IntegralFamily::new(
            "generated-cylindrical-row-system-concrete-oracle",
            vec!["k".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                coefficients.parse("-m2").unwrap(),
                vec![coefficients.one()],
            )],
            Vec::new(),
            vec![coefficients.zero()],
        )
        .unwrap()
    }

    fn fixture() -> (
        IntegralFamily,
        ParametricCoefficientContext,
        GeneratedCylindricalRowSystemCertificate,
    ) {
        let family = massive_tadpole();
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_new([true]).unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            discovery_limits,
        )
        .unwrap();
        let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
        queue_limits.translation_radius = 0;
        queue_limits.max_translation_points = 1;
        let queue = Arc::new(
            GeneratedSectorLiveLeafQueueCompiler::compile(
                &family,
                &context,
                &discovery,
                queue_limits,
            )
            .unwrap(),
        );
        let item = queue
            .work_items()
            .iter()
            .find(|item| {
                !item.extraction().assignment().is_empty()
                    && !item
                        .extraction()
                        .unresolved_predicates()
                        .iter()
                        .any(|predicate| {
                            predicate.kind() == SymbolicPolynomialPredicateKind::EqualZero
                        })
            })
            .expect("the tadpole fixture must retain a literal independent equality start");
        let item_ordinal = item.ordinal();
        let start = Arc::new(
            GeneratedCylindricalResidualStartCertificate::compile(
                &family,
                &context,
                queue,
                item_ordinal,
                1,
                GeneratedCylindricalResidualStartLimits::default(),
            )
            .unwrap(),
        );
        assert!(start.completeness().is_complete_integer_cylinder());
        let certificate = GeneratedCylindricalRowSystemCertificate::compile(
            &family,
            &context,
            start,
            GeneratedCylindricalRowSystemLimits::default(),
        )
        .unwrap();
        (family, context, certificate)
    }

    fn sector_root_fixture() -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedCylindricalSectorRootStartCertificate>,
        GeneratedCylindricalRowSystemCertificate,
    ) {
        let family = massive_tadpole();
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let inventory = Arc::new(
            FamilySectorInventoryCompiler::compile(
                &family,
                SectorRestrictions::unrestricted(family.denominator_count()).unwrap(),
                PowerShiftPolicy::FormalGeneric,
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                FamilySectorInventoryLimits::default(),
            )
            .unwrap(),
        );
        let root = Arc::new(
            GeneratedCylindricalSectorRootStartCertificate::compile(
                &family,
                &context,
                inventory,
                SectorMask::try_new([true]).unwrap(),
                ParametricIbpConfig::default(),
                GeneratedSymbolicRowSpanConfig::default(),
                1,
                GeneratedCylindricalSectorRootStartLimits::default(),
            )
            .unwrap(),
        );
        let certificate = GeneratedCylindricalRowSystemCertificate::compile_from_sector_root(
            &family,
            &context,
            root.clone(),
            GeneratedCylindricalRowSystemLimits::default(),
        )
        .unwrap();
        (family, context, root, certificate)
    }

    fn insert_unique(polynomials: &mut Vec<BasePolynomial>, polynomial: BasePolynomial) {
        if !polynomials.iter().any(|existing| existing == &polynomial) {
            polynomials.push(polynomial);
        }
    }

    #[test]
    fn anchor_free_sector_root_compiles_and_replays_as_v2() {
        let (family, context, root, certificate) = sector_root_fixture();

        assert_eq!(
            certificate.schema(),
            GENERATED_CYLINDRICAL_ROW_SYSTEM_V2_SCHEMA
        );
        assert!(certificate.start().is_sector_root());
        assert!(certificate.start().assignment().is_empty());
        assert!(Arc::ptr_eq(
            certificate.start().sector_root_start().unwrap(),
            &root
        ));
        assert_eq!(certificate.start().row_span().rows().len(), 1);
        assert!(!certificate.witnesses().is_empty());
        certificate.replay(&family, &context).unwrap();

        let mut wrong_schema = certificate;
        wrong_schema.schema = GENERATED_CYLINDRICAL_ROW_SYSTEM_V1_SCHEMA;
        assert_eq!(
            wrong_schema.replay(&family, &context),
            Err(GeneratedCylindricalRowSystemError::SchemaMismatch)
        );
    }

    #[test]
    fn sector_root_start_row_span_arc_dispatch_retains_allocation_and_replay() {
        let (family, context, root, certificate) = sector_root_fixture();

        assert!(Arc::ptr_eq(
            certificate.start().row_span_arc(),
            root.row_span_arc(),
        ));
        assert!(std::ptr::eq(
            certificate.start().row_span(),
            certificate.start().row_span_arc().as_ref(),
        ));
        certificate.replay(&family, &context).unwrap();
    }

    #[test]
    fn legacy_residual_start_remains_v1_and_replays() {
        let (family, context, certificate) = fixture();
        assert_eq!(
            certificate.schema(),
            GENERATED_CYLINDRICAL_ROW_SYSTEM_V1_SCHEMA
        );
        assert!(matches!(
            certificate.start(),
            GeneratedCylindricalRowSystemStartCertificate::Residual(_)
        ));
        assert!(certificate.start().residual_start().is_some());
        assert!(certificate.start().sector_root_start().is_none());
        certificate.replay(&family, &context).unwrap();
    }

    #[test]
    fn residual_start_row_span_arc_dispatch_retains_allocation_and_replay() {
        let (family, context, certificate) = fixture();
        let residual = certificate
            .start()
            .residual_start()
            .expect("residual fixture must retain its residual start");

        assert!(Arc::ptr_eq(
            certificate.start().row_span_arc(),
            residual.row_span_arc(),
        ));
        assert!(Arc::ptr_eq(
            certificate.start().row_span_arc(),
            residual.source_queue().discovery().row_span_arc(),
        ));
        assert!(std::ptr::eq(
            certificate.start().row_span(),
            certificate.start().row_span_arc().as_ref(),
        ));
        certificate.replay(&family, &context).unwrap();
    }

    #[test]
    fn start_authority_arms_cannot_be_exchanged_across_schemas() {
        let (family, context, residual) = fixture();
        let (_, _, _, root) = sector_root_fixture();
        assert!(!residual.start.payload_eq(&root.start));

        let mut v2_with_residual = root.clone();
        v2_with_residual.start = residual.start.clone();
        assert_eq!(
            v2_with_residual.replay(&family, &context),
            Err(GeneratedCylindricalRowSystemError::SchemaMismatch)
        );

        let mut v1_with_root = residual;
        v1_with_root.start = root.start.clone();
        assert_eq!(
            v1_with_root.replay(&family, &context),
            Err(GeneratedCylindricalRowSystemError::SchemaMismatch)
        );
    }

    #[test]
    fn retained_rows_share_specialization_payloads_across_certificate_clones() {
        let (_, _, certificate) = fixture();
        let cloned = certificate.clone();
        assert!(!certificate.retained_rows.is_empty());
        assert_eq!(certificate.retained_rows.len(), cloned.retained_rows.len());

        for (retained, exposed) in certificate
            .retained_rows
            .iter()
            .zip(certificate.prevalidated_relations())
        {
            assert!(std::ptr::eq(
                retained.specialization.relation_for_bound_reelimination(),
                exposed,
            ));
        }

        for (retained, cloned_retained) in
            certificate.retained_rows.iter().zip(&cloned.retained_rows)
        {
            assert!(Arc::ptr_eq(
                &retained.specialization,
                &cloned_retained.specialization,
            ));
            assert!(std::ptr::eq(
                retained.specialization.relation_for_bound_reelimination(),
                cloned_retained
                    .specialization
                    .relation_for_bound_reelimination(),
            ));
        }
    }

    fn assert_translate_then_partial_specialization_matches_direct_concrete_oracle(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        certificate: &GeneratedCylindricalRowSystemCertificate,
    ) {
        let start = certificate.start();
        let source_rows = start.row_span().rows();
        let arithmetic = ParametricArithmeticLimits::default();
        let mut base_assignment = start
            .sector()
            .active_bits()
            .iter()
            .map(|active| if *active { 2 } else { -1 })
            .collect::<Vec<_>>();
        for &(position, value) in start.assignment().entries() {
            base_assignment[position] = value;
        }

        for witness in certificate.witnesses() {
            let translation = &start.schedule().layers()[witness.layer_ordinal()]
                .ordered_translations()[witness.prepare_point_ordinal()];
            let source = &source_rows[witness.source_row_ordinal()];
            let shifted_assignment = base_assignment
                .iter()
                .zip(translation.values())
                .enumerate()
                .map(|(position, (&value, &shift))| {
                    value
                        .checked_add(shift)
                        .unwrap_or_else(|| panic!("oracle index {position} overflowed"))
                })
                .collect::<Vec<_>>();
            let direct = source.specialize(context, &shifted_assignment, arithmetic);

            match witness.outcome() {
                GeneratedCylindricalSourceRowOutcome::UnsatisfiableDomain => {
                    assert!(matches!(
                        direct,
                        Err(ParametricRelationError::UnsatisfiableDomain)
                    ));
                }
                GeneratedCylindricalSourceRowOutcome::Retained {
                    retained_row_ordinal,
                    specialization,
                    base_assumptions,
                } => {
                    let retained = &certificate.retained_rows[*retained_row_ordinal];
                    assert_eq!(retained.expanded_ordinal, witness.expanded_ordinal());
                    assert_eq!(&retained.specialization.stats(), specialization);
                    assert_eq!(
                        retained.specialization.base_assumptions().len(),
                        *base_assumptions
                    );
                    let retained_relation =
                        retained.specialization.relation_for_bound_reelimination();
                    let expected_label = format!(
                        "cylindrical-d{}-p{}-r{}",
                        witness.depth(),
                        witness.prepare_point_ordinal(),
                        witness.source_row_ordinal()
                    );
                    assert!(matches!(
                        retained_relation.row_id(),
                        ParametricRowId::Derived { label } if label.as_ref() == expected_label
                    ));

                    let via_partial = retained_relation
                        .specialize(context, &base_assignment, arithmetic)
                        .unwrap();
                    let direct = direct.unwrap();
                    assert_eq!(via_partial.terms(), direct.terms());

                    let mut expected_conditions = Vec::new();
                    for polynomial in direct.nonzero_conditions() {
                        insert_unique(&mut expected_conditions, polynomial.clone());
                    }
                    let mut actual_conditions = Vec::new();
                    for polynomial in via_partial.nonzero_conditions() {
                        insert_unique(&mut actual_conditions, polynomial.clone());
                    }
                    for assumption in retained.specialization.base_assumptions() {
                        let specialized = context
                            .specialize_nonzero_condition(
                                assumption.condition(),
                                &base_assignment,
                                arithmetic,
                            )
                            .unwrap();
                        insert_unique(&mut actual_conditions, specialized.polynomial().clone());
                    }
                    assert_eq!(actual_conditions.len(), expected_conditions.len());
                    assert!(expected_conditions.iter().all(|expected| {
                        actual_conditions.iter().any(|actual| actual == expected)
                    }));
                }
            }
        }
        certificate.replay(family, context).unwrap();
    }

    #[test]
    fn residual_translate_then_partial_specialization_matches_direct_concrete_oracle() {
        let (family, context, certificate) = fixture();
        assert_translate_then_partial_specialization_matches_direct_concrete_oracle(
            &family,
            &context,
            &certificate,
        );
    }

    #[test]
    fn sector_root_v2_translate_specialize_preserves_guards_and_base_assumptions() {
        let (family, context, _, certificate) = sector_root_fixture();
        assert!(certificate.start().assignment().is_empty());
        assert_translate_then_partial_specialization_matches_direct_concrete_oracle(
            &family,
            &context,
            &certificate,
        );
    }

    #[test]
    fn replay_rejects_private_point_major_provenance_tampering() {
        let (family, context, certificate) = fixture();
        certificate.replay(&family, &context).unwrap();
        let mut tampered = certificate;
        tampered.witnesses[0].expanded_ordinal += 1;
        assert_eq!(
            tampered.replay(&family, &context),
            Err(GeneratedCylindricalRowSystemError::ReplayMismatch)
        );
    }
}
