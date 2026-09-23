//! Bounded immutable lookup preparation followed by original-order publication.
//!
//! Inspection producers can block waiting for publication, so these helpers
//! deliberately use a separate, reserved Rayon pool. Sharing the producer pool
//! would allow all threads to block while the publisher waits for a lookup.
use super::super::queue::PreparedAdmission;
use super::super::worker_budget::WorkerBudget;
use super::*;
use rayon::prelude::*;

const BATCH_RECORDS: usize = 256;
const MIN_ADMISSIONS: usize = 16;
const MIN_CANDIDATES: usize = 128;

pub(super) struct Metrics {
    budget: WorkerBudget,
    batches: usize,
    records: usize,
    preparations: usize,
    speculative_checks: usize,
    preparation_seconds: f64,
    ordered_commit_seconds: f64,
    counter_saturated: bool,
}
impl Default for Metrics {
    fn default() -> Self {
        Self::new(WorkerBudget::new(
            1,
            None,
            None,
            super::super::OwnerDomainWalkPublicationPolicy::Ordered,
        ))
    }
}
impl Metrics {
    pub fn new(budget: WorkerBudget) -> Self {
        Self {
            budget,
            batches: 0,
            records: 0,
            preparations: 0,
            speculative_checks: 0,
            preparation_seconds: 0.0,
            ordered_commit_seconds: 0.0,
            counter_saturated: false,
        }
    }
    fn add(counter: &mut usize, amount: usize, saturated: &mut bool) {
        *counter = match counter.checked_add(amount) {
            Some(next) => next,
            None => {
                *saturated = true;
                usize::MAX
            }
        };
    }
    pub fn json(&self) -> Value {
        json!({"policy":"immutable_bounded_batch_ordered_commit",
            "requested_worker_budget":self.budget.requested,
            "inspection_worker_limit":self.budget.inspection,
            "lookup_worker_limit":self.budget.helpers,
            "coordinator_worker_limit":self.budget.coordinator,
            "total_compute_worker_limit":self.budget.inspection+self.budget.helpers+self.budget.coordinator,
            "batch_record_limit":BATCH_RECORDS, "minimum_admissions":MIN_ADMISSIONS,
            "minimum_candidates":MIN_CANDIDATES, "parallel_batches":self.batches,
            "prepared_batch_records":self.records, "speculative_admission_requests":self.preparations,
            "speculative_containment_checks":self.speculative_checks,
            "speculative_check_scope":"all_completed_preparation_checks; overlaps_committed_checks_when_reused; do_not_sum",
            "preparation_wall_seconds":self.preparation_seconds,
            "ordered_commit_wall_seconds":self.ordered_commit_seconds,
            "counter_saturated":self.counter_saturated,
            "timing_scope":"coordinator_wall; preparation_includes_wait_for_all_helpers; commit_excludes_observer",
            "speculative_work_is_not_admission":true})
    }
}

enum PreparedEvent<const N: usize> {
    Original(Event<N>),
    Admission {
        count: usize,
        successor: bool,
        conditional: bool,
        prepared: PreparedAdmission<N>,
    },
}
impl<const N: usize> PreparedEvent<N> {
    fn prepare(
        event: Event<N>,
        queue: &Queue<N>,
        cancellation: &AtomicBool,
        producer_stop: &AtomicBool,
    ) -> Self {
        let Event { count, effect } = event;
        match effect {
            Effect::Admit {
                domain,
                successor,
                conditional,
            } => Self::Admission {
                count,
                successor,
                conditional,
                prepared: queue.prepare_admission_with_stop(domain, cancellation, producer_stop),
            },
            effect => Self::Original(Event { count, effect }),
        }
    }
    fn commit(
        self,
        state: &mut State<N>,
        request: &OwnerDomainWalkRequest,
    ) -> Result<(), &'static str> {
        match self {
            Self::Original(event) => state.accept(event, request),
            Self::Admission {
                count,
                successor,
                conditional,
                prepared,
            } => {
                state.charge_event(count, request)?;
                state.apply_admission(successor, conditional, |queue| {
                    queue.admit_prepared(prepared)
                })
            }
        }
    }
}

