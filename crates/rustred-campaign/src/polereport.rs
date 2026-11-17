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
//! (pinned by sha256, run as a subprocess on the verified results). Resume: the result store (a stored job is never
//! rerun) and the per-cell records in the campaign directory.
//!
//! Generic (L8): nodes, slots, cells, packages, masters, depths and seeds are data.

use crate::adapters::job::{JobAdapter, Program, RunOptions, run_attempt, sha256_hex};
use crate::adapters::tadicjob::{
    DataFile, ResultStore, TadicAdapter, TadicInput, TadicJobRequest, TadicLevel, TadicLimits,
    TadicRequest,
};
use crate::launch::RealLauncher;
use crate::resources::{Sampler, SamplerConfig};
use rustred_campaign_contract::tadicjob::{self, Pins, certificate};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
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
            let certified = rep.job_certified() && rerun_diffs.as_ref().is_none_or(|d| d.is_empty());
            json!({"bound": true, "status": c.status, "error_code": c.error_code, "level": c.level,
                "seeds": c.seeds, "primes": c.primes.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
                "requests": c.requests.iter().map(|q| json!({"label": q.label, "outcome": q.outcome.word(),
                    "certified": rep.certified.get(&q.label).copied().unwrap_or(false)})).collect::<Vec<_>>(),
                "findings": rep.findings.iter().map(|f| f.to_json()).collect::<Vec<_>>(),
                "rerun_diffs": rerun_diffs, "certified": certified})
        }
    }
}

/// `rr_campaign tadic-check --job J --result R --binary-sha256 X --driver-sha256 Y [--rerun R2]`.
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
    Ok(serde_json::to_string_pretty(&check_report(&job, &jb, &res, &b, &d, rerun.as_ref())).expect("json"))
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
}

/// Read a cell-tops TSV (columns by header name).
pub fn read_cell_tops(text: &str) -> Result<Vec<CellRow>, String> {
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().ok_or("empty cell table")?.split('\t').collect();
    let col = |n: &str| header.iter().position(|h| *h == n).ok_or(format!("cell table: no column {n}"));
    let (cn, ce, cd, cr, ct, ch, cb, ck, cu) = (
        col("node")?,
        col("E")?,
        col("D")?,
        col("r")?,
        col("top_r_at_D")?,
        col("top_r_at_D_if_n4hint")?,
        col("box_r_max")?,
        col("keys_in_cell")?,
        col("qcd_units_reaching")?,
    );
    let mut out = vec![];
    for (n, l) in lines.enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = l.split('\t').collect();
        let get = |i: usize| f.get(i).copied().unwrap_or("");
        let int = |i: usize| -> Result<i64, String> {
            get(i).trim().parse::<i64>().map_err(|e| format!("cell table line {}: column {i}: {e}", n + 2))
        };
        let upper = int(ct)?;
        let top = if get(ch).trim().is_empty() { upper } else { int(ch)? };
        out.push(CellRow {
            node: get(cn).to_string(),
            e: int(ce)?,
            d: int(cd)?,
            r: int(cr)?,
            top,
            top_upper: upper,
            box_r_max: int(cb)?,
            keys_in_cell: get(ck).trim().parse().map_err(|e| format!("keys_in_cell: {e}"))?,
            units_reaching: get(cu).to_string(),
        });
    }
    Ok(out)
}

/// The keys of cell (D, r): line slots at power 1 + dots (sum D, each <= `max_dots`), ISP slots at power <= 0 (sum -r).
pub fn cell_keys(arity: usize, lines: &[usize], isps: &[usize], d: i64, r: i64, max_dots: i64) -> Vec<Vec<i64>> {
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
        let b = std::fs::read(&self.path).map_err(|e| format!("{what} {}: {e}", self.path.display()))?;
        let h = sha256_hex(&b);
        if h != self.sha256 {
            return Err(format!("{what} {}: sha256 {h} is not the spec's {}", self.path.display(), self.sha256));
        }
        Ok(b)
    }
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
}

fn four() -> i64 {
    4
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
    #[serde(default)]
    pub r_max: Option<i64>,
    #[serde(default)]
    pub include_margin: bool,
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
    pub seeds: Vec<u64>,
    pub primes: u32,
    pub order: i64,
    pub combination_seed: u64,
    pub m_initial: usize,
    pub m_step: usize,
    pub m_max: usize,
    pub wall_seconds: u64,
    pub max_length: u64,
    pub threads: u32,
    pub memory_cap_bytes: u64,
    #[serde(default)]
    pub licence_env: Option<String>,
    /// Test stand: the fake child runs the jobs (program = rr_fake_child).
    #[serde(default)]
    pub fake_program: Option<PathBuf>,
}

