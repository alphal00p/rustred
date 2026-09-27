//! Struct-of-arrays candidate blocks (master plan §3.3 / A5): 32 entries per
//! block, u32 ids, u64 filter words, 36 u8 comparison lanes (field-major),
//! block OR/AND words and u8 block envelopes. Lanes are a monotone u8 image
//! of every tight extremum, normalised so that forward containment C ⊇ Q is
//! `lane(C)[f] <= lane(Q)[f]` for every f (upper-type fields are stored as
//! 255 - clamp(value)); reverse Q ⊇ C is `lane(Q)[f] <= lane(C)[f]`. A
//! monotone map preserves every inequality, so the lane test is a necessary
//! condition (a prefilter) whatever the value range; values that saturate
//! (none in gen 7) only add false positives, which the exact `T::contains`
//! verify (A1) removes.
use crate::ckpt::{N, NEG, POS, Sig, T, U16INF, UINF};
use crate::ckpt::StoredBucket;

pub const F: usize = 36;
pub const B: usize = 32;

#[repr(C, align(64))]
pub struct SBlock {
    pub lanes: [[u8; B]; F],
    pub words: [u64; B],
    pub ids: [u32; B],
    pub env_min: [u8; 64],
    pub env_max: [u8; 64],
    pub or_word: u64,
    pub and_word: u64,
    pub len: u32,
    pub min_id: u32,
}

pub struct SGroup {
    pub sig: Sig,
    pub blocks: Vec<SBlock>,
}
pub struct SBucket {
    pub groups: Vec<SGroup>,
}
pub struct Soa {
    pub buckets: Vec<SBucket>,
    pub id_order: bool,
    pub blocks: usize,
    pub entries: usize,
}

#[inline]
fn lo8(v: u32) -> u8 {
    v.min(255) as u8
}
#[inline]
fn up8(v: u32, inf: bool) -> u8 {
    if inf { 0 } else { 255 - v.min(254) as u8 }
}

/// The 36 normalised lanes of a nonempty summary.
pub fn lanes(t: &T) -> [u8; F] {
    let mut l = [0u8; F];
    for i in 0..N {
        l[i] = lo8(t.lo[i] as u32);
        l[N + i] = up8(t.up[i] as u32, t.up[i] == U16INF);
    }
    l[30] = lo8(t.pl);
    l[31] = up8(t.pu, t.pu == UINF);
    l[32] = lo8(t.nl);
    l[33] = up8(t.nu, t.nu == UINF);
    l[34] = if t.dl == NEG { 0 } else { (t.dl as i64 + 128).clamp(1, 255) as u8 };
    l[35] = if t.du == POS { 0 } else { 255 - (t.du as i64 + 128).clamp(0, 254) as u8 };
    l
}

/// Whether any lane value of `t` saturates (information loss, not error).
pub fn saturates(t: &T) -> bool {
    !t.empty
        && ((0..N).any(|i| t.lo[i] > 254 || (t.up[i] != U16INF && t.up[i] > 254))
            || t.pl > 254
            || (t.pu != UINF && t.pu > 254)
            || t.nl > 254
            || (t.nu != UINF && t.nu > 254)
            || (t.dl != NEG && !(-127..=127).contains(&t.dl))
            || (t.du != POS && !(-128..=126).contains(&t.du)))
}

