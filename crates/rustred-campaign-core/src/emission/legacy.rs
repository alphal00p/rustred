//! LEGACY IMPORT WITH EVIDENCE BINDING (emission contract section 12; lane gencheck round 2, item 1): the class of a
//! record brought into a table UNCHANGED from an old one, derived from the record's own bytes and from the evidence
//! files alone. No class table is read: a class is never copied, it is decided here from
//!
//! - VERDICT ROWS (tab-separated files of a reviewing lane: one row per record of a REFERENCE table, its class text,
//!   detail text, optionally the record's relation / domain sha256 and a reference to a proof object (a markdown table
//!   of face verdicts whose exactness item is read)). Only a WHOLE-DOMAIN PROOF OBJECT gives a class (`whole domain`);
//!   a vacuous / dead verdict is NO evidence (a live record with one at its own position is a contradiction and is
//!   refused); a summary word without its exactness statement gives nothing; per-key verdict TEXT gives nothing either
//!   (its keys are not listed): per-key classes come from proof lines.
//! - PER-KEY PROOF LINES (one line per (rule, key): `rule`, `line_sha256` = sha256 of the rule's line, `key`, `status`):
//!   a line binds only to the record whose line bytes have that sha256 at that position.
//! - FIRST-MATCH COUNTS AND LIVENESS (one line per table position: `table_index`, `liveness` live / dead,
//!   `fm_keys_by_shell`, `fm_count_complete`, optionally `target_rhs_sha256` which must then be the record's).
//!
//! BINDING (the strict rule of 2026-11-18, TRICKS 67): a verdict row binds at the record's OWN position (reference rows:
//! the record's `origin.dispatch_position`; table rows: its position) iff its relation sha256 AND its domain sha256 are
//! the record's (the row's own values, else those of the reference record at the row's index, computed from the
//! reference table's bytes). A row at ANOTHER position binds only when it is a whole-domain proof object of the
//! IDENTICAL STATEMENT (equal relation and equal domain sha256: the same identity on the same domain); nothing else of
//! another position is used.
//!
//! CLASS: `whole domain` <- a bound whole-domain proof object; else (delivery word `dead in cone` when the counts say
//! dead) `every first-match key in cone` <- the proof lines bound to the record prove, shell by shell D + r <= bound, as
//! many distinct keys as the complete first-match count of that shell (and none in a shell the counts do not list);
//! else `sampled` <- N > 0 distinct proven keys; else none.
use super::body::sha256_hex;
use super::class::{ClassWord, ConeEvidence, SampleEvidence};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const SCHEMA: &str = "rustred.legacy-import.v1";
/// The delivery word of the sub-class "whole domain by a proof object of the identical statement at another position".
pub const IDENTICAL_WORD: &str = "whole domain, by identical statement";

/// What one verdict row says about exactness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerdictKind {
    /// a whole-domain proof object
    Whole,
    /// a per-key / sampled / face-restricted verdict (gives no class here: its keys are not listed)
    PerKey,
    /// a vacuous or dead verdict: no evidence of exactness
    Vacuous,
    None,
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `s` starts with `p` followed by a word boundary (end, or a non-word character).
fn starts_word(s: &str, p: &str) -> bool {
    s.strip_prefix(p)
        .is_some_and(|r| r.chars().next().is_none_or(|c| !is_word(c)))
}

/// `s` contains `p` preceded by a word boundary (start, or a non-word character).
fn contains_after_boundary(s: &str, p: &str) -> bool {
    s.match_indices(p)
        .any(|(i, _)| s[..i].chars().next_back().is_none_or(|c| !is_word(c)))
}

/// `s` contains `p` followed by a word boundary.
fn contains_before_boundary(s: &str, p: &str) -> bool {
    s.match_indices(p)
        .any(|(i, m)| s[i + m.len()..].chars().next().is_none_or(|c| !is_word(c)))
}

/// "dead at D + r" with optional single spaces around '+', case-insensitive (`low` is lower-cased).
fn dead_at_dr(low: &str) -> bool {
    low.match_indices("dead at d").any(|(i, m)| {
        let mut r = &low[i + m.len()..];
        r = r.strip_prefix(' ').unwrap_or(r);
        let Some(r) = r.strip_prefix('+') else {
            return false;
        };
        let r = r.strip_prefix(' ').unwrap_or(r);
        r.starts_with('r')
    })
}

fn vacuous(text: &str) -> bool {
    let low = text.to_ascii_lowercase();
    low.contains("vacuous")
        || low.contains("dead in cone")
        || dead_at_dr(&low)
        || low.contains("not-run-dead")
        || low.contains("out of scope: dead")
        || contains_after_boundary(&low, "0 first-match keys")
}

fn whole_class(cls: &str) -> bool {
    ["E-WHOLE", "POINT", "SYM", "whole domain"]
        .iter()
        .any(|p| starts_word(cls, p))
}

fn per_key_class(cls: &str) -> bool {
    if [
        "E-KEYS",
        "per-key certificates",
        "every first-match key in cone",
        "FACE-VERDICT",
    ]
    .iter()
    .any(|p| cls.starts_with(p))
    {
        return true;
    }
    let digits_then = |r: &str, tail: &str| {
        let n = r.bytes().take_while(u8::is_ascii_digit).count();
        n > 0 && r[n..].starts_with(tail)
    };
    cls.strip_prefix('K').is_some_and(|r| digits_then(r, " "))
        || cls
            .strip_prefix("sampled ")
            .is_some_and(|r| digits_then(r, " keys"))
}

