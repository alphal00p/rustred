//! Exact physical lattice coordinates shared by one generated affine group.
//!
//! Every case in a contiguous inventory group has the same integer matrix
//! `A` and differs only by its constant vector:
//!
//! ```text
//! s_u(n) = A n + b_u = A n + b_0 + o_u,   o_u = b_u - b_0.
//! ```
//!
//! A local integral key `q` produced for case `u` therefore denotes the
//! group-physical key `r = o_u + q`.  Conversely, a physical pivot `r` has
//! local coordinate `p = r - o_u` in that case.  These operations must use
//! arbitrary-precision integers: both inventory offsets and pivots produced
//! after cross-case elimination can exceed `i64` even though an individual
//! generated source row initially stores compact [`IndexShift`] keys.
//!
//! The production implementation is topology and loop-count independent.  A
//! frame is bound to the exact [`GeneratedAffineResidualCaseAuthority`] `Arc`,
//! authenticates the complete group once, and retains canonical Symbolica
//! integers.  Canonicalization is essential because Symbolica's public
//! `Integer::{Double,Large}` variants permit value-equal noncanonical
//! representations while its derived `Eq`/`Hash` compare representation.

#[cfg(test)]
use std::cell::Cell;
use std::cmp::Ordering;
use std::fmt;
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};
use std::mem::{align_of, replace, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use symbolica::prelude::Integer;

use crate::affine_parametric_ordering::integer_magnitude_bits;
use crate::generated_affine_residual_case_inventory::{
    GeneratedAffineResidualCaseAuthority, GeneratedAffineResidualCaseAuthorityError,
    GeneratedAffineResidualInventoryCaseSourceRecordView,
    GeneratedAffineResidualInventoryGroupSourceView,
};
use crate::{
    IndexShift, IntegralFamily, IntegralOrderingPolicy, ParametricCoefficientContext, SectorMask,
};

pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_FRAME_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-group-physical-frame-v1";
pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_KEY_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-group-physical-key-v1";

const AUTHORITY_REPLAYS: usize = 1;
const CASE_VIEW_RESOLUTIONS: usize = 1;
const GROUP_VIEW_RESOLUTIONS: usize = 1;
const RETAINED_AUTHORITY_REFERENCES: usize = 1;

#[cfg(test)]
thread_local! {
    static PHYSICAL_FROM_LOCAL_EXECUTIONS: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_physical_from_local_executions_for_test() {
    PHYSICAL_FROM_LOCAL_EXECUTIONS.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn physical_from_local_executions_for_test() -> usize {
    PHYSICAL_FROM_LOCAL_EXECUTIONS.with(Cell::get)
}

/// Complete construction and future-key logical resource envelope for one
/// group.  The retained-byte ceilings admit conservative prospective payload
/// charges and then check observed owner-visible capacities.  They are not an
/// allocator-independent peak-RSS contract: `Vec::try_reserve_exact` and
/// `Arc` do not expose such a contract, and conservative GMP admission can be
/// larger than the ultimately retained payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupPhysicalKeyLimits {
    pub(crate) max_authority_replays: usize,
    pub(crate) max_case_view_resolutions: usize,
    pub(crate) max_group_view_resolutions: usize,
    pub(crate) max_retained_authority_references: usize,
    pub(crate) max_group_cases: usize,
    pub(crate) max_arity: usize,
    pub(crate) max_free_positions: usize,
    pub(crate) max_matrix_entries_inspected: usize,
    pub(crate) max_offset_components: usize,
    pub(crate) max_geometry_integer_bits: usize,
    pub(crate) max_geometry_integer_bit_work: usize,
    pub(crate) max_geometry_total_integer_bits: usize,
    pub(crate) max_frame_retained_bytes: usize,
    pub(crate) max_manifest_bytes: usize,
    pub(crate) max_shift_integer_bits: usize,
    pub(crate) max_shift_total_integer_bits: usize,
    pub(crate) max_shift_retained_bytes: usize,
    pub(crate) max_key_integer_bits: usize,
    pub(crate) max_key_total_integer_bits: usize,
    pub(crate) max_key_retained_bytes: usize,
}

impl Default for GeneratedAffineResidualGroupPhysicalKeyLimits {
    fn default() -> Self {
        Self {
            max_authority_replays: AUTHORITY_REPLAYS,
            max_case_view_resolutions: CASE_VIEW_RESOLUTIONS,
            max_group_view_resolutions: GROUP_VIEW_RESOLUTIONS,
            max_retained_authority_references: RETAINED_AUTHORITY_REFERENCES,
            max_group_cases: 256_000_000,
            max_arity: 1_000_000,
            max_free_positions: 1_000_000,
            max_matrix_entries_inspected: 16_777_216_000,
            max_offset_components: 16_777_216_000,
            max_geometry_integer_bits: 1_000_000,
            max_geometry_integer_bit_work: 64_000_000_000,
            max_geometry_total_integer_bits: 512 * 1024 * 1024,
            max_frame_retained_bytes: 64 * 1024 * 1024 * 1024,
            max_manifest_bytes: 2 * 1024 * 1024 * 1024,
            max_shift_integer_bits: 1_000_000,
            max_shift_total_integer_bits: 512 * 1024 * 1024,
            max_shift_retained_bytes: 8 * 1024 * 1024 * 1024,
            max_key_integer_bits: 1_000_016,
            max_key_total_integer_bits: 1024 * 1024 * 1024,
            max_key_retained_bytes: 16 * 1024 * 1024 * 1024,
        }
    }
}

/// Exact construction census.  Integer-bit totals count magnitude bits; zero
/// contributes zero.  Retained bytes are a conservative owner-visible payload
/// estimate including observed vector buffers and GMP capacities, but not the
/// separately retained authority child or allocator metadata.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupPhysicalKeyStats {
    authority_replays: usize,
    case_view_resolutions: usize,
    group_view_resolutions: usize,
    retained_authority_references: usize,
    group_cases: usize,
    arity: usize,
    free_positions: usize,
    constant_positions: usize,
    symbolic_positions: usize,
    matrix_entries_inspected: usize,
    offset_components: usize,
    largest_geometry_integer_bits: usize,
    geometry_integer_bit_work: usize,
    retained_geometry_integer_bits: usize,
    frame_retained_bytes: usize,
    manifest_bytes: usize,
}

/// Opaque, geometry-free admission census for constructing one physical key.
///
/// The selected-group solve-plan builder sums these values for every retained
/// anchor offset before it constructs any temporary GMP-backed key.  Byte
/// demand is deliberately conservative and includes the referenced physical
/// shift payload even though the executed key shares that payload through an
/// `Arc`; it is therefore an admission/scratch bound, not a unique-allocation
/// or peak-RSS measurement.
pub(crate) struct GeneratedAffineResidualGroupPhysicalKeyPreflight {
    frame: Arc<GeneratedAffineResidualGroupPhysicalFrame>,
    physical: GeneratedAffineResidualGroupLatticeShift,
    census: GeneratedAffineResidualGroupPhysicalKeyPreflightCensus,
}

/// Allocation-free prospective census for mapping one compact case-local key
/// into a physical key.
///
/// Unlike [`GeneratedAffineResidualGroupPhysicalKeyPreflight`], this value is
/// not an execution token and owns no physical shift or frame reference. It
/// contains only scalar upper bounds derived by scanning borrowed frame
/// geometry and the borrowed `i64` local shift. Exact-group owners sum these
/// censuses before any GMP-backed physical shift is allocated, then repeat
/// the scan immediately before execution and verify the ordinary physical-key
/// preflight remains within this bound.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupLocalPhysicalKeyPreflightCensus {
    component_scans: usize,
    integer_bit_work: usize,
    prospective_retained_integer_bits: usize,
    prospective_retained_bytes: usize,
    prospective_comparison_integer_bit_work: usize,
}

impl GeneratedAffineResidualGroupLocalPhysicalKeyPreflightCensus {
    pub(crate) const fn component_scans(self) -> usize {
        self.component_scans
    }

    pub(crate) const fn integer_bit_work(self) -> usize {
        self.integer_bit_work
    }

    pub(crate) const fn prospective_retained_integer_bits(self) -> usize {
        self.prospective_retained_integer_bits
    }

    pub(crate) const fn prospective_retained_bytes(self) -> usize {
        self.prospective_retained_bytes
    }

    pub(crate) const fn prospective_comparison_integer_bit_work(self) -> usize {
        self.prospective_comparison_integer_bit_work
    }

    pub(crate) fn authenticates_physical_preflight(
        self,
        physical: &GeneratedAffineResidualGroupPhysicalKeyPreflight,
    ) -> bool {
        physical.component_scans() <= self.component_scans
            && physical.integer_bit_work() <= self.integer_bit_work
            && physical.prospective_retained_integer_bits()
                <= self.prospective_retained_integer_bits
            && physical.prospective_retained_bytes() <= self.prospective_retained_bytes
    }
}

#[derive(Clone, Copy)]
struct GeneratedAffineResidualGroupPhysicalKeyPreflightCensus {
    component_scans: usize,
    integer_bit_work: usize,
    prospective_retained_integer_bits: usize,
    prospective_retained_bytes: usize,
}

impl fmt::Debug for GeneratedAffineResidualGroupPhysicalKeyPreflight {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupPhysicalKeyPreflight")
            .field("component_scans", &self.census.component_scans)
            .field("integer_bit_work", &self.census.integer_bit_work)
            .field(
                "prospective_retained_integer_bits",
                &self.census.prospective_retained_integer_bits,
            )
            .field(
                "prospective_retained_bytes",
                &self.census.prospective_retained_bytes,
            )
            .field("private_frame_binding", &"<redacted>")
            .field("private_physical_shift", &"<redacted>")
            .finish()
    }
}

impl GeneratedAffineResidualGroupPhysicalKeyPreflight {
    pub(crate) const fn component_scans(&self) -> usize {
        self.census.component_scans
    }
    pub(crate) const fn integer_bit_work(&self) -> usize {
        self.census.integer_bit_work
    }
    pub(crate) const fn prospective_retained_integer_bits(&self) -> usize {
        self.census.prospective_retained_integer_bits
    }
    pub(crate) const fn prospective_retained_bytes(&self) -> usize {
        self.census.prospective_retained_bytes
    }
}

macro_rules! stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedAffineResidualGroupPhysicalKeyStats {
    stats_getters!(
        authority_replays,
        case_view_resolutions,
        group_view_resolutions,
        retained_authority_references,
        group_cases,
        arity,
        free_positions,
        constant_positions,
        symbolic_positions,
        matrix_entries_inspected,
        offset_components,
        largest_geometry_integer_bits,
        geometry_integer_bit_work,
        retained_geometry_integer_bits,
        frame_retained_bytes,
        manifest_bytes,
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupPhysicalKeyError {
    SchemaMismatch,
    ReplayMismatch,
    WrongAuthorityAllocation,
    WrongFrameAllocation,
    WrongFamily,
    WrongContext,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    WrongGroup,
    WrongCase,
    WrongCasePosition,
    MalformedGeometry,
    IntegerOutsideI64 {
        position: usize,
    },
    Authority,
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
}

impl GeneratedAffineResidualGroupPhysicalKeyError {
    const fn kind(self) -> &'static str {
        match self {
            Self::SchemaMismatch => "SchemaMismatch",
            Self::ReplayMismatch => "ReplayMismatch",
            Self::WrongAuthorityAllocation => "WrongAuthorityAllocation",
            Self::WrongFrameAllocation => "WrongFrameAllocation",
            Self::WrongFamily => "WrongFamily",
            Self::WrongContext => "WrongContext",
            Self::WrongArity { .. } => "WrongArity",
            Self::WrongGroup => "WrongGroup",
            Self::WrongCase => "WrongCase",
            Self::WrongCasePosition => "WrongCasePosition",
            Self::MalformedGeometry => "MalformedGeometry",
            Self::IntegerOutsideI64 { .. } => "IntegerOutsideI64",
            Self::Authority => "Authority",
            Self::ResourceLimit { .. } => "ResourceLimit",
            Self::ResourceCountOverflow { .. } => "ResourceCountOverflow",
            Self::AllocationFailure { .. } => "AllocationFailure",
            Self::SymbolicaPanic => "SymbolicaPanic",
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupPhysicalKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupPhysicalKeyError")
            .field("kind", &self.kind())
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupPhysicalKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "generated affine group physical-key {}",
            self.kind()
        )
    }
}

impl std::error::Error for GeneratedAffineResidualGroupPhysicalKeyError {}

impl From<GeneratedAffineResidualCaseAuthorityError>
    for GeneratedAffineResidualGroupPhysicalKeyError
{
    fn from(_: GeneratedAffineResidualCaseAuthorityError) -> Self {
        Self::Authority
    }
}

/// Canonical arbitrary-precision displacement in one integral-index lattice.
///
/// The value has no group identity on its own.  The future group database
/// binds its entire key collection to one exact frame/solve-plan allocation.
/// Fields remain private so representation-based equality and hashing cannot
/// be invalidated by a noncanonical `Integer`.
#[derive(Clone)]
pub(crate) struct GeneratedAffineResidualGroupLatticeShift {
    values: Arc<Vec<Integer>>,
    retained_integer_bits: usize,
    retained_bytes: usize,
}

impl GeneratedAffineResidualGroupLatticeShift {
    pub(crate) fn values(&self) -> &[Integer] {
        self.values.as_slice()
    }
    pub(crate) fn arity(&self) -> usize {
        self.values.len()
    }
    pub(crate) const fn retained_integer_bits(&self) -> usize {
        self.retained_integer_bits
    }
    pub(crate) const fn retained_bytes(&self) -> usize {
        // This is the observed owner-visible payload census.  Reconstructing
        // under this exact value can still fail a deliberately conservative
        // pre-arithmetic admission charge.
        self.retained_bytes
    }

    pub(crate) fn try_to_index_shift(
        &self,
    ) -> Result<IndexShift, GeneratedAffineResidualGroupPhysicalKeyError> {
        for (position, value) in self.values.iter().enumerate() {
            if integer_to_i64(value).is_none() {
                return Err(
                    GeneratedAffineResidualGroupPhysicalKeyError::IntegerOutsideI64 { position },
                );
            }
        }
        let mut values = try_vec_with_capacity("i64 lattice-shift components", self.arity())?;
        for value in self.values.iter() {
            values.push(integer_to_i64(value).expect("range was authenticated before allocation"));
        }
        IndexShift::try_from_preallocated(values, self.arity())
            .map_err(|_| GeneratedAffineResidualGroupPhysicalKeyError::ReplayMismatch)
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupLatticeShift {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupLatticeShift")
            .field("arity", &self.arity())
            .field("retained_integer_bits", &self.retained_integer_bits)
            .field("private_values", &"<redacted>")
            .finish()
    }
}

impl PartialEq for GeneratedAffineResidualGroupLatticeShift {
    fn eq(&self, other: &Self) -> bool {
        self.values == other.values
    }
}
impl Eq for GeneratedAffineResidualGroupLatticeShift {}
impl Hash for GeneratedAffineResidualGroupLatticeShift {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.values.hash(state);
    }
}
impl Ord for GeneratedAffineResidualGroupLatticeShift {
    fn cmp(&self, other: &Self) -> Ordering {
        self.values.cmp(&other.values)
    }
}
impl PartialOrd for GeneratedAffineResidualGroupLatticeShift {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Pure formal complexity key for `J(A*n + b_anchor + r)`.
///
/// Group ancestry deliberately lives in the enclosing frame/solve plan, not
/// in this ordered value.  Putting allocation identity, manifests, or limits
/// into `Ord` would make mathematical pivot order depend on resource policy.
/// Production databases must therefore accept keys only through their exact
/// retained frame/plan and never mix independently scoped key collections.
#[derive(Clone)]
pub(crate) struct GeneratedAffineResidualGroupPhysicalKey {
    schema: &'static str,
    policy: IntegralOrderingPolicy,
    arity: usize,
    propagators: usize,
    formal_sector: Arc<SectorMask>,
    corner_distance_offset: Arc<Integer>,
    dots_offset: Arc<Integer>,
    numerators_offset: Arc<Integer>,
    signed_index_excess: Arc<Vec<Integer>>,
    shift: GeneratedAffineResidualGroupLatticeShift,
    retained_integer_bits: usize,
    retained_bytes: usize,
}

impl GeneratedAffineResidualGroupPhysicalKey {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }
    pub(crate) const fn policy(&self) -> IntegralOrderingPolicy {
        self.policy
    }
    pub(crate) const fn propagators(&self) -> usize {
        self.propagators
    }
    pub(crate) fn formal_sector(&self) -> &SectorMask {
        self.formal_sector.as_ref()
    }
    pub(crate) fn shift(&self) -> &GeneratedAffineResidualGroupLatticeShift {
        &self.shift
    }
    pub(crate) fn signed_index_excess(&self) -> &[Integer] {
        self.signed_index_excess.as_slice()
    }
    pub(crate) const fn retained_integer_bits(&self) -> usize {
        self.retained_integer_bits
    }
    pub(crate) const fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    /// Conservative GMP comparison work for one complete physical-key `cmp`.
    ///
    /// Key ordering can short-circuit before later fields, but persistent
    /// exact elimination admits the complete compared payload so its budget
    /// is independent of the first unequal component. Zero-valued fields
    /// still cost one unit to inspect. The integer-bearing payload is exactly
    /// the three totals, signed-excess vector, and physical-shift vector.
    pub(crate) fn comparison_integer_bit_work(
        &self,
        other: &Self,
    ) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
        if self.arity != other.arity {
            return Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongArity {
                expected: self.arity,
                actual: other.arity,
            });
        }
        let left = self
            .signed_index_excess
            .iter()
            .chain(self.shift.values().iter())
            .chain([
                self.corner_distance_offset.as_ref(),
                self.dots_offset.as_ref(),
                self.numerators_offset.as_ref(),
            ]);
        let right = other
            .signed_index_excess
            .iter()
            .chain(other.shift.values().iter())
            .chain([
                other.corner_distance_offset.as_ref(),
                other.dots_offset.as_ref(),
                other.numerators_offset.as_ref(),
            ]);
        integer_field_comparison_bit_work(left.chain(right))
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupPhysicalKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupPhysicalKey")
            .field("schema", &self.schema)
            .field("arity", &self.arity)
            .field("propagators", &self.propagators)
            .field("retained_integer_bits", &self.retained_integer_bits)
            .field("private_shift", &"<redacted>")
            .field("private_frame", &"<redacted>")
            .finish()
    }
}

