//! Independent canonical-source authentication for published K6 rule cells.

use std::sync::Arc;

use crate::foundry::cell::{RuleCell, SourceViewConstruction};
use crate::identity::{
    CompletedIbpSourceRows, ParametricIbpConfig, ParametricIbpGenerator, ParametricRelation,
    TranslatedSource, TranslatedSourceError, TranslatedSourceLimits, TranslatedSourceRequest,
};

use super::super::super::error::ArtifactError;
use super::super::ClosingArtifactCandidate;
use super::validate_candidate_shell;

/// Rebuild the complete ordinary source module independently of the campaign,
/// then join every retained translated source view back to that immutable
/// module. This is the cold installation trust boundary: cell-local replay
/// alone is intentionally insufficient because a forged source view and rule
/// could otherwise remain mutually self-consistent.
pub(crate) fn authenticate_canonical_source_views(
    candidate: &ClosingArtifactCandidate,
) -> Result<(), ArtifactError> {
    validate_candidate_shell(candidate)?;
    let generator = ParametricIbpGenerator::try_new_with_config(
        &candidate.family,
        ParametricIbpConfig::default(),
    )?;
    let prepared = generator.prepare_ordinary_ibp()?;
    if prepared.len() != 9 {
        return Err(ArtifactError::InvalidReplayEvidence {
            detail: "the K6 family did not independently regenerate nine ordinary source rows",
        });
    }
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    let completed = prepared.complete(rows)?;
    if !completed.is_complete_ordinary() || completed.source_row_count() != 9 {
        return Err(ArtifactError::InvalidReplayEvidence {
            detail: "the independently regenerated K6 source barrier is not complete ordinary",
        });
    }
    authenticate_canonical_source_manifest(candidate, &completed)?;
    authenticate_rule_cell_source_views(&generator, &completed, &candidate.rule_cells)
}

fn authenticate_canonical_source_manifest(
    candidate: &ClosingArtifactCandidate,
    completed: &CompletedIbpSourceRows,
) -> Result<(), ArtifactError> {
    if completed.family_fingerprint() != candidate.family.fingerprint()
        || completed.context_fingerprint() != candidate.context.fingerprint()
        || completed.source_row_count() != candidate.source_relations.len()
    {
        return Err(ArtifactError::InvalidReplayEvidence {
            detail: "the K6 ordinary source manifest differs from its regenerated authority",
        });
    }
    for (ordinal, actual) in candidate.source_relations.iter().enumerate() {
        let Some(expected) = completed.source_relation(ordinal) else {
            return Err(ArtifactError::InvalidReplayEvidence {
                detail: "the K6 ordinary source manifest has a foreign source ordinal",
            });
        };
        if actual != expected {
            return Err(ArtifactError::InvalidReplayEvidence {
                detail: "a K6 ordinary source row differs from its independently regenerated row",
            });
        }
    }
    Ok(())
}

