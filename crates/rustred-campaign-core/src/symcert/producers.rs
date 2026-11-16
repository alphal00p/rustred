//! Readers of the three producers' records into [`SymRelation`]s (lane symcert). Each reader takes the producer's OWN
//! files as they are written today and the matrices of the producer's verified group (by element index; the caller
//! re-derives the group and checks its digest against the producer's). Nothing is re-solved here: the relation is
//! exactly the producer's (its key, its rhs keys and coefficient texts); the rows are the ones the producer says
//! derived it.
//!
//! - repair flow COMPOSITES (`rr_rf_symflow`: `composite.table.jsonl` + `composite.verification.jsonl`): the element's
//!   expanded row at the key + every catalogue rule's IBP rows at its term (absolute seeds from the catalogue record);
//! - lane tworow (`two_row_relations` JSONL, one line per key): the rows of the relation (element, seed) as written;
//! - the closing loop's SYMMETRY RELATION AT THE KEY (derived table `symmetry-at-key.table.jsonl` + escalation lines
//!   `symrel.r<k>.jsonl`): the rows of every listed element at the key.

use super::{CatalogueSeeds, NotExpressible, SymRelation, SymRow, ZeroSectors, record_seeds_at};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// SHA-256 (hex) of a byte string (the import's record digest).
pub fn sha256_hex(b: &[u8]) -> String {
    let mut h = crate::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

/// Parse a constant key text `I(a,b,..)`.
pub fn parse_key_text(t: &str) -> Option<Vec<i64>> {
    let s: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    let inner = s.strip_prefix("I(")?.strip_suffix(')')?;
    inner.split(',').map(|x| x.parse::<i64>().ok()).collect()
}

/// A relation's right-hand side: (key, exact coefficient text).
pub type Rhs = Vec<(Vec<i64>, String)>;

/// A point-rule record line: (key, rhs (key, text)).
pub fn point_record(v: &Value) -> Result<(Vec<i64>, Rhs), String> {
    let key = v["target"]
        .as_str()
        .and_then(parse_key_text)
        .ok_or("target is not a constant key")?;
    let mut rhs = vec![];
    for t in v["rhs"].as_array().ok_or("rhs")? {
        let k = match &t[0] {
            Value::String(s) => parse_key_text(s),
            Value::Array(_) => serde_json::from_value(t[0].clone()).ok(),
            _ => None,
        }
        .ok_or("rhs key")?;
        rhs.push((
            k,
            t[1].as_str()
                .ok_or("rhs coefficient is not a text")?
                .to_string(),
        ));
    }
    Ok((key, rhs))
}

fn rows_of(
    elements_seeds: &[(usize, Vec<i64>)],
    matrices: &[Option<Vec<i64>>],
) -> Result<Vec<SymRow>, NotExpressible> {
    elements_seeds
        .iter()
        .map(|(e, s)| {
            let m = matrices
                .get(*e)
                .cloned()
                .flatten()
                .ok_or_else(|| NotExpressible {
                    class: "element_without_matrix",
                    why: format!("element {e} has no momentum matrix in the verified group"),
                })?;
            Ok(SymRow {
                seed: s.clone(),
                element: *e,
                matrix: m,
            })
        })
        .collect()
}

// ------------------------------------------------------------------------------------------------ composites

/// One catalogue replacement of a composite (from its verification line).
#[derive(Clone, Debug, PartialEq)]
pub struct Replacement {
    pub term: Vec<i64>,
    pub source: String,
    pub label: Value,
    pub record_sha256: String,
}

/// One shipped composite relation with its verification line.
#[derive(Clone, Debug, PartialEq)]
pub struct CompositeEntry {
    pub index: usize,
    pub key: Vec<i64>,
    pub rhs: Vec<(Vec<i64>, String)>,
    /// SHA-256 of the shipped record line.
    pub record_sha256: String,
    pub element: usize,
    pub group_digest: String,
    pub plain: bool,
    pub replacements: Vec<Replacement>,
    pub verification: Value,
}

/// The shipped composites of an `rr_rf_symflow` result, each paired with its verification line (by target).
pub fn composites(table: &str, verification: &str) -> Result<Vec<CompositeEntry>, String> {
    let mut ver: BTreeMap<Vec<i64>, Value> = BTreeMap::new();
    for (i, l) in verification.lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let v: Value =
            serde_json::from_str(l).map_err(|e| format!("verification {}: {e}", i + 1))?;
        let k = v["target"]
            .as_str()
            .and_then(parse_key_text)
            .ok_or_else(|| format!("verification {}: target", i + 1))?;
        if ver.insert(k, v).is_some() {
            return Err(format!(
                "verification {}: a second line for its target",
                i + 1
            ));
        }
    }
    let mut out = vec![];
    for (i, l) in table.lines().enumerate() {
        if !l.starts_with("{\"") || !l.contains("\"type\":\"rule\"") {
            continue;
        }
        let v: Value = serde_json::from_str(l).map_err(|e| format!("table {}: {e}", i + 1))?;
        if v["type"] != "rule" {
            continue;
        }
        let (key, rhs) = point_record(&v).map_err(|e| format!("table {}: {e}", i + 1))?;
        let w = ver
            .remove(&key)
            .ok_or_else(|| format!("table {}: no verification line for its target", i + 1))?;
        let element = w["element"].as_u64().ok_or("verification element")? as usize;
        if v["symmetry_elements"] != json!([element]) {
            return Err(format!(
                "table {}: element differs from its verification",
                i + 1
            ));
        }
        let mut replacements = vec![];
        for r in w["replacements"].as_array().ok_or("replacements")? {
            replacements.push(Replacement {
                term: r["term"]
                    .as_str()
                    .and_then(parse_key_text)
                    .ok_or("replacement term")?,
                source: r["source"].as_str().unwrap_or("").to_string(),
                label: r["label"].clone(),
                record_sha256: r["record_sha256"]
                    .as_str()
                    .ok_or("replacement record_sha256")?
                    .to_string(),
            });
        }
        out.push(CompositeEntry {
            index: v["index"].as_u64().unwrap_or(i as u64) as usize,
            key,
            rhs,
            record_sha256: sha256_hex(l.as_bytes()),
            element,
            group_digest: w["group_digest"].as_str().unwrap_or("").to_string(),
            plain: w["plain"].as_bool().unwrap_or(false),
            replacements,
            verification: w,
        });
    }
    if let Some((k, _)) = ver.into_iter().next() {
        return Err(format!(
            "a verification line without a shipped record ({k:?})"
        ));
    }
    Ok(out)
}

