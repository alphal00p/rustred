//! Lane import, BIG DATA (ignored): reproduction of the T178 hand statement (boxed close with recipes, confirmed key by
//! key by the independent checker) from the EXISTING material through the import: the order-B piece tables
//! (generation order, the 16 elided records removed as the statement did), the 110 lane rules in NUMERIC FACE ID order
//! (the coordinator's addendum), the 4 supplied symmetry relations (with their verification record), the 3 terminals.
//! Closure under the STRICT core dispatcher, Tier K (the hand statement follows every top rhs key; T178's facts carry no
//! record_sha256 and cannot be bound), both elided policies; comparison typed.
//!
//! int8 (round 3): the statement is the PINNED box statement v1.0.3 and every table source carries its sha256 pin from
//! the coordinator's services file (`plan/auto-campaign/reports/T178-MARGIN-SERVICES.json`, service 1:
//! statement_dir, statement_json_sha256, 138 tables with sha256; `RUSTRED_T178_SERVICES` overrides the path). A piece of
//! the dispatch order that the statement's table list does not pin (10 order-B pieces: no rule of theirs is the first
//! match of a statement key, 8 have no end record) is left out with its removed records, and named. Before int8 the
//! test read the unpinned regenB set of statement-20261114T040105Z (34 tables without an end record at the time:
//! refused since partfix4).
//!
//! Paths (test data only): `RUSTRED_IMPORT_WS` (default the p_integral_dev workspace), scratch output
//! `RUSTRED_IMPORT_SCRATCH` (default /common/build-cache/$USER/p_integral_dev/implementer-scratch/import).
//! Run: `light_build.sh import test -p rustred-campaign-core --release --test import_t178 -- --ignored --nocapture`.

mod inputs;
use rustred_campaign_core::import;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use import::{
    CheckOptions, Evidence, HandFormat, TableSetSpec, check, compare, load, read_hand_statement,
    write_statement,
};
use serde_json::{Value, json};

/// lane closeloop4 (audit-closeloop M1 / m4): the spec text with its evidence PINNED (each file's sha256, a
/// SHA256SUMS manifest beside a directory) and the family of its tables named (spec evidence is read only so).
#[allow(dead_code)]
fn pin_ev(spec: &serde_json::Value) -> String {
    use std::io::BufRead;
    let mut v = spec.clone();
    let fam = v["sources"].as_array().and_then(|a| {
        a.iter().find_map(|s| {
            let p = s["path"].as_str()?;
            let f = std::fs::File::open(p).ok()?;
            let mut l = String::new();
            std::io::BufReader::new(f).read_line(&mut l).ok()?;
            let h: serde_json::Value = serde_json::from_str(l.trim()).ok()?;
            h["family"].as_str().map(str::to_string)
        })
    });
    let sha = |p: &std::path::Path| rustred_campaign_core::import::lines::file_sha256(p).unwrap();
    if let Some(ev) = v.get_mut("evidence").and_then(|e| e.as_object_mut()) {
        for k in ["facts", "proofs", "solved"] {
            if let Some(a) = ev.get_mut(k).and_then(|x| x.as_array_mut()) {
                for e in a {
                    let p = std::path::PathBuf::from(e["path"].as_str().unwrap_or(""));
                    if p.is_file() && e.get("sha256").is_none() {
                        e["sha256"] = serde_json::json!(sha(&p));
                    }
                    if p.is_dir() && e.get("manifest").is_none() {
                        let mut names: Vec<String> = std::fs::read_dir(&p)
                            .unwrap()
                            .flatten()
                            .map(|x| x.file_name().to_string_lossy().into_owned())
                            .filter(|n| n.ends_with(".json") || n.ends_with(".jsonl"))
                            .collect();
                        names.sort();
                        let body: String = names
                            .iter()
                            .map(|n| format!("{}  {n}\n", sha(&p.join(n))))
                            .collect();
                        let m = std::env::temp_dir().join(format!(
                            "pin-ev-{}-{}.SHA256SUMS",
                            std::process::id(),
                            rustred_campaign_core::closure::sha256::sha256_tag(
                                p.display().to_string().as_bytes()
                            )
                        ));
                        std::fs::write(&m, body).unwrap();
                        e["manifest"] =
                            serde_json::json!({"path": m.display().to_string(), "sha256": sha(&m)});
                    }
                    if e.get("family").is_none()
                        && let Some(f) = &fam
                    {
                        e["family"] = serde_json::json!(f);
                    }
                }
            }
        }
    }
    v.to_string()
}