/// ONE verdict row -> its kind (the frozen binding tool's rules of 2026-11-18: vacuous / dead first; whole only from a
/// whole-domain class word, or a FACE-VERDICT whose proof object states `PROVED whole domain` / `(E-WHOLE` and says
/// nothing about first-match keys; per-key class words -> PerKey).
pub fn classify_verdict(cls: &str, detail: &str) -> VerdictKind {
    let text = format!("{cls} || {detail}");
    if vacuous(&text) {
        return VerdictKind::Vacuous;
    }
    if whole_class(cls) {
        return VerdictKind::Whole;
    }
    let whole_text = detail.contains("PROVED whole domain")
        || detail.contains("(E-WHOLE:")
        || detail.contains("(E-WHOLE)");
    let fm_text = detail.contains("every FM key")
        || contains_before_boundary(detail, "FM keys")
        || detail.contains("first-match key");
    if cls.starts_with("FACE-VERDICT") && whole_text && !fm_text {
        return VerdictKind::Whole;
    }
    if per_key_class(cls) {
        return VerdictKind::PerKey;
    }
    VerdictKind::None
}

/// A markdown verdict table -> {record key: exactness statement} (header row, separator row, data rows; a non-table line
/// ends a table).
pub fn proof_objects(md: &str, key_col: &str, exact_col: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut hdr: Option<Vec<String>> = None;
    for line in md.lines() {
        if !line.starts_with('|') {
            hdr = None;
            continue;
        }
        let cells: Vec<String> = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(|c| c.trim().to_string())
            .collect();
        let Some(h) = &hdr else {
            hdr = Some(cells);
            continue;
        };
        if cells
            .concat()
            .chars()
            .all(|c| c == '-' || c == ':' || c == ' ')
        {
            continue;
        }
        let get = |name: &str| h.iter().position(|x| x == name).and_then(|i| cells.get(i));
        if let (Some(k), Some(e)) = (get(key_col), get(exact_col)) {
            out.insert(k.clone(), e.clone());
        }
    }
    out
}

/// How a verdict source numbers its rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyedBy {
    /// the index of the REFERENCE table (the record's `origin.dispatch_position` in the imported table)
    Reference,
    /// the position in the imported table itself
    Table,
}

/// A verdict source of the import configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerdictSource {
    pub name: String,
    pub path: String,
    pub keyed_by: KeyedBy,
    pub index_col: String,
    #[serde(default)]
    pub hash_col: Option<String>,
    #[serde(default)]
    pub domain_col: Option<String>,
    pub class_col: String,
    #[serde(default)]
    pub detail_cols: Vec<String>,
    /// a column `<doc> <record>` naming a row of a proof-object table
    #[serde(default)]
    pub proof_object_col: Option<String>,
}

/// A proof-object table of the configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofObjectSource {
    pub doc: String,
    pub path: String,
    pub key_col: String,
    pub exact_col: String,
}

/// A per-key proof-line source: every `*.jsonl` file of `dir` (or the one file `file`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyProofSource {
    pub name: String,
    #[serde(default)]
    pub dir: Option<String>,
    #[serde(default)]
    pub file: Option<String>,
}

/// A first-match count / liveness source (jsonl). Rows without `target_rhs_sha256` bind by position only, hence
/// `table_sha256` (the table the counts were made on) is then required and must be the imported table's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CountSource {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub table_sha256: Option<String>,
}

/// The import configuration (`rustred.legacy-import.v1`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: String,
    /// the reference table whose positions reference-keyed rows use (rule `index` members)
    #[serde(default)]
    pub reference_table: Option<String>,
    /// the region of the per-key and liveness evidence: D + r <= bound on the record's chart
    pub cone_bound: i64,
    #[serde(default)]
    pub verdicts: Vec<VerdictSource>,
    #[serde(default)]
    pub proof_objects: Vec<ProofObjectSource>,
    #[serde(default)]
    pub key_proofs: Vec<KeyProofSource>,
    /// later sources replace earlier ones position by position (a re-count)
    #[serde(default)]
    pub counts: Vec<CountSource>,
}

/// One verdict row with its identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerdictRow {
    pub source: String,
    pub source_sha256: String,
    /// 1-based line of the row in its source file
    pub line: u64,
    pub keyed_by: KeyedBy,
    pub index: i64,
    pub hash: Option<String>,
    pub domain: Option<String>,
    pub class: String,
    pub detail: String,
    pub kind: VerdictKind,
}

impl VerdictRow {
    /// The side-file form of a bound row (what table-check re-reads and re-classifies).
    pub fn side_json(&self, hash: &str, domain: &str) -> Value {
        json!({"schema": "rustred.legacy-import.verdict-row.v1", "source": self.source, "source_sha256": self.source_sha256,
               "line": self.line, "keyed_by": self.keyed_by, "index": self.index, "class": self.class,
               "detail": self.detail, "relation_sha256": hash, "domain_sha256": domain})
    }
}

