//! Generated IBP/LI row-span augmentation by verified whole-row symmetries.
//!
//! LiteRed's `SR` operation is a numeric quotient on concrete integrals.  It
//! is not a symbolic rule `I(n+s) -> I(n+P s)`.  This module implements the
//! distinct, sound symbolic operation which is useful before elimination:
//! generate the canonical parametric IBP/LI identities and transport each
//! *complete identity* through a verified family symmetry.  Coefficients and
//! shifts are transformed together by [`crate::SymbolicSymmetryRowTransportCompiler`].
//!
//! The result is an augmented row span, not a claim that individual terms are
//! symmetry-equivalent.  Every retained transported row owns a replayable
//! whole-row certificate whose source is one freshly generated canonical
//! identity.  Exact deduplication compares complete sparse algebraic content
//! and exceptional polynomial sets; row labels are deliberately irrelevant
//! to that mathematical comparison.

use std::fmt;
use std::sync::Arc;

use crate::exact_identity::{ExactIdentityError, ExactIdentityWriter};
use crate::{
    IntegralFamily, InternalSymmetrySearchCompletion, InternalSymmetrySearchError,
    InternalSymmetrySearchLimits, ParametricCoefficientContext, ParametricIbpConfig,
    ParametricIbpError, ParametricIbpGenerator, ParametricRelation, SectorFoundationError,
    SectorRestrictions, SymbolicSymmetryRowTransportCertificate,
    SymbolicSymmetryRowTransportCompiler, SymbolicSymmetryRowTransportError,
    SymbolicSymmetryRowTransportLimits, VerifiedInternalFamilyPermutationSymmetry,
    discover_bounded_vacuum_internal_symmetries,
};

pub const GENERATED_SYMBOLIC_ROW_SPAN_V1_SCHEMA: &str = "rustred-generated-symbolic-row-span-v1";
// This stable identity is scoped to the V1 construction schemas encoded
// below. In particular, the bounded-search domain fingerprint carries
// `BOUNDED_INTEGER_VACUUM_SYMMETRY_SEARCH_V1_SCHEMA`; changing its current
// Debug-sized admission algorithm requires a construction-schema and durable-
// identity schema bump.
pub(crate) const GENERATED_SYMBOLIC_ROW_SPAN_STABLE_VALUE_IDENTITY_V1_SCHEMA: &str =
    "rustred-generated-symbolic-row-span-stable-value-identity-v1";

/// How verified symmetries are supplied to the row-span compiler.
///
/// `VerifiedInputs` is intentionally not accepted by [`compile`](GeneratedSymbolicRowSpanCompiler::compile):
/// callers must use [`compile_with_verified_symmetries`](GeneratedSymbolicRowSpanCompiler::compile_with_verified_symmetries)
/// and provide the proof-carrying values explicitly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedSymbolicRowSpanStrategy {
    Disabled,
    BoundedVacuumInternal {
        search: InternalSymmetrySearchLimits,
        /// If true, a resource-limited search prefix is rejected.  If false,
        /// every retained symmetry is still individually verified and sound,
        /// but the augmented span is not claimed complete within the search
        /// alphabet.
        require_exhaustive: bool,
    },
    VerifiedInputs,
}

impl GeneratedSymbolicRowSpanStrategy {
    pub const fn is_disabled(self) -> bool {
        matches!(self, Self::Disabled)
    }

    pub const fn uses_verified_inputs(self) -> bool {
        matches!(self, Self::VerifiedInputs)
    }
}

/// Per-row exact arithmetic and aggregate row-span retention bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedSymbolicRowSpanLimits {
    pub transport: SymbolicSymmetryRowTransportLimits,
    pub max_canonical_rows: usize,
    pub max_canonical_terms: usize,
    pub max_verified_symmetries: usize,
    pub max_transport_attempts: usize,
    pub max_augmented_rows: usize,
    pub max_augmented_terms: usize,
    pub max_exact_dedup_comparisons: usize,
    pub max_aggregate_manifest_bytes: usize,
}

