//! Minimal reproductions of the Symbolica defects this lane hit (SYMBOLICA PR CANDIDATES, WS/notes/implementer.rows3.md).
//! Each test asserts that the defect is STILL PRESENT in the vendored tree; when an upstream fix lands, the test
//! fails: then drop the workaround named in the test (crate rustred-campaign-algebra, src/backend.rs) and this test.

mod common;
mod inputs;

use std::panic::{AssertUnwindSafe, catch_unwind};

use common::licensed_or_skip;
use symbolica::domains::integer::{Integer, Z};
use symbolica::poly::polynomial::{MultivariatePolynomial, PolynomialRing};
use symbolica::prelude::Ring;

/// PR candidate 1: `Integer::is_prime` panics (`unreachable!()`, lib/numerica/src/domains/finite_field.rs:3270) for
/// n = u64::MAX: `is_prime_field` selects its witness set with `else if n < u64::MAX` and has no branch for u64::MAX.
/// Workaround: `SymbolicaAlgebra::is_prime` answers u64::MAX itself (composite).
#[test]
fn pr_candidate_is_prime_u64_max_panics() {
    if !licensed_or_skip("pr_candidate_is_prime_u64_max_panics") {
        return;
    }
    let r = catch_unwind(|| Integer::from(u64::MAX).is_prime(20));
    assert!(
        r.is_err(),
        "fixed upstream: Integer::is_prime(u64::MAX) = {:?}",
        r.ok()
    );
}

/// PR candidate 2: a polynomial with NO variables (e.g. `PolynomialRing::zero()` / `one()`) panics in
/// `exponents_iter()` (`self.exponents.chunks(self.nvars())` with nvars = 0: "chunk size must be non-zero",
/// src/poly/polynomial.rs:1222); `Matrix<PolynomialRing<Z>>::det` reaches it. Workaround: `zpoly_det` expands the
/// determinant itself with `MultivariatePolynomial` add / mul on polynomials that carry their variables.
#[test]
fn pr_candidate_exponents_iter_without_variables_panics() {
    if !licensed_or_skip("pr_candidate_exponents_iter_without_variables_panics") {
        return;
    }
    let ring = PolynomialRing::<_, u8>::new(Z);
    let one: MultivariatePolynomial<_, u8> = ring.one();
    let r = catch_unwind(AssertUnwindSafe(|| one.exponents_iter().count()));
    assert!(
        r.is_err(),
        "fixed upstream: exponents_iter() on a variable-less polynomial = {:?}",
        r.ok()
    );
}
