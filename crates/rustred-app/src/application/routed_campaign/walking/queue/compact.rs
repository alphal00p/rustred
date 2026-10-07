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
//! Summaries are per ID and immutable: every admitted ID of the unlimited lane
//! keeps its summary for good, whether its candidate is live, retired or
//! aliased, so any reader may read any admitted ID (v3 design §3.1).
use super::index::{Lanes, Lower, Signature, Upper};
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
        Self::try_from_parts(
            domain.phase,
            domain.owner,
            &domain.lower,
            &domain.upper,
            domain.rank,
            domain.powers,
        )
    }

    /// The same checked encoding for transport domains and borrowed coordinates.
    /// This checks representability only; native summary construction remains
    /// responsible for validating the mathematical domain.
    pub(in super::super) fn try_from_parts(
        phase: Phase,
        owner: [bool; N],
        lower_bounds: &[u64],
        upper_bounds: &[Option<u64>],
        rank: Option<u32>,
        powers: DomainPowerBounds,
    ) -> Result<Self, &'static str> {
        const { assert!(N <= MAX_COMPACT_ARITY) };
        if !crate::application::routed_campaign::storage::compatible_width(lower_bounds.len(), N)
            || upper_bounds.len() != lower_bounds.len()
            || owner
                .get(lower_bounds.len()..)
                .is_some_and(|tail| tail.iter().any(|active| *active))
        {
            return Err("domain coordinate arity");
        }
        let lower_bounds =
            crate::application::routed_campaign::storage::restore_array::<_, N>(lower_bounds, 0)
                .ok_or("domain lower storage padding")?;
        let upper_bounds = crate::application::routed_campaign::storage::restore_array::<_, N>(
            upper_bounds,
            Some(0),
        )
        .ok_or("domain upper storage padding")?;
        let mut lower = [0; N];
        let mut upper = [INFINITE_COORDINATE; N];
        for axis in 0..N {
            lower[axis] = compact_coordinate(lower_bounds.get(axis).copied().unwrap_or(0))?;
            if let Some(value) = upper_bounds.get(axis).copied().unwrap_or(Some(0)) {
                upper[axis] = compact_coordinate(value)?;
            }
        }
        let flag = |none: bool, bit: u8| if none { bit } else { 0 };
        Ok(Self {
            phase: match phase {
                Phase::Apply => 0,
                Phase::Route => 1,
            },
            flags: flag(rank.is_none(), RANK_NONE)
                | flag(powers.max_positive_power.is_none(), POSITIVE_POWER_NONE)
                | flag(powers.min_power_difference.is_none(), MIN_DIFFERENCE_NONE)
                | flag(powers.max_power_difference.is_none(), MAX_DIFFERENCE_NONE),
            owner: owner_bits(&owner),
            rank: rank.unwrap_or(0),
            lower,
            upper,
            max_positive_power: powers.max_positive_power.unwrap_or(0),
            min_power_difference: powers.min_power_difference.unwrap_or(0),
            max_power_difference: powers.max_power_difference.unwrap_or(0),
        })
    }

    /// Checkpoint restore: the historical record checks, then the compact range.
    pub(in super::super) fn restore(domain: &Domain<N>) -> Result<Self, String> {
        if !crate::application::routed_campaign::storage::compatible_width(domain.lower.len(), N)
            || domain.upper.len() != domain.lower.len()
        {
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

    /// Raw coordinate bounds, `u16::MAX` being +infinity (every finite
    /// coordinate is smaller). Read by the G2' anchor index's hot loops.
    pub(in super::super) fn raw_bounds(&self) -> (&[u16; N], &[u16; N]) {
        (&self.lower, &self.upper)
    }

    /// `Domain::contains` on the encoded fields.
    pub(in super::super) fn contains(&self, other: &Self) -> bool {
        self.phase == other.phase
            && self.owner == other.owner
            && super::rank_contains(self.rank(), other.rank())
            && self.powers().contains(&other.powers())
            && self.lower.iter().zip(&other.lower).all(|(a, b)| a <= b)
            // +infinity is the largest code, so upper inclusion is plain <=.
            && self.upper.iter().zip(&other.upper).all(|(a, b)| b <= a)
    }

    /// The (phase, owner) bucket key, compared explicitly by every positive
    /// (A1): summary containment ignores phase and accepts an empty
    /// candidate before it compares owners.
    pub(in super::super) fn same_bucket(&self, phase: u8, owner: u32) -> bool {
        self.phase == phase && self.owner == owner
    }

    pub(in super::super) fn bucket_code(&self) -> (u8, u32) {
        (self.phase, self.owner)
    }

    pub(in super::super) fn is_full_orthant(&self) -> bool {
        self.powers().is_unconstrained()
            && self.lower.iter().all(|&x| x == 0)
            // Same box predicate as general containment: rank is not part of it.
            && self.upper.iter().all(|&x| x == INFINITE_COORDINATE)
    }

    pub(in super::super) fn try_native_summary(
        &self,
    ) -> Result<DomainPowerSummary<N>, DomainPowerError> {
        let lower: [u64; N] = std::array::from_fn(|axis| self.lower(axis));
        let upper: [Option<u64>; N] = std::array::from_fn(|axis| self.upper(axis));
        DomainPowerSummary::try_new(self.owner(), &lower, &upper, self.rank(), self.powers())
    }

    /// The native summary this domain was admitted with. `try_new` is a pure
    /// function of the domain, and it succeeded at admission or restore.
    pub(in super::super) fn native_summary(&self) -> DomainPowerSummary<N> {
        self.try_native_summary()
            .expect("an admitted domain's native summary is reproducible")
    }

    /// Decode the `trace_image` bytes of the research admission trace (W0.4
    /// `admission-trace` feature); None unless the image is canonical.
    #[cfg(test)]
    pub(in super::super) fn from_trace_image(b: &[u8]) -> Option<Self> {
        if b.len() != 34 + 4 * N {
            return None;
        }
        let u32_at = |p: usize| u32::from_le_bytes(b[p..p + 4].try_into().expect("four bytes"));
        let u64_at = |p: usize| u64::from_le_bytes(b[p..p + 8].try_into().expect("eight bytes"));
        let mut at = 10;
        let mut coordinate = || {
            let value = u16::from_le_bytes([b[at], b[at + 1]]);
            at += 2;
            value
        };
        let lower = std::array::from_fn(|_| coordinate());
        let upper = std::array::from_fn(|_| coordinate());
        let image = Self {
            phase: b[0],
            flags: b[1],
            owner: u32_at(2),
            rank: u32_at(6),
            lower,
            upper,
            max_positive_power: u64_at(10 + 4 * N),
            min_power_difference: u64_at(18 + 4 * N) as i64,
            max_power_difference: u64_at(26 + 4 * N) as i64,
        };
        (Self::try_from_domain(&image.expand()).ok() == Some(image)).then_some(image)
    }

    /// 64-bit blake3 prefix of the canonical little-endian field bytes
    /// (padding never enters the digest).
    pub(in super::super) fn digest(&self) -> Digest {
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
        Digest(u64::from_le_bytes(
            hash.as_bytes()[..8].try_into().expect("eight bytes"),
        ))
    }
}

/// Exact-map key. Every hit is confirmed on the stored domain and genuine
/// collisions live in `overflow`, so the width only sets how rare that path
/// is (about 4e-5 expected colliding pairs among 38M domains, an estimate);
/// one word keeps a map bucket at 16 bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct Digest(pub u64);

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

/// An exact-index miss: whether a different domain already holds the digest's
/// primary slot, so publishing this one goes to `overflow`. Valid until the
/// index next changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Miss {
    collides: bool,
}