/// The outcome of one cell.
#[derive(Clone, Debug, Serialize)]
pub struct CellOutcome {
    pub node: String,
    pub d: i64,
    pub r: i64,
    pub keys: usize,
    pub line: &'static str,
    pub outside_box: bool,
    pub units_reaching: String,
    pub combinations: usize,
    pub verdict: String,
    pub rank: Value,
    pub pivots: Value,
    pub rank_kind: String,
    pub cpu_seconds: f64,
    pub peak_gb: f64,
    pub max_plan_need: Value,
    pub max_series_length: Value,
    pub jobs: Vec<String>,
}

fn rank_of(spec: &PoleReportSpec, dir: &Path, results: &[PathBuf], tag: &str) -> Result<Value, String> {
    let out = dir.join(format!("rank.{tag}.json"));
    let mut cmd = std::process::Command::new(&spec.rank_bin.path);
    for r in results {
        cmd.arg("--result").arg(r);
    }
    cmd.arg("--depths")
        .arg(&spec.depths.path)
        .arg("--library")
        .arg(&spec.library.path)
        .arg("--catalogue")
        .arg(&spec.catalogue.path)
        .arg("--rank-prefix")
        .arg("c")
        .arg("--out")
        .arg(&out);
    if let Some(d) = &spec.dependences {
        cmd.arg("--dependences").arg(&d.path);
    }
    let o = cmd.output().map_err(|e| format!("rr_pole_rank: {e}"))?;
    if !o.status.success() {
        return Err(format!("rr_pole_rank exit {}: {}", o.status, String::from_utf8_lossy(&o.stderr).trim()));
    }
    read_json(&out.display().to_string()).map(|x| x.0)
}

