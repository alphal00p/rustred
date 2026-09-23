//! Failure cleanup never advances past the original owner FIFO head.
use super::*;

pub(super) fn retain<const N: usize>(
    walk: &mut Walk<N>,
    mut pending: BTreeMap<usize, (Key<N>, usize)>,
    leftovers: Vec<(usize, Finished)>,
) {
    // A failure can leave several finished inspectors for one owner. Only the
    // original, not-yet-failed FIFO head may be recorded through State::commit.
    // That method advances the cursor even on failure, so never call it for a
    // later job or after a ledger halt. In particular, do not publish aliases
    // during cleanup to bridge a missing head.
    let mut cleanup_heads = walk
        .buckets
        .iter()
        .map(|(key, bucket)| {
            (
                *key,
                (bucket.state.queue.next, bucket.state.error.is_none()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    if !pending.is_empty() {
        walk.error
            .get_or_insert_with(|| "native completion was not published".into());
    }
    for (ticket, finished) in leftovers {
        if let Some((key, id)) = pending.remove(&ticket) {
            let (head, may_publish) = cleanup_heads.get_mut(&key).expect("cleanup bucket");
            if *head == id && *may_publish {
                *may_publish = false;
                publish(walk, key, id, finished);
                walk.buckets
                    .get_mut(&key)
                    .expect("leftover bucket")
                    .outstanding_native_jobs -= 1;
            } else {
                retain_uncommitted(walk, key, id, ticket, *head == id, Some(finished));
            }
        }
    }
    for (ticket, (key, id)) in pending {
        walk.error
            .get_or_insert_with(|| "native completion unavailable".into());
        retain_uncommitted(walk, key, id, ticket, cleanup_heads[&key].0 == id, None);
    }
}

fn retain_uncommitted<const N: usize>(
    walk: &mut Walk<N>,
    key: Key<N>,
    id: usize,
    ticket: usize,
    head: bool,
    finished: Option<Finished>,
) {
    let bucket = walk.buckets.get_mut(&key).expect("pending source");
    let state = &mut bucket.state;
    state
        .error
        .get_or_insert_with(|| walk.error.clone().expect("outer failure"));
    let domain = &state.queue.domains[id];
    let mut record = json!({"id":id,"native_ticket":ticket,"stats":null,
        "phase":format!("{:?}", domain.phase),"owner":mask(&domain.owner),
        "lower":domain.lower,"upper":domain.upper,"rank":domain.rank,
        "power_bounds":power_bounds_json(domain.powers),
        "committed":false,"partial_native_statistics_unavailable":finished.is_none(),
        "source_stream_fully_admitted":false,"error":walk.error,
        "frontiers":if head { std::mem::take(&mut state.details) } else { Vec::new() }});
    if head {
        record["optional_refusals"] = json!(std::mem::take(&mut state.refusals).records);
    }
    if let Some(finished) = finished {
        if let Some(scope) = finished.initial_overlap_scope() {
            record["native_inspection_scope"] = json!("low_D_residual_only");
            record["initial_overlap"] = json!({"anchor_id":scope.anchor_id,"cut":scope.cut,
                "covered_slice":"original_intersect_D_ge_cut",
                "residual_power_bounds":power_bounds_json(scope.residual_powers),
                "coordinates_and_rank_unchanged":true,"responsibility_published":false});
        }
        bucket.native_seconds += finished.seconds;
        record["stats"] = native_stats(finished.stats);
        record["seconds"] = json!(finished.seconds);
        record["native_error"] = json!(finished.error);
        record["native_error_kind"] = json!(finished.error_kind);
    }
    state.uncommitted.push(record);
}
