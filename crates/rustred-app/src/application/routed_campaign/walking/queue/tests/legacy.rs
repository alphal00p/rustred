//! The pre-kernel admission index, verbatim (fable_5_1 e74d31ce `queue/index.rs`,
//! `index/blocks.rs`, the `SummarySlab` of `compact.rs` and the per-ID callbacks
//! of `queue.rs`/`prepared.rs`), read paths only. It decodes the same CP5 index
//! bytes and serves as the in-process reference of the gen-7 differential and
//! the "today's layout" arm of the per-candidate cost A/B.
use super::super::Phase;
use super::super::bits;
use super::super::compact::{CompactDomain, Query};
use super::super::index::Signature;
use rustred::solver::DomainPowerSummary;
use std::collections::HashMap;

const BLOCK_SIZE: usize = 32;

#[derive(serde::Deserialize)]
struct AxisEnvelope {
    min_lower: u64,
    max_lower: u64,
    min_upper: Option<u64>,
    max_upper: Option<u64>,
}

#[derive(serde::Deserialize)]
struct Block {
    ids: [usize; BLOCK_SIZE],
    len: usize,
    envelope: Vec<AxisEnvelope>,
}

#[derive(Clone, Copy)]
pub(super) struct Coordinates<'a> {
    lower: &'a [u64],
    upper: &'a [Option<u64>],
}

impl<'a> Coordinates<'a> {
    pub(super) fn of<const N: usize>(summary: &'a DomainPowerSummary<N>) -> Option<Self> {
        summary.extrema().map(|extrema| Self {
            lower: extrema.lower(),
            upper: extrema.upper(),
        })
    }
}

fn upper_contains(container: Option<u64>, candidate: Option<u64>) -> bool {
    container.is_none_or(|a| candidate.is_some_and(|b| a >= b))
}

impl Block {
    fn ids(&self) -> &[usize] {
        &self.ids[..self.len]
    }

    fn may_contain(&self, query: Option<Coordinates<'_>>) -> bool {
        query.is_none_or(|query| {
            self.envelope
                .iter()
                .zip(query.lower.iter().zip(query.upper))
                .all(|(axis, (&lower, &upper))| {
                    axis.min_lower <= lower && upper_contains(axis.max_upper, upper)
                })
        })
    }

    fn may_be_contained(&self, query: Option<Coordinates<'_>>) -> bool {
        query.is_none_or(|query| {
            self.envelope
                .iter()
                .zip(query.lower.iter().zip(query.upper))
                .all(|(axis, (&lower, &upper))| {
                    lower <= axis.max_lower && upper_contains(upper, axis.min_upper)
                })
        })
    }
}

#[derive(serde::Deserialize)]
struct Group {
    signature: Signature,
    blocks: Vec<Block>,
    #[allow(dead_code)]
    live: usize,
}

#[derive(serde::Deserialize)]
pub(super) struct AggregateIndex {
    groups: Vec<Group>,
    #[allow(dead_code)]
    live: usize,
}

