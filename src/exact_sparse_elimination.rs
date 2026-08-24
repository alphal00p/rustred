//! Deterministic exact sparse elimination with replayable provenance.
//!
//! Columns are integer slots ordered from easiest to hardest.  Callers supply
//! a hardest-first pivot skeleton `(source_row_index, expected_pivot_column)`.
//! The skeleton is only a proposal: every selected row is reduced exactly,
//! its expected hardest surviving column is authenticated, and all source rows
//! must reduce to zero before a certificate is returned.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::prelude::*;

use crate::coefficient::{
    coefficient_product_degree_bound, coefficient_sum_degree_bound, coefficient_variable_degrees,
    symbolica_coefficient_degree_is_representable,
};
use crate::{Coefficient, CoefficientContext, SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT};

const EXACT_SPARSE_ELIMINATION_SCHEMA: &str = "rustred-exact-sparse-elimination-v1";
const FNV1A64_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV1A64_PRIME: u64 = 0x0000_0100_0000_01b3;

/// One sparse row over RustRed's exact Symbolica coefficient field.
pub type ExactSparseRow = BTreeMap<usize, Coefficient>;

/// Exact location of a malformed coefficient at an input or certificate
/// boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExactSparseCoefficientLocation {
    SourceEntry {
        row_index: usize,
        column: usize,
    },
    PivotEntry {
        pivot_ordinal: usize,
        column: usize,
    },
    TraceDivisor {
        pivot_ordinal: usize,
    },
    TraceReductionFactor {
        pivot_ordinal: usize,
        reduction_index: usize,
    },
}

impl fmt::Display for ExactSparseCoefficientLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceEntry { row_index, column } => {
                write!(formatter, "source row {row_index}, column {column}")
            }
            Self::PivotEntry {
                pivot_ordinal,
                column,
            } => write!(formatter, "pivot {pivot_ordinal}, row column {column}"),
            Self::TraceDivisor { pivot_ordinal } => {
                write!(formatter, "pivot {pivot_ordinal} trace divisor")
            }
            Self::TraceReductionFactor {
                pivot_ordinal,
                reduction_index,
            } => write!(
                formatter,
                "pivot {pivot_ordinal} trace reduction {reduction_index} factor"
            ),
        }
    }
}

/// Independent resource envelopes for exact construction and replay.
///
/// `max_reductions` and `max_updates` cover construction of the proposed
/// pivot rows.  The separate replay limits cover deterministic trace
/// reconstruction plus the final reduction of every source row.  A logical
/// update is one touched sparse coefficient slot; arithmetic is additionally
/// preflighted by degree, actual-term, and dense-monomial bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExactSparseEliminationConfig {
    pub max_rows: usize,
    pub max_columns: usize,
    pub max_input_entries: usize,
    pub max_input_coefficient_bytes: usize,
    pub max_reductions: usize,
    pub max_updates: usize,
    pub max_retained_entries: usize,
    pub max_retained_coefficient_terms: usize,
    pub max_retained_coefficient_bytes: usize,
    pub max_coefficient_degree: usize,
    pub max_coefficient_operation_terms: usize,
    pub max_coefficient_dense_terms: usize,
    pub max_integer_bits: usize,
    pub max_coefficient_pair_products: usize,
    pub max_canonicalization_work: usize,
    pub max_replay_reductions: usize,
    pub max_replay_updates: usize,
}

impl Default for ExactSparseEliminationConfig {
    fn default() -> Self {
        Self {
            max_rows: 10_000,
            max_columns: 10_000,
            max_input_entries: 10_000_000,
            max_input_coefficient_bytes: 2 * 1024 * 1024 * 1024,
            max_reductions: 100_000_000,
            max_updates: 1_000_000_000,
            max_retained_entries: 100_000_000,
            max_retained_coefficient_terms: 500_000_000,
            max_retained_coefficient_bytes: 2 * 1024 * 1024 * 1024,
            max_coefficient_degree: 4_096,
            max_coefficient_operation_terms: 10_000_000,
            max_coefficient_dense_terms: 100_000_000,
            max_integer_bits: 1_000_000,
            max_coefficient_pair_products: 1_000_000_000,
            max_canonicalization_work: 10_000_000_000,
            max_replay_reductions: 200_000_000,
            max_replay_updates: 2_000_000_000,
        }
    }
}

/// One exact coefficient used to eliminate a prior unit pivot row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactSparseDerivationReduction {
    prior_pivot_ordinal: usize,
    factor: Coefficient,
}

impl ExactSparseDerivationReduction {
    pub const fn prior_pivot_ordinal(&self) -> usize {
        self.prior_pivot_ordinal
    }

    pub const fn factor(&self) -> &Coefficient {
        &self.factor
    }
}

/// Compact recursive derivation of a stored unit pivot row.
///
/// For a trace `T`, with prior stored unit rows `P`, the invariant is
///
/// ```text
/// unit_row(T) =
///   (source_rows[T.base_source_row_index]
///    - sum T.reductions[i].factor * P[T.reductions[i].prior_pivot_ordinal])
///   / T.divisor.
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactSparseDerivationTrace {
    base_source_row_index: usize,
    reductions: Vec<ExactSparseDerivationReduction>,
    divisor: Coefficient,
}

impl ExactSparseDerivationTrace {
    pub const fn base_source_row_index(&self) -> usize {
        self.base_source_row_index
    }

    pub fn reductions(&self) -> &[ExactSparseDerivationReduction] {
        &self.reductions
    }

    pub const fn divisor(&self) -> &Coefficient {
        &self.divisor
    }
}

/// One exact unit-pivot row and its recursive source provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactSparsePivotRule {
    ordinal: usize,
    source_row_index: usize,
    pivot_column: usize,
    row: ExactSparseRow,
    trace: ExactSparseDerivationTrace,
}

impl ExactSparsePivotRule {
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub const fn source_row_index(&self) -> usize {
        self.source_row_index
    }

    pub const fn pivot_column(&self) -> usize {
        self.pivot_column
    }

    /// Unit row in equation form: `pivot + easier terms = 0`.
    pub const fn row(&self) -> &ExactSparseRow {
        &self.row
    }

    pub const fn trace(&self) -> &ExactSparseDerivationTrace {
        &self.trace
    }
}

/// Exact construction, storage, and replay census.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExactSparseEliminationStats {
    source_rows: usize,
    columns: usize,
    input_entries: usize,
    rank: usize,
    free_columns: usize,
    pivot_reductions: usize,
    verification_reductions: usize,
    arithmetic_updates: usize,
    retained_entries: usize,
    retained_coefficient_terms: usize,
    retained_coefficient_bytes: usize,
    maximum_row_width: usize,
    maximum_coefficient_degree: usize,
    replay_reductions: usize,
    replay_updates: usize,
}

impl ExactSparseEliminationStats {
    pub const fn source_rows(self) -> usize {
        self.source_rows
    }

    pub const fn columns(self) -> usize {
        self.columns
    }

    pub const fn input_entries(self) -> usize {
        self.input_entries
    }

    pub const fn rank(self) -> usize {
        self.rank
    }

    pub const fn free_columns(self) -> usize {
        self.free_columns
    }

    pub const fn pivot_reductions(self) -> usize {
        self.pivot_reductions
    }

    /// Reductions used by the all-source zero proof during replay.
    pub const fn verification_reductions(self) -> usize {
        self.verification_reductions
    }

    /// Logical sparse coefficient updates used to construct pivot rows.
    pub const fn arithmetic_updates(self) -> usize {
        self.arithmetic_updates
    }

    pub const fn retained_entries(self) -> usize {
        self.retained_entries
    }

    pub const fn retained_coefficient_terms(self) -> usize {
        self.retained_coefficient_terms
    }

    pub const fn retained_coefficient_bytes(self) -> usize {
        self.retained_coefficient_bytes
    }

    pub const fn maximum_row_width(self) -> usize {
        self.maximum_row_width
    }

    pub const fn maximum_coefficient_degree(self) -> usize {
        self.maximum_coefficient_degree
    }

    /// Trace reconstruction plus all-source reductions in internal replay.
    pub const fn replay_reductions(self) -> usize {
        self.replay_reductions
    }

    pub const fn replay_updates(self) -> usize {
        self.replay_updates
    }
}

/// A deterministic, exact, replayable rank certificate.
#[derive(Clone, Debug)]
pub struct ExactSparseElimination {
    coefficient_context: CoefficientContext,
    config: ExactSparseEliminationConfig,
    source_row_count: usize,
    column_count: usize,
    source_checksum: u64,
    pivot_rules: Vec<ExactSparsePivotRule>,
    free_columns: Vec<usize>,
    stats: ExactSparseEliminationStats,
    checksum: u64,
}

impl ExactSparseElimination {
    pub const SCHEMA: &'static str = EXACT_SPARSE_ELIMINATION_SCHEMA;

