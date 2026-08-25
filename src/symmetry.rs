//! Exact, loop-count-independent verification of momentum and family maps.
//!
//! LiteRed uses Symanzik signatures to *propose* sector symmetries and then
//! solves for momentum shifts.  This module starts at the proof boundary: a
//! caller supplies exact matrices
//!
//! ```text
//! l_source = A l_target + B p_target,
//! p_source = C p_target,
//! ```
//!
//! and RustRed derives and replays the induced scalar-product and affine
//! denominator maps.  No graph signature, numerical sample, denominator
//! count, or topology name is accepted as a proof.

use std::collections::BTreeSet;
use std::fmt;

use crate::generic_family::BasePolynomial;
use crate::symbolica_coefficient_matrix::{
    DEFAULT_MAX_INPUT_RETAINED_BYTES, DEFAULT_MAX_OUTPUT_RETAINED_BYTES,
    SymbolicaCoefficientMatrixError, SymbolicaCoefficientMatrixLimits,
    SymbolicaCoefficientMatrixStats, congruence_of_coefficient_matrix,
    determinant_of_coefficient_matrix, multiply_coefficient_matrices,
    multiply_three_coefficient_matrices,
};
use crate::{
    Coefficient, CoefficientContext, ExactAlgebraError, ExactAlgebraLimits, FamilyDomain,
    GenericFamilyError, GuardOrigin, IntegralFamily, ScalarProductCoordinate,
};

pub const AFFINE_FAMILY_MAP_V1_SCHEMA: &str = "rustred-affine-family-map-v1";
pub const AFFINE_FAMILY_MAP_V2_SCHEMA: &str = "rustred-affine-family-map-v2";
pub const DEFAULT_MAX_EXACT_MATRIX_ENTRIES: usize = 16_000_000;

/// A checked row-major matrix.  Empty dimensions are supported because a
/// vacuum family has `B: L x 0` and `C: 0 x 0`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactMatrix<T> {
    rows: usize,
    columns: usize,
    row_major: Box<[T]>,
}

impl<T> ExactMatrix<T> {
    pub fn try_new(
        rows: usize,
        columns: usize,
        row_major: impl IntoIterator<Item = T>,
    ) -> Result<Self, SymmetryVerificationError> {
        Self::try_new_with_max_entries(rows, columns, row_major, DEFAULT_MAX_EXACT_MATRIX_ENTRIES)
    }

    /// Construct a matrix without ever consuming more than the declared
    /// payload plus one sentinel entry.  The explicit cap is checked before
    /// reserving storage, so even an infinite iterator cannot bypass the
    /// caller's allocation policy.
    pub fn try_new_with_max_entries(
        rows: usize,
        columns: usize,
        row_major: impl IntoIterator<Item = T>,
        max_entries: usize,
    ) -> Result<Self, SymmetryVerificationError> {
        let expected =
            rows.checked_mul(columns)
                .ok_or(SymmetryVerificationError::ResourceCountOverflow {
                    resource: "exact matrix entries",
                })?;
        check_limit("exact matrix entries", expected, max_entries)?;

        let mut iterator = row_major.into_iter();
        let mut retained = Vec::new();
        retained.try_reserve_exact(expected).map_err(|_| {
            SymmetryVerificationError::AllocationFailure {
                resource: "exact matrix entries",
                requested: expected,
            }
        })?;
        for actual in 0..expected {
            let Some(entry) = iterator.next() else {
                return Err(SymmetryVerificationError::MatrixPayloadSize {
                    rows,
                    columns,
                    expected,
                    actual,
                });
            };
            retained.push(entry);
        }
        if iterator.next().is_some() {
            return Err(SymmetryVerificationError::MatrixPayloadTooLarge {
                rows,
                columns,
                expected,
            });
        }
        Ok(Self {
            rows,
            columns,
            row_major: retained.into_boxed_slice(),
        })
    }

    pub const fn rows(&self) -> usize {
        self.rows
    }

    pub const fn columns(&self) -> usize {
        self.columns
    }

    pub fn entries(&self) -> &[T] {
        &self.row_major
    }

    pub fn get(&self, row: usize, column: usize) -> Option<&T> {
        if row >= self.rows || column >= self.columns {
            return None;
        }
        self.row_major.get(row * self.columns + column)
    }

    fn at(&self, row: usize, column: usize) -> &T {
        &self.row_major[row * self.columns + column]
    }
}

/// Exact source-to-target momentum substitution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MomentumMap {
    loop_linear: ExactMatrix<Coefficient>,
    loop_external: ExactMatrix<Coefficient>,
    external_linear: ExactMatrix<Coefficient>,
}

impl MomentumMap {
    pub fn new(
        loop_linear: ExactMatrix<Coefficient>,
        loop_external: ExactMatrix<Coefficient>,
        external_linear: ExactMatrix<Coefficient>,
    ) -> Self {
        Self {
            loop_linear,
            loop_external,
            external_linear,
        }
    }

    pub const fn loop_linear(&self) -> &ExactMatrix<Coefficient> {
        &self.loop_linear
    }

    pub const fn loop_external(&self) -> &ExactMatrix<Coefficient> {
        &self.loop_external
    }

    pub const fn external_linear(&self) -> &ExactMatrix<Coefficient> {
        &self.external_linear
    }
}

/// `S_source = constant + linear * S_target` in each family's declared
/// scalar-product coordinate order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AffineScalarProductMap {
    constant: Box<[Coefficient]>,
    linear: ExactMatrix<Coefficient>,
}

impl AffineScalarProductMap {
    pub fn constant(&self) -> &[Coefficient] {
        &self.constant
    }

    pub const fn linear(&self) -> &ExactMatrix<Coefficient> {
        &self.linear
    }
}

/// `D_source = constant + linear * D_target`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AffineDenominatorMap {
    constant: Box<[Coefficient]>,
    linear: ExactMatrix<Coefficient>,
}

impl AffineDenominatorMap {
    pub fn constant(&self) -> &[Coefficient] {
        &self.constant
    }

    pub const fn linear(&self) -> &ExactMatrix<Coefficient> {
        &self.linear
    }
}

/// Exact action of one source denominator row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DenominatorRowAction {
    Monomial { target: usize, scale: Coefficient },
    Affine,
}

/// Loop-measure witness.  The first parametric rule compiler accepts only
/// `Unit`; a formal determinant power must never be silently discarded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JacobianWitness {
    Unit { determinant_sign: i8 },
    FormalDeterminantPower { determinant: Coefficient },
}

/// One stable reason why a polynomial must remain nonzero for an affine map.
/// Family origins are wrapped with their source/target role; candidate and
/// derived origins retain their exact matrix or denominator location.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SymmetryGuardOrigin {
    SourceFamily(GuardOrigin),
    TargetFamily(GuardOrigin),
    MomentumMapDenominator {
        matrix: &'static str,
        row: usize,
        column: usize,
    },
    LoopMapDeterminantNumerator,
    ExternalMapDeterminantNumerator,
    DenominatorScaleNumerator {
        source_denominator: usize,
        target_denominator: usize,
    },
}

/// One merged, exact nonzero condition retained by authoritative replay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymmetryNonZeroCondition {
    polynomial: BasePolynomial,
    origins: BTreeSet<SymmetryGuardOrigin>,
}

impl SymmetryNonZeroCondition {
    pub const fn polynomial(&self) -> &BasePolynomial {
        &self.polynomial
    }

    pub const fn origins(&self) -> &BTreeSet<SymmetryGuardOrigin> {
        &self.origins
    }
}

