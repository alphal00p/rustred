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
//! Exceptional-source results can be staged by the compact companion batch in
//! `result_batch`; it transfers admitted outputs into `CampaignResident`
//! ownership without copying algebraic payload. Applicable-provider admission
//! and the RAM-bounded mathematical re-entry coordinator remain separate.

mod result_batch;

use std::fmt;
use std::mem::size_of;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering as AtomicOrdering};

use symbolica::prelude::Integer;

use super::{ExactPublicationHandoffSlot, ExactPublicationHandoffWave, LEAF_ISSUED, LEAF_PENDING};
use crate::campaign::{CampaignJobKey, CampaignWorkKey};
use crate::exact_identity::{
    ExactIdentityError, ExactIdentityLimits, ExactIdentityPayload, ExactIdentityWriter,
    ExactStructuralIdentity, encode_exact_identity,
};
use crate::solver::closure::case_inventory::{
    GeneratedAffineResidualCaseAuthorityError, GeneratedAffineResidualCaseAuthoritySourceKind,
    GeneratedAffineResidualCaseSourceRowLimits, GeneratedAffineResidualCaseSourceRowView,
};
use crate::solver::closure::committed_exceptional_reentry::CommittedExceptionalAuthorityCopyPermit;
use crate::solver::exact_session::GeneratedAffineResidualGroupSolveTargetLocator;
use crate::solver::exact_session::{
    ApplicableRuleHandle, CommittedPublicationDomainView, CommittedPublicationEventHandle,
    CommittedPublicationEventView, CommittedPublicationLeafView, ExceptionalResidualHandle,
    ExceptionalResidualKind,
};
use crate::{
    IntegralFamily, IntegralOrderingPolicy, ParametricCoefficientContext, ParametricRelation,
    SectorMask, SymbolicPolynomialPredicateKind,
};

const SOURCE_PENDING: u8 = 0;
const SOURCE_ISSUED: u8 = 1;
const SOURCE_STAGED: u8 = 2;
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
    staged: usize,
}

impl ExactPublicationEpochSourceStateStats {
    pub(crate) const fn pending(self) -> usize {
        self.pending
    }
    pub(crate) const fn issued(self) -> usize {
        self.issued
    }
    pub(crate) const fn staged(self) -> usize {
        self.staged
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

    pub(crate) fn to_campaign_work_key(self) -> CampaignWorkKey {
        CampaignWorkKey::exact_publication_exceptional_leaf(
            self.job.clone(),
            self.context_fingerprint,
            self.closure_epoch_ordinal,
            self.session_lane_ordinal,
            self.event_ordinal,
            self.leaf_ordinal,
        )
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

    /// Owner-visible bytes of the immutable event allocation and event-local
    /// payload transferred into a narrowed child source.  The separate event
    /// authority/parent ancestry is not included.
    pub(crate) fn retained_event_bytes(self) -> usize {
        self.residual.event().retained_event_bytes()
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

/// Owning, allocation-bound source for one same-sector exceptional-domain
/// child.  Its fields are private to this epoch owner: the only minting path
/// consumes one mint opportunity from an authenticated issued source lease.
/// Lease issuance and the consuming mint are the uniqueness boundary; this
/// source deliberately does not implement `Clone`.
pub(in crate::solver::closure) struct CommittedExceptionalSingletonSource {
    event: CommittedPublicationEventHandle,
    leaf_ordinal: usize,
}

const COMMITTED_EXCEPTIONAL_SINGLETON_STABLE_VALUE_IDENTITY_V1_SCHEMA: &str =
    "rustred-committed-exceptional-singleton-stable-value-identity-v1";

impl CommittedExceptionalSingletonSource {
    /// Private copy used only while one coordinator-owned, memory-admitted
    /// resident transform replaces its input owner with the derived owner.
    /// It shares the immutable event allocation and fixed leaf, but provides
    /// no capability to mint or retry epoch work.
    pub(in crate::solver::closure) fn clone_for_fresh_session_authority(
        &self,
        _permit: CommittedExceptionalAuthorityCopyPermit,
    ) -> Self {
        Self {
            event: self.event.clone(),
            leaf_ordinal: self.leaf_ordinal,
        }
    }

    fn residual(&self) -> ExceptionalResidualHandle<'_> {
        match self
            .event
            .view()
            .leaf(self.leaf_ordinal)
            .expect("sealed exceptional singleton lost its event leaf")
        {
            CommittedPublicationLeafView::Applicable(_) => {
                unreachable!("sealed exceptional singleton changed classification")
            }
            CommittedPublicationLeafView::Exceptional(residual) => {
                debug_assert_eq!(residual.kind(), ExceptionalResidualKind::Domain);
                residual
            }
        }
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedAffineResidualCaseAuthorityError> {
        let event = self.event.view();
        if self.residual().kind() != ExceptionalResidualKind::Domain
            || family.fingerprint_ref() != event.family_fingerprint()
            || context.fingerprint() != event.context_fingerprint()
            || context.index_count() != event.ambient_arity()
        {
            return Err(GeneratedAffineResidualCaseAuthorityError::SourceBinding);
        }
        event.replay_retained_parent_source_authority(family, context)
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        self.event.view().family_fingerprint()
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        self.event.view().context_fingerprint()
    }

    pub(crate) fn sector(&self) -> &SectorMask {
        self.event.view().sector()
    }

    pub(crate) fn ordering(&self) -> IntegralOrderingPolicy {
        self.event.view().ordering()
    }

    pub(crate) fn ambient_arity(&self) -> usize {
        self.event.view().ambient_arity()
    }

    pub(crate) fn constants(&self) -> &[Integer] {
        self.event.view().target_offset()
    }

    pub(crate) fn free_positions(&self) -> &[usize] {
        self.event.view().free_positions()
    }

    pub(crate) fn compact_affine_matrix(&self) -> &[Integer] {
        self.event.view().compact_affine_matrix()
    }

    pub(crate) fn target_premises(&self) -> &[crate::ParametricNonZeroCondition] {
        self.event.view().target_premises()
    }

    pub(crate) fn predicate_count(&self) -> usize {
        self.residual().domain().predicate_count()
    }

    pub(crate) fn predicate(
        &self,
        ordinal: usize,
    ) -> Option<crate::solver::exact_session::CommittedPublicationPredicateView<'_>> {
        self.residual().domain().predicate(ordinal)
    }

    pub(crate) fn source_row_count(&self) -> usize {
        self.event.view().retained_parent_source_row_count()
    }

    pub(crate) fn authenticated_source_row_view(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source_row_ordinal: usize,
        limits: GeneratedAffineResidualCaseSourceRowLimits,
    ) -> Result<
        GeneratedAffineResidualCaseSourceRowView<'_>,
        GeneratedAffineResidualCaseAuthorityError,
    > {
        self.event.view().authenticated_retained_parent_source_row(
            family,
            context,
            source_row_ordinal,
            limits,
        )
    }

    pub(crate) fn same_event_leaf_allocation(&self, other: &Self) -> bool {
        self.event.same_event_allocation(&other.event) && self.leaf_ordinal == other.leaf_ordinal
    }

    pub(in crate::solver::closure) fn event_allocation_identity_for_closure(&self) -> usize {
        self.event.event_allocation_identity_for_handoff()
    }

    pub(crate) fn event_ordinal(&self) -> usize {
        self.event.view().event_ordinal()
    }

    pub(crate) const fn leaf_ordinal(&self) -> usize {
        self.leaf_ordinal
    }

    pub(crate) fn retained_parent_plan_manifest(&self) -> &str {
        self.event.view().retained_parent_plan_manifest()
    }

    /// Owner-visible event allocation and event-local payload bytes behind
    /// this source's Arc.  A campaign must keep these bytes charged after the
    /// originating epoch owner drops, in addition to shared ancestry.
    pub(crate) fn retained_event_bytes(&self) -> usize {
        self.event.view().retained_event_bytes()
    }

    pub(crate) const fn durable_identity_schema(&self) -> &'static str {
        COMMITTED_EXCEPTIONAL_SINGLETON_STABLE_VALUE_IDENTITY_V1_SCHEMA
    }

    pub(crate) fn encode_durable_identity(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source_row_limits: GeneratedAffineResidualCaseSourceRowLimits,
        limits: ExactIdentityLimits,
    ) -> Result<ExactStructuralIdentity, ExactIdentityError> {
        encode_exact_identity(
            &CommittedExceptionalSingletonIdentity {
                source: self,
                family,
                context,
                source_row_limits,
                compact_affine_matrix: self.compact_affine_matrix(),
            },
            limits,
        )
    }
}

struct CommittedExceptionalSingletonIdentity<'source> {
    source: &'source CommittedExceptionalSingletonSource,
    family: &'source IntegralFamily,
    context: &'source ParametricCoefficientContext,
    source_row_limits: GeneratedAffineResidualCaseSourceRowLimits,
    compact_affine_matrix: &'source [Integer],
}

impl ExactIdentityPayload for CommittedExceptionalSingletonIdentity<'_> {
    const SCHEMA: &'static str = COMMITTED_EXCEPTIONAL_SINGLETON_STABLE_VALUE_IDENTITY_V1_SCHEMA;