impl Default for GeneratedSymbolicRowSpanLimits {
    fn default() -> Self {
        Self {
            transport: SymbolicSymmetryRowTransportLimits::default(),
            max_canonical_rows: 100_000,
            max_canonical_terms: 16_000_000,
            max_verified_symmetries: 1_000_000,
            max_transport_attempts: 10_000_000,
            max_augmented_rows: 10_000_000,
            max_augmented_terms: 64_000_000,
            max_exact_dedup_comparisons: 100_000_000,
            max_aggregate_manifest_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedSymbolicRowSpanConfig {
    pub strategy: GeneratedSymbolicRowSpanStrategy,
    pub limits: GeneratedSymbolicRowSpanLimits,
}

impl Default for GeneratedSymbolicRowSpanConfig {
    fn default() -> Self {
        Self {
            strategy: GeneratedSymbolicRowSpanStrategy::Disabled,
            limits: GeneratedSymbolicRowSpanLimits::default(),
        }
    }
}

/// Provenance of one row in the augmented source basis.
#[derive(Clone, Debug)]
pub enum GeneratedSymbolicRowSpanLineage {
    Canonical {
        canonical_ordinal: usize,
    },
    VerifiedWholeRowSymmetryTransport {
        canonical_ordinal: usize,
        symmetry_ordinal: usize,
        symmetry_permutation: Box<[usize]>,
        transport: Arc<SymbolicSymmetryRowTransportCertificate>,
    },
}

impl GeneratedSymbolicRowSpanLineage {
    pub const fn canonical_ordinal(&self) -> usize {
        match self {
            Self::Canonical { canonical_ordinal }
            | Self::VerifiedWholeRowSymmetryTransport {
                canonical_ordinal, ..
            } => *canonical_ordinal,
        }
    }

    pub const fn symmetry_ordinal(&self) -> Option<usize> {
        match self {
            Self::Canonical { .. } => None,
            Self::VerifiedWholeRowSymmetryTransport {
                symmetry_ordinal, ..
            } => Some(*symmetry_ordinal),
        }
    }

    pub fn symmetry_permutation(&self) -> Option<&[usize]> {
        match self {
            Self::Canonical { .. } => None,
            Self::VerifiedWholeRowSymmetryTransport {
                symmetry_permutation,
                ..
            } => Some(symmetry_permutation),
        }
    }

    pub fn transport(&self) -> Option<&SymbolicSymmetryRowTransportCertificate> {
        match self {
            Self::Canonical { .. } => None,
            Self::VerifiedWholeRowSymmetryTransport { transport, .. } => Some(transport.as_ref()),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedSymbolicRowSpanStats {
    canonical_rows: usize,
    canonical_terms: usize,
    verified_symmetries: usize,
    nonidentity_symmetries: usize,
    transport_attempts: usize,
    retained_transports: usize,
    exact_duplicate_transports: usize,
    augmented_rows: usize,
    augmented_terms: usize,
    exact_dedup_comparisons: usize,
    aggregate_manifest_bytes: usize,
}

impl GeneratedSymbolicRowSpanStats {
    pub const fn canonical_rows(self) -> usize {
        self.canonical_rows
    }
    pub const fn canonical_terms(self) -> usize {
        self.canonical_terms
    }
    pub const fn verified_symmetries(self) -> usize {
        self.verified_symmetries
    }
    pub const fn nonidentity_symmetries(self) -> usize {
        self.nonidentity_symmetries
    }
    pub const fn transport_attempts(self) -> usize {
        self.transport_attempts
    }
    pub const fn retained_transports(self) -> usize {
        self.retained_transports
    }
    pub const fn exact_duplicate_transports(self) -> usize {
        self.exact_duplicate_transports
    }
    pub const fn augmented_rows(self) -> usize {
        self.augmented_rows
    }
    pub const fn augmented_terms(self) -> usize {
        self.augmented_terms
    }
    pub const fn exact_dedup_comparisons(self) -> usize {
        self.exact_dedup_comparisons
    }
    pub const fn aggregate_manifest_bytes(self) -> usize {
        self.aggregate_manifest_bytes
    }
}

/// Replayable generated canonical-plus-transported source basis.
///
/// This certificate is intentionally reusable.  Coverage systems should
/// eventually accept one immutable shared instance (for example behind an
/// `Arc`) and authenticate all candidates against it.  Rebuilding and owning
/// an identical bounded symmetry search and transported basis once per
/// candidate is sound, but scales poorly beyond small sectors.
#[derive(Clone, Debug)]
pub struct GeneratedSymbolicRowSpanCertificate {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    ibp: ParametricIbpConfig,
    config: GeneratedSymbolicRowSpanConfig,
    search_completion: Option<InternalSymmetrySearchCompletion>,
    symmetries: Box<[VerifiedInternalFamilyPermutationSymmetry]>,
    rows: Box<[ParametricRelation]>,
    lineages: Box<[GeneratedSymbolicRowSpanLineage]>,
    stats: GeneratedSymbolicRowSpanStats,
}

impl GeneratedSymbolicRowSpanCertificate {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }
    pub const fn ibp_config(&self) -> ParametricIbpConfig {
        self.ibp
    }
    pub const fn config(&self) -> GeneratedSymbolicRowSpanConfig {
        self.config
    }
    pub const fn search_completion(&self) -> Option<&InternalSymmetrySearchCompletion> {
        self.search_completion.as_ref()
    }
    pub fn symmetries(&self) -> &[VerifiedInternalFamilyPermutationSymmetry] {
        &self.symmetries
    }
    pub fn rows(&self) -> &[ParametricRelation] {
        &self.rows
    }
    pub fn lineages(&self) -> &[GeneratedSymbolicRowSpanLineage] {
        &self.lineages
    }
    pub const fn stats(&self) -> GeneratedSymbolicRowSpanStats {
        self.stats
    }

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedSymbolicRowSpanError> {
        if self.schema != GENERATED_SYMBOLIC_ROW_SPAN_V1_SCHEMA {
            return Err(GeneratedSymbolicRowSpanError::SchemaMismatch);
        }
        if self.family_fingerprint.as_ref() != family.fingerprint() {
            return Err(GeneratedSymbolicRowSpanError::WrongFamily);
        }
        if self.context_fingerprint.as_ref() != context.fingerprint() {
            return Err(GeneratedSymbolicRowSpanError::WrongContext);
        }
        let rebuilt = if self.config.strategy.uses_verified_inputs() {
            GeneratedSymbolicRowSpanCompiler::compile_with_verified_symmetries(
                family,
                context,
                self.ibp,
                &self.symmetries,
                self.config.limits,
            )?
        } else {
            GeneratedSymbolicRowSpanCompiler::compile(family, context, self.ibp, self.config)?
        };
        if self.payload_eq(&rebuilt) {
            Ok(())
        } else {
            Err(GeneratedSymbolicRowSpanError::ReplayMismatch)
        }
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.family_fingerprint == other.family_fingerprint
            && self.context_fingerprint == other.context_fingerprint
            && self.ibp == other.ibp
            && self.config == other.config
            && self.search_completion == other.search_completion
            && self.stats == other.stats
            && self.symmetries.len() == other.symmetries.len()
            && self
                .symmetries
                .iter()
                .zip(&other.symmetries)
                .all(|(left, right)| {
                    left.family_fingerprint() == right.family_fingerprint()
                        && left.restrictions_fingerprint() == right.restrictions_fingerprint()
                        && left.denominator_permutation() == right.denominator_permutation()
                })
            && self.rows.len() == other.rows.len()
            && self
                .rows
                .iter()
                .zip(&other.rows)
                .all(|(left, right)| left.has_identical_guard_provenance(right))
            && self.lineages.len() == other.lineages.len()
            && self
                .lineages
                .iter()
                .zip(&other.lineages)
                .all(lineage_payload_eq)
    }

    /// Write the allocation-independent stable value of this replayable row
    /// span. Process-local `Arc` identity is intentionally excluded; all
    /// schemas, policies, resource envelopes, proofs, rows, and lineage are
    /// represented structurally.
    pub(crate) fn write_stable_value_identity(
        &self,
        writer: &mut ExactIdentityWriter<'_>,
        tag: &str,
    ) -> Result<(), ExactIdentityError> {
        writer.begin_record(tag, 11)?;
        writer.string(
            "identity_schema",
            GENERATED_SYMBOLIC_ROW_SPAN_STABLE_VALUE_IDENTITY_V1_SCHEMA,
        )?;
        writer.string("certificate_schema", self.schema)?;
        writer.string("family_fingerprint", &self.family_fingerprint)?;
        writer.string("context_fingerprint", &self.context_fingerprint)?;
        write_ibp_config_identity(writer, "ibp_config", self.ibp)?;
        write_row_span_config_identity(writer, "config", self.config)?;
        write_search_completion_identity(
            writer,
            "search_completion",
            self.search_completion.as_ref(),
        )?;
        writer.begin_sequence("symmetries", self.symmetries.len())?;
        for (ordinal, symmetry) in self.symmetries.iter().enumerate() {
            writer.begin_record("symmetry", 2)?;
            writer.usize("ordinal", ordinal)?;
            symmetry.write_stable_value_identity(writer, "certificate")?;
            writer.end_record()?;
        }
        writer.end_sequence()?;
        writer.begin_sequence("rows", self.rows.len())?;
        for (ordinal, row) in self.rows.iter().enumerate() {
            writer.begin_record("row", 2)?;
            writer.usize("ordinal", ordinal)?;
            writer.parametric_relation("relation", row)?;
            writer.end_record()?;
        }
        writer.end_sequence()?;
        writer.begin_sequence("lineages", self.lineages.len())?;
        for (ordinal, lineage) in self.lineages.iter().enumerate() {
            write_lineage_identity(writer, ordinal, lineage, &self.rows, &self.symmetries)?;
        }
        writer.end_sequence()?;
        write_row_span_stats_identity(writer, "stats", self.stats)?;
        writer.end_record()
    }
}

pub(crate) fn write_ibp_config_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    config: ParametricIbpConfig,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 1)?;
    write_parametric_arithmetic_limits_identity(
        writer,
        "arithmetic_limits",
        config.arithmetic_limits,
    )?;
    writer.end_record()
}

pub(crate) fn write_row_span_config_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    config: GeneratedSymbolicRowSpanConfig,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 2)?;
    write_row_span_strategy_identity(writer, "strategy", config.strategy)?;
    write_row_span_limits_identity(writer, "limits", config.limits)?;
    writer.end_record()
}

fn write_row_span_strategy_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    strategy: GeneratedSymbolicRowSpanStrategy,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 2)?;
    match strategy {
        GeneratedSymbolicRowSpanStrategy::Disabled => {
            writer.variant("variant", "Disabled")?;
            writer.begin_record("fields", 0)?;
        }
        GeneratedSymbolicRowSpanStrategy::BoundedVacuumInternal {
            search,
            require_exhaustive,
        } => {
            writer.variant("variant", "BoundedVacuumInternal")?;
            writer.begin_record("fields", 2)?;
            write_internal_symmetry_search_limits_identity(writer, "search", search)?;
            writer.boolean("require_exhaustive", require_exhaustive)?;
        }
        GeneratedSymbolicRowSpanStrategy::VerifiedInputs => {
            writer.variant("variant", "VerifiedInputs")?;
            writer.begin_record("fields", 0)?;
        }
    }
    writer.end_record()?;
    writer.end_record()
}