impl Miss {
    /// The miss of an exact duplicate an amended walk admits on purpose (its
    /// earlier equal ID is quarantined): publication goes to `overflow`.
    pub fn duplicate() -> Self {
        Self { collides: true }
    }
}

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

    /// The confirmed ID of `candidate`, or the miss that `try_reserve` needs
    /// to prepare its publication without probing the table again.
    pub fn get(
        &self,
        key: Digest,
        candidate: &CompactDomain<N>,
        domains: &[CompactDomain<N>],
    ) -> Result<usize, Miss> {
        let Some(&id) = self.primary.get(&key) else {
            return Err(Miss { collides: false });
        };
        if domains[id] == *candidate {
            return Ok(id);
        }
        self.overflow
            .get(&key)
            .and_then(|ids| ids.iter().copied().find(|&id| domains[id] == *candidate))
            .ok_or(Miss { collides: true })
    }

    /// `get` that skips IDs in the rescue quarantine (`rescue.rs`): the first
    /// non-quarantined ID holding exactly `candidate`. An amended walk may
    /// hold exact duplicates, a quarantined ID and the later live one (in
    /// `overflow`, in admission order). An empty quarantine is plain `get`.
    pub fn get_live(
        &self,
        key: Digest,
        candidate: &CompactDomain<N>,
        domains: &[CompactDomain<N>],
        quarantine: &[u64],
    ) -> Result<usize, Miss> {
        if quarantine.is_empty() {
            return self.get(key, candidate, domains);
        }
        let Some(&id) = self.primary.get(&key) else {
            return Err(Miss { collides: false });
        };
        if domains[id] == *candidate && !quarantined(quarantine, id) {
            return Ok(id);
        }
        self.overflow
            .get(&key)
            .and_then(|ids| {
                ids.iter()
                    .copied()
                    .find(|&id| domains[id] == *candidate && !quarantined(quarantine, id))
            })
            .ok_or(Miss { collides: true })
    }

    /// Every exact-duplicate group (IDs of one domain, ascending).
    #[cfg(test)]
    pub fn duplicate_groups(&self, domains: &[CompactDomain<N>]) -> Vec<Vec<usize>> {
        let mut groups = Vec::new();
        for (key, ids) in &self.overflow {
            let mut all: Vec<usize> = self.primary.get(key).copied().into_iter().collect();
            all.extend_from_slice(ids);
            let mut seen = vec![false; all.len()];
            for i in 0..all.len() {
                if seen[i] {
                    continue;
                }
                let mut group = vec![all[i]];
                for j in i + 1..all.len() {
                    if !seen[j] && domains[all[j]] == domains[all[i]] {
                        seen[j] = true;
                        group.push(all[j]);
                    }
                }
                if group.len() > 1 {
                    group.sort_unstable();
                    groups.push(group);
                }
            }
        }
        groups.sort_unstable();
        groups
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

    /// Reserve everything `insert(key, _)` needs after `miss` (the index is
    /// unchanged since that lookup), so publication is infallible. A collision
    /// list left empty by a later failed preflight is harmless.
    pub fn try_reserve(&mut self, key: Digest, miss: Miss) -> Result<(), &'static str> {
        if miss.collides {
            self.overflow
                .try_reserve(1)
                .map_err(|_| "exact domain index allocation")?;
            self.overflow
                .entry(key)
                .or_default()
                .try_reserve(1)
                .map_err(|_| "exact domain index allocation")?;
        } else {
            self.primary
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
const INFINITE_EXTREMUM: u16 = u16::MAX;

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

/// Tight native extrema of one admitted domain, 92 bytes at N=15 (the native
/// `DomainPowerSummary<15>` is about 560 bytes). Kept for every admitted ID,
/// so the widths are the campaign's needs: coordinates below 65535, A and R
/// below 2^32, D within i32; anything else is `wide` (exact native fallback).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in super::super) struct CompactSummary<const N: usize> {
    owner: u32,
    positive_lower: u32,
    positive_upper: u32,
    numerator_lower: u32,
    numerator_upper: u32,
    difference_lower: i32,
    difference_upper: i32,
    lower: [u16; N],
    /// `INFINITE_EXTREMUM` is +infinity; every finite value is smaller.
    upper: [u16; N],
    flags: u8,
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
            lower[axis] = u16::try_from(extrema.lower()[axis]).ok()?;
            if let Some(value) = extrema.upper()[axis] {
                upper[axis] = u16::try_from(value)
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
            positive_lower: u32::try_from(a_lower).ok()?,
            positive_upper: optional(a_upper, POSITIVE_UPPER_NONE, &mut flags)?,
            numerator_lower: u32::try_from(r_lower).ok()?,
            numerator_upper: optional(r_upper, NUMERATOR_UPPER_NONE, &mut flags)?,
            difference_lower: optional(d_lower, DIFFERENCE_LOWER_NONE, &mut flags)?,
            difference_upper: optional(d_upper, DIFFERENCE_UPPER_NONE, &mut flags)?,
            flags,
        })
    }

    pub fn is_wide(&self) -> bool {
        self.flags & WIDE != 0
    }

    /// Tight rank geometry for telemetry. `None` requests the existing native
    /// fallback for a wide summary; an absent explicit cap is not infinity.
    pub fn numerator_rank_extent(&self) -> Option<super::super::rank_telemetry::RankExtent> {
        use super::super::rank_telemetry::RankExtent;
        if self.is_wide() {
            None
        } else if self.flags & EMPTY != 0 {
            Some(RankExtent::Empty)
        } else if self.flags & NUMERATOR_UPPER_NONE != 0 {
            Some(RankExtent::Unbounded)
        } else {
            Some(RankExtent::Finite(u128::from(self.numerator_upper)))
        }
    }

    /// `DomainPowerSummary::contains` on the compact fields, or None when
    /// either side is wide (a release check, A1: the caller then compares
    /// the rebuilt native summaries). Like the native predicate it ignores
    /// phase and accepts an empty candidate before comparing owners.
    #[inline]
    pub fn contains(&self, candidate: &Self) -> Option<bool> {
        if (self.flags | candidate.flags) & WIDE != 0 {
            return None;
        }
        if candidate.flags & EMPTY != 0 {
            return Some(true);
        }
        if self.flags & EMPTY != 0 {
            return Some(false);
        }
        let none = |summary: &Self, bit: u8| summary.flags & bit != 0;
        let upper = |bit: u8, container: u32, candidate_value: u32| {
            none(self, bit) || (!none(candidate, bit) && candidate_value <= container)
        };
        Some(
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
                && self.upper.iter().zip(&candidate.upper).all(|(a, b)| b <= a),
        )
    }

    /// The kernel lanes of this summary (the same construction as
    /// `Lanes::of_core` on the native summary it encodes). None for an empty
    /// or a wide summary; a wide one takes its lanes from the native form.
    pub fn lanes(&self) -> Option<Lanes<N>> {
        if self.flags & (EMPTY | WIDE) != 0 {
            return None;
        }
        let none = |bit: u8| self.flags & bit != 0;
        let upper = |bit: u8, value: u32| (!none(bit)).then_some(u128::from(value));
        let signed = |bit: u8, value: i32| (!none(bit)).then_some(i128::from(value));
        Some(Lanes::build(super::index::LaneSource {
            lower: &|axis| u128::from(self.lower[axis]),
            upper: &|axis| {
                (self.upper[axis] != INFINITE_EXTREMUM).then_some(u128::from(self.upper[axis]))
            },
            positive: (
                u128::from(self.positive_lower),
                upper(POSITIVE_UPPER_NONE, self.positive_upper),
            ),
            numerator: (
                u128::from(self.numerator_lower),
                upper(NUMERATOR_UPPER_NONE, self.numerator_upper),
            ),
            difference: (
                signed(DIFFERENCE_LOWER_NONE, self.difference_lower),
                signed(DIFFERENCE_UPPER_NONE, self.difference_upper),
            ),
        }))
    }

    /// `bits::word` of the native summary this compact image encodes; None
    /// for a wide summary.
    pub fn word(&self) -> Option<u64> {
        if self.is_wide() {
            return None;
        }
        if self.flags & EMPTY != 0 {
            return Some(0);
        }
        let none = |bit: u8| self.flags & bit != 0;
        let mut word = 0_u64;
        for axis in 0..N.min(super::bits::MAX_ARITY) {
            word |= u64::from(self.upper[axis] == INFINITE_EXTREMUM) << axis;
            word |= u64::from(self.lower[axis] == 0) << (super::bits::MAX_ARITY + axis);
        }
        word |= u64::from(none(POSITIVE_UPPER_NONE)) << 32;
        word |= u64::from(none(NUMERATOR_UPPER_NONE)) << 33;
        word |= u64::from(none(DIFFERENCE_LOWER_NONE)) << 34;
        word |= u64::from(none(DIFFERENCE_UPPER_NONE)) << 35;
        word |= u64::from(self.positive_lower == 0) << 36;
        word |= u64::from(self.numerator_lower == 0) << 37;
        word |= u64::from(none(DIFFERENCE_LOWER_NONE) || self.difference_lower <= 0) << 38;
        Some(word)
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
        let upper = |bit: u8, value: u32| {
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
        let upper = |bit: u8, value: u32| (!none(bit)).then_some(u128::from(value));
        let signed = |bit: u8, value: i32| (!none(bit)).then_some(i128::from(value));
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
}

/// Query side of one admission: its native summary (block filters, the rare
/// wide comparison), the compact image compared on the hot path, and the
/// kernel's word and lanes. `bucket` is the query's (phase, owner) code.
#[derive(Clone)]
pub(in super::super) struct Query<const N: usize> {
    pub core: DomainPowerSummary<N>,
    pub compact: CompactSummary<N>,
    pub word: u64,
    pub lanes: Option<Lanes<N>>,
    pub bucket: (u8, u32),
}

impl<const N: usize> Query<N> {
    pub fn new(core: DomainPowerSummary<N>, phase: Phase) -> Self {
        let compact = CompactSummary::from_core(&core);
        Self {
            lanes: compact.lanes().or_else(|| Lanes::of_core(&core)),
            word: super::bits::word(&core),
            bucket: (
                match phase {
                    Phase::Apply => 0,
                    Phase::Route => 1,
                },
                owner_bits(core.owner()),
            ),
            compact,
            core,
        }
    }

    /// The kernel word and lanes this query is stored with when admitted.
    pub fn image(&self) -> (u64, Option<Lanes<N>>) {
        (self.word, self.lanes)
    }
}

/// The kernel word and lanes of an admitted ID, rebuilt from its immutable
/// summary (a wide one from its native summary): exactly what `Query::image`
/// gave at its admission.
pub(in super::super) fn stored_image<const N: usize>(
    domain: &CompactDomain<N>,
    summary: &CompactSummary<N>,
) -> Result<(u64, Option<Lanes<N>>), String> {
    match summary.word() {
        Some(word) => Ok((word, summary.lanes())),
        None => {
            let core = domain.try_native_summary().map_err(|e| e.to_string())?;
            Ok((super::bits::word(&core), Lanes::of_core(&core)))
        }
    }
}

/// Read-only view of the stored candidate geometry for containment callbacks.
#[derive(Clone, Copy)]
pub(in super::super) struct Stored<'a, const N: usize> {
    pub domains: &'a [CompactDomain<N>],
    pub summaries: &'a [CompactSummary<N>],
    /// Rescue quarantine bitset (`rescue.rs`); empty unless the walk was
    /// amended. A quarantined ID is never a forward (containing) positive.
    pub quarantine: &'a [u64],
}

