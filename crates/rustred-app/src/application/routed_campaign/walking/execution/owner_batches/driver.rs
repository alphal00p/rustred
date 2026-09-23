//! Bounded owner-local FIFO publication with independently reserved inspectors.
use super::*;
use selection::choose;

mod jobs;
mod leftovers;
mod selection;

#[cfg(test)]
mod tests;

struct Job<const N: usize> {
    key: Key<N>,
    local_id: usize,
    ticket: usize,
    domain: Arc<Domain<N>>,
}

fn started<const N: usize>(
    walk: &Walk<N>,
    request: &OwnerDomainWalkRequest,
    observer: &impl Fn(Value),
    job: &Job<N>,
    parallel: Value,
) {
    let mut event = report::progress(walk, request, parallel, Some(job.key));
    event["event"] = json!("domain_started");
    event["id"] = json!(job.local_id);
    event["bucket"] = json!(key_name(&job.key));
    observer(event);
}

fn publish<const N: usize>(walk: &mut Walk<N>, key: Key<N>, id: usize, finished: Finished) {
    // Native Debug text is not the failure classification (for example,
    // `Matching(Cancelled)`). Preserve its detail while exposing the typed
    // cancellation cause, unless an earlier delivery failure already won.
    if finished.error_kind == "cancelled" && walk.error.is_none() {
        walk.error = Some(finished.error.as_ref().map_or_else(
            || "cancelled".to_owned(),
            |detail| format!("cancelled: {detail}"),
        ));
    }
    let bucket = walk.buckets.get_mut(&key).expect("source bucket");
    bucket.native_seconds += finished.seconds;
    if let Some(error) = &walk.error {
        bucket.state.error.get_or_insert_with(|| error.clone());
    }
    bucket.state.commit(id, finished);
    if let Some(error) = &bucket.state.error {
        walk.error.get_or_insert_with(|| error.clone());
    }
}

fn inline<const N: usize>(
    walk: &mut Walk<N>,
    reducer: &RoutedCandidateReducer<N>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
    initial: &InitialOrthants<N>,
    overlaps: &BTreeMap<Key<N>, InitialOverlapIndex<N>>,
) -> Value {
    let empty = InitialOverlapIndex::empty();
    let mut heartbeat = Instant::now();
    while walk.error.is_none() {
        let jobs = match choose(walk, 1, &[], cancellation) {
            Ok(jobs) => jobs,
            Err(error) => {
                walk.error = Some(error);
                break;
            }
        };
        let Some(job) = jobs.into_iter().next() else {
            break;
        };
        if let Err(error) = walk
            .buckets
            .get_mut(&job.key)
            .expect("source bucket")
            .state
            .note_native_started(job.local_id)
        {
            walk.error = Some(error);
            break;
        }
        walk.buckets
            .get_mut(&job.key)
            .expect("inline bucket")
            .peak_outstanding_native_jobs = 1;
        walk.buckets
            .get_mut(&job.key)
            .expect("inline bucket")
            .peak_occupied_native_slots = 1;
        let Some(rounds) = walk.metrics.rounds.checked_add(1) else {
            walk.error = Some("round counter overflow".into());
            break;
        };
        walk.metrics.rounds = rounds;
        started(
            walk,
            request,
            observer,
            &job,
            json!({"workers":1,"active_workers":1}),
        );
        // Observers may request cancellation at the start notification. Match
        // the parallel dispatch path: leave the responsibility pending rather
        // than entering a native visitor after the cancellation was observed.
        if cancellation.load(Ordering::Acquire) {
            walk.error = Some("cancelled".into());
            break;
        }
        let result = inspection::inspect(
            reducer,
            &job.domain,
            request,
            cancellation,
            initial,
            overlaps.get(&job.key).unwrap_or(&empty),
            &mut |event| {
                let Some(rounds) = walk.metrics.chunk_rounds.checked_add(1) else {
                    walk.error = Some("chunk round counter overflow".into());
                    return ControlFlow::Break(());
                };
                walk.metrics.chunk_rounds = rounds;
                if let Err(error) = delivery::deliver(
                    walk,
                    vec![(job.key, vec![event])],
                    request,
                    cancellation,
                    None,
                ) {
                    walk.error = Some(error);
                    return ControlFlow::Break(());
                }
                if heartbeat.elapsed() >= Duration::from_millis(250) {
                    observer(report::progress(
                        walk,
                        request,
                        json!({"workers":1,"active_workers":1}),
                        Some(job.key),
                    ));
                    heartbeat = Instant::now();
                }
                ControlFlow::Continue(())
            },
        );
        publish(walk, job.key, job.local_id, result);
        observer(report::progress(
            walk,
            request,
            json!({"workers":1,"active_workers":0}),
            Some(job.key),
        ));
    }
    json!({"workers":1,"active_workers":0,"inspection_worker_limit":1,
        "admission_worker_limit":0,"coordinator_worker_limit":0,"total_compute_worker_limit":1})
}

