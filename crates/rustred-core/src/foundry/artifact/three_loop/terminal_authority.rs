//! Production-sealed terminal authority for the K=6 campaign family.
//!
//! This fixture deliberately proves no ordinary-rule closure. It owns only
//! exact zero sectors, compiled lower-family factorizations, the typed
//! parent-family terminals reached by those factorizations, and a small
//! intentional manifest of provisional same-family numerical masters.

use std::collections::BTreeSet;
use std::sync::{Arc, OnceLock};

use crate::family::IntegralKey;
use crate::foundry::artifact::install::{TerminalAuthorityCandidate, install_terminal_authority};
use crate::foundry::artifact::model::{
    ArtifactSchemaVersion, ZeroSectorTerminal, ZeroTerminalProof,
};
use crate::foundry::artifact::one_loop::derive_one_loop_unit_mass_tadpole;
use crate::foundry::artifact::two_loop::derive_two_loop_unit_mass_sunset;
use crate::foundry::artifact::{ArtifactError, ClosedTerminalAuthority};
use crate::identity::{ParametricIbpConfig, ParametricIbpGenerator};
use crate::sector::OrderingPolicy;

use super::factorization::factorization_rules;
use super::terminals::exact_zero_sectors;
use super::{FULL_RANK_ORBITS, canonical_family, canonical_s4, canonical_s4_with_ordering};

const AUTHORITY_ID: &str = "rustred.three-loop-unit-mass-vacuum-k6.terminal-authority.v1";
static AUTHORITY: OnceLock<Result<Arc<ClosedTerminalAuthority>, ArtifactError>> = OnceLock::new();

/// Install the K=6 terminal registry through the same exact generic proof
/// boundary used by production closing artifacts. The result, including a
/// typed cold-boundary failure, is retained once so campaigns never repeat
/// Symbolica rank or factorization work.
pub(crate) fn derive_k6_terminal_authority() -> Result<Arc<ClosedTerminalAuthority>, ArtifactError>
{
    AUTHORITY.get_or_init(build_k6_terminal_authority).clone()
}

/// Install a distinct K6 terminal authority for authority-identity tests.
///
/// Production callers always use the cached owner above.  This seam exists
/// only to prove that equal structural snapshot payloads do not make two
/// independently installed authorities interchangeable.
#[cfg(test)]
pub(crate) fn fresh_k6_terminal_authority_for_test()
-> Result<Arc<ClosedTerminalAuthority>, ArtifactError> {
    build_k6_terminal_authority()
}

fn build_k6_terminal_authority() -> Result<Arc<ClosedTerminalAuthority>, ArtifactError> {
    install_terminal_authority(k6_candidate()?).map(Arc::new)
}

/// Install an uncached K6 terminal registry under the exact ordering used by
/// an executable rule owner.  This keeps symmetry representatives,
/// factorization embeddings, and recurrence descent coherent.
pub(crate) fn derive_k6_terminal_authority_with_ordering(
    ordering: OrderingPolicy,
) -> Result<ClosedTerminalAuthority, ArtifactError> {
    install_terminal_authority(k6_candidate_with_ordering(ordering)?)
}

fn k6_candidate() -> Result<TerminalAuthorityCandidate, ArtifactError> {
    k6_candidate_with_ordering(OrderingPolicy::default())
}

