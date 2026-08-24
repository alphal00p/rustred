//! Authenticated empty-cylinder starts at an unresolved family-sector root.
//!
//! This is the anchor-free start origin required for globally valid generated
//! candidates.  Its source is a replayed [`FamilySectorInventoryCertificate`]
//! entry classified as [`FamilySectorInventoryStatus::UnresolvedNoZeroCertificate`],
//! an actually empty [`PartialIndexAssignment`], and a freshly generated
//! symbolic IBP/LI row span.  No anchored discovery certificate or residual
//! live-leaf queue participates in this construction.
//!
//! The certificate proves only that its rows are global parametric identities
//! on one admissible sector root and that its prepare points follow the exact
//! cylindrical order.  It does not prove a pivot, rule, coverage result,
//! zero-sector statement, or master integral.

use std::fmt;
use std::mem::size_of;
use std::sync::Arc;

use crate::{
    CylindricalOrderingError, CylindricalOrderingLimits, CylindricalParametricEliminationOrdering,
    CylindricalPreparePointScheduleCertificate, CylindricalPreparePointScheduleError,
    CylindricalPreparePointScheduleLimits, FamilySectorInventoryCertificate,
    FamilySectorInventoryError, FamilySectorInventoryStatus, FullColumnRankWitness,
    GeneratedCylindricalStartCompleteness, GeneratedSymbolicRowSpanCertificate,
    GeneratedSymbolicRowSpanCompiler, GeneratedSymbolicRowSpanConfig,
    GeneratedSymbolicRowSpanError, IntegralFamily, IntegralOrderingPolicy,
    ParametricCoefficientContext, ParametricCoefficientError, ParametricIbpConfig,
    PartialIndexAssignment, PowerShiftPolicy, SectorExclusion, SectorFoundationError, SectorMask,
    SectorRestrictions, ZeroSectorCertificate, ZeroSectorError, ZeroSectorResource,
};

pub const GENERATED_CYLINDRICAL_SECTOR_ROOT_START_V1_SCHEMA: &str =
    "rustred-generated-cylindrical-sector-root-start-v1";

const SECTOR_ARITY_RESOURCE: &str = "sector-root arity";
const ASSIGNMENT_ENTRIES_RESOURCE: &str = "sector-root assignment entries";
const SECTOR_WITNESS_ENTRIES_RESOURCE: &str = "sector-root witness entries";
const ROW_SPAN_ROWS_RESOURCE: &str = "sector-root row-span rows";
const ROW_SPAN_TERMS_RESOURCE: &str = "sector-root row-span terms";
const ROW_SPAN_MANIFEST_BYTES_RESOURCE: &str = "sector-root row-span manifest bytes";
const ORDERING_MANIFEST_BYTES_RESOURCE: &str = "sector-root ordering manifest bytes";
const PREPARE_POINT_LAYERS_RESOURCE: &str = "sector-root prepare-point layers";
const PREPARE_POINTS_RESOURCE: &str = "sector-root prepare points";
const BINDING_BYTES_RESOURCE: &str = "sector-root binding bytes";
const REPLAY_COMPARISON_UNITS_RESOURCE: &str = "sector-root logical replay comparison units";

/// Root-level retained-payload and replay budgets.
///
/// `ordering`, `schedule`, and the row-span configuration retain their own
/// more detailed work limits. Any directly corresponding outer cap is
/// intersected into that child limit before construction, and the effective
/// child configuration is retained. These outer limits independently bound
/// the proof objects shared by the root certificate and the size of its
/// cross-certificate binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedCylindricalSectorRootStartLimits {
    pub ordering: CylindricalOrderingLimits,
    pub schedule: CylindricalPreparePointScheduleLimits,
    pub max_sector_arity: usize,
    pub max_assignment_entries: usize,
    pub max_sector_witness_entries: usize,
    pub max_row_span_rows: usize,
    pub max_row_span_terms: usize,
    pub max_row_span_manifest_bytes: usize,
    pub max_ordering_manifest_bytes: usize,
    pub max_prepare_point_layers: usize,
    pub max_prepare_points: usize,
    pub max_binding_bytes: usize,
    /// Maximum deterministic logical units in the root's shallow replay
    /// census.  The compatibility name is retained, but this is deliberately
    /// not a count or a bound on deep polynomial/GMP comparisons performed by
    /// the separately budgeted inventory and row-span replays.
    pub max_replay_comparisons: usize,
}

impl Default for GeneratedCylindricalSectorRootStartLimits {
    fn default() -> Self {
        Self {
            ordering: CylindricalOrderingLimits::default(),
            schedule: CylindricalPreparePointScheduleLimits::default(),
            max_sector_arity: 4_096,
            // A sector-root start is defined by an empty assignment.  A zero
            // default therefore expresses the exact retained-payload budget.
            max_assignment_entries: 0,
            max_sector_witness_entries: 16_384,
            max_row_span_rows: 10_000_000,
            max_row_span_terms: 64_000_000,
            max_row_span_manifest_bytes: 2 * 1024 * 1024 * 1024,
            max_ordering_manifest_bytes: 16 * 1024 * 1024,
            max_prepare_point_layers: 65,
            max_prepare_points: 16_000_000,
            max_binding_bytes: 2 * 1024 * 1024 * 1024,
            max_replay_comparisons: 1_000_000_000,
        }
    }
}

/// Exact logical census of the shared source, empty cylinder, schedule, and
/// shallow root binding retained by one sector-root start.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedCylindricalSectorRootStartStats {
    sector_arity: usize,
    assignment_entries: usize,
    free_positions: usize,
    sector_witness_entries: usize,
    row_span_rows: usize,
    row_span_terms: usize,
    row_span_manifest_bytes: usize,
    ordering_manifest_bytes: usize,
    prepare_point_layers: usize,
    prepare_points: usize,
    binding_bytes: usize,
    replay_comparisons: usize,
}

impl GeneratedCylindricalSectorRootStartStats {
    pub const fn sector_arity(self) -> usize {
        self.sector_arity
    }
    pub const fn assignment_entries(self) -> usize {
        self.assignment_entries
    }
    pub const fn free_positions(self) -> usize {
        self.free_positions
    }
    pub const fn sector_witness_entries(self) -> usize {
        self.sector_witness_entries
    }
    pub const fn row_span_rows(self) -> usize {
        self.row_span_rows
    }
    pub const fn row_span_terms(self) -> usize {
        self.row_span_terms
    }
    pub const fn row_span_manifest_bytes(self) -> usize {
        self.row_span_manifest_bytes
    }
    pub const fn ordering_manifest_bytes(self) -> usize {
        self.ordering_manifest_bytes
    }
    pub const fn prepare_point_layers(self) -> usize {
        self.prepare_point_layers
    }
    pub const fn prepare_points(self) -> usize {
        self.prepare_points
    }
    pub const fn binding_bytes(self) -> usize {
        self.binding_bytes
    }
    /// Compatibility-named deterministic logical replay units. This is not a
    /// count of nested polynomial/GMP equality operations.
    pub const fn replay_comparisons(self) -> usize {
        self.replay_comparisons
    }
}

/// Replayable global cylindrical start for one unresolved sector root.
#[derive(Clone, Debug)]
pub struct GeneratedCylindricalSectorRootStartCertificate {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    inventory: Arc<FamilySectorInventoryCertificate>,
    restrictions: SectorRestrictions,
    power_shift_policy: PowerShiftPolicy,
    sector: SectorMask,
    sector_witness: FullColumnRankWitness,
    ordering_policy: IntegralOrderingPolicy,
    ibp: ParametricIbpConfig,
    row_span_config: GeneratedSymbolicRowSpanConfig,
    row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    assignment: PartialIndexAssignment,
    completeness: GeneratedCylindricalStartCompleteness,
    schedule: CylindricalPreparePointScheduleCertificate,
    limits: GeneratedCylindricalSectorRootStartLimits,
    stats: GeneratedCylindricalSectorRootStartStats,
}

