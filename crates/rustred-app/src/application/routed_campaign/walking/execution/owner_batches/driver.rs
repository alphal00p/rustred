//! Bounded ready-stream batches; a quiet owner cannot hold up ready peers.
use super::*;

#[cfg(test)]
mod tests;

struct Job<const N: usize> {
    key: Key<N>,
    local_id: usize,
    ticket: usize,
    domain: Arc<Domain<N>>,
    finished: bool,
}

fn choose<const N: usize>(
    walk: &mut Walk<N>,
    width: usize,
    excluded: &std::collections::BTreeSet<Key<N>>,
    cancellation: &AtomicBool,
) -> Result<Vec<Job<N>>, String> {
    if width == 0 {
        return Ok(Vec::new());
    }
    let mut keys = walk.buckets.keys().copied().collect::<Vec<_>>();
    if let Some(previous) = walk.last_key {
        let split = keys.partition_point(|key| *key <= previous);
        keys.rotate_left(split);
    }
    let mut jobs = Vec::new();
    jobs.try_reserve_exact(width.min(keys.len()))
        .map_err(|_| "owner round allocation")?;
    for key in keys {
        if cancellation.load(Ordering::Acquire) {
            return Err("cancelled".into());
        }
        if excluded.contains(&key) {
            continue;
        }
        let state = &mut walk.buckets.get_mut(&key).expect("known bucket").state;
        while state.queue.next < state.queue.domains.len() && state.current_is_delegated() {
            if cancellation.load(Ordering::Acquire) {
                return Err("cancelled".into());
            }
            state.commit_delegated()?;
        }
        let id = state.queue.next;
        if id == state.queue.domains.len() {
            continue;
        }
        let ticket = walk.metrics.native_tickets;
        walk.metrics.native_tickets = ticket.checked_add(1).ok_or("native ticket overflow")?;
        jobs.push(Job {
            key,
            local_id: id,
            ticket,
            domain: state.queue.domains[id].clone(),
            finished: false,
        });
        walk.last_key = Some(key);
        if jobs.len() == width {
            break;
        }
    }
    Ok(jobs)
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
    let excluded = std::collections::BTreeSet::new();
    let mut heartbeat = Instant::now();
    while walk.error.is_none() {
        let jobs = match choose(walk, 1, &excluded, cancellation) {
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
            let excluded = jobs.iter().map(|job| job.key).collect();
            let incoming = match choose(walk, inspectors - jobs.len(), &excluded, cancellation) {
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
                jobs.push(job);
            }
            if jobs.is_empty() {
                break;
            }
            let mut chunks = Vec::new();
            let mut finished = Vec::new();
            // One nonblocking poll per producer, at most one bounded
            // chunk each. Never wait on a quiet stream while another can
            // publish. Rotation keeps tie-breaking fair as slots refill.
            jobs.rotate_left(1);
            for job in jobs.iter().filter(|job| !job.finished) {
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
                let tickets = jobs.iter().map(|job| job.ticket).collect::<Vec<_>>();
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
    for (ticket, finished) in leftovers {
        if let Some((key, id)) = pending.remove(&ticket) {
            publish(walk, key, id, finished);
        }
    }
    for (ticket, (key, id)) in pending {
        walk.error
            .get_or_insert_with(|| "native completion unavailable".into());
        let bucket = walk.buckets.get_mut(&key).expect("pending source");
        bucket
            .state
            .error
            .get_or_insert_with(|| walk.error.clone().expect("outer failure"));
        bucket
            .state
            .uncommitted
            .push(json!({"id":id,"native_ticket":ticket,"stats":null,
            "committed":false,"partial_native_statistics_unavailable":true,
            "frontiers":std::mem::take(&mut bucket.state.details),"error":walk.error}));
    }
    snapshot["inspection_worker_limit"] = json!(inspectors);
    snapshot["admission_worker_limit"] = json!(helpers);
    snapshot["coordinator_worker_limit"] = json!(1);
    snapshot["total_compute_worker_limit"] = json!(request.workers);
    snapshot["native_pool_ids_are_opaque_tickets"] = json!(true);
    snapshot
}
