use std::sync::Arc;

use symbolica::domains::{Ring, RingOps};

use crate::foundry::completion::frame::modular::{ModularPhysicalFrame, ModularRightObstruction};
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator, SelectedTranslatedSourceBatch,
};

use super::model::IncidentNominationOrigin;
use super::nominate::{check_limit, checked_add, try_vec};
use super::{
    IncidentTranslationNominations, NonzeroIncidentTranslationResiduals,
    OrdinarySourceIncidenceIndex, SourceDiscoveryError, SourceDiscoveryLimits,
};

const RESIDUAL_CANDIDATES: &str = "source-discovery residual candidates";
const RESIDUAL_SOURCE_TERMS: &str = "source-discovery residual exact-source terms";
const RESIDUAL_SUPPORT_COORDINATES: &str =
    "source-discovery residual obstruction-support coordinate cells";
const RESIDUAL_CLASSIFICATIONS: &str = "source-discovery residual candidate classifications";
const NONZERO_RESIDUAL_REQUESTS: &str = "source-discovery nonzero residual requests";

struct RawObstructionEntry<'entry> {
    shift: &'entry IntegralShift,
    coefficient: &'entry symbolica::domains::finite_field::FiniteFieldElement<u64>,
}

impl OrdinarySourceIncidenceIndex<'_> {
    /// Evaluate every nominated exact translated row against one checked
    /// target-normalized modular right obstruction.
    ///
    /// Translation goes through the selected-source identity boundary.  The
    /// admitted frame then evaluates every source condition and every term,
    /// including modular zeros and terms outside the obstruction support,
    /// before this layer performs any sparse projection.  Only requests with
    /// a nonzero complete-row residual are retained, in canonical request
    /// order.  The result is discovery telemetry and has no rule authority.
    pub(crate) fn try_retain_nonzero_residuals(
        &self,
        generator: &ParametricIbpGenerator<'_>,
        completed: &CompletedIbpSourceRows,
        nominations: &IncidentTranslationNominations,
        frame: &ModularPhysicalFrame<'_>,
        obstruction: &ModularRightObstruction<'_>,
        limits: SourceDiscoveryLimits,
    ) -> Result<NonzeroIncidentTranslationResiduals, SourceDiscoveryError> {
        validate_join(self, generator, completed, nominations, frame, obstruction)?;
        let support = raw_obstruction_support(self, frame, obstruction, limits)?;

        let candidate_count = nominations.requests().len();
        check_limit(
            RESIDUAL_CANDIDATES,
            candidate_count,
            limits.max_residual_candidates,
        )?;
        check_limit(
            RESIDUAL_CLASSIFICATIONS,
            candidate_count,
            limits.max_residual_classifications,
        )?;
        let evaluated_source_terms = preflight_candidate_terms(self, nominations, limits)?;

        if candidate_count == 0 {
            return Ok(NonzeroIncidentTranslationResiduals::from_parts(
                Vec::new(),
                0,
                0,
                0,
                support.len(),
            ));
        }

        let translated = generator
            .translate_selected_completed_source_rows(
                completed,
                nominations.requests().iter().cloned(),
                limits.translation,
            )
            .map_err(SourceDiscoveryError::SourceTranslation)?;
        pair_translated_sources(
            self,
            generator,
            nominations,
            frame,
            &support,
            translated,
            candidate_count,
            evaluated_source_terms,
            limits,
        )
    }
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn pair_selected_sources_for_test(
    incidence: &OrdinarySourceIncidenceIndex<'_>,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    nominations: &IncidentTranslationNominations,
    frame: &ModularPhysicalFrame<'_>,
    obstruction: &ModularRightObstruction<'_>,
    selected: SelectedTranslatedSourceBatch,
    limits: SourceDiscoveryLimits,
) -> Result<NonzeroIncidentTranslationResiduals, SourceDiscoveryError> {
    validate_join(
        incidence,
        generator,
        completed,
        nominations,
        frame,
        obstruction,
    )?;
    let support = raw_obstruction_support(incidence, frame, obstruction, limits)?;
    let candidate_count = nominations.requests().len();
    check_limit(
        RESIDUAL_CANDIDATES,
        candidate_count,
        limits.max_residual_candidates,
    )?;
    check_limit(
        RESIDUAL_CLASSIFICATIONS,
        candidate_count,
        limits.max_residual_classifications,
    )?;
    let evaluated_source_terms = preflight_candidate_terms(incidence, nominations, limits)?;
    pair_translated_sources(
        incidence,
        generator,
        nominations,
        frame,
        &support,
        selected,
        candidate_count,
        evaluated_source_terms,
        limits,
    )
}

