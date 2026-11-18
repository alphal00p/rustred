//! `rr_campaign bind-check` (lane gencheck, item A of round 2): RE-BIND independent checker outputs to a SHIPPED table, at
//! each rule's own position (TRICKS 67: a class binds to the record at its position, never by hash from elsewhere).
//! One streaming pass over the table: its sha256, the file position (among the rule records of the chart) and the byte
//! offset of every rule record, and its body identity (relation / domain sha256, as the checker computes them). For every
//! checker output: the record at the output's offset must be a rule record whose index == its file position == the
//! output's index, whose bytes have the output's record sha256, whose relation and domain are the output's; the
//! certificate file must have the output's certificate sha256. Only then may the output's class be the rule's class.
//! Writes per checked rule one JSON line (and a TSV), and a CLASS MANIFEST of every rule of the table (the class from a
//! given class table, replaced for a re-bound rule by the class its check gives).
use super::child::file_sha256;
use rustred_campaign_core::closure::sha256::Sha256;
use rustred_campaign_core::emission::body::{body_ids, sha256_hex, string_member};
use rustred_campaign_core::emission::class::CheckSummary;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::PathBuf;

/// One checked rule: a label, the checker output file, the certificate file.
#[derive(Clone, Debug)]
pub struct Item {
    pub label: String,
    pub check: PathBuf,
    pub cert: PathBuf,
}

struct Rec {
    position: u64,
    index: i64,
    chart: String,
    record_sha256: String,
    relation: String,
    domain: String,
    body: String,
}

/// The class word a re-bound check gives (contract words) and the reason when it is not whole domain.
fn class_from(s: &CheckSummary) -> (String, Option<String>) {
    let base = s.binding_equal() && s.identity_exact() && s.rows_verified();
    if base && s.checker_class == "WHOLE_DOMAIN" && s.open_count() == 0 {
        (
            "whole domain".into(),
            (s.rows != "ROWS_VERIFIED_BY_RECOVERY" && s.rows != "ROWS_VERIFIED")
                .then(|| format!("rows {}", s.rows)),
        )
    } else {
        (
            "none".into(),
            Some(rustred_campaign_core::emission::class::check_reason(s)),
        )
    }
}

