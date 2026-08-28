//! Bounded, topology-independent discovery of internal vacuum symmetries.
//!
//! This is the first finite candidate backend for LiteRed-style `FindShifts` /
//! `FindSymmetries` semantics.  It streams every integer loop map
//!
//! ```text
//! l_source = A l_target,  A_ij in [-radius, radius], det(A) = +/-1,
//! ```
//!
//! for a vacuum family.  Candidate generation is deliberately separate from
//! proof: every unimodular matrix is passed through
//! [`crate::verify_affine_family_map`].  A verified affine family map is then
//! compiled into an integral symmetry only when all denominator rows form a
//! bijective, unit-scale permutation and that permutation preserves formal
//! power shifts, cuts, and sector-pattern slots.
//!
//! Exhausting a work limit is retained as [`InternalSymmetrySearchCompletion::ResourceLimited`].
//! It is never interpreted as proof that no further symmetry exists.

use std::fmt::{self, Write as _};

use symbolica::domains::integer::Integer;

use crate::exact_identity::{ExactIdentityError, ExactIdentityWriter};
use crate::{
    ConcreteIntegralKey, DenominatorRowAction, ExactMatrix, IntegralFamily, JacobianWitness,
    MomentumMap, SectorRestrictions, SymmetryVerificationError, SymmetryVerificationLimits,
    VerifiedAffineFamilyMap, algebra::CoefficientContext, algebra::ExactAlgebraError,
    verify_affine_family_map,
};

pub const BOUNDED_INTEGER_VACUUM_SYMMETRY_SEARCH_V1_SCHEMA: &str =
    "rustred-bounded-integer-vacuum-symmetry-search-v1";
pub const INTERNAL_FAMILY_PERMUTATION_SYMMETRY_V1_SCHEMA: &str =
    "rustred-internal-family-permutation-symmetry-v1";
pub(crate) const INTERNAL_FAMILY_PERMUTATION_SYMMETRY_STABLE_VALUE_IDENTITY_V1_SCHEMA: &str =
    "rustred-internal-family-permutation-symmetry-stable-value-identity-v1";

/// Finite coefficient alphabet and aggregate work bounds for one search.
///
/// The coefficient radius defines the mathematical search domain.  The other
/// fields are execution limits.  Reaching an execution limit produces a
/// partial report, not a negative symmetry result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InternalSymmetrySearchLimits {
    /// Enumerate each integer entry of `A` in `[-coefficient_radius,
    /// coefficient_radius]`.
    pub coefficient_radius: u32,
    /// Maximum retained entries in one streamed loop map.
    pub max_loop_map_entries: usize,
    /// Maximum row-major integer matrices inspected, including singular ones.
    pub max_enumerated_matrices: usize,
    /// Aggregate checked integer operations used by the Bareiss prefilter.
    pub max_integer_determinant_operations: usize,
    /// Conservative maximum bit width of any exact Bareiss minor.
    pub max_integer_bits: usize,
    /// Maximum exact affine-verifier calls (one per unimodular candidate).
    pub max_verifier_calls: usize,
    /// Maximum distinct, compiled denominator-permutation certificates.
    pub max_retained_symmetries: usize,
    /// Aggregate logical entries retained by distinct certificates.  Entries
    /// include exact map coefficients, guards/origins, row actions,
    /// restrictions, and denominator-permutation slots.
    pub max_retained_certificate_entries: usize,
    /// Aggregate bytes in the bounded structural `Debug` encoding of distinct
    /// retained certificates.  The encoding is streamed into a counter and is
    /// never materialized as one large string.
    pub max_retained_certificate_bytes: usize,
    /// Per-candidate authoritative affine-map replay bounds.
    pub verification: SymmetryVerificationLimits,
}

impl Default for InternalSymmetrySearchLimits {
    fn default() -> Self {
        Self {
            coefficient_radius: 1,
            max_loop_map_entries: 1_000_000,
            max_enumerated_matrices: 10_000_000,
            max_integer_determinant_operations: 100_000_000,
            max_integer_bits: 1_000_000,
            max_verifier_calls: 1_000_000,
            max_retained_symmetries: 1_000_000,
            max_retained_certificate_entries: 100_000_000,
            max_retained_certificate_bytes: 2 * 1024 * 1024 * 1024,
            verification: SymmetryVerificationLimits::default(),
        }
    }
}

/// Exact meaning of the returned candidate set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InternalSymmetrySearchCompletion {
    /// Every matrix in the fingerprinted finite coefficient alphabet was
    /// inspected.
    ExhaustiveWithinBounds { domain_fingerprint: String },
    /// The report contains only the certified prefix produced before this
    /// bound was reached.
    ResourceLimited {
        domain_fingerprint: String,
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
}

impl InternalSymmetrySearchCompletion {
    pub fn domain_fingerprint(&self) -> &str {
        match self {
            Self::ExhaustiveWithinBounds { domain_fingerprint }
            | Self::ResourceLimited {
                domain_fingerprint, ..
            } => domain_fingerprint,
        }
    }

