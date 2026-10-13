//! Stable guard membership, without a second owned coefficient payload.
use std::collections::{HashMap, hash_map::RandomState};
use std::hash::BuildHasher;

use crate::algebra::Coefficient;
use crate::reduction::ReductionError;

const END: usize = usize::MAX;

/// The cache keeps its original ordered Vec. This temporary index owns only
/// hashes and vector indices, so Vec growth cannot invalidate its references.
/// One chain link per retained guard plus at most one head per distinct hash
/// bounds scratch metadata linearly in the retained guard count, independently
/// of how many duplicate guards are visited. Equality, never a hash alone,
/// decides membership; no guard normalization or algebra occurs here.
pub(crate) struct GuardUnion<'a, S = RandomState> {
    output: &'a mut Vec<Coefficient>,
    heads: HashMap<u64, usize>,
    previous: Vec<usize>,
    hasher: S,
}

impl<'a> GuardUnion<'a> {
    pub(crate) fn try_new(output: &'a mut Vec<Coefficient>) -> Result<Self, ReductionError> {
        Self::with_hasher(output, RandomState::new())
    }
}

impl<'a, S: BuildHasher> GuardUnion<'a, S> {
    fn with_hasher(output: &'a mut Vec<Coefficient>, hasher: S) -> Result<Self, ReductionError> {
        let mut heads = HashMap::new();
        let mut previous = Vec::new();
        heads
            .try_reserve(output.len())
            .map_err(|_| allocation(output.len()))?;
        previous
            .try_reserve_exact(output.len())
            .map_err(|_| allocation(output.len()))?;
        for (index, value) in output.iter().enumerate() {
            let hash = hasher.hash_one(value);
            previous.push(heads.insert(hash, index).unwrap_or(END));
        }
        Ok(Self {
            output,
            heads,
            previous,
            hasher,
        })
    }

    pub(crate) fn extend(&mut self, conditions: &[Coefficient]) -> Result<(), ReductionError> {
        self.extend_with(conditions, |_| Ok(()))
    }

    /// Admit payload for each genuinely new guard before retaining its clone.
    /// Metadata allocation is preflighted first; duplicate guards need no
    /// second payload grant. The caller owns rollback/RAII of that grant.
    pub(crate) fn extend_with(
        &mut self,
        conditions: &[Coefficient],
        mut admit: impl FnMut(&Coefficient) -> Result<(), ReductionError>,
    ) -> Result<(), ReductionError> {
        for condition in conditions {
            let hash = self.hasher.hash_one(condition);
            let head = self.heads.get(&hash).copied().unwrap_or(END);
            let mut index = head;
            let mut found = false;
            while index != END {
                if self.output[index] == *condition {
                    found = true;
                    break;
                }
                index = self.previous[index];
            }
            if found {
                continue;
            }
            let index = self.output.len();
            let requested =
                index
                    .checked_add(1)
                    .ok_or(ReductionError::CacheResourceCountOverflow {
                        resource: "routed guard membership indices",
                    })?;
            // Admit all metadata growth before retaining the new coefficient.
            // A colliding hash needs only another chain link, not another head.
            if head == END {
                self.heads
                    .try_reserve(1)
                    .map_err(|_| allocation(requested))?;
            }
            self.previous
                .try_reserve(1)
                .map_err(|_| allocation(requested))?;
            self.output
                .try_reserve(1)
                .map_err(|_| ReductionError::AllocationFailure {
                    resource: "routed merged guards",
                    requested,
                })?;
            admit(condition)?;
            self.output.push(condition.clone());
            self.previous.push(head);
            self.heads.insert(hash, index);
        }
        Ok(())
    }
}

fn allocation(requested: usize) -> ReductionError {
    ReductionError::AllocationFailure {
        resource: "routed guard membership index",
        requested,
    }
}

#[cfg(test)]
mod tests;
