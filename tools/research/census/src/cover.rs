//! Union coverage of Apply domains by other Apply domains of the same
//! (phase, owner) bucket: the G2' bound (residual inspection against merged
//! Native anchors, plan §3.10), residual piece counts under several cut
//! vocabularies, point-space saturation, and the hot-owner pilot overlap.
//!
//! Coverage is decided point by point on the query domain Q (every lattice
//! point of Q, or a uniform random sample of them when |Q| exceeds a cap),
//! against the anchors whose box and aggregate ranges intersect Q. Anchors
//! are never enumerated, so arbitrarily large anchors are fine.
use crate::ckpt::{L_RESERVED, L_STARTED, L_UNRESERVED};
use crate::compose::cost_law;
use crate::geom::{self, DHI_NONE, DLO_NONE, Dom, INF8, Levels, MAXN, NONE16};
use crate::recs::{self, Rec};
use crate::util::{self, Ckpt, DomSum, Opts};
use rayon::prelude::*;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::Path;

// ---------------------------------------------------------------- rng
#[derive(Clone)]
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    pub fn next(&mut self) -> u64 {
        // splitmix64
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

// ------------------------------------------------------- point sampler
/// Uniform sampler of the lattice points of one finite domain.
pub struct Sampler {
    t: i32,
    act: Vec<usize>,
    ina: Vec<usize>,
    lo: [u8; MAXN],
    hi: [u8; MAXN],
    /// suffix counts per group: suf[k][s] = ways coordinates k.. sum to s.
    suf_a: Vec<Vec<f64>>,
    suf_r: Vec<Vec<f64>>,
    /// cumulative level weights over (sa, sr).
    cum: Vec<(f64, usize, usize)>,
}

fn suffix(coords: &[usize], lo: &[u8; MAXN], hi: &[u8; MAXN], cap: usize) -> Vec<Vec<f64>> {
    let m = coords.len();
    let mut suf = vec![vec![0f64; cap + 1]; m + 1];
    suf[m][0] = 1.0;
    for k in (0..m).rev() {
        let i = coords[k];
        for s in 0..=cap {
            let mut acc = 0.0;
            for v in lo[i]..=hi[i] {
                let v = v as usize;
                if v > s {
                    break;
                }
                acc += suf[k + 1][s - v];
            }
            suf[k][s] = acc;
        }
    }
    suf
}

impl Sampler {
    pub fn new(d: &Dom, n: usize, l: &Levels) -> Option<Self> {
        let hi = geom::eff_hi(d, n)?;
        let act: Vec<usize> = (0..n).filter(|&i| d.active(i)).collect();
        let ina: Vec<usize> = (0..n).filter(|&i| !d.active(i)).collect();
        let ca = l.da.len().saturating_sub(1);
        let cr = l.dr.len().saturating_sub(1);
        let suf_a = suffix(&act, &d.lo, &hi, ca);
        let suf_r = suffix(&ina, &d.lo, &hi, cr);
        let mut cum = Vec::new();
        let mut acc = 0.0;
        for sa in 0..l.da.len() {
            for sr in 0..l.dr.len() {
                let w = l.at(sa, sr) as f64;
                if w > 0.0 {
                    acc += w;
                    cum.push((acc, sa, sr));
                }
            }
        }
        if cum.is_empty() {
            return None;
        }
        Some(Sampler { t: l.t, act, ina, lo: d.lo, hi, suf_a, suf_r, cum })
    }
    fn group(&self, coords: &[usize], suf: &[Vec<f64>], mut s: usize, rng: &mut Rng, x: &mut [u8; MAXN]) {
        for (k, &i) in coords.iter().enumerate() {
            let tot = suf[k][s];
            let mut u = rng.unit() * tot;
            let mut chosen = self.lo[i];
            for v in self.lo[i]..=self.hi[i] {
                if v as usize > s {
                    break;
                }
                let w = suf[k + 1][s - v as usize];
                chosen = v;
                if u < w {
                    break;
                }
                u -= w;
            }
            x[i] = chosen;
            s -= chosen as usize;
        }
    }
    /// One uniform point: (x, sa, sr).
    pub fn sample(&self, rng: &mut Rng) -> ([u8; MAXN], i32, i32) {
        let tot = self.cum.last().unwrap().0;
        let u = rng.unit() * tot;
        let k = self.cum.partition_point(|c| c.0 <= u).min(self.cum.len() - 1);
        let (_, sa, sr) = self.cum[k];
        let mut x = [0u8; MAXN];
        self.group(&self.act, &self.suf_a, sa, rng, &mut x);
        self.group(&self.ina, &self.suf_r, sr, rng, &mut x);
        let _ = self.t;
        (x, sa as i32, sr as i32)
    }
}

// ------------------------------------------------------------ anchors
/// Compact anchor: effective box and aggregate bounds.
#[derive(Clone, Copy)]
pub struct Anchor {
    pub id: u32,
    /// Commit order of the anchor's record (u32::MAX for non-natives).
    pub seq: u32,
    pub lo: [u8; MAXN],
    /// Effective upper bound per coordinate (INF8 = unbounded).
    pub hi: [u8; MAXN],
    pub amax: i32,
    pub rmax: i32,
    pub dmin: i32,
    pub dmax: i32,
}
impl Anchor {
    pub fn new(id: u32, seq: u32, d: &Dom, s: &DomSum, n: usize) -> Self {
        let mut hi = d.hi;
        if let Some(h) = geom::eff_hi(d, n) {
            hi = h;
        }
        let _ = s;
        Anchor {
            id,
            seq,
            lo: d.lo,
            hi,
            amax: if d.amax == NONE16 { i32::MAX } else { d.amax as i32 },
            rmax: if d.rank == INF8 { i32::MAX } else { d.rank as i32 },
            dmin: if d.dmin == DLO_NONE { i32::MIN } else { d.dmin as i32 },
            dmax: if d.dmax == DHI_NONE { i32::MAX } else { d.dmax as i32 },
        }
    }
    #[inline]
    pub fn hit(&self, n: usize, x: &[u8; MAXN], a: i32, r: i32) -> bool {
        if a > self.amax || r > self.rmax {
            return false;
        }
        let d = a - r;
        if d < self.dmin || d > self.dmax {
            return false;
        }
        for i in 0..n {
            if x[i] < self.lo[i] || x[i] > self.hi[i] {
                return false;
            }
        }
        true
    }
    /// Box and aggregate ranges intersect the query's (necessary condition).
    #[inline]
    fn meets(&self, n: usize, q: &Anchor, qext: &[i32; 8]) -> bool {
        for i in 0..n {
            if self.lo[i] > q.hi[i] || self.hi[i] < q.lo[i] {
                return false;
            }
        }
        self.amax >= qext[0] && self.rmax >= qext[2] && self.dmax >= qext[6] && self.dmin <= qext[7]
    }
    /// Contains the whole query (box and aggregate ranges).
    #[inline]
    fn contains(&self, n: usize, q: &Anchor, qext: &[i32; 8]) -> bool {
        for i in 0..n {
            if self.lo[i] > q.lo[i] || self.hi[i] < q.hi[i] {
                return false;
            }
        }
        self.amax >= qext[1] && self.rmax >= qext[3] && self.dmin <= qext[6] && self.dmax >= qext[7]
    }
}

// --------------------------------------------------------- evaluation
/// One residual piece family: number of pieces and their point counts.
#[derive(Clone, Debug, Default)]
pub struct Pieces {
    pub pieces: usize,
    pub points: f64,
    pub sizes: Vec<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct Eval {
    pub points: f64,
    /// True if every point was tested (else a uniform sample).
    pub exact: bool,
    pub tested: usize,
    pub candidates: usize,
    pub single_container: bool,
    /// Estimated uncovered points (pointwise residual).
    pub uncovered: f64,
    pub d_only: Pieces,
    pub ar: Pieces,
    pub hull: Pieces,
    pub hull_c2: Pieces,
    pub hull_d: Pieces,
    /// Distinct anchors that were the first hit of some tested point (a
    /// greedy cover size, not a minimum; 0 for a single container).
    pub anchors_used: usize,
}

fn count_region(q: &Dom, n: usize, lo: &[u8; MAXN], hi: &[u8; MAXN], a: (i32, i32), r: (i32, i32), d: (i32, i32)) -> f64 {
    let mut h = *q;
    h.lo = *lo;
    h.hi = *hi;
    h.amax = if q.amax == NONE16 { a.1 as u16 } else { q.amax.min(a.1 as u16) };
    h.rank = if q.rank == INF8 { r.1 as u8 } else { q.rank.min(r.1 as u8) };
    h.dmin = if q.dmin == DLO_NONE { d.0 as i16 } else { q.dmin.max(d.0 as i16) };
    h.dmax = if q.dmax == DHI_NONE { d.1 as i16 } else { q.dmax.min(d.1 as i16) };
    let Some(l) = geom::levels(&h, n) else {
        return f64::NAN;
    };
    let mut s = 0f64;
    for sa in 0..l.da.len() {
        let av = sa as i32 + l.t;
        if av < a.0 {
            continue;
        }
        for sr in 0..l.dr.len() {
            if (sr as i32) < r.0 {
                continue;
            }
            s += l.at(sa, sr) as f64;
        }
    }
    s
}

#[derive(Clone, Copy)]
struct BBox {
    lo: [u8; MAXN],
    hi: [u8; MAXN],
    a: (i32, i32),
    r: (i32, i32),
    d: (i32, i32),
    any: bool,
}
impl BBox {
    fn empty() -> Self {
        BBox { lo: [u8::MAX; MAXN], hi: [0; MAXN], a: (i32::MAX, i32::MIN), r: (i32::MAX, i32::MIN), d: (i32::MAX, i32::MIN), any: false }
    }
    fn add(&mut self, n: usize, x: &[u8; MAXN], a: i32, r: i32) {
        for i in 0..n {
            self.lo[i] = self.lo[i].min(x[i]);
            self.hi[i] = self.hi[i].max(x[i]);
        }
        self.a = (self.a.0.min(a), self.a.1.max(a));
        self.r = (self.r.0.min(r), self.r.1.max(r));
        self.d = (self.d.0.min(a - r), self.d.1.max(a - r));
        self.any = true;
    }
    fn merge(&mut self, n: usize, o: &BBox) {
        if !o.any {
            return;
        }
        for i in 0..n {
            self.lo[i] = self.lo[i].min(o.lo[i]);
            self.hi[i] = self.hi[i].max(o.hi[i]);
        }
        self.a = (self.a.0.min(o.a.0), self.a.1.max(o.a.1));
        self.r = (self.r.0.min(o.r.0), self.r.1.max(o.r.1));
        self.d = (self.d.0.min(o.d.0), self.d.1.max(o.d.1));
        self.any = true;
    }
}

/// Evaluate the coverage of `q` by `anchors` (already filtered by the
/// caller's admission rule). `cap`: maximum points tested exactly; above it,
/// `samples` uniform points are tested.
pub fn evaluate(q: &Dom, n: usize, pool: &[Anchor], admit: &dyn Fn(&Anchor) -> bool, cap: f64, samples: usize, rng: &mut Rng) -> Option<Eval> {
    let l = geom::levels(q, n)?;
    let total = l.total() as f64;
    let ext = l.extrema()?;
    let qa = Anchor::new(u32::MAX, u32::MAX, q, &DomSum::default(), n);
    let mut cands: Vec<&Anchor> = pool.iter().filter(|a| a.meets(n, &qa, &ext) && admit(a)).collect();
    let mut ev = Eval { points: total, candidates: cands.len(), ..Default::default() };
    if cands.iter().any(|a| a.contains(n, &qa, &ext)) {
        ev.single_container = true;
        ev.exact = true;
        return Some(ev);
    }
    let (la, lr) = (l.da.len(), l.dr.len());
    let mut tot = vec![0f64; la * lr];
    let mut unc = vec![0f64; la * lr];
    let dmin_q = ext[6];
    let nd = (ext[7] - ext[6] + 1) as usize;
    let mut dbox = vec![BBox::empty(); nd];
    let t = l.t;
    let mut grid = Grid::new(&cands, &qa, n);
    let mut used: std::collections::HashSet<u32> = Default::default();
    let mut test = |x: &[u8; MAXN], sa: i32, sr: i32, grid: &mut Grid| {
        let a = sa + t;
        let li = sa as usize * lr + sr as usize;
        tot[li] += 1.0;
        let mut hit = false;
        let cands = grid.cell(x);
        for k in 0..cands.len() {
            if cands[k].hit(n, x, a, sr) {
                hit = true;
                used.insert(cands[k].id);
                if k > 0 {
                    cands.swap(0, k);
                }
                break;
            }
        }
        if !hit {
            unc[li] += 1.0;
            dbox[(a - sr - dmin_q) as usize].add(n, x, a, sr);
        }
    };
    let scale;
    if total <= cap {
        // A finite domain with an effective coordinate bound above 62 cannot
        // be enumerated: report it as unevaluated (never as covered).
        if !geom::enumerate(q, n, &mut |_, x, sa, sr| test(x, sa, sr, &mut grid)) {
            return None;
        }
        ev.exact = true;
        ev.tested = total as usize;
        scale = 1.0;
    } else {
        let s = Sampler::new(q, n, &l)?;
        for _ in 0..samples {
            let (x, sa, sr) = s.sample(rng);
            test(&x, sa, sr, &mut grid);
        }
        ev.tested = samples;
        scale = total / samples as f64;
    }
    ev.anchors_used = used.len();
    // Exact level totals (independent of sampling).
    let mut ltot = vec![0f64; la * lr];
    for sa in 0..la {
        for sr in 0..lr {
            ltot[sa * lr + sr] = l.at(sa, sr) as f64;
        }
    }
    let unc_total: f64 = unc.iter().sum();
    ev.uncovered = unc_total * scale;
    if unc_total == 0.0 {
        return Some(ev);
    }
    let _ = tot;
    // D-only: D values with an uncovered point; runs broken by D values
    // whose points are all covered (D values absent from Q do not break).
    let mut dtot = vec![0f64; nd];
    let mut dunc = vec![0f64; nd];
    for sa in 0..la {
        for sr in 0..lr {
            let d = sa as i32 + t - sr as i32;
            if d < ext[6] || d > ext[7] {
                continue;
            }
            dtot[(d - dmin_q) as usize] += ltot[sa * lr + sr];
            dunc[(d - dmin_q) as usize] += unc[sa * lr + sr];
        }
    }
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut cur: Option<(usize, usize)> = None;
    for k in 0..nd {
        if dunc[k] > 0.0 {
            cur = Some(match cur {
                None => (k, k),
                Some((s, _)) => (s, k),
            });
        } else if dtot[k] > 0.0 {
            if let Some(c) = cur.take() {
                runs.push(c);
            }
        }
    }
    if let Some(c) = cur {
        runs.push(c);
    }
    for &(s, e) in &runs {
        let sz: f64 = dtot[s..=e].iter().sum();
        ev.d_only.sizes.push(sz);
        ev.d_only.points += sz;
        // Box hull of the uncovered points within this D run.
        let mut b = BBox::empty();
        for k in s..=e {
            b.merge(n, &dbox[k]);
        }
        let c = count_region(q, n, &b.lo, &b.hi, (i32::MIN, b.a.1), (i32::MIN, b.r.1), (s as i32 + dmin_q, e as i32 + dmin_q));
        ev.hull_d.sizes.push(c);
        ev.hull_d.points += c;
    }
    ev.d_only.pieces = runs.len();
    ev.hull_d.pieces = runs.len();
    // A/R rectangles: per R row, runs over A of levels with uncovered points
    // (broken by fully covered nonempty levels, trimmed to uncovered ends);
    // identical runs in consecutive rows merge into one rectangle.
    let mut prev: Vec<(usize, usize)> = Vec::new();
    let mut rects: Vec<(usize, usize, usize, usize)> = Vec::new(); // a1, a2, r1, r2
    let mut open: Vec<usize> = Vec::new(); // indices into rects for prev runs
    for sr in 0..lr {
        let mut row: Vec<(usize, usize)> = Vec::new();
        let mut cur: Option<(usize, usize)> = None;
        for sa in 0..la {
            let li = sa * lr + sr;
            if unc[li] > 0.0 {
                cur = Some(match cur {
                    None => (sa, sa),
                    Some((s, _)) => (s, sa),
                });
            } else if ltot[li] > 0.0 {
                if let Some(c) = cur.take() {
                    row.push(c);
                }
            }
        }
        if let Some(c) = cur {
            row.push(c);
        }
        let mut nopen = Vec::new();
        for run in &row {
            if let Some(p) = prev.iter().position(|x| x == run) {
                let ri = open[p];
                rects[ri].3 = sr;
                nopen.push(ri);
            } else {
                rects.push((run.0, run.1, sr, sr));
                nopen.push(rects.len() - 1);
            }
        }
        prev = row;
        open = nopen;
    }
    for &(a1, a2, r1, r2) in &rects {
        let mut sz = 0f64;
        for sa in a1..=a2 {
            for sr in r1..=r2 {
                sz += ltot[sa * lr + sr];
            }
        }
        ev.ar.sizes.push(sz);
        ev.ar.points += sz;
    }
    ev.ar.pieces = rects.len();
    // Box hulls of all uncovered points (one piece).
    let mut b = BBox::empty();
    for x in &dbox {
        b.merge(n, x);
    }
    let c = count_region(q, n, &b.lo, &b.hi, (i32::MIN, b.a.1), (i32::MIN, b.r.1), b.d);
    ev.hull = Pieces { pieces: 1, points: c, sizes: vec![c] };
    let c2 = count_region(q, n, &b.lo, &b.hi, b.a, b.r, b.d);
    ev.hull_c2 = Pieces { pieces: 1, points: c2, sizes: vec![c2] };
    Some(ev)
}

// ------------------------------------------------------------ samples
/// Sample `k` indices with probability proportional to `w` (with
/// replacement), returned deduplicated with multiplicities.
pub fn pps(w: &[f64], k: usize, rng: &mut Rng) -> Vec<(usize, usize)> {
    let mut cum = Vec::with_capacity(w.len());
    let mut acc = 0.0;
    for &x in w {
        acc += x.max(0.0);
        cum.push(acc);
    }
    let mut m: BTreeMap<usize, usize> = BTreeMap::new();
    if acc <= 0.0 {
        return vec![];
    }
    for _ in 0..k {
        let u = rng.unit() * acc;
        let i = cum.partition_point(|&c| c <= u).min(w.len() - 1);
        *m.entry(i).or_default() += 1;
    }
    m.into_iter().collect()
}
pub fn uniform(len: usize, k: usize, rng: &mut Rng) -> Vec<(usize, usize)> {
    let mut m: BTreeMap<usize, usize> = BTreeMap::new();
    if len == 0 {
        return vec![];
    }
    for _ in 0..k {
        *m.entry((rng.next() % len as u64) as usize).or_default() += 1;
    }
    m.into_iter().collect()
}

struct Buckets {
    /// Apply natives per owner, sorted by seq.
    natives: HashMap<u16, Vec<Anchor>>,
    /// All Apply domains per owner, sorted by id (seq = native seq or MAX).
    all: HashMap<u16, Vec<Anchor>>,
}

fn buckets(ck: &Ckpt, sums: &[DomSum], with_all: bool) -> Buckets {
    let n = ck.n;
    let mut natives: HashMap<u16, Vec<Anchor>> = HashMap::new();
    for r in &ck.recs {
        if !recs::is_native(r.kind) || r.phase != 0 {
            continue;
        }
        let d = &ck.doms[r.id as usize];
        let s = &sums[r.id as usize];
        if s.empty {
            continue;
        }
        natives.entry(d.owner).or_default().push(Anchor::new(r.id, r.seq, d, s, n));
    }
    let mut all: HashMap<u16, Vec<Anchor>> = HashMap::new();
    if with_all {
        for (id, d) in ck.doms.iter().enumerate() {
            if d.phase != 0 || sums[id].empty {
                continue;
            }
            let seq = ck.native_of[id];
            let seq = if seq == u32::MAX { u32::MAX } else { ck.recs[seq as usize].seq };
            all.entry(d.owner).or_default().push(Anchor::new(id as u32, seq, d, &sums[id], n));
        }
    }
    Buckets { natives, all }
}

/// Aggregate over evaluated queries for one (sample, anchor set).
#[derive(Default)]
struct Summary {
    w: f64,
    n: usize,
    evaluated: usize,
    skipped_w: f64,
    single_w: f64,
    full_w: f64,
    /// per vocabulary: [gate weight, residual-points-weighted, projected work]
    voc: BTreeMap<&'static str, [f64; 5]>,
    unc_frac_w: f64,
}

pub fn run(dir: &Path, opts: &Opts) {
    let ck = util::load(dir, opts, true);
    let n = ck.n;
    let sums = util::dom_sums(&ck.doms, n);
    let law = cost_law(&ck, &sums, 0);
    let seed = opts.num("seed", 20260927u64);
    let k = opts.num("k", 4000usize);
    let cap = opts.num("cap", 2.0e6f64);
    let samples = opts.num("samples", 20000usize);
    let margin_base = opts.num("margin-base", 5000f64);
    let margin_rate = opts.num("margin-rate", 3000f64);
    let with_all = opts.num("with-all", 1u8) == 1;
    // Optional heartbeat series "elapsed_seconds committed_records" of the run
    // that wrote the checkpoint: the dispatch threshold then becomes the
    // number of records committed by (commit time - seconds - wait).
    let series: Option<Vec<(f64, u64)>> = opts.get("series").map(|p| {
        std::fs::read_to_string(p)
            .unwrap()
            .lines()
            .filter_map(|l| {
                let mut it = l.split_whitespace();
                Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?))
            })
            .collect()
    });
    let wait = opts.num("wait", 30f64);
    let dispatch_thr = |seq: u32, secs: f64| -> u32 {
        match &series {
            None => (seq as f64 - margin_base - margin_rate * secs).max(0.0) as u32,
            Some(s) => {
                let i = s.partition_point(|&(_, c)| c <= seq as u64);
                let tc = if i < s.len() { s[i].0 } else { s[s.len() - 1].0 };
                let td = tc - secs - wait;
                let j = s.partition_point(|&(t, _)| t <= td);
                if j == 0 { 0 } else { (s[j - 1].1 as u32).min(seq) }
            }
        }
    };
    let out_rows = opts.get("rows").map(|p| std::sync::Mutex::new(std::fs::File::create(p).unwrap()));
    let t0 = std::time::Instant::now();
    let b = buckets(&ck, &sums, with_all);
    eprintln!("cover: buckets built ({:.1} s)", t0.elapsed().as_secs_f64());
    let mut rng = Rng::new(seed);
    // Candidate query populations.
    let apply_natives: Vec<&Rec> = ck.recs.iter().filter(|r| recs::is_native(r.kind) && r.phase == 0).collect();
    let ledger = ck.ledger.as_ref();
    let pending: Vec<usize> = match ledger {
        Some(l) => (0..ck.doms.len())
            .filter(|&id| ck.doms[id].phase == 0 && matches!(l.entries[id].state, L_UNRESERVED | L_RESERVED | L_STARTED) && !sums[id].empty && !sums[id].infinite)
            .collect(),
        None => vec![],
    };
    let pend_pred: Vec<f64> = pending.iter().map(|&id| law.predict_binned(ck.doms[id].owner, sums[id].points)).collect();
    let hist_secs: Vec<f64> = apply_natives.iter().map(|r| r.seconds as f64).collect();
    // (sample name, query kind, list of (index, multiplicity), weight basis)
    let mut plans: Vec<(&str, Vec<(usize, usize)>)> = Vec::new();
    plans.push(("hist_pps_seconds", pps(&hist_secs, k, &mut rng)));
    plans.push(("hist_uniform", uniform(apply_natives.len(), k, &mut rng)));
    if !pending.is_empty() {
        plans.push(("pending_pps_predicted", pps(&pend_pred, k, &mut rng)));
        plans.push(("pending_uniform", uniform(pending.len(), k, &mut rng)));
    }
    let mut report = serde_json::Map::new();
    report.insert("dir".into(), json!(dir));
    report.insert("generation".into(), json!(ck.m.generation));
    report.insert("apply_natives".into(), json!(apply_natives.len()));
    report.insert("apply_native_seconds".into(), json!(hist_secs.iter().sum::<f64>()));
    report.insert("pending_apply_finite".into(), json!(pending.len()));
    report.insert("pending_apply_predicted_seconds[E]".into(), json!(pend_pred.iter().sum::<f64>()));
    report.insert("params".into(), json!({"k": k, "cap": cap, "samples": samples, "margin_base": margin_base, "margin_rate": margin_rate, "seed": seed, "series": opts.get("series"), "wait": wait}));
    for (name, plan) in plans {
        let hist = name.starts_with("hist");
        // Anchor sets per query kind.
        let sets: Vec<&str> = if hist {
            if with_all { vec!["natives_before_dispatch", "natives_before_commit", "all_earlier_ids"] } else { vec!["natives_before_dispatch", "natives_before_commit"] }
        } else if with_all {
            vec!["all_natives", "all_other_domains"]
        } else {
            vec!["all_natives"]
        };
        let results: Vec<(usize, usize, u32, u16, f64, f64, Vec<Option<Eval>>)> = plan
            .par_iter()
            .map(|&(ix, mult)| {
                let mut rng = Rng::new(seed ^ (ix as u64).wrapping_mul(0x2545F4914F6CDD1D));
                let (id, seq, secs) = if hist {
                    let r = apply_natives[ix];
                    (r.id as usize, r.seq, r.seconds as f64)
                } else {
                    (pending[ix], u32::MAX, 0.0)
                };
                let q = &ck.doms[id];
                let pred = law.predict_binned(q.owner, sums[id].points);
                let mut evs = Vec::new();
                for set in &sets {
                    let pool: &[Anchor] = match *set {
                        "all_earlier_ids" | "all_other_domains" => b.all.get(&q.owner).map(|v| v.as_slice()).unwrap_or(&[]),
                        _ => b.natives.get(&q.owner).map(|v| v.as_slice()).unwrap_or(&[]),
                    };
                    let thr = if *set == "natives_before_dispatch" {
                        dispatch_thr(seq, secs)
                    } else {
                        seq
                    };
                    let admit = |a: &Anchor| match *set {
                            "natives_before_dispatch" | "natives_before_commit" => a.seq < thr,
                            "all_earlier_ids" => (a.id as usize) < id,
                            "all_natives" => true,
                            _ => a.id as usize != id,
                        };
                    evs.push(evaluate(q, n, pool, &admit, cap, samples, &mut rng));
                }
                (ix, mult, id as u32, q.owner, secs, pred, evs)
            })
            .collect();
        // Aggregate.
        let alpha = |owner: u16| law.per_owner.get(&owner).map_or(law.pooled.0, |x| if x.3 >= 30 { x.0 } else { law.pooled.0 });
        let mut sums_by_set: Vec<Summary> = sets.iter().map(|_| Summary::default()).collect();
        let hot = opts.get("hot").map(|s| s.bytes().enumerate().fold(0u16, |m, (i, c)| m | (((c == b'1') as u16) << i)));
        let mut hot_sums: Vec<Summary> = sets.iter().map(|_| Summary::default()).collect();
        for (_, mult, id, owner, secs, pred, evs) in &results {
            let w = *mult as f64; // PPS: each draw has equal weight; uniform: equal weight per domain
            let _ = (secs, pred);
            for (si, ev) in evs.iter().enumerate() {
                let targets: Vec<&mut Summary> = if Some(*owner) == hot {
                    vec![&mut sums_by_set[si], &mut hot_sums[si]]
                } else {
                    vec![&mut sums_by_set[si]]
                };
                for s in targets {
                    s.w += w;
                    s.n += 1;
                    let Some(ev) = ev else {
                        // Unevaluated (infinite) queries count conservatively: no
                        // coverage, full residual, full cost, gate failed.
                        s.skipped_w += w;
                        s.unc_frac_w += w;
                        for vn in ["exact_pointwise", "d_only", "ar_c2", "hull", "hull_c2", "hull_per_d_run"] {
                            let e = s.voc.entry(vn).or_default();
                            e[1] += w;
                            e[2] += w;
                            e[3] += w;
                            e[4] += w;
                        }
                        continue;
                    };
                    s.evaluated += 1;
                    if ev.single_container {
                        s.single_w += w;
                    }
                    if ev.uncovered == 0.0 {
                        s.full_w += w;
                    }
                    let frac = if ev.points > 0.0 { ev.uncovered / ev.points } else { 0.0 };
                    s.unc_frac_w += w * frac;
                    let al = alpha(*owner);
                    for (vn, p) in [("exact_pointwise", None), ("d_only", Some(&ev.d_only)), ("ar_c2", Some(&ev.ar)), ("hull", Some(&ev.hull)), ("hull_c2", Some(&ev.hull_c2)), ("hull_per_d_run", Some(&ev.hull_d))] {
                        let (pieces, pts, sizes): (usize, f64, Vec<f64>) = match p {
                            None => (if ev.uncovered > 0.0 { 1 } else { 0 }, ev.uncovered, vec![ev.uncovered]),
                            Some(p) => {
                                if ev.uncovered == 0.0 {
                                    (0, 0.0, vec![])
                                } else {
                                    (p.pieces, p.points, p.sizes.clone())
                                }
                            }
                        };
                        let rf = if ev.points > 0.0 { pts / ev.points } else { 0.0 };
                        let gate = rf <= 0.10 && pieces <= 8;
                        // Projected relative cost [E]: sum over pieces of (size/|Q|)^alpha, capped at 1.
                        let proj = if ev.points > 0.0 {
                            sizes.iter().map(|s| (s / ev.points).max(0.0).powf(al)).sum::<f64>().min(1.0)
                        } else {
                            1.0
                        };
                        // Binned-law projection [E]: sum of predicted piece costs over
                        // the predicted cost of Q, capped at 1.
                        let pq = law.predict_binned(*owner, ev.points);
                        let projb = if ev.points > 0.0 && pq > 0.0 {
                            (sizes.iter().filter(|s| **s > 0.0).map(|s| law.predict_binned(*owner, *s)).sum::<f64>() / pq).min(1.0)
                        } else {
                            1.0
                        };
                        let e = s.voc.entry(vn).or_default();
                        e[0] += w * gate as u8 as f64;
                        e[4] += w * projb;
                        e[1] += w * rf;
                        e[2] += w * proj;
                        e[3] += w * pieces as f64;
                    }
                }
            }
            if let Some(f) = &out_rows {
                let row = json!({"sample": name, "id": id, "owner": Dom { owner: *owner, ..Default::default() }.owner_string(n),
                    "mult": mult, "seconds": secs, "predicted_seconds": pred, "admission_gen": util::id_gen(&ck.m, *id as usize),
                    "evals": evs.iter().zip(&sets).map(|(e, s)| (s.to_string(), e.as_ref().map(|e| json!({
                        "points": e.points, "exact": e.exact, "tested": e.tested, "candidates": e.candidates,
                        "single": e.single_container, "uncovered": e.uncovered,
                        "d_only": [e.d_only.pieces, e.d_only.points], "ar": [e.ar.pieces, e.ar.points],
                        "hull": e.hull.points, "hull_c2": e.hull_c2.points, "hull_d": [e.hull_d.pieces, e.hull_d.points],
                        "d_sizes": e.d_only.sizes, "ar_sizes": e.ar.sizes})))).collect::<BTreeMap<_, _>>()});
                let mut f = f.lock().unwrap();
                writeln!(f, "{row}").unwrap();
            }
        }
        let render = |v: &[Summary]| -> Value {
            let mut m = serde_json::Map::new();
            for (s, set) in v.iter().zip(&sets) {
                if s.w == 0.0 {
                    continue;
                }
                let voc: BTreeMap<&str, Value> = s
                    .voc
                    .iter()
                    .map(|(k, e)| {
                        (*k, json!({"gate_share(resid<=10%,pieces<=8)": e[0] / s.w, "mean_residual_fraction": e[1] / s.w,
                            "projected_relative_cost_ols[E]": e[2] / s.w, "projected_relative_cost_binned[E]": e[4] / s.w, "mean_pieces": e[3] / s.w}))
                    })
                    .collect();
                m.insert(set.to_string(), json!({"draws": s.w, "distinct": s.n, "evaluated": s.evaluated,
                    "unevaluated_share(infinite)": s.skipped_w / s.w,
                    "single_container_share": s.single_w / s.w, "fully_covered_share": s.full_w / s.w,
                    "mean_uncovered_fraction": s.unc_frac_w / s.w, "vocabularies": voc}));
            }
            Value::Object(m)
        };
        report.insert(name.to_string(), json!({"all_owners": render(&sums_by_set), "hot_owner": render(&hot_sums)}));
        eprintln!("cover: {name} done ({:.1} s)", t0.elapsed().as_secs_f64());
    }
    println!("{}", serde_json::to_string_pretty(&Value::Object(report)).unwrap());
}