/// Parse one tab-separated verdict source (`text` = its bytes as UTF-8, `sha` their sha256).
pub fn parse_verdicts(
    src: &VerdictSource,
    text: &str,
    sha: &str,
    pobj: &BTreeMap<(String, String), String>,
) -> Result<Vec<VerdictRow>, String> {
    let mut lines = text.lines().enumerate();
    let (_, hdr) = lines.next().ok_or_else(|| format!("{}: empty", src.name))?;
    let cols: Vec<&str> = hdr.split('\t').collect();
    let col = |n: &str| cols.iter().position(|c| *c == n);
    let ix =
        col(&src.index_col).ok_or_else(|| format!("{}: no column {}", src.name, src.index_col))?;
    let cl =
        col(&src.class_col).ok_or_else(|| format!("{}: no column {}", src.name, src.class_col))?;
    let hc = src.hash_col.as_deref().map(|c| col(c).ok_or(c)).transpose();
    let hc = hc.map_err(|c| format!("{}: no column {c}", src.name))?;
    let dc = src
        .domain_col
        .as_deref()
        .map(|c| col(c).ok_or(c))
        .transpose();
    let dc = dc.map_err(|c| format!("{}: no column {c}", src.name))?;
    let pc = src
        .proof_object_col
        .as_deref()
        .map(|c| col(c).ok_or(c))
        .transpose();
    let pc = pc.map_err(|c| format!("{}: no column {c}", src.name))?;
    let mut detail_ix = vec![];
    for d in &src.detail_cols {
        detail_ix.push(col(d).ok_or_else(|| format!("{}: no column {d}", src.name))?);
    }
    let mut out = vec![];
    for (ln, l) in lines {
        if l.is_empty() {
            continue;
        }
        let cells: Vec<&str> = l.split('\t').collect();
        let cell = |i: usize| cells.get(i).copied().unwrap_or("");
        let index: i64 = cell(ix)
            .trim()
            .parse()
            .map_err(|_| format!("{} line {}: no usable {}", src.name, ln + 1, src.index_col))?;
        let class = cell(cl).trim().to_string();
        let mut detail = detail_ix
            .iter()
            .map(|&i| cell(i))
            .collect::<Vec<_>>()
            .join(" | ");
        if let Some(p) = pc {
            let po = cell(p).trim();
            if let Some((doc, rec)) = po.split_once(' ')
                && let Some(st) = pobj.get(&(doc.to_string(), rec.to_string()))
            {
                detail.push_str(&format!(" || proof object {po}: {st}"));
            }
        }
        let opt = |c: Option<usize>| {
            c.map(|i| cell(i).trim().to_string())
                .filter(|s| !s.is_empty())
        };
        let kind = classify_verdict(&class, &detail);
        out.push(VerdictRow {
            source: src.name.clone(),
            source_sha256: sha.to_string(),
            line: ln as u64 + 1,
            keyed_by: src.keyed_by,
            index,
            hash: opt(hc),
            domain: opt(dc),
            class,
            detail,
            kind,
        });
    }
    Ok(out)
}

/// The identity of a record (imported or reference).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordId {
    pub position: u64,
    pub index: i64,
    pub dispatch_position: Option<i64>,
    pub relation_sha256: String,
    pub domain_sha256: String,
    pub line_sha256: String,
    pub chart: String,
}

/// Proof lines bound to one table position: line sha256 -> distinct keys proven EXACT on that line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyProofs {
    pub by_line: BTreeMap<String, BTreeSet<Vec<i64>>>,
    pub sources: BTreeSet<String>,
}

/// Parse proof lines (one JSON object per line) into `out[rule]`; only EXACT_IN_SPAN_AT_KEY lines count.
pub fn add_key_proofs(
    name: &str,
    text: &str,
    out: &mut BTreeMap<u64, KeyProofs>,
) -> Result<u64, String> {
    let mut n = 0;
    for (ln, l) in text.lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let v: Value =
            serde_json::from_str(l).map_err(|e| format!("{name} line {}: {e}", ln + 1))?;
        if v["status"].as_str() != Some("EXACT_IN_SPAN_AT_KEY") {
            continue;
        }
        let (Some(rule), Some(sha), Some(key)) = (
            v["rule"].as_u64(),
            v["line_sha256"].as_str(),
            v["key"].as_array(),
        ) else {
            return Err(format!(
                "{name} line {}: no rule / line_sha256 / key",
                ln + 1
            ));
        };
        let key: Option<Vec<i64>> = key.iter().map(Value::as_i64).collect();
        let key = key.ok_or_else(|| format!("{name} line {}: key not integer", ln + 1))?;
        let e = out.entry(rule).or_default();
        e.by_line.entry(sha.to_string()).or_default().insert(key);
        e.sources.insert(name.to_string());
        n += 1;
    }
    Ok(n)
}

/// First-match counts / liveness of one table position.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
    pub source: String,
    pub source_sha256: String,
    pub liveness: Option<String>,
    pub basis: Option<String>,
    pub relation_sha256: Option<String>,
    pub fm_keys: Option<u64>,
    pub fm_keys_by_shell: Option<BTreeMap<i64, u64>>,
    pub fm_count_complete: Option<bool>,
}

/// Parse a count source into `out[table_index]` (a later row replaces an earlier one).
pub fn add_counts(
    src: &CountSource,
    text: &str,
    sha: &str,
    out: &mut BTreeMap<u64, Counts>,
) -> Result<u64, String> {
    let mut n = 0;
    for (ln, l) in text.lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let v: Value =
            serde_json::from_str(l).map_err(|e| format!("{} line {}: {e}", src.name, ln + 1))?;
        let Some(ti) = v["table_index"].as_u64() else {
            return Err(format!("{} line {}: no table_index", src.name, ln + 1));
        };
        let by_shell = v["fm_keys_by_shell"].as_object().map(|o| {
            o.iter()
                .filter_map(|(k, c)| Some((k.parse::<i64>().ok()?, c.as_u64()?)))
                .collect::<BTreeMap<_, _>>()
        });
        let c = Counts {
            source: src.name.clone(),
            source_sha256: sha.to_string(),
            liveness: v["liveness"].as_str().map(str::to_string),
            basis: v["basis"].as_str().map(str::to_string),
            relation_sha256: v["target_rhs_sha256"].as_str().map(str::to_string),
            fm_keys: v["fm_keys"].as_u64(),
            fm_keys_by_shell: by_shell,
            fm_count_complete: v["fm_count_complete"].as_bool(),
        };
        if c.relation_sha256.is_none() && src.table_sha256.is_none() {
            return Err(format!(
                "{}: rows without target_rhs_sha256 bind by position only: state the table_sha256 they were made on",
                src.name
            ));
        }
        out.insert(ti, c);
        n += 1;
    }
    Ok(n)
}

/// D + r of a key on a chart mask ('1' = line: n - 1; '0' = numerator: -n).
pub fn dr(chart: &str, key: &[i64]) -> i64 {
    chart
        .bytes()
        .zip(key)
        .map(|(m, &x)| if m == b'1' { x - 1 } else { -x })
        .sum()
}

/// The sha256 of a key list (one compact JSON array per line, sorted), the `keys` side file.
pub fn keys_text(keys: &BTreeSet<Vec<i64>>) -> String {
    let mut s = String::new();
    for k in keys {
        s.push_str(&serde_json::to_string(k).unwrap_or_default());
        s.push('\n');
    }
    s
}

