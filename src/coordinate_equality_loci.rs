//! Replayable extraction of exact coordinate equalities from symbolic sector leaves.
//!
//! A recognized predicate has the exact expanded form
//!
//! ```text
//! a(theta) * (n_i - c) = 0    or    a(theta) * (n_i - c) != 0,
//! ```
//!
//! where `a(theta)` is a nonzero element of the formal coefficient field
//! `K = Q(theta)` and `c` is representable by `i64`.  Such an `a(theta)` is a
//! unit in `K`, so the polynomial has exactly the coordinate locus `n_i = c`.
//! The recognizer compares the expanded slope and constant polynomials; it does
//! not factor, sample, or infer radical-ideal equivalence.
//!
//! Equality predicates produce a canonical [`crate::PartialIndexAssignment`].
//! Nonzero predicates are retained as recognized exclusions and can prove a
//! leaf empty when the same leaf also fixes that coordinate to the excluded
//! value.  Conflicting fixed values and values outside the recorded sector
//! orthant are likewise exact empty-leaf proofs.  Every predicate outside this
//! narrow language is retained verbatim as unresolved metadata and never used
//! to claim either a coordinate assignment or emptiness.

use std::collections::BTreeMap;
use std::fmt;
use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::prelude::{EuclideanDomain, Integer, Z};

use crate::{
    ParametricCoefficientContext, ParametricCoefficientError, ParametricPolynomial,
    PartialIndexAssignment, SectorOrthantSide, SymbolicPolynomialPredicateKind,
    SymbolicSectorCaseError, SymbolicSectorCaseId, SymbolicSectorCaseLimits,
    SymbolicSectorCasePartitionCertificate, algebra::ExactAlgebraLimits,
};

pub const COORDINATE_EQUALITY_LOCUS_V1_SCHEMA: &str = "rustred-coordinate-equality-locus-v1";

/// Aggregate work and retained-proof limits for one leaf extraction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoordinateEqualityLocusLimits {
    /// Limits used while replaying the retained source partition.
    pub partition_replay: SymbolicSectorCaseLimits,
    /// Authentication limits for every source polynomial.
    pub exact_algebra: ExactAlgebraLimits,
    pub max_context_fingerprint_bytes: usize,
    pub max_predicates: usize,
    pub max_polynomial_terms_inspected: usize,
    pub max_exponent_entries_inspected: usize,
    pub max_recognition_operations: usize,
    pub max_integer_coefficient_bits: usize,
    pub max_recognized_predicates: usize,
    pub max_unresolved_predicates: usize,
    pub max_assignments: usize,
    pub max_total_witness_ordinals: usize,
    /// Terms retained by the owned source partition plus unresolved copies.
    pub max_retained_polynomial_terms: usize,
    /// Canonical-display bytes retained by the source partition plus unresolved copies.
    pub max_retained_polynomial_bytes: usize,
}

impl Default for CoordinateEqualityLocusLimits {
    fn default() -> Self {
        Self {
            partition_replay: SymbolicSectorCaseLimits::default(),
            exact_algebra: ExactAlgebraLimits::default(),
            max_context_fingerprint_bytes: 1024 * 1024,
            max_predicates: 1_000_000,
            max_polynomial_terms_inspected: 16_000_000,
            max_exponent_entries_inspected: 256_000_000,
            max_recognition_operations: 512_000_000,
            max_integer_coefficient_bits: 1_000_000,
            max_recognized_predicates: 1_000_000,
            max_unresolved_predicates: 1_000_000,
            max_assignments: 1_000_000,
            max_total_witness_ordinals: 8_000_000,
            max_retained_polynomial_terms: 32_000_000,
            max_retained_polynomial_bytes: usize::MAX / 4,
        }
    }
}

/// One source predicate proved associate over `K` to `n_i-c`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoordinateLocusPredicateWitness {
    predicate_ordinal: usize,
    kind: SymbolicPolynomialPredicateKind,
    index: usize,
    value: i64,
}

impl CoordinateLocusPredicateWitness {
    pub fn predicate_ordinal(&self) -> usize {
        self.predicate_ordinal
    }

    pub fn kind(&self) -> SymbolicPolynomialPredicateKind {
        self.kind
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn value(&self) -> i64 {
        self.value
    }
}

/// Exact source ordinals supporting one entry of the partial assignment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoordinateAssignmentWitness {
    index: usize,
    value: i64,
    equality_predicate_ordinals: Box<[usize]>,
}