#[allow(clippy::too_many_arguments)]
fn pair_translated_sources(
    incidence: &OrdinarySourceIncidenceIndex<'_>,
    generator: &ParametricIbpGenerator<'_>,
    nominations: &IncidentTranslationNominations,
    frame: &ModularPhysicalFrame<'_>,
    support: &[RawObstructionEntry<'_>],
    translated: SelectedTranslatedSourceBatch,
    candidate_count: usize,
    evaluated_source_terms: usize,
    limits: SourceDiscoveryLimits,
) -> Result<NonzeroIncidentTranslationResiduals, SourceDiscoveryError> {
    if !translated.is_complete_ordinary() {
        return Err(SourceDiscoveryError::WrongSourceLayout {
            actual: translated.source_layout_name(),
        });
    }
    if translated.family_fingerprint() != incidence.family_fingerprint()
        || translated.family_fingerprint() != frame.plan().family_fingerprint()
        || translated.context_fingerprint() != incidence.context_fingerprint()
        || translated.context_fingerprint() != frame.plan().context_fingerprint()
    {
        return Err(SourceDiscoveryError::ScopeMismatch {
            detail: "selected residual translations do not belong to the indexed frame scope",
        });
    }
    if translated.completed_source_row_count() != incidence.source_count()
        || translated.requests() != nominations.requests()
        || translated.sources().len() != candidate_count
    {
        return Err(SourceDiscoveryError::Invariant {
            detail: "selected residual translation changed the nominated source chronology",
        });
    }
    for (candidate_ordinal, (request, source)) in translated
        .requests()
        .iter()
        .zip(translated.sources())
        .enumerate()
    {
        if source.provenance().source_ordinal() != request.source_ordinal()
            || source.provenance().offset() != request.offset()
        {
            return Err(SourceDiscoveryError::SelectedRequestProvenanceMismatch {
                candidate_ordinal,
            });
        }
        let expected = incidence.sources().get(request.source_ordinal()).ok_or(
            SourceDiscoveryError::ScopeMismatch {
                detail: "selected residual source is outside the declared ordinary module",
            },
        )?;
        if source.row_id() != expected.row_id() {
            return Err(SourceDiscoveryError::SelectedSourceRowMismatch {
                candidate_ordinal,
                source_ordinal: request.source_ordinal(),
            });
        }
    }

    let mut classifications = try_vec(RESIDUAL_CLASSIFICATIONS, candidate_count)?;
    let mut evaluated = Vec::new();
    let mut paired_source_terms = 0usize;
    let field = frame.field();

    for (candidate_ordinal, (request, source)) in translated
        .requests()
        .iter()
        .zip(translated.sources())
        .enumerate()
    {
        frame
            .try_evaluate_translated_source(generator.context(), source, &mut evaluated)
            .map_err(|error| SourceDiscoveryError::CandidateEvaluation {
                candidate_ordinal,
                source_ordinal: request.source_ordinal(),
                error,
            })?;
        if evaluated.len() != source.terms().len() {
            return Err(SourceDiscoveryError::Invariant {
                detail: "complete modular source evaluation changed exact term cardinality",
            });
        }

        let mut residual = field.zero();
        for (term_shift, coefficient) in source.terms().keys().zip(&evaluated) {
            let Ok(position) =
                support.binary_search_by(|entry| entry.shift.values().cmp(term_shift.values()))
            else {
                continue;
            };
            paired_source_terms = checked_add(RESIDUAL_SOURCE_TERMS, paired_source_terms, 1)?;
            residual = field.add(
                &residual,
                &field.mul(coefficient, support[position].coefficient),
            );
        }
        classifications.push(!field.is_zero(&residual));
    }

    if classifications.len() != candidate_count {
        return Err(SourceDiscoveryError::Invariant {
            detail: "residual pairing changed its preflighted candidate count",
        });
    }
    let nonzero_count = classifications.iter().filter(|&&keep| keep).count();
    check_limit(
        NONZERO_RESIDUAL_REQUESTS,
        nonzero_count,
        limits.max_nonzero_residual_requests,
    )?;
    let mut retained = try_vec(NONZERO_RESIDUAL_REQUESTS, nonzero_count)?;
    for (request, &keep) in nominations.requests().iter().zip(&classifications) {
        if keep {
            retained.push(request.clone());
        }
    }
    if retained.len() != nonzero_count || retained.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(SourceDiscoveryError::Invariant {
            detail: "nonzero residual requests are not canonical and unique",
        });
    }

    Ok(NonzeroIncidentTranslationResiduals::from_parts(
        retained,
        candidate_count,
        evaluated_source_terms,
        paired_source_terms,
        support.len(),
    ))
}

