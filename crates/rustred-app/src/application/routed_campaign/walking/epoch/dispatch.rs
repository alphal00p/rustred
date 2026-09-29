//! Dispatch (W2.0 protocol §8.3), default `Fifo`: requeued Reserved IDs
//! (attempts < 2) first, a guaranteed share for deferred IDs (attempts >= 2),
//! then a monotone ID cursor over Pending. T2 (Pending -> Reserved) happens
//! at enqueue. Private S3 restore reserves a durable session before reissuing
//! an unfinished cut; crash-attribution/isolation policy remains separate.
use super::checkpoint::{SEQUENCE_COUNTER_LIMIT, SESSION_LIMIT, Session};
use super::job::Job;
use super::ledger6::{Entry6, Transition};
use super::state::{EpochState, JobMeta};
use std::collections::VecDeque;

pub(super) mod adaptive;

pub(super) struct Dispatch {
    cursor: u32,
    requeue: VecDeque<u32>,
    deferred: VecDeque<u32>,
    counter: u64,
    session: u64,
    /// A restored batch is returned separately; no new Pending work may
    /// overtake it. This flag clears only after its in-flight map is empty.
    replay_pending: bool,
    /// A restored input prefix is saveable, never a complete query set.
    admission_ready: bool,
    adaptive: Option<adaptive::Saved>,
}

/// Borrowed execution order, not reconstructed from the ledger. Session and
/// counter are actual issuance state, not a fresh-session constant.
#[derive(Clone, Copy)]
pub(super) struct DispatchSnapshot<'a> {
    pub session: u64,
    pub counter: u64,
    pub cursor: u32,
    pub requeue: &'a VecDeque<u32>,
    pub deferred: &'a VecDeque<u32>,
    pub adaptive: Option<&'a adaptive::Saved>,
}

pub(super) enum Refill<const N: usize> {
    Jobs(Vec<Job<N>>),
    /// Nothing Pending or Reserved, nothing in flight.
    Drained,
    /// Pending or Reserved IDs exist, none can be dispatched (C5).
    Stalled,
    /// No sequence bits remain. Persist the coherent boundary; a new durable
    /// session can continue, but this one must never wrap or reuse a sequence.
    SequenceExhausted,
}

impl Dispatch {
    pub fn rescue_retry(
        &mut self,
        id: u32,
        ledger: &mut super::ledger6::Ledger6,
    ) -> Result<(), String> {
        self.requeue
            .try_reserve(1)
            .map_err(|_| "rescue requeue allocation".to_owned())?;
        ledger
            .apply(id, Transition::T9Retry)
            .map_err(|e| e.to_string())?;
        ledger
            .apply(id, Transition::T2Reserve)
            .map_err(|e| e.to_string())?;
        self.requeue.push_back(id);
        Ok(())
    }
    /// Admission and replay readiness are distinct: restoring an unfinished
    /// cut must rebuild its immutable input overlap before replay can run.
    pub fn admission_complete(&self) -> bool {
        self.admission_ready
    }

    pub fn admission_ready(&self) -> bool {
        self.admission_ready && !self.replay_pending
    }

    /// Fresh publisher session 1 exists before this private constructor runs.
    pub fn for_admission() -> Self {
        Self {
            admission_ready: false,
            ..Self::new()
        }
    }

    pub fn validate_admission<const N: usize>(
        state: &EpochState<N>,
        saved: DispatchSnapshot<'_>,
    ) -> Result<(), String> {
        let mut expected = super::state::WalkCounters::default();
        expected.frontiers = state.counters.frontiers;
        if state.poisoned
            || state.k != 0
            || state.p0 != state.watermark()
            || saved.cursor != 0
            || saved.counter != 0
            || !saved.requeue.is_empty()
            || !saved.deferred.is_empty()
            || saved.adaptive.is_some_and(|saved| !saved.is_initial())
            || !state.in_flight.is_empty()
            || state.edges.runs() != 0
            || !state.edges.log().is_empty()
            || state.anchors.len() != 0
            || state.merged_view.len() != 0
            || !state.frontier_counts.is_empty()
            || state.counters != expected
            || state.nodes.iter().any(|&flags| flags != 0)
            || (0..state.watermark())
                .any(|id| state.ledger.get(id) != Ok(Entry6::Pending(Default::default())))
        {
            return Err("epoch incomplete admission has issued or noninitial state".into());
        }
        Ok(())
    }