impl PartialEq for GeneratedAffineResidualGroupPhysicalKey {
    fn eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.policy == other.policy
            && self.arity == other.arity
            && self.propagators == other.propagators
            && self.formal_sector == other.formal_sector
            && self.corner_distance_offset == other.corner_distance_offset
            && self.dots_offset == other.dots_offset
            && self.numerators_offset == other.numerators_offset
            && self.signed_index_excess == other.signed_index_excess
            && self.shift == other.shift
    }
}
impl Eq for GeneratedAffineResidualGroupPhysicalKey {}
impl Hash for GeneratedAffineResidualGroupPhysicalKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.schema.hash(state);
        self.policy.hash(state);
        self.arity.hash(state);
        self.propagators.hash(state);
        self.formal_sector.hash(state);
        self.corner_distance_offset.hash(state);
        self.dots_offset.hash(state);
        self.numerators_offset.hash(state);
        self.signed_index_excess.hash(state);
        self.shift.hash(state);
    }
}
impl Ord for GeneratedAffineResidualGroupPhysicalKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.policy
            .cmp(&other.policy)
            .then_with(|| self.arity.cmp(&other.arity))
            .then_with(|| self.propagators.cmp(&other.propagators))
            .then_with(|| self.formal_sector.cmp(&other.formal_sector))
            .then_with(|| {
                self.corner_distance_offset
                    .cmp(&other.corner_distance_offset)
            })
            .then_with(|| self.dots_offset.cmp(&other.dots_offset))
            .then_with(|| self.numerators_offset.cmp(&other.numerators_offset))
            .then_with(|| self.signed_index_excess.cmp(&other.signed_index_excess))
            .then_with(|| self.shift.cmp(&other.shift))
            .then_with(|| self.schema.cmp(other.schema))
    }
}
impl PartialOrd for GeneratedAffineResidualGroupPhysicalKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Authenticated common-coordinate frame for one exact inventory group.
#[derive(Clone)]
pub(crate) struct GeneratedAffineResidualGroupPhysicalFrame {
    schema: &'static str,
    authority: Arc<GeneratedAffineResidualCaseAuthority>,
    source_case_ordinal: usize,
    source_ordinal_within_group: usize,
    group_ordinal: usize,
    anchor_case_ordinal: usize,
    policy: IntegralOrderingPolicy,
    sector: Arc<SectorMask>,
    case_ordinals: Arc<Vec<usize>>,
    anchor_offsets: Arc<Vec<GeneratedAffineResidualGroupLatticeShift>>,
    anchor_constants: GeneratedAffineResidualGroupLatticeShift,
    constant_positions: Arc<Vec<usize>>,
    symbolic_positions: Arc<Vec<usize>>,
    limits: GeneratedAffineResidualGroupPhysicalKeyLimits,
    stats: GeneratedAffineResidualGroupPhysicalKeyStats,
    stable_manifest: Arc<String>,
}

impl fmt::Debug for GeneratedAffineResidualGroupPhysicalFrame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupPhysicalFrame")
            .field("schema", &self.schema)
            .field("source_case_ordinal", &self.source_case_ordinal)
            .field("group_ordinal", &self.group_ordinal)
            .field("anchor_case_ordinal", &self.anchor_case_ordinal)
            .field("arity", &self.arity())
            .field("case_count", &self.case_ordinals.len())
            .field("private_authority", &"<redacted>")
            .field("private_geometry", &"<redacted>")
            .finish()
    }
}

impl PartialEq for GeneratedAffineResidualGroupPhysicalFrame {
    fn eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && Arc::ptr_eq(&self.authority, &other.authority)
            && self.source_case_ordinal == other.source_case_ordinal
            && self.source_ordinal_within_group == other.source_ordinal_within_group
            && self.group_ordinal == other.group_ordinal
            && self.anchor_case_ordinal == other.anchor_case_ordinal
            && self.policy == other.policy
            && self.sector == other.sector
            && self.case_ordinals == other.case_ordinals
            && self.anchor_offsets == other.anchor_offsets
            && self.anchor_constants == other.anchor_constants
            && self.constant_positions == other.constant_positions
            && self.symbolic_positions == other.symbolic_positions
            && self.limits == other.limits
            && self.stats == other.stats
            && self.stable_manifest == other.stable_manifest
    }
}
impl Eq for GeneratedAffineResidualGroupPhysicalFrame {}

impl GeneratedAffineResidualGroupPhysicalFrame {
    pub(crate) fn try_new(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: Arc<GeneratedAffineResidualCaseAuthority>,
        limits: GeneratedAffineResidualGroupPhysicalKeyLimits,
    ) -> Result<Self, GeneratedAffineResidualGroupPhysicalKeyError> {
        catch_unwind(AssertUnwindSafe(|| {
            Self::try_new_unwind_boundary(family, context, authority, limits)
        }))
        .map_err(|_| GeneratedAffineResidualGroupPhysicalKeyError::SymbolicaPanic)?
    }

