//! Topology-neutral refinement of one current-target affine equality.
//!
//! The input polynomial is already expressed in the current target
//! coordinates: its index support must be contained in the free positions of
//! the supplied parent compact map.  This is the representation emitted by
//! exact `WhenBad` materialization.  Consequently this compiler does not map
//! the equality through the parent geometry a second time.
//!
//! This first production-safe slice accepts exactly the integer-affine cases
//! having a literal unit coefficient on a current free coordinate.  Primitive
//! normalization is owned by [`ResidualAffineAtomRowCertificate`], affine-map
//! composition is owned by Symbolica's native integer matrix multiplication,
//! and polynomial substitution is owned by the existing compact composition
//! plan.  General primitive rows without a unit coefficient are reported as
//! requiring an integer normal-form API; this module never calls RustRed's
//! historical hand-written lattice solvers.
//!
//! The compiler is intentionally authority-neutral.  A subsequent adapter
//! will bind its refined certificate to a committed exceptional source before
//! regenerating generic IBP rows in a fresh child session.

use std::fmt;

use symbolica::domains::RingOps;
use symbolica::prelude::{Integer, Z};

use crate::parametric_coefficient::{
    ResidualAffineCompactCompositionPlan, ResidualAffineCompactCompositionPlanLimits,
    ResidualAffineCompactCompositionPlanStats, ResidualAffineCompactMapView,
    ResidualUnitAffineCompositionError, ResidualUnitAffinePolynomialCompositionLimits,
    ResidualUnitAffinePolynomialCompositionStats,
};
use crate::residual_affine_atom_rows::residual_affine_atom_row_attempt_logical_memory_census;
use crate::symbolica_coefficient_matrix::{
    SymbolicaIntegerMatrixEntryRef, SymbolicaIntegerMatrixError, SymbolicaIntegerMatrixLimits,
    SymbolicaIntegerMatrixStats, multiply_integer_matrices,
    preflight_integer_matrix_product_with_accessors,
};
use crate::{
    ParametricCoefficientContext, ParametricPolynomial, ResidualAffineAtomRowCertificate,
    ResidualAffineAtomRowError, ResidualAffineAtomRowLimits, ResidualAffineAtomRowOutcome,
    ResidualAffineAtomRowStats, ResidualAffineAtomRowUnsupported,
};

pub(crate) const GENERATED_AFFINE_RESIDUAL_CASE_UNIT_EQUALITY_REFINEMENT_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-case-unit-equality-refinement-v1";

/// Complete admission policy for one unit-pivot equality attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCaseUnitEqualityRefinementLimits {
    pub(crate) atom_row: ResidualAffineAtomRowLimits,
    pub(crate) compact_plan: ResidualAffineCompactCompositionPlanLimits,
    pub(crate) polynomial_composition: ResidualUnitAffinePolynomialCompositionLimits,
    pub(crate) integer_matrix: SymbolicaIntegerMatrixLimits,
    /// Number of supplied predicates whose complete polynomial payload may be
    /// authenticated before cardinality is classified.
    pub(crate) max_equal_zero_predicates_inspected: usize,
    /// Complete logical peak of the borrowed atom-row recognition preflight.
    pub(crate) max_atom_attempt_owned_logical_peak_upper_bound: usize,
    /// Conservative owned bytes of the one sparse equality payload before it
    /// is copied into the replayable atom-row certificate.
    pub(crate) max_equality_copy_retained_bytes: usize,
    /// Aggregate entries in the parent augmented matrix, the unit-pivot
    /// substitution matrix, and their product.  This is checked before the
    /// first matrix-shaped allocation in this module.
    pub(crate) max_temporary_integer_matrix_entries: usize,
    /// Aggregate scalar slots retained by the child constant vector, free
    /// position vector, and row-major compact linear matrix.
    pub(crate) max_retained_child_geometry_entries: usize,
    pub(crate) max_context_fingerprint_bytes: usize,
}

impl Default for GeneratedAffineResidualCaseUnitEqualityRefinementLimits {
    fn default() -> Self {
        Self {
            atom_row: ResidualAffineAtomRowLimits::default(),
            compact_plan: ResidualAffineCompactCompositionPlanLimits::default(),
            polynomial_composition: ResidualUnitAffinePolynomialCompositionLimits::default(),
            integer_matrix: SymbolicaIntegerMatrixLimits::default(),
            max_equal_zero_predicates_inspected: 4096,
            max_atom_attempt_owned_logical_peak_upper_bound: 64 * 1024 * 1024 * 1024,
            max_equality_copy_retained_bytes: 32 * 1024 * 1024 * 1024,
            max_temporary_integer_matrix_entries: 64_000_000,
            max_retained_child_geometry_entries: 32_000_000,
            max_context_fingerprint_bytes: 1024 * 1024,
        }
    }
}

/// Successful-path work retained beside a refined certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCaseUnitEqualityRefinementStats {
    parent_plan: ResidualAffineCompactCompositionPlanStats,
    atom_row: ResidualAffineAtomRowStats,
    integer_matrix: SymbolicaIntegerMatrixStats,
    child_plan: ResidualAffineCompactCompositionPlanStats,
    equality_verification: ResidualUnitAffinePolynomialCompositionStats,
    atom_attempt_owned_logical_peak_upper_bound: usize,
    equality_copy_retained_bytes: usize,
    prospective_integer_matrix_input_retained_bytes: usize,
    temporary_integer_matrix_entries: usize,
    retained_child_geometry_entries: usize,
}

impl GeneratedAffineResidualCaseUnitEqualityRefinementStats {
    pub(crate) const fn parent_plan(self) -> ResidualAffineCompactCompositionPlanStats {
        self.parent_plan
    }

    pub(crate) const fn atom_row(self) -> ResidualAffineAtomRowStats {
        self.atom_row
    }

    pub(crate) const fn integer_matrix(self) -> SymbolicaIntegerMatrixStats {
        self.integer_matrix
    }

    pub(crate) const fn child_plan(self) -> ResidualAffineCompactCompositionPlanStats {
        self.child_plan
    }

    pub(crate) const fn equality_verification(
        self,
    ) -> ResidualUnitAffinePolynomialCompositionStats {
        self.equality_verification
    }

    pub(crate) const fn temporary_integer_matrix_entries(self) -> usize {
        self.temporary_integer_matrix_entries
    }

    pub(crate) const fn atom_attempt_owned_logical_peak_upper_bound(self) -> usize {
        self.atom_attempt_owned_logical_peak_upper_bound
    }

    pub(crate) const fn equality_copy_retained_bytes(self) -> usize {
        self.equality_copy_retained_bytes
    }

    /// Sign-aware retained-byte envelope admitted before either virtual
    /// integer matrix is materialized.  It may exceed the native input census
    /// because negating inline `i128::MIN` promotes through GMP.
    pub(crate) const fn prospective_integer_matrix_input_retained_bytes(self) -> usize {
        self.prospective_integer_matrix_input_retained_bytes
    }

    pub(crate) const fn retained_child_geometry_entries(self) -> usize {
        self.retained_child_geometry_entries
    }
}

/// Completeness boundary for equality shapes not handled by this first slice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualCaseUnitEqualityRefinementUnsupported {
    MultipleEqualZeroPredicates {
        actual: usize,
    },
    NonAffineOrNonAssociate {
        reason: ResidualAffineAtomRowUnsupported,
    },
    RequiresIntegerNormalForm,
}

/// One classified current-target equality attempt.
#[derive(Debug)]
pub(crate) enum GeneratedAffineResidualCaseUnitEqualityRefinementOutcome {
    Refined(GeneratedAffineResidualCaseUnitEqualityRefinementCertificate),
    /// The empty conjunction, or an identically zero supplied equality.
    /// This authority-neutral classification is not branch-pruning evidence.
    AlreadySatisfied,
    /// The equality is a nonzero coefficient-field constant.
    /// A future committed-source adapter must retain/replay the atom-row proof
    /// before using this diagnostic classification as pruning authority.
    ProvedEmpty,
    Unsupported(GeneratedAffineResidualCaseUnitEqualityRefinementUnsupported),
}