fn validate_join(
    incidence: &OrdinarySourceIncidenceIndex<'_>,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    nominations: &IncidentTranslationNominations,
    frame: &ModularPhysicalFrame<'_>,
    obstruction: &ModularRightObstruction<'_>,
) -> Result<(), SourceDiscoveryError> {
    if !std::ptr::eq(frame.plan(), obstruction.plan()) {
        return Err(SourceDiscoveryError::ObstructionPlanMismatch);
    }
    if !Arc::ptr_eq(frame.sample_fingerprint(), obstruction.sample_fingerprint()) {
        return Err(SourceDiscoveryError::ObstructionSampleMismatch);
    }
    if !incidence.owns_identity(nominations.incidence_identity()) {
        return Err(SourceDiscoveryError::NominationIncidenceMismatch);
    }
    match nominations.origin() {
        IncidentNominationOrigin::TargetUnit => {
            return Err(SourceDiscoveryError::TargetUnitNominationForObstruction);
        }
        IncidentNominationOrigin::CheckedObstruction(identity) => {
            if !identity.belongs_to(obstruction) {
                return Err(SourceDiscoveryError::NominationObstructionMismatch);
            }
        }
    }
    if frame.plan().family_fingerprint() != incidence.family_fingerprint()
        || frame.plan().context_fingerprint() != incidence.context_fingerprint()
    {
        return Err(SourceDiscoveryError::ScopeMismatch {
            detail: "residual-pairing frame differs from the declared ordinary source module",
        });
    }
    if generator.context().fingerprint() != incidence.context_fingerprint() {
        return Err(SourceDiscoveryError::ScopeMismatch {
            detail: "residual-pairing generator differs from the declared ordinary source module",
        });
    }
    if !completed.is_complete_ordinary() {
        return Err(SourceDiscoveryError::WrongSourceLayout {
            actual: completed.layout_name(),
        });
    }
    generator
        .validate_completed_scope(completed)
        .map_err(SourceDiscoveryError::SourceTranslation)?;
    if completed.source_row_count() != incidence.source_count()
        || (0..completed.source_row_count()).any(|source_ordinal| {
            completed.source_row_id(source_ordinal)
                != incidence
                    .sources()
                    .get(source_ordinal)
                    .map(|source| source.row_id())
        })
    {
        return Err(SourceDiscoveryError::CompletedSourceChronologyMismatch);
    }
    let unique_after_exclusion = nominations
        .unique_before_existing_exclusion()
        .checked_sub(nominations.excluded_existing_requests())
        .ok_or(SourceDiscoveryError::Invariant {
            detail: "nomination exclusion telemetry underflowed",
        })?;
    if unique_after_exclusion != nominations.requests().len()
        || nominations
            .requests()
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(SourceDiscoveryError::Invariant {
            detail: "residual nominations are not canonical, unique, and telemetry-complete",
        });
    }
    Ok(())
}

fn preflight_candidate_terms(
    incidence: &OrdinarySourceIncidenceIndex<'_>,
    nominations: &IncidentTranslationNominations,
    limits: SourceDiscoveryLimits,
) -> Result<usize, SourceDiscoveryError> {
    let mut terms = 0usize;
    for request in nominations.requests() {
        if request.offset().len() != incidence.arity() {
            return Err(SourceDiscoveryError::WrongArity {
                object: "residual candidate offset",
                expected: incidence.arity(),
                actual: request.offset().len(),
            });
        }
        let source = incidence.sources().get(request.source_ordinal()).ok_or(
            SourceDiscoveryError::ScopeMismatch {
                detail: "residual candidate source is outside the declared ordinary module",
            },
        )?;
        terms = checked_add(RESIDUAL_SOURCE_TERMS, terms, source.terms().len())?;
        check_limit(
            RESIDUAL_SOURCE_TERMS,
            terms,
            limits.max_residual_source_terms,
        )?;
    }
    Ok(terms)
}

fn raw_obstruction_support<'entry>(
    incidence: &OrdinarySourceIncidenceIndex<'_>,
    frame: &'entry ModularPhysicalFrame<'_>,
    obstruction: &'entry ModularRightObstruction<'_>,
    limits: SourceDiscoveryLimits,
) -> Result<Vec<RawObstructionEntry<'entry>>, SourceDiscoveryError> {
    check_limit(
        "source-discovery obstruction support entries",
        obstruction.entries().len(),
        limits.max_obstruction_support,
    )?;
    let coordinate_cells = obstruction
        .entries()
        .len()
        .checked_mul(incidence.arity())
        .ok_or(SourceDiscoveryError::ResourceCountOverflow {
            resource: RESIDUAL_SUPPORT_COORDINATES,
        })?;
    check_limit(
        RESIDUAL_SUPPORT_COORDINATES,
        coordinate_cells,
        limits.max_residual_support_coordinate_cells,
    )?;

    let mut support = try_vec(
        "source-discovery obstruction support entries",
        obstruction.entries().len(),
    )?;
    for entry in obstruction.entries() {
        let physical = *obstruction
            .logical_physical_columns()
            .get(entry.logical_column())
            .ok_or(SourceDiscoveryError::Invariant {
                detail: "residual obstruction entry is outside its logical column map",
            })?;
        let shift =
            frame
                .plan()
                .columns()
                .get(physical)
                .ok_or(SourceDiscoveryError::Invariant {
                    detail: "residual obstruction support is outside its physical frame",
                })?;
        if shift.len() != incidence.arity() {
            return Err(SourceDiscoveryError::WrongArity {
                object: "residual obstruction support",
                expected: incidence.arity(),
                actual: shift.len(),
            });
        }
        support.push(RawObstructionEntry {
            shift,
            coefficient: entry.coefficient(),
        });
    }
    support.sort_unstable_by(|left, right| left.shift.values().cmp(right.shift.values()));
    if support.is_empty()
        || support
            .windows(2)
            .any(|pair| pair[0].shift.values() >= pair[1].shift.values())
    {
        return Err(SourceDiscoveryError::Invariant {
            detail: "residual obstruction raw support is empty or nonunique",
        });
    }
    Ok(support)
}
