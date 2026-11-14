//! THE ONLY INTERFACE between the identity test's DOMAIN LOGIC (family model, row generation, columns / served keys,
//! verdicts, records: [`crate::ibp_rows`], [`super::family`], [`super::rows`], [`super::symrows`], [`super::primes`])
//! and ANY ALGEBRA (DEV-17 REVISED, user decision 2026-11-14). The domain logic is generic over [`CheckAlgebra`]
//! (static dispatch) and calls algebra only through it. Two implementations exist:
//!
//! - [`crate::independent_algebra::Independent`] (this crate): the segregated, Symbolica-free duplicate whose reason for
//!   existence is stated at the top of `src/independent_algebra/mod.rs`. It is the CERTIFICATE OF RECORD: only a run
//!   on this backend can grant the `replay` evidence level.
//! - `rustred_campaign_algebra::SymbolicaAlgebra` (crate `rustred-campaign-algebra`): the same trait on Symbolica /
//!   numerica, for the alternative / `Both` (triage) modes of the checker.
//!
//! This file holds the trait and the plain DATA types exchanged through it; it contains no algebra (no arithmetic
//! beyond constructors). [`Q`] is data: its arithmetic is `CheckAlgebra::q_*`.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

/// An error text of the replay code (unreadable input, overflow, singular matrix, ...).
pub type Unknown = String;

/// An exact rational as DATA: `n / d` in lowest terms with `d > 0` (every backend returns this normal form, so `==`
/// is value equality). Arithmetic ONLY through [`CheckAlgebra`] (`q_add`, `q_mul`, ...).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Q {
    pub n: i128,
    pub d: i128,
}

/// Result of an exact rational operation (`Err` = overflow of the i128 normal form, division by zero, bad text).
pub type QResult = Result<Q, String>;

impl Q {
    pub const ZERO: Q = Q { n: 0, d: 1 };
    pub const ONE: Q = Q { n: 1, d: 1 };
    /// the integer `n` (already in normal form)
    pub fn int(n: i128) -> Q {
        Q { n, d: 1 }
    }
    pub fn is_zero(&self) -> bool {
        self.n == 0
    }
}

/// One evaluation context of a coefficient: a prime and the values of n0..n{N-1} followed by d (length arity + 1),
/// all reduced mod p.
#[derive(Clone, Debug)]
pub struct Ctx {
    pub p: u64,
    pub vals: Vec<u64>,
}

/// Size and degrees of one side (numerator or denominator) of a coefficient text, as read by the backend (the
/// independent reader counts monomials syntactically; a backend that merges like terms may report fewer).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct SideStats {
    pub terms: usize,
    /// max total degree of a monomial (n and d)
    pub deg: u32,
    /// max degree in d of a monomial
    pub degd: u32,
    /// max degree in the n variables of a monomial
    pub degn: u32,
    /// max number of factors of a monomial (audit M1: a bare denominator must have exactly one)
    pub factors: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct CoeffStats {
    pub num: SideStats,
    pub den: SideStats,
    pub has_den: bool,
}

/// A coefficient text the backend refuses (outside the grammar, or unsupported by that backend).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoeffError {
    pub pos: usize,
    pub msg: String,
}

impl fmt::Display for CoeffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "coefficient syntax at byte {}: {}", self.pos, self.msg)
    }
}

impl std::error::Error for CoeffError {}

/// Prefix of the error of [`CheckAlgebra::eliminate`] when its entry cap is hit.
pub const MEMORY_GUARD: &str = "memory guard";

/// The span test of one desired vector against eliminated rows mod p: weights `w` with `desired - sum w_i rows_i`
/// reduced to `remainder`; `in_span` iff the remainder is empty. A backend MUST re-verify the certificate (recompute
/// `desired - sum w_i rows_i` on the ORIGINAL rows and compare with the remainder) before returning it.
#[derive(Clone, Debug)]
pub struct Span {
    pub in_span: bool,
    pub rank: usize,
    pub weights: BTreeMap<usize, u64>,
    pub remainder: Vec<(usize, u64)>,
    /// source row index of every pivot, in elimination order
    pub pivot_rows: Vec<usize>,
}

