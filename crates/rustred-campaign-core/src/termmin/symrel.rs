//! The EXACT SYMMETRY RELATION AT THE KEY (interfaces/termmin.md section 2; requirements 03:02Z / 04:34Z / 09:36Z): the
//! rows of the VERIFIED group at the single seed `{key}` in the expanded mode (multi-term numerator substitutions
//! included, `replay_base::symrows::rows_at_with`), reduced to row echelon form with the columns in the node comparator's
//! order (highest first). If the key is a pivot, its row reads `I(key) + sum r_j I(k_j) = 0` with every `k_j` in a later
//! column, i.e. strictly LOWER: the relation `I(key) = sum (-r_j) I(k_j)` with rational, d-free coefficients. Every
//! rational operation goes through `CheckAlgebra` (L11: no algebra of our own; the certificate backend is the caller's
//! choice, the import's verifier re-derives the relation independently). Nothing node-specific.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use crate::replay_base::algebra::{CheckAlgebra, Q};
use crate::replay_base::symrows::{SymRowMode, VerifiedGroup, rows_at_with};

/// Outcome of the symmetry step at one key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymRelation {
    /// `I(key) = sum c_j I(k_j)`, every `k_j` lower under the comparator; `elements` = the group elements whose rows
    /// exist at the key (indices into the verified group's non-identity elements).
    Relation {
        rhs: Vec<(Vec<i32>, Q)>,
        rows: usize,
        columns: usize,
        elements: Vec<usize>,
    },
    /// The rows at the key do not reach the key as a pivot (`rows` may be 0: no element acts at the key).
    NoRelation {
        rows: usize,
        rank: usize,
        columns: usize,
    },
    /// An expansion over its bound, an overflow of the exact rationals.
    Unknown(String),
}

/// Rational text `n` or `n/d` (the supplied-relation coefficient form).
pub fn q_text(q: &Q) -> String {
    if q.d == 1 {
        q.n.to_string()
    } else {
        format!("{}/{}", q.n, q.d)
    }
}

/// The relation at `key` from the symmetry rows at `key` alone. `cmp(a, b) == Less` iff `a` is LOWER than `b`.
pub fn symmetry_relation_at_key<A: CheckAlgebra>(
    alg: &A,
    group: &VerifiedGroup,
    key: &[i32],
    cmp: &dyn Fn(&[i32], &[i32]) -> Ordering,
    max_terms: usize,
) -> SymRelation {
    let seed = (0u64, key.iter().map(|&x| x as i64).collect::<Vec<i64>>());
    let rows = match rows_at_with(alg, group, &seed, SymRowMode::Expanded, max_terms) {
        Ok(r) => r,
        Err(e) => return SymRelation::Unknown(format!("symmetry rows at the key: {e}")),
    };
    let mut cols: BTreeSet<Vec<i32>> = BTreeSet::new();
    let mut elements = BTreeSet::new();
    let mut sparse: Vec<Vec<(Vec<i32>, Q)>> = vec![];
    for r in &rows {
        let mut v = vec![];
        for ((mask, k), c) in &r.terms {
            if *mask != 0 {
                return SymRelation::Unknown("a symbolic term at an integer seed".into());
            }
            let Ok(k32) = k.iter().map(|&x| i32::try_from(x)).collect::<Result<Vec<i32>, _>>()
            else {
                return SymRelation::Unknown("a row key outside i32".into());
            };
            cols.insert(k32.clone());
            v.push((k32, *c));
        }
        if !v.is_empty() {
            elements.insert(r.element);
            sparse.push(v);
        }
    }
    let mut order: Vec<Vec<i32>> = cols.into_iter().collect();
    // highest first: a pivot's other entries then lie in LOWER columns
    order.sort_by(|a, b| cmp(b, a));
    let ncols = order.len();
    let Some(tcol) = order.iter().position(|k| k.as_slice() == key) else {
        return SymRelation::NoRelation {
            rows: sparse.len(),
            rank: 0,
            columns: ncols,
        };
    };
    let index: std::collections::HashMap<&Vec<i32>, usize> =
        order.iter().enumerate().map(|(i, k)| (k, i)).collect();
    let dense: Vec<Vec<Q>> = sparse
        .iter()
        .map(|v| {
            let mut row = vec![Q::ZERO; ncols];
            for (k, c) in v {
                row[index[k]] = *c;
            }
            row
        })
        .collect();
    let (rref, pivots) = match alg.q_rref(&dense, ncols) {
        Ok(x) => x,
        Err(e) => return SymRelation::Unknown(format!("row echelon form: {e:?}")),
    };
    let Some(pr) = pivots.iter().position(|&p| p == tcol) else {
        return SymRelation::NoRelation {
            rows: sparse.len(),
            rank: pivots.len(),
            columns: ncols,
        };
    };
    let row = &rref[pr];
    let mut rhs = vec![];
    for (j, c) in row.iter().enumerate() {
        if j == tcol || c.is_zero() {
            continue;
        }
        if j < tcol {
            // impossible in a row echelon form (entries left of the pivot are zero); fail closed
            return SymRelation::Unknown("an entry left of the pivot".into());
        }
        match alg.q_neg(*c) {
            Ok(q) => rhs.push((order[j].clone(), q)),
            Err(e) => return SymRelation::Unknown(e),
        }
    }
    SymRelation::Relation {
        rhs,
        rows: sparse.len(),
        columns: ncols,
        elements: elements.into_iter().collect(),
    }
}
