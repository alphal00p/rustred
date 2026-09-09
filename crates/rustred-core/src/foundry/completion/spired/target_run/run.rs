use std::{cmp::Ordering, sync::Arc};

use crate::foundry::completion::guard::ExactGuardProbeWitness;
use crate::foundry::completion::source_discovery::{
    CampaignModularProbe, ExactRuleCellPromotionDisposition,
};
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator, TranslatedSourceRequest};

use super::super::{
    SignedL1ShellScheduler, SpiredCompactLift, SpiredExecutionCase, SpiredModularHit,
    SpiredModularStreamOutcome, SpiredPostHitCandidate, SpiredPreparedStreamingDiscovery,
    SpiredStructuralPreparation, SpiredStructuralPreparationCensus, SpiredStructuralScopeIdentity,
    SpiredValidatedPrime, try_lift_spired_compact_support, try_lift_spired_rooted_compact_support,
    try_promote_spired_replayed_rule_cell,
};
use super::tape::{PREPARED_TAPE_ROWS, SpiredPreparedRowTape, SpiredPreparedRowTapeError};
use super::{
    SpiredAlternativeExhaustion, SpiredKnownZeroPromotionRejection, SpiredTargetRunCensus,
    SpiredTargetRunError, SpiredTargetRunErrorCause, SpiredTargetRunLimits, SpiredTargetRunOutcome,
    SpiredTargetRunReport, SpiredTargetRunStage,
};

const CHUNKS: &str = "target-run materialized chunks";
const SHELLS: &str = "target-run opened or completed shells";
const REQUESTS: &str = "target-run scheduled requests";
const REQUEST_COORDINATES: &str = "target-run scheduled request coordinate cells";
const STREAMED_ROWS: &str = "target-run aggregate streamed rows";
const SEARCH_BRANCHES: &str = "target-run canonical source-exclusion branches";
const EXCLUSION_REQUESTS: &str = "target-run retained source-exclusion requests";
const EXCLUSION_COORDINATES: &str = "target-run retained source-exclusion request coordinate cells";
const REJECTED_SUPPORT_REQUESTS: &str = "target-run retained rejected-support requests";
const REJECTED_SUPPORT_COORDINATES: &str =
    "target-run retained rejected-support request coordinate cells";
const KNOWN_ZERO_PROMOTIONS: &str = "target-run exact known-zero promotion rejections";
const POST_HIT_ROWS: &str = "target-run post-hit streamed rows";
const POST_HIT_CANDIDATES: &str = "target-run post-hit modular candidates";
const ROOTED_LIFT_ATTEMPTS: &str = "target-run rooted compact-lift attempts";
const VIABLE_CANDIDATES: &str = "target-run viable exact candidates compared";
const BEST_REPLACEMENTS: &str = "target-run best-candidate replacements";
const DEPTH: &str = "target-run next signed-L1 depth";

/// Reusable structural workspace for one immutable case and target.
///
/// Exact source validation, shift classification, and stable forbidden IDs
/// live here and run once in canonical scheduler chronology. Every modular
/// probe and source-exclusion branch receives fresh evaluator/reducer state
/// while replaying the same immutable row-plan tape.
#[derive(Debug)]
pub(crate) struct SpiredTargetRunWorkspace<'workspace, 'family> {
    generator: &'workspace ParametricIbpGenerator<'family>,
    completed: &'workspace CompletedIbpSourceRows,
    case: SpiredExecutionCase,
    scheduler: SignedL1ShellScheduler,
    preparation: SpiredStructuralPreparation<'workspace, 'workspace>,
    structural_scope: Arc<SpiredStructuralScopeIdentity>,
    /// A flat immutable row tape retains lazy early-hit semantics without one
    /// heap allocation and `Arc` control block per streamed source. Future
    /// rows are never classified merely because they shared a scheduler chunk
    /// with the row that hit.
    prepared_tape: SpiredPreparedRowTape,
    limits: SpiredTargetRunLimits,
}

impl<'workspace, 'family> SpiredTargetRunWorkspace<'workspace, 'family> {
    pub(crate) fn try_new(
        generator: &'workspace ParametricIbpGenerator<'family>,
        completed: &'workspace CompletedIbpSourceRows,
        case: &SpiredExecutionCase,
        limits: SpiredTargetRunLimits,
    ) -> Result<Self, SpiredTargetRunError> {
        let census = SpiredTargetRunCensus::default();
        if limits.request_chunk_size == 0 {
            return Err(component_error(
                SpiredTargetRunStage::Scheduling,
                census,
                SpiredTargetRunErrorCause::Scheduling(
                    super::super::SpiredFoundationError::EmptyRequestChunk,
                ),
            ));
        }
        check_limit(SEARCH_BRANCHES, 1, limits.max_search_branches, census)?;
        check_limit(
            "target-run distinct compact-lift attempts",
            1,
            limits.max_distinct_compact_lift_attempts,
            census,
        )?;
        let scheduler = SignedL1ShellScheduler::try_new(
            case.arity(),
            completed.source_row_count(),
            limits.schedule,
        )
        .map_err(|error| {
            component_error(
                SpiredTargetRunStage::Scheduling,
                census,
                SpiredTargetRunErrorCause::Scheduling(error),
            )
        })?;
        let mut structural_limits = limits.streaming.structural_preparation();
        // The shared tape advances through skipped requests as well as rows
        // admitted to one probe-local kernel. Bound it by the outer canonical
        // request universe rather than the per-kernel admitted-row cap.
        structural_limits.max_prepared_rows = structural_limits
            .max_request_inspections
            .min(limits.max_total_scheduled_requests)
            .min(limits.max_retained_prepared_rows);
        structural_limits.max_prepared_term_roles = structural_limits
            .max_source_term_inspections
            .min(limits.max_retained_prepared_term_roles);
        structural_limits.max_prepared_request_offset_cells =
            limits.max_total_request_coordinate_cells;
        let preparation = SpiredStructuralPreparation::try_new(
            generator.context(),
            completed,
            case,
            structural_limits,
        )
        .map_err(|error| {
            component_error(
                SpiredTargetRunStage::Streaming,
                census,
                SpiredTargetRunErrorCause::Streaming(error.into()),
            )
        })?;
        let structural_scope = preparation.scope();
        Ok(Self {
            generator,
            completed,
            case: case.clone(),
            scheduler,
            preparation,
            structural_scope,
            prepared_tape: SpiredPreparedRowTape::default(),
            limits,
        })
    }

    pub(crate) const fn structural_census(&self) -> SpiredStructuralPreparationCensus {
        self.preparation.census()
    }

    pub(crate) fn retained_prepared_rows(&self) -> usize {
        self.prepared_tape.row_count()
    }

    pub(crate) fn retained_prepared_term_roles(&self) -> usize {
        self.prepared_tape.term_role_count()
    }

    pub(crate) fn try_run_probe(
        &mut self,
        probe: &CampaignModularProbe,
    ) -> Result<SpiredTargetRunReport, SpiredTargetRunError> {
        try_run_spired_target_inner(self, probe, None)
    }

    pub(crate) fn try_run_guarded_probe(
        &mut self,
        probe: &CampaignModularProbe,
        witness: &ExactGuardProbeWitness,
    ) -> Result<SpiredTargetRunReport, SpiredTargetRunError> {
        try_run_spired_target_inner(self, probe, Some(witness))
    }

