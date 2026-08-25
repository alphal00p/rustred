//! Exact formal ordering on one authenticated integer-affine residual start.
//!
//! A column key denotes `J(F(t)+q)`, where the complete, replayable map
//! `F(t)=b+A*t` is owned behind an [`Arc`].  Rows of `A` which are identically
//! zero are exact integer constants and are evaluated against the source
//! sector.  Every nonzero row remains formal; no representative value of its
//! free variables is invented.
//!
//! On a concrete specialization this normalized order agrees with
//! [`IntegralOrderingPolicy::RustRedUnshiftedV1`] only while every compared
//! translation keeps all symbolic rows inside their authenticated source
//! chamber. Crossing a symbolic-row boundary is a residual-case split, not an
//! ordering operation on this certificate.  A Boolean-branch source retains
//! its complete branch certificate and original nonzero guards.  Neither a
//! key nor a prepare-point translation proves that `F(t)+q` remains in that
//! Boolean branch.

use std::cmp::Ordering;
use std::fmt;
use std::fmt::Write as _;
use std::mem::{align_of, size_of};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use symbolica::prelude::Integer;

use crate::{
    IndexShift, IntegralFamily, IntegralOrderingPolicy, ParametricCoefficientContext,
    ParametricPolynomial, ResidualAffineBranchSystemCertificate, ResidualAffineBranchSystemError,
    ResidualAffineIntegerMap, ResidualProductLocusBooleanCoverCertificate,
    ResidualUnitAffineIndexMapCertificate, ResidualUnitAffineIndexMapError, SectorMask,
};

pub const AFFINE_START_PARAMETRIC_ELIMINATION_ORDERING_V1_SCHEMA: &str =
    "rustred-affine-start-parametric-elimination-ordering-v1";
pub const AFFINE_START_INTEGRAL_COMPLEXITY_KEY_V1_SCHEMA: &str =
    "rustred-affine-start-integral-complexity-key-v1";
pub const RUSTRED_AFFINE_START_UNSHIFTED_ORDER_V1_KEY_SCHEMA: &str = "arity,propagators,formal-sector-bits,signed-corner-distance-offset,\
signed-dots-offset,signed-numerators-offset,signed-index-excess,lattice-shift";

const KEY_FIXED_COMPONENTS: usize = 5;
const KEY_COMPONENT_VECTORS: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AffineStartSourceKind {
    ResidualUnit,
    ResidualBooleanBranch,
}

/// Complete replay provenance for one affine start.
///
/// The Boolean-branch variant is intentionally not converted into a
/// [`ResidualUnitAffineIndexMapCertificate`]: doing so would invent one
/// predicate/bound-position locator for a simultaneous system of equalities.
#[derive(Clone, Debug)]
pub enum AffineStartSourceCertificate {
    ResidualUnit(Arc<ResidualUnitAffineIndexMapCertificate>),
    ResidualBooleanBranch(Arc<ResidualAffineBranchSystemCertificate>),
}

impl AffineStartSourceCertificate {
    pub const fn kind(&self) -> AffineStartSourceKind {
        match self {
            Self::ResidualUnit(_) => AffineStartSourceKind::ResidualUnit,
            Self::ResidualBooleanBranch(_) => AffineStartSourceKind::ResidualBooleanBranch,
        }
    }

    pub const fn legacy_affine_map(&self) -> Option<&Arc<ResidualUnitAffineIndexMapCertificate>> {
        match self {
            Self::ResidualUnit(map) => Some(map),
            Self::ResidualBooleanBranch(_) => None,
        }
    }

    pub const fn residual_branch(&self) -> Option<&Arc<ResidualAffineBranchSystemCertificate>> {
        match self {
            Self::ResidualUnit(_) => None,
            Self::ResidualBooleanBranch(branch) => Some(branch),
        }
    }

    /// Original Coverage V4 nonzero-atom ordinals. They remain uncomposed
    /// through the affine map and say nothing about translated points.
    pub fn uncomposed_nonzero_guard_locus_ordinals(&self) -> &[usize] {
        match self {
            Self::ResidualUnit(_) => &[],
            Self::ResidualBooleanBranch(branch) => branch.nonzero_guard_locus_ordinals(),
        }
    }

    fn context_fingerprint(&self) -> &str {
        match self {
            Self::ResidualUnit(map) => map.context_fingerprint(),
            Self::ResidualBooleanBranch(branch) => branch.context_fingerprint(),
        }
    }

    fn source_sector(&self) -> &SectorMask {
        match self {
            Self::ResidualUnit(map) => map.source().source_partition().orthant().sector(),
            Self::ResidualBooleanBranch(branch) => branch.source_cover().sector(),
        }
    }

    fn payload_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::ResidualUnit(left), Self::ResidualUnit(right)) => {
                Arc::ptr_eq(left, right) || left.payload_eq(right)
            }
            (Self::ResidualBooleanBranch(left), Self::ResidualBooleanBranch(right)) => {
                Arc::ptr_eq(left, right) || left.payload_eq(right)
            }
            _ => false,
        }
    }
}

/// Replay authority must match the retained source kind. Boolean branches
/// require the complete family/context/cover boundary on every public replay.
#[derive(Clone, Copy, Debug)]
pub enum AffineStartReplayAuthority<'a> {
    ContextOnly(&'a ParametricCoefficientContext),
    ResidualBooleanBranch {
        family: &'a IntegralFamily,
        context: &'a ParametricCoefficientContext,
        cover: &'a Arc<ResidualProductLocusBooleanCoverCertificate>,
    },
}

impl<'a> AffineStartReplayAuthority<'a> {
    pub const fn context(self) -> &'a ParametricCoefficientContext {
        match self {
            Self::ContextOnly(context) | Self::ResidualBooleanBranch { context, .. } => context,
        }
    }
}

/// Borrowed geometry-only view `F(t)=b+A*t`.
///
/// The column argument is always a free-coordinate ordinal. Branch-system
/// maps store a square ambient matrix, so this view translates that ordinal
/// to the corresponding original ambient free-position column.
#[derive(Clone, Copy, Debug)]
pub struct AffineStartGeometryRef<'a> {
    source: &'a AffineStartSourceCertificate,
}

impl<'a> AffineStartGeometryRef<'a> {
    pub fn ambient_arity(self) -> usize {
        match self.source {
            AffineStartSourceCertificate::ResidualUnit(map) => map.ambient_arity(),
            AffineStartSourceCertificate::ResidualBooleanBranch(branch) => branch
                .affine_map()
                .map_or(0, ResidualAffineIntegerMap::ambient_arity),
        }
    }

    pub fn free_positions(self) -> &'a [usize] {
        match self.source {
            AffineStartSourceCertificate::ResidualUnit(map) => map.free_positions(),
            AffineStartSourceCertificate::ResidualBooleanBranch(branch) => branch
                .affine_map()
                .map_or(&[], ResidualAffineIntegerMap::free_positions),
        }
    }

    pub fn constant(self, position: usize) -> Option<&'a Integer> {
        match self.source {
            AffineStartSourceCertificate::ResidualUnit(map) => map.constant(position),
            AffineStartSourceCertificate::ResidualBooleanBranch(branch) => {
                branch.affine_map()?.constant(position)
            }
        }
    }

    pub fn linear_coefficient(self, position: usize, free_ordinal: usize) -> Option<&'a Integer> {
        match self.source {
            AffineStartSourceCertificate::ResidualUnit(map) => {
                map.linear_coefficient(position, free_ordinal)
            }
            AffineStartSourceCertificate::ResidualBooleanBranch(branch) => {
                let map = branch.affine_map()?;
                let &ambient_column = map.free_positions().get(free_ordinal)?;
                map.linear_coefficient(position, ambient_column)
            }
        }
    }
}

/// Source-independent read-only integer-affine geometry used by the ordering
/// algebra.  Authority adapters implement only this narrow interface; no V1
/// source locator or certificate kind is part of key construction.
pub(crate) trait AffineParametricOrderingGeometry<'source> {
    fn ambient_arity(&self) -> usize;
    fn free_positions(&self) -> &'source [usize];
    fn constant(&self, position: usize) -> Option<&'source Integer>;
    fn linear_coefficient(&self, position: usize, free_ordinal: usize) -> Option<&'source Integer>;
}

impl<'source> AffineParametricOrderingGeometry<'source> for AffineStartGeometryRef<'source> {
    fn ambient_arity(&self) -> usize {
        (*self).ambient_arity()
    }

    fn free_positions(&self) -> &'source [usize] {
        (*self).free_positions()
    }

    fn constant(&self, position: usize) -> Option<&'source Integer> {
        (*self).constant(position)
    }

    fn linear_coefficient(&self, position: usize, free_ordinal: usize) -> Option<&'source Integer> {
        (*self).linear_coefficient(position, free_ordinal)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AffineParametricOrderingLimits {
    pub max_arity: usize,
    pub max_free_positions: usize,
    pub max_constant_positions: usize,
    pub max_symbolic_positions: usize,
    pub max_matrix_entries_inspected: usize,
    pub max_key_components: usize,
    pub max_affine_integer_bits: usize,
    pub max_key_integer_bits: usize,
    /// Exact sum of magnitude bits retained by all arbitrary-precision
    /// integer fields in one key (three totals plus one excess per row).
    pub max_key_total_integer_bits: usize,
    /// Exact bytes of the persisted source identity. Arbitrary integers use
    /// the identity's versioned sign-magnitude hexadecimal encoding.
    pub max_map_identity_bytes: usize,
    pub max_manifest_bytes: usize,
    /// Prospective ceiling for the human-readable decimal key diagnostic.
    /// GMP decimal digit preflight is conservative and may reject a limit
    /// which would fit the eventual rendering. Persisted identity sizing is
    /// exact and does not use this human-diagnostic estimate.
    pub max_key_diagnostic_bytes: usize,
}

impl Default for AffineParametricOrderingLimits {
    fn default() -> Self {
        Self {
            max_arity: 4096,
            max_free_positions: 4096,
            max_constant_positions: 4096,
            max_symbolic_positions: 4096,
            max_matrix_entries_inspected: 16_777_216,
            max_key_components: 16_384,
            max_affine_integer_bits: 1_000_000,
            // A key may sum one excess per ambient row.  This leaves the
            // default map bound plus ceil(log2(4096)) and conservative carries.
            max_key_integer_bits: 1_000_016,
            max_key_total_integer_bits: 512 * 1024 * 1024,
            max_map_identity_bytes: 1024 * 1024 * 1024,
            max_manifest_bytes: 2 * 1024 * 1024 * 1024,
            max_key_diagnostic_bytes: 256 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AffineParametricOrderingStats {
    ambient_arity: usize,
    free_positions: usize,
    constant_positions: usize,
    symbolic_positions: usize,
    matrix_entries_inspected: usize,
    largest_affine_integer_bits: usize,
    map_identity_bytes: usize,
    manifest_bytes: usize,
}

/// Exact chamber transition of one affine row already authenticated as
/// constant by [`AffineStartParametricEliminationOrdering`].
///
/// This type is crate-private because the row position and transition are
/// private target geometry.  Public affine-`WhenBad` views expose only
/// aggregate counts and proof classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AffineConstantRowSectorTransition {
    StaysInSourceSector,
    UniversalActivePinch,
    UniversalInactiveActivation,
}

/// Bounded result of classifying one authenticated constant affine row.
///
/// Construction may obtain the classification by performing the admitted
/// arbitrary-precision addition.  Replay obtains the same Boolean result by
/// comparing the original constant with the exact `1 - displacement`
/// threshold, without cloning or allocating a GMP integer.  In either route,
/// `integer_bit_work` is the route-specific charged integer work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AffineConstantRowShiftClassification {
    position: usize,
    source_active: bool,
    shifted_active: bool,
    transition: AffineConstantRowSectorTransition,
    integer_bit_work: usize,
}

impl AffineConstantRowShiftClassification {
    pub(crate) const fn position(self) -> usize {
        self.position
    }

    pub(crate) const fn source_active(self) -> bool {
        self.source_active
    }

    pub(crate) const fn shifted_active(self) -> bool {
        self.shifted_active
    }

    pub(crate) const fn transition(self) -> AffineConstantRowSectorTransition {
        self.transition
    }

    pub(crate) const fn integer_bit_work(self) -> usize {
        self.integer_bit_work
    }
}

/// First exact cross-sector component which proves a target sector simpler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AffineSectorPrefixDescentComponent {
    PropagatorCount,
    SectorBits,
}

/// Policy-bound proof that one exact sector prefix is lower than this affine
/// ordering's source sector prefix.
///
/// The target sector itself remains owned by the caller's private replay
/// transcript.  This compact witness records the policy and decisive field;
/// replay rebuilds and compares the exact target bits again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AffineSectorPrefixDescentWitness {
    policy: IntegralOrderingPolicy,
    source_propagators: usize,
    target_propagators: usize,
    decisive_component: AffineSectorPrefixDescentComponent,
}

/// Exact work census returned by one allocation-free sector-prefix proof.
///
/// One comparison unit is one source/target activity-bit visit while counting
/// propagators, or one source/target bit-pair comparison in the equal-count
/// lexicographic tie break.  Keeping this census beside the proof prevents an
/// outer generated compiler from treating a bounded mask allocation as if it
/// also bounded every later scan of that mask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AffineSectorPrefixDescentCensus {
    witness: Option<AffineSectorPrefixDescentWitness>,
    comparison_units: usize,
}

impl AffineSectorPrefixDescentCensus {
    pub(crate) const fn witness(self) -> Option<AffineSectorPrefixDescentWitness> {
        self.witness
    }

    pub(crate) const fn comparison_units(self) -> usize {
        self.comparison_units
    }
}

impl AffineSectorPrefixDescentWitness {
    pub(crate) const fn policy(self) -> IntegralOrderingPolicy {
        self.policy
    }

    pub(crate) const fn source_propagators(self) -> usize {
        self.source_propagators
    }

    pub(crate) const fn target_propagators(self) -> usize {
        self.target_propagators
    }