impl GeneratedCylindricalSectorRootStartCertificate {
    /// Compile an authenticated sector-root start.
    ///
    /// The row span is deliberately generated inside this acceptance path.
    /// In particular, callers cannot supply an anchored discovery's row span.
    /// `VerifiedInputs` remains unavailable through this method because the
    /// generic [`GeneratedSymbolicRowSpanCompiler::compile`] path requires an
    /// explicit future proof-bearing constructor for those inputs.
    #[allow(clippy::too_many_arguments)]
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        inventory: Arc<FamilySectorInventoryCertificate>,
        sector: SectorMask,
        ibp: ParametricIbpConfig,
        row_span_config: GeneratedSymbolicRowSpanConfig,
        through_depth: usize,
        limits: GeneratedCylindricalSectorRootStartLimits,
    ) -> Result<Self, GeneratedCylindricalSectorRootStartError> {
        let (row_span_config, limits) =
            effective_child_configuration(row_span_config, through_depth, limits)?;
        validate_compile_scope(family, context, &inventory, &sector, limits)?;
        inventory.replay(family)?;
        preflight_sector_witness(unresolved_sector_witness(&inventory, &sector)?, limits)?;

        // This is the only row-span construction accepted here: fresh generic
        // IBP/LI generation, followed by its own exact replay.
        let row_span = Arc::new(GeneratedSymbolicRowSpanCompiler::compile(
            family,
            context,
            ibp,
            row_span_config,
        )?);
        row_span.replay(family, context)?;

        let certificate = reconstruct_with_replayed_sources(
            family,
            context,
            inventory,
            sector,
            ibp,
            row_span_config,
            row_span,
            through_depth,
            limits,
        )?;
        Ok(certificate)
    }

    /// Compile an authenticated sector-root start from an already generated,
    /// replayable family row span.
    ///
    /// This is the multi-sector family path: every sector root may retain the
    /// same exact [`Arc`] instead of regenerating the family-wide IBP/LI and
    /// symmetry basis.  The supplied inventory and row span are still replayed
    /// at their owning proof boundaries before the root is accepted.  A row
    /// span whose retained child limits are looser than the effective root caps
    /// is rejected rather than silently relabelled with stricter limits.
    #[allow(clippy::too_many_arguments)]
    pub fn compile_with_replayed_inventory_and_row_span(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        inventory: Arc<FamilySectorInventoryCertificate>,
        sector: SectorMask,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        through_depth: usize,
        limits: GeneratedCylindricalSectorRootStartLimits,
    ) -> Result<Self, GeneratedCylindricalSectorRootStartError> {
        let row_span_config = row_span.config();
        let (effective_row_span_config, limits) =
            effective_child_configuration(row_span_config, through_depth, limits)?;
        if effective_row_span_config != row_span_config {
            return Err(
                GeneratedCylindricalSectorRootStartError::ReplayedRowSpanConfigurationMismatch,
            );
        }

        validate_compile_scope(family, context, &inventory, &sector, limits)?;
        if row_span.family_fingerprint() != family.fingerprint_ref() {
            return Err(GeneratedCylindricalSectorRootStartError::WrongFamily);
        }
        if row_span.context_fingerprint() != context.fingerprint() {
            return Err(GeneratedCylindricalSectorRootStartError::WrongContext);
        }
        preflight_sector_witness(unresolved_sector_witness(&inventory, &sector)?, limits)?;

        // Reconstruct and validate the complete shallow root census before a
        // child certificate may enter its independently budgeted deep replay.
        // This makes tiny root replay/binding caps effective preflight gates
        // even when the shared sources themselves have large proof payloads.
        let certificate = reconstruct_with_replayed_sources(
            family,
            context,
            inventory,
            sector,
            row_span.ibp_config(),
            row_span_config,
            row_span,
            through_depth,
            limits,
        )?;
        certificate.inventory.replay(family)?;
        certificate.row_span.replay(family, context)?;
        Ok(certificate)
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
    pub fn inventory(&self) -> &FamilySectorInventoryCertificate {
        self.inventory.as_ref()
    }
    pub const fn inventory_arc(&self) -> &Arc<FamilySectorInventoryCertificate> {
        &self.inventory
    }
    pub const fn restrictions(&self) -> &SectorRestrictions {
        &self.restrictions
    }
    pub const fn power_shift_policy(&self) -> PowerShiftPolicy {
        self.power_shift_policy
    }
    pub const fn sector(&self) -> &SectorMask {
        &self.sector
    }
    pub const fn sector_witness(&self) -> &FullColumnRankWitness {
        &self.sector_witness
    }
    pub const fn ordering_policy(&self) -> IntegralOrderingPolicy {
        self.ordering_policy
    }
    pub const fn ibp_config(&self) -> ParametricIbpConfig {
        self.ibp
    }
    pub const fn row_span_config(&self) -> GeneratedSymbolicRowSpanConfig {
        self.row_span_config
    }
    pub fn row_span(&self) -> &GeneratedSymbolicRowSpanCertificate {
        self.row_span.as_ref()
    }
    pub const fn row_span_arc(&self) -> &Arc<GeneratedSymbolicRowSpanCertificate> {
        &self.row_span
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
    pub const fn limits(&self) -> GeneratedCylindricalSectorRootStartLimits {
        self.limits
    }
    pub const fn stats(&self) -> GeneratedCylindricalSectorRootStartStats {
        self.stats
    }

    /// Preflight the shallow logical replay census, replay every owning source
    /// at its own exact proof boundary, reconstruct the root deterministically,
    /// and compare the complete typed payload.
    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedCylindricalSectorRootStartError> {
        if self.schema != GENERATED_CYLINDRICAL_SECTOR_ROOT_START_V1_SCHEMA {
            return Err(GeneratedCylindricalSectorRootStartError::SchemaMismatch);
        }
        // This validates the compatibility-named logical comparison-unit cap
        // before inventory or row-span replay enters nested algebra.
        self.verify_structural_payload(family, context)?;
        self.inventory.replay(family)?;
        self.row_span.replay(family, context)?;
        let replayed = reconstruct_with_replayed_sources(
            family,
            context,
            self.inventory.clone(),
            self.sector.clone(),
            self.ibp,
            self.row_span_config,
            self.row_span.clone(),
            self.schedule.through_depth(),
            self.limits,
        )?;
        if self.payload_eq(&replayed) {
            Ok(())
        } else {
            Err(GeneratedCylindricalSectorRootStartError::ReplayMismatch)
        }
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.family_fingerprint == other.family_fingerprint
            && self.context_fingerprint == other.context_fingerprint
            && (Arc::ptr_eq(&self.inventory, &other.inventory)
                || self.inventory.payload_eq(&other.inventory))
            && self.restrictions == other.restrictions
            && self.power_shift_policy == other.power_shift_policy
            && self.sector == other.sector
            && self.sector_witness == other.sector_witness
            && self.ordering_policy == other.ordering_policy
            && self.ibp == other.ibp
            && self.row_span_config == other.row_span_config
            && (Arc::ptr_eq(&self.row_span, &other.row_span)
                || self.row_span.payload_eq(&other.row_span))
            && self.assignment == other.assignment
            && self.completeness == other.completeness
            && self.schedule == other.schedule
            && self.limits == other.limits
            && self.stats == other.stats
    }

    fn verify_structural_payload(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedCylindricalSectorRootStartError> {
        let (effective_row_span_config, effective_limits) = effective_child_configuration(
            self.row_span_config,
            self.schedule.through_depth(),
            self.limits,
        )?;
        if effective_row_span_config != self.row_span_config || effective_limits != self.limits {
            return Err(GeneratedCylindricalSectorRootStartError::ReplayMismatch);
        }
        if self.family_fingerprint.as_ref() != family.fingerprint_ref()
            || self.inventory.family_fingerprint() != family.fingerprint_ref()
            || self.row_span.family_fingerprint() != family.fingerprint_ref()
        {
            return Err(GeneratedCylindricalSectorRootStartError::WrongFamily);
        }
        if self.context_fingerprint.as_ref() != context.fingerprint()
            || self.row_span.context_fingerprint() != context.fingerprint()
            || !family
                .coefficient_context()
                .has_same_variable_map(context.base())
        {
            return Err(GeneratedCylindricalSectorRootStartError::WrongContext);
        }
        if family.denominator_count() != context.index_count() {
            return Err(
                GeneratedCylindricalSectorRootStartError::WrongContextArity {
                    expected: family.denominator_count(),
                    actual: context.index_count(),
                },
            );
        }
        if self.sector.arity() != family.denominator_count() {
            return Err(GeneratedCylindricalSectorRootStartError::WrongSectorArity {
                expected: family.denominator_count(),
                actual: self.sector.arity(),
            });
        }
        if self.restrictions.arity() != family.denominator_count()
            || self.inventory.restrictions().arity() != family.denominator_count()
        {
            return Err(
                GeneratedCylindricalSectorRootStartError::WrongRestrictionsArity {
                    expected: family.denominator_count(),
                    actual: self.restrictions.arity(),
                },
            );
        }
        if self.restrictions != *self.inventory.restrictions()
            || self.power_shift_policy != self.inventory.power_shift_policy()
            || self.ordering_policy != self.inventory.ordering()
            || self.ibp != self.row_span.ibp_config()
            || self.row_span_config != self.row_span.config()
            || self.assignment.arity() != self.sector.arity()
            || !self.assignment.is_empty()
            || !self.completeness.is_complete_integer_cylinder()
            || self.schedule.ordering().policy() != self.ordering_policy
            || self.schedule.ordering().sector() != &self.sector
            || self.schedule.ordering().assignment() != &self.assignment
            || self.schedule.ordering().limits() != self.limits.ordering
            || self.schedule.limits() != self.limits.schedule
        {
            return Err(GeneratedCylindricalSectorRootStartError::ReplayMismatch);
        }
        let witness = unresolved_sector_witness(&self.inventory, &self.sector)?;
        if witness != &self.sector_witness
            || witness.raw_sector() != &self.sector
            || witness.effective_sector().arity() != self.sector.arity()
        {
            return Err(GeneratedCylindricalSectorRootStartError::ReplayMismatch);
        }
        let recomputed = compute_stats(
            family,
            context,
            &self.inventory,
            &self.restrictions,
            self.power_shift_policy,
            &self.sector,
            &self.sector_witness,
            self.ordering_policy,
            self.ibp,
            self.row_span_config,
            &self.row_span,
            &self.assignment,
            &self.schedule,
        )?;
        if recomputed != self.stats {
            return Err(GeneratedCylindricalSectorRootStartError::ReplayMismatch);
        }
        validate_stats(recomputed, self.limits)
    }
}

/// Intersect every root-level cap which has a direct child equivalent before
/// that child can generate, replay, or retain its payload.  `min` preserves a
/// tighter caller-supplied child limit; the returned effective configuration is
/// the one persisted by the certificate and therefore the one used on replay.
pub(crate) fn effective_child_configuration(
    mut row_span_config: GeneratedSymbolicRowSpanConfig,
    through_depth: usize,
    mut limits: GeneratedCylindricalSectorRootStartLimits,
) -> Result<
    (
        GeneratedSymbolicRowSpanConfig,
        GeneratedCylindricalSectorRootStartLimits,
    ),
    GeneratedCylindricalSectorRootStartError,
> {
    let layer_count = through_depth.checked_add(1).ok_or(
        GeneratedCylindricalSectorRootStartError::ResourceCountOverflow {
            resource: PREPARE_POINT_LAYERS_RESOURCE,
        },
    )?;
    check_limit(
        PREPARE_POINT_LAYERS_RESOURCE,
        layer_count,
        limits.max_prepare_point_layers,
    )?;

    row_span_config.limits.max_canonical_rows = row_span_config
        .limits
        .max_canonical_rows
        .min(limits.max_row_span_rows);
    row_span_config.limits.max_augmented_rows = row_span_config
        .limits
        .max_augmented_rows
        .min(limits.max_row_span_rows);
    row_span_config.limits.max_canonical_terms = row_span_config
        .limits
        .max_canonical_terms
        .min(limits.max_row_span_terms);
    row_span_config.limits.max_augmented_terms = row_span_config
        .limits
        .max_augmented_terms
        .min(limits.max_row_span_terms);
    row_span_config.limits.max_aggregate_manifest_bytes = row_span_config
        .limits
        .max_aggregate_manifest_bytes
        .min(limits.max_row_span_manifest_bytes);

    limits.ordering.max_manifest_bytes = limits
        .ordering
        .max_manifest_bytes
        .min(limits.max_ordering_manifest_bytes);
    // `layer_count > 0` was established above, so the outer depth ceiling has
    // the exact `layers - 1` representation used by the child schedule.
    limits.schedule.max_depth = limits
        .schedule
        .max_depth
        .min(limits.max_prepare_point_layers.saturating_sub(1));
    limits.schedule.max_retained_points = limits
        .schedule
        .max_retained_points
        .min(limits.max_prepare_points);

    Ok((row_span_config, limits))
}

#[allow(clippy::too_many_arguments)]
fn reconstruct_with_replayed_sources(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    inventory: Arc<FamilySectorInventoryCertificate>,
    sector: SectorMask,
    ibp: ParametricIbpConfig,
    row_span_config: GeneratedSymbolicRowSpanConfig,
    row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    through_depth: usize,
    limits: GeneratedCylindricalSectorRootStartLimits,
) -> Result<GeneratedCylindricalSectorRootStartCertificate, GeneratedCylindricalSectorRootStartError>
{
    let (effective_row_span_config, effective_limits) =
        effective_child_configuration(row_span_config, through_depth, limits)?;
    if effective_row_span_config != row_span_config || effective_limits != limits {
        return Err(GeneratedCylindricalSectorRootStartError::ReplayMismatch);
    }
    validate_compile_scope(family, context, &inventory, &sector, limits)?;
    let sector_witness = {
        let witness = unresolved_sector_witness(&inventory, &sector)?;
        preflight_sector_witness(witness, limits)?;
        witness.clone()
    };

    // Use an explicit zero retention allowance as well as an empty input
    // iterator. No later mutation can turn this into a residual locus.
    let assignment =
        PartialIndexAssignment::try_new(std::iter::empty::<(usize, i64)>(), sector.arity(), 0)?;
    let ordering_policy = inventory.ordering();
    let ordering = CylindricalParametricEliminationOrdering::try_new(
        ordering_policy,
        sector.clone(),
        assignment.clone(),
        limits.ordering,
    )?;
    let schedule = CylindricalPreparePointScheduleCertificate::compile(
        ordering,
        through_depth,
        limits.schedule,
    )?;
    let restrictions = inventory.restrictions().clone();
    let power_shift_policy = inventory.power_shift_policy();
    let completeness = GeneratedCylindricalStartCompleteness::IndependentIntegerCylinder;
    let stats = compute_stats(
        family,
        context,
        &inventory,
        &restrictions,
        power_shift_policy,
        &sector,
        &sector_witness,
        ordering_policy,
        ibp,
        row_span_config,
        &row_span,
        &assignment,
        &schedule,
    )?;
    validate_stats(stats, limits)?;

    let certificate = GeneratedCylindricalSectorRootStartCertificate {
        schema: GENERATED_CYLINDRICAL_SECTOR_ROOT_START_V1_SCHEMA,
        family_fingerprint: family.fingerprint_ref().into(),
        context_fingerprint: context.fingerprint().into(),
        inventory,
        restrictions,
        power_shift_policy,
        sector,
        sector_witness,
        ordering_policy,
        ibp,
        row_span_config,
        row_span,
        assignment,
        completeness,
        schedule,
        limits,
        stats,
    };
    certificate.verify_structural_payload(family, context)?;
    Ok(certificate)
}

fn validate_compile_scope(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    inventory: &FamilySectorInventoryCertificate,
    sector: &SectorMask,
    limits: GeneratedCylindricalSectorRootStartLimits,
) -> Result<(), GeneratedCylindricalSectorRootStartError> {
    if inventory.family_fingerprint() != family.fingerprint_ref() {
        return Err(GeneratedCylindricalSectorRootStartError::WrongFamily);
    }
    if !family
        .coefficient_context()
        .has_same_variable_map(context.base())
    {
        return Err(GeneratedCylindricalSectorRootStartError::WrongContext);
    }
    if family.denominator_count() != context.index_count() {
        return Err(
            GeneratedCylindricalSectorRootStartError::WrongContextArity {
                expected: family.denominator_count(),
                actual: context.index_count(),
            },
        );
    }
    if sector.arity() != family.denominator_count() {
        return Err(GeneratedCylindricalSectorRootStartError::WrongSectorArity {
            expected: family.denominator_count(),
            actual: sector.arity(),
        });
    }
    if inventory.restrictions().arity() != family.denominator_count() {
        return Err(
            GeneratedCylindricalSectorRootStartError::WrongRestrictionsArity {
                expected: family.denominator_count(),
                actual: inventory.restrictions().arity(),
            },
        );
    }
    check_limit(
        SECTOR_ARITY_RESOURCE,
        sector.arity(),
        limits.max_sector_arity,
    )?;
    check_limit(
        ASSIGNMENT_ENTRIES_RESOURCE,
        0,
        limits.max_assignment_entries,
    )
}

fn unresolved_sector_witness<'a>(
    inventory: &'a FamilySectorInventoryCertificate,
    sector: &SectorMask,
) -> Result<&'a FullColumnRankWitness, GeneratedCylindricalSectorRootStartError> {
    match inventory.status(sector) {
        Some(FamilySectorInventoryStatus::UnresolvedNoZeroCertificate(witness)) => Ok(witness),
        Some(FamilySectorInventoryStatus::Excluded(exclusion)) => {
            Err(GeneratedCylindricalSectorRootStartError::SourceSectorExcluded(exclusion.clone()))
        }
        Some(FamilySectorInventoryStatus::ProvedZero(certificate)) => Err(
            GeneratedCylindricalSectorRootStartError::SourceSectorProvedZero(certificate.clone()),
        ),
        Some(FamilySectorInventoryStatus::ResourceLimited(resource)) => Err(
            GeneratedCylindricalSectorRootStartError::SourceSectorResourceLimited(resource.clone()),
        ),
        Some(FamilySectorInventoryStatus::Failed(error)) => {
            Err(GeneratedCylindricalSectorRootStartError::SourceSectorAnalysisFailed(error.clone()))
        }
        None => Err(GeneratedCylindricalSectorRootStartError::SourceSectorMissing),
    }
}