fn ws() -> PathBuf {
    std::env::var_os("RUSTRED_IMPORT_WS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/shared/vhirschi/t3code/vakint_dev/p_integral_dev"))
}

fn scratch() -> PathBuf {
    std::env::var_os("RUSTRED_IMPORT_SCRATCH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(format!(
                "/common/build-cache/{}/p_integral_dev/implementer-scratch/import",
                std::env::var("USER").unwrap_or_else(|_| "vhirschi".into())
            ))
        })
}

pub fn vm_hwm_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1).and_then(|x| x.parse().ok()))
        })
        .unwrap_or(0)
}

fn piece_path(regen: &Path, piece: &str) -> PathBuf {
    let (dir, sub) = piece.split_once('_').expect("piece name dir_sub");
    regen.join(dir).join(sub).join("node/F.table.jsonl")
}

/// Leading number of a face id ("F12" -> 12, "F5x" -> 5); ids without one sort after every numbered one.
fn face_num(face: &str) -> u64 {
    let d: String = face
        .trim_start_matches('F')
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    d.parse().unwrap_or(u64::MAX)
}

fn hand_format() -> HandFormat {
    {
        let mut f = HandFormat::positional(0, 1, 2, 3);
        f.round = Some(import::spec::Col::Index(4));
        f
    }
}

/// int8: the statement's in-box key count (its summary.json `in_box`).
fn summary_in_box(stmt: &Path) -> u64 {
    let s: Value =
        serde_json::from_str(&std::fs::read_to_string(stmt.join("summary.json")).unwrap()).unwrap();
    s["in_box"].as_u64().unwrap()
}

/// int8: the statement directory's SHA256SUMS as file name -> sha256.
fn statement_sums(dir: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(dir.join("SHA256SUMS"))
        .unwrap()
        .lines()
        .filter_map(|l| l.split_once("  "))
        .map(|(h, n)| (n.to_string(), h.to_string()))
        .collect()
}

fn verify_sums(dir: &Path) {
    for l in std::fs::read_to_string(dir.join("SHA256SUMS"))
        .unwrap()
        .lines()
    {
        let (h, n) = l.split_once("  ").unwrap();
        assert_eq!(
            import::lines::file_sha256(&dir.join(n)).unwrap(),
            h,
            "{n} differs from its SHA256SUMS"
        );
    }
}