    fn try_new_unwind_boundary(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: Arc<GeneratedAffineResidualCaseAuthority>,
        limits: GeneratedAffineResidualGroupPhysicalKeyLimits,
    ) -> Result<Self, GeneratedAffineResidualGroupPhysicalKeyError> {
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
                "retained authority references",
                RETAINED_AUTHORITY_REFERENCES,
                limits.max_retained_authority_references,
            ),
        ] {
            check_limit(resource, requested, limit)?;
        }
        if family.fingerprint_ref() != authority.family_fingerprint() {
            return Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongFamily);
        }
        if context.fingerprint() != authority.context_fingerprint() {
            return Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongContext);
        }
        authority.replay(family, context)?;
        let case = authority.authenticated_case_view(context)?;
        let group = authority.authenticated_group_view(context)?;
        validate_group_binding(authority.as_ref(), case, group)?;

        let arity = authority.arity();
        let group_cases = group.case_ordinals().len();
        let free_positions = group.free_positions().len();
        let matrix_entries = checked_mul("group matrix entries", arity, free_positions)?;
        // A successful construction authenticates the free-row identity once,
        // scans all coefficients for integer work and row classification, and
        // traverses them in both manifest passes.
        let matrix_entries_inspected = checked_add(
            "matrix entries inspected",
            checked_mul("matrix entries inspected", free_positions, free_positions)?,
            checked_mul("matrix entries inspected", matrix_entries, 4)?,
        )?;
        let offset_components = checked_mul("group offset components", group_cases, arity)?;
        for (resource, requested, limit) in [
            ("group cases", group_cases, limits.max_group_cases),
            ("ambient arity", arity, limits.max_arity),
            ("free positions", free_positions, limits.max_free_positions),
            (
                "matrix entries inspected",
                matrix_entries_inspected,
                limits.max_matrix_entries_inspected,
            ),
            (
                "offset components",
                offset_components,
                limits.max_offset_components,
            ),
        ] {
            check_limit(resource, requested, limit)?;
        }
        if group.compact_linear_coefficients().len() != matrix_entries
            || group.anchor_offsets().len() != group_cases
            || group.case_ordinals().first().copied() != Some(group.anchor_case_ordinal())
        {
            return Err(GeneratedAffineResidualGroupPhysicalKeyError::MalformedGeometry);
        }
        let mut previous_free = None;
        for (free_ordinal, &free_position) in group.free_positions().iter().enumerate() {
            if free_position >= arity
                || previous_free.is_some_and(|previous| previous >= free_position)
            {
                return Err(GeneratedAffineResidualGroupPhysicalKeyError::MalformedGeometry);
            }
            previous_free = Some(free_position);
            for column in 0..free_positions {
                let compact_offset = free_position
                    .checked_mul(free_positions)
                    .and_then(|offset| offset.checked_add(column))
                    .ok_or(
                        GeneratedAffineResidualGroupPhysicalKeyError::ResourceCountOverflow {
                            resource: "group free-row matrix offset",
                        },
                    )?;
                let coefficient = group
                    .compact_linear_coefficients()
                    .get(compact_offset)
                    .ok_or(GeneratedAffineResidualGroupPhysicalKeyError::MalformedGeometry)?;
                let expected = Integer::from(usize::from(column == free_ordinal));
                if coefficient.cmp(&expected) != Ordering::Equal {
                    return Err(GeneratedAffineResidualGroupPhysicalKeyError::MalformedGeometry);
                }
            }
        }

        let source_position = case.ordinal_within_group();
        let source_offset = group
            .anchor_offsets()
            .get(source_position)
            .ok_or(GeneratedAffineResidualGroupPhysicalKeyError::WrongCasePosition)?;
        if source_offset.len() != arity || case.constants().len() != arity {
            return Err(GeneratedAffineResidualGroupPhysicalKeyError::MalformedGeometry);
        }

        // Admit all arbitrary-precision geometry before cloning or arithmetic.
        let mut geometry_integer_bit_work = 0usize;
        let mut retained_geometry_integer_bits = 0usize;
        let mut largest_geometry_integer_bits = 0usize;
        let mut prospective_offset_heap_bytes = 0usize;
        for offset in group.anchor_offsets() {
            if offset.len() != arity {
                return Err(GeneratedAffineResidualGroupPhysicalKeyError::MalformedGeometry);
            }
            for value in offset {
                let bits = checked_integer_bits(
                    "group geometry integer bits",
                    value,
                    limits.max_geometry_integer_bits,
                )?;
                geometry_integer_bit_work = bounded_add(
                    "group geometry integer-bit work",
                    geometry_integer_bit_work,
                    bits.max(1),
                    limits.max_geometry_integer_bit_work,
                )?;
                largest_geometry_integer_bits = largest_geometry_integer_bits.max(bits);
                retained_geometry_integer_bits = bounded_add(
                    "retained group geometry integer bits",
                    retained_geometry_integer_bits,
                    bits,
                    limits.max_geometry_total_integer_bits,
                )?;
                prospective_offset_heap_bytes = checked_add(
                    "prospective frame retained bytes",
                    prospective_offset_heap_bytes,
                    prospective_integer_heap_bytes(bits)?,
                )?;
            }
        }
        let mut prospective_anchor_heap_bytes = 0usize;
        for (constant, offset) in case.constants().iter().zip(source_offset) {
            let left_bits = checked_integer_bits(
                "group geometry integer bits",
                constant,
                limits.max_geometry_integer_bits,
            )?;
            let right_bits = checked_integer_bits(
                "group geometry integer bits",
                offset,
                limits.max_geometry_integer_bits,
            )?;
            let requested = prospective_add_bits(left_bits, right_bits)?;
            check_limit(
                "group anchor integer bits",
                requested,
                limits.max_geometry_integer_bits,
            )?;
            geometry_integer_bit_work = bounded_add(
                "group geometry integer-bit work",
                geometry_integer_bit_work,
                checked_add(
                    "group geometry integer-bit work",
                    left_bits.max(1),
                    right_bits.max(1),
                )?,
                limits.max_geometry_integer_bit_work,
            )?;
            largest_geometry_integer_bits = largest_geometry_integer_bits.max(requested);
            prospective_anchor_heap_bytes = checked_add(
                "prospective frame retained bytes",
                prospective_anchor_heap_bytes,
                prospective_integer_heap_bytes(requested)?,
            )?;
        }
        for coefficient in group.compact_linear_coefficients() {
            let bits = checked_integer_bits(
                "group geometry integer bits",
                coefficient,
                limits.max_geometry_integer_bits,
            )?;
            geometry_integer_bit_work = bounded_add(
                "group geometry integer-bit work",
                geometry_integer_bit_work,
                bits.max(1),
                limits.max_geometry_integer_bit_work,
            )?;
            largest_geometry_integer_bits = largest_geometry_integer_bits.max(bits);
        }
        check_limit(
            "group geometry integer-bit work",
            geometry_integer_bit_work,
            limits.max_geometry_integer_bit_work,
        )?;
        let prospective_frame_base_bytes = prospective_frame_base_retained_bytes(
            arity,
            group_cases,
            prospective_offset_heap_bytes,
            prospective_anchor_heap_bytes,
        )?;
        check_limit(
            "frame retained bytes",
            prospective_frame_base_bytes,
            limits.max_frame_retained_bytes,
        )?;

        let mut sector_bits = try_vec_with_capacity("group sector bits", arity)?;
        sector_bits.extend_from_slice(authority.sector().active_bits());
        let sector = SectorMask::try_from_preallocated(sector_bits)
            .map_err(|_| GeneratedAffineResidualGroupPhysicalKeyError::MalformedGeometry)?;
        let mut case_ordinals = try_vec_with_capacity("group case ordinals", group_cases)?;
        case_ordinals.extend_from_slice(group.case_ordinals());
        let mut anchor_offsets = try_vec_with_capacity("group anchor offsets", group_cases)?;
        for offset in group.anchor_offsets() {
            anchor_offsets.push(make_shift_from_borrowed(
                offset,
                arity,
                limits.max_geometry_integer_bits,
                limits.max_geometry_total_integer_bits,
                limits.max_frame_retained_bytes,
            )?);
        }
        if anchor_offsets
            .first()
            .is_none_or(|offset| offset.values().iter().any(|value| !value.is_zero()))
        {
            return Err(GeneratedAffineResidualGroupPhysicalKeyError::MalformedGeometry);
        }

        let anchor_constants = subtract_borrowed_vectors(
            case.constants(),
            source_offset,
            arity,
            limits.max_geometry_integer_bits,
            limits.max_geometry_total_integer_bits,
            limits.max_frame_retained_bytes,
        )?;
        retained_geometry_integer_bits = bounded_add(
            "retained group geometry integer bits",
            retained_geometry_integer_bits,
            anchor_constants.retained_integer_bits(),
            limits.max_geometry_total_integer_bits,
        )?;
        for &free_position in group.free_positions() {
            if !anchor_constants.values()[free_position].is_zero()
                || group.anchor_offsets().iter().any(|offset| {
                    offset
                        .get(free_position)
                        .is_none_or(|value| !value.is_zero())
                })
            {
                return Err(GeneratedAffineResidualGroupPhysicalKeyError::MalformedGeometry);
            }
        }

        let mut constant_positions = try_vec_with_capacity("constant positions", arity)?;
        let mut symbolic_positions = try_vec_with_capacity("symbolic positions", arity)?;
        for position in 0..arity {
            let row_start = checked_mul("group matrix row", position, free_positions)?;
            let row_end = checked_add("group matrix row", row_start, free_positions)?;
            let row = group
                .compact_linear_coefficients()
                .get(row_start..row_end)
                .ok_or(GeneratedAffineResidualGroupPhysicalKeyError::MalformedGeometry)?;
            if row.iter().all(Integer::is_zero) {
                let active = anchor_constants.values()[position] >= Integer::from(1);
                if authority.sector().active_bits().get(position).copied() != Some(active) {
                    return Err(GeneratedAffineResidualGroupPhysicalKeyError::MalformedGeometry);
                }
                constant_positions.push(position);
            } else {
                symbolic_positions.push(position);
            }
        }

        let prospective_frame_bytes = frame_retained_byte_bound(
            &case_ordinals,
            &anchor_offsets,
            &anchor_constants,
            &constant_positions,
            &symbolic_positions,
            &sector,
            None,
        )?;
        check_limit(
            "frame retained bytes",
            prospective_frame_bytes,
            limits.max_frame_retained_bytes,
        )?;

        let manifest_bytes =
            manifest_exact_bytes(authority.as_ref(), case, group, &anchor_constants, limits)?;
        let prospective_frame_with_manifest = checked_add(
            "frame retained bytes",
            prospective_frame_bytes,
            checked_add(
                "frame retained bytes",
                arc_payload_control_and_padding_byte_bound::<String>("frame retained bytes")?,
                manifest_bytes,
            )?,
        )?;
        check_limit(
            "frame retained bytes",
            prospective_frame_with_manifest,
            limits.max_frame_retained_bytes,
        )?;
        let manifest = render_manifest(
            authority.as_ref(),
            case,
            group,
            &anchor_constants,
            limits,
            manifest_bytes,
        )?;
        let frame_retained_bytes = frame_retained_byte_bound(
            &case_ordinals,
            &anchor_offsets,
            &anchor_constants,
            &constant_positions,
            &symbolic_positions,
            &sector,
            Some(&manifest),
        )?;
        check_limit(
            "frame retained bytes",
            frame_retained_bytes,
            limits.max_frame_retained_bytes,
        )?;
        let stats = GeneratedAffineResidualGroupPhysicalKeyStats {
            authority_replays: AUTHORITY_REPLAYS,
            case_view_resolutions: CASE_VIEW_RESOLUTIONS,
            group_view_resolutions: GROUP_VIEW_RESOLUTIONS,
            retained_authority_references: RETAINED_AUTHORITY_REFERENCES,
            group_cases,
            arity,
            free_positions,
            constant_positions: constant_positions.len(),
            symbolic_positions: symbolic_positions.len(),
            matrix_entries_inspected,
            offset_components,
            largest_geometry_integer_bits,
            geometry_integer_bit_work,
            retained_geometry_integer_bits,
            frame_retained_bytes,
            manifest_bytes,
        };
        Ok(Self {
            schema: GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_FRAME_V1_SCHEMA,
            source_case_ordinal: authority.case_ordinal(),
            source_ordinal_within_group: source_position,
            group_ordinal: authority.group_ordinal(),
            anchor_case_ordinal: group.anchor_case_ordinal(),
            policy: authority.ordering(),
            sector: Arc::new(sector),
            authority,
            case_ordinals: Arc::new(case_ordinals),
            anchor_offsets: Arc::new(anchor_offsets),
            anchor_constants,
            constant_positions: Arc::new(constant_positions),
            symbolic_positions: Arc::new(symbolic_positions),
            limits,
            stats,
            stable_manifest: Arc::new(manifest),
        })
    }

    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }
    pub(crate) const fn group_ordinal(&self) -> usize {
        self.group_ordinal
    }
    pub(crate) const fn anchor_case_ordinal(&self) -> usize {
        self.anchor_case_ordinal
    }
    pub(crate) fn arity(&self) -> usize {
        self.anchor_constants.arity()
    }
    pub(crate) fn case_ordinals(&self) -> &[usize] {
        self.case_ordinals.as_slice()
    }
    pub(crate) fn anchor_constants(&self) -> &GeneratedAffineResidualGroupLatticeShift {
        &self.anchor_constants
    }
    pub(crate) fn constant_positions(&self) -> &[usize] {
        self.constant_positions.as_slice()
    }
    pub(crate) fn symbolic_positions(&self) -> &[usize] {
        self.symbolic_positions.as_slice()
    }
    pub(crate) const fn limits(&self) -> GeneratedAffineResidualGroupPhysicalKeyLimits {
        self.limits
    }
    pub(crate) const fn stats(&self) -> GeneratedAffineResidualGroupPhysicalKeyStats {
        self.stats
    }
    pub(crate) fn stable_manifest(&self) -> &str {
        self.stable_manifest.as_str()
    }

    /// Allocation-identity check used only to deduplicate retained source
    /// graphs against the frame/solve-plan anchor authority. Sharing the same
    /// inventory is not sufficient: non-anchor case authorities are distinct
    /// allocations and remain chargeable.
    pub(crate) fn same_authority_allocation(
        &self,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
    ) -> bool {
        Arc::ptr_eq(&self.authority, authority)
    }

    pub(crate) fn anchor_offset(
        &self,
        ordinal_within_group: usize,
        case_ordinal: usize,
    ) -> Result<
        &GeneratedAffineResidualGroupLatticeShift,
        GeneratedAffineResidualGroupPhysicalKeyError,
    > {
        if self.case_ordinals.get(ordinal_within_group).copied() != Some(case_ordinal) {
            return Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongCasePosition);
        }
        self.anchor_offsets
            .get(ordinal_within_group)
            .ok_or(GeneratedAffineResidualGroupPhysicalKeyError::WrongCasePosition)
    }

    /// Census `key(o_u + q)` without constructing `o_u + q` or cloning any
    /// GMP integer.
    ///
    /// Every bound is prospective and may overestimate cancellation in the
    /// exact additions. The method performs borrowed magnitude scans and
    /// scalar arithmetic only; in particular it creates no `Integer`, `Vec`,
    /// `Arc`, physical shift, or physical-key admission token.
    pub(crate) fn preflight_key_for_local(
        &self,
        ordinal_within_group: usize,
        case_ordinal: usize,
        local: &IndexShift,
    ) -> Result<
        GeneratedAffineResidualGroupLocalPhysicalKeyPreflightCensus,
        GeneratedAffineResidualGroupPhysicalKeyError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            if local.arity() != self.arity() {
                return Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongArity {
                    expected: self.arity(),
                    actual: local.arity(),
                });
            }
            let offset = self.anchor_offset(ordinal_within_group, case_ordinal)?;
            let mut prospective_shift_bits = 0usize;
            let mut prospective_shift_bytes =
                prospective_arc_vec_bytes::<Integer>("lattice-shift retained bytes", self.arity())?;
            let mut prospective_total_bits = 0usize;
            let mut prospective_excess_heap_bytes = 0usize;
            let mut largest_excess_bits = 0usize;
            let mut integer_bit_work = 0usize;
            let mut comparison_integer_bit_work = 0usize;
            let mut constant_cursor = 0usize;
            for position in 0..self.arity() {
                let offset_bits = checked_integer_bits(
                    "physical shift integer bits",
                    &offset.values()[position],
                    self.limits.max_shift_integer_bits,
                )?;
                let local_bits = i64_bits(local.values()[position]);
                let shift_bits = prospective_add_bits(offset_bits, local_bits)?;
                check_limit(
                    "physical shift integer bits",
                    shift_bits,
                    self.limits.max_shift_integer_bits,
                )?;
                check_limit(
                    "physical key integer bits",
                    shift_bits,
                    self.limits.max_key_integer_bits,
                )?;
                prospective_shift_bits = bounded_add(
                    "lattice-shift total integer bits",
                    prospective_shift_bits,
                    shift_bits,
                    self.limits.max_shift_total_integer_bits,
                )?;
                prospective_shift_bytes = bounded_add(
                    "lattice-shift retained bytes",
                    prospective_shift_bytes,
                    prospective_integer_heap_bytes(shift_bits)?,
                    self.limits.max_shift_retained_bytes,
                )?;
                integer_bit_work = checked_add(
                    "local physical-key integer-bit work",
                    integer_bit_work,
                    checked_add(
                        "local physical-key integer-bit work",
                        offset_bits.max(1),
                        checked_add(
                            "local physical-key integer-bit work",
                            local_bits.max(1),
                            shift_bits.max(1),
                        )?,
                    )?,
                )?;
                comparison_integer_bit_work = checked_add(
                    "physical-key comparison integer-bit work",
                    comparison_integer_bit_work,
                    shift_bits.max(1),
                )?;

                let is_constant =
                    self.constant_positions.get(constant_cursor).copied() == Some(position);
                constant_cursor += usize::from(is_constant);
                let excess_bits = if is_constant {
                    let anchor_bits = checked_integer_bits(
                        "physical key integer bits",
                        &self.anchor_constants.values()[position],
                        self.limits.max_key_integer_bits,
                    )?;
                    integer_bit_work = checked_add(
                        "local physical-key integer-bit work",
                        integer_bit_work,
                        anchor_bits.max(1),
                    )?;
                    checked_add(
                        "physical key integer bits",
                        prospective_add_bits(anchor_bits, shift_bits)?,
                        1,
                    )?
                } else {
                    shift_bits
                };
                check_limit(
                    "physical key integer bits",
                    excess_bits,
                    self.limits.max_key_integer_bits,
                )?;
                integer_bit_work = checked_add(
                    "local physical-key integer-bit work",
                    integer_bit_work,
                    excess_bits.max(1),
                )?;
                comparison_integer_bit_work = checked_add(
                    "physical-key comparison integer-bit work",
                    comparison_integer_bit_work,
                    excess_bits.max(1),
                )?;
                prospective_total_bits = bounded_add(
                    "physical key total integer bits",
                    prospective_total_bits,
                    excess_bits,
                    self.limits.max_key_total_integer_bits,
                )?;
                largest_excess_bits = largest_excess_bits.max(excess_bits);
                prospective_excess_heap_bytes = checked_add(
                    "physical key retained bytes",
                    prospective_excess_heap_bytes,
                    prospective_integer_heap_bytes(excess_bits)?,
                )?;
            }
            prospective_total_bits = bounded_add(
                "physical key total integer bits",
                prospective_total_bits,
                prospective_shift_bits,
                self.limits.max_key_total_integer_bits,
            )?;
            let one_total_allowance = if largest_excess_bits == 0 {
                0
            } else {
                checked_add(
                    "physical key total integer bits",
                    largest_excess_bits,
                    ceil_log2(self.arity()),
                )?
            };
            check_limit(
                "physical key integer bits",
                one_total_allowance,
                self.limits.max_key_integer_bits,
            )?;
            let totals_allowance =
                checked_mul("physical key total integer bits", 3, one_total_allowance)?;
            let prospective_retained_integer_bits = bounded_add(
                "physical key total integer bits",
                prospective_total_bits,
                totals_allowance,
                self.limits.max_key_total_integer_bits,
            )?;
            integer_bit_work = checked_add(
                "local physical-key integer-bit work",
                integer_bit_work,
                checked_mul(
                    "local physical-key integer-bit work",
                    self.arity(),
                    checked_mul(
                        "local physical-key integer-bit work",
                        4,
                        one_total_allowance.max(1),
                    )?,
                )?,
            )?;
            comparison_integer_bit_work = checked_add(
                "physical-key comparison integer-bit work",
                comparison_integer_bit_work,
                checked_mul(
                    "physical-key comparison integer-bit work",
                    3,
                    one_total_allowance.max(1),
                )?,
            )?;
            let prospective_retained_bytes = prospective_key_retained_bytes_from_shift_bound(
                self.arity(),
                prospective_excess_heap_bytes,
                one_total_allowance,
                prospective_shift_bytes,
            )?;
            check_limit(
                "physical key retained bytes",
                prospective_retained_bytes,
                self.limits.max_key_retained_bytes,
            )?;
            Ok(
                GeneratedAffineResidualGroupLocalPhysicalKeyPreflightCensus {
                    component_scans: self.arity(),
                    integer_bit_work,
                    prospective_retained_integer_bits,
                    prospective_retained_bytes,
                    prospective_comparison_integer_bit_work: comparison_integer_bit_work,
                },
            )
        }))
        .map_err(|_| GeneratedAffineResidualGroupPhysicalKeyError::SymbolicaPanic)?
    }

    /// Convert one compact local generated-row key to the common physical key
    /// `o_u + q` without any `i64` arithmetic.
    pub(crate) fn physical_from_local(
        &self,
        ordinal_within_group: usize,
        case_ordinal: usize,
        local: &IndexShift,
    ) -> Result<
        GeneratedAffineResidualGroupLatticeShift,
        GeneratedAffineResidualGroupPhysicalKeyError,
    > {
        #[cfg(test)]
        PHYSICAL_FROM_LOCAL_EXECUTIONS.with(|count| count.set(count.get().saturating_add(1)));
        catch_unwind(AssertUnwindSafe(|| {
            if local.arity() != self.arity() {
                return Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongArity {
                    expected: self.arity(),
                    actual: local.arity(),
                });
            }
            let offset = self.anchor_offset(ordinal_within_group, case_ordinal)?;
            add_i64_vector(
                offset.values(),
                local.values(),
                self.limits.max_shift_integer_bits,
                self.limits.max_shift_total_integer_bits,
                self.limits.max_shift_retained_bytes,
            )
        }))
        .map_err(|_| GeneratedAffineResidualGroupPhysicalKeyError::SymbolicaPanic)?
    }

    /// Convert one physical group pivot to its exact local coordinate
    /// `r - o_u`.  The result intentionally remains arbitrary precision.
    pub(crate) fn local_from_physical(
        &self,
        ordinal_within_group: usize,
        case_ordinal: usize,
        physical: &GeneratedAffineResidualGroupLatticeShift,
    ) -> Result<
        GeneratedAffineResidualGroupLatticeShift,
        GeneratedAffineResidualGroupPhysicalKeyError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            if physical.arity() != self.arity() {
                return Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongArity {
                    expected: self.arity(),
                    actual: physical.arity(),
                });
            }
            let offset = self.anchor_offset(ordinal_within_group, case_ordinal)?;
            subtract_borrowed_vectors(
                physical.values(),
                offset.values(),
                self.arity(),
                self.limits.max_shift_integer_bits,
                self.limits.max_shift_total_integer_bits,
                self.limits.max_shift_retained_bytes,
            )
        }))
        .map_err(|_| GeneratedAffineResidualGroupPhysicalKeyError::SymbolicaPanic)?
    }

    pub(crate) fn key_for_physical(
        &self,
        physical: &GeneratedAffineResidualGroupLatticeShift,
    ) -> Result<GeneratedAffineResidualGroupPhysicalKey, GeneratedAffineResidualGroupPhysicalKeyError>
    {
        catch_unwind(AssertUnwindSafe(|| {
            let census = self.preflight_key_for_physical_inner(physical)?;
            self.execute_preflighted_key(physical.clone(), census)
        }))
        .map_err(|_| GeneratedAffineResidualGroupPhysicalKeyError::SymbolicaPanic)?
    }

    /// Test-only ingress for exercising downstream exact-key owners with
    /// arbitrary borrowed Symbolica integers. Construction remains behind the
    /// canonical shift boundary and the resulting key is admitted and
    /// consumed by this exact outer frame allocation.
    #[cfg(test)]
    pub(crate) fn test_key_for_borrowed_physical_values(
        self: &Arc<Self>,
        values: &[Integer],
    ) -> Result<GeneratedAffineResidualGroupPhysicalKey, GeneratedAffineResidualGroupPhysicalKeyError>
    {
        let physical = catch_unwind(AssertUnwindSafe(|| {
            make_shift_from_borrowed(
                values,
                self.arity(),
                self.limits.max_shift_integer_bits,
                self.limits.max_shift_total_integer_bits,
                self.limits.max_shift_retained_bytes,
            )
        }))
        .map_err(|_| GeneratedAffineResidualGroupPhysicalKeyError::SymbolicaPanic)??;
        let preflight = self.preflight_key_for_physical(&physical)?;
        self.key_for_preflight(preflight)
    }

    /// Preflight one exact key without cloning or performing arithmetic on any
    /// GMP integer.  The returned token retains the exact canonical shift and
    /// outer frame allocation privately; its exposed census contains only
    /// scalar resource counts, so physical coordinates never cross the API
    /// boundary.
    pub(crate) fn preflight_key_for_physical(
        self: &Arc<Self>,
        physical: &GeneratedAffineResidualGroupLatticeShift,
    ) -> Result<
        GeneratedAffineResidualGroupPhysicalKeyPreflight,
        GeneratedAffineResidualGroupPhysicalKeyError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            let census = self.preflight_key_for_physical_inner(physical)?;
            Ok(GeneratedAffineResidualGroupPhysicalKeyPreflight {
                frame: Arc::clone(self),
                physical: physical.clone(),
                census,
            })
        }))
        .map_err(|_| GeneratedAffineResidualGroupPhysicalKeyError::SymbolicaPanic)?
    }

    fn preflight_key_for_physical_inner(
        &self,
        physical: &GeneratedAffineResidualGroupLatticeShift,
    ) -> Result<
        GeneratedAffineResidualGroupPhysicalKeyPreflightCensus,
        GeneratedAffineResidualGroupPhysicalKeyError,
    > {
        if physical.arity() != self.arity() {
            return Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongArity {
                expected: self.arity(),
                actual: physical.arity(),
            });
        }
        check_limit(
            "physical shift integer bits",
            physical.retained_integer_bits(),
            self.limits.max_shift_total_integer_bits,
        )?;
        check_limit(
            "physical shift retained bytes",
            physical.retained_bytes(),
            self.limits.max_shift_retained_bytes,
        )?;

        let mut prospective_total_bits = physical.retained_integer_bits();
        let mut prospective_excess_heap_bytes = 0usize;
        let mut largest_excess_bits = 0usize;
        let mut integer_bit_work = 0usize;
        let mut constant_cursor = 0usize;
        for position in 0..self.arity() {
            let is_constant =
                self.constant_positions.get(constant_cursor).copied() == Some(position);
            constant_cursor += usize::from(is_constant);
            let shift_bits = checked_integer_bits(
                "physical shift integer bits",
                &physical.values()[position],
                self.limits.max_shift_integer_bits,
            )?;
            check_limit(
                "physical key integer bits",
                shift_bits,
                self.limits.max_key_integer_bits,
            )?;
            integer_bit_work = checked_add(
                "physical key integer-bit work",
                integer_bit_work,
                shift_bits.max(1),
            )?;
            let excess_bits = if is_constant {
                let anchor_bits = checked_integer_bits(
                    "physical key integer bits",
                    &self.anchor_constants.values()[position],
                    self.limits.max_key_integer_bits,
                )?;
                integer_bit_work = checked_add(
                    "physical key integer-bit work",
                    integer_bit_work,
                    anchor_bits.max(1),
                )?;
                checked_add(
                    "physical key integer bits",
                    prospective_add_bits(anchor_bits, shift_bits)?,
                    1,
                )?
            } else {
                shift_bits
            };
            check_limit(
                "physical key integer bits",
                excess_bits,
                self.limits.max_key_integer_bits,
            )?;
            integer_bit_work = checked_add(
                "physical key integer-bit work",
                integer_bit_work,
                excess_bits.max(1),
            )?;
            prospective_total_bits = bounded_add(
                "physical key total integer bits",
                prospective_total_bits,
                excess_bits,
                self.limits.max_key_total_integer_bits,
            )?;
            largest_excess_bits = largest_excess_bits.max(excess_bits);
            prospective_excess_heap_bytes = checked_add(
                "physical key retained bytes",
                prospective_excess_heap_bytes,
                prospective_integer_heap_bytes(excess_bits)?,
            )?;
        }
        let one_total_allowance = if largest_excess_bits == 0 {
            0
        } else {
            checked_add(
                "physical key total integer bits",
                largest_excess_bits,
                ceil_log2(self.arity()),
            )?
        };
        check_limit(
            "physical key integer bits",
            one_total_allowance,
            self.limits.max_key_integer_bits,
        )?;
        let totals_allowance =
            checked_mul("physical key total integer bits", 3, one_total_allowance)?;
        let prospective_retained_integer_bits = bounded_add(
            "physical key total integer bits",
            prospective_total_bits,
            totals_allowance,
            self.limits.max_key_total_integer_bits,
        )?;
        // Per component, execution may update the corner total and one of the
        // dot/numerator totals. Four total-width operations conservatively
        // cover those two additions plus canonicalization/comparison work.
        integer_bit_work = checked_add(
            "physical key integer-bit work",
            integer_bit_work,
            checked_mul(
                "physical key integer-bit work",
                self.arity(),
                checked_mul(
                    "physical key integer-bit work",
                    4,
                    one_total_allowance.max(1),
                )?,
            )?,
        )?;
        let prospective_retained_bytes = prospective_key_retained_bytes(
            self.arity(),
            prospective_excess_heap_bytes,
            one_total_allowance,
            physical,
        )?;
        check_limit(
            "physical key retained bytes",
            prospective_retained_bytes,
            self.limits.max_key_retained_bytes,
        )?;
        Ok(GeneratedAffineResidualGroupPhysicalKeyPreflightCensus {
            component_scans: self.arity(),
            integer_bit_work,
            prospective_retained_integer_bits,
            prospective_retained_bytes,
        })
    }

    /// Consume an admitted key construction.  The token owns the exact shift
    /// selected during preflight and is bound to this frame allocation, so a
    /// caller cannot substitute either operand between admission and GMP
    /// execution.
    pub(crate) fn key_for_preflight(
        self: &Arc<Self>,
        preflight: GeneratedAffineResidualGroupPhysicalKeyPreflight,
    ) -> Result<GeneratedAffineResidualGroupPhysicalKey, GeneratedAffineResidualGroupPhysicalKeyError>
    {
        catch_unwind(AssertUnwindSafe(|| self.key_for_preflight_inner(preflight)))
            .map_err(|_| GeneratedAffineResidualGroupPhysicalKeyError::SymbolicaPanic)?
    }

    fn key_for_preflight_inner(
        self: &Arc<Self>,
        preflight: GeneratedAffineResidualGroupPhysicalKeyPreflight,
    ) -> Result<GeneratedAffineResidualGroupPhysicalKey, GeneratedAffineResidualGroupPhysicalKeyError>
    {
        if !Arc::ptr_eq(self, &preflight.frame) {
            return Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongFrameAllocation);
        }
        self.execute_preflighted_key(preflight.physical, preflight.census)
    }

    fn execute_preflighted_key(
        &self,
        physical: GeneratedAffineResidualGroupLatticeShift,
        preflight: GeneratedAffineResidualGroupPhysicalKeyPreflightCensus,
    ) -> Result<GeneratedAffineResidualGroupPhysicalKey, GeneratedAffineResidualGroupPhysicalKeyError>
    {
        let physical = &physical;

        let mut bits = try_vec_with_capacity("physical key sector bits", self.arity())?;
        let mut excesses = try_vec_with_capacity("physical key signed excesses", self.arity())?;
        let mut propagators = 0usize;
        let mut corner = Integer::from(0);
        let mut dots = Integer::from(0);
        let mut numerators = Integer::from(0);
        let mut constant_cursor = 0usize;
        for position in 0..self.arity() {
            let is_constant =
                self.constant_positions.get(constant_cursor).copied() == Some(position);
            constant_cursor += usize::from(is_constant);
            let (active, excess) = if is_constant {
                let shifted =
                    &self.anchor_constants.values()[position] + &physical.values()[position];
                if shifted >= Integer::from(1) {
                    (true, shifted - Integer::from(1))
                } else {
                    (false, -shifted)
                }
            } else {
                let active = self.sector.active_bits()[position];
                (
                    active,
                    if active {
                        physical.values()[position].clone()
                    } else {
                        -physical.values()[position].clone()
                    },
                )
            };
            check_limit(
                "physical key integer bits",
                integer_bits(&excess)?,
                self.limits.max_key_integer_bits,
            )?;
            bits.push(active);
            corner += &excess;
            if active {
                propagators = checked_add("physical key propagators", propagators, 1)?;
                dots += &excess;
            } else {
                numerators += &excess;
            }
            excesses.push(canonical_integer(excess));
        }
        for total in [&corner, &dots, &numerators] {
            check_limit(
                "physical key integer bits",
                integer_bits(total)?,
                self.limits.max_key_integer_bits,
            )?;
        }
        let formal_sector = SectorMask::try_from_preallocated(bits)
            .map_err(|_| GeneratedAffineResidualGroupPhysicalKeyError::ReplayMismatch)?;
        let mut retained_integer_bits = physical.retained_integer_bits();
        for value in excesses.iter().chain([&corner, &dots, &numerators]) {
            retained_integer_bits = bounded_add(
                "physical key total integer bits",
                retained_integer_bits,
                integer_bits(value)?,
                self.limits.max_key_total_integer_bits,
            )?;
        }
        let retained_bytes = key_retained_byte_bound(
            &formal_sector,
            &excesses,
            [&corner, &dots, &numerators],
            physical,
        )?;
        check_limit(
            "physical key retained bytes",
            retained_bytes,
            self.limits.max_key_retained_bytes,
        )?;
        if retained_integer_bits > preflight.prospective_retained_integer_bits
            || retained_bytes > preflight.prospective_retained_bytes
        {
            return Err(GeneratedAffineResidualGroupPhysicalKeyError::ReplayMismatch);
        }
        Ok(GeneratedAffineResidualGroupPhysicalKey {
            schema: GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_KEY_V1_SCHEMA,
            policy: self.policy,
            arity: self.arity(),
            propagators,
            formal_sector: Arc::new(formal_sector),
            corner_distance_offset: Arc::new(canonical_integer(corner)),
            dots_offset: Arc::new(canonical_integer(dots)),
            numerators_offset: Arc::new(canonical_integer(numerators)),
            signed_index_excess: Arc::new(excesses),
            shift: physical.clone(),
            retained_integer_bits,
            retained_bytes,
        })
    }

    pub(crate) fn replay_key(
        &self,
        key: &GeneratedAffineResidualGroupPhysicalKey,
    ) -> Result<(), GeneratedAffineResidualGroupPhysicalKeyError> {
        if key.schema != GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_KEY_V1_SCHEMA {
            return Err(GeneratedAffineResidualGroupPhysicalKeyError::SchemaMismatch);
        }
        let replayed = self.key_for_physical(key.shift())?;
        if replayed == *key {
            Ok(())
        } else {
            Err(GeneratedAffineResidualGroupPhysicalKeyError::ReplayMismatch)
        }
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
    ) -> Result<(), GeneratedAffineResidualGroupPhysicalKeyError> {
        catch_unwind(AssertUnwindSafe(|| {
            if self.schema != GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_FRAME_V1_SCHEMA {
                return Err(GeneratedAffineResidualGroupPhysicalKeyError::SchemaMismatch);
            }
            if !Arc::ptr_eq(&self.authority, authority) {
                return Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongAuthorityAllocation);
            }
            let rebuilt =
                Self::try_new_unwind_boundary(family, context, Arc::clone(authority), self.limits)?;
            if rebuilt == *self {
                Ok(())
            } else {
                Err(GeneratedAffineResidualGroupPhysicalKeyError::ReplayMismatch)
            }
        }))
        .map_err(|_| GeneratedAffineResidualGroupPhysicalKeyError::SymbolicaPanic)?
    }

    /// Replay this frame while authenticating an arbitrary source case in
    /// the same retained inventory group.
    ///
    /// A frame is constructed from one exact authority allocation (normally
    /// the group anchor), whereas later exact rows can originate from any
    /// case in the group. Requiring the caller's authority `Arc` to equal the
    /// anchor would therefore reject valid non-anchor sources. This seam
    /// keeps the anchor authority private, checks exact inventory allocation
    /// identity and group membership, replays the source authority, and then
    /// rebuilds the frame against its own retained authority.
    pub(crate) fn replay_for_source_authority(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source: &Arc<GeneratedAffineResidualCaseAuthority>,
    ) -> Result<(), GeneratedAffineResidualGroupPhysicalKeyError> {
        catch_unwind(AssertUnwindSafe(|| {
            if !self.authority.same_inventory_allocation_as(source.as_ref()) {
                return Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongAuthorityAllocation);
            }
            source.replay(family, context)?;
            let source_case = source.authenticated_case_view(context)?;
            let source_group = source.authenticated_group_view(context)?;
            if source_case.ordinal() != source.case_ordinal()
                || source_case.group_ordinal() != self.group_ordinal
                || source_group.ordinal() != self.group_ordinal
                || source_group.ambient_arity() != self.arity()
                || self
                    .case_ordinals
                    .get(source_case.ordinal_within_group())
                    .copied()
                    != Some(source_case.ordinal())
            {
                return Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongGroup);
            }
            self.replay(family, context, &self.authority)
        }))
        .map_err(|_| GeneratedAffineResidualGroupPhysicalKeyError::SymbolicaPanic)?
    }
}

