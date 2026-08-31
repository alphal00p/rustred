use std::cmp::{Ordering, Reverse};

use symbolica::domains::finite_field::{FiniteFieldElement, Zp64};
use symbolica::domains::{Ring, RingOps};

use crate::foundry::completion::frame::modular::{ModularPhysicalFrame, ModularRightObstruction};
use crate::foundry::completion::stratum::{ProspectiveColumnKind, TargetColumnPartition};
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator, TranslatedSourceRequest};

use super::super::model::IncidentNominationOrigin;
use super::super::nominate::{check_limit, checked_add, try_vec};
use super::super::{
    NonzeroIncidentTranslationResiduals, OrdinarySourceIncidenceIndex, SourceDiscoveryError,
    SourceDiscoveryLimits,
};
use super::{ObstructionBlockNominations, ProbeRowEvaluationCache};

const SIGNATURE_CANDIDATES: &str = "source-discovery obstruction-block signature candidates";
const SIGNATURE_CELLS: &str = "source-discovery obstruction-block signature cells";
const SIGNATURE_PAIRING: &str = "source-discovery obstruction-block signature pairing operations";
const CANDIDATE_CLASSIFICATIONS: &str =
    "source-discovery obstruction-block candidate classifications";
const PRIMARY_CROSSCHECK: &str =
    "source-discovery obstruction-block q0 residual cross-check comparisons";

/// Structural frontier cost used after marginal obstruction-signature rank.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ObstructionBlockProposalScore {
    new_forbidden_columns: usize,
    new_physical_columns: usize,
    union_support_terms: usize,
    source_terms: usize,
}

impl ObstructionBlockProposalScore {
    pub(crate) fn compare_preference(self, other: Self) -> Ordering {
        (
            self.new_forbidden_columns,
            self.new_physical_columns,
            Reverse(self.union_support_terms),
            self.source_terms,
        )
            .cmp(&(
                other.new_forbidden_columns,
                other.new_physical_columns,
                Reverse(other.union_support_terms),
                other.source_terms,
            ))
    }