    pub(crate) const fn decisive_component(self) -> AffineSectorPrefixDescentComponent {
        self.decisive_component
    }

    /// Replay this compact witness against the exact private target bits.
    /// Equality checks the persisted policy, both propagator counts, and the
    /// decisive prefix field; no later complexity component is consulted.
    pub(crate) fn replay(
        self,
        ordering: &AffineStartParametricEliminationOrdering,
        target_bits: &[bool],
    ) -> Result<bool, AffineParametricOrderingError> {
        Ok(self.replay_with_census(ordering, target_bits)?.0)
    }

    /// Replay while returning the exact bit-comparison work performed.
    pub(crate) fn replay_with_census(
        self,
        ordering: &AffineStartParametricEliminationOrdering,
        target_bits: &[bool],
    ) -> Result<(bool, usize), AffineParametricOrderingError> {
        if self.policy != ordering.policy() {
            return Ok((false, 0));
        }
        let census = ordering.prove_strict_sector_prefix_descent_bits_with_census(target_bits)?;
        Ok((census.witness() == Some(self), census.comparison_units()))
    }
}

impl AffineParametricOrderingStats {
    pub const fn ambient_arity(self) -> usize {
        self.ambient_arity
    }
    pub const fn free_positions(self) -> usize {
        self.free_positions
    }
    pub const fn constant_positions(self) -> usize {
        self.constant_positions
    }
    pub const fn symbolic_positions(self) -> usize {
        self.symbolic_positions
    }
    pub const fn matrix_entries_inspected(self) -> usize {
        self.matrix_entries_inspected
    }
    pub const fn largest_affine_integer_bits(self) -> usize {
        self.largest_affine_integer_bits
    }
    pub const fn map_identity_bytes(self) -> usize {
        self.map_identity_bytes
    }
    pub const fn manifest_bytes(self) -> usize {
        self.manifest_bytes
    }
}

/// One replay-bound symbolic-start ordering.  The complete map is shared;
/// keys share only the stable ordering manifest.
///
/// V1 deliberately authenticates one affine branch. A future finite union of
/// affine branches belongs in an outer scheduler whose children each expose
/// this same map-level interface; row-system code must not infer that one
/// residual leaf can only ever have one affine branch.
#[derive(Clone, Debug)]
pub struct AffineStartParametricEliminationOrdering {
    schema: &'static str,
    key_schema: &'static str,
    policy: IntegralOrderingPolicy,
    source: AffineStartSourceCertificate,
    constant_positions: Arc<Vec<usize>>,
    symbolic_positions: Arc<Vec<usize>>,
    limits: AffineParametricOrderingLimits,
    stats: AffineParametricOrderingStats,
    // This owns a bounded copy of the complete length-delimited source and
    // local map identities. `stats.manifest_bytes` reports those owned bytes;
    // the large typed source certificate itself remains shared by Arc.
    stable_manifest: Arc<String>,
}

impl PartialEq for AffineStartParametricEliminationOrdering {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
            || self.schema == other.schema
                && self.key_schema == other.key_schema
                && self.policy == other.policy
                && self.source.payload_eq(&other.source)
                && self.constant_positions == other.constant_positions
                && self.symbolic_positions == other.symbolic_positions
                && self.limits == other.limits
                && self.stats == other.stats
                && self.stable_manifest == other.stable_manifest
    }
}

impl Eq for AffineStartParametricEliminationOrdering {}

impl AffineStartParametricEliminationOrdering {
    pub fn try_new(
        context: &ParametricCoefficientContext,
        policy: IntegralOrderingPolicy,
        sector: SectorMask,
        affine_map: Arc<ResidualUnitAffineIndexMapCertificate>,
        limits: AffineParametricOrderingLimits,
    ) -> Result<Self, AffineParametricOrderingError> {
        let source = AffineStartSourceCertificate::ResidualUnit(affine_map.clone());
        let preflight = preflight_untrusted_source_metadata(context, &sector, &source, limits)?;
        affine_map.replay(context)?;
        let result =
            Self::try_new_with_authenticated_preflight(context, policy, source, limits, preflight)?;
        result.rebuild_and_compare_with_authenticated_source(context)?;
        Ok(result)
    }