pub struct SQuery {
    pub l: [u8; 64],
    pub word: u64,
    pub sig: Sig,
}
impl SQuery {
    pub fn of(t: &T) -> Self {
        let mut l = [0u8; 64];
        if !t.empty {
            l[..F].copy_from_slice(&lanes(t));
        }
        SQuery {
            l,
            word: t.word(),
            sig: Sig::of(t),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Order {
    Id,
    PatternLex,
}

pub fn pattern_key(t: &T) -> [u16; 16] {
    let mut k = [0u16; 16];
    k[0] = t.finite_pattern();
    k[1..16].copy_from_slice(&t.lo);
    k
}

fn block_of(ids: &[u32], t: &[T], words: &[u64]) -> SBlock {
    let mut b = SBlock {
        lanes: [[255u8; B]; F],
        words: [u64::MAX; B],
        ids: [u32::MAX; B],
        env_min: [0u8; 64],
        env_max: [0u8; 64],
        or_word: 0,
        and_word: u64::MAX,
        len: ids.len() as u32,
        min_id: ids.iter().copied().min().unwrap_or(u32::MAX),
    };
    let mut env_min = [255u8; F];
    let mut env_max = [0u8; F];
    for (k, &id) in ids.iter().enumerate() {
        let s = &t[id as usize];
        let l = if s.empty { [255u8; F] } else { lanes(s) };
        for f in 0..F {
            b.lanes[f][k] = l[f];
            env_min[f] = env_min[f].min(l[f]);
            env_max[f] = env_max[f].max(l[f]);
        }
        b.words[k] = words[id as usize];
        b.ids[k] = id;
        b.or_word |= words[id as usize];
        b.and_word &= words[id as usize];
    }
    b.env_min[..F].copy_from_slice(&env_min);
    b.env_max[..F].copy_from_slice(&env_max);
    b
}

impl Soa {
    /// Rebuild from the stored buckets/groups, keeping candidate `id` iff
    /// `keep(id)`, ordering each group's candidates by `order`.
    pub fn build(stored: &[StoredBucket], t: &[T], words: &[u64], keep: &(dyn Fn(u32) -> bool + Sync), order: Order) -> Soa {
        let buckets: Vec<SBucket> = std::thread::scope(|s| {
            let chunks: Vec<&[StoredBucket]> = stored.chunks(stored.len().div_ceil(16).max(1)).collect();
            let handles: Vec<_> = chunks
                .into_iter()
                .map(|chunk| {
                    s.spawn(move || {
                        chunk
                            .iter()
                            .map(|bucket| SBucket {
                                groups: bucket
                                    .groups
                                    .iter()
                                    .map(|g| {
                                        let mut ids: Vec<u32> = g
                                            .blocks
                                            .iter()
                                            .flat_map(|b| b.ids.iter().copied())
                                            .filter(|&id| keep(id))
                                            .collect();
                                        match order {
                                            Order::Id => ids.sort_unstable(),
                                            Order::PatternLex => {
                                                ids.sort_by_cached_key(|&id| (pattern_key(&t[id as usize]), id))
                                            }
                                        }
                                        SGroup {
                                            sig: g.sig,
                                            blocks: ids.chunks(B).map(|c| block_of(c, t, words)).collect(),
                                        }
                                    })
                                    .filter(|g| !g.blocks.is_empty())
                                    .collect(),
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
        });
        let blocks = buckets.iter().flat_map(|b| &b.groups).map(|g| g.blocks.len()).sum();
        let entries = buckets
            .iter()
            .flat_map(|b| &b.groups)
            .flat_map(|g| &g.blocks)
            .map(|b| b.len as usize)
            .sum();
        Soa {
            buckets,
            id_order: order == Order::Id,
            blocks,
            entries,
        }
    }
}

#[derive(Default, Clone, Copy, Debug)]
pub struct SWork {
    pub groups: u64,
    pub groups_eligible: u64,
    pub blocks: u64,
    pub blocks_passed: u64,
    /// Candidates whose lanes were evaluated (full block lengths).
    pub tested: u64,
    pub word_pass: u64,
    pub lane_pass: u64,
    pub exact_calls: u64,
    pub found: u64,
    pub retired: u64,
}
impl SWork {
    pub fn add(&mut self, o: &SWork) {
        self.groups += o.groups;
        self.groups_eligible += o.groups_eligible;
        self.blocks += o.blocks;
        self.blocks_passed += o.blocks_passed;
        self.tested += o.tested;
        self.word_pass += o.word_pass;
        self.lane_pass += o.lane_pass;
        self.exact_calls += o.exact_calls;
        self.found += o.found;
        self.retired += o.retired;
    }
}

#[inline]
fn len_mask(len: u32) -> u32 {
    if len >= 32 { u32::MAX } else { (1u32 << len) - 1 }
}

#[inline]
fn block_fwd(b: &SBlock, q: &SQuery) -> bool {
    if q.word & !b.or_word != 0 {
        return false;
    }
    let mut ok = true;
    for f in 0..64 {
        ok &= b.env_min[f] <= q.l[f];
    }
    ok
}
#[inline]
fn block_rev(b: &SBlock, q: &SQuery) -> bool {
    if b.and_word & !q.word != 0 {
        return false;
    }
    let mut ok = true;
    for f in 0..64 {
        ok &= q.l[f] <= b.env_max[f];
    }
    ok
}

/// Portable reference kernels (used for the differential self-check).
pub fn fwd_mask_scalar(b: &SBlock, q: &SQuery) -> (u32, u32) {
    let mut wm = 0u32;
    for k in 0..b.len as usize {
        wm |= ((q.word & !b.words[k] == 0) as u32) << k;
    }
    let mut m = wm;
    for f in 0..F {
        let mut fm = 0u32;
        for k in 0..B {
            fm |= ((b.lanes[f][k] <= q.l[f]) as u32) << k;
        }
        m &= fm;
    }
    (wm, m)
}
pub fn rev_mask_scalar(b: &SBlock, q: &SQuery) -> (u32, u32) {
    let mut wm = 0u32;
    for k in 0..b.len as usize {
        wm |= ((b.words[k] & !q.word == 0) as u32) << k;
    }
    let mut m = wm;
    for f in 0..F {
        let mut fm = 0u32;
        for k in 0..B {
            fm |= ((q.l[f] <= b.lanes[f][k]) as u32) << k;
        }
        m &= fm;
    }
    (wm, m)
}

#[cfg(target_arch = "x86_64")]
mod simd {
    use super::{B, F, SBlock, SQuery, len_mask};
    use std::arch::x86_64::*;

    #[inline]
    #[target_feature(enable = "avx512f,avx512bw,avx512vl")]
    fn word_fwd(b: &SBlock, q: u64) -> u32 {
        let qw = _mm512_set1_epi64(q as i64);
        let mut m = 0u32;
        for k in 0..B / 8 {
            // SAFETY: `words` is 64-byte aligned (SBlock is align(64), words at 1152).
            let cw = unsafe { _mm512_load_si512(b.words.as_ptr().add(8 * k) as *const _) };
            let x = _mm512_andnot_si512(cw, qw);
            m |= (_mm512_testn_epi64_mask(x, x) as u32) << (8 * k);
        }
        m
    }
    #[inline]
    #[target_feature(enable = "avx512f,avx512bw,avx512vl")]
    fn word_rev(b: &SBlock, q: u64) -> u32 {
        let qw = _mm512_set1_epi64(q as i64);
        let mut m = 0u32;
        for k in 0..B / 8 {
            // SAFETY: as above.
            let cw = unsafe { _mm512_load_si512(b.words.as_ptr().add(8 * k) as *const _) };
            let x = _mm512_andnot_si512(qw, cw);
            m |= (_mm512_testn_epi64_mask(x, x) as u32) << (8 * k);
        }
        m
    }

    /// (word mask, word & lane mask) for forward containment C ⊇ Q.
    #[inline]
    #[target_feature(enable = "avx512f,avx512bw,avx512vl")]
    pub fn fwd_mask(b: &SBlock, q: &SQuery) -> (u32, u32) {
        let wm = word_fwd(b, q.word) & len_mask(b.len);
        let mut m = wm;
        if m == 0 {
            return (0, 0);
        }
        let mut f = 0;
        while f < F {
            for g in f..(f + 6).min(F) {
                // SAFETY: lane rows are 32-byte aligned inside the 64-aligned block.
                let c = unsafe { _mm256_load_si256(b.lanes[g].as_ptr() as *const __m256i) };
                let qv = _mm256_set1_epi8(q.l[g] as i8);
                m &= _mm256_cmple_epu8_mask(c, qv);
            }
            if m == 0 {
                return (wm, 0);
            }
            f += 6;
        }
        (wm, m)
    }

    /// (word mask, word & lane mask) for reverse containment Q ⊇ C.
    #[inline]
    #[target_feature(enable = "avx512f,avx512bw,avx512vl")]
    pub fn rev_mask(b: &SBlock, q: &SQuery) -> (u32, u32) {
        let wm = word_rev(b, q.word) & len_mask(b.len);
        let mut m = wm;
        if m == 0 {
            return (0, 0);
        }
        let mut f = 0;
        while f < F {
            for g in f..(f + 6).min(F) {
                // SAFETY: as above.
                let c = unsafe { _mm256_load_si256(b.lanes[g].as_ptr() as *const __m256i) };
                let qv = _mm256_set1_epi8(q.l[g] as i8);
                m &= _mm256_cmpge_epu8_mask(c, qv);
            }
            if m == 0 {
                return (wm, 0);
            }
            f += 6;
        }
        (wm, m)
    }

    #[inline]
    #[target_feature(enable = "avx512f,avx512bw,avx512vl")]
    pub fn word_only(b: &SBlock, q: u64) -> u32 {
        word_fwd(b, q) & len_mask(b.len)
    }
}

pub fn fwd_mask(b: &SBlock, q: &SQuery) -> (u32, u32) {
    // SAFETY: the tool is built for znver4 (AVX-512BW/VL); main() checks it.
    unsafe { simd::fwd_mask(b, q) }
}
pub fn rev_mask(b: &SBlock, q: &SQuery) -> (u32, u32) {
    unsafe { simd::rev_mask(b, q) }
}

impl Soa {
    /// Forward lookup: first-found stops at the first verified container in
    /// layout order; min-ID continues (with the ID-ordered early break when
    /// the layout is ID-ordered).
    #[inline(never)]
    pub fn find(&self, bucket: usize, q: &SQuery, qt: &T, t: &[T], first_found: bool, exclude: u32, w: &mut SWork) -> Option<u32> {
        let mut best: Option<u32> = None;
        'g: for g in &self.buckets[bucket].groups {
            w.groups += 1;
            if !g.sig.may_contain(&q.sig) {
                continue;
            }
            w.groups_eligible += 1;
            for b in &g.blocks {
                if self.id_order && best.is_some_and(|best| b.min_id >= best) {
                    break;
                }
                w.blocks += 1;
                if qt.empty {
                    // An empty query is contained by every candidate.
                    w.tested += 1;
                    w.exact_calls += 1;
                    let id = b.ids[0];
                    if id != exclude {
                        best = Some(best.map_or(id, |x| x.min(id)));
                        if first_found || self.id_order {
                            break 'g;
                        }
                    }
                    continue;
                }
                if !block_fwd(b, q) {
                    continue;
                }
                w.blocks_passed += 1;
                w.tested += b.len as u64;
                let (wm, mut m) = fwd_mask(b, q);
                w.word_pass += wm.count_ones() as u64;
                w.lane_pass += m.count_ones() as u64;
                while m != 0 {
                    let k = m.trailing_zeros() as usize;
                    m &= m - 1;
                    let id = b.ids[k];
                    if id == exclude || best.is_some_and(|best| id >= best) {
                        continue;
                    }
                    w.exact_calls += 1;
                    if t[id as usize].contains(qt) {
                        best = Some(id);
                        if first_found {
                            break 'g;
                        }
                        if self.id_order {
                            break;
                        }
                    }
                }
            }
        }
        if best.is_some() {
            w.found += 1;
        }
        best
    }

    /// Word-only pass over the blocks a miss scan evaluates.
    #[inline(never)]
    pub fn word_only(&self, bucket: usize, q: &SQuery, w: &mut SWork) -> u64 {
        let mut pass = 0u64;
        for g in &self.buckets[bucket].groups {
            if !g.sig.may_contain(&q.sig) {
                continue;
            }
            for b in &g.blocks {
                if !block_fwd(b, q) {
                    continue;
                }
                w.tested += b.len as u64;
                // SAFETY: as above.
                pass += unsafe { simd::word_only(b, q.word) }.count_ones() as u64;
            }
        }
        pass
    }

    /// Reverse set of a new domain Q (candidates Q contains).
    #[inline(never)]
    pub fn reverse(&self, bucket: usize, q: &SQuery, qt: &T, t: &[T], exclude: u32, w: &mut SWork) -> u64 {
        let mut contained = 0u64;
        for g in &self.buckets[bucket].groups {
            w.groups += 1;
            if !q.sig.may_contain(&g.sig) {
                continue;
            }
            w.groups_eligible += 1;
            for b in &g.blocks {
                w.blocks += 1;
                if !block_rev(b, q) {
                    continue;
                }
                w.blocks_passed += 1;
                w.tested += b.len as u64;
                let (wm, mut m) = rev_mask(b, q);
                w.word_pass += wm.count_ones() as u64;
                w.lane_pass += m.count_ones() as u64;
                while m != 0 {
                    let k = m.trailing_zeros() as usize;
                    m &= m - 1;
                    let id = b.ids[k];
                    if id == exclude {
                        continue;
                    }
                    w.exact_calls += 1;
                    if qt.contains(&t[id as usize]) {
                        contained += 1;
                    }
                }
            }
        }
        w.retired += contained;
        contained
    }

    /// Differential self-check of the SIMD kernels against the scalar ones
    /// on every block of `bucket` for query `q`; returns mismatching blocks.
    pub fn check_kernels(&self, bucket: usize, q: &SQuery) -> usize {
        let mut bad = 0;
        for g in &self.buckets[bucket].groups {
            for b in &g.blocks {
                let a = fwd_mask(b, q);
                let s = fwd_mask_scalar(b, q);
                // The SIMD kernel may stop early with m == 0; compare final masks.
                if a.1 != s.1 || (a.0 != 0 && a.0 != s.0) {
                    bad += 1;
                }
                let a = rev_mask(b, q);
                let s = rev_mask_scalar(b, q);
                if a.1 != s.1 || (a.0 != 0 && a.0 != s.0) {
                    bad += 1;
                }
            }
        }
        bad
    }
}

/// Exact lane-test agreement: for nonsaturating summaries, the lane test
/// (plus the word test) must equal `T::contains` within a bucket.
pub fn lane_contains(c: &T, q: &T) -> bool {
    if q.empty {
        return true;
    }
    if c.empty {
        return false;
    }
    let (lc, lq) = (lanes(c), lanes(q));
    (q.word() & !c.word() == 0) && (0..F).all(|f| lc[f] <= lq[f])
}
