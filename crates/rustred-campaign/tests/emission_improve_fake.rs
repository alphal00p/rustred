//! Lane gencheck (emission contract, round 2 item 1): the IMPROVE flow with `emission` ON under a REAL `rr_campaign`
//! supervisor, `rr_fake_child` answering the generate jobs AND standing in for the independent checker
//! (`rr_fake_child --mode weight-cert`: binds its answer to the rule line's relation / domain and the certificate's
//! sha256 like `rr_proof`, verdicts from the certificate's `fake_checker` member). Spawns processes: run ONLY inside a
//! private pid namespace (`LIGHT_PIDNS=1 .../light_build.sh gencheck test -p rustred-campaign --test
//! emission_improve_fake`).
//!
//! TOY table (arity 3, test data only), first brought in by `legacy-import` (whole-domain proof objects keyed by table
//! position): every record carries its derivation member. Improve: rule 0's candidate `rung` (smaller, certificate
//! checked WHOLE_DOMAIN) is REPLACED through the emitter; rule 1's `rung` (smaller, but its check has an open locus)
//! is REFUSED by the emission policy and the old rule stays. The improved table passes `table-check` on its own (the
//! kept records' side files carried beside it); without the replacement's derivation member it is refused.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

mod closeloop_support;
mod sup_common;

use closeloop_support::{blake3_file, sha256_file};
use rustred_campaign_core::emission::body::{body_ids, strip_derivation};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use sup_common::*;

const FAMILY: &str = "I(\n name(TOY),\n loops(k1),\n externals(q),\n dimension(d),\n prop(D1,(k1)^2,1),\n prop(D2,(k1+q)^2,1),\n prop(D3,(k1-q)^2,0),\n gram(q,q,1)\n)\n";

fn rec(fixed: Value, target: &str, exc: Value, monomials: u64) -> Value {
    json!({"chart":"110","fixed":fixed,"equations":[],"target":target,"rhs":[["I(n0-1,n1,n2)","1"]],
        "exceptions":exc,"dispatch":"Partition","witness_loci":[],"monomials":monomials})
}

/// The toy table and the legacy-import evidence (one whole-domain verdict per rule, keyed by table position, with the
/// rule's own relation and domain sha256).
fn table_and_evidence(dir: &Path) -> (PathBuf, PathBuf) {
    let t = dir.join("T.table.jsonl");
    let mut lines = vec![
        json!({"type":"header","schema":"table_dump.v1","family":"TOY","mask":"110","denominators":3,
        "node_order":{"name":"id","slot_permutation":null,"coordinate_priority":null,"source_order":"Input"}}),
    ];
    let rules = [
        (
            json!([null, null, null]),
            "I(n0,n1,n2)",
            json!([["-1+n0"], ["n2"]]),
            5000u64,
        ),
        (json!([1, null, null]), "I(1,n1,n2)", json!([]), 3000),
        (json!([1, 1, null]), "I(1,1,n2)", json!([]), 10),
    ];
    let mut tsv = String::from("index\ttarget_rhs_sha256\tdomain_sha256\tclass\tdetail\n");
    for (i, (f, tg, e, m)) in rules.into_iter().enumerate() {
        let mut r = rec(f, tg, e, m);
        r["type"] = json!("rule");
        r["index"] = json!(i);
        let ids = body_ids(&r.to_string()).unwrap();
        tsv.push_str(&format!(
            "{i}\t{}\t{}\tE-WHOLE\ttoy whole-domain proof object\n",
            ids.relation_sha256, ids.domain_sha256
        ));
        lines.push(r);
    }
    lines.push(json!({"type":"end","charts":1,"rules":3}));
    std::fs::write(
        &t,
        lines.iter().map(|l| format!("{l}\n")).collect::<String>(),
    )
    .unwrap();
    let ev = dir.join("verdicts.tsv");
    std::fs::write(&ev, tsv).unwrap();
    let cfg = dir.join("import.json");
    std::fs::write(
        &cfg,
        json!({"schema": "rustred.legacy-import.v1", "cone_bound": 10,
               "verdicts": [{"name": "toy reviewer", "path": ev.display().to_string(), "keyed_by": "table",
                             "index_col": "index", "hash_col": "target_rhs_sha256", "domain_col": "domain_sha256",
                             "class_col": "class", "detail_cols": ["detail"]}]})
        .to_string(),
    )
    .unwrap();
    (t, cfg)
}