/// Authenticate all cell source translations in one deduplicated Symbolica
/// translation plan. The selected-source primitive owns arity, source-row,
/// aggregate term/condition, and allocation bounds; this layer only performs
/// the exact canonical join and construction-specific selection.
pub(crate) fn authenticate_rule_cell_source_views(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    cells: &[Arc<RuleCell>],
) -> Result<(), ArtifactError> {
    let limits = TranslatedSourceLimits::default();
    let requested =
        cells.iter().try_fold(0usize, |count, cell| {
            count.checked_add(cell.sources().provenance().len()).ok_or(
                ArtifactError::TranslatedSource(TranslatedSourceError::ResourceCountOverflow {
                    resource: "K6 canonical source-view joins",
                }),
            )
        })?;
    if requested == 0 {
        return Err(ArtifactError::InvalidReplayEvidence {
            detail: "the published K6 rule cells retain no translated source views",
        });
    }
    if requested > limits.max_requested_source_translations {
        return Err(ArtifactError::TranslatedSource(
            TranslatedSourceError::ResourceLimit {
                resource: "K6 canonical source-view joins",
                requested,
                limit: limits.max_requested_source_translations,
            },
        ));
    }

    let mut requests = Vec::new();
    requests.try_reserve_exact(requested).map_err(|_| {
        ArtifactError::TranslatedSource(TranslatedSourceError::AllocationFailure {
            resource: "K6 canonical source-view joins",
            requested,
        })
    })?;
    for cell in cells {
        if cell.sources().family_fingerprint() != completed.family_fingerprint()
            || cell.sources().context_fingerprint() != completed.context_fingerprint()
            || cell.sources().len() != cell.sources().provenance().len()
        {
            return Err(ArtifactError::InvalidReplayEvidence {
                detail: "a K6 rule cell has foreign or incomplete source-view ownership",
            });
        }
        for provenance in cell.sources().provenance() {
            // No production constructor currently defines an algebraic
            // symmetry transform for a source view. Residual routing has its
            // own exact group-action witness; an isolated provenance tag must
            // fail closed rather than be treated as an authenticated action.
            if provenance.symmetry().is_some() {
                return Err(ArtifactError::InvalidReplayEvidence {
                    detail: "a K6 source view names an unregistered symmetry transformation",
                });
            }
            let translated = provenance.translated();
            requests.push(TranslatedSourceRequest::new(
                translated.source_ordinal(),
                translated.offset().clone(),
            ));
        }
    }

    let expected =
        generator.translate_selected_completed_source_rows(completed, requests, limits)?;
    if !expected.is_complete_ordinary()
        || expected.completed_source_row_count() != completed.source_row_count()
        || expected.family_fingerprint() != completed.family_fingerprint()
        || expected.context_fingerprint() != completed.context_fingerprint()
    {
        return Err(ArtifactError::InvalidReplayEvidence {
            detail: "the canonical K6 translated-source plan lost its ordinary-source authority",
        });
    }

    for cell in cells {
        let originals = match cell.sources().construction() {
            SourceViewConstruction::ResidualProjection(evidence) => {
                if evidence.original_relations().len() != cell.sources().len() {
                    return Err(ArtifactError::InvalidReplayEvidence {
                        detail: "a K6 residual source view has no one-to-one original relation span",
                    });
                }
                Some(evidence.original_relations())
            }
            SourceViewConstruction::Direct => None,
            // Exact lowering deliberately retains the complete translated
            // relations unchanged in this construction. The fixed-index
            // quotient is applied only while replaying the rule and guards,
            // after this canonical-source join. Direct equality here proves
            // that specialization did not rewrite or discard source data.
            SourceViewConstruction::FixedIndexSpecialization(_) => None,
        };
        for (ordinal, provenance) in cell.sources().provenance().iter().enumerate() {
            let translated = provenance.translated();
            let request = TranslatedSourceRequest::new(
                translated.source_ordinal(),
                translated.offset().clone(),
            );
            let expected_ordinal = expected.requests().binary_search(&request).map_err(|_| {
                ArtifactError::InvalidReplayEvidence {
                    detail: "a K6 source view is absent from its canonical translation plan",
                }
            })?;
            let expected_source = &expected.sources()[expected_ordinal];
            let actual = originals.map_or_else(
                || &cell.sources().relations()[ordinal],
                |relations| &relations[ordinal],
            );
            if expected_source.provenance() != translated
                || !translated_relation_matches(
                    expected_source,
                    actual,
                    generator,
                    completed.family_fingerprint(),
                )
            {
                return Err(ArtifactError::InvalidReplayEvidence {
                    detail: "a K6 source view differs from its canonical translated source row",
                });
            }
        }
    }
    Ok(())
}

fn translated_relation_matches(
    expected: &TranslatedSource,
    actual: &ParametricRelation,
    generator: &ParametricIbpGenerator<'_>,
    expected_family: &str,
) -> bool {
    actual.validate_context(generator.context()).is_ok()
        && actual.family_fingerprint_owner().as_str() == expected_family
        && actual.row_id() == expected.row_id()
        && actual.terms() == expected.terms()
        && actual.nonzero_conditions() == expected.nonzero_conditions()
}