fn write_row_span_limits_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    limits: GeneratedSymbolicRowSpanLimits,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 9)?;
    write_transport_limits_identity(writer, "transport", limits.transport)?;
    writer.usize("max_canonical_rows", limits.max_canonical_rows)?;
    writer.usize("max_canonical_terms", limits.max_canonical_terms)?;
    writer.usize("max_verified_symmetries", limits.max_verified_symmetries)?;
    writer.usize("max_transport_attempts", limits.max_transport_attempts)?;
    writer.usize("max_augmented_rows", limits.max_augmented_rows)?;
    writer.usize("max_augmented_terms", limits.max_augmented_terms)?;
    writer.usize(
        "max_exact_dedup_comparisons",
        limits.max_exact_dedup_comparisons,
    )?;
    writer.usize(
        "max_aggregate_manifest_bytes",
        limits.max_aggregate_manifest_bytes,
    )?;
    writer.end_record()
}

fn write_internal_symmetry_search_limits_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    limits: InternalSymmetrySearchLimits,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 10)?;
    writer.unsigned_u64("coefficient_radius", u64::from(limits.coefficient_radius))?;
    writer.usize("max_loop_map_entries", limits.max_loop_map_entries)?;
    writer.usize("max_enumerated_matrices", limits.max_enumerated_matrices)?;
    writer.usize(
        "max_integer_determinant_operations",
        limits.max_integer_determinant_operations,
    )?;
    writer.usize("max_integer_bits", limits.max_integer_bits)?;
    writer.usize("max_verifier_calls", limits.max_verifier_calls)?;
    writer.usize("max_retained_symmetries", limits.max_retained_symmetries)?;
    writer.usize(
        "max_retained_certificate_entries",
        limits.max_retained_certificate_entries,
    )?;
    writer.usize(
        "max_retained_certificate_bytes",
        limits.max_retained_certificate_bytes,
    )?;
    write_symmetry_verification_limits_identity(writer, "verification", limits.verification)?;
    writer.end_record()
}

fn write_transport_limits_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    limits: SymbolicSymmetryRowTransportLimits,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 8)?;
    write_parametric_arithmetic_limits_identity(writer, "arithmetic", limits.arithmetic)?;
    write_symmetry_verification_limits_identity(writer, "symmetry", limits.symmetry)?;
    writer.usize("max_source_terms", limits.max_source_terms)?;
    writer.usize("max_source_guards", limits.max_source_guards)?;
    writer.usize(
        "max_symmetry_domain_conditions",
        limits.max_symmetry_domain_conditions,
    )?;
    writer.usize("max_output_terms", limits.max_output_terms)?;
    writer.usize("max_output_guards", limits.max_output_guards)?;
    writer.usize("max_manifest_bytes", limits.max_manifest_bytes)?;
    writer.end_record()
}

fn write_parametric_arithmetic_limits_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    limits: crate::ParametricArithmeticLimits,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 6)?;
    write_exact_algebra_limits_identity(writer, "exact_algebra", limits.exact_algebra)?;
    writer.usize("max_source_terms", limits.max_source_terms)?;
    writer.usize("max_output_terms", limits.max_output_terms)?;
    writer.usize(
        "max_specialization_power_operations",
        limits.max_specialization_power_operations,
    )?;
    writer.usize(
        "max_specialization_integer_bits",
        limits.max_specialization_integer_bits,
    )?;
    writer.usize("max_guard_origins", limits.max_guard_origins)?;
    writer.end_record()
}

