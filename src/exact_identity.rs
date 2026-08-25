//! Deterministic, exact structural identities for proof-bearing certificates.
//!
//! This module deliberately exposes only typed writes.  Callers cannot make a
//! process-local `Debug` rendering, a Symbolica symbol id, or a probabilistic
//! digest authoritative by accident.  Encoding performs one allocation-free
//! census pass followed by one pass into a single exactly reserved `String`
//! and one allocation-free byte-for-byte replay comparison. Embedded
//! manifests make one semantic-observer traversal per pass; their canonical
//! length prefixes use allocation-free length-only serialization subpasses,
//! followed by no-op-observer emission, so this is not a claim of one physical
//! serialization traversal.

use std::fmt;
use std::fmt::Write as _;
use std::sync::Arc;

use symbolica::prelude::Integer;

use crate::parametric_coefficient::CoefficientPolynomial;
use crate::parametric_relation::{
    ParametricRelation, ParametricRelationV2Observer, write_relation_manifest_v2_observed,
    write_typed_polynomial,
};
use crate::{Coefficient, CoefficientLocation, GuardOrigin, GuardRowId};

pub(crate) const EXACT_STRUCTURAL_IDENTITY_V1_SCHEMA: &str = "rustred-exact-structural-identity-v1";

const IDENTITY_BYTES_RESOURCE: &str = "exact structural identity bytes";
const IDENTITY_FIELDS_RESOURCE: &str = "exact structural identity fields";
const IDENTITY_TAG_BYTES_RESOURCE: &str = "exact structural identity tag bytes";
const IDENTITY_STRING_VALUES_RESOURCE: &str = "exact structural identity string values";
const IDENTITY_STRING_BYTES_RESOURCE: &str = "exact structural identity string bytes";
const IDENTITY_NESTING_DEPTH_RESOURCE: &str = "exact structural identity nesting depth";
const IDENTITY_POLYNOMIALS_RESOURCE: &str = "exact structural identity polynomials";
const IDENTITY_POLYNOMIAL_VARIABLES_RESOURCE: &str =
    "exact structural identity polynomial variables";
const IDENTITY_POLYNOMIAL_TERMS_RESOURCE: &str = "exact structural identity polynomial terms";
const IDENTITY_EXPONENT_ENTRIES_RESOURCE: &str =
    "exact structural identity polynomial exponent entries";
const IDENTITY_INTEGERS_RESOURCE: &str = "exact structural identity integer values";
const IDENTITY_INTEGER_BITS_RESOURCE: &str = "exact structural identity integer bits";
const IDENTITY_ALLOCATION_RESOURCE: &str = "exact structural identity output";

// V5 certificate payloads have shallow, statically known structure.  A fixed
// stack keeps both passes allocation-free without making an attacker-selected
// nesting limit itself allocate memory.
const IMPLEMENTATION_MAX_NESTING_DEPTH: usize = 256;

/// Aggregate limits for one complete structural identity.
///
/// `max_integer_bits` is the exact sum of sign-magnitude bits for arbitrary
/// signed integers and every typed integer nested inside the outer grammar,
/// sparse polynomials, and ParametricRelation V2 fields. This includes
/// preamble, tag, and string-value length prefixes; record and sequence counts;
/// arities, shifts, provenance ordinals, and exponents. Zero costs one bit
/// because its canonical magnitude is one digit. Standalone fixed-width
/// `u128` and self-census numeric values remain bounded by their field count
/// and byte ceiling and are intentionally excluded from the integer census;
/// the self-census record count and component-tag lengths are still included.
/// Every limit is enforced during the allocation-free census traversal.  A
/// rejected payload therefore reports the first exact checked crossing, which
/// can be a lower bound on the complete payload's final census.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactIdentityLimits {
    pub max_identity_bytes: usize,
    pub max_fields: usize,
    pub max_tag_bytes: usize,
    pub max_string_values: usize,
    pub max_string_bytes: usize,
    pub max_nesting_depth: usize,
    pub max_polynomials: usize,
    pub max_polynomial_variables: usize,
    pub max_polynomial_terms: usize,
    pub max_exponent_entries: usize,
    pub max_integers: usize,
    pub max_integer_bits: usize,
}

impl Default for ExactIdentityLimits {
    fn default() -> Self {
        Self {
            max_identity_bytes: portable_default_limit(64u128 * 1024 * 1024 * 1024),
            max_fields: 1_000_000_000,
            max_tag_bytes: portable_default_limit(64u128 * 1024 * 1024 * 1024),
            max_string_values: 1_000_000_000,
            max_string_bytes: portable_default_limit(64u128 * 1024 * 1024 * 1024),
            max_nesting_depth: 64,
            max_polynomials: 1_000_000_000,
            max_polynomial_variables: portable_default_limit(64_000_000_000),
            max_polynomial_terms: portable_default_limit(64_000_000_000),
            max_exponent_entries: portable_default_limit(4_000_000_000_000_000_000),
            max_integers: portable_default_limit(64_000_000_000),
            max_integer_bits: portable_default_limit(4_000_000_000_000_000_000),
        }
    }
}

fn portable_default_limit(preferred: u128) -> usize {
    usize::try_from(preferred).unwrap_or(usize::MAX)
}

/// Exact census of the stream retained by [`encode_exact_identity`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ExactIdentityStats {
    identity_bytes: usize,
    fields: usize,
    tag_bytes: usize,
    string_values: usize,
    string_bytes: usize,
    maximum_nesting_depth: usize,
    polynomials: usize,
    polynomial_variables: usize,
    polynomial_terms: usize,
    exponent_entries: usize,
    integers: usize,
    integer_bits: usize,
}

impl ExactIdentityStats {
    pub(crate) const fn identity_bytes(self) -> usize {
        self.identity_bytes
    }

    pub(crate) const fn fields(self) -> usize {
        self.fields
    }

    pub(crate) const fn tag_bytes(self) -> usize {
        self.tag_bytes
    }

    pub(crate) const fn string_values(self) -> usize {
        self.string_values
    }

    pub(crate) const fn string_bytes(self) -> usize {
        self.string_bytes
    }

    pub(crate) const fn maximum_nesting_depth(self) -> usize {
        self.maximum_nesting_depth
    }

    pub(crate) const fn polynomials(self) -> usize {
        self.polynomials
    }

    pub(crate) const fn polynomial_variables(self) -> usize {
        self.polynomial_variables
    }

    pub(crate) const fn polynomial_terms(self) -> usize {
        self.polynomial_terms
    }

    pub(crate) const fn exponent_entries(self) -> usize {
        self.exponent_entries
    }

    pub(crate) const fn integers(self) -> usize {
        self.integers
    }

    pub(crate) const fn integer_bits(self) -> usize {
        self.integer_bits
    }
}

/// Authoritative exact bytes and the census which admitted them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExactStructuralIdentity {
    bytes: Arc<String>,
    stats: ExactIdentityStats,
}

impl ExactStructuralIdentity {
    pub(crate) fn as_str(&self) -> &str {
        self.bytes.as_str()
    }

    pub(crate) const fn bytes(&self) -> &Arc<String> {
        &self.bytes
    }

    pub(crate) const fn stats(&self) -> ExactIdentityStats {
        self.stats
    }

    pub(crate) fn into_bytes(self) -> Arc<String> {
        self.bytes
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExactIdentityContainerKind {
    Record,
    Sequence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExactIdentityError {
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
    ReferenceBindingMismatch {
        reference: &'static str,
        ordinal: usize,
    },
    ImplementationNestingLimit {
        requested: usize,
        limit: usize,
    },
    RootValueCount {
        actual: usize,
    },
    UnexpectedContainerEnd {
        expected: Option<ExactIdentityContainerKind>,
        actual: ExactIdentityContainerKind,
    },
    ContainerItemCount {
        kind: ExactIdentityContainerKind,
        expected: usize,
        actual: usize,
    },
    UnclosedContainer {
        kind: ExactIdentityContainerKind,
        depth: usize,
    },
    EncodingMismatch {
        expected_bytes: usize,
        actual_bytes: usize,
    },
    EncodingContentMismatch {
        byte_offset: usize,
    },
    CensusMismatch {
        expected: ExactIdentityStats,
        actual: ExactIdentityStats,
    },
}

impl fmt::Display for ExactIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} bytes for {resource}"
            ),
            Self::ReferenceBindingMismatch { reference, ordinal } => write!(
                formatter,
                "exact structural identity {reference} reference at ordinal {ordinal} does not match its owning payload"
            ),
            Self::ImplementationNestingLimit { requested, limit } => write!(
                formatter,
                "exact structural identity nesting depth {requested} exceeds the fixed allocation-free implementation limit {limit}"
            ),
            Self::RootValueCount { actual } => write!(
                formatter,
                "an exact structural identity needs exactly one root value, found {actual}"
            ),
            Self::UnexpectedContainerEnd { expected, actual } => write!(
                formatter,
                "ended {actual:?} while the open exact-identity container was {expected:?}"
            ),
            Self::ContainerItemCount {
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "exact-identity {kind:?} declared {expected} items but encoded {actual}"
            ),
            Self::UnclosedContainer { kind, depth } => write!(
                formatter,
                "exact-identity {kind:?} at nesting depth {depth} was not closed"
            ),
            Self::EncodingMismatch {
                expected_bytes,
                actual_bytes,
            } => write!(
                formatter,
                "exact structural identity counted {expected_bytes} bytes but wrote {actual_bytes}"
            ),
            Self::EncodingContentMismatch { byte_offset } => write!(
                formatter,
                "exact structural identity replay changed at byte offset {byte_offset}"
            ),
            Self::CensusMismatch { expected, actual } => write!(
                formatter,
                "exact structural identity changed between census and output passes: expected {expected:?}, wrote {actual:?}"
            ),
        }
    }
}

impl std::error::Error for ExactIdentityError {}

/// Immutable payload contract for a canonical structural identity.
///
/// Implementations must emit one root value and must be deterministic across
/// two calls on the same borrowed value.  Every collection is represented by
/// `begin_sequence` with its exact item count, and every aggregate is a
/// `begin_record` with its exact field count.
pub(crate) trait ExactIdentityPayload {
    const SCHEMA: &'static str;

    fn write_exact_identity(
        &self,
        writer: &mut ExactIdentityWriter<'_>,
    ) -> Result<(), ExactIdentityError>;
}

/// Build an authoritative identity using an allocation-free exact census, one
/// fallible user-sized reservation for the retained byte buffer, and an
/// allocation-free comparison replay.
///
/// Constructing the final `Arc<String>` also allocates its small ownership
/// header through Rust's ordinary infallible `Arc::new` API; the claim above is
/// specifically about fallible payload-sized buffer reservations.
pub(crate) fn encode_exact_identity<T: ExactIdentityPayload + ?Sized>(
    payload: &T,
    limits: ExactIdentityLimits,
) -> Result<ExactStructuralIdentity, ExactIdentityError> {
    let expected = run_identity_pass(
        payload,
        ByteSink::counter(limits.max_identity_bytes),
        limits,
        None,
    )?;
    let expected_bytes = expected.identity_bytes;

    let mut output = String::new();
    output.try_reserve_exact(expected_bytes).map_err(|_| {
        ExactIdentityError::AllocationFailure {
            resource: IDENTITY_ALLOCATION_RESOURCE,
            requested: expected_bytes,
        }
    })?;

    let actual = run_identity_pass(
        payload,
        ByteSink::output(&mut output, expected_bytes),
        limits,
        Some(expected),
    )?;
    if output.len() != expected_bytes {
        return Err(ExactIdentityError::EncodingMismatch {
            expected_bytes,
            actual_bytes: output.len(),
        });
    }
    if actual != expected {
        return Err(ExactIdentityError::CensusMismatch { expected, actual });
    }

    let replay = run_identity_pass(
        payload,
        ByteSink::comparison(&output),
        limits,
        Some(expected),
    )?;
    if replay != expected {
        return Err(ExactIdentityError::CensusMismatch {
            expected,
            actual: replay,
        });
    }

    Ok(ExactStructuralIdentity {
        bytes: Arc::new(output),
        stats: expected,
    })
}

fn run_identity_pass<T: ExactIdentityPayload + ?Sized>(
    payload: &T,
    sink: ByteSink<'_>,
    limits: ExactIdentityLimits,
    deferred_census: Option<ExactIdentityStats>,
) -> Result<ExactIdentityStats, ExactIdentityError> {
    let mut writer = ExactIdentityWriter::new(sink, limits, deferred_census);
    writer.write_document_preamble(T::SCHEMA)?;
    payload.write_exact_identity(&mut writer)?;
    writer.finish()
}

#[derive(Clone, Copy)]
struct ContainerFrame {
    kind: ExactIdentityContainerKind,
    expected: usize,
    actual: usize,
}

const EMPTY_CONTAINER_FRAME: ContainerFrame = ContainerFrame {
    kind: ExactIdentityContainerKind::Record,
    expected: 0,
    actual: 0,
};

/// Typed canonical event writer used by [`ExactIdentityPayload`].
pub(crate) struct ExactIdentityWriter<'a> {
    sink: ByteSink<'a>,
    limits: ExactIdentityLimits,
    stats: ExactIdentityStats,
    stack: [ContainerFrame; IMPLEMENTATION_MAX_NESTING_DEPTH],
    depth: usize,
    root_values: usize,
    deferred_census: Option<ExactIdentityStats>,
}

impl<'a> ExactIdentityWriter<'a> {
    fn new(
        sink: ByteSink<'a>,
        limits: ExactIdentityLimits,
        deferred_census: Option<ExactIdentityStats>,
    ) -> Self {
        Self {
            sink,
            limits,
            stats: ExactIdentityStats::default(),
            stack: [EMPTY_CONTAINER_FRAME; IMPLEMENTATION_MAX_NESTING_DEPTH],
            depth: 0,
            root_values: 0,
            deferred_census,
        }
    }

    fn write_document_preamble(&mut self, payload_schema: &str) -> Result<(), ExactIdentityError> {
        self.observe_unsigned(EXACT_STRUCTURAL_IDENTITY_V1_SCHEMA.len() as u128)?;
        self.observe_unsigned(payload_schema.len() as u128)?;
        self.observe_string(payload_schema)?;
        self.sink.write_text("D")?;
        self.sink.write_arguments(format_args!(
            "{}:",
            EXACT_STRUCTURAL_IDENTITY_V1_SCHEMA.len()
        ))?;
        self.sink.write_text(EXACT_STRUCTURAL_IDENTITY_V1_SCHEMA)?;
        self.sink.write_text("|schema=")?;
        self.sink
            .write_arguments(format_args!("{}:", payload_schema.len()))?;
        self.sink.write_text(payload_schema)?;
        self.sink.write_text("|payload=")
    }

    /// Begin a tagged aggregate containing exactly `field_count` direct
    /// values.  Nested aggregates count once in their parent.
    pub(crate) fn begin_record(
        &mut self,
        tag: &str,
        field_count: usize,
    ) -> Result<(), ExactIdentityError> {
        self.begin_container(
            ExactIdentityContainerKind::Record,
            tag,
            field_count,
            "R",
            "{",
        )
    }

    pub(crate) fn end_record(&mut self) -> Result<(), ExactIdentityError> {
        self.end_container(ExactIdentityContainerKind::Record, "}")
    }

    /// Begin a tagged ordered sequence with exactly `item_count` direct
    /// values.  Ordering is semantic and is preserved byte-for-byte.
    pub(crate) fn begin_sequence(
        &mut self,
        tag: &str,
        item_count: usize,
    ) -> Result<(), ExactIdentityError> {
        self.begin_container(
            ExactIdentityContainerKind::Sequence,
            tag,
            item_count,
            "Q",
            "[",
        )
    }

    pub(crate) fn end_sequence(&mut self) -> Result<(), ExactIdentityError> {
        self.end_container(ExactIdentityContainerKind::Sequence, "]")
    }

    /// Write a length-delimited UTF-8 field.  Delimiters inside either tag or
    /// value cannot change the parse tree.
    pub(crate) fn string(&mut self, tag: &str, value: &str) -> Result<(), ExactIdentityError> {
        self.observe_leaf(tag)?;
        self.observe_unsigned(value.len() as u128)?;
        self.observe_string(value)?;
        self.write_tag_header("S", tag)?;
        self.sink
            .write_arguments(format_args!("={}:{};", value.len(), value))
    }

    /// Write a stable string-valued enum variant.  This has a distinct type
    /// tag from an arbitrary string field.
    pub(crate) fn variant(&mut self, tag: &str, value: &str) -> Result<(), ExactIdentityError> {
        self.observe_leaf(tag)?;
        self.observe_unsigned(value.len() as u128)?;
        self.observe_string(value)?;
        self.write_tag_header("V", tag)?;
        self.sink
            .write_arguments(format_args!("={}:{};", value.len(), value))
    }

    pub(crate) fn boolean(&mut self, tag: &str, value: bool) -> Result<(), ExactIdentityError> {
        self.observe_leaf(tag)?;
        self.write_tag_header("B", tag)?;
        self.sink.write_text(if value { "=1;" } else { "=0;" })
    }

    /// Write one architecture-neutral unsigned value as exactly 32 uppercase
    /// hexadecimal digits.  Fixed width also permits deferred self-census
    /// fields to occupy identical space in all passes.
    pub(crate) fn u128(&mut self, tag: &str, value: u128) -> Result<(), ExactIdentityError> {
        self.observe_leaf(tag)?;
        self.write_tag_header("U", tag)?;
        self.sink.write_text("=")?;
        write_fixed_u128_hex(&mut self.sink, value)?;
        self.sink.write_text(";")
    }

    /// Write a platform-sized semantic integer in the fixed-width unsigned
    /// grammar while charging its value to the integer census. Raw `u128`
    /// fields remain fixed-width structural values and are not charged.
    pub(crate) fn usize(&mut self, tag: &str, value: usize) -> Result<(), ExactIdentityError> {
        self.observe_leaf(tag)?;
        self.observe_unsigned(value as u128)?;
        self.write_tag_header("U", tag)?;
        self.sink.write_text("=")?;
        write_fixed_u128_hex(&mut self.sink, value as u128)?;
        self.sink.write_text(";")
    }

