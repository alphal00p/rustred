//! CHECK MODE of the tadic oracle (L11): an independent route through Symbolica / numerica, for small and medium
//! systems (dense matrices; tests and spot checks).
//!
//! - W_0 from numerica's own row reduction (`Matrix<RationalPolynomialField<Z>>::row_reduce`, upper columns first),
//!   in the variable t = d - d0; V = its rows without upper entries, projected to (columns of interest + K).
//! - UPPER bound: the returned relation is in the row space (rank of rows = rank of rows + relation vector, numerica)
//!   and its pole order is recomputed from its TEXT (Symbolica parser, shift to t, lowest exponents).
//! - LOWER bound: the DUAL CERTIFICATE y (notes: y_K = 1, y ⊥ V, v(y_S) >= p*) is built from the oracle's basis and
//!   checked against numerica's V entry by entry; "no relation" is checked as "every V vector has K-coordinate 0".
//! - [`brute_force_upper`]: an ansatz search (Laurent-polynomial combination coefficients of bounded degree, linear
//!   algebra over Q with numerica) giving an upper bound of p* that converges to p* as the ansatz grows.

use std::sync::Arc;

use rustred_campaign_core::tadic::{ExactSystem, Interest, PStar, PoleOrderResult, PoleQuery};
use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::domains::integer::{Integer, IntegerRing, Z};
use symbolica::domains::rational::{Q, Rational};
use symbolica::domains::rational_polynomial::{
    FromNumeratorAndDenominator, RationalPolynomial, RationalPolynomialField,
};
use symbolica::domains::{Field, RingOps};
use symbolica::poly::PolyVariable;
use symbolica::poly::polynomial::MultivariatePolynomial;
use symbolica::prelude::AtomCore;
use symbolica::tensors::matrix::Matrix;
use symbolica::try_parse;

use super::{ZP, exact_basis, t_vars};

pub(crate) type RF = RationalPolynomial<IntegerRing, u16>;

/// What the check mode found.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct CheckReport {
    pub rank_rows: usize,
    pub dim_w0: usize,
    /// the relation lies in the row space (None = no relation to check)
    pub relation_in_span: Option<bool>,
    /// pole order of the relation recomputed from its text
    pub relation_order: Option<u32>,
    /// the dual certificate is orthogonal to V and has v(y_S) >= the claimed p* (None = not applicable)
    pub dual_ok: Option<bool>,
    /// the certificate's own bound min v(y_S)
    pub dual_bound: Option<u32>,
    /// "no relation": every V vector has K-coordinate 0
    pub no_relation_ok: Option<bool>,
    /// everything that applies agrees with the claimed p*
    pub consistent: bool,
}

pub(crate) struct Ctx {
    pub(crate) vars: Arc<Vec<PolyVariable>>,
    pub(crate) field: RationalPolynomialField<IntegerRing, u16>,
}

impl Ctx {
    pub(crate) fn new() -> Ctx {
        Ctx {
            vars: t_vars(),
            field: RationalPolynomialField::new(Z),
        }
    }
    pub(crate) fn poly(&self, c: &[(u16, Integer)]) -> ZP {
        let mut p: ZP = MultivariatePolynomial::new(&Z, Some(c.len()), self.vars.clone());
        for (e, x) in c {
            if !x.is_zero() {
                p.append_monomial(x.clone(), &[*e]);
            }
        }
        p
    }
    pub(crate) fn rf(&self, num: ZP, den: ZP) -> RF {
        RationalPolynomial::from_num_den(num, den, &Z, true)
    }
    fn zero(&self) -> RF {
        self.rf(self.poly(&[]), self.poly(&[(0, Integer::one())]))
    }
    fn one(&self) -> RF {
        self.rf(
            self.poly(&[(0, Integer::one())]),
            self.poly(&[(0, Integer::one())]),
        )
    }
}

fn val_poly(p: &ZP) -> i64 {
    if p.is_zero() {
        i64::MAX
    } else {
        p.exponents(0)[0] as i64
    }
}

