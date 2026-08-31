use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::family::IntegralKey;
use crate::foundry::completion::frame::admission::{
    ExactOwnerCoverObstructionKind, ExactOwnerCoverStatus,
};
use crate::foundry::completion::stratum::ImmutableOwnerSnapshot;
use crate::sector::{Mask, OrderingPolicy};

use super::super::{
    ClosedExactExecutableOwnerCover, ClosedSectorLayer, ExactExecutableOwnerCover,
    ExactExecutableOwnerLimits, ExactSemanticExecutableOwner, compare_exact_owner_group_content,
    compare_exact_owner_proof_content,
};
use super::model::StagedSectorKey;
use super::{
    ClosedSectorClosureWave, StagedSectorClosureError, StagedSectorClosureLimits,
    StagedSectorClosureOutcome, StagedSectorClosureStop, StagedSectorClosureStopEvidence,
};

const STAGED_SECTORS: &str = "staged sector-closure frontier sectors";
const STAGED_OWNERS: &str = "staged sector-closure executable owners";
const STAGED_TERMINALS: &str = "staged sector-closure explicit terminals";
const FRONTIER_COORDINATES: &str = "staged sector-closure frontier coordinate cells";
const OWNER_COORDINATES: &str = "staged sector-closure owner coordinate cells";
const OWNER_CANDIDATES: &str = "staged sector-closure owner candidate slots";
const OWNER_CONTENT_BYTES: &str = "staged sector-closure owner canonical content bytes";
const OWNER_COMPARISONS: &str = "staged sector-closure owner order comparisons";
const TERMINAL_COORDINATES: &str = "staged sector-closure terminal coordinate cells";
const COMPILED_PAIRING_PROBES: &str = "staged sector-closure compiled pairing probes";
const COMPILED_FINITE_POINTS: &str = "staged sector-closure finite complement points";
const COMPILED_FINITE_COORDINATES: &str =
    "staged sector-closure finite-complement coordinate cells";
const COMPILED_POINT_PROBES: &str = "staged sector-closure point-owner probes";
const COMPILED_UNCOVERED_BOXES: &str = "staged sector-closure compiled uncovered boxes";
const COMPILED_UNCOVERED_COORDINATES: &str =
    "staged sector-closure compiled uncovered-box coordinate cells";
const COMPILED_SPLITS: &str = "staged sector-closure cover split operations";

#[derive(Debug)]
struct StagedSector {
    key: StagedSectorKey,
    owners: Vec<Arc<ExactSemanticExecutableOwner>>,
    terminals: Vec<IntegralKey>,
}

/// Topology-neutral coordinator for one complete same-active-count wave.
///
/// All stages retain the same predecessor authority. Input order is erased at
/// insertion: sectors, executable owner groups, and terminals are kept in
/// their exact canonical orders. `try_finish` first compiles every cover and
/// returns all normal stops; it seals and publishes layers only when every
/// cover is exactly `Closed`.
#[derive(Debug)]
pub(crate) struct StagedSectorClosureCoordinator {
    context: IndexedCoefficientContext,
    predecessor: ImmutableOwnerSnapshot,
    stages: Vec<StagedSector>,
    frontier_coordinate_cells: usize,
    owner_count: usize,
    owner_coordinate_cells: usize,
    owner_candidate_slots: usize,
    owner_content_order_bytes: usize,
    owner_order_comparisons: usize,
    terminal_count: usize,
    terminal_coordinate_cells: usize,
    limits: StagedSectorClosureLimits,
}

