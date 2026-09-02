use std::collections::BTreeSet;

use crate::algebra::IndexedCoefficientContext;
use crate::family::{IntegralFamily, IntegralKey};
use crate::sector::OrderingPolicy;
use crate::sector::symmetry::Canonicalizer;

use super::super::error::ArtifactError;
use super::super::factorization::FactorizationRule;
use super::super::model::{ArtifactSchemaVersion, ClosedArtifact, ZeroSectorTerminal};
use super::super::{ClosedTerminalAuthority, DeclaredMasterManifest};
use super::{TerminalBindings, factorization, validate_terminal_bindings};

/// Untrusted construction payload for one locally complete terminal registry.
///
/// The payload contains no ordinary rules and therefore cannot be confused
/// with a closing artifact candidate. Installation proves only the listed
/// zero and factorization regions, and seals explicitly declared finite
/// same-family master points as an intentional evaluation policy.
pub(crate) struct TerminalAuthorityCandidate {
    pub(crate) schema: ArtifactSchemaVersion,
    pub(crate) authority_id: &'static str,
    pub(crate) arity: usize,
    pub(crate) family: IntegralFamily,
    pub(crate) context: IndexedCoefficientContext,
    pub(crate) canonicalizer: Option<Canonicalizer>,
    pub(crate) dependencies: Vec<Box<ClosedArtifact>>,
    pub(crate) factorization_rules: Vec<FactorizationRule>,
    pub(crate) parent_terminals: BTreeSet<IntegralKey>,
    /// Deliberately selected same-family numerical-master points. Unlike
    /// `parent_terminals`, these need not be factorization outputs.
    pub(crate) declared_master_terminals: Vec<IntegralKey>,
    pub(crate) zero_sectors: Vec<ZeroSectorTerminal>,
    pub(crate) expected_ordering: OrderingPolicy,
}

/// Seal terminal authority after one exact generic replay at this cold
/// boundary. The returned owner never reruns Symbolica validation during
/// lookup or immutable-snapshot verification.
pub(crate) fn install_terminal_authority(
    mut candidate: TerminalAuthorityCandidate,
) -> Result<ClosedTerminalAuthority, ArtifactError> {
    if candidate.authority_id.is_empty() {
        return Err(ArtifactError::InvalidRuleShape {
            detail: "the terminal-authority identifier is empty",
        });
    }
    validate_terminal_bindings(TerminalBindings {
        schema: candidate.schema,
        arity: candidate.arity,
        family: &candidate.family,
        context: &candidate.context,
        canonicalizer: candidate.canonicalizer.as_ref(),
        parent_terminals: &candidate.parent_terminals,
        zero_sectors: &candidate.zero_sectors,
        require_parent_terminals: false,
        expected_ordering: candidate.expected_ordering,
    })?;
    factorization::validate_and_compile(
        factorization::InstallContext::new(
            candidate.arity,
            &candidate.family,
            candidate.canonicalizer.as_ref(),
            &candidate.dependencies,
            &candidate.parent_terminals,
            &candidate.zero_sectors,
        ),
        &mut candidate.factorization_rules,
    )?;
    let compiled_parent_terminals = candidate
        .factorization_rules
        .iter()
        .flat_map(|rule| rule.master_embeddings())
        .map(|embedding| embedding.parent_terminal().clone())
        .collect::<BTreeSet<_>>();
    if compiled_parent_terminals != candidate.parent_terminals {
        return Err(ArtifactError::InvalidMasterManifest);
    }
    let declared_masters = install_declared_master_manifest(
        candidate.arity,
        &candidate.family,
        &candidate.context,
        candidate.canonicalizer.as_ref(),
        &candidate.zero_sectors,
        candidate.declared_master_terminals,
    )?;
    for parent in &candidate.parent_terminals {
        let canonical_parent = match candidate.canonicalizer.as_ref() {
            Some(canonicalizer) => canonicalizer.canonicalize(parent)?.canonical().clone(),
            None => parent.clone(),
        };
        if declared_masters.terminals().contains(&canonical_parent) {
            return Err(ArtifactError::InvalidDeclaredMasterManifest {
                detail: "a declared master duplicates a factorization-image terminal",
            });
        }
    }
    let factorized_product_programs =
        super::super::factorized_product_moments::compile_factorized_product_moment_programs(
            &candidate.family,
            &candidate.dependencies,
            &candidate.factorization_rules,
        )
        .map_err(|_| ArtifactError::InvalidFactorization {
            detail: "the authenticated product factorization could not compile its exact dependency-root preimage executor",
        })?;
    Ok(ClosedTerminalAuthority::from_validated_parts(
        candidate.authority_id,
        candidate.arity,
        candidate.family,
        candidate.context,
        candidate.canonicalizer,
        candidate.dependencies,
        candidate.factorization_rules,
        factorized_product_programs,
        candidate.parent_terminals,
        declared_masters,
        candidate.zero_sectors,
    ))
}

