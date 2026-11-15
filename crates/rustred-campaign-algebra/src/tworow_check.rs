//! SECOND EVALUATION ROUTE of a symmetry-row combination (lane tworow, TRICKS 8; interfaces/tworow.md; item 4 of the
//! lane brief). The core's search builds every row with `replay_base::symrows::rows_at_with` (the core's own iterative
//! expansion of `(sum c_m D_m + c_0)^a`) and combines them through `CheckAlgebra`. This route recomputes each row of
//! the combination INDEPENDENTLY from the verified slot images: the image integrand is a product of Symbolica
//! polynomials (`MultivariatePolynomial` over Q, `pow` and `mul`), read back monomial by monomial (a factor `D_m` lowers
//! index `m` by 1), the rows are weighted with Symbolica rationals, and the result is compared TERM BY TERM with the
//! core's relation vector `e_K - sum rhs`. It also re-checks the descent of every term under the caller's comparator.
//!
//! WHY a route of our own: no engine request kind certifies a combination of multi-term symmetry rows (rr_proof
//! instance-span adds one-term symmetry rows only), so this route and the import's span verifier are the two checks;
//! the record says "no engine certificate". Shared with the core: the VERIFIED group data (slot images; the group's
//! exact momentum proof is the identity test's) and the row convention `I(S) - s E_g(S)` (interfaces/tworow.md 1).
//!
//! LICENCE: refuses to run without the Symbolica licence (`crate::licensed`), like every entry point of this crate.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::Arc;

use rustred_campaign_core::replay_base::algebra::Q as RQ;
use rustred_campaign_core::replay_base::symrows::{SlotImage, VerifiedGroup};
use rustred_campaign_core::tworow::Combination;
use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::{Q, Rational, RationalField};
use symbolica::domains::{Ring, RingOps};
use symbolica::poly::PolyVariable;
use symbolica::poly::polynomial::MultivariatePolynomial;

type P = MultivariatePolynomial<RationalField, u16>;

/// The comparison of one combination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecondRoute {
    pub rows: usize,
    /// Terms of the recomputed combination (after the zero drop) = terms of the core's relation vector when equal.
    pub terms_compared: usize,
    pub equal: bool,
    /// The first differing term: key, core coefficient, second-route coefficient (texts).
    pub difference: Option<(Vec<i32>, String, String)>,
    /// A term (other than the key) not strictly lower under the comparator, if any.
    pub not_lower: Option<Vec<i32>>,
}

fn variables(n: usize) -> Result<Arc<Vec<PolyVariable>>, String> {
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let q = format!("rustred::tworow_d{i}");
        let ns = NamespacedSymbol::try_parse(&q).ok_or_else(|| format!("symbol {q}"))?;
        let s = SymbolBuilder::new(ns).build().map_err(|e| e.to_string())?;
        v.push(PolyVariable::Symbol(s));
    }
    Ok(Arc::new(v))
}

fn rat(q: RQ) -> Rational {
    Rational::new(Integer::from(q.n), Integer::from(q.d))
}