fn sector_witness_entry_count(
    sector_witness: &FullColumnRankWitness,
) -> Result<usize, GeneratedCylindricalSectorRootStartError> {
    checked_sum(
        SECTOR_WITNESS_ENTRIES_RESOURCE,
        [
            sector_witness.raw_sector().arity(),
            sector_witness.effective_sector().arity(),
            sector_witness.active_parameter_order().len(),
        ],
    )
}

fn preflight_sector_witness(
    sector_witness: &FullColumnRankWitness,
    limits: GeneratedCylindricalSectorRootStartLimits,
) -> Result<(), GeneratedCylindricalSectorRootStartError> {
    check_limit(
        SECTOR_WITNESS_ENTRIES_RESOURCE,
        sector_witness_entry_count(sector_witness)?,
        limits.max_sector_witness_entries,
    )
}

#[allow(clippy::too_many_arguments)]
fn compute_stats(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    inventory: &FamilySectorInventoryCertificate,
    restrictions: &SectorRestrictions,
    power_shift_policy: PowerShiftPolicy,
    sector: &SectorMask,
    sector_witness: &FullColumnRankWitness,
    ordering_policy: IntegralOrderingPolicy,
    ibp: ParametricIbpConfig,
    row_span_config: GeneratedSymbolicRowSpanConfig,
    row_span: &GeneratedSymbolicRowSpanCertificate,
    assignment: &PartialIndexAssignment,
    schedule: &CylindricalPreparePointScheduleCertificate,
) -> Result<GeneratedCylindricalSectorRootStartStats, GeneratedCylindricalSectorRootStartError> {
    let sector_witness_entries = sector_witness_entry_count(sector_witness)?;
    let binding_bytes = binding_bytes(
        family,
        context,
        inventory,
        restrictions,
        power_shift_policy,
        sector,
        sector_witness,
        ordering_policy,
        ibp,
        row_span_config,
        row_span,
        schedule,
    )?;
    let replay_comparisons =
        logical_replay_comparison_units(inventory, sector_witness_entries, row_span, schedule)?;
    Ok(GeneratedCylindricalSectorRootStartStats {
        sector_arity: sector.arity(),
        assignment_entries: assignment.entries().len(),
        free_positions: schedule.ordering().free_positions().len(),
        sector_witness_entries,
        row_span_rows: row_span.stats().augmented_rows(),
        row_span_terms: row_span.stats().augmented_terms(),
        row_span_manifest_bytes: row_span.stats().aggregate_manifest_bytes(),
        ordering_manifest_bytes: schedule.ordering().stable_manifest().len(),
        prepare_point_layers: schedule.stats().layer_count(),
        prepare_points: schedule.stats().retained_points(),
        binding_bytes,
        replay_comparisons,
    })
}