    pub(super) const fn new(
        new_forbidden_columns: usize,
        new_physical_columns: usize,
        union_support_terms: usize,
        source_terms: usize,
    ) -> Self {
        Self {
            new_forbidden_columns,
            new_physical_columns,
            union_support_terms,
            source_terms,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ObstructionBlockProposalCandidate {
    request: TranslatedSourceRequest,
    signature: Box<[FiniteFieldElement<u64>]>,
    score: ObstructionBlockProposalScore,
}

impl ObstructionBlockProposalCandidate {
    pub(crate) const fn request(&self) -> &TranslatedSourceRequest {
        &self.request
    }

    pub(crate) fn signature(&self) -> &[FiniteFieldElement<u64>] {
        &self.signature
    }

    pub(crate) const fn score(&self) -> ObstructionBlockProposalScore {
        self.score
    }

    pub(super) fn from_parts(
        request: TranslatedSourceRequest,
        signature: Vec<FiniteFieldElement<u64>>,
        score: ObstructionBlockProposalScore,
    ) -> Self {
        Self {
            request,
            signature: signature.into_boxed_slice(),
            score,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ObstructionBlockProposalTelemetry {
    logical_candidates: usize,
    logical_source_terms: usize,
    signature_pairing_operations: usize,
}

impl ObstructionBlockProposalTelemetry {
    pub(crate) const fn logical_candidates(self) -> usize {
        self.logical_candidates
    }

    pub(crate) const fn logical_source_terms(self) -> usize {
        self.logical_source_terms
    }

    pub(crate) const fn signature_pairing_operations(self) -> usize {
        self.signature_pairing_operations
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ObstructionBlockProposalBatch {
    modulus: u64,
    width: usize,
    candidates: Box<[ObstructionBlockProposalCandidate]>,
    telemetry: ObstructionBlockProposalTelemetry,
}

impl ObstructionBlockProposalBatch {
    pub(crate) const fn modulus(&self) -> u64 {
        self.modulus
    }

    pub(crate) const fn width(&self) -> usize {
        self.width
    }

    pub(crate) fn candidates(&self) -> &[ObstructionBlockProposalCandidate] {
        &self.candidates
    }

    pub(crate) const fn telemetry(&self) -> ObstructionBlockProposalTelemetry {
        self.telemetry
    }

    /// Pin the proposal signature's q0 coordinate to the independently
    /// authoritative primary residual census before any selection heuristic
    /// runs. Equality failure is an invariant, never negative evidence.
    pub(crate) fn try_verify_primary_residuals(
        &self,
        residuals: &NonzeroIncidentTranslationResiduals,
        limits: SourceDiscoveryLimits,
    ) -> Result<(), SourceDiscoveryError> {
        self.try_verify_primary_requests(residuals.requests(), limits)
    }

    fn try_verify_primary_requests(
        &self,
        primary_requests: &[TranslatedSourceRequest],
        limits: SourceDiscoveryLimits,
    ) -> Result<(), SourceDiscoveryError> {
        if self.width == 0 {
            return Err(SourceDiscoveryError::Invariant {
                detail: "q0 residual cross-check received an empty block signature",
            });
        }
        let field = Zp64::new(self.modulus);
        let mut q0 = try_vec(PRIMARY_CROSSCHECK, self.candidates.len())?;
        for candidate in &self.candidates {
            if !field.is_zero(&candidate.signature[0]) {
                q0.push(candidate.request.clone());
            }
        }
        let canonical_windows = self.candidates.len().saturating_sub(1);
        let equality_comparisons = if q0.len() == primary_requests.len() {
            q0.len()
        } else {
            0
        };
        let comparisons = checked_add(PRIMARY_CROSSCHECK, canonical_windows, equality_comparisons)?;
        check_limit(
            PRIMARY_CROSSCHECK,
            comparisons,
            limits.max_block_primary_crosscheck_comparisons,
        )?;
        if self
            .candidates
            .windows(2)
            .any(|pair| pair[0].request() >= pair[1].request())
        {
            return Err(SourceDiscoveryError::Invariant {
                detail: "obstruction-block proposal requests are not canonical and unique",
            });
        }
        if q0.as_slice() != primary_requests {
            return Err(SourceDiscoveryError::Invariant {
                detail: "block signature q0 support differs from the authoritative primary residual census",
            });
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn try_verify_primary_requests_for_test(
        &self,
        primary_requests: &[TranslatedSourceRequest],
        limits: SourceDiscoveryLimits,
    ) -> Result<(), SourceDiscoveryError> {
        self.try_verify_primary_requests(primary_requests, limits)
    }

    #[cfg(test)]
    pub(super) fn from_candidates_for_test(
        modulus: u64,
        width: usize,
        candidates: Vec<ObstructionBlockProposalCandidate>,
    ) -> Self {
        Self {
            modulus,
            width,
            telemetry: ObstructionBlockProposalTelemetry {
                logical_candidates: candidates.len(),
                logical_source_terms: 0,
                signature_pairing_operations: 0,
            },
            candidates: candidates.into_boxed_slice(),
        }
    }
}

impl OrdinarySourceIncidenceIndex<'_> {
    /// Evaluate proposal-only union rows into a bounded right-kernel
    /// signature. Every exact row is translated again; only its complete
    /// finite-field coefficient vector may be reused by the probe-local
    /// computation cache.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_evaluate_obstruction_block_proposals(
        &self,
        generator: &ParametricIbpGenerator<'_>,
        completed: &CompletedIbpSourceRows,
        nominations: &ObstructionBlockNominations<'_>,
        frame: &ModularPhysicalFrame<'_>,
        obstruction: &ModularRightObstruction<'_>,
        partition: &TargetColumnPartition<'_>,
        cache: &mut ProbeRowEvaluationCache,
        limits: SourceDiscoveryLimits,
    ) -> Result<ObstructionBlockProposalBatch, SourceDiscoveryError> {
        validate_join(
            self,
            generator,
            completed,
            nominations,
            frame,
            obstruction,
            partition,
        )?;
        let union = nominations.union();
        let width = union.direction_count();
        if width == 0 || width > 4 {
            return Err(SourceDiscoveryError::Invariant {
                detail: "obstruction-block proposal width is outside 1..=4",
            });
        }
        let candidate_count = union.requests().len();
        check_limit(
            SIGNATURE_CANDIDATES,
            candidate_count,
            limits.max_block_signature_candidates,
        )?;
        let signature_cells = checked_mul(SIGNATURE_CELLS, candidate_count, width)?;
        check_limit(
            SIGNATURE_CELLS,
            signature_cells,
            limits.max_block_signature_cells,
        )?;
        let mut logical_source_terms = 0usize;
        for request in union.requests() {
            let source = self.sources().get(request.source_ordinal()).ok_or(
                SourceDiscoveryError::ScopeMismatch {
                    detail: "obstruction-block request is outside the declared ordinary module",
                },
            )?;
            logical_source_terms = checked_add(
                SIGNATURE_PAIRING,
                logical_source_terms,
                source.terms().len(),
            )?;
        }
        let pairing_upper = checked_mul(SIGNATURE_PAIRING, logical_source_terms, width)?;
        check_limit(
            SIGNATURE_PAIRING,
            pairing_upper,
            limits.max_block_signature_pairing_operations,
        )?;
        check_limit(
            CANDIDATE_CLASSIFICATIONS,
            logical_source_terms,
            limits.max_block_candidate_classifications,
        )?;

        let translated = generator
            .translate_selected_completed_source_rows(
                completed,
                union.requests().iter().cloned(),
                limits.translation,
            )
            .map_err(SourceDiscoveryError::SourceTranslation)?;
        if translated.requests() != union.requests()
            || translated.sources().len() != candidate_count
            || !translated.is_complete_ordinary()
        {
            return Err(SourceDiscoveryError::Invariant {
                detail: "obstruction-block translation changed canonical request chronology",
            });
        }

        let field = frame.field();
        let mut candidates = try_vec(SIGNATURE_CANDIDATES, candidate_count)?;
        let mut actual_pairing = 0usize;
        for (candidate_ordinal, (request, source)) in translated
            .requests()
            .iter()
            .zip(translated.sources())
            .enumerate()
        {
            let evaluated = cache.try_evaluate(
                self,
                generator.context(),
                request,
                source,
                frame,
                candidate_ordinal,
                limits,
            )?;
            if evaluated.len() != source.terms().len() {
                return Err(SourceDiscoveryError::Invariant {
                    detail: "cached block proposal row changed exact term cardinality",
                });
            }
            let mut signature = try_vec(SIGNATURE_CELLS, width)?;
            signature.resize_with(width, || field.zero());
            let mut union_support_terms = 0usize;
            for (shift, coefficient) in source.terms().keys().zip(evaluated.iter()) {
                let Ok(position) = union
                    .support()
                    .binary_search_by(|entry| entry.shift().values().cmp(shift.values()))
                else {
                    continue;
                };
                union_support_terms = checked_add(SIGNATURE_PAIRING, union_support_terms, 1)?;
                for (residual, obstruction_coefficient) in signature
                    .iter_mut()
                    .zip(union.support()[position].coefficients())
                {
                    actual_pairing = checked_add(SIGNATURE_PAIRING, actual_pairing, 1)?;
                    check_limit(
                        SIGNATURE_PAIRING,
                        actual_pairing,
                        limits.max_block_signature_pairing_operations,
                    )?;
                    *residual =
                        field.add(&*residual, &field.mul(coefficient, obstruction_coefficient));
                }
            }
            let (new_forbidden, new_physical) =
                classify_new_columns(frame, partition, source.terms().keys(), limits)?;
            candidates.push(ObstructionBlockProposalCandidate::from_parts(
                request.clone(),
                signature,
                ObstructionBlockProposalScore::new(
                    new_forbidden,
                    new_physical,
                    union_support_terms,
                    source.terms().len(),
                ),
            ));
        }
        Ok(ObstructionBlockProposalBatch {
            modulus: frame.sample_fingerprint().modulus(),
            width,
            candidates: candidates.into_boxed_slice(),
            telemetry: ObstructionBlockProposalTelemetry {
                logical_candidates: candidate_count,
                logical_source_terms,
                signature_pairing_operations: actual_pairing,
            },
        })
    }
}

fn validate_join(
    incidence: &OrdinarySourceIncidenceIndex<'_>,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    nominations: &ObstructionBlockNominations<'_>,
    frame: &ModularPhysicalFrame<'_>,
    obstruction: &ModularRightObstruction<'_>,
    partition: &TargetColumnPartition<'_>,
) -> Result<(), SourceDiscoveryError> {
    if !std::ptr::eq(frame.plan(), obstruction.plan())
        || !std::sync::Arc::ptr_eq(frame.sample_fingerprint(), obstruction.sample_fingerprint())
    {
        return Err(SourceDiscoveryError::ObstructionPlanMismatch);
    }
    if !std::ptr::eq(partition.frame(), frame.plan())
        || partition.target_column() != obstruction.target_physical_column()
        || partition.forbidden_columns() != obstruction.logical_forbidden_columns()
    {
        return Err(SourceDiscoveryError::ProposalPartitionMismatch);
    }
    match nominations.primary().origin() {
        IncidentNominationOrigin::CheckedObstruction(identity)
            if identity.belongs_to(obstruction) => {}
        _ => return Err(SourceDiscoveryError::NominationObstructionMismatch),
    }
    if nominations.primary().requests() != nominations.union().primary_requests() {
        return Err(SourceDiscoveryError::Invariant {
            detail: "obstruction-block union lost its exact primary nomination subset",
        });
    }
    if generator.context().fingerprint() != incidence.context_fingerprint()
        || frame.plan().family_fingerprint() != incidence.family_fingerprint()
        || frame.plan().context_fingerprint() != incidence.context_fingerprint()
    {
        return Err(SourceDiscoveryError::ScopeMismatch {
            detail: "obstruction-block proposal scope differs from its incidence module",
        });
    }
    if !completed.is_complete_ordinary()
        || completed.source_row_count() != incidence.source_count()
        || (0..completed.source_row_count()).any(|ordinal| {
            completed.source_row_id(ordinal)
                != incidence
                    .sources()
                    .get(ordinal)
                    .map(|source| source.row_id())
        })
    {
        return Err(SourceDiscoveryError::CompletedSourceChronologyMismatch);
    }
    Ok(())
}

fn classify_new_columns<'shift>(
    frame: &ModularPhysicalFrame<'_>,
    partition: &TargetColumnPartition<'_>,
    shifts: impl Iterator<Item = &'shift crate::identity::IndexShift>,
    limits: SourceDiscoveryLimits,
) -> Result<(usize, usize), SourceDiscoveryError> {
    let mut new_forbidden = 0usize;
    let mut new_physical = 0usize;
    for shift in shifts {
        if frame
            .plan()
            .columns()
            .binary_search_by(|candidate| candidate.values().cmp(shift.values()))
            .is_ok()
        {
            continue;
        }
        new_physical = checked_add(CANDIDATE_CLASSIFICATIONS, new_physical, 1)?;
        check_limit(
            CANDIDATE_CLASSIFICATIONS,
            new_physical,
            limits.max_block_candidate_classifications,
        )?;
        match partition
            .try_classify_prospective_shift(shift.values())
            .map_err(SourceDiscoveryError::ProposalClassification)?
        {
            ProspectiveColumnKind::Forbidden => {
                new_forbidden = checked_add(CANDIDATE_CLASSIFICATIONS, new_forbidden, 1)?;
            }
            ProspectiveColumnKind::Allowed => {}
            ProspectiveColumnKind::Target => {
                return Err(SourceDiscoveryError::Invariant {
                    detail: "an absent block-proposal shift classified as the materialized target",
                });
            }
        }
    }
    Ok((new_forbidden, new_physical))
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SourceDiscoveryError> {
    left.checked_mul(right)
        .ok_or(SourceDiscoveryError::ResourceCountOverflow { resource })
}
