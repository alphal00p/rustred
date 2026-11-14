//! INDEPENDENT ALGEBRA (see mod.rs for why this exists). Small exact linear algebra, MOVED here unchanged in substance
//! from the domain files (rows3, 2026-11-14): `det_i128`, `inverse_q`, `rank_q` (replay_base/symrows.rs, campaign
//! code), the Gauss-Jordan of `solve_equations` (ibp_rows.rs, campaign code), Lee's affine-hull test and the
//! polynomial determinant of `zero` / `graph_polys` (replay_base/family.rs, engine copy b9f06de7 of
//! node/replay/family.rs; the copy's private `inverse` was identical to `inverse_q` and is gone).
//! Duplicates numerica `Matrix<Q>`: `det` (lib/numerica/src/tensors/matrix.rs:1256), `inv` (1731), `rank` (2023),
//! `row_reduce` (2016), and Symbolica polynomial arithmetic (`MultivariatePolynomial<Z>`, src/poly/polynomial.rs).

use std::collections::HashMap;

use super::arith::{Q, QArith};
use crate::replay_base::algebra::{Unknown, ZPoly};

/// Bareiss fraction-free determinant, checked (`None` on overflow).
pub fn det_i128(m: &[Vec<i128>]) -> Option<i128> {
    let mut a: Vec<Vec<i128>> = m.to_vec();
    let n = a.len();
    if n == 0 {
        return Some(1);
    }
    let mut sign = 1i128;
    let mut prev = 1i128;
    for k in 0..n {
        if a[k][k] == 0 {
            match (k + 1..n).find(|&r| a[r][k] != 0) {
                Some(r) => {
                    a.swap(k, r);
                    sign = -sign;
                }
                None => return Some(0),
            }
        }
        for i in k + 1..n {
            for j in k + 1..n {
                let v = a[i][j]
                    .checked_mul(a[k][k])?
                    .checked_sub(a[i][k].checked_mul(a[k][j])?)?;
                a[i][j] = v / prev;
            }
        }
        prev = a[k][k];
    }
    a[n - 1][n - 1].checked_mul(sign)
}

/// Gauss-Jordan inverse of a square rational matrix.
pub fn inverse_q(m: &[Vec<Q>]) -> Result<Vec<Vec<Q>>, Unknown> {
    let n = m.len();
    let mut a: Vec<Vec<Q>> = m
        .iter()
        .enumerate()
        .map(|(i, row)| {
            row.iter()
                .copied()
                .chain((0..n).map(|j| if i == j { Q::ONE } else { Q::ZERO }))
                .collect()
        })
        .collect();
    for c in 0..n {
        let p = (c..n)
            .find(|&r| !a[r][c].is_zero())
            .ok_or("singular matrix")?;
        a.swap(c, p);
        let f = a[c][c];
        a[c] = a[c].iter().map(|x| x.div(f)).collect::<Result<_, _>>()?;
        for r in 0..n {
            if r != c && !a[r][c].is_zero() {
                let g = a[r][c];
                let pivot = a[c].clone();
                a[r] = a[r]
                    .iter()
                    .zip(&pivot)
                    .map(|(x, y)| x.sub(g.mul(*y)?))
                    .collect::<Result<_, _>>()?;
            }
        }
    }
    Ok(a.into_iter().map(|row| row[n..].to_vec()).collect())
}

/// Rank of a rational matrix by forward elimination.
pub fn rank_q(rows: &[Vec<Q>]) -> Result<usize, Unknown> {
    let mut a: Vec<Vec<Q>> = rows.to_vec();
    let m = a.first().map_or(0, Vec::len);
    let mut r = 0;
    for c in 0..m {
        let Some(p) = (r..a.len()).find(|&i| !a[i][c].is_zero()) else {
            continue;
        };
        a.swap(r, p);
        for i in r + 1..a.len() {
            if a[i][c].is_zero() {
                continue;
            }
            let f = a[i][c].div(a[r][c])?;
            let pivot = a[r].clone();
            for (x, y) in a[i].iter_mut().zip(&pivot) {
                *x = x.sub(f.mul(*y)?)?;
            }
        }
        r += 1;
    }
    Ok(r)
}