#[cfg(test)]
mod tests {
    use super::*;
    fn rand_dom(rng: &mut Rng, n: usize, owner: u16) -> Dom {
        let mut d = Dom { owner, phase: 0, rank: INF8, amax: NONE16, dmin: DLO_NONE, dmax: DHI_NONE, ..Default::default() };
        for i in 0..n {
            let a = (rng.next() % 4) as u8;
            let b = a + (rng.next() % 4) as u8;
            d.lo[i] = a;
            d.hi[i] = b;
        }
        if rng.next() % 2 == 0 {
            d.amax = (d.t() + 3 + (rng.next() % 6) as i32) as u16;
        }
        if rng.next() % 2 == 0 {
            d.rank = (1 + rng.next() % 6) as u8;
        }
        if rng.next() % 3 == 0 {
            d.dmin = (rng.next() % 5) as i16 - 2;
        }
        if rng.next() % 3 == 0 {
            d.dmax = (rng.next() % 6) as i16;
        }
        d
    }
    /// Pointwise coverage equals brute force; every vocabulary's residual
    /// contains all uncovered points; sampled estimates are close.
    #[test]
    fn evaluate_matches_bruteforce() {
        let n = 4;
        let owner = 0b0011;
        let mut rng = Rng::new(7);
        let mut checked = 0;
        for _ in 0..3000 {
            let q = rand_dom(&mut rng, n, owner);
            let Some(total) = geom::points(&q, n) else { continue };
            if total == 0 {
                continue;
            }
            let anchors: Vec<Anchor> = (0..(1 + rng.next() % 6))
                .map(|k| {
                    let a = rand_dom(&mut rng, n, owner);
                    Anchor::new(k as u32, k as u32, &a, &DomSum::default(), n)
                })
                .collect();
            let mut unc = 0f64;
            let mut unc_pts = Vec::new();
            geom::enumerate(&q, n, &mut |_, x, sa, sr| {
                if !anchors.iter().any(|a| a.hit(n, x, sa + q.t(), sr)) {
                    unc += 1.0;
                    unc_pts.push((*x, sa + q.t(), sr));
                }
            });
            let ev = evaluate(&q, n, &anchors, &|_| true, 1e9, 0, &mut rng).unwrap();
            assert_eq!(ev.uncovered, unc, "{q:?}");
            if unc > 0.0 {
                // Superset property: each vocabulary's residual >= uncovered points,
                // and <= |Q|.
                for p in [&ev.d_only, &ev.ar, &ev.hull, &ev.hull_c2, &ev.hull_d] {
                    assert!(p.points >= unc - 1e-9 && p.points <= total as f64 + 1e-9, "{p:?} {unc} {total}");
                    assert!(p.pieces >= 1);
                }
                // Hull membership: every uncovered point is inside the hull box.
                let mut lo = [u8::MAX; MAXN];
                let mut hi = [0u8; MAXN];
                for (x, _, _) in &unc_pts {
                    for i in 0..n {
                        lo[i] = lo[i].min(x[i]);
                        hi[i] = hi[i].max(x[i]);
                    }
                }
                let _ = (lo, hi);
            } else {
                assert!(ev.d_only.pieces == 0 && ev.ar.pieces == 0);
            }
            checked += 1;
        }
        assert!(checked > 500, "{checked}");
    }
    /// A finite domain that cannot be enumerated (a coordinate above 62) is
    /// unevaluated, never reported as covered.
    #[test]
    fn non_enumerable_domain_is_unevaluated() {
        let n = 2;
        let mut q = Dom { owner: 0b01, phase: 0, rank: INF8, amax: NONE16, dmin: DLO_NONE, dmax: DHI_NONE, ..Default::default() };
        q.hi[0] = 70;
        q.hi[1] = 0;
        assert_eq!(geom::points(&q, n), Some(71));
        let mut rng = Rng::new(1);
        assert!(evaluate(&q, n, &[], &|_| true, 1e9, 0, &mut rng).is_none());
    }
    #[test]
    fn sampler_is_uniform_enough() {
        let n = 4;
        let q = Dom { owner: 0b0101, phase: 0, rank: 5, amax: 8, dmin: DLO_NONE, dmax: DHI_NONE, lo: [0; MAXN], hi: { let mut h = [0u8; MAXN]; h[..4].copy_from_slice(&[4, 3, 4, 3]); h } };
        let l = geom::levels(&q, n).unwrap();
        let s = Sampler::new(&q, n, &l).unwrap();
        let mut counts: HashMap<[u8; MAXN], usize> = HashMap::new();
        let mut rng = Rng::new(3);
        let draws = 200000;
        for _ in 0..draws {
            let (x, sa, sr) = s.sample(&mut rng);
            assert!(geom::contains_point(&q, n, &x, sa + q.t(), sr));
            *counts.entry(x).or_default() += 1;
        }
        let total = l.total() as f64;
        assert_eq!(counts.len() as f64, total);
        let exp = draws as f64 / total;
        for c in counts.values() {
            assert!((*c as f64 - exp).abs() < 6.0 * exp.sqrt() + 5.0, "{c} vs {exp}");
        }
    }
}

