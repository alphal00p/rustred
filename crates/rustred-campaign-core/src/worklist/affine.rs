//! Integer affine equations `sum c_i n_i + constant = 0` over the slots of a key, their canonical systems (integer
//! RREF, unique for the rational solution space) and the FIT fallback through left-over points (R32: marked `fit`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

use crate::poly::{D_VAR, Poly};

/// `sum coeffs[k].1 * n_{coeffs[k].0} + constant = 0`. Normalised: slots ascending, no zero coefficient, primitive
/// (gcd of all coefficients and the constant is 1), first coefficient positive.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AffineEq {
    pub coeffs: Vec<(usize, i64)>,
    pub constant: i64,
}

/// Why a system of case equations could not be canonicalised.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AffineError {
    /// The equations have no rational solution (e.g. `0 = 1` after substitution).
    Inconsistent,
    /// A single-slot equation forces a non-integer value (e.g. `2 n0 = 1`): no integer point.
    NoIntegerPoint,
    /// Arithmetic left the i64 range.
    Overflow,
}

impl fmt::Display for AffineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AffineError::Inconsistent => write!(f, "inconsistent equations"),
            AffineError::NoIntegerPoint => write!(f, "no integer point"),
            AffineError::Overflow => write!(f, "overflow"),
        }
    }
}

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

impl AffineEq {
    /// Normalised equation; `None` when every coefficient is zero (then the equation is `constant = 0`) or when the
    /// normalised coefficients leave the i64 range (use [`Self::try_new`] to tell the two apart).
    pub fn new(coeffs: Vec<(usize, i64)>, constant: i64) -> Option<Self> {
        Self::try_new(coeffs, constant).ok().flatten()
    }

    /// [`Self::new`] with overflow reported (`Err(AffineError::Overflow)`), never a silent truncation (audit A).
    pub fn try_new(coeffs: Vec<(usize, i64)>, constant: i64) -> Result<Option<Self>, AffineError> {
        let mut m: BTreeMap<usize, i128> = BTreeMap::new();
        for (s, c) in coeffs {
            *m.entry(s).or_insert(0) += c as i128;
        }
        let terms: Vec<(usize, i128)> = m.into_iter().filter(|(_, c)| *c != 0).collect();
        if terms.is_empty() {
            return Ok(None);
        }
        let mut g = constant as i128;
        for (_, c) in &terms {
            g = gcd(g, *c);
        }
        let sign = if terms[0].1 < 0 { -1 } else { 1 };
        let g = g.max(1) * sign;
        let coeffs = terms
            .iter()
            .map(|(s, c)| i64::try_from(*c / g).map(|c| (*s, c)))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| AffineError::Overflow)?;
        let constant = i64::try_from(constant as i128 / g).map_err(|_| AffineError::Overflow)?;
        Ok(Some(AffineEq { coeffs, constant }))
    }

    /// `n_slot = value`.
    pub fn fixed(slot: usize, value: i32) -> Self {
        AffineEq {
            coeffs: vec![(slot, 1)],
            constant: -(value as i64),
        }
    }

    /// Value of the left-hand side at a key (exact, i128).
    pub fn eval(&self, key: &[i32]) -> Option<i128> {
        let mut v = self.constant as i128;
        for &(s, c) in &self.coeffs {
            v += c as i128 * *key.get(s)? as i128;
        }
        Some(v)
    }

    /// The equation holds at the key (false if the key is too short).
    pub fn holds(&self, key: &[i32]) -> bool {
        self.eval(key) == Some(0)
    }

    pub fn slots(&self) -> Vec<usize> {
        self.coeffs.iter().map(|(s, _)| *s).collect()
    }

    /// `Some((slot, value))` if this is `n_slot = value`.
    pub fn as_fixed(&self) -> Option<(usize, i32)> {
        if self.coeffs.len() == 1 && self.coeffs[0].1 == 1 {
            i32::try_from(-self.constant)
                .ok()
                .map(|v| (self.coeffs[0].0, v))
        } else {
            None
        }
    }

    /// Read an equation from a d-free polynomial of degree <= 1 (an exception literal or a pole coefficient).
    pub fn from_poly(p: &Poly) -> Option<Self> {
        if p.has_d() || p.degree() > 1 {
            return None;
        }
        let mut coeffs = Vec::new();
        let mut constant: i128 = 0;
        for (mono, c) in p.terms() {
            match mono.as_slice() {
                [] => constant += *c,
                [(v, 1)] if *v != D_VAR => coeffs.push((*v as usize, i64::try_from(*c).ok()?)),
                _ => return None,
            }
        }
        AffineEq::try_new(coeffs, i64::try_from(constant).ok()?)
            .ok()
            .flatten()
    }

    /// Engine spelling of the left-hand side (`--initial 'SPEC [EQS]'` takes linear equations `= 0`): "n6+2*n11-1".
    pub fn to_engine(&self) -> String {
        let mut s = String::new();
        for (k, &(slot, c)) in self.coeffs.iter().enumerate() {
            let sign = if c < 0 {
                "-"
            } else if k > 0 {
                "+"
            } else {
                ""
            };
            let a = c.abs();
            if a == 1 {
                s.push_str(&format!("{sign}n{slot}"));
            } else {
                s.push_str(&format!("{sign}{a}*n{slot}"));
            }
        }
        if self.constant > 0 {
            s.push_str(&format!("+{}", self.constant));
        } else if self.constant < 0 {
            s.push_str(&format!("{}", self.constant));
        }
        s
    }

    /// Parse "a = b" (lenient equation form, as the checker's locus files) or "expr" (meaning expr = 0) through the
    /// core polynomial parser; affine only.
    pub fn parse(text: &str, arity: usize) -> Result<Self, String> {
        let opts = crate::poly::ParseOptions {
            allow_d: false,
            allow_equation: true,
        };
        let p = Poly::parse_with(text, arity, opts).map_err(|e| e.to_string())?;
        AffineEq::from_poly(&p).ok_or_else(|| format!("not an affine equation with a slot: {text}"))
    }
}

