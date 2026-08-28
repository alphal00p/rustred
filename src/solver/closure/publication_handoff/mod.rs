//! Frozen, shallow handoff of committed exact publication leaves.
//!
//! This module is deliberately algebra-free. It moves one committed event
//! handle into each canonical publication slot and stores one byte of mutable
//! handoff state per leaf. Relation terms, loci, cases, and predicates remain
//! owned exactly once by the committed event. Newly discovered work cannot be
//! inserted into a compiled wave; it belongs to a later frozen epoch.
//!
//! The wave gates its incremental shallow allocation, the retained event
//! allocations transferred by its input receipts, and the number of live
//! fixed-size tickets. Shared session authority, family plans, and catalogs
//! are campaign-baseline owners and must be deduplicated there. Admission of
//! worker result buffers and downstream exceptional-source owners belongs to
//! the future production coordinator; this handoff does not claim that layer.

pub(in crate::solver::closure) mod publication_epoch_owner;

use std::cmp::Ordering;
use std::fmt;
use std::mem::size_of;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering as AtomicOrdering};

use crate::campaign::CampaignJobKey;
use crate::solver::exact_session::{
    CommittedPublicationEventHandle, CommittedPublicationEventView, CommittedPublicationLeafView,
    ExceptionalResidualKind, PublicationReceipt,
};

const LEAF_PENDING: u8 = 0;
const LEAF_ISSUED: u8 = 1;
const LEAF_ACKNOWLEDGED: u8 = 2;

const _: () = assert!(size_of::<AtomicU8>() == 1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactPublicationHandoffLimits {
    pub(crate) max_slots: usize,
    pub(crate) max_leaves: usize,
    /// Hard ceiling on live issued-but-unacknowledged tickets.
    pub(crate) max_in_flight_tickets: usize,
    /// Complete retained-event census carried across the owner handoff.
    ///
    /// The permissive default is suitable for internal tests only. A
    /// production campaign must derive a finite value from its resident-memory
    /// envelope before it hydrates this wave.
    pub(crate) max_retained_event_payload_bytes: usize,
    /// Maximum incremental shallow storage retained by this wave.
    ///
    /// Deep event payloads already owned by the input receipts are excluded.
    pub(crate) max_retained_shallow_bytes: usize,
    /// Maximum incremental compile-time storage attributable to this wave.
    ///
    /// This includes retained shallow storage and compiler scratch, but not
    /// the pre-existing receipt payloads supplied by the caller.
    pub(crate) max_compilation_peak_bytes: usize,
}

impl Default for ExactPublicationHandoffLimits {
    fn default() -> Self {
        Self {
            max_slots: 1_000_000,
            max_leaves: 64_000_000,
            max_in_flight_tickets: 4_096,
            max_retained_event_payload_bytes: usize::MAX,
            max_retained_shallow_bytes: 2 * 1024 * 1024 * 1024,
            max_compilation_peak_bytes: 3 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ExactPublicationHandoffStats {
    slots: usize,
    leaves: usize,
    applicable: usize,
    exceptional_domain: usize,
    exceptional_leak: usize,
    retained_event_payload_bytes: usize,
    max_in_flight_ticket_bytes: usize,
    /// Incremental shallow bytes retained by the compiled wave.
    retained_shallow_bytes: usize,
    /// Incremental wave-owned bytes at compilation peak.
    compilation_peak_bytes: usize,
}

impl ExactPublicationHandoffStats {
    pub(crate) const fn slots(self) -> usize {
        self.slots
    }

    pub(crate) const fn leaves(self) -> usize {
        self.leaves
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

    /// Sum of the duplicate-rejected committed event allocations kept alive
    /// by this wave. A campaign resident-owner transfer must preserve this
    /// charge; if a live session shares an event, it must deduplicate rather
    /// than silently release or double-count it.
    pub(crate) const fn retained_event_payload_bytes(self) -> usize {
        self.retained_event_payload_bytes
    }

    pub(crate) const fn max_in_flight_ticket_bytes(self) -> usize {
        self.max_in_flight_ticket_bytes
    }

    pub(crate) const fn retained_shallow_bytes(self) -> usize {
        self.retained_shallow_bytes
    }

    pub(crate) const fn compilation_peak_bytes(self) -> usize {
        self.compilation_peak_bytes
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ExactPublicationHandoffStateStats {
    pending: usize,
    issued: usize,
    acknowledged: usize,
}

impl ExactPublicationHandoffStateStats {
    pub(crate) const fn pending(self) -> usize {
        self.pending
    }

    pub(crate) const fn issued(self) -> usize {
        self.issued
    }

    pub(crate) const fn acknowledged(self) -> usize {
        self.acknowledged
    }
}

/// Move-only input for one committed publication event.
///
/// `session_lane_ordinal` is assigned by the frozen-epoch coordinator. It is
/// a stable scheduling coordinate, not a substitute for the private session
/// authority retained by the receipt.
pub(crate) struct ExactPublicationHandoffInput {
    job: CampaignJobKey,
    session_lane_ordinal: usize,
    receipt: PublicationReceipt,
}

impl ExactPublicationHandoffInput {
    pub(crate) const fn new(
        job: CampaignJobKey,
        session_lane_ordinal: usize,
        receipt: PublicationReceipt,
    ) -> Self {
        Self {
            job,
            session_lane_ordinal,
            receipt,
        }
    }

    pub(crate) const fn job(&self) -> &CampaignJobKey {
        &self.job
    }

    pub(crate) const fn session_lane_ordinal(&self) -> usize {
        self.session_lane_ordinal
    }

    pub(crate) fn event(&self) -> CommittedPublicationEventView<'_> {
        self.receipt.event()
    }
}

impl fmt::Debug for ExactPublicationHandoffInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactPublicationHandoffInput")
            .field("job", &self.job)
            .field("session_lane_ordinal", &self.session_lane_ordinal)
            .field("event_ordinal", &self.receipt.event_ordinal())
            .field("private_receipt", &"<redacted>")
            .finish()
    }
}

struct ExactPublicationHandoffSlot {
    job: CampaignJobKey,
    session_lane_ordinal: usize,
    database_epoch: usize,
    group_ordinal: usize,
    event_ordinal: usize,
    first_leaf_state: usize,
    leaf_count: usize,
    retained_event_bytes: usize,
    event: CommittedPublicationEventHandle,
}

#[derive(Clone, Copy)]
pub(crate) struct ExactPublicationHandoffSlotView<'wave> {
    slot_ordinal: usize,
    slot: &'wave ExactPublicationHandoffSlot,
}

impl<'wave> ExactPublicationHandoffSlotView<'wave> {
    pub(crate) const fn slot_ordinal(self) -> usize {
        self.slot_ordinal
    }

    pub(crate) const fn job(self) -> &'wave CampaignJobKey {
        &self.slot.job
    }

    pub(crate) const fn session_lane_ordinal(self) -> usize {
        self.slot.session_lane_ordinal
    }

    pub(crate) const fn database_epoch(self) -> usize {
        self.slot.database_epoch
    }

    pub(crate) const fn group_ordinal(self) -> usize {
        self.slot.group_ordinal
    }

    pub(crate) const fn event_ordinal(self) -> usize {
        self.slot.event_ordinal
    }

    pub(crate) const fn leaf_count(self) -> usize {
        self.slot.leaf_count
    }

    pub(crate) const fn retained_event_bytes(self) -> usize {
        self.slot.retained_event_bytes
    }

    #[cfg(test)]
    pub(crate) fn event(self) -> CommittedPublicationEventView<'wave> {
        self.slot.event.view()
    }
}

impl fmt::Debug for ExactPublicationHandoffSlotView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactPublicationHandoffSlotView")
            .field("slot_ordinal", &self.slot_ordinal)
            .field("job", &self.slot.job)
            .field("session_lane_ordinal", &self.slot.session_lane_ordinal)
            .field("database_epoch", &self.slot.database_epoch)
            .field("group_ordinal", &self.slot.group_ordinal)
            .field("event_ordinal", &self.slot.event_ordinal)
            .field("leaf_count", &self.slot.leaf_count)
            .field("private_event", &"<redacted>")
            .finish()
    }
}

