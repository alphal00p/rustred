//! Worker-local conservative routing; only the coordinator admits domains.
use super::{
    OwnerDomainWalkRequest,
    initial_orthants::InitialOrthants,
    inspection::{Effect, Event, Finished, NativeStats, debug},
    mask,
    queue::{Domain, Phase},
};
use rustred::solver::{
    CandidateDomainRouteEvent, CandidateDomainRouteLimits, RoutedCandidateReducer,
};
use serde_json::{Value, json};
use std::ops::ControlFlow;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

fn reentry<const N: usize>(
    sector: [bool; N],
    lower: &[u64],
    upper: &[Option<u64>],
    rank: Option<u32>,
    conditions: bool,
) -> Result<Domain<N>, Value> {
    if conditions {
        Err(json!({"kind":"route_reentry_source_validity_obligation",
            "owner":mask(&sector), "lower":lower, "upper":upper,
            "rank":rank, "reached_missing_rule_claim":false}))
    } else {
        Ok(Domain {
            phase: Phase::Route,
            owner: sector,
            lower: lower.to_vec(),
            upper: upper.to_vec(),
            rank,
        })
    }
}

pub(super) fn inspect<const N: usize>(
    reducer: &RoutedCandidateReducer<N>,
    domain: &Domain<N>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    initial: &InitialOrthants<N>,
    emit: &mut (impl FnMut(Event<N>) -> ControlFlow<()> + ?Sized),
) -> Finished {
    let started = Instant::now();
    let conditions = reducer.domain_routing_requires_source_conditions();
    let result = reducer.visit_bounded_domain_route_overcover(
        domain.owner, &domain.lower, &domain.upper, domain.rank,
        CandidateDomainRouteLimits { max_masks: request.max_route_masks,
            max_coordinate_cells: request.max_route_masks.saturating_mul(N).saturating_mul(2) },
        cancellation, |event| {
            let effect = match event {
                CandidateDomainRouteEvent::Apply { owner_sector, cover } if initial.contains(Phase::Apply, &owner_sector, cover.actual_rank) =>
                    Effect::PreAdmittedOrthantReuse { successor: false, conditional: false },
                CandidateDomainRouteEvent::Apply { owner_sector, cover } => Effect::Admit {
                    successor: false, conditional: false, domain: Domain { phase: Phase::Apply,
                    owner: owner_sector, lower: cover.lower.to_vec(), upper: cover.upper.to_vec(), rank: cover.actual_rank } },
                // Source validity remains mandatory before considering reuse.
                CandidateDomainRouteEvent::Route { sector, cover } if !conditions && initial.contains(Phase::Route, &sector, cover.actual_rank) =>
                    Effect::PreAdmittedOrthantReuse { successor: false, conditional: false },
                CandidateDomainRouteEvent::Route { sector, cover } => match reentry(sector, &cover.lower, &cover.upper, cover.actual_rank, conditions) {
                    Ok(domain) => Effect::Admit { domain, successor: false, conditional: false },
                    Err(value) => Effect::Frontier { value, successor: false, conditional: false },
                },
                CandidateDomainRouteEvent::MissingRoute { source_sector, actual_rank } => Effect::Frontier {
                    successor: false, conditional: false, value: json!({"kind":"missing_route_cover",
                        "owner":mask(&source_sector), "lower":domain.lower, "upper":domain.upper,
                        "rank":actual_rank, "reached_missing_rule_claim":false}) },
                CandidateDomainRouteEvent::ZeroSector { sector, actual_rank, source_conditions_required } => {
                    if source_conditions_required { Effect::Frontier { successor: false, conditional: false,
                        value: json!({"kind":"zero_source_validity_obligation", "owner":mask(&sector),
                            "lower":domain.lower, "upper":domain.upper,
                            "rank":actual_rank, "reached_missing_rule_claim":false}) }
                    } else { Effect::Count }
                },
            };
            emit(Event::one(effect))
        });
    let (stats, error, error_kind) = match result {
        Ok(s) => (s, None, "none"),
        Err(e) => {
            use rustred::solver::CandidateDomainRouteFailure as F;
            let kind = match &e.failure {
                F::Cancelled => "cancelled",
                F::StoppedByConsumer => "consumer_stop",
                _ => "native_failure",
            };
            (e.stats, Some(debug(&e.failure)), kind)
        }
    };
    Finished {
        stats: NativeStats::Route(stats),
        error,
        error_kind,
        seconds: started.elapsed().as_secs_f64(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn route_generated_reentry_keeps_unchecked_source_validity_and_rank() {
        for rank in [Some(11), None] {
            let obligation =
                reentry([true, false], &[0, 0], &[None, None], rank, true).unwrap_err();
            assert_eq!(
                obligation["kind"],
                "route_reentry_source_validity_obligation"
            );
            assert_eq!(obligation["rank"], json!(rank));
            assert_eq!(obligation["reached_missing_rule_claim"], false);
            let admitted = reentry([true, false], &[0, 0], &[None, None], rank, false).unwrap();
            assert_eq!(admitted.phase, Phase::Route);
            assert_eq!(admitted.rank, rank);
            assert_eq!(admitted.upper, vec![None, None]);
        }
    }

    #[test]
    fn bounded_route_reentry_preserves_box_and_source_obligation() {
        let lower = [0, 2, 0];
        let upper = [Some(7), Some(4), None];
        for rank in [Some(11), None] {
            let obligation = reentry([true, false, true], &lower, &upper, rank, true).unwrap_err();
            assert_eq!(obligation["lower"], json!(lower));
            assert_eq!(obligation["upper"], json!(upper));
            assert_eq!(obligation["rank"], json!(rank));
            assert_eq!(obligation["reached_missing_rule_claim"], false);
            let admitted = reentry([true, false, true], &lower, &upper, rank, false).unwrap();
            assert_eq!(admitted.phase, Phase::Route);
            assert_eq!(admitted.lower, lower);
            assert_eq!(admitted.upper, upper);
            assert_eq!(admitted.rank, rank);
        }
    }
}