/// Row `I(seed) - s E_g(seed)` of `element` by Symbolica polynomial expansion; `Ok(None)` when the element gives no
/// row at the seed (a slot with a nonzero power has no usable image, or two slots land on one target). `zero` drops
/// zero-sector keys (the same predicate as the search). Terms over `max_terms` = `Err`.
pub fn row_by_polynomials(
    group: &VerifiedGroup,
    element: usize,
    seed: &[i32],
    max_terms: usize,
    zero: Option<&dyn Fn(&[i32]) -> bool>,
) -> Result<Option<BTreeMap<Vec<i32>, Rational>>, String> {
    if !crate::licensed() {
        return Err("no Symbolica licence (source WS/ops/env.sh)".into());
    }
    let g = group.group();
    let n = g.arity;
    let e = g
        .elements
        .get(element)
        .ok_or_else(|| format!("no element {element}"))?;
    if seed.len() != n {
        return Err(format!("seed of length {} for arity {n}", seed.len()));
    }
    let vars = variables(n)?;
    let mut base = vec![0i64; n];
    let mut taken = vec![false; n];
    let mut neg = false;
    let mut poly: P = MultivariatePolynomial::new(&Q, Some(1), vars.clone());
    poly.append_monomial(Rational::one(), &vec![0u16; n]);
    for i in 0..n {
        let p = seed[i];
        if p == 0 {
            continue;
        }
        match &e.action[i] {
            SlotImage::Exact { target, flip } => {
                if taken[*target] {
                    return Ok(None);
                }
                taken[*target] = true;
                base[*target] = p as i64;
                if *flip && p % 2 != 0 {
                    neg = !neg;
                }
            }
            SlotImage::Linear { terms, constant } if p < 0 => {
                let mut lin: P =
                    MultivariatePolynomial::new(&Q, Some(terms.len() + 1), vars.clone());
                for (m, c) in terms {
                    let mut ex = vec![0u16; n];
                    ex[*m] = 1;
                    lin.append_monomial(rat(*c), &ex);
                }
                if constant.n != 0 {
                    lin.append_monomial(rat(*constant), &vec![0u16; n]);
                }
                let a = usize::try_from(-p).map_err(|e| e.to_string())?;
                poly = &poly * &lin.pow(a);
                if poly.nterms() > max_terms {
                    return Err(format!(
                        "second route: element {element} over {max_terms} terms at {seed:?}"
                    ));
                }
            }
            _ => return Ok(None),
        }
    }
    let mut row: BTreeMap<Vec<i32>, Rational> = BTreeMap::new();
    let mut add = |k: Vec<i32>, c: Rational| {
        if zero.is_some_and(|z| z(&k)) {
            return;
        }
        let e = row.entry(k).or_insert_with(Rational::zero);
        *e = Q.add(e, &c);
    };
    add(seed.to_vec(), Rational::one());
    for mono in &poly {
        let mut k = Vec::with_capacity(n);
        for (b, &x) in base.iter().zip(mono.exponents) {
            let v = b - x as i64;
            k.push(i32::try_from(v).map_err(|_| "a row key outside i32".to_string())?);
        }
        let c = mono.coefficient.clone();
        add(k, if neg { c } else { Q.neg(&c) });
    }
    row.retain(|_, c| !Q.is_zero(c));
    Ok((!row.is_empty()).then_some(row))
}

/// Recompute `comb` by the second route and compare it with the core's relation term by term.
pub fn second_route(
    group: &VerifiedGroup,
    comb: &Combination,
    cmp: &dyn Fn(&[i32], &[i32]) -> Ordering,
    max_terms: usize,
    zero: Option<&dyn Fn(&[i32]) -> bool>,
) -> Result<SecondRoute, String> {
    if comb.rows.len() != comb.weights.len() {
        return Err("rows and weights differ in number".into());
    }
    let mut acc: BTreeMap<Vec<i32>, Rational> = BTreeMap::new();
    for (r, w) in comb.rows.iter().zip(&comb.weights) {
        let row = row_by_polynomials(group, r.spec.element, &r.seed, max_terms, zero)?.ok_or_else(
            || {
                format!(
                    "second route: no row of g{} at {:?}",
                    r.spec.element, r.seed
                )
            },
        )?;
        let w = rat(*w);
        for (k, c) in row {
            let e = acc.entry(k).or_insert_with(Rational::zero);
            *e = Q.add(e, &Q.mul(&w, &c));
        }
    }
    acc.retain(|_, c| !Q.is_zero(c));
    // the core's relation vector: e_K - sum rhs
    let mut core: BTreeMap<Vec<i32>, Rational> = BTreeMap::new();
    core.insert(comb.key.clone(), Rational::one());
    for (k, c) in &comb.rhs {
        let e = core.entry(k.clone()).or_insert_with(Rational::zero);
        *e = Q.sub(e, &rat(*c));
    }
    core.retain(|_, c| !Q.is_zero(c));
    let mut difference = None;
    let keys: std::collections::BTreeSet<&Vec<i32>> = acc.keys().chain(core.keys()).collect();
    for k in keys {
        let a = core.get(k).cloned().unwrap_or_else(Rational::zero);
        let b = acc.get(k).cloned().unwrap_or_else(Rational::zero);
        if a != b {
            difference = Some((k.clone(), a.to_string(), b.to_string()));
            break;
        }
    }
    let not_lower = acc
        .keys()
        .find(|k| k.as_slice() != comb.key.as_slice() && cmp(k, &comb.key) != Ordering::Less)
        .cloned();
    Ok(SecondRoute {
        rows: comb.rows.len(),
        terms_compared: acc.len(),
        equal: difference.is_none(),
        difference,
        not_lower,
    })
}
