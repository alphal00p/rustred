//! Bounded simultaneous integer-affine elimination for residual branches.
//!
//! The input rows are topology-independent equations
//!
//! ```text
//! c + a_0*n_0 + ... + a_(N-1)*n_(N-1) = 0
//! ```
//!
//! recognized by [`crate::ResidualAffineAtomRowCertificate`].  This layer
//! deliberately solves only the primitive unit-pivot cylinder supported by
//! LiteRed-style dependent starts.  At every search node an original index
//! column is eligible precisely when the gcd of its active entries is one.
//! Exact Bezout row operations create a positive unit pivot, after which that
//! column is eliminated everywhere.  Candidate columns are visited in
//! increasing original-coordinate order by deterministic depth-first search.
//!
//! A consistent system outside this original-coordinate unit-pivot graph
//! boundary is typed unsupported, never silently widened.  Such a system may
//! need a genuine congruence parameterization, or it may admit an integral
//! parameterization only after mixing original coordinates (for example,
//! `2*n_0 + 3*n_1 = 0`).  All arithmetic uses Symbolica's arbitrary-precision
//! [`Integer`]; no Symbolica matrix or machine-integer narrowing is used.

use std::cmp::Ordering;
use std::fmt;
use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::prelude::Integer;

use crate::ResidualAffinePrimitiveRow;

pub const RESIDUAL_AFFINE_INTEGER_SYSTEM_V1_SCHEMA: &str =
    "rustred-residual-affine-integer-system-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResidualAffineIntegerSystemLimits {
    pub max_ambient_arity: usize,
    pub max_input_rows: usize,
    pub max_input_components: usize,
    pub max_input_lineage_ordinals: usize,
    pub max_canonical_rows: usize,
    pub max_canonical_comparisons: usize,
    pub max_prework_operations: usize,
    pub max_allocation_entries_reserved: usize,
    pub max_integer_coefficient_bits: usize,
    /// Cumulative conservative bit-work charged before every retained clone,
    /// primitive-row validation, addition, multiplication, division, or
    /// negation, plus explicit deep-verification comparisons. Successful
    /// stats report this certified prospective charge, not merely the bit
    /// lengths of already-materialized results.
    pub max_integer_bit_work: usize,
    pub max_lineage_operations: usize,
    pub max_lineage_entries_materialized: usize,
    pub max_dfs_states: usize,
    pub max_dfs_depth: usize,
    pub max_frontier_states: usize,
    pub max_state_entries_materialized: usize,
    pub max_search_operations: usize,
    pub max_euclidean_steps: usize,
    pub max_row_operations: usize,
    pub max_operation_integer_entries: usize,
    pub max_map_entries: usize,
    pub max_verification_operations: usize,
}

impl Default for ResidualAffineIntegerSystemLimits {
    fn default() -> Self {
        Self {
            max_ambient_arity: 4096,
            max_input_rows: 1_000_000,
            max_input_components: 256_000_000,
            max_input_lineage_ordinals: 256_000_000,
            max_canonical_rows: 1_000_000,
            max_canonical_comparisons: 1_000_000_000,
            max_prework_operations: 1_000_000_000,
            max_allocation_entries_reserved: 4_000_000_000,
            max_integer_coefficient_bits: 1_000_000,
            max_integer_bit_work: 1_000_000_000_000_000,
            max_lineage_operations: 4_000_000_000,
            max_lineage_entries_materialized: 4_000_000_000,
            max_dfs_states: 10_000_000,
            max_dfs_depth: 4096,
            max_frontier_states: 10_000_000,
            max_state_entries_materialized: 16_000_000_000,
            max_search_operations: 100_000_000_000,
            max_euclidean_steps: 100_000_000_000,
            max_row_operations: 10_000_000_000,
            max_operation_integer_entries: 100_000_000_000,
            max_map_entries: 16_781_312,
            max_verification_operations: 100_000_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResidualAffineIntegerSystemStats {
    ambient_arity: usize,
    input_rows: usize,
    input_components: usize,
    input_lineage_ordinals: usize,
    canonical_rows: usize,
    canonical_comparisons: usize,
    prework_operations: usize,
    allocation_entries_reserved: usize,
    largest_integer_coefficient_bits: usize,
    integer_bit_work: usize,
    lineage_operations: usize,
    lineage_entries_materialized: usize,
    dfs_states: usize,
    deepest_dfs_depth: usize,
    frontier_states_peak: usize,
    state_entries_materialized: usize,
    search_operations: usize,
    euclidean_steps: usize,
    row_operations: usize,
    operation_integer_entries: usize,
    rank: usize,
    free_positions: usize,
    map_entries: usize,
    verification_operations: usize,
}

macro_rules! stats_getters {
    ($($field:ident),* $(,)?) => {$ (
        pub const fn $field(self) -> usize { self.$field }
    )* };
}

impl ResidualAffineIntegerSystemStats {
    stats_getters!(
        ambient_arity,
        input_rows,
        input_components,
        input_lineage_ordinals,
        canonical_rows,
        canonical_comparisons,
        prework_operations,
        allocation_entries_reserved,
        largest_integer_coefficient_bits,
        integer_bit_work,
        lineage_operations,
        lineage_entries_materialized,
        dfs_states,
        deepest_dfs_depth,
        frontier_states_peak,
        state_entries_materialized,
        search_operations,
        euclidean_steps,
        row_operations,
        operation_integer_entries,
        rank,
        free_positions,
        map_entries,
        verification_operations,
    );
}

/// Allocation-independent logical-memory envelope derived only from the
/// existing V1 integer-system limits.
///
/// This is a V2 parent preflight seam, not part of the frozen V1 certificate
/// payload. Shared inputs are excluded and every proportional term is checked
/// before the fresh compiler enters integer-system construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResidualAffineIntegerSystemMemoryEnvelope {
    retained_owned_logical_bytes_upper_bound: usize,
    compilation_owned_logical_peak_upper_bound: usize,
}

impl ResidualAffineIntegerSystemMemoryEnvelope {
    pub(crate) const fn retained_owned_logical_bytes_upper_bound(self) -> usize {
        self.retained_owned_logical_bytes_upper_bound
    }

    pub(crate) const fn compilation_owned_logical_peak_upper_bound(self) -> usize {
        self.compilation_owned_logical_peak_upper_bound
    }
}

/// Compile-only counters which the frozen V1 successful statistics already
/// contain, but which were previously lost when DFS returned `Unsupported`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ResidualAffineIntegerSystemRawTransientCensus {
    allocation_entries_reserved: usize,
    state_entries_materialized: usize,
    integer_bit_work: usize,
    frontier_states_peak: usize,
}

impl ResidualAffineIntegerSystemRawTransientCensus {
    const fn from_stats(stats: ResidualAffineIntegerSystemStats) -> Self {
        Self {
            allocation_entries_reserved: stats.allocation_entries_reserved,
            state_entries_materialized: stats.state_entries_materialized,
            integer_bit_work: stats.integer_bit_work,
            frontier_states_peak: stats.frontier_states_peak,
        }
    }

    pub(crate) const fn allocation_entries_reserved(self) -> usize {
        self.allocation_entries_reserved
    }

    pub(crate) const fn state_entries_materialized(self) -> usize {
        self.state_entries_materialized
    }

    pub(crate) const fn integer_bit_work(self) -> usize {
        self.integer_bit_work
    }

    pub(crate) const fn frontier_states_peak(self) -> usize {
        self.frontier_states_peak
    }
}

/// Complete allocation-independent census for two independently allocated,
/// equal integer-system payload operands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ResidualAffineIntegerSystemPayloadComparisonCensus {
    units: usize,
    bytes: usize,
    integer_bits: usize,
}

impl ResidualAffineIntegerSystemPayloadComparisonCensus {
    pub(crate) const fn units(self) -> usize {
        self.units
    }

    pub(crate) const fn bytes(self) -> usize {
        self.bytes
    }

    pub(crate) const fn integer_bits(self) -> usize {
        self.integer_bits
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResidualAffineIntegerSystemInputError {
    EmptyStructuralLocusLineage,
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
}

impl fmt::Display for ResidualAffineIntegerSystemInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyStructuralLocusLineage => formatter
                .write_str("an affine-system input row must retain structural-locus lineage"),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "affine-system input {resource} requested {requested}, configured limit is {limit}"
            ),
        }
    }
}

impl std::error::Error for ResidualAffineIntegerSystemInputError {}

/// One primitive equation plus caller-supplied Boolean-cover atom lineage.
///
/// The lineage is canonicalized before retention and is therefore always
/// nonempty, sorted, and duplicate-free. This structural container does not
/// authenticate the ordinals against a Boolean-cover certificate; the
/// enclosing residual-branch certificate must bind and replay that claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResidualAffineIntegerSystemInputRow {
    row: ResidualAffinePrimitiveRow,
    structural_locus_ordinals: Vec<usize>,
}

impl ResidualAffineIntegerSystemInputRow {
    pub fn try_new(
        row: ResidualAffinePrimitiveRow,
        mut structural_locus_ordinals: Vec<usize>,
        max_structural_locus_ordinals: usize,
    ) -> Result<Self, ResidualAffineIntegerSystemInputError> {
        if structural_locus_ordinals.len() > max_structural_locus_ordinals {
            return Err(ResidualAffineIntegerSystemInputError::ResourceLimit {
                resource: "structural-locus ordinals",
                requested: structural_locus_ordinals.len(),
                limit: max_structural_locus_ordinals,
            });
        }
        if structural_locus_ordinals.is_empty() {
            return Err(ResidualAffineIntegerSystemInputError::EmptyStructuralLocusLineage);
        }
        structural_locus_ordinals.sort_unstable();
        structural_locus_ordinals.dedup();
        Ok(Self {
            row,
            structural_locus_ordinals,
        })
    }

    pub const fn row(&self) -> &ResidualAffinePrimitiveRow {
        &self.row
    }

