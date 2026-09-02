//! Authority-minimal requested-domain support dispatch.
//!
//! The detached sidecar may select additional ordinary-source parents, but it
//! never bypasses the adapter's incidence regeneration, exact replay, or the
//! coordinator's single transactional ledger mutation boundary.

use crate::foundry::completion::source_discovery::cover_delta::{
    CanonicalExactOwnerLedger, ExactOwnerLedgerSnapshotIdentity,
};
use crate::foundry::completion::source_discovery::leader_walk::{
    RequestedDomainPlan, RequestedDomainTask,
};
use crate::foundry::completion::source_discovery::requested_domain_support::{
    RequestedDomainSupportProposal, RequestedDomainSupportUnion,
};

use super::compact::{CompactTaskCommit, RequestedSupportRoute};
use super::run::{
    requested_failed_public, try_commit_coordinated_evaluation, try_materialize_probes,
};
use super::{
    BoundaryProbeCoordinator, ProbeCoordinatorCensus, ProbeCoordinatorConfig,
    ProbeCoordinatorFailure, RequestedProbeCoordinatorStop,
};
use crate::foundry::completion::source_discovery::probe_campaign::ProbeCampaignAdapter;

impl<'inputs, 'sources, 'family> BoundaryProbeCoordinator<'inputs, 'sources, 'family> {
    /// Execute one freshly planned requested-domain phase while consulting an
    /// authority-minimal canonical parent-support sidecar.
    ///
    /// A nonempty sidecar must share at least one stable scope and sector with
    /// the plan. Declared scopes are retained even when their requests are
    /// already fully covered and produce no tasks. Within a compatible plan,
    /// exact semantic-domain hits use assisted source nomination while each
    /// genuine per-domain miss takes the ordinary path. Both routes are
    /// reported in deterministic scalar census fields.
    pub(crate) fn try_run_requested_plan_with_support(
        &mut self,
        plan: &RequestedDomainPlan,
        ledger: &mut CanonicalExactOwnerLedger,
        support: &RequestedDomainSupportUnion,
    ) -> RequestedProbeCoordinatorStop {
        let live_identity = ledger.snapshot_identity();
        if !self.bound_ledger.same_ledger_as(&live_identity) {
            return requested_failed_public(
                self.census,
                crate::foundry::completion::source_discovery::cover_delta::ExactOwnerCoverDeltaError::ForeignLedgerSnapshotIdentity.into(),
            );
        }
        if plan.epoch_ordinal() != ledger.revision().get() {
            return requested_failed_public(
                self.census,
                super::super::ProbeCampaignError::StaleLedgerRevision {
                    planned: plan.epoch_ordinal(),
                    current: ledger.revision().get(),
                }
                .into(),
            );
        }
        if !support.proposals().iter().any(|proposal| {
            let domain = proposal.domain();
            plan.declares_scope(domain.stable_scope_key(), domain.sector())
        }) {
            return requested_failed_public(
                self.census,
                ProbeCoordinatorFailure::UnmatchedRequestedSupportScope {
                    support_domains: support.proposals().len(),
                    declared_scopes: plan.input_scope_count(),
                },
            );
        }
        self.try_run_requested_plan_with_optional_support(plan, ledger, Some(support))
    }
}

/// Find the unique canonical support entry for one requested task without
/// allocating or manufacturing a detached semantic key. The comparison order
/// exactly matches the derived order of the canonical union key.
pub(super) fn requested_support_for_task<'a>(
    support: &'a RequestedDomainSupportUnion,
    task: &RequestedDomainTask,
) -> Option<&'a RequestedDomainSupportProposal> {
    let key = task.key();
    support
        .proposals()
        .binary_search_by(|proposal| {
            let domain = proposal.domain();
            domain
                .stable_scope_key()
                .cmp(key.stable_scope_key())
                .then_with(|| domain.sector().cmp(key.sector()))
                .then_with(|| domain.point().cmp(key.requested_domain_lower()))
                .then_with(|| domain.symbolic_axes().cmp(key.symbolic_axes()))
        })
        .ok()
        .map(|ordinal| &support.proposals()[ordinal])
}

#[allow(clippy::too_many_arguments)]
pub(super) fn try_execute_requested_task(
    config: &ProbeCoordinatorConfig,
    adapter: &ProbeCampaignAdapter<'_, '_, '_>,
    plan: &RequestedDomainPlan,
    task: &RequestedDomainTask,
    support: Option<&RequestedDomainSupportUnion>,
    ledger: &mut CanonicalExactOwnerLedger,
    epoch_identity: &ExactOwnerLedgerSnapshotIdentity,
    baseline_census: ProbeCoordinatorCensus,
    requested_report: usize,
    invalidated_tickets: usize,
    expected_probes_per_task: usize,
) -> Result<CompactTaskCommit, ProbeCoordinatorFailure> {
    ledger.try_require_current_snapshot(epoch_identity)?;
    let before = ledger.snapshot();
    let binding = adapter.try_bind_task(plan, task, ledger)?;
    let probes = try_materialize_probes(config, adapter, task)?;
    let selected_support = support.and_then(|support| requested_support_for_task(support, task));
    let support_route = support.map(|_| {
        if selected_support.is_some() {
            RequestedSupportRoute::Assisted
        } else {
            RequestedSupportRoute::OrdinaryFallback
        }
    });
    let evaluated = match selected_support {
        Some(support) => adapter
            .try_evaluate_requested_task_with_parent_support(binding, ledger, support, probes)?,
        None => adapter.try_evaluate_task(binding, ledger, probes)?,
    };
    try_commit_coordinated_evaluation(
        adapter,
        evaluated,
        ledger,
        before,
        baseline_census,
        requested_report,
        invalidated_tickets,
        expected_probes_per_task,
        support_route,
    )
}
