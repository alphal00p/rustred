use crate::foundry::completion::source_discovery::leader_walk::RequestedDomainTask;
use crate::foundry::completion::source_discovery::{
    AccumulatedSourceRequests, CampaignModularProbe, CanonicalExactOwnerLedger,
    ExactExecutableOwnerProposal, InitialParentSourceProposal, InteriorReplayRunDisposition,
    OrdinarySourceIncidenceIndex, RequestedDomainSupportProposal, try_run_interior_replay_task,
    try_run_interior_replay_task_with_initial_parent_proposal,
};
use crate::foundry::completion::stratum::{
    CampaignStratumAnchor, DecoratedStratum, MaximalStratumAnchor,
};
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::{InteriorBounds, SectorMonotoneDomain};

use super::{
    ProbeCampaignBootstrapCensus, ProbeCampaignCensus, ProbeCampaignError,
    ProbeCampaignEvaluatedTask, ProbeCampaignLimits, ProbeCampaignPlannedTask,
    ProbeCampaignTaskBinding, ProbeCampaignTaskReport,
};

const PHYSICAL_SHIFTS: &str = "bootstrap physical shift occurrences";
const PHYSICAL_SHIFT_CELLS: &str = "bootstrap physical shift coordinate cells";
const PHYSICAL_SHIFT_SORT_WORK: &str = "bootstrap physical shift logical sort work reservation";
const DISTINCT_PHYSICAL_SHIFTS: &str = "bootstrap distinct physical shifts";
const EXACT_OBSTRUCTIONS: &str = "retained exact candidate obstructions";

/// Immutable shared inputs for serial, one-task semantic transactions.
#[derive(Debug)]
pub(crate) struct ProbeCampaignAdapter<'inputs, 'sources, 'family> {
    generator: &'inputs ParametricIbpGenerator<'family>,
    completed: &'inputs CompletedIbpSourceRows,
    zero_sources: &'sources crate::identity::TranslatedSourceBatch,
    incidence: OrdinarySourceIncidenceIndex<'sources>,
    limits: ProbeCampaignLimits,
}

impl<'inputs, 'sources, 'family> ProbeCampaignAdapter<'inputs, 'sources, 'family> {
    pub(crate) fn try_new(
        generator: &'inputs ParametricIbpGenerator<'family>,
        completed: &'inputs CompletedIbpSourceRows,
        zero_sources: &'sources crate::identity::TranslatedSourceBatch,
        limits: ProbeCampaignLimits,
    ) -> Result<Self, ProbeCampaignError> {
        generator
            .validate_completed_scope(completed)
            .map_err(ProbeCampaignError::SourceScope)?;
        if !completed.is_complete_ordinary() {
            return Err(ProbeCampaignError::WrongSourceLayout {
                actual: completed.layout_name(),
            });
        }
        let incidence = OrdinarySourceIncidenceIndex::try_new(
            zero_sources,
            limits.replay.scheduler.source_discovery,
        )
        .map_err(ProbeCampaignError::SourceDiscovery)?;
        if incidence.arity() != generator.context().index_count() {
            return Err(ProbeCampaignError::Scope {
                detail: "incidence index and generator have different arities",
            });
        }
        if incidence.context_fingerprint() != generator.context().fingerprint() {
            return Err(ProbeCampaignError::Scope {
                detail: "incidence index and generator have different coefficient contexts",
            });
        }
        if incidence.source_count() != completed.source_row_count() {
            return Err(ProbeCampaignError::Scope {
                detail: "incidence index and completed source module have different row counts",
            });
        }
        if !incidence.exactly_replays_completed(completed) {
            return Err(ProbeCampaignError::Scope {
                detail: "zero-source incidence is not the exact translation of the completed source barrier",
            });
        }
        Ok(Self {
            generator,
            completed,
            zero_sources,
            incidence,
            limits,
        })
    }

    /// Rebind the same immutable ordinary module to a different bounded
    /// resource envelope. This is used by adaptive research drivers that
    /// widen only the exact resource named by a typed resumable stop.
    pub(crate) fn try_with_limits(
        &self,
        limits: ProbeCampaignLimits,
    ) -> Result<Self, ProbeCampaignError> {
        Self::try_new(self.generator, self.completed, self.zero_sources, limits)
    }