#[allow(clippy::too_many_arguments)]
fn binding_bytes(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    inventory: &FamilySectorInventoryCertificate,
    restrictions: &SectorRestrictions,
    _power_shift_policy: PowerShiftPolicy,
    sector: &SectorMask,
    sector_witness: &FullColumnRankWitness,
    ordering_policy: IntegralOrderingPolicy,
    _ibp: ParametricIbpConfig,
    _row_span_config: GeneratedSymbolicRowSpanConfig,
    row_span: &GeneratedSymbolicRowSpanCertificate,
    schedule: &CylindricalPreparePointScheduleCertificate,
) -> Result<usize, GeneratedCylindricalSectorRootStartError> {
    // Fixed-size typed configurations and rank scalars are charged by their
    // in-memory scalar footprint; variable identities are charged by their
    // exact retained or stable-manifest byte lengths.
    let witness_scalar_bytes = checked_mul(
        BINDING_BYTES_RESOURCE,
        size_of::<usize>(),
        checked_sum(
            BINDING_BYTES_RESOURCE,
            [sector_witness.active_parameter_order().len(), 3],
        )?,
    )?;
    let typed_config_bytes = checked_sum(
        BINDING_BYTES_RESOURCE,
        [
            size_of::<ParametricIbpConfig>(),
            size_of::<GeneratedSymbolicRowSpanConfig>(),
        ],
    )?;
    checked_sum(
        BINDING_BYTES_RESOURCE,
        [
            GENERATED_CYLINDRICAL_SECTOR_ROOT_START_V1_SCHEMA.len(),
            family.fingerprint_ref().len(),
            context.fingerprint().len(),
            inventory.schema().len(),
            inventory.family_fingerprint().len(),
            inventory.symanzik_g_fingerprint().len(),
            restrictions.cuts().arity(),
            restrictions.pattern().arity(),
            inventory.power_shift_policy_id().len(),
            inventory.power_support().arity(),
            sector.arity(),
            sector_witness.raw_sector().arity(),
            sector_witness.effective_sector().arity(),
            witness_scalar_bytes,
            ordering_policy.stable_id().len(),
            typed_config_bytes,
            row_span.schema().len(),
            row_span.family_fingerprint().len(),
            row_span.context_fingerprint().len(),
            row_span.stats().aggregate_manifest_bytes(),
            schedule.schema().len(),
            schedule.ordering().stable_manifest().len(),
        ],
    )
}

