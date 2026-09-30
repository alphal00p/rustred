//! Struct-of-arrays candidate blocks: the admission kernel of the legacy lanes
//! (master plan §3.3 and amendment A5; v3 design stage S1).
//!
//! A group keeps its live candidates in blocks of at most 32, in increasing
//! admission-ID order. Each block has two parts:
//!
//! - `Meta`, stored contiguously per group: the ID bounds, the length and the
//!   historical coordinate envelope. A scan reads it sequentially and touches
//!   the boxed block only when the envelope admits the query.
//! - `Block`, boxed: the candidates as parallel arrays (u32 IDs, u64 filter
//!   words from `bits`, one u8 lane row per comparison field) plus the block
//!   OR/AND words.
//!
//! # Lanes (A5)
//!
//! Field order: N coordinate lowers, N coordinate uppers, then A lower, A
//! upper, R lower, R upper, D lower, D upper of the tight native extrema. Each
//! field is mapped monotonically to u8 so that native inclusion C ⊇ Q implies
//! `lane(C)[f] <= lane(Q)[f]` for every field f:
//!
//! | field | code |
//! |---|---|
//! | coordinate, A, R lower | `min(v, 255)` |
//! | coordinate, A, R upper | +inf -> 0, finite v -> `255 - min(v, 254)` |
//! | D lower (bias 128) | -inf -> 0, finite d -> `clamp(d + 128, 1, 255)` |
//! | D upper (bias 128) | +inf -> 0, finite d -> `255 - clamp(d + 128, 0, 254)` |
//!
//! A saturating monotone map preserves every non-strict inequality, and an
//! infinity keeps a code no finite value takes, so the lane test is a
//! necessary condition in both directions and for every value range: forward
//! (stored candidate = container) `lane(C) <= lane(Q)`, reverse (query =
//! container) `lane(Q) <= lane(C)`. A value outside the exact range (a
//! coordinate, A or R value above 254, a D lower outside -127..=127, a D upper
//! outside -128..=126) saturates and marks its summary `lossy`, which can only
//! admit false positives. This normalisation is what the raw A5 encoding needs
//! its two escape rules for (a forward query value above 254 saturating to the
//! +inf code, a reverse finite container bound above 254 taking the scalar
//! path): here a saturated finite bound never shares a code with an infinity,
//! so both rules hold by construction. Escape lists: a candidate without lanes
//! (an empty summary) passes every lane test (`escape` bit), and a query
//! without lanes (an empty query) tests every candidate that passes the word.
//!
//! Lanes and words are only a prefilter. Every candidate that passes them is
//! decided by the exact predicate (A1); for two non-lossy nonempty summaries
//! of the same phase/owner the lane test coincides with native inclusion.
//!
//! # The historical envelope
//!
//! The per-block coordinate envelope (min/max of the tight coordinate lowers
//! and uppers of every candidate ever inserted, never narrowed by retirement)
//! is persisted by CP5 and decides which candidates a lookup charges to
//! `containment_checks`, so it is kept exactly: as u16 codes (0xFFFF = +inf)
//! while every value is at most 65534, as the exact u64 form otherwise.

use rustred::solver::DomainPowerSummary;

pub(super) const BLOCK_SIZE: usize = 32;
/// A lower, A upper, R lower, R upper, D lower, D upper.
pub(super) const POWER_LANES: usize = 6;
/// Largest value a narrow envelope code stores; `NARROW_NONE` is +infinity.
const NARROW_MAX: u64 = u16::MAX as u64 - 1;
const NARROW_NONE: u16 = u16::MAX;

#[derive(Clone, Copy)]
pub(in super::super::super) struct Coordinates<'a> {
    lower: &'a [u64],
    upper: &'a [Option<u64>],
}

impl<'a> Coordinates<'a> {
    pub(in super::super::super) fn of<const N: usize>(
        summary: &'a DomainPowerSummary<N>,
    ) -> Option<Self> {
        summary.extrema().map(|extrema| Self {
            lower: extrema.lower(),
            upper: extrema.upper(),
        })
    }
}

// ---- lanes ------------------------------------------------------------------