/// Rows eliminated modulo a prime, reusable for several desired vectors (the rule vector, mutants, corners).
pub trait Eliminator {
    fn rank(&self) -> usize;
    /// stored entries (a memory measure; backends count differently)
    fn entries(&self) -> usize;
    /// pivot columns in elimination order (a trace quantity of the triage)
    fn pivot_columns(&self) -> Vec<usize>;
    fn solve(&self, desired: &[(usize, u64)]) -> Result<Span, Unknown>;
}

/// A sparse polynomial over Z in t variables (exponent vector -> coefficient): the graph polynomials U, F.
pub type ZPoly = HashMap<Vec<u8>, i128>;

/// The stages of the identity test at which the triage (`Both` mode) compares intermediate quantities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
pub enum Stage {
    /// the family model: operator table, quadratic basis (rational arithmetic, matrix inverse)
    Family,
    /// the two verification primes
    Primes,
    /// a coefficient value at one evaluation context (num / den mod p)
    Coefficient,
    /// the rows of one cell (one entry per row: a digest of its entries mod p)
    Rows,
    /// the columns of one cell after the zero-sector test (Lee criterion)
    Columns,
    /// the elimination of one cell: rank and pivot structure
    Elimination,
    /// a span-membership verdict (rule vector, mutant, corners) with its witness column
    Membership,
    /// the symmetry group: verification (determinants, inverses, decompositions)
    Symmetry,
    /// the final verdict of the domain logic
    Verdict,
}

/// The algebra the identity test needs (DEV-17 REVISED). Residues are canonical `u64` in `[0, p)`; rationals are
/// [`Q`]. Every method is a pure function of its arguments (deterministic, no hidden state beyond caches that do not
/// change results), so two backends fed the same inputs must return the same outputs: the triage relies on that.
pub trait CheckAlgebra: Sync {
    /// a parsed case literal (polynomial in n0..n{N-1}, no d) of this backend
    type Literal;
    /// eliminated rows of this backend
    type Elim<'r>: Eliminator
    where
        Self: 'r;

    /// `"independent"` (the certificate of record) or the name of a diagnostic backend
    fn name(&self) -> &'static str;

    // ---- primes ----
    /// primality of a u64 (deterministic)
    fn is_prime(&self, n: u64) -> bool;

    // ---- the prime field F_p (p odd prime < 2^63) ----
    fn add(&self, p: u64, a: u64, b: u64) -> u64;
    fn sub(&self, p: u64, a: u64, b: u64) -> u64;
    fn mul(&self, p: u64, a: u64, b: u64) -> u64;
    fn neg(&self, p: u64, a: u64) -> u64;
    /// `None` iff `a = 0 mod p`
    fn inv(&self, p: u64, a: u64) -> Option<u64>;
    fn from_i128(&self, p: u64, x: i128) -> u64;

    // ---- exact rationals (i128 normal form; overflow = Err) ----
    fn q_new(&self, n: i128, d: i128) -> QResult;
    fn q_add(&self, a: Q, b: Q) -> QResult;
    fn q_sub(&self, a: Q, b: Q) -> QResult;
    fn q_mul(&self, a: Q, b: Q) -> QResult;
    fn q_div(&self, a: Q, b: Q) -> QResult;
    fn q_neg(&self, a: Q) -> QResult;
    /// `a` or `a/b`
    fn q_parse(&self, s: &str) -> QResult;
    /// `None` iff p divides the denominator
    fn q_modp(&self, a: Q, p: u64) -> Option<u64>;

