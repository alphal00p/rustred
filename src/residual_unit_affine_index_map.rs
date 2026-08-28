//! Replayable unit-affine index maps extracted from one symbolic residual leaf.
//!
//! This is the first dependent-`startp` proof boundary.  It accepts one exact
//! equality predicate which is an associate over the formal base field to an
//! integer-affine row with a caller-selected unit pivot.  Existing literal
//! coordinate equalities are folded into that row.  The resulting map
//!
//! ```text
//! F(t) = b + A t
//! ```
//!
//! keeps the unbound original indices as canonical free coordinates, so the
//! free rows of `A` are the identity and `F(F(n)) = F(n)`.  Non-affine,
//! rational/non-unit, cyclic, or otherwise unsupported predicates are never
//! sampled or widened into an integer cylinder.

use std::fmt;
use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::prelude::{EuclideanDomain, Integer, Z};

use crate::{
    CoordinateEqualityLeafStatus, CoordinateEqualityLocusCertificate, CoordinateEqualityLocusError,
    ParametricCoefficientContext, ParametricCoefficientError, SymbolicPolynomialPredicateKind,
    SymbolicSectorCaseId, algebra::ExactAlgebraLimits,
};

pub const RESIDUAL_UNIT_AFFINE_INDEX_MAP_V1_SCHEMA: &str =
    "rustred-residual-unit-affine-index-map-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResidualUnitAffineIndexMapLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_context_fingerprint_bytes: usize,
    pub max_source_identity_bytes_referenced: usize,
    pub max_ambient_arity: usize,
    pub max_source_polynomial_terms: usize,
    pub max_unresolved_predicates_scanned: usize,
    pub max_exponent_entries_inspected: usize,
    pub max_affine_blocks: usize,
    pub max_retained_block_exponent_entries: usize,
    pub max_retained_term_references: usize,
    pub max_recognition_operations: usize,
    pub max_integer_coefficient_bits: usize,
    pub max_free_positions: usize,
    pub max_literal_positions: usize,
    pub max_matrix_entries: usize,
    pub max_manifest_bytes: usize,
}

impl Default for ResidualUnitAffineIndexMapLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_context_fingerprint_bytes: 1024 * 1024,
            max_source_identity_bytes_referenced: 1024 * 1024 * 1024,
            max_ambient_arity: 4096,
            max_source_polynomial_terms: 4_000_000,
            max_unresolved_predicates_scanned: 4_000_000,
            max_exponent_entries_inspected: 256_000_000,
            max_affine_blocks: 4_000_000,
            max_retained_block_exponent_entries: 256_000_000,
            max_retained_term_references: 4_000_000,
            max_recognition_operations: 512_000_000,
            max_integer_coefficient_bits: 1_000_000,
            max_free_positions: 4096,
            max_literal_positions: 4096,
            max_matrix_entries: 16_777_216,
            max_manifest_bytes: 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResidualUnitAffineIndexMapStats {
    ambient_arity: usize,
    source_identity_bytes_referenced: usize,
    source_polynomial_terms: usize,
    unresolved_predicates_scanned: usize,
    exponent_entries_inspected: usize,
    affine_blocks: usize,
    retained_block_exponent_entries: usize,
    retained_term_references: usize,
    recognition_operations: usize,
    largest_integer_coefficient_bits: usize,
    free_positions: usize,
    literal_positions: usize,
    matrix_entries: usize,
    manifest_bytes: usize,
}

macro_rules! stats_getters {
    ($($field:ident),* $(,)?) => {$ (
        pub const fn $field(self) -> usize { self.$field }
    )* };
}

impl ResidualUnitAffineIndexMapStats {
    stats_getters!(
        ambient_arity,
        source_identity_bytes_referenced,
        source_polynomial_terms,
        unresolved_predicates_scanned,
        exponent_entries_inspected,
        affine_blocks,
        retained_block_exponent_entries,
        retained_term_references,
        recognition_operations,
        largest_integer_coefficient_bits,
        free_positions,
        literal_positions,
        matrix_entries,
        manifest_bytes,
    );
}

