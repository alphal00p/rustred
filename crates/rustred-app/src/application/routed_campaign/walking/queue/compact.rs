//! Fixed-size in-RAM encodings of queued domains and their native summaries.
//!
//! `Domain<N>` (two heap coordinate vectors) stays the transport type of
//! inspection, events, records and checkpoint sections: the queue converts at
//! admission and expands at dispatch. Both encodings here are exact:
//!
//! - `CompactDomain<N>` is injective on domains whose finite coordinates are
//!   at most `MAX_COMPACT_COORDINATE`, and every field of `Domain<N>` is
//!   encoded (absent optional values are canonical zeros plus a flag), so
//!   derived equality is `Domain` equality and `contains` is
//!   `Domain::contains`. A larger finite coordinate is the only admission
//!   refusal the compact queue introduces.
//! - `CompactSummary<N>` carries the tight extrema of `DomainPowerSummary<N>`
//!   in narrower integers. A summary whose extrema do not fit (possible only
//!   with extreme power or rank bounds, never with the campaign inputs) is
//!   stored as a `wide` marker; comparisons involving it rebuild the native
//!   summary from the queued domain, so no input is ever approximated.
//!
//! Summaries of live lookup candidates sit in a slab with an ID -> slot map.
//! Retiring a candidate from the index releases its slot for reuse; a
//! released ID's slot is never read again (see `PreparedLookup::revalidate`).
use super::index::{Lower, Signature, Upper};
use super::{Domain, Phase};
use rustred::solver::{DomainPowerBounds, DomainPowerError, DomainPowerSummary};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::hash::{BuildHasherDefault, Hash, Hasher};

/// Largest finite coordinate of a queued domain; `u16::MAX` encodes +infinity.
pub(in super::super) const MAX_COMPACT_COORDINATE: u64 = u16::MAX as u64 - 1;
/// The only refusal introduced by the compact queue (never reached by inputs
/// whose coordinates stay at or below `MAX_COMPACT_COORDINATE`).
pub(in super::super) const COMPACT_RANGE_ERROR: &str =
    "domain coordinate exceeds the compact queue range (finite coordinates must be <= 65534)";
/// Owner axes are packed into one `u32` bitmask.
const MAX_COMPACT_ARITY: usize = 32;
const INFINITE_COORDINATE: u16 = u16::MAX;

// `CompactDomain::flags`: which optional fields are absent (None).
const RANK_NONE: u8 = 1;
const POSITIVE_POWER_NONE: u8 = 1 << 1;
const MIN_DIFFERENCE_NONE: u8 = 1 << 2;
const MAX_DIFFERENCE_NONE: u8 = 1 << 3;

/// One queued domain, 96 bytes at N=15 (`Domain<15>` behind an `Arc` held
/// three heap blocks, about 500 bytes).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in super::super) struct CompactDomain<const N: usize> {
    phase: u8,
    flags: u8,
    owner: u32,
    rank: u32,
    lower: [u16; N],
    /// `INFINITE_COORDINATE` is +infinity; every finite value is smaller.
    upper: [u16; N],
    max_positive_power: u64,
    min_power_difference: i64,
    max_power_difference: i64,
}

impl<const N: usize> std::fmt::Debug for CompactDomain<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.expand().fmt(f)
    }
}

fn compact_coordinate(value: u64) -> Result<u16, &'static str> {
    if value <= MAX_COMPACT_COORDINATE {
        Ok(value as u16)
    } else {
        Err(COMPACT_RANGE_ERROR)
    }
}

fn owner_bits<const N: usize>(owner: &[bool; N]) -> u32 {
    const { assert!(N <= MAX_COMPACT_ARITY) };
    owner.iter().enumerate().fold(0, |bits, (axis, &active)| {
        bits | (u32::from(active) << axis)
    })
}