/// Failures in authentication, bounded allocation, or native Symbolica work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualCaseUnitEqualityRefinementError {
    SchemaMismatch,
    ReplayMismatch,
    CurrentTargetCoordinateViolation {
        position: usize,
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
    },
    AtomRow(ResidualAffineAtomRowError),
    Composition(ResidualUnitAffineCompositionError),
    IntegerMatrix(SymbolicaIntegerMatrixError),
}

impl fmt::Display for GeneratedAffineResidualCaseUnitEqualityRefinementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("unit-equality refinement schema mismatch"),
            Self::ReplayMismatch => formatter.write_str("unit-equality refinement did not replay"),
            Self::CurrentTargetCoordinateViolation { position } => write!(
                formatter,
                "current-target equality uses nonfree index coordinate {position}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "unit-equality {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "unit-equality {resource} count overflowed usize")
            }
            Self::AllocationFailure { resource } => {
                write!(formatter, "unit-equality allocation failed for {resource}")
            }
            Self::AtomRow(error) => error.fmt(formatter),
            Self::Composition(error) => error.fmt(formatter),
            Self::IntegerMatrix(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedAffineResidualCaseUnitEqualityRefinementError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::AtomRow(error) => Some(error),
            Self::Composition(error) => Some(error),
            Self::IntegerMatrix(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ResidualAffineAtomRowError> for GeneratedAffineResidualCaseUnitEqualityRefinementError {
    fn from(value: ResidualAffineAtomRowError) -> Self {
        Self::AtomRow(value)
    }
}

impl From<ResidualUnitAffineCompositionError>
    for GeneratedAffineResidualCaseUnitEqualityRefinementError
{
    fn from(value: ResidualUnitAffineCompositionError) -> Self {
        Self::Composition(value)
    }
}

impl From<SymbolicaIntegerMatrixError> for GeneratedAffineResidualCaseUnitEqualityRefinementError {
    fn from(value: SymbolicaIntegerMatrixError) -> Self {
        Self::IntegerMatrix(value)
    }
}

/// Replayable, authority-neutral direct child geometry for one unit equality.
#[derive(Clone, Debug)]
pub(crate) struct GeneratedAffineResidualCaseUnitEqualityRefinementCertificate {
    schema: &'static str,
    context_fingerprint: String,
    atom_row: ResidualAffineAtomRowCertificate,
    pivot_free_ordinal: usize,
    pivot_ambient_position: usize,
    child_constants: Vec<Integer>,
    child_free_positions: Vec<usize>,
    child_compact_linear_coefficients: Vec<Integer>,
    parent_plan: ResidualAffineCompactCompositionPlan,
    child_plan: ResidualAffineCompactCompositionPlan,
    limits: GeneratedAffineResidualCaseUnitEqualityRefinementLimits,
    stats: GeneratedAffineResidualCaseUnitEqualityRefinementStats,
}

impl GeneratedAffineResidualCaseUnitEqualityRefinementCertificate {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) fn equality(&self) -> &ParametricPolynomial {
        self.atom_row.source()
    }

    pub(crate) const fn atom_row(&self) -> &ResidualAffineAtomRowCertificate {
        &self.atom_row
    }

    pub(crate) const fn pivot_free_ordinal(&self) -> usize {
        self.pivot_free_ordinal
    }

    pub(crate) const fn pivot_ambient_position(&self) -> usize {
        self.pivot_ambient_position
    }

    pub(crate) fn child_geometry(&self) -> ResidualAffineCompactMapView<'_> {
        ResidualAffineCompactMapView::new(
            &self.context_fingerprint,
            self.child_constants.len(),
            &self.child_constants,
            &self.child_free_positions,
            &self.child_compact_linear_coefficients,
        )
    }

    pub(crate) const fn child_plan(&self) -> &ResidualAffineCompactCompositionPlan {
        &self.child_plan
    }

    pub(crate) const fn limits(&self) -> GeneratedAffineResidualCaseUnitEqualityRefinementLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> GeneratedAffineResidualCaseUnitEqualityRefinementStats {
        self.stats
    }

    /// Reauthenticate the parent geometry, primitive row, native matrix
    /// product, child geometry, and final zero substitution.
    pub(crate) fn replay(
        &self,
        context: &ParametricCoefficientContext,
        parent_geometry: ResidualAffineCompactMapView<'_>,
    ) -> Result<(), GeneratedAffineResidualCaseUnitEqualityRefinementError> {
        if self.schema != GENERATED_AFFINE_RESIDUAL_CASE_UNIT_EQUALITY_REFINEMENT_V1_SCHEMA {
            return Err(GeneratedAffineResidualCaseUnitEqualityRefinementError::SchemaMismatch);
        }
        if self.context_fingerprint != context.fingerprint() {
            return Err(GeneratedAffineResidualCaseUnitEqualityRefinementError::ReplayMismatch);
        }
        self.parent_plan.replay(context, parent_geometry)?;
        self.atom_row.replay(context)?;
        let atom_attempt_census = residual_affine_atom_row_attempt_logical_memory_census(
            context,
            self.atom_row.source(),
            self.limits.atom_row,
        )?;
        let equality_copy_retained_bytes =
            self.atom_row.source().owned_retained_byte_bound().ok_or(
                GeneratedAffineResidualCaseUnitEqualityRefinementError::ResourceCountOverflow {
                    resource: "equality copy retained bytes",
                },
            )?;
        check_limit(
            "atom attempt owned logical peak upper bound",
            atom_attempt_census.owned_logical_peak_upper_bound(),
            self.limits.max_atom_attempt_owned_logical_peak_upper_bound,
        )?;
        check_limit(
            "equality copy retained bytes",
            equality_copy_retained_bytes,
            self.limits.max_equality_copy_retained_bytes,
        )?;
        if atom_attempt_census.owned_logical_peak_upper_bound()
            != self.stats.atom_attempt_owned_logical_peak_upper_bound
            || equality_copy_retained_bytes != self.stats.equality_copy_retained_bytes
        {
            return Err(GeneratedAffineResidualCaseUnitEqualityRefinementError::ReplayMismatch);
        }
        let row = self
            .atom_row
            .row()
            .ok_or(GeneratedAffineResidualCaseUnitEqualityRefinementError::ReplayMismatch)?;
        authenticate_current_target_support(row.coefficients(), parent_geometry.free_positions())?;
        let replayed_pivot =
            select_unit_pivot(row.coefficients(), parent_geometry.free_positions())
                .ok_or(GeneratedAffineResidualCaseUnitEqualityRefinementError::ReplayMismatch)?;
        if replayed_pivot != (self.pivot_free_ordinal, self.pivot_ambient_position) {
            return Err(GeneratedAffineResidualCaseUnitEqualityRefinementError::ReplayMismatch);
        }
        let replayed = derive_child_geometry(
            parent_geometry,
            row.constant(),
            row.coefficients(),
            self.pivot_free_ordinal,
            self.limits,
        )?;
        if replayed.constants != self.child_constants
            || replayed.free_positions != self.child_free_positions
            || replayed.compact_linear_coefficients != self.child_compact_linear_coefficients
            || replayed.matrix_stats != self.stats.integer_matrix
            || replayed.prospective_integer_matrix_input_retained_bytes
                != self.stats.prospective_integer_matrix_input_retained_bytes
            || replayed.temporary_matrix_entries != self.stats.temporary_integer_matrix_entries
            || replayed.retained_geometry_entries != self.stats.retained_child_geometry_entries
        {
            return Err(GeneratedAffineResidualCaseUnitEqualityRefinementError::ReplayMismatch);
        }
        self.child_plan.replay(context, self.child_geometry())?;
        let verification = context.compose_guard_on_residual_affine_compact_composition_plan(
            self.atom_row.source(),
            &self.child_plan,
            self.limits.polynomial_composition,
        )?;
        if !verification.value().is_zero()
            || verification.stats() != self.stats.equality_verification
            || self.parent_plan.stats() != self.stats.parent_plan
            || self.atom_row.stats() != self.stats.atom_row
            || self.child_plan.stats() != self.stats.child_plan
        {
            return Err(GeneratedAffineResidualCaseUnitEqualityRefinementError::ReplayMismatch);
        }
        Ok(())
    }
}

