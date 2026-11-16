//! Lane sizegen (interfaces/genchoice.md 5): IMPROVE MODE under a REAL `rr_campaign` supervisor with `rr_fake_child`
//! answering the single-rule generate jobs. Spawns processes: run ONLY inside a private pid namespace
//! (`LIGHT_PIDNS=1 .../light_build.sh sizegen test -p rustred-campaign --test improve_fake`).
//!
//! TOY table (arity 3, test data only): rule 0 (5,000 monomials), rule 1 (3,000), rule 2 (10). Filter: the 2 largest.
//! Rule 0: candidate `rung` gives 100 monomials with a SUBSET of the exceptions -> replaced; `tight` gives 50 but a new
//! exception branch -> refused. Rule 1: `rung` is not smaller, `tight` hits the wall -> kept. A second run on the
//! improved table + record does nothing; widening the filter tries rule 2 only.

mod closeloop_support;
mod sup_common;

use closeloop_support::{blake3_file, sha256_file};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use sup_common::*;

const FAMILY: &str = "I(\n name(TOY),\n loops(k1),\n externals(q),\n dimension(d),\n prop(D1,(k1)^2,1),\n prop(D2,(k1+q)^2,1),\n prop(D3,(k1-q)^2,0),\n gram(q,q,1)\n)\n";

fn rec(fixed: Value, target: &str, exc: Value, monomials: u64) -> Value {
    json!({"chart":"110","fixed":fixed,"equations":[],"target":target,"rhs":[["I(n0-1,n1,n2)","1"]],
        "exceptions":exc,"dispatch":"Partition","witness_loci":[],"monomials":monomials})
}

fn table(dir: &Path) -> PathBuf {
    let t = dir.join("T.table.jsonl");
    let mut lines = vec![
        json!({"type":"header","schema":"table_dump.v1","family":"TOY",
        "node_order":{"name":"id","slot_permutation":null,"coordinate_priority":null,"source_order":"Input"}}),
    ];
    for (i, (f, tg, e, m)) in [
        (
            json!([null, null, null]),
            "I(n0,n1,n2)",
            json!([["-1+n0"], ["n2"]]),
            5000u64,
        ),
        (json!([1, null, null]), "I(1,n1,n2)", json!([]), 3000),
        (json!([1, 1, null]), "I(1,1,n2)", json!([]), 10),
    ]
    .into_iter()
    .enumerate()
    {
        let mut r = rec(f, tg, e, m);
        r["type"] = json!("rule");
        r["index"] = json!(i);
        if i == 1 {
            // a heavy rule's record is long (bytes ~ monomials in real tables)
            r["pad"] = json!("x".repeat(2000));
        }
        lines.push(r);
    }
    lines.push(json!({"type":"end","charts":1,"rules":3}));
    std::fs::write(
        &t,
        lines.iter().map(|l| format!("{l}\n")).collect::<String>(),
    )
    .unwrap();
    t
}

fn config(dir: &Path, name: &str, table: &Path, record: Option<&Path>, largest: usize) -> PathBuf {
    config_mode(dir, name, table, record, largest, false)
}

fn config_mode(
    dir: &Path,
    name: &str,
    table: &Path,
    record: Option<&Path>,
    largest: usize,
    subtree: bool,
) -> PathBuf {
    let fam = dir.join("TOY.sym");
    std::fs::write(&fam, FAMILY).unwrap();
    let run = |rules: Vec<Value>, stop: Value| json!({"rules": rules, "stop": stop});
    let runs = json!({
        "0": {"rung": run(vec![rec(json!([null,null,null]), "I(n0,n1,n2)", json!([["-1+n0"]]), 100)], json!("max_cases")),
              "tight": run(vec![rec(json!([null,null,null]), "I(n0,n1,n2)", json!([["n1"]]), 50)], json!("max_cases"))},
        "1": {"rung": run(vec![rec(json!([1,null,null]), "I(1,n1,n2)", json!([]), 4000)], json!("max_cases")),
              "tight": run(vec![], json!("budget_wall"))},
        "2": {"rung": run(vec![rec(json!([1,1,null]), "I(1,1,n2)", json!([]), 5)], json!("max_cases")),
              "tight": run(vec![rec(json!([1,1,null]), "I(1,1,n2)", json!([]), 9)], json!("max_cases"))},
    });
    let mut node = json!({"node":"toy","family":fam.display().to_string(),"family_sha256":sha256_file(&fam),
        "table":{"path":table.display().to_string(),"sha256":sha256_file(table)}});
    if let Some(r) = record {
        node["record"] = json!({"path": r.display().to_string(), "sha256": sha256_file(r)});
    }
    let cfg = json!({"schema":"rustred.campaign.improve.v1","program":"rr_node","nodes":[node],
        "improve":{"filter":{"largest":largest},"subtree":subtree,
                   "candidates":[{"name":"rung","args":["--seed-pose-rung"]},{"name":"tight","args":["--seed-layer-cap","0,0,1","--max-depth","3"]}]},
        "job":{"cap_bytes":268435456u64,"expected_peak_bytes":67108864u64,"full_cap_bytes":268435456u64,
               "full_expected_peak_bytes":67108864u64,"heartbeat_s":1,"stop_grace_s":3},
        "fake":{"program":"rr_fake_child","ms":5,"runs":runs}});
    let p = dir.join(name);
    std::fs::write(&p, serde_json::to_vec_pretty(&cfg).unwrap()).unwrap();
    p
}