    fn try_ensure_prepared_row(
        &mut self,
        row_ordinal: usize,
        request: &TranslatedSourceRequest,
        census: SpiredTargetRunCensus,
    ) -> Result<usize, SpiredTargetRunError> {
        let source_term_count = self.try_source_term_count(request, census)?;
        if row_ordinal < self.prepared_tape.row_count() {
            let row = self.prepared_tape.row(row_ordinal).ok_or_else(|| {
                streaming_invariant_error(census, "a cached flat prepared row disappeared")
            })?;
            if row.ordinal() != row_ordinal
                || row.request() != request
                || row.term_role_count() != source_term_count
            {
                return Err(streaming_invariant_error(
                    census,
                    "a cached structural row disagreed with canonical scheduler chronology",
                ));
            }
            return Ok(row_ordinal);
        }
        if row_ordinal != self.prepared_tape.row_count() {
            return Err(streaming_invariant_error(
                census,
                "a target branch requested a noncontiguous structural row",
            ));
        }
        self.prepared_tape
            .try_reserve_new_row(
                row_ordinal,
                source_term_count,
                self.limits.max_retained_prepared_rows,
                self.limits.max_retained_prepared_term_roles,
            )
            .map_err(|error| prepared_tape_error(error, census))?;
        let row = {
            let preparation = &mut self.preparation;
            let term_roles = self.prepared_tape.term_roles_mut();
            preparation
                .try_prepare_one_request_into(request, term_roles)
                .map_err(|error| {
                    component_error(
                        SpiredTargetRunStage::Streaming,
                        census,
                        SpiredTargetRunErrorCause::Streaming(error.into()),
                    )
                })?
        };
        if row.ordinal() != row_ordinal
            || row.request() != request
            || row.term_role_count() != source_term_count
        {
            return Err(streaming_invariant_error(
                census,
                "new structural preparation disagreed with canonical scheduler chronology",
            ));
        }
        Ok(self.prepared_tape.push_prepared_row_after_preflight(row))
    }

    fn try_source_term_count(
        &self,
        request: &TranslatedSourceRequest,
        census: SpiredTargetRunCensus,
    ) -> Result<usize, SpiredTargetRunError> {
        if request.offset().len() != self.case.arity() {
            return Err(component_error(
                SpiredTargetRunStage::Streaming,
                census,
                SpiredTargetRunErrorCause::Streaming(
                    super::super::DirectShiftedSourceError::WrongOffsetArity {
                        expected: self.case.arity(),
                        actual: request.offset().len(),
                    }
                    .into(),
                ),
            ));
        }
        self.completed
            .source_relation(request.source_ordinal())
            .map(|source| source.terms().len())
            .ok_or_else(|| {
                component_error(
                    SpiredTargetRunStage::Streaming,
                    census,
                    SpiredTargetRunErrorCause::Streaming(
                        super::super::DirectShiftedSourceError::SourceOrdinalOutOfRange {
                            source_ordinal: request.source_ordinal(),
                            source_count: self.completed.source_row_count(),
                        }
                        .into(),
                    ),
                )
            })
    }
}

/// Run one deterministic signed-L1 source lane through ordinary RuleCell
/// promotion. A bounded continuation of the live all-source reducer compares
/// later exact GPLU roots against the first rule; only if none is usable does
/// the canonical source-exclusion fallback nominate alternative circuits.
pub(crate) fn try_run_spired_target(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    case: &SpiredExecutionCase,
    probe: &CampaignModularProbe,
    limits: SpiredTargetRunLimits,
) -> Result<SpiredTargetRunReport, SpiredTargetRunError> {
    try_preflight_one_shot_probe(generator, case, probe, None)?;
    SpiredTargetRunWorkspace::try_new(generator, completed, case, limits)?.try_run_probe(probe)
}

/// Run one guarded target only after binding its exact branch predicates to
/// the retained raw probe. The witness is checked before any streaming state
/// or modular row is constructed.
pub(crate) fn try_run_spired_guarded_target(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    case: &SpiredExecutionCase,
    probe: &CampaignModularProbe,
    witness: &ExactGuardProbeWitness,
    limits: SpiredTargetRunLimits,
) -> Result<SpiredTargetRunReport, SpiredTargetRunError> {
    try_preflight_one_shot_probe(generator, case, probe, Some(witness))?;
    SpiredTargetRunWorkspace::try_new(generator, completed, case, limits)?
        .try_run_guarded_probe(probe, witness)
}

fn try_preflight_one_shot_probe(
    generator: &ParametricIbpGenerator<'_>,
    case: &SpiredExecutionCase,
    probe: &CampaignModularProbe,
    witness: Option<&ExactGuardProbeWitness>,
) -> Result<(), SpiredTargetRunError> {
    let census = SpiredTargetRunCensus::default();
    let result = (|| -> Result<(), super::super::SpiredStreamingError> {
        match (case.stratum().guards().is_empty(), witness) {
            (false, None) => {
                return Err(
                    super::super::SpiredStreamingError::GuardedStratumRequiresSampleWitness {
                        guard_count: case.stratum().guards().len(),
                    },
                );
            }
            (true, Some(_)) => {
                return Err(
                    crate::foundry::completion::guard::ExactGuardProbeError::GuardBlindStratum
                        .into(),
                );
            }
            _ => {}
        }
        let expected_base = generator.context().base().parameter_names().len();
        if probe.base_parameters().len() != expected_base {
            return Err(
                super::super::DirectShiftedSourceError::WrongBaseParameterArity {
                    expected: expected_base,
                    actual: probe.base_parameters().len(),
                }
                .into(),
            );
        }
        let exact_indices = probe.try_index_anchor_for_stratum(case.stratum())?;
        if let Some(witness) = witness {
            witness.try_validate_scope(
                generator.context(),
                case.stratum(),
                probe.base_parameters(),
                &exact_indices,
            )?;
        }
        let prime = SpiredValidatedPrime::try_new(probe.modulus())?;
        if let Some(witness) = witness {
            witness.try_validate_modulus(prime.field())?;
        }
        Ok(())
    })();
    result.map_err(|error| {
        component_error(
            SpiredTargetRunStage::Streaming,
            census,
            SpiredTargetRunErrorCause::Streaming(error),
        )
    })
}

fn try_run_spired_target_inner(
    workspace: &mut SpiredTargetRunWorkspace<'_, '_>,
    probe: &CampaignModularProbe,
    guard_witness: Option<&ExactGuardProbeWitness>,
) -> Result<SpiredTargetRunReport, SpiredTargetRunError> {
    let mut census = SpiredTargetRunCensus::default();
    let limits = workspace.limits;
    // One rejected sampled circuit S partitions alternative sampled circuits
    // into the canonical children E union {r}, r in S.  This is a bounded
    // discovery portfolio, not an exact module-completeness proof: an unlucky
    // specialization may collapse a larger exact support onto S.  Therefore
    // exhausting this frontier always remains explicitly inconclusive and an
    // independent probe is still permitted (and usually preferable).
    let mut frontier = ExclusionFrontier::try_new(census)?;
    let mut rejected_supports = Vec::<Box<[TranslatedSourceRequest]>>::new();
    let mut last_rejection = None;

    while let Some(branch_ordinal) = frontier.next_branch_ordinal() {
        census.set_search_branches_started(checked_add(
            SEARCH_BRANCHES,
            census.search_branches_started(),
            1,
            census,
        )?);
        let branch = frontier.branch(branch_ordinal).ok_or_else(|| {
            invariant_error(
                census,
                "the source-exclusion frontier lost an admitted branch",
            )
        })?;
        let lane = try_run_exclusion_branch(
            workspace,
            probe,
            guard_witness,
            branch,
            branch_ordinal != 0,
            &rejected_supports,
            &mut census,
            limits,
        )?;
        match lane {
            BranchLaneOutcome::RuleCell(usable) => {
                return Ok(SpiredTargetRunReport::new(
                    census,
                    SpiredTargetRunOutcome::RuleCell(usable),
                ));
            }
            BranchLaneOutcome::DuplicateHit(hit) => {
                let existing = rejected_supports
                    .iter()
                    .find(|support| support.as_ref() == hit.support())
                    .ok_or_else(|| {
                        invariant_error(
                            census,
                            "a duplicate branch hit lost its retained rejected support",
                        )
                    })?;
                census.set_duplicate_rejected_support_hits(checked_add(
                    "target-run duplicate rejected-support hits",
                    census.duplicate_rejected_support_hits(),
                    1,
                    census,
                )?);
                frontier.try_enqueue_children_for_branch(
                    branch_ordinal,
                    existing,
                    workspace.case.arity(),
                    &mut census,
                    limits,
                )?;
            }
            BranchLaneOutcome::Rejected {
                first_hit,
                last,
                exhaustion,
            } => {
                last_rejection = Some(last);
                if let Some(exhaustion) = exhaustion {
                    return finish_inconclusive(census, last_rejection, exhaustion);
                }
                let retention = try_retain_rejected_support(
                    &mut rejected_supports,
                    first_hit.support(),
                    workspace.case.arity(),
                    &mut census,
                    limits,
                )?;
                if let Some(exhaustion) = retention {
                    return finish_inconclusive(census, last_rejection, exhaustion);
                }
                frontier.try_enqueue_children_for_branch(
                    branch_ordinal,
                    first_hit.support(),
                    workspace.case.arity(),
                    &mut census,
                    limits,
                )?;
            }
            BranchLaneOutcome::DepthExhausted => {
                census.set_search_branches_exhausted(checked_add(
                    SEARCH_BRANCHES,
                    census.search_branches_exhausted(),
                    1,
                    census,
                )?);
            }
            BranchLaneOutcome::AlternativeRowsExhausted => {
                return finish_inconclusive(
                    census,
                    last_rejection,
                    SpiredAlternativeExhaustion::AlternativeStreamedRows,
                );
            }
            BranchLaneOutcome::AttemptBudgetExhausted => {
                return finish_inconclusive(
                    census,
                    last_rejection,
                    SpiredAlternativeExhaustion::DistinctCompactLiftAttempts,
                );
            }
        }
    }

    match last_rejection {
        Some(last) => finish_inconclusive(
            census,
            Some(last),
            frontier
                .truncation()
                .unwrap_or(SpiredAlternativeExhaustion::SearchDepth),
        ),
        None => Ok(SpiredTargetRunReport::new(
            census,
            SpiredTargetRunOutcome::SearchDepthExhausted,
        )),
    }
}

