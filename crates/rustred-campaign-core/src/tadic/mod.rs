//! # tadic: the SYSTEM pole order p* of a target key at d = d0 (TRICKS 15, 16; lane tadic)
//!
//! A seed set gives a finite linear system of IBP rows over Q(d) in the unknown integrals. For a target key K and a
//! "lower" predicate (the node comparator by default) an ADMISSIBLE relation is an element of the row space over Q(d)
//! of the form `I(K) + (terms on lower columns only)`. With `t = d - d0` and R = Q[t] localised at t = 0 (a discrete
//! valuation ring), the system pole order is
//!
//! `p*(K) = min over admissible relations (coefficient 1 at K) of max(0, max over the columns of interest of the pole
//! order at t = 0)`,
//!
//! `+inf` ("no relation") when no admissible relation exists. It is an invariant of the row SPACE (not of the rows,
//! their order or a pivot rule): with V the projection of {row space, upper part 0} to the columns of interest + K and
//! Λ = V ∩ R^{..} its saturated lattice, p* is the valuation of the generator of the ideal {x_K : x in Λ}. Proofs,
//! the dual certificate and what a screen at d0 can and cannot conclude: WS/notes/implementer.tadic.md
//! ("THE MATHEMATICAL OBJECT"); interface WS/plan/auto-campaign/interfaces/tadic.md.
//!
//! This module is the Symbolica-free part (core rule): the SYSTEM (rows with exact coefficients `a + b d`, built from
//! the family text by the campaign's own IBP generator [`crate::replay_base::family::Family::exact_row`]), the seed
//! shells, the query / result records and the oracle trait [`PoleOrderOracle`]. The elimination itself (exact over
//! Z[t] with Symbolica polynomials; truncated screens) lives in `rustred_campaign_algebra::tadic`, which implements
//! the trait. Nothing here is specific to a node (L8): arity, lines, shells, d0, columns of interest and every bound
//! are inputs.

pub mod system;

use serde::Serialize;

pub use system::{
    ExactSystem, RowLabel, ShellSpec, SystemId, build_ibp_system, parse_shell, seed_set_sha256,
    seeds_around, shell_offsets,
};

/// Version of the p* record and of the elimination convention (bump on any change of either).
pub const TADIC_VERSION: &str = "rustred.campaign.tadic.v1";

/// An exact rational `n / d` (d > 0), for d0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Rat {
    pub n: i128,
    pub d: i128,
}

impl Rat {
    pub fn int(n: i128) -> Rat {
        Rat { n, d: 1 }
    }
    /// `a/b` in lowest terms, b > 0 (None for b = 0).
    pub fn new(a: i128, b: i128) -> Option<Rat> {
        if b == 0 {
            return None;
        }
        let g = gcd_i128(a, b).max(1);
        let s = if b < 0 { -1 } else { 1 };
        Some(Rat {
            n: s * a / g,
            d: s * b / g,
        })
    }
    /// Text `a` or `a/b`.
    pub fn parse(text: &str) -> Option<Rat> {
        let t = text.trim();
        match t.split_once('/') {
            None => t.parse().ok().map(Rat::int),
            Some((a, b)) => Rat::new(a.trim().parse().ok()?, b.trim().parse().ok()?),
        }
    }
    pub fn text(&self) -> String {
        if self.d == 1 {
            self.n.to_string()
        } else {
            format!("{}/{}", self.n, self.d)
        }
    }
}

fn gcd_i128(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// Where poles count (the brief's "columns of interest"); every other lower column is allowed with any pole.
#[derive(Clone, Debug)]
pub enum Interest {
    /// every right-hand-side term (default)
    AllLower,
    /// only these keys (e.g. the terminal set); keys that are not lower or not in the system are ignored
    Keys(Vec<Vec<i64>>),
}

/// How the elimination computes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Arith {
    /// exact over Z[t] (no truncation, exact zero tests): the only mode whose p* is a VERDICT
    Exact,
    /// SCREEN: truncated power series mod `prime` up to (excluding) t^order, precision tracked; its p* is never a
    /// verdict ("try exact"); `order = 1` is "the rows evaluated at d0" (what the engine's probe mode computes)
    Screen { order: u32, prime: u64 },
}

