//! Bounded, replayable recognition of one residual polynomial as an integer-affine row.
//!
//! A polynomial in the authenticated ring `K[n]`, with `K = Q(theta)`, is
//! traversed directly in Symbolica's sparse representation.  Monomials are
//! grouped by their base-variable exponent prefix.  Every nonzero prefix
//! block must have the form
//!
//! ```text
//! s_beta * (c + a_0*n_0 + ... + a_(N-1)*n_(N-1)),
//! ```
//!
//! where `s_beta` is a nonzero integer and the common row is primitive with
//! its first nonzero component positive.  No base-field sampling and no
//! machine-integer narrowing is used.  This certificate recognizes algebraic
//! shape only: an enclosing Boolean-branch certificate remains responsible
//! for proving that the supplied atom is required to vanish.

use std::fmt;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::prelude::{EuclideanDomain, Integer, Z};

use crate::{
    ExactAlgebraLimits, ParametricCoefficientContext, ParametricCoefficientError,
    ParametricPolynomial,
};

pub const RESIDUAL_AFFINE_ATOM_ROW_V1_SCHEMA: &str = "rustred-residual-affine-atom-row-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResidualAffineAtomRowLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_context_fingerprint_bytes: usize,
    pub max_ambient_arity: usize,
    pub max_base_variables: usize,
    pub max_source_variables: usize,
    pub max_source_terms: usize,
    pub max_exponent_entries_inspected: usize,
    pub max_affine_blocks: usize,
    pub max_retained_block_witnesses: usize,
    pub max_retained_block_exponent_entries: usize,
    pub max_primitive_row_components: usize,
    pub max_recognition_operations: usize,
    pub max_gcd_operations: usize,
    pub max_exact_quotient_operations: usize,
    pub max_integer_coefficient_bits: usize,
    pub max_integer_bit_work: usize,
}

impl Default for ResidualAffineAtomRowLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_context_fingerprint_bytes: 1024 * 1024,
            max_ambient_arity: 4096,
            max_base_variables: 4096,
            max_source_variables: 8192,
            max_source_terms: 4_000_000,
            max_exponent_entries_inspected: 256_000_000,
            max_affine_blocks: 4_000_000,
            max_retained_block_witnesses: 4_000_000,
            max_retained_block_exponent_entries: 256_000_000,
            max_primitive_row_components: 4097,
            max_recognition_operations: 512_000_000,
            max_gcd_operations: 4_000_000,
            max_exact_quotient_operations: 4_000_000,
            max_integer_coefficient_bits: 1_000_000,
            max_integer_bit_work: 1_000_000_000_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResidualAffineAtomRowStats {
    context_fingerprint_bytes: usize,
    ambient_arity: usize,
    base_variables: usize,
    source_variables: usize,
    source_terms: usize,
    exponent_entries_inspected: usize,
    affine_blocks: usize,
    retained_block_witnesses: usize,
    retained_block_exponent_entries: usize,
    primitive_row_components: usize,
    recognition_operations: usize,
    gcd_operations: usize,
    exact_quotient_operations: usize,
    largest_integer_coefficient_bits: usize,
    integer_bit_work: usize,
}

macro_rules! stats_getters {
    ($($field:ident),* $(,)?) => {$ (
        pub const fn $field(self) -> usize { self.$field }
    )* };
}

impl ResidualAffineAtomRowStats {
    stats_getters!(
        context_fingerprint_bytes,
        ambient_arity,
        base_variables,
        source_variables,
        source_terms,
        exponent_entries_inspected,
        affine_blocks,
        retained_block_witnesses,
        retained_block_exponent_entries,
        primitive_row_components,
        recognition_operations,
        gcd_operations,
        exact_quotient_operations,
        largest_integer_coefficient_bits,
        integer_bit_work,
    );
}

/// Allocation-independent logical peak of one affine-atom recognition
/// attempt. The scalar result is intentionally copyable; it carries no
/// certificate, source, or freshness authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResidualAffineAtomRowAttemptLogicalMemoryCensus {
    owned_logical_peak_upper_bound: usize,
}

impl ResidualAffineAtomRowAttemptLogicalMemoryCensus {
    pub(crate) const fn owned_logical_peak_upper_bound(self) -> usize {
        self.owned_logical_peak_upper_bound
    }
}

/// V2-only adjacent result of one fresh atom-row attempt. Unsupported shape
/// carries the same cheap, recomputable memory census as a completed
/// certificate, without manufacturing a certificate for an unsupported atom.
#[derive(Debug)]
pub(crate) enum ResidualAffineAtomRowFreshCompilationAttempt {
    Complete {
        certificate: ResidualAffineAtomRowCertificate,
        logical_memory_census: ResidualAffineAtomRowAttemptLogicalMemoryCensus,
    },
    Unsupported {
        reason: ResidualAffineAtomRowUnsupported,
        logical_memory_census: ResidualAffineAtomRowAttemptLogicalMemoryCensus,
    },
}

impl ResidualAffineAtomRowFreshCompilationAttempt {
    pub(crate) const fn logical_memory_census(
        &self,
    ) -> ResidualAffineAtomRowAttemptLogicalMemoryCensus {
        match self {
            Self::Complete {
                logical_memory_census,
                ..
            }
            | Self::Unsupported {
                logical_memory_census,
                ..
            } => *logical_memory_census,
        }
    }
}

/// Canonical component order is `[constant, coefficient(n_0), ...]`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResidualAffinePrimitiveRow {
    components: Vec<Integer>,
}

impl ResidualAffinePrimitiveRow {
    pub fn arity(&self) -> usize {
        self.components.len() - 1
    }

    pub fn constant(&self) -> &Integer {
        &self.components[0]
    }

    pub fn coefficients(&self) -> &[Integer] {
        &self.components[1..]
    }

    pub fn coefficient(&self, position: usize) -> Option<&Integer> {
        self.coefficients().get(position)
    }

    pub fn components(&self) -> &[Integer] {
        &self.components
    }

    /// Checked internal boundary for rows assembled by a future simultaneous
    /// integer-affine solver.  The caller supplies an already canonical row;
    /// this function validates, rather than silently normalizes, its content.
    pub(crate) fn try_from_canonical_components_with_limits(
        components: Vec<Integer>,
        max_components: usize,
        max_integer_coefficient_bits: usize,
        max_integer_bit_work: usize,
    ) -> Result<Self, ResidualAffinePrimitiveRowError> {
        catch_unwind(AssertUnwindSafe(|| {
            validate_canonical_primitive_row(
                &components,
                max_components,
                max_integer_coefficient_bits,
                max_integer_bit_work,
            )?;
            Ok(Self { components })
        }))
        .map_err(|_| ResidualAffinePrimitiveRowError::SymbolicaPanic)?
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResidualAffinePrimitiveRowError {
    EmptyComponents,
    ZeroRow,
    NotPrimitive,
    SignNotCanonical,
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    SymbolicaPanic,
}

impl fmt::Display for ResidualAffinePrimitiveRowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyComponents => {
                formatter.write_str("an affine row must have a constant component")
            }
            Self::ZeroRow => formatter.write_str("the all-zero row is not a primitive affine row"),
            Self::NotPrimitive => {
                formatter.write_str("the affine row components do not have gcd one")
            }
            Self::SignNotCanonical => {
                formatter.write_str("the first nonzero affine-row component must be positive")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "primitive-row {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "primitive-row {resource} count overflowed usize")
            }
            Self::SymbolicaPanic => {
                formatter.write_str("Symbolica panicked while validating a primitive affine row")
            }
        }
    }
}

impl std::error::Error for ResidualAffinePrimitiveRowError {}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResidualAffineBaseBlockWitness {
    base_exponents: Vec<u16>,
    signed_scalar: Integer,
}

impl ResidualAffineBaseBlockWitness {
    pub fn base_exponents(&self) -> &[u16] {
        &self.base_exponents
    }