/// `rr_campaign pole-report --spec S.json --dir D`: run (or resume) the report; prints the markdown table.
pub fn report_cmd(args: &[String]) -> Result<String, String> {
    let (Some(sp), Some(dir)) = (arg(args, "--spec"), arg(args, "--dir")) else {
        return Err("usage: pole-report --spec S.json --dir D".into());
    };
    let (sv, sb) = read_json(&sp)?;
    let spec: PoleReportSpec = serde_json::from_value(sv).map_err(|e| format!("spec: {e}"))?;
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
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(dir.join("cells")).map_err(|e| e.to_string())?;
    let rows = read_cell_tops(std::str::from_utf8(&cells_bytes).map_err(|e| e.to_string())?)?;
    let program = match &spec.fake_program {
        Some(p) => Program::from_file("rr_fake_child", p, crate::adapters::job::Invocation::Contract)
            .map_err(|e| e.to_string())?,
        None => Program::from_file("tadic_job", &spec.python, crate::adapters::job::Invocation::Legacy)
            .map_err(|e| e.to_string())?,
    };
    let driver = DataFile::from_file(&spec.driver.path).map_err(|e| e.to_string())?;
    let adapter = TadicAdapter {
        program,
        driver,
        package_binary: (spec.package_binary.path.clone(), spec.package_binary.sha256.clone()),
        node_binary: (spec.node_binary.path.clone(), spec.node_binary.sha256.clone()),
        licence_env: spec.licence_env.clone(),
        fake: spec.fake_program.as_ref().map(|_| crate::adapters::default_emulation_script()),
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
    for ns in &spec.nodes {
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
            .filter(|(c, _)| spec.d_max.is_none_or(|x| c.d <= x) && spec.r_max.is_none_or(|x| c.r <= x))
            .collect();
        cells.sort_by_key(|(c, _)| (c.keys_in_cell, c.d, c.r));
        for (c, line) in cells {
            let keys = cell_keys(ns.arity, &ns.lines, &ns.isps, c.d, c.r, ns.max_dots_per_line);
            let mut out = CellOutcome {
                node: ns.node.clone(),
                d: c.d,
                r: c.r,
                keys: keys.len(),
                line,
                outside_box: c.r > c.box_r_max,
                units_reaching: c.units_reaching.clone(),
                combinations: 0,
                verdict: String::new(),
                rank: Value::Null,
                pivots: Value::Null,
                rank_kind: String::new(),
                cpu_seconds: 0.0,
                peak_gb: 0.0,
                max_plan_need: Value::Null,
                max_series_length: Value::Null,
                jobs: vec![],
            };
            if keys.len() as u64 != c.keys_in_cell {
                out.verdict = format!(
                    "not decided (KEY_COUNT: enumerator {} keys, cell table {})",
                    keys.len(),
                    c.keys_in_cell
                );
                outcomes.push(out);
                continue;
            }
            let cdir = dir.join("cells").join(format!("{}_D{}r{}", ns.node, c.d, c.r));
            std::fs::create_dir_all(&cdir).map_err(|e| e.to_string())?;
            let mut results: Vec<PathBuf> = vec![];
            let mut m_done = 0usize;
            let mut m = spec.m_initial.max(1);
            loop {
                let reqs: Vec<TadicRequest> = (m_done..m)
                    .map(|ci| TadicRequest {
                        label: format!("c{ci}"),
                        inputs: keys
                            .iter()
                            .enumerate()
                            .map(|(ki, k)| TadicInput::Package {
                                top: ns.top.clone(),
                                powers: k.clone(),
                                weight: weight(spec.combination_seed, &ns.node, c.d, c.r, ci, ki),
                            })
                            .collect(),
                    })
                    .collect();
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
                    max_primes: 8,
                    seeds: spec.seeds.clone(),
                    limits: TadicLimits {
                        wall_seconds: spec.wall_seconds,
                        max_length: spec.max_length,
                        threads: spec.threads,
                        memory_gib: None,
                        no_step_cache: false,
                    },
                    crt: false,
                    terminals: Some(DataFile::from_file(&spec.library.path).map_err(|e| e.to_string())?),
                    dependences: deps.clone(),
                    depths: Some(depths.clone()),
                };
                let content = adapter.content_of(&jr);
                let stored = store.get(&adapter, &jr)?;
                if stored.is_none() {
                    let plans = adapter.plan(std::slice::from_ref(&jr)).map_err(|e| e.to_string())?;
                    let opts = RunOptions {
                        cpus: vec![],
                        memory_cap_bytes: Some(spec.memory_cap_bytes),
                        licence_env: spec.licence_env.clone(),
                        wall_cap: Some(Duration::from_secs(spec.wall_seconds + 600)),
                        grace: Some(Duration::from_secs(30)),
                    };
                    let mut attempt = 1u32;
                    while rustred_campaign_contract::layout::attempt_dir(&dir.join("work"), &plans[0].job_id, attempt).exists() {
                        attempt += 1;
                    }
                    let mut l = RealLauncher::new(sampler.clone());
                    let (b, rep) = run_attempt(&mut l, &adapter, std::slice::from_ref(&jr), &plans[0], &dir.join("work"), attempt, &opts)
                        .map_err(|e| e.to_string())?;
                    if let Some(r) = &rep.report {
                        out.cpu_seconds += r.cpu_seconds;
                        out.peak_gb = out.peak_gb.max(r.peak_rss_bytes as f64 / 1e9);
                    }
                    if b.result.is_none() {
                        out.verdict = format!("not decided (JOB: {})", b.unread.unwrap_or_default());
                        break;
                    }
                    store.put(&content, &rep.prepared)?;
                }
                let (checked, _, _) = store.get(&adapter, &jr)?.ok_or("stored result vanished")?;
                let cert = certificate::check(&checked);
                if let Some(f) = cert.findings.first() {
                    out.verdict = format!("not decided (NOT_CERTIFIED: {})", f.to_json());
                    break;
                }
                if let Some(q) = checked.requests.iter().find(|q| !q.outcome.is_decided()) {
                    out.verdict = format!("not decided ({}: {})", q.label, q.outcome.word());
                    break;
                }
                out.jobs.push(content.clone());
                results.push(store.root.join(&content).join(tadicjob::CHILD_OUT));
                m_done = m;
                out.combinations = m;
                let rank = rank_of(&spec, &cdir, &results, &format!("m{m}"))?;
                out.verdict = rank["verdict"].as_str().unwrap_or("").to_string();
                out.rank = rank["rank"].clone();
                out.pivots = rank["pivots"].clone();
                out.rank_kind = rank["rank_kind"].as_str().unwrap_or("").to_string();
                let mut mpn: Option<i64> = None;
                let mut msl: Option<i64> = None;
                for q in rank["requests"].as_array().into_iter().flatten() {
                    for c in q["replay"].as_array().into_iter().flatten() {
                        mpn = mpn.max(c["plan_max_need"].as_i64());
                        msl = msl.max(c["max_series_length"].as_i64());
                    }
                }
                out.max_plan_need = json!(mpn);
                out.max_series_length = json!(msl);
                if out.verdict.contains("NEEDS_MORE_COMBINATIONS") && m < spec.m_max {
                    m = (m + spec.m_step.max(1)).min(spec.m_max);
                    continue;
                }
                break;
            }
            let _ = std::fs::write(cdir.join("cell.json"), serde_json::to_vec_pretty(&out).expect("json"));
            outcomes.push(out);
        }
    }
    let upper = spec.dependences.is_none();
    let mut nodes = BTreeMap::new();
    for ns in &spec.nodes {
        let mine: Vec<&CellOutcome> = outcomes.iter().filter(|o| o.node == ns.node && o.line == "main").collect();
        let all_zero = !mine.is_empty() && mine.iter().all(|o| o.verdict == "count zero: yes");
        let any_no = mine.iter().any(|o| o.verdict.starts_with("count zero: no"));
        let verdict = if all_zero {
            "count zero on all cells up to top(D): yes".to_string()
        } else if any_no && !upper {
            "count zero on all cells up to top(D): no".to_string()
        } else if any_no {
            "count zero on all cells up to top(D): not decided (naive upper bound > 0: no dependence table)".to_string()
        } else {
            "count zero on all cells up to top(D): not decided (cells not decided or not run)".to_string()
        };
        nodes.insert(ns.node.clone(), verdict);
    }
    let report = json!({"schema": REPORT_SCHEMA, "spec_sha256": sha256_hex(&sb), "cells_sha256": spec.cells.sha256,
        "depths_sha256": spec.depths.sha256, "dependences_sha256": spec.dependences.as_ref().map(|d| &d.sha256),
        "score": {"level": "composed", "form": "rank", "claim": if upper {"objective_upper_bound"} else {"objective"}},
        "cell_definition": "cells (D, r) with r <= top(D); top(D) = top_r_at_D_if_n4hint of the cell table (top_r_at_D where empty), not clipped by box_r_max; margin = cells between the hint and top_r_at_D (TRICKS 45, coordinator ruling 2026-11-17)",
        "nodes": nodes, "cells": outcomes});
    let tmp = dir.join("pole_report.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&report).expect("json")).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dir.join("pole_report.json")).map_err(|e| e.to_string())?;
    let md = markdown(&report);
    std::fs::write(dir.join("pole_report.md"), &md).map_err(|e| e.to_string())?;
    Ok(md)
}

/// The markdown table of a report.
pub fn markdown(report: &Value) -> String {
    let mut s = String::from("# Pole report (rank of rho per cell)\n\n");
    s.push_str(&format!(
        "Score: level `composed`, form `rank`, claim `{}`. Cell definition: {}.\n\n",
        report["score"]["claim"].as_str().unwrap_or(""),
        report["cell_definition"].as_str().unwrap_or("")
    ));
    for (n, v) in report["nodes"].as_object().into_iter().flatten() {
        s.push_str(&format!("- **{n}**: {}\n", v.as_str().unwrap_or("")));
    }
    s.push_str("\n| node | cell | line | keys | comb. | rank | surviving directions | verdict | CPU-s | peak GB | max need | max series |\n|---|---|---|---|---|---|---|---|---|---|---|---|\n");
    for c in report["cells"].as_array().into_iter().flatten() {
        let mut line = c["line"].as_str().unwrap_or("").to_string();
        if c["outside_box"] == true {
            line = format!("{line}; outside BOXDEF v3, reached by {} units", c["units_reaching"].as_str().unwrap_or("?"));
        }
        s.push_str(&format!(
            "| {} | D{}r{} | {} | {} | {} | {} | {} | {} | {:.0} | {:.1} | {} | {} |\n",
            c["node"].as_str().unwrap_or(""),
            c["d"],
            c["r"],
            line,
            c["keys"],
            c["combinations"],
            c["rank"],
            c["pivots"],
            c["verdict"].as_str().unwrap_or(""),
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
    fn weights_are_reproducible_and_in_range() {
        let w = weight(7, "N", 1, 2, 3, 4);
        assert_eq!(w, weight(7, "N", 1, 2, 3, 4));
        assert!(w >= 1 && w < (1i64 << 62));
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
    }
}