    /// Construct an ordering from one fully authenticated Boolean terminal.
    /// The original nonzero guards remain attached to `branch`; this method
    /// does not compose or discharge them through the affine map.
    pub fn try_new_from_residual_branch(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        cover: Arc<ResidualProductLocusBooleanCoverCertificate>,
        policy: IntegralOrderingPolicy,
        branch: Arc<ResidualAffineBranchSystemCertificate>,
        limits: AffineParametricOrderingLimits,
    ) -> Result<Self, AffineParametricOrderingError> {
        let source = AffineStartSourceCertificate::ResidualBooleanBranch(branch.clone());
        // Authenticate all borrowed source metadata before retaining anything.
        // The ordering subsequently derives its sector from this immutable
        // source, avoiding a second user-sized sector allocation entirely.
        let preflight =
            preflight_untrusted_source_metadata(context, source.source_sector(), &source, limits)?;
        branch.replay_with_cover(family, context, cover)?;
        let result =
            Self::try_new_with_authenticated_preflight(context, policy, source, limits, preflight)?;
        // The branch has just crossed its complete family/context/cover replay
        // boundary. This private check only rebuilds ordering-owned metadata
        // from that authenticated immutable source; it is not a weaker source
        // replay path.
        result.rebuild_and_compare_with_authenticated_source(context)?;
        Ok(result)
    }

    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    pub const fn key_schema(&self) -> &'static str {
        self.key_schema
    }
    pub fn context_fingerprint(&self) -> &str {
        self.source.context_fingerprint()
    }
    pub const fn policy(&self) -> IntegralOrderingPolicy {
        self.policy
    }
    pub fn sector(&self) -> &SectorMask {
        self.source.source_sector()
    }
    pub const fn source(&self) -> &AffineStartSourceCertificate {
        &self.source
    }
    pub const fn source_kind(&self) -> AffineStartSourceKind {
        self.source.kind()
    }
    pub const fn legacy_affine_map(&self) -> Option<&Arc<ResidualUnitAffineIndexMapCertificate>> {
        self.source.legacy_affine_map()
    }
    /// Compatibility spelling for callers which explicitly handle the
    /// absence of a legacy single-predicate map.
    pub const fn affine_map(&self) -> Option<&Arc<ResidualUnitAffineIndexMapCertificate>> {
        self.legacy_affine_map()
    }
    pub const fn residual_branch(&self) -> Option<&Arc<ResidualAffineBranchSystemCertificate>> {
        self.source.residual_branch()
    }
    pub const fn geometry(&self) -> AffineStartGeometryRef<'_> {
        AffineStartGeometryRef {
            source: &self.source,
        }
    }
    pub fn uncomposed_nonzero_guard_locus_ordinals(&self) -> &[usize] {
        self.source.uncomposed_nonzero_guard_locus_ordinals()
    }
    pub fn free_positions(&self) -> &[usize] {
        self.geometry().free_positions()
    }
    pub fn constant_positions(&self) -> &[usize] {
        self.constant_positions.as_slice()
    }
    pub fn symbolic_positions(&self) -> &[usize] {
        self.symbolic_positions.as_slice()
    }
    pub const fn limits(&self) -> AffineParametricOrderingLimits {
        self.limits
    }
    pub const fn stats(&self) -> AffineParametricOrderingStats {
        self.stats
    }
    pub fn stable_manifest(&self) -> &str {
        self.stable_manifest.as_str()
    }
    pub fn arity(&self) -> usize {
        self.sector().arity()
    }

    /// Conservative bytes owned locally by this ordering, excluding the deep
    /// payload of its shared source certificate.  Arc control blocks, retained
    /// Vec/String headers, actual backing capacities, and the inline ordering
    /// are all included so an outer compiler can verify its own aggregate
    /// retained-byte envelope after construction.
    pub(crate) fn owned_retained_byte_bound(&self) -> Option<usize> {
        size_of::<Self>()
            .checked_add(arc_vec_owned_byte_bound(&self.constant_positions)?)?
            .checked_add(arc_vec_owned_byte_bound(&self.symbolic_positions)?)?
            .checked_add(arc_string_owned_byte_bound(&self.stable_manifest)?)
    }

    /// The exact constant value of one zero matrix row.  Symbolic rows and
    /// out-of-range positions return `None`.
    pub fn constant_start_value(&self, position: usize) -> Option<&Integer> {
        self.constant_positions
            .binary_search(&position)
            .ok()
            .and_then(|_| self.geometry().constant(position))
    }

    /// Conservative aggregate integer-bit work admitted before cloning and
    /// shifting one authenticated constant row.
    ///
    /// Target-local compilers use this allocation-free census for every RHS
    /// before proving the first RHS.  The later exact classification is
    /// checked against the same bound.
    pub(crate) fn constant_row_shift_integer_bit_work_bound(
        &self,
        position: usize,
        displacement: i64,
    ) -> Result<usize, AffineParametricOrderingError> {
        let geometry = self.geometry();
        AffineParametricOrderingAlgebra::new(
            self.policy,
            self.sector(),
            &geometry,
            self.constant_positions(),
            self.limits,
            &self.stable_manifest,
            self.key_schema,
        )
        .constant_row_shift_integer_bit_work_bound(position, displacement)
    }

    /// Ordinal form used by full-row generated scans.  `constant_positions`
    /// is authenticated and sorted, so an ordinal cursor avoids repeating a
    /// logarithmic search for every row and every replay pass.
    pub(crate) fn constant_row_shift_integer_bit_work_bound_by_ordinal(
        &self,
        constant_ordinal: usize,
        displacement: i64,
    ) -> Result<(usize, usize), AffineParametricOrderingError> {
        let geometry = self.geometry();
        AffineParametricOrderingAlgebra::new(
            self.policy,
            self.sector(),
            &geometry,
            self.constant_positions(),
            self.limits,
            &self.stable_manifest,
            self.key_schema,
        )
        .constant_row_shift_integer_bit_work_bound_by_ordinal(constant_ordinal, displacement)
    }

    /// Shift and classify one row already proved constant by this exact
    /// target ordering.
    pub(crate) fn classify_constant_row_shift(
        &self,
        position: usize,
        displacement: i64,
    ) -> Result<AffineConstantRowShiftClassification, AffineParametricOrderingError> {
        let geometry = self.geometry();
        AffineParametricOrderingAlgebra::new(
            self.policy,
            self.sector(),
            &geometry,
            self.constant_positions(),
            self.limits,
            &self.stable_manifest,
            self.key_schema,
        )
        .classify_constant_row_shift(position, displacement)
    }

    /// Ordinal form paired with a caller's linear constant-position cursor.
    pub(crate) fn classify_constant_row_shift_by_ordinal(
        &self,
        constant_ordinal: usize,
        displacement: i64,
    ) -> Result<AffineConstantRowShiftClassification, AffineParametricOrderingError> {
        let geometry = self.geometry();
        AffineParametricOrderingAlgebra::new(
            self.policy,
            self.sector(),
            &geometry,
            self.constant_positions(),
            self.limits,
            &self.stable_manifest,
            self.key_schema,
        )
        .classify_constant_row_shift_by_ordinal(constant_ordinal, displacement)
    }

    /// Conservative integer-bit work for allocation-free replay of one
    /// constant-row classification.  Replay compares the authenticated
    /// constant with `1 - displacement`; both operands' exact magnitude-bit
    /// counts are charged before any retained proof is constructed.
    pub(crate) fn replay_constant_row_shift_integer_bit_work_bound_by_ordinal(
        &self,
        constant_ordinal: usize,
        displacement: i64,
    ) -> Result<(usize, usize), AffineParametricOrderingError> {
        let geometry = self.geometry();
        AffineParametricOrderingAlgebra::new(
            self.policy,
            self.sector(),
            &geometry,
            self.constant_positions(),
            self.limits,
            &self.stable_manifest,
            self.key_schema,
        )
        .replay_constant_row_shift_integer_bit_work_bound_by_ordinal(constant_ordinal, displacement)
    }

    /// Allocation-free replay route for a previously classified constant
    /// row.  Since `displacement` is `i64`, the activation threshold is always
    /// representable by the inline `Integer::Double` variant; comparison with
    /// it cannot allocate or clone a GMP-backed source constant.
    pub(crate) fn replay_classify_constant_row_shift_by_ordinal(
        &self,
        constant_ordinal: usize,
        displacement: i64,
    ) -> Result<AffineConstantRowShiftClassification, AffineParametricOrderingError> {
        let geometry = self.geometry();
        AffineParametricOrderingAlgebra::new(
            self.policy,
            self.sector(),
            &geometry,
            self.constant_positions(),
            self.limits,
            &self.stable_manifest,
            self.key_schema,
        )
        .replay_classify_constant_row_shift_by_ordinal(constant_ordinal, displacement)
    }

    /// Test one displacement on a row already proved constant, while applying
    /// the ordering's arbitrary-precision resource ceiling before cloning or
    /// adding its value.  Prepare-point enumeration uses this boundary so it
    /// never performs an unchecked GMP allocation merely to reject a point.
    pub(crate) fn constant_row_shift_stays_in_source_sector(
        &self,
        position: usize,
        displacement: i64,
    ) -> Result<bool, AffineParametricOrderingError> {
        Ok(matches!(
            self.classify_constant_row_shift(position, displacement)?
                .transition(),
            AffineConstantRowSectorTransition::StaysInSourceSector
        ))
    }

    /// Compare only the exact cross-sector prefix owned by the persisted
    /// integral-ordering policy.  Later affine complexity and deterministic
    /// key tie-break fields are deliberately excluded.
    pub(crate) fn compare_sector_prefix(
        &self,
        target: &SectorMask,
    ) -> Result<Ordering, AffineParametricOrderingError> {
        self.compare_sector_prefix_bits(target.active_bits())
    }

    /// Allocation-free form of [`Self::compare_sector_prefix`] for a caller
    /// which has already pre-reserved and filled an exact private bit row.
    pub(crate) fn compare_sector_prefix_bits(
        &self,
        target_bits: &[bool],
    ) -> Result<Ordering, AffineParametricOrderingError> {
        compare_sector_prefix_bits_for_policy(self.policy, self.sector().active_bits(), target_bits)
    }

    /// Produce a compact policy-bound witness only when `target` is strictly
    /// lower at the exact sector prefix.
    pub(crate) fn prove_strict_sector_prefix_descent(
        &self,
        target: &SectorMask,
    ) -> Result<Option<AffineSectorPrefixDescentWitness>, AffineParametricOrderingError> {
        self.prove_strict_sector_prefix_descent_bits(target.active_bits())
    }

    /// Allocation-free form of [`Self::prove_strict_sector_prefix_descent`].
    pub(crate) fn prove_strict_sector_prefix_descent_bits(
        &self,
        target_bits: &[bool],
    ) -> Result<Option<AffineSectorPrefixDescentWitness>, AffineParametricOrderingError> {
        Ok(self
            .prove_strict_sector_prefix_descent_bits_with_census(target_bits)?
            .witness())
    }

    /// Allocation-free proof with an exact component-visit census.
    pub(crate) fn prove_strict_sector_prefix_descent_bits_with_census(
        &self,
        target_bits: &[bool],
    ) -> Result<AffineSectorPrefixDescentCensus, AffineParametricOrderingError> {
        let comparison = compare_sector_prefix_bits_for_policy_with_census(
            self.policy,
            self.sector().active_bits(),
            target_bits,
        )?;
        let witness =
            (comparison.ordering == Ordering::Less).then_some(AffineSectorPrefixDescentWitness {
                policy: self.policy,
                source_propagators: comparison.source_propagators,
                target_propagators: comparison.target_propagators,
                decisive_component: if comparison.target_propagators
                    != comparison.source_propagators
                {
                    AffineSectorPrefixDescentComponent::PropagatorCount
                } else {
                    AffineSectorPrefixDescentComponent::SectorBits
                },
            });
        Ok(AffineSectorPrefixDescentCensus {
            witness,
            comparison_units: comparison.comparison_units,
        })
    }

    pub fn key_for_shift(
        &self,
        shift: &IndexShift,
    ) -> Result<AffineStartIntegralComplexityKey, AffineParametricOrderingError> {
        let key = self.key_for_shift_unreplayed(shift, self.limits.max_key_total_integer_bits)?;
        self.replay_key(&key)?;
        Ok(key)
    }

    /// Construct one key under an additional caller-owned retained-integer
    /// allowance. The effective ceiling is the minimum of this allowance and
    /// the authenticated ordering ceiling.
    ///
    /// This is crate-private because callers must already have replayed the
    /// ordering. Prepare-point layers use it with their exact unspent
    /// cumulative allowance, so a key that cannot fit is rejected while its
    /// integer payload is still being assembled rather than after the whole
    /// key has been returned and retained.
    pub(crate) fn key_for_owned_shift_with_total_integer_bit_limit(
        &self,
        shift: IndexShift,
        max_retained_total_integer_bits: usize,
    ) -> Result<AffineStartIntegralComplexityKey, AffineParametricOrderingError> {
        self.key_for_owned_shift_unreplayed(
            shift,
            max_retained_total_integer_bits.min(self.limits.max_key_total_integer_bits),
        )
    }

    pub fn compare_shifts(
        &self,
        left: &IndexShift,
        right: &IndexShift,
    ) -> Result<Ordering, AffineParametricOrderingError> {
        // This is the formal within-branch comparison documented at module
        // scope. It intentionally does not sample free variables to decide
        // whether either shift exits a symbolic row's source chamber.
        Ok(self.key_for_shift(left)?.cmp(&self.key_for_shift(right)?))
    }

    pub fn replay(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), AffineParametricOrderingError> {
        self.replay_with_authority(AffineStartReplayAuthority::ContextOnly(context))
    }

    pub fn replay_with_authority(
        &self,
        authority: AffineStartReplayAuthority<'_>,
    ) -> Result<(), AffineParametricOrderingError> {
        if self.schema != AFFINE_START_PARAMETRIC_ELIMINATION_ORDERING_V1_SCHEMA
            || self.key_schema != RUSTRED_AFFINE_START_UNSHIFTED_ORDER_V1_KEY_SCHEMA
        {
            return Err(AffineParametricOrderingError::SchemaMismatch);
        }
        let context = authority.context();
        match (&self.source, authority) {
            (
                AffineStartSourceCertificate::ResidualUnit(map),
                AffineStartReplayAuthority::ContextOnly(_),
            ) => {
                preflight_untrusted_source_metadata(
                    context,
                    self.sector(),
                    &self.source,
                    self.limits,
                )?;
                map.replay(context)?;
            }
            (
                AffineStartSourceCertificate::ResidualBooleanBranch(_),
                AffineStartReplayAuthority::ContextOnly(_),
            ) => return Err(AffineParametricOrderingError::BranchReplayAuthorityRequired),
            (
                AffineStartSourceCertificate::ResidualUnit(_),
                AffineStartReplayAuthority::ResidualBooleanBranch { .. },
            ) => return Err(AffineParametricOrderingError::ReplayAuthoritySourceMismatch),
            (
                AffineStartSourceCertificate::ResidualBooleanBranch(branch),
                AffineStartReplayAuthority::ResidualBooleanBranch {
                    family,
                    context,
                    cover,
                },
            ) => {
                preflight_untrusted_source_metadata(
                    context,
                    self.sector(),
                    &self.source,
                    self.limits,
                )?;
                branch.replay_with_cover(family, context, (*cover).clone())?;
            }
        }
        self.rebuild_and_compare_with_authenticated_source(context)
    }

    pub fn replay_key(
        &self,
        key: &AffineStartIntegralComplexityKey,
    ) -> Result<(), AffineParametricOrderingError> {
        let geometry = self.geometry();
        AffineParametricOrderingAlgebra::new(
            self.policy,
            self.sector(),
            &geometry,
            self.constant_positions(),
            self.limits,
            &self.stable_manifest,
            self.key_schema,
        )
        .replay_key(key)
    }

    fn try_new_with_replayed_source(
        context: &ParametricCoefficientContext,
        policy: IntegralOrderingPolicy,
        source: AffineStartSourceCertificate,
        limits: AffineParametricOrderingLimits,
    ) -> Result<Self, AffineParametricOrderingError> {
        let preflight =
            preflight_untrusted_source_metadata(context, source.source_sector(), &source, limits)?;
        Self::try_new_with_authenticated_preflight(context, policy, source, limits, preflight)
    }

    fn try_new_with_authenticated_preflight(
        context: &ParametricCoefficientContext,
        policy: IntegralOrderingPolicy,
        source: AffineStartSourceCertificate,
        limits: AffineParametricOrderingLimits,
        preflight: AffineMapMetadataPreflight,
    ) -> Result<Self, AffineParametricOrderingError> {
        match policy {
            IntegralOrderingPolicy::RustRedUnshiftedV1 => {}
        }
        let arity = preflight.arity;
        let matrix_entries = preflight.matrix_entries;
        let constant_positions = preflight.constant_positions;
        let symbolic_positions = preflight.symbolic_positions;
        let largest_affine_integer_bits = preflight.largest_affine_integer_bits;

        let source_identity = affine_source_identity(&source, limits.max_map_identity_bytes)?;
        let map_identity_bytes = source_identity.len();

        let stable_manifest = ordering_manifest(
            context,
            policy,
            source.source_sector(),
            &source_identity,
            &constant_positions,
            &symbolic_positions,
            limits,
            limits.max_manifest_bytes,
        )?;
        let stats = AffineParametricOrderingStats {
            ambient_arity: arity,
            free_positions: AffineStartGeometryRef { source: &source }
                .free_positions()
                .len(),
            constant_positions: constant_positions.len(),
            symbolic_positions: symbolic_positions.len(),
            matrix_entries_inspected: matrix_entries,
            largest_affine_integer_bits,
            map_identity_bytes,
            manifest_bytes: stable_manifest.len(),
        };
        // Fixed-size Arc control blocks retain buffers already obtained via
        // fallible reservation; certificate cloning does not recopy them.
        let constant_positions = Arc::new(constant_positions);
        let symbolic_positions = Arc::new(symbolic_positions);
        Ok(Self {
            schema: AFFINE_START_PARAMETRIC_ELIMINATION_ORDERING_V1_SCHEMA,
            key_schema: RUSTRED_AFFINE_START_UNSHIFTED_ORDER_V1_KEY_SCHEMA,
            policy,
            source,
            constant_positions,
            symbolic_positions,
            limits,
            stats,
            // `Arc<String>` shares the already fallibly-grown buffer without
            // the infallible full-buffer copy performed by String -> Arc<str>.
            stable_manifest: Arc::new(stable_manifest),
        })
    }

    fn rebuild_and_compare_with_authenticated_source(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), AffineParametricOrderingError> {
        let replayed = Self::try_new_with_replayed_source(
            context,
            self.policy,
            self.source.clone(),
            self.limits,
        )?;
        if replayed == *self {
            Ok(())
        } else {
            Err(AffineParametricOrderingError::ReplayMismatch)
        }
    }

    fn key_for_shift_unreplayed(
        &self,
        shift: &IndexShift,
        max_retained_total_integer_bits: usize,
    ) -> Result<AffineStartIntegralComplexityKey, AffineParametricOrderingError> {
        let geometry = self.geometry();
        AffineParametricOrderingAlgebra::new(
            self.policy,
            self.sector(),
            &geometry,
            self.constant_positions(),
            self.limits,
            &self.stable_manifest,
            self.key_schema,
        )
        .key_for_shift(shift, max_retained_total_integer_bits)
    }

    fn key_for_owned_shift_unreplayed(
        &self,
        shift: IndexShift,
        max_retained_total_integer_bits: usize,
    ) -> Result<AffineStartIntegralComplexityKey, AffineParametricOrderingError> {
        let geometry = self.geometry();
        AffineParametricOrderingAlgebra::new(
            self.policy,
            self.sector(),
            &geometry,
            self.constant_positions(),
            self.limits,
            &self.stable_manifest,
            self.key_schema,
        )
        .key_for_owned_shift(shift, max_retained_total_integer_bits)
    }
}

/// Source-neutral implementation of the affine-start ordering algebra.
///
/// The caller supplies an authenticated sector, a geometry-only view, the
/// already classified constant rows, and a stable provenance manifest.  This
/// deliberately knows nothing about residual-unit locators, Boolean terminal
/// locators, or generated-inventory case ordinals.  Consequently V1 and the
/// generated-case ordering can share the exact key and constant-row semantics
/// without weakening either source's replay boundary.
pub(crate) struct AffineParametricOrderingAlgebra<'view, 'source> {
    policy: IntegralOrderingPolicy,
    sector: &'source SectorMask,
    geometry: &'view dyn AffineParametricOrderingGeometry<'source>,
    constant_positions: &'view [usize],
    limits: AffineParametricOrderingLimits,
    stable_manifest: &'view Arc<String>,
    key_schema: &'static str,
}

impl<'view, 'source> AffineParametricOrderingAlgebra<'view, 'source> {
    pub(crate) const fn new(
        policy: IntegralOrderingPolicy,
        sector: &'source SectorMask,
        geometry: &'view dyn AffineParametricOrderingGeometry<'source>,
        constant_positions: &'view [usize],
        limits: AffineParametricOrderingLimits,
        stable_manifest: &'view Arc<String>,
        key_schema: &'static str,
    ) -> Self {
        Self {
            policy,
            sector,
            geometry,
            constant_positions,
            limits,
            stable_manifest,
            key_schema,
        }
    }

    pub(crate) fn arity(&self) -> usize {
        self.sector.arity()
    }

    pub(crate) const fn constant_positions(&self) -> &[usize] {
        self.constant_positions
    }

    pub(crate) const fn max_key_total_integer_bits(&self) -> usize {
        self.limits.max_key_total_integer_bits
    }