/// int8: the pinned statement of T178 (service 1 of the services file): (statement dir, statement.json sha256, table
/// path -> sha256).
fn pinned_statement(ws: &Path) -> Option<(PathBuf, String, BTreeMap<String, String>)> {
    let f = std::env::var_os("RUSTRED_T178_SERVICES")
        .map(PathBuf::from)
        .unwrap_or_else(|| ws.join("plan/auto-campaign/reports/T178-MARGIN-SERVICES.json"));
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&f).ok()?).ok()?;
    let s = v["services"].as_array()?.iter().find(|s| s["order"] == 1)?;
    let pins = s["tables"]
        .as_array()?
        .iter()
        .map(|t| {
            (
                t["path"].as_str().unwrap().to_string(),
                t["sha256"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    Some((
        PathBuf::from(s["statement_dir"].as_str()?),
        s["statement_json_sha256"].as_str()?.to_string(),
        pins,
    ))
}

fn build_spec(ws: &Path, stmt: &Path, pins: &BTreeMap<String, String>) -> Value {
    let summary: Value =
        serde_json::from_str(&std::fs::read_to_string(stmt.join("summary.json")).unwrap()).unwrap();
    let rules: BTreeMap<String, Value> =
        serde_json::from_str(&std::fs::read_to_string(stmt.join("rules.json")).unwrap()).unwrap();
    let regen = ws.join("SCRATCHPAD/t178/regenB");
    // removed: piece#seq (record order) -> label of that record
    let mut removed: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for s in summary["skipped_elided_records"].as_array().unwrap() {
        let (piece, seq) = s.as_str().unwrap().split_once('#').unwrap();
        let seq: usize = seq.parse().unwrap();
        let raw =
            import::lines::scan_table(piece, &piece_path(&regen, piece), false, &|o, _| o == seq)
                .unwrap();
        let r = &raw.records[0];
        assert_eq!(r.ordinal, seq);
        removed.entry(piece.to_string()).or_default().push(json!({"label": r.rec.index.unwrap(),
            "reason": "elided witness loci: removed from the dispatch by the hand statement (skip_elided)"}));
    }
    let mut sources = vec![];
    let mut unpinned = vec![];
    for n in summary["dispatch_order"].as_array().unwrap() {
        let n = n.as_str().unwrap();
        if n == "new" {
            continue;
        }
        let p = piece_path(&regen, n);
        assert!(p.is_file(), "{}", p.display());
        // int8: only the statement's PINNED tables (a piece it does not pin serves no statement key)
        let Some(sha) = pins.get(&p.display().to_string()) else {
            removed.remove(n);
            unpinned.push(n.to_string());
            continue;
        };
        sources.push(json!({"id": n, "kind":"engine_table", "class":"orderB", "path": p.display().to_string(),
            "sha256": sha, "removed": removed.remove(n).unwrap_or_default(), "hand_name":{"template":"{source}#{ordinal}"}}));
    }
    println!(
        "T178 spec: {} order-B pieces pinned, {} left out (not in the statement's pinned table list): {unpinned:?}",
        sources.len(),
        unpinned.len()
    );
    assert!(
        removed.is_empty(),
        "every skipped record belongs to a dispatched piece"
    );
    // the 110 lane rules in numeric face id order
    let mut new: Vec<(u64, String, &Value)> = rules
        .iter()
        .filter(|(k, _)| k.starts_with("faces"))
        .map(|(k, v)| (face_num(k.split('/').nth(1).unwrap()), k.clone(), v))
        .collect();
    new.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    assert_eq!(new.len(), 110);
    for (_, name, v) in &new {
        let sha = pins
            .get(v["path"].as_str().unwrap())
            .unwrap_or_else(|| panic!("lane rule {name}: table not pinned by the statement"));
        sources.push(
            json!({"id": name, "kind":"engine_table", "class":"lane", "path": v["path"], "sha256": sha,
            "records":[v["index"]], "hand_name":{"template":"{source}"}}),
        );
    }
    let sym = rules["symmetry#0"]["path"].as_str().unwrap().to_string();
    let sym_sha = pins
        .get(&sym)
        .expect("symmetry relations not pinned by the statement");
    sources.push(json!({"id":"symmetry","kind":"supplied_relations","class":"symmetry","path": sym, "sha256": sym_sha,
        "verification":{"path": ws.join("SCRATCHPAD/t178-close/data/symrules.verify.json").display().to_string()},
        "hand_name":{"template":"symmetry#{index}"}}));
    // int8: the statement's own files carry their pins from its SHA256SUMS (verified by `verify_sums`); a terminals
    // source without its sha256 is refused since closeloop4 (also on the earlier statement)
    let sums = statement_sums(stmt);
    sources.push(json!({"id":"terminals","kind":"terminals","path": stmt.join("terminals.json").display().to_string(),
        "sha256": sums["terminals.json"]}));
    json!({"schema":"rustred.campaign.import-spec.v1","node":"T178","frame_from":"R0_try1","bits":6,
        "start":{"path": stmt.join("statement.jsonl").display().to_string(),"format":"hand_statement",
                 "sha256": sums["statement.jsonl"],
                 "hand_format": serde_json::to_value(hand_format()).unwrap()},
        "required_evidence":"per-key","policies":{"tier":"K"},
        "sources": sources})
}

// int8 (int/10, 2026-11-18): the third phase "lift_with_per_key_proof WITH the bound per-key proofs of lane t178-bind"
// (t178-close/proofs-bound-20261114T091622Z, proofs sha256 66f3022f...) is REMOVED. Those proofs were per-key proofs of the
// OLD boxed T178 table (statement 2026-11-14); their proofs.jsonl was deleted by a clean-up on 2026-11-18 (only the manifest
// set survives in WS/handoff/sym-investigation-for-ben/t178-close/) and is not to be regenerated (coordinator's ruling).
// The current delivery's evidence (T178-final: exactness classes keyed by the hash of target + rhs of ITS tables
// core.table.jsonl / beyond.table.jsonl) binds to other records than this statement's 138 pinned tables, so it cannot
// stand in. The phase ran SILENTLY skipped once its directory was gone (gated on the manifest existing): never again.
// Last run WITH it: f75febc3, 2026-11-18T06:49Z: key sets equal 335,306 = 335,306, same server 335,306, differences {}.

#[test]
#[ignore = "big data: the T178 hand statement and its material; run with --ignored in release"]
fn t178_reproduction_of_the_hand_statement() {
    let ws = ws();
    let Some((stmt, stmt_sha, pins)) = pinned_statement(&ws) else {
        inputs::missing(format_args!(
            "SKIP: services file (pinned T178 statement) missing"
        ));
        return;
    };
    if !stmt.join("statement.jsonl").is_file() {
        inputs::missing(format_args!("SKIP: {} missing", stmt.display()));
        return;
    }
    assert_eq!(
        import::lines::file_sha256(&stmt.join("statement.json")).unwrap(),
        stmt_sha,
        "statement.json differs from its pin"
    );
    verify_sums(&stmt);
    let out = scratch().join("t178");
    std::fs::create_dir_all(&out).unwrap();
    let spec_v = build_spec(&ws, &stmt, &pins);
    std::fs::write(
        out.join("spec.T178.json"),
        serde_json::to_string_pretty(&spec_v).unwrap(),
    )
    .unwrap();
    let hand = read_hand_statement(&stmt.join("statement.jsonl"), &hand_format(), 20).unwrap();
    assert_eq!(hand.rows.len(), 335_306);
    // (policy, expected comparison): both reproduced in every run since int8 round 3 (pinned statement v1.0.3)
    let runs: [(&str, bool, u64); 2] = [
        ("block", false, 335_221),
        ("lift_with_per_key_proof", true, 335_226),
    ];
    for (policy, want_equal, want_same_server) in runs {
        let t0 = std::time::Instant::now();
        let mut sv = spec_v.clone();
        sv["policies"]["elided"] = json!(policy);
        let spec = TableSetSpec::from_json(&pin_ev(&sv)).unwrap();
        let set = load(&spec, Path::new("/")).unwrap();
        let t_load = t0.elapsed().as_secs_f64();
        // int8: the statement's own box count (v1.0.3: the ENLARGED R* box, 286,286 keys; 040105Z had 286,066)
        assert_eq!(
            set.start.len() as u64,
            summary_in_box(&stmt),
            "start = the statement's in-box keys"
        );
        let ev = Evidence::from_spec(&set, Path::new("/")).unwrap();
        let res = check(
            &set,
            &ev,
            &CheckOptions {
                threads: 4,
                ..Default::default()
            },
        )
        .unwrap();
        let tag = "";
        let dir = out.join(format!("statement-{policy}{tag}"));
        let timing = json!({"load_s": t_load, "check_s": res.seconds, "vm_hwm_kb": vm_hwm_kb()});
        write_statement(&set, &ev, &res, &dir, Some(timing)).unwrap();
        let cmp = compare(&set, &res, &hand);
        cmp.write(&dir).unwrap();
        println!(
            "T178 [{policy}{tag}] evidence import {:?}; evidence by class {:?}; closure per_key_lifted {:?} \
             requires_per_key_proof {} dispatch_not_strict {:?} diagnostic {}",
            ev.counts,
            res.evidence_by_class(&set),
            res.summary.per_key_lifted,
            res.summary.requires_per_key_proof,
            res.summary.dispatch_not_strict,
            res.summary.diagnostic
        );
        println!(
            "T178 [{policy}{tag}]: load {:.1}s check {:.1}s VmHWM {} MB; tables {} records {}; keys ours {} hand {}; \
             counts {:?}; closure verdict {:?} rounds {} edges {} violations {}; blocking {:?}",
            t_load,
            res.seconds,
            vm_hwm_kb() / 1024,
            set.report.tables,
            set.report.records,
            res.rows.len(),
            hand.rows.len(),
            res.counts(),
            res.summary.verdict,
            res.summary.rounds,
            res.summary.edges,
            res.summary.descent_violations,
            res.blocking()
        );
        println!(
            "T178 [{policy}{tag}] comparison: key sets equal {} same server {} terminals equal {} ({:?} vs {:?}) rounds {:?} vs {}; \
             differences {:?}; unmapped hand rules {}; label matrix {:?}; requests solve {} facts {} proofs {}",
            cmp.key_sets_equal,
            cmp.same_server,
            cmp.terminals_equal,
            cmp.terminals_hand,
            cmp.terminals_ours,
            cmp.rounds_hand,
            cmp.rounds_ours,
            cmp.counts,
            cmp.unmapped_hand_rules.len(),
            cmp.label_matrix,
            res.solve_requests.len(),
            res.fact_requests.len(),
            res.proof_requests.len()
        );
        for d in cmp.differences.iter().take(12) {
            println!("  diff {:?}", d);
        }
        // int8 (int/10): the comparison is ASSERTED, not only printed
        assert_eq!(cmp.key_sets_equal, want_equal, "[{policy}] key sets equal");
        assert_eq!(
            cmp.same_server as u64, want_same_server,
            "[{policy}] same server"
        );
        assert!(cmp.terminals_equal, "[{policy}] terminals equal");
    }
}

/// The FIRST END-TO-END shape (coordinator 07:11Z): T178's order-B material + RECIPES instead of the lane's symbolic /
/// point rules (the 136 explicit `rustred.recipe.v1` recipes of lane engine-line, derived from the checker's
/// reference) + the supplied symmetry relations + terminals. No relation is solved here: the check lists the (recipe,
/// key) SOLVE REQUESTS for lane `adapters` (interfaces/import.md 6.1) and the keys no recipe domain contains.
#[test]
#[ignore = "big data: T178 order-B material and the engine-line recipes; run with --ignored in release"]
fn t178_recipe_variant_solve_requests() {
    let ws = ws();
    let Some((stmt, _, pins)) = pinned_statement(&ws) else {
        inputs::missing(format_args!(
            "SKIP: services file (pinned T178 statement) missing"
        ));
        return;
    };
    let rdir = ws.join("SCRATCHPAD/engine-line/t178");
    if !stmt.join("statement.jsonl").is_file() || !rdir.join("index.json").is_file() {
        inputs::missing(format_args!("SKIP: material missing"));
        return;
    }
    let mut sv = build_spec(&ws, &stmt, &pins);
    let srcs = sv["sources"].as_array_mut().unwrap();
    srcs.retain(|s| s["class"] != "lane");
    let tail: Vec<Value> = srcs.split_off(srcs.len() - 2); // symmetry, terminals
    let index: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(rdir.join("index.json")).unwrap()).unwrap();
    for e in &index {
        let id = e["id"].as_str().unwrap();
        let p = rdir.join("recipes").join(format!("{id}.json"));
        assert!(p.is_file(), "{}", p.display());
        srcs.push(json!({"id": format!("recipe:{id}"), "kind":"recipe", "class":"recipe", "path": p.display().to_string()}));
    }
    srcs.extend(tail);
    let out = scratch().join("t178-recipes");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(
        out.join("spec.T178-recipes.json"),
        serde_json::to_string_pretty(&sv).unwrap(),
    )
    .unwrap();
    let t0 = std::time::Instant::now();
    let spec = TableSetSpec::from_json(&pin_ev(&sv)).unwrap();
    let set = load(&spec, Path::new("/")).unwrap();
    let ev = Evidence::from_spec(&set, Path::new("/")).unwrap();
    let res = check(
        &set,
        &ev,
        &CheckOptions {
            threads: 4,
            ..Default::default()
        },
    )
    .unwrap();
    write_statement(
        &set,
        &ev,
        &res,
        &out.join("statement"),
        Some(json!({"total_s": t0.elapsed().as_secs_f64(), "vm_hwm_kb": vm_hwm_kb()})),
    )
    .unwrap();
    let per_recipe: BTreeMap<&str, usize> =
        res.solve_requests.iter().fold(BTreeMap::new(), |mut m, r| {
            *m.entry(r.source.as_str()).or_default() += 1;
            m
        });
    let no_domain = res
        .rows
        .iter()
        .filter(|r| {
            r.open_status.as_deref() == Some("uncovered")
                && r.open_reason.as_deref() == Some("uncovered; no recipe domain contains the key")
        })
        .count();
    println!(
        "T178 recipe variant: {:.1}s VmHWM {} MB; keys {} counts {:?}; solve requests {} over {} recipes; uncovered keys in no recipe domain {}; blocking {:?}",
        t0.elapsed().as_secs_f64(),
        vm_hwm_kb() / 1024,
        res.rows.len(),
        res.counts(),
        res.solve_requests.len(),
        per_recipe.len(),
        no_domain,
        res.blocking()
    );
}
