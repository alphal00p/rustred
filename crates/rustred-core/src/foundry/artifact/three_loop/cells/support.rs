use crate::foundry::artifact::ArtifactError;
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};

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