/// Cost control (all spec parameters; `None` = unbounded). Exceeding one gives `Verdict::Undecided`, never a p*.
#[derive(Clone, Debug, Serialize)]
pub struct TadicBudget {
    pub max_rows: Option<usize>,
    pub max_columns: Option<usize>,
    /// total number of polynomial terms held by the pivot rows at any time (memory proxy)
    pub max_terms: Option<usize>,
    /// largest t-degree of one entry
    pub max_degree: Option<u32>,
    /// wall clock (a TIME decision: recorded as such, U-1)
    pub max_seconds: Option<f64>,
}

impl Default for TadicBudget {
    fn default() -> Self {
        TadicBudget {
            max_rows: None,
            max_columns: None,
            max_terms: Some(200_000_000),
            max_degree: None,
            max_seconds: None,
        }
    }
}

/// One question to the oracle.
pub struct PoleQuery<'a> {
    pub target: &'a [i64],
    /// `lower(key)`: may `key` appear on the right-hand side of a relation for `target` (strictly lower under the
    /// node order; the caller's predicate, e.g. `NodeComparator::lower(key, target)`)
    pub lower: &'a dyn Fn(&[i64]) -> bool,
    pub d0: Rat,
    pub interest: Interest,
    pub arith: Arith,
    pub budget: TadicBudget,
    /// return an exact relation attaining p* (costs one more pass over the pivot rows)
    pub want_relation: bool,
}

/// p* itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum PStar {
    /// the best admissible relation has pole order exactly this at d0 (0 = a regular relation exists)
    Order(u32),
    /// the system contains no admissible relation for the target
    NoRelation,
}

/// What the p* of a result is worth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Verdict {
    /// exact computation over Z[t]: p* is an invariant of THIS system (named by `system`)
    Exact,
    /// a truncated / modular screen: p* is a hint only ("try exact"); a screen hit can prove regularity only after an
    /// exact confirmation (see `confirmed_upper`)
    Screen,
    /// a budget ran out (which one in the text); no p*
    Undecided(String),
}

/// One term of an exact relation `I(K) = sum c_j I(k_j)`: the coefficient as a reduced rational function in d (text).
#[derive(Clone, Debug, Serialize)]
pub struct RelTerm {
    pub key: Vec<i64>,
    pub coefficient: String,
    /// pole order at d0 of this coefficient (0 = regular)
    pub pole_order: u32,
    pub of_interest: bool,
}

