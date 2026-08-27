//! Frozen owner for one accepted exact-publication closure epoch.
//!
//! Compilation is algebra-free. It consumes a quiescent, fully acknowledged
//! handoff wave, retains its canonical slots and their single event handles,
//! and replaces obsolete per-leaf handoff states by compact applicable and
//! exceptional flat-leaf indexes plus one byte per exceptional source. No
//! relation, predicate, or affine-geometry payload is copied.
//!
//! The reported resident total is an enumerated component charge: transferred
//! event payload plus this owner's shallow buffers. Shared campaign jobs,
//! session authority/plan/catalog graphs, allocator metadata, Symbolica TLS,
//! result buffers, and RSS headroom belong to the global deduplicated campaign
//! envelope and are deliberately excluded here.
//!
//! Only exceptional-source leases are admitted at this boundary. A future
//! applicable-rule provider must independently bound its live work and result
//! buffers. This module does not implement the `CampaignWorkKey` result table,
//! atomic result-charge transfer, or a RAM-bounded re-entry coordinator.

use std::fmt;
use std::mem::size_of;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering as AtomicOrdering};

use symbolica::prelude::Integer;

use super::{ExactPublicationHandoffSlot, ExactPublicationHandoffWave, LEAF_ISSUED, LEAF_PENDING};
use crate::generated_affine_residual_group_exact_session::{
    ApplicableRuleHandle, CommittedPublicationDomainView, CommittedPublicationEventView,
    CommittedPublicationLeafView, ExceptionalResidualHandle, ExceptionalResidualKind,
};
use crate::generated_affine_residual_group_solve_plan::GeneratedAffineResidualGroupSolveTargetLocator;
use crate::{CampaignJobKey, IntegralOrderingPolicy, SectorMask};

const SOURCE_PENDING: u8 = 0;
const SOURCE_ISSUED: u8 = 1;
const _: () = assert!(size_of::<AtomicU8>() == 1);

const fn portable_byte_limit(value: u128) -> usize {
    if value > usize::MAX as u128 {
        usize::MAX
    } else {
        value as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactPublicationEpochLimits {
    pub(crate) max_slots: usize,
    /// Handoff-state, classification, and fill passes are charged together.
    pub(crate) max_leaf_visits: usize,
    pub(crate) max_applicable_leaves: usize,
    pub(crate) max_exceptional_sources: usize,
    pub(crate) max_in_flight_sources: usize,
    pub(crate) max_in_flight_source_lease_bytes: usize,
    pub(crate) max_transferred_event_payload_bytes: usize,
    pub(crate) max_retained_shallow_bytes: usize,
    /// Ceiling for this module's enumerated component charge, not process RSS.
    pub(crate) max_total_resident_bytes: usize,
    pub(crate) max_compilation_peak_bytes: usize,
}

/// Internal convenience limits, not a production campaign memory policy.
///
/// These values are not derived from `M_operational`/`--max-memory`. A
/// production coordinator must construct explicit limits from its global
/// deduplicated resident and transient-memory envelope.
impl Default for ExactPublicationEpochLimits {
    fn default() -> Self {
        const GIB: u128 = 1024 * 1024 * 1024;
        Self {
            max_slots: 1_000_000,
            max_leaf_visits: 128_000_000,
            max_applicable_leaves: 64_000_000,
            max_exceptional_sources: 64_000_000,
            max_in_flight_sources: 4_096,
            max_in_flight_source_lease_bytes: 1024 * 1024,
            max_transferred_event_payload_bytes: portable_byte_limit(512 * GIB),
            max_retained_shallow_bytes: portable_byte_limit(64 * GIB),
            max_total_resident_bytes: portable_byte_limit(768 * GIB),
            max_compilation_peak_bytes: portable_byte_limit(896 * GIB),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ExactPublicationEpochStats {
    slots: usize,
    leaf_visits: usize,
    applicable: usize,
    exceptional_domain: usize,
    exceptional_leak: usize,
    transferred_event_payload_bytes: usize,
    released_handoff_leaf_state_bytes: usize,
    max_in_flight_sources: usize,
    max_in_flight_source_lease_bytes: usize,
    retained_shallow_bytes: usize,
    total_resident_bytes: usize,
    compilation_peak_bytes: usize,
}

impl ExactPublicationEpochStats {
    pub(crate) const fn slots(self) -> usize {
        self.slots
    }
    pub(crate) const fn leaf_visits(self) -> usize {
        self.leaf_visits
    }
    pub(crate) const fn applicable(self) -> usize {
        self.applicable
    }
    pub(crate) const fn exceptional_domain(self) -> usize {
        self.exceptional_domain
    }
    pub(crate) const fn exceptional_leak(self) -> usize {
        self.exceptional_leak
    }
    pub(crate) const fn exceptional(self) -> usize {
        self.exceptional_domain + self.exceptional_leak
    }
    pub(crate) const fn transferred_event_payload_bytes(self) -> usize {
        self.transferred_event_payload_bytes
    }
    pub(crate) const fn released_handoff_leaf_state_bytes(self) -> usize {
        self.released_handoff_leaf_state_bytes
    }
    pub(crate) const fn max_in_flight_sources(self) -> usize {
        self.max_in_flight_sources
    }
    pub(crate) const fn max_in_flight_source_lease_bytes(self) -> usize {
        self.max_in_flight_source_lease_bytes
    }
    pub(crate) const fn retained_shallow_bytes(self) -> usize {
        self.retained_shallow_bytes
    }
    /// Transferred event census plus enumerated owner buffers only.
    /// See the module-level exclusions; this is not reachable bytes or RSS.
    pub(crate) const fn total_resident_bytes(self) -> usize {
        self.total_resident_bytes
    }
    pub(crate) const fn compilation_peak_bytes(self) -> usize {
        self.compilation_peak_bytes
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ExactPublicationEpochSourceStateStats {
    pending: usize,
    issued: usize,
}

impl ExactPublicationEpochSourceStateStats {
    pub(crate) const fn pending(self) -> usize {
        self.pending
    }
    pub(crate) const fn issued(self) -> usize {
        self.issued
    }
}

/// Stable in-process scheduling coordinate within one prepared campaign.
///
/// This is not mathematical identity, rule equality, or a semantic key across
/// independently prepared campaigns. `closure_epoch_ordinal` is a caller-
/// supplied iteration label; it proves neither mathematical closure nor a
/// durable checkpoint. All usize coordinates are checked to fit `u64` before
/// the owner is created.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactPublicationEpochSchedulingKey<'owner> {
    job: &'owner CampaignJobKey,
    context_fingerprint: &'owner str,
    closure_epoch_ordinal: u64,
    session_lane_ordinal: u64,
    event_ordinal: u64,
    leaf_ordinal: u64,
}

impl<'owner> ExactPublicationEpochSchedulingKey<'owner> {
    pub(crate) const fn job(self) -> &'owner CampaignJobKey {
        self.job
    }
    /// Required scheduling-scope discriminator until context is part of the
    /// campaign job key itself.
    pub(crate) const fn context_fingerprint(self) -> &'owner str {
        self.context_fingerprint
    }
    pub(crate) const fn closure_epoch_ordinal(self) -> u64 {
        self.closure_epoch_ordinal
    }
    pub(crate) const fn session_lane_ordinal(self) -> u64 {
        self.session_lane_ordinal
    }
    pub(crate) const fn event_ordinal(self) -> u64 {
        self.event_ordinal
    }
    pub(crate) const fn leaf_ordinal(self) -> u64 {
        self.leaf_ordinal
    }
}

/// Repeatable zero-copy view of an applicable leaf. This seam deliberately
/// carries no provider/application state yet.
#[derive(Clone, Copy)]
pub(crate) struct ExactPublicationEpochApplicableView<'owner> {
    scheduling_key: ExactPublicationEpochSchedulingKey<'owner>,
    rule: ApplicableRuleHandle<'owner>,
}

impl<'owner> ExactPublicationEpochApplicableView<'owner> {
    pub(crate) const fn scheduling_key(self) -> ExactPublicationEpochSchedulingKey<'owner> {
        self.scheduling_key
    }
    pub(crate) const fn rule(self) -> ApplicableRuleHandle<'owner> {
        self.rule
    }
    pub(crate) const fn event(self) -> CommittedPublicationEventView<'owner> {
        self.rule.event()
    }
    pub(crate) const fn domain(self) -> CommittedPublicationDomainView<'owner> {
        self.rule.domain()
    }
}

impl fmt::Debug for ExactPublicationEpochApplicableView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactPublicationEpochApplicableView")
            .field("scheduling_key", &self.scheduling_key)
            .field("private_rule", &"<borrowed>")
            .finish()
    }
}