// --------------------------------------------------------- saturation
/// Draw one uniform point of `d` (finite, nonempty).
pub fn draw_point(d: &Dom, n: usize, rng: &mut Rng) -> Option<([u8; MAXN], i32, i32)> {
    let l = geom::levels(d, n)?;
    let s = Sampler::new(d, n, &l)?;
    let (x, sa, sr) = s.sample(rng);
    Some((x, sa + l.t, sr))
}

/// Point-space saturation per Apply owner (plan M-pts): the union of native
/// domains committed by the end of each record segment (generation) and of
/// all admitted domains by the end of each domain segment, estimated by the
/// first-cover decomposition |U| = sum_i |D_i minus earlier D_j|. Each draw
/// picks a domain with probability proportional to its points (or to its
/// native seconds) and one uniform point in it, and tests that point against
/// the earlier domains of the same owner.
pub fn saturation(dir: &Path, opts: &Opts) {
    let ck = util::load(dir, opts, true);
    let n = ck.n;
    let sums = util::dom_sums(&ck.doms, n);
    let draws = opts.num("draws", 400usize);
    let seed = opts.num("seed", 20260927u64);
    let b = buckets(&ck, &sums, true);
    // Record windows (natives) and ID windows (admitted).
    let mut rwin: Vec<(u64, u32, u32)> = Vec::new();
    let mut s0 = 0u32;
    for seg in &ck.m.records {
        rwin.push((seg.generation, s0, s0 + seg.count as u32));
        s0 += seg.count as u32;
    }
    if rwin.is_empty() {
        rwin.push((0, 0, ck.recs.len() as u32));
    }
    let mut iwin: Vec<(u64, u32, u32)> = ck.m.domains.iter().map(|s| (s.generation, s.first as u32, (s.first + s.count) as u32)).collect();
    if iwin.is_empty() {
        iwin.push((0, 0, ck.doms.len() as u32));
    }
    // Tasks: (owner, kind 0 natives / 1 admitted, window index).
    let owners: Vec<u16> = b.natives.keys().copied().collect();
    let mut tasks = Vec::new();
    for &o in &owners {
        for w in 0..rwin.len() {
            tasks.push((o, 0u8, w));
        }
        for w in 0..iwin.len() {
            tasks.push((o, 1u8, w));
        }
    }
    let results: Vec<(u16, u8, usize, Value)> = tasks
        .par_iter()
        .map(|&(o, kind, w)| {
            let mut rng = Rng::new(seed ^ ((o as u64) << 20) ^ ((kind as u64) << 16) ^ w as u64);
            let pool: &[Anchor] = if kind == 0 { &b.natives[&o] } else { b.all.get(&o).map(|v| v.as_slice()).unwrap_or(&[]) };
            // Members of the window.
            let members: Vec<&Anchor> = pool
                .iter()
                .filter(|a| {
                    let key = if kind == 0 { a.seq } else { a.id };
                    let (_, lo, hi) = if kind == 0 { rwin[w] } else { iwin[w] };
                    key >= lo && key < hi
                })
                .collect();
            let (mut npts, mut nsec, mut ninf, mut sum_pts, mut sum_sec) = (0usize, 0usize, 0usize, 0f64, 0f64);
            let wp: Vec<f64> = members
                .iter()
                .map(|a| {
                    let s = &sums[a.id as usize];
                    if s.infinite {
                        ninf += 1;
                        0.0
                    } else {
                        npts += 1;
                        sum_pts += s.points;
                        s.points
                    }
                })
                .collect();
            let ws: Vec<f64> = members
                .iter()
                .map(|a| {
                    let ix = ck.native_of[a.id as usize];
                    let sec = if ix == u32::MAX { 0.0 } else { ck.recs[ix as usize].seconds as f64 };
                    if sec > 0.0 {
                        nsec += 1;
                    }
                    sum_sec += sec;
                    sec
                })
                .collect();
            let mut est = |weights: &[f64], rng: &mut Rng| -> (f64, usize) {
                let picks = pps(weights, draws, rng);
                let (mut new, mut tot) = (0usize, 0usize);
                for (i, mult) in picks {
                    let a = members[i];
                    let d = &ck.doms[a.id as usize];
                    for _ in 0..mult {
                        let Some((x, av, rv)) = draw_point(d, n, rng) else { continue };
                        tot += 1;
                        let covered = pool.iter().any(|c| {
                            let earlier = if kind == 0 { c.seq < a.seq } else { c.id < a.id };
                            earlier && c.hit(n, &x, av, rv)
                        });
                        if !covered {
                            new += 1;
                        }
                    }
                }
                (if tot > 0 { new as f64 / tot as f64 } else { f64::NAN }, tot)
            };
            let (f_pts, t_pts) = est(&wp, &mut rng);
            let (f_sec, t_sec) = if kind == 0 { est(&ws, &mut rng) } else { (f64::NAN, 0) };
            let v = json!({"members": members.len(), "finite": npts, "infinite": ninf, "sum_points": sum_pts,
                "sum_seconds": sum_sec, "new_fraction_points": f_pts, "draws_points": t_pts,
                "new_fraction_cpu": f_sec, "draws_cpu": t_sec, "with_seconds": nsec,
                "new_points_est": f_pts * sum_pts});
            (o, kind, w, v)
        })
        .collect();
    // Assemble per owner, cumulative unions.
    let mut per: BTreeMap<String, Value> = BTreeMap::new();
    let mut tot_nat: BTreeMap<u64, f64> = BTreeMap::new();
    let mut tot_adm: BTreeMap<u64, f64> = BTreeMap::new();
    let mut tot_nat_n: BTreeMap<u64, f64> = BTreeMap::new();
    for &o in &owners {
        let name = Dom { owner: o, ..Default::default() }.owner_string(n);
        let mut nat = Vec::new();
        let mut adm = Vec::new();
        let (mut un, mut ua) = (0f64, 0f64);
        for (oo, kind, w, v) in &results {
            if *oo != o {
                continue;
            }
            let np = v["new_points_est"].as_f64().unwrap_or(0.0);
            let np = if np.is_finite() { np } else { 0.0 };
            if *kind == 0 {
                un += np;
                let g = rwin[*w].0;
                *tot_nat.entry(g).or_default() += np;
                *tot_nat_n.entry(g).or_default() += v["members"].as_f64().unwrap();
                let per_native = if v["members"].as_f64().unwrap() > 0.0 { np / v["members"].as_f64().unwrap() } else { 0.0 };
                nat.push(json!({"gen": g, "window": v, "union_after": un, "new_points_per_native": per_native}));
            } else {
                ua += np;
                let g = iwin[*w].0;
                *tot_adm.entry(g).or_default() += np;
                adm.push(json!({"gen": g, "window": v, "union_after": ua}));
            }
        }
        per.insert(name, json!({"natives": nat, "admitted": adm}));
    }
    let mut cum = 0.0;
    let nat_tot: Vec<Value> = tot_nat
        .iter()
        .map(|(g, v)| {
            cum += v;
            json!({"gen": g, "new_points": v, "natives": tot_nat_n[g], "new_points_per_native": v / tot_nat_n[g].max(1.0), "union_after": cum})
        })
        .collect();
    let mut cum = 0.0;
    let adm_tot: Vec<Value> = tot_adm
        .iter()
        .map(|(g, v)| {
            cum += v;
            json!({"gen": g, "new_points": v, "union_after": cum})
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&json!({"dir": dir, "draws": draws, "totals_natives": nat_tot, "totals_admitted": adm_tot, "owners": per})).unwrap());
}

