//! Kernel certificates of MULTI-TERM symmetry relations (lane symcert; interfaces/proof-symrows-job.md 7): the PURE
//! part. A relation `I(K) = sum_c r_c I(k_c)` that a producer derived from expanded symmetry rows (the closing loop's
//! symmetry relation at the key, the repair flow's composites, lane tworow's two-row forms) is turned into the files
//! of ONE `proof-symrows` job: a point-rule record of the relation (absolute seeds of the catalogue rules it used), the
//! key, and the symmetry rows (seed + the momentum matrix of the producer's VERIFIED group element). The kernel
//! (rr_proof `--symrows`) re-derives every row from the matrix and the family itself and proves the relation vector at
//! the key in the span of those rows (plus the catalogue rules' IBP rows): a certificate independent of our algebra.
//!
//! Generic (L8): arities, loop counts, kernel limits and zero-sector censuses are DATA ([`KernelCaps`],
//! [`ZeroSectors`]); nothing here knows a node.

pub mod census;
pub mod producers;
pub mod second;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// One expanded symmetry row `I(seed) - s E_g(seed)` of a verified group element.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymRow {
    pub seed: Vec<i64>,
    /// Index of the element in the producer's verified group.
    pub element: usize,
    /// The element's momentum map T (row-vector convention v -> v T over (k_1..k_L, q), row-major).
    pub matrix: Vec<i64>,
}

/// A catalogue rule a composite used at one of its terms: its identity and its seeds AT that term (absolute: the
/// rule's seed patterns evaluated at the term, plus its recenter).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogueSeeds {
    pub source: String,
    pub label: Value,
    pub record_sha256: String,
    pub term: Vec<i64>,
    pub seeds: Vec<Vec<i64>>,
}

/// A zero-sector census to write into the table (`zero_sectors` record): masks over every slot (`1` = line > 0).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZeroSectors {
    /// Who computed it (e.g. `engine table <path> sha256 <hex>`, `campaign-core lee-criterion`).
    pub producer: String,
    pub scope: String,
    pub masks: Vec<String>,
}

/// One relation to certify.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SymRelation {
    /// The producer's id of the relation (journal key together with the relation digest).
    pub id: String,
    /// `closeloop:symmetry-at-key` | `repairflow:composite` | `repairflow:plain` | `tworow:<kind>` | ...
    pub producer: String,
    /// SHA-256 of the producer's shipped record line (the import binds evidence to it), if any.
    pub producer_record_sha256: Option<String>,
    pub key: Vec<i64>,
    /// The relation's right-hand side: (key, the producer's exact coefficient text).
    pub rhs: Vec<(Vec<i64>, String)>,
    pub rows: Vec<SymRow>,
    pub catalogue: Vec<CatalogueSeeds>,
    pub group_digest: Option<String>,
    /// Set when the producer dropped zero-sector terms (the kernel then drops the same columns, `--drop-zero-sectors`).
    pub zero_sectors: Option<ZeroSectors>,
    /// What OUR algebra said about the relation (the other side of a finding): free JSON of the producer.
    pub own: Value,
}

/// What the kernel binary can express (DATA of the kernel program, never constants here).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelCaps {
    /// The arity the kernel's keys have.
    pub arity: usize,
    /// Largest number of symmetry rows per job (None = no limit).
    #[serde(default)]
    pub max_rows: Option<usize>,
    /// Largest number of absolute seeds per job (None = no limit).
    #[serde(default)]
    pub max_seeds: Option<usize>,
}

/// Why a relation cannot be put to the kernel (typed; never a refusal of the relation).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NotExpressible {
    pub class: &'static str,
    pub why: String,
}

fn nx(class: &'static str, why: impl Into<String>) -> NotExpressible {
    NotExpressible {
        class,
        why: why.into(),
    }
}

impl SymRelation {
    /// The absolute seeds of every catalogue use (deduplicated, in first-seen order).
    pub fn seeds(&self) -> Vec<Vec<i64>> {
        let mut out: Vec<Vec<i64>> = vec![];
        for c in &self.catalogue {
            for s in &c.seeds {
                if !out.contains(s) {
                    out.push(s.clone());
                }
            }
        }
        out
    }

