//! Ordered shell for independent probe-local campaigns.

mod admission;
mod budget;
mod probe;

use crate::foundry::completion::stratum::{ImmutableOwnerSnapshot, MaximalStratumAnchor};
use crate::identity::{CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator};
use crate::sector::OrderingPolicy;

use super::super::{CampaignModularProbe, OrdinarySourceIncidenceIndex};
use super::{
    ProbeLocalBudgetScope, ProbeLocalSchedulerError, ProbeLocalSchedulerLimits,
    ProbeLocalSchedulerReport,
};
use admission::{admit_probes, validate_fixed_task};
use budget::{RunBudget, try_vec};
use probe::{run_single_probe, unexecuted_suffix_report};

const OUTCOMES: &str = "probe-local retained outcomes";

/// Ordered shell around independent serial single-probe obstruction runners.
#[derive(Debug)]
pub(crate) struct ProbeLocalObstructionScheduler<'inputs, 'family> {
    generator: &'inputs ParametricIbpGenerator<'family>,
    completed: &'inputs CompletedIbpSourceRows,
    target_shift: IntegralShift,
    stratum: MaximalStratumAnchor,
    owners: ImmutableOwnerSnapshot,
    ordering: OrderingPolicy,
    probes: Box<[CampaignModularProbe]>,
    limits: ProbeLocalSchedulerLimits,
}

impl<'inputs, 'family> ProbeLocalObstructionScheduler<'inputs, 'family> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_new(
        generator: &'inputs ParametricIbpGenerator<'family>,
        completed: &'inputs CompletedIbpSourceRows,
        target_shift: IntegralShift,
        stratum: MaximalStratumAnchor,
        owners: ImmutableOwnerSnapshot,
        ordering: OrderingPolicy,
        probes: impl IntoIterator<Item = CampaignModularProbe>,
        limits: ProbeLocalSchedulerLimits,
    ) -> Result<Self, ProbeLocalSchedulerError> {
        validate_fixed_task(
            generator,
            completed,
            &target_shift,
            &stratum,
            &owners,
            limits,
        )?;
        let probes = admit_probes(generator, &stratum, probes, limits)?;
        Ok(Self {
            generator,
            completed,
            target_shift,
            stratum,
            owners,
            ordering,
            probes: probes.into_boxed_slice(),
            limits,
        })
    }

    /// Execute every admitted probe independently in its declared order.
    pub(crate) fn run(self) -> Result<ProbeLocalSchedulerReport, ProbeLocalSchedulerError> {
        let zero = IntegralShift::try_new_with_component_limit(
            std::iter::repeat_n(0, self.target_shift.len()),
            self.limits.source_discovery.max_arity,
        )
        .map_err(ProbeLocalSchedulerError::Shift)?;
        let zero_sources = self
            .generator
            .translate_completed_source_rows(
                self.completed,
                [zero],
                self.limits.source_discovery.translation,
            )
            .map_err(ProbeLocalSchedulerError::SourceTranslation)?;
        let incidence =
            OrdinarySourceIncidenceIndex::try_new(&zero_sources, self.limits.source_discovery)
                .map_err(ProbeLocalSchedulerError::SourceModule)?;

        let probe_count = self.probes.len();
        let mut reports = try_vec(OUTCOMES, probe_count)?;
        let mut budget = RunBudget::default();
        let mut aggregate_stop: Option<(usize, &'static str)> = None;
        for (probe_ordinal, probe) in Vec::from(self.probes).into_iter().enumerate() {
            let report = if let Some((triggering_probe_ordinal, resource)) = aggregate_stop {
                unexecuted_suffix_report(probe_ordinal, probe, triggering_probe_ordinal, resource)
            } else {
                let report = run_single_probe(
                    probe_ordinal,
                    probe,
                    self.generator,
                    self.completed,
                    &incidence,
                    &self.target_shift,
                    &self.stratum,
                    &self.owners,
                    self.ordering,
                    self.limits,
                    &mut budget,
                )?;
                if let Some(stop) = report.outcome().budget_stop()
                    && stop.cause().scope() == ProbeLocalBudgetScope::Aggregate
                {
                    aggregate_stop = Some((probe_ordinal, stop.cause().resource()));
                }
                report
            };
            reports.push(report);
        }
        if reports.len() != probe_count {
            return Err(ProbeLocalSchedulerError::Invariant {
                detail: "ordered execution did not retain exactly one outcome per probe",
            });
        }
        Ok(ProbeLocalSchedulerReport::new(reports, budget.census()))
    }
}