impl fmt::Display for AffineEq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} = 0", self.to_engine())
    }
}

/// Canonical form of case equations over the free slots: fixed values are substituted, the system is brought to
/// integer reduced row echelon form (pivots on the lowest slot first, every row primitive with a positive pivot:
/// unique for the rational solution space), and every row with a single slot becomes a fixed value (repeated until
/// stable). Returns the canonical equations and the enlarged fixed map.
pub fn canonical_system(
    eqs: &[AffineEq],
    fixed: &BTreeMap<usize, i32>,
) -> Result<(Vec<AffineEq>, BTreeMap<usize, i32>), AffineError> {
    let mut fixed = fixed.clone();
    let mut current: Vec<AffineEq> = eqs.to_vec();
    loop {
        // substitute fixed values
        let mut subst = Vec::new();
        for e in &current {
            let mut constant = e.constant as i128;
            let mut coeffs = Vec::new();
            for &(s, c) in &e.coeffs {
                match fixed.get(&s) {
                    Some(&v) => constant += c as i128 * v as i128,
                    None => coeffs.push((s, c)),
                }
            }
            let constant = i64::try_from(constant).map_err(|_| AffineError::Overflow)?;
            match AffineEq::try_new(coeffs, constant)? {
                Some(n) => subst.push(n),
                None if constant == 0 => {}
                None => return Err(AffineError::Inconsistent),
            }
        }
        let rows = rref(&subst)?;
        let mut changed = false;
        let mut keep = Vec::new();
        for r in rows {
            if r.coeffs.len() == 1 {
                let (s, c) = r.coeffs[0];
                if r.constant % c != 0 {
                    return Err(AffineError::NoIntegerPoint);
                }
                let v = i32::try_from(-r.constant / c).map_err(|_| AffineError::Overflow)?;
                fixed.insert(s, v);
                changed = true;
            } else {
                // gcd test: the equation has an integer solution only if gcd(coeffs) divides the constant
                let g = r.coeffs.iter().fold(0i128, |g, (_, c)| gcd(g, *c as i128));
                if g > 1 && (r.constant as i128) % g != 0 {
                    return Err(AffineError::NoIntegerPoint);
                }
                keep.push(r);
            }
        }
        if !changed {
            return Ok((keep, fixed));
        }
        current = keep;
    }
}

