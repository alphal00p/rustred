//! Source accounting stays with the publisher; geometry goes to its destination.
use super::*;
use rayon::prelude::*;

struct Incoming<const N: usize> {
    ordinal: usize,
    source: Key<N>,
    domain: Domain<N>,
}

fn ensure_bucket<const N: usize>(
    walk: &mut Walk<N>,
    key: Key<N>,
    request: &OwnerDomainWalkRequest,
) -> Result<(), String> {
    if !walk.buckets.contains_key(&key) {
        let mut bucket = Bucket::new(request)?;
        // A later-created bucket has an empty, but still pinned, initial prefix.
        bucket.finish_initial(request)?;
        walk.buckets.insert(key, bucket);
    }
    Ok(())
}

fn account<const N: usize>(
    walk: &mut Walk<N>,
    source: Key<N>,
    event: Event<N>,
    request: &OwnerDomainWalkRequest,
    limited: bool,
) -> Result<Option<Domain<N>>, String> {
    let state = &mut walk
        .buckets
        .get_mut(&source)
        .ok_or("missing source bucket")?
        .state;
    let (max_events, max_frontiers) = if limited {
        let max_events = state
            .events
            .checked_add(request.max_events.saturating_sub(walk.budget.events))
            .ok_or("event limit overflow")?;
        let max_frontiers = state
            .frontiers
            .checked_add(request.max_frontiers.saturating_sub(walk.budget.frontiers))
            .ok_or("frontier limit overflow")?;
        (max_events, max_frontiers)
    } else {
        (request.max_events, request.max_frontiers)
    };
    let events_before = state.events;
    let frontiers_before = state.frontiers;
    let result = match event {
        Event {
            count,
            effect:
                Effect::Admit {
                    domain,
                    successor,
                    conditional,
                },
        } => {
            if count != 1 {
                return Err("non-unit geometric admission event".into());
            }
            state
                .charge_event_with_limit(count, max_events)
                .and_then(|()| {
                    state.successors = state
                        .successors
                        .checked_add(usize::from(successor))
                        .ok_or("successor counter overflow")?;
                    state.conditional = state
                        .conditional
                        .checked_add(usize::from(conditional))
                        .ok_or("conditional successor overflow")?;
                    Ok(Some(domain))
                })
        }
        event => state
            .accept_with_limits(event, max_events, max_frontiers)
            .map(|()| None),
    };
    walk.budget.events = walk
        .budget
        .events
        .checked_add(state.events - events_before)
        .ok_or("global event counter overflow")?;
    walk.budget.frontiers = walk
        .budget
        .frontiers
        .checked_add(state.frontiers - frontiers_before)
        .ok_or("global frontier counter overflow")?;
    result.map_err(str::to_owned)
}

/// The canonical fallback is important near aggregate caps: reserving every
/// proposal as a new domain would wrongly refuse a batch of reusable requests.
fn serial<const N: usize>(
    walk: &mut Walk<N>,
    chunks: Vec<(Key<N>, Vec<Event<N>>)>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
) -> Result<(), String> {
    for (source, events) in chunks {
        for event in events {
            if cancellation.load(Ordering::Acquire) {
                return Err("cancelled".into());
            }
            let Some(domain) = account(walk, source, event, request, true)? else {
                continue;
            };
            let key = (domain.phase, domain.owner);
            ensure_bucket(walk, key, request)?;
            let bucket = walk.buckets.get_mut(&key).expect("destination exists");
            let before = bucket.state.queue.domains.len();
            let before_checks = bucket.state.queue.containment_checks;
            let limit = before
                .checked_add(request.max_domains.saturating_sub(walk.budget.domains))
                .ok_or("domain limit overflow")?;
            // Preserve None: switching to Some would switch the queue from the
            // semantic index to its historical finite-cap/raw-bound lane.
            let checks = request
                .max_containment_checks
                .map(|cap| {
                    before_checks
                        .checked_add(cap.saturating_sub(walk.budget.checks))
                        .ok_or("comparison limit overflow")
                })
                .transpose()?;
            let start = Instant::now();
            let result = bucket.state.queue.admit_with_budget(domain, limit, checks);
            bucket.admission_seconds += start.elapsed().as_secs_f64();
            bucket.incoming_requests = bucket
                .incoming_requests
                .checked_add(1)
                .ok_or("incoming counter overflow")?;
            if source != key {
                bucket.cross_owner_requests = bucket
                    .cross_owner_requests
                    .checked_add(1)
                    .ok_or("cross-owner counter overflow")?;
                walk.metrics.delivered_cross_owner_requests = walk
                    .metrics
                    .delivered_cross_owner_requests
                    .checked_add(1)
                    .ok_or("cross-owner counter overflow")?;
            }
            walk.budget.domains = walk
                .budget
                .domains
                .checked_add(bucket.state.queue.domains.len() - before)
                .ok_or("domain counter overflow")?;
            walk.budget.checks = walk
                .budget
                .checks
                .checked_add(bucket.state.queue.containment_checks - before_checks)
                .ok_or("comparison counter overflow")?;
            result.map_err(str::to_owned)?;
        }
    }
    Ok(())
}