/// Zero-copy exceptional source accessible only through one live lease.
#[derive(Clone, Copy)]
pub(crate) struct ExactPublicationEpochExceptionalSourceView<'owner> {
    scheduling_key: ExactPublicationEpochSchedulingKey<'owner>,
    residual: ExceptionalResidualHandle<'owner>,
}

impl<'owner> ExactPublicationEpochExceptionalSourceView<'owner> {
    pub(crate) const fn scheduling_key(self) -> ExactPublicationEpochSchedulingKey<'owner> {
        self.scheduling_key
    }
    pub(crate) const fn kind(self) -> ExceptionalResidualKind {
        self.residual.kind()
    }
    /// Event-bound conjunction of target premises and relative predicates.
    pub(crate) const fn domain(self) -> CommittedPublicationDomainView<'owner> {
        self.residual.domain()
    }

    pub(crate) fn family_fingerprint(self) -> &'owner str {
        self.residual.event().family_fingerprint()
    }

    pub(crate) fn context_fingerprint(self) -> &'owner str {
        self.residual.event().context_fingerprint()
    }

    pub(crate) fn sector(self) -> &'owner SectorMask {
        self.residual.event().sector()
    }

    pub(crate) fn ordering(self) -> IntegralOrderingPolicy {
        self.residual.event().ordering()
    }

    pub(crate) fn target_locator(self) -> GeneratedAffineResidualGroupSolveTargetLocator {
        self.residual.event().target_locator()
    }

    pub(crate) fn target_offset(self) -> &'owner [Integer] {
        self.residual.event().target_offset()
    }

    pub(crate) fn ambient_arity(self) -> usize {
        self.residual.event().ambient_arity()
    }

    pub(crate) fn free_positions(self) -> &'owner [usize] {
        self.residual.event().free_positions()
    }

    /// Row-major `ambient_arity() * free_positions().len()` exact matrix.
    pub(crate) fn compact_affine_matrix(self) -> &'owner [Integer] {
        self.residual.event().compact_affine_matrix()
    }
}

impl fmt::Debug for ExactPublicationEpochExceptionalSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactPublicationEpochExceptionalSourceView")
            .field("scheduling_key", &self.scheduling_key)
            .field("kind", &self.residual.kind())
            .field("private_residual", &"<borrowed>")
            .finish()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ExactPublicationEpochSourceLocator<'owner> {
    owner: &'owner ExactPublicationEpochOwner,
    source_ordinal: usize,
}

impl ExactPublicationEpochSourceLocator<'_> {
    pub(crate) const fn source_ordinal(self) -> usize {
        self.source_ordinal
    }
}

impl fmt::Debug for ExactPublicationEpochSourceLocator<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactPublicationEpochSourceLocator")
            .field("source_ordinal", &self.source_ordinal)
            .finish_non_exhaustive()
    }
}

/// Non-cloneable borrowed permit for one exceptional source.
#[must_use = "dropping an exceptional-source lease returns it to pending"]
pub(crate) struct ExactPublicationEpochSourceLease<'owner> {
    owner: &'owner ExactPublicationEpochOwner,
    source_ordinal: usize,
}

impl ExactPublicationEpochSourceLease<'_> {
    pub(crate) const fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }
}

impl Drop for ExactPublicationEpochSourceLease<'_> {
    fn drop(&mut self) {
        if self.owner.exceptional_source_states[self.source_ordinal]
            .compare_exchange(
                SOURCE_ISSUED,
                SOURCE_PENDING,
                AtomicOrdering::AcqRel,
                AtomicOrdering::Acquire,
            )
            .is_ok()
        {
            self.owner.release_in_flight_source();
        }
    }
}

impl fmt::Debug for ExactPublicationEpochSourceLease<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactPublicationEpochSourceLease")
            .field("source_ordinal", &self.source_ordinal)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExactPublicationEpochError {
    RecoveredStrandedHandoffTickets {
        recovered: usize,
        prior_in_flight: usize,
    },
    HandoffIssuanceInvariantMismatch {
        issued: usize,
        in_flight: usize,
    },
    HandoffNotQuiescent {
        issued: usize,
        in_flight: usize,
    },
    HandoffNotFullyAcknowledged {
        pending: usize,
        acknowledged: usize,
        expected: usize,
    },
    CoordinateExceedsU64 {
        coordinate: &'static str,
        slot_ordinal: usize,
        leaf_ordinal: usize,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    ForeignLocator,
    ForeignLease,
    UnknownSource,
    NotIssued,
    AlreadyIssued,
    SourceIssuanceInvariantMismatch {
        issued: usize,
        in_flight: usize,
    },
    InFlightSourceLimit {
        requested: usize,
        limit: usize,
    },
}

impl fmt::Display for ExactPublicationEpochError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecoveredStrandedHandoffTickets {
                recovered,
                prior_in_flight,
            } => write!(
                formatter,
                "recovered {recovered} stranded publication-handoff tickets ({prior_in_flight} previously in flight); retry their leaves before epoch conversion"
            ),
            Self::HandoffIssuanceInvariantMismatch { issued, in_flight } => write!(
                formatter,
                "publication-handoff issued-state count {issued} differs from in-flight count {in_flight}"
            ),
            Self::HandoffNotQuiescent { issued, in_flight } => write!(
                formatter,
                "publication handoff is not quiescent ({issued} issued leaves, {in_flight} live tickets)"
            ),
            Self::HandoffNotFullyAcknowledged {
                pending,
                acknowledged,
                expected,
            } => write!(
                formatter,
                "publication handoff is not fully acknowledged ({pending} pending, {acknowledged} acknowledged, {expected} total)"
            ),
            Self::CoordinateExceedsU64 { coordinate, .. } => {
                write!(
                    formatter,
                    "publication epoch {coordinate} coordinate exceeds u64"
                )
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "{resource} allocation of {requested} entries failed after bounded preflight"
            ),
            Self::ForeignLocator => formatter.write_str("exceptional-source locator is foreign"),
            Self::ForeignLease => formatter.write_str("exceptional-source lease is foreign"),
            Self::UnknownSource => formatter.write_str("exceptional source is out of range"),
            Self::NotIssued => formatter.write_str("exceptional source is not issued"),
            Self::AlreadyIssued => formatter.write_str("exceptional source was already issued"),
            Self::SourceIssuanceInvariantMismatch { issued, in_flight } => write!(
                formatter,
                "exceptional-source issued-state count {issued} differs from in-flight count {in_flight}"
            ),
            Self::InFlightSourceLimit { requested, limit } => write!(
                formatter,
                "exceptional-source admission requested {requested} live leases, configured limit is {limit}"
            ),
        }
    }
}

impl std::error::Error for ExactPublicationEpochError {}

/// Transactional failure preserving the complete move-only handoff wave.
pub(crate) struct ExactPublicationEpochFailure {
    error: ExactPublicationEpochError,
    wave: ExactPublicationHandoffWave,
}

impl ExactPublicationEpochFailure {
    pub(crate) const fn error(&self) -> ExactPublicationEpochError {
        self.error
    }
    pub(crate) fn into_parts(self) -> (ExactPublicationEpochError, ExactPublicationHandoffWave) {
        (self.error, self.wave)
    }
}

impl fmt::Debug for ExactPublicationEpochFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactPublicationEpochFailure")
            .field("error", &self.error)
            .field("handoff_stats", &self.wave.stats())
            .field("private_wave", &"<redacted>")
            .finish()
    }
}

/// Frozen owner of the accepted leaves from one exact closure epoch.
///
/// The input handoff acknowledgement proves only no-loss ownership transfer.
/// Exceptional-source leases have no terminal state in this seam: staging or
/// merging one requires a separately RAM-admitted result owner.
pub(crate) struct ExactPublicationEpochOwner {
    closure_epoch_ordinal: u64,
    slots: Vec<ExactPublicationHandoffSlot>,
    applicable_flat_leaf_indexes: Vec<usize>,
    exceptional_flat_leaf_indexes: Vec<usize>,
    exceptional_source_states: Vec<AtomicU8>,
    in_flight_sources: AtomicUsize,
    limits: ExactPublicationEpochLimits,
    stats: ExactPublicationEpochStats,
}

impl ExactPublicationEpochOwner {
    pub(crate) fn compile(
        mut wave: ExactPublicationHandoffWave,
        closure_epoch_ordinal: u64,
        limits: ExactPublicationEpochLimits,
    ) -> Result<Self, ExactPublicationEpochFailure> {
        if let Err(error) = preflight_epoch_scalar_limits(&wave, limits) {
            return Err(ExactPublicationEpochFailure { error, wave });
        }
        match recover_stranded_handoff_tickets(&mut wave) {
            Ok(Some(error)) | Err(error) => {
                return Err(ExactPublicationEpochFailure { error, wave });
            }
            Ok(None) => {}
        }
        match prepare_epoch_owner(&wave, limits) {
            Ok(prepared) => Ok(prepared.finish(wave, closure_epoch_ordinal)),
            Err(error) => Err(ExactPublicationEpochFailure { error, wave }),
        }
    }