/// Compile zero or one current-target-coordinate equality.
pub(crate) fn compile_generated_affine_residual_case_unit_equality_refinement(
    context: &ParametricCoefficientContext,
    parent_geometry: ResidualAffineCompactMapView<'_>,
    equal_zero_predicates: &[ParametricPolynomial],
    limits: GeneratedAffineResidualCaseUnitEqualityRefinementLimits,
) -> Result<
    GeneratedAffineResidualCaseUnitEqualityRefinementOutcome,
    GeneratedAffineResidualCaseUnitEqualityRefinementError,
> {
    check_limit(
        "context fingerprint bytes",
        context.fingerprint().len(),
        limits.max_context_fingerprint_bytes,
    )?;
    // Authenticate the complete parent map even when predicate cardinality or
    // affine shape leads to a non-refined outcome.
    let parent_plan = context
        .compile_residual_affine_compact_composition_plan(parent_geometry, limits.compact_plan)?;

    check_limit(
        "equal-zero predicates inspected",
        equal_zero_predicates.len(),
        limits.max_equal_zero_predicates_inspected,
    )?;
    // Cardinality is only a completeness classification, never a shortcut
    // around context authentication.  In particular, a foreign pair must not
    // be reported as a benign unsupported multiple.
    for equality in equal_zero_predicates {
        context
            .validate_polynomial_with_limits(equality, limits.atom_row.exact_algebra)
            .map_err(ResidualAffineAtomRowError::from)?;
    }

    let equality = match equal_zero_predicates {
        [] => {
            return Ok(GeneratedAffineResidualCaseUnitEqualityRefinementOutcome::AlreadySatisfied);
        }
        [equality] => equality,
        multiple => {
            return Ok(
                GeneratedAffineResidualCaseUnitEqualityRefinementOutcome::Unsupported(
                    GeneratedAffineResidualCaseUnitEqualityRefinementUnsupported::MultipleEqualZeroPredicates {
                        actual: multiple.len(),
                    },
                ),
            );
        }
    };

    // The borrowed census enforces the atom-row shape, term, exponent, and GMP
    // bit policies before the equality's sparse payload is duplicated.
    let atom_attempt_census =
        residual_affine_atom_row_attempt_logical_memory_census(context, equality, limits.atom_row)?;
    check_limit(
        "atom attempt owned logical peak upper bound",
        atom_attempt_census.owned_logical_peak_upper_bound(),
        limits.max_atom_attempt_owned_logical_peak_upper_bound,
    )?;
    let equality_copy_retained_bytes = equality.owned_retained_byte_bound().ok_or(
        GeneratedAffineResidualCaseUnitEqualityRefinementError::ResourceCountOverflow {
            resource: "equality copy retained bytes",
        },
    )?;
    check_limit(
        "equality copy retained bytes",
        equality_copy_retained_bytes,
        limits.max_equality_copy_retained_bytes,
    )?;
    let equality_copy = equality
        .try_copy_authenticated_sparse_payload()
        .map_err(
            |_| GeneratedAffineResidualCaseUnitEqualityRefinementError::AllocationFailure {
                resource: "authenticated equality polynomial",
            },
        )?;
    let atom_row = match ResidualAffineAtomRowCertificate::compile(
        context,
        equality_copy,
        limits.atom_row,
    ) {
        Ok(certificate) => certificate,
        Err(ResidualAffineAtomRowError::Unsupported { reason }) => {
            return Ok(
                GeneratedAffineResidualCaseUnitEqualityRefinementOutcome::Unsupported(
                    GeneratedAffineResidualCaseUnitEqualityRefinementUnsupported::NonAffineOrNonAssociate {
                        reason,
                    },
                ),
            );
        }
        Err(error) => return Err(error.into()),
    };

    match atom_row.outcome() {
        ResidualAffineAtomRowOutcome::RedundantZeroPolynomial => {
            return Ok(GeneratedAffineResidualCaseUnitEqualityRefinementOutcome::AlreadySatisfied);
        }
        ResidualAffineAtomRowOutcome::InconsistentNonzeroConstant => {
            return Ok(GeneratedAffineResidualCaseUnitEqualityRefinementOutcome::ProvedEmpty);
        }
        ResidualAffineAtomRowOutcome::Row => {}
    }

    let row = atom_row
        .row()
        .ok_or(GeneratedAffineResidualCaseUnitEqualityRefinementError::ReplayMismatch)?;
    authenticate_current_target_support(row.coefficients(), parent_geometry.free_positions())?;
    let Some((pivot_free_ordinal, pivot_ambient_position)) =
        select_unit_pivot(row.coefficients(), parent_geometry.free_positions())
    else {
        return Ok(
            GeneratedAffineResidualCaseUnitEqualityRefinementOutcome::Unsupported(
                GeneratedAffineResidualCaseUnitEqualityRefinementUnsupported::RequiresIntegerNormalForm,
            ),
        );
    };

    let child = derive_child_geometry(
        parent_geometry,
        row.constant(),
        row.coefficients(),
        pivot_free_ordinal,
        limits,
    )?;
    let mut context_fingerprint = String::new();
    context_fingerprint
        .try_reserve_exact(context.fingerprint().len())
        .map_err(
            |_| GeneratedAffineResidualCaseUnitEqualityRefinementError::AllocationFailure {
                resource: "context fingerprint",
            },
        )?;
    context_fingerprint.push_str(context.fingerprint());

    let child_geometry = ResidualAffineCompactMapView::new(
        &context_fingerprint,
        context.index_count(),
        &child.constants,
        &child.free_positions,
        &child.compact_linear_coefficients,
    );
    let child_plan = context
        .compile_residual_affine_compact_composition_plan(child_geometry, limits.compact_plan)?;
    child_plan.replay(context, child_geometry)?;
    let verification = context.compose_guard_on_residual_affine_compact_composition_plan(
        atom_row.source(),
        &child_plan,
        limits.polynomial_composition,
    )?;
    if !verification.value().is_zero() {
        return Err(GeneratedAffineResidualCaseUnitEqualityRefinementError::ReplayMismatch);
    }
    let stats = GeneratedAffineResidualCaseUnitEqualityRefinementStats {
        parent_plan: parent_plan.stats(),
        atom_row: atom_row.stats(),
        integer_matrix: child.matrix_stats,
        child_plan: child_plan.stats(),
        equality_verification: verification.stats(),
        atom_attempt_owned_logical_peak_upper_bound: atom_attempt_census
            .owned_logical_peak_upper_bound(),
        equality_copy_retained_bytes,
        prospective_integer_matrix_input_retained_bytes: child
            .prospective_integer_matrix_input_retained_bytes,
        temporary_integer_matrix_entries: child.temporary_matrix_entries,
        retained_child_geometry_entries: child.retained_geometry_entries,
    };
    let certificate = GeneratedAffineResidualCaseUnitEqualityRefinementCertificate {
        schema: GENERATED_AFFINE_RESIDUAL_CASE_UNIT_EQUALITY_REFINEMENT_V1_SCHEMA,
        context_fingerprint,
        atom_row,
        pivot_free_ordinal,
        pivot_ambient_position,
        child_constants: child.constants,
        child_free_positions: child.free_positions,
        child_compact_linear_coefficients: child.compact_linear_coefficients,
        parent_plan,
        child_plan,
        limits,
        stats,
    };
    certificate.replay(context, parent_geometry)?;
    Ok(GeneratedAffineResidualCaseUnitEqualityRefinementOutcome::Refined(certificate))
}