/// A bound whole-domain proof object and how it binds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundProof {
    pub row: VerdictRow,
    /// "own position" or "identical statement at <keyed_by> index <i>"
    pub binding: String,
}

/// The decision for one record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    pub class: ClassWord,
    /// the delivery's word: whole domain / dead in cone / every first-match key in cone / sampled N keys / none
    pub word: String,
    pub dead: Option<bool>,
    pub proof: Option<BoundProof>,
    pub cone: Option<ConeEvidence>,
    pub sample: Option<SampleEvidence>,
    /// the proven key set (side file) when cone / sample evidence is used
    pub keys: Option<BTreeSet<Vec<i64>>>,
    pub proofs_line_sha256: Option<String>,
    /// a LIVE record with a vacuous / dead verdict at its own position (refused)
    pub contradiction: bool,
    pub own_rows: usize,
    pub stale_rows: usize,
    pub notes: Vec<String>,
}

/// Decide one record. `own` = verdict rows at its own position; `same` = rows of other positions whose relation
/// sha256 equals the record's (by the row's own hash, or the reference record's); `ref_ids(index)` gives the reference
/// record identity for reference-keyed rows that lack a hash / domain.
pub fn decide(
    rec: &RecordId,
    own: &[&VerdictRow],
    same: &[&VerdictRow],
    ref_ids: &dyn Fn(i64) -> Option<(String, String)>,
    proofs: Option<&KeyProofs>,
    counts: Option<&Counts>,
    cone_bound: i64,
) -> Decision {
    let ident = |r: &VerdictRow| -> (Option<String>, Option<String>) {
        let rf = if r.keyed_by == KeyedBy::Reference {
            ref_ids(r.index)
        } else {
            None
        };
        (
            r.hash.clone().or_else(|| rf.as_ref().map(|x| x.0.clone())),
            r.domain
                .clone()
                .or_else(|| rf.as_ref().map(|x| x.1.clone())),
        )
    };
    let mut d = Decision {
        class: ClassWord::None,
        word: "none".into(),
        dead: None,
        proof: None,
        cone: None,
        sample: None,
        keys: None,
        proofs_line_sha256: None,
        contradiction: false,
        own_rows: 0,
        stale_rows: 0,
        notes: vec![],
    };
    let mut own_vacuous = false;
    for r in own {
        let (h, dm) = ident(r);
        let ok = h.as_deref() == Some(rec.relation_sha256.as_str())
            && dm.as_deref() == Some(rec.domain_sha256.as_str());
        if !ok {
            d.stale_rows += 1;
            continue;
        }
        d.own_rows += 1;
        if r.kind == VerdictKind::Vacuous {
            own_vacuous = true;
        }
        if r.kind == VerdictKind::Whole && d.proof.is_none() {
            d.proof = Some(BoundProof {
                row: (*r).clone(),
                binding: "own position".into(),
            });
        }
    }
    if d.proof.is_none() {
        for r in same {
            if r.kind != VerdictKind::Whole {
                continue;
            }
            let (h, dm) = ident(r);
            if h.as_deref() == Some(rec.relation_sha256.as_str())
                && dm.as_deref() == Some(rec.domain_sha256.as_str())
            {
                d.proof = Some(BoundProof {
                    row: (*r).clone(),
                    binding: format!(
                        "identical statement at {} index {}",
                        if r.keyed_by == KeyedBy::Reference {
                            "reference"
                        } else {
                            "table"
                        },
                        r.index
                    ),
                });
                break;
            }
        }
    }
    // counts bind by relation when they carry one
    let counts = counts.filter(|c| {
        c.relation_sha256
            .as_deref()
            .is_none_or(|h| h == rec.relation_sha256)
    });
    d.dead = counts.and_then(|c| match c.liveness.as_deref() {
        Some("dead") => Some(true),
        Some("live") => Some(false),
        _ => None,
    });
    d.contradiction = own_vacuous && d.dead == Some(false);
    // per-key: the proof lines made on THIS line
    let keys = proofs
        .and_then(|p| p.by_line.get(&rec.line_sha256))
        .cloned();
    if let Some(p) = proofs {
        let other = p.by_line.keys().filter(|s| **s != rec.line_sha256).count();
        if other > 0 {
            d.notes.push(format!(
                "{other} proof-line set(s) made on another line: not bound"
            ));
        }
    }
    if let Some(keys) = keys.filter(|k| !k.is_empty()) {
        let mut by_shell: BTreeMap<i64, u64> = BTreeMap::new();
        let mut cone_keys: BTreeSet<Vec<i64>> = BTreeSet::new();
        for k in &keys {
            let s = dr(&rec.chart, k);
            if s <= cone_bound {
                *by_shell.entry(s).or_default() += 1;
                cone_keys.insert(k.clone());
            }
        }
        let in_cone = cone_keys.len() as u64;
        let fm = counts.and_then(|c| {
            (c.fm_count_complete == Some(true))
                .then_some(c.fm_keys_by_shell.as_ref())
                .flatten()
        });
        let every = fm.is_some_and(|fm| {
            let want: BTreeMap<i64, u64> = fm
                .iter()
                .filter(|(s, n)| **n > 0 && **s <= cone_bound)
                .map(|(s, n)| (*s, *n))
                .collect();
            want == by_shell
        });
        if every {
            d.cone = Some(ConeEvidence {
                cone: format!("D + r <= {cone_bound}"),
                scope: "first-match keys at the position".into(),
                keys: in_cone,
                keys_sha256: sha256_hex(keys_text(&cone_keys).as_bytes()),
                complete: true,
                proven: in_cone,
                position: Some(rec.position),
                prefix_sha256: None,
            });
            d.keys = Some(cone_keys);
        } else {
            d.sample = Some(SampleEvidence {
                keys: keys.len() as u64,
                proven: keys.len() as u64,
                keys_sha256: sha256_hex(keys_text(&keys).as_bytes()),
                how: format!(
                    "per-key proof lines on this line ({} of them in D + r <= {cone_bound}); first-match count {}",
                    in_cone,
                    counts
                        .and_then(|c| c.fm_keys)
                        .map_or("unknown".into(), |n| n.to_string())
                ),
            });
            d.keys = Some(keys);
        }
        d.proofs_line_sha256 = Some(rec.line_sha256.clone());
    }
    d.class = if d.proof.is_some() {
        ClassWord::WholeDomain
    } else if d.cone.is_some() {
        ClassWord::EveryKeyInCone
    } else if d.sample.is_some() {
        ClassWord::Sampled
    } else {
        ClassWord::None
    };
    d.word = match d.class {
        // implementer's decision 2026-11-18T22:00Z: a proof object of the identical statement at ANOTHER position is a
        // visible sub-class, counted apart, until the checker has run on the record's own bytes
        ClassWord::WholeDomain
            if d.proof
                .as_ref()
                .is_some_and(|p| p.binding.starts_with("identical")) =>
        {
            let p = d.proof.as_ref().expect("proof");
            format!(
                "{IDENTICAL_WORD} at {} index {}",
                if p.row.keyed_by == KeyedBy::Reference {
                    "reference"
                } else {
                    "table"
                },
                p.row.index
            )
        }
        ClassWord::WholeDomain => "whole domain".into(),
        _ if d.dead == Some(true) => "dead in cone".into(),
        ClassWord::EveryKeyInCone => "every first-match key in cone".into(),
        ClassWord::Sampled => format!("sampled {} keys", d.sample.as_ref().map_or(0, |s| s.keys)),
        ClassWord::None => "none".into(),
    };
    d
}

