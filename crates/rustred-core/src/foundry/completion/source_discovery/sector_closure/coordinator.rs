use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::family::IntegralKey;
use crate::foundry::completion::frame::admission::{
    ExactOwnerCoverObstructionKind, ExactOwnerCoverStatus,
};
use crate::foundry::completion::stratum::ImmutableOwnerSnapshot;
use crate::foundry::completion::{LatticeBox, SectorChart};
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
const OWNER_CONTENT_KEY_BYTES: &str = "staged sector-closure retained owner content-key bytes";
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

#[derive(Debug)]
struct StagedClosureCarrier {
    key: StagedSectorKey,
    carrier: LatticeBox,
}

/// Publish one already compiler-closed same-rank wave without rebuilding any
/// exact owner cover.
///
/// Every input has already crossed [`ClosedExactExecutableOwnerCover::try_seal`].
/// This boundary performs only bounded scope/census checks, computes each
/// immutable layer identity once, and extends the predecessor after *all*
/// layers have published successfully. A failure therefore exposes neither a
/// partial successor nor a sibling sector as authority to another cover in
/// the same wave.
pub(crate) fn try_publish_sealed_sector_wave(
    predecessor: ImmutableOwnerSnapshot,
    mut covers: Vec<ClosedExactExecutableOwnerCover>,
    limits: StagedSectorClosureLimits,
) -> Result<ClosedSectorClosureWave, StagedSectorClosureError> {
    if covers.is_empty() {
        return Err(StagedSectorClosureError::EmptyFrontier);
    }
    if !predecessor.try_verify(limits.registry)? {
        return Err(StagedSectorClosureError::InvalidPredecessor);
    }
    check_limit(STAGED_SECTORS, covers.len(), limits.max_sectors)?;

    let mut frontier_coordinate_cells = 0usize;
    let mut owner_count = 0usize;
    let mut owner_coordinate_cells = 0usize;
    let mut owner_candidate_slots = 0usize;
    let mut owner_content_key_bytes = 0usize;
    let mut terminal_count = 0usize;
    let mut terminal_coordinate_cells = 0usize;
    let mut pairing_probes = 0usize;
    let mut compiled_work = CompiledWaveWork::default();
    for (cover_ordinal, sealed) in covers.iter().enumerate() {
        let cover = sealed.executable_cover();
        let proof = cover.proof_cover();
        if proof.family_fingerprint() != predecessor.family_fingerprint() {
            return Err(StagedSectorClosureError::WrongSealedCoverFamily {
                cover: cover_ordinal,
            });
        }
        if proof.context_fingerprint() != predecessor.context_fingerprint() {
            return Err(StagedSectorClosureError::WrongSealedCoverContext {
                cover: cover_ordinal,
            });
        }
        if proof.sector().arity() != predecessor.arity() {
            return Err(StagedSectorClosureError::WrongSectorArity {
                sector: cover_ordinal,
                expected: predecessor.arity(),
                actual: proof.sector().arity(),
            });
        }
        if !sealed
            .predecessor_snapshot()
            .same_authority_as(&predecessor)
        {
            return Err(StagedSectorClosureError::WrongSealedCoverPredecessor {
                cover: cover_ordinal,
            });
        }
        frontier_coordinate_cells = checked_add(
            FRONTIER_COORDINATES,
            frontier_coordinate_cells,
            proof.sector().arity(),
        )?;
        check_limit(
            FRONTIER_COORDINATES,
            frontier_coordinate_cells,
            limits.max_frontier_coordinate_cells,
        )?;
        owner_count = checked_add(STAGED_OWNERS, owner_count, cover.owners().len())?;
        check_limit(STAGED_OWNERS, owner_count, limits.max_staged_owners)?;
        owner_coordinate_cells = checked_add(
            OWNER_COORDINATES,
            owner_coordinate_cells,
            checked_mul(OWNER_COORDINATES, cover.owners().len(), predecessor.arity())?,
        )?;
        check_limit(
            OWNER_COORDINATES,
            owner_coordinate_cells,
            limits.max_staged_owner_coordinate_cells,
        )?;
        for owner in cover.owners() {
            owner_candidate_slots = checked_add(
                OWNER_CANDIDATES,
                owner_candidate_slots,
                owner.executable_candidates().len(),
            )?;
            check_limit(
                OWNER_CANDIDATES,
                owner_candidate_slots,
                limits.max_staged_owner_candidate_slots,
            )?;
            owner_content_key_bytes = checked_add(
                OWNER_CONTENT_KEY_BYTES,
                owner_content_key_bytes,
                owner.content_order_key().retained_bytes(),
            )?;
            check_limit(
                OWNER_CONTENT_KEY_BYTES,
                owner_content_key_bytes,
                limits.max_staged_owner_content_key_bytes,
            )?;
        }
        terminal_count = checked_add(STAGED_TERMINALS, terminal_count, cover.terminals().len())?;
        check_limit(
            STAGED_TERMINALS,
            terminal_count,
            limits.max_staged_terminals,
        )?;
        terminal_coordinate_cells = checked_add(
            TERMINAL_COORDINATES,
            terminal_coordinate_cells,
            checked_mul(
                TERMINAL_COORDINATES,
                cover.terminals().len(),
                predecessor.arity(),
            )?,
        )?;
        check_limit(
            TERMINAL_COORDINATES,
            terminal_coordinate_cells,
            limits.max_staged_terminal_coordinate_cells,
        )?;
        pairing_probes = checked_add(
            COMPILED_PAIRING_PROBES,
            pairing_probes,
            checked_mul(
                COMPILED_PAIRING_PROBES,
                cover.owners().len(),
                cover.owners().len(),
            )?,
        )?;
        check_limit(
            COMPILED_PAIRING_PROBES,
            pairing_probes,
            limits.max_compiled_pairing_probes,
        )?;
        compiled_work.try_retain(cover, limits)?;
    }
    validate_same_active_count(
        covers
            .iter()
            .enumerate()
            .map(|(ordinal, cover)| (ordinal, cover.executable_cover().proof_cover().sector())),
    )?;

    covers.sort_unstable_by(|left, right| {
        let left = left.executable_cover().proof_cover();
        let right = right.executable_cover().proof_cover();
        left.sector()
            .cmp(right.sector())
            .then_with(|| left.ordering().cmp(&right.ordering()))
    });
    if covers.windows(2).any(|pair| {
        let left = pair[0].executable_cover().proof_cover();
        let right = pair[1].executable_cover().proof_cover();
        left.sector() == right.sector() && left.ordering() == right.ordering()
    }) {
        return Err(StagedSectorClosureError::DuplicateSector);
    }

    let mut layers = try_vec(covers.len(), STAGED_SECTORS)?;
    for cover in covers {
        layers.push(ClosedSectorLayer::try_publish(cover, limits.registry)?);
    }
    let extension_layers = try_clone_arcs(&layers, STAGED_SECTORS)?;
    let successor = predecessor.try_extend_with_closed_layers(extension_layers, limits.registry)?;
    Ok(ClosedSectorClosureWave::new(predecessor, successor, layers))
}