/// Normalised u8 image of a nonempty summary's comparison fields (see the
/// module documentation). `lossy` records whether any field saturated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super::super) struct Lanes<const N: usize> {
    pub lower: [u8; N],
    pub upper: [u8; N],
    pub power: [u8; POWER_LANES],
    pub lossy: bool,
}

/// One lane-building step per field; `lossy` accumulates saturation.
struct LaneWriter {
    lossy: bool,
}

impl LaneWriter {
    fn lower(&mut self, value: u128) -> u8 {
        self.lossy |= value > 254;
        value.min(255) as u8
    }
    fn upper(&mut self, value: Option<u128>) -> u8 {
        value.map_or(0, |value| {
            self.lossy |= value > 254;
            255 - value.min(254) as u8
        })
    }
    fn difference_lower(&mut self, value: Option<i128>) -> u8 {
        value.map_or(0, |value| {
            self.lossy |= !(-127..=127).contains(&value);
            (value.clamp(-127, 127) + 128) as u8
        })
    }
    fn difference_upper(&mut self, value: Option<i128>) -> u8 {
        value.map_or(0, |value| {
            self.lossy |= !(-128..=126).contains(&value);
            255 - (value.clamp(-128, 126) + 128) as u8
        })
    }
}

/// The tight extrema a lane image is built from, in native widths.
pub(in super::super::super) struct LaneSource<'a, const N: usize> {
    pub lower: &'a dyn Fn(usize) -> u128,
    pub upper: &'a dyn Fn(usize) -> Option<u128>,
    pub positive: (u128, Option<u128>),
    pub numerator: (u128, Option<u128>),
    pub difference: (Option<i128>, Option<i128>),
}

impl<const N: usize> Lanes<N> {
    pub(in super::super) fn build(source: LaneSource<'_, N>) -> Self {
        let mut w = LaneWriter { lossy: false };
        let lower = std::array::from_fn(|axis| w.lower((source.lower)(axis)));
        let upper = std::array::from_fn(|axis| w.upper((source.upper)(axis)));
        let power = [
            w.lower(source.positive.0),
            w.upper(source.positive.1),
            w.lower(source.numerator.0),
            w.upper(source.numerator.1),
            w.difference_lower(source.difference.0),
            w.difference_upper(source.difference.1),
        ];
        Self {
            lower,
            upper,
            power,
            lossy: w.lossy,
        }
    }

    /// Lanes of a native summary; None for an empty one.
    pub(in super::super) fn of_core(summary: &DomainPowerSummary<N>) -> Option<Self> {
        let extrema = summary.extrema()?;
        Some(Self::build(LaneSource {
            lower: &|axis| u128::from(extrema.lower()[axis]),
            upper: &|axis| extrema.upper()[axis].map(u128::from),
            positive: extrema.positive_power(),
            numerator: extrema.numerator_rank(),
            difference: extrema.power_difference(),
        }))
    }

    /// Forward necessary condition `C ⊇ Q` for `self = C`, `query = Q`.
    #[cfg(test)]
    pub(in super::super) fn may_contain(&self, query: &Self) -> bool {
        let le = |a: &[u8], b: &[u8]| a.iter().zip(b).all(|(a, b)| a <= b);
        le(&self.lower, &query.lower)
            && le(&self.upper, &query.upper)
            && le(&self.power, &query.power)
    }
}

// ---- the historical envelope ------------------------------------------------

/// The persisted (CP5) form of one envelope axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(in super::super) struct AxisEnvelope {
    pub min_lower: u64,
    pub max_lower: u64,
    pub min_upper: Option<u64>,
    pub max_upper: Option<u64>,
}

impl AxisEnvelope {
    fn point(lower: u64, upper: Option<u64>) -> Self {
        Self {
            min_lower: lower,
            max_lower: lower,
            min_upper: upper,
            max_upper: upper,
        }
    }

    fn widen(&mut self, lower: u64, upper: Option<u64>) {
        self.min_lower = self.min_lower.min(lower);
        self.max_lower = self.max_lower.max(lower);
        self.min_upper = match (self.min_upper, upper) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        self.max_upper = match (self.max_upper, upper) {
            (Some(a), Some(b)) => Some(a.max(b)),
            _ => None,
        };
    }

