//! Exact affine-start ordering bound to one generated actionable case.
//!
//! This is the generated-inventory counterpart of the public V1 affine-start
//! ordering.  It owns the exact [`GeneratedAffineResidualCaseAuthority`] Arc
//! and derives all geometry through authenticated borrowed case/group views.
//! No residual-unit predicate locator is synthesized for a simultaneous
//! generated affine system.

use std::cmp::Ordering;
use std::fmt;
use std::fmt::Write as _;
use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use symbolica::prelude::Integer;

use crate::affine_parametric_ordering::{
    AFFINE_START_INTEGRAL_COMPLEXITY_KEY_V1_SCHEMA, AffineConstantRowShiftClassification,
    AffineParametricOrderingAlgebra, AffineParametricOrderingError,
    AffineParametricOrderingGeometry, AffineParametricOrderingLimits,
    AffineStartIntegralComplexityKey, RUSTRED_AFFINE_START_UNSHIFTED_ORDER_V1_KEY_SCHEMA,
    integer_magnitude_bits, key_component_count,
};
use crate::solver::closure::case_inventory::{
    GeneratedAffineResidualCaseAuthority, GeneratedAffineResidualCaseAuthorityError,
    GeneratedAffineResidualCaseAuthoritySourceKind, GeneratedAffineResidualCaseSourceLocator,
    GeneratedAffineResidualCaseSourceRecordView, GeneratedAffineResidualInventoryGroupSourceView,
};
use crate::{
    IndexShift, IntegralFamily, IntegralOrderingPolicy, ParametricCoefficientContext, SectorMask,
};

pub(crate) const GENERATED_AFFINE_PARAMETRIC_ORDERING_V2_SCHEMA: &str =
    "rustred-generated-affine-parametric-ordering-v2";
pub(crate) const GENERATED_AFFINE_PARAMETRIC_ORDERING_V3_SCHEMA: &str =
    "rustred-generated-affine-parametric-ordering-v3";
pub(crate) const GENERATED_AFFINE_PARAMETRIC_ORDERING_V4_SCHEMA: &str =
    "rustred-generated-affine-parametric-ordering-v4";

const CONSTRUCTION_CASE_GROUP_MEMBERSHIP_CHECKS: usize = 8;

const fn ordering_schema_for_source(
    source: GeneratedAffineResidualCaseAuthoritySourceKind,
) -> &'static str {
    match source {
        GeneratedAffineResidualCaseAuthoritySourceKind::InitialInventory => {
            GENERATED_AFFINE_PARAMETRIC_ORDERING_V2_SCHEMA
        }
        GeneratedAffineResidualCaseAuthoritySourceKind::DirectFormulaSingleton => {
            GENERATED_AFFINE_PARAMETRIC_ORDERING_V3_SCHEMA
        }
        GeneratedAffineResidualCaseAuthoritySourceKind::CommittedExceptionalSingleton => {
            GENERATED_AFFINE_PARAMETRIC_ORDERING_V4_SCHEMA
        }
    }
}

/// Prospective construction envelope for one generated affine ordering.
///
/// `ordering` bounds all geometry, manifest, and future key allocations.  The
/// remaining limits account exactly for this outer certificate's authority
/// and generated-inventory navigation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineParametricOrderingLimits {
    pub(crate) ordering: AffineParametricOrderingLimits,
    pub(crate) max_authority_replays: usize,
    pub(crate) max_case_view_resolutions: usize,
    pub(crate) max_group_view_resolutions: usize,
    pub(crate) max_case_group_membership_checks: usize,
    pub(crate) max_group_case_references: usize,
    pub(crate) max_source_rows: usize,
    pub(crate) max_retained_authority_references: usize,
}

impl Default for GeneratedAffineParametricOrderingLimits {
    fn default() -> Self {
        Self {
            ordering: AffineParametricOrderingLimits::default(),
            max_authority_replays: 1,
            max_case_view_resolutions: 1,
            max_group_view_resolutions: 1,
            max_case_group_membership_checks: CONSTRUCTION_CASE_GROUP_MEMBERSHIP_CHECKS,
            max_group_case_references: 256_000_000,
            max_source_rows: 1_000_000,
            max_retained_authority_references: 1,
        }
    }
}

/// Exact census for all work and retained variable-size payload admitted by
/// construction.  Every positive field has a corresponding limit above (or
/// in its nested `ordering` envelope).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineParametricOrderingStats {
    authority_replays: usize,
    case_view_resolutions: usize,
    group_view_resolutions: usize,
    case_group_membership_checks: usize,
    group_case_references: usize,
    source_rows: usize,
    retained_authority_references: usize,
    ambient_arity: usize,
    free_positions: usize,
    constant_positions: usize,
    symbolic_positions: usize,
    matrix_entries_inspected: usize,
    largest_affine_integer_bits: usize,
    source_identity_bytes: usize,
    manifest_bytes: usize,
}

impl GeneratedAffineParametricOrderingStats {
    pub(crate) const fn authority_replays(self) -> usize {
        self.authority_replays
    }
    pub(crate) const fn case_view_resolutions(self) -> usize {
        self.case_view_resolutions
    }
    pub(crate) const fn group_view_resolutions(self) -> usize {
        self.group_view_resolutions
    }
    pub(crate) const fn case_group_membership_checks(self) -> usize {
        self.case_group_membership_checks
    }
    pub(crate) const fn group_case_references(self) -> usize {
        self.group_case_references
    }
    pub(crate) const fn source_rows(self) -> usize {
        self.source_rows
    }
    pub(crate) const fn retained_authority_references(self) -> usize {
        self.retained_authority_references
    }
    pub(crate) const fn ambient_arity(self) -> usize {
        self.ambient_arity
    }
    pub(crate) const fn free_positions(self) -> usize {
        self.free_positions
    }
    pub(crate) const fn constant_positions(self) -> usize {
        self.constant_positions
    }
    pub(crate) const fn symbolic_positions(self) -> usize {
        self.symbolic_positions
    }
    pub(crate) const fn matrix_entries_inspected(self) -> usize {
        self.matrix_entries_inspected
    }
    pub(crate) const fn largest_affine_integer_bits(self) -> usize {
        self.largest_affine_integer_bits
    }
    pub(crate) const fn source_identity_bytes(self) -> usize {
        self.source_identity_bytes
    }
    pub(crate) const fn manifest_bytes(self) -> usize {
        self.manifest_bytes
    }
}

/// Failures intentionally expose no family/context fingerprints, affine
/// coefficients, inventory payloads, or independently supplied authorities.
pub(crate) enum GeneratedAffineParametricOrderingError {
    SchemaMismatch,
    AuthorityAllocationMismatch,
    CaseBindingMismatch,
    GroupBindingMismatch,
    GeometryBindingMismatch,
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
    Authority(GeneratedAffineResidualCaseAuthorityError),
    Ordering(AffineParametricOrderingError),
    SymbolicaPanic,
}

impl fmt::Debug for GeneratedAffineParametricOrderingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl fmt::Display for GeneratedAffineParametricOrderingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => {
                formatter.write_str("generated affine ordering schema mismatch")
            }
            Self::AuthorityAllocationMismatch => formatter.write_str(
                "generated affine ordering requires its exact source authority allocation",
            ),
            Self::CaseBindingMismatch => {
                formatter.write_str("generated affine ordering case binding mismatch")
            }
            Self::GroupBindingMismatch => {
                formatter.write_str("generated affine ordering group binding mismatch")
            }
            Self::GeometryBindingMismatch => {
                formatter.write_str("generated affine ordering geometry binding mismatch")
            }
            Self::ResourceLimit { resource, .. } => {
                write!(
                    formatter,
                    "generated affine ordering {resource} resource limit exceeded"
                )
            }
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "generated affine ordering {resource} count overflowed usize"
                )
            }
            Self::AllocationFailure { resource, .. } => {
                write!(
                    formatter,
                    "generated affine ordering could not allocate {resource}"
                )
            }
            Self::Authority(_) => formatter
                .write_str("generated affine ordering source authority authentication failed"),
            Self::Ordering(_) => {
                formatter.write_str("generated affine ordering algebra operation failed")
            }
            Self::SymbolicaPanic => {
                formatter.write_str("Symbolica panicked during generated affine ordering operation")
            }
        }
    }
}

impl std::error::Error for GeneratedAffineParametricOrderingError {}

impl From<GeneratedAffineResidualCaseAuthorityError> for GeneratedAffineParametricOrderingError {
    fn from(value: GeneratedAffineResidualCaseAuthorityError) -> Self {
        Self::Authority(value)
    }
}