fn validate_group_binding(
    authority: &GeneratedAffineResidualCaseAuthority,
    case: GeneratedAffineResidualInventoryCaseSourceRecordView<'_>,
    group: GeneratedAffineResidualInventoryGroupSourceView<'_>,
) -> Result<(), GeneratedAffineResidualGroupPhysicalKeyError> {
    if case.ordinal() != authority.case_ordinal() {
        return Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongCase);
    }
    if case.group_ordinal() != authority.group_ordinal()
        || group.ordinal() != authority.group_ordinal()
    {
        return Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongGroup);
    }
    if group
        .case_ordinals()
        .get(case.ordinal_within_group())
        .copied()
        != Some(case.ordinal())
    {
        return Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongCasePosition);
    }
    if group.ambient_arity() != authority.arity() || authority.sector().arity() != authority.arity()
    {
        return Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongArity {
            expected: authority.arity(),
            actual: group.ambient_arity(),
        });
    }
    Ok(())
}

fn make_shift_from_borrowed(
    values: &[Integer],
    expected_arity: usize,
    max_integer_bits: usize,
    max_total_integer_bits: usize,
    max_retained_bytes: usize,
) -> Result<GeneratedAffineResidualGroupLatticeShift, GeneratedAffineResidualGroupPhysicalKeyError>
{
    if values.len() != expected_arity {
        return Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongArity {
            expected: expected_arity,
            actual: values.len(),
        });
    }
    let mut retained_integer_bits = 0usize;
    let mut prospective_bytes =
        prospective_arc_vec_bytes::<Integer>("lattice-shift retained bytes", expected_arity)?;
    for value in values {
        let bits = checked_integer_bits("lattice-shift integer bits", value, max_integer_bits)?;
        retained_integer_bits = bounded_add(
            "lattice-shift total integer bits",
            retained_integer_bits,
            bits,
            max_total_integer_bits,
        )?;
        prospective_bytes = bounded_add(
            "lattice-shift retained bytes",
            prospective_bytes,
            prospective_integer_heap_bytes(bits)?,
            max_retained_bytes,
        )?;
    }
    check_limit(
        "lattice-shift retained bytes",
        prospective_bytes,
        max_retained_bytes,
    )?;
    let mut retained = try_vec_with_capacity("lattice-shift components", expected_arity)?;
    for value in values {
        retained.push(canonical_integer_from_borrowed(value));
    }
    finish_owned_shift(retained, retained_integer_bits, max_retained_bytes)
}