    pub const fn is_exhaustive_within_bounds(&self) -> bool {
        matches!(self, Self::ExhaustiveWithinBounds { .. })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InternalSymmetrySearchStats {
    enumerated_matrices: usize,
    integer_determinant_operations: usize,
    unimodular_candidates: usize,
    verifier_calls: usize,
    affine_candidates_rejected: usize,
    incompatible_integral_maps: usize,
    duplicate_row_actions: usize,
    retained_symmetries: usize,
    retained_certificate_entries: usize,
    retained_certificate_bytes: usize,
}

impl InternalSymmetrySearchStats {
    pub const fn enumerated_matrices(self) -> usize {
        self.enumerated_matrices
    }

    pub const fn integer_determinant_operations(self) -> usize {
        self.integer_determinant_operations
    }

    pub const fn unimodular_candidates(self) -> usize {
        self.unimodular_candidates
    }

    pub const fn verifier_calls(self) -> usize {
        self.verifier_calls
    }

    pub const fn affine_candidates_rejected(self) -> usize {
        self.affine_candidates_rejected
    }

    pub const fn incompatible_integral_maps(self) -> usize {
        self.incompatible_integral_maps
    }

    pub const fn duplicate_row_actions(self) -> usize {
        self.duplicate_row_actions
    }

    pub const fn retained_symmetries(self) -> usize {
        self.retained_symmetries
    }

    pub const fn retained_certificate_entries(self) -> usize {
        self.retained_certificate_entries
    }

    pub const fn retained_certificate_bytes(self) -> usize {
        self.retained_certificate_bytes
    }
}

/// An affine proof compiled into a symmetry of arbitrary integer index
/// vectors for one authenticated family/restriction pair.
///
/// The affine certificate alone is intentionally not exposed as an integral
/// symmetry: this wrapper additionally proves all rule-compilation
/// preconditions which [`crate::verify_affine_family_map`] does not check.
#[derive(Clone, Debug)]
pub struct VerifiedInternalFamilyPermutationSymmetry {
    family_fingerprint: String,
    restrictions_fingerprint: String,
    restrictions: SectorRestrictions,
    denominator_permutation: Vec<usize>,
    affine_map: VerifiedAffineFamilyMap,
}

impl VerifiedInternalFamilyPermutationSymmetry {
    pub const SCHEMA: &'static str = INTERNAL_FAMILY_PERMUTATION_SYMMETRY_V1_SCHEMA;

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn restrictions_fingerprint(&self) -> &str {
        &self.restrictions_fingerprint
    }

    /// Source denominator `i` maps to target denominator
    /// `denominator_permutation[i]`.
    pub fn denominator_permutation(&self) -> &[usize] {
        &self.denominator_permutation
    }

    pub const fn restrictions(&self) -> &SectorRestrictions {
        &self.restrictions
    }

    /// The owned, authoritative momentum/denominator proof from which this
    /// stricter integral-symmetry certificate was compiled.
    pub const fn affine_map(&self) -> &VerifiedAffineFamilyMap {
        &self.affine_map
    }

    pub(crate) fn stable_value_eq(&self, other: &Self) -> bool {
        self.family_fingerprint == other.family_fingerprint
            && self.restrictions_fingerprint == other.restrictions_fingerprint
            && self.restrictions == other.restrictions
            && self.denominator_permutation == other.denominator_permutation
            && self.affine_map.stable_value_eq(&other.affine_map)
    }

    /// Transport one exact source integral into target denominator order.
    /// If `D_source[i] = D_target[pi(i)]`, then
    /// `target_power[pi(i)] = source_power[i]`.
    pub fn transport_source_key(
        &self,
        source: &ConcreteIntegralKey,
    ) -> Result<ConcreteIntegralKey, InternalSymmetryKeyTransportError> {
        if source.powers().len() != self.denominator_permutation.len() {
            return Err(InternalSymmetryKeyTransportError::WrongArity {
                expected: self.denominator_permutation.len(),
                actual: source.powers().len(),
            });
        }
        let mut target = Vec::new();
        target
            .try_reserve_exact(self.denominator_permutation.len())
            .map_err(|_| InternalSymmetryKeyTransportError::AllocationFailure {
                requested: self.denominator_permutation.len(),
            })?;
        target.resize(self.denominator_permutation.len(), 0i64);
        for (source_denominator, &target_denominator) in
            self.denominator_permutation.iter().enumerate()
        {
            target[target_denominator] = source.powers()[source_denominator];
        }
        ConcreteIntegralKey::try_new(target)
            .map_err(|_| InternalSymmetryKeyTransportError::InvalidCertificateArity)
    }

    /// Replay an asserted source-to-target key image exactly.
    pub fn replay_key_transport(
        &self,
        source: &ConcreteIntegralKey,
        target: &ConcreteIntegralKey,
    ) -> Result<(), InternalSymmetryKeyTransportError> {
        let replayed = self.transport_source_key(source)?;
        if &replayed != target {
            return Err(InternalSymmetryKeyTransportError::TargetMismatch);
        }
        Ok(())
    }

    /// Independently replay the affine proof and all integral-symmetry
    /// compilation checks against current family metadata.
    pub fn replay(
        &self,
        family: &IntegralFamily,
        restrictions: &SectorRestrictions,
        limits: SymmetryVerificationLimits,
    ) -> Result<(), InternalSymmetryReplayError> {
        if family.fingerprint() != self.family_fingerprint {
            return Err(InternalSymmetryReplayError::FamilyFingerprintMismatch);
        }
        if restrictions != &self.restrictions {
            return Err(InternalSymmetryReplayError::RestrictionsMismatch);
        }
        if restriction_fingerprint(restrictions) != self.restrictions_fingerprint {
            return Err(InternalSymmetryReplayError::RestrictionsMismatch);
        }
        self.affine_map.replay(family, family, limits)?;
        let replayed = compile_internal_family_permutation_symmetry(
            family,
            restrictions,
            self.affine_map.clone(),
        )?;
        if replayed.family_fingerprint != self.family_fingerprint
            || replayed.restrictions != self.restrictions
            || replayed.restrictions_fingerprint != self.restrictions_fingerprint
            || replayed.denominator_permutation != self.denominator_permutation
        {
            return Err(InternalSymmetryReplayError::CertificateReplayMismatch);
        }
        Ok(())
    }

    pub(crate) fn write_stable_value_identity(
        &self,
        writer: &mut ExactIdentityWriter<'_>,
        tag: &str,
    ) -> Result<(), ExactIdentityError> {
        writer.begin_record(tag, 7)?;
        writer.string(
            "identity_schema",
            INTERNAL_FAMILY_PERMUTATION_SYMMETRY_STABLE_VALUE_IDENTITY_V1_SCHEMA,
        )?;
        writer.string("certificate_schema", Self::SCHEMA)?;
        writer.string("family_fingerprint", &self.family_fingerprint)?;
        writer.string("restrictions_fingerprint", &self.restrictions_fingerprint)?;
        write_sector_restrictions_identity(writer, "restrictions", &self.restrictions)?;
        writer.begin_sequence(
            "denominator_permutation",
            self.denominator_permutation.len(),
        )?;
        for &target in &self.denominator_permutation {
            writer.usize("target", target)?;
        }
        writer.end_sequence()?;
        self.affine_map
            .write_stable_value_identity(writer, "affine_map")?;
        writer.end_record()
    }

    /// Prove that this already-verified family map preserves another exact
    /// cut/pattern policy. The affine proof remains owned by `self`; callers
    /// may later compile a restricted certificate lazily only when the map is
    /// selected for a concrete rewrite.
    pub fn validate_restriction_compatibility(
        &self,
        family: &IntegralFamily,
        restrictions: &SectorRestrictions,
    ) -> Result<(), InternalSymmetryCompatibilityError> {
        if self.family_fingerprint() != family.fingerprint() {
            return Err(InternalSymmetryCompatibilityError::FamilyFingerprintMismatch);
        }
        if restrictions.arity() != family.denominator_count() {
            return Err(InternalSymmetryCompatibilityError::WrongRestrictionArity {
                expected: family.denominator_count(),
                actual: restrictions.arity(),
            });
        }
        for (source, &target) in self.denominator_permutation.iter().enumerate() {
            if family.power_shifts()[source] != family.power_shifts()[target] {
                return Err(InternalSymmetryCompatibilityError::PowerShiftMismatch {
                    source_denominator: source,
                    target_denominator: target,
                });
            }
            let source_cut = restrictions.cuts().required_active().active_bits()[source];
            let target_cut = restrictions.cuts().required_active().active_bits()[target];
            if source_cut != target_cut {
                return Err(InternalSymmetryCompatibilityError::CutTransportMismatch {
                    source_denominator: source,
                    target_denominator: target,
                });
            }
            if restrictions.pattern().slots()[source] != restrictions.pattern().slots()[target] {
                return Err(
                    InternalSymmetryCompatibilityError::SectorPatternTransportMismatch {
                        source_denominator: source,
                        target_denominator: target,
                    },
                );
            }
        }
        Ok(())
    }
}

fn write_sector_restrictions_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    restrictions: &SectorRestrictions,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 2)?;
    let required = restrictions.cuts().required_active();
    writer.begin_sequence("required_active", required.arity())?;
    for &active in required.active_bits() {
        writer.boolean("active", active)?;
    }
    writer.end_sequence()?;
    writer.begin_sequence("pattern", restrictions.pattern().arity())?;
    for slot in restrictions.pattern().slots() {
        writer.variant(
            "slot",
            match slot {
                crate::SectorPatternSlot::Any => "Any",
                crate::SectorPatternSlot::Active => "Active",
                crate::SectorPatternSlot::Inactive => "Inactive",
            },
        )?;
    }
    writer.end_sequence()?;
    writer.end_record()
}

