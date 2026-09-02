use std::cmp::Ordering;
use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::family::IntegralKey;
use crate::foundry::completion::LatticeBox;
use crate::foundry::completion::frame::admission::{
    ExactCircuitOuterExtensionWitness, ExactCircuitOwnerCover, ExactCircuitOwnerInput,
    ExactCircuitSemanticDag, ExactOwnerCoverSelection, compare_exact_circuit_content,
};
use crate::foundry::completion::guard::decision::GuardDecisionEvaluationLimits;

use super::super::{
    AdmittedExactRuleCandidate, CanonicalRebasedCandidate, CanonicalReplayBatch,
    ExactRuleCellPromotionDisposition, FreshTaskEpoch, try_promote_replayed_rule_cell_on_partition,
};
use super::layer::{try_build_owner_content_key, try_compare_owner_content_exact};
use super::{
    ExactExecutableCandidateObstruction, ExactExecutableOwnerCover, ExactExecutableOwnerError,
    ExactExecutableOwnerLimits, ExactExecutableOwnerObstruction, ExactExecutableOwnerProposal,
    ExactExecutableOwnerSelection, ExactSemanticExecutableOwner, UnpublishedCanonicalOwnerProposal,
};

const OWNER_CANDIDATES: &str = "semantic executable owner candidates";
const OWNER_GROUPS: &str = "semantic executable owner groups";
const PROMOTION_ATTEMPTS: &str = "exact owner promotion attempts";
const RETRY_SUPPORTS: &str = "exact owner retry supports";
const RETRY_ANCHOR_COORDINATES: &str = "exact owner retry anchor coordinate cells";
const PAIRING_PROBES: &str = "semantic executable owner pairing probes";

/// Promote every usable exact candidate on one shared authenticated partition,
/// retain normal guard obstructions, and compile only the globally executable
/// candidates into one semantic owner.
pub(crate) fn try_compile_canonical_executable_owner(
    context: &IndexedCoefficientContext,
    batch: CanonicalReplayBatch,
    limits: ExactExecutableOwnerLimits,
) -> Result<ExactExecutableOwnerProposal, ExactExecutableOwnerError> {
    if context.fingerprint() != batch.epoch().plan().context_fingerprint() {
        return Err(ExactExecutableOwnerError::WrongContext);
    }
    let candidate_count = batch.candidates().len();
    if candidate_count == 0 {
        return Err(ExactExecutableOwnerError::EmptyCandidates);
    }
    check_limit(
        OWNER_CANDIDATES,
        candidate_count,
        limits.max_candidates_per_owner,
    )?;

    let epoch = batch.epoch().clone();
    let partition = epoch.try_partition(limits.promotion.partition)?;
    let mut executable = try_vec(OWNER_CANDIDATES, candidate_count)?;
    let mut obstructions = try_vec(OWNER_CANDIDATES, candidate_count)?;
    let mut promotion_attempts = 0usize;

    for (candidate_ordinal, candidate) in batch.candidates().iter().enumerate() {
        match try_promote_candidate(
            context,
            &epoch,
            &partition,
            candidate,
            candidate_ordinal,
            limits,
            &mut promotion_attempts,
        )? {
            Ok(admitted) => {
                if let Some(split) = admitted.guard_domain_split() {
                    obstructions.push(ExactExecutableCandidateObstruction::new(
                        candidate_ordinal,
                        admitted.epoch().clone(),
                        admitted.circuit().clone(),
                        admitted.cleared().clone(),
                        ExactExecutableOwnerObstruction::ExceptionalGuardDomain {
                            refinement: admitted.guard_refinement().clone(),
                            split: split.clone(),
                        },
                    ));
                }
                executable.push(admitted);
            }
            Err(obstruction) => obstructions.push(obstruction),
        }
    }

    if executable.is_empty() {
        drop(partition);
        return Ok(ExactExecutableOwnerProposal::Incomplete(
            UnpublishedCanonicalOwnerProposal::new(batch, obstructions.into_boxed_slice()),
        ));
    }

    executable.sort_unstable_by(|left, right| {
        compare_exact_circuit_content(left.circuit(), right.circuit())
    });
    if executable.windows(2).any(|pair| {
        compare_exact_circuit_content(pair[0].circuit(), pair[1].circuit()) == Ordering::Equal
    }) {
        return Err(ExactExecutableOwnerError::PairingInvariant(
            "canonical replay emitted duplicate admitted exact content",
        ));
    }

    let mut circuits = try_vec(OWNER_CANDIDATES, executable.len())?;
    circuits.extend(
        executable
            .iter()
            .map(|candidate| (candidate.circuit().clone(), candidate.cleared().clone())),
    );
    let semantic = Arc::new(ExactCircuitSemanticDag::try_compile_cleared(
        context,
        &partition,
        &circuits,
        limits.semantic,
    )?);
    validate_candidate_pairing(&semantic, &executable)?;
    drop(partition);
    let content_order_key = try_build_owner_content_key(
        &epoch,
        &semantic,
        &executable,
        limits.max_owner_encoded_content_bytes,
    )?;

    Ok(ExactExecutableOwnerProposal::Compiled {
        owner: Arc::new(ExactSemanticExecutableOwner {
            epoch,
            semantic,
            executable: executable.into_boxed_slice(),
            content_order_key,
        }),
        obstructions: obstructions.into_boxed_slice(),
    })
}