    pub fn structural_locus_ordinals(&self) -> &[usize] {
        &self.structural_locus_ordinals
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualAffineIntegerSystemUnsupported {
    /// No complete elimination path exists using unit pivots in original
    /// coordinate columns.
    ///
    /// The historical variant name is retained for public compatibility.  It
    /// includes, but is not limited to, systems with a genuine congruence
    /// obstruction: an integral graph can also require a unimodular mixing of
    /// original coordinates before a unit pivot is visible.
    GeneralCongruenceCaseNotSupported { remaining_equations: usize },
}

impl fmt::Display for ResidualAffineIntegerSystemUnsupported {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GeneralCongruenceCaseNotSupported {
                remaining_equations,
            } => write!(
                formatter,
                "no complete original-coordinate unit-pivot graph exists (first dead end retained {remaining_equations} active equations)"
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResidualAffineIntegerSystemError {
    SchemaMismatch,
    ReplayMismatch,
    ArityMismatch {
        row_ordinal: usize,
        expected: usize,
        actual: usize,
    },
    Unsupported(ResidualAffineIntegerSystemUnsupported),
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
    ArithmeticInvariantFailure(&'static str),
    SymbolicaPanic,
}

impl fmt::Display for ResidualAffineIntegerSystemError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("residual affine-system schema mismatch"),
            Self::ReplayMismatch => formatter.write_str("residual affine system did not replay"),
            Self::ArityMismatch {
                row_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "affine-system row {row_ordinal} has arity {actual}, expected {expected}"
            ),
            Self::Unsupported(reason) => {
                write!(formatter, "unsupported residual affine system: {reason}")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "residual affine-system {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "residual affine-system {resource} count overflowed usize"
                )
            }
            Self::AllocationFailure { resource } => write!(
                formatter,
                "residual affine-system {resource} allocation failed after bounded preflight"
            ),
            Self::ArithmeticInvariantFailure(message) => {
                write!(
                    formatter,
                    "affine-system arithmetic invariant failed: {message}"
                )
            }
            Self::SymbolicaPanic => {
                formatter.write_str("Symbolica panicked during residual affine-system elimination")
            }
        }
    }
}

impl std::error::Error for ResidualAffineIntegerSystemError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResidualAffineIntegerRowOperation {
    Swap {
        left_row: usize,
        right_row: usize,
    },
    BezoutPair {
        pivot_row: usize,
        other_row: usize,
        column: usize,
        pivot_coefficient: Integer,
        other_coefficient: Integer,
        gcd: Integer,
        pivot_bezout: Integer,
        other_bezout: Integer,
    },
    Negate {
        row: usize,
    },
    Eliminate {
        target_row: usize,
        pivot_row: usize,
        column: usize,
        multiple: Integer,
    },
    ExactNormalize {
        row: usize,
        divisor: Integer,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResidualAffineIntegerFinalRow {
    components: Vec<Integer>,
    structural_locus_ordinals: Vec<usize>,
}

impl ResidualAffineIntegerFinalRow {
    pub fn constant(&self) -> &Integer {
        &self.components[0]
    }

    pub fn coefficients(&self) -> &[Integer] {
        &self.components[1..]
    }

    pub fn components(&self) -> &[Integer] {
        &self.components
    }

    pub fn structural_locus_ordinals(&self) -> &[usize] {
        &self.structural_locus_ordinals
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResidualAffineIntegerEmptyWitness {
    ZeroEqualsNonzero {
        row: usize,
        constant: Integer,
        structural_locus_ordinals: Vec<usize>,
    },
    CoefficientGcdDoesNotDivideConstant {
        row: usize,
        constant: Integer,
        coefficient_gcd: Integer,
        remainder: Integer,
        structural_locus_ordinals: Vec<usize>,
    },
}

/// Per-query bounds for exact membership in a retained integer-affine map.
///
/// The query performs two complete matrix passes: one allocation-free
/// prospective arithmetic census and, only after that census succeeds, one
/// exact evaluation pass.  `max_matrix_entries_inspected` therefore governs
/// both passes rather than only the GMP-producing pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResidualAffineIntegerMapPointLimits {
    pub(crate) max_ambient_arity: usize,
    pub(crate) max_matrix_entries_inspected: usize,
    pub(crate) max_nonzero_multiplications: usize,
    pub(crate) max_additions: usize,
    pub(crate) max_fixed_point_comparisons: usize,
    /// Peak owner-visible and conservative transient GMP storage.  The
    /// preflight derives this from four simultaneously live `Integer` slots,
    /// each with a whole-limb payload at `largest_integer_bits`; four covers
    /// the accumulator, product/input temporary, and a reallocating output.
    pub(crate) max_peak_temporary_bytes: usize,
    pub(crate) max_integer_bits: usize,
    /// Conservative cumulative GMP work charged before any GMP arithmetic.
    /// Multiplication charges both operand-bit product and prospective output
    /// bits; clones, additions, and comparisons charge prospective result or
    /// comparison bits.
    pub(crate) max_integer_bit_work: usize,
}

impl Default for ResidualAffineIntegerMapPointLimits {
    fn default() -> Self {
        Self {
            max_ambient_arity: 4096,
            max_matrix_entries_inspected: 34_000_000,
            max_nonzero_multiplications: 16_781_312,
            max_additions: 16_781_312,
            max_fixed_point_comparisons: 4096,
            max_peak_temporary_bytes: 64 * 1024 * 1024,
            max_integer_bits: 1_000_000,
            max_integer_bit_work: 1_000_000_000_000_000,
        }
    }
}

/// Exact prospective and executed work for one map-membership query.
///
/// The integer fields are a certified upper-bound census for the arithmetic
/// that was subsequently executed, not a post-hoc measurement of possibly
/// cancellation-shortened GMP results.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ResidualAffineIntegerMapPointStats {
    ambient_arity: usize,
    matrix_entries_inspected: usize,
    nonzero_multiplications: usize,
    additions: usize,
    fixed_point_comparisons: usize,
    peak_temporary_bytes: usize,
    largest_integer_bits: usize,
    integer_bit_work: usize,
}

impl ResidualAffineIntegerMapPointStats {
    pub(crate) const fn ambient_arity(self) -> usize {
        self.ambient_arity
    }

    pub(crate) const fn matrix_entries_inspected(self) -> usize {
        self.matrix_entries_inspected
    }

    pub(crate) const fn nonzero_multiplications(self) -> usize {
        self.nonzero_multiplications
    }

    pub(crate) const fn additions(self) -> usize {
        self.additions
    }

    pub(crate) const fn fixed_point_comparisons(self) -> usize {
        self.fixed_point_comparisons
    }

    pub(crate) const fn peak_temporary_bytes(self) -> usize {
        self.peak_temporary_bytes
    }

    pub(crate) const fn largest_integer_bits(self) -> usize {
        self.largest_integer_bits
    }

    pub(crate) const fn integer_bit_work(self) -> usize {
        self.integer_bit_work
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ResidualAffineIntegerMapPointError {
    ArityMismatch {
        expected: usize,
        actual: usize,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    MapInvariantFailure(&'static str),
    SymbolicaPanic,
}

impl fmt::Display for ResidualAffineIntegerMapPointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArityMismatch { expected, actual } => write!(
                formatter,
                "integer-affine map point has arity {actual}, expected {expected}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "integer-affine map point {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => write!(
                formatter,
                "integer-affine map point {resource} count overflowed usize"
            ),
            Self::MapInvariantFailure(message) => {
                write!(formatter, "integer-affine map invariant failed: {message}")
            }
            Self::SymbolicaPanic => {
                formatter.write_str("Symbolica panicked during integer-affine map point membership")
            }
        }
    }
}

impl std::error::Error for ResidualAffineIntegerMapPointError {}

/// Ambient-square affine projection onto a supported integer solution locus.
///
/// The retained convention is
///
/// ```text
/// F(n) = b + A*n,
/// ```
///
/// where `b` has `ambient_arity` entries and `A` is an
/// `ambient_arity`-by-`ambient_arity` row-major matrix.  Thus
/// `linear_coefficient(row, column)` returns `A[row, column]`.  Rows indexed by
/// [`Self::free_positions`] are identity rows with zero translation.  Rows
/// indexed by [`Self::pivot_positions`] are their solved affine expressions.
/// The map is a projection (`A^2 = A` and `A*b = 0`), so applying it twice has
/// the same result as applying it once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResidualAffineIntegerMap {
    ambient_arity: usize,
    constants: Vec<Integer>,
    linear_coefficients: Vec<Integer>,
    pivot_positions: Vec<usize>,
    free_positions: Vec<usize>,
}

impl ResidualAffineIntegerMap {
    /// Number of original index coordinates in both the domain and codomain.
    pub const fn ambient_arity(&self) -> usize {
        self.ambient_arity
    }

    /// Returns translation entry `b[position]` in `F(n) = b + A*n`.
    pub fn constant(&self, position: usize) -> Option<&Integer> {
        self.constants.get(position)
    }

    /// Returns row-major matrix entry `A[row, column]` in `F(n) = b + A*n`.
    ///
    /// Both indices are original ambient-coordinate positions.  This is a
    /// square ambient map, not a compact matrix whose columns are numbered by
    /// free-parameter ordinal.
    pub fn linear_coefficient(&self, row: usize, column: usize) -> Option<&Integer> {
        if row >= self.ambient_arity || column >= self.ambient_arity {
            return None;
        }
        self.linear_coefficients
            .get(row.checked_mul(self.ambient_arity)?.checked_add(column)?)
    }

    /// Original-coordinate pivots in deterministic elimination order.
    ///
    /// This order is a DFS path and is not promised to be numerically sorted.
    pub fn pivot_positions(&self) -> &[usize] {
        &self.pivot_positions
    }

    /// Numerically sorted original-coordinate positions retained as free.
    ///
    /// Each corresponding row of `A` is an identity row and its entry in `b`
    /// is zero.
    pub fn free_positions(&self) -> &[usize] {
        &self.free_positions
    }

    /// Prove exact membership in this map's image at one original-coordinate
    /// integer point by checking `F(n) == n`.
    ///
    /// This is intentionally stronger than branch-level Boolean-terminal
    /// applicability: it evaluates the retained affine projection itself.
    /// The implementation retains no query vector.  It first preflights the
    /// complete matrix and every prospective arbitrary-precision operation,
    /// then evaluates all rows with exact Symbolica integers.  Consequently a
    /// resource failure occurs before any GMP clone, multiply, add, or
    /// comparison belonging to the evaluation phase.
    pub(crate) fn fixes_i64_point_with_limits(
        &self,
        indices: &[i64],
        limits: ResidualAffineIntegerMapPointLimits,
    ) -> Result<(bool, ResidualAffineIntegerMapPointStats), ResidualAffineIntegerMapPointError>
    {
        catch_unwind(AssertUnwindSafe(|| {
            fixes_i64_point_inner(self, indices, limits)
        }))
        .map_err(|_| ResidualAffineIntegerMapPointError::SymbolicaPanic)?
    }
}

fn fixes_i64_point_inner(
    map: &ResidualAffineIntegerMap,
    indices: &[i64],
    limits: ResidualAffineIntegerMapPointLimits,
) -> Result<(bool, ResidualAffineIntegerMapPointStats), ResidualAffineIntegerMapPointError> {
    let stats = preflight_i64_point_membership(map, indices, limits)?;

    // No GMP-producing operation is permitted above this boundary.  The
    // complete prospective census has succeeded, so every temporary created
    // below has a configured bit bound and only a constant number of Integer
    // values is live at once.
    let mut fixed = true;
    for row in 0..map.ambient_arity {
        let mut value = map
            .constants
            .get(row)
            .ok_or(ResidualAffineIntegerMapPointError::MapInvariantFailure(
                "translation length differs from ambient arity",
            ))?
            .clone();
        for (column, &coordinate) in indices.iter().enumerate() {
            let coefficient = map.linear_coefficient(row, column).ok_or(
                ResidualAffineIntegerMapPointError::MapInvariantFailure(
                    "matrix length differs from ambient square",
                ),
            )?;
            if coefficient.is_zero() || coordinate == 0 {
                continue;
            }
            let contribution = coefficient * Integer::from(coordinate);
            value += contribution;
        }
        fixed &= value == Integer::from(indices[row]);
    }
    Ok((fixed, stats))
}

fn preflight_i64_point_membership(
    map: &ResidualAffineIntegerMap,
    indices: &[i64],
    limits: ResidualAffineIntegerMapPointLimits,
) -> Result<ResidualAffineIntegerMapPointStats, ResidualAffineIntegerMapPointError> {
    if indices.len() != map.ambient_arity {
        return Err(ResidualAffineIntegerMapPointError::ArityMismatch {
            expected: map.ambient_arity,
            actual: indices.len(),
        });
    }
    point_check_limit("ambient arity", map.ambient_arity, limits.max_ambient_arity)?;
    let matrix_entries = point_checked_mul(
        "matrix entries inspected",
        map.ambient_arity,
        map.ambient_arity,
    )?;
    let matrix_entries_inspected =
        point_checked_mul("matrix entries inspected", matrix_entries, 2)?;
    point_check_limit(
        "matrix entries inspected",
        matrix_entries_inspected,
        limits.max_matrix_entries_inspected,
    )?;
    if map.constants.len() != map.ambient_arity {
        return Err(ResidualAffineIntegerMapPointError::MapInvariantFailure(
            "translation length differs from ambient arity",
        ));
    }
    if map.linear_coefficients.len() != matrix_entries {
        return Err(ResidualAffineIntegerMapPointError::MapInvariantFailure(
            "matrix length differs from ambient square",
        ));
    }

    let mut stats = ResidualAffineIntegerMapPointStats {
        ambient_arity: map.ambient_arity,
        matrix_entries_inspected,
        ..ResidualAffineIntegerMapPointStats::default()
    };
    for &coordinate in indices {
        point_observe_integer_bits(i64_magnitude_bits(coordinate)?, limits, &mut stats)?;
    }

    for row in 0..map.ambient_arity {
        let constant = &map.constants[row];
        let constant_bits = point_integer_magnitude_bits(constant)?;
        point_observe_integer_bits(constant_bits, limits, &mut stats)?;
        point_charge_integer_bit_work(constant_bits.max(1), limits, &mut stats)?;
        let mut accumulator_bit_bound = constant_bits;

        for (column, &coordinate) in indices.iter().enumerate() {
            let matrix_offset = point_checked_add(
                "matrix offset",
                point_checked_mul("matrix offset", row, map.ambient_arity)?,
                column,
            )?;
            let coefficient = map.linear_coefficients.get(matrix_offset).ok_or(
                ResidualAffineIntegerMapPointError::MapInvariantFailure(
                    "matrix length differs from ambient square",
                ),
            )?;
            let coefficient_bits = point_integer_magnitude_bits(coefficient)?;
            point_observe_integer_bits(coefficient_bits, limits, &mut stats)?;
            if coefficient.is_zero() || coordinate == 0 {
                continue;
            }

            stats.nonzero_multiplications = point_bounded_add(
                "nonzero multiplications",
                stats.nonzero_multiplications,
                1,
                limits.max_nonzero_multiplications,
            )?;
            let coordinate_bits = i64_magnitude_bits(coordinate)?;
            let product_bit_bound =
                point_checked_add("integer bits", coefficient_bits, coordinate_bits)?;
            point_observe_integer_bits(product_bit_bound, limits, &mut stats)?;
            let multiplication_work = point_checked_add(
                "integer bit work",
                point_checked_mul(
                    "integer bit work",
                    coefficient_bits.max(1),
                    coordinate_bits.max(1),
                )?,
                product_bit_bound.max(1),
            )?;
            point_charge_integer_bit_work(multiplication_work, limits, &mut stats)?;

            stats.additions =
                point_bounded_add("additions", stats.additions, 1, limits.max_additions)?;
            let sum_bit_bound = point_checked_add(
                "integer bits",
                accumulator_bit_bound.max(product_bit_bound),
                1,
            )?;
            point_observe_integer_bits(sum_bit_bound, limits, &mut stats)?;
            point_charge_integer_bit_work(sum_bit_bound.max(1), limits, &mut stats)?;
            accumulator_bit_bound = sum_bit_bound;
        }

        stats.fixed_point_comparisons = point_bounded_add(
            "fixed-point comparisons",
            stats.fixed_point_comparisons,
            1,
            limits.max_fixed_point_comparisons,
        )?;
        point_charge_integer_bit_work(
            accumulator_bit_bound
                .max(i64_magnitude_bits(indices[row])?)
                .max(1),
            limits,
            &mut stats,
        )?;
    }
    stats.peak_temporary_bytes = point_temporary_byte_envelope(stats.largest_integer_bits)?;
    point_check_limit(
        "peak temporary bytes",
        stats.peak_temporary_bytes,
        limits.max_peak_temporary_bytes,
    )?;
    Ok(stats)
}

fn point_integer_magnitude_bits(
    value: &Integer,
) -> Result<usize, ResidualAffineIntegerMapPointError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(
        |_| ResidualAffineIntegerMapPointError::ResourceCountOverflow {
            resource: "integer bits",
        },
    )
}

fn i64_magnitude_bits(value: i64) -> Result<usize, ResidualAffineIntegerMapPointError> {
    usize::try_from(u128::from(i64::BITS - value.unsigned_abs().leading_zeros())).map_err(|_| {
        ResidualAffineIntegerMapPointError::ResourceCountOverflow {
            resource: "integer bits",
        }
    })
}

fn point_temporary_byte_envelope(
    largest_integer_bits: usize,
) -> Result<usize, ResidualAffineIntegerMapPointError> {
    let limb_payload_bytes = if largest_integer_bits == 0 {
        0
    } else {
        point_checked_add("peak temporary bytes", largest_integer_bits, 7)?
            .checked_div(8)
            .and_then(|bytes| bytes.checked_add(size_of::<usize>()))
            .ok_or(ResidualAffineIntegerMapPointError::ResourceCountOverflow {
                resource: "peak temporary bytes",
            })?
    };
    let per_integer_bytes = point_checked_add(
        "peak temporary bytes",
        size_of::<Integer>(),
        limb_payload_bytes,
    )?;
    point_checked_mul("peak temporary bytes", per_integer_bytes, 4)
}

fn point_observe_integer_bits(
    bits: usize,
    limits: ResidualAffineIntegerMapPointLimits,
    stats: &mut ResidualAffineIntegerMapPointStats,
) -> Result<(), ResidualAffineIntegerMapPointError> {
    point_check_limit("integer bits", bits, limits.max_integer_bits)?;
    stats.largest_integer_bits = stats.largest_integer_bits.max(bits);
    Ok(())
}

fn point_charge_integer_bit_work(
    work: usize,
    limits: ResidualAffineIntegerMapPointLimits,
    stats: &mut ResidualAffineIntegerMapPointStats,
) -> Result<(), ResidualAffineIntegerMapPointError> {
    stats.integer_bit_work = point_bounded_add(
        "integer bit work",
        stats.integer_bit_work,
        work,
        limits.max_integer_bit_work,
    )?;
    Ok(())
}

fn point_check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ResidualAffineIntegerMapPointError> {
    if requested > limit {
        Err(ResidualAffineIntegerMapPointError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn point_bounded_add(
    resource: &'static str,
    current: usize,
    additional: usize,
    limit: usize,
) -> Result<usize, ResidualAffineIntegerMapPointError> {
    let requested = point_checked_add(resource, current, additional)?;
    point_check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn point_checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ResidualAffineIntegerMapPointError> {
    left.checked_add(right)
        .ok_or(ResidualAffineIntegerMapPointError::ResourceCountOverflow { resource })
}

fn point_checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ResidualAffineIntegerMapPointError> {
    left.checked_mul(right)
        .ok_or(ResidualAffineIntegerMapPointError::ResourceCountOverflow { resource })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualAffineIntegerSystemOutcome {
    AffineMap,
    ProvedEmpty,
}

#[derive(Clone, Debug)]
pub struct ResidualAffineIntegerSystemCertificate {
    schema: &'static str,
    ambient_arity: usize,
    replay_source_rows: Vec<ResidualAffineIntegerSystemInputRow>,
    source_rows: Vec<ResidualAffineIntegerSystemInputRow>,
    operations: Vec<ResidualAffineIntegerRowOperation>,
    final_rows: Vec<ResidualAffineIntegerFinalRow>,
    pivot_positions: Vec<usize>,
    free_positions: Vec<usize>,
    affine_map: Option<ResidualAffineIntegerMap>,
    empty_witness: Option<ResidualAffineIntegerEmptyWitness>,
    limits: ResidualAffineIntegerSystemLimits,
    stats: ResidualAffineIntegerSystemStats,
}

/// Unforgeable proof that the exact owned certificate allocation was produced
/// by the immediately preceding integer-system compilation.
///
/// This V2-only value is deliberately crate-private and non-`Clone`. Its
/// constructor and owner are private to this module, so an old or independently
/// allocated equal V1 certificate cannot be relabelled as fresh.
pub(crate) struct ResidualAffineIntegerSystemFreshCompilation {
    certificate: Arc<ResidualAffineIntegerSystemCertificate>,
    retained_owned_logical_bytes_upper_bound: usize,
    compilation_owned_logical_peak_upper_bound: usize,
    raw_transient_census: ResidualAffineIntegerSystemRawTransientCensus,
    payload_comparison_census: ResidualAffineIntegerSystemPayloadComparisonCensus,
}

impl ResidualAffineIntegerSystemFreshCompilation {
    pub(crate) const fn retained_owned_logical_bytes_upper_bound(&self) -> usize {
        self.retained_owned_logical_bytes_upper_bound
    }

    pub(crate) const fn compilation_owned_logical_peak_upper_bound(&self) -> usize {
        self.compilation_owned_logical_peak_upper_bound
    }

    pub(crate) const fn raw_transient_census(
        &self,
    ) -> ResidualAffineIntegerSystemRawTransientCensus {
        self.raw_transient_census
    }

    pub(crate) const fn payload_comparison_census(
        &self,
    ) -> ResidualAffineIntegerSystemPayloadComparisonCensus {
        self.payload_comparison_census
    }

    /// Consume the unique fresh result into the Arc retained by the future
    /// branch and the one non-clone authorization consumed later by guard-plan
    /// construction. Both handles point at the exact same allocation.
    pub(crate) fn into_certificate_and_plan_authorization(
        self,
    ) -> Result<
        (
            Arc<ResidualAffineIntegerSystemCertificate>,
            ResidualAffineIntegerSystemFreshPlanAuthorization,
        ),
        ResidualAffineIntegerSystemError,
    > {
        self.authenticate_adjacent_census()?;
        let retained_certificate = Arc::clone(&self.certificate);
        let authorization = ResidualAffineIntegerSystemFreshPlanAuthorization {
            certificate: self.certificate,
            retained_owned_logical_bytes_upper_bound: self.retained_owned_logical_bytes_upper_bound,
            compilation_owned_logical_peak_upper_bound: self
                .compilation_owned_logical_peak_upper_bound,
            raw_transient_census: self.raw_transient_census,
            payload_comparison_census: self.payload_comparison_census,
        };
        if !authorization.authenticates_certificate_allocation(&retained_certificate) {
            return Err(ResidualAffineIntegerSystemError::ReplayMismatch);
        }
        Ok((retained_certificate, authorization))
    }

    fn authenticate_adjacent_census(&self) -> Result<(), ResidualAffineIntegerSystemError> {
        authenticate_fresh_integer_system_adjacent_census(
            &self.certificate,
            self.retained_owned_logical_bytes_upper_bound,
            self.compilation_owned_logical_peak_upper_bound,
            self.raw_transient_census,
            self.payload_comparison_census,
        )
    }

    #[cfg(test)]
    pub(crate) fn tamper_retained_census_for_test(&mut self) {
        self.retained_owned_logical_bytes_upper_bound = self
            .retained_owned_logical_bytes_upper_bound
            .saturating_add(1);
    }

    #[cfg(test)]
    pub(crate) fn tamper_peak_census_for_test(&mut self) {
        self.compilation_owned_logical_peak_upper_bound = self
            .compilation_owned_logical_peak_upper_bound
            .saturating_add(1);
    }

    #[cfg(test)]
    pub(crate) fn tamper_transient_census_for_test(&mut self) {
        self.raw_transient_census.allocation_entries_reserved = self
            .raw_transient_census
            .allocation_entries_reserved
            .saturating_add(1);
    }

    #[cfg(test)]
    pub(crate) fn tamper_payload_units_for_test(&mut self) {
        self.payload_comparison_census.units =
            self.payload_comparison_census.units.saturating_add(1);
    }

    #[cfg(test)]
    pub(crate) fn tamper_payload_bytes_for_test(&mut self) {
        self.payload_comparison_census.bytes =
            self.payload_comparison_census.bytes.saturating_add(1);
    }

    #[cfg(test)]
    pub(crate) fn tamper_payload_integer_bits_for_test(&mut self) {
        self.payload_comparison_census.integer_bits = self
            .payload_comparison_census
            .integer_bits
            .saturating_add(1);
    }
}

/// Single-consumer authorization for replay-free composition-plan building.
///
/// The future branch retains the sibling Arc returned by the fresh split and
/// carries this non-Clone value opaquely to the sole guard consumer. No raw
/// Arc can be borrowed or cloned out of the authorization.
pub(crate) struct ResidualAffineIntegerSystemFreshPlanAuthorization {
    certificate: Arc<ResidualAffineIntegerSystemCertificate>,
    retained_owned_logical_bytes_upper_bound: usize,
    compilation_owned_logical_peak_upper_bound: usize,
    raw_transient_census: ResidualAffineIntegerSystemRawTransientCensus,
    payload_comparison_census: ResidualAffineIntegerSystemPayloadComparisonCensus,
}

impl ResidualAffineIntegerSystemFreshPlanAuthorization {
    pub(crate) fn authenticates_certificate_allocation(
        &self,
        certificate: &Arc<ResidualAffineIntegerSystemCertificate>,
    ) -> bool {
        Arc::ptr_eq(&self.certificate, certificate)
    }

    pub(crate) fn into_authenticated_certificate_arc(
        self,
    ) -> Result<Arc<ResidualAffineIntegerSystemCertificate>, ResidualAffineIntegerSystemError> {
        authenticate_fresh_integer_system_adjacent_census(
            &self.certificate,
            self.retained_owned_logical_bytes_upper_bound,
            self.compilation_owned_logical_peak_upper_bound,
            self.raw_transient_census,
            self.payload_comparison_census,
        )?;
        Ok(self.certificate)
    }

    #[cfg(test)]
    pub(crate) fn tamper_payload_units_for_test(&mut self) {
        self.payload_comparison_census.units =
            self.payload_comparison_census.units.saturating_add(1);
    }
}

impl fmt::Debug for ResidualAffineIntegerSystemFreshPlanAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResidualAffineIntegerSystemFreshPlanAuthorization")
            .field("private_certificate", &"<redacted>")
            .finish_non_exhaustive()
    }
}

fn authenticate_fresh_integer_system_adjacent_census(
    certificate: &ResidualAffineIntegerSystemCertificate,
    retained_owned_logical_bytes_upper_bound: usize,
    compilation_owned_logical_peak_upper_bound: usize,
    raw_transient_census: ResidualAffineIntegerSystemRawTransientCensus,
    payload_comparison_census: ResidualAffineIntegerSystemPayloadComparisonCensus,
) -> Result<(), ResidualAffineIntegerSystemError> {
    let retained = certificate.recompute_retained_owned_logical_bytes_upper_bound()?;
    let transient = ResidualAffineIntegerSystemRawTransientCensus::from_stats(certificate.stats);
    let peak = integer_system_compilation_owned_logical_peak_upper_bound(transient, retained)?;
    let payload = integer_system_equal_payload_comparison_census(certificate)?;
    if retained != retained_owned_logical_bytes_upper_bound
        || transient != raw_transient_census
        || peak != compilation_owned_logical_peak_upper_bound
        || payload != payload_comparison_census
    {
        return Err(ResidualAffineIntegerSystemError::ReplayMismatch);
    }
    Ok(())
}

impl fmt::Debug for ResidualAffineIntegerSystemFreshCompilation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResidualAffineIntegerSystemFreshCompilation")
            .field("private_certificate", &"<redacted>")
            .field(
                "retained_owned_logical_bytes_upper_bound",
                &self.retained_owned_logical_bytes_upper_bound,
            )
            .field(
                "compilation_owned_logical_peak_upper_bound",
                &self.compilation_owned_logical_peak_upper_bound,
            )
            .finish_non_exhaustive()
    }
}

/// Unsupported fresh attempt with its otherwise-lost transient work census.
/// The record is intentionally non-`Clone` and exposes no source rows.
pub(crate) struct ResidualAffineIntegerSystemFreshUnsupported {
    reason: ResidualAffineIntegerSystemUnsupported,
    raw_transient_census: ResidualAffineIntegerSystemRawTransientCensus,
    compilation_owned_logical_peak_upper_bound: usize,
}

impl ResidualAffineIntegerSystemFreshUnsupported {
    pub(crate) const fn reason(&self) -> ResidualAffineIntegerSystemUnsupported {
        self.reason
    }

    pub(crate) const fn raw_transient_census(
        &self,
    ) -> ResidualAffineIntegerSystemRawTransientCensus {
        self.raw_transient_census
    }

    pub(crate) const fn compilation_owned_logical_peak_upper_bound(&self) -> usize {
        self.compilation_owned_logical_peak_upper_bound
    }
}

pub(crate) enum ResidualAffineIntegerSystemFreshCompilationAttempt {
    Complete(ResidualAffineIntegerSystemFreshCompilation),
    Unsupported(ResidualAffineIntegerSystemFreshUnsupported),
}

struct ResidualAffineIntegerSystemInternalCompilation {
    certificate: ResidualAffineIntegerSystemCertificate,
    raw_transient_census: ResidualAffineIntegerSystemRawTransientCensus,
}

enum ResidualAffineIntegerSystemInternalCompilationAttempt {
    Complete(ResidualAffineIntegerSystemInternalCompilation),
    Unsupported {
        reason: ResidualAffineIntegerSystemUnsupported,
        raw_transient_census: ResidualAffineIntegerSystemRawTransientCensus,
    },
}

impl ResidualAffineIntegerSystemCertificate {
    pub fn compile(
        ambient_arity: usize,
        source_rows: &[ResidualAffineIntegerSystemInputRow],
        limits: ResidualAffineIntegerSystemLimits,
    ) -> Result<Self, ResidualAffineIntegerSystemError> {
        catch_unwind(AssertUnwindSafe(|| {
            compile_inner(ambient_arity, source_rows, limits)
        }))
        .map_err(|_| ResidualAffineIntegerSystemError::SymbolicaPanic)?
    }

    /// V2-only compilation which returns a non-forgeable fresh allocation or
    /// preserves the transient census of a typed unsupported DFS exit.
    ///
    /// The complete limit-derived logical envelope is checked for arithmetic
    /// overflow before either outcome performs proportional construction.
    pub(crate) fn compile_fresh(
        ambient_arity: usize,
        source_rows: &[ResidualAffineIntegerSystemInputRow],
        limits: ResidualAffineIntegerSystemLimits,
    ) -> Result<ResidualAffineIntegerSystemFreshCompilationAttempt, ResidualAffineIntegerSystemError>
    {
        let limit_envelope = residual_affine_integer_system_memory_envelope_from_limits(limits)?;
        let attempt = catch_unwind(AssertUnwindSafe(|| {
            compile_inner_with_census(ambient_arity, source_rows, limits)
        }))
        .map_err(|_| ResidualAffineIntegerSystemError::SymbolicaPanic)??;
        match attempt {
            ResidualAffineIntegerSystemInternalCompilationAttempt::Complete(compiled) => {
                let transient = compiled.raw_transient_census;
                let certificate = Arc::new(compiled.certificate);
                let retained_owned_logical_bytes_upper_bound =
                    certificate.recompute_retained_owned_logical_bytes_upper_bound()?;
                let compilation_owned_logical_peak_upper_bound =
                    integer_system_compilation_owned_logical_peak_upper_bound(
                        transient,
                        retained_owned_logical_bytes_upper_bound,
                    )?;
                let payload_comparison_census =
                    integer_system_equal_payload_comparison_census(&certificate)?;
                if retained_owned_logical_bytes_upper_bound
                    > limit_envelope.retained_owned_logical_bytes_upper_bound()
                    || compilation_owned_logical_peak_upper_bound
                        > limit_envelope.compilation_owned_logical_peak_upper_bound()
                    || transient
                        != ResidualAffineIntegerSystemRawTransientCensus::from_stats(
                            certificate.stats,
                        )
                {
                    return Err(
                        ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                            "fresh integer-system census exceeds its limits-derived envelope",
                        ),
                    );
                }
                let fresh = ResidualAffineIntegerSystemFreshCompilation {
                    certificate,
                    retained_owned_logical_bytes_upper_bound,
                    compilation_owned_logical_peak_upper_bound,
                    raw_transient_census: transient,
                    payload_comparison_census,
                };
                fresh.authenticate_adjacent_census()?;
                Ok(ResidualAffineIntegerSystemFreshCompilationAttempt::Complete(fresh))
            }
            ResidualAffineIntegerSystemInternalCompilationAttempt::Unsupported {
                reason,
                raw_transient_census,
            } => {
                let compilation_owned_logical_peak_upper_bound =
                    integer_system_compilation_owned_logical_peak_upper_bound(
                        raw_transient_census,
                        0,
                    )?;
                if compilation_owned_logical_peak_upper_bound
                    > limit_envelope.compilation_owned_logical_peak_upper_bound()
                {
                    return Err(
                        ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                            "unsupported integer-system peak exceeds its limits-derived envelope",
                        ),
                    );
                }
                Ok(
                    ResidualAffineIntegerSystemFreshCompilationAttempt::Unsupported(
                        ResidualAffineIntegerSystemFreshUnsupported {
                            reason,
                            raw_transient_census,
                            compilation_owned_logical_peak_upper_bound,
                        },
                    ),
                )
            }
        }
    }

    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub const fn ambient_arity(&self) -> usize {
        self.ambient_arity
    }

    /// Canonically sorted and duplicate-merged source rows.
    ///
    /// These are not the supplied-order replay rows. The latter are retained
    /// privately so replay also authenticates the exact caller payload.
    pub fn source_rows(&self) -> &[ResidualAffineIntegerSystemInputRow] {
        &self.source_rows
    }

    pub fn operations(&self) -> &[ResidualAffineIntegerRowOperation] {
        &self.operations
    }

    pub fn final_rows(&self) -> &[ResidualAffineIntegerFinalRow] {
        &self.final_rows
    }

    pub fn pivot_positions(&self) -> &[usize] {
        &self.pivot_positions
    }

    pub fn free_positions(&self) -> &[usize] {
        &self.free_positions
    }

    pub const fn outcome(&self) -> ResidualAffineIntegerSystemOutcome {
        if self.affine_map.is_some() {
            ResidualAffineIntegerSystemOutcome::AffineMap
        } else {
            ResidualAffineIntegerSystemOutcome::ProvedEmpty
        }
    }

    pub const fn affine_map(&self) -> Option<&ResidualAffineIntegerMap> {
        self.affine_map.as_ref()
    }

    pub const fn empty_witness(&self) -> Option<&ResidualAffineIntegerEmptyWitness> {
        self.empty_witness.as_ref()
    }

    pub const fn limits(&self) -> ResidualAffineIntegerSystemLimits {
        self.limits
    }

    pub const fn stats(&self) -> ResidualAffineIntegerSystemStats {
        self.stats
    }

    /// Recompute the complete certificate-owned logical retained bytes from
    /// initialized payload lengths and exact large-integer magnitudes.
    pub(crate) fn recompute_retained_owned_logical_bytes_upper_bound(
        &self,
    ) -> Result<usize, ResidualAffineIntegerSystemError> {
        integer_system_retained_owned_logical_bytes_upper_bound(self)
    }

    pub(crate) fn recompute_payload_comparison_census(
        &self,
    ) -> Result<ResidualAffineIntegerSystemPayloadComparisonCensus, ResidualAffineIntegerSystemError>
    {
        integer_system_equal_payload_comparison_census(self)
    }

    pub fn replay(&self) -> Result<(), ResidualAffineIntegerSystemError> {
        if self.schema != RESIDUAL_AFFINE_INTEGER_SYSTEM_V1_SCHEMA {
            return Err(ResidualAffineIntegerSystemError::SchemaMismatch);
        }
        let replayed = catch_unwind(AssertUnwindSafe(|| {
            compile_inner(self.ambient_arity, &self.replay_source_rows, self.limits)
        }))
        .map_err(|_| ResidualAffineIntegerSystemError::SymbolicaPanic)??;
        // Replay compilation and final payload authentication are separately
        // capped phases under the same retained limits.  Seeding this budget
        // with the compilation stats would make a certificate compiled exactly
        // at a limit intrinsically unreplayable.  A comparison-phase limit is
        // nevertheless public and typed: it propagates from this method.
        if self.payload_eq_checked(&replayed)? {
            Ok(())
        } else {
            Err(ResidualAffineIntegerSystemError::ReplayMismatch)
        }
    }

    /// Fail-closed crate boundary for enclosing certificate comparisons.
    ///
    /// The fallible comparator below still prospectively charges the retained
    /// limits, but this legacy Boolean surface maps comparison-budget
    /// exhaustion to inequality. Public [`Self::replay`] instead propagates the
    /// typed resource error.
    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.payload_eq_checked(other).unwrap_or(false)
    }

    /// Fallible crate boundary for enclosing replay certificates that must
    /// preserve typed comparison-budget exhaustion. The receiver is the
    /// authoritative persisted payload, so its retained limits cap this
    /// directional authentication comparison.
    pub(crate) fn payload_eq_checked(
        &self,
        other: &Self,
    ) -> Result<bool, ResidualAffineIntegerSystemError> {
        let mut comparison_budget = Budget::new(self.limits, self.ambient_arity);
        self.payload_eq_with_budget(other, &mut comparison_budget)
    }

    fn payload_eq_with_budget(
        &self,
        other: &Self,
        budget: &mut Budget,
    ) -> Result<bool, ResidualAffineIntegerSystemError> {
        verify_certificate_payload_equal(self, other, budget)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct WorkRow {
    components: Vec<Integer>,
    lineage: Vec<usize>,
}

#[derive(Clone, Debug)]
struct SearchState {
    rows: Vec<WorkRow>,
    remaining_rows: Vec<usize>,
    pivot_positions: Vec<usize>,
    operations: Vec<ResidualAffineIntegerRowOperation>,
}

enum SearchConclusion {
    Solved(SearchState),
    Empty(SearchState, ResidualAffineIntegerEmptyWitness),
}

struct Budget {
    limits: ResidualAffineIntegerSystemLimits,
    stats: ResidualAffineIntegerSystemStats,
}

impl Budget {
    fn new(limits: ResidualAffineIntegerSystemLimits, ambient_arity: usize) -> Self {
        Self {
            limits,
            stats: ResidualAffineIntegerSystemStats {
                ambient_arity,
                ..ResidualAffineIntegerSystemStats::default()
            },
        }
    }

    fn add_counter(
        counter: &mut usize,
        amount: usize,
        resource: &'static str,
        limit: usize,
    ) -> Result<(), ResidualAffineIntegerSystemError> {
        let requested = checked_add(resource, *counter, amount)?;
        check_limit(resource, requested, limit)?;
        *counter = requested;
        Ok(())
    }

    fn preflight_counter(
        current: usize,
        amount: usize,
        resource: &'static str,
        limit: usize,
    ) -> Result<(), ResidualAffineIntegerSystemError> {
        check_limit(resource, checked_add(resource, current, amount)?, limit)
    }

    fn prework(&mut self, amount: usize) -> Result<(), ResidualAffineIntegerSystemError> {
        Self::add_counter(
            &mut self.stats.prework_operations,
            amount,
            "prework operations",
            self.limits.max_prework_operations,
        )
    }

    fn canonical_comparison(&mut self) -> Result<(), ResidualAffineIntegerSystemError> {
        Self::add_counter(
            &mut self.stats.canonical_comparisons,
            1,
            "canonical comparisons",
            self.limits.max_canonical_comparisons,
        )
    }

    fn reserve(
        &mut self,
        resource: &'static str,
        amount: usize,
    ) -> Result<(), ResidualAffineIntegerSystemError> {
        Self::add_counter(
            &mut self.stats.allocation_entries_reserved,
            amount,
            "allocation entries reserved",
            self.limits.max_allocation_entries_reserved,
        )?;
        // Keep the caller-specific label available on actual allocation
        // failure; the logical census above deliberately has one common cap.
        let _ = resource;
        Ok(())
    }

    fn lineage_operation(&mut self, amount: usize) -> Result<(), ResidualAffineIntegerSystemError> {
        Self::add_counter(
            &mut self.stats.lineage_operations,
            amount,
            "lineage operations",
            self.limits.max_lineage_operations,
        )
    }

    fn lineage_materialized(
        &mut self,
        amount: usize,
    ) -> Result<(), ResidualAffineIntegerSystemError> {
        Self::add_counter(
            &mut self.stats.lineage_entries_materialized,
            amount,
            "lineage entries materialized",
            self.limits.max_lineage_entries_materialized,
        )
    }

    fn state_materialized(
        &mut self,
        amount: usize,
    ) -> Result<(), ResidualAffineIntegerSystemError> {
        Self::add_counter(
            &mut self.stats.state_entries_materialized,
            amount,
            "state entries materialized",
            self.limits.max_state_entries_materialized,
        )
    }

    fn search(&mut self, amount: usize) -> Result<(), ResidualAffineIntegerSystemError> {
        Self::add_counter(
            &mut self.stats.search_operations,
            amount,
            "search operations",
            self.limits.max_search_operations,
        )
    }

    fn euclidean_step(&mut self) -> Result<(), ResidualAffineIntegerSystemError> {
        Self::add_counter(
            &mut self.stats.euclidean_steps,
            1,
            "Euclidean steps",
            self.limits.max_euclidean_steps,
        )
    }

    fn row_operation(&mut self) -> Result<(), ResidualAffineIntegerSystemError> {
        Self::add_counter(
            &mut self.stats.row_operations,
            1,
            "row operations",
            self.limits.max_row_operations,
        )
    }

    fn operation_integers(
        &mut self,
        amount: usize,
    ) -> Result<(), ResidualAffineIntegerSystemError> {
        Self::add_counter(
            &mut self.stats.operation_integer_entries,
            amount,
            "operation integer entries",
            self.limits.max_operation_integer_entries,
        )
    }

    fn verify(&mut self, amount: usize) -> Result<(), ResidualAffineIntegerSystemError> {
        Self::add_counter(
            &mut self.stats.verification_operations,
            amount,
            "verification operations",
            self.limits.max_verification_operations,
        )
    }

    fn observe_integer(&mut self, value: &Integer) -> Result<(), ResidualAffineIntegerSystemError> {
        let bits = integer_magnitude_bits(value)?;
        self.charge_integer_bit_work(bits.max(1))?;
        self.validate_integer(value)
    }

    fn validate_integer(
        &mut self,
        value: &Integer,
    ) -> Result<(), ResidualAffineIntegerSystemError> {
        let bits = integer_magnitude_bits(value)?;
        check_limit(
            "integer coefficient bits",
            bits,
            self.limits.max_integer_coefficient_bits,
        )?;
        self.stats.largest_integer_coefficient_bits =
            self.stats.largest_integer_coefficient_bits.max(bits);
        Ok(())
    }

    fn charge_integer_bit_work(
        &mut self,
        amount: usize,
    ) -> Result<(), ResidualAffineIntegerSystemError> {
        Self::add_counter(
            &mut self.stats.integer_bit_work,
            amount,
            "integer bit work",
            self.limits.max_integer_bit_work,
        )
    }

    fn preflight_addition(
        &self,
        left: &Integer,
        right: &Integer,
    ) -> Result<usize, ResidualAffineIntegerSystemError> {
        if left.is_zero() || right.is_zero() {
            return Ok(integer_magnitude_bits(if left.is_zero() {
                right
            } else {
                left
            })?);
        }
        let requested = checked_add(
            "integer coefficient bits",
            integer_magnitude_bits(left)?.max(integer_magnitude_bits(right)?),
            1,
        )?;
        check_limit(
            "integer coefficient bits",
            requested,
            self.limits.max_integer_coefficient_bits,
        )?;
        Ok(requested)
    }

    fn preflight_multiplication(
        &self,
        left: &Integer,
        right: &Integer,
    ) -> Result<usize, ResidualAffineIntegerSystemError> {
        if left.is_zero() || right.is_zero() {
            return Ok(0);
        }
        let left_unit = left == &Integer::one() || left == &Integer::from(-1);
        let right_unit = right == &Integer::one() || right == &Integer::from(-1);
        let requested = if left_unit {
            integer_magnitude_bits(right)?
        } else if right_unit {
            integer_magnitude_bits(left)?
        } else {
            checked_add(
                "integer coefficient bits",
                integer_magnitude_bits(left)?,
                integer_magnitude_bits(right)?,
            )?
        };
        check_limit(
            "integer coefficient bits",
            requested,
            self.limits.max_integer_coefficient_bits,
        )?;
        Ok(requested)
    }

    fn add_integer(
        &mut self,
        left: &Integer,
        right: &Integer,
    ) -> Result<Integer, ResidualAffineIntegerSystemError> {
        let output_bits = self.preflight_addition(left, right)?;
        self.charge_integer_bit_work(output_bits.max(1))?;
        let result = left + right;
        self.validate_integer(&result)?;
        Ok(result)
    }

    fn subtract_integer(
        &mut self,
        left: &Integer,
        right: &Integer,
    ) -> Result<Integer, ResidualAffineIntegerSystemError> {
        let output_bits = self.preflight_addition(left, right)?;
        self.charge_integer_bit_work(output_bits.max(1))?;
        let result = left - right;
        self.validate_integer(&result)?;
        Ok(result)
    }

    fn multiply_integer(
        &mut self,
        left: &Integer,
        right: &Integer,
    ) -> Result<Integer, ResidualAffineIntegerSystemError> {
        let output_bits = self.preflight_multiplication(left, right)?;
        let operand_work = checked_mul(
            "integer bit work",
            integer_magnitude_bits(left)?.max(1),
            integer_magnitude_bits(right)?.max(1),
        )?;
        self.charge_integer_bit_work(checked_add(
            "integer bit work",
            operand_work,
            output_bits.max(1),
        )?)?;
        let result = left * right;
        self.validate_integer(&result)?;
        Ok(result)
    }

    fn negate_integer(
        &mut self,
        value: &Integer,
    ) -> Result<Integer, ResidualAffineIntegerSystemError> {
        self.charge_integer_bit_work(integer_magnitude_bits(value)?.max(1))?;
        let result = -value;
        self.validate_integer(&result)?;
        Ok(result)
    }

    fn clone_integer(
        &mut self,
        value: &Integer,
    ) -> Result<Integer, ResidualAffineIntegerSystemError> {
        self.charge_integer_bit_work(integer_magnitude_bits(value)?.max(1))?;
        let result = value.clone();
        self.validate_integer(&result)?;
        Ok(result)
    }

    fn quotient_remainder(
        &mut self,
        numerator: &Integer,
        denominator: &Integer,
    ) -> Result<(Integer, Integer), ResidualAffineIntegerSystemError> {
        if denominator.is_zero() {
            return Err(
                ResidualAffineIntegerSystemError::ArithmeticInvariantFailure("division by zero"),
            );
        }
        let numerator_bits = integer_magnitude_bits(numerator)?.max(1);
        let denominator_bits = integer_magnitude_bits(denominator)?.max(1);
        let division_work = checked_mul("integer bit work", numerator_bits, denominator_bits)?;
        let result_work = checked_add("integer bit work", numerator_bits, denominator_bits)?;
        self.charge_integer_bit_work(checked_add("integer bit work", division_work, result_work)?)?;
        self.euclidean_step()?;
        let (quotient, remainder) = numerator.quot_rem(denominator);
        self.validate_integer(&quotient)?;
        self.validate_integer(&remainder)?;
        Ok((quotient, remainder))
    }
}

fn compile_inner(
    ambient_arity: usize,
    source_rows: &[ResidualAffineIntegerSystemInputRow],
    limits: ResidualAffineIntegerSystemLimits,
) -> Result<ResidualAffineIntegerSystemCertificate, ResidualAffineIntegerSystemError> {
    match compile_inner_with_census(ambient_arity, source_rows, limits)? {
        ResidualAffineIntegerSystemInternalCompilationAttempt::Complete(compiled) => {
            Ok(compiled.certificate)
        }
        ResidualAffineIntegerSystemInternalCompilationAttempt::Unsupported { reason, .. } => {
            Err(ResidualAffineIntegerSystemError::Unsupported(reason))
        }
    }
}

fn compile_inner_with_census(
    ambient_arity: usize,
    source_rows: &[ResidualAffineIntegerSystemInputRow],
    limits: ResidualAffineIntegerSystemLimits,
) -> Result<ResidualAffineIntegerSystemInternalCompilationAttempt, ResidualAffineIntegerSystemError>
{
    check_limit("ambient arity", ambient_arity, limits.max_ambient_arity)?;
    check_limit("input rows", source_rows.len(), limits.max_input_rows)?;

    let mut budget = Budget::new(limits, ambient_arity);
    budget.stats.input_rows = source_rows.len();
    let component_width = checked_add("input components", ambient_arity, 1)?;
    let input_components = checked_mul("input components", source_rows.len(), component_width)?;
    check_limit(
        "input components",
        input_components,
        limits.max_input_components,
    )?;
    budget.stats.input_components = input_components;

    let mut input_lineage_ordinals = 0usize;
    for (row_ordinal, source) in source_rows.iter().enumerate() {
        budget.prework(1)?;
        if source.row().arity() != ambient_arity {
            return Err(ResidualAffineIntegerSystemError::ArityMismatch {
                row_ordinal,
                expected: ambient_arity,
                actual: source.row().arity(),
            });
        }
        input_lineage_ordinals = checked_add(
            "input lineage ordinals",
            input_lineage_ordinals,
            source.structural_locus_ordinals().len(),
        )?;
        check_limit(
            "input lineage ordinals",
            input_lineage_ordinals,
            limits.max_input_lineage_ordinals,
        )?;
        if source.structural_locus_ordinals().is_empty()
            || !is_strictly_increasing(source.structural_locus_ordinals())
        {
            return Err(
                ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                    "input lineage is not canonical",
                ),
            );
        }
        for component in source.row().components() {
            budget.prework(1)?;
            budget.observe_integer(component)?;
        }
    }
    budget.stats.input_lineage_ordinals = input_lineage_ordinals;

    let replay_source_rows = clone_source_rows_preserving_order(source_rows, &mut budget)?;
    let canonical = canonicalize_source_rows(&replay_source_rows, &mut budget)?;
    budget.stats.canonical_rows = canonical.len();
    check_limit("canonical rows", canonical.len(), limits.max_canonical_rows)?;

    // The initial search state is itself the first live frontier state.  Reject
    // a strict-zero frontier bound before reserving or materializing any of its
    // work-row payload.
    check_limit("frontier states", 1, limits.max_frontier_states)?;
    let initial = initial_search_state(&canonical, &mut budget)?;
    let conclusion = match search_integer_affine_system(initial, ambient_arity, &mut budget) {
        Ok(conclusion) => conclusion,
        Err(ResidualAffineIntegerSystemError::Unsupported(reason)) => {
            return Ok(
                ResidualAffineIntegerSystemInternalCompilationAttempt::Unsupported {
                    reason,
                    raw_transient_census: ResidualAffineIntegerSystemRawTransientCensus::from_stats(
                        budget.stats,
                    ),
                },
            );
        }
        Err(error) => return Err(error),
    };

    let (state, empty_witness) = match conclusion {
        SearchConclusion::Solved(state) => (state, None),
        SearchConclusion::Empty(state, witness) => (state, Some(witness)),
    };
    let final_rows = copy_final_rows(&state.rows, &mut budget)?;
    let free_positions = complement_positions(ambient_arity, &state.pivot_positions, &mut budget)?;

    let affine_map = if empty_witness.is_none() {
        Some(build_affine_map(
            ambient_arity,
            &state,
            &free_positions,
            &mut budget,
        )?)
    } else {
        None
    };

    verify_transcript(&canonical, &state, &mut budget)?;
    if let Some(map) = &affine_map {
        verify_affine_map(&replay_source_rows, map, &mut budget)?;
    } else {
        verify_empty_witness(
            empty_witness.as_ref().ok_or(
                ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                    "missing empty witness",
                ),
            )?,
            &state.rows,
            &mut budget,
        )?;
    }

    budget.stats.rank = state.pivot_positions.len();
    budget.stats.free_positions = free_positions.len();
    let stats = budget.stats;
    Ok(
        ResidualAffineIntegerSystemInternalCompilationAttempt::Complete(
            ResidualAffineIntegerSystemInternalCompilation {
                certificate: ResidualAffineIntegerSystemCertificate {
                    schema: RESIDUAL_AFFINE_INTEGER_SYSTEM_V1_SCHEMA,
                    ambient_arity,
                    replay_source_rows,
                    source_rows: canonical,
                    operations: state.operations,
                    final_rows,
                    pivot_positions: state.pivot_positions,
                    free_positions,
                    affine_map,
                    empty_witness,
                    limits,
                    stats,
                },
                raw_transient_census: ResidualAffineIntegerSystemRawTransientCensus::from_stats(
                    stats,
                ),
            },
        ),
    )
}

fn clone_source_rows_preserving_order(
    source_rows: &[ResidualAffineIntegerSystemInputRow],
    budget: &mut Budget,
) -> Result<Vec<ResidualAffineIntegerSystemInputRow>, ResidualAffineIntegerSystemError> {
    let components = source_rows.iter().try_fold(0usize, |total, row| {
        checked_add(
            "replay source components",
            total,
            row.row().components().len(),
        )
    })?;
    let lineages = source_rows.iter().try_fold(0usize, |total, row| {
        checked_add(
            "replay source lineages",
            total,
            row.structural_locus_ordinals().len(),
        )
    })?;
    let allocation = checked_add(
        "replay source allocation entries",
        source_rows.len(),
        checked_add("replay source allocation entries", components, lineages)?,
    )?;
    Budget::preflight_counter(
        budget.stats.allocation_entries_reserved,
        allocation,
        "allocation entries reserved",
        budget.limits.max_allocation_entries_reserved,
    )?;
    Budget::preflight_counter(
        budget.stats.state_entries_materialized,
        checked_add(
            "replay source state entries",
            source_rows.len(),
            checked_add("replay source state entries", components, lineages)?,
        )?,
        "state entries materialized",
        budget.limits.max_state_entries_materialized,
    )?;
    Budget::preflight_counter(
        budget.stats.lineage_entries_materialized,
        lineages,
        "lineage entries materialized",
        budget.limits.max_lineage_entries_materialized,
    )?;
    let integer_bit_work = source_rows.iter().try_fold(0usize, |total, row| {
        checked_add(
            "integer bit work",
            total,
            checked_add(
                "integer bit work",
                integer_clone_bit_work(row.row().components())?,
                primitive_row_validation_bit_work(row.row().components())?,
            )?,
        )
    })?;
    Budget::preflight_counter(
        budget.stats.integer_bit_work,
        integer_bit_work,
        "integer bit work",
        budget.limits.max_integer_bit_work,
    )?;
    budget.reserve("replay source rows", source_rows.len())?;
    let mut retained = Vec::new();
    retained.try_reserve_exact(source_rows.len()).map_err(|_| {
        ResidualAffineIntegerSystemError::AllocationFailure {
            resource: "replay source rows",
        }
    })?;
    for row in source_rows {
        let components = clone_integer_slice(row.row().components(), budget, true)?;
        precharge_primitive_row_validation(&components, budget)?;
        let retained_row = ResidualAffineIntegerSystemInputRow {
            row: ResidualAffinePrimitiveRow::try_from_canonical_components_with_limits(
                components,
                row.row().components().len(),
                budget.limits.max_integer_coefficient_bits,
                budget.limits.max_integer_bit_work,
            )
            .map_err(|_| {
                ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                    "replay source primitive row did not revalidate",
                )
            })?,
            structural_locus_ordinals: clone_lineage(row.structural_locus_ordinals(), budget)?,
        };
        budget.state_materialized(1)?;
        retained.push(retained_row);
    }
    Ok(retained)
}

fn canonicalize_source_rows(
    source_rows: &[ResidualAffineIntegerSystemInputRow],
    budget: &mut Budget,
) -> Result<Vec<ResidualAffineIntegerSystemInputRow>, ResidualAffineIntegerSystemError> {
    let component_entries = source_rows.iter().try_fold(0usize, |total, source| {
        checked_add(
            "canonical source component entries",
            total,
            source.row().components().len(),
        )
    })?;
    let lineage_entries = source_rows.iter().try_fold(0usize, |total, source| {
        checked_add(
            "canonical source lineage entries",
            total,
            source.structural_locus_ordinals().len(),
        )
    })?;
    let allocation_entries = checked_add(
        "canonical source allocation entries",
        source_rows.len(),
        checked_add(
            "canonical source allocation entries",
            component_entries,
            lineage_entries,
        )?,
    )?;
    Budget::preflight_counter(
        budget.stats.allocation_entries_reserved,
        allocation_entries,
        "allocation entries reserved",
        budget.limits.max_allocation_entries_reserved,
    )?;
    Budget::preflight_counter(
        budget.stats.state_entries_materialized,
        checked_add(
            "canonical source state entries",
            source_rows.len(),
            checked_add(
                "canonical source state entries",
                component_entries,
                lineage_entries,
            )?,
        )?,
        "state entries materialized",
        budget.limits.max_state_entries_materialized,
    )?;
    Budget::preflight_counter(
        budget.stats.lineage_entries_materialized,
        lineage_entries,
        "lineage entries materialized",
        budget.limits.max_lineage_entries_materialized,
    )?;
    let integer_bit_work = source_rows.iter().try_fold(0usize, |total, source| {
        checked_add(
            "integer bit work",
            total,
            checked_add(
                "integer bit work",
                integer_clone_bit_work(source.row().components())?,
                primitive_row_validation_bit_work(source.row().components())?,
            )?,
        )
    })?;
    Budget::preflight_counter(
        budget.stats.integer_bit_work,
        integer_bit_work,
        "integer bit work",
        budget.limits.max_integer_bit_work,
    )?;
    budget.reserve("canonical source rows", source_rows.len())?;
    let mut sorted = Vec::new();
    sorted.try_reserve_exact(source_rows.len()).map_err(|_| {
        ResidualAffineIntegerSystemError::AllocationFailure {
            resource: "canonical source rows",
        }
    })?;
    for source in source_rows {
        let components = clone_integer_slice(source.row().components(), budget, true)?;
        precharge_primitive_row_validation(&components, budget)?;
        let row = ResidualAffinePrimitiveRow::try_from_canonical_components_with_limits(
            components,
            source.row().components().len(),
            budget.limits.max_integer_coefficient_bits,
            budget.limits.max_integer_bit_work,
        )
        .map_err(|_| {
            ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                "canonical source primitive row did not revalidate",
            )
        })?;
        let lineage = clone_lineage(source.structural_locus_ordinals(), budget)?;
        budget.state_materialized(1)?;
        sorted.push(ResidualAffineIntegerSystemInputRow {
            row,
            structural_locus_ordinals: lineage,
        });
    }

