use std::collections::BTreeSet;

use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::{Mask, symmetry::Canonicalizer};

use super::super::manifest::ZERO_ORBITS;

pub(super) fn complete_ordinary_sources(
    generator: &ParametricIbpGenerator<'_>,
) -> Result<(CompletedIbpSourceRows, usize), ArtifactError> {
    let prepared = generator.prepare_ordinary_ibp()?;
    let source_count = prepared.len();
    let rows = (0..source_count)
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    Ok((prepared.complete(rows)?, source_count))
}

pub(super) fn exact_zero_sectors(
    canonicalizer: &Canonicalizer,
) -> Result<Vec<Mask>, ArtifactError> {
    let zero_representatives = ZERO_ORBITS
        .iter()
        .map(|orbit| orbit.representative)
        .collect::<BTreeSet<_>>();
    (0_u64..64)
        .map(|bits| {
            let powers: [i64; 6] = std::array::from_fn(|slot| i64::from(((bits >> slot) & 1) != 0));
            let key = IntegralKey::try_new(powers)?;
            let canonical = canonicalizer.canonicalize(&key)?;
            Ok::<_, ArtifactError>(
                zero_representatives
                    .contains(canonical.canonical().powers())
                    .then(|| Mask::try_from_indices(key.powers()))
                    .transpose()?,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|sectors| sectors.into_iter().flatten().collect())
}