    pub fn check_admission<const N: usize>(&self, state: &EpochState<N>) -> Result<(), String> {
        if self.admission_ready || self.replay_pending {
            return Err("epoch admission readiness transition is not fresh".into());
        }
        Self::validate_admission(state, self.checkpoint_snapshot())?;
        Ok(())
    }

    pub fn complete_admission<const N: usize>(
        &mut self,
        state: &EpochState<N>,
    ) -> Result<(), String> {
        self.check_admission(state)?;
        self.admission_ready = true;
        Ok(())
    }

    pub fn new() -> Self {
        Self {
            cursor: 0,
            requeue: VecDeque::new(),
            deferred: VecDeque::new(),
            counter: 0,
            session: 1,
            replay_pending: false,
            admission_ready: true,
            adaptive: None,
        }
    }

    /// Fresh-only policy configuration. Restore carries its authenticated state
    /// through `restored`; it must never reset observations or candidate order.
    pub fn enable_adaptive(&mut self) -> Result<(), String> {
        if self.cursor != 0
            || self.counter != 0
            || self.replay_pending
            || !self.requeue.is_empty()
            || !self.deferred.is_empty()
            || self.adaptive.is_some()
        {
            return Err("adaptive dispatch must be configured once before fresh issuance".into());
        }
        self.adaptive = Some(adaptive::Saved::default());
        Ok(())
    }

    /// Observe only verified entries whose P3 publication succeeded. New IDs
    /// are the actual cut aggregate, distributed in deterministic sequence order
    /// as a bucket estimate, not claimed parent-specific mathematical provenance.
    pub fn observe_completed<const N: usize>(
        &mut self,
        state: &EpochState<N>,
        observations: &[(u32, f64, u64)],
        new_domains: u64,
    ) {
        let Some(adaptive) = self.adaptive.as_mut() else {
            return;
        };
        if observations.is_empty() {
            return;
        }
        let count = observations.len() as u64;
        let share = new_domains / count;
        let remainder = new_domains % count;
        for (index, &(parent, seconds, reuse)) in observations.iter().enumerate() {
            if let Some(image) = state.store.domains.get(parent as usize) {
                adaptive.observe(
                    image,
                    seconds,
                    share + u64::from((index as u64) < remainder),
                    reuse,
                );
            }
        }
    }

    /// All restored arrays/reservations have been authenticated first. The
    /// non-cloneable session was durably reserved under the publisher lock.
    /// Replay follows original sequence order, not the BTree's parent-ID order.
    pub fn restored<const N: usize>(
        session: Session,
        cursor: u32,
        requeue: VecDeque<u32>,
        deferred: VecDeque<u32>,
        adaptive: Option<adaptive::Saved>,
        admission_ready: bool,
        state: &mut EpochState<N>,
    ) -> Result<(Self, Vec<Job<N>>), String> {
        let number = session.number();
        if number == 0
            || number >= SESSION_LIMIT
            || state.in_flight.len() as u64 >= SEQUENCE_COUNTER_LIMIT
        {
            return Err("epoch restored sequence range".into());
        }
        let mut jobs = Vec::new();
        jobs.try_reserve_exact(state.in_flight.len())
            .map_err(|_| "epoch replay allocation")?;
        for (&id, meta) in &state.in_flight {
            let Some(image) = state.store.domains.get(id as usize).copied() else {
                return Err("epoch replay image range".into());
            };
            let Ok(Entry6::Reserved(counters)) = state.ledger.get(id) else {
                return Err("epoch replay ID is not Reserved".into());
            };
            if meta.v0 > state.k || meta.seq >> 40 >= number {
                return Err("epoch replay does not have a fresh session".into());
            }
            jobs.push(Job {
                seq: meta.seq,
                parent: id,
                v0: state.k,
                attempts: counters.attempts,
                flags: super::rescue::job_flags(state, id),
                image,
            });
        }
        jobs.sort_unstable_by_key(|job| job.seq);
        let mut dispatch = Self {
            cursor,
            requeue,
            deferred,
            counter: 0,
            session: number,
            replay_pending: !jobs.is_empty(),
            admission_ready,
            adaptive,
        };
        for job in &mut jobs {
            dispatch.counter += 1; // Bounded by the preflighted job count above.
            job.seq = (number << 40) | dispatch.counter;
            *state
                .in_flight
                .get_mut(&job.parent)
                .expect("validated replay parent") = JobMeta {
                seq: job.seq,
                v0: job.v0,
                published_len: state.watermark(),
            };
        }
        Ok((dispatch, jobs))
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

    pub fn checkpoint_snapshot(&self) -> DispatchSnapshot<'_> {
        DispatchSnapshot {
            session: self.session,
            counter: self.counter,
            cursor: self.cursor,
            requeue: &self.requeue,
            deferred: &self.deferred,
            adaptive: self.adaptive.as_ref(),
        }
    }

