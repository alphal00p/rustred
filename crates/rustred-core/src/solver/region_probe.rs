//! Region-aware modular discovery point (t266-regions).
//!
//! A deferred region of a node is `{chart} & {f_1 = .. = f_k = 0}`, where the `f_j` are the d-coefficients of an
//! irreducible factor of some rule's pivot denominator (`exception.rs`): on the region that rule's denominator
//! vanishes identically in `d`. A chart-owning run with the usual random discovery point finds the same rule again
//! (the point lies off the region, where that rule is fine). Placing the modular discovery point ON the region
//! changes this: the first-hit search then only accepts a pivot that is nonzero at a point of the region, so the
//! exact rule it lifts has no exceptional branch containing that point.
//!
//! Exactness: the point only steers WHICH relation the search selects; the exact lift and every exact gate are
//! unchanged. Whether the new rule's branches still contain the region is re-checked by the caller from the
//! branches themselves ([`vanishes_at`]: a branch with a polynomial nonzero at the region point cannot contain the
//! region's component through that point, for every prime not dividing the leading data).
//!
//! The point is a random Zp point of `V(system)` found by alternating exact Zp Groebner bases (Symbolica, GREVLEX
//! only) and random specialisation: a variable with a univariate basis element is set to a random Zp root of it
//! (native finite-field factorisation); on a zero-dimensional ideal without one, the minimal polynomial of a variable
//! in the quotient ring is computed by linear algebra mod p (Krylov sequence of grevlex normal forms 1, x, x^2, ..);
//! otherwise a variable of the basis gets a random value. Variables not occurring in the system (the dimension `d`,
//! unconstrained indices) keep random values. `Ok(None)` = no point found within the attempt budget (empty over
//! Zp-bar, or only points over extensions): the caller keeps its ordinary point. `Err` = a typed refusal (quotient
//! ring larger than [`REGION_QUOTIENT_CAP`]): the caller refuses the case (an open obligation with the reason).
//!
//! gb-fix (2026-11-14): no LEX Groebner basis is computed any more. Symbolica's F4 in lex order on the T266 region
//! systems (R56.3.2: 11 polynomials, 5 variables, degree <= 5, zero-dimensional, lex basis of degree 16 by FGLM)
//! ran to 40-170 GB and panicked with a u16 exponent overflow (handoff/symbolica-gb-overflow).

use rand::{Rng, SeedableRng, rngs::StdRng};
use std::sync::RwLock;
use symbolica::domains::RingOps;
use symbolica::domains::finite_field::{FiniteFieldCore, FiniteFieldElement, ToFiniteField, Zp64};
use symbolica::poly::groebner::GroebnerBasis;
use symbolica::poly::polynomial::MultivariatePolynomial;
use symbolica::poly::{GrevLexOrder, LexOrder};
use symbolica::prelude::{Factorize, Field, Ring};

use crate::algebra::CoefficientPolynomial;

type ZpPoly = MultivariatePolynomial<Zp64, u16>;

/// Attempts (independent random restarts) before giving up.
pub const REGION_POINT_ATTEMPTS: usize = 64;

/// Largest dimension of the quotient ring `Zp[x]/I` (number of standard monomials of a zero-dimensional ideal) for
/// which the probe computes a minimal polynomial; beyond it the region is refused (typed, never a panic). The Krylov
/// step costs O(D^2) field operations per power and stores D vectors of length <= D (D = 4096: 16M elements, 128 MB).
pub const REGION_QUOTIENT_CAP: usize = 4096;

/// A region the probe refuses to place a point on (an open obligation for the caller, with this reason).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionRefusal(pub String);

impl std::fmt::Display for RegionRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "region point not placed: {}", self.0)
    }
}

type GrevPoly = MultivariatePolynomial<Zp64, u16, GrevLexOrder>;

