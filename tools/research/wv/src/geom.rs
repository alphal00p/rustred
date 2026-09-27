//! Lattice geometry of one walk domain, read-only and independent of the
//! engine. A domain of arity n is a box in local coordinates x_i >= 0 with
//! aggregate predicates. Active coordinates (owner bit set) carry positive
//! powers a_i = x_i + 1, inactive ones numerator powers r_i = x_i. With t the
//! number of active coordinates: A = t + sum_active x, R = sum_inactive x,
//! D = A - R, P = A + R. Predicates: A <= amax, R <= rank, dmin <= D <= dmax
//! (each optional). This mirrors `DomainPowerBounds` in rustred-core
//! (`power_domain/mod.rs`: "Positive local coordinates are n=x+1").
//!
//! Only integer box arithmetic and point counting live here; nothing
//! algebraic (no polynomials) is computed.

pub const MAXN: usize = 16;
pub const INF8: u8 = u8::MAX;
pub const NONE16: u16 = u16::MAX;
pub const DLO_NONE: i16 = i16::MIN;
pub const DHI_NONE: i16 = i16::MAX;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Dom {
    pub owner: u16,
    /// 0 Apply, 1 Route.
    pub phase: u8,
    /// INF8 = no rank bound.
    pub rank: u8,
    /// NONE16 = no bound on A.
    pub amax: u16,
    pub dmin: i16,
    pub dmax: i16,
    pub lo: [u8; MAXN],
    /// INF8 = unbounded coordinate.
    pub hi: [u8; MAXN],
}

impl Dom {
    pub fn t(&self) -> i32 {
        self.owner.count_ones() as i32
    }
    pub fn active(&self, i: usize) -> bool {
        self.owner >> i & 1 == 1
    }
    pub fn owner_string(&self, n: usize) -> String {
        (0..n).map(|i| if self.active(i) { '1' } else { '0' }).collect()
    }
    /// Bucket key (phase, owner) as one u32.
    pub fn bucket(&self) -> u32 {
        (self.phase as u32) << 16 | self.owner as u32
    }
}

/// Caps on the sums of active and inactive local coordinates, derived as in
/// the lens tool `rtool` (`limits`): A <= amax gives sum_active <= amax - t;
/// without amax, A = D + R <= dmax + rank. Without rank, R = A - D <= A_max - dmin.
#[derive(Clone, Copy, Debug)]
pub struct Caps {
    pub t: i32,
    pub acap: Option<i32>,
    pub rcap: Option<i32>,
}

pub fn caps(d: &Dom) -> Caps {
    let t = d.t();
    let rank = (d.rank != INF8).then_some(d.rank as i32);
    let dmin = (d.dmin != DLO_NONE).then_some(d.dmin as i32);
    let dmax = (d.dmax != DHI_NONE).then_some(d.dmax as i32);
    let mut acap = (d.amax != NONE16).then(|| d.amax as i32 - t);
    let mut rcap = rank;
    if acap.is_none() {
        if let (Some(dx), Some(r)) = (dmax, rcap) {
            acap = Some(r + dx - t);
        }
    }
    if rcap.is_none() {
        if let (Some(dn), Some(a)) = (dmin, acap) {
            rcap = Some(a + t - dn);
        }
    }
    Caps { t, acap, rcap }
}

/// Number of vectors with lo_i <= x_i <= hi_i and sum = s, for s in 0..=cap.
/// None if the group is unbounded (no cap and an infinite coordinate).
fn dist(bounds: &[(u8, u8)], cap: Option<i32>) -> Option<Vec<u128>> {
    let cap = match cap {
        Some(c) => c,
        None => {
            if bounds.iter().any(|b| b.1 == INF8) {
                return None;
            }
            bounds.iter().map(|b| b.1 as i32).sum()
        }
    };
    if cap < 0 {
        return Some(vec![]);
    }
    let cap = cap as usize;
    let mut counts = vec![0u128; cap + 1];
    counts[0] = 1;
    let mut pre = vec![0u128; cap + 2];
    for &(lo, hi) in bounds {
        let lo = lo as usize;
        let hi2 = if hi == INF8 { cap } else { (hi as usize).min(cap) };
        if lo > hi2 {
            return Some(vec![0; cap + 1]);
        }
        for s in 0..=cap {
            pre[s + 1] = pre[s] + counts[s];
        }
        for s in 0..=cap {
            counts[s] = if s < lo {
                0
            } else {
                let b = s - lo;
                let a = s.saturating_sub(hi2);
                pre[b + 1] - pre[a]
            };
        }
    }
    Some(counts)
}