impl<const N: usize> CompactDomain<N> {
    pub(in super::super) fn try_from_domain(domain: &Domain<N>) -> Result<Self, &'static str> {
        const { assert!(N <= MAX_COMPACT_ARITY) };
        if domain.lower.len() != N || domain.upper.len() != N {
            return Err("domain coordinate arity");
        }
        let mut lower = [0; N];
        let mut upper = [INFINITE_COORDINATE; N];
        for axis in 0..N {
            lower[axis] = compact_coordinate(domain.lower[axis])?;
            if let Some(value) = domain.upper[axis] {
                upper[axis] = compact_coordinate(value)?;
            }
        }
        let powers = domain.powers;
        let flag = |none: bool, bit: u8| if none { bit } else { 0 };
        Ok(Self {
            phase: match domain.phase {
                Phase::Apply => 0,
                Phase::Route => 1,
            },
            flags: flag(domain.rank.is_none(), RANK_NONE)
                | flag(powers.max_positive_power.is_none(), POSITIVE_POWER_NONE)
                | flag(powers.min_power_difference.is_none(), MIN_DIFFERENCE_NONE)
                | flag(powers.max_power_difference.is_none(), MAX_DIFFERENCE_NONE),
            owner: owner_bits(&domain.owner),
            rank: domain.rank.unwrap_or(0),
            lower,
            upper,
            max_positive_power: powers.max_positive_power.unwrap_or(0),
            min_power_difference: powers.min_power_difference.unwrap_or(0),
            max_power_difference: powers.max_power_difference.unwrap_or(0),
        })
    }

    /// Checkpoint restore: the historical record checks, then the compact range.
    pub(in super::super) fn restore(domain: &Domain<N>) -> Result<Self, String> {
        if domain.lower.len() != N || domain.upper.len() != N {
            return Err("checkpoint coordinate arity".into());
        }
        domain.powers.validate().map_err(|e| e.to_string())?;
        Self::try_from_domain(domain).map_err(|e| format!("checkpoint domain: {e}"))
    }

    pub(in super::super) fn expand(&self) -> Domain<N> {
        Domain {
            phase: self.phase(),
            owner: self.owner(),
            lower: (0..N).map(|axis| self.lower(axis)).collect(),
            upper: (0..N).map(|axis| self.upper(axis)).collect(),
            rank: self.rank(),
            powers: self.powers(),
        }
    }

    pub(in super::super) fn phase(&self) -> Phase {
        if self.phase == 0 {
            Phase::Apply
        } else {
            Phase::Route
        }
    }

    pub(in super::super) fn owner(&self) -> [bool; N] {
        std::array::from_fn(|axis| self.owner >> axis & 1 == 1)
    }

    pub(in super::super) fn rank(&self) -> Option<u32> {
        (self.flags & RANK_NONE == 0).then_some(self.rank)
    }

    pub(in super::super) fn powers(&self) -> DomainPowerBounds {
        let value = |bit: u8| self.flags & bit == 0;
        DomainPowerBounds {
            max_positive_power: value(POSITIVE_POWER_NONE).then_some(self.max_positive_power),
            min_power_difference: value(MIN_DIFFERENCE_NONE).then_some(self.min_power_difference),
            max_power_difference: value(MAX_DIFFERENCE_NONE).then_some(self.max_power_difference),
        }
    }

    pub(in super::super) fn lower(&self, axis: usize) -> u64 {
        u64::from(self.lower[axis])
    }

    pub(in super::super) fn upper(&self, axis: usize) -> Option<u64> {
        let value = self.upper[axis];
        (value != INFINITE_COORDINATE).then_some(u64::from(value))
    }

    /// `Domain::contains` on the encoded fields.
    pub(super) fn contains(&self, other: &Self) -> bool {
        self.phase == other.phase
            && self.owner == other.owner
            && super::rank_contains(self.rank(), other.rank())
            && self.powers().contains(&other.powers())
            && self.lower.iter().zip(&other.lower).all(|(a, b)| a <= b)
            // +infinity is the largest code, so upper inclusion is plain <=.
            && self.upper.iter().zip(&other.upper).all(|(a, b)| b <= a)
    }

    pub(super) fn is_full_orthant(&self) -> bool {
        self.powers().is_unconstrained()
            && self.lower.iter().all(|&x| x == 0)
            // Same box predicate as general containment: rank is not part of it.
            && self.upper.iter().all(|&x| x == INFINITE_COORDINATE)
    }

    pub(super) fn try_native_summary(&self) -> Result<DomainPowerSummary<N>, DomainPowerError> {
        let domain = self.expand();
        DomainPowerSummary::try_new(
            domain.owner,
            &domain.lower,
            &domain.upper,
            domain.rank,
            domain.powers,
        )
    }

    /// The native summary this domain was admitted with. `try_new` is a pure
    /// function of the domain, and it succeeded at admission or restore.
    pub(super) fn native_summary(&self) -> DomainPowerSummary<N> {
        self.try_native_summary()
            .expect("an admitted domain's native summary is reproducible")
    }

    /// 128-bit blake3 prefix of the canonical little-endian field bytes
    /// (padding never enters the digest).
    pub(super) fn digest(&self) -> Digest {
        let mut bytes = [0_u8; 12 + 4 * MAX_COMPACT_ARITY + 24];
        let mut at = 0;
        let mut put = |field: &[u8]| {
            bytes[at..at + field.len()].copy_from_slice(field);
            at += field.len();
        };
        put(&[self.phase, self.flags]);
        put(&self.owner.to_le_bytes());
        put(&self.rank.to_le_bytes());
        for value in self.lower.iter().chain(&self.upper) {
            put(&value.to_le_bytes());
        }
        put(&self.max_positive_power.to_le_bytes());
        put(&self.min_power_difference.to_le_bytes());
        put(&self.max_power_difference.to_le_bytes());
        let hash = blake3::hash(&bytes[..at]);
        let hash = hash.as_bytes();
        Digest(
            u64::from_le_bytes(hash[..8].try_into().expect("eight bytes")),
            u64::from_le_bytes(hash[8..16].try_into().expect("eight bytes")),
        )
    }
}