impl From<AffineParametricOrderingError> for GeneratedAffineParametricOrderingError {
    fn from(value: AffineParametricOrderingError) -> Self {
        Self::Ordering(value)
    }
}

/// Replay-bound formal ordering for exactly one generated actionable case.
#[derive(Clone)]
pub(crate) struct GeneratedAffineParametricOrderingCertificate {
    schema: &'static str,
    key_schema: &'static str,
    authority: Arc<GeneratedAffineResidualCaseAuthority>,
    case_ordinal: usize,
    group_ordinal: usize,
    ordinal_within_group: usize,
    policy: IntegralOrderingPolicy,
    constant_positions: Arc<Vec<usize>>,
    symbolic_positions: Arc<Vec<usize>>,
    limits: GeneratedAffineParametricOrderingLimits,
    stats: GeneratedAffineParametricOrderingStats,
    stable_manifest: Arc<String>,
}

impl fmt::Debug for GeneratedAffineParametricOrderingCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineParametricOrderingCertificate")
            .field("schema", &self.schema)
            .field("case_ordinal", &self.case_ordinal)
            .field("group_ordinal", &self.group_ordinal)
            .field("ordinal_within_group", &self.ordinal_within_group)
            .field("arity", &self.arity())
            .field("private_authority", &"<redacted>")
            .field("private_manifest", &"<redacted>")
            .finish()
    }
}

impl PartialEq for GeneratedAffineParametricOrderingCertificate {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
            || self.schema == other.schema
                && self.key_schema == other.key_schema
                && Arc::ptr_eq(&self.authority, &other.authority)
                && self.case_ordinal == other.case_ordinal
                && self.group_ordinal == other.group_ordinal
                && self.ordinal_within_group == other.ordinal_within_group
                && self.policy == other.policy
                && self.constant_positions == other.constant_positions
                && self.symbolic_positions == other.symbolic_positions
                && self.limits == other.limits
                && self.stats == other.stats
                && self.stable_manifest == other.stable_manifest
    }
}

impl Eq for GeneratedAffineParametricOrderingCertificate {}

impl GeneratedAffineParametricOrderingCertificate {
    pub(crate) fn try_new(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: Arc<GeneratedAffineResidualCaseAuthority>,
        limits: GeneratedAffineParametricOrderingLimits,
    ) -> Result<Self, GeneratedAffineParametricOrderingError> {
        catch_unwind(AssertUnwindSafe(|| {
            Self::try_new_unwind_boundary(family, context, authority, limits)
        }))
        .map_err(|_| GeneratedAffineParametricOrderingError::SymbolicaPanic)?
    }