    fn may_contain(&self, lower: u64, upper: Option<u64>) -> bool {
        self.min_lower <= lower && upper_contains(self.max_upper, upper)
    }

    fn may_be_contained(&self, lower: u64, upper: Option<u64>) -> bool {
        lower <= self.max_lower && upper_contains(upper, self.min_upper)
    }
}

fn upper_contains(container: Option<u64>, candidate: Option<u64>) -> bool {
    container.is_none_or(|a| candidate.is_some_and(|b| a >= b))
}

fn narrow_lower(value: u64) -> Option<u16> {
    (value <= NARROW_MAX).then_some(value as u16)
}

fn narrow_upper(value: Option<u64>) -> Option<u16> {
    value.map_or(Some(NARROW_NONE), |value| {
        (value <= NARROW_MAX).then_some(value as u16)
    })
}

fn wide_upper(code: u16) -> Option<u64> {
    (code != NARROW_NONE).then_some(u64::from(code))
}

/// Exact u16 envelope codes. With +inf as the largest code, `min` and `max`
/// over codes are the historical Option rules, and the upper tests are plain
/// comparisons.
#[derive(Clone, Copy)]
struct Narrow<const N: usize> {
    min_lower: [u16; N],
    max_lower: [u16; N],
    min_upper: [u16; N],
    max_upper: [u16; N],
}

impl<const N: usize> Narrow<N> {
    fn axis(&self, axis: usize) -> AxisEnvelope {
        AxisEnvelope {
            min_lower: u64::from(self.min_lower[axis]),
            max_lower: u64::from(self.max_lower[axis]),
            min_upper: wide_upper(self.min_upper[axis]),
            max_upper: wide_upper(self.max_upper[axis]),
        }
    }
}

enum Envelope<const N: usize> {
    /// Prepared without coordinates (an empty summary): never filters.
    Absent,
    Narrow(Narrow<N>),
    Wide(Box<[AxisEnvelope]>),
}

/// Fallible exact envelope storage for coordinates beyond the narrow codes.
fn wide_storage(
    axes: impl ExactSizeIterator<Item = AxisEnvelope>,
) -> Result<Box<[AxisEnvelope]>, &'static str> {
    let mut storage = Vec::new();
    storage
        .try_reserve_exact(axes.len())
        .map_err(|_| "coordinate block envelope allocation")?;
    storage.extend(axes);
    Ok(storage.into_boxed_slice())
}

