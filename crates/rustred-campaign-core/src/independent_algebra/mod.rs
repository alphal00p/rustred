//! # INDEPENDENT ALGEBRA of the identity test: a DELIBERATE, SEGREGATED duplicate of Symbolica features
//!
//! READ THIS BEFORE TOUCHING ANYTHING IN THIS DIRECTORY.
//!
//! The house rule of RustRed is "use Symbolica" (docs/algebra.md: one computer-algebra authority; lane rule L11: own
//! algebra only when verified missing). This module BREAKS that rule ON PURPOSE, for one consumer only: the
//! INDEPENDENT IDENTITY TEST of the automatic IBP campaign (`crate::ibp_rows`, `crate::replay_base`), which grants
//! the `replay` evidence level, the certificate of record for complete nodes. Every routine here has a Symbolica /
//! numerica counterpart (except the one-pass multi-prime streaming evaluation of `coeff`, see its row); the table says
//! where (vendored tree `vendor/symbolica`, `num/` = `lib/numerica/src`, `sym/` = `src`; verified by the study
//! plan/auto-campaign/interfaces/symbolica-usage.md, section 2):
//!
//! | routine here | what it does | Symbolica / numerica counterpart |
//! |---|---|---|
//! | [`arith`] `add sub mul pow inv neg from_i128` | the prime field on u64 residues (u128 products) | `num/domains/finite_field.rs:41` `Zp64 = FiniteField<u64>` (Montgomery; `Zp64::new(p)` for odd p < 2^64) |
//! | [`arith`] `QArith` on `Q` | checked i128 rationals | `num/domains/rational.rs` `Rational`, field `Q` |
//! | [`expr`] `Expr` (engine copy), `Literal` | parse + evaluate the engine expression language mod p, affine forms, exact case literals | `sym/parser.rs` (`Atom::parse`), `Atom::evaluate`, `to_rational_polynomial` (`sym/poly.rs:2773`) |
//! | [`coeff`] `eval_ratio` | one-pass streaming evaluation of "(P)/(Q)" at several (prime, point) contexts | `sym/parser.rs:1909` `Token::parse_polynomial` + `sym/poly/polynomial.rs:2966` `replace_all` per prime (no one-pass multi-prime streaming evaluator exists: study 2.3) |
//! | [`elim`] `Elim`, `sparse_subtract` (engine copy) | sparse Gaussian elimination mod p with a re-verified certificate | `num/tensors/sparse.rs:1791` `SparseRowReducer` (`add_row`, `pivots` 1777, `back_substitute` 1884), `SparseMatrix::solve` (1040) |
//! | [`primes`] `is_prime` | deterministic Miller-Rabin for u64 | `num/domains/integer.rs:1751-1771` `Integer::is_prime`; `PrimeIteratorU64` (`finite_field.rs:3444`) |
//! | [`linalg`] `det_i128 inverse_q rank_q rref_q` | exact small linear algebra (symmetry proofs, case equations, quadratic basis) | `num/tensors/matrix.rs` `Matrix::det` (1256), `inv` (1731), `rank` (2023), `row_reduce` (2016) |
//! | [`linalg`] `unit_in_row_space` | Lee's scaleless-sector test (affine hull of the U+F support) | `Matrix<Q>::rank` of the support with / without the ones column |
//! | [`linalg`] `gcd_i128 int_mat_mul int_div_exact` | integer gcd, products, exact division (locus proof, symmetry maps) | `Integer::gcd` (`num/domains/integer.rs:1905`), `Matrix<Z>` product, `Integer::quot_rem` |
//! | [`linalg`] `zpoly_det` | the graph polynomials U, F as determinants of polynomial matrices | `sym/poly/polynomial.rs` `MultivariatePolynomial<Z>` arithmetic |
//!
//! ## WHY it exists nevertheless
//!
//! The identity test CERTIFIES rules that the engine GENERATED with Symbolica (`Zp64`, `SparseRowReducer`, rational
//! function reconstruction). A checker built on the same algebra would share its bugs: a Symbolica defect (docs/algebra.md
//! lists real ones of the previous pin: singular `inv` accepted, inverted inconsistent-row detection, lost determinant
//! parity; the study found four parser defects, see `rustred-campaign-algebra`) would then confirm a wrong rule.
//! The engine made exactly this choice for its own replay: crates/rustred-core/src/node/replay/mod.rs lines 15-17
//! ("Independence: ... all arithmetic is own u64/i128 code; no RustRed solver code and no Symbolica arithmetic is
//! used"); [`arith`], [`expr`], [`elim`] are attributed copies of that replay (provenance in each file header).
//!
//! ## WHAT the user decided (DEV-17 REVISED, plan/auto-campaign/DEVIATIONS.md)
//!
//! 2026-11-14 (02:53Z), after the line count: "Ok, if it's only 2'200 lines then I'm ok, but really SEGREGATE that
//! 'repeat' of symbolica feature into a separate rust module / rust file, whose REASON FOR EXISTENCE is made
//! EXCEEDINGLY CLEAR. And add an ALTERNATIVE MODE for the checker where SYMBOLICA is used instead, thus allowing for
//! immediately DIAGNOSING if the issue is in symbolica or your own code or if the rule checked is indeed actually
//! problematic."
//!
//! So: ALL algebra of the identity test lives HERE and nowhere else in `ibp_rows` / `replay_base`; the domain logic
//! reaches it only through the trait `crate::replay_base::algebra::CheckAlgebra` ([`Independent`] below implements
//! it); the crate `rustred-campaign-algebra` implements the SAME trait on Symbolica / numerica.
//!
//! ## RULES for touching this module
//!
//! 1. NO NEW FEATURE here without first checking that Symbolica / numerica cannot do it in the identity test's
//!    Symbolica mode; record the check in the lane notes (L11). This module only mirrors what the independent check
//!    needs; it is not a general algebra library and nothing outside the identity test may use it.
//! 2. KEEP IT SMALL: the budget is [`LINE_BUDGET`] non-test lines for the whole directory, asserted by the test
//!    `independent_algebra_within_line_budget` below (it counts every line of every file here before its
//!    test module). Raising the budget needs the user.
//! 3. EVERY ROUTINE HAS A DIFFERENTIAL TEST against the Symbolica-backed implementation (crate
//!    `rustred-campaign-algebra`, `tests/differential.rs`); a new routine comes with its test there.
//! 4. Only the concrete certificate entry points (which run [`Independent`] themselves) grant the `replay` level (rows4:
//!    `AlgebraOrigin`, sealed; a generic run on any backend is diagnostic).
//!
//! ## HOW to run the checker in the alternative modes
//!
//! `rustred_campaign_algebra::{identity_test_mode, replay_evidence_mode}` with `IdentityMode::Independent` (default,
//! the certificate of record), `IdentityMode::Symbolica` (the same domain logic on Symbolica / numerica; its records
//! are DIAGNOSTIC and never carry the level) or `IdentityMode::Both` (both backends on the same rule, family, served
//! keys, primes and random stream; a TRIAGE report says `AgreePass`, `AgreeFail` (not derived from its recorded seeds),
//! `RecordInconsistent`, `SharedLogicDiffers` (the shared domain logic disagrees with the oracle) or `Differ` with the
//! first differing intermediate quantity and the outlier named by a third implementation, the referee).
//! Tests of that crate need the Symbolica licence: `source WS/ops/env.sh` first (interfaces/ibp-rows.md section 11).

