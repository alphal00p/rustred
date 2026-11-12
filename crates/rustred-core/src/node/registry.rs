//! Proof-bearing shared case registry of the parallel pieces of ONE node, explicit obligation records, a compact
//! indexed cover store, an obligation pool for dynamic work distribution, and the final join of a split node
//! (rr-registry, 2026-11-11; design risk found by Astra in the rr-consolidate registry).
//!
//! Soundness rules (coordinator note 2026-11-11T17:37Z):
//! * a case is discharged ONLY by a valid-domain proof: one accepted rule contains it and none of the rule's
//!   exceptional branches meets it (`covers`), or the union argument of `union_covered` (every piece of the case
//!   inside an exceptional branch is covered in turn, depth-bounded, intersections exact). Case containment in a
//!   sibling's rule case is never a discharge: it only LEASES the case (the piece postpones it, hoping the sibling's
//!   rules will prove it covered) and every lease is resolved by proof or returned to the piece's own queue;
//! * every run writes `F.obligations.json`: state `complete` (no open obligation) or `stopped` with the exact list
//!   of open obligations (pending / current / numerical / refused / deferred regions / unresolved leases), so a
//!   stopped run never reads like a closure;
//! * the join accepts runs only with the same mathematical key (presented family, chart, ONE integral order, full
//!   certificate = source rows + basis derivation), discharges every open obligation by an emptiness proof, by
//!   containment in the INITIAL case of ANOTHER run (an explicit dependency edge) or by a valid-domain proof against
//!   the joined rules, and rejects every strongly connected component of the run dependency graph.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, Read as _, Seek as _, Write as _};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::driver::{
    CaseSpec, build_case, compile_family, covers, parse_index_poly, union_covered,
};
use super::order::NodeOrder;
use crate::algebra::CoefficientPolynomial;
use crate::solver::{Case, CaseIntersectionLimits, SourceSystem};

pub const REGISTRY_SCHEMA: &str = "rustred.node.registry.v2";
pub const OBLIGATIONS_SCHEMA: &str = "rustred.node.obligations.v1";
pub const COVER_INDEX_SCHEMA: &str = "rustred.node.cover_index.v1";
pub const POOL_SCHEMA: &str = "rustred.node.pool.v1";
pub const JOIN_SCHEMA: &str = "rustred.node.join.v1";

// ---------------------------------------------------------------------------------------------------------------
// keys

/// blake3 (hex) of the parts, each terminated by a NUL byte.
pub fn hash_parts(parts: &[&[u8]]) -> String {
    let mut h = blake3::Hasher::new();
    for p in parts {
        h.update(p);
        h.update(&[0]);
    }
    h.finalize().to_hex().to_string()
}

/// Hash of the certificate rows of an `F.cert.jsonl` text: header, original source rows and basis derivation rows
/// (the full source basis; `cert_end` statistics are excluded).
pub fn cert_hash(text: &str) -> String {
    let mut h = blake3::Hasher::new();
    for l in text.lines() {
        if l.starts_with("{\"type\":\"cert_header\"")
            || l.starts_with("{\"type\":\"source_row\"")
            || l.starts_with("{\"type\":\"basis_row\"")
        {
            h.update(l.as_bytes());
            h.update(b"\n");
        }
    }
    h.finalize().to_hex().to_string()
}

/// Canonical JSON text (keys sorted, no spaces) of a JSON document; the text itself if it does not parse.
pub fn canonical_json(text: &str) -> String {
    serde_json::from_str::<serde_json::Value>(text)
        .map(|v| v.to_string())
        .unwrap_or_else(|_| text.to_string())
}

/// Mathematical key of one chart generation: presented family, chart, integral order descriptor (canonical JSON) and
/// the certificate hash ([`cert_hash`]). Rules of two runs may be joined only when their math keys agree.
pub fn math_key(presented: &str, chart: &str, descriptor: &str, cert: &str) -> String {
    hash_parts(&[
        b"rustred.node.math_key.v1",
        presented.as_bytes(),
        chart.as_bytes(),
        canonical_json(descriptor).as_bytes(),
        cert.as_bytes(),
    ])
}

/// Math key of a finished run directory (`F.table.jsonl` header + chart record, `F.cert.jsonl`).
pub fn run_math_key(dir: &Path) -> Result<(String, RunHeader), String> {
    let h = read_run_header(&dir.join("F.table.jsonl"))?;
    let cert = std::fs::read_to_string(dir.join("F.cert.jsonl"))
        .map_err(|e| format!("{}: {e}", dir.join("F.cert.jsonl").display()))?;
    let key = math_key(&h.presented, &h.chart, &h.descriptor, &cert_hash(&cert));
    Ok((key, h))
}

/// The parts of a table header the join needs.
#[derive(Clone, Debug, Default, Serialize)]
pub struct RunHeader {
    pub presented: String,
    pub chart: String,
    pub descriptor: String,
    pub order_name: String,
    pub node_order: serde_json::Value,
}

fn read_run_header(table: &Path) -> Result<RunHeader, String> {
    let f = std::fs::File::open(table).map_err(|e| format!("{}: {e}", table.display()))?;
    let mut rd = std::io::BufReader::new(f);
    let mut h = RunHeader::default();
    let mut line = String::new();
    for _ in 0..4 {
        line.clear();
        if rd.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            break;
        }
        let v: serde_json::Value = serde_json::from_str(line.trim_end())
            .map_err(|e| format!("{}: {e}", table.display()))?;
        match v["type"].as_str() {
            Some("header") => {
                h.presented = v["family_source"].as_str().unwrap_or_default().to_string();
                h.descriptor = v["order"].to_string();
                h.order_name = v["order_name"].as_str().unwrap_or_default().to_string();
                h.node_order = v["node_order"].clone();
            }
            Some("chart") => {
                h.chart = v["chart"].as_str().unwrap_or_default().to_string();
                break;
            }
            _ => {}
        }
    }
    if h.presented.is_empty() || h.chart.is_empty() {
        return Err(format!("{}: no header / chart record", table.display()));
    }
    Ok(h)
}

// ---------------------------------------------------------------------------------------------------------------
// registry

/// One accepted sibling rule as a cover entry: (label, case, exceptional branches as index-polynomial strings).
pub type SiblingRule = (String, CaseSpec, Vec<Vec<String>>);