    // ---- exact linear algebra ----
    /// inverse of a square rational matrix (`Err` if singular or on overflow)
    fn q_inverse(&self, m: &[Vec<Q>]) -> Result<Vec<Vec<Q>>, Unknown>;
    /// rank of a rational matrix (`Err` on overflow)
    fn q_rank(&self, rows: &[Vec<Q>]) -> Result<usize, Unknown>;
    /// determinant of a square integer matrix (`None` on overflow)
    fn det_i128(&self, m: &[Vec<i128>]) -> Option<i128>;
    /// reduced row echelon form over Q of `rows` on their first `ncols` columns (later columns are carried along):
    /// (rows in RREF, pivot columns); every pivot is 1 and the other entries of a pivot column are 0
    fn q_rref(&self, rows: &[Vec<Q>], ncols: usize) -> Result<(Vec<Vec<Q>>, Vec<usize>), Unknown>;
    /// is the unit vector `e_col` in the row space of `rows` (all rows of equal length)?
    fn q_unit_in_row_space(&self, rows: &[Vec<Q>], col: usize) -> Result<bool, Unknown>;
    /// gcd of two integers (non-negative; gcd(0, 0) = 0)
    fn gcd_i128(&self, a: i128, b: i128) -> i128;
    /// determinant of the leading `dim x dim` block of a matrix of integer polynomials in `t` variables
    fn zpoly_det(&self, m: &[Vec<ZPoly>], dim: usize, t: usize) -> ZPoly;

    // ---- coefficient texts "(P)/(Q)" (the engine's strict grammar; interfaces/ibp-rows.md section 3) ----
    /// evaluate one coefficient text at every context: `num[k]`, `den[k]` mod `ctxs[k].p` (`den` = 1 without '/');
    /// a zero `den` is returned (a pole), never inverted
    fn eval_ratio(
        &self,
        text: &str,
        arity: usize,
        ctxs: &[Ctx],
        num: &mut [u64],
        den: &mut [u64],
    ) -> Result<CoeffStats, CoeffError>;
    /// a second reading of an accepted coefficient at one context (`num / den`, `den != 0`): `Err` = the backend's
    /// two readers disagree (a typed refusal). A backend without a second reader returns `Ok(())`.
    fn cross_check_coefficient(
        &self,
        text: &str,
        arity: usize,
        ctx: &Ctx,
        num: u64,
        den: u64,
    ) -> Result<(), Unknown>;

    // ---- the engine expression language (case equations, witness loci, power shifts) ----
    /// the affine form `[constant, n0 .. n{N-1}, d]` of an expression (`Err` if not affine or unreadable)
    fn expr_linear(&self, text: &str, n: usize) -> Result<Vec<Q>, Unknown>;
    /// does the expression contain d?
    fn expr_has_d(&self, text: &str, n: usize) -> Result<bool, Unknown>;

    // ---- case literals (exact integer polynomials in n, no d) ----
    fn literal_parse(&self, text: &str, n: usize) -> Result<Self::Literal, Unknown>;
    /// exact zero test at an integer point
    fn literal_is_zero_at(&self, lit: &Self::Literal, point: &[i32]) -> Result<bool, Unknown>;
    /// equal up to a nonzero constant factor (exact)
    fn literal_same_up_to_constant(&self, a: &str, b: &str, n: usize) -> bool;

    // ---- sparse elimination mod p ----
    /// eliminate sparse rows (sorted `(column, value)` lists) over F_p; `Err(MEMORY_GUARD ..)` above `max_entries`
    fn eliminate<'r>(
        &self,
        rows: &'r [Vec<(usize, u64)>],
        ncols: usize,
        p: u64,
        max_entries: usize,
    ) -> Result<Self::Elim<'r>, Unknown>;

    // ---- triage hook ----
    /// called by the domain logic at every [`Stage`] quantity (lazy: `at` names the place, `value` renders the
    /// quantity). The certificate backends do nothing; the triage's recording wrapper stores the pairs.
    fn observe(&self, _stage: Stage, _at: &dyn Fn() -> String, _value: &dyn Fn() -> String) {}
}
