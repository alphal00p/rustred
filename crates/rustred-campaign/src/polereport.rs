//! The node pole report (lane tadicjob; interfaces/tadic-job.md 3, 5): `rr_campaign tadic-check` (the certificate checks
//! C1-C6 of one driver result, no trust in the driver) and `rr_campaign pole-report` (a resumable campaign over a node
//! list: per cell (D, r) up to top(D) the rank of rho from integer-weighted random combinations of the cell's keys, two
//! seeds, the surviving directions and the verdict "count zero on all cells up to top(D): yes / no / not decided").
//!
//! Every input is bound by its SHA-256 in the spec and checked at start. The cell list is INPUT DATA (TRICKS 45): rows of
//! a cell-tops table (helper VH 2's CELL_TOPS.tsv: main line top(D) = `top_r_at_D_if_n4hint`, or `top_r_at_D` where the
//! hint is empty, never clipped by the box; margin = the cells between the hint and `top_r_at_D`; cells above
//! `box_r_max` are flagged "outside BOXDEF v3, reached by N units"). Keys of a cell: the compositions of D dots over the
//! node's line slots (at most `max_dots_per_line` each) times the ISP powers summing to -r (generic enumerator; its
//! count is checked against the table's `keys_in_cell`). Jobs run through the tadic-replay adapter and the process layer
//! (RealLauncher; run the campaign inside a private pid namespace); the rank step is the Symbolica-backed `rr_pole_rank`
//! (pinned by sha256, checked at every launch, run on the verified results as a child of the same process layer:
//! [`run_program_child`]; caps `rank_memory_cap_bytes` / `rank_wall_seconds`). Resume: the result store (a stored job is never
//! rerun) and the per-cell records in the campaign directory.
//!
//! Generic (L8): nodes, slots, cells, packages, masters, depths and seeds are data.

use crate::adapters::job::{JobAdapter, Program, RunOptions, drive, run_attempt, sha256_hex};
use crate::adapters::tadicjob::{
    DataFile, ResultStore, TadicAdapter, TadicInput, TadicJobRequest, TadicLevel, TadicLimits,
    TadicRequest, WeightVectors,
};
use crate::launch::{ChildLauncher, ExitState, FinalReport, LaunchSpec, RealLauncher};
use crate::resources::{Sampler, SamplerConfig};
use rustred_campaign_contract::tadicjob::{self, Pins, certificate};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Schema of the report spec.
pub const SPEC_SCHEMA: &str = "rustred.campaign.pole-report-spec.v1";
/// Schema of the report.
pub const REPORT_SCHEMA: &str = "rustred.campaign.pole-report.v1";

fn arg(args: &[String], k: &str) -> Option<String> {
    args.iter()
        .position(|a| a == k)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn read_json(p: &str) -> Result<(Value, Vec<u8>), String> {
    let b = std::fs::read(p).map_err(|e| format!("{p}: {e}"))?;
    let v = serde_json::from_slice(&b).map_err(|e| format!("{p}: {e}"))?;
    Ok((v, b))
}

// ---------------------------------------------------------------------------------------------------------------
// tadic-check
// ---------------------------------------------------------------------------------------------------------------

/// The checks C1-C5 (+ C6 with a rerun) of one result as a JSON report.
pub fn check_report(
    job: &Value,
    job_bytes: &[u8],
    result: &Value,
    binary_sha256: &str,
    driver_sha256: &str,
    rerun: Option<&Value>,
) -> Value {
    let pins = Pins {
        job_sha256: sha256_hex(job_bytes),
        binary_sha256: binary_sha256.into(),
        driver_sha256: driver_sha256.into(),
    };
    match tadicjob::check_result(job, result, &pins) {
        Err(why) => json!({"bound": false, "why": why, "certified": false}),
        Ok(c) => {
            let rep = certificate::check(&c);
            let rerun_diffs = rerun.map(|r| certificate::compare_rerun(result, r));
            let certified =
                rep.job_certified() && rerun_diffs.as_ref().is_none_or(|d| d.is_empty());
            json!({"bound": true, "status": c.status, "error_code": c.error_code, "level": c.level,
                "seeds": c.seeds, "primes": c.primes.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
                "requests": c.requests.iter().map(|q| json!({"label": q.label, "outcome": q.outcome.word(),
                    "certified": rep.certified.get(&q.label).copied().unwrap_or(false)})).collect::<Vec<_>>(),
                "findings": rep.findings.iter().map(|f| f.to_json()).collect::<Vec<_>>(),
                "rerun_diffs": rerun_diffs, "certified": certified})
        }
    }
}

/// C4 across two results of the same requests run with DIFFERENT seeds (two jobs, one seed each): disjoint primes and
/// equal valuations per (label, basis, tag); a term absent from one result counts as zero there. Returns the findings.
pub fn compare_seeds(a: &Value, b: &Value) -> Vec<String> {
    let mut out = vec![];
    let primes = |r: &Value| -> std::collections::BTreeSet<String> {
        r["primes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| p.as_str().map(str::to_string))
            .collect()
    };
    let common: Vec<String> = primes(a).intersection(&primes(b)).cloned().collect();
    if !common.is_empty() {
        out.push(format!("primes shared by the two results: {common:?}"));
    }
    let vals = |r: &Value| -> BTreeMap<(String, String, String), Option<i64>> {
        let mut m = BTreeMap::new();
        for q in r["requests"].as_array().into_iter().flatten() {
            for t in q["terms"].as_array().into_iter().flatten() {
                let v = t["per_prime"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|p| p["valuation"].as_i64())
                    .min();
                m.insert(
                    (
                        q["label"].as_str().unwrap_or("").into(),
                        t["basis"].as_str().unwrap_or("").into(),
                        t["tag"].as_str().unwrap_or("").into(),
                    ),
                    v,
                );
            }
        }
        m
    };
    let (va, vb) = (vals(a), vals(b));
    for k in va
        .keys()
        .chain(vb.keys())
        .collect::<std::collections::BTreeSet<_>>()
    {
        let (x, y) = (va.get(k).copied().flatten(), vb.get(k).copied().flatten());
        if x != y {
            out.push(format!("{} {} {}: valuations {x:?} / {y:?}", k.0, k.1, k.2));
        }
    }
    out
}

/// `rr_campaign tadic-check --job J --result R --binary-sha256 X --driver-sha256 Y [--rerun R2] [--other-seed R3]`.
pub fn check_cmd(args: &[String]) -> Result<String, String> {
    let (Some(j), Some(r), Some(b), Some(d)) = (
        arg(args, "--job"),
        arg(args, "--result"),
        arg(args, "--binary-sha256"),
        arg(args, "--driver-sha256"),
    ) else {
        return Err("usage: tadic-check --job J --result R --binary-sha256 X --driver-sha256 Y [--rerun R2]".into());
    };
    let (job, jb) = read_json(&j)?;
    let (res, _) = read_json(&r)?;
    let rerun = match arg(args, "--rerun") {
        Some(p) => Some(read_json(&p)?.0),
        None => None,
    };
    let mut rep = check_report(&job, &jb, &res, &b, &d, rerun.as_ref());
    if let Some(p) = arg(args, "--other-seed") {
        let other = read_json(&p)?.0;
        let f = compare_seeds(&res, &other);
        rep["other_seed"] = json!({"path": p, "findings": f, "agree": f.is_empty()});
    }
    Ok(serde_json::to_string_pretty(&rep).expect("json"))
}

// ---------------------------------------------------------------------------------------------------------------
// cells
// ---------------------------------------------------------------------------------------------------------------

/// One row of a cell-tops table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CellRow {
    pub node: String,
    pub e: i64,
    pub d: i64,
    pub r: i64,
    /// Main-line top at this D (the hint column, or the upper bound where the hint is empty).
    pub top: i64,
    /// The upper bound `top_r_at_D` (n4 = 0).
    pub top_upper: i64,
    pub box_r_max: i64,
    pub keys_in_cell: u64,
    pub units_reaching: String,
    /// The row's position against the reference line of the offset (`vs_old_line`: below / on / above; empty when the
    /// table has no such column).
    pub vs_line: String,
}

/// Read a cell-tops TSV (columns by header name).
pub fn read_cell_tops(text: &str) -> Result<Vec<CellRow>, String> {
    let mut lines = text.lines();
    let header: Vec<&str> = lines
        .next()
        .ok_or("empty cell table")?
        .split('\t')
        .collect();
    let col = |n: &str| {
        header
            .iter()
            .position(|h| *h == n)
            .ok_or(format!("cell table: no column {n}"))
    };
    let (cn, ce, cd, cr, ct, cb, ck, cu) = (
        col("node")?,
        col("E")?,
        col("D")?,
        col("r")?,
        col("top_r_at_D")?,
        col("box_r_max")?,
        col("keys_in_cell")?,
        col("qcd_units_reaching")?,
    );
    // the main-line top: the n4 hint when numeric (coordinator ruling 2026-11-17), else the exact top (`top_r_exact`,
    // the table regenerated with exact n4, TRICKS 49: the hint is then 'superseded'), else `top_r_at_D`
    let ch = col("top_r_at_D_if_n4hint").ok();
    let cx = col("top_r_exact").ok();
    let cl = col("vs_old_line").ok();
    let mut out = vec![];
    for (n, l) in lines.enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = l.split('\t').collect();
        let get = |i: usize| f.get(i).copied().unwrap_or("");
        let int = |i: usize| -> Result<i64, String> {
            get(i)
                .trim()
                .parse::<i64>()
                .map_err(|e| format!("cell table line {}: column {i}: {e}", n + 2))
        };
        let opt = |c: Option<usize>| c.and_then(|i| get(i).trim().parse::<i64>().ok());
        let upper = int(ct)?;
        let top = opt(ch).or(opt(cx)).unwrap_or(upper);
        out.push(CellRow {
            node: get(cn).to_string(),
            e: int(ce)?,
            d: int(cd)?,
            r: int(cr)?,
            top,
            top_upper: upper,
            box_r_max: int(cb)?,
            keys_in_cell: get(ck)
                .trim()
                .parse()
                .map_err(|e| format!("keys_in_cell: {e}"))?,
            units_reaching: get(cu).to_string(),
            vs_line: cl.map(|i| get(i).trim().to_string()).unwrap_or_default(),
        });
    }
    Ok(out)
}