    fn try_new_unwind_boundary(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: Arc<GeneratedAffineResidualCaseAuthority>,
        limits: GeneratedAffineParametricOrderingLimits,
    ) -> Result<Self, GeneratedAffineParametricOrderingError> {
        const AUTHORITY_REPLAYS: usize = 1;
        const CASE_VIEW_RESOLUTIONS: usize = 1;
        const GROUP_VIEW_RESOLUTIONS: usize = 1;
        const RETAINED_AUTHORITY_REFERENCES: usize = 1;
        for (resource, requested, limit) in [
            (
                "authority replays",
                AUTHORITY_REPLAYS,
                limits.max_authority_replays,
            ),
            (
                "case view resolutions",
                CASE_VIEW_RESOLUTIONS,
                limits.max_case_view_resolutions,
            ),
            (
                "group view resolutions",
                GROUP_VIEW_RESOLUTIONS,
                limits.max_group_view_resolutions,
            ),
            (
                "case/group membership checks",
                CONSTRUCTION_CASE_GROUP_MEMBERSHIP_CHECKS,
                limits.max_case_group_membership_checks,
            ),
            (
                "retained authority references",
                RETAINED_AUTHORITY_REFERENCES,
                limits.max_retained_authority_references,
            ),
        ] {
            check_limit(resource, requested, limit)?;
        }

        let arity = authority.arity();
        check_limit("ambient arity", arity, limits.ordering.max_arity)?;
        check_limit(
            "order-key components",
            key_component_count(arity).map_err(GeneratedAffineParametricOrderingError::Ordering)?,
            limits.ordering.max_key_components,
        )?;
        let source_rows = authority.source_row_count();
        check_limit("source rows", source_rows, limits.max_source_rows)?;

        // This is the only full source replay.  All outer constant work was
        // admitted above, before the authority can invoke an allocating
        // inventory replay under its own independently authenticated limits.
        authority.replay(family, context)?;
        let case = authority.authenticated_source_neutral_case_view(context)?;
        let group = authority.authenticated_source_neutral_group_view(context)?;
        let group_case_references = group.case_ordinals().len();
        check_limit(
            "group case references",
            group_case_references,
            limits.max_group_case_references,
        )?;
        validate_case_group_binding(authority.as_ref(), case, group)?;

        let geometry = GeneratedCaseOrderingGeometry { case };
        let preflight = preflight_geometry(authority.as_ref(), &geometry, group, limits.ordering)?;
        let schema = ordering_schema_for_source(authority.source_kind());
        let source_identity_bytes = count_source_identity(authority.as_ref(), case, group)?;
        check_limit(
            "source identity bytes",
            source_identity_bytes,
            limits.ordering.max_map_identity_bytes,
        )?;
        let manifest_bytes = count_manifest(
            schema,
            authority.as_ref(),
            case,
            group,
            limits,
            &preflight,
            source_identity_bytes,
        )?;
        check_limit(
            "manifest bytes",
            manifest_bytes,
            limits.ordering.max_manifest_bytes,
        )?;

        // Every variable-size count and exact manifest byte length is known
        // before the first ordering-owned buffer allocation.
        let mut constant_positions = Vec::new();
        constant_positions
            .try_reserve_exact(preflight.constant_positions)
            .map_err(
                |_| GeneratedAffineParametricOrderingError::AllocationFailure {
                    resource: "constant positions",
                    requested: preflight.constant_positions,
                },
            )?;
        let mut symbolic_positions = Vec::new();
        symbolic_positions
            .try_reserve_exact(preflight.symbolic_positions)
            .map_err(
                |_| GeneratedAffineParametricOrderingError::AllocationFailure {
                    resource: "symbolic positions",
                    requested: preflight.symbolic_positions,
                },
            )?;
        fill_classified_positions(&geometry, &mut constant_positions, &mut symbolic_positions)?;
        if constant_positions.len() != preflight.constant_positions
            || symbolic_positions.len() != preflight.symbolic_positions
        {
            return Err(GeneratedAffineParametricOrderingError::GeometryBindingMismatch);
        }
        let stable_manifest = render_manifest(
            authority.as_ref(),
            schema,
            case,
            group,
            limits,
            &constant_positions,
            &symbolic_positions,
            source_identity_bytes,
            manifest_bytes,
        )?;
        let stats = GeneratedAffineParametricOrderingStats {
            authority_replays: AUTHORITY_REPLAYS,
            case_view_resolutions: CASE_VIEW_RESOLUTIONS,
            group_view_resolutions: GROUP_VIEW_RESOLUTIONS,
            case_group_membership_checks: CONSTRUCTION_CASE_GROUP_MEMBERSHIP_CHECKS,
            group_case_references,
            source_rows,
            retained_authority_references: RETAINED_AUTHORITY_REFERENCES,
            ambient_arity: arity,
            free_positions: geometry.free_positions().len(),
            constant_positions: constant_positions.len(),
            symbolic_positions: symbolic_positions.len(),
            matrix_entries_inspected: preflight.matrix_entries_inspected,
            largest_affine_integer_bits: preflight.largest_affine_integer_bits,
            source_identity_bytes,
            manifest_bytes,
        };
        Ok(Self {
            schema,
            key_schema: RUSTRED_AFFINE_START_UNSHIFTED_ORDER_V1_KEY_SCHEMA,
            case_ordinal: case.ordinal(),
            group_ordinal: group.ordinal(),
            ordinal_within_group: case.ordinal_within_group(),
            policy: authority.ordering(),
            authority,
            constant_positions: Arc::new(constant_positions),
            symbolic_positions: Arc::new(symbolic_positions),
            limits,
            stats,
            stable_manifest: Arc::new(stable_manifest),
        })
    }

    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }
    pub(crate) const fn key_schema(&self) -> &'static str {
        self.key_schema
    }
    pub(crate) const fn policy(&self) -> IntegralOrderingPolicy {
        self.policy
    }
    pub(crate) fn sector(&self) -> &SectorMask {
        self.authority.sector()
    }
    pub(crate) fn family_fingerprint(&self) -> &str {
        self.authority.family_fingerprint()
    }
    pub(crate) fn context_fingerprint(&self) -> &str {
        self.authority.context_fingerprint()
    }
    pub(crate) fn arity(&self) -> usize {
        self.authority.arity()
    }
    pub(crate) const fn case_ordinal(&self) -> usize {
        self.case_ordinal
    }
    pub(crate) const fn group_ordinal(&self) -> usize {
        self.group_ordinal
    }
    pub(crate) const fn ordinal_within_group(&self) -> usize {
        self.ordinal_within_group
    }
    pub(crate) fn constant_positions(&self) -> &[usize] {
        self.constant_positions.as_slice()
    }
    pub(crate) fn symbolic_positions(&self) -> &[usize] {
        self.symbolic_positions.as_slice()
    }
    pub(crate) const fn limits(&self) -> GeneratedAffineParametricOrderingLimits {
        self.limits
    }
    pub(crate) const fn stats(&self) -> GeneratedAffineParametricOrderingStats {
        self.stats
    }
    pub(crate) fn stable_manifest(&self) -> &str {
        self.stable_manifest.as_str()
    }

    /// Complete local owner graph for this ordering, excluding only the
    /// authority pointee. The outer `Arc` control block of this certificate is
    /// charged by its retaining parent graph. The manifest allocation is
    /// included here and is consequently excluded from every prepare-point
    /// key which shares it.
    pub(crate) fn owner_retained_bytes_excluding_authority(&self) -> Option<usize> {
        size_of::<Self>()
            .checked_add(arc_vec_owned_byte_bound(&self.constant_positions)?)?
            .checked_add(arc_vec_owned_byte_bound(&self.symbolic_positions)?)?
            .checked_add(arc_string_owned_byte_bound(&self.stable_manifest)?)
    }

    /// Replay requires the exact authority allocation retained at
    /// construction. An independently allocated value-equal authority is not
    /// substitutable, even if it owns the same inventory allocation.
    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
    ) -> Result<(), GeneratedAffineParametricOrderingError> {
        catch_unwind(AssertUnwindSafe(|| {
            if self.schema != ordering_schema_for_source(authority.source_kind())
                || self.key_schema != RUSTRED_AFFINE_START_UNSHIFTED_ORDER_V1_KEY_SCHEMA
            {
                return Err(GeneratedAffineParametricOrderingError::SchemaMismatch);
            }
            if !Arc::ptr_eq(&self.authority, authority) {
                return Err(GeneratedAffineParametricOrderingError::AuthorityAllocationMismatch);
            }
            if self.case_ordinal != authority.case_ordinal() {
                return Err(GeneratedAffineParametricOrderingError::CaseBindingMismatch);
            }
            if self.group_ordinal != authority.group_ordinal() {
                return Err(GeneratedAffineParametricOrderingError::GroupBindingMismatch);
            }
            let rebuilt =
                Self::try_new_unwind_boundary(family, context, Arc::clone(authority), self.limits)?;
            if rebuilt == *self {
                Ok(())
            } else {
                Err(GeneratedAffineParametricOrderingError::GeometryBindingMismatch)
            }
        }))
        .map_err(|_| GeneratedAffineParametricOrderingError::SymbolicaPanic)?
    }

    pub(crate) fn key_for_shift(
        &self,
        context: &ParametricCoefficientContext,
        shift: &IndexShift,
    ) -> Result<AffineStartIntegralComplexityKey, GeneratedAffineParametricOrderingError> {
        catch_unwind(AssertUnwindSafe(|| {
            self.with_algebra(context, |algebra| {
                algebra.key_for_shift(shift, self.limits.ordering.max_key_total_integer_bits)
            })
        }))
        .map_err(|_| GeneratedAffineParametricOrderingError::SymbolicaPanic)?
    }

    pub(crate) fn compare_shifts(
        &self,
        context: &ParametricCoefficientContext,
        left: &IndexShift,
        right: &IndexShift,
    ) -> Result<Ordering, GeneratedAffineParametricOrderingError> {
        catch_unwind(AssertUnwindSafe(|| {
            self.with_algebra(context, |algebra| {
                Ok(algebra
                    .key_for_shift(left, self.limits.ordering.max_key_total_integer_bits)?
                    .cmp(
                        &algebra.key_for_shift(
                            right,
                            self.limits.ordering.max_key_total_integer_bits,
                        )?,
                    ))
            })
        }))
        .map_err(|_| GeneratedAffineParametricOrderingError::SymbolicaPanic)?
    }

    pub(crate) fn replay_key(
        &self,
        context: &ParametricCoefficientContext,
        key: &AffineStartIntegralComplexityKey,
    ) -> Result<(), GeneratedAffineParametricOrderingError> {
        catch_unwind(AssertUnwindSafe(|| {
            self.with_algebra(context, |algebra| algebra.replay_key(key))
        }))
        .map_err(|_| GeneratedAffineParametricOrderingError::SymbolicaPanic)?
    }

    pub(crate) fn constant_start_value<'ordering>(
        &'ordering self,
        context: &ParametricCoefficientContext,
        position: usize,
    ) -> Result<Option<&'ordering Integer>, GeneratedAffineParametricOrderingError> {
        let case = self.authenticated_case(context)?;
        Ok(self
            .constant_positions
            .binary_search(&position)
            .ok()
            .and_then(|_| case.constants().get(position)))
    }

    pub(crate) fn classify_constant_row_shift(
        &self,
        context: &ParametricCoefficientContext,
        position: usize,
        displacement: i64,
    ) -> Result<AffineConstantRowShiftClassification, GeneratedAffineParametricOrderingError> {
        catch_unwind(AssertUnwindSafe(|| {
            self.with_algebra(context, |algebra| {
                algebra.classify_constant_row_shift(position, displacement)
            })
        }))
        .map_err(|_| GeneratedAffineParametricOrderingError::SymbolicaPanic)?
    }

    fn authenticated_case<'ordering>(
        &'ordering self,
        context: &ParametricCoefficientContext,
    ) -> Result<
        GeneratedAffineResidualCaseSourceRecordView<'ordering>,
        GeneratedAffineParametricOrderingError,
    > {
        let case = self
            .authority
            .authenticated_source_neutral_case_view(context)?;
        if case.ordinal() != self.case_ordinal {
            return Err(GeneratedAffineParametricOrderingError::CaseBindingMismatch);
        }
        if case.group_ordinal() != self.group_ordinal
            || case.ordinal_within_group() != self.ordinal_within_group
        {
            return Err(GeneratedAffineParametricOrderingError::GroupBindingMismatch);
        }
        Ok(case)
    }

    fn with_algebra<T>(
        &self,
        context: &ParametricCoefficientContext,
        operation: impl FnOnce(
            &AffineParametricOrderingAlgebra<'_, '_>,
        ) -> Result<T, AffineParametricOrderingError>,
    ) -> Result<T, GeneratedAffineParametricOrderingError> {
        self.with_authenticated_algebra(context, |algebra| {
            operation(algebra).map_err(GeneratedAffineParametricOrderingError::Ordering)
        })
    }

    /// Run one caller-owned operation against a callback-scoped algebra after
    /// authenticating this certificate's exact generated case. The borrowed
    /// case geometry cannot escape the callback. Callers that batch many
    /// shell operations use this boundary once and must provide their own
    /// outer unwind boundary.
    pub(crate) fn with_authenticated_algebra<T, E>(
        &self,
        context: &ParametricCoefficientContext,
        operation: impl FnOnce(&AffineParametricOrderingAlgebra<'_, '_>) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<GeneratedAffineParametricOrderingError>,
    {
        let case = self.authenticated_case(context)?;
        let geometry = GeneratedCaseOrderingGeometry { case };
        let algebra = AffineParametricOrderingAlgebra::new(
            self.policy,
            self.sector(),
            &geometry,
            self.constant_positions.as_slice(),
            self.limits.ordering,
            &self.stable_manifest,
            self.key_schema,
        );
        operation(&algebra)
    }
}

fn arc_payload_control_and_padding_byte_bound<T>() -> Option<usize> {
    size_of::<AtomicUsize>()
        .checked_mul(2)?
        .checked_add(align_of::<T>().saturating_sub(1))?
        .checked_add(size_of::<T>())
}

fn arc_vec_owned_byte_bound<T>(value: &Arc<Vec<T>>) -> Option<usize> {
    arc_payload_control_and_padding_byte_bound::<Vec<T>>()?
        .checked_add(value.capacity().checked_mul(size_of::<T>())?)
}

fn arc_string_owned_byte_bound(value: &Arc<String>) -> Option<usize> {
    arc_payload_control_and_padding_byte_bound::<String>()?.checked_add(value.capacity())
}