    /// Bind one plan task to the exact current ledger revision and require its
    /// complete planner box to occur in that ledger's exact partition.
    pub(crate) fn try_bind_task<'plan, Task: ProbeCampaignPlannedTask>(
        &self,
        plan: &'plan Task::Plan,
        task: &'plan Task,
        ledger: &CanonicalExactOwnerLedger,
    ) -> Result<ProbeCampaignTaskBinding<'plan, Task>, ProbeCampaignError> {
        task.validate_in_plan(plan)?;
        self.validate_task_scope(task, ledger)?;
        let current_revision = ledger.revision().get();
        if task.planned_ledger_revision() != current_revision {
            return Err(ProbeCampaignError::StaleLedgerRevision {
                planned: task.planned_ledger_revision(),
                current: current_revision,
            });
        }
        if !ledger.has_exact_uncovered_box(task.parent_box_lower(), task.parent_box_upper()) {
            return Err(ProbeCampaignError::StaleParentGeometry);
        }
        Ok(ProbeCampaignTaskBinding::new(
            plan,
            task,
            ledger.snapshot_identity(),
        ))
    }

    /// Evaluate one bound task without mutating the exact owner ledger.
    /// Application must later rejoin the same opaque ledger snapshot through
    /// [`Self::try_apply_evaluated_task`].
    pub(crate) fn try_evaluate_task<Task: ProbeCampaignPlannedTask>(
        &self,
        binding: ProbeCampaignTaskBinding<'_, Task>,
        ledger: &CanonicalExactOwnerLedger,
        probes: impl IntoIterator<Item = CampaignModularProbe>,
    ) -> Result<ProbeCampaignEvaluatedTask, ProbeCampaignError> {
        binding.task.validate_in_plan(binding.plan)?;
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
        let census = ProbeCampaignCensus::new(bootstrap, &replay, exact_obstructions);
        Ok(ProbeCampaignEvaluatedTask::new(
            binding.task.canonical_ordinal(),
            binding.ledger_snapshot,
            census,
            replay,
        ))
    }

    /// Evaluate one bound requested-domain task with detached canonical
    /// parent support. The support proposal has no source or owner authority:
    /// this method first requires its semantic domain to match the original
    /// request, then regenerates an [`super::super::InitialParentSourceProposal`]
    /// through this adapter's trusted ordinary-source incidence index before
    /// entering the existing modular and exact-replay path.
    ///
    /// The explicit support-aware requested coordinator selects this path
    /// only after a fresh plan and exact semantic-domain lookup. Ordinary
    /// requested tasks without matching detached support retain the existing
    /// evaluation path.
    pub(crate) fn try_evaluate_requested_task_with_parent_support(
        &self,
        binding: ProbeCampaignTaskBinding<'_, RequestedDomainTask>,
        ledger: &CanonicalExactOwnerLedger,
        support: &RequestedDomainSupportProposal,
        probes: impl IntoIterator<Item = CampaignModularProbe>,
    ) -> Result<ProbeCampaignEvaluatedTask, ProbeCampaignError> {
        binding.task.validate_in_plan(binding.plan)?;
        ledger.try_require_current_snapshot(&binding.ledger_snapshot)?;
        self.validate_task_scope(binding.task, ledger)?;
        let task_key = binding.task.key();
        let support_key = support.domain();
        if support_key.stable_scope_key() != task_key.stable_scope_key()
            || support_key.sector() != task_key.sector()
            || support_key.point() != task_key.requested_domain_lower()
            || support_key.symbolic_axes() != task_key.symbolic_axes()
        {
            return Err(ProbeCampaignError::Scope {
                detail: "parent-support proposal and requested task have different semantic domains",
            });
        }
        let initial_parent_proposal = self
            .incidence
            .try_nominate_initial_parent_support(
                self.completed,
                support.parent_support(),
                self.limits.replay.scheduler.source_discovery,
            )
            .map_err(ProbeCampaignError::SourceDiscovery)?;
        let (bootstrap, anchor) = self.try_build_anchor_with_initial_parent_proposal(
            binding.task,
            &initial_parent_proposal,
        )?;
        let replay = try_run_interior_replay_task_with_initial_parent_proposal(
            self.generator,
            self.completed,
            binding.task.target_shift().clone(),
            anchor,
            ledger.predecessor_snapshot().clone(),
            ledger.ordering(),
            initial_parent_proposal,
            probes,
            self.limits.replay,
        )?;
        let exact_obstructions = exact_obstruction_count(&replay);
        check_limit(
            EXACT_OBSTRUCTIONS,
            exact_obstructions,
            self.limits.max_retained_exact_obstructions,
        )?;
        let census = ProbeCampaignCensus::new(bootstrap, &replay, exact_obstructions);
        Ok(ProbeCampaignEvaluatedTask::new(
            binding.task.canonical_ordinal(),
            binding.ledger_snapshot,
            census,
            replay,
        ))
    }

    /// Revalidate and transactionally apply one evaluated task. The opaque
    /// snapshot join prevents delayed worker results from crossing any owner
    /// mutation, including a change that left the geometric cover equal.
    pub(crate) fn try_apply_evaluated_task(
        &self,
        evaluated: ProbeCampaignEvaluatedTask,
        ledger: &mut CanonicalExactOwnerLedger,
    ) -> Result<ProbeCampaignTaskReport, ProbeCampaignError> {
        ledger.try_require_current_snapshot(&evaluated.planned_ledger_snapshot)?;
        let delta = match evaluated.replay.disposition() {
            InteriorReplayRunDisposition::OwnerProposal {
                proposal: ExactExecutableOwnerProposal::Compiled { owner, .. },
                ..
            } => Some(ledger.try_apply_owner(owner.clone())?),
            _ => None,
        };
        Ok(evaluated.into_report(delta))
    }

    /// Evaluate and immediately apply one task in serial canonical order.
    /// This compatibility wrapper deliberately makes no exhaustion,
    /// publication, parallelism, or closure claim beyond the exact ledger
    /// status represented by its typed outcome.
    pub(crate) fn try_run_task<Task: ProbeCampaignPlannedTask>(
        &self,
        binding: ProbeCampaignTaskBinding<'_, Task>,
        ledger: &mut CanonicalExactOwnerLedger,
        probes: impl IntoIterator<Item = CampaignModularProbe>,
    ) -> Result<ProbeCampaignTaskReport, ProbeCampaignError> {
        let evaluated = self.try_evaluate_task(binding, ledger, probes)?;
        self.try_apply_evaluated_task(evaluated, ledger)
    }

    pub(crate) fn probe_base_parameter_count(&self) -> usize {
        self.generator.context().base().parameter_names().len()
    }

    pub(crate) fn probe_chart_arity(&self) -> usize {
        self.incidence.arity()
    }

    pub(crate) const fn limits(&self) -> ProbeCampaignLimits {
        self.limits
    }

    pub(crate) fn validate_ledger_scope(
        &self,
        ledger: &CanonicalExactOwnerLedger,
    ) -> Result<(), ProbeCampaignError> {
        let predecessor = ledger.predecessor_snapshot();
        if ledger.sector().arity() != self.incidence.arity() {
            return Err(ProbeCampaignError::Scope {
                detail: "canonical ledger sector and source incidence have different arities",
            });
        }
        if predecessor.arity() != self.incidence.arity()
            || predecessor.family_fingerprint() != self.incidence.family_fingerprint()
            || predecessor.context_fingerprint() != self.incidence.context_fingerprint()
        {
            return Err(ProbeCampaignError::Scope {
                detail: "canonical ledger and source incidence have different exact scopes",
            });
        }
        Ok(())
    }

    fn validate_task_scope<Task: ProbeCampaignPlannedTask>(
        &self,
        task: &Task,
        ledger: &CanonicalExactOwnerLedger,
    ) -> Result<(), ProbeCampaignError> {
        self.validate_ledger_scope(ledger)?;
        if task.sector() != ledger.sector() {
            return Err(ProbeCampaignError::Scope {
                detail: "planned task and canonical ledger have different sectors",
            });
        }
        if task.target_shift().len() != self.incidence.arity()
            || task.lattice_target().len() != self.incidence.arity()
        {
            return Err(ProbeCampaignError::Scope {
                detail: "planned task and source incidence have different arities",
            });
        }
        Ok(())
    }

    fn try_build_anchor<Task: ProbeCampaignPlannedTask>(
        &self,
        task: &Task,
    ) -> Result<(ProbeCampaignBootstrapCensus, CampaignStratumAnchor), ProbeCampaignError> {
        self.try_build_anchor_from_source_frame(task, None)
    }

    /// Build the epoch-zero anchor from the exact canonical request frame
    /// that the scheduler will regenerate. This prevents detached support
    /// from widening source geometry after a bootstrap-only anchor has
    /// already been fixed.
    fn try_build_anchor_with_initial_parent_proposal<Task: ProbeCampaignPlannedTask>(
        &self,
        task: &Task,
        proposal: &InitialParentSourceProposal,
    ) -> Result<(ProbeCampaignBootstrapCensus, CampaignStratumAnchor), ProbeCampaignError> {
        self.try_build_anchor_from_source_frame(task, Some(proposal))
    }

    fn try_build_anchor_from_source_frame<Task: ProbeCampaignPlannedTask>(
        &self,
        task: &Task,
        initial_parent_proposal: Option<&InitialParentSourceProposal>,
    ) -> Result<(ProbeCampaignBootstrapCensus, CampaignStratumAnchor), ProbeCampaignError> {
        let has_additional_initial_requests = initial_parent_proposal.is_some();
        let discovery = self.limits.replay.scheduler.source_discovery;
        let nominations = self
            .incidence
            .try_nominate_target_unit(task.target_shift(), discovery)
            .map_err(ProbeCampaignError::SourceDiscovery)?;
        let merged_requests = initial_parent_proposal
            .map(|proposal| {
                AccumulatedSourceRequests::try_new(
                    self.incidence.arity(),
                    nominations
                        .requests()
                        .iter()
                        .cloned()
                        .chain(proposal.requests().iter().cloned()),
                    self.limits.replay.scheduler.campaign,
                )
            })
            .transpose()
            .map_err(ProbeCampaignError::InitialRequestCampaign)?;
        let frame_requests = match &merged_requests {
            Some(requests) => requests.requests(),
            None => nominations.requests(),
        };
        let selected = self
            .generator
            .translate_selected_completed_source_rows(
                self.completed,
                frame_requests.iter().cloned(),
                self.limits.replay.scheduler.campaign.translated_sources,
            )
            .map_err(ProbeCampaignError::SourceTranslation)?;
        if selected.requests() != frame_requests {
            return Err(ProbeCampaignError::Invariant {
                detail: "anchor-frame translation changed the canonical request set",
            });
        }
        if selected.family_fingerprint() != self.incidence.family_fingerprint()
            || selected.context_fingerprint() != self.incidence.context_fingerprint()
        {
            return Err(ProbeCampaignError::Scope {
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
            .map_err(|_| ProbeCampaignError::AllocationFailure {
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

        let maximal_domain = SectorMonotoneDomain::try_maximal_for_rule(
            task.sector().clone(),
            task.target_shift().values(),
            &physical_shifts,
        )
        .map_err(ProbeCampaignError::Sector)?;
        let (domain, restricted) = match task.restricted_symbolic_axes() {
            None => (maximal_domain, false),
            Some(symbolic_axes) => {
                let fixed_indices = task.restricted_fixed_indices();
                if fixed_indices.is_some_and(|indices| indices.len() != self.incidence.arity()) {
                    return Err(ProbeCampaignError::Invariant {
                        detail: "restricted task fixed-index vector has the wrong arity",
                    });
                }
                if symbolic_axes.windows(2).any(|pair| pair[0] >= pair[1])
                    || symbolic_axes
                        .last()
                        .is_some_and(|&position| position >= self.incidence.arity())
                {
                    return Err(ProbeCampaignError::Invariant {
                        detail: "boundary task symbolic axes are not canonical in-range positions",
                    });
                }
                if symbolic_axes.len() == self.incidence.arity() {
                    (maximal_domain, false)
                } else {
                    let mut bounds = Vec::new();
                    bounds
                        .try_reserve_exact(self.incidence.arity())
                        .map_err(|_| ProbeCampaignError::AllocationFailure {
                            resource: "restricted bootstrap stratum bounds",
                            requested: self.incidence.arity(),
                        })?;
                    for (position, (&maximal, index)) in maximal_domain
                        .bounds()
                        .iter()
                        .zip(task.sector().corner_indices())
                        .enumerate()
                    {
                        if symbolic_axes.binary_search(&position).is_ok() {
                            bounds.push(maximal);
                        } else {
                            let index = fixed_indices.map_or(index, |indices| indices[position]);
                            if !maximal.contains(index) {
                                return Err(ProbeCampaignError::Scope {
                                    detail: "boundary task fixed coordinate lies outside the bootstrap recurrence domain",
                                });
                            }
                            bounds.push(InteriorBounds::new(index, index));
                        }
                    }
                    let restricted_domain = SectorMonotoneDomain::try_new_for_rule(
                        task.sector().clone(),
                        bounds,
                        task.target_shift().values(),
                        &physical_shifts,
                    )
                    .map_err(ProbeCampaignError::Sector)?;
                    (restricted_domain, true)
                }
            }
        };
        let stratum = DecoratedStratum::try_guard_blind(
            self.incidence.family_fingerprint(),
            self.incidence.context_fingerprint(),
            domain,
            self.limits.replay.scheduler.campaign.stratum,
        )
        .map_err(ProbeCampaignError::Stratum)?;
        let anchor = if restricted || has_additional_initial_requests {
            // A support-assisted anchor is maximal for the exact merged
            // epoch-zero frame, but is intentionally fixed as a campaign
            // restriction. Canonical replay first authenticates its
            // target-unit bootstrap before rebuilding that same merged frame;
            // the restricted lane permits the smaller exact anchor through
            // both materializations without misrepresenting it as maximal for
            // the bootstrap-only intermediate frame.
            CampaignStratumAnchor::try_restricted(
                stratum,
                self.limits.replay.scheduler.campaign.stratum,
            )
            .map_err(ProbeCampaignError::Stratum)?
        } else {
            MaximalStratumAnchor::try_new(stratum, self.limits.replay.scheduler.campaign.stratum)
                .map(CampaignStratumAnchor::from)
                .map_err(ProbeCampaignError::Stratum)?
        };
        let census = ProbeCampaignBootstrapCensus::new(
            nominations.raw_incidence_visits(),
            nominations.unique_before_existing_exclusion(),
            nominations.excluded_existing_requests(),
            frame_requests.len(),
            selected.len(),
            physical_shift_occurrences,
            physical_shifts.len(),
            physical_shift_coordinate_cells,
            physical_shift_sort_work,
        );
        Ok((census, anchor))
    }

    /// Test-only view of the exact task-to-stratum adapter boundary.
    ///
    /// Production callers must go through task binding/evaluation so ledger
    /// authority is rejoined.  Focused planner/replay regressions use this
    /// seam to inspect the derived restricted stratum without weakening that
    /// production transaction.
    #[cfg(test)]
    pub(crate) fn try_build_anchor_for_test<Task: ProbeCampaignPlannedTask>(
        &self,
        task: &Task,
    ) -> Result<(ProbeCampaignBootstrapCensus, CampaignStratumAnchor), ProbeCampaignError> {
        self.try_build_anchor(task)
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
) -> Result<usize, ProbeCampaignError> {
    left.checked_add(right)
        .ok_or(ProbeCampaignError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ProbeCampaignError> {
    left.checked_mul(right)
        .ok_or(ProbeCampaignError::ResourceCountOverflow { resource })
}

fn logical_sort_work(count: usize) -> Result<usize, ProbeCampaignError> {
    let normalized = count.max(2);
    // `normalized >= 2`, so this exact subtraction cannot underflow.
    let levels = usize::BITS as usize - (normalized - 1).leading_zeros() as usize;
    checked_mul(PHYSICAL_SHIFT_SORT_WORK, count, levels)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ProbeCampaignError> {
    if requested > limit {
        Err(ProbeCampaignError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