    pub(crate) fn constant_start_value(&self, position: usize) -> Option<&'source Integer> {
        self.constant_positions
            .binary_search(&position)
            .ok()
            .and_then(|_| self.geometry.constant(position))
    }

    fn constant_start_value_by_ordinal(
        &self,
        constant_ordinal: usize,
    ) -> Option<(usize, &'source Integer)> {
        let &position = self.constant_positions.get(constant_ordinal)?;
        Some((position, self.geometry.constant(position)?))
    }

    pub(crate) fn constant_row_shift_integer_bit_work_bound(
        &self,
        position: usize,
        displacement: i64,
    ) -> Result<usize, AffineParametricOrderingError> {
        let constant = self
            .constant_start_value(position)
            .ok_or(AffineParametricOrderingError::ReplayMismatch)?;
        self.constant_row_shift_integer_bit_work_bound_from_value(constant, displacement)
    }

    pub(crate) fn constant_row_shift_stays_in_source_sector(
        &self,
        position: usize,
        displacement: i64,
    ) -> Result<bool, AffineParametricOrderingError> {
        Ok(matches!(
            self.classify_constant_row_shift(position, displacement)?
                .transition(),
            AffineConstantRowSectorTransition::StaysInSourceSector
        ))
    }

    pub(crate) fn constant_row_shift_integer_bit_work_bound_by_ordinal(
        &self,
        constant_ordinal: usize,
        displacement: i64,
    ) -> Result<(usize, usize), AffineParametricOrderingError> {
        let (position, constant) = self
            .constant_start_value_by_ordinal(constant_ordinal)
            .ok_or(AffineParametricOrderingError::ReplayMismatch)?;
        Ok((
            position,
            self.constant_row_shift_integer_bit_work_bound_from_value(constant, displacement)?,
        ))
    }

    fn constant_row_shift_integer_bit_work_bound_from_value(
        &self,
        constant: &Integer,
        displacement: i64,
    ) -> Result<usize, AffineParametricOrderingError> {
        let constant_bits = integer_magnitude_bits(constant)?;
        let displacement_bits = i64_magnitude_bits(displacement);
        let shifted_bits = prospective_integer_add_bits(constant_bits, displacement_bits)?;
        check_limit(
            "affine key integer bits",
            shifted_bits,
            self.limits.max_key_integer_bits,
        )?;
        checked_add(
            "affine constant-row shift integer-bit work",
            checked_add(
                "affine constant-row shift integer-bit work",
                constant_bits,
                displacement_bits,
            )?,
            shifted_bits,
        )
    }

    pub(crate) fn classify_constant_row_shift(
        &self,
        position: usize,
        displacement: i64,
    ) -> Result<AffineConstantRowShiftClassification, AffineParametricOrderingError> {
        let constant = self
            .constant_start_value(position)
            .ok_or(AffineParametricOrderingError::ReplayMismatch)?;
        self.classify_constant_row_shift_from_value(position, constant, displacement)
    }

    pub(crate) fn classify_constant_row_shift_by_ordinal(
        &self,
        constant_ordinal: usize,
        displacement: i64,
    ) -> Result<AffineConstantRowShiftClassification, AffineParametricOrderingError> {
        let (position, constant) = self
            .constant_start_value_by_ordinal(constant_ordinal)
            .ok_or(AffineParametricOrderingError::ReplayMismatch)?;
        self.classify_constant_row_shift_from_value(position, constant, displacement)
    }

    pub(crate) fn replay_constant_row_shift_integer_bit_work_bound_by_ordinal(
        &self,
        constant_ordinal: usize,
        displacement: i64,
    ) -> Result<(usize, usize), AffineParametricOrderingError> {
        let (position, constant) = self
            .constant_start_value_by_ordinal(constant_ordinal)
            .ok_or(AffineParametricOrderingError::ReplayMismatch)?;
        Ok((
            position,
            self.replay_constant_row_shift_integer_bit_work_bound_from_value(
                constant,
                displacement,
            )?,
        ))
    }

    pub(crate) fn replay_classify_constant_row_shift_by_ordinal(
        &self,
        constant_ordinal: usize,
        displacement: i64,
    ) -> Result<AffineConstantRowShiftClassification, AffineParametricOrderingError> {
        let (position, constant) = self
            .constant_start_value_by_ordinal(constant_ordinal)
            .ok_or(AffineParametricOrderingError::ReplayMismatch)?;
        let integer_bit_work = self
            .replay_constant_row_shift_integer_bit_work_bound_from_value(constant, displacement)?;
        let source_active = *self
            .sector
            .active_bits()
            .get(position)
            .ok_or(AffineParametricOrderingError::ReplayMismatch)?;
        let activation_threshold = 1i128 - i128::from(displacement);
        let shifted_active = constant >= &Integer::Double(activation_threshold);
        let transition = match (source_active, shifted_active) {
            (true, false) => AffineConstantRowSectorTransition::UniversalActivePinch,
            (false, true) => AffineConstantRowSectorTransition::UniversalInactiveActivation,
            _ => AffineConstantRowSectorTransition::StaysInSourceSector,
        };
        Ok(AffineConstantRowShiftClassification {
            position,
            source_active,
            shifted_active,
            transition,
            integer_bit_work,
        })
    }

    fn replay_constant_row_shift_integer_bit_work_bound_from_value(
        &self,
        constant: &Integer,
        displacement: i64,
    ) -> Result<usize, AffineParametricOrderingError> {
        let constant_bits = integer_magnitude_bits(constant)?;
        let activation_threshold = 1i128 - i128::from(displacement);
        let threshold_bits = i128_magnitude_bits(activation_threshold);
        checked_add(
            "affine constant-row replay integer-bit work",
            constant_bits,
            threshold_bits,
        )
    }

    fn classify_constant_row_shift_from_value(
        &self,
        position: usize,
        constant: &Integer,
        displacement: i64,
    ) -> Result<AffineConstantRowShiftClassification, AffineParametricOrderingError> {
        let admitted_integer_bit_work =
            self.constant_row_shift_integer_bit_work_bound_from_value(constant, displacement)?;
        let mut shifted = constant.clone();
        shifted += Integer::from(displacement);
        check_integer_bits(
            "affine key integer bits",
            &shifted,
            self.limits.max_key_integer_bits,
        )?;
        let source_active = *self
            .sector
            .active_bits()
            .get(position)
            .ok_or(AffineParametricOrderingError::ReplayMismatch)?;
        let shifted_active = shifted >= Integer::from(1);
        let transition = match (source_active, shifted_active) {
            (true, false) => AffineConstantRowSectorTransition::UniversalActivePinch,
            (false, true) => AffineConstantRowSectorTransition::UniversalInactiveActivation,
            _ => AffineConstantRowSectorTransition::StaysInSourceSector,
        };
        let observed_integer_bit_work = checked_add(
            "affine constant-row shift integer-bit work",
            checked_add(
                "affine constant-row shift integer-bit work",
                integer_magnitude_bits(constant)?,
                i64_magnitude_bits(displacement),
            )?,
            integer_magnitude_bits(&shifted)?,
        )?;
        if observed_integer_bit_work > admitted_integer_bit_work {
            return Err(AffineParametricOrderingError::ResourceLimit {
                resource: "affine constant-row shift integer-bit work",
                requested: observed_integer_bit_work,
                limit: admitted_integer_bit_work,
            });
        }
        Ok(AffineConstantRowShiftClassification {
            position,
            source_active,
            shifted_active,
            transition,
            integer_bit_work: observed_integer_bit_work,
        })
    }

    pub(crate) fn compare_sector_prefix_bits(
        &self,
        target_bits: &[bool],
    ) -> Result<Ordering, AffineParametricOrderingError> {
        compare_sector_prefix_bits_for_policy(self.policy, self.sector.active_bits(), target_bits)
    }

    pub(crate) fn key_for_shift(
        &self,
        shift: &IndexShift,
        max_retained_total_integer_bits: usize,
    ) -> Result<AffineStartIntegralComplexityKey, AffineParametricOrderingError> {
        let arity = self.preflight_shift_shape(shift)?;
        let shift = try_copy_index_shift(shift, arity)?;
        self.key_for_owned_shift_preflighted(shift, arity, max_retained_total_integer_bits)
    }

    pub(crate) fn key_for_owned_shift(
        &self,
        shift: IndexShift,
        max_retained_total_integer_bits: usize,
    ) -> Result<AffineStartIntegralComplexityKey, AffineParametricOrderingError> {
        let arity = self.preflight_shift_shape(&shift)?;
        self.key_for_owned_shift_preflighted(shift, arity, max_retained_total_integer_bits)
    }

    fn preflight_shift_shape(
        &self,
        shift: &IndexShift,
    ) -> Result<usize, AffineParametricOrderingError> {
        let arity = self.sector.arity();
        if shift.arity() != arity {
            return Err(AffineParametricOrderingError::WrongShiftArity {
                expected: arity,
                actual: shift.arity(),
            });
        }
        check_limit("ambient arity", arity, self.limits.max_arity)?;
        check_limit(
            "affine order-key components",
            key_component_count(arity)?,
            self.limits.max_key_components,
        )?;
        checked_mul("affine key shift bytes", arity, std::mem::size_of::<i64>())?;
        Ok(arity)
    }

    fn key_for_owned_shift_preflighted(
        &self,
        shift: IndexShift,
        arity: usize,
        max_retained_total_integer_bits: usize,
    ) -> Result<AffineStartIntegralComplexityKey, AffineParametricOrderingError> {
        let mut bits = Vec::new();
        bits.try_reserve_exact(arity).map_err(|_| {
            AffineParametricOrderingError::AllocationFailure {
                resource: "affine key sector bits",
                requested: arity,
            }
        })?;
        let mut propagators = 0usize;
        let mut corner = Integer::from(0);
        let mut dots = Integer::from(0);
        let mut numerators = Integer::from(0);
        for (position, (&source_active, &displacement)) in self
            .sector
            .active_bits()
            .iter()
            .zip(shift.values())
            .enumerate()
        {
            let (active, excess) = self.shifted_excess(position, source_active, displacement)?;
            bits.push(active);
            check_prospective_integer_add(
                "affine key integer bits",
                integer_magnitude_bits(&corner)?,
                integer_magnitude_bits(&excess)?,
                self.limits.max_key_integer_bits,
            )?;
            corner += &excess;
            check_integer_bits(
                "affine key integer bits",
                &corner,
                self.limits.max_key_integer_bits,
            )?;
            if active {
                propagators = checked_add("affine propagator count", propagators, 1)?;
                check_prospective_integer_add(
                    "affine key integer bits",
                    integer_magnitude_bits(&dots)?,
                    integer_magnitude_bits(&excess)?,
                    self.limits.max_key_integer_bits,
                )?;
                dots += &excess;
                check_integer_bits(
                    "affine key integer bits",
                    &dots,
                    self.limits.max_key_integer_bits,
                )?;
            } else {
                check_prospective_integer_add(
                    "affine key integer bits",
                    integer_magnitude_bits(&numerators)?,
                    integer_magnitude_bits(&excess)?,
                    self.limits.max_key_integer_bits,
                )?;
                numerators += &excess;
                check_integer_bits(
                    "affine key integer bits",
                    &numerators,
                    self.limits.max_key_integer_bits,
                )?;
            }
        }

        let effective_total_limit =
            max_retained_total_integer_bits.min(self.limits.max_key_total_integer_bits);
        let mut retained_integer_bits = 0usize;
        for total in [&corner, &dots, &numerators] {
            retained_integer_bits = bounded_add(
                "affine key total integer bits",
                retained_integer_bits,
                integer_magnitude_bits(total)?,
                effective_total_limit,
            )?;
        }

        let mut excesses = Vec::new();
        excesses.try_reserve_exact(arity).map_err(|_| {
            AffineParametricOrderingError::AllocationFailure {
                resource: "affine key signed excesses",
                requested: arity,
            }
        })?;
        for (position, (&source_active, &displacement)) in self
            .sector
            .active_bits()
            .iter()
            .zip(shift.values())
            .enumerate()
        {
            let (active, excess) = self.shifted_excess(position, source_active, displacement)?;
            if bits.get(position).copied() != Some(active) {
                return Err(AffineParametricOrderingError::ReplayMismatch);
            }
            retained_integer_bits = bounded_add(
                "affine key total integer bits",
                retained_integer_bits,
                integer_magnitude_bits(&excess)?,
                effective_total_limit,
            )?;
            excesses.push(excess);
        }
        let formal_sector = SectorMask::try_from_preallocated(bits)
            .map_err(|_| AffineParametricOrderingError::ReplayMismatch)?;
        Ok(AffineStartIntegralComplexityKey {
            schema: AFFINE_START_INTEGRAL_COMPLEXITY_KEY_V1_SCHEMA,
            key_schema: self.key_schema,
            policy: self.policy,
            arity,
            propagators,
            formal_sector: Arc::new(formal_sector),
            corner_distance_offset: Arc::new(corner),
            dots_offset: Arc::new(dots),
            numerators_offset: Arc::new(numerators),
            signed_index_excess: Arc::new(excesses),
            retained_integer_bits,
            shift: Arc::new(shift),
            ordering_manifest: self.stable_manifest.clone(),
            diagnostic_limit_bytes: self.limits.max_key_diagnostic_bytes,
        })
    }

    pub(crate) fn replay_key(
        &self,
        key: &AffineStartIntegralComplexityKey,
    ) -> Result<(), AffineParametricOrderingError> {
        if key.schema != AFFINE_START_INTEGRAL_COMPLEXITY_KEY_V1_SCHEMA
            || key.key_schema != self.key_schema
        {
            return Err(AffineParametricOrderingError::SchemaMismatch);
        }
        if key.ordering_manifest.as_ref() != self.stable_manifest.as_ref() {
            return Err(AffineParametricOrderingError::KeyOrderingMismatch);
        }
        let replayed =
            self.key_for_shift(key.shift.as_ref(), self.limits.max_key_total_integer_bits)?;
        if replayed == *key {
            Ok(())
        } else {
            Err(AffineParametricOrderingError::ReplayMismatch)
        }
    }

    fn shifted_excess(
        &self,
        position: usize,
        source_active: bool,
        displacement: i64,
    ) -> Result<(bool, Integer), AffineParametricOrderingError> {
        let result = if let Some(constant) = self.constant_start_value(position) {
            let constant_bits = integer_magnitude_bits(constant)?;
            let displacement_bits = i64_magnitude_bits(displacement);
            check_prospective_integer_add(
                "affine key integer bits",
                constant_bits,
                displacement_bits,
                self.limits.max_key_integer_bits,
            )?;
            let mut shifted = constant.clone();
            shifted += Integer::from(displacement);
            check_integer_bits(
                "affine key integer bits",
                &shifted,
                self.limits.max_key_integer_bits,
            )?;
            if shifted >= Integer::from(1) {
                check_prospective_integer_add(
                    "affine key integer bits",
                    integer_magnitude_bits(&shifted)?,
                    1,
                    self.limits.max_key_integer_bits,
                )?;
                let mut excess = shifted;
                excess -= Integer::from(1);
                (true, excess)
            } else {
                (false, -shifted)
            }
        } else {
            check_limit(
                "affine key integer bits",
                i64_magnitude_bits(displacement),
                self.limits.max_key_integer_bits,
            )?;
            let displacement = Integer::from(displacement);
            (
                source_active,
                if source_active {
                    displacement
                } else {
                    -displacement
                },
            )
        };
        check_integer_bits(
            "affine key integer bits",
            &result.1,
            self.limits.max_key_integer_bits,
        )?;
        Ok(result)
    }
}

fn arc_payload_control_and_padding_byte_bound<T>() -> Option<usize> {
    size_of::<AtomicUsize>()
        .checked_mul(2)?
        .checked_add(align_of::<T>().saturating_sub(1))?
        .checked_add(size_of::<T>())
}