impl StagedSectorClosureCoordinator {
    pub(crate) fn try_new(
        context: &IndexedCoefficientContext,
        predecessor: ImmutableOwnerSnapshot,
        frontier: impl IntoIterator<Item = (Mask, OrderingPolicy)>,
        limits: StagedSectorClosureLimits,
    ) -> Result<Self, StagedSectorClosureError> {
        if context.fingerprint() != predecessor.context_fingerprint() {
            return Err(StagedSectorClosureError::WrongPredecessorContext);
        }
        if !predecessor.try_verify(limits.registry)? {
            return Err(StagedSectorClosureError::InvalidPredecessor);
        }

        let mut stages = Vec::new();
        let mut expected_active_count = None;
        let mut frontier_coordinate_cells = 0usize;
        for (sector_ordinal, (sector, ordering)) in frontier.into_iter().enumerate() {
            let requested = checked_add(STAGED_SECTORS, stages.len(), 1)?;
            check_limit(STAGED_SECTORS, requested, limits.max_sectors)?;
            if sector.arity() != predecessor.arity() {
                return Err(StagedSectorClosureError::WrongSectorArity {
                    sector: sector_ordinal,
                    expected: predecessor.arity(),
                    actual: sector.arity(),
                });
            }
            frontier_coordinate_cells = checked_add(
                FRONTIER_COORDINATES,
                frontier_coordinate_cells,
                sector.arity(),
            )?;
            check_limit(
                FRONTIER_COORDINATES,
                frontier_coordinate_cells,
                limits.max_frontier_coordinate_cells,
            )?;
            let active_count = sector.active_count();
            if let Some(expected) = expected_active_count {
                if active_count != expected {
                    return Err(StagedSectorClosureError::MixedFrontierActiveCount {
                        sector: sector_ordinal,
                        expected,
                        actual: active_count,
                    });
                }
            } else {
                expected_active_count = Some(active_count);
            }
            stages
                .try_reserve(1)
                .map_err(|_| StagedSectorClosureError::AllocationFailure {
                    resource: STAGED_SECTORS,
                    requested,
                })?;
            stages.push(StagedSector {
                key: StagedSectorKey { sector, ordering },
                owners: Vec::new(),
                terminals: Vec::new(),
            });
        }
        if stages.is_empty() {
            return Err(StagedSectorClosureError::EmptyFrontier);
        }
        stages.sort_unstable_by(|left, right| left.key.cmp(&right.key));
        if stages.windows(2).any(|pair| pair[0].key == pair[1].key) {
            return Err(StagedSectorClosureError::DuplicateSector);
        }
        Ok(Self {
            context: context.clone(),
            predecessor,
            stages,
            frontier_coordinate_cells,
            owner_count: 0,
            owner_coordinate_cells: 0,
            owner_candidate_slots: 0,
            owner_content_order_bytes: 0,
            owner_order_comparisons: 0,
            terminal_count: 0,
            terminal_coordinate_cells: 0,
            limits,
        })
    }

    pub(crate) const fn predecessor_snapshot(&self) -> &ImmutableOwnerSnapshot {
        &self.predecessor
    }

    pub(crate) fn sector_count(&self) -> usize {
        self.stages.len()
    }

    pub(crate) const fn frontier_coordinate_cells(&self) -> usize {
        self.frontier_coordinate_cells
    }

    pub(crate) const fn owner_count(&self) -> usize {
        self.owner_count
    }

    pub(crate) const fn owner_coordinate_cells(&self) -> usize {
        self.owner_coordinate_cells
    }

    pub(crate) const fn owner_candidate_slots(&self) -> usize {
        self.owner_candidate_slots
    }

    pub(crate) const fn owner_content_order_bytes(&self) -> usize {
        self.owner_content_order_bytes
    }

    pub(crate) const fn owner_order_comparisons(&self) -> usize {
        self.owner_order_comparisons
    }

    pub(crate) const fn terminal_count(&self) -> usize {
        self.terminal_count
    }

    pub(crate) const fn terminal_coordinate_cells(&self) -> usize {
        self.terminal_coordinate_cells
    }