fn val_rf(r: &RF) -> i64 {
    if r.numerator.is_zero() {
        i64::MAX
    } else {
        val_poly(&r.numerator) - val_poly(&r.denominator)
    }
}

/// Coefficient text (in d) -> rational function in t = d - d0 (exact; rational d0 by clearing denominators).
pub(crate) fn parse_in_t_with(
    ctx: &Ctx,
    text: &str,
    d0: &rustred_campaign_core::tadic::Rat,
) -> Result<RF, String> {
    let ns = NamespacedSymbol::try_parse("rustred::d").ok_or("symbol d")?;
    let ds = SymbolBuilder::new(ns).build().map_err(|e| e.to_string())?;
    let dvars = Arc::new(vec![PolyVariable::Symbol(ds)]);
    let atom = try_parse!(text, default_namespace = "rustred").map_err(|e| e.to_string())?;
    let rp: RF = atom
        .as_view()
        .try_to_rational_polynomial::<_, _, u16>(&Q, &Z, Some(dvars.clone()))
        .map_err(|e| format!("not a rational function of d: {e}"))?;
    if rp.numerator.get_vars_ref().len() != 1 {
        return Err("coefficient has symbols other than d".into());
    }
    let shift = Rational::new(Integer::from(d0.n), Integer::from(d0.d));
    let to_t =
        |p: &ZP| -> MultivariatePolynomial<symbolica::domains::rational::RationalField, u16> {
            p.map_coeff(|c| Rational::from(c.clone()), Q)
                .shift_var(0, &shift)
        };
    let n = to_t(&rp.numerator);
    let d = to_t(&rp.denominator);
    let mut l = Integer::one();
    for c in n.coefficients.iter().chain(&d.coefficients) {
        l = l.lcm(&c.denominator());
    }
    let z = |p: &MultivariatePolynomial<symbolica::domains::rational::RationalField, u16>| -> ZP {
        let c: Vec<(u16, Integer)> = (0..p.nterms())
            .map(|i| {
                (
                    p.exponents(i)[0],
                    (&p.coefficients[i] * &Rational::from(l.clone())).numerator(),
                )
            })
            .collect();
        ctx.poly(&c)
    };
    Ok(ctx.rf(z(&n), z(&d)))
}

/// The DUAL vector y (y_K = 1, y orthogonal to V, `min v(y_S)` = the lower bound of p*) from the oracle's exact
/// triangular basis by back substitution, keyed by system column; and the basis' own p*.
fn dual_vector(
    ctx: &Ctx,
    sys: &ExactSystem,
    q: &PoleQuery<'_>,
) -> Result<(PStar, std::collections::HashMap<u32, RF>), String> {
    let eb = exact_basis(sys, q)?;
    let t = eb.target_col.ok_or("target absent")?;
    let get = |e: &[(u32, ZP)], col: u32| e.iter().find(|x| x.0 == col).map(|x| x.1.clone());
    let shift_rf = |p: ZP, sh: u32| -> RF {
        let den = ctx.poly(&[(sh as u16, Integer::one())]);
        ctx.rf(p, den)
    };
    let mut y: std::collections::HashMap<u32, RF> = std::collections::HashMap::new();
    for (pc, s, c, sh) in eb.rows.iter().rev() {
        // y_pc = -((h)_K + sum_{later pivots l} y_l h_l) / h_pc
        let mut acc = get(c, t).map_or_else(|| ctx.zero(), |k| shift_rf(k, *sh));
        for (col, val) in s {
            if col == pc {
                continue;
            }
            if let Some(yl) = y.get(col) {
                let term = ctx
                    .field
                    .mul(yl, &ctx.rf(val.clone(), ctx.poly(&[(0, Integer::one())])));
                acc = ctx.field.add(&acc, &term);
            }
        }
        let hp = ctx.rf(
            get(s, *pc).ok_or("pivot entry")?,
            ctx.poly(&[(0, Integer::one())]),
        );
        let ypc = ctx.field.neg(&ctx.field.div(&acc, &hp));
        y.insert(*pc, ypc);
    }
    // the pivots are columns of interest (phase 2): y lives there (y_K = 1 implied)
    y.retain(|c, _| eb.interest_cols.contains(c));
    Ok((eb.p_star, y))
}