    /// Persist this document's final byte count without circular sizing.
    ///
    /// The allocation-free census pass emits an all-zero fixed-width value;
    /// output and comparison passes inject the already known final census.
    /// Every pass therefore writes exactly the same number of bytes.
    pub(crate) fn identity_byte_count(&mut self, tag: &str) -> Result<(), ExactIdentityError> {
        let value = self
            .deferred_census
            .map_or(0, |stats| stats.identity_bytes as u128);
        self.observe_leaf(tag)?;
        self.write_tag_header("C", tag)?;
        self.sink.write_text("=")?;
        write_fixed_u128_hex(&mut self.sink, value)?;
        self.sink.write_text(";")
    }

    /// Persist the complete final census as one fixed-width typed field.
    /// This is self-referentially safe for the same reason as
    /// [`Self::identity_byte_count`].
    pub(crate) fn identity_stats(&mut self, tag: &str) -> Result<(), ExactIdentityError> {
        let stats = self.deferred_census.unwrap_or_default();
        self.observe_leaf(tag)?;
        self.observe_unsigned(12)?;
        self.write_tag_header("T", tag)?;
        self.sink.write_text("#12{")?;
        self.write_census_component("identity_bytes", stats.identity_bytes)?;
        self.write_census_component("fields", stats.fields)?;
        self.write_census_component("tag_bytes", stats.tag_bytes)?;
        self.write_census_component("string_values", stats.string_values)?;
        self.write_census_component("string_bytes", stats.string_bytes)?;
        self.write_census_component("maximum_nesting_depth", stats.maximum_nesting_depth)?;
        self.write_census_component("polynomials", stats.polynomials)?;
        self.write_census_component("polynomial_variables", stats.polynomial_variables)?;
        self.write_census_component("polynomial_terms", stats.polynomial_terms)?;
        self.write_census_component("exponent_entries", stats.exponent_entries)?;
        self.write_census_component("integers", stats.integers)?;
        self.write_census_component("integer_bits", stats.integer_bits)?;
        self.sink.write_text("};")
    }

    pub(crate) fn signed_i64(&mut self, tag: &str, value: i64) -> Result<(), ExactIdentityError> {
        self.observe_leaf(tag)?;
        self.observe_integer_bits((i64::BITS - value.unsigned_abs().leading_zeros()) as usize)?;
        self.write_tag_header("I", tag)?;
        self.sink.write_text("=")?;
        write_signed_i64_hex(&mut self.sink, value)?;
        self.sink.write_text(";")
    }

    /// Write an architecture-neutral semantic `u64`, charging its exact
    /// magnitude to the integer census.  This is distinct from [`Self::u128`],
    /// whose fixed-width values are reserved for structural/self-census data.
    pub(crate) fn unsigned_u64(&mut self, tag: &str, value: u64) -> Result<(), ExactIdentityError> {
        self.observe_leaf(tag)?;
        self.observe_unsigned(u128::from(value))?;
        self.write_tag_header("N", tag)?;
        self.sink.write_text("=")?;
        self.sink.write_arguments(format_args!("{value:X}"))?;
        self.sink.write_text(";")
    }

    /// Write an architecture-neutral semantic `u128`, charging its exact
    /// magnitude to the integer census.
    pub(crate) fn unsigned_u128(
        &mut self,
        tag: &str,
        value: u128,
    ) -> Result<(), ExactIdentityError> {
        self.observe_leaf(tag)?;
        self.observe_unsigned(value)?;
        self.write_tag_header("W", tag)?;
        self.sink.write_text("=")?;
        self.sink.write_arguments(format_args!("{value:X}"))?;
        self.sink.write_text(";")
    }

    /// Write one signed semantic `i128` in canonical sign-magnitude form.
    pub(crate) fn signed_i128(&mut self, tag: &str, value: i128) -> Result<(), ExactIdentityError> {
        self.observe_leaf(tag)?;
        self.observe_integer_bits((i128::BITS - value.unsigned_abs().leading_zeros()) as usize)?;
        self.write_tag_header("J", tag)?;
        self.sink.write_text("=")?;
        self.sink.write_text(if value < 0 { "-" } else { "+" })?;
        self.sink
            .write_arguments(format_args!("{:X}", value.unsigned_abs()))?;
        self.sink.write_text(";")
    }

    /// Write a Symbolica arbitrary integer in version-stable signed
    /// hexadecimal form.  No GMP limb width or decimal formatter is part of
    /// the identity.
    pub(crate) fn integer(&mut self, tag: &str, value: &Integer) -> Result<(), ExactIdentityError> {
        self.observe_leaf(tag)?;
        self.observe_integer(value)?;
        self.write_tag_header("Z", tag)?;
        self.sink.write_text("=")?;
        write_signed_integer_hex(&mut self.sink, value)?;
        self.sink.write_text(";")
    }

    /// Write Symbolica's canonical sparse polynomial structure.  Variable
    /// count, term count, every signed coefficient and every exponent are
    /// encoded; raw Symbolica variable ids are intentionally excluded.  The
    /// caller must bind the ordered `K(n)` context in a separate authoritative
    /// string field in the same payload; a polynomial field is not an identity
    /// for its variable map by itself.
    pub(crate) fn polynomial(
        &mut self,
        tag: &str,
        polynomial: &CoefficientPolynomial,
    ) -> Result<(), ExactIdentityError> {
        self.observe_leaf(tag)?;
        self.observe_polynomial(polynomial)?;
        self.write_tag_header("P", tag)?;
        self.sink.write_text("{")?;
        self.sink.write_polynomial(polynomial)?;
        self.sink.write_text("};")
    }

    /// Write a canonical Symbolica rational polynomial without materializing
    /// an expression or textual manifest.
    pub(crate) fn rational_coefficient(
        &mut self,
        tag: &str,
        coefficient: &Coefficient,
    ) -> Result<(), ExactIdentityError> {
        self.begin_record(tag, 2)?;
        self.polynomial("numerator", &coefficient.numerator)?;
        self.polynomial("denominator", &coefficient.denominator)?;
        self.end_record()
    }

    /// Embed a complete canonical ParametricRelation V2 manifest without
    /// allocating its standalone `stable_manifest` String.  The exact same
    /// grammar writer serves both APIs.  Nested coefficient/guard polynomial
    /// structure remains charged to this identity's polynomial and integer
    /// census.
    pub(crate) fn parametric_relation(
        &mut self,
        tag: &str,
        relation: &ParametricRelation,
    ) -> Result<(), ExactIdentityError> {
        self.observe_leaf(tag)?;
        self.write_tag_header("L", tag)?;
        self.sink.write_text("{")?;
        let (result, observer_error) = {
            // Borrow the byte sink and resource census independently so the
            // authoritative V2 grammar emits one semantic-observer traversal.
            // Its length-only serialization subpasses own the one semantic
            // observation; final nested emission uses a no-op observer and
            // allocates no intermediate payload Strings.
            let byte_offset = self.sink.bytes_written();
            let byte_ceiling = self.sink.byte_ceiling();
            let mut observer = ExactIdentityRelationObserver::new(
                &mut self.stats,
                self.limits,
                byte_offset,
                byte_ceiling,
            );
            let result =
                write_relation_manifest_v2_observed(&mut self.sink, relation, &mut observer);
            (result, observer.error)
        };
        if let Some(error) = observer_error {
            return Err(error);
        }
        self.sink.finish_write(result)?;
        self.sink.write_text("};")
    }

    /// Write one flat guard-provenance atom without allocating its standalone
    /// stable string.  The exhaustive typed mirror is deliberately owned by
    /// the exact-identity layer so all proof owners share one injective,
    /// resource-accounted representation.
    pub(crate) fn guard_origin(
        &mut self,
        tag: &str,
        origin: &GuardOrigin,
    ) -> Result<(), ExactIdentityError> {
        self.begin_record(tag, 2)?;
        let (variant, fields) = guard_origin_shape(origin);
        self.variant("variant", variant)?;
        self.begin_record("fields", fields)?;
        match origin {
            GuardOrigin::FamilyInputCoefficientDenominator { location } => {
                self.coefficient_location("location", location)?;
            }
            GuardOrigin::FamilyBasisDeterminantNumerator
            | GuardOrigin::GuardedDivisionDividendDenominator
            | GuardOrigin::GuardedDivisionDivisorDenominator
            | GuardOrigin::GuardedDivisionDivisorNumerator
            | GuardOrigin::ExplicitRelationCondition
            | GuardOrigin::GeneratedAffineSealedCondition
            | GuardOrigin::CoefficientSpecializationDenominator
            | GuardOrigin::CoefficientPartialSpecializationDenominator
            | GuardOrigin::QuotientPivotNumerator
            | GuardOrigin::ExplicitShiftOperatorCondition => {}
            GuardOrigin::PowerShiftSupport { denominator } => {
                self.usize("denominator", *denominator)?;
            }
            GuardOrigin::RelationConditionAttached { row }
            | GuardOrigin::ShiftOperatorConditionAttached { row }
            | GuardOrigin::ShiftOperatorInputTermDenominator { row }
            | GuardOrigin::ShiftOperatorCollectedTermDenominator { row }
            | GuardOrigin::ShiftOperatorFromRelationAdapter { row }
            | GuardOrigin::ShiftOperatorToRelationAdapter { row } => {
                self.write_guard_row_id("row", row)?;
            }
            GuardOrigin::RelationInputTermDenominator { row, shift }
            | GuardOrigin::RelationCollectedTermDenominator { row, shift }
            | GuardOrigin::RelationPartialSpecializationTermDenominator { row, shift } => {
                self.write_guard_row_id("row", row)?;
                self.write_i64_sequence("shift", shift)?;
            }
            GuardOrigin::RelationScaleFactorDenominator {
                target_row,
                source_row,
            } => {
                self.write_guard_row_id("target_row", target_row)?;
                self.write_guard_row_id("source_row", source_row)?;
            }
            GuardOrigin::RelationTranslation {
                source_row,
                target_row,
                offset,
            } => {
                self.write_guard_row_id("source_row", source_row)?;
                self.write_guard_row_id("target_row", target_row)?;
                self.write_i64_sequence("offset", offset)?;
            }
            GuardOrigin::RelationAffineFreeRecentering {
                source_row,
                target_row,
                coefficient_offset,
                key_center,
            } => {
                self.write_guard_row_id("source_row", source_row)?;
                self.write_guard_row_id("target_row", target_row)?;
                self.write_i64_sequence("coefficient_offset", coefficient_offset)?;
                self.write_i64_sequence("key_center", key_center)?;
            }
            GuardOrigin::RelationIndexPermutation {
                source_row,
                target_row,
                source_to_target,
            } => {
                self.write_guard_row_id("source_row", source_row)?;
                self.write_guard_row_id("target_row", target_row)?;
                self.write_usize_sequence("source_to_target", source_to_target)?;
            }
            GuardOrigin::IndexTranslation { offset } => {
                self.write_i64_sequence("offset", offset)?;
            }
            GuardOrigin::IndexPermutation { source_to_target } => {
                self.write_usize_sequence("source_to_target", source_to_target)?;
            }
            GuardOrigin::VerifiedSymmetryMapDomain {
                source_to_target,
                condition_ordinal,
            } => {
                self.write_usize_sequence("source_to_target", source_to_target)?;
                self.usize("condition_ordinal", *condition_ordinal)?;
            }
            GuardOrigin::IndexSpecialization { assignment } => {
                self.write_i64_sequence("assignment", assignment)?;
            }
            GuardOrigin::PartialIndexSpecialization { assignments } => {
                self.begin_sequence("assignments", assignments.len())?;
                for &(position, value) in assignments {
                    self.begin_record("assignment", 2)?;
                    self.usize("position", position)?;
                    self.signed_i64("value", value)?;
                    self.end_record()?;
                }
                self.end_sequence()?;
            }
            GuardOrigin::ResidualUnitAffineIndexSubstitution {
                source_case,
                predicate_ordinal,
                bound_position,
            }
            | GuardOrigin::CoefficientResidualUnitAffineSubstitutionDenominator {
                source_case,
                predicate_ordinal,
                bound_position,
            } => {
                self.unsigned_u64("source_case", *source_case)?;
                self.usize("predicate_ordinal", *predicate_ordinal)?;
                self.usize("bound_position", *bound_position)?;
            }
            GuardOrigin::ResidualAffineBranchNonzeroGuardSubstitution {
                source_case,
                source_work_item_ordinal,
                ready_terminal_ordinal,
                structural_locus_ordinal,
            } => {
                self.unsigned_u64("source_case", *source_case)?;
                self.usize("source_work_item_ordinal", *source_work_item_ordinal)?;
                self.usize("ready_terminal_ordinal", *ready_terminal_ordinal)?;
                self.usize("structural_locus_ordinal", *structural_locus_ordinal)?;
            }
            GuardOrigin::RelationResidualUnitAffineSubstitutionTermDenominator {
                row,
                shift,
                source_case,
                predicate_ordinal,
                bound_position,
            } => {
                self.write_guard_row_id("row", row)?;
                self.write_i64_sequence("shift", shift)?;
                self.unsigned_u64("source_case", *source_case)?;
                self.usize("predicate_ordinal", *predicate_ordinal)?;
                self.usize("bound_position", *bound_position)?;
            }
            GuardOrigin::RelationResidualAffineBranchSubstitutionTermDenominator {
                row,
                shift,
                source_case,
                source_work_item_ordinal,
                ready_terminal_ordinal,
            } => {
                self.write_guard_row_id("row", row)?;
                self.write_i64_sequence("shift", shift)?;
                self.unsigned_u64("source_case", *source_case)?;
                self.usize("source_work_item_ordinal", *source_work_item_ordinal)?;
                self.usize("ready_terminal_ordinal", *ready_terminal_ordinal)?;
            }
            GuardOrigin::RelationResidualUnitAffineSubstitution {
                source_row,
                target_row,
                source_case,
                predicate_ordinal,
                bound_position,
            } => {
                self.write_guard_row_id("source_row", source_row)?;
                self.write_guard_row_id("target_row", target_row)?;
                self.unsigned_u64("source_case", *source_case)?;
                self.usize("predicate_ordinal", *predicate_ordinal)?;
                self.usize("bound_position", *bound_position)?;
            }
            GuardOrigin::RelationResidualAffineBranchSubstitution {
                source_row,
                target_row,
                source_case,
                source_work_item_ordinal,
                ready_terminal_ordinal,
            } => {
                self.write_guard_row_id("source_row", source_row)?;
                self.write_guard_row_id("target_row", target_row)?;
                self.unsigned_u64("source_case", *source_case)?;
                self.usize("source_work_item_ordinal", *source_work_item_ordinal)?;
                self.usize("ready_terminal_ordinal", *ready_terminal_ordinal)?;
            }
            GuardOrigin::ConcreteQuotientEliminationPivotNumerator { pivot } => {
                self.usize("pivot", *pivot)?;
            }
            GuardOrigin::GeneratedAffineGroupRecentering {
                solve_group_ordinal,
                database_epoch,
                event_ordinal,
            } => {
                self.usize("solve_group_ordinal", *solve_group_ordinal)?;
                self.usize("database_epoch", *database_epoch)?;
                self.usize("event_ordinal", *event_ordinal)?;
            }
            GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
                solve_group_ordinal,
                database_epoch,
                event_ordinal,
                operation_ordinal,
                term_ordinal,
                pivot_normalization,
            } => {
                self.usize("solve_group_ordinal", *solve_group_ordinal)?;
                self.usize("database_epoch", *database_epoch)?;
                self.usize("event_ordinal", *event_ordinal)?;
                self.usize("operation_ordinal", *operation_ordinal)?;
                self.usize("term_ordinal", *term_ordinal)?;
                self.boolean("pivot_normalization", *pivot_normalization)?;
            }
        }
        self.end_record()?;
        self.end_record()
    }

    fn write_i64_sequence(&mut self, tag: &str, values: &[i64]) -> Result<(), ExactIdentityError> {
        self.begin_sequence(tag, values.len())?;
        for &value in values {
            self.signed_i64("value", value)?;
        }
        self.end_sequence()
    }

    fn write_usize_sequence(
        &mut self,
        tag: &str,
        values: &[usize],
    ) -> Result<(), ExactIdentityError> {
        self.begin_sequence(tag, values.len())?;
        for &value in values {
            self.usize("value", value)?;
        }
        self.end_sequence()
    }

    fn write_guard_row_id(
        &mut self,
        tag: &str,
        row: &GuardRowId,
    ) -> Result<(), ExactIdentityError> {
        match row {
            GuardRowId::OrdinaryIbp {
                contraction_momentum,
                differentiated_loop,
            } => {
                self.begin_record(tag, 3)?;
                self.variant("variant", "OrdinaryIbp")?;
                self.usize("contraction_momentum", *contraction_momentum)?;
                self.usize("differentiated_loop", *differentiated_loop)?;
            }
            GuardRowId::LorentzInvariance {
                first_external,
                second_external,
            } => {
                self.begin_record(tag, 3)?;
                self.variant("variant", "LorentzInvariance")?;
                self.usize("first_external", *first_external)?;
                self.usize("second_external", *second_external)?;
            }
            GuardRowId::Derived { label } => {
                self.begin_record(tag, 2)?;
                self.variant("variant", "Derived")?;
                self.string("label", label)?;
            }
        }
        self.end_record()
    }

    pub(crate) fn coefficient_location(
        &mut self,
        tag: &str,
        location: &CoefficientLocation,
    ) -> Result<(), ExactIdentityError> {
        match location {
            CoefficientLocation::Dimension => {
                self.begin_record(tag, 1)?;
                self.variant("variant", "Dimension")?;
            }
            CoefficientLocation::DenominatorConstant { denominator } => {
                self.begin_record(tag, 2)?;
                self.variant("variant", "DenominatorConstant")?;
                self.usize("denominator", *denominator)?;
            }
            CoefficientLocation::DenominatorCoefficient {
                denominator,
                coordinate,
            } => {
                self.begin_record(tag, 3)?;
                self.variant("variant", "DenominatorCoefficient")?;
                self.usize("denominator", *denominator)?;
                self.usize("coordinate", *coordinate)?;
            }
            CoefficientLocation::ExternalGram { row, column } => {
                self.begin_record(tag, 3)?;
                self.variant("variant", "ExternalGram")?;
                self.usize("row", *row)?;
                self.usize("column", *column)?;
            }
            CoefficientLocation::PowerShift { denominator } => {
                self.begin_record(tag, 2)?;
                self.variant("variant", "PowerShift")?;
                self.usize("denominator", *denominator)?;
            }
            CoefficientLocation::BasisDeterminantNumerator => {
                self.begin_record(tag, 1)?;
                self.variant("variant", "BasisDeterminantNumerator")?;
            }
        }
        self.end_record()
    }

    fn begin_container(
        &mut self,
        kind: ExactIdentityContainerKind,
        tag: &str,
        expected: usize,
        type_tag: &str,
        delimiter: &str,
    ) -> Result<(), ExactIdentityError> {
        self.observe_leaf(tag)?;
        self.observe_unsigned(expected as u128)?;
        // Every declared direct child necessarily contributes at least one
        // field. Reject an impossible container before caller code enters a
        // potentially attacker-sized child loop.
        checked_limited_add(
            IDENTITY_FIELDS_RESOURCE,
            self.stats.fields,
            expected,
            self.limits.max_fields,
        )?;
        let requested_depth =
            self.depth
                .checked_add(1)
                .ok_or(ExactIdentityError::ResourceCountOverflow {
                    resource: IDENTITY_NESTING_DEPTH_RESOURCE,
                })?;
        if requested_depth > IMPLEMENTATION_MAX_NESTING_DEPTH {
            return Err(ExactIdentityError::ImplementationNestingLimit {
                requested: requested_depth,
                limit: IMPLEMENTATION_MAX_NESTING_DEPTH,
            });
        }
        check_limit(
            IDENTITY_NESTING_DEPTH_RESOURCE,
            requested_depth,
            self.limits.max_nesting_depth,
        )?;
        self.stats.maximum_nesting_depth = self.stats.maximum_nesting_depth.max(requested_depth);

        self.write_tag_header(type_tag, tag)?;
        self.sink
            .write_arguments(format_args!("#{expected}{delimiter}"))?;
        self.stack[self.depth] = ContainerFrame {
            kind,
            expected,
            actual: 0,
        };
        self.depth = requested_depth;
        Ok(())
    }

    fn end_container(
        &mut self,
        actual_kind: ExactIdentityContainerKind,
        delimiter: &str,
    ) -> Result<(), ExactIdentityError> {
        if self.depth == 0 {
            return Err(ExactIdentityError::UnexpectedContainerEnd {
                expected: None,
                actual: actual_kind,
            });
        }
        let frame = self.stack[self.depth - 1];
        if frame.kind != actual_kind {
            return Err(ExactIdentityError::UnexpectedContainerEnd {
                expected: Some(frame.kind),
                actual: actual_kind,
            });
        }
        if frame.actual != frame.expected {
            return Err(ExactIdentityError::ContainerItemCount {
                kind: frame.kind,
                expected: frame.expected,
                actual: frame.actual,
            });
        }
        self.depth -= 1;
        self.sink.write_text(delimiter)
    }

    fn observe_leaf(&mut self, tag: &str) -> Result<(), ExactIdentityError> {
        if self.depth == 0 {
            self.root_values = checked_add(IDENTITY_FIELDS_RESOURCE, self.root_values, 1)?;
            if self.root_values > 1 {
                return Err(ExactIdentityError::RootValueCount {
                    actual: self.root_values,
                });
            }
        } else {
            let frame = &mut self.stack[self.depth - 1];
            let actual =
                frame
                    .actual
                    .checked_add(1)
                    .ok_or(ExactIdentityError::ResourceCountOverflow {
                        resource: IDENTITY_FIELDS_RESOURCE,
                    })?;
            if actual > frame.expected {
                return Err(ExactIdentityError::ContainerItemCount {
                    kind: frame.kind,
                    expected: frame.expected,
                    actual,
                });
            }
            frame.actual = actual;
        }
        let fields = checked_limited_add(
            IDENTITY_FIELDS_RESOURCE,
            self.stats.fields,
            1,
            self.limits.max_fields,
        )?;
        let tag_bytes = checked_limited_add(
            IDENTITY_TAG_BYTES_RESOURCE,
            self.stats.tag_bytes,
            tag.len(),
            self.limits.max_tag_bytes,
        )?;
        self.stats.fields = fields;
        self.stats.tag_bytes = tag_bytes;
        self.observe_unsigned(tag.len() as u128)?;
        Ok(())
    }

    fn observe_string(&mut self, value: &str) -> Result<(), ExactIdentityError> {
        observe_string_bytes(&mut self.stats, self.limits, value.len())
    }

    fn observe_integer(&mut self, value: &Integer) -> Result<(), ExactIdentityError> {
        observe_symbolica_integer(&mut self.stats, self.limits, value)
    }

    fn observe_integer_bits(&mut self, bits: usize) -> Result<(), ExactIdentityError> {
        observe_integer_bits(&mut self.stats, self.limits, bits)
    }

    fn observe_unsigned(&mut self, value: u128) -> Result<(), ExactIdentityError> {
        observe_unsigned(&mut self.stats, self.limits, value)
    }

    fn observe_polynomial(
        &mut self,
        polynomial: &CoefficientPolynomial,
    ) -> Result<(), ExactIdentityError> {
        observe_polynomial_shape(&mut self.stats, self.limits, polynomial)?;
        observe_unsigned(
            &mut self.stats,
            self.limits,
            polynomial.variables.len() as u128,
        )?;
        observe_unsigned(&mut self.stats, self.limits, polynomial.nterms() as u128)?;
        for term in 0..polynomial.nterms() {
            self.observe_integer(&polynomial.coefficients[term])?;
            let exponents = polynomial.exponents(term);
            observe_unsigned(&mut self.stats, self.limits, exponents.len() as u128)?;
            for exponent in exponents {
                observe_unsigned(&mut self.stats, self.limits, u128::from(*exponent))?;
            }
        }
        Ok(())
    }

    fn write_census_component(
        &mut self,
        tag: &str,
        value: usize,
    ) -> Result<(), ExactIdentityError> {
        self.observe_unsigned(tag.len() as u128)?;
        self.sink
            .write_arguments(format_args!("{}:{}=", tag.len(), tag))?;
        write_fixed_u128_hex(&mut self.sink, value as u128)?;
        self.sink.write_text(";")
    }

    fn write_tag_header(&mut self, type_tag: &str, tag: &str) -> Result<(), ExactIdentityError> {
        self.sink.write_text(type_tag)?;
        self.sink
            .write_arguments(format_args!("{}:{}", tag.len(), tag))
    }

    fn finish(mut self) -> Result<ExactIdentityStats, ExactIdentityError> {
        if self.depth != 0 {
            let frame = self.stack[self.depth - 1];
            return Err(ExactIdentityError::UnclosedContainer {
                kind: frame.kind,
                depth: self.depth,
            });
        }
        if self.root_values != 1 {
            return Err(ExactIdentityError::RootValueCount {
                actual: self.root_values,
            });
        }
        self.stats.identity_bytes = self.sink.bytes_written();
        validate_census_limits(self.stats, self.limits)?;
        Ok(self.stats)
    }
}