impl ExactExecutableOwnerCover {
    /// Compile a complete immutable owner set. Input order cannot affect owner
    /// IDs: the proof cover supplies the canonical order and a pointer-only
    /// cold join recovers the already-owned executable groups.
    pub(crate) fn try_compile(
        context: &IndexedCoefficientContext,
        owners: Vec<Arc<ExactSemanticExecutableOwner>>,
        terminals: Vec<IntegralKey>,
        limits: ExactExecutableOwnerLimits,
    ) -> Result<Self, ExactExecutableOwnerError> {
        Self::try_compile_in_carrier(context, owners, terminals, None, limits)
    }

    /// Compile a diagnostic preview relative to an explicit supported-root
    /// carrier. The proof cover retains that carrier as part of its scope.
    pub(crate) fn try_compile_with_carrier(
        context: &IndexedCoefficientContext,
        owners: Vec<Arc<ExactSemanticExecutableOwner>>,
        terminals: Vec<IntegralKey>,
        carrier: &LatticeBox,
        limits: ExactExecutableOwnerLimits,
    ) -> Result<Self, ExactExecutableOwnerError> {
        Self::try_compile_in_carrier(context, owners, terminals, Some(carrier), limits)
    }

    fn try_compile_in_carrier(
        context: &IndexedCoefficientContext,
        owners: Vec<Arc<ExactSemanticExecutableOwner>>,
        terminals: Vec<IntegralKey>,
        carrier: Option<&LatticeBox>,
        limits: ExactExecutableOwnerLimits,
    ) -> Result<Self, ExactExecutableOwnerError> {
        if owners.is_empty() {
            return Err(ExactExecutableOwnerError::EmptyOwners);
        }
        check_limit(OWNER_GROUPS, owners.len(), limits.max_owners)?;
        let proof_cover = compile_proof_cover(context, &owners, &terminals, carrier, limits)?;
        let permutation = try_pairing_permutation(&proof_cover, &owners, limits)?;
        let mut ordered = try_vec(OWNER_GROUPS, owners.len())?;
        for source in permutation {
            ordered.push(owners[source].clone());
        }
        validate_cover_pairing(&proof_cover, &ordered)?;
        let mut canonical_terminals = try_vec(
            "semantic executable explicit terminals",
            proof_cover.terminals().len(),
        )?;
        canonical_terminals.extend(
            proof_cover
                .terminals()
                .iter()
                .map(|terminal| terminal.integral().clone()),
        );
        Ok(Self {
            owners: ordered.into_boxed_slice(),
            terminals: canonical_terminals.into_boxed_slice(),
            cover: proof_cover,
        })
    }

