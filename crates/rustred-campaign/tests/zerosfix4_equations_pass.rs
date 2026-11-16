//! Lane zerosfix4 (the T162 box run of lane wireflow, round-2 line): a positive-numerator zero term whose numerator
//! vanishes on the plane ONLY through the record's case equation. The first route (rr_proof facts) says zero; the zeros
//! route answers `zero` by `plane:3=0`, backed by a structural proof that used the case equation (`"equations": [0]`).
//! Before lane zerosfix4 the core loader never counted such a proof: the word was unbound (`all_zeros_unbound`), the
//! pair disputed and the node open. Now the loader re-checks the claim (the equation solved for a unit parameter) and
//! the node closes, memory and big-node mode alike. A FALSE claim of the same form still blocks.
//! Pure (no process): `closeloop::pass::run_pass`. Toy record: arity 4, mask 1110, case equation n0 + 3 n1 - n2 + n3 = 0.

use rustred_campaign::closeloop::config::ScaleCfg;
use rustred_campaign::closeloop::pass::{PassIn, ProofPlan, run_pass};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const HEADER: &str =
    r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":4,"mask":"1110"}"#;
const CHART: &str =
    r#"{"type":"chart","chart":"1110","guards":["n>=1","n>=1","n>=1","n<=0"],"zero":false}"#;
const KEY: [i32; 4] = [1, 1, 4, 0];

fn sha(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

fn digest(k: &[i32]) -> String {
    format!(
        "{:016x}",
        rustred_campaign_core::import::lines::term_key_digest(k)
    )
}

fn b3(p: &Path) -> String {
    rustred_campaign::atomic::blake3_file(p).unwrap().1
}

/// The material; `numerator` = t1's numerator text (true zero on n3 = 0 through the equation, or not).
fn material(dir: &Path, numerator: &str) -> (String, Vec<(PathBuf, String)>) {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
    let rec = json!({"type":"rule","chart":"1110","index":0,"fixed":[null,null,null,null],
        "equations":["n0+3*n1-n2+n3"],"target":"I(n0,n1,n2,n3)",
        "rhs":[["I(n0,n1-1,n2,n3)","1"],["I(n0,n1,n2,n3+1)",format!("({numerator})/(d-4)")]],
        "exceptions":[],"dispatch":"Partition","witness_loci":[]})
    .to_string();
    let rsha = sha(rec.as_bytes());
    let t = format!("{HEADER}\n{CHART}\n{rec}\n{{\"type\":\"end\",\"charts\":1}}\n");
    std::fs::write(dir.join("A.table.jsonl"), &t).unwrap();
    // first route: t0 -> [1,0,4,0] (a hand-down) non-zero, t1 (positive numerator) ZERO
    let facts = format!(
        "{}\n",
        json!({"schema":"rustred.campaign.fact-verdict.v1","producer":"rr_proof","record_sha256": rsha,
            "index": 0, "key": KEY, "verdict": "regular", "nonzero": [digest(&[1, 0, 4, 0])], "unknown": [],
            "n_terms": 2})
    );
    std::fs::write(dir.join("facts.jsonl"), &facts).unwrap();
    let proofs = format!(
        "{}\n",
        json!({"record_sha256": rsha, "index": 0, "key": KEY, "verdict": "EXACT_IN_SPAN_AT_KEY"})
    );
    std::fs::write(dir.join("proofs.jsonl"), &proofs).unwrap();
    std::fs::write(dir.join("start.jsonl"), format!("{}\n", json!(KEY))).unwrap();
    // the zeros route: t1 zero by the plane n3 = 0, backed by a proof that used the case equation
    let z = dir.join("second.zeros2.jsonl");
    std::fs::write(
        &z,
        format!(
            "{}\n{}\n",
            json!({"schema": rustred_campaign::adapters::legacy::ZERO_VERDICT_SCHEMA, "producer": "exact_eval",
                "record_sha256": rsha, "rule": 0, "key": KEY, "terms": [[1, "zero", "plane:3=0"]]}),
            json!({"schema":"rustred.campaign.zero-proof.v1","record_sha256": rsha, "rule": 0, "term": 1,
                "locus": {"plane": [3, 0]}, "verdict": "zero_on_locus", "denominator": "nowhere_zero",
                "equations": [0], "method": "structural.v1", "tool": "rr_coef_facts@0.1.0"})
        ),
    )
    .unwrap();
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": dir.join("start.jsonl").display().to_string(),"format":"keys_jsonl"},
        "required_evidence":"imported","policies":{"tier":"C"},
        "sources":[{"id":"A","kind":"engine_table","class":"base","path": dir.join("A.table.jsonl").display().to_string(),
                    "sha256": sha(t.as_bytes())}],
        "evidence":{"facts":[{"path": dir.join("facts.jsonl").display().to_string(),
                              "sha256": sha(facts.as_bytes()), "family": "TOY"}],
                    "proofs":[{"path": dir.join("proofs.jsonl").display().to_string(),
                               "sha256": sha(proofs.as_bytes()), "family": "TOY"}]}})
    .to_string();
    (spec, vec![(z.clone(), b3(&z))])
}