/// A completeness boundary, not an algebra or replay failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResidualUnitAffineIndexMapUnsupported {
    PredicateIsNotEquality,
    SourceLeafProvedEmpty,
    BoundPositionAlreadyLiteral { position: usize },
    NonAffineIndexEquality { term_ordinal: usize },
    BoundVariableAbsent { position: usize },
    UnconsumedEqualityPredicates { additional: usize },
    NonIntegralAffineCoefficient { component: usize },
    NotAssociateToSingleIntegerAffineRow { block_ordinal: usize },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResidualUnitAffineIndexMapError {
    SchemaMismatch,
    ReplayMismatch,
    WrongContext,
    PredicateNotFound {
        source_case: SymbolicSectorCaseId,
        predicate_ordinal: usize,
    },
    BoundPositionOutOfRange {
        source_case: SymbolicSectorCaseId,
        predicate_ordinal: usize,
        position: usize,
        arity: usize,
    },
    Unsupported {
        source_case: SymbolicSectorCaseId,
        predicate_ordinal: usize,
        reason: ResidualUnitAffineIndexMapUnsupported,
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
    SymbolicaPanic,
    Coordinate(CoordinateEqualityLocusError),
    Coefficient(ParametricCoefficientError),
}

impl fmt::Display for ResidualUnitAffineIndexMapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("unit-affine index-map schema mismatch"),
            Self::ReplayMismatch => formatter.write_str("unit-affine index map does not replay"),
            Self::WrongContext => {
                formatter.write_str("unit-affine index map belongs to another K(n) context")
            }
            Self::PredicateNotFound {
                source_case,
                predicate_ordinal,
            } => write!(
                formatter,
                "residual case {source_case} equality predicate ordinal {predicate_ordinal} was not retained"
            ),
            Self::BoundPositionOutOfRange {
                source_case,
                predicate_ordinal,
                position,
                arity,
            } => write!(
                formatter,
                "residual case {source_case} predicate {predicate_ordinal} unit-affine bound position {position} is outside arity {arity}"
            ),
            Self::Unsupported {
                source_case,
                predicate_ordinal,
                reason,
            } => write!(
                formatter,
                "unsupported unit-affine residual start at case {source_case}, predicate {predicate_ordinal}: {reason:?}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "unit-affine {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "unit-affine {resource} count overflowed usize")
            }
            Self::AllocationFailure { resource } => write!(
                formatter,
                "unit-affine {resource} allocation failed after bounded preflight"
            ),
            Self::SymbolicaPanic => {
                formatter.write_str("Symbolica panicked during unit-affine index-map compilation")
            }
            Self::Coordinate(error) => error.fmt(formatter),
            Self::Coefficient(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ResidualUnitAffineIndexMapError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Coordinate(error) => Some(error),
            Self::Coefficient(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CoordinateEqualityLocusError> for ResidualUnitAffineIndexMapError {
    fn from(value: CoordinateEqualityLocusError) -> Self {
        Self::Coordinate(value)
    }
}

impl From<ParametricCoefficientError> for ResidualUnitAffineIndexMapError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::Coefficient(value)
    }
}

/// One replay-bound, canonical idempotent map `F(t)=b+A*t`.
#[derive(Clone, Debug)]
pub struct ResidualUnitAffineIndexMapCertificate {
    schema: &'static str,
    context_fingerprint: Arc<str>,
    source: Arc<CoordinateEqualityLocusCertificate>,
    source_case: SymbolicSectorCaseId,
    source_equality_predicate_ordinal: usize,
    bound_position: usize,
    free_positions: Box<[usize]>,
    literal_positions: Box<[usize]>,
    constants: Box<[Integer]>,
    linear_coefficients: Box<[Integer]>,
    source_partition_identity: Arc<str>,
    local_manifest: Arc<str>,
    limits: ResidualUnitAffineIndexMapLimits,
    stats: ResidualUnitAffineIndexMapStats,
}

impl ResidualUnitAffineIndexMapCertificate {
    pub fn compile(
        context: &ParametricCoefficientContext,
        source: Arc<CoordinateEqualityLocusCertificate>,
        source_equality_predicate_ordinal: usize,
        bound_position: usize,
        limits: ResidualUnitAffineIndexMapLimits,
    ) -> Result<Self, ResidualUnitAffineIndexMapError> {
        catch_unwind(AssertUnwindSafe(|| {
            compile_inner(
                context,
                source,
                source_equality_predicate_ordinal,
                bound_position,
                limits,
                true,
            )
        }))
        .map_err(|_| ResidualUnitAffineIndexMapError::SymbolicaPanic)?
    }

    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }

    pub const fn source(&self) -> &Arc<CoordinateEqualityLocusCertificate> {
        &self.source
    }

    pub const fn source_case(&self) -> SymbolicSectorCaseId {
        self.source_case
    }

    pub const fn source_equality_predicate_ordinal(&self) -> usize {
        self.source_equality_predicate_ordinal
    }

    pub const fn bound_position(&self) -> usize {
        self.bound_position
    }

    pub fn free_positions(&self) -> &[usize] {
        &self.free_positions
    }

    pub fn literal_positions(&self) -> &[usize] {
        &self.literal_positions
    }

    pub fn ambient_arity(&self) -> usize {
        self.constants.len()
    }

    pub fn constant(&self, position: usize) -> Option<&Integer> {
        self.constants.get(position)
    }

    pub fn linear_coefficient(&self, position: usize, free_ordinal: usize) -> Option<&Integer> {
        if position >= self.ambient_arity() || free_ordinal >= self.free_positions.len() {
            return None;
        }
        let offset = position
            .checked_mul(self.free_positions.len())?
            .checked_add(free_ordinal)?;
        self.linear_coefficients.get(offset)
    }

    /// Exact source-owned identity shared by all maps from this partition.
    pub fn source_partition_identity(&self) -> &str {
        &self.source_partition_identity
    }

    /// Canonical map-local identity component.
    ///
    /// This must be paired with [`Self::source_partition_identity`] before it
    /// is used to authorize grouping or row mixing.
    pub fn local_manifest(&self) -> &str {
        &self.local_manifest
    }

    /// Structured, collision-free identity of this map without copying the
    /// potentially large partition transcript into every local manifest.
    pub fn identity_parts(&self) -> (&str, &str) {
        (&self.source_partition_identity, &self.local_manifest)
    }

    pub const fn limits(&self) -> ResidualUnitAffineIndexMapLimits {
        self.limits
    }

    pub const fn stats(&self) -> ResidualUnitAffineIndexMapStats {
        self.stats
    }

    pub fn replay(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), ResidualUnitAffineIndexMapError> {
        if self.schema != RESIDUAL_UNIT_AFFINE_INDEX_MAP_V1_SCHEMA {
            return Err(ResidualUnitAffineIndexMapError::SchemaMismatch);
        }
        let replayed = catch_unwind(AssertUnwindSafe(|| {
            compile_inner(
                context,
                self.source.clone(),
                self.source_equality_predicate_ordinal,
                self.bound_position,
                self.limits,
                false,
            )
        }))
        .map_err(|_| ResidualUnitAffineIndexMapError::SymbolicaPanic)??;
        if self.payload_eq(&replayed) {
            Ok(())
        } else {
            Err(ResidualUnitAffineIndexMapError::ReplayMismatch)
        }
    }

    /// Complete typed equality used only by enclosing replay certificates.
    /// Compact local-manifest equality alone is deliberately insufficient.
    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.context_fingerprint == other.context_fingerprint
            && self.source == other.source
            && self.source_case == other.source_case
            && self.source_equality_predicate_ordinal == other.source_equality_predicate_ordinal
            && self.bound_position == other.bound_position
            && self.free_positions == other.free_positions
            && self.literal_positions == other.literal_positions
            && self.constants == other.constants
            && self.linear_coefficients == other.linear_coefficients
            && self.source_partition_identity == other.source_partition_identity
            && self.local_manifest == other.local_manifest
            && self.limits == other.limits
            && self.stats == other.stats
    }
}

