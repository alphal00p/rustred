use crate::foundry::completion::source_discovery::{
    CampaignModularProbe, CanonicalExactOwnerLedger, ExactExecutableOwnerProposal,
    InteriorReplayRunDisposition, OrdinarySourceIncidenceIndex, try_run_interior_replay_task,
};
use crate::foundry::completion::stratum::{DecoratedStratum, MaximalStratumAnchor};
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::SectorMonotoneDomain;

use super::super::interior_simplex::{InteriorSimplexPlan, InteriorSimplexTask};
use super::{
    InteriorCampaignBootstrapCensus, InteriorCampaignCensus, InteriorCampaignError,
    InteriorCampaignLimits, InteriorCampaignTaskBinding, InteriorCampaignTaskReport,
};

const PHYSICAL_SHIFTS: &str = "bootstrap physical shift occurrences";
const PHYSICAL_SHIFT_CELLS: &str = "bootstrap physical shift coordinate cells";
const PHYSICAL_SHIFT_SORT_WORK: &str = "bootstrap physical shift logical sort work reservation";
const DISTINCT_PHYSICAL_SHIFTS: &str = "bootstrap distinct physical shifts";
const EXACT_OBSTRUCTIONS: &str = "retained exact candidate obstructions";

/// Immutable shared inputs for serial, one-task semantic transactions.
#[derive(Debug)]
pub(crate) struct InteriorCampaignAdapter<'inputs, 'sources, 'family> {
    generator: &'inputs ParametricIbpGenerator<'family>,
    completed: &'inputs CompletedIbpSourceRows,
    incidence: &'inputs OrdinarySourceIncidenceIndex<'sources>,
    limits: InteriorCampaignLimits,
}

impl<'inputs, 'sources, 'family> InteriorCampaignAdapter<'inputs, 'sources, 'family> {
    pub(crate) fn try_new(
        generator: &'inputs ParametricIbpGenerator<'family>,
        completed: &'inputs CompletedIbpSourceRows,
        incidence: &'inputs OrdinarySourceIncidenceIndex<'sources>,
        limits: InteriorCampaignLimits,
    ) -> Result<Self, InteriorCampaignError> {
        generator
            .validate_completed_scope(completed)
            .map_err(InteriorCampaignError::SourceScope)?;
        if !completed.is_complete_ordinary() {
            return Err(InteriorCampaignError::WrongSourceLayout {
                actual: completed.layout_name(),
            });
        }
        if incidence.arity() != generator.context().index_count() {
            return Err(InteriorCampaignError::Scope {
                detail: "incidence index and generator have different arities",
            });
        }
        if incidence.context_fingerprint() != generator.context().fingerprint() {
            return Err(InteriorCampaignError::Scope {
                detail: "incidence index and generator have different coefficient contexts",
            });
        }
        if incidence.source_count() != completed.source_row_count() {
            return Err(InteriorCampaignError::Scope {
                detail: "incidence index and completed source module have different row counts",
            });
        }
        incidence
            .try_verify_limits(limits.replay.scheduler.source_discovery)
            .map_err(InteriorCampaignError::SourceDiscovery)?;
        Ok(Self {
            generator,
            completed,
            incidence,
            limits,
        })
    }