/// The OFFSET of a cell (TRICKS 64: need follows o, not D or r): o = r - (E + D - s), the numerator rank above the
/// reference line r = E + D - s. The shift s is DATA: the spec's `offset_shift`, else read from the cell table's rows
/// labelled `on` the line (E + D - r on every such row of the node; rows that disagree are an error, never averaged).
/// Ok(None) = no reference line for this node (no `on` row and no spec value): the cells are shown without an offset.
pub fn offset_shift(
    rows: &[CellRow],
    node: &str,
    spec_shift: Option<i64>,
) -> Result<Option<(i64, String)>, String> {
    if let Some(s) = spec_shift {
        return Ok(Some((s, "spec offset_shift".into())));
    }
    let on: std::collections::BTreeSet<i64> = rows
        .iter()
        .filter(|c| c.node == node && c.vs_line == "on")
        .map(|c| c.e + c.d - c.r)
        .collect();
    match on.len() {
        0 => Ok(None),
        1 => Ok(on
            .first()
            .map(|&s| (s, "cell table rows 'on' the line (E + D - r)".to_string()))),
        _ => Err(format!(
            "cell table: the rows of {node} 'on' the line disagree on E + D - r: {on:?}"
        )),
    }
}

/// One TRIPWIRE master of a node (TRICKS 64; data of the spec, never of the code): its need on a cell above its known
/// depth q by more than `allowance` orders is reported as `fired` beside the rank (the node's own argument says what a
/// fired tripwire proves; the verdict stays the rank's).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tripwire {
    pub master: String,
    #[serde(default)]
    pub allowance: i64,
}

/// The tripwire records of a cell from a rank record (`cell_needs`, `cell_depths`; `cell_beyond` for q when an older
/// rank binary wrote no depths). need = null: the master does not occur on the cell (never fired).
pub fn tripwire_records(wires: &[Tripwire], rank: &Value) -> Vec<Value> {
    wires
        .iter()
        .map(|w| {
            let need = rank["cell_needs"][&w.master].as_i64();
            let q = rank["cell_depths"][&w.master].as_i64().or_else(|| {
                rank["cell_beyond"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|b| b["master"] == w.master.as_str())
                    .and_then(|b| b["q"].as_i64())
            });
            let fired = match (need, q) {
                (Some(n), Some(q)) => json!(n > q.saturating_add(w.allowance)),
                (None, _) => json!(false),
                (Some(_), None) => Value::Null,
            };
            json!({"master": w.master, "need": need, "q": q, "allowance": w.allowance, "fired": fired})
        })
        .collect()
}

/// The keys of cell (D, r): line slots at power 1 + dots (sum D, each <= `max_dots`), ISP slots at power <= 0 (sum -r).
pub fn cell_keys(
    arity: usize,
    lines: &[usize],
    isps: &[usize],
    d: i64,
    r: i64,
    max_dots: i64,
) -> Vec<Vec<i64>> {
    fn comps(total: i64, n: usize, cap: i64, out: &mut Vec<Vec<i64>>, cur: &mut Vec<i64>) {
        if n == 0 {
            if total == 0 {
                out.push(cur.clone());
            }
            return;
        }
        for x in (0..=total.min(cap)).rev() {
            cur.push(x);
            comps(total - x, n - 1, cap, out, cur);
            cur.pop();
        }
    }
    let (mut dots, mut nums) = (vec![], vec![]);
    comps(d, lines.len(), max_dots, &mut dots, &mut vec![]);
    comps(r, isps.len(), r.max(0), &mut nums, &mut vec![]);
    let mut keys = vec![];
    for dv in &dots {
        for nv in &nums {
            let mut k = vec![0i64; arity];
            for (i, &s) in lines.iter().enumerate() {
                k[s] = 1 + dv[i];
            }
            for (i, &s) in isps.iter().enumerate() {
                k[s] = -nv[i];
            }
            keys.push(k);
        }
    }
    keys
}

/// The integer weight of key `k` in combination `c` of a cell: uniform in [1, 2^62) from SHA-256 of the campaign seed,
/// node, cell, c and k (reproducible, independent of the order of runs).
pub fn weight(seed: u64, node: &str, d: i64, r: i64, c: usize, k: usize) -> i64 {
    let h = sha256_hex(format!("pole-report-weight:{seed}:{node}:{d}:{r}:{c}:{k}").as_bytes());
    let x = u64::from_str_radix(&h[..16], 16).unwrap_or(1);
    ((x % ((1u64 << 62) - 1)) + 1) as i64
}

// ---------------------------------------------------------------------------------------------------------------
// spec
// ---------------------------------------------------------------------------------------------------------------

/// A file bound by SHA-256.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bound {
    pub path: PathBuf,
    pub sha256: String,
}

impl Bound {
    fn check(&self, what: &str) -> Result<Vec<u8>, String> {
        let b = std::fs::read(&self.path)
            .map_err(|e| format!("{what} {}: {e}", self.path.display()))?;
        let h = sha256_hex(&b);
        if h != self.sha256 {
            return Err(format!(
                "{what} {}: sha256 {h} is not the spec's {}",
                self.path.display(),
                self.sha256
            ));
        }
        Ok(b)
    }
}

/// A stored driver attempt (driver job + driver result, both by SHA-256) offered for the rank-only route of one cell.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReuseEntry {
    pub cell: (i64, i64),
    pub job: Bound,
    pub result: Bound,
}

/// Fields of a driver job that do not change the replay's answers (helper VH 2's driver): `job_id` (derived from a
/// content id that also hashes the rank-step roles), `limits.wall_seconds` (a stop, never an answer) and
/// `dependence_table` (the driver binds it by SHA-256 and does NOT apply it). Every other field must be equal.
pub const RANK_ONLY_JOB_FIELDS: [&str; 3] = ["job_id", "limits.wall_seconds", "dependence_table"];

/// Ok when a stored driver job equals the job this cell would issue up to [`RANK_ONLY_JOB_FIELDS`]; else the differing
/// top-level fields.
pub fn reuse_compatible(stored: &Value, ours: &Value) -> Result<(), String> {
    let strip = |v: &Value| {
        let mut v = v.clone();
        if let Some(o) = v.as_object_mut() {
            o.remove("job_id");
            o.remove("dependence_table");
            if let Some(l) = o.get_mut("limits").and_then(|l| l.as_object_mut()) {
                l.remove("wall_seconds");
            }
        }
        v
    };
    let (a, b) = (strip(stored), strip(ours));
    if a == b {
        return Ok(());
    }
    let keys: std::collections::BTreeSet<&String> = a
        .as_object()
        .into_iter()
        .flatten()
        .chain(b.as_object().into_iter().flatten())
        .map(|(k, _)| k)
        .collect();
    let differ: Vec<&str> = keys
        .into_iter()
        .filter(|k| a.get(k.as_str()) != b.get(k.as_str()))
        .map(|k| k.as_str())
        .collect();
    Err(format!(
        "the stored driver job differs from this cell's job in: {}",
        differ.join(", ")
    ))
}

/// The rank-only route's gate: a stored attempt answers THIS cell's job `jr` iff (1) its job equals the job the tool
/// issues for `jr` up to [`RANK_ONLY_JOB_FIELDS`], (2) its result binds to ITS job bytes and to the adapter's pinned
/// binary and driver (`tadicjob::check_result`), (3) the certificate checks find nothing and (4) every request is
/// decided. The rank-step roles (depths, dependences, terminal map) are then the spec's, never the stored job's.
pub fn verify_reuse(
    adapter: &TadicAdapter,
    jr: &TadicJobRequest,
    job_bytes: &[u8],
    result_bytes: &[u8],
) -> Result<tadicjob::CheckedResult, String> {
    let stored: Value =
        serde_json::from_slice(job_bytes).map_err(|e| format!("stored job: {e}"))?;
    let ours = adapter.driver_job(jr, stored["job_id"].as_str().unwrap_or(""));
    reuse_compatible(&stored, &ours)?;
    let result: Value =
        serde_json::from_slice(result_bytes).map_err(|e| format!("stored result: {e}"))?;
    let checked = tadicjob::check_result(&stored, &result, &adapter.pins(job_bytes, &jr.level))?;
    let cert = certificate::check(&checked);
    if let Some(f) = cert.findings.first() {
        return Err(format!("NOT_CERTIFIED: {}", f.to_json()));
    }
    if let Some(q) = checked.requests.iter().find(|q| !q.outcome.is_decided()) {
        return Err(format!("{}: {}", q.label, q.outcome.word()));
    }
    Ok(checked)
}