/// The relation of a composite: the element's row at the key + the catalogue rules' seeds at their terms.
/// `catalogue(record_sha256)` returns the catalogue record (parsed) or None.
pub fn composite_relation(
    c: &CompositeEntry,
    matrices: &[Option<Vec<i64>>],
    catalogue: &dyn Fn(&str) -> Option<Value>,
    zero_sectors: Option<ZeroSectors>,
) -> Result<SymRelation, NotExpressible> {
    let rows = rows_of(&[(c.element, c.key.clone())], matrices)?;
    let mut cat = vec![];
    for r in &c.replacements {
        let rec = catalogue(&r.record_sha256).ok_or_else(|| NotExpressible {
            class: "catalogue_record_missing",
            why: format!(
                "catalogue record {}#{} (sha256 {}) not found",
                r.source, r.label, r.record_sha256
            ),
        })?;
        let seeds = record_seeds_at(&rec, &r.term).map_err(|e| NotExpressible {
            class: "catalogue_without_seeds",
            why: format!("catalogue rule {}#{}: {e}", r.source, r.label),
        })?;
        if rec["sym_rows"].as_i64().unwrap_or(0) > 0 {
            return Err(NotExpressible {
                class: "catalogue_symmetry_rows",
                why: format!(
                    "catalogue rule {}#{} uses one-term symmetry rows at its seeds",
                    r.source, r.label
                ),
            });
        }
        cat.push(CatalogueSeeds {
            source: r.source.clone(),
            label: r.label.clone(),
            record_sha256: r.record_sha256.clone(),
            term: r.term.clone(),
            seeds,
        });
    }
    Ok(SymRelation {
        id: format!("composite#{}", c.index),
        producer: if c.plain {
            "repairflow:plain".into()
        } else {
            "repairflow:composite".into()
        },
        producer_record_sha256: Some(c.record_sha256.clone()),
        key: c.key.clone(),
        rhs: c.rhs.clone(),
        rows,
        catalogue: cat,
        group_digest: Some(c.group_digest.clone()),
        zero_sectors,
        own: json!({"verdict": "accepted", "by": "import composite verification (own algebra)",
            "verification": c.verification}),
    })
}