/// Search output.  Each retained item owns both the verified affine map and
/// its stricter integral-symmetry certificate.
#[derive(Clone, Debug)]
pub struct InternalSymmetrySearchReport {
    symmetries: Vec<VerifiedInternalFamilyPermutationSymmetry>,
    completion: InternalSymmetrySearchCompletion,
    stats: InternalSymmetrySearchStats,
}

impl InternalSymmetrySearchReport {
    pub fn symmetries(&self) -> &[VerifiedInternalFamilyPermutationSymmetry] {
        &self.symmetries
    }

    pub const fn completion(&self) -> &InternalSymmetrySearchCompletion {
        &self.completion
    }

    pub const fn stats(&self) -> InternalSymmetrySearchStats {
        self.stats
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InternalSymmetryCompatibilityError {
    FamilyFingerprintMismatch,
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    WrongRestrictionArity {
        expected: usize,
        actual: usize,
    },
    NonMonomialDenominator {
        source_denominator: usize,
    },
    NonUnitDenominatorScale {
        source_denominator: usize,
        target_denominator: usize,
    },
    NonBijectiveDenominatorAction {
        target_denominator: usize,
    },
    UnsupportedJacobian,
    PowerShiftMismatch {
        source_denominator: usize,
        target_denominator: usize,
    },
    CutTransportMismatch {
        source_denominator: usize,
        target_denominator: usize,
    },
    SectorPatternTransportMismatch {
        source_denominator: usize,
        target_denominator: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InternalSymmetryKeyTransportError {
    WrongArity { expected: usize, actual: usize },
    InvalidCertificateArity,
    AllocationFailure { requested: usize },
    TargetMismatch,
}

impl fmt::Display for InternalSymmetryKeyTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "integral symmetry expects {expected} powers, found {actual}"
            ),
            Self::InvalidCertificateArity => {
                formatter.write_str("integral-symmetry certificate has empty arity")
            }
            Self::AllocationFailure { requested } => write!(
                formatter,
                "could not reserve {requested} integral-key transport entries"
            ),
            Self::TargetMismatch => {
                formatter.write_str("asserted target key does not match symmetry transport")
            }
        }
    }
}

impl std::error::Error for InternalSymmetryKeyTransportError {}

impl fmt::Display for InternalSymmetryCompatibilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FamilyFingerprintMismatch => {
                formatter.write_str("affine map was verified for a different family")
            }
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "integral-symmetry {resource} count overflowed usize"
                )
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} {resource} while compiling an integral symmetry"
            ),
            Self::WrongRestrictionArity { expected, actual } => write!(
                formatter,
                "symmetry restrictions have arity {actual}; family expects {expected}"
            ),
            Self::NonMonomialDenominator { source_denominator } => write!(
                formatter,
                "source denominator {source_denominator} does not have a monomial image"
            ),
            Self::NonUnitDenominatorScale {
                source_denominator,
                target_denominator,
            } => write!(
                formatter,
                "source denominator {source_denominator} maps to {target_denominator} with a non-unit scale"
            ),
            Self::NonBijectiveDenominatorAction { target_denominator } => write!(
                formatter,
                "target denominator {target_denominator} is not hit exactly once"
            ),
            Self::UnsupportedJacobian => {
                formatter.write_str("integral symmetry requires a unit loop Jacobian")
            }
            Self::PowerShiftMismatch {
                source_denominator,
                target_denominator,
            } => write!(
                formatter,
                "power shift on source denominator {source_denominator} differs from target {target_denominator}"
            ),
            Self::CutTransportMismatch {
                source_denominator,
                target_denominator,
            } => write!(
                formatter,
                "cut membership on source denominator {source_denominator} differs from target {target_denominator}"
            ),
            Self::SectorPatternTransportMismatch {
                source_denominator,
                target_denominator,
            } => write!(
                formatter,
                "sector-pattern slot on source denominator {source_denominator} differs from target {target_denominator}"
            ),
        }
    }
}