pub(super) fn run<const N: usize>(
    walk: &mut Walk<N>,
    reducer: &RoutedCandidateReducer<N>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
) -> Value {
    // Move the immutable lookup fields out to permit independent mutable owner
    // queues while every inspector borrows exactly the same initial snapshot.
    let initial = std::mem::replace(&mut walk.initial, InitialOrthants::empty());
    let overlaps = std::mem::take(&mut walk.overlaps);
    if request.workers == 1 {
        return inline(
            walk,
            reducer,
            request,
            cancellation,
            observer,
            &initial,
            &overlaps,
        );
    }
    let empty = InitialOverlapIndex::empty();
    run_parallel(
        walk,
        request,
        cancellation,
        observer,
        |domain, stop, emit| {
            inspection::inspect(
                reducer,
                domain,
                request,
                stop,
                &initial,
                overlaps
                    .get(&(domain.phase, domain.owner))
                    .unwrap_or(&empty),
                emit,
            )
        },
    )
}

/// The production coordinator also accepts controlled native streams in tests;
/// no alternate test-only publication or admission algorithm is involved.
fn run_parallel<const N: usize>(
    walk: &mut Walk<N>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
    inspect: impl Fn(&Domain<N>, &AtomicBool, &mut dyn FnMut(Event<N>) -> ControlFlow<()>) -> Finished
    + Sync,
) -> Value {
    let budget = super::super::super::worker_budget::WorkerBudget::for_request(request);
    let helpers = budget.helpers;
    let inspectors = budget.inspection;
    let admission_pool = if helpers == 0 {
        None
    } else {
        match rayon::ThreadPoolBuilder::new()
            .num_threads(helpers)
            .thread_name(|id| format!("owner-batch-admit-{id}"))
            .build()
        {
            Ok(pool) => Some(pool),
            Err(error) => {
                walk.error = Some(format!("owner admission worker spawn: {error}"));
                return json!({"workers":0});
            }
        }
    };
    let mut jobs = jobs::Jobs::new();
    let mut heartbeat = Instant::now();
    let (_, mut snapshot, leftovers) = parallel::with_pool(inspectors, inspect, |pool| {
        'rounds: while walk.error.is_none() {
            let reclaimed = pool.reclaim_completed_except(&jobs.protected());
            jobs.release_slots(&reclaimed);
            let incoming = match choose(
                walk,
                inspectors - jobs.active.len(),
                &jobs.active,
                cancellation,
            ) {
                Ok(jobs) => jobs,
                Err(error) => {
                    walk.error = Some(error);
                    break;
                }
            };
            if !incoming.is_empty() {
                let Some(rounds) = walk.metrics.rounds.checked_add(1) else {
                    walk.error = Some("round counter overflow".into());
                    break;
                };
                walk.metrics.rounds = rounds;
            }
            for job in incoming {
                if let Err(error) = walk
                    .buckets
                    .get_mut(&job.key)
                    .expect("source bucket")
                    .state
                    .note_native_started(job.local_id)
                {
                    walk.error = Some(error);
                    break 'rounds;
                }
                jobs.register(&job);
                started(walk, request, observer, &job, pool.snapshot());
                if cancellation.load(Ordering::Acquire) {
                    walk.error = Some("cancelled".into());
                    break 'rounds;
                }
                if !pool.dispatch(job.ticket, job.domain.clone()) {
                    walk.error = Some("owner-batch dispatch refused".into());
                    break 'rounds;
                }
                let bucket = walk.buckets.get_mut(&job.key).expect("dispatched bucket");
                bucket.outstanding_native_jobs += 1;
                bucket.peak_outstanding_native_jobs = bucket
                    .peak_outstanding_native_jobs
                    .max(bucket.outstanding_native_jobs);
                let occupied = 1 + jobs
                    .active
                    .iter()
                    .filter(|active| active.key == job.key)
                    .count();
                bucket.peak_occupied_native_slots = bucket.peak_occupied_native_slots.max(occupied);
                jobs.active.push(job);
            }
            if jobs.pending.is_empty() {
                break;
            }
            let heads = jobs.eligible(walk);
            let held = jobs.pending.len() - heads.len();
            walk.metrics.peak_fifo_held_jobs = walk.metrics.peak_fifo_held_jobs.max(held);
            let head_tickets = heads
                .iter()
                .map(|head| head.ticket)
                .collect::<std::collections::BTreeSet<_>>();
            let mut chunks = Vec::new();
            let mut finished = Vec::new();
            // Poll only the FIFO head of each owner. Its State owns the source
            // details/refusals; later results remain in bounded physical slots
            // or shared completed-result storage, never a second publisher.
            for job in jobs.poll_batch(heads, inspectors) {
                if cancellation.load(Ordering::Acquire) {
                    walk.error = Some("cancelled".into());
                    break;
                }
                if let Some(failure) = pool.failure() {
                    walk.error = Some(failure.detail);
                    break;
                }
                match pool.poll(job.ticket) {
                    Poll::Events(events) => chunks.push((job.key, events)),
                    Poll::Finished(value) => {
                        finished.push((job, value));
                    }
                    Poll::Waiting => {}
                }
            }
            if walk.error.is_none() && chunks.is_empty() && finished.is_empty() {
                // Recheck all selected slots under the pool mutex before
                // sleeping: a chunk arriving after the scan must not lose
                // its notification. Cancellation is observed after at
                // most the bounded wait even without a producer signal.
                // Later unfinished chunks cannot wake publication. A later
                // successful completion may wake bounded slot reclamation.
                if head_tickets.is_empty() {
                    walk.error = Some("owner publication has no eligible FIFO head".into());
                    break 'rounds;
                }
                let start = Instant::now();
                pool.wait_for_owner_progress(&head_tickets);
                walk.metrics.idle_stream_wait_seconds += start.elapsed().as_secs_f64();
                if heartbeat.elapsed() >= Duration::from_millis(250) {
                    observer(report::progress(walk, request, pool.snapshot(), None));
                    heartbeat = Instant::now();
                }
                continue;
            }
            if walk.error.is_none() && !chunks.is_empty() {
                let Some(rounds) = walk.metrics.chunk_rounds.checked_add(1) else {
                    walk.error = Some("chunk round counter overflow".into());
                    break 'rounds;
                };
                walk.metrics.chunk_rounds = rounds;
                if let Err(error) =
                    delivery::deliver(walk, chunks, request, cancellation, admission_pool.as_ref())
                {
                    walk.error = Some(error);
                }
            }
            // Every earlier source chunk must be accepted before this
            // successful publication. On an outer delivery failure the
            // existing State::commit records failure, never discharge.
            for (head, value) in finished {
                publish(walk, head.key, head.local_id, value);
                walk.buckets
                    .get_mut(&head.key)
                    .expect("published bucket")
                    .outstanding_native_jobs -= 1;
                jobs.published(head);
            }
            if walk.error.is_some() {
                break 'rounds;
            }
            // Refill on the next immediate pass, without requiring an
            // event or completion from any still-running peer.
            if heartbeat.elapsed() >= Duration::from_millis(250) {
                observer(report::progress(walk, request, pool.snapshot(), None));
                heartbeat = Instant::now();
            }
        }
        if let Some(error) = &walk.error {
            pool.fail(Failure {
                id: None,
                phase: None,
                kind: "owner_batch_publication",
                detail: error.clone(),
            });
            while !pool.wait_drained() {
                observer(report::progress(walk, request, pool.snapshot(), None));
            }
        }
    });
    if walk.error.is_none() {
        if let Some(detail) = snapshot["first_failure"]["detail"].as_str() {
            walk.error = Some(detail.to_owned());
        }
    }
    leftovers::retain(walk, jobs.pending, leftovers);
    snapshot["inspection_worker_limit"] = json!(inspectors);
    snapshot["admission_worker_limit"] = json!(helpers);
    snapshot["coordinator_worker_limit"] = json!(1);
    snapshot["total_compute_worker_limit"] = json!(request.workers);
    snapshot["native_pool_ids_are_opaque_tickets"] = json!(true);
    snapshot
}
