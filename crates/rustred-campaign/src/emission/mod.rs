//! The EMISSION path (lane gencheck; plan/auto-campaign/interfaces/emission-check.md): the process part of the
//! contract whose pure part is `rustred_campaign_core::emission`.
//! - [`child`]: one child process with a wall budget and its own peak RSS;
//! - [`emitter`]: the emitter (check on the bytes being written, class, policy, derivation member, side files);
//! - [`replace`]: certify by replacement (lift, emit, ladder over open loci and new branches, partition proof);
//! - [`tablecheck`]: `rr_campaign table-check`.
//!
//! CLI: `rr_campaign emit-replace ...` ([`emit_replace_cmd`]) and `rr_campaign table-check ...` ([`tablecheck::cmd`]).
pub mod child;
pub mod emitter;
pub mod replace;
pub mod tablecheck;

use emitter::{Checker, Emitter};
use replace::{LadderConfig, Outcome, RrNodeWitness, certify_by_replacement};
use rustred_campaign_core::economy::improve::{restate_marker, set_index};
use rustred_campaign_core::emission::body::string_member;
use rustred_campaign_core::emission::class::Policy;
use rustred_campaign_core::emission::side_dir;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufRead, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// The header line, the chart lines and the rule record with `index` (at `offset` when given) of a table.
pub fn read_record(
    table: &Path,
    index: i64,
    offset: Option<u64>,
) -> Result<(String, Vec<String>, String), String> {
    let f = std::fs::File::open(table).map_err(|e| format!("{}: {e}", table.display()))?;
    let mut r = std::io::BufReader::new(f);
    let mut header = String::new();
    r.read_line(&mut header).map_err(|e| e.to_string())?;
    let header = header.trim_end().to_string();
    let mut charts = vec![];
    let is_rule = |l: &str| string_member(l.trim_end(), "type").as_deref() == Some("rule");
    let has_index = |l: &str| {
        serde_json::from_str::<Value>(l)
            .ok()
            .is_some_and(|v| v["index"].as_i64() == Some(index))
    };
    if let Some(o) = offset {
        // charts before the first rule, then the line at the offset
        let mut line = String::new();
        loop {
            line.clear();
            if r.read_line(&mut line).map_err(|e| e.to_string())? == 0 || is_rule(&line) {
                break;
            }
            if string_member(line.trim_end(), "type").as_deref() == Some("chart") {
                charts.push(line.trim_end().to_string());
            }
        }
        let mut f = std::fs::File::open(table).map_err(|e| e.to_string())?;
        f.seek(SeekFrom::Start(o)).map_err(|e| e.to_string())?;
        let mut rr = std::io::BufReader::new(f);
        let mut l = String::new();
        rr.read_line(&mut l).map_err(|e| e.to_string())?;
        let l = l.trim_end().to_string();
        if !is_rule(&l) || !has_index(&l) {
            return Err(format!(
                "offset {o} is not the rule record with index {index}"
            ));
        }
        return Ok((header, charts, l));
    }
    let mut line = String::new();
    loop {
        line.clear();
        if r.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            return Err(format!(
                "{}: no rule record with index {index}",
                table.display()
            ));
        }
        if is_rule(&line) {
            let l = line.trim_end().to_string();
            if has_index(&l) {
                return Ok((header, charts, l));
            }
        } else if string_member(line.trim_end(), "type").as_deref() == Some("chart") {
            charts.push(line.trim_end().to_string());
        }
    }
}