    /// Rebuild the entire proof cover with one additional immutable owner and
    /// publish it only after every join and sidecar check succeeds.
    pub(crate) fn try_insert(
        &mut self,
        context: &IndexedCoefficientContext,
        owner: Arc<ExactSemanticExecutableOwner>,
        limits: ExactExecutableOwnerLimits,
    ) -> Result<bool, ExactExecutableOwnerError> {
        if context.fingerprint() != self.cover.context_fingerprint() {
            return Err(ExactExecutableOwnerError::WrongContext);
        }
        let retained_predecessor = self
            .owners
            .first()
            .ok_or(ExactExecutableOwnerError::EmptyOwners)?
            .epoch()
            .predecessor_snapshot();
        if !owner
            .epoch()
            .predecessor_snapshot()
            .same_authority_as(retained_predecessor)
        {
            return Err(ExactExecutableOwnerError::AuthorityMismatch {
                candidate: 0,
                detail: "inserted owner uses a structurally equal but independently installed predecessor authority",
            });
        }
        let proof_equivalent = self
            .owners
            .iter()
            .position(|existing| compare_exact_owner_proof_content(existing, &owner).is_eq());
        if let Some(ordinal) = proof_equivalent {
            if !compare_exact_owner_group_content(&owner, &self.owners[ordinal])?.is_lt() {
                return Ok(false);
            }
        }
        let requested = if proof_equivalent.is_some() {
            self.owners.len()
        } else {
            checked_add(OWNER_GROUPS, self.owners.len(), 1)?
        };
        check_limit(OWNER_GROUPS, requested, limits.max_owners)?;
        let mut owners = try_vec(OWNER_GROUPS, requested)?;
        owners.extend(self.owners.iter().cloned());
        match proof_equivalent {
            Some(ordinal) => owners[ordinal] = owner,
            None => owners.push(owner),
        }
        let mut terminals = try_vec(
            "semantic executable explicit terminals",
            self.terminals.len(),
        )?;
        terminals.extend(self.terminals.iter().cloned());
        // Recompilation must preserve the exact universe against which this
        // cover was proved. Falling back to the full machine carrier here can
        // manufacture a representability-fringe obligation and silently
        // change a bounded publication transaction into a different claim.
        let replacement = Self::try_compile_with_carrier(
            context,
            owners,
            terminals,
            self.cover.closure_carrier(),
            limits,
        )?;
        *self = replacement;
        Ok(true)
    }

    /// Route through the exact proof cover and recover the already-paired
    /// executable cell without any topology or source rediscovery.
    pub(crate) fn try_select_at(
        &self,
        context: &IndexedCoefficientContext,
        target: &IntegralKey,
        limits: GuardDecisionEvaluationLimits,
    ) -> Result<ExactExecutableOwnerSelection<'_>, ExactExecutableOwnerError> {
        match self.cover.try_select_at(context, target, limits)? {
            ExactOwnerCoverSelection::Descending { owner, candidate } => {
                let owner_ordinal = owner.id().ordinal();
                let candidate_ordinal = candidate.id().ordinal();
                let group = self.owners.get(owner_ordinal).ok_or(
                    ExactExecutableOwnerError::PairingInvariant(
                        "proof owner selected an absent executable group",
                    ),
                )?;
                if !Arc::ptr_eq(owner.semantic(), group.semantic()) {
                    return Err(ExactExecutableOwnerError::PairingInvariant(
                        "proof owner and executable group semantic identities differ",
                    ));
                }
                let executable = group.executable_candidates().get(candidate_ordinal).ok_or(
                    ExactExecutableOwnerError::PairingInvariant(
                        "semantic candidate selected an absent executable cell",
                    ),
                )?;
                if !Arc::ptr_eq(candidate.circuit(), executable.circuit()) {
                    return Err(ExactExecutableOwnerError::PairingInvariant(
                        "semantic candidate and executable cell circuit identities differ",
                    ));
                }
                if executable.cell().assignment_for_target(target)?.is_none() {
                    return Err(ExactExecutableOwnerError::PairingInvariant(
                        "selected executable cell does not contain the target assignment",
                    ));
                }
                Ok(ExactExecutableOwnerSelection::Descending {
                    owner_ordinal,
                    candidate_ordinal,
                    circuit: executable.circuit(),
                    cell: executable.cell(),
                })
            }
            ExactOwnerCoverSelection::Terminal(terminal) => {
                Ok(ExactExecutableOwnerSelection::Terminal(terminal.integral()))
            }
            ExactOwnerCoverSelection::Incomplete => Ok(ExactExecutableOwnerSelection::Incomplete),
        }
    }
}

pub(crate) fn compare_exact_owner_group_content(
    left: &ExactSemanticExecutableOwner,
    right: &ExactSemanticExecutableOwner,
) -> Result<Ordering, crate::foundry::completion::stratum::StratumRegistryError> {
    let compact = left.content_order_key().cmp(&right.content_order_key());
    if compact.is_eq() {
        try_compare_owner_content_exact(left, right)
    } else {
        Ok(compact)
    }
}

