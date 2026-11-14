#![allow(dead_code)]
//! Test data of the closeloop tests: a TOY node (arity 3, chart 110: two lines, one numerator; nothing five-loop
//! specific) whose start set needs several rounds of recipe solves, a per-face recipe set, a chart-wide default recipe
//! (R49), a fake-child oracle (the per-key answers rr_fake_child gives), a hand statement, and the campaign spec.

use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub const HEADER: &str =
    r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":3,"mask":"110"}"#;

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

/// A rule line over the toy node.
pub fn rule(index: i64, fixed: [Option<i64>; 3], rhs: &[&str]) -> String {
    let t: Vec<String> = (0..3)
        .map(|i| fixed[i].map(|v| v.to_string()).unwrap_or(format!("n{i}")))
        .collect();
    json!({"type":"rule","chart":"110","index":index,"fixed":fixed,"equations":[],
        "target": format!("I({})", t.join(",")),
        "rhs": rhs.iter().map(|k| json!([k, "1"])).collect::<Vec<_>>(),
        "exceptions":[],"dispatch":"Partition","witness_loci":[]})
    .to_string()
}

pub fn write_table(dir: &Path, name: &str, rules: &[String]) -> PathBuf {
    let p = dir.join(name);
    let mut s = String::from(HEADER);
    s.push('\n');
    s.push_str(r#"{"type":"chart","chart":"110","guards":["n>=1","n>=1","n<=0"],"zero":false}"#);
    s.push('\n');
    for r in rules {
        s.push_str(r);
        s.push('\n');
    }
    s.push_str(r#"{"type":"end","charts":1}"#);
    s.push('\n');
    std::fs::write(&p, s).unwrap();
    p
}

pub fn recipe(dir: &Path, name: &str, fixed: [Option<i64>; 3]) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(
        &p,
        json!({"schema":"rustred.recipe.v1","id": name,"arity":3,"chart":"110",
            "domain":{"fixed":fixed,"equations":[],"exceptions":[]},
            "seeds":{"unit":"harder","centres":[],"generator":{"shells":["S0","S1"]}},
            "symmetry_rows":false,"elimination":{"convention":"rustred.recipe-elim.v1"}})
        .to_string(),
    )
    .unwrap();
    p
}

/// An oracle override: (recipe source, key, verdict, rhs keys).
pub type Override = (String, [i32; 3], String, Vec<[i32; 3]>);

/// What a toy test varies.
#[derive(Clone)]
pub struct Toy {
    /// Length of the n1 = 1 chain below the start key (keys [top..1, 1, 0]).
    pub top: i32,
    /// Also start at [k2, 2, 0] (a second face R2, whose answers reach into face R1).
    pub second_face: Option<i32>,
    pub default_recipe: bool,
    /// Face R1 covers n0 >= 2 only (so [1,1,0] is first met while closing and needs the default recipe, R49).
    pub r1_from_two: bool,
    /// Oracle overrides: (source, key) -> (verdict, rhs).
    pub overrides: Vec<Override>,
    pub quirks: Vec<String>,
    pub ms: u64,
    /// Pin an engine digest in the import spec for R1 (a wrong one makes every answer of R1 a lie).
    pub r1_engine_digest: Option<String>,
    pub max_keys_per_job: usize,
    /// Give the table rule its fact + per-key proof (else the node cannot close: Tier C, per-key required).
    pub table_evidence: bool,
}

impl Default for Toy {
    fn default() -> Self {
        Toy {
            top: 4,
            second_face: Some(3),
            default_recipe: true,
            r1_from_two: true,
            overrides: vec![],
            quirks: vec![],
            ms: 20,
            r1_engine_digest: None,
            max_keys_per_job: 2,
            table_evidence: true,
        }
    }
}

/// The files of one toy campaign.
pub struct ToyFiles {
    pub dir: PathBuf,
    pub config: PathBuf,
    pub import_spec: PathBuf,
    pub hand: PathBuf,
}

/// Build the toy material and the closeloop config in `dir`.
pub fn toy(dir: &Path, t: &Toy, fake: &str) -> ToyFiles {
    std::fs::create_dir_all(dir).unwrap();
    // table A: the start key [top+1, 1, 0] -> [top, 1, 0] (+ a hand-down); nothing else is covered by tables
    let start_top = t.top + 1;
    let a_rule = rule(
        0,
        [Some(start_top as i64), Some(1), Some(0)],
        &[
            &format!("I({},1,0)", t.top),
            &format!("I({},0,0)", start_top),
        ],
    );
    let a = write_table(dir, "A.table.jsonl", std::slice::from_ref(&a_rule));
    // the table rule's own evidence at the start key, bound to (record_sha256, key): a regularity fact and an exact
    // per-key proof (what the hand campaign produced with rr_proof)
    let a_sha = {
        let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
        h.update(a_rule.as_bytes());
        h.finish_hex()
    };
    let facts = dir.join("facts.jsonl");
    std::fs::write(
        &facts,
        json!({"schema":"rustred.proof-kernel.instantiate.v1","record_sha256":a_sha,"rule":0,
            "key":[start_top,1,0],"in_case":true,"chart_ok":true,"branch":null,"regular":"regular",
            "irregular_terms":[],"hidden_loci":[],
            "rhs":[{"term":0,"key":[t.top,1,0],"coef":"nonzero"},{"term":1,"key":[start_top,0,0],"coef":"nonzero"}],
            "zero_test":"exact","error":null})
        .to_string()
            + "\n",
    )
    .unwrap();
    let proofs = dir.join("proofs.json");
    std::fs::write(
        &proofs,
        json!({"schema":"rustred.proof-kernel.span.v1","keys":[
            {"record_sha256": a_sha, "key": [start_top,1,0], "status":"EXACT_IN_SPAN_AT_KEY"}]})
        .to_string(),
    )
    .unwrap();
    let r1 = recipe(dir, "R1.recipe.json", [None, Some(1), Some(0)]);
    let r2 = recipe(dir, "R2.recipe.json", [None, Some(2), Some(0)]);
    let dflt = recipe(dir, "default.recipe.json", [None, None, None]);
    let mut start = vec![[start_top, 1, 0]];
    if let Some(k2) = t.second_face {
        start.push([k2, 2, 0]);
    }
    let st = dir.join("start.jsonl");
    std::fs::write(
        &st,
        start
            .iter()
            .map(|k| format!("{}\n", json!(k)))
            .collect::<String>(),
    )
    .unwrap();
    let mut r1src = json!({"id":"R1","kind":"recipe","class":"recipe","path": r1.display().to_string(),
        "sha256": sha256_file(&r1)});
    if let Some(d) = &t.r1_engine_digest {
        r1src["engine_digest"] = json!(d);
    }
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "required_evidence":"per-key","policies":{"tier":"C"},
        "sources":[{"id":"A","kind":"engine_table","class":"base","path": a.display().to_string(),
                    "sha256": sha256_file(&a), "hand_name":{"template":"{source}#{label}"}},
                   r1src,
                   {"id":"R2","kind":"recipe","class":"recipe","path": r2.display().to_string()}],
        "evidence": if t.table_evidence {
            json!({"facts":[{"path": facts.display().to_string()}],"proofs":[{"path": proofs.display().to_string()}]})
        } else { json!({}) }});
    let import_spec = dir.join("import-spec.json");
    std::fs::write(&import_spec, serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
    // oracle: face R1 at [k,1,0] -> [k-1,1,0] + hand-down; face R2 at [k,2,0] -> [k-1,2,0] + [k,1,0];
    // the default recipe at [1,1,0] / [1,2,0] -> hand-downs only
    let mut oracle = vec![];
    let lo1 = if t.r1_from_two { 2 } else { 1 };
    for k in lo1..=t.top + 2 {
        let rhs = if k > 1 {
            vec![[k - 1, 1, 0], [k, 0, 0]]
        } else {
            vec![[1, 0, 0], [0, 1, 0]]
        };
        oracle.push(("R1".to_string(), [k, 1, 0], "SOLVED".to_string(), rhs));
    }
    for k in 1..=t.second_face.unwrap_or(0) + 1 {
        let rhs = if k > 1 {
            vec![[k - 1, 2, 0], [k, 1, 0]]
        } else {
            vec![[1, 1, 0], [0, 2, 0]]
        };
        oracle.push(("R2".to_string(), [k, 2, 0], "SOLVED".to_string(), rhs));
    }
    oracle.push((
        "default-recipe".to_string(),
        [1, 1, 0],
        "SOLVED".to_string(),
        vec![[1, 0, 0], [0, 1, 0]],
    ));
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
    // R1 restricted to n0 >= 2 when asked: its domain gets a bound
    if t.r1_from_two {
        std::fs::write(
            &r1,
            json!({"schema":"rustred.recipe.v1","id":"R1.recipe.json","arity":3,"chart":"110",
                "domain":{"fixed":[null,1,0],"equations":[],"exceptions":[],"bounds":{"min":[2,null,null],"max":[null,null,null]}},
                "seeds":{"unit":"harder","centres":[],"generator":{"shells":["S0","S1"]}},
                "symmetry_rows":false,"elimination":{"convention":"rustred.recipe-elim.v1"}})
            .to_string(),
        )
        .unwrap();
        // the sha256 in the import spec must follow the file
        let mut v: Value = serde_json::from_slice(&std::fs::read(&import_spec).unwrap()).unwrap();
        v["sources"][1]["sha256"] = json!(sha256_file(&r1));
        std::fs::write(&import_spec, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
    }
    let family = dir.join("TOY.family");
    std::fs::write(&family, "toy family (the fake child never reads it)\n").unwrap();
    let hand = dir.join("hand.jsonl");
    let config = dir.join("closeloop.json");
    let cfg = json!({"schema":"rustred.campaign.closeloop.v1","nodes":[{
        "node":"toy", "import_spec": import_spec.display().to_string(), "target":"boxed",
        "hand": {"path": hand.display().to_string(), "format": {"key":0,"in_box":1,"label":2,"rule":3}},
        "check_threads": 2,
        "default_recipe": if t.default_recipe { json!(dflt.display().to_string()) } else { Value::Null },
        "budgets": {"max_rounds": 20, "max_job_attempts": 2},
        "solve": {"program": "rr_fake_child", "family": family.display().to_string(), "order": "toy-order",
                  "max_keys_per_job": t.max_keys_per_job, "threads": 1, "cap_bytes": 268435456,
                  "expected_peak_bytes": 67108864, "heartbeat_s": 1, "stop_grace_s": 3,
                  "fake": {"oracle": op.display().to_string(), "ms": t.ms, "quirks": t.quirks, "program": "rr_fake_child"}}
    }]});
    std::fs::write(&config, serde_json::to_vec_pretty(&cfg).unwrap()).unwrap();
    let _ = fake;
    ToyFiles {
        dir: dir.to_path_buf(),
        config,
        import_spec,
        hand,
    }
}

/// The campaign spec of a toy closeloop campaign.
pub fn campaign_spec(campaign: &str, files: &ToyFiles, fake: &str, cpus: &str) -> Value {
    json!({
        "schema": "rustred.campaign.spec.v1", "campaign": campaign,
        "programs": {"rr_fake_child": {"path": fake}},
        "resources": {"policy": "plain", "cpus": cpus, "max_jobs": 4, "mem_total": "2G", "disk_floor": "1G",
                      "grace_seconds": 5, "sampler_ms": 300, "host_floor": "100M", "heartbeat_timeout_s": 60,
                      "start_timeout_s": 120},
        "progress": {"cadence_seconds": 1},
        "inputs": [{"id": "closeloop-config", "role": "closeloop", "path": files.config.display().to_string(),
                    "blake3": blake3_file(&files.config)}]
    })
}