    // Insertion sort gives an exact comparison census and has no hidden
    // temporary allocation.  This path is intended for the small affine atom
    // sets of one Boolean terminal, and is bounded independently of row work.
    for right in 1..sorted.len() {
        let mut cursor = right;
        while cursor > 0 {
            let ordering =
                canonical_primitive_row_cmp(&sorted[cursor - 1].row, &sorted[cursor].row, budget)?;
            if ordering != Ordering::Greater {
                break;
            }
            sorted.swap(cursor - 1, cursor);
            cursor -= 1;
        }
    }

    budget.reserve("deduplicated source rows", sorted.len())?;
    let mut canonical: Vec<ResidualAffineIntegerSystemInputRow> = Vec::new();
    canonical.try_reserve_exact(sorted.len()).map_err(|_| {
        ResidualAffineIntegerSystemError::AllocationFailure {
            resource: "deduplicated source rows",
        }
    })?;
    for source in sorted {
        let is_duplicate = if let Some(previous) = canonical.last() {
            canonical_primitive_row_cmp(&previous.row, &source.row, budget)? == Ordering::Equal
        } else {
            false
        };
        if is_duplicate {
            let previous = canonical.last_mut().ok_or(
                ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                    "missing duplicate destination",
                ),
            )?;
            previous.structural_locus_ordinals = merge_lineage(
                &previous.structural_locus_ordinals,
                &source.structural_locus_ordinals,
                budget,
            )?;
        } else {
            canonical.push(source);
        }
    }
    Ok(canonical)
}

/// Fallible lexicographic comparison matching `ResidualAffinePrimitiveRow`'s
/// derived ordering while charging both the structural canonical-comparison
/// census and every arbitrary-precision component comparison prospectively.
fn canonical_primitive_row_cmp(
    left: &ResidualAffinePrimitiveRow,
    right: &ResidualAffinePrimitiveRow,
    budget: &mut Budget,
) -> Result<Ordering, ResidualAffineIntegerSystemError> {
    budget.canonical_comparison()?;
    if std::ptr::eq(left, right) {
        return Ok(Ordering::Equal);
    }
    for (left_component, right_component) in left.components().iter().zip(right.components()) {
        let ordering = charged_integer_cmp(left_component, right_component, budget)?;
        if ordering != Ordering::Equal {
            return Ok(ordering);
        }
    }
    Ok(left.components().len().cmp(&right.components().len()))
}

fn initial_search_state(
    canonical: &[ResidualAffineIntegerSystemInputRow],
    budget: &mut Budget,
) -> Result<SearchState, ResidualAffineIntegerSystemError> {
    budget.reserve("initial work rows", canonical.len())?;
    budget.reserve("initial remaining rows", canonical.len())?;
    let mut rows = Vec::new();
    let mut remaining_rows = Vec::new();
    rows.try_reserve_exact(canonical.len()).map_err(|_| {
        ResidualAffineIntegerSystemError::AllocationFailure {
            resource: "initial work rows",
        }
    })?;
    remaining_rows
        .try_reserve_exact(canonical.len())
        .map_err(|_| ResidualAffineIntegerSystemError::AllocationFailure {
            resource: "initial remaining rows",
        })?;
    for (ordinal, source) in canonical.iter().enumerate() {
        let components = clone_integer_slice(source.row().components(), budget, true)?;
        let lineage = clone_lineage(source.structural_locus_ordinals(), budget)?;
        budget.state_materialized(1)?;
        rows.push(WorkRow {
            components,
            lineage,
        });
        remaining_rows.push(ordinal);
    }
    Ok(SearchState {
        rows,
        remaining_rows,
        pivot_positions: Vec::new(),
        operations: Vec::new(),
    })
}

fn search_integer_affine_system(
    initial: SearchState,
    ambient_arity: usize,
    budget: &mut Budget,
) -> Result<SearchConclusion, ResidualAffineIntegerSystemError> {
    // `compile_inner` performs this check before constructing `initial`; keep
    // the local preflight too so this function can never materialize an
    // over-limit frontier if its call boundary changes later.
    check_limit("frontier states", 1, budget.limits.max_frontier_states)?;
    budget.reserve("DFS frontier", 1)?;
    let mut frontier = Vec::new();
    frontier.try_reserve_exact(1).map_err(|_| {
        ResidualAffineIntegerSystemError::AllocationFailure {
            resource: "DFS frontier",
        }
    })?;
    frontier.push(initial);
    budget.stats.frontier_states_peak = 1;
    let mut first_unsupported_remaining = None;

    while let Some(mut state) = frontier.pop() {
        Budget::add_counter(
            &mut budget.stats.dfs_states,
            1,
            "DFS states",
            budget.limits.max_dfs_states,
        )?;
        let depth = state.pivot_positions.len();
        check_limit("DFS depth", depth, budget.limits.max_dfs_depth)?;
        budget.stats.deepest_dfs_depth = budget.stats.deepest_dfs_depth.max(depth);

        let active = match active_rows_or_empty_witness(&state, budget)? {
            ActiveRows::Rows(active) => active,
            ActiveRows::Empty(witness) => {
                return Ok(SearchConclusion::Empty(state, witness));
            }
        };
        state.remaining_rows = active;
        if state.remaining_rows.is_empty() {
            return Ok(SearchConclusion::Solved(state));
        }

        let candidates = eligible_columns(&state, ambient_arity, budget)?;
        if candidates.is_empty() {
            first_unsupported_remaining.get_or_insert(state.remaining_rows.len());
            continue;
        }

        // Push in reverse so the explicit LIFO stack visits original columns
        // in increasing order.  Each sibling owns an independently bounded
        // exact state; the first complete unit-pivot path is returned.
        for &column in candidates.iter().rev() {
            let requested_depth = checked_add("DFS depth", depth, 1)?;
            check_limit("DFS depth", requested_depth, budget.limits.max_dfs_depth)?;
            let requested_frontier = checked_add("frontier states", frontier.len(), 1)?;
            check_limit(
                "frontier states",
                requested_frontier,
                budget.limits.max_frontier_states,
            )?;
            budget.reserve("DFS frontier growth", 1)?;
            frontier.try_reserve_exact(1).map_err(|_| {
                ResidualAffineIntegerSystemError::AllocationFailure {
                    resource: "DFS frontier growth",
                }
            })?;
            let mut child = try_clone_search_state(&state, budget)?;
            apply_unit_pivot(&mut child, column, budget)?;
            frontier.push(child);
            budget.stats.frontier_states_peak =
                budget.stats.frontier_states_peak.max(requested_frontier);
        }
    }

    Err(ResidualAffineIntegerSystemError::Unsupported(
        ResidualAffineIntegerSystemUnsupported::GeneralCongruenceCaseNotSupported {
            remaining_equations: first_unsupported_remaining.unwrap_or(0),
        },
    ))
}

enum ActiveRows {
    Rows(Vec<usize>),
    Empty(ResidualAffineIntegerEmptyWitness),
}

fn active_rows_or_empty_witness(
    state: &SearchState,
    budget: &mut Budget,
) -> Result<ActiveRows, ResidualAffineIntegerSystemError> {
    budget.reserve("active row indices", state.remaining_rows.len())?;
    let mut active = Vec::new();
    active
        .try_reserve_exact(state.remaining_rows.len())
        .map_err(|_| ResidualAffineIntegerSystemError::AllocationFailure {
            resource: "active row indices",
        })?;
    for &row_ordinal in &state.remaining_rows {
        budget.search(1)?;
        let row = state.rows.get(row_ordinal).ok_or(
            ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                "remaining row index is out of range",
            ),
        )?;
        let mut coefficient_gcd = Integer::zero();
        for coefficient in &row.components[1..] {
            budget.search(1)?;
            coefficient_gcd = bounded_gcd(&coefficient_gcd, coefficient, budget)?;
        }
        if coefficient_gcd.is_zero() {
            if row.components[0].is_zero() {
                continue;
            }
            return Ok(ActiveRows::Empty(
                ResidualAffineIntegerEmptyWitness::ZeroEqualsNonzero {
                    row: row_ordinal,
                    constant: budget.clone_integer(&row.components[0])?,
                    structural_locus_ordinals: clone_lineage(&row.lineage, budget)?,
                },
            ));
        }
        let (_, remainder) = budget.quotient_remainder(&row.components[0], &coefficient_gcd)?;
        if !remainder.is_zero() {
            return Ok(ActiveRows::Empty(
                ResidualAffineIntegerEmptyWitness::CoefficientGcdDoesNotDivideConstant {
                    row: row_ordinal,
                    constant: budget.clone_integer(&row.components[0])?,
                    coefficient_gcd,
                    remainder,
                    structural_locus_ordinals: clone_lineage(&row.lineage, budget)?,
                },
            ));
        }
        active.push(row_ordinal);
    }
    Ok(ActiveRows::Rows(active))
}

fn eligible_columns(
    state: &SearchState,
    ambient_arity: usize,
    budget: &mut Budget,
) -> Result<Vec<usize>, ResidualAffineIntegerSystemError> {
    budget.reserve("eligible columns", ambient_arity)?;
    let mut eligible = Vec::new();
    eligible.try_reserve_exact(ambient_arity).map_err(|_| {
        ResidualAffineIntegerSystemError::AllocationFailure {
            resource: "eligible columns",
        }
    })?;
    for column in 0..ambient_arity {
        budget.search(1)?;
        let mut already_pivoted = false;
        for &pivot in &state.pivot_positions {
            budget.search(1)?;
            if pivot == column {
                already_pivoted = true;
                break;
            }
        }
        if already_pivoted {
            continue;
        }
        let mut gcd = Integer::zero();
        for &row_ordinal in &state.remaining_rows {
            budget.search(1)?;
            let coefficient = state
                .rows
                .get(row_ordinal)
                .and_then(|row| row.components.get(column + 1))
                .ok_or(
                    ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                        "candidate coefficient is out of range",
                    ),
                )?;
            gcd = bounded_gcd(&gcd, coefficient, budget)?;
            if gcd.is_one() {
                // Remaining entries cannot change gcd one.
                break;
            }
        }
        if gcd.is_one() {
            eligible.push(column);
        }
    }
    Ok(eligible)
}

fn try_clone_search_state(
    source: &SearchState,
    budget: &mut Budget,
) -> Result<SearchState, ResidualAffineIntegerSystemError> {
    let row_components = source.rows.iter().try_fold(0usize, |total, row| {
        checked_add("DFS clone row components", total, row.components.len())
    })?;
    let row_lineages = source.rows.iter().try_fold(0usize, |total, row| {
        checked_add("DFS clone row lineages", total, row.lineage.len())
    })?;
    let operation_integers = source
        .operations
        .iter()
        .try_fold(0usize, |total, operation| {
            checked_add(
                "DFS clone operation integers",
                total,
                operation_integer_count(operation),
            )
        })?;
    let clone_bit_work = source.rows.iter().try_fold(0usize, |total, row| {
        checked_add(
            "integer bit work",
            total,
            integer_clone_bit_work(&row.components)?,
        )
    })?;
    let clone_bit_work =
        source
            .operations
            .iter()
            .try_fold(clone_bit_work, |total, operation| {
                checked_add(
                    "integer bit work",
                    total,
                    row_operation_clone_bit_work(operation)?,
                )
            })?;
    let allocation_entries = [
        source.rows.len(),
        source.remaining_rows.len(),
        source.pivot_positions.len(),
        source.operations.len(),
        row_components,
        row_lineages,
    ]
    .into_iter()
    .try_fold(0usize, |total, amount| {
        checked_add("DFS clone allocation entries", total, amount)
    })?;
    let state_entries = [
        source.rows.len(),
        source.remaining_rows.len(),
        source.pivot_positions.len(),
        source.operations.len(),
        row_components,
        row_lineages,
        operation_integers,
    ]
    .into_iter()
    .try_fold(0usize, |total, amount| {
        checked_add("DFS clone state entries", total, amount)
    })?;
    Budget::preflight_counter(
        budget.stats.allocation_entries_reserved,
        allocation_entries,
        "allocation entries reserved",
        budget.limits.max_allocation_entries_reserved,
    )?;
    Budget::preflight_counter(
        budget.stats.state_entries_materialized,
        state_entries,
        "state entries materialized",
        budget.limits.max_state_entries_materialized,
    )?;
    Budget::preflight_counter(
        budget.stats.lineage_entries_materialized,
        row_lineages,
        "lineage entries materialized",
        budget.limits.max_lineage_entries_materialized,
    )?;
    Budget::preflight_counter(
        budget.stats.operation_integer_entries,
        operation_integers,
        "operation integer entries",
        budget.limits.max_operation_integer_entries,
    )?;
    Budget::preflight_counter(
        budget.stats.integer_bit_work,
        clone_bit_work,
        "integer bit work",
        budget.limits.max_integer_bit_work,
    )?;

    budget.reserve("DFS state rows", source.rows.len())?;
    budget.reserve("DFS state operations", source.operations.len())?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(source.rows.len()).map_err(|_| {
        ResidualAffineIntegerSystemError::AllocationFailure {
            resource: "DFS state rows",
        }
    })?;
    budget.state_materialized(source.rows.len())?;
    for row in &source.rows {
        rows.push(WorkRow {
            components: clone_integer_slice(&row.components, budget, true)?,
            lineage: clone_lineage(&row.lineage, budget)?,
        });
    }
    let remaining_rows = clone_usize_slice(
        &source.remaining_rows,
        "DFS state remaining rows",
        budget,
        true,
    )?;
    let pivot_positions =
        clone_usize_slice(&source.pivot_positions, "DFS state pivots", budget, true)?;
    let mut operations = Vec::new();
    operations
        .try_reserve_exact(source.operations.len())
        .map_err(|_| ResidualAffineIntegerSystemError::AllocationFailure {
            resource: "DFS state operations",
        })?;
    for operation in &source.operations {
        budget.state_materialized(checked_add(
            "state entries materialized",
            1,
            operation_integer_count(operation),
        )?)?;
        budget.operation_integers(operation_integer_count(operation))?;
        operations.push(clone_row_operation(operation, budget)?);
    }
    Ok(SearchState {
        rows,
        remaining_rows,
        pivot_positions,
        operations,
    })
}