    /// Whether the kernel can be asked about this relation (arity, rows, matrices, catalogue seeds, rhs).
    pub fn expressible(&self, caps: &KernelCaps) -> Result<(), NotExpressible> {
        let n = caps.arity;
        if self.key.len() != n {
            return Err(nx(
                "arity",
                format!("key of arity {} (the kernel's is {n})", self.key.len()),
            ));
        }
        if self.rows.is_empty() {
            return Err(nx("no_rows", "no symmetry row derives the relation"));
        }
        if let Some(m) = caps.max_rows
            && self.rows.len() > m
        {
            return Err(nx(
                "too_many_rows",
                format!("{} symmetry rows (kernel cap {m})", self.rows.len()),
            ));
        }
        for (i, r) in self.rows.iter().enumerate() {
            if r.seed.len() != n {
                return Err(nx(
                    "arity",
                    format!("row {i}: seed of arity {}", r.seed.len()),
                ));
            }
            let len = r.matrix.len();
            let w = (0..=len).find(|w| w * w >= len).unwrap_or(0);
            if w < 2 || w * w != len {
                return Err(nx(
                    "matrix",
                    format!(
                        "row {i}: element {} has no square momentum matrix",
                        r.element
                    ),
                ));
            }
        }
        for (k, c) in &self.rhs {
            if k.len() != n {
                return Err(nx("arity", "a right-hand-side key of another arity"));
            }
            if k == &self.key {
                return Err(nx("key_on_rhs", "the key itself on the right-hand side"));
            }
            if c.trim().is_empty() {
                return Err(nx("coefficient", "an empty coefficient text"));
            }
        }
        let mut ks: Vec<&Vec<i64>> = self.rhs.iter().map(|(k, _)| k).collect();
        ks.sort();
        if ks.windows(2).any(|w| w[0] == w[1]) {
            return Err(nx("repeated_term", "a repeated right-hand-side key"));
        }
        for c in &self.catalogue {
            if c.seeds.is_empty() {
                return Err(nx(
                    "catalogue_without_seeds",
                    format!(
                        "catalogue rule {}#{} carries no seeds (its IBP rows cannot be put to the kernel)",
                        c.source, c.label
                    ),
                ));
            }
            if c.seeds.iter().any(|s| s.len() != n) {
                return Err(nx("arity", "a catalogue seed of another arity"));
            }
        }
        let seeds = self.seeds();
        if let Some(m) = caps.max_seeds
            && seeds.len() > m
        {
            return Err(nx(
                "too_many_seeds",
                format!("{} absolute seeds (kernel cap {m})", seeds.len()),
            ));
        }
        if let Some(z) = &self.zero_sectors
            && z.masks.iter().any(|m| m.len() != n)
        {
            return Err(nx("census", "a zero-sector mask of another arity"));
        }
        Ok(())
    }
}

/// A kernel certificate's verdict for the import (interfaces/proof-symrows-job.md 6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CertVerdict {
    /// `EXACT_IN_SPAN_AT_KEY` (the certificate line's SHA-256).
    Certified(String),
    /// The kernel refused (its answer): the relation is NOT per-key while the disagreement stands.
    Refused(String),
}

/// The import's evidence of kernel certificates: (SHA-256 of the producer's supplied record, key) -> verdict. A
/// refusal wins over a certificate of the same pair (contradicting answers are never per-key).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SymmetryCertificates {
    map: std::collections::HashMap<(String, Vec<i32>), CertVerdict>,
}

impl SymmetryCertificates {
    pub fn certify(&mut self, record_sha256: &str, key: Vec<i32>, certificate_sha256: String) {
        self.map
            .entry((record_sha256.to_string(), key))
            .or_insert(CertVerdict::Certified(certificate_sha256));
    }
    pub fn refuse(&mut self, record_sha256: &str, key: Vec<i32>, answer: String) {
        self.map.insert(
            (record_sha256.to_string(), key),
            CertVerdict::Refused(answer),
        );
    }
    pub fn get(&self, record_sha256: &str, key: &[i32]) -> Option<&CertVerdict> {
        self.map.get(&(record_sha256.to_string(), key.to_vec()))
    }
    pub fn len(&self) -> usize {
        self.map.len()
    }
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// The constant pattern text `I(a,b,..)` of a key.
pub fn key_pattern(k: &[i64]) -> String {
    format!(
        "I({})",
        k.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
    )
}

/// The sector mask of a key (`1` for a slot with a positive power).
pub fn mask_of(k: &[i64]) -> String {
    k.iter().map(|&x| if x > 0 { '1' } else { '0' }).collect()
}

/// The files of one job (bytes; digests are the supervisor's).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobFiles {
    pub table: Vec<u8>,
    /// Byte offset of the record line in `table`.
    pub offset: u64,
    /// The record line (without newline).
    pub record_line: Vec<u8>,
    /// The census line written into the table (when zero sectors are dropped).
    pub census_line: Option<Vec<u8>>,
    pub census_masks: u64,
    pub keys: Vec<u8>,
    pub symrows: Vec<u8>,
    /// Element of every symrows line, in order.
    pub elements: Vec<u64>,
    /// Whether IBP rows are needed (catalogue seeds present).
    pub needs_ibp_rows: bool,
}

