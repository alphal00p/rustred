//! INPUT BINDING: the delivery manifest's sha256 (own SHA-256), every rule's `origin.record_sha256` against the
//! ORIGINAL line of its source file (sources are only read), the field-level difference between each table record and
//! its original (expected: `index`, `origin`, and for a transformed record exactly one appended exception branch equal
//! to its moved locus), the per-line index file, and the table order against the stated dispatch order.

use std::collections::{BTreeMap, HashMap};

use serde_json::{Value, json};

use crate::sha256;
use crate::table::for_each_line;
use crate::{Args, die};

pub fn check_manifest(dir: &str) -> Value {
    let mpath = format!("{dir}/MANIFEST.json");
    let m: Value = serde_json::from_str(
        &std::fs::read_to_string(&mpath).unwrap_or_else(|e| die(&e.to_string())),
    )
    .unwrap_or_else(|e| die(&e.to_string()));
    let files = m["files"]
        .as_array()
        .unwrap_or_else(|| die("manifest without files"));
    let mut ok = 0usize;
    let mut bad = vec![];
    for f in files {
        let p = f["path"].as_str().unwrap();
        let full = format!("{dir}/{p}");
        match sha256::file_hex(&full) {
            Ok((h, n)) => {
                if Some(h.as_str()) == f["sha256"].as_str() && Some(n) == f["bytes"].as_u64() {
                    ok += 1;
                } else {
                    bad.push(json!({"path": p, "sha256": h, "bytes": n}));
                }
            }
            Err(e) => bad.push(json!({"path": p, "error": e})),
        }
    }
    // files present but not listed
    let mut listed: std::collections::BTreeSet<String> = files
        .iter()
        .map(|f| f["path"].as_str().unwrap().to_string())
        .collect();
    listed.insert("MANIFEST.json".into());
    let mut extra = vec![];
    fn walk(base: &str, rel: &str, out: &mut Vec<String>) {
        let d = if rel.is_empty() {
            base.to_string()
        } else {
            format!("{base}/{rel}")
        };
        if let Ok(rd) = std::fs::read_dir(&d) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                let r = if rel.is_empty() {
                    name.clone()
                } else {
                    format!("{rel}/{name}")
                };
                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    walk(base, &r, out);
                } else {
                    out.push(r);
                }
            }
        }
    }
    let mut all = vec![];
    walk(dir, "", &mut all);
    for f in all {
        if !listed.contains(&f) {
            extra.push(f);
        }
    }
    json!({"schema": "rr_pverify.hashes.v1", "files": files.len(), "match": ok, "mismatch": bad,
           "unlisted_files": extra, "verdict": if bad.is_empty() { "YES" } else { "NO" }})
}

/// sha256 of every rule line of a source file, by its `index` field, and the raw line (kept only when asked).
struct Source {
    by_index: HashMap<u64, (String, Option<Fields>)>,
}

/// sha256 of each top-level field's canonical JSON (serde_json maps are key-sorted), plus the exceptions value.
#[derive(Clone)]
struct Fields {
    h: BTreeMap<String, String>,
    exceptions: Value,
    index: Option<u64>,
}

fn fields_of(v: &Value) -> Fields {
    let mut h = BTreeMap::new();
    if let Some(o) = v.as_object() {
        for (k, x) in o {
            h.insert(
                k.clone(),
                sha256::hex(serde_json::to_string(x).unwrap().as_bytes()),
            );
        }
    }
    Fields {
        h,
        exceptions: v["exceptions"].clone(),
        index: v["index"].as_u64(),
    }
}

fn index_source(path: &str, keep: &std::collections::HashSet<u64>) -> Result<Source, String> {
    let mut s = Source {
        by_index: HashMap::new(),
    };
    for_each_line(path, |no, raw, v| {
        let h = sha256::hex(raw.as_bytes());
        if let Some(i) = v.get("index").and_then(Value::as_u64)
            && (v["type"].as_str() == Some("rule") || v["type"].as_str() == Some("sym"))
        {
            let line = if keep.contains(&i) {
                Some(fields_of(&v))
            } else {
                None
            };
            if s.by_index.insert(i, (h.clone(), line)).is_some() {
                return Err(format!("{path}: duplicate index {i}"));
            }
        }
        let _ = no;
        Ok(())
    })?;
    Ok(s)
}