#[derive(Debug)]
struct AffineBlock {
    base_exponents: Vec<u16>,
    term_components: Vec<(usize, usize)>,
}

#[allow(clippy::too_many_arguments)]
fn compile_inner(
    context: &ParametricCoefficientContext,
    source: Arc<CoordinateEqualityLocusCertificate>,
    source_equality_predicate_ordinal: usize,
    bound_position: usize,
    limits: ResidualUnitAffineIndexMapLimits,
    replay_result: bool,
) -> Result<ResidualUnitAffineIndexMapCertificate, ResidualUnitAffineIndexMapError> {
    // Retained metadata is immutable.  It may be inspected here only to
    // reject work that exceeds this compiler's local budgets; no semantic
    // success or Unsupported result is emitted until the complete source has
    // replayed below.
    if source.source_partition().context_fingerprint() != context.fingerprint() {
        return Err(ResidualUnitAffineIndexMapError::WrongContext);
    }
    let source_case = source.source_case();
    let arity = context.index_count();
    check_limit("ambient arity", arity, limits.max_ambient_arity)?;
    check_limit(
        "context fingerprint bytes",
        context.fingerprint().len(),
        limits.max_context_fingerprint_bytes,
    )?;
    if bound_position >= arity {
        return Err(ResidualUnitAffineIndexMapError::BoundPositionOutOfRange {
            source_case,
            predicate_ordinal: source_equality_predicate_ordinal,
            position: bound_position,
            arity,
        });
    }
    check_limit(
        "unresolved predicates scanned",
        source.unresolved_predicates().len(),
        limits.max_unresolved_predicates_scanned,
    )?;
    check_limit(
        "literal positions",
        source.assignment().entries().len(),
        limits.max_literal_positions,
    )?;
    check_limit(
        "source identity bytes referenced",
        source.source_partition().source_identity().len(),
        limits.max_source_identity_bytes_referenced,
    )?;

    // If the selected retained predicate is present, preflight its shape
    // before source replay can reconstruct the much larger proof.  A missing
    // predicate remains a semantic error and is reported only after replay.
    if let Some(predicate) = source
        .unresolved_predicates()
        .iter()
        .find(|predicate| predicate.predicate_ordinal() == source_equality_predicate_ordinal)
    {
        let raw = predicate.polynomial().raw();
        check_limit(
            "source polynomial terms",
            raw.nterms(),
            limits.max_source_polynomial_terms,
        )?;
        check_limit(
            "retained term references",
            raw.nterms(),
            limits.max_retained_term_references,
        )?;
        let exponent_entries = checked_mul(
            "exponent entries inspected",
            raw.nterms(),
            raw.variables.len(),
        )?;
        check_limit(
            "exponent entries inspected",
            exponent_entries,
            limits.max_exponent_entries_inspected,
        )?;
        if let Some(base_count) = raw.variables.len().checked_sub(arity) {
            let recognition_upper_bound =
                recognition_operation_upper_bound(raw.nterms(), base_count, arity, raw.nterms())?;
            check_limit(
                "recognition operations",
                recognition_upper_bound,
                limits.max_recognition_operations,
            )?;
        }
    }

    source.replay(context)?;
    if source.assignment().arity() != arity {
        return Err(ResidualUnitAffineIndexMapError::ReplayMismatch);
    }
    if !matches!(
        source.status(),
        CoordinateEqualityLeafStatus::NotProvedEmpty
    ) {
        return Err(unsupported(
            source_case,
            source_equality_predicate_ordinal,
            ResidualUnitAffineIndexMapUnsupported::SourceLeafProvedEmpty,
        ));
    }
    if source
        .assignment()
        .entries()
        .iter()
        .any(|&(position, _)| position == bound_position)
    {
        return Err(unsupported(
            source_case,
            source_equality_predicate_ordinal,
            ResidualUnitAffineIndexMapUnsupported::BoundPositionAlreadyLiteral {
                position: bound_position,
            },
        ));
    }
    let mut selected_predicate_index = None;
    let mut equality_predicates = 0usize;
    for (index, predicate) in source.unresolved_predicates().iter().enumerate() {
        if predicate.predicate_ordinal() == source_equality_predicate_ordinal {
            selected_predicate_index = Some(index);
        }
        if predicate.kind() == SymbolicPolynomialPredicateKind::EqualZero {
            equality_predicates =
                checked_add("unresolved equality predicates", equality_predicates, 1)?;
        }
    }
    let predicate = source
        .unresolved_predicates()
        .get(selected_predicate_index.ok_or(
            ResidualUnitAffineIndexMapError::PredicateNotFound {
                source_case,
                predicate_ordinal: source_equality_predicate_ordinal,
            },
        )?)
        .ok_or(ResidualUnitAffineIndexMapError::ReplayMismatch)?;
    if predicate.kind() != SymbolicPolynomialPredicateKind::EqualZero {
        return Err(unsupported(
            source_case,
            source_equality_predicate_ordinal,
            ResidualUnitAffineIndexMapUnsupported::PredicateIsNotEquality,
        ));
    }
    if equality_predicates != 1 {
        return Err(unsupported(
            source_case,
            source_equality_predicate_ordinal,
            ResidualUnitAffineIndexMapUnsupported::UnconsumedEqualityPredicates {
                additional: equality_predicates.saturating_sub(1),
            },
        ));
    }
    let polynomial = predicate.polynomial();
    context.validate_polynomial_with_limits(polynomial, limits.exact_algebra)?;
    let raw = polynomial.raw();
    let source_terms = raw.nterms();
    check_limit(
        "source polynomial terms",
        source_terms,
        limits.max_source_polynomial_terms,
    )?;
    let base_count = raw
        .variables
        .len()
        .checked_sub(arity)
        .ok_or(ResidualUnitAffineIndexMapError::ReplayMismatch)?;
    let exponent_entries = checked_mul(
        "exponent entries inspected",
        source_terms,
        raw.variables.len(),
    )?;
    check_limit(
        "exponent entries inspected",
        exponent_entries,
        limits.max_exponent_entries_inspected,
    )?;

    let mut blocks = Vec::<AffineBlock>::new();
    let mut retained_block_exponent_entries = 0usize;
    let mut retained_term_references = 0usize;
    let mut recognition_operations = 0usize;
    let mut largest_integer_coefficient_bits = 0usize;
    for term_ordinal in 0..source_terms {
        let exponents = raw.exponents(term_ordinal);
        recognition_operations = bounded_add(
            "recognition operations",
            recognition_operations,
            checked_add("recognition operations", arity, 1)?,
            limits.max_recognition_operations,
        )?;
        let component =
            classify_affine_component(exponents, base_count, arity).ok_or_else(|| {
                unsupported(
                    source_case,
                    source_equality_predicate_ordinal,
                    ResidualUnitAffineIndexMapUnsupported::NonAffineIndexEquality { term_ordinal },
                )
            })?;
        let bits = integer_magnitude_bits(&raw.coefficients[term_ordinal])?;
        check_limit(
            "integer coefficient bits",
            bits,
            limits.max_integer_coefficient_bits,
        )?;
        largest_integer_coefficient_bits = largest_integer_coefficient_bits.max(bits);

        // Charge the retained reference before either growing an existing
        // block or inserting a new key/value allocation into the map.
        retained_term_references = bounded_add(
            "retained term references",
            retained_term_references,
            1,
            limits.max_retained_term_references,
        )?;
        // `ParametricPolynomial` uses Symbolica's base-first LexOrder.  Equal
        // base prefixes are therefore contiguous even while their trailing
        // index exponents vary, so a linear last-block comparison is both
        // complete and exactly chargeable.
        recognition_operations = bounded_add(
            "recognition operations",
            recognition_operations,
            if blocks.is_empty() { 0 } else { base_count },
            limits.max_recognition_operations,
        )?;
        if blocks
            .last()
            .is_some_and(|block| block.base_exponents == exponents[..base_count])
        {
            let block = blocks
                .last_mut()
                .ok_or(ResidualUnitAffineIndexMapError::ReplayMismatch)?;
            block.term_components.try_reserve(1).map_err(|_| {
                ResidualUnitAffineIndexMapError::AllocationFailure {
                    resource: "affine block term references",
                }
            })?;
            block.term_components.push((term_ordinal, component));
        } else {
            let requested_blocks = checked_add("affine blocks", blocks.len(), 1)?;
            check_limit("affine blocks", requested_blocks, limits.max_affine_blocks)?;
            retained_block_exponent_entries = bounded_add(
                "retained block exponent entries",
                retained_block_exponent_entries,
                base_count,
                limits.max_retained_block_exponent_entries,
            )?;
            blocks.try_reserve(1).map_err(|_| {
                ResidualUnitAffineIndexMapError::AllocationFailure {
                    resource: "affine blocks",
                }
            })?;
            let mut term_components = Vec::new();
            term_components.try_reserve(1).map_err(|_| {
                ResidualUnitAffineIndexMapError::AllocationFailure {
                    resource: "affine block term references",
                }
            })?;
            term_components.push((term_ordinal, component));
            let mut base_exponents = Vec::new();
            base_exponents.try_reserve_exact(base_count).map_err(|_| {
                ResidualUnitAffineIndexMapError::AllocationFailure {
                    resource: "affine block base exponents",
                }
            })?;
            base_exponents.extend_from_slice(&exponents[..base_count]);
            blocks.push(AffineBlock {
                base_exponents,
                term_components,
            });
        }
    }

    let bound_component = bound_position + 1;
    // Charge the worst-case linear term-reference scan before searching for
    // a block containing the chosen pivot component.
    recognition_operations = bounded_add(
        "recognition operations",
        recognition_operations,
        source_terms,
        limits.max_recognition_operations,
    )?;
    let (normalizing_block_ordinal, normalizing_block) = blocks
        .iter()
        .enumerate()
        .find(|(_, block)| {
            block
                .term_components
                .iter()
                .any(|&(_, component)| component == bound_component)
        })
        .ok_or_else(|| {
            unsupported(
                source_case,
                source_equality_predicate_ordinal,
                ResidualUnitAffineIndexMapUnsupported::BoundVariableAbsent {
                    position: bound_position,
                },
            )
        })?;
    let normalizing_component_work = checked_add(
        "recognition operations",
        normalizing_block.term_components.len(),
        checked_add("recognition operations", arity, 1)?,
    )?;
    recognition_operations = bounded_add(
        "recognition operations",
        recognition_operations,
        normalizing_component_work,
        limits.max_recognition_operations,
    )?;
    let normalizing_components = block_components(raw, normalizing_block, arity)?;
    let slope = normalizing_components[bound_component]
        .ok_or(ResidualUnitAffineIndexMapError::ReplayMismatch)?;
    let mut normalized = Vec::new();
    normalized.try_reserve_exact(arity + 1).map_err(|_| {
        ResidualUnitAffineIndexMapError::AllocationFailure {
            resource: "normalized affine row",
        }
    })?;
    recognition_operations = bounded_add(
        "recognition operations",
        recognition_operations,
        checked_add("recognition operations", arity, 1)?,
        limits.max_recognition_operations,
    )?;
    for (component, value) in normalizing_components.iter().enumerate() {
        let value = value.cloned().unwrap_or_else(|| Integer::from(0));
        let (quotient, remainder) = Z.quot_rem(&value, slope);
        if !remainder.is_zero() {
            return Err(unsupported(
                source_case,
                source_equality_predicate_ordinal,
                ResidualUnitAffineIndexMapUnsupported::NonIntegralAffineCoefficient { component },
            ));
        }
        let bits = integer_magnitude_bits(&quotient)?;
        check_limit(
            "integer coefficient bits",
            bits,
            limits.max_integer_coefficient_bits,
        )?;
        largest_integer_coefficient_bits = largest_integer_coefficient_bits.max(bits);
        normalized.push(quotient);
    }
    if normalized[bound_component] != Integer::from(1) {
        return Err(ResidualUnitAffineIndexMapError::ReplayMismatch);
    }

    // Rebuilding every block component vector visits all retained term
    // references once and initializes one `(arity+1)` vector per block.  The
    // associate comparison then performs one product/comparison per component.
    // Charge all of it before entering either allocation/arithmetic loop.
    let block_component_initialization = checked_mul(
        "recognition operations",
        blocks.len(),
        checked_add("recognition operations", arity, 1)?,
    )?;
    let block_component_products_and_checks =
        checked_mul("recognition operations", block_component_initialization, 2)?;
    let block_component_checks = checked_add(
        "recognition operations",
        source_terms,
        checked_add(
            "recognition operations",
            block_component_initialization,
            block_component_products_and_checks,
        )?,
    )?;
    recognition_operations = bounded_add(
        "recognition operations",
        recognition_operations,
        block_component_checks,
        limits.max_recognition_operations,
    )?;
    for (block_ordinal, block) in blocks.iter().enumerate() {
        let components = block_components(raw, block, arity)?;
        let Some(block_slope) = components[bound_component] else {
            return Err(unsupported(
                source_case,
                source_equality_predicate_ordinal,
                ResidualUnitAffineIndexMapUnsupported::NotAssociateToSingleIntegerAffineRow {
                    block_ordinal,
                },
            ));
        };
        for component in 0..=arity {
            let product_bits = checked_add(
                "integer coefficient bits",
                integer_magnitude_bits(block_slope)?,
                integer_magnitude_bits(&normalized[component])?,
            )?;
            check_limit(
                "integer coefficient bits",
                product_bits,
                limits.max_integer_coefficient_bits,
            )?;
            let expected = block_slope * &normalized[component];
            match components[component] {
                Some(actual) if actual == &expected => {}
                None if expected.is_zero() => {}
                _ => {
                    return Err(unsupported(
                        source_case,
                        source_equality_predicate_ordinal,
                        ResidualUnitAffineIndexMapUnsupported::NotAssociateToSingleIntegerAffineRow {
                            block_ordinal,
                        },
                    ));
                }
            }
        }
    }
    let _ = normalizing_block_ordinal;

    check_limit(
        "literal positions",
        source.assignment().entries().len(),
        limits.max_literal_positions,
    )?;
    let mut literal_positions = Vec::new();
    literal_positions
        .try_reserve_exact(source.assignment().entries().len())
        .map_err(|_| ResidualUnitAffineIndexMapError::AllocationFailure {
            resource: "literal positions",
        })?;
    literal_positions.extend(
        source
            .assignment()
            .entries()
            .iter()
            .map(|&(position, _)| position),
    );
    let free_position_count = arity
        .checked_sub(literal_positions.len())
        .and_then(|remaining| remaining.checked_sub(1))
        .ok_or(ResidualUnitAffineIndexMapError::ReplayMismatch)?;
    check_limit(
        "free positions",
        free_position_count,
        limits.max_free_positions,
    )?;
    let mut free_positions = Vec::new();
    free_positions
        .try_reserve_exact(free_position_count)
        .map_err(|_| ResidualUnitAffineIndexMapError::AllocationFailure {
            resource: "free positions",
        })?;
    for position in 0..arity {
        if position != bound_position && !literal_positions.binary_search(&position).is_ok() {
            free_positions.push(position);
        }
    }
    if free_positions.len() != free_position_count {
        return Err(ResidualUnitAffineIndexMapError::ReplayMismatch);
    }
    let matrix_entries = checked_mul("affine matrix entries", arity, free_positions.len())?;
    check_limit(
        "affine matrix entries",
        matrix_entries,
        limits.max_matrix_entries,
    )?;

    let mut folded_constant = normalized[0].clone();
    for &(position, value) in source.assignment().entries() {
        let product_bits = checked_add(
            "integer coefficient bits",
            integer_magnitude_bits(&normalized[position + 1])?,
            usize::try_from(i64::BITS - value.unsigned_abs().leading_zeros()).map_err(|_| {
                ResidualUnitAffineIndexMapError::ResourceCountOverflow {
                    resource: "integer coefficient bits",
                }
            })?,
        )?;
        check_limit(
            "integer coefficient bits",
            product_bits,
            limits.max_integer_coefficient_bits,
        )?;
        let sum_bits = checked_add(
            "integer coefficient bits",
            integer_magnitude_bits(&folded_constant)?.max(product_bits),
            1,
        )?;
        check_limit(
            "integer coefficient bits",
            sum_bits,
            limits.max_integer_coefficient_bits,
        )?;
        folded_constant += &normalized[position + 1] * Integer::from(value);
        let folded_bits = integer_magnitude_bits(&folded_constant)?;
        check_limit(
            "integer coefficient bits",
            folded_bits,
            limits.max_integer_coefficient_bits,
        )?;
        largest_integer_coefficient_bits = largest_integer_coefficient_bits.max(folded_bits);
    }
    let mut constants = Vec::new();
    constants.try_reserve_exact(arity).map_err(|_| {
        ResidualUnitAffineIndexMapError::AllocationFailure {
            resource: "affine constants",
        }
    })?;
    let mut matrix = Vec::new();
    matrix.try_reserve_exact(matrix_entries).map_err(|_| {
        ResidualUnitAffineIndexMapError::AllocationFailure {
            resource: "affine matrix",
        }
    })?;
    for position in 0..arity {
        if let Ok(literal_ordinal) = literal_positions.binary_search(&position) {
            constants.push(Integer::from(
                source.assignment().entries()[literal_ordinal].1,
            ));
            matrix.extend((0..free_positions.len()).map(|_| Integer::from(0)));
        } else if position == bound_position {
            constants.push(-folded_constant.clone());
            for &free_position in &free_positions {
                matrix.push(-normalized[free_position + 1].clone());
            }
        } else {
            constants.push(Integer::from(0));
            for &free_position in &free_positions {
                matrix.push(Integer::from(usize::from(free_position == position)));
            }
        }
    }
    for value in constants.iter().chain(&matrix) {
        let bits = integer_magnitude_bits(value)?;
        check_limit(
            "integer coefficient bits",
            bits,
            limits.max_integer_coefficient_bits,
        )?;
        largest_integer_coefficient_bits = largest_integer_coefficient_bits.max(bits);
    }

    let mut manifest = BoundedManifestBuilder::new(limits.max_manifest_bytes);
    write!(
        &mut manifest,
        "{RESIDUAL_UNIT_AFFINE_INDEX_MAP_V1_SCHEMA}|case={}|predicate={source_equality_predicate_ordinal}|bound={bound_position}|arity={arity}|free=",
        source.source_case().value(),
    )
    .map_err(|_| manifest.error("local manifest"))?;
    write_usize_list(&mut manifest, &free_positions)?;
    manifest
        .write_str("|literal=")
        .map_err(|_| manifest.error("local manifest"))?;
    write_usize_list(&mut manifest, &literal_positions)?;
    manifest
        .write_str("|b=")
        .map_err(|_| manifest.error("local manifest"))?;
    write_integer_list(&mut manifest, &constants)?;
    manifest
        .write_str("|A=")
        .map_err(|_| manifest.error("local manifest"))?;
    write_integer_list(&mut manifest, &matrix)?;
    let (local_manifest, manifest_bytes) = manifest.finish();
    let source_partition_identity = source.source_partition().source_identity().clone();

    let stats = ResidualUnitAffineIndexMapStats {
        ambient_arity: arity,
        source_identity_bytes_referenced: source_partition_identity.len(),
        source_polynomial_terms: source_terms,
        unresolved_predicates_scanned: source.unresolved_predicates().len(),
        exponent_entries_inspected: exponent_entries,
        affine_blocks: blocks.len(),
        retained_block_exponent_entries,
        retained_term_references,
        recognition_operations,
        largest_integer_coefficient_bits,
        free_positions: free_positions.len(),
        literal_positions: literal_positions.len(),
        matrix_entries,
        manifest_bytes,
    };
    let result = ResidualUnitAffineIndexMapCertificate {
        schema: RESIDUAL_UNIT_AFFINE_INDEX_MAP_V1_SCHEMA,
        context_fingerprint: context.fingerprint().into(),
        source,
        source_case,
        source_equality_predicate_ordinal,
        bound_position,
        free_positions: free_positions.into_boxed_slice(),
        literal_positions: literal_positions.into_boxed_slice(),
        constants: constants.into_boxed_slice(),
        linear_coefficients: matrix.into_boxed_slice(),
        source_partition_identity,
        local_manifest: local_manifest.into(),
        limits,
        stats,
    };
    if replay_result {
        result.replay(context)?;
    }
    Ok(result)
}

