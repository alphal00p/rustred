//! Separate bounded physical occupancy from unpublished owner FIFO obligations.
use std::collections::{BTreeSet, VecDeque};

use super::*;

#[derive(Clone, Copy)]
pub(super) struct Head<const N: usize> {
    pub key: Key<N>,
    pub local_id: usize,
    pub ticket: usize,
}

pub(super) struct Jobs<const N: usize> {
    /// At most the native worker limit; retained results hold no extra Arc here.
    pub active: Vec<Job<N>>,
    /// Includes retained results and failed dispatches awaiting cleanup.
    pub pending: BTreeMap<usize, (Key<N>, usize)>,
    owners: BTreeMap<Key<N>, VecDeque<(usize, usize)>>,
    last_polled: Option<Key<N>>,
}

impl<const N: usize> Jobs<N> {
    pub fn new() -> Self {
        Self {
            active: Vec::new(),
            pending: BTreeMap::new(),
            owners: BTreeMap::new(),
            last_polled: None,
        }
    }

    pub fn register(&mut self, job: &Job<N>) {
        assert!(
            self.pending
                .insert(job.ticket, (job.key, job.local_id))
                .is_none()
        );
        let queue = self.owners.entry(job.key).or_default();
        debug_assert!(queue.back().is_none_or(|&(id, _)| id < job.local_id));
        queue.push_back((job.local_id, job.ticket));
    }

    /// Head lookup is O(owners), independent of retained queue lengths.
    pub fn protected(&self) -> BTreeSet<usize> {
        self.owners
            .values()
            .filter_map(|q| q.front().map(|&(_, t)| t))
            .collect()
    }

    pub fn eligible(&self, walk: &Walk<N>) -> Vec<Head<N>> {
        self.owners
            .iter()
            .filter_map(|(&key, q)| {
                let &(local_id, ticket) = q.front()?;
                (walk.buckets[&key].state.queue.next == local_id).then_some(Head {
                    key,
                    local_id,
                    ticket,
                })
            })
            .collect()
    }

    /// At most one chunk per owner and at most W coordinator-owned chunks.
    pub fn poll_batch(&mut self, mut heads: Vec<Head<N>>, width: usize) -> Vec<Head<N>> {
        if let Some(previous) = self.last_polled {
            let split = heads.partition_point(|head| head.key <= previous);
            heads.rotate_left(split);
        }
        heads.truncate(width);
        if let Some(last) = heads.last() {
            self.last_polled = Some(last.key);
        }
        heads
    }

    pub fn release_slots(&mut self, tickets: &[usize]) {
        // Both lists have at most W elements, never reservoir-sized scans.
        self.active.retain(|job| !tickets.contains(&job.ticket));
    }

    pub fn published(&mut self, head: Head<N>) {
        assert_eq!(
            self.pending.remove(&head.ticket),
            Some((head.key, head.local_id))
        );
        let queue = self.owners.get_mut(&head.key).expect("published owner");
        assert_eq!(queue.pop_front(), Some((head.local_id, head.ticket)));
        if queue.is_empty() {
            self.owners.remove(&head.key);
        }
        self.release_slots(&[head.ticket]);
    }
}