    pub(crate) const fn closure_epoch_ordinal(&self) -> u64 {
        self.closure_epoch_ordinal
    }
    pub(crate) const fn limits(&self) -> ExactPublicationEpochLimits {
        self.limits
    }
    pub(crate) const fn stats(&self) -> ExactPublicationEpochStats {
        self.stats
    }
    pub(crate) fn in_flight_sources(&self) -> usize {
        self.in_flight_sources.load(AtomicOrdering::Acquire)
    }

    /// Diagnostic snapshot; concurrent loads need not form one atomic global
    /// snapshot while leases are changing state.
    pub(crate) fn source_state_stats(&self) -> ExactPublicationEpochSourceStateStats {
        let mut stats = ExactPublicationEpochSourceStateStats::default();
        for state in &self.exceptional_source_states {
            match state.load(AtomicOrdering::Acquire) {
                SOURCE_PENDING => stats.pending += 1,
                SOURCE_ISSUED => stats.issued += 1,
                _ => unreachable!("sealed publication epoch has an invalid source state"),
            }
        }
        stats
    }

    /// Barrier-only recovery for leases deliberately forgotten by safe code.
    ///
    /// Exclusive access proves that no usable lease borrow remains. This
    /// seam cannot stage results, so resetting `Issued` to `Pending` cannot
    /// duplicate an accepted result. A future staging owner must replace this
    /// contract atomically rather than calling recovery after accepting work.
    pub(crate) fn recover_stranded_exceptional_sources(
        &mut self,
    ) -> Result<usize, ExactPublicationEpochError> {
        let prior_in_flight = *self.in_flight_sources.get_mut();
        let mut issued = 0usize;
        for state in &mut self.exceptional_source_states {
            if *state.get_mut() == SOURCE_ISSUED {
                issued = issued.checked_add(1).ok_or(
                    ExactPublicationEpochError::ResourceCountOverflow {
                        resource: "publication epoch recovered source leases",
                    },
                )?;
            }
        }
        if issued != prior_in_flight {
            return Err(
                ExactPublicationEpochError::SourceIssuanceInvariantMismatch {
                    issued,
                    in_flight: prior_in_flight,
                },
            );
        }
        for state in &mut self.exceptional_source_states {
            if *state.get_mut() == SOURCE_ISSUED {
                *state.get_mut() = SOURCE_PENDING;
            }
        }
        *self.in_flight_sources.get_mut() = 0;
        Ok(issued)
    }

    /// Repeatable zero-copy view; no provider/application state is implied.
    pub(crate) fn applicable(
        &self,
        applicable_ordinal: usize,
    ) -> Option<ExactPublicationEpochApplicableView<'_>> {
        let flat_leaf_index = *self.applicable_flat_leaf_indexes.get(applicable_ordinal)?;
        let (slot, leaf_ordinal) = self.resolve_flat_leaf(flat_leaf_index)?;
        let rule = match slot.event.view().leaf(leaf_ordinal)? {
            CommittedPublicationLeafView::Applicable(rule) => rule,
            CommittedPublicationLeafView::Exceptional(_) => {
                unreachable!("applicable flat index changed classification")
            }
        };
        Some(ExactPublicationEpochApplicableView {
            scheduling_key: scheduling_key(self.closure_epoch_ordinal, slot, leaf_ordinal),
            rule,
        })
    }

    pub(crate) fn exceptional_source_locator(
        &self,
        source_ordinal: usize,
    ) -> Option<ExactPublicationEpochSourceLocator<'_>> {
        (source_ordinal < self.exceptional_flat_leaf_indexes.len()).then_some(
            ExactPublicationEpochSourceLocator {
                owner: self,
                source_ordinal,
            },
        )
    }

    pub(crate) fn issue_exceptional_source<'owner>(
        &'owner self,
        locator: ExactPublicationEpochSourceLocator<'_>,
    ) -> Result<ExactPublicationEpochSourceLease<'owner>, ExactPublicationEpochError> {
        if !std::ptr::eq(self, locator.owner) {
            return Err(ExactPublicationEpochError::ForeignLocator);
        }
        let state = self
            .exceptional_source_states
            .get(locator.source_ordinal)
            .ok_or(ExactPublicationEpochError::UnknownSource)?;
        self.reserve_in_flight_source()?;
        match state.compare_exchange(
            SOURCE_PENDING,
            SOURCE_ISSUED,
            AtomicOrdering::AcqRel,
            AtomicOrdering::Acquire,
        ) {
            Ok(SOURCE_PENDING) => {}
            Err(SOURCE_ISSUED) => {
                self.release_in_flight_source();
                return Err(ExactPublicationEpochError::AlreadyIssued);
            }
            _ => unreachable!("sealed publication epoch has an invalid source state"),
        }
        Ok(ExactPublicationEpochSourceLease {
            owner: self,
            source_ordinal: locator.source_ordinal,
        })
    }

    /// Borrow a source only for the lifetime of an exclusive lease borrow.
    pub(crate) fn resolve_exceptional_source<'view>(
        &'view self,
        lease: &'view mut ExactPublicationEpochSourceLease<'_>,
    ) -> Result<ExactPublicationEpochExceptionalSourceView<'view>, ExactPublicationEpochError> {
        if !std::ptr::eq(self, lease.owner) {
            return Err(ExactPublicationEpochError::ForeignLease);
        }
        match self.exceptional_source_states[lease.source_ordinal].load(AtomicOrdering::Acquire) {
            SOURCE_ISSUED => {}
            SOURCE_PENDING => return Err(ExactPublicationEpochError::NotIssued),
            _ => unreachable!("sealed publication epoch has an invalid source state"),
        }
        let flat_leaf_index = *self
            .exceptional_flat_leaf_indexes
            .get(lease.source_ordinal)
            .ok_or(ExactPublicationEpochError::UnknownSource)?;
        let (slot, leaf_ordinal) = self
            .resolve_flat_leaf(flat_leaf_index)
            .ok_or(ExactPublicationEpochError::UnknownSource)?;
        let residual = match slot
            .event
            .view()
            .leaf(leaf_ordinal)
            .ok_or(ExactPublicationEpochError::UnknownSource)?
        {
            CommittedPublicationLeafView::Applicable(_) => {
                unreachable!("exceptional flat index changed classification")
            }
            CommittedPublicationLeafView::Exceptional(residual) => residual,
        };
        Ok(ExactPublicationEpochExceptionalSourceView {
            scheduling_key: scheduling_key(self.closure_epoch_ordinal, slot, leaf_ordinal),
            residual,
        })
    }

    /// Explicitly end an attempt without claiming a result or progress.
    pub(crate) fn release_exceptional_source(
        &self,
        lease: ExactPublicationEpochSourceLease<'_>,
    ) -> Result<(), ExactPublicationEpochError> {
        if !std::ptr::eq(self, lease.owner) {
            return Err(ExactPublicationEpochError::ForeignLease);
        }
        // Drop is the lease's single state transition. Performing the CAS
        // here and then letting Drop run would permit an ABA race if another
        // worker reissued the newly pending source between those operations.
        drop(lease);
        Ok(())
    }

    fn resolve_flat_leaf(
        &self,
        flat_leaf_index: usize,
    ) -> Option<(&ExactPublicationHandoffSlot, usize)> {
        let slot_index = self.slots.partition_point(|slot| {
            slot.first_leaf_state
                .checked_add(slot.leaf_count)
                .expect("validated publication leaf range overflow")
                <= flat_leaf_index
        });
        let slot = self.slots.get(slot_index)?;
        let leaf_ordinal = flat_leaf_index.checked_sub(slot.first_leaf_state)?;
        (leaf_ordinal < slot.leaf_count).then_some((slot, leaf_ordinal))
    }

    fn reserve_in_flight_source(&self) -> Result<(), ExactPublicationEpochError> {
        loop {
            let current = self.in_flight_sources.load(AtomicOrdering::Acquire);
            let requested = current.checked_add(1).ok_or(
                ExactPublicationEpochError::ResourceCountOverflow {
                    resource: "publication epoch in-flight sources",
                },
            )?;
            if requested > self.limits.max_in_flight_sources {
                return Err(ExactPublicationEpochError::InFlightSourceLimit {
                    requested,
                    limit: self.limits.max_in_flight_sources,
                });
            }
            if self
                .in_flight_sources
                .compare_exchange(
                    current,
                    requested,
                    AtomicOrdering::AcqRel,
                    AtomicOrdering::Acquire,
                )
                .is_ok()
            {
                return Ok(());
            }
        }
    }

    fn release_in_flight_source(&self) {
        self.in_flight_sources
            .fetch_update(AtomicOrdering::AcqRel, AtomicOrdering::Acquire, |current| {
                current.checked_sub(1)
            })
            .expect("publication epoch released an unreserved source lease");
    }

    #[cfg(test)]
    fn event_for_test(&self, slot_ordinal: usize) -> Option<CommittedPublicationEventView<'_>> {
        Some(self.slots.get(slot_ordinal)?.event.view())
    }
}