fn write_exact_algebra_limits_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    limits: crate::ExactAlgebraLimits,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 3)?;
    writer.unsigned_u128("max_exponent", limits.max_exponent)?;
    writer.usize("max_polynomial_terms", limits.max_polynomial_terms)?;
    writer.usize("max_term_operations", limits.max_term_operations)?;
    writer.end_record()
}

fn write_symmetry_verification_limits_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    limits: crate::SymmetryVerificationLimits,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 10)?;
    write_exact_algebra_limits_identity(writer, "exact_algebra", limits.exact_algebra)?;
    writer.usize("max_matrix_entries", limits.max_matrix_entries)?;
    writer.usize("max_exact_operations", limits.max_exact_operations)?;
    writer.usize("max_determinant_states", limits.max_determinant_states)?;
    writer.usize(
        "max_symbolica_single_matrix_entries",
        limits.max_symbolica_single_matrix_entries,
    )?;
    writer.usize(
        "max_symbolica_live_matrix_entries",
        limits.max_symbolica_live_matrix_entries,
    )?;
    writer.usize(
        "max_symbolica_input_retained_bytes",
        limits.max_symbolica_input_retained_bytes,
    )?;
    writer.usize(
        "max_symbolica_output_retained_bytes",
        limits.max_symbolica_output_retained_bytes,
    )?;
    writer.usize("max_guard_polynomials", limits.max_guard_polynomials)?;
    writer.usize("max_guard_origins", limits.max_guard_origins)?;
    writer.end_record()
}

fn write_search_completion_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    completion: Option<&InternalSymmetrySearchCompletion>,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 2)?;
    match completion {
        None => {
            writer.variant("variant", "None")?;
            writer.begin_record("fields", 0)?;
        }
        Some(InternalSymmetrySearchCompletion::ExhaustiveWithinBounds { domain_fingerprint }) => {
            writer.variant("variant", "ExhaustiveWithinBounds")?;
            writer.begin_record("fields", 1)?;
            writer.string("domain_fingerprint", domain_fingerprint)?;
        }
        Some(InternalSymmetrySearchCompletion::ResourceLimited {
            domain_fingerprint,
            resource,
            requested,
            limit,
        }) => {
            writer.variant("variant", "ResourceLimited")?;
            writer.begin_record("fields", 4)?;
            writer.string("domain_fingerprint", domain_fingerprint)?;
            writer.string("resource", resource)?;
            writer.usize("requested", *requested)?;
            writer.usize("limit", *limit)?;
        }
    }
    writer.end_record()?;
    writer.end_record()
}

fn write_lineage_identity(
    writer: &mut ExactIdentityWriter<'_>,
    ordinal: usize,
    lineage: &GeneratedSymbolicRowSpanLineage,
    rows: &[ParametricRelation],
    symmetries: &[VerifiedInternalFamilyPermutationSymmetry],
) -> Result<(), ExactIdentityError> {
    match lineage {
        GeneratedSymbolicRowSpanLineage::Canonical { canonical_ordinal } => {
            writer.begin_record("lineage", 3)?;
            writer.usize("ordinal", ordinal)?;
            writer.variant("variant", "Canonical")?;
            writer.usize("canonical_ordinal", *canonical_ordinal)?;
        }
        GeneratedSymbolicRowSpanLineage::VerifiedWholeRowSymmetryTransport {
            canonical_ordinal,
            symmetry_ordinal,
            symmetry_permutation,
            transport,
        } => {
            let source = rows.get(*canonical_ordinal).ok_or(
                ExactIdentityError::ReferenceBindingMismatch {
                    reference: "row-span canonical transport source",
                    ordinal: *canonical_ordinal,
                },
            )?;
            let transported =
                rows.get(ordinal)
                    .ok_or(ExactIdentityError::ReferenceBindingMismatch {
                        reference: "row-span transported output",
                        ordinal,
                    })?;
            let symmetry = symmetries.get(*symmetry_ordinal).ok_or(
                ExactIdentityError::ReferenceBindingMismatch {
                    reference: "row-span transport symmetry",
                    ordinal: *symmetry_ordinal,
                },
            )?;
            if !transport.source().has_identical_guard_provenance(source) {
                return Err(ExactIdentityError::ReferenceBindingMismatch {
                    reference: "row-span canonical transport source",
                    ordinal: *canonical_ordinal,
                });
            }
            if !transport
                .transported_relation()
                .has_identical_guard_provenance(transported)
            {
                return Err(ExactIdentityError::ReferenceBindingMismatch {
                    reference: "row-span transported output",
                    ordinal,
                });
            }
            if !transport.symmetry().stable_value_eq(symmetry)
                || symmetry_permutation.as_ref() != symmetry.denominator_permutation()
                || transport.symmetry_permutation() != symmetry_permutation.as_ref()
            {
                return Err(ExactIdentityError::ReferenceBindingMismatch {
                    reference: "row-span transport symmetry",
                    ordinal: *symmetry_ordinal,
                });
            }

            writer.begin_record("lineage", 15)?;
            writer.usize("ordinal", ordinal)?;
            writer.variant("variant", "VerifiedWholeRowSymmetryTransport")?;
            writer.usize("canonical_ordinal", *canonical_ordinal)?;
            writer.usize("symmetry_ordinal", *symmetry_ordinal)?;
            writer.begin_sequence("symmetry_permutation", symmetry_permutation.len())?;
            for &position in symmetry_permutation.iter() {
                writer.usize("position", position)?;
            }
            writer.end_sequence()?;
            writer.string("transport_schema", transport.schema())?;
            writer.string("transport_family", transport.family_fingerprint())?;
            writer.string("transport_context", transport.context_fingerprint())?;
            write_relation_reference_identity(
                writer,
                "transport_source_reference",
                "CanonicalRow",
                *canonical_ordinal,
            )?;
            write_symmetry_reference_identity(
                writer,
                "transport_symmetry_reference",
                *symmetry_ordinal,
            )?;
            writer.begin_sequence(
                "transport_symmetry_permutation",
                transport.symmetry_permutation().len(),
            )?;
            for &position in transport.symmetry_permutation() {
                writer.usize("position", position)?;
            }
            writer.end_sequence()?;
            writer.begin_sequence(
                "transport_symmetry_map_guard_polynomials",
                transport.symmetry_map_guard_polynomials().len(),
            )?;
            for polynomial in transport.symmetry_map_guard_polynomials() {
                writer.polynomial("polynomial", polynomial)?;
            }
            writer.end_sequence()?;
            write_relation_reference_identity(
                writer,
                "transported_relation_reference",
                "AugmentedRow",
                ordinal,
            )?;
            write_transport_limits_identity(writer, "transport_limits", transport.limits())?;
            let stats = transport.stats();
            writer.begin_record("transport_stats", 6)?;
            writer.usize("source_terms", stats.source_terms())?;
            writer.usize("source_guards", stats.source_guards())?;
            writer.usize(
                "symmetry_domain_conditions",
                stats.symmetry_domain_conditions(),
            )?;
            writer.usize("output_terms", stats.output_terms())?;
            writer.usize("output_guards", stats.output_guards())?;
            writer.usize("output_manifest_bytes", stats.output_manifest_bytes())?;
            writer.end_record()?;
        }
    }
    writer.end_record()
}

