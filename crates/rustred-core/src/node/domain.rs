//! Integer domain of a queued case before any lift (T211 lesson, coordinator 2026-11-11T03:39Z): a hard case was
//! lifted although its integer domain in the chart was EMPTY (n18 + n19 = -1 with both ISP powers <= 0 and the two
//! zero faces owned elsewhere). A case is the set of integer points of its linear equations (fixed coordinates are
//! equations too) inside the chart's orthant: physical slots n >= 1, ISP slots n <= 0. This module decides, by
//! interval propagation, gcd tests and a capped enumeration, whether that set is empty, finite (then the case is
//! closed by point rules / numeric endpoints, and a union of earlier rules can be checked point by point) or
//! infinite. It never claims emptiness or finiteness it has not proved: anything else is `Unknown`.

use std::collections::BTreeMap;

/// Integer domain of a case in the chart orthant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IntegerDomain {
    /// No integer point (proved).
    Empty,
    /// Finitely many points, all listed (full coordinate vectors, presented slots).
    Finite(Vec<Vec<i64>>),
    /// At least one point and infinitely many (a free coordinate is unbounded).
    Infinite,
    /// Not decided (unparsed equation, unbounded coupled coordinates, or more points than the cap).
    Unknown,
}

/// Parse a linear form `c0 + c1*n_i + ...` as printed by the engine (`1-n11-n2+n0`, `2*n3-n4`, `-n5+3`).
/// Returns (constant, coefficient per slot) or None when the text is not a linear form in `n<k>`.
pub fn parse_linear(text: &str, arity: usize) -> Option<(i64, Vec<i64>)> {
    let t: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if t.is_empty() {
        return None;
    }
    let mut constant = 0i64;
    let mut coeffs = vec![0i64; arity];
    let bytes = t.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let mut sign = 1i64;
        while i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
            if bytes[i] == b'-' {
                sign = -sign;
            }
            i += 1;
        }
        let start = i;
        while i < bytes.len() && bytes[i] != b'+' && bytes[i] != b'-' {
            i += 1;
        }
        let term = &t[start..i];
        if term.is_empty() {
            return None;
        }
        let (factor, var) = match term.split_once('*') {
            Some((a, b)) => {
                if let Some(v) = a.strip_prefix('n') {
                    (b, Some(v))
                } else {
                    (a, b.strip_prefix('n'))
                }
            }
            None => match term.strip_prefix('n') {
                Some(v) => ("1", Some(v)),
                None => (term, None),
            },
        };
        let c: i64 = factor.parse().ok()?;
        match var {
            Some(v) => {
                let k: usize = v.parse().ok()?;
                if k >= arity {
                    return None;
                }
                coeffs[k] = coeffs[k].checked_add(sign.checked_mul(c)?)?;
            }
            None => {
                if term.contains('*') {
                    return None; // a product of two non-variables or an unknown symbol
                }
                constant = constant.checked_add(sign.checked_mul(c)?)?;
            }
        }
    }
    Some((constant, coeffs))
}