/// Compare only the exact proof-cover identity of two executable groups.
/// Distinct full content within one equal class is resolved by canonical-min
/// replacement; retaining both would be rejected by the proof compiler.
pub(crate) fn compare_exact_owner_proof_content(
    left: &ExactSemanticExecutableOwner,
    right: &ExactSemanticExecutableOwner,
) -> Ordering {
    left.epoch()
        .plan()
        .family_fingerprint()
        .cmp(right.epoch().plan().family_fingerprint())
        .then_with(|| {
            left.epoch()
                .plan()
                .context_fingerprint()
                .cmp(right.epoch().plan().context_fingerprint())
        })
        .then_with(|| {
            left.epoch()
                .plan()
                .sector()
                .cmp(right.epoch().plan().sector())
        })
        .then_with(|| {
            left.epoch()
                .fixed_ordering()
                .cmp(&right.epoch().fixed_ordering())
        })
        .then_with(|| {
            left.epoch()
                .fixed_snapshot_id()
                .as_str()
                .cmp(right.epoch().fixed_snapshot_id().as_str())
        })
        .then_with(|| {
            left.epoch()
                .target_shift()
                .cmp(right.epoch().target_shift())
        })
        .then_with(|| {
            for (left, right) in left
                .semantic()
                .candidates()
                .iter()
                .zip(right.semantic().candidates())
            {
                let ordering = compare_exact_circuit_content(left.circuit(), right.circuit());
                if !ordering.is_eq() {
                    return ordering;
                }
            }
            left.semantic()
                .candidates()
                .len()
                .cmp(&right.semantic().candidates().len())
        })
}

fn try_promote_candidate(
    context: &IndexedCoefficientContext,
    epoch: &Arc<super::super::FreshTaskEpoch>,
    partition: &crate::foundry::completion::stratum::TargetColumnPartition<'_>,
    candidate: &CanonicalRebasedCandidate,
    candidate_ordinal: usize,
    limits: ExactExecutableOwnerLimits,
    attempts: &mut usize,
) -> Result<
    Result<AdmittedExactRuleCandidate, ExactExecutableCandidateObstruction>,
    ExactExecutableOwnerError,
> {
    let retained_circuit = candidate.circuit().clone();
    let mut outcome = promote_at_anchor(
        context,
        epoch,
        partition,
        &retained_circuit,
        candidate.anchor(),
        candidate_ordinal,
        limits,
        attempts,
    )?;
    let mut retry_anchors = None;

    loop {
        validate_disposition_authority(candidate_ordinal, epoch, &retained_circuit, &outcome)?;
        match outcome {
            ExactRuleCellPromotionDisposition::Admitted(admitted) => return Ok(Ok(admitted)),
            ExactRuleCellPromotionDisposition::BlockedByKnownZero {
                epoch,
                circuit,
                cleared,
                required_predicate_ordinal,
                first_circuit_guard_ordinal,
                zero_branch,
            } => {
                return Ok(Err(ExactExecutableCandidateObstruction::new(
                    candidate_ordinal,
                    epoch,
                    circuit,
                    cleared,
                    ExactExecutableOwnerObstruction::BlockedByKnownZero {
                        required_predicate_ordinal,
                        first_circuit_guard_ordinal,
                        zero_branch,
                    },
                )));
            }
            ExactRuleCellPromotionDisposition::NeedsGuardedStratum {
                epoch,
                circuit,
                cleared,
                refinement,
                obstruction,
            } => {
                return Ok(Err(ExactExecutableCandidateObstruction::new(
                    candidate_ordinal,
                    epoch,
                    circuit,
                    cleared,
                    ExactExecutableOwnerObstruction::NeedsGuardedStratum {
                        refinement,
                        obstruction,
                    },
                )));
            }
            ExactRuleCellPromotionDisposition::AnchorOnGuardWall {
                epoch: wall_epoch,
                circuit: wall_circuit,
                cleared: wall_cleared,
                refinement,
                guard_ordinal,
            } => {
                let retry_anchors = match retry_anchors.as_mut() {
                    Some(anchors) => anchors,
                    None => retry_anchors
                        .insert(try_canonical_retry_anchors(epoch, candidate, limits)?.into_iter()),
                };
                let Some(anchor) = retry_anchors.next() else {
                    return Ok(Err(ExactExecutableCandidateObstruction::new(
                        candidate_ordinal,
                        wall_epoch,
                        wall_circuit,
                        wall_cleared,
                        ExactExecutableOwnerObstruction::AnchorOnGuardWall {
                            refinement,
                            guard_ordinal,
                        },
                    )));
                };
                outcome = promote_at_anchor(
                    context,
                    epoch,
                    partition,
                    &retained_circuit,
                    &anchor,
                    candidate_ordinal,
                    limits,
                    attempts,
                )?;
            }
        }
    }
}