/// The point-rule record line of the relation: it STARTS with `{"type":"rule"` (rr_proof's offset rule).
pub fn record_line(r: &SymRelation) -> Vec<u8> {
    let rhs: Vec<Value> = r
        .rhs
        .iter()
        .map(|(k, c)| json!([key_pattern(k), c]))
        .collect();
    let seeds: Vec<String> = r.seeds().iter().map(|s| key_pattern(s)).collect();
    let members: Vec<(&str, Value)> = vec![
        ("chart", json!(mask_of(&r.key))),
        ("index", json!(0)),
        ("case", json!("point")),
        ("fixed", json!(r.key)),
        ("equations", json!([])),
        ("target", json!(key_pattern(&r.key))),
        ("rhs", json!(rhs)),
        ("exceptions", json!([])),
        ("seeds", json!(seeds)),
        ("recenter", json!(vec![0i64; r.key.len()])),
        ("dispatch", json!("Point")),
        ("witness_loci", json!([])),
        ("symcert", json!({"id": r.id, "producer": r.producer})),
    ];
    let mut s = String::from("{\"type\":\"rule\"");
    for (k, v) in members {
        s.push_str(&format!(",{}:{}", json!(k), v));
    }
    s.push('}');
    s.into_bytes()
}

/// Build the job files of a relation. `family_source` = the presented family text of the frame the keys are in (the
/// kernel derives the slot momenta of every row from it).
pub fn build_files(r: &SymRelation, family_source: &str) -> Result<JobFiles, String> {
    let head = json!({"type": "header", "schema": "table_dump.v1", "family_source": family_source,
        "producer": "rustred-campaign symcert", "relation_id": r.id});
    let mut table = format!("{head}\n").into_bytes();
    let mut census_line = None;
    let mut census_masks = 0;
    if let Some(z) = &r.zero_sectors {
        let l = json!({"type": "zero_sectors", "scope": z.scope, "count": z.masks.len(), "masks": z.masks,
            "producer": z.producer})
        .to_string()
        .into_bytes();
        table.extend_from_slice(&l);
        table.push(b'\n');
        census_masks = z.masks.len() as u64;
        census_line = Some(l);
    }
    let offset = table.len() as u64;
    let line = record_line(r);
    table.extend_from_slice(&line);
    table.push(b'\n');
    let keys = format!(
        "{}\n",
        r.key
            .iter()
            .enumerate()
            .map(|(i, v)| format!("n{i}={v}"))
            .collect::<Vec<_>>()
            .join(",")
    )
    .into_bytes();
    let mut symrows = Vec::new();
    for (i, row) in r.rows.iter().enumerate() {
        let l =
            json!({"seed": row.seed, "matrix": row.matrix, "id": format!("e{}#{i}", row.element)});
        symrows.extend_from_slice(l.to_string().as_bytes());
        symrows.push(b'\n');
    }
    Ok(JobFiles {
        table,
        offset,
        record_line: line,
        census_line,
        census_masks,
        keys,
        symrows,
        elements: r.rows.iter().map(|x| x.element as u64).collect(),
        needs_ibp_rows: !r.catalogue.is_empty(),
    })
}

/// Evaluate a seed / target pattern `I(n0+1,n1,2,..)` at a key (the kernel's grammar: each slot `n<i>`, `n<i>+c`,
/// `n<i>-c` with its own index, or an integer).
pub fn pattern_at(pat: &str, key: &[i64]) -> Result<Vec<i64>, String> {
    let s: String = pat.chars().filter(|c| !c.is_whitespace()).collect();
    let inner = s
        .strip_prefix("I(")
        .and_then(|x| x.strip_suffix(')'))
        .ok_or_else(|| format!("pattern {pat:?}"))?;
    let parts: Vec<&str> = inner.split(',').collect();
    if parts.len() != key.len() {
        return Err(format!(
            "pattern of arity {} at a key of arity {}",
            parts.len(),
            key.len()
        ));
    }
    parts
        .iter()
        .enumerate()
        .map(|(i, a)| {
            if let Some(r) = a.strip_prefix('n') {
                let p = r.find(['+', '-']).unwrap_or(r.len());
                let idx: usize = r[..p].parse().map_err(|_| format!("pattern slot {a:?}"))?;
                if idx != i {
                    return Err(format!("pattern slot index {idx} at position {i}"));
                }
                let off: i64 = if p < r.len() {
                    r[p..]
                        .trim_start_matches('+')
                        .parse()
                        .map_err(|_| format!("pattern offset {a:?}"))?
                } else {
                    0
                };
                Ok(key[i] + off)
            } else {
                a.parse::<i64>().map_err(|_| format!("pattern value {a:?}"))
            }
        })
        .collect()
}

