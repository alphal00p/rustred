//! THE CERTIFICATE BINDING (rows4, re-audit RA3-M1 / RA3-m1): the only backend value that may produce objects of
//! certificate origin ([`AlgebraOrigin::CERTIFICATE`]) is [`CertAlg`], which is PRIVATE to this crate and delegates every
//! algebraic step to `crate::independent_algebra::Independent` (optionally recording the [`Stage`] trace into a
//! [`Recorder`]: the `Both` mode's certificate run). It contains no algebra. The concrete certificate entry points
//! (`Family::parse*`, `verify_group*`, `identity_test*`, `replay_evidence*`, `verify_record`) construct it themselves;
//! a caller cannot pass a backend of its own into a certificate run, so neither `CheckAlgebra::name` nor any other
//! self-declaration of a backend decides the level.

use super::algebra::{
    AlgebraOrigin, CheckAlgebra, CoeffError, CoeffStats, Ctx, Q, QResult, Recorder, Stage, Unknown,
    ZPoly,
};
use crate::independent_algebra::Independent;

/// `Independent`, seen through an optional trace recorder (crate-private: see the module doc).
#[derive(Clone, Copy, Default)]
pub(crate) struct CertAlg<'a> {
    rec: Option<&'a Recorder>,
}

impl<'a> CertAlg<'a> {
    pub(crate) fn new(rec: Option<&'a Recorder>) -> CertAlg<'a> {
        CertAlg { rec }
    }
    pub(crate) const ORIGIN: AlgebraOrigin = AlgebraOrigin::CERTIFICATE;
}

const I: Independent = Independent;

impl CheckAlgebra for CertAlg<'_> {
    type Literal = <Independent as CheckAlgebra>::Literal;
    type Elim<'r>
        = <Independent as CheckAlgebra>::Elim<'r>
    where
        Self: 'r;

    fn name(&self) -> &'static str {
        I.name()
    }
    fn is_prime(&self, n: u64) -> bool {
        I.is_prime(n)
    }
    fn add(&self, p: u64, a: u64, b: u64) -> u64 {
        I.add(p, a, b)
    }
    fn sub(&self, p: u64, a: u64, b: u64) -> u64 {
        I.sub(p, a, b)
    }
    fn mul(&self, p: u64, a: u64, b: u64) -> u64 {
        I.mul(p, a, b)
    }
    fn neg(&self, p: u64, a: u64) -> u64 {
        I.neg(p, a)
    }
    fn inv(&self, p: u64, a: u64) -> Option<u64> {
        I.inv(p, a)
    }
    fn from_i128(&self, p: u64, x: i128) -> u64 {
        I.from_i128(p, x)
    }
    fn q_new(&self, n: i128, d: i128) -> QResult {
        I.q_new(n, d)
    }
    fn q_add(&self, a: Q, b: Q) -> QResult {
        I.q_add(a, b)
    }
    fn q_sub(&self, a: Q, b: Q) -> QResult {
        I.q_sub(a, b)
    }
    fn q_mul(&self, a: Q, b: Q) -> QResult {
        I.q_mul(a, b)
    }
    fn q_div(&self, a: Q, b: Q) -> QResult {
        I.q_div(a, b)
    }
    fn q_neg(&self, a: Q) -> QResult {
        I.q_neg(a)
    }
    fn q_parse(&self, s: &str) -> QResult {
        I.q_parse(s)
    }
    fn q_modp(&self, a: Q, p: u64) -> Option<u64> {
        I.q_modp(a, p)
    }
    fn q_inverse(&self, m: &[Vec<Q>]) -> Result<Vec<Vec<Q>>, Unknown> {
        I.q_inverse(m)
    }
    fn q_rank(&self, rows: &[Vec<Q>]) -> Result<usize, Unknown> {
        I.q_rank(rows)
    }
    fn det_i128(&self, m: &[Vec<i128>]) -> Option<i128> {
        I.det_i128(m)
    }
    fn q_rref(&self, rows: &[Vec<Q>], ncols: usize) -> Result<(Vec<Vec<Q>>, Vec<usize>), Unknown> {
        I.q_rref(rows, ncols)
    }
    fn q_unit_in_row_space(&self, rows: &[Vec<Q>], col: usize) -> Result<bool, Unknown> {
        I.q_unit_in_row_space(rows, col)
    }
    fn gcd_i128(&self, a: i128, b: i128) -> i128 {
        I.gcd_i128(a, b)
    }
    fn int_mat_mul(
        &self,
        a: &[i128],
        b: &[i128],
        r: usize,
        k: usize,
        c: usize,
    ) -> Option<Vec<i128>> {
        I.int_mat_mul(a, b, r, k, c)
    }
    fn int_div_exact(&self, a: i128, b: i128) -> Option<i128> {
        I.int_div_exact(a, b)
    }
    fn zpoly_det(&self, m: &[Vec<ZPoly>], dim: usize, t: usize) -> ZPoly {
        I.zpoly_det(m, dim, t)
    }
    fn eval_ratio(
        &self,
        text: &str,
        arity: usize,
        ctxs: &[Ctx],
        num: &mut [u64],
        den: &mut [u64],
    ) -> Result<CoeffStats, CoeffError> {
        I.eval_ratio(text, arity, ctxs, num, den)
    }
    fn cross_check_coefficient(
        &self,
        text: &str,
        arity: usize,
        ctx: &Ctx,
        num: u64,
        den: u64,
    ) -> Result<(), Unknown> {
        I.cross_check_coefficient(text, arity, ctx, num, den)
    }
    fn expr_linear(&self, text: &str, n: usize) -> Result<Vec<Q>, Unknown> {
        I.expr_linear(text, n)
    }
    fn expr_has_d(&self, text: &str, n: usize) -> Result<bool, Unknown> {
        I.expr_has_d(text, n)
    }
    fn literal_parse(&self, text: &str, n: usize) -> Result<Self::Literal, Unknown> {
        I.literal_parse(text, n)
    }
    fn literal_is_zero_at(&self, lit: &Self::Literal, point: &[i32]) -> Result<bool, Unknown> {
        I.literal_is_zero_at(lit, point)
    }
    fn literal_same_up_to_constant(&self, a: &str, b: &str, n: usize) -> bool {
        I.literal_same_up_to_constant(a, b, n)
    }
    fn eliminate<'r>(
        &self,
        rows: &'r [Vec<(usize, u64)>],
        ncols: usize,
        p: u64,
        max_entries: usize,
    ) -> Result<<Independent as CheckAlgebra>::Elim<'r>, Unknown> {
        I.eliminate(rows, ncols, p, max_entries)
    }
    fn observe(&self, stage: Stage, at: &dyn Fn() -> String, value: &dyn Fn() -> String) {
        if let Some(r) = self.rec {
            r.record(stage, at, value);
        }
    }
}