/// Derive retry authority only from exact chart anchors. Supporting probes are
/// diagnostics: their modulus/base ordering must neither duplicate work nor
/// choose which concrete anchor owns the promoted cell.
pub(super) fn try_canonical_retry_anchors(
    epoch: &FreshTaskEpoch,
    candidate: &CanonicalRebasedCandidate,
    limits: ExactExecutableOwnerLimits,
) -> Result<Vec<Box<[i64]>>, ExactExecutableOwnerError> {
    let support_count = candidate.supporting_probes().len();
    check_limit(RETRY_SUPPORTS, support_count, limits.max_retry_supports)?;
    let coordinate_cells = checked_mul(
        RETRY_ANCHOR_COORDINATES,
        support_count,
        epoch.fixed_stratum().domain().arity(),
    )?;
    check_limit(
        RETRY_ANCHOR_COORDINATES,
        coordinate_cells,
        limits.max_retry_anchor_coordinate_cells,
    )?;
    let mut anchors = try_vec(RETRY_SUPPORTS, support_count)?;
    for probe in candidate.supporting_probes() {
        let anchor = epoch.try_anchor_for_probe(probe)?;
        if anchor.as_ref() != candidate.anchor() {
            anchors.push(anchor);
        }
    }
    anchors.sort_unstable();
    anchors.dedup();
    Ok(anchors)
}

#[allow(clippy::too_many_arguments)]
fn promote_at_anchor(
    context: &IndexedCoefficientContext,
    epoch: &Arc<super::super::FreshTaskEpoch>,
    partition: &crate::foundry::completion::stratum::TargetColumnPartition<'_>,
    circuit: &Arc<crate::foundry::completion::frame::exact::ExactTargetCircuit>,
    anchor: &[i64],
    candidate: usize,
    limits: ExactExecutableOwnerLimits,
    attempts: &mut usize,
) -> Result<ExactRuleCellPromotionDisposition, ExactExecutableOwnerError> {
    *attempts = checked_add(PROMOTION_ATTEMPTS, *attempts, 1)?;
    check_limit(PROMOTION_ATTEMPTS, *attempts, limits.max_promotion_attempts)?;
    try_promote_replayed_rule_cell_on_partition(
        context,
        epoch.clone(),
        circuit.clone(),
        anchor,
        partition,
        limits.promotion,
    )
    .map_err(|error| ExactExecutableOwnerError::Promotion { candidate, error })
}

fn validate_disposition_authority(
    candidate: usize,
    epoch: &Arc<super::super::FreshTaskEpoch>,
    circuit: &Arc<crate::foundry::completion::frame::exact::ExactTargetCircuit>,
    disposition: &ExactRuleCellPromotionDisposition,
) -> Result<(), ExactExecutableOwnerError> {
    let (actual_epoch, actual_circuit) = match disposition {
        ExactRuleCellPromotionDisposition::Admitted(admitted) => {
            (admitted.epoch(), admitted.circuit())
        }
        ExactRuleCellPromotionDisposition::BlockedByKnownZero { epoch, circuit, .. }
        | ExactRuleCellPromotionDisposition::NeedsGuardedStratum { epoch, circuit, .. }
        | ExactRuleCellPromotionDisposition::AnchorOnGuardWall { epoch, circuit, .. } => {
            (epoch, circuit)
        }
    };
    if !Arc::ptr_eq(epoch, actual_epoch) {
        return Err(ExactExecutableOwnerError::AuthorityMismatch {
            candidate,
            detail: "promotion returned another epoch",
        });
    }
    if !Arc::ptr_eq(circuit, actual_circuit) {
        return Err(ExactExecutableOwnerError::AuthorityMismatch {
            candidate,
            detail: "promotion returned another exact circuit",
        });
    }
    Ok(())
}

fn validate_candidate_pairing(
    semantic: &ExactCircuitSemanticDag,
    executable: &[AdmittedExactRuleCandidate],
) -> Result<(), ExactExecutableOwnerError> {
    if semantic.candidates().len() != executable.len() {
        return Err(ExactExecutableOwnerError::PairingInvariant(
            "semantic and executable candidate counts differ",
        ));
    }
    for (semantic, executable) in semantic.candidates().iter().zip(executable) {
        if !Arc::ptr_eq(semantic.circuit(), executable.circuit()) {
            return Err(ExactExecutableOwnerError::PairingInvariant(
                "semantic sorting detached an executable cell from its exact circuit",
            ));
        }
    }
    Ok(())
}

