#![allow(dead_code)]
//! Test data of the termmin phase-E tests: a TOY node (arity 3, chart 110: two lines, one numerator; nothing five-loop
//! specific) that the closing loop closes at its first check with THREE declared terminals: M = I(1,1,0) (no relation
//! exists: a "true master" the tool does not know about), E = I(2,1,0) (eliminable by the depth-2 point recipe onto M
//! and a hand-down), N = I(1,3,0) (only by a relation onto the NEW key X = I(1,1,-1), which the node's default recipe
//! serves in the closing try). The fake child answers from an oracle keyed by (source, key); phase E's point recipes
//! are the sources `termmin/<step>`.

use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub const HEADER: &str =
    r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":3,"mask":"110"}"#;
pub const M: [i32; 3] = [1, 1, 0];
pub const E: [i32; 3] = [2, 1, 0];
pub const N: [i32; 3] = [1, 3, 0];
pub const X: [i32; 3] = [1, 1, -1];
pub const SEQUENCE: [&str; 5] = [
    "dispatch",
    "symmetry_relation",
    "d1_cnone_foff",
    "d2_clast_fon",
    "d3_cfirst_foff",
];

pub fn sha256_file(p: &Path) -> String {
    let b = std::fs::read(p).unwrap();
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(&b);
    h.finish_hex()
}

pub fn blake3_file(p: &Path) -> String {
    blake3::hash(&std::fs::read(p).unwrap())
        .to_hex()
        .to_string()
}

/// One oracle line: (source, key, verdict, rhs keys).
pub type OracleLine = (String, [i32; 3], String, Vec<[i32; 3]>);

/// What a termmin toy test varies.
#[derive(Clone)]
pub struct TmToy {
    /// Oracle lines (source, key, verdict, rhs) on top of the base answers (an entry replaces the base one).
    pub overrides: Vec<OracleLine>,
    /// No termmin answers at all (every attempt fails).
    pub no_relations: bool,
    /// The default recipe cannot serve X (the relation of N then brings an unservable key).
    pub x_unservable: bool,
    pub quirks: Vec<String>,
    pub ms: u64,
    /// The termmin spec (None: no input of role termmin: the built-in default, enabled).
    pub termmin: Option<Value>,
}

impl Default for TmToy {
    fn default() -> Self {
        TmToy {
            overrides: vec![],
            no_relations: false,
            x_unservable: false,
            quirks: vec![],
            ms: 20,
            termmin: Some(json!({"schema": "rustred.campaign.termmin.v1", "sequence": SEQUENCE})),
        }
    }
}

pub struct TmFiles {
    pub dir: PathBuf,
    pub config: PathBuf,
    pub termmin: Option<PathBuf>,
}