#[derive(Clone, Copy)]
struct GeneratedCaseOrderingGeometry<'source> {
    case: GeneratedAffineResidualCaseSourceRecordView<'source>,
}

impl<'source> GeneratedCaseOrderingGeometry<'source> {
    fn geometry(
        self,
    ) -> crate::solver::closure::case_inventory::GeneratedAffineResidualCaseGeometryView<'source>
    {
        self.case.source().geometry()
    }
}

impl<'source> AffineParametricOrderingGeometry<'source> for GeneratedCaseOrderingGeometry<'source> {
    fn ambient_arity(&self) -> usize {
        self.geometry().ambient_arity()
    }

    fn free_positions(&self) -> &'source [usize] {
        self.geometry().free_positions()
    }

    fn constant(&self, position: usize) -> Option<&'source Integer> {
        self.case.constants().get(position)
    }

    fn linear_coefficient(&self, position: usize, free_ordinal: usize) -> Option<&'source Integer> {
        self.geometry()
            .compact_linear_coefficient(position, free_ordinal)
    }
}

struct GeometryPreflight {
    constant_positions: usize,
    symbolic_positions: usize,
    constant_position_decimal_bytes: usize,
    symbolic_position_decimal_bytes: usize,
    matrix_entries_inspected: usize,
    largest_affine_integer_bits: usize,
}

fn validate_case_group_binding(
    authority: &GeneratedAffineResidualCaseAuthority,
    case: GeneratedAffineResidualCaseSourceRecordView<'_>,
    group: GeneratedAffineResidualInventoryGroupSourceView<'_>,
) -> Result<(), GeneratedAffineParametricOrderingError> {
    if case.ordinal() != authority.case_ordinal() {
        return Err(GeneratedAffineParametricOrderingError::CaseBindingMismatch);
    }
    if case.group_ordinal() != authority.group_ordinal()
        || group.ordinal() != authority.group_ordinal()
        || group
            .case_ordinals()
            .get(case.ordinal_within_group())
            .copied()
            != Some(case.ordinal())
    {
        return Err(GeneratedAffineParametricOrderingError::GroupBindingMismatch);
    }
    if group.case_ordinals().is_empty() {
        return Err(GeneratedAffineParametricOrderingError::GroupBindingMismatch);
    }
    if !group.case_ordinals().contains(&group.anchor_case_ordinal())
        || group.anchor_offsets().len() != group.case_ordinals().len()
        || group
            .anchor_offsets()
            .get(case.ordinal_within_group())
            .map(Vec::len)
            != Some(authority.arity())
    {
        return Err(GeneratedAffineParametricOrderingError::GroupBindingMismatch);
    }
    Ok(())
}

fn preflight_geometry(
    authority: &GeneratedAffineResidualCaseAuthority,
    geometry: &GeneratedCaseOrderingGeometry<'_>,
    group: GeneratedAffineResidualInventoryGroupSourceView<'_>,
    limits: AffineParametricOrderingLimits,
) -> Result<GeometryPreflight, GeneratedAffineParametricOrderingError> {
    let arity = geometry.ambient_arity();
    let free_positions = geometry.free_positions();
    let free_count = free_positions.len();
    if arity != authority.arity()
        || authority.sector().arity() != arity
        || geometry.case.constants().len() != arity
        || group.ambient_arity() != arity
        || group.free_positions() != free_positions
    {
        return Err(GeneratedAffineParametricOrderingError::GeometryBindingMismatch);
    }
    check_limit("free positions", free_count, limits.max_free_positions)?;
    let one_pass_matrix_entries = checked_mul("matrix entries inspected", arity, free_count)?;
    let matrix_entries_inspected =
        checked_mul("matrix entries inspected", one_pass_matrix_entries, 2)?;
    check_limit(
        "matrix entries inspected",
        matrix_entries_inspected,
        limits.max_matrix_entries_inspected,
    )?;
    if group.compact_linear_coefficients().len() != one_pass_matrix_entries {
        return Err(GeneratedAffineParametricOrderingError::GeometryBindingMismatch);
    }
    let mut previous_free = None;
    for &position in free_positions {
        if position >= arity || previous_free.is_some_and(|previous| previous >= position) {
            return Err(GeneratedAffineParametricOrderingError::GeometryBindingMismatch);
        }
        previous_free = Some(position);
    }

    let mut constant_positions = 0usize;
    let mut symbolic_positions = 0usize;
    let mut constant_position_decimal_bytes = 0usize;
    let mut symbolic_position_decimal_bytes = 0usize;
    let mut largest_affine_integer_bits = 0usize;
    for position in 0..arity {
        let constant = geometry
            .constant(position)
            .ok_or(GeneratedAffineParametricOrderingError::GeometryBindingMismatch)?;
        if geometry.geometry().constant(position) != Some(constant) {
            return Err(GeneratedAffineParametricOrderingError::GeometryBindingMismatch);
        }
        let constant_bits = integer_magnitude_bits(constant)
            .map_err(GeneratedAffineParametricOrderingError::Ordering)?;
        check_limit(
            "affine integer bits",
            constant_bits,
            limits.max_affine_integer_bits,
        )?;
        largest_affine_integer_bits = largest_affine_integer_bits.max(constant_bits);
        let mut constant_row = true;
        for free_ordinal in 0..free_count {
            let coefficient = geometry
                .linear_coefficient(position, free_ordinal)
                .ok_or(GeneratedAffineParametricOrderingError::GeometryBindingMismatch)?;
            let compact_offset = position
                .checked_mul(free_count)
                .and_then(|offset| offset.checked_add(free_ordinal))
                .ok_or(
                    GeneratedAffineParametricOrderingError::ResourceCountOverflow {
                        resource: "compact matrix offset",
                    },
                )?;
            if group.compact_linear_coefficients().get(compact_offset) != Some(coefficient) {
                return Err(GeneratedAffineParametricOrderingError::GeometryBindingMismatch);
            }
            let coefficient_bits = integer_magnitude_bits(coefficient)
                .map_err(GeneratedAffineParametricOrderingError::Ordering)?;
            check_limit(
                "affine integer bits",
                coefficient_bits,
                limits.max_affine_integer_bits,
            )?;
            largest_affine_integer_bits = largest_affine_integer_bits.max(coefficient_bits);
            constant_row &= coefficient.is_zero();
        }
        if constant_row {
            constant_positions = checked_add("constant positions", constant_positions, 1)?;
            check_limit(
                "constant positions",
                constant_positions,
                limits.max_constant_positions,
            )?;
            constant_position_decimal_bytes = checked_add(
                "constant position decimal bytes",
                constant_position_decimal_bytes,
                usize_decimal_digits(position),
            )?;
            let exact_active = constant >= &Integer::from(1);
            let source_active = authority.sector().active_bits()[position];
            if exact_active != source_active {
                return Err(GeneratedAffineParametricOrderingError::Ordering(
                    AffineParametricOrderingError::ConstantStartOutsideSourceSector {
                        position,
                        constant_integer_bits: constant_bits,
                        constant_is_negative: constant.is_negative(),
                        source_active,
                    },
                ));
            }
        } else {
            symbolic_positions = checked_add("symbolic positions", symbolic_positions, 1)?;
            check_limit(
                "symbolic positions",
                symbolic_positions,
                limits.max_symbolic_positions,
            )?;
            symbolic_position_decimal_bytes = checked_add(
                "symbolic position decimal bytes",
                symbolic_position_decimal_bytes,
                usize_decimal_digits(position),
            )?;
        }
    }
    if checked_add(
        "classified positions",
        constant_positions,
        symbolic_positions,
    )? != arity
    {
        return Err(GeneratedAffineParametricOrderingError::GeometryBindingMismatch);
    }
    Ok(GeometryPreflight {
        constant_positions,
        symbolic_positions,
        constant_position_decimal_bytes,
        symbolic_position_decimal_bytes,
        matrix_entries_inspected,
        largest_affine_integer_bits,
    })
}

fn fill_classified_positions(
    geometry: &GeneratedCaseOrderingGeometry<'_>,
    constant_positions: &mut Vec<usize>,
    symbolic_positions: &mut Vec<usize>,
) -> Result<(), GeneratedAffineParametricOrderingError> {
    for position in 0..geometry.ambient_arity() {
        let mut constant_row = true;
        for free_ordinal in 0..geometry.free_positions().len() {
            constant_row &= geometry
                .linear_coefficient(position, free_ordinal)
                .ok_or(GeneratedAffineParametricOrderingError::GeometryBindingMismatch)?
                .is_zero();
        }
        if constant_row {
            constant_positions.push(position);
        } else {
            symbolic_positions.push(position);
        }
    }
    Ok(())
}