/// Exact-map key. Stored as two words so a map bucket is 24 bytes (a `u128`
/// key would be 16-byte aligned and make it 32).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Digest(pub u64, pub u64);

impl Hash for Digest {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.0);
    }
}

/// The key is already a uniformly distributed digest; hash it by identity.
#[derive(Default)]
pub(super) struct DigestHasher(u64);

impl Hasher for DigestHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = (self.0.rotate_left(8) ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }
    }
    fn write_u64(&mut self, value: u64) {
        self.0 = value;
    }
}

type DigestMap<V> = HashMap<Digest, V, BuildHasherDefault<DigestHasher>>;

/// Exact-duplicate index. A digest hit is only a candidate: the stored compact
/// domain is compared field by field, and genuinely different domains with
/// the same digest live in `overflow`, so lookups never depend on the digest
/// being collision free.
#[derive(Debug)]
pub(super) struct ExactIndex<const N: usize> {
    primary: DigestMap<usize>,
    overflow: DigestMap<Vec<usize>>,
    #[cfg(test)]
    key: fn(&CompactDomain<N>) -> Digest,
}

/// Same indexed content (the test-only key function is not state).
impl<const N: usize> PartialEq for ExactIndex<N> {
    fn eq(&self, other: &Self) -> bool {
        self.primary == other.primary && self.overflow == other.overflow
    }
}

impl<const N: usize> ExactIndex<N> {
    pub fn new() -> Self {
        Self {
            primary: DigestMap::default(),
            overflow: DigestMap::default(),
            #[cfg(test)]
            key: CompactDomain::digest,
        }
    }

    /// Test seam: force digest collisions to exercise the overflow path.
    #[cfg(test)]
    pub fn set_key_function(&mut self, key: fn(&CompactDomain<N>) -> Digest) {
        assert!(self.primary.is_empty(), "key function of a populated index");
        self.key = key;
    }

    pub fn key(&self, domain: &CompactDomain<N>) -> Digest {
        #[cfg(test)]
        return (self.key)(domain);
        #[cfg(not(test))]
        domain.digest()
    }

    pub fn get(
        &self,
        key: Digest,
        candidate: &CompactDomain<N>,
        domains: &[CompactDomain<N>],
    ) -> Option<usize> {
        let &id = self.primary.get(&key)?;
        if domains[id] == *candidate {
            return Some(id);
        }
        self.overflow
            .get(&key)?
            .iter()
            .copied()
            .find(|&id| domains[id] == *candidate)
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.primary.len() + self.overflow.values().map(Vec::len).sum::<usize>()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.primary.is_empty()
    }

    #[cfg(test)]
    pub fn overflow_len(&self) -> usize {
        self.overflow.values().map(Vec::len).sum()
    }

