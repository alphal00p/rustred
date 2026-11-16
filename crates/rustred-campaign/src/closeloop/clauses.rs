//! Lane t266run (interfaces/closeloop.md 17.4): a box made of several CLAUSES (e.g. a proved box and an added layer that
//! is derived but not yet seen in a run). The node is closed on the UNION (the import spec's start set); at the final
//! pass every clause is checked on its own (the same tables, relations, evidence and terminals; its own start set),
//! and every key of the union closure is attributed to the FIRST clause (in configuration order) whose own closure
//! contains it ("the clause that brought it in"). Outputs: per clause verdict and counts (keys, certified, open,
//! terminals, rule class x level), the attribution of every key outside the first clause
//! (`statement/clause-keys.jsonl`; every other statement row belongs to the first clause) and the clause of every
//! open key. Memory: one closure at a time (each clause's key set is spilled to a key-set file before the next).

use super::config::ClauseCfg;
use rustred_campaign_core::closure::{KeySet, Tier};
use rustred_campaign_core::import::load::read_start;
use rustred_campaign_core::import::{CheckOptions, Evidence, ImportedSet, Level, TableSetSpec};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::Path;

/// Per-clause checks + attribution. `union_rows` = the union check's rows file (canonical statement lines);
/// `certify(row line value) -> (class, level word, certified)` scores one row exactly as the node's verdict does.
#[allow(clippy::too_many_arguments)]
pub fn per_clause(
    clauses: &[ClauseCfg],
    spec_text: &str,
    base: &Path,
    set: &ImportedSet,
    ev: &Evidence,
    tier: Tier,
    threads: usize,
    required: Level,
    basis_ok: bool,
    union_rows: &Path,
    dir: &Path,
    statement_dir: &Path,
) -> Result<Value, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut summaries = vec![];
    let mut ks_files = vec![];
    for c in clauses {
        let mut spec =
            TableSetSpec::from_json(spec_text).map_err(|e| format!("import spec: {e}"))?;
        spec.start = serde_json::from_value(c.start.clone())
            .map_err(|e| format!("clause {}: start: {e}", c.id))?;
        let ks = read_start(&spec, base, set.codec).map_err(|e| format!("clause {}: {e}", c.id))?;
        let rows = dir.join(format!("{}.rows.jsonl", c.id));
        let res = rustred_campaign_core::import::check::check_streamed_from(
            set,
            ev,
            &CheckOptions {
                threads,
                tier: Some(tier),
                max_rounds: None,
            },
            &rows,
            &ks,
        )
        .map_err(|e| format!("clause {} check: {e}", c.id))?;
        let keys = res
            .streamed
            .as_ref()
            .map(|s| s.rows)
            .unwrap_or(res.rows.len() as u64);
        let kept = res.rows.len() as u64;
        let kept_cert = res
            .rows
            .iter()
            .filter(|r| super::pass::row_certified(r, required, basis_ok))
            .count() as u64;
        let plain_cert = if basis_ok && required <= res.required {
            keys - kept
        } else {
            0
        };
        let certified = kept_cert + plain_cert;
        let counts = res.counts();
        let open = counts.get("status:open").copied().unwrap_or(0);
        let terminal = counts.get("status:terminal").copied().unwrap_or(0);
        let mut blocking = res.blocking();
        blocking.retain(|b| !b.starts_with("below_required_evidence:") || required == res.required);
        let verdict = if certified == keys && open == 0 {
            "closed"
        } else {
            "open"
        };
        let ksp = dir.join(format!("{}.keyset", c.id));
        if let Some(k) = &res.key_set {
            let f = std::fs::File::create(&ksp).map_err(|e| format!("{}: {e}", ksp.display()))?;
            k.write(std::io::BufWriter::new(f))
                .map_err(|e| format!("{}: {e}", ksp.display()))?;
        } else {
            return Err(format!(
                "clause {}: the streamed check kept no key set",
                c.id
            ));
        }
        ks_files.push(ksp);
        summaries.push(
            json!({"id": c.id, "start_keys": ks.len(), "closure_keys": keys, "certified": certified,
            "uncertified": keys - certified, "open": open, "terminal": terminal, "verdict": verdict,
            "label": format!("{}: {}", c.id, if verdict == "closed" { "closed".to_string() } else {
                format!("open ({})", keys - certified) }),
            "closure_verdict": format!("{:?}", res.summary.verdict), "blocking": blocking,
            "rows": rows.display().to_string()}),
        );
        drop(res);
    }
    // attribution: the first clause whose own closure contains the key
    let mut sets: Vec<KeySet> = vec![];
    for p in &ks_files {
        let f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
        sets.push(
            KeySet::read(std::io::BufReader::new(f))
                .map_err(|e| format!("{}: {e:?}", p.display()))?,
        );
    }
    let mut by: Vec<BTreeMap<String, BTreeMap<String, u64>>> = vec![BTreeMap::new(); clauses.len()];
    let mut n_by = vec![0u64; clauses.len()];
    let mut outside = 0u64;
    let ck = statement_dir.join("clause-keys.jsonl");
    let mut w = std::io::BufWriter::new(
        std::fs::File::create(&ck).map_err(|e| format!("{}: {e}", ck.display()))?,
    );
    let f =
        std::fs::File::open(union_rows).map_err(|e| format!("{}: {e}", union_rows.display()))?;
    for l in std::io::BufReader::with_capacity(1 << 22, f).lines() {
        let l = l.map_err(|e| e.to_string())?;
        if l.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(&l).map_err(|e| e.to_string())?;
        let key: Vec<i32> = serde_json::from_value(v["key"].clone()).map_err(|e| e.to_string())?;
        let Some(ci) = sets.iter().position(|s| s.contains(&key)) else {
            outside += 1;
            continue;
        };
        n_by[ci] += 1;
        let class = v["server"]["class"].as_str().unwrap_or("-").to_string();
        let level = match v["status"].as_str() {
            Some("served") => v["evidence"].as_str().unwrap_or("none").to_string(),
            Some(s) => s.to_string(),
            None => "-".into(),
        };
        *by[ci].entry(class).or_default().entry(level).or_default() += 1;
        if ci > 0 {
            writeln!(w, "{}", json!({"key": key, "clause": clauses[ci].id}))
                .map_err(|e| e.to_string())?;
        }
    }
    w.flush().map_err(|e| e.to_string())?;
    for (ci, s) in summaries.iter_mut().enumerate() {
        s["brought_in"] = json!(n_by[ci]);
        s["by_class_level_brought_in"] = json!(by[ci]);
    }
    Ok(
        json!({"clauses": summaries, "default_clause": clauses.first().map(|c| c.id.clone()),
        "clause_keys_file": ck.display().to_string(), "union_keys_outside_every_clause": outside}),
    )
}

/// The clause of a key (first clause whose key set contains it), from the key-set files written by [`per_clause`].
pub fn clause_of_keys(
    clauses: &[ClauseCfg],
    dir: &Path,
    keys: &[Vec<i32>],
) -> Result<Vec<Option<String>>, String> {
    let mut out = vec![None; keys.len()];
    for c in clauses {
        let p = dir.join(format!("{}.keyset", c.id));
        let f = std::fs::File::open(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        let s = KeySet::read(std::io::BufReader::new(f)).map_err(|e| format!("{e:?}"))?;
        for (i, k) in keys.iter().enumerate() {
            if out[i].is_none() && s.contains(k) {
                out[i] = Some(c.id.clone());
            }
        }
    }
    Ok(out)
}