impl CoordinateAssignmentWitness {
    pub fn index(&self) -> usize {
        self.index
    }

    pub fn value(&self) -> i64 {
        self.value
    }

    pub fn equality_predicate_ordinals(&self) -> &[usize] {
        &self.equality_predicate_ordinals
    }
}

/// A source predicate deliberately left outside the coordinate-locus language.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnresolvedCoordinatePredicate {
    predicate_ordinal: usize,
    kind: SymbolicPolynomialPredicateKind,
    polynomial: ParametricPolynomial,
}

impl UnresolvedCoordinatePredicate {
    pub fn predicate_ordinal(&self) -> usize {
        self.predicate_ordinal
    }

    pub fn kind(&self) -> SymbolicPolynomialPredicateKind {
        self.kind
    }

    pub fn polynomial(&self) -> &ParametricPolynomial {
        &self.polynomial
    }
}

/// Deterministic exact reason why the selected leaf is empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoordinateEqualityEmptyReason {
    ConflictingFixedValues {
        index: usize,
        first_value: i64,
        first_equality_predicate_ordinals: Box<[usize]>,
        second_value: i64,
        second_equality_predicate_ordinals: Box<[usize]>,
    },
    EqualityNonzeroContradiction {
        index: usize,
        value: i64,
        equality_predicate_ordinals: Box<[usize]>,
        nonzero_predicate_ordinals: Box<[usize]>,
    },
    OrthantViolation {
        index: usize,
        value: i64,
        equality_predicate_ordinals: Box<[usize]>,
        side: SectorOrthantSide,
    },
}

/// The extractor proves only the named empty cases.  The other status does not
/// assert that unresolved polynomial conditions have an integer solution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoordinateEqualityLeafStatus {
    NotProvedEmpty,
    ProvedEmpty(CoordinateEqualityEmptyReason),
}

/// Exact census of inspected work and retained certificate data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CoordinateEqualityLocusStats {
    predicates: usize,
    polynomial_terms_inspected: usize,
    exponent_entries_inspected: usize,
    recognition_operations: usize,
    recognized_predicates: usize,
    equality_predicates: usize,
    nonzero_predicates: usize,
    unresolved_predicates: usize,
    assignments: usize,
    total_witness_ordinals: usize,
    retained_polynomial_terms: usize,
    retained_polynomial_bytes: usize,
}

impl CoordinateEqualityLocusStats {
    pub fn predicates(self) -> usize {
        self.predicates
    }
    pub fn polynomial_terms_inspected(self) -> usize {
        self.polynomial_terms_inspected
    }
    pub fn exponent_entries_inspected(self) -> usize {
        self.exponent_entries_inspected
    }
    pub fn recognition_operations(self) -> usize {
        self.recognition_operations
    }
    pub fn recognized_predicates(self) -> usize {
        self.recognized_predicates
    }
    pub fn equality_predicates(self) -> usize {
        self.equality_predicates
    }
    pub fn nonzero_predicates(self) -> usize {
        self.nonzero_predicates
    }
    pub fn unresolved_predicates(self) -> usize {
        self.unresolved_predicates
    }
    pub fn assignments(self) -> usize {
        self.assignments
    }
    pub fn total_witness_ordinals(self) -> usize {
        self.total_witness_ordinals
    }
    pub fn retained_polynomial_terms(self) -> usize {
        self.retained_polynomial_terms
    }
    pub fn retained_polynomial_bytes(self) -> usize {
        self.retained_polynomial_bytes
    }
}

/// Self-owned extraction proof.  Replay starts from the retained partition and
/// selected final case, not from trusted extracted metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoordinateEqualityLocusCertificate {
    schema: &'static str,
    source_partition: SymbolicSectorCasePartitionCertificate,
    source_case: SymbolicSectorCaseId,
    assignment: PartialIndexAssignment,
    assignment_witnesses: Box<[CoordinateAssignmentWitness]>,
    recognized_predicates: Box<[CoordinateLocusPredicateWitness]>,
    unresolved_predicates: Box<[UnresolvedCoordinatePredicate]>,
    status: CoordinateEqualityLeafStatus,
    limits: CoordinateEqualityLocusLimits,
    stats: CoordinateEqualityLocusStats,
}