    /// Construct and independently replay an exact rank certificate.
    ///
    /// Neither the input slice nor any source row is mutated.  The pivot
    /// skeleton is interpreted strictly as `(source row, expected column)` in
    /// hardest-first order.
    pub fn build(
        context: &CoefficientContext,
        source_rows: &[ExactSparseRow],
        column_count: usize,
        pivot_skeleton: &[(usize, usize)],
        config: ExactSparseEliminationConfig,
    ) -> Result<Self, ExactSparseEliminationError> {
        validate_config(config)?;
        let input = validate_source(context, source_rows, column_count, config)?;
        validate_skeleton(pivot_skeleton, source_rows.len(), column_count, config)?;

        let mut construction = WorkBudget::construction(config, input.maximum_row_width);
        let mut arithmetic = CheckedArithmetic::new(context, config, input.maximum_degree)?;
        let mut pivot_rules = Vec::new();
        let mut retained = RetainedCensus::default();
        pivot_rules
            .try_reserve_exact(pivot_skeleton.len())
            .map_err(|_| ExactSparseEliminationError::ResourceLimit {
                resource: "pivot rules",
                requested: pivot_skeleton.len() as u128,
                limit: config.max_rows as u128,
            })?;

        for (ordinal, &(source_row_index, expected_pivot_column)) in
            pivot_skeleton.iter().enumerate()
        {
            let rule = derive_pivot_rule(
                ordinal,
                source_row_index,
                expected_pivot_column,
                source_rows,
                &pivot_rules,
                &mut construction,
                &mut arithmetic,
            )?;
            charge_retained_rule(&mut retained, &rule, context, config)?;
            pivot_rules.push(rule);
        }

        let free_columns = free_column_complement(column_count, &pivot_rules)?;
        let proof = prove_certificate(
            context,
            source_rows,
            column_count,
            &pivot_rules,
            &free_columns,
            config,
            input.maximum_row_width,
            input.maximum_degree,
        )?;
        if proof.derivation_reductions != construction.reductions
            || proof.derivation_updates != construction.updates
        {
            return Err(ExactSparseEliminationError::CertificateMismatch {
                component: "construction and deterministic trace replay census",
            });
        }

        let maximum_row_width = construction.maximum_row_width.max(proof.maximum_row_width);
        let maximum_degree_u128 = arithmetic
            .maximum_degree
            .max(proof.maximum_degree)
            .max(retained.maximum_degree);
        let maximum_coefficient_degree = usize::try_from(maximum_degree_u128).map_err(|_| {
            ExactSparseEliminationError::ArithmeticOverflow {
                resource: "maximum coefficient degree",
            }
        })?;
        let replay_reductions = checked_add(
            proof.derivation_reductions,
            proof.verification_reductions,
            "replay reductions",
        )?;
        let stats = ExactSparseEliminationStats {
            source_rows: source_rows.len(),
            columns: column_count,
            input_entries: input.entries,
            rank: pivot_rules.len(),
            free_columns: free_columns.len(),
            pivot_reductions: construction.reductions,
            verification_reductions: proof.verification_reductions,
            arithmetic_updates: construction.updates,
            retained_entries: retained.entries,
            retained_coefficient_terms: retained.terms,
            retained_coefficient_bytes: retained.bytes,
            maximum_row_width,
            maximum_coefficient_degree,
            replay_reductions,
            replay_updates: proof.total_updates,
        };
        let checksum = certificate_checksum(
            context,
            config,
            source_rows.len(),
            column_count,
            input.checksum,
            &pivot_rules,
            &free_columns,
            stats,
        );

        Ok(Self {
            coefficient_context: context.clone(),
            config,
            source_row_count: source_rows.len(),
            column_count,
            source_checksum: input.checksum,
            pivot_rules,
            free_columns,
            stats,
            checksum,
        })
    }

    pub const fn coefficient_context(&self) -> &CoefficientContext {
        &self.coefficient_context
    }

    pub const fn config(&self) -> &ExactSparseEliminationConfig {
        &self.config
    }

    pub const fn source_row_count(&self) -> usize {
        self.source_row_count
    }

    pub const fn column_count(&self) -> usize {
        self.column_count
    }

    pub fn rank(&self) -> usize {
        self.pivot_rules.len()
    }

    pub fn pivot_rules(&self) -> &[ExactSparsePivotRule] {
        &self.pivot_rules
    }

    pub fn pivot_rows(
        &self,
    ) -> impl ExactSizeIterator<Item = &ExactSparseRow> + DoubleEndedIterator {
        self.pivot_rules.iter().map(ExactSparsePivotRule::row)
    }

    pub fn traces(
        &self,
    ) -> impl ExactSizeIterator<Item = &ExactSparseDerivationTrace> + DoubleEndedIterator {
        self.pivot_rules.iter().map(ExactSparsePivotRule::trace)
    }

    pub fn free_columns(&self) -> &[usize] {
        &self.free_columns
    }

    pub const fn stats(&self) -> ExactSparseEliminationStats {
        self.stats
    }

    pub const fn source_checksum(&self) -> u64 {
        self.source_checksum
    }

    pub const fn checksum(&self) -> u64 {
        self.checksum
    }

    /// Collision-free comparison of every semantic certificate component.
    /// Checksums remain diagnostics/indexing aids and are deliberately not
    /// used as proof equality.
    pub fn has_identical_semantic_payload(&self, other: &Self) -> bool {
        self.coefficient_context
            .has_same_variable_map(&other.coefficient_context)
            && self.config == other.config
            && self.source_row_count == other.source_row_count
            && self.column_count == other.column_count
            && self.pivot_rules == other.pivot_rules
            && self.free_columns == other.free_columns
            && self.stats == other.stats
    }

    /// Re-authenticate immutable source rows and repeat every exact proof.
    pub fn replay(
        &self,
        context: &CoefficientContext,
        source_rows: &[ExactSparseRow],
    ) -> Result<(), ExactSparseEliminationError> {
        if !self.coefficient_context.has_same_variable_map(context) {
            return Err(ExactSparseEliminationError::CoefficientContextMismatch);
        }
        if source_rows.len() != self.source_row_count {
            return Err(ExactSparseEliminationError::SourceShapeMismatch {
                expected_rows: self.source_row_count,
                actual_rows: source_rows.len(),
            });
        }
        validate_config(self.config)?;
        let input = validate_source(context, source_rows, self.column_count, self.config)?;
        if input.checksum != self.source_checksum {
            return Err(ExactSparseEliminationError::SourceChecksumMismatch {
                expected: self.source_checksum,
                actual: input.checksum,
            });
        }
        validate_stored_certificate(
            context,
            self.source_row_count,
            self.column_count,
            &self.pivot_rules,
            &self.free_columns,
            self.config,
        )?;
        let retained = charge_retained(&self.pivot_rules, context, self.config)?;
        let proof = prove_certificate(
            context,
            source_rows,
            self.column_count,
            &self.pivot_rules,
            &self.free_columns,
            self.config,
            input.maximum_row_width,
            input.maximum_degree,
        )?;
        let replay_reductions = checked_add(
            proof.derivation_reductions,
            proof.verification_reductions,
            "replay reductions",
        )?;
        if proof.derivation_reductions != self.stats.pivot_reductions
            || proof.derivation_updates != self.stats.arithmetic_updates
            || proof.verification_reductions != self.stats.verification_reductions
            || replay_reductions != self.stats.replay_reductions
            || proof.total_updates != self.stats.replay_updates
            || proof.maximum_row_width != self.stats.maximum_row_width
            || usize::try_from(proof.maximum_degree.max(retained.maximum_degree)).ok()
                != Some(self.stats.maximum_coefficient_degree)
            || retained.entries != self.stats.retained_entries
            || retained.terms != self.stats.retained_coefficient_terms
            || retained.bytes != self.stats.retained_coefficient_bytes
        {
            return Err(ExactSparseEliminationError::CertificateMismatch {
                component: "replayed statistics or retention census",
            });
        }
        let stats_shape = (
            self.stats.source_rows,
            self.stats.columns,
            self.stats.input_entries,
            self.stats.rank,
            self.stats.free_columns,
        );
        let expected_shape = (
            source_rows.len(),
            self.column_count,
            input.entries,
            self.pivot_rules.len(),
            self.free_columns.len(),
        );
        if stats_shape != expected_shape {
            return Err(ExactSparseEliminationError::CertificateMismatch {
                component: "stored matrix shape statistics",
            });
        }
        let checksum = certificate_checksum(
            context,
            self.config,
            self.source_row_count,
            self.column_count,
            input.checksum,
            &self.pivot_rules,
            &self.free_columns,
            self.stats,
        );
        if checksum != self.checksum {
            return Err(ExactSparseEliminationError::CertificateChecksumMismatch {
                expected: self.checksum,
                actual: checksum,
            });
        }
        Ok(())
    }
}

impl fmt::Display for ExactSparseElimination {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} rows={} columns={} rank={} free={} source_checksum=0x{:016x} checksum=0x{:016x}",
            Self::SCHEMA,
            self.source_row_count,
            self.column_count,
            self.rank(),
            self.free_columns.len(),
            self.source_checksum,
            self.checksum,
        )
    }
}

/// Typed failures from exact sparse construction or replay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExactSparseEliminationError {
    CoefficientContextMismatch,
    SourceShapeMismatch {
        expected_rows: usize,
        actual_rows: usize,
    },
    SourceChecksumMismatch {
        expected: u64,
        actual: u64,
    },
    ColumnOutOfRange {
        row_index: usize,
        column: usize,
        column_count: usize,
    },
    ExplicitZeroCoefficient {
        row_index: usize,
        column: usize,
    },
    MalformedCoefficient {
        location: ExactSparseCoefficientLocation,
        reason: &'static str,
    },
    SkeletonSourceRowOutOfRange {
        ordinal: usize,
        source_row_index: usize,
        source_row_count: usize,
    },
    SkeletonPivotColumnOutOfRange {
        ordinal: usize,
        pivot_column: usize,
        column_count: usize,
    },
    DuplicateSkeletonSourceRow {
        ordinal: usize,
        source_row_index: usize,
    },
    DuplicateSkeletonPivotColumn {
        ordinal: usize,
        pivot_column: usize,
    },
    SkeletonNotHardestFirst {
        ordinal: usize,
        previous_pivot_column: usize,
        pivot_column: usize,
    },
    ExpectedPivotMismatch {
        ordinal: usize,
        source_row_index: usize,
        expected_pivot_column: usize,
        actual_hardest_column: Option<usize>,
    },
    ZeroPivotDivisor {
        ordinal: usize,
        source_row_index: usize,
        pivot_column: usize,
    },
    NonUnitPivot {
        ordinal: usize,
        pivot_column: usize,
    },
    IncompleteSkeleton {
        source_row_index: usize,
        hardest_remaining_column: usize,
    },
    InvalidCertificate {
        ordinal: Option<usize>,
        reason: &'static str,
    },
    TraceMismatch {
        ordinal: usize,
    },
    CertificateMismatch {
        component: &'static str,
    },
    CertificateChecksumMismatch {
        expected: u64,
        actual: u64,
    },
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    ArithmeticOverflow {
        resource: &'static str,
    },
}