fn finish_owned_shift(
    mut values: Vec<Integer>,
    retained_integer_bits: usize,
    max_retained_bytes: usize,
) -> Result<GeneratedAffineResidualGroupLatticeShift, GeneratedAffineResidualGroupPhysicalKeyError>
{
    for value in &mut values {
        let owned = replace(value, Integer::from(0));
        *value = canonical_integer(owned);
    }
    let retained_bytes = integer_arc_vec_retained_bytes(&values)?;
    check_limit(
        "lattice-shift retained bytes",
        retained_bytes,
        max_retained_bytes,
    )?;
    Ok(GeneratedAffineResidualGroupLatticeShift {
        values: Arc::new(values),
        retained_integer_bits,
        retained_bytes,
    })
}

fn add_i64_vector(
    left: &[Integer],
    right: &[i64],
    max_integer_bits: usize,
    max_total_integer_bits: usize,
    max_retained_bytes: usize,
) -> Result<GeneratedAffineResidualGroupLatticeShift, GeneratedAffineResidualGroupPhysicalKeyError>
{
    if left.len() != right.len() {
        return Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongArity {
            expected: left.len(),
            actual: right.len(),
        });
    }
    let mut prospective_total_bits = 0usize;
    let mut prospective_bytes =
        prospective_arc_vec_bytes::<Integer>("lattice-shift retained bytes", left.len())?;
    for (lhs, rhs) in left.iter().zip(right) {
        let bits = prospective_add_bits(integer_bits(lhs)?, i64_bits(*rhs))?;
        check_limit("lattice-shift integer bits", bits, max_integer_bits)?;
        prospective_total_bits = bounded_add(
            "lattice-shift total integer bits",
            prospective_total_bits,
            bits,
            max_total_integer_bits,
        )?;
        prospective_bytes = bounded_add(
            "lattice-shift retained bytes",
            prospective_bytes,
            prospective_integer_heap_bytes(bits)?,
            max_retained_bytes,
        )?;
    }
    check_limit(
        "lattice-shift retained bytes",
        prospective_bytes,
        max_retained_bytes,
    )?;
    let mut values = try_vec_with_capacity("lattice-shift components", left.len())?;
    let mut actual_bits = 0usize;
    for (lhs, rhs) in left.iter().zip(right) {
        let value = canonical_integer(lhs + Integer::from(*rhs));
        actual_bits = bounded_add(
            "lattice-shift total integer bits",
            actual_bits,
            integer_bits(&value)?,
            max_total_integer_bits,
        )?;
        values.push(value);
    }
    finish_owned_shift(values, actual_bits, max_retained_bytes)
}

fn subtract_borrowed_vectors(
    left: &[Integer],
    right: &[Integer],
    expected_arity: usize,
    max_integer_bits: usize,
    max_total_integer_bits: usize,
    max_retained_bytes: usize,
) -> Result<GeneratedAffineResidualGroupLatticeShift, GeneratedAffineResidualGroupPhysicalKeyError>
{
    if left.len() != expected_arity || right.len() != expected_arity {
        return Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongArity {
            expected: expected_arity,
            actual: left.len().min(right.len()),
        });
    }
    let mut prospective_total_bits = 0usize;
    let mut prospective_bytes =
        prospective_arc_vec_bytes::<Integer>("lattice-shift retained bytes", expected_arity)?;
    for (lhs, rhs) in left.iter().zip(right) {
        let bits = prospective_add_bits(integer_bits(lhs)?, integer_bits(rhs)?)?;
        check_limit("lattice-shift integer bits", bits, max_integer_bits)?;
        prospective_total_bits = bounded_add(
            "lattice-shift total integer bits",
            prospective_total_bits,
            bits,
            max_total_integer_bits,
        )?;
        prospective_bytes = bounded_add(
            "lattice-shift retained bytes",
            prospective_bytes,
            prospective_integer_heap_bytes(bits)?,
            max_retained_bytes,
        )?;
    }
    check_limit(
        "lattice-shift retained bytes",
        prospective_bytes,
        max_retained_bytes,
    )?;
    let mut values = try_vec_with_capacity("lattice-shift components", expected_arity)?;
    let mut actual_bits = 0usize;
    for (lhs, rhs) in left.iter().zip(right) {
        let value = canonical_integer(lhs - rhs);
        actual_bits = bounded_add(
            "lattice-shift total integer bits",
            actual_bits,
            integer_bits(&value)?,
            max_total_integer_bits,
        )?;
        values.push(value);
    }
    finish_owned_shift(values, actual_bits, max_retained_bytes)
}

fn canonical_integer(value: Integer) -> Integer {
    match value {
        Integer::Single(value) => Integer::from(value),
        Integer::Double(value) => Integer::from(value),
        Integer::Large(value) => Integer::from(value),
    }
}

fn canonical_integer_from_borrowed(value: &Integer) -> Integer {
    match value {
        Integer::Single(value) => Integer::from(*value),
        Integer::Double(value) => Integer::from(*value),
        Integer::Large(value) => {
            if let Some(compact) = value.to_i64() {
                Integer::from(compact)
            } else if let Some(compact) = value.to_i128() {
                Integer::from(compact)
            } else {
                Integer::Large(value.clone())
            }
        }
    }
}

fn integer_to_i64(value: &Integer) -> Option<i64> {
    match value {
        Integer::Single(value) => Some(*value),
        Integer::Double(value) => i64::try_from(*value).ok(),
        Integer::Large(value) => value.to_i64(),
    }
}

fn prospective_frame_base_retained_bytes(
    arity: usize,
    group_cases: usize,
    offset_heap_bytes: usize,
    anchor_heap_bytes: usize,
) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
    let resource = "prospective frame retained bytes";
    let mut bytes = size_of::<GeneratedAffineResidualGroupPhysicalFrame>();
    bytes = checked_add(
        resource,
        bytes,
        prospective_arc_vec_bytes::<usize>(resource, group_cases)?,
    )?;
    bytes = checked_add(
        resource,
        bytes,
        prospective_arc_vec_bytes::<GeneratedAffineResidualGroupLatticeShift>(
            resource,
            group_cases,
        )?,
    )?;
    bytes = checked_add(
        resource,
        bytes,
        checked_mul(
            resource,
            group_cases,
            prospective_arc_vec_bytes::<Integer>(resource, arity)?,
        )?,
    )?;
    bytes = checked_add(resource, bytes, offset_heap_bytes)?;
    bytes = checked_add(
        resource,
        bytes,
        prospective_arc_vec_bytes::<Integer>(resource, arity)?,
    )?;
    bytes = checked_add(resource, bytes, anchor_heap_bytes)?;
    bytes = checked_add(
        resource,
        bytes,
        checked_mul(
            resource,
            2,
            prospective_arc_vec_bytes::<usize>(resource, arity)?,
        )?,
    )?;
    checked_add(
        resource,
        bytes,
        checked_add(
            resource,
            arc_payload_control_and_padding_byte_bound::<SectorMask>(resource)?,
            prospective_sector_owned_retained_bytes(resource, arity)?,
        )?,
    )
}

fn frame_retained_byte_bound(
    case_ordinals: &Vec<usize>,
    offsets: &Vec<GeneratedAffineResidualGroupLatticeShift>,
    anchor_constants: &GeneratedAffineResidualGroupLatticeShift,
    constant_positions: &Vec<usize>,
    symbolic_positions: &Vec<usize>,
    sector: &SectorMask,
    manifest: Option<&String>,
) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
    let fixed = size_of::<GeneratedAffineResidualGroupPhysicalFrame>();
    let mut bytes = checked_add(
        "frame retained bytes",
        fixed,
        arc_vec_owned_byte_bound("frame retained bytes", case_ordinals)?,
    )?;
    bytes = checked_add(
        "frame retained bytes",
        bytes,
        arc_vec_owned_byte_bound("frame retained bytes", offsets)?,
    )?;
    for offset in offsets {
        bytes = checked_add("frame retained bytes", bytes, offset.retained_bytes())?;
    }
    bytes = checked_add(
        "frame retained bytes",
        bytes,
        anchor_constants.retained_bytes(),
    )?;
    for positions in [constant_positions, symbolic_positions] {
        bytes = checked_add(
            "frame retained bytes",
            bytes,
            arc_vec_owned_byte_bound("frame retained bytes", positions)?,
        )?;
    }
    bytes = checked_add(
        "frame retained bytes",
        bytes,
        checked_add(
            "frame retained bytes",
            arc_payload_control_and_padding_byte_bound::<SectorMask>("frame retained bytes")?,
            sector_owned_retained_bytes("frame retained bytes", sector)?,
        )?,
    )?;
    if let Some(manifest) = manifest {
        bytes = checked_add(
            "frame retained bytes",
            bytes,
            checked_add(
                "frame retained bytes",
                arc_payload_control_and_padding_byte_bound::<String>("frame retained bytes")?,
                manifest.capacity(),
            )?,
        )?;
    }
    Ok(bytes)
}

