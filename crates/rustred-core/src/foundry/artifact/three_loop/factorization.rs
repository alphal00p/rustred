//! Exact factorization recipes for the test-only K=6 pressure family.

use crate::family::IntegralFamily;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::artifact::factorization::{
    FactorizationFactor, FactorizationRule, UnimodularLoopBasis,
};
use crate::sector::{InteriorBounds, Mask, SectorInteriorDomain};

pub(super) fn factorization_rules(
    family: &IntegralFamily,
) -> Result<Vec<FactorizationRule>, ArtifactError> {
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
