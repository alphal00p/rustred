use crate::foundry::completion::source_discovery::OrdinarySourceIncidenceIndex;
use crate::foundry::completion::stratum::{DecoratedStratum, MaximalStratumAnchor};
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::SectorMonotoneDomain;

use super::super::InteriorSimplexTask;
use super::InteriorSimplexExecutionError;
use super::limits::InteriorSimplexExecutionLimits;
use super::model::InteriorSimplexBootstrapTelemetry;
use super::resource::{check_limit, checked_add, checked_mul, try_reserve_exact};

#[derive(Default)]
pub(super) struct BootstrapAggregateBudget {
    requests: usize,
    physical_shifts: usize,
    physical_shift_coordinate_cells: usize,
}

pub(super) fn try_derive_bootstrap_stratum(
    task: &InteriorSimplexTask,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    incidence: &OrdinarySourceIncidenceIndex<'_>,
    limits: InteriorSimplexExecutionLimits,
    aggregate: &mut BootstrapAggregateBudget,
) -> Result<(InteriorSimplexBootstrapTelemetry, MaximalStratumAnchor), InteriorSimplexExecutionError>
{
    let nominations = incidence
        .try_nominate_target_unit(task.target_shift(), limits.scheduler.source_discovery)
        .map_err(InteriorSimplexExecutionError::SourceDiscovery)?;
    let next_requests = checked_add(
        "aggregate bootstrap requests",
        aggregate.requests,
        nominations.requests().len(),
    )?;
    check_limit(
        "aggregate bootstrap requests",
        next_requests,
        limits.max_aggregate_bootstrap_requests,
    )?;

    let selected = generator
        .translate_selected_completed_source_rows(
            completed,
            nominations.requests().iter().cloned(),
            limits.scheduler.campaign.translated_sources,
        )
        .map_err(InteriorSimplexExecutionError::SourceTranslation)?;
    if selected.requests() != nominations.requests() {
        return Err(InteriorSimplexExecutionError::Invariant {
            detail: "bootstrap translation changed the canonical nominated request set",
        });
    }

    let mut physical_shift_occurrences = 0usize;
    for source in selected.sources() {
        physical_shift_occurrences = checked_add(
            "bootstrap physical shift occurrences",
            physical_shift_occurrences,
            source.terms().len(),
        )?;
    }
    check_limit(
        "bootstrap physical shifts per task",
        physical_shift_occurrences,
        limits.max_bootstrap_physical_shifts_per_task,
    )?;
    let next_physical_shifts = checked_add(
        "aggregate bootstrap physical shifts",
        aggregate.physical_shifts,
        physical_shift_occurrences,
    )?;
    check_limit(
        "aggregate bootstrap physical shifts",
        next_physical_shifts,
        limits.max_aggregate_bootstrap_physical_shifts,
    )?;
    let task_shift_cells = checked_mul(
        "aggregate bootstrap physical-shift coordinate cells",
        physical_shift_occurrences,
        task.target_shift().len(),
    )?;
    let next_shift_cells = checked_add(
        "aggregate bootstrap physical-shift coordinate cells",
        aggregate.physical_shift_coordinate_cells,
        task_shift_cells,
    )?;
    check_limit(
        "aggregate bootstrap physical-shift coordinate cells",
        next_shift_cells,
        limits.max_aggregate_bootstrap_shift_coordinate_cells,
    )?;

    let mut physical_shifts: Vec<&[i64]> = Vec::new();
    try_reserve_exact(
        &mut physical_shifts,
        physical_shift_occurrences,
        "bootstrap physical shift views",
    )?;
    physical_shifts.extend(
        selected
            .sources()
            .iter()
            .flat_map(|source| source.terms().keys())
            .map(|shift| shift.values()),
    );
    physical_shifts.sort_unstable();
    physical_shifts.dedup();
    let distinct_physical_shifts = physical_shifts.len();

    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        task.key().sector().clone(),
        task.target_shift().values(),
        &physical_shifts,
    )
    .map_err(InteriorSimplexExecutionError::Sector)?;
    let stratum = DecoratedStratum::try_guard_blind(
        selected.family_fingerprint(),
        selected.context_fingerprint(),
        domain,
        limits.scheduler.campaign.stratum,
    )
    .map_err(InteriorSimplexExecutionError::Stratum)?;
    let anchor = MaximalStratumAnchor::try_new(stratum, limits.scheduler.campaign.stratum)
        .map_err(InteriorSimplexExecutionError::Stratum)?;

    aggregate.requests = next_requests;
    aggregate.physical_shifts = next_physical_shifts;
    aggregate.physical_shift_coordinate_cells = next_shift_cells;
    Ok((
        InteriorSimplexBootstrapTelemetry::new(
            nominations.raw_incidence_visits(),
            nominations.unique_before_existing_exclusion(),
            nominations.excluded_existing_requests(),
            selected.len(),
            physical_shift_occurrences,
            distinct_physical_shifts,
        ),
        anchor,
    ))
}