/// Point counts by level (A, R): `levels[(ai, ri)]` with A = t + ai, R = ri.
#[derive(Clone, Debug, Default)]
pub struct Levels {
    pub t: i32,
    /// da[s]: active vectors with sum s; dr[s]: inactive vectors with sum s.
    pub da: Vec<u128>,
    pub dr: Vec<u128>,
    pub dmin: Option<i32>,
    pub dmax: Option<i32>,
}

impl Levels {
    pub fn d_ok(&self, d: i32) -> bool {
        self.dmin.is_none_or(|m| d >= m) && self.dmax.is_none_or(|m| d <= m)
    }
    /// Count at level (sa, sr) (local sums).
    pub fn at(&self, sa: usize, sr: usize) -> u128 {
        let a = sa as i32 + self.t;
        if sa >= self.da.len() || sr >= self.dr.len() || !self.d_ok(a - sr as i32) {
            return 0;
        }
        self.da[sa] * self.dr[sr]
    }
    pub fn total(&self) -> u128 {
        let mut s = 0u128;
        for (sa, &ca) in self.da.iter().enumerate() {
            if ca == 0 {
                continue;
            }
            for (sr, &cr) in self.dr.iter().enumerate() {
                if cr != 0 && self.d_ok(sa as i32 + self.t - sr as i32) {
                    s += ca * cr;
                }
            }
        }
        s
    }
    /// Extrema over nonempty levels: (Amin, Amax, Rmin, Rmax, Pmin, Pmax, Dmin, Dmax).
    pub fn extrema(&self) -> Option<[i32; 8]> {
        let mut e: Option<[i32; 8]> = None;
        for (sa, &ca) in self.da.iter().enumerate() {
            if ca == 0 {
                continue;
            }
            for (sr, &cr) in self.dr.iter().enumerate() {
                let a = sa as i32 + self.t;
                let r = sr as i32;
                if cr == 0 || !self.d_ok(a - r) {
                    continue;
                }
                let v = [a, a, r, r, a + r, a + r, a - r, a - r];
                e = Some(match e {
                    None => v,
                    Some(o) => [
                        o[0].min(a),
                        o[1].max(a),
                        o[2].min(r),
                        o[3].max(r),
                        o[4].min(a + r),
                        o[5].max(a + r),
                        o[6].min(a - r),
                        o[7].max(a - r),
                    ],
                });
            }
        }
        e
    }
}

pub fn levels(d: &Dom, n: usize) -> Option<Levels> {
    let c = caps(d);
    let act: Vec<(u8, u8)> = (0..n).filter(|&i| d.active(i)).map(|i| (d.lo[i], d.hi[i])).collect();
    let ina: Vec<(u8, u8)> = (0..n).filter(|&i| !d.active(i)).map(|i| (d.lo[i], d.hi[i])).collect();
    let da = dist(&act, c.acap)?;
    let dr = dist(&ina, c.rcap)?;
    Some(Levels {
        t: c.t,
        da,
        dr,
        dmin: (d.dmin != DLO_NONE).then_some(d.dmin as i32),
        dmax: (d.dmax != DHI_NONE).then_some(d.dmax as i32),
    })
}

/// Total lattice points, None if infinite.
pub fn points(d: &Dom, n: usize) -> Option<u128> {
    levels(d, n).map(|l| l.total())
}

/// Effective finite upper bound per coordinate (None if unbounded).
pub fn eff_hi(d: &Dom, n: usize) -> Option<[u8; MAXN]> {
    let c = caps(d);
    let mut hi = [0u8; MAXN];
    for i in 0..n {
        let cap = if d.active(i) { c.acap } else { c.rcap };
        let h = match (d.hi[i], cap) {
            (INF8, None) => return None,
            (INF8, Some(c)) => c.max(0),
            (h, None) => h as i32,
            (h, Some(c)) => (h as i32).min(c.max(0)),
        };
        if h >= 63 {
            return None;
        }
        hi[i] = h as u8;
    }
    Some(hi)
}