fn guard_origin_shape(origin: &GuardOrigin) -> (&'static str, usize) {
    match origin {
        GuardOrigin::FamilyInputCoefficientDenominator { location: _ } => {
            ("FamilyInputCoefficientDenominator", 1)
        }
        GuardOrigin::FamilyBasisDeterminantNumerator => ("FamilyBasisDeterminantNumerator", 0),
        GuardOrigin::PowerShiftSupport { denominator: _ } => ("PowerShiftSupport", 1),
        GuardOrigin::GuardedDivisionDividendDenominator => {
            ("GuardedDivisionDividendDenominator", 0)
        }
        GuardOrigin::GuardedDivisionDivisorDenominator => ("GuardedDivisionDivisorDenominator", 0),
        GuardOrigin::GuardedDivisionDivisorNumerator => ("GuardedDivisionDivisorNumerator", 0),
        GuardOrigin::ExplicitRelationCondition => ("ExplicitRelationCondition", 0),
        GuardOrigin::GeneratedAffineSealedCondition => ("GeneratedAffineSealedCondition", 0),
        GuardOrigin::RelationConditionAttached { row: _ } => ("RelationConditionAttached", 1),
        GuardOrigin::RelationInputTermDenominator { row: _, shift: _ } => {
            ("RelationInputTermDenominator", 2)
        }
        GuardOrigin::RelationCollectedTermDenominator { row: _, shift: _ } => {
            ("RelationCollectedTermDenominator", 2)
        }
        GuardOrigin::RelationScaleFactorDenominator {
            target_row: _,
            source_row: _,
        } => ("RelationScaleFactorDenominator", 2),
        GuardOrigin::RelationTranslation {
            source_row: _,
            target_row: _,
            offset: _,
        } => ("RelationTranslation", 3),
        GuardOrigin::RelationAffineFreeRecentering {
            source_row: _,
            target_row: _,
            coefficient_offset: _,
            key_center: _,
        } => ("RelationAffineFreeRecentering", 4),
        GuardOrigin::RelationIndexPermutation {
            source_row: _,
            target_row: _,
            source_to_target: _,
        } => ("RelationIndexPermutation", 3),
        GuardOrigin::IndexTranslation { offset: _ } => ("IndexTranslation", 1),
        GuardOrigin::IndexPermutation {
            source_to_target: _,
        } => ("IndexPermutation", 1),
        GuardOrigin::VerifiedSymmetryMapDomain {
            source_to_target: _,
            condition_ordinal: _,
        } => ("VerifiedSymmetryMapDomain", 2),
        GuardOrigin::IndexSpecialization { assignment: _ } => ("IndexSpecialization", 1),
        GuardOrigin::PartialIndexSpecialization { assignments: _ } => {
            ("PartialIndexSpecialization", 1)
        }
        GuardOrigin::ResidualUnitAffineIndexSubstitution {
            source_case: _,
            predicate_ordinal: _,
            bound_position: _,
        } => ("ResidualUnitAffineIndexSubstitution", 3),
        GuardOrigin::ResidualAffineBranchNonzeroGuardSubstitution {
            source_case: _,
            source_work_item_ordinal: _,
            ready_terminal_ordinal: _,
            structural_locus_ordinal: _,
        } => ("ResidualAffineBranchNonzeroGuardSubstitution", 4),
        GuardOrigin::CoefficientSpecializationDenominator => {
            ("CoefficientSpecializationDenominator", 0)
        }
        GuardOrigin::CoefficientPartialSpecializationDenominator => {
            ("CoefficientPartialSpecializationDenominator", 0)
        }
        GuardOrigin::RelationPartialSpecializationTermDenominator { row: _, shift: _ } => {
            ("RelationPartialSpecializationTermDenominator", 2)
        }
        GuardOrigin::CoefficientResidualUnitAffineSubstitutionDenominator {
            source_case: _,
            predicate_ordinal: _,
            bound_position: _,
        } => ("CoefficientResidualUnitAffineSubstitutionDenominator", 3),
        GuardOrigin::RelationResidualUnitAffineSubstitutionTermDenominator {
            row: _,
            shift: _,
            source_case: _,
            predicate_ordinal: _,
            bound_position: _,
        } => ("RelationResidualUnitAffineSubstitutionTermDenominator", 5),
        GuardOrigin::RelationResidualAffineBranchSubstitutionTermDenominator {
            row: _,
            shift: _,
            source_case: _,
            source_work_item_ordinal: _,
            ready_terminal_ordinal: _,
        } => ("RelationResidualAffineBranchSubstitutionTermDenominator", 5),
        GuardOrigin::RelationResidualUnitAffineSubstitution {
            source_row: _,
            target_row: _,
            source_case: _,
            predicate_ordinal: _,
            bound_position: _,
        } => ("RelationResidualUnitAffineSubstitution", 5),
        GuardOrigin::RelationResidualAffineBranchSubstitution {
            source_row: _,
            target_row: _,
            source_case: _,
            source_work_item_ordinal: _,
            ready_terminal_ordinal: _,
        } => ("RelationResidualAffineBranchSubstitution", 5),
        GuardOrigin::QuotientPivotNumerator => ("QuotientPivotNumerator", 0),
        GuardOrigin::ConcreteQuotientEliminationPivotNumerator { pivot: _ } => {
            ("ConcreteQuotientEliminationPivotNumerator", 1)
        }
        GuardOrigin::ExplicitShiftOperatorCondition => ("ExplicitShiftOperatorCondition", 0),
        GuardOrigin::ShiftOperatorConditionAttached { row: _ } => {
            ("ShiftOperatorConditionAttached", 1)
        }
        GuardOrigin::ShiftOperatorInputTermDenominator { row: _ } => {
            ("ShiftOperatorInputTermDenominator", 1)
        }
        GuardOrigin::ShiftOperatorCollectedTermDenominator { row: _ } => {
            ("ShiftOperatorCollectedTermDenominator", 1)
        }
        GuardOrigin::ShiftOperatorFromRelationAdapter { row: _ } => {
            ("ShiftOperatorFromRelationAdapter", 1)
        }
        GuardOrigin::ShiftOperatorToRelationAdapter { row: _ } => {
            ("ShiftOperatorToRelationAdapter", 1)
        }
        GuardOrigin::GeneratedAffineGroupRecentering {
            solve_group_ordinal: _,
            database_epoch: _,
            event_ordinal: _,
        } => ("GeneratedAffineGroupRecentering", 3),
        GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
            solve_group_ordinal: _,
            database_epoch: _,
            event_ordinal: _,
            operation_ordinal: _,
            term_ordinal: _,
            pivot_normalization: _,
        } => ("GeneratedAffineGroupTopReductionCoefficientDenominator", 6),
    }
}

fn observe_string_bytes(
    stats: &mut ExactIdentityStats,
    limits: ExactIdentityLimits,
    bytes: usize,
) -> Result<(), ExactIdentityError> {
    let string_values = checked_limited_add(
        IDENTITY_STRING_VALUES_RESOURCE,
        stats.string_values,
        1,
        limits.max_string_values,
    )?;
    let string_bytes = checked_limited_add(
        IDENTITY_STRING_BYTES_RESOURCE,
        stats.string_bytes,
        bytes,
        limits.max_string_bytes,
    )?;
    stats.string_values = string_values;
    stats.string_bytes = string_bytes;
    Ok(())
}

fn observe_integer_bits(
    stats: &mut ExactIdentityStats,
    limits: ExactIdentityLimits,
    bits: usize,
) -> Result<(), ExactIdentityError> {
    let integers = checked_limited_add(
        IDENTITY_INTEGERS_RESOURCE,
        stats.integers,
        1,
        limits.max_integers,
    )?;
    let integer_bits = checked_limited_add(
        IDENTITY_INTEGER_BITS_RESOURCE,
        stats.integer_bits,
        bits.max(1),
        limits.max_integer_bits,
    )?;
    stats.integers = integers;
    stats.integer_bits = integer_bits;
    Ok(())
}

fn observe_unsigned(
    stats: &mut ExactIdentityStats,
    limits: ExactIdentityLimits,
    value: u128,
) -> Result<(), ExactIdentityError> {
    let bits = usize::try_from(u128::BITS - value.leading_zeros()).map_err(|_| {
        ExactIdentityError::ResourceCountOverflow {
            resource: IDENTITY_INTEGER_BITS_RESOURCE,
        }
    })?;
    observe_integer_bits(stats, limits, bits)
}

fn observe_signed_i64(
    stats: &mut ExactIdentityStats,
    limits: ExactIdentityLimits,
    value: i64,
) -> Result<(), ExactIdentityError> {
    let bits = usize::try_from(i64::BITS - value.unsigned_abs().leading_zeros()).map_err(|_| {
        ExactIdentityError::ResourceCountOverflow {
            resource: IDENTITY_INTEGER_BITS_RESOURCE,
        }
    })?;
    observe_integer_bits(stats, limits, bits)
}

fn observe_symbolica_integer(
    stats: &mut ExactIdentityStats,
    limits: ExactIdentityLimits,
    value: &Integer,
) -> Result<(), ExactIdentityError> {
    let bits = match value {
        Integer::Single(value) => usize::try_from(i64::BITS - value.unsigned_abs().leading_zeros())
            .map_err(|_| ExactIdentityError::ResourceCountOverflow {
                resource: IDENTITY_INTEGER_BITS_RESOURCE,
            })?,
        Integer::Double(value) => {
            usize::try_from(i128::BITS - value.unsigned_abs().leading_zeros()).map_err(|_| {
                ExactIdentityError::ResourceCountOverflow {
                    resource: IDENTITY_INTEGER_BITS_RESOURCE,
                }
            })?
        }
        Integer::Large(value) => usize::try_from(value.significant_bits()).map_err(|_| {
            ExactIdentityError::ResourceCountOverflow {
                resource: IDENTITY_INTEGER_BITS_RESOURCE,
            }
        })?,
    };
    observe_integer_bits(stats, limits, bits)
}