/// Validate the single invariant that makes a layer batch one transactional
/// bottom-up wave. Kept separate so the sealed-cover publisher and focused
/// metadata tests cannot drift to subtly different rank semantics.
pub(super) fn validate_same_active_count<'sector>(
    sectors: impl IntoIterator<Item = (usize, &'sector Mask)>,
) -> Result<(), StagedSectorClosureError> {
    let mut expected_active_count = None;
    let mut found = false;
    for (sector_ordinal, sector) in sectors {
        found = true;
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
    }
    if found {
        Ok(())
    } else {
        Err(StagedSectorClosureError::EmptyFrontier)
    }
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
    owner_content_key_bytes: usize,
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
            owner_content_key_bytes: 0,
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

    pub(crate) const fn owner_content_key_bytes(&self) -> usize {
        self.owner_content_key_bytes
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

    /// Compile the exact cover for one staged sector without sealing or
    /// publishing it. This is the cold preview boundary used by discovery
    /// campaigns to compare an immutable owner ledger before and after one
    /// canonical proposal. Closure authority remains with the returned exact
    /// compiler status; this method never extends the predecessor snapshot.
    pub(crate) fn try_compile_single_sector_preview(
        self,
        carrier: &LatticeBox,
    ) -> Result<ExactExecutableOwnerCover, StagedSectorClosureError> {
        let Self {
            context,
            stages,
            limits,
            ..
        } = self;
        if stages.len() != 1 {
            return Err(StagedSectorClosureError::PreviewRequiresSingleSector {
                actual: stages.len(),
            });
        }
        preflight_pairing_work(&stages, limits.max_compiled_pairing_probes)?;
        let stage = stages
            .into_iter()
            .next()
            .ok_or(StagedSectorClosureError::PreviewRequiresSingleSector { actual: 0 })?;
        if stage.owners.is_empty() {
            return Err(StagedSectorClosureError::PreviewRequiresOwner);
        }
        let mut compiled_work = CompiledWaveWork::default();
        let executable_limits = compiled_work.constrain(limits)?;
        let cover = ExactExecutableOwnerCover::try_compile_with_carrier(
            &context,
            stage.owners,
            stage.terminals,
            carrier,
            executable_limits,
        )?;
        compiled_work.try_retain(&cover, limits)?;
        Ok(cover)
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
        let owner_content_key_bytes = checked_add(
            OWNER_CONTENT_KEY_BYTES,
            self.owner_content_key_bytes,
            owner.content_order_key().retained_bytes(),
        )?;
        check_limit(
            OWNER_CONTENT_KEY_BYTES,
            owner_content_key_bytes,
            self.limits.max_staged_owner_content_key_bytes,
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
        self.owner_content_key_bytes = owner_content_key_bytes;
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
        )?
        .is_lt()
        {
            return Ok(false);
        }
        let existing_candidates = self.stages[stage_ordinal].owners[existing_ordinal]
            .executable_candidates()
            .len();
        let existing_key_bytes = self.stages[stage_ordinal].owners[existing_ordinal]
            .content_order_key()
            .retained_bytes();
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
        let owner_content_key_bytes = checked_replace(
            OWNER_CONTENT_KEY_BYTES,
            self.owner_content_key_bytes,
            existing_key_bytes,
            owner.content_order_key().retained_bytes(),
        )?;
        check_limit(
            OWNER_CONTENT_KEY_BYTES,
            owner_content_key_bytes,
            self.limits.max_staged_owner_content_key_bytes,
        )?;

        self.stages[stage_ordinal].owners[existing_ordinal] = owner;
        self.owner_candidate_slots = owner_candidate_slots;
        self.owner_content_key_bytes = owner_content_key_bytes;
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

    /// Compile every staged sector against its complete machine-index carrier.
    ///
    /// This convenience remains useful for exact diagnostics, but translated
    /// source frames commonly stop short of a representability fringe. A
    /// publication caller should normally use
    /// [`Self::try_finish_with_closure_carriers`] and state the supported root
    /// universe explicitly.
    pub(crate) fn try_finish(self) -> Result<StagedSectorClosureOutcome, StagedSectorClosureError> {
        let mut carriers = try_vec(self.stages.len(), STAGED_SECTORS)?;
        for stage in &self.stages {
            carriers.push(StagedClosureCarrier {
                key: stage.key.clone(),
                carrier: SectorChart::new(stage.key.sector.clone()).carrier_box()?,
            });
        }
        self.try_finish_with_prepared_carriers(carriers)
    }

    /// Compile and transactionally publish one wave relative to an explicit
    /// finite carrier for every exact `(sector, ordering)` stage.
    ///
    /// Carrier input order is immaterial. Missing, duplicate, mismatched, or
    /// non-origin-anchored carriers are rejected before any cover is compiled;
    /// no uncovered representability fringe can be reclassified as a terminal.
    pub(crate) fn try_finish_with_closure_carriers(
        self,
        closure_carriers: impl IntoIterator<Item = (Mask, OrderingPolicy, LatticeBox)>,
    ) -> Result<StagedSectorClosureOutcome, StagedSectorClosureError> {
        let carriers = prepare_closure_carriers(&self.stages, closure_carriers)?;
        self.try_finish_with_prepared_carriers(carriers)
    }

    fn try_finish_with_prepared_carriers(
        self,
        carriers: Vec<StagedClosureCarrier>,
    ) -> Result<StagedSectorClosureOutcome, StagedSectorClosureError> {
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
        if stages.len() != carriers.len() {
            return Err(StagedSectorClosureError::ClosureCarrierCountMismatch {
                expected: stages.len(),
                actual: carriers.len(),
            });
        }
        for (stage, scoped_carrier) in stages.into_iter().zip(carriers) {
            if stage.key != scoped_carrier.key {
                return Err(StagedSectorClosureError::OwnerScope {
                    detail: "prepared closure carrier differs from its staged sector",
                });
            }
            if stage.owners.is_empty() {
                stops.push(StagedSectorClosureStop::NoExecutableOwners(
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
            let cover = ExactExecutableOwnerCover::try_compile_with_carrier(
                &context,
                stage.owners,
                stage.terminals,
                &scoped_carrier.carrier,
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

fn prepare_closure_carriers(
    stages: &[StagedSector],
    closure_carriers: impl IntoIterator<Item = (Mask, OrderingPolicy, LatticeBox)>,
) -> Result<Vec<StagedClosureCarrier>, StagedSectorClosureError> {
    let expected = stages.len();
    let mut carriers = try_vec(expected, STAGED_SECTORS)?;
    for (sector, ordering, carrier) in closure_carriers {
        if carriers.len() == expected {
            return Err(StagedSectorClosureError::ClosureCarrierCountMismatch {
                expected,
                actual: expected.saturating_add(1),
            });
        }
        carriers.push(StagedClosureCarrier {
            key: StagedSectorKey { sector, ordering },
            carrier,
        });
    }
    if carriers.len() != expected {
        return Err(StagedSectorClosureError::ClosureCarrierCountMismatch {
            expected,
            actual: carriers.len(),
        });
    }
    carriers.sort_unstable_by(|left, right| left.key.cmp(&right.key));
    if carriers.windows(2).any(|pair| pair[0].key == pair[1].key) {
        return Err(StagedSectorClosureError::DuplicateClosureCarrier);
    }
    for (ordinal, (stage, scoped)) in stages.iter().zip(&carriers).enumerate() {
        if stage.key != scoped.key {
            return Err(StagedSectorClosureError::ClosureCarrierScopeMismatch { carrier: ordinal });
        }
        let full = SectorChart::new(stage.key.sector.clone()).carrier_box()?;
        if scoped.carrier.arity() != stage.key.sector.arity()
            || scoped.carrier.lower().iter().any(|&lower| lower != 0)
            || scoped
                .carrier
                .upper()
                .iter()
                .zip(full.upper())
                .any(|(&upper, &full_upper)| match (upper, full_upper) {
                    (Some(upper), Some(full_upper)) => upper > full_upper,
                    _ => true,
                })
        {
            return Err(StagedSectorClosureError::InvalidClosureCarrier { carrier: ordinal });
        }
    }
    Ok(carriers)
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
        // The aggregate `max_compiled_*` fields bound retained proof census,
        // not one compiler invocation's transient partition scratch. Feeding
        // the remaining retained count back into geometry compilation can
        // reject a cover whose final census is exactly on budget (for
        // example, one retained uncovered box may require two temporary
        // boxes while clipping to a finite carrier). Per-cover scratch remains
        // governed by `limits.executable`; `try_retain` below enforces the
        // aggregate envelope transactionally after compilation.
        check_limit(
            COMPILED_FINITE_POINTS,
            self.finite_points,
            limits.max_compiled_finite_complement_points,
        )?;
        check_limit(
            COMPILED_FINITE_COORDINATES,
            self.finite_coordinate_cells,
            limits.max_compiled_finite_complement_coordinate_cells,
        )?;
        check_limit(
            COMPILED_POINT_PROBES,
            self.point_owner_probes,
            limits.max_compiled_point_owner_probes,
        )?;
        check_limit(
            COMPILED_UNCOVERED_BOXES,
            self.uncovered_boxes,
            limits.max_compiled_uncovered_boxes,
        )?;
        check_limit(
            COMPILED_UNCOVERED_COORDINATES,
            self.uncovered_box_coordinate_cells,
            limits.max_compiled_uncovered_box_coordinate_cells,
        )?;
        check_limit(
            COMPILED_SPLITS,
            self.split_operations,
            limits.max_compiled_split_operations,
        )?;
        Ok(limits.executable)
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