fn classify_affine_component(exponents: &[u16], base_count: usize, arity: usize) -> Option<usize> {
    let mut component = 0usize;
    for (position, &exponent) in exponents[base_count..base_count + arity].iter().enumerate() {
        if exponent == 0 {
            continue;
        }
        if exponent != 1 || component != 0 {
            return None;
        }
        component = position + 1;
    }
    Some(component)
}

fn block_components<'a>(
    raw: &'a crate::CoefficientPolynomial,
    block: &AffineBlock,
    arity: usize,
) -> Result<Vec<Option<&'a Integer>>, ResidualUnitAffineIndexMapError> {
    let mut components = Vec::new();
    components.try_reserve_exact(arity + 1).map_err(|_| {
        ResidualUnitAffineIndexMapError::AllocationFailure {
            resource: "temporary affine components",
        }
    })?;
    components.resize(arity + 1, None);
    for &(term_ordinal, component) in &block.term_components {
        if components[component]
            .replace(&raw.coefficients[term_ordinal])
            .is_some()
        {
            return Err(ResidualUnitAffineIndexMapError::ReplayMismatch);
        }
    }
    Ok(components)
}

fn write_usize_list(
    manifest: &mut BoundedManifestBuilder,
    values: &[usize],
) -> Result<(), ResidualUnitAffineIndexMapError> {
    for (ordinal, value) in values.iter().enumerate() {
        if ordinal != 0 {
            manifest
                .write_char(',')
                .map_err(|_| manifest.error("local manifest"))?;
        }
        write!(manifest, "{value}").map_err(|_| manifest.error("local manifest"))?;
    }
    Ok(())
}