fn write_relation_reference_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    kind: &str,
    ordinal: usize,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 3)?;
    writer.variant("kind", kind)?;
    writer.usize("ordinal", ordinal)?;
    writer.string(
        "relation_schema",
        crate::parametric_relation::PARAMETRIC_RELATION_MANIFEST_V2_SCHEMA,
    )?;
    writer.end_record()
}

fn write_symmetry_reference_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    ordinal: usize,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 3)?;
    writer.variant("kind", "RowSpanSymmetry")?;
    writer.usize("ordinal", ordinal)?;
    writer.string(
        "symmetry_identity_schema",
        crate::symmetry_discovery::INTERNAL_FAMILY_PERMUTATION_SYMMETRY_STABLE_VALUE_IDENTITY_V1_SCHEMA,
    )?;
    writer.end_record()
}

fn write_row_span_stats_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    stats: GeneratedSymbolicRowSpanStats,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 11)?;
    writer.usize("canonical_rows", stats.canonical_rows())?;
    writer.usize("canonical_terms", stats.canonical_terms())?;
    writer.usize("verified_symmetries", stats.verified_symmetries())?;
    writer.usize("nonidentity_symmetries", stats.nonidentity_symmetries())?;
    writer.usize("transport_attempts", stats.transport_attempts())?;
    writer.usize("retained_transports", stats.retained_transports())?;
    writer.usize(
        "exact_duplicate_transports",
        stats.exact_duplicate_transports(),
    )?;
    writer.usize("augmented_rows", stats.augmented_rows())?;
    writer.usize("augmented_terms", stats.augmented_terms())?;
    writer.usize("exact_dedup_comparisons", stats.exact_dedup_comparisons())?;
    writer.usize("aggregate_manifest_bytes", stats.aggregate_manifest_bytes())?;
    writer.end_record()
}

fn lineage_payload_eq(
    (left, right): (
        &GeneratedSymbolicRowSpanLineage,
        &GeneratedSymbolicRowSpanLineage,
    ),
) -> bool {
    match (left, right) {
        (
            GeneratedSymbolicRowSpanLineage::Canonical {
                canonical_ordinal: left,
            },
            GeneratedSymbolicRowSpanLineage::Canonical {
                canonical_ordinal: right,
            },
        ) => left == right,
        (
            GeneratedSymbolicRowSpanLineage::VerifiedWholeRowSymmetryTransport {
                canonical_ordinal: left_canonical,
                symmetry_ordinal: left_symmetry,
                symmetry_permutation: left_permutation,
                transport: left_transport,
            },
            GeneratedSymbolicRowSpanLineage::VerifiedWholeRowSymmetryTransport {
                canonical_ordinal: right_canonical,
                symmetry_ordinal: right_symmetry,
                symmetry_permutation: right_permutation,
                transport: right_transport,
            },
        ) => {
            left_canonical == right_canonical
                && left_symmetry == right_symmetry
                && left_permutation == right_permutation
                && left_transport
                    .source()
                    .has_identical_guard_provenance(right_transport.source())
                && left_transport
                    .transported_relation()
                    .has_identical_guard_provenance(right_transport.transported_relation())
        }
        _ => false,
    }
}

pub struct GeneratedSymbolicRowSpanCompiler;

