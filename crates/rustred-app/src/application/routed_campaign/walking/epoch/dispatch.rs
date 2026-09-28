//! Dispatch (W2.0 protocol §8.3), default `Fifo`: requeued Reserved IDs
//! (attempts < 2) first, a guaranteed share for deferred IDs (attempts >= 2),
//! then a monotone ID cursor over Pending. T2 (Pending -> Reserved) happens
//! at enqueue. Isolation and the crash-journal guard are S3 (no crash
//! journal exists in S2).
use super::job::Job;
use super::ledger6::{Entry6, Transition};
use super::state::{EpochState, JobMeta};
use std::collections::VecDeque;

/// Session number of the S2 engine (no restore exists yet): sequence numbers
/// are `session << 40 | counter`, deterministic and unique in the run.
const SESSION: u64 = 1;

pub(super) struct Dispatch {
    cursor: u32,
    requeue: VecDeque<u32>,
    deferred: VecDeque<u32>,
    counter: u64,
}

pub(super) enum Refill<const N: usize> {
    Jobs(Vec<Job<N>>),
    /// Nothing Pending or Reserved, nothing in flight.
    Drained,
    /// Pending or Reserved IDs exist, none can be dispatched (C5).
    Stalled,
}

impl Dispatch {
    pub fn new() -> Self {
        Self {
            cursor: 0,
            requeue: VecDeque::new(),
            deferred: VecDeque::new(),
            counter: 0,
        }
    }

    /// A discarded result's ID goes back (it stays Reserved).
    pub fn requeue(&mut self, id: u32, attempts: u8) {
        if attempts >= 2 {
            self.deferred.push_back(id);
        } else {
            self.requeue.push_back(id);
        }
    }

    pub fn queued(&self) -> (usize, usize) {
        (self.requeue.len(), self.deferred.len())
    }

    fn job<const N: usize>(&mut self, state: &mut EpochState<N>, id: u32) -> Job<N> {
        self.counter += 1;
        let seq = (SESSION << 40) | self.counter;
        state.in_flight.insert(id, JobMeta { seq, v0: state.k });
        let attempts = state
            .ledger
            .get(id)
            .ok()
            .and_then(Entry6::counters)
            .map_or(0, |c| c.attempts);
        Job {
            seq,
            parent: id,
            v0: state.k,
            attempts,
            flags: 0,
            image: state.store.domains[id as usize],
        }
    }

    /// Up to `want` jobs: requeued first (at most half of the refill while
    /// Pending IDs exist, the age bound), at least one deferred ID per refill
    /// while any is waiting, then the lowest Pending IDs by the cursor.
    pub fn refill<const N: usize>(&mut self, state: &mut EpochState<N>, want: usize) -> Refill<N> {
        let mut jobs = Vec::with_capacity(want);
        if let Some(id) = self.deferred.pop_front() {
            jobs.push(self.job(state, id));
        }
        let requeue_budget = if state.ledger.counts().get(super::ledger6::Tag::Pending) > 0 {
            want / 2
        } else {
            want
        };
        while jobs.len() < requeue_budget.max(1).min(want) {
            let Some(id) = self.requeue.pop_front() else {
                break;
            };
            jobs.push(self.job(state, id));
        }
        let watermark = state.watermark();
        while jobs.len() < want && self.cursor < watermark {
            let id = self.cursor;
            if matches!(state.ledger.get(id), Ok(Entry6::Pending(_))) {
                state
                    .ledger
                    .apply(id, Transition::T2Reserve)
                    .expect("T2 on a Pending ID");
                state.counters.dispatched += 1;
                jobs.push(self.job(state, id));
            }
            self.cursor += 1;
        }
        while jobs.len() < want {
            let Some(id) = self
                .requeue
                .pop_front()
                .or_else(|| self.deferred.pop_front())
            else {
                break;
            };
            jobs.push(self.job(state, id));
        }
        if !jobs.is_empty() {
            return Refill::Jobs(jobs);
        }
        if state.in_flight.is_empty() && state.pending_or_reserved() == 0 {
            Refill::Drained
        } else {
            Refill::Stalled
        }
    }
}