/// Append-only shared registry of the pieces of one node. Each piece writes its own epoch file
/// `<dir>/<piece>.<epoch>.reg.jsonl` (created new, never truncated): a header binding the registry identity (math key,
/// seed schedules, backend and engine version) followed by lifecycle records (`rule` accepted, `lease`,
/// `lease_resolved`, `lease_returned`, `end`). Siblings with another identity are ignored.
pub struct Registry {
    dir: PathBuf,
    own: PathBuf,
    file: std::fs::File,
    pub piece: String,
    pub epoch: String,
    pub identity: String,
    offsets: BTreeMap<PathBuf, u64>,
    foreign: BTreeSet<PathBuf>,
    accepted_files: BTreeSet<PathBuf>,
    /// sibling (piece.epoch) -> ended (its `end` record seen)
    pub siblings: BTreeMap<String, bool>,
}

impl Registry {
    pub fn open(
        dir: &str,
        piece: &str,
        identity: &str,
        math_key: &str,
        provenance: serde_json::Value,
    ) -> Result<Self, String> {
        if piece.is_empty() || piece.contains(['/', '.']) {
            return Err(format!("registry piece name '{piece}': no '/' or '.'"));
        }
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).map_err(|e| format!("registry {}: {e}", dir.display()))?;
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let epoch = format!("{ms}-{}", std::process::id());
        let own = dir.join(format!("{piece}.{epoch}.reg.jsonl"));
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .append(true)
            .open(&own)
            .map_err(|e| format!("registry {}: {e}", own.display()))?;
        let header = serde_json::json!({
            "type": "registry_header", "schema": REGISTRY_SCHEMA, "piece": piece, "epoch": epoch,
            "identity": identity, "math_key": math_key, "provenance": provenance,
        });
        writeln!(file, "{header}").map_err(|e| e.to_string())?;
        file.flush().map_err(|e| e.to_string())?;
        Ok(Self {
            dir,
            own,
            file,
            piece: piece.to_string(),
            epoch,
            identity: identity.to_string(),
            offsets: Default::default(),
            foreign: Default::default(),
            accepted_files: Default::default(),
            siblings: Default::default(),
        })
    }

    fn write(&mut self, v: serde_json::Value) {
        // one write per complete line (O_APPEND, one writer per file); readers take complete lines only
        let _ = self.file.write_all(format!("{v}\n").as_bytes());
        let _ = self.file.flush();
    }

    /// An accepted rule of this piece (its case and full exceptional branches).
    pub fn publish_rule(&mut self, index: usize, spec: &CaseSpec, exceptions: Vec<Vec<String>>) {
        self.write(serde_json::json!({
            "type": "rule", "state": "accepted", "index": index, "fixed": spec.fixed,
            "equations": spec.equations, "exceptions": exceptions,
        }));
    }

    /// A lifecycle record (lease, lease_resolved, lease_returned, end, ...).
    pub fn record(&mut self, mut v: serde_json::Value) {
        v["piece"] = self.piece.clone().into();
        self.write(v);
    }

    /// Own label `piece.epoch`.
    pub fn label(&self) -> String {
        format!("{}.{}", self.piece, self.epoch)
    }

    /// New accepted rules of sibling epoch files with the same identity. Returns them and the newly found foreign
    /// files (other identity: other family/chart/order/basis/certificate, seed schedule or engine).
    pub fn poll(&mut self) -> (Vec<SiblingRule>, Vec<PathBuf>) {
        let mut out = Vec::new();
        let mut foreign = Vec::new();
        let Ok(rd) = std::fs::read_dir(&self.dir) else {
            return (out, foreign);
        };
        let mut files: Vec<PathBuf> = rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.to_string_lossy().ends_with(".reg.jsonl") && *p != self.own)
            .collect();
        files.sort();
        for path in files {
            if self.foreign.contains(&path) {
                continue;
            }
            let off = *self.offsets.get(&path).unwrap_or(&0);
            let Ok(mut f) = std::fs::File::open(&path) else {
                continue;
            };
            if f.seek(std::io::SeekFrom::Start(off)).is_err() {
                continue;
            }
            let mut bytes = Vec::new();
            if f.read_to_end(&mut bytes).is_err() || bytes.is_empty() {
                continue;
            }
            let Some(last_nl) = bytes.iter().rposition(|b| *b == b'\n') else {
                continue;
            };
            let complete = &bytes[..=last_nl];
            self.offsets
                .insert(path.clone(), off + complete.len() as u64);
            for line in String::from_utf8_lossy(complete).lines() {
                let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                    continue;
                };
                match v["type"].as_str() {
                    Some("registry_header") => {
                        if v["identity"].as_str() != Some(self.identity.as_str())
                            || v["schema"].as_str() != Some(REGISTRY_SCHEMA)
                        {
                            self.foreign.insert(path.clone());
                            foreign.push(path.clone());
                            break;
                        }
                        self.accepted_files.insert(path.clone());
                        let lbl = format!(
                            "{}.{}",
                            v["piece"].as_str().unwrap_or("?"),
                            v["epoch"].as_str().unwrap_or("?")
                        );
                        self.siblings.entry(lbl).or_insert(false);
                    }
                    Some("rule") if self.accepted_files.contains(&path) => {
                        if v["state"].as_str() != Some("accepted") {
                            continue;
                        }
                        let stem = path
                            .file_name()
                            .map(|s| {
                                s.to_string_lossy()
                                    .trim_end_matches(".reg.jsonl")
                                    .to_string()
                            })
                            .unwrap_or_default();
                        let spec = CaseSpec {
                            fixed: v["fixed"]
                                .as_array()
                                .map(|a| a.iter().map(|x| x.as_i64()).collect())
                                .unwrap_or_default(),
                            equations: str_list(&v["equations"]),
                        };
                        let ex: Vec<Vec<String>> = v["exceptions"]
                            .as_array()
                            .map(|bs| bs.iter().map(str_list).collect())
                            .unwrap_or_default();
                        out.push((format!("registry:{stem}#{}", v["index"]), spec, ex));
                    }
                    Some("end") if self.accepted_files.contains(&path) => {
                        let stem = path
                            .file_name()
                            .map(|s| {
                                s.to_string_lossy()
                                    .trim_end_matches(".reg.jsonl")
                                    .to_string()
                            })
                            .unwrap_or_default();
                        self.siblings.insert(stem, true);
                    }
                    _ => {}
                }
            }
        }
        (out, foreign)
    }

    /// Every sibling seen so far has written its `end` record.
    pub fn siblings_ended(&self) -> bool {
        self.siblings.values().all(|e| *e)
    }
}

