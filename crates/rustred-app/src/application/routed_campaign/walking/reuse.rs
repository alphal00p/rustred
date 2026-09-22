//! Optional job-local proof of an earlier ordered scheduling request.
//!
//! Continue means accepted into this job's stream, not necessarily committed.
//! The sole publisher cannot commit a later marker until its preceding Admit
//! succeeded; a failed admission stops that prefix. There is no cross-job cache,
//! replay, token table, completed-coverage claim or native-algebra shortcut.
use std::collections::HashSet;
use std::ops::ControlFlow;

use super::inspection::{Effect, Event};
use super::queue::{Domain, Phase};

pub(super) const MAX_KEYS: usize = 4096;
pub(super) const MAX_KEY_BYTES: usize = 2 * 1024 * 1024;

/// Fixed arrays avoid cloning coordinate Vecs for cache storage. Existing
/// native-to-Event conversion still allocates the ordinary owned descriptor.
#[derive(PartialEq, Eq, Hash)]
struct Key<const N: usize> {
    phase: Phase,
    owner: [bool; N],
    lower: [u64; N],
    upper: [Option<u64>; N],
    rank: Option<u32>,
}
impl<const N: usize> Key<N> {
    fn from_domain(d: &Domain<N>) -> Option<Self> {
        Some(Self {
            phase: d.phase,
            owner: d.owner,
            lower: d.lower.as_slice().try_into().ok()?,
            upper: d.upper.as_slice().try_into().ok()?,
            rank: d.rank,
        })
    }
}

pub(super) struct Cache<const N: usize> {
    keys: HashSet<Key<N>>,
    max_keys: usize,
    max_bytes: usize,
}
impl<const N: usize> Cache<N> {
    pub fn new(enabled: bool) -> Self {
        Self {
            keys: HashSet::new(),
            max_keys: if enabled { MAX_KEYS } else { 0 },
            max_bytes: MAX_KEY_BYTES,
        }
    }
    pub fn forward(
        &mut self,
        event: Event<N>,
        emit: &mut (impl FnMut(Event<N>) -> ControlFlow<()> + ?Sized),
    ) -> ControlFlow<()> {
        // Only count-one native admissions can establish a cache entry. Never
        // cache a frontier, optional diagnostic, stopped emit, or counted run.
        let key = match &event.effect {
            Effect::Admit { domain, .. } if event.count == 1 && self.max_keys != 0 => {
                Key::from_domain(domain)
            }
            _ => None,
        };
        if let Some(key) = key {
            if self.keys.contains(&key) {
                let Effect::Admit {
                    successor,
                    conditional,
                    ..
                } = event.effect
                else {
                    unreachable!()
                };
                return emit(Event::one(Effect::KnownReuse {
                    successor,
                    conditional,
                }));
            }
            let accepted = emit(event);
            if accepted.is_continue()
                && self.keys.len() < self.max_keys
                && self
                    .keys
                    .len()
                    .checked_add(1)
                    .and_then(|n| n.checked_mul(size_of::<Key<N>>()))
                    .is_some_and(|n| n <= self.max_bytes)
                && self.keys.try_reserve(1).is_ok()
            {
                self.keys.insert(key);
            }
            accepted
        } else {
            emit(event)
        }
    }
}

// MAX_KEY_BYTES counts logical keys, not HashSet spare capacity, allocation
// overhead, process RSS, native scratch, or the unchanged converted descriptor.
#[cfg(test)]
mod tests;
