//! Today's layout (`walking/queue/index.rs`, `index/blocks.rs`, `bits.rs`,
//! `compact.rs` at fable_5_1 = 4a17f9c7), replicated type for type so that
//! memory traffic matches: per bucket a Vec of signature groups, each a Vec of
//! 32-slot blocks (`[usize; 32]` ids + a heap `Vec<AxisEnvelope>` of 48-byte
//! u64/Option<u64> envelopes); per ID a `bits: Vec<u64>` filter word and a
//! `slots: Vec<u32>` map into a slab of 176-byte `CompactSummary<15>` entries.
//! The forward scan is `AggregateIndex::find_controlled` (minimum ID, with its
//! block-order breaks) or the same traversal stopping at the first container.
use crate::ckpt::{N, NEG, POS, Sig, T, U16INF, UINF};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Upper {
    Finite(u128),
    Infinity,
}
impl Upper {
    #[inline]
    fn contains(self, other: Self) -> bool {
        match (self, other) {
            (Self::Infinity, _) => true,
            (Self::Finite(a), Self::Finite(b)) => a >= b,
            (Self::Finite(_), Self::Infinity) => false,
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lower {
    NegativeInfinity,
    Finite(i128),
}
impl Lower {
    #[inline]
    fn contains(self, other: Self) -> bool {
        match (self, other) {
            (Self::NegativeInfinity, _) => true,
            (Self::Finite(a), Self::Finite(b)) => a <= b,
            (Self::Finite(_), Self::NegativeInfinity) => false,
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Signature {
    Empty,
    Nonempty {
        positive: Upper,
        numerator: Upper,
        difference: Lower,
    },
}
impl Signature {
    pub fn of(s: &Sig) -> Self {
        if s.empty {
            return Self::Empty;
        }
        let up = |v: u32| {
            if v == UINF {
                Upper::Infinity
            } else {
                Upper::Finite(v as u128)
            }
        };
        Self::Nonempty {
            positive: up(s.pos),
            numerator: up(s.num),
            difference: if s.dif == NEG {
                Lower::NegativeInfinity
            } else {
                Lower::Finite(s.dif as i128)
            },
        }
    }
    #[inline]
    pub fn may_contain(self, other: Self) -> bool {
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

pub struct AxisEnvelope {
    pub min_lower: u64,
    pub max_lower: u64,
    pub min_upper: Option<u64>,
    pub max_upper: Option<u64>,
}

pub const BLOCK_SIZE: usize = 32;

pub struct Block {
    pub ids: [usize; BLOCK_SIZE],
    pub len: usize,
    pub envelope: Vec<AxisEnvelope>,
}

fn upper_contains(container: Option<u64>, candidate: Option<u64>) -> bool {
    container.is_none_or(|a| candidate.is_some_and(|b| a >= b))
}

impl Block {
    #[inline]
    pub fn ids(&self) -> &[usize] {
        &self.ids[..self.len]
    }
    #[inline]
    fn may_contain(&self, q: Option<&Coords>) -> bool {
        q.is_none_or(|q| {
            self.envelope
                .iter()
                .zip(q.lower.iter().zip(&q.upper))
                .all(|(axis, (&lower, &upper))| axis.min_lower <= lower && upper_contains(axis.max_upper, upper))
        })
    }
    #[inline]
    fn may_be_contained(&self, q: Option<&Coords>) -> bool {
        q.is_none_or(|q| {
            self.envelope
                .iter()
                .zip(q.lower.iter().zip(&q.upper))
                .all(|(axis, (&lower, &upper))| lower <= axis.max_lower && upper_contains(upper, axis.min_upper))
        })
    }
}

pub struct Group {
    pub signature: Signature,
    pub blocks: Vec<Block>,
    pub live: usize,
}

pub struct Bucket {
    pub groups: Vec<Group>,
}

// CompactSummary<15> flags.
const EMPTY: u8 = 1;
const POSITIVE_UPPER_NONE: u8 = 1 << 1;
const NUMERATOR_UPPER_NONE: u8 = 1 << 2;
const DIFFERENCE_LOWER_NONE: u8 = 1 << 3;
const DIFFERENCE_UPPER_NONE: u8 = 1 << 4;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CompactSummary {
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
const _: () = assert!(std::mem::size_of::<CompactSummary>() == 176);

impl CompactSummary {
    pub fn of(t: &T) -> Self {
        let mut s = CompactSummary {
            flags: 0,
            owner: t.owner as u32,
            lower: [0; N],
            upper: [u32::MAX; N],
            positive_lower: 0,
            positive_upper: 0,
            numerator_lower: 0,
            numerator_upper: 0,
            difference_lower: 0,
            difference_upper: 0,
        };
        if t.empty {
            s.flags = EMPTY;
            s.upper = [0; N];
            return s;
        }
        for i in 0..N {
            s.lower[i] = t.lo[i] as u32;
            s.upper[i] = if t.up[i] == U16INF { u32::MAX } else { t.up[i] as u32 };
        }
        s.positive_lower = t.pl as u64;
        if t.pu == UINF {
            s.flags |= POSITIVE_UPPER_NONE;
        } else {
            s.positive_upper = t.pu as u64;
        }
        s.numerator_lower = t.nl as u64;
        if t.nu == UINF {
            s.flags |= NUMERATOR_UPPER_NONE;
        } else {
            s.numerator_upper = t.nu as u64;
        }
        if t.dl == NEG {
            s.flags |= DIFFERENCE_LOWER_NONE;
        } else {
            s.difference_lower = t.dl as i64;
        }
        if t.du == POS {
            s.flags |= DIFFERENCE_UPPER_NONE;
        } else {
            s.difference_upper = t.du as i64;
        }
        s
    }

    /// `CompactSummary::contains`, verbatim logic.
    #[inline]
    pub fn contains(&self, candidate: &Self) -> bool {
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
            && upper(POSITIVE_UPPER_NONE, self.positive_upper, candidate.positive_upper)
            && self.numerator_lower <= candidate.numerator_lower
            && upper(NUMERATOR_UPPER_NONE, self.numerator_upper, candidate.numerator_upper)
            && (none(self, DIFFERENCE_LOWER_NONE)
                || (!none(candidate, DIFFERENCE_LOWER_NONE) && candidate.difference_lower >= self.difference_lower))
            && (none(self, DIFFERENCE_UPPER_NONE)
                || (!none(candidate, DIFFERENCE_UPPER_NONE) && candidate.difference_upper <= self.difference_upper))
            && self.lower.iter().zip(&candidate.lower).all(|(a, b)| a <= b)
            && self.upper.iter().zip(&candidate.upper).all(|(a, b)| b <= a)
    }
}

/// `Coordinates` (tight extrema as u64 / Option<u64>).
pub struct Coords {
    pub lower: [u64; N],
    pub upper: [Option<u64>; N],
}

pub struct Query {
    pub compact: CompactSummary,
    pub word: u64,
    pub signature: Signature,
    pub coords: Option<Coords>,
}
impl Query {
    pub fn of(t: &T) -> Self {
        Query {
            compact: CompactSummary::of(t),
            word: t.word(),
            signature: Signature::of(&Sig::of(t)),
            coords: (!t.empty).then(|| Coords {
                lower: std::array::from_fn(|i| t.lo[i] as u64),
                upper: std::array::from_fn(|i| (t.up[i] != U16INF).then_some(t.up[i] as u64)),
            }),
        }
    }
}

pub struct L0 {
    pub buckets: Vec<Bucket>,
    pub bits: Vec<u64>,
    pub slots: Vec<u32>,
    pub entries: Vec<CompactSummary>,
}

#[derive(Default, Clone, Copy, Debug)]
pub struct Work {
    pub groups: u64,
    pub groups_eligible: u64,
    pub blocks: u64,
    pub blocks_passed: u64,
    /// Callbacks (`containment_checks` unit): candidates handed to the
    /// comparison closure.
    pub tested: u64,
    pub bit_rejected: u64,
    pub exact_calls: u64,
    pub found: u64,
    /// Reverse: `maintenance_len` (charged bound) and retirements.
    pub maint_bound: u64,
    pub retired: u64,
}
impl Work {
    pub fn add(&mut self, o: &Work) {
        self.groups += o.groups;
        self.groups_eligible += o.groups_eligible;
        self.blocks += o.blocks;
        self.blocks_passed += o.blocks_passed;
        self.tested += o.tested;
        self.bit_rejected += o.bit_rejected;
        self.exact_calls += o.exact_calls;
        self.found += o.found;
        self.maint_bound += o.maint_bound;
        self.retired += o.retired;
    }
}

pub const RELEASED: u32 = u32::MAX;

impl L0 {
    /// Forward lookup. `first_found=false` is `find_controlled(first_id=0)`.
    /// `exclude` skips one ID (the query itself for miss proxies) without
    /// charging it.
    #[inline(never)]
    pub fn find(&self, bucket: usize, q: &Query, first_found: bool, exclude: usize, w: &mut Work) -> Option<usize> {
        let coords = q.coords.as_ref();
        let mut best: Option<usize> = None;
        'groups: for group in &self.buckets[bucket].groups {
            w.groups += 1;
            if !group.signature.may_contain(q.signature) {
                continue;
            }
            w.groups_eligible += 1;
            for block in &group.blocks {
                let ids = block.ids();
                if ids.is_empty() {
                    continue;
                }
                if best.is_some_and(|best| ids[0] >= best) {
                    break;
                }
                w.blocks += 1;
                if !block.may_contain(coords) {
                    continue;
                }
                w.blocks_passed += 1;
                for &id in ids {
                    if best.is_some_and(|best| id >= best) {
                        break;
                    }
                    if id == exclude {
                        continue;
                    }
                    w.tested += 1;
                    if q.word & !self.bits[id] != 0 {
                        w.bit_rejected += 1;
                        continue;
                    }
                    w.exact_calls += 1;
                    let stored = &self.entries[self.slots[id] as usize];
                    if stored.contains(&q.compact) {
                        best = Some(id);
                        if first_found {
                            break 'groups;
                        }
                        break;
                    }
                }
            }
        }
        if best.is_some() {
            w.found += 1;
        }
        best
    }

    /// Bits-only pass over exactly the candidates the min-ID scan would test
    /// when nothing is found (a miss): measures the per-candidate cost of the
    /// filter-word gather alone.
    #[inline(never)]
    pub fn bits_only(&self, bucket: usize, q: &Query, exclude: usize, w: &mut Work) -> u64 {
        let coords = q.coords.as_ref();
        let mut pass = 0u64;
        for group in &self.buckets[bucket].groups {
            if !group.signature.may_contain(q.signature) {
                continue;
            }
            for block in &group.blocks {
                if block.len == 0 || !block.may_contain(coords) {
                    continue;
                }
                for &id in block.ids() {
                    if id == exclude {
                        continue;
                    }
                    w.tested += 1;
                    pass += (q.word & !self.bits[id] == 0) as u64;
                }
            }
        }
        pass
    }

    /// Reverse candidates (`collect_contained` traversal): IDs the new domain
    /// contains, plus the `maintenance_len` charge.
    #[inline(never)]
    pub fn reverse(&self, bucket: usize, q: &Query, exclude: usize, w: &mut Work) -> u64 {
        let coords = q.coords.as_ref();
        let mut contained = 0u64;
        for group in &self.buckets[bucket].groups {
            w.groups += 1;
            if !q.signature.may_contain(group.signature) {
                continue;
            }
            w.groups_eligible += 1;
            w.maint_bound += group.live as u64;
            for block in &group.blocks {
                w.blocks += 1;
                if !block.may_be_contained(coords) {
                    continue;
                }
                w.blocks_passed += 1;
                for &id in block.ids() {
                    if id == exclude {
                        continue;
                    }
                    w.tested += 1;
                    if self.bits[id] & !q.word != 0 {
                        w.bit_rejected += 1;
                        continue;
                    }
                    w.exact_calls += 1;
                    if q.compact.contains(&self.entries[self.slots[id] as usize]) {
                        contained += 1;
                    }
                }
            }
        }
        w.retired += contained;
        contained
    }
}

/// Exact envelope of a set of tight summaries (`Block::insert` fold).
pub fn envelope_of(ids: &[usize], t: &[T]) -> Vec<AxisEnvelope> {
    let mut env: Vec<AxisEnvelope> = Vec::new();
    for &id in ids {
        let s = &t[id];
        if s.empty {
            continue;
        }
        if env.is_empty() {
            env = (0..N)
                .map(|i| {
                    let u = (s.up[i] != U16INF).then_some(s.up[i] as u64);
                    AxisEnvelope {
                        min_lower: s.lo[i] as u64,
                        max_lower: s.lo[i] as u64,
                        min_upper: u,
                        max_upper: u,
                    }
                })
                .collect();
            continue;
        }
        for (i, axis) in env.iter_mut().enumerate() {
            let lower = s.lo[i] as u64;
            let upper = (s.up[i] != U16INF).then_some(s.up[i] as u64);
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
    env
}