fn str_list(v: &serde_json::Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------------------------
// obligations

/// One open obligation of a run (PRESENTED slots). `kind`: `pending`, `current`, `numerical`, `refused`,
/// `region` (deferred exceptional region: the case is its chart, `detail.conjunction` the unresolved equations),
/// `branch` (failed branch admission: the case is the rule case), `leased` (postponed sibling-contained case).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Obligation {
    pub kind: String,
    pub case: CaseSpec,
    #[serde(default)]
    pub detail: serde_json::Value,
}

/// `F.obligations.json` of a run: its initial cases (what it owns), its state and its open obligations.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Obligations {
    pub schema: String,
    pub math_key: String,
    #[serde(default)]
    pub identity: Option<String>,
    /// `complete` (no open obligation) or `stopped`.
    pub state: String,
    #[serde(default)]
    pub stop: Option<String>,
    pub chart: String,
    /// Initial cases (PRESENTED slots) this run closes modulo `open`; pool claims included.
    pub initial: Vec<CaseSpec>,
    #[serde(default)]
    pub pool_claims: Vec<String>,
    pub open: Vec<Obligation>,
    pub rules: usize,
    #[serde(default)]
    pub residual_keys: Vec<String>,
    #[serde(default)]
    pub source: String,
}

impl Obligations {
    pub fn read(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let o: Self =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        if o.schema != OBLIGATIONS_SCHEMA {
            return Err(format!(
                "{}: schema {} != {OBLIGATIONS_SCHEMA}",
                path.display(),
                o.schema
            ));
        }
        if o.state == "complete" && !o.open.is_empty() {
            return Err(format!(
                "{}: state complete with {} open obligations",
                path.display(),
                o.open.len()
            ));
        }
        Ok(o)
    }

