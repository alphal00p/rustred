//! Lane zerosfix3 (audit-r2-zeros F6 / F7, brief items 3 and 4): FAIL CLOSED second-route selection and the layers x
//! zeros wiring, memory mode and BIG-NODE mode (`scale`). Pure (no process): `closeloop::pass::run_pass` only.
//!
//! Toy node: arity 3, line mask 110 (lines n0, n1; numerator n2); one point record at [3,1,0] whose terms the cases
//! choose (a terminal [2,1,0]; a top key [2,2,0] nothing else reaches: a DECISIVE zero; positive-numerator keys);
//! the first route's compact fact names its producer; second-route answers (closeloop4's facts2 lines, the zeros
//! route's zero-verdict lines) state the truth. An UNKNOWN producer code is planted in the built evidence store
//! (data.bin, the entry's producer varint: what an entry of a build that knows more producers, or a damaged entry,
//! reads as): every pair that relies on it blocks with a typed word, elided, decisive and zero pairs alike.

use rustred_campaign::closeloop::config::{LayersCfg, ScaleCfg};
use rustred_campaign::closeloop::pass::{PassIn, ProofPlan, run_pass};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const HEADER: &str =
    r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":3,"mask":"110"}"#;
const CHART: &str =
    r#"{"type":"chart","chart":"110","guards":["n>=1","n>=1","n<=0"],"zero":false}"#;
const KEY: [i32; 3] = [3, 1, 0];

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

fn kt(k: &[i32]) -> String {
    format!(
        "I({})",
        k.iter().map(i32::to_string).collect::<Vec<_>>().join(",")
    )
}

/// One case: the record's terms (term key, TRUE zero), elided witness loci, the first route's producer (`compact` =
/// no producer named), the second-route answers present.
struct Case {
    terms: Vec<([i32; 3], bool)>,
    elided: bool,
    producer: &'static str,
    /// closeloop4's route: the producer of a facts2 line stating the truth (None = no answer)
    facts2: Option<&'static str>,
    /// the zeros route: a zero-verdict line stating the truth of every first-route zero term
    zeros2: bool,
}

struct Material {
    dir: PathBuf,
    spec: String,
    fact2_files: Vec<(PathBuf, String)>,
    zeros2_files: Vec<(PathBuf, String)>,
}

fn b3(p: &Path) -> String {
    rustred_campaign::atomic::blake3_file(p).unwrap().1
}

fn material(dir: &Path, c: &Case) -> Material {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
    let rec =
        json!({"type":"rule","chart":"110","index":0,"fixed":KEY,"equations":[],"target":kt(&KEY),
        "rhs": c.terms.iter().map(|(k, _)| json!([kt(k), "1"])).collect::<Vec<_>>(),
        "exceptions":[],"dispatch":"Partition",
        "witness_loci": if c.elided { json!(["elided"]) } else { json!([]) }})
        .to_string();
    let rsha = sha(rec.as_bytes());
    let t = format!("{HEADER}\n{CHART}\n{rec}\n{{\"type\":\"end\",\"charts\":1}}\n");
    std::fs::write(dir.join("A.table.jsonl"), &t).unwrap();
    let verdict = if c.elided { "hidden" } else { "regular" };
    let line = |producer: &str| {
        let nonzero: Vec<String> = c
            .terms
            .iter()
            .filter(|(_, z)| !z)
            .map(|(k, _)| digest(k))
            .collect();
        let mut v = json!({"schema":"rustred.campaign.fact-verdict.v1","record_sha256": rsha, "index": 0,
            "key": KEY, "verdict": if producer == "rr_proof_elided" { "hidden" } else { verdict },
            "nonzero": nonzero, "unknown": [], "n_terms": c.terms.len()});
        if producer != "compact" {
            v["producer"] = json!(producer);
        }
        v.to_string()
    };
    let facts = format!("{}\n", line(c.producer));
    std::fs::write(dir.join("facts.jsonl"), &facts).unwrap();
    let proofs = format!(
        "{}\n",
        json!({"record_sha256": rsha, "index": 0, "key": KEY, "verdict": "EXACT_IN_SPAN_AT_KEY"})
    );
    std::fs::write(dir.join("proofs.jsonl"), &proofs).unwrap();
    std::fs::write(dir.join("start.jsonl"), format!("{}\n", json!(KEY))).unwrap();
    let mut fact2_files = vec![];
    if let Some(p2) = c.facts2 {
        let p = dir.join("second.facts2.jsonl");
        std::fs::write(&p, format!("{}\n", line(p2))).unwrap();
        fact2_files.push((p.clone(), b3(&p)));
    }
    let mut zeros2_files = vec![];
    if c.zeros2 {
        let terms: Vec<Value> = c
            .terms
            .iter()
            .enumerate()
            .filter(|(_, (_, z))| *z)
            .map(|(j, _)| json!([j, "zero", "key"]))
            .collect();
        let p = dir.join("second.zeros2.jsonl");
        std::fs::write(
            &p,
            format!(
                "{}\n",
                json!({"schema": rustred_campaign::adapters::legacy::ZERO_VERDICT_SCHEMA,
                    "producer": "exact_eval", "record_sha256": rsha, "rule": 0, "key": KEY, "terms": terms})
            ),
        )
        .unwrap();
        zeros2_files.push((p.clone(), b3(&p)));
    }
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": dir.join("start.jsonl").display().to_string(),"format":"keys_jsonl"},
        "required_evidence":"imported","policies":{"tier":"C","elided":"lift_with_per_key_proof"},
        "sources":[{"id":"A","kind":"engine_table","class":"base","path": dir.join("A.table.jsonl").display().to_string(),
                    "sha256": sha(t.as_bytes())},
                   {"id":"T","kind":"terminals","keys":[[2,1,0]]}],
        "evidence":{"facts":[{"path": dir.join("facts.jsonl").display().to_string(),
                              "sha256": sha(facts.as_bytes()), "family": "TOY"}],
                    "proofs":[{"path": dir.join("proofs.jsonl").display().to_string(),
                               "sha256": sha(proofs.as_bytes()), "family": "TOY"}]}})
    .to_string();
    Material {
        dir: dir.to_path_buf(),
        spec,
        fact2_files,
        zeros2_files,
    }
}