/// Reduced row echelon form on the first `ncols` columns (later columns carried along): (rows, pivot columns).
pub fn rref_q(rows: &[Vec<Q>], ncols: usize) -> Result<(Vec<Vec<Q>>, Vec<usize>), Unknown> {
    let mut rows = rows.to_vec();
    let mut pivots: Vec<usize> = Vec::new();
    let mut r = 0;
    for col in 0..ncols {
        let Some(pr) = (r..rows.len()).find(|&i| !rows[i][col].is_zero()) else {
            continue;
        };
        rows.swap(r, pr);
        let f = rows[r][col];
        rows[r] = rows[r].iter().map(|x| x.div(f)).collect::<Result<_, _>>()?;
        for i in 0..rows.len() {
            if i != r && !rows[i][col].is_zero() {
                let g = rows[i][col];
                let pivot = rows[r].clone();
                rows[i] = rows[i]
                    .iter()
                    .zip(&pivot)
                    .map(|(x, y)| x.sub(g.mul(*y)?))
                    .collect::<Result<_, _>>()?;
            }
        }
        pivots.push(col);
        r += 1;
    }
    Ok((rows, pivots))
}

/// Is `e_col` in the row space of `rows`? (Lee's test of the engine copy: incremental reduction; a row reducing to a
/// multiple of `e_col` answers yes.) Exact for `col` the LAST column (the only use: the column of ones).
pub fn unit_in_row_space(rows: &[Vec<Q>], col: usize) -> Result<bool, Unknown> {
    let mut piv: Vec<(usize, Vec<Q>)> = Vec::new();
    for k in rows {
        let mut r = k.clone();
        for (c, pr) in &piv {
            if !r[*c].is_zero() {
                let f = r[*c];
                r = r
                    .iter()
                    .zip(pr)
                    .map(|(a, b)| a.sub(f.mul(*b)?))
                    .collect::<Result<_, _>>()?;
            }
        }
        let Some(c) = (0..r.len()).find(|&j| !r[j].is_zero()) else {
            continue;
        };
        if c == col {
            return Ok(true);
        }
        let f = r[c];
        r = r.iter().map(|a| a.div(f)).collect::<Result<_, _>>()?;
        piv.push((c, r));
    }
    Ok(false)
}

/// Determinant of the leading `dim x dim` block of a matrix of integer polynomials (memoised Laplace expansion along
/// the first row; i128 coefficients, as the engine copy).
pub fn zpoly_det(m: &[Vec<ZPoly>], dim: usize, t: usize) -> ZPoly {
    fn det(
        mm: &[Vec<ZPoly>],
        rows: &[usize],
        cols: &[usize],
        memo: &mut HashMap<(Vec<usize>, Vec<usize>), ZPoly>,
        t: usize,
    ) -> ZPoly {
        if rows.is_empty() {
            let mut one = ZPoly::new();
            one.insert(vec![0u8; t], 1);
            return one;
        }
        let key = (rows.to_vec(), cols.to_vec());
        if let Some(r) = memo.get(&key) {
            return r.clone();
        }
        let r0 = rows[0];
        let mut out = ZPoly::new();
        for (j, &c) in cols.iter().enumerate() {
            let e = &mm[r0][c];
            if e.is_empty() {
                continue;
            }
            let sub_cols: Vec<usize> = cols
                .iter()
                .enumerate()
                .filter(|(k, _)| *k != j)
                .map(|(_, &x)| x)
                .collect();
            let sub = det(mm, &rows[1..], &sub_cols, memo, t);
            if sub.is_empty() {
                continue;
            }
            let sign: i128 = if j % 2 == 0 { 1 } else { -1 };
            for (k1, v1) in e {
                for (k2, v2) in &sub {
                    let k: Vec<u8> = k1.iter().zip(k2).map(|(a, b)| a + b).collect();
                    let x = out.entry(k.clone()).or_insert(0);
                    *x += sign * v1 * v2;
                    if *x == 0 {
                        out.remove(&k);
                    }
                }
            }
        }
        memo.insert(key, out.clone());
        out
    }
    let idx: Vec<usize> = (0..dim).collect();
    det(m, &idx, &idx, &mut HashMap::new(), t)
}

/// gcd of two i128 (non-negative result; gcd(0, 0) = 0).
pub fn gcd_i128(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}
