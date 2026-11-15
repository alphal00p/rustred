//! The expanded symmetry rows at one integer seed with ZERO-SECTOR PRUNING DURING THE EXPANSION.
//!
//! Same object as `replay_base::symrows::rows_at_with(.., SymRowMode::Expanded, ..)` followed by dropping the terms the
//! caller's zero-sector predicate accepts, but a partial monomial is dropped as soon as its key is zero-sector: every
//! further factor only LOWERS indices (a line factor `D_m` lowers index `m` by 1), so the set of lines at a positive
//! power can only shrink and the final key lies in a sub-sector of a zero sector, which is zero again. REQUIREMENT on
//! the predicate: it is a DOWNSET in the set of positive lines (true for scaleless sectors, Lee's criterion:
//! `Family::zero_key`; the engine's tables record `zero_scope: "downset"`). Without a predicate the rows are exactly
//! `rows_at_with`'s (tested). Why a copy of the expansion: `rows_at_with` applies its term cap BEFORE any drop, so the
//! rows of deep keys exceed the cap although most terms vanish (REQUEST TO I-INT / rows owner: a prune hook in
//! `rows_at_with`; then this file reduces to that call). Arithmetic only through `CheckAlgebra` (L11).

use std::collections::BTreeMap;

use crate::replay_base::algebra::{CheckAlgebra, Q};
use crate::replay_base::symrows::{SlotImage, VerifiedGroup};

/// The rows at one seed: (element, terms) per element that gives a row.
pub type SeedRowList = Vec<(usize, Vec<(Vec<i32>, Q)>)>;
/// A zero-sector predicate (a downset in the positive lines); `None` = keep every term.
pub type ZeroPredicate<'a> = Option<&'a dyn Fn(&[i32]) -> bool>;

/// Why the rows at a seed are not available.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowsError {
    /// An expansion step above the term cap (after pruning): that seed is over the cap.
    OverCap { element: usize, cap: usize },
    /// Exact-arithmetic failure or a key outside i32.
    Undecided(String),
}

/// The expanded rows at `seed` (one per element that acts, element order), zero-sector terms pruned during the
/// expansion and dropped at the end; rows that cancel completely are left out.
pub fn rows_pruned<A: CheckAlgebra>(
    alg: &A,
    group: &VerifiedGroup,
    seed: &[i32],
    max_terms: usize,
    zero: ZeroPredicate,
) -> Result<SeedRowList, RowsError> {
    let g = group.group();
    let n = g.arity;
    if seed.len() != n {
        return Err(RowsError::Undecided(format!(
            "seed of length {} for arity {n}",
            seed.len()
        )));
    }
    let und = |e: String| RowsError::Undecided(e);
    let to_key = |base: &[i64], dec: &[i64]| -> Result<Vec<i32>, RowsError> {
        base.iter()
            .zip(dec)
            .map(|(b, d)| i32::try_from(b - d).map_err(|_| und("a row key outside i32".into())))
            .collect()
    };
    let is_zero = |k: &[i32]| zero.is_some_and(|z| z(k));
    let mut out = vec![];
    'el: for (ei, e) in g.elements.iter().enumerate() {
        let mut base = vec![0i64; n];
        let mut taken = vec![false; n];
        let mut neg = false;
        let mut expand: Vec<(usize, i64)> = vec![];
        for i in (0..n).filter(|&i| seed[i] != 0) {
            match &e.action[i] {
                SlotImage::Exact { target, flip } => {
                    if taken[*target] {
                        continue 'el;
                    }
                    taken[*target] = true;
                    base[*target] = seed[i] as i64;
                    if *flip && seed[i] % 2 != 0 {
                        neg = !neg;
                    }
                }
                SlotImage::Linear { .. } if seed[i] < 0 => expand.push((i, -(seed[i] as i64))),
                _ => continue 'el,
            }
        }
        let mut terms: BTreeMap<Vec<i64>, Q> = BTreeMap::new();
        if !is_zero(&to_key(&base, &vec![0; n])?) {
            terms.insert(vec![0; n], Q::ONE);
        }
        for (i, times) in expand {
            let SlotImage::Linear {
                terms: lin,
                constant,
            } = &e.action[i]
            else {
                unreachable!("expand holds linear slots only")
            };
            for _ in 0..times {
                let mut next: BTreeMap<Vec<i64>, Q> = BTreeMap::new();
                for (dec, c) in &terms {
                    if !constant.is_zero() {
                        let v = alg.q_mul(*c, *constant).map_err(und)?;
                        let en = next.entry(dec.clone()).or_insert(Q::ZERO);
                        *en = alg.q_add(*en, v).map_err(und)?;
                    }
                    for (m, a) in lin {
                        let mut d2 = dec.clone();
                        d2[*m] += 1;
                        if zero.is_some() && is_zero(&to_key(&base, &d2)?) {
                            continue;
                        }
                        let v = alg.q_mul(*c, *a).map_err(und)?;
                        let en = next.entry(d2).or_insert(Q::ZERO);
                        *en = alg.q_add(*en, v).map_err(und)?;
                    }
                }
                next.retain(|_, c| !c.is_zero());
                if next.len() > max_terms {
                    return Err(RowsError::OverCap {
                        element: ei,
                        cap: max_terms,
                    });
                }
                terms = next;
            }
        }
        let mut row: BTreeMap<Vec<i32>, Q> = BTreeMap::new();
        if !is_zero(seed) {
            row.insert(seed.to_vec(), Q::ONE);
        }
        for (dec, c) in terms {
            let key = to_key(&base, &dec)?;
            if is_zero(&key) {
                continue;
            }
            let v = if neg { c } else { alg.q_neg(c).map_err(und)? };
            let en = row.entry(key).or_insert(Q::ZERO);
            *en = alg.q_add(*en, v).map_err(und)?;
        }
        row.retain(|_, c| !c.is_zero());
        if !row.is_empty() {
            out.push((ei, row.into_iter().collect()));
        }
    }
    Ok(out)
}
