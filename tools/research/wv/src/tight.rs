//! Exact extrema of a domain's lattice point set and the resulting exact
//! inclusion test, independent of the engine's containment code.
//!
//! A domain (see `geom.rs`) is {x in N^n : lo <= x <= hi, A <= amax,
//! R <= rank, dmin <= D <= dmax} with A = t + sum_active x, R = sum_inactive x,
//! D = A - R. A and R are sums of integer intervals over disjoint axis
//! groups, so each takes every integer between its box extremes and (A, R)
//! ranges over a full integer rectangle; the D band cuts that rectangle to
//! the feasible sets
//!   A in [max(a_lo, dmin + r_lo), min(a_hi, dmax + r_hi)],
//!   R in [max(r_lo, a_lo - dmax), min(r_hi, a_hi - dmin)],
//!   D in [max(dmin, a_lo - r_hi), min(dmax, a_hi - r_lo)]
//! (a_hi, r_hi already capped by amax / rank). A coordinate x_i of an active
//! axis reaches x_i = v iff t + v + sum_{j != i} lo_j <= A_max_feasible and
//! t + v + sum_{j != i} hi_j >= A_min_feasible (the others fill the rest), so
//!   max x_i = min(hi_i, Afmax - t - sum_{j != i} lo_j),
//!   min x_i = max(lo_i, Afmin - t - sum_{j != i} hi_j),
//! and likewise for inactive axes with R. Q <= C (as point sets) iff every
//! linear defining inequality of C holds at the matching extremum of Q, i.e.
//! C.lo <= min x, max x <= C.hi, max A <= C.amax, max R <= C.rank,
//! min D >= C.dmin, max D <= C.dmax. Integer interval arithmetic only.
use crate::geom::{Dom, DHI_NONE, DLO_NONE, INF8, MAXN, NONE16};

/// "Infinity" in the i16 key encoding (all finite values are < 300).
pub const BIG: i16 = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tight {
    pub empty: bool,
    pub min: [i16; MAXN],
    /// BIG = unbounded.
    pub max: [i16; MAXN],
    pub amax: i16,
    pub rmax: i16,
    /// -BIG = unbounded below.
    pub dmin: i16,
    pub dmax: i16,
}

fn opt(v: i64) -> Option<i64> {
    (v < BIG as i64 / 2 && v > -(BIG as i64) / 2).then_some(v)
}

pub fn tight(d: &Dom, n: usize) -> Tight {
    let t = d.t() as i64;
    // Box sums per group (None = +inf).
    let (mut a_lo, mut a_hi, mut r_lo, mut r_hi) = (t, Some(t), 0i64, Some(0i64));
    let (mut na_inf, mut nr_inf) = (0usize, 0usize);
    let (mut sa_hi_fin, mut sr_hi_fin) = (0i64, 0i64);
    for i in 0..n {
        let lo = d.lo[i] as i64;
        let hi = (d.hi[i] != INF8).then_some(d.hi[i] as i64);
        if d.active(i) {
            a_lo += lo;
            a_hi = a_hi.zip(hi).map(|(s, h)| s + h);
            match hi {
                Some(h) => sa_hi_fin += h,
                None => na_inf += 1,
            }
        } else {
            r_lo += lo;
            r_hi = r_hi.zip(hi).map(|(s, h)| s + h);
            match hi {
                Some(h) => sr_hi_fin += h,
                None => nr_inf += 1,
            }
        }
    }
    let amax = (d.amax != NONE16).then_some(d.amax as i64);
    let rank = (d.rank != INF8).then_some(d.rank as i64);
    let dmin = (d.dmin != DLO_NONE).then_some(d.dmin as i64);
    let dmax = (d.dmax != DHI_NONE).then_some(d.dmax as i64);
    let cap = |x: Option<i64>, c: Option<i64>| match (x, c) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    let a_hi = cap(a_hi, amax);
    let r_hi = cap(r_hi, rank);
    // Feasible A, R, D ranges (None = unbounded).
    let fa_lo = match dmin {
        Some(dn) => a_lo.max(dn + r_lo),
        None => a_lo,
    };
    let fa_hi = cap(a_hi, dmax.zip(r_hi).map(|(dx, rh)| dx + rh));
    let fr_lo = match dmax {
        Some(dx) => r_lo.max(a_lo - dx),
        None => r_lo,
    };
    let fr_hi = cap(r_hi, a_hi.zip(dmin).map(|(ah, dn)| ah - dn));
    let fd_lo = match (dmin, r_hi) {
        (Some(dn), Some(rh)) => Some(dn.max(a_lo - rh)),
        (Some(dn), None) => Some(dn),
        (None, Some(rh)) => Some(a_lo - rh),
        (None, None) => None,
    };
    let fd_hi = cap(dmax, a_hi.map(|ah| ah - r_lo));
    let empty = fa_hi.is_some_and(|h| h < fa_lo)
        || fr_hi.is_some_and(|h| h < fr_lo)
        || matches!((fd_lo, fd_hi), (Some(l), Some(h)) if l > h)
        || (0..n).any(|i| d.hi[i] != INF8 && d.hi[i] < d.lo[i]);
    let mut out = Tight {
        empty,
        min: [0; MAXN],
        max: [0; MAXN],
        amax: fa_hi.map_or(BIG, |v| v as i16),
        rmax: fr_hi.map_or(BIG, |v| v as i16),
        dmin: fd_lo.map_or(-BIG, |v| v as i16),
        dmax: fd_hi.map_or(BIG, |v| v as i16),
    };
    if empty {
        return out;
    }
    for i in 0..n {
        let lo = d.lo[i] as i64;
        let hi = (d.hi[i] != INF8).then_some(d.hi[i] as i64);
        let (glo, fmin, fmax, base, sum_hi_fin, ninf) = if d.active(i) {
            (a_lo, fa_lo, fa_hi, t, sa_hi_fin, na_inf)
        } else {
            (r_lo, fr_lo, fr_hi, 0, sr_hi_fin, nr_inf)
        };
        let _ = base;
        // Others' low sum and high sum (None if another axis is unbounded).
        let others_lo = glo - lo; // includes t for the active group
        let others_hi = if hi.is_none() {
            (ninf == 1).then_some(sum_hi_fin)
        } else {
            (ninf == 0).then(|| sum_hi_fin - hi.unwrap())
        };
        // group total = x_i + others, others in [others_lo_raw, others_hi_raw]
        // (for the active group others_lo already contains t; add t to hi).
        let others_hi = others_hi.map(|s| if d.active(i) { s + d.t() as i64 } else { s });
        let mx = cap(hi, fmax.map(|f| f - others_lo));
        let mn = match others_hi {
            Some(oh) => lo.max(fmin - oh),
            None => lo,
        };
        out.min[i] = mn as i16;
        out.max[i] = mx.map_or(BIG, |v| v as i16);
        debug_assert!(opt(mn).is_some());
    }
    out
}