// --------------------------------------------------------------- pilot
/// Overlap of a drained pilot's closure (its `result.json`) with a CP5
/// checkpoint: exact identical domains, and pointwise coverage of the
/// checkpoint's domains by the pilot's natives of the same (phase, owner)
/// (what importing the pilot's natives as merged anchors would discharge),
/// plus the reverse direction.
pub fn pilot(dir: &Path, opts: &Opts) {
    let ck = util::load(dir, opts, true);
    let n = ck.n;
    let pp = opts.get("pilot").expect("--pilot RESULT.json");
    let pk = util::load_result(Path::new(pp), n);
    let k = opts.num("k", 3000usize);
    let cap = opts.num("cap", 2.0e6f64);
    let samples = opts.num("samples", 20000usize);
    let seed = opts.num("seed", 20260927u64);
    let hot = opts.get("hot").map(|s| s.bytes().enumerate().fold(0u16, |m, (i, c)| m | (((c == b'1') as u16) << i)));
    let sums = util::dom_sums(&ck.doms, n);
    let psums = util::dom_sums(&pk.doms, n);
    let law = cost_law(&ck, &sums, 0);
    // Pilot pools per bucket (natives) and exact set of all pilot domains.
    let mut ppool: HashMap<u32, Vec<Anchor>> = HashMap::new();
    let mut pset: std::collections::HashSet<Dom> = Default::default();
    for r in &pk.recs {
        let d = &pk.doms[r.id as usize];
        pset.insert(*d);
        if recs::is_native(r.kind) && !psums[r.id as usize].empty {
            ppool.entry(d.bucket()).or_default().push(Anchor::new(r.id, r.seq, d, &psums[r.id as usize], n));
        }
    }
    let mut vpool: HashMap<u32, Vec<Anchor>> = HashMap::new();
    for r in &ck.recs {
        if recs::is_native(r.kind) && !sums[r.id as usize].empty {
            let d = &ck.doms[r.id as usize];
            vpool.entry(d.bucket()).or_default().push(Anchor::new(r.id, r.seq, d, &sums[r.id as usize], n));
        }
    }
    let ledger = ck.ledger.as_ref();
    let class = |id: usize| -> usize {
        if ck.native_of[id] != u32::MAX {
            0
        } else if ledger.is_some_and(|l| matches!(l.entries[id].state, L_UNRESERVED | L_RESERVED | L_STARTED)) {
            1
        } else {
            2
        }
    };
    // Exact identical domains, by class x phase, restricted to pilot buckets.
    let mut exact: BTreeMap<String, [usize; 3]> = BTreeMap::new(); // [in pilot buckets, identical, total]
    for (id, d) in ck.doms.iter().enumerate() {
        let key = format!("{}|{}|{}", ["native", "pending", "delegated"][class(id)], if d.phase == 0 { "Apply" } else { "Route" }, if Some(d.owner) == hot && d.phase == 0 { "hot" } else { "other" });
        let e = exact.entry(key).or_default();
        e[2] += 1;
        if ppool.contains_key(&d.bucket()) {
            e[0] += 1;
            if pset.contains(d) {
                e[1] += 1;
            }
        }
    }
    let mut rng = Rng::new(seed);
    let in_pilot = |id: usize| ppool.contains_key(&ck.doms[id].bucket()) && !sums[id].empty && !sums[id].infinite;
    let v2_nat: Vec<usize> = (0..ck.doms.len()).filter(|&id| class(id) == 0 && ck.doms[id].phase == 0 && in_pilot(id)).collect();
    let v2_pend: Vec<usize> = (0..ck.doms.len()).filter(|&id| class(id) == 1 && ck.doms[id].phase == 0 && in_pilot(id)).collect();
    let nat_sec: Vec<f64> = v2_nat.iter().map(|&id| ck.recs[ck.native_of[id] as usize].seconds as f64).collect();
    let pend_pred: Vec<f64> = v2_pend.iter().map(|&id| law.predict_binned(ck.doms[id].owner, sums[id].points)).collect();
    let p_nat: Vec<usize> = pk.recs.iter().filter(|r| recs::is_native(r.kind) && r.phase == 0 && !psums[r.id as usize].empty && !psums[r.id as usize].infinite).map(|r| r.id as usize).collect();
    let p_sec: Vec<f64> = p_nat.iter().map(|&id| pk.recs[pk.native_of[id] as usize].seconds as f64).collect();
    let plans: Vec<(&str, bool, Vec<usize>, Vec<(usize, usize)>)> = vec![
        ("v2_apply_natives_pps_seconds", true, v2_nat.clone(), pps(&nat_sec, k, &mut rng)),
        ("v2_apply_pending_pps_predicted", true, v2_pend.clone(), pps(&pend_pred, k, &mut rng)),
        ("v2_apply_pending_uniform", true, v2_pend.clone(), uniform(v2_pend.len(), k, &mut rng)),
        ("pilot_apply_natives_pps_seconds_vs_v2", false, p_nat.clone(), pps(&p_sec, k, &mut rng)),
    ];
    let mut report = serde_json::Map::new();
    report.insert("dir".into(), json!(dir));
    report.insert("pilot".into(), json!(pp));
    report.insert("pilot_records".into(), json!(pk.recs.len()));
    report.insert("pilot_buckets".into(), json!(ppool.len()));
    report.insert("exact_identical[class|phase|hot: in_pilot_buckets, identical, total]".into(), json!(exact));
    report.insert("v2_apply_natives_in_pilot_buckets".into(), json!([v2_nat.len(), nat_sec.iter().sum::<f64>()]));
    report.insert("v2_apply_pending_in_pilot_buckets".into(), json!([v2_pend.len(), pend_pred.iter().sum::<f64>()]));
    report.insert("pilot_apply_natives".into(), json!([p_nat.len(), p_sec.iter().sum::<f64>()]));
    for (name, v2side, pop, plan) in plans {
        let res: Vec<(u16, usize, Option<Eval>)> = plan
            .par_iter()
            .map(|&(ix, mult)| {
                let mut rng = Rng::new(seed ^ (ix as u64).wrapping_mul(0x2545F4914F6CDD1D));
                let id = pop[ix];
                let (q, pool) = if v2side {
                    (&ck.doms[id], ppool.get(&ck.doms[id].bucket()))
                } else {
                    (&pk.doms[id], vpool.get(&pk.doms[id].bucket()))
                };
                let pool = pool.map(|v| v.as_slice()).unwrap_or(&[]);
                (q.owner, mult, evaluate(q, n, pool, &|_| true, cap, samples, &mut rng))
            })
            .collect();
        let mut agg: BTreeMap<&str, [f64; 7]> = BTreeMap::new(); // [draws, full, gate hull, gate d_only, mean uncovered frac, proj cost hull, unevaluated]
        for (owner, mult, ev) in &res {
            let w = *mult as f64;
            for key in ["all", if Some(*owner) == hot { "hot" } else { "other" }] {
                let e = agg.entry(key).or_default();
                e[0] += w;
                let Some(ev) = ev else {
                    // Unevaluated (infinite) queries count conservatively: not
                    // covered, gates failed, full residual, full cost.
                    e[4] += w;
                    e[5] += w;
                    e[6] += w;
                    continue;
                };
                let rf = |p: f64| if ev.points > 0.0 { p / ev.points } else { 0.0 };
                e[1] += w * (ev.uncovered == 0.0) as u8 as f64;
                let hull = if ev.uncovered == 0.0 { 0.0 } else { ev.hull.points };
                let donly = if ev.uncovered == 0.0 { (0usize, 0.0) } else { (ev.d_only.pieces, ev.d_only.points) };
                e[2] += w * (rf(hull) <= 0.1) as u8 as f64;
                e[3] += w * (rf(donly.1) <= 0.1 && donly.0 <= 8) as u8 as f64;
                e[4] += w * rf(ev.uncovered);
                let pq = law.predict_binned(*owner, ev.points);
                e[5] += w * if hull > 0.0 { (law.predict_binned(*owner, hull) / pq).min(1.0) } else { 0.0 };
            }
        }
        let v: BTreeMap<&str, Value> = agg
            .iter()
            .map(|(k2, e)| (*k2, json!({"draws": e[0], "fully_covered_share": e[1] / e[0], "gate_hull_share": e[2] / e[0],
                "gate_d_only_share": e[3] / e[0], "mean_uncovered_fraction": e[4] / e[0], "projected_relative_cost_hull[E]": e[5] / e[0], "unevaluated_share(infinite)": e[6] / e[0]})))
            .collect();
        report.insert(name.to_string(), json!(v));
        eprintln!("pilot: {name} done");
    }
    println!("{}", serde_json::to_string_pretty(&Value::Object(report)).unwrap());
}