impl GeneratedSymbolicRowSpanCompiler {
    /// Generate canonical IBP/LI rows and optionally augment them by a bounded
    /// generic vacuum-symmetry search.
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        ibp: ParametricIbpConfig,
        config: GeneratedSymbolicRowSpanConfig,
    ) -> Result<GeneratedSymbolicRowSpanCertificate, GeneratedSymbolicRowSpanError> {
        preflight_family_shape(family, context, config.limits)?;
        match config.strategy {
            GeneratedSymbolicRowSpanStrategy::Disabled => {
                Self::compile_impl(family, context, ibp, config, None, Vec::new())
            }
            GeneratedSymbolicRowSpanStrategy::BoundedVacuumInternal {
                search,
                require_exhaustive,
            } => {
                let restrictions = SectorRestrictions::unrestricted(family.denominator_count())?;
                // The outer retention limit is authoritative for this
                // certificate. Clamp the search itself so a tighter row-span
                // policy cannot first accumulate a larger temporary report.
                let mut effective_search = search;
                effective_search.max_retained_symmetries = effective_search
                    .max_retained_symmetries
                    .min(config.limits.max_verified_symmetries);
                let report = discover_bounded_vacuum_internal_symmetries(
                    family,
                    &restrictions,
                    effective_search,
                )?;
                if require_exhaustive && !report.completion().is_exhaustive_within_bounds() {
                    return Err(GeneratedSymbolicRowSpanError::IncompleteRequiredSearch);
                }
                check_limit(
                    "generated row-span verified symmetries",
                    report.symmetries().len(),
                    config.limits.max_verified_symmetries,
                )?;
                Self::compile_impl(
                    family,
                    context,
                    ibp,
                    config,
                    Some(report.completion().clone()),
                    report.symmetries().to_vec(),
                )
            }
            GeneratedSymbolicRowSpanStrategy::VerifiedInputs => {
                Err(GeneratedSymbolicRowSpanError::MissingVerifiedSymmetryInputs)
            }
        }
    }

    /// Generate canonical IBP/LI rows and augment them with caller-supplied,
    /// replayed family-bound symmetry certificates.
    pub fn compile_with_verified_symmetries(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        ibp: ParametricIbpConfig,
        symmetries: &[VerifiedInternalFamilyPermutationSymmetry],
        limits: GeneratedSymbolicRowSpanLimits,
    ) -> Result<GeneratedSymbolicRowSpanCertificate, GeneratedSymbolicRowSpanError> {
        preflight_family_shape(family, context, limits)?;
        check_limit(
            "generated row-span verified symmetries",
            symmetries.len(),
            limits.max_verified_symmetries,
        )?;
        // Reject foreign, stale, or restriction-inconsistent proof payloads
        // before cloning them into the retained certificate.  Passing the
        // certificate's exact owned restrictions is intentional: replay then
        // checks both the structural restriction value and its stable
        // fingerprint before recompiling the integral permutation.
        for symmetry in symmetries {
            symmetry.replay(family, symmetry.restrictions(), limits.transport.symmetry)?;
        }
        Self::compile_impl(
            family,
            context,
            ibp,
            GeneratedSymbolicRowSpanConfig {
                strategy: GeneratedSymbolicRowSpanStrategy::VerifiedInputs,
                limits,
            },
            None,
            symmetries.to_vec(),
        )
    }

    fn compile_impl(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        ibp: ParametricIbpConfig,
        config: GeneratedSymbolicRowSpanConfig,
        search_completion: Option<InternalSymmetrySearchCompletion>,
        symmetries: Vec<VerifiedInternalFamilyPermutationSymmetry>,
    ) -> Result<GeneratedSymbolicRowSpanCertificate, GeneratedSymbolicRowSpanError> {
        if !family
            .coefficient_context()
            .has_same_variable_map(context.base())
        {
            return Err(GeneratedSymbolicRowSpanError::WrongContext);
        }
        if family.denominator_count() != context.index_count() {
            return Err(GeneratedSymbolicRowSpanError::WrongArity {
                expected: family.denominator_count(),
                actual: context.index_count(),
            });
        }
        check_limit(
            "generated row-span verified symmetries",
            symmetries.len(),
            config.limits.max_verified_symmetries,
        )?;

        let generated =
            ParametricIbpGenerator::try_with_context(family, context.clone(), ibp)?.generate()?;
        let canonical = generated.ibp_li().cloned().collect::<Vec<_>>();
        let expected_canonical = expected_generated_row_count(family)?;
        if canonical.len() != expected_canonical {
            return Err(GeneratedSymbolicRowSpanError::GeneratedRowCountMismatch {
                expected: expected_canonical,
                actual: canonical.len(),
            });
        }
        check_limit(
            "generated row-span canonical rows",
            canonical.len(),
            config.limits.max_canonical_rows,
        )?;
        let canonical_terms = aggregate_terms(
            "generated row-span canonical terms",
            canonical.iter(),
            config.limits.max_canonical_terms,
        )?;

        for symmetry in &symmetries {
            symmetry.replay(
                family,
                symmetry.restrictions(),
                config.limits.transport.symmetry,
            )?;
        }
        let nonidentity = symmetries
            .iter()
            .enumerate()
            .filter(|(_, symmetry)| !is_identity(symmetry.denominator_permutation()))
            .collect::<Vec<_>>();
        let transport_attempts = canonical.len().checked_mul(nonidentity.len()).ok_or(
            GeneratedSymbolicRowSpanError::ResourceCountOverflow {
                resource: "generated row-span transport attempts",
            },
        )?;
        check_limit(
            "generated row-span transport attempts",
            transport_attempts,
            config.limits.max_transport_attempts,
        )?;
        if transport_attempts != 0 {
            // Every attempted transport must be compared with at least the
            // first canonical row.  Fail before constructing a transported
            // relation when even that unavoidable comparison is disallowed.
            check_limit(
                "generated row-span exact dedup comparisons",
                1,
                config.limits.max_exact_dedup_comparisons,
            )?;
        }
        check_limit(
            "generated row-span augmented rows",
            canonical.len(),
            config.limits.max_augmented_rows,
        )?;
        check_limit(
            "generated row-span augmented terms",
            canonical_terms,
            config.limits.max_augmented_terms,
        )?;

        let mut rows = canonical;
        let mut lineages = (0..rows.len())
            .map(
                |canonical_ordinal| GeneratedSymbolicRowSpanLineage::Canonical {
                    canonical_ordinal,
                },
            )
            .collect::<Vec<_>>();
        let mut retained_transports = 0usize;
        let mut duplicate_transports = 0usize;
        let mut comparisons = 0usize;
        let mut augmented_terms = canonical_terms;
        let mut manifest_bytes = 0usize;
        for row in &rows {
            manifest_bytes = checked_add(
                "generated row-span aggregate manifest bytes",
                manifest_bytes,
                row.stable_manifest().len(),
            )?;
            check_limit(
                "generated row-span aggregate manifest bytes",
                manifest_bytes,
                config.limits.max_aggregate_manifest_bytes,
            )?;
        }
        for (symmetry_ordinal, symmetry) in nonidentity {
            // Only the canonical prefix is transported.  Transporting a
            // transported row would add no new group action principle and
            // would obscure the direct generated-source witness.
            let canonical_count = rows.len().min(lineages.len());
            let canonical_count = lineages[..canonical_count]
                .iter()
                .take_while(|lineage| {
                    matches!(lineage, GeneratedSymbolicRowSpanLineage::Canonical { .. })
                })
                .count();
            for canonical_ordinal in 0..canonical_count {
                let transport = SymbolicSymmetryRowTransportCompiler::compile(
                    family,
                    context,
                    &rows[canonical_ordinal],
                    symmetry,
                    config.limits.transport,
                )?;
                let candidate = transport.transported_relation();
                let mut duplicate = false;
                for retained in &rows {
                    comparisons =
                        checked_add("generated row-span exact dedup comparisons", comparisons, 1)?;
                    check_limit(
                        "generated row-span exact dedup comparisons",
                        comparisons,
                        config.limits.max_exact_dedup_comparisons,
                    )?;
                    if same_mathematical_row(retained, candidate) {
                        duplicate = true;
                        break;
                    }
                }
                if duplicate {
                    duplicate_transports = checked_add(
                        "generated row-span exact duplicate transports",
                        duplicate_transports,
                        1,
                    )?;
                    continue;
                }
                let requested_rows =
                    checked_add("generated row-span augmented rows", rows.len(), 1)?;
                check_limit(
                    "generated row-span augmented rows",
                    requested_rows,
                    config.limits.max_augmented_rows,
                )?;
                let transported_terms = checked_add(
                    "generated row-span augmented terms",
                    augmented_terms,
                    candidate.terms().len(),
                )?;
                check_limit(
                    "generated row-span augmented terms",
                    transported_terms,
                    config.limits.max_augmented_terms,
                )?;
                let transported_manifest_bytes = checked_add(
                    "generated row-span aggregate manifest bytes",
                    manifest_bytes,
                    transport.stats().output_manifest_bytes(),
                )?;
                check_limit(
                    "generated row-span aggregate manifest bytes",
                    transported_manifest_bytes,
                    config.limits.max_aggregate_manifest_bytes,
                )?;
                rows.push(candidate.clone());
                lineages.push(
                    GeneratedSymbolicRowSpanLineage::VerifiedWholeRowSymmetryTransport {
                        canonical_ordinal,
                        symmetry_ordinal,
                        symmetry_permutation: symmetry
                            .denominator_permutation()
                            .to_vec()
                            .into_boxed_slice(),
                        transport: Arc::new(transport),
                    },
                );
                retained_transports = checked_add(
                    "generated row-span retained transports",
                    retained_transports,
                    1,
                )?;
                augmented_terms = transported_terms;
                manifest_bytes = transported_manifest_bytes;
            }
        }

        let stats = GeneratedSymbolicRowSpanStats {
            canonical_rows: lineages
                .iter()
                .filter(|lineage| {
                    matches!(lineage, GeneratedSymbolicRowSpanLineage::Canonical { .. })
                })
                .count(),
            canonical_terms,
            verified_symmetries: symmetries.len(),
            nonidentity_symmetries: symmetries
                .iter()
                .filter(|symmetry| !is_identity(symmetry.denominator_permutation()))
                .count(),
            transport_attempts,
            retained_transports,
            exact_duplicate_transports: duplicate_transports,
            augmented_rows: rows.len(),
            augmented_terms,
            exact_dedup_comparisons: comparisons,
            aggregate_manifest_bytes: manifest_bytes,
        };
        Ok(GeneratedSymbolicRowSpanCertificate {
            schema: GENERATED_SYMBOLIC_ROW_SPAN_V1_SCHEMA,
            family_fingerprint: Arc::from(family.fingerprint()),
            context_fingerprint: Arc::from(context.fingerprint()),
            ibp,
            config,
            search_completion,
            symmetries: symmetries.into_boxed_slice(),
            rows: rows.into_boxed_slice(),
            lineages: lineages.into_boxed_slice(),
            stats,
        })
    }
}

