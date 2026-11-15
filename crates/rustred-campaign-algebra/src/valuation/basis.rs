//! B2: THE CHECK OF A TERMINAL SET AT d0 THROUGH ITS MASTER MAP (lane poles; pole-diagnostics.md 1.5, 3 route X,
//! addenda 3 and 5; POLES_OBJECTIVE.md 2(B)). API for lanes regbasis and epsdepth.
//!
//! Input: an exact map `T_i = sum_j M_ij(d) R_j` (modulo subsectors) of a terminal set T onto a reference set R, entries
//! rational functions of d (text). Output ([`MapCheck`]), all exact (Symbolica `Matrix` over the field Q(d) =
//! `RationalPolynomialField<Z>`: `det`, `inv`, `rank`; over Q at d0: `rank`, `row_reduce`):
//! - the order at d0 of every entry of M and of M^-1;
//! - `det M` and the ORDER OF ITS ZERO at d0 (addendum 3: the order, not only the rank loss);
//! - the generic rank, the rank of M(d0) (when M is regular at d0) and the LEFT NULL DIRECTIONS of M(d0): primitive
//!   integer vectors n with n.T = O(d - d0) (first non-zero entry positive);
//! - the verdict on T RELATIVE TO R: a box key K = x.R = x.M^-1.T, so with R regular at d0 the poles K can have on T are
//!   those of M^-1: `regular` iff every entry of M^-1 is regular at d0, `degenerate` with order = the largest pole of
//!   M^-1 otherwise; `unresolved` when M is not invertible over Q(d) or the input does not parse.
//!   The verdict is about T only as far as R is regular (the reference's regularity on the full band is lane
//!   regbasis' statement); the record names R.
//!
//! WHY NOT A SAMPLE AT d0: the node-local system itself can lose rank at d = d0 (T162: 4 more free columns at d = 4
//! exactly, helper P7), so rank_T at a d0 sample (route E, `rr_node symmasters --dims 4,..`) is a screen that proves
//! nothing alone (TRICKS 16 / 17 in their basis form); the verdict of record is the exact map's.
//!
//! Text forms: Symbolica's parser; `**` (sympy printing, as in the helper's files) is read as `^`.

use super::{D0, Valuation, ValuationError, multiplicity, variable_map};
use serde_json::{Value, json};
use symbolica::domains::integer::{Integer, Z};
use symbolica::domains::rational::{Q, Rational};
use symbolica::domains::rational_polynomial::{RationalPolynomial, RationalPolynomialField};
use symbolica::domains::{Field, RingOps};
use symbolica::prelude::AtomCore;
use symbolica::printer::PrintOptions;
use symbolica::tensors::matrix::Matrix;
use symbolica::try_parse;

type RatFn = RationalPolynomial<symbolica::domains::integer::IntegerRing, u16>;

/// An exact master map `T = M R`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterMap {
    /// Labels / key texts of T (rows).
    pub rows: Vec<String>,
    /// Labels / key texts of R (columns).
    pub cols: Vec<String>,
    /// `entries[i][j]` = M_ij as text (a rational function of d).
    pub entries: Vec<Vec<String>>,
}

fn strs(v: &Value) -> Option<Vec<String>> {
    v.as_array()?
        .iter()
        .map(|x| x.as_str().map(str::to_string))
        .collect()
}

fn text_matrix(v: &Value, member: Option<&str>) -> Option<Vec<Vec<String>>> {
    v.as_array()?
        .iter()
        .map(|r| {
            r.as_array()?
                .iter()
                .map(|x| match member {
                    None => x.as_str().map(str::to_string),
                    Some(m) => x[m].as_str().map(str::to_string),
                })
                .collect::<Option<Vec<String>>>()
        })
        .collect()
}