    /// Insert one already pointer-paired canonical executable owner. A
    /// content-equal duplicate is ignored without replacing retained pointer
    /// authority.
    pub(crate) fn try_insert_owner(
        &mut self,
        owner: Arc<ExactSemanticExecutableOwner>,
    ) -> Result<bool, StagedSectorClosureError> {
        self.validate_owner_scope(&owner)?;
        let key = StagedSectorKey {
            sector: owner.epoch().plan().sector().clone(),
            ordering: owner.epoch().fixed_ordering(),
        };
        let stage_ordinal = self.stage_ordinal(&key)?;
        let insertion = match self.proof_owner_position(stage_ordinal, &owner)? {
            Ok(existing) => {
                return self.try_replace_proof_equivalent(stage_ordinal, existing, owner);
            }
            Err(insertion) => insertion,
        };
        let requested = checked_add(STAGED_OWNERS, self.owner_count, 1)?;
        check_limit(STAGED_OWNERS, requested, self.limits.max_staged_owners)?;
        let owner_coordinate_cells = checked_add(
            OWNER_COORDINATES,
            self.owner_coordinate_cells,
            self.predecessor.arity(),
        )?;
        check_limit(
            OWNER_COORDINATES,
            owner_coordinate_cells,
            self.limits.max_staged_owner_coordinate_cells,
        )?;
        let owner_candidate_slots = checked_add(
            OWNER_CANDIDATES,
            self.owner_candidate_slots,
            owner.executable_candidates().len(),
        )?;
        check_limit(
            OWNER_CANDIDATES,
            owner_candidate_slots,
            self.limits.max_staged_owner_candidate_slots,
        )?;
        let owner_content_order_bytes = checked_add(
            OWNER_CONTENT_BYTES,
            self.owner_content_order_bytes,
            owner.content_order_key().len(),
        )?;
        check_limit(
            OWNER_CONTENT_BYTES,
            owner_content_order_bytes,
            self.limits.max_staged_owner_content_order_bytes,
        )?;
        let stage = &mut self.stages[stage_ordinal];
        stage
            .owners
            .try_reserve(1)
            .map_err(|_| StagedSectorClosureError::AllocationFailure {
                resource: STAGED_OWNERS,
                requested,
            })?;
        stage.owners.insert(insertion, owner);
        self.owner_count = requested;
        self.owner_coordinate_cells = owner_coordinate_cells;
        self.owner_candidate_slots = owner_candidate_slots;
        self.owner_content_order_bytes = owner_content_order_bytes;
        Ok(true)
    }

    /// Binary-search the exact proof-cover key. Each stage retains at most one
    /// canonical executable representative per proof class, so insertion and
    /// worker-result merging stay logarithmic in the number of owners.
    fn proof_owner_position(
        &mut self,
        stage_ordinal: usize,
        owner: &ExactSemanticExecutableOwner,
    ) -> Result<Result<usize, usize>, StagedSectorClosureError> {
        let mut left = 0usize;
        let mut right = self.stages[stage_ordinal].owners.len();
        while left < right {
            let middle = left + (right - left) / 2;
            self.charge_owner_comparison()?;
            match compare_exact_owner_proof_content(
                &self.stages[stage_ordinal].owners[middle],
                owner,
            ) {
                std::cmp::Ordering::Less => left = middle + 1,
                std::cmp::Ordering::Greater => right = middle,
                std::cmp::Ordering::Equal => return Ok(Ok(middle)),
            }
        }
        Ok(Err(left))
    }

    /// Canonical-min replacement is required when two worker arrivals prove
    /// the same cover owner but retain different executable replay anchors or
    /// guard refinements. Retaining both would be a proof-cover duplicate;
    /// retaining the first would make arrival order observable.
    fn try_replace_proof_equivalent(
        &mut self,
        stage_ordinal: usize,
        existing_ordinal: usize,
        owner: Arc<ExactSemanticExecutableOwner>,
    ) -> Result<bool, StagedSectorClosureError> {
        self.charge_owner_comparison()?;
        if !compare_exact_owner_group_content(
            &owner,
            &self.stages[stage_ordinal].owners[existing_ordinal],
        )
        .is_lt()
        {
            return Ok(false);
        }
        let existing_candidates = self.stages[stage_ordinal].owners[existing_ordinal]
            .executable_candidates()
            .len();
        let existing_bytes = self.stages[stage_ordinal].owners[existing_ordinal]
            .content_order_key()
            .len();
        let owner_candidate_slots = checked_replace(
            OWNER_CANDIDATES,
            self.owner_candidate_slots,
            existing_candidates,
            owner.executable_candidates().len(),
        )?;
        check_limit(
            OWNER_CANDIDATES,
            owner_candidate_slots,
            self.limits.max_staged_owner_candidate_slots,
        )?;
        let owner_content_order_bytes = checked_replace(
            OWNER_CONTENT_BYTES,
            self.owner_content_order_bytes,
            existing_bytes,
            owner.content_order_key().len(),
        )?;
        check_limit(
            OWNER_CONTENT_BYTES,
            owner_content_order_bytes,
            self.limits.max_staged_owner_content_order_bytes,
        )?;

        self.stages[stage_ordinal].owners[existing_ordinal] = owner;
        self.owner_candidate_slots = owner_candidate_slots;
        self.owner_content_order_bytes = owner_content_order_bytes;
        Ok(true)
    }

    fn charge_owner_comparison(&mut self) -> Result<(), StagedSectorClosureError> {
        let requested = checked_add(OWNER_COMPARISONS, self.owner_order_comparisons, 1)?;
        check_limit(
            OWNER_COMPARISONS,
            requested,
            self.limits.max_owner_order_comparisons,
        )?;
        self.owner_order_comparisons = requested;
        Ok(())
    }