#[derive(Debug)]
enum BranchLaneOutcome {
    RuleCell(ExactRuleCellPromotionDisposition),
    DuplicateHit(SpiredModularHit),
    Rejected {
        first_hit: SpiredModularHit,
        last: AlternativeCandidateRejection,
        exhaustion: Option<SpiredAlternativeExhaustion>,
    },
    DepthExhausted,
    AlternativeRowsExhausted,
    AttemptBudgetExhausted,
}

#[derive(Debug)]
pub(super) enum AlternativeCandidateRejection {
    Compact(SpiredCompactLift),
    KnownZero(SpiredKnownZeroPromotionRejection),
}

#[derive(Clone, Copy, Debug)]
pub(super) enum BranchCandidateProposal<'candidate> {
    First(&'candidate SpiredModularHit),
    Rooted(&'candidate SpiredPostHitCandidate),
}

#[derive(Debug)]
pub(super) enum BranchCandidateAttempt {
    Viable(ExactRuleCellPromotionDisposition),
    Rejected(AlternativeCandidateRejection),
    AttemptBudgetExhausted,
}

/// Cheap topology-independent ordering for the bounded post-hit portfolio.
///
/// Lower is better. Exact admissibility wins first, then fewer new guard
/// branches and predicates, then smaller exact circuits and source frames.
/// Canonical translated-source provenance breaks scalar ties below without
/// retaining a second copy in this key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct BranchCandidateQuality {
    promotion_class: u8,
    new_guard_branches: usize,
    required_guard_predicates: usize,
    residual_terms: usize,
    source_combination_terms: usize,
    source_requests: usize,
}

fn candidate_quality(
    candidate: &ExactRuleCellPromotionDisposition,
) -> Option<BranchCandidateQuality> {
    let (promotion_class, refinement, circuit, source_requests) = match candidate {
        ExactRuleCellPromotionDisposition::Admitted(candidate) => (
            0,
            candidate.guard_refinement(),
            candidate.circuit().as_ref(),
            candidate.epoch().requests().len(),
        ),
        ExactRuleCellPromotionDisposition::NeedsGuardedStratum {
            epoch,
            circuit,
            refinement,
            ..
        } => (1, refinement, circuit.as_ref(), epoch.requests().len()),
        ExactRuleCellPromotionDisposition::AnchorOnGuardWall {
            epoch,
            circuit,
            refinement,
            ..
        } => (2, refinement, circuit.as_ref(), epoch.requests().len()),
        ExactRuleCellPromotionDisposition::BlockedByKnownZero { .. } => return None,
    };
    Some(BranchCandidateQuality {
        promotion_class,
        new_guard_branches: refinement.newly_split_predicate_ordinals().len(),
        required_guard_predicates: refinement.required_predicates().len(),
        residual_terms: circuit.residual_terms().len(),
        source_combination_terms: circuit.source_combination().len(),
        source_requests,
    })
}

fn candidate_provenance(
    candidate: &ExactRuleCellPromotionDisposition,
) -> Option<&[TranslatedSourceRequest]> {
    match candidate {
        ExactRuleCellPromotionDisposition::Admitted(candidate) => {
            Some(candidate.epoch().requests().requests())
        }
        ExactRuleCellPromotionDisposition::NeedsGuardedStratum { epoch, .. }
        | ExactRuleCellPromotionDisposition::AnchorOnGuardWall { epoch, .. } => {
            Some(epoch.requests().requests())
        }
        ExactRuleCellPromotionDisposition::BlockedByKnownZero { .. } => None,
    }
}

fn compare_viable_candidates(
    left: &ExactRuleCellPromotionDisposition,
    right: &ExactRuleCellPromotionDisposition,
) -> Option<Ordering> {
    let left_quality = candidate_quality(left)?;
    let right_quality = candidate_quality(right)?;
    let left_provenance = candidate_provenance(left)?;
    let right_provenance = candidate_provenance(right)?;
    Some(
        left_quality
            .cmp(&right_quality)
            .then_with(|| left_provenance.cmp(right_provenance)),
    )
}

fn try_retain_viable_candidate(
    best: &mut Option<ExactRuleCellPromotionDisposition>,
    candidate: ExactRuleCellPromotionDisposition,
    census: &mut SpiredTargetRunCensus,
) -> Result<(), SpiredTargetRunError> {
    census.set_viable_candidates_compared(checked_add(
        VIABLE_CANDIDATES,
        census.viable_candidates_compared(),
        1,
        *census,
    )?);
    let replace = match best.as_ref() {
        None => true,
        Some(retained) => compare_viable_candidates(&candidate, retained)
            .ok_or_else(|| {
                invariant_error(
                    *census,
                    "a known-zero candidate entered the viable comparison portfolio",
                )
            })?
            .is_lt(),
    };
    if replace {
        if best.is_some() {
            census.set_best_candidate_replacements(checked_add(
                BEST_REPLACEMENTS,
                census.best_candidate_replacements(),
                1,
                *census,
            )?);
        }
        *best = Some(candidate);
    }
    Ok(())
}

pub(super) fn try_attempt_branch_candidate(
    workspace: &SpiredTargetRunWorkspace<'_, '_>,
    probe: &CampaignModularProbe,
    proposal: BranchCandidateProposal<'_>,
    census: &mut SpiredTargetRunCensus,
    limits: SpiredTargetRunLimits,
) -> Result<BranchCandidateAttempt, SpiredTargetRunError> {
    let next_attempts = checked_add(
        "target-run distinct compact-lift attempts",
        census.distinct_compact_lift_attempts(),
        1,
        *census,
    )?;
    if next_attempts > limits.max_distinct_compact_lift_attempts {
        return Ok(BranchCandidateAttempt::AttemptBudgetExhausted);
    }
    census.set_distinct_compact_lift_attempts(next_attempts);
    if matches!(proposal, BranchCandidateProposal::Rooted(_)) {
        census.set_rooted_compact_lift_attempts(checked_add(
            ROOTED_LIFT_ATTEMPTS,
            census.rooted_compact_lift_attempts(),
            1,
            *census,
        )?);
    }

    let lift = match proposal {
        BranchCandidateProposal::First(hit) => try_lift_spired_compact_support(
            &workspace.case,
            workspace.generator,
            workspace.completed,
            hit,
            probe,
            limits.compact_lift,
        ),
        BranchCandidateProposal::Rooted(candidate) => try_lift_spired_rooted_compact_support(
            &workspace.case,
            workspace.generator,
            workspace.completed,
            candidate,
            probe,
            limits.compact_lift,
        ),
    }
    .map_err(|error| {
        component_error(
            SpiredTargetRunStage::CompactLift,
            *census,
            SpiredTargetRunErrorCause::CompactLift(error),
        )
    })?;

    match lift {
        SpiredCompactLift::Replayed(replayed) => {
            let promoted = try_promote_spired_replayed_rule_cell(
                workspace.generator.context(),
                replayed,
                limits.promotion,
            )
            .map_err(|error| {
                component_error(
                    SpiredTargetRunStage::RuleCellPromotion,
                    *census,
                    SpiredTargetRunErrorCause::RuleCellPromotion(error),
                )
            })?;
            match promoted {
                ExactRuleCellPromotionDisposition::BlockedByKnownZero {
                    required_predicate_ordinal,
                    first_circuit_guard_ordinal,
                    zero_branch,
                    ..
                } => {
                    census.set_known_zero_promotions_rejected(checked_add(
                        KNOWN_ZERO_PROMOTIONS,
                        census.known_zero_promotions_rejected(),
                        1,
                        *census,
                    )?);
                    Ok(BranchCandidateAttempt::Rejected(
                        AlternativeCandidateRejection::KnownZero(
                            SpiredKnownZeroPromotionRejection::new(
                                required_predicate_ordinal,
                                first_circuit_guard_ordinal,
                                zero_branch,
                            ),
                        ),
                    ))
                }
                viable => Ok(BranchCandidateAttempt::Viable(viable)),
            }
        }
        inconclusive @ (SpiredCompactLift::FreshFrameDidNotHit { .. }
        | SpiredCompactLift::ExactSupportDidNotLift { .. }
        | SpiredCompactLift::ExactSupportBudgetExceeded { .. }
        | SpiredCompactLift::RootedExactDidNotLift { .. }) => Ok(BranchCandidateAttempt::Rejected(
            AlternativeCandidateRejection::Compact(inconclusive),
        )),
    }
}