fn compile_proof_cover(
    context: &IndexedCoefficientContext,
    owners: &[Arc<ExactSemanticExecutableOwner>],
    terminals: &[IntegralKey],
    carrier: Option<&LatticeBox>,
    limits: ExactExecutableOwnerLimits,
) -> Result<ExactCircuitOwnerCover, ExactExecutableOwnerError> {
    let mut partitions = try_vec(OWNER_GROUPS, owners.len())?;
    for owner in owners {
        partitions.push(owner.epoch().try_partition(limits.promotion.partition)?);
    }
    let mut extensions = try_vec(OWNER_GROUPS, owners.len())?;
    for (owner_ordinal, (owner, partition)) in owners.iter().zip(&partitions).enumerate() {
        extensions.push(
            ExactCircuitOuterExtensionWitness::try_prove(partition, owner.semantic().clone())
                .map_err(|error| ExactExecutableOwnerError::OuterExtension {
                    owner: owner_ordinal,
                    error,
                })?,
        );
    }
    let mut inputs = try_vec(OWNER_GROUPS, owners.len())?;
    for (partition, extension) in partitions.iter().zip(extensions) {
        inputs.push(ExactCircuitOwnerInput::new(partition, extension));
    }
    Ok(match carrier {
        Some(carrier) => ExactCircuitOwnerCover::try_compile_with_carrier(
            context,
            inputs,
            terminals.iter().cloned(),
            carrier,
            limits.cover,
        )?,
        None => ExactCircuitOwnerCover::try_compile(
            context,
            inputs,
            terminals.iter().cloned(),
            limits.cover,
        )?,
    })
}

fn try_pairing_permutation(
    cover: &ExactCircuitOwnerCover,
    owners: &[Arc<ExactSemanticExecutableOwner>],
    limits: ExactExecutableOwnerLimits,
) -> Result<Vec<usize>, ExactExecutableOwnerError> {
    let mut probes = 0usize;
    let mut permutation = try_vec(OWNER_GROUPS, owners.len())?;
    let mut used = try_vec(OWNER_GROUPS, owners.len())?;
    used.resize(owners.len(), false);
    for proof_owner in cover.owners() {
        let mut found = None;
        for (ordinal, owner) in owners.iter().enumerate() {
            probes = checked_add(PAIRING_PROBES, probes, 1)?;
            check_limit(PAIRING_PROBES, probes, limits.max_pairing_probes)?;
            if Arc::ptr_eq(proof_owner.semantic(), owner.semantic()) {
                if found.replace(ordinal).is_some() {
                    return Err(ExactExecutableOwnerError::PairingInvariant(
                        "one proof owner matches multiple executable groups",
                    ));
                }
            }
        }
        let source = found.ok_or(ExactExecutableOwnerError::PairingInvariant(
            "one proof owner has no executable group",
        ))?;
        if used[source] {
            return Err(ExactExecutableOwnerError::PairingInvariant(
                "one executable group was assigned to multiple proof owners",
            ));
        }
        used[source] = true;
        permutation.push(source);
    }
    if permutation.len() != owners.len() || used.iter().any(|used| !used) {
        return Err(ExactExecutableOwnerError::PairingInvariant(
            "proof/executable owner cardinalities differ",
        ));
    }
    Ok(permutation)
}

fn validate_cover_pairing(
    cover: &ExactCircuitOwnerCover,
    owners: &[Arc<ExactSemanticExecutableOwner>],
) -> Result<(), ExactExecutableOwnerError> {
    if cover.owners().len() != owners.len() {
        return Err(ExactExecutableOwnerError::PairingInvariant(
            "proof and executable owner counts differ after sorting",
        ));
    }
    for (proof, owner) in cover.owners().iter().zip(owners) {
        if !Arc::ptr_eq(proof.semantic(), owner.semantic()) {
            return Err(ExactExecutableOwnerError::PairingInvariant(
                "canonical owner sorting lost semantic/executable identity",
            ));
        }
        validate_candidate_pairing(owner.semantic(), owner.executable_candidates())?;
    }
    Ok(())
}

fn try_vec<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, ExactExecutableOwnerError> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| {
        ExactExecutableOwnerError::AllocationFailure {
            resource,
            requested: capacity,
        }
    })?;
    Ok(values)
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ExactExecutableOwnerError> {
    left.checked_add(right)
        .ok_or(ExactExecutableOwnerError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ExactExecutableOwnerError> {
    left.checked_mul(right)
        .ok_or(ExactExecutableOwnerError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ExactExecutableOwnerError> {
    if requested > limit {
        Err(ExactExecutableOwnerError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