impl AggregateIndex {
    pub(super) fn find_controlled(
        &self,
        signature: Signature,
        coordinates: Option<Coordinates<'_>>,
        first_id: usize,
        mut checkpoint: impl FnMut() -> Result<(), &'static str>,
        mut contains: impl FnMut(usize) -> Result<bool, &'static str>,
    ) -> Result<Option<usize>, &'static str> {
        let mut best = None;
        for group in &self.groups {
            checkpoint()?;
            let eligible = group.signature.may_contain(signature);
            if !eligible {
                continue;
            }
            for block in &group.blocks {
                checkpoint()?;
                let ids = block.ids();
                if ids.last().is_none_or(|&id| id < first_id) {
                    continue;
                }
                if best.is_some_and(|best| ids[0] >= best) {
                    break;
                }
                let eligible = block.may_contain(coordinates);
                if !eligible {
                    continue;
                }
                let start = ids.partition_point(|&id| id < first_id);
                for &id in &ids[start..] {
                    if best.is_some_and(|best| id >= best) {
                        break;
                    }
                    if contains(id)? {
                        best = Some(id);
                        break;
                    }
                }
            }
        }
        Ok(best)
    }

    pub(super) fn collect_contained(
        &self,
        signature: Signature,
        coordinates: Option<Coordinates<'_>>,
        limit: usize,
        mut checkpoint: impl FnMut() -> Result<(), &'static str>,
        mut contains: impl FnMut(usize) -> Result<bool, &'static str>,
    ) -> Result<Vec<usize>, &'static str> {
        let mut contained = Vec::new();
        for group in &self.groups {
            checkpoint()?;
            let eligible = signature.may_contain(group.signature);
            if !eligible {
                continue;
            }
            for block in &group.blocks {
                checkpoint()?;
                let eligible = block.may_be_contained(coordinates);
                if !eligible {
                    continue;
                }
                for &id in block.ids() {
                    if contains(id)? {
                        if contained.len() >= limit {
                            return Err("prepared retirement set limit");
                        }
                        contained
                            .try_reserve(1)
                            .map_err(|_| "prepared retirement set allocation")?;
                        contained.push(id);
                    }
                }
            }
        }
        contained.sort_unstable();
        Ok(contained)
    }

    /// Every live ID with its block's group position (test bookkeeping).
    pub(super) fn for_each_id(&self, mut f: impl FnMut(usize)) {
        for group in &self.groups {
            for block in &group.blocks {
                block.ids().iter().for_each(|&id| f(id));
            }
        }
    }
}

#[derive(serde::Deserialize)]
pub(super) struct OwnerBucket {
    #[allow(dead_code)]
    ids: Vec<usize>,
    pub indexed: AggregateIndex,
    #[allow(dead_code)]
    orthant: Option<usize>,
}

#[derive(serde::Deserialize)]
pub(super) struct Buckets(pub Vec<(Phase, Vec<bool>, OwnerBucket)>);

// The historical `CompactSummary` (176 bytes at N=15), verbatim.
const EMPTY: u8 = 1;
const POSITIVE_UPPER_NONE: u8 = 1 << 1;
const NUMERATOR_UPPER_NONE: u8 = 1 << 2;
const DIFFERENCE_LOWER_NONE: u8 = 1 << 3;
const DIFFERENCE_UPPER_NONE: u8 = 1 << 4;
const WIDE: u8 = 1 << 5;
const INFINITE_EXTREMUM: u32 = u32::MAX;