/// THE READER of a master map (API for lanes regbasis / epsdepth). Accepted forms:
/// - generic `{"rows": [..], "cols": [..], "entries": [[text, ..], ..]}` (`T_i = sum_j entries[i][j] R_j`);
/// - a regular-terminals record `{"T": {"full" | "keys": [..]}, "R": {...}, "M": [[{"exact": text, ..}, ..], ..]}`;
/// - a reconstruction-list node `{"A": {"rows_published_masters", "cols_regular_basis", "matrix"}}` (published masters
///   P = A L: the need on published masters).
pub fn read_master_map(v: &Value) -> Result<MasterMap, String> {
    if let (Some(rows), Some(cols), Some(entries)) = (
        strs(&v["rows"]),
        strs(&v["cols"]),
        text_matrix(&v["entries"], None),
    ) {
        return Ok(MasterMap {
            rows,
            cols,
            entries,
        });
    }
    if v["M"].is_array() {
        let side = |x: &Value| strs(&x["full"]).or_else(|| strs(&x["keys"]));
        let entries =
            text_matrix(&v["M"], Some("exact")).ok_or("M entries are not {\"exact\": text}")?;
        let rows = side(&v["T"]).ok_or("no T.full / T.keys")?;
        let cols = side(&v["R"]).ok_or("no R.full / R.keys")?;
        return Ok(MasterMap {
            rows,
            cols,
            entries,
        });
    }
    if v["A"].is_object() {
        let a = &v["A"];
        return Ok(MasterMap {
            rows: strs(&a["rows_published_masters"]).ok_or("A.rows_published_masters")?,
            cols: strs(&a["cols_regular_basis"]).ok_or("A.cols_regular_basis")?,
            entries: text_matrix(&a["matrix"], None).ok_or("A.matrix")?,
        });
    }
    Err("not a master map (generic rows/cols/entries, T/R/M, or A)".into())
}

/// The exact check of a master map at d0.
#[derive(Debug, Clone, PartialEq)]
pub struct MapCheck {
    pub d0: String,
    pub n_rows: usize,
    pub n_cols: usize,
    pub generic_rank: usize,
    /// `det M` (square maps) as text, and its order at d0 (> 0: a zero of that order).
    pub det: Option<String>,
    pub det_order: Option<Valuation>,
    /// Orders of the entries of M (`Zero` = identically 0).
    pub entry_orders: Vec<Vec<Valuation>>,
    /// M^-1 (square, invertible maps) and the orders of its entries.
    pub inverse: Option<Vec<Vec<String>>>,
    pub inverse_orders: Option<Vec<Vec<Valuation>>>,
    /// Rank of M(d0) (None when an entry of M has a pole at d0).
    pub rank_at_d0: Option<usize>,
    /// Left null directions of M(d0) on the rows (primitive integer vectors as texts).
    pub null_directions: Vec<Vec<String>>,
    /// `regular` / `degenerate` / `unresolved`.
    pub verdict: String,
    /// Degenerate: the largest pole order of M^-1 at d0 (the extra eps depth on T of a key regular on R).
    pub order: Option<u64>,
    pub reason: Option<String>,
}

fn fix_text(t: &str) -> String {
    t.replace("**", "^")
}

fn parse_entry(text: &str) -> Result<RatFn, ValuationError> {
    let vars = variable_map(0)?;
    let atom = try_parse!(&fix_text(text), default_namespace = "rustred").map_err(|e| {
        ValuationError::NotRational {
            reason: e.to_string(),
        }
    })?;
    let rp: RatFn = atom
        .as_view()
        .try_to_rational_polynomial::<_, _, u16>(&Q, &Z, Some(vars.clone()))
        .map_err(|e| ValuationError::NotRational {
            reason: e.to_string(),
        })?;
    if rp.numerator.get_vars_ref().len() != vars.len() {
        return Err(ValuationError::NotRationalInD {
            symbols: vec![format!("in {text:?}")],
        });
    }
    Ok(rp)
}

fn order_of(r: &RatFn, d0: &D0) -> Valuation {
    if r.numerator.is_zero() {
        return Valuation::Zero;
    }
    let (nm, _) = multiplicity(&r.numerator, 0, d0.value());
    let (dm, _) = multiplicity(&r.denominator, 0, d0.value());
    Valuation::Order(nm as i64 - dm as i64)
}

fn print(r: &RatFn) -> String {
    let opts = PrintOptions {
        hide_all_namespaces: true,
        ..PrintOptions::new()
    };
    let n = r
        .numerator
        .to_expression()
        .printer(opts.clone())
        .to_string();
    let d = r.denominator.to_expression().printer(opts).to_string();
    if d == "1" { n } else { format!("({n})/({d})") }
}

