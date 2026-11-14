//! [`Referee`]: the cheap THIRD OPINION of the triage. When the independent and the Symbolica runs first differ at
//! a stage this backend computes with a third code path, the triage reruns the rule on it and names the outlier:
//!
//! | stage | referee's code path (different from both sides) |
//! |---|---|
//! | `Coefficient` | the copied ENGINE expression reader (`independent_algebra::expr`, a tree parser: neither the streaming reader `coeff` nor Symbolica's atom parser) |
//! | `Rows` | field arithmetic in arbitrary precision (numerica `Integer` products reduced mod p: neither u128 nor Montgomery `Zp64`) |
//! | `Elimination`, `Membership` | DENSE row reduction (numerica `Matrix<Zp64>::row_reduce`: neither the copied sparse `Elim` nor `SparseRowReducer`), up to [`REFEREE_DENSE_ENTRIES`] |
//!
//! Every other routine delegates to the independent backend, so a difference first seen at another stage (family
//! table, columns / Lee test, symmetry proofs, primes) has no third opinion: the triage says `Unresolved` there.

use rustred_campaign_core::independent_algebra::{Independent, expr::Expr};
use rustred_campaign_core::replay_base::algebra::{
    CheckAlgebra, CoeffError, CoeffStats, Ctx, Eliminator, Q, QResult, Span, Stage, Unknown, ZPoly,
};
use symbolica::domains::Ring;
use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
use symbolica::domains::integer::Integer;
use symbolica::tensors::matrix::Matrix;

/// Largest dense system (rows x columns) the referee eliminates.
pub const REFEREE_DENSE_ENTRIES: usize = 4_000_000;