impl std::error::Error for InternalSymmetryCompatibilityError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InternalSymmetrySearchError {
    NonVacuumFamily { external_momenta: usize },
    EmptyLoopSpace,
    WrongRestrictionArity { expected: usize, actual: usize },
    ResourceCountOverflow { resource: &'static str },
    IntegerDeterminantExactDivisionFailure,
    MatrixConstruction(SymmetryVerificationError),
    UnexpectedVerificationFailure(SymmetryVerificationError),
}

impl fmt::Display for InternalSymmetrySearchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonVacuumFamily { external_momenta } => write!(
                formatter,
                "bounded v1 internal search supports vacuum families, but found {external_momenta} external momenta"
            ),
            Self::EmptyLoopSpace => {
                formatter.write_str("internal symmetry search needs at least one loop momentum")
            }
            Self::WrongRestrictionArity { expected, actual } => write!(
                formatter,
                "symmetry restrictions have arity {actual}; family expects {expected}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "symmetry-search {resource} count overflowed usize"
                )
            }
            Self::IntegerDeterminantExactDivisionFailure => formatter.write_str(
                "fraction-free integer determinant encountered a non-exact internal division",
            ),
            Self::MatrixConstruction(error) => error.fmt(formatter),
            Self::UnexpectedVerificationFailure(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for InternalSymmetrySearchError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InternalSymmetryReplayError {
    FamilyFingerprintMismatch,
    RestrictionsMismatch,
    AffineVerification(SymmetryVerificationError),
    Compatibility(InternalSymmetryCompatibilityError),
    CertificateReplayMismatch,
}

impl fmt::Display for InternalSymmetryReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FamilyFingerprintMismatch => {
                formatter.write_str("integral-symmetry family fingerprint mismatch")
            }
            Self::RestrictionsMismatch => {
                formatter.write_str("integral-symmetry restrictions mismatch")
            }
            Self::AffineVerification(error) => error.fmt(formatter),
            Self::Compatibility(error) => error.fmt(formatter),
            Self::CertificateReplayMismatch => {
                formatter.write_str("integral-symmetry certificate replay mismatch")
            }
        }
    }
}

impl std::error::Error for InternalSymmetryReplayError {}

impl From<SymmetryVerificationError> for InternalSymmetryReplayError {
    fn from(value: SymmetryVerificationError) -> Self {
        Self::AffineVerification(value)
    }
}

impl From<InternalSymmetryCompatibilityError> for InternalSymmetryReplayError {
    fn from(value: InternalSymmetryCompatibilityError) -> Self {
        Self::Compatibility(value)
    }
}

/// Compile an already verified affine self-map into an arbitrary-index
/// family permutation.  This function is useful for explicit candidate
/// backends as well as the bounded enumerator.
pub fn compile_internal_family_permutation_symmetry(
    family: &IntegralFamily,
    restrictions: &SectorRestrictions,
    affine_map: VerifiedAffineFamilyMap,
) -> Result<VerifiedInternalFamilyPermutationSymmetry, InternalSymmetryCompatibilityError> {
    let family_fingerprint = family.fingerprint();
    if affine_map.source_family_fingerprint() != family_fingerprint
        || affine_map.target_family_fingerprint() != family_fingerprint
    {
        return Err(InternalSymmetryCompatibilityError::FamilyFingerprintMismatch);
    }
    if restrictions.arity() != family.denominator_count() {
        return Err(InternalSymmetryCompatibilityError::WrongRestrictionArity {
            expected: family.denominator_count(),
            actual: restrictions.arity(),
        });
    }
    if !matches!(affine_map.jacobian(), JacobianWitness::Unit { .. }) {
        return Err(InternalSymmetryCompatibilityError::UnsupportedJacobian);
    }

    let context = family.coefficient_context();
    let one = context.one();
    let denominator_count = family.denominator_count();
    let mut permutation = Vec::new();
    permutation
        .try_reserve_exact(denominator_count)
        .map_err(|_| InternalSymmetryCompatibilityError::AllocationFailure {
            resource: "denominator-permutation entries",
            requested: denominator_count,
        })?;
    let mut target_hits = Vec::new();
    target_hits
        .try_reserve_exact(denominator_count)
        .map_err(|_| InternalSymmetryCompatibilityError::AllocationFailure {
            resource: "denominator-permutation hit counters",
            requested: denominator_count,
        })?;
    target_hits.resize(denominator_count, 0usize);
    for (source, action) in affine_map.row_actions().iter().enumerate() {
        let DenominatorRowAction::Monomial { target, scale } = action else {
            return Err(InternalSymmetryCompatibilityError::NonMonomialDenominator {
                source_denominator: source,
            });
        };
        if scale != &one {
            return Err(
                InternalSymmetryCompatibilityError::NonUnitDenominatorScale {
                    source_denominator: source,
                    target_denominator: *target,
                },
            );
        }
        let Some(hit_count) = target_hits.get_mut(*target) else {
            return Err(
                InternalSymmetryCompatibilityError::NonBijectiveDenominatorAction {
                    target_denominator: *target,
                },
            );
        };
        *hit_count = hit_count.checked_add(1).ok_or(
            InternalSymmetryCompatibilityError::ResourceCountOverflow {
                resource: "denominator-permutation hit counters",
            },
        )?;
        permutation.push(*target);
    }
    for (target, hits) in target_hits.into_iter().enumerate() {
        if hits != 1 {
            return Err(
                InternalSymmetryCompatibilityError::NonBijectiveDenominatorAction {
                    target_denominator: target,
                },
            );
        }
    }

    for (source, &target) in permutation.iter().enumerate() {
        if family.power_shifts()[source] != family.power_shifts()[target] {
            return Err(InternalSymmetryCompatibilityError::PowerShiftMismatch {
                source_denominator: source,
                target_denominator: target,
            });
        }
        let source_cut = restrictions.cuts().required_active().active_bits()[source];
        let target_cut = restrictions.cuts().required_active().active_bits()[target];
        if source_cut != target_cut {
            return Err(InternalSymmetryCompatibilityError::CutTransportMismatch {
                source_denominator: source,
                target_denominator: target,
            });
        }
        if restrictions.pattern().slots()[source] != restrictions.pattern().slots()[target] {
            return Err(
                InternalSymmetryCompatibilityError::SectorPatternTransportMismatch {
                    source_denominator: source,
                    target_denominator: target,
                },
            );
        }
    }

    Ok(VerifiedInternalFamilyPermutationSymmetry {
        family_fingerprint,
        restrictions_fingerprint: restriction_fingerprint(restrictions),
        restrictions: restrictions.clone(),
        denominator_permutation: permutation,
        affine_map,
    })
}