fn count_source_identity(
    authority: &GeneratedAffineResidualCaseAuthority,
    case: GeneratedAffineResidualCaseSourceRecordView<'_>,
    group: GeneratedAffineResidualInventoryGroupSourceView<'_>,
) -> Result<usize, GeneratedAffineParametricOrderingError> {
    if matches!(
        authority.source_kind(),
        GeneratedAffineResidualCaseAuthoritySourceKind::DirectFormulaSingleton
            | GeneratedAffineResidualCaseAuthoritySourceKind::CommittedExceptionalSingleton
    ) {
        return authority
            .stable_value_identity()
            .filter(|identity| identity.kind() == authority.source_kind())
            .map(|identity| identity.bytes().len())
            .ok_or(GeneratedAffineParametricOrderingError::CaseBindingMismatch);
    }
    let mut output = ByteCounter::default();
    write_source_identity_with(&mut output, authority, case, group, |output, value| {
        let exact_bytes = identity_integer_bytes(value).map_err(|_| fmt::Error)?;
        output
            .add_exact(exact_bytes, "source identity bytes")
            .map_err(|_| fmt::Error)
    })
    .map_err(|_| output.error("source identity bytes"))?;
    output.finish("source identity bytes")
}

fn write_source_identity(
    output: &mut impl fmt::Write,
    authority: &GeneratedAffineResidualCaseAuthority,
    case: GeneratedAffineResidualCaseSourceRecordView<'_>,
    group: GeneratedAffineResidualInventoryGroupSourceView<'_>,
) -> fmt::Result {
    if matches!(
        authority.source_kind(),
        GeneratedAffineResidualCaseAuthoritySourceKind::DirectFormulaSingleton
            | GeneratedAffineResidualCaseAuthoritySourceKind::CommittedExceptionalSingleton
    ) {
        return output.write_str(
            authority
                .stable_value_identity()
                .filter(|identity| identity.kind() == authority.source_kind())
                .map(|identity| identity.bytes())
                .ok_or(fmt::Error)?,
        );
    }
    write_source_identity_with(output, authority, case, group, |output, value| {
        write_identity_integer(output, value)
    })
}

/// Write the generated case identity with its complete affine geometry.
///
/// The caller selects how integers are emitted.  The counting path adds their
/// exact sign-plus-hex byte lengths without invoking a GMP formatter; the
/// rendering path is reached only after both the identity and whole-manifest
/// byte ceilings have admitted those lengths.
fn write_source_identity_with<O, F>(
    output: &mut O,
    authority: &GeneratedAffineResidualCaseAuthority,
    case: GeneratedAffineResidualCaseSourceRecordView<'_>,
    group: GeneratedAffineResidualInventoryGroupSourceView<'_>,
    write_integer: F,
) -> fmt::Result
where
    O: fmt::Write,
    F: FnMut(&mut O, &Integer) -> fmt::Result,
{
    write_source_identity_parts_with(
        output,
        authority,
        case.constants(),
        group.free_positions(),
        group.compact_linear_coefficients(),
        write_integer,
    )
}

fn write_source_identity_parts_with<O, F>(
    output: &mut O,
    authority: &GeneratedAffineResidualCaseAuthority,
    constants: &[Integer],
    free_positions: &[usize],
    coefficients: &[Integer],
    mut write_integer: F,
) -> fmt::Result
where
    O: fmt::Write,
    F: FnMut(&mut O, &Integer) -> fmt::Result,
{
    write!(
        output,
        "generated-affine-case-authority-v2|integer-encoding=sign-magnitude-hex-v1|family-bytes={}:{}|context-bytes={}:{}|sector={}|policy={}|case={}|group={}|arity={}|source-rows={}|geometry=arity:{}|free:{}[",
        authority.family_fingerprint().len(),
        authority.family_fingerprint(),
        authority.context_fingerprint().len(),
        authority.context_fingerprint(),
        authority.sector(),
        authority.ordering().stable_id(),
        authority.case_ordinal(),
        authority.group_ordinal(),
        authority.arity(),
        authority.source_row_count(),
        authority.arity(),
        free_positions.len(),
    )?;
    write_positions(output, free_positions)?;
    write!(output, "]|b:{}[", constants.len())?;
    for (ordinal, value) in constants.iter().enumerate() {
        if ordinal != 0 {
            output.write_char(',')?;
        }
        write_integer(output, value)?;
    }
    write!(
        output,
        "]|A:{},{}[",
        authority.arity(),
        free_positions.len()
    )?;
    for row in 0..authority.arity() {
        if row != 0 {
            output.write_char(';')?;
        }
        for free_ordinal in 0..free_positions.len() {
            if free_ordinal != 0 {
                output.write_char(',')?;
            }
            let offset = row
                .checked_mul(free_positions.len())
                .and_then(|start| start.checked_add(free_ordinal))
                .ok_or(fmt::Error)?;
            let value = coefficients.get(offset).ok_or(fmt::Error)?;
            write_integer(output, value)?;
        }
    }
    output.write_char(']')
}

fn identity_integer_bytes(
    value: &Integer,
) -> Result<usize, GeneratedAffineParametricOrderingError> {
    let bits =
        integer_magnitude_bits(value).map_err(GeneratedAffineParametricOrderingError::Ordering)?;
    let digits = if bits == 0 {
        1
    } else {
        checked_add("source identity bytes", bits, 3)? / 4
    };
    checked_add(
        "source identity bytes",
        digits,
        usize::from(value.is_negative()),
    )
}

fn write_identity_integer(output: &mut impl fmt::Write, value: &Integer) -> fmt::Result {
    match value {
        Integer::Single(value) => {
            if value.is_negative() {
                output.write_char('-')?;
            }
            write!(output, "{:x}", value.unsigned_abs())
        }
        Integer::Double(value) => {
            if value.is_negative() {
                output.write_char('-')?;
            }
            write!(output, "{:x}", value.unsigned_abs())
        }
        Integer::Large(value) => {
            if value.is_negative() {
                output.write_char('-')?;
            }
            write!(output, "{:x}", value.as_abs())
        }
    }
}

fn count_manifest(
    schema: &'static str,
    authority: &GeneratedAffineResidualCaseAuthority,
    case: GeneratedAffineResidualCaseSourceRecordView<'_>,
    group: GeneratedAffineResidualInventoryGroupSourceView<'_>,
    limits: GeneratedAffineParametricOrderingLimits,
    preflight: &GeometryPreflight,
    source_identity_bytes: usize,
) -> Result<usize, GeneratedAffineParametricOrderingError> {
    let mut output = ByteCounter::default();
    write_manifest_prefix(&mut output, schema, authority, source_identity_bytes)
        .map_err(|_| output.error("manifest bytes"))?;
    output.add_exact(source_identity_bytes, "manifest bytes")?;
    write_manifest_suffix(&mut output, authority, case, group, limits)
        .map_err(|_| output.error("manifest bytes"))?;
    output.add_exact(
        position_payload_bytes(
            preflight.constant_positions,
            preflight.constant_position_decimal_bytes,
        )?,
        "manifest bytes",
    )?;
    output
        .write_str("]|symbolic=[")
        .map_err(|_| output.error("manifest bytes"))?;
    output.add_exact(
        position_payload_bytes(
            preflight.symbolic_positions,
            preflight.symbolic_position_decimal_bytes,
        )?,
        "manifest bytes",
    )?;
    output
        .write_char(']')
        .map_err(|_| output.error("manifest bytes"))?;
    output.finish("manifest bytes")
}

fn render_manifest(
    authority: &GeneratedAffineResidualCaseAuthority,
    schema: &'static str,
    case: GeneratedAffineResidualCaseSourceRecordView<'_>,
    group: GeneratedAffineResidualInventoryGroupSourceView<'_>,
    limits: GeneratedAffineParametricOrderingLimits,
    constant_positions: &[usize],
    symbolic_positions: &[usize],
    source_identity_bytes: usize,
    exact_bytes: usize,
) -> Result<String, GeneratedAffineParametricOrderingError> {
    let mut output = String::new();
    output.try_reserve_exact(exact_bytes).map_err(|_| {
        GeneratedAffineParametricOrderingError::AllocationFailure {
            resource: "manifest bytes",
            requested: exact_bytes,
        }
    })?;
    write_manifest_prefix(&mut output, schema, authority, source_identity_bytes).map_err(|_| {
        GeneratedAffineParametricOrderingError::AllocationFailure {
            resource: "manifest bytes",
            requested: exact_bytes,
        }
    })?;
    write_source_identity(&mut output, authority, case, group).map_err(|_| {
        GeneratedAffineParametricOrderingError::AllocationFailure {
            resource: "manifest bytes",
            requested: exact_bytes,
        }
    })?;
    write_manifest_suffix(&mut output, authority, case, group, limits).map_err(|_| {
        GeneratedAffineParametricOrderingError::AllocationFailure {
            resource: "manifest bytes",
            requested: exact_bytes,
        }
    })?;
    write_positions(&mut output, constant_positions).map_err(|_| {
        GeneratedAffineParametricOrderingError::AllocationFailure {
            resource: "manifest bytes",
            requested: exact_bytes,
        }
    })?;
    output.push_str("]|symbolic=[");
    write_positions(&mut output, symbolic_positions).map_err(|_| {
        GeneratedAffineParametricOrderingError::AllocationFailure {
            resource: "manifest bytes",
            requested: exact_bytes,
        }
    })?;
    output.push(']');
    if output.len() != exact_bytes {
        return Err(GeneratedAffineParametricOrderingError::GeometryBindingMismatch);
    }
    Ok(output)
}