pub mod arith;
pub mod coeff;
pub mod elim;
pub mod expr;
pub mod linalg;
pub mod primes;

use crate::replay_base::algebra::{
    CheckAlgebra, CoeffError, CoeffStats, Ctx, Eliminator, Q, QResult, Span, Unknown, ZPoly,
};
use arith::QArith;

/// Non-test line budget of this directory (rule 2 above; the user accepted "2'200 lines").
pub const LINE_BUDGET: usize = 2_200;

/// THE CERTIFICATE BACKEND: the identity test's own, Symbolica-free algebra (this module).
#[derive(Clone, Copy, Debug, Default)]
pub struct Independent;

impl Eliminator for elim::Elim<'_> {
    fn rank(&self) -> usize {
        elim::Elim::rank(self)
    }
    fn entries(&self) -> usize {
        elim::Elim::entries(self)
    }
    fn pivot_columns(&self) -> Vec<usize> {
        elim::Elim::pivot_columns(self)
    }
    fn solve(&self, desired: &[(usize, u64)]) -> Result<Span, Unknown> {
        elim::Elim::solve(self, desired)
    }
}

impl CheckAlgebra for Independent {
    /// case literals: the exact expansion [`expr::Literal`] of the engine expression reader (rows4: no longer the
    /// shared `crate::poly`, re-audit RA3-m5)
    type Literal = expr::Literal;
    type Elim<'r> = elim::Elim<'r>;