/// Exhaustively search the named finite integer-entry domain unless an
/// execution bound is reached.
pub fn discover_bounded_vacuum_internal_symmetries(
    family: &IntegralFamily,
    restrictions: &SectorRestrictions,
    limits: InternalSymmetrySearchLimits,
) -> Result<InternalSymmetrySearchReport, InternalSymmetrySearchError> {
    if family.external_count() != 0 {
        return Err(InternalSymmetrySearchError::NonVacuumFamily {
            external_momenta: family.external_count(),
        });
    }
    if family.loop_count() == 0 {
        return Err(InternalSymmetrySearchError::EmptyLoopSpace);
    }
    if restrictions.arity() != family.denominator_count() {
        return Err(InternalSymmetrySearchError::WrongRestrictionArity {
            expected: family.denominator_count(),
            actual: restrictions.arity(),
        });
    }

    let domain_fingerprint = search_domain_fingerprint(family, restrictions, limits);
    let matrix_entries = family.loop_count().checked_mul(family.loop_count()).ok_or(
        InternalSymmetrySearchError::ResourceCountOverflow {
            resource: "loop-map entries",
        },
    )?;
    let mut stats = InternalSymmetrySearchStats::default();
    let mut retained = Vec::<VerifiedInternalFamilyPermutationSymmetry>::new();
    if matrix_entries > limits.max_loop_map_entries {
        return Ok(finish_report(
            retained,
            resource_limited(
                domain_fingerprint,
                "loop-map entries",
                matrix_entries,
                limits.max_loop_map_entries,
            ),
            stats,
        ));
    }

    let radius = i64::from(limits.coefficient_radius);
    let determinant_bit_bound = integer_determinant_bit_bound(family.loop_count(), radius).ok_or(
        InternalSymmetrySearchError::ResourceCountOverflow {
            resource: "integer determinant bit bound",
        },
    )?;
    if determinant_bit_bound > limits.max_integer_bits {
        return Ok(finish_report(
            retained,
            resource_limited(
                domain_fingerprint,
                "integer determinant bits",
                determinant_bit_bound,
                limits.max_integer_bits,
            ),
            stats,
        ));
    }
    let mut entries = Vec::new();
    if entries.try_reserve_exact(matrix_entries).is_err() {
        return Ok(finish_report(
            retained,
            resource_limited(domain_fingerprint, "loop-map allocation", matrix_entries, 0),
            stats,
        ));
    }
    entries.resize(matrix_entries, -radius);
    loop {
        if stats.enumerated_matrices >= limits.max_enumerated_matrices {
            let requested = stats.enumerated_matrices.checked_add(1).ok_or(
                InternalSymmetrySearchError::ResourceCountOverflow {
                    resource: "enumerated loop maps",
                },
            )?;
            return Ok(finish_report(
                retained,
                resource_limited(
                    domain_fingerprint,
                    "enumerated loop maps",
                    requested,
                    limits.max_enumerated_matrices,
                ),
                stats,
            ));
        }
        stats.enumerated_matrices += 1;

        let determinant = match checked_integer_determinant(
            &entries,
            family.loop_count(),
            &mut stats.integer_determinant_operations,
            limits.max_integer_determinant_operations,
        ) {
            Ok(value) => value,
            Err(DeterminantFailure::Limit(bound)) => {
                return Ok(finish_report(
                    retained,
                    resource_limited(
                        domain_fingerprint,
                        bound.resource,
                        bound.requested,
                        bound.limit,
                    ),
                    stats,
                ));
            }
            Err(DeterminantFailure::CountOverflow { resource }) => {
                return Err(InternalSymmetrySearchError::ResourceCountOverflow { resource });
            }
            Err(DeterminantFailure::NonExactDivision) => {
                return Err(InternalSymmetrySearchError::IntegerDeterminantExactDivisionFailure);
            }
        };

        if determinant == 1 || determinant == -1 {
            stats.unimodular_candidates += 1;
            if stats.verifier_calls >= limits.max_verifier_calls {
                let requested = stats.verifier_calls.checked_add(1).ok_or(
                    InternalSymmetrySearchError::ResourceCountOverflow {
                        resource: "affine verifier calls",
                    },
                )?;
                return Ok(finish_report(
                    retained,
                    resource_limited(
                        domain_fingerprint,
                        "affine verifier calls",
                        requested,
                        limits.max_verifier_calls,
                    ),
                    stats,
                ));
            }
            stats.verifier_calls += 1;
            let momentum = match integer_vacuum_momentum_map(
                family.coefficient_context(),
                family.loop_count(),
                &entries,
                limits.verification.max_matrix_entries,
            ) {
                Ok(momentum) => momentum,
                Err(InternalSymmetrySearchError::MatrixConstruction(error)) => {
                    if let Some(bound) = verification_resource_bound(&error) {
                        return Ok(finish_report(
                            retained,
                            resource_limited(
                                domain_fingerprint,
                                bound.resource,
                                bound.requested,
                                bound.limit,
                            ),
                            stats,
                        ));
                    }
                    return Err(InternalSymmetrySearchError::MatrixConstruction(error));
                }
                Err(error) => return Err(error),
            };
            let affine =
                match verify_affine_family_map(family, family, momentum, limits.verification) {
                    Ok(verified) => verified,
                    Err(error) => {
                        if let Some(bound) = verification_resource_bound(&error) {
                            return Ok(finish_report(
                                retained,
                                resource_limited(
                                    domain_fingerprint,
                                    bound.resource,
                                    bound.requested,
                                    bound.limit,
                                ),
                                stats,
                            ));
                        }
                        // The integer determinant prefilter has already proved
                        // this vacuum self-map unimodular.  A complete affine
                        // family basis must therefore replay for every such
                        // momentum map.  Treating any verifier failure as an
                        // ordinary rejected candidate could silently turn an
                        // internal proof defect into an exhaustive report.
                        return Err(InternalSymmetrySearchError::UnexpectedVerificationFailure(
                            error,
                        ));
                    }
                };

            match compile_internal_family_permutation_symmetry(family, restrictions, affine) {
                Ok(certificate) => {
                    let duplicate = retained.iter().any(|existing| {
                        existing.denominator_permutation == certificate.denominator_permutation
                    });
                    if duplicate {
                        stats.duplicate_row_actions += 1;
                    } else {
                        if retained.len() >= limits.max_retained_symmetries {
                            let requested = retained.len().checked_add(1).ok_or(
                                InternalSymmetrySearchError::ResourceCountOverflow {
                                    resource: "retained integral symmetries",
                                },
                            )?;
                            return Ok(finish_report(
                                retained,
                                resource_limited(
                                    domain_fingerprint,
                                    "retained integral symmetries",
                                    requested,
                                    limits.max_retained_symmetries,
                                ),
                                stats,
                            ));
                        }
                        let certificate_entries = retained_certificate_entry_count(&certificate)?;
                        let next_entries = stats
                            .retained_certificate_entries
                            .checked_add(certificate_entries)
                            .ok_or(InternalSymmetrySearchError::ResourceCountOverflow {
                                resource: "retained certificate entries",
                            })?;
                        if next_entries > limits.max_retained_certificate_entries {
                            return Ok(finish_report(
                                retained,
                                resource_limited(
                                    domain_fingerprint,
                                    "retained certificate entries",
                                    next_entries,
                                    limits.max_retained_certificate_entries,
                                ),
                                stats,
                            ));
                        }
                        let next_bytes = match retained_certificate_debug_bytes(
                            &certificate,
                            stats.retained_certificate_bytes,
                            limits.max_retained_certificate_bytes,
                        ) {
                            Ok(bytes) => bytes,
                            Err(CertificateByteCountFailure::Limit(bound)) => {
                                return Ok(finish_report(
                                    retained,
                                    resource_limited(
                                        domain_fingerprint,
                                        bound.resource,
                                        bound.requested,
                                        bound.limit,
                                    ),
                                    stats,
                                ));
                            }
                            Err(CertificateByteCountFailure::CountOverflow) => {
                                return Err(InternalSymmetrySearchError::ResourceCountOverflow {
                                    resource: "retained certificate bytes",
                                });
                            }
                        };
                        if retained.try_reserve(1).is_err() {
                            let requested = retained.len().checked_add(1).ok_or(
                                InternalSymmetrySearchError::ResourceCountOverflow {
                                    resource: "retained symmetry allocation",
                                },
                            )?;
                            return Ok(finish_report(
                                retained,
                                resource_limited(
                                    domain_fingerprint,
                                    "retained symmetry allocation",
                                    requested,
                                    limits.max_retained_symmetries,
                                ),
                                stats,
                            ));
                        }
                        retained.push(certificate);
                        stats.retained_symmetries = retained.len();
                        stats.retained_certificate_entries = next_entries;
                        stats.retained_certificate_bytes = next_bytes;
                    }
                }
                Err(InternalSymmetryCompatibilityError::AllocationFailure {
                    resource,
                    requested,
                }) => {
                    return Ok(finish_report(
                        retained,
                        resource_limited(domain_fingerprint, resource, requested, 0),
                        stats,
                    ));
                }
                Err(InternalSymmetryCompatibilityError::ResourceCountOverflow { resource }) => {
                    return Err(InternalSymmetrySearchError::ResourceCountOverflow { resource });
                }
                Err(_) => stats.incompatible_integral_maps += 1,
            }
        }

        if !increment_matrix(&mut entries, radius) {
            break;
        }
    }

    Ok(finish_report(
        retained,
        InternalSymmetrySearchCompletion::ExhaustiveWithinBounds { domain_fingerprint },
        stats,
    ))
}