struct Out {
    verdict: String,
    blockers: Vec<String>,
    check: Value,
}

fn run(m: &Material, tag: &str, big: bool, layers: bool) -> Out {
    run_with_store(m, tag, tag, big, layers)
}

/// One pass in run directory `tag`; big-node mode keeps its store (and index) in run directory `store_tag`.
fn run_with_store(m: &Material, tag: &str, store_tag: &str, big: bool, layers: bool) -> Out {
    let rd = m.dir.join(tag);
    std::fs::create_dir_all(&rd).unwrap();
    let sd = m.dir.join(store_tag);
    let out = run_pass(
        &PassIn {
            threads: 1,
            exact_tier: true,
            rows_path: rd.join("rows.jsonl"),
            scale: big.then(|| ScaleCfg {
                index_dir: Some(sd.join("index")),
                store_dir: Some(sd.join("store")),
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
            fact2_files: m.fact2_files.clone(),
            zeros2_files: m.zeros2_files.clone(),
            layers: layers.then(|| LayersCfg {
                order: "coarse".into(),
                list: vec![],
                freeze: true,
            }),
            ..PassIn::first("toy", m.spec.clone(), m.dir.clone(), rd.join("final"))
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
    Out {
        verdict,
        blockers,
        check: out.check,
    }
}

/// Plant producer code 127 (unknown to this build) into the store entry of (record 0, KEY) of the big-node run `tag`.
fn plant_unknown_producer(m: &Material, tag: &str) {
    let sd = m.dir.join(tag).join("store");
    let mut hits = 0;
    for e in std::fs::read_dir(&sd).unwrap().flatten() {
        let p = e.path().join("data.bin");
        let Ok(mut b) = std::fs::read(&p) else {
            continue;
        };
        // entry head: varint(record id 0), zigzag varints of KEY = (3, 1, 0) -> 6, 2, 0
        let head = [0u8, 6, 2, 0];
        let Some(i) = b.windows(4).position(|w| w == head) else {
            continue;
        };
        let flags = b[i + 4];
        if flags & 2 == 0 {
            continue; // no fact in this entry
        }
        // producer varint right after the flags byte (after the proof status varint when a proof is stored)
        let at = if flags & 1 != 0 { i + 6 } else { i + 5 };
        assert!(
            (1..=4).contains(&b[at]),
            "a known producer code before planting: {}",
            b[at]
        );
        b[at] = 127;
        std::fs::write(&p, &b).unwrap();
        hits += 1;
    }
    assert_eq!(hits, 1, "exactly one store entry patched");
}

fn root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("zerosfix3-fc-{tag}-{}", std::process::id()))
}

fn has(b: &[String], prefix: &str) -> bool {
    b.iter().any(|x| x.starts_with(prefix))
}

/// The three kinds of pair the closure relies on, each closable as built (memory = big-node), each BLOCKED in big-node
/// mode once its store entry carries a producer code no route knows.
#[test]
fn an_unknown_producer_code_blocks_elided_decisive_and_zero_pairs() {
    let cases: Vec<(&str, Case)> = vec![
        (
            "elided",
            Case {
                terms: vec![([2, 1, 0], false), ([3, 1, 1], true)],
                elided: true,
                producer: "exact_eval",
                facts2: Some("rr_proof_elided"),
                zeros2: false,
            },
        ),
        (
            "decisive",
            Case {
                terms: vec![([2, 1, 0], false), ([2, 2, 0], true)],
                elided: false,
                producer: "rr_proof",
                facts2: Some("exact_eval"),
                zeros2: false,
            },
        ),
        (
            "zeros",
            Case {
                terms: vec![([2, 1, 0], false), ([3, 1, 1], true)],
                elided: false,
                producer: "rr_proof",
                facts2: None,
                zeros2: true,
            },
        ),
    ];
    for (tag, c) in &cases {
        let m = material(&root(tag), c);
        let mem = run(&m, "mem", false, false);
        let big = run(&m, "big", true, false);
        assert_eq!(
            mem.verdict, "closed",
            "{tag}: {:?} {:#}",
            mem.blockers, mem.check["second_route"]
        );
        assert_eq!(
            (&big.verdict, &big.blockers),
            (&mem.verdict, &mem.blockers),
            "{tag}"
        );
        plant_unknown_producer(&m, "big");
        let bad = run(&m, "big", true, false);
        assert_ne!(
            bad.verdict, "closed",
            "{tag}: {:#}",
            bad.check["second_route"]
        );
        // with an answer of closeloop4's route the comparison exists but its producers are not independent; without
        // one no route is selected at all: both block by name
        let want = if c.facts2.is_some() {
            "second_route_not_independent:1"
        } else {
            "second_route_basis_unknown:1"
        };
        assert!(
            bad.blockers.iter().any(|b| b == want),
            "{tag}: {:?} {:#}",
            bad.blockers,
            bad.check["second_route"]
        );
        if c.facts2.is_none() {
            assert_eq!(
                bad.check["second_route"]["basis_unknown_words"],
                json!({"unknown_producer": 1}),
                "{tag}"
            );
        }
        let _ = std::fs::remove_dir_all(&m.dir);
    }
    // the same with NO second-route answer at all: the decisive and the elided pair get no route either
    for (tag, mut c) in cases.into_iter().take(2) {
        c.facts2 = None;
        let m = material(&root(&format!("{tag}-noanswer")), &c);
        let big = run(&m, "big", true, false);
        // (a second-route job is issued: no verdict yet)
        assert_eq!(big.verdict, "none", "{tag}: {:?}", big.blockers);
        assert_eq!(
            big.check["second_route"]["pending"], 1,
            "{tag}: {:#}",
            big.check["second_route"]
        );
        plant_unknown_producer(&m, "big");
        let bad = run(&m, "big", true, false);
        assert_ne!(bad.verdict, "closed");
        assert!(
            bad.blockers
                .iter()
                .any(|b| b == "second_route_basis_unknown:1")
                && !has(&bad.blockers, "second_route_pending"),
            "{tag}: {:?}",
            bad.blockers
        );
        let _ = std::fs::remove_dir_all(&m.dir);
    }
}

/// An agreement of two DEPENDENT producers (the exact evaluation confirming the exact evaluation) confirms nothing:
/// status `not_independent`, blocking, the fact basis stays the producer's; memory = big-node.
#[test]
fn an_agreement_of_dependent_producers_confirms_nothing() {
    let m = material(
        &root("dependent"),
        &Case {
            terms: vec![([2, 1, 0], false), ([3, 1, 1], true)],
            elided: true,
            producer: "exact_eval",
            facts2: Some("exact_eval"),
            zeros2: false,
        },
    );
    for (tag, big) in [("mem", false), ("big", true)] {
        let o = run(&m, tag, big, false);
        assert_ne!(o.verdict, "closed", "{tag}");
        assert!(
            o.blockers
                .iter()
                .any(|b| b == "second_route_not_independent:1"),
            "{tag}: {:?}",
            o.blockers
        );
        assert_eq!(o.check["second_route"]["confirmed"], 0, "{tag}");
        assert_eq!(o.check["fact_basis"], json!({"exact_eval": 1}), "{tag}");
        assert_eq!(
            o.check["second_route"]["disagreement_list"],
            json!([]),
            "{tag}"
        );
    }
    let _ = std::fs::remove_dir_all(&m.dir);
}

/// A pair closeloop4's route compared term by term (an earlier pass's decisive / elided pair) that is now only a pair
/// with a zero term keeps THAT comparison as its second route (int/7: sent to the zeros route, whose answers skip such
/// a pair: `all_zeros_pending` / `unanswered` for ever); memory = big-node.
#[test]
fn a_pair_compared_by_closeloop4s_route_keeps_that_comparison_for_its_zero_terms() {
    let m = material(
        &root("route1"),
        &Case {
            terms: vec![([2, 1, 0], false), ([3, 1, 1], true)],
            elided: false,
            producer: "rr_proof",
            facts2: Some("exact_eval"),
            zeros2: false,
        },
    );
    for (tag, big) in [("mem", false), ("big", true)] {
        let o = run(&m, tag, big, false);
        assert_eq!(
            o.verdict, "closed",
            "{tag}: {:?} {:#}",
            o.blockers, o.check["second_route"]
        );
        let az = &o.check["second_route"]["all_zeros"];
        assert_eq!(az["compared_by_route1"], 1, "{tag}: {az:#}");
        assert_eq!(az["pending"], 0, "{tag}");
        assert_eq!(o.check["second_route"]["confirmed"], 1, "{tag}");
        assert_eq!(
            o.check["fact_basis"],
            json!({"rr_proof + exact_eval": 1}),
            "{tag}"
        );
    }
    let _ = std::fs::remove_dir_all(&m.dir);
}

/// lane layers x lane zeros2 in BIG-NODE mode (int7 wire-in): a layer is frozen only when every served pair with a
/// first-route zero term is confirmed by its route (the zeros route; rr_proof for exact-evaluation facts); else its
/// verdict names `all_zeros_unconfirmed:N`. A producer code no route knows is never confirmed. Memory = big-node.
#[test]
fn layers_and_zeros_in_big_node_mode_freeze_only_confirmed_zero_pairs() {
    let layer = |o: &Out| o.check["layers"]["layers"][0].clone();
    let cases = [
        // (tag, producer, facts2, zeros2, frozen)
        ("rr-noans", "rr_proof", None, false, false),
        ("rr-zeros", "rr_proof", None, true, true),
        ("ex-noans", "exact_eval", None, false, false),
        // an exact-evaluation fact is never confirmed by the zeros route (same algebra)
        ("ex-zeros", "exact_eval", None, true, false),
        ("ex-rr", "exact_eval", Some("rr_proof"), false, true),
    ];
    for (tag, producer, facts2, zeros2, frozen) in cases {
        let m = material(
            &root(&format!("layers-{tag}")),
            &Case {
                terms: vec![([2, 1, 0], false), ([3, 1, 1], true)],
                elided: false,
                producer,
                facts2,
                zeros2,
            },
        );
        let mem = run(&m, "mem", false, true);
        let big = run(&m, "big", true, true);
        for (mode, o) in [("mem", &mem), ("big", &big)] {
            let l = layer(o);
            let why: Vec<String> = l["blocking"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect();
            assert_eq!(
                why.iter().any(|w| w == "all_zeros_unconfirmed:1"),
                !frozen,
                "{tag} {mode}: {l:#}"
            );
            assert_eq!(
                l["status"].as_str().unwrap_or("").contains("frozen"),
                frozen,
                "{tag} {mode}: {l:#}"
            );
            assert_eq!(
                o.verdict == "closed",
                frozen,
                "{tag} {mode}: {:?}",
                o.blockers
            );
        }
        assert_eq!(
            (&mem.verdict, &mem.blockers),
            (&big.verdict, &big.blockers),
            "{tag}"
        );
        assert_eq!(layer(&mem)["blocking"], layer(&big)["blocking"], "{tag}");
        if frozen {
            // the frozen layer's pair, its producer replaced by a code no route knows: not frozen, node blocked
            plant_unknown_producer(&m, "big");
            let bad = run_with_store(&m, "big-planted", "big", true, true);
            let l = layer(&bad);
            assert!(
                l["blocking"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|w| w == "all_zeros_unconfirmed:1"),
                "{tag}: {l:#}"
            );
            assert_ne!(bad.verdict, "closed", "{tag}");
        }
        let _ = std::fs::remove_dir_all(&m.dir);
    }
}