impl<const N: usize> Envelope<N> {
    fn prepare(coordinates: Option<Coordinates<'_>>) -> Result<Self, &'static str> {
        let Some(coordinates) = coordinates else {
            return Ok(Self::Absent);
        };
        debug_assert_eq!(coordinates.lower.len(), N);
        match NarrowCoordinates::<N>::of(coordinates) {
            Some(point) => Ok(Self::Narrow(Narrow {
                min_lower: point.lower,
                max_lower: point.lower,
                min_upper: point.upper,
                max_upper: point.upper,
            })),
            None => Ok(Self::Wide(wide_storage(
                coordinates
                    .lower
                    .iter()
                    .zip(coordinates.upper)
                    .map(|(&lower, &upper)| AxisEnvelope::point(lower, upper)),
            )?)),
        }
    }

    /// Restore the persisted form; its length is N, or 0 without coordinates.
    fn restore(axes: &[AxisEnvelope]) -> Result<Self, String> {
        if axes.is_empty() {
            return Ok(Self::Absent);
        }
        if axes.len() != N {
            return Err("checkpoint coordinate block envelope arity".into());
        }
        let narrow = (|| {
            let mut out = Narrow {
                min_lower: [0; N],
                max_lower: [0; N],
                min_upper: [0; N],
                max_upper: [0; N],
            };
            for (axis, e) in axes.iter().enumerate() {
                out.min_lower[axis] = narrow_lower(e.min_lower)?;
                out.max_lower[axis] = narrow_lower(e.max_lower)?;
                out.min_upper[axis] = narrow_upper(e.min_upper)?;
                out.max_upper[axis] = narrow_upper(e.max_upper)?;
            }
            Some(out)
        })();
        match narrow {
            Some(narrow) => Ok(Self::Narrow(narrow)),
            None => wide_storage(axes.iter().copied())
                .map(Self::Wide)
                .map_err(str::to_string),
        }
    }

    /// Whether widening by `coordinates` leaves the narrow form, so that the
    /// insertion must carry preallocated exact storage.
    fn needs_wide_storage(&self, coordinates: Option<Coordinates<'_>>) -> bool {
        matches!(self, Self::Narrow(_))
            && coordinates.is_some_and(|c| NarrowCoordinates::<N>::of(c).is_none())
    }

    /// The historical widening (only outwards); `spare` is the storage a
    /// narrow envelope needs to become exact.
    fn widen(&mut self, coordinates: Option<Coordinates<'_>>, spare: Option<Box<[AxisEnvelope]>>) {
        let Some(coordinates) = coordinates else {
            return;
        };
        if let Self::Narrow(narrow) = self {
            if let Some(point) = NarrowCoordinates::<N>::of(coordinates) {
                for axis in 0..N {
                    narrow.min_lower[axis] = narrow.min_lower[axis].min(point.lower[axis]);
                    narrow.max_lower[axis] = narrow.max_lower[axis].max(point.lower[axis]);
                    narrow.min_upper[axis] = narrow.min_upper[axis].min(point.upper[axis]);
                    narrow.max_upper[axis] = narrow.max_upper[axis].max(point.upper[axis]);
                }
                return;
            }
            let mut storage = spare.expect("preallocated exact envelope storage");
            debug_assert_eq!(storage.len(), N);
            for (axis, slot) in storage.iter_mut().enumerate() {
                *slot = narrow.axis(axis);
            }
            *self = Self::Wide(storage);
        }
        if let Self::Wide(axes) = self {
            for (axis, (&lower, &upper)) in axes
                .iter_mut()
                .zip(coordinates.lower.iter().zip(coordinates.upper))
            {
                axis.widen(lower, upper);
            }
        }
    }

    fn may_contain(&self, probe: &Probe<'_, N>) -> bool {
        let Some(coordinates) = probe.coordinates else {
            return true;
        };
        match self {
            Self::Absent => true,
            Self::Narrow(e) => match &probe.narrow {
                Some(q) => {
                    let mut ok = true;
                    for axis in 0..N {
                        ok &= e.min_lower[axis] <= q.lower[axis];
                        ok &= e.max_upper[axis] >= q.upper[axis];
                    }
                    ok
                }
                None => (0..N).all(|axis| {
                    e.axis(axis)
                        .may_contain(coordinates.lower[axis], coordinates.upper[axis])
                }),
            },
            Self::Wide(axes) => axes
                .iter()
                .zip(coordinates.lower.iter().zip(coordinates.upper))
                .all(|(axis, (&lower, &upper))| axis.may_contain(lower, upper)),
        }
    }

    fn may_be_contained(&self, probe: &Probe<'_, N>) -> bool {
        let Some(coordinates) = probe.coordinates else {
            return true;
        };
        match self {
            Self::Absent => true,
            Self::Narrow(e) => match &probe.narrow {
                Some(q) => {
                    let mut ok = true;
                    for axis in 0..N {
                        ok &= q.lower[axis] <= e.max_lower[axis];
                        ok &= q.upper[axis] >= e.min_upper[axis];
                    }
                    ok
                }
                None => (0..N).all(|axis| {
                    e.axis(axis)
                        .may_be_contained(coordinates.lower[axis], coordinates.upper[axis])
                }),
            },
            Self::Wide(axes) => axes
                .iter()
                .zip(coordinates.lower.iter().zip(coordinates.upper))
                .all(|(axis, (&lower, &upper))| axis.may_be_contained(lower, upper)),
        }
    }

    /// The persisted form: N axes, or none without coordinates.
    fn axes(&self) -> impl ExactSizeIterator<Item = AxisEnvelope> + '_ {
        let count = match self {
            Self::Absent => 0,
            _ => N,
        };
        (0..count).map(move |axis| match self {
            Self::Absent => unreachable!("an absent envelope has no axes"),
            Self::Narrow(narrow) => narrow.axis(axis),
            Self::Wide(axes) => axes[axis],
        })
    }