fn diff_fields(a: &Fields, b: &Fields) -> Vec<String> {
    let mut keys: Vec<&String> = a.h.keys().chain(b.h.keys()).collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .filter(|k| a.h.get(*k) != b.h.get(*k))
        .map(|k| k.to_string())
        .collect()
}

/// `origins --table T --sources sources.json --moved moved_loci.json --index index.jsonl [--provenance P]`
pub fn cmd(mut a: Args) {
    let tp = a.need("--table");
    let sp = a.need("--sources");
    let mp = a.need("--moved");
    let ip = a.need("--index");
    let out = a.take("--out");
    let recipes = a.take("--recipes");
    a.done();
    // pass 1: table records (origin, raw sha, record without rhs for the comparison: rhs compared by sha of its text)
    struct Rec {
        line: usize,
        origin: Value,
        body: Fields,
        len: usize,
    }
    let mut recs: Vec<Rec> = vec![];
    let mut table_records_math: HashMap<usize, Value> = HashMap::new();
    let mut line_sha: Vec<(String, usize)> = vec![];
    for_each_line(&tp, |no, raw, v| {
        let sha = sha256::hex(raw.as_bytes());
        line_sha.push((sha.clone(), raw.len()));
        if v["type"].as_str() == Some("rule") {
            let mut body = v;
            let origin = body
                .as_object_mut()
                .unwrap()
                .remove("origin")
                .unwrap_or(Value::Null);
            if origin.get("original_origin").is_some() {
                let mut m = serde_json::Map::new();
                for k in [
                    "chart",
                    "fixed",
                    "equations",
                    "target",
                    "rhs",
                    "exceptions",
                    "domain_bounds",
                ] {
                    m.insert(k.into(), body.get(k).cloned().unwrap_or(Value::Null));
                }
                table_records_math.insert(recs.len(), Value::Object(m));
            }
            recs.push(Rec {
                line: no,
                origin,
                body: fields_of(&body),
                len: raw.len(),
            });
        }
        Ok(())
    })
    .unwrap_or_else(|e| die(&e));
    eprintln!(
        "table: {} lines, {} rule records",
        line_sha.len(),
        recs.len()
    );
    // index file
    let mut idx_ok = 0usize;
    let mut idx_bad = vec![];
    let mut idx_lines = 0usize;
    let mut off = 0u64;
    let mut offsets = vec![];
    for (h, l) in &line_sha {
        offsets.push((off, *l));
        off += *l as u64 + 1;
        let _ = h;
    }
    for_each_line(&ip, |no, _, v| {
        idx_lines += 1;
        let (o, l) = offsets.get(no).copied().unwrap_or((u64::MAX, 0));
        let h = &line_sha.get(no).map(|x| x.0.clone()).unwrap_or_default();
        if v["offset"].as_u64() == Some(o)
            && v["len"].as_u64() == Some(l as u64)
            && v["sha256"].as_str() == Some(h)
        {
            idx_ok += 1;
        } else if idx_bad.len() < 20 {
            idx_bad.push(json!({"line": no}));
        }
        Ok(())
    })
    .unwrap_or_else(|e| die(&e));
    // moved loci
    let moved: Value = serde_json::from_str(&std::fs::read_to_string(&mp).unwrap()).unwrap();
    let mut moved_map: HashMap<(String, u64), Value> = HashMap::new();
    for m in moved.as_array().unwrap() {
        moved_map.insert(
            (m[0].as_str().unwrap().to_string(), m[1].as_u64().unwrap()),
            m[2].clone(),
        );
    }
    // pass 2: sources
    let mut by_path: BTreeMap<String, std::collections::HashSet<u64>> = BTreeMap::new();
    for r in &recs {
        let p = r.origin["path"].as_str().unwrap_or("").to_string();
        by_path
            .entry(p)
            .or_default()
            .insert(r.origin["label"].as_u64().unwrap_or(u64::MAX));
        if let Some(oo) = r.origin.get("original_origin")
            && let Some(p2) = oo["path"].as_str()
        {
            let lab = oo.get("source_index").and_then(Value::as_u64).unwrap_or(0);
            by_path.entry(p2.to_string()).or_default().insert(lab);
        }
    }
    let mut sources: HashMap<String, Source> = HashMap::new();
    for (p, keep) in &by_path {
        match index_source(p, keep) {
            Ok(s) => {
                sources.insert(p.clone(), s);
            }
            Err(e) => eprintln!("source {p}: {e}"),
        }
    }
    eprintln!("indexed {} source files", sources.len());
    let mut sha_ok = 0usize;
    let mut sha_bad = vec![];
    let mut diff_hist: BTreeMap<String, usize> = BTreeMap::new();
    let mut transformed_ok = 0usize;
    let mut transformed_bad = vec![];
    let mut unexpected = vec![];
    let mut original_origin_report = vec![];
    for (pos, r) in recs.iter().enumerate() {
        let p = r.origin["path"].as_str().unwrap_or("");
        let lab = r.origin["label"].as_u64().unwrap_or(u64::MAX);
        let want = r.origin["record_sha256"].as_str().unwrap_or("");
        let Some(src) = sources.get(p) else {
            sha_bad.push(json!({"pos": pos, "why": "source not readable"}));
            continue;
        };
        let Some((h, raw)) = src.by_index.get(&lab) else {
            sha_bad.push(json!({"pos": pos, "why": "label not in source"}));
            continue;
        };
        if h == want {
            sha_ok += 1;
        } else {
            sha_bad
                .push(json!({"pos": pos, "source": p, "label": lab, "have": h, "claimed": want}));
        }
        // field diff against the original
        let orig: &Fields = match raw {
            Some(f) => f,
            None => continue,
        };
        let fields = diff_fields(&r.body, orig);
        for f in &fields {
            *diff_hist.entry(f.clone()).or_default() += 1;
        }
        let transform = !r.origin["transform"].is_null();
        let allowed: Vec<&str> = if transform {
            vec!["index", "exceptions"]
        } else {
            vec!["index"]
        };
        let extra: Vec<&String> = fields
            .iter()
            .filter(|f| !allowed.contains(&f.as_str()))
            .collect();
        if !extra.is_empty() {
            unexpected.push(json!({"pos": pos, "fields": extra, "source": r.origin["source"]}));
        }
        if r.body.index != Some(pos as u64) || orig.index != Some(lab) {
            unexpected.push(json!({"pos": pos, "why": "index fields"}));
        }
        if transform {
            let src_name = r.origin["source"].as_str().unwrap_or("").to_string();
            let mv = moved_map.get(&(src_name.clone(), lab));
            let oe = orig.exceptions.as_array().cloned().unwrap_or_default();
            let ne = r.body.exceptions.as_array().cloned().unwrap_or_default();
            let ok = ne.len() == oe.len() + 1
                && ne[..oe.len()] == oe[..]
                && mv.is_some_and(|m| ne[oe.len()] == *m);
            if ok {
                transformed_ok += 1;
            } else {
                transformed_bad.push(json!({"pos": pos, "source": src_name, "label": lab}));
            }
        }
        if let Some(oo) = r.origin.get("original_origin") {
            // the helper / package originals: record sha and file sha
            let p2 = oo["path"].as_str().unwrap_or("");
            let lab2 = oo.get("source_index").and_then(Value::as_u64).unwrap_or(0);
            let mut entry = json!({"pos": pos, "path": p2});
            if let Some(fs) = oo.get("file_sha256").and_then(Value::as_str) {
                let got = sha256::file_hex(p2).map(|x| x.0).unwrap_or_default();
                entry["file_sha256_ok"] = json!(got == fs);
            }
            if oo.get("domain").is_some() {
                // nested original (one JSON line {.., "rule": {...}, "record_sha256": ..}): field-level comparison
                let mut first: Option<Value> = None;
                let _ = for_each_line(p2, |_, _, v| {
                    if first.is_none() {
                        first = Some(v);
                    }
                    Ok(())
                });
                if let Some(orig) = first {
                    let rule = &orig["rule"];
                    let tab = table_records_math.get(&pos).cloned().unwrap_or(Value::Null);
                    let mut same = vec![];
                    let mut differ = vec![];
                    for k in ["chart", "fixed", "equations", "target", "rhs", "exceptions"] {
                        if rule.get(k).unwrap_or(&Value::Null) == tab.get(k).unwrap_or(&Value::Null)
                        {
                            same.push(k);
                        } else {
                            differ.push(k);
                        }
                    }
                    // domain {"n_i": ">= m" | "<= m"} against domain_bounds.min / max (and nothing else)
                    let mut dom_ok = true;
                    let n = rule["fixed"].as_array().map(|a| a.len()).unwrap_or(0);
                    let mut want_min: Vec<Value> = vec![Value::Null; n];
                    let mut want_max: Vec<Value> = vec![Value::Null; n];
                    for (k, v) in rule["domain"].as_object().cloned().unwrap_or_default() {
                        let i: usize = k.trim_start_matches('n').parse().unwrap_or(usize::MAX);
                        let t = v.as_str().unwrap_or("").trim();
                        let (ge, rest) = if let Some(r) = t.strip_prefix(">=") {
                            (true, r)
                        } else if let Some(r) = t.strip_prefix("<=") {
                            (false, r)
                        } else {
                            (true, "x")
                        };
                        match (i < n, rest.trim().parse::<i64>()) {
                            (true, Ok(m)) => {
                                if ge {
                                    want_min[i] = json!(m)
                                } else {
                                    want_max[i] = json!(m)
                                }
                            }
                            _ => dom_ok = false,
                        }
                    }
                    let db = &tab["domain_bounds"];
                    let norm = |x: &Value| -> Value {
                        if x.is_null() {
                            Value::Array(vec![Value::Null; n])
                        } else {
                            x.clone()
                        }
                    };
                    if norm(&db["min"]) != Value::Array(want_min.clone())
                        || norm(&db["max"]) != Value::Array(want_max.clone())
                    {
                        dom_ok = false;
                    }
                    // a stated rewrite of diagonal target slots: original slot "c*n_a+o" = new slot "n_j+o" with a case
                    // equation n_j - c*n_a = 0; every other slot, the coefficients and the exceptions identical
                    let mut rewrite_check = Value::Null;
                    if !oo["rewrite"].is_null() {
                        rewrite_check = json!(check_diagonal_rewrite(rule, &tab));
                    }
                    entry["nested_original"] = json!({"record_sha256_field": orig["record_sha256"],
                        "claimed": oo["record_sha256"], "field_equal_to_claim": orig["record_sha256"] == oo["record_sha256"],
                        "math_fields_equal": same, "math_fields_differ": differ, "domain_to_domain_bounds_ok": dom_ok,
                        "rewrite": oo["rewrite"], "rewrite_check": rewrite_check});
                }
            }
            if let Some(rs) = oo.get("record_sha256").and_then(Value::as_str) {
                let got = sources
                    .get(p2)
                    .and_then(|s| s.by_index.get(&lab2))
                    .map(|x| x.0.clone());
                // single-record files: the whole first rule line
                let got = got.or_else(|| {
                    let mut first = None;
                    let _ = for_each_line(p2, |_, raw, v| {
                        if first.is_none()
                            && (v["type"].as_str() == Some("rule")
                                || v["type"].as_str() == Some("sym"))
                        {
                            first = Some(sha256::hex(raw.as_bytes()));
                        }
                        Ok(())
                    });
                    first
                });
                entry["record_sha256_ok"] = json!(got.as_deref() == Some(rs));
                if let Some(Some(o2)) = sources
                    .get(p2)
                    .and_then(|s| s.by_index.get(&lab2))
                    .map(|x| x.1.clone())
                {
                    entry["fields_differing_from_original"] = json!(diff_fields(&r.body, &o2));
                }
            }
            original_origin_report.push(entry);
        }
    }
    // order: sources.json sequence, minus recipe-class entries, then the table's per-key point rules
    let sj: Value = serde_json::from_str(&std::fs::read_to_string(&sp).unwrap()).unwrap();
    // records stated as moved out of the table (recipe stage of the delivered sources)
    let mut moved_out: std::collections::HashSet<(String, u64)> = Default::default();
    if let Some(rp) = &recipes {
        let rj: Value = serde_json::from_str(&std::fs::read_to_string(rp).unwrap()).unwrap();
        for s in rj["sources"].as_array().unwrap() {
            if s["class"].as_str() == Some("recipe") {
                for r in s["records"].as_array().cloned().unwrap_or_default() {
                    moved_out.insert((
                        s["path"].as_str().unwrap_or("").to_string(),
                        r.as_u64().unwrap(),
                    ));
                }
            }
        }
    }
    let mut expected: Vec<(String, u64)> = vec![];
    for s in sj["sources"].as_array().unwrap() {
        let p = s["path"].as_str().unwrap_or("").to_string();
        let recs: Vec<u64> = match s["records"].as_array() {
            Some(a) => a.iter().map(|r| r.as_u64().unwrap()).collect(),
            None => {
                // every rule record of the file, in file order
                let mut v = vec![];
                let _ = for_each_line(&p, |_, _, x| {
                    if x["type"].as_str() == Some("rule") {
                        v.push(x["index"].as_u64().unwrap_or(u64::MAX));
                    }
                    Ok(())
                });
                v
            }
        };
        for r in recs {
            if !moved_out.contains(&(p.clone(), r)) {
                expected.push((p.clone(), r));
            }
        }
    }
    let have: Vec<(String, u64)> = recs
        .iter()
        .map(|r| {
            (
                r.origin["path"].as_str().unwrap_or("").to_string(),
                r.origin["label"].as_u64().unwrap_or(0),
            )
        })
        .collect();
    let prefix_ok = have.len() >= expected.len() && have[..expected.len()] == expected[..];
    let first_diff = (0..expected.len().min(have.len())).find(|&i| have[i] != expected[i]);
    let tail: Vec<Value> = have[expected.len().min(have.len())..]
        .iter()
        .map(|(p, l)| json!([p, l]))
        .collect();
    let excluded_from_sources: Vec<Value> = sj["sources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| {
            s["class"].as_str() == Some("recipe")
                || s["id"].as_str().unwrap_or("").contains("F1 / F8")
        })
        .map(|s| json!({"id": s["id"], "records": s["records"]}))
        .collect();
    let res = json!({
        "schema": "rr_pverify.origins.v1",
        "rules": recs.len(),
        "index_file": {"lines": idx_lines, "ok": idx_ok, "bad": idx_bad},
        "record_sha256": {"ok": sha_ok, "bad": sha_bad},
        "field_differences_histogram": diff_hist,
        "unexpected_differences": unexpected,
        "transformed": {"ok": transformed_ok, "bad": transformed_bad, "moved_loci": moved_map.len()},
        "original_origins": original_origin_report,
        "order": {"expected_from_sources": expected.len(), "prefix_equal": prefix_ok, "first_difference": first_diff,
                  "moved_out_as_recipes": moved_out.iter().map(|(p, l)| json!([p, l])).collect::<Vec<_>>(),
                  "appended_after_sources": tail, "sources_entries_excluded": excluded_from_sources},
        "rule_bytes_total": recs.iter().map(|r| r.len).sum::<usize>(),
        "first_rule_line": recs.first().map(|r| r.line),
        "table_line_sha_count": line_sha.len(),
    });
    let s = serde_json::to_string_pretty(&res).unwrap();
    if let Some(p) = out {
        std::fs::write(&p, &s).unwrap();
    }
    println!("{s}");
}