/// Spec rule (2026-11-18: a dependence table republished during a 4.7 h attempt broke the wrapper's post-run binding):
/// every DATA input (cell table, library, depths, dependences, catalogue, terminal map) is copied at launch into
/// `<dir>/inputs/<sha256>/<file name>` (read-only, re-hashed after the copy; an existing snapshot is re-hashed and must
/// match) and the spec is rewritten to read only the snapshots: driver jobs, the wrapper's re-hash and every rank step
/// see bytes that cannot change. The spec's own paths stay as provenance in the report. Returns that provenance.
pub fn snapshot_inputs(spec: &mut PoleReportSpec, dir: &Path) -> Result<Value, String> {
    let mut prov = serde_json::Map::new();
    let mut one = |role: &str, b: &mut Bound| -> Result<(), String> {
        let bytes = b.check(role)?;
        let name = b
            .path
            .file_name()
            .ok_or(format!("{role}: no file name"))?
            .to_owned();
        let d = dir.join("inputs").join(&b.sha256);
        std::fs::create_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
        let snap = d.join(&name);
        match std::fs::read(&snap) {
            Ok(have) => {
                if sha256_hex(&have) != b.sha256 {
                    return Err(format!(
                        "{role}: snapshot {} is not the spec's sha256",
                        snap.display()
                    ));
                }
            }
            Err(_) => {
                let tmp = d.join(format!(".{}.tmp", name.to_string_lossy()));
                std::fs::write(&tmp, &bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
                let mut perm = std::fs::File::open(&tmp)
                    .and_then(|f| f.metadata())
                    .map_err(|e| e.to_string())?
                    .permissions();
                perm.set_readonly(true);
                std::fs::set_permissions(&tmp, perm).map_err(|e| e.to_string())?;
                std::fs::rename(&tmp, &snap).map_err(|e| e.to_string())?;
                let again = std::fs::read(&snap).map_err(|e| e.to_string())?;
                if sha256_hex(&again) != b.sha256 {
                    return Err(format!("{role}: snapshot copy differs"));
                }
            }
        }
        prov.insert(
            role.into(),
            json!({"source": b.path, "snapshot": snap, "sha256": b.sha256}),
        );
        b.path = snap;
        Ok(())
    };
    one("cells", &mut spec.cells)?;
    one("library", &mut spec.library)?;
    one("depths", &mut spec.depths)?;
    one("catalogue", &mut spec.catalogue)?;
    if let Some(d) = spec.dependences.as_mut() {
        one("dependences", d)?;
    }
    if let Some(m) = spec.terminal_map.as_mut() {
        one("terminal_map", m)?;
    }
    Ok(Value::Object(prov))
}

/// One node of the report: its presented top and slots.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeSpec {
    pub node: String,
    /// The package's top name or `pres:<id>` of the presentation the keys are written in.
    pub top: String,
    pub arity: usize,
    pub lines: Vec<usize>,
    pub isps: Vec<usize>,
    #[serde(default = "four")]
    pub max_dots_per_line: i64,
    /// Tripwire masters of the node (data; TRICKS 64), reported per cell.
    #[serde(default)]
    pub tripwires: Vec<Tripwire>,
}

fn four() -> i64 {
    4
}

fn eight() -> u32 {
    8
}

/// The report spec (interfaces/tadic-job.md 5).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoleReportSpec {
    pub schema: String,
    pub cells: Bound,
    pub nodes: Vec<NodeSpec>,
    /// Which main-line cells to run (None = all); margin cells only with `include_margin`.
    #[serde(default)]
    pub d_max: Option<i64>,
    /// An explicit list of cells [D, r] to run (with d_max / r_max still applied); absent = every selected cell.
    #[serde(default)]
    pub cell_list: Option<Vec<(i64, i64)>>,
    #[serde(default)]
    pub r_max: Option<i64>,
    #[serde(default)]
    pub include_margin: bool,
    /// The shift s of the offset o = r - (E + D - s) (absent = read from the cell table's rows 'on' the line).
    #[serde(default)]
    pub offset_shift: Option<i64>,
    /// RANK-ONLY reuse of stored driver attempts (one entry per cell): when the inputs of the RANK step changed after a
    /// driver attempt (a republished dependence table broke the wrapper's post-run binding), the attempt is reused only
    /// through [`verify_reuse`] and the rank step binds the spec's (snapshotted) inputs; the cell says so.
    #[serde(default)]
    pub reuse: Vec<ReuseEntry>,
    pub package: PathBuf,
    pub relations: Option<PathBuf>,
    pub library: Bound,
    pub python: PathBuf,
    pub driver: Bound,
    pub package_binary: Bound,
    pub node_binary: Bound,
    pub rank_bin: Bound,
    pub depths: Bound,
    pub dependences: Option<Bound>,
    pub catalogue: Bound,
    /// Helper VH 2's terminal map (`helper-vh2.terminal-map.v1`): when given, the rank step reads it instead of
    /// library.json + catalogue; `rows_route` = "v3" (default) or "v2f".
    #[serde(default)]
    pub terminal_map: Option<Bound>,
    #[serde(default)]
    pub rows_route: Option<String>,
    pub seeds: Vec<u64>,
    pub primes: u32,
    /// Primes a seed may consume (usable or not); the driver's default when absent.
    #[serde(default = "eight")]
    pub max_primes: u32,
    pub order: i64,
    pub combination_seed: u64,
    pub m_initial: usize,
    pub m_step: usize,
    pub m_max: usize,
    pub wall_seconds: u64,
    /// Requests per driver job (one engine process per job and seed): the engine keeps state across the requests of
    /// one process (T201 D0r2: 1 request 38.9 GB, the second request of the same process above 48 GB), so large
    /// unions run one request per job. Absent = all new combinations of a round in one job.
    #[serde(default)]
    pub max_requests_per_job: Option<usize>,
    /// Job v2 (frozen 083d2f6c): a round's combinations as weight vectors of ONE request (one recording for all of
    /// them and for both seeds); `release_every` = the engine's retained state released every n requests.
    #[serde(default)]
    pub vectors: bool,
    /// Drop the engine's in-memory step cache (it only costs time and memory on a large single-request trace: T201 D0r2
    /// recorded in 618 s without it, 1,245-1,984 s with it).
    #[serde(default)]
    pub no_step_cache: bool,
    #[serde(default)]
    pub release_every: Option<u32>,
    pub max_length: u64,
    pub threads: u32,
    pub memory_cap_bytes: u64,
    /// Memory cap of one rank step (`rr_pole_rank`, a child of the process layer); absent = `memory_cap_bytes`.
    #[serde(default)]
    pub rank_memory_cap_bytes: Option<u64>,
    /// Wall cap of one rank step in seconds; absent = `wall_seconds`.
    #[serde(default)]
    pub rank_wall_seconds: Option<u64>,
    #[serde(default)]
    pub licence_env: Option<String>,
    /// Test stand: the fake child runs the jobs (program = rr_fake_child).
    #[serde(default)]
    pub fake_program: Option<PathBuf>,
    /// `union` (one weighted request over ALL keys of a cell) or `blocks` (random combinations inside sub-blocks of
    /// at most `block_keys` keys; rank of the cell = rank of the stacked rows of its blocks).
    #[serde(default)]
    pub route: Route,
}

/// The cell route and, for `blocks`, how blocks are sized.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Route {
    #[default]
    Union,
    /// Blocks of at most `max_keys`, or sized from a memory budget and a memory law (data, measured):
    /// keys per block = floor((budget_gb - base_gb) / per_key_gb[r]) (per numerator rank r; at least 1).
    Blocks {
        #[serde(default)]
        max_keys: Option<usize>,
        #[serde(default)]
        budget_gb: Option<f64>,
        #[serde(default)]
        base_gb: Option<f64>,
        #[serde(default)]
        per_key_gb: BTreeMap<String, f64>,
    },
}

impl Route {
    /// Keys per block for numerator rank `r` (None = the union route).
    pub fn block_keys(&self, r: i64) -> Result<Option<usize>, String> {
        match self {
            Route::Union => Ok(None),
            Route::Blocks {
                max_keys: Some(k), ..
            } => Ok(Some((*k).max(1))),
            Route::Blocks {
                budget_gb: Some(b),
                base_gb: Some(a),
                per_key_gb,
                ..
            } => {
                let s = per_key_gb
                    .get(&r.to_string())
                    .ok_or(format!("route blocks: no per_key_gb for rank {r}"))?;
                if *s <= 0.0 {
                    return Err("route blocks: per_key_gb must be positive".into());
                }
                Ok(Some((((b - a) / s).floor() as i64).max(1) as usize))
            }
            Route::Blocks { .. } => {
                Err("route blocks: max_keys, or budget_gb + base_gb + per_key_gb".into())
            }
        }
    }
}

/// The outcome of one cell.
#[derive(Clone, Debug, Serialize)]
pub struct CellOutcome {
    pub node: String,
    pub d: i64,
    pub r: i64,
    /// o = r - (E + D - s) (None: no reference line for the node).
    pub offset: Option<i64>,
    pub keys: usize,
    pub line: &'static str,
    pub outside_box: bool,
    pub units_reaching: String,
    pub combinations: usize,
    pub verdict: String,
    pub rank: Value,
    pub pivots: Value,
    pub rank_kind: String,
    /// The NAIVE rank (no dependence table: every master order beyond its known depth its own symbol) beside the true one.
    pub naive_rank: Value,
    pub cpu_seconds: f64,
    pub peak_gb: f64,
    pub max_plan_need: Value,
    pub max_series_length: Value,
    pub jobs: Vec<String>,
    /// `union` or `blocks`, with the block structure (keys per block, label prefix, weight stream, combinations).
    pub route: String,
    /// The package the cell was computed on (library.json sha256): cells of different packages never enter one verdict.
    pub package: String,
    pub blocks: Vec<Value>,
    /// Per request of the cell: keys in the union, combinations, trace ops, plan max need, max series length.
    pub requests: Vec<Value>,
    /// The node's tripwire masters on this cell: need, q, allowance, fired.
    pub tripwires: Vec<Value>,
    /// Driver attempts reused by the rank-only route (job / result SHA-256, the copy in the campaign directory).
    pub reused: Vec<Value>,
}

fn rank_of(
    ctx: &CellCtx<'_>,
    dir: &Path,
    results: &[PathBuf],
    tag: &str,
    prefix: &str,
) -> Result<Value, String> {
    rank_of_with(ctx, dir, results, tag, prefix, true)
}

