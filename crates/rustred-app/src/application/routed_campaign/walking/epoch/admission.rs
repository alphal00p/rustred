//! Shared one-row initial handling; no dispatcher, checkpoint or CAS policy.
use super::super::{
    power_bounds_json,
    queue::{Domain, Phase},
};
use super::{AdmissionError, EpochState, admit_initial_with, input_row, matching};
use rustred::solver::RoutedCandidateReducer;
use serde_json::{Value, json};

pub(super) fn phase(installed: bool, overcover: bool, source_conditions: bool) -> Option<Phase> {
    if installed || !overcover {
        Some(Phase::Apply)
    } else if !source_conditions {
        Some(Phase::Route)
    } else {
        None
    }
}

pub(super) fn query_phase<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    overcover: bool,
    query: &matching::input::Query,
) -> Option<Phase> {
    phase(
        reducer.programs().owner_sectors().any(|owner| {
            owner.as_slice()
                == rustred::storage_array::<_, N>(&query.owner, false)
                    .as_ref()
                    .map(|owner| owner.as_slice())
                    .unwrap_or(&[])
        }),
        overcover,
        reducer.domain_routing_requires_source_conditions(),
    )
}

pub(super) fn source_frontier<const N: usize>(query: &matching::input::Query) -> Value {
    json!({"id":query.id,"kind":"initial_route_source_validity_obligation",
        "owner":query.owner.iter().map(|&active| if active {'1'} else {'0'}).collect::<String>(),
        "lower":query.lower,"upper":query.upper,"rank":query.rank,
        "power_bounds":power_bounds_json(query.powers),"reached_missing_rule_claim":false})
}

pub(super) fn one<const N: usize>(
    state: &mut EpochState<N>,
    query: &matching::input::Query,
    phase: Option<Phase>,
    rows: &mut Vec<Value>,
    frontiers: &mut Vec<Value>,
) -> Result<(), AdmissionError> {
    one_with(state, query, phase, rows, frontiers, || Ok(()))
}

/// Existing reservation fault seam; ordinary callers supply an inlined no-op.
pub(super) fn one_with<const N: usize>(
    state: &mut EpochState<N>,
    query: &matching::input::Query,
    phase: Option<Phase>,
    rows: &mut Vec<Value>,
    frontiers: &mut Vec<Value>,
    mut checkpoint: impl FnMut() -> Result<(), &'static str>,
) -> Result<(), AdmissionError> {
    use AdmissionError as E;
    if state.poisoned {
        return Err(E::Internal("initial admission: state is poisoned".into()));
    }
    if !rustred::fits_storage(query.owner.len(), N)
        || query.lower.len() != query.owner.len()
        || query.upper.len() != query.owner.len()
    {
        return Err(E::Refused("input: domain coordinate arity".into()));
    }
    if phase.is_none() && frontiers.len() as u64 >= state.max_frontiers {
        return Err(E::FrontierCap);
    }
    checkpoint().map_err(E::index)?;
    rows.try_reserve(1)
        .map_err(|_| E::Alloc("initial query row allocation".into()))?;
    // Own all JSON/key/string allocations before an ID or frontier is visible.
    let mut row = input_row(query, None);
    if let Some(phase) = phase {
        let domain = Domain {
            phase,
            owner: rustred::storage_array(&query.owner, false).expect("arity checked"),
            lower: rustred::storage_array::<_, N>(&query.lower, 0)
                .expect("validated query arity")
                .to_vec(),
            upper: rustred::storage_array::<_, N>(&query.upper, Some(0))
                .expect("validated query arity")
                .to_vec(),
            rank: query.rank,
            powers: query.powers,
        };
        let id = admit_initial_with(state, &domain, checkpoint)?;
        *row.get_mut("domain").expect("preowned domain field") = id.into();
    } else {
        checkpoint().map_err(E::index)?;
        frontiers
            .try_reserve(1)
            .map_err(|_| E::Alloc("initial input frontier allocation".into()))?;
        row["source_validity_unresolved"] = true.into();
        let frontier = source_frontier::<N>(query);
        checkpoint().map_err(E::index)?;
        frontiers.push(frontier);
    }
    rows.push(row);
    Ok(())
}

#[cfg(test)]
mod tests;