    fn job<const N: usize>(&mut self, state: &mut EpochState<N>, id: u32) -> Job<N> {
        self.counter = self
            .counter
            .checked_add(1)
            .filter(|&next| next < SEQUENCE_COUNTER_LIMIT)
            .expect("refill preflighted sequence capacity");
        let seq = (self.session << 40) | self.counter;
        state.in_flight.insert(
            id,
            JobMeta {
                seq,
                v0: state.k,
                published_len: state.watermark(),
            },
        );
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
            flags: super::rescue::job_flags(state, id),
            image: state.store.domains[id as usize],
        }
    }

    /// Up to `want` jobs: requeued first (at most half of the refill while
    /// Pending IDs exist, the age bound), at least one deferred ID per refill
    /// while any is waiting, then the lowest Pending IDs by the cursor.
    pub fn refill<const N: usize>(&mut self, state: &mut EpochState<N>, want: usize) -> Refill<N> {
        assert!(want > 0, "epoch refill requires a positive dispatch budget");
        if !self.admission_ready {
            return Refill::Stalled;
        }
        if self.replay_pending {
            if !state.in_flight.is_empty() {
                return Refill::Stalled;
            }
            self.replay_pending = false;
        }
        if state.in_flight.is_empty() && state.pending_or_reserved() == 0 {
            return Refill::Drained;
        }
        let remaining = (SEQUENCE_COUNTER_LIMIT - 1)
            .checked_sub(self.counter)
            .unwrap_or(0);
        if remaining == 0 {
            return Refill::SequenceExhausted;
        }
        let want = want.min(usize::try_from(remaining).unwrap_or(usize::MAX));
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
        while jobs.len() < want {
            let next = if let Some(adaptive) = self.adaptive.as_mut() {
                // Publication may have resolved a buffered obligation. Retire
                // only its scheduling hint; the authoritative ledger is kept.
                adaptive
                    .candidates
                    .retain(|&id| matches!(state.ledger.get(id), Ok(Entry6::Pending(_))));
                while adaptive.candidates.len() < adaptive::WINDOW && self.cursor < watermark {
                    let id = self.cursor;
                    self.cursor += 1;
                    if matches!(state.ledger.get(id), Ok(Entry6::Pending(_))) {
                        adaptive.candidates.push(id);
                    }
                }
                adaptive.choose(&state.store.domains)
            } else {
                let mut next = None;
                while self.cursor < watermark {
                    let id = self.cursor;
                    self.cursor += 1;
                    if matches!(state.ledger.get(id), Ok(Entry6::Pending(_))) {
                        next = Some(id);
                        break;
                    }
                }
                next
            };
            if let Some(id) = next {
                state
                    .ledger
                    .apply(id, Transition::T2Reserve)
                    .expect("T2 on a Pending ID");
                state.counters.dispatched += 1;
                jobs.push(self.job(state, id));
            } else {
                break;
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn adaptive_state(count: u64) -> EpochState<1> {
        use super::super::super::queue::{Domain, Phase};
        let mut state = EpochState::<1>::new(1000, 1000, 1000);
        for point in 0..count {
            super::super::admit_initial(
                &mut state,
                &Domain {
                    phase: Phase::Apply,
                    owner: [true],
                    lower: vec![point],
                    upper: vec![Some(point)],
                    rank: None,
                    powers: Default::default(),
                },
            )
            .unwrap();
        }
        state
    }

    #[test]
    fn adaptive_window_preserves_every_pending_id_and_bounds_buffer() {
        let mut state = adaptive_state(200);
        let mut dispatch = Dispatch::new();
        dispatch.enable_adaptive().unwrap();
        let mut issued = Vec::new();
        while issued.len() < 200 {
            let Refill::Jobs(jobs) = dispatch.refill(&mut state, 7) else {
                panic!("pending work");
            };
            issued.extend(jobs.iter().map(|job| job.parent));
            let saved = dispatch.checkpoint_snapshot();
            assert!(saved.adaptive.unwrap().candidates.len() <= adaptive::WINDOW);
            assert!(saved.adaptive.unwrap().valid(saved.cursor));
        }
        assert_eq!(issued, (0..200).collect::<Vec<_>>());
        assert_eq!(
            state
                .ledger
                .counts()
                .get(super::super::ledger6::Tag::Pending),
            0
        );
        assert_eq!(state.in_flight.len(), 200);
    }

    #[test]
    fn adaptive_keeps_deferred_then_retry_priority_and_refuses_late_enable() {
        let mut state = adaptive_state(70);
        let mut dispatch = Dispatch::new();
        dispatch.enable_adaptive().unwrap();
        assert!(dispatch.enable_adaptive().is_err());
        let Refill::Jobs(first) = dispatch.refill(&mut state, 2) else {
            panic!("initial jobs");
        };
        assert_eq!(
            first.iter().map(|job| job.parent).collect::<Vec<_>>(),
            [0, 1]
        );
        for (id, attempts) in [(0, 1), (1, 2)] {
            state.in_flight.remove(&id);
            state
                .ledger
                .apply(
                    id,
                    Transition::T7Requeue {
                        d_attempts: attempts,
                        d_guard: 1,
                        last_err: 4,
                    },
                )
                .unwrap();
            dispatch.requeue(id, attempts);
        }
        let Refill::Jobs(next) = dispatch.refill(&mut state, 4) else {
            panic!("refill");
        };
        assert_eq!(
            next.iter().map(|job| job.parent).collect::<Vec<_>>(),
            [1, 0, 2, 3]
        );
        let mut fifo = Dispatch::new();
        let mut other = adaptive_state(1);
        assert!(matches!(fifo.refill(&mut other, 1), Refill::Jobs(_)));
        assert!(fifo.enable_adaptive().is_err());
    }

    #[test]
    fn candidate_inventory_is_required_to_authorize_pending_below_cursor() {
        let mut state = adaptive_state(70);
        let mut dispatch = Dispatch::new();
        dispatch.enable_adaptive().unwrap();
        assert!(matches!(dispatch.refill(&mut state, 1), Refill::Jobs(_)));
        let validate = |dispatch: &Dispatch| {
            super::super::checkpoint::Reservations {
                ledger: &state.ledger,
                nodes: &state.nodes,
                k: state.k,
                in_flight: &state.in_flight,
                dispatch: dispatch.checkpoint_snapshot(),
                lockstep_b: 16,
            }
            .validate()
        };
        validate(&dispatch).unwrap();
        dispatch.adaptive.as_mut().unwrap().candidates.remove(0);
        assert!(validate(&dispatch).is_err());
    }

    #[test]
    fn zero_budget_refill_rejects_before_consuming_deferred_work() {
        let mut state = EpochState::<2>::new(10, 10, 10);
        let mut dispatch = Dispatch::new();
        dispatch.requeue(7, 2);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dispatch.refill(&mut state, 0)
        }));
        assert!(result.is_err());
        assert_eq!(dispatch.deferred.iter().copied().collect::<Vec<_>>(), [7]);
        assert_eq!(dispatch.counter, 0);
        assert!(state.in_flight.is_empty());
        assert!(state.ledger.words().is_empty());
    }

    #[test]
    fn sequence_limit_is_checked_before_reservation_or_queue_mutation() {
        use super::super::super::queue::{Domain, Phase};
        let mut state = EpochState::<1>::new(10, 10, 10);
        for point in 0..2 {
            super::super::admit_initial(
                &mut state,
                &Domain {
                    phase: Phase::Apply,
                    owner: [true],
                    lower: vec![point],
                    upper: vec![Some(point)],
                    rank: None,
                    powers: Default::default(),
                },
            )
            .unwrap();
        }
        let mut dispatch = Dispatch::new();
        dispatch.counter = SEQUENCE_COUNTER_LIMIT - 2;
        let Refill::Jobs(jobs) = dispatch.refill(&mut state, 16) else {
            panic!("last legal sequence")
        };
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].seq, (1 << 40) | (SEQUENCE_COUNTER_LIMIT - 1));
        let words = state.ledger.words().to_vec();
        let cursor = dispatch.cursor;
        assert!(matches!(
            dispatch.refill(&mut state, 16),
            Refill::SequenceExhausted
        ));
        assert_eq!(state.ledger.words(), words);
        assert_eq!(dispatch.cursor, cursor);
        assert_eq!(state.in_flight.len(), 1);
        assert!(matches!(state.ledger.get(1).unwrap(), Entry6::Pending(_)));
        // Even a corrupted scalar beyond the range never wraps the OR bits.
        dispatch.counter = u64::MAX;
        assert!(matches!(
            dispatch.refill(&mut state, 16),
            Refill::SequenceExhausted
        ));
    }
}