/// Borrowed canonical address used only to request a move-only ticket.
#[derive(Clone, Copy)]
pub(crate) struct ExactPublicationHandoffLocator<'wave> {
    wave: &'wave ExactPublicationHandoffWave,
    slot_ordinal: usize,
    leaf_ordinal: usize,
}

impl ExactPublicationHandoffLocator<'_> {
    pub(crate) const fn slot_ordinal(self) -> usize {
        self.slot_ordinal
    }

    pub(crate) const fn leaf_ordinal(self) -> usize {
        self.leaf_ordinal
    }
}

impl fmt::Debug for ExactPublicationHandoffLocator<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactPublicationHandoffLocator")
            .field("slot_ordinal", &self.slot_ordinal)
            .field("leaf_ordinal", &self.leaf_ordinal)
            .finish_non_exhaustive()
    }
}

/// Non-cloneable borrowed proof that one leaf was issued by one live wave.
#[must_use = "dropping an unacknowledged handoff ticket returns its leaf to pending"]
pub(crate) struct ExactPublicationHandoffTicket<'wave> {
    wave: &'wave ExactPublicationHandoffWave,
    slot_ordinal: usize,
    leaf_ordinal: usize,
}

impl ExactPublicationHandoffTicket<'_> {
    pub(crate) const fn slot_ordinal(&self) -> usize {
        self.slot_ordinal
    }

    pub(crate) const fn leaf_ordinal(&self) -> usize {
        self.leaf_ordinal
    }
}

impl Drop for ExactPublicationHandoffTicket<'_> {
    fn drop(&mut self) {
        let state_index = self
            .wave
            .leaf_state_index(self.slot_ordinal, self.leaf_ordinal)
            .expect("issued publication handoff ticket lost its leaf");
        if self.wave.leaf_states[state_index]
            .compare_exchange(
                LEAF_ISSUED,
                LEAF_PENDING,
                AtomicOrdering::AcqRel,
                AtomicOrdering::Acquire,
            )
            .is_ok()
        {
            self.wave.release_in_flight_ticket();
        }
    }
}

impl fmt::Debug for ExactPublicationHandoffTicket<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactPublicationHandoffTicket")
            .field("slot_ordinal", &self.slot_ordinal)
            .field("leaf_ordinal", &self.leaf_ordinal)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExactPublicationHandoffError {
    NoInputs,
    JobScopeMismatch {
        input_ordinal: usize,
    },
    ContextScopeMismatch {
        first_input_ordinal: usize,
        second_input_ordinal: usize,
    },
    DuplicateEvent {
        first_input_ordinal: usize,
        second_input_ordinal: usize,
    },
    SessionLaneCollision,
    SessionAuthorityLaneMismatch,
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
    ForeignTicket,
    UnknownSlot,
    UnknownLeaf,
    NotIssued,
    AlreadyIssued,
    AlreadyAcknowledged,
    InFlightTicketLimit {
        requested: usize,
        limit: usize,
    },
}

impl fmt::Display for ExactPublicationHandoffError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoInputs => formatter.write_str("publication handoff wave has no inputs"),
            Self::JobScopeMismatch { input_ordinal } => write!(
                formatter,
                "publication handoff input {input_ordinal} does not belong to its campaign job"
            ),
            Self::ContextScopeMismatch { .. } => formatter.write_str(
                "publication handoff campaign job contains distinct coefficient contexts",
            ),
            Self::DuplicateEvent { .. } => {
                formatter.write_str("publication handoff contains the same committed event twice")
            }
            Self::SessionLaneCollision => formatter.write_str(
                "publication handoff stable session-lane key names distinct session authorities",
            ),
            Self::SessionAuthorityLaneMismatch => formatter.write_str(
                "publication handoff assigns one session authority multiple stable lane keys",
            ),
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
            Self::ForeignLocator => formatter.write_str("publication handoff locator is foreign"),
            Self::ForeignTicket => formatter.write_str("publication handoff ticket is foreign"),
            Self::UnknownSlot => formatter.write_str("publication handoff slot is out of range"),
            Self::UnknownLeaf => formatter.write_str("publication handoff leaf is out of range"),
            Self::NotIssued => formatter.write_str("publication handoff leaf is not issued"),
            Self::AlreadyIssued => {
                formatter.write_str("publication handoff leaf was already issued")
            }
            Self::AlreadyAcknowledged => {
                formatter.write_str("publication handoff leaf was already acknowledged")
            }
            Self::InFlightTicketLimit { requested, limit } => write!(
                formatter,
                "publication handoff requested {requested} in-flight tickets, configured limit is {limit}"
            ),
        }
    }
}

impl std::error::Error for ExactPublicationHandoffError {}

/// Transactional compilation failure retaining every move-only input owner.
pub(crate) struct ExactPublicationHandoffFailure {
    error: ExactPublicationHandoffError,
    inputs: Vec<ExactPublicationHandoffInput>,
}

impl ExactPublicationHandoffFailure {
    pub(crate) const fn error(&self) -> ExactPublicationHandoffError {
        self.error
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        ExactPublicationHandoffError,
        Vec<ExactPublicationHandoffInput>,
    ) {
        (self.error, self.inputs)
    }
}