fn write_integer_list(
    manifest: &mut BoundedManifestBuilder,
    values: &[Integer],
) -> Result<(), ResidualUnitAffineIndexMapError> {
    for (ordinal, value) in values.iter().enumerate() {
        if ordinal != 0 {
            manifest
                .write_char(',')
                .map_err(|_| manifest.error("local manifest"))?;
        }
        write!(manifest, "{value}").map_err(|_| manifest.error("local manifest"))?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ManifestFailure {
    Limit { requested: usize },
    Overflow,
    Allocation,
}

struct BoundedManifestBuilder {
    value: String,
    limit: usize,
    failure: Option<ManifestFailure>,
}

impl BoundedManifestBuilder {
    fn new(limit: usize) -> Self {
        Self {
            value: String::new(),
            limit,
            failure: None,
        }
    }

    fn error(&self, resource: &'static str) -> ResidualUnitAffineIndexMapError {
        match self.failure {
            Some(ManifestFailure::Limit { requested }) => {
                ResidualUnitAffineIndexMapError::ResourceLimit {
                    resource,
                    requested,
                    limit: self.limit,
                }
            }
            Some(ManifestFailure::Overflow) => {
                ResidualUnitAffineIndexMapError::ResourceCountOverflow { resource }
            }
            Some(ManifestFailure::Allocation) | None => {
                ResidualUnitAffineIndexMapError::AllocationFailure { resource }
            }
        }
    }

    fn finish(self) -> (String, usize) {
        let bytes = self.value.len();
        (self.value, bytes)
    }
}

impl fmt::Write for BoundedManifestBuilder {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let Some(requested) = self.value.len().checked_add(value.len()) else {
            self.failure = Some(ManifestFailure::Overflow);
            return Err(fmt::Error);
        };
        if requested > self.limit {
            self.failure = Some(ManifestFailure::Limit { requested });
            return Err(fmt::Error);
        }
        if self.value.try_reserve(value.len()).is_err() {
            self.failure = Some(ManifestFailure::Allocation);
            return Err(fmt::Error);
        }
        self.value.push_str(value);
        Ok(())
    }
}

fn integer_magnitude_bits(value: &Integer) -> Result<usize, ResidualUnitAffineIndexMapError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| ResidualUnitAffineIndexMapError::ResourceCountOverflow {
        resource: "integer coefficient bits",
    })
}