    fn write_exact_identity(
        &self,
        writer: &mut ExactIdentityWriter<'_>,
    ) -> Result<(), ExactIdentityError> {
        write_committed_exceptional_singleton_identity(
            self.source,
            self.family,
            self.context,
            self.source_row_limits,
            self.compact_affine_matrix,
            CommittedExceptionalParentRowsProjection::Retained,
            writer,
        )
    }
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
enum CommittedExceptionalParentRowsProjection<'rows> {
    Retained,
    LegacyOverride(&'rows [ParametricRelation]),
}

fn write_committed_exceptional_singleton_identity(
    source: &CommittedExceptionalSingletonSource,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    source_row_limits: GeneratedAffineResidualCaseSourceRowLimits,
    compact_affine_matrix: &[Integer],
    parent_rows: CommittedExceptionalParentRowsProjection<'_>,
    writer: &mut ExactIdentityWriter<'_>,
) -> Result<(), ExactIdentityError> {
    let event = source.event.view();
    let locator = event.target_locator();
    writer.begin_record("committed_exceptional_singleton", 13)?;
    writer.string(
        "parent_plan_source_manifest",
        event.retained_parent_plan_manifest(),
    )?;
    let parent_source_kind = match parent_rows {
        CommittedExceptionalParentRowsProjection::Retained => event.retained_parent_source_kind(),
        CommittedExceptionalParentRowsProjection::LegacyOverride(_) => {
            GeneratedAffineResidualCaseAuthoritySourceKind::InitialInventory
        }
    };
    writer.variant("parent_source_kind", parent_source_kind.stable_id())?;
    let legacy_source_rows = match parent_rows {
        CommittedExceptionalParentRowsProjection::Retained
            if parent_source_kind
                == GeneratedAffineResidualCaseAuthoritySourceKind::InitialInventory =>
        {
            event.retained_parent_source_row_count()
        }
        CommittedExceptionalParentRowsProjection::LegacyOverride(rows) => rows.len(),
        _ => 0,
    };
    writer.begin_sequence("parent_legacy_source_rows", legacy_source_rows)?;
    for source_row_ordinal in 0..legacy_source_rows {
        writer.begin_record("source_row", 2)?;
        writer.usize("ordinal", source_row_ordinal)?;
        match parent_rows {
            CommittedExceptionalParentRowsProjection::Retained => {
                let row = source
                    .authenticated_source_row_view(
                        family,
                        context,
                        source_row_ordinal,
                        source_row_limits,
                    )
                    .map_err(|error| match error {
                        GeneratedAffineResidualCaseAuthorityError::ResourceCountOverflow {
                            resource,
                        } => ExactIdentityError::ResourceCountOverflow { resource },
                        GeneratedAffineResidualCaseAuthorityError::ResourceLimit {
                            resource,
                            requested,
                            limit,
                        } => ExactIdentityError::ResourceLimit {
                            resource,
                            requested,
                            limit,
                        },
                        _ => ExactIdentityError::ReferenceBindingMismatch {
                            reference: "committed exceptional inherited source row",
                            ordinal: source_row_ordinal,
                        },
                    })?;
                if row.source_row_ordinal() != source_row_ordinal {
                    return Err(ExactIdentityError::ReferenceBindingMismatch {
                        reference: "committed exceptional inherited source row ordinal",
                        ordinal: source_row_ordinal,
                    });
                }
                writer.parametric_relation("relation", row.relation())?;
            }
            CommittedExceptionalParentRowsProjection::LegacyOverride(rows) => {
                let relation = rows.get(source_row_ordinal).ok_or(
                    ExactIdentityError::ReferenceBindingMismatch {
                        reference: "committed exceptional legacy override source row",
                        ordinal: source_row_ordinal,
                    },
                )?;
                writer.parametric_relation("relation", relation)?;
            }
        }
        writer.end_record()?;
    }
    writer.end_sequence()?;
    writer.string("family", event.family_fingerprint())?;
    writer.string("context", event.context_fingerprint())?;
    writer.begin_sequence("sector", event.sector().arity())?;
    for &active in event.sector().active_bits() {
        writer.boolean("active", active)?;
    }
    writer.end_sequence()?;
    writer.variant("ordering", event.ordering().stable_id())?;
    writer.begin_record("target_locator", 3)?;
    writer.usize("solve_ordinal", locator.solve_ordinal())?;
    writer.usize("inventory_position", locator.inventory_position())?;
    writer.usize("case_ordinal", locator.case_ordinal())?;
    writer.end_record()?;
    writer.begin_sequence("target_offset", event.target_offset().len())?;
    for value in event.target_offset() {
        writer.integer("value", value)?;
    }
    writer.end_sequence()?;
    writer.begin_record("geometry", 3)?;
    writer.usize("ambient_arity", event.ambient_arity())?;
    writer.begin_sequence("free_positions", event.free_positions().len())?;
    for &position in event.free_positions() {
        writer.usize("position", position)?;
    }
    writer.end_sequence()?;
    writer.begin_sequence("compact_affine_matrix", compact_affine_matrix.len())?;
    for value in compact_affine_matrix {
        writer.integer("value", value)?;
    }
    writer.end_sequence()?;
    writer.end_record()?;
    writer.begin_sequence("target_premises", event.target_premises().len())?;
    for condition in event.target_premises() {
        writer.begin_record("premise", 2)?;
        writer.polynomial("polynomial", condition.polynomial().raw())?;
        writer.begin_sequence("origins", condition.origins().len())?;
        for origin in condition.origins() {
            writer.guard_origin("origin", origin)?;
        }
        writer.end_sequence()?;
        writer.end_record()?;
    }
    writer.end_sequence()?;
    writer.variant("residual_kind", "Domain")?;
    writer.begin_sequence("predicates", source.predicate_count())?;
    for ordinal in 0..source.predicate_count() {
        let predicate =
            source
                .predicate(ordinal)
                .ok_or(ExactIdentityError::ReferenceBindingMismatch {
                    reference: "committed exceptional predicate",
                    ordinal,
                })?;
        // The sequence position binds predicate order. The event-local
        // locus ordinal is allocation provenance, not stable value.
        writer.begin_record("predicate", 2)?;
        writer.variant(
            "kind",
            match predicate.kind() {
                SymbolicPolynomialPredicateKind::EqualZero => "EqualZero",
                SymbolicPolynomialPredicateKind::NonZero => "NonZero",
            },
        )?;
        writer.polynomial("polynomial", predicate.polynomial().raw())?;
        writer.end_record()?;
    }
    writer.end_sequence()?;
    writer.end_record()
}

impl fmt::Debug for CommittedExceptionalSingletonSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommittedExceptionalSingletonSource")
            .field("event_ordinal", &self.event.view().event_ordinal())
            .field("leaf_ordinal", &self.leaf_ordinal)
            .field("kind", &ExceptionalResidualKind::Domain)
            .field("private_event", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExactPublicationEpochReentryError {
    Epoch(ExactPublicationEpochError),
    SectorLeakRequiresOutOfSectorRouting,
}

/// Transactional mint failure.  The exact issued lease is returned, so a
/// caller may hand a sector leak to the separate out-of-sector router or drop
/// it to restore `Pending`.
pub(crate) struct ExactPublicationEpochReentryMintFailure<'owner> {
    error: ExactPublicationEpochReentryError,
    lease: ExactPublicationEpochSourceLease<'owner>,
}

impl<'owner> ExactPublicationEpochReentryMintFailure<'owner> {
    pub(crate) const fn error(&self) -> ExactPublicationEpochReentryError {
        self.error
    }