/// The rank step; `with_dependences = false` = the NAIVE upper bound (no dependence table). `rr_pole_rank` runs as a
/// child of the process layer ([`run_program_child`]), never spawned directly (L10, tripwire procfix3_proc m5).
fn rank_of_with(
    ctx: &CellCtx<'_>,
    dir: &Path,
    results: &[PathBuf],
    tag: &str,
    prefix: &str,
    with_dependences: bool,
) -> Result<Value, String> {
    let spec = ctx.spec;
    let out = dir.join(format!("rank.{tag}.json"));
    let mut a: Vec<OsString> = vec![];
    for r in results {
        a.push("--result".into());
        a.push(r.into());
    }
    a.push("--depths".into());
    a.push((&spec.depths.path).into());
    match &spec.terminal_map {
        Some(m) => {
            a.push("--terminal-map".into());
            a.push((&m.path).into());
            if let Some(r) = &spec.rows_route {
                a.push("--route".into());
                a.push(r.into());
            }
        }
        None => {
            a.push("--library".into());
            a.push((&spec.library.path).into());
            a.push("--catalogue".into());
            a.push((&spec.catalogue.path).into());
        }
    }
    for x in ["--rank-prefix", prefix, "--out"] {
        a.push(x.into());
    }
    a.push((&out).into());
    if with_dependences && let Some(d) = &spec.dependences {
        a.push("--dependences".into());
        a.push((&d.path).into());
    }
    // the binary that runs is the pinned one (checked again at every launch, not only at the start of the campaign)
    spec.rank_bin.check("rank binary")?;
    let opts = RunOptions {
        cpus: vec![],
        memory_cap_bytes: Some(spec.rank_memory_cap_bytes.unwrap_or(spec.memory_cap_bytes)),
        licence_env: spec.licence_env.clone(),
        wall_cap: Some(Duration::from_secs(
            spec.rank_wall_seconds.unwrap_or(spec.wall_seconds),
        )),
        grace: Some(Duration::from_secs(10)),
    };
    let run = run_program_child(
        ctx.sampler,
        &spec.rank_bin.path,
        &a,
        &dir.join("rank-runs"),
        &format!("pole-rank-{tag}"),
        &opts,
    )?;
    if !run.success() {
        return Err(format!("rr_pole_rank {}: {}", run.how(), run.stderr_tail()));
    }
    read_json(&out.display().to_string()).map(|x| x.0)
}

/// One finished child of [`run_program_child`]: its launcher report and the run directory (stdout.log, stderr.log).
#[derive(Debug, Clone)]
pub struct ProgramRun {
    pub report: FinalReport,
    pub run_dir: PathBuf,
}

impl ProgramRun {
    /// Exit 0, no kill by the supervisor, and the session proven empty.
    pub fn success(&self) -> bool {
        self.report.exit == Some(ExitState::Exited(0))
            && self.report.kill_reason.is_none()
            && self.report.verification.is_verified()
    }

    /// How the child ended, in words (exit code or signal, the supervisor's kill reason, an unverified session).
    pub fn how(&self) -> String {
        let mut w = match self.report.exit {
            Some(ExitState::Exited(c)) => format!("exit {c}"),
            Some(ExitState::Signaled { signal, .. }) => format!("signal {signal}"),
            Some(ExitState::Unknown) | None => "exit unknown".to_string(),
        };
        if let Some(k) = &self.report.kill_reason {
            w.push_str(&format!(", killed by the supervisor ({})", k.outcome()));
        }
        if !self.report.verification.is_verified() {
            w.push_str(", session not verified empty");
        }
        w
    }

    /// The last 2,000 bytes of the child's stderr (empty when unreadable).
    pub fn stderr_tail(&self) -> String {
        let b = std::fs::read(self.run_dir.join("stderr.log")).unwrap_or_default();
        let from = b.len().saturating_sub(2000);
        String::from_utf8_lossy(&b[from..]).trim().to_string()
    }
}

/// Run one program to its end as a child of the process layer (L10): the [`RealLauncher`] starts it in its own session
/// under the campaign marker, with the minimal contract environment (TMPDIR = `<run dir>/tmp`), the licence variable
/// passed BY NAME only, the memory cap and the wall cap of `opts`; stdout / stderr go to the run directory
/// `<runs>/<label>.<n>` (n = the first free attempt number; nothing is overwritten). A launch error (nothing started)
/// is an Err; a started child returns its report whatever its exit (the caller decides).
pub fn run_program_child(
    sampler: &Arc<Sampler>,
    program: &Path,
    args: &[OsString],
    runs: &Path,
    label: &str,
    opts: &RunOptions,
) -> Result<ProgramRun, String> {
    std::fs::create_dir_all(runs).map_err(|e| format!("{}: {e}", runs.display()))?;
    let mut n = 1u32;
    let run_dir = loop {
        let d = runs.join(format!("{label}.{n}"));
        match std::fs::create_dir(&d) {
            Ok(()) => break d,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => n += 1,
            Err(e) => return Err(format!("{}: {e}", d.display())),
        }
    };
    let tmp = run_dir.join("tmp");
    std::fs::create_dir(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let mut ls = LaunchSpec::new(format!("{label}.{n}"), program, &run_dir).with_contract_env(&tmp);
    ls.args = args.to_vec();
    ls.cpus = opts.cpus.clone();
    ls.memory_cap_bytes = opts.memory_cap_bytes;
    if let Some(name) = &opts.licence_env {
        ls.env_passthrough.push(name.clone());
    }
    let mut l = RealLauncher::new(sampler.clone());
    let mut h = l
        .launch(&ls)
        .map_err(|e| format!("launch of {}: {e}", program.display()))?;
    let report = drive(&mut h, opts);
    Ok(ProgramRun { report, run_dir })
}

/// "boxed node N key I(..)" from an engine refusal text ("... failed for boxed node N: ... could not solve the key I(..)"),
/// or None.
pub fn stop_key(message: &str) -> Option<String> {
    let node = message
        .split("boxed node ")
        .nth(1)
        .and_then(|s| s.split(|c: char| c == ':' || c.is_whitespace()).next())
        .map(str::to_string);
    let key = message.split("the key ").nth(1).and_then(|s| {
        let end = s.find(')')?;
        s.starts_with("I(").then(|| s[..=end].to_string())
    });
    match (node, key) {
        (Some(n), Some(k)) => Some(format!("boxed node {n} key {k}")),
        (None, Some(k)) => Some(format!("key {k}")),
        _ => None,
    }
}

/// What one cell's run needs from the campaign.
struct CellCtx<'a> {
    spec: &'a PoleReportSpec,
    adapter: &'a TadicAdapter,
    store: &'a ResultStore,
    sampler: &'a Arc<Sampler>,
    depths: &'a DataFile,
    deps: &'a Option<DataFile>,
    dir: &'a Path,
    /// The node's offset shift s (o = r - (E + D - s)), None = no reference line.
    shift: Option<i64>,
}

/// One group of keys combined at random: the whole cell (union route) or one block.
struct Group {
    prefix: String,
    weight_tag: String,
    keys: Vec<Vec<i64>>,
    m_done: usize,
    m: usize,
    results: Vec<PathBuf>,
    stable: bool,
    rank: Value,
}

/// Run (or resume) one job of `g`'s new combinations; Err(reason) = not decided.
fn group_round(
    ctx: &CellCtx<'_>,
    ns: &NodeSpec,
    c: &CellRow,
    g: &mut Group,
    out: &mut CellOutcome,
) -> Result<Result<(), String>, String> {
    let per = ctx.spec.max_requests_per_job.unwrap_or(usize::MAX).max(1);
    let (start, end) = (g.m_done, g.m);
    let mut from = start;
    while from < end {
        let to = end.min(from.saturating_add(per));
        g.m_done = from;
        g.m = to;
        match group_job(ctx, ns, c, g, out)? {
            Ok(()) => {}
            Err(why) => {
                g.m = end;
                return Ok(Err(why));
            }
        }
        from = to;
    }
    g.m_done = end;
    g.m = end;
    Ok(Ok(()))
}

/// ONE driver job of `g`'s combinations `m_done .. m`.
fn group_job(
    ctx: &CellCtx<'_>,
    ns: &NodeSpec,
    c: &CellRow,
    g: &mut Group,
    out: &mut CellOutcome,
) -> Result<Result<(), String>, String> {
    let spec = ctx.spec;
    let weights = |ci: usize| -> Vec<i64> {
        (0..g.keys.len())
            .map(|ki| weight(spec.combination_seed, &g.weight_tag, c.d, c.r, ci, ki))
            .collect()
    };
    let inputs_with = |w: &[i64]| -> Vec<TadicInput> {
        g.keys
            .iter()
            .zip(w)
            .map(|(k, &wt)| TadicInput::Package {
                top: ns.top.clone(),
                powers: k.clone(),
                weight: wt,
            })
            .collect()
    };
    // job v2 (`vectors`): the round's combinations as weight vectors of ONE request (one recording); labels
    // `<prefix><first>#<j>` = combination first + j. Else one request per combination, label `<prefix><i>`.
    let reqs: Vec<TadicRequest> = if spec.vectors {
        let ws: Vec<Vec<i64>> = (g.m_done..g.m).map(weights).collect();
        vec![TadicRequest {
            label: format!("{}{}", g.prefix, g.m_done),
            inputs: inputs_with(&ws[0]),
            weight_vectors: Some(WeightVectors::Explicit { vectors: ws }),
        }]
    } else {
        (g.m_done..g.m)
            .map(|ci| TadicRequest {
                label: format!("{}{ci}", g.prefix),
                inputs: inputs_with(&weights(ci)),
                weight_vectors: None,
            })
            .collect()
    };
    let jr = TadicJobRequest {
        level: TadicLevel::Package {
            path: spec.package.clone(),
            relations: spec.relations.clone(),
            library_sha256: spec.library.sha256.clone(),
        },
        requests: reqs,
        d0: 4,
        order: spec.order,
        primes: spec.primes,
        max_primes: spec.max_primes,
        seeds: spec.seeds.clone(),
        limits: TadicLimits {
            wall_seconds: spec.wall_seconds,
            max_length: spec.max_length,
            threads: spec.threads,
            memory_gib: None,
            no_step_cache: spec.no_step_cache,
        },
        crt: false,
        release_every: spec.release_every,
        record_per_seed: false,
        terminals: Some(DataFile::from_file(&spec.library.path).map_err(|e| e.to_string())?),
        dependences: ctx.deps.clone(),
        depths: Some(ctx.depths.clone()),
    };
    let content = ctx.adapter.content_of(&jr);
    if ctx.store.get(ctx.adapter, &jr)?.is_none()
        && let Some(re) = spec.reuse.iter().find(|e| e.cell == (c.d, c.r))
    {
        return reuse_job(ctx, &jr, re, g, out);
    }
    if ctx.store.get(ctx.adapter, &jr)?.is_none() {
        let plans = ctx
            .adapter
            .plan(std::slice::from_ref(&jr))
            .map_err(|e| e.to_string())?;
        let opts = RunOptions {
            cpus: vec![],
            memory_cap_bytes: Some(spec.memory_cap_bytes),
            licence_env: spec.licence_env.clone(),
            wall_cap: Some(Duration::from_secs(spec.wall_seconds + 600)),
            grace: Some(Duration::from_secs(30)),
        };
        let work = ctx.dir.join("work");
        let mut attempt = 1u32;
        while rustred_campaign_contract::layout::attempt_dir(&work, &plans[0].job_id, attempt)
            .exists()
        {
            attempt += 1;
        }
        let mut l = RealLauncher::new(ctx.sampler.clone());
        let (b, rep) = run_attempt(
            &mut l,
            ctx.adapter,
            std::slice::from_ref(&jr),
            &plans[0],
            &work,
            attempt,
            &opts,
        )
        .map_err(|e| e.to_string())?;
        if let Some(r) = &rep.report {
            out.cpu_seconds += r.cpu_seconds;
            out.peak_gb = out.peak_gb.max(r.peak_rss_bytes as f64 / 1e9);
        }
        if b.result.is_none() {
            return Ok(Err(format!("JOB: {}", b.unread.unwrap_or_default())));
        }
        ctx.store.put(&content, &rep.prepared)?;
    }
    let (checked, _, _) = ctx
        .store
        .get(ctx.adapter, &jr)?
        .ok_or("stored result vanished")?;
    let cert = certificate::check(&checked);
    if let Some(f) = cert.findings.first() {
        return Ok(Err(format!("NOT_CERTIFIED: {}", f.to_json())));
    }
    if let Some(q) = checked.requests.iter().find(|q| !q.outcome.is_decided()) {
        // a request that stopped at a key the package cannot reduce: name the boxed node and the key (acceptance cases)
        let stop = match &q.outcome {
            tadicjob::RequestOutcome::NotDecided { message, .. } => stop_key(message),
            _ => None,
        };
        return Ok(Err(match stop {
            Some(s) => format!("{}: {} at {s}", q.label, q.outcome.word()),
            None => format!("{}: {}", q.label, q.outcome.word()),
        }));
    }
    out.jobs.push(content.clone());
    g.results
        .push(ctx.store.root.join(&content).join(tadicjob::CHILD_OUT));
    g.m_done = g.m;
    Ok(Ok(()))
}