fn k6_candidate_with_ordering(
    ordering: OrderingPolicy,
) -> Result<TerminalAuthorityCandidate, ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = if ordering == OrderingPolicy::default() {
        canonical_s4(&family)?
    } else {
        canonical_s4_with_ordering(&family, ordering)?
    };
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let context = generator.context().clone();
    drop(generator);

    let zero_sectors = exact_zero_sectors(&canonicalizer)?
        .into_iter()
        .map(|sector| {
            let proof = if sector.active_bits().iter().any(|&active| active) {
                ZeroTerminalProof::LeePomeranskyRankDeficiency
            } else {
                ZeroTerminalProof::ScalelessVacuumPolynomial
            };
            ZeroSectorTerminal::new(sector, proof)
        })
        .collect();
    let dependencies = vec![
        Box::new(derive_two_loop_unit_mass_sunset()?),
        Box::new(derive_one_loop_unit_mass_tadpole()?),
    ];
    let factorization_rules = factorization_rules(&family)?;
    let parent_terminals = [
        IntegralKey::try_new([0, 0, 1, 0, 1, 1])?,
        IntegralKey::try_new([0, 0, 1, 1, 0, 1])?,
        IntegralKey::try_new([0, 0, 1, 1, 1, 1])?,
    ]
    .into_iter()
    .map(|terminal| {
        canonicalizer
            .canonicalize(&terminal)
            .map(|canonical| canonical.canonical().clone())
            .map_err(ArtifactError::from)
    })
    .collect::<Result<BTreeSet<_>, _>>()?;
    // The first three full-rank orbit corners are exact factorization images.
    // The remaining irreducible corners are an explicit, provisional
    // numerical-master policy: only these finite scalar points terminate,
    // never their arbitrary-power sectors.
    let declared_master_terminals = FULL_RANK_ORBITS[3..]
        .iter()
        .map(|orbit| IntegralKey::try_new(orbit.representative))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(TerminalAuthorityCandidate {
        schema: ArtifactSchemaVersion::CURRENT,
        authority_id: AUTHORITY_ID,
        arity: 6,
        family,
        context,
        canonicalizer: Some(canonicalizer),
        dependencies,
        factorization_rules,
        parent_terminals,
        declared_master_terminals,
        zero_sectors,
        expected_ordering: ordering,
    })
}

#[cfg(test)]
mod tests {
    use crate::family::IntegralKey;
    use crate::foundry::artifact::ArtifactError;
    use crate::foundry::artifact::install::install_terminal_authority;

    use super::k6_candidate;

    fn winner_ordering() -> crate::sector::OrderingPolicy {
        let priority =
            crate::sector::CoordinatePriority::try_new(6, &[5, 3, 4, 2, 0, 1], Default::default())
                .unwrap();
        crate::sector::OrderingPolicy::try_with_coordinate_priority(&priority).unwrap()
    }

    #[test]
    fn custom_ordering_coherently_remaps_all_k6_terminal_orbits() {
        let authority = super::derive_k6_terminal_authority_with_ordering(winner_ordering())
            .expect("winner-order K6 terminal authority must install");
        assert_eq!(authority.master_terminal_count(), 6);
        assert_eq!(
            authority.canonicalizer().unwrap().ordering(),
            winner_ordering()
        );

        // Five terminal cells left unmatched by the executable FORM geometry,
        // transported from AlphaLoop slots into RustRed's family slots.
        let alpha_source_for_rust_target = [0, 2, 1, 3, 5, 4];
        let alpha = [
            [0, 0, 1, 0, 1, 1],
            [0, 0, 1, 1, 1, 1],
            [0, 1, 1, 1, 0, 1],
            [0, 1, 1, 1, 1, 1],
            [1, 1, 1, 1, 1, 1],
        ];
        for powers in alpha {
            let rust = std::array::from_fn::<_, 6, _>(|target| {
                powers[alpha_source_for_rust_target[target]]
            });
            let key = IntegralKey::try_new(rust).unwrap();
            let canonical = authority
                .canonicalizer()
                .unwrap()
                .canonicalize(&key)
                .unwrap()
                .canonical()
                .clone();
            let classified = authority
                .master_terminals()
                .any(|master| master == &canonical)
                || authority.factorization_rules().iter().any(|rule| {
                    rule.application_domain()
                        .contains(canonical.powers())
                        .unwrap()
                });
            assert!(classified, "unclassified FORM terminal alias {rust:?}");
        }
    }

