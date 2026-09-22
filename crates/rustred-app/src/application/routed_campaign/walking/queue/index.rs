//! Necessary aggregate filters; native summary inclusion remains authoritative.
//! Groups contain lookup candidates only, never own queued obligations.

use rustred::solver::DomainPowerSummary;
#[cfg(test)]
use std::cell::Cell;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Upper {
    Finite(u128),
    Infinity,
}

impl Upper {
    fn contains(self, other: Self) -> bool {
        match (self, other) {
            (Self::Infinity, _) => true,
            (Self::Finite(a), Self::Finite(b)) => a >= b,
            (Self::Finite(_), Self::Infinity) => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Lower {
    NegativeInfinity,
    Finite(i128),
}

impl Lower {
    fn contains(self, other: Self) -> bool {
        match (self, other) {
            (Self::NegativeInfinity, _) => true,
            (Self::Finite(a), Self::Finite(b)) => a <= b,
            (Self::Finite(_), Self::NegativeInfinity) => false,
        }
    }
}

/// Tight native extrema, not optional raw input labels or finite sentinels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Signature {
    Empty,
    Nonempty {
        positive: Upper,
        numerator: Upper,
        difference: Lower,
    },
}

impl Signature {
    pub(super) fn of<const N: usize>(summary: &DomainPowerSummary<N>) -> Self {
        summary
            .extrema()
            .map_or(Self::Empty, |extrema| Self::Nonempty {
                positive: extrema
                    .positive_power()
                    .1
                    .map_or(Upper::Infinity, Upper::Finite),
                numerator: extrema
                    .numerator_rank()
                    .1
                    .map_or(Upper::Infinity, Upper::Finite),
                difference: extrema
                    .power_difference()
                    .0
                    .map_or(Lower::NegativeInfinity, Lower::Finite),
            })
    }

    /// A necessary condition only: Q subset C implies this test for (C,Q).
    fn may_contain(self, other: Self) -> bool {
        match (self, other) {
            (_, Self::Empty) => true,
            (Self::Empty, Self::Nonempty { .. }) => false,
            (
                Self::Nonempty {
                    positive: a,
                    numerator: r,
                    difference: d,
                },
                Self::Nonempty {
                    positive: b,
                    numerator: s,
                    difference: e,
                },
            ) => a.contains(b) && r.contains(s) && d.contains(e),
        }
    }
}

struct Group {
    signature: Signature,
    /// Increasing immutable admission IDs; no duplicate membership.
    ids: Vec<usize>,
}

/// Prepared insertion owns new storage until all queue preflights succeed.
pub(super) struct Insertion {
    signature: Signature,
    new_group: Option<Group>,
}

#[derive(Default)]
pub(super) struct AggregateIndex {
    groups: Vec<Group>,
    positions: HashMap<Signature, usize>,
    live: usize,
    #[cfg(test)]
    work: Cell<FilterWork>,
}

/// Test-replay instrumentation only; no additional production per-group work.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct FilterWork {
    pub groups_visited: usize,
    pub groups_rejected: usize,
}

impl AggregateIndex {
    pub(super) fn find(
        &self,
        signature: Signature,
        mut contains: impl FnMut(usize) -> Result<bool, &'static str>,
    ) -> Result<Option<usize>, &'static str> {
        let mut best = None;
        for group in &self.groups {
            let eligible = group.signature.may_contain(signature);
            #[cfg(test)]
            self.record_group(eligible);
            if !eligible {
                continue;
            }
            for &id in &group.ids {
                // Group order may change during retirement. Minimum admission
                // ID, not hash/vector traversal order, determines the result.
                if best.is_some_and(|best| id >= best) {
                    break;
                }
                if contains(id)? {
                    best = Some(id);
                    break;
                }
            }
        }
        Ok(best)
    }