/// The minimal polynomial of variable `var` in `Zp[x]/I`, for a GREVLEX Groebner basis `basis` of a zero-dimensional
/// ideal `I` (no constant in it): the first linear dependency of the normal forms NF(1), NF(x), NF(x^2), .. found by
/// incremental Gaussian elimination mod p. Its roots are the `var`-coordinates of the points of V(I) over Zp-bar.
/// `Err` when no dependency appears within `cap` powers (the quotient is larger than the cap; the probe passes
/// [`REGION_QUOTIENT_CAP`]).
fn minimal_polynomial(
    basis: &[GrevPoly],
    var: usize,
    field: &Zp64,
    cap: usize,
) -> Result<ZpPoly, RegionRefusal> {
    use std::collections::HashMap;
    let template = &basis[0];
    let nvars = template.nvars();
    let mut e = vec![0u16; nvars];
    e[var] = 1;
    let mut cur = template.one().reduce(basis);
    let mut columns: HashMap<Vec<u16>, usize> = HashMap::new();
    // echelon rows: (pivot column, row over the columns, combination over the powers x^0..x^k)
    #[allow(clippy::type_complexity)]
    let mut rows: Vec<(
        usize,
        Vec<FiniteFieldElement<u64>>,
        Vec<FiniteFieldElement<u64>>,
    )> = Vec::new();
    for k in 0..=cap {
        let mut v = vec![field.zero(); columns.len()];
        for i in 0..cur.nterms() {
            let next = columns.len();
            let c = *columns.entry(cur.exponents(i).to_vec()).or_insert(next);
            if c >= v.len() {
                v.resize(c + 1, field.zero());
            }
            v[c] = cur.coefficients[i];
        }
        if columns.len() > cap {
            return Err(RegionRefusal(format!(
                "quotient ring of dimension > {cap} (standard monomials met in the normal forms of \
                 the powers of variable {var})"
            )));
        }
        let mut comb = vec![field.zero(); k + 1];
        comb[k] = field.one();
        for (pivot, row, rc) in &rows {
            if *pivot >= v.len() || field.is_zero(&v[*pivot]) {
                continue;
            }
            let f = v[*pivot];
            for (j, r) in row.iter().enumerate() {
                if !field.is_zero(r) {
                    if j >= v.len() {
                        v.resize(j + 1, field.zero());
                    }
                    v[j] = field.sub(&v[j], &field.mul(&f, r));
                }
            }
            for (j, r) in rc.iter().enumerate() {
                comb[j] = field.sub(&comb[j], &field.mul(&f, r));
            }
        }
        match v.iter().position(|c| !field.is_zero(c)) {
            None => {
                // sum_j comb_j x^j lies in I: the minimal polynomial of x (monic in x^k)
                let mut m = to_lex_zero(template);
                for (j, c) in comb.iter().enumerate() {
                    if !field.is_zero(c) {
                        let mut e = vec![0u16; nvars];
                        e[var] = j as u16;
                        m.append_monomial(*c, &e);
                    }
                }
                return Ok(m);
            }
            Some(pivot) => {
                let inv = field.inv(&v[pivot]);
                for c in v.iter_mut() {
                    *c = field.mul(&*c, &inv);
                }
                for c in comb.iter_mut() {
                    *c = field.mul(&*c, &inv);
                }
                rows.push((pivot, v, comb));
            }
        }
        // multiplying every term by the same monomial keeps the (multiplicative) monomial order
        cur = cur.mul_exp(&e).reduce(basis);
    }
    Err(RegionRefusal(format!(
        "no minimal polynomial of variable {var} within {cap} powers (quotient ring larger than the cap)"
    )))
}

fn to_lex_zero(p: &GrevPoly) -> ZpPoly {
    p.zero().reorder::<LexOrder>()
}

fn to_zp(p: &CoefficientPolynomial, field: &Zp64) -> ZpPoly {
    p.map_coeff(|c| c.to_finite_field(field), field.clone())
}

fn occurring(p: &ZpPoly) -> Vec<usize> {
    (0..p.nvars())
        .filter(|&v| (0..p.nterms()).any(|i| p.exponents(i)[v] != 0))
        .collect()
}

/// Zp roots of a univariate (in `var`) polynomial, by native factorisation (linear factors only).
fn roots(p: &ZpPoly, var: usize, field: &Zp64) -> Vec<FiniteFieldElement<u64>> {
    let mut out = Vec::new();
    for (f, _) in p.factor() {
        if f.degree(var) != 1 {
            continue;
        }
        // f = a*x + b
        let mut a = field.zero();
        let mut b = field.zero();
        for i in 0..f.nterms() {
            if f.exponents(i)[var] == 1 {
                a = f.coefficients[i];
            } else {
                b = f.coefficients[i];
            }
        }
        if field.is_zero(&a) {
            continue;
        }
        out.push(field.neg(&field.div(&b, &a)));
    }
    out
}