/// Deterministic, shallow logical units used to cap the root-level binding
/// census before deep source replay. One unit represents one retained item or
/// one fixed root field; it intentionally does not model nested polynomial,
/// GMP, manifest, or child-certificate equality operations.
fn logical_replay_comparison_units(
    inventory: &FamilySectorInventoryCertificate,
    sector_witness_entries: usize,
    row_span: &GeneratedSymbolicRowSpanCertificate,
    schedule: &CylindricalPreparePointScheduleCertificate,
) -> Result<usize, GeneratedCylindricalSectorRootStartError> {
    const FIXED_ROOT_FIELDS: usize = 16;
    checked_sum(
        REPLAY_COMPARISON_UNITS_RESOURCE,
        [
            FIXED_ROOT_FIELDS,
            inventory.entries().len(),
            inventory.unresolved_solve_order().len(),
            sector_witness_entries,
            row_span.symmetries().len(),
            row_span.rows().len(),
            row_span.lineages().len(),
            row_span.stats().augmented_terms(),
            schedule.layers().len(),
            schedule.stats().retained_points(),
        ],
    )
}

fn validate_stats(
    stats: GeneratedCylindricalSectorRootStartStats,
    limits: GeneratedCylindricalSectorRootStartLimits,
) -> Result<(), GeneratedCylindricalSectorRootStartError> {
    check_limit(
        SECTOR_ARITY_RESOURCE,
        stats.sector_arity,
        limits.max_sector_arity,
    )?;
    check_limit(
        ASSIGNMENT_ENTRIES_RESOURCE,
        stats.assignment_entries,
        limits.max_assignment_entries,
    )?;
    check_limit(
        SECTOR_WITNESS_ENTRIES_RESOURCE,
        stats.sector_witness_entries,
        limits.max_sector_witness_entries,
    )?;
    check_limit(
        ROW_SPAN_ROWS_RESOURCE,
        stats.row_span_rows,
        limits.max_row_span_rows,
    )?;
    check_limit(
        ROW_SPAN_TERMS_RESOURCE,
        stats.row_span_terms,
        limits.max_row_span_terms,
    )?;
    check_limit(
        ROW_SPAN_MANIFEST_BYTES_RESOURCE,
        stats.row_span_manifest_bytes,
        limits.max_row_span_manifest_bytes,
    )?;
    check_limit(
        ORDERING_MANIFEST_BYTES_RESOURCE,
        stats.ordering_manifest_bytes,
        limits.max_ordering_manifest_bytes,
    )?;
    check_limit(
        PREPARE_POINT_LAYERS_RESOURCE,
        stats.prepare_point_layers,
        limits.max_prepare_point_layers,
    )?;
    check_limit(
        PREPARE_POINTS_RESOURCE,
        stats.prepare_points,
        limits.max_prepare_points,
    )?;
    check_limit(
        BINDING_BYTES_RESOURCE,
        stats.binding_bytes,
        limits.max_binding_bytes,
    )?;
    check_limit(
        REPLAY_COMPARISON_UNITS_RESOURCE,
        stats.replay_comparisons,
        limits.max_replay_comparisons,
    )
}