/// The stages at which the referee has its own code path.
pub fn referee_covers(stage: Stage) -> bool {
    matches!(
        stage,
        Stage::Coefficient | Stage::Rows | Stage::Elimination | Stage::Membership
    )
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Referee;

fn big_mod(x: Integer, p: u64) -> u64 {
    let pp = Integer::from(p);
    let mut r = x % &pp;
    if r < 0 {
        r += &pp;
    }
    u64::try_from(r).expect("residue")
}

/// Dense elimination for the referee.
pub struct DenseElim {
    p: u64,
    /// RREF rows (dense residues) and their pivot columns
    rref: Vec<Vec<u64>>,
    pivots: Vec<usize>,
    ncols: usize,
}

impl Eliminator for DenseElim {
    fn rank(&self) -> usize {
        self.pivots.len()
    }
    fn entries(&self) -> usize {
        self.rref.len() * self.ncols
    }
    fn pivot_columns(&self) -> Vec<usize> {
        self.pivots.clone()
    }
    fn solve(&self, desired: &[(usize, u64)]) -> Result<Span, Unknown> {
        let p = self.p;
        let mut v = vec![0u64; self.ncols];
        for &(c, x) in desired {
            v[c] = x % p;
        }
        for (row, &pc) in self.rref.iter().zip(&self.pivots) {
            let f = v[pc];
            if f != 0 {
                for (vj, rj) in v.iter_mut().zip(row) {
                    *vj = big_mod(
                        Integer::from(*vj) - Integer::from(f) * Integer::from(*rj),
                        p,
                    );
                }
            }
        }
        let remainder: Vec<(usize, u64)> = v
            .iter()
            .enumerate()
            .filter(|(_, x)| **x != 0)
            .map(|(c, x)| (c, *x))
            .collect();
        Ok(Span {
            in_span: remainder.is_empty(),
            rank: self.pivots.len(),
            weights: Default::default(),
            remainder,
            pivot_rows: Vec::new(),
        })
    }
}

impl CheckAlgebra for Referee {
    type Literal = <Independent as CheckAlgebra>::Literal;
    type Elim<'r> = DenseElim;

    fn name(&self) -> &'static str {
        "referee"
    }
    fn is_prime(&self, n: u64) -> bool {
        Independent.is_prime(n)
    }
    fn add(&self, p: u64, a: u64, b: u64) -> u64 {
        big_mod(Integer::from(a) + Integer::from(b), p)
    }
    fn sub(&self, p: u64, a: u64, b: u64) -> u64 {
        big_mod(Integer::from(a) - Integer::from(b), p)
    }
    fn mul(&self, p: u64, a: u64, b: u64) -> u64 {
        big_mod(Integer::from(a) * Integer::from(b), p)
    }
    fn neg(&self, p: u64, a: u64) -> u64 {
        big_mod(-Integer::from(a), p)
    }
    fn inv(&self, p: u64, a: u64) -> Option<u64> {
        if a.is_multiple_of(p) {
            return None;
        }
        // Fermat: a^(p-2) by square and multiply in arbitrary precision
        let (mut base, mut e, mut r) = (a % p, p - 2, 1u64);
        while e > 0 {
            if e & 1 == 1 {
                r = self.mul(p, r, base);
            }
            base = self.mul(p, base, base);
            e >>= 1;
        }
        Some(r)
    }
    fn from_i128(&self, p: u64, x: i128) -> u64 {
        big_mod(Integer::from(x), p)
    }
    fn q_new(&self, n: i128, d: i128) -> QResult {
        Independent.q_new(n, d)
    }
    fn q_add(&self, a: Q, b: Q) -> QResult {
        Independent.q_add(a, b)
    }
    fn q_sub(&self, a: Q, b: Q) -> QResult {
        Independent.q_sub(a, b)
    }
    fn q_mul(&self, a: Q, b: Q) -> QResult {
        Independent.q_mul(a, b)
    }
    fn q_div(&self, a: Q, b: Q) -> QResult {
        Independent.q_div(a, b)
    }
    fn q_neg(&self, a: Q) -> QResult {
        Independent.q_neg(a)
    }
    fn q_parse(&self, s: &str) -> QResult {
        Independent.q_parse(s)
    }
    fn q_modp(&self, a: Q, p: u64) -> Option<u64> {
        let d = big_mod(Integer::from(a.d), p);
        self.inv(p, d)
            .map(|iv| self.mul(p, big_mod(Integer::from(a.n), p), iv))
    }
    fn q_inverse(&self, m: &[Vec<Q>]) -> Result<Vec<Vec<Q>>, Unknown> {
        Independent.q_inverse(m)
    }
    fn q_rank(&self, rows: &[Vec<Q>]) -> Result<usize, Unknown> {
        Independent.q_rank(rows)
    }
    fn det_i128(&self, m: &[Vec<i128>]) -> Option<i128> {
        Independent.det_i128(m)
    }
    fn q_rref(&self, rows: &[Vec<Q>], ncols: usize) -> Result<(Vec<Vec<Q>>, Vec<usize>), Unknown> {
        Independent.q_rref(rows, ncols)
    }
    fn q_unit_in_row_space(&self, rows: &[Vec<Q>], col: usize) -> Result<bool, Unknown> {
        Independent.q_unit_in_row_space(rows, col)
    }
    fn gcd_i128(&self, a: i128, b: i128) -> i128 {
        Independent.gcd_i128(a, b)
    }
    fn zpoly_det(&self, m: &[Vec<ZPoly>], dim: usize, t: usize) -> ZPoly {
        Independent.zpoly_det(m, dim, t)
    }
    fn eval_ratio(
        &self,
        text: &str,
        arity: usize,
        ctxs: &[Ctx],
        num: &mut [u64],
        den: &mut [u64],
    ) -> Result<CoeffStats, CoeffError> {
        let e = Expr::parse(text, arity).map_err(|msg| CoeffError { pos: 0, msg })?;
        for (k, c) in ctxs.iter().enumerate() {
            match e.at_mod(c.p, &c.vals[..arity], c.vals[arity]) {
                Ok(v) => {
                    num[k] = v;
                    den[k] = 1 % c.p;
                }
                Err(_) => {
                    num[k] = 0;
                    den[k] = 0;
                }
            }
        }
        Ok(CoeffStats::default())
    }
    fn cross_check_coefficient(
        &self,
        _: &str,
        _: usize,
        _: &Ctx,
        _: u64,
        _: u64,
    ) -> Result<(), Unknown> {
        Ok(())
    }
    fn expr_linear(&self, text: &str, n: usize) -> Result<Vec<Q>, Unknown> {
        Independent.expr_linear(text, n)
    }
    fn expr_has_d(&self, text: &str, n: usize) -> Result<bool, Unknown> {
        Independent.expr_has_d(text, n)
    }
    fn literal_parse(&self, text: &str, n: usize) -> Result<Self::Literal, Unknown> {
        Independent.literal_parse(text, n)
    }
    fn literal_is_zero_at(&self, lit: &Self::Literal, point: &[i32]) -> Result<bool, Unknown> {
        Independent.literal_is_zero_at(lit, point)
    }
    fn literal_same_up_to_constant(&self, a: &str, b: &str, n: usize) -> bool {
        Independent.literal_same_up_to_constant(a, b, n)
    }
    fn eliminate(
        &self,
        rows: &[Vec<(usize, u64)>],
        ncols: usize,
        p: u64,
        _max_entries: usize,
    ) -> Result<DenseElim, Unknown> {
        if rows.len().saturating_mul(ncols) > REFEREE_DENSE_ENTRIES || rows.is_empty() || ncols == 0
        {
            return Err(format!(
                "referee: dense system {} x {ncols} outside the referee's size",
                rows.len()
            ));
        }
        let f = Zp64::new(p);
        let data: Vec<Vec<_>> = rows
            .iter()
            .map(|r| {
                let mut d = vec![f.zero(); ncols];
                for &(c, v) in r {
                    d[c] = f.to_element(v % p);
                }
                d
            })
            .collect();
        let mut m = Matrix::from_nested_vec(data, f).map_err(|e| e.to_string())?;
        let rank = m.row_reduce(ncols as u32);
        let f = Zp64::new(p);
        let mut rref = Vec::with_capacity(rank);
        let mut pivots = Vec::with_capacity(rank);
        for i in 0..rank {
            let mut row: Vec<u64> = (0..ncols)
                .map(|j| f.from_element(&m[(i as u32, j as u32)]))
                .collect();
            let pc = row
                .iter()
                .position(|&x| x != 0)
                .ok_or("referee: zero rank row")?;
            let iv = self.inv(p, row[pc]).ok_or("referee: zero pivot")?;
            for x in row.iter_mut() {
                *x = self.mul(p, *x, iv);
            }
            pivots.push(pc);
            rref.push(row);
        }
        Ok(DenseElim {
            p,
            rref,
            pivots,
            ncols,
        })
    }
}