fn write_manifest_prefix(
    output: &mut impl fmt::Write,
    schema: &'static str,
    authority: &GeneratedAffineResidualCaseAuthority,
    source_identity_bytes: usize,
) -> fmt::Result {
    write!(
        output,
        "{schema}|key-schema={AFFINE_START_INTEGRAL_COMPLEXITY_KEY_V1_SCHEMA}:{}",
        RUSTRED_AFFINE_START_UNSHIFTED_ORDER_V1_KEY_SCHEMA,
    )?;
    if matches!(
        authority.source_kind(),
        GeneratedAffineResidualCaseAuthoritySourceKind::DirectFormulaSingleton
            | GeneratedAffineResidualCaseAuthoritySourceKind::CommittedExceptionalSingleton
    ) {
        let identity = authority
            .stable_value_identity()
            .filter(|identity| identity.kind() == authority.source_kind())
            .ok_or(fmt::Error)?;
        write!(
            output,
            "|source-identity-kind={}|source-identity-schema-bytes={}:{}",
            identity.kind().stable_id(),
            identity.schema().len(),
            identity.schema(),
        )?;
    }
    write!(output, "|source-identity-bytes={source_identity_bytes}:")
}

fn write_manifest_suffix(
    output: &mut impl fmt::Write,
    authority: &GeneratedAffineResidualCaseAuthority,
    case: GeneratedAffineResidualCaseSourceRecordView<'_>,
    group: GeneratedAffineResidualInventoryGroupSourceView<'_>,
    limits: GeneratedAffineParametricOrderingLimits,
) -> fmt::Result {
    let ordering = limits.ordering;
    match case.locator() {
        GeneratedAffineResidualCaseSourceLocator::Legacy(locator)
            if authority.source_kind()
                == GeneratedAffineResidualCaseAuthoritySourceKind::InitialInventory =>
        {
            write!(output, "|terminal={}", locator.boolean_record_ordinal())?;
        }
        GeneratedAffineResidualCaseSourceLocator::DirectFormula { .. }
            if authority.source_kind()
                == GeneratedAffineResidualCaseAuthoritySourceKind::DirectFormulaSingleton =>
        {
            output.write_str("|source=direct-formula-singleton")?;
        }
        GeneratedAffineResidualCaseSourceLocator::CommittedExceptional { .. }
            if authority.source_kind()
            == GeneratedAffineResidualCaseAuthoritySourceKind::CommittedExceptionalSingleton =>
        {
            output.write_str("|source=committed-exceptional-singleton")?;
        }
        _ => return Err(fmt::Error),
    }
    write!(
        output,
        "|case-within-group={}|group-cases={}|anchor-case={}|free=[",
        case.ordinal_within_group(),
        group.case_ordinals().len(),
        group.anchor_case_ordinal(),
    )?;
    write_positions(output, group.free_positions())?;
    write!(
        output,
        "]|limits={},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}|schema-bytes={}|constant=[",
        ordering.max_arity,
        ordering.max_free_positions,
        ordering.max_constant_positions,
        ordering.max_symbolic_positions,
        ordering.max_matrix_entries_inspected,
        ordering.max_key_components,
        ordering.max_affine_integer_bits,
        ordering.max_key_integer_bits,
        ordering.max_key_total_integer_bits,
        ordering.max_map_identity_bytes,
        ordering.max_manifest_bytes,
        ordering.max_key_diagnostic_bytes,
        limits.max_authority_replays,
        limits.max_case_view_resolutions,
        limits.max_group_view_resolutions,
        limits.max_case_group_membership_checks,
        limits.max_group_case_references,
        limits.max_source_rows,
        limits.max_retained_authority_references,
        ordering_schema_for_source(authority.source_kind()).len(),
    )
}

fn write_positions(output: &mut impl fmt::Write, positions: &[usize]) -> fmt::Result {
    for (ordinal, position) in positions.iter().enumerate() {
        if ordinal != 0 {
            output.write_char(',')?;
        }
        write!(output, "{position}")?;
    }
    Ok(())
}

#[derive(Default)]
struct ByteCounter {
    bytes: usize,
    overflowed: bool,
}

impl ByteCounter {
    fn add_exact(
        &mut self,
        bytes: usize,
        resource: &'static str,
    ) -> Result<(), GeneratedAffineParametricOrderingError> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or(GeneratedAffineParametricOrderingError::ResourceCountOverflow { resource })?;
        Ok(())
    }

    fn finish(
        self,
        resource: &'static str,
    ) -> Result<usize, GeneratedAffineParametricOrderingError> {
        if self.overflowed {
            Err(GeneratedAffineParametricOrderingError::ResourceCountOverflow { resource })
        } else {
            Ok(self.bytes)
        }
    }

    fn error(&self, resource: &'static str) -> GeneratedAffineParametricOrderingError {
        GeneratedAffineParametricOrderingError::ResourceCountOverflow { resource }
    }
}

impl fmt::Write for ByteCounter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        match self.bytes.checked_add(value.len()) {
            Some(bytes) => {
                self.bytes = bytes;
                Ok(())
            }
            None => {
                self.overflowed = true;
                Err(fmt::Error)
            }
        }
    }
}

fn position_payload_bytes(
    count: usize,
    decimal_bytes: usize,
) -> Result<usize, GeneratedAffineParametricOrderingError> {
    checked_add(
        "position payload bytes",
        decimal_bytes,
        count.saturating_sub(1),
    )
}