fn unsupported(
    source_case: SymbolicSectorCaseId,
    predicate_ordinal: usize,
    reason: ResidualUnitAffineIndexMapUnsupported,
) -> ResidualUnitAffineIndexMapError {
    ResidualUnitAffineIndexMapError::Unsupported {
        source_case,
        predicate_ordinal,
        reason,
    }
}

/// Conservative complete recognition-work preflight available from retained
/// cardinalities alone. `block_upper_bound` may be `source_terms` before
/// replay/grouping; after grouping, dynamic charges below use the exact block
/// count and term distribution.
fn recognition_operation_upper_bound(
    source_terms: usize,
    base_count: usize,
    arity: usize,
    block_upper_bound: usize,
) -> Result<usize, ResidualUnitAffineIndexMapError> {
    let components = checked_add("recognition operations", arity, 1)?;
    let classification = checked_mul("recognition operations", source_terms, components)?;
    let prefix_comparisons = checked_mul(
        "recognition operations",
        source_terms.saturating_sub(1),
        base_count,
    )?;
    let pivot_scan = source_terms;
    let normalizing_component_build =
        checked_add("recognition operations", source_terms, components)?;
    let normalization = components;
    let per_block_components =
        checked_mul("recognition operations", block_upper_bound, components)?;
    let all_block_work = checked_add(
        "recognition operations",
        source_terms,
        checked_mul("recognition operations", per_block_components, 3)?,
    )?;
    [
        classification,
        prefix_comparisons,
        pivot_scan,
        normalizing_component_build,
        normalization,
        all_block_work,
    ]
    .into_iter()
    .try_fold(0usize, |total, work| {
        checked_add("recognition operations", total, work)
    })
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ResidualUnitAffineIndexMapError> {
    left.checked_add(right)
        .ok_or(ResidualUnitAffineIndexMapError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ResidualUnitAffineIndexMapError> {
    left.checked_mul(right)
        .ok_or(ResidualUnitAffineIndexMapError::ResourceCountOverflow { resource })
}

fn bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, ResidualUnitAffineIndexMapError> {
    let requested = checked_add(resource, left, right)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ResidualUnitAffineIndexMapError> {
    if requested > limit {
        Err(ResidualUnitAffineIndexMapError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