fn config(dir: &Path, table: &Path) -> PathBuf {
    let fam = dir.join("TOY.sym");
    std::fs::write(&fam, FAMILY).unwrap();
    let run = |rules: Vec<Value>, stop: Value| json!({"rules": rules, "stop": stop});
    let with_cert = |mut r: Value, fake: Value| {
        r["fake_weight_cert"] = json!({"schema": "fake.weight-cert", "fake_checker": fake});
        r
    };
    let runs = json!({
        "0": {"rung": run(vec![with_cert(rec(json!([null,null,null]), "I(n0,n1,n2)", json!([["-1+n0"]]), 100), json!({}))], json!("max_cases")),
              "tight": run(vec![rec(json!([null,null,null]), "I(n0,n1,n2)", json!([["n1"]]), 50)], json!("max_cases"))},
        "1": {"rung": run(vec![with_cert(rec(json!([1,null,null]), "I(1,n1,n2)", json!([]), 2000),
                                         json!({"class": "OPEN_LOCI", "open": 1}))], json!("max_cases")),
              "tight": run(vec![], json!("budget_wall"))},
    });
    let node = json!({"node":"toy","family":fam.display().to_string(),"family_sha256":sha256_file(&fam),
        "table":{"path":table.display().to_string(),"sha256":sha256_file(table)}});
    let cfg = json!({"schema":"rustred.campaign.improve.v1","program":"rr_node","nodes":[node],
        "improve":{"filter":{"largest":2},"subtree":false,
                   "candidates":[{"name":"rung","args":["--seed-pose-rung"]},{"name":"tight","args":["--seed-layer-cap","0,0,1","--max-depth","3"]}]},
        "emission": {"checker": FAKE, "recover_rows": false, "cert_dir": "out/wcert", "descent_node": FAKE},
        "job":{"cap_bytes":268435456u64,"expected_peak_bytes":67108864u64,"full_cap_bytes":268435456u64,
               "full_expected_peak_bytes":67108864u64,"heartbeat_s":1,"stop_grace_s":3},
        "fake":{"program":"rr_fake_child","ms":5,"runs":runs}});
    let p = dir.join("improve.json");
    std::fs::write(&p, serde_json::to_vec_pretty(&cfg).unwrap()).unwrap();
    p
}

fn spec(t: &Tmp, cfg: &Path) -> PathBuf {
    write_spec(
        t,
        "s.json",
        &json!({
            "schema": "rustred.campaign.spec.v1", "campaign": format!("{}-emit", t.campaign),
            "programs": {"rr_fake_child": {"path": FAKE}, "rr_node": {"path": FAKE}},
            "resources": {"policy": "plain", "cpus": cpus(), "containment": "marker_only", "max_jobs": 4, "mem_total": "2G", "disk_floor": "1G",
                          "grace_seconds": 5, "sampler_ms": 300, "host_floor": "100M", "heartbeat_timeout_s": 60,
                          "start_timeout_s": 120},
            "progress": {"cadence_seconds": 1},
            "inputs": [{"id": "improve-config", "role": "genchoice", "path": cfg.display().to_string(),
                        "blake3": blake3_file(cfg)}]
        }),
    )
}

