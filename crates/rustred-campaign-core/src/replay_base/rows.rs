//! COPY (not a move) of pieces of rustred-core/src/node/replay/mod.rs at engine commit b9f06de7 (`Built`, `rows_for_p`,
//! `strip_p`, `addto_p`) for the campaign's independent A2 row check (plan 3.1 (e)). Changes ("campaign copy"): items
//! made `pub`; `Support` reduced to its `Exported` variant (all original IBPs at the exported seeds): the
//! retained-trace / direct-basis paths need the engine's trace certificate (`Cert`, `source_basis`), which is not
//! copied; therefore the `cert` and `chart` parameters are gone. The `rows_for` wrapper (fixed P = 2^61-1) is dropped.
//! rows3 (DEV-17 REVISED): DOMAIN LOGIC; the field addition is `CheckAlgebra::add` (generic `alg`).

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use super::algebra::{CheckAlgebra, Unknown};
use super::family::Family;
use super::record::{Key, Pat};

/// A row of the replay: key -> coefficient mod p.
pub type Row = BTreeMap<Key, u64>;

pub fn strip_p(p: u64, r: &Row) -> Row {
    r.iter()
        .filter(|(_, v)| **v % p != 0)
        .map(|(k, v)| (k.clone(), *v))
        .collect()
}

pub fn addto_p<A: CheckAlgebra>(alg: &A, p: u64, r: &mut Row, k: Key, x: u64) {
    let e = r.entry(k).or_insert(0);
    *e = alg.add(p, *e, x);
}

/// campaign copy: the one support kind kept (engine `Support::Exported`).
pub enum Support {
    Exported(Vec<Pat>),
}

pub struct Built {
    pub rows: Vec<Row>,
    pub labels: Vec<Value>,
    pub universe: BTreeSet<Key>,
}

/// The original IBP rows modulo the prime `p` at the exported seeds evaluated at `point` (+ `recenter` on symbolic
/// slots); repeated seed keys are generated once (engine semantics).
pub fn rows_for_p<A: CheckAlgebra>(
    alg: &A,
    p: u64,
    support: &Support,
    point: &[i64],
    recenter: &[i64],
    d: u64,
    f: &Family,
) -> Result<Built, Unknown> {
    let mut out = Built {
        rows: Vec::new(),
        labels: Vec::new(),
        universe: BTreeSet::new(),
    };
    let mut seen_y: BTreeSet<Key> = BTreeSet::new();
    let at_pat = |pat: &Pat| -> Key {
        pat.iter()
            .enumerate()
            .map(|(i, (ax, off))| ax.map_or(*off, |a| point[a] + off + recenter[i]))
            .collect()
    };
    let items: Vec<Key> = match support {
        Support::Exported(pats) => pats.iter().map(at_pat).collect(),
    };
    for y in items {
        if !seen_y.insert(y.clone()) {
            continue;
        }
        for (op, r) in f.rows_at_with(alg, &y, d, p)?.into_iter().enumerate() {
            let mut rr = Row::new();
            for (delta, v) in r {
                let key: Key = y.iter().zip(&delta).map(|(a, b)| a + b).collect();
                out.universe.insert(key.clone());
                addto_p(alg, p, &mut rr, key, v);
            }
            out.rows.push(strip_p(p, &rr));
            out.labels
                .push(json!({"independent_original_operator": op, "at": y}));
        }
    }
    Ok(out)
}