/// Run the check mode on a result of the oracle (exact verdicts only). Dense: refuses above `max_entries`.
pub fn check_result(
    sys: &ExactSystem,
    q: &PoleQuery<'_>,
    res: &PoleOrderResult,
    max_entries: usize,
) -> Result<CheckReport, String> {
    let ctx = Ctx::new();
    let dual = match res.p_star {
        Some(PStar::Order(p)) if p > 0 => {
            let (eb_p, y) = dual_vector(&ctx, sys, q)?;
            Some((eb_p == PStar::Order(p), y))
        }
        _ => None,
    };
    check_with(sys, q, res.p_star, res.relation.as_ref(), dual, max_entries)
}

/// The checks of [`check_result`] with the dual vector GIVEN (by system column; `bool` = its source agrees with the
/// claim): the oracle's own basis, or a certificate file's vector ([`check_certificate`]).
fn check_with(
    sys: &ExactSystem,
    q: &PoleQuery<'_>,
    claimed: Option<PStar>,
    relation: Option<&rustred_campaign_core::tadic::Relation>,
    dual: Option<(bool, std::collections::HashMap<u32, RF>)>,
    max_entries: usize,
) -> Result<CheckReport, String> {
    let ncol = sys.columns.len();
    if sys.rows.len() * ncol > max_entries {
        return Err(format!(
            "check mode: {} x {ncol} above max_entries {max_entries}",
            sys.rows.len()
        ));
    }
    let ctx = Ctx {
        vars: t_vars(),
        field: RationalPolynomialField::new(Z),
    };
    // column order: upper, interest, carried (target last)
    let interest: Option<std::collections::HashSet<&Vec<i64>>> = match &q.interest {
        Interest::AllLower => None,
        Interest::Keys(k) => Some(k.iter().collect()),
    };
    let (mut up, mut int, mut car) = (Vec::new(), Vec::new(), Vec::new());
    let mut tcol = None;
    for (i, k) in sys.columns.iter().enumerate() {
        if k.as_slice() == q.target {
            tcol = Some(i);
        } else if (q.lower)(k) {
            if interest.as_ref().is_none_or(|s| s.contains(k)) {
                int.push(i)
            } else {
                car.push(i)
            }
        } else {
            up.push(i);
        }
    }
    let mut cols: Vec<usize> = up.iter().chain(&int).chain(&car).copied().collect();
    if let Some(t) = tcol {
        cols.push(t);
    }
    let pos_of = {
        let mut v = vec![usize::MAX; ncol];
        for (p, &c) in cols.iter().enumerate() {
            v[c] = p;
        }
        v
    };
    let (dn, dd) = (Integer::from(q.d0.n), Integer::from(q.d0.d));
    let entry = |a: i128, b: i128| -> RF {
        // (a + b d) * dd with d = t + dn/dd: (a dd + b dn) + (b dd) t
        let (a, b) = (Integer::from(a), Integer::from(b));
        let c0 = &(&a * &dd) + &(&b * &dn);
        let c1 = &b * &dd;
        ctx.rf(
            ctx.poly(&[(0, c0), (1, c1)]),
            ctx.poly(&[(0, Integer::one())]),
        )
    };
    let mut data: Vec<Vec<RF>> = Vec::with_capacity(sys.rows.len() + 1);
    for r in &sys.rows {
        let mut row = vec![ctx.zero(); ncol];
        for &(c, a, b) in r {
            row[pos_of[c as usize]] = entry(a, b);
        }
        data.push(row);
    }
    let base =
        Matrix::from_nested_vec(data.clone(), ctx.field.clone()).map_err(|e| e.to_string())?;
    let mut red = base.clone();
    let rank = red.row_reduce(ncol as u32);
    let mut rep = CheckReport {
        rank_rows: rank,
        ..Default::default()
    };
    let nu = up.len();
    // W_0 = reduced rows whose first nonzero column is not upper; V = projection on interest + target
    let mut v_rows: Vec<Vec<RF>> = Vec::new();
    for i in 0..red.nrows() {
        let row: Vec<RF> = (0..ncol)
            .map(|j| red[(i as u32, j as u32)].clone())
            .collect();
        let Some(first) = row.iter().position(|x| !x.numerator.is_zero()) else {
            continue;
        };
        if first < nu {
            continue;
        }
        let mut v: Vec<RF> = int.iter().map(|&c| row[pos_of[c]].clone()).collect();
        v.push(tcol.map_or_else(|| ctx.zero(), |t| row[pos_of[t]].clone()));
        v_rows.push(v);
    }
    rep.dim_w0 = v_rows.len();
    let mut ok = true;
    // upper bound: relation membership + recomputed order
    if let Some(rel) = relation {
        let mut w = vec![ctx.zero(); ncol];
        if let Some(t) = tcol {
            w[pos_of[t]] = ctx.one();
        }
        let mut order = 0i64;
        for term in &rel.terms {
            let c = sys
                .columns
                .iter()
                .position(|k| *k == term.key)
                .ok_or("relation key not in the system")?;
            let co = parse_in_t_with(&ctx, &term.coefficient, &q.d0)?;
            if term.of_interest {
                order = order.max(-val_rf(&co));
            }
            w[pos_of[c]] = ctx.field.neg(&co);
        }
        let mut aug = data.clone();
        aug.push(w);
        let r2 = Matrix::from_nested_vec(aug, ctx.field.clone())
            .map_err(|e| e.to_string())?
            .rank();
        rep.relation_in_span = Some(r2 == rank);
        rep.relation_order = Some(order.max(0) as u32);
        ok &= r2 == rank;
        ok &= claimed.is_some_and(|p| p == PStar::Order(order.max(0) as u32));
    }
    match claimed {
        Some(PStar::NoRelation) => {
            let all_zero = v_rows
                .iter()
                .all(|v| v.last().is_none_or(|x| x.numerator.is_zero()));
            rep.no_relation_ok = Some(all_zero);
            ok &= all_zero;
        }
        Some(PStar::Order(p)) if p > 0 => {
            // the dual certificate (lower bound): without one the claim p > 0 is not certified
            let Some((agrees, y)) = dual else {
                rep.dual_ok = Some(false);
                rep.consistent = false;
                return Ok(rep);
            };
            ok &= agrees;
            // check against numerica's V (columns: interest order `int`, then target)
            let mut dual_ok = true;
            let mut bound = i64::MAX;
            for (k, &c) in int.iter().enumerate() {
                let _ = k;
                if let Some(yc) = y.get(&(c as u32)) {
                    bound = bound.min(val_rf(yc));
                }
            }
            for v in &v_rows {
                let mut s = v.last().cloned().unwrap_or_else(|| ctx.zero());
                for (k, &c) in int.iter().enumerate() {
                    if let Some(yc) = y.get(&(c as u32)) {
                        s = ctx.field.add(&s, &ctx.field.mul(yc, &v[k]));
                    }
                }
                dual_ok &= s.numerator.is_zero();
            }
            let b = if bound == i64::MAX {
                u32::MAX
            } else {
                bound.max(0) as u32
            };
            rep.dual_bound = Some(b);
            dual_ok &= b >= p;
            rep.dual_ok = Some(dual_ok);
            ok &= dual_ok;
        }
        _ => {}
    }
    rep.consistent = ok;
    Ok(rep)
}

