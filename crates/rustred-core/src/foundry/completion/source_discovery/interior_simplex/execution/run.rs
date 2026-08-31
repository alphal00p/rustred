use crate::foundry::completion::source_discovery::scheduler::ProbeLocalObstructionScheduler;
use crate::foundry::completion::source_discovery::{
    CampaignModularProbe, OrdinarySourceIncidenceIndex,
};
use crate::foundry::completion::stratum::ImmutableOwnerSnapshot;
use crate::identity::{CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator};
use crate::sector::OrderingPolicy;

use super::super::InteriorSimplexPlan;
use super::InteriorSimplexExecutionError;
use super::bootstrap::{BootstrapAggregateBudget, try_derive_bootstrap_stratum};
use super::compact::{RetainedIterationBudget, try_compact_scheduler_report};
use super::limits::InteriorSimplexExecutionLimits;
use super::model::{
    InteriorSimplexExecutionReport, InteriorSimplexRetainedPayloadCensus,
    InteriorSimplexTaskExecutionReport,
};
use super::resource::{check_limit, checked_add, checked_mul, try_reserve_exact};

/// Immutable-input, serial executor for one proposal-only simplex plan.
#[derive(Debug)]
pub(crate) struct InteriorSimplexProbeExecutor<'inputs, 'family> {
    plan: InteriorSimplexPlan,
    generator: &'inputs ParametricIbpGenerator<'family>,
    completed: &'inputs CompletedIbpSourceRows,
    owners: ImmutableOwnerSnapshot,
    ordering: OrderingPolicy,
    probes: Vec<CampaignModularProbe>,
    limits: InteriorSimplexExecutionLimits,
    retained_payload: InteriorSimplexRetainedPayloadCensus,
}

impl<'inputs, 'family> InteriorSimplexProbeExecutor<'inputs, 'family> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_new(
        plan: InteriorSimplexPlan,
        generator: &'inputs ParametricIbpGenerator<'family>,
        completed: &'inputs CompletedIbpSourceRows,
        owners: ImmutableOwnerSnapshot,
        ordering: OrderingPolicy,
        probes: impl IntoIterator<Item = CampaignModularProbe>,
        limits: InteriorSimplexExecutionLimits,
    ) -> Result<Self, InteriorSimplexExecutionError> {
        generator
            .validate_completed_scope(completed)
            .map_err(InteriorSimplexExecutionError::SourceScope)?;
        if !completed.is_complete_ordinary() {
            return Err(InteriorSimplexExecutionError::WrongSourceLayout {
                actual: completed.layout_name(),
            });
        }
        let arity = generator.context().index_count();
        if owners.arity() != arity {
            return Err(InteriorSimplexExecutionError::WrongImmutableOwnerScope {
                detail: "owner snapshot and generator have different arities",
            });
        }
        if owners.context_fingerprint() != generator.context().fingerprint() {
            return Err(InteriorSimplexExecutionError::WrongImmutableOwnerScope {
                detail: "owner snapshot and generator have different coefficient contexts",
            });
        }
        validate_plan(&plan, arity, limits)?;

        let mut retained_probes = Vec::new();
        let mut probe_coordinate_cells = 0usize;
        for probe in probes {
            let requested = checked_add("declared probes", retained_probes.len(), 1)?;
            check_limit("declared probes", requested, limits.scheduler.max_probes)?;
            let cells = checked_add(
                "declared probe coordinate cells",
                probe.base_parameters().len(),
                probe.chart_coordinates().len(),
            )?;
            probe_coordinate_cells = checked_add(
                "declared probe coordinate cells",
                probe_coordinate_cells,
                cells,
            )?;
            try_reserve_exact(&mut retained_probes, 1, "declared probes")?;
            retained_probes.push(probe);
        }
        if retained_probes.is_empty() {
            return Err(InteriorSimplexExecutionError::EmptyProbeSchedule);
        }
        let task_probe_runs =
            checked_mul("task-probe runs", plan.tasks().len(), retained_probes.len())?;
        check_limit(
            "task-probe runs",
            task_probe_runs,
            limits.max_task_probe_runs,
        )?;
        let retained_probe_coordinate_cells = checked_mul(
            "retained task-probe coordinate cells",
            plan.tasks().len(),
            probe_coordinate_cells,
        )?;
        check_limit(
            "retained task-probe coordinate cells",
            retained_probe_coordinate_cells,
            limits.max_retained_probe_coordinate_cells,
        )?;

        let (task_key_coordinate_cells, stable_scope_key_bytes) =
            retained_task_identity_payload(&plan)?;
        check_limit(
            "retained task-key coordinate cells",
            task_key_coordinate_cells,
            limits.max_retained_task_key_coordinate_cells,
        )?;
        check_limit(
            "retained stable-scope-key bytes",
            stable_scope_key_bytes,
            limits.max_retained_stable_scope_key_bytes,
        )?;
        let retained_payload = InteriorSimplexRetainedPayloadCensus::new(
            plan.tasks().len(),
            task_probe_runs,
            task_key_coordinate_cells,
            stable_scope_key_bytes,
            retained_probe_coordinate_cells,
            0,
        );

        Ok(Self {
            plan,
            generator,
            completed,
            owners,
            ordering,
            probes: retained_probes,
            limits,
            retained_payload,
        })
    }

    /// Run a fresh scheduler for every task in canonical serial order.
    pub(crate) fn run(
        self,
    ) -> Result<InteriorSimplexExecutionReport, InteriorSimplexExecutionError> {
        let zero = IntegralShift::try_new_with_component_limit(
            std::iter::repeat_n(0, self.generator.context().index_count()),
            self.limits.scheduler.source_discovery.max_arity,
        )
        .map_err(InteriorSimplexExecutionError::SourceTranslation)?;
        let zero_sources = self
            .generator
            .translate_completed_source_rows(
                self.completed,
                [zero],
                self.limits.scheduler.source_discovery.translation,
            )
            .map_err(InteriorSimplexExecutionError::SourceTranslation)?;
        if zero_sources.family_fingerprint() != self.owners.family_fingerprint()
            || zero_sources.context_fingerprint() != self.owners.context_fingerprint()
        {
            return Err(InteriorSimplexExecutionError::WrongImmutableOwnerScope {
                detail: "owner snapshot and complete ordinary source module have different identities",
            });
        }
        let incidence = OrdinarySourceIncidenceIndex::try_new(
            &zero_sources,
            self.limits.scheduler.source_discovery,
        )
        .map_err(InteriorSimplexExecutionError::SourceDiscovery)?;

        let mut reports = Vec::new();
        try_reserve_exact(
            &mut reports,
            self.plan.tasks().len(),
            "retained task reports",
        )?;
        let mut bootstrap_budget = BootstrapAggregateBudget::default();
        let mut iteration_budget = RetainedIterationBudget::default();
        for task in self.plan.tasks() {
            self.plan
                .validate_task(task)
                .map_err(InteriorSimplexExecutionError::Plan)?;
            let (bootstrap, stratum) = try_derive_bootstrap_stratum(
                task,
                self.generator,
                self.completed,
                &incidence,
                self.limits,
                &mut bootstrap_budget,
            )?;
            let scheduler = ProbeLocalObstructionScheduler::try_new(
                self.generator,
                self.completed,
                task.target_shift().clone(),
                stratum,
                self.owners.clone(),
                self.ordering,
                self.probes.iter().cloned(),
                self.limits.scheduler,
            )
            .map_err(InteriorSimplexExecutionError::Scheduler)?
            .run()
            .map_err(InteriorSimplexExecutionError::Scheduler)?;
            let (probes, census) =
                try_compact_scheduler_report(scheduler, self.limits, &mut iteration_budget)?;
            reports.push(InteriorSimplexTaskExecutionReport::new(
                task.canonical_ordinal(),
                task.key().clone(),
                task.target_shift().clone(),
                bootstrap,
                probes,
                census,
            ));
        }
        if reports.len() != self.plan.tasks().len()
            || reports
                .iter()
                .enumerate()
                .any(|(ordinal, report)| report.canonical_ordinal() != ordinal)
        {
            return Err(InteriorSimplexExecutionError::Invariant {
                detail: "serial executor did not retain one report per canonical task",
            });
        }
        Ok(InteriorSimplexExecutionReport::new(
            self.plan.epoch_ordinal(),
            self.plan.interior_margin(),
            self.plan.polynomial_degree_ceiling(),
            self.retained_payload
                .with_iteration_records(iteration_budget.records()),
            reports,
        ))
    }
}