/// Run the re-binding. `classes`: an optional class table (JSON lines with `table_index`, `exactness_class`,
/// `target_rhs_sha256`, `domain_sha256`) giving the class BEFORE.
pub fn bind_check(
    table: &std::path::Path,
    items: &[Item],
    classes: Option<&std::path::Path>,
) -> Result<(Value, Vec<Value>, Vec<Value>), String> {
    // the checker outputs first: which offsets to look at
    let mut outs: Vec<(Item, Value, String)> = vec![];
    for it in items {
        let t = std::fs::read_to_string(&it.check)
            .map_err(|e| format!("{}: {e}", it.check.display()))?;
        let line = t
            .lines()
            .rev()
            .find(|l| l.trim_start().starts_with('{'))
            .ok_or_else(|| format!("{}: no output line", it.check.display()))?
            .to_string();
        let v: Value =
            serde_json::from_str(&line).map_err(|e| format!("{}: {e}", it.check.display()))?;
        outs.push((it.clone(), v, line));
    }
    let want: BTreeMap<u64, usize> = outs
        .iter()
        .enumerate()
        .filter_map(|(k, (_, v, _))| v["rule"]["offset"].as_u64().map(|o| (o, k)))
        .collect();
    let before: BTreeMap<i64, Value> = match classes {
        Some(p) => std::fs::read_to_string(p)
            .map_err(|e| format!("{}: {e}", p.display()))?
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .filter_map(|v| v["table_index"].as_i64().map(|i| (i, v)))
            .collect(),
        None => BTreeMap::new(),
    };
    // one pass over the table
    let f = std::fs::File::open(table).map_err(|e| format!("{}: {e}", table.display()))?;
    let mut r = std::io::BufReader::with_capacity(1 << 22, f);
    let mut whole = Sha256::new();
    let mut offset = 0u64;
    let mut pos: BTreeMap<String, u64> = BTreeMap::new();
    let mut at: BTreeMap<u64, Rec> = BTreeMap::new();
    let mut all: Vec<Rec> = vec![];
    let mut line = String::new();
    loop {
        line.clear();
        let n = r.read_line(&mut line).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        whole.update(line.as_bytes());
        let l = line.trim_end_matches('\n');
        if string_member(l, "type").as_deref() == Some("rule") {
            let chart = string_member(l, "chart").unwrap_or_default();
            let p = pos.entry(chart.clone()).or_insert(0);
            let ids = body_ids(l)?;
            let index = rustred_campaign_core::emission::body::members(l)?
                .iter()
                .find(|m| m.key == "index")
                .and_then(|m| l[m.value.clone()].trim().parse::<i64>().ok())
                .unwrap_or(-1);
            let rec = Rec {
                position: *p,
                index,
                chart,
                record_sha256: if want.contains_key(&offset) {
                    sha256_hex(l.as_bytes())
                } else {
                    String::new()
                },
                relation: ids.relation_sha256,
                domain: ids.domain_sha256,
                body: ids.body_sha256,
            };
            *p += 1;
            if want.contains_key(&offset) {
                at.insert(
                    offset,
                    Rec {
                        position: rec.position,
                        index: rec.index,
                        chart: rec.chart.clone(),
                        record_sha256: rec.record_sha256.clone(),
                        relation: rec.relation.clone(),
                        domain: rec.domain.clone(),
                        body: rec.body.clone(),
                    },
                );
            }
            all.push(rec);
        }
        offset += n as u64;
    }
    let table_sha = whole.finish_hex();
    // the checked rules
    let mut rows = vec![];
    let mut now: BTreeMap<i64, (String, Value)> = BTreeMap::new();
    for (it, v, raw) in &outs {
        let mut problems: Vec<String> = vec![];
        let off = v["rule"]["offset"].as_u64();
        let rec = off.and_then(|o| at.get(&o));
        let idx = v["rule"]["index"].as_i64().unwrap_or(-1);
        match rec {
            None => problems.push(format!(
                "offset {off:?} is not the start of a rule record of the table"
            )),
            Some(rc) => {
                if rc.index != idx || rc.position as i64 != idx {
                    problems.push(format!(
                        "record at the offset: index {} position {} (check index {idx})",
                        rc.index, rc.position
                    ));
                }
                if v["rule"]["record_sha256"].as_str() != Some(rc.record_sha256.as_str()) {
                    problems.push("the checker read other bytes than the table's record".into());
                }
                if v["rule"]["target_rhs_sha256"].as_str() != Some(rc.relation.as_str()) {
                    problems.push("relation sha256 differs".into());
                }
                if v["rule"]["domain_sha256"].as_str() != Some(rc.domain.as_str()) {
                    problems.push("domain sha256 differs".into());
                }
            }
        }
        let cert_sha = file_sha256(&it.cert).unwrap_or_default();
        if v["cert"]["sha256"].as_str() != Some(cert_sha.as_str()) {
            problems.push("certificate file sha256 differs from the checker's".into());
        }
        let summary = CheckSummary::parse(raw);
        let (class, reason) = match (&summary, problems.is_empty()) {
            (Ok(s), true) => class_from(s),
            (Ok(_), false) => (
                "none".into(),
                Some(format!("binding: {}", problems.join("; "))),
            ),
            (Err(e), _) => ("none".into(), Some(e.clone())),
        };
        let b = before.get(&idx);
        let row = json!({"label": it.label, "table_index": idx, "position": rec.map(|r| r.position),
            "offset": off, "chart": rec.map(|r| r.chart.clone()),
            "target_rhs_sha256": v["rule"]["target_rhs_sha256"], "domain_sha256": v["rule"]["domain_sha256"],
            "record_sha256": v["rule"]["record_sha256"], "body_sha256": rec.map(|r| r.body.clone()),
            "class_before": b.map(|b| b["exactness_class"].clone()),
            "class_table_hashes_equal": b.map(|b| b["target_rhs_sha256"] == v["rule"]["target_rhs_sha256"] && b["domain_sha256"] == v["rule"]["domain_sha256"]),
            "class_now": class, "reason": reason, "binding_problems": problems,
            "certificate": {"file": it.cert.display().to_string(), "sha256": cert_sha, "bytes": std::fs::metadata(&it.cert).map(|m| m.len()).ok(), "form": v["cert"]["form"]},
            "check_output": {"file": it.check.display().to_string(), "sha256": sha256_hex(raw.as_bytes())},
            "stages": {"rows": v["rows"]["verdict"], "identity": v["identity"]["verdict"], "binding": v["binding"]["verdict"],
                       "factors": v["factors"]["producer"], "loci": v["loci_counts"]},
            "recovery": v["rows"]["recovery"], "checker_class": v["class"],
            "seconds_total": v["seconds_total"], "peak_mb": v["peak_mb"]});
        if !class.is_empty() {
            now.insert(idx, (class.clone(), row.clone()));
        }
        rows.push(row);
    }
    // the class manifest of every rule
    let mut manifest = vec![];
    for rc in &all {
        let b = before.get(&rc.index);
        let (class, cert, out) = match now.get(&rc.index) {
            Some((c, row)) if c == "whole domain" => (
                json!(c),
                row["certificate"]["sha256"].clone(),
                row["check_output"]["sha256"].clone(),
            ),
            _ => (
                b.map(|b| b["exactness_class"].clone())
                    .unwrap_or(Value::Null),
                Value::Null,
                Value::Null,
            ),
        };
        manifest.push(json!({"chart": rc.chart, "position": rc.position, "index": rc.index, "body_sha256": rc.body,
            "target_rhs_sha256": rc.relation, "domain_sha256": rc.domain, "class": class,
            "class_source": if now.get(&rc.index).is_some_and(|(c, _)| c == "whole domain") { "this addendum: weight certificate re-checked by row recovery, re-bound here" } else { "the delivery's class table" },
            "class_table_hashes_equal": b.map(|b| b["target_rhs_sha256"] == json!(rc.relation) && b["domain_sha256"] == json!(rc.domain)),
            "certificate_sha256": cert, "check_output_sha256": out}));
    }
    let summary = json!({"schema": "rustred.campaign.bind-check.v1", "table": table.display().to_string(), "table_sha256": table_sha,
        "rule_records": all.len(), "checked": rows.len(),
        "rebound_whole_domain": rows.iter().filter(|r| r["class_now"] == "whole domain").count(),
        "binding_failures": rows.iter().filter(|r| r["binding_problems"].as_array().is_some_and(|a| !a.is_empty())).count(),
        "class_table_mismatches": manifest.iter().filter(|m| m["class_table_hashes_equal"] == json!(false)).count()});
    Ok((summary, rows, manifest))
}