/// Aggregate bounds for one authoritative replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SymmetryVerificationLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_matrix_entries: usize,
    /// Aggregate replayable admission envelope for checked scalar calls and
    /// native Symbolica schedules.  Actual native calls are retained
    /// separately in [`SymmetryVerificationStats::symbolica_exact_operations`].
    pub max_exact_operations: usize,
    /// Legacy V1 subset-DP ceiling retained for source compatibility.  V2
    /// delegates determinants to Symbolica and does not allocate or charge
    /// subset states, so this field is intentionally ignored.
    pub max_determinant_states: usize,
    /// Largest individual matrix admitted inside one authenticated Symbolica
    /// determinant or product session.
    pub max_symbolica_single_matrix_entries: usize,
    /// Largest conservative simultaneously-live native matrix payload in one
    /// authenticated Symbolica session.
    pub max_symbolica_live_matrix_entries: usize,
    /// Aggregate clone-owned bytes copied into authenticated Symbolica matrix
    /// inputs across one complete derivation/replay pass.
    pub max_symbolica_input_retained_bytes: usize,
    /// Aggregate clone-owned bytes authenticated in native determinant and
    /// product outputs across one complete derivation/replay pass.
    pub max_symbolica_output_retained_bytes: usize,
    pub max_guard_polynomials: usize,
    pub max_guard_origins: usize,
}

impl Default for SymmetryVerificationLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_matrix_entries: 16_000_000,
            max_exact_operations: 100_000_000,
            max_determinant_states: 16_000_000,
            max_symbolica_single_matrix_entries: 16_000_000,
            max_symbolica_live_matrix_entries: 32_000_000,
            max_symbolica_input_retained_bytes: DEFAULT_MAX_INPUT_RETAINED_BYTES,
            max_symbolica_output_retained_bytes: DEFAULT_MAX_OUTPUT_RETAINED_BYTES,
            max_guard_polynomials: 1_000_000,
            max_guard_origins: 4_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SymmetryVerificationStats {
    matrix_entries: usize,
    /// Aggregate admitted exact-operation envelope.  Native sessions retain
    /// their public Symbolica preflight bound so this value is replayable as
    /// an exact limit even when a determinant's actual schedule is smaller.
    exact_operations: usize,
    /// Legacy V1 subset-DP state census.  Always zero for V2 certificates.
    determinant_states: usize,
    symbolica_exact_operations: usize,
    symbolica_admitted_exact_operations: usize,
    symbolica_largest_matrix_entries: usize,
    symbolica_peak_live_matrix_entries: usize,
    symbolica_input_retained_bytes: usize,
    symbolica_output_retained_bytes: usize,
    symbolica_determinant_calls: usize,
    symbolica_product_calls: usize,
    symbolica_transpose_calls: usize,
    guard_polynomials: usize,
    guard_origins: usize,
}

impl SymmetryVerificationStats {
    pub const fn matrix_entries(self) -> usize {
        self.matrix_entries
    }

    pub const fn exact_operations(self) -> usize {
        self.exact_operations
    }

    pub const fn determinant_states(self) -> usize {
        self.determinant_states
    }

    /// Exact arithmetic calls actually observed inside native Symbolica
    /// determinant and product sessions.
    pub const fn symbolica_exact_operations(self) -> usize {
        self.symbolica_exact_operations
    }

    /// Aggregate public-Symbolica operation envelope admitted before native
    /// execution.  This may exceed `symbolica_exact_operations` for a
    /// data-dependent determinant schedule.
    pub const fn symbolica_admitted_exact_operations(self) -> usize {
        self.symbolica_admitted_exact_operations
    }

    pub const fn symbolica_largest_matrix_entries(self) -> usize {
        self.symbolica_largest_matrix_entries
    }

    pub const fn symbolica_peak_live_matrix_entries(self) -> usize {
        self.symbolica_peak_live_matrix_entries
    }

    pub const fn symbolica_input_retained_bytes(self) -> usize {
        self.symbolica_input_retained_bytes
    }

    pub const fn symbolica_output_retained_bytes(self) -> usize {
        self.symbolica_output_retained_bytes
    }

    pub const fn symbolica_determinant_calls(self) -> usize {
        self.symbolica_determinant_calls
    }

    pub const fn symbolica_product_calls(self) -> usize {
        self.symbolica_product_calls
    }

    pub const fn symbolica_transpose_calls(self) -> usize {
        self.symbolica_transpose_calls
    }

    pub const fn guard_polynomials(self) -> usize {
        self.guard_polynomials
    }

    pub const fn guard_origins(self) -> usize {
        self.guard_origins
    }
}

/// Newly derived proof object.  All fields are private so callers cannot
/// construct a certificate without replay.
#[derive(Clone, Debug)]
pub struct VerifiedAffineFamilyMap {
    source_family_fingerprint: String,
    target_family_fingerprint: String,
    momentum: MomentumMap,
    scalar_products: AffineScalarProductMap,
    denominators: AffineDenominatorMap,
    row_actions: Box<[DenominatorRowAction]>,
    loop_determinant: Coefficient,
    external_determinant: Coefficient,
    jacobian: JacobianWitness,
    source_domain: FamilyDomain,
    target_domain: FamilyDomain,
    candidate_denominator_guards: Box<[BasePolynomial]>,
    replay_guards: Box<[SymmetryNonZeroCondition]>,
    stats: SymmetryVerificationStats,
}

impl VerifiedAffineFamilyMap {
    pub const SCHEMA: &'static str = AFFINE_FAMILY_MAP_V2_SCHEMA;

    pub fn source_family_fingerprint(&self) -> &str {
        &self.source_family_fingerprint
    }

    pub fn target_family_fingerprint(&self) -> &str {
        &self.target_family_fingerprint
    }

    pub const fn momentum(&self) -> &MomentumMap {
        &self.momentum
    }

    pub const fn scalar_products(&self) -> &AffineScalarProductMap {
        &self.scalar_products
    }

    pub const fn denominators(&self) -> &AffineDenominatorMap {
        &self.denominators
    }

    pub fn row_actions(&self) -> &[DenominatorRowAction] {
        &self.row_actions
    }

    pub const fn loop_determinant(&self) -> &Coefficient {
        &self.loop_determinant
    }

    pub const fn external_determinant(&self) -> &Coefficient {
        &self.external_determinant
    }

    pub const fn jacobian(&self) -> &JacobianWitness {
        &self.jacobian
    }

    pub const fn source_domain(&self) -> &FamilyDomain {
        &self.source_domain
    }

    pub const fn target_domain(&self) -> &FamilyDomain {
        &self.target_domain
    }

    /// Denominators of caller-supplied rational map entries, before any
    /// cancellation in derived matrices.
    pub fn candidate_denominator_guards(&self) -> &[BasePolynomial] {
        &self.candidate_denominator_guards
    }

    /// The exact numerator of `det(A)`, required to be nonzero.
    pub const fn determinant_nonzero_guard(&self) -> &BasePolynomial {
        &self.loop_determinant.numerator
    }

    /// Complete merged nonzero domain for replay, including both family
    /// domains, all candidate denominators, both determinant numerators, and
    /// every monomial denominator scale numerator.
    pub fn replay_guards(&self) -> &[SymmetryNonZeroCondition] {
        &self.replay_guards
    }

    pub const fn stats(&self) -> SymmetryVerificationStats {
        self.stats
    }