#[derive(Debug)]
struct ExclusionFrontier {
    branches: Vec<Box<[TranslatedSourceRequest]>>,
    next: usize,
    truncation: Option<SpiredAlternativeExhaustion>,
}

impl ExclusionFrontier {
    fn try_new(census: SpiredTargetRunCensus) -> Result<Self, SpiredTargetRunError> {
        let mut branches = Vec::new();
        branches
            .try_reserve_exact(1)
            .map_err(|_| allocation_error(SEARCH_BRANCHES, 1, census))?;
        branches.push(Vec::new().into_boxed_slice());
        Ok(Self {
            branches,
            next: 0,
            truncation: None,
        })
    }

    fn next_branch_ordinal(&mut self) -> Option<usize> {
        let ordinal = self.next;
        if ordinal == self.branches.len() {
            None
        } else {
            self.next += 1;
            Some(ordinal)
        }
    }

    fn branch(&self, ordinal: usize) -> Option<&[TranslatedSourceRequest]> {
        self.branches.get(ordinal).map(AsRef::as_ref)
    }

    const fn truncation(&self) -> Option<SpiredAlternativeExhaustion> {
        self.truncation
    }

    #[allow(clippy::too_many_arguments)]
    fn try_enqueue_children_for_branch(
        &mut self,
        parent_ordinal: usize,
        rejected_support: &[TranslatedSourceRequest],
        arity: usize,
        census: &mut SpiredTargetRunCensus,
        limits: SpiredTargetRunLimits,
    ) -> Result<(), SpiredTargetRunError> {
        let parent = self.branches.get(parent_ordinal).ok_or_else(|| {
            invariant_error(
                *census,
                "the source-exclusion frontier lost an admitted parent branch",
            )
        })?;
        if rejected_support
            .iter()
            .any(|request| parent.binary_search(request).is_ok())
        {
            return Err(invariant_error(
                *census,
                "a modular support retained a branch-excluded source request",
            ));
        }
        if rejected_support.is_empty() {
            return Err(invariant_error(
                *census,
                "a rejected modular hit retained an empty source support",
            ));
        }

        let child_len = checked_add(EXCLUSION_REQUESTS, parent.len(), 1, *census)?;
        if child_len > limits.max_excluded_requests_per_branch {
            self.note_truncation(SpiredAlternativeExhaustion::ExcludedRequestsPerBranch);
            return Ok(());
        }
        let has_novel_child = rejected_support.iter().any(|omitted| {
            let position = parent.binary_search(omitted).unwrap_err();
            !self
                .branches
                .iter()
                .any(|existing| inserted_child_matches(existing, parent, position, omitted))
        });
        if !has_novel_child {
            return Ok(());
        }
        if self.branches.len() >= limits.max_search_branches {
            self.note_truncation(SpiredAlternativeExhaustion::SearchBranches);
            return Ok(());
        }
        let next_requests = checked_add(
            EXCLUSION_REQUESTS,
            census.retained_exclusion_requests(),
            child_len,
            *census,
        )?;
        if next_requests > limits.max_retained_exclusion_requests {
            self.note_truncation(SpiredAlternativeExhaustion::RetainedExclusionRequests);
            return Ok(());
        }
        let child_coordinates = checked_mul(EXCLUSION_COORDINATES, child_len, arity, *census)?;
        let next_coordinates = checked_add(
            EXCLUSION_COORDINATES,
            census.retained_exclusion_coordinate_cells(),
            child_coordinates,
            *census,
        )?;
        if next_coordinates > limits.max_retained_exclusion_coordinate_cells {
            self.note_truncation(SpiredAlternativeExhaustion::RetainedExclusionCoordinateCells);
            return Ok(());
        }

        // The cheapest global/per-child limits have now admitted at least one
        // novel child. Only now copy the parent that must outlive mutation of
        // the frontier below.
        let parent = try_clone_requests(parent, EXCLUSION_REQUESTS, *census)?;
        self.try_enqueue_children(&parent, rejected_support, arity, census, limits)
    }

    #[allow(clippy::too_many_arguments)]
    fn try_enqueue_children(
        &mut self,
        parent: &[TranslatedSourceRequest],
        rejected_support: &[TranslatedSourceRequest],
        arity: usize,
        census: &mut SpiredTargetRunCensus,
        limits: SpiredTargetRunLimits,
    ) -> Result<(), SpiredTargetRunError> {
        for omitted in rejected_support {
            let position = match parent.binary_search(omitted) {
                Ok(_) => {
                    return Err(invariant_error(
                        *census,
                        "a modular support retained a branch-excluded source request",
                    ));
                }
                Err(position) => position,
            };
            let child_len = checked_add(EXCLUSION_REQUESTS, parent.len(), 1, *census)?;
            if child_len > limits.max_excluded_requests_per_branch {
                self.note_truncation(SpiredAlternativeExhaustion::ExcludedRequestsPerBranch);
                continue;
            }
            if self
                .branches
                .iter()
                .any(|existing| inserted_child_matches(existing, parent, position, omitted))
            {
                continue;
            }
            if self.branches.len() >= limits.max_search_branches {
                self.note_truncation(SpiredAlternativeExhaustion::SearchBranches);
                continue;
            }
            let next_requests = checked_add(
                EXCLUSION_REQUESTS,
                census.retained_exclusion_requests(),
                child_len,
                *census,
            )?;
            if next_requests > limits.max_retained_exclusion_requests {
                self.note_truncation(SpiredAlternativeExhaustion::RetainedExclusionRequests);
                continue;
            }
            let child_coordinates = checked_mul(EXCLUSION_COORDINATES, child_len, arity, *census)?;
            let next_coordinates = checked_add(
                EXCLUSION_COORDINATES,
                census.retained_exclusion_coordinate_cells(),
                child_coordinates,
                *census,
            )?;
            if next_coordinates > limits.max_retained_exclusion_coordinate_cells {
                self.note_truncation(SpiredAlternativeExhaustion::RetainedExclusionCoordinateCells);
                continue;
            }
            let mut child = Vec::new();
            child
                .try_reserve_exact(child_len)
                .map_err(|_| allocation_error(EXCLUSION_REQUESTS, child_len, *census))?;
            child.extend_from_slice(&parent[..position]);
            child.push(omitted.clone());
            child.extend_from_slice(&parent[position..]);
            self.branches
                .try_reserve_exact(1)
                .map_err(|_| allocation_error(SEARCH_BRANCHES, self.branches.len() + 1, *census))?;
            self.branches.push(child.into_boxed_slice());
            census.set_retained_exclusion_requests(next_requests);
            census.set_retained_exclusion_coordinate_cells(next_coordinates);
            census.set_alternative_branches_enqueued(checked_add(
                SEARCH_BRANCHES,
                census.alternative_branches_enqueued(),
                1,
                *census,
            )?);
        }
        Ok(())
    }