/// Whether `id` is in the rescue quarantine bitset (false when it is empty).
#[inline]
pub(in super::super) fn quarantined(quarantine: &[u64], id: usize) -> bool {
    quarantine
        .get(id >> 6)
        .is_some_and(|word| word >> (id & 63) & 1 != 0)
}

impl<const N: usize> Stored<'_, N> {
    /// Exact native inclusion `stored[id] ⊇ query` within the query's
    /// (phase, owner) bucket (A1: a candidate of another bucket is never a
    /// positive, even for an empty query). A quarantined container is never
    /// a positive (the rescue keeps new work out of frontier-tainted cones).
    #[inline]
    pub fn contains(&self, id: usize, query: &Query<N>) -> bool {
        let (phase, owner) = query.bucket;
        (self.quarantine.is_empty() || !quarantined(self.quarantine, id))
            && self.domains[id].same_bucket(phase, owner)
            && self.summaries[id]
                .contains(&query.compact)
                .unwrap_or_else(|| self.domains[id].native_summary().contains(&query.core))
    }

    /// Exact native inclusion `query ⊇ stored[id]` within the query's bucket.
    #[inline]
    pub fn contained_by(&self, id: usize, query: &Query<N>) -> bool {
        let (phase, owner) = query.bucket;
        self.domains[id].same_bucket(phase, owner)
            && query
                .compact
                .contains(&self.summaries[id])
                .unwrap_or_else(|| query.core.contains(&self.domains[id].native_summary()))
    }

    /// `Signature::of` the native summary of an admitted ID.
    pub fn signature(&self, id: usize) -> Signature {
        self.summaries[id]
            .signature()
            .unwrap_or_else(|| Signature::of(&self.domains[id].native_summary()))
    }
}