fn observe_polynomial_shape(
    stats: &mut ExactIdentityStats,
    limits: ExactIdentityLimits,
    polynomial: &CoefficientPolynomial,
) -> Result<(), ExactIdentityError> {
    let polynomials = checked_limited_add(
        IDENTITY_POLYNOMIALS_RESOURCE,
        stats.polynomials,
        1,
        limits.max_polynomials,
    )?;
    let polynomial_variables = checked_limited_add(
        IDENTITY_POLYNOMIAL_VARIABLES_RESOURCE,
        stats.polynomial_variables,
        polynomial.variables.len(),
        limits.max_polynomial_variables,
    )?;
    let polynomial_terms = checked_limited_add(
        IDENTITY_POLYNOMIAL_TERMS_RESOURCE,
        stats.polynomial_terms,
        polynomial.nterms(),
        limits.max_polynomial_terms,
    )?;
    let mut exponent_entries = stats.exponent_entries;
    for term in 0..polynomial.nterms() {
        exponent_entries = checked_limited_add(
            IDENTITY_EXPONENT_ENTRIES_RESOURCE,
            exponent_entries,
            polynomial.exponents(term).len(),
            limits.max_exponent_entries,
        )?;
    }
    stats.polynomials = polynomials;
    stats.polynomial_variables = polynomial_variables;
    stats.polynomial_terms = polynomial_terms;
    stats.exponent_entries = exponent_entries;
    Ok(())
}

struct ExactIdentityRelationObserver<'a> {
    stats: &'a mut ExactIdentityStats,
    limits: ExactIdentityLimits,
    length_prefix_byte_offset: usize,
    identity_byte_ceiling: usize,
    error: Option<ExactIdentityError>,
}

impl<'a> ExactIdentityRelationObserver<'a> {
    fn new(
        stats: &'a mut ExactIdentityStats,
        limits: ExactIdentityLimits,
        length_prefix_byte_offset: usize,
        identity_byte_ceiling: usize,
    ) -> Self {
        Self {
            stats,
            limits,
            length_prefix_byte_offset,
            identity_byte_ceiling,
            error: None,
        }
    }

    fn record(&mut self, result: Result<(), ExactIdentityError>) -> Result<(), fmt::Error> {
        match result {
            Ok(()) => Ok(()),
            Err(error) => {
                if self.error.is_none() {
                    self.error = Some(error);
                }
                Err(fmt::Error)
            }
        }
    }
}

impl ParametricRelationV2Observer for ExactIdentityRelationObserver<'_> {
    fn length_prefix_byte_limit(&self) -> usize {
        self.identity_byte_ceiling
            .saturating_sub(self.length_prefix_byte_offset)
    }

    fn observe_length_prefix_limit_exceeded(
        &mut self,
        local_requested: usize,
        _local_limit: usize,
    ) -> fmt::Result {
        let result = self
            .length_prefix_byte_offset
            .checked_add(local_requested)
            .ok_or(ExactIdentityError::ResourceCountOverflow {
                resource: IDENTITY_BYTES_RESOURCE,
            })
            .and_then(|requested| {
                check_limit(
                    IDENTITY_BYTES_RESOURCE,
                    requested,
                    self.identity_byte_ceiling,
                )
            });
        self.record(result)
    }

    fn observe_text_payload(&mut self, bytes: usize) -> fmt::Result {
        let result = observe_string_bytes(self.stats, self.limits, bytes);
        self.record(result)
    }

    fn observe_unsigned(&mut self, value: u128) -> fmt::Result {
        let result = observe_unsigned(self.stats, self.limits, value);
        self.record(result)
    }

    fn observe_signed_i64(&mut self, value: i64) -> fmt::Result {
        let result = observe_signed_i64(self.stats, self.limits, value);
        self.record(result)
    }

    fn observe_integer(&mut self, value: &Integer) -> fmt::Result {
        let result = observe_symbolica_integer(self.stats, self.limits, value);
        self.record(result)
    }

    fn observe_polynomial(&mut self, polynomial: &CoefficientPolynomial) -> fmt::Result {
        let result = observe_polynomial_shape(self.stats, self.limits, polynomial);
        self.record(result)
    }
}

enum ByteSinkMode<'a> {
    Counter,
    Output(&'a mut String),
    Comparison(&'a str),
}

struct ByteSink<'a> {
    mode: ByteSinkMode<'a>,
    bytes: usize,
    ceiling: usize,
    output_expected_bytes: Option<usize>,
    error: Option<ExactIdentityError>,
}

impl<'a> ByteSink<'a> {
    fn counter(max_bytes: usize) -> Self {
        Self {
            mode: ByteSinkMode::Counter,
            bytes: 0,
            ceiling: max_bytes,
            output_expected_bytes: None,
            error: None,
        }
    }

    fn output(output: &'a mut String, expected_bytes: usize) -> Self {
        Self {
            mode: ByteSinkMode::Output(output),
            bytes: 0,
            ceiling: expected_bytes,
            output_expected_bytes: Some(expected_bytes),
            error: None,
        }
    }

    fn comparison(expected: &'a str) -> Self {
        Self {
            mode: ByteSinkMode::Comparison(expected),
            bytes: 0,
            ceiling: expected.len(),
            output_expected_bytes: Some(expected.len()),
            error: None,
        }
    }

    const fn bytes_written(&self) -> usize {
        self.bytes
    }

    const fn byte_ceiling(&self) -> usize {
        self.ceiling
    }

    fn take_error(&mut self) -> ExactIdentityError {
        self.error
            .take()
            .unwrap_or(ExactIdentityError::ResourceCountOverflow {
                resource: IDENTITY_BYTES_RESOURCE,
            })
    }

    fn finish_write(&mut self, result: fmt::Result) -> Result<(), ExactIdentityError> {
        result.map_err(|_| self.take_error())
    }

    fn write_text(&mut self, value: &str) -> Result<(), ExactIdentityError> {
        let result = self.write_str(value);
        self.finish_write(result)
    }

    fn write_arguments(&mut self, value: fmt::Arguments<'_>) -> Result<(), ExactIdentityError> {
        let result = self.write_fmt(value);
        self.finish_write(result)
    }

    fn write_polynomial(
        &mut self,
        polynomial: &CoefficientPolynomial,
    ) -> Result<(), ExactIdentityError> {
        let result = write_typed_polynomial(self, polynomial);
        self.finish_write(result)
    }
}

impl fmt::Write for ByteSink<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let Some(requested) = self.bytes.checked_add(value.len()) else {
            self.error = Some(ExactIdentityError::ResourceCountOverflow {
                resource: IDENTITY_BYTES_RESOURCE,
            });
            return Err(fmt::Error);
        };
        if requested > self.ceiling {
            self.error = Some(match self.output_expected_bytes {
                Some(expected_bytes) => ExactIdentityError::EncodingMismatch {
                    expected_bytes,
                    actual_bytes: requested,
                },
                None => ExactIdentityError::ResourceLimit {
                    resource: IDENTITY_BYTES_RESOURCE,
                    requested,
                    limit: self.ceiling,
                },
            });
            return Err(fmt::Error);
        }
        match &mut self.mode {
            ByteSinkMode::Counter => {}
            ByteSinkMode::Output(output) => {
                // The complete retained output was reserved before this pass.
                output.push_str(value);
            }
            ByteSinkMode::Comparison(expected) => {
                let expected_slice = &expected.as_bytes()[self.bytes..requested];
                if expected_slice != value.as_bytes() {
                    let local_offset = expected_slice
                        .iter()
                        .zip(value.as_bytes())
                        .position(|(expected, actual)| expected != actual)
                        .unwrap_or(0);
                    self.error = Some(ExactIdentityError::EncodingContentMismatch {
                        byte_offset: self.bytes + local_offset,
                    });
                    return Err(fmt::Error);
                }
            }
        }
        self.bytes = requested;
        Ok(())
    }
}

fn write_fixed_u128_hex(sink: &mut ByteSink<'_>, value: u128) -> Result<(), ExactIdentityError> {
    sink.write_arguments(format_args!("{value:032X}"))
}

fn write_signed_i64_hex(sink: &mut ByteSink<'_>, value: i64) -> Result<(), ExactIdentityError> {
    sink.write_text(if value < 0 { "-" } else { "+" })?;
    sink.write_arguments(format_args!("{:X}", value.unsigned_abs()))
}

fn write_signed_integer_hex(
    sink: &mut ByteSink<'_>,
    value: &Integer,
) -> Result<(), ExactIdentityError> {
    match value {
        Integer::Single(value) => write_signed_i64_hex(sink, *value),
        Integer::Double(value) => {
            sink.write_text(if *value < 0 { "-" } else { "+" })?;
            sink.write_arguments(format_args!("{:X}", value.unsigned_abs()))
        }
        Integer::Large(value) => {
            sink.write_text(if value.is_negative() { "-" } else { "+" })?;
            let limbs = value.as_limbs();
            let Some((most_significant, lower_limbs)) = limbs.split_last() else {
                return sink.write_text("0");
            };
            sink.write_arguments(format_args!("{most_significant:X}"))?;
            let limb_hex_digits = std::mem::size_of_val(most_significant) * 2;
            for limb in lower_limbs.iter().rev() {
                sink.write_arguments(format_args!("{limb:0limb_hex_digits$X}"))?;
            }
            Ok(())
        }
    }
}

fn checked_add(
    resource: &'static str,
    current: usize,
    additional: usize,
) -> Result<usize, ExactIdentityError> {
    current
        .checked_add(additional)
        .ok_or(ExactIdentityError::ResourceCountOverflow { resource })
}