/// Recover only tickets made unreachable through `mem::forget`.
///
/// The consuming caller owns the complete wave, so safe Rust proves that no
/// usable ticket borrow remains. Recovered leaves return to `Pending`; they
/// are never reinterpreted as acknowledged, and conversion stops immediately.
fn recover_stranded_handoff_tickets(
    wave: &mut ExactPublicationHandoffWave,
) -> Result<Option<ExactPublicationEpochError>, ExactPublicationEpochError> {
    let prior_in_flight = *wave.in_flight_tickets.get_mut();
    if prior_in_flight == 0 {
        return Ok(None);
    }
    let mut issued = 0usize;
    for state in &mut wave.leaf_states {
        if *state.get_mut() == LEAF_ISSUED {
            issued =
                issued
                    .checked_add(1)
                    .ok_or(ExactPublicationEpochError::ResourceCountOverflow {
                        resource: "publication epoch recovered handoff tickets",
                    })?;
        }
    }
    if issued != prior_in_flight {
        return Err(
            ExactPublicationEpochError::HandoffIssuanceInvariantMismatch {
                issued,
                in_flight: prior_in_flight,
            },
        );
    }
    for state in &mut wave.leaf_states {
        if *state.get_mut() == LEAF_ISSUED {
            *state.get_mut() = LEAF_PENDING;
        }
    }
    *wave.in_flight_tickets.get_mut() = 0;
    Ok(Some(
        ExactPublicationEpochError::RecoveredStrandedHandoffTickets {
            recovered: issued,
            prior_in_flight,
        },
    ))
}

impl fmt::Debug for ExactPublicationEpochOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactPublicationEpochOwner")
            .field("closure_epoch_ordinal", &self.closure_epoch_ordinal)
            .field("stats", &self.stats)
            .field("source_state_stats", &self.source_state_stats())
            .field("limits", &self.limits)
            .field("private_slots", &"<redacted>")
            .finish()
    }
}

struct PreparedEpochOwner {
    applicable_flat_leaf_indexes: Vec<usize>,
    exceptional_flat_leaf_indexes: Vec<usize>,
    exceptional_source_states: Vec<AtomicU8>,
    limits: ExactPublicationEpochLimits,
    stats: ExactPublicationEpochStats,
}

#[derive(Clone, Copy)]
struct EpochMemoryEnvelope {
    released_handoff_leaf_state_bytes: usize,
    max_in_flight_source_lease_bytes: usize,
    retained_shallow_bytes: usize,
    total_resident_bytes: usize,
    compilation_peak_bytes: usize,
}

impl PreparedEpochOwner {
    fn finish(
        self,
        wave: ExactPublicationHandoffWave,
        closure_epoch_ordinal: u64,
    ) -> ExactPublicationEpochOwner {
        let ExactPublicationHandoffWave {
            slots,
            leaf_states,
            in_flight_tickets: _,
            limits: _,
            stats: _,
        } = wave;
        // The accepted handoff no longer needs one acknowledgement byte for
        // every leaf. Only exceptional sources acquire fresh attempt state.
        drop(leaf_states);
        ExactPublicationEpochOwner {
            closure_epoch_ordinal,
            slots,
            applicable_flat_leaf_indexes: self.applicable_flat_leaf_indexes,
            exceptional_flat_leaf_indexes: self.exceptional_flat_leaf_indexes,
            exceptional_source_states: self.exceptional_source_states,
            in_flight_sources: AtomicUsize::new(0),
            limits: self.limits,
            stats: self.stats,
        }
    }
}

fn prepare_epoch_owner(
    wave: &ExactPublicationHandoffWave,
    limits: ExactPublicationEpochLimits,
) -> Result<PreparedEpochOwner, ExactPublicationEpochError> {
    // Scalar traversal/count admission ran before even the optional forgotten-
    // ticket recovery scan. Preserve the exact successful-compile census here.
    let leaf_visits = checked_mul(
        "publication epoch total leaf visits",
        wave.stats().leaves(),
        3,
    )?;

    let state_stats = wave.state_stats();
    let in_flight = wave.in_flight_tickets();
    if state_stats.issued() != in_flight {
        return Err(
            ExactPublicationEpochError::HandoffIssuanceInvariantMismatch {
                issued: state_stats.issued(),
                in_flight,
            },
        );
    }
    if state_stats.issued() != 0 || in_flight != 0 {
        return Err(ExactPublicationEpochError::HandoffNotQuiescent {
            issued: state_stats.issued(),
            in_flight,
        });
    }
    if state_stats.pending() != 0 || state_stats.acknowledged() != wave.stats().leaves() {
        return Err(ExactPublicationEpochError::HandoffNotFullyAcknowledged {
            pending: state_stats.pending(),
            acknowledged: state_stats.acknowledged(),
            expected: wave.stats().leaves(),
        });
    }

    // First pass: validate canonical ranges/coordinates and classify without
    // allocating any buffer or moving the wave.
    let mut applicable = 0usize;
    let mut exceptional_domain = 0usize;
    let mut exceptional_leak = 0usize;
    let mut expected_first_leaf = 0usize;
    for (slot_ordinal, slot) in wave.slots.iter().enumerate() {
        if slot.first_leaf_state != expected_first_leaf {
            unreachable!("sealed handoff slots lost canonical flat-leaf contiguity")
        }
        let event = slot.event.view();
        checked_u64("session lane", slot.session_lane_ordinal, slot_ordinal, 0)?;
        checked_u64("event", slot.event_ordinal, slot_ordinal, 0)?;
        for leaf_ordinal in 0..slot.leaf_count {
            checked_u64("leaf", leaf_ordinal, slot_ordinal, leaf_ordinal)?;
            match event
                .leaf(leaf_ordinal)
                .expect("committed publication slot lost a leaf")
            {
                CommittedPublicationLeafView::Applicable(_) => {
                    applicable = checked_add("publication epoch applicable leaves", applicable, 1)?;
                }
                CommittedPublicationLeafView::Exceptional(residual) => match residual.kind() {
                    ExceptionalResidualKind::Domain => {
                        exceptional_domain = checked_add(
                            "publication epoch exceptional-domain sources",
                            exceptional_domain,
                            1,
                        )?;
                    }
                    ExceptionalResidualKind::SectorLeak => {
                        exceptional_leak = checked_add(
                            "publication epoch exceptional-leak sources",
                            exceptional_leak,
                            1,
                        )?;
                    }
                },
            }
        }
        expected_first_leaf = checked_add(
            "publication epoch canonical flat leaves",
            expected_first_leaf,
            slot.leaf_count,
        )?;
    }
    debug_assert_eq!(expected_first_leaf, wave.stats().leaves());
    debug_assert_eq!(applicable, wave.stats().applicable());
    debug_assert_eq!(
        exceptional_domain + exceptional_leak,
        wave.stats().exceptional()
    );

    let max_in_flight_sources = limits.max_in_flight_sources.min(wave.stats().exceptional());
    // Admit the prospective heap envelope before reserving any new buffer.
    // The allocator may return larger capacities, so the actual envelope is
    // checked again immediately after all exact reservations succeed.
    let prospective_memory = epoch_memory_envelope(
        wave,
        applicable,
        wave.stats().exceptional(),
        wave.stats().exceptional(),
        max_in_flight_sources,
    )?;
    enforce_memory_envelope(prospective_memory, limits)?;

    // Reserve every buffer before the second pass writes any index.
    let mut applicable_flat_leaf_indexes =
        try_vec_capacity::<usize>("publication epoch applicable flat indexes", applicable)?;
    let mut exceptional_flat_leaf_indexes = try_vec_capacity::<usize>(
        "publication epoch exceptional flat indexes",
        wave.stats().exceptional(),
    )?;
    let mut exceptional_source_states = try_vec_capacity::<AtomicU8>(
        "publication epoch exceptional source states",
        wave.stats().exceptional(),
    )?;

    let memory = epoch_memory_envelope(
        wave,
        applicable_flat_leaf_indexes.capacity(),
        exceptional_flat_leaf_indexes.capacity(),
        exceptional_source_states.capacity(),
        max_in_flight_sources,
    )?;
    enforce_memory_envelope(memory, limits)?;

    // Second pass: fill canonical indexes with no further allocation.
    for slot in &wave.slots {
        let event = slot.event.view();
        for leaf_ordinal in 0..slot.leaf_count {
            let flat_leaf_index = slot
                .first_leaf_state
                .checked_add(leaf_ordinal)
                .expect("validated publication flat-leaf index overflow");
            match event
                .leaf(leaf_ordinal)
                .expect("committed publication slot lost a leaf")
            {
                CommittedPublicationLeafView::Applicable(_) => {
                    applicable_flat_leaf_indexes.push(flat_leaf_index);
                }
                CommittedPublicationLeafView::Exceptional(_) => {
                    exceptional_flat_leaf_indexes.push(flat_leaf_index);
                    exceptional_source_states.push(AtomicU8::new(SOURCE_PENDING));
                }
            }
        }
    }

    Ok(PreparedEpochOwner {
        applicable_flat_leaf_indexes,
        exceptional_flat_leaf_indexes,
        exceptional_source_states,
        limits,
        stats: ExactPublicationEpochStats {
            slots: wave.stats().slots(),
            leaf_visits,
            applicable,
            exceptional_domain,
            exceptional_leak,
            transferred_event_payload_bytes: wave.stats().retained_event_payload_bytes(),
            released_handoff_leaf_state_bytes: memory.released_handoff_leaf_state_bytes,
            max_in_flight_sources,
            max_in_flight_source_lease_bytes: memory.max_in_flight_source_lease_bytes,
            retained_shallow_bytes: memory.retained_shallow_bytes,
            total_resident_bytes: memory.total_resident_bytes,
            compilation_peak_bytes: memory.compilation_peak_bytes,
        },
    })
}