fn retained_task_identity_payload(
    plan: &InteriorSimplexPlan,
) -> Result<(usize, usize), InteriorSimplexExecutionError> {
    let mut coordinate_cells = 0usize;
    let mut scope_key_bytes = 0usize;
    for task in plan.tasks() {
        for cells in [
            task.key().sector().arity(),
            task.key().box_lower().len(),
            task.key().box_upper().len(),
            task.key().simplex_offset().len(),
            task.target_shift().len(),
        ] {
            coordinate_cells = checked_add(
                "retained task-key coordinate cells",
                coordinate_cells,
                cells,
            )?;
        }
        scope_key_bytes = checked_add(
            "retained stable-scope-key bytes",
            scope_key_bytes,
            task.key().stable_scope_key().len(),
        )?;
    }
    Ok((coordinate_cells, scope_key_bytes))
}

fn validate_plan(
    plan: &InteriorSimplexPlan,
    arity: usize,
    limits: InteriorSimplexExecutionLimits,
) -> Result<(), InteriorSimplexExecutionError> {
    check_limit("plan tasks", plan.tasks().len(), limits.max_tasks)?;
    check_limit(
        "retained task reports",
        plan.tasks().len(),
        limits.max_retained_task_reports,
    )?;
    for (canonical_ordinal, task) in plan.tasks().iter().enumerate() {
        plan.validate_task(task)
            .map_err(InteriorSimplexExecutionError::Plan)?;
        if task.canonical_ordinal() != canonical_ordinal {
            return Err(InteriorSimplexExecutionError::Invariant {
                detail: "plan task chronology is not its canonical ordinal order",
            });
        }
        for (object, actual) in [
            ("sector", task.key().sector().arity()),
            ("target shift", task.target_shift().len()),
            ("lattice target", task.lattice_target().len()),
        ] {
            if actual != arity {
                return Err(InteriorSimplexExecutionError::WrongTaskArity {
                    canonical_ordinal,
                    object,
                    expected: arity,
                    actual,
                });
            }
        }
        if task.key().interior_margin() != plan.interior_margin() {
            return Err(InteriorSimplexExecutionError::Invariant {
                detail: "task identity and plan disagree on the interior margin",
            });
        }
    }
    Ok(())
}
