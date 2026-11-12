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
//! The point is a random Zp point of `V(system)` found by alternating exact Zp Groebner bases (Symbolica) and
//! random specialisation: a variable with a univariate basis element is set to a random Zp root of it (native
//! finite-field factorisation), otherwise a variable of the basis gets a random value. Variables not occurring in
//! the system (the dimension `d`, unconstrained indices) keep random values. `None` = no point found within the
//! attempt budget (empty over Zp-bar, or only points over extensions): the caller keeps its ordinary point.

use rand::{Rng, SeedableRng, rngs::StdRng};
use symbolica::domains::finite_field::{FiniteFieldCore, FiniteFieldElement, ToFiniteField, Zp64};
use symbolica::poly::groebner::GroebnerBasis;
use symbolica::poly::polynomial::MultivariatePolynomial;
use symbolica::domains::RingOps;
use symbolica::prelude::{Factorize, Field, Ring};
use std::sync::RwLock;

use crate::algebra::CoefficientPolynomial;

type ZpPoly = MultivariatePolynomial<Zp64, u16>;

/// Attempts (independent random restarts) before giving up.
pub const REGION_POINT_ATTEMPTS: usize = 64;

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
) -> bool {
    let random = |rng: &mut StdRng| {
        symbolica::prelude::Integer::from(rng.random_range(1..field.get_prime()))
            .to_finite_field(field)
    };
    system.retain(|p| !p.is_zero());
    // every step fixes one more variable; the loop is bounded by the variable count
    for _ in 0..=point.len() {
        if system.is_empty() {
            return true;
        }
        if system.iter().any(|p| p.is_constant()) {
            return false;
        }
        let basis = GroebnerBasis::new(&system, false).system;
        let basis: Vec<ZpPoly> = basis.into_iter().filter(|p| !p.is_zero()).collect();
        if basis.is_empty() {
            return true;
        }
        if basis.iter().any(|p| p.is_constant()) {
            return false;
        }
        // a univariate element: a root of it
        let uni = basis.iter().find_map(|p| {
            let vars = occurring(p);
            (vars.len() == 1).then(|| (p.clone(), vars[0]))
        });
        let (var, value) = if let Some((p, var)) = uni {
            let rs = roots(&p, var, field);
            if rs.is_empty() {
                return false;
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
    system.is_empty()
}

/// A random Zp point of `V(system)` (all polynomials in one variable map of `nvars` variables). Variables the
/// system does not fix keep the values of `base` (the caller's random point).
pub fn region_point(
    system: &[CoefficientPolynomial],
    base: &[FiniteFieldElement<u64>],
    field: &Zp64,
    seed: u64,
) -> Option<Vec<FiniteFieldElement<u64>>> {
    let zp: Vec<ZpPoly> = system.iter().map(|p| to_zp(p, field)).collect();
    let mut rng = StdRng::seed_from_u64(seed ^ 0x7266_5e61_6f6e_5072);
    for _ in 0..REGION_POINT_ATTEMPTS {
        let mut point = base.to_vec();
        if attempt(zp.clone(), field, &mut rng, &mut point)
            && zp.iter().all(|p| field.is_zero(&eval(p, &point, field)))
        {
            return Some(point);
        }
    }
    None
}

fn eval(
    p: &ZpPoly,
    point: &[FiniteFieldElement<u64>],
    field: &Zp64,
) -> FiniteFieldElement<u64> {
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
    *LAST_POINT.write().unwrap() = Some((prime, point.iter().map(|x| field.from_element(x)).collect()));
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