impl CoordinateEqualityLocusCertificate {
    pub fn schema(&self) -> &'static str {
        self.schema
    }
    pub fn source_partition(&self) -> &SymbolicSectorCasePartitionCertificate {
        &self.source_partition
    }
    pub fn source_case(&self) -> SymbolicSectorCaseId {
        self.source_case
    }

    /// Coordinate equalities retained from the leaf.
    ///
    /// Callers driving conditional elimination must inspect [`Self::status`]
    /// first.  A proved-empty leaf may still expose its non-conflicting
    /// equality consequences (including the equality that violates the sector
    /// orthant); those assignments are provenance, not an instruction to solve
    /// an empty case.
    pub fn assignment(&self) -> &PartialIndexAssignment {
        &self.assignment
    }
    pub fn assignment_witnesses(&self) -> &[CoordinateAssignmentWitness] {
        &self.assignment_witnesses
    }
    pub fn recognized_predicates(&self) -> &[CoordinateLocusPredicateWitness] {
        &self.recognized_predicates
    }
    pub fn unresolved_predicates(&self) -> &[UnresolvedCoordinatePredicate] {
        &self.unresolved_predicates
    }
    pub fn status(&self) -> &CoordinateEqualityLeafStatus {
        &self.status
    }
    pub fn limits(&self) -> CoordinateEqualityLocusLimits {
        self.limits
    }
    pub fn stats(&self) -> CoordinateEqualityLocusStats {
        self.stats
    }
    pub fn is_proved_empty(&self) -> bool {
        matches!(&self.status, CoordinateEqualityLeafStatus::ProvedEmpty(_))
    }

    /// Regenerate the extraction from the retained partition and compare every
    /// assignment, source ordinal, unresolved polynomial, status, limit, and
    /// statistic.
    pub fn replay(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), CoordinateEqualityLocusError> {
        if self.schema != COORDINATE_EQUALITY_LOCUS_V1_SCHEMA {
            return Err(CoordinateEqualityLocusError::SchemaMismatch);
        }
        let replayed = CoordinateEqualityLocusExtractor::extract(
            context,
            &self.source_partition,
            self.source_case,
            self.limits,
        )?;
        if self == &replayed {
            Ok(())
        } else {
            Err(CoordinateEqualityLocusError::ReplayMismatch)
        }
    }
}

/// Stateless exact compiler for one final symbolic sector case.
pub struct CoordinateEqualityLocusExtractor;