/// Integer RREF (see [`canonical_system`]); rows are primitive, pivots positive, pivot columns cleared elsewhere.
pub fn rref(eqs: &[AffineEq]) -> Result<Vec<AffineEq>, AffineError> {
    let mut slots: Vec<usize> = eqs.iter().flat_map(|e| e.slots()).collect();
    slots.sort_unstable();
    slots.dedup();
    let n = slots.len();
    let col = |s: usize| slots.binary_search(&s).ok();
    let mut m: Vec<Vec<i128>> = eqs
        .iter()
        .map(|e| {
            let mut r = vec![0i128; n + 1];
            for &(s, c) in &e.coeffs {
                r[col(s).unwrap_or(0)] += c as i128;
            }
            r[n] = e.constant as i128;
            r
        })
        .collect();
    let mut prow = 0;
    for c in 0..n {
        let Some(p) = (prow..m.len()).find(|&r| m[r][c] != 0) else {
            continue;
        };
        m.swap(prow, p);
        for r in 0..m.len() {
            if r != prow && m[r][c] != 0 {
                let a = m[prow][c];
                let b = m[r][c];
                let pivot_row = m[prow].clone();
                for (x, &y) in m[r].iter_mut().zip(&pivot_row) {
                    *x = x
                        .checked_mul(a)
                        .and_then(|u| y.checked_mul(b).and_then(|w| u.checked_sub(w)))
                        .ok_or(AffineError::Overflow)?;
                }
                normalise_row(&mut m[r]);
            }
        }
        normalise_row(&mut m[prow]);
        if m[prow][c] < 0 {
            m[prow].iter_mut().for_each(|x| *x = -*x);
        }
        prow += 1;
    }
    let mut out = Vec::new();
    for r in &m {
        if r[..n].iter().all(|&x| x == 0) {
            if r[n] != 0 {
                return Err(AffineError::Inconsistent);
            }
            continue;
        }
        let coeffs = (0..n)
            .filter(|&k| r[k] != 0)
            .map(|k| i64::try_from(r[k]).map(|c| (slots[k], c)))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| AffineError::Overflow)?;
        let constant = i64::try_from(r[n]).map_err(|_| AffineError::Overflow)?;
        if let Some(e) = AffineEq::try_new(coeffs, constant)? {
            out.push(e);
        }
    }
    out.sort();
    Ok(out)
}

fn normalise_row(r: &mut [i128]) {
    let g = r.iter().fold(0i128, |g, &x| gcd(g, x));
    if g > 1 {
        r.iter_mut().for_each(|x| *x /= g);
    }
}

