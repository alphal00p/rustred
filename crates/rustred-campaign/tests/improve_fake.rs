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
use std::time::Duration;
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
    // TRICKS 31 / 35: every generated candidate points at its effective derivation, written beside the record
    let t0 = e0["tried"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["verdict"] == "replaced")
        .unwrap();
    let dptr = t0["derivation"].as_str().unwrap();
    let (dfile, djob) = dptr.split_once('#').unwrap();
    let ders: Vec<Value> = std::fs::read_to_string(d1.join(dfile))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let dl = ders
        .iter()
        .find(|d| format!("{}.a{}", d["job"].as_str().unwrap(), d["attempt"]) == djob)
        .unwrap();
    assert_eq!(dl["schema"], "rustred.campaign.rule-derivation.v1");
    assert!(
        dl["rule"]["committed"]["canon_blake3"].is_string(),
        "{dl:#}"
    );
    assert_eq!(
        dl["replay_args"], dl["args"],
        "replay = the producing job's own args (deterministic): {dl:#}"
    );
    assert!(dl["relation_retries"].is_array() && dl["attempt_that_produced"].is_string());
    assert!(dl["engine"]["sha256"].as_str().unwrap().len() == 64);
    assert_eq!(
        rec["runs"][0]["totals"]["derivations"]["lines"]
            .as_u64()
            .unwrap(),
        ders.len() as u64
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
    assert_eq!(
        res2["skipped_by_record"],
        json!([1, 0]),
        "the relaunch skips what the record holds: {res2:#}"
    );
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
    // POSITIONED (native importer order rule): the child takes the next position, the rule after it moves by one
    assert_eq!(rules[2]["index"], 2, "its child takes the next position");
    assert_eq!(rules[2]["monomials"], 50);
    assert_eq!(
        (rules[3]["index"].as_i64(), rules[3]["monomials"].as_u64()),
        (Some(3), Some(10)),
        "old rule 2 moved to 3"
    );
    for (k, r) in rules.iter().enumerate() {
        assert_eq!(r["index"], k as i64, "index = position in file order");
    }
    assert_eq!(rules[2]["origin"]["replaces"]["index"], 1, "{:#}", rules[2]);
    // strict read of the written table (every member known to the reader) + the positions map
    let tf = rustred_campaign_core::record::read_table_file(&out, Default::default()).unwrap();
    assert_eq!(tf.rules.len(), 4);
    assert!(
        tf.rules.iter().all(|r| r.unknown_members.is_empty()),
        "no member unknown to the reader (origin is a known member)"
    );
    let pm = read_json(Path::new(res["positions"].as_str().unwrap()));
    let entries = pm["entries"].as_array().unwrap();
    assert!(
        entries
            .iter()
            .any(|e| e["old"] == 1 && e["new"] == json!([1, 2]) && e["replaced"] == true),
        "{pm:#}"
    );
    assert!(
        entries
            .iter()
            .any(|e| e["old"] == 2 && e["new"] == json!([3])),
        "{pm:#}"
    );
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
        std::fs::write(
            &p,
            lines.iter().map(|l| format!("{l}\n")).collect::<String>(),
        )
        .unwrap();
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
    assert_eq!(
        (res1["replaced"].clone(), res1["nothing_to_try"].clone()),
        (json!(0), json!(false)),
        "{res1:#}"
    );
    let rec1p = PathBuf::from(res1["record"]["path"].as_str().unwrap());
    let rec1 = read_json(&rec1p);
    let entry = |rec: &Value, i: i64| {
        rec["rules"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["index"] == i)
            .unwrap()
            .clone()
    };
    assert!(
        entry(&rec1, 0)["tried"].as_array().unwrap().is_empty(),
        "regular rule 0 not targeted"
    );
    let tried = |rec: &Value, i: i64, c: &str| {
        entry(rec, i)["tried"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["cand"] == c && t["scope"] == "rule")
            .unwrap()
            .clone()
    };
    let t1r = tried(&rec1, 1, "rung");
    assert!(
        t1r["verdict"]
            .as_str()
            .unwrap()
            .starts_with("pole_score_pending"),
        "{t1r}"
    );
    assert!(
        tried(&rec1, 1, "tight")["verdict"]
            .as_str()
            .unwrap()
            .starts_with("no_rule")
    );
    // the scorer's lines for the generated candidates (rule 1 rung regular; rule 2 rung order 1, tight order 2)
    let cand_line = |i: i64, c: &str, m: u32| {
        let tp = tried(&rec1, i, c)["table"].as_str().unwrap().to_string();
        json!({"schema":"rustred.campaign.pole-score.v1","node":"toy","scope":"rule","cand":c,"rule_index":i,
            "table":{"path":tp,"sha256":sha256_file(Path::new(&tp))},"score":sc(Some(m), u64::from(m))})
    };
    let mut lines = originals.to_vec();
    lines.extend([
        cand_line(1, "rung", 0),
        cand_line(2, "rung", 1),
        cand_line(2, "tight", 2),
    ]);
    let s2p = write_scores("s2.jsonl", &lines);
    // run 2: decide from the scores, no job
    let c2 = cfg("p2.json", &tab, Some(&rec1p), &s2p);
    let d2 = t.cdir("cdirp2");
    let codes = run_to_end(&spec(&t, "sp2.json", &c2), &d2, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&d2));
    assert!(
        !d2.join("jobs").exists() || std::fs::read_dir(d2.join("jobs")).unwrap().next().is_none(),
        "no job"
    );
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
        rules
            .iter()
            .map(|r| r["monomials"].as_u64().unwrap())
            .collect::<Vec<_>>(),
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
        .map(|t| {
            (
                t["cand"].as_str().unwrap().to_string(),
                t["verdict"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert!(
        v2.contains(&("rung".into(), "replaced_size".into())),
        "{v2:?}"
    );
    assert!(
        v2.iter()
            .any(|(c, v)| c == "tight" && v.starts_with("poles_not_better")),
        "{v2:?}"
    );
    let e1 = entry(&rec2, 1);
    assert_eq!(e1["chosen"], "rule:rung");
    assert_eq!(
        e1["pole"]["max_order"], 0,
        "the record carries the new rule's score"
    );
    assert!(e1["why"].as_str().unwrap().contains("pole objective"));
    assert_eq!(rec2["runs"][1]["totals"]["objective"], "poles");
    assert_eq!(
        rec2["runs"][1]["totals"]["score_form"], "count",
        "records name the score form"
    );
    assert_eq!(
        rec2["runs"][1]["totals"]["score_level"], "rule",
        "and its level"
    );
    assert_eq!(
        rec2["runs"][1]["totals"]["dependences"], "upper_bound",
        "no dependence table given"
    );
    assert_eq!(e1["pole"]["level"], "rule");
    assert_eq!(e1["pole"]["form"], "count");
    // run 3: the improved table, unchanged scores: the `original` lines describe the old table: nothing to do
    let c3 = cfg("p3.json", Path::new(&out), Some(&rec2p), &s2p);
    let d3 = t.cdir("cdirp3");
    let codes = run_to_end(&spec(&t, "sp3.json", &c3), &d3, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&d3));
    let res3 = read_json(&d3.join("genchoice/toy/improve/result.r3.json"));
    assert_eq!(
        (res3["nothing_to_try"].clone(), res3["replaced"].clone()),
        (json!(true), json!(0)),
        "{res3:#}"
    );
}

/// interfaces/genchoice.md 5b (v1.7; tool defects found by lane sizegen-takeover on v31). A YIELD STOP engages after the first
/// (fast, qualifying) answer while two slow jobs run: a PARTIAL snapshot is written and a `improve_yield` note says so. An orderly
/// stop (SIGTERM drain) requeues the slow attempts (queued) and writes a partial snapshot again. The resumed session must NOT
/// idle (the v31 deadlock: queued attempts counted as running under the stop): it finishes at once with the final outputs, the
/// not-run pairs listed. Variant Y: before the resume the operator SKIPS one queued in-progress item and RETRIES the other: both
/// are applied (were refused on v31), the queued attempts are cancelled, the retried item is held back by the stop (note).
#[test]
fn improve_yield_stop_drain_partial_outputs_no_deadlock_and_operator_controls() {
    let _g = serial();
    let t = Tmp::new("improveys");
    let m = t.path.join("material");
    std::fs::create_dir_all(&m).unwrap();
    let tab = table(&m);
    let fam = m.join("TOY.sym");
    std::fs::write(&fam, FAMILY).unwrap();
    let run =
        |rules: Vec<Value>, stop: Value, ms: u64| json!({"rules": rules, "stop": stop, "ms": ms});
    // the fast job must spend MEASURABLE cpu (a job's cpu = sampled /proc ticks of 10 ms; a sleeping fake spends none): its
    // memory growth touches 150 MB of pages within ~1 s (under the 256 MiB job cap)
    let burn = |mut r: Value| {
        r["rss"] = json!({"grow_bytes_per_s": 150_000_000u64, "max_bytes": 150_000_000u64});
        r
    };
    // rule 0: qualifies (100 < 5000 monomials) after 2 s; rules 1 and 2: slow (killed by the drain long before they end)
    let runs = json!({
        "0": {"rung": burn(run(vec![rec(json!([null,null,null]), "I(n0,n1,n2)", json!([["-1+n0"]]), 100)], json!("max_cases"), 2000))},
        "1": {"rung": run(vec![rec(json!([1,null,null]), "I(1,n1,n2)", json!([]), 10)], json!("max_cases"), 600_000)},
        "2": {"rung": run(vec![rec(json!([1,1,null]), "I(1,1,n2)", json!([]), 1)], json!("max_cases"), 600_000)},
    });
    let cfg = json!({"schema":"rustred.campaign.improve.v1","program":"rr_node",
        "nodes":[{"node":"toy","family":fam.display().to_string(),"family_sha256":sha256_file(&fam),
                  "table":{"path":tab.display().to_string(),"sha256":sha256_file(&tab)}}],
        "improve":{"filter":{"largest":3},"subtree":false,"candidates":[{"name":"rung","args":["--seed-pose-rung"]}],
                   // stop as soon as any CPU is spent: every yield is below 1e30 B/CPU-h
                   "yield_stop":{"min_fraction":1.0,"reference_bytes_per_cpu_h":1e30,"min_cpu_s":0}},
        "job":{"cap_bytes":268435456u64,"expected_peak_bytes":67108864u64,"full_cap_bytes":268435456u64,
               "full_expected_peak_bytes":67108864u64,"heartbeat_s":1,"stop_grace_s":3},
        "fake":{"program":"rr_fake_child","ms":5,"runs":runs}});
    let cfg_p = m.join("ys.json");
    std::fs::write(&cfg_p, serde_json::to_vec_pretty(&cfg).unwrap()).unwrap();
    let spec_p = write_spec(
        &t,
        "ys.spec.json",
        &json!({
            "schema": "rustred.campaign.spec.v1", "campaign": format!("{}-ys", t.campaign),
            "programs": {"rr_fake_child": {"path": FAKE}, "rr_node": {"path": FAKE}},
            "resources": {"policy": "plain", "cpus": cpus(), "max_jobs": 3, "mem_total": "2G", "disk_floor": "1G",
                          "grace_seconds": 5, "sampler_ms": 300, "host_floor": "100M", "heartbeat_timeout_s": 60,
                          "start_timeout_s": 120},
            "progress": {"cadence_seconds": 1},
            "inputs": [{"id": "improve-config", "role": "genchoice", "path": cfg_p.display().to_string(),
                        "blake3": blake3_file(&cfg_p)}]
        }),
    );
    let im = |d: &Path| d.join("genchoice/toy/improve");
    let item = |i: u32| format!("toy.im.{i}.rung");
    for variant in ["X", "Y"] {
        let d = t.cdir(&format!("cdir{variant}"));
        // session 1: the fast answer engages the stop -> partial snapshot; then SIGTERM (orderly stop)
        let mut c = start(&spec_p, &d, &["--plain"], &[]);
        let part = im(&d).join("partial/result.r1.partial.json");
        let t0 = std::time::Instant::now();
        while !part.exists() {
            assert!(
                t0.elapsed() < Duration::from_secs(120),
                "no partial snapshot at the yield stop; notes {:?}; log {}",
                notes(&d, "improve_"),
                log_tail(&d)
            );
            std::thread::sleep(Duration::from_millis(100));
        }
        let p1 = read_json(&part);
        assert_eq!(p1["partial"], true, "{p1:#}");
        assert_eq!(p1["replaced"], 1, "rule 0 qualified: {p1:#}");
        assert!(p1["why"].as_str().unwrap().contains("yield stop"), "{p1:#}");
        assert!(p1["yield"]["stopped_by_yield"].as_bool().unwrap(), "{p1:#}");
        assert!(
            !im(&d).join("result.r1.json").exists(),
            "no final result while jobs run"
        );
        let eng = notes(&d, "improve_yield ");
        assert!(
            eng.iter().any(|n| n.contains("\"state\":\"engaged\"")),
            "{eng:?}"
        );
        signal(&c, libc::SIGTERM);
        let code = wait(&mut c, Duration::from_secs(120));
        assert_ne!(code, 0, "an orderly stop is not a finish: {}", log_tail(&d));
        let p2 = read_json(&part);
        assert!(
            p2["why"].as_str().unwrap().contains("orderly stop"),
            "drain snapshot: {p2:#}"
        );
        let pending: Vec<String> = p2["pending"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect();
        assert_eq!(pending, vec![item(1), item(2)], "{p2:#}");
        assert!(
            im(&d)
                .join("partial/improved.r1.partial.table.jsonl")
                .exists(),
            "the snapshot carries the table"
        );
        assert!(im(&d).join("partial/record.r1.partial.json").exists());
        assert!(
            notes(&d, "improve_partial ").len() >= 2,
            "one note per snapshot"
        );
        let st = state(&d);
        for i in [1, 2] {
            let it = &st.items[&item(i)];
            assert_eq!(
                it.phase,
                rustred_campaign::state::ItemPhase::InProgress,
                "{}",
                item(i)
            );
            let j = &st.jobs[it.jobs.last().unwrap()];
            assert_eq!(
                j.state,
                rustred_campaign::state::AttemptState::Queued,
                "requeued by the drain: {}",
                item(i)
            );
        }
        let launches_before = count(&d, |e| {
            matches!(
                e,
                rustred_campaign::ledger::Event::Job {
                    ev: rustred_campaign::ledger::JobEvent::Launch { .. },
                    ..
                }
            )
        });
        if variant == "Y" {
            let ds = d.to_string_lossy().into_owned();
            assert_eq!(cli(&["control", &ds, "skip", &item(1)]).0, 0);
            assert_eq!(cli(&["control", &ds, "retry", &item(2)]).0, 0);
        }
        // session 2: must finish without relaunching anything (v31: idled forever)
        let t0 = std::time::Instant::now();
        let codes = run_to_end(&spec_p, &d, &["--plain"]);
        assert_eq!(codes.last(), Some(&0), "{}", log_tail(&d));
        assert!(
            t0.elapsed() < Duration::from_secs(120),
            "no slow job may run again"
        );
        let launches_after = count(&d, |e| {
            matches!(
                e,
                rustred_campaign::ledger::Event::Job {
                    ev: rustred_campaign::ledger::JobEvent::Launch { .. },
                    ..
                }
            )
        });
        assert_eq!(
            launches_after, launches_before,
            "nothing relaunched under the yield stop"
        );
        let res = read_json(&im(&d).join("result.r1.json"));
        assert_eq!(res["partial"], false, "{res:#}");
        assert_eq!(res["replaced"], 1, "{res:#}");
        assert_eq!(res["stopped_by"], "yield_stop", "{res:#}");
        assert_eq!(res["not_run"], 2, "{res:#}");
        assert!(
            res["yield"]["stop"]
                .as_str()
                .unwrap()
                .starts_with("yield stop: stop below"),
            "{res:#}"
        );
        let rec = read_json(Path::new(res["record"]["path"].as_str().unwrap()));
        let untried = |i: i64| -> Vec<String> {
            rec["rules"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["index"] == i)
                .unwrap()["untried"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap().to_string())
                .collect()
        };
        let log = std::fs::read_to_string(d.with_extension("log")).unwrap_or_default();
        assert!(
            log.contains("improve_yield {"),
            "the yield decisions are log lines too"
        );
        assert!(
            log.contains("yield stop: stop below"),
            "the effective setting is logged at the start"
        );
        let st = state(&d);
        if variant == "X" {
            assert!(
                untried(1)[0].contains("not run: tranche yield stop"),
                "{:?}",
                untried(1)
            );
            assert!(
                untried(2)[0].contains("not run: tranche yield stop"),
                "{:?}",
                untried(2)
            );
            // the resumed session logs how many launches the stop holds back (the two queued attempts)
            assert!(
                notes(&d, "improve_yield ")
                    .iter()
                    .any(|n| n.contains("\"held_back\":2")),
                "{:?}",
                notes(&d, "improve_yield ")
            );
        } else {
            let applied = |verb: &str| {
                controls(&d)
                    .iter()
                    .any(|(_, v, s)| v == verb && s == "applied")
            };
            assert!(applied("skip") && applied("retry"), "{:?}", controls(&d));
            assert!(
                untried(1)[0].contains("skipped by the operator"),
                "{:?}",
                untried(1)
            );
            assert!(
                untried(2)[0].contains("not run: tranche yield stop"),
                "{:?}",
                untried(2)
            );
            assert_eq!(
                st.items[&item(1)].phase,
                rustred_campaign::state::ItemPhase::Skipped
            );
            assert_eq!(
                st.items[&item(2)].phase,
                rustred_campaign::state::ItemPhase::Open
            );
            for i in [1, 2] {
                let it = &st.items[&item(i)];
                assert!(
                    it.jobs
                        .iter()
                        .all(|j| st.jobs[j].state != rustred_campaign::state::AttemptState::Queued),
                    "the queued attempt of {} was cancelled",
                    item(i)
                );
            }
            assert!(
                notes(&d, "improve_yield ")
                    .iter()
                    .any(|n| n.contains("\"held_back\":1")),
                "the retried item is held back"
            );
        }
    }
}

/// interfaces/genchoice.md 5b (v1.7): an improve config WITHOUT `improve.yield_stop` has no stop (until v1.6 a silent default
/// tied to the record's first run); the start line says so.
#[test]
fn improve_without_yield_stop_has_none_and_says_so() {
    let _g = serial();
    let t = Tmp::new("improvenys");
    let m = t.path.join("material");
    std::fs::create_dir_all(&m).unwrap();
    let tab = table(&m);
    let c1 = config(&m, "c1.json", &tab, None, 1);
    let s1 = spec(&t, "s1.json", &c1);
    let d1 = t.cdir("cdir1");
    let codes = run_to_end(&s1, &d1, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&d1));
    let log = std::fs::read_to_string(d1.with_extension("log")).unwrap_or_default();
    assert!(
        log.contains("yield stop: none (improve.yield_stop not set)"),
        "{}",
        log_tail(&d1)
    );
    let res = read_json(&d1.join("genchoice/toy/improve/result.r1.json"));
    assert_eq!(res["yield"]["stopped_by_yield"], false, "{res:#}");
    assert_eq!(res["yield"]["yield_stop"], Value::Null, "{res:#}");
    assert_eq!(res["yield"]["measure"], "cumulative", "{res:#}");
    assert!(
        notes(&d1, "improve_yield ").is_empty(),
        "no stop, no yield note"
    );
}