    /// Insert one explicit finite terminal into its declared sector stage.
    /// Terminals never acquire authority merely by being finite complement
    /// points; this API requires the exact integral key explicitly.
    pub(crate) fn try_insert_terminal(
        &mut self,
        sector: &Mask,
        ordering: OrderingPolicy,
        terminal: IntegralKey,
    ) -> Result<bool, StagedSectorClosureError> {
        let key = StagedSectorKey {
            sector: sector.clone(),
            ordering,
        };
        let stage_ordinal = self.stage_ordinal(&key)?;
        if Mask::try_from_indices(terminal.powers())? != *sector {
            return Err(StagedSectorClosureError::TerminalOutsideSector);
        }
        if !self
            .predecessor
            .authenticates_explicit_terminal(&terminal)?
        {
            return Err(StagedSectorClosureError::UnauthenticatedTerminal);
        }
        let stage = &mut self.stages[stage_ordinal];
        let insertion = match stage
            .terminals
            .binary_search_by(|existing| existing.powers().cmp(terminal.powers()))
        {
            Ok(_) => return Ok(false),
            Err(insertion) => insertion,
        };
        let requested = checked_add(STAGED_TERMINALS, self.terminal_count, 1)?;
        check_limit(
            STAGED_TERMINALS,
            requested,
            self.limits.max_staged_terminals,
        )?;
        let terminal_coordinate_cells = checked_add(
            TERMINAL_COORDINATES,
            self.terminal_coordinate_cells,
            terminal.powers().len(),
        )?;
        check_limit(
            TERMINAL_COORDINATES,
            terminal_coordinate_cells,
            self.limits.max_staged_terminal_coordinate_cells,
        )?;
        stage.terminals.try_reserve(1).map_err(|_| {
            StagedSectorClosureError::AllocationFailure {
                resource: STAGED_TERMINALS,
                requested,
            }
        })?;
        stage.terminals.insert(insertion, terminal);
        self.terminal_count = requested;
        self.terminal_coordinate_cells = terminal_coordinate_cells;
        Ok(true)
    }

    /// Compile every staged sector. No cover is sealed if any normal stop is
    /// present; successful publication extends the predecessor with the full
    /// same-rank layer vector in one immutable transaction.
    pub(crate) fn try_finish(self) -> Result<StagedSectorClosureOutcome, StagedSectorClosureError> {
        let Self {
            context,
            predecessor,
            stages,
            limits,
            ..
        } = self;
        let stage_count = stages.len();
        preflight_pairing_work(&stages, limits.max_compiled_pairing_probes)?;
        let mut covers = try_vec(stage_count, STAGED_SECTORS)?;
        let mut stops = try_vec(stage_count, STAGED_SECTORS)?;
        let mut compiled_work = CompiledWaveWork::default();
        for stage in stages {
            if stage.owners.is_empty() {
                stops.push(StagedSectorClosureStop::NonFinite(
                    StagedSectorClosureStopEvidence::new(
                        &stage.key,
                        0,
                        stage.terminals.len(),
                        1,
                        0,
                        0,
                    ),
                ));
                continue;
            }
            let executable_limits = compiled_work.constrain(limits)?;
            let cover = ExactExecutableOwnerCover::try_compile(
                &context,
                stage.owners,
                stage.terminals,
                executable_limits,
            )?;
            compiled_work.try_retain(&cover, limits)?;
            match cover.proof_cover().status() {
                ExactOwnerCoverStatus::Closed => covers.push(cover),
                ExactOwnerCoverStatus::Incomplete(obstruction) => {
                    stops.push(stop_from_cover(&stage.key, &cover, obstruction));
                }
            }
        }
        if !stops.is_empty() {
            return Ok(StagedSectorClosureOutcome::Stopped(
                stops.into_boxed_slice(),
            ));
        }
        if covers.len() != stage_count {
            return Err(StagedSectorClosureError::OwnerScope {
                detail: "closed cover count differs from the declared frontier",
            });
        }

        let mut layers = try_vec(covers.len(), STAGED_SECTORS)?;
        for cover in covers {
            let sealed = ClosedExactExecutableOwnerCover::try_seal(cover)?;
            layers.push(ClosedSectorLayer::try_publish(sealed, limits.registry)?);
        }
        let extension_layers = try_clone_arcs(&layers, STAGED_SECTORS)?;
        let successor =
            predecessor.try_extend_with_closed_layers(extension_layers, limits.registry)?;
        Ok(StagedSectorClosureOutcome::Closed(
            ClosedSectorClosureWave::new(predecessor, successor, layers),
        ))
    }