/// The rank-only route of one round: the stored attempt `re` replaces the driver job iff [`verify_reuse`] accepts it;
/// a read-only copy keyed by the job's SHA-256 is kept in the campaign directory and its result feeds the rank step.
fn reuse_job(
    ctx: &CellCtx<'_>,
    jr: &TadicJobRequest,
    re: &ReuseEntry,
    g: &mut Group,
    out: &mut CellOutcome,
) -> Result<Result<(), String>, String> {
    let jb = re.job.check("reused driver job")?;
    let rb = re.result.check("reused driver result")?;
    if let Err(why) = verify_reuse(ctx.adapter, jr, &jb, &rb) {
        return Ok(Err(format!("REUSE REFUSED: {why}")));
    }
    let (jsha, rsha) = (sha256_hex(&jb), sha256_hex(&rb));
    let d = ctx.dir.join("reused").join(&jsha);
    std::fs::create_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
    for (name, b) in [(tadicjob::JOB_INPUT, &jb), (tadicjob::CHILD_OUT, &rb)] {
        let f = d.join(name);
        match std::fs::read(&f) {
            Ok(have) if have == **b => {}
            Ok(_) => return Err(format!("{}: differs from the reused bytes", f.display())),
            Err(_) => std::fs::write(&f, b).map_err(|e| format!("{}: {e}", f.display()))?,
        }
    }
    let seconds = serde_json::from_slice::<Value>(&rb)
        .ok()
        .and_then(|v| v["seconds"].as_f64());
    out.reused.push(json!({"job": re.job.path, "job_sha256": jsha, "result": re.result.path,
        "result_sha256": rsha, "copy": d, "driver_seconds": seconds,
        "route": "rank-only: driver result reused (verify_reuse), rank step bound to the spec's inputs"}));
    out.jobs.push(format!("reused:{jsha}"));
    g.results.push(d.join(tadicjob::CHILD_OUT));
    g.m_done = g.m;
    Ok(Ok(()))
}

