//! Select one bounded stream chunk; admission remains in the shared coordinator.
use super::*;
use std::collections::BTreeSet;

#[derive(Default)]
pub(super) struct ReadyStreams {
    pending: BTreeSet<usize>,
    last: Option<usize>,
}
impl ReadyStreams {
    pub fn dispatched(&mut self, raw: usize) {
        assert!(self.pending.insert(raw));
    }
    pub fn finished(&mut self, raw: usize) {
        assert!(self.pending.remove(&raw));
    }
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
    pub fn poll<const N: usize>(&mut self, pool: &parallel::Pool<N>) -> Option<(usize, Poll<N>)> {
        use std::ops::Bound::{Excluded, Unbounded};
        let last = self.last;
        let after = last.map_or(Unbounded, Excluded);
        let ids = self.pending.range((after, Unbounded)).copied().chain(
            self.pending
                .iter()
                .copied()
                .take_while(|id| last.is_some_and(|last| *id <= last)),
        );
        for raw in ids {
            let result = pool.poll(raw);
            if !matches!(result, Poll::Waiting) {
                self.last = Some(raw);
                return Some((raw, result));
            }
        }
        None
    }
    pub fn wait<const N: usize>(&self, pool: &parallel::Pool<N>) {
        pool.wait_for_owner_progress(&self.pending);
    }
}
