//! Exact A, M = A^-1, det M over Q(d) (numerica `Matrix` over Symbolica's `RationalPolynomialField`), and the order at
//! d0 of every entry by lane poles' exact valuation (`valuation::order_of_text`, the one valuation of the tree).
//!
//! Convention: `R = A T` (row j of A = the exact coefficients of R_j on the terminals), `T = M R`, `M = A^-1`.

use symbolica::domains::integer::Z;
use symbolica::domains::rational::Rational;
use symbolica::domains::rational_polynomial::RationalPolynomialField;
use symbolica::tensors::matrix::Matrix;

use crate::valuation::{D0, Valuation, order_of_text};

use super::series::{RF, parse_rf, rf_text, rf_valuation};

/// The exact map between the closure's terminals and a regular set.
#[derive(Clone, Debug)]
pub struct ExactMap {
    pub a: Vec<Vec<RF>>,
    pub m: Vec<Vec<RF>>,
    pub det_m: RF,
    pub a_text: Vec<Vec<String>>,
    pub m_text: Vec<Vec<String>>,
    pub det_m_text: String,
    /// Orders at d0 (lane poles' valuation): `Some(v)` (> 0 zero, < 0 pole), `None` identically zero.
    pub m_orders: Vec<Vec<Option<i64>>>,
    pub a_orders: Vec<Vec<Option<i64>>>,
    pub det_m_order: Option<i64>,
}

fn order(text: &str, d0: &D0) -> Result<Option<i64>, String> {
    match order_of_text(text, d0).map_err(|e| format!("valuation of {text:?}: {e}"))? {
        Valuation::Zero => Ok(None),
        Valuation::Order(v) => Ok(Some(v)),
    }
}

/// Build the exact map from the exact rows of A (texts). Err when A is singular over Q(d) or a text does not parse.
pub fn exact_map(a_rows: &[Vec<String>], d0: &D0) -> Result<ExactMap, String> {
    let n = a_rows.len();
    if n == 0 || a_rows.iter().any(|r| r.len() != n) {
        return Err(format!("A must be square and non-empty, got {n} rows"));
    }
    let field = RationalPolynomialField::new(Z);
    let mut data = Vec::with_capacity(n * n);
    let mut a = Vec::with_capacity(n);
    for r in a_rows {
        let row: Vec<RF> = r.iter().map(|t| parse_rf(t)).collect::<Result<_, _>>()?;
        data.extend(row.iter().cloned());
        a.push(row);
    }
    // all entries must share one variable map (parse_rf uses the map [d]); constants too
    let mat = Matrix::from_linear(data, n as u32, n as u32, field.clone())?;
    let inv = mat
        .inv()
        .map_err(|e| format!("A is singular over Q(d): {e:?}"))?;
    let det_a = mat.det().map_err(|e| format!("det A: {e:?}"))?;
    let m: Vec<Vec<RF>> = (0..n)
        .map(|i| (0..n).map(|j| inv[(i as u32, j as u32)].clone()).collect())
        .collect();
    let det_m = det_a.inv();
    let a_text: Vec<Vec<String>> = a.iter().map(|r| r.iter().map(rf_text).collect()).collect();
    let m_text: Vec<Vec<String>> = m.iter().map(|r| r.iter().map(rf_text).collect()).collect();
    let det_m_text = rf_text(&det_m);
    let m_orders = m_text
        .iter()
        .map(|r| {
            r.iter()
                .map(|t| order(t, d0))
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let a_orders = a_text
        .iter()
        .map(|r| {
            r.iter()
                .map(|t| order(t, d0))
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let det_m_order = order(&det_m_text, d0)?;
    // cross-check of the printed texts against the internal objects (own small valuation on the same objects)
    for (i, r) in m.iter().enumerate() {
        for (j, x) in r.iter().enumerate() {
            if rf_valuation(x, d0.value()) != m_orders[i][j] {
                return Err(format!(
                    "valuation mismatch on M[{i}][{j}] (text {:?})",
                    m_text[i][j]
                ));
            }
        }
    }
    Ok(ExactMap {
        a,
        m,
        det_m,
        a_text,
        m_text,
        det_m_text,
        m_orders,
        a_orders,
        det_m_order,
    })
}

/// `x * M` exactly for exact coefficient rows (rational functions): the coefficients of a key on R.
pub fn row_times(x: &[RF], m: &[Vec<RF>]) -> Vec<RF> {
    let n = m.len();
    (0..n)
        .map(|j| {
            let mut acc = &x[0] * &m[0][j];
            for i in 1..n {
                acc = &acc + &(&x[i] * &m[i][j]);
            }
            acc
        })
        .collect()
}

/// The d0 value of a D0 (convenience).
pub fn d0_value(d0: &D0) -> Rational {
    d0.value().clone()
}