/// Ansatz upper bound: the smallest q <= max_q such that some combination sum_i lambda_i r_i with lambda_i Laurent
/// polynomials in t with exponents in [lo, hi] (rational coefficients) is admissible-type with v(w_K) = q exactly and
/// w regular on the columns of interest. `None`: none found in the ansatz. Exact (numerica over Q); exponential in
/// nothing but cubic in rows * (hi - lo + 1): small systems only.
pub fn brute_force_upper(
    sys: &ExactSystem,
    q: &PoleQuery<'_>,
    lo: i32,
    hi: i32,
    max_q: u32,
) -> Option<u32> {
    let interest: Option<std::collections::HashSet<&Vec<i64>>> = match &q.interest {
        Interest::AllLower => None,
        Interest::Keys(k) => Some(k.iter().collect()),
    };
    let tcol = sys.columns.iter().position(|k| k.as_slice() == q.target)?;
    let m = sys.rows.len();
    let w = (hi - lo + 1) as usize;
    let nun = m * w;
    let (dn, dd) = (q.d0.n, q.d0.d);
    // entry of row i at column c: alpha + beta t
    let mut ent: Vec<std::collections::HashMap<usize, (Rational, Rational)>> =
        vec![Default::default(); m];
    for (i, r) in sys.rows.iter().enumerate() {
        for &(c, a, b) in r {
            let al = Rational::new(Integer::from(a * dd + b * dn), Integer::from(dd));
            let be = Rational::from(Integer::from(b));
            ent[i].insert(c as usize, (al, be));
        }
    }
    for qq in 0..=max_q {
        let mut rows: Vec<Vec<Rational>> = Vec::new();
        for c in 0..sys.columns.len() {
            let key = &sys.columns[c];
            let kind = if c == tcol {
                2
            } else if (q.lower)(key) {
                if interest.as_ref().is_none_or(|s| s.contains(key)) {
                    1
                } else {
                    3
                }
            } else {
                0
            };
            if kind == 3 {
                continue;
            }
            for e in lo..=hi + 1 {
                // coefficient of t^e in w_c
                let constrained = match kind {
                    0 => Some(Rational::zero()),
                    1 => (e < 0).then(Rational::zero),
                    _ => {
                        if e < qq as i32 {
                            Some(Rational::zero())
                        } else if e == qq as i32 {
                            Some(Rational::one())
                        } else {
                            None
                        }
                    }
                };
                let Some(rhs) = constrained else { continue };
                let mut row = vec![Rational::zero(); nun + 1];
                let mut any = false;
                for (i, en) in ent.iter().enumerate() {
                    if let Some((al, be)) = en.get(&c) {
                        let j0 = e - lo;
                        if (0..w as i32).contains(&j0) {
                            row[i * w + j0 as usize] = &row[i * w + j0 as usize] + al;
                            any |= !al.is_zero();
                        }
                        let j1 = e - 1 - lo;
                        if (0..w as i32).contains(&j1) {
                            row[i * w + j1 as usize] = &row[i * w + j1 as usize] + be;
                            any |= !be.is_zero();
                        }
                    }
                }
                if !any && rhs.is_zero() {
                    continue;
                }
                row[nun] = rhs;
                rows.push(row);
            }
        }
        if rows.is_empty() {
            continue;
        }
        let a: Vec<Vec<Rational>> = rows.iter().map(|r| r[..nun].to_vec()).collect();
        let ra = Matrix::from_nested_vec(a, Q).ok()?.rank();
        let rb = Matrix::from_nested_vec(rows, Q).ok()?.rank();
        if ra == rb {
            return Some(qq);
        }
    }
    None
}