impl fmt::Display for ExactSparseEliminationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoefficientContextMismatch => formatter.write_str(
                "a coefficient does not use the caller's exact ordered Symbolica variable map",
            ),
            Self::SourceShapeMismatch {
                expected_rows,
                actual_rows,
            } => write!(
                formatter,
                "source row count mismatch: expected {expected_rows}, found {actual_rows}"
            ),
            Self::SourceChecksumMismatch { expected, actual } => write!(
                formatter,
                "source checksum mismatch: expected 0x{expected:016x}, found 0x{actual:016x}"
            ),
            Self::ColumnOutOfRange {
                row_index,
                column,
                column_count,
            } => write!(
                formatter,
                "source row {row_index} names column {column}, outside 0..{column_count}"
            ),
            Self::ExplicitZeroCoefficient { row_index, column } => write!(
                formatter,
                "source row {row_index}, column {column} stores an explicit zero coefficient"
            ),
            Self::MalformedCoefficient { location, reason } => {
                write!(
                    formatter,
                    "malformed exact coefficient at {location}: {reason}"
                )
            }
            Self::SkeletonSourceRowOutOfRange {
                ordinal,
                source_row_index,
                source_row_count,
            } => write!(
                formatter,
                "pivot {ordinal} names source row {source_row_index}, outside 0..{source_row_count}"
            ),
            Self::SkeletonPivotColumnOutOfRange {
                ordinal,
                pivot_column,
                column_count,
            } => write!(
                formatter,
                "pivot {ordinal} names column {pivot_column}, outside 0..{column_count}"
            ),
            Self::DuplicateSkeletonSourceRow {
                ordinal,
                source_row_index,
            } => write!(
                formatter,
                "pivot {ordinal} reuses source row {source_row_index}"
            ),
            Self::DuplicateSkeletonPivotColumn {
                ordinal,
                pivot_column,
            } => write!(formatter, "pivot {ordinal} reuses column {pivot_column}"),
            Self::SkeletonNotHardestFirst {
                ordinal,
                previous_pivot_column,
                pivot_column,
            } => write!(
                formatter,
                "pivot {ordinal} column {pivot_column} is not strictly easier than prior column {previous_pivot_column}"
            ),
            Self::ExpectedPivotMismatch {
                ordinal,
                source_row_index,
                expected_pivot_column,
                actual_hardest_column,
            } => write!(
                formatter,
                "pivot {ordinal} from source row {source_row_index} expected hardest column {expected_pivot_column}, found {actual_hardest_column:?}"
            ),
            Self::ZeroPivotDivisor {
                ordinal,
                source_row_index,
                pivot_column,
            } => write!(
                formatter,
                "pivot {ordinal} from source row {source_row_index}, column {pivot_column} has a zero divisor"
            ),
            Self::NonUnitPivot {
                ordinal,
                pivot_column,
            } => write!(
                formatter,
                "pivot {ordinal}, column {pivot_column} did not normalize exactly to one"
            ),
            Self::IncompleteSkeleton {
                source_row_index,
                hardest_remaining_column,
            } => write!(
                formatter,
                "source row {source_row_index} retains column {hardest_remaining_column} after every proposed pivot"
            ),
            Self::InvalidCertificate { ordinal, reason } => {
                write!(
                    formatter,
                    "invalid stored certificate at pivot {ordinal:?}: {reason}"
                )
            }
            Self::TraceMismatch { ordinal } => {
                write!(formatter, "exact trace replay differs at pivot {ordinal}")
            }
            Self::CertificateMismatch { component } => {
                write!(formatter, "exact certificate mismatch in {component}")
            }
            Self::CertificateChecksumMismatch { expected, actual } => write!(
                formatter,
                "certificate checksum mismatch: expected 0x{expected:016x}, found 0x{actual:016x}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "exact sparse {resource} requested {requested}, limit is {limit}"
            ),
            Self::ArithmeticOverflow { resource } => {
                write!(formatter, "arithmetic overflow while counting {resource}")
            }
        }
    }
}

impl Error for ExactSparseEliminationError {}

#[derive(Clone, Copy)]
enum WorkKind {
    Construction,
    Replay,
}

struct WorkBudget {
    kind: WorkKind,
    config: ExactSparseEliminationConfig,
    reductions: usize,
    updates: usize,
    maximum_row_width: usize,
}

impl WorkBudget {
    const fn construction(config: ExactSparseEliminationConfig, maximum_row_width: usize) -> Self {
        Self {
            kind: WorkKind::Construction,
            config,
            reductions: 0,
            updates: 0,
            maximum_row_width,
        }
    }

    const fn replay(config: ExactSparseEliminationConfig, maximum_row_width: usize) -> Self {
        Self {
            kind: WorkKind::Replay,
            config,
            reductions: 0,
            updates: 0,
            maximum_row_width,
        }
    }

    fn charge_reduction(&mut self) -> Result<(), ExactSparseEliminationError> {
        self.reductions = checked_add(self.reductions, 1, "elimination reductions")?;
        let limit = match self.kind {
            WorkKind::Construction => self.config.max_reductions,
            WorkKind::Replay => self.config.max_replay_reductions,
        };
        check_resource("elimination reductions", self.reductions, limit)
    }

    fn charge_update(&mut self) -> Result<(), ExactSparseEliminationError> {
        self.updates = checked_add(self.updates, 1, "sparse coefficient updates")?;
        let limit = match self.kind {
            WorkKind::Construction => self.config.max_updates,
            WorkKind::Replay => self.config.max_replay_updates,
        };
        check_resource("sparse coefficient updates", self.updates, limit)
    }

    fn observe_row(&mut self, row: &ExactSparseRow) {
        self.maximum_row_width = self.maximum_row_width.max(row.len());
    }
}

struct CheckedArithmetic {
    zero: Coefficient,
    one: Coefficient,
    minus_one: Coefficient,
    config: ExactSparseEliminationConfig,
    maximum_degree: u128,
}

impl CheckedArithmetic {
    fn new(
        context: &CoefficientContext,
        config: ExactSparseEliminationConfig,
        initial_maximum_degree: u128,
    ) -> Result<Self, ExactSparseEliminationError> {
        let mut value = Self {
            zero: context.zero(),
            one: context.one(),
            minus_one: context.integer(-1),
            config,
            maximum_degree: initial_maximum_degree,
        };
        let zero = value.zero.clone();
        value.check_existing(&zero)?;
        Ok(value)
    }

    fn check_existing(
        &mut self,
        coefficient: &Coefficient,
    ) -> Result<(), ExactSparseEliminationError> {
        if coefficient.get_variables() != self.zero.get_variables() {
            return Err(ExactSparseEliminationError::CoefficientContextMismatch);
        }
        let degree = coefficient_maximum_degree(coefficient);
        check_coefficient_degree(degree, self.config)?;
        self.maximum_degree = self.maximum_degree.max(degree);
        let terms = coefficient
            .numerator
            .nterms()
            .max(coefficient.denominator.nterms());
        check_resource(
            "coefficient operand/result terms",
            terms,
            self.config.max_coefficient_operation_terms,
        )?;
        check_dense_bound(existing_dense_bound(coefficient), self.config)?;
        check_integer_bits(coefficient, self.config)
    }