    /// Replay a retained map from its momentum witness.  Derived data is
    /// compared structurally, including the complete pre-cancellation domain.
    pub fn replay(
        &self,
        source: &IntegralFamily,
        target: &IntegralFamily,
        limits: SymmetryVerificationLimits,
    ) -> Result<(), SymmetryVerificationError> {
        let replayed = verify_affine_family_map(source, target, self.momentum.clone(), limits)?;
        if replayed.source_family_fingerprint != self.source_family_fingerprint
            || replayed.target_family_fingerprint != self.target_family_fingerprint
            || replayed.scalar_products != self.scalar_products
            || replayed.denominators != self.denominators
            || replayed.row_actions != self.row_actions
            || replayed.loop_determinant != self.loop_determinant
            || replayed.external_determinant != self.external_determinant
            || replayed.jacobian != self.jacobian
            || replayed.source_domain != self.source_domain
            || replayed.target_domain != self.target_domain
            || replayed.candidate_denominator_guards != self.candidate_denominator_guards
            || replayed.replay_guards != self.replay_guards
        {
            return Err(SymmetryVerificationError::CertificateReplayMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymmetryVerificationError {
    UnequalLoopCount {
        source: usize,
        target: usize,
    },
    UnequalExternalCount {
        source: usize,
        target: usize,
    },
    ForeignCoefficientContext,
    WrongMatrixShape {
        matrix: &'static str,
        expected_rows: usize,
        expected_columns: usize,
        actual_rows: usize,
        actual_columns: usize,
    },
    MatrixPayloadSize {
        rows: usize,
        columns: usize,
        expected: usize,
        actual: usize,
    },
    MatrixPayloadTooLarge {
        rows: usize,
        columns: usize,
        expected: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    ForeignMapCoefficient {
        matrix: &'static str,
        row: usize,
        column: usize,
    },
    SingularLoopMap,
    SingularExternalMap,
    ExternalGramMismatch {
        row: usize,
        column: usize,
    },
    DenominatorReplayMismatch {
        denominator: usize,
        coordinate: Option<usize>,
    },
    CertificateReplayMismatch,
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    InternalSymbolicaAlgebra {
        detail: String,
    },
    ExactAlgebra(ExactAlgebraError),
    Family(GenericFamilyError),
}

impl fmt::Display for SymmetryVerificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnequalLoopCount { source, target } => write!(
                formatter,
                "source has {source} loops but target has {target} loops"
            ),
            Self::UnequalExternalCount { source, target } => write!(
                formatter,
                "source has {source} external momenta but target has {target}"
            ),
            Self::ForeignCoefficientContext => formatter.write_str(
                "source and target do not share the authenticated coefficient variable map",
            ),
            Self::WrongMatrixShape {
                matrix,
                expected_rows,
                expected_columns,
                actual_rows,
                actual_columns,
            } => write!(
                formatter,
                "momentum matrix {matrix} is {actual_rows}x{actual_columns}, expected {expected_rows}x{expected_columns}"
            ),
            Self::MatrixPayloadSize {
                rows,
                columns,
                expected,
                actual,
            } => write!(
                formatter,
                "a {rows}x{columns} matrix needs {expected} entries, received {actual}"
            ),
            Self::MatrixPayloadTooLarge {
                rows,
                columns,
                expected,
            } => write!(
                formatter,
                "a {rows}x{columns} matrix needs exactly {expected} entries, but the payload contains more"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve storage for {requested} {resource}"
            ),
            Self::ForeignMapCoefficient {
                matrix,
                row,
                column,
            } => write!(
                formatter,
                "momentum matrix {matrix}[{row},{column}] uses a foreign coefficient map"
            ),
            Self::SingularLoopMap => {
                formatter.write_str("the exact loop-momentum matrix is singular")
            }
            Self::SingularExternalMap => {
                formatter.write_str("the exact external-momentum matrix is singular")
            }
            Self::ExternalGramMismatch { row, column } => write!(
                formatter,
                "external Gram transport fails at entry [{row},{column}]"
            ),
            Self::DenominatorReplayMismatch {
                denominator,
                coordinate,
            } => match coordinate {
                Some(coordinate) => write!(
                    formatter,
                    "affine denominator replay fails for D{denominator} at scalar coordinate {coordinate}"
                ),
                None => write!(
                    formatter,
                    "affine denominator replay fails for the constant of D{denominator}"
                ),
            },
            Self::CertificateReplayMismatch => {
                formatter.write_str("the retained affine-family certificate differs on replay")
            }
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "symmetry {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::InternalSymbolicaAlgebra { detail } => {
                write!(
                    formatter,
                    "native Symbolica symmetry algebra failed: {detail}"
                )
            }
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::Family(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SymmetryVerificationError {}

impl From<ExactAlgebraError> for SymmetryVerificationError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}

impl From<GenericFamilyError> for SymmetryVerificationError {
    fn from(value: GenericFamilyError) -> Self {
        Self::Family(value)
    }
}

/// Derive and independently replay the exact family map induced by an
/// explicit momentum substitution.
pub fn verify_affine_family_map(
    source: &IntegralFamily,
    target: &IntegralFamily,
    momentum: MomentumMap,
    limits: SymmetryVerificationLimits,
) -> Result<VerifiedAffineFamilyMap, SymmetryVerificationError> {
    if source.loop_count() != target.loop_count() {
        return Err(SymmetryVerificationError::UnequalLoopCount {
            source: source.loop_count(),
            target: target.loop_count(),
        });
    }
    if source.external_count() != target.external_count() {
        return Err(SymmetryVerificationError::UnequalExternalCount {
            source: source.external_count(),
            target: target.external_count(),
        });
    }
    if !source
        .coefficient_context()
        .has_same_variable_map(target.coefficient_context())
    {
        return Err(SymmetryVerificationError::ForeignCoefficientContext);
    }

    let loops = source.loop_count();
    let externals = source.external_count();
    check_shape(&momentum.loop_linear, "A", loops, loops)?;
    check_shape(&momentum.loop_external, "B", loops, externals)?;
    check_shape(&momentum.external_linear, "C", externals, externals)?;

    let mut algebra = ReplayAlgebra::new(source.coefficient_context(), limits);
    algebra.retain_matrix(&momentum.loop_linear, "A")?;
    algebra.retain_matrix(&momentum.loop_external, "B")?;
    algebra.retain_matrix(&momentum.external_linear, "C")?;

    let mut replay_guards = SymmetryGuardCollector::new(limits);
    replay_guards.add_family_domain(source.domain(), true)?;
    replay_guards.add_family_domain(target.domain(), false)?;
    let candidate_denominator_guards = collect_candidate_denominators(
        [
            ("A", &momentum.loop_linear),
            ("B", &momentum.loop_external),
            ("C", &momentum.external_linear),
        ],
        &mut replay_guards,
    )?;

    let loop_determinant = checked_determinant(&momentum.loop_linear, &mut algebra)?;
    if loop_determinant.is_zero() {
        return Err(SymmetryVerificationError::SingularLoopMap);
    }
    replay_guards.add(
        loop_determinant.numerator.clone(),
        SymmetryGuardOrigin::LoopMapDeterminantNumerator,
    )?;
    let external_determinant = checked_determinant(&momentum.external_linear, &mut algebra)?;
    if external_determinant.is_zero() {
        return Err(SymmetryVerificationError::SingularExternalMap);
    }
    replay_guards.add(
        external_determinant.numerator.clone(),
        SymmetryGuardOrigin::ExternalMapDeterminantNumerator,
    )?;

    verify_external_gram(source, target, &momentum, &mut algebra)?;
    let jacobian = classify_jacobian(&loop_determinant, &mut algebra)?;
    let scalar_products = derive_scalar_product_map(source, target, &momentum, &mut algebra)?;
    let denominators = derive_denominator_map(source, target, &scalar_products, &mut algebra)?;
    replay_denominator_map(source, target, &momentum, &denominators, &mut algebra)?;
    let row_actions = classify_rows(&denominators, &mut replay_guards)?;

    algebra.stats.guard_polynomials = replay_guards.polynomial_count();
    algebra.stats.guard_origins = replay_guards.origin_count();
    let replay_guards = replay_guards.finish();

    Ok(VerifiedAffineFamilyMap {
        source_family_fingerprint: source.fingerprint(),
        target_family_fingerprint: target.fingerprint(),
        momentum,
        scalar_products,
        denominators,
        row_actions: row_actions.into_boxed_slice(),
        loop_determinant,
        external_determinant,
        jacobian,
        source_domain: source.domain().clone(),
        target_domain: target.domain().clone(),
        candidate_denominator_guards: candidate_denominator_guards.into_boxed_slice(),
        replay_guards,
        stats: algebra.stats,
    })
}

fn check_shape<T>(
    matrix: &ExactMatrix<T>,
    name: &'static str,
    rows: usize,
    columns: usize,
) -> Result<(), SymmetryVerificationError> {
    if matrix.rows == rows && matrix.columns == columns {
        Ok(())
    } else {
        Err(SymmetryVerificationError::WrongMatrixShape {
            matrix: name,
            expected_rows: rows,
            expected_columns: columns,
            actual_rows: matrix.rows,
            actual_columns: matrix.columns,
        })
    }
}

struct ReplayAlgebra<'a> {
    context: &'a CoefficientContext,
    limits: SymmetryVerificationLimits,
    stats: SymmetryVerificationStats,
}

impl<'a> ReplayAlgebra<'a> {
    fn new(context: &'a CoefficientContext, limits: SymmetryVerificationLimits) -> Self {
        Self {
            context,
            limits,
            stats: SymmetryVerificationStats::default(),
        }
    }

    fn charge_entries(&mut self, entries: usize) -> Result<(), SymmetryVerificationError> {
        self.stats.matrix_entries = checked_add(
            self.stats.matrix_entries,
            entries,
            "retained symmetry matrix entries",
        )?;
        check_limit(
            "retained matrix entries",
            self.stats.matrix_entries,
            self.limits.max_matrix_entries,
        )
    }

    fn charge_operation(&mut self) -> Result<(), SymmetryVerificationError> {
        self.stats.exact_operations =
            checked_add(self.stats.exact_operations, 1, "exact symmetry operations")?;
        check_limit(
            "exact operations",
            self.stats.exact_operations,
            self.limits.max_exact_operations,
        )
    }

    fn remaining_symbolica_limits(
        &self,
    ) -> Result<SymbolicaCoefficientMatrixLimits, SymmetryVerificationError> {
        let max_exact_operations = self
            .limits
            .max_exact_operations
            .checked_sub(self.stats.exact_operations)
            .ok_or(SymmetryVerificationError::ResourceLimit {
                resource: "exact operations",
                requested: self.stats.exact_operations,
                limit: self.limits.max_exact_operations,
            })?;
        let max_input_retained_bytes = self
            .limits
            .max_symbolica_input_retained_bytes
            .checked_sub(self.stats.symbolica_input_retained_bytes)
            .ok_or(SymmetryVerificationError::ResourceLimit {
                resource: "Symbolica input retained bytes",
                requested: self.stats.symbolica_input_retained_bytes,
                limit: self.limits.max_symbolica_input_retained_bytes,
            })?;
        let max_output_retained_bytes = self
            .limits
            .max_symbolica_output_retained_bytes
            .checked_sub(self.stats.symbolica_output_retained_bytes)
            .ok_or(SymmetryVerificationError::ResourceLimit {
                resource: "Symbolica output retained bytes",
                requested: self.stats.symbolica_output_retained_bytes,
                limit: self.limits.max_symbolica_output_retained_bytes,
            })?;
        Ok(SymbolicaCoefficientMatrixLimits {
            exact_algebra: self.limits.exact_algebra,
            max_single_matrix_entries: self.limits.max_symbolica_single_matrix_entries,
            max_live_matrix_entries: self.limits.max_symbolica_live_matrix_entries,
            max_exact_operations,
            max_input_retained_bytes,
            max_output_retained_bytes,
        })
    }

    fn absorb_symbolica_stats(
        &mut self,
        stats: SymbolicaCoefficientMatrixStats,
    ) -> Result<(), SymmetryVerificationError> {
        // Symbolica preflights its complete native schedule.  Retaining that
        // admitted count, rather than a data-dependent prefix, makes the
        // aggregate limit replayable exactly at its reported boundary.
        let exact_operations = checked_add(
            self.stats.exact_operations,
            stats.admitted_exact_operations(),
            "exact symmetry operations",
        )?;
        check_limit(
            "exact operations",
            exact_operations,
            self.limits.max_exact_operations,
        )?;
        let symbolica_exact_operations = checked_add(
            self.stats.symbolica_exact_operations,
            stats.exact_operations(),
            "Symbolica exact operations",
        )?;
        let symbolica_admitted_exact_operations = checked_add(
            self.stats.symbolica_admitted_exact_operations,
            stats.admitted_exact_operations(),
            "admitted Symbolica exact operations",
        )?;
        let symbolica_input_retained_bytes = checked_add(
            self.stats.symbolica_input_retained_bytes,
            stats.input_retained_bytes(),
            "Symbolica input retained bytes",
        )?;
        check_limit(
            "Symbolica input retained bytes",
            symbolica_input_retained_bytes,
            self.limits.max_symbolica_input_retained_bytes,
        )?;
        let symbolica_output_retained_bytes = checked_add(
            self.stats.symbolica_output_retained_bytes,
            stats.output_retained_bytes(),
            "Symbolica output retained bytes",
        )?;
        check_limit(
            "Symbolica output retained bytes",
            symbolica_output_retained_bytes,
            self.limits.max_symbolica_output_retained_bytes,
        )?;
        let symbolica_determinant_calls = checked_add(
            self.stats.symbolica_determinant_calls,
            stats.determinant_calls(),
            "Symbolica determinant calls",
        )?;
        let symbolica_product_calls = checked_add(
            self.stats.symbolica_product_calls,
            stats.product_calls(),
            "Symbolica product calls",
        )?;
        let symbolica_transpose_calls = checked_add(
            self.stats.symbolica_transpose_calls,
            stats.transpose_calls(),
            "Symbolica transpose calls",
        )?;

        self.stats.exact_operations = exact_operations;
        self.stats.symbolica_exact_operations = symbolica_exact_operations;
        self.stats.symbolica_admitted_exact_operations = symbolica_admitted_exact_operations;
        self.stats.symbolica_largest_matrix_entries = self
            .stats
            .symbolica_largest_matrix_entries
            .max(stats.admitted_single_matrix_entries());
        self.stats.symbolica_peak_live_matrix_entries = self
            .stats
            .symbolica_peak_live_matrix_entries
            .max(stats.admitted_peak_live_entries());
        self.stats.symbolica_input_retained_bytes = symbolica_input_retained_bytes;
        self.stats.symbolica_output_retained_bytes = symbolica_output_retained_bytes;
        self.stats.symbolica_determinant_calls = symbolica_determinant_calls;
        self.stats.symbolica_product_calls = symbolica_product_calls;
        self.stats.symbolica_transpose_calls = symbolica_transpose_calls;
        Ok(())
    }

    fn map_symbolica_matrix_error(
        &self,
        error: SymbolicaCoefficientMatrixError,
    ) -> SymmetryVerificationError {
        map_symbolica_matrix_error(error, self.limits, self.stats)
    }

    fn retain_matrix(
        &mut self,
        matrix: &ExactMatrix<Coefficient>,
        name: &'static str,
    ) -> Result<(), SymmetryVerificationError> {
        self.charge_entries(matrix.entries().len())?;
        for row in 0..matrix.rows {
            for column in 0..matrix.columns {
                if let Err(error) = self
                    .context
                    .validate_with_limits(matrix.at(row, column), self.limits.exact_algebra)
                {
                    return Err(match error {
                        ExactAlgebraError::VariableMapMismatch { .. } => {
                            SymmetryVerificationError::ForeignMapCoefficient {
                                matrix: name,
                                row,
                                column,
                            }
                        }
                        other => SymmetryVerificationError::ExactAlgebra(other),
                    });
                }
            }
        }
        Ok(())
    }

    fn add(
        &mut self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, SymmetryVerificationError> {
        self.charge_operation()?;
        Ok(self
            .context
            .try_add(left, right, self.limits.exact_algebra)?)
    }

    fn sub(
        &mut self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, SymmetryVerificationError> {
        self.charge_operation()?;
        Ok(self
            .context
            .try_sub(left, right, self.limits.exact_algebra)?)
    }

    fn mul(
        &mut self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, SymmetryVerificationError> {
        self.charge_operation()?;
        Ok(self
            .context
            .try_mul(left, right, self.limits.exact_algebra)?)
    }

    fn add_product(
        &mut self,
        accumulator: Coefficient,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, SymmetryVerificationError> {
        let product = self.mul(left, right)?;
        self.add(&accumulator, &product)
    }

    fn equal(
        &mut self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<bool, SymmetryVerificationError> {
        Ok(self.sub(left, right)?.is_zero())
    }
}

struct SymmetryGuardCollector {
    limits: SymmetryVerificationLimits,
    conditions: Vec<SymmetryNonZeroCondition>,
    origin_count: usize,
}

impl SymmetryGuardCollector {
    fn new(limits: SymmetryVerificationLimits) -> Self {
        Self {
            limits,
            conditions: Vec::new(),
            origin_count: 0,
        }
    }

    fn add_family_domain(
        &mut self,
        domain: &FamilyDomain,
        source: bool,
    ) -> Result<(), SymmetryVerificationError> {
        for condition in domain.conditions() {
            for origin in condition.origins() {
                let origin = if source {
                    SymmetryGuardOrigin::SourceFamily(origin.clone())
                } else {
                    SymmetryGuardOrigin::TargetFamily(origin.clone())
                };
                self.add(condition.polynomial().clone(), origin)?;
            }
        }
        Ok(())
    }

    fn add(
        &mut self,
        polynomial: BasePolynomial,
        origin: SymmetryGuardOrigin,
    ) -> Result<(), SymmetryVerificationError> {
        if let Some(existing) = self
            .conditions
            .iter_mut()
            .find(|condition| condition.polynomial == polynomial)
        {
            if existing.origins.contains(&origin) {
                return Ok(());
            }
            let requested = checked_add(self.origin_count, 1, "symmetry guard origins")?;
            check_limit("guard origins", requested, self.limits.max_guard_origins)?;
            existing.origins.insert(origin);
            self.origin_count = requested;
            return Ok(());
        }

        let polynomial_count = checked_add(self.conditions.len(), 1, "symmetry guard polynomials")?;
        check_limit(
            "guard polynomials",
            polynomial_count,
            self.limits.max_guard_polynomials,
        )?;
        let origin_count = checked_add(self.origin_count, 1, "symmetry guard origins")?;
        check_limit("guard origins", origin_count, self.limits.max_guard_origins)?;
        self.conditions.push(SymmetryNonZeroCondition {
            polynomial,
            origins: BTreeSet::from([origin]),
        });
        self.origin_count = origin_count;
        Ok(())
    }

    fn polynomial_count(&self) -> usize {
        self.conditions.len()
    }

    fn origin_count(&self) -> usize {
        self.origin_count
    }

    fn finish(self) -> Box<[SymmetryNonZeroCondition]> {
        self.conditions.into_boxed_slice()
    }
}

fn verify_external_gram(
    source: &IntegralFamily,
    target: &IntegralFamily,
    momentum: &MomentumMap,
    algebra: &mut ReplayAlgebra<'_>,
) -> Result<(), SymmetryVerificationError> {
    let externals = source.external_count();
    if externals == 0 {
        return Ok(());
    }

    let transform = clone_exact_matrix_rows(
        &momentum.external_linear,
        algebra,
        "external Gram transform rows",
    )?;
    algebra.charge_entries(externals.checked_mul(externals).ok_or(
        SymmetryVerificationError::ResourceCountOverflow {
            resource: "mapped external Gram entries",
        },
    )?)?;
    let native_limits = algebra.remaining_symbolica_limits()?;
    let (mapped, stats) = match congruence_of_coefficient_matrix(
        algebra.context,
        &transform,
        target.external_gram(),
        native_limits,
    ) {
        Ok(result) => result,
        Err(error) => return Err(algebra.map_symbolica_matrix_error(error)),
    };
    algebra.absorb_symbolica_stats(stats)?;
    if mapped.len() != externals || mapped.iter().any(|row| row.len() != externals) {
        return Err(SymmetryVerificationError::InternalSymbolicaAlgebra {
            detail: "external Gram congruence returned the wrong shape".to_owned(),
        });
    }
    for mu in 0..externals {
        for nu in 0..externals {
            if !algebra.equal(&mapped[mu][nu], &source.external_gram()[mu][nu])? {
                return Err(SymmetryVerificationError::ExternalGramMismatch {
                    row: mu,
                    column: nu,
                });
            }
        }
    }
    Ok(())
}

fn derive_scalar_product_map(
    source: &IntegralFamily,
    target: &IntegralFamily,
    momentum: &MomentumMap,
    algebra: &mut ReplayAlgebra<'_>,
) -> Result<AffineScalarProductMap, SymmetryVerificationError> {
    let source_count = source.denominator_count();
    let target_count = target.denominator_count();
    let entries = source_count.checked_mul(target_count).ok_or(
        SymmetryVerificationError::ResourceCountOverflow {
            resource: "scalar-product map entries",
        },
    )?;
    algebra.charge_entries(checked_add(
        entries,
        source_count,
        "scalar-product map entries",
    )?)?;
    let mut constant = vec![algebra.context.zero(); source_count];
    let mut linear = vec![algebra.context.zero(); entries];

    for (source_coordinate, coordinate) in source.coordinates().iter().copied().enumerate() {
        match coordinate {
            ScalarProductCoordinate::LoopLoop { left, right } => {
                for alpha in 0..source.external_count() {
                    for beta in 0..source.external_count() {
                        let product = algebra.mul(
                            momentum.loop_external.at(left, alpha),
                            momentum.loop_external.at(right, beta),
                        )?;
                        let contribution =
                            algebra.mul(&product, &target.external_gram()[alpha][beta])?;
                        constant[source_coordinate] =
                            algebra.add(&constant[source_coordinate], &contribution)?;
                    }
                }
                for first in 0..source.loop_count() {
                    for second in first..source.loop_count() {
                        let target_coordinate =
                            target.coordinate_index(ScalarProductCoordinate::LoopLoop {
                                left: first,
                                right: second,
                            })?;
                        let value = if first == second {
                            algebra.mul(
                                momentum.loop_linear.at(left, first),
                                momentum.loop_linear.at(right, first),
                            )?
                        } else {
                            let direct = algebra.mul(
                                momentum.loop_linear.at(left, first),
                                momentum.loop_linear.at(right, second),
                            )?;
                            let crossed = algebra.mul(
                                momentum.loop_linear.at(left, second),
                                momentum.loop_linear.at(right, first),
                            )?;
                            algebra.add(&direct, &crossed)?
                        };
                        linear[source_coordinate * target_count + target_coordinate] = value;
                    }
                }
                for target_loop in 0..source.loop_count() {
                    for external in 0..source.external_count() {
                        let target_coordinate =
                            target.coordinate_index(ScalarProductCoordinate::LoopExternal {
                                loop_index: target_loop,
                                external_index: external,
                            })?;
                        let direct = algebra.mul(
                            momentum.loop_linear.at(left, target_loop),
                            momentum.loop_external.at(right, external),
                        )?;
                        let crossed = algebra.mul(
                            momentum.loop_linear.at(right, target_loop),
                            momentum.loop_external.at(left, external),
                        )?;
                        linear[source_coordinate * target_count + target_coordinate] =
                            algebra.add(&direct, &crossed)?;
                    }
                }
            }
            ScalarProductCoordinate::LoopExternal {
                loop_index,
                external_index,
            } => {
                for alpha in 0..source.external_count() {
                    for beta in 0..source.external_count() {
                        let product = algebra.mul(
                            momentum.loop_external.at(loop_index, alpha),
                            momentum.external_linear.at(external_index, beta),
                        )?;
                        let contribution =
                            algebra.mul(&product, &target.external_gram()[alpha][beta])?;
                        constant[source_coordinate] =
                            algebra.add(&constant[source_coordinate], &contribution)?;
                    }
                }
                for target_loop in 0..source.loop_count() {
                    for target_external in 0..source.external_count() {
                        let target_coordinate =
                            target.coordinate_index(ScalarProductCoordinate::LoopExternal {
                                loop_index: target_loop,
                                external_index: target_external,
                            })?;
                        linear[source_coordinate * target_count + target_coordinate] = algebra
                            .mul(
                                momentum.loop_linear.at(loop_index, target_loop),
                                momentum.external_linear.at(external_index, target_external),
                            )?;
                    }
                }
            }
        }
    }

    Ok(AffineScalarProductMap {
        constant: constant.into_boxed_slice(),
        linear: ExactMatrix::try_new_with_max_entries(
            source_count,
            target_count,
            linear,
            algebra.limits.max_matrix_entries,
        )?,
    })
}

fn derive_denominator_map(
    source: &IntegralFamily,
    target: &IntegralFamily,
    scalar_products: &AffineScalarProductMap,
    algebra: &mut ReplayAlgebra<'_>,
) -> Result<AffineDenominatorMap, SymmetryVerificationError> {
    let source_count = source.denominator_count();
    let target_count = target.denominator_count();
    let entries = source_count.checked_mul(target_count).ok_or(
        SymmetryVerificationError::ResourceCountOverflow {
            resource: "affine denominator map entries",
        },
    )?;
    algebra.charge_entries(checked_add(
        entries,
        source_count,
        "affine denominator map entries",
    )?)?;

    if source_count == 0 {
        return Ok(AffineDenominatorMap {
            constant: Box::new([]),
            linear: ExactMatrix::try_new_with_max_entries(
                0,
                0,
                std::iter::empty(),
                algebra.limits.max_matrix_entries,
            )?,
        });
    }

    let source_rows =
        clone_denominator_coefficient_rows(source, algebra, "source denominator coefficient rows")?;
    let scalar_rows = clone_exact_matrix_rows(
        &scalar_products.linear,
        algebra,
        "scalar-product linear rows",
    )?;

    // c_source + R_source h.  The matrix-vector product is native; only the
    // affine translation by c_source remains coefficient-level bookkeeping.
    let scalar_constant_column = clone_coefficient_column(
        &scalar_products.constant,
        algebra,
        "scalar-product constant column",
    )?;
    algebra.charge_entries(source_count)?;
    let transformed_shift = native_matrix_product(algebra, &source_rows, &scalar_constant_column)?;
    let mut transformed_constant = Vec::new();
    transformed_constant
        .try_reserve_exact(source_count)
        .map_err(|_| SymmetryVerificationError::AllocationFailure {
            resource: "transformed denominator constants",
            requested: source_count,
        })?;
    for denominator in 0..source_count {
        transformed_constant.push(algebra.add(
            source.denominators()[denominator].constant(),
            &transformed_shift[denominator][0],
        )?);
    }

    // P = R_source T R_target^-1.  Both ordinary products are owned by one
    // authenticated Symbolica session; RustRed retains only family semantics.
    let native_limits = algebra.remaining_symbolica_limits()?;
    let (denominator_linear, stats) = match multiply_three_coefficient_matrices(
        algebra.context,
        &source_rows,
        &scalar_rows,
        target.inverse_basis(),
        native_limits,
    ) {
        Ok(result) => result,
        Err(error) => return Err(algebra.map_symbolica_matrix_error(error)),
    };
    algebra.absorb_symbolica_stats(stats)?;
    if denominator_linear.len() != source_count
        || denominator_linear
            .iter()
            .any(|row| row.len() != target_count)
    {
        return Err(SymmetryVerificationError::InternalSymbolicaAlgebra {
            detail: "denominator linear map product returned the wrong shape".to_owned(),
        });
    }

    // b = transformed_constant - P c_target.  This matvec is native too.
    let target_constant_column =
        clone_denominator_constant_column(target, algebra, "target denominator constant column")?;
    algebra.charge_entries(source_count)?;
    let target_shift =
        native_matrix_product(algebra, &denominator_linear, &target_constant_column)?;
    let mut constant = Vec::new();
    constant.try_reserve_exact(source_count).map_err(|_| {
        SymmetryVerificationError::AllocationFailure {
            resource: "affine denominator constants",
            requested: source_count,
        }
    })?;
    for source_denominator in 0..source_count {
        constant.push(algebra.sub(
            &transformed_constant[source_denominator],
            &target_shift[source_denominator][0],
        )?);
    }

    Ok(AffineDenominatorMap {
        constant: constant.into_boxed_slice(),
        linear: ExactMatrix::try_new_with_max_entries(
            source_count,
            target_count,
            denominator_linear.into_iter().flatten(),
            algebra.limits.max_matrix_entries,
        )?,
    })
}

fn add_exact_product(
    accumulator: &mut Coefficient,
    factors: &[&Coefficient],
    algebra: &mut ReplayAlgebra<'_>,
) -> Result<(), SymmetryVerificationError> {
    debug_assert!(!factors.is_empty());
    let mut product = factors[0].clone();
    for factor in &factors[1..] {
        product = algebra.mul(&product, factor)?;
    }
    *accumulator = algebra.add(accumulator, &product)?;
    Ok(())
}

/// Replay each source denominator by a fresh bilinear expansion of `A/B/C/G`.
/// This route deliberately does not consume `AffineScalarProductMap`: ordered
/// loop-loop pairs are folded into the target upper triangle directly, so an
/// orientation or off-diagonal-factor defect in the retained map cannot
/// certify itself.
fn replay_denominator_map(
    source: &IntegralFamily,
    target: &IntegralFamily,
    momentum: &MomentumMap,
    denominators: &AffineDenominatorMap,
    algebra: &mut ReplayAlgebra<'_>,
) -> Result<(), SymmetryVerificationError> {
    let target_count = target.denominator_count();
    algebra.charge_entries(target_count)?;
    for source_denominator in 0..source.denominator_count() {
        let mut direct_constant = source.denominators()[source_denominator].constant().clone();
        let mut direct_linear = vec![algebra.context.zero(); target_count];
        for (source_coordinate, coordinate) in source.coordinates().iter().copied().enumerate() {
            let weight =
                &source.denominators()[source_denominator].coefficients()[source_coordinate];
            match coordinate {
                ScalarProductCoordinate::LoopLoop { left, right } => {
                    // Sum ordered target-loop pairs, then fold (a,b) and (b,a)
                    // into the same upper-triangular scalar coordinate.
                    for first in 0..source.loop_count() {
                        for second in 0..source.loop_count() {
                            let target_coordinate =
                                target.coordinate_index(ScalarProductCoordinate::LoopLoop {
                                    left: first.min(second),
                                    right: first.max(second),
                                })?;
                            add_exact_product(
                                &mut direct_linear[target_coordinate],
                                &[
                                    weight,
                                    momentum.loop_linear.at(left, first),
                                    momentum.loop_linear.at(right, second),
                                ],
                                algebra,
                            )?;
                        }
                    }
                    for target_loop in 0..source.loop_count() {
                        for external in 0..source.external_count() {
                            let target_coordinate =
                                target.coordinate_index(ScalarProductCoordinate::LoopExternal {
                                    loop_index: target_loop,
                                    external_index: external,
                                })?;
                            add_exact_product(
                                &mut direct_linear[target_coordinate],
                                &[
                                    weight,
                                    momentum.loop_linear.at(left, target_loop),
                                    momentum.loop_external.at(right, external),
                                ],
                                algebra,
                            )?;
                            add_exact_product(
                                &mut direct_linear[target_coordinate],
                                &[
                                    weight,
                                    momentum.loop_external.at(left, external),
                                    momentum.loop_linear.at(right, target_loop),
                                ],
                                algebra,
                            )?;
                        }
                    }
                    for alpha in 0..source.external_count() {
                        for beta in 0..source.external_count() {
                            add_exact_product(
                                &mut direct_constant,
                                &[
                                    weight,
                                    momentum.loop_external.at(left, alpha),
                                    momentum.loop_external.at(right, beta),
                                    &target.external_gram()[alpha][beta],
                                ],
                                algebra,
                            )?;
                        }
                    }
                }
                ScalarProductCoordinate::LoopExternal {
                    loop_index,
                    external_index,
                } => {
                    for target_loop in 0..source.loop_count() {
                        for target_external in 0..source.external_count() {
                            let target_coordinate =
                                target.coordinate_index(ScalarProductCoordinate::LoopExternal {
                                    loop_index: target_loop,
                                    external_index: target_external,
                                })?;
                            add_exact_product(
                                &mut direct_linear[target_coordinate],
                                &[
                                    weight,
                                    momentum.loop_linear.at(loop_index, target_loop),
                                    momentum.external_linear.at(external_index, target_external),
                                ],
                                algebra,
                            )?;
                        }
                    }
                    for alpha in 0..source.external_count() {
                        for beta in 0..source.external_count() {
                            add_exact_product(
                                &mut direct_constant,
                                &[
                                    weight,
                                    momentum.loop_external.at(loop_index, alpha),
                                    momentum.external_linear.at(external_index, beta),
                                    &target.external_gram()[alpha][beta],
                                ],
                                algebra,
                            )?;
                        }
                    }
                }
            }
        }

        let mut mapped_constant = denominators.constant[source_denominator].clone();
        for target_denominator in 0..target.denominator_count() {
            mapped_constant = algebra.add_product(
                mapped_constant,
                denominators
                    .linear
                    .at(source_denominator, target_denominator),
                target.denominators()[target_denominator].constant(),
            )?;
        }
        if !algebra.equal(&direct_constant, &mapped_constant)? {
            return Err(SymmetryVerificationError::DenominatorReplayMismatch {
                denominator: source_denominator,
                coordinate: None,
            });
        }

        for target_coordinate in 0..target_count {
            let mut mapped = algebra.context.zero();
            for target_denominator in 0..target.denominator_count() {
                mapped = algebra.add_product(
                    mapped,
                    denominators
                        .linear
                        .at(source_denominator, target_denominator),
                    &target.denominators()[target_denominator].coefficients()[target_coordinate],
                )?;
            }
            if !algebra.equal(&direct_linear[target_coordinate], &mapped)? {
                return Err(SymmetryVerificationError::DenominatorReplayMismatch {
                    denominator: source_denominator,
                    coordinate: Some(target_coordinate),
                });
            }
        }
    }
    Ok(())
}

fn classify_rows(
    map: &AffineDenominatorMap,
    guards: &mut SymmetryGuardCollector,
) -> Result<Vec<DenominatorRowAction>, SymmetryVerificationError> {
    let mut actions = Vec::with_capacity(map.linear.rows);
    for row in 0..map.linear.rows {
        if !map.constant[row].is_zero() {
            actions.push(DenominatorRowAction::Affine);
            continue;
        }
        let mut nonzero =
            (0..map.linear.columns).filter(|&column| !map.linear.at(row, column).is_zero());
        let Some(target) = nonzero.next() else {
            actions.push(DenominatorRowAction::Affine);
            continue;
        };
        if nonzero.next().is_some() {
            actions.push(DenominatorRowAction::Affine);
            continue;
        }
        let scale = map.linear.at(row, target).clone();
        guards.add(
            scale.numerator.clone(),
            SymmetryGuardOrigin::DenominatorScaleNumerator {
                source_denominator: row,
                target_denominator: target,
            },
        )?;
        actions.push(DenominatorRowAction::Monomial { target, scale });
    }
    Ok(actions)
}

fn classify_jacobian(
    determinant: &Coefficient,
    algebra: &mut ReplayAlgebra<'_>,
) -> Result<JacobianWitness, SymmetryVerificationError> {
    let one = algebra.context.one();
    if algebra.equal(determinant, &one)? {
        return Ok(JacobianWitness::Unit {
            determinant_sign: 1,
        });
    }
    let negative_one = algebra.context.integer(-1);
    if algebra.equal(determinant, &negative_one)? {
        return Ok(JacobianWitness::Unit {
            determinant_sign: -1,
        });
    }
    Ok(JacobianWitness::FormalDeterminantPower {
        determinant: determinant.clone(),
    })
}

/// Compute an exact determinant through Symbolica's public matrix API.
///
/// Symbolica 2.2.0 reports a `0 x 0` determinant as singular, while the empty
/// external-momentum map of a vacuum family has determinant one.  That unique
/// structural case is handled before entering the native boundary.
fn checked_determinant(
    matrix: &ExactMatrix<Coefficient>,
    algebra: &mut ReplayAlgebra<'_>,
) -> Result<Coefficient, SymmetryVerificationError> {
    debug_assert_eq!(matrix.rows, matrix.columns);
    if matrix.rows == 0 {
        return Ok(algebra.context.one());
    }

    let rows = clone_exact_matrix_rows(matrix, algebra, "determinant input rows")?;
    let limits = algebra.remaining_symbolica_limits()?;
    let (determinant, stats) =
        match determinant_of_coefficient_matrix(algebra.context, &rows, limits) {
            Ok(result) => result,
            Err(error) => return Err(algebra.map_symbolica_matrix_error(error)),
        };
    algebra.absorb_symbolica_stats(stats)?;
    Ok(determinant)
}

fn clone_exact_matrix_rows(
    matrix: &ExactMatrix<Coefficient>,
    algebra: &mut ReplayAlgebra<'_>,
    resource: &'static str,
) -> Result<Vec<Vec<Coefficient>>, SymmetryVerificationError> {
    algebra.charge_entries(matrix.entries().len())?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(matrix.rows).map_err(|_| {
        SymmetryVerificationError::AllocationFailure {
            resource,
            requested: matrix.rows,
        }
    })?;
    for row in 0..matrix.rows {
        let mut values = Vec::new();
        values.try_reserve_exact(matrix.columns).map_err(|_| {
            SymmetryVerificationError::AllocationFailure {
                resource,
                requested: matrix.columns,
            }
        })?;
        values.extend((0..matrix.columns).map(|column| matrix.at(row, column).clone()));
        rows.push(values);
    }
    Ok(rows)
}

fn clone_denominator_coefficient_rows(
    family: &IntegralFamily,
    algebra: &mut ReplayAlgebra<'_>,
    resource: &'static str,
) -> Result<Vec<Vec<Coefficient>>, SymmetryVerificationError> {
    let rows = family.denominator_count();
    let entries = rows
        .checked_mul(rows)
        .ok_or(SymmetryVerificationError::ResourceCountOverflow { resource })?;
    algebra.charge_entries(entries)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(rows)
        .map_err(|_| SymmetryVerificationError::AllocationFailure {
            resource,
            requested: rows,
        })?;
    for denominator in family.denominators() {
        let mut row = Vec::new();
        row.try_reserve_exact(rows)
            .map_err(|_| SymmetryVerificationError::AllocationFailure {
                resource,
                requested: rows,
            })?;
        row.extend(denominator.coefficients().iter().cloned());
        output.push(row);
    }
    Ok(output)
}

fn clone_coefficient_column(
    values: &[Coefficient],
    algebra: &mut ReplayAlgebra<'_>,
    resource: &'static str,
) -> Result<Vec<Vec<Coefficient>>, SymmetryVerificationError> {
    algebra.charge_entries(values.len())?;
    let mut output = Vec::new();
    output.try_reserve_exact(values.len()).map_err(|_| {
        SymmetryVerificationError::AllocationFailure {
            resource,
            requested: values.len(),
        }
    })?;
    for value in values {
        let mut row = Vec::new();
        row.try_reserve_exact(1)
            .map_err(|_| SymmetryVerificationError::AllocationFailure {
                resource,
                requested: 1,
            })?;
        row.push(value.clone());
        output.push(row);
    }
    Ok(output)
}

fn clone_denominator_constant_column(
    family: &IntegralFamily,
    algebra: &mut ReplayAlgebra<'_>,
    resource: &'static str,
) -> Result<Vec<Vec<Coefficient>>, SymmetryVerificationError> {
    let rows = family.denominator_count();
    algebra.charge_entries(rows)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(rows)
        .map_err(|_| SymmetryVerificationError::AllocationFailure {
            resource,
            requested: rows,
        })?;
    for denominator in family.denominators() {
        let mut row = Vec::new();
        row.try_reserve_exact(1)
            .map_err(|_| SymmetryVerificationError::AllocationFailure {
                resource,
                requested: 1,
            })?;
        row.push(denominator.constant().clone());
        output.push(row);
    }
    Ok(output)
}

fn native_matrix_product(
    algebra: &mut ReplayAlgebra<'_>,
    left: &[Vec<Coefficient>],
    right: &[Vec<Coefficient>],
) -> Result<Vec<Vec<Coefficient>>, SymmetryVerificationError> {
    let expected_rows = left.len();
    let expected_columns = right.first().map_or(0, Vec::len);
    let limits = algebra.remaining_symbolica_limits()?;
    let (product, stats) = match multiply_coefficient_matrices(algebra.context, left, right, limits)
    {
        Ok(result) => result,
        Err(error) => return Err(algebra.map_symbolica_matrix_error(error)),
    };
    algebra.absorb_symbolica_stats(stats)?;
    if product.len() != expected_rows || product.iter().any(|row| row.len() != expected_columns) {
        return Err(SymmetryVerificationError::InternalSymbolicaAlgebra {
            detail: "matrix product returned the wrong shape".to_owned(),
        });
    }
    Ok(product)
}

fn map_symbolica_matrix_error(
    error: SymbolicaCoefficientMatrixError,
    limits: SymmetryVerificationLimits,
    stats: SymmetryVerificationStats,
) -> SymmetryVerificationError {
    match error {
        SymbolicaCoefficientMatrixError::ResourceLimit {
            resource: "Symbolica coefficient matrix exact operations",
            requested,
            ..
        }
        | SymbolicaCoefficientMatrixError::ExactAlgebra(ExactAlgebraError::ResourceLimit {
            resource: "Symbolica coefficient matrix exact operations",
            requested,
            ..
        }) => aggregate_symbolica_resource_limit(
            "exact operations",
            stats.exact_operations,
            requested,
            limits.max_exact_operations,
        ),
        SymbolicaCoefficientMatrixError::ResourceLimit {
            resource: "coefficient matrix input retained bytes",
            requested,
            ..
        } => aggregate_symbolica_resource_limit(
            "Symbolica input retained bytes",
            stats.symbolica_input_retained_bytes,
            requested,
            limits.max_symbolica_input_retained_bytes,
        ),
        SymbolicaCoefficientMatrixError::ResourceLimit {
            resource: "coefficient matrix output retained bytes",
            requested,
            ..
        } => aggregate_symbolica_resource_limit(
            "Symbolica output retained bytes",
            stats.symbolica_output_retained_bytes,
            requested,
            limits.max_symbolica_output_retained_bytes,
        ),
        SymbolicaCoefficientMatrixError::ResourceLimit {
            resource: "single Symbolica matrix entries",
            requested,
            ..
        } => SymmetryVerificationError::ResourceLimit {
            resource: "Symbolica single matrix entries",
            requested,
            limit: limits.max_symbolica_single_matrix_entries,
        },
        SymbolicaCoefficientMatrixError::ResourceLimit {
            resource: "live Symbolica matrix entries",
            requested,
            ..
        } => SymmetryVerificationError::ResourceLimit {
            resource: "Symbolica live matrix entries",
            requested,
            limit: limits.max_symbolica_live_matrix_entries,
        },
        SymbolicaCoefficientMatrixError::ResourceLimit {
            resource,
            requested,
            limit,
        } => SymmetryVerificationError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        SymbolicaCoefficientMatrixError::ResourceCountOverflow { resource }
        | SymbolicaCoefficientMatrixError::ExactAlgebra(
            ExactAlgebraError::ResourceCountOverflow { resource },
        )
        | SymbolicaCoefficientMatrixError::InvalidCoefficient {
            error: ExactAlgebraError::ResourceCountOverflow { resource },
            ..
        } => SymmetryVerificationError::ResourceCountOverflow { resource },
        SymbolicaCoefficientMatrixError::AllocationFailure {
            resource,
            requested,
        } => SymmetryVerificationError::AllocationFailure {
            resource,
            requested,
        },
        SymbolicaCoefficientMatrixError::DimensionOverflow { .. } => {
            SymmetryVerificationError::ResourceCountOverflow {
                resource: "Symbolica matrix dimensions",
            }
        }
        SymbolicaCoefficientMatrixError::ExactAlgebra(error)
        | SymbolicaCoefficientMatrixError::InvalidCoefficient { error, .. } => {
            SymmetryVerificationError::ExactAlgebra(error)
        }
        internal => SymmetryVerificationError::InternalSymbolicaAlgebra {
            detail: internal.to_string(),
        },
    }
}

fn aggregate_symbolica_resource_limit(
    resource: &'static str,
    current: usize,
    local_requested: usize,
    limit: usize,
) -> SymmetryVerificationError {
    match current.checked_add(local_requested) {
        Some(requested) => SymmetryVerificationError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        None => SymmetryVerificationError::ResourceCountOverflow { resource },
    }
}

fn collect_candidate_denominators<'a>(
    matrices: impl IntoIterator<Item = (&'static str, &'a ExactMatrix<Coefficient>)>,
    guards: &mut SymmetryGuardCollector,
) -> Result<Vec<BasePolynomial>, SymmetryVerificationError> {
    let mut candidate_denominators = Vec::new();
    for (name, matrix) in matrices {
        for row in 0..matrix.rows {
            for column in 0..matrix.columns {
                let coefficient = matrix.at(row, column);
                let denominator = coefficient.denominator.clone();
                if denominator.is_one() {
                    continue;
                }
                guards.add(
                    denominator.clone(),
                    SymmetryGuardOrigin::MomentumMapDenominator {
                        matrix: name,
                        row,
                        column,
                    },
                )?;
                if !candidate_denominators
                    .iter()
                    .any(|guard| guard == &denominator)
                {
                    candidate_denominators.push(denominator);
                }
            }
        }
    }
    Ok(candidate_denominators)
}

fn checked_add(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, SymmetryVerificationError> {
    left.checked_add(right)
        .ok_or(SymmetryVerificationError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SymmetryVerificationError> {
    if requested > limit {
        Err(SymmetryVerificationError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