/// Schema of the self-contained certificate file of a p* claim.
pub const CERTIFICATE_SCHEMA: &str = "rustred.campaign.tadic-certificate.v1";

/// A SELF-CONTAINED certificate of one p* claim (lane polefix, audit r2-poles F12): the system (rows `sum (a + b d)
/// I(col) = 0`, integers as text), the partition of its columns (target, upper = must be eliminated, interest =
/// columns of interest; every other column is carried), d0, the claim, the UPPER-bound certificate (an exact relation
/// `I(K) = sum c_j I(k_j)`, coefficients in d) and the LOWER-bound certificate (the dual vector y on the columns of
/// interest, y_K = 1, entries in d). [`check_certificate`] verifies both from this object alone (no oracle, no other
/// input): the relation lies in the row space and has the claimed order; y is orthogonal to V and `min v(y_S)` >= the
/// claim; "no relation" = every V vector has K-coordinate 0.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Certificate {
    pub schema: String,
    pub arity: usize,
    pub target: Vec<i64>,
    pub d0: String,
    /// Rows: (key, a, b) with integer texts.
    pub rows: Vec<Vec<(Vec<i64>, String, String)>>,
    pub upper: Vec<Vec<i64>>,
    pub interest: Vec<Vec<i64>>,
    /// `{"order": p}` or `"no_relation"`.
    pub p_star: serde_json::Value,
    /// Upper bound: (key, coefficient in d, of interest).
    pub relation: Option<Vec<(Vec<i64>, String, bool)>>,
    /// Lower bound: (key of interest, y entry in d); absent when the claim needs none (p* = 0, no relation).
    pub dual: Option<Vec<(Vec<i64>, String)>>,
    /// The system's identity as the oracle named it (information).
    pub system: serde_json::Value,
}

