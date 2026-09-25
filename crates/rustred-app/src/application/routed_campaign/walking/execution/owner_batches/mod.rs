//! Opt-in, bounded owner-local publication over one immutable native reducer.
//!
//! Each phase/owner keeps the existing admission index and responsibility
//! ledger. Ready native streams feed bounded batches without waiting for quiet
//! peers. Multiple native inspectors may share a key, but only its FIFO head
//! publishes; later streams remain in bounded worker buffers or shared bounded
//! completed-result storage, with all publication obligations still pending.
//! Destination admission remains synchronously batched and exclusive per key.
//! Diagnostic order/cover shapes can vary even at a fixed worker budget.
use super::super::queue::{Domain, Phase};
use super::*;
use std::collections::BTreeMap;
use std::sync::Arc;

mod delivery;
mod driver;
mod report;

type Key<const N: usize> = (Phase, [bool; N]);

fn key_name<const N: usize>(key: &Key<N>) -> String {
    format!(
        "{}:{}",
        if key.0 == Phase::Apply {
            "apply"
        } else {
            "route"
        },
        mask(&key.1)
    )
}

pub(in super::super) struct OwnerBatchReport {
    pub document: Value,
    /// One composite handle for each supplied (already admitted) initial ID.
    pub initial_handles: Vec<Value>,
}