    fn note_truncation(&mut self, reason: SpiredAlternativeExhaustion) {
        if self.truncation.is_none() {
            self.truncation = Some(reason);
        }
    }
}

fn inserted_child_matches(
    existing: &[TranslatedSourceRequest],
    parent: &[TranslatedSourceRequest],
    insertion: usize,
    inserted: &TranslatedSourceRequest,
) -> bool {
    existing.len() == parent.len() + 1
        && existing[..insertion] == parent[..insertion]
        && existing[insertion] == *inserted
        && existing[insertion + 1..] == parent[insertion..]
}

fn try_run_exclusion_branch(
    workspace: &mut SpiredTargetRunWorkspace<'_, '_>,
    probe: &CampaignModularProbe,
    guard_witness: Option<&ExactGuardProbeWitness>,
    excluded: &[TranslatedSourceRequest],
    alternative: bool,
    rejected_supports: &[Box<[TranslatedSourceRequest]>],
    census: &mut SpiredTargetRunCensus,
    limits: SpiredTargetRunLimits,
) -> Result<BranchLaneOutcome, SpiredTargetRunError> {
    // A fresh evaluator and two Symbolica sparse reducers are substantial
    // probe-local state. Reject an aggregate budget that cannot admit even one
    // row before constructing any of them. The ordering matches the row loop:
    // scheduling geometry first, then the alternative window, then streamed
    // work shared by every branch.
    let first_request = checked_add(REQUESTS, census.scheduled_requests(), 1, *census)?;
    check_limit(
        REQUESTS,
        first_request,
        limits.max_total_scheduled_requests,
        *census,
    )?;
    let first_request_coordinates = checked_add(
        REQUEST_COORDINATES,
        census.scheduled_request_coordinate_cells(),
        workspace.case.arity(),
        *census,
    )?;
    check_limit(
        REQUEST_COORDINATES,
        first_request_coordinates,
        limits.max_total_request_coordinate_cells,
        *census,
    )?;
    if alternative && census.alternative_streamed_rows() >= limits.max_alternative_streamed_rows {
        return Ok(BranchLaneOutcome::AlternativeRowsExhausted);
    }
    let first_streamed = checked_add(STREAMED_ROWS, census.streamed_rows(), 1, *census)?;
    check_limit(
        STREAMED_ROWS,
        first_streamed,
        limits.max_total_streamed_rows,
        *census,
    )?;

    let discovery = match guard_witness {
        Some(witness) => SpiredPreparedStreamingDiscovery::try_new_guarded(
            &workspace.preparation,
            probe,
            witness,
            limits.streaming.modular,
        ),
        None => SpiredPreparedStreamingDiscovery::try_new(
            &workspace.preparation,
            probe,
            limits.streaming.modular,
        ),
    };
    let mut discovery = discovery.map_err(|error| {
        component_error(
            SpiredTargetRunStage::Streaming,
            *census,
            SpiredTargetRunErrorCause::Streaming(error),
        )
    })?;

    let mut depth = 0usize;
    let mut prepared_row_ordinal = 0usize;
    let mut first_hit = None;
    let mut last_rejection = None;
    let mut best = None;
    let mut first_hit_was_duplicate = false;
    let mut branch_post_hit_rows = 0usize;
    loop {
        census.set_current_depth(depth);
        let shell = workspace.scheduler.try_depth_shell(depth);
        let mut shell = match shell {
            Ok(shell) => shell,
            Err(error) if best.is_some() && is_scheduling_resource_stop(&error) => {
                return finish_branch_after_hit(first_hit, best, last_rejection, None, *census);
            }
            Err(error) => {
                return Err(component_error(
                    SpiredTargetRunStage::Scheduling,
                    *census,
                    SpiredTargetRunErrorCause::Scheduling(error),
                ));
            }
        };
        census.set_shells_opened(checked_add(SHELLS, census.shells_opened(), 1, *census)?);

        loop {
            let remaining = shell.remaining_request_count();
            if remaining == 0 {
                let exhausted =
                    shell
                        .try_next_chunk(limits.request_chunk_size)
                        .map_err(|error| {
                            component_error(
                                SpiredTargetRunStage::Scheduling,
                                *census,
                                SpiredTargetRunErrorCause::Scheduling(error),
                            )
                        })?;
                if exhausted.is_some() {
                    return Err(invariant_error(
                        *census,
                        "an exhausted signed-L1 shell emitted another chunk",
                    ));
                }
                census.set_shells_completed(checked_add(
                    SHELLS,
                    census.shells_completed(),
                    1,
                    *census,
                )?);
                break;
            }

            if first_hit.is_some()
                && (branch_post_hit_rows >= limits.max_post_hit_streamed_rows
                    || branch_post_hit_rows >= limits.streaming.modular.max_post_hit_rows)
            {
                return finish_branch_after_hit(first_hit, best, last_rejection, None, *census);
            }
            if first_hit.is_some()
                && census.distinct_compact_lift_attempts()
                    >= limits.max_distinct_compact_lift_attempts
            {
                let exhaustion = best
                    .is_none()
                    .then_some(SpiredAlternativeExhaustion::DistinctCompactLiftAttempts);
                return finish_branch_after_hit(
                    first_hit,
                    best,
                    last_rejection,
                    exhaustion,
                    *census,
                );
            }

            let residual_request_budget = limits
                .max_total_scheduled_requests
                .checked_sub(census.scheduled_requests())
                .ok_or_else(|| {
                    invariant_error(
                        *census,
                        "scheduled-request census exceeded its aggregate limit",
                    )
                })?;
            let residual_coordinate_budget = limits
                .max_total_request_coordinate_cells
                .checked_sub(census.scheduled_request_coordinate_cells())
                .ok_or_else(|| {
                    invariant_error(
                        *census,
                        "scheduled-coordinate census exceeded its aggregate limit",
                    )
                })?;
            let chunk_capacity = remaining
                .min(limits.request_chunk_size)
                .min(residual_request_budget)
                .min(residual_coordinate_budget / workspace.case.arity());
            if chunk_capacity == 0 {
                if best.is_some() {
                    return finish_branch_after_hit(first_hit, best, last_rejection, None, *census);
                }
                return Err(exhausted_schedule_budget_error(
                    workspace.case.arity(),
                    residual_request_budget,
                    *census,
                    limits,
                ));
            }
            let next_chunks = checked_add(CHUNKS, census.chunks_materialized(), 1, *census)?;
            if best.is_some() && next_chunks > limits.max_chunks {
                return finish_branch_after_hit(first_hit, best, last_rejection, None, *census);
            }
            check_limit(CHUNKS, next_chunks, limits.max_chunks, *census)?;
            let next_requests = checked_add(
                REQUESTS,
                census.scheduled_requests(),
                chunk_capacity,
                *census,
            )?;
            check_limit(
                REQUESTS,
                next_requests,
                limits.max_total_scheduled_requests,
                *census,
            )?;
            let chunk_coordinates = checked_mul(
                REQUEST_COORDINATES,
                chunk_capacity,
                workspace.case.arity(),
                *census,
            )?;
            let next_coordinates = checked_add(
                REQUEST_COORDINATES,
                census.scheduled_request_coordinate_cells(),
                chunk_coordinates,
                *census,
            )?;
            check_limit(
                REQUEST_COORDINATES,
                next_coordinates,
                limits.max_total_request_coordinate_cells,
                *census,
            )?;

            let chunk = shell.try_next_chunk(chunk_capacity);
            let chunk = match chunk {
                Ok(Some(chunk)) => chunk,
                Ok(None) => {
                    return Err(invariant_error(
                        *census,
                        "a nonempty signed-L1 shell failed to emit its next chunk",
                    ));
                }
                Err(error) if best.is_some() && is_scheduling_resource_stop(&error) => {
                    return finish_branch_after_hit(first_hit, best, last_rejection, None, *census);
                }
                Err(error) => {
                    return Err(component_error(
                        SpiredTargetRunStage::Scheduling,
                        *census,
                        SpiredTargetRunErrorCause::Scheduling(error),
                    ));
                }
            };
            if chunk.len() != chunk_capacity {
                return Err(invariant_error(
                    *census,
                    "a signed-L1 chunk differed from its exact preflight capacity",
                ));
            }
            let chunk_ordinal = census.chunks_materialized();
            census.set_chunks_materialized(next_chunks);
            census.set_scheduled_requests(next_requests);
            census.set_scheduled_request_coordinate_cells(next_coordinates);
            for (request_ordinal_in_chunk, request) in chunk.requests().iter().enumerate() {
                let is_excluded = excluded.binary_search(request).is_ok();
                let streamed_preflight = if is_excluded {
                    None
                } else {
                    if first_hit.is_some()
                        && (branch_post_hit_rows >= limits.max_post_hit_streamed_rows
                            || branch_post_hit_rows >= limits.streaming.modular.max_post_hit_rows)
                    {
                        return finish_branch_after_hit(
                            first_hit,
                            best,
                            last_rejection,
                            None,
                            *census,
                        );
                    }
                    if first_hit.is_some()
                        && census.distinct_compact_lift_attempts()
                            >= limits.max_distinct_compact_lift_attempts
                    {
                        let exhaustion = best
                            .is_none()
                            .then_some(SpiredAlternativeExhaustion::DistinctCompactLiftAttempts);
                        return finish_branch_after_hit(
                            first_hit,
                            best,
                            last_rejection,
                            exhaustion,
                            *census,
                        );
                    }
                    if alternative
                        && census.alternative_streamed_rows()
                            >= limits.max_alternative_streamed_rows
                    {
                        if best.is_some() {
                            return finish_branch_after_hit(
                                first_hit,
                                best,
                                last_rejection,
                                None,
                                *census,
                            );
                        }
                        return Ok(BranchLaneOutcome::AlternativeRowsExhausted);
                    }
                    let next_streamed =
                        checked_add(STREAMED_ROWS, census.streamed_rows(), 1, *census)?;
                    if next_streamed > limits.max_total_streamed_rows {
                        if best.is_some() {
                            return finish_branch_after_hit(
                                first_hit,
                                best,
                                last_rejection,
                                None,
                                *census,
                            );
                        }
                        check_limit(
                            STREAMED_ROWS,
                            next_streamed,
                            limits.max_total_streamed_rows,
                            *census,
                        )?;
                    }
                    let next_alternative = if alternative {
                        Some(checked_add(
                            "target-run alternative streamed rows",
                            census.alternative_streamed_rows(),
                            1,
                            *census,
                        )?)
                    } else {
                        None
                    };
                    Some((next_streamed, next_alternative))
                };

                // Excluded rows still need their exact prepared plan so the
                // probe can authenticate and advance canonical chronology.
                // For admitted rows, however, streamed-work caps are checked
                // before either tape allocation or fresh structural work.
                let prepared_index =
                    match workspace.try_ensure_prepared_row(prepared_row_ordinal, request, *census)
                    {
                        Ok(index) => index,
                        Err(error) if best.is_some() && is_target_resource_stop(&error) => {
                            return finish_branch_after_hit(
                                first_hit,
                                best,
                                last_rejection,
                                None,
                                *census,
                            );
                        }
                        Err(error) => return Err(error),
                    };
                let prepared = workspace
                    .prepared_tape
                    .row_view(prepared_index)
                    .ok_or_else(|| {
                        streaming_invariant_error(
                            *census,
                            "a retained prepared-row index escaped the flat tape",
                        )
                    })?;
                if is_excluded {
                    let skipped = if first_hit.is_some() {
                        discovery.try_skip_prepared_row_view_continuing(
                            &workspace.structural_scope,
                            prepared,
                        )
                    } else {
                        discovery.try_skip_prepared_row_view(&workspace.structural_scope, prepared)
                    };
                    skipped.map_err(|error| {
                        component_error(
                            SpiredTargetRunStage::Streaming,
                            *census,
                            SpiredTargetRunErrorCause::Streaming(error),
                        )
                    })?;
                    census.set_excluded_requests_skipped(checked_add(
                        "target-run excluded requests skipped",
                        census.excluded_requests_skipped(),
                        1,
                        *census,
                    )?);
                    prepared_row_ordinal =
                        prepared_row_ordinal.checked_add(1).ok_or_else(|| {
                            count_overflow_error(
                                PREPARED_TAPE_ROWS,
                                SpiredTargetRunStage::Streaming,
                                *census,
                            )
                        })?;
                    continue;
                }
                let Some((next_streamed, next_alternative)) = streamed_preflight else {
                    return Err(streaming_invariant_error(
                        *census,
                        "an admitted row lost its streamed-work preflight",
                    ));
                };
                let rows_before = discovery.rows_consumed();
                let was_post_hit = first_hit.is_some();
                let stream = discovery.try_consume_prepared_row_view_continuing(
                    &workspace.structural_scope,
                    prepared,
                );
                let stream = match stream {
                    Ok(stream) => stream,
                    Err(error) => {
                        let error = component_error(
                            SpiredTargetRunStage::Streaming,
                            *census,
                            SpiredTargetRunErrorCause::Streaming(error),
                        );
                        if best.is_some() && is_target_resource_stop(&error) {
                            return finish_branch_after_hit(
                                first_hit,
                                best,
                                last_rejection,
                                None,
                                *census,
                            );
                        }
                        return Err(error);
                    }
                };
                let consumed = discovery
                    .rows_consumed()
                    .checked_sub(rows_before)
                    .ok_or_else(|| {
                        streaming_invariant_error(
                            *census,
                            "streaming row count moved backwards within a branch",
                        )
                    })?;
                if consumed != 1 {
                    return Err(streaming_invariant_error(
                        *census,
                        "one admitted request did not consume exactly one modular row",
                    ));
                }
                census.set_streamed_rows(next_streamed);
                if let Some(next_alternative) = next_alternative {
                    census.set_alternative_streamed_rows(next_alternative);
                }
                if was_post_hit {
                    branch_post_hit_rows =
                        checked_add(POST_HIT_ROWS, branch_post_hit_rows, 1, *census)?;
                    census.set_post_hit_rows_streamed(checked_add(
                        POST_HIT_ROWS,
                        census.post_hit_rows_streamed(),
                        1,
                        *census,
                    )?);
                }
                prepared_row_ordinal = prepared_row_ordinal.checked_add(1).ok_or_else(|| {
                    count_overflow_error(
                        PREPARED_TAPE_ROWS,
                        SpiredTargetRunStage::Streaming,
                        *census,
                    )
                })?;
                match stream {
                    SpiredModularStreamOutcome::Pending => {}
                    SpiredModularStreamOutcome::FirstHit(hit) => {
                        if first_hit.is_some() {
                            return Err(streaming_invariant_error(
                                *census,
                                "a continued branch produced a second first-hit event",
                            ));
                        }
                        let shell_request_ordinal = chunk
                            .first_request_ordinal()
                            .checked_add(request_ordinal_in_chunk)
                            .ok_or_else(|| {
                                count_overflow_error(
                                    REQUESTS,
                                    SpiredTargetRunStage::Streaming,
                                    *census,
                                )
                            })?;
                        census.set_modular_hits_seen(checked_add(
                            "target-run modular hits",
                            census.modular_hits_seen(),
                            1,
                            *census,
                        )?);
                        census.record_hit(
                            depth,
                            chunk_ordinal,
                            shell_request_ordinal,
                            request_ordinal_in_chunk,
                            hit.support().len(),
                        );
                        first_hit_was_duplicate = rejected_supports
                            .iter()
                            .any(|support| support.as_ref() == hit.support());
                        let attempt = if first_hit_was_duplicate {
                            None
                        } else {
                            Some(try_attempt_branch_candidate(
                                workspace,
                                probe,
                                BranchCandidateProposal::First(&hit),
                                census,
                                limits,
                            )?)
                        };
                        first_hit = Some(hit);
                        match attempt {
                            None => {}
                            Some(BranchCandidateAttempt::Viable(candidate)) => {
                                try_retain_viable_candidate(&mut best, candidate, census)?;
                            }
                            Some(BranchCandidateAttempt::Rejected(rejection)) => {
                                last_rejection = Some(rejection);
                            }
                            Some(BranchCandidateAttempt::AttemptBudgetExhausted) => {
                                return Ok(BranchLaneOutcome::AttemptBudgetExhausted);
                            }
                        }
                        if limits.max_post_hit_streamed_rows == 0
                            || limits.streaming.modular.max_post_hit_rows == 0
                        {
                            return finish_branch_after_hit(
                                first_hit,
                                best,
                                last_rejection,
                                None,
                                *census,
                            );
                        }
                        if census.distinct_compact_lift_attempts()
                            >= limits.max_distinct_compact_lift_attempts
                        {
                            let exhaustion = best.is_none().then_some(
                                SpiredAlternativeExhaustion::DistinctCompactLiftAttempts,
                            );
                            return finish_branch_after_hit(
                                first_hit,
                                best,
                                last_rejection,
                                exhaustion,
                                *census,
                            );
                        }
                    }
                    SpiredModularStreamOutcome::PostHitCandidate(candidate) => {
                        if first_hit.is_none() {
                            return Err(streaming_invariant_error(
                                *census,
                                "a post-hit candidate preceded the branch's first target pivot",
                            ));
                        }
                        census.set_post_hit_candidates_seen(checked_add(
                            POST_HIT_CANDIDATES,
                            census.post_hit_candidates_seen(),
                            1,
                            *census,
                        )?);
                        let attempt = try_attempt_branch_candidate(
                            workspace,
                            probe,
                            BranchCandidateProposal::Rooted(&candidate),
                            census,
                            limits,
                        );
                        let attempt = match attempt {
                            Ok(attempt) => attempt,
                            Err(error) if best.is_some() && is_target_resource_stop(&error) => {
                                return finish_branch_after_hit(
                                    first_hit,
                                    best,
                                    last_rejection,
                                    None,
                                    *census,
                                );
                            }
                            Err(error) => return Err(error),
                        };
                        match attempt {
                            BranchCandidateAttempt::Viable(candidate) => {
                                try_retain_viable_candidate(&mut best, candidate, census)?;
                            }
                            BranchCandidateAttempt::Rejected(rejection) => {
                                if !first_hit_was_duplicate {
                                    last_rejection = Some(rejection);
                                }
                            }
                            BranchCandidateAttempt::AttemptBudgetExhausted => {
                                let exhaustion = best.is_none().then_some(
                                    SpiredAlternativeExhaustion::DistinctCompactLiftAttempts,
                                );
                                return finish_branch_after_hit(
                                    first_hit,
                                    best,
                                    last_rejection,
                                    exhaustion,
                                    *census,
                                );
                            }
                        }
                    }
                }
            }
        }

        if depth == limits.max_depth_inclusive {
            return if first_hit.is_some() {
                finish_branch_after_hit(first_hit, best, last_rejection, None, *census)
            } else {
                Ok(BranchLaneOutcome::DepthExhausted)
            };
        }
        depth = depth.checked_add(1).ok_or_else(|| {
            count_overflow_error(DEPTH, SpiredTargetRunStage::Scheduling, *census)
        })?;
    }
}