fn optional<W, T: TryFrom<W> + Default>(value: Option<W>, bit: u8, flags: &mut u8) -> Option<T> {
    match value {
        None => {
            *flags |= bit;
            Some(T::default())
        }
        Some(value) => T::try_from(value).ok(),
    }
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct Summary<const N: usize> {
    flags: u8,
    owner: u32,
    lower: [u32; N],
    upper: [u32; N],
    positive_lower: u64,
    positive_upper: u64,
    numerator_lower: u64,
    numerator_upper: u64,
    difference_lower: i64,
    difference_upper: i64,
}

impl<const N: usize> Summary<N> {
    const ZERO: Self = Self {
        flags: 0,
        owner: 0,
        lower: [0; N],
        upper: [0; N],
        positive_lower: 0,
        positive_upper: 0,
        numerator_lower: 0,
        numerator_upper: 0,
        difference_lower: 0,
        difference_upper: 0,
    };

    pub(super) fn from_core(summary: &DomainPowerSummary<N>) -> Self {
        let owner = summary
            .owner()
            .iter()
            .enumerate()
            .fold(0, |bits, (axis, &active)| {
                bits | (u32::from(active) << axis)
            });
        let Some(extrema) = summary.extrema() else {
            return Self {
                flags: EMPTY,
                owner,
                ..Self::ZERO
            };
        };
        (|| {
            let mut lower = [0; N];
            let mut upper = [INFINITE_EXTREMUM; N];
            for axis in 0..N {
                lower[axis] = u32::try_from(extrema.lower()[axis]).ok()?;
                if let Some(value) = extrema.upper()[axis] {
                    upper[axis] = u32::try_from(value)
                        .ok()
                        .filter(|&v| v != INFINITE_EXTREMUM)?;
                }
            }
            let (a_lower, a_upper) = extrema.positive_power();
            let (r_lower, r_upper) = extrema.numerator_rank();
            let (d_lower, d_upper) = extrema.power_difference();
            let mut flags = 0;
            Some(Self {
                owner,
                lower,
                upper,
                positive_lower: u64::try_from(a_lower).ok()?,
                positive_upper: optional(a_upper, POSITIVE_UPPER_NONE, &mut flags)?,
                numerator_lower: u64::try_from(r_lower).ok()?,
                numerator_upper: optional(r_upper, NUMERATOR_UPPER_NONE, &mut flags)?,
                difference_lower: optional(d_lower, DIFFERENCE_LOWER_NONE, &mut flags)?,
                difference_upper: optional(d_upper, DIFFERENCE_UPPER_NONE, &mut flags)?,
                flags,
            })
        })()
        .unwrap_or(Self {
            flags: WIDE,
            ..Self::ZERO
        })
    }

    pub(super) fn is_wide(&self) -> bool {
        self.flags & WIDE != 0
    }

    #[inline]
    pub(super) fn contains(&self, candidate: &Self) -> bool {
        debug_assert!(!self.is_wide() && !candidate.is_wide());
        if candidate.flags & EMPTY != 0 {
            return true;
        }
        if self.flags & EMPTY != 0 {
            return false;
        }
        let none = |summary: &Self, bit: u8| summary.flags & bit != 0;
        let upper = |bit: u8, container: u64, candidate_value: u64| {
            none(self, bit) || (!none(candidate, bit) && candidate_value <= container)
        };
        self.owner == candidate.owner
            && self.positive_lower <= candidate.positive_lower
            && upper(
                POSITIVE_UPPER_NONE,
                self.positive_upper,
                candidate.positive_upper,
            )
            && self.numerator_lower <= candidate.numerator_lower
            && upper(
                NUMERATOR_UPPER_NONE,
                self.numerator_upper,
                candidate.numerator_upper,
            )
            && (none(self, DIFFERENCE_LOWER_NONE)
                || (!none(candidate, DIFFERENCE_LOWER_NONE)
                    && candidate.difference_lower >= self.difference_lower))
            && (none(self, DIFFERENCE_UPPER_NONE)
                || (!none(candidate, DIFFERENCE_UPPER_NONE)
                    && candidate.difference_upper <= self.difference_upper))
            && self.lower.iter().zip(&candidate.lower).all(|(a, b)| a <= b)
            && self.upper.iter().zip(&candidate.upper).all(|(a, b)| b <= a)
    }
}

/// The historical query side: native summary, historical compact image, word.
pub(super) struct OldQuery<const N: usize> {
    pub core: DomainPowerSummary<N>,
    pub compact: Summary<N>,
    pub word: u64,
}

impl<const N: usize> OldQuery<N> {
    pub(super) fn of(query: &Query<N>) -> Self {
        Self {
            core: query.core.clone(),
            compact: Summary::from_core(&query.core),
            word: bits::word(&query.core),
        }
    }
}

const RELEASED: u32 = u32::MAX;

/// The restored (dense) summary slab: live candidates only, in ID order.
pub(super) struct SummarySlab<const N: usize> {
    slots: Vec<u32>,
    entries: Vec<Summary<N>>,
}

impl<const N: usize> SummarySlab<N> {
    /// `summaries`: the historical summary of every live ID, in ID order.
    pub(super) fn restore(
        summaries: impl IntoIterator<Item = Option<Summary<N>>>,
        ids: usize,
    ) -> Self {
        let mut slab = Self {
            slots: Vec::with_capacity(ids),
            entries: Vec::new(),
        };
        for summary in summaries {
            match summary {
                Some(summary) => {
                    slab.slots.push(slab.entries.len() as u32);
                    slab.entries.push(summary);
                }
                None => slab.slots.push(RELEASED),
            }
        }
        slab
    }

    #[inline]
    fn get(&self, id: usize) -> &Summary<N> {
        &self.entries[self.slots[id] as usize]
    }
}

/// The historical read-only view: slab summaries, per-ID words, domains.
pub(super) struct Legacy<'a, const N: usize> {
    pub domains: &'a [CompactDomain<N>],
    pub slab: SummarySlab<N>,
    pub bits: Vec<u64>,
    pub buckets: HashMap<(Phase, [bool; N]), AggregateIndex>,
}