fn apply_unit_pivot(
    state: &mut SearchState,
    column: usize,
    budget: &mut Budget,
) -> Result<(), ResidualAffineIntegerSystemError> {
    let mut first_position = None;
    for (position, &row) in state.remaining_rows.iter().enumerate() {
        budget.search(1)?;
        if !state.rows[row].components[column + 1].is_zero() {
            first_position = Some(position);
            break;
        }
    }
    let first_position = first_position.ok_or(
        ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
            "eligible column has no nonzero entry",
        ),
    )?;
    let pivot_row = state.remaining_rows[0];
    let first_nonzero_row = state.remaining_rows[first_position];
    if pivot_row != first_nonzero_row {
        budget.row_operation()?;
        state.rows.swap(pivot_row, first_nonzero_row);
        push_operation(
            state,
            ResidualAffineIntegerRowOperation::Swap {
                left_row: pivot_row,
                right_row: first_nonzero_row,
            },
            budget,
        )?;
    }

    for position in 1..state.remaining_rows.len() {
        let other_row = state.remaining_rows[position];
        if state.rows[other_row].components[column + 1].is_zero() {
            continue;
        }
        apply_bezout_pair(state, pivot_row, other_row, column, budget)?;
    }

    let pivot_coefficient = &state.rows[pivot_row].components[column + 1];
    if verify_integer_equal(pivot_coefficient, &Integer::from(-1), budget)? {
        negate_row(state, pivot_row, budget)?;
    } else if !verify_integer_is_one(pivot_coefficient, budget)? {
        return Err(
            ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                "Bezout reduction did not create a unit pivot",
            ),
        );
    }

    for target_row in 0..state.rows.len() {
        if target_row == pivot_row {
            continue;
        }
        let multiple = budget.clone_integer(&state.rows[target_row].components[column + 1])?;
        if multiple.is_zero() {
            continue;
        }
        eliminate_row(state, target_row, pivot_row, column, &multiple, budget)?;
        exact_normalize_row(state, target_row, budget)?;
    }

    budget.reserve("pivot positions growth", 1)?;
    budget.state_materialized(1)?;
    state.pivot_positions.try_reserve_exact(1).map_err(|_| {
        ResidualAffineIntegerSystemError::AllocationFailure {
            resource: "pivot positions growth",
        }
    })?;
    state.pivot_positions.push(column);
    state.remaining_rows.remove(0);
    Ok(())
}

fn apply_bezout_pair(
    state: &mut SearchState,
    pivot_row: usize,
    other_row: usize,
    column: usize,
    budget: &mut Budget,
) -> Result<(), ResidualAffineIntegerSystemError> {
    budget.row_operation()?;
    let pivot_coefficient = budget.clone_integer(&state.rows[pivot_row].components[column + 1])?;
    let other_coefficient = budget.clone_integer(&state.rows[other_row].components[column + 1])?;
    let (gcd, pivot_bezout, other_bezout) =
        bounded_extended_gcd(&pivot_coefficient, &other_coefficient, budget)?;
    if verify_integer_is_zero(&gcd, budget)? {
        return Err(
            ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                "Bezout pair has zero gcd",
            ),
        );
    }
    let (other_over_gcd, other_remainder) = budget.quotient_remainder(&other_coefficient, &gcd)?;
    let (pivot_over_gcd, pivot_remainder) = budget.quotient_remainder(&pivot_coefficient, &gcd)?;
    let other_divides = verify_integer_is_zero(&other_remainder, budget)?;
    let pivot_divides = verify_integer_is_zero(&pivot_remainder, budget)?;
    if !other_divides || !pivot_divides {
        return Err(
            ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                "Bezout gcd is not an exact divisor",
            ),
        );
    }
    let negative_other_over_gcd = budget.negate_integer(&other_over_gcd)?;
    let new_left = linear_combination_row(
        &state.rows[pivot_row],
        &pivot_bezout,
        &state.rows[other_row],
        &other_bezout,
        budget,
    )?;
    let new_right = linear_combination_row(
        &state.rows[pivot_row],
        &negative_other_over_gcd,
        &state.rows[other_row],
        &pivot_over_gcd,
        budget,
    )?;
    let pivot_matches = verify_integer_equal(&new_left.components[column + 1], &gcd, budget)?;
    let companion_is_zero = verify_integer_is_zero(&new_right.components[column + 1], budget)?;
    if !pivot_matches || !companion_is_zero {
        return Err(
            ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                "Bezout pair transcript does not reduce its column",
            ),
        );
    }
    state.rows[pivot_row] = new_left;
    state.rows[other_row] = new_right;
    push_operation(
        state,
        ResidualAffineIntegerRowOperation::BezoutPair {
            pivot_row,
            other_row,
            column,
            pivot_coefficient,
            other_coefficient,
            gcd,
            pivot_bezout,
            other_bezout,
        },
        budget,
    )?;
    // A unimodular pair can leave the annihilated companion row with a
    // nontrivial common content.  Equality by that row is equivalent after
    // exact content division, and exposing the primitive companion is needed
    // before the next unit-column eligibility test.
    exact_normalize_row(state, other_row, budget)
}

fn negate_row(
    state: &mut SearchState,
    row: usize,
    budget: &mut Budget,
) -> Result<(), ResidualAffineIntegerSystemError> {
    budget.row_operation()?;
    let components = negate_components(&state.rows[row].components, budget)?;
    state.rows[row].components = components;
    push_operation(
        state,
        ResidualAffineIntegerRowOperation::Negate { row },
        budget,
    )
}

fn eliminate_row(
    state: &mut SearchState,
    target_row: usize,
    pivot_row: usize,
    column: usize,
    multiple: &Integer,
    budget: &mut Budget,
) -> Result<(), ResidualAffineIntegerSystemError> {
    budget.row_operation()?;
    let pivot_is_one =
        verify_integer_is_one(&state.rows[pivot_row].components[column + 1], budget)?;
    let multiple_matches = verify_integer_equal(
        &state.rows[target_row].components[column + 1],
        multiple,
        budget,
    )?;
    if !pivot_is_one || !multiple_matches {
        return Err(
            ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                "invalid unit elimination",
            ),
        );
    }
    let negative_multiple = budget.negate_integer(multiple)?;
    let replacement = linear_combination_row(
        &state.rows[target_row],
        &Integer::one(),
        &state.rows[pivot_row],
        &negative_multiple,
        budget,
    )?;
    state.rows[target_row] = replacement;
    if !verify_integer_is_zero(&state.rows[target_row].components[column + 1], budget)? {
        return Err(
            ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                "unit elimination left a nonzero pivot column",
            ),
        );
    }
    let retained_multiple = budget.clone_integer(multiple)?;
    push_operation(
        state,
        ResidualAffineIntegerRowOperation::Eliminate {
            target_row,
            pivot_row,
            column,
            multiple: retained_multiple,
        },
        budget,
    )
}

fn exact_normalize_row(
    state: &mut SearchState,
    row: usize,
    budget: &mut Budget,
) -> Result<(), ResidualAffineIntegerSystemError> {
    let mut divisor = Integer::zero();
    for component in &state.rows[row].components {
        divisor = bounded_gcd(&divisor, component, budget)?;
        if divisor.is_one() {
            break;
        }
    }
    if divisor.is_zero() || divisor.is_one() {
        return Ok(());
    }
    budget.row_operation()?;
    budget.reserve(
        "exact-normalized row components",
        state.rows[row].components.len(),
    )?;
    budget.state_materialized(state.rows[row].components.len())?;
    let mut normalized = Vec::new();
    normalized
        .try_reserve_exact(state.rows[row].components.len())
        .map_err(|_| ResidualAffineIntegerSystemError::AllocationFailure {
            resource: "exact-normalized row components",
        })?;
    for component in &state.rows[row].components {
        let (quotient, remainder) = budget.quotient_remainder(component, &divisor)?;
        if !verify_integer_is_zero(&remainder, budget)? {
            return Err(
                ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                    "row normalization is not exact",
                ),
            );
        }
        normalized.push(quotient);
    }
    state.rows[row].components = normalized;
    push_operation(
        state,
        ResidualAffineIntegerRowOperation::ExactNormalize { row, divisor },
        budget,
    )
}

fn linear_combination_row(
    left: &WorkRow,
    left_multiplier: &Integer,
    right: &WorkRow,
    right_multiplier: &Integer,
    budget: &mut Budget,
) -> Result<WorkRow, ResidualAffineIntegerSystemError> {
    if left.components.len() != right.components.len() {
        return Err(
            ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                "row widths differ in linear combination",
            ),
        );
    }
    budget.reserve("linear-combination row components", left.components.len())?;
    budget.state_materialized(left.components.len())?;
    let mut components = Vec::new();
    components
        .try_reserve_exact(left.components.len())
        .map_err(|_| ResidualAffineIntegerSystemError::AllocationFailure {
            resource: "linear-combination row components",
        })?;
    for (left_component, right_component) in left.components.iter().zip(&right.components) {
        let left_term = budget.multiply_integer(left_multiplier, left_component)?;
        let right_term = budget.multiply_integer(right_multiplier, right_component)?;
        components.push(budget.add_integer(&left_term, &right_term)?);
    }
    let lineage = merge_lineage(&left.lineage, &right.lineage, budget)?;
    Ok(WorkRow {
        components,
        lineage,
    })
}

fn negate_components(
    source: &[Integer],
    budget: &mut Budget,
) -> Result<Vec<Integer>, ResidualAffineIntegerSystemError> {
    budget.reserve("negated row components", source.len())?;
    budget.state_materialized(source.len())?;
    let mut result = Vec::new();
    result.try_reserve_exact(source.len()).map_err(|_| {
        ResidualAffineIntegerSystemError::AllocationFailure {
            resource: "negated row components",
        }
    })?;
    for component in source {
        result.push(budget.negate_integer(component)?);
    }
    Ok(result)
}

fn push_operation(
    state: &mut SearchState,
    operation: ResidualAffineIntegerRowOperation,
    budget: &mut Budget,
) -> Result<(), ResidualAffineIntegerSystemError> {
    let integers = operation_integer_count(&operation);
    budget.operation_integers(integers)?;
    budget.state_materialized(checked_add("state entries materialized", 1, integers)?)?;
    budget.reserve("row-operation transcript", 1)?;
    state.operations.try_reserve_exact(1).map_err(|_| {
        ResidualAffineIntegerSystemError::AllocationFailure {
            resource: "row-operation transcript",
        }
    })?;
    state.operations.push(operation);
    Ok(())
}

fn operation_integer_count(operation: &ResidualAffineIntegerRowOperation) -> usize {
    match operation {
        ResidualAffineIntegerRowOperation::Swap { .. }
        | ResidualAffineIntegerRowOperation::Negate { .. } => 0,
        ResidualAffineIntegerRowOperation::BezoutPair { .. } => 5,
        ResidualAffineIntegerRowOperation::Eliminate { .. }
        | ResidualAffineIntegerRowOperation::ExactNormalize { .. } => 1,
    }
}

fn clone_row_operation(
    operation: &ResidualAffineIntegerRowOperation,
    budget: &mut Budget,
) -> Result<ResidualAffineIntegerRowOperation, ResidualAffineIntegerSystemError> {
    Ok(match operation {
        ResidualAffineIntegerRowOperation::Swap {
            left_row,
            right_row,
        } => ResidualAffineIntegerRowOperation::Swap {
            left_row: *left_row,
            right_row: *right_row,
        },
        ResidualAffineIntegerRowOperation::BezoutPair {
            pivot_row,
            other_row,
            column,
            pivot_coefficient,
            other_coefficient,
            gcd,
            pivot_bezout,
            other_bezout,
        } => ResidualAffineIntegerRowOperation::BezoutPair {
            pivot_row: *pivot_row,
            other_row: *other_row,
            column: *column,
            pivot_coefficient: budget.clone_integer(pivot_coefficient)?,
            other_coefficient: budget.clone_integer(other_coefficient)?,
            gcd: budget.clone_integer(gcd)?,
            pivot_bezout: budget.clone_integer(pivot_bezout)?,
            other_bezout: budget.clone_integer(other_bezout)?,
        },
        ResidualAffineIntegerRowOperation::Negate { row } => {
            ResidualAffineIntegerRowOperation::Negate { row: *row }
        }
        ResidualAffineIntegerRowOperation::Eliminate {
            target_row,
            pivot_row,
            column,
            multiple,
        } => ResidualAffineIntegerRowOperation::Eliminate {
            target_row: *target_row,
            pivot_row: *pivot_row,
            column: *column,
            multiple: budget.clone_integer(multiple)?,
        },
        ResidualAffineIntegerRowOperation::ExactNormalize { row, divisor } => {
            ResidualAffineIntegerRowOperation::ExactNormalize {
                row: *row,
                divisor: budget.clone_integer(divisor)?,
            }
        }
    })
}

fn row_operation_clone_bit_work(
    operation: &ResidualAffineIntegerRowOperation,
) -> Result<usize, ResidualAffineIntegerSystemError> {
    match operation {
        ResidualAffineIntegerRowOperation::Swap { .. }
        | ResidualAffineIntegerRowOperation::Negate { .. } => Ok(0),
        ResidualAffineIntegerRowOperation::BezoutPair {
            pivot_coefficient,
            other_coefficient,
            gcd,
            pivot_bezout,
            other_bezout,
            ..
        } => [
            pivot_coefficient,
            other_coefficient,
            gcd,
            pivot_bezout,
            other_bezout,
        ]
        .into_iter()
        .try_fold(0usize, |total, value| {
            checked_add(
                "integer bit work",
                total,
                integer_magnitude_bits(value)?.max(1),
            )
        }),
        ResidualAffineIntegerRowOperation::Eliminate { multiple, .. } => {
            Ok(integer_magnitude_bits(multiple)?.max(1))
        }
        ResidualAffineIntegerRowOperation::ExactNormalize { divisor, .. } => {
            Ok(integer_magnitude_bits(divisor)?.max(1))
        }
    }
}

fn bounded_gcd(
    left: &Integer,
    right: &Integer,
    budget: &mut Budget,
) -> Result<Integer, ResidualAffineIntegerSystemError> {
    let mut previous = absolute_integer(left, budget)?;
    let mut current = absolute_integer(right, budget)?;
    while !current.is_zero() {
        let (_, remainder) = budget.quotient_remainder(&previous, &current)?;
        previous = current;
        current = remainder;
    }
    Ok(previous)
}

fn bounded_extended_gcd(
    left: &Integer,
    right: &Integer,
    budget: &mut Budget,
) -> Result<(Integer, Integer, Integer), ResidualAffineIntegerSystemError> {
    let mut old_remainder = absolute_integer(left, budget)?;
    let mut remainder = absolute_integer(right, budget)?;
    let mut old_left = Integer::one();
    let mut current_left = Integer::zero();
    let mut old_right = Integer::zero();
    let mut current_right = Integer::one();

    while !remainder.is_zero() {
        let (quotient, next_remainder) = budget.quotient_remainder(&old_remainder, &remainder)?;
        old_remainder = remainder;
        remainder = next_remainder;

        let q_left = budget.multiply_integer(&quotient, &current_left)?;
        let next_left = budget.subtract_integer(&old_left, &q_left)?;
        old_left = current_left;
        current_left = next_left;

        let q_right = budget.multiply_integer(&quotient, &current_right)?;
        let next_right = budget.subtract_integer(&old_right, &q_right)?;
        old_right = current_right;
        current_right = next_right;
    }
    if left.is_negative() {
        old_left = budget.negate_integer(&old_left)?;
    }
    if right.is_negative() {
        old_right = budget.negate_integer(&old_right)?;
    }
    let left_product = budget.multiply_integer(left, &old_left)?;
    let right_product = budget.multiply_integer(right, &old_right)?;
    let reconstructed = budget.add_integer(&left_product, &right_product)?;
    let identity_matches = verify_integer_equal(&reconstructed, &old_remainder, budget)?;
    let gcd_is_negative = verify_integer_is_negative(&old_remainder, budget)?;
    if !identity_matches || gcd_is_negative {
        return Err(
            ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                "extended gcd identity failed",
            ),
        );
    }
    Ok((old_remainder, old_left, old_right))
}

fn absolute_integer(
    value: &Integer,
    budget: &mut Budget,
) -> Result<Integer, ResidualAffineIntegerSystemError> {
    if value.is_negative() {
        budget.negate_integer(value)
    } else {
        budget.clone_integer(value)
    }
}

/// Precharge the conservative gcd work performed by the atom layer's checked
/// primitive-row constructor. The running gcd cannot contain more bits than
/// the largest component visited so far, so this upper bound is independent
/// of the constructor's arithmetic result and is charged in full beforehand.
fn precharge_primitive_row_validation(
    components: &[Integer],
    budget: &mut Budget,
) -> Result<(), ResidualAffineIntegerSystemError> {
    budget.charge_integer_bit_work(primitive_row_validation_bit_work(components)?)
}

fn primitive_row_validation_bit_work(
    components: &[Integer],
) -> Result<usize, ResidualAffineIntegerSystemError> {
    let mut previous_max_bits = 0usize;
    let mut work = 0usize;
    for component in components {
        let bits = integer_magnitude_bits(component)?;
        let gcd_work = checked_mul("integer bit work", bits.max(1), previous_max_bits.max(1))?;
        work = checked_add("integer bit work", work, gcd_work)?;
        previous_max_bits = previous_max_bits.max(bits);
    }
    Ok(work)
}

fn integer_clone_bit_work(values: &[Integer]) -> Result<usize, ResidualAffineIntegerSystemError> {
    values.iter().try_fold(0usize, |total, value| {
        checked_add(
            "integer bit work",
            total,
            integer_magnitude_bits(value)?.max(1),
        )
    })
}

fn clone_integer_slice(
    source: &[Integer],
    budget: &mut Budget,
    count_state: bool,
) -> Result<Vec<Integer>, ResidualAffineIntegerSystemError> {
    budget.reserve("integer-vector clone", source.len())?;
    if count_state {
        budget.state_materialized(source.len())?;
    }
    let mut result = Vec::new();
    result.try_reserve_exact(source.len()).map_err(|_| {
        ResidualAffineIntegerSystemError::AllocationFailure {
            resource: "integer-vector clone",
        }
    })?;
    for value in source {
        result.push(budget.clone_integer(value)?);
    }
    Ok(result)
}

fn clone_usize_slice(
    source: &[usize],
    resource: &'static str,
    budget: &mut Budget,
    count_state: bool,
) -> Result<Vec<usize>, ResidualAffineIntegerSystemError> {
    budget.reserve(resource, source.len())?;
    if count_state {
        budget.state_materialized(source.len())?;
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(source.len())
        .map_err(|_| ResidualAffineIntegerSystemError::AllocationFailure { resource })?;
    result.extend_from_slice(source);
    Ok(result)
}

fn clone_lineage(
    source: &[usize],
    budget: &mut Budget,
) -> Result<Vec<usize>, ResidualAffineIntegerSystemError> {
    budget.lineage_materialized(source.len())?;
    clone_usize_slice(source, "structural-locus lineage clone", budget, true)
}

fn merge_lineage(
    left: &[usize],
    right: &[usize],
    budget: &mut Budget,
) -> Result<Vec<usize>, ResidualAffineIntegerSystemError> {
    let capacity = checked_add("structural-locus lineage union", left.len(), right.len())?;
    budget.reserve("structural-locus lineage union", capacity)?;
    let mut result = Vec::new();
    result.try_reserve_exact(capacity).map_err(|_| {
        ResidualAffineIntegerSystemError::AllocationFailure {
            resource: "structural-locus lineage union",
        }
    })?;
    let mut left_cursor = 0usize;
    let mut right_cursor = 0usize;
    while left_cursor < left.len() || right_cursor < right.len() {
        budget.lineage_operation(1)?;
        let next = match (left.get(left_cursor), right.get(right_cursor)) {
            (Some(&left_value), Some(&right_value)) => match left_value.cmp(&right_value) {
                Ordering::Less => {
                    left_cursor += 1;
                    left_value
                }
                Ordering::Equal => {
                    left_cursor += 1;
                    right_cursor += 1;
                    left_value
                }
                Ordering::Greater => {
                    right_cursor += 1;
                    right_value
                }
            },
            (Some(&left_value), None) => {
                left_cursor += 1;
                left_value
            }
            (None, Some(&right_value)) => {
                right_cursor += 1;
                right_value
            }
            (None, None) => break,
        };
        budget.lineage_materialized(1)?;
        budget.state_materialized(1)?;
        result.push(next);
    }
    Ok(result)
}

fn copy_final_rows(
    source: &[WorkRow],
    budget: &mut Budget,
) -> Result<Vec<ResidualAffineIntegerFinalRow>, ResidualAffineIntegerSystemError> {
    budget.reserve("final rows", source.len())?;
    let mut result = Vec::new();
    result.try_reserve_exact(source.len()).map_err(|_| {
        ResidualAffineIntegerSystemError::AllocationFailure {
            resource: "final rows",
        }
    })?;
    for row in source {
        result.push(ResidualAffineIntegerFinalRow {
            components: clone_integer_slice(&row.components, budget, false)?,
            structural_locus_ordinals: clone_lineage(&row.lineage, budget)?,
        });
    }
    Ok(result)
}

fn complement_positions(
    ambient_arity: usize,
    pivot_positions: &[usize],
    budget: &mut Budget,
) -> Result<Vec<usize>, ResidualAffineIntegerSystemError> {
    budget.reserve("free positions", ambient_arity)?;
    let mut result = Vec::new();
    result.try_reserve_exact(ambient_arity).map_err(|_| {
        ResidualAffineIntegerSystemError::AllocationFailure {
            resource: "free positions",
        }
    })?;
    for position in 0..ambient_arity {
        budget.search(1)?;
        let mut pivoted = false;
        for &pivot in pivot_positions {
            budget.search(1)?;
            if pivot == position {
                pivoted = true;
                break;
            }
        }
        if !pivoted {
            result.push(position);
        }
    }
    Ok(result)
}

fn build_affine_map(
    ambient_arity: usize,
    state: &SearchState,
    free_positions: &[usize],
    budget: &mut Budget,
) -> Result<ResidualAffineIntegerMap, ResidualAffineIntegerSystemError> {
    let matrix_entries = checked_mul("map entries", ambient_arity, ambient_arity)?;
    let map_entries = checked_add("map entries", ambient_arity, matrix_entries)?;
    check_limit("map entries", map_entries, budget.limits.max_map_entries)?;
    budget.stats.map_entries = map_entries;
    budget.reserve("map constants", ambient_arity)?;
    budget.reserve("map matrix", matrix_entries)?;
    let mut constants = Vec::new();
    constants.try_reserve_exact(ambient_arity).map_err(|_| {
        ResidualAffineIntegerSystemError::AllocationFailure {
            resource: "map constants",
        }
    })?;
    constants.resize(ambient_arity, Integer::zero());
    let mut linear_coefficients = Vec::new();
    linear_coefficients
        .try_reserve_exact(matrix_entries)
        .map_err(|_| ResidualAffineIntegerSystemError::AllocationFailure {
            resource: "map matrix",
        })?;
    linear_coefficients.resize(matrix_entries, Integer::zero());

    for &free in free_positions {
        let offset = checked_add(
            "map matrix offset",
            checked_mul("map matrix offset", free, ambient_arity)?,
            free,
        )?;
        linear_coefficients[offset] = Integer::one();
    }
    for &pivot in &state.pivot_positions {
        let mut pivot_row = None;
        for (row_ordinal, row) in state.rows.iter().enumerate() {
            budget.search(1)?;
            if row.components[pivot + 1].is_one() {
                pivot_row = Some(row_ordinal);
                break;
            }
        }
        let pivot_row = pivot_row.ok_or(
            ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                "final unit-pivot row is missing",
            ),
        )?;
        let row = &state.rows[pivot_row];
        for &other_pivot in &state.pivot_positions {
            budget.search(1)?;
            let expected = if other_pivot == pivot {
                Integer::one()
            } else {
                Integer::zero()
            };
            if !verify_integer_equal(&row.components[other_pivot + 1], &expected, budget)? {
                return Err(
                    ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                        "final pivot block is not the identity",
                    ),
                );
            }
        }
        constants[pivot] = budget.negate_integer(&row.components[0])?;
        for &free in free_positions {
            let offset = checked_add(
                "map matrix offset",
                checked_mul("map matrix offset", pivot, ambient_arity)?,
                free,
            )?;
            linear_coefficients[offset] = budget.negate_integer(&row.components[free + 1])?;
        }
    }
    Ok(ResidualAffineIntegerMap {
        ambient_arity,
        constants,
        linear_coefficients,
        pivot_positions: clone_usize_slice(
            &state.pivot_positions,
            "map pivot positions",
            budget,
            false,
        )?,
        free_positions: clone_usize_slice(free_positions, "map free positions", budget, false)?,
    })
}