fn finish_branch_after_hit(
    first_hit: Option<SpiredModularHit>,
    best: Option<ExactRuleCellPromotionDisposition>,
    last_rejection: Option<AlternativeCandidateRejection>,
    exhaustion: Option<SpiredAlternativeExhaustion>,
    census: SpiredTargetRunCensus,
) -> Result<BranchLaneOutcome, SpiredTargetRunError> {
    if let Some(best) = best {
        return Ok(BranchLaneOutcome::RuleCell(best));
    }
    let first_hit = first_hit.ok_or_else(|| {
        invariant_error(
            census,
            "a post-hit portfolio finished before retaining its first modular hit",
        )
    })?;
    let Some(last) = last_rejection else {
        return Ok(BranchLaneOutcome::DuplicateHit(first_hit));
    };
    Ok(BranchLaneOutcome::Rejected {
        first_hit,
        last,
        exhaustion,
    })
}

fn is_target_resource_stop(error: &SpiredTargetRunError) -> bool {
    match error.cause() {
        SpiredTargetRunErrorCause::ResourceLimit { .. } => true,
        SpiredTargetRunErrorCause::Streaming(error) => is_streaming_resource_stop(error),
        SpiredTargetRunErrorCause::CompactLift(error) => match error {
            super::super::SpiredCompactLiftError::Stratum(
                crate::foundry::completion::stratum::StratumRegistryError::ResourceLimit { .. },
            )
            | super::super::SpiredCompactLiftError::Exact(
                crate::foundry::completion::frame::exact::ExactCircuitError::ResourceLimit {
                    ..
                },
            ) => true,
            super::super::SpiredCompactLiftError::Campaign(error) => {
                error.budget_exhaustion().is_some()
            }
            _ => false,
        },
        SpiredTargetRunErrorCause::RuleCellPromotion(error) => is_promotion_resource_stop(error),
        SpiredTargetRunErrorCause::Scheduling(error) => is_scheduling_resource_stop(error),
        SpiredTargetRunErrorCause::ResourceCountOverflow { .. }
        | SpiredTargetRunErrorCause::AllocationFailure { .. } => false,
    }
}