    pub fn write(&self, path: &Path) -> Result<(), String> {
        let tmp = path.with_extension("json.part");
        std::fs::write(
            &tmp,
            serde_json::to_string_pretty(self).map_err(|e| e.to_string())? + "\n",
        )
        .map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// pool (dynamic work distribution of a split: one claim per item, atomic file creation)

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PoolItem {
    pub id: String,
    /// PRESENTED slots.
    pub case: CaseSpec,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Pool {
    pub schema: String,
    /// Math key of the runs that may claim from this pool (`None` = unchecked).
    #[serde(default)]
    pub math_key: Option<String>,
    pub items: Vec<PoolItem>,
}

pub struct PoolClaimer {
    dir: PathBuf,
    pool: Pool,
    next: usize,
    piece: String,
}

impl PoolClaimer {
    pub fn open(dir: &str, piece: &str, math_key: &str) -> Result<Self, String> {
        let dir = PathBuf::from(dir);
        let text = std::fs::read_to_string(dir.join("POOL.json"))
            .map_err(|e| format!("pool {}: {e}", dir.display()))?;
        let pool: Pool = serde_json::from_str(&text).map_err(|e| format!("pool: {e}"))?;
        if pool.schema != POOL_SCHEMA {
            return Err(format!("pool schema {} != {POOL_SCHEMA}", pool.schema));
        }
        if let Some(k) = &pool.math_key {
            if k != math_key {
                return Err(format!(
                    "pool math key {k} != this run's {math_key} (other family/chart/order/certificate)"
                ));
            }
        }
        std::fs::create_dir_all(dir.join("claims")).map_err(|e| e.to_string())?;
        Ok(Self {
            dir,
            pool,
            next: 0,
            piece: piece.to_string(),
        })
    }

    /// The next unclaimed item, claimed atomically (`claims/<id>` created new).
    pub fn claim(&mut self) -> Option<PoolItem> {
        while self.next < self.pool.items.len() {
            let item = self.pool.items[self.next].clone();
            self.next += 1;
            let path = self.dir.join("claims").join(&item.id);
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
            {
                let _ = writeln!(f, "{}", self.piece);
                return Some(item);
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------------------------------------------
// compact cover store

/// Case and exceptional branches of one table rule line (the only fields a cover check reads).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RuleShape {
    #[serde(rename = "type", default)]
    pub ty: String,
    #[serde(default)]
    pub index: Option<u64>,
    #[serde(default)]
    pub fixed: Vec<Option<i64>>,
    #[serde(default)]
    pub equations: Vec<String>,
    #[serde(default)]
    pub exceptions: Vec<Vec<String>>,
}

fn head_hash(path: &Path) -> Result<(u64, String), String> {
    let f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let bytes = f.metadata().map(|m| m.len()).unwrap_or(0);
    let mut rd = std::io::BufReader::new(f);
    let mut line = String::new();
    rd.read_line(&mut line).map_err(|e| e.to_string())?;
    Ok((bytes, hash_parts(&[line.trim_end().as_bytes()])))
}

/// Rule shapes of a table (`upto` = rules 0..upto) or of its compact cover index, streamed line by line (the
/// coefficients are skipped by the typed parser, never kept). An index is bound to its table: if the table still
/// exists, its size and header hash must match.
pub fn read_rule_shapes(
    path: &str,
    upto: usize,
) -> Result<(Vec<RuleShape>, serde_json::Value), String> {
    let f = std::fs::File::open(path).map_err(|e| format!("cover {path}: {e}"))?;
    let mut rd = std::io::BufReader::with_capacity(1 << 20, f);
    let mut first = String::new();
    rd.read_line(&mut first)
        .map_err(|e| format!("cover {path}: {e}"))?;
    let head: serde_json::Value = serde_json::from_str(first.trim_end()).unwrap_or_default();
    let mut out = Vec::new();
    if head["type"] == "cover_index" {
        if head["schema"].as_str() != Some(COVER_INDEX_SCHEMA) {
            return Err(format!("cover index {path}: schema"));
        }
        if let Some(t) = head["table"].as_str() {
            if Path::new(t).exists() {
                let (bytes, hh) = head_hash(Path::new(t))?;
                if Some(bytes) != head["table_bytes"].as_u64()
                    || Some(hh.as_str()) != head["table_head_hash"].as_str()
                {
                    return Err(format!("cover index {path} is stale: table {t} changed"));
                }
            }
        }
        if head["upto"].as_u64().is_some_and(|u| (u as usize) < upto) {
            return Err(format!(
                "cover index {path} holds rules < {} only (asked {upto})",
                head["upto"]
            ));
        }
    }
    let mut line = String::new();
    loop {
        line.clear();
        if rd
            .read_line(&mut line)
            .map_err(|e| format!("cover {path}: {e}"))?
            == 0
        {
            break;
        }
        if !line.starts_with("{\"type\":\"rule\"") {
            continue;
        }
        let s: RuleShape =
            serde_json::from_str(line.trim_end()).map_err(|e| format!("cover {path}: {e}"))?;
        if s.index.is_none_or(|i| i as usize >= upto) {
            continue;
        }
        out.push(s);
    }
    Ok((out, head))
}

/// Write the compact cover index of `table` (rules 0..upto): header binding the table (path, size, header hash,
/// math key when `F.cert.jsonl` is next to it) + one `rule` line per rule with case and exceptions only.
pub fn write_cover_index(table: &str, upto: usize, out: &str) -> Result<usize, String> {
    let (shapes, _) = read_rule_shapes(table, upto)?;
    let (bytes, hh) = head_hash(Path::new(table))?;
    let mkey = Path::new(table)
        .parent()
        .and_then(|d| run_math_key(d).ok())
        .map(|(k, _)| k);
    let tmp = format!("{out}.part");
    let mut f =
        std::io::BufWriter::new(std::fs::File::create(&tmp).map_err(|e| format!("{tmp}: {e}"))?);
    let header = serde_json::json!({
        "type": "cover_index", "schema": COVER_INDEX_SCHEMA, "table": table, "table_bytes": bytes,
        "table_head_hash": hh, "upto": upto, "rules": shapes.len(), "math_key": mkey,
    });
    writeln!(f, "{header}").map_err(|e| e.to_string())?;
    for s in &shapes {
        let mut s = s.clone();
        s.ty = "rule".into();
        writeln!(
            f,
            "{}",
            serde_json::to_string(&s).map_err(|e| e.to_string())?
        )
        .map_err(|e| e.to_string())?;
    }
    f.flush().map_err(|e| e.to_string())?;
    drop(f);
    std::fs::rename(&tmp, out).map_err(|e| format!("{out}: {e}"))?;
    Ok(shapes.len())
}

/// Case and exceptional branches of a rule shape in the solver's index variables.
pub(super) fn shape_entry<const N: usize>(
    spec: &CaseSpec,
    exceptions: &[Vec<String>],
    template: &CoefficientPolynomial,
    idx: &[usize; N],
    sector: &[bool; N],
    n: usize,
) -> Option<(Case<N>, Vec<Vec<CoefficientPolynomial>>)> {
    let case = build_case::<N>(spec, template, idx, sector, n)
        .ok()
        .flatten()?;
    let mut branches = Vec::with_capacity(exceptions.len());
    for b in exceptions {
        let mut polys = Vec::with_capacity(b.len());
        for p in b {
            polys.push(parse_index_poly(p, template, idx)?);
        }
        branches.push(polys);
    }
    Some((case, branches))
}

// ---------------------------------------------------------------------------------------------------------------
// solver context of a chart (index variables, sector, coefficient template) for the offline tools

struct Ctx<const N: usize> {
    n: usize,
    idx: [usize; N],
    sector: [bool; N],
    template: CoefficientPolynomial,
    label: String,
}

fn context<const N: usize>(
    fam: &crate::family::IntegralFamily,
    presented: &str,
    chart: Option<&str>,
) -> Result<Ctx<N>, String> {
    let n = fam.denominator_count();
    let sources = SourceSystem::<N>::from_family_with_capacity(fam, true)
        .map_err(|e| format!("sources: {e:?}"))?;
    let flags = super::order::physical_slots(presented);
    let base: [bool; N] = match chart {
        Some(m) => {
            let b: Vec<bool> = m.chars().map(|c| c == '1').collect();
            if b.len() != n {
                return Err(format!("chart {m}: arity"));
            }
            std::array::from_fn(|i| i < n && b[i])
        }
        None => std::array::from_fn(|i| i < n && flags[i]),
    };
    let idx: [usize; N] = *sources.index_variables();
    let template = sources
        .rows()
        .iter()
        .flat_map(|r| r.iter())
        .next()
        .ok_or("empty source system")?
        .coefficient
        .clone();
    let label: String = base[..n]
        .iter()
        .map(|b| if *b { '1' } else { '0' })
        .collect();
    // the solver's case sector is the chart's sign pattern (driver: solver.ordering().sector() of the chart)
    Ok(Ctx {
        n,
        idx,
        sector: base,
        template,
        label,
    })
}

macro_rules! dispatch {
    ($n:expr, $f:ident, $($arg:expr),*) => {
        match $n {
            5 => $f::<5>($($arg),*),
            9 => $f::<9>($($arg),*),
            12 => $f::<12>($($arg),*),
            14 => $f::<14>($($arg),*),
            20 => $f::<20>($($arg),*),
            other => Err(format!("arity {other}: node tools compiled for 5, 9, 12, 14, 20")),
        }
    };
}

// ---------------------------------------------------------------------------------------------------------------
// obligations of an older (stopped) run, reconstructed from its queue log

/// Rebuild the open obligations of a stopped run from its `F.queue.jsonl` by replaying the queue (insertion in
/// `queue_cmp` order, the silent pruning of queued cases contained in a new candidate, pops). Every pop recorded in
/// the log must be the head of the simulated queue: the reconstruction is exact or it fails.
pub fn obligations_from_queue(
    family_text: &str,
    order: &NodeOrder,
    run_dir: &Path,
) -> Result<Obligations, String> {
    let presented = order.present_family(family_text)?;
    let fam = compile_family(&presented)?;
    let n = fam.denominator_count();
    dispatch!(n, reconstruct, &fam, &presented, run_dir)
}

fn reconstruct<const N: usize>(
    fam: &crate::family::IntegralFamily,
    presented: &str,
    run_dir: &Path,
) -> Result<Obligations, String> {
    let (key, header) = run_math_key(run_dir)?;
    if header.presented != presented {
        return Err(
            "the run's presented family differs from the family under the given order".into(),
        );
    }
    let ctx = context::<N>(fam, presented, Some(&header.chart))?;
    let n = ctx.n;
    let qpath = run_dir.join("F.queue.jsonl");
    let f = std::fs::File::open(&qpath).map_err(|e| format!("{}: {e}", qpath.display()))?;
    let rd = std::io::BufReader::with_capacity(1 << 20, f);
    let mut pending: Vec<(Case<N>, CaseSpec)> = Vec::new();
    let mut numerical: Vec<CaseSpec> = Vec::new();
    let mut regions: Vec<Obligation> = Vec::new();
    let mut in_flight: Option<CaseSpec> = None;
    let mut refused: Vec<Obligation> = Vec::new();
    let mut leased: Vec<Obligation> = Vec::new();
    let mut initial: Vec<CaseSpec> = Vec::new();
    let (mut pops, mut rules, mut lines) = (0usize, 0usize, 0usize);
    let spec_of = |v: &serde_json::Value| -> CaseSpec {
        CaseSpec {
            fixed: v["fixed"]
                .as_array()
                .map(|a| a.iter().map(|x| x.as_i64()).collect())
                .unwrap_or_default(),
            equations: str_list(&v["equations"]),
        }
    };
    for line in rd.lines() {
        let line = line.map_err(|e| e.to_string())?;
        lines += 1;
        let v: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue, // a torn last line of a killed run
        };
        match v["type"].as_str() {
            Some("initial") => initial.push(spec_of(&v["case"])),
            Some("pool_claim") => initial.push(spec_of(&v["case"])),
            Some("lease") => {
                let spec = spec_of(&v["case"]);
                if let Some(p) = pending.iter().position(|(_, s)| *s == spec) {
                    pending.remove(p);
                    pops += 1;
                }
                leased.push(Obligation { kind: "leased".into(), case: spec, detail: serde_json::json!({"lessor": v["lessor"]}) });
            }
            Some("lease_resolved") | Some("lease_returned") => {
                let spec = spec_of(&v["case"]);
                leased.retain(|o| o.case != spec);
            }
            Some("guard_timeout") => {
                if let Some(s) = in_flight.take() {
                    refused.push(Obligation { kind: "refused".into(), case: s, detail: serde_json::json!({"reason": "guard timeout"}) });
                }
            }
            Some("enqueue") => {
                let spec = spec_of(&v["case"]);
                let case = build_case::<N>(&spec, &ctx.template, &ctx.idx, &ctx.sector, n)?
                    .ok_or_else(|| format!("queue line {lines}: enqueued case is empty"))?;
                pending.retain(|(c, _)| !case.contains(c).unwrap_or(false));
                match pending.binary_search_by(|(q, _)| q.queue_cmp(&case)) {
                    Ok(_) => return Err(format!("queue line {lines}: duplicate enqueue")),
                    Err(p) => pending.insert(p, (case, spec)),
                }
            }
            Some("enqueue_numerical") => {
                let spec = spec_of(&v["case"]);
                let case = build_case::<N>(&spec, &ctx.template, &ctx.idx, &ctx.sector, n)?
                    .ok_or_else(|| format!("queue line {lines}: numerical case is empty"))?;
                pending.retain(|(c, _)| !case.contains(c).unwrap_or(false));
                if !numerical.contains(&spec) {
                    numerical.push(spec);
                }
            }
            Some("case_started") => {
                let spec = spec_of(&v["case"]);
                if pending.first().map(|(_, s)| s) != Some(&spec) {
                    return Err(format!(
                        "queue line {lines}: popped case is not the head of the reconstructed queue (pending {})",
                        pending.len()
                    ));
                }
                pending.remove(0);
                pops += 1;
                in_flight = Some(spec);
            }
            Some("empty_domain") | Some("finite_domain") | Some("bank_adopted") => {
                let spec = spec_of(&v["case"]);
                if let Some(p) = pending.iter().position(|(_, s)| *s == spec) {
                    pending.remove(p);
                    pops += 1;
                } else {
                    return Err(format!("queue line {lines}: popped case not queued"));
                }
            }
            Some("covered") if v.get("by_registry").is_some() => {
                let spec = spec_of(&v["case"]);
                if let Some(p) = pending.iter().position(|(_, s)| *s == spec) {
                    pending.remove(p);
                    pops += 1;
                }
            }
            Some("rule") => {
                if v["numerical"].as_bool() == Some(true) {
                    if let Some(fx) = v["fixed"].as_array() {
                        let s = CaseSpec {
                            fixed: fx.iter().map(|x| x.as_i64()).collect(),
                            equations: vec![],
                        };
                        numerical.retain(|x| *x != s);
                    }
                } else {
                    in_flight = None;
                }
                rules += 1;
            }
            Some("numerical_covered") => {
                if let Some(fx) = v["fixed"].as_array() {
                    let s = CaseSpec {
                        fixed: fx.iter().map(|x| x.as_i64()).collect(),
                        equations: vec![],
                    };
                    numerical.retain(|x| *x != s);
                }
            }
            Some("deferred_region") => regions.push(Obligation {
                kind: "region".into(),
                case: spec_of(&v["chart"]),
                detail: serde_json::json!({"rule": v["rule"], "branch": v["branch"], "k": v["k"], "reason": v["reason"],
                    "conjunction": v["conjunction"], "exclusions": null, "logged_envelope": true}),
            }),
            Some("deferred_branch") => regions.push(Obligation {
                kind: "branch".into(),
                case: CaseSpec::default(),
                detail: serde_json::json!({"rule": v["rule"], "branch": v["branch"], "polys": v["polys"],
                    "note": "rule case not in the log line: take it from the table rule"}),
            }),
            Some("lift_refused") => {
                if let Some(s) = in_flight.take() {
                    refused.push(Obligation { kind: "refused".into(), case: s, detail: serde_json::json!({"reason": "lift refused"}) });
                }
            }
            _ => {}
        }
    }
    // a 'branch' obligation's case is its rule's case: look the rules up in the table (streamed)
    let need: BTreeSet<u64> = regions
        .iter()
        .filter(|o| o.kind == "branch")
        .filter_map(|o| o.detail["rule"].as_u64())
        .collect();
    if !need.is_empty() {
        let (shapes, _) = read_rule_shapes(
            &run_dir.join("F.table.jsonl").display().to_string(),
            usize::MAX,
        )?;
        for o in regions.iter_mut().filter(|o| o.kind == "branch") {
            if let Some(s) = o.detail["rule"]
                .as_u64()
                .and_then(|r| shapes.iter().find(|s| s.index == Some(r)))
            {
                o.case = CaseSpec {
                    fixed: s.fixed.clone(),
                    equations: s.equations.clone(),
                };
            }
        }
    }
    let mut open: Vec<Obligation> = Vec::new();
    if let Some(s) = in_flight {
        open.push(Obligation {
            kind: "current".into(),
            case: s,
            detail: serde_json::Value::Null,
        });
    }
    open.extend(refused);
    open.extend(leased);
    open.extend(pending.into_iter().map(|(_, s)| Obligation {
        kind: "pending".into(),
        case: s,
        detail: serde_json::Value::Null,
    }));
    open.extend(numerical.into_iter().map(|s| Obligation {
        kind: "numerical".into(),
        case: s,
        detail: serde_json::Value::Null,
    }));
    open.extend(regions);
    if initial.is_empty() {
        initial.push(CaseSpec {
            fixed: vec![None; n],
            equations: vec![],
        });
    }
    Ok(Obligations {
        schema: OBLIGATIONS_SCHEMA.into(),
        math_key: key,
        identity: None,
        state: if open.is_empty() {
            "complete".into()
        } else {
            "stopped".into()
        },
        stop: Some(format!(
            "reconstructed from {} ({lines} queue lines, {pops} pops verified, {rules} rules)",
            qpath.display()
        )),
        chart: header.chart,
        initial,
        pool_claims: vec![],
        open,
        rules,
        residual_keys: vec![],
        source: run_dir.display().to_string(),
    })
}

// ---------------------------------------------------------------------------------------------------------------
// join

#[derive(Clone, Debug)]
pub struct JoinRun {
    pub dir: PathBuf,
    /// `F.obligations.json` of the run (default `<dir>/F.obligations.json`).
    pub obligations: PathBuf,
    /// Optional compact cover index of the run's table (instead of streaming the table itself).
    pub cover_index: Option<PathBuf>,
}

#[derive(Clone, Debug, Default)]
pub struct JoinOptions {
    pub runs: Vec<JoinRun>,
    pub out: PathBuf,
    pub expect_residuals: Option<usize>,
    /// Certificates of deferred regions: JSON lists of {chart, conjunction, status: "EMPTY", method} or
    /// {chart, conjunction, status: "COVERED", method, cases: [CaseSpec]} (decomposition into owned/covered cases).
    pub region_proofs: Vec<PathBuf>,
    pub union_depth: u32,
    pub write_table: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct JoinReport {
    pub schema: String,
    /// `closed`: every obligation discharged, no dependency cycle, residuals as expected;
    /// `open`: obligations remain (listed); `rejected`: key mismatch or a dependency cycle.
    pub verdict: String,
    pub math_key: String,
    pub runs: Vec<serde_json::Value>,
    pub rules: usize,
    pub discharged: BTreeMap<String, usize>,
    pub edges: Vec<(String, String, usize)>,
    pub cycles: Vec<Vec<String>>,
    pub unresolved: Vec<serde_json::Value>,
    pub residual_keys: Vec<String>,
    pub residuals_ok: Option<bool>,
    pub table: Option<String>,
    pub reasons: Vec<String>,
    pub seconds: f64,
}

/// Join the runs of one split node (see the module documentation). Writes `<out>/JOIN.json` and, with
/// `write_table`, the joined `<out>/F.table.jsonl` (rules of every run, re-indexed, residuals, end record).
pub fn join(opts: &JoinOptions) -> Result<JoinReport, String> {
    let t0 = std::time::Instant::now();
    std::fs::create_dir_all(&opts.out).map_err(|e| format!("{}: {e}", opts.out.display()))?;
    let mut report = JoinReport {
        schema: JOIN_SCHEMA.into(),
        ..Default::default()
    };
    let mut obls: Vec<Obligations> = Vec::new();
    let mut headers: Vec<RunHeader> = Vec::new();
    for r in &opts.runs {
        let (key, h) = run_math_key(&r.dir)?;
        let o = Obligations::read(&r.obligations)?;
        if o.math_key != key {
            report.reasons.push(format!(
                "{}: obligations bound to math key {} but the run's files give {key}",
                r.dir.display(),
                o.math_key
            ));
        }
        if report.math_key.is_empty() {
            report.math_key = key.clone();
        } else if report.math_key != key {
            report.reasons.push(format!(
                "{}: math key {key} differs from {} (another family presentation, chart, integral ORDER or certificate: rules of different orders are never joined)",
                r.dir.display(),
                report.math_key
            ));
        }
        report.runs.push(serde_json::json!({"dir": r.dir, "state": o.state, "open": o.open.len(), "initial": o.initial.len(),
            "rules": o.rules, "order": h.order_name, "math_key": key}));
        headers.push(h);
        obls.push(o);
    }
    if obls.is_empty() {
        return Err("join: no runs".into());
    }
    if !report.reasons.is_empty() {
        report.verdict = "rejected".into();
        report.seconds = t0.elapsed().as_secs_f64();
        write_report(&opts.out, &report)?;
        return Ok(report);
    }
    let fam = compile_family(&headers[0].presented)?;
    let n = fam.denominator_count();
    let mut proofs: Vec<serde_json::Value> = Vec::new();
    for p in &opts.region_proofs {
        let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let v: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", p.display()))?;
        proofs.extend(v.as_array().cloned().unwrap_or_default());
    }
    dispatch!(
        n,
        join_impl,
        &fam,
        &headers[0],
        opts,
        &obls,
        &proofs,
        &mut report
    )?;
    report.seconds = t0.elapsed().as_secs_f64();
    write_report(&opts.out, &report)?;
    Ok(report)
}

fn write_report(out: &Path, r: &JoinReport) -> Result<(), String> {
    std::fs::write(
        out.join("JOIN.json"),
        serde_json::to_string_pretty(r).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(|e| format!("JOIN.json: {e}"))
}

/// Tarjan's strongly connected components of a directed graph on `0..n`.
pub fn sccs(n: usize, edges: &[(usize, usize)]) -> Vec<Vec<usize>> {
    let mut adj = vec![Vec::new(); n];
    for &(a, b) in edges {
        adj[a].push(b);
    }
    let (mut index, mut stack, mut on) = (0usize, Vec::new(), vec![false; n]);
    let (mut idx, mut low) = (vec![usize::MAX; n], vec![0usize; n]);
    let mut out = Vec::new();
    // iterative Tarjan: (node, next child position)
    for s in 0..n {
        if idx[s] != usize::MAX {
            continue;
        }
        let mut call: Vec<(usize, usize)> = vec![(s, 0)];
        idx[s] = index;
        low[s] = index;
        index += 1;
        stack.push(s);
        on[s] = true;
        while let Some(&mut (v, ref mut k)) = call.last_mut() {
            if *k < adj[v].len() {
                let w = adj[v][*k];
                *k += 1;
                if idx[w] == usize::MAX {
                    idx[w] = index;
                    low[w] = index;
                    index += 1;
                    stack.push(w);
                    on[w] = true;
                    call.push((w, 0));
                } else if on[w] {
                    low[v] = low[v].min(idx[w]);
                }
            } else {
                call.pop();
                if let Some(&(u, _)) = call.last() {
                    low[u] = low[u].min(low[v]);
                }
                if low[v] == idx[v] {
                    let mut comp = Vec::new();
                    while let Some(w) = stack.pop() {
                        on[w] = false;
                        comp.push(w);
                        if w == v {
                            break;
                        }
                    }
                    out.push(comp);
                }
            }
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn join_impl<const N: usize>(
    fam: &crate::family::IntegralFamily,
    header: &RunHeader,
    opts: &JoinOptions,
    obls: &[Obligations],
    proofs: &[serde_json::Value],
    report: &mut JoinReport,
) -> Result<(), String> {
    let ctx = context::<N>(fam, &header.presented, Some(&header.chart))?;
    let n = ctx.n;
    let limits = CaseIntersectionLimits::default();
    let build = |s: &CaseSpec| build_case::<N>(s, &ctx.template, &ctx.idx, &ctx.sector, n);
    // initial cases per run
    let mut initials: Vec<Vec<Case<N>>> = Vec::new();
    for (k, o) in obls.iter().enumerate() {
        let mut v = Vec::new();
        for s in &o.initial {
            match build(s)? {
                Some(c) => v.push(c),
                None => report
                    .reasons
                    .push(format!("run {k}: initial case {s:?} is empty")),
            }
        }
        initials.push(v);
    }
    // the root: a run owning the whole chart
    let whole = CaseSpec {
        fixed: vec![None; n],
        equations: vec![],
    };
    let roots: Vec<usize> = (0..obls.len())
        .filter(|&k| obls[k].initial.contains(&whole))
        .collect();
    if roots.is_empty() {
        report
            .reasons
            .push("no run owns the whole chart (initial = all coordinates free)".into());
    }
    // rule entries of every run
    let mut entries: Vec<(Case<N>, Vec<Vec<CoefficientPolynomial>>)> = Vec::new();
    for (k, r) in opts.runs.iter().enumerate() {
        let src = r
            .cover_index
            .clone()
            .unwrap_or_else(|| r.dir.join("F.table.jsonl"));
        let (shapes, _) = read_rule_shapes(&src.display().to_string(), usize::MAX)?;
        if shapes.len() != obls[k].rules {
            report.reasons.push(format!(
                "run {k}: {} rules in {} but {} in its obligations record",
                shapes.len(),
                src.display(),
                obls[k].rules
            ));
        }
        for s in shapes {
            let spec = CaseSpec {
                fixed: s.fixed,
                equations: s.equations,
            };
            match shape_entry::<N>(
                &spec,
                &s.exceptions,
                &ctx.template,
                &ctx.idx,
                &ctx.sector,
                n,
            ) {
                Some(e) => entries.push(e),
                None => report
                    .reasons
                    .push(format!("run {k}: rule {:?} does not parse", s.index)),
            }
        }
    }
    report.rules = entries.len();
    let refs: Vec<(&Case<N>, &[Vec<CoefficientPolynomial>])> =
        entries.iter().map(|(c, b)| (c, b.as_slice())).collect();
    let mut edges: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    let mut discharged: BTreeMap<String, usize> = BTreeMap::new();
    let total: usize = obls.iter().map(|o| o.open.len()).sum();
    let mut seen = 0usize;
    for (k, o) in obls.iter().enumerate() {
        for ob in &o.open {
            seen += 1;
            if seen % 250 == 0 {
                eprintln!(
                    "[join] {seen}/{total} obligations, {} unresolved",
                    report.unresolved.len()
                );
            }
            // (1) emptiness certificate of a deferred region (matched on chart and conjunction)
            if ob.kind == "region" || ob.kind == "branch" {
                let conj = if ob.kind == "region" {
                    &ob.detail["conjunction"]
                } else {
                    &ob.detail["polys"]
                };
                if let Some(p) = proofs.iter().find(|p| {
                    p["status"] == "EMPTY"
                        && serde_json::from_value::<CaseSpec>(p["chart"].clone())
                            .ok()
                            .as_ref()
                            == Some(&ob.case)
                        && p["conjunction"] == *conj
                }) {
                    *discharged
                        .entry(format!(
                            "region_empty:{}",
                            p["method"].as_str().unwrap_or("?")
                        ))
                        .or_default() += 1;
                    continue;
                }
                // (1b) decomposition certificate: {chart, conjunction, status: "COVERED", method, cases: [CaseSpec]}
                // = an exact (external) proof that the integer points of chart & conjunction lie in the union of the
                // listed cases; each listed case must be empty, owned by a COMPLETE run other than this one (an edge
                // into a sink) or covered by one accepted rule's valid domain. Otherwise the certificate is not used.
                if let Some(p) = proofs.iter().find(|p| {
                    p["status"] == "COVERED"
                        && serde_json::from_value::<CaseSpec>(p["chart"].clone())
                            .ok()
                            .as_ref()
                            == Some(&ob.case)
                        && p["conjunction"] == *conj
                }) {
                    let pieces: Vec<CaseSpec> =
                        serde_json::from_value(p["cases"].clone()).unwrap_or_default();
                    let mut ok = !pieces.is_empty();
                    let mut new_edges: Vec<usize> = Vec::new();
                    for s in &pieces {
                        if !ok {
                            break;
                        }
                        let Some(pc) = (if s.fixed.len() == n { build(s)? } else { None }) else {
                            ok = s.fixed.len() == n; // an empty piece needs nothing
                            continue;
                        };
                        let owner = (0..obls.len()).find(|&j| {
                            j != k
                                && obls[j].state == "complete"
                                && initials[j]
                                    .iter()
                                    .any(|ic| ic.contains(&pc).unwrap_or(false))
                        });
                        if let Some(j) = owner {
                            new_edges.push(j);
                        } else if !refs
                            .iter()
                            .any(|(c, b)| covers(c, b, &pc, &ctx.idx, &ctx.sector, limits))
                        {
                            ok = false;
                        }
                    }
                    if ok {
                        for j in new_edges {
                            *edges.entry((k, j)).or_default() += 1;
                        }
                        *discharged
                            .entry(format!(
                                "region_covered:{}",
                                p["method"].as_str().unwrap_or("?")
                            ))
                            .or_default() += 1;
                        continue;
                    }
                }
            }
            let Some(case) = (if ob.case.fixed.len() == n {
                build(&ob.case)?
            } else {
                None
            }) else {
                if ob.case.fixed.len() == n {
                    *discharged.entry("empty_case".into()).or_default() += 1;
                } else {
                    report
                        .unresolved
                        .push(serde_json::json!({"run": k, "obligation": ob, "why": "no case"}));
                }
                continue;
            };
            // candidate owners: ANOTHER run whose initial case contains it (never a self discharge)
            let mut owners: Vec<usize> = (0..obls.len())
                .filter(|&j| {
                    j != k
                        && initials[j]
                            .iter()
                            .any(|ic| ic.contains(&case).unwrap_or(false))
                })
                .collect();
            owners.sort_by_key(|&j| (obls[j].state != "complete", roots.contains(&j), j));
            // (2) a COMPLETE run owns it: a dependency edge into a sink (a complete run has no open obligation, so no
            // cycle can pass through it); cheap containment, tried before any intersection
            if let Some(&j) = owners.first().filter(|&&j| obls[j].state == "complete") {
                *edges.entry((k, j)).or_default() += 1;
                *discharged.entry("run_initial_complete".into()).or_default() += 1;
                continue;
            }
            // a deferred region's chart is where the intersection with an exceptional branch got stuck: no
            // intersection-based proof is attempted on it here (it can hang, rr-consolidate T182 lesson); it needs an
            // emptiness certificate or a run owning its chart
            let is_region = ob.kind == "region" || ob.kind == "branch";
            // (3) one accepted rule covers it (valid domain): no dependency at all
            if !is_region
                && refs
                    .iter()
                    .any(|(c, b)| covers(c, b, &case, &ctx.idx, &ctx.sector, limits))
            {
                *discharged.entry("rule_cover".into()).or_default() += 1;
                continue;
            }
            // (3b) a stopped run owns it: an edge (cycles among stopped runs are rejected below)
            if let Some(&j) = owners.first() {
                *edges.entry((k, j)).or_default() += 1;
                *discharged.entry("run_initial".into()).or_default() += 1;
                continue;
            }
            // (4) the depth-bounded union argument against the joined rules
            let mut budget = 400usize;
            if !is_region
                && union_covered(
                    &case,
                    &refs,
                    &ctx.idx,
                    &ctx.sector,
                    limits,
                    opts.union_depth.max(1),
                    &mut budget,
                )
            {
                *discharged.entry("union_cover".into()).or_default() += 1;
                continue;
            }
            report
                .unresolved
                .push(serde_json::json!({"run": k, "dir": opts.runs[k].dir, "obligation": ob}));
        }
    }
    let edge_list: Vec<(usize, usize)> = edges.keys().copied().collect();
    for comp in sccs(obls.len(), &edge_list) {
        let selfloop = comp.len() == 1 && edges.contains_key(&(comp[0], comp[0]));
        if comp.len() > 1 || selfloop {
            report.cycles.push(
                comp.iter()
                    .map(|&k| opts.runs[k].dir.display().to_string())
                    .collect(),
            );
        }
    }
    report.edges = edges
        .iter()
        .map(|((a, b), c)| {
            (
                opts.runs[*a].dir.display().to_string(),
                opts.runs[*b].dir.display().to_string(),
                *c,
            )
        })
        .collect();
    report.discharged = discharged;
    let mut residuals: BTreeSet<String> = BTreeSet::new();
    for o in obls {
        residuals.extend(o.residual_keys.iter().cloned());
    }
    report.residual_keys = residuals.into_iter().collect();
    report.residuals_ok = opts
        .expect_residuals
        .map(|e| e == report.residual_keys.len());
    if !report.cycles.is_empty() {
        report.reasons.push(format!(
            "{} dependency cycle(s) between runs (unresolved strongly connected components)",
            report.cycles.len()
        ));
    }
    report.verdict = if !report.reasons.is_empty() || !report.cycles.is_empty() {
        "rejected".into()
    } else if !report.unresolved.is_empty() || report.residuals_ok == Some(false) {
        "open".into()
    } else {
        "closed".into()
    };
    if opts.write_table {
        report.table = Some(write_joined_table(opts, &ctx.label, &report.residual_keys)?);
    }
    Ok(())
}

/// Concatenate the runs' rule lines (re-indexed, chart label kept), the residuals and an end record.
fn write_joined_table(
    opts: &JoinOptions,
    label: &str,
    residuals: &[String],
) -> Result<String, String> {
    let path = opts.out.join("F.table.jsonl");
    let mut out = std::io::BufWriter::with_capacity(
        1 << 22,
        std::fs::File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?,
    );
    let mut index = 0usize;
    for (k, r) in opts.runs.iter().enumerate() {
        let f = std::fs::File::open(r.dir.join("F.table.jsonl")).map_err(|e| e.to_string())?;
        let mut rd = std::io::BufReader::with_capacity(1 << 22, f);
        let mut line = String::new();
        loop {
            line.clear();
            if rd.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                break;
            }
            let l = line.trim_end();
            if k == 0
                && (l.starts_with("{\"type\":\"header\"")
                    || l.starts_with("{\"type\":\"zero_sectors\"")
                    || l.starts_with("{\"type\":\"chart\""))
            {
                writeln!(out, "{l}").map_err(|e| e.to_string())?;
                continue;
            }
            if !l.starts_with("{\"type\":\"rule\"") {
                continue;
            }
            // re-index: the first "index": field of a rule line is the rule index
            let pos = l.find("\"index\":").ok_or("rule line without index")? + 8;
            let end = pos
                + l[pos..]
                    .find(|c: char| !c.is_ascii_digit())
                    .ok_or("rule index")?;
            let body = l
                .strip_suffix('}')
                .ok_or("rule line does not end with '}'")?;
            writeln!(
                out,
                "{}{index}{},\"join_source\":{{\"run\":{k},\"index\":{}}}}}",
                &body[..pos],
                &body[end..],
                &l[pos..end]
            )
            .map_err(|e| e.to_string())?;
            index += 1;
        }
    }
    for key in residuals {
        writeln!(
            out,
            "{}",
            serde_json::json!({"type": "residual", "chart": label, "key": key})
        )
        .map_err(|e| e.to_string())?;
    }
    writeln!(out, "{}", serde_json::json!({"type": "end", "charts": 1, "rules": index, "residuals": residuals.len(), "chart_errors": 0,
        "harness": "rustred node join (rr-registry)", "state": "joined", "runs": opts.runs.len()})).map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}