fn verify_transcript(
    source: &[ResidualAffineIntegerSystemInputRow],
    expected: &SearchState,
    budget: &mut Budget,
) -> Result<(), ResidualAffineIntegerSystemError> {
    budget.reserve("verification rows", source.len())?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(source.len()).map_err(|_| {
        ResidualAffineIntegerSystemError::AllocationFailure {
            resource: "verification rows",
        }
    })?;
    for source_row in source {
        rows.push(WorkRow {
            components: clone_integer_slice(source_row.row().components(), budget, false)?,
            lineage: clone_lineage(source_row.structural_locus_ordinals(), budget)?,
        });
    }
    for operation in &expected.operations {
        budget.verify(1)?;
        match operation {
            ResidualAffineIntegerRowOperation::Swap {
                left_row,
                right_row,
            } => {
                if *left_row >= rows.len() || *right_row >= rows.len() {
                    return Err(
                        ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                            "transcript swap row is out of range",
                        ),
                    );
                }
                rows.swap(*left_row, *right_row);
            }
            ResidualAffineIntegerRowOperation::BezoutPair {
                pivot_row,
                other_row,
                column,
                pivot_coefficient,
                other_coefficient,
                gcd,
                pivot_bezout,
                other_bezout,
            } => {
                let left = rows.get(*pivot_row).ok_or(
                    ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                        "transcript Bezout pivot row is out of range",
                    ),
                )?;
                let right = rows.get(*other_row).ok_or(
                    ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                        "transcript Bezout other row is out of range",
                    ),
                )?;
                let left_coefficient = left.components.get(column + 1).ok_or(
                    ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                        "transcript Bezout pivot column is out of range",
                    ),
                )?;
                let right_coefficient = right.components.get(column + 1).ok_or(
                    ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                        "transcript Bezout other column is out of range",
                    ),
                )?;
                let pivot_source_matches =
                    verify_integer_equal(left_coefficient, pivot_coefficient, budget)?;
                let other_source_matches =
                    verify_integer_equal(right_coefficient, other_coefficient, budget)?;
                if !pivot_source_matches || !other_source_matches {
                    return Err(
                        ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                            "transcript Bezout source coefficients differ",
                        ),
                    );
                }
                let left_term = budget.multiply_integer(pivot_coefficient, pivot_bezout)?;
                let right_term = budget.multiply_integer(other_coefficient, other_bezout)?;
                let reconstructed_gcd = budget.add_integer(&left_term, &right_term)?;
                if !verify_integer_equal(&reconstructed_gcd, gcd, budget)? {
                    return Err(
                        ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                            "transcript Bezout identity differs",
                        ),
                    );
                }
                let (other_over_gcd, other_remainder) =
                    budget.quotient_remainder(other_coefficient, gcd)?;
                let (pivot_over_gcd, pivot_remainder) =
                    budget.quotient_remainder(pivot_coefficient, gcd)?;
                let other_divides = verify_integer_is_zero(&other_remainder, budget)?;
                let pivot_divides = verify_integer_is_zero(&pivot_remainder, budget)?;
                if !other_divides || !pivot_divides {
                    return Err(
                        ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                            "transcript Bezout gcd division is not exact",
                        ),
                    );
                }
                let negative_other = budget.negate_integer(&other_over_gcd)?;
                let new_left =
                    linear_combination_row(left, pivot_bezout, right, other_bezout, budget)?;
                let new_right =
                    linear_combination_row(left, &negative_other, right, &pivot_over_gcd, budget)?;
                rows[*pivot_row] = new_left;
                rows[*other_row] = new_right;
            }
            ResidualAffineIntegerRowOperation::Negate { row } => {
                let source_components = rows
                    .get(*row)
                    .ok_or(
                        ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                            "transcript negated row is out of range",
                        ),
                    )?
                    .components
                    .as_slice();
                let negated = negate_components(source_components, budget)?;
                rows[*row].components = negated;
            }
            ResidualAffineIntegerRowOperation::Eliminate {
                target_row,
                pivot_row,
                column,
                multiple,
            } => {
                let target = rows.get(*target_row).ok_or(
                    ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                        "transcript elimination target is out of range",
                    ),
                )?;
                let pivot = rows.get(*pivot_row).ok_or(
                    ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                        "transcript elimination pivot is out of range",
                    ),
                )?;
                let target_coefficient = target.components.get(column + 1).ok_or(
                    ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                        "transcript elimination target column is out of range",
                    ),
                )?;
                let pivot_coefficient = pivot.components.get(column + 1).ok_or(
                    ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                        "transcript elimination pivot column is out of range",
                    ),
                )?;
                let multiple_matches = verify_integer_equal(target_coefficient, multiple, budget)?;
                let pivot_is_one = verify_integer_is_one(pivot_coefficient, budget)?;
                if !multiple_matches || !pivot_is_one {
                    return Err(
                        ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                            "transcript elimination source differs",
                        ),
                    );
                }
                let negative_multiple = budget.negate_integer(multiple)?;
                let replacement = linear_combination_row(
                    target,
                    &Integer::one(),
                    pivot,
                    &negative_multiple,
                    budget,
                )?;
                rows[*target_row] = replacement;
            }
            ResidualAffineIntegerRowOperation::ExactNormalize { row, divisor } => {
                let divisor_is_zero = verify_integer_is_zero(divisor, budget)?;
                let divisor_is_negative = verify_integer_is_negative(divisor, budget)?;
                if divisor_is_zero || divisor_is_negative {
                    return Err(
                        ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                            "transcript normalization divisor is not positive",
                        ),
                    );
                }
                let source_components = rows
                    .get(*row)
                    .ok_or(
                        ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                            "transcript normalized row is out of range",
                        ),
                    )?
                    .components
                    .as_slice();
                budget.reserve("verification normalized row", source_components.len())?;
                let mut normalized = Vec::new();
                normalized
                    .try_reserve_exact(source_components.len())
                    .map_err(|_| ResidualAffineIntegerSystemError::AllocationFailure {
                        resource: "verification normalized row",
                    })?;
                for component in source_components {
                    budget.verify(1)?;
                    let (quotient, remainder) = budget.quotient_remainder(component, divisor)?;
                    if !verify_integer_is_zero(&remainder, budget)? {
                        return Err(
                            ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                                "transcript normalization is not exact",
                            ),
                        );
                    }
                    normalized.push(quotient);
                }
                rows[*row].components = normalized;
            }
        }
    }
    if !verify_work_rows_equal(&rows, &expected.rows, budget)? {
        return Err(
            ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                "row-operation transcript does not reconstruct final rows",
            ),
        );
    }
    Ok(())
}