fn arc_vec_owned_byte_bound<T>(value: &Arc<Vec<T>>) -> Option<usize> {
    arc_payload_control_and_padding_byte_bound::<Vec<T>>()?
        .checked_add(value.capacity().checked_mul(size_of::<T>())?)
}

fn arc_string_owned_byte_bound(value: &Arc<String>) -> Option<usize> {
    arc_payload_control_and_padding_byte_bound::<String>()?.checked_add(value.capacity())
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AffineStartIntegralComplexityKey {
    schema: &'static str,
    key_schema: &'static str,
    policy: IntegralOrderingPolicy,
    arity: usize,
    propagators: usize,
    formal_sector: Arc<SectorMask>,
    corner_distance_offset: Arc<Integer>,
    dots_offset: Arc<Integer>,
    numerators_offset: Arc<Integer>,
    signed_index_excess: Arc<Vec<Integer>>,
    retained_integer_bits: usize,
    shift: Arc<IndexShift>,
    ordering_manifest: Arc<String>,
    diagnostic_limit_bytes: usize,
}

impl AffineStartIntegralComplexityKey {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    pub const fn key_schema(&self) -> &'static str {
        self.key_schema
    }
    pub const fn policy(&self) -> IntegralOrderingPolicy {
        self.policy
    }
    pub const fn arity(&self) -> usize {
        self.arity
    }
    pub const fn propagators(&self) -> usize {
        self.propagators
    }
    pub fn formal_sector(&self) -> &SectorMask {
        self.formal_sector.as_ref()
    }
    pub fn corner_distance_offset(&self) -> &Integer {
        self.corner_distance_offset.as_ref()
    }
    pub fn dots_offset(&self) -> &Integer {
        self.dots_offset.as_ref()
    }
    pub fn numerators_offset(&self) -> &Integer {
        self.numerators_offset.as_ref()
    }
    pub fn signed_index_excess(&self) -> &[Integer] {
        self.signed_index_excess.as_slice()
    }
    /// Exact sum of magnitude bits in the retained `Integer` payload.  Zero
    /// values contribute zero bits; fixed-size metadata is charged separately
    /// by prepare-point component budgets.
    pub const fn retained_integer_bits(&self) -> usize {
        self.retained_integer_bits
    }
    pub fn shift(&self) -> &IndexShift {
        self.shift.as_ref()
    }
    pub(crate) fn into_shift(self) -> Result<IndexShift, AffineParametricOrderingError> {
        Arc::try_unwrap(self.shift).map_err(|_| AffineParametricOrderingError::ReplayMismatch)
    }
    pub fn ordering_manifest(&self) -> &str {
        self.ordering_manifest.as_str()
    }

    /// Complete bytes owned by this key apart from the generated ordering's
    /// shared manifest allocation. The manifest `Arc` handle is inline in the
    /// key and is therefore covered by `size_of::<Self>()`; its pointee is
    /// charged exactly once by the ordering certificate. All other `Arc`
    /// pointees are minted for this key and are included here.
    pub(crate) fn owned_retained_byte_bound(&self) -> Option<usize> {
        let mut bytes = size_of::<Self>();
        bytes = bytes.checked_add(
            arc_payload_control_and_padding_byte_bound::<SectorMask>()?
                .checked_add(self.formal_sector.owned_retained_byte_bound()?)?,
        )?;
        for value in [
            &self.corner_distance_offset,
            &self.dots_offset,
            &self.numerators_offset,
        ] {
            bytes = bytes.checked_add(
                arc_payload_control_and_padding_byte_bound::<Integer>()?
                    .checked_add(integer_owned_heap_bytes(value.as_ref())?)?,
            )?;
        }
        bytes = bytes.checked_add(arc_vec_owned_byte_bound(&self.signed_index_excess)?)?;
        for value in self.signed_index_excess.iter() {
            bytes = bytes.checked_add(integer_owned_heap_bytes(value)?)?;
        }
        bytes.checked_add(
            arc_payload_control_and_padding_byte_bound::<IndexShift>()?
                .checked_add(self.shift.owned_retained_byte_bound()?)?,
        )
    }

    /// Render the complete replay diagnostic under the ordering's explicit
    /// byte ceiling. Every GMP-backed decimal first receives a prospective
    /// sign-plus-digit check before its formatter can allocate. That decimal
    /// estimate is deliberately conservative and may reject a limit which
    /// would fit the eventual decimal; the approximation gap grows slowly
    /// with bit length. The persisted map identity uses an exact
    /// sign-magnitude hexadecimal encoding instead.
    pub fn try_to_stable_string(&self) -> Result<String, AffineParametricOrderingError> {
        const RESOURCE: &str = "affine key diagnostic bytes";
        let mut output = BoundedManifestBuilder::new(self.diagnostic_limit_bytes);
        write!(
            &mut output,
            "{}|ordering-bytes={}|ordering={}|arity={}|propagators={}|sector={}|corner-offset=",
            self.schema,
            self.ordering_manifest.len(),
            self.ordering_manifest.as_str(),
            self.arity,
            self.propagators,
            self.formal_sector.as_ref(),
        )
        .map_err(|_| output.error(RESOURCE))?;
        write_decimal_integer(&mut output, self.corner_distance_offset.as_ref(), RESOURCE)?;
        output
            .write_str("|dots-offset=")
            .map_err(|_| output.error(RESOURCE))?;
        write_decimal_integer(&mut output, self.dots_offset.as_ref(), RESOURCE)?;
        output
            .write_str("|numerators-offset=")
            .map_err(|_| output.error(RESOURCE))?;
        write_decimal_integer(&mut output, self.numerators_offset.as_ref(), RESOURCE)?;
        write!(
            &mut output,
            "|integer-bits={}|excess=[",
            self.retained_integer_bits,
        )
        .map_err(|_| output.error(RESOURCE))?;
        write_decimal_integer_values(&mut output, self.signed_index_excess.as_slice(), RESOURCE)?;
        output
            .write_str("]|shift=[")
            .map_err(|_| output.error(RESOURCE))?;
        write_values(&mut output, self.shift.as_ref().values(), RESOURCE)?;
        output.write_char(']').map_err(|_| output.error(RESOURCE))?;
        Ok(output.finish())
    }
}

fn integer_owned_heap_bytes(value: &Integer) -> Option<usize> {
    match value {
        Integer::Single(_) | Integer::Double(_) => Some(0),
        Integer::Large(value) => value.capacity().checked_add(7).map(|bits| bits / 8),
    }
}

impl Ord for AffineStartIntegralComplexityKey {
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
            .then_with(|| self.retained_integer_bits.cmp(&other.retained_integer_bits))
            .then_with(|| self.shift.cmp(&other.shift))
            .then_with(|| self.ordering_manifest.cmp(&other.ordering_manifest))
            .then_with(|| {
                self.diagnostic_limit_bytes
                    .cmp(&other.diagnostic_limit_bytes)
            })
            .then_with(|| self.schema.cmp(other.schema))
            .then_with(|| self.key_schema.cmp(other.key_schema))
    }
}

impl PartialOrd for AffineStartIntegralComplexityKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AffineParametricOrderingError {
    SchemaMismatch,
    ReplayMismatch,
    BranchReplayAuthorityRequired,
    ReplayAuthoritySourceMismatch,
    BranchSourceHasNoAffineMap,
    WrongContext,
    WrongSectorArity {
        expected: usize,
        actual: usize,
    },
    SourceSectorMismatch,
    WrongShiftArity {
        expected: usize,
        actual: usize,
    },
    ConstantStartOutsideSourceSector {
        position: usize,
        /// Exact magnitude-bit census of the rejected constant.  Keeping only
        /// bounded scalar metadata prevents an error path from cloning and
        /// later decimal-formatting an arbitrary GMP allocation.
        constant_integer_bits: usize,
        constant_is_negative: bool,
        source_active: bool,
    },
    KeyOrderingMismatch,
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
    Map(ResidualUnitAffineIndexMapError),
    Branch(ResidualAffineBranchSystemError),
}