/// Check a diagonal-slot rewrite: original pattern slot `c*n_a(+o)` versus new slot `n_j(+o)`, with the new record's case
/// equations implying n_j = c * n_a; every other slot equal; coefficients equal term by term; exceptions equal.
fn check_diagonal_rewrite(orig: &Value, new: &Value) -> Value {
    let n = orig["fixed"].as_array().map(|a| a.len()).unwrap_or(0);
    let split = |t: &str| -> Vec<String> {
        t.trim()
            .trim_start_matches("I(")
            .trim_end_matches(')')
            .split(',')
            .map(|x| x.trim().to_string())
            .collect()
    };
    // parse "c*n_a+o" / "n_j+o"
    fn lin_slot(x: &str) -> Option<(i64, usize, i64)> {
        let (c, rest) = match x.find("*n") {
            Some(p) => (x[..p].parse::<i64>().ok()?, &x[p + 1..]),
            None => (1, x),
        };
        let rest = rest.strip_prefix('n')?;
        let end = rest.find(['+', '-']).unwrap_or(rest.len());
        let a: usize = rest[..end].parse().ok()?;
        let o: i64 = if end == rest.len() {
            0
        } else {
            rest[end..].trim_start_matches('+').parse().ok()?
        };
        Some((c, a, o))
    }
    let mut diag: std::collections::BTreeMap<usize, (i64, usize)> = Default::default();
    let ot = split(orig["target"].as_str().unwrap_or(""));
    let nt = split(new["target"].as_str().unwrap_or(""));
    if ot.len() != n || nt.len() != n {
        return json!({"ok": false, "why": "arity"});
    }
    for j in 0..n {
        if ot[j] != nt[j] {
            match (
                ot[j].contains('*').then(|| lin_slot(&ot[j])).flatten(),
                lin_slot(&nt[j]),
            ) {
                (Some((c, a, 0)), Some((1, jj, 0))) if jj == j => {
                    diag.insert(j, (c, a));
                }
                _ => {
                    return json!({"ok": false, "why": format!("target slot {j}: {} vs {}", ot[j], nt[j])});
                }
            }
        }
    }
    // equations: each diagonal slot needs n_j - c n_a = 0 among the new equations (as affine forms)
    let eqs: Vec<crate::rat::Lin> = new["equations"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|e| crate::expr::Expr::parse(e.as_str().unwrap_or(""), n).ok())
        .filter_map(|e| crate::rat::Lin::from_node(&e.root, n))
        .collect();
    for (&j, &(c, a)) in &diag {
        let mut want = crate::rat::Lin::zero(n);
        want.a[j] = crate::rat::Rat::ONE;
        want.a[a] = want.a[a].sub(crate::rat::Rat::int(c as i128));
        let found = eqs.iter().any(|l| {
            let s = if l.a[j].is_zero() {
                return false;
            } else {
                crate::rat::Rat::ONE.div(l.a[j])
            };
            l.scale(s) == want
        });
        if !found {
            return json!({"ok": false, "why": format!("no case equation n{j} = {c} n{a}")});
        }
    }
    let orhs = orig["rhs"].as_array().cloned().unwrap_or_default();
    let nrhs = new["rhs"].as_array().cloned().unwrap_or_default();
    if orhs.len() != nrhs.len() {
        return json!({"ok": false, "why": "rhs length"});
    }
    for (ti, (x, y)) in orhs.iter().zip(&nrhs).enumerate() {
        if x[1] != y[1] {
            return json!({"ok": false, "why": format!("coefficient of term {ti} differs")});
        }
        let (op, np) = (
            split(x[0].as_str().unwrap_or("")),
            split(y[0].as_str().unwrap_or("")),
        );
        for j in 0..n {
            if op[j] == np[j] {
                continue;
            }
            // allowed: c*n_a+o -> n_j'+o with (j' diagonal for (c, a))
            let ok = match (lin_slot(&op[j]), lin_slot(&np[j])) {
                (Some((c, a, o)), Some((1, jj, oo))) => {
                    o == oo && diag.get(&jj) == Some(&(c, a)) && op[j].contains('*')
                }
                _ => false,
            };
            if !ok {
                return json!({"ok": false, "why": format!("term {ti} slot {j}: {} vs {}", op[j], np[j])});
            }
        }
    }
    if orig["exceptions"] != new["exceptions"] {
        return json!({"ok": false, "why": "exceptions differ"});
    }
    json!({"ok": true, "diagonal_slots": diag.iter().map(|(j, (c, a))| json!([j, c, a])).collect::<Vec<_>>(), "terms": orhs.len()})
}