/// Candidate lists bucketed by the query's two widest coordinates, so a
/// point is tested only against anchors whose range holds its values there.
struct Grid<'a> {
    axes: [usize; 2],
    lo: [u8; 2],
    dims: [usize; 2],
    cells: Vec<Vec<&'a Anchor>>,
}
impl<'a> Grid<'a> {
    fn new(cands: &[&'a Anchor], q: &Anchor, n: usize) -> Self {
        let mut spans: Vec<(usize, usize)> = (0..n).map(|i| ((q.hi[i] - q.lo[i]) as usize + 1, i)).collect();
        spans.sort_by(|a, b| b.cmp(a));
        let flat = || Grid { axes: [0, 0], lo: [0, 0], dims: [1, 1], cells: vec![cands.to_vec()] };
        if cands.len() <= 16 || spans.is_empty() || spans[0].0 < 2 {
            return flat();
        }
        let (i1, i2) = (spans[0].1, if n > 1 && spans[1].0 >= 2 { spans[1].1 } else { spans[0].1 });
        let two = i1 != i2;
        let dims = [spans[0].0, if two { spans[1].0 } else { 1 }];
        let ov = |a: &Anchor, i: usize| -> (usize, usize) {
            let lo = a.lo[i].max(q.lo[i]);
            let hi = a.hi[i].min(q.hi[i]);
            ((lo - q.lo[i]) as usize, (hi - q.lo[i]) as usize)
        };
        let entries: usize = cands
            .iter()
            .map(|a| {
                let (l1, h1) = ov(a, i1);
                let w2 = if two { let (l2, h2) = ov(a, i2); h2 - l2 + 1 } else { 1 };
                (h1 - l1 + 1) * w2
            })
            .sum();
        let (two, dims) = if entries > 40_000_000 && two { (false, [dims[0], 1]) } else { (two, dims) };
        let mut cells: Vec<Vec<&Anchor>> = vec![Vec::new(); dims[0] * dims[1]];
        for &a in cands {
            let (l1, h1) = ov(a, i1);
            let (l2, h2) = if two { ov(a, i2) } else { (0, 0) };
            for v1 in l1..=h1 {
                for v2 in l2..=h2 {
                    cells[v1 * dims[1] + v2].push(a);
                }
            }
        }
        Grid { axes: [i1, if two { i2 } else { i1 }], lo: [q.lo[i1], if two { q.lo[i2] } else { 0 }], dims, cells }
    }
    #[inline]
    fn cell(&mut self, x: &[u8; MAXN]) -> &mut Vec<&'a Anchor> {
        if self.cells.len() == 1 {
            return &mut self.cells[0];
        }
        let v1 = (x[self.axes[0]] - self.lo[0]) as usize;
        let v2 = if self.dims[1] > 1 { (x[self.axes[1]] - self.lo[1]) as usize } else { 0 };
        &mut self.cells[v1 * self.dims[1] + v2]
    }
}