impl fmt::Display for AffineParametricOrderingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("affine-start ordering schema mismatch"),
            Self::ReplayMismatch => formatter.write_str("affine-start ordering replay mismatch"),
            Self::BranchReplayAuthorityRequired => formatter.write_str(
                "affine-start Boolean branch replay requires its family, context, and exact source cover",
            ),
            Self::ReplayAuthoritySourceMismatch => formatter.write_str(
                "affine-start replay authority does not match the retained source kind",
            ),
            Self::BranchSourceHasNoAffineMap => formatter.write_str(
                "affine-start Boolean branch does not have a guarded affine-map outcome",
            ),
            Self::WrongContext => {
                formatter.write_str("affine-start ordering belongs to another K(n) context")
            }
            Self::WrongSectorArity { expected, actual } => write!(
                formatter,
                "affine-start sector arity is {actual}, expected {expected}"
            ),
            Self::SourceSectorMismatch => formatter.write_str(
                "affine-start ordering sector differs from the map's authenticated source sector",
            ),
            Self::WrongShiftArity { expected, actual } => write!(
                formatter,
                "affine-start shift arity is {actual}, expected {expected}"
            ),
            Self::ConstantStartOutsideSourceSector {
                position,
                constant_integer_bits,
                constant_is_negative,
                source_active,
            } => write!(
                formatter,
                "constant affine start component {position} (negative={constant_is_negative}, magnitude-bits={constant_integer_bits}) is outside the source {} half-line",
                if *source_active { "active" } else { "inactive" }
            ),
            Self::KeyOrderingMismatch => {
                formatter.write_str("affine-start key belongs to another ordering context")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "affine-start {resource} requires {requested}, exceeding configured limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "affine-start {resource} count overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "affine-start {resource} could not reserve {requested} entries"
            ),
            Self::Map(error) => error.fmt(formatter),
            Self::Branch(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for AffineParametricOrderingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Map(error) => Some(error),
            Self::Branch(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ResidualUnitAffineIndexMapError> for AffineParametricOrderingError {
    fn from(value: ResidualUnitAffineIndexMapError) -> Self {
        Self::Map(value)
    }
}

impl From<ResidualAffineBranchSystemError> for AffineParametricOrderingError {
    fn from(value: ResidualAffineBranchSystemError) -> Self {
        Self::Branch(value)
    }
}

struct AffineMapMetadataPreflight {
    arity: usize,
    matrix_entries: usize,
    constant_positions: Vec<usize>,
    symbolic_positions: Vec<usize>,
    largest_affine_integer_bits: usize,
}

/// Reject-only checks over already-retained certificate metadata.  This runs
/// before the expensive source-certificate replay.  None of these checks
/// authenticate the map; successful preflight is always followed by replay.
fn preflight_untrusted_source_metadata(
    context: &ParametricCoefficientContext,
    sector: &SectorMask,
    source: &AffineStartSourceCertificate,
    limits: AffineParametricOrderingLimits,
) -> Result<AffineMapMetadataPreflight, AffineParametricOrderingError> {
    // A string equality can scan the whole fingerprint.  Bound both operand
    // lengths before inspecting content; each fingerprint is also copied
    // verbatim into the bounded ordering manifest, so that manifest ceiling
    // is an authenticated upper bound for this comparison.
    let retained_context_fingerprint = source.context_fingerprint();
    let supplied_context_fingerprint = context.fingerprint();
    check_limit(
        "affine context fingerprint comparison bytes",
        retained_context_fingerprint.len(),
        limits.max_manifest_bytes,
    )?;
    check_limit(
        "affine context fingerprint comparison bytes",
        supplied_context_fingerprint.len(),
        limits.max_manifest_bytes,
    )?;
    if retained_context_fingerprint.len() != supplied_context_fingerprint.len()
        || retained_context_fingerprint != supplied_context_fingerprint
    {
        return Err(AffineParametricOrderingError::WrongContext);
    }
    if matches!(
        source,
        AffineStartSourceCertificate::ResidualBooleanBranch(branch) if branch.affine_map().is_none()
    ) {
        return Err(AffineParametricOrderingError::BranchSourceHasNoAffineMap);
    }
    let geometry = AffineStartGeometryRef { source };
    let arity = geometry.ambient_arity();
    check_limit("affine ordering arity", arity, limits.max_arity)?;
    if sector.arity() != arity {
        return Err(AffineParametricOrderingError::WrongSectorArity {
            expected: arity,
            actual: sector.arity(),
        });
    }
    if source.source_sector() != sector {
        return Err(AffineParametricOrderingError::SourceSectorMismatch);
    }
    let free_positions = geometry.free_positions();
    let free_count = free_positions.len();
    check_limit(
        "affine ordering free positions",
        free_count,
        limits.max_free_positions,
    )?;
    check_limit(
        "affine order-key components",
        key_component_count(arity)?,
        limits.max_key_components,
    )?;
    let matrix_entries = checked_mul("affine matrix entries inspected", arity, free_count)?;
    check_limit(
        "affine matrix entries inspected",
        matrix_entries,
        limits.max_matrix_entries_inspected,
    )?;
    let map_identity_lower_bound = match source {
        AffineStartSourceCertificate::ResidualUnit(map) => checked_add(
            "affine map identity bytes",
            map.source_partition_identity().len(),
            map.local_manifest().len(),
        )?,
        AffineStartSourceCertificate::ResidualBooleanBranch(branch) => checked_add(
            "affine map identity bytes",
            branch.source_partition_identity().len(),
            checked_add(
                "affine map identity bytes",
                branch.family_fingerprint().len(),
                branch.context_fingerprint().len(),
            )?,
        )?,
    };
    check_limit(
        "affine map identity bytes",
        map_identity_lower_bound,
        limits.max_map_identity_bytes,
    )?;
    // The complete manifest contains both identities and the context verbatim,
    // plus delimiters and metadata.  This lower bound can reject a tiny limit
    // cheaply; exact bounded rendering after replay remains authoritative.
    let manifest_lower_bound = checked_add(
        "affine ordering manifest bytes",
        map_identity_lower_bound,
        context.fingerprint().len(),
    )?;
    check_limit(
        "affine ordering manifest bytes",
        manifest_lower_bound,
        limits.max_manifest_bytes,
    )?;

    let mut constant_positions = Vec::new();
    constant_positions
        .try_reserve_exact(arity.min(limits.max_constant_positions))
        .map_err(|_| AffineParametricOrderingError::AllocationFailure {
            resource: "constant affine positions",
            requested: arity.min(limits.max_constant_positions),
        })?;
    let mut symbolic_positions = Vec::new();
    symbolic_positions
        .try_reserve_exact(arity.min(limits.max_symbolic_positions))
        .map_err(|_| AffineParametricOrderingError::AllocationFailure {
            resource: "symbolic affine positions",
            requested: arity.min(limits.max_symbolic_positions),
        })?;
    let mut largest_affine_integer_bits = 0usize;
    let mut previous_free = None;
    for &free_position in free_positions {
        if free_position >= arity || previous_free.is_some_and(|value| value >= free_position) {
            return Err(AffineParametricOrderingError::ReplayMismatch);
        }
        previous_free = Some(free_position);
    }
    for position in 0..arity {
        let constant = geometry
            .constant(position)
            .ok_or(AffineParametricOrderingError::ReplayMismatch)?;
        let constant_bits = integer_magnitude_bits(constant)?;
        check_limit(
            "affine ordering integer bits",
            constant_bits,
            limits.max_affine_integer_bits,
        )?;
        largest_affine_integer_bits = largest_affine_integer_bits.max(constant_bits);
        let mut constant_row = true;
        for free_ordinal in 0..free_count {
            let coefficient = geometry
                .linear_coefficient(position, free_ordinal)
                .ok_or(AffineParametricOrderingError::ReplayMismatch)?;
            let bits = integer_magnitude_bits(coefficient)?;
            check_limit(
                "affine ordering integer bits",
                bits,
                limits.max_affine_integer_bits,
            )?;
            largest_affine_integer_bits = largest_affine_integer_bits.max(bits);
            constant_row &= coefficient.is_zero();
        }
        if constant_row {
            let requested = checked_add("constant affine positions", constant_positions.len(), 1)?;
            check_limit(
                "constant affine positions",
                requested,
                limits.max_constant_positions,
            )?;
            let exact_active = constant >= &Integer::from(1);
            let source_active = sector.active_bits()[position];
            if exact_active != source_active {
                return Err(
                    AffineParametricOrderingError::ConstantStartOutsideSourceSector {
                        position,
                        constant_integer_bits: constant_bits,
                        constant_is_negative: constant.is_negative(),
                        source_active,
                    },
                );
            }
            constant_positions.push(position);
        } else {
            let requested = checked_add("symbolic affine positions", symbolic_positions.len(), 1)?;
            check_limit(
                "symbolic affine positions",
                requested,
                limits.max_symbolic_positions,
            )?;
            symbolic_positions.push(position);
        }
    }
    if checked_add(
        "classified affine positions",
        constant_positions.len(),
        symbolic_positions.len(),
    )? != arity
    {
        return Err(AffineParametricOrderingError::ReplayMismatch);
    }
    Ok(AffineMapMetadataPreflight {
        arity,
        matrix_entries,
        constant_positions,
        symbolic_positions,
        largest_affine_integer_bits,
    })
}

fn affine_source_identity(
    source: &AffineStartSourceCertificate,
    limit: usize,
) -> Result<String, AffineParametricOrderingError> {
    let mut output = BoundedManifestBuilder::new(limit);
    match source {
        AffineStartSourceCertificate::ResidualUnit(map) => {
            write!(
                &mut output,
                "affine-source-v1|integer-encoding=sign-magnitude-hex-v1|kind=residual-unit|source-bytes={}:{}|local-bytes={}:{}",
                map.source_partition_identity().len(),
                map.source_partition_identity(),
                map.local_manifest().len(),
                map.local_manifest(),
            )
            .map_err(|_| output.error("affine map identity bytes"))?;
        }
        AffineStartSourceCertificate::ResidualBooleanBranch(branch) => {
            let cover = branch.source_cover();
            let terminal = branch
                .ready_terminal()
                .ok_or(AffineParametricOrderingError::ReplayMismatch)?;
            let geometry = AffineStartGeometryRef { source };
            write!(
                &mut output,
                "affine-source-v1|integer-encoding=sign-magnitude-hex-v1|kind=residual-boolean-branch|family-bytes={}:{}\
|context-bytes={}:{}|partition-bytes={}:{}|sector={}\
|work={}|case={}|terminal={}",
                branch.family_fingerprint().len(),
                branch.family_fingerprint(),
                branch.context_fingerprint().len(),
                branch.context_fingerprint(),
                branch.source_partition_identity().len(),
                branch.source_partition_identity(),
                cover.sector(),
                cover.source_work_item_ordinal(),
                cover.source_case().value(),
                branch.ready_terminal_ordinal(),
            )
            .map_err(|_| output.error("affine map identity bytes"))?;
            write_branch_atom_identity(&mut output, "zero", terminal.equal_zero_atoms(), branch)?;
            write_branch_atom_identity(&mut output, "nonzero", terminal.nonzero_atoms(), branch)?;
            write_affine_geometry_identity(
                &mut output,
                geometry.ambient_arity(),
                geometry.free_positions(),
                |position| geometry.constant(position),
                |position, free_ordinal| geometry.linear_coefficient(position, free_ordinal),
            )?;
        }
    }
    Ok(output.finish())
}

fn write_branch_atom_identity(
    output: &mut BoundedManifestBuilder,
    label: &'static str,
    ordinals: &[usize],
    branch: &ResidualAffineBranchSystemCertificate,
) -> Result<(), AffineParametricOrderingError> {
    let coverage = branch.source_cover().source_queue().discovery().coverage();
    write_branch_atom_entries(
        output,
        label,
        ordinals.len(),
        ordinals.iter().map(|&ordinal| {
            coverage
                .structural_locus(ordinal)
                .map(|polynomial| (ordinal, polynomial))
                .ok_or(AffineParametricOrderingError::ReplayMismatch)
        }),
    )
}

fn write_branch_atom_entries<'a>(
    output: &mut BoundedManifestBuilder,
    label: &'static str,
    atom_count: usize,
    atoms: impl IntoIterator<
        Item = Result<(usize, &'a ParametricPolynomial), AffineParametricOrderingError>,
    >,
) -> Result<(), AffineParametricOrderingError> {
    write!(output, "|{label}:{atom_count}[")
        .map_err(|_| output.error("affine map identity bytes"))?;
    let mut observed_count = 0usize;
    for (entry, atom) in atoms.into_iter().enumerate() {
        let (ordinal, polynomial) = atom?;
        if entry != 0 {
            output
                .write_char(';')
                .map_err(|_| output.error("affine map identity bytes"))?;
        }
        write!(output, "{ordinal}:").map_err(|_| output.error("affine map identity bytes"))?;
        write_polynomial_identity(output, polynomial)?;
        observed_count = observed_count.checked_add(1).ok_or(
            AffineParametricOrderingError::ResourceCountOverflow {
                resource: "affine map identity atoms",
            },
        )?;
    }
    if observed_count != atom_count {
        return Err(AffineParametricOrderingError::ReplayMismatch);
    }
    output
        .write_char(']')
        .map_err(|_| output.error("affine map identity bytes"))?;
    Ok(())
}

fn write_affine_geometry_identity<'a>(
    output: &mut BoundedManifestBuilder,
    arity: usize,
    free_positions: &[usize],
    mut constant: impl FnMut(usize) -> Option<&'a Integer>,
    mut linear_coefficient: impl FnMut(usize, usize) -> Option<&'a Integer>,
) -> Result<(), AffineParametricOrderingError> {
    write!(
        output,
        "|geometry=arity:{arity}|free:{}[",
        free_positions.len(),
    )
    .map_err(|_| output.error("affine map identity bytes"))?;
    write_positions_identity(output, free_positions)?;
    write!(output, "]|b:{arity}[").map_err(|_| output.error("affine map identity bytes"))?;
    for position in 0..arity {
        if position != 0 {
            output
                .write_char(',')
                .map_err(|_| output.error("affine map identity bytes"))?;
        }
        let value = constant(position).ok_or(AffineParametricOrderingError::ReplayMismatch)?;
        write_identity_integer(output, value)?;
    }
    write!(output, "]|A:{arity},{}[", free_positions.len())
        .map_err(|_| output.error("affine map identity bytes"))?;
    for position in 0..arity {
        if position != 0 {
            output
                .write_char(';')
                .map_err(|_| output.error("affine map identity bytes"))?;
        }
        for free_ordinal in 0..free_positions.len() {
            if free_ordinal != 0 {
                output
                    .write_char(',')
                    .map_err(|_| output.error("affine map identity bytes"))?;
            }
            let value = linear_coefficient(position, free_ordinal)
                .ok_or(AffineParametricOrderingError::ReplayMismatch)?;
            write_identity_integer(output, value)?;
        }
    }
    output
        .write_char(']')
        .map_err(|_| output.error("affine map identity bytes"))?;
    Ok(())
}

fn write_polynomial_identity(
    output: &mut BoundedManifestBuilder,
    polynomial: &ParametricPolynomial,
) -> Result<(), AffineParametricOrderingError> {
    let raw = polynomial.raw();
    write!(output, "{},{}[", raw.variables.len(), raw.nterms())
        .map_err(|_| output.error("affine map identity bytes"))?;
    for term in 0..raw.nterms() {
        if term != 0 {
            output
                .write_char(';')
                .map_err(|_| output.error("affine map identity bytes"))?;
        }
        write_identity_integer(output, &raw.coefficients[term])?;
        output
            .write_char(':')
            .map_err(|_| output.error("affine map identity bytes"))?;
        for (variable, exponent) in raw.exponents(term).iter().enumerate() {
            if variable != 0 {
                output
                    .write_char(',')
                    .map_err(|_| output.error("affine map identity bytes"))?;
            }
            write!(output, "{exponent}").map_err(|_| output.error("affine map identity bytes"))?;
        }
    }
    output
        .write_char(']')
        .map_err(|_| output.error("affine map identity bytes"))?;
    Ok(())
}

/// Write the map-identity integer encoding exactly as sign plus lowercase
/// hexadecimal magnitude. Its byte count is `sign + ceil(bits/4)` (one digit
/// for zero), so the identity cap is exact even for a GMP-backed value and is
/// checked before the hexadecimal formatter can allocate.
fn write_identity_integer(
    output: &mut BoundedManifestBuilder,
    value: &Integer,
) -> Result<(), AffineParametricOrderingError> {
    const RESOURCE: &str = "affine map identity bytes";
    let bits = integer_magnitude_bits(value)?;
    let digits = if bits == 0 {
        1
    } else {
        checked_add(RESOURCE, bits, 3)? / 4
    };
    let exact_bytes = checked_add(RESOURCE, digits, usize::from(value.is_negative()))?;
    output.preflight_additional(exact_bytes, RESOURCE)?;
    let start = output.value.len();
    match value {
        Integer::Single(value) => {
            if value.is_negative() {
                output.write_char('-').map_err(|_| output.error(RESOURCE))?;
            }
            write!(output, "{:x}", value.unsigned_abs()).map_err(|_| output.error(RESOURCE))?;
        }
        Integer::Double(value) => {
            if value.is_negative() {
                output.write_char('-').map_err(|_| output.error(RESOURCE))?;
            }
            write!(output, "{:x}", value.unsigned_abs()).map_err(|_| output.error(RESOURCE))?;
        }
        Integer::Large(value) => {
            if value.is_negative() {
                output.write_char('-').map_err(|_| output.error(RESOURCE))?;
            }
            write!(output, "{:x}", value.as_abs()).map_err(|_| output.error(RESOURCE))?;
        }
    }
    if output.value.len().checked_sub(start) != Some(exact_bytes) {
        return Err(AffineParametricOrderingError::ReplayMismatch);
    }
    Ok(())
}

fn unsigned_decimal_digits(mut value: u128) -> usize {
    let mut digits = 1usize;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

/// Prospective sign-plus-decimal bound used only for human diagnostics.
/// Inline values are counted exactly. For GMP values, `30103 / 100000` is a
/// strict upper rational approximation to `log10(2)`, so this may reject a
/// fitting limit conservatively (with a gap that grows slowly with bit
/// length) but cannot allow formatting past the configured cap.
fn prospective_decimal_integer_bytes(
    value: &Integer,
    resource: &'static str,
) -> Result<usize, AffineParametricOrderingError> {
    let digits = match value {
        Integer::Single(value) => unsigned_decimal_digits(u128::from(value.unsigned_abs())),
        Integer::Double(value) => unsigned_decimal_digits(value.unsigned_abs()),
        Integer::Large(_) => {
            let bits = integer_magnitude_bits(value)?;
            if bits == 0 {
                1
            } else {
                let scaled = checked_mul(resource, bits, 30_103)?;
                checked_add(resource, scaled, 99_999)? / 100_000
            }
        }
    };
    checked_add(resource, digits, usize::from(value.is_negative()))
}

fn write_decimal_integer(
    output: &mut BoundedManifestBuilder,
    value: &Integer,
    resource: &'static str,
) -> Result<(), AffineParametricOrderingError> {
    let prospective_bytes = prospective_decimal_integer_bytes(value, resource)?;
    output.preflight_additional(prospective_bytes, resource)?;
    write!(output, "{value}").map_err(|_| output.error(resource))?;
    Ok(())
}

fn write_decimal_integer_values(
    output: &mut BoundedManifestBuilder,
    values: &[Integer],
    resource: &'static str,
) -> Result<(), AffineParametricOrderingError> {
    for (ordinal, value) in values.iter().enumerate() {
        if ordinal != 0 {
            output.write_char(',').map_err(|_| output.error(resource))?;
        }
        write_decimal_integer(output, value, resource)?;
    }
    Ok(())
}

fn write_positions_identity(
    output: &mut BoundedManifestBuilder,
    positions: &[usize],
) -> Result<(), AffineParametricOrderingError> {
    for (ordinal, position) in positions.iter().enumerate() {
        if ordinal != 0 {
            output
                .write_char(',')
                .map_err(|_| output.error("affine map identity bytes"))?;
        }
        write!(output, "{position}").map_err(|_| output.error("affine map identity bytes"))?;
    }
    Ok(())
}

fn ordering_manifest(
    context: &ParametricCoefficientContext,
    policy: IntegralOrderingPolicy,
    sector: &SectorMask,
    source_identity: &str,
    constant_positions: &[usize],
    symbolic_positions: &[usize],
    limits: AffineParametricOrderingLimits,
    limit: usize,
) -> Result<String, AffineParametricOrderingError> {
    let mut output = BoundedManifestBuilder::new(limit);
    write!(
        &mut output,
        "{AFFINE_START_PARAMETRIC_ELIMINATION_ORDERING_V1_SCHEMA}|policy={}\
|key-schema={RUSTRED_AFFINE_START_UNSHIFTED_ORDER_V1_KEY_SCHEMA}|context-bytes={}:{}\
|sector={}|limits={},{},{},{},{},{},{},{},{},{},{},{}|map-identity-bytes={}:{}|constant=[",
        policy.stable_id(),
        context.fingerprint().len(),
        context.fingerprint(),
        sector,
        limits.max_arity,
        limits.max_free_positions,
        limits.max_constant_positions,
        limits.max_symbolic_positions,
        limits.max_matrix_entries_inspected,
        limits.max_key_components,
        limits.max_affine_integer_bits,
        limits.max_key_integer_bits,
        limits.max_key_total_integer_bits,
        limits.max_map_identity_bytes,
        limits.max_manifest_bytes,
        limits.max_key_diagnostic_bytes,
        source_identity.len(),
        source_identity,
    )
    .map_err(|_| output.error("affine ordering manifest bytes"))?;
    write_positions(&mut output, constant_positions)?;
    output
        .write_str("]|symbolic=[")
        .map_err(|_| output.error("affine ordering manifest bytes"))?;
    write_positions(&mut output, symbolic_positions)?;
    output
        .write_char(']')
        .map_err(|_| output.error("affine ordering manifest bytes"))?;
    Ok(output.finish())
}

fn write_positions(
    output: &mut BoundedManifestBuilder,
    positions: &[usize],
) -> Result<(), AffineParametricOrderingError> {
    for (ordinal, position) in positions.iter().enumerate() {
        if ordinal != 0 {
            output
                .write_char(',')
                .map_err(|_| output.error("affine ordering manifest bytes"))?;
        }
        write!(output, "{position}").map_err(|_| output.error("affine ordering manifest bytes"))?;
    }
    Ok(())
}

struct BoundedManifestBuilder {
    value: String,
    limit: usize,
    failure: Option<ManifestFailure>,
}

#[derive(Clone, Copy)]
enum ManifestFailure {
    Overflow,
    Limit(usize),
    Allocation(usize),
}

impl BoundedManifestBuilder {
    fn new(limit: usize) -> Self {
        Self {
            value: String::new(),
            limit,
            failure: None,
        }
    }
    fn error(&self, resource: &'static str) -> AffineParametricOrderingError {
        match self.failure {
            Some(ManifestFailure::Overflow) => {
                AffineParametricOrderingError::ResourceCountOverflow { resource }
            }
            Some(ManifestFailure::Limit(requested)) => {
                AffineParametricOrderingError::ResourceLimit {
                    resource,
                    requested,
                    limit: self.limit,
                }
            }
            Some(ManifestFailure::Allocation(requested)) => {
                AffineParametricOrderingError::AllocationFailure {
                    resource,
                    requested,
                }
            }
            None => AffineParametricOrderingError::AllocationFailure {
                resource,
                requested: self.value.len(),
            },
        }
    }
    fn preflight_additional(
        &mut self,
        additional: usize,
        resource: &'static str,
    ) -> Result<(), AffineParametricOrderingError> {
        let Some(requested) = self.value.len().checked_add(additional) else {
            self.failure = Some(ManifestFailure::Overflow);
            return Err(self.error(resource));
        };
        if requested > self.limit {
            self.failure = Some(ManifestFailure::Limit(requested));
            return Err(self.error(resource));
        }
        Ok(())
    }
    fn finish(self) -> String {
        self.value
    }
}

impl fmt::Write for BoundedManifestBuilder {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let Some(requested) = self.value.len().checked_add(value.len()) else {
            self.failure = Some(ManifestFailure::Overflow);
            return Err(fmt::Error);
        };
        if requested > self.limit {
            self.failure = Some(ManifestFailure::Limit(requested));
            return Err(fmt::Error);
        }
        if self.value.try_reserve_exact(value.len()).is_err() {
            self.failure = Some(ManifestFailure::Allocation(requested));
            return Err(fmt::Error);
        }
        self.value.push_str(value);
        Ok(())
    }
}

fn write_values<T: fmt::Display>(
    output: &mut BoundedManifestBuilder,
    values: &[T],
    resource: &'static str,
) -> Result<(), AffineParametricOrderingError> {
    for (ordinal, value) in values.iter().enumerate() {
        if ordinal != 0 {
            output.write_char(',').map_err(|_| output.error(resource))?;
        }
        write!(output, "{value}").map_err(|_| output.error(resource))?;
    }
    Ok(())
}

pub(crate) fn integer_magnitude_bits(
    value: &Integer,
) -> Result<usize, AffineParametricOrderingError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| AffineParametricOrderingError::ResourceCountOverflow {
        resource: "affine integer bits",
    })
}