fn substitute(system: &mut Vec<ZpPoly>, var: usize, value: &FiniteFieldElement<u64>) {
    for p in system.iter_mut() {
        *p = p.replace(var, value);
    }
    system.retain(|p| !p.is_zero());
}

/// One attempt: returns the values of the variables that the system constrains (others untouched).
fn attempt(
    mut system: Vec<ZpPoly>,
    field: &Zp64,
    rng: &mut StdRng,
    point: &mut [FiniteFieldElement<u64>],
) -> Result<bool, RegionRefusal> {
    let random = |rng: &mut StdRng| {
        symbolica::prelude::Integer::from(rng.random_range(1..field.get_prime()))
            .to_finite_field(field)
    };
    system.retain(|p| !p.is_zero());
    // every step fixes one more variable; the loop is bounded by the variable count
    for _ in 0..=point.len() {
        if system.is_empty() {
            return Ok(true);
        }
        if system.iter().any(|p| p.is_constant()) {
            return Ok(false);
        }
        // t266-4: the basis in GREVLEX order. `ZpPoly` carries the default lex order, and a lex basis of a
        // positive-dimensional region (R05.0.0 / R77.0 of T266: quartics with 14-digit coefficients) exhausted a 10 GB
        // cap within a minute; any basis of the same ideal serves here (univariate elements, constants, the variables
        // still occurring), so the cheaper order is taken and converted back.
        let grevlex: Vec<MultivariatePolynomial<Zp64, u16, GrevLexOrder>> =
            system.iter().map(|p| p.reorder::<GrevLexOrder>()).collect();
        let gbasis: Vec<_> = GroebnerBasis::new(&grevlex, false)
            .system
            .into_iter()
            .filter(|p| !p.is_zero())
            .collect();
        // zero-dimensional (every occurring variable has a pure power among the leading monomials): no free choice is
        // left, so a univariate element is needed: the minimal polynomial of an occurring variable in the quotient
        // ring, by linear algebra on grevlex normal forms (gb-fix: never a lex basis, see the module docs)
        let occ: Vec<usize> = {
            let mut v: Vec<usize> = gbasis
                .iter()
                .flat_map(|p| {
                    (0..p.nvars())
                        .filter(|&x| (0..p.nterms()).any(|i| p.exponents(i)[x] != 0))
                        .collect::<Vec<_>>()
                })
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        let zero_dim = !occ.is_empty()
            && occ.iter().all(|&x| {
                gbasis.iter().any(|p| {
                    let lm = p.last_exponents();
                    lm[x] != 0 && lm.iter().enumerate().all(|(y, e)| y == x || *e == 0)
                })
            });
        let has_uni = gbasis.iter().any(|p| {
            (0..p.nvars())
                .filter(|&x| (0..p.nterms()).any(|i| p.exponents(i)[x] != 0))
                .count()
                == 1
        });
        if gbasis.iter().any(|p| p.is_constant()) {
            return Ok(false);
        }
        let eliminant: Option<(ZpPoly, usize)> = if zero_dim && !has_uni {
            let var = occ[rng.random_range(0..occ.len())];
            Some((
                minimal_polynomial(&gbasis, var, field, REGION_QUOTIENT_CAP)?,
                var,
            ))
        } else {
            None
        };
        let basis: Vec<ZpPoly> = gbasis
            .into_iter()
            .map(|p| p.reorder::<LexOrder>())
            .collect();
        if basis.is_empty() {
            return Ok(true);
        }
        // a univariate element: a root of it
        let uni = eliminant.or_else(|| {
            basis.iter().find_map(|p| {
                let vars = occurring(p);
                (vars.len() == 1).then(|| (p.clone(), vars[0]))
            })
        });
        let (var, value) = if let Some((p, var)) = uni {
            let rs = roots(&p, var, field);
            if rs.is_empty() {
                return Ok(false);
            }
            let k = rng.random_range(0..rs.len());
            (var, rs[k])
        } else {
            let mut vars: Vec<usize> = basis.iter().flat_map(occurring).collect();
            vars.sort_unstable();
            vars.dedup();
            let var = vars[rng.random_range(0..vars.len())];
            (var, random(rng))
        };
        point[var] = value;
        system = basis;
        substitute(&mut system, var, &value);
    }
    Ok(system.is_empty())
}

/// A random Zp point of `V(system)` (all polynomials in one variable map of `nvars` variables). Variables the
/// system does not fix keep the values of `base` (the caller's random point). `Ok(None)`: no point within the
/// attempt budget; `Err`: the region is refused (see [`RegionRefusal`]).
pub fn region_point(
    system: &[CoefficientPolynomial],
    base: &[FiniteFieldElement<u64>],
    field: &Zp64,
    seed: u64,
) -> Result<Option<Vec<FiniteFieldElement<u64>>>, RegionRefusal> {
    let zp: Vec<ZpPoly> = system.iter().map(|p| to_zp(p, field)).collect();
    let mut rng = StdRng::seed_from_u64(seed ^ 0x7266_5e61_6f6e_5072);
    for _ in 0..REGION_POINT_ATTEMPTS {
        let mut point = base.to_vec();
        if attempt(zp.clone(), field, &mut rng, &mut point)?
            && zp.iter().all(|p| field.is_zero(&eval(p, &point, field)))
        {
            return Ok(Some(point));
        }
    }
    Ok(None)
}

fn eval(p: &ZpPoly, point: &[FiniteFieldElement<u64>], field: &Zp64) -> FiniteFieldElement<u64> {
    p.evaluate_with_coeff_map(|c| *c, point, field)
}

/// Does every polynomial of `branch` vanish at the Zp `point`? (`false` = the branch cannot contain an
/// irreducible component of the region through this point, for this prime.)
pub fn vanishes_at(
    branch: &[CoefficientPolynomial],
    point: &[FiniteFieldElement<u64>],
    field: &Zp64,
) -> bool {
    branch
        .iter()
        .all(|p| field.is_zero(&eval(&to_zp(p, field), point, field)))
}

// ---------------------------------------------------------------------------------------------------------------
// run-wide region (set by the node driver for a region-restricted run)

/// The region of a region-restricted run: conjunction polynomials (index variables of the source system's
/// coefficient map), plus the chart constraints added per case by the search (fixed slots, affine equations).
static REGION: RwLock<Option<Vec<CoefficientPolynomial>>> = RwLock::new(None);
/// The last region point used by a discovery probe (for the driver's per-rule check).
static LAST_POINT: RwLock<Option<(u64, Vec<u64>)>> = RwLock::new(None);

/// Install (or clear) the run-wide region.
pub fn set_region(conjunction: Option<Vec<CoefficientPolynomial>>) {
    *REGION.write().unwrap() = conjunction;
    *LAST_POINT.write().unwrap() = None;
}

pub fn region() -> Option<Vec<CoefficientPolynomial>> {
    REGION.read().unwrap().clone()
}

pub(super) fn record_point(prime: u64, point: &[FiniteFieldElement<u64>], field: &Zp64) {
    *LAST_POINT.write().unwrap() =
        Some((prime, point.iter().map(|x| field.from_element(x)).collect()));
}

/// Drop the recorded point (before a case's search; a search that does not reach the probe leaves none).
pub fn clear_last_point() {
    *LAST_POINT.write().unwrap() = None;
}

/// The last recorded region point (prime, residues), if the last discovery probe was placed on the region.
pub fn last_point() -> Option<(u64, Vec<u64>)> {
    LAST_POINT.read().unwrap().clone()
}

/// [`vanishes_at`] at the recorded point.
pub fn vanishes_at_last(branch: &[CoefficientPolynomial]) -> Option<bool> {
    let (prime, residues) = last_point()?;
    let field = Zp64::new(prime);
    let point: Vec<_> = residues.iter().map(|&r| field.to_element(r)).collect();
    Some(vanishes_at(branch, &point, &field))
}

#[cfg(test)]
#[path = "region_probe/tests.rs"]
mod tests;