    pub fn try_reserve_total(&mut self, additional: usize) -> Result<(), &'static str> {
        self.primary
            .try_reserve(additional)
            .map_err(|_| "exact domain index allocation")
    }

    /// Reserve everything `insert(key, _)` needs, so publication is infallible.
    /// A collision list left empty by a later failed preflight is harmless.
    pub fn try_reserve(&mut self, key: Digest) -> Result<(), &'static str> {
        self.primary
            .try_reserve(1)
            .map_err(|_| "exact domain index allocation")?;
        if self.primary.contains_key(&key) {
            self.overflow
                .try_reserve(1)
                .map_err(|_| "exact domain index allocation")?;
            self.overflow
                .entry(key)
                .or_default()
                .try_reserve(1)
                .map_err(|_| "exact domain index allocation")?;
        }
        Ok(())
    }

    pub fn insert(&mut self, key: Digest, id: usize) {
        match self.primary.entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(id);
            }
            Entry::Occupied(_) => self.overflow.entry(key).or_default().push(id),
        }
    }

    /// Bytes of the reserved table storage (entries plus control bytes; a
    /// table holds 8/7 buckets per usable slot).
    pub fn capacity_bytes(&self) -> usize {
        let buckets = |capacity: usize| capacity / 7 * 8 + capacity % 7;
        buckets(self.primary.capacity()) * (std::mem::size_of::<(Digest, usize)>() + 1)
            + buckets(self.overflow.capacity()) * (std::mem::size_of::<(Digest, Vec<usize>)>() + 1)
            + self
                .overflow
                .values()
                .map(|ids| ids.capacity() * std::mem::size_of::<usize>())
                .sum::<usize>()
    }
}

// `CompactSummary::flags`.
const EMPTY: u8 = 1;
const POSITIVE_UPPER_NONE: u8 = 1 << 1;
const NUMERATOR_UPPER_NONE: u8 = 1 << 2;
const DIFFERENCE_LOWER_NONE: u8 = 1 << 3;
const DIFFERENCE_UPPER_NONE: u8 = 1 << 4;
/// Extrema outside the compact ranges; compare the rebuilt native summary.
const WIDE: u8 = 1 << 5;
/// A released slab slot; `positive_lower` links the next free slot.
const FREE: u8 = 1 << 6;
const INFINITE_EXTREMUM: u32 = u32::MAX;

/// A present value narrowed to `T`, or the canonical zero with `bit` set in
/// `flags` for an absent one. None when a present value does not fit.
fn optional<W, T: TryFrom<W> + Default>(value: Option<W>, bit: u8, flags: &mut u8) -> Option<T> {
    match value {
        None => {
            *flags |= bit;
            Some(T::default())
        }
        Some(value) => T::try_from(value).ok(),
    }
}

/// Tight native extrema of one live candidate, 176 bytes at N=15 (the native
/// `DomainPowerSummary<15>` is about 560 bytes).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct CompactSummary<const N: usize> {
    flags: u8,
    owner: u32,
    lower: [u32; N],
    /// `INFINITE_EXTREMUM` is +infinity; every finite value is smaller.
    upper: [u32; N],
    positive_lower: u64,
    positive_upper: u64,
    numerator_lower: u64,
    numerator_upper: u64,
    difference_lower: i64,
    difference_upper: i64,
}

impl<const N: usize> CompactSummary<N> {
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

    /// Exact compact image, or the `wide` marker when an extremum does not fit.
    pub fn from_core(summary: &DomainPowerSummary<N>) -> Self {
        let owner = owner_bits(summary.owner());
        let Some(extrema) = summary.extrema() else {
            return Self {
                flags: EMPTY,
                owner,
                ..Self::ZERO
            };
        };
        Self::try_from_extrema(owner, extrema).unwrap_or(Self {
            flags: WIDE,
            ..Self::ZERO
        })
    }

    fn try_from_extrema(
        owner: u32,
        extrema: &rustred::solver::DomainPowerExtrema<N>,
    ) -> Option<Self> {
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
    }

    pub fn is_wide(&self) -> bool {
        self.flags & WIDE != 0
    }