struct DerivedChildGeometry {
    constants: Vec<Integer>,
    free_positions: Vec<usize>,
    compact_linear_coefficients: Vec<Integer>,
    matrix_stats: SymbolicaIntegerMatrixStats,
    prospective_integer_matrix_input_retained_bytes: usize,
    temporary_matrix_entries: usize,
    retained_geometry_entries: usize,
}

fn authenticate_current_target_support(
    coefficients: &[Integer],
    free_positions: &[usize],
) -> Result<(), GeneratedAffineResidualCaseUnitEqualityRefinementError> {
    let mut next_free = 0usize;
    for (position, coefficient) in coefficients.iter().enumerate() {
        if free_positions.get(next_free) == Some(&position) {
            next_free += 1;
        } else if !coefficient.is_zero() {
            return Err(
                GeneratedAffineResidualCaseUnitEqualityRefinementError::CurrentTargetCoordinateViolation {
                    position,
                },
            );
        }
    }
    Ok(())
}

fn select_unit_pivot(coefficients: &[Integer], free_positions: &[usize]) -> Option<(usize, usize)> {
    free_positions
        .iter()
        .copied()
        .enumerate()
        .find(|&(_, position)| {
            coefficients.get(position).is_some_and(|coefficient| {
                coefficient.is_one() || coefficient == &Integer::from(-1)
            })
        })
}