    fn capacity_bytes(&self) -> usize {
        match self {
            Self::Wide(axes) => std::mem::size_of_val::<[AxisEnvelope]>(axes),
            _ => 0,
        }
    }
}

/// A query's coordinates as narrow codes, when every one fits.
#[derive(Clone, Copy)]
struct NarrowCoordinates<const N: usize> {
    lower: [u16; N],
    upper: [u16; N],
}

impl<const N: usize> NarrowCoordinates<N> {
    fn of(coordinates: Coordinates<'_>) -> Option<Self> {
        if coordinates.lower.len() != N || coordinates.upper.len() != N {
            return None;
        }
        let mut out = Self {
            lower: [0; N],
            upper: [0; N],
        };
        for axis in 0..N {
            out.lower[axis] = narrow_lower(coordinates.lower[axis])?;
            out.upper[axis] = narrow_upper(coordinates.upper[axis])?;
        }
        Some(out)
    }
}

// ---- the query side ---------------------------------------------------------

/// Everything a scan needs from one query, computed once per lookup.
pub(in super::super::super) struct Probe<'a, const N: usize> {
    coordinates: Option<Coordinates<'a>>,
    narrow: Option<NarrowCoordinates<N>>,
    word: u64,
    lanes: Option<Lanes<N>>,
    /// False only for the test seam that proves results do not depend on
    /// the prefilter: every charged candidate then reaches the predicate.
    filter: bool,
}

impl<'a, const N: usize> Probe<'a, N> {
    pub(in super::super::super) fn new(
        coordinates: Option<Coordinates<'a>>,
        word: u64,
        lanes: Option<Lanes<N>>,
        filter: bool,
    ) -> Self {
        Self {
            coordinates,
            narrow: coordinates.and_then(NarrowCoordinates::of),
            word,
            lanes,
            filter,
        }
    }

    /// Only the historical envelope filters; every charged candidate is
    /// tested (the pre-kernel callback contract, for index-level tests).
    #[cfg(test)]
    pub(in super::super) fn unfiltered(coordinates: Option<Coordinates<'a>>) -> Self {
        Self::new(coordinates, 0, None, false)
    }
}

// ---- blocks -----------------------------------------------------------------

/// Sequential per-block row of a group (see the module documentation).
pub(super) struct Meta<const N: usize> {
    /// Smallest and largest live ID; `u32::MAX` for an empty block, which
    /// keeps `last` monotone across a group for `partition_point`.
    pub first: u32,
    pub last: u32,
    pub len: u32,
    envelope: Envelope<N>,
}

/// The candidates of one block, struct-of-arrays. Slots at or beyond the
/// length are dead; `ids` keeps the historical stale values there (retain
/// compacts in place), so a CP5 image round-trips byte for byte.
#[repr(C, align(64))]
#[derive(Clone)]
pub(super) struct Block<const N: usize> {
    words: [u64; BLOCK_SIZE],
    pub ids: [u32; BLOCK_SIZE],
    power: [[u8; BLOCK_SIZE]; POWER_LANES],
    lower: [[u8; BLOCK_SIZE]; N],
    upper: [[u8; BLOCK_SIZE]; N],
    /// Union and intersection of the live words (0 and all ones when empty).
    or_word: u64,
    and_word: u64,
    /// Live slots without lanes (empty summaries): they pass every lane test.
    escape: u32,
    /// Live slots whose lanes saturate (bookkeeping for the exactness claim).
    lossy: u32,
}

/// A boxed block: blocks move as pointers when a group drops an empty one.
pub(super) type BlockBox<const N: usize> = Box<[Block<N>; 1]>;

/// One candidate as the index stores it.
pub(in super::super::super) struct Entry<'a, const N: usize> {
    pub id: usize,
    pub coordinates: Option<Coordinates<'a>>,
    pub word: u64,
    pub lanes: Option<Lanes<N>>,
}

#[inline]
fn len_mask(len: usize) -> u32 {
    if len >= BLOCK_SIZE {
        u32::MAX
    } else {
        (1_u32 << len) - 1
    }
}

/// Positions `start..end` as a mask.
#[inline]
pub(super) fn range_mask(start: usize, end: usize) -> u32 {
    len_mask(end) & !len_mask(start)
}