fn finish_report(
    mut symmetries: Vec<VerifiedInternalFamilyPermutationSymmetry>,
    completion: InternalSymmetrySearchCompletion,
    stats: InternalSymmetrySearchStats,
) -> InternalSymmetrySearchReport {
    symmetries.sort_by(|left, right| {
        left.denominator_permutation
            .cmp(&right.denominator_permutation)
    });
    InternalSymmetrySearchReport {
        symmetries,
        completion,
        stats,
    }
}

fn retained_certificate_entry_count(
    certificate: &VerifiedInternalFamilyPermutationSymmetry,
) -> Result<usize, InternalSymmetrySearchError> {
    fn add(total: &mut usize, amount: usize) -> Result<(), InternalSymmetrySearchError> {
        *total = total.checked_add(amount).ok_or(
            InternalSymmetrySearchError::ResourceCountOverflow {
                resource: "retained certificate entries",
            },
        )?;
        Ok(())
    }

    let affine = certificate.affine_map();
    let affine_stats = affine.stats();
    let mut entries = 0usize;
    add(&mut entries, affine_stats.matrix_entries())?;
    add(&mut entries, affine_stats.guard_polynomials())?;
    add(&mut entries, affine_stats.guard_origins())?;
    add(&mut entries, affine.row_actions().len())?;
    add(&mut entries, affine.candidate_denominator_guards().len())?;
    add(&mut entries, certificate.denominator_permutation().len())?;
    add(
        &mut entries,
        certificate.restrictions().arity().checked_mul(2).ok_or(
            InternalSymmetrySearchError::ResourceCountOverflow {
                resource: "retained certificate entries",
            },
        )?,
    )?;
    // Two fingerprints, two determinant coefficients, and the two owned
    // family-domain basis determinants.
    add(&mut entries, 6)?;
    for domain in [affine.source_domain(), affine.target_domain()] {
        add(&mut entries, domain.input_denominators().len())?;
        add(&mut entries, 1)?; // separately owned determinant condition
        for condition in domain
            .input_denominators()
            .iter()
            .chain(std::iter::once(domain.determinant_nonzero()))
        {
            add(&mut entries, condition.origins().len())?;
        }
    }
    Ok(entries)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CertificateByteCountFailure {
    Limit(WorkBound),
    CountOverflow,
}

struct BoundedCertificateByteCounter {
    used: usize,
    limit: usize,
    failure: Option<CertificateByteCountFailure>,
}

impl fmt::Write for BoundedCertificateByteCounter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let Some(requested) = self.used.checked_add(value.len()) else {
            self.failure = Some(CertificateByteCountFailure::CountOverflow);
            return Err(fmt::Error);
        };
        if requested > self.limit {
            self.failure = Some(CertificateByteCountFailure::Limit(WorkBound {
                resource: "retained certificate bytes",
                requested,
                limit: self.limit,
            }));
            return Err(fmt::Error);
        }
        self.used = requested;
        Ok(())
    }
}