fn pstar_json(p: PStar) -> serde_json::Value {
    match p {
        PStar::Order(o) => serde_json::json!({"order": o}),
        PStar::NoRelation => serde_json::json!("no_relation"),
    }
}

fn pstar_of(v: &serde_json::Value) -> Result<PStar, String> {
    if v.as_str() == Some("no_relation") {
        return Ok(PStar::NoRelation);
    }
    v["order"]
        .as_u64()
        .and_then(|o| u32::try_from(o).ok())
        .map(PStar::Order)
        .ok_or_else(|| format!("p_star {v} is neither {{order: p}} nor \"no_relation\""))
}

/// EXPORT the certificate of an exact result: the relation from the result, the dual vector from the oracle's exact
/// basis (when p* > 0). Refused for a result without an exact p*.
pub fn certificate(
    sys: &ExactSystem,
    q: &PoleQuery<'_>,
    res: &PoleOrderResult,
) -> Result<Certificate, String> {
    let p = res.p_star.ok_or("no p* to certify (undecided result)")?;
    let interest_of = |k: &Vec<i64>| match &q.interest {
        Interest::AllLower => true,
        Interest::Keys(ks) => ks.contains(k),
    };
    let mut upper = vec![];
    let mut interest = vec![];
    for k in &sys.columns {
        if k.as_slice() == q.target {
            continue;
        }
        if !(q.lower)(k) {
            upper.push(k.clone());
        } else if interest_of(k) {
            interest.push(k.clone());
        }
    }
    let rows = sys
        .rows
        .iter()
        .map(|r| {
            r.iter()
                .map(|&(c, a, b)| {
                    (
                        sys.columns[c as usize].clone(),
                        a.to_string(),
                        b.to_string(),
                    )
                })
                .collect()
        })
        .collect();
    let dual = match p {
        PStar::Order(o) if o > 0 => {
            let ctx = Ctx::new();
            let (eb_p, y) = dual_vector(&ctx, sys, q)?;
            if eb_p != p {
                return Err(format!(
                    "the exact basis gives {eb_p:?}, the result claims {p:?}"
                ));
            }
            let mut v: Vec<(Vec<i64>, String)> = y
                .iter()
                .filter(|(c, _)| interest_of(&sys.columns[**c as usize]))
                .map(|(c, r)| {
                    (
                        sys.columns[*c as usize].clone(),
                        super::ratio_text_in_d(&r.numerator, &r.denominator, &q.d0),
                    )
                })
                .collect();
            v.sort();
            Some(v)
        }
        _ => None,
    };
    Ok(Certificate {
        schema: CERTIFICATE_SCHEMA.into(),
        arity: sys.arity,
        target: q.target.to_vec(),
        d0: q.d0.text(),
        rows,
        upper,
        interest,
        p_star: pstar_json(p),
        relation: res.relation.as_ref().map(|r| {
            r.terms
                .iter()
                .map(|t| (t.key.clone(), t.coefficient.clone(), t.of_interest))
                .collect()
        }),
        dual,
        system: serde_json::to_value(&res.system).unwrap_or(serde_json::Value::Null),
    })
}