    fn name(&self) -> &'static str {
        "independent"
    }
    fn is_prime(&self, n: u64) -> bool {
        primes::is_prime(n)
    }
    fn add(&self, p: u64, a: u64, b: u64) -> u64 {
        arith::add(p, a, b)
    }
    fn sub(&self, p: u64, a: u64, b: u64) -> u64 {
        arith::sub(p, a, b)
    }
    fn mul(&self, p: u64, a: u64, b: u64) -> u64 {
        arith::mul(p, a, b)
    }
    fn neg(&self, p: u64, a: u64) -> u64 {
        arith::neg(p, a)
    }
    fn inv(&self, p: u64, a: u64) -> Option<u64> {
        arith::inv(p, a)
    }
    fn from_i128(&self, p: u64, x: i128) -> u64 {
        arith::from_i128(p, x)
    }
    fn q_new(&self, n: i128, d: i128) -> QResult {
        <Q as QArith>::new(n, d)
    }
    fn q_add(&self, a: Q, b: Q) -> QResult {
        a.add(b)
    }
    fn q_sub(&self, a: Q, b: Q) -> QResult {
        a.sub(b)
    }
    fn q_mul(&self, a: Q, b: Q) -> QResult {
        a.mul(b)
    }
    fn q_div(&self, a: Q, b: Q) -> QResult {
        a.div(b)
    }
    fn q_neg(&self, a: Q) -> QResult {
        a.n.checked_neg()
            .map(|n| Q { n, d: a.d })
            .ok_or_else(|| "exact rational overflow".to_string())
    }
    fn q_parse(&self, s: &str) -> QResult {
        <Q as QArith>::parse(s)
    }
    fn q_modp(&self, a: Q, p: u64) -> Option<u64> {
        a.modp(p)
    }
    fn q_inverse(&self, m: &[Vec<Q>]) -> Result<Vec<Vec<Q>>, Unknown> {
        linalg::inverse_q(m)
    }
    fn q_rank(&self, rows: &[Vec<Q>]) -> Result<usize, Unknown> {
        linalg::rank_q(rows)
    }
    fn det_i128(&self, m: &[Vec<i128>]) -> Option<i128> {
        linalg::det_i128(m)
    }
    fn q_rref(&self, rows: &[Vec<Q>], ncols: usize) -> Result<(Vec<Vec<Q>>, Vec<usize>), Unknown> {
        linalg::rref_q(rows, ncols)
    }
    fn q_unit_in_row_space(&self, rows: &[Vec<Q>], col: usize) -> Result<bool, Unknown> {
        linalg::unit_in_row_space(rows, col)
    }
    fn gcd_i128(&self, a: i128, b: i128) -> i128 {
        linalg::gcd_i128(a, b)
    }
    fn int_mat_mul(
        &self,
        a: &[i128],
        b: &[i128],
        r: usize,
        k: usize,
        c: usize,
    ) -> Option<Vec<i128>> {
        linalg::int_mat_mul(a, b, r, k, c)
    }
    fn int_div_exact(&self, a: i128, b: i128) -> Option<i128> {
        linalg::int_div_exact(a, b)
    }
    fn zpoly_det(&self, m: &[Vec<ZPoly>], dim: usize, t: usize) -> ZPoly {
        linalg::zpoly_det(m, dim, t)
    }
    fn eval_ratio(
        &self,
        text: &str,
        arity: usize,
        ctxs: &[Ctx],
        num: &mut [u64],
        den: &mut [u64],
    ) -> Result<CoeffStats, CoeffError> {
        coeff::eval_ratio(text, arity, ctxs, num, den)
    }
    fn cross_check_coefficient(
        &self,
        text: &str,
        arity: usize,
        ctx: &Ctx,
        num: u64,
        den: u64,
    ) -> Result<(), Unknown> {
        coeff::cross_check_engine(text, arity, ctx, num, den).map(|_| ())
    }
    fn expr_linear(&self, text: &str, n: usize) -> Result<Vec<Q>, Unknown> {
        expr::Expr::parse(text, n).and_then(|e| e.linear())
    }
    fn expr_has_d(&self, text: &str, n: usize) -> Result<bool, Unknown> {
        expr::Expr::parse(text, n).map(|e| e.has_d())
    }
    fn literal_parse(&self, text: &str, n: usize) -> Result<expr::Literal, Unknown> {
        expr::Literal::parse(text, n)
    }
    fn literal_is_zero_at(&self, lit: &expr::Literal, point: &[i32]) -> Result<bool, Unknown> {
        lit.is_zero_at(point)
    }
    fn literal_same_up_to_constant(&self, a: &str, b: &str, n: usize) -> bool {
        match (expr::Literal::parse(a, n), expr::Literal::parse(b, n)) {
            (Ok(x), Ok(y)) => x.same_up_to_constant(&y),
            _ => false,
        }
    }
    fn eliminate<'r>(
        &self,
        rows: &'r [Vec<(usize, u64)>],
        ncols: usize,
        p: u64,
        max_entries: usize,
    ) -> Result<elim::Elim<'r>, Unknown> {
        elim::Elim::new_guarded(rows, ncols, p, max_entries)
    }
}