    fn stage_ordinal(&self, key: &StagedSectorKey) -> Result<usize, StagedSectorClosureError> {
        self.stages
            .binary_search_by(|stage| stage.key.cmp(key))
            .map_err(|_| StagedSectorClosureError::UnregisteredSector)
    }

    fn validate_owner_scope(
        &self,
        owner: &ExactSemanticExecutableOwner,
    ) -> Result<(), StagedSectorClosureError> {
        let epoch = owner.epoch();
        let plan = epoch.plan();
        let detail = if plan.family_fingerprint() != self.predecessor.family_fingerprint() {
            Some("family fingerprint differs")
        } else if plan.context_fingerprint() != self.context.fingerprint() {
            Some("coefficient-context fingerprint differs")
        } else if plan.sector().arity() != self.predecessor.arity() {
            Some("arity differs")
        } else if epoch.fixed_snapshot_id() != self.predecessor.id() {
            Some("predecessor snapshot identity differs")
        } else if !epoch
            .predecessor_snapshot()
            .same_authority_as(&self.predecessor)
        {
            Some("predecessor snapshot authority differs")
        } else {
            None
        };
        if let Some(detail) = detail {
            Err(StagedSectorClosureError::OwnerScope { detail })
        } else {
            Ok(())
        }
    }
}