/// Container key: C contains Q iff key(C) <= query(Q) componentwise.
pub const fn key_len(n: usize) -> usize {
    2 * n + 4
}

pub fn container_key(c: &Dom, n: usize, out: &mut [i16]) {
    for i in 0..n {
        out[i] = c.lo[i] as i16;
        out[n + i] = if c.hi[i] == INF8 {
            -BIG
        } else {
            -(c.hi[i] as i16)
        };
    }
    out[2 * n] = if c.amax == NONE16 {
        -BIG
    } else {
        -(c.amax as i16)
    };
    out[2 * n + 1] = if c.rank == INF8 {
        -BIG
    } else {
        -(c.rank as i16)
    };
    out[2 * n + 2] = if c.dmin == DLO_NONE { -BIG } else { c.dmin };
    out[2 * n + 3] = if c.dmax == DHI_NONE { -BIG } else { -c.dmax };
}

/// Query key of the image of Q under a slot permutation: coordinate j of the
/// image is coordinate `src[j]` of Q (`source_for_target`).
pub fn query_key(q: &Tight, n: usize, src: &[usize], out: &mut [i16]) {
    for j in 0..n {
        let s = src[j];
        out[j] = q.min[s];
        out[n + j] = if q.max[s] >= BIG { -BIG } else { -q.max[s] };
    }
    out[2 * n] = if q.amax >= BIG { -BIG } else { -q.amax };
    out[2 * n + 1] = if q.rmax >= BIG { -BIG } else { -q.rmax };
    out[2 * n + 2] = q.dmin; // -BIG if unbounded below
    out[2 * n + 3] = if q.dmax >= BIG { -BIG } else { -q.dmax };
}

#[inline]
pub fn dominated(c: &[i16], q: &[i16]) -> bool {
    c.iter().zip(q).all(|(a, b)| a <= b)
}

