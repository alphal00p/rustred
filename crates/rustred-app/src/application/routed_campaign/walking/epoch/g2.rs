//! Epoch authority adapter for the existing indexed exact-union planner.
//! Plans remain job-local: unpublished results are discarded on interruption.
use super::super::{
    g2 as planner,
    initial_orthants::InitialOrthants,
    inspection,
    queue::{CompactDomain, Phase},
};
use super::anchors::{AnchorKind, AnchorMap, AnchorScope, Lent, Piece};
use super::job::{G2Part, Job};
use super::ledger6::{Entry6, Ledger6};
use super::state::EpochState;
use std::ops::ControlFlow;
use std::sync::Arc;

/// Local inspected authority, shared by publication, P1 and restore. Descendant
/// obligations remain explicit historical edges; they are not closure claims.
pub(super) fn eligible<const N: usize>(
    domains: &[CompactDomain<N>],
    ledger: &Ledger6,
    anchors: &AnchorMap,
    id: u32,
) -> bool {
    domains
        .get(id as usize)
        .is_some_and(|d| d.phase() == Phase::Apply)
        && matches!(ledger.get(id), Ok(Entry6::Native { .. }))
        && !anchors.get(id).is_some_and(
            |record| matches!(&record.scope, AnchorScope::Residual(pieces) if pieces.is_empty()),
        )
}

pub(super) fn publish<const N: usize>(state: &mut EpochState<N>, id: u32) {
    if state.store.is_quarantined(id)
        || !eligible(&state.store.domains, &state.ledger, &state.anchors, id)
    {
        return;
    }
    let Ok(Entry6::Native { epoch, .. }) = state.ledger.get(id) else {
        return;
    };
    let image = &state.store.domains[id as usize];
    let mut scope = image.expand();
    let (lent, kind) = match state.anchors.get(id) {
        Some(record) if record.kind == AnchorKind::InitialDBand => {
            let (_, cut) = record.d_band().expect("validated initial anchor");
            let Some(hi) = cut.checked_sub(1) else {
                return;
            };
            scope.powers.max_power_difference =
                Some(scope.powers.max_power_difference.map_or(hi, |v| v.min(hi)));
            (Lent::LowSlice, planner::kind::INITIAL_D_BAND)
        }
        Some(_) => (Lent::Full, planner::kind::G2_RESIDUAL),
        None => (Lent::Full, planner::kind::NATIVE),
    };
    let bucket = state.store.bucket_of[&super::store::bucket_key(image)];
    state.merged_view.push(bucket, id, epoch, lent);
    if let Some(index) = &state.g2_store {
        index.append(
            &scope.owner,
            planner::Store::entry(&scope, id as usize, epoch, kind),
        );
    }
}

/// Rebuild only after complete restore validation, or completed fresh admission.
pub(super) fn enable<const N: usize>(state: &mut EpochState<N>) {
    state.g2_store = Some(Arc::new(planner::Store::new(
        None,
        state.p0 as usize,
        state.k + 1,
    )));
    state.merged_view = Default::default();
    for id in 0..state.watermark() {
        publish(state, id);
    }
}

pub(super) fn report<const N: usize>(state: &EpochState<N>) -> Option<serde_json::Value> {
    let index = state.g2_store.as_ref()?;
    let mut value = index.report();
    value["snapshot_rule"] =
        serde_json::json!("epoch: lender merge_epoch <= job dispatch_version < job merge_epoch");
    value["logged_g2_records"] = serde_json::json!(state.counters.g2_records);
    value["planner_telemetry"] = index.telemetry();
    Some(value)
}

pub(super) fn inspect<const N: usize>(
    context: &super::inspector::Context<'_, N>,
    job: &Job<N>,
    emit: &mut impl FnMut(inspection::Event<N>) -> ControlFlow<()>,
) -> (inspection::Finished, Option<G2Part>) {
    let domain = job.image.expand();
    let initial = InitialOrthants::empty();
    let ordinary = |emit: &mut dyn FnMut(inspection::Event<N>) -> ControlFlow<()>| {
        inspection::inspect(
            context.reducer,
            &domain,
            context.request,
            context.cancellation,
            &initial,
            context.overlap,
            emit,
        )
    };
    let Some(index) = context
        .g2
        .filter(|index| index.eligible(job.parent as usize, &domain))
    else {
        return (ordinary(emit), None);
    };
    if let Some(finished) = inspection::inspect_initial_overlap(
        context.reducer,
        &domain,
        context.request,
        context.cancellation,
        &initial,
        context.overlap,
        emit,
    ) {
        return (finished, None);
    }
    let started = std::time::Instant::now();
    let Some(exclusive) = job.v0.checked_add(1) else {
        return (ordinary(emit), None);
    };
    let planner::Outcome::Planned(plan) = index.plan_at(exclusive, &domain, context.cancellation)
    else {
        let mut finished = inspection::inspect_options(
            context.reducer,
            &domain,
            context.request,
            context.cancellation,
            &initial,
            true,
            emit,
        );
        finished.seconds = started.elapsed().as_secs_f64();
        return (finished, None);
    };
    let mut finished = match plan.residual_domain(&domain) {
        Some(residual) => inspection::inspect_options(
            context.reducer,
            &residual,
            context.request,
            context.cancellation,
            &initial,
            true,
            emit,
        ),
        None => inspection::Finished {
            stats: inspection::NativeStats::Apply(Default::default()),
            error: None,
            error_kind: "none",
            seconds: 0.0,
        },
    };
    finished.seconds = started.elapsed().as_secs_f64();
    let (lower, upper) = job.image.raw_bounds();
    let pieces = plan
        .residual
        .into_iter()
        .map(|(lo, hi)| Piece {
            d_lo: Some(lo),
            d_hi: Some(hi),
            lower: lower.to_vec(),
            upper: upper.to_vec(),
        })
        .collect();
    let part = G2Part {
        kind: AnchorKind::G2Residual as u8,
        anchors: plan
            .anchors
            .into_iter()
            .map(|a| {
                (
                    a.id,
                    a.stamp,
                    if a.kind == planner::kind::INITIAL_D_BAND {
                        Lent::LowSlice
                    } else {
                        Lent::Full
                    } as u8,
                )
            })
            .collect(),
        pieces,
    };
    (finished, Some(part))
}