fn spec(t: &Tmp, name: &str, cfg: &Path) -> PathBuf {
    write_spec(
        t,
        name,
        &json!({
            "schema": "rustred.campaign.spec.v1", "campaign": format!("{}-{name}", t.campaign),
            "programs": {"rr_fake_child": {"path": FAKE}, "rr_node": {"path": FAKE}},
            "resources": {"policy": "plain", "cpus": cpus(), "max_jobs": 4, "mem_total": "2G", "disk_floor": "1G",
                          "grace_seconds": 5, "sampler_ms": 300, "host_floor": "100M", "heartbeat_timeout_s": 60,
                          "start_timeout_s": 120},
            "progress": {"cadence_seconds": 1},
            "inputs": [{"id": "improve-config", "role": "genchoice", "path": cfg.display().to_string(),
                        "blake3": blake3_file(cfg)}]
        }),
    )
}

fn read_json(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display())))
        .unwrap()
}

#[test]
fn improve_replaces_only_verified_smaller_rules_records_everything_and_iterates() {
    let _g = serial();
    let t = Tmp::new("improve");
    let m = t.path.join("material");
    std::fs::create_dir_all(&m).unwrap();
    let tab = table(&m);
    // run 1: empty record, the 2 largest rules
    let c1 = config(&m, "c1.json", &tab, None, 2);
    let s1 = spec(&t, "s1.json", &c1);
    let d1 = t.cdir("cdir1");
    let codes = run_to_end(&s1, &d1, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&d1));
    let res = read_json(&d1.join("genchoice/toy/improve/result.r1.json"));
    assert_eq!(res["replaced"], 1, "{res:#}");
    let out = res["table_out"]["path"].as_str().unwrap().to_string();
    let lines: Vec<Value> = std::fs::read_to_string(&out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 5);
    assert_eq!(
        (lines[1]["index"].as_i64(), lines[1]["monomials"].as_u64()),
        (Some(0), Some(100)),
        "same position"
    );
    assert_eq!(lines[2]["monomials"], 3000, "rule 1 kept");
    let old =
        std::fs::read_to_string(d1.join("genchoice/toy/improve/replaced_rules.r1.jsonl")).unwrap();
    assert!(old.contains("5000"), "the old rule is kept aside");
    let rec_p = PathBuf::from(res["record"]["path"].as_str().unwrap());
    let rec = read_json(&rec_p);
    assert_eq!(rec["table_sha256"], res["table_out"]["sha256"]);
    assert_eq!(rec["version"], 1);
    let e0 = rec["rules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["index"] == 0)
        .unwrap();
    assert_eq!(e0["chosen"], "rule:rung");
    let v: Vec<&str> = e0["tried"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["verdict"].as_str().unwrap())
        .collect();
    assert!(
        v.contains(&"replaced") && v.iter().any(|x| x.starts_with("exceptions_not_subset")),
        "{v:?}"
    );
    let e1 = rec["rules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["index"] == 1)
        .unwrap();
    assert_eq!(e1["chosen"], "original");
    assert_eq!(e1["tried"].as_array().unwrap().len(), 2);
    let e2 = rec["rules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["index"] == 2)
        .unwrap();
    assert!(
        e2["tried"].as_array().unwrap().is_empty(),
        "outside the filter: untried"
    );
    let rk: Vec<u64> = rec["ranking_by_bytes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x[1].as_u64().unwrap())
        .collect();
    assert!(
        rk.windows(2).all(|w| w[0] >= w[1]),
        "ranking by bytes, largest first: {rk:?}"
    );
    // run 2: the improved table + its record, unchanged settings: nothing to try, table unchanged
    let c2 = config(&m, "c2.json", Path::new(&out), Some(&rec_p), 2);
    let s2 = spec(&t, "s2.json", &c2);
    let d2 = t.cdir("cdir2");
    let codes = run_to_end(&s2, &d2, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&d2));
    // the largest 2 are now rules 1 (tried) and 0 (rung / tight tried): nothing new
    let res2 = read_json(&d2.join("genchoice/toy/improve/result.r2.json"));
    assert_eq!(res2["nothing_to_try"], true, "{res2:#}");
    assert_eq!(res2["table_out"]["sha256"], res["table_out"]["sha256"]);
    // run 3: widen the filter: rule 2 only; rung gives 5 < 10 monomials
    let rec2 = PathBuf::from(res2["record"]["path"].as_str().unwrap());
    let c3 = config(&m, "c3.json", Path::new(&out), Some(&rec2), 3);
    let s3 = spec(&t, "s3.json", &c3);
    let d3 = t.cdir("cdir3");
    let codes = run_to_end(&s3, &d3, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&d3));
    let res3 = read_json(&d3.join("genchoice/toy/improve/result.r3.json"));
    assert_eq!(res3["replaced"], 1, "{res3:#}");
    let rec3 = read_json(Path::new(res3["record"]["path"].as_str().unwrap()));
    assert_eq!(rec3["runs"].as_array().unwrap().len(), 3);
}

/// SUB-TREE mode: rule 1's regeneration is a COMPLETE sub-table (its case rule with a NEW exception branch + the branch's
/// child): the exception-subset test would refuse the case rule alone, the sub-table replaces it (smaller in sum).
#[test]
fn improve_subtree_replaces_a_rule_by_a_complete_smaller_sub_table() {
    let _g = serial();
    let t = Tmp::new("improvest");
    let m = t.path.join("material");
    std::fs::create_dir_all(&m).unwrap();
    let tab = table(&m);
    let fam = m.join("TOY.sym");
    std::fs::write(&fam, FAMILY).unwrap();
    let run = |rules: Vec<Value>, stop: Value| json!({"rules": rules, "stop": stop});
    let runs = json!({
        "0": {"rung": run(vec![rec(json!([null,null,null]), "I(n0,n1,n2)", json!([]), 9000)], Value::Null)},
        "1": {"rung": run(vec![rec(json!([1,null,null]), "I(1,n1,n2)", json!([["n2"]]), 100),
                              rec(json!([1,null,0]), "I(1,n1,0)", json!([]), 50)], Value::Null)},
    });
    let cfg = json!({"schema":"rustred.campaign.improve.v1","program":"rr_node",
        "nodes":[{"node":"toy","family":fam.display().to_string(),"family_sha256":sha256_file(&fam),
                  "table":{"path":tab.display().to_string(),"sha256":sha256_file(&tab)}}],
        "improve":{"filter":{"largest":2},"subtree":true,"candidates":[{"name":"rung","args":["--seed-pose-rung"]}]},
        "job":{"cap_bytes":268435456u64,"expected_peak_bytes":67108864u64,"full_cap_bytes":268435456u64,
               "full_expected_peak_bytes":67108864u64,"heartbeat_s":1,"stop_grace_s":3},
        "fake":{"program":"rr_fake_child","ms":5,"runs":runs}});
    let c = m.join("cst.json");
    std::fs::write(&c, serde_json::to_vec_pretty(&cfg).unwrap()).unwrap();
    let s = spec(&t, "sst.json", &c);
    let d = t.cdir("cdirst");
    let codes = run_to_end(&s, &d, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&d));
    let res = read_json(&d.join("genchoice/toy/improve/result.r1.json"));
    let rec = read_json(Path::new(res["record"]["path"].as_str().unwrap()));
    assert_eq!(res["replaced"], 1, "{res:#} {:#}", rec["rules"]);
    let out = res["table_out"]["path"].as_str().unwrap().to_string();
    let lines: Vec<Value> = std::fs::read_to_string(&out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let rules: Vec<&Value> = lines.iter().filter(|l| l["type"] == "rule").collect();
    assert_eq!(rules.len(), 4, "rule 1 -> 2 rules");
    assert_eq!(
        rules[1]["index"], 1,
        "the sub-table's case rule keeps the old index"
    );
    assert_eq!(rules[2]["index"], 3, "its child takes a fresh index");
    assert_eq!(rules[2]["monomials"], 50);
    assert_eq!(
        lines.last().unwrap()["rules"],
        4,
        "the end record restates the rule count"
    );
    // rule 0's regeneration is larger: kept
    assert_eq!(rules[0]["monomials"], 5000);
}

/// IMPROVE POLES (interfaces/genchoice.md 6): the pole objective's two-run protocol. Own route-pole scores: rule 0
/// regular (not targeted), rule 1 order 2, rule 2 order 1. Run 1 generates (pending, nothing replaced); the candidates are
/// scored (here: given); run 2 decides from the scores without generating again: rule 1 <- `rung` (regular, larger but
/// within the tolerance 2), rule 2 <- `rung` (equal poles, smaller), rule 2's `tight` (order 2) refused. Run 3 on the
/// improved table: the old `original` scores are bound to the old table, nothing to do.
#[test]
fn improve_poles_generates_then_decides_from_scores_and_never_regenerates() {
    let _g = serial();
    let t = Tmp::new("improvepl");
    let m = t.path.join("material");
    std::fs::create_dir_all(&m).unwrap();
    let tab = table(&m);
    let fam = m.join("TOY.sym");
    std::fs::write(&fam, FAMILY).unwrap();
    let tab_sha = sha256_file(&tab);
    let sc = |m: Option<u32>, kwp: u64| {
        json!({"keys": 100, "keys_with_pole": kwp, "undecided": u64::from(m.is_none()), "max_order": m,
            "composed_max_order": null})
    };
    let own = |i: i64, m: u32| {
        json!({"schema":"rustred.campaign.pole-score.v1","node":"toy","scope":"rule","cand":"original","rule_index":i,
            "table":{"path":tab.display().to_string(),"sha256":tab_sha},"score":sc(Some(m), u64::from(m) * 3)})
    };
    let write_scores = |name: &str, lines: &[Value]| {
        let p = m.join(name);
        std::fs::write(&p, lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
        p
    };
    let run = |rules: Vec<Value>, stop: Value| json!({"rules": rules, "stop": stop});
    let runs = json!({
        "1": {"rung": run(vec![rec(json!([1,null,null]), "I(1,n1,n2)", json!([]), 4000)], json!("max_cases")),
              "tight": run(vec![], json!("budget_wall"))},
        "2": {"rung": run(vec![rec(json!([1,1,null]), "I(1,1,n2)", json!([]), 5)], json!("max_cases")),
              "tight": run(vec![rec(json!([1,1,null]), "I(1,1,n2)", json!([]), 9)], json!("max_cases"))},
    });
    let cfg = |name: &str, table: &Path, record: Option<&Path>, scores: &Path| {
        let mut node = json!({"node":"toy","family":fam.display().to_string(),"family_sha256":sha256_file(&fam),
            "table":{"path":table.display().to_string(),"sha256":sha256_file(table)},
            "pole_scores":{"path":scores.display().to_string(),"sha256":sha256_file(scores)}});
        if let Some(r) = record {
            node["record"] = json!({"path": r.display().to_string(), "sha256": sha256_file(r)});
        }
        let c = json!({"schema":"rustred.campaign.improve.v1","program":"rr_node","nodes":[node],
            "improve":{"filter":{"largest":3},"poles":{},
                       "candidates":[{"name":"rung","args":["--seed-pose-rung"]},{"name":"tight","args":["--seed-layer-cap","0,0,1","--max-depth","3"]}]},
            "job":{"cap_bytes":268435456u64,"expected_peak_bytes":67108864u64,"full_cap_bytes":268435456u64,
                   "full_expected_peak_bytes":67108864u64,"heartbeat_s":1,"stop_grace_s":3},
            "fake":{"program":"rr_fake_child","ms":5,"runs":runs}});
        let p = m.join(name);
        std::fs::write(&p, serde_json::to_vec_pretty(&c).unwrap()).unwrap();
        p
    };
    let originals = [own(0, 0), own(1, 2), own(2, 1)];
    // run 1: generate, everything pending
    let s1p = write_scores("s1.jsonl", &originals);
    let c1 = cfg("p1.json", &tab, None, &s1p);
    let d1 = t.cdir("cdirp1");
    let codes = run_to_end(&spec(&t, "sp1.json", &c1), &d1, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&d1));
    let res1 = read_json(&d1.join("genchoice/toy/improve/result.r1.json"));
    assert_eq!((res1["replaced"].clone(), res1["nothing_to_try"].clone()), (json!(0), json!(false)), "{res1:#}");
    let rec1p = PathBuf::from(res1["record"]["path"].as_str().unwrap());
    let rec1 = read_json(&rec1p);
    let entry = |rec: &Value, i: i64| rec["rules"].as_array().unwrap().iter().find(|e| e["index"] == i).unwrap().clone();
    assert!(entry(&rec1, 0)["tried"].as_array().unwrap().is_empty(), "regular rule 0 not targeted");
    let tried = |rec: &Value, i: i64, c: &str| {
        entry(rec, i)["tried"].as_array().unwrap().iter().find(|t| t["cand"] == c && t["scope"] == "rule").unwrap().clone()
    };
    let t1r = tried(&rec1, 1, "rung");
    assert!(t1r["verdict"].as_str().unwrap().starts_with("pole_score_pending"), "{t1r}");
    assert!(tried(&rec1, 1, "tight")["verdict"].as_str().unwrap().starts_with("no_rule"));
    // the scorer's lines for the generated candidates (rule 1 rung regular; rule 2 rung order 1, tight order 2)
    let cand_line = |i: i64, c: &str, m: u32| {
        let tp = tried(&rec1, i, c)["table"].as_str().unwrap().to_string();
        json!({"schema":"rustred.campaign.pole-score.v1","node":"toy","scope":"rule","cand":c,"rule_index":i,
            "table":{"path":tp,"sha256":sha256_file(Path::new(&tp))},"score":sc(Some(m), u64::from(m))})
    };
    let mut lines = originals.to_vec();
    lines.extend([cand_line(1, "rung", 0), cand_line(2, "rung", 1), cand_line(2, "tight", 2)]);
    let s2p = write_scores("s2.jsonl", &lines);
    // run 2: decide from the scores, no job
    let c2 = cfg("p2.json", &tab, Some(&rec1p), &s2p);
    let d2 = t.cdir("cdirp2");
    let codes = run_to_end(&spec(&t, "sp2.json", &c2), &d2, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&d2));
    assert!(!d2.join("jobs").exists() || std::fs::read_dir(d2.join("jobs")).unwrap().next().is_none(), "no job");
    let res2 = read_json(&d2.join("genchoice/toy/improve/result.r2.json"));
    let dbg = read_json(Path::new(res2["record"]["path"].as_str().unwrap()));
    assert_eq!(res2["replaced"], 2, "{res2:#} {:#}", dbg["rules"]);
    let out = res2["table_out"]["path"].as_str().unwrap().to_string();
    let rules: Vec<Value> = std::fs::read_to_string(&out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|l| l["type"] == "rule")
        .collect();
    assert_eq!(
        rules.iter().map(|r| r["monomials"].as_u64().unwrap()).collect::<Vec<_>>(),
        vec![5000, 4000, 5],
        "rule 1 <- regular rung (larger, within tolerance 2), rule 2 <- rung (equal poles, smaller)"
    );
    let rec2p = PathBuf::from(res2["record"]["path"].as_str().unwrap());
    let rec2 = read_json(&rec2p);
    let v2: Vec<(String, String)> = entry(&rec2, 2)["tried"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["scope"] == "rule_poles")
        .map(|t| (t["cand"].as_str().unwrap().to_string(), t["verdict"].as_str().unwrap().to_string()))
        .collect();
    assert!(v2.contains(&("rung".into(), "replaced_size".into())), "{v2:?}");
    assert!(v2.iter().any(|(c, v)| c == "tight" && v.starts_with("poles_not_better")), "{v2:?}");
    let e1 = entry(&rec2, 1);
    assert_eq!(e1["chosen"], "rule:rung");
    assert_eq!(e1["pole"]["max_order"], 0, "the record carries the new rule's score");
    assert!(e1["why"].as_str().unwrap().contains("pole objective"));
    assert_eq!(rec2["runs"][1]["totals"]["objective"], "poles");
    assert_eq!(rec2["runs"][1]["totals"]["score_form"], "count", "records name the score form");
    assert_eq!(e1["pole"]["form"], "count");
    // run 3: the improved table, unchanged scores: the `original` lines describe the old table: nothing to do
    let c3 = cfg("p3.json", Path::new(&out), Some(&rec2p), &s2p);
    let d3 = t.cdir("cdirp3");
    let codes = run_to_end(&spec(&t, "sp3.json", &c3), &d3, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&d3));
    let res3 = read_json(&d3.join("genchoice/toy/improve/result.r3.json"));
    assert_eq!((res3["nothing_to_try"].clone(), res3["replaced"].clone()), (json!(true), json!(0)), "{res3:#}");
}