#[cfg(test)]
mod tests {
    /// Rule 2 of the module header: the non-test lines of this directory stay within [`super::LINE_BUDGET`].
    #[test]
    fn independent_algebra_within_line_budget() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src/independent_algebra");
        let mut total = 0;
        let mut per_file = Vec::new();
        for e in std::fs::read_dir(dir).unwrap() {
            let path = e.unwrap().path();
            if path.extension().and_then(|x| x.to_str()) != Some("rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            let body = text.split("#[cfg(test)]").next().unwrap_or("");
            let n = body.lines().count();
            per_file.push((path.file_name().unwrap().to_string_lossy().to_string(), n));
            total += n;
        }
        per_file.sort();
        println!("independent_algebra non-test lines: {total} {per_file:?}");
        assert!(
            total <= super::LINE_BUDGET,
            "independent_algebra has {total} non-test lines > budget {}: {per_file:?}",
            super::LINE_BUDGET
        );
    }

    /// The segregation (rows3): no file of `ibp_rows` / `replay_base` names an item of this module other than the
    /// backend type `Independent` (the convenience entry points that run the certificate backend).
    #[test]
    fn domain_logic_reaches_this_module_only_through_the_trait() {
        let root = env!("CARGO_MANIFEST_DIR");
        let mut files = vec![format!("{root}/src/ibp_rows.rs")];
        for e in std::fs::read_dir(format!("{root}/src/replay_base")).unwrap() {
            files.push(e.unwrap().path().to_string_lossy().to_string());
        }
        for f in files {
            let text = std::fs::read_to_string(&f).unwrap();
            let body = text.split("#[cfg(test)]").next().unwrap_or("");
            for (i, line) in body.lines().enumerate() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                let mut rest = line;
                while let Some(at) = rest.find("independent_algebra::") {
                    let after = &rest[at + "independent_algebra::".len()..];
                    assert!(
                        after.starts_with("Independent"),
                        "{f}:{}: domain logic names independent_algebra::{after}",
                        i + 1
                    );
                    rest = after;
                }
            }
        }
    }
}