fn is_scheduling_resource_stop(error: &super::super::SpiredFoundationError) -> bool {
    matches!(
        error,
        super::super::SpiredFoundationError::ResourceLimit { .. }
    )
}

fn is_promotion_resource_stop(error: &super::super::SpiredRuleCellAuthorityError) -> bool {
    use crate::foundry::completion::frame::admission::ExactGuardRefinementError;
    use crate::foundry::completion::frame::exact::{
        ClearedCircuitError, ExactCircuitLoweringError,
    };
    use crate::foundry::completion::source_discovery::ExactRuleCellPromotionError;

    let error = match error {
        super::super::SpiredRuleCellAuthorityError::Anchor(error) => {
            return error.budget_exhaustion().is_some();
        }
        super::super::SpiredRuleCellAuthorityError::Promotion(error) => error,
    };
    match error {
        ExactRuleCellPromotionError::Partition(error) => error.budget_exhaustion().is_some(),
        ExactRuleCellPromotionError::GuardRefinement(
            ExactGuardRefinementError::ResourceLimit { .. },
        )
        | ExactRuleCellPromotionError::Clearing(ClearedCircuitError::ResourceLimit { .. })
        | ExactRuleCellPromotionError::Lowering(ExactCircuitLoweringError::ResourceLimit {
            ..
        })
        | ExactRuleCellPromotionError::Lowering(ExactCircuitLoweringError::Cell(
            crate::foundry::cell::RuleCellError::ResourceLimit { .. },
        ))
        | ExactRuleCellPromotionError::Cell(crate::foundry::cell::RuleCellError::ResourceLimit {
            ..
        }) => true,
        _ => false,
    }
}

fn is_streaming_resource_stop(error: &super::super::SpiredStreamingError) -> bool {
    match error {
        super::super::SpiredStreamingError::ResourceLimit { .. } => true,
        super::super::SpiredStreamingError::Evaluation(
            super::super::DirectShiftedSourceError::ResourceLimit { .. },
        ) => true,
        super::super::SpiredStreamingError::StructuralPreparation(error) => matches!(
            error,
            super::super::SpiredStructuralPreparationError::ResourceLimit { .. }
                | super::super::SpiredStructuralPreparationError::Evaluation(
                    super::super::DirectShiftedSourceError::ResourceLimit { .. }
                )
        ),
        super::super::SpiredStreamingError::Modular(
            super::super::SpiredModularError::ResourceLimit { .. },
        ) => true,
        _ => false,
    }
}

