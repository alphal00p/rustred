//! ROUTE 0b (interfaces/repairflow.md 14): the EXACT multi-term symmetry relation at a key, PER GROUP ELEMENT, keeping
//! the SMALLEST lower relation (directing session 2026-11-14: "the element matters": another element serves the same
//! keys with 229 to 116,428 terms). The rows are the verified group's expanded symmetry rows at the key
//! (`replay_base::symrows::rows_at_with`, the import's derivation); for each element alone, an exact row echelon form
//! with the columns in comparator order (highest first) gives `I(key) = sum c_j I(k_j)` with every `k_j` lower when the
//! key is a pivot. No algebra of our own: the elimination is `CheckAlgebra::q_rref` (as termmin's symmetry step).

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::replay_base::algebra::{CheckAlgebra, Q};
use crate::replay_base::symrows::{SymRowMode, VerifiedGroup, rows_at_with};

/// One element's relation at the key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementRelation {
    /// Index into the verified group's non-identity elements.
    pub element: usize,
    pub rhs: Vec<(Vec<i32>, Q)>,
}

/// The per-element outcome at a key: the smallest relation (fewest terms; ties: the comparator on the highest RHS
/// key, lower first; then the element index) and every element's size (None = no relation from that element alone).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmallestSym {
    pub best: Option<ElementRelation>,
    pub sizes: BTreeMap<usize, Option<usize>>,
}

fn relation_from_rows<A: CheckAlgebra>(
    alg: &A,
    rows: &[Vec<(Vec<i32>, Q)>],
    key: &[i32],
    cmp: &dyn Fn(&[i32], &[i32]) -> Ordering,
) -> Result<Option<Vec<(Vec<i32>, Q)>>, String> {
    let mut cols: BTreeSet<Vec<i32>> = BTreeSet::new();
    for r in rows {
        for (k, _) in r {
            cols.insert(k.clone());
        }
    }
    let mut order: Vec<Vec<i32>> = cols.into_iter().collect();
    order.sort_by(|a, b| cmp(b, a));
    let Some(tcol) = order.iter().position(|k| k.as_slice() == key) else {
        return Ok(None);
    };
    let index: std::collections::HashMap<&Vec<i32>, usize> =
        order.iter().enumerate().map(|(i, k)| (k, i)).collect();
    let n = order.len();
    let dense: Vec<Vec<Q>> = rows
        .iter()
        .map(|v| {
            let mut row = vec![Q::ZERO; n];
            for (k, c) in v {
                row[index[k]] = *c;
            }
            row
        })
        .collect();
    let (rref, pivots) = alg.q_rref(&dense, n).map_err(|e| format!("{e:?}"))?;
    let Some(pr) = pivots.iter().position(|&p| p == tcol) else {
        return Ok(None);
    };
    let mut rhs = vec![];
    for (j, c) in rref[pr].iter().enumerate() {
        if j == tcol || c.is_zero() {
            continue;
        }
        if j < tcol {
            return Err("an entry left of the pivot".into());
        }
        rhs.push((order[j].clone(), alg.q_neg(*c)?));
    }
    Ok(Some(rhs))
}

/// Route 0b at one key: every element alone, the smallest lower relation kept.
pub fn smallest_symmetry_relation<A: CheckAlgebra>(
    alg: &A,
    group: &VerifiedGroup,
    key: &[i32],
    cmp: &dyn Fn(&[i32], &[i32]) -> Ordering,
    max_terms: usize,
) -> Result<SmallestSym, String> {
    let seed = (0u64, key.iter().map(|&x| x as i64).collect::<Vec<i64>>());
    let rows = rows_at_with(alg, group, &seed, SymRowMode::Expanded, max_terms)
        .map_err(|e| format!("symmetry rows at the key: {e}"))?;
    let mut by: BTreeMap<usize, Vec<Vec<(Vec<i32>, Q)>>> = BTreeMap::new();
    for r in &rows {
        let mut v = vec![];
        for ((mask, k), c) in &r.terms {
            if *mask != 0 {
                return Err("a symbolic term at an integer seed".into());
            }
            let k32: Vec<i32> = k
                .iter()
                .map(|&x| i32::try_from(x).map_err(|_| "a row key outside i32".to_string()))
                .collect::<Result<_, _>>()?;
            v.push((k32, *c));
        }
        if !v.is_empty() {
            by.entry(r.element).or_default().push(v);
        }
    }
    let mut sizes = BTreeMap::new();
    let mut best: Option<ElementRelation> = None;
    for (e, rs) in by {
        let rel = relation_from_rows(alg, &rs, key, cmp)?;
        sizes.insert(e, rel.as_ref().map(|r| r.len()));
        if let Some(rhs) = rel {
            let top =
                |r: &[(Vec<i32>, Q)]| r.iter().map(|(k, _)| k.clone()).max_by(|a, b| cmp(a, b));
            let better = match &best {
                None => true,
                Some(b) => {
                    rhs.len() < b.rhs.len()
                        || (rhs.len() == b.rhs.len()
                            && match (top(&rhs), top(&b.rhs)) {
                                (Some(x), Some(y)) => cmp(&x, &y) == Ordering::Less,
                                _ => false,
                            })
                }
            };
            if better {
                best = Some(ElementRelation { element: e, rhs });
            }
        }
    }
    Ok(SmallestSym { best, sizes })
}
