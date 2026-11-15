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

/// What a row can contain, from the element's action alone (no expansion): every term `t` of `E_g(seed)` is
/// `base - dec` with `0 <= dec_m <= cap_m` per slot and `sum dec <= budget` (each unit of numerator power on a linear
/// slot contributes one factor, which lowers by 1 one index of that image's support, or is the constant); the row also
/// contains the seed itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowShape {
    pub base: Vec<i64>,
    pub budget: i64,
    /// Per slot: the most its index can be lowered (the numerator power of the linear images containing that slot).
    pub cap: Vec<i64>,
}

impl RowShape {
    /// NECESSARY condition for `t` to be a term of the row at `seed` (exact; used to skip expansions).
    pub fn may_contain(&self, seed: &[i32], t: &[i32]) -> bool {
        if t == seed {
            return true;
        }
        let mut sum = 0i64;
        for ((b, &x), c) in self.base.iter().zip(t).zip(&self.cap) {
            let d = b - x as i64;
            if d < 0 || d > *c {
                return false;
            }
            sum += d;
        }
        sum <= self.budget
    }
}

/// The [`RowShape`] of element `element` at `seed`, or `None` when the element gives no row there (a slot with a
/// nonzero power without a usable image, or two slots onto one target): the same structural rules as the expansion.
pub fn row_shape(group: &VerifiedGroup, element: usize, seed: &[i32]) -> Option<RowShape> {
    let g = group.group();
    let e = g.elements.get(element)?;
    let n = g.arity;
    if seed.len() != n {
        return None;
    }
    let mut base = vec![0i64; n];
    let mut taken = vec![false; n];
    let mut budget = 0i64;
    let mut cap = vec![0i64; n];
    for i in (0..n).filter(|&i| seed[i] != 0) {
        match &e.action[i] {
            SlotImage::Exact { target, .. } => {
                if std::mem::replace(&mut taken[*target], true) {
                    return None;
                }
                base[*target] = seed[i] as i64;
            }
            SlotImage::Linear { terms, .. } if seed[i] < 0 => {
                let a = -(seed[i] as i64);
                budget += a;
                for (m, _) in terms {
                    cap[*m] += a;
                }
            }
            _ => return None,
        }
    }
    Some(RowShape { base, budget, cap })
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
    rows_pruned_for(alg, group, seed, max_terms, zero, &|_| true)
}

/// [`rows_pruned`] for the elements `want` accepts only (the others are not expanded); any row over the cap = `Err`.
pub fn rows_pruned_for<A: CheckAlgebra>(
    alg: &A,
    group: &VerifiedGroup,
    seed: &[i32],
    max_terms: usize,
    zero: ZeroPredicate,
    want: &dyn Fn(usize) -> bool,
) -> Result<SeedRowList, RowsError> {
    let (rows, over) = rows_partial_for(alg, group, seed, max_terms, zero, want)?;
    match over.first() {
        Some(&element) => Err(RowsError::OverCap {
            element,
            cap: max_terms,
        }),
        None => Ok(rows),
    }
}

/// [`rows_pruned_for`] with the cap PER ELEMENT: the rows that fit, and the elements whose row went over the cap
/// (those rows are unknown, never empty).
pub fn rows_partial_for<A: CheckAlgebra>(
    alg: &A,
    group: &VerifiedGroup,
    seed: &[i32],
    max_terms: usize,
    zero: ZeroPredicate,
    want: &dyn Fn(usize) -> bool,
) -> Result<(SeedRowList, Vec<usize>), RowsError> {
    let mut over = vec![];
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
        if !want(ei) {
            continue;
        }
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
                    over.push(ei);
                    continue 'el;
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
    Ok((out, over))
}