/// Value at d0 (None at a pole).
fn value_at(r: &RatFn, d0: &D0) -> Option<Rational> {
    let to_q = |p: &symbolica::poly::polynomial::MultivariatePolynomial<
        symbolica::domains::integer::IntegerRing,
        u16,
    >| {
        p.map_coeff(|c| Rational::from(c), Q)
            .replace_all(std::slice::from_ref(d0.value()))
    };
    let den = to_q(&r.denominator);
    if den.is_zero() {
        return None;
    }
    Some(Q.div(&to_q(&r.numerator), &den))
}

/// Primitive integer form of a rational vector (first non-zero entry positive).
fn primitive(v: &[Rational]) -> Vec<String> {
    let mut l = Integer::one();
    for x in v {
        l = l.lcm(&x.denominator());
    }
    let ints: Vec<Integer> = v
        .iter()
        .map(|x| x.numerator() * &(&l / &x.denominator()))
        .collect();
    let mut g = Integer::zero();
    for x in &ints {
        g = g.gcd(x);
    }
    if g.is_zero() {
        return ints.iter().map(|x| x.to_string()).collect();
    }
    let sign = ints
        .iter()
        .find(|x| !x.is_zero())
        .map_or(false, |x| x.is_negative());
    ints.iter()
        .map(|x| {
            let y = x / &g;
            if sign {
                (-y).to_string()
            } else {
                y.to_string()
            }
        })
        .collect()
}

/// Left null space of a rational matrix `a` (rows x cols): vectors n (length rows) with n.a = 0, as primitive integer
/// vectors, via Symbolica's RREF of the transpose.
fn left_null(a: &[Vec<Rational>]) -> Vec<Vec<String>> {
    let rows = a.len();
    if rows == 0 {
        return vec![];
    }
    let cols = a[0].len();
    // B = a^T: cols x rows; solve B n = 0
    let bt: Vec<Vec<Rational>> = (0..cols)
        .map(|j| (0..rows).map(|i| a[i][j].clone()).collect())
        .collect();
    let mut m = match Matrix::from_nested_vec(bt, Q) {
        Ok(m) => m,
        Err(_) => return vec![],
    };
    m.row_reduce(rows as u32);
    let mut pivots: Vec<(usize, usize)> = vec![]; // (row, col)
    for r in 0..cols {
        if let Some(c) = (0..rows).find(|&c| !m[(r as u32, c as u32)].is_zero()) {
            pivots.push((r, c));
        }
    }
    let piv_cols: Vec<usize> = pivots.iter().map(|p| p.1).collect();
    let mut out = vec![];
    for f in (0..rows).filter(|c| !piv_cols.contains(c)) {
        let mut n = vec![Rational::zero(); rows];
        n[f] = Rational::one();
        for &(r, c) in &pivots {
            // pivot row: m[r][c] n_c + sum_free m[r][f] n_f = 0
            let p = &m[(r as u32, c as u32)];
            let x = &m[(r as u32, f as u32)];
            n[c] = Q.neg(&Q.div(x, p));
        }
        out.push(primitive(&n));
    }
    out
}