fn run(
    dir: &Path,
    spec: &str,
    zeros2: &[(PathBuf, String)],
    big: bool,
) -> (String, Vec<String>, Value) {
    let rd = dir.join(if big { "big" } else { "mem" });
    std::fs::create_dir_all(&rd).unwrap();
    let out = run_pass(
        &PassIn {
            threads: 1,
            exact_tier: true,
            rows_path: rd.join("rows.jsonl"),
            scale: big.then(|| ScaleCfg {
                index_dir: Some(rd.join("index")),
                store_dir: Some(rd.join("store")),
                store_budget_mib: 16,
                store_threads: 1,
                store_chunk_files: 1,
                stream_keys_only: true,
                keys_only_first: true,
                require_pinned_evidence: true,
            }),
            proof: Some(ProofPlan {
                import_requests: false,
                relations: false,
                facts: true,
                max_keys_per_job: 64,
                max_fact_keys_per_job: 64,
                exact_keys_per_job: Some(64),
                max_fact_terms_per_job: None,
            }),
            zeros2_files: zeros2.to_vec(),
            ..PassIn::first("toy", spec.to_string(), dir.to_path_buf(), rd.join("final"))
        },
        None,
    )
    .unwrap();
    let (verdict, mut blockers) = match &out.final_ {
        Some(f) => (
            f.verdict.clone(),
            f.result["blockers"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
        ),
        None => ("none".to_string(), vec![]),
    };
    blockers.sort();
    (verdict, blockers, out.check)
}

fn root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("zerosfix4-eqpass-{tag}-{}", std::process::id()))
}

#[test]
fn a_zero_proved_through_the_case_equation_confirms_the_pair_and_the_node_closes() {
    let d = root("true");
    let (spec, z2) = material(&d, "n0+3*n1-n2");
    for big in [false, true] {
        let (verdict, blockers, check) = run(&d, &spec, &z2, big);
        let az = &check["second_route"]["all_zeros"];
        assert_eq!(verdict, "closed", "big {big}: {blockers:?} {az:#}");
        assert_eq!(az["unbound"], 0, "big {big}");
        assert_eq!(az["confirmed_pairs"], 1, "big {big}");
        assert_eq!(az["confirmed_by"]["structural"], 1, "big {big}");
        assert_eq!(az["proofs"]["proved"], 1, "big {big}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// The same shape with a numerator the equation does NOT kill ((n0+3*n1+n2) = 2 n2 on the locus): the first route's
/// "zero" is false, the claim is refused by the loader (never a proof), the word is unbound and the node does not close.
#[test]
fn a_false_claim_through_the_case_equation_never_confirms() {
    let d = root("false");
    let (spec, z2) = material(&d, "n0+3*n1+n2");
    for big in [false, true] {
        let (verdict, blockers, check) = run(&d, &spec, &z2, big);
        assert_ne!(verdict, "closed", "big {big}");
        assert!(
            blockers.iter().any(|b| b.starts_with("all_zeros_unbound:")),
            "big {big}: {blockers:?} {:#}",
            check["second_route"]["all_zeros"]
        );
    }
    let _ = std::fs::remove_dir_all(&d);
}