fn gcd(a: i64, b: i64) -> i64 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// Decide the integer domain of a case: `fixed[i]` (presented slots, None = free), `equations` (each `= 0`),
/// `physical[i]` (true: n >= 1, false: n <= 0). Finite domains with more than `cap` points, or whose bounding box
/// has more than `box_cap` points, are `Unknown`.
pub fn integer_domain(fixed: &[Option<i64>], equations: &[String], physical: &[bool], cap: usize, box_cap: u64) -> IntegerDomain {
    let n = fixed.len();
    // fixed values outside the orthant: empty
    for i in 0..n {
        if let Some(v) = fixed[i] {
            if (physical[i] && v < 1) || (!physical[i] && v > 0) {
                return IntegerDomain::Empty;
            }
        }
    }
    let free: Vec<usize> = (0..n).filter(|&i| fixed[i].is_none()).collect();
    // y_i >= 0 with x_i = 1 + y_i (physical) or x_i = -y_i (ISP); rows: sum c_i y_i = r
    let mut rows: Vec<(BTreeMap<usize, i64>, i64)> = Vec::new();
    for e in equations {
        let Some((c0, a)) = parse_linear(e, n) else {
            return IntegerDomain::Unknown;
        };
        let mut r = -c0;
        let mut row = BTreeMap::new();
        for i in 0..n {
            if a[i] == 0 {
                continue;
            }
            match fixed[i] {
                Some(v) => r -= a[i] * v,
                None => {
                    if physical[i] {
                        r -= a[i];
                        row.insert(i, a[i]);
                    } else {
                        row.insert(i, -a[i]);
                    }
                }
            }
        }
        if row.is_empty() {
            if r != 0 {
                return IntegerDomain::Empty;
            }
            continue;
        }
        let g = row.values().fold(0, |g, &c| gcd(g, c));
        if r % g != 0 {
            return IntegerDomain::Empty;
        }
        rows.push((row, r));
    }
    // interval propagation on y >= 0: ub[i]
    let mut ub: BTreeMap<usize, i64> = BTreeMap::new();
    for _ in 0..(2 * free.len() + 2) {
        let mut changed = false;
        for (row, r) in &rows {
            for (&j, &cj) in row {
                // cj y_j = r - sum_{k != j} c_k y_k; bound the RHS above/below with y_k in [0, ub_k]
                let (mut hi, mut lo) = (Some(*r as i128), Some(*r as i128));
                for (&k, &ck) in row {
                    if k == j {
                        continue;
                    }
                    let u = ub.get(&k).copied();
                    // - ck y_k: max is (ck < 0 ? -ck*ub : 0), min is (ck > 0 ? -ck*ub : 0)
                    if ck < 0 {
                        hi = match (hi, u) {
                            (Some(h), Some(u)) => Some(h + (-(ck as i128)) * u as i128),
                            _ => None,
                        };
                    } else {
                        lo = match (lo, u) {
                            (Some(l), Some(u)) => Some(l - (ck as i128) * u as i128),
                            _ => None,
                        };
                    }
                }
                // cj > 0: y_j <= hi / cj ; cj < 0: y_j <= -lo / |cj| ... i.e. |cj| y_j <= -lo
                let bound = if cj > 0 { hi.map(|h| h.div_euclid(cj as i128)) } else { lo.map(|l| (-l).div_euclid(-(cj as i128))) };
                if let Some(b) = bound {
                    if b < 0 {
                        return IntegerDomain::Empty;
                    }
                    let b = b.min(i64::MAX as i128) as i64;
                    if ub.get(&j).is_none_or(|&old| b < old) {
                        ub.insert(j, b);
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    let coupled: Vec<usize> = {
        let mut v: Vec<usize> = rows.iter().flat_map(|(r, _)| r.keys().copied()).collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    let unbounded_free = free.iter().any(|i| !coupled.contains(i));
    if coupled.iter().any(|i| !ub.contains_key(i)) {
        return IntegerDomain::Unknown;
    }
    // enumerate the coupled coordinates in their box
    let mut size: u64 = 1;
    for &i in &coupled {
        size = size.saturating_mul(ub[&i] as u64 + 1);
        if size > box_cap {
            return IntegerDomain::Unknown;
        }
    }
    let mut points: Vec<Vec<i64>> = Vec::new();
    let mut y = vec![0i64; coupled.len()];
    'outer: loop {
        let ok = rows.iter().all(|(row, r)| {
            let s: i128 = row.iter().map(|(k, c)| *c as i128 * y[coupled.iter().position(|x| x == k).unwrap()] as i128).sum();
            s == *r as i128
        });
        if ok {
            if unbounded_free {
                return IntegerDomain::Infinite;
            }
            let mut p: Vec<i64> = fixed.iter().map(|v| v.unwrap_or(0)).collect();
            for (t, &i) in coupled.iter().enumerate() {
                p[i] = if physical[i] { 1 + y[t] } else { -y[t] };
            }
            points.push(p);
            if points.len() > cap {
                return IntegerDomain::Unknown;
            }
        }
        // next y
        let mut t = 0;
        loop {
            if t == coupled.len() {
                break 'outer;
            }
            if y[t] < ub[&coupled[t]] {
                y[t] += 1;
                break;
            }
            y[t] = 0;
            t += 1;
        }
    }
    if points.is_empty() {
        IntegerDomain::Empty
    } else if unbounded_free {
        IntegerDomain::Infinite
    } else {
        IntegerDomain::Finite(points)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_engine_linear_forms() {
        assert_eq!(parse_linear("1-n11-n2+n0", 12), Some((1, { let mut v = vec![0; 12]; v[11] = -1; v[2] = -1; v[0] = 1; v })));
        assert_eq!(parse_linear("2*n3-n4+3", 5), Some((3, vec![0, 0, 0, 2, -1])));
        assert_eq!(parse_linear("n1*3", 2), Some((0, vec![0, 3])));
        assert_eq!(parse_linear("n1^2", 2), None);
        assert_eq!(parse_linear("d-n1", 2), None);
    }

    #[test]
    fn t211_two_isp_sum_is_finite_and_a_negative_physical_sum_is_empty() {
        // slots: 0 physical, 1, 2 ISP. n1 + n2 = -1 with n1, n2 <= 0: points (0,-1), (-1,0); n0 free -> infinite
        let phys = [true, false, false];
        assert_eq!(integer_domain(&[None, None, None], &["n1+n2+1".into()], &phys, 64, 1 << 20), IntegerDomain::Infinite);
        // n0 fixed: finite, two points
        match integer_domain(&[Some(1), None, None], &["n1+n2+1".into()], &phys, 64, 1 << 20) {
            IntegerDomain::Finite(p) => {
                assert_eq!(p.len(), 2);
                assert!(p.contains(&vec![1, 0, -1]) && p.contains(&vec![1, -1, 0]));
            }
            d => panic!("{d:?}"),
        }
        // n1 + n2 = +1 with both ISPs: empty
        assert_eq!(integer_domain(&[Some(1), None, None], &["n1+n2-1".into()], &phys, 64, 1 << 20), IntegerDomain::Empty);
        // physical n0 = 0: empty (fixed outside the orthant through an equation)
        assert_eq!(integer_domain(&[None, Some(0), Some(0)], &["n0".into()], &phys, 64, 1 << 20), IntegerDomain::Empty);
        // gcd: 2 n1 - 2 n2 = 1 has no integer solution
        assert_eq!(integer_domain(&[Some(1), None, None], &["2*n1-2*n2-1".into()], &phys, 64, 1 << 20), IntegerDomain::Empty);
        // a mixed-sign equation leaves the coordinates unbounded: undecided
        assert_eq!(integer_domain(&[Some(1), None, None], &["n1-n2".into()], &phys, 64, 1 << 20), IntegerDomain::Unknown);
        // T206 refused case: 1 - n11 - n2 + n0 = 0 with n0, n2, n11 physical (n0 = n2 + n11 - 1): infinite domain
        let phys12 = [true; 12];
        let mut fixed = vec![Some(1i64); 12];
        fixed[0] = None;
        fixed[2] = None;
        fixed[11] = None;
        assert_eq!(integer_domain(&fixed, &["1-n11-n2+n0".into()], &phys12, 64, 1 << 20), IntegerDomain::Unknown);
    }

    #[test]
    fn fixed_point_outside_the_orthant_is_empty() {
        assert_eq!(integer_domain(&[Some(0), None], &[], &[true, false], 64, 1 << 20), IntegerDomain::Empty);
        assert_eq!(integer_domain(&[Some(1), None], &[], &[true, false], 64, 1 << 20), IntegerDomain::Infinite);
    }
}