/// Apply a slot permutation to a domain (image coordinate j = source src[j]).
pub fn permute(d: &Dom, n: usize, src: &[usize]) -> Dom {
    let mut o = *d;
    o.owner = 0;
    for j in 0..n {
        let s = src[j];
        o.lo[j] = d.lo[s];
        o.hi[j] = d.hi[s];
        if d.active(s) {
            o.owner |= 1 << j;
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom;

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, k: u64) -> u64 {
            self.next() % k
        }
    }

    fn random_dom(rng: &mut Rng, n: usize, owner: u16) -> Dom {
        let mut d = Dom {
            owner,
            phase: 0,
            rank: INF8,
            amax: NONE16,
            dmin: DLO_NONE,
            dmax: DHI_NONE,
            ..Default::default()
        };
        for i in 0..n {
            d.lo[i] = rng.below(3) as u8;
            d.hi[i] = if rng.below(4) == 0 {
                INF8
            } else {
                d.lo[i] + rng.below(4) as u8
            };
        }
        if rng.below(2) == 0 {
            d.rank = rng.below(9) as u8;
        }
        if rng.below(2) == 0 {
            d.amax = (d.t() as u64 + rng.below(9)) as u16;
        }
        if rng.below(2) == 0 {
            d.dmin = rng.below(12) as i16 - 6;
        }
        if rng.below(2) == 0 {
            d.dmax = rng.below(12) as i16 - 4;
        }
        d
    }

    /// Clip unbounded domains to a finite window for enumeration.
    fn clip(d: &Dom, n: usize, w: u8) -> Dom {
        let mut c = *d;
        for i in 0..n {
            if c.hi[i] == INF8 {
                c.hi[i] = c.lo[i] + w;
            }
        }
        c
    }

    fn points(d: &Dom, n: usize) -> Vec<([u8; MAXN], i32, i32)> {
        let mut v = Vec::new();
        geom::enumerate(d, n, &mut |_, x, sa, sr| v.push((*x, sa + d.t(), sr)));
        v
    }

    #[test]
    fn extrema_match_enumeration() {
        let n = 5;
        let mut rng = Rng(0x9e3779b97f4a7c15);
        for _ in 0..20000 {
            let owner = rng.below(1 << n) as u16;
            let d = random_dom(&mut rng, n, owner);
            if (0..n).any(|i| d.hi[i] == INF8) {
                continue;
            }
            let t = tight(&d, n);
            let pts = points(&d, n);
            assert_eq!(t.empty, pts.is_empty(), "{d:?}");
            if pts.is_empty() {
                continue;
            }
            for i in 0..n {
                let mn = pts.iter().map(|p| p.0[i] as i16).min().unwrap();
                let mx = pts.iter().map(|p| p.0[i] as i16).max().unwrap();
                assert_eq!((t.min[i], t.max[i]), (mn, mx), "axis {i} {d:?}");
            }
            assert_eq!(
                t.amax,
                pts.iter().map(|p| p.1 as i16).max().unwrap(),
                "{d:?}"
            );
            assert_eq!(
                t.rmax,
                pts.iter().map(|p| p.2 as i16).max().unwrap(),
                "{d:?}"
            );
            assert_eq!(
                t.dmin,
                pts.iter().map(|p| (p.1 - p.2) as i16).min().unwrap(),
                "{d:?}"
            );
            assert_eq!(
                t.dmax,
                pts.iter().map(|p| (p.1 - p.2) as i16).max().unwrap(),
                "{d:?}"
            );
        }
    }

    #[test]
    fn inclusion_matches_enumeration() {
        let n = 4;
        let mut rng = Rng(0x2545f4914f6cdd1d);
        let mut positives = 0;
        let (mut kq, mut kc) = (vec![0i16; key_len(n)], vec![0i16; key_len(n)]);
        let ident: Vec<usize> = (0..n).collect();
        for _ in 0..200000 {
            let owner = rng.below(1 << n) as u16;
            let q = random_dom(&mut rng, n, owner);
            let mut c = random_dom(&mut rng, n, owner);
            if rng.below(3) == 0 {
                // widen c around q to create positives
                c = q;
                for i in 0..n {
                    c.lo[i] = c.lo[i].saturating_sub(rng.below(2) as u8);
                    if c.hi[i] != INF8 && rng.below(2) == 0 {
                        c.hi[i] = if rng.below(3) == 0 { INF8 } else { c.hi[i] + 1 };
                    }
                }
                if rng.below(2) == 0 {
                    c.rank = INF8;
                }
            }
            let tq = tight(&q, n);
            if tq.empty {
                continue;
            }
            container_key(&c, n, &mut kc);
            query_key(&tq, n, &ident, &mut kq);
            let claim = dominated(&kc, &kq);
            // Enumerate q on a window large enough to expose any violation
            // along unbounded axes (all finite bounds are < 12).
            let qw = clip(&q, n, 14);
            let brute = points(&qw, n)
                .iter()
                .all(|(x, a, r)| geom::contains_point(&c, n, x, *a, *r));
            // If q is unbounded, an unbounded axis of q against a finite c.hi
            // is a violation the window exposes; A/R/D unbounded likewise.
            assert_eq!(claim, brute, "q {q:?} c {c:?}");
            positives += claim as usize;
        }
        assert!(positives > 1000, "{positives}");
    }

    #[test]
    fn permutation_commutes_with_tight() {
        let n = 5;
        let mut rng = Rng(77);
        let src = [2usize, 0, 1, 4, 3];
        for _ in 0..5000 {
            let owner = rng.below(1 << n) as u16;
            let d = random_dom(&mut rng, n, owner);
            let p = permute(&d, n, &src);
            let (tp, td) = (tight(&p, n), tight(&d, n));
            assert_eq!(tp.empty, td.empty);
            if td.empty {
                continue;
            }
            for j in 0..n {
                assert_eq!((tp.min[j], tp.max[j]), (td.min[src[j]], td.max[src[j]]));
            }
            assert_eq!(
                (tp.amax, tp.rmax, tp.dmin, tp.dmax),
                (td.amax, td.rmax, td.dmin, td.dmax)
            );
        }
    }
}