fn retained_certificate_debug_bytes(
    certificate: &VerifiedInternalFamilyPermutationSymmetry,
    already_retained: usize,
    limit: usize,
) -> Result<usize, CertificateByteCountFailure> {
    let mut counter = BoundedCertificateByteCounter {
        used: already_retained,
        limit,
        failure: None,
    };
    if write!(&mut counter, "{certificate:?}").is_err() {
        return Err(counter
            .failure
            .unwrap_or(CertificateByteCountFailure::CountOverflow));
    }
    Ok(counter.used)
}

fn search_domain_fingerprint(
    family: &IntegralFamily,
    restrictions: &SectorRestrictions,
    limits: InternalSymmetrySearchLimits,
) -> String {
    let family_fingerprint = family.fingerprint();
    format!(
        "{BOUNDED_INTEGER_VACUUM_SYMMETRY_SEARCH_V1_SCHEMA}|family={}:{}|L={}|E=0|A=integer[-{},{}]|B=zero|C=empty|cuts={}|pattern={}",
        family_fingerprint.len(),
        family_fingerprint,
        family.loop_count(),
        limits.coefficient_radius,
        limits.coefficient_radius,
        restrictions.cuts().to_bit_string(),
        restrictions.pattern().to_stable_string(),
    )
}

fn restriction_fingerprint(restrictions: &SectorRestrictions) -> String {
    format!(
        "rustred-sector-restrictions-v1|arity={}|cuts={}|pattern={}",
        restrictions.arity(),
        restrictions.cuts().to_bit_string(),
        restrictions.pattern().to_stable_string(),
    )
}

fn integer_vacuum_momentum_map(
    context: &CoefficientContext,
    loops: usize,
    entries: &[i64],
    max_matrix_entries: usize,
) -> Result<MomentumMap, InternalSymmetrySearchError> {
    let loop_linear = ExactMatrix::try_new_with_max_entries(
        loops,
        loops,
        entries.iter().map(|&entry| context.integer(entry)),
        max_matrix_entries,
    )
    .map_err(InternalSymmetrySearchError::MatrixConstruction)?;
    let loop_external =
        ExactMatrix::try_new_with_max_entries(loops, 0, std::iter::empty(), max_matrix_entries)
            .map_err(InternalSymmetrySearchError::MatrixConstruction)?;
    let external_linear =
        ExactMatrix::try_new_with_max_entries(0, 0, std::iter::empty(), max_matrix_entries)
            .map_err(InternalSymmetrySearchError::MatrixConstruction)?;
    Ok(MomentumMap::new(
        loop_linear,
        loop_external,
        external_linear,
    ))
}