fn preflight_epoch_scalar_limits(
    wave: &ExactPublicationHandoffWave,
    limits: ExactPublicationEpochLimits,
) -> Result<(), ExactPublicationEpochError> {
    // Admit every possible full traversal before the first state scan: one
    // recovery/quiescence pass plus classification and fill passes. Recovery
    // stops conversion, so three passes are conservative on that failure path.
    let leaf_visits = checked_mul(
        "publication epoch total leaf visits",
        wave.stats().leaves(),
        3,
    )?;
    for (resource, requested, limit) in [
        (
            "publication epoch slots",
            wave.slots.len(),
            limits.max_slots,
        ),
        (
            "publication epoch total leaf visits",
            leaf_visits,
            limits.max_leaf_visits,
        ),
        (
            "publication epoch applicable leaves",
            wave.stats().applicable(),
            limits.max_applicable_leaves,
        ),
        (
            "publication epoch exceptional sources",
            wave.stats().exceptional(),
            limits.max_exceptional_sources,
        ),
        (
            "publication epoch transferred event payload bytes",
            wave.stats().retained_event_payload_bytes(),
            limits.max_transferred_event_payload_bytes,
        ),
    ] {
        check_limit(resource, requested, limit)?;
    }
    Ok(())
}

fn scheduling_key<'owner>(
    closure_epoch_ordinal: u64,
    slot: &'owner ExactPublicationHandoffSlot,
    leaf_ordinal: usize,
) -> ExactPublicationEpochSchedulingKey<'owner> {
    ExactPublicationEpochSchedulingKey {
        job: &slot.job,
        context_fingerprint: slot.event.view().context_fingerprint(),
        closure_epoch_ordinal,
        session_lane_ordinal: u64::try_from(slot.session_lane_ordinal)
            .expect("validated session-lane coordinate exceeds u64"),
        event_ordinal: u64::try_from(slot.event_ordinal)
            .expect("validated event coordinate exceeds u64"),
        leaf_ordinal: u64::try_from(leaf_ordinal).expect("validated leaf coordinate exceeds u64"),
    }
}

fn retained_shallow_bytes_for_capacities(
    slot_capacity: usize,
    applicable_capacity: usize,
    exceptional_capacity: usize,
    source_state_capacity: usize,
) -> Result<usize, ExactPublicationEpochError> {
    checked_add(
        "publication epoch retained shallow bytes",
        size_of::<ExactPublicationEpochOwner>(),
        checked_add(
            "publication epoch retained shallow bytes",
            checked_mul(
                "publication epoch retained slot bytes",
                slot_capacity,
                size_of::<ExactPublicationHandoffSlot>(),
            )?,
            checked_add(
                "publication epoch retained index/state bytes",
                checked_mul(
                    "publication epoch retained applicable-index bytes",
                    applicable_capacity,
                    size_of::<usize>(),
                )?,
                checked_add(
                    "publication epoch retained exceptional bytes",
                    checked_mul(
                        "publication epoch retained exceptional-index bytes",
                        exceptional_capacity,
                        size_of::<usize>(),
                    )?,
                    checked_mul(
                        "publication epoch retained source-state bytes",
                        source_state_capacity,
                        size_of::<AtomicU8>(),
                    )?,
                )?,
            )?,
        )?,
    )
}

fn epoch_memory_envelope(
    wave: &ExactPublicationHandoffWave,
    applicable_capacity: usize,
    exceptional_capacity: usize,
    source_state_capacity: usize,
    max_in_flight_sources: usize,
) -> Result<EpochMemoryEnvelope, ExactPublicationEpochError> {
    let released_handoff_leaf_state_bytes = checked_mul(
        "publication epoch released handoff leaf-state bytes",
        wave.leaf_states.capacity(),
        size_of::<AtomicU8>(),
    )?;
    let retained_shallow_bytes = retained_shallow_bytes_for_capacities(
        wave.slots.capacity(),
        applicable_capacity,
        exceptional_capacity,
        source_state_capacity,
    )?;
    let total_resident_bytes = checked_add(
        "publication epoch total resident bytes",
        wave.stats().retained_event_payload_bytes(),
        retained_shallow_bytes,
    )?;
    // H + E - shared slot buffer: final resident storage already counts the
    // moved slot buffer and deep events once; peak adds only obsolete handoff
    // state bytes plus the input-wave header while new E buffers coexist.
    let compilation_peak_bytes = checked_add(
        "publication epoch compilation peak bytes",
        total_resident_bytes,
        checked_add(
            "publication epoch compilation peak bytes",
            released_handoff_leaf_state_bytes,
            size_of::<ExactPublicationHandoffWave>(),
        )?,
    )?;
    let max_in_flight_source_lease_bytes = checked_mul(
        "publication epoch in-flight source lease bytes",
        max_in_flight_sources,
        size_of::<ExactPublicationEpochSourceLease<'static>>(),
    )?;
    Ok(EpochMemoryEnvelope {
        released_handoff_leaf_state_bytes,
        max_in_flight_source_lease_bytes,
        retained_shallow_bytes,
        total_resident_bytes,
        compilation_peak_bytes,
    })
}

fn enforce_memory_envelope(
    memory: EpochMemoryEnvelope,
    limits: ExactPublicationEpochLimits,
) -> Result<(), ExactPublicationEpochError> {
    for (resource, requested, limit) in [
        (
            "publication epoch retained shallow bytes",
            memory.retained_shallow_bytes,
            limits.max_retained_shallow_bytes,
        ),
        (
            "publication epoch total resident bytes",
            memory.total_resident_bytes,
            limits.max_total_resident_bytes,
        ),
        (
            "publication epoch compilation peak bytes",
            memory.compilation_peak_bytes,
            limits.max_compilation_peak_bytes,
        ),
        (
            "publication epoch in-flight source lease bytes",
            memory.max_in_flight_source_lease_bytes,
            limits.max_in_flight_source_lease_bytes,
        ),
    ] {
        check_limit(resource, requested, limit)?;
    }
    Ok(())
}