    fn multiply(
        &mut self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, ExactSparseEliminationError> {
        self.check_existing(left)?;
        self.check_existing(right)?;
        check_pair_product_work(product_pair_work(left, right), self.config)?;
        check_coefficient_degree(coefficient_product_degree_bound(left, right), self.config)?;
        check_dense_bound(product_dense_bound(left, right), self.config)?;
        let output = if left == &self.one {
            right.clone()
        } else if right == &self.one {
            left.clone()
        } else if left == &self.minus_one {
            -right.clone()
        } else if right == &self.minus_one {
            -left.clone()
        } else {
            left * right
        };
        self.check_existing(&output)?;
        Ok(output)
    }

    fn subtract(
        &mut self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, ExactSparseEliminationError> {
        self.check_existing(left)?;
        self.check_existing(right)?;
        check_pair_product_work(sum_pair_work(left, right), self.config)?;
        check_coefficient_degree(coefficient_sum_degree_bound(left, right), self.config)?;
        check_dense_bound(sum_dense_bound(left, right), self.config)?;
        let output = left - right;
        self.check_existing(&output)?;
        Ok(output)
    }

    fn divide(
        &mut self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, ExactSparseEliminationError> {
        if right.is_zero() {
            return Err(ExactSparseEliminationError::InvalidCertificate {
                ordinal: None,
                reason: "attempted division by zero",
            });
        }
        self.check_existing(left)?;
        self.check_existing(right)?;
        check_pair_product_work(quotient_pair_work(left, right), self.config)?;
        check_coefficient_degree(coefficient_quotient_degree_bound(left, right), self.config)?;
        check_dense_bound(quotient_dense_bound(left, right), self.config)?;
        let output = if right == &self.one {
            left.clone()
        } else {
            left / right
        };
        self.check_existing(&output)?;
        Ok(output)
    }
}

struct InputCensus {
    entries: usize,
    maximum_row_width: usize,
    maximum_degree: u128,
    checksum: u64,
}

#[derive(Default)]
struct CoefficientValidationBudget {
    canonicalization_work: usize,
}

fn validate_config(
    config: ExactSparseEliminationConfig,
) -> Result<(), ExactSparseEliminationError> {
    if config.max_coefficient_degree as u128 > SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT {
        return Err(ExactSparseEliminationError::ResourceLimit {
            resource: "configured coefficient exponent degree",
            requested: config.max_coefficient_degree as u128,
            limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
        });
    }
    Ok(())
}

fn validate_exact_coefficient(
    context: &CoefficientContext,
    coefficient: &Coefficient,
    location: ExactSparseCoefficientLocation,
    config: ExactSparseEliminationConfig,
    term_resource: &'static str,
    budget: &mut CoefficientValidationBudget,
) -> Result<(), ExactSparseEliminationError> {
    let template = context.zero();
    let expected_variables = template.get_variables().as_ref();
    validate_polynomial_structure(
        &coefficient.numerator,
        expected_variables,
        location,
        "numerator variable map differs from the coefficient context",
        "numerator exponent layout does not match its term and variable census",
        "numerator contains a noncanonical integer coefficient",
        "numerator contains an explicit zero term",
        "numerator monomials are not strictly ordered",
    )?;
    validate_polynomial_structure(
        &coefficient.denominator,
        expected_variables,
        location,
        "denominator variable map differs from the coefficient context",
        "denominator exponent layout does not match its term and variable census",
        "denominator contains a noncanonical integer coefficient",
        "denominator contains an explicit zero term",
        "denominator monomials are not strictly ordered",
    )?;
    if coefficient.denominator.is_zero() {
        return Err(ExactSparseEliminationError::MalformedCoefficient {
            location,
            reason: "denominator is zero",
        });
    }

    let degree = coefficient_maximum_degree(coefficient);
    check_coefficient_degree(degree, config)?;
    let terms = coefficient
        .numerator
        .nterms()
        .max(coefficient.denominator.nterms());
    check_resource(term_resource, terms, config.max_coefficient_operation_terms)?;
    check_dense_bound(existing_dense_bound(coefficient), config)?;
    check_integer_bits(coefficient, config)?;
    let requested = checked_add(
        budget.canonicalization_work,
        canonicalization_work_bound(coefficient)?,
        "coefficient canonicalization work",
    )?;
    check_resource(
        "coefficient canonicalization work",
        requested,
        config.max_canonicalization_work,
    )?;
    budget.canonicalization_work = requested;

    let canonical =
        <Coefficient as FromNumeratorAndDenominator<IntegerRing, IntegerRing, u16>>::from_num_den(
            coefficient.numerator.clone(),
            coefficient.denominator.clone(),
            &Z,
            true,
        );
    if &canonical != coefficient {
        return Err(ExactSparseEliminationError::MalformedCoefficient {
            location,
            reason: "rational polynomial is not in canonical reduced form",
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_polynomial_structure(
    polynomial: &MultivariatePolynomial<IntegerRing, u16>,
    expected_variables: &[PolyVariable],
    location: ExactSparseCoefficientLocation,
    variable_map_reason: &'static str,
    exponent_layout_reason: &'static str,
    integer_reason: &'static str,
    zero_term_reason: &'static str,
    ordering_reason: &'static str,
) -> Result<(), ExactSparseEliminationError> {
    if polynomial.variables.as_ref() != expected_variables {
        return Err(ExactSparseEliminationError::MalformedCoefficient {
            location,
            reason: variable_map_reason,
        });
    }
    let expected_exponents = polynomial
        .coefficients
        .len()
        .checked_mul(expected_variables.len())
        .ok_or(ExactSparseEliminationError::MalformedCoefficient {
            location,
            reason: exponent_layout_reason,
        })?;
    if polynomial.exponents.len() != expected_exponents {
        return Err(ExactSparseEliminationError::MalformedCoefficient {
            location,
            reason: exponent_layout_reason,
        });
    }
    for coefficient in &polynomial.coefficients {
        if !integer_is_canonical(coefficient) {
            return Err(ExactSparseEliminationError::MalformedCoefficient {
                location,
                reason: integer_reason,
            });
        }
        if coefficient.is_zero() {
            return Err(ExactSparseEliminationError::MalformedCoefficient {
                location,
                reason: zero_term_reason,
            });
        }
    }
    let variables = expected_variables.len();
    for term in 1..polynomial.coefficients.len() {
        let previous_start = (term - 1) * variables;
        let current_start = term * variables;
        if polynomial.exponents[previous_start..current_start]
            >= polynomial.exponents[current_start..current_start + variables]
        {
            return Err(ExactSparseEliminationError::MalformedCoefficient {
                location,
                reason: ordering_reason,
            });
        }
    }
    Ok(())
}

fn integer_is_canonical(value: &Integer) -> bool {
    match value {
        Integer::Single(_) => true,
        Integer::Double(number) => matches!(Integer::from(*number), Integer::Double(_)),
        Integer::Large(number) => matches!(Integer::from(number.clone()), Integer::Large(_)),
    }
}

fn integer_bit_length(value: &Integer) -> u128 {
    match value {
        Integer::Single(0) => 0,
        Integer::Single(number) => u128::from(number.unsigned_abs().ilog2() + 1),
        Integer::Double(0) => 0,
        Integer::Double(number) => u128::from(number.unsigned_abs().ilog2() + 1),
        Integer::Large(number) => u128::from(number.significant_bits()),
    }
}

fn check_integer_bits(
    coefficient: &Coefficient,
    config: ExactSparseEliminationConfig,
) -> Result<(), ExactSparseEliminationError> {
    let requested = coefficient
        .numerator
        .coefficients
        .iter()
        .chain(&coefficient.denominator.coefficients)
        .map(integer_bit_length)
        .max()
        .unwrap_or(0);
    if requested > config.max_integer_bits as u128 {
        return Err(ExactSparseEliminationError::ResourceLimit {
            resource: "coefficient integer bits",
            requested,
            limit: config.max_integer_bits as u128,
        });
    }
    Ok(())
}

fn term_pair_product(left: usize, right: usize) -> u128 {
    (left as u128).saturating_mul(right as u128)
}

fn product_pair_work(left: &Coefficient, right: &Coefficient) -> u128 {
    term_pair_product(left.numerator.nterms(), right.numerator.nterms())
        .saturating_add(term_pair_product(
            left.denominator.nterms(),
            right.denominator.nterms(),
        ))
        .saturating_add(term_pair_product(
            left.numerator.nterms(),
            right.denominator.nterms(),
        ))
        .saturating_add(term_pair_product(
            left.denominator.nterms(),
            right.numerator.nterms(),
        ))
}

fn sum_pair_work(left: &Coefficient, right: &Coefficient) -> u128 {
    term_pair_product(left.denominator.nterms(), right.denominator.nterms())
        .saturating_add(term_pair_product(
            left.numerator.nterms(),
            right.denominator.nterms(),
        ))
        .saturating_add(term_pair_product(
            right.numerator.nterms(),
            left.denominator.nterms(),
        ))
}

fn quotient_pair_work(left: &Coefficient, right: &Coefficient) -> u128 {
    product_pair_work(left, right)
}

fn check_pair_product_work(
    requested: u128,
    config: ExactSparseEliminationConfig,
) -> Result<(), ExactSparseEliminationError> {
    if requested > config.max_coefficient_pair_products as u128 {
        return Err(ExactSparseEliminationError::ResourceLimit {
            resource: "coefficient term-pair products",
            requested,
            limit: config.max_coefficient_pair_products as u128,
        });
    }
    Ok(())
}

fn canonicalization_work_bound(
    coefficient: &Coefficient,
) -> Result<usize, ExactSparseEliminationError> {
    let term_pairs = term_pair_product(
        coefficient.numerator.nterms(),
        coefficient.denominator.nterms(),
    );
    let degree = coefficient_maximum_degree(coefficient).saturating_add(1);
    usize::try_from(term_pairs.saturating_mul(degree)).map_err(|_| {
        ExactSparseEliminationError::ArithmeticOverflow {
            resource: "coefficient canonicalization work",
        }
    })
}

fn validate_source(
    context: &CoefficientContext,
    source_rows: &[ExactSparseRow],
    column_count: usize,
    config: ExactSparseEliminationConfig,
) -> Result<InputCensus, ExactSparseEliminationError> {
    check_resource("source rows", source_rows.len(), config.max_rows)?;
    check_resource("columns", column_count, config.max_columns)?;

    let mut entries = 0_usize;
    let mut maximum_row_width = 0_usize;
    let mut maximum_degree = 0_u128;
    let mut serialized_bytes = 0_usize;
    let mut coefficient_budget = CoefficientValidationBudget::default();
    let mut checksum = FNV1A64_OFFSET;
    hash_length_prefixed(&mut checksum, EXACT_SPARSE_ELIMINATION_SCHEMA.as_bytes());
    for name in context.parameter_names() {
        hash_length_prefixed(&mut checksum, name.as_bytes());
    }
    hash_usize(&mut checksum, source_rows.len());
    hash_usize(&mut checksum, column_count);

    for (row_index, row) in source_rows.iter().enumerate() {
        maximum_row_width = maximum_row_width.max(row.len());
        entries = checked_add(entries, row.len(), "input entries")?;
        check_resource("input entries", entries, config.max_input_entries)?;
        hash_usize(&mut checksum, row_index);
        hash_usize(&mut checksum, row.len());
        for (&column, coefficient) in row {
            if column >= column_count {
                return Err(ExactSparseEliminationError::ColumnOutOfRange {
                    row_index,
                    column,
                    column_count,
                });
            }
            validate_exact_coefficient(
                context,
                coefficient,
                ExactSparseCoefficientLocation::SourceEntry { row_index, column },
                config,
                "input coefficient terms",
                &mut coefficient_budget,
            )?;
            if coefficient.is_zero() {
                return Err(ExactSparseEliminationError::ExplicitZeroCoefficient {
                    row_index,
                    column,
                });
            }
            let degree = coefficient_maximum_degree(coefficient);
            maximum_degree = maximum_degree.max(degree);
            hash_usize(&mut checksum, column);
            let bytes = hash_display_bounded(
                &mut checksum,
                coefficient,
                serialized_bytes,
                config.max_input_coefficient_bytes,
                "input coefficient bytes",
            )?;
            serialized_bytes = checked_add(serialized_bytes, bytes, "input coefficient bytes")?;
        }
    }
    Ok(InputCensus {
        entries,
        maximum_row_width,
        maximum_degree,
        checksum,
    })
}

fn validate_skeleton(
    skeleton: &[(usize, usize)],
    source_row_count: usize,
    column_count: usize,
    config: ExactSparseEliminationConfig,
) -> Result<(), ExactSparseEliminationError> {
    check_resource("pivot skeleton", skeleton.len(), config.max_rows)?;
    check_resource("pivot skeleton", skeleton.len(), config.max_columns)?;
    let mut source_rows = BTreeSet::new();
    let mut columns = BTreeSet::new();
    let mut previous_column = None;
    for (ordinal, &(source_row_index, pivot_column)) in skeleton.iter().enumerate() {
        if source_row_index >= source_row_count {
            return Err(ExactSparseEliminationError::SkeletonSourceRowOutOfRange {
                ordinal,
                source_row_index,
                source_row_count,
            });
        }
        if pivot_column >= column_count {
            return Err(ExactSparseEliminationError::SkeletonPivotColumnOutOfRange {
                ordinal,
                pivot_column,
                column_count,
            });
        }
        if !source_rows.insert(source_row_index) {
            return Err(ExactSparseEliminationError::DuplicateSkeletonSourceRow {
                ordinal,
                source_row_index,
            });
        }
        if !columns.insert(pivot_column) {
            return Err(ExactSparseEliminationError::DuplicateSkeletonPivotColumn {
                ordinal,
                pivot_column,
            });
        }
        if let Some(previous_pivot_column) = previous_column {
            if pivot_column >= previous_pivot_column {
                return Err(ExactSparseEliminationError::SkeletonNotHardestFirst {
                    ordinal,
                    previous_pivot_column,
                    pivot_column,
                });
            }
        }
        previous_column = Some(pivot_column);
    }
    Ok(())
}

fn derive_pivot_rule(
    ordinal: usize,
    source_row_index: usize,
    expected_pivot_column: usize,
    source_rows: &[ExactSparseRow],
    prior_rules: &[ExactSparsePivotRule],
    work: &mut WorkBudget,
    arithmetic: &mut CheckedArithmetic,
) -> Result<ExactSparsePivotRule, ExactSparseEliminationError> {
    let mut row = source_rows[source_row_index].clone();
    work.observe_row(&row);
    let reductions = reduce_through_rules(&mut row, prior_rules, work, arithmetic, true)?;
    let actual_hardest_column = row.keys().next_back().copied();
    if actual_hardest_column != Some(expected_pivot_column) {
        return Err(ExactSparseEliminationError::ExpectedPivotMismatch {
            ordinal,
            source_row_index,
            expected_pivot_column,
            actual_hardest_column,
        });
    }
    let divisor = row.get(&expected_pivot_column).cloned().ok_or(
        ExactSparseEliminationError::ZeroPivotDivisor {
            ordinal,
            source_row_index,
            pivot_column: expected_pivot_column,
        },
    )?;
    if divisor.is_zero() {
        return Err(ExactSparseEliminationError::ZeroPivotDivisor {
            ordinal,
            source_row_index,
            pivot_column: expected_pivot_column,
        });
    }
    normalize_row(&mut row, expected_pivot_column, &divisor, work, arithmetic)?;
    if row.get(&expected_pivot_column) != Some(&arithmetic.one) {
        return Err(ExactSparseEliminationError::NonUnitPivot {
            ordinal,
            pivot_column: expected_pivot_column,
        });
    }
    work.observe_row(&row);
    Ok(ExactSparsePivotRule {
        ordinal,
        source_row_index,
        pivot_column: expected_pivot_column,
        row,
        trace: ExactSparseDerivationTrace {
            base_source_row_index: source_row_index,
            reductions,
            divisor,
        },
    })
}

fn reduce_through_rules(
    row: &mut ExactSparseRow,
    rules: &[ExactSparsePivotRule],
    work: &mut WorkBudget,
    arithmetic: &mut CheckedArithmetic,
    record_trace: bool,
) -> Result<Vec<ExactSparseDerivationReduction>, ExactSparseEliminationError> {
    let mut trace = Vec::new();
    for rule in rules {
        let Some(factor) = row.get(&rule.pivot_column).cloned() else {
            continue;
        };
        if factor.is_zero() {
            return Err(ExactSparseEliminationError::InvalidCertificate {
                ordinal: Some(rule.ordinal),
                reason: "a working row retained an explicit zero pivot factor",
            });
        }
        work.charge_reduction()?;
        work.charge_update()?;
        row.remove(&rule.pivot_column);
        for (&column, pivot_coefficient) in &rule.row {
            if column == rule.pivot_column {
                continue;
            }
            work.charge_update()?;
            let delta = arithmetic.multiply(&factor, pivot_coefficient)?;
            if let Some(current) = row.remove(&column) {
                if current != delta {
                    let updated = arithmetic.subtract(&current, &delta)?;
                    if !updated.is_zero() {
                        row.insert(column, updated);
                    }
                }
            } else {
                let updated = -delta;
                arithmetic.check_existing(&updated)?;
                if !updated.is_zero() {
                    row.insert(column, updated);
                }
            }
        }
        work.observe_row(row);
        if record_trace {
            trace.push(ExactSparseDerivationReduction {
                prior_pivot_ordinal: rule.ordinal,
                factor,
            });
        }
    }
    Ok(trace)
}

fn normalize_row(
    row: &mut ExactSparseRow,
    pivot_column: usize,
    divisor: &Coefficient,
    work: &mut WorkBudget,
    arithmetic: &mut CheckedArithmetic,
) -> Result<(), ExactSparseEliminationError> {
    let columns = row.keys().copied().collect::<Vec<_>>();
    for column in columns {
        work.charge_update()?;
        if column == pivot_column {
            row.insert(column, arithmetic.one.clone());
            continue;
        }
        let value = row
            .remove(&column)
            .ok_or(ExactSparseEliminationError::InvalidCertificate {
                ordinal: None,
                reason: "normalization lost a sparse row entry",
            })?;
        let normalized = arithmetic.divide(&value, divisor)?;
        if !normalized.is_zero() {
            row.insert(column, normalized);
        }
    }
    Ok(())
}

fn free_column_complement(
    column_count: usize,
    rules: &[ExactSparsePivotRule],
) -> Result<Vec<usize>, ExactSparseEliminationError> {
    let pivots = rules
        .iter()
        .map(|rule| rule.pivot_column)
        .collect::<BTreeSet<_>>();
    if pivots.len() != rules.len() {
        return Err(ExactSparseEliminationError::InvalidCertificate {
            ordinal: None,
            reason: "pivot columns are not unique",
        });
    }
    Ok((0..column_count)
        .filter(|column| !pivots.contains(column))
        .collect())
}

struct ProofCensus {
    derivation_reductions: usize,
    derivation_updates: usize,
    verification_reductions: usize,
    total_updates: usize,
    maximum_row_width: usize,
    maximum_degree: u128,
}

#[allow(clippy::too_many_arguments)]
fn prove_certificate(
    context: &CoefficientContext,
    source_rows: &[ExactSparseRow],
    column_count: usize,
    stored_rules: &[ExactSparsePivotRule],
    free_columns: &[usize],
    config: ExactSparseEliminationConfig,
    initial_maximum_row_width: usize,
    initial_maximum_degree: u128,
) -> Result<ProofCensus, ExactSparseEliminationError> {
    validate_stored_certificate(
        context,
        source_rows.len(),
        column_count,
        stored_rules,
        free_columns,
        config,
    )?;
    let mut work = WorkBudget::replay(config, initial_maximum_row_width);
    let mut arithmetic = CheckedArithmetic::new(context, config, initial_maximum_degree)?;
    let mut reconstructed = Vec::new();
    reconstructed
        .try_reserve_exact(stored_rules.len())
        .map_err(|_| ExactSparseEliminationError::ResourceLimit {
            resource: "replayed pivot rows",
            requested: stored_rules.len() as u128,
            limit: config.max_rows as u128,
        })?;
    for stored in stored_rules {
        let rule = derive_pivot_rule(
            stored.ordinal,
            stored.source_row_index,
            stored.pivot_column,
            source_rows,
            &reconstructed,
            &mut work,
            &mut arithmetic,
        )?;
        if &rule != stored {
            return Err(ExactSparseEliminationError::TraceMismatch {
                ordinal: stored.ordinal,
            });
        }
        reconstructed.push(rule);
    }
    let derivation_reductions = work.reductions;
    let derivation_updates = work.updates;

    for (source_row_index, source) in source_rows.iter().enumerate() {
        let mut row = source.clone();
        work.observe_row(&row);
        reduce_through_rules(&mut row, &reconstructed, &mut work, &mut arithmetic, false)?;
        if let Some(&hardest_remaining_column) = row.keys().next_back() {
            return Err(ExactSparseEliminationError::IncompleteSkeleton {
                source_row_index,
                hardest_remaining_column,
            });
        }
    }
    let verification_reductions = work.reductions.checked_sub(derivation_reductions).ok_or(
        ExactSparseEliminationError::ArithmeticOverflow {
            resource: "verification reductions",
        },
    )?;
    Ok(ProofCensus {
        derivation_reductions,
        derivation_updates,
        verification_reductions,
        total_updates: work.updates,
        maximum_row_width: work.maximum_row_width,
        maximum_degree: arithmetic.maximum_degree,
    })
}

fn validate_stored_certificate(
    context: &CoefficientContext,
    source_row_count: usize,
    column_count: usize,
    rules: &[ExactSparsePivotRule],
    free_columns: &[usize],
    config: ExactSparseEliminationConfig,
) -> Result<(), ExactSparseEliminationError> {
    let one = context.one();
    let mut prior_source_rows = BTreeSet::new();
    let mut prior_pivots = BTreeSet::new();
    let mut previous_pivot = None;
    let mut coefficient_budget = CoefficientValidationBudget::default();
    for (ordinal, rule) in rules.iter().enumerate() {
        if rule.ordinal != ordinal {
            return Err(ExactSparseEliminationError::InvalidCertificate {
                ordinal: Some(ordinal),
                reason: "stored ordinal does not match rule order",
            });
        }
        if rule.source_row_index >= source_row_count
            || rule.trace.base_source_row_index != rule.source_row_index
        {
            return Err(ExactSparseEliminationError::InvalidCertificate {
                ordinal: Some(ordinal),
                reason: "trace base does not identify the pivot source row",
            });
        }
        if rule.pivot_column >= column_count {
            return Err(ExactSparseEliminationError::InvalidCertificate {
                ordinal: Some(ordinal),
                reason: "pivot column is outside the column catalog",
            });
        }
        if !prior_source_rows.insert(rule.source_row_index)
            || !prior_pivots.insert(rule.pivot_column)
        {
            return Err(ExactSparseEliminationError::InvalidCertificate {
                ordinal: Some(ordinal),
                reason: "source row or pivot column is repeated",
            });
        }
        if previous_pivot.is_some_and(|previous| rule.pivot_column >= previous) {
            return Err(ExactSparseEliminationError::InvalidCertificate {
                ordinal: Some(ordinal),
                reason: "pivot columns are not strictly hardest-first",
            });
        }
        previous_pivot = Some(rule.pivot_column);
        if rule.row.keys().next_back().copied() != Some(rule.pivot_column)
            || rule.row.get(&rule.pivot_column) != Some(&one)
        {
            return Err(ExactSparseEliminationError::InvalidCertificate {
                ordinal: Some(ordinal),
                reason: "stored row does not have the advertised unit hardest pivot",
            });
        }
        for (&column, coefficient) in &rule.row {
            if column >= column_count {
                return Err(ExactSparseEliminationError::InvalidCertificate {
                    ordinal: Some(ordinal),
                    reason: "stored row has an out-of-range entry",
                });
            }
            validate_exact_coefficient(
                context,
                coefficient,
                ExactSparseCoefficientLocation::PivotEntry {
                    pivot_ordinal: ordinal,
                    column,
                },
                config,
                "stored coefficient operand terms",
                &mut coefficient_budget,
            )?;
            if coefficient.is_zero() {
                return Err(ExactSparseEliminationError::InvalidCertificate {
                    ordinal: Some(ordinal),
                    reason: "stored row has an explicit-zero entry",
                });
            }
        }
        validate_exact_coefficient(
            context,
            &rule.trace.divisor,
            ExactSparseCoefficientLocation::TraceDivisor {
                pivot_ordinal: ordinal,
            },
            config,
            "stored coefficient operand terms",
            &mut coefficient_budget,
        )?;
        if rule.trace.divisor.is_zero() {
            return Err(ExactSparseEliminationError::InvalidCertificate {
                ordinal: Some(ordinal),
                reason: "trace divisor is zero",
            });
        }
        let mut previous_reduction = None;
        for (reduction_index, reduction) in rule.trace.reductions.iter().enumerate() {
            validate_exact_coefficient(
                context,
                &reduction.factor,
                ExactSparseCoefficientLocation::TraceReductionFactor {
                    pivot_ordinal: ordinal,
                    reduction_index,
                },
                config,
                "stored coefficient operand terms",
                &mut coefficient_budget,
            )?;
            if reduction.prior_pivot_ordinal >= ordinal
                || previous_reduction
                    .is_some_and(|previous| reduction.prior_pivot_ordinal <= previous)
                || reduction.factor.is_zero()
            {
                return Err(ExactSparseEliminationError::InvalidCertificate {
                    ordinal: Some(ordinal),
                    reason: "trace reduction is not a strict nonzero prior-pivot reference",
                });
            }
            previous_reduction = Some(reduction.prior_pivot_ordinal);
        }
    }

    if free_columns.windows(2).any(|pair| pair[0] >= pair[1])
        || free_columns.iter().any(|&column| column >= column_count)
    {
        return Err(ExactSparseEliminationError::InvalidCertificate {
            ordinal: None,
            reason: "free columns are not a strictly ordered in-range list",
        });
    }
    let expected_free = (0..column_count)
        .filter(|column| !prior_pivots.contains(column))
        .collect::<Vec<_>>();
    if free_columns != expected_free {
        return Err(ExactSparseEliminationError::InvalidCertificate {
            ordinal: None,
            reason: "pivot and free columns do not partition the column catalog",
        });
    }
    Ok(())
}

#[derive(Default)]
struct RetainedCensus {
    entries: usize,
    terms: usize,
    bytes: usize,
    maximum_degree: u128,
    coefficient_validation: CoefficientValidationBudget,
}

fn charge_retained(
    rules: &[ExactSparsePivotRule],
    context: &CoefficientContext,
    config: ExactSparseEliminationConfig,
) -> Result<RetainedCensus, ExactSparseEliminationError> {
    let mut census = RetainedCensus::default();
    for rule in rules {
        charge_retained_rule(&mut census, rule, context, config)?;
    }
    Ok(census)
}

fn charge_retained_rule(
    census: &mut RetainedCensus,
    rule: &ExactSparsePivotRule,
    context: &CoefficientContext,
    config: ExactSparseEliminationConfig,
) -> Result<(), ExactSparseEliminationError> {
    for (&column, coefficient) in &rule.row {
        charge_retained_coefficient(
            census,
            coefficient,
            context,
            ExactSparseCoefficientLocation::PivotEntry {
                pivot_ordinal: rule.ordinal,
                column,
            },
            config,
        )?;
    }
    for (reduction_index, reduction) in rule.trace.reductions.iter().enumerate() {
        charge_retained_coefficient(
            census,
            &reduction.factor,
            context,
            ExactSparseCoefficientLocation::TraceReductionFactor {
                pivot_ordinal: rule.ordinal,
                reduction_index,
            },
            config,
        )?;
    }
    charge_retained_coefficient(
        census,
        &rule.trace.divisor,
        context,
        ExactSparseCoefficientLocation::TraceDivisor {
            pivot_ordinal: rule.ordinal,
        },
        config,
    )
}

fn charge_retained_coefficient(
    census: &mut RetainedCensus,
    coefficient: &Coefficient,
    context: &CoefficientContext,
    location: ExactSparseCoefficientLocation,
    config: ExactSparseEliminationConfig,
) -> Result<(), ExactSparseEliminationError> {
    validate_exact_coefficient(
        context,
        coefficient,
        location,
        config,
        "retained coefficient operand terms",
        &mut census.coefficient_validation,
    )?;
    let next_entries = checked_add(census.entries, 1, "retained coefficient entries")?;
    check_resource(
        "retained coefficient entries",
        next_entries,
        config.max_retained_entries,
    )?;
    let coefficient_terms = coefficient
        .numerator
        .nterms()
        .checked_add(coefficient.denominator.nterms())
        .ok_or(ExactSparseEliminationError::ArithmeticOverflow {
            resource: "retained coefficient terms",
        })?;
    let next_terms = checked_add(
        census.terms,
        coefficient_terms,
        "retained coefficient terms",
    )?;
    check_resource(
        "retained coefficient terms",
        next_terms,
        config.max_retained_coefficient_terms,
    )?;
    let bytes = bounded_display_len(
        coefficient,
        census.bytes,
        config.max_retained_coefficient_bytes,
        "retained coefficient bytes",
    )?;
    let next_bytes = checked_add(census.bytes, bytes, "retained coefficient bytes")?;
    check_resource(
        "retained coefficient bytes",
        next_bytes,
        config.max_retained_coefficient_bytes,
    )?;
    census.entries = next_entries;
    census.terms = next_terms;
    census.bytes = next_bytes;
    census.maximum_degree = census
        .maximum_degree
        .max(coefficient_maximum_degree(coefficient));
    Ok(())
}

fn coefficient_maximum_degree(coefficient: &Coefficient) -> u128 {
    coefficient_variable_degrees(coefficient)
        .into_iter()
        .map(|(numerator, denominator)| numerator.max(denominator))
        .max()
        .unwrap_or(0)
}

fn check_coefficient_degree(
    requested: u128,
    config: ExactSparseEliminationConfig,
) -> Result<(), ExactSparseEliminationError> {
    let limit = (config.max_coefficient_degree as u128).min(SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT);
    if requested > limit || !symbolica_coefficient_degree_is_representable(requested) {
        return Err(ExactSparseEliminationError::ResourceLimit {
            resource: "Symbolica coefficient exponent degree",
            requested,
            limit,
        });
    }
    Ok(())
}

fn coefficient_quotient_degree_bound(left: &Coefficient, right: &Coefficient) -> u128 {
    if left.get_variables() != right.get_variables() {
        return u128::MAX;
    }
    coefficient_variable_degrees(left)
        .into_iter()
        .zip(coefficient_variable_degrees(right))
        .map(
            |((left_numerator, left_denominator), (right_numerator, right_denominator))| {
                left_numerator
                    .saturating_add(right_denominator)
                    .max(left_denominator.saturating_add(right_numerator))
            },
        )
        .max()
        .unwrap_or(0)
}

fn dense_monomial_bound(degrees: impl IntoIterator<Item = u128>) -> u128 {
    degrees.into_iter().fold(1_u128, |count, degree| {
        count.saturating_mul(degree.saturating_add(1))
    })
}

fn existing_dense_bound(value: &Coefficient) -> u128 {
    let degrees = coefficient_variable_degrees(value);
    dense_monomial_bound(degrees.iter().map(|&(numerator, _)| numerator)).max(dense_monomial_bound(
        degrees.iter().map(|&(_, denominator)| denominator),
    ))
}

fn product_dense_bound(left: &Coefficient, right: &Coefficient) -> u128 {
    let left = coefficient_variable_degrees(left);
    let right = coefficient_variable_degrees(right);
    dense_monomial_bound(left.iter().zip(&right).map(
        |(&(left_numerator, _), &(right_numerator, _))| {
            left_numerator.saturating_add(right_numerator)
        },
    ))
    .max(dense_monomial_bound(left.iter().zip(&right).map(
        |(&(_, left_denominator), &(_, right_denominator))| {
            left_denominator.saturating_add(right_denominator)
        },
    )))
}

fn sum_dense_bound(left: &Coefficient, right: &Coefficient) -> u128 {
    let left = coefficient_variable_degrees(left);
    let right = coefficient_variable_degrees(right);
    dense_monomial_bound(left.iter().zip(&right).map(
        |(&(left_numerator, left_denominator), &(right_numerator, right_denominator))| {
            left_numerator
                .saturating_add(right_denominator)
                .max(right_numerator.saturating_add(left_denominator))
        },
    ))
    .max(dense_monomial_bound(left.iter().zip(&right).map(
        |(&(_, left_denominator), &(_, right_denominator))| {
            left_denominator.saturating_add(right_denominator)
        },
    )))
}

fn quotient_dense_bound(left: &Coefficient, right: &Coefficient) -> u128 {
    let left = coefficient_variable_degrees(left);
    let right = coefficient_variable_degrees(right);
    dense_monomial_bound(left.iter().zip(&right).map(
        |(&(left_numerator, _), &(_, right_denominator))| {
            left_numerator.saturating_add(right_denominator)
        },
    ))
    .max(dense_monomial_bound(left.iter().zip(&right).map(
        |(&(_, left_denominator), &(right_numerator, _))| {
            left_denominator.saturating_add(right_numerator)
        },
    )))
}

fn check_dense_bound(
    requested: u128,
    config: ExactSparseEliminationConfig,
) -> Result<(), ExactSparseEliminationError> {
    if requested > config.max_coefficient_dense_terms as u128 {
        return Err(ExactSparseEliminationError::ResourceLimit {
            resource: "coefficient dense operand/result universe",
            requested,
            limit: config.max_coefficient_dense_terms as u128,
        });
    }
    Ok(())
}

fn checked_add(
    current: usize,
    addend: usize,
    resource: &'static str,
) -> Result<usize, ExactSparseEliminationError> {
    current
        .checked_add(addend)
        .ok_or(ExactSparseEliminationError::ArithmeticOverflow { resource })
}

fn check_resource(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ExactSparseEliminationError> {
    if requested > limit {
        return Err(ExactSparseEliminationError::ResourceLimit {
            resource,
            requested: requested as u128,
            limit: limit as u128,
        });
    }
    Ok(())
}

fn certificate_checksum(
    context: &CoefficientContext,
    config: ExactSparseEliminationConfig,
    source_row_count: usize,
    column_count: usize,
    source_checksum: u64,
    rules: &[ExactSparsePivotRule],
    free_columns: &[usize],
    stats: ExactSparseEliminationStats,
) -> u64 {
    let mut hash = FNV1A64_OFFSET;
    hash_length_prefixed(&mut hash, EXACT_SPARSE_ELIMINATION_SCHEMA.as_bytes());
    for name in context.parameter_names() {
        hash_length_prefixed(&mut hash, name.as_bytes());
    }
    hash_usize(&mut hash, source_row_count);
    hash_usize(&mut hash, column_count);
    hash_u64(&mut hash, source_checksum);
    hash_config(&mut hash, config);
    hash_usize(&mut hash, rules.len());
    for rule in rules {
        hash_usize(&mut hash, rule.ordinal);
        hash_usize(&mut hash, rule.source_row_index);
        hash_usize(&mut hash, rule.pivot_column);
        hash_usize(&mut hash, rule.row.len());
        for (&column, coefficient) in &rule.row {
            hash_usize(&mut hash, column);
            hash_display(&mut hash, coefficient);
        }
        hash_usize(&mut hash, rule.trace.base_source_row_index);
        hash_usize(&mut hash, rule.trace.reductions.len());
        for reduction in &rule.trace.reductions {
            hash_usize(&mut hash, reduction.prior_pivot_ordinal);
            hash_display(&mut hash, &reduction.factor);
        }
        hash_display(&mut hash, &rule.trace.divisor);
    }
    hash_usize(&mut hash, free_columns.len());
    for &column in free_columns {
        hash_usize(&mut hash, column);
    }
    hash_stats(&mut hash, stats);
    hash
}

fn hash_config(hash: &mut u64, config: ExactSparseEliminationConfig) {
    for value in [
        config.max_rows,
        config.max_columns,
        config.max_input_entries,
        config.max_input_coefficient_bytes,
        config.max_reductions,
        config.max_updates,
        config.max_retained_entries,
        config.max_retained_coefficient_terms,
        config.max_retained_coefficient_bytes,
        config.max_coefficient_degree,
        config.max_coefficient_operation_terms,
        config.max_coefficient_dense_terms,
        config.max_integer_bits,
        config.max_coefficient_pair_products,
        config.max_canonicalization_work,
        config.max_replay_reductions,
        config.max_replay_updates,
    ] {
        hash_usize(hash, value);
    }
}

fn hash_stats(hash: &mut u64, stats: ExactSparseEliminationStats) {
    for value in [
        stats.source_rows,
        stats.columns,
        stats.input_entries,
        stats.rank,
        stats.free_columns,
        stats.pivot_reductions,
        stats.verification_reductions,
        stats.arithmetic_updates,
        stats.retained_entries,
        stats.retained_coefficient_terms,
        stats.retained_coefficient_bytes,
        stats.maximum_row_width,
        stats.maximum_coefficient_degree,
        stats.replay_reductions,
        stats.replay_updates,
    ] {
        hash_usize(hash, value);
    }
}

fn hash_u64(hash: &mut u64, value: u64) {
    hash_bytes(hash, &value.to_le_bytes());
}

fn hash_usize(hash: &mut u64, value: usize) {
    hash_u64(hash, value as u64);
}

fn hash_length_prefixed(hash: &mut u64, bytes: &[u8]) {
    hash_usize(hash, bytes.len());
    hash_bytes(hash, bytes);
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for &byte in bytes {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(FNV1A64_PRIME);
    }
}

struct HashWriter<'hash> {
    hash: &'hash mut u64,
}

impl fmt::Write for HashWriter<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        hash_bytes(self.hash, value.as_bytes());
        Ok(())
    }
}

fn hash_display(hash: &mut u64, value: &Coefficient) {
    let mut writer = HashWriter { hash };
    write!(&mut writer, "{value}").expect("hash writer is infallible");
    hash_u64(writer.hash, u64::MAX);
}

struct BoundedHashWriter<'hash> {
    hash: &'hash mut u64,
    length: usize,
    limit: usize,
}

impl fmt::Write for BoundedHashWriter<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let next = self.length.checked_add(value.len()).ok_or(fmt::Error)?;
        if next > self.limit {
            return Err(fmt::Error);
        }
        hash_bytes(self.hash, value.as_bytes());
        self.length = next;
        Ok(())
    }
}

fn hash_display_bounded(
    hash: &mut u64,
    value: &Coefficient,
    used: usize,
    total_limit: usize,
    resource: &'static str,
) -> Result<usize, ExactSparseEliminationError> {
    let remaining = total_limit.saturating_sub(used);
    let mut writer = BoundedHashWriter {
        hash,
        length: 0,
        limit: remaining,
    };
    write!(&mut writer, "{value}").map_err(|_| ExactSparseEliminationError::ResourceLimit {
        resource,
        requested: total_limit as u128 + 1,
        limit: total_limit as u128,
    })?;
    hash_u64(writer.hash, u64::MAX);
    Ok(writer.length)
}

struct BoundedLengthWriter {
    length: usize,
    limit: usize,
}

impl fmt::Write for BoundedLengthWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let next = self.length.checked_add(value.len()).ok_or(fmt::Error)?;
        if next > self.limit {
            return Err(fmt::Error);
        }
        self.length = next;
        Ok(())
    }
}

