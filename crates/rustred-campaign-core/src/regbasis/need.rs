//! NEED (POLES_OBJECTIVE 1, 3.4): need(X_i) = base order + max over the band of the pole order of the composed
//! coefficient of X_i (X = the closure's terminals T, the regular set R, or the published masters P), against what is
//! available (the user's optional eps box, addendum 8; lane epsdepth owns its parsing: an adapter implements
//! [`Availability`] on its `EpsBox`; [`MapAvailability`] is the plain form for tests and CLIs).

use std::collections::HashMap;

use serde_json::{Value, json};

use crate::key::Key;

/// The eps order up to which a basis element's expansion is available (`None` = not stated: the row is `unknown`).
pub trait Availability {
    fn available(&self, key: Option<&[i32]>, label: &str) -> Option<i64>;
}

/// Per-key / per-label orders with an optional default.
#[derive(Clone, Debug, Default)]
pub struct MapAvailability {
    pub default: Option<i64>,
    pub keys: HashMap<Key, i64>,
    pub labels: HashMap<String, i64>,
}

impl Availability for MapAvailability {
    fn available(&self, key: Option<&[i32]>, label: &str) -> Option<i64> {
        key.and_then(|k| self.keys.get(k).copied())
            .or_else(|| self.labels.get(label).copied())
            .or(self.default)
    }
}

/// One basis element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeedRow {
    pub label: String,
    pub key: Option<Key>,
    /// Max pole order over the decided band keys (`None` = no band key has a non-zero coefficient on it).
    pub max_pole: Option<u32>,
    /// base + max_pole (base when no pole / never excited).
    pub need: i64,
    pub available: Option<i64>,
    /// max(0, need - available) when available is known.
    pub missing: Option<i64>,
}

/// The need table of one basis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeedTable {
    pub basis: String,
    pub base_order: i64,
    pub rows: Vec<NeedRow>,
    /// Sum of `missing` over the rows with a known availability (`None` when no availability was given).
    pub missing_total: Option<i64>,
    pub rows_exceeding: usize,
    pub rows_unknown: usize,
    /// "regular through rank r (lower bound)" / "full band": copied from the band verdict.
    pub established: String,
}

/// Build the need table. `labels[i]`, `keys[i]` and `max_poles[i]` describe basis element i.
pub fn need_table(
    basis: &str,
    base_order: i64,
    labels: &[String],
    keys: &[Option<Key>],
    max_poles: &[Option<u32>],
    avail: Option<&dyn Availability>,
    established: &str,
) -> NeedTable {
    let mut rows = Vec::with_capacity(labels.len());
    let (mut total, mut exceeding, mut unknown) = (0i64, 0usize, 0usize);
    for i in 0..labels.len() {
        let need = base_order + i64::from(max_poles[i].unwrap_or(0));
        let available = avail.and_then(|a| a.available(keys[i].as_deref(), &labels[i]));
        let missing = available.map(|a| (need - a).max(0));
        match missing {
            Some(m) => {
                total += m;
                if m > 0 {
                    exceeding += 1;
                }
            }
            None => unknown += 1,
        }
        rows.push(NeedRow {
            label: labels[i].clone(),
            key: keys[i].clone(),
            max_pole: max_poles[i],
            need,
            available,
            missing,
        });
    }
    NeedTable {
        basis: basis.to_string(),
        base_order,
        rows,
        missing_total: avail.map(|_| total),
        rows_exceeding: exceeding,
        rows_unknown: unknown,
        established: established.to_string(),
    }
}

impl NeedTable {
    pub fn to_value(&self) -> Value {
        json!({
            "basis": self.basis,
            "base_order": self.base_order,
            "established": self.established,
            "missing_total": self.missing_total,
            "rows_exceeding": self.rows_exceeding,
            "rows_unknown": self.rows_unknown,
            "rows": self.rows.iter().map(|r| json!({
                "label": r.label, "key": r.key, "max_pole": r.max_pole, "need": r.need,
                "available": r.available, "missing": r.missing,
                "status": match r.missing { None => "unknown".to_string(), Some(0) => "within".to_string(),
                                            Some(k) => format!("exceeds by {k}") },
            })).collect::<Vec<_>>(),
        })
    }
}