fn verify_work_rows_equal(
    actual: &[WorkRow],
    expected: &[WorkRow],
    budget: &mut Budget,
) -> Result<bool, ResidualAffineIntegerSystemError> {
    budget.verify(1)?;
    if actual.len() != expected.len() {
        return Ok(false);
    }
    for (actual_row, expected_row) in actual.iter().zip(expected) {
        budget.verify(1)?;
        if actual_row.components.len() != expected_row.components.len()
            || actual_row.lineage.len() != expected_row.lineage.len()
        {
            return Ok(false);
        }
        for (actual_component, expected_component) in
            actual_row.components.iter().zip(&expected_row.components)
        {
            if !verify_integer_equal(actual_component, expected_component, budget)? {
                return Ok(false);
            }
        }
        for (actual_ordinal, expected_ordinal) in
            actual_row.lineage.iter().zip(&expected_row.lineage)
        {
            budget.verify(1)?;
            if actual_ordinal != expected_ordinal {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn verify_integer_equal(
    left: &Integer,
    right: &Integer,
    budget: &mut Budget,
) -> Result<bool, ResidualAffineIntegerSystemError> {
    budget.verify(1)?;
    charged_integer_equal(left, right, budget)
}

/// Compare arbitrary-precision integers only after charging the prospective
/// deep-comparison work.  Construction-time arithmetic invariant checks use
/// this helper through `verify_integer_equal`, just like transcript and map
/// verification, so the successful census covers every proof-bearing deep
/// equality.
fn charged_integer_equal(
    left: &Integer,
    right: &Integer,
    budget: &mut Budget,
) -> Result<bool, ResidualAffineIntegerSystemError> {
    // Comparison never legitimizes an over-limit retained coefficient, even
    // on the sound pointer-reflexive fast path.
    budget.validate_integer(left)?;
    budget.validate_integer(right)?;
    if std::ptr::eq(left, right) {
        return Ok(true);
    }
    budget.charge_integer_bit_work(
        integer_magnitude_bits(left)?
            .max(integer_magnitude_bits(right)?)
            .max(1),
    )?;
    Ok(left == right)
}

fn charged_integer_cmp(
    left: &Integer,
    right: &Integer,
    budget: &mut Budget,
) -> Result<Ordering, ResidualAffineIntegerSystemError> {
    budget.validate_integer(left)?;
    budget.validate_integer(right)?;
    if std::ptr::eq(left, right) {
        return Ok(Ordering::Equal);
    }
    budget.charge_integer_bit_work(
        integer_magnitude_bits(left)?
            .max(integer_magnitude_bits(right)?)
            .max(1),
    )?;
    Ok(left.cmp(right))
}

// Symbolica's normalized `Integer` representation makes zero, unit, and sign
// predicates representation checks rather than limb-wise deep comparisons.
// They therefore consume one prospectively charged verification operation but
// no magnitude-proportional integer bit-work.
fn verify_integer_is_zero(
    value: &Integer,
    budget: &mut Budget,
) -> Result<bool, ResidualAffineIntegerSystemError> {
    budget.verify(1)?;
    Ok(value.is_zero())
}

fn verify_integer_is_one(
    value: &Integer,
    budget: &mut Budget,
) -> Result<bool, ResidualAffineIntegerSystemError> {
    budget.verify(1)?;
    Ok(value.is_one())
}

fn verify_integer_is_negative(
    value: &Integer,
    budget: &mut Budget,
) -> Result<bool, ResidualAffineIntegerSystemError> {
    budget.verify(1)?;
    Ok(value.is_negative())
}

fn verify_usize_slice_equal(
    left: &[usize],
    right: &[usize],
    budget: &mut Budget,
) -> Result<bool, ResidualAffineIntegerSystemError> {
    budget.verify(1)?;
    if std::ptr::eq(left, right) {
        return Ok(true);
    }
    let mut equal = left.len() == right.len();
    let shared = left.len().min(right.len());
    for ordinal in 0..shared {
        budget.verify(1)?;
        equal &= left[ordinal] == right[ordinal];
    }
    // Charge every unmatched retained entry too.  Although the first length
    // mismatch is decisive, these entries are still part of the proof-bearing
    // payload whose bounded census the verifier certifies.
    budget.verify(left.len().max(right.len()) - shared)?;
    Ok(equal)
}

fn verify_integer_slice_equal(
    left: &[Integer],
    right: &[Integer],
    budget: &mut Budget,
) -> Result<bool, ResidualAffineIntegerSystemError> {
    budget.verify(1)?;
    if std::ptr::eq(left, right) {
        return Ok(true);
    }
    if left.len() != right.len() {
        return Ok(false);
    }
    for (left_value, right_value) in left.iter().zip(right) {
        if !verify_integer_equal(left_value, right_value, budget)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn verify_primitive_row_payload_equal(
    left: &ResidualAffinePrimitiveRow,
    right: &ResidualAffinePrimitiveRow,
    budget: &mut Budget,
) -> Result<bool, ResidualAffineIntegerSystemError> {
    budget.verify(1)?;
    if std::ptr::eq(left, right) {
        return Ok(true);
    }
    verify_integer_slice_equal(left.components(), right.components(), budget)
}

fn verify_input_row_payload_equal(
    left: &ResidualAffineIntegerSystemInputRow,
    right: &ResidualAffineIntegerSystemInputRow,
    budget: &mut Budget,
) -> Result<bool, ResidualAffineIntegerSystemError> {
    budget.verify(1)?;
    if std::ptr::eq(left, right) {
        return Ok(true);
    }
    if !verify_primitive_row_payload_equal(&left.row, &right.row, budget)? {
        return Ok(false);
    }
    verify_usize_slice_equal(
        &left.structural_locus_ordinals,
        &right.structural_locus_ordinals,
        budget,
    )
}

fn verify_input_rows_payload_equal(
    left: &[ResidualAffineIntegerSystemInputRow],
    right: &[ResidualAffineIntegerSystemInputRow],
    budget: &mut Budget,
) -> Result<bool, ResidualAffineIntegerSystemError> {
    budget.verify(1)?;
    if std::ptr::eq(left, right) {
        return Ok(true);
    }
    if left.len() != right.len() {
        return Ok(false);
    }
    for (left_row, right_row) in left.iter().zip(right) {
        if !verify_input_row_payload_equal(left_row, right_row, budget)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn verify_row_operation_payload_equal(
    left: &ResidualAffineIntegerRowOperation,
    right: &ResidualAffineIntegerRowOperation,
    budget: &mut Budget,
) -> Result<bool, ResidualAffineIntegerSystemError> {
    budget.verify(1)?;
    if std::ptr::eq(left, right) {
        return Ok(true);
    }
    match (left, right) {
        (
            ResidualAffineIntegerRowOperation::Swap {
                left_row: left_left_row,
                right_row: left_right_row,
            },
            ResidualAffineIntegerRowOperation::Swap {
                left_row: right_left_row,
                right_row: right_right_row,
            },
        ) => {
            budget.verify(1)?;
            Ok(left_left_row == right_left_row && left_right_row == right_right_row)
        }
        (
            ResidualAffineIntegerRowOperation::BezoutPair {
                pivot_row: left_pivot_row,
                other_row: left_other_row,
                column: left_column,
                pivot_coefficient: left_pivot_coefficient,
                other_coefficient: left_other_coefficient,
                gcd: left_gcd,
                pivot_bezout: left_pivot_bezout,
                other_bezout: left_other_bezout,
            },
            ResidualAffineIntegerRowOperation::BezoutPair {
                pivot_row: right_pivot_row,
                other_row: right_other_row,
                column: right_column,
                pivot_coefficient: right_pivot_coefficient,
                other_coefficient: right_other_coefficient,
                gcd: right_gcd,
                pivot_bezout: right_pivot_bezout,
                other_bezout: right_other_bezout,
            },
        ) => {
            budget.verify(1)?;
            if left_pivot_row != right_pivot_row
                || left_other_row != right_other_row
                || left_column != right_column
            {
                return Ok(false);
            }
            for (left_value, right_value) in [
                (left_pivot_coefficient, right_pivot_coefficient),
                (left_other_coefficient, right_other_coefficient),
                (left_gcd, right_gcd),
                (left_pivot_bezout, right_pivot_bezout),
                (left_other_bezout, right_other_bezout),
            ] {
                if !verify_integer_equal(left_value, right_value, budget)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (
            ResidualAffineIntegerRowOperation::Negate { row: left_row },
            ResidualAffineIntegerRowOperation::Negate { row: right_row },
        ) => {
            budget.verify(1)?;
            Ok(left_row == right_row)
        }
        (
            ResidualAffineIntegerRowOperation::Eliminate {
                target_row: left_target_row,
                pivot_row: left_pivot_row,
                column: left_column,
                multiple: left_multiple,
            },
            ResidualAffineIntegerRowOperation::Eliminate {
                target_row: right_target_row,
                pivot_row: right_pivot_row,
                column: right_column,
                multiple: right_multiple,
            },
        ) => {
            budget.verify(1)?;
            if left_target_row != right_target_row
                || left_pivot_row != right_pivot_row
                || left_column != right_column
            {
                return Ok(false);
            }
            verify_integer_equal(left_multiple, right_multiple, budget)
        }
        (
            ResidualAffineIntegerRowOperation::ExactNormalize {
                row: left_row,
                divisor: left_divisor,
            },
            ResidualAffineIntegerRowOperation::ExactNormalize {
                row: right_row,
                divisor: right_divisor,
            },
        ) => {
            budget.verify(1)?;
            if left_row != right_row {
                return Ok(false);
            }
            verify_integer_equal(left_divisor, right_divisor, budget)
        }
        _ => Ok(false),
    }
}

fn verify_row_operations_payload_equal(
    left: &[ResidualAffineIntegerRowOperation],
    right: &[ResidualAffineIntegerRowOperation],
    budget: &mut Budget,
) -> Result<bool, ResidualAffineIntegerSystemError> {
    budget.verify(1)?;
    if std::ptr::eq(left, right) {
        return Ok(true);
    }
    if left.len() != right.len() {
        return Ok(false);
    }
    for (left_operation, right_operation) in left.iter().zip(right) {
        if !verify_row_operation_payload_equal(left_operation, right_operation, budget)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn verify_final_row_payload_equal(
    left: &ResidualAffineIntegerFinalRow,
    right: &ResidualAffineIntegerFinalRow,
    budget: &mut Budget,
) -> Result<bool, ResidualAffineIntegerSystemError> {
    budget.verify(1)?;
    if std::ptr::eq(left, right) {
        return Ok(true);
    }
    if !verify_integer_slice_equal(&left.components, &right.components, budget)? {
        return Ok(false);
    }
    verify_usize_slice_equal(
        &left.structural_locus_ordinals,
        &right.structural_locus_ordinals,
        budget,
    )
}

fn verify_final_rows_payload_equal(
    left: &[ResidualAffineIntegerFinalRow],
    right: &[ResidualAffineIntegerFinalRow],
    budget: &mut Budget,
) -> Result<bool, ResidualAffineIntegerSystemError> {
    budget.verify(1)?;
    if std::ptr::eq(left, right) {
        return Ok(true);
    }
    if left.len() != right.len() {
        return Ok(false);
    }
    for (left_row, right_row) in left.iter().zip(right) {
        if !verify_final_row_payload_equal(left_row, right_row, budget)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn verify_affine_map_payload_equal(
    left: &ResidualAffineIntegerMap,
    right: &ResidualAffineIntegerMap,
    budget: &mut Budget,
) -> Result<bool, ResidualAffineIntegerSystemError> {
    budget.verify(1)?;
    if std::ptr::eq(left, right) {
        return Ok(true);
    }
    budget.verify(1)?;
    if left.ambient_arity != right.ambient_arity {
        return Ok(false);
    }
    if !verify_integer_slice_equal(&left.constants, &right.constants, budget)?
        || !verify_integer_slice_equal(
            &left.linear_coefficients,
            &right.linear_coefficients,
            budget,
        )?
        || !verify_usize_slice_equal(&left.pivot_positions, &right.pivot_positions, budget)?
        || !verify_usize_slice_equal(&left.free_positions, &right.free_positions, budget)?
    {
        return Ok(false);
    }
    Ok(true)
}

fn verify_optional_affine_map_payload_equal(
    left: &Option<ResidualAffineIntegerMap>,
    right: &Option<ResidualAffineIntegerMap>,
    budget: &mut Budget,
) -> Result<bool, ResidualAffineIntegerSystemError> {
    budget.verify(1)?;
    if std::ptr::eq(left, right) {
        return Ok(true);
    }
    match (left, right) {
        (Some(left), Some(right)) => verify_affine_map_payload_equal(left, right, budget),
        (None, None) => Ok(true),
        _ => Ok(false),
    }
}

fn verify_empty_witness_payload_equal(
    left: &ResidualAffineIntegerEmptyWitness,
    right: &ResidualAffineIntegerEmptyWitness,
    budget: &mut Budget,
) -> Result<bool, ResidualAffineIntegerSystemError> {
    budget.verify(1)?;
    if std::ptr::eq(left, right) {
        return Ok(true);
    }
    match (left, right) {
        (
            ResidualAffineIntegerEmptyWitness::ZeroEqualsNonzero {
                row: left_row,
                constant: left_constant,
                structural_locus_ordinals: left_lineage,
            },
            ResidualAffineIntegerEmptyWitness::ZeroEqualsNonzero {
                row: right_row,
                constant: right_constant,
                structural_locus_ordinals: right_lineage,
            },
        ) => {
            budget.verify(1)?;
            if left_row != right_row
                || !verify_integer_equal(left_constant, right_constant, budget)?
            {
                return Ok(false);
            }
            verify_usize_slice_equal(left_lineage, right_lineage, budget)
        }
        (
            ResidualAffineIntegerEmptyWitness::CoefficientGcdDoesNotDivideConstant {
                row: left_row,
                constant: left_constant,
                coefficient_gcd: left_gcd,
                remainder: left_remainder,
                structural_locus_ordinals: left_lineage,
            },
            ResidualAffineIntegerEmptyWitness::CoefficientGcdDoesNotDivideConstant {
                row: right_row,
                constant: right_constant,
                coefficient_gcd: right_gcd,
                remainder: right_remainder,
                structural_locus_ordinals: right_lineage,
            },
        ) => {
            budget.verify(1)?;
            if left_row != right_row {
                return Ok(false);
            }
            for (left_value, right_value) in [
                (left_constant, right_constant),
                (left_gcd, right_gcd),
                (left_remainder, right_remainder),
            ] {
                if !verify_integer_equal(left_value, right_value, budget)? {
                    return Ok(false);
                }
            }
            verify_usize_slice_equal(left_lineage, right_lineage, budget)
        }
        _ => Ok(false),
    }
}

fn verify_optional_empty_witness_payload_equal(
    left: &Option<ResidualAffineIntegerEmptyWitness>,
    right: &Option<ResidualAffineIntegerEmptyWitness>,
    budget: &mut Budget,
) -> Result<bool, ResidualAffineIntegerSystemError> {
    budget.verify(1)?;
    if std::ptr::eq(left, right) {
        return Ok(true);
    }
    match (left, right) {
        (Some(left), Some(right)) => verify_empty_witness_payload_equal(left, right, budget),
        (None, None) => Ok(true),
        _ => Ok(false),
    }
}

fn verify_certificate_payload_equal(
    left: &ResidualAffineIntegerSystemCertificate,
    right: &ResidualAffineIntegerSystemCertificate,
    budget: &mut Budget,
) -> Result<bool, ResidualAffineIntegerSystemError> {
    budget.verify(1)?;
    if std::ptr::eq(left, right) {
        return Ok(true);
    }
    budget.verify(1)?;
    if left.schema != right.schema || left.ambient_arity != right.ambient_arity {
        return Ok(false);
    }
    if !verify_input_rows_payload_equal(
        &left.replay_source_rows,
        &right.replay_source_rows,
        budget,
    )? || !verify_input_rows_payload_equal(&left.source_rows, &right.source_rows, budget)?
        || !verify_row_operations_payload_equal(&left.operations, &right.operations, budget)?
        || !verify_final_rows_payload_equal(&left.final_rows, &right.final_rows, budget)?
        || !verify_usize_slice_equal(&left.pivot_positions, &right.pivot_positions, budget)?
        || !verify_usize_slice_equal(&left.free_positions, &right.free_positions, budget)?
        || !verify_optional_affine_map_payload_equal(&left.affine_map, &right.affine_map, budget)?
        || !verify_optional_empty_witness_payload_equal(
            &left.empty_witness,
            &right.empty_witness,
            budget,
        )?
    {
        return Ok(false);
    }
    budget.verify(1)?;
    if left.limits != right.limits {
        return Ok(false);
    }
    budget.verify(1)?;
    Ok(left.stats == right.stats)
}

fn verify_affine_map(
    source: &[ResidualAffineIntegerSystemInputRow],
    map: &ResidualAffineIntegerMap,
    budget: &mut Budget,
) -> Result<(), ResidualAffineIntegerSystemError> {
    let arity = map.ambient_arity;
    if map.constants.len() != arity
        || map.linear_coefficients.len() != checked_mul("map verification entries", arity, arity)?
    {
        return Err(
            ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                "affine map dimensions differ",
            ),
        );
    }

    // Every authenticated source equation vanishes after n -> b + A*n.
    for source_row in source {
        let row = source_row.row();
        let mut constant_image = budget.clone_integer(row.constant())?;
        for position in 0..arity {
            budget.verify(1)?;
            let product =
                budget.multiply_integer(&row.coefficients()[position], &map.constants[position])?;
            constant_image = budget.add_integer(&constant_image, &product)?;
        }
        if !verify_integer_is_zero(&constant_image, budget)? {
            return Err(
                ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                    "affine map does not solve a source-row constant",
                ),
            );
        }
        for column in 0..arity {
            let mut image = Integer::zero();
            for position in 0..arity {
                budget.verify(1)?;
                let product = budget.multiply_integer(
                    &row.coefficients()[position],
                    map_entry(map, position, column)?,
                )?;
                image = budget.add_integer(&image, &product)?;
            }
            if !verify_integer_is_zero(&image, budget)? {
                return Err(
                    ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                        "affine map does not annihilate a source-row linear part",
                    ),
                );
            }
        }
    }

    // The retained square map is an affine projection: A^2=A and A*b=0.
    for row in 0..arity {
        for column in 0..arity {
            let mut product_sum = Integer::zero();
            for middle in 0..arity {
                budget.verify(1)?;
                let product = budget.multiply_integer(
                    map_entry(map, row, middle)?,
                    map_entry(map, middle, column)?,
                )?;
                product_sum = budget.add_integer(&product_sum, &product)?;
            }
            if !verify_integer_equal(&product_sum, map_entry(map, row, column)?, budget)? {
                return Err(
                    ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                        "affine map matrix is not idempotent",
                    ),
                );
            }
        }
        let mut translated = Integer::zero();
        for column in 0..arity {
            budget.verify(1)?;
            let product =
                budget.multiply_integer(map_entry(map, row, column)?, &map.constants[column])?;
            translated = budget.add_integer(&translated, &product)?;
        }
        if !verify_integer_is_zero(&translated, budget)? {
            return Err(
                ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                    "affine map violates A*b=0",
                ),
            );
        }
    }

    for &free in &map.free_positions {
        for column in 0..arity {
            budget.verify(1)?;
            let expected = if column == free {
                Integer::one()
            } else {
                Integer::zero()
            };
            let entry_matches =
                verify_integer_equal(map_entry(map, free, column)?, &expected, budget)?;
            let translation_is_zero = verify_integer_is_zero(&map.constants[free], budget)?;
            if !entry_matches || !translation_is_zero {
                return Err(
                    ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                        "free row of affine map is not the identity",
                    ),
                );
            }
        }
    }
    Ok(())
}

fn verify_empty_witness(
    witness: &ResidualAffineIntegerEmptyWitness,
    final_rows: &[WorkRow],
    budget: &mut Budget,
) -> Result<(), ResidualAffineIntegerSystemError> {
    match witness {
        ResidualAffineIntegerEmptyWitness::ZeroEqualsNonzero {
            row,
            constant,
            structural_locus_ordinals,
        } => {
            let final_row = final_rows.get(*row).ok_or(
                ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                    "empty-witness row is out of range",
                ),
            )?;
            let constant_matches =
                verify_integer_equal(&final_row.components[0], constant, budget)?;
            let constant_is_nonzero = !verify_integer_is_zero(constant, budget)?;
            let mut coefficients_are_zero = true;
            for coefficient in &final_row.components[1..] {
                coefficients_are_zero &= verify_integer_is_zero(coefficient, budget)?;
            }
            let lineage_matches =
                verify_usize_slice_equal(&final_row.lineage, structural_locus_ordinals, budget)?;
            if !constant_matches
                || !constant_is_nonzero
                || !coefficients_are_zero
                || !lineage_matches
            {
                return Err(
                    ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                        "zero-equals-nonzero witness differs from final row",
                    ),
                );
            }
        }
        ResidualAffineIntegerEmptyWitness::CoefficientGcdDoesNotDivideConstant {
            row,
            constant,
            coefficient_gcd,
            remainder,
            structural_locus_ordinals,
        } => {
            let final_row = final_rows.get(*row).ok_or(
                ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                    "congruence-witness row is out of range",
                ),
            )?;
            let mut reconstructed_gcd = Integer::zero();
            for coefficient in &final_row.components[1..] {
                budget.verify(1)?;
                reconstructed_gcd = bounded_gcd(&reconstructed_gcd, coefficient, budget)?;
            }
            let (_, reconstructed_remainder) =
                budget.quotient_remainder(&final_row.components[0], &reconstructed_gcd)?;
            let constant_matches =
                verify_integer_equal(&final_row.components[0], constant, budget)?;
            let gcd_matches = verify_integer_equal(&reconstructed_gcd, coefficient_gcd, budget)?;
            let remainder_matches =
                verify_integer_equal(&reconstructed_remainder, remainder, budget)?;
            let remainder_is_nonzero = !verify_integer_is_zero(remainder, budget)?;
            let lineage_matches =
                verify_usize_slice_equal(&final_row.lineage, structural_locus_ordinals, budget)?;
            if !constant_matches
                || !gcd_matches
                || !remainder_matches
                || !remainder_is_nonzero
                || !lineage_matches
            {
                return Err(
                    ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                        "coefficient-gcd empty witness differs from final row",
                    ),
                );
            }
        }
    }
    Ok(())
}

fn map_entry(
    map: &ResidualAffineIntegerMap,
    row: usize,
    column: usize,
) -> Result<&Integer, ResidualAffineIntegerSystemError> {
    map.linear_coefficient(row, column).ok_or(
        ResidualAffineIntegerSystemError::ArithmeticInvariantFailure("map entry is out of range"),
    )
}

fn is_strictly_increasing(values: &[usize]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

fn retained_position_slices_form_partition(
    pivot_positions: &[usize],
    free_positions: &[usize],
    ambient_arity: usize,
) -> Result<bool, ResidualAffineIntegerSystemError> {
    if checked_add(
        "integer-system retained partition positions",
        pivot_positions.len(),
        free_positions.len(),
    )? != ambient_arity
        || !is_strictly_increasing(free_positions)
    {
        return Ok(false);
    }
    for (ordinal, &pivot) in pivot_positions.iter().enumerate() {
        if pivot >= ambient_arity
            || pivot_positions[..ordinal].contains(&pivot)
            || free_positions.contains(&pivot)
        {
            return Ok(false);
        }
    }
    Ok(free_positions
        .iter()
        .all(|&position| position < ambient_arity))
}

fn integer_system_arc_owned_logical_bytes<T>() -> Result<usize, ResidualAffineIntegerSystemError> {
    checked_add(
        "integer-system Arc owned logical bytes",
        checked_add(
            "integer-system Arc owned logical bytes",
            checked_mul(
                "integer-system Arc owned logical bytes",
                2,
                size_of::<usize>(),
            )?,
            align_of::<T>().saturating_sub(1),
        )?,
        size_of::<T>(),
    )
}

fn logical_bytes_for_bits(bits: usize) -> usize {
    bits / u8::BITS as usize + usize::from(bits % u8::BITS as usize != 0)
}

fn integer_system_gmp_logical_bytes_upper_bound(
    integer_entries: usize,
    total_integer_bits: usize,
) -> Result<usize, ResidualAffineIntegerSystemError> {
    checked_add(
        "integer-system GMP logical bytes",
        logical_bytes_for_bits(total_integer_bits),
        checked_add(
            "integer-system GMP logical bytes",
            checked_mul(
                "integer-system GMP logical bytes",
                integer_entries,
                size_of::<usize>(),
            )?,
            integer_entries.saturating_sub(1),
        )?,
    )
}

fn integer_system_largest_work_entry_bytes() -> usize {
    [
        size_of::<ResidualAffineIntegerSystemInputRow>(),
        size_of::<WorkRow>(),
        size_of::<SearchState>(),
        size_of::<ResidualAffineIntegerRowOperation>(),
        size_of::<ResidualAffineIntegerFinalRow>(),
        size_of::<Integer>(),
        size_of::<usize>(),
    ]
    .into_iter()
    .max()
    .unwrap_or(0)
}

fn integer_system_compilation_owned_logical_peak_upper_bound(
    transient: ResidualAffineIntegerSystemRawTransientCensus,
    retained_owned_logical_bytes_upper_bound: usize,
) -> Result<usize, ResidualAffineIntegerSystemError> {
    let transient_peak = checked_add(
        "integer-system compilation owned logical peak upper bound",
        integer_system_arc_owned_logical_bytes::<ResidualAffineIntegerSystemCertificate>()?,
        checked_add(
            "integer-system compilation owned logical peak upper bound",
            checked_mul(
                "integer-system compilation work-entry bytes",
                transient.allocation_entries_reserved,
                integer_system_largest_work_entry_bytes(),
            )?,
            integer_system_gmp_logical_bytes_upper_bound(
                transient.integer_bit_work,
                transient.integer_bit_work,
            )?,
        )?,
    )?;
    Ok(transient_peak.max(retained_owned_logical_bytes_upper_bound))
}

/// V2 parent seam for independently reauthenticating a nested fresh attempt.
/// The raw transient census is the replayable basis even when DFS exits as an
/// unsupported general-congruence case and no child certificate exists.
pub(crate) fn residual_affine_integer_system_compilation_owned_logical_peak_from_census(
    transient: ResidualAffineIntegerSystemRawTransientCensus,
    retained_owned_logical_bytes_upper_bound: usize,
) -> Result<usize, ResidualAffineIntegerSystemError> {
    integer_system_compilation_owned_logical_peak_upper_bound(
        transient,
        retained_owned_logical_bytes_upper_bound,
    )
}

/// Conservative prospective memory from the immutable V1 hard limits.
pub(crate) fn residual_affine_integer_system_memory_envelope_from_limits(
    limits: ResidualAffineIntegerSystemLimits,
) -> Result<ResidualAffineIntegerSystemMemoryEnvelope, ResidualAffineIntegerSystemError> {
    let row_headers = checked_mul(
        "integer-system retained input-row headers",
        checked_add(
            "integer-system retained input-row headers",
            limits.max_input_rows,
            limits.max_canonical_rows,
        )?,
        size_of::<ResidualAffineIntegerSystemInputRow>(),
    )?;
    let retained_integer_slots = checked_mul(
        "integer-system retained integer slots",
        checked_mul(
            "integer-system retained integer slots",
            3,
            limits.max_input_components,
        )?,
        size_of::<Integer>(),
    )?;
    let input_lineage_bytes = checked_mul(
        "integer-system retained input lineage bytes",
        checked_mul(
            "integer-system retained input lineage bytes",
            2,
            limits.max_input_lineage_ordinals,
        )?,
        size_of::<usize>(),
    )?;
    let operation_bytes = checked_mul(
        "integer-system retained row-operation bytes",
        limits.max_row_operations,
        size_of::<ResidualAffineIntegerRowOperation>(),
    )?;
    let final_row_headers = checked_mul(
        "integer-system retained final-row headers",
        limits.max_canonical_rows,
        size_of::<ResidualAffineIntegerFinalRow>(),
    )?;
    let derived_lineage_bytes = checked_mul(
        "integer-system retained derived-lineage bytes",
        checked_mul(
            "integer-system retained derived-lineage bytes",
            2,
            limits.max_lineage_entries_materialized,
        )?,
        size_of::<usize>(),
    )?;
    let position_bytes = checked_mul(
        "integer-system retained position bytes",
        checked_mul(
            "integer-system retained position bytes",
            2,
            limits.max_ambient_arity,
        )?,
        size_of::<usize>(),
    )?;
    let map_integer_bytes = checked_mul(
        "integer-system retained map integer bytes",
        limits.max_map_entries,
        size_of::<Integer>(),
    )?;
    let retained_gmp_entries = checked_add(
        "integer-system retained GMP entries",
        checked_add(
            "integer-system retained GMP entries",
            checked_mul(
                "integer-system retained GMP entries",
                3,
                limits.max_input_components,
            )?,
            limits.max_operation_integer_entries,
        )?,
        checked_add(
            "integer-system retained GMP entries",
            limits.max_map_entries,
            3,
        )?,
    )?;
    let retained_gmp_bytes = integer_system_gmp_logical_bytes_upper_bound(
        retained_gmp_entries,
        limits.max_integer_bit_work,
    )?;
    let retained_owned_logical_bytes_upper_bound = [
        integer_system_arc_owned_logical_bytes::<ResidualAffineIntegerSystemCertificate>()?,
        row_headers,
        retained_integer_slots,
        input_lineage_bytes,
        operation_bytes,
        final_row_headers,
        derived_lineage_bytes,
        position_bytes,
        map_integer_bytes,
        retained_gmp_bytes,
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| {
        checked_add(
            "integer-system retained owned logical bytes upper bound",
            sum,
            value,
        )
    })?;
    let compilation_owned_logical_peak_upper_bound =
        integer_system_compilation_owned_logical_peak_upper_bound(
            ResidualAffineIntegerSystemRawTransientCensus {
                allocation_entries_reserved: limits.max_allocation_entries_reserved,
                state_entries_materialized: limits.max_state_entries_materialized,
                integer_bit_work: limits.max_integer_bit_work,
                frontier_states_peak: limits.max_frontier_states,
            },
            retained_owned_logical_bytes_upper_bound,
        )?;
    Ok(ResidualAffineIntegerSystemMemoryEnvelope {
        retained_owned_logical_bytes_upper_bound,
        compilation_owned_logical_peak_upper_bound,
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ResidualAffineIntegerSystemRetainedShapeCensus {
    input_row_headers: usize,
    final_row_headers: usize,
    operation_entries: usize,
    slotted_integer_entries: usize,
    operation_integer_entries: usize,
    witness_integer_entries: usize,
    lineage_entries: usize,
    position_entries: usize,
    total_integer_bits: usize,
    large_integer_payload_bytes: usize,
}

impl ResidualAffineIntegerSystemRetainedShapeCensus {
    fn observe_integer(
        &mut self,
        value: &Integer,
        inline_slot: bool,
        operation_integer: bool,
        witness_integer: bool,
    ) -> Result<(), ResidualAffineIntegerSystemError> {
        let bits = integer_magnitude_bits(value)?;
        self.total_integer_bits = checked_add(
            "integer-system retained integer bits",
            self.total_integer_bits,
            bits,
        )?;
        if inline_slot {
            self.slotted_integer_entries = checked_add(
                "integer-system retained slotted integers",
                self.slotted_integer_entries,
                1,
            )?;
        }
        if operation_integer {
            self.operation_integer_entries = checked_add(
                "integer-system retained operation integers",
                self.operation_integer_entries,
                1,
            )?;
        }
        if witness_integer {
            self.witness_integer_entries = checked_add(
                "integer-system retained witness integers",
                self.witness_integer_entries,
                1,
            )?;
        }
        if matches!(value, Integer::Large(_)) {
            self.large_integer_payload_bytes = checked_add(
                "integer-system retained large-integer payload bytes",
                self.large_integer_payload_bytes,
                checked_add(
                    "integer-system retained large-integer payload bytes",
                    logical_bytes_for_bits(bits),
                    size_of::<usize>(),
                )?,
            )?;
        }
        Ok(())
    }

    fn total_integer_entries(self) -> Result<usize, ResidualAffineIntegerSystemError> {
        checked_add(
            "integer-system retained integer entries",
            self.slotted_integer_entries,
            checked_add(
                "integer-system retained integer entries",
                self.operation_integer_entries,
                self.witness_integer_entries,
            )?,
        )
    }

    fn heap_entry_units(self) -> Result<usize, ResidualAffineIntegerSystemError> {
        [
            checked_mul(
                "integer-system payload comparison units",
                self.input_row_headers,
                scalar_representation_units::<ResidualAffineIntegerSystemInputRow>(),
            )?,
            checked_mul(
                "integer-system payload comparison units",
                self.final_row_headers,
                scalar_representation_units::<ResidualAffineIntegerFinalRow>(),
            )?,
            checked_mul(
                "integer-system payload comparison units",
                self.operation_entries,
                scalar_representation_units::<ResidualAffineIntegerRowOperation>(),
            )?,
            checked_mul(
                "integer-system payload comparison units",
                self.total_integer_entries()?,
                scalar_representation_units::<Integer>(),
            )?,
            self.lineage_entries,
            self.position_entries,
        ]
        .into_iter()
        .try_fold(0usize, |sum, value| {
            checked_add("integer-system payload comparison units", sum, value)
        })
    }
}

fn bounded_shape_add(
    resource: &'static str,
    current: usize,
    additional: usize,
    limit: usize,
) -> Result<usize, ResidualAffineIntegerSystemError> {
    let requested = checked_add(resource, current, additional)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn census_retained_input_rows(
    rows: &[ResidualAffineIntegerSystemInputRow],
    expected_component_width: usize,
    component_limit: usize,
    lineage_limit: usize,
    shape: &mut ResidualAffineIntegerSystemRetainedShapeCensus,
) -> Result<(), ResidualAffineIntegerSystemError> {
    let mut components = 0usize;
    let mut lineages = 0usize;
    for row in rows {
        if row.row.components().len() != expected_component_width {
            return Err(
                ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                    "retained input-row width differs from ambient arity",
                ),
            );
        }
        components = bounded_shape_add(
            "input components",
            components,
            row.row.components().len(),
            component_limit,
        )?;
        lineages = bounded_shape_add(
            "input lineage ordinals",
            lineages,
            row.structural_locus_ordinals.len(),
            lineage_limit,
        )?;
        if row.structural_locus_ordinals.is_empty()
            || !is_strictly_increasing(&row.structural_locus_ordinals)
        {
            return Err(
                ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                    "retained input lineage is not canonical",
                ),
            );
        }
        for integer in row.row.components() {
            shape.observe_integer(integer, true, false, false)?;
        }
    }
    shape.lineage_entries = checked_add(
        "integer-system retained lineage entries",
        shape.lineage_entries,
        lineages,
    )?;
    Ok(())
}

fn census_retained_operation(
    operation: &ResidualAffineIntegerRowOperation,
    shape: &mut ResidualAffineIntegerSystemRetainedShapeCensus,
) -> Result<(), ResidualAffineIntegerSystemError> {
    match operation {
        ResidualAffineIntegerRowOperation::Swap { .. }
        | ResidualAffineIntegerRowOperation::Negate { .. } => Ok(()),
        ResidualAffineIntegerRowOperation::BezoutPair {
            pivot_coefficient,
            other_coefficient,
            gcd,
            pivot_bezout,
            other_bezout,
            ..
        } => {
            for integer in [
                pivot_coefficient,
                other_coefficient,
                gcd,
                pivot_bezout,
                other_bezout,
            ] {
                shape.observe_integer(integer, false, true, false)?;
            }
            Ok(())
        }
        ResidualAffineIntegerRowOperation::Eliminate { multiple, .. } => {
            shape.observe_integer(multiple, false, true, false)
        }
        ResidualAffineIntegerRowOperation::ExactNormalize { divisor, .. } => {
            shape.observe_integer(divisor, false, true, false)
        }
    }
}

fn integer_system_retained_shape_census(
    certificate: &ResidualAffineIntegerSystemCertificate,
) -> Result<ResidualAffineIntegerSystemRetainedShapeCensus, ResidualAffineIntegerSystemError> {
    if certificate.schema != RESIDUAL_AFFINE_INTEGER_SYSTEM_V1_SCHEMA {
        return Err(ResidualAffineIntegerSystemError::SchemaMismatch);
    }
    let limits = certificate.limits;
    check_limit(
        "ambient arity",
        certificate.ambient_arity,
        limits.max_ambient_arity,
    )?;
    check_limit(
        "input rows",
        certificate.replay_source_rows.len(),
        limits.max_input_rows,
    )?;
    check_limit(
        "canonical rows",
        certificate.source_rows.len(),
        limits.max_canonical_rows,
    )?;
    check_limit(
        "row operations",
        certificate.operations.len(),
        limits.max_row_operations,
    )?;
    check_limit(
        "canonical rows",
        certificate.final_rows.len(),
        limits.max_canonical_rows,
    )?;

    let component_width = checked_add(
        "integer-system retained component width",
        certificate.ambient_arity,
        1,
    )?;
    let mut shape = ResidualAffineIntegerSystemRetainedShapeCensus {
        input_row_headers: checked_add(
            "integer-system retained input-row headers",
            certificate.replay_source_rows.len(),
            certificate.source_rows.len(),
        )?,
        final_row_headers: certificate.final_rows.len(),
        operation_entries: certificate.operations.len(),
        ..ResidualAffineIntegerSystemRetainedShapeCensus::default()
    };
    census_retained_input_rows(
        &certificate.replay_source_rows,
        component_width,
        limits.max_input_components,
        limits.max_input_lineage_ordinals,
        &mut shape,
    )?;
    census_retained_input_rows(
        &certificate.source_rows,
        component_width,
        limits.max_input_components,
        limits.max_input_lineage_ordinals,
        &mut shape,
    )?;

    for operation in &certificate.operations {
        census_retained_operation(operation, &mut shape)?;
        check_limit(
            "operation integer entries",
            shape.operation_integer_entries,
            limits.max_operation_integer_entries,
        )?;
    }

    let mut final_components = 0usize;
    let mut final_lineages = 0usize;
    for row in &certificate.final_rows {
        if row.components.len() != component_width {
            return Err(
                ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                    "retained final-row width differs from ambient arity",
                ),
            );
        }
        final_components = bounded_shape_add(
            "input components",
            final_components,
            row.components.len(),
            limits.max_input_components,
        )?;
        final_lineages = bounded_shape_add(
            "lineage entries materialized",
            final_lineages,
            row.structural_locus_ordinals.len(),
            limits.max_lineage_entries_materialized,
        )?;
        for integer in &row.components {
            shape.observe_integer(integer, true, false, false)?;
        }
    }
    shape.lineage_entries = checked_add(
        "integer-system retained lineage entries",
        shape.lineage_entries,
        final_lineages,
    )?;

    let outer_positions = checked_add(
        "integer-system retained outer positions",
        certificate.pivot_positions.len(),
        certificate.free_positions.len(),
    )?;
    if outer_positions != certificate.ambient_arity
        || !retained_position_slices_form_partition(
            &certificate.pivot_positions,
            &certificate.free_positions,
            certificate.ambient_arity,
        )?
    {
        return Err(
            ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                "retained outer positions do not partition ambient coordinates",
            ),
        );
    }
    shape.position_entries = outer_positions;

    if let Some(map) = &certificate.affine_map {
        if map.ambient_arity != certificate.ambient_arity
            || map.constants.len() != certificate.ambient_arity
            || map.linear_coefficients.len()
                != checked_mul(
                    "integer-system retained map entries",
                    certificate.ambient_arity,
                    certificate.ambient_arity,
                )?
        {
            return Err(
                ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                    "retained affine-map dimensions differ from ambient arity",
                ),
            );
        }
        let map_integer_entries = checked_add(
            "map entries",
            map.constants.len(),
            map.linear_coefficients.len(),
        )?;
        check_limit("map entries", map_integer_entries, limits.max_map_entries)?;
        for integer in map.constants.iter().chain(&map.linear_coefficients) {
            shape.observe_integer(integer, true, false, false)?;
        }
        let map_positions = checked_add(
            "integer-system retained map positions",
            map.pivot_positions.len(),
            map.free_positions.len(),
        )?;
        if map_positions != certificate.ambient_arity
            || map.pivot_positions != certificate.pivot_positions
            || map.free_positions != certificate.free_positions
            || !retained_position_slices_form_partition(
                &map.pivot_positions,
                &map.free_positions,
                certificate.ambient_arity,
            )?
        {
            return Err(
                ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                    "retained map positions do not partition ambient coordinates",
                ),
            );
        }
        shape.position_entries = checked_add(
            "integer-system retained position entries",
            shape.position_entries,
            map_positions,
        )?;
    }

    if let Some(witness) = &certificate.empty_witness {
        let lineage = match witness {
            ResidualAffineIntegerEmptyWitness::ZeroEqualsNonzero {
                constant,
                structural_locus_ordinals,
                ..
            } => {
                shape.observe_integer(constant, false, false, true)?;
                structural_locus_ordinals
            }
            ResidualAffineIntegerEmptyWitness::CoefficientGcdDoesNotDivideConstant {
                constant,
                coefficient_gcd,
                remainder,
                structural_locus_ordinals,
                ..
            } => {
                for integer in [constant, coefficient_gcd, remainder] {
                    shape.observe_integer(integer, false, false, true)?;
                }
                structural_locus_ordinals
            }
        };
        check_limit(
            "lineage entries materialized",
            lineage.len(),
            limits.max_lineage_entries_materialized,
        )?;
        shape.lineage_entries = checked_add(
            "integer-system retained lineage entries",
            shape.lineage_entries,
            lineage.len(),
        )?;
    }
    Ok(shape)
}

fn integer_system_retained_owned_logical_bytes_upper_bound(
    certificate: &ResidualAffineIntegerSystemCertificate,
) -> Result<usize, ResidualAffineIntegerSystemError> {
    let shape = integer_system_retained_shape_census(certificate)?;
    [
        integer_system_arc_owned_logical_bytes::<ResidualAffineIntegerSystemCertificate>()?,
        checked_mul(
            "integer-system retained input-row headers",
            shape.input_row_headers,
            size_of::<ResidualAffineIntegerSystemInputRow>(),
        )?,
        checked_mul(
            "integer-system retained final-row headers",
            shape.final_row_headers,
            size_of::<ResidualAffineIntegerFinalRow>(),
        )?,
        checked_mul(
            "integer-system retained operations",
            shape.operation_entries,
            size_of::<ResidualAffineIntegerRowOperation>(),
        )?,
        checked_mul(
            "integer-system retained slotted integers",
            shape.slotted_integer_entries,
            size_of::<Integer>(),
        )?,
        checked_mul(
            "integer-system retained lineage entries",
            shape.lineage_entries,
            size_of::<usize>(),
        )?,
        checked_mul(
            "integer-system retained position entries",
            shape.position_entries,
            size_of::<usize>(),
        )?,
        shape.large_integer_payload_bytes,
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| {
        checked_add(
            "integer-system retained owned logical bytes upper bound",
            sum,
            value,
        )
    })
}

fn scalar_representation_units<T>() -> usize {
    let bytes = size_of::<T>();
    let word = size_of::<usize>();
    bytes / word + usize::from(bytes % word != 0)
}

fn integer_system_payload_operand_census(
    certificate: &ResidualAffineIntegerSystemCertificate,
    total: &mut ResidualAffineIntegerSystemPayloadComparisonCensus,
) -> Result<(), ResidualAffineIntegerSystemError> {
    let shape = integer_system_retained_shape_census(certificate)?;
    let retained = integer_system_retained_owned_logical_bytes_upper_bound(certificate)?;
    let units = checked_add(
        "integer-system payload comparison units",
        checked_add(
            "integer-system payload comparison units",
            scalar_representation_units::<ResidualAffineIntegerSystemCertificate>(),
            certificate.schema.len(),
        )?,
        shape.heap_entry_units()?,
    )?;
    total.units = checked_add(
        "integer-system payload comparison units",
        total.units,
        units,
    )?;
    total.bytes = checked_add(
        "integer-system payload comparison bytes",
        total.bytes,
        checked_add(
            "integer-system payload comparison bytes",
            retained,
            certificate.schema.len(),
        )?,
    )?;
    total.integer_bits = checked_add(
        "integer-system payload comparison integer bits",
        total.integer_bits,
        shape.total_integer_bits,
    )?;
    Ok(())
}

fn integer_system_equal_payload_comparison_census(
    certificate: &ResidualAffineIntegerSystemCertificate,
) -> Result<ResidualAffineIntegerSystemPayloadComparisonCensus, ResidualAffineIntegerSystemError> {
    let mut census = ResidualAffineIntegerSystemPayloadComparisonCensus::default();
    // Count two independently allocated operands. Calling the V1 comparator
    // on `(certificate, certificate)` would take pointer fast paths and omit
    // every deep integer and nested-container charge.
    integer_system_payload_operand_census(certificate, &mut census)?;
    integer_system_payload_operand_census(certificate, &mut census)?;
    Ok(census)
}

fn integer_magnitude_bits(value: &Integer) -> Result<usize, ResidualAffineIntegerSystemError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(
        |_| ResidualAffineIntegerSystemError::ResourceCountOverflow {
            resource: "integer coefficient bits",
        },
    )
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ResidualAffineIntegerSystemError> {
    if requested > limit {
        Err(ResidualAffineIntegerSystemError::ResourceLimit {
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
) -> Result<usize, ResidualAffineIntegerSystemError> {
    left.checked_add(right)
        .ok_or(ResidualAffineIntegerSystemError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ResidualAffineIntegerSystemError> {
    left.checked_mul(right)
        .ok_or(ResidualAffineIntegerSystemError::ResourceCountOverflow { resource })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(components: Vec<Integer>, lineage: &[usize]) -> ResidualAffineIntegerSystemInputRow {
        let row = ResidualAffinePrimitiveRow::try_from_canonical_components_with_limits(
            components, 64, 100_000, 10_000_000,
        )
        .expect("test row is canonical primitive");
        ResidualAffineIntegerSystemInputRow::try_new(row, lineage.to_vec(), 64)
            .expect("test lineage is valid")
    }

    fn ints(values: &[i64]) -> Vec<Integer> {
        values.iter().copied().map(Integer::from).collect()
    }

    fn zero_integer_system_limits() -> ResidualAffineIntegerSystemLimits {
        ResidualAffineIntegerSystemLimits {
            max_ambient_arity: 0,
            max_input_rows: 0,
            max_input_components: 0,
            max_input_lineage_ordinals: 0,
            max_canonical_rows: 0,
            max_canonical_comparisons: 0,
            max_prework_operations: 0,
            max_allocation_entries_reserved: 0,
            max_integer_coefficient_bits: 0,
            max_integer_bit_work: 0,
            max_lineage_operations: 0,
            max_lineage_entries_materialized: 0,
            max_dfs_states: 0,
            max_dfs_depth: 0,
            max_frontier_states: 0,
            max_state_entries_materialized: 0,
            max_search_operations: 0,
            max_euclidean_steps: 0,
            max_row_operations: 0,
            max_operation_integer_entries: 0,
            max_map_entries: 0,
            max_verification_operations: 0,
        }
    }

    fn fresh_complete(
        ambient_arity: usize,
        sources: &[ResidualAffineIntegerSystemInputRow],
        limits: ResidualAffineIntegerSystemLimits,
    ) -> ResidualAffineIntegerSystemFreshCompilation {
        match ResidualAffineIntegerSystemCertificate::compile_fresh(ambient_arity, sources, limits)
            .unwrap()
        {
            ResidualAffineIntegerSystemFreshCompilationAttempt::Complete(fresh) => fresh,
            ResidualAffineIntegerSystemFreshCompilationAttempt::Unsupported(unsupported) => {
                panic!(
                    "expected complete fresh compilation, got {:?}",
                    unsupported.reason()
                )
            }
        }
    }

    fn translated_point_map_certificate() -> ResidualAffineIntegerSystemCertificate {
        ResidualAffineIntegerSystemCertificate::compile(
            2,
            &[input(ints(&[2, 1, -3]), &[4])],
            ResidualAffineIntegerSystemLimits::default(),
        )
        .expect("translated point-membership fixture must compile")
    }

    fn large_coefficient_map_certificate() -> ResidualAffineIntegerSystemCertificate {
        ResidualAffineIntegerSystemCertificate::compile(
            2,
            &[input(
                vec![Integer::zero(), Integer::one(), Integer::from(u128::MAX)],
                &[7],
            )],
            ResidualAffineIntegerSystemLimits::default(),
        )
        .expect("large-coefficient point-membership fixture must compile")
    }

    fn index_combinations(length: usize, choose: usize) -> Vec<Vec<usize>> {
        fn extend(
            length: usize,
            choose: usize,
            start: usize,
            prefix: &mut Vec<usize>,
            result: &mut Vec<Vec<usize>>,
        ) {
            if prefix.len() == choose {
                result.push(prefix.clone());
                return;
            }
            let needed = choose - prefix.len();
            if needed > length.saturating_sub(start) {
                return;
            }
            let last = length - needed;
            for value in start..=last {
                prefix.push(value);
                extend(length, choose, value + 1, prefix, result);
                prefix.pop();
            }
        }

        if choose > length {
            return Vec::new();
        }
        let mut result = Vec::new();
        extend(length, choose, 0, &mut Vec::new(), &mut result);
        result
    }

    fn determinant_i64(matrix: &[Vec<i64>]) -> i64 {
        match matrix.len() {
            0 => 1,
            1 => matrix[0][0],
            width => {
                let mut determinant = 0i64;
                for column in 0..width {
                    let minor = matrix[1..]
                        .iter()
                        .map(|row| {
                            row.iter()
                                .enumerate()
                                .filter_map(|(ordinal, value)| {
                                    (ordinal != column).then_some(*value)
                                })
                                .collect::<Vec<_>>()
                        })
                        .collect::<Vec<_>>();
                    let signed = if column % 2 == 0 {
                        matrix[0][column]
                    } else {
                        -matrix[0][column]
                    };
                    determinant += signed * determinant_i64(&minor);
                }
                determinant
            }
        }
    }

    fn minors_of_size(coefficients: &[Vec<i64>], arity: usize, size: usize) -> Vec<i64> {
        let mut minors = Vec::new();
        for row_ordinals in index_combinations(coefficients.len(), size) {
            for columns in index_combinations(arity, size) {
                let matrix = row_ordinals
                    .iter()
                    .map(|&row| {
                        columns
                            .iter()
                            .map(|&column| coefficients[row][column])
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                minors.push(determinant_i64(&matrix));
            }
        }
        minors
    }

    fn has_full_rank_unit_original_minor(coefficients: &[Vec<i64>], arity: usize) -> bool {
        let mut rank = 0usize;
        for size in 1..=coefficients.len().min(arity) {
            if minors_of_size(coefficients, arity, size)
                .into_iter()
                .any(|minor| minor != 0)
            {
                rank = size;
            }
        }
        rank == 0
            || minors_of_size(coefficients, arity, rank)
                .into_iter()
                .any(|minor| minor.abs() == 1)
    }

    fn small_primitive_homogeneous_rows(arity: usize) -> Vec<Vec<i64>> {
        let candidate_count = 3usize.pow(u32::try_from(arity).unwrap());
        let mut rows = Vec::new();
        for mut encoded in 0..candidate_count {
            let mut coefficients = Vec::with_capacity(arity);
            for _ in 0..arity {
                coefficients.push(i64::try_from(encoded % 3).unwrap() - 1);
                encoded /= 3;
            }
            let Some(first_nonzero) = coefficients.iter().copied().find(|value| *value != 0) else {
                continue;
            };
            if first_nonzero > 0 {
                rows.push(coefficients);
            }
        }
        rows.sort();
        rows
    }

    fn apply_test_map(map: &ResidualAffineIntegerMap, point: &[Integer]) -> Vec<Integer> {
        (0..map.ambient_arity())
            .map(|row| {
                let mut image = map.constant(row).unwrap().clone();
                for (column, coordinate) in point.iter().enumerate() {
                    let product = map.linear_coefficient(row, column).unwrap() * coordinate;
                    image = &image + &product;
                }
                image
            })
            .collect()
    }

    fn homogeneous_row_vanishes(coefficients: &[i64], point: &[Integer]) -> bool {
        let mut image = Integer::zero();
        for (&coefficient, coordinate) in coefficients.iter().zip(point) {
            let product = &Integer::from(coefficient) * coordinate;
            image = &image + &product;
        }
        image.is_zero()
    }

    fn assert_small_map_projection(map: &ResidualAffineIntegerMap, coefficients: &[Vec<i64>]) {
        let arity = map.ambient_arity();
        let point_count = 3usize.pow(u32::try_from(arity).unwrap());
        for mut encoded in 0..point_count {
            let mut point = Vec::with_capacity(arity);
            for _ in 0..arity {
                point.push(Integer::from(i64::try_from(encoded % 3).unwrap() - 1));
                encoded /= 3;
            }
            let image = apply_test_map(map, &point);
            assert!(
                coefficients
                    .iter()
                    .all(|row| homogeneous_row_vanishes(row, &image)),
                "map image does not solve {coefficients:?} at source point {point:?}"
            );
            assert_eq!(
                apply_test_map(map, &image),
                image,
                "map is not a fixed-point projection for {coefficients:?}"
            );
            if coefficients
                .iter()
                .all(|row| homogeneous_row_vanishes(row, &point))
            {
                assert_eq!(
                    apply_test_map(map, &point),
                    point,
                    "map does not fix a source solution of {coefficients:?}"
                );
            }
        }
    }

    #[test]
    fn empty_system_is_the_identity_map() {
        let certificate = ResidualAffineIntegerSystemCertificate::compile(
            3,
            &[],
            ResidualAffineIntegerSystemLimits::default(),
        )
        .unwrap();
        assert_eq!(
            certificate.outcome(),
            ResidualAffineIntegerSystemOutcome::AffineMap
        );
        let map = certificate.affine_map().unwrap();
        assert!(map.pivot_positions().is_empty());
        assert_eq!(map.free_positions(), &[0, 1, 2]);
        for row in 0..3 {
            assert_eq!(map.constant(row), Some(&Integer::zero()));
            for column in 0..3 {
                assert_eq!(
                    map.linear_coefficient(row, column),
                    Some(&Integer::from(usize::from(row == column)))
                );
            }
        }
        certificate.replay().unwrap();
    }

    #[test]
    fn strict_zero_frontier_limit_rejects_the_initial_state() {
        let mut limits = ResidualAffineIntegerSystemLimits::default();
        limits.max_frontier_states = 0;
        assert!(matches!(
            ResidualAffineIntegerSystemCertificate::compile(3, &[], limits),
            Err(ResidualAffineIntegerSystemError::ResourceLimit {
                resource: "frontier states",
                requested: 1,
                limit: 0,
            })
        ));
    }

    #[test]
    fn deep_integer_equality_is_prospectively_censused() {
        let huge: Integer = "1361129467683753853853498429727072845947".parse().unwrap();
        let huge_copy = huge.clone();
        let comparison_bits = integer_magnitude_bits(&huge).unwrap().max(1);

        let mut exact_limits = ResidualAffineIntegerSystemLimits::default();
        exact_limits.max_integer_bit_work = comparison_bits;
        exact_limits.max_verification_operations = 1;
        let mut exact = Budget::new(exact_limits, 0);
        assert!(verify_integer_equal(&huge, &huge_copy, &mut exact).unwrap());
        assert_eq!(exact.stats.integer_bit_work(), comparison_bits);
        assert_eq!(exact.stats.verification_operations(), 1);

        let mut bit_limited = exact_limits;
        bit_limited.max_integer_bit_work = comparison_bits - 1;
        let mut bit_budget = Budget::new(bit_limited, 0);
        assert!(matches!(
            verify_integer_equal(&huge, &huge_copy, &mut bit_budget),
            Err(ResidualAffineIntegerSystemError::ResourceLimit {
                resource: "integer bit work",
                requested,
                limit,
            }) if requested == comparison_bits && limit == comparison_bits - 1
        ));
        assert_eq!(bit_budget.stats.integer_bit_work(), 0);
        assert_eq!(bit_budget.stats.verification_operations(), 1);

        let mut verification_limited = exact_limits;
        verification_limited.max_verification_operations = 0;
        let mut verification_budget = Budget::new(verification_limited, 0);
        assert!(matches!(
            verify_integer_equal(&huge, &huge_copy, &mut verification_budget),
            Err(ResidualAffineIntegerSystemError::ResourceLimit {
                resource: "verification operations",
                requested: 1,
                limit: 0,
            })
        ));
        assert_eq!(verification_budget.stats.integer_bit_work(), 0);
        assert_eq!(verification_budget.stats.verification_operations(), 0);
    }

    #[test]
    fn late_canonical_row_comparison_is_fallible_and_exactly_censused() {
        let huge: Integer = "1361129467683753853853498429727072845947".parse().unwrap();
        let left = input(
            vec![Integer::zero(), huge.clone(), huge.clone(), Integer::one()],
            &[0],
        )
        .row
        .clone();
        let right = input(
            vec![Integer::zero(), huge.clone(), huge, Integer::from(2)],
            &[1],
        )
        .row
        .clone();

        let baseline_limits = ResidualAffineIntegerSystemLimits::default();
        let mut baseline = Budget::new(baseline_limits, 3);
        assert_eq!(
            canonical_primitive_row_cmp(&left, &right, &mut baseline).unwrap(),
            Ordering::Less
        );
        let bit_work = baseline.stats.integer_bit_work();
        assert!(bit_work > 128);
        assert_eq!(baseline.stats.canonical_comparisons(), 1);

        let mut exact_limits = baseline_limits;
        exact_limits.max_canonical_comparisons = 1;
        exact_limits.max_integer_bit_work = bit_work;
        let mut exact = Budget::new(exact_limits, 3);
        assert_eq!(
            canonical_primitive_row_cmp(&left, &right, &mut exact).unwrap(),
            Ordering::Less
        );
        assert_eq!(exact.stats.canonical_comparisons(), 1);
        assert_eq!(exact.stats.integer_bit_work(), bit_work);

        let mut comparison_limited = exact_limits;
        comparison_limited.max_canonical_comparisons = 0;
        let mut comparison_budget = Budget::new(comparison_limited, 3);
        assert!(matches!(
            canonical_primitive_row_cmp(&left, &right, &mut comparison_budget),
            Err(ResidualAffineIntegerSystemError::ResourceLimit {
                resource: "canonical comparisons",
                requested: 1,
                limit: 0,
            })
        ));
        assert_eq!(comparison_budget.stats.integer_bit_work(), 0);

        let mut bit_limited = exact_limits;
        bit_limited.max_integer_bit_work = bit_work - 1;
        let mut bit_budget = Budget::new(bit_limited, 3);
        assert!(matches!(
            canonical_primitive_row_cmp(&left, &right, &mut bit_budget),
            Err(ResidualAffineIntegerSystemError::ResourceLimit {
                resource: "integer bit work",
                ..
            })
        ));
        assert_eq!(bit_budget.stats.canonical_comparisons(), 1);
        assert!(bit_budget.stats.integer_bit_work() <= bit_work - 1);

        let duplicates = vec![
            input(left.components().to_vec(), &[2]),
            input(left.components().to_vec(), &[3]),
        ];
        let duplicate_certificate = ResidualAffineIntegerSystemCertificate::compile(
            3,
            &duplicates,
            ResidualAffineIntegerSystemLimits::default(),
        )
        .unwrap();
        assert_eq!(duplicate_certificate.source_rows().len(), 1);
        // One structural comparison sorts the adjacent rows and the second is
        // the formerly-derived deep equality in the dedup pass.
        assert_eq!(duplicate_certificate.stats().canonical_comparisons(), 2);
        let mut dedup_limited = ResidualAffineIntegerSystemLimits::default();
        dedup_limited.max_canonical_comparisons = 1;
        assert!(matches!(
            ResidualAffineIntegerSystemCertificate::compile(3, &duplicates, dedup_limited),
            Err(ResidualAffineIntegerSystemError::ResourceLimit {
                resource: "canonical comparisons",
                requested: 2,
                limit: 1,
            })
        ));
    }

    #[test]
    fn late_replay_payload_difference_is_fallible_and_exactly_censused() {
        let sources = vec![input(ints(&[0, 2, 1]), &[0])];
        let certificate = ResidualAffineIntegerSystemCertificate::compile(
            2,
            &sources,
            ResidualAffineIntegerSystemLimits::default(),
        )
        .unwrap();
        certificate.replay().unwrap();

        let mut tampered = certificate.clone();
        *tampered
            .affine_map
            .as_mut()
            .unwrap()
            .linear_coefficients
            .last_mut()
            .unwrap() = "1361129467683753853853498429727072845947".parse().unwrap();

        let baseline_limits = ResidualAffineIntegerSystemLimits::default();
        let mut baseline = Budget::new(baseline_limits, certificate.ambient_arity);
        assert!(
            !certificate
                .payload_eq_with_budget(&tampered, &mut baseline)
                .unwrap()
        );
        let bit_work = baseline.stats.integer_bit_work();
        let verification_operations = baseline.stats.verification_operations();
        assert!(bit_work > 128);
        assert!(verification_operations > 1);

        let mut exact_limits = baseline_limits;
        exact_limits.max_integer_bit_work = bit_work;
        exact_limits.max_verification_operations = verification_operations;
        let mut exact = Budget::new(exact_limits, certificate.ambient_arity);
        assert!(
            !certificate
                .payload_eq_with_budget(&tampered, &mut exact)
                .unwrap()
        );
        assert_eq!(exact.stats.integer_bit_work(), bit_work);
        assert_eq!(
            exact.stats.verification_operations(),
            verification_operations
        );

        let mut bit_limited = exact_limits;
        bit_limited.max_integer_bit_work = bit_work - 1;
        let mut bit_budget = Budget::new(bit_limited, certificate.ambient_arity);
        assert!(matches!(
            certificate.payload_eq_with_budget(&tampered, &mut bit_budget),
            Err(ResidualAffineIntegerSystemError::ResourceLimit {
                resource: "integer bit work",
                ..
            })
        ));

        let mut verification_limited = exact_limits;
        verification_limited.max_verification_operations = verification_operations - 1;
        let mut verification_budget = Budget::new(verification_limited, certificate.ambient_arity);
        assert!(matches!(
            certificate.payload_eq_with_budget(&tampered, &mut verification_budget),
            Err(ResidualAffineIntegerSystemError::ResourceLimit {
                resource: "verification operations",
                ..
            })
        ));

        let mut reflexive_limits = baseline_limits;
        reflexive_limits.max_integer_bit_work = 0;
        reflexive_limits.max_verification_operations = 1;
        let mut reflexive = Budget::new(reflexive_limits, certificate.ambient_arity);
        assert!(
            certificate
                .payload_eq_with_budget(&certificate, &mut reflexive)
                .unwrap()
        );
        assert_eq!(reflexive.stats.integer_bit_work(), 0);
        assert_eq!(reflexive.stats.verification_operations(), 1);

        assert_eq!(
            tampered.replay(),
            Err(ResidualAffineIntegerSystemError::ReplayMismatch)
        );
    }

    #[test]
    fn replay_comparison_phase_propagates_limits_and_boolean_payload_eq_fails_closed() {
        let mut limits = ResidualAffineIntegerSystemLimits::default();
        limits.max_verification_operations = 1;
        let certificate = ResidualAffineIntegerSystemCertificate::compile(0, &[], limits).unwrap();
        assert_eq!(certificate.stats().verification_operations(), 1);
        assert!(matches!(
            certificate.replay(),
            Err(ResidualAffineIntegerSystemError::ResourceLimit {
                resource: "verification operations",
                requested: 2,
                limit: 1,
            })
        ));
        assert!(!certificate.payload_eq(&certificate.clone()));
    }

    #[test]
    fn unit_original_minors_are_supported_and_all_small_maps_project() {
        let mut checked_systems = 0usize;
        let mut unit_minor_systems = 0usize;
        let mut supported_after_exact_normalization = 0usize;
        for arity in 1..=3 {
            let candidate_rows = small_primitive_homogeneous_rows(arity);
            for row_count in 0..=candidate_rows.len().min(3) {
                for selected in index_combinations(candidate_rows.len(), row_count) {
                    let coefficients = selected
                        .iter()
                        .map(|&ordinal| candidate_rows[ordinal].clone())
                        .collect::<Vec<_>>();
                    let sources = selected
                        .iter()
                        .enumerate()
                        .map(|(lineage, &ordinal)| {
                            let mut components = vec![Integer::zero()];
                            components
                                .extend(candidate_rows[ordinal].iter().copied().map(Integer::from));
                            input(components, &[lineage])
                        })
                        .collect::<Vec<_>>();
                    let has_unit_minor = has_full_rank_unit_original_minor(&coefficients, arity);
                    let compiled = ResidualAffineIntegerSystemCertificate::compile(
                        arity,
                        &sources,
                        ResidualAffineIntegerSystemLimits::default(),
                    );
                    match compiled {
                        Ok(certificate) => {
                            if has_unit_minor {
                                unit_minor_systems += 1;
                            } else {
                                // Exact primitive normalization can expose a
                                // later unit pivot even when the original
                                // coefficient matrix has no full-rank unit
                                // minor, e.g. rows (1,1) and (1,-1).
                                supported_after_exact_normalization += 1;
                            }
                            assert_eq!(
                                certificate.outcome(),
                                ResidualAffineIntegerSystemOutcome::AffineMap
                            );
                            assert_small_map_projection(
                                certificate.affine_map().unwrap(),
                                &coefficients,
                            );
                            certificate.replay().unwrap();
                        }
                        Err(ResidualAffineIntegerSystemError::Unsupported(
                            ResidualAffineIntegerSystemUnsupported::GeneralCongruenceCaseNotSupported {
                                ..
                            },
                        )) if !has_unit_minor => {}
                        Err(error) => {
                            panic!(
                                "small homogeneous system {coefficients:?} had unit_minor={has_unit_minor} but failed unexpectedly: {error}"
                            )
                        }
                    }
                    checked_systems += 1;
                }
            }
        }
        assert_eq!(checked_systems, 395);
        assert!(unit_minor_systems > 0);
        assert!(supported_after_exact_normalization > 0);
    }

    #[test]
    fn affine_map_retains_a_nonzero_translation_and_free_identity_row() {
        let sources = vec![input(ints(&[2, 1, -3]), &[4])];
        let certificate = ResidualAffineIntegerSystemCertificate::compile(
            2,
            &sources,
            ResidualAffineIntegerSystemLimits::default(),
        )
        .unwrap();
        let map = certificate.affine_map().unwrap();
        assert_eq!(map.pivot_positions(), &[0]);
        assert_eq!(map.free_positions(), &[1]);
        assert_eq!(map.constant(0), Some(&Integer::from(-2)));
        assert_eq!(map.constant(1), Some(&Integer::zero()));
        assert_eq!(map.linear_coefficient(0, 0), Some(&Integer::zero()));
        assert_eq!(map.linear_coefficient(0, 1), Some(&Integer::from(3)));
        assert_eq!(map.linear_coefficient(1, 0), Some(&Integer::zero()));
        assert_eq!(map.linear_coefficient(1, 1), Some(&Integer::one()));
        certificate.replay().unwrap();
    }

    #[test]
    fn residual_affine_integer_map_point_membership_is_exact_and_arity_checked() {
        let certificate = translated_point_map_certificate();
        let map = certificate.affine_map().unwrap();

        let (on_map, on_map_stats) = map
            .fixes_i64_point_with_limits(&[4, 2], ResidualAffineIntegerMapPointLimits::default())
            .unwrap();
        assert!(on_map, "(-2 + 3*2, 2) must fix (4, 2)");
        assert_eq!(on_map_stats.ambient_arity(), 2);
        assert_eq!(on_map_stats.matrix_entries_inspected(), 8);
        assert_eq!(on_map_stats.nonzero_multiplications(), 2);
        assert_eq!(on_map_stats.additions(), 2);
        assert_eq!(on_map_stats.fixed_point_comparisons(), 2);
        assert!(on_map_stats.peak_temporary_bytes() > 0);

        let (off_map, _) = map
            .fixes_i64_point_with_limits(&[5, 2], ResidualAffineIntegerMapPointLimits::default())
            .unwrap();
        assert!(!off_map, "(-2 + 3*2, 2) must not fix (5, 2)");

        assert_eq!(
            map.fixes_i64_point_with_limits(&[4], ResidualAffineIntegerMapPointLimits::default(),),
            Err(ResidualAffineIntegerMapPointError::ArityMismatch {
                expected: 2,
                actual: 1,
            })
        );
    }

    #[test]
    fn residual_affine_integer_map_point_membership_budgets_are_exact_and_one_below() {
        let certificate = translated_point_map_certificate();
        let map = certificate.affine_map().unwrap();
        let baseline_limits = ResidualAffineIntegerMapPointLimits::default();
        let (fixed, stats) = map
            .fixes_i64_point_with_limits(&[4, 2], baseline_limits)
            .unwrap();
        assert!(fixed);

        let exact_limits = ResidualAffineIntegerMapPointLimits {
            max_ambient_arity: stats.ambient_arity(),
            max_matrix_entries_inspected: stats.matrix_entries_inspected(),
            max_nonzero_multiplications: stats.nonzero_multiplications(),
            max_additions: stats.additions(),
            max_fixed_point_comparisons: stats.fixed_point_comparisons(),
            max_peak_temporary_bytes: stats.peak_temporary_bytes(),
            max_integer_bits: stats.largest_integer_bits(),
            max_integer_bit_work: stats.integer_bit_work(),
        };
        assert_eq!(
            map.fixes_i64_point_with_limits(&[4, 2], exact_limits),
            Ok((true, stats))
        );

        macro_rules! one_below {
            ($limit:ident, $stat:ident, $resource:literal) => {{
                let value = stats.$stat();
                assert!(value > 0, "{} census must be positive", stringify!($stat));
                let mut limits = exact_limits;
                limits.$limit = value - 1;
                assert!(matches!(
                    map.fixes_i64_point_with_limits(&[4, 2], limits),
                    Err(ResidualAffineIntegerMapPointError::ResourceLimit {
                        resource: $resource,
                        requested,
                        limit,
                    }) if requested == value && limit == value - 1
                ));
            }};
        }

        one_below!(max_ambient_arity, ambient_arity, "ambient arity");
        one_below!(
            max_matrix_entries_inspected,
            matrix_entries_inspected,
            "matrix entries inspected"
        );
        one_below!(
            max_nonzero_multiplications,
            nonzero_multiplications,
            "nonzero multiplications"
        );
        one_below!(max_additions, additions, "additions");
        one_below!(
            max_fixed_point_comparisons,
            fixed_point_comparisons,
            "fixed-point comparisons"
        );
        one_below!(
            max_peak_temporary_bytes,
            peak_temporary_bytes,
            "peak temporary bytes"
        );
        one_below!(max_integer_bits, largest_integer_bits, "integer bits");
        one_below!(max_integer_bit_work, integer_bit_work, "integer bit work");
    }

    #[test]
    fn residual_affine_integer_map_point_membership_large_gmp_stats_are_independent() {
        let certificate = large_coefficient_map_certificate();
        let map = certificate.affine_map().unwrap();
        assert!(matches!(
            map.linear_coefficient(0, 1),
            Some(Integer::Large(_))
        ));

        let (fixed, stats) = map
            .fixes_i64_point_with_limits(&[0, 1], ResidualAffineIntegerMapPointLimits::default())
            .unwrap();
        assert!(!fixed, "the large first coordinate cannot equal zero");
        assert_eq!(stats.ambient_arity(), 2);
        assert_eq!(stats.matrix_entries_inspected(), 8);
        assert_eq!(stats.nonzero_multiplications(), 2);
        assert_eq!(stats.additions(), 2);
        assert_eq!(stats.fixed_point_comparisons(), 2);
        assert_eq!(stats.largest_integer_bits(), 130);
        assert_eq!(stats.integer_bit_work(), 528);
        let expected_peak_temporary_bytes =
            4 * (std::mem::size_of::<Integer>() + 17 + std::mem::size_of::<usize>());
        assert_eq!(stats.peak_temporary_bytes(), expected_peak_temporary_bytes);
    }

    #[test]
    fn residual_affine_integer_map_point_membership_rejects_malformed_map_shapes() {
        let certificate = translated_point_map_certificate();
        let map = certificate.affine_map().unwrap();

        let mut malformed_translation = map.clone();
        malformed_translation.constants.pop();
        assert_eq!(
            malformed_translation.fixes_i64_point_with_limits(
                &[4, 2],
                ResidualAffineIntegerMapPointLimits::default(),
            ),
            Err(ResidualAffineIntegerMapPointError::MapInvariantFailure(
                "translation length differs from ambient arity"
            ))
        );

        let mut malformed_matrix = map.clone();
        malformed_matrix.linear_coefficients.pop();
        assert_eq!(
            malformed_matrix.fixes_i64_point_with_limits(
                &[4, 2],
                ResidualAffineIntegerMapPointLimits::default(),
            ),
            Err(ResidualAffineIntegerMapPointError::MapInvariantFailure(
                "matrix length differs from ambient square"
            ))
        );
    }

    #[test]
    fn residual_affine_integer_map_point_membership_handles_i64_min_exactly() {
        let certificate = ResidualAffineIntegerSystemCertificate::compile(
            1,
            &[],
            ResidualAffineIntegerSystemLimits::default(),
        )
        .expect("one-dimensional identity-map fixture must compile");
        let map = certificate.affine_map().unwrap();

        let (fixed, stats) = map
            .fixes_i64_point_with_limits(
                &[i64::MIN],
                ResidualAffineIntegerMapPointLimits::default(),
            )
            .unwrap();
        assert!(fixed);
        assert_eq!(stats.ambient_arity(), 1);
        assert_eq!(stats.matrix_entries_inspected(), 2);
        assert_eq!(stats.nonzero_multiplications(), 1);
        assert_eq!(stats.additions(), 1);
        assert_eq!(stats.fixed_point_comparisons(), 1);
        assert_eq!(stats.largest_integer_bits(), 66);
        assert_eq!(stats.integer_bit_work(), 262);
    }

    #[test]
    fn residual_affine_integer_map_point_membership_skips_zero_coordinate_products() {
        let certificate = large_coefficient_map_certificate();
        let map = certificate.affine_map().unwrap();

        let (fixed, stats) = map
            .fixes_i64_point_with_limits(&[0, 0], ResidualAffineIntegerMapPointLimits::default())
            .unwrap();
        assert!(fixed);
        assert_eq!(stats.ambient_arity(), 2);
        assert_eq!(stats.matrix_entries_inspected(), 8);
        assert_eq!(stats.nonzero_multiplications(), 0);
        assert_eq!(stats.additions(), 0);
        assert_eq!(stats.fixed_point_comparisons(), 2);
        assert_eq!(stats.largest_integer_bits(), 128);
        assert_eq!(stats.integer_bit_work(), 4);
    }

    #[test]
    fn duplicate_rows_merge_sorted_lineage_and_dependent_rows_normalize() {
        let sources = vec![
            input(ints(&[0, 1, 1]), &[9, 2]),
            input(ints(&[0, 1, -1]), &[8]),
            input(ints(&[0, 1, 1]), &[4, 2]),
        ];
        let certificate = ResidualAffineIntegerSystemCertificate::compile(
            2,
            &sources,
            ResidualAffineIntegerSystemLimits::default(),
        )
        .unwrap();
        assert_eq!(certificate.source_rows().len(), 2);
        let merged = certificate
            .source_rows()
            .iter()
            .find(|row| row.row().components() == ints(&[0, 1, 1]))
            .unwrap();
        assert_eq!(merged.structural_locus_ordinals(), &[2, 4, 9]);
        assert!(
            certificate
                .affine_map()
                .unwrap()
                .free_positions()
                .is_empty()
        );
        assert!(certificate.operations().iter().any(|operation| matches!(
            operation,
            ResidualAffineIntegerRowOperation::ExactNormalize { .. }
        )));
        certificate.replay().unwrap();
    }

    #[test]
    fn modular_conflict_and_zero_equals_nonzero_are_proved_empty() {
        let modular = vec![input(ints(&[1, 2]), &[0])];
        let certificate = ResidualAffineIntegerSystemCertificate::compile(
            1,
            &modular,
            ResidualAffineIntegerSystemLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            certificate.empty_witness(),
            Some(ResidualAffineIntegerEmptyWitness::CoefficientGcdDoesNotDivideConstant { .. })
        ));
        certificate.replay().unwrap();

        let contradictory = vec![input(ints(&[0, 1]), &[1]), input(ints(&[1, 1]), &[2])];
        let certificate = ResidualAffineIntegerSystemCertificate::compile(
            1,
            &contradictory,
            ResidualAffineIntegerSystemLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            certificate.empty_witness(),
            Some(ResidualAffineIntegerEmptyWitness::ZeroEqualsNonzero { .. })
        ));
        match certificate.empty_witness().unwrap() {
            ResidualAffineIntegerEmptyWitness::ZeroEqualsNonzero {
                structural_locus_ordinals,
                ..
            } => assert_eq!(structural_locus_ordinals, &[1, 2]),
            other => panic!("unexpected derived empty witness: {other:?}"),
        }
        certificate.replay().unwrap();
    }

    #[test]
    fn bezout_pair_creates_a_unit_pivot() {
        let sources = vec![input(ints(&[0, 2, 1]), &[3]), input(ints(&[0, 3, 1]), &[5])];
        let certificate = ResidualAffineIntegerSystemCertificate::compile(
            2,
            &sources,
            ResidualAffineIntegerSystemLimits::default(),
        )
        .unwrap();
        assert!(certificate.operations().iter().any(|operation| matches!(
            operation,
            ResidualAffineIntegerRowOperation::BezoutPair { column: 0, .. }
        )));
        assert!(
            certificate
                .affine_map()
                .unwrap()
                .free_positions()
                .is_empty()
        );
        certificate.replay().unwrap();
    }

    #[test]
    fn unit_column_is_accepted_and_no_unit_original_graph_is_typed_unsupported() {
        let accepted = vec![input(ints(&[0, 2, 1]), &[0])];
        let certificate = ResidualAffineIntegerSystemCertificate::compile(
            2,
            &accepted,
            ResidualAffineIntegerSystemLimits::default(),
        )
        .unwrap();
        assert_eq!(certificate.pivot_positions(), &[1]);
        assert_eq!(certificate.free_positions(), &[0]);

        let unsupported = vec![input(ints(&[0, 2, 3]), &[0])];
        let error = ResidualAffineIntegerSystemCertificate::compile(
            2,
            &unsupported,
            ResidualAffineIntegerSystemLimits::default(),
        )
        .unwrap_err();
        assert!(matches!(
            &error,
            ResidualAffineIntegerSystemError::Unsupported(
                ResidualAffineIntegerSystemUnsupported::GeneralCongruenceCaseNotSupported {
                    remaining_equations: 1
                }
            )
        ));
        assert!(
            error
                .to_string()
                .contains("no complete original-coordinate unit-pivot graph exists")
        );
    }

    #[test]
    fn deterministic_dfs_backtracks_from_the_first_eligible_column() {
        // Columns are (2,3), (1,0), (0,1).  Pivoting column zero leaves
        // coefficients 3 and 2, hence no second unit column.  Column one then
        // column two is the first complete DFS path.
        let sources = vec![
            input(ints(&[0, 2, 1, 0]), &[0]),
            input(ints(&[0, 3, 0, 1]), &[1]),
        ];
        let certificate = ResidualAffineIntegerSystemCertificate::compile(
            3,
            &sources,
            ResidualAffineIntegerSystemLimits::default(),
        )
        .unwrap();
        assert_eq!(certificate.pivot_positions(), &[1, 2]);
        assert_eq!(certificate.free_positions(), &[0]);
        assert!(certificate.stats().dfs_states() > 3);
        certificate.replay().unwrap();
    }

    #[test]
    fn retained_census_accepts_numerically_unsorted_dfs_pivot_order() {
        let sources = vec![input(ints(&[0, 2, 1]), &[0]), input(ints(&[0, 4, 3]), &[1])];
        let certificate = ResidualAffineIntegerSystemCertificate::compile(
            2,
            &sources,
            ResidualAffineIntegerSystemLimits::default(),
        )
        .unwrap();
        assert_eq!(certificate.pivot_positions(), &[1, 0]);
        assert!(certificate.free_positions().is_empty());
        assert_eq!(certificate.affine_map().unwrap().pivot_positions(), &[1, 0]);
        certificate.replay().unwrap();
        let retained_census = integer_system_retained_shape_census(&certificate).unwrap();

        let fresh = fresh_complete(2, &sources, ResidualAffineIntegerSystemLimits::default());
        assert!(fresh.certificate.payload_eq(&certificate));
        assert_eq!(
            integer_system_retained_shape_census(&fresh.certificate).unwrap(),
            retained_census
        );
        let exact_allocation = Arc::clone(&fresh.certificate);
        let (retained, authorization) = fresh.into_certificate_and_plan_authorization().unwrap();
        assert!(Arc::ptr_eq(&retained, &exact_allocation));
        assert!(authorization.authenticates_certificate_allocation(&retained));
        let authorized = authorization.into_authenticated_certificate_arc().unwrap();
        assert!(Arc::ptr_eq(&authorized, &retained));

        let mut mismatched_map_positions = certificate.clone();
        mismatched_map_positions
            .affine_map
            .as_mut()
            .unwrap()
            .pivot_positions
            .swap(0, 1);
        assert!(integer_system_retained_shape_census(&mismatched_map_positions).is_err());

        assert!(retained_position_slices_form_partition(&[1, 0], &[], 2).unwrap());
        assert!(!retained_position_slices_form_partition(&[1, 1], &[], 2).unwrap());
        assert!(!retained_position_slices_form_partition(&[2, 0], &[], 2).unwrap());
        assert!(!retained_position_slices_form_partition(&[1], &[1], 2).unwrap());
        assert!(!retained_position_slices_form_partition(&[], &[1, 0], 2).unwrap());
    }

    #[test]
    fn arbitrary_precision_coefficients_are_not_narrowed() {
        let huge: Integer = "1361129467683753853853498429727072845947".parse().unwrap();
        let sources = vec![input(
            vec![Integer::zero(), huge.clone(), Integer::one()],
            &[7],
        )];
        let certificate = ResidualAffineIntegerSystemCertificate::compile(
            2,
            &sources,
            ResidualAffineIntegerSystemLimits::default(),
        )
        .unwrap();
        let map = certificate.affine_map().unwrap();
        assert_eq!(map.linear_coefficient(1, 0), Some(&(-huge)));
        assert!(certificate.stats().largest_integer_coefficient_bits() > 128);
        certificate.replay().unwrap();
    }

    #[test]
    fn arbitrary_precision_bezout_pair_and_negative_signs_are_exact() {
        let huge: Integer = "1361129467683753853853498429727072845947".parse().unwrap();
        let huge_minus_one = &huge - &Integer::one();
        let huge_plus_one = &huge + &Integer::one();
        let sources = vec![
            input(vec![Integer::zero(), huge.clone(), huge_minus_one], &[10]),
            input(vec![Integer::zero(), huge_plus_one, huge.clone()], &[11]),
        ];
        let certificate = ResidualAffineIntegerSystemCertificate::compile(
            2,
            &sources,
            ResidualAffineIntegerSystemLimits::default(),
        )
        .unwrap();
        assert!(certificate.operations().iter().any(|operation| matches!(
            operation,
            ResidualAffineIntegerRowOperation::BezoutPair {
                column: 0,
                pivot_coefficient: Integer::Large(_),
                other_coefficient: Integer::Large(_),
                gcd,
                ..
            } if gcd.is_one()
        )));
        assert!(certificate.free_positions().is_empty());
        certificate.replay().unwrap();

        let negative_sources = vec![
            input(ints(&[1, -2, 1]), &[20]),
            input(ints(&[1, -3, 1]), &[21]),
        ];
        let negative = ResidualAffineIntegerSystemCertificate::compile(
            2,
            &negative_sources,
            ResidualAffineIntegerSystemLimits::default(),
        )
        .unwrap();
        assert!(negative.operations().iter().any(|operation| matches!(
            operation,
            ResidualAffineIntegerRowOperation::BezoutPair {
                column: 0,
                pivot_coefficient,
                other_coefficient,
                gcd,
                ..
            } if pivot_coefficient.is_negative()
                && other_coefficient.is_negative()
                && gcd.is_one()
        )));
        negative.replay().unwrap();
    }

    #[test]
    fn integer_bit_work_is_rejected_before_negation_and_division() {
        let mut limits = ResidualAffineIntegerSystemLimits::default();
        limits.max_integer_bit_work = 6;
        let mut budget = Budget::new(limits, 1);
        budget.observe_integer(&Integer::from(8)).unwrap(); // four bits
        budget.observe_integer(&Integer::from(2)).unwrap(); // two bits
        assert_eq!(budget.stats.integer_bit_work(), 6);
        assert!(matches!(
            budget.negate_integer(&Integer::from(8)),
            Err(ResidualAffineIntegerSystemError::ResourceLimit {
                resource: "integer bit work",
                ..
            })
        ));
        assert_eq!(budget.stats.integer_bit_work(), 6);
        assert_eq!(budget.stats.euclidean_steps(), 0);
        assert!(matches!(
            budget.quotient_remainder(&Integer::from(8), &Integer::from(2)),
            Err(ResidualAffineIntegerSystemError::ResourceLimit {
                resource: "integer bit work",
                ..
            })
        ));
        assert_eq!(budget.stats.integer_bit_work(), 6);
        assert_eq!(budget.stats.euclidean_steps(), 0);
    }

    #[test]
    fn every_positive_resource_census_rejects_one_below() {
        let sources = vec![
            input(ints(&[0, 2, 1, 0]), &[5, 1]),
            input(ints(&[0, 3, 0, 1]), &[8, 2]),
        ];
        let baseline_limits = ResidualAffineIntegerSystemLimits::default();
        let baseline =
            ResidualAffineIntegerSystemCertificate::compile(3, &sources, baseline_limits).unwrap();
        let stats = baseline.stats();

        macro_rules! one_below {
            ($limit:ident, $stat:ident) => {{
                let value = stats.$stat();
                assert!(value > 0, "{} census must be positive", stringify!($stat));
                let mut limits = baseline_limits;
                limits.$limit = value - 1;
                assert!(
                    matches!(
                        ResidualAffineIntegerSystemCertificate::compile(3, &sources, limits),
                        Err(ResidualAffineIntegerSystemError::ResourceLimit { .. })
                    ),
                    "{} did not reject one below its {} census",
                    stringify!($limit),
                    stringify!($stat)
                );
            }};
        }

        one_below!(max_ambient_arity, ambient_arity);
        one_below!(max_input_rows, input_rows);
        one_below!(max_input_components, input_components);
        one_below!(max_input_lineage_ordinals, input_lineage_ordinals);
        one_below!(max_canonical_rows, canonical_rows);
        one_below!(max_canonical_comparisons, canonical_comparisons);
        one_below!(max_prework_operations, prework_operations);
        one_below!(max_allocation_entries_reserved, allocation_entries_reserved);
        one_below!(
            max_integer_coefficient_bits,
            largest_integer_coefficient_bits
        );
        one_below!(max_integer_bit_work, integer_bit_work);
        one_below!(max_lineage_operations, lineage_operations);
        one_below!(
            max_lineage_entries_materialized,
            lineage_entries_materialized
        );
        one_below!(max_dfs_states, dfs_states);
        one_below!(max_dfs_depth, deepest_dfs_depth);
        one_below!(max_frontier_states, frontier_states_peak);
        one_below!(max_state_entries_materialized, state_entries_materialized);
        one_below!(max_search_operations, search_operations);
        one_below!(max_euclidean_steps, euclidean_steps);
        one_below!(max_row_operations, row_operations);
        one_below!(max_operation_integer_entries, operation_integer_entries);
        one_below!(max_map_entries, map_entries);
        one_below!(max_verification_operations, verification_operations);
    }

    #[test]
    fn fresh_memory_envelope_is_checked_exact_and_dominates_retained() {
        assert_eq!(logical_bytes_for_bits(0), 0);
        assert_eq!(logical_bytes_for_bits(1), 1);
        assert_eq!(logical_bytes_for_bits(8), 1);
        assert_eq!(logical_bytes_for_bits(9), 2);
        assert_eq!(
            integer_system_gmp_logical_bytes_upper_bound(1, 9).unwrap(),
            2 + size_of::<usize>()
        );
        let two_130_bit_values = integer_system_gmp_logical_bytes_upper_bound(2, 260).unwrap();
        assert_eq!(
            two_130_bit_values,
            logical_bytes_for_bits(260) + 2 * size_of::<usize>() + 1
        );
        assert_eq!(
            two_130_bit_values,
            2 * (logical_bytes_for_bits(130) + size_of::<usize>())
        );
        assert!(matches!(
            integer_system_gmp_logical_bytes_upper_bound(usize::MAX, 0),
            Err(ResidualAffineIntegerSystemError::ResourceCountOverflow {
                resource: "integer-system GMP logical bytes"
            })
        ));

        let zero = residual_affine_integer_system_memory_envelope_from_limits(
            zero_integer_system_limits(),
        )
        .unwrap();
        let arc =
            integer_system_arc_owned_logical_bytes::<ResidualAffineIntegerSystemCertificate>()
                .unwrap();
        let zero_retained = arc + integer_system_gmp_logical_bytes_upper_bound(3, 0).unwrap();
        assert_eq!(
            zero.retained_owned_logical_bytes_upper_bound(),
            zero_retained
        );
        assert_eq!(
            zero.compilation_owned_logical_peak_upper_bound(),
            zero_retained
        );

        let mut one = zero_integer_system_limits();
        one.max_ambient_arity = 1;
        one.max_input_rows = 1;
        one.max_input_components = 1;
        one.max_input_lineage_ordinals = 1;
        one.max_canonical_rows = 1;
        one.max_allocation_entries_reserved = 1;
        one.max_integer_bit_work = 9;
        one.max_lineage_entries_materialized = 1;
        one.max_frontier_states = 1;
        one.max_state_entries_materialized = 1;
        one.max_row_operations = 1;
        one.max_operation_integer_entries = 1;
        one.max_map_entries = 1;
        let one_envelope = residual_affine_integer_system_memory_envelope_from_limits(one).unwrap();
        let expected_retained = [
            arc,
            2 * size_of::<ResidualAffineIntegerSystemInputRow>(),
            3 * size_of::<Integer>(),
            2 * size_of::<usize>(),
            size_of::<ResidualAffineIntegerRowOperation>(),
            size_of::<ResidualAffineIntegerFinalRow>(),
            2 * size_of::<usize>(),
            2 * size_of::<usize>(),
            size_of::<Integer>(),
            integer_system_gmp_logical_bytes_upper_bound(8, 9).unwrap(),
        ]
        .into_iter()
        .sum::<usize>();
        assert_eq!(
            one_envelope.retained_owned_logical_bytes_upper_bound(),
            expected_retained
        );
        assert!(
            one_envelope.compilation_owned_logical_peak_upper_bound()
                >= one_envelope.retained_owned_logical_bytes_upper_bound()
        );

        let mut overflow = one;
        overflow.max_input_rows = usize::MAX;
        assert!(matches!(
            residual_affine_integer_system_memory_envelope_from_limits(overflow),
            Err(ResidualAffineIntegerSystemError::ResourceCountOverflow {
                resource: "integer-system retained input-row headers"
            })
        ));

        let large_130: Integer = "680564733841876926926749214863536422912".parse().unwrap();
        let large_131: Integer = "1361129467683753853853498429727072845824".parse().unwrap();
        assert!(matches!(&large_130, Integer::Large(_)));
        assert!(matches!(&large_131, Integer::Large(_)));
        assert_eq!(integer_magnitude_bits(&large_130).unwrap(), 130);
        assert_eq!(integer_magnitude_bits(&large_131).unwrap(), 131);
        let large_sources = vec![input(
            vec![Integer::zero(), Integer::one(), large_130, large_131],
            &[0],
        )];
        let large_fresh = fresh_complete(
            3,
            &large_sources,
            ResidualAffineIntegerSystemLimits::default(),
        );
        let large_shape = integer_system_retained_shape_census(&large_fresh.certificate).unwrap();
        let per_130_bit_payload = logical_bytes_for_bits(130) + size_of::<usize>();
        let per_131_bit_payload = logical_bytes_for_bits(131) + size_of::<usize>();
        // Replay source, canonical source, final row, and affine map each retain
        // one independent copy of both non-byte-aligned Large coefficients.
        assert_eq!(
            large_shape.large_integer_payload_bytes,
            4 * (per_130_bit_payload + per_131_bit_payload)
        );

        let large_v1 = ResidualAffineIntegerSystemCertificate::compile(
            3,
            &large_sources,
            ResidualAffineIntegerSystemLimits::default(),
        )
        .unwrap();
        let exact_integer_bit_work = large_v1.stats().integer_bit_work();
        assert!(exact_integer_bit_work > 0);
        let mut exact_large_limits = ResidualAffineIntegerSystemLimits::default();
        exact_large_limits.max_integer_bit_work = exact_integer_bit_work;
        let exact_large_fresh = fresh_complete(3, &large_sources, exact_large_limits);
        let exact_large_envelope =
            residual_affine_integer_system_memory_envelope_from_limits(exact_large_limits).unwrap();
        assert!(
            exact_large_fresh.retained_owned_logical_bytes_upper_bound()
                <= exact_large_envelope.retained_owned_logical_bytes_upper_bound()
        );
        assert!(
            exact_large_fresh.compilation_owned_logical_peak_upper_bound()
                <= exact_large_envelope.compilation_owned_logical_peak_upper_bound()
        );
    }

    #[test]
    fn fresh_success_has_exact_censes_and_rejects_one_below_v1_work() {
        let sources = vec![input(ints(&[0, 2, 1]), &[0])];
        let baseline_limits = ResidualAffineIntegerSystemLimits::default();
        let fresh = fresh_complete(2, &sources, baseline_limits);
        let exact_allocation = Arc::clone(&fresh.certificate);
        let stats = exact_allocation.stats();
        let envelope =
            residual_affine_integer_system_memory_envelope_from_limits(baseline_limits).unwrap();
        assert!(fresh.retained_owned_logical_bytes_upper_bound() > 0);
        assert!(
            fresh.compilation_owned_logical_peak_upper_bound()
                >= fresh.retained_owned_logical_bytes_upper_bound()
        );
        assert!(
            envelope.compilation_owned_logical_peak_upper_bound()
                >= envelope.retained_owned_logical_bytes_upper_bound()
        );
        assert!(
            fresh.retained_owned_logical_bytes_upper_bound()
                <= envelope.retained_owned_logical_bytes_upper_bound()
        );
        assert!(
            fresh.compilation_owned_logical_peak_upper_bound()
                <= envelope.compilation_owned_logical_peak_upper_bound()
        );
        assert_eq!(
            fresh.raw_transient_census(),
            ResidualAffineIntegerSystemRawTransientCensus::from_stats(stats)
        );
        assert_eq!(
            fresh.payload_comparison_census(),
            exact_allocation
                .recompute_payload_comparison_census()
                .unwrap()
        );
        assert!(fresh.payload_comparison_census().units() > 0);
        assert!(fresh.payload_comparison_census().bytes() > 0);
        assert!(fresh.payload_comparison_census().integer_bits() > 0);
        let shape = integer_system_retained_shape_census(&exact_allocation).unwrap();
        let expected_operand_units = scalar_representation_units::<
            ResidualAffineIntegerSystemCertificate,
        >() + exact_allocation.schema.len()
            + shape.heap_entry_units().unwrap();
        assert_eq!(
            fresh.payload_comparison_census().units(),
            2 * expected_operand_units
        );
        assert_eq!(
            fresh.payload_comparison_census().bytes(),
            2 * (fresh.retained_owned_logical_bytes_upper_bound() + exact_allocation.schema.len())
        );
        assert_eq!(
            fresh.payload_comparison_census().integer_bits(),
            2 * shape.total_integer_bits
        );
        let independent_equal = Arc::new((*exact_allocation).clone());
        assert!(exact_allocation.payload_eq(&independent_equal));
        let (certificate, authorization) = fresh.into_certificate_and_plan_authorization().unwrap();
        assert!(Arc::ptr_eq(&certificate, &exact_allocation));
        assert!(authorization.authenticates_certificate_allocation(&certificate));
        assert!(!authorization.authenticates_certificate_allocation(&independent_equal));
        let authorized_certificate = authorization.into_authenticated_certificate_arc().unwrap();
        assert!(Arc::ptr_eq(&authorized_certificate, &certificate));

        let legacy =
            ResidualAffineIntegerSystemCertificate::compile(2, &sources, baseline_limits).unwrap();
        assert_eq!(legacy.schema(), RESIDUAL_AFFINE_INTEGER_SYSTEM_V1_SCHEMA);
        assert!(legacy.payload_eq(&certificate));
        legacy.replay().unwrap();

        macro_rules! exact_and_one_below {
            ($field:ident, $stat:ident, $resource:literal) => {{
                let exact = stats.$stat();
                assert!(exact > 0, "{} must be positive", stringify!($stat));
                let mut exact_limits = baseline_limits;
                exact_limits.$field = exact;
                assert!(matches!(
                    ResidualAffineIntegerSystemCertificate::compile_fresh(
                        2,
                        &sources,
                        exact_limits
                    ),
                    Ok(ResidualAffineIntegerSystemFreshCompilationAttempt::Complete(_))
                ));
                let mut below_limits = baseline_limits;
                below_limits.$field = exact - 1;
                assert!(matches!(
                    ResidualAffineIntegerSystemCertificate::compile_fresh(
                        2,
                        &sources,
                        below_limits
                    ),
                    Err(ResidualAffineIntegerSystemError::ResourceLimit {
                        resource: $resource,
                        ..
                    })
                ));
            }};
        }
        exact_and_one_below!(
            max_allocation_entries_reserved,
            allocation_entries_reserved,
            "allocation entries reserved"
        );
        exact_and_one_below!(
            max_state_entries_materialized,
            state_entries_materialized,
            "state entries materialized"
        );
        exact_and_one_below!(max_integer_bit_work, integer_bit_work, "integer bit work");
        exact_and_one_below!(max_frontier_states, frontier_states_peak, "frontier states");
        exact_and_one_below!(
            max_verification_operations,
            verification_operations,
            "verification operations"
        );
    }

    #[test]
    fn fresh_unsupported_preserves_transient_census_and_v1_error() {
        let sources = vec![input(ints(&[0, 2, 3]), &[0])];
        let limits = ResidualAffineIntegerSystemLimits::default();
        let unsupported =
            match ResidualAffineIntegerSystemCertificate::compile_fresh(2, &sources, limits)
                .unwrap()
            {
                ResidualAffineIntegerSystemFreshCompilationAttempt::Unsupported(unsupported) => {
                    unsupported
                }
                ResidualAffineIntegerSystemFreshCompilationAttempt::Complete(_) => {
                    panic!("congruence fixture unexpectedly completed")
                }
            };
        assert_eq!(
            unsupported.reason(),
            ResidualAffineIntegerSystemUnsupported::GeneralCongruenceCaseNotSupported {
                remaining_equations: 1
            }
        );
        let transient = unsupported.raw_transient_census();
        assert!(transient.allocation_entries_reserved() > 0);
        assert!(transient.state_entries_materialized() > 0);
        assert!(transient.integer_bit_work() > 0);
        assert!(transient.frontier_states_peak() > 0);
        assert!(unsupported.compilation_owned_logical_peak_upper_bound() > 0);
        assert!(matches!(
            ResidualAffineIntegerSystemCertificate::compile(2, &sources, limits),
            Err(ResidualAffineIntegerSystemError::Unsupported(
                ResidualAffineIntegerSystemUnsupported::GeneralCongruenceCaseNotSupported {
                    remaining_equations: 1
                }
            ))
        ));

        macro_rules! unsupported_exact_and_one_below {
            ($field:ident, $value:expr, $resource:literal) => {{
                let exact = $value;
                assert!(exact > 0);
                let mut exact_limits = limits;
                exact_limits.$field = exact;
                assert!(matches!(
                    ResidualAffineIntegerSystemCertificate::compile_fresh(
                        2,
                        &sources,
                        exact_limits
                    ),
                    Ok(ResidualAffineIntegerSystemFreshCompilationAttempt::Unsupported(_))
                ));
                let mut below_limits = limits;
                below_limits.$field = exact - 1;
                assert!(matches!(
                    ResidualAffineIntegerSystemCertificate::compile_fresh(
                        2,
                        &sources,
                        below_limits
                    ),
                    Err(ResidualAffineIntegerSystemError::ResourceLimit {
                        resource: $resource,
                        ..
                    })
                ));
            }};
        }
        unsupported_exact_and_one_below!(
            max_allocation_entries_reserved,
            transient.allocation_entries_reserved(),
            "allocation entries reserved"
        );
        unsupported_exact_and_one_below!(
            max_state_entries_materialized,
            transient.state_entries_materialized(),
            "state entries materialized"
        );
        unsupported_exact_and_one_below!(
            max_integer_bit_work,
            transient.integer_bit_work(),
            "integer bit work"
        );
        unsupported_exact_and_one_below!(
            max_frontier_states,
            transient.frontier_states_peak(),
            "frontier states"
        );
    }

    #[test]
    fn fresh_seal_rejects_each_adjacent_census_tamper() {
        let sources = vec![input(ints(&[0, 2, 1]), &[0])];
        let limits = ResidualAffineIntegerSystemLimits::default();
        macro_rules! rejects_tamper {
            ($method:ident) => {{
                let mut fresh = fresh_complete(2, &sources, limits);
                fresh.$method();
                assert!(matches!(
                    fresh.authenticate_adjacent_census(),
                    Err(ResidualAffineIntegerSystemError::ReplayMismatch)
                ));
            }};
        }
        rejects_tamper!(tamper_retained_census_for_test);
        rejects_tamper!(tamper_peak_census_for_test);
        rejects_tamper!(tamper_transient_census_for_test);
        rejects_tamper!(tamper_payload_units_for_test);
        rejects_tamper!(tamper_payload_bytes_for_test);
        rejects_tamper!(tamper_payload_integer_bits_for_test);

        let fresh = fresh_complete(2, &sources, limits);
        let (certificate, mut authorization) =
            fresh.into_certificate_and_plan_authorization().unwrap();
        assert!(authorization.authenticates_certificate_allocation(&certificate));
        authorization.tamper_payload_units_for_test();
        assert!(matches!(
            authorization.into_authenticated_certificate_arc(),
            Err(ResidualAffineIntegerSystemError::ReplayMismatch)
        ));
    }

    #[test]
    fn replay_rejects_tampering_across_every_retained_proof_layer() {
        let sources = vec![input(ints(&[0, 2, 1]), &[0])];
        let certificate = ResidualAffineIntegerSystemCertificate::compile(
            2,
            &sources,
            ResidualAffineIntegerSystemLimits::default(),
        )
        .unwrap();

        let mut tampered_map = certificate.clone();
        tampered_map.affine_map.as_mut().unwrap().constants[0] = Integer::one();
        assert_eq!(
            tampered_map.replay(),
            Err(ResidualAffineIntegerSystemError::ReplayMismatch)
        );

        let mut tampered_transcript = certificate.clone();
        tampered_transcript
            .operations
            .push(ResidualAffineIntegerRowOperation::Negate { row: 0 });
        assert_eq!(
            tampered_transcript.replay(),
            Err(ResidualAffineIntegerSystemError::ReplayMismatch)
        );

        let mut tampered_final = certificate.clone();
        tampered_final.final_rows[0].components[0] = Integer::one();
        assert_eq!(
            tampered_final.replay(),
            Err(ResidualAffineIntegerSystemError::ReplayMismatch)
        );

        let mut tampered_pivots = certificate.clone();
        tampered_pivots.pivot_positions.clear();
        assert_eq!(
            tampered_pivots.replay(),
            Err(ResidualAffineIntegerSystemError::ReplayMismatch)
        );

        let mut tampered_free = certificate.clone();
        tampered_free.free_positions.push(1);
        assert_eq!(
            tampered_free.replay(),
            Err(ResidualAffineIntegerSystemError::ReplayMismatch)
        );

        let mut tampered_canonical_source = certificate.clone();
        tampered_canonical_source.source_rows[0].structural_locus_ordinals[0] = 31;
        assert_eq!(
            tampered_canonical_source.replay(),
            Err(ResidualAffineIntegerSystemError::ReplayMismatch)
        );

        let mut tampered_supplied_source = certificate.clone();
        tampered_supplied_source.replay_source_rows[0].structural_locus_ordinals[0] = 37;
        assert_eq!(
            tampered_supplied_source.replay(),
            Err(ResidualAffineIntegerSystemError::ReplayMismatch)
        );

        let mut tampered_stats = certificate.clone();
        tampered_stats.stats.rank = 99;
        assert_eq!(
            tampered_stats.replay(),
            Err(ResidualAffineIntegerSystemError::ReplayMismatch)
        );

        let mut tampered_schema = certificate.clone();
        tampered_schema.schema = "tampered-affine-system-schema";
        assert_eq!(
            tampered_schema.replay(),
            Err(ResidualAffineIntegerSystemError::SchemaMismatch)
        );

        let mut tampered_limits = certificate.clone();
        tampered_limits.limits.max_map_entries = 0;
        assert!(matches!(
            tampered_limits.replay(),
            Err(ResidualAffineIntegerSystemError::ResourceLimit {
                resource: "map entries",
                ..
            })
        ));

        let empty_sources = vec![input(ints(&[0, 1]), &[1]), input(ints(&[1, 1]), &[2])];
        let empty = ResidualAffineIntegerSystemCertificate::compile(
            1,
            &empty_sources,
            ResidualAffineIntegerSystemLimits::default(),
        )
        .unwrap();
        let mut tampered_empty = empty.clone();
        match tampered_empty.empty_witness.as_mut().unwrap() {
            ResidualAffineIntegerEmptyWitness::ZeroEqualsNonzero { constant, .. }
            | ResidualAffineIntegerEmptyWitness::CoefficientGcdDoesNotDivideConstant {
                constant,
                ..
            } => *constant = Integer::from(17),
        }
        assert_eq!(
            tampered_empty.replay(),
            Err(ResidualAffineIntegerSystemError::ReplayMismatch)
        );
    }
}