fn checked_sum<const N: usize>(
    resource: &'static str,
    values: [usize; N],
) -> Result<usize, GeneratedCylindricalSectorRootStartError> {
    values.into_iter().try_fold(0usize, |total, value| {
        total
            .checked_add(value)
            .ok_or(GeneratedCylindricalSectorRootStartError::ResourceCountOverflow { resource })
    })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedCylindricalSectorRootStartError> {
    left.checked_mul(right)
        .ok_or(GeneratedCylindricalSectorRootStartError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedCylindricalSectorRootStartError> {
    if requested > limit {
        Err(GeneratedCylindricalSectorRootStartError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedCylindricalSectorRootStartError {
    SchemaMismatch,
    ReplayMismatch,
    WrongFamily,
    WrongContext,
    WrongContextArity {
        expected: usize,
        actual: usize,
    },
    WrongSectorArity {
        expected: usize,
        actual: usize,
    },
    WrongRestrictionsArity {
        expected: usize,
        actual: usize,
    },
    ReplayedRowSpanConfigurationMismatch,
    SourceSectorMissing,
    SourceSectorExcluded(SectorExclusion),
    SourceSectorProvedZero(ZeroSectorCertificate),
    SourceSectorResourceLimited(ZeroSectorResource),
    SourceSectorAnalysisFailed(ZeroSectorError),
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    Inventory(FamilySectorInventoryError),
    RowSpan(GeneratedSymbolicRowSpanError),
    Coefficient(ParametricCoefficientError),
    Ordering(CylindricalOrderingError),
    Schedule(CylindricalPreparePointScheduleError),
    Sector(SectorFoundationError),
}

impl fmt::Display for GeneratedCylindricalSectorRootStartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => {
                formatter.write_str("cylindrical sector-root start schema mismatch")
            }
            Self::ReplayMismatch => {
                formatter.write_str("cylindrical sector-root start does not replay")
            }
            Self::WrongFamily => {
                formatter.write_str("cylindrical sector-root start belongs to another family")
            }
            Self::WrongContext => {
                formatter.write_str("cylindrical sector-root start belongs to another K(n) context")
            }
            Self::WrongContextArity { expected, actual } => write!(
                formatter,
                "cylindrical sector-root K(n) context has arity {actual}, expected {expected}"
            ),
            Self::WrongSectorArity { expected, actual } => write!(
                formatter,
                "cylindrical sector root has arity {actual}, expected {expected}"
            ),
            Self::WrongRestrictionsArity { expected, actual } => write!(
                formatter,
                "cylindrical sector-root restrictions have arity {actual}, expected {expected}"
            ),
            Self::ReplayedRowSpanConfigurationMismatch => formatter.write_str(
                "replayed row-span child limits do not match the effective sector-root limits",
            ),
            Self::SourceSectorMissing => {
                formatter.write_str("sector-root source is absent from the family inventory")
            }
            Self::SourceSectorExcluded(_) => formatter
                .write_str("excluded inventory sectors cannot produce a cylindrical root start"),
            Self::SourceSectorProvedZero(_) => formatter.write_str(
                "a sector proved zero by the inventory cannot produce a cylindrical root start",
            ),
            Self::SourceSectorResourceLimited(resource) => write!(
                formatter,
                "sector-root zero analysis was resource-limited at {}: requested {}, limit {}",
                resource.resource(),
                resource.requested(),
                resource.limit()
            ),
            Self::SourceSectorAnalysisFailed(error) => {
                write!(formatter, "sector-root zero analysis failed: {error}")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::Inventory(error) => error.fmt(formatter),
            Self::RowSpan(error) => error.fmt(formatter),
            Self::Coefficient(error) => error.fmt(formatter),
            Self::Ordering(error) => error.fmt(formatter),
            Self::Schedule(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedCylindricalSectorRootStartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SourceSectorAnalysisFailed(error) => Some(error),
            Self::Inventory(error) => Some(error),
            Self::RowSpan(error) => Some(error),
            Self::Coefficient(error) => Some(error),
            Self::Ordering(error) => Some(error),
            Self::Schedule(error) => Some(error),
            Self::Sector(error) => Some(error),
            _ => None,
        }
    }
}

impl From<FamilySectorInventoryError> for GeneratedCylindricalSectorRootStartError {
    fn from(value: FamilySectorInventoryError) -> Self {
        Self::Inventory(value)
    }
}

impl From<GeneratedSymbolicRowSpanError> for GeneratedCylindricalSectorRootStartError {
    fn from(value: GeneratedSymbolicRowSpanError) -> Self {
        Self::RowSpan(value)
    }
}

impl From<ParametricCoefficientError> for GeneratedCylindricalSectorRootStartError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::Coefficient(value)
    }
}

impl From<CylindricalOrderingError> for GeneratedCylindricalSectorRootStartError {
    fn from(value: CylindricalOrderingError) -> Self {
        Self::Ordering(value)
    }
}

impl From<CylindricalPreparePointScheduleError> for GeneratedCylindricalSectorRootStartError {
    fn from(value: CylindricalPreparePointScheduleError) -> Self {
        Self::Schedule(value)
    }
}

impl From<SectorFoundationError> for GeneratedCylindricalSectorRootStartError {
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AffineDenominator, CoefficientContext, CutConstraint, FamilySectorInventoryCompiler,
        FamilySectorInventoryLimits, GeneratedCylindricalRowSystemCertificate,
        GeneratedCylindricalRowSystemError, GeneratedCylindricalRowSystemLimits,
        ParametricIbpGenerator, SectorPattern,
    };

    fn massive_tadpole(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        IntegralFamily::new(
            name,
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

    fn equal_mass_sunset(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let zero = coefficients.zero();
        let one = coefficients.one();
        let minus_m2 = coefficients.parse("-m2").unwrap();
        IntegralFamily::new(
            name,
            vec!["k1".into(), "k2".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![one.clone(), zero.clone(), zero.clone()],
                ),
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![zero.clone(), zero.clone(), one.clone()],
                ),
                AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
            ],
            Vec::new(),
            vec![zero.clone(), zero.clone(), zero],
        )
        .unwrap()
    }

    fn inventory(
        family: &IntegralFamily,
        restrictions: SectorRestrictions,
    ) -> Arc<FamilySectorInventoryCertificate> {
        Arc::new(
            FamilySectorInventoryCompiler::compile(
                family,
                restrictions,
                PowerShiftPolicy::FormalGeneric,
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                FamilySectorInventoryLimits::default(),
            )
            .unwrap(),
        )
    }

    fn compile_with_limits(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        inventory: Arc<FamilySectorInventoryCertificate>,
        limits: GeneratedCylindricalSectorRootStartLimits,
    ) -> Result<
        GeneratedCylindricalSectorRootStartCertificate,
        GeneratedCylindricalSectorRootStartError,
    > {
        GeneratedCylindricalSectorRootStartCertificate::compile(
            family,
            context,
            inventory,
            SectorMask::try_new([true]).unwrap(),
            ParametricIbpConfig::default(),
            GeneratedSymbolicRowSpanConfig::default(),
            1,
            limits,
        )
    }

    fn fixture() -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<FamilySectorInventoryCertificate>,
        GeneratedCylindricalSectorRootStartCertificate,
    ) {
        let family = massive_tadpole("generated-sector-root-massive-tadpole");
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let source = inventory(
            &family,
            SectorRestrictions::unrestricted(family.denominator_count()).unwrap(),
        );
        let certificate = compile_with_limits(
            &family,
            &context,
            source.clone(),
            GeneratedCylindricalSectorRootStartLimits::default(),
        )
        .unwrap();
        (family, context, source, certificate)
    }

    fn exact_limits(
        certificate: &GeneratedCylindricalSectorRootStartCertificate,
    ) -> GeneratedCylindricalSectorRootStartLimits {
        let stats = certificate.stats();
        GeneratedCylindricalSectorRootStartLimits {
            ordering: certificate.limits().ordering,
            schedule: certificate.limits().schedule,
            max_sector_arity: stats.sector_arity(),
            max_assignment_entries: stats.assignment_entries(),
            max_sector_witness_entries: stats.sector_witness_entries(),
            max_row_span_rows: stats.row_span_rows(),
            max_row_span_terms: stats.row_span_terms(),
            max_row_span_manifest_bytes: stats.row_span_manifest_bytes(),
            max_ordering_manifest_bytes: stats.ordering_manifest_bytes(),
            max_prepare_point_layers: stats.prepare_point_layers(),
            max_prepare_points: stats.prepare_points(),
            max_binding_bytes: stats.binding_bytes(),
            max_replay_comparisons: stats.replay_comparisons(),
        }
    }

    #[test]
    fn effective_child_intersection_preserves_every_stricter_caller_cap() {
        let mut row_span = GeneratedSymbolicRowSpanConfig::default();
        row_span.limits.max_canonical_rows = 7;
        row_span.limits.max_augmented_rows = 8;
        row_span.limits.max_canonical_terms = 9;
        row_span.limits.max_augmented_terms = 10;
        row_span.limits.max_aggregate_manifest_bytes = 11;
        let mut limits = GeneratedCylindricalSectorRootStartLimits::default();
        limits.ordering.max_manifest_bytes = 12;
        limits.schedule.max_depth = 13;
        limits.schedule.max_retained_points = 14;

        let (effective_row_span, effective_limits) =
            effective_child_configuration(row_span, 1, limits).unwrap();
        assert_eq!(effective_row_span, row_span);
        assert_eq!(effective_limits, limits);
    }

    #[test]
    fn active_massive_tadpole_root_is_global_empty_and_anchor_free() {
        let (family, context, source, certificate) = fixture();
        certificate.replay(&family, &context).unwrap();

        assert!(Arc::ptr_eq(certificate.inventory_arc(), &source));
        assert_eq!(certificate.restrictions(), source.restrictions());
        assert_eq!(
            certificate.power_shift_policy(),
            PowerShiftPolicy::FormalGeneric
        );
        assert_eq!(
            certificate.ordering_policy(),
            IntegralOrderingPolicy::RustRedUnshiftedV1
        );
        assert_eq!(certificate.sector().active_bits(), &[true]);
        assert_eq!(
            certificate.sector_witness().raw_sector(),
            certificate.sector()
        );
        assert!(certificate.assignment().is_empty());
        assert!(certificate.assignment().entries().is_empty());
        assert!(certificate.completeness().is_complete_integer_cylinder());
        assert_eq!(certificate.schedule().ordering().free_positions(), &[0]);
        assert_eq!(certificate.row_span().rows().len(), 1);
        assert_eq!(certificate.stats().assignment_entries(), 0);
        assert_eq!(certificate.stats().free_positions(), 1);
        assert_eq!(
            certificate.row_span_config().limits.max_augmented_rows,
            certificate.limits().max_row_span_rows.min(
                GeneratedSymbolicRowSpanConfig::default()
                    .limits
                    .max_augmented_rows
            )
        );
        assert_eq!(
            certificate.limits().schedule.max_retained_points,
            certificate.limits().max_prepare_points.min(
                GeneratedCylindricalSectorRootStartLimits::default()
                    .schedule
                    .max_retained_points
            )
        );
        // The public surface intentionally has no discovery-anchor accessor.
    }

    #[test]
    fn replayed_family_row_span_matches_fresh_compile_and_retains_exact_arcs() {
        let (family, context, inventory, freshly_compiled) = fixture();
        let shared_row_span = freshly_compiled.row_span_arc().clone();
        let replayed = GeneratedCylindricalSectorRootStartCertificate::
            compile_with_replayed_inventory_and_row_span(
                &family,
                &context,
                inventory.clone(),
                SectorMask::try_new([true]).unwrap(),
                shared_row_span.clone(),
                1,
                GeneratedCylindricalSectorRootStartLimits::default(),
            )
            .unwrap();

        assert!(freshly_compiled.payload_eq(&replayed));
        assert!(Arc::ptr_eq(replayed.inventory_arc(), &inventory));
        assert!(Arc::ptr_eq(replayed.row_span_arc(), &shared_row_span));
        replayed.replay(&family, &context).unwrap();
    }

    #[test]
    fn one_family_row_span_is_shared_across_distinct_sunset_sector_roots() {
        let family = equal_mass_sunset("generated-sector-root-shared-sunset");
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let inventory = inventory(
            &family,
            SectorRestrictions::unrestricted(family.denominator_count()).unwrap(),
        );
        let shared_row_span = Arc::new(
            GeneratedSymbolicRowSpanCompiler::compile(
                &family,
                &context,
                ParametricIbpConfig::default(),
                GeneratedSymbolicRowSpanConfig::default(),
            )
            .unwrap(),
        );
        let boundary = GeneratedCylindricalSectorRootStartCertificate::
            compile_with_replayed_inventory_and_row_span(
                &family,
                &context,
                inventory.clone(),
                SectorMask::try_new([false, true, true]).unwrap(),
                shared_row_span.clone(),
                1,
                GeneratedCylindricalSectorRootStartLimits::default(),
            )
            .unwrap();
        let top = GeneratedCylindricalSectorRootStartCertificate::
            compile_with_replayed_inventory_and_row_span(
                &family,
                &context,
                inventory.clone(),
                SectorMask::try_new([true, true, true]).unwrap(),
                shared_row_span.clone(),
                1,
                GeneratedCylindricalSectorRootStartLimits::default(),
            )
            .unwrap();

        assert_ne!(boundary.sector(), top.sector());
        assert_eq!(shared_row_span.rows().len(), 4);
        assert!(Arc::ptr_eq(boundary.inventory_arc(), &inventory));
        assert!(Arc::ptr_eq(top.inventory_arc(), &inventory));
        assert!(Arc::ptr_eq(boundary.row_span_arc(), &shared_row_span));
        assert!(Arc::ptr_eq(top.row_span_arc(), &shared_row_span));
        boundary.replay(&family, &context).unwrap();
        top.replay(&family, &context).unwrap();
    }

    #[test]
    fn replayed_family_row_span_rejects_foreign_scope_and_incompatible_child_limits() {
        let (family, context, inventory, freshly_compiled) = fixture();
        let shared_row_span = freshly_compiled.row_span_arc().clone();
        let wrong_context = ParametricCoefficientContext::try_new(
            family.coefficient_context(),
            "generated-sector-root-shared-foreign-context",
            family.denominator_count(),
        )
        .unwrap();
        assert_eq!(
            GeneratedCylindricalSectorRootStartCertificate::
                compile_with_replayed_inventory_and_row_span(
                    &family,
                    &wrong_context,
                    inventory.clone(),
                    SectorMask::try_new([true]).unwrap(),
                    shared_row_span.clone(),
                    1,
                    GeneratedCylindricalSectorRootStartLimits::default(),
                )
                .unwrap_err(),
            GeneratedCylindricalSectorRootStartError::WrongContext
        );

        let foreign_family = massive_tadpole("generated-sector-root-shared-foreign-family");
        assert_eq!(
            GeneratedCylindricalSectorRootStartCertificate::
                compile_with_replayed_inventory_and_row_span(
                    &foreign_family,
                    &context,
                    inventory.clone(),
                    SectorMask::try_new([true]).unwrap(),
                    shared_row_span.clone(),
                    1,
                    GeneratedCylindricalSectorRootStartLimits::default(),
                )
                .unwrap_err(),
            GeneratedCylindricalSectorRootStartError::WrongFamily
        );

        let mut replay_limited = GeneratedCylindricalSectorRootStartLimits::default();
        replay_limited.max_replay_comparisons = 0;
        assert!(matches!(
            GeneratedCylindricalSectorRootStartCertificate::
                compile_with_replayed_inventory_and_row_span(
                    &family,
                    &context,
                    inventory.clone(),
                    SectorMask::try_new([true]).unwrap(),
                    shared_row_span.clone(),
                    1,
                    replay_limited,
                )
                .unwrap_err(),
            GeneratedCylindricalSectorRootStartError::ResourceLimit {
                resource: REPLAY_COMPARISON_UNITS_RESOURCE,
                requested: _,
                limit: 0,
            }
        ));

        let mut incompatible_limits = GeneratedCylindricalSectorRootStartLimits::default();
        incompatible_limits.max_row_span_rows = 0;
        assert_eq!(
            GeneratedCylindricalSectorRootStartCertificate::
                compile_with_replayed_inventory_and_row_span(
                    &family,
                    &context,
                    inventory,
                    SectorMask::try_new([true]).unwrap(),
                    shared_row_span,
                    1,
                    incompatible_limits,
                )
                .unwrap_err(),
            GeneratedCylindricalSectorRootStartError::ReplayedRowSpanConfigurationMismatch
        );
    }

    #[test]
    fn wrong_scope_and_private_payload_tampering_fail_replay() {
        let (family, context, _, certificate) = fixture();
        let wrong_context = ParametricCoefficientContext::try_new(
            family.coefficient_context(),
            "generated-sector-root-foreign-scope",
            family.denominator_count(),
        )
        .unwrap();
        assert_eq!(
            certificate.replay(&family, &wrong_context),
            Err(GeneratedCylindricalSectorRootStartError::WrongContext)
        );

        let foreign_family = massive_tadpole("generated-sector-root-foreign-family");
        assert_eq!(
            certificate.replay(&foreign_family, &context),
            Err(GeneratedCylindricalSectorRootStartError::WrongFamily)
        );

        let mut tampered = certificate.clone();
        tampered.schema = "rustred-generated-cylindrical-sector-root-start-v999";
        assert_eq!(
            tampered.replay(&family, &context),
            Err(GeneratedCylindricalSectorRootStartError::SchemaMismatch)
        );

        let mut tampered = certificate.clone();
        tampered.assignment = PartialIndexAssignment::try_new([(0, 1)], 1, 1).unwrap();
        assert_eq!(
            tampered.replay(&family, &context),
            Err(GeneratedCylindricalSectorRootStartError::ReplayMismatch)
        );

        let mut tampered = certificate.clone();
        tampered.restrictions = SectorRestrictions::try_new(
            CutConstraint::none(1).unwrap(),
            SectorPattern::try_from_string("1").unwrap(),
        )
        .unwrap();
        assert_eq!(
            tampered.replay(&family, &context),
            Err(GeneratedCylindricalSectorRootStartError::ReplayMismatch)
        );

        let mut tampered = certificate.clone();
        tampered.stats.binding_bytes += 1;
        assert_eq!(
            tampered.replay(&family, &context),
            Err(GeneratedCylindricalSectorRootStartError::ReplayMismatch)
        );

        let foreign_row_span = GeneratedSymbolicRowSpanCompiler::compile(
            &family,
            &wrong_context,
            certificate.ibp_config(),
            certificate.row_span_config(),
        )
        .unwrap();
        let mut tampered = certificate.clone();
        tampered.row_span = Arc::new(foreign_row_span);
        assert_eq!(
            tampered.replay(&family, &context),
            Err(GeneratedCylindricalSectorRootStartError::WrongContext)
        );

        let mut tampered = certificate.clone();
        tampered.completeness =
            GeneratedCylindricalStartCompleteness::DependentSymbolicStartPending {
                unresolved_equality_predicate_ordinals: vec![0].into_boxed_slice(),
            };
        assert_eq!(
            tampered.replay(&family, &context),
            Err(GeneratedCylindricalSectorRootStartError::ReplayMismatch)
        );

        let mut tampered = certificate.clone();
        tampered.ibp.arithmetic_limits.max_source_terms += 1;
        assert_eq!(
            tampered.replay(&family, &context),
            Err(GeneratedCylindricalSectorRootStartError::ReplayMismatch)
        );

        let mut tampered = certificate.clone();
        tampered.row_span_config.limits.max_augmented_rows += 1;
        assert_eq!(
            tampered.replay(&family, &context),
            Err(GeneratedCylindricalSectorRootStartError::ReplayMismatch)
        );

        let mut tampered = certificate.clone();
        tampered.limits.max_prepare_point_layers = 1;
        assert!(matches!(
            tampered.replay(&family, &context),
            Err(GeneratedCylindricalSectorRootStartError::ResourceLimit {
                resource: PREPARE_POINT_LAYERS_RESOURCE,
                requested: 2,
                limit: 1,
            })
        ));
    }

    #[test]
    fn excluded_sector_is_rejected_before_row_span_generation() {
        let family = massive_tadpole("generated-sector-root-excluded");
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let restrictions = SectorRestrictions::try_new(
            CutConstraint::none(1).unwrap(),
            SectorPattern::try_from_string("0").unwrap(),
        )
        .unwrap();
        let source = inventory(&family, restrictions);
        assert!(matches!(
            compile_with_limits(
                &family,
                &context,
                source,
                GeneratedCylindricalSectorRootStartLimits::default(),
            ),
            Err(GeneratedCylindricalSectorRootStartError::SourceSectorExcluded(_))
        ));
    }

    #[test]
    fn sector_proved_zero_is_rejected_before_row_span_generation() {
        let family = massive_tadpole("generated-sector-root-proved-zero");
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let source = inventory(
            &family,
            SectorRestrictions::unrestricted(family.denominator_count()).unwrap(),
        );
        let zero_sector = SectorMask::try_new([false]).unwrap();
        assert!(matches!(
            source.status(&zero_sector),
            Some(FamilySectorInventoryStatus::ProvedZero(_))
        ));
        assert!(matches!(
            GeneratedCylindricalSectorRootStartCertificate::compile(
                &family,
                &context,
                source,
                zero_sector,
                ParametricIbpConfig::default(),
                GeneratedSymbolicRowSpanConfig::default(),
                1,
                GeneratedCylindricalSectorRootStartLimits::default(),
            ),
            Err(GeneratedCylindricalSectorRootStartError::SourceSectorProvedZero(_))
        ));
    }

    #[test]
    fn root_payload_tampering_propagates_through_row_system_acceptance() {
        let (family, context, _, mut root) = fixture();
        root.completeness = GeneratedCylindricalStartCompleteness::DependentSymbolicStartPending {
            unresolved_equality_predicate_ordinals: vec![0].into_boxed_slice(),
        };
        let error = GeneratedCylindricalRowSystemCertificate::compile_from_sector_root(
            &family,
            &context,
            Arc::new(root),
            GeneratedCylindricalRowSystemLimits::default(),
        )
        .unwrap_err();
        assert_eq!(
            error,
            GeneratedCylindricalRowSystemError::SectorRootStart(
                GeneratedCylindricalSectorRootStartError::ReplayMismatch,
            )
        );
    }

    #[test]
    fn exact_outer_limits_compile_and_every_positive_limit_has_one_below_evidence() {
        let (family, context, source, certificate) = fixture();
        let stats = certificate.stats();
        let exact = exact_limits(&certificate);
        let exact_certificate =
            compile_with_limits(&family, &context, source.clone(), exact).unwrap();
        assert_eq!(exact_certificate.stats(), stats);
        assert_eq!(
            exact_certificate
                .row_span_config()
                .limits
                .max_canonical_rows,
            stats.row_span_rows()
        );
        assert_eq!(
            exact_certificate
                .row_span_config()
                .limits
                .max_augmented_rows,
            stats.row_span_rows()
        );
        assert_eq!(
            exact_certificate
                .row_span_config()
                .limits
                .max_canonical_terms,
            stats.row_span_terms()
        );
        assert_eq!(
            exact_certificate
                .row_span_config()
                .limits
                .max_augmented_terms,
            stats.row_span_terms()
        );
        assert_eq!(
            exact_certificate
                .row_span_config()
                .limits
                .max_aggregate_manifest_bytes,
            stats.row_span_manifest_bytes()
        );
        assert_eq!(
            exact_certificate.limits().ordering.max_manifest_bytes,
            stats.ordering_manifest_bytes()
        );
        assert_eq!(
            exact_certificate.limits().schedule.max_depth,
            stats.prepare_point_layers() - 1
        );
        assert_eq!(
            exact_certificate.limits().schedule.max_retained_points,
            stats.prepare_points()
        );
        exact_certificate.replay(&family, &context).unwrap();
        assert_eq!(stats.assignment_entries(), 0);
        assert_eq!(exact.max_assignment_entries, 0);

        macro_rules! one_below {
            ($field:ident, $getter:ident, $resource:expr) => {{
                let requested = stats.$getter();
                assert!(requested > 0, "{} fixture must be positive", $resource);
                let mut limits = exact;
                limits.$field = requested - 1;
                assert_eq!(
                    validate_stats(stats, limits),
                    Err(GeneratedCylindricalSectorRootStartError::ResourceLimit {
                        resource: $resource,
                        requested,
                        limit: requested - 1,
                    })
                );
                assert!(
                    compile_with_limits(&family, &context, source.clone(), limits).is_err(),
                    "public compile path accepted one-below {}",
                    $resource
                );
            }};
        }

        one_below!(max_sector_arity, sector_arity, SECTOR_ARITY_RESOURCE);
        one_below!(
            max_sector_witness_entries,
            sector_witness_entries,
            SECTOR_WITNESS_ENTRIES_RESOURCE
        );
        one_below!(max_row_span_rows, row_span_rows, ROW_SPAN_ROWS_RESOURCE);
        one_below!(max_row_span_terms, row_span_terms, ROW_SPAN_TERMS_RESOURCE);
        one_below!(
            max_row_span_manifest_bytes,
            row_span_manifest_bytes,
            ROW_SPAN_MANIFEST_BYTES_RESOURCE
        );
        one_below!(
            max_ordering_manifest_bytes,
            ordering_manifest_bytes,
            ORDERING_MANIFEST_BYTES_RESOURCE
        );
        one_below!(
            max_prepare_point_layers,
            prepare_point_layers,
            PREPARE_POINT_LAYERS_RESOURCE
        );
        one_below!(max_prepare_points, prepare_points, PREPARE_POINTS_RESOURCE);
        one_below!(max_binding_bytes, binding_bytes, BINDING_BYTES_RESOURCE);
        one_below!(
            max_replay_comparisons,
            replay_comparisons,
            REPLAY_COMPARISON_UNITS_RESOURCE
        );
    }
}
