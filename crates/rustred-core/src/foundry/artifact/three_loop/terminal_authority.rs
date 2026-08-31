//! Production-sealed terminal authority for the test-only K=6 pressure family.
//!
//! This fixture deliberately proves no ordinary-rule closure. It owns only
//! exact zero sectors, compiled lower-family factorizations, and the typed
//! parent-family terminals reached by those factorizations.

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

use super::factorization::factorization_rules;
use super::terminals::exact_zero_sectors;
use super::{canonical_family, canonical_s4};

const AUTHORITY_ID: &str = "rustred.test.three-loop-k6-terminal-authority.v1";
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

fn k6_candidate() -> Result<TerminalAuthorityCandidate, ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
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
    let parent_terminals = BTreeSet::from([
        IntegralKey::try_new([0, 0, 1, 0, 1, 1])?,
        IntegralKey::try_new([0, 0, 1, 1, 0, 1])?,
        IntegralKey::try_new([0, 0, 1, 1, 1, 1])?,
    ]);

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
        zero_sectors,
    })
}

#[cfg(test)]
mod tests {
    use crate::family::IntegralKey;
    use crate::foundry::artifact::ArtifactError;
    use crate::foundry::artifact::install::install_terminal_authority;

    use super::k6_candidate;

    #[test]
    fn k6_terminal_manifest_is_exactly_the_compiled_factorization_image() {
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
}