/// `acc[k]` keeps 0xFF only where `row[k] <= value` for every row.
#[inline]
fn fold_le(rows: &[[u8; BLOCK_SIZE]], values: &[u8], acc: &mut [u8; BLOCK_SIZE]) {
    for (row, &value) in rows.iter().zip(values) {
        for k in 0..BLOCK_SIZE {
            acc[k] &= 0_u8.wrapping_sub(u8::from(row[k] <= value));
        }
    }
}

/// `acc[k]` keeps 0xFF only where `row[k] >= value` for every row.
#[inline]
fn fold_ge(rows: &[[u8; BLOCK_SIZE]], values: &[u8], acc: &mut [u8; BLOCK_SIZE]) {
    for (row, &value) in rows.iter().zip(values) {
        for k in 0..BLOCK_SIZE {
            acc[k] &= 0_u8.wrapping_sub(u8::from(row[k] >= value));
        }
    }
}

#[inline]
fn any(acc: &[u8; BLOCK_SIZE]) -> bool {
    acc.iter().fold(0, |bits, &byte| bits | byte) != 0
}

#[inline]
fn mask_of(acc: &[u8; BLOCK_SIZE]) -> u32 {
    acc.iter()
        .enumerate()
        .fold(0, |mask, (k, &byte)| mask | (u32::from(byte >> 7) << k))
}

/// Fallible single allocation of a block (no abort on exhaustion).
fn try_box<const N: usize>(block: Block<N>) -> Result<BlockBox<N>, &'static str> {
    let mut storage = Vec::new();
    storage
        .try_reserve_exact(1)
        .map_err(|_| "coordinate block allocation")?;
    storage.push(block);
    storage
        .into_boxed_slice()
        .try_into()
        .map_err(|_| "coordinate block allocation")
}

impl<const N: usize> Block<N> {
    fn empty() -> Self {
        Self {
            words: [0; BLOCK_SIZE],
            ids: [0; BLOCK_SIZE],
            power: [[0; BLOCK_SIZE]; POWER_LANES],
            lower: [[0; BLOCK_SIZE]; N],
            upper: [[0; BLOCK_SIZE]; N],
            or_word: 0,
            and_word: u64::MAX,
            escape: 0,
            lossy: 0,
        }
    }

    /// (word mask, prefilter mask) over the live slots for forward
    /// containment `stored ⊇ query`; the prefilter mask is word AND lanes.
    #[inline]
    pub(super) fn forward(&self, probe: &Probe<'_, N>, len: usize) -> (u32, u32) {
        let live = len_mask(len);
        if !probe.filter {
            return (live, live);
        }
        if probe.word & !self.or_word != 0 {
            // Every live word misses a query bit.
            return (0, 0);
        }
        let mut words = 0;
        for k in 0..BLOCK_SIZE {
            words |= u32::from(probe.word & !self.words[k] == 0) << k;
        }
        let words = words & live;
        let Some(lanes) = &probe.lanes else {
            return (words, words);
        };
        if words & !self.escape == 0 {
            return (words, words);
        }
        let mut acc = [u8::MAX; BLOCK_SIZE];
        fold_le(&self.power, &lanes.power, &mut acc);
        if any(&acc) {
            fold_le(&self.lower, &lanes.lower, &mut acc);
            if any(&acc) {
                fold_le(&self.upper, &lanes.upper, &mut acc);
            }
        }
        (words, words & (mask_of(&acc) | self.escape))
    }

    /// As `forward`, for reverse containment `query ⊇ stored`.
    #[inline]
    pub(super) fn reverse(&self, probe: &Probe<'_, N>, len: usize) -> (u32, u32) {
        let live = len_mask(len);
        if !probe.filter {
            return (live, live);
        }
        if self.and_word & !probe.word != 0 {
            // Every live word has a bit the query lacks.
            return (0, 0);
        }
        let mut words = 0;
        for k in 0..BLOCK_SIZE {
            words |= u32::from(self.words[k] & !probe.word == 0) << k;
        }
        let words = words & live;
        let Some(lanes) = &probe.lanes else {
            return (words, words);
        };
        if words & !self.escape == 0 {
            return (words, words);
        }
        let mut acc = [u8::MAX; BLOCK_SIZE];
        fold_ge(&self.power, &lanes.power, &mut acc);
        if any(&acc) {
            fold_ge(&self.lower, &lanes.lower, &mut acc);
            if any(&acc) {
                fold_ge(&self.upper, &lanes.upper, &mut acc);
            }
        }
        (words, words & (mask_of(&acc) | self.escape))
    }