fn key_retained_byte_bound(
    sector: &SectorMask,
    excesses: &Vec<Integer>,
    totals: [&Integer; 3],
    shift: &GeneratedAffineResidualGroupLatticeShift,
) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
    let mut bytes = size_of::<GeneratedAffineResidualGroupPhysicalKey>();
    bytes = checked_add(
        "physical key retained bytes",
        bytes,
        checked_add(
            "physical key retained bytes",
            arc_payload_control_and_padding_byte_bound::<SectorMask>(
                "physical key retained bytes",
            )?,
            sector_owned_retained_bytes("physical key retained bytes", sector)?,
        )?,
    )?;
    bytes = checked_add(
        "physical key retained bytes",
        bytes,
        integer_arc_vec_retained_bytes(excesses)?,
    )?;
    for value in totals {
        bytes = checked_add(
            "physical key retained bytes",
            bytes,
            checked_add(
                "physical key retained bytes",
                arc_payload_control_and_padding_byte_bound::<Integer>(
                    "physical key retained bytes",
                )?,
                integer_owned_heap_bytes(value)?,
            )?,
        )?;
    }
    checked_add("physical key retained bytes", bytes, shift.retained_bytes())
}

fn integer_arc_vec_retained_bytes(
    values: &Vec<Integer>,
) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
    let mut bytes = arc_vec_owned_byte_bound("integer vector retained bytes", values)?;
    for value in values {
        bytes = checked_add(
            "integer vector retained bytes",
            bytes,
            integer_owned_heap_bytes(value)?,
        )?;
    }
    Ok(bytes)
}

fn prospective_key_retained_bytes(
    arity: usize,
    excess_heap_bytes: usize,
    total_integer_bits: usize,
    shift: &GeneratedAffineResidualGroupLatticeShift,
) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
    prospective_key_retained_bytes_from_shift_bound(
        arity,
        excess_heap_bytes,
        total_integer_bits,
        shift.retained_bytes(),
    )
}

fn prospective_key_retained_bytes_from_shift_bound(
    arity: usize,
    excess_heap_bytes: usize,
    total_integer_bits: usize,
    shift_retained_bytes: usize,
) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
    let resource = "physical key retained bytes";
    let mut bytes = size_of::<GeneratedAffineResidualGroupPhysicalKey>();
    bytes = checked_add(
        resource,
        bytes,
        checked_add(
            resource,
            arc_payload_control_and_padding_byte_bound::<SectorMask>(resource)?,
            prospective_sector_owned_retained_bytes(resource, arity)?,
        )?,
    )?;
    bytes = checked_add(
        resource,
        bytes,
        prospective_arc_vec_bytes::<Integer>(resource, arity)?,
    )?;
    bytes = checked_add(resource, bytes, excess_heap_bytes)?;
    let one_total = checked_add(
        resource,
        arc_payload_control_and_padding_byte_bound::<Integer>(resource)?,
        prospective_integer_heap_bytes(total_integer_bits)?,
    )?;
    bytes = checked_add(resource, bytes, checked_mul(resource, 3, one_total)?)?;
    checked_add(resource, bytes, shift_retained_bytes)
}

fn arc_payload_control_and_padding_byte_bound<T>(
    resource: &'static str,
) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
    // Logical retained-payload estimate: two ownership counters, alignment,
    // and the value.  Rust deliberately exposes neither Arc's allocation
    // layout nor allocator metadata, so this is not a peak-allocation claim.
    checked_add(
        resource,
        checked_mul(resource, 2, size_of::<AtomicUsize>())?,
        checked_add(resource, align_of::<T>().saturating_sub(1), size_of::<T>())?,
    )
}

fn prospective_arc_vec_bytes<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
    checked_add(
        resource,
        arc_payload_control_and_padding_byte_bound::<Vec<T>>(resource)?,
        checked_mul(resource, capacity, size_of::<T>())?,
    )
}

fn arc_vec_owned_byte_bound<T>(
    resource: &'static str,
    values: &Vec<T>,
) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
    prospective_arc_vec_bytes::<T>(resource, values.capacity())
}

fn prospective_sector_owned_retained_bytes(
    resource: &'static str,
    arity: usize,
) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
    let word_bits = usize::BITS as usize;
    let rounded_bits = checked_mul(
        resource,
        checked_add(resource, arity, word_bits - 1)? / word_bits,
        word_bits,
    )?;
    checked_mul(resource, rounded_bits, size_of::<bool>())
}

fn sector_owned_retained_bytes(
    resource: &'static str,
    sector: &SectorMask,
) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
    sector
        .owned_retained_byte_bound()
        .ok_or(GeneratedAffineResidualGroupPhysicalKeyError::ResourceCountOverflow { resource })
}

fn integer_owned_heap_bytes(
    value: &Integer,
) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
    match value {
        Integer::Single(_) | Integer::Double(_) => Ok(0),
        Integer::Large(value) => value.capacity().checked_add(7).map(|bits| bits / 8).ok_or(
            GeneratedAffineResidualGroupPhysicalKeyError::ResourceCountOverflow {
                resource: "integer owned heap bytes",
            },
        ),
    }
}

fn prospective_integer_heap_bytes(
    bits: usize,
) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
    if bits <= i128::BITS as usize - 1 {
        Ok(0)
    } else {
        let rounded = checked_add("prospective integer heap bytes", bits, 191)? / 64;
        checked_mul("prospective integer heap bytes", rounded, size_of::<u64>())
    }
}

fn integer_bits(value: &Integer) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
    integer_magnitude_bits(value).map_err(|_| {
        GeneratedAffineResidualGroupPhysicalKeyError::ResourceCountOverflow {
            resource: "integer magnitude bits",
        }
    })
}

fn integer_field_comparison_bit_work<'a>(
    values: impl IntoIterator<Item = &'a Integer>,
) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
    values.into_iter().try_fold(0usize, |total, value| {
        checked_add(
            "physical-key comparison integer-bit work",
            total,
            integer_bits(value)?.max(1),
        )
    })
}

#[cfg(test)]
pub(crate) fn test_integer_field_comparison_bit_work(
    left: &[Integer],
    right: &[Integer],
) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
    integer_field_comparison_bit_work(left.iter().chain(right))
}

fn checked_integer_bits(
    resource: &'static str,
    value: &Integer,
    limit: usize,
) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
    let bits = integer_bits(value)?;
    check_limit(resource, bits, limit)?;
    Ok(bits)
}

fn prospective_add_bits(
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
    if left == 0 {
        Ok(right)
    } else if right == 0 {
        Ok(left)
    } else {
        checked_add("prospective integer addition bits", left.max(right), 1)
    }
}

fn i64_bits(value: i64) -> usize {
    (i64::BITS - value.unsigned_abs().leading_zeros()) as usize
}

fn ceil_log2(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::BITS as usize - (value - 1).leading_zeros() as usize
    }
}

fn manifest_exact_bytes(
    authority: &GeneratedAffineResidualCaseAuthority,
    case: GeneratedAffineResidualInventoryCaseSourceRecordView<'_>,
    group: GeneratedAffineResidualInventoryGroupSourceView<'_>,
    anchor_constants: &GeneratedAffineResidualGroupLatticeShift,
    limits: GeneratedAffineResidualGroupPhysicalKeyLimits,
) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
    let mut counter = CountingWriter::default();
    write_manifest_with_integer_writer(
        &mut counter,
        authority,
        case,
        group,
        anchor_constants,
        limits,
        &mut |output, value| {
            let Some(bytes) = identity_integer_bytes(value) else {
                output.overflowed = true;
                return Err(fmt::Error);
            };
            output.add_exact(bytes)
        },
    )
    .map_err(
        |_| GeneratedAffineResidualGroupPhysicalKeyError::ResourceCountOverflow {
            resource: "manifest bytes",
        },
    )?;
    let exact = counter.finish()?;
    check_limit("manifest bytes", exact, limits.max_manifest_bytes)?;
    Ok(exact)
}

fn render_manifest(
    authority: &GeneratedAffineResidualCaseAuthority,
    case: GeneratedAffineResidualInventoryCaseSourceRecordView<'_>,
    group: GeneratedAffineResidualInventoryGroupSourceView<'_>,
    anchor_constants: &GeneratedAffineResidualGroupLatticeShift,
    limits: GeneratedAffineResidualGroupPhysicalKeyLimits,
    exact: usize,
) -> Result<String, GeneratedAffineResidualGroupPhysicalKeyError> {
    let mut output = String::new();
    output.try_reserve_exact(exact).map_err(|_| {
        GeneratedAffineResidualGroupPhysicalKeyError::AllocationFailure {
            resource: "manifest bytes",
        }
    })?;
    write_manifest_with_integer_writer(
        &mut output,
        authority,
        case,
        group,
        anchor_constants,
        limits,
        &mut |output, value| write_identity_integer(output, value),
    )
    .map_err(
        |_| GeneratedAffineResidualGroupPhysicalKeyError::AllocationFailure {
            resource: "manifest bytes",
        },
    )?;
    if output.len() != exact {
        return Err(GeneratedAffineResidualGroupPhysicalKeyError::ReplayMismatch);
    }
    Ok(output)
}

fn write_manifest_with_integer_writer<W, F>(
    output: &mut W,
    authority: &GeneratedAffineResidualCaseAuthority,
    case: GeneratedAffineResidualInventoryCaseSourceRecordView<'_>,
    group: GeneratedAffineResidualInventoryGroupSourceView<'_>,
    anchor_constants: &GeneratedAffineResidualGroupLatticeShift,
    limits: GeneratedAffineResidualGroupPhysicalKeyLimits,
    write_integer: &mut F,
) -> fmt::Result
where
    W: fmt::Write,
    F: FnMut(&mut W, &Integer) -> fmt::Result,
{
    write!(
        output,
        "{GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_FRAME_V1_SCHEMA}|integer-encoding=sign-magnitude-hex-v1|family-bytes={}:{}|context-bytes={}:{}|policy={}|sector={}|source-case={}|source-within-group={}|group={}|anchor-case={}|arity={}|cases=[",
        authority.family_fingerprint().len(),
        authority.family_fingerprint(),
        authority.context_fingerprint().len(),
        authority.context_fingerprint(),
        authority.ordering().stable_id(),
        authority.sector(),
        case.ordinal(),
        case.ordinal_within_group(),
        group.ordinal(),
        group.anchor_case_ordinal(),
        authority.arity(),
    )?;
    write_usizes(output, group.case_ordinals())?;
    output.write_str("]|anchor-b=[")?;
    write_integers_with(output, anchor_constants.values(), write_integer)?;
    output.write_str("]|offsets=[")?;
    for (position, offset) in group.anchor_offsets().iter().enumerate() {
        if position != 0 {
            output.write_char(';')?;
        }
        output.write_char('[')?;
        write_integers_with(output, offset, write_integer)?;
        output.write_char(']')?;
    }
    output.write_str("]|free=[")?;
    write_usizes(output, group.free_positions())?;
    output.write_str("]|A=[")?;
    write_integers_with(output, group.compact_linear_coefficients(), write_integer)?;
    write!(
        output,
        "]|limits={},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        limits.max_authority_replays,
        limits.max_case_view_resolutions,
        limits.max_group_view_resolutions,
        limits.max_retained_authority_references,
        limits.max_group_cases,
        limits.max_arity,
        limits.max_free_positions,
        limits.max_matrix_entries_inspected,
        limits.max_offset_components,
        limits.max_geometry_integer_bits,
        limits.max_geometry_integer_bit_work,
        limits.max_geometry_total_integer_bits,
        limits.max_frame_retained_bytes,
        limits.max_manifest_bytes,
        limits.max_shift_integer_bits,
        limits.max_shift_total_integer_bits,
        limits.max_shift_retained_bytes,
        limits.max_key_integer_bits,
        limits.max_key_total_integer_bits,
    )?;
    write!(output, ",{}", limits.max_key_retained_bytes)
}

fn write_usizes(output: &mut impl fmt::Write, values: &[usize]) -> fmt::Result {
    for (position, value) in values.iter().enumerate() {
        if position != 0 {
            output.write_char(',')?;
        }
        write!(output, "{value}")?;
    }
    Ok(())
}

fn write_integers_with<W, F>(
    output: &mut W,
    values: &[Integer],
    write_integer: &mut F,
) -> fmt::Result
where
    W: fmt::Write,
    F: FnMut(&mut W, &Integer) -> fmt::Result,
{
    for (position, value) in values.iter().enumerate() {
        if position != 0 {
            output.write_char(',')?;
        }
        write_integer(output, value)?;
    }
    Ok(())
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

fn identity_integer_bytes(value: &Integer) -> Option<usize> {
    let bits = integer_bits(value).ok()?;
    let digits = if bits == 0 {
        1
    } else {
        bits.checked_add(3)? / 4
    };
    digits.checked_add(usize::from(value.is_negative()))
}

#[derive(Default)]
struct CountingWriter {
    bytes: usize,
    overflowed: bool,
}

impl CountingWriter {
    fn add_exact(&mut self, bytes: usize) -> fmt::Result {
        if let Some(total) = self.bytes.checked_add(bytes) {
            self.bytes = total;
            Ok(())
        } else {
            self.overflowed = true;
            Err(fmt::Error)
        }
    }

    fn finish(self) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
        if self.overflowed {
            Err(
                GeneratedAffineResidualGroupPhysicalKeyError::ResourceCountOverflow {
                    resource: "manifest bytes",
                },
            )
        } else {
            Ok(self.bytes)
        }
    }
}

impl fmt::Write for CountingWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if let Some(bytes) = self.bytes.checked_add(value.len()) {
            self.bytes = bytes;
            Ok(())
        } else {
            self.overflowed = true;
            Err(fmt::Error)
        }
    }
}

fn try_vec_with_capacity<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, GeneratedAffineResidualGroupPhysicalKeyError> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| {
        GeneratedAffineResidualGroupPhysicalKeyError::AllocationFailure { resource }
    })?;
    Ok(values)
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
    left.checked_add(right)
        .ok_or(GeneratedAffineResidualGroupPhysicalKeyError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
    left.checked_mul(right)
        .ok_or(GeneratedAffineResidualGroupPhysicalKeyError::ResourceCountOverflow { resource })
}