// ------------------------------------------------------------------------------------------------ tworow

/// One key's best two-row relation (fewest rhs terms; ties: the first written).
#[derive(Clone, Debug, PartialEq)]
pub struct TwoRowEntry {
    pub key: Vec<i64>,
    pub kind: String,
    pub label: String,
    pub rows: Vec<(usize, Vec<i64>)>,
    pub weights: Vec<String>,
    pub rhs: Vec<(Vec<i64>, String)>,
    pub relation: Value,
}

/// The served keys of a tworow result (summary / non-served lines skipped).
pub fn tworow(lines: &str) -> Result<Vec<TwoRowEntry>, String> {
    let mut out = vec![];
    for (i, l) in lines.lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(l).map_err(|e| format!("tworow {}: {e}", i + 1))?;
        if v["outcome"] != "served" {
            continue;
        }
        let key: Vec<i64> = serde_json::from_value(v["key"].clone())
            .map_err(|e| format!("tworow {}: key {e}", i + 1))?;
        let rels = v["relations"].as_array().ok_or("relations")?;
        let best = rels
            .iter()
            .enumerate()
            .min_by_key(|(j, r)| (r["rhs_terms"].as_u64().unwrap_or(u64::MAX), *j))
            .map(|(_, r)| r)
            .ok_or_else(|| format!("tworow {}: served without a relation", i + 1))?;
        let rows = best["rows"]
            .as_array()
            .ok_or("rows")?
            .iter()
            .map(|r| {
                Ok((
                    r["element"].as_u64().ok_or("row element")? as usize,
                    serde_json::from_value::<Vec<i64>>(r["seed"].clone())
                        .map_err(|e| format!("row seed {e}"))?,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let mut rhs = vec![];
        for t in best["rhs"].as_array().ok_or("rhs")? {
            rhs.push((
                serde_json::from_value::<Vec<i64>>(t[0].clone())
                    .map_err(|e| format!("rhs key {e}"))?,
                t[1].as_str().ok_or("rhs coefficient")?.to_string(),
            ));
        }
        out.push(TwoRowEntry {
            key,
            kind: best["kind"].as_str().unwrap_or("").to_string(),
            label: v["label"].as_str().unwrap_or("").to_string(),
            rows,
            weights: best["weights"]
                .as_array()
                .map(|w| {
                    w.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            rhs,
            relation: best.clone(),
        });
    }
    Ok(out)
}

/// The relation of a tworow entry (its own rows; zero-sector terms were pruned by the producer: `zero_sectors`).
pub fn tworow_relation(
    t: &TwoRowEntry,
    matrices: &[Option<Vec<i64>>],
    group_digest: &str,
    zero_sectors: Option<ZeroSectors>,
) -> Result<SymRelation, NotExpressible> {
    Ok(SymRelation {
        id: format!("tworow:{}:{}", t.label, super::key_pattern(&t.key)),
        producer: format!("tworow:{}", t.kind),
        producer_record_sha256: None,
        key: t.key.clone(),
        rhs: t.rhs.clone(),
        rows: rows_of(&t.rows, matrices)?,
        catalogue: vec![],
        group_digest: Some(group_digest.to_string()),
        zero_sectors,
        own: json!({"verdict": "accepted", "by": "tworow exact RREF (own algebra) + second route",
            "weights": t.weights, "relation_rows": t.relation["rows"]}),
    })
}

// ------------------------------------------------------------------------------------------------ symmetry at key

/// One closing-loop symmetry relation at its key.
#[derive(Clone, Debug, PartialEq)]
pub struct AtKeyEntry {
    pub index: usize,
    pub key: Vec<i64>,
    pub rhs: Vec<(Vec<i64>, String)>,
    pub record_sha256: String,
    pub elements: Vec<usize>,
    pub group_digest: String,
}

/// The derived table's relations, each paired with its escalation line (verdict `relation`, same key and rhs).
pub fn symmetry_at_key(table: &str, escalation: &str) -> Result<Vec<AtKeyEntry>, String> {
    let mut esc: BTreeMap<Vec<i64>, Value> = BTreeMap::new();
    for (i, l) in escalation.lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(l).map_err(|e| format!("escalation {}: {e}", i + 1))?;
        if v["verdict"] != "relation" {
            continue;
        }
        let k: Vec<i64> = serde_json::from_value(v["key"].clone())
            .map_err(|e| format!("escalation {}: key {e}", i + 1))?;
        esc.insert(k, v);
    }
    let mut out = vec![];
    for (i, l) in table.lines().enumerate() {
        if !l.contains("\"type\":\"rule\"") {
            continue;
        }
        let v: Value = serde_json::from_str(l).map_err(|e| format!("table {}: {e}", i + 1))?;
        let (key, rhs) = point_record(&v).map_err(|e| format!("table {}: {e}", i + 1))?;
        let e = esc
            .get(&key)
            .ok_or_else(|| format!("table {}: no escalation relation for its key", i + 1))?;
        let mut erhs = vec![];
        for t in e["rhs"].as_array().ok_or("escalation rhs")? {
            erhs.push((
                serde_json::from_value::<Vec<i64>>(t[0].clone()).map_err(|x| x.to_string())?,
                t[1].as_str().ok_or("escalation coefficient")?.to_string(),
            ));
        }
        let norm = |r: &[(Vec<i64>, String)]| {
            let mut x = r.to_vec();
            x.sort();
            x
        };
        if norm(&erhs) != norm(&rhs) {
            return Err(format!(
                "table {}: rhs differs from its escalation line",
                i + 1
            ));
        }
        out.push(AtKeyEntry {
            index: v["index"].as_u64().unwrap_or(i as u64) as usize,
            key,
            rhs,
            record_sha256: sha256_hex(l.as_bytes()),
            elements: e["elements"]
                .as_array()
                .ok_or("escalation elements")?
                .iter()
                .map(|x| x.as_u64().map(|u| u as usize).ok_or("element"))
                .collect::<Result<_, _>>()?,
            group_digest: e["group_digest"].as_str().unwrap_or("").to_string(),
        });
    }
    Ok(out)
}

/// The relation of a symmetry-at-key entry: the row of every listed element at the key.
pub fn at_key_relation(
    a: &AtKeyEntry,
    matrices: &[Option<Vec<i64>>],
    zero_sectors: Option<ZeroSectors>,
) -> Result<SymRelation, NotExpressible> {
    let es: Vec<(usize, Vec<i64>)> = a.elements.iter().map(|&e| (e, a.key.clone())).collect();
    Ok(SymRelation {
        id: format!("symmetry-at-key#{}", a.index),
        producer: "closeloop:symmetry-at-key".into(),
        producer_record_sha256: Some(a.record_sha256.clone()),
        key: a.key.clone(),
        rhs: a.rhs.clone(),
        rows: rows_of(&es, matrices)?,
        catalogue: vec![],
        group_digest: Some(a.group_digest.clone()),
        zero_sectors,
        own: json!({"verdict": "accepted", "by": "import re-derivation from the verified group (own algebra)",
            "elements": a.elements}),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: [i64; 4] = [1, 0, 0, 1];

    #[test]
    fn composites_pair_records_with_verification_lines() {
        let table = "{\"type\":\"header\"}\n{\"case\":\"point\",\"chart\":\"110\",\"fixed\":[1,1,-1],\"index\":0,\"rhs\":[[\"I(1,1,0)\",\"2\"],[\"I(1,0,-1)\",\"(d-4)/3\"]],\"symmetry_elements\":[1],\"target\":\"I(1,1,-1)\",\"type\":\"rule\"}\n";
        let ver = "{\"element\":1,\"group_digest\":\"g\",\"plain\":false,\"replacements\":[{\"term\":\"I(1,0,-1)\",\"source\":\"own\",\"label\":5,\"record_sha256\":\"r5\",\"depth\":0}],\"target\":\"I(1,1,-1)\"}\n";
        let c = composites(table, ver).unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].key, vec![1, 1, -1]);
        assert_eq!(c[0].replacements[0].term, vec![1, 0, -1]);
        let mats = vec![None, Some(M.to_vec())];
        let cat = |s: &str| {
            (s == "r5").then(|| json!({"seeds": ["I(n0,n1+1,n2)"], "recenter": [0, 0, 1]}))
        };
        let r = composite_relation(&c[0], &mats, &cat, None).unwrap();
        assert_eq!(r.catalogue[0].seeds, vec![vec![1, 1, 0]]);
        assert_eq!(r.rows[0].seed, vec![1, 1, -1]);
        assert_eq!(r.producer, "repairflow:composite");
        let none = |_: &str| None;
        assert_eq!(
            composite_relation(&c[0], &mats, &none, None)
                .unwrap_err()
                .class,
            "catalogue_record_missing"
        );
        let nomat = vec![None, None];
        assert_eq!(
            composite_relation(&c[0], &nomat, &cat, None)
                .unwrap_err()
                .class,
            "element_without_matrix"
        );
        // a verification line without its record is refused
        assert!(composites("", ver).is_err());
    }

    #[test]
    fn tworow_takes_the_smallest_relation() {
        let l = json!({"outcome": "served", "label": "F", "key": [1, 1, -1], "relations": [
            {"kind": "pair", "rhs_terms": 2, "rows": [{"element": 0, "seed": [1, 1, -1]}, {"element": 0, "seed": [1, 0, -1]}],
             "weights": ["1", "-1"], "rhs": [[[1, 1, 0], "1"], [[1, 0, 0], "2"]]},
            {"kind": "pair", "rhs_terms": 1, "rows": [{"element": 0, "seed": [1, 1, -1]}],
             "weights": ["1"], "rhs": [[[1, 1, 0], "1"]]}]});
        let t = tworow(&format!("{l}\n{{\"summary\":1}}\n")).unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].rhs.len(), 1);
        let r = tworow_relation(&t[0], &[Some(M.to_vec())], "g", None).unwrap();
        assert_eq!(r.rows.len(), 1);
    }

    #[test]
    fn symmetry_at_key_pairs_and_checks_the_rhs() {
        let table = "{\"type\":\"header\"}\n{\"case\":\"point\",\"fixed\":[1,1,-1],\"index\":0,\"rhs\":[[\"I(1,1,0)\",\"-1\"]],\"target\":\"I(1,1,-1)\",\"type\":\"rule\"}\n";
        let esc = "{\"verdict\":\"no_relation\",\"key\":[1,0,-1]}\n{\"verdict\":\"relation\",\"key\":[1,1,-1],\"elements\":[0,1],\"group_digest\":\"g\",\"rhs\":[[[1,1,0],\"-1\"]]}\n";
        let a = symmetry_at_key(table, esc).unwrap();
        assert_eq!(a[0].elements, vec![0, 1]);
        let r = at_key_relation(&a[0], &[Some(M.to_vec()), Some(M.to_vec())], None).unwrap();
        assert_eq!(r.rows.len(), 2);
        let bad = esc.replace("\"-1\"]]}", "\"-2\"]]}");
        assert!(symmetry_at_key(table, &bad).is_err());
    }
}
