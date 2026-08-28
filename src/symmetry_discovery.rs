//! Proof-carrying compilation of verified affine self-maps into integral symmetries.
//!
//! Candidate discovery is deliberately outside this module. Callers may use
//! explicit maps, graph automorphisms, routing equivalences, or future generic
//! candidate backends, but every candidate must first pass through
//! [`crate::verify_affine_family_map`]. This module then accepts the verified
//! affine map only when its denominator action is a bijective unit-scale
//! permutation preserving formal power shifts, cuts, and sector-pattern slots.

use std::fmt;

use crate::{
    ConcreteIntegralKey, DenominatorRowAction, IntegralFamily, JacobianWitness, SectorRestrictions,
    SymmetryVerificationError, SymmetryVerificationLimits, VerifiedAffineFamilyMap,
};

pub const INTERNAL_FAMILY_PERMUTATION_SYMMETRY_V1_SCHEMA: &str =
    "rustred-internal-family-permutation-symmetry-v1";

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
/// family permutation. Candidate discovery remains outside this proof boundary.
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

fn restriction_fingerprint(restrictions: &SectorRestrictions) -> String {
    format!(
        "rustred-sector-restrictions-v1|arity={}|cuts={}|pattern={}",
        restrictions.arity(),
        restrictions.cuts().to_bit_string(),
        restrictions.pattern().to_stable_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::{InternalSymmetryCompatibilityError, compile_internal_family_permutation_symmetry};
    use crate::{
        AffineDenominator, CoefficientLocation, ConcreteIntegralKey, CutConstraint, ExactMatrix,
        IntegralFamily, MomentumMap, SectorPattern, SectorPatternSlot, SectorRestrictions,
        SymmetryVerificationLimits, algebra::Coefficient, algebra::CoefficientContext,
        symmetry::SymmetryConditionSource, verify_affine_family_map,
    };

    fn equal_mass_sunset() -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let zero = coefficients.zero();
        let one = coefficients.one();
        let minus_m2 = coefficients.coefficient_fixture("-m2");
        IntegralFamily::new(
            "explicit-symmetry-candidate",
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

    fn swap_loop_momenta(coefficients: &CoefficientContext) -> MomentumMap {
        let zero = coefficients.zero();
        let one = coefficients.one();
        MomentumMap::new(
            ExactMatrix::try_new(2, 2, [zero.clone(), one.clone(), one, zero]).unwrap(),
            ExactMatrix::<Coefficient>::try_new(2, 0, []).unwrap(),
            ExactMatrix::<Coefficient>::try_new(0, 0, []).unwrap(),
        )
    }

    #[test]
    fn explicit_candidate_compiles_replays_and_respects_restrictions() {
        let family = equal_mass_sunset();
        let verified = verify_affine_family_map(
            &family,
            &family,
            swap_loop_momenta(family.coefficient_context()),
            SymmetryVerificationLimits::default(),
        )
        .unwrap();
        let determinant_condition = verified
            .nonzero_conditions()
            .iter()
            .find(|condition| {
                condition.polynomial() == family.domain().determinant_nonzero().polynomial()
            })
            .unwrap();
        assert!(
            determinant_condition
                .sources()
                .contains(&SymmetryConditionSource::SourceFamily(
                    CoefficientLocation::BasisDeterminantNumerator,
                ))
        );
        assert!(
            determinant_condition
                .sources()
                .contains(&SymmetryConditionSource::TargetFamily(
                    CoefficientLocation::BasisDeterminantNumerator,
                ))
        );
        assert_eq!(
            verified.stats().condition_sources(),
            verified
                .nonzero_conditions()
                .iter()
                .map(|condition| condition.sources().len())
                .sum::<usize>()
        );
        let unrestricted = SectorRestrictions::unrestricted(family.denominator_count()).unwrap();
        let symmetry =
            compile_internal_family_permutation_symmetry(&family, &unrestricted, verified.clone())
                .unwrap();

        assert_eq!(symmetry.denominator_permutation(), &[1, 0, 2]);
        let source = ConcreteIntegralKey::try_new([2, 3, -1]).unwrap();
        let target = ConcreteIntegralKey::try_new([3, 2, -1]).unwrap();
        assert_eq!(symmetry.transport_source_key(&source).unwrap(), target);
        symmetry.replay_key_transport(&source, &target).unwrap();
        symmetry
            .replay(
                &family,
                &unrestricted,
                SymmetryVerificationLimits::default(),
            )
            .unwrap();

        let asymmetric = SectorRestrictions::try_new(
            CutConstraint::none(family.denominator_count()).unwrap(),
            SectorPattern::try_new([
                SectorPatternSlot::Active,
                SectorPatternSlot::Any,
                SectorPatternSlot::Any,
            ])
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            compile_internal_family_permutation_symmetry(&family, &asymmetric, verified),
            Err(
                InternalSymmetryCompatibilityError::SectorPatternTransportMismatch {
                    source_denominator: 0,
                    target_denominator: 1,
                }
            )
        ));
    }
}