fn checked_limited_add(
    resource: &'static str,
    current: usize,
    additional: usize,
    limit: usize,
) -> Result<usize, ExactIdentityError> {
    let requested = checked_add(resource, current, additional)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn validate_census_limits(
    stats: ExactIdentityStats,
    limits: ExactIdentityLimits,
) -> Result<(), ExactIdentityError> {
    // Observation sites enforce these limits before expensive formatting or
    // payload loops. Keep this final validation as a complete invariant check
    // before the retained-output reservation.
    check_limit(
        IDENTITY_BYTES_RESOURCE,
        stats.identity_bytes,
        limits.max_identity_bytes,
    )?;
    check_limit(IDENTITY_FIELDS_RESOURCE, stats.fields, limits.max_fields)?;
    check_limit(
        IDENTITY_TAG_BYTES_RESOURCE,
        stats.tag_bytes,
        limits.max_tag_bytes,
    )?;
    check_limit(
        IDENTITY_STRING_VALUES_RESOURCE,
        stats.string_values,
        limits.max_string_values,
    )?;
    check_limit(
        IDENTITY_STRING_BYTES_RESOURCE,
        stats.string_bytes,
        limits.max_string_bytes,
    )?;
    check_limit(
        IDENTITY_NESTING_DEPTH_RESOURCE,
        stats.maximum_nesting_depth,
        limits.max_nesting_depth,
    )?;
    check_limit(
        IDENTITY_POLYNOMIALS_RESOURCE,
        stats.polynomials,
        limits.max_polynomials,
    )?;
    check_limit(
        IDENTITY_POLYNOMIAL_VARIABLES_RESOURCE,
        stats.polynomial_variables,
        limits.max_polynomial_variables,
    )?;
    check_limit(
        IDENTITY_POLYNOMIAL_TERMS_RESOURCE,
        stats.polynomial_terms,
        limits.max_polynomial_terms,
    )?;
    check_limit(
        IDENTITY_EXPONENT_ENTRIES_RESOURCE,
        stats.exponent_entries,
        limits.max_exponent_entries,
    )?;
    check_limit(
        IDENTITY_INTEGERS_RESOURCE,
        stats.integers,
        limits.max_integers,
    )?;
    check_limit(
        IDENTITY_INTEGER_BITS_RESOURCE,
        stats.integer_bits,
        limits.max_integer_bits,
    )
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ExactIdentityError> {
    if requested > limit {
        Err(ExactIdentityError::ResourceLimit {
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
    use std::cell::Cell;
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use super::*;
    use crate::{
        CoefficientContext, CoefficientLocation, GuardOrigin, GuardRowId, IndexSpace,
        ParametricCoefficientContext, ParametricRelation, ParametricRowId,
    };

    #[derive(Clone)]
    struct SamplePayload {
        family: String,
        ordinal: usize,
        enabled: bool,
        shift: i64,
        huge: Integer,
        polynomial: CoefficientPolynomial,
        labels: [String; 2],
    }

    impl ExactIdentityPayload for SamplePayload {
        const SCHEMA: &'static str = "rustred-test-exact-identity-payload-v1";

        fn write_exact_identity(
            &self,
            writer: &mut ExactIdentityWriter<'_>,
        ) -> Result<(), ExactIdentityError> {
            writer.begin_record("certificate", 7)?;
            writer.string("family", &self.family)?;
            writer.usize("ordinal", self.ordinal)?;
            writer.boolean("enabled", self.enabled)?;
            writer.signed_i64("shift", self.shift)?;
            writer.integer("huge", &self.huge)?;
            writer.polynomial("locus", &self.polynomial)?;
            writer.begin_sequence("labels", self.labels.len())?;
            for (ordinal, label) in self.labels.iter().enumerate() {
                writer.begin_record("label", 2)?;
                writer.usize("ordinal", ordinal)?;
                writer.variant("value", label)?;
                writer.end_record()?;
            }
            writer.end_sequence()?;
            writer.end_record()
        }
    }

    fn sample_payload() -> SamplePayload {
        let context = CoefficientContext::new(["x", "y"]);
        let polynomial = context.parse("3*x^2*y-5*y+7").unwrap().numerator;
        SamplePayload {
            family: "family|with:length:delimiters;[]{}".to_owned(),
            ordinal: 17,
            enabled: true,
            shift: i64::MIN,
            huge: -((Integer::from(1) << 257u32) + Integer::from(0xABCDu64)),
            polynomial,
            labels: ["left:value".to_owned(), "right|value".to_owned()],
        }
    }

    struct UsizeCensusPayload {
        value: usize,
        semantic: bool,
    }

    impl ExactIdentityPayload for UsizeCensusPayload {
        const SCHEMA: &'static str = "rustred-test-usize-census-v1";

        fn write_exact_identity(
            &self,
            writer: &mut ExactIdentityWriter<'_>,
        ) -> Result<(), ExactIdentityError> {
            if self.semantic {
                writer.usize("value", self.value)
            } else {
                writer.u128("value", self.value as u128)
            }
        }
    }

    #[test]
    fn usize_is_censused_semantically_without_changing_fixed_width_encoding() {
        let limits = ExactIdentityLimits::default();
        let raw_zero = encode_exact_identity(
            &UsizeCensusPayload {
                value: 0,
                semantic: false,
            },
            limits,
        )
        .unwrap();
        let usize_zero = encode_exact_identity(
            &UsizeCensusPayload {
                value: 0,
                semantic: true,
            },
            limits,
        )
        .unwrap();
        let raw_max = encode_exact_identity(
            &UsizeCensusPayload {
                value: usize::MAX,
                semantic: false,
            },
            limits,
        )
        .unwrap();
        let usize_max = encode_exact_identity(
            &UsizeCensusPayload {
                value: usize::MAX,
                semantic: true,
            },
            limits,
        )
        .unwrap();

        assert_eq!(raw_zero.as_str(), usize_zero.as_str());
        assert_eq!(raw_max.as_str(), usize_max.as_str());
        assert_eq!(usize_zero.bytes().len(), usize_max.bytes().len());
        assert_eq!(
            usize_zero.stats().integers(),
            raw_zero.stats().integers() + 1
        );
        assert_eq!(usize_max.stats().integers(), raw_max.stats().integers() + 1);
        assert_eq!(usize_zero.stats().integers(), usize_max.stats().integers());
        assert_eq!(
            usize_zero.stats().integer_bits(),
            raw_zero.stats().integer_bits() + 1
        );
        assert_eq!(
            usize_max.stats().integer_bits(),
            raw_max.stats().integer_bits() + usize::BITS as usize
        );
        assert!(usize_max.stats().integer_bits() > usize_zero.stats().integer_bits());
    }

    fn exact_limits(stats: ExactIdentityStats) -> ExactIdentityLimits {
        ExactIdentityLimits {
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
        }
    }

    fn assert_one_below<T: ExactIdentityPayload + ?Sized>(
        payload: &T,
        limits: ExactIdentityLimits,
        resource: &'static str,
        complete_requested: usize,
    ) {
        assert!(matches!(
            encode_exact_identity(payload, limits),
            Err(ExactIdentityError::ResourceLimit {
                resource: actual_resource,
                requested,
                limit,
            }) if actual_resource == resource
                && requested > limit
                && requested <= complete_requested
                && limit + 1 == complete_requested
        ));
    }

    #[test]
    fn exact_census_and_every_one_below_limit_are_enforced() {
        let payload = sample_payload();
        let baseline = encode_exact_identity(&payload, ExactIdentityLimits::default()).unwrap();
        let exact = exact_limits(baseline.stats());
        let exact_identity = encode_exact_identity(&payload, exact).unwrap();
        assert_eq!(exact_identity.as_str(), baseline.as_str());
        assert_eq!(exact_identity.stats(), baseline.stats());
        assert_eq!(
            exact_identity.bytes().len(),
            baseline.stats().identity_bytes()
        );

        let mut one_below = exact;
        one_below.max_identity_bytes -= 1;
        assert!(matches!(
            encode_exact_identity(&payload, one_below),
            Err(ExactIdentityError::ResourceLimit {
                resource: IDENTITY_BYTES_RESOURCE,
                requested,
                limit,
            }) if requested > limit
                && requested <= baseline.stats().identity_bytes()
                && limit + 1 == baseline.stats().identity_bytes()
        ));

        let mut one_below = exact;
        one_below.max_fields -= 1;
        assert_one_below(
            &payload,
            one_below,
            IDENTITY_FIELDS_RESOURCE,
            baseline.stats().fields(),
        );

        let mut one_below = exact;
        one_below.max_tag_bytes -= 1;
        assert_one_below(
            &payload,
            one_below,
            IDENTITY_TAG_BYTES_RESOURCE,
            baseline.stats().tag_bytes(),
        );

        let mut one_below = exact;
        one_below.max_string_values -= 1;
        assert_one_below(
            &payload,
            one_below,
            IDENTITY_STRING_VALUES_RESOURCE,
            baseline.stats().string_values(),
        );

        let mut one_below = exact;
        one_below.max_string_bytes -= 1;
        assert_one_below(
            &payload,
            one_below,
            IDENTITY_STRING_BYTES_RESOURCE,
            baseline.stats().string_bytes(),
        );

        let mut one_below = exact;
        one_below.max_nesting_depth -= 1;
        assert_one_below(
            &payload,
            one_below,
            IDENTITY_NESTING_DEPTH_RESOURCE,
            baseline.stats().maximum_nesting_depth(),
        );

        let mut one_below = exact;
        one_below.max_polynomials -= 1;
        assert_one_below(
            &payload,
            one_below,
            IDENTITY_POLYNOMIALS_RESOURCE,
            baseline.stats().polynomials(),
        );

        let mut one_below = exact;
        one_below.max_polynomial_variables -= 1;
        assert_one_below(
            &payload,
            one_below,
            IDENTITY_POLYNOMIAL_VARIABLES_RESOURCE,
            baseline.stats().polynomial_variables(),
        );

        let mut one_below = exact;
        one_below.max_polynomial_terms -= 1;
        assert_one_below(
            &payload,
            one_below,
            IDENTITY_POLYNOMIAL_TERMS_RESOURCE,
            baseline.stats().polynomial_terms(),
        );

        let mut one_below = exact;
        one_below.max_exponent_entries -= 1;
        assert_one_below(
            &payload,
            one_below,
            IDENTITY_EXPONENT_ENTRIES_RESOURCE,
            baseline.stats().exponent_entries(),
        );

        let mut one_below = exact;
        one_below.max_integers -= 1;
        assert_one_below(
            &payload,
            one_below,
            IDENTITY_INTEGERS_RESOURCE,
            baseline.stats().integers(),
        );

        let mut one_below = exact;
        one_below.max_integer_bits -= 1;
        assert_one_below(
            &payload,
            one_below,
            IDENTITY_INTEGER_BITS_RESOURCE,
            baseline.stats().integer_bits(),
        );
    }

    struct DeclaredLoopPayload<'a> {
        iterations: &'a Cell<usize>,
        items: usize,
    }

    impl ExactIdentityPayload for DeclaredLoopPayload<'_> {
        const SCHEMA: &'static str = "rustred-test-declared-loop-payload-v1";

        fn write_exact_identity(
            &self,
            writer: &mut ExactIdentityWriter<'_>,
        ) -> Result<(), ExactIdentityError> {
            writer.begin_sequence("items", self.items)?;
            for ordinal in 0..self.items {
                self.iterations.set(ordinal + 1);
                writer.boolean("item", true)?;
            }
            writer.end_sequence()
        }
    }

    #[test]
    fn declared_child_count_is_rejected_before_the_payload_loop() {
        let iterations = Cell::new(0);
        let payload = DeclaredLoopPayload {
            iterations: &iterations,
            items: 1_000_000,
        };
        let mut limits = ExactIdentityLimits::default();
        limits.max_fields = 1;
        assert!(matches!(
            encode_exact_identity(&payload, limits),
            Err(ExactIdentityError::ResourceLimit {
                resource: IDENTITY_FIELDS_RESOURCE,
                requested: 1_000_001,
                limit: 1,
            })
        ));
        assert_eq!(iterations.get(), 0);
    }

    struct PreambleLimitPayload<'a> {
        entered: &'a Cell<bool>,
    }

    impl ExactIdentityPayload for PreambleLimitPayload<'_> {
        const SCHEMA: &'static str = "rustred-test-preamble-limit-payload-v1";

        fn write_exact_identity(
            &self,
            writer: &mut ExactIdentityWriter<'_>,
        ) -> Result<(), ExactIdentityError> {
            self.entered.set(true);
            writer.boolean("unreachable", true)
        }
    }

    #[test]
    fn first_pass_byte_ceiling_rejects_before_entering_the_payload() {
        let entered = Cell::new(false);
        let payload = PreambleLimitPayload { entered: &entered };
        let mut limits = ExactIdentityLimits::default();
        limits.max_identity_bytes = 0;
        assert!(matches!(
            encode_exact_identity(&payload, limits),
            Err(ExactIdentityError::ResourceLimit {
                resource: IDENTITY_BYTES_RESOURCE,
                requested,
                limit: 0,
            }) if requested > 0
        ));
        assert!(!entered.get());
    }

    #[test]
    fn exact_identity_is_deterministic_and_structurally_injective() {
        let payload = sample_payload();
        let identity = encode_exact_identity(&payload, ExactIdentityLimits::default()).unwrap();
        let independent =
            encode_exact_identity(&sample_payload(), ExactIdentityLimits::default()).unwrap();
        assert_eq!(identity.as_str(), independent.as_str());
        assert!(
            identity
                .as_str()
                .starts_with("D36:rustred-exact-structural-identity-v1")
        );
        assert!(
            identity
                .as_str()
                .contains("=-2000000000000000000000000000000000000000000000000000000000000ABCD;")
        );

        let mut changed = payload.clone();
        changed.family.push('|');
        assert_ne!(
            identity.as_str(),
            encode_exact_identity(&changed, ExactIdentityLimits::default())
                .unwrap()
                .as_str()
        );

        let mut changed = payload.clone();
        changed.labels.swap(0, 1);
        assert_ne!(
            identity.as_str(),
            encode_exact_identity(&changed, ExactIdentityLimits::default())
                .unwrap()
                .as_str()
        );

        let context = CoefficientContext::new(["x", "y"]);
        let mut changed = payload.clone();
        changed.polynomial = context.parse("3*x*y-5*y+7").unwrap().numerator;
        assert_ne!(
            identity.as_str(),
            encode_exact_identity(&changed, ExactIdentityLimits::default())
                .unwrap()
                .as_str()
        );

        let mut changed = payload;
        changed.huge = -changed.huge;
        assert_ne!(
            identity.as_str(),
            encode_exact_identity(&changed, ExactIdentityLimits::default())
                .unwrap()
                .as_str()
        );
    }

    struct NonDeterministicPayload {
        calls: Cell<usize>,
    }

    impl ExactIdentityPayload for NonDeterministicPayload {
        const SCHEMA: &'static str = "rustred-test-nondeterministic-exact-identity-v1";

        fn write_exact_identity(
            &self,
            writer: &mut ExactIdentityWriter<'_>,
        ) -> Result<(), ExactIdentityError> {
            let call = self.calls.get();
            self.calls.set(call + 1);
            writer.begin_record("root", 1)?;
            writer.string("changing", if call == 0 { "a" } else { "longer" })?;
            writer.end_record()
        }
    }

    #[test]
    fn a_payload_that_changes_between_passes_is_rejected() {
        let payload = NonDeterministicPayload {
            calls: Cell::new(0),
        };
        assert!(matches!(
            encode_exact_identity(&payload, ExactIdentityLimits::default()),
            Err(ExactIdentityError::EncodingMismatch {
                expected_bytes,
                actual_bytes,
            }) if actual_bytes > expected_bytes
        ));
    }

    struct MalformedPayload;

    impl ExactIdentityPayload for MalformedPayload {
        const SCHEMA: &'static str = "rustred-test-malformed-exact-identity-v1";

        fn write_exact_identity(
            &self,
            writer: &mut ExactIdentityWriter<'_>,
        ) -> Result<(), ExactIdentityError> {
            writer.begin_record("root", 2)?;
            writer.boolean("only-field", true)?;
            writer.end_record()
        }
    }

    #[test]
    fn declared_container_counts_are_authenticated() {
        assert!(matches!(
            encode_exact_identity(&MalformedPayload, ExactIdentityLimits::default()),
            Err(ExactIdentityError::ContainerItemCount {
                kind: ExactIdentityContainerKind::Record,
                expected: 2,
                actual: 1,
            })
        ));
    }

    struct SelfCensusPayload;

    impl ExactIdentityPayload for SelfCensusPayload {
        const SCHEMA: &'static str = "rustred-test-self-census-exact-identity-v1";

        fn write_exact_identity(
            &self,
            writer: &mut ExactIdentityWriter<'_>,
        ) -> Result<(), ExactIdentityError> {
            writer.begin_record("root", 4)?;
            writer.u128("zero", 0)?;
            writer.u128("maximum", u128::MAX)?;
            writer.identity_byte_count("bytes")?;
            writer.identity_stats("census")?;
            writer.end_record()
        }
    }

    #[test]
    fn fixed_width_u128_and_deferred_self_census_are_exact() {
        let identity =
            encode_exact_identity(&SelfCensusPayload, ExactIdentityLimits::default()).unwrap();
        let byte_count = identity.stats().identity_bytes() as u128;
        // Preamble lengths, five outer tag lengths, the root count, and the
        // self-census record count plus twelve component-tag lengths. The two
        // u128 values and twelve fixed-width census values are excluded.
        assert_eq!(identity.stats().integers(), 21);
        assert!(
            identity
                .as_str()
                .contains("U4:zero=00000000000000000000000000000000;")
        );
        assert!(
            identity
                .as_str()
                .contains("U7:maximum=FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF;")
        );
        assert!(
            identity
                .as_str()
                .contains(&format!("C5:bytes={byte_count:032X};"))
        );
        assert!(
            identity
                .as_str()
                .contains(&format!("14:identity_bytes={byte_count:032X};"))
        );
        assert!(
            !identity
                .as_str()
                .contains("C5:bytes=00000000000000000000000000000000;")
        );
    }

    struct EqualLengthChangingPayload {
        calls: Cell<usize>,
    }

    impl ExactIdentityPayload for EqualLengthChangingPayload {
        const SCHEMA: &'static str = "rustred-test-equal-length-changing-identity-v1";

        fn write_exact_identity(
            &self,
            writer: &mut ExactIdentityWriter<'_>,
        ) -> Result<(), ExactIdentityError> {
            let call = self.calls.get();
            self.calls.set(call + 1);
            let value = ["aa", "bb", "cc"][call.min(2)];
            writer.begin_record("root", 1)?;
            writer.string("changing", value)?;
            writer.end_record()
        }
    }

    #[test]
    fn equal_length_nondeterminism_is_rejected_by_comparison_replay() {
        let payload = EqualLengthChangingPayload {
            calls: Cell::new(0),
        };
        assert!(matches!(
            encode_exact_identity(&payload, ExactIdentityLimits::default()),
            Err(ExactIdentityError::EncodingContentMismatch { .. })
        ));
    }

    struct CensusThenStablePayload {
        calls: Cell<usize>,
    }

    impl ExactIdentityPayload for CensusThenStablePayload {
        const SCHEMA: &'static str = "rustred-test-a-b-b-identity-contract-v1";

        fn write_exact_identity(
            &self,
            writer: &mut ExactIdentityWriter<'_>,
        ) -> Result<(), ExactIdentityError> {
            let call = self.calls.get();
            self.calls.set(call + 1);
            writer.string("value", if call == 0 { "aa" } else { "bb" })
        }
    }

    #[test]
    fn a_b_b_interior_mutation_is_explicitly_outside_the_immutable_payload_contract() {
        // The contract requires a stable borrowed payload. The comparison
        // replay authenticates retained pass B against replay B; it cannot and
        // does not claim to detect arbitrary interior mutation where census A
        // has the same resource shape. Keeping this executable caveat prevents
        // the three-pass protocol from being overstated as a mutability proof.
        let payload = CensusThenStablePayload {
            calls: Cell::new(0),
        };
        let identity = encode_exact_identity(&payload, ExactIdentityLimits::default()).unwrap();
        assert_eq!(payload.calls.get(), 3);
        assert!(identity.as_str().contains("S5:value=2:bb;"));
    }

    enum StructuralFault {
        NoRoot,
        TwoRoots,
        WrongEnd,
        Unclosed,
    }

    impl ExactIdentityPayload for StructuralFault {
        const SCHEMA: &'static str = "rustred-test-structural-fault-identity-v1";

        fn write_exact_identity(
            &self,
            writer: &mut ExactIdentityWriter<'_>,
        ) -> Result<(), ExactIdentityError> {
            match self {
                Self::NoRoot => Ok(()),
                Self::TwoRoots => {
                    writer.boolean("first", false)?;
                    writer.boolean("second", true)
                }
                Self::WrongEnd => {
                    writer.begin_record("root", 0)?;
                    writer.end_sequence()
                }
                Self::Unclosed => writer.begin_record("root", 0),
            }
        }
    }

    #[test]
    fn root_and_container_structure_is_rejected_adversarially() {
        assert!(matches!(
            encode_exact_identity(&StructuralFault::NoRoot, ExactIdentityLimits::default()),
            Err(ExactIdentityError::RootValueCount { actual: 0 })
        ));
        assert!(matches!(
            encode_exact_identity(&StructuralFault::TwoRoots, ExactIdentityLimits::default()),
            Err(ExactIdentityError::RootValueCount { actual: 2 })
        ));
        assert!(matches!(
            encode_exact_identity(&StructuralFault::WrongEnd, ExactIdentityLimits::default()),
            Err(ExactIdentityError::UnexpectedContainerEnd {
                expected: Some(ExactIdentityContainerKind::Record),
                actual: ExactIdentityContainerKind::Sequence,
            })
        ));
        assert!(matches!(
            encode_exact_identity(&StructuralFault::Unclosed, ExactIdentityLimits::default()),
            Err(ExactIdentityError::UnclosedContainer {
                kind: ExactIdentityContainerKind::Record,
                depth: 1,
            })
        ));
    }

    struct ExcessiveNestingPayload;

    impl ExactIdentityPayload for ExcessiveNestingPayload {
        const SCHEMA: &'static str = "rustred-test-excessive-nesting-identity-v1";

        fn write_exact_identity(
            &self,
            writer: &mut ExactIdentityWriter<'_>,
        ) -> Result<(), ExactIdentityError> {
            for _ in 0..=IMPLEMENTATION_MAX_NESTING_DEPTH {
                writer.begin_record("nested", 1)?;
            }
            unreachable!("the fixed allocation-free nesting bound must reject this payload")
        }
    }

    #[test]
    fn fixed_stack_nesting_overflow_is_rejected_before_indexing_the_stack() {
        let mut limits = ExactIdentityLimits::default();
        // This test targets the fixed allocation-free stack rather than the
        // deliberately lower configurable production ceiling.
        limits.max_nesting_depth = IMPLEMENTATION_MAX_NESTING_DEPTH;
        assert!(matches!(
            encode_exact_identity(&ExcessiveNestingPayload, limits),
            Err(ExactIdentityError::ImplementationNestingLimit {
                requested,
                limit: IMPLEMENTATION_MAX_NESTING_DEPTH,
            }) if requested == IMPLEMENTATION_MAX_NESTING_DEPTH + 1
        ));
    }

    #[test]
    fn checked_resource_addition_reports_usize_overflow() {
        assert!(matches!(
            checked_add(IDENTITY_FIELDS_RESOURCE, usize::MAX, 1),
            Err(ExactIdentityError::ResourceCountOverflow {
                resource: IDENTITY_FIELDS_RESOURCE,
            })
        ));
    }

    struct DelimiterTagPayload {
        alternate: bool,
    }

    impl ExactIdentityPayload for DelimiterTagPayload {
        const SCHEMA: &'static str = "rustred-test-delimiter-tag-identity-v1";

        fn write_exact_identity(
            &self,
            writer: &mut ExactIdentityWriter<'_>,
        ) -> Result<(), ExactIdentityError> {
            let tag = if self.alternate {
                "a:;=[]|#"
            } else {
                "a|;=[]:#"
            };
            writer.string(tag, "v:;=[]|#")
        }
    }

    #[test]
    fn delimiter_rich_tags_are_length_delimited_and_injective() {
        let first = encode_exact_identity(
            &DelimiterTagPayload { alternate: false },
            ExactIdentityLimits::default(),
        )
        .unwrap();
        let second = encode_exact_identity(
            &DelimiterTagPayload { alternate: true },
            ExactIdentityLimits::default(),
        )
        .unwrap();
        assert!(first.as_str().contains("S8:a|;=[]:#=8:v:;=[]|#;"));
        assert!(second.as_str().contains("S8:a:;=[]|#=8:v:;=[]|#;"));
        assert_ne!(first.as_str(), second.as_str());
    }

    struct TypedTextPayload {
        variant: bool,
    }

    impl ExactIdentityPayload for TypedTextPayload {
        const SCHEMA: &'static str = "rustred-test-typed-text-identity-v1";

        fn write_exact_identity(
            &self,
            writer: &mut ExactIdentityWriter<'_>,
        ) -> Result<(), ExactIdentityError> {
            if self.variant {
                writer.variant("value", "same")
            } else {
                writer.string("value", "same")
            }
        }
    }

    #[test]
    fn equal_text_with_different_field_types_is_injective() {
        let string = encode_exact_identity(
            &TypedTextPayload { variant: false },
            ExactIdentityLimits::default(),
        )
        .unwrap();
        let variant = encode_exact_identity(
            &TypedTextPayload { variant: true },
            ExactIdentityLimits::default(),
        )
        .unwrap();
        assert_eq!(string.as_str().len(), variant.as_str().len());
        assert_ne!(string.as_str(), variant.as_str());
    }

    struct IntegerBoundaryPayload;

    impl ExactIdentityPayload for IntegerBoundaryPayload {
        const SCHEMA: &'static str = "rustred-test-integer-boundary-identity-v1";

        fn write_exact_identity(
            &self,
            writer: &mut ExactIdentityWriter<'_>,
        ) -> Result<(), ExactIdentityError> {
            let positive_large = (Integer::from(1) << 256u32) + Integer::from(0xABu64);
            let negative_large = -positive_large.clone();
            writer.begin_record("root", 7)?;
            writer.signed_i64("i64_min", i64::MIN)?;
            writer.signed_i64("i64_max", i64::MAX)?;
            writer.signed_i64("minus_one", -1)?;
            writer.integer("zero", &Integer::from(0))?;
            writer.integer("i128_min", &Integer::from(i128::MIN))?;
            writer.integer("large_positive", &positive_large)?;
            writer.integer("large_negative", &negative_large)?;
            writer.end_record()
        }
    }

    #[test]
    fn signed_integer_boundaries_have_canonical_sign_magnitude_hex() {
        let identity =
            encode_exact_identity(&IntegerBoundaryPayload, ExactIdentityLimits::default()).unwrap();
        assert!(identity.as_str().contains("I7:i64_min=-8000000000000000;"));
        assert!(identity.as_str().contains("I7:i64_max=+7FFFFFFFFFFFFFFF;"));
        assert!(identity.as_str().contains("I9:minus_one=-1;"));
        assert!(identity.as_str().contains("Z4:zero=+0;"));
        assert!(
            identity
                .as_str()
                .contains("Z8:i128_min=-80000000000000000000000000000000;")
        );
        assert!(
            identity
                .as_str()
                .contains(&format!("Z14:large_positive=+1{:064X};", 0xABu64))
        );
        assert!(
            identity
                .as_str()
                .contains(&format!("Z14:large_negative=-1{:064X};", 0xABu64))
        );
    }

    struct RelationPayload {
        relation: ParametricRelation,
    }

    impl ExactIdentityPayload for RelationPayload {
        const SCHEMA: &'static str = "rustred-test-relation-v2-identity-v1";

        fn write_exact_identity(
            &self,
            writer: &mut ExactIdentityWriter<'_>,
        ) -> Result<(), ExactIdentityError> {
            writer.parametric_relation("relation", &self.relation)
        }
    }

    const FROZEN_RELATION_V2_MANIFEST: &str = concat!(
        "rustred-parametric-relation-manifest-v2",
        "|family=5:F|:[]",
        "|context=100:rustred-parametric-context-v1|base=rustred-base-context-v1|parameters=1|5:theta|scope=2:v2|indices=2",
        "|row=15:derived:5:r:|[]",
        "|arity=2|terms=1",
        "|shift=4:-2,3",
        "|coefficient=276:rustred-parametric-coefficient-sparse-v1",
        "|numerator=rustred-parametric-polynomial-sparse-v1|variables=3|terms=1",
        "|coefficient=+1000000000000000000000000000000AB|exponents=0,0,0",
        "|denominator=rustred-parametric-polynomial-sparse-v1|variables=3|terms=1",
        "|coefficient=+1|exponents=0,0,0",
        "|guards=1",
        "|polynomial=90:rustred-parametric-polynomial-sparse-v1|variables=3|terms=1",
        "|coefficient=+1|exponents=0,1,0",
        "|origins=2",
        "|origin=43:relation-condition-attached:derived:5:r:|[]",
        "|origin=58:relation-translation:derived:3:s|::ordinary-ibp:7:8:[-9,4]",
    );

    fn sample_relation_with(
        family: &str,
        scope: &str,
        row_label: &str,
        shift: [i64; 2],
        origin_offset: [i64; 2],
    ) -> ParametricRelation {
        // Symbolica's sparse iterator cannot traverse a zero-variable
        // polynomial (`chunks(0)`). Keep one inert base parameter while the
        // GMP-sized constant below remains independent of that parameter.
        let base = CoefficientContext::new(["theta"]);
        let context = ParametricCoefficientContext::try_new(&base, scope, 2).unwrap();
        // 2^128 + 0xAB forces Symbolica's GMP-backed Integer representation.
        let huge = base
            .parse("340282366920938463463374607431768211627")
            .unwrap();
        let coefficient = context.lift(&huge).unwrap();
        let mut relation = ParametricRelation::new(
            family,
            ParametricRowId::Derived {
                label: Arc::from(row_label),
            },
            &context,
        );
        relation
            .add_term(
                &context,
                IndexSpace::try_new(2).unwrap().shift(shift).unwrap(),
                coefficient,
            )
            .unwrap();
        let guard = context
            .numerator_condition(&context.index(0).unwrap())
            .unwrap();
        let condition = context
            .nonzero_condition(
                guard,
                GuardOrigin::RelationTranslation {
                    source_row: GuardRowId::Derived {
                        label: Arc::from("s|:"),
                    },
                    target_row: GuardRowId::OrdinaryIbp {
                        contraction_momentum: 7,
                        differentiated_loop: 8,
                    },
                    offset: origin_offset.into_iter().collect(),
                },
            )
            .unwrap();
        relation
            .add_guarded_nonzero_condition(&context, condition)
            .unwrap();
        relation
    }

    fn sample_relation() -> ParametricRelation {
        sample_relation_with("F|:[]", "v2", "r:|[]", [-2, 3], [-9, 4])
    }

    fn ordinary_guard_row(contraction_momentum: usize, differentiated_loop: usize) -> GuardRowId {
        GuardRowId::OrdinaryIbp {
            contraction_momentum,
            differentiated_loop,
        }
    }

    fn lorentz_guard_row(first_external: usize, second_external: usize) -> GuardRowId {
        GuardRowId::LorentzInvariance {
            first_external,
            second_external,
        }
    }

    fn derived_guard_row(label: &'static str) -> GuardRowId {
        GuardRowId::Derived {
            label: Arc::from(label),
        }
    }

    fn guard_origin_context() -> ParametricCoefficientContext {
        let base = CoefficientContext::new(["theta"]);
        ParametricCoefficientContext::try_new(&base, "guard-origin-identity", 1).unwrap()
    }

    fn relation_with_guard_origin(
        context: &ParametricCoefficientContext,
        origin: GuardOrigin,
    ) -> ParametricRelation {
        let mut relation = ParametricRelation::new(
            "guard-origin-family",
            ParametricRowId::Derived {
                label: Arc::from("attached-row"),
            },
            context,
        );
        let polynomial = context
            .numerator_condition(&context.index(0).unwrap())
            .unwrap();
        let condition = context.nonzero_condition(polynomial, origin).unwrap();
        relation
            .add_guarded_nonzero_condition(context, condition)
            .unwrap();
        relation
    }

    fn identity_with_guard_origin(
        context: &ParametricCoefficientContext,
        origin: GuardOrigin,
    ) -> ExactStructuralIdentity {
        encode_exact_identity(
            &RelationPayload {
                relation: relation_with_guard_origin(context, origin),
            },
            ExactIdentityLimits::default(),
        )
        .unwrap()
    }

    fn guard_origin_representatives() -> Vec<(&'static str, GuardOrigin)> {
        vec![
            (
                "family-input-coefficient-denominator",
                GuardOrigin::FamilyInputCoefficientDenominator {
                    location: CoefficientLocation::DenominatorCoefficient {
                        denominator: 1,
                        coordinate: 2,
                    },
                },
            ),
            (
                "family-basis-determinant-numerator",
                GuardOrigin::FamilyBasisDeterminantNumerator,
            ),
            (
                "power-shift-support",
                GuardOrigin::PowerShiftSupport { denominator: 1 },
            ),
            (
                "guarded-division-dividend-denominator",
                GuardOrigin::GuardedDivisionDividendDenominator,
            ),
            (
                "guarded-division-divisor-denominator",
                GuardOrigin::GuardedDivisionDivisorDenominator,
            ),
            (
                "guarded-division-divisor-numerator",
                GuardOrigin::GuardedDivisionDivisorNumerator,
            ),
            (
                "explicit-relation-condition",
                GuardOrigin::ExplicitRelationCondition,
            ),
            (
                "generated-affine-sealed-condition",
                GuardOrigin::GeneratedAffineSealedCondition,
            ),
            (
                "relation-condition-attached",
                GuardOrigin::RelationConditionAttached {
                    row: derived_guard_row("source"),
                },
            ),
            (
                "relation-input-term-denominator",
                GuardOrigin::RelationInputTermDenominator {
                    row: derived_guard_row("source"),
                    shift: vec![1].into_boxed_slice(),
                },
            ),
            (
                "relation-collected-term-denominator",
                GuardOrigin::RelationCollectedTermDenominator {
                    row: derived_guard_row("source"),
                    shift: vec![1].into_boxed_slice(),
                },
            ),
            (
                "relation-scale-factor-denominator",
                GuardOrigin::RelationScaleFactorDenominator {
                    target_row: derived_guard_row("target"),
                    source_row: derived_guard_row("source"),
                },
            ),
            (
                "relation-translation",
                GuardOrigin::RelationTranslation {
                    source_row: derived_guard_row("source"),
                    target_row: derived_guard_row("target"),
                    offset: vec![1].into_boxed_slice(),
                },
            ),
            (
                "relation-affine-free-recentering",
                GuardOrigin::RelationAffineFreeRecentering {
                    source_row: derived_guard_row("source"),
                    target_row: derived_guard_row("target"),
                    coefficient_offset: vec![1],
                    key_center: vec![1],
                },
            ),
            (
                "relation-index-permutation",
                GuardOrigin::RelationIndexPermutation {
                    source_row: derived_guard_row("source"),
                    target_row: derived_guard_row("target"),
                    source_to_target: vec![1].into_boxed_slice(),
                },
            ),
            (
                "index-translation",
                GuardOrigin::IndexTranslation {
                    offset: vec![1].into_boxed_slice(),
                },
            ),
            (
                "index-permutation",
                GuardOrigin::IndexPermutation {
                    source_to_target: vec![1].into_boxed_slice(),
                },
            ),
            (
                "verified-symmetry-map-domain",
                GuardOrigin::VerifiedSymmetryMapDomain {
                    source_to_target: vec![1].into_boxed_slice(),
                    condition_ordinal: 1,
                },
            ),
            (
                "index-specialization",
                GuardOrigin::IndexSpecialization {
                    assignment: vec![1].into_boxed_slice(),
                },
            ),
            (
                "partial-index-specialization",
                GuardOrigin::PartialIndexSpecialization {
                    assignments: vec![(1, 1)].into_boxed_slice(),
                },
            ),
            (
                "residual-unit-affine-index-substitution",
                GuardOrigin::ResidualUnitAffineIndexSubstitution {
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
            ),
            (
                "residual-affine-branch-nonzero-guard-substitution",
                GuardOrigin::ResidualAffineBranchNonzeroGuardSubstitution {
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                    structural_locus_ordinal: 1,
                },
            ),
            (
                "coefficient-specialization-denominator",
                GuardOrigin::CoefficientSpecializationDenominator,
            ),
            (
                "coefficient-partial-specialization-denominator",
                GuardOrigin::CoefficientPartialSpecializationDenominator,
            ),
            (
                "relation-partial-specialization-term-denominator",
                GuardOrigin::RelationPartialSpecializationTermDenominator {
                    row: derived_guard_row("source"),
                    shift: vec![1].into_boxed_slice(),
                },
            ),
            (
                "coefficient-residual-unit-affine-substitution-denominator",
                GuardOrigin::CoefficientResidualUnitAffineSubstitutionDenominator {
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
            ),
            (
                "relation-residual-unit-affine-substitution-term-denominator",
                GuardOrigin::RelationResidualUnitAffineSubstitutionTermDenominator {
                    row: derived_guard_row("source"),
                    shift: vec![1].into_boxed_slice(),
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
            ),
            (
                "relation-residual-affine-branch-substitution-term-denominator",
                GuardOrigin::RelationResidualAffineBranchSubstitutionTermDenominator {
                    row: derived_guard_row("source"),
                    shift: vec![1].into_boxed_slice(),
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                },
            ),
            (
                "relation-residual-unit-affine-substitution",
                GuardOrigin::RelationResidualUnitAffineSubstitution {
                    source_row: derived_guard_row("source"),
                    target_row: derived_guard_row("target"),
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
            ),
            (
                "relation-residual-affine-branch-substitution",
                GuardOrigin::RelationResidualAffineBranchSubstitution {
                    source_row: derived_guard_row("source"),
                    target_row: derived_guard_row("target"),
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                },
            ),
            (
                "quotient-pivot-numerator",
                GuardOrigin::QuotientPivotNumerator,
            ),
            (
                "concrete-quotient-elimination-pivot-numerator",
                GuardOrigin::ConcreteQuotientEliminationPivotNumerator { pivot: 1 },
            ),
            (
                "explicit-shift-operator-condition",
                GuardOrigin::ExplicitShiftOperatorCondition,
            ),
            (
                "shift-operator-condition-attached",
                GuardOrigin::ShiftOperatorConditionAttached {
                    row: derived_guard_row("source"),
                },
            ),
            (
                "shift-operator-input-term-denominator",
                GuardOrigin::ShiftOperatorInputTermDenominator {
                    row: derived_guard_row("source"),
                },
            ),
            (
                "shift-operator-collected-term-denominator",
                GuardOrigin::ShiftOperatorCollectedTermDenominator {
                    row: derived_guard_row("source"),
                },
            ),
            (
                "shift-operator-from-relation-adapter",
                GuardOrigin::ShiftOperatorFromRelationAdapter {
                    row: derived_guard_row("source"),
                },
            ),
            (
                "shift-operator-to-relation-adapter",
                GuardOrigin::ShiftOperatorToRelationAdapter {
                    row: derived_guard_row("source"),
                },
            ),
            (
                "generated-affine-group-recentering",
                GuardOrigin::GeneratedAffineGroupRecentering {
                    solve_group_ordinal: 1,
                    database_epoch: 1,
                    event_ordinal: 1,
                },
            ),
            (
                "generated-affine-group-top-reduction-coefficient-denominator",
                GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
                    solve_group_ordinal: 1,
                    database_epoch: 1,
                    event_ordinal: 1,
                    operation_ordinal: 1,
                    term_ordinal: 1,
                    pivot_normalization: false,
                },
            ),
        ]
    }

    struct GuardOriginPayload {
        origin: GuardOrigin,
    }

    impl ExactIdentityPayload for GuardOriginPayload {
        const SCHEMA: &'static str = "rustred-test-direct-guard-origin-identity-v1";

        fn write_exact_identity(
            &self,
            writer: &mut ExactIdentityWriter<'_>,
        ) -> Result<(), ExactIdentityError> {
            writer.guard_origin("origin", &self.origin)
        }
    }

    #[test]
    fn direct_guard_origin_writer_is_exhaustive_injective_and_exactly_bounded() {
        let representatives = guard_origin_representatives();
        assert_eq!(representatives.len(), 40);
        let mut identities = BTreeSet::new();
        for (name, origin) in representatives {
            let identity = encode_exact_identity(
                &GuardOriginPayload { origin },
                ExactIdentityLimits::default(),
            )
            .unwrap();
            assert!(identities.insert(identity.as_str().to_owned()), "{name}");
        }

        let payload = GuardOriginPayload {
            origin: GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
                solve_group_ordinal: 1,
                database_epoch: 2,
                event_ordinal: 3,
                operation_ordinal: 4,
                term_ordinal: 5,
                pivot_normalization: true,
            },
        };
        let baseline = encode_exact_identity(&payload, ExactIdentityLimits::default()).unwrap();
        let exact = exact_limits(baseline.stats());
        assert_eq!(
            encode_exact_identity(&payload, exact).unwrap().as_str(),
            baseline.as_str()
        );

        let mut one_below = exact;
        one_below.max_fields -= 1;
        assert_one_below(
            &payload,
            one_below,
            IDENTITY_FIELDS_RESOURCE,
            baseline.stats().fields(),
        );
        let mut one_below = exact;
        one_below.max_integers -= 1;
        assert_one_below(
            &payload,
            one_below,
            IDENTITY_INTEGERS_RESOURCE,
            baseline.stats().integers(),
        );
        let mut one_below = exact;
        one_below.max_integer_bits -= 1;
        assert_one_below(
            &payload,
            one_below,
            IDENTITY_INTEGER_BITS_RESOURCE,
            baseline.stats().integer_bits(),
        );
    }

    #[test]
    fn every_guard_origin_variant_is_bound_by_the_exact_relation_identity() {
        let context = guard_origin_context();
        let representatives = guard_origin_representatives();
        assert_eq!(representatives.len(), 40);
        let mut names = BTreeSet::new();
        let mut identities = BTreeSet::new();
        for (name, origin) in representatives {
            assert!(names.insert(name));
            let stable_origin = origin.stable_string();
            let identity = identity_with_guard_origin(&context, origin);
            assert!(identity.as_str().contains(&stable_origin), "{name}");
            assert!(identities.insert(identity.as_str().to_owned()), "{name}");
        }
    }

    #[derive(Clone, Copy)]
    enum GuardOriginMutationExpectation {
        IdentityOnly,
        IntegerBitsIncrease,
        IntegerCountIncrease,
    }

    struct GuardOriginMutation {
        name: &'static str,
        baseline: GuardOrigin,
        mutated: GuardOrigin,
        expectation: GuardOriginMutationExpectation,
    }

    fn guard_origin_mutation(
        name: &'static str,
        baseline: GuardOrigin,
        mutated: GuardOrigin,
        expectation: GuardOriginMutationExpectation,
    ) -> GuardOriginMutation {
        GuardOriginMutation {
            name,
            baseline,
            mutated,
            expectation,
        }
    }

    fn guard_origin_field_mutations() -> Vec<GuardOriginMutation> {
        use GuardOriginMutationExpectation::{
            IdentityOnly, IntegerBitsIncrease, IntegerCountIncrease,
        };

        vec![
            guard_origin_mutation(
                "family-input.location.variant",
                GuardOrigin::FamilyInputCoefficientDenominator {
                    location: CoefficientLocation::Dimension,
                },
                GuardOrigin::FamilyInputCoefficientDenominator {
                    location: CoefficientLocation::BasisDeterminantNumerator,
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "family-input.denominator-constant.denominator",
                GuardOrigin::FamilyInputCoefficientDenominator {
                    location: CoefficientLocation::DenominatorConstant { denominator: 1 },
                },
                GuardOrigin::FamilyInputCoefficientDenominator {
                    location: CoefficientLocation::DenominatorConstant { denominator: 2 },
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "family-input.denominator-coefficient.denominator",
                GuardOrigin::FamilyInputCoefficientDenominator {
                    location: CoefficientLocation::DenominatorCoefficient {
                        denominator: 1,
                        coordinate: 1,
                    },
                },
                GuardOrigin::FamilyInputCoefficientDenominator {
                    location: CoefficientLocation::DenominatorCoefficient {
                        denominator: 2,
                        coordinate: 1,
                    },
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "family-input.denominator-coefficient.coordinate",
                GuardOrigin::FamilyInputCoefficientDenominator {
                    location: CoefficientLocation::DenominatorCoefficient {
                        denominator: 1,
                        coordinate: 1,
                    },
                },
                GuardOrigin::FamilyInputCoefficientDenominator {
                    location: CoefficientLocation::DenominatorCoefficient {
                        denominator: 1,
                        coordinate: 2,
                    },
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "family-input.external-gram.row",
                GuardOrigin::FamilyInputCoefficientDenominator {
                    location: CoefficientLocation::ExternalGram { row: 1, column: 1 },
                },
                GuardOrigin::FamilyInputCoefficientDenominator {
                    location: CoefficientLocation::ExternalGram { row: 2, column: 1 },
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "family-input.external-gram.column",
                GuardOrigin::FamilyInputCoefficientDenominator {
                    location: CoefficientLocation::ExternalGram { row: 1, column: 1 },
                },
                GuardOrigin::FamilyInputCoefficientDenominator {
                    location: CoefficientLocation::ExternalGram { row: 1, column: 2 },
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "family-input.power-shift.denominator",
                GuardOrigin::FamilyInputCoefficientDenominator {
                    location: CoefficientLocation::PowerShift { denominator: 1 },
                },
                GuardOrigin::FamilyInputCoefficientDenominator {
                    location: CoefficientLocation::PowerShift { denominator: 2 },
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "power-shift-support.denominator",
                GuardOrigin::PowerShiftSupport { denominator: 1 },
                GuardOrigin::PowerShiftSupport { denominator: 2 },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "guard-row.variant",
                GuardOrigin::RelationConditionAttached {
                    row: derived_guard_row("a"),
                },
                GuardOrigin::RelationConditionAttached {
                    row: ordinary_guard_row(1, 1),
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "guard-row.ordinary-ibp.contraction-momentum",
                GuardOrigin::RelationConditionAttached {
                    row: ordinary_guard_row(1, 1),
                },
                GuardOrigin::RelationConditionAttached {
                    row: ordinary_guard_row(2, 1),
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "guard-row.ordinary-ibp.differentiated-loop",
                GuardOrigin::RelationConditionAttached {
                    row: ordinary_guard_row(1, 1),
                },
                GuardOrigin::RelationConditionAttached {
                    row: ordinary_guard_row(1, 2),
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "guard-row.lorentz-invariance.first-external",
                GuardOrigin::RelationConditionAttached {
                    row: lorentz_guard_row(1, 1),
                },
                GuardOrigin::RelationConditionAttached {
                    row: lorentz_guard_row(2, 1),
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "guard-row.lorentz-invariance.second-external",
                GuardOrigin::RelationConditionAttached {
                    row: lorentz_guard_row(1, 1),
                },
                GuardOrigin::RelationConditionAttached {
                    row: lorentz_guard_row(1, 2),
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "guard-row.derived.label-content",
                GuardOrigin::RelationConditionAttached {
                    row: derived_guard_row("a"),
                },
                GuardOrigin::RelationConditionAttached {
                    row: derived_guard_row("b"),
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "guard-row.derived.label-length",
                GuardOrigin::RelationConditionAttached {
                    row: derived_guard_row("a"),
                },
                GuardOrigin::RelationConditionAttached {
                    row: derived_guard_row("aa"),
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "relation-input.row",
                GuardOrigin::RelationInputTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                },
                GuardOrigin::RelationInputTermDenominator {
                    row: derived_guard_row("b"),
                    shift: vec![1].into_boxed_slice(),
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "relation-input.shift-value",
                GuardOrigin::RelationInputTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                },
                GuardOrigin::RelationInputTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![2].into_boxed_slice(),
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "relation-input.shift-length",
                GuardOrigin::RelationInputTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                },
                GuardOrigin::RelationInputTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1, 1].into_boxed_slice(),
                },
                IntegerCountIncrease,
            ),
            guard_origin_mutation(
                "relation-collected.row",
                GuardOrigin::RelationCollectedTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                },
                GuardOrigin::RelationCollectedTermDenominator {
                    row: derived_guard_row("b"),
                    shift: vec![1].into_boxed_slice(),
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "relation-collected.shift-value",
                GuardOrigin::RelationCollectedTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                },
                GuardOrigin::RelationCollectedTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![2].into_boxed_slice(),
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "relation-collected.shift-length",
                GuardOrigin::RelationCollectedTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                },
                GuardOrigin::RelationCollectedTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1, 1].into_boxed_slice(),
                },
                IntegerCountIncrease,
            ),
            guard_origin_mutation(
                "relation-scale.target-row",
                GuardOrigin::RelationScaleFactorDenominator {
                    target_row: derived_guard_row("a"),
                    source_row: derived_guard_row("s"),
                },
                GuardOrigin::RelationScaleFactorDenominator {
                    target_row: derived_guard_row("b"),
                    source_row: derived_guard_row("s"),
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "relation-scale.source-row",
                GuardOrigin::RelationScaleFactorDenominator {
                    target_row: derived_guard_row("t"),
                    source_row: derived_guard_row("a"),
                },
                GuardOrigin::RelationScaleFactorDenominator {
                    target_row: derived_guard_row("t"),
                    source_row: derived_guard_row("b"),
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "relation-translation.source-row",
                GuardOrigin::RelationTranslation {
                    source_row: derived_guard_row("a"),
                    target_row: derived_guard_row("t"),
                    offset: vec![1].into_boxed_slice(),
                },
                GuardOrigin::RelationTranslation {
                    source_row: derived_guard_row("b"),
                    target_row: derived_guard_row("t"),
                    offset: vec![1].into_boxed_slice(),
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "relation-translation.target-row",
                GuardOrigin::RelationTranslation {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("a"),
                    offset: vec![1].into_boxed_slice(),
                },
                GuardOrigin::RelationTranslation {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("b"),
                    offset: vec![1].into_boxed_slice(),
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "relation-translation.offset-value",
                GuardOrigin::RelationTranslation {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    offset: vec![1].into_boxed_slice(),
                },
                GuardOrigin::RelationTranslation {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    offset: vec![2].into_boxed_slice(),
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "relation-translation.offset-length",
                GuardOrigin::RelationTranslation {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    offset: vec![1].into_boxed_slice(),
                },
                GuardOrigin::RelationTranslation {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    offset: vec![1, 1].into_boxed_slice(),
                },
                IntegerCountIncrease,
            ),
            guard_origin_mutation(
                "relation-recentering.source-row",
                GuardOrigin::RelationAffineFreeRecentering {
                    source_row: derived_guard_row("a"),
                    target_row: derived_guard_row("t"),
                    coefficient_offset: vec![1],
                    key_center: vec![1],
                },
                GuardOrigin::RelationAffineFreeRecentering {
                    source_row: derived_guard_row("b"),
                    target_row: derived_guard_row("t"),
                    coefficient_offset: vec![1],
                    key_center: vec![1],
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "relation-recentering.target-row",
                GuardOrigin::RelationAffineFreeRecentering {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("a"),
                    coefficient_offset: vec![1],
                    key_center: vec![1],
                },
                GuardOrigin::RelationAffineFreeRecentering {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("b"),
                    coefficient_offset: vec![1],
                    key_center: vec![1],
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "relation-recentering.coefficient-offset-value",
                GuardOrigin::RelationAffineFreeRecentering {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    coefficient_offset: vec![1],
                    key_center: vec![1],
                },
                GuardOrigin::RelationAffineFreeRecentering {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    coefficient_offset: vec![2],
                    key_center: vec![1],
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "relation-recentering.coefficient-offset-length",
                GuardOrigin::RelationAffineFreeRecentering {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    coefficient_offset: vec![1],
                    key_center: vec![1],
                },
                GuardOrigin::RelationAffineFreeRecentering {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    coefficient_offset: vec![1, 1],
                    key_center: vec![1],
                },
                IntegerCountIncrease,
            ),
            guard_origin_mutation(
                "relation-recentering.key-center-value",
                GuardOrigin::RelationAffineFreeRecentering {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    coefficient_offset: vec![1],
                    key_center: vec![1],
                },
                GuardOrigin::RelationAffineFreeRecentering {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    coefficient_offset: vec![1],
                    key_center: vec![2],
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "relation-recentering.key-center-length",
                GuardOrigin::RelationAffineFreeRecentering {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    coefficient_offset: vec![1],
                    key_center: vec![1],
                },
                GuardOrigin::RelationAffineFreeRecentering {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    coefficient_offset: vec![1],
                    key_center: vec![1, 1],
                },
                IntegerCountIncrease,
            ),
            guard_origin_mutation(
                "relation-index-permutation.source-row",
                GuardOrigin::RelationIndexPermutation {
                    source_row: derived_guard_row("a"),
                    target_row: derived_guard_row("t"),
                    source_to_target: vec![1].into_boxed_slice(),
                },
                GuardOrigin::RelationIndexPermutation {
                    source_row: derived_guard_row("b"),
                    target_row: derived_guard_row("t"),
                    source_to_target: vec![1].into_boxed_slice(),
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "relation-index-permutation.target-row",
                GuardOrigin::RelationIndexPermutation {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("a"),
                    source_to_target: vec![1].into_boxed_slice(),
                },
                GuardOrigin::RelationIndexPermutation {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("b"),
                    source_to_target: vec![1].into_boxed_slice(),
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "relation-index-permutation.mapping-value",
                GuardOrigin::RelationIndexPermutation {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    source_to_target: vec![1].into_boxed_slice(),
                },
                GuardOrigin::RelationIndexPermutation {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    source_to_target: vec![2].into_boxed_slice(),
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "relation-index-permutation.mapping-length",
                GuardOrigin::RelationIndexPermutation {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    source_to_target: vec![1].into_boxed_slice(),
                },
                GuardOrigin::RelationIndexPermutation {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    source_to_target: vec![1, 1].into_boxed_slice(),
                },
                IntegerCountIncrease,
            ),
            guard_origin_mutation(
                "index-translation.offset-value",
                GuardOrigin::IndexTranslation {
                    offset: vec![1].into_boxed_slice(),
                },
                GuardOrigin::IndexTranslation {
                    offset: vec![2].into_boxed_slice(),
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "index-translation.offset-length",
                GuardOrigin::IndexTranslation {
                    offset: vec![1].into_boxed_slice(),
                },
                GuardOrigin::IndexTranslation {
                    offset: vec![1, 1].into_boxed_slice(),
                },
                IntegerCountIncrease,
            ),
            guard_origin_mutation(
                "index-permutation.mapping-value",
                GuardOrigin::IndexPermutation {
                    source_to_target: vec![1].into_boxed_slice(),
                },
                GuardOrigin::IndexPermutation {
                    source_to_target: vec![2].into_boxed_slice(),
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "index-permutation.mapping-length",
                GuardOrigin::IndexPermutation {
                    source_to_target: vec![1].into_boxed_slice(),
                },
                GuardOrigin::IndexPermutation {
                    source_to_target: vec![1, 1].into_boxed_slice(),
                },
                IntegerCountIncrease,
            ),
            guard_origin_mutation(
                "verified-symmetry.mapping-value",
                GuardOrigin::VerifiedSymmetryMapDomain {
                    source_to_target: vec![1].into_boxed_slice(),
                    condition_ordinal: 1,
                },
                GuardOrigin::VerifiedSymmetryMapDomain {
                    source_to_target: vec![2].into_boxed_slice(),
                    condition_ordinal: 1,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "verified-symmetry.mapping-length",
                GuardOrigin::VerifiedSymmetryMapDomain {
                    source_to_target: vec![1].into_boxed_slice(),
                    condition_ordinal: 1,
                },
                GuardOrigin::VerifiedSymmetryMapDomain {
                    source_to_target: vec![1, 1].into_boxed_slice(),
                    condition_ordinal: 1,
                },
                IntegerCountIncrease,
            ),
            guard_origin_mutation(
                "verified-symmetry.condition-ordinal",
                GuardOrigin::VerifiedSymmetryMapDomain {
                    source_to_target: vec![1].into_boxed_slice(),
                    condition_ordinal: 1,
                },
                GuardOrigin::VerifiedSymmetryMapDomain {
                    source_to_target: vec![1].into_boxed_slice(),
                    condition_ordinal: 2,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "index-specialization.assignment-value",
                GuardOrigin::IndexSpecialization {
                    assignment: vec![1].into_boxed_slice(),
                },
                GuardOrigin::IndexSpecialization {
                    assignment: vec![2].into_boxed_slice(),
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "index-specialization.assignment-length",
                GuardOrigin::IndexSpecialization {
                    assignment: vec![1].into_boxed_slice(),
                },
                GuardOrigin::IndexSpecialization {
                    assignment: vec![1, 1].into_boxed_slice(),
                },
                IntegerCountIncrease,
            ),
            guard_origin_mutation(
                "partial-index-specialization.position",
                GuardOrigin::PartialIndexSpecialization {
                    assignments: vec![(1, 1)].into_boxed_slice(),
                },
                GuardOrigin::PartialIndexSpecialization {
                    assignments: vec![(2, 1)].into_boxed_slice(),
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "partial-index-specialization.value",
                GuardOrigin::PartialIndexSpecialization {
                    assignments: vec![(1, 1)].into_boxed_slice(),
                },
                GuardOrigin::PartialIndexSpecialization {
                    assignments: vec![(1, 2)].into_boxed_slice(),
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "partial-index-specialization.length",
                GuardOrigin::PartialIndexSpecialization {
                    assignments: vec![(1, 1)].into_boxed_slice(),
                },
                GuardOrigin::PartialIndexSpecialization {
                    assignments: vec![(1, 1), (2, 2)].into_boxed_slice(),
                },
                IntegerCountIncrease,
            ),
            guard_origin_mutation(
                "residual-unit-affine.source-case",
                GuardOrigin::ResidualUnitAffineIndexSubstitution {
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                GuardOrigin::ResidualUnitAffineIndexSubstitution {
                    source_case: 2,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "residual-unit-affine.predicate-ordinal",
                GuardOrigin::ResidualUnitAffineIndexSubstitution {
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                GuardOrigin::ResidualUnitAffineIndexSubstitution {
                    source_case: 1,
                    predicate_ordinal: 2,
                    bound_position: 1,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "residual-unit-affine.bound-position",
                GuardOrigin::ResidualUnitAffineIndexSubstitution {
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                GuardOrigin::ResidualUnitAffineIndexSubstitution {
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 2,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "residual-affine-branch.source-case",
                GuardOrigin::ResidualAffineBranchNonzeroGuardSubstitution {
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                    structural_locus_ordinal: 1,
                },
                GuardOrigin::ResidualAffineBranchNonzeroGuardSubstitution {
                    source_case: 2,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                    structural_locus_ordinal: 1,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "residual-affine-branch.source-work-item-ordinal",
                GuardOrigin::ResidualAffineBranchNonzeroGuardSubstitution {
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                    structural_locus_ordinal: 1,
                },
                GuardOrigin::ResidualAffineBranchNonzeroGuardSubstitution {
                    source_case: 1,
                    source_work_item_ordinal: 2,
                    ready_terminal_ordinal: 1,
                    structural_locus_ordinal: 1,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "residual-affine-branch.ready-terminal-ordinal",
                GuardOrigin::ResidualAffineBranchNonzeroGuardSubstitution {
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                    structural_locus_ordinal: 1,
                },
                GuardOrigin::ResidualAffineBranchNonzeroGuardSubstitution {
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 2,
                    structural_locus_ordinal: 1,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "residual-affine-branch.structural-locus-ordinal",
                GuardOrigin::ResidualAffineBranchNonzeroGuardSubstitution {
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                    structural_locus_ordinal: 1,
                },
                GuardOrigin::ResidualAffineBranchNonzeroGuardSubstitution {
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                    structural_locus_ordinal: 2,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "relation-partial-specialization.row",
                GuardOrigin::RelationPartialSpecializationTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                },
                GuardOrigin::RelationPartialSpecializationTermDenominator {
                    row: derived_guard_row("b"),
                    shift: vec![1].into_boxed_slice(),
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "relation-partial-specialization.shift-value",
                GuardOrigin::RelationPartialSpecializationTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                },
                GuardOrigin::RelationPartialSpecializationTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![2].into_boxed_slice(),
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "relation-partial-specialization.shift-length",
                GuardOrigin::RelationPartialSpecializationTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                },
                GuardOrigin::RelationPartialSpecializationTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1, 1].into_boxed_slice(),
                },
                IntegerCountIncrease,
            ),
            guard_origin_mutation(
                "coefficient-residual-unit.source-case",
                GuardOrigin::CoefficientResidualUnitAffineSubstitutionDenominator {
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                GuardOrigin::CoefficientResidualUnitAffineSubstitutionDenominator {
                    source_case: 2,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "coefficient-residual-unit.predicate-ordinal",
                GuardOrigin::CoefficientResidualUnitAffineSubstitutionDenominator {
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                GuardOrigin::CoefficientResidualUnitAffineSubstitutionDenominator {
                    source_case: 1,
                    predicate_ordinal: 2,
                    bound_position: 1,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "coefficient-residual-unit.bound-position",
                GuardOrigin::CoefficientResidualUnitAffineSubstitutionDenominator {
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                GuardOrigin::CoefficientResidualUnitAffineSubstitutionDenominator {
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 2,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "relation-residual-unit-term.row",
                GuardOrigin::RelationResidualUnitAffineSubstitutionTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                GuardOrigin::RelationResidualUnitAffineSubstitutionTermDenominator {
                    row: derived_guard_row("b"),
                    shift: vec![1].into_boxed_slice(),
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "relation-residual-unit-term.shift-value",
                GuardOrigin::RelationResidualUnitAffineSubstitutionTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                GuardOrigin::RelationResidualUnitAffineSubstitutionTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![2].into_boxed_slice(),
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "relation-residual-unit-term.shift-length",
                GuardOrigin::RelationResidualUnitAffineSubstitutionTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                GuardOrigin::RelationResidualUnitAffineSubstitutionTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1, 1].into_boxed_slice(),
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                IntegerCountIncrease,
            ),
            guard_origin_mutation(
                "relation-residual-unit-term.source-case",
                GuardOrigin::RelationResidualUnitAffineSubstitutionTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                GuardOrigin::RelationResidualUnitAffineSubstitutionTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                    source_case: 2,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "relation-residual-unit-term.predicate-ordinal",
                GuardOrigin::RelationResidualUnitAffineSubstitutionTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                GuardOrigin::RelationResidualUnitAffineSubstitutionTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                    source_case: 1,
                    predicate_ordinal: 2,
                    bound_position: 1,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "relation-residual-unit-term.bound-position",
                GuardOrigin::RelationResidualUnitAffineSubstitutionTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                GuardOrigin::RelationResidualUnitAffineSubstitutionTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 2,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "relation-residual-affine-term.row",
                GuardOrigin::RelationResidualAffineBranchSubstitutionTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                },
                GuardOrigin::RelationResidualAffineBranchSubstitutionTermDenominator {
                    row: derived_guard_row("b"),
                    shift: vec![1].into_boxed_slice(),
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "relation-residual-affine-term.shift-value",
                GuardOrigin::RelationResidualAffineBranchSubstitutionTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                },
                GuardOrigin::RelationResidualAffineBranchSubstitutionTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![2].into_boxed_slice(),
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "relation-residual-affine-term.shift-length",
                GuardOrigin::RelationResidualAffineBranchSubstitutionTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                },
                GuardOrigin::RelationResidualAffineBranchSubstitutionTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1, 1].into_boxed_slice(),
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                },
                IntegerCountIncrease,
            ),
            guard_origin_mutation(
                "relation-residual-affine-term.source-case",
                GuardOrigin::RelationResidualAffineBranchSubstitutionTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                },
                GuardOrigin::RelationResidualAffineBranchSubstitutionTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                    source_case: 2,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "relation-residual-affine-term.source-work-item-ordinal",
                GuardOrigin::RelationResidualAffineBranchSubstitutionTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                },
                GuardOrigin::RelationResidualAffineBranchSubstitutionTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                    source_case: 1,
                    source_work_item_ordinal: 2,
                    ready_terminal_ordinal: 1,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "relation-residual-affine-term.ready-terminal-ordinal",
                GuardOrigin::RelationResidualAffineBranchSubstitutionTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                },
                GuardOrigin::RelationResidualAffineBranchSubstitutionTermDenominator {
                    row: derived_guard_row("a"),
                    shift: vec![1].into_boxed_slice(),
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 2,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "relation-residual-unit.source-row",
                GuardOrigin::RelationResidualUnitAffineSubstitution {
                    source_row: derived_guard_row("a"),
                    target_row: derived_guard_row("t"),
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                GuardOrigin::RelationResidualUnitAffineSubstitution {
                    source_row: derived_guard_row("b"),
                    target_row: derived_guard_row("t"),
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "relation-residual-unit.target-row",
                GuardOrigin::RelationResidualUnitAffineSubstitution {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("a"),
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                GuardOrigin::RelationResidualUnitAffineSubstitution {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("b"),
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "relation-residual-unit.source-case",
                GuardOrigin::RelationResidualUnitAffineSubstitution {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                GuardOrigin::RelationResidualUnitAffineSubstitution {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    source_case: 2,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "relation-residual-unit.predicate-ordinal",
                GuardOrigin::RelationResidualUnitAffineSubstitution {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                GuardOrigin::RelationResidualUnitAffineSubstitution {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    source_case: 1,
                    predicate_ordinal: 2,
                    bound_position: 1,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "relation-residual-unit.bound-position",
                GuardOrigin::RelationResidualUnitAffineSubstitution {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 1,
                },
                GuardOrigin::RelationResidualUnitAffineSubstitution {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    source_case: 1,
                    predicate_ordinal: 1,
                    bound_position: 2,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "relation-residual-affine.source-row",
                GuardOrigin::RelationResidualAffineBranchSubstitution {
                    source_row: derived_guard_row("a"),
                    target_row: derived_guard_row("t"),
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                },
                GuardOrigin::RelationResidualAffineBranchSubstitution {
                    source_row: derived_guard_row("b"),
                    target_row: derived_guard_row("t"),
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "relation-residual-affine.target-row",
                GuardOrigin::RelationResidualAffineBranchSubstitution {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("a"),
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                },
                GuardOrigin::RelationResidualAffineBranchSubstitution {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("b"),
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "relation-residual-affine.source-case",
                GuardOrigin::RelationResidualAffineBranchSubstitution {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                },
                GuardOrigin::RelationResidualAffineBranchSubstitution {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    source_case: 2,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "relation-residual-affine.source-work-item-ordinal",
                GuardOrigin::RelationResidualAffineBranchSubstitution {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                },
                GuardOrigin::RelationResidualAffineBranchSubstitution {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    source_case: 1,
                    source_work_item_ordinal: 2,
                    ready_terminal_ordinal: 1,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "relation-residual-affine.ready-terminal-ordinal",
                GuardOrigin::RelationResidualAffineBranchSubstitution {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 1,
                },
                GuardOrigin::RelationResidualAffineBranchSubstitution {
                    source_row: derived_guard_row("s"),
                    target_row: derived_guard_row("t"),
                    source_case: 1,
                    source_work_item_ordinal: 1,
                    ready_terminal_ordinal: 2,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "concrete-quotient-elimination.pivot",
                GuardOrigin::ConcreteQuotientEliminationPivotNumerator { pivot: 1 },
                GuardOrigin::ConcreteQuotientEliminationPivotNumerator { pivot: 2 },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "shift-operator-condition.row",
                GuardOrigin::ShiftOperatorConditionAttached {
                    row: derived_guard_row("a"),
                },
                GuardOrigin::ShiftOperatorConditionAttached {
                    row: derived_guard_row("b"),
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "shift-operator-input.row",
                GuardOrigin::ShiftOperatorInputTermDenominator {
                    row: derived_guard_row("a"),
                },
                GuardOrigin::ShiftOperatorInputTermDenominator {
                    row: derived_guard_row("b"),
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "shift-operator-collected.row",
                GuardOrigin::ShiftOperatorCollectedTermDenominator {
                    row: derived_guard_row("a"),
                },
                GuardOrigin::ShiftOperatorCollectedTermDenominator {
                    row: derived_guard_row("b"),
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "shift-operator-from-relation.row",
                GuardOrigin::ShiftOperatorFromRelationAdapter {
                    row: derived_guard_row("a"),
                },
                GuardOrigin::ShiftOperatorFromRelationAdapter {
                    row: derived_guard_row("b"),
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "shift-operator-to-relation.row",
                GuardOrigin::ShiftOperatorToRelationAdapter {
                    row: derived_guard_row("a"),
                },
                GuardOrigin::ShiftOperatorToRelationAdapter {
                    row: derived_guard_row("b"),
                },
                IdentityOnly,
            ),
            guard_origin_mutation(
                "generated-affine-group-recentering.solve-group-ordinal",
                GuardOrigin::GeneratedAffineGroupRecentering {
                    solve_group_ordinal: 1,
                    database_epoch: 1,
                    event_ordinal: 1,
                },
                GuardOrigin::GeneratedAffineGroupRecentering {
                    solve_group_ordinal: 2,
                    database_epoch: 1,
                    event_ordinal: 1,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "generated-affine-group-recentering.database-epoch",
                GuardOrigin::GeneratedAffineGroupRecentering {
                    solve_group_ordinal: 1,
                    database_epoch: 1,
                    event_ordinal: 1,
                },
                GuardOrigin::GeneratedAffineGroupRecentering {
                    solve_group_ordinal: 1,
                    database_epoch: 2,
                    event_ordinal: 1,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "generated-affine-group-recentering.event-ordinal",
                GuardOrigin::GeneratedAffineGroupRecentering {
                    solve_group_ordinal: 1,
                    database_epoch: 1,
                    event_ordinal: 1,
                },
                GuardOrigin::GeneratedAffineGroupRecentering {
                    solve_group_ordinal: 1,
                    database_epoch: 1,
                    event_ordinal: 2,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "generated-affine-group-top-reduction-coefficient-denominator.solve-group-ordinal",
                GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
                    solve_group_ordinal: 1,
                    database_epoch: 1,
                    event_ordinal: 1,
                    operation_ordinal: 1,
                    term_ordinal: 1,
                    pivot_normalization: false,
                },
                GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
                    solve_group_ordinal: 2,
                    database_epoch: 1,
                    event_ordinal: 1,
                    operation_ordinal: 1,
                    term_ordinal: 1,
                    pivot_normalization: false,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "generated-affine-group-top-reduction-coefficient-denominator.database-epoch",
                GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
                    solve_group_ordinal: 1,
                    database_epoch: 1,
                    event_ordinal: 1,
                    operation_ordinal: 1,
                    term_ordinal: 1,
                    pivot_normalization: false,
                },
                GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
                    solve_group_ordinal: 1,
                    database_epoch: 2,
                    event_ordinal: 1,
                    operation_ordinal: 1,
                    term_ordinal: 1,
                    pivot_normalization: false,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "generated-affine-group-top-reduction-coefficient-denominator.event-ordinal",
                GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
                    solve_group_ordinal: 1,
                    database_epoch: 1,
                    event_ordinal: 1,
                    operation_ordinal: 1,
                    term_ordinal: 1,
                    pivot_normalization: false,
                },
                GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
                    solve_group_ordinal: 1,
                    database_epoch: 1,
                    event_ordinal: 2,
                    operation_ordinal: 1,
                    term_ordinal: 1,
                    pivot_normalization: false,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "generated-affine-group-top-reduction-coefficient-denominator.operation-ordinal",
                GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
                    solve_group_ordinal: 1,
                    database_epoch: 1,
                    event_ordinal: 1,
                    operation_ordinal: 1,
                    term_ordinal: 1,
                    pivot_normalization: false,
                },
                GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
                    solve_group_ordinal: 1,
                    database_epoch: 1,
                    event_ordinal: 1,
                    operation_ordinal: 2,
                    term_ordinal: 1,
                    pivot_normalization: false,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "generated-affine-group-top-reduction-coefficient-denominator.term-ordinal",
                GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
                    solve_group_ordinal: 1,
                    database_epoch: 1,
                    event_ordinal: 1,
                    operation_ordinal: 1,
                    term_ordinal: 1,
                    pivot_normalization: false,
                },
                GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
                    solve_group_ordinal: 1,
                    database_epoch: 1,
                    event_ordinal: 1,
                    operation_ordinal: 1,
                    term_ordinal: 2,
                    pivot_normalization: false,
                },
                IntegerBitsIncrease,
            ),
            guard_origin_mutation(
                "generated-affine-group-top-reduction-coefficient-denominator.pivot-normalization",
                GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
                    solve_group_ordinal: 1,
                    database_epoch: 1,
                    event_ordinal: 1,
                    operation_ordinal: 1,
                    term_ordinal: 1,
                    pivot_normalization: false,
                },
                GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
                    solve_group_ordinal: 1,
                    database_epoch: 1,
                    event_ordinal: 1,
                    operation_ordinal: 1,
                    term_ordinal: 1,
                    pivot_normalization: true,
                },
                IdentityOnly,
            ),
        ]
    }

    #[test]
    fn every_guard_origin_field_mutation_changes_identity_and_semantic_census() {
        let context = guard_origin_context();
        let mutations = guard_origin_field_mutations();
        assert_eq!(mutations.len(), 99);
        let mut names = BTreeSet::new();
        for mutation in mutations {
            assert!(names.insert(mutation.name));
            assert_ne!(
                mutation.baseline.stable_string(),
                mutation.mutated.stable_string(),
                "{}",
                mutation.name
            );
            let baseline = identity_with_guard_origin(&context, mutation.baseline);
            let mutated = identity_with_guard_origin(&context, mutation.mutated);
            assert_ne!(baseline.as_str(), mutated.as_str(), "{}", mutation.name);
            match mutation.expectation {
                GuardOriginMutationExpectation::IdentityOnly => {}
                GuardOriginMutationExpectation::IntegerBitsIncrease => assert!(
                    mutated.stats().integer_bits() > baseline.stats().integer_bits(),
                    "{}",
                    mutation.name
                ),
                GuardOriginMutationExpectation::IntegerCountIncrease => assert!(
                    mutated.stats().integers() > baseline.stats().integers(),
                    "{}",
                    mutation.name
                ),
            }
        }
    }

    #[test]
    fn parametric_relation_v2_field_matches_a_literal_frozen_golden_fixture() {
        let relation = sample_relation();
        // This literal is intentionally independent of both persistence and
        // exact-identity writers. It freezes delimiter handling, one GMP-sized
        // coefficient, and a nested-row/vector GuardOrigin payload.
        assert_eq!(relation.stable_manifest(), FROZEN_RELATION_V2_MANIFEST);
        let identity = encode_exact_identity(
            &RelationPayload { relation },
            ExactIdentityLimits::default(),
        )
        .unwrap();
        assert!(
            identity
                .as_str()
                .contains(&format!("L8:relation{{{FROZEN_RELATION_V2_MANIFEST}}};"))
        );
        assert_eq!(identity.stats().string_values(), 6);
        assert_eq!(identity.stats().polynomials(), 3);
        assert_eq!(identity.stats().polynomial_terms(), 3);
        assert_eq!(identity.stats().polynomial_variables(), 9);
        assert_eq!(identity.stats().exponent_entries(), 9);
        // Forty-four integers belong to the V2 relation event stream. The
        // outer identity adds two preamble lengths and the relation tag length.
        assert_eq!(identity.stats().integers(), 47);
        assert!(identity.stats().integer_bits() >= 129);
    }

    #[test]
    fn embedded_relation_length_prefix_maps_its_finite_ceiling_to_identity_bytes() {
        let base = CoefficientContext::new(["theta"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "finite-prefix-identity", 1).unwrap();
        let relation = ParametricRelation::new(
            "f".repeat(4_096),
            ParametricRowId::Derived {
                label: Arc::from("row"),
            },
            &context,
        );
        let payload = RelationPayload { relation };
        let mut limits = ExactIdentityLimits::default();
        limits.max_identity_bytes = 512;
        assert!(matches!(
            encode_exact_identity(&payload, limits),
            Err(ExactIdentityError::ResourceLimit {
                resource: IDENTITY_BYTES_RESOURCE,
                requested,
                limit: 512,
            }) if requested > 512
        ));
    }

    #[test]
    fn embedded_relation_identity_binds_every_major_relation_class() {
        let baseline = encode_exact_identity(
            &RelationPayload {
                relation: sample_relation(),
            },
            ExactIdentityLimits::default(),
        )
        .unwrap();

        let mutations = [
            // Family identity.
            sample_relation_with("G|:[]", "v2", "r:|[]", [-2, 3], [-9, 4]),
            // Parametric coefficient-context identity.
            sample_relation_with("F|:[]", "w2", "r:|[]", [-2, 3], [-9, 4]),
            // Row identity (and its mechanically attached guard provenance).
            sample_relation_with("F|:[]", "v2", "q:|[]", [-2, 3], [-9, 4]),
            // Integral-lattice shift.
            sample_relation_with("F|:[]", "v2", "r:|[]", [-1, 3], [-9, 4]),
            // Representative nested/vector guard provenance.
            sample_relation_with("F|:[]", "v2", "r:|[]", [-2, 3], [-8, 4]),
        ];
        for mutation in mutations {
            let identity = encode_exact_identity(
                &RelationPayload { relation: mutation },
                ExactIdentityLimits::default(),
            )
            .unwrap();
            assert_ne!(identity.as_str(), baseline.as_str());
        }
    }

    #[test]
    fn embedded_relation_v2_resources_enforce_exact_one_below_limits() {
        let payload = RelationPayload {
            relation: sample_relation(),
        };
        let baseline = encode_exact_identity(&payload, ExactIdentityLimits::default()).unwrap();
        let exact = exact_limits(baseline.stats());
        assert_eq!(
            encode_exact_identity(&payload, exact).unwrap().as_str(),
            baseline.as_str()
        );

        let mut one_below = exact;
        one_below.max_string_values -= 1;
        assert_one_below(
            &payload,
            one_below,
            IDENTITY_STRING_VALUES_RESOURCE,
            baseline.stats().string_values(),
        );

        let mut one_below = exact;
        one_below.max_string_bytes -= 1;
        assert_one_below(
            &payload,
            one_below,
            IDENTITY_STRING_BYTES_RESOURCE,
            baseline.stats().string_bytes(),
        );

        let mut one_below = exact;
        one_below.max_polynomials -= 1;
        assert_one_below(
            &payload,
            one_below,
            IDENTITY_POLYNOMIALS_RESOURCE,
            baseline.stats().polynomials(),
        );

        let mut one_below = exact;
        one_below.max_polynomial_variables -= 1;
        assert_one_below(
            &payload,
            one_below,
            IDENTITY_POLYNOMIAL_VARIABLES_RESOURCE,
            baseline.stats().polynomial_variables(),
        );

        let mut one_below = exact;
        one_below.max_polynomial_terms -= 1;
        assert_one_below(
            &payload,
            one_below,
            IDENTITY_POLYNOMIAL_TERMS_RESOURCE,
            baseline.stats().polynomial_terms(),
        );

        let mut one_below = exact;
        one_below.max_exponent_entries -= 1;
        assert_one_below(
            &payload,
            one_below,
            IDENTITY_EXPONENT_ENTRIES_RESOURCE,
            baseline.stats().exponent_entries(),
        );

        let mut one_below = exact;
        one_below.max_integers -= 1;
        assert_one_below(
            &payload,
            one_below,
            IDENTITY_INTEGERS_RESOURCE,
            baseline.stats().integers(),
        );

        let mut one_below = exact;
        one_below.max_integer_bits -= 1;
        assert_one_below(
            &payload,
            one_below,
            IDENTITY_INTEGER_BITS_RESOURCE,
            baseline.stats().integer_bits(),
        );
    }
}