/// CLI: `bind-check --table T --items LIST.tsv (label \t check output \t certificate) [--classes CLASSES.jsonl]
/// --out RULES.jsonl [--tsv RULES.tsv] [--manifest MANIFEST.jsonl]`.
pub fn cmd(rest: &[String]) -> Result<(String, bool), String> {
    let mut a: BTreeMap<String, String> = BTreeMap::new();
    let mut i = 0;
    while i < rest.len() {
        let k = rest[i].trim_start_matches("--").to_string();
        let v = rest
            .get(i + 1)
            .cloned()
            .ok_or(format!("{} needs a value", rest[i]))?;
        a.insert(k, v);
        i += 2;
    }
    let table = PathBuf::from(a.get("table").ok_or("--table")?);
    let list =
        std::fs::read_to_string(a.get("items").ok_or("--items")?).map_err(|e| e.to_string())?;
    let items: Vec<Item> = list
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            Item {
                label: f[0].to_string(),
                check: PathBuf::from(f.get(1).copied().unwrap_or_default()),
                cert: PathBuf::from(f.get(2).copied().unwrap_or_default()),
            }
        })
        .collect();
    let (summary, rows, manifest) = bind_check(
        &table,
        &items,
        a.get("classes").map(PathBuf::from).as_deref(),
    )?;
    let write = |p: &str, vs: &[Value]| -> Result<(), String> {
        let mut f =
            std::io::BufWriter::new(std::fs::File::create(p).map_err(|e| format!("{p}: {e}"))?);
        for v in vs {
            writeln!(f, "{v}").map_err(|e| e.to_string())?;
        }
        f.flush().map_err(|e| e.to_string())
    };
    write(a.get("out").ok_or("--out")?, &rows)?;
    if let Some(p) = a.get("manifest") {
        write(p, &manifest)?;
    }
    if let Some(p) = a.get("tsv") {
        let mut f =
            std::io::BufWriter::new(std::fs::File::create(p).map_err(|e| format!("{p}: {e}"))?);
        writeln!(f, "table_index\tposition\ttarget_rhs_sha256\tdomain_sha256\tclass_before\tclass_now\treason\tcertificate_file\tcertificate_sha256\tcheck_output_file\tcheck_output_sha256\trows\tidentity\tbinding\tfactors\tloci\tseconds\tpeak_mb").map_err(|e| e.to_string())?;
        let s = |v: &Value| match v {
            Value::String(t) => t.clone(),
            Value::Null => "-".into(),
            o => o.to_string(),
        };
        for r in &rows {
            writeln!(
                f,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                s(&r["table_index"]),
                s(&r["position"]),
                s(&r["target_rhs_sha256"]),
                s(&r["domain_sha256"]),
                s(&r["class_before"]),
                s(&r["class_now"]),
                s(&r["reason"]),
                s(&r["certificate"]["file"]),
                s(&r["certificate"]["sha256"]),
                s(&r["check_output"]["file"]),
                s(&r["check_output"]["sha256"]),
                s(&r["stages"]["rows"]),
                s(&r["stages"]["identity"]),
                s(&r["stages"]["binding"]),
                s(&r["stages"]["factors"]),
                s(&r["stages"]["loci"]),
                s(&r["seconds_total"]),
                s(&r["peak_mb"])
            )
            .map_err(|e| e.to_string())?;
        }
        f.flush().map_err(|e| e.to_string())?;
    }
    let ok = summary["binding_failures"] == json!(0);
    Ok((summary.to_string(), ok))
}