fn install_declared_master_manifest(
    arity: usize,
    family: &IntegralFamily,
    context: &IndexedCoefficientContext,
    canonicalizer: Option<&Canonicalizer>,
    zero_sectors: &[ZeroSectorTerminal],
    terminals: Vec<IntegralKey>,
) -> Result<DeclaredMasterManifest, ArtifactError> {
    if context.index_count() != arity {
        return Err(ArtifactError::WrongArity {
            expected: context.index_count(),
            actual: arity,
        });
    }
    if family.denominator_count() != arity
        || !family
            .coefficient_context()
            .has_same_variable_map(context.base())
    {
        return Err(ArtifactError::WrongCoefficientContext);
    }
    if canonicalizer.is_some_and(|canonicalizer| {
        canonicalizer.arity() != arity || canonicalizer.family_fingerprint() != family.fingerprint()
    }) {
        return Err(ArtifactError::InvalidCanonicalizer);
    }

    let mut canonical_terminals = BTreeSet::new();
    for terminal in terminals {
        if terminal.powers().len() != arity {
            return Err(ArtifactError::WrongArity {
                expected: arity,
                actual: terminal.powers().len(),
            });
        }
        let canonical = match canonicalizer {
            Some(canonicalizer) => canonicalizer.canonicalize(&terminal)?.canonical().clone(),
            None => terminal,
        };
        if zero_sectors.iter().any(|zero| {
            zero.sector()
                .active_bits()
                .iter()
                .zip(canonical.powers())
                .all(|(&active, &power)| active == (power >= 1))
        }) {
            return Err(ArtifactError::InvalidDeclaredMasterManifest {
                detail: "a declared master belongs to an authenticated zero sector",
            });
        }
        if !canonical_terminals.insert(canonical) {
            return Err(ArtifactError::InvalidDeclaredMasterManifest {
                detail: "declared masters contain duplicate symmetry-orbit representatives",
            });
        }
    }
    Ok(DeclaredMasterManifest::from_validated_parts(
        arity,
        family.fingerprint_owner(),
        context.fingerprint_owner(),
        canonical_terminals,
    ))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::algebra::CoefficientContext;
    use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};
    use crate::identity::ParametricIbpGenerator;
    use crate::sector::symmetry::permutation::compile;
    use crate::sector::symmetry::{
        CanonicalizationLimits, Canonicalizer, CoefficientMatrix, Limits, MomentumMap, verify,
    };
    use crate::sector::{Mask, OrderingPolicy};

    use super::super::super::model::{
        ArtifactSchemaVersion, ZeroSectorTerminal, ZeroTerminalProof,
    };
    use super::{ArtifactError, TerminalAuthorityCandidate, install_terminal_authority};

    fn family(identity: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::try_new(["d"]).unwrap();
        let dimension = coefficients.parameter("d").unwrap();
        let one = coefficients.one();
        let minus_one = coefficients.integer(-1);
        let zero = coefficients.zero();
        IntegralFamily::new(
            identity,
            vec!["k".to_owned()],
            Vec::new(),
            coefficients,
            dimension,
            vec![AffineDenominator::new(minus_one, vec![one])],
            Vec::new(),
            vec![zero],
        )
        .unwrap()
    }

    fn candidate() -> TerminalAuthorityCandidate {
        let family = family("terminal-authority-installer-test");
        let generator = ParametricIbpGenerator::try_new(&family).unwrap();
        let context = generator.context().clone();
        drop(generator);
        TerminalAuthorityCandidate {
            schema: ArtifactSchemaVersion::CURRENT,
            authority_id: "rustred.test.terminal-authority.v1",
            arity: 1,
            family,
            context,
            canonicalizer: None,
            dependencies: Vec::new(),
            factorization_rules: Vec::new(),
            parent_terminals: BTreeSet::new(),
            declared_master_terminals: Vec::new(),
            zero_sectors: vec![ZeroSectorTerminal::new(
                Mask::try_new([false]).unwrap(),
                ZeroTerminalProof::ScalelessVacuumPolynomial,
            )],
            expected_ordering: OrderingPolicy::default(),
        }
    }

    fn canonicalizer_for(family: &IntegralFamily) -> Canonicalizer {
        let coefficients = family.coefficient_context();
        let identity = MomentumMap::new(
            CoefficientMatrix::try_new(1, 1, [coefficients.one()]).unwrap(),
            CoefficientMatrix::try_new(1, 0, []).unwrap(),
            CoefficientMatrix::try_new(0, 0, []).unwrap(),
        );
        let verified = verify(family, family, identity, Limits::default()).unwrap();
        let permutation = compile(family, verified).unwrap();
        Canonicalizer::try_new(
            OrderingPolicy::default(),
            [permutation],
            CanonicalizationLimits::default(),
        )
        .unwrap()
    }

    #[test]
    fn terminal_authority_seals_without_claiming_rule_closure() {
        let authority = install_terminal_authority(candidate()).unwrap();
        assert_eq!(authority.arity(), 1);
        assert_eq!(authority.zero_sectors().len(), 1);
        assert!(authority.parent_terminals().is_empty());
        assert!(authority.declared_master_manifest().terminals().is_empty());
        assert!(authority.factorization_rules().is_empty());
    }

    #[test]
    fn invalid_terminal_candidates_never_seal() {
        let mut empty_identity = candidate();
        empty_identity.authority_id = "";
        assert_eq!(
            install_terminal_authority(empty_identity).unwrap_err(),
            ArtifactError::InvalidRuleShape {
                detail: "the terminal-authority identifier is empty",
            }
        );

        let mut unproved_parent = candidate();
        unproved_parent
            .parent_terminals
            .insert(IntegralKey::try_new([1]).unwrap());
        assert_eq!(
            install_terminal_authority(unproved_parent).unwrap_err(),
            ArtifactError::InvalidMasterManifest
        );

        let mut false_zero = candidate();
        false_zero.parent_terminals = BTreeSet::from([IntegralKey::try_new([-1]).unwrap()]);
        false_zero.zero_sectors = vec![ZeroSectorTerminal::new(
            Mask::try_new([true]).unwrap(),
            ZeroTerminalProof::LeePomeranskyRankDeficiency,
        )];
        assert_eq!(
            install_terminal_authority(false_zero).unwrap_err(),
            ArtifactError::InvalidZeroTerminal
        );

        let mut duplicate_zero = candidate();
        duplicate_zero
            .zero_sectors
            .push(duplicate_zero.zero_sectors[0].clone());
        assert_eq!(
            install_terminal_authority(duplicate_zero).unwrap_err(),
            ArtifactError::InvalidZeroTerminal
        );

        let foreign_family = family("terminal-authority-foreign-family");
        let mut foreign_symmetry = candidate();
        foreign_symmetry.canonicalizer = Some(canonicalizer_for(&foreign_family));
        assert_eq!(
            install_terminal_authority(foreign_symmetry).unwrap_err(),
            ArtifactError::InvalidCanonicalizer
        );
    }
}
