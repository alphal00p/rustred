//! Bounded owner-local FIFO publication with independently reserved inspectors.
use super::*;
use selection::choose;

mod leftovers;
mod selection;

#[cfg(test)]
mod tests;

struct Job<const N: usize> {
    key: Key<N>,
    local_id: usize,
    ticket: usize,
    domain: Arc<Domain<N>>,
    finished: bool,
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
    let helpers = if request.workers >= 4 && request.max_containment_checks.is_none() {
        (request.workers - 1) / 2
    } else {
        0
    };
    let inspectors = request.workers - helpers - 1;
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
    let mut pending: BTreeMap<usize, (Key<N>, usize)> = BTreeMap::new();
    let mut heartbeat = Instant::now();
    let (_, mut snapshot, leftovers) = parallel::with_pool(inspectors, inspect, |pool| {
        let mut jobs: Vec<Job<N>> = Vec::new();
        'rounds: while walk.error.is_none() {
            let incoming = match choose(walk, inspectors - jobs.len(), &jobs, cancellation) {
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
                pending.insert(job.ticket, (job.key, job.local_id));
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
                jobs.push(job);
            }
            if jobs.is_empty() {
                break;
            }
            let held = jobs
                .iter()
                .filter(|job| walk.buckets[&job.key].state.queue.next != job.local_id)
                .count();
            walk.metrics.peak_fifo_held_jobs = walk.metrics.peak_fifo_held_jobs.max(held);
            let mut chunks = Vec::new();
            let mut finished = Vec::new();
            // Poll only the FIFO head of each owner. Its State owns the source
            // details/refusals; later jobs retain chunks/Finished in bounded
            // pool slots and cannot contaminate that publisher's provenance.
            jobs.rotate_left(1);
            for job in jobs.iter().filter(|job| {
                !job.finished && walk.buckets[&job.key].state.queue.next == job.local_id
            }) {
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
                        finished.push((job.ticket, job.key, job.local_id, value));
                    }
                    Poll::Waiting => {}
                }
            }
            if walk.error.is_none() && chunks.is_empty() && finished.is_empty() {
                // Recheck all selected slots under the pool mutex before
                // sleeping: a chunk arriving after the scan must not lose
                // its notification. Cancellation is observed after at
                // most the bounded wait even without a producer signal.
                // Later jobs may already have buffered data. Including their
                // tickets here would spin while the only eligible head is quiet.
                let tickets = jobs
                    .iter()
                    .filter(|job| walk.buckets[&job.key].state.queue.next == job.local_id)
                    .map(|job| job.ticket)
                    .collect::<Vec<_>>();
                if tickets.is_empty() {
                    walk.error = Some("owner publication has no eligible FIFO head".into());
                    break 'rounds;
                }
                let start = Instant::now();
                pool.wait_for_any_stream(&tickets);
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
            for (ticket, key, id, value) in finished {
                publish(walk, key, id, value);
                pending.remove(&ticket);
                walk.buckets
                    .get_mut(&key)
                    .expect("published bucket")
                    .outstanding_native_jobs -= 1;
                jobs.iter_mut()
                    .find(|job| job.ticket == ticket)
                    .expect("active ticket")
                    .finished = true;
            }
            if walk.error.is_some() {
                break 'rounds;
            }
            // Refill on the next immediate pass, without requiring an
            // event or completion from any still-running peer.
            jobs.retain(|job| !job.finished);
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
    leftovers::retain(walk, pending, leftovers);
    snapshot["inspection_worker_limit"] = json!(inspectors);
    snapshot["admission_worker_limit"] = json!(helpers);
    snapshot["coordinator_worker_limit"] = json!(1);
    snapshot["total_compute_worker_limit"] = json!(request.workers);
    snapshot["native_pool_ids_are_opaque_tickets"] = json!(true);
    snapshot
}