#[cfg(test)]
thread_local! {
    static TEST_INDEX_SHIFT_COPY_COMPONENTS: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
}

fn try_copy_index_shift(
    shift: &IndexShift,
    admitted_arity: usize,
) -> Result<IndexShift, AffineParametricOrderingError> {
    if shift.arity() != admitted_arity {
        return Err(AffineParametricOrderingError::WrongShiftArity {
            expected: admitted_arity,
            actual: shift.arity(),
        });
    }
    let requested = admitted_arity;
    #[cfg(test)]
    TEST_INDEX_SHIFT_COPY_COMPONENTS.with(|components| {
        components.set(components.get().saturating_add(requested));
    });
    let mut values = Vec::new();
    values.try_reserve_exact(requested).map_err(|_| {
        AffineParametricOrderingError::AllocationFailure {
            resource: "affine key shift components",
            requested,
        }
    })?;
    values.extend_from_slice(shift.values());
    IndexShift::try_from_preallocated(values, requested)
        .map_err(|_| AffineParametricOrderingError::ReplayMismatch)
}

fn check_integer_bits(
    resource: &'static str,
    value: &Integer,
    limit: usize,
) -> Result<(), AffineParametricOrderingError> {
    check_limit(resource, integer_magnitude_bits(value)?, limit)
}

fn i64_magnitude_bits(value: i64) -> usize {
    (i64::BITS - value.unsigned_abs().leading_zeros()) as usize
}

fn i128_magnitude_bits(value: i128) -> usize {
    (i128::BITS - value.unsigned_abs().leading_zeros()) as usize
}

/// Conservative preflight before an arbitrary-precision integer addition or
/// subtraction.  The exact magnitude of `a +/- b` is below
/// `2^(max(bits(a),bits(b))+1)`; checking that bound happens before cloning or
/// mutating a potentially GMP-backed operand.
fn check_prospective_integer_add(
    resource: &'static str,
    left_bits: usize,
    right_bits: usize,
    limit: usize,
) -> Result<(), AffineParametricOrderingError> {
    let requested =
        prospective_integer_add_bits(left_bits, right_bits).map_err(|error| match error {
            AffineParametricOrderingError::ResourceCountOverflow { .. } => {
                AffineParametricOrderingError::ResourceCountOverflow { resource }
            }
            other => other,
        })?;
    check_limit(resource, requested, limit)
}

fn prospective_integer_add_bits(
    left_bits: usize,
    right_bits: usize,
) -> Result<usize, AffineParametricOrderingError> {
    if left_bits == 0 {
        Ok(right_bits)
    } else if right_bits == 0 {
        Ok(left_bits)
    } else {
        left_bits.max(right_bits).checked_add(1).ok_or(
            AffineParametricOrderingError::ResourceCountOverflow {
                resource: "affine prospective integer addition bits",
            },
        )
    }
}