    pub(crate) fn into_lease(self) -> ExactPublicationEpochSourceLease<'owner> {
        self.lease
    }
}

impl fmt::Debug for ExactPublicationEpochReentryMintFailure<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactPublicationEpochReentryMintFailure")
            .field("error", &self.error)
            .field("source_ordinal", &self.lease.source_ordinal())
            .finish_non_exhaustive()
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
    active: bool,
}

impl ExactPublicationEpochSourceLease<'_> {
    pub(crate) const fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }
}

impl Drop for ExactPublicationEpochSourceLease<'_> {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        if self.owner.exceptional_source_states[self.source_ordinal]
            .compare_exchange(
                SOURCE_ISSUED,
                SOURCE_PENDING,
                AtomicOrdering::AcqRel,
                AtomicOrdering::Acquire,
            )
            .is_ok()
        {
            self.active = false;
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
    AlreadyStaged,
    SourceIssuanceInvariantMismatch {
        issued: usize,
        in_flight: usize,
    },
    SourceStateInvariantMismatch {
        observed: u8,
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
            Self::AlreadyStaged => {
                formatter.write_str("exceptional source already has a staged result")
            }
            Self::SourceIssuanceInvariantMismatch { issued, in_flight } => write!(
                formatter,
                "exceptional-source issued-state count {issued} differs from in-flight count {in_flight}"
            ),
            Self::SourceStateInvariantMismatch { observed } => write!(
                formatter,
                "exceptional source had invalid state byte {observed}"
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
/// Exceptional sources move from `Pending` to `Issued`; the companion result
/// batch can terminalize an admitted charged output as `Staged`. Mathematical
/// re-entry, merging, and durable publication remain separate responsibilities.
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
                SOURCE_STAGED => stats.staged += 1,
                _ => unreachable!("sealed publication epoch has an invalid source state"),
            }
        }
        stats
    }

    /// Barrier-only recovery for leases deliberately forgotten by safe code.
    ///
    /// Exclusive access proves that no usable lease borrow remains. Only
    /// `Issued` is recovered. `Staged` is terminal for this epoch and is
    /// never made retryable by lease recovery.
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
            Err(SOURCE_STAGED) => {
                self.release_in_flight_source();
                return Err(ExactPublicationEpochError::AlreadyStaged);
            }
            Ok(observed) | Err(observed) => {
                self.release_in_flight_source();
                return Err(ExactPublicationEpochError::SourceStateInvariantMismatch { observed });
            }
        }
        Ok(ExactPublicationEpochSourceLease {
            owner: self,
            source_ordinal: locator.source_ordinal,
            active: true,
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
            SOURCE_STAGED => return Err(ExactPublicationEpochError::AlreadyStaged),
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

    /// Consume one live same-epoch lease into the result-batch worker owner for
    /// a fresh same-sector exceptional-domain lane.  There is no borrowing mint
    /// API, so one issued lease cannot manufacture two owning sources.
    pub(crate) fn mint_domain_reentry_worker_result<'owner>(
        &'owner self,
        lease: ExactPublicationEpochSourceLease<'owner>,
    ) -> Result<
        result_batch::ExactPublicationEpochWorkerResult<
            'owner,
            CommittedExceptionalSingletonSource,
        >,
        ExactPublicationEpochReentryMintFailure<'owner>,
    > {
        let source_ordinal = lease.source_ordinal;
        if let Err(error) = self.preflight_stage_lease(&lease, source_ordinal) {
            return Err(ExactPublicationEpochReentryMintFailure {
                error: ExactPublicationEpochReentryError::Epoch(error),
                lease,
            });
        }
        let flat_leaf_index = match self.exceptional_flat_leaf_indexes.get(source_ordinal) {
            Some(value) => *value,
            None => {
                return Err(ExactPublicationEpochReentryMintFailure {
                    error: ExactPublicationEpochReentryError::Epoch(
                        ExactPublicationEpochError::UnknownSource,
                    ),
                    lease,
                });
            }
        };
        let (slot, leaf_ordinal) = match self.resolve_flat_leaf(flat_leaf_index) {
            Some(value) => value,
            None => {
                return Err(ExactPublicationEpochReentryMintFailure {
                    error: ExactPublicationEpochReentryError::Epoch(
                        ExactPublicationEpochError::UnknownSource,
                    ),
                    lease,
                });
            }
        };
        let residual = match slot.event.view().leaf(leaf_ordinal) {
            Some(CommittedPublicationLeafView::Exceptional(residual)) => residual,
            _ => {
                return Err(ExactPublicationEpochReentryMintFailure {
                    error: ExactPublicationEpochReentryError::Epoch(
                        ExactPublicationEpochError::UnknownSource,
                    ),
                    lease,
                });
            }
        };
        if residual.kind() == ExceptionalResidualKind::SectorLeak {
            return Err(ExactPublicationEpochReentryMintFailure {
                error: ExactPublicationEpochReentryError::SectorLeakRequiresOutOfSectorRouting,
                lease,
            });
        }
        let source = CommittedExceptionalSingletonSource {
            event: slot.event.clone(),
            leaf_ordinal,
        };
        Ok(result_batch::ExactPublicationEpochWorkerResult::new(
            source, lease,
        ))
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

    pub(super) fn exceptional_source_work_key(
        &self,
        source_ordinal: usize,
    ) -> Result<CampaignWorkKey, ExactPublicationEpochError> {
        Ok(self
            .exceptional_source_scheduling_key(source_ordinal)?
            .to_campaign_work_key())
    }

    pub(super) fn exceptional_source_scheduling_key(
        &self,
        source_ordinal: usize,
    ) -> Result<ExactPublicationEpochSchedulingKey<'_>, ExactPublicationEpochError> {
        let flat_leaf_index = *self
            .exceptional_flat_leaf_indexes
            .get(source_ordinal)
            .ok_or(ExactPublicationEpochError::UnknownSource)?;
        let (slot, leaf_ordinal) = self
            .resolve_flat_leaf(flat_leaf_index)
            .ok_or(ExactPublicationEpochError::UnknownSource)?;
        Ok(scheduling_key(
            self.closure_epoch_ordinal,
            slot,
            leaf_ordinal,
        ))
    }

    pub(super) fn exceptional_source_is_staged(&self, source_ordinal: usize) -> bool {
        self.exceptional_source_states
            .get(source_ordinal)
            .is_some_and(|state| state.load(AtomicOrdering::Acquire) == SOURCE_STAGED)
    }

    pub(super) fn preflight_stage_lease(
        &self,
        lease: &ExactPublicationEpochSourceLease<'_>,
        source_ordinal: usize,
    ) -> Result<(), ExactPublicationEpochError> {
        if !std::ptr::eq(self, lease.owner) {
            return Err(ExactPublicationEpochError::ForeignLease);
        }
        if lease.source_ordinal != source_ordinal {
            return Err(ExactPublicationEpochError::UnknownSource);
        }
        if !lease.active {
            return Err(ExactPublicationEpochError::NotIssued);
        }
        let state = self
            .exceptional_source_states
            .get(source_ordinal)
            .ok_or(ExactPublicationEpochError::UnknownSource)?;
        match state.load(AtomicOrdering::Acquire) {
            SOURCE_ISSUED => Ok(()),
            SOURCE_PENDING => Err(ExactPublicationEpochError::NotIssued),
            SOURCE_STAGED => Err(ExactPublicationEpochError::AlreadyStaged),
            observed => Err(ExactPublicationEpochError::SourceStateInvariantMismatch { observed }),
        }
    }

    /// Terminalize one already installed, charged result. This tail performs
    /// no allocation, user callback, payload destruction, panicking indexing,
    /// or panicking invariant assertion. Any impossible mismatch is left
    /// fail-closed in `Staged` while the batch retains the resident result.
    pub(super) fn terminalize_staged_result(
        &self,
        lease: &mut ExactPublicationEpochSourceLease<'_>,
    ) -> Result<(), ExactPublicationEpochError> {
        let Some(state) = self.exceptional_source_states.get(lease.source_ordinal) else {
            lease.active = false;
            return Err(ExactPublicationEpochError::UnknownSource);
        };
        let transition = state.compare_exchange(
            SOURCE_ISSUED,
            SOURCE_STAGED,
            AtomicOrdering::AcqRel,
            AtomicOrdering::Acquire,
        );
        lease.active = false;
        match transition {
            Ok(_) => {
                if self
                    .in_flight_sources
                    .fetch_update(AtomicOrdering::AcqRel, AtomicOrdering::Acquire, |current| {
                        current.checked_sub(1)
                    })
                    .is_ok()
                {
                    Ok(())
                } else {
                    Err(
                        ExactPublicationEpochError::SourceIssuanceInvariantMismatch {
                            issued: 1,
                            in_flight: 0,
                        },
                    )
                }
            }
            Err(SOURCE_STAGED) => Err(ExactPublicationEpochError::AlreadyStaged),
            Err(SOURCE_PENDING) => {
                state.store(SOURCE_STAGED, AtomicOrdering::Release);
                Err(ExactPublicationEpochError::NotIssued)
            }
            Err(observed) => {
                state.store(SOURCE_STAGED, AtomicOrdering::Release);
                Err(ExactPublicationEpochError::SourceStateInvariantMismatch { observed })
            }
        }
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
    use std::collections::BTreeMap;
    use std::mem::size_of;
    use std::sync::Arc;

    use super::super::{
        ExactPublicationHandoffInput, ExactPublicationHandoffLimits, ExactPublicationHandoffWave,
    };
    use super::*;
    use crate::campaign::{
        CampaignAdmissionController, CampaignBytes, CampaignEstimatorRevision,
        CampaignMemoryEstimate, CampaignPlan, CampaignPlanLimits, CampaignResident,
        CampaignResidentToken, CampaignRootSpec, CampaignTaskMemoryEnvelope,
        CampaignTaskResourceEstimate, CampaignWavePlanner,
    };
    use crate::generated_affine_parametric_ordering::{
        GeneratedAffineParametricOrderingCertificate, GeneratedAffineParametricOrderingLimits,
    };
    use crate::generated_affine_prepare_point_schedule::{
        GeneratedAffinePreparePointScheduleCertificate, GeneratedAffinePreparePointScheduleLimits,
    };
    use crate::generated_affine_residual_case_premises::{
        GeneratedAffineResidualCasePremisesLimits, GeneratedAffineResidualCasePremisesOutcome,
        compile_generated_affine_residual_case_premises,
    };
    use crate::generated_affine_residual_case_reelimination::{
        GeneratedAffineResidualCaseReeliminationCompilation,
        GeneratedAffineResidualCaseReeliminationCompiler,
        GeneratedAffineResidualCaseReeliminationLimits,
    };
    use crate::parametric_sector_formula_affine_terminal::{
        ParametricSectorFormulaAffineTerminalCompiler, ParametricSectorFormulaAffineTerminalLimits,
    };
    use crate::parametric_sector_formula_residual::{
        ParametricSectorFormulaResidualCursor, ParametricSectorFormulaResidualLimits,
        ParametricSectorFormulaResidualRequest,
    };
    use crate::parametric_sector_normalized_source::{
        ParametricSectorNormalizedCoverageSourceCompiler,
        ParametricSectorNormalizedCoverageSourceLimits,
    };
    use crate::solver::closure::case_inventory::{
        GeneratedAffineResidualCaseAuthority, GeneratedAffineResidualCaseAuthorityLimits,
        GeneratedAffineResidualCaseAuthoritySourceKind,
    };
    use crate::solver::closure::committed_exceptional_reentry::{
        CommittedExceptionalFreshSessionLimits,
        try_build_fresh_exact_session_for_admitted_transform,
    };
    use crate::solver::closure::post_ready::{
        GeneratedAffineResidualGroupExactConditionPlanCompiler,
        GeneratedAffineResidualGroupExactConditionPlanLimits,
        GeneratedAffineResidualGroupExactWhenBadMaterializationCompiler,
        GeneratedAffineResidualGroupExactWhenBadMaterializationLimits,
        GeneratedAffineResidualGroupExactWhenBadPartitionCompilation,
        GeneratedAffineResidualGroupExactWhenBadPartitionCompiler,
        GeneratedAffineResidualGroupExactWhenBadPartitionLimits,
        GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler,
        GeneratedAffineResidualGroupReadyPublicationAnalysisLimits,
        GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome, PreparedPublication,
        PublicationLimits,
    };
    use crate::solver::exact_session::{
        GeneratedAffineResidualGroupExactPhysicalRow,
        GeneratedAffineResidualGroupExactPhysicalRowCompiler,
        GeneratedAffineResidualGroupExactPhysicalRowLimits,
        GeneratedAffineResidualGroupExactSession, GeneratedAffineResidualGroupExactSessionLimits,
        GeneratedAffineResidualGroupExactSessionRecenterOutcome,
        GeneratedAffineResidualGroupPhysicalFrame, GeneratedAffineResidualGroupPhysicalKeyLimits,
        GeneratedAffineResidualGroupSolvePlan, GeneratedAffineResidualGroupSolvePlanLimits,
    };
    use crate::{
        AffineDenominator, IntegralFamily, IntegralOrderingPolicy, ParallelExecution,
        ParametricCoefficientContext, ParametricIbpGenerator, SectorMask,
        SymbolicPolynomialPredicateKind, algebra::CoefficientContext,
    };

    struct DirectPublicationFixture {
        family: IntegralFamily,
        context: ParametricCoefficientContext,
        plan: Arc<GeneratedAffineResidualGroupSolvePlan>,
        rows: Vec<Arc<GeneratedAffineResidualGroupExactPhysicalRow>>,
    }

    fn test_family(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let zero = coefficients.zero();
        let one = coefficients.one();
        let minus_m2 = coefficients.parse("-m2").unwrap();
        IntegralFamily::new(
            name,
            vec!["k1".into(), "k2".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![one.clone(), zero.clone(), zero.clone()],
                ),
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![zero.clone(), zero.clone(), one.clone()],
                ),
                AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
            ],
            Vec::new(),
            vec![zero.clone(), zero.clone(), zero],
        )
        .unwrap()
    }

    fn direct_publication_fixture(name: &str) -> DirectPublicationFixture {
        let family = test_family(name);
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let normalized = Arc::new(
            ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
                &family,
                &context,
                SectorMask::try_from_bit_string("011").unwrap(),
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                Vec::new(),
                ParametricSectorNormalizedCoverageSourceLimits::default(),
            )
            .unwrap(),
        );
        let mut cursor = ParametricSectorFormulaResidualCursor::try_new(
            &family,
            &context,
            normalized,
            ParametricSectorFormulaResidualRequest::Uncovered,
            ParametricSectorFormulaResidualLimits::default(),
        )
        .unwrap();
        let path = Arc::new(cursor.next_path().unwrap().unwrap());
        assert!(cursor.next_path().unwrap().is_none());
        let terminal = Arc::new(
            ParametricSectorFormulaAffineTerminalCompiler::compile(
                &family,
                &context,
                path,
                ParametricSectorFormulaAffineTerminalLimits::default(),
            )
            .unwrap(),
        );
        let authority = Arc::new(
            GeneratedAffineResidualCaseAuthority::try_new_direct_formula_singleton(
                &family,
                &context,
                terminal,
                GeneratedAffineResidualCaseAuthorityLimits::default(),
            )
            .unwrap(),
        );
        let premises = match compile_generated_affine_residual_case_premises(
            &family,
            &context,
            Arc::clone(&authority),
            GeneratedAffineResidualCasePremisesLimits::default(),
        )
        .unwrap()
        {
            GeneratedAffineResidualCasePremisesOutcome::Ready(value) => Arc::new(value),
            GeneratedAffineResidualCasePremisesOutcome::RequiresAffineEqualityRefinement(_) => {
                panic!("Direct publication fixture unexpectedly requires equality refinement")
            }
        };
        let ordering = Arc::new(
            GeneratedAffineParametricOrderingCertificate::try_new(
                &family,
                &context,
                Arc::clone(&authority),
                GeneratedAffineParametricOrderingLimits::default(),
            )
            .unwrap(),
        );
        let schedule = Arc::new(
            GeneratedAffinePreparePointScheduleCertificate::compile(
                &family,
                &context,
                Arc::clone(&ordering),
                &authority,
                0,
                GeneratedAffinePreparePointScheduleLimits::default(),
            )
            .unwrap(),
        );
        let compilation = GeneratedAffineResidualCaseReeliminationCompiler::compile(
            &family,
            &context,
            Arc::clone(&authority),
            premises,
            ordering,
            schedule,
            GeneratedAffineResidualCaseReeliminationLimits::default(),
        )
        .unwrap();
        let GeneratedAffineResidualCaseReeliminationCompilation::Eliminated(certificate) =
            compilation
        else {
            panic!("Direct publication fixture produced no eliminable rows")
        };
        let certificate = Arc::new(certificate);
        let frame = Arc::new(
            GeneratedAffineResidualGroupPhysicalFrame::try_new(
                &family,
                &context,
                Arc::clone(&authority),
                GeneratedAffineResidualGroupPhysicalKeyLimits::default(),
            )
            .unwrap(),
        );
        let plan = Arc::new(
            GeneratedAffineResidualGroupSolvePlan::try_new_direct_formula_singleton(
                &family,
                &context,
                authority,
                Arc::clone(&frame),
                GeneratedAffineResidualGroupSolvePlanLimits::default(),
            )
            .unwrap(),
        );
        let mut retained_row_ordinal = 0usize;
        let mut rows = Vec::new();
        for (witness_ordinal, witness) in certificate.witnesses().iter().enumerate() {
            if !witness.outcome().is_retained() {
                continue;
            }
            rows.push(Arc::new(
                GeneratedAffineResidualGroupExactPhysicalRowCompiler::compile_from_reelimination_for_test(
                    &family,
                    &context,
                    Arc::clone(&certificate),
                    retained_row_ordinal,
                    witness_ordinal,
                    Arc::clone(&frame),
                    GeneratedAffineResidualGroupExactPhysicalRowLimits::default(),
                )
                .unwrap(),
            ));
            retained_row_ordinal += 1;
        }
        assert_eq!(rows.len(), certificate.retained_row_count());
        assert!(!rows.is_empty());
        DirectPublicationFixture {
            family,
            context,
            plan,
            rows,
        }
    }

    fn committed_input(
        name: &str,
        lane: usize,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        ExactPublicationHandoffInput,
    ) {
        let fixture = direct_publication_fixture(name);
        let mut session = GeneratedAffineResidualGroupExactSession::try_new(
            &fixture.family,
            &fixture.context,
            Arc::clone(&fixture.plan),
            211,
            GeneratedAffineResidualGroupExactSessionLimits::default(),
        )
        .unwrap();
        for row in &fixture.rows {
            let transaction = session
                .stage_replayed_row(&fixture.family, &fixture.context, row)
                .unwrap();
            let transaction = match session.classify_dependent(transaction) {
                Ok(classified) => {
                    session
                        .commit_dependent(&fixture.family, &fixture.context, classified)
                        .unwrap();
                    continue;
                }
                Err(failure) => failure.into_transaction(),
            };
            match session
                .recenter_staged_new_pivot(&fixture.family, &fixture.context, transaction)
                .unwrap()
            {
                GeneratedAffineResidualGroupExactSessionRecenterOutcome::NoTarget(no_target) => {
                    session = session
                        .commit_no_target(&fixture.family, &fixture.context, no_target)
                        .unwrap()
                        .into_session();
                }
                GeneratedAffineResidualGroupExactSessionRecenterOutcome::RequiresAffineEqualityRefinement(
                    _,
                ) => panic!("publication fixture unexpectedly requires equality refinement"),
                GeneratedAffineResidualGroupExactSessionRecenterOutcome::Ready(ready) => {
                    let analyzed =
                        GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler::analyze(
                            &fixture.family,
                            &fixture.context,
                            &session,
                            ready,
                            GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default(),
                        )
                        .unwrap();
                    let GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome::ReadyForConditions(
                        ready,
                    ) = analyzed
                    else {
                        panic!("publication fixture Ready row failed exact descent")
                    };
                    let plan = GeneratedAffineResidualGroupExactConditionPlanCompiler::compile(
                        &fixture.family,
                        &fixture.context,
                        &session,
                        ready,
                        GeneratedAffineResidualGroupExactConditionPlanLimits::default(),
                    )
                    .unwrap();
                    let materialized =
                        GeneratedAffineResidualGroupExactWhenBadMaterializationCompiler::compile(
                            &fixture.family,
                            &fixture.context,
                            &session,
                            plan,
                            GeneratedAffineResidualGroupExactWhenBadMaterializationLimits::default(),
                        )
                        .unwrap();
                    let partitioned =
                        GeneratedAffineResidualGroupExactWhenBadPartitionCompiler::compile(
                            &fixture.family,
                            &fixture.context,
                            &session,
                            materialized,
                            GeneratedAffineResidualGroupExactWhenBadPartitionLimits::default(),
                        )
                        .unwrap();
                    let GeneratedAffineResidualGroupExactWhenBadPartitionCompilation::ReadyForPublication(
                        ready,
                    ) = partitioned
                    else {
                        panic!("publication fixture unexpectedly became identically bad")
                    };
                    let prepared =
                        PreparedPublication::prepare(ready, PublicationLimits::default()).unwrap();
                    let receipt = session.commit_publication(prepared).unwrap();
                    let job = job(&fixture.family, receipt.event().sector());
                    let input = ExactPublicationHandoffInput::new(job, lane, receipt);
                    return (fixture.family, fixture.context, input);
                }
            }
        }
        panic!("publication fixture exhausted generated rows before exact Ready")
    }

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
        committed_input(name, lane).2
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

    pub(super) fn fully_acknowledged_handoff(
        name: &str,
        lanes_in_input_order: &[usize],
    ) -> ExactPublicationHandoffWave {
        let wave = handoff(name, lanes_in_input_order);
        fully_acknowledge(&wave);
        wave
    }

    fn owner_with_scope(
        name: &str,
        closure_epoch_ordinal: u64,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        ExactPublicationEpochOwner,
    ) {
        let (family, context, input) = committed_input(name, 0);
        let wave = ExactPublicationHandoffWave::compile(
            vec![input],
            ExactPublicationHandoffLimits::default(),
        )
        .unwrap();
        fully_acknowledge(&wave);
        let owner = ExactPublicationEpochOwner::compile(
            wave,
            closure_epoch_ordinal,
            ExactPublicationEpochLimits::default(),
        )
        .unwrap();
        (family, context, owner)
    }

    fn exceptional_source_ordinals(owner: &ExactPublicationEpochOwner) -> (usize, usize) {
        let mut domain_with_equality = None;
        let mut leak = None;
        for source_ordinal in 0..owner.stats().exceptional() {
            let locator = owner.exceptional_source_locator(source_ordinal).unwrap();
            let mut lease = owner.issue_exceptional_source(locator).unwrap();
            let (kind, has_equality) = {
                let view = owner.resolve_exceptional_source(&mut lease).unwrap();
                (
                    view.kind(),
                    view.domain().predicates().any(|predicate| {
                        predicate.kind() == SymbolicPolynomialPredicateKind::EqualZero
                    }),
                )
            };
            drop(lease);
            match kind {
                ExceptionalResidualKind::Domain if has_equality => {
                    domain_with_equality.get_or_insert(source_ordinal);
                }
                ExceptionalResidualKind::Domain => {}
                ExceptionalResidualKind::SectorLeak => {
                    leak.get_or_insert(source_ordinal);
                }
            }
        }
        (
            domain_with_equality.expect("fixture must expose an EqualZero domain leaf"),
            leak.expect("fixture must expose an out-of-sector leak leaf"),
        )
    }

    fn admission_controller() -> CampaignAdmissionController {
        CampaignAdmissionController::try_new(
            ParallelExecution::try_new(2).unwrap(),
            CampaignEstimatorRevision::try_new(1).unwrap(),
            CampaignBytes::new(128 * 1024 * 1024),
            CampaignBytes::ZERO,
            CampaignBytes::ZERO,
        )
        .unwrap()
    }

    fn task_estimate(
        retained_visible: u64,
        retained_opaque: u64,
        transient_visible: u64,
        transient_opaque: u64,
    ) -> CampaignTaskResourceEstimate {
        CampaignTaskResourceEstimate::try_new(
            CampaignEstimatorRevision::try_new(1).unwrap(),
            1,
            CampaignTaskMemoryEnvelope::try_new(
                CampaignMemoryEstimate::try_new(
                    CampaignBytes::new(retained_visible),
                    CampaignBytes::new(retained_opaque),
                )
                .unwrap(),
                CampaignMemoryEstimate::try_new(
                    CampaignBytes::new(transient_visible),
                    CampaignBytes::new(transient_opaque),
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn reserve_one(
        controller: &mut CampaignAdmissionController,
        work: &crate::campaign::CampaignWorkKey,
        estimate: CampaignTaskResourceEstimate,
        predecessor: Option<CampaignResidentToken>,
    ) -> crate::campaign::CampaignTaskReservation {
        let requests = BTreeMap::from([(work.clone(), estimate)]);
        let snapshot = controller.try_snapshot().unwrap();
        let plan = CampaignWavePlanner::try_plan(snapshot.policy(), &requests).unwrap();
        let predecessors = predecessor
            .map(|token| BTreeMap::from([(work.clone(), token)]))
            .unwrap_or_default();
        controller
            .try_reserve_wave_with_predecessors(&snapshot, &plan, &requests, &predecessors)
            .unwrap()
            .into_tasks()
            .pop()
            .unwrap()
    }

    fn staged_domain_source_from_real_mint(
        owner: &ExactPublicationEpochOwner,
    ) -> CampaignResident<CommittedExceptionalSingletonSource> {
        let (source_ordinal, _) = exceptional_source_ordinals(owner);
        let mut admission = admission_controller();
        let mut batch = result_batch::ExactPublicationEpochResultBatch::<
            CommittedExceptionalSingletonSource,
        >::try_new(
            owner,
            &mut admission,
            vec![source_ordinal],
            result_batch::ExactPublicationEpochResultBatchLimits::default(),
        )
        .unwrap();
        let work = batch.schedule().work(0).unwrap().clone();
        let source_visible_bytes = {
            let locator = owner.exceptional_source_locator(source_ordinal).unwrap();
            let mut census_lease = owner.issue_exceptional_source(locator).unwrap();
            let event_bytes = owner
                .resolve_exceptional_source(&mut census_lease)
                .unwrap()
                .retained_event_bytes();
            drop(census_lease);
            event_bytes
                .checked_add(size_of::<CommittedExceptionalSingletonSource>())
                .unwrap()
        };
        let reservation = reserve_one(
            &mut admission,
            &work,
            task_estimate(
                u64::try_from(source_visible_bytes).unwrap(),
                256 * 1024,
                128 * 1024,
                256 * 1024,
            ),
            None,
        );
        let lease = batch.schedule().issue(0).unwrap();
        let worker = owner.mint_domain_reentry_worker_result(lease).unwrap();
        batch.try_stage(reservation.bind(worker)).unwrap();
        let mut staged = batch.into_staged_results().unwrap();
        let source = staged
            .take_resident(0)
            .expect("real mint path must stage one domain source");
        drop(staged);
        source
    }

    #[test]
    fn sentinel_committed_exceptional_child_replays_from_real_epoch_mint() {
        const DATABASE_EPOCH: usize = 2_029;
        let (family, context, owner) =
            owner_with_scope("publication-epoch-fresh-child-sentinel", 101);
        let source_resident = staged_domain_source_from_real_mint(&owner);
        let (source, source_charge) = source_resident.split_owner_charge();
        let expected_source_rows = source.source_row_count();

        let build = match try_build_fresh_exact_session_for_admitted_transform(
            source,
            &family,
            &context,
            DATABASE_EPOCH,
            CommittedExceptionalFreshSessionLimits::default(),
        ) {
            Ok(build) => build,
            Err(_) => panic!("real committed exceptional source must build a fresh session"),
        };
        assert_eq!(build.inherited_source_rows(), expected_source_rows);
        let session = build.into_session();
        assert_eq!(
            session.source_kind(),
            GeneratedAffineResidualCaseAuthoritySourceKind::CommittedExceptionalSingleton,
        );
        assert_eq!(session.database_epoch(), DATABASE_EPOCH);
        assert_eq!(session.target_catalog_stats().targets(), 1);
        assert_eq!(
            session.target_catalog_stats().equality_refinement_targets(),
            1
        );
        session.replay(&family, &context).unwrap();
        drop(source_charge);
    }
}