fn checked_u64(
    coordinate: &'static str,
    value: usize,
    slot_ordinal: usize,
    leaf_ordinal: usize,
) -> Result<u64, ExactPublicationEpochError> {
    u64::try_from(value).map_err(|_| ExactPublicationEpochError::CoordinateExceedsU64 {
        coordinate,
        slot_ordinal,
        leaf_ordinal,
    })
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ExactPublicationEpochError> {
    left.checked_add(right)
        .ok_or(ExactPublicationEpochError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ExactPublicationEpochError> {
    left.checked_mul(right)
        .ok_or(ExactPublicationEpochError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ExactPublicationEpochError> {
    if requested > limit {
        Err(ExactPublicationEpochError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn try_vec_capacity<T>(
    resource: &'static str,
    requested: usize,
) -> Result<Vec<T>, ExactPublicationEpochError> {
    let mut values = Vec::new();
    values.try_reserve_exact(requested).map_err(|_| {
        ExactPublicationEpochError::AllocationFailure {
            resource,
            requested,
        }
    })?;
    Ok(values)
}

#[cfg(test)]
mod tests {
    use std::mem::{forget, size_of};
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::sync::{Arc, Barrier, mpsc};
    use std::thread;

    use super::super::{
        ExactPublicationHandoffInput, ExactPublicationHandoffLimits, ExactPublicationHandoffWave,
    };
    use super::*;
    use crate::generated_affine_residual_group_exact_publication::{
        PreparedPublication, PublicationLimits,
    };
    use crate::generated_affine_residual_group_exact_publication_tests::ready_for_publication;
    use crate::{
        CampaignPlan, CampaignPlanLimits, CampaignRootSpec, IntegralFamily, IntegralOrderingPolicy,
        SectorMask,
    };

    fn job(family: &IntegralFamily, sector: &SectorMask) -> CampaignJobKey {
        let plan = CampaignPlan::compile(
            [CampaignRootSpec::try_new(
                "epoch-owner-root",
                Arc::new(family.clone()),
                sector.clone(),
            )
            .unwrap()],
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            CampaignPlanLimits::default(),
        )
        .unwrap();
        plan.intrinsic_jobs().next().unwrap().clone()
    }

    fn input(name: &str, lane: usize) -> ExactPublicationHandoffInput {
        let (family, _, mut session, _, ready) = ready_for_publication(name);
        let prepared = PreparedPublication::prepare(ready, PublicationLimits::default()).unwrap();
        let receipt = session.commit_publication(prepared).unwrap();
        let job = job(&family, receipt.event().sector());
        ExactPublicationHandoffInput::new(job, lane, receipt)
    }

    fn handoff(name: &str, lanes_in_input_order: &[usize]) -> ExactPublicationHandoffWave {
        ExactPublicationHandoffWave::compile(
            lanes_in_input_order
                .iter()
                .map(|lane| input(name, *lane))
                .collect(),
            ExactPublicationHandoffLimits::default(),
        )
        .unwrap()
    }

    fn fully_acknowledge(wave: &ExactPublicationHandoffWave) {
        for slot_ordinal in 0..wave.stats().slots() {
            let leaf_count = wave.slot(slot_ordinal).unwrap().leaf_count();
            for leaf_ordinal in 0..leaf_count {
                let locator = wave.locator(slot_ordinal, leaf_ordinal).unwrap();
                wave.acknowledge(wave.issue(locator).unwrap()).unwrap();
            }
        }
        assert_eq!(wave.in_flight_tickets(), 0);
        assert_eq!(wave.state_stats().pending(), 0);
        assert_eq!(wave.state_stats().issued(), 0);
        assert_eq!(wave.state_stats().acknowledged(), wave.stats().leaves());
    }

    fn fully_acknowledged_handoff(
        name: &str,
        lanes_in_input_order: &[usize],
    ) -> ExactPublicationHandoffWave {
        let wave = handoff(name, lanes_in_input_order);
        fully_acknowledge(&wave);
        wave
    }

    #[test]
    fn portable_byte_limit_saturates_at_the_native_usize_boundary() {
        let native_max = usize::MAX as u128;
        assert_eq!(portable_byte_limit(native_max), usize::MAX);
        if native_max < u128::MAX {
            assert_eq!(portable_byte_limit(native_max + 1), usize::MAX);
        }
    }

    fn domain_transcript(domain: CommittedPublicationDomainView<'_>) -> String {
        let premises = domain
            .target_premises()
            .iter()
            .map(|premise| premise.polynomial().to_expression().to_string())
            .collect::<Vec<_>>()
            .join(",");
        let predicates = domain
            .predicates()
            .map(|predicate| {
                format!(
                    "{:?}:{}",
                    predicate.kind(),
                    predicate.polynomial().to_expression()
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("premises=[{premises}]|predicates=[{predicates}]")
    }

    fn exceptional_transcript(view: ExactPublicationEpochExceptionalSourceView<'_>) -> String {
        let key = view.scheduling_key();
        let target = view.target_locator();
        let offset = view
            .target_offset()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let free_positions = view
            .free_positions()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let compact_matrix = view
            .compact_affine_matrix()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{}|{}|{:?}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            key.job().family_id().as_str(),
            key.job().sector(),
            key.job().ordering(),
            key.context_fingerprint(),
            key.closure_epoch_ordinal(),
            key.session_lane_ordinal(),
            key.event_ordinal(),
            key.leaf_ordinal(),
            match view.kind() {
                ExceptionalResidualKind::Domain => "domain",
                ExceptionalResidualKind::SectorLeak => "leak",
            },
            target.solve_ordinal(),
            target.inventory_position(),
            target.case_ordinal(),
            offset,
            view.ambient_arity(),
            free_positions,
            format!(
                "{}|matrix=[{compact_matrix}]",
                domain_transcript(view.domain())
            ),
        )
    }

    #[test]
    fn pending_and_forgotten_issued_handoffs_fail_without_losing_the_wave() {
        let pending = handoff("publication-epoch-pending", &[0]);
        let pending_stats = pending.stats();
        let pending_terms = pending.slot(0).unwrap().event().terms().as_ptr();
        let failure = ExactPublicationEpochOwner::compile(
            pending,
            11,
            ExactPublicationEpochLimits::default(),
        )
        .unwrap_err();
        let (error, pending) = failure.into_parts();
        assert_eq!(
            error,
            ExactPublicationEpochError::HandoffNotFullyAcknowledged {
                pending: pending_stats.leaves(),
                acknowledged: 0,
                expected: pending_stats.leaves(),
            }
        );
        assert_eq!(pending.stats(), pending_stats);
        assert_eq!(
            pending.slot(0).unwrap().event().terms().as_ptr(),
            pending_terms
        );
        fully_acknowledge(&pending);
        let owner = ExactPublicationEpochOwner::compile(
            pending,
            11,
            ExactPublicationEpochLimits::default(),
        )
        .unwrap();
        assert_eq!(owner.stats().slots(), pending_stats.slots());

        let issued = handoff("publication-epoch-forgotten", &[0]);
        let issued_stats = issued.stats();
        let issued_terms = issued.slot(0).unwrap().event().terms().as_ptr();
        let ticket = issued.issue(issued.locator(0, 0).unwrap()).unwrap();
        forget(ticket);
        assert_eq!(issued.state_stats().issued(), 1);
        assert_eq!(issued.in_flight_tickets(), 1);
        let failure =
            ExactPublicationEpochOwner::compile(issued, 12, ExactPublicationEpochLimits::default())
                .unwrap_err();
        let (error, issued) = failure.into_parts();
        assert_eq!(
            error,
            ExactPublicationEpochError::RecoveredStrandedHandoffTickets {
                recovered: 1,
                prior_in_flight: 1,
            }
        );
        assert_eq!(issued.stats(), issued_stats);
        assert_eq!(issued.state_stats().pending(), issued_stats.leaves());
        assert_eq!(issued.state_stats().issued(), 0);
        assert_eq!(issued.state_stats().acknowledged(), 0);
        assert_eq!(issued.in_flight_tickets(), 0);
        assert_eq!(
            issued.slot(0).unwrap().event().terms().as_ptr(),
            issued_terms
        );
        fully_acknowledge(&issued);
        let owner =
            ExactPublicationEpochOwner::compile(issued, 12, ExactPublicationEpochLimits::default())
                .unwrap();
        assert_eq!(
            owner.event_for_test(0).unwrap().terms().as_ptr(),
            issued_terms
        );
    }

    #[test]
    fn accepted_owner_preserves_payload_domains_counts_and_scheduling_scope() {
        const CLOSURE_EPOCH: u64 = 37;
        const LANE: usize = 7;
        let input = input("publication-epoch-preservation", LANE);
        let event = input.event();
        let expected_job = input.job().clone();
        let expected_terms = event.terms().as_ptr();
        let expected_context = event.context_fingerprint().to_owned();
        let expected_event_ordinal = event.event_ordinal();
        let expected_target_premises = event.target_premises().as_ptr();
        let mut expected_applicable = Vec::new();
        let mut expected_exceptional = Vec::new();
        let mut domain_count = 0usize;
        let mut leak_count = 0usize;
        for leaf in event.leaves() {
            match leaf {
                CommittedPublicationLeafView::Applicable(rule) => expected_applicable.push((
                    rule.leaf_ordinal(),
                    rule.domain().relative_case() as *const _ as usize,
                    domain_transcript(rule.domain()),
                )),
                CommittedPublicationLeafView::Exceptional(residual) => {
                    match residual.kind() {
                        ExceptionalResidualKind::Domain => domain_count += 1,
                        ExceptionalResidualKind::SectorLeak => leak_count += 1,
                    }
                    expected_exceptional.push((
                        residual.leaf_ordinal(),
                        residual.kind(),
                        residual.domain().relative_case() as *const _ as usize,
                        domain_transcript(residual.domain()),
                    ));
                }
            }
        }
        assert!(!expected_applicable.is_empty());
        assert!(domain_count > 0);
        assert!(leak_count > 0);

        let wave = ExactPublicationHandoffWave::compile(
            vec![input],
            ExactPublicationHandoffLimits::default(),
        )
        .unwrap();
        fully_acknowledge(&wave);
        let owner = ExactPublicationEpochOwner::compile(
            wave,
            CLOSURE_EPOCH,
            ExactPublicationEpochLimits::default(),
        )
        .unwrap();
        let stats = owner.stats();
        assert_eq!(owner.closure_epoch_ordinal(), CLOSURE_EPOCH);
        assert_eq!(stats.slots(), 1);
        assert_eq!(stats.applicable(), expected_applicable.len());
        assert_eq!(stats.exceptional_domain(), domain_count);
        assert_eq!(stats.exceptional_leak(), leak_count);
        assert_eq!(stats.exceptional(), expected_exceptional.len());
        assert_eq!(
            stats.leaf_visits(),
            3 * (stats.applicable() + stats.exceptional())
        );
        assert_eq!(
            owner.event_for_test(0).unwrap().terms().as_ptr(),
            expected_terms
        );

        for (ordinal, (leaf_ordinal, case_ptr, domain)) in expected_applicable.iter().enumerate() {
            let view = owner.applicable(ordinal).unwrap();
            let key = view.scheduling_key();
            assert_eq!(key.job(), &expected_job);
            assert_eq!(key.context_fingerprint(), expected_context);
            assert_eq!(key.closure_epoch_ordinal(), CLOSURE_EPOCH);
            assert_eq!(key.session_lane_ordinal(), LANE as u64);
            assert_eq!(key.event_ordinal(), expected_event_ordinal as u64);
            assert_eq!(key.leaf_ordinal(), *leaf_ordinal as u64);
            assert_eq!(view.rule().leaf_ordinal(), *leaf_ordinal);
            assert_eq!(view.event().terms().as_ptr(), expected_terms);
            assert_eq!(
                view.domain().target_premises().as_ptr(),
                expected_target_premises
            );
            assert_eq!(
                view.domain().relative_case() as *const _ as usize,
                *case_ptr
            );
            assert_eq!(&domain_transcript(view.domain()), domain);
        }
        assert!(owner.applicable(expected_applicable.len()).is_none());

        for (ordinal, (leaf_ordinal, kind, case_ptr, domain)) in
            expected_exceptional.iter().enumerate()
        {
            let locator = owner.exceptional_source_locator(ordinal).unwrap();
            let mut lease = owner.issue_exceptional_source(locator).unwrap();
            {
                let view = owner.resolve_exceptional_source(&mut lease).unwrap();
                let key = view.scheduling_key();
                assert_eq!(key.job(), &expected_job);
                assert_eq!(key.context_fingerprint(), expected_context);
                assert_eq!(key.closure_epoch_ordinal(), CLOSURE_EPOCH);
                assert_eq!(key.session_lane_ordinal(), LANE as u64);
                assert_eq!(key.event_ordinal(), expected_event_ordinal as u64);
                assert_eq!(key.leaf_ordinal(), *leaf_ordinal as u64);
                assert_eq!(view.kind(), *kind);
                assert_eq!(
                    view.domain().target_premises().as_ptr(),
                    expected_target_premises
                );
                assert_eq!(
                    view.domain().relative_case() as *const _ as usize,
                    *case_ptr
                );
                assert_eq!(&domain_transcript(view.domain()), domain);
            }
            owner.release_exceptional_source(lease).unwrap();
        }
        assert!(
            owner
                .exceptional_source_locator(expected_exceptional.len())
                .is_none()
        );
        assert_eq!(
            owner.source_state_stats(),
            ExactPublicationEpochSourceStateStats {
                pending: expected_exceptional.len(),
                issued: 0,
            }
        );
    }

    #[test]
    fn exceptional_leases_are_bounded_borrowed_and_lossless_on_all_release_paths() {
        let wave = fully_acknowledged_handoff("publication-epoch-leases", &[0]);
        let mut owner = ExactPublicationEpochOwner::compile(
            wave,
            19,
            ExactPublicationEpochLimits {
                max_in_flight_sources: 1,
                ..ExactPublicationEpochLimits::default()
            },
        )
        .unwrap();
        assert!(owner.stats().exceptional() >= 2);
        let first = owner.exceptional_source_locator(0).unwrap();
        let second = owner.exceptional_source_locator(1).unwrap();
        let mut lease = owner.issue_exceptional_source(first).unwrap();
        assert_eq!(owner.in_flight_sources(), 1);
        assert_eq!(
            owner.issue_exceptional_source(second).unwrap_err(),
            ExactPublicationEpochError::InFlightSourceLimit {
                requested: 2,
                limit: 1,
            }
        );
        {
            let view = owner.resolve_exceptional_source(&mut lease).unwrap();
            let key = view.scheduling_key();
            assert_eq!(key.closure_epoch_ordinal(), 19);
            assert_eq!(view.family_fingerprint(), key.job().family_id().as_str());
            assert_eq!(view.context_fingerprint(), key.context_fingerprint());
            assert_eq!(view.sector(), key.job().sector());
            assert_eq!(view.ordering(), key.job().ordering());
        }
        owner.release_exceptional_source(lease).unwrap();
        assert_eq!(owner.in_flight_sources(), 0);
        assert_eq!(owner.source_state_stats().issued(), 0);

        let lease = owner.issue_exceptional_source(first).unwrap();
        drop(lease);
        assert_eq!(owner.in_flight_sources(), 0);
        assert_eq!(
            owner.source_state_stats().pending(),
            owner.stats().exceptional()
        );

        let panic = thread::scope(|scope| {
            scope
                .spawn(|| {
                    let mut lease = owner.issue_exceptional_source(first).unwrap();
                    let _view = owner.resolve_exceptional_source(&mut lease).unwrap();
                    panic!("injected exceptional-source worker panic");
                })
                .join()
        });
        assert!(panic.is_err());
        assert_eq!(owner.in_flight_sources(), 0);
        assert_eq!(
            owner.source_state_stats().pending(),
            owner.stats().exceptional()
        );
        let mut retry = owner.issue_exceptional_source(first).unwrap();
        let before = {
            let view = owner.resolve_exceptional_source(&mut retry).unwrap();
            exceptional_transcript(view)
        };
        owner.release_exceptional_source(retry).unwrap();
        let mut retry = owner.issue_exceptional_source(first).unwrap();
        let after = {
            let view = owner.resolve_exceptional_source(&mut retry).unwrap();
            exceptional_transcript(view)
        };
        assert_eq!(after, before);
        drop(retry);

        let mut stranded = owner.issue_exceptional_source(first).unwrap();
        let stranded_transcript = {
            let view = owner.resolve_exceptional_source(&mut stranded).unwrap();
            exceptional_transcript(view)
        };
        forget(stranded);
        assert_eq!(owner.in_flight_sources(), 1);
        assert_eq!(owner.source_state_stats().issued(), 1);
        assert_eq!(owner.recover_stranded_exceptional_sources().unwrap(), 1);
        assert_eq!(owner.in_flight_sources(), 0);
        assert_eq!(owner.source_state_stats().issued(), 0);
        let first = owner.exceptional_source_locator(0).unwrap();
        let mut retry = owner.issue_exceptional_source(first).unwrap();
        let recovered_transcript = {
            let view = owner.resolve_exceptional_source(&mut retry).unwrap();
            exceptional_transcript(view)
        };
        assert_eq!(recovered_transcript, stranded_transcript);
        owner.release_exceptional_source(retry).unwrap();

        let foreign_wave = fully_acknowledged_handoff("publication-epoch-foreign-leases", &[0]);
        let foreign_owner = ExactPublicationEpochOwner::compile(
            foreign_wave,
            20,
            ExactPublicationEpochLimits::default(),
        )
        .unwrap();
        assert_eq!(
            owner
                .issue_exceptional_source(foreign_owner.exceptional_source_locator(0).unwrap())
                .unwrap_err(),
            ExactPublicationEpochError::ForeignLocator
        );
        let mut foreign_lease = foreign_owner
            .issue_exceptional_source(foreign_owner.exceptional_source_locator(0).unwrap())
            .unwrap();
        assert_eq!(
            owner
                .resolve_exceptional_source(&mut foreign_lease)
                .unwrap_err(),
            ExactPublicationEpochError::ForeignLease
        );
        assert_eq!(
            owner.release_exceptional_source(foreign_lease).unwrap_err(),
            ExactPublicationEpochError::ForeignLease
        );
        assert_eq!(foreign_owner.in_flight_sources(), 0);
        assert_eq!(foreign_owner.source_state_stats().issued(), 0);

        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<ExactPublicationEpochSourceLease<'static>>();
        assert_sync::<ExactPublicationEpochOwner>();
        assert_eq!(size_of::<AtomicU8>(), size_of::<u8>());
    }

    #[test]
    fn exact_and_every_positive_resource_one_below_are_transactional_and_retryable() {
        const NAME: &str = "publication-epoch-limits";
        let pilot_wave = fully_acknowledged_handoff(NAME, &[0]);
        let pilot = ExactPublicationEpochOwner::compile(
            pilot_wave,
            41,
            ExactPublicationEpochLimits::default(),
        )
        .unwrap();
        let stats = pilot.stats();
        assert!(stats.slots() > 0);
        assert!(stats.leaf_visits() > 0);
        assert!(stats.applicable() > 0);
        assert!(stats.exceptional() > 0);
        assert!(stats.max_in_flight_sources() > 0);
        assert!(stats.max_in_flight_source_lease_bytes() > 0);
        assert!(stats.transferred_event_payload_bytes() > 0);
        assert!(stats.retained_shallow_bytes() > 0);
        assert!(stats.total_resident_bytes() > 0);
        assert!(stats.compilation_peak_bytes() > 0);
        let exact = ExactPublicationEpochLimits {
            max_slots: stats.slots(),
            max_leaf_visits: stats.leaf_visits(),
            max_applicable_leaves: stats.applicable(),
            max_exceptional_sources: stats.exceptional(),
            max_in_flight_sources: stats.max_in_flight_sources(),
            max_in_flight_source_lease_bytes: stats.max_in_flight_source_lease_bytes(),
            max_transferred_event_payload_bytes: stats.transferred_event_payload_bytes(),
            max_retained_shallow_bytes: stats.retained_shallow_bytes(),
            max_total_resident_bytes: stats.total_resident_bytes(),
            max_compilation_peak_bytes: stats.compilation_peak_bytes(),
        };
        let exact_owner =
            ExactPublicationEpochOwner::compile(fully_acknowledged_handoff(NAME, &[0]), 41, exact)
                .unwrap();
        assert_eq!(exact_owner.stats(), stats);

        let one_below = [
            ExactPublicationEpochLimits {
                max_slots: exact.max_slots - 1,
                ..exact
            },
            ExactPublicationEpochLimits {
                max_leaf_visits: exact.max_leaf_visits - 1,
                ..exact
            },
            ExactPublicationEpochLimits {
                max_applicable_leaves: exact.max_applicable_leaves - 1,
                ..exact
            },
            ExactPublicationEpochLimits {
                max_exceptional_sources: exact.max_exceptional_sources - 1,
                ..exact
            },
            ExactPublicationEpochLimits {
                max_in_flight_source_lease_bytes: exact.max_in_flight_source_lease_bytes - 1,
                ..exact
            },
            ExactPublicationEpochLimits {
                max_transferred_event_payload_bytes: exact.max_transferred_event_payload_bytes - 1,
                ..exact
            },
            ExactPublicationEpochLimits {
                max_retained_shallow_bytes: exact.max_retained_shallow_bytes - 1,
                ..exact
            },
            ExactPublicationEpochLimits {
                max_total_resident_bytes: exact.max_total_resident_bytes - 1,
                ..exact
            },
            ExactPublicationEpochLimits {
                max_compilation_peak_bytes: exact.max_compilation_peak_bytes - 1,
                ..exact
            },
        ];
        for limits in one_below {
            let wave = fully_acknowledged_handoff(NAME, &[0]);
            let wave_stats = wave.stats();
            let terms = wave.slot(0).unwrap().event().terms().as_ptr();
            let failure = ExactPublicationEpochOwner::compile(wave, 41, limits).unwrap_err();
            assert!(matches!(
                failure.error(),
                ExactPublicationEpochError::ResourceLimit { .. }
            ));
            let (_, returned) = failure.into_parts();
            assert_eq!(returned.stats(), wave_stats);
            assert_eq!(returned.state_stats().acknowledged(), wave_stats.leaves());
            assert_eq!(returned.slot(0).unwrap().event().terms().as_ptr(), terms);
            assert_eq!(
                ExactPublicationEpochOwner::compile(returned, 41, exact)
                    .unwrap()
                    .stats(),
                stats
            );
        }

        // Worker width is a tunable ceiling, not a minimum compile resource.
        // Lowering it must reduce admitted lease bytes instead of rejecting
        // the immutable owner.
        if exact.max_in_flight_sources > 1 {
            let reduced_width = exact.max_in_flight_sources - 1;
            let owner = ExactPublicationEpochOwner::compile(
                fully_acknowledged_handoff(NAME, &[0]),
                41,
                ExactPublicationEpochLimits {
                    max_in_flight_sources: reduced_width,
                    max_in_flight_source_lease_bytes: reduced_width
                        * size_of::<ExactPublicationEpochSourceLease<'static>>(),
                    ..exact
                },
            )
            .unwrap();
            assert_eq!(owner.stats().max_in_flight_sources(), reduced_width);
        }
    }

    fn parallel_exceptional_transcript(width: usize, shuffled: bool) -> Vec<String> {
        assert!((1..=4).contains(&width));
        let lanes = if shuffled {
            vec![2, 0, 1]
        } else {
            vec![0, 1, 2]
        };
        let wave = fully_acknowledged_handoff("publication-epoch-parallel", &lanes);
        let owner = ExactPublicationEpochOwner::compile(
            wave,
            53,
            ExactPublicationEpochLimits {
                max_in_flight_sources: width,
                ..ExactPublicationEpochLimits::default()
            },
        )
        .unwrap();
        let mut next_source = 0usize;
        let mut transcript = Vec::with_capacity(owner.stats().exceptional());
        while next_source < owner.stats().exceptional() {
            let end = owner
                .stats()
                .exceptional()
                .min(next_source.saturating_add(width));
            let leases = (next_source..end)
                .map(|source_ordinal| {
                    owner
                        .issue_exceptional_source(
                            owner.exceptional_source_locator(source_ordinal).unwrap(),
                        )
                        .unwrap()
                })
                .collect::<Vec<_>>();
            assert!(owner.in_flight_sources() <= width);
            let mut completed = thread::scope(|scope| {
                let barrier = Arc::new(Barrier::new(leases.len()));
                let completion_turn = Arc::new(AtomicUsize::new(leases.len() - 1));
                let (sender, receiver) = mpsc::channel();
                for (worker_ordinal, mut lease) in leases.into_iter().enumerate() {
                    let barrier = Arc::clone(&barrier);
                    let completion_turn = Arc::clone(&completion_turn);
                    let sender = sender.clone();
                    let owner = &owner;
                    scope.spawn(move || {
                        barrier.wait();
                        let entry = {
                            let view = owner.resolve_exceptional_source(&mut lease).unwrap();
                            exceptional_transcript(view)
                        };
                        barrier.wait();
                        while completion_turn.load(AtomicOrdering::Acquire) != worker_ordinal {
                            thread::yield_now();
                        }
                        sender
                            .send((worker_ordinal, lease.source_ordinal(), entry, lease))
                            .unwrap();
                        if worker_ordinal > 0 {
                            completion_turn.store(worker_ordinal - 1, AtomicOrdering::Release);
                        }
                    });
                }
                drop(sender);
                receiver.into_iter().collect::<Vec<_>>()
            });
            assert_eq!(
                completed
                    .iter()
                    .map(|(worker_ordinal, _, _, _)| *worker_ordinal)
                    .collect::<Vec<_>>(),
                (0..completed.len()).rev().collect::<Vec<_>>()
            );
            completed.sort_by_key(|(_, source_ordinal, _, _)| *source_ordinal);
            for (_, source_ordinal, entry, lease) in completed {
                assert_eq!(source_ordinal, transcript.len());
                transcript.push(entry);
                owner.release_exceptional_source(lease).unwrap();
            }
            next_source = end;
        }
        assert_eq!(owner.in_flight_sources(), 0);
        assert_eq!(owner.source_state_stats().issued(), 0);
        assert_eq!(
            owner.source_state_stats().pending(),
            owner.stats().exceptional()
        );
        transcript
    }

    #[test]
    fn shuffled_inputs_and_one_two_four_reverse_completion_workers_are_canonical() {
        let serial = parallel_exceptional_transcript(1, false);
        assert!(!serial.is_empty());
        assert_eq!(parallel_exceptional_transcript(1, true), serial);
        assert_eq!(parallel_exceptional_transcript(2, true), serial);
        assert_eq!(parallel_exceptional_transcript(4, true), serial);
    }
}
