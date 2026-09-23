//! Fair bounded native selection, independent of the owner publication cursor.
use super::*;

pub(super) fn choose<const N: usize>(
    walk: &mut Walk<N>,
    width: usize,
    active: &[Job<N>],
    cancellation: &AtomicBool,
) -> Result<Vec<Job<N>>, String> {
    let mut keys = walk.buckets.keys().copied().collect::<Vec<_>>();
    if let Some(previous) = walk.last_key {
        let split = keys.partition_point(|key| *key <= previous);
        keys.rotate_left(split);
    }
    let mut jobs = Vec::new();
    jobs.try_reserve_exact(width)
        .map_err(|_| "owner round allocation")?;
    let mut counts = BTreeMap::<Key<N>, usize>::new();
    for job in active {
        *counts.entry(job.key).or_default() += 1;
    }
    for key in &keys {
        if cancellation.load(Ordering::Acquire) {
            return Err("cancelled".into());
        }
        let bucket = walk.buckets.get_mut(key).expect("known bucket");
        let state = &mut bucket.state;
        // Aliases may advance only the canonical publication cursor. A future
        // alias can be skipped by selection without prematurely publishing it.
        while state.queue.next < state.queue.domains.len() && state.current_is_delegated() {
            if cancellation.load(Ordering::Acquire) {
                return Err("cancelled".into());
            }
            state.commit_delegated()?;
        }
        bucket.dispatch_cursor = bucket.dispatch_cursor.max(state.queue.next);
    }
    while jobs.len() < width {
        let mut selected = None;
        for (position, key) in keys.iter().enumerate() {
            if cancellation.load(Ordering::Acquire) {
                return Err("cancelled".into());
            }
            let bucket = walk.buckets.get_mut(key).expect("known bucket");
            let queue = &bucket.state.queue;
            let end = queue
                .delegation
                .as_ref()
                .map_or(queue.domains.len(), |ledger| {
                    queue.domains.len().min(ledger.dispatch_fence())
                });
            while bucket.dispatch_cursor < end
                && queue
                    .delegation
                    .as_ref()
                    .is_some_and(|ledger| ledger.delegated_to(bucket.dispatch_cursor).is_some())
            {
                bucket.dispatch_cursor += 1;
            }
            if bucket.dispatch_cursor == end || bucket.dispatch_cursor >= queue.domains.len() {
                continue;
            }
            if queue
                .delegation
                .as_ref()
                .is_some_and(|ledger| !ledger.can_dispatch(bucket.dispatch_cursor))
            {
                return Err("owner native dispatch has no reserved responsibility".into());
            }
            let count = counts.get(key).copied().unwrap_or(0);
            // Offer every ready key its first inspector before a hot key gets
            // another. Round-robin breaks ties, including on later refills.
            if selected.is_none_or(|(_, best)| count < best) {
                selected = Some((position, count));
            }
        }
        let Some((position, count)) = selected else {
            break;
        };
        let key = keys[position];
        let bucket = walk.buckets.get_mut(&key).expect("selected bucket");
        let id = bucket.dispatch_cursor;
        let ticket = walk.metrics.native_tickets;
        walk.metrics.native_tickets = ticket.checked_add(1).ok_or("native ticket overflow")?;
        bucket.dispatch_cursor += 1;
        jobs.push(Job {
            key,
            local_id: id,
            ticket,
            domain: bucket.state.queue.domains[id].clone(),
        });
        walk.last_key = Some(key);
        counts.insert(key, count + 1);
        keys.rotate_left(position + 1);
    }
    Ok(jobs)
}