    pub(super) fn prepare(&mut self, signature: Signature) -> Result<Insertion, &'static str> {
        self.prepare_with(signature, || Ok(()))
    }

    // The inlined no-op checkpoint lets tests fault-inject each reservation
    // without changing the production storage policy or allocating huge memory.
    fn prepare_with(
        &mut self,
        signature: Signature,
        mut checkpoint: impl FnMut() -> Result<(), &'static str>,
    ) -> Result<Insertion, &'static str> {
        self.live
            .checked_add(1)
            .ok_or("candidate index count overflow")?;
        let new_group = if let Some(&position) = self.positions.get(&signature) {
            checkpoint()?;
            self.groups[position]
                .ids
                .try_reserve(1)
                .map_err(|_| "aggregate candidate ID allocation")?;
            None
        } else {
            checkpoint()?;
            self.groups
                .try_reserve(1)
                .map_err(|_| "aggregate group allocation")?;
            checkpoint()?;
            self.positions
                .try_reserve(1)
                .map_err(|_| "aggregate group index allocation")?;
            let mut ids = Vec::new();
            checkpoint()?;
            ids.try_reserve(1)
                .map_err(|_| "aggregate candidate ID allocation")?;
            Some(Group { signature, ids })
        };
        Ok(Insertion {
            signature,
            new_group,
        })
    }

    /// Upper bound used to preflight every fallible counter before mutation.
    /// Only groups eligible for Q containing C can need a reverse comparison.
    pub(super) fn maintenance_len(&self, signature: Signature) -> Result<usize, &'static str> {
        self.groups
            .iter()
            .filter(|group| {
                let eligible = signature.may_contain(group.signature);
                #[cfg(test)]
                self.record_group(eligible);
                eligible
            })
            .try_fold(0_usize, |count, group| {
                count
                    .checked_add(group.ids.len())
                    .ok_or("candidate index count overflow")
            })
    }

    /// Infallible after queue counter/storage preflight. Preserve the insertion
    /// signature's reserved group even if it becomes empty; remove other empty
    /// groups so historical signatures do not accumulate in the hot scan.
    pub(super) fn retire(
        &mut self,
        insertion: &Insertion,
        mut contains: impl FnMut(usize) -> bool,
    ) -> usize {
        let mut removed = 0;
        let mut position = 0;
        while position < self.groups.len() {
            let eligible = insertion
                .signature
                .may_contain(self.groups[position].signature);
            #[cfg(test)]
            self.record_group(eligible);
            let group = &mut self.groups[position];
            if eligible {
                let previous = group.ids.len();
                group.ids.retain(|&id| !contains(id));
                removed += previous - group.ids.len();
            }
            if group.ids.is_empty() && group.signature != insertion.signature {
                let key = group.signature;
                self.groups.swap_remove(position);
                self.positions.remove(&key);
                if let Some(moved) = self.groups.get(position) {
                    *self
                        .positions
                        .get_mut(&moved.signature)
                        .expect("indexed group") = position;
                }
            } else {
                position += 1;
            }
        }
        self.live -= removed;
        removed
    }

    pub(super) fn insert(&mut self, mut insertion: Insertion, id: usize) {
        if let Some(mut group) = insertion.new_group.take() {
            group.ids.push(id);
            self.positions
                .insert(insertion.signature, self.groups.len());
            self.groups.push(group);
        } else {
            let position = self.positions[&insertion.signature];
            let ids = &mut self.groups[position].ids;
            debug_assert!(ids.last().is_none_or(|&old| old < id));
            ids.push(id);
        }
        self.live += 1; // checked by prepare before any retirement
    }

    #[cfg(test)]
    pub(super) fn ids(&self) -> Vec<usize> {
        let mut ids: Vec<_> = self
            .groups
            .iter()
            .flat_map(|group| group.ids.iter().copied())
            .collect();
        ids.sort_unstable();
        ids
    }

    #[cfg(test)]
    pub(super) fn groups(&self) -> usize {
        self.groups.len()
    }

    #[cfg(test)]
    fn record_group(&self, eligible: bool) {
        let old = self.work.get();
        self.work.set(FilterWork {
            groups_visited: old.groups_visited.checked_add(1).expect("test probe count"),
            groups_rejected: old
                .groups_rejected
                .checked_add(usize::from(!eligible))
                .expect("test rejection count"),
        });
    }

    #[cfg(test)]
    pub(super) fn work(&self) -> FilterWork {
        self.work.get()
    }
}

#[cfg(test)]
mod tests;