    fn put(&mut self, slot: usize, id: u32, word: u64, lanes: Option<&Lanes<N>>) {
        self.ids[slot] = id;
        self.words[slot] = word;
        let bit = 1_u32 << slot;
        self.escape &= !bit;
        self.lossy &= !bit;
        match lanes {
            Some(lanes) => {
                for (row, &value) in self.power.iter_mut().zip(&lanes.power) {
                    row[slot] = value;
                }
                for (row, &value) in self.lower.iter_mut().zip(&lanes.lower) {
                    row[slot] = value;
                }
                for (row, &value) in self.upper.iter_mut().zip(&lanes.upper) {
                    row[slot] = value;
                }
                if lanes.lossy {
                    self.lossy |= bit;
                }
            }
            None => self.escape |= bit,
        }
    }

    /// Move live slot `from` to slot `to < from`, IDs included.
    fn relocate(&mut self, from: usize, to: usize) {
        self.ids[to] = self.ids[from];
        self.words[to] = self.words[from];
        for row in self
            .power
            .iter_mut()
            .chain(self.lower.iter_mut())
            .chain(self.upper.iter_mut())
        {
            row[to] = row[from];
        }
        let (from_bit, to_bit) = (1_u32 << from, 1_u32 << to);
        for mask in [&mut self.escape, &mut self.lossy] {
            *mask = (*mask & !to_bit) | if *mask & from_bit != 0 { to_bit } else { 0 };
        }
    }

    fn refresh_words(&mut self, len: usize) {
        self.or_word = self.words[..len].iter().fold(0, |a, &w| a | w);
        self.and_word = self.words[..len].iter().fold(u64::MAX, |a, &w| a & w);
        let live = len_mask(len);
        self.escape &= live;
        self.lossy &= live;
    }

    pub(super) fn lossy(&self, len: usize) -> u32 {
        self.lossy & len_mask(len)
    }

    #[cfg(test)]
    pub(super) fn escaped(&self, len: usize) -> u32 {
        self.escape & len_mask(len)
    }

    /// The stored candidate in `slot`: ID, word and lanes (None if escaped).
    #[cfg(test)]
    pub(super) fn entry(&self, slot: usize) -> (u32, u64, Option<Lanes<N>>) {
        let bit = 1_u32 << slot;
        let lanes = (self.escape & bit == 0).then(|| Lanes {
            lower: std::array::from_fn(|axis| self.lower[axis][slot]),
            upper: std::array::from_fn(|axis| self.upper[axis][slot]),
            power: std::array::from_fn(|field| self.power[field][slot]),
            lossy: self.lossy & bit != 0,
        });
        (self.ids[slot], self.words[slot], lanes)
    }

    #[cfg(test)]
    pub(super) fn block_words(&self) -> (u64, u64) {
        (self.or_word, self.and_word)
    }
}