/// The `legacy` member of the derivation's class (references only; the bound row lives in the side file).
pub fn legacy_member(
    d: &Decision,
    row_sha256: Option<&str>,
    counts: Option<&Counts>,
    cone_bound: i64,
) -> Value {
    // contract section 13: a legacy record brought in unchanged replaces nothing: its d = 4 regularity is not compared
    let mut m = json!({"schema": SCHEMA, "regularity_d4": "not compared (legacy record brought in unchanged)"});
    if let (Some(p), Some(sha)) = (&d.proof, row_sha256) {
        m["proof_object"] = json!({"row_sha256": sha, "binding": p.binding});
    }
    if let Some(s) = &d.proofs_line_sha256 {
        m["proofs_line_sha256"] = json!(s);
    }
    if let Some(c) = counts {
        m["liveness"] = json!({"cone": format!("D + r <= {cone_bound}"), "verdict": c.liveness, "basis": c.basis,
                               "source": c.source, "source_sha256": c.source_sha256});
    }
    m
}

/// Re-verify a bound row side file against a record body (table-check): its relation and domain are the record's and
/// its class + detail still classify as a whole-domain proof object.
pub fn verify_row(row: &Value, relation_sha256: &str, domain_sha256: &str) -> Result<(), String> {
    if row["relation_sha256"].as_str() != Some(relation_sha256) {
        return Err("the bound proof object is of another relation".into());
    }
    if row["domain_sha256"].as_str() != Some(domain_sha256) {
        return Err("the bound proof object is of another domain".into());
    }
    let k = classify_verdict(
        row["class"].as_str().unwrap_or(""),
        row["detail"].as_str().unwrap_or(""),
    );
    if k != VerdictKind::Whole {
        return Err(format!(
            "the bound row is not a whole-domain proof object ({k:?})"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emission::body::{attach_derivation, body_ids};
    use crate::emission::class::{Classed, Kind, Origin, Policy};
    use crate::emission::validate::check_record;
    use crate::emission::{AREA_EVID, AREA_KEYS, AREA_POLICY, Parts, derivation};

    #[test]
    fn verdict_kinds_follow_the_strict_binding_rules() {
        use VerdictKind::*;
        let k = |c: &str, d: &str| classify_verdict(c, d);
        assert_eq!(k("E-WHOLE", "support 1, 33 columns verified"), Whole);
        assert_eq!(k("E-WHOLE:", ""), Whole);
        assert_eq!(k("E-WHOLEX", ""), None);
        assert_eq!(k("SYM+POINT", ""), Whole);
        assert_eq!(k("SYMMETRY", ""), None);
        assert_eq!(k("POINT", ""), Whole);
        assert_eq!(k("whole domain", ""), Whole);
        assert_eq!(k("NOT-RUN-DEAD", ""), Vacuous);
        assert_eq!(k("SKIPPED", "out of scope: dead"), Vacuous);
        // a summary word is no proof of the whole domain
        assert_eq!(
            k("FACE-VERDICT", "PASS (1-3 whole domain; 4-5 per key)"),
            PerKey
        );
        assert_eq!(
            k(
                "FACE-VERDICT",
                "PASS || proof object faces.part1 12: PROVED whole domain"
            ),
            Whole
        );
        assert_eq!(k("FACE-VERDICT", "PASS (E-WHOLE)"), Whole);
        assert_eq!(
            k(
                "FACE-VERDICT",
                "PROVED whole domain at every FM key D + r <= 39"
            ),
            PerKey
        );
        assert_eq!(
            k("FACE-VERDICT", "PROVED whole domain on 12 FM keys"),
            PerKey
        );
        assert_eq!(
            k("FACE-VERDICT", "exactness vacuous: dead at D + r <= 39"),
            Vacuous
        );
        assert_eq!(k("FACE-VERDICT", "dead at D+r <= 39"), Vacuous);
        assert_eq!(k("E-KEYS", "EXACT at 494 keys"), PerKey);
        assert_eq!(k("K12 ok", ""), PerKey);
        assert_eq!(k("K12", ""), None);
        assert_eq!(k("sampled 494 keys", ""), PerKey);
        assert_eq!(k("x", "10 first-match keys"), None);
        assert_eq!(k("x", "has 0 first-match keys"), Vacuous);
        assert_eq!(k("OPEN", ""), None);
    }

    #[test]
    fn proof_object_tables_are_read_by_column_name() {
        let md = "text\n| record | 2 | 3 exactness |\n|---|---|:--|\n| 12 | a | PROVED whole domain |\n| 13 | b | vacuous |\nend\n| other | 3 exactness |\n|--|--|\n| 99 | x |\n";
        let m = proof_objects(md, "record", "3 exactness");
        assert_eq!(m.get("12").map(String::as_str), Some("PROVED whole domain"));
        assert_eq!(m.get("13").map(String::as_str), Some("vacuous"));
        assert!(!m.contains_key("99"));
    }

    fn row(
        keyed_by: KeyedBy,
        index: i64,
        h: Option<&str>,
        d: Option<&str>,
        class: &str,
        detail: &str,
    ) -> VerdictRow {
        VerdictRow {
            source: "A".into(),
            source_sha256: "00".into(),
            line: 1,
            keyed_by,
            index,
            hash: h.map(str::to_string),
            domain: d.map(str::to_string),
            class: class.into(),
            detail: detail.into(),
            kind: classify_verdict(class, detail),
        }
    }

    fn rec() -> RecordId {
        RecordId {
            position: 5,
            index: 5,
            dispatch_position: Some(7),
            relation_sha256: "R".into(),
            domain_sha256: "D".into(),
            line_sha256: "L".into(),
            chart: "110".into(),
        }
    }

    fn counts(live: &str, shells: &[(i64, u64)]) -> Counts {
        Counts {
            liveness: Some(live.into()),
            relation_sha256: Some("R".into()),
            fm_keys: Some(shells.iter().map(|x| x.1).sum()),
            fm_keys_by_shell: Some(shells.iter().copied().collect()),
            fm_count_complete: Some(true),
            ..Default::default()
        }
    }

    #[test]
    fn binding_needs_relation_and_domain_and_twins_transfer_only_whole_domain_proofs() {
        let none_ref = |_: i64| Option::<(String, String)>::None;
        let r = rec();
        // own position, hash and domain equal
        let a = row(KeyedBy::Reference, 7, Some("R"), Some("D"), "E-WHOLE", "");
        let d = decide(&r, &[&a], &[], &none_ref, None, None, 39);
        assert_eq!(
            (d.class, d.word.as_str()),
            (ClassWord::WholeDomain, "whole domain")
        );
        assert_eq!(d.proof.unwrap().binding, "own position");
        // own position, another domain: stale, no class
        let b = row(KeyedBy::Reference, 7, Some("R"), Some("D2"), "E-WHOLE", "");
        let d = decide(&r, &[&b], &[], &none_ref, None, None, 39);
        assert_eq!((d.class, d.stale_rows), (ClassWord::None, 1));
        // own position, the domain from the reference record
        let c = row(KeyedBy::Reference, 7, Some("R"), None, "E-WHOLE", "");
        let refd = |i: i64| (i == 7).then(|| ("R".to_string(), "D".to_string()));
        assert_eq!(
            decide(&r, &[&c], &[], &refd, None, None, 39).class,
            ClassWord::WholeDomain
        );
        // identical statement at another position: only a whole-domain proof object transfers
        let t1 = row(KeyedBy::Reference, 9, Some("R"), Some("D"), "E-WHOLE", "");
        let d = decide(&r, &[], &[&t1], &none_ref, None, None, 39);
        assert_eq!(d.class, ClassWord::WholeDomain);
        assert!(d.proof.unwrap().binding.starts_with("identical statement"));
        let t2 = row(
            KeyedBy::Reference,
            9,
            Some("R"),
            Some("D"),
            "E-KEYS",
            "EXACT at 494 keys",
        );
        let t3 = row(KeyedBy::Reference, 9, Some("R"), Some("D2"), "E-WHOLE", "");
        let t4 = row(
            KeyedBy::Reference,
            9,
            Some("R"),
            Some("D"),
            "FACE-VERDICT",
            "exactness vacuous: dead at D + r <= 39",
        );
        assert_eq!(
            decide(&r, &[], &[&t2, &t3, &t4], &none_ref, None, None, 39).class,
            ClassWord::None
        );
        // a live record with a vacuous verdict at its own position is a contradiction
        let v = row(
            KeyedBy::Reference,
            7,
            Some("R"),
            Some("D"),
            "NOT-RUN-DEAD",
            "",
        );
        let c1 = counts("live", &[(0, 1)]);
        assert!(decide(&r, &[&v], &[], &none_ref, None, Some(&c1), 39).contradiction);
        let c2 = counts("dead", &[]);
        let d = decide(&r, &[&v], &[], &none_ref, None, Some(&c2), 39);
        assert!(!d.contradiction);
        assert_eq!(d.word, "dead in cone");
    }

    #[test]
    fn per_key_classes_come_from_proof_lines_on_this_line_shell_by_shell() {
        let none_ref = |_: i64| Option::<(String, String)>::None;
        let r = rec();
        // chart 110: D + r = (k0 - 1) + (k1 - 1) - k2
        let mut p = BTreeMap::new();
        let t = "{\"rule\":5,\"line_sha256\":\"L\",\"key\":[1,1,0],\"status\":\"EXACT_IN_SPAN_AT_KEY\"}\n\
                 {\"rule\":5,\"line_sha256\":\"L\",\"key\":[2,1,0],\"status\":\"EXACT_IN_SPAN_AT_KEY\"}\n\
                 {\"rule\":5,\"line_sha256\":\"L\",\"key\":[1,2,0],\"status\":\"EXACT_IN_SPAN_AT_KEY\"}\n\
                 {\"rule\":5,\"line_sha256\":\"L\",\"key\":[1,1,-1],\"status\":\"NOT_DECIDED\"}\n";
        assert_eq!(add_key_proofs("x", t, &mut p).unwrap(), 3);
        let all = counts("live", &[(0, 1), (1, 2)]);
        let d = decide(&r, &[], &[], &none_ref, p.get(&5), Some(&all), 39);
        assert_eq!(
            (d.class, d.word.as_str()),
            (ClassWord::EveryKeyInCone, "every first-match key in cone")
        );
        assert_eq!(
            d.cone.as_ref().map(|c| (c.keys, c.proven, c.complete)),
            Some((3, 3, true))
        );
        // one more first-match key in shell 1 than proven: sampled
        let more = counts("live", &[(0, 1), (1, 3)]);
        let d = decide(&r, &[], &[], &none_ref, p.get(&5), Some(&more), 39);
        assert_eq!(
            (d.class, d.word.as_str()),
            (ClassWord::Sampled, "sampled 3 keys")
        );
        // an incomplete count never gives every
        let mut inc = all.clone();
        inc.fm_count_complete = Some(false);
        assert_eq!(
            decide(&r, &[], &[], &none_ref, p.get(&5), Some(&inc), 39).class,
            ClassWord::Sampled
        );
        // counts of another relation do not bind
        let mut other = all.clone();
        other.relation_sha256 = Some("R2".into());
        assert_eq!(
            decide(&r, &[], &[], &none_ref, p.get(&5), Some(&other), 39).class,
            ClassWord::Sampled
        );
        // proof lines made on another line do not bind
        let mut q = BTreeMap::new();
        add_key_proofs("x", &t.replace("\"L\"", "\"L2\""), &mut q).unwrap();
        let d = decide(&r, &[], &[], &none_ref, q.get(&5), Some(&all), 39);
        assert_eq!(d.class, ClassWord::None);
        assert_eq!(d.notes.len(), 1);
        // dead in cone wins over the per-key class in the delivery word, not over whole domain
        let dead = counts("dead", &[(0, 1), (1, 2)]);
        let d = decide(&r, &[], &[], &none_ref, p.get(&5), Some(&dead), 39);
        assert_eq!(
            (d.class, d.word.as_str()),
            (ClassWord::EveryKeyInCone, "dead in cone")
        );
        let w = row(KeyedBy::Reference, 7, Some("R"), Some("D"), "E-WHOLE", "");
        assert_eq!(
            decide(&r, &[&w], &[], &none_ref, p.get(&5), Some(&dead), 39).word,
            "whole domain"
        );
    }

    /// A point record's exactness comes only from a stored instance-span output bound to its body: a stated
    /// `exact_at_key` is not read; an output of another line is refused.
    #[test]
    fn point_records_need_a_stored_instance_span_output_of_their_own_body() {
        use crate::emission::{AREA_CHECK, AREA_DESCENT};
        let line = r#"{"type":"rule","chart":"110","index":0,"case":"Coordinate(P)","fixed":[1,1,0],"equations":[],"target":"I(1,1,0)","rhs":[["I(0,1,0)","1"]],"exceptions":[],"domain_bounds":null}"#;
        let ids = body_ids(line).unwrap();
        let policy = Policy::default();
        let pt = serde_json::to_vec(&policy).unwrap();
        let mk = |span: Option<(String, Vec<u8>)>, stated: bool| {
            let mut side: BTreeMap<(String, String), Vec<u8>> = BTreeMap::new();
            side.insert((AREA_POLICY.into(), sha256_hex(&pt)), pt.clone());
            // the engine's descent output on this one-term record (contract section 14): LOWER
            let dout = br#"{"failing_rules":[],"first":null,"not_lower":0,"order":{"name":"t"},"rules":1,"terms":1}"#.to_vec();
            let dsha = sha256_hex(&dout);
            side.insert((AREA_DESCENT.into(), dsha.clone()), dout);
            let sp = span.map(|(verdict, out)| {
                let sha = sha256_hex(&out);
                side.insert((AREA_CHECK.into(), sha.clone()), out);
                ("tool".to_string(), sha, verdict)
            });
            let parts = Parts {
                kind: Kind::Point,
                origin: Origin::Lift,
                body_sha256: ids.body_sha256.clone(),
                producer: None,
                certificate: None,
                check: Err("point record: instance-span check".into()),
                span: sp,
                descent: Some(("node".into(), dsha, "LOWER".into(), None)),
                symrows: None,
                classed: Classed {
                    class: ClassWord::WholeDomain,
                    reason: None,
                    evidence: vec!["exact_at_key"],
                },
                cone: None,
                sample: None,
                ladder: None,
                legacy: None,
                policy: (policy.name.clone(), sha256_hex(&pt)),
            };
            let mut d = derivation(&parts);
            if stated {
                d["class"]["exact_at_key"] = json!(true);
            }
            (attach_derivation(line, &d).unwrap(), side)
        };
        let span_out = |l: &str, verdict: &str, exact: bool| -> Vec<u8> {
            json!({"mode": "instance-span", "record_sha256": sha256_hex(l.as_bytes()), "keys": [{"status": "x"}],
                   "verdict": verdict, "counts": {"in_span": if exact {1} else {0}, "not_in_span": if exact {0} else {1}, "other": 0}})
            .to_string().into_bytes()
        };
        let no_alias = |_: i64| None;
        let look = |s: BTreeMap<(String, String), Vec<u8>>| {
            move |a: &str, h: &str| s.get(&(a.to_string(), h.to_string())).cloned()
        };
        let (l, s) = mk(
            Some((
                "EXACT_ALL_KEYS_IN_SPAN".into(),
                span_out(line, "EXACT_ALL_KEYS_IN_SPAN", true),
            )),
            false,
        );
        let v = check_record(&l, 0, &look(s), &no_alias);
        assert!(v.ok(), "{:?}", v.problems);
        assert_eq!(v.class, Some(ClassWord::WholeDomain));
        // a stated exact_at_key without any check output: not whole domain, refused
        let (l, s) = mk(None, true);
        assert!(!check_record(&l, 0, &look(s), &no_alias).ok());
        // an output made on another line: refused
        let other = line.replace("I(0,1,0)", "I(0,0,0)");
        let (l, s) = mk(
            Some((
                "EXACT_ALL_KEYS_IN_SPAN".into(),
                span_out(&other, "EXACT_ALL_KEYS_IN_SPAN", true),
            )),
            false,
        );
        assert!(!check_record(&l, 0, &look(s), &no_alias).ok());
        // not in span: not whole domain
        let (l, s) = mk(
            Some((
                "NOT_ALL_KEYS_IN_SPAN".into(),
                span_out(line, "NOT_ALL_KEYS_IN_SPAN", false),
            )),
            false,
        );
        let v = check_record(&l, 0, &look(s), &no_alias);
        assert!(!v.ok());
        assert_eq!(v.class, Some(ClassWord::None));
    }

    /// An imported record is accepted by the record-level table-check and refused when its bound row, its proof-line
    /// binding or its key list does not fit the body.
    #[test]
    fn imported_records_are_re_verified_by_table_check() {
        let line = r#"{"type":"rule","chart":"110","index":0,"case":"Coordinate(x)","fixed":[null,null,null],"equations":[],"target":"I(n0,n1,n2)","rhs":[["I(n0-1,n1,n2)","1"]],"exceptions":[],"domain_bounds":null,"origin":{"dispatch_position":7}}"#;
        let ids = body_ids(line).unwrap();
        let policy = Policy::default();
        let policy_text = serde_json::to_vec(&policy).unwrap();
        let mk = |row_json: Option<Value>,
                  cone: Option<ConeEvidence>,
                  keys: Option<&str>,
                  line_sha: Option<String>| {
            let mut side: BTreeMap<(String, String), Vec<u8>> = BTreeMap::new();
            side.insert(
                (AREA_POLICY.into(), sha256_hex(&policy_text)),
                policy_text.clone(),
            );
            let mut lm = json!({"schema": SCHEMA});
            let mut ev = vec![];
            if let Some(r) = &row_json {
                let b = serde_json::to_vec(r).unwrap();
                let sha = sha256_hex(&b);
                side.insert((AREA_EVID.into(), sha.clone()), b);
                lm["proof_object"] = json!({"row_sha256": sha, "binding": "own position"});
                ev.push("proof_object");
            }
            if let Some(k) = keys {
                side.insert(
                    (AREA_KEYS.into(), sha256_hex(k.as_bytes())),
                    k.as_bytes().to_vec(),
                );
                ev.push("cone");
            }
            if let Some(s) = line_sha {
                lm["proofs_line_sha256"] = json!(s);
            }
            let class = if row_json.is_some() {
                ClassWord::WholeDomain
            } else {
                ClassWord::EveryKeyInCone
            };
            let parts = Parts {
                kind: Kind::Legacy,
                origin: Origin::Import,
                body_sha256: ids.body_sha256.clone(),
                producer: None,
                certificate: None,
                check: Err("legacy record".into()),
                span: None,
                descent: None,
                symrows: None,
                classed: Classed {
                    class,
                    reason: None,
                    evidence: ev,
                },
                cone,
                sample: None,
                ladder: None,
                legacy: Some(lm),
                policy: (policy.name.clone(), sha256_hex(&policy_text)),
            };
            (attach_derivation(line, &derivation(&parts)).unwrap(), side)
        };
        let lookup = |s: &BTreeMap<(String, String), Vec<u8>>| {
            let s = s.clone();
            move |a: &str, h: &str| s.get(&(a.to_string(), h.to_string())).cloned()
        };
        let no_alias = |_: i64| None;
        let r = row(
            KeyedBy::Reference,
            7,
            Some(&ids.relation_sha256),
            Some(&ids.domain_sha256),
            "E-WHOLE",
            "",
        );
        let good = r.side_json(&ids.relation_sha256, &ids.domain_sha256);
        let (l, s) = mk(Some(good.clone()), None, None, None);
        let v = check_record(&l, 0, &lookup(&s), &no_alias);
        assert!(v.ok(), "{:?}", v.problems);
        assert_eq!(v.class, Some(ClassWord::WholeDomain));
        // a row of another domain: refused (and no whole domain)
        let bad = r.side_json(&ids.relation_sha256, "other");
        let (l, s) = mk(Some(bad), None, None, None);
        let v = check_record(&l, 0, &lookup(&s), &no_alias);
        assert!(!v.ok());
        assert_ne!(v.class, Some(ClassWord::WholeDomain));
        // a row whose text is a summary word only: refused
        let mut summ = good.clone();
        summ["class"] = json!("FACE-VERDICT");
        summ["detail"] = json!("PASS (1-3 whole domain)");
        let (l, s) = mk(Some(summ), None, None, None);
        assert!(!check_record(&l, 0, &lookup(&s), &no_alias).ok());
        // cone evidence: the proof lines must be of THIS body, the key list must be beside the table with its count
        let keys = "[1,1,0]\n[2,1,0]\n";
        let cone = ConeEvidence {
            cone: "D + r <= 39".into(),
            scope: "first-match keys at the position".into(),
            keys: 2,
            keys_sha256: sha256_hex(keys.as_bytes()),
            complete: true,
            proven: 2,
            position: Some(0),
            prefix_sha256: None,
        };
        let (l, s) = mk(
            None,
            Some(cone.clone()),
            Some(keys),
            Some(sha256_hex(line.as_bytes())),
        );
        let v = check_record(&l, 0, &lookup(&s), &no_alias);
        assert!(v.ok(), "{:?}", v.problems);
        assert_eq!(v.class, Some(ClassWord::EveryKeyInCone));
        let (l, s) = mk(
            None,
            Some(cone.clone()),
            Some(keys),
            Some("another line".into()),
        );
        assert!(!check_record(&l, 0, &lookup(&s), &no_alias).ok());
        let (l, s) = mk(
            None,
            Some(cone),
            Some("[1,1,0]\n"),
            Some(sha256_hex(line.as_bytes())),
        );
        assert!(!check_record(&l, 0, &lookup(&s), &no_alias).ok());
    }
}