impl fmt::Debug for ExactPublicationHandoffFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactPublicationHandoffFailure")
            .field("error", &self.error)
            .field("input_count", &self.inputs.len())
            .field("private_inputs", &"<redacted>")
            .finish()
    }
}

pub(crate) struct ExactPublicationHandoffWave {
    slots: Vec<ExactPublicationHandoffSlot>,
    leaf_states: Vec<AtomicU8>,
    in_flight_tickets: AtomicUsize,
    limits: ExactPublicationHandoffLimits,
    stats: ExactPublicationHandoffStats,
}

impl ExactPublicationHandoffWave {
    pub(crate) fn compile(
        mut inputs: Vec<ExactPublicationHandoffInput>,
        limits: ExactPublicationHandoffLimits,
    ) -> Result<Self, ExactPublicationHandoffFailure> {
        match compile_handoff(&mut inputs, limits) {
            Ok(prepared) => Ok(prepared.finish(inputs)),
            Err(error) => Err(ExactPublicationHandoffFailure { error, inputs }),
        }
    }

    pub(crate) const fn limits(&self) -> ExactPublicationHandoffLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> ExactPublicationHandoffStats {
        self.stats
    }

    pub(crate) fn in_flight_tickets(&self) -> usize {
        self.in_flight_tickets.load(AtomicOrdering::Acquire)
    }

    /// Barrier/quiescent diagnostic snapshot. Concurrent loads do not form a
    /// single atomic global snapshot while tickets are changing state.
    pub(crate) fn state_stats(&self) -> ExactPublicationHandoffStateStats {
        let mut stats = ExactPublicationHandoffStateStats::default();
        for state in &self.leaf_states {
            match state.load(AtomicOrdering::Acquire) {
                LEAF_PENDING => stats.pending += 1,
                LEAF_ISSUED => stats.issued += 1,
                LEAF_ACKNOWLEDGED => stats.acknowledged += 1,
                _ => unreachable!("sealed publication handoff has an invalid leaf state"),
            }
        }
        stats
    }

    pub(crate) fn slot(&self, ordinal: usize) -> Option<ExactPublicationHandoffSlotView<'_>> {
        Some(ExactPublicationHandoffSlotView {
            slot_ordinal: ordinal,
            slot: self.slots.get(ordinal)?,
        })
    }

    pub(crate) fn locator(
        &self,
        slot_ordinal: usize,
        leaf_ordinal: usize,
    ) -> Option<ExactPublicationHandoffLocator<'_>> {
        let slot = self.slots.get(slot_ordinal)?;
        (leaf_ordinal < slot.leaf_count).then_some(ExactPublicationHandoffLocator {
            wave: self,
            slot_ordinal,
            leaf_ordinal,
        })
    }

    pub(crate) fn issue<'wave>(
        &'wave self,
        locator: ExactPublicationHandoffLocator<'_>,
    ) -> Result<ExactPublicationHandoffTicket<'wave>, ExactPublicationHandoffError> {
        if !std::ptr::eq(self, locator.wave) {
            return Err(ExactPublicationHandoffError::ForeignLocator);
        }
        let state_index = self.leaf_state_index(locator.slot_ordinal, locator.leaf_ordinal)?;
        self.reserve_in_flight_ticket()?;
        match self.leaf_states[state_index].compare_exchange(
            LEAF_PENDING,
            LEAF_ISSUED,
            AtomicOrdering::AcqRel,
            AtomicOrdering::Acquire,
        ) {
            Ok(LEAF_PENDING) => {}
            Err(LEAF_ISSUED) => {
                self.release_in_flight_ticket();
                return Err(ExactPublicationHandoffError::AlreadyIssued);
            }
            Err(LEAF_ACKNOWLEDGED) => {
                self.release_in_flight_ticket();
                return Err(ExactPublicationHandoffError::AlreadyAcknowledged);
            }
            _ => unreachable!("sealed publication handoff has an invalid leaf state"),
        }
        Ok(ExactPublicationHandoffTicket {
            wave: self,
            slot_ordinal: locator.slot_ordinal,
            leaf_ordinal: locator.leaf_ordinal,
        })
    }

    /// Borrow one leaf only for as long as its live ticket remains borrowed.
    /// Tying the returned view to this common borrow prevents safe code from
    /// releasing the admission permit while retaining access to the event.
    pub(crate) fn resolve<'view>(
        &'view self,
        ticket: &'view mut ExactPublicationHandoffTicket<'_>,
    ) -> Result<CommittedPublicationLeafView<'view>, ExactPublicationHandoffError> {
        if !std::ptr::eq(self, ticket.wave) {
            return Err(ExactPublicationHandoffError::ForeignTicket);
        }
        let state_index = self.leaf_state_index(ticket.slot_ordinal, ticket.leaf_ordinal)?;
        match self.leaf_states[state_index].load(AtomicOrdering::Acquire) {
            LEAF_ISSUED => {}
            LEAF_PENDING => return Err(ExactPublicationHandoffError::NotIssued),
            LEAF_ACKNOWLEDGED => return Err(ExactPublicationHandoffError::AlreadyAcknowledged),
            _ => unreachable!("sealed publication handoff has an invalid leaf state"),
        }
        self.slots[ticket.slot_ordinal]
            .event
            .view()
            .leaf(ticket.leaf_ordinal)
            .ok_or(ExactPublicationHandoffError::UnknownLeaf)
    }

    /// Record only acceptance of the handoff by the designated consumer.
    /// This does not assert application, discharge, coverage, terminal status,
    /// zero, or master status.
    pub(crate) fn acknowledge(
        &self,
        ticket: ExactPublicationHandoffTicket<'_>,
    ) -> Result<(), ExactPublicationHandoffError> {
        if !std::ptr::eq(self, ticket.wave) {
            return Err(ExactPublicationHandoffError::ForeignTicket);
        }
        let state_index = self.leaf_state_index(ticket.slot_ordinal, ticket.leaf_ordinal)?;
        match self.leaf_states[state_index].compare_exchange(
            LEAF_ISSUED,
            LEAF_ACKNOWLEDGED,
            AtomicOrdering::AcqRel,
            AtomicOrdering::Acquire,
        ) {
            Ok(LEAF_ISSUED) => {
                self.release_in_flight_ticket();
                Ok(())
            }
            Err(LEAF_PENDING) => Err(ExactPublicationHandoffError::NotIssued),
            Err(LEAF_ACKNOWLEDGED) => Err(ExactPublicationHandoffError::AlreadyAcknowledged),
            _ => unreachable!("sealed publication handoff has an invalid leaf state"),
        }
    }

    fn leaf_state_index(
        &self,
        slot_ordinal: usize,
        leaf_ordinal: usize,
    ) -> Result<usize, ExactPublicationHandoffError> {
        let slot = self
            .slots
            .get(slot_ordinal)
            .ok_or(ExactPublicationHandoffError::UnknownSlot)?;
        if leaf_ordinal >= slot.leaf_count {
            return Err(ExactPublicationHandoffError::UnknownLeaf);
        }
        slot.first_leaf_state
            .checked_add(leaf_ordinal)
            .ok_or(ExactPublicationHandoffError::UnknownLeaf)
    }

    fn reserve_in_flight_ticket(&self) -> Result<(), ExactPublicationHandoffError> {
        loop {
            let current = self.in_flight_tickets.load(AtomicOrdering::Acquire);
            let requested = current.checked_add(1).ok_or(
                ExactPublicationHandoffError::ResourceCountOverflow {
                    resource: "publication handoff in-flight tickets",
                },
            )?;
            if requested > self.limits.max_in_flight_tickets {
                return Err(ExactPublicationHandoffError::InFlightTicketLimit {
                    requested,
                    limit: self.limits.max_in_flight_tickets,
                });
            }
            if self
                .in_flight_tickets
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

    fn release_in_flight_ticket(&self) {
        self.in_flight_tickets
            .fetch_update(AtomicOrdering::AcqRel, AtomicOrdering::Acquire, |current| {
                current.checked_sub(1)
            })
            .expect("publication handoff released an unreserved in-flight ticket");
    }
}

impl fmt::Debug for ExactPublicationHandoffWave {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactPublicationHandoffWave")
            .field("stats", &self.stats)
            .field("state_stats", &self.state_stats())
            .field("limits", &self.limits)
            .field("private_slots", &"<redacted>")
            .finish()
    }
}