    /// `DomainPowerSummary::contains` on the compact fields; neither side may
    /// be wide.
    #[inline]
    pub fn contains(&self, candidate: &Self) -> bool {
        debug_assert!(!self.is_wide() && !candidate.is_wide());
        debug_assert!((self.flags | candidate.flags) & FREE == 0);
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

    /// `Signature::of` the native summary; None for a wide summary.
    pub fn signature(&self) -> Option<Signature> {
        if self.is_wide() {
            return None;
        }
        if self.flags & EMPTY != 0 {
            return Some(Signature::Empty);
        }
        let none = |bit: u8| self.flags & bit != 0;
        let upper = |bit: u8, value: u64| {
            if none(bit) {
                Upper::Infinity
            } else {
                Upper::Finite(u128::from(value))
            }
        };
        Some(Signature::Nonempty {
            positive: upper(POSITIVE_UPPER_NONE, self.positive_upper),
            numerator: upper(NUMERATOR_UPPER_NONE, self.numerator_upper),
            difference: if none(DIFFERENCE_LOWER_NONE) {
                Lower::NegativeInfinity
            } else {
                Lower::Finite(i128::from(self.difference_lower))
            },
        })
    }

    /// Every extremum equals the native one (the round-trip property).
    #[cfg(test)]
    pub fn matches_core(&self, summary: &DomainPowerSummary<N>) -> bool {
        if self.is_wide() || self.owner != owner_bits(summary.owner()) {
            return false;
        }
        let Some(extrema) = summary.extrema() else {
            return self.flags == EMPTY;
        };
        let none = |bit: u8| self.flags & bit != 0;
        let upper = |bit: u8, value: u64| (!none(bit)).then_some(u128::from(value));
        let signed = |bit: u8, value: i64| (!none(bit)).then_some(i128::from(value));
        self.flags & EMPTY == 0
            && (0..N).all(|axis| {
                u64::from(self.lower[axis]) == extrema.lower()[axis]
                    && (self.upper[axis] != INFINITE_EXTREMUM)
                        .then_some(u64::from(self.upper[axis]))
                        == extrema.upper()[axis]
            })
            && (
                u128::from(self.positive_lower),
                upper(POSITIVE_UPPER_NONE, self.positive_upper),
            ) == extrema.positive_power()
            && (
                u128::from(self.numerator_lower),
                upper(NUMERATOR_UPPER_NONE, self.numerator_upper),
            ) == extrema.numerator_rank()
            && (
                signed(DIFFERENCE_LOWER_NONE, self.difference_lower),
                signed(DIFFERENCE_UPPER_NONE, self.difference_upper),
            ) == extrema.power_difference()
    }

    fn free_link(next: u32) -> Self {
        Self {
            flags: FREE,
            positive_lower: u64::from(next),
            ..Self::ZERO
        }
    }

    fn next_free(&self) -> u32 {
        debug_assert!(self.flags & FREE != 0);
        self.positive_lower as u32
    }
}

const RELEASED: u32 = u32::MAX;
const NO_FREE_SLOT: u32 = u32::MAX;

/// Compact summaries of the live lookup candidates. `slots[id]` locates the
/// summary of an indexed ID; retirement from the candidate index releases the
/// slot onto an intrusive free list (no allocation) for the next admission.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct SummarySlab<const N: usize> {
    slots: Vec<u32>,
    entries: Vec<CompactSummary<N>>,
    free: u32,
    live: usize,
}