pub fn toy(dir: &Path, t: &TmToy) -> TmFiles {
    std::fs::create_dir_all(dir).unwrap();
    // the table serves the start key [5,1,0] onto [4,1,0] (+ a hand-down); [4,1,0] needs one recipe round (the default
    // recipe, onto the three terminals): the node is closed BOXED after its second check
    let rule = json!({"type":"rule","chart":"110","index":0,"fixed":[5,1,0],"equations":[],"target":"I(5,1,0)",
        "rhs":[["I(4,1,0)","1"],["I(5,0,0)","1"]],
        "exceptions":[],"dispatch":"Partition","witness_loci":[]})
    .to_string();
    let a = dir.join("A.table.jsonl");
    std::fs::write(
        &a,
        format!(
            "{HEADER}\n{}\n{rule}\n{}\n",
            r#"{"type":"chart","chart":"110","guards":["n>=1","n>=1","n<=0"],"zero":false}"#,
            r#"{"type":"end","charts":1}"#
        ),
    )
    .unwrap();
    let a_sha = {
        let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
        h.update(rule.as_bytes());
        h.finish_hex()
    };
    let facts = dir.join("facts.jsonl");
    std::fs::write(
        &facts,
        json!({"schema":"rustred.proof-kernel.instantiate.v1","record_sha256":a_sha,"rule":0,
            "key":[5,1,0],"in_case":true,"chart_ok":true,"branch":null,"regular":"regular",
            "irregular_terms":[],"hidden_loci":[],
            "rhs":[{"term":0,"key":[4,1,0],"coef":"nonzero"},{"term":1,"key":[5,0,0],"coef":"nonzero"}],
            "zero_test":"exact","error":null})
        .to_string()
            + "\n",
    )
    .unwrap();
    let proofs = dir.join("proofs.json");
    std::fs::write(
        &proofs,
        json!({"schema":"rustred.proof-kernel.span.v1","keys":[
            {"record_sha256": a_sha, "key": [5,1,0], "status":"EXACT_IN_SPAN_AT_KEY"}]})
        .to_string(),
    )
    .unwrap();
    let dflt = dir.join("default.recipe.json");
    std::fs::write(
        &dflt,
        json!({"schema":"rustred.recipe.v1","id":"default","arity":3,"chart":"110",
            "domain":{"fixed":[null,null,null],"equations":[],"exceptions":[]},
            "seeds":{"unit":"harder","centres":[],"generator":{"shells":["S0","S1"]}},
            "symmetry_rows":false,"elimination":{"convention":"rustred.recipe-elim.v1"}})
        .to_string(),
    )
    .unwrap();
    let st = dir.join("start.jsonl");
    std::fs::write(&st, "[5,1,0]\n").unwrap();
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "required_evidence":"per-key","policies":{"tier":"C"},
        "sources":[{"id":"A","kind":"engine_table","class":"base","path": a.display().to_string(),
                    "sha256": sha256_file(&a), "hand_name":{"template":"{source}#{label}"}},
                   {"id":"terminals","kind":"terminals","keys":[M, E, N]}],
        // lane closeloop4 (audit-closeloop M1 / m4): pinned spec evidence of the toy family
        "evidence": {"facts":[{"path": facts.display().to_string(), "sha256": sha256_file(&facts), "family": "TOY"}],
                     "proofs":[{"path": proofs.display().to_string(), "sha256": sha256_file(&proofs), "family": "TOY"}]}});
    let import_spec = dir.join("import-spec.json");
    std::fs::write(&import_spec, serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
    // the oracle
    let mut oracle: Vec<OracleLine> = vec![(
        "default-recipe".into(),
        [4, 1, 0],
        "SOLVED".into(),
        vec![E, N, M, [4, 0, 0]],
    )];
    if !t.no_relations {
        oracle.push((
            "termmin/d2_clast_fon".into(),
            E,
            "SOLVED".into(),
            vec![M, [2, 0, 0]],
        ));
        oracle.push((
            "termmin/d2_clast_fon".into(),
            N,
            "SOLVED".into(),
            vec![X, [1, 0, 0]],
        ));
    }
    if !t.x_unservable {
        oracle.push((
            "default-recipe".into(),
            X,
            "SOLVED".into(),
            vec![M, [0, 1, -1]],
        ));
    }
    for o in &t.overrides {
        oracle.retain(|x| !(x.0 == o.0 && x.1 == o.1));
        oracle.push(o.clone());
    }
    let op = dir.join("oracle.jsonl");
    std::fs::write(
        &op,
        oracle
            .iter()
            .map(|(s, k, v, r)| {
                format!(
                    "{}\n",
                    json!({"source": s, "key": k, "verdict": v, "rhs": r})
                )
            })
            .collect::<String>(),
    )
    .unwrap();
    let family = dir.join("TOY.family");
    std::fs::write(&family, "toy family (the fake child never reads it)\n").unwrap();
    let config = dir.join("closeloop.json");
    let cfg = json!({"schema":"rustred.campaign.closeloop.v1","nodes":[{
        "node":"toy", "import_spec": import_spec.display().to_string(), "target":"boxed",
        "check_threads": 2, "default_recipe": dflt.display().to_string(),
        "budgets": {"max_rounds": 20, "max_job_attempts": 2},
        "solve": {"program": "rr_fake_child", "family": family.display().to_string(), "order": "toy-order",
                  // lane closeloop3: the fake child emulates solve-recipe v1.1 (no relation_rule field): the closing
                  // loop's m1_first default (closeloop2) would refuse every answer
                  "relation_rule": "first_admissible",
                  "max_keys_per_job": 2, "threads": 1, "cap_bytes": 268435456,
                  "expected_peak_bytes": 67108864, "heartbeat_s": 1, "stop_grace_s": 3,
                  "fake": {"oracle": op.display().to_string(), "ms": t.ms, "quirks": t.quirks, "program": "rr_fake_child"}}
    }]});
    // lane closeloop3 (closeloop.md 16.7): recipe relations (the closing loop's and phase E's) are per-key only with
    // their bound span proof: fake rr_proof jobs answer EXACT_IN_SPAN_AT_KEY (an empty oracle)
    let po = dir.join("proof-oracle.jsonl");
    std::fs::write(&po, "").unwrap();
    let mut cfg = cfg;
    cfg["nodes"][0]["proof"] = json!({"program": "rr_proof", "rows": "engine",
        "family": family.display().to_string(), "max_keys_per_job": 4, "wall_s": 60,
        "cap_bytes": 268435456, "expected_peak_bytes": 67108864,
        "fake": {"oracle": po.display().to_string(), "ms": t.ms, "program": "rr_fake_child"}});
    std::fs::write(&config, serde_json::to_vec_pretty(&cfg).unwrap()).unwrap();
    let termmin = t.termmin.as_ref().map(|v| {
        let p = dir.join("termmin.json");
        let mut v = v.clone();
        // fake jobs: small caps (the fake child is tiny)
        v["caps"] =
            json!({"cap_bytes": 268435456, "expected_peak_bytes": 67108864, "max_seeds": 1000});
        std::fs::write(&p, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
        p
    });
    TmFiles {
        dir: dir.to_path_buf(),
        config,
        termmin,
    }
}

pub fn campaign_spec(campaign: &str, files: &TmFiles, fake: &str, cpus: &str) -> Value {
    let mut inputs = vec![
        json!({"id": "closeloop-config", "role": "closeloop", "path": files.config.display().to_string(),
                    "blake3": blake3_file(&files.config)}),
    ];
    if let Some(p) = &files.termmin {
        inputs.push(
            json!({"id": "termmin-spec", "role": "termmin", "path": p.display().to_string(),
                           "blake3": blake3_file(p)}),
        );
    }
    json!({
        "schema": "rustred.campaign.spec.v1", "campaign": campaign,
        "programs": {"rr_fake_child": {"path": fake}},
        "resources": {"policy": "plain", "cpus": cpus, "containment": "marker_only", "max_jobs": 4, "mem_total": "2G", "disk_floor": "1G",
                      "grace_seconds": 5, "sampler_ms": 300, "host_floor": "100M", "heartbeat_timeout_s": 60,
                      "start_timeout_s": 120},
        "progress": {"cadence_seconds": 1},
        "inputs": inputs
    })
}
