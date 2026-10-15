//! V3's coordinator-side finite-case gate. Ordinary descendants may pass a
//! blocked initial query, but that query remains Pending at/above the durable
//! cursor. At most one initial query is issued until P3 retires it.
use super::*;

impl Dispatch {
    pub(super) fn refill_finite<const N: usize>(
        &mut self,
        state: &mut EpochState<N>,
        want: usize,
    ) -> Refill<N> {
        assert!(self.adaptive.is_none(), "finite v3 requires FIFO");
        let roots = state.finite_roots.as_ref().expect("finite branch").clone();
        let mut busy = state.in_flight.keys().any(|&id| roots.contains(id));
        let mut jobs = Vec::with_capacity(want);
        // Never remove a blocked finite reservation from its durable queue.
        // The queues remain authoritative across checkpoint/restart.
        for deferred in [true, false] {
            let cap = if deferred {
                1
            } else if state
                .ledger
                .counts()
                .get(super::super::ledger6::Tag::Pending)
                > 0
            {
                want / 2
            } else {
                want
            };
            while jobs.len() < cap.max(1).min(want) {
                let queue = if deferred {
                    &mut self.deferred
                } else {
                    &mut self.requeue
                };
                let Some(position) = queue.iter().position(|&id| !busy || !roots.contains(id))
                else {
                    break;
                };
                let id = queue.remove(position).expect("selected position");
                busy |= roots.contains(id);
                jobs.push(self.job(state, id));
            }
        }
        let watermark = state.watermark();
        while jobs.len() < want {
            // Advance over non-Pending IDs only. In particular, a finite root
            // withheld by the gate must never be lost behind the saved cursor.
            while self.cursor < watermark
                && !matches!(state.ledger.get(self.cursor), Ok(Entry6::Pending(_)))
            {
                self.cursor += 1;
            }
            if self.cursor >= watermark {
                break;
            }
            let id = if !busy || !roots.contains(self.cursor) {
                self.cursor
            } else {
                self.finite_scan = self.finite_scan.max(self.cursor + 1);
                let mut next = None;
                while self.finite_scan < watermark {
                    let candidate = self.finite_scan;
                    self.finite_scan += 1;
                    if !roots.contains(candidate)
                        && matches!(state.ledger.get(candidate), Ok(Entry6::Pending(_)))
                    {
                        next = Some(candidate);
                        break;
                    }
                }
                let Some(id) = next else {
                    break;
                };
                id
            };
            state
                .ledger
                .apply(id, Transition::T2Reserve)
                .expect("finite T2 on Pending");
            state.counters.dispatched += 1;
            busy |= roots.contains(id);
            jobs.push(self.job(state, id));
        }
        while jobs.len() < want {
            let choice = self
                .requeue
                .iter()
                .position(|&id| !busy || !roots.contains(id))
                .map(|position| (false, position))
                .or_else(|| {
                    self.deferred
                        .iter()
                        .position(|&id| !busy || !roots.contains(id))
                        .map(|position| (true, position))
                });
            let Some((deferred, position)) = choice else {
                break;
            };
            let id = if deferred {
                self.deferred.remove(position)
            } else {
                self.requeue.remove(position)
            }
            .expect("selected position");
            busy |= roots.contains(id);
            jobs.push(self.job(state, id));
        }
        if !jobs.is_empty() {
            Refill::Jobs(jobs)
        } else if state.in_flight.is_empty() && state.pending_or_reserved() == 0 {
            Refill::Drained
        } else {
            Refill::Stalled
        }
    }
}