struct PreparedHandoff {
    slots: Vec<ExactPublicationHandoffSlot>,
    leaf_states: Vec<AtomicU8>,
    limits: ExactPublicationHandoffLimits,
    stats: ExactPublicationHandoffStats,
}

impl PreparedHandoff {
    /// Move the receipts only after every fallible validation and allocation
    /// has completed, so failures retain the caller's original input order.
    fn finish(
        mut self,
        mut inputs: Vec<ExactPublicationHandoffInput>,
    ) -> ExactPublicationHandoffWave {
        inputs.sort_unstable_by(compare_inputs);
        let mut first_leaf_state = 0usize;
        for input in inputs {
            let event = input.receipt.event();
            let database_epoch = event.database_epoch();
            let group_ordinal = event.group_ordinal();
            let event_ordinal = event.event_ordinal();
            let leaf_count = event.leaf_count();
            let retained_event_bytes = input.receipt.retained_event_bytes();
            let event = input.receipt.into_event_handle();
            self.slots.push(ExactPublicationHandoffSlot {
                job: input.job,
                session_lane_ordinal: input.session_lane_ordinal,
                database_epoch,
                group_ordinal,
                event_ordinal,
                first_leaf_state,
                leaf_count,
                retained_event_bytes,
                event,
            });
            first_leaf_state += leaf_count;
        }
        debug_assert_eq!(self.slots.len(), self.stats.slots);
        debug_assert_eq!(first_leaf_state, self.stats.leaves);
        ExactPublicationHandoffWave {
            slots: self.slots,
            leaf_states: self.leaf_states,
            in_flight_tickets: AtomicUsize::new(0),
            limits: self.limits,
            stats: self.stats,
        }
    }
}