fn derive_child_geometry(
    parent: ResidualAffineCompactMapView<'_>,
    equality_constant: &Integer,
    equality_coefficients: &[Integer],
    pivot_free_ordinal: usize,
    limits: GeneratedAffineResidualCaseUnitEqualityRefinementLimits,
) -> Result<DerivedChildGeometry, GeneratedAffineResidualCaseUnitEqualityRefinementError> {
    let ambient_arity = parent.ambient_arity();
    let parent_free_count = parent.free_positions().len();
    let child_free_count = parent_free_count
        .checked_sub(1)
        .ok_or(GeneratedAffineResidualCaseUnitEqualityRefinementError::ReplayMismatch)?;
    let parent_columns = parent_free_count.checked_add(1).ok_or(
        GeneratedAffineResidualCaseUnitEqualityRefinementError::ResourceCountOverflow {
            resource: "parent augmented matrix columns",
        },
    )?;
    let child_columns = child_free_count.checked_add(1).ok_or(
        GeneratedAffineResidualCaseUnitEqualityRefinementError::ResourceCountOverflow {
            resource: "child augmented matrix columns",
        },
    )?;
    // `child_columns == parent_free_count`, but retain the explicit checked
    // derivation above to keep the homogeneous-coordinate meaning visible.
    let parent_entries = checked_mul(
        "parent augmented matrix entries",
        ambient_arity,
        parent_columns,
    )?;
    let substitution_entries = checked_mul(
        "unit-pivot substitution matrix entries",
        parent_columns,
        child_columns,
    )?;
    let product_entries = checked_mul(
        "child augmented matrix entries",
        ambient_arity,
        child_columns,
    )?;
    let temporary_matrix_entries = checked_add(
        "temporary integer matrix entries",
        checked_add(
            "temporary integer matrix entries",
            parent_entries,
            substitution_entries,
        )?,
        product_entries,
    )?;
    check_limit(
        "temporary integer matrix entries",
        temporary_matrix_entries,
        limits.max_temporary_integer_matrix_entries,
    )?;
    let child_linear_entries = checked_mul(
        "retained child linear entries",
        ambient_arity,
        child_free_count,
    )?;
    let retained_geometry_entries = checked_add(
        "retained child geometry entries",
        checked_add(
            "retained child geometry entries",
            ambient_arity,
            child_free_count,
        )?,
        child_linear_entries,
    )?;
    check_limit(
        "retained child geometry entries",
        retained_geometry_entries,
        limits.max_retained_child_geometry_entries,
    )?;

    let pivot_ambient_position = *parent
        .free_positions()
        .get(pivot_free_ordinal)
        .ok_or(GeneratedAffineResidualCaseUnitEqualityRefinementError::ReplayMismatch)?;
    let pivot_coefficient = equality_coefficients
        .get(pivot_ambient_position)
        .ok_or(GeneratedAffineResidualCaseUnitEqualityRefinementError::ReplayMismatch)?;
    if !pivot_coefficient.is_one() && pivot_coefficient != &Integer::from(-1) {
        return Err(GeneratedAffineResidualCaseUnitEqualityRefinementError::ReplayMismatch);
    }

    // Pre-admit the two logical inputs through borrowed entries before either
    // dense matrix clones a GMP payload.  The virtual accessor records solved
    // signs explicitly because `-i128::MIN` promotes from an inline integer to
    // a GMP-backed value during staging.
    let zero = Integer::from(0);
    let one = Integer::from(1);
    let negate_pivot_components = pivot_coefficient.is_one();
    let matrix_preflight = preflight_integer_matrix_product_with_accessors(
        ambient_arity,
        parent_columns,
        |row, column| {
            if column == 0 {
                SymbolicaIntegerMatrixEntryRef::Borrowed(&parent.constants()[row])
            } else {
                SymbolicaIntegerMatrixEntryRef::Borrowed(
                    &parent.compact_linear_coefficients()[row * parent_free_count + (column - 1)],
                )
            }
        },
        parent_columns,
        child_columns,
        |row, column| {
            if row == 0 {
                SymbolicaIntegerMatrixEntryRef::Borrowed(if column == 0 { &one } else { &zero })
            } else {
                let parent_free_ordinal = row - 1;
                if parent_free_ordinal == pivot_free_ordinal {
                    let source = if column == 0 {
                        equality_constant
                    } else {
                        let child_free_ordinal = column - 1;
                        let other_parent_free_ordinal = if child_free_ordinal < pivot_free_ordinal {
                            child_free_ordinal
                        } else {
                            child_free_ordinal + 1
                        };
                        &equality_coefficients[parent.free_positions()[other_parent_free_ordinal]]
                    };
                    if negate_pivot_components {
                        SymbolicaIntegerMatrixEntryRef::Negated(source)
                    } else {
                        SymbolicaIntegerMatrixEntryRef::Borrowed(source)
                    }
                } else {
                    let identity_column = if parent_free_ordinal < pivot_free_ordinal {
                        parent_free_ordinal + 1
                    } else {
                        parent_free_ordinal
                    };
                    SymbolicaIntegerMatrixEntryRef::Borrowed(if column == identity_column {
                        &one
                    } else {
                        &zero
                    })
                }
            }
        },
        limits.integer_matrix,
    )?;

    let mut parent_matrix = Vec::<Vec<Integer>>::new();
    parent_matrix
        .try_reserve_exact(ambient_arity)
        .map_err(
            |_| GeneratedAffineResidualCaseUnitEqualityRefinementError::AllocationFailure {
                resource: "parent augmented matrix rows",
            },
        )?;
    for row in 0..ambient_arity {
        let mut augmented = Vec::<Integer>::new();
        augmented.try_reserve_exact(parent_columns).map_err(|_| {
            GeneratedAffineResidualCaseUnitEqualityRefinementError::AllocationFailure {
                resource: "parent augmented matrix row",
            }
        })?;
        augmented.push(
            parent
                .constants()
                .get(row)
                .ok_or(GeneratedAffineResidualCaseUnitEqualityRefinementError::ReplayMismatch)?
                .clone(),
        );
        let start = row.checked_mul(parent_free_count).ok_or(
            GeneratedAffineResidualCaseUnitEqualityRefinementError::ResourceCountOverflow {
                resource: "parent compact row offset",
            },
        )?;
        let end = start.checked_add(parent_free_count).ok_or(
            GeneratedAffineResidualCaseUnitEqualityRefinementError::ResourceCountOverflow {
                resource: "parent compact row end",
            },
        )?;
        augmented.extend_from_slice(
            parent
                .compact_linear_coefficients()
                .get(start..end)
                .ok_or(GeneratedAffineResidualCaseUnitEqualityRefinementError::ReplayMismatch)?,
        );
        parent_matrix.push(augmented);
    }

    let mut substitution = Vec::<Vec<Integer>>::new();
    substitution
        .try_reserve_exact(parent_columns)
        .map_err(
            |_| GeneratedAffineResidualCaseUnitEqualityRefinementError::AllocationFailure {
                resource: "unit-pivot substitution matrix rows",
            },
        )?;
    for _ in 0..parent_columns {
        let mut row = Vec::<Integer>::new();
        row.try_reserve_exact(child_columns).map_err(|_| {
            GeneratedAffineResidualCaseUnitEqualityRefinementError::AllocationFailure {
                resource: "unit-pivot substitution matrix row",
            }
        })?;
        row.resize(child_columns, Integer::from(0));
        substitution.push(row);
    }
    substitution[0][0] = Integer::from(1);

    let solve_component = |value: &Integer| {
        if pivot_coefficient.is_one() {
            Z.neg(value)
        } else {
            value.clone()
        }
    };
    let pivot_substitution_row = pivot_free_ordinal + 1;
    substitution[pivot_substitution_row][0] = solve_component(equality_constant);
    let mut next_child_free = 0usize;
    for (parent_free_ordinal, &ambient_position) in parent.free_positions().iter().enumerate() {
        let substitution_row = parent_free_ordinal + 1;
        if parent_free_ordinal == pivot_free_ordinal {
            for (other_parent_free_ordinal, &other_ambient_position) in
                parent.free_positions().iter().enumerate()
            {
                if other_parent_free_ordinal == pivot_free_ordinal {
                    continue;
                }
                let output_column = if other_parent_free_ordinal < pivot_free_ordinal {
                    other_parent_free_ordinal + 1
                } else {
                    other_parent_free_ordinal
                };
                substitution[substitution_row][output_column] =
                    solve_component(equality_coefficients.get(other_ambient_position).ok_or(
                        GeneratedAffineResidualCaseUnitEqualityRefinementError::ReplayMismatch,
                    )?);
            }
        } else {
            let output_column = next_child_free + 1;
            substitution[substitution_row][output_column] = Integer::from(1);
            next_child_free += 1;
        }
        // The equality support contract was checked before this helper. This
        // read pins the ambient/free correspondence against accidental drift.
        if equality_coefficients.get(ambient_position).is_none() {
            return Err(GeneratedAffineResidualCaseUnitEqualityRefinementError::ReplayMismatch);
        }
    }
    if next_child_free != child_free_count {
        return Err(GeneratedAffineResidualCaseUnitEqualityRefinementError::ReplayMismatch);
    }

    let (product, matrix_stats) =
        multiply_integer_matrices(&parent_matrix, &substitution, limits.integer_matrix)?;
    if matrix_stats.input_entries() != matrix_preflight.input_entries()
        || matrix_stats.output_entries() != matrix_preflight.output_entries()
        || matrix_stats.admitted_single_matrix_entries()
            != matrix_preflight.admitted_single_matrix_entries()
        || matrix_stats.admitted_peak_live_entries()
            != matrix_preflight.admitted_peak_live_entries()
        || matrix_stats.admitted_scalar_multiplications()
            != matrix_preflight.admitted_scalar_multiplications()
        || matrix_stats.admitted_scalar_additions() != matrix_preflight.admitted_scalar_additions()
        || matrix_stats.input_retained_bytes() > matrix_preflight.input_retained_bytes()
        || matrix_stats.prospective_output_retained_bytes()
            != matrix_preflight.prospective_output_retained_bytes()
        || matrix_stats.maximum_input_integer_bits()
            != matrix_preflight.maximum_input_integer_bits()
        || matrix_stats.admitted_intermediate_integer_bits()
            != matrix_preflight.admitted_intermediate_integer_bits()
    {
        return Err(GeneratedAffineResidualCaseUnitEqualityRefinementError::ReplayMismatch);
    }
    if product.len() != ambient_arity || product.iter().any(|row| row.len() != child_columns) {
        return Err(GeneratedAffineResidualCaseUnitEqualityRefinementError::ReplayMismatch);
    }

    let mut constants = Vec::<Integer>::new();
    constants.try_reserve_exact(ambient_arity).map_err(|_| {
        GeneratedAffineResidualCaseUnitEqualityRefinementError::AllocationFailure {
            resource: "child affine constants",
        }
    })?;
    let mut compact_linear_coefficients = Vec::<Integer>::new();
    compact_linear_coefficients
        .try_reserve_exact(child_linear_entries)
        .map_err(
            |_| GeneratedAffineResidualCaseUnitEqualityRefinementError::AllocationFailure {
                resource: "child compact linear coefficients",
            },
        )?;
    for row in product {
        let mut entries = row.into_iter();
        constants.push(
            entries
                .next()
                .ok_or(GeneratedAffineResidualCaseUnitEqualityRefinementError::ReplayMismatch)?,
        );
        compact_linear_coefficients.extend(entries);
    }
    let mut free_positions = Vec::<usize>::new();
    free_positions
        .try_reserve_exact(child_free_count)
        .map_err(
            |_| GeneratedAffineResidualCaseUnitEqualityRefinementError::AllocationFailure {
                resource: "child free positions",
            },
        )?;
    free_positions.extend(
        parent
            .free_positions()
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(ordinal, position)| (ordinal != pivot_free_ordinal).then_some(position)),
    );
    if constants.len() != ambient_arity
        || free_positions.len() != child_free_count
        || compact_linear_coefficients.len() != child_linear_entries
    {
        return Err(GeneratedAffineResidualCaseUnitEqualityRefinementError::ReplayMismatch);
    }

    Ok(DerivedChildGeometry {
        constants,
        free_positions,
        compact_linear_coefficients,
        matrix_stats,
        prospective_integer_matrix_input_retained_bytes: matrix_preflight.input_retained_bytes(),
        temporary_matrix_entries,
        retained_geometry_entries,
    })
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualCaseUnitEqualityRefinementError> {
    left.checked_add(right).ok_or(
        GeneratedAffineResidualCaseUnitEqualityRefinementError::ResourceCountOverflow { resource },
    )
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualCaseUnitEqualityRefinementError> {
    left.checked_mul(right).ok_or(
        GeneratedAffineResidualCaseUnitEqualityRefinementError::ResourceCountOverflow { resource },
    )
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualCaseUnitEqualityRefinementError> {
    if requested > limit {
        Err(
            GeneratedAffineResidualCaseUnitEqualityRefinementError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CoefficientContext, ParametricCoefficient};

    struct OwnedGeometry {
        context_fingerprint: String,
        ambient_arity: usize,
        constants: Vec<Integer>,
        free_positions: Vec<usize>,
        compact_linear_coefficients: Vec<Integer>,
    }

    impl OwnedGeometry {
        fn identity(context: &ParametricCoefficientContext) -> Self {
            let arity = context.index_count();
            let mut compact_linear_coefficients = Vec::with_capacity(arity * arity);
            for row in 0..arity {
                for column in 0..arity {
                    compact_linear_coefficients.push(Integer::from(if row == column {
                        1
                    } else {
                        0
                    }));
                }
            }
            Self {
                context_fingerprint: context.fingerprint().to_owned(),
                ambient_arity: arity,
                constants: vec![Integer::from(0); arity],
                free_positions: (0..arity).collect(),
                compact_linear_coefficients,
            }
        }

        fn view(&self) -> ResidualAffineCompactMapView<'_> {
            ResidualAffineCompactMapView::new(
                &self.context_fingerprint,
                self.ambient_arity,
                &self.constants,
                &self.free_positions,
                &self.compact_linear_coefficients,
            )
        }
    }

    fn context(scope: &str, arity: usize) -> ParametricCoefficientContext {
        let base = CoefficientContext::new(["d"]);
        ParametricCoefficientContext::try_new(&base, scope, arity).unwrap()
    }

    fn polynomial(
        context: &ParametricCoefficientContext,
        value: &ParametricCoefficient,
    ) -> ParametricPolynomial {
        context.numerator_condition(value).unwrap()
    }

    fn affine(
        context: &ParametricCoefficientContext,
        constant: i64,
        coefficients: &[i64],
    ) -> ParametricPolynomial {
        let mut value = context.integer(constant);
        for (position, &coefficient) in coefficients.iter().enumerate() {
            let term = context
                .mul(
                    &context.integer(coefficient),
                    &context.index(position).unwrap(),
                )
                .unwrap();
            value = context.add(&value, &term).unwrap();
        }
        polynomial(context, &value)
    }

    fn refined(
        context: &ParametricCoefficientContext,
        parent: ResidualAffineCompactMapView<'_>,
        equality: ParametricPolynomial,
    ) -> GeneratedAffineResidualCaseUnitEqualityRefinementCertificate {
        match compile_generated_affine_residual_case_unit_equality_refinement(
            context,
            parent,
            &[equality],
            GeneratedAffineResidualCaseUnitEqualityRefinementLimits::default(),
        )
        .unwrap()
        {
            GeneratedAffineResidualCaseUnitEqualityRefinementOutcome::Refined(certificate) => {
                certificate
            }
            outcome => panic!("expected refined equality, got {outcome:?}"),
        }
    }

    #[test]
    fn positive_unit_pivot_builds_and_replays_direct_child_geometry() {
        let context = context("unit-equality-positive", 3);
        let parent = OwnedGeometry::identity(&context);
        let certificate = refined(&context, parent.view(), affine(&context, 2, &[1, -3, 0]));

        assert_eq!(certificate.pivot_free_ordinal(), 0);
        assert_eq!(certificate.pivot_ambient_position(), 0);
        assert_eq!(certificate.child_free_positions, [1, 2]);
        assert_eq!(
            certificate.child_constants,
            [Integer::from(-2), Integer::from(0), Integer::from(0)]
        );
        assert_eq!(
            certificate.child_compact_linear_coefficients,
            [
                Integer::from(3),
                Integer::from(0),
                Integer::from(1),
                Integer::from(0),
                Integer::from(0),
                Integer::from(1),
            ]
        );
        assert_eq!(certificate.stats().integer_matrix().product_calls(), 1);
        assert!(
            certificate
                .equality()
                .to_expression()
                .to_string()
                .contains("2")
        );
        certificate.replay(&context, parent.view()).unwrap();
    }

    #[test]
    fn negative_unit_coefficient_is_solved_without_division() {
        let context = context("unit-equality-negative", 2);
        let parent = OwnedGeometry::identity(&context);
        // Canonical primitive row [1,-1,2], hence n0 = 1 + 2*n1.
        let certificate = refined(&context, parent.view(), affine(&context, 1, &[-1, 2]));

        assert_eq!(certificate.pivot_ambient_position(), 0);
        assert_eq!(certificate.child_free_positions, [1]);
        assert_eq!(
            certificate.child_constants,
            [Integer::from(1), Integer::from(0)]
        );
        assert_eq!(
            certificate.child_compact_linear_coefficients,
            [Integer::from(2), Integer::from(1)]
        );
        certificate.replay(&context, parent.view()).unwrap();
    }

    #[test]
    fn first_free_unit_is_selected_deterministically() {
        let context = context("unit-equality-deterministic", 3);
        let parent = OwnedGeometry::identity(&context);
        let certificate = refined(&context, parent.view(), affine(&context, 1, &[1, -1, 1]));

        assert_eq!(certificate.pivot_free_ordinal(), 0);
        assert_eq!(certificate.pivot_ambient_position(), 0);
        assert_eq!(certificate.child_free_positions, [1, 2]);
    }

    #[test]
    fn nontrivial_parent_map_is_composed_by_native_integer_matrix_product() {
        let context = context("unit-equality-parent-map", 3);
        // n0 = 2 + 3*n1 + 4*n2; n1,n2 are the current target coordinates.
        let parent = OwnedGeometry {
            context_fingerprint: context.fingerprint().to_owned(),
            ambient_arity: 3,
            constants: vec![Integer::from(2), Integer::from(0), Integer::from(0)],
            free_positions: vec![1, 2],
            compact_linear_coefficients: vec![
                Integer::from(3),
                Integer::from(4),
                Integer::from(1),
                Integer::from(0),
                Integer::from(0),
                Integer::from(1),
            ],
        };
        // Current-target input: n1 + n2 + 1 = 0. It must not be mapped
        // through the parent a second time.
        let certificate = refined(&context, parent.view(), affine(&context, 1, &[0, 1, 1]));

        assert_eq!(certificate.pivot_ambient_position(), 1);
        assert_eq!(certificate.child_free_positions, [2]);
        assert_eq!(
            certificate.child_constants,
            [Integer::from(-1), Integer::from(-1), Integer::from(0)]
        );
        assert_eq!(
            certificate.child_compact_linear_coefficients,
            [Integer::from(1), Integer::from(-1), Integer::from(1)]
        );
        certificate.replay(&context, parent.view()).unwrap();
    }

    #[test]
    fn arbitrary_precision_constant_survives_refinement_and_replay() {
        let context = context("unit-equality-gmp", 2);
        let parent = OwnedGeometry::identity(&context);
        let mut large = context.one();
        // Cross Symbolica's inline i128 representation into the required GMP
        // backend rather than merely exercising its `Double` variant.
        for _ in 0..200 {
            large = context.add(&large, &large).unwrap();
        }
        let equality_value = context.add(&large, &context.index(0).unwrap()).unwrap();
        let certificate = refined(
            &context,
            parent.view(),
            polynomial(&context, &equality_value),
        );

        assert!(matches!(certificate.child_constants[0], Integer::Large(_)));
        assert!(certificate.child_constants[0].is_negative());
        assert!(
            certificate
                .stats()
                .integer_matrix()
                .maximum_input_integer_bits()
                >= 201
        );
        certificate.replay(&context, parent.view()).unwrap();
    }

    #[test]
    fn virtual_negated_i128_min_is_preflighted_as_gmp_backed() {
        let context = context("unit-equality-negated-i128-min", 2);
        let parent = OwnedGeometry::identity(&context);
        let minimum = context
            .lift(
                &context
                    .base()
                    .parse("-170141183460469231731687303715884105728")
                    .unwrap(),
            )
            .unwrap();
        let equality_value = context
            .add(
                &context.index(0).unwrap(),
                &context.mul(&minimum, &context.index(1).unwrap()).unwrap(),
            )
            .unwrap();
        let equality = polynomial(&context, &equality_value);
        let certificate = refined(&context, parent.view(), equality.clone());

        assert_eq!(
            certificate.atom_row().row().unwrap().coefficients()[1],
            Integer::Double(i128::MIN)
        );
        assert!(matches!(
            certificate.child_compact_linear_coefficients[0],
            Integer::Large(_)
        ));
        assert!(
            certificate
                .stats()
                .prospective_integer_matrix_input_retained_bytes()
                > certificate.stats().integer_matrix().input_retained_bytes()
        );
        certificate.replay(&context, parent.view()).unwrap();

        let mut one_below = GeneratedAffineResidualCaseUnitEqualityRefinementLimits::default();
        one_below.integer_matrix.max_input_retained_bytes = certificate
            .stats()
            .prospective_integer_matrix_input_retained_bytes()
            - 1;
        assert!(matches!(
            compile_generated_affine_residual_case_unit_equality_refinement(
                &context,
                parent.view(),
                &[equality],
                one_below,
            ),
            Err(
                GeneratedAffineResidualCaseUnitEqualityRefinementError::IntegerMatrix(
                    SymbolicaIntegerMatrixError::ResourceLimit {
                        resource: "integer matrix input retained bytes",
                        ..
                    }
                )
            )
        ));
    }

    #[test]
    fn physical_parameter_block_reuses_the_same_primitive_affine_row() {
        let context = context("unit-equality-physical-block", 2);
        let parent = OwnedGeometry::identity(&context);
        let d = context
            .lift(&context.base().parameter("d").unwrap())
            .unwrap();
        let affine_value = context
            .add(
                &context.integer(1),
                &context
                    .add(&context.index(0).unwrap(), &context.index(1).unwrap())
                    .unwrap(),
            )
            .unwrap();
        let equality = polynomial(&context, &context.mul(&d, &affine_value).unwrap());
        let certificate = refined(&context, parent.view(), equality);

        assert_eq!(
            certificate.atom_row().row().unwrap().components(),
            [Integer::from(1), Integer::from(1), Integer::from(1)]
        );
        assert_eq!(certificate.pivot_ambient_position(), 0);
        assert_eq!(certificate.child_free_positions, [1]);
        certificate.replay(&context, parent.view()).unwrap();
    }

    #[test]
    fn primitive_nonunit_row_requires_integer_normal_form() {
        let context = context("unit-equality-normal-form", 2);
        let parent = OwnedGeometry::identity(&context);
        let outcome = compile_generated_affine_residual_case_unit_equality_refinement(
            &context,
            parent.view(),
            &[affine(&context, 1, &[2, 3])],
            GeneratedAffineResidualCaseUnitEqualityRefinementLimits::default(),
        )
        .unwrap();

        assert!(matches!(
            outcome,
            GeneratedAffineResidualCaseUnitEqualityRefinementOutcome::Unsupported(
                GeneratedAffineResidualCaseUnitEqualityRefinementUnsupported::RequiresIntegerNormalForm
            )
        ));
    }

    #[test]
    fn nonlinear_equality_is_a_typed_completeness_boundary() {
        let context = context("unit-equality-quadratic", 2);
        let parent = OwnedGeometry::identity(&context);
        let n0 = context.index(0).unwrap();
        let quadratic = polynomial(&context, &context.mul(&n0, &n0).unwrap());
        let outcome = compile_generated_affine_residual_case_unit_equality_refinement(
            &context,
            parent.view(),
            &[quadratic],
            GeneratedAffineResidualCaseUnitEqualityRefinementLimits::default(),
        )
        .unwrap();

        assert!(matches!(
            outcome,
            GeneratedAffineResidualCaseUnitEqualityRefinementOutcome::Unsupported(
                GeneratedAffineResidualCaseUnitEqualityRefinementUnsupported::NonAffineOrNonAssociate {
                    reason: ResidualAffineAtomRowUnsupported::NonAffineIndexMonomial { .. }
                }
            )
        ));
    }

    #[test]
    fn zero_constant_and_empty_conjunction_are_classified_exactly() {
        let context = context("unit-equality-constants", 2);
        let parent = OwnedGeometry::identity(&context);
        let limits = GeneratedAffineResidualCaseUnitEqualityRefinementLimits::default();

        assert!(matches!(
            compile_generated_affine_residual_case_unit_equality_refinement(
                &context,
                parent.view(),
                &[],
                limits,
            )
            .unwrap(),
            GeneratedAffineResidualCaseUnitEqualityRefinementOutcome::AlreadySatisfied
        ));
        assert!(matches!(
            compile_generated_affine_residual_case_unit_equality_refinement(
                &context,
                parent.view(),
                &[polynomial(&context, &context.zero())],
                limits,
            )
            .unwrap(),
            GeneratedAffineResidualCaseUnitEqualityRefinementOutcome::AlreadySatisfied
        ));
        assert!(matches!(
            compile_generated_affine_residual_case_unit_equality_refinement(
                &context,
                parent.view(),
                &[polynomial(&context, &context.integer(7))],
                limits,
            )
            .unwrap(),
            GeneratedAffineResidualCaseUnitEqualityRefinementOutcome::ProvedEmpty
        ));
    }

    #[test]
    fn multiple_predicates_are_typed_only_after_every_context_is_authenticated() {
        let context = context("unit-equality-multiple", 2);
        let parent = OwnedGeometry::identity(&context);
        let predicates = [affine(&context, 0, &[1, 0]), affine(&context, 0, &[0, 1])];
        let outcome = compile_generated_affine_residual_case_unit_equality_refinement(
            &context,
            parent.view(),
            &predicates,
            GeneratedAffineResidualCaseUnitEqualityRefinementLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            outcome,
            GeneratedAffineResidualCaseUnitEqualityRefinementOutcome::Unsupported(
                GeneratedAffineResidualCaseUnitEqualityRefinementUnsupported::MultipleEqualZeroPredicates {
                    actual: 2
                }
            )
        ));

        let foreign_base = CoefficientContext::new(["d"]);
        let foreign = ParametricCoefficientContext::try_new(
            &foreign_base,
            "unit-equality-multiple-foreign",
            2,
        )
        .unwrap();
        let foreign_predicates = [predicates[0].clone(), affine(&foreign, 0, &[0, 1])];
        assert!(matches!(
            compile_generated_affine_residual_case_unit_equality_refinement(
                &context,
                parent.view(),
                &foreign_predicates,
                GeneratedAffineResidualCaseUnitEqualityRefinementLimits::default(),
            ),
            Err(
                GeneratedAffineResidualCaseUnitEqualityRefinementError::AtomRow(
                    ResidualAffineAtomRowError::Coefficient(_)
                )
            )
        ));
    }

    #[test]
    fn current_target_contract_rejects_parent_nonfree_support() {
        let context = context("unit-equality-current-target", 3);
        let parent = OwnedGeometry {
            context_fingerprint: context.fingerprint().to_owned(),
            ambient_arity: 3,
            constants: vec![Integer::from(2), Integer::from(0), Integer::from(0)],
            free_positions: vec![1, 2],
            compact_linear_coefficients: vec![
                Integer::from(3),
                Integer::from(4),
                Integer::from(1),
                Integer::from(0),
                Integer::from(0),
                Integer::from(1),
            ],
        };
        let error = compile_generated_affine_residual_case_unit_equality_refinement(
            &context,
            parent.view(),
            &[affine(&context, 0, &[1, 1, 0])],
            GeneratedAffineResidualCaseUnitEqualityRefinementLimits::default(),
        )
        .unwrap_err();

        assert_eq!(
            error,
            GeneratedAffineResidualCaseUnitEqualityRefinementError::CurrentTargetCoordinateViolation {
                position: 0
            }
        );
    }

    #[test]
    fn replay_detects_geometry_tamper_and_wrong_parent() {
        let context = context("unit-equality-tamper", 3);
        let parent = OwnedGeometry::identity(&context);
        let mut certificate = refined(&context, parent.view(), affine(&context, 1, &[1, 1, 0]));
        certificate.child_constants[0] = Integer::from(99);
        assert!(matches!(
            certificate.replay(&context, parent.view()),
            Err(GeneratedAffineResidualCaseUnitEqualityRefinementError::ReplayMismatch)
        ));

        let certificate = refined(&context, parent.view(), affine(&context, 1, &[1, 1, 0]));
        let mut wrong_parent = OwnedGeometry::identity(&context);
        wrong_parent.constants[2] = Integer::from(1);
        assert!(certificate.replay(&context, wrong_parent.view()).is_err());
    }

    #[test]
    fn temporary_matrix_resource_is_rejected_before_matrix_construction() {
        let context = context("unit-equality-resource", 3);
        let parent = OwnedGeometry::identity(&context);
        let limits = GeneratedAffineResidualCaseUnitEqualityRefinementLimits {
            max_temporary_integer_matrix_entries: 0,
            ..GeneratedAffineResidualCaseUnitEqualityRefinementLimits::default()
        };
        assert!(matches!(
            compile_generated_affine_residual_case_unit_equality_refinement(
                &context,
                parent.view(),
                &[affine(&context, 1, &[1, 1, 0])],
                limits,
            ),
            Err(
                GeneratedAffineResidualCaseUnitEqualityRefinementError::ResourceLimit {
                    resource: "temporary integer matrix entries",
                    ..
                }
            )
        ));
    }

    #[test]
    fn atom_attempt_and_sparse_copy_bytes_have_exact_preclone_boundaries() {
        let context = context("unit-equality-copy-preflight", 3);
        let parent = OwnedGeometry::identity(&context);
        let equality = affine(&context, 7, &[1, -2, 3]);
        let baseline = refined(&context, parent.view(), equality.clone());
        let stats = baseline.stats();
        assert!(stats.atom_attempt_owned_logical_peak_upper_bound() > 0);
        assert!(stats.equality_copy_retained_bytes() > 0);

        let exact = GeneratedAffineResidualCaseUnitEqualityRefinementLimits {
            max_atom_attempt_owned_logical_peak_upper_bound: stats
                .atom_attempt_owned_logical_peak_upper_bound(),
            max_equality_copy_retained_bytes: stats.equality_copy_retained_bytes(),
            ..GeneratedAffineResidualCaseUnitEqualityRefinementLimits::default()
        };
        let exact_certificate =
            match compile_generated_affine_residual_case_unit_equality_refinement(
                &context,
                parent.view(),
                std::slice::from_ref(&equality),
                exact,
            )
            .unwrap()
            {
                GeneratedAffineResidualCaseUnitEqualityRefinementOutcome::Refined(certificate) => {
                    certificate
                }
                outcome => panic!("expected exact-bound refinement, got {outcome:?}"),
            };
        exact_certificate.replay(&context, parent.view()).unwrap();

        for (limits, resource) in [
            (
                GeneratedAffineResidualCaseUnitEqualityRefinementLimits {
                    max_atom_attempt_owned_logical_peak_upper_bound: stats
                        .atom_attempt_owned_logical_peak_upper_bound()
                        - 1,
                    ..exact
                },
                "atom attempt owned logical peak upper bound",
            ),
            (
                GeneratedAffineResidualCaseUnitEqualityRefinementLimits {
                    max_equality_copy_retained_bytes: stats.equality_copy_retained_bytes() - 1,
                    ..exact
                },
                "equality copy retained bytes",
            ),
        ] {
            assert!(matches!(
                compile_generated_affine_residual_case_unit_equality_refinement(
                    &context,
                    parent.view(),
                    std::slice::from_ref(&equality),
                    limits,
                ),
                Err(GeneratedAffineResidualCaseUnitEqualityRefinementError::ResourceLimit {
                    resource: actual,
                    ..
                }) if actual == resource
            ));
        }
    }

    #[test]
    fn atom_gmp_bit_limit_is_enforced_before_sparse_copy() {
        let context = context("unit-equality-atom-bit-preflight", 2);
        let parent = OwnedGeometry::identity(&context);
        let mut large = context.one();
        for _ in 0..200 {
            large = context.add(&large, &large).unwrap();
        }
        let equality = polynomial(
            &context,
            &context.add(&large, &context.index(0).unwrap()).unwrap(),
        );
        let mut limits = GeneratedAffineResidualCaseUnitEqualityRefinementLimits::default();
        limits.atom_row.max_integer_coefficient_bits = 128;
        assert!(matches!(
            compile_generated_affine_residual_case_unit_equality_refinement(
                &context,
                parent.view(),
                &[equality],
                limits,
            ),
            Err(
                GeneratedAffineResidualCaseUnitEqualityRefinementError::AtomRow(
                    ResidualAffineAtomRowError::ResourceLimit {
                        resource: "integer coefficient bits",
                        ..
                    }
                )
            )
        ));
    }

    #[test]
    fn virtual_matrix_payload_is_admitted_before_dense_staging() {
        let context = context("unit-equality-matrix-preflight", 3);
        let parent = OwnedGeometry::identity(&context);
        let equality = affine(&context, 5, &[1, -2, 3]);
        let baseline = refined(&context, parent.view(), equality.clone());
        let matrix_stats = baseline.stats().integer_matrix();
        assert!(matrix_stats.input_retained_bytes() > 0);
        assert!(matrix_stats.prospective_output_retained_bytes() > 0);
        assert_eq!(
            baseline
                .stats()
                .prospective_integer_matrix_input_retained_bytes(),
            matrix_stats.input_retained_bytes()
        );

        let mut exact = GeneratedAffineResidualCaseUnitEqualityRefinementLimits::default();
        exact.integer_matrix.max_input_retained_bytes = matrix_stats.input_retained_bytes();
        exact.integer_matrix.max_prospective_output_retained_bytes =
            matrix_stats.prospective_output_retained_bytes();
        let exact_certificate =
            match compile_generated_affine_residual_case_unit_equality_refinement(
                &context,
                parent.view(),
                std::slice::from_ref(&equality),
                exact,
            )
            .unwrap()
            {
                GeneratedAffineResidualCaseUnitEqualityRefinementOutcome::Refined(certificate) => {
                    certificate
                }
                outcome => panic!("expected exact-bound matrix refinement, got {outcome:?}"),
            };
        exact_certificate.replay(&context, parent.view()).unwrap();

        for (limits, resource) in [
            (
                GeneratedAffineResidualCaseUnitEqualityRefinementLimits {
                    integer_matrix: SymbolicaIntegerMatrixLimits {
                        max_input_retained_bytes: matrix_stats.input_retained_bytes() - 1,
                        ..exact.integer_matrix
                    },
                    ..exact
                },
                "integer matrix input retained bytes",
            ),
            (
                GeneratedAffineResidualCaseUnitEqualityRefinementLimits {
                    integer_matrix: SymbolicaIntegerMatrixLimits {
                        max_prospective_output_retained_bytes: matrix_stats
                            .prospective_output_retained_bytes()
                            - 1,
                        ..exact.integer_matrix
                    },
                    ..exact
                },
                "prospective integer matrix output retained bytes",
            ),
        ] {
            assert!(matches!(
                compile_generated_affine_residual_case_unit_equality_refinement(
                    &context,
                    parent.view(),
                    std::slice::from_ref(&equality),
                    limits,
                ),
                Err(GeneratedAffineResidualCaseUnitEqualityRefinementError::IntegerMatrix(
                    SymbolicaIntegerMatrixError::ResourceLimit {
                        resource: actual,
                        ..
                    }
                )) if actual == resource
            ));
        }
    }

    #[test]
    fn matrix_bit_policy_is_enforced_before_dense_gmp_staging() {
        let context = context("unit-equality-matrix-bit-preflight", 2);
        let parent = OwnedGeometry::identity(&context);
        let mut large = context.one();
        for _ in 0..200 {
            large = context.add(&large, &large).unwrap();
        }
        let equality = polynomial(
            &context,
            &context.add(&large, &context.index(0).unwrap()).unwrap(),
        );
        let mut limits = GeneratedAffineResidualCaseUnitEqualityRefinementLimits::default();
        limits.integer_matrix.max_integer_bits = 128;
        assert!(matches!(
            compile_generated_affine_residual_case_unit_equality_refinement(
                &context,
                parent.view(),
                &[equality],
                limits,
            ),
            Err(GeneratedAffineResidualCaseUnitEqualityRefinementError::IntegerMatrix(
                SymbolicaIntegerMatrixError::IntegerBitLimit {
                    payload: crate::symbolica_coefficient_matrix::SymbolicaIntegerMatrixPayload::RightInput,
                    ..
                }
            ))
        ));
    }

    #[test]
    fn source_contains_no_handwritten_integer_system_or_lattice_kernel_dependency() {
        let source = include_str!("generated_affine_residual_case_unit_equality_refinement.rs");
        assert!(!source.contains(concat!("residual_affine_", "integer_system")));
        assert!(!source.contains(concat!("residual_affine_", "integer_lattice_kernel")));
    }
}