pub(super) fn deliver<const N: usize>(
    walk: &mut Walk<N>,
    chunks: Vec<(Key<N>, Vec<Event<N>>)>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    pool: Option<&rayon::ThreadPool>,
) -> Result<(), String> {
    let mut events = 0usize;
    let mut frontiers = 0usize;
    let mut admissions = 0usize;
    let mut bytes = 0usize;
    for (_, chunk) in &chunks {
        for event in chunk {
            events = events
                .checked_add(event.count)
                .ok_or("chunk event overflow")?;
            frontiers = frontiers
                .checked_add(usize::from(matches!(
                    &event.effect,
                    Effect::Frontier { .. }
                )))
                .ok_or("chunk frontier overflow")?;
            admissions = admissions
                .checked_add(usize::from(matches!(&event.effect, Effect::Admit { .. })))
                .ok_or("chunk admission overflow")?;
            bytes = bytes
                .checked_add(event.weight())
                .ok_or("chunk byte overflow")?;
        }
    }
    walk.metrics.peak_coordinator_logical_bytes =
        walk.metrics.peak_coordinator_logical_bytes.max(bytes);
    let start = Instant::now();
    let fast = request.max_containment_checks.is_none()
        && events <= request.max_events.saturating_sub(walk.budget.events)
        && frontiers <= request.max_frontiers.saturating_sub(walk.budget.frontiers)
        && admissions <= request.max_domains.saturating_sub(walk.budget.domains);
    if !fast || pool.is_none() {
        walk.metrics.serial_budget_batches = walk
            .metrics
            .serial_budget_batches
            .checked_add(1)
            .ok_or("serial batch counter overflow")?;
        let result = serial(walk, chunks, request, cancellation);
        walk.metrics.delivery_seconds += start.elapsed().as_secs_f64();
        return result;
    }
    let mut incoming: BTreeMap<Key<N>, Vec<Incoming<N>>> = BTreeMap::new();
    let mut ordinal = 0usize;
    for (source, chunk) in chunks {
        for event in chunk {
            if cancellation.load(Ordering::Acquire) {
                return Err("cancelled".into());
            }
            if let Some(domain) = account(walk, source, event, request, false)? {
                let key = (domain.phase, domain.owner);
                ensure_bucket(walk, key, request)?;
                let rows = incoming.entry(key).or_default();
                rows.try_reserve(1).map_err(|_| "owner inbox allocation")?;
                rows.push(Incoming {
                    ordinal,
                    source,
                    domain,
                });
            }
            ordinal = ordinal.checked_add(1).ok_or("chunk ordinal overflow")?;
        }
    }
    let mut batches = Vec::new();
    batches
        .try_reserve_exact(incoming.len())
        .map_err(|_| "owner delivery batch allocation")?;
    for (key, bucket) in &mut walk.buckets {
        if let Some(rows) = incoming.remove(key) {
            batches.push((*key, bucket, rows));
        }
    }
    walk.metrics.parallel_admission_batches = walk
        .metrics
        .parallel_admission_batches
        .checked_add(1)
        .ok_or("parallel batch counter overflow")?;
    let outcomes = pool.expect("fast path admission pool").install(|| {
        batches
            .into_par_iter()
            .map(|(key, bucket, rows)| {
                let start = Instant::now();
                let before = bucket.state.queue.domains.len();
                let checks = bucket.state.queue.containment_checks;
                let mut cross = 0usize;
                let mut error = None;
                for row in rows {
                    if cancellation.load(Ordering::Acquire) {
                        error = Some((row.ordinal, "cancelled".to_owned()));
                        break;
                    }
                    let Some(next_incoming) = bucket.incoming_requests.checked_add(1) else {
                        error = Some((row.ordinal, "incoming counter overflow".into()));
                        break;
                    };
                    let crosses = usize::from(row.source != key);
                    let Some(next_cross) = bucket.cross_owner_requests.checked_add(crosses) else {
                        error = Some((row.ordinal, "cross-owner counter overflow".into()));
                        break;
                    };
                    let Some(next_batch_cross) = cross.checked_add(crosses) else {
                        error = Some((row.ordinal, "batch cross-owner counter overflow".into()));
                        break;
                    };
                    let result = bucket.state.queue.admit(row.domain);
                    bucket.incoming_requests = next_incoming;
                    bucket.cross_owner_requests = next_cross;
                    cross = next_batch_cross;
                    if let Err(reason) = result {
                        error = Some((row.ordinal, reason.to_owned()));
                        break;
                    }
                }
                bucket.admission_seconds += start.elapsed().as_secs_f64();
                (
                    bucket.state.queue.domains.len() - before,
                    bucket.state.queue.containment_checks - checks,
                    cross,
                    error,
                )
            })
            .collect::<Vec<_>>()
    });
    let mut first_error = None;
    for (domains, checks, cross, error) in outcomes {
        walk.budget.domains = walk
            .budget
            .domains
            .checked_add(domains)
            .ok_or("global domain counter overflow")?;
        walk.budget.checks = walk
            .budget
            .checks
            .checked_add(checks)
            .ok_or("global comparison counter overflow")?;
        walk.metrics.delivered_cross_owner_requests = walk
            .metrics
            .delivered_cross_owner_requests
            .checked_add(cross)
            .ok_or("global cross-owner counter overflow")?;
        if let Some(error) = error {
            if first_error
                .as_ref()
                .is_none_or(|(ordinal, _)| error.0 < *ordinal)
            {
                first_error = Some(error);
            }
        }
    }
    walk.metrics.delivery_seconds += start.elapsed().as_secs_f64();
    first_error.map_or(Ok(()), |(_, error)| Err(error))
}