fn compile_handoff(
    inputs: &mut [ExactPublicationHandoffInput],
    limits: ExactPublicationHandoffLimits,
) -> Result<PreparedHandoff, ExactPublicationHandoffError> {
    if inputs.is_empty() {
        return Err(ExactPublicationHandoffError::NoInputs);
    }
    check_limit("publication handoff slots", inputs.len(), limits.max_slots)?;

    let mut leaves = 0usize;
    let mut applicable = 0usize;
    let mut exceptional_domain = 0usize;
    let mut exceptional_leak = 0usize;
    let mut retained_event_payload_bytes = 0usize;
    for (input_ordinal, input) in inputs.iter().enumerate() {
        let event = input.event();
        if input.job.family_id().as_str() != event.family_fingerprint()
            || input.job.sector() != event.sector()
            || input.job.ordering() != event.ordering()
        {
            return Err(ExactPublicationHandoffError::JobScopeMismatch { input_ordinal });
        }
        leaves = checked_add("publication handoff leaves", leaves, event.leaf_count())?;
        retained_event_payload_bytes = checked_add(
            "publication handoff retained event payload bytes",
            retained_event_payload_bytes,
            input.receipt.retained_event_bytes(),
        )?;
        for leaf in event.leaves() {
            match leaf {
                CommittedPublicationLeafView::Applicable(_) => {
                    applicable =
                        checked_add("publication handoff applicable leaves", applicable, 1)?;
                }
                CommittedPublicationLeafView::Exceptional(residual) => match residual.kind() {
                    ExceptionalResidualKind::Domain => {
                        exceptional_domain = checked_add(
                            "publication handoff exceptional-domain leaves",
                            exceptional_domain,
                            1,
                        )?;
                    }
                    ExceptionalResidualKind::SectorLeak => {
                        exceptional_leak = checked_add(
                            "publication handoff exceptional-leak leaves",
                            exceptional_leak,
                            1,
                        )?;
                    }
                },
            }
        }
    }
    check_limit("publication handoff leaves", leaves, limits.max_leaves)?;
    check_limit(
        "publication handoff retained event payload bytes",
        retained_event_payload_bytes,
        limits.max_retained_event_payload_bytes,
    )?;

    // Check the requested incremental allocation envelope before reserving
    // any compiler or wave-owned buffer. Once all reservations succeed, their
    // actual allocator capacities are checked again before any receipt moves.
    let prospective_retained_shallow_bytes =
        retained_shallow_bytes_for_capacities(inputs.len(), leaves)?;
    check_limit(
        "publication handoff retained shallow bytes",
        prospective_retained_shallow_bytes,
        limits.max_retained_shallow_bytes,
    )?;
    let prospective_temporary_bytes =
        compilation_temporary_bytes_for_capacities(inputs.len(), inputs.len(), inputs.len())?;
    let prospective_compilation_peak_bytes = checked_add(
        "publication handoff compilation peak bytes",
        prospective_retained_shallow_bytes,
        prospective_temporary_bytes,
    )?;
    check_limit(
        "publication handoff compilation peak bytes",
        prospective_compilation_peak_bytes,
        limits.max_compilation_peak_bytes,
    )?;
    let max_in_flight_ticket_bytes = checked_mul(
        "publication handoff in-flight ticket bytes",
        limits.max_in_flight_tickets.min(leaves),
        size_of::<ExactPublicationHandoffTicket<'static>>(),
    )?;

    let mut order = try_vec_capacity::<usize>("publication handoff sort order", inputs.len())?;
    order.extend(0..inputs.len());
    order.sort_unstable_by(|left, right| compare_inputs(&inputs[*left], &inputs[*right]));

    let mut event_identities =
        try_vec_capacity::<(usize, usize)>("publication handoff event identities", inputs.len())?;
    let mut session_identities =
        try_vec_capacity::<(usize, usize)>("publication handoff session identities", inputs.len())?;
    for (input_ordinal, input) in inputs.iter().enumerate() {
        event_identities.push((
            input.receipt.event_allocation_identity_for_handoff(),
            input_ordinal,
        ));
        session_identities.push((
            input
                .receipt
                .session_authority_allocation_identity_for_handoff(),
            input_ordinal,
        ));
    }

    // Until coefficient context becomes part of CampaignJobKey, one job may
    // not silently mix otherwise-identical exact sessions from two contexts.
    let mut job_cursor = 0usize;
    while job_cursor < order.len() {
        let first = order[job_cursor];
        let first_context = inputs[first].event().context_fingerprint();
        job_cursor += 1;
        while job_cursor < order.len() && inputs[first].job == inputs[order[job_cursor]].job {
            let next = order[job_cursor];
            if inputs[next].event().context_fingerprint() != first_context {
                return Err(ExactPublicationHandoffError::ContextScopeMismatch {
                    first_input_ordinal: first,
                    second_input_ordinal: next,
                });
            }
            job_cursor += 1;
        }
    }

    // The coordinator's stable `(job, lane)` key and the exact session
    // authority must form a bijection within one frozen wave. Check this
    // before duplicate-event rejection so a mislabeled session is never
    // hidden by two shallow owners of the same event.
    session_identities.sort_unstable();
    for pair in session_identities.windows(2) {
        if pair[0].0 == pair[1].0
            && compare_session_keys(&inputs[pair[0].1], &inputs[pair[1].1]) != Ordering::Equal
        {
            return Err(ExactPublicationHandoffError::SessionAuthorityLaneMismatch);
        }
    }

    let mut cursor = 0usize;
    while cursor < order.len() {
        let first = order[cursor];
        let first_session = inputs[first]
            .receipt
            .session_authority_allocation_identity_for_handoff();
        cursor += 1;
        while cursor < order.len()
            && compare_session_keys(&inputs[first], &inputs[order[cursor]]) == Ordering::Equal
        {
            let next_session = inputs[order[cursor]]
                .receipt
                .session_authority_allocation_identity_for_handoff();
            if first_session != next_session {
                return Err(ExactPublicationHandoffError::SessionLaneCollision);
            }
            cursor += 1;
        }
    }

    event_identities.sort_unstable();
    for pair in event_identities.windows(2) {
        if pair[0].0 == pair[1].0 {
            return Err(ExactPublicationHandoffError::DuplicateEvent {
                first_input_ordinal: pair[0].1,
                second_input_ordinal: pair[1].1,
            });
        }
    }

    let slots =
        try_vec_capacity::<ExactPublicationHandoffSlot>("publication handoff slots", inputs.len())?;
    let mut leaf_states = try_vec_capacity::<AtomicU8>("publication handoff leaf states", leaves)?;
    leaf_states.extend((0..leaves).map(|_| AtomicU8::new(LEAF_PENDING)));

    let retained_shallow_bytes =
        retained_shallow_bytes_for_capacities(slots.capacity(), leaf_states.capacity())?;
    check_limit(
        "publication handoff retained shallow bytes",
        retained_shallow_bytes,
        limits.max_retained_shallow_bytes,
    )?;

    let temporary_bytes = compilation_temporary_bytes_for_capacities(
        order.capacity(),
        event_identities.capacity(),
        session_identities.capacity(),
    )?;
    let compilation_peak_bytes = checked_add(
        "publication handoff compilation peak bytes",
        retained_shallow_bytes,
        temporary_bytes,
    )?;
    check_limit(
        "publication handoff compilation peak bytes",
        compilation_peak_bytes,
        limits.max_compilation_peak_bytes,
    )?;

    let classified = checked_add(
        "publication handoff classified leaves",
        applicable,
        checked_add(
            "publication handoff exceptional leaves",
            exceptional_domain,
            exceptional_leak,
        )?,
    )?;
    if classified != leaves {
        unreachable!("committed publication leaf classification is not total")
    }

    Ok(PreparedHandoff {
        slots,
        leaf_states,
        limits,
        stats: ExactPublicationHandoffStats {
            slots: inputs.len(),
            leaves,
            applicable,
            exceptional_domain,
            exceptional_leak,
            retained_event_payload_bytes,
            max_in_flight_ticket_bytes,
            retained_shallow_bytes,
            compilation_peak_bytes,
        },
    })
}

fn retained_shallow_bytes_for_capacities(
    slot_capacity: usize,
    leaf_state_capacity: usize,
) -> Result<usize, ExactPublicationHandoffError> {
    checked_add(
        "publication handoff retained shallow bytes",
        size_of::<ExactPublicationHandoffWave>(),
        checked_add(
            "publication handoff retained shallow bytes",
            checked_mul(
                "publication handoff retained slot bytes",
                slot_capacity,
                size_of::<ExactPublicationHandoffSlot>(),
            )?,
            checked_mul(
                "publication handoff retained leaf-state bytes",
                leaf_state_capacity,
                size_of::<AtomicU8>(),
            )?,
        )?,
    )
}

fn compilation_temporary_bytes_for_capacities(
    order_capacity: usize,
    event_identity_capacity: usize,
    session_identity_capacity: usize,
) -> Result<usize, ExactPublicationHandoffError> {
    checked_add(
        "publication handoff compilation temporary bytes",
        checked_mul(
            "publication handoff sort-order bytes",
            order_capacity,
            size_of::<usize>(),
        )?,
        checked_add(
            "publication handoff compilation temporary bytes",
            checked_mul(
                "publication handoff event-identity bytes",
                event_identity_capacity,
                size_of::<(usize, usize)>(),
            )?,
            checked_mul(
                "publication handoff session-identity bytes",
                session_identity_capacity,
                size_of::<(usize, usize)>(),
            )?,
        )?,
    )
}

