//! Test-only, exactly compiled factorization support for the K=6 pressure family.
//!
//! This owner is deliberately an unsealed fixture: it reuses the production
//! installer's generic factorization proofs, but it makes no claim that the
//! surrounding three-loop rule partition is closed.

use std::collections::BTreeSet;

use crate::algebra::IndexedCoefficientContext;
use crate::family::{IntegralFamily, IntegralKey};
use crate::foundry::artifact::ArtifactError;
use crate::foundry::artifact::factorization::{
    FactorizationFactor, FactorizationRule, UnimodularLoopBasis,
};
use crate::foundry::artifact::install::{ClosingArtifactCandidate, validate_factorization_fixture};
use crate::foundry::artifact::model::{
    ArtifactSchemaVersion, ArtifactValidationWitness, ClosedArtifact, CommonMassHomogeneityProof,
};
use crate::foundry::artifact::one_loop::derive_one_loop_unit_mass_tadpole;
use crate::foundry::artifact::two_loop::derive_two_loop_unit_mass_sunset;
use crate::identity::{ParametricIbpConfig, ParametricIbpGenerator};
use crate::sector::symmetry::Canonicalizer;
use crate::sector::{InteriorBounds, Mask, SectorInteriorDomain};

use super::{canonical_family, canonical_s4};

const ALGORITHM_ID: &str = "rustred.test.three-loop-factorization-fixture.v1";

/// Exactly compiled lower-family product domains for the incomplete K=6
/// pressure fixture.
///
/// Construction runs the production generic-binding and factorization
/// compiler. It therefore owns proof-backed domains and master embeddings,
/// but it is not a [`ClosedArtifact`] and is never publishable.
pub(crate) struct K6FactorizationSupport {
    candidate: ClosingArtifactCandidate,
}

impl K6FactorizationSupport {
    pub(crate) fn try_new() -> Result<Self, ArtifactError> {
        let family = canonical_family()?;
        let canonicalizer = canonical_s4(&family)?;
        let generator =
            ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
        let context = generator.context().clone();
        drop(generator);

        let dependencies = vec![
            Box::new(derive_two_loop_unit_mass_sunset()?),
            Box::new(derive_one_loop_unit_mass_tadpole()?),
        ];
        let factorization_rules = factorization_rules(&family)?;
        let masters = BTreeSet::from([
            IntegralKey::try_new([0, 0, 1, 0, 1, 1])?,
            IntegralKey::try_new([0, 0, 1, 1, 0, 1])?,
            IntegralKey::try_new([0, 0, 1, 1, 1, 1])?,
        ]);
        let mut candidate = ClosingArtifactCandidate {
            schema: ArtifactSchemaVersion::CURRENT,
            algorithm_id: ALGORITHM_ID,
            arity: 6,
            supported_root_power_bounds: vec![InteriorBounds::new(i64::MIN, i64::MAX); 6]
                .into_boxed_slice(),
            family,
            context,
            source_relations: Vec::new(),
            rules: Vec::new(),
            rule_cells: Vec::new(),
            canonicalizer: Some(canonicalizer),
            dependencies,
            factorization_rules,
            masters,
            zero_sectors: Vec::new(),
            common_mass_homogeneity: Some(CommonMassHomogeneityProof::UniformVacuumMassSquared),
        };
        validate_factorization_fixture(&mut candidate)?;
        Ok(Self { candidate })
    }

    pub(crate) fn family(&self) -> &IntegralFamily {
        &self.candidate.family
    }

    pub(crate) fn context(&self) -> &IndexedCoefficientContext {
        &self.candidate.context
    }

    pub(crate) fn canonicalizer(&self) -> &Canonicalizer {
        self.candidate
            .canonicalizer
            .as_ref()
            .expect("the compiled K=6 fixture always owns exact S4")
    }

    pub(crate) fn factorization_rules(&self) -> &[FactorizationRule] {
        &self.candidate.factorization_rules
    }

    /// Retain the historical generic reducer harness without confusing it for
    /// a registered or publishable K=6 closure artifact.
    pub(crate) fn into_synthetic_reducer_artifact(self) -> ClosedArtifact {
        let candidate = self.candidate;
        let family_fingerprint = candidate.family.fingerprint_owner();
        ClosedArtifact {
            schema: candidate.schema,
            algorithm_id: candidate.algorithm_id,
            arity: candidate.arity,
            supported_root_power_bounds: candidate.supported_root_power_bounds,
            family: candidate.family,
            family_fingerprint,
            context: candidate.context,
            source_relations: candidate.source_relations,
            rules: candidate.rules,
            rule_cells: candidate.rule_cells,
            canonicalizer: candidate.canonicalizer,
            dependencies: candidate.dependencies,
            factorization_rules: candidate.factorization_rules,
            masters: candidate.masters,
            zero_sectors: candidate.zero_sectors,
            common_mass_homogeneity: candidate.common_mass_homogeneity,
            // This internal harness isolates the generic compiler and reducer.
            // It bypasses the absent K=6 closure verifier and carries only an
            // honest local factorization census, never publication status.
            validation: ArtifactValidationWitness::new(0, 0, 0, 0, 0, 3, 0),
        }
    }
}

fn factorization_rules(family: &IntegralFamily) -> Result<Vec<FactorizationRule>, ArtifactError> {
    let k3_times_k1 = FactorizationRule::new(
        factorization_domain([0, 0, 1, 1, 1, 1])?,
        [
            // q0=k3-k1, q1=k1-k2: the K3 dependency denominators are
            // parent D4,D5,D6 in zero-based slots 3,4,5.
            FactorizationFactor::new(0, [3, 4, 5], [0, 1]),
            // q2=k3 owns parent D2.
            FactorizationFactor::new(1, [2], [2]),
        ],
        family.coefficient_context().one(),
        UnimodularLoopBasis::new(3, [-1, 0, 1, 1, -1, 0, 0, 0, 1]),
    );

    let star_k1_cubed = FactorizationRule::new(
        factorization_domain([0, 0, 1, 1, 0, 1])?,
        [
            // q0=k3 owns parent D3.
            FactorizationFactor::new(1, [2], [0]),
            // q1=k3-k1 owns parent D4.
            FactorizationFactor::new(1, [3], [1]),
            // q2=k2-k3 owns parent D6.
            FactorizationFactor::new(1, [5], [2]),
        ],
        family.coefficient_context().one(),
        UnimodularLoopBasis::new(3, [0, 0, 1, -1, 0, 1, 0, 1, -1]),
    );

    let path_k1_cubed = FactorizationRule::new(
        factorization_domain([0, 0, 1, 0, 1, 1])?,
        [
            // q0=k3 owns parent D3.
            FactorizationFactor::new(1, [2], [0]),
            // q1=k1-k2 owns parent D5.
            FactorizationFactor::new(1, [4], [1]),
            // q2=k2-k3 owns parent D6.
            FactorizationFactor::new(1, [5], [2]),
        ],
        family.coefficient_context().one(),
        UnimodularLoopBasis::new(3, [0, 0, 1, 1, -1, 0, 0, 1, -1]),
    );

    Ok(vec![k3_times_k1, star_k1_cubed, path_k1_cubed])
}

fn factorization_domain(sector: [i64; 6]) -> Result<SectorInteriorDomain, ArtifactError> {
    Ok(SectorInteriorDomain::try_new(
        Mask::try_from_indices(&sector)?,
        sector.map(|power| {
            if power >= 1 {
                InteriorBounds::new(1, i64::MAX)
            } else {
                InteriorBounds::new(0, 0)
            }
        }),
    )?)
}