impl<const N: usize> SummarySlab<N> {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            entries: Vec::new(),
            free: NO_FREE_SLOT,
            live: 0,
        }
    }

    /// IDs that ever received a summary (every admitted ID in the unlimited
    /// lane, none in the finite-cap lane).
    pub fn ids(&self) -> usize {
        self.slots.len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Summaries currently held (IDs not released).
    pub fn live(&self) -> usize {
        self.live
    }

    /// Allocated slots, live or free.
    #[cfg(test)]
    pub fn slots_allocated(&self) -> usize {
        self.entries.len()
    }

    pub fn is_released(&self, id: usize) -> bool {
        self.slots[id] == RELEASED
    }

    /// The summary of a live candidate. A released ID has no summary: its slot
    /// may already hold another ID's, so callers check `is_released` first
    /// whenever the ID might have been retired (only `revalidate` can).
    #[inline]
    pub fn get(&self, id: usize) -> &CompactSummary<N> {
        let slot = self.slots[id];
        debug_assert_ne!(slot, RELEASED, "released summary slot read");
        // RELEASED is never a valid index: entries stay below u32::MAX.
        &self.entries[slot as usize]
    }

    /// Reserve for one `push`, so publication is infallible.
    pub fn try_reserve(&mut self) -> Result<(), &'static str> {
        self.slots
            .try_reserve(1)
            .map_err(|_| "domain summary allocation")?;
        if self.free == NO_FREE_SLOT {
            if self.entries.len() >= RELEASED as usize {
                return Err("domain summary slot range");
            }
            self.entries
                .try_reserve(1)
                .map_err(|_| "domain summary allocation")?;
        }
        Ok(())
    }

    /// Restore: exact room for `ids` further IDs of which `live` hold a summary.
    pub fn try_reserve_exact(&mut self, ids: usize, live: usize) -> Result<(), &'static str> {
        if self.entries.len().saturating_add(live) >= RELEASED as usize {
            return Err("domain summary slot range");
        }
        self.slots
            .try_reserve_exact(ids)
            .map_err(|_| "domain summary allocation")?;
        self.entries
            .try_reserve_exact(live)
            .map_err(|_| "domain summary allocation")
    }

    /// Store the next ID's summary, reusing the most recently released slot.
    pub fn push(&mut self, summary: CompactSummary<N>) {
        let slot = if self.free != NO_FREE_SLOT {
            let slot = self.free;
            self.free = self.entries[slot as usize].next_free();
            self.entries[slot as usize] = summary;
            slot
        } else {
            let slot = self.entries.len() as u32; // bounded by try_reserve
            self.entries.push(summary);
            slot
        };
        self.slots.push(slot);
        self.live += 1;
    }

    /// Restore: the next ID is not an indexed candidate and holds no summary.
    pub fn push_released(&mut self) {
        self.slots.push(RELEASED);
    }

    /// The ID left the candidate index for good; infallible.
    pub fn release(&mut self, id: usize) {
        let slot = std::mem::replace(&mut self.slots[id], RELEASED);
        debug_assert_ne!(slot, RELEASED, "summary slot released twice");
        self.entries[slot as usize] = CompactSummary::free_link(self.free);
        self.free = slot;
        self.live -= 1;
    }

    /// Bytes of the reserved slot map and slab.
    pub fn capacity_bytes(&self) -> usize {
        self.slots.capacity() * std::mem::size_of::<u32>()
            + self.entries.capacity() * std::mem::size_of::<CompactSummary<N>>()
    }
}

/// Query side of one admission: its native summary (block filters, bit word,
/// the rare wide comparison) and the compact image compared on the hot path.
#[derive(Clone)]
pub(super) struct Query<const N: usize> {
    pub core: DomainPowerSummary<N>,
    pub compact: CompactSummary<N>,
    pub word: u64,
}

impl<const N: usize> Query<N> {
    pub fn new(core: DomainPowerSummary<N>) -> Self {
        Self {
            compact: CompactSummary::from_core(&core),
            word: super::bits::word(&core),
            core,
        }
    }
}

/// Read-only view of the stored candidate geometry for containment callbacks.
#[derive(Clone, Copy)]
pub(super) struct Stored<'a, const N: usize> {
    pub domains: &'a [CompactDomain<N>],
    pub summaries: &'a SummarySlab<N>,
}

impl<const N: usize> Stored<'_, N> {
    /// Exact native inclusion `stored[id] ⊇ query` of a live candidate.
    #[inline]
    pub fn contains(&self, id: usize, query: &Query<N>) -> bool {
        let stored = self.summaries.get(id);
        if !stored.is_wide() && !query.compact.is_wide() {
            stored.contains(&query.compact)
        } else {
            self.domains[id].native_summary().contains(&query.core)
        }
    }

    /// Exact native inclusion `query ⊇ stored[id]` of a live candidate.
    #[inline]
    pub fn contained_by(&self, id: usize, query: &Query<N>) -> bool {
        let stored = self.summaries.get(id);
        if !stored.is_wide() && !query.compact.is_wide() {
            query.compact.contains(stored)
        } else {
            query.core.contains(&self.domains[id].native_summary())
        }
    }

    /// `Signature::of` the native summary of a live candidate.
    pub fn signature(&self, id: usize) -> Signature {
        self.summaries
            .get(id)
            .signature()
            .unwrap_or_else(|| Signature::of(&self.domains[id].native_summary()))
    }
}