fn bounded_display_len(
    value: &Coefficient,
    used: usize,
    total_limit: usize,
    resource: &'static str,
) -> Result<usize, ExactSparseEliminationError> {
    let remaining = total_limit.saturating_sub(used);
    let mut writer = BoundedLengthWriter {
        length: 0,
        limit: remaining,
    };
    write!(&mut writer, "{value}").map_err(|_| ExactSparseEliminationError::ResourceLimit {
        resource,
        requested: total_limit as u128 + 1,
        limit: total_limit as u128,
    })?;
    Ok(writer.length)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_term(row: &mut ExactSparseRow, column: usize, coefficient: Coefficient) {
        if coefficient.is_zero() {
            return;
        }
        if let Some(current) = row.remove(&column) {
            let sum = &current + &coefficient;
            if !sum.is_zero() {
                row.insert(column, sum);
            }
        } else {
            row.insert(column, coefficient);
        }
    }

    fn add_scaled(target: &mut ExactSparseRow, source: &ExactSparseRow, factor: &Coefficient) {
        for (&column, coefficient) in source {
            add_term(target, column, coefficient * factor);
        }
    }

    fn synthetic_rows(context: &CoefficientContext) -> Vec<ExactSparseRow> {
        let d = context.parameter("d").unwrap();
        let mut first = ExactSparseRow::new();
        first.insert(2, context.integer(2));
        first.insert(0, d.clone());

        // Eliminating six times the first unit row leaves pivot 3 at column 1.
        let mut second = ExactSparseRow::new();
        second.insert(2, context.integer(6));
        second.insert(1, context.integer(3));
        second.insert(0, &context.integer(7) + &(&context.integer(3) * &d));

        let mut dependent = ExactSparseRow::new();
        add_scaled(&mut dependent, &first, &context.integer(2));
        add_scaled(&mut dependent, &second, &context.integer(-1));
        vec![first, second, dependent]
    }

    fn malformed_single_entry_error(
        context: &CoefficientContext,
        coefficient: Coefficient,
    ) -> ExactSparseEliminationError {
        let mut row = ExactSparseRow::new();
        row.insert(0, coefficient);
        ExactSparseElimination::build(
            context,
            &[row],
            1,
            &[(0, 0)],
            ExactSparseEliminationConfig::default(),
        )
        .unwrap_err()
    }

    #[test]
    fn rejects_a_zero_denominator_before_pivot_normalization() {
        let context = CoefficientContext::new(["d"]);
        let mut malformed = context.one();
        malformed.denominator.coefficients.clear();
        malformed.denominator.exponents.clear();
        assert!(matches!(
            malformed_single_entry_error(&context, malformed),
            ExactSparseEliminationError::MalformedCoefficient {
                location: ExactSparseCoefficientLocation::SourceEntry {
                    row_index: 0,
                    column: 0,
                },
                reason: "denominator is zero",
            }
        ));
    }

    #[test]
    fn rejects_a_foreign_denominator_variable_map() {
        let context = CoefficientContext::new(["d"]);
        let foreign = CoefficientContext::new(["x"]);
        let mut malformed = context.one();
        malformed.denominator.variables = foreign.one().denominator.variables;
        assert!(matches!(
            malformed_single_entry_error(&context, malformed),
            ExactSparseEliminationError::MalformedCoefficient {
                location: ExactSparseCoefficientLocation::SourceEntry {
                    row_index: 0,
                    column: 0,
                },
                reason: "denominator variable map differs from the coefficient context",
            }
        ));
    }

    #[test]
    fn rejects_a_malformed_exponent_layout() {
        let context = CoefficientContext::new(["d"]);
        let mut malformed = context.one();
        malformed.numerator.exponents.push(0);
        assert!(matches!(
            malformed_single_entry_error(&context, malformed),
            ExactSparseEliminationError::MalformedCoefficient {
                location: ExactSparseCoefficientLocation::SourceEntry {
                    row_index: 0,
                    column: 0,
                },
                reason: "numerator exponent layout does not match its term and variable census",
            }
        ));
    }

    #[test]
    fn rejects_a_noncanonical_rational_polynomial() {
        let context = CoefficientContext::new(["d"]);
        let twice_d = &context.integer(2) * &context.parameter("d").unwrap();
        let two = context.integer(2);
        let malformed = RationalPolynomial {
            numerator: twice_d.numerator,
            denominator: two.numerator,
        };
        assert!(matches!(
            malformed_single_entry_error(&context, malformed),
            ExactSparseEliminationError::MalformedCoefficient {
                location: ExactSparseCoefficientLocation::SourceEntry {
                    row_index: 0,
                    column: 0,
                },
                reason: "rational polynomial is not in canonical reduced form",
            }
        ));
    }

    #[test]
    fn constructs_unit_rules_and_replays_recursive_provenance() {
        let context = CoefficientContext::new(["d"]);
        let rows = synthetic_rows(&context);
        let original = rows.clone();
        let certificate = ExactSparseElimination::build(
            &context,
            &rows,
            3,
            &[(0, 2), (1, 1)],
            ExactSparseEliminationConfig::default(),
        )
        .unwrap();

        assert_eq!(rows, original);
        assert_eq!(certificate.rank(), 2);
        assert_eq!(certificate.free_columns(), &[0]);
        assert_eq!(certificate.pivot_rows().len(), 2);
        assert_eq!(certificate.traces().len(), 2);
        assert_eq!(
            certificate.pivot_rules()[0].row().get(&2),
            Some(&context.one())
        );
        let second_trace = certificate.pivot_rules()[1].trace();
        assert_eq!(second_trace.base_source_row_index(), 1);
        assert_eq!(second_trace.reductions().len(), 1);
        assert_eq!(second_trace.reductions()[0].prior_pivot_ordinal(), 0);
        assert_eq!(second_trace.reductions()[0].factor(), &context.integer(6));
        assert_eq!(second_trace.divisor(), &context.integer(3));
        certificate.replay(&context, &rows).unwrap();
    }

    #[test]
    fn rejects_a_wrong_expected_hardest_pivot() {
        let context = CoefficientContext::new(["d"]);
        let rows = synthetic_rows(&context);
        let error = ExactSparseElimination::build(
            &context,
            &rows,
            3,
            &[(0, 1)],
            ExactSparseEliminationConfig::default(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ExactSparseEliminationError::ExpectedPivotMismatch {
                expected_pivot_column: 1,
                actual_hardest_column: Some(2),
                ..
            }
        ));
    }

    #[test]
    fn rejects_duplicate_non_descending_and_out_of_range_skeleton_slots() {
        let context = CoefficientContext::new(["d"]);
        let rows = synthetic_rows(&context);
        let config = ExactSparseEliminationConfig::default();
        assert!(matches!(
            ExactSparseElimination::build(&context, &rows, 3, &[(0, 2), (0, 1)], config),
            Err(ExactSparseEliminationError::DuplicateSkeletonSourceRow { ordinal: 1, .. })
        ));
        assert!(matches!(
            ExactSparseElimination::build(&context, &rows, 3, &[(0, 2), (1, 2)], config),
            Err(ExactSparseEliminationError::DuplicateSkeletonPivotColumn { ordinal: 1, .. })
        ));
        assert!(matches!(
            ExactSparseElimination::build(&context, &rows, 3, &[(1, 1), (0, 2)], config),
            Err(ExactSparseEliminationError::SkeletonNotHardestFirst { ordinal: 1, .. })
        ));
        assert!(matches!(
            ExactSparseElimination::build(&context, &rows, 3, &[(rows.len(), 2)], config),
            Err(ExactSparseEliminationError::SkeletonSourceRowOutOfRange { ordinal: 0, .. })
        ));
        assert!(matches!(
            ExactSparseElimination::build(&context, &rows, 3, &[(0, 3)], config),
            Err(ExactSparseEliminationError::SkeletonPivotColumnOutOfRange { ordinal: 0, .. })
        ));
    }

    #[test]
    fn enforces_integer_pair_product_canonicalization_degree_dense_and_replay_limits() {
        let context = CoefficientContext::new(["d"]);
        let rows = synthetic_rows(&context);

        let mut config = ExactSparseEliminationConfig::default();
        config.max_integer_bits = 1;
        assert!(matches!(
            ExactSparseElimination::build(&context, &rows, 3, &[(0, 2), (1, 1)], config),
            Err(ExactSparseEliminationError::ResourceLimit {
                resource: "coefficient integer bits",
                ..
            })
        ));

        let mut config = ExactSparseEliminationConfig::default();
        config.max_canonicalization_work = 0;
        assert!(matches!(
            ExactSparseElimination::build(&context, &rows, 3, &[(0, 2), (1, 1)], config),
            Err(ExactSparseEliminationError::ResourceLimit {
                resource: "coefficient canonicalization work",
                ..
            })
        ));

        let mut config = ExactSparseEliminationConfig::default();
        config.max_coefficient_pair_products = 0;
        assert!(matches!(
            ExactSparseElimination::build(&context, &rows, 3, &[(0, 2), (1, 1)], config),
            Err(ExactSparseEliminationError::ResourceLimit {
                resource: "coefficient term-pair products",
                ..
            })
        ));

        let mut config = ExactSparseEliminationConfig::default();
        config.max_coefficient_degree = 0;
        assert!(matches!(
            ExactSparseElimination::build(&context, &rows, 3, &[(0, 2), (1, 1)], config),
            Err(ExactSparseEliminationError::ResourceLimit {
                resource: "Symbolica coefficient exponent degree",
                ..
            })
        ));

        let mut config = ExactSparseEliminationConfig::default();
        config.max_coefficient_dense_terms = 1;
        assert!(matches!(
            ExactSparseElimination::build(&context, &rows, 3, &[(0, 2), (1, 1)], config),
            Err(ExactSparseEliminationError::ResourceLimit {
                resource: "coefficient dense operand/result universe",
                ..
            })
        ));

        let mut config = ExactSparseEliminationConfig::default();
        config.max_replay_reductions = 0;
        assert!(matches!(
            ExactSparseElimination::build(&context, &rows, 3, &[(0, 2), (1, 1)], config),
            Err(ExactSparseEliminationError::ResourceLimit {
                resource: "elimination reductions",
                ..
            })
        ));
    }

    #[test]
    fn rejects_an_incomplete_skeleton() {
        let context = CoefficientContext::new(["d"]);
        let rows = synthetic_rows(&context);
        let error = ExactSparseElimination::build(
            &context,
            &rows,
            3,
            &[(0, 2)],
            ExactSparseEliminationConfig::default(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ExactSparseEliminationError::IncompleteSkeleton {
                hardest_remaining_column: 1,
                ..
            }
        ));
    }

    #[test]
    fn replay_rejects_source_tampering() {
        let context = CoefficientContext::new(["d"]);
        let rows = synthetic_rows(&context);
        let certificate = ExactSparseElimination::build(
            &context,
            &rows,
            3,
            &[(0, 2), (1, 1)],
            ExactSparseEliminationConfig::default(),
        )
        .unwrap();
        let mut tampered = rows.clone();
        tampered[2].insert(0, context.integer(123));
        assert!(matches!(
            certificate.replay(&context, &tampered),
            Err(ExactSparseEliminationError::SourceChecksumMismatch { .. })
        ));
    }

    #[test]
    fn semantic_payload_comparison_does_not_trust_retained_checksums() {
        let context = CoefficientContext::new(["d"]);
        let rows = synthetic_rows(&context);
        let certificate = ExactSparseElimination::build(
            &context,
            &rows,
            3,
            &[(0, 2), (1, 1)],
            ExactSparseEliminationConfig::default(),
        )
        .unwrap();
        let mut tampered = certificate.clone();

        // Directly corrupt semantic payload while deliberately retaining both
        // diagnostic hashes.  Outer certificate comparison must still reject
        // it without relying on collision resistance from either checksum.
        tampered.pivot_rules[0].row.insert(0, context.integer(123));
        assert_eq!(tampered.source_checksum(), certificate.source_checksum());
        assert_eq!(tampered.checksum(), certificate.checksum());
        assert!(!certificate.has_identical_semantic_payload(&tampered));
    }

    #[test]
    fn replay_rejects_a_malformed_retained_trace_coefficient() {
        let context = CoefficientContext::new(["d"]);
        let rows = synthetic_rows(&context);
        let mut certificate = ExactSparseElimination::build(
            &context,
            &rows,
            3,
            &[(0, 2), (1, 1)],
            ExactSparseEliminationConfig::default(),
        )
        .unwrap();
        certificate.pivot_rules[0]
            .trace
            .divisor
            .denominator
            .coefficients
            .clear();
        certificate.pivot_rules[0]
            .trace
            .divisor
            .denominator
            .exponents
            .clear();
        assert!(matches!(
            certificate.replay(&context, &rows),
            Err(ExactSparseEliminationError::MalformedCoefficient {
                location: ExactSparseCoefficientLocation::TraceDivisor { pivot_ordinal: 0 },
                reason: "denominator is zero",
            })
        ));
    }

    #[test]
    fn retained_entry_limit_is_enforced_before_certificate_return() {
        let context = CoefficientContext::new(["d"]);
        let rows = synthetic_rows(&context);
        let mut config = ExactSparseEliminationConfig::default();
        config.max_retained_entries = 1;
        assert!(matches!(
            ExactSparseElimination::build(&context, &rows, 3, &[(0, 2), (1, 1)], config),
            Err(ExactSparseEliminationError::ResourceLimit {
                resource: "retained coefficient entries",
                ..
            })
        ));
    }
}