struct Bucket<const N: usize> {
    state: State<N>,
    /// Selection is separate from publication. IDs below this cursor were
    /// selected for native dispatch or skipped as aliases, never discharged.
    dispatch_cursor: usize,
    outstanding_native_jobs: usize,
    peak_outstanding_native_jobs: usize,
    peak_occupied_native_slots: usize,
    admission_seconds: f64,
    native_seconds: f64,
    incoming_requests: usize,
    cross_owner_requests: usize,
}
impl<const N: usize> Bucket<N> {
    fn new(request: &OwnerDomainWalkRequest) -> Result<Self, String> {
        let mut queue = Queue::with_policy(
            request.max_domains,
            request.max_containment_checks,
            request.scheduling_policy,
        )?;
        if request.reuse_initial_d_bands {
            queue
                .delegation
                .as_mut()
                .ok_or("initial-D reuse requires transfer policy")?
                .begin_initial_admission()
                .map_err(|e| e.to_string())?;
        }
        let state = State::new(queue, 0, None);
        state
            .closure
            .borrow_mut()
            .disable("owner-batched cross-bucket dependency tracking unavailable");
        Ok(Self {
            state,
            dispatch_cursor: 0,
            outstanding_native_jobs: 0,
            peak_outstanding_native_jobs: 0,
            peak_occupied_native_slots: 0,
            admission_seconds: 0.0,
            native_seconds: 0.0,
            incoming_requests: 0,
            cross_owner_requests: 0,
        })
    }
    fn finish_initial(&mut self, request: &OwnerDomainWalkRequest) -> Result<(), String> {
        if request.reuse_initial_d_bands {
            self.state
                .queue
                .delegation
                .as_mut()
                .ok_or("initial-D ledger missing")?
                .finish_initial_admission()
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

#[derive(Default)]
struct Budget {
    domains: usize,
    events: usize,
    frontiers: usize,
    checks: usize,
}

#[derive(Default)]
struct Metrics {
    rounds: usize,
    chunk_rounds: usize,
    parallel_admission_batches: usize,
    serial_budget_batches: usize,
    idle_stream_wait_seconds: f64,
    delivery_seconds: f64,
    peak_coordinator_logical_bytes: usize,
    delivered_cross_owner_requests: usize,
    native_tickets: usize,
    peak_fifo_held_jobs: usize,
}

struct Walk<const N: usize> {
    buckets: BTreeMap<Key<N>, Bucket<N>>,
    initial: InitialOrthants<N>,
    overlaps: BTreeMap<Key<N>, InitialOverlapIndex<N>>,
    budget: Budget,
    /// Work already spent by the common initial-query admission pass, including
    /// queries discarded by inclusion. It still consumes the aggregate budget.
    initial_prepass_checks: usize,
    metrics: Metrics,
    error: Option<String>,
    /// Round-robin key cursor prevents a productive low key starving others.
    last_key: Option<Key<N>>,
}

pub(in super::super) fn run<const N: usize>(
    initial_domains: &[Arc<Domain<N>>],
    initial_frontiers: usize,
    initial_prepass_checks: usize,
    reducer: &RoutedCandidateReducer<N>,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<OwnerBatchReport, String> {
    let (mut walk, handles) = initialize(
        initial_domains,
        initial_frontiers,
        initial_prepass_checks,
        request,
        cancellation,
    )?;
    if request.reuse_initial_d_bands {
        observer(
            json!({"event":"initial_overlap_prepared","operation":"owner_domain_walk",
            "initial_overlap_index":report::overlap_summary(&walk)}),
        );
    }
    let started = Instant::now();
    let parallel = driver::run(&mut walk, reducer, request, cancellation, observer);
    let document = report::finish(walk, request, started.elapsed().as_secs_f64(), parallel);
    Ok(OwnerBatchReport {
        document,
        initial_handles: handles,
    })
}

fn initialize<const N: usize>(
    initial_domains: &[Arc<Domain<N>>],
    initial_frontiers: usize,
    initial_prepass_checks: usize,
    request: &OwnerDomainWalkRequest,
    cancellation: &AtomicBool,
) -> Result<(Walk<N>, Vec<Value>), String> {
    if initial_frontiers > request.max_frontiers
        || request
            .max_containment_checks
            .is_some_and(|cap| initial_prepass_checks > cap)
    {
        return Err("initial owner-batch aggregate allowance".into());
    }
    let mut walk = Walk {
        buckets: BTreeMap::new(),
        initial: InitialOrthants::from_initial(initial_domains, cancellation),
        overlaps: BTreeMap::new(),
        budget: Budget {
            frontiers: initial_frontiers,
            checks: initial_prepass_checks,
            ..Default::default()
        },
        initial_prepass_checks,
        metrics: Metrics::default(),
        error: None,
        last_key: None,
    };
    let mut handles = Vec::new();
    handles
        .try_reserve_exact(initial_domains.len())
        .map_err(|_| "initial handle allocation")?;
    for domain in initial_domains {
        let key = (domain.phase, domain.owner);
        if !walk.buckets.contains_key(&key) {
            walk.buckets.insert(key, Bucket::new(request)?);
        }
        let bucket = walk.buckets.get_mut(&key).expect("inserted initial bucket");
        let before = bucket.state.queue.containment_checks;
        let checks = request
            .max_containment_checks
            .map(|cap| {
                before
                    .checked_add(cap.saturating_sub(walk.budget.checks))
                    .ok_or("comparison limit overflow")
            })
            .transpose()?;
        let domains = bucket
            .state
            .queue
            .domains
            .len()
            .checked_add(request.max_domains.saturating_sub(walk.budget.domains))
            .ok_or("domain limit overflow")?;
        let result = bucket
            .state
            .queue
            .admit_with_budget((**domain).clone(), domains, checks);
        walk.budget.checks = walk
            .budget
            .checks
            .checked_add(bucket.state.queue.containment_checks - before)
            .ok_or("comparison counter overflow")?;
        let (id, added) = result.map_err(str::to_owned)?;
        walk.budget.domains = walk
            .budget
            .domains
            .checked_add(usize::from(added))
            .ok_or("domain counter overflow")?;
        handles.push(json!({"bucket":key_name(&key),"local_id":id}));
    }
    for (key, bucket) in &mut walk.buckets {
        bucket.finish_initial(request)?;
        if request.reuse_initial_d_bands {
            let index =
                InitialOverlapIndex::from_initial(&bucket.state.queue.domains, cancellation);
            bucket.state.initial_overlap_report = Some(index.build_report());
            walk.overlaps.insert(*key, index);
        }
    }
    if walk.budget.domains > request.max_domains
        || walk.budget.frontiers > request.max_frontiers
        || request
            .max_containment_checks
            .is_some_and(|n| walk.budget.checks > n)
    {
        return Err("initial owner-batch aggregate allowance".into());
    }
    Ok((walk, handles))
}

#[cfg(test)]
mod tests;