    #[test]
    fn k6_factorization_manifest_remains_exactly_the_compiled_image() {
        let mut extra = k6_candidate().unwrap();
        extra
            .parent_terminals
            .insert(IntegralKey::try_new([1; 6]).unwrap());
        assert_eq!(
            install_terminal_authority(extra).unwrap_err(),
            ArtifactError::InvalidMasterManifest
        );

        let mut missing = k6_candidate().unwrap();
        assert!(
            missing
                .parent_terminals
                .remove(&IntegralKey::try_new([0, 0, 1, 0, 1, 1]).unwrap())
        );
        assert!(matches!(
            install_terminal_authority(missing),
            Err(ArtifactError::InvalidFactorization { .. })
        ));
    }

    #[test]
    fn k6_declared_manifest_is_the_three_nonfactorizing_full_rank_corners() {
        let authority = install_terminal_authority(k6_candidate().unwrap()).unwrap();
        assert_eq!(
            authority
                .declared_master_manifest()
                .terminals()
                .iter()
                .map(|terminal| terminal.powers())
                .collect::<Vec<_>>(),
            [
                &[0, 1, 1, 1, 1, 0][..],
                &[0, 1, 1, 1, 1, 1][..],
                &[1, 1, 1, 1, 1, 1][..],
            ]
        );
        assert_eq!(authority.parent_terminals().len(), 3);
        assert_eq!(authority.master_terminal_count(), 6);
    }

    #[test]
    fn declared_manifest_rejects_bad_bindings_and_orbit_duplicates() {
        let mut wrong_arity = k6_candidate().unwrap();
        wrong_arity
            .declared_master_terminals
            .push(IntegralKey::try_new([1]).unwrap());
        assert_eq!(
            install_terminal_authority(wrong_arity).unwrap_err(),
            ArtifactError::WrongArity {
                expected: 6,
                actual: 1,
            }
        );

        let mut zero = k6_candidate().unwrap();
        zero.declared_master_terminals
            .push(IntegralKey::try_new([0; 6]).unwrap());
        assert_eq!(
            install_terminal_authority(zero).unwrap_err(),
            ArtifactError::InvalidDeclaredMasterManifest {
                detail: "a declared master belongs to an authenticated zero sector",
            }
        );

        let mut factorization_duplicate = k6_candidate().unwrap();
        factorization_duplicate
            .declared_master_terminals
            .push(IntegralKey::try_new([0, 0, 1, 0, 1, 1]).unwrap());
        assert_eq!(
            install_terminal_authority(factorization_duplicate).unwrap_err(),
            ArtifactError::InvalidDeclaredMasterManifest {
                detail: "a declared master duplicates a factorization-image terminal",
            }
        );

        let mut orbit_duplicate = k6_candidate().unwrap();
        let representative = orbit_duplicate.declared_master_terminals[0].clone();
        let image = orbit_duplicate
            .canonicalizer
            .as_ref()
            .unwrap()
            .orbit(&representative)
            .unwrap()
            .images()
            .iter()
            .find(|image| image.integral() != &representative)
            .unwrap()
            .integral()
            .clone();
        orbit_duplicate.declared_master_terminals.push(image);
        assert_eq!(
            install_terminal_authority(orbit_duplicate).unwrap_err(),
            ArtifactError::InvalidDeclaredMasterManifest {
                detail: "declared masters contain duplicate symmetry-orbit representatives",
            }
        );
    }

    #[test]
    fn declared_manifest_installation_is_representative_and_order_deterministic() {
        let mut first = k6_candidate().unwrap();
        first.declared_master_terminals.reverse();

        let mut second = k6_candidate().unwrap();
        let canonicalizer = second.canonicalizer.as_ref().unwrap();
        for terminal in &mut second.declared_master_terminals {
            let image = canonicalizer
                .orbit(terminal)
                .unwrap()
                .images()
                .last()
                .unwrap()
                .integral()
                .clone();
            *terminal = image;
        }

        let first = install_terminal_authority(first).unwrap();
        let second = install_terminal_authority(second).unwrap();
        assert_eq!(
            first.declared_master_manifest(),
            second.declared_master_manifest()
        );
    }
}