fn table_check(table: &Path) -> (Value, bool) {
    rustred_campaign::emission::tablecheck::table_check(
        &rustred_campaign::emission::tablecheck::Args {
            table: table.to_path_buf(),
            every: 1,
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
fn improve_with_emission_writes_replacements_only_through_the_emitter_and_table_check_accepts() {
    let _g = serial();
    let t = Tmp::new("emitimprove");
    let m = t.path.join("material");
    std::fs::create_dir_all(&m).unwrap();
    let (raw, icfg) = table_and_evidence(&m);
    // 1. legacy import: every record carries its derivation member (class from the bound proof objects)
    let imp = m.join("imported");
    let (s, ok) = rustred_campaign::emission::legacy_import::run(
        &rustred_campaign::emission::legacy_import::Args {
            table: raw.clone(),
            config: icfg,
            out: imp.clone(),
            write_table: true,
            policy: None,
        },
    )
    .unwrap();
    assert!(ok, "{s:#}");
    assert_eq!(s["by_word"]["whole domain"], 3, "{s:#}");
    let tab = imp.join("T.table.jsonl");
    let (tc, ok) = table_check(&tab);
    assert!(ok, "imported table: {tc:#}");
    // 2. improve with emission on
    let cfg = config(&m, &tab);
    let sp = spec(&t, &cfg);
    let d = t.cdir("cdir");
    let codes = run_to_end(&sp, &d, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&d));
    let res: Value = serde_json::from_slice(
        &std::fs::read(d.join("genchoice/toy/improve/result.r1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(res["replaced"], 1, "{res:#}");
    let out = PathBuf::from(res["table_out"]["path"].as_str().unwrap());
    let lines: Vec<String> = std::fs::read_to_string(&out)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    let rule = |i: usize| -> (Value, Value) {
        let (body, der) = strip_derivation(&lines[i]).unwrap();
        (
            serde_json::from_str(&body).unwrap(),
            serde_json::from_str(&der.expect("every rule line carries its derivation member"))
                .unwrap(),
        )
    };
    // rule 0: the replacement, emitted (origin lift, the fake checker's output and the certificate beside the table)
    let (b0, d0) = rule(1);
    assert_eq!(
        (b0["index"].as_i64(), b0["monomials"].as_u64()),
        (Some(0), Some(100))
    );
    assert_eq!(d0["origin"], "lift", "{d0:#}");
    assert_eq!(d0["class"]["word"], "whole domain", "{d0:#}");
    assert!(
        b0.get("fake_weight_cert").is_none(),
        "the certificate is not in the record"
    );
    let side = rustred_campaign_core::emission::side_dir(&out);
    let cert = d0["certificate"]["sha256"].as_str().unwrap();
    assert!(side.join(format!("cert/{cert}.json")).is_file());
    // rule 1: the smaller candidate's check has an open locus: refused by the policy, the old (imported) rule stays
    let (b1, d1) = rule(2);
    assert_eq!(b1["monomials"], 3000, "rule 1 kept");
    assert_eq!(d1["origin"], "import");
    let rec: Value =
        serde_json::from_slice(&std::fs::read(res["record"]["path"].as_str().unwrap()).unwrap())
            .unwrap();
    let e1 = rec["rules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["index"] == 1)
        .unwrap();
    assert_eq!(e1["chosen"], "original");
    assert!(
        e1["untried"]
            .as_array()
            .unwrap()
            .iter()
            .any(|u| u.as_str().unwrap().contains("emission refused")),
        "{e1:#}"
    );
    // 3. the improved table checks on its own (kept records' side files carried)
    let (tc, ok) = table_check(&out);
    assert!(ok, "improved table: {tc:#}");
    assert_eq!(tc["classes"]["whole domain"], 3, "{tc:#}");
    // a replacement line written WITHOUT the emitter (its derivation member removed) is refused
    let mut forged = lines.clone();
    forged[1] = strip_derivation(&lines[1]).unwrap().0;
    let ft = d.join("forged.table.jsonl");
    std::fs::write(
        &ft,
        forged.iter().map(|l| format!("{l}\n")).collect::<String>(),
    )
    .unwrap();
    rustred_campaign::emission::carry_side(&side, &rustred_campaign_core::emission::side_dir(&ft))
        .unwrap();
    let (tc, ok) = table_check(&ft);
    assert!(!ok, "{tc:#}");
    assert_eq!(tc["refused"], 1, "{tc:#}");
}