fn stop_from_cover(
    key: &StagedSectorKey,
    cover: &ExactExecutableOwnerCover,
    obstruction: ExactOwnerCoverObstructionKind,
) -> StagedSectorClosureStop {
    let proof = cover.proof_cover();
    let evidence = StagedSectorClosureStopEvidence::new(
        key,
        cover.owners().len(),
        cover.terminals().len(),
        proof.uncovered_partition().boxes().len(),
        proof.missing_terminals().len(),
        proof.guard_incomplete_owners().len(),
    );
    match obstruction {
        ExactOwnerCoverObstructionKind::NonFinite => StagedSectorClosureStop::NonFinite(evidence),
        ExactOwnerCoverObstructionKind::GuardIncomplete => {
            StagedSectorClosureStop::GuardIncomplete(evidence)
        }
        ExactOwnerCoverObstructionKind::FiniteTerminalOwnership => {
            StagedSectorClosureStop::FiniteTerminalOwnership(evidence)
        }
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, StagedSectorClosureError> {
    left.checked_add(right)
        .ok_or(StagedSectorClosureError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, StagedSectorClosureError> {
    left.checked_mul(right)
        .ok_or(StagedSectorClosureError::ResourceCountOverflow { resource })
}

fn checked_replace(
    resource: &'static str,
    current: usize,
    removed: usize,
    added: usize,
) -> Result<usize, StagedSectorClosureError> {
    current
        .checked_sub(removed)
        .and_then(|remaining| remaining.checked_add(added))
        .ok_or(StagedSectorClosureError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), StagedSectorClosureError> {
    if requested > limit {
        Err(StagedSectorClosureError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn try_vec<T>(capacity: usize, resource: &'static str) -> Result<Vec<T>, StagedSectorClosureError> {
    let mut values = Vec::new();
    values
        .try_reserve(capacity)
        .map_err(|_| StagedSectorClosureError::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(values)
}

fn try_clone_arcs<T>(
    values: &[Arc<T>],
    resource: &'static str,
) -> Result<Vec<Arc<T>>, StagedSectorClosureError> {
    let mut cloned = try_vec(values.len(), resource)?;
    cloned.extend(values.iter().cloned());
    Ok(cloned)
}

fn preflight_pairing_work(
    stages: &[StagedSector],
    limit: usize,
) -> Result<(), StagedSectorClosureError> {
    let mut probes = 0usize;
    for stage in stages {
        let stage_probes = checked_mul(
            COMPILED_PAIRING_PROBES,
            stage.owners.len(),
            stage.owners.len(),
        )?;
        probes = checked_add(COMPILED_PAIRING_PROBES, probes, stage_probes)?;
        check_limit(COMPILED_PAIRING_PROBES, probes, limit)?;
    }
    Ok(())
}

#[derive(Default)]
struct CompiledWaveWork {
    finite_points: usize,
    finite_coordinate_cells: usize,
    point_owner_probes: usize,
    uncovered_boxes: usize,
    uncovered_box_coordinate_cells: usize,
    split_operations: usize,
}

impl CompiledWaveWork {
    fn constrain(
        &self,
        limits: StagedSectorClosureLimits,
    ) -> Result<ExactExecutableOwnerLimits, StagedSectorClosureError> {
        let mut executable = limits.executable;
        executable.cover.max_finite_complement_points =
            executable.cover.max_finite_complement_points.min(remaining(
                COMPILED_FINITE_POINTS,
                limits.max_compiled_finite_complement_points,
                self.finite_points,
            )?);
        executable.cover.max_finite_complement_coordinate_cells = executable
            .cover
            .max_finite_complement_coordinate_cells
            .min(remaining(
                COMPILED_FINITE_COORDINATES,
                limits.max_compiled_finite_complement_coordinate_cells,
                self.finite_coordinate_cells,
            )?);
        executable.cover.max_point_owner_probes =
            executable.cover.max_point_owner_probes.min(remaining(
                COMPILED_POINT_PROBES,
                limits.max_compiled_point_owner_probes,
                self.point_owner_probes,
            )?);
        executable.cover.geometry.max_uncovered_boxes =
            executable.cover.geometry.max_uncovered_boxes.min(remaining(
                COMPILED_UNCOVERED_BOXES,
                limits.max_compiled_uncovered_boxes,
                self.uncovered_boxes,
            )?);
        executable.cover.geometry.max_uncovered_box_coordinate_cells = executable
            .cover
            .geometry
            .max_uncovered_box_coordinate_cells
            .min(remaining(
                COMPILED_UNCOVERED_COORDINATES,
                limits.max_compiled_uncovered_box_coordinate_cells,
                self.uncovered_box_coordinate_cells,
            )?);
        executable.cover.geometry.max_split_operations = executable
            .cover
            .geometry
            .max_split_operations
            .min(remaining(
                COMPILED_SPLITS,
                limits.max_compiled_split_operations,
                self.split_operations,
            )?);
        Ok(executable)
    }

    fn try_retain(
        &mut self,
        cover: &ExactExecutableOwnerCover,
        limits: StagedSectorClosureLimits,
    ) -> Result<(), StagedSectorClosureError> {
        let proof = cover.proof_cover();
        let arity = proof.sector().arity();
        self.finite_points = accumulate(
            COMPILED_FINITE_POINTS,
            self.finite_points,
            proof.finite_complement_point_count(),
            limits.max_compiled_finite_complement_points,
        )?;
        self.finite_coordinate_cells = accumulate(
            COMPILED_FINITE_COORDINATES,
            self.finite_coordinate_cells,
            checked_mul(
                COMPILED_FINITE_COORDINATES,
                proof.finite_complement_point_count(),
                arity,
            )?,
            limits.max_compiled_finite_complement_coordinate_cells,
        )?;
        self.point_owner_probes = accumulate(
            COMPILED_POINT_PROBES,
            self.point_owner_probes,
            proof.point_owner_probe_count(),
            limits.max_compiled_point_owner_probes,
        )?;
        self.uncovered_boxes = accumulate(
            COMPILED_UNCOVERED_BOXES,
            self.uncovered_boxes,
            proof.compiled_uncovered_box_count(),
            limits.max_compiled_uncovered_boxes,
        )?;
        self.uncovered_box_coordinate_cells = accumulate(
            COMPILED_UNCOVERED_COORDINATES,
            self.uncovered_box_coordinate_cells,
            proof.compiled_uncovered_box_coordinate_cells(),
            limits.max_compiled_uncovered_box_coordinate_cells,
        )?;
        self.split_operations = accumulate(
            COMPILED_SPLITS,
            self.split_operations,
            proof.compiled_split_operation_count(),
            limits.max_compiled_split_operations,
        )?;
        Ok(())
    }
}

fn remaining(
    resource: &'static str,
    limit: usize,
    used: usize,
) -> Result<usize, StagedSectorClosureError> {
    limit
        .checked_sub(used)
        .ok_or(StagedSectorClosureError::ResourceCountOverflow { resource })
}

fn accumulate(
    resource: &'static str,
    used: usize,
    additional: usize,
    limit: usize,
) -> Result<usize, StagedSectorClosureError> {
    let requested = checked_add(resource, used, additional)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}