fn try_retain_rejected_support(
    rejected: &mut Vec<Box<[TranslatedSourceRequest]>>,
    support: &[TranslatedSourceRequest],
    arity: usize,
    census: &mut SpiredTargetRunCensus,
    limits: SpiredTargetRunLimits,
) -> Result<Option<SpiredAlternativeExhaustion>, SpiredTargetRunError> {
    // The same first-hit support can recur in a different exclusion branch.
    // Its exact rejection is reusable for storage identity, but the branch's
    // distinct reducer history may expose a new post-hit root, so callers only
    // arrive here after that continuation window has run. Avoid retaining the
    // support twice while preserving the bounded frontier expansion below.
    if rejected.iter().any(|retained| retained.as_ref() == support) {
        census.set_duplicate_rejected_support_hits(checked_add(
            "target-run duplicate rejected-support hits",
            census.duplicate_rejected_support_hits(),
            1,
            *census,
        )?);
        return Ok(None);
    }
    let next_requests = checked_add(
        REJECTED_SUPPORT_REQUESTS,
        census.retained_rejected_support_requests(),
        support.len(),
        *census,
    )?;
    if next_requests > limits.max_retained_rejected_support_requests {
        return Ok(Some(
            SpiredAlternativeExhaustion::RetainedRejectedSupportRequests,
        ));
    }
    let support_coordinates =
        checked_mul(REJECTED_SUPPORT_COORDINATES, support.len(), arity, *census)?;
    let next_coordinates = checked_add(
        REJECTED_SUPPORT_COORDINATES,
        census.retained_rejected_support_coordinate_cells(),
        support_coordinates,
        *census,
    )?;
    if next_coordinates > limits.max_retained_rejected_support_coordinate_cells {
        return Ok(Some(
            SpiredAlternativeExhaustion::RetainedRejectedSupportCoordinateCells,
        ));
    }
    rejected
        .try_reserve_exact(1)
        .map_err(|_| allocation_error(REJECTED_SUPPORT_REQUESTS, rejected.len() + 1, *census))?;
    let retained = try_clone_requests(support, REJECTED_SUPPORT_REQUESTS, *census)?;
    rejected.push(retained.into_boxed_slice());
    census.set_retained_rejected_support_requests(next_requests);
    census.set_retained_rejected_support_coordinate_cells(next_coordinates);
    Ok(None)
}

fn try_clone_requests(
    requests: &[TranslatedSourceRequest],
    resource: &'static str,
    census: SpiredTargetRunCensus,
) -> Result<Vec<TranslatedSourceRequest>, SpiredTargetRunError> {
    let mut cloned = Vec::new();
    cloned
        .try_reserve_exact(requests.len())
        .map_err(|_| allocation_error(resource, requests.len(), census))?;
    cloned.extend_from_slice(requests);
    Ok(cloned)
}

fn finish_inconclusive(
    census: SpiredTargetRunCensus,
    last: Option<AlternativeCandidateRejection>,
    exhaustion: SpiredAlternativeExhaustion,
) -> Result<SpiredTargetRunReport, SpiredTargetRunError> {
    let Some(last) = last else {
        return Err(invariant_error(
            census,
            "alternative-search exhaustion preceded its first exact-inconclusive support",
        ));
    };
    let outcome = match last {
        AlternativeCandidateRejection::Compact(last) => {
            SpiredTargetRunOutcome::CompactInconclusive { last, exhaustion }
        }
        AlternativeCandidateRejection::KnownZero(last) => {
            SpiredTargetRunOutcome::KnownZeroPromotionInconclusive { last, exhaustion }
        }
    };
    Ok(SpiredTargetRunReport::new(census, outcome))
}

fn exhausted_schedule_budget_error(
    arity: usize,
    residual_request_budget: usize,
    census: SpiredTargetRunCensus,
    limits: SpiredTargetRunLimits,
) -> SpiredTargetRunError {
    let (resource, requested, limit) = if residual_request_budget == 0 {
        let Some(requested) = census.scheduled_requests().checked_add(1) else {
            return count_overflow_error(REQUESTS, SpiredTargetRunStage::Scheduling, census);
        };
        (REQUESTS, requested, limits.max_total_scheduled_requests)
    } else {
        let Some(requested) = census
            .scheduled_request_coordinate_cells()
            .checked_add(arity)
        else {
            return count_overflow_error(
                REQUEST_COORDINATES,
                SpiredTargetRunStage::Scheduling,
                census,
            );
        };
        (
            REQUEST_COORDINATES,
            requested,
            limits.max_total_request_coordinate_cells,
        )
    };
    SpiredTargetRunError::new(
        SpiredTargetRunStage::Scheduling,
        census,
        SpiredTargetRunErrorCause::ResourceLimit {
            resource,
            requested,
            limit,
        },
    )
}

fn allocation_error(
    resource: &'static str,
    requested: usize,
    census: SpiredTargetRunCensus,
) -> SpiredTargetRunError {
    SpiredTargetRunError::new(
        SpiredTargetRunStage::Scheduling,
        census,
        SpiredTargetRunErrorCause::AllocationFailure {
            resource,
            requested,
        },
    )
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
    census: SpiredTargetRunCensus,
) -> Result<usize, SpiredTargetRunError> {
    left.checked_add(right)
        .ok_or_else(|| count_overflow_error(resource, SpiredTargetRunStage::Scheduling, census))
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
    census: SpiredTargetRunCensus,
) -> Result<usize, SpiredTargetRunError> {
    left.checked_mul(right)
        .ok_or_else(|| count_overflow_error(resource, SpiredTargetRunStage::Scheduling, census))
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
    census: SpiredTargetRunCensus,
) -> Result<(), SpiredTargetRunError> {
    if requested > limit {
        Err(SpiredTargetRunError::new(
            SpiredTargetRunStage::Scheduling,
            census,
            SpiredTargetRunErrorCause::ResourceLimit {
                resource,
                requested,
                limit,
            },
        ))
    } else {
        Ok(())
    }
}

fn count_overflow_error(
    resource: &'static str,
    stage: SpiredTargetRunStage,
    census: SpiredTargetRunCensus,
) -> SpiredTargetRunError {
    SpiredTargetRunError::new(
        stage,
        census,
        SpiredTargetRunErrorCause::ResourceCountOverflow { resource },
    )
}

fn prepared_tape_error(
    error: SpiredPreparedRowTapeError,
    census: SpiredTargetRunCensus,
) -> SpiredTargetRunError {
    let cause = match error {
        SpiredPreparedRowTapeError::ResourceCountOverflow { resource } => {
            SpiredTargetRunErrorCause::ResourceCountOverflow { resource }
        }
        SpiredPreparedRowTapeError::ResourceLimit {
            resource,
            requested,
            limit,
        } => SpiredTargetRunErrorCause::ResourceLimit {
            resource,
            requested,
            limit,
        },
        SpiredPreparedRowTapeError::AllocationFailure {
            resource,
            requested,
        } => SpiredTargetRunErrorCause::AllocationFailure {
            resource,
            requested,
        },
        SpiredPreparedRowTapeError::Invariant { detail } => {
            return streaming_invariant_error(census, detail);
        }
    };
    SpiredTargetRunError::new(SpiredTargetRunStage::Streaming, census, cause)
}

fn invariant_error(census: SpiredTargetRunCensus, detail: &'static str) -> SpiredTargetRunError {
    component_error(
        SpiredTargetRunStage::Scheduling,
        census,
        SpiredTargetRunErrorCause::Scheduling(super::super::SpiredFoundationError::Invariant {
            detail,
        }),
    )
}

fn streaming_invariant_error(
    census: SpiredTargetRunCensus,
    detail: &'static str,
) -> SpiredTargetRunError {
    component_error(
        SpiredTargetRunStage::Streaming,
        census,
        SpiredTargetRunErrorCause::Streaming(super::super::SpiredStreamingError::Invariant {
            detail,
        }),
    )
}

const fn component_error(
    stage: SpiredTargetRunStage,
    census: SpiredTargetRunCensus,
    cause: SpiredTargetRunErrorCause,
) -> SpiredTargetRunError {
    SpiredTargetRunError::new(stage, census, cause)
}

#[cfg(test)]
mod score_tests {
    use super::BranchCandidateQuality;

    fn quality(promotion_class: u8, branches: usize, sources: usize) -> BranchCandidateQuality {
        BranchCandidateQuality {
            promotion_class,
            new_guard_branches: branches,
            required_guard_predicates: branches,
            residual_terms: 2,
            source_combination_terms: sources,
            source_requests: sources,
        }
    }

    #[test]
    fn admissibility_then_guard_burden_then_exact_size_order_the_portfolio() {
        assert!(quality(0, 100, 100) < quality(1, 0, 1));
        assert!(quality(0, 1, 100) < quality(0, 2, 1));
        assert!(quality(0, 1, 2) < quality(0, 1, 3));
    }
}