/// FIT fallback (R32: only when no exception branch of a rule holds the points; marked `Derived::Fit`): groups of
/// at least two points sharing a slot value, then a constant difference of two slots, greedily (largest group
/// first; ties by slot order). Returns (equation, indices of the points it holds); points in no group are absent.
pub fn fit_loci(points: &[Vec<i32>], slots: &[usize]) -> Vec<(AffineEq, Vec<usize>)> {
    let mut left: Vec<usize> = (0..points.len()).collect();
    let mut out = Vec::new();
    loop {
        let mut best: Option<(AffineEq, Vec<usize>)> = None;
        let consider = |eq: AffineEq, best: &mut Option<(AffineEq, Vec<usize>)>| {
            let held: Vec<usize> = left
                .iter()
                .copied()
                .filter(|&i| eq.holds(&points[i]))
                .collect();
            if held.len() >= 2 && best.as_ref().is_none_or(|(_, b)| held.len() > b.len()) {
                *best = Some((eq, held));
            }
        };
        for &s in slots {
            let mut vals: Vec<i32> = left.iter().map(|&i| points[i][s]).collect();
            vals.sort_unstable();
            vals.dedup();
            for v in vals {
                consider(AffineEq::fixed(s, v), &mut best);
            }
        }
        for (a, &s) in slots.iter().enumerate() {
            for &t in &slots[a + 1..] {
                let mut diffs: Vec<i64> = left
                    .iter()
                    .map(|&i| points[i][s] as i64 - points[i][t] as i64)
                    .collect();
                diffs.sort_unstable();
                diffs.dedup();
                for dlt in diffs {
                    if let Some(eq) = AffineEq::new(vec![(s, 1), (t, -1)], -dlt) {
                        consider(eq, &mut best);
                    }
                }
            }
        }
        if best.is_none() {
            // general affine line in two slots with integer (possibly non-unit) coefficients through >= 3 points
            // (R32: e.g. n6 + 2 n11 - 1 = 0), from the first two points of every pair of slots
            'pairs: for (a, &s) in slots.iter().enumerate() {
                for &t in &slots[a + 1..] {
                    for (x, &i) in left.iter().enumerate() {
                        for &j in &left[x + 1..] {
                            let (p, q) = (&points[i], &points[j]);
                            let (dx, dy) = (q[s] as i64 - p[s] as i64, q[t] as i64 - p[t] as i64);
                            if dx == 0 && dy == 0 {
                                continue;
                            }
                            // dy * (n_s - p_s) - dx * (n_t - p_t) = 0
                            let c = -(dy * p[s] as i64) + dx * p[t] as i64;
                            if let Some(eq) = AffineEq::new(vec![(s, dy), (t, -dx)], c) {
                                let held: Vec<usize> = left
                                    .iter()
                                    .copied()
                                    .filter(|&k| eq.holds(&points[k]))
                                    .collect();
                                if held.len() >= 3 {
                                    best = Some((eq, held));
                                    break 'pairs;
                                }
                            }
                        }
                    }
                }
            }
        }
        match best {
            Some((eq, held)) => {
                left.retain(|i| !held.contains(i));
                out.push((eq, held));
            }
            None => return out,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalise_and_engine_text() {
        let e = AffineEq::new(vec![(11, -4), (6, -2)], 2).unwrap();
        assert_eq!(e.coeffs, vec![(6, 1), (11, 2)]);
        assert_eq!(e.constant, -1);
        assert_eq!(e.to_engine(), "n6+2*n11-1");
        assert_eq!(AffineEq::parse("n6=-2*n11+1", 20).unwrap(), e);
    }

    #[test]
    fn rref_unique_and_fixed_extraction() {
        let a = vec![
            AffineEq::parse("n1-n3", 20).unwrap(),
            AffineEq::parse("n3-n4", 20).unwrap(),
        ];
        let b = vec![
            AffineEq::parse("n1-n4", 20).unwrap(),
            AffineEq::parse("n4-n3", 20).unwrap(),
        ];
        assert_eq!(rref(&a).unwrap(), rref(&b).unwrap());
        let (eqs, fx) = canonical_system(
            &[
                AffineEq::parse("n0-n4", 20).unwrap(),
                AffineEq::parse("n4-2", 20).unwrap(),
            ],
            &BTreeMap::new(),
        )
        .unwrap();
        assert!(eqs.is_empty());
        assert_eq!(fx.get(&0), Some(&2));
        assert_eq!(fx.get(&4), Some(&2));
        assert_eq!(
            canonical_system(&[AffineEq::parse("2*n0-1", 20).unwrap()], &BTreeMap::new()),
            Err(AffineError::NoIntegerPoint)
        );
        assert_eq!(
            canonical_system(
                &[AffineEq::parse("4*n0+4*n11-1", 20).unwrap()],
                &BTreeMap::new()
            ),
            Err(AffineError::NoIntegerPoint)
        );
    }

    #[test]
    fn fit_groups() {
        let pts = vec![vec![2, 2, 2], vec![3, 3, 3], vec![2, 5, 4]];
        let f = fit_loci(&pts, &[0, 1, 2]);
        assert_eq!(f[0].1, vec![0, 2]); // n0 = 2 holds two points (first by slot order among ties)
    }
}