fn same_mathematical_row(left: &ParametricRelation, right: &ParametricRelation) -> bool {
    left.family_fingerprint() == right.family_fingerprint()
        && left.context_fingerprint() == right.context_fingerprint()
        && left.arity() == right.arity()
        && left.terms() == right.terms()
        && left.nonzero_conditions() == right.nonzero_conditions()
}

fn preflight_family_shape(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    limits: GeneratedSymbolicRowSpanLimits,
) -> Result<(), GeneratedSymbolicRowSpanError> {
    if !family
        .coefficient_context()
        .has_same_variable_map(context.base())
    {
        return Err(GeneratedSymbolicRowSpanError::WrongContext);
    }
    if family.denominator_count() != context.index_count() {
        return Err(GeneratedSymbolicRowSpanError::WrongArity {
            expected: family.denominator_count(),
            actual: context.index_count(),
        });
    }
    let expected = expected_generated_row_count(family)?;
    check_limit(
        "generated row-span canonical rows",
        expected,
        limits.max_canonical_rows,
    )?;
    check_limit(
        "generated row-span augmented rows",
        expected,
        limits.max_augmented_rows,
    )?;
    if expected != 0 {
        // Each canonical row's stable manifest contains at least its schema.
        check_limit(
            "generated row-span aggregate manifest bytes",
            1,
            limits.max_aggregate_manifest_bytes,
        )?;
    }
    Ok(())
}

fn expected_generated_row_count(
    family: &IntegralFamily,
) -> Result<usize, GeneratedSymbolicRowSpanError> {
    let contractions = family
        .loop_count()
        .checked_add(family.external_count())
        .ok_or(GeneratedSymbolicRowSpanError::ResourceCountOverflow {
            resource: "generated row-span canonical rows",
        })?;
    let ordinary = family.loop_count().checked_mul(contractions).ok_or(
        GeneratedSymbolicRowSpanError::ResourceCountOverflow {
            resource: "generated row-span canonical rows",
        },
    )?;
    let li = family
        .external_count()
        .checked_mul(family.external_count().saturating_sub(1))
        .and_then(|count| count.checked_div(2))
        .ok_or(GeneratedSymbolicRowSpanError::ResourceCountOverflow {
            resource: "generated row-span canonical rows",
        })?;
    checked_add("generated row-span canonical rows", ordinary, li)
}

fn is_identity(permutation: &[usize]) -> bool {
    permutation
        .iter()
        .enumerate()
        .all(|(source, &target)| source == target)
}