fn usize_decimal_digits(mut value: usize) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineParametricOrderingError> {
    left.checked_add(right)
        .ok_or(GeneratedAffineParametricOrderingError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineParametricOrderingError> {
    left.checked_mul(right)
        .ok_or(GeneratedAffineParametricOrderingError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineParametricOrderingError> {
    if requested > limit {
        Err(GeneratedAffineParametricOrderingError::ResourceLimit {
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
    use std::sync::{Arc, Weak};

    use super::*;
    use crate::generated_affine_residual_boolean_cover::{
        GeneratedAffineResidualBooleanCoverCompiler, GeneratedAffineResidualBooleanCoverLimits,
    };
    use crate::generated_affine_residual_source_authority::GeneratedAffineResidualSourceAuthority;
    use crate::solver::closure::case_inventory::{
        GeneratedAffineResidualCaseAuthorityLimits,
        GeneratedAffineResidualCaseInventoryCertificate,
        GeneratedAffineResidualCaseInventoryCompiler, GeneratedAffineResidualCaseInventoryLimits,
        GeneratedAffineResidualInventoryCaseSourceRecordView,
    };
    use crate::{
        AffineDenominator, CoefficientContext, GeneratedSectorDiscoveryCompiler,
        GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCompiler,
        GeneratedSectorLiveLeafQueueLimits, ParametricIbpGenerator,
    };

    fn equal_mass_two_loop_family(name: &str) -> IntegralFamily {
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

    fn fixture(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedAffineResidualCaseInventoryCertificate>,
        Arc<GeneratedAffineResidualCaseAuthority>,
    ) {
        let family = equal_mass_two_loop_family(name);
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_from_bit_string("011").unwrap(),
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
        let source = GeneratedAffineResidualSourceAuthority::initial_global(queue);
        let boolean = Arc::new(
            GeneratedAffineResidualBooleanCoverCompiler::compile(
                &family,
                &context,
                source,
                GeneratedAffineResidualBooleanCoverLimits::default(),
            )
            .unwrap(),
        );
        let inventory = Arc::new(
            GeneratedAffineResidualCaseInventoryCompiler::compile(
                &family,
                &context,
                boolean,
                GeneratedAffineResidualCaseInventoryLimits::default(),
            )
            .unwrap(),
        );
        let case_ordinal = (0..inventory.case_count())
            .max_by_key(|&ordinal| {
                inventory
                    .authenticated_case_view(&context, ordinal)
                    .unwrap()
                    .source()
                    .affine_map()
                    .free_positions()
                    .len()
            })
            .expect("natural two-loop fixture has an actionable affine case");
        let authority = Arc::new(
            GeneratedAffineResidualCaseAuthority::try_new(
                &family,
                &context,
                Arc::clone(&inventory),
                case_ordinal,
                GeneratedAffineResidualCaseAuthorityLimits::default(),
            )
            .unwrap(),
        );
        (family, context, inventory, authority)
    }

    fn exact_limits(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
    ) -> (
        GeneratedAffineParametricOrderingLimits,
        GeneratedAffineParametricOrderingStats,
    ) {
        let mut limits = GeneratedAffineParametricOrderingLimits::default();
        for _ in 0..8 {
            let ordering = GeneratedAffineParametricOrderingCertificate::try_new(
                family,
                context,
                Arc::clone(authority),
                limits,
            )
            .unwrap();
            let stats = ordering.stats();
            let mut next = limits;
            next.max_authority_replays = stats.authority_replays();
            next.max_case_view_resolutions = stats.case_view_resolutions();
            next.max_group_view_resolutions = stats.group_view_resolutions();
            next.max_case_group_membership_checks = stats.case_group_membership_checks();
            next.max_group_case_references = stats.group_case_references();
            next.max_source_rows = stats.source_rows();
            next.max_retained_authority_references = stats.retained_authority_references();
            next.ordering.max_arity = stats.ambient_arity();
            next.ordering.max_free_positions = stats.free_positions();
            next.ordering.max_constant_positions = stats.constant_positions();
            next.ordering.max_symbolic_positions = stats.symbolic_positions();
            next.ordering.max_matrix_entries_inspected = stats.matrix_entries_inspected();
            next.ordering.max_key_components = key_component_count(stats.ambient_arity()).unwrap();
            next.ordering.max_affine_integer_bits = stats.largest_affine_integer_bits();
            next.ordering.max_map_identity_bytes = stats.source_identity_bytes();
            next.ordering.max_manifest_bytes = stats.manifest_bytes();
            if next == limits {
                return (limits, stats);
            }
            limits = next;
        }
        panic!("exact generated ordering manifest limit did not converge");
    }

    fn assert_resource_rejected(
        result: Result<
            GeneratedAffineParametricOrderingCertificate,
            GeneratedAffineParametricOrderingError,
        >,
    ) {
        assert!(matches!(
            result,
            Err(GeneratedAffineParametricOrderingError::ResourceLimit { .. })
                | Err(GeneratedAffineParametricOrderingError::Ordering(
                    AffineParametricOrderingError::ResourceLimit { .. }
                ))
        ));
    }

    struct ForgedGeometry<'geometry> {
        arity: usize,
        constants: &'geometry [Integer],
        free_positions: &'geometry [usize],
        compact_linear_coefficients: &'geometry [Integer],
    }

    impl<'geometry> AffineParametricOrderingGeometry<'geometry> for ForgedGeometry<'geometry> {
        fn ambient_arity(&self) -> usize {
            self.arity
        }

        fn free_positions(&self) -> &'geometry [usize] {
            self.free_positions
        }

        fn constant(&self, position: usize) -> Option<&'geometry Integer> {
            self.constants.get(position)
        }

        fn linear_coefficient(
            &self,
            position: usize,
            free_ordinal: usize,
        ) -> Option<&'geometry Integer> {
            position
                .checked_mul(self.free_positions.len())
                .and_then(|offset| offset.checked_add(free_ordinal))
                .and_then(|offset| self.compact_linear_coefficients.get(offset))
        }
    }

    fn rendered_source_identity_from_parts(
        authority: &GeneratedAffineResidualCaseAuthority,
        constants: &[Integer],
        free_positions: &[usize],
        compact_linear_coefficients: &[Integer],
    ) -> String {
        let mut output = String::new();
        write_source_identity_parts_with(
            &mut output,
            authority,
            constants,
            free_positions,
            compact_linear_coefficients,
            |output, value| write_identity_integer(output, value),
        )
        .unwrap();
        output
    }

    fn same_width_nonzero_mutation(value: &Integer) -> Integer {
        let exact_bytes = identity_integer_bytes(value).unwrap();
        for displacement in [1, -1] {
            let candidate = value.clone() + Integer::from(displacement);
            if !candidate.is_zero()
                && candidate.is_negative() == value.is_negative()
                && identity_integer_bytes(&candidate).unwrap() == exact_bytes
            {
                return candidate;
            }
        }
        panic!("nonzero sign-magnitude integer has a same-width neighbor");
    }

    #[test]
    fn exact_limits_pointer_bound_replay_and_redaction() {
        let (family, context, inventory, authority) =
            fixture("generated-affine-ordering-v2-limits-private");
        let (exact, stats) = exact_limits(&family, &context, &authority);
        let ordering = GeneratedAffineParametricOrderingCertificate::try_new(
            &family,
            &context,
            Arc::clone(&authority),
            exact,
        )
        .unwrap();
        assert_eq!(ordering.stats(), stats);
        assert_eq!(
            ordering.schema(),
            GENERATED_AFFINE_PARAMETRIC_ORDERING_V2_SCHEMA
        );
        assert_eq!(
            ordering.key_schema(),
            RUSTRED_AFFINE_START_UNSHIFTED_ORDER_V1_KEY_SCHEMA
        );
        assert_eq!(ordering.case_ordinal(), authority.case_ordinal());
        assert_eq!(ordering.group_ordinal(), authority.group_ordinal());
        assert_eq!(ordering.policy(), authority.ordering());
        assert_eq!(ordering.sector(), authority.sector());
        assert_eq!(
            ordering.family_fingerprint(),
            authority.family_fingerprint()
        );
        assert_eq!(
            ordering.context_fingerprint(),
            authority.context_fingerprint()
        );
        assert_eq!(ordering.arity(), authority.arity());
        assert_eq!(ordering.limits(), exact);
        assert_eq!(
            ordering.constant_positions().len() + ordering.symbolic_positions().len(),
            ordering.arity()
        );
        assert_eq!(ordering.stable_manifest().len(), stats.manifest_bytes());

        macro_rules! outer_one_below {
            ($field:ident, $stat:ident) => {
                if stats.$stat() > 0 {
                    let mut limits = exact;
                    limits.$field = stats.$stat() - 1;
                    assert_resource_rejected(
                        GeneratedAffineParametricOrderingCertificate::try_new(
                            &family,
                            &context,
                            Arc::clone(&authority),
                            limits,
                        ),
                    );
                }
            };
        }
        outer_one_below!(max_authority_replays, authority_replays);
        outer_one_below!(max_case_view_resolutions, case_view_resolutions);
        outer_one_below!(max_group_view_resolutions, group_view_resolutions);
        outer_one_below!(
            max_case_group_membership_checks,
            case_group_membership_checks
        );
        outer_one_below!(max_group_case_references, group_case_references);
        outer_one_below!(max_source_rows, source_rows);
        outer_one_below!(
            max_retained_authority_references,
            retained_authority_references
        );

        macro_rules! ordering_one_below {
            ($field:ident, $stat:ident) => {
                if stats.$stat() > 0 {
                    let mut limits = exact;
                    limits.ordering.$field = stats.$stat() - 1;
                    assert_resource_rejected(
                        GeneratedAffineParametricOrderingCertificate::try_new(
                            &family,
                            &context,
                            Arc::clone(&authority),
                            limits,
                        ),
                    );
                }
            };
        }
        ordering_one_below!(max_arity, ambient_arity);
        ordering_one_below!(max_free_positions, free_positions);
        ordering_one_below!(max_constant_positions, constant_positions);
        ordering_one_below!(max_symbolic_positions, symbolic_positions);
        ordering_one_below!(max_matrix_entries_inspected, matrix_entries_inspected);
        ordering_one_below!(max_affine_integer_bits, largest_affine_integer_bits);
        ordering_one_below!(max_map_identity_bytes, source_identity_bytes);
        ordering_one_below!(max_manifest_bytes, manifest_bytes);
        let key_components = key_component_count(stats.ambient_arity()).unwrap();
        let mut key_component_one_below = exact;
        key_component_one_below.ordering.max_key_components = key_components - 1;
        assert_resource_rejected(GeneratedAffineParametricOrderingCertificate::try_new(
            &family,
            &context,
            Arc::clone(&authority),
            key_component_one_below,
        ));

        let weak: Weak<GeneratedAffineResidualCaseAuthority> = Arc::downgrade(&authority);
        let exact_authority = Arc::clone(&authority);
        drop(authority);
        drop(inventory);
        assert!(weak.upgrade().is_some());
        ordering
            .replay(&family, &context, &exact_authority)
            .unwrap();

        let independent = Arc::new((*exact_authority).clone());
        assert!(!Arc::ptr_eq(&exact_authority, &independent));
        assert!(matches!(
            ordering.replay(&family, &context, &independent),
            Err(GeneratedAffineParametricOrderingError::AuthorityAllocationMismatch)
        ));
        let mut wrong_case = ordering.clone();
        wrong_case.case_ordinal = wrong_case.case_ordinal.saturating_add(1);
        assert!(matches!(
            wrong_case.replay(&family, &context, &exact_authority),
            Err(GeneratedAffineParametricOrderingError::CaseBindingMismatch)
        ));
        let mut wrong_group = ordering.clone();
        wrong_group.group_ordinal = wrong_group.group_ordinal.saturating_add(1);
        assert!(matches!(
            wrong_group.replay(&family, &context, &exact_authority),
            Err(GeneratedAffineParametricOrderingError::GroupBindingMismatch)
        ));
        let mut wrong_group_position = ordering.clone();
        wrong_group_position.ordinal_within_group =
            wrong_group_position.ordinal_within_group.saturating_add(1);
        assert!(
            wrong_group_position
                .replay(&family, &context, &exact_authority)
                .is_err()
        );

        let debug = format!("{ordering:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("m2"));
        drop(wrong_group_position);
        drop(wrong_group);
        drop(wrong_case);
        drop(ordering);
        drop(exact_authority);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn exact_geometry_identity_rejects_same_metadata_matrix_substitution() {
        let (family, context, _inventory, authority) =
            fixture("generated-affine-ordering-v2-geometry-identity-private");
        let ordering = GeneratedAffineParametricOrderingCertificate::try_new(
            &family,
            &context,
            Arc::clone(&authority),
            GeneratedAffineParametricOrderingLimits::default(),
        )
        .unwrap();
        let case = authority.authenticated_case_view(&context).unwrap();
        let group = authority.authenticated_group_view(&context).unwrap();
        let original_identity = rendered_source_identity_from_parts(
            authority.as_ref(),
            case.constants(),
            group.free_positions(),
            group.compact_linear_coefficients(),
        );
        assert_eq!(
            original_identity.len(),
            ordering.stats().source_identity_bytes()
        );
        assert!(ordering.stable_manifest().contains(&original_identity));
        assert!(original_identity.contains("|geometry=arity:"));
        assert!(original_identity.contains("|b:"));
        assert!(original_identity.contains("|A:"));

        // Change a deep nonzero coefficient without changing arity, free
        // positions, case/group ordinals, or any constant/symbolic row class.
        // This is precisely the substitution that the former metadata-only
        // identity could not distinguish.
        let free_count = group.free_positions().len();
        assert!(free_count > 0);
        let mut forged_coefficients = group.compact_linear_coefficients().to_vec();
        let changed_offset = forged_coefficients
            .iter()
            .position(|coefficient| !coefficient.is_zero())
            .expect("actionable fixture has a symbolic affine row");
        forged_coefficients[changed_offset] =
            same_width_nonzero_mutation(&forged_coefficients[changed_offset]);
        let changed_row = changed_offset / free_count;
        assert!(
            group.compact_linear_coefficients()
                [changed_row * free_count..(changed_row + 1) * free_count]
                .iter()
                .any(|coefficient| !coefficient.is_zero())
        );
        assert!(
            forged_coefficients[changed_row * free_count..(changed_row + 1) * free_count]
                .iter()
                .any(|coefficient| !coefficient.is_zero())
        );

        let forged_identity = rendered_source_identity_from_parts(
            authority.as_ref(),
            case.constants(),
            group.free_positions(),
            &forged_coefficients,
        );
        assert_eq!(forged_identity.len(), original_identity.len());
        assert_ne!(forged_identity, original_identity);
        let forged_manifest = Arc::new(ordering.stable_manifest().replacen(
            &original_identity,
            &forged_identity,
            1,
        ));
        assert_eq!(forged_manifest.len(), ordering.stats().manifest_bytes());
        assert_ne!(forged_manifest.as_str(), ordering.stable_manifest());

        let zero_shift = IndexShift::try_new(vec![0; ordering.arity()], ordering.arity()).unwrap();
        let key = ordering.key_for_shift(&context, &zero_shift).unwrap();
        let forged_geometry = ForgedGeometry {
            arity: ordering.arity(),
            constants: case.constants(),
            free_positions: group.free_positions(),
            compact_linear_coefficients: &forged_coefficients,
        };
        let forged_algebra = AffineParametricOrderingAlgebra::new(
            ordering.policy(),
            ordering.sector(),
            &forged_geometry,
            ordering.constant_positions(),
            ordering.limits().ordering,
            &forged_manifest,
            ordering.key_schema(),
        );
        assert!(matches!(
            forged_algebra.replay_key(&key),
            Err(AffineParametricOrderingError::KeyOrderingMismatch)
        ));
        ordering.replay_key(&context, &key).unwrap();
    }

    fn exact_map_witness(
        case: GeneratedAffineResidualInventoryCaseSourceRecordView<'_>,
        sector: &SectorMask,
    ) -> Vec<i64> {
        let map = case.source().affine_map();
        let seed = sector
            .active_bits()
            .iter()
            .map(|&active| Integer::from(i64::from(active)))
            .collect::<Vec<_>>();
        (0..map.ambient_arity())
            .map(|row| {
                let mut image = map.constant(row).unwrap().clone();
                for (column, coordinate) in seed.iter().enumerate() {
                    let product = map.linear_coefficient(row, column).unwrap() * coordinate;
                    image = &image + &product;
                }
                image.to_i64().expect("natural fixture values fit i64")
            })
            .collect()
    }

    fn enumerate_shifts(position: usize, values: &mut [i64], output: &mut Vec<IndexShift>) {
        if position == values.len() {
            output.push(IndexShift::try_new(values.iter().copied(), values.len()).unwrap());
            return;
        }
        for value in -1..=1 {
            values[position] = value;
            enumerate_shifts(position + 1, values, output);
        }
    }

    #[test]
    fn formal_keys_match_an_independent_concrete_ordering_oracle() {
        let (family, context, _inventory, authority) =
            fixture("generated-affine-ordering-v2-concrete-oracle-private");
        let ordering = GeneratedAffineParametricOrderingCertificate::try_new(
            &family,
            &context,
            Arc::clone(&authority),
            GeneratedAffineParametricOrderingLimits::default(),
        )
        .unwrap();
        let case = authority.authenticated_case_view(&context).unwrap();
        let start = exact_map_witness(case, ordering.sector());
        assert_eq!(
            SectorMask::try_from_indices(&start).unwrap(),
            *ordering.sector()
        );
        let mut candidates = Vec::new();
        enumerate_shifts(0, &mut vec![0; ordering.arity()], &mut candidates);
        candidates.retain(|shift| {
            let point = start
                .iter()
                .zip(shift.values())
                .map(|(&index, &displacement)| index + displacement)
                .collect::<Vec<_>>();
            SectorMask::try_from_indices(&point).is_ok_and(|sector| sector == *ordering.sector())
        });
        assert!(candidates.len() >= 4);

        for left in &candidates {
            let left_point = start
                .iter()
                .zip(left.values())
                .map(|(&index, &displacement)| index + displacement)
                .collect::<Vec<_>>();
            let left_key = ordering.key_for_shift(&context, left).unwrap();
            ordering.replay_key(&context, &left_key).unwrap();
            for right in &candidates {
                let right_point = start
                    .iter()
                    .zip(right.values())
                    .map(|(&index, &displacement)| index + displacement)
                    .collect::<Vec<_>>();
                assert_eq!(
                    ordering.compare_shifts(&context, left, right).unwrap(),
                    ordering
                        .policy()
                        .compare(&left_point, &right_point)
                        .unwrap(),
                    "formal order differs from concrete oracle for {left_point:?} vs {right_point:?}",
                );
            }
        }

        for &position in ordering.constant_positions() {
            let value = ordering
                .constant_start_value(&context, position)
                .unwrap()
                .unwrap();
            assert_eq!(value.to_i64(), Some(start[position]));
            let classification = ordering
                .classify_constant_row_shift(&context, position, 0)
                .unwrap();
            assert_eq!(classification.position(), position);
            assert_eq!(
                classification.source_active(),
                ordering.sector().active_bits()[position]
            );
            assert_eq!(
                classification.shifted_active(),
                ordering.sector().active_bits()[position]
            );
        }
        ordering.replay(&family, &context, &authority).unwrap();
    }
}