impl<const N: usize> Meta<N> {
    /// A new empty block row and its storage (`checkpoint` precedes the one
    /// allocation, as the historical envelope reservation did).
    pub(super) fn prepare(
        coordinates: Option<Coordinates<'_>>,
        checkpoint: &mut impl FnMut() -> Result<(), &'static str>,
    ) -> Result<(Self, BlockBox<N>), &'static str> {
        checkpoint()?;
        let envelope = Envelope::prepare(coordinates)?;
        let block = try_box(Block::empty())?;
        Ok((
            Self {
                first: u32::MAX,
                last: u32::MAX,
                len: 0,
                envelope,
            },
            block,
        ))
    }

    pub(super) fn has_room(&self) -> bool {
        (self.len as usize) < BLOCK_SIZE
    }

    pub(super) fn needs_wide_storage(&self, coordinates: Option<Coordinates<'_>>) -> bool {
        self.envelope.needs_wide_storage(coordinates)
    }

    pub(super) fn wide_storage(
        coordinates: Coordinates<'_>,
    ) -> Result<Box<[AxisEnvelope]>, &'static str> {
        wide_storage(
            coordinates
                .lower
                .iter()
                .zip(coordinates.upper)
                .map(|(&lower, &upper)| AxisEnvelope::point(lower, upper)),
        )
    }

    pub(super) fn may_contain(&self, probe: &Probe<'_, N>) -> bool {
        self.envelope.may_contain(probe)
    }

    pub(super) fn may_be_contained(&self, probe: &Probe<'_, N>) -> bool {
        self.envelope.may_be_contained(probe)
    }

    /// Append one candidate (ID above every current one).
    pub(super) fn push(
        &mut self,
        block: &mut Block<N>,
        entry: Entry<'_, N>,
        spare: Option<Box<[AxisEnvelope]>>,
    ) {
        debug_assert!(self.has_room());
        let id = u32::try_from(entry.id).expect("indexed candidate IDs are below u32::MAX");
        debug_assert!(self.len == 0 || self.last < id);
        self.envelope.widen(entry.coordinates, spare);
        let slot = self.len as usize;
        block.put(slot, id, entry.word, entry.lanes.as_ref());
        block.or_word |= entry.word;
        block.and_word &= entry.word;
        if self.len == 0 {
            self.first = id;
        }
        self.last = id;
        self.len += 1;
    }

    /// Keep the live slots whose bit is set in `keep`, in order; returns how
    /// many were removed. The envelope is deliberately not narrowed.
    pub(super) fn retain(&mut self, block: &mut Block<N>, keep: u32) -> usize {
        let len = self.len as usize;
        let keep = keep & len_mask(len);
        if keep == len_mask(len) {
            return 0;
        }
        let mut retained = 0;
        for slot in 0..len {
            if keep & (1 << slot) != 0 {
                if slot != retained {
                    block.relocate(slot, retained);
                }
                retained += 1;
            }
        }
        block.refresh_words(retained);
        self.len = retained as u32;
        if retained == 0 {
            self.first = u32::MAX;
            self.last = u32::MAX;
        } else {
            self.first = block.ids[0];
            self.last = block.ids[retained - 1];
        }
        len - retained
    }

    /// Validate and rebuild one persisted block. `entry` supplies the stored
    /// word and lanes of each live ID.
    pub(super) fn restore(
        ids: &[usize; BLOCK_SIZE],
        len: usize,
        envelope: &[AxisEnvelope],
        domain_count: usize,
        mut entry: impl FnMut(usize) -> Result<(u64, Option<Lanes<N>>), String>,
    ) -> Result<(Self, BlockBox<N>), String> {
        if len > BLOCK_SIZE
            || ids[..len]
                .iter()
                .any(|&id| id >= domain_count || id >= u32::MAX as usize)
        {
            return Err("invalid checkpoint coordinate block".into());
        }
        let mut block = try_box(Block::empty()).map_err(str::to_string)?;
        let storage = &mut block[0];
        for (slot, &id) in ids.iter().enumerate() {
            if slot < len {
                let (word, lanes) = entry(id)?;
                storage.put(slot, id as u32, word, lanes.as_ref());
            } else {
                // Dead slot: keep the historical stale ID exactly, or refuse.
                storage.ids[slot] = u32::try_from(id)
                    .ok()
                    .filter(|&id| (id as usize) < domain_count)
                    .ok_or("invalid checkpoint stale block slot")?;
            }
        }
        storage.refresh_words(len);
        let meta = Self {
            first: if len == 0 { u32::MAX } else { ids[0] as u32 },
            last: if len == 0 {
                u32::MAX
            } else {
                ids[len - 1] as u32
            },
            len: len as u32,
            envelope: Envelope::restore(envelope)?,
        };
        Ok((meta, block))
    }

    /// The persisted envelope axes.
    pub(super) fn envelope_axes(&self) -> impl ExactSizeIterator<Item = AxisEnvelope> + '_ {
        self.envelope.axes()
    }

    pub(super) fn envelope_capacity_bytes(&self) -> usize {
        self.envelope.capacity_bytes()
    }
}

#[cfg(test)]
mod kernel_tests;