fn increment_matrix(entries: &mut [i64], radius: i64) -> bool {
    for entry in entries.iter_mut().rev() {
        if *entry < radius {
            *entry += 1;
            return true;
        }
        *entry = -radius;
    }
    false
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WorkBound {
    resource: &'static str,
    requested: usize,
    limit: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeterminantFailure {
    Limit(WorkBound),
    CountOverflow { resource: &'static str },
    NonExactDivision,
}

fn charge_determinant_operation(used: &mut usize, limit: usize) -> Result<(), DeterminantFailure> {
    let requested = used
        .checked_add(1)
        .ok_or(DeterminantFailure::CountOverflow {
            resource: "integer determinant operations",
        })?;
    if requested > limit {
        return Err(DeterminantFailure::Limit(WorkBound {
            resource: "integer determinant operations",
            requested,
            limit,
        }));
    }
    *used = requested;
    Ok(())
}

/// Fraction-free Bareiss determinant over Symbolica's GMP-backed exact
/// integer type.  [`integer_determinant_bit_bound`] preflights all minors.
fn checked_integer_determinant(
    entries: &[i64],
    size: usize,
    operations: &mut usize,
    operation_limit: usize,
) -> Result<Integer, DeterminantFailure> {
    debug_assert_eq!(entries.len(), size * size);
    if size == 1 {
        return Ok(Integer::from(entries[0]));
    }
    let mut matrix = Vec::new();
    matrix.try_reserve_exact(entries.len()).map_err(|_| {
        DeterminantFailure::Limit(WorkBound {
            resource: "integer determinant scratch allocation",
            requested: entries.len(),
            limit: 0,
        })
    })?;
    matrix.extend(entries.iter().map(|&entry| Integer::from(entry)));
    let mut previous_pivot = Integer::one();
    let mut negative = false;
    for pivot_column in 0..size - 1 {
        let Some(pivot_row) =
            (pivot_column..size).find(|&row| !matrix[row * size + pivot_column].is_zero())
        else {
            return Ok(Integer::zero());
        };
        if pivot_row != pivot_column {
            for column in 0..size {
                matrix.swap(pivot_row * size + column, pivot_column * size + column);
            }
            negative = !negative;
        }
        let pivot = matrix[pivot_column * size + pivot_column].clone();
        for row in pivot_column + 1..size {
            for column in pivot_column + 1..size {
                charge_determinant_operation(operations, operation_limit)?;
                let diagonal = &matrix[row * size + column] * &pivot;
                charge_determinant_operation(operations, operation_limit)?;
                let cross =
                    &matrix[row * size + pivot_column] * &matrix[pivot_column * size + column];
                charge_determinant_operation(operations, operation_limit)?;
                let numerator = diagonal - cross;
                if pivot_column == 0 {
                    matrix[row * size + column] = numerator;
                } else {
                    charge_determinant_operation(operations, operation_limit)?;
                    if previous_pivot.is_zero() {
                        return Err(DeterminantFailure::NonExactDivision);
                    }
                    let quotient = &numerator / &previous_pivot;
                    charge_determinant_operation(operations, operation_limit)?;
                    if &quotient * &previous_pivot != numerator {
                        return Err(DeterminantFailure::NonExactDivision);
                    }
                    matrix[row * size + column] = quotient;
                }
            }
            matrix[row * size + pivot_column] = Integer::zero();
        }
        previous_pivot = pivot;
    }
    let determinant = matrix[(size - 1) * size + size - 1].clone();
    Ok(if negative { -determinant } else { determinant })
}

/// Conservative bit bound from the Leibniz inequality
/// `|det(A)| <= n! radius^n`.  Bareiss entries are minors, so the largest
/// bound also covers every intermediate.
fn integer_determinant_bit_bound(size: usize, radius: i64) -> Option<usize> {
    let radius = radius.unsigned_abs();
    let radius_bits = if radius <= 1 {
        usize::from(radius != 0)
    } else {
        (u64::BITS - (radius - 1).leading_zeros()) as usize
    };
    let mut bound = size.checked_mul(radius_bits)?;
    for factor in 2..=size {
        let ceil_log2 = (usize::BITS - (factor - 1).leading_zeros()) as usize;
        bound = bound.checked_add(ceil_log2)?;
    }
    bound.checked_add(1)
}

fn resource_limited(
    domain_fingerprint: String,
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> InternalSymmetrySearchCompletion {
    InternalSymmetrySearchCompletion::ResourceLimited {
        domain_fingerprint,
        resource,
        requested,
        limit,
    }
}

fn verification_resource_bound(error: &SymmetryVerificationError) -> Option<WorkBound> {
    match error {
        SymmetryVerificationError::ResourceLimit {
            resource,
            requested,
            limit,
        }
        | SymmetryVerificationError::ExactAlgebra(ExactAlgebraError::ResourceLimit {
            resource,
            requested,
            limit,
        }) => Some(WorkBound {
            resource,
            requested: *requested,
            limit: *limit,
        }),
        SymmetryVerificationError::AllocationFailure {
            resource,
            requested,
        } => Some(WorkBound {
            resource,
            requested: *requested,
            limit: 0,
        }),
        SymmetryVerificationError::ResourceCountOverflow { resource }
        | SymmetryVerificationError::ExactAlgebra(ExactAlgebraError::ResourceCountOverflow {
            resource,
        }) => Some(WorkBound {
            resource,
            requested: usize::MAX,
            limit: usize::MAX - 1,
        }),
        SymmetryVerificationError::ExactAlgebra(ExactAlgebraError::ExponentLimit {
            requested,
            limit,
            ..
        }) => Some(WorkBound {
            resource: "exact coefficient exponent",
            requested: usize::try_from(*requested).unwrap_or(usize::MAX),
            limit: usize::try_from(*limit).unwrap_or(usize::MAX),
        }),
        SymmetryVerificationError::ExactAlgebra(ExactAlgebraError::ConfiguredExponentLimit {
            requested,
            representation_limit,
        }) => Some(WorkBound {
            resource: "configured exact coefficient exponent",
            requested: usize::try_from(*requested).unwrap_or(usize::MAX),
            limit: usize::try_from(*representation_limit).unwrap_or(usize::MAX),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{DeterminantFailure, WorkBound, checked_integer_determinant, increment_matrix};

    #[test]
    fn checked_bareiss_determinant_and_odometer_are_exact() {
        let mut operations = 0;
        assert_eq!(
            checked_integer_determinant(&[1, 1, 1, 0], 2, &mut operations, 100).unwrap(),
            -1
        );
        assert_eq!(
            checked_integer_determinant(&[2, 4, 1, 2], 2, &mut operations, 100).unwrap(),
            0
        );

        let mut values = vec![-1, -1];
        let mut seen = vec![values.clone()];
        while increment_matrix(&mut values, 1) {
            seen.push(values.clone());
        }
        assert_eq!(seen.len(), 9);
        assert_eq!(seen.first().unwrap(), &vec![-1, -1]);
        assert_eq!(seen.last().unwrap(), &vec![1, 1]);
    }

    #[test]
    fn bareiss_matches_independent_three_by_three_formula_on_full_radius_one_domain() {
        let mut entries = vec![-1i64; 9];
        loop {
            let expected = entries[0] * (entries[4] * entries[8] - entries[5] * entries[7])
                - entries[1] * (entries[3] * entries[8] - entries[5] * entries[6])
                + entries[2] * (entries[3] * entries[7] - entries[4] * entries[6]);
            let mut operations = 0;
            assert_eq!(
                checked_integer_determinant(&entries, 3, &mut operations, 1_000).unwrap(),
                expected
            );
            if !increment_matrix(&mut entries, 1) {
                break;
            }
        }
    }

    #[test]
    fn bareiss_operation_budget_includes_exact_division_replay_multiplication() {
        let entries = [1, 1, 0, 0, 1, 1, 1, 0, 1];
        let mut operations = 0;
        assert_eq!(
            checked_integer_determinant(&entries, 3, &mut operations, 16).unwrap_err(),
            DeterminantFailure::Limit(WorkBound {
                resource: "integer determinant operations",
                requested: 17,
                limit: 16,
            })
        );
        assert_eq!(operations, 16);

        let mut operations = 0;
        assert_eq!(
            checked_integer_determinant(&entries, 3, &mut operations, 17).unwrap(),
            2
        );
        assert_eq!(operations, 17);
    }
}