    /// Bind one plan task to the exact current ledger revision and require its
    /// complete planner box to occur in that ledger's exact partition.
    pub(crate) fn try_bind_task<'plan>(
        &self,
        plan: &'plan InteriorSimplexPlan,
        task: &'plan InteriorSimplexTask,
        ledger: &CanonicalExactOwnerLedger,
    ) -> Result<InteriorCampaignTaskBinding<'plan>, InteriorCampaignError> {
        plan.validate_task(task)
            .map_err(InteriorCampaignError::Plan)?;
        self.validate_task_scope(task, ledger)?;
        let partition = ledger.try_clone_uncovered_partition()?;
        if !partition.boxes().iter().any(|cell| {
            cell.lower() == task.key().box_lower() && cell.upper() == task.key().box_upper()
        }) {
            return Err(InteriorCampaignError::StalePlanGeometry);
        }
        Ok(InteriorCampaignTaskBinding::new(
            plan,
            task,
            ledger.snapshot_identity(),
        ))
    }

    /// Execute and transactionally apply one previously bound task. This
    /// serial adapter deliberately makes no exhaustion, publication,
    /// parallelism, or closure claim beyond the exact ledger status
    /// represented by its typed outcome.
    pub(crate) fn try_run_task(
        &self,
        binding: InteriorCampaignTaskBinding<'_>,
        ledger: &mut CanonicalExactOwnerLedger,
        probes: impl IntoIterator<Item = CampaignModularProbe>,
    ) -> Result<InteriorCampaignTaskReport, InteriorCampaignError> {
        binding
            .plan
            .validate_task(binding.task)
            .map_err(InteriorCampaignError::Plan)?;
        ledger.try_require_current_snapshot(&binding.ledger_snapshot)?;
        self.validate_task_scope(binding.task, ledger)?;
        let (bootstrap, anchor) = self.try_build_anchor(binding.task)?;
        let replay = try_run_interior_replay_task(
            self.generator,
            self.completed,
            binding.task.target_shift().clone(),
            anchor,
            ledger.predecessor_snapshot().clone(),
            ledger.ordering(),
            probes,
            self.limits.replay,
        )?;
        // Replay's owner limit bounds transient exact-candidate allocation.
        // This outer cap separately decides whether the already compiled
        // obstruction evidence may be retained in the returned task report;
        // failure occurs before any ledger mutation.
        let exact_obstructions = exact_obstruction_count(&replay);
        check_limit(
            EXACT_OBSTRUCTIONS,
            exact_obstructions,
            self.limits.max_retained_exact_obstructions,
        )?;
        let delta = match replay.disposition() {
            InteriorReplayRunDisposition::OwnerProposal {
                proposal: ExactExecutableOwnerProposal::Compiled { owner, .. },
                ..
            } => Some(ledger.try_apply_owner(owner.clone())?),
            _ => None,
        };
        let census = InteriorCampaignCensus::new(bootstrap, &replay, exact_obstructions);
        Ok(InteriorCampaignTaskReport::new(
            binding.task.canonical_ordinal(),
            binding.ledger_snapshot.revision(),
            census,
            replay,
            delta,
        ))
    }

    fn validate_task_scope(
        &self,
        task: &InteriorSimplexTask,
        ledger: &CanonicalExactOwnerLedger,
    ) -> Result<(), InteriorCampaignError> {
        let predecessor = ledger.predecessor_snapshot();
        if task.key().sector() != ledger.sector() {
            return Err(InteriorCampaignError::Scope {
                detail: "simplex task and canonical ledger have different sectors",
            });
        }
        if task.target_shift().len() != self.incidence.arity()
            || task.lattice_target().len() != self.incidence.arity()
        {
            return Err(InteriorCampaignError::Scope {
                detail: "simplex task and source incidence have different arities",
            });
        }
        if predecessor.arity() != self.incidence.arity()
            || predecessor.family_fingerprint() != self.incidence.family_fingerprint()
            || predecessor.context_fingerprint() != self.incidence.context_fingerprint()
        {
            return Err(InteriorCampaignError::Scope {
                detail: "canonical ledger and source incidence have different exact scopes",
            });
        }
        Ok(())
    }

    fn try_build_anchor(
        &self,
        task: &InteriorSimplexTask,
    ) -> Result<(InteriorCampaignBootstrapCensus, MaximalStratumAnchor), InteriorCampaignError>
    {
        let discovery = self.limits.replay.scheduler.source_discovery;
        let nominations = self
            .incidence
            .try_nominate_target_unit(task.target_shift(), discovery)
            .map_err(InteriorCampaignError::SourceDiscovery)?;
        let selected = self
            .generator
            .translate_selected_completed_source_rows(
                self.completed,
                nominations.requests().iter().cloned(),
                self.limits.replay.scheduler.campaign.translated_sources,
            )
            .map_err(InteriorCampaignError::SourceTranslation)?;
        if selected.requests() != nominations.requests() {
            return Err(InteriorCampaignError::Invariant {
                detail: "bootstrap translation changed the canonical request set",
            });
        }
        if selected.family_fingerprint() != self.incidence.family_fingerprint()
            || selected.context_fingerprint() != self.incidence.context_fingerprint()
        {
            return Err(InteriorCampaignError::Scope {
                detail: "bootstrap translation and source incidence have different exact scopes",
            });
        }

        let mut physical_shift_occurrences = 0usize;
        for source in selected.sources() {
            physical_shift_occurrences = checked_add(
                PHYSICAL_SHIFTS,
                physical_shift_occurrences,
                source.terms().len(),
            )?;
        }
        check_limit(
            PHYSICAL_SHIFTS,
            physical_shift_occurrences,
            self.limits.max_bootstrap_physical_shift_occurrences,
        )?;
        let physical_shift_coordinate_cells = checked_mul(
            PHYSICAL_SHIFT_CELLS,
            physical_shift_occurrences,
            self.incidence.arity(),
        )?;
        check_limit(
            PHYSICAL_SHIFT_CELLS,
            physical_shift_coordinate_cells,
            self.limits.max_bootstrap_physical_shift_coordinate_cells,
        )?;
        // `sort_unstable` does not expose a comparator census. Reserve the
        // same deterministic logical n*ceil(log2(max(n, 2))) envelope used by
        // the other source-discovery canonicalization boundaries before any
        // view allocation or sorting begins.
        let physical_shift_sort_work = logical_sort_work(physical_shift_occurrences)?;
        check_limit(
            PHYSICAL_SHIFT_SORT_WORK,
            physical_shift_sort_work,
            self.limits.max_bootstrap_physical_shift_sort_work,
        )?;
        let mut physical_shifts: Vec<&[i64]> = Vec::new();
        physical_shifts
            .try_reserve_exact(physical_shift_occurrences)
            .map_err(|_| InteriorCampaignError::AllocationFailure {
                resource: PHYSICAL_SHIFTS,
                requested: physical_shift_occurrences,
            })?;
        physical_shifts.extend(
            selected
                .sources()
                .iter()
                .flat_map(|source| source.terms().keys())
                .map(|shift| shift.values()),
        );
        physical_shifts.sort_unstable();
        physical_shifts.dedup();
        check_limit(
            DISTINCT_PHYSICAL_SHIFTS,
            physical_shifts.len(),
            self.limits.max_bootstrap_distinct_physical_shifts,
        )?;

        let domain = SectorMonotoneDomain::try_maximal_for_rule(
            task.key().sector().clone(),
            task.target_shift().values(),
            &physical_shifts,
        )
        .map_err(InteriorCampaignError::Sector)?;
        let stratum = DecoratedStratum::try_guard_blind(
            self.incidence.family_fingerprint(),
            self.incidence.context_fingerprint(),
            domain,
            self.limits.replay.scheduler.campaign.stratum,
        )
        .map_err(InteriorCampaignError::Stratum)?;
        let anchor =
            MaximalStratumAnchor::try_new(stratum, self.limits.replay.scheduler.campaign.stratum)
                .map_err(InteriorCampaignError::Stratum)?;
        let census = InteriorCampaignBootstrapCensus::new(
            nominations.raw_incidence_visits(),
            nominations.unique_before_existing_exclusion(),
            nominations.excluded_existing_requests(),
            nominations.requests().len(),
            selected.len(),
            physical_shift_occurrences,
            physical_shifts.len(),
            physical_shift_coordinate_cells,
            physical_shift_sort_work,
        );
        Ok((census, anchor))
    }
}

fn exact_obstruction_count(report: &super::super::InteriorReplayTaskReport) -> usize {
    match report.disposition() {
        InteriorReplayRunDisposition::OwnerProposal {
            proposal: ExactExecutableOwnerProposal::Compiled { obstructions, .. },
            ..
        } => obstructions.len(),
        InteriorReplayRunDisposition::OwnerProposal {
            proposal: ExactExecutableOwnerProposal::Incomplete(proposal),
            ..
        } => proposal.obstructions().len(),
        _ => 0,
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, InteriorCampaignError> {
    left.checked_add(right)
        .ok_or(InteriorCampaignError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, InteriorCampaignError> {
    left.checked_mul(right)
        .ok_or(InteriorCampaignError::ResourceCountOverflow { resource })
}

fn logical_sort_work(count: usize) -> Result<usize, InteriorCampaignError> {
    let normalized = count.max(2);
    let levels = usize::BITS as usize - normalized.saturating_sub(1).leading_zeros() as usize;
    checked_mul(PHYSICAL_SHIFT_SORT_WORK, count, levels)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), InteriorCampaignError> {
    if requested > limit {
        Err(InteriorCampaignError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
