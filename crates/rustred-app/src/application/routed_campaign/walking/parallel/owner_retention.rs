//! Reuse physical slots without changing any owner's publication responsibility.
use std::collections::BTreeSet;

use super::*;

impl<const N: usize> Pool<N> {
    /// Move only successful, fully flushed, non-head results into the shared
    /// bounded store. Tickets remain pending; this is neither polling nor commit.
    /// Only physical slots are scanned, never the retained-result reservoir.
    pub fn reclaim_completed_except(&self, heads: &BTreeSet<usize>) -> Vec<usize> {
        let mut state = self.lock();
        let mut reclaimed = Vec::new();
        if self.stop.load(Ordering::Acquire) || state.shutdown {
            return reclaimed;
        }
        if reclaimed.try_reserve_exact(state.slots.len()).is_err() {
            // Match storage allocation fallback: wait must not keep reporting
            // a fitting completion which reclamation cannot actually detach.
            state.escrow.reserve_failed = true;
            return reclaimed;
        }
        for index in 0..state.slots.len() {
            let Some(id) = state.slots[index].id.filter(|id| !heads.contains(id)) else {
                continue;
            };
            let Some(charge) = Escrow::charge(&state.slots[index]) else {
                continue;
            };
            if !state.escrow.reserve(charge) {
                continue;
            }
            let State { slots, escrow, .. } = &mut *state;
            escrow.insert(id, &mut slots[index], charge);
            reclaimed.push(id);
        }
        reclaimed
    }

    /// Wait for a publishable head OR a completed non-head which can release a
    /// physical slot. An unfinished later chunk or a full retained store cannot
    /// wake this predicate into a busy loop. Both tests use the pool mutex, so a
    /// finish between polling and waiting cannot lose its notification.
    pub fn wait_for_owner_progress(&self, heads: &BTreeSet<usize>) -> bool {
        if heads.is_empty() {
            return false;
        }
        let ready = |state: &State<N>| {
            heads.iter().any(|&id| state.escrow.contains(id))
                || state.slots.iter().any(|slot| {
                    slot.id.is_some_and(|id| {
                        if heads.contains(&id) {
                            slot.chunk.is_some() || slot.finished.is_some()
                        } else {
                            Escrow::charge(slot).is_some_and(|charge| state.escrow.fits(charge))
                        }
                    })
                })
        };
        let waiting = |state: &mut State<N>| {
            !self.stop.load(Ordering::Acquire) && !state.shutdown && !ready(state)
        };
        let (state, _) = self
            .changed
            .wait_timeout_while(self.lock(), Duration::from_millis(100), waiting)
            .unwrap_or_else(|error| error.into_inner());
        !self.stop.load(Ordering::Acquire) && !state.shutdown && ready(&state)
    }
}