    pub fn signed_scalar(&self) -> &Integer {
        &self.signed_scalar
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ResidualAffineAtomRowOutcome {
    Row,
    RedundantZeroPolynomial,
    InconsistentNonzeroConstant,
}

/// A completeness boundary, never a proof that the source branch is empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResidualAffineAtomRowUnsupported {
    NonAffineIndexMonomial { term_ordinal: usize },
    NonAssociateBaseBlock { block_ordinal: usize },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResidualAffineAtomRowError {
    SchemaMismatch,
    ReplayMismatch,
    WrongContext,
    Unsupported {
        reason: ResidualAffineAtomRowUnsupported,
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
    Coefficient(ParametricCoefficientError),
}

impl fmt::Display for ResidualAffineAtomRowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("residual affine-atom row schema mismatch"),
            Self::ReplayMismatch => formatter.write_str("residual affine-atom row did not replay"),
            Self::WrongContext => {
                formatter.write_str("residual affine-atom row belongs to another K(n) context")
            }
            Self::Unsupported { reason } => {
                write!(formatter, "unsupported residual affine atom: {reason:?}")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "residual affine-atom {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "residual affine-atom {resource} count overflowed usize"
                )
            }
            Self::AllocationFailure { resource } => write!(
                formatter,
                "residual affine-atom {resource} allocation failed after bounded preflight"
            ),
            Self::SymbolicaPanic => {
                formatter.write_str("Symbolica panicked during residual affine-atom recognition")
            }
            Self::Coefficient(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ResidualAffineAtomRowError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Coefficient(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ParametricCoefficientError> for ResidualAffineAtomRowError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::Coefficient(value)
    }
}

#[derive(Clone, Debug)]
pub struct ResidualAffineAtomRowCertificate {
    schema: &'static str,
    context_fingerprint: String,
    source: ParametricPolynomial,
    outcome: ResidualAffineAtomRowOutcome,
    primitive_row: Option<ResidualAffinePrimitiveRow>,
    block_witnesses: Vec<ResidualAffineBaseBlockWitness>,
    limits: ResidualAffineAtomRowLimits,
    stats: ResidualAffineAtomRowStats,
}

impl ResidualAffineAtomRowCertificate {
    pub fn compile(
        context: &ParametricCoefficientContext,
        source: ParametricPolynomial,
        limits: ResidualAffineAtomRowLimits,
    ) -> Result<Self, ResidualAffineAtomRowError> {
        catch_unwind(AssertUnwindSafe(|| compile_inner(context, source, limits)))
            .map_err(|_| ResidualAffineAtomRowError::SymbolicaPanic)?
    }

    /// Compile one V2 child attempt with an adjacent, allocation-independent
    /// logical-memory census. The census is a cheap shape/bit scan; the exact
    /// gcd/quotient recognizer below still runs exactly once.
    pub(crate) fn compile_fresh(
        context: &ParametricCoefficientContext,
        source: ParametricPolynomial,
        limits: ResidualAffineAtomRowLimits,
    ) -> Result<ResidualAffineAtomRowFreshCompilationAttempt, ResidualAffineAtomRowError> {
        let logical_memory_census =
            residual_affine_atom_row_attempt_logical_memory_census(context, &source, limits)?;
        match Self::compile(context, source, limits) {
            Ok(certificate) => Ok(ResidualAffineAtomRowFreshCompilationAttempt::Complete {
                certificate,
                logical_memory_census,
            }),
            Err(ResidualAffineAtomRowError::Unsupported { reason }) => {
                Ok(ResidualAffineAtomRowFreshCompilationAttempt::Unsupported {
                    reason,
                    logical_memory_census,
                })
            }
            Err(error) => Err(error),
        }
    }

    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }

    pub const fn source(&self) -> &ParametricPolynomial {
        &self.source
    }

    pub const fn outcome(&self) -> ResidualAffineAtomRowOutcome {
        self.outcome
    }

    /// The usable affine row.  Constant inconsistency deliberately returns
    /// `None`, even though its normalized diagnostic primitive is retained.
    pub fn row(&self) -> Option<&ResidualAffinePrimitiveRow> {
        matches!(self.outcome, ResidualAffineAtomRowOutcome::Row)
            .then_some(())
            .and(self.primitive_row.as_ref())
    }

    /// The normalized primitive for either a row or a nonzero constant.
    pub fn primitive_row(&self) -> Option<&ResidualAffinePrimitiveRow> {
        self.primitive_row.as_ref()
    }

    pub fn block_witnesses(&self) -> &[ResidualAffineBaseBlockWitness] {
        &self.block_witnesses
    }

    pub const fn limits(&self) -> ResidualAffineAtomRowLimits {
        self.limits
    }

    pub const fn stats(&self) -> ResidualAffineAtomRowStats {
        self.stats
    }

    pub fn replay(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), ResidualAffineAtomRowError> {
        if self.schema != RESIDUAL_AFFINE_ATOM_ROW_V1_SCHEMA {
            return Err(ResidualAffineAtomRowError::SchemaMismatch);
        }
        if self.context_fingerprint != context.fingerprint() {
            return Err(ResidualAffineAtomRowError::WrongContext);
        }
        let replayed = catch_unwind(AssertUnwindSafe(|| {
            compile_payload(context, &self.source, self.limits)
        }))
        .map_err(|_| ResidualAffineAtomRowError::SymbolicaPanic)??;
        if self.outcome == replayed.outcome
            && self.primitive_row == replayed.primitive_row
            && self.block_witnesses == replayed.block_witnesses
            && self.stats == replayed.stats
        {
            Ok(())
        } else {
            Err(ResidualAffineAtomRowError::ReplayMismatch)
        }
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.context_fingerprint == other.context_fingerprint
            && self.source == other.source
            && self.outcome == other.outcome
            && self.primitive_row == other.primitive_row
            && self.block_witnesses == other.block_witnesses
            && self.limits == other.limits
            && self.stats == other.stats
    }
}

struct RecognitionPayload {
    outcome: ResidualAffineAtomRowOutcome,
    primitive_row: Option<ResidualAffinePrimitiveRow>,
    block_witnesses: Vec<ResidualAffineBaseBlockWitness>,
    stats: ResidualAffineAtomRowStats,
}

fn compile_inner(
    context: &ParametricCoefficientContext,
    source: ParametricPolynomial,
    limits: ResidualAffineAtomRowLimits,
) -> Result<ResidualAffineAtomRowCertificate, ResidualAffineAtomRowError> {
    check_limit(
        "context fingerprint bytes",
        context.fingerprint().len(),
        limits.max_context_fingerprint_bytes,
    )?;
    let mut context_fingerprint = String::new();
    context_fingerprint
        .try_reserve_exact(context.fingerprint().len())
        .map_err(|_| ResidualAffineAtomRowError::AllocationFailure {
            resource: "context fingerprint",
        })?;
    context_fingerprint.push_str(context.fingerprint());

    let payload = compile_payload(context, &source, limits)?;
    Ok(ResidualAffineAtomRowCertificate {
        schema: RESIDUAL_AFFINE_ATOM_ROW_V1_SCHEMA,
        context_fingerprint,
        source,
        outcome: payload.outcome,
        primitive_row: payload.primitive_row,
        block_witnesses: payload.block_witnesses,
        limits,
        stats: payload.stats,
    })
}

fn compile_payload(
    context: &ParametricCoefficientContext,
    source: &ParametricPolynomial,
    limits: ResidualAffineAtomRowLimits,
) -> Result<RecognitionPayload, ResidualAffineAtomRowError> {
    let raw = source.raw();
    let arity = context.index_count();
    let base_variables = context.base().variables().len();
    let source_variables = raw.variables.len();
    let source_terms = raw.nterms();
    let expected_variables = checked_add("source variables", base_variables, arity)?;
    let exponent_entries =
        checked_mul("exponent entries inspected", source_terms, source_variables)?;
    let row_components = checked_add("primitive row components", arity, 1)?;

    check_limit(
        "context fingerprint bytes",
        context.fingerprint().len(),
        limits.max_context_fingerprint_bytes,
    )?;
    check_limit("ambient arity", arity, limits.max_ambient_arity)?;
    check_limit("base variables", base_variables, limits.max_base_variables)?;
    check_limit(
        "source variables",
        source_variables,
        limits.max_source_variables,
    )?;
    check_limit("source terms", source_terms, limits.max_source_terms)?;
    check_limit(
        "exponent entries inspected",
        exponent_entries,
        limits.max_exponent_entries_inspected,
    )?;
    check_limit(
        "primitive row components",
        row_components,
        limits.max_primitive_row_components,
    )?;
    // Every canonical sparse term participates in one gcd and one exact
    // quotient.  Reject these complete cardinalities before either loop.
    check_limit("gcd operations", source_terms, limits.max_gcd_operations)?;
    check_limit(
        "exact quotient operations",
        source_terms,
        limits.max_exact_quotient_operations,
    )?;

    context.validate_polynomial_with_limits(source, limits.exact_algebra)?;
    if source_variables != expected_variables || raw.coefficients.len() != source_terms {
        return Err(ResidualAffineAtomRowError::ReplayMismatch);
    }

    if source_terms == 0 {
        return Ok(RecognitionPayload {
            outcome: ResidualAffineAtomRowOutcome::RedundantZeroPolynomial,
            primitive_row: None,
            block_witnesses: Vec::new(),
            stats: ResidualAffineAtomRowStats {
                context_fingerprint_bytes: context.fingerprint().len(),
                ambient_arity: arity,
                base_variables,
                source_variables,
                source_terms,
                exponent_entries_inspected: exponent_entries,
                primitive_row_components: row_components,
                ..ResidualAffineAtomRowStats::default()
            },
        });
    }

    let mut component_ordinals = Vec::<Option<usize>>::new();
    component_ordinals
        .try_reserve_exact(row_components)
        .map_err(|_| ResidualAffineAtomRowError::AllocationFailure {
            resource: "temporary affine components",
        })?;
    component_ordinals.resize(row_components, None);

    let mut block_witnesses = Vec::<ResidualAffineBaseBlockWitness>::new();
    let mut common_row: Option<ResidualAffinePrimitiveRow> = None;
    let mut affine_blocks = 0usize;
    let mut retained_block_exponent_entries = 0usize;
    let mut recognition_operations = 0usize;
    let mut gcd_operations = 0usize;
    let mut exact_quotient_operations = 0usize;
    let mut integer_bit_work = 0usize;
    let mut largest_integer_coefficient_bits = 0usize;
    let mut block_start = 0usize;

    while block_start < source_terms {
        let block_ordinal = affine_blocks;
        affine_blocks = bounded_add("affine blocks", affine_blocks, 1, limits.max_affine_blocks)?;
        let retained_witnesses = checked_add("retained block witnesses", block_witnesses.len(), 1)?;
        check_limit(
            "retained block witnesses",
            retained_witnesses,
            limits.max_retained_block_witnesses,
        )?;

        let prefix = &raw.exponents(block_start)[..base_variables];
        let mut block_end = checked_add("source terms", block_start, 1)?;
        while block_end < source_terms {
            recognition_operations = bounded_add(
                "recognition operations",
                recognition_operations,
                base_variables,
                limits.max_recognition_operations,
            )?;
            if &raw.exponents(block_end)[..base_variables] != prefix {
                break;
            }
            block_end += 1;
        }

        recognition_operations = bounded_add(
            "recognition operations",
            recognition_operations,
            row_components,
            limits.max_recognition_operations,
        )?;
        component_ordinals.fill(None);
        for term_ordinal in block_start..block_end {
            let exponents = raw.exponents(term_ordinal);
            recognition_operations = bounded_add(
                "recognition operations",
                recognition_operations,
                checked_add("recognition operations", arity, 1)?,
                limits.max_recognition_operations,
            )?;
            let component = classify_affine_component(exponents, base_variables, arity).ok_or(
                ResidualAffineAtomRowError::Unsupported {
                    reason: ResidualAffineAtomRowUnsupported::NonAffineIndexMonomial {
                        term_ordinal,
                    },
                },
            )?;
            if component_ordinals[component]
                .replace(term_ordinal)
                .is_some()
            {
                return Err(ResidualAffineAtomRowError::ReplayMismatch);
            }
        }

        // `flatten()` still visits every absent component slot. Charge the
        // complete dense traversal before entering it; gcd operations below
        // count only the retained sparse terms themselves.
        recognition_operations = bounded_add(
            "recognition operations",
            recognition_operations,
            row_components,
            limits.max_recognition_operations,
        )?;
        let mut gcd = Integer::from(0);
        for term_ordinal in component_ordinals.iter().flatten().copied() {
            let coefficient = &raw.coefficients[term_ordinal];
            if coefficient.is_zero() {
                return Err(ResidualAffineAtomRowError::ReplayMismatch);
            }
            let coefficient_bits = integer_magnitude_bits(coefficient)?;
            check_limit(
                "integer coefficient bits",
                coefficient_bits,
                limits.max_integer_coefficient_bits,
            )?;
            largest_integer_coefficient_bits =
                largest_integer_coefficient_bits.max(coefficient_bits);
            let gcd_bits = integer_magnitude_bits(&gcd)?;
            integer_bit_work = bounded_bit_work(
                integer_bit_work,
                coefficient_bits,
                gcd_bits,
                limits.max_integer_bit_work,
            )?;
            gcd_operations = bounded_add(
                "gcd operations",
                gcd_operations,
                1,
                limits.max_gcd_operations,
            )?;
            gcd = Z.gcd(&gcd, coefficient);
        }
        if gcd.is_zero() {
            return Err(ResidualAffineAtomRowError::ReplayMismatch);
        }
        let gcd_bits = integer_magnitude_bits(&gcd)?;
        check_limit(
            "integer coefficient bits",
            gcd_bits,
            limits.max_integer_coefficient_bits,
        )?;
        largest_integer_coefficient_bits = largest_integer_coefficient_bits.max(gcd_bits);

        // The first present component may be the final index slot, so charge
        // the full-width search rather than its data-dependent exit distance.
        recognition_operations = bounded_add(
            "recognition operations",
            recognition_operations,
            row_components,
            limits.max_recognition_operations,
        )?;
        let first_component = component_ordinals
            .iter()
            .flatten()
            .next()
            .copied()
            .ok_or(ResidualAffineAtomRowError::ReplayMismatch)?;
        let negate_row = raw.coefficients[first_component].is_negative();
        let mut normalized = Vec::<Integer>::new();
        normalized.try_reserve_exact(row_components).map_err(|_| {
            ResidualAffineAtomRowError::AllocationFailure {
                resource: "normalized primitive row",
            }
        })?;
        for component_ordinal in &component_ordinals {
            recognition_operations = bounded_add(
                "recognition operations",
                recognition_operations,
                1,
                limits.max_recognition_operations,
            )?;
            let quotient = if let Some(term_ordinal) = component_ordinal {
                let coefficient = &raw.coefficients[*term_ordinal];
                let coefficient_bits = integer_magnitude_bits(coefficient)?;
                integer_bit_work = bounded_bit_work(
                    integer_bit_work,
                    coefficient_bits,
                    gcd_bits,
                    limits.max_integer_bit_work,
                )?;
                exact_quotient_operations = bounded_add(
                    "exact quotient operations",
                    exact_quotient_operations,
                    1,
                    limits.max_exact_quotient_operations,
                )?;
                let (mut quotient, remainder) = Z.quot_rem(coefficient, &gcd);
                if !remainder.is_zero() {
                    return Err(ResidualAffineAtomRowError::ReplayMismatch);
                }
                let quotient_bits = integer_magnitude_bits(&quotient)?;
                check_limit(
                    "integer coefficient bits",
                    quotient_bits,
                    limits.max_integer_coefficient_bits,
                )?;
                largest_integer_coefficient_bits =
                    largest_integer_coefficient_bits.max(quotient_bits);
                if negate_row {
                    integer_bit_work = bounded_add(
                        "integer bit work",
                        integer_bit_work,
                        quotient_bits,
                        limits.max_integer_bit_work,
                    )?;
                    quotient = -quotient;
                }
                quotient
            } else {
                Integer::from(0)
            };
            normalized.push(quotient);
        }
        let signed_scalar = if negate_row {
            integer_bit_work = bounded_add(
                "integer bit work",
                integer_bit_work,
                gcd_bits,
                limits.max_integer_bit_work,
            )?;
            -gcd
        } else {
            gcd
        };
        let normalized = ResidualAffinePrimitiveRow {
            components: normalized,
        };
        if let Some(expected) = &common_row {
            for (expected_component, actual_component) in
                expected.components().iter().zip(normalized.components())
            {
                recognition_operations = bounded_add(
                    "recognition operations",
                    recognition_operations,
                    1,
                    limits.max_recognition_operations,
                )?;
                integer_bit_work = bounded_add(
                    "integer bit work",
                    integer_bit_work,
                    integer_magnitude_bits(expected_component)?
                        .max(integer_magnitude_bits(actual_component)?),
                    limits.max_integer_bit_work,
                )?;
                if expected_component != actual_component {
                    return Err(ResidualAffineAtomRowError::Unsupported {
                        reason: ResidualAffineAtomRowUnsupported::NonAssociateBaseBlock {
                            block_ordinal,
                        },
                    });
                }
            }
        } else {
            common_row = Some(normalized);
        }

        retained_block_exponent_entries = bounded_add(
            "retained block exponent entries",
            retained_block_exponent_entries,
            base_variables,
            limits.max_retained_block_exponent_entries,
        )?;
        let mut base_exponents = Vec::<u16>::new();
        base_exponents
            .try_reserve_exact(base_variables)
            .map_err(|_| ResidualAffineAtomRowError::AllocationFailure {
                resource: "block base exponents",
            })?;
        base_exponents.extend_from_slice(prefix);
        block_witnesses.try_reserve(1).map_err(|_| {
            ResidualAffineAtomRowError::AllocationFailure {
                resource: "block witnesses",
            }
        })?;
        block_witnesses.push(ResidualAffineBaseBlockWitness {
            base_exponents,
            signed_scalar,
        });
        block_start = block_end;
    }

    if gcd_operations != source_terms || exact_quotient_operations != source_terms {
        return Err(ResidualAffineAtomRowError::ReplayMismatch);
    }
    let primitive_row = common_row.ok_or(ResidualAffineAtomRowError::ReplayMismatch)?;
    // All-zero and sign invariants were established for every block above.
    // Avoid debug-only rescans here, which would make the authoritative work
    // census build-profile dependent. The final coefficient classification
    // itself is a full scan of the index-component tail.
    recognition_operations = bounded_add(
        "recognition operations",
        recognition_operations,
        arity,
        limits.max_recognition_operations,
    )?;
    let outcome = if primitive_row.coefficients().iter().all(Integer::is_zero) {
        ResidualAffineAtomRowOutcome::InconsistentNonzeroConstant
    } else {
        ResidualAffineAtomRowOutcome::Row
    };
    let stats = ResidualAffineAtomRowStats {
        context_fingerprint_bytes: context.fingerprint().len(),
        ambient_arity: arity,
        base_variables,
        source_variables,
        source_terms,
        exponent_entries_inspected: exponent_entries,
        affine_blocks,
        retained_block_witnesses: block_witnesses.len(),
        retained_block_exponent_entries,
        primitive_row_components: row_components,
        recognition_operations,
        gcd_operations,
        exact_quotient_operations,
        largest_integer_coefficient_bits,
        integer_bit_work,
    };
    Ok(RecognitionPayload {
        outcome,
        primitive_row: Some(primitive_row),
        block_witnesses,
        stats,
    })
}

/// Recompute a conservative logical-memory peak for exactly one recognition
/// attempt without cloning the source polynomial or rerunning any GMP-producing
/// recognition algebra.
///
/// The scan uses initialized sparse lengths, actual source `Integer::Large`
/// payloads, base-prefix block counts, and the largest initialized source
/// coefficient bit length. A gcd, exact quotient, normalized component, or
/// signed scalar cannot exceed that source bit length. Assuming every
/// structurally present block reaches retention consequently covers a late
/// non-associate exit (and may conservatively include a never-visited suffix)
/// without a second gcd/quotient pass.
pub(crate) fn residual_affine_atom_row_attempt_logical_memory_census(
    context: &ParametricCoefficientContext,
    source: &ParametricPolynomial,
    limits: ResidualAffineAtomRowLimits,
) -> Result<ResidualAffineAtomRowAttemptLogicalMemoryCensus, ResidualAffineAtomRowError> {
    catch_unwind(AssertUnwindSafe(|| {
        residual_affine_atom_row_attempt_logical_memory_census_inner(context, source, limits)
    }))
    .map_err(|_| ResidualAffineAtomRowError::SymbolicaPanic)?
}

fn residual_affine_atom_row_attempt_logical_memory_census_inner(
    context: &ParametricCoefficientContext,
    source: &ParametricPolynomial,
    limits: ResidualAffineAtomRowLimits,
) -> Result<ResidualAffineAtomRowAttemptLogicalMemoryCensus, ResidualAffineAtomRowError> {
    let raw = source.raw();
    let arity = context.index_count();
    let base_variables = context.base().variables().len();
    let source_variables = raw.variables.len();
    let source_terms = raw.nterms();
    let expected_variables = checked_add("source variables", base_variables, arity)?;
    let exponent_entries =
        checked_mul("exponent entries inspected", source_terms, source_variables)?;
    let row_components = checked_add("primitive row components", arity, 1)?;

    check_limit(
        "context fingerprint bytes",
        context.fingerprint().len(),
        limits.max_context_fingerprint_bytes,
    )?;
    check_limit("ambient arity", arity, limits.max_ambient_arity)?;
    check_limit("base variables", base_variables, limits.max_base_variables)?;
    check_limit(
        "source variables",
        source_variables,
        limits.max_source_variables,
    )?;
    check_limit("source terms", source_terms, limits.max_source_terms)?;
    check_limit(
        "exponent entries inspected",
        exponent_entries,
        limits.max_exponent_entries_inspected,
    )?;
    check_limit(
        "primitive row components",
        row_components,
        limits.max_primitive_row_components,
    )?;
    context.validate_polynomial_with_limits(source, limits.exact_algebra)?;
    if source_variables != expected_variables || raw.coefficients.len() != source_terms {
        return Err(ResidualAffineAtomRowError::ReplayMismatch);
    }

    let resource = "affine-atom attempt logical memory";
    let source_dynamic_logical_bytes = residual_affine_atom_source_dynamic_logical_bytes(source)?;
    let source_owned_logical_bytes = checked_add(
        resource,
        size_of::<ParametricPolynomial>(),
        source_dynamic_logical_bytes,
    )?;
    let context_fingerprint_copy_bytes =
        checked_add(resource, size_of::<String>(), context.fingerprint().len())?;
    let base_owned_logical_bytes = [
        source_owned_logical_bytes,
        context_fingerprint_copy_bytes,
        size_of::<ResidualAffineAtomRowAttemptLogicalMemoryCensus>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| checked_add(resource, sum, bytes))?;
    let unsupported_output_bytes = size_of::<ResidualAffineAtomRowFreshCompilationAttempt>();
    let complete_output_fixed_and_source_bytes = [
        size_of::<ResidualAffineAtomRowFreshCompilationAttempt>(),
        context.fingerprint().len(),
        source_dynamic_logical_bytes,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| checked_add(resource, sum, bytes))?;
    if source_terms == 0 {
        return Ok(ResidualAffineAtomRowAttemptLogicalMemoryCensus {
            owned_logical_peak_upper_bound: base_owned_logical_bytes
                .max(complete_output_fixed_and_source_bytes)
                .max(unsupported_output_bytes),
        });
    }

    let component_ordinal_bytes = checked_add(
        resource,
        size_of::<Vec<Option<usize>>>(),
        checked_mul(resource, row_components, size_of::<Option<usize>>())?,
    )?;
    let fixed_recognition_bytes = [
        base_owned_logical_bytes,
        component_ordinal_bytes,
        size_of::<Vec<ResidualAffineBaseBlockWitness>>(),
        size_of::<Option<ResidualAffinePrimitiveRow>>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| checked_add(resource, sum, bytes))?;

    let mut affine_blocks = 1usize;
    for term_ordinal in 1..source_terms {
        let previous = &raw.exponents(term_ordinal - 1)[..base_variables];
        let current = &raw.exponents(term_ordinal)[..base_variables];
        if previous != current {
            affine_blocks = checked_add("affine blocks", affine_blocks, 1)?;
        }
    }
    check_limit("affine blocks", affine_blocks, limits.max_affine_blocks)?;
    check_limit(
        "retained block witnesses",
        affine_blocks,
        limits.max_retained_block_witnesses,
    )?;
    let witness_exponent_entries = checked_mul(
        "retained block exponent entries",
        affine_blocks,
        base_variables,
    )?;
    check_limit(
        "retained block exponent entries",
        witness_exponent_entries,
        limits.max_retained_block_exponent_entries,
    )?;

    let mut largest_source_integer_bits = 0usize;
    for coefficient in &raw.coefficients {
        let bits = integer_magnitude_bits(coefficient)?;
        check_limit(
            "integer coefficient bits",
            bits,
            limits.max_integer_coefficient_bits,
        )?;
        largest_source_integer_bits = largest_source_integer_bits.max(bits);
    }
    let bounded_large_payload =
        residual_affine_atom_gmp_dynamic_logical_byte_upper_bound(largest_source_integer_bits)?;
    let bounded_integer = checked_add(resource, size_of::<Integer>(), bounded_large_payload)?;
    let normalized_row_payload = checked_mul(resource, row_components, bounded_integer)?;
    // At the first block one normalized row becomes the common row. From the
    // second block onward the common and current normalized rows coexist.
    let live_normalized_rows = checked_add(resource, 1, usize::from(affine_blocks > 1))?;
    let normalized_rows_bytes = checked_add(
        resource,
        checked_mul(resource, live_normalized_rows, normalized_row_payload)?,
        checked_mul(
            resource,
            usize::from(affine_blocks > 1),
            size_of::<ResidualAffinePrimitiveRow>(),
        )?,
    )?;
    let witness_bytes = [
        checked_mul(
            resource,
            affine_blocks,
            size_of::<ResidualAffineBaseBlockWitness>(),
        )?,
        checked_mul(resource, witness_exponent_entries, size_of::<u16>())?,
        checked_mul(resource, affine_blocks, bounded_large_payload)?,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| checked_add(resource, sum, bytes))?;
    let current_base_exponent_bytes = checked_add(
        resource,
        size_of::<Vec<u16>>(),
        checked_mul(resource, base_variables, size_of::<u16>())?,
    )?;
    // Old/new gcd or quotient, a remainder, and a possible negated result are
    // the largest simultaneously owned GMP temporaries in the recognizer.
    let integer_temporary_bytes = checked_mul(resource, 4, bounded_integer)?;
    let working_peak = [
        fixed_recognition_bytes,
        witness_bytes,
        normalized_rows_bytes,
        current_base_exponent_bytes,
        integer_temporary_bytes,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| checked_add(resource, sum, bytes))?;
    // The working vectors move into a returned certificate on success, so the
    // complete phase counts the fresh-attempt wrapper once and then only the
    // dynamic payload whose headers are already inside that wrapper. An
    // unsupported result owns neither source nor partial recognition vectors.
    let complete_output_bytes = [
        complete_output_fixed_and_source_bytes,
        normalized_row_payload,
        witness_bytes,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| checked_add(resource, sum, bytes))?;
    let peak = working_peak
        .max(complete_output_bytes)
        .max(unsupported_output_bytes);
    Ok(ResidualAffineAtomRowAttemptLogicalMemoryCensus {
        owned_logical_peak_upper_bound: peak,
    })
}

fn residual_affine_atom_source_dynamic_logical_bytes(
    source: &ParametricPolynomial,
) -> Result<usize, ResidualAffineAtomRowError> {
    let resource = "affine-atom attempt logical memory";
    let raw = source.raw();
    let coefficient_bytes =
        residual_affine_atom_integer_slice_dynamic_logical_bytes(&raw.coefficients)?;
    let exponent_bytes = checked_mul(resource, raw.exponents.len(), size_of::<u16>())?;
    [coefficient_bytes, exponent_bytes]
        .into_iter()
        .try_fold(0usize, |sum, bytes| checked_add(resource, sum, bytes))
}

fn residual_affine_atom_integer_slice_dynamic_logical_bytes(
    values: &[Integer],
) -> Result<usize, ResidualAffineAtomRowError> {
    let resource = "affine-atom attempt logical memory";
    values.iter().try_fold(
        checked_mul(resource, values.len(), size_of::<Integer>())?,
        |sum, value| {
            checked_add(
                resource,
                sum,
                residual_affine_atom_large_integer_dynamic_logical_bytes(value)?,
            )
        },
    )
}

fn residual_affine_atom_large_integer_dynamic_logical_bytes(
    value: &Integer,
) -> Result<usize, ResidualAffineAtomRowError> {
    let Integer::Large(value) = value else {
        return Ok(0);
    };
    let resource = "affine-atom attempt logical memory";
    let bits = usize::try_from(value.significant_bits())
        .map_err(|_| ResidualAffineAtomRowError::ResourceCountOverflow { resource })?;
    bits.checked_add(7)
        .and_then(|bits| bits.checked_div(8))
        .and_then(|bytes| bytes.checked_add(size_of::<usize>()))
        .ok_or(ResidualAffineAtomRowError::ResourceCountOverflow { resource })
}

fn residual_affine_atom_gmp_dynamic_logical_byte_upper_bound(
    bits: usize,
) -> Result<usize, ResidualAffineAtomRowError> {
    if bits == 0 {
        return Ok(0);
    }
    let resource = "affine-atom attempt logical memory";
    bits.checked_add(7)
        .and_then(|bits| bits.checked_div(8))
        .and_then(|bytes| bytes.checked_add(size_of::<usize>()))
        .ok_or(ResidualAffineAtomRowError::ResourceCountOverflow { resource })
}

fn classify_affine_component(
    exponents: &[u16],
    base_variables: usize,
    arity: usize,
) -> Option<usize> {
    let mut component = 0usize;
    for (position, &exponent) in exponents[base_variables..base_variables + arity]
        .iter()
        .enumerate()
    {
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

fn validate_canonical_primitive_row(
    components: &[Integer],
    max_components: usize,
    max_integer_coefficient_bits: usize,
    max_integer_bit_work: usize,
) -> Result<(), ResidualAffinePrimitiveRowError> {
    if components.is_empty() {
        return Err(ResidualAffinePrimitiveRowError::EmptyComponents);
    }
    primitive_check_limit("components", components.len(), max_components)?;
    let mut gcd = Integer::from(0);
    let mut bit_work = 0usize;
    let mut first_nonzero_negative = None;
    for component in components {
        let bits = primitive_integer_magnitude_bits(component)?;
        primitive_check_limit(
            "integer coefficient bits",
            bits,
            max_integer_coefficient_bits,
        )?;
        let gcd_bits = primitive_integer_magnitude_bits(&gcd)?;
        let work = bits.max(1).checked_mul(gcd_bits.max(1)).ok_or(
            ResidualAffinePrimitiveRowError::ResourceCountOverflow {
                resource: "integer bit work",
            },
        )?;
        bit_work = bit_work.checked_add(work).ok_or(
            ResidualAffinePrimitiveRowError::ResourceCountOverflow {
                resource: "integer bit work",
            },
        )?;
        primitive_check_limit("integer bit work", bit_work, max_integer_bit_work)?;
        if !component.is_zero() && first_nonzero_negative.is_none() {
            first_nonzero_negative = Some(component.is_negative());
        }
        gcd = Z.gcd(&gcd, component);
    }
    match first_nonzero_negative {
        None => return Err(ResidualAffinePrimitiveRowError::ZeroRow),
        Some(true) => return Err(ResidualAffinePrimitiveRowError::SignNotCanonical),
        Some(false) => {}
    }
    if !gcd.is_one() {
        return Err(ResidualAffinePrimitiveRowError::NotPrimitive);
    }
    Ok(())
}

fn primitive_integer_magnitude_bits(
    value: &Integer,
) -> Result<usize, ResidualAffinePrimitiveRowError> {
    integer_magnitude_bits_raw(value).map_err(|_| {
        ResidualAffinePrimitiveRowError::ResourceCountOverflow {
            resource: "integer coefficient bits",
        }
    })
}

fn primitive_check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ResidualAffinePrimitiveRowError> {
    if requested > limit {
        Err(ResidualAffinePrimitiveRowError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn integer_magnitude_bits(value: &Integer) -> Result<usize, ResidualAffineAtomRowError> {
    integer_magnitude_bits_raw(value).map_err(|_| {
        ResidualAffineAtomRowError::ResourceCountOverflow {
            resource: "integer coefficient bits",
        }
    })
}

fn integer_magnitude_bits_raw(value: &Integer) -> Result<usize, ()> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| ())
}

fn bounded_bit_work(
    current: usize,
    left_bits: usize,
    right_bits: usize,
    limit: usize,
) -> Result<usize, ResidualAffineAtomRowError> {
    let work = checked_mul("integer bit work", left_bits.max(1), right_bits.max(1))?;
    bounded_add("integer bit work", current, work, limit)
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ResidualAffineAtomRowError> {
    left.checked_add(right)
        .ok_or(ResidualAffineAtomRowError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ResidualAffineAtomRowError> {
    left.checked_mul(right)
        .ok_or(ResidualAffineAtomRowError::ResourceCountOverflow { resource })
}

fn bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, ResidualAffineAtomRowError> {
    let requested = checked_add(resource, left, right)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ResidualAffineAtomRowError> {
    if requested > limit {
        Err(ResidualAffineAtomRowError::ResourceLimit {
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
    use crate::{CoefficientContext, ParametricCoefficient};

    fn context(
        scope: &str,
        base_names: &[&str],
        arity: usize,
    ) -> (CoefficientContext, ParametricCoefficientContext) {
        let base = CoefficientContext::new(base_names.iter().copied());
        let context = ParametricCoefficientContext::try_new(&base, scope, arity).unwrap();
        (base, context)
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
    ) -> ParametricCoefficient {
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
        value
    }

    fn power_two(context: &ParametricCoefficientContext, exponent: usize) -> ParametricCoefficient {
        let mut value = context.one();
        for _ in 0..exponent {
            value = context.add(&value, &value).unwrap();
        }
        value
    }

    #[test]
    fn recognizes_multiple_base_prefix_blocks_and_signed_associate_witnesses() {
        let (base, context) = context("affine-atom-multi-block", &["theta", "phi"], 2);
        let theta = context.lift(&base.parameter("theta").unwrap()).unwrap();
        let phi = context.lift(&base.parameter("phi").unwrap()).unwrap();
        let theta_phi = context.mul(&theta, &phi).unwrap();
        let first = context.mul(&context.integer(6), &theta_phi).unwrap();
        let second = context.mul(&context.integer(-10), &theta).unwrap();
        let third = context.integer(14);
        let factor = context
            .add(&context.add(&first, &second).unwrap(), &third)
            .unwrap();
        let source = polynomial(
            &context,
            &context
                .mul(&factor, &affine(&context, 1, &[2, -3]))
                .unwrap(),
        );

        let certificate = ResidualAffineAtomRowCertificate::compile(
            &context,
            source,
            ResidualAffineAtomRowLimits::default(),
        )
        .unwrap();
        assert_eq!(certificate.outcome(), ResidualAffineAtomRowOutcome::Row);
        assert_eq!(
            certificate.row().unwrap().components(),
            &[Integer::from(1), Integer::from(2), Integer::from(-3)]
        );
        let mut scalars = certificate
            .block_witnesses()
            .iter()
            .map(|witness| witness.signed_scalar().clone())
            .collect::<Vec<_>>();
        scalars.sort();
        assert_eq!(
            scalars,
            vec![Integer::from(-10), Integer::from(6), Integer::from(14)]
        );
        assert_eq!(certificate.block_witnesses().len(), 3);
        certificate.replay(&context).unwrap();
    }

    #[test]
    fn recognition_census_charges_absent_component_slots_and_final_classification() {
        let (base, context) = context("affine-atom-dense-scan-census", &["theta"], 4);
        let theta = context.lift(&base.parameter("theta").unwrap()).unwrap();
        let factor = context.add(&theta, &context.one()).unwrap();
        let source = polynomial(
            &context,
            &context.mul(&factor, &context.index(3).unwrap()).unwrap(),
        );
        let certificate = ResidualAffineAtomRowCertificate::compile(
            &context,
            source.clone(),
            ResidualAffineAtomRowLimits::default(),
        )
        .unwrap();

        // Two one-term base blocks with only the last affine component set:
        // prefix grouping 1, block clears 10, affine classification 10,
        // dense gcd-slot visits 10, first-present searches 10,
        // normalization 10, common-row comparison 5, final tail scan 4.
        assert_eq!(certificate.stats().recognition_operations(), 60);
        let mut exact_limits = ResidualAffineAtomRowLimits::default();
        exact_limits.max_recognition_operations = 60;
        ResidualAffineAtomRowCertificate::compile(&context, source.clone(), exact_limits).unwrap();
        exact_limits.max_recognition_operations = 59;
        assert!(matches!(
            ResidualAffineAtomRowCertificate::compile(&context, source, exact_limits),
            Err(ResidualAffineAtomRowError::ResourceLimit {
                resource: "recognition operations",
                requested: 60,
                limit: 59,
            })
        ));
    }

    #[test]
    fn rejects_nonlinear_mixed_and_nonassociate_blocks_as_typed_unsupported() {
        let (base, context) = context("affine-atom-unsupported", &["theta"], 2);
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        for source in [
            context.mul(&n0, &n0).unwrap(),
            context.mul(&n0, &n1).unwrap(),
        ] {
            assert!(matches!(
                ResidualAffineAtomRowCertificate::compile(
                    &context,
                    polynomial(&context, &source),
                    ResidualAffineAtomRowLimits::default(),
                ),
                Err(ResidualAffineAtomRowError::Unsupported {
                    reason: ResidualAffineAtomRowUnsupported::NonAffineIndexMonomial { .. }
                })
            ));
        }

        let theta = context.lift(&base.parameter("theta").unwrap()).unwrap();
        let theta_row = context.mul(&theta, &affine(&context, 1, &[1, 0])).unwrap();
        let other_row = affine(&context, 1, &[0, 1]);
        let nonassociate = context.add(&theta_row, &other_row).unwrap();
        assert!(matches!(
            ResidualAffineAtomRowCertificate::compile(
                &context,
                polynomial(&context, &nonassociate),
                ResidualAffineAtomRowLimits::default(),
            ),
            Err(ResidualAffineAtomRowError::Unsupported {
                reason: ResidualAffineAtomRowUnsupported::NonAssociateBaseBlock { .. }
            })
        ));
    }

    #[test]
    fn fresh_memory_census_covers_zero_small_and_late_unsupported_output_phases() {
        let (base, context) = context("affine-atom-fresh-memory-phases", &["theta"], 2);
        let limits = ResidualAffineAtomRowLimits::default();

        let zero_source = polynomial(&context, &context.zero());
        let zero_census =
            residual_affine_atom_row_attempt_logical_memory_census(&context, &zero_source, limits)
                .unwrap();
        let zero_output_floor = size_of::<ResidualAffineAtomRowFreshCompilationAttempt>()
            + context.fingerprint().len()
            + residual_affine_atom_source_dynamic_logical_bytes(&zero_source).unwrap();
        assert!(zero_census.owned_logical_peak_upper_bound() >= zero_output_floor);
        let zero_fresh =
            ResidualAffineAtomRowCertificate::compile_fresh(&context, zero_source.clone(), limits)
                .unwrap();
        assert_eq!(zero_fresh.logical_memory_census(), zero_census);
        match zero_fresh {
            ResidualAffineAtomRowFreshCompilationAttempt::Complete { certificate, .. } => {
                assert_eq!(
                    certificate.outcome(),
                    ResidualAffineAtomRowOutcome::RedundantZeroPolynomial
                );
            }
            ResidualAffineAtomRowFreshCompilationAttempt::Unsupported { .. } => {
                panic!("zero polynomial unexpectedly unsupported")
            }
        }

        let small_source = polynomial(&context, &affine(&context, 1, &[2, -3]));
        let small_census =
            residual_affine_atom_row_attempt_logical_memory_census(&context, &small_source, limits)
                .unwrap();
        let small_fresh =
            ResidualAffineAtomRowCertificate::compile_fresh(&context, small_source.clone(), limits)
                .unwrap();
        assert_eq!(small_fresh.logical_memory_census(), small_census);
        assert!(small_census.owned_logical_peak_upper_bound() > zero_output_floor);
        assert!(matches!(
            small_fresh,
            ResidualAffineAtomRowFreshCompilationAttempt::Complete { .. }
        ));

        let theta = context.lift(&base.parameter("theta").unwrap()).unwrap();
        let theta_squared = context.mul(&theta, &theta).unwrap();
        let common = affine(&context, 1, &[1, 0]);
        let first = common.clone();
        let second = context
            .mul(&theta, &context.mul(&context.integer(2), &common).unwrap())
            .unwrap();
        let third = context
            .mul(&theta_squared, &affine(&context, 1, &[0, 1]))
            .unwrap();
        let late_source = polynomial(
            &context,
            &context
                .add(&context.add(&first, &second).unwrap(), &third)
                .unwrap(),
        );
        let late_census =
            residual_affine_atom_row_attempt_logical_memory_census(&context, &late_source, limits)
                .unwrap();
        let late_fresh =
            ResidualAffineAtomRowCertificate::compile_fresh(&context, late_source.clone(), limits)
                .unwrap();
        assert_eq!(late_fresh.logical_memory_census(), late_census);
        assert!(
            late_census.owned_logical_peak_upper_bound()
                > small_census.owned_logical_peak_upper_bound()
        );
        let fresh_reason = match late_fresh {
            ResidualAffineAtomRowFreshCompilationAttempt::Unsupported { reason, .. } => reason,
            ResidualAffineAtomRowFreshCompilationAttempt::Complete { .. } => {
                panic!("late non-associate fixture unexpectedly completed")
            }
        };
        assert_eq!(
            fresh_reason,
            ResidualAffineAtomRowUnsupported::NonAssociateBaseBlock { block_ordinal: 2 }
        );
        assert!(matches!(
            ResidualAffineAtomRowCertificate::compile(&context, late_source.clone(), limits),
            Err(ResidualAffineAtomRowError::Unsupported { reason }) if reason == fresh_reason
        ));
        // The source retained at the cover seam can cheaply reauthenticate the
        // adjacent scalar after the fresh attempt consumed only its clone.
        assert_eq!(
            residual_affine_atom_row_attempt_logical_memory_census(&context, &late_source, limits)
                .unwrap(),
            late_census
        );
    }

    #[test]
    fn preserves_gmp_coefficients_beyond_i128_without_narrowing() {
        let (_, context) = context("affine-atom-gmp", &[], 2);
        let huge = power_two(&context, 200);
        let huge_plus_one = context.add(&huge, &context.one()).unwrap();
        let row = context
            .add(
                &context
                    .mul(&huge_plus_one, &context.index(0).unwrap())
                    .unwrap(),
                &context.index(1).unwrap(),
            )
            .unwrap();
        let source = polynomial(&context, &context.mul(&huge, &row).unwrap());
        let certificate = ResidualAffineAtomRowCertificate::compile(
            &context,
            source,
            ResidualAffineAtomRowLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            certificate.row().unwrap().coefficient(0),
            Some(Integer::Large(_))
        ));
        assert_eq!(
            certificate.row().unwrap().coefficient(1),
            Some(&Integer::from(1))
        );
        assert!(matches!(
            certificate.block_witnesses()[0].signed_scalar(),
            Integer::Large(_)
        ));
        certificate.replay(&context).unwrap();
    }

    #[test]
    fn fresh_memory_census_bounds_large_quotient_sign_and_exact_one_below_bits() {
        let (_, context) = context("affine-atom-fresh-memory-large-sign", &[], 1);
        let huge = context
            .add(&power_two(&context, 200), &context.one())
            .unwrap();
        let source_value = context
            .add(
                &context.mul(&context.integer(-2), &huge).unwrap(),
                &context
                    .mul(&context.integer(-2), &context.index(0).unwrap())
                    .unwrap(),
            )
            .unwrap();
        let source = polynomial(&context, &source_value);
        assert!(
            source
                .raw()
                .coefficients
                .iter()
                .any(|coefficient| matches!(coefficient, Integer::Large(_)))
        );

        let baseline = residual_affine_atom_row_attempt_logical_memory_census(
            &context,
            &source,
            ResidualAffineAtomRowLimits::default(),
        )
        .unwrap();
        let fresh = ResidualAffineAtomRowCertificate::compile_fresh(
            &context,
            source.clone(),
            ResidualAffineAtomRowLimits::default(),
        )
        .unwrap();
        assert_eq!(fresh.logical_memory_census(), baseline);
        let certificate = match fresh {
            ResidualAffineAtomRowFreshCompilationAttempt::Complete { certificate, .. } => {
                certificate
            }
            ResidualAffineAtomRowFreshCompilationAttempt::Unsupported { .. } => {
                panic!("large signed quotient fixture unexpectedly unsupported")
            }
        };
        assert!(matches!(
            certificate.row().unwrap().constant(),
            Integer::Large(_)
        ));
        assert!(!certificate.row().unwrap().constant().is_negative());

        let exact_bits = source
            .raw()
            .coefficients
            .iter()
            .map(|coefficient| integer_magnitude_bits(coefficient).unwrap())
            .max()
            .unwrap();
        let mut exact = ResidualAffineAtomRowLimits::default();
        exact.max_integer_coefficient_bits = exact_bits;
        assert_eq!(
            residual_affine_atom_row_attempt_logical_memory_census(&context, &source, exact)
                .unwrap(),
            baseline
        );
        ResidualAffineAtomRowCertificate::compile_fresh(&context, source.clone(), exact).unwrap();

        let mut one_below = exact;
        one_below.max_integer_coefficient_bits = exact_bits - 1;
        assert!(matches!(
            ResidualAffineAtomRowCertificate::compile_fresh(&context, source, one_below),
            Err(ResidualAffineAtomRowError::ResourceLimit {
                resource: "integer coefficient bits",
                requested,
                limit,
            }) if requested == exact_bits && limit + 1 == exact_bits
        ));
    }

    #[test]
    fn classifies_zero_and_nonzero_base_constants_without_fabricating_rows() {
        let (base, context) = context("affine-atom-constants", &["theta"], 3);
        let zero = ResidualAffineAtomRowCertificate::compile(
            &context,
            polynomial(&context, &context.zero()),
            ResidualAffineAtomRowLimits::default(),
        )
        .unwrap();
        assert_eq!(
            zero.outcome(),
            ResidualAffineAtomRowOutcome::RedundantZeroPolynomial
        );
        assert!(zero.primitive_row().is_none());
        assert!(zero.block_witnesses().is_empty());

        let theta = context.lift(&base.parameter("theta").unwrap()).unwrap();
        let constant = context
            .add(
                &context.mul(&context.integer(-2), &theta).unwrap(),
                &context.integer(3),
            )
            .unwrap();
        let inconsistent = ResidualAffineAtomRowCertificate::compile(
            &context,
            polynomial(&context, &constant),
            ResidualAffineAtomRowLimits::default(),
        )
        .unwrap();
        assert_eq!(
            inconsistent.outcome(),
            ResidualAffineAtomRowOutcome::InconsistentNonzeroConstant
        );
        assert!(inconsistent.row().is_none());
        assert_eq!(
            inconsistent.primitive_row().unwrap().components(),
            &[
                Integer::from(1),
                Integer::from(0),
                Integer::from(0),
                Integer::from(0)
            ]
        );
        assert_eq!(inconsistent.block_witnesses().len(), 2);
        inconsistent.replay(&context).unwrap();
    }

    #[test]
    fn every_positive_census_limit_fails_one_below() {
        let (base, context) = context("affine-atom-limits", &["theta", "phi"], 2);
        let theta = context.lift(&base.parameter("theta").unwrap()).unwrap();
        let phi = context.lift(&base.parameter("phi").unwrap()).unwrap();
        let factor = context
            .add(
                &context.mul(&context.integer(2), &theta).unwrap(),
                &context.mul(&context.integer(-3), &phi).unwrap(),
            )
            .unwrap();
        let source = polynomial(
            &context,
            &context
                .mul(&factor, &affine(&context, 5, &[7, -11]))
                .unwrap(),
        );
        let certificate = ResidualAffineAtomRowCertificate::compile(
            &context,
            source.clone(),
            ResidualAffineAtomRowLimits::default(),
        )
        .unwrap();
        let stats = certificate.stats();

        macro_rules! one_below {
            ($field:ident, $value:expr) => {{
                let value = $value;
                assert!(value > 0, "{} census must be positive", stringify!($field));
                let mut limits = ResidualAffineAtomRowLimits::default();
                limits.$field = value - 1;
                assert!(
                    matches!(
                        ResidualAffineAtomRowCertificate::compile(&context, source.clone(), limits),
                        Err(ResidualAffineAtomRowError::ResourceLimit { .. })
                    ),
                    "{} did not fail one below",
                    stringify!($field)
                );
            }};
        }
        one_below!(
            max_context_fingerprint_bytes,
            stats.context_fingerprint_bytes()
        );
        one_below!(max_ambient_arity, stats.ambient_arity());
        one_below!(max_base_variables, stats.base_variables());
        one_below!(max_source_variables, stats.source_variables());
        one_below!(max_source_terms, stats.source_terms());
        one_below!(
            max_exponent_entries_inspected,
            stats.exponent_entries_inspected()
        );
        one_below!(max_affine_blocks, stats.affine_blocks());
        one_below!(
            max_retained_block_witnesses,
            stats.retained_block_witnesses()
        );
        one_below!(
            max_retained_block_exponent_entries,
            stats.retained_block_exponent_entries()
        );
        one_below!(
            max_primitive_row_components,
            stats.primitive_row_components()
        );
        one_below!(max_recognition_operations, stats.recognition_operations());
        one_below!(max_gcd_operations, stats.gcd_operations());
        one_below!(
            max_exact_quotient_operations,
            stats.exact_quotient_operations()
        );
        one_below!(
            max_integer_coefficient_bits,
            stats.largest_integer_coefficient_bits()
        );
        one_below!(max_integer_bit_work, stats.integer_bit_work());
    }

    #[test]
    fn replay_detects_payload_tampering_and_wrong_context() {
        let (_, first_context) = context("affine-atom-replay", &[], 2);
        let source = polynomial(&first_context, &affine(&first_context, 1, &[2, 3]));
        let certificate = ResidualAffineAtomRowCertificate::compile(
            &first_context,
            source,
            ResidualAffineAtomRowLimits::default(),
        )
        .unwrap();

        let mut primitive_tamper = certificate.clone();
        primitive_tamper.primitive_row.as_mut().unwrap().components[1] = Integer::from(9);
        assert!(matches!(
            primitive_tamper.replay(&first_context),
            Err(ResidualAffineAtomRowError::ReplayMismatch)
        ));

        let mut witness_tamper = certificate.clone();
        witness_tamper.block_witnesses[0].signed_scalar = Integer::from(7);
        assert!(matches!(
            witness_tamper.replay(&first_context),
            Err(ResidualAffineAtomRowError::ReplayMismatch)
        ));

        let mut source_tamper = certificate.clone();
        source_tamper.source = polynomial(&first_context, &affine(&first_context, 5, &[11, -13]));
        assert!(matches!(
            source_tamper.replay(&first_context),
            Err(ResidualAffineAtomRowError::ReplayMismatch)
        ));

        let mut stats_tamper = certificate.clone();
        stats_tamper.stats.recognition_operations += 1;
        assert!(matches!(
            stats_tamper.replay(&first_context),
            Err(ResidualAffineAtomRowError::ReplayMismatch)
        ));

        let mut schema_tamper = certificate.clone();
        schema_tamper.schema = "rustred-residual-affine-atom-row-tampered";
        assert!(matches!(
            schema_tamper.replay(&first_context),
            Err(ResidualAffineAtomRowError::SchemaMismatch)
        ));

        let (_, other) = context("affine-atom-replay-other", &[], 2);
        assert!(matches!(
            certificate.replay(&other),
            Err(ResidualAffineAtomRowError::WrongContext)
        ));
    }

    #[test]
    fn internal_primitive_constructor_is_bounded_and_checks_canonicality() {
        let row = ResidualAffinePrimitiveRow::try_from_canonical_components_with_limits(
            vec![Integer::from(1), Integer::from(-2), Integer::from(0)],
            3,
            8,
            64,
        )
        .unwrap();
        assert_eq!(row.arity(), 2);
        assert!(matches!(
            ResidualAffinePrimitiveRow::try_from_canonical_components_with_limits(
                vec![Integer::from(2), Integer::from(4)],
                2,
                8,
                64,
            ),
            Err(ResidualAffinePrimitiveRowError::NotPrimitive)
        ));
        assert!(matches!(
            ResidualAffinePrimitiveRow::try_from_canonical_components_with_limits(
                vec![Integer::from(0), Integer::from(-1)],
                2,
                8,
                64,
            ),
            Err(ResidualAffinePrimitiveRowError::SignNotCanonical)
        ));
        assert!(matches!(
            ResidualAffinePrimitiveRow::try_from_canonical_components_with_limits(
                Vec::new(),
                0,
                0,
                0,
            ),
            Err(ResidualAffinePrimitiveRowError::EmptyComponents)
        ));
        assert!(matches!(
            ResidualAffinePrimitiveRow::try_from_canonical_components_with_limits(
                vec![Integer::from(0), Integer::from(0)],
                2,
                1,
                2,
            ),
            Err(ResidualAffinePrimitiveRowError::ZeroRow)
        ));

        let canonical = vec![Integer::from(1), Integer::from(-2), Integer::from(0)];
        assert!(matches!(
            ResidualAffinePrimitiveRow::try_from_canonical_components_with_limits(
                canonical.clone(),
                2,
                2,
                4,
            ),
            Err(ResidualAffinePrimitiveRowError::ResourceLimit {
                resource: "components",
                requested: 3,
                limit: 2,
            })
        ));
        assert!(matches!(
            ResidualAffinePrimitiveRow::try_from_canonical_components_with_limits(
                canonical.clone(),
                3,
                1,
                4,
            ),
            Err(ResidualAffinePrimitiveRowError::ResourceLimit {
                resource: "integer coefficient bits",
                requested: 2,
                limit: 1,
            })
        ));
        assert!(matches!(
            ResidualAffinePrimitiveRow::try_from_canonical_components_with_limits(
                canonical, 3, 2, 3,
            ),
            Err(ResidualAffinePrimitiveRowError::ResourceLimit {
                resource: "integer bit work",
                requested: 4,
                limit: 3,
            })
        ));
    }
}