pub(super) struct Engine {
    pool: Option<rayon::ThreadPool>,
}
impl Engine {
    pub fn new(budget: WorkerBudget) -> Result<Self, String> {
        let pool = if budget.helpers == 0 {
            None
        } else {
            Some(
                rayon::ThreadPoolBuilder::new()
                    .num_threads(budget.helpers)
                    .thread_name(|index| format!("owner-admission-{index}"))
                    .build()
                    .map_err(|error| format!("admission worker pool: {error}"))?,
            )
        };
        Ok(Self { pool })
    }

    fn prepare<const N: usize>(
        &self,
        queue: &Queue<N>,
        events: Vec<Event<N>>,
        cancellation: &AtomicBool,
        producer_stop: &AtomicBool,
        metrics: &mut Metrics,
    ) -> Vec<PreparedEvent<N>> {
        let admissions = events
            .iter()
            .filter(|event| matches!(event.effect, Effect::Admit { .. }))
            .count();
        if let Some(pool) = &self.pool
            && queue.containment_limit().is_none()
            && queue.containment_candidate_count() >= MIN_CANDIDATES
            && admissions >= MIN_ADMISSIONS
        {
            Metrics::add(&mut metrics.batches, 1, &mut metrics.counter_saturated);
            Metrics::add(
                &mut metrics.records,
                events.len(),
                &mut metrics.counter_saturated,
            );
            Metrics::add(
                &mut metrics.preparations,
                admissions,
                &mut metrics.counter_saturated,
            );
            let started = Instant::now();
            // Vec's indexed parallel iterator preserves every original ordinal,
            // including non-admission callbacks; no queue mutation is possible
            // until all scoped work has joined and this shared borrow ends.
            let prepared: Vec<_> = pool.install(|| {
                events
                    .into_par_iter()
                    .map(|event| PreparedEvent::prepare(event, queue, cancellation, producer_stop))
                    .collect()
            });
            metrics.preparation_seconds += started.elapsed().as_secs_f64();
            for event in &prepared {
                if let PreparedEvent::Admission { prepared, .. } = event {
                    Metrics::add(
                        &mut metrics.speculative_checks,
                        prepared.speculative_checks(),
                        &mut metrics.counter_saturated,
                    );
                }
            }
            prepared
        } else {
            events.into_iter().map(PreparedEvent::Original).collect()
        }
    }

    pub fn commit_chunk<const N: usize>(
        &self,
        state: &mut State<N>,
        request: &OwnerDomainWalkRequest,
        chunk: Vec<Event<N>>,
        cancellation: &AtomicBool,
        producer_stop: &AtomicBool,
        heartbeat: &mut impl FnMut(&State<N>),
    ) -> Result<(), &'static str> {
        let mut events = chunk.into_iter();
        loop {
            if cancellation.load(Ordering::Acquire) || producer_stop.load(Ordering::Acquire) {
                return Err("cancelled");
            }
            let batch: Vec<_> = events.by_ref().take(BATCH_RECORDS).collect();
            if batch.is_empty() {
                return Ok(());
            }
            let prepared = self.prepare(
                &state.queue,
                batch,
                cancellation,
                producer_stop,
                &mut state.admission,
            );
            Self::commit_prepared(state, request, prepared, cancellation, producer_stop)?;
            heartbeat(state);
        }
    }

    fn commit_prepared<const N: usize>(
        state: &mut State<N>,
        request: &OwnerDomainWalkRequest,
        prepared: Vec<PreparedEvent<N>>,
        cancellation: &AtomicBool,
        producer_stop: &AtomicBool,
    ) -> Result<(), &'static str> {
        // A token is never authority to commit after cancellation or a
        // producer failure. Every helper has joined before this check.
        let started = Instant::now();
        let result = (|| {
            for event in prepared {
                if cancellation.load(Ordering::Acquire) || producer_stop.load(Ordering::Acquire) {
                    return Err("cancelled");
                }
                event.commit(state, request)?;
            }
            Ok(())
        })();
        state.admission.ordered_commit_seconds += started.elapsed().as_secs_f64();
        result
    }
}

#[cfg(test)]
mod tests;
