//! INDEPENDENT ALGEBRA (see mod.rs for why this exists). Duplicates numerica's sparse elimination
//! (`SparseRowReducer`, lib/numerica/src/tensors/sparse.rs:1791) and `SparseMatrix::solve` (sparse.rs:1040).
//!
//! COPY (not a move) of pieces of rustred-core/src/node/replay/mod.rs at engine commit b9f06de7 (`Span`, `sparse_subtract`, `Elim`, `ElimPivot`, `span`) for the
//! campaign's independent A2 row check (plan 3.1 (e)); changes: items made `pub` (and the ones marked "campaign copy").

use super::arith;

pub use crate::replay_base::algebra::{MEMORY_GUARD, Span, Unknown};

pub fn sparse_subtract(
    p: u64,
    a: &[(usize, u64)],
    b: &[(usize, u64)],
    f: u64,
) -> Vec<(usize, u64)> {
    let mut out = Vec::with_capacity(a.len() + b.len());
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        if j == b.len() || (i < a.len() && a[i].0 < b[j].0) {
            out.push(a[i]);
            i += 1;
        } else if i == a.len() || b[j].0 < a[i].0 {
            let v = arith::mul(p, f, b[j].1);
            if v != 0 {
                out.push((b[j].0, p - v));
            }
            j += 1;
        } else {
            let v = arith::sub(p, a[i].1, arith::mul(p, f, b[j].1));
            if v != 0 {
                out.push((a[i].0, v));
            }
            i += 1;
            j += 1;
        }
    }
    out
}

/// The row echelon form of the replay rows (insertion order), reusable for several desired vectors
/// (the rule row and its re-solved mutants).
pub struct Elim<'r> {
    rows: &'r [Vec<(usize, u64)>],
    prime: u64,
    ps: Vec<ElimPivot>,
    bycol: Vec<Option<usize>>,
}

struct ElimPivot {
    row: Vec<(usize, u64)>,
    source: usize,
    inverse: u64,
    deps: Vec<(usize, u64)>,
}

impl<'r> Elim<'r> {
    pub fn new(rows: &'r [Vec<(usize, u64)>], ncols: usize, prime: u64) -> Result<Self, Unknown> {
        Self::new_guarded(rows, ncols, prime, usize::MAX)
    }

    /// campaign copy, ADDED: [`Elim::new`] that stops with `Err("memory guard: ...")` once the stored pivot rows and
    /// dependency lists exceed `max_entries` entries (16 bytes each).
    pub fn new_guarded(
        rows: &'r [Vec<(usize, u64)>],
        ncols: usize,
        prime: u64,
        max_entries: usize,
    ) -> Result<Self, Unknown> {
        let mut stored = 0usize;
        let mut ps: Vec<ElimPivot> = Vec::new();
        let mut bycol: Vec<Option<usize>> = vec![None; ncols];
        for (rid, original) in rows.iter().enumerate() {
            let mut r = original.clone();
            let mut deps = Vec::new();
            while let Some(&(c, f)) = r.first() {
                let Some(k) = bycol[c] else { break };
                deps.push((k, f));
                r = sparse_subtract(prime, &r, &ps[k].row, f);
            }
            if let Some(&(c, lead)) = r.first() {
                let iv = arith::inv(prime, lead).ok_or("zero inversion")?;
                for t in r.iter_mut() {
                    t.1 = arith::mul(prime, t.1, iv);
                }
                bycol[c] = Some(ps.len());
                stored = stored.saturating_add(r.len() + deps.len());
                if stored > max_entries {
                    return Err(format!("{MEMORY_GUARD}: {stored} entries > {max_entries}"));
                }
                ps.push(ElimPivot {
                    row: r,
                    source: rid,
                    inverse: iv,
                    deps,
                });
            }
        }
        Ok(Elim {
            rows,
            prime,
            ps,
            bycol,
        })
    }

    pub fn rank(&self) -> usize {
        self.ps.len()
    }

    /// campaign copy, ADDED (rows3): the pivot column of every pivot row, in elimination order.
    pub fn pivot_columns(&self) -> Vec<usize> {
        self.ps.iter().map(|q| q.row[0].0).collect()
    }

    /// campaign copy, ADDED: stored entries (pivot rows + dependency lists).
    pub fn entries(&self) -> usize {
        self.ps.iter().map(|q| q.row.len() + q.deps.len()).sum()
    }

    pub fn solve(&self, desired: &[(usize, u64)]) -> Result<Span, Unknown> {
        let p = self.prime;
        let mut rem = desired.to_vec();
        let mut alpha = vec![0u64; self.ps.len()];
        while let Some(&(c, f)) = rem.first() {
            let Some(k) = self.bycol.get(c).copied().flatten() else {
                break;
            };
            alpha[k] = arith::add(p, alpha[k], f);
            rem = sparse_subtract(p, &rem, &self.ps[k].row, f);
        }
        let mut weights = vec![0u64; self.rows.len()];
        for i in (0..self.ps.len()).rev() {
            let w = arith::mul(p, alpha[i], self.ps[i].inverse);
            weights[self.ps[i].source] = arith::add(p, weights[self.ps[i].source], w);
            for &(j, f) in &self.ps[i].deps {
                if j >= i {
                    return Err("non-DAG dependency".into());
                }
                alpha[j] = arith::sub(p, alpha[j], arith::mul(p, w, f));
            }
        }
        let mut replay = desired.to_vec();
        for (j, w) in weights.iter().enumerate() {
            if *w != 0 {
                replay = sparse_subtract(p, &replay, &self.rows[j], *w);
            }
        }
        if replay != rem {
            return Err(
                "independent original-row product differs from elimination remainder".into(),
            );
        }
        Ok(Span {
            in_span: rem.is_empty(),
            rank: self.ps.len(),
            weights: weights
                .into_iter()
                .enumerate()
                .filter(|(_, w)| *w != 0)
                .collect(),
            remainder: rem,
            pivot_rows: self.ps.iter().map(|q| q.source).collect(),
        })
    }
}

/// campaign copy: the prime is a PARAMETER (the engine's `span` fixes P = 2^61-1).
pub fn span(
    rows: &[Vec<(usize, u64)>],
    desired: &[(usize, u64)],
    ncols: usize,
    prime: u64,
) -> Result<Span, Unknown> {
    Elim::new(rows, ncols, prime)?.solve(desired)
}