fn compare_sector_prefix_bits_for_policy(
    policy: IntegralOrderingPolicy,
    source_bits: &[bool],
    target_bits: &[bool],
) -> Result<Ordering, AffineParametricOrderingError> {
    Ok(
        compare_sector_prefix_bits_for_policy_with_census(policy, source_bits, target_bits)?
            .ordering,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AffineSectorPrefixComparisonCensus {
    ordering: Ordering,
    source_propagators: usize,
    target_propagators: usize,
    comparison_units: usize,
}

fn compare_sector_prefix_bits_for_policy_with_census(
    policy: IntegralOrderingPolicy,
    source_bits: &[bool],
    target_bits: &[bool],
) -> Result<AffineSectorPrefixComparisonCensus, AffineParametricOrderingError> {
    if target_bits.len() != source_bits.len() {
        return Err(AffineParametricOrderingError::WrongSectorArity {
            expected: source_bits.len(),
            actual: target_bits.len(),
        });
    }
    let mut source_propagators = 0usize;
    let mut target_propagators = 0usize;
    let mut comparison_units = 0usize;
    for &active in source_bits {
        comparison_units = comparison_units.checked_add(1).ok_or(
            AffineParametricOrderingError::ResourceCountOverflow {
                resource: "affine sector-prefix comparison units",
            },
        )?;
        source_propagators += if active { 1 } else { 0 };
    }
    for &active in target_bits {
        comparison_units = comparison_units.checked_add(1).ok_or(
            AffineParametricOrderingError::ResourceCountOverflow {
                resource: "affine sector-prefix comparison units",
            },
        )?;
        target_propagators += if active { 1 } else { 0 };
    }
    match policy {
        IntegralOrderingPolicy::RustRedUnshiftedV1 => {
            let mut ordering = target_propagators.cmp(&source_propagators);
            if ordering == Ordering::Equal {
                for (&target, &source) in target_bits.iter().zip(source_bits) {
                    comparison_units = comparison_units.checked_add(1).ok_or(
                        AffineParametricOrderingError::ResourceCountOverflow {
                            resource: "affine sector-prefix comparison units",
                        },
                    )?;
                    ordering = target.cmp(&source);
                    if ordering != Ordering::Equal {
                        break;
                    }
                }
            }
            Ok(AffineSectorPrefixComparisonCensus {
                ordering,
                source_propagators,
                target_propagators,
                comparison_units,
            })
        }
    }
}

pub(crate) fn key_component_count(arity: usize) -> Result<usize, AffineParametricOrderingError> {
    checked_add(
        "affine order-key components",
        checked_mul("affine order-key components", arity, KEY_COMPONENT_VECTORS)?,
        KEY_FIXED_COMPONENTS,
    )
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, AffineParametricOrderingError> {
    left.checked_add(right)
        .ok_or(AffineParametricOrderingError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, AffineParametricOrderingError> {
    left.checked_mul(right)
        .ok_or(AffineParametricOrderingError::ResourceCountOverflow { resource })
}

fn bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, AffineParametricOrderingError> {
    let requested = checked_add(resource, left, right)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), AffineParametricOrderingError> {
    if requested > limit {
        Err(AffineParametricOrderingError::ResourceLimit {
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

    fn polynomial(
        context: &ParametricCoefficientContext,
        value: &ParametricCoefficient,
    ) -> ParametricPolynomial {
        context.numerator_condition(value).unwrap()
    }

    struct BorrowedTestGeometry<'geometry> {
        constants: &'geometry [Integer],
    }

    impl<'geometry> AffineParametricOrderingGeometry<'geometry> for BorrowedTestGeometry<'geometry> {
        fn ambient_arity(&self) -> usize {
            self.constants.len()
        }

        fn free_positions(&self) -> &'geometry [usize] {
            &[]
        }

        fn constant(&self, position: usize) -> Option<&'geometry Integer> {
            self.constants.get(position)
        }

        fn linear_coefficient(
            &self,
            _position: usize,
            _free_ordinal: usize,
        ) -> Option<&'geometry Integer> {
            None
        }
    }

    #[test]
    fn borrowed_wrong_arity_is_rejected_before_copy_or_key_work() {
        let constants = [Integer::from(1)];
        let geometry = BorrowedTestGeometry {
            constants: &constants,
        };
        let sector = SectorMask::try_new([true]).unwrap();
        let constant_positions = [0usize];
        let manifest = Arc::new(String::from("wrong-arity-pre-copy-private"));
        let algebra = AffineParametricOrderingAlgebra::new(
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            &sector,
            &geometry,
            &constant_positions,
            AffineParametricOrderingLimits::default(),
            &manifest,
            RUSTRED_AFFINE_START_UNSHIFTED_ORDER_V1_KEY_SCHEMA,
        );
        let hostile_arity = 1_000_001usize;
        let hostile =
            IndexShift::try_new(std::iter::repeat_n(0, hostile_arity), hostile_arity).unwrap();

        TEST_INDEX_SHIFT_COPY_COMPONENTS.with(|components| components.set(0));
        let error = algebra.key_for_shift(&hostile, usize::MAX).unwrap_err();
        assert!(matches!(
            error,
            AffineParametricOrderingError::WrongShiftArity {
                expected: 1,
                actual,
            } if actual == hostile_arity
        ));
        TEST_INDEX_SHIFT_COPY_COMPONENTS.with(|components| {
            assert_eq!(components.get(), 0, "wrong-arity path copied shift data")
        });

        let valid = IndexShift::try_new([0], 1).unwrap();
        algebra.key_for_shift(&valid, usize::MAX).unwrap();
        TEST_INDEX_SHIFT_COPY_COMPONENTS.with(|components| assert_eq!(components.get(), 1));
    }

    /// Narrow identity fixture: the fixed prefix stands in for already
    /// authenticated family/context/cover metadata, while every mutable
    /// component is rendered by the same writers as a production branch.
    fn synthetic_branch_identity(
        zero_atoms: &[(usize, ParametricPolynomial)],
        nonzero_atoms: &[(usize, ParametricPolynomial)],
        free_positions: &[usize],
        constants: &[Integer],
        free_column_matrix: &[Vec<Integer>],
        limit: usize,
    ) -> Result<String, AffineParametricOrderingError> {
        let mut output = BoundedManifestBuilder::new(limit);
        output
            .write_str("affine-source-v1|integer-encoding=sign-magnitude-hex-v1|kind=residual-boolean-branch|authenticated-test-prefix")
            .map_err(|_| output.error("affine map identity bytes"))?;
        write_branch_atom_entries(
            &mut output,
            "zero",
            zero_atoms.len(),
            zero_atoms
                .iter()
                .map(|(ordinal, value)| Ok::<_, AffineParametricOrderingError>((*ordinal, value))),
        )?;
        write_branch_atom_entries(
            &mut output,
            "nonzero",
            nonzero_atoms.len(),
            nonzero_atoms
                .iter()
                .map(|(ordinal, value)| Ok::<_, AffineParametricOrderingError>((*ordinal, value))),
        )?;
        write_affine_geometry_identity(
            &mut output,
            constants.len(),
            free_positions,
            |position| constants.get(position),
            |position, free_ordinal| {
                free_column_matrix
                    .get(position)
                    .and_then(|row| row.get(free_ordinal))
            },
        )?;
        Ok(output.finish())
    }

    #[test]
    fn ordering_stats_expose_every_retained_measure() {
        let stats = AffineParametricOrderingStats {
            ambient_arity: 1,
            free_positions: 2,
            constant_positions: 3,
            symbolic_positions: 4,
            matrix_entries_inspected: 5,
            largest_affine_integer_bits: 6,
            map_identity_bytes: 7,
            manifest_bytes: 8,
        };

        assert_eq!(stats.ambient_arity(), 1);
        assert_eq!(stats.free_positions(), 2);
        assert_eq!(stats.constant_positions(), 3);
        assert_eq!(stats.symbolic_positions(), 4);
        assert_eq!(stats.matrix_entries_inspected(), 5);
        assert_eq!(stats.largest_affine_integer_bits(), 6);
        assert_eq!(stats.map_identity_bytes(), 7);
        assert_eq!(stats.manifest_bytes(), 8);
    }

    #[test]
    fn branch_identity_commits_to_atoms_guards_and_affine_geometry() {
        let base = CoefficientContext::new(["d"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "affine-ordering-identity-components", 2)
                .unwrap();
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let atom = polynomial(&context, &context.sub(&n0, &context.integer(1)).unwrap());
        let changed_atom = polynomial(&context, &context.sub(&n0, &context.integer(2)).unwrap());
        let guard = polynomial(&context, &context.add(&n1, &context.integer(1)).unwrap());
        let changed_guard = polynomial(&context, &context.add(&n1, &context.integer(2)).unwrap());
        let zero_atoms = vec![(3, atom.clone())];
        let guards = vec![(5, guard.clone())];
        let changed_atom_entries = vec![(3, changed_atom)];
        let changed_guard_set = vec![(7, guard.clone())];
        let changed_guard_polynomial = vec![(5, changed_guard)];
        let free_positions = [1usize];
        let constants = vec![Integer::from(1), Integer::from(0)];
        let changed_constants = vec![Integer::from(2), Integer::from(0)];
        let free_column_matrix = vec![vec![Integer::from(0)], vec![Integer::from(1)]];
        let changed_free_column_matrix = vec![vec![Integer::from(1)], vec![Integer::from(1)]];
        let identity_limit = 1024 * 1024;

        let baseline = synthetic_branch_identity(
            &zero_atoms,
            &guards,
            &free_positions,
            &constants,
            &free_column_matrix,
            identity_limit,
        )
        .unwrap();
        let atom_polynomial_identity = synthetic_branch_identity(
            &changed_atom_entries,
            &guards,
            &free_positions,
            &constants,
            &free_column_matrix,
            identity_limit,
        )
        .unwrap();
        let guard_set_identity = synthetic_branch_identity(
            &zero_atoms,
            &changed_guard_set,
            &free_positions,
            &constants,
            &free_column_matrix,
            identity_limit,
        )
        .unwrap();
        let guard_polynomial_identity = synthetic_branch_identity(
            &zero_atoms,
            &changed_guard_polynomial,
            &free_positions,
            &constants,
            &free_column_matrix,
            identity_limit,
        )
        .unwrap();
        let constant_identity = synthetic_branch_identity(
            &zero_atoms,
            &guards,
            &free_positions,
            &changed_constants,
            &free_column_matrix,
            identity_limit,
        )
        .unwrap();
        let free_column_identity = synthetic_branch_identity(
            &zero_atoms,
            &guards,
            &free_positions,
            &constants,
            &changed_free_column_matrix,
            identity_limit,
        )
        .unwrap();

        assert_ne!(baseline, atom_polynomial_identity);
        assert_ne!(baseline, guard_set_identity);
        assert_ne!(baseline, guard_polynomial_identity);
        assert_ne!(baseline, constant_identity);
        assert_ne!(baseline, free_column_identity);
    }

    #[test]
    fn large_gmp_hex_identity_has_exact_at_limit_and_one_below_boundaries() {
        for (value, expected) in [
            (Integer::from(i64::MIN), "-8000000000000000"),
            (
                Integer::from(i128::MIN),
                "-80000000000000000000000000000000",
            ),
        ] {
            let mut primitive = BoundedManifestBuilder::new(expected.len());
            write_identity_integer(&mut primitive, &value).unwrap();
            assert_eq!(primitive.finish(), expected);
        }

        let huge = Integer::from(1) << 100_000u32;
        assert!(matches!(&huge, Integer::Large(_)));
        let exact_bytes = 25_001usize;
        let mut exact = BoundedManifestBuilder::new(exact_bytes);

        write_identity_integer(&mut exact, &huge).unwrap();

        let rendered = exact.finish();
        assert_eq!(rendered.len(), exact_bytes);
        assert!(rendered.starts_with('1'));
        assert!(rendered[1..].bytes().all(|byte| byte == b'0'));

        let mut one_below = BoundedManifestBuilder::new(exact_bytes - 1);
        let error = write_identity_integer(&mut one_below, &huge).unwrap_err();

        assert!(matches!(
            error,
            AffineParametricOrderingError::ResourceLimit {
                resource: "affine map identity bytes",
                requested,
                limit,
            } if requested == exact_bytes && limit == exact_bytes - 1
        ));
        assert!(one_below.value.is_empty());
    }

    #[test]
    fn large_gmp_decimal_diagnostic_is_preflighted_before_formatting() {
        let huge = Integer::from(1) << 100_000u32;
        let mut output = BoundedManifestBuilder::new(32);

        let error =
            write_decimal_integer(&mut output, &huge, "affine key diagnostic bytes").unwrap_err();

        assert!(matches!(
            error,
            AffineParametricOrderingError::ResourceLimit {
                resource: "affine key diagnostic bytes",
                requested,
                limit: 32,
            } if requested > 32
        ));
        assert!(output.value.is_empty());
    }

    #[test]
    fn every_key_diagnostic_integer_field_uses_the_gmp_preflight() {
        let huge = Arc::new(Integer::from(1) << 100_000u32);
        let zero = Arc::new(Integer::from(0));
        for huge_field in 0..4 {
            let key = AffineStartIntegralComplexityKey {
                schema: AFFINE_START_INTEGRAL_COMPLEXITY_KEY_V1_SCHEMA,
                key_schema: RUSTRED_AFFINE_START_UNSHIFTED_ORDER_V1_KEY_SCHEMA,
                policy: IntegralOrderingPolicy::RustRedUnshiftedV1,
                arity: 1,
                propagators: 1,
                formal_sector: Arc::new(SectorMask::try_new([true]).unwrap()),
                corner_distance_offset: if huge_field == 0 {
                    huge.clone()
                } else {
                    zero.clone()
                },
                dots_offset: if huge_field == 1 {
                    huge.clone()
                } else {
                    zero.clone()
                },
                numerators_offset: if huge_field == 2 {
                    huge.clone()
                } else {
                    zero.clone()
                },
                signed_index_excess: Arc::new(vec![if huge_field == 3 {
                    huge.as_ref().clone()
                } else {
                    Integer::from(0)
                }]),
                retained_integer_bits: 100_001,
                shift: Arc::new(IndexShift::try_new([0], 1).unwrap()),
                ordering_manifest: Arc::new(String::new()),
                diagnostic_limit_bytes: 1024,
            };
            let cloned = key.clone();
            assert!(Arc::ptr_eq(&key.formal_sector, &cloned.formal_sector));
            assert!(Arc::ptr_eq(
                &key.signed_index_excess,
                &cloned.signed_index_excess
            ));
            assert!(Arc::ptr_eq(&key.shift, &cloned.shift));
            assert!(matches!(
                key.try_to_stable_string(),
                Err(AffineParametricOrderingError::ResourceLimit {
                    resource: "affine key diagnostic bytes",
                    requested,
                    limit: 1024,
                }) if requested > 1024
            ));
        }
    }

    #[test]
    fn key_ord_distinguishes_every_field_used_by_derived_equality() {
        let baseline = AffineStartIntegralComplexityKey {
            schema: AFFINE_START_INTEGRAL_COMPLEXITY_KEY_V1_SCHEMA,
            key_schema: RUSTRED_AFFINE_START_UNSHIFTED_ORDER_V1_KEY_SCHEMA,
            policy: IntegralOrderingPolicy::RustRedUnshiftedV1,
            arity: 1,
            propagators: 1,
            formal_sector: Arc::new(SectorMask::try_new([true]).unwrap()),
            corner_distance_offset: Arc::new(Integer::from(0)),
            dots_offset: Arc::new(Integer::from(0)),
            numerators_offset: Arc::new(Integer::from(0)),
            signed_index_excess: Arc::new(vec![Integer::from(0)]),
            retained_integer_bits: 0,
            shift: Arc::new(IndexShift::try_new([0], 1).unwrap()),
            ordering_manifest: Arc::new(String::new()),
            diagnostic_limit_bytes: 16,
        };

        let mut changed_retained_bits = baseline.clone();
        changed_retained_bits.retained_integer_bits = 1;
        assert_ne!(baseline, changed_retained_bits);
        assert_ne!(baseline.cmp(&changed_retained_bits), Ordering::Equal);

        let mut changed_diagnostic_limit = baseline.clone();
        changed_diagnostic_limit.diagnostic_limit_bytes = 17;
        assert_ne!(baseline, changed_diagnostic_limit);
        assert_ne!(baseline.cmp(&changed_diagnostic_limit), Ordering::Equal);
    }

    #[test]
    fn v1_sector_prefix_counts_simultaneous_pinch_and_activation_exactly() {
        let source = [true, true, false, false];
        // The first constant row pinches while the third activates.  Equal
        // propagator counts therefore reach the exact persisted bit prefix.
        let target = [false, true, true, false];
        assert_eq!(
            compare_sector_prefix_bits_for_policy(
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                &source,
                &target,
            )
            .unwrap(),
            Ordering::Less
        );

        let wrong_arity = compare_sector_prefix_bits_for_policy(
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            &source,
            &target[..3],
        )
        .unwrap_err();
        assert!(matches!(
            wrong_arity,
            AffineParametricOrderingError::WrongSectorArity {
                expected: 4,
                actual: 3,
            }
        ));
    }

    #[test]
    fn v1_sector_prefix_propagator_count_precedes_exact_bit_word() {
        // Lexicographically the target bit word is greater, but losing one
        // propagator is the earlier and therefore decisive V1 component.
        let source = [false, true, true];
        let fewer_propagators = [true, false, false];
        let census = compare_sector_prefix_bits_for_policy_with_census(
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            &source,
            &fewer_propagators,
        )
        .unwrap();
        assert_eq!(census.ordering, Ordering::Less);
        assert_eq!(census.source_propagators, 2);
        assert_eq!(census.target_propagators, 1);
        assert_eq!(census.comparison_units, 6);

        // Equal propagator counts reach the exact index-major bit word.
        let source = [true, false, false];
        let equal_count_lower_bits = [false, true, false];
        let census = compare_sector_prefix_bits_for_policy_with_census(
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            &source,
            &equal_count_lower_bits,
        )
        .unwrap();
        assert_eq!(census.ordering, Ordering::Less);
        assert_eq!(census.source_propagators, 1);
        assert_eq!(census.target_propagators, 1);
        assert_eq!(census.comparison_units, 7);
    }

    #[test]
    fn constant_shift_threshold_matches_construction_at_integer_extremes() {
        let huge = Integer::from(1) << 256u32;
        assert!(matches!(&huge, Integer::Large(_)));
        let huge_negative = -huge.clone();

        for displacement in [i64::MIN, i64::MAX, -1, 0, 1] {
            let threshold = 1i128 - i128::from(displacement);
            let around_threshold = [
                Integer::Double(threshold - 1),
                Integer::Double(threshold),
                Integer::Double(threshold + 1),
                Integer::Double(i128::MIN),
                Integer::Double(i128::MAX),
                huge.clone(),
                huge_negative.clone(),
            ];
            for constant in around_threshold {
                let mut shifted = constant.clone();
                shifted += Integer::from(displacement);
                let constructed_active = shifted >= Integer::from(1);
                let replayed_active = constant >= Integer::Double(threshold);
                assert_eq!(
                    constructed_active, replayed_active,
                    "constant={constant}, displacement={displacement}"
                );
            }

            // The replay threshold remains an inline i128 even for the two
            // extreme i64 displacements, and its magnitude census is exact.
            assert!(i128_magnitude_bits(threshold) <= 64);
        }
    }
}