/// Pole profile (pole-diagnostics.md addendum 8): measures of one relation, local (per rhs term).
#[derive(Clone, Debug, Default, Serialize)]
pub struct Profile {
    /// max pole order over the columns of interest (= p of the relation)
    pub max_order: u32,
    /// sum of the pole orders over the columns of interest
    pub sum_orders: u64,
    /// number of singular terms among the columns of interest
    pub singular_terms: usize,
    pub terms: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct Relation {
    pub terms: Vec<RelTerm>,
    pub profile: Profile,
}

/// Rank data of one run (`dim_v` = number of saturated pivots on the columns of interest).
#[derive(Clone, Debug, Default, Serialize)]
pub struct RankData {
    pub rows: usize,
    pub columns: usize,
    pub upper_columns: usize,
    pub lower_columns: usize,
    pub interest_columns: usize,
    /// pivots found on upper columns (phase 1)
    pub upper_pivots: usize,
    /// generators of {row space, upper part 0} handed to phase 2
    pub w0_generators: usize,
    /// rank of V (phase-2 pivots, + 1 if e_K lies in V)
    pub dim_v: usize,
    /// rank of the row space over Q(d) when every lower column is of interest (= upper_pivots + dim_v); else None
    pub rank_rows: Option<usize>,
    /// total t-exponent divided out by the saturation steps of phase 2 (> 0: the row module was not saturated there)
    pub saturation_exponent: u64,
    /// the target column appears in no row
    pub target_absent: bool,
}

/// The answer.
#[derive(Clone, Debug, Serialize)]
pub struct PoleOrderResult {
    pub schema: &'static str,
    pub system: SystemId,
    pub target: Vec<i64>,
    pub d0: String,
    pub arith: Arith,
    pub verdict: Verdict,
    /// `None` iff `verdict` is `Undecided`
    pub p_star: Option<PStar>,
    pub relation: Option<Relation>,
    pub rank: RankData,
    /// what "rows evaluated at d0" alone says (screen, order 1), when computed: `Some(true)` = an admissible relation
    /// exists at d0; `Some(false)` = none at d0 (proves NOTHING about p*: TRICKS 16)
    pub screen_at_d0: Option<bool>,
    pub seconds: f64,
    pub notes: Vec<String>,
}

/// The p* oracle (implemented by `rustred_campaign_algebra::tadic::SymbolicaTadic`; core code such as the seed
/// minimiser takes `&dyn PoleOrderOracle` so that it stays Symbolica-free).
pub trait PoleOrderOracle {
    fn best_pole_order(&self, sys: &ExactSystem, q: &PoleQuery<'_>) -> PoleOrderResult;
}

/// Verdict of the seed-minimisation acceptance hook.
#[derive(Clone, Debug, Serialize)]
pub struct KeepVerdict {
    /// `Some(true)`: p*(after) <= bound (the drop keeps the pole objective); `Some(false)`: it does not (or the
    /// after-system has no relation); `None`: undecided (budget): the caller must treat it as "do not drop"
    pub kept: Option<bool>,
    /// the bound used (p* of the before-system, or the caller's)
    pub bound: Option<PStar>,
    pub before: Option<PoleOrderResult>,
    pub after: PoleOrderResult,
}

/// Ordering of p* values: a smaller order is better; `NoRelation` is the worst.
pub fn pstar_le(a: PStar, b: PStar) -> bool {
    match (a, b) {
        (_, PStar::NoRelation) => true,
        (PStar::NoRelation, PStar::Order(_)) => false,
        (PStar::Order(x), PStar::Order(y)) => x <= y,
    }
}

/// The seed-minimisation acceptance test (TRICKS 15 (iii)): does dropping seeds keep the pole objective?
///
/// `after` must be a SUB-system of `before` (fewer seeds of the same family): then p*(after) >= p*(before) always,
/// and the drop keeps the objective iff p*(after) <= p*(before). With `bound = Some(b)` the before-system is not
/// solved (the minimiser computes the bound once, from the full system, and reuses it for every drop). Only EXACT
/// p* values decide; any other verdict answers `kept: None` (= do not drop).
pub fn keeps_regularity(
    oracle: &dyn PoleOrderOracle,
    before: &ExactSystem,
    after: &ExactSystem,
    q: &PoleQuery<'_>,
    bound: Option<PStar>,
) -> KeepVerdict {
    let (bound, before_res) = match bound {
        Some(b) => (Some(b), None),
        None => {
            let r = oracle.best_pole_order(before, q);
            let b = match (&r.verdict, r.p_star) {
                (Verdict::Exact, Some(p)) => Some(p),
                _ => None,
            };
            (b, Some(r))
        }
    };
    let after_res = oracle.best_pole_order(after, q);
    let kept = match (bound, &after_res.verdict, after_res.p_star) {
        (Some(_), Verdict::Exact, Some(PStar::NoRelation)) => Some(false),
        (Some(b), Verdict::Exact, Some(p)) => Some(pstar_le(p, b)),
        _ => None,
    };
    KeepVerdict {
        kept,
        bound,
        before: before_res,
        after: after_res,
    }
}