impl CoordinateEqualityLocusExtractor {
    pub fn extract(
        context: &ParametricCoefficientContext,
        partition: &SymbolicSectorCasePartitionCertificate,
        case: SymbolicSectorCaseId,
        limits: CoordinateEqualityLocusLimits,
    ) -> Result<CoordinateEqualityLocusCertificate, CoordinateEqualityLocusError> {
        catch_unwind(AssertUnwindSafe(|| {
            extract_inner(context, partition, case, limits)
        }))
        .map_err(|_| CoordinateEqualityLocusError::SymbolicaPanic)?
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoordinateEqualityLocusError {
    SchemaMismatch,
    ReplayMismatch,
    CaseNotFound {
        case: SymbolicSectorCaseId,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    SymbolicaPanic,
    SourcePartition(SymbolicSectorCaseError),
    ParametricCoefficient(ParametricCoefficientError),
}

impl fmt::Display for CoordinateEqualityLocusError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("coordinate-locus schema mismatch"),
            Self::ReplayMismatch => {
                formatter.write_str("coordinate-locus certificate does not replay")
            }
            Self::CaseNotFound { case } => write!(
                formatter,
                "symbolic sector case {case} is not a retained leaf"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::SymbolicaPanic => {
                formatter.write_str("Symbolica panicked during coordinate-locus extraction")
            }
            Self::SourcePartition(error) => error.fmt(formatter),
            Self::ParametricCoefficient(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CoordinateEqualityLocusError {}

impl From<SymbolicSectorCaseError> for CoordinateEqualityLocusError {
    fn from(value: SymbolicSectorCaseError) -> Self {
        Self::SourcePartition(value)
    }
}

impl From<ParametricCoefficientError> for CoordinateEqualityLocusError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::ParametricCoefficient(value)
    }
}

fn extract_inner(
    context: &ParametricCoefficientContext,
    partition: &SymbolicSectorCasePartitionCertificate,
    case_id: SymbolicSectorCaseId,
    limits: CoordinateEqualityLocusLimits,
) -> Result<CoordinateEqualityLocusCertificate, CoordinateEqualityLocusError> {
    check_limit(
        "coordinate-locus context fingerprint bytes",
        context.fingerprint().len(),
        limits.max_context_fingerprint_bytes,
    )?;
    partition.replay_with_limits(
        context,
        partition.orthant().sector(),
        limits.partition_replay,
    )?;
    let case = partition
        .case(case_id)
        .ok_or(CoordinateEqualityLocusError::CaseNotFound { case: case_id })?;
    check_limit(
        "coordinate-locus predicates",
        case.predicates().len(),
        limits.max_predicates,
    )?;

    let mut stats = CoordinateEqualityLocusStats {
        predicates: case.predicates().len(),
        retained_polynomial_terms: partition.stats().retained_polynomial_terms(),
        retained_polynomial_bytes: partition.stats().retained_polynomial_bytes(),
        ..CoordinateEqualityLocusStats::default()
    };
    check_limit(
        "coordinate-locus retained polynomial terms",
        stats.retained_polynomial_terms,
        limits.max_retained_polynomial_terms,
    )?;
    check_limit(
        "coordinate-locus retained polynomial bytes",
        stats.retained_polynomial_bytes,
        limits.max_retained_polynomial_bytes,
    )?;

    let mut equalities = BTreeMap::<usize, BTreeMap<i64, Vec<usize>>>::new();
    let mut nonzero = BTreeMap::<(usize, i64), Vec<usize>>::new();
    let mut recognized = Vec::new();
    let mut unresolved = Vec::new();

    for (ordinal, predicate) in case.predicates().iter().enumerate() {
        context.validate_polynomial_with_limits(predicate.polynomial(), limits.exact_algebra)?;
        charge_polynomial(predicate.polynomial(), &mut stats, limits)?;
        match recognize_coordinate_locus(context, predicate.polynomial(), &mut stats, limits)? {
            Some((index, value)) => {
                let retained_recognized =
                    checked_add("coordinate-locus witness ordinals", recognized.len(), 1)?;
                // Every recognized predicate retains at least its source
                // ordinal in `recognized_predicates`, independent of whether
                // it later also supports an assignment or an empty proof.
                // Enforce this lower bound before growing either retained
                // witness collection.
                check_limit(
                    "coordinate-locus witness ordinals",
                    retained_recognized,
                    limits.max_total_witness_ordinals,
                )?;
                check_limit(
                    "recognized coordinate-locus predicates",
                    retained_recognized,
                    limits.max_recognized_predicates,
                )?;
                match predicate.kind() {
                    SymbolicPolynomialPredicateKind::EqualZero => {
                        stats.equality_predicates = checked_add(
                            "coordinate equality predicates",
                            stats.equality_predicates,
                            1,
                        )?;
                        equalities
                            .entry(index)
                            .or_default()
                            .entry(value)
                            .or_default()
                            .push(ordinal);
                    }
                    SymbolicPolynomialPredicateKind::NonZero => {
                        stats.nonzero_predicates = checked_add(
                            "coordinate nonzero predicates",
                            stats.nonzero_predicates,
                            1,
                        )?;
                        nonzero.entry((index, value)).or_default().push(ordinal);
                    }
                }
                recognized.push(CoordinateLocusPredicateWitness {
                    predicate_ordinal: ordinal,
                    kind: predicate.kind(),
                    index,
                    value,
                });
            }
            None => {
                check_limit(
                    "unresolved coordinate-locus predicates",
                    checked_add(
                        "unresolved coordinate-locus predicates",
                        unresolved.len(),
                        1,
                    )?,
                    limits.max_unresolved_predicates,
                )?;
                let retained_polynomial_terms = checked_add(
                    "coordinate-locus retained polynomial terms",
                    stats.retained_polynomial_terms,
                    predicate.polynomial().term_count(),
                )?;
                check_limit(
                    "coordinate-locus retained polynomial terms",
                    retained_polynomial_terms,
                    limits.max_retained_polynomial_terms,
                )?;
                // Term retention is cheaper to preflight than bounded display
                // formatting.  Do not format a polynomial that the caller has
                // already forbidden us to clone into the certificate.
                let bytes = polynomial_display_bytes(
                    predicate.polynomial(),
                    limits
                        .max_retained_polynomial_bytes
                        .saturating_sub(stats.retained_polynomial_bytes),
                )?;
                stats.retained_polynomial_terms = retained_polynomial_terms;
                stats.retained_polynomial_bytes = checked_add(
                    "coordinate-locus retained polynomial bytes",
                    stats.retained_polynomial_bytes,
                    bytes,
                )?;
                check_limit(
                    "coordinate-locus retained polynomial bytes",
                    stats.retained_polynomial_bytes,
                    limits.max_retained_polynomial_bytes,
                )?;
                unresolved.push(UnresolvedCoordinatePredicate {
                    predicate_ordinal: ordinal,
                    kind: predicate.kind(),
                    polynomial: predicate.polynomial().clone(),
                });
            }
        }
    }
    stats.recognized_predicates = recognized.len();
    stats.unresolved_predicates = unresolved.len();

    #[derive(Clone, Copy)]
    enum PendingEmptyReason {
        ConflictingFixedValues {
            index: usize,
            first_value: i64,
            second_value: i64,
        },
        EqualityNonzeroContradiction {
            index: usize,
            value: i64,
        },
        OrthantViolation {
            index: usize,
            value: i64,
            side: SectorOrthantSide,
        },
    }

    let conflict = equalities.iter().find_map(|(&index, values)| {
        if values.len() < 2 {
            return None;
        }
        let mut values = values.iter();
        let (&first_value, _) = values.next()?;
        let (&second_value, _) = values.next()?;
        Some(PendingEmptyReason::ConflictingFixedValues {
            index,
            first_value,
            second_value,
        })
    });

    // A conflicting coordinate is excluded from the usable partial assignment.
    // Every other uniquely fixed coordinate remains a valid retained consequence.
    let mut assignments = Vec::new();
    for (&index, values) in &equalities {
        if values.len() != 1 {
            continue;
        }
        let requested = checked_add("coordinate-locus assignments", assignments.len(), 1)?;
        check_limit(
            "coordinate-locus assignments",
            requested,
            limits.max_assignments,
        )?;
        assignments.push((index, *values.first_key_value().unwrap().0));
    }

    let mut pending_status = conflict;
    if pending_status.is_none() {
        for &(index, value) in &assignments {
            if nonzero.contains_key(&(index, value)) {
                pending_status =
                    Some(PendingEmptyReason::EqualityNonzeroContradiction { index, value });
                break;
            }
        }
    }
    if pending_status.is_none() {
        for &(index, value) in &assignments {
            let constraint = partition
                .orthant()
                .constraints()
                .get(index)
                .ok_or(CoordinateEqualityLocusError::ReplayMismatch)?;
            if constraint.index() != index {
                return Err(CoordinateEqualityLocusError::ReplayMismatch);
            }
            if !constraint.accepts(value) {
                pending_status = Some(PendingEmptyReason::OrthantViolation {
                    index,
                    value,
                    side: constraint.side(),
                });
                break;
            }
        }
    }

    // Compute the exact final ordinal census before cloning any ordinal lists
    // into assignment or empty-reason witnesses.
    let assignment_ordinal_count =
        assignments
            .iter()
            .try_fold(0usize, |total, &(index, value)| {
                checked_add(
                    "coordinate-locus witness ordinals",
                    total,
                    equalities[&index][&value].len(),
                )
            })?;
    let empty_ordinal_count = match pending_status {
        None => 0,
        Some(PendingEmptyReason::ConflictingFixedValues {
            index,
            first_value,
            second_value,
        }) => checked_add(
            "coordinate-locus witness ordinals",
            equalities[&index][&first_value].len(),
            equalities[&index][&second_value].len(),
        )?,
        Some(PendingEmptyReason::EqualityNonzeroContradiction { index, value }) => checked_add(
            "coordinate-locus witness ordinals",
            equalities[&index][&value].len(),
            nonzero[&(index, value)].len(),
        )?,
        Some(PendingEmptyReason::OrthantViolation { index, value, .. }) => {
            equalities[&index][&value].len()
        }
    };
    stats.total_witness_ordinals = checked_add(
        "coordinate-locus witness ordinals",
        recognized.len(),
        checked_add(
            "coordinate-locus witness ordinals",
            assignment_ordinal_count,
            empty_ordinal_count,
        )?,
    )?;
    check_limit(
        "coordinate-locus witness ordinals",
        stats.total_witness_ordinals,
        limits.max_total_witness_ordinals,
    )?;

    let assignment = PartialIndexAssignment::try_new(
        assignments.iter().copied(),
        context.index_count(),
        limits.max_assignments,
    )?;
    let assignment_witnesses = assignments
        .iter()
        .map(|&(index, value)| CoordinateAssignmentWitness {
            index,
            value,
            equality_predicate_ordinals: equalities[&index][&value].clone().into_boxed_slice(),
        })
        .collect::<Vec<_>>();
    stats.assignments = assignment_witnesses.len();

    let status = match pending_status {
        None => CoordinateEqualityLeafStatus::NotProvedEmpty,
        Some(PendingEmptyReason::ConflictingFixedValues {
            index,
            first_value,
            second_value,
        }) => CoordinateEqualityLeafStatus::ProvedEmpty(
            CoordinateEqualityEmptyReason::ConflictingFixedValues {
                index,
                first_value,
                first_equality_predicate_ordinals: equalities[&index][&first_value]
                    .clone()
                    .into_boxed_slice(),
                second_value,
                second_equality_predicate_ordinals: equalities[&index][&second_value]
                    .clone()
                    .into_boxed_slice(),
            },
        ),
        Some(PendingEmptyReason::EqualityNonzeroContradiction { index, value }) => {
            CoordinateEqualityLeafStatus::ProvedEmpty(
                CoordinateEqualityEmptyReason::EqualityNonzeroContradiction {
                    index,
                    value,
                    equality_predicate_ordinals: equalities[&index][&value]
                        .clone()
                        .into_boxed_slice(),
                    nonzero_predicate_ordinals: nonzero[&(index, value)].clone().into_boxed_slice(),
                },
            )
        }
        Some(PendingEmptyReason::OrthantViolation { index, value, side }) => {
            CoordinateEqualityLeafStatus::ProvedEmpty(
                CoordinateEqualityEmptyReason::OrthantViolation {
                    index,
                    value,
                    equality_predicate_ordinals: equalities[&index][&value]
                        .clone()
                        .into_boxed_slice(),
                    side,
                },
            )
        }
    };

    Ok(CoordinateEqualityLocusCertificate {
        schema: COORDINATE_EQUALITY_LOCUS_V1_SCHEMA,
        source_partition: partition.clone(),
        source_case: case_id,
        assignment,
        assignment_witnesses: assignment_witnesses.into_boxed_slice(),
        recognized_predicates: recognized.into_boxed_slice(),
        unresolved_predicates: unresolved.into_boxed_slice(),
        status,
        limits,
        stats,
    })
}

fn charge_polynomial(
    polynomial: &ParametricPolynomial,
    stats: &mut CoordinateEqualityLocusStats,
    limits: CoordinateEqualityLocusLimits,
) -> Result<(), CoordinateEqualityLocusError> {
    stats.polynomial_terms_inspected = checked_add(
        "coordinate-locus polynomial terms inspected",
        stats.polynomial_terms_inspected,
        polynomial.term_count(),
    )?;
    check_limit(
        "coordinate-locus polynomial terms inspected",
        stats.polynomial_terms_inspected,
        limits.max_polynomial_terms_inspected,
    )?;
    let exponent_entries = checked_mul(
        "coordinate-locus exponent entries inspected",
        polynomial.term_count(),
        polynomial.raw().variables.len(),
    )?;
    stats.exponent_entries_inspected = checked_add(
        "coordinate-locus exponent entries inspected",
        stats.exponent_entries_inspected,
        exponent_entries,
    )?;
    check_limit(
        "coordinate-locus exponent entries inspected",
        stats.exponent_entries_inspected,
        limits.max_exponent_entries_inspected,
    )?;
    for coefficient in &polynomial.raw().coefficients {
        let bits = integer_magnitude_bits(coefficient)?;
        check_limit(
            "coordinate-locus integer coefficient bits",
            bits,
            limits.max_integer_coefficient_bits,
        )?;
    }
    Ok(())
}

/// Return `(index,c)` only for an exact expanded associate of `n_index-c`.
fn recognize_coordinate_locus(
    context: &ParametricCoefficientContext,
    polynomial: &ParametricPolynomial,
    stats: &mut CoordinateEqualityLocusStats,
    limits: CoordinateEqualityLocusLimits,
) -> Result<Option<(usize, i64)>, CoordinateEqualityLocusError> {
    let raw = polynomial.raw();
    let index_count = context.index_count();
    let base_count = raw
        .variables
        .len()
        .checked_sub(index_count)
        .ok_or(CoordinateEqualityLocusError::ReplayMismatch)?;
    let operations = checked_mul(
        "coordinate-locus recognition operations",
        raw.nterms(),
        checked_add("coordinate-locus recognition operations", index_count, 4)?,
    )?;
    stats.recognition_operations = checked_add(
        "coordinate-locus recognition operations",
        stats.recognition_operations,
        operations,
    )?;
    check_limit(
        "coordinate-locus recognition operations",
        stats.recognition_operations,
        limits.max_recognition_operations,
    )?;

    let mut coordinate = None;
    for exponents in raw.exponents_iter() {
        for (index, &exponent) in exponents[base_count..].iter().enumerate() {
            if exponent == 0 {
                continue;
            }
            if exponent != 1 {
                return Ok(None);
            }
            match coordinate {
                None => coordinate = Some(index),
                Some(existing) if existing == index => {}
                Some(_) => return Ok(None),
            }
        }
    }
    let Some(coordinate) = coordinate else {
        return Ok(None);
    };

    let mut slope = BTreeMap::<Vec<u16>, Integer>::new();
    let mut intercept = BTreeMap::<Vec<u16>, Integer>::new();
    for (coefficient, exponents) in raw.coefficients.iter().zip(raw.exponents_iter()) {
        if exponents[base_count..]
            .iter()
            .enumerate()
            .any(|(index, &exponent)| index != coordinate && exponent != 0)
        {
            return Ok(None);
        }
        let key = exponents[..base_count].to_vec();
        match exponents[base_count + coordinate] {
            0 => {
                if intercept.insert(key, coefficient.clone()).is_some() {
                    return Err(CoordinateEqualityLocusError::ReplayMismatch);
                }
            }
            1 => {
                if slope.insert(key, coefficient.clone()).is_some() {
                    return Err(CoordinateEqualityLocusError::ReplayMismatch);
                }
            }
            _ => return Ok(None),
        }
    }
    let Some((first_key, first_slope)) = slope.first_key_value() else {
        return Ok(None);
    };
    let first_intercept = intercept.get(first_key);
    let Some(value) = exact_coordinate_value(first_slope, first_intercept)? else {
        return Ok(None);
    };
    if intercept.keys().any(|key| !slope.contains_key(key)) {
        return Ok(None);
    }
    for (key, slope_coefficient) in &slope {
        if exact_coordinate_value(slope_coefficient, intercept.get(key))? != Some(value) {
            return Ok(None);
        }
    }
    Ok(Some((coordinate, value)))
}

/// Recognize one coordinate locus without requiring a finished partition.
///
/// The sector-coverage composer uses this narrow, exact primitive while it is
/// still constructing its global split transcript.  It performs the same
/// authentication, sparse-term charging, coefficient-bit checks, and
/// recognition as [`CoordinateEqualityLocusExtractor`], but deliberately does
/// not infer anything from any other predicate.  Conjunction-level reasoning
/// remains the caller's responsibility.
pub(crate) fn recognize_coordinate_locus_for_pruning(
    context: &ParametricCoefficientContext,
    polynomial: &ParametricPolynomial,
    limits: CoordinateEqualityLocusLimits,
) -> Result<Option<(usize, i64)>, CoordinateEqualityLocusError> {
    context.validate_polynomial_with_limits(polynomial, limits.exact_algebra)?;
    let mut stats = CoordinateEqualityLocusStats::default();
    charge_polynomial(polynomial, &mut stats, limits)?;
    recognize_coordinate_locus(context, polynomial, &mut stats, limits)
}

fn exact_coordinate_value(
    slope: &Integer,
    intercept: Option<&Integer>,
) -> Result<Option<i64>, CoordinateEqualityLocusError> {
    if slope.is_zero() {
        return Err(CoordinateEqualityLocusError::ReplayMismatch);
    }
    let Some(intercept) = intercept else {
        return Ok(Some(0));
    };
    let negative_intercept = -intercept;
    let (quotient, remainder) = Z.quot_rem(&negative_intercept, slope);
    if !remainder.is_zero() {
        return Ok(None);
    }
    Ok(i64::try_from(quotient).ok())
}

fn integer_magnitude_bits(value: &Integer) -> Result<usize, CoordinateEqualityLocusError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| CoordinateEqualityLocusError::ResourceCountOverflow {
        resource: "coordinate-locus integer coefficient bits",
    })
}

fn polynomial_display_bytes(
    polynomial: &ParametricPolynomial,
    remaining_limit: usize,
) -> Result<usize, CoordinateEqualityLocusError> {
    let mut counter = BoundedByteCounter {
        bytes: 0,
        limit: remaining_limit,
    };
    if write!(&mut counter, "{}", polynomial.raw()).is_err() {
        return Err(CoordinateEqualityLocusError::ResourceLimit {
            resource: "coordinate-locus retained polynomial bytes",
            requested: remaining_limit.saturating_add(1),
            limit: remaining_limit,
        });
    }
    Ok(counter.bytes)
}

struct BoundedByteCounter {
    bytes: usize,
    limit: usize,
}

impl fmt::Write for BoundedByteCounter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.bytes = self.bytes.checked_add(value.len()).ok_or(fmt::Error)?;
        if self.bytes > self.limit {
            Err(fmt::Error)
        } else {
            Ok(())
        }
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, CoordinateEqualityLocusError> {
    left.checked_add(right)
        .ok_or(CoordinateEqualityLocusError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, CoordinateEqualityLocusError> {
    left.checked_mul(right)
        .ok_or(CoordinateEqualityLocusError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), CoordinateEqualityLocusError> {
    if requested > limit {
        Err(CoordinateEqualityLocusError::ResourceLimit {
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
    use crate::{SymbolicSectorCasePartitionBuilder, algebra::CoefficientContext};

    fn context(scope: &str) -> ParametricCoefficientContext {
        ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            scope,
            1,
        )
        .unwrap()
    }

    fn coordinate_polynomial(
        context: &ParametricCoefficientContext,
        value: i64,
    ) -> ParametricPolynomial {
        let coefficient = context
            .sub(&context.index(0).unwrap(), &context.integer(value))
            .unwrap();
        context.numerator_condition(&coefficient).unwrap()
    }

    fn one_leaf_certificate(
        context: &ParametricCoefficientContext,
    ) -> CoordinateEqualityLocusCertificate {
        let mut builder = SymbolicSectorCasePartitionBuilder::try_new(
            context,
            crate::SectorMask::try_new([true]).unwrap(),
            SymbolicSectorCaseLimits::default(),
        )
        .unwrap();
        let leaf = builder
            .split_on_bad_polynomial(
                context,
                builder.root_case(),
                coordinate_polynomial(context, 2),
            )
            .unwrap()
            .equal_zero_case();
        let partition = builder.finish(context).unwrap();
        CoordinateEqualityLocusExtractor::extract(
            context,
            &partition,
            leaf,
            CoordinateEqualityLocusLimits::default(),
        )
        .unwrap()
    }

    #[test]
    fn replay_rejects_extracted_metadata_tampering() {
        let context = context("coordinate-locus-unit-replay");
        let certificate = one_leaf_certificate(&context);
        certificate.replay(&context).unwrap();

        let mut tampered = certificate.clone();
        tampered.assignment_witnesses[0].value = 3;
        assert_eq!(
            tampered.replay(&context),
            Err(CoordinateEqualityLocusError::ReplayMismatch)
        );

        let mut tampered = certificate.clone();
        tampered.stats.recognized_predicates = 0;
        assert_eq!(
            tampered.replay(&context),
            Err(CoordinateEqualityLocusError::ReplayMismatch)
        );

        let mut tampered = certificate;
        tampered.schema = "foreign-coordinate-locus-schema";
        assert_eq!(
            tampered.replay(&context),
            Err(CoordinateEqualityLocusError::SchemaMismatch)
        );
    }
}