fn bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, GeneratedAffineResidualGroupPhysicalKeyError> {
    let requested = checked_add(resource, left, right)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualGroupPhysicalKeyError> {
    if requested > limit {
        Err(
            GeneratedAffineResidualGroupPhysicalKeyError::ResourceLimit {
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
    use std::collections::{BTreeSet, HashSet};
    use std::sync::{Arc, Weak};

    use symbolica::domains::integer::MultiPrecisionInteger;

    use super::*;
    use crate::generated_affine_parametric_ordering::{
        GeneratedAffineParametricOrderingCertificate, GeneratedAffineParametricOrderingLimits,
    };
    use crate::generated_affine_residual_boolean_cover::{
        GeneratedAffineResidualBooleanCoverCompiler, GeneratedAffineResidualBooleanCoverLimits,
    };
    use crate::generated_affine_residual_case_inventory::{
        GeneratedAffineResidualCaseAuthorityLimits,
        GeneratedAffineResidualCaseInventoryCertificate,
        GeneratedAffineResidualCaseInventoryCompiler, GeneratedAffineResidualCaseInventoryLimits,
    };
    use crate::generated_affine_residual_source_authority::GeneratedAffineResidualSourceAuthority;
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
        let group_ordinal = (0..inventory.group_count())
            .max_by_key(|&ordinal| {
                inventory
                    .authenticated_group_view(&context, ordinal)
                    .unwrap()
                    .case_ordinals()
                    .len()
            })
            .unwrap();
        let group = inventory
            .authenticated_group_view(&context, group_ordinal)
            .unwrap();
        assert!(group.case_ordinals().len() > 1);
        let anchor_case_ordinal = group.anchor_case_ordinal();
        let authority = Arc::new(
            GeneratedAffineResidualCaseAuthority::try_new(
                &family,
                &context,
                Arc::clone(&inventory),
                anchor_case_ordinal,
                GeneratedAffineResidualCaseAuthorityLimits::default(),
            )
            .unwrap(),
        );
        (family, context, inventory, authority)
    }

    fn exact_frame_limits(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
    ) -> (
        GeneratedAffineResidualGroupPhysicalKeyLimits,
        GeneratedAffineResidualGroupPhysicalKeyStats,
    ) {
        let mut limits = GeneratedAffineResidualGroupPhysicalKeyLimits::default();
        for _ in 0..16 {
            let frame = GeneratedAffineResidualGroupPhysicalFrame::try_new(
                family,
                context,
                Arc::clone(authority),
                limits,
            )
            .unwrap();
            let stats = frame.stats();
            let mut next = limits;
            next.max_authority_replays = stats.authority_replays();
            next.max_case_view_resolutions = stats.case_view_resolutions();
            next.max_group_view_resolutions = stats.group_view_resolutions();
            next.max_retained_authority_references = stats.retained_authority_references();
            next.max_group_cases = stats.group_cases();
            next.max_arity = stats.arity();
            next.max_free_positions = stats.free_positions();
            next.max_matrix_entries_inspected = stats.matrix_entries_inspected();
            next.max_offset_components = stats.offset_components();
            next.max_geometry_integer_bits = stats.largest_geometry_integer_bits();
            next.max_geometry_integer_bit_work = stats.geometry_integer_bit_work();
            next.max_geometry_total_integer_bits = stats.retained_geometry_integer_bits();
            next.max_frame_retained_bytes = stats.frame_retained_bytes();
            next.max_manifest_bytes = stats.manifest_bytes();
            if next == limits {
                return (limits, stats);
            }
            limits = next;
        }
        panic!("exact group-frame manifest limit did not converge")
    }

    fn raw_large(value: i128) -> Integer {
        Integer::Large(MultiPrecisionInteger::from(value))
    }

    fn minimum_admitted_key_limit(
        frame: &GeneratedAffineResidualGroupPhysicalFrame,
        physical: &GeneratedAffineResidualGroupLatticeShift,
        upper: usize,
        set_limit: fn(&mut GeneratedAffineResidualGroupPhysicalKeyLimits, usize),
    ) -> usize {
        assert!(upper > 0);
        let mut upper_frame = frame.clone();
        set_limit(&mut upper_frame.limits, upper);
        assert!(upper_frame.key_for_physical(physical).is_ok());
        let mut lower = 0usize;
        let mut upper = upper;
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            let mut candidate = frame.clone();
            set_limit(&mut candidate.limits, middle);
            if candidate.key_for_physical(physical).is_ok() {
                upper = middle;
            } else {
                lower = middle + 1;
            }
        }
        lower
    }

    #[test]
    fn canonical_integer_variants_obey_eq_hash_and_ord_contracts() {
        let inputs = [Integer::Single(1), Integer::Double(1), raw_large(1)];
        let shifts = inputs
            .iter()
            .map(|value| {
                make_shift_from_borrowed(std::slice::from_ref(value), 1, 1024, 1024, 4096).unwrap()
            })
            .collect::<Vec<_>>();
        assert!(shifts.windows(2).all(|pair| pair[0] == pair[1]));
        assert!(
            shifts
                .windows(2)
                .all(|pair| pair[0].cmp(&pair[1]) == Ordering::Equal)
        );
        assert_eq!(shifts.iter().cloned().collect::<HashSet<_>>().len(), 1);
        assert_eq!(shifts.iter().cloned().collect::<BTreeSet<_>>().len(), 1);
        assert!(matches!(shifts[0].values(), [Integer::Single(1)]));

        let double_boundary = Integer::Double(i128::from(i64::MAX) + 1);
        let boundary = make_shift_from_borrowed(&[double_boundary], 1, 128, 128, 4096).unwrap();
        assert!(matches!(boundary.values(), [Integer::Double(_)]));
        let minimum =
            make_shift_from_borrowed(&[Integer::Double(i128::MIN)], 1, 128, 128, 4096).unwrap();
        let negated_minimum = canonical_integer(-minimum.values()[0].clone());
        assert!(!negated_minimum.is_negative());
        assert_eq!(integer_bits(&negated_minimum).unwrap(), 128);

        let mut huge = MultiPrecisionInteger::from(1);
        huge <<= 300_u32;
        let huge = make_shift_from_borrowed(&[Integer::Large(huge)], 1, 512, 512, 4096).unwrap();
        assert_eq!(integer_bits(&huge.values()[0]).unwrap(), 301);
        assert!(matches!(huge.values(), [Integer::Large(_)]));
        let debug = format!("{huge:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("2037035976334486086268445688409378161051468393665936250636140449354381299763336706183397376"));
    }

    #[test]
    fn exact_gmp_rekey_round_trip_cancellation_and_one_below_limits() {
        let mut huge = MultiPrecisionInteger::from(1);
        huge <<= 300_u32;
        let offset_values = [Integer::Large(huge), Integer::Double(i128::MIN)];
        let offset = make_shift_from_borrowed(&offset_values, 2, 512, 1024, 8192).unwrap();
        let local = IndexShift::try_new([1, -1], 2).unwrap();
        let physical = add_i64_vector(offset.values(), local.values(), 512, 1024, 8192).unwrap();
        assert_eq!(integer_bits(&physical.values()[0]).unwrap(), 301);
        let recovered =
            subtract_borrowed_vectors(physical.values(), offset.values(), 2, 512, 1024, 8192)
                .unwrap();
        assert_eq!(recovered.try_to_index_shift().unwrap(), local);

        let zero =
            subtract_borrowed_vectors(physical.values(), physical.values(), 2, 512, 1024, 8192)
                .unwrap();
        assert!(zero.values().iter().all(Integer::is_zero));
        assert_eq!(zero.retained_integer_bits(), 0);

        let largest = offset
            .values()
            .iter()
            .map(|value| integer_bits(value).unwrap())
            .max()
            .unwrap();
        assert!(matches!(
            make_shift_from_borrowed(&offset_values, 2, largest - 1, 1024, 8192),
            Err(GeneratedAffineResidualGroupPhysicalKeyError::ResourceLimit { .. })
        ));
        assert!(matches!(
            make_shift_from_borrowed(
                &offset_values,
                2,
                512,
                offset.retained_integer_bits() - 1,
                8192,
            ),
            Err(GeneratedAffineResidualGroupPhysicalKeyError::ResourceLimit { .. })
        ));
        assert!(matches!(
            make_shift_from_borrowed(&offset_values, 2, 512, 1024, offset.retained_bytes() - 1,),
            Err(GeneratedAffineResidualGroupPhysicalKeyError::ResourceLimit { .. })
        ));
    }

    #[test]
    fn admitted_key_token_is_exact_frame_and_shift_bound_with_gmp_census() {
        let (family, context, _inventory, authority) =
            fixture("group-physical-key-preflight-private");
        let frame = Arc::new(
            GeneratedAffineResidualGroupPhysicalFrame::try_new(
                &family,
                &context,
                Arc::clone(&authority),
                GeneratedAffineResidualGroupPhysicalKeyLimits::default(),
            )
            .unwrap(),
        );
        let independently_built_frame = Arc::new(
            GeneratedAffineResidualGroupPhysicalFrame::try_new(
                &family,
                &context,
                Arc::clone(&authority),
                GeneratedAffineResidualGroupPhysicalKeyLimits::default(),
            )
            .unwrap(),
        );

        let mut wide_component = MultiPrecisionInteger::from(1);
        wide_component <<= 300_u32;
        let mut wide_values = vec![Integer::from(0); frame.arity()];
        wide_values[0] = Integer::Large(wide_component);
        if frame.arity() > 1 {
            let mut negative = MultiPrecisionInteger::from(1);
            negative <<= 332_u32;
            wide_values[1] = Integer::Large(-negative);
        }
        let wide_physical = make_shift_from_borrowed(
            &wide_values,
            frame.arity(),
            512,
            frame.limits().max_shift_total_integer_bits,
            frame.limits().max_shift_retained_bytes,
        )
        .unwrap();
        assert_eq!(integer_bits(&wide_physical.values()[0]).unwrap(), 301);
        if frame.arity() > 1 {
            assert_eq!(integer_bits(&wide_physical.values()[1]).unwrap(), 333);
        }

        let admitted = frame.preflight_key_for_physical(&wide_physical).unwrap();
        assert_eq!(admitted.component_scans(), frame.arity());
        assert!(admitted.integer_bit_work() >= wide_physical.retained_integer_bits());
        let admitted_integer_bits = admitted.prospective_retained_integer_bits();
        let admitted_bytes = admitted.prospective_retained_bytes();
        let debug = format!("{admitted:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains(
            "2037035976334486086268445688409378161051468393665936250636140449354381299763336706183397376"
        ));
        assert!(matches!(
            independently_built_frame.key_for_preflight(admitted),
            Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongFrameAllocation)
        ));

        // Even a value clone shares every child `Arc`; only binding to the
        // exact outer frame allocation rejects this subtle substitute.
        let value_cloned_frame = Arc::new((*frame).clone());
        let admitted = frame.preflight_key_for_physical(&wide_physical).unwrap();
        assert!(matches!(
            value_cloned_frame.key_for_preflight(admitted),
            Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongFrameAllocation)
        ));

        // A cheap clone of the exact frame preserves its private allocation
        // identity, while a clone of the frame value above does not.
        let exact_frame_clone = Arc::clone(&frame);
        let admitted = frame.preflight_key_for_physical(&wide_physical).unwrap();
        let preflighted_key = exact_frame_clone.key_for_preflight(admitted).unwrap();
        let ordinary_key = frame.key_for_physical(&wide_physical).unwrap();
        assert_eq!(preflighted_key, ordinary_key);
        assert!(ordinary_key.retained_integer_bits() <= admitted_integer_bits);
        assert!(ordinary_key.retained_bytes() <= admitted_bytes);

        for (upper, set_limit) in [
            (
                frame.limits().max_key_integer_bits,
                (|limits: &mut GeneratedAffineResidualGroupPhysicalKeyLimits, value| {
                    limits.max_key_integer_bits = value;
                }) as fn(&mut GeneratedAffineResidualGroupPhysicalKeyLimits, usize),
            ),
            (
                frame.limits().max_key_total_integer_bits,
                (|limits: &mut GeneratedAffineResidualGroupPhysicalKeyLimits, value| {
                    limits.max_key_total_integer_bits = value;
                }) as fn(&mut GeneratedAffineResidualGroupPhysicalKeyLimits, usize),
            ),
            (
                frame.limits().max_key_retained_bytes,
                (|limits: &mut GeneratedAffineResidualGroupPhysicalKeyLimits, value| {
                    limits.max_key_retained_bytes = value;
                }) as fn(&mut GeneratedAffineResidualGroupPhysicalKeyLimits, usize),
            ),
        ] {
            let minimum = minimum_admitted_key_limit(&frame, &wide_physical, upper, set_limit);
            assert!(minimum > 0);
            let mut exact_value = (*frame).clone();
            set_limit(&mut exact_value.limits, minimum);
            let exact = Arc::new(exact_value);
            let admitted = exact.preflight_key_for_physical(&wide_physical).unwrap();
            exact.key_for_preflight(admitted).unwrap();
            let mut below_value = (*frame).clone();
            set_limit(&mut below_value.limits, minimum - 1);
            let below = Arc::new(below_value);
            assert!(matches!(
                below.preflight_key_for_physical(&wide_physical),
                Err(GeneratedAffineResidualGroupPhysicalKeyError::ResourceLimit { .. })
            ));
        }
    }

    #[test]
    fn spare_gmp_capacity_is_canonicalized_before_preflight_and_replay() {
        let (family, context, _inventory, authority) =
            fixture("group-physical-key-spare-gmp-capacity-private");
        let frame = Arc::new(
            GeneratedAffineResidualGroupPhysicalFrame::try_new(
                &family,
                &context,
                authority,
                GeneratedAffineResidualGroupPhysicalKeyLimits::default(),
            )
            .unwrap(),
        );
        let symbolic_position = *frame
            .symbolic_positions()
            .first()
            .expect("the fixture must exercise the symbolic GMP-clone path");

        // This is a genuinely `Large` value with a modest 301-bit magnitude
        // but one million bits of deliberately unused GMP allocation.
        let mut spare_capacity_value = MultiPrecisionInteger::with_capacity(1_000_000);
        spare_capacity_value += 1;
        spare_capacity_value <<= 300_u32;
        spare_capacity_value += 37;
        let source_capacity = spare_capacity_value.capacity();
        assert!(source_capacity >= 1_000_000);
        let mut raw_values = vec![Integer::from(0); frame.arity()];
        raw_values[symbolic_position] = Integer::Large(spare_capacity_value);

        let physical = make_shift_from_borrowed(
            &raw_values,
            frame.arity(),
            frame.limits().max_shift_integer_bits,
            frame.limits().max_shift_total_integer_bits,
            frame.limits().max_shift_retained_bytes,
        )
        .unwrap();
        assert_eq!(
            integer_bits(&physical.values()[symbolic_position]).unwrap(),
            301
        );
        let physical_capacity = match &physical.values()[symbolic_position] {
            Integer::Large(value) => value.capacity(),
            Integer::Single(_) | Integer::Double(_) => {
                panic!("a 301-bit component must remain GMP-backed")
            }
        };
        assert!(physical_capacity < source_capacity);

        let shift_owners_before = Arc::strong_count(&physical.values);
        let admitted = frame.preflight_key_for_physical(&physical).unwrap();
        assert!(Arc::ptr_eq(&physical.values, &admitted.physical.values));
        assert_eq!(Arc::strong_count(&physical.values), shift_owners_before + 1);
        let admitted_integer_bits = admitted.prospective_retained_integer_bits();
        let admitted_bytes = admitted.prospective_retained_bytes();

        let key = frame.key_for_preflight(admitted).unwrap();
        assert!(Arc::ptr_eq(&physical.values, &key.shift.values));
        assert!(key.retained_integer_bits() <= admitted_integer_bits);
        assert!(key.retained_bytes() <= admitted_bytes);
        let excess_capacity = match &key.signed_index_excess()[symbolic_position] {
            Integer::Large(value) => value.capacity(),
            Integer::Single(_) | Integer::Double(_) => {
                panic!("the exact symbolic excess must remain GMP-backed")
            }
        };
        assert!(excess_capacity < source_capacity);
        frame.replay_key(&key).unwrap();
        let helper_key = frame
            .test_key_for_borrowed_physical_values(&raw_values)
            .unwrap();
        assert_eq!(helper_key, key);
        frame.replay_key(&helper_key).unwrap();
    }

    #[test]
    fn authenticated_two_loop_group_round_trips_and_matches_case_ordering() {
        let (family, context, inventory, authority) =
            fixture("group-physical-key-two-loop-private");
        let (exact, exact_stats) = exact_frame_limits(&family, &context, &authority);
        let frame = GeneratedAffineResidualGroupPhysicalFrame::try_new(
            &family,
            &context,
            Arc::clone(&authority),
            exact,
        )
        .unwrap();
        assert_eq!(frame.stats(), exact_stats);
        assert_eq!(
            frame.schema(),
            GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_FRAME_V1_SCHEMA
        );
        assert_eq!(frame.group_ordinal(), authority.group_ordinal());
        assert_eq!(frame.anchor_case_ordinal(), authority.case_ordinal());
        assert_eq!(frame.case_ordinals().len(), exact_stats.group_cases());
        assert_eq!(
            frame.constant_positions().len() + frame.symbolic_positions().len(),
            frame.arity()
        );
        assert_eq!(frame.stable_manifest().len(), exact_stats.manifest_bytes());
        assert!(
            frame
                .stable_manifest()
                .starts_with(GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_FRAME_V1_SCHEMA)
        );
        frame.replay(&family, &context, &authority).unwrap();

        let independent = Arc::new((*authority).clone());
        assert!(!Arc::ptr_eq(&authority, &independent));
        assert!(matches!(
            frame.replay(&family, &context, &independent),
            Err(GeneratedAffineResidualGroupPhysicalKeyError::WrongAuthorityAllocation)
        ));
        let debug = format!("{frame:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("m2"));
        assert!(!debug.contains("group-physical-key-two-loop-private"));

        let group = authority.authenticated_group_view(&context).unwrap();
        for (position, &case_ordinal) in frame.case_ordinals().iter().enumerate() {
            let offset = frame.anchor_offset(position, case_ordinal).unwrap();
            for &free_position in group.free_positions() {
                assert!(offset.values()[free_position].is_zero());
            }
            let local_values = (0..frame.arity())
                .map(|component| component as i64 - position as i64)
                .collect::<Vec<_>>();
            let local = IndexShift::try_new(local_values, frame.arity()).unwrap();
            let physical = frame
                .physical_from_local(position, case_ordinal, &local)
                .unwrap();
            let replayed_local = frame
                .local_from_physical(position, case_ordinal, &physical)
                .unwrap();
            assert_eq!(replayed_local.try_to_index_shift().unwrap(), local);
            let key = frame.key_for_physical(&physical).unwrap();
            assert_eq!(
                key.schema(),
                GENERATED_AFFINE_RESIDUAL_GROUP_PHYSICAL_KEY_V1_SCHEMA
            );
            assert_eq!(key.policy(), authority.ordering());
            assert_eq!(key.shift(), &physical);
            assert_eq!(key.signed_index_excess().len(), frame.arity());
            assert_eq!(key.formal_sector().arity(), frame.arity());
            assert!(key.propagators() <= frame.arity());
            assert!(key.retained_integer_bits() >= physical.retained_integer_bits());
            assert!(key.retained_bytes() >= physical.retained_bytes());
            frame.replay_key(&key).unwrap();
        }

        // The same anchor integral has one exact physical representation from
        // every case, even though its local coordinates differ.
        let zero = IndexShift::try_new(vec![0; frame.arity()], frame.arity()).unwrap();
        let anchor_physical = frame
            .physical_from_local(0, frame.anchor_case_ordinal(), &zero)
            .unwrap();
        for (position, &case_ordinal) in frame.case_ordinals().iter().enumerate() {
            let local = frame
                .local_from_physical(position, case_ordinal, &anchor_physical)
                .unwrap();
            if let Ok(compact) = local.try_to_index_shift() {
                assert_eq!(
                    frame
                        .physical_from_local(position, case_ordinal, &compact)
                        .unwrap(),
                    anchor_physical
                );
            }
        }

        // On representable points from every case, including nonzero anchor
        // offsets, the common physical key agrees with the authenticated
        // case-local ordering.  Rebuilding the frame from each case must also
        // recover the same anchor geometry and physical values.
        let mut shifts = vec![zero];
        for position in 0..frame.arity() {
            for value in [-2, -1, 1, 2] {
                let mut components = vec![0; frame.arity()];
                components[position] = value;
                shifts.push(IndexShift::try_new(components, frame.arity()).unwrap());
            }
        }
        for (case_position, &case_ordinal) in frame.case_ordinals().iter().enumerate() {
            let case_authority = Arc::new(
                GeneratedAffineResidualCaseAuthority::try_new(
                    &family,
                    &context,
                    Arc::clone(&inventory),
                    case_ordinal,
                    GeneratedAffineResidualCaseAuthorityLimits::default(),
                )
                .unwrap(),
            );
            let case_frame = GeneratedAffineResidualGroupPhysicalFrame::try_new(
                &family,
                &context,
                Arc::clone(&case_authority),
                GeneratedAffineResidualGroupPhysicalKeyLimits::default(),
            )
            .unwrap();
            assert_eq!(case_frame.source_case_ordinal, case_ordinal);
            assert_eq!(case_frame.source_ordinal_within_group, case_position);
            assert_eq!(case_frame.anchor_constants(), frame.anchor_constants());
            assert_eq!(case_frame.case_ordinals(), frame.case_ordinals());
            let ordering = GeneratedAffineParametricOrderingCertificate::try_new(
                &family,
                &context,
                Arc::clone(&case_authority),
                GeneratedAffineParametricOrderingLimits::default(),
            )
            .unwrap();
            for left in &shifts {
                for right in &shifts {
                    let left_physical = frame
                        .physical_from_local(case_position, case_ordinal, left)
                        .unwrap();
                    let right_physical = frame
                        .physical_from_local(case_position, case_ordinal, right)
                        .unwrap();
                    assert_eq!(
                        case_frame
                            .physical_from_local(case_position, case_ordinal, left)
                            .unwrap(),
                        left_physical
                    );
                    let group_comparison = frame
                        .key_for_physical(&left_physical)
                        .unwrap()
                        .cmp(&frame.key_for_physical(&right_physical).unwrap());
                    assert_eq!(
                        group_comparison,
                        ordering.compare_shifts(&context, left, right).unwrap(),
                        "ordering mismatch for case {case_ordinal}, {left:?}, and {right:?}"
                    );
                }
            }
        }

        // Resource policy is frame state, not mathematical key state.
        let baseline = frame.key_for_physical(&anchor_physical).unwrap();
        let mut alternate_frame = frame.clone();
        alternate_frame.limits.max_manifest_bytes =
            alternate_frame.limits.max_manifest_bytes.saturating_add(1);
        let alternate = alternate_frame.key_for_physical(&anchor_physical).unwrap();
        assert_eq!(baseline, alternate);
        assert_eq!(baseline.cmp(&alternate), Ordering::Equal);

        // A canonical lattice shift is intentionally frame-neutral.  A
        // receiving frame must re-enforce its own component ceiling even if
        // the shift was admitted under wider limits.
        let mut wide_component = MultiPrecisionInteger::from(1);
        wide_component <<= 300_u32;
        let mut wide_values = vec![Integer::from(0); frame.arity()];
        wide_values[0] = Integer::Large(wide_component);
        let wide_physical = make_shift_from_borrowed(
            &wide_values,
            frame.arity(),
            512,
            frame.limits().max_shift_total_integer_bits,
            frame.limits().max_shift_retained_bytes,
        )
        .unwrap();
        assert_eq!(integer_bits(&wide_physical.values()[0]).unwrap(), 301);
        let wide_key = frame.key_for_physical(&wide_physical).unwrap();
        let wide_debug = format!("{wide_key:?}");
        assert!(wide_debug.contains("<redacted>"));
        assert!(!wide_debug.contains(
            "2037035976334486086268445688409378161051468393665936250636140449354381299763336706183397376"
        ));
        let mut narrow_frame = frame.clone();
        narrow_frame.limits.max_shift_integer_bits = 300;
        assert!(matches!(
            narrow_frame.key_for_physical(&wide_physical),
            Err(GeneratedAffineResidualGroupPhysicalKeyError::ResourceLimit { .. })
        ));
        assert!(matches!(
            narrow_frame.local_from_physical(0, frame.anchor_case_ordinal(), &wide_physical),
            Err(GeneratedAffineResidualGroupPhysicalKeyError::ResourceLimit { .. })
        ));

        for (exact_limit, set_limit) in [
            (
                wide_physical.retained_integer_bits(),
                (|limits: &mut GeneratedAffineResidualGroupPhysicalKeyLimits, value| {
                    limits.max_shift_total_integer_bits = value;
                }) as fn(&mut GeneratedAffineResidualGroupPhysicalKeyLimits, usize),
            ),
            (
                wide_physical.retained_bytes(),
                (|limits: &mut GeneratedAffineResidualGroupPhysicalKeyLimits, value| {
                    limits.max_shift_retained_bytes = value;
                }) as fn(&mut GeneratedAffineResidualGroupPhysicalKeyLimits, usize),
            ),
        ] {
            let mut exact_frame = frame.clone();
            set_limit(&mut exact_frame.limits, exact_limit);
            exact_frame.key_for_physical(&wide_physical).unwrap();
            let mut below = frame.clone();
            set_limit(&mut below.limits, exact_limit - 1);
            assert!(matches!(
                below.key_for_physical(&wide_physical),
                Err(GeneratedAffineResidualGroupPhysicalKeyError::ResourceLimit { .. })
            ));
        }

        for (upper, set_limit) in [
            (
                frame.limits().max_key_integer_bits,
                (|limits: &mut GeneratedAffineResidualGroupPhysicalKeyLimits, value| {
                    limits.max_key_integer_bits = value;
                }) as fn(&mut GeneratedAffineResidualGroupPhysicalKeyLimits, usize),
            ),
            (
                frame.limits().max_key_total_integer_bits,
                (|limits: &mut GeneratedAffineResidualGroupPhysicalKeyLimits, value| {
                    limits.max_key_total_integer_bits = value;
                }) as fn(&mut GeneratedAffineResidualGroupPhysicalKeyLimits, usize),
            ),
            (
                frame.limits().max_key_retained_bytes,
                (|limits: &mut GeneratedAffineResidualGroupPhysicalKeyLimits, value| {
                    limits.max_key_retained_bytes = value;
                }) as fn(&mut GeneratedAffineResidualGroupPhysicalKeyLimits, usize),
            ),
        ] {
            let minimum = minimum_admitted_key_limit(&frame, &wide_physical, upper, set_limit);
            assert!(minimum > 0);
            let mut exact_frame = frame.clone();
            set_limit(&mut exact_frame.limits, minimum);
            exact_frame.key_for_physical(&wide_physical).unwrap();
            let mut below = frame.clone();
            set_limit(&mut below.limits, minimum - 1);
            assert!(matches!(
                below.key_for_physical(&wide_physical),
                Err(GeneratedAffineResidualGroupPhysicalKeyError::ResourceLimit { .. })
            ));
        }

        macro_rules! one_below {
            ($field:ident, $stat:ident) => {
                if exact_stats.$stat() > 0 {
                    let mut limits = exact;
                    limits.$field = exact_stats.$stat() - 1;
                    assert!(matches!(
                        GeneratedAffineResidualGroupPhysicalFrame::try_new(
                            &family,
                            &context,
                            Arc::clone(&authority),
                            limits,
                        ),
                        Err(GeneratedAffineResidualGroupPhysicalKeyError::ResourceLimit { .. })
                    ));
                }
            };
        }
        one_below!(max_authority_replays, authority_replays);
        one_below!(max_case_view_resolutions, case_view_resolutions);
        one_below!(max_group_view_resolutions, group_view_resolutions);
        one_below!(
            max_retained_authority_references,
            retained_authority_references
        );
        one_below!(max_group_cases, group_cases);
        one_below!(max_arity, arity);
        one_below!(max_free_positions, free_positions);
        one_below!(max_matrix_entries_inspected, matrix_entries_inspected);
        one_below!(max_offset_components, offset_components);
        one_below!(max_geometry_integer_bits, largest_geometry_integer_bits);
        one_below!(max_geometry_integer_bit_work, geometry_integer_bit_work);
        one_below!(
            max_geometry_total_integer_bits,
            retained_geometry_integer_bits
        );
        one_below!(max_frame_retained_bytes, frame_retained_bytes);
        one_below!(max_manifest_bytes, manifest_bytes);

        let weak: Weak<GeneratedAffineResidualCaseAuthority> = Arc::downgrade(&authority);
        drop(alternate_frame);
        drop(narrow_frame);
        drop(frame);
        drop(inventory);
        drop(independent);
        drop(authority);
        assert!(weak.upgrade().is_none());
    }
}
