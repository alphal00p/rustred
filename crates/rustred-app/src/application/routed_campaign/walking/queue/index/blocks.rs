//! Bounded necessary coordinate filters over native tight extrema.

use rustred::solver::DomainPowerSummary;

pub(super) const BLOCK_SIZE: usize = 32;

#[derive(Clone, Copy)]
pub(in super::super) struct Coordinates<'a> {
    lower: &'a [u64],
    upper: &'a [Option<u64>],
}

impl<'a> Coordinates<'a> {
    pub(in super::super) fn of<const N: usize>(summary: &'a DomainPowerSummary<N>) -> Option<Self> {
        summary.extrema().map(|extrema| Self {
            lower: extrema.lower(),
            upper: extrema.upper(),
        })
    }
}

struct AxisEnvelope {
    min_lower: u64,
    max_lower: u64,
    min_upper: Option<u64>,
    max_upper: Option<u64>,
}

pub(super) struct Block {
    ids: [usize; BLOCK_SIZE],
    len: usize,
    /// One O(N) allocation per block, not one copy of each candidate geometry.
    /// Empty summaries have no coordinates and bypass this optional filter.
    envelope: Vec<AxisEnvelope>,
}

impl Block {
    pub(super) fn prepare(
        coordinates: Option<Coordinates<'_>>,
        checkpoint: &mut impl FnMut() -> Result<(), &'static str>,
    ) -> Result<Self, &'static str> {
        let mut envelope = Vec::new();
        if let Some(coordinates) = coordinates {
            checkpoint()?;
            envelope
                .try_reserve_exact(coordinates.lower.len())
                .map_err(|_| "coordinate block envelope allocation")?;
            envelope.extend(coordinates.lower.iter().zip(coordinates.upper).map(
                |(&lower, &upper)| AxisEnvelope {
                    min_lower: lower,
                    max_lower: lower,
                    min_upper: upper,
                    max_upper: upper,
                },
            ));
        }
        Ok(Self {
            ids: [0; BLOCK_SIZE],
            len: 0,
            envelope,
        })
    }

    pub(super) fn ids(&self) -> &[usize] {
        &self.ids[..self.len]
    }

    pub(super) fn has_room(&self) -> bool {
        self.len < BLOCK_SIZE
    }

    pub(super) fn insert(&mut self, id: usize, coordinates: Option<Coordinates<'_>>) {
        debug_assert!(self.has_room());
        debug_assert!(self.ids().last().is_none_or(|&old| old < id));
        if let Some(coordinates) = coordinates {
            // Production groups never mix empty and nonempty native summaries.
            // An absent test-only envelope simply disables the optional filter.
            debug_assert!(
                self.envelope.is_empty() || self.envelope.len() == coordinates.lower.len()
            );
            for (axis, (&lower, &upper)) in self
                .envelope
                .iter_mut()
                .zip(coordinates.lower.iter().zip(coordinates.upper))
            {
                axis.min_lower = axis.min_lower.min(lower);
                axis.max_lower = axis.max_lower.max(lower);
                axis.min_upper = match (axis.min_upper, upper) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                };
                axis.max_upper = match (axis.max_upper, upper) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    _ => None,
                };
            }
        }
        self.ids[self.len] = id;
        self.len += 1;
    }

    pub(super) fn may_contain(&self, query: Option<Coordinates<'_>>) -> bool {
        query.is_none_or(|query| {
            debug_assert!(self.envelope.is_empty() || self.envelope.len() == query.lower.len());
            self.envelope
                .iter()
                .zip(query.lower.iter().zip(query.upper))
                .all(|(axis, (&lower, &upper))| {
                    axis.min_lower <= lower && upper_contains(axis.max_upper, upper)
                })
        })
    }

    pub(super) fn may_be_contained(&self, query: Option<Coordinates<'_>>) -> bool {
        query.is_none_or(|query| {
            debug_assert!(self.envelope.is_empty() || self.envelope.len() == query.lower.len());
            self.envelope
                .iter()
                .zip(query.lower.iter().zip(query.upper))
                .all(|(axis, (&lower, &upper))| {
                    lower <= axis.max_lower && upper_contains(upper, axis.min_upper)
                })
        })
    }

    pub(super) fn retain(&mut self, mut keep: impl FnMut(usize) -> bool) {
        let mut retained = 0;
        for position in 0..self.len {
            let id = self.ids[position];
            if keep(id) {
                self.ids[retained] = id;
                retained += 1;
            }
        }
        self.len = retained;
        // Old extrema may be stale after deletion, but only outwards. Keeping
        // them can admit false positives; it cannot reject a true inclusion.
    }

    #[cfg(test)]
    pub(super) fn envelope_capacity_bytes(&self) -> usize {
        self.envelope.capacity() * std::mem::size_of::<AxisEnvelope>()
    }
}

fn upper_contains(container: Option<u64>, candidate: Option<u64>) -> bool {
    container.is_none_or(|a| candidate.is_some_and(|b| a >= b))
}