/// Stream `table` into `out`, the rule with `index` replaced by `pieces` (positioned), every later rule of its chart
/// renumbered (only its index changes), completion markers restated.
pub fn write_replaced_table(
    table: &Path,
    out: &Path,
    index: i64,
    chart: &str,
    pieces: &[String],
) -> Result<(String, u64), String> {
    // rule counts per chart (for the markers)
    let mut per_chart: BTreeMap<String, u64> = BTreeMap::new();
    let rule_chart = |l: &str| -> Option<String> {
        if string_member(l, "type").as_deref() != Some("rule") {
            return None;
        }
        Some(string_member(l, "chart").unwrap_or_default())
    };
    for l in std::io::BufReader::new(std::fs::File::open(table).map_err(|e| e.to_string())?).lines()
    {
        let l = l.map_err(|e| e.to_string())?;
        if let Some(c) = rule_chart(&l) {
            *per_chart.entry(c).or_default() += 1;
        }
    }
    *per_chart.entry(chart.to_string()).or_default() += pieces.len() as u64 - 1;
    let total: u64 = per_chart.values().sum();
    let tmp = out.with_extension("jsonl.tmp");
    let mut w = std::io::BufWriter::new(std::fs::File::create(&tmp).map_err(|e| e.to_string())?);
    let mut sha = rustred_campaign_core::closure::sha256::Sha256::new();
    let mut put = |w: &mut std::io::BufWriter<std::fs::File>, s: &str| -> Result<(), String> {
        sha.update(s.as_bytes());
        sha.update(b"\n");
        w.write_all(s.as_bytes()).map_err(|e| e.to_string())?;
        w.write_all(b"\n").map_err(|e| e.to_string())
    };
    let mut pos: BTreeMap<String, i64> = BTreeMap::new();
    let mut replaced = false;
    for l in std::io::BufReader::new(std::fs::File::open(table).map_err(|e| e.to_string())?).lines()
    {
        let l = l.map_err(|e| e.to_string())?;
        match rule_chart(&l) {
            Some(c) => {
                let p = pos.entry(c.clone()).or_insert(0);
                let is_target = c == chart
                    && !replaced
                    && serde_json::from_str::<Value>(&l[..])
                        .ok()
                        .is_some_and(|v| v["index"].as_i64() == Some(index));
                if is_target {
                    for s in pieces {
                        put(&mut w, &set_index(s, *p)?)?;
                        *p += 1;
                    }
                    replaced = true;
                } else {
                    put(&mut w, &set_index(&l, *p)?)?;
                    *p += 1;
                }
            }
            None => put(&mut w, &restate_marker(&l, total, &per_chart))?,
        }
    }
    w.flush().map_err(|e| e.to_string())?;
    drop(w);
    if !replaced {
        return Err(format!("no rule with index {index} in chart {chart}"));
    }
    std::fs::rename(&tmp, out).map_err(|e| e.to_string())?;
    Ok((
        sha.finish_hex(),
        std::fs::metadata(out).map(|m| m.len()).unwrap_or(0),
    ))
}

fn flag_value(rest: &[String], i: usize) -> Result<String, String> {
    rest.get(i + 1)
        .cloned()
        .ok_or_else(|| format!("{} needs a value", rest[i]))
}