fn aggregate_terms<'a>(
    resource: &'static str,
    rows: impl Iterator<Item = &'a ParametricRelation>,
    limit: usize,
) -> Result<usize, GeneratedSymbolicRowSpanError> {
    let mut total = 0usize;
    for row in rows {
        total = checked_add(resource, total, row.terms().len())?;
        check_limit(resource, total, limit)?;
    }
    Ok(total)
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedSymbolicRowSpanError> {
    left.checked_add(right)
        .ok_or(GeneratedSymbolicRowSpanError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedSymbolicRowSpanError> {
    if requested > limit {
        Err(GeneratedSymbolicRowSpanError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedSymbolicRowSpanError {
    SchemaMismatch,
    ReplayMismatch,
    WrongFamily,
    WrongContext,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    GeneratedRowCountMismatch {
        expected: usize,
        actual: usize,
    },
    MissingVerifiedSymmetryInputs,
    IncompleteRequiredSearch,
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    Ibp(ParametricIbpError),
    Search(InternalSymmetrySearchError),
    SymmetryReplay(crate::InternalSymmetryReplayError),
    Transport(SymbolicSymmetryRowTransportError),
    Sector(SectorFoundationError),
}

impl fmt::Display for GeneratedSymbolicRowSpanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("generated symbolic row-span schema mismatch"),
            Self::ReplayMismatch => formatter.write_str("generated symbolic row-span replay mismatch"),
            Self::WrongFamily => formatter.write_str("generated symbolic row-span family mismatch"),
            Self::WrongContext => formatter.write_str("generated symbolic row-span context mismatch"),
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "generated symbolic row-span arity is {actual}, expected {expected}"
            ),
            Self::GeneratedRowCountMismatch { expected, actual } => write!(
                formatter,
                "fresh IBP/LI generation produced {actual} rows, expected {expected}"
            ),
            Self::MissingVerifiedSymmetryInputs => formatter.write_str(
                "verified-input row-span strategy requires explicit verified symmetry certificates",
            ),
            Self::IncompleteRequiredSearch => formatter.write_str(
                "bounded symmetry discovery was resource-limited but exhaustive completion was required",
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::ResourceLimit { resource, requested, limit } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::Ibp(error) => error.fmt(formatter),
            Self::Search(error) => error.fmt(formatter),
            Self::SymmetryReplay(error) => error.fmt(formatter),
            Self::Transport(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedSymbolicRowSpanError {}

impl From<ParametricIbpError> for GeneratedSymbolicRowSpanError {
    fn from(value: ParametricIbpError) -> Self {
        Self::Ibp(value)
    }
}
impl From<InternalSymmetrySearchError> for GeneratedSymbolicRowSpanError {
    fn from(value: InternalSymmetrySearchError) -> Self {
        Self::Search(value)
    }
}
impl From<crate::InternalSymmetryReplayError> for GeneratedSymbolicRowSpanError {
    fn from(value: crate::InternalSymmetryReplayError) -> Self {
        Self::SymmetryReplay(value)
    }
}
impl From<SymbolicSymmetryRowTransportError> for GeneratedSymbolicRowSpanError {
    fn from(value: SymbolicSymmetryRowTransportError) -> Self {
        Self::Transport(value)
    }
}
impl From<SectorFoundationError> for GeneratedSymbolicRowSpanError {
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exact_identity::{
        ExactIdentityLimits, ExactIdentityPayload, ExactStructuralIdentity, encode_exact_identity,
    };
    use crate::{AffineDenominator, CoefficientContext};

    struct RowSpanPayload<'a>(&'a GeneratedSymbolicRowSpanCertificate);

    impl ExactIdentityPayload for RowSpanPayload<'_> {
        const SCHEMA: &'static str = GENERATED_SYMBOLIC_ROW_SPAN_STABLE_VALUE_IDENTITY_V1_SCHEMA;

        fn write_exact_identity(
            &self,
            writer: &mut ExactIdentityWriter<'_>,
        ) -> Result<(), ExactIdentityError> {
            self.0.write_stable_value_identity(writer, "row_span")
        }
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

    fn bounded_certificate(
        name: &str,
        extra_dedup_budget: usize,
    ) -> GeneratedSymbolicRowSpanCertificate {
        let family = equal_mass_sunset(name);
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut config = GeneratedSymbolicRowSpanConfig::default();
        config.strategy = GeneratedSymbolicRowSpanStrategy::BoundedVacuumInternal {
            search: InternalSymmetrySearchLimits::default(),
            require_exhaustive: true,
        };
        config.limits.max_exact_dedup_comparisons += extra_dedup_budget;
        GeneratedSymbolicRowSpanCompiler::compile(
            &family,
            &context,
            ParametricIbpConfig::default(),
            config,
        )
        .unwrap()
    }

    fn identity(
        certificate: &GeneratedSymbolicRowSpanCertificate,
        limits: ExactIdentityLimits,
    ) -> Result<ExactStructuralIdentity, ExactIdentityError> {
        encode_exact_identity(&RowSpanPayload(certificate), limits)
    }

    #[test]
    fn stable_identity_binds_transport_references_without_relation_duplication() {
        let left = bounded_certificate("row-span-stable-value", 0);
        let right = bounded_certificate("row-span-stable-value", 0);
        assert!(left.stats().retained_transports() > 0);
        let left_identity = identity(&left, ExactIdentityLimits::default()).unwrap();
        let right_identity = identity(&right, ExactIdentityLimits::default()).unwrap();
        assert_eq!(left_identity.as_str(), right_identity.as_str());
        assert!(
            left_identity
                .as_str()
                .contains("transport_source_reference")
        );
        assert!(!left_identity.as_str().contains("transport_source="));

        let changed_limits = bounded_certificate("row-span-stable-value", 1);
        assert_ne!(
            left_identity.as_str(),
            identity(&changed_limits, ExactIdentityLimits::default())
                .unwrap()
                .as_str()
        );

        let stats = left_identity.stats();
        let exact = ExactIdentityLimits {
            max_identity_bytes: stats.identity_bytes(),
            max_fields: stats.fields(),
            max_tag_bytes: stats.tag_bytes(),
            max_string_values: stats.string_values(),
            max_string_bytes: stats.string_bytes(),
            max_nesting_depth: stats.maximum_nesting_depth(),
            max_polynomials: stats.polynomials(),
            max_polynomial_variables: stats.polynomial_variables(),
            max_polynomial_terms: stats.polynomial_terms(),
            max_exponent_entries: stats.exponent_entries(),
            max_integers: stats.integers(),
            max_integer_bits: stats.integer_bits(),
        };
        assert_eq!(
            identity(&left, exact).unwrap().as_str(),
            left_identity.as_str()
        );
        let mut one_below = exact;
        one_below.max_fields -= 1;
        assert!(matches!(
            identity(&left, one_below),
            Err(ExactIdentityError::ResourceLimit {
                resource: "exact structural identity fields",
                requested,
                limit,
            }) if requested <= stats.fields() && requested > limit && limit + 1 == stats.fields()
        ));
    }
}