/// VERIFY a certificate from its content ALONE (numerica / Symbolica route of the check mode; no oracle).
pub fn check_certificate(cert: &Certificate, max_entries: usize) -> Result<CheckReport, String> {
    use rustred_campaign_core::tadic::{Arith, Profile, Rat, RelTerm, Relation, TadicBudget};
    if cert.schema != CERTIFICATE_SCHEMA {
        return Err(format!(
            "schema {:?}, expected {CERTIFICATE_SCHEMA}",
            cert.schema
        ));
    }
    let int = |t: &str| {
        t.trim()
            .parse::<i128>()
            .map_err(|_| format!("integer {t:?}"))
    };
    let mut rows = Vec::with_capacity(cert.rows.len());
    for r in &cert.rows {
        let mut row = Vec::with_capacity(r.len());
        for (k, a, b) in r {
            if k.len() != cert.arity {
                return Err(format!("key {k:?} of arity {}", k.len()));
            }
            row.push((k.clone(), int(a)?, int(b)?));
        }
        rows.push(row);
    }
    let sys = ExactSystem::from_rows(cert.arity, &rows, "certificate");
    let d0 = Rat::parse(&cert.d0).ok_or_else(|| format!("d0 {:?}", cert.d0))?;
    let upper: std::collections::HashSet<&Vec<i64>> = cert.upper.iter().collect();
    let target = cert.target.clone();
    let lower = |k: &[i64]| k != target.as_slice() && !upper.contains(&k.to_vec());
    let q = PoleQuery {
        target: &cert.target,
        lower: &lower,
        d0,
        interest: Interest::Keys(cert.interest.clone()),
        arith: Arith::Exact,
        budget: TadicBudget::default(),
        want_relation: false,
    };
    let claimed = pstar_of(&cert.p_star)?;
    let relation = cert.relation.as_ref().map(|ts| Relation {
        terms: ts
            .iter()
            .map(|(k, c, i)| RelTerm {
                key: k.clone(),
                coefficient: c.clone(),
                pole_order: 0,
                of_interest: *i,
            })
            .collect(),
        profile: Profile::default(),
    });
    let ctx = Ctx::new();
    let dual = match &cert.dual {
        None => None,
        Some(ys) => {
            let mut y = std::collections::HashMap::new();
            for (k, text) in ys {
                let c = sys
                    .columns
                    .iter()
                    .position(|x| x == k)
                    .ok_or_else(|| format!("dual key {k:?} not a column of the system"))?;
                y.insert(c as u32, parse_in_t_with(&ctx, text, &d0)?);
            }
            Some((true, y))
        }
    };
    check_with(
        &sys,
        &q,
        Some(claimed),
        relation.as_ref(),
        dual,
        max_entries,
    )
}

/// Write a certificate file (canonical JSON + "\n").
pub fn write_certificate(path: &std::path::Path, cert: &Certificate) -> Result<(), String> {
    let v = serde_json::to_value(cert).map_err(|e| e.to_string())?;
    let mut t = rustred_campaign_core::schema::canon::canonical_json(&v, false)?;
    t.push('\n');
    std::fs::write(path, t).map_err(|e| format!("{}: {e}", path.display()))
}

/// Read a certificate file.
pub fn read_certificate(path: &std::path::Path) -> Result<Certificate, String> {
    let b = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_slice(&b).map_err(|e| format!("{}: {e}", path.display()))
}