impl<const N: usize> Legacy<'_, N> {
    /// Exact native inclusion `stored[id] ⊇ query` of a live candidate.
    #[inline]
    pub(super) fn contains(&self, id: usize, query: &OldQuery<N>) -> bool {
        let stored = self.slab.get(id);
        if !stored.is_wide() && !query.compact.is_wide() {
            stored.contains(&query.compact)
        } else {
            self.domains[id].native_summary().contains(&query.core)
        }
    }

    /// Exact native inclusion `query ⊇ stored[id]` of a live candidate.
    #[inline]
    pub(super) fn contained_by(&self, id: usize, query: &OldQuery<N>) -> bool {
        let stored = self.slab.get(id);
        if !stored.is_wide() && !query.compact.is_wide() {
            query.compact.contains(stored)
        } else {
            query.core.contains(&self.domains[id].native_summary())
        }
    }

    /// The ordered-commit forward callback: charge, bit prefilter, predicate.
    pub(super) fn forward_verdict(&self, id: usize, query: &OldQuery<N>) -> (bool, bool) {
        let rejected = !bits::may_contain(self.bits[id], query.word);
        (rejected, !rejected && self.contains(id, query))
    }

    pub(super) fn reverse_verdict(&self, id: usize, query: &OldQuery<N>) -> (bool, bool) {
        let rejected = !bits::may_contain(query.word, self.bits[id]);
        (rejected, !rejected && self.contained_by(id, query))
    }

    /// The historical serial forward lookup: (found, charged checks).
    pub(super) fn find(
        &self,
        key: (Phase, [bool; N]),
        query: &OldQuery<N>,
    ) -> (Option<usize>, usize) {
        let Some(index) = self.buckets.get(&key) else {
            return (None, 0);
        };
        let mut checks = 0_usize;
        let mut session = super::super::SessionCounters::default();
        let found = index
            .find_controlled(
                Signature::of(&query.core),
                Coordinates::of(&query.core),
                0,
                || Ok(()),
                |id| {
                    checks = checks
                        .checked_add(1)
                        .ok_or("domain containment counter overflow")?;
                    let (rejected, contained) = self.forward_verdict(id, query);
                    session.forward_run(1, usize::from(rejected));
                    Ok(contained)
                },
            )
            .unwrap();
        (found, checks)
    }

    /// The historical helper reverse set: (set, examined candidates).
    pub(super) fn reverse(
        &self,
        key: (Phase, [bool; N]),
        query: &OldQuery<N>,
    ) -> (Vec<usize>, usize) {
        let Some(index) = self.buckets.get(&key) else {
            return (Vec::new(), 0);
        };
        let mut checks = 0;
        let set = index
            .collect_contained(
                Signature::of(&query.core),
                Coordinates::of(&query.core),
                usize::MAX,
                || Ok(()),
                |id| {
                    checks += 1;
                    Ok(self.reverse_verdict(id, query).1)
                },
            )
            .unwrap();
        (set, checks)
    }
}