/// `rr_campaign emit-replace --table T --rule N [--offset B] --rr-node BIN --checker BIN --out DIR [--max-depth 6]
/// [--max-pieces 16] [--case-budget S] [--cone S] [--policy FILE] [--family F] [--no-recover-rows] [--write-table]
/// [--witness-off]`: certify rule N of T by replacement; writes DIR/report.json, DIR/cand.emission/ (side files) and,
/// when the ladder closed, DIR/pieces.jsonl, DIR/pieces.mini.table.jsonl and (with --write-table) DIR/cand.table.jsonl.
pub fn emit_replace_cmd(rest: &[String]) -> Result<(String, bool), String> {
    let (mut table, mut out, mut rr_node, mut checker) = (None, None, None, None);
    let mut index: Option<i64> = None;
    let mut offset = None;
    let mut cfg = LadderConfig::default();
    let mut case_budget = 7200u64;
    let mut policy = Policy::default();
    let mut family = None;
    let mut recover = true;
    let mut write_table = false;
    let mut i = 0;
    while i < rest.len() {
        let mut one = false;
        match rest[i].as_str() {
            "--table" => table = Some(PathBuf::from(flag_value(rest, i)?)),
            "--rule" => {
                index = Some(
                    flag_value(rest, i)?
                        .parse()
                        .map_err(|_| "--rule: an index")?,
                )
            }
            "--offset" => {
                offset = Some(
                    flag_value(rest, i)?
                        .parse()
                        .map_err(|_| "--offset: bytes")?,
                )
            }
            "--rr-node" => rr_node = Some(PathBuf::from(flag_value(rest, i)?)),
            "--checker" => checker = Some(PathBuf::from(flag_value(rest, i)?)),
            "--out" => out = Some(PathBuf::from(flag_value(rest, i)?)),
            "--max-depth" => {
                cfg.max_depth = flag_value(rest, i)?.parse().map_err(|_| "--max-depth")?
            }
            "--max-pieces" => {
                cfg.max_pieces = flag_value(rest, i)?.parse().map_err(|_| "--max-pieces")?
            }
            "--case-budget" => {
                case_budget = flag_value(rest, i)?.parse().map_err(|_| "--case-budget")?
            }
            "--cone" => {
                cfg.cone_s = Some(
                    flag_value(rest, i)?
                        .parse()
                        .map_err(|_| "--cone: the simplex bound")?,
                )
            }
            "--family" => family = Some(PathBuf::from(flag_value(rest, i)?)),
            "--policy" => {
                let p = flag_value(rest, i)?;
                let t = std::fs::read_to_string(&p).map_err(|e| format!("{p}: {e}"))?;
                policy = serde_json::from_str(&t).map_err(|e| format!("{p}: {e}"))?;
            }
            "--no-recover-rows" => {
                recover = false;
                one = true;
            }
            "--write-table" => {
                write_table = true;
                one = true;
            }
            "--witness-off" => {
                cfg.witness_modes = vec![false];
                one = true;
            }
            o => return Err(format!("unknown argument {o}")),
        }
        i += if one { 1 } else { 2 };
    }
    let table = table.ok_or("--table is required")?;
    let index = index.ok_or("--rule is required")?;
    let out = out.ok_or("--out is required")?;
    let rr_node = rr_node.ok_or("--rr-node is required")?;
    let checker = checker.ok_or("--checker is required")?;
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let t0 = std::time::Instant::now();
    let (header, charts, line) = read_record(&table, index, offset)?;
    let original: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
    let hv: Value = serde_json::from_str(&header).map_err(|e| format!("header: {e}"))?;
    cfg.is_line = hv["mask"]
        .as_str()
        .unwrap_or_default()
        .chars()
        .map(|c| c == '1')
        .collect();
    let mut ck = Checker::new(&checker)?;
    ck.recover_rows = recover;
    ck.family = family;
    let cand = out.join("cand.table.jsonl");
    let side = side_dir(&cand);
    let emitter = Emitter::new(ck, policy, &side, &out.join("work"))?;
    let mut lifter = RrNodeWitness::new(&rr_node)?;
    lifter.case_budget_s = case_budget;
    let rep = certify_by_replacement(
        &header,
        &charts,
        &original,
        index,
        &lifter,
        &emitter,
        &cfg,
        &out.join("work"),
    )?;
    let closed = rep.outcome == Outcome::Closed;
    let orig_rhs = original["rhs"].as_array().map_or(0, Vec::len);
    let mut summary = json!({"schema": "rustred.campaign.emit-replace.v1", "table": table.display().to_string(), "rule": index,
        "outcome": rep.outcome.json(), "original": {"bytes": line.len(), "rhs_terms": orig_rhs,
            "lift_seconds": original["lift_seconds"], "trace_rows": original["trace_rows"], "monomials": original["monomials"]},
        "pieces": rep.pieces.iter().map(|p| json!({"tag": p.tag, "depth": p.depth, "class": p.emission.classed.class.word(),
            "rhs_terms": p.rhs_terms, "bytes": p.bytes, "lift_seconds": p.lift_seconds, "lift_peak_mb": p.lift_peak_mb,
            "check_seconds": p.check_seconds, "check_peak_mb": p.check_peak_mb, "certs_tried": p.certs_tried,
            "stages": p.emission.summary.as_ref().map(|s| s.record_json()), "body_sha256": p.emission.ids.body_sha256,
            "fixed": p.record["fixed"], "exceptions": p.record["exceptions"]})).collect::<Vec<_>>(),
        "pieces_total": {"bytes": rep.pieces.iter().map(|p| p.bytes).sum::<usize>(), "rhs_terms": rep.pieces.iter().map(|p| p.rhs_terms).sum::<usize>()},
        "partition": rep.partition.as_ref().map(|(ok, checks)| json!({"partition": ok, "checks": checks.iter().map(|c| json!({"check": c.check, "witness": c.witness, "verdict": c.verdict, "why": c.why})).collect::<Vec<_>>()})),
        "log": rep.log, "seconds": t0.elapsed().as_secs_f64(),
        "checker": {"binary": checker.display().to_string(), "sha256": emitter.checker.sha256, "recover_rows": recover},
        "producer": {"binary": rr_node.display().to_string(), "sha256": lifter.sha256}});
    if closed {
        let lines = rep.lines(index)?;
        let mut pj = String::new();
        for l in &lines {
            pj.push_str(l);
            pj.push('\n');
        }
        std::fs::write(out.join("pieces.jsonl"), &pj).map_err(|e| e.to_string())?;
        let mut mini = format!("{header}\n");
        for c in &charts {
            mini.push_str(c);
            mini.push('\n');
        }
        mini.push_str(&pj);
        std::fs::write(out.join("pieces.mini.table.jsonl"), mini).map_err(|e| e.to_string())?;
        if write_table {
            let chart = original["chart"].as_str().unwrap_or_default().to_string();
            let (sha, bytes) = write_replaced_table(&table, &cand, index, &chart, &lines)?;
            summary["candidate_table"] =
                json!({"path": cand.display().to_string(), "sha256": sha, "bytes": bytes});
        }
    }
    let text = serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?;
    std::fs::write(out.join("report.json"), &text).map_err(|e| e.to_string())?;
    let mut brief = summary.clone();
    if let Some(o) = brief.as_object_mut() {
        o.remove("log");
        o.remove("partition");
    }
    Ok((brief.to_string(), closed))
}