/// Check a master map at d0 (module header).
pub fn check_master_map(map: &MasterMap, d0: &D0) -> MapCheck {
    let unresolved = |reason: String| MapCheck {
        d0: d0.text().to_string(),
        n_rows: map.rows.len(),
        n_cols: map.cols.len(),
        generic_rank: 0,
        det: None,
        det_order: None,
        entry_orders: vec![],
        inverse: None,
        inverse_orders: None,
        rank_at_d0: None,
        null_directions: vec![],
        verdict: "unresolved".into(),
        order: None,
        reason: Some(reason),
    };
    if map.entries.len() != map.rows.len() || map.entries.iter().any(|r| r.len() != map.cols.len())
    {
        return unresolved("entries do not match rows x cols".into());
    }
    let mut ents: Vec<Vec<RatFn>> = Vec::with_capacity(map.rows.len());
    for (i, row) in map.entries.iter().enumerate() {
        let mut out = Vec::with_capacity(row.len());
        for (j, t) in row.iter().enumerate() {
            match parse_entry(t) {
                Ok(r) => out.push(r),
                Err(e) => return unresolved(format!("entry ({i},{j}): {e}")),
            }
        }
        ents.push(out);
    }
    let entry_orders: Vec<Vec<Valuation>> = ents
        .iter()
        .map(|r| r.iter().map(|x| order_of(x, d0)).collect())
        .collect();
    let field = RationalPolynomialField::new(Z);
    let m = match Matrix::from_nested_vec(ents.clone(), field) {
        Ok(m) => m,
        Err(e) => return unresolved(format!("matrix: {e}")),
    };
    let generic_rank = m.rank();
    // M(d0) when every entry is regular there
    let at0: Option<Vec<Vec<Rational>>> = ents
        .iter()
        .map(|r| {
            r.iter()
                .map(|x| value_at(x, d0))
                .collect::<Option<Vec<_>>>()
        })
        .collect();
    let rank_at_d0 = at0
        .as_ref()
        .and_then(|a| Matrix::from_nested_vec(a.clone(), Q).ok())
        .map(|mm| mm.rank());
    let null_directions = at0.as_ref().map(|a| left_null(a)).unwrap_or_default();
    let square = map.rows.len() == map.cols.len();
    let (det, det_order, inverse, inverse_orders) = if square && generic_rank == map.rows.len() {
        let det = m.det().ok();
        let inv = m.inv().ok();
        let inv_rows: Option<Vec<Vec<RatFn>>> = inv.map(|iv| {
            let n = map.rows.len();
            (0..n)
                .map(|i| (0..n).map(|j| iv[(i as u32, j as u32)].clone()).collect())
                .collect()
        });
        (
            det.as_ref().map(print),
            det.as_ref().map(|x| order_of(x, d0)),
            inv_rows.as_ref().map(|r| {
                r.iter()
                    .map(|row| row.iter().map(print).collect())
                    .collect()
            }),
            inv_rows.as_ref().map(|r| {
                r.iter()
                    .map(|row| {
                        row.iter()
                            .map(|x| order_of(x, d0))
                            .collect::<Vec<Valuation>>()
                    })
                    .collect::<Vec<_>>()
            }),
        )
    } else {
        (None, None, None, None)
    };
    let (verdict, order, reason) = match &inverse_orders {
        None => (
            "unresolved".to_string(),
            None,
            Some(format!(
                "M is not square and invertible over Q(d) (rows {}, cols {}, generic rank {generic_rank})",
                map.rows.len(),
                map.cols.len()
            )),
        ),
        Some(io) => {
            let worst = io
                .iter()
                .flatten()
                .map(Valuation::pole_order)
                .max()
                .unwrap_or(0);
            if worst == 0 {
                ("regular".into(), Some(0), None)
            } else {
                ("degenerate".into(), Some(worst), None)
            }
        }
    };
    MapCheck {
        d0: d0.text().to_string(),
        n_rows: map.rows.len(),
        n_cols: map.cols.len(),
        generic_rank,
        det,
        det_order,
        entry_orders,
        inverse,
        inverse_orders,
        rank_at_d0,
        null_directions,
        verdict,
        order,
        reason,
    }
}

fn orders_json(o: &[Vec<Valuation>]) -> Value {
    json!(
        o.iter()
            .map(|r| r
                .iter()
                .map(|v| match v {
                    Valuation::Zero => Value::Null,
                    Valuation::Order(k) => json!(k),
                })
                .collect::<Vec<_>>())
            .collect::<Vec<_>>()
    )
}

impl MapCheck {
    /// The check as JSON (orders: integer = order at d0, + zero / - pole, null = identically zero entry).
    pub fn to_json(&self) -> Value {
        json!({
            "d0": self.d0, "rows": self.n_rows, "cols": self.n_cols, "generic_rank": self.generic_rank,
            "det": self.det, "det_order_at_d0": self.det_order.map(|v| match v { Valuation::Zero => Value::Null, Valuation::Order(k) => json!(k) }),
            "M_orders": orders_json(&self.entry_orders),
            "M_inverse": self.inverse, "M_inverse_orders": self.inverse_orders.as_deref().map(orders_json),
            "rank_at_d0": self.rank_at_d0, "null_directions_at_d0": self.null_directions,
            "verdict": self.verdict, "order": self.order, "reason": self.reason,
        })
    }
}