/// Enumerate all points; `out(key, x, sa, sr)` with key = 6 bits per
/// coordinate (coordinate 0 most significant). Returns false if the domain
/// is infinite or a coordinate exceeds 62.
pub fn enumerate(d: &Dom, n: usize, out: &mut dyn FnMut(u128, &[u8; MAXN], i32, i32)) -> bool {
    let Some(hi) = eff_hi(d, n) else {
        return false;
    };
    let c = caps(d);
    if (0..n).any(|i| d.lo[i] > hi[i]) {
        return true;
    }
    let mut rest_a = [0i32; MAXN + 1];
    let mut rest_r = [0i32; MAXN + 1];
    for i in (0..n).rev() {
        rest_a[i] = rest_a[i + 1] + if d.active(i) { d.lo[i] as i32 } else { 0 };
        rest_r[i] = rest_r[i + 1] + if d.active(i) { 0 } else { d.lo[i] as i32 };
    }
    let dmin = (d.dmin != DLO_NONE).then_some(d.dmin as i32);
    let dmax = (d.dmax != DHI_NONE).then_some(d.dmax as i32);
    let mut x = [0u8; MAXN];
    struct Ctx<'a> {
        n: usize,
        d: &'a Dom,
        hi: [u8; MAXN],
        acap: Option<i32>,
        rcap: Option<i32>,
        t: i32,
        dmin: Option<i32>,
        dmax: Option<i32>,
        rest_a: [i32; MAXN + 1],
        rest_r: [i32; MAXN + 1],
    }
    fn rec(
        c: &Ctx,
        i: usize,
        key: u128,
        sa: i32,
        sr: i32,
        x: &mut [u8; MAXN],
        out: &mut dyn FnMut(u128, &[u8; MAXN], i32, i32),
    ) {
        if i == c.n {
            let dd = sa + c.t - sr;
            if c.dmin.is_none_or(|m| dd >= m) && c.dmax.is_none_or(|m| dd <= m) {
                out(key, x, sa, sr);
            }
            return;
        }
        let act = c.d.active(i);
        for v in c.d.lo[i]..=c.hi[i] {
            let (na, nr) = if act { (sa + v as i32, sr) } else { (sa, sr + v as i32) };
            if c.acap.is_some_and(|cap| na + c.rest_a[i + 1] > cap)
                || c.rcap.is_some_and(|cap| nr + c.rest_r[i + 1] > cap)
            {
                break;
            }
            x[i] = v;
            rec(c, i + 1, (key << 6) | v as u128, na, nr, x, out);
        }
    }
    let ctx = Ctx {
        n,
        d,
        hi,
        acap: c.acap,
        rcap: c.rcap,
        t: c.t,
        dmin,
        dmax,
        rest_a,
        rest_r,
    };
    rec(&ctx, 0, 0, 0, 0, &mut x, out);
    true
}

/// Point membership (same bucket assumed): box, A, R and D predicates.
pub fn contains_point(d: &Dom, n: usize, x: &[u8; MAXN], a: i32, r: i32) -> bool {
    for i in 0..n {
        if x[i] < d.lo[i] || (d.hi[i] != INF8 && x[i] > d.hi[i]) {
            return false;
        }
    }
    if d.amax != NONE16 && a > d.amax as i32 {
        return false;
    }
    if d.rank != INF8 && r > d.rank as i32 {
        return false;
    }
    let dd = a - r;
    !((d.dmin != DLO_NONE && dd < d.dmin as i32) || (d.dmax != DHI_NONE && dd > d.dmax as i32))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dom(owner: u16, lo: &[u8], hi: &[u8], rank: u8, amax: u16, dmin: i16, dmax: i16) -> Dom {
        let mut d = Dom { owner, phase: 0, rank, amax, dmin, dmax, ..Default::default() };
        d.lo[..lo.len()].copy_from_slice(lo);
        d.hi[..hi.len()].copy_from_slice(hi);
        d
    }
    /// Brute force over a small box agrees with the level convolution and
    /// with the enumerator, including the derived caps.
    #[test]
    fn count_matches_bruteforce() {
        let n = 4;
        let cases = [
            dom(0b0011, &[0, 0, 0, 0], &[3, 4, 2, 5], 4, 7, -2, 6),
            dom(0b0101, &[1, 0, 0, 1], &[4, 3, 3, 4], INF8, 8, 1, DHI_NONE),
            dom(0b1111, &[0, 0, 0, 0], &[INF8, INF8, 3, INF8], INF8, 9, DLO_NONE, DHI_NONE),
            dom(0b0001, &[0, 0, 0, 0], &[5, INF8, INF8, 2], 5, NONE16, DLO_NONE, 3),
        ];
        for d in cases {
            let t = d.t();
            let mut brute = 0u128;
            for x0 in 0..10u8 {
                for x1 in 0..10u8 {
                    for x2 in 0..10u8 {
                        for x3 in 0..10u8 {
                            let x = [x0, x1, x2, x3];
                            let mut xx = [0u8; MAXN];
                            xx[..4].copy_from_slice(&x);
                            let a = t + (0..4).filter(|&i| d.active(i)).map(|i| x[i] as i32).sum::<i32>();
                            let r = (0..4).filter(|&i| !d.active(i)).map(|i| x[i] as i32).sum::<i32>();
                            if contains_point(&d, n, &xx, a, r) {
                                brute += 1;
                            }
                        }
                    }
                }
            }
            let mut en = 0u128;
            assert!(enumerate(&d, n, &mut |_, x, sa, sr| {
                assert!(contains_point(&d, n, x, sa + t, sr));
                en += 1;
            }));
            assert_eq!(points(&d, n), Some(brute), "{d:?}");
            assert_eq!(en, brute);
        }
    }
}