/// One cell: the union route (one group over all keys) or the block route (one group per block; the cell rank from
/// the stacked rows of every block, each block stable on its own: combinations >= its rank + 3).
fn run_cell(
    ctx: &CellCtx<'_>,
    ns: &NodeSpec,
    c: &CellRow,
    line: &'static str,
) -> Result<CellOutcome, String> {
    let spec = ctx.spec;
    let keys = cell_keys(
        ns.arity,
        &ns.lines,
        &ns.isps,
        c.d,
        c.r,
        ns.max_dots_per_line,
    );
    let block = spec.route.block_keys(c.r)?;
    let mut out = CellOutcome {
        node: ns.node.clone(),
        d: c.d,
        r: c.r,
        offset: ctx.shift.map(|s| c.r - (c.e + c.d - s)),
        keys: keys.len(),
        line,
        outside_box: c.r > c.box_r_max,
        units_reaching: c.units_reaching.clone(),
        combinations: 0,
        verdict: String::new(),
        rank: Value::Null,
        pivots: Value::Null,
        rank_kind: String::new(),
        naive_rank: Value::Null,
        cpu_seconds: 0.0,
        peak_gb: 0.0,
        max_plan_need: Value::Null,
        max_series_length: Value::Null,
        jobs: vec![],
        package: spec.library.sha256.clone(),
        route: if block.is_some() {
            "blocks".into()
        } else {
            "union".into()
        },
        blocks: vec![],
        requests: vec![],
        tripwires: vec![],
        reused: vec![],
    };
    if keys.len() as u64 != c.keys_in_cell {
        out.verdict = format!(
            "not decided (KEY_COUNT: enumerator {} keys, cell table {})",
            keys.len(),
            c.keys_in_cell
        );
        return Ok(out);
    }
    let cdir = ctx
        .dir
        .join("cells")
        .join(format!("{}_D{}r{}", ns.node, c.d, c.r));
    std::fs::create_dir_all(&cdir).map_err(|e| e.to_string())?;
    // the cost of jobs reused from the store is the cost recorded when they ran (the previous cell record, same jobs)
    let previous: Option<Value> = std::fs::read(cdir.join("cell.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok());
    let mut groups: Vec<Group> = match block {
        None => vec![Group {
            prefix: "c".into(),
            weight_tag: ns.node.clone(),
            keys: keys.clone(),
            m_done: 0,
            m: spec.m_initial.max(1),
            results: vec![],
            stable: false,
            rank: Value::Null,
        }],
        Some(k) => keys
            .chunks(k)
            .enumerate()
            .map(|(j, ch)| Group {
                prefix: format!("b{j}c"),
                weight_tag: format!("{}#b{j}", ns.node),
                keys: ch.to_vec(),
                m_done: 0,
                m: spec.m_initial.max(1),
                results: vec![],
                stable: false,
                rank: Value::Null,
            })
            .collect(),
    };
    let mut failed: Option<String> = None;
    for g in groups.iter_mut() {
        loop {
            if let Err(why) = group_round(ctx, ns, c, g, &mut out)? {
                failed = Some(why);
                break;
            }
            let r = rank_of(
                ctx,
                &cdir,
                &g.results,
                &format!("{}m{}", g.prefix, g.m),
                &g.prefix,
            )?;
            let v = r["verdict"].as_str().unwrap_or("").to_string();
            g.rank = r["rank"].clone();
            if v.contains("NEEDS_MORE_COMBINATIONS") && g.m < spec.m_max {
                g.m = (g.m + spec.m_step.max(1)).min(spec.m_max);
                continue;
            }
            g.stable = !v.starts_with("not decided");
            if !g.stable && !v.contains("NEEDS_MORE_COMBINATIONS") {
                failed = Some(v);
            }
            break;
        }
        if failed.is_some() {
            break;
        }
    }
    out.combinations = groups.iter().map(|g| g.m_done).sum();
    if !out.reused.is_empty() {
        out.route = format!(
            "{} (driver result reused; rank step bound to the spec's inputs)",
            out.route
        );
    }
    out.blocks = groups
        .iter()
        .map(|g| {
            json!({"prefix": g.prefix, "weight_stream": g.weight_tag, "keys": g.keys.len(),
            "combinations": g.m_done, "rank": g.rank, "stable": g.stable})
        })
        .collect();
    let all: Vec<PathBuf> = groups.iter().flat_map(|g| g.results.clone()).collect();
    if let Some(why) = failed {
        out.verdict = format!("not decided ({why})");
    } else if !all.is_empty() {
        let prefix = if block.is_some() { "b" } else { "c" };
        let rank = rank_of(ctx, &cdir, &all, "cell", prefix)?;
        if spec.dependences.is_some() {
            out.naive_rank =
                rank_of_with(ctx, &cdir, &all, "cell_naive", prefix, false)?["rank"].clone();
        }
        out.rank = rank["rank"].clone();
        out.pivots = rank["pivots"].clone();
        out.tripwires = tripwire_records(&ns.tripwires, &rank);
        out.rank_kind = rank["rank_kind"].as_str().unwrap_or("").to_string();
        let v = rank["verdict"].as_str().unwrap_or("").to_string();
        out.verdict = if groups.iter().any(|g| !g.stable) {
            format!(
                "not decided (NEEDS_MORE_COMBINATIONS: a group did not stabilise within m_max {})",
                spec.m_max
            )
        } else if v.starts_with("not decided") && !v.contains("NEEDS_MORE_COMBINATIONS") {
            v
        } else {
            match out.rank.as_u64() {
                Some(0) => "count zero: yes".into(),
                Some(r) => format!("count zero: no (rank {r})"),
                None => v,
            }
        };
        let (mut mpn, mut msl): (Option<i64>, Option<i64>) = (None, None);
        for q in rank["requests"].as_array().into_iter().flatten() {
            let label = q["label"].as_str().unwrap_or("");
            let g = groups.iter().find(|g| label.starts_with(&g.prefix));
            for cc in q["replay"].as_array().into_iter().flatten() {
                mpn = mpn.max(cc["plan_max_need"].as_i64());
                msl = msl.max(cc["max_series_length"].as_i64());
            }
            out.requests.push(
                json!({"label": label, "keys_in_union": g.map(|g| g.keys.len()), "r": c.r, "d": c.d,
                "replay": q["replay"]}),
            );
        }
        out.max_plan_need = json!(mpn);
        out.max_series_length = json!(msl);
    }
    if let Some(p) = previous
        && out.cpu_seconds == 0.0
        && !out.jobs.is_empty()
        && p["jobs"] == json!(out.jobs)
    {
        out.cpu_seconds = p["cpu_seconds"].as_f64().unwrap_or(0.0);
        out.peak_gb = p["peak_gb"].as_f64().unwrap_or(0.0);
    }
    let _ = std::fs::write(
        cdir.join("cell.json"),
        serde_json::to_vec_pretty(&out).expect("json"),
    );
    Ok(out)
}

/// `rr_campaign pole-report --spec S.json --dir D`: run (or resume) the report; prints the markdown table.
pub fn report_cmd(args: &[String]) -> Result<String, String> {
    let (Some(sp), Some(dir)) = (arg(args, "--spec"), arg(args, "--dir")) else {
        return Err("usage: pole-report --spec S.json --dir D".into());
    };
    let (sv, sb) = read_json(&sp)?;
    let mut spec: PoleReportSpec = serde_json::from_value(sv).map_err(|e| format!("spec: {e}"))?;
    if spec.schema != SPEC_SCHEMA {
        return Err(format!("spec schema {} is not {SPEC_SCHEMA}", spec.schema));
    }
    if spec.seeds.len() < 2 {
        return Err("two seeds at least (the second producer)".into());
    }
    let cells_bytes = spec.cells.check("cell table")?;
    for (b, w) in [
        (&spec.library, "library"),
        (&spec.driver, "driver"),
        (&spec.package_binary, "package binary"),
        (&spec.node_binary, "node binary"),
        (&spec.rank_bin, "rank binary"),
        (&spec.depths, "depths"),
        (&spec.catalogue, "catalogue"),
    ] {
        b.check(w)?;
    }
    if let Some(d) = &spec.dependences {
        d.check("dependences")?;
    }
    if let Some(m) = &spec.terminal_map {
        m.check("terminal map")?;
    }
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(dir.join("cells")).map_err(|e| e.to_string())?;
    // spec rule: every data input is read from a launch-time snapshot by sha256 (never from a republishable path)
    let input_snapshots = snapshot_inputs(&mut spec, &dir)?;
    let rows = read_cell_tops(std::str::from_utf8(&cells_bytes).map_err(|e| e.to_string())?)?;
    let program = match &spec.fake_program {
        Some(p) => Program::from_file(
            "rr_fake_child",
            p,
            crate::adapters::job::Invocation::Contract,
        )
        .map_err(|e| e.to_string())?,
        None => Program::from_file(
            "tadic_job",
            &spec.python,
            crate::adapters::job::Invocation::Legacy,
        )
        .map_err(|e| e.to_string())?,
    };
    let driver = DataFile::from_file(&spec.driver.path).map_err(|e| e.to_string())?;
    let adapter = TadicAdapter {
        program,
        driver,
        package_binary: (
            spec.package_binary.path.clone(),
            spec.package_binary.sha256.clone(),
        ),
        node_binary: (
            spec.node_binary.path.clone(),
            spec.node_binary.sha256.clone(),
        ),
        licence_env: spec.licence_env.clone(),
        fake: spec
            .fake_program
            .as_ref()
            .map(|_| crate::adapters::default_emulation_script()),
    };
    let store = ResultStore {
        root: dir.join("store"),
    };
    let sampler = Arc::new(
        Sampler::spawn(SamplerConfig {
            interval: Duration::from_millis(500),
            collect_foreign_pinning: false,
        })
        .map_err(|e| format!("sampler: {e}"))?,
    );
    let depths = DataFile::from_file(&spec.depths.path).map_err(|e| e.to_string())?;
    let deps = match &spec.dependences {
        Some(d) => Some(DataFile::from_file(&d.path).map_err(|e| e.to_string())?),
        None => None,
    };
    let mut outcomes: Vec<CellOutcome> = vec![];
    let mut shifts: BTreeMap<String, Option<(i64, String)>> = BTreeMap::new();
    for ns in &spec.nodes {
        shifts.insert(
            ns.node.clone(),
            offset_shift(&rows, &ns.node, spec.offset_shift)?,
        );
    }
    for ns in &spec.nodes {
        let shift = shifts.get(&ns.node).cloned().flatten().map(|x| x.0);
        let mut cells: Vec<(&CellRow, &'static str)> = rows
            .iter()
            .filter(|c| c.node == ns.node)
            .filter_map(|c| {
                if c.r <= c.top {
                    Some((c, "main"))
                } else if c.r <= c.top_upper && spec.include_margin {
                    Some((c, "margin"))
                } else {
                    None
                }
            })
            .filter(|(c, _)| {
                spec.d_max.is_none_or(|x| c.d <= x)
                    && spec.r_max.is_none_or(|x| c.r <= x)
                    && spec
                        .cell_list
                        .as_ref()
                        .is_none_or(|l| l.contains(&(c.d, c.r)))
            })
            .collect();
        cells.sort_by_key(|(c, _)| (c.keys_in_cell, c.d, c.r));
        for (c, line) in cells {
            let ctx = CellCtx {
                spec: &spec,
                adapter: &adapter,
                store: &store,
                sampler: &sampler,
                depths: &depths,
                deps: &deps,
                dir: &dir,
                shift,
            };
            let out = run_cell(&ctx, ns, c, line)?;
            outcomes.push(out);
        }
    }
    // TRICKS 49: a cell computed earlier (same campaign directory) that the CURRENT cell table no longer lists is kept and
    // shown as "outside the exact tops, computed anyway" (its results stay in the store and are reused by content)
    let listed: std::collections::BTreeSet<(String, i64, i64)> = outcomes
        .iter()
        .map(|o| (o.node.clone(), o.d, o.r))
        .collect();
    if let Ok(rd) = std::fs::read_dir(dir.join("cells")) {
        let mut extra = vec![];
        for e in rd.flatten() {
            let Ok(b) = std::fs::read(e.path().join("cell.json")) else {
                continue;
            };
            let Ok(v) = serde_json::from_slice::<Value>(&b) else {
                continue;
            };
            let key = (
                v["node"].as_str().unwrap_or("").to_string(),
                v["d"].as_i64().unwrap_or(-1),
                v["r"].as_i64().unwrap_or(-1),
            );
            // only cells of THIS package (a cell record of another package is never mixed into the verdict)
            if !listed.contains(&key)
                && spec.nodes.iter().any(|n| n.node == key.0)
                && v["package"].as_str() == Some(spec.library.sha256.as_str())
            {
                extra.push(v);
            }
        }
        extra.sort_by_key(|v| (v["d"].as_i64(), v["r"].as_i64()));
        for v in extra {
            // a cell the CURRENT table still lists (not selected in this run) keeps its line and verdict; only a cell the
            // table no longer lists is "dropped"
            let (d, r) = (v["d"].as_i64().unwrap_or(-1), v["r"].as_i64().unwrap_or(-1));
            let node = v["node"].as_str().unwrap_or("");
            let in_table = rows
                .iter()
                .find(|c| c.node == node && c.d == d && c.r == r)
                .map(|c| {
                    if c.r <= c.top {
                        "main"
                    } else if c.r <= c.top_upper {
                        "margin"
                    } else {
                        "dropped"
                    }
                });
            let line: &'static str = in_table.unwrap_or("dropped");
            let verdict = if line == "dropped" {
                format!(
                    "outside the exact tops, computed anyway: {}",
                    v["verdict"].as_str().unwrap_or("")
                )
            } else {
                v["verdict"].as_str().unwrap_or("").to_string()
            };
            let offset = shifts
                .get(node)
                .cloned()
                .flatten()
                .zip(rows.iter().find(|c| c.node == node).map(|c| c.e))
                .map(|((s, _), e)| r - (e + d - s))
                .or(v["offset"].as_i64());
            outcomes.push(CellOutcome {
                node: v["node"].as_str().unwrap_or("").to_string(),
                d,
                r,
                offset,
                keys: v["keys"].as_u64().unwrap_or(0) as usize,
                line,
                outside_box: v["outside_box"].as_bool().unwrap_or(false),
                units_reaching: v["units_reaching"].as_str().unwrap_or("").to_string(),
                combinations: v["combinations"].as_u64().unwrap_or(0) as usize,
                verdict,
                rank: v["rank"].clone(),
                pivots: v["pivots"].clone(),
                rank_kind: v["rank_kind"].as_str().unwrap_or("").to_string(),
                naive_rank: v["naive_rank"].clone(),
                cpu_seconds: v["cpu_seconds"].as_f64().unwrap_or(0.0),
                peak_gb: v["peak_gb"].as_f64().unwrap_or(0.0),
                max_plan_need: v["max_plan_need"].clone(),
                max_series_length: v["max_series_length"].clone(),
                jobs: vec![],
                route: v["route"].as_str().unwrap_or("").to_string(),
                package: v["package"].as_str().unwrap_or("").to_string(),
                blocks: v["blocks"].as_array().cloned().unwrap_or_default(),
                requests: v["requests"].as_array().cloned().unwrap_or_default(),
                tripwires: v["tripwires"].as_array().cloned().unwrap_or_default(),
                reused: v["reused"].as_array().cloned().unwrap_or_default(),
            });
        }
    }
    let upper = spec.dependences.is_none();
    let mut nodes = BTreeMap::new();
    for ns in &spec.nodes {
        let mine: Vec<&CellOutcome> = outcomes
            .iter()
            .filter(|o| o.node == ns.node && o.line == "main")
            .collect();
        // EVERY main-line cell of the node in the cell table must be computed (a spec that selects fewer cells never
        // gives "yes")
        let table_main = rows
            .iter()
            .filter(|c| c.node == ns.node && c.r <= c.top)
            .count();
        let all_zero = mine.len() == table_main
            && !mine.is_empty()
            && mine.iter().all(|o| o.verdict == "count zero: yes");
        let any_no = mine.iter().any(|o| o.verdict.starts_with("count zero: no"));
        // TRICKS 59: a cell touching an unmapped terminal is never counted (neither zero nor non-zero)
        let incomplete: Vec<String> = mine
            .iter()
            .filter(|o| o.verdict.starts_with("incomplete"))
            .map(|o| format!("D{}r{}", o.d, o.r))
            .collect();
        let verdict = if all_zero {
            "count zero on all cells up to top(D): yes".to_string()
        } else if any_no && !upper {
            "count zero on all cells up to top(D): no".to_string()
        } else if any_no {
            "count zero on all cells up to top(D): not decided (naive upper bound > 0: no dependence table)".to_string()
        } else if !incomplete.is_empty() {
            format!(
                "count zero on all cells up to top(D): not decided ({} cell(s) incomplete: unmapped terminal, never counted: {})",
                incomplete.len(),
                incomplete.join(", ")
            )
        } else {
            "count zero on all cells up to top(D): not decided (cells not decided or not run)"
                .to_string()
        };
        nodes.insert(ns.node.clone(), verdict);
    }
    let report = json!({"schema": REPORT_SCHEMA, "spec_sha256": sha256_hex(&sb), "cells_sha256": spec.cells.sha256,
        "depths_sha256": spec.depths.sha256, "dependences_sha256": spec.dependences.as_ref().map(|d| &d.sha256),
        "score": {"level": "composed", "form": "rank", "claim": if upper {"objective_upper_bound"} else {"objective"}},
        "cell_definition": "cells (D, r) with r <= top(D); top(D) = top_r_at_D_if_n4hint of the cell table when numeric, else top_r_exact (exact n4, TRICKS 49), else top_r_at_D; never clipped by box_r_max (a main-line cell above it is shown as outside BOXDEF v3); margin = cells between top(D) and top_r_at_D (TRICKS 45, coordinator ruling 2026-11-17)",
        "offsets": shifts.iter().map(|(n, x)| (n.clone(), match x {
            Some((s, src)) => json!({"definition": "o = r - (E + D - s)", "s": s, "source": src}),
            None => json!({"definition": "o = r - (E + D - s)", "s": null, "source": "no reference line (no row 'on' the line, no spec offset_shift)"}),
        })).collect::<BTreeMap<_, _>>(),
        "inputs": input_snapshots,
        "nodes": nodes, "cells": outcomes});
    let tmp = dir.join("pole_report.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&report).expect("json"))
        .map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dir.join("pole_report.json")).map_err(|e| e.to_string())?;
    let md = markdown(&report);
    std::fs::write(dir.join("pole_report.md"), &md).map_err(|e| e.to_string())?;
    Ok(md)
}

/// The tripwire column of one cell: `master need/q` per wire, `FIRED` where the need passes q + allowance.
fn tripwire_text(c: &Value) -> String {
    let w: Vec<String> = c["tripwires"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|t| {
            let need = t["need"]
                .as_i64()
                .map_or("-".to_string(), |n| n.to_string());
            let q = t["q"].as_i64().map_or("?".to_string(), |n| n.to_string());
            let mark = match t["fired"].as_bool() {
                Some(true) => " FIRED",
                Some(false) => "",
                None => " (q unknown)",
            };
            format!("{} {need}/{q}{mark}", t["master"].as_str().unwrap_or(""))
        })
        .collect();
    if w.is_empty() {
        "-".into()
    } else {
        w.join("; ")
    }
}

/// The markdown table of a report: per node, cells by OFFSET o = r - (E + D - s) (TRICKS 64), then D; the tripwire
/// masters beside the true and the naive rank; a cell touching an unmapped terminal is "incomplete", never counted.
pub fn markdown(report: &Value) -> String {
    let mut s = String::from("# Pole report (rank of rho per cell)\n\n");
    s.push_str(&format!(
        "Score: level `composed`, form `rank`, claim `{}`. Cell definition: {}.\n\n",
        report["score"]["claim"].as_str().unwrap_or(""),
        report["cell_definition"].as_str().unwrap_or("")
    ));
    for (n, v) in report["nodes"].as_object().into_iter().flatten() {
        let off = &report["offsets"][n];
        s.push_str(&format!(
            "- **{n}**: {} (offset o = r - (E + D - s), s = {} from {})\n",
            v.as_str().unwrap_or(""),
            off["s"],
            off["source"].as_str().unwrap_or("-")
        ));
    }
    s.push_str("\n| node | o | cell | line | keys | verdict (TRUE rank) | true rank | naive rank | tripwires need/q | surviving directions | comb. | CPU-s | peak GB | max need | max series |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n");
    let mut cells: Vec<&Value> = report["cells"].as_array().into_iter().flatten().collect();
    cells.sort_by_key(|c| {
        (
            c["node"].as_str().unwrap_or("").to_string(),
            c["offset"].as_i64().is_none(),
            c["offset"].as_i64(),
            c["d"].as_i64(),
            c["r"].as_i64(),
        )
    });
    for c in cells {
        let mut line = c["line"].as_str().unwrap_or("").to_string();
        if c["outside_box"] == true {
            line = format!(
                "{line}; outside BOXDEF v3, reached by {} units",
                c["units_reaching"].as_str().unwrap_or("?")
            );
        }
        s.push_str(&format!(
            "| {} | {} | D{}r{} | {} | {} | {} | {} | {} | {} | {} | {} | {:.0} | {:.1} | {} | {} |\n",
            c["node"].as_str().unwrap_or(""),
            c["offset"].as_i64().map_or("-".to_string(), |o| format!("{o:+}")),
            c["d"],
            c["r"],
            line,
            c["keys"],
            c["verdict"].as_str().unwrap_or(""),
            c["rank"],
            c["naive_rank"],
            tripwire_text(c),
            c["pivots"],
            c["combinations"],
            c["cpu_seconds"].as_f64().unwrap_or(0.0),
            c["peak_gb"].as_f64().unwrap_or(0.0),
            c["max_plan_need"],
            c["max_series_length"]
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_keys_count_and_shape() {
        // 12 line slots, 8 ISP slots (an example frame): D0r1 -> 8, D0r2 -> 36, D1r2 -> 12 x 36
        let lines: Vec<usize> = (0..12).collect();
        let isps: Vec<usize> = (12..20).collect();
        assert_eq!(cell_keys(20, &lines, &isps, 0, 1, 4).len(), 8);
        assert_eq!(cell_keys(20, &lines, &isps, 0, 2, 4).len(), 36);
        assert_eq!(cell_keys(20, &lines, &isps, 1, 2, 4).len(), 432);
        let k = &cell_keys(20, &lines, &isps, 1, 0, 4)[0];
        assert_eq!(k.iter().filter(|&&x| x == 2).count(), 1);
        // the dot cap: 5 dots on 2 lines with at most 4 each -> (4,1), (3,2), (2,3), (1,4)
        assert_eq!(cell_keys(3, &[0, 1], &[2], 5, 0, 4).len(), 4);
    }

    #[test]
    fn stop_key_is_read_from_an_engine_refusal() {
        let m = "seeds [1]: recipe OUTSIDE_NO_RELATION failed for boxed node T9: recipe x could not solve the key I(1,5,-2) (recipe frame I(5,1,-2)): outside";
        assert_eq!(stop_key(m).as_deref(), Some("boxed node T9 key I(1,5,-2)"));
        assert_eq!(stop_key("nothing"), None);
    }

    #[test]
    fn weights_are_reproducible_and_in_range() {
        let w = weight(7, "N", 1, 2, 3, 4);
        assert_eq!(w, weight(7, "N", 1, 2, 3, 4));
        assert!((1..(1i64 << 62)).contains(&w));
        assert_ne!(w, weight(7, "N", 1, 2, 3, 5));
    }

    #[test]
    fn cell_tops_main_and_margin() {
        let t = "node\tE\tD\tr\ttop_r_at_D\tis_top\ttop_r_at_D_if_n4hint\tkeys_in_cell\tbox_r_max\tin_box\tvs_old_line\tqcd_units_reaching\n\
                 X\t12\t0\t5\t6\t0\t5\t792\t5\t1\tabove\t52\n\
                 X\t12\t0\t6\t6\t1\t5\t1716\t5\t0\tabove\t19\n\
                 Y\t14\t1\t7\t7\t1\t\t9\t7\t1\tabove\t3\n";
        let rows = read_cell_tops(t).unwrap();
        assert_eq!(rows[0].top, 5);
        assert!(rows[0].r <= rows[0].top);
        assert!(rows[1].r > rows[1].top && rows[1].r <= rows[1].top_upper);
        assert_eq!(rows[2].top, 7, "an empty hint takes the upper bound");
        // the exact-n4 table: the hint is 'superseded', the exact top decides
        let t2 = "node\tE\tD\tr\ttop_r_at_D\ttop_r_at_D_if_n4hint\tkeys_in_cell\tbox_r_max\tqcd_units_reaching\ttop_r_exact\n\
                  X\t12\t0\t3\t3\tsuperseded\t120\t5\t52\t3\n";
        let rows = read_cell_tops(t2).unwrap();
        assert_eq!((rows[0].top, rows[0].top_upper), (3, 3));
    }

    #[test]
    fn offset_shift_from_the_rows_on_the_line_or_the_spec() {
        // test data: a node with E = 12 whose line is r = E + D - 11 (rows 'on' at D0r1 and D1r2)
        let t = "node\tE\tD\tr\ttop_r_at_D\tkeys_in_cell\tbox_r_max\tvs_old_line\tqcd_units_reaching\n\
                 X\t12\t0\t0\t3\t1\t5\tbelow\t52\n\
                 X\t12\t0\t1\t3\t8\t5\ton\t52\n\
                 X\t12\t1\t2\t4\t432\t5\ton\t52\n\
                 X\t12\t0\t3\t3\t120\t5\tabove\t52\n\
                 Y\t13\t0\t2\t3\t36\t6\tabove\t9\n";
        let rows = read_cell_tops(t).unwrap();
        assert_eq!(rows[1].vs_line, "on");
        let (sx, _) = offset_shift(&rows, "X", None).unwrap().unwrap();
        assert_eq!(sx, 11);
        // o = r - (E + D - s): D0r3 -> +2, D0r0 -> -1
        assert_eq!(rows[3].r - (rows[3].e + rows[3].d - sx), 2);
        assert_eq!(rows[0].r - (rows[0].e + rows[0].d - sx), -1);
        // no row on the line and no spec value: no offset; the spec value wins over the table
        assert_eq!(offset_shift(&rows, "Y", None).unwrap(), None);
        assert_eq!(offset_shift(&rows, "Y", Some(9)).unwrap().unwrap().0, 9);
        // rows that disagree are an error, never averaged
        let bad = "node\tE\tD\tr\ttop_r_at_D\tkeys_in_cell\tbox_r_max\tvs_old_line\tqcd_units_reaching\n\
                   X\t12\t0\t1\t3\t8\t5\ton\t52\n\
                   X\t12\t1\t3\t4\t1440\t5\ton\t52\n";
        assert!(offset_shift(&read_cell_tops(bad).unwrap(), "X", None).is_err());
    }

    #[test]
    fn tripwires_need_against_depth_with_allowance() {
        let wires = vec![
            Tripwire {
                master: "m8".into(),
                allowance: 0,
            },
            Tripwire {
                master: "m11".into(),
                allowance: 0,
            },
            Tripwire {
                master: "m137".into(),
                allowance: 1,
            },
            Tripwire {
                master: "absent".into(),
                allowance: 0,
            },
            Tripwire {
                master: "old".into(),
                allowance: 0,
            },
        ];
        let rank = json!({"cell_needs": {"m8": 5, "m11": 6, "m137": 3, "old": 4},
            "cell_depths": {"m8": 5, "m11": 5, "m137": 2},
            "cell_beyond": [{"master": "old", "need": 4, "q": 3}]});
        let t = tripwire_records(&wires, &rank);
        let fired: Vec<Value> = t.iter().map(|x| x["fired"].clone()).collect();
        // m8 at its depth: not fired; m11 one above: fired; m137 one above with allowance 1: not fired; absent: not
        // fired; "old" (no cell_depths entry, q from cell_beyond): fired
        assert_eq!(
            fired,
            vec![
                json!(false),
                json!(true),
                json!(false),
                json!(false),
                json!(true)
            ]
        );
        assert_eq!(t[1]["need"], 6);
        assert_eq!(t[1]["q"], 5);
        assert_eq!(t[3]["need"], Value::Null);
        // need known, q unknown: not decided (null), never "not fired"
        let u = tripwire_records(&wires[..1], &json!({"cell_needs": {"m8": 9}}));
        assert_eq!(u[0]["fired"], Value::Null);
    }

    #[test]
    fn markdown_orders_cells_by_offset_and_shows_tripwires() {
        let cell = |d: i64, r: i64, o: Option<i64>, v: &str| {
            json!({"node": "X", "d": d, "r": r, "offset": o, "line": "main", "keys": 1, "verdict": v,
                "rank": 0, "naive_rank": 0, "pivots": [], "combinations": 4, "cpu_seconds": 1.0, "peak_gb": 1.0,
                "max_plan_need": 1, "max_series_length": 1,
                "tripwires": [{"master": "m11", "need": 6, "q": 5, "allowance": 0, "fired": true}]})
        };
        let report = json!({"score": {"claim": "objective"}, "cell_definition": "test",
            "offsets": {"X": {"s": 11, "source": "test"}},
            "nodes": {"X": "count zero on all cells up to top(D): not decided (1 cell(s) incomplete: unmapped terminal, never counted: D0r2)"},
            "cells": [cell(0, 3, Some(2), "count zero: yes"), cell(1, 0, Some(-2), "count zero: yes"),
                      cell(0, 2, Some(1), "incomplete: unmapped terminal (R:9)"), cell(0, 1, Some(0), "count zero: yes")]});
        let md = markdown(&report);
        let order: Vec<usize> = ["| -2 | D1r0", "| +0 | D0r1", "| +1 | D0r2", "| +2 | D0r3"]
            .iter()
            .map(|k| md.find(k).unwrap_or_else(|| panic!("{k} missing in\n{md}")))
            .collect();
        assert!(order.windows(2).all(|w| w[0] < w[1]), "{md}");
        assert!(md.contains("m11 6/5 FIRED"), "{md}");
        assert!(md.contains("incomplete: unmapped terminal (R:9)"));
        assert!(md.contains("s = 11"));
    }

    #[test]
    fn reuse_ignores_only_the_rank_only_fields() {
        let ours = json!({"schema": "s", "job_id": "a.tr", "requests": [{"label": "c0", "inputs": [{"top": "X", "powers": [1, 0], "weight": 5}]}],
            "expansion": {"d0": 4, "order": 1}, "primes": {"count": 2, "max_primes": 8, "seeds": [1, 2]},
            "limits": {"wall_seconds": 100, "max_length": 64, "threads": 4}, "crt": false,
            "binary": {"path": "/b", "sha256": "bb"}, "dependence_table": {"path": "/d1", "sha256": "d1"}});
        // another job id, wall cap and (bound, not applied) dependence table: compatible
        let mut stored = ours.clone();
        stored["job_id"] = json!("b.tr");
        stored["limits"]["wall_seconds"] = json!(7);
        stored["dependence_table"] = json!({"path": "/d0", "sha256": "d0"});
        assert!(reuse_compatible(&stored, &ours).is_ok());
        let mut no_table = stored.clone();
        no_table.as_object_mut().unwrap().remove("dependence_table");
        assert!(reuse_compatible(&no_table, &ours).is_ok());
        // anything that changes the answers: refused, naming the field
        let mut w = stored.clone();
        w["requests"][0]["inputs"][0]["weight"] = json!(6);
        assert!(
            reuse_compatible(&w, &ours)
                .unwrap_err()
                .contains("requests")
        );
        let mut b = stored.clone();
        b["binary"]["sha256"] = json!("cc");
        assert!(reuse_compatible(&b, &ours).unwrap_err().contains("binary"));
        let mut l = stored.clone();
        l["limits"]["max_length"] = json!(65);
        assert!(reuse_compatible(&l, &ours).unwrap_err().contains("limits"));
        let mut p = stored;
        p["primes"]["seeds"] = json!([1, 3]);
        assert!(reuse_compatible(&p, &ours).unwrap_err().contains("primes"));
    }

    #[test]
    fn data_inputs_are_read_from_launch_snapshots_by_sha256() {
        let d = std::env::temp_dir().join(format!("polereport-snap-{}", std::process::id()));
        let src = d.join("src");
        std::fs::create_dir_all(&src).unwrap();
        let bound = |name: &str, body: &str| {
            let p = src.join(name);
            std::fs::write(&p, body).unwrap();
            json!({"path": p, "sha256": sha256_hex(body.as_bytes())})
        };
        let spec_json = json!({"schema": SPEC_SCHEMA, "cells": bound("cells.tsv", "c"), "nodes": [],
            "package": "/pkg", "library": bound("library.json", "l"), "python": "/py",
            "driver": bound("drv.py", "v"), "package_binary": bound("pkb", "p"), "node_binary": bound("nb", "n"),
            "rank_bin": bound("rank", "r"), "depths": bound("depths.json", "depths-v1"),
            "dependences": bound("deps.json", "deps-v1"), "catalogue": bound("cat.json", "k"),
            "terminal_map": bound("map.json", "m"), "seeds": [1, 2], "primes": 2, "order": 1,
            "combination_seed": 1, "m_initial": 4, "m_step": 3, "m_max": 16, "wall_seconds": 1, "max_length": 1,
            "threads": 1, "memory_cap_bytes": 1});
        let campaign = d.join("campaign");
        let mut spec: PoleReportSpec = serde_json::from_value(spec_json.clone()).unwrap();
        let prov = snapshot_inputs(&mut spec, &campaign).unwrap();
        let deps = spec.dependences.clone().unwrap();
        assert!(
            deps.path.starts_with(campaign.join("inputs")),
            "{}",
            deps.path.display()
        );
        assert_eq!(std::fs::read(&deps.path).unwrap(), b"deps-v1");
        assert_eq!(prov["dependences"]["source"], json!(src.join("deps.json")));
        // binaries and the driver are pinned in place (frozen sets), not copied
        assert_eq!(spec.driver.path, src.join("drv.py"));
        // the source is REPUBLISHED: the snapshot still holds the launch bytes and still matches the bound sha256
        std::fs::write(src.join("deps.json"), "deps-v2").unwrap();
        assert_eq!(std::fs::read(&deps.path).unwrap(), b"deps-v1");
        assert!(deps.check("dependences").is_ok());
        // a resumed launch (source restored) re-hashes the existing snapshot and accepts it
        std::fs::write(src.join("deps.json"), "deps-v1").unwrap();
        let mut again: PoleReportSpec = serde_json::from_value(spec_json.clone()).unwrap();
        snapshot_inputs(&mut again, &campaign).unwrap();
        assert_eq!(again.dependences.unwrap().path, deps.path);
        // a tampered snapshot is refused
        let mut perm = std::fs::metadata(&deps.path).unwrap().permissions();
        assert!(perm.readonly());
        #[allow(clippy::permissions_set_readonly_false)]
        perm.set_readonly(false);
        std::fs::set_permissions(&deps.path, perm).unwrap();
        std::fs::write(&deps.path, "tampered").unwrap();
        let mut third: PoleReportSpec = serde_json::from_value(spec_json).unwrap();
        assert!(snapshot_inputs(&mut third, &campaign).is_err());
        // a launch whose source no longer matches the spec's sha256 is refused before any snapshot
        std::fs::write(src.join("deps.json"), "deps-v3").unwrap();
        let mut fourth = spec;
        fourth.dependences.as_mut().unwrap().path = src.join("deps.json");
        assert!(snapshot_inputs(&mut fourth, &d.join("campaign2")).is_err());
    }
}