/// The absolute seeds of a table rule record AT a term: every `seeds` pattern evaluated at the term, plus the record's
/// `recenter` (zeros when absent). `Err` names a record without seeds.
pub fn record_seeds_at(record: &Value, term: &[i64]) -> Result<Vec<Vec<i64>>, String> {
    let pats = record["seeds"]
        .as_array()
        .ok_or("the record carries no seeds")?;
    let rc: Vec<i64> = match record.get("recenter") {
        Some(Value::Array(a)) => a
            .iter()
            .map(|x| x.as_i64().ok_or("recenter entry"))
            .collect::<Result<_, _>>()?,
        _ => vec![0; term.len()],
    };
    if rc.len() != term.len() {
        return Err("recenter of another arity".into());
    }
    let mut out = vec![];
    for p in pats {
        let s = pattern_at(p.as_str().ok_or("seed is not a text")?, term)?;
        out.push(s.iter().zip(&rc).map(|(a, b)| a + b).collect());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel() -> SymRelation {
        SymRelation {
            id: "t".into(),
            producer: "test".into(),
            producer_record_sha256: None,
            key: vec![1, 1, -2],
            rhs: vec![
                (vec![1, 1, -1], "-2".into()),
                (vec![1, 0, -2], "1/2".into()),
            ],
            rows: vec![SymRow {
                seed: vec![1, 1, -2],
                element: 3,
                matrix: vec![1, 0, 0, 0, 1, 0, 0, 0, 1],
            }],
            catalogue: vec![],
            group_digest: Some("g".into()),
            zero_sectors: None,
            own: json!({}),
        }
    }

    #[test]
    fn record_line_starts_with_the_rule_type_and_files_are_consistent() {
        let r = rel();
        let f = build_files(&r, "I(name(x),loops(k1,k2),externals(q))").unwrap();
        assert!(f.record_line.starts_with(b"{\"type\":\"rule\""));
        let at = &f.table[f.offset as usize..];
        assert!(at.starts_with(&f.record_line));
        assert_eq!(f.elements, vec![3]);
        assert_eq!(f.keys, b"n0=1,n1=1,n2=-2\n");
        let v: Value = serde_json::from_slice(&f.record_line).unwrap();
        assert_eq!(v["target"], "I(1,1,-2)");
        assert_eq!(v["chart"], "110");
        assert!(!f.needs_ibp_rows);
    }

    #[test]
    fn expressibility_is_typed() {
        let caps = KernelCaps {
            arity: 3,
            max_rows: None,
            max_seeds: None,
        };
        assert!(rel().expressible(&caps).is_ok());
        let mut r = rel();
        r.key = vec![1, 1];
        assert_eq!(r.expressible(&caps).unwrap_err().class, "arity");
        let mut r = rel();
        r.rows.clear();
        assert_eq!(r.expressible(&caps).unwrap_err().class, "no_rows");
        let mut r = rel();
        r.rows[0].matrix.pop();
        assert_eq!(r.expressible(&caps).unwrap_err().class, "matrix");
        let mut r = rel();
        r.catalogue.push(CatalogueSeeds {
            source: "bank".into(),
            label: json!(4),
            record_sha256: "x".into(),
            term: vec![1, 0, -2],
            seeds: vec![],
        });
        assert_eq!(
            r.expressible(&caps).unwrap_err().class,
            "catalogue_without_seeds"
        );
        let mut r = rel();
        r.rhs.push((vec![1, 1, -2], "1".into()));
        assert_eq!(r.expressible(&caps).unwrap_err().class, "key_on_rhs");
    }

    #[test]
    fn patterns_and_record_seeds() {
        assert_eq!(
            pattern_at("I(n0+1,n1,2)", &[3, 4, 5]).unwrap(),
            vec![4, 4, 2]
        );
        assert_eq!(
            pattern_at("I(n0-2, n1 , -1)", &[3, 4, 5]).unwrap(),
            vec![1, 4, -1]
        );
        assert!(pattern_at("I(n1,n1,0)", &[1, 1, 1]).is_err());
        let rec = json!({"seeds": ["I(n0,n1+1,n2)", "I(n0-1,n1,n2)"], "recenter": [1, 0, 0]});
        assert_eq!(
            record_seeds_at(&rec, &[2, 1, -1]).unwrap(),
            vec![vec![3, 2, -1], vec![2, 1, -1]]
        );
        assert!(record_seeds_at(&json!({}), &[1]).is_err());
    }
}