fn compare_inputs(
    left: &ExactPublicationHandoffInput,
    right: &ExactPublicationHandoffInput,
) -> Ordering {
    compare_session_keys(left, right).then_with(|| {
        left.event()
            .event_ordinal()
            .cmp(&right.event().event_ordinal())
    })
}

fn compare_session_keys(
    left: &ExactPublicationHandoffInput,
    right: &ExactPublicationHandoffInput,
) -> Ordering {
    left.job
        .cmp(&right.job)
        .then_with(|| left.session_lane_ordinal.cmp(&right.session_lane_ordinal))
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ExactPublicationHandoffError> {
    left.checked_add(right)
        .ok_or(ExactPublicationHandoffError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ExactPublicationHandoffError> {
    left.checked_mul(right)
        .ok_or(ExactPublicationHandoffError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ExactPublicationHandoffError> {
    if requested > limit {
        Err(ExactPublicationHandoffError::ResourceLimit {
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
) -> Result<Vec<T>, ExactPublicationHandoffError> {
    let mut values = Vec::new();
    values.try_reserve_exact(requested).map_err(|_| {
        ExactPublicationHandoffError::AllocationFailure {
            resource,
            requested,
        }
    })?;
    Ok(values)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier, mpsc};
    use std::thread;

    use super::*;
    use crate::campaign::{CampaignPlan, CampaignPlanLimits, CampaignRootSpec};
    use crate::solver::closure::post_ready::{
        PreparedPublication, PublicationLimits, ready_for_publication,
    };
    use crate::{IntegralFamily, IntegralOrderingPolicy, SectorMask};

    fn job(family: &IntegralFamily, sector: &SectorMask) -> CampaignJobKey {
        let plan = CampaignPlan::compile(
            [
                CampaignRootSpec::try_new("handoff-root", Arc::new(family.clone()), sector.clone())
                    .unwrap(),
            ],
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

    fn semantic_leaf_transcript(
        wave: &ExactPublicationHandoffWave,
        ticket: &mut ExactPublicationHandoffTicket<'_>,
    ) -> String {
        let slot = wave.slot(ticket.slot_ordinal()).unwrap();
        let leaf_ordinal = ticket.leaf_ordinal();
        let leaf = wave.resolve(ticket).unwrap();
        let (class, domain) = match leaf {
            CommittedPublicationLeafView::Applicable(rule) => ("applicable", rule.domain()),
            CommittedPublicationLeafView::Exceptional(residual) => (
                match residual.kind() {
                    ExceptionalResidualKind::Domain => "exceptional-domain",
                    ExceptionalResidualKind::SectorLeak => "exceptional-leak",
                },
                residual.domain(),
            ),
        };
        let event = domain.event();
        let target = event.target_locator();
        let target_offset = event
            .target_offset()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
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
        let terms = event
            .terms()
            .iter()
            .map(|term| {
                let shift = term
                    .shift()
                    .values()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                format!("[{shift}]:{}", term.coefficient().to_expression())
            })
            .collect::<Vec<_>>()
            .join(";");
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            slot.job().family_id().as_str(),
            slot.job().sector(),
            slot.session_lane_ordinal(),
            event.event_ordinal(),
            leaf_ordinal,
            class,
            target.solve_ordinal(),
            target.inventory_position(),
            target.case_ordinal(),
            event.pivot_term_ordinal(),
            target_offset,
            premises,
            predicates,
            terms,
        )
    }

    fn resolved_transcript_at_width(width: usize) -> Vec<String> {
        assert!((1..=4).contains(&width));
        let wave = ExactPublicationHandoffWave::compile(
            vec![
                input("publication-handoff-parallel", 1),
                input("publication-handoff-parallel", 0),
            ],
            ExactPublicationHandoffLimits {
                max_in_flight_tickets: width,
                ..ExactPublicationHandoffLimits::default()
            },
        )
        .unwrap();
        let mut slot_ordinal = 0usize;
        let mut leaf_ordinal = 0usize;
        let mut transcript = Vec::new();
        loop {
            let mut batch = Vec::new();
            while batch.len() < width && slot_ordinal < wave.stats().slots() {
                let slot = wave.slot(slot_ordinal).unwrap();
                if leaf_ordinal == slot.leaf_count() {
                    slot_ordinal += 1;
                    leaf_ordinal = 0;
                    continue;
                }
                batch.push(
                    wave.issue(wave.locator(slot_ordinal, leaf_ordinal).unwrap())
                        .unwrap(),
                );
                leaf_ordinal += 1;
            }
            if batch.is_empty() {
                break;
            }
            assert!(wave.in_flight_tickets() <= width);
            let completed = thread::scope(|scope| {
                let barrier = Arc::new(Barrier::new(batch.len()));
                let completion_turn = Arc::new(AtomicUsize::new(batch.len() - 1));
                let (sender, receiver) = mpsc::channel();
                for (worker_ordinal, mut ticket) in batch.into_iter().enumerate() {
                    let barrier = Arc::clone(&barrier);
                    let completion_turn = Arc::clone(&completion_turn);
                    let sender = sender.clone();
                    let wave = &wave;
                    scope.spawn(move || {
                        barrier.wait();
                        let entry = semantic_leaf_transcript(wave, &mut ticket);
                        barrier.wait();
                        while completion_turn.load(AtomicOrdering::Acquire) != worker_ordinal {
                            thread::yield_now();
                        }
                        sender.send((worker_ordinal, entry, ticket)).unwrap();
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
                    .map(|(worker_ordinal, _, _)| *worker_ordinal)
                    .collect::<Vec<_>>(),
                (0..completed.len()).rev().collect::<Vec<_>>()
            );
            for (_, entry, ticket) in completed.into_iter().rev() {
                transcript.push(entry);
                wave.acknowledge(ticket).unwrap();
            }
            assert_eq!(wave.in_flight_tickets(), 0);
        }
        transcript.sort();
        assert_eq!(wave.state_stats().acknowledged(), wave.stats().leaves());
        transcript
    }

    #[test]
    fn mixed_event_hands_every_leaf_once_without_copying_the_event_payload() {
        let input = input("publication-handoff-mixed", 7);
        let expected_terms = input.event().terms().as_ptr();
        let expected_leaves = input.event().leaf_count();
        let expected_event_bytes = input.receipt.retained_event_bytes();
        let wave = ExactPublicationHandoffWave::compile(
            vec![input],
            ExactPublicationHandoffLimits::default(),
        )
        .unwrap();

        assert_eq!(wave.stats().slots(), 1);
        assert_eq!(wave.stats().leaves(), expected_leaves);
        assert_eq!(
            wave.stats().applicable() + wave.stats().exceptional(),
            expected_leaves
        );
        assert_eq!(wave.state_stats().pending(), expected_leaves);
        assert_eq!(
            wave.stats().retained_event_payload_bytes(),
            expected_event_bytes
        );
        assert_eq!(
            wave.slot(0).unwrap().retained_event_bytes(),
            expected_event_bytes
        );
        assert_eq!(
            wave.stats().max_in_flight_ticket_bytes(),
            expected_leaves.min(wave.limits().max_in_flight_tickets)
                * size_of::<ExactPublicationHandoffTicket<'static>>()
        );
        assert_eq!(
            wave.slot(0).unwrap().event().terms().as_ptr(),
            expected_terms
        );

        for leaf_ordinal in 0..expected_leaves {
            let locator = wave.locator(0, leaf_ordinal).unwrap();
            let mut ticket = wave.issue(locator).unwrap();
            assert_eq!(ticket.slot_ordinal(), 0);
            assert_eq!(ticket.leaf_ordinal(), leaf_ordinal);
            let resolved = wave.resolve(&mut ticket).unwrap();
            let resolved_ordinal = match resolved {
                CommittedPublicationLeafView::Applicable(rule) => rule.leaf_ordinal(),
                CommittedPublicationLeafView::Exceptional(residual) => residual.leaf_ordinal(),
            };
            assert_eq!(resolved_ordinal, leaf_ordinal);
            wave.acknowledge(ticket).unwrap();
            assert_eq!(wave.in_flight_tickets(), 0);
        }
        assert_eq!(wave.state_stats().acknowledged(), expected_leaves);
        assert_eq!(
            wave.slot(0).unwrap().event().terms().as_ptr(),
            expected_terms
        );
    }

    #[test]
    fn duplicate_event_failure_returns_both_move_only_inputs() {
        let first = input("publication-handoff-duplicate", 0);
        let duplicate_receipt = first.receipt.duplicate_for_handoff_test();
        let second = ExactPublicationHandoffInput::new(first.job.clone(), 0, duplicate_receipt);
        let failure = ExactPublicationHandoffWave::compile(
            vec![first, second],
            ExactPublicationHandoffLimits::default(),
        )
        .unwrap_err();
        let (error, returned) = failure.into_parts();
        assert!(matches!(
            error,
            ExactPublicationHandoffError::DuplicateEvent { .. }
        ));
        assert_eq!(returned.len(), 2);
        assert_eq!(
            returned[0].receipt.event_allocation_identity_for_handoff(),
            returned[1].receipt.event_allocation_identity_for_handoff()
        );
    }

    #[test]
    fn distinct_sessions_with_equal_local_event_ordinals_sort_by_stable_lane() {
        let first = input("publication-handoff-distinct-session", 0);
        let second = input("publication-handoff-distinct-session", 1);
        assert_eq!(
            first.event().event_ordinal(),
            second.event().event_ordinal()
        );
        assert_ne!(
            first
                .receipt
                .session_authority_allocation_identity_for_handoff(),
            second
                .receipt
                .session_authority_allocation_identity_for_handoff()
        );
        let wave = ExactPublicationHandoffWave::compile(
            vec![second, first],
            ExactPublicationHandoffLimits::default(),
        )
        .unwrap();
        assert_eq!(wave.slot(0).unwrap().session_lane_ordinal(), 0);
        assert_eq!(wave.slot(1).unwrap().session_lane_ordinal(), 1);
        assert_eq!(
            wave.slot(0).unwrap().event_ordinal(),
            wave.slot(1).unwrap().event_ordinal()
        );
    }

    #[test]
    fn session_lane_bijection_and_job_scope_fail_without_reordering_owners() {
        let first = input("publication-handoff-session-collision", 0);
        let second = input("publication-handoff-session-collision", 0);
        let identities = [
            first.receipt.event_allocation_identity_for_handoff(),
            second.receipt.event_allocation_identity_for_handoff(),
        ];
        let failure = ExactPublicationHandoffWave::compile(
            vec![first, second],
            ExactPublicationHandoffLimits::default(),
        )
        .unwrap_err();
        let (error, returned) = failure.into_parts();
        assert_eq!(error, ExactPublicationHandoffError::SessionLaneCollision);
        assert_eq!(
            returned
                .iter()
                .map(|input| input.receipt.event_allocation_identity_for_handoff())
                .collect::<Vec<_>>(),
            identities
        );

        let first = input("publication-handoff-session-alias", 3);
        let duplicate = ExactPublicationHandoffInput::new(
            first.job.clone(),
            4,
            first.receipt.duplicate_for_handoff_test(),
        );
        let failure = ExactPublicationHandoffWave::compile(
            vec![first, duplicate],
            ExactPublicationHandoffLimits::default(),
        )
        .unwrap_err();
        assert_eq!(
            failure.error(),
            ExactPublicationHandoffError::SessionAuthorityLaneMismatch
        );

        let mut wrong_scope = input("publication-handoff-right-scope", 5);
        let foreign = input("publication-handoff-wrong-scope", 9);
        wrong_scope.job = foreign.job.clone();
        let identity = wrong_scope.receipt.event_allocation_identity_for_handoff();
        let failure = ExactPublicationHandoffWave::compile(
            vec![wrong_scope],
            ExactPublicationHandoffLimits::default(),
        )
        .unwrap_err();
        let (error, returned) = failure.into_parts();
        assert_eq!(
            error,
            ExactPublicationHandoffError::JobScopeMismatch { input_ordinal: 0 }
        );
        assert_eq!(
            returned[0].receipt.event_allocation_identity_for_handoff(),
            identity
        );
    }

    #[test]
    fn the_same_lane_ordinal_is_independently_scoped_by_job() {
        let first = input("publication-handoff-scoped-job-first", 0);
        let second = input("publication-handoff-scoped-job-second", 0);
        assert_ne!(first.job, second.job);
        let wave = ExactPublicationHandoffWave::compile(
            vec![second, first],
            ExactPublicationHandoffLimits::default(),
        )
        .unwrap();
        assert_eq!(wave.stats().slots(), 2);
        assert_eq!(wave.slot(0).unwrap().session_lane_ordinal(), 0);
        assert_eq!(wave.slot(1).unwrap().session_lane_ordinal(), 0);
        assert!(wave.slot(0).unwrap().job() < wave.slot(1).unwrap().job());
    }

    #[test]
    fn exact_and_each_positive_one_below_compile_limit_are_transactional() {
        const NAME: &str = "publication-handoff-limits";
        let pilot = ExactPublicationHandoffWave::compile(
            vec![input(NAME, 0)],
            ExactPublicationHandoffLimits {
                max_in_flight_tickets: 1,
                ..ExactPublicationHandoffLimits::default()
            },
        )
        .unwrap();
        let stats = pilot.stats();
        assert!(stats.slots() > 0);
        assert!(stats.leaves() > 0);
        assert!(stats.retained_event_payload_bytes() > 0);
        assert!(stats.retained_shallow_bytes() > 0);
        assert!(stats.compilation_peak_bytes() > 0);
        let exact = ExactPublicationHandoffLimits {
            max_slots: stats.slots(),
            max_leaves: stats.leaves(),
            max_in_flight_tickets: 1,
            max_retained_event_payload_bytes: stats.retained_event_payload_bytes(),
            max_retained_shallow_bytes: stats.retained_shallow_bytes(),
            max_compilation_peak_bytes: stats.compilation_peak_bytes(),
        };
        assert_eq!(
            ExactPublicationHandoffWave::compile(vec![input(NAME, 0)], exact)
                .unwrap()
                .stats(),
            stats
        );

        let one_below = [
            ExactPublicationHandoffLimits {
                max_slots: exact.max_slots - 1,
                ..exact
            },
            ExactPublicationHandoffLimits {
                max_leaves: exact.max_leaves - 1,
                ..exact
            },
            ExactPublicationHandoffLimits {
                max_retained_event_payload_bytes: exact.max_retained_event_payload_bytes - 1,
                ..exact
            },
            ExactPublicationHandoffLimits {
                max_retained_shallow_bytes: exact.max_retained_shallow_bytes - 1,
                ..exact
            },
            ExactPublicationHandoffLimits {
                max_compilation_peak_bytes: exact.max_compilation_peak_bytes - 1,
                ..exact
            },
        ];
        for limits in one_below {
            let owner = input(NAME, 0);
            let identity = owner.receipt.event_allocation_identity_for_handoff();
            let failure = ExactPublicationHandoffWave::compile(vec![owner], limits).unwrap_err();
            assert!(matches!(
                failure.error(),
                ExactPublicationHandoffError::ResourceLimit { .. }
            ));
            let (_, returned) = failure.into_parts();
            assert_eq!(returned.len(), 1);
            assert_eq!(
                returned[0].receipt.event_allocation_identity_for_handoff(),
                identity
            );
            assert_eq!(
                ExactPublicationHandoffWave::compile(returned, exact)
                    .unwrap()
                    .stats(),
                stats
            );
        }
    }

    #[test]
    fn foreign_locator_and_dropped_ticket_do_not_acknowledge_work() {
        let first = ExactPublicationHandoffWave::compile(
            vec![input("publication-handoff-foreign-first", 0)],
            ExactPublicationHandoffLimits::default(),
        )
        .unwrap();
        let second = ExactPublicationHandoffWave::compile(
            vec![input("publication-handoff-foreign-second", 0)],
            ExactPublicationHandoffLimits::default(),
        )
        .unwrap();
        let foreign = second.locator(0, 0).unwrap();
        assert!(matches!(
            first.issue(foreign),
            Err(ExactPublicationHandoffError::ForeignLocator)
        ));
        let own = first.locator(0, 0).unwrap();
        let mut ticket = first.issue(own).unwrap();
        assert_eq!(first.in_flight_tickets(), 1);
        assert_eq!(
            first.issue(own).unwrap_err(),
            ExactPublicationHandoffError::AlreadyIssued
        );
        assert_eq!(
            second.resolve(&mut ticket).unwrap_err(),
            ExactPublicationHandoffError::ForeignTicket
        );
        assert_eq!(
            second.acknowledge(ticket).unwrap_err(),
            ExactPublicationHandoffError::ForeignTicket
        );
        assert_eq!(first.in_flight_tickets(), 0);
        let ticket = first.issue(own).unwrap();
        drop(ticket);
        assert_eq!(first.state_stats().pending(), first.stats().leaves());
        assert_eq!(first.state_stats().issued(), 0);
        assert_eq!(first.state_stats().acknowledged(), 0);
        assert_eq!(first.in_flight_tickets(), 0);
    }

    #[test]
    fn bounded_issue_acknowledgement_and_worker_panic_are_lossless() {
        let wave = ExactPublicationHandoffWave::compile(
            vec![input("publication-handoff-panic", 0)],
            ExactPublicationHandoffLimits {
                max_in_flight_tickets: 1,
                ..ExactPublicationHandoffLimits::default()
            },
        )
        .unwrap();
        let first = wave.locator(0, 0).unwrap();
        let second = wave.locator(0, 1).unwrap();
        let ticket = wave.issue(first).unwrap();
        assert!(matches!(
            wave.issue(second),
            Err(ExactPublicationHandoffError::InFlightTicketLimit {
                requested: 2,
                limit: 1
            })
        ));
        drop(ticket);
        assert_eq!(wave.in_flight_tickets(), 0);
        assert_eq!(wave.state_stats().pending(), wave.stats().leaves());

        let panic = thread::scope(|scope| {
            scope
                .spawn(|| -> () {
                    let _ticket = wave.issue(first).unwrap();
                    panic!("injected handoff worker panic");
                })
                .join()
        });
        assert!(panic.is_err());
        assert_eq!(wave.in_flight_tickets(), 0);
        assert_eq!(wave.state_stats().pending(), wave.stats().leaves());

        let mut ticket = wave.issue(first).unwrap();
        let before = semantic_leaf_transcript(&wave, &mut ticket);
        wave.acknowledge(ticket).unwrap();
        assert_eq!(wave.in_flight_tickets(), 0);
        assert_eq!(
            wave.issue(first).unwrap_err(),
            ExactPublicationHandoffError::AlreadyAcknowledged
        );
        let after = match wave.slot(0).unwrap().event().leaf(0).unwrap() {
            CommittedPublicationLeafView::Applicable(_) => "applicable",
            CommittedPublicationLeafView::Exceptional(residual) => match residual.kind() {
                ExceptionalResidualKind::Domain => "exceptional-domain",
                ExceptionalResidualKind::SectorLeak => "exceptional-leak",
            },
        };
        assert!(before.contains(after));
        assert_eq!(wave.state_stats().acknowledged(), 1);
    }

    #[test]
    fn ticket_and_wave_traits_support_borrowed_parallel_resolution() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<ExactPublicationHandoffTicket<'static>>();
        assert_sync::<ExactPublicationHandoffWave>();
        assert_eq!(size_of::<AtomicU8>(), size_of::<u8>());
    }

    #[test]
    fn one_two_and_four_worker_resolution_have_identical_semantics() {
        let serial = resolved_transcript_at_width(1);
        assert_eq!(resolved_transcript_at_width(2), serial);
        assert_eq!(resolved_transcript_at_width(4), serial);
    }
}
