//! Independent audit of lane zeros2 (audit-r2-zeros, 2026-11-15): FORGED zero-proof material presented to the core's
//! loader (`import::zero_proofs`), to the closing loop's accept path (`closeloop::zeros::ZerosAnswers` + `compare_zeros`)
//! and to the ship gate (`pieces::import_edges::gate_check_with` + `ZeroProofFacts`).
//!
//! lane zerosfix3: F1-F4 pass since that lane (un-ignored); F5 stays ignored (lane planfix).
//! Tests named `fN_*` are marked `#[ignore = "AUDIT-R2-ZEROS ..."]`: they assert the SPECIFIED behaviour
//! (interfaces/zero-proofs.md 3 "verified on load ... ONE semantics", the audit brief "every forgery must be refused")
//! and FAIL on int/7 d60f3839: they are the reproductions of the findings (run with `--include-ignored`).
//! Tests named `ok_*` are forgeries / edge cases that ARE refused or answered correctly today (they pass).
//! Toy material only (arity 3, line mask 110: lines n0, n1, numerator n2); nothing five-loop specific.

use rustred_campaign::closeloop::zeros::{ZerosAnswers, compare_zeros};
use rustred_campaign_core::closure::Tier;
use rustred_campaign_core::import::check::check;
use rustred_campaign_core::import::zero_proofs::{
    LoadOptions, ZERO_PROOF_SCHEMA, ZeroProofFacts, ZeroProofs, ZeroStatus,
};
use rustred_campaign_core::import::{CheckOptions, Evidence, ImportedSet, TableSetSpec, load};
use rustred_campaign_core::pieces::import_edges::{gate_check, gate_check_with};
use rustred_campaign_core::pieces::{GateConfig, GateVerdict};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const HEADER: &str =
    r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":3,"mask":"110"}"#;
const CHART: &str =
    r#"{"type":"chart","chart":"110","guards":["n>=1","n>=1","n<=0"],"zero":false}"#;
const VERDICT_SCHEMA: &str = "rustred.campaign.zero-verdict.v1";

fn sha(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("audit-r2-zeros-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

// ---------------------------------------------------------------------------------------------------------------------
// Material P: ONE parametric record (canonical target, whole chart) whose terms cover the cases of the brief.
//   t0 I(n0-1,n1,n2)    "1"                      an ordinary lower term
//   t1 I(n0,n1,n2+1)    "n2"                     positive numerator on the plane n2 = 0: TRUE zero there
//   t2 I(n0,n1-1,n2+1)  "1+n2"                   positive numerator on n2 = 0: truly NON-ZERO (= 1) there
//   t3 I(n0-1,n1,n2+1)  "(n2*(n0-3))/(n0-3)"     zero numerator on n2 = 0; denominator d-free, vanishes at n0 = 3:
//                                                where_regular; 0/0 at the key [3,1,0]
//   t4 I(n0,n1,n2+2)    "(n2*(n2+1))/(d-4)"      zero on the planes n2 = -1 and n2 = 0; denominator nowhere zero
// ---------------------------------------------------------------------------------------------------------------------

const K: [i32; 3] = [3, 1, 0]; // t3 is 0/0 here
const K2: [i32; 3] = [4, 1, 0]; // t3's denominator is 1 here

fn p_record() -> String {
    json!({"type":"rule","chart":"110","index":0,"fixed":[null,null,null],"equations":[],"target":"I(n0,n1,n2)",
        "rhs":[["I(n0-1,n1,n2)","1"],["I(n0,n1,n2+1)","n2"],["I(n0,n1-1,n2+1)","1+n2"],
               ["I(n0-1,n1,n2+1)","(n2*(n0-3))/(n0-3)"],["I(n0,n1,n2+2)","(n2*(n2+1))/(d-4)"]],
        "exceptions":[],"dispatch":"Partition","witness_loci":[]})
    .to_string()
}

fn term_keys_at(k: [i32; 3]) -> Vec<[i32; 3]> {
    let [a, b, c] = k;
    vec![
        [a - 1, b, c],
        [a, b, c + 1],
        [a, b - 1, c + 1],
        [a - 1, b, c + 1],
        [a, b, c + 2],
    ]
}

/// The set of material P, with first-route facts (rr_proof instantiate lines) at `keys`: `words[j]` per term.
fn p_material(dir: &Path, keys: &[[i32; 3]], words: &[&str; 5]) -> (ImportedSet, Evidence, String) {
    let rec = p_record();
    let t = format!("{HEADER}\n{CHART}\n{rec}\n{{\"type\":\"end\",\"charts\":1}}\n");
    let tp = dir.join("P.table.jsonl");
    std::fs::write(&tp, &t).unwrap();
    let rsha = sha(rec.as_bytes());
    let mut facts = String::new();
    for k in keys {
        let rhs: Vec<Value> = term_keys_at(*k)
            .iter()
            .enumerate()
            .map(|(j, tk)| json!({"term": j, "key": tk, "coef": words[j]}))
            .collect();
        facts.push_str(
            &json!({"schema":"rustred.proof-kernel.instantiate.v1","record_sha256": rsha, "rule": 0, "key": k,
                "in_case": true, "chart_ok": true, "branch": null, "regular": "regular", "irregular_terms": [],
                "hidden_loci": [], "rhs": rhs, "zero_test": "exact", "error": null})
            .to_string(),
        );
        facts.push('\n');
    }
    std::fs::write(dir.join("facts.jsonl"), &facts).unwrap();
    let start: String = keys
        .iter()
        .map(|k| format!("[{},{},{}]\n", k[0], k[1], k[2]))
        .collect();
    std::fs::write(dir.join("start.jsonl"), start).unwrap();
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"P","bits":6,
        "start":{"path": dir.join("start.jsonl").display().to_string(),"format":"keys_jsonl"},
        "required_evidence":"imported","policies":{"tier":"C"},
        "sources":[{"id":"P","kind":"engine_table","class":"base","path": tp.display().to_string(),
                    "sha256": sha(t.as_bytes())}],
        "evidence":{"facts":[{"path": dir.join("facts.jsonl").display().to_string(),
                              "sha256": sha(facts.as_bytes()), "family": "TOY"}]}});
    let spec = TableSetSpec::from_json(&spec.to_string()).unwrap();
    let set = load(&spec, Path::new("/")).unwrap();
    let ev = Evidence::from_spec(&set, Path::new("/")).unwrap();
    (set, ev, rsha)
}

fn proof(rsha: &str, term: u64, locus: Value, verdict: &str, den: Option<&str>) -> Value {
    let mut v = json!({"schema": ZERO_PROOF_SCHEMA, "record_sha256": rsha, "rule": 0, "term": term,
        "locus": locus, "verdict": verdict, "method": "structural.v1", "tool": "rr_coef_facts@0.1.0"});
    if let Some(d) = den {
        v["denominator"] = json!(d);
    }
    v
}

fn plane(p: u32, v: i64) -> Value {
    json!({"plane": [p, v]})
}

fn write_lines(dir: &Path, name: &str, lines: &[Value]) -> PathBuf {
    let p = dir.join(name);
    let mut s = String::new();
    for l in lines {
        s.push_str(&l.to_string());
        s.push('\n');
    }
    std::fs::write(&p, s).unwrap();
    p
}

/// Load `lines` as a proof file of material P; Ok(proofs) or the loader's refusal.
fn load_p(set: &ImportedSet, dir: &Path, lines: &[Value]) -> Result<ZeroProofs, String> {
    let p = write_lines(dir, "proofs.jsonl", lines);
    // (lane zerosfix3: the loader's refusals are typed; their text is compared here)
    ZeroProofs::load(set, &[p], &LoadOptions::default()).map_err(|e| e.to_string())
}

fn st(z: &ZeroProofs, set: &ImportedSet, term: usize, key: [i32; 3]) -> ZeroStatus {
    z.is_zero_term(set, 0, 0, term, &key)
}

// ---------------------------------------------------------------------------------------------------------------------
// The honest material answers with the ONE semantics (baseline).
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn ok_honest_proofs_answer_with_the_one_semantics() {
    let d = tmp("ok-honest");
    let (set, _, rsha) = p_material(
        &d,
        &[K, K2],
        &["nonzero", "zero", "nonzero", "unknown", "zero"],
    );
    let z = load_p(
        &set,
        &d,
        &[
            proof(&rsha, 1, plane(2, 0), "zero_on_locus", Some("nowhere_zero")),
            {
                let mut v = proof(
                    &rsha,
                    3,
                    plane(2, 0),
                    "zero_on_locus",
                    Some("where_regular"),
                );
                v["den"] = json!("(n0-3)");
                v
            },
            proof(&rsha, 4, plane(2, 0), "zero_on_locus", Some("nowhere_zero")),
            proof(
                &rsha,
                4,
                plane(2, -1),
                "zero_on_locus",
                Some("nowhere_zero"),
            ),
            proof(&rsha, 2, plane(2, 0), "not_proved", None),
        ],
    )
    .unwrap();
    assert_eq!(st(&z, &set, 1, K), ZeroStatus::Proved);
    assert_eq!(
        st(&z, &set, 3, K),
        ZeroStatus::DenominatorVanishes,
        "0/0 is not a proof"
    );
    assert_eq!(st(&z, &set, 3, K2), ZeroStatus::Proved);
    assert_eq!(st(&z, &set, 2, K), ZeroStatus::NotProved);
    assert_eq!(
        st(&z, &set, 1, [3, 1, -1]),
        ZeroStatus::NotProved,
        "plane n2 = 0 does not hold n2 = -1"
    );
    assert_eq!(st(&z, &set, 4, [3, 1, -1]), ZeroStatus::Proved);
    let _ = std::fs::remove_dir_all(&d);
}

// ---------------------------------------------------------------------------------------------------------------------
// Forgeries the loader REFUSES or does not bind (they pass today).
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn ok_loader_refuses_malformed_or_misbound_lines() {
    let d = tmp("ok-refuse");
    let (set, _, rsha) = p_material(&d, &[K], &["nonzero", "zero", "nonzero", "unknown", "zero"]);
    let good = proof(&rsha, 1, plane(2, 0), "zero_on_locus", Some("nowhere_zero"));
    // term index beyond the record's terms
    let mut v = good.clone();
    v["term"] = json!(5);
    assert!(load_p(&set, &d, &[v]).unwrap_err().contains("term 5"));
    // plane on a parameter beyond the arity
    let mut v = good.clone();
    v["locus"] = plane(3, 0);
    assert!(load_p(&set, &d, &[v]).unwrap_err().contains("parameter 3"));
    // a wrong rule echo / no echo
    let mut v = good.clone();
    v["rule"] = json!(7);
    assert!(load_p(&set, &d, &[v]).unwrap_err().contains("rule echo"));
    let mut v = good.clone();
    v["rule"] = Value::Null;
    assert!(load_p(&set, &d, &[v]).is_err());
    // an unknown verdict, a proved line without its denominator class, an unknown class
    let mut v = good.clone();
    v["verdict"] = json!("zero");
    assert!(load_p(&set, &d, &[v]).is_err());
    let mut v = good.clone();
    v.as_object_mut().unwrap().remove("denominator");
    assert!(load_p(&set, &d, &[v]).is_err());
    let mut v = good.clone();
    v["denominator"] = json!("never_zero");
    assert!(load_p(&set, &d, &[v]).is_err());
    // a malformed sha / locus
    let mut v = good.clone();
    v["record_sha256"] = json!(&rsha[..63]);
    assert!(load_p(&set, &d, &[v]).is_err());
    let mut v = good.clone();
    v["locus"] = json!({"plane": [2]});
    assert!(load_p(&set, &d, &[v]).is_err());
    // a producer outside the allow-list
    let p = write_lines(&d, "allow.jsonl", std::slice::from_ref(&good));
    let opts = LoadOptions {
        tools: Some(["rr_coef_facts@9.9.9".to_string()].into_iter().collect()),
    };
    assert!(
        ZeroProofs::load(&set, &[p], &opts)
            .unwrap_err()
            .to_string()
            .contains("not allowed")
    );
    // a file truncated INSIDE a line
    let s = format!("{good}\n");
    let p = d.join("trunc.jsonl");
    std::fs::write(&p, &s[..s.len() / 2]).unwrap();
    assert!(ZeroProofs::load(&set, &[p], &LoadOptions::default()).is_err());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn ok_misbound_or_partial_material_is_never_a_proof() {
    let d = tmp("ok-misbound");
    let (set, _, rsha) = p_material(
        &d,
        &[K, K2],
        &["nonzero", "zero", "nonzero", "unknown", "zero"],
    );
    // a proof RE-DIGESTED for a mutated record (t2's text "1+n2" -> "n2"): the record is not in the set (foreign)
    let mutated = p_record().replace("\"1+n2\"", "\"n2\"");
    let msha = sha(mutated.as_bytes());
    assert_ne!(msha, rsha);
    let mut v = proof(&msha, 2, plane(2, 0), "zero_on_locus", Some("nowhere_zero"));
    v["record_sha256"] = json!(msha);
    let z = load_p(&set, &d, &[v]).unwrap();
    assert_eq!(z.stats.foreign, 1);
    assert_eq!(st(&z, &set, 2, K), ZeroStatus::NotProved);
    // a proof on the WRONG plane value (n2 = -1) does not cover the key (n2 = 0)
    let z = load_p(
        &set,
        &d,
        &[proof(
            &rsha,
            1,
            plane(2, -1),
            "zero_on_locus",
            Some("nowhere_zero"),
        )],
    )
    .unwrap();
    assert_eq!(st(&z, &set, 1, K), ZeroStatus::NotProved);
    // where_regular without a den text / with an unreadable one: never a proof
    let z = load_p(
        &set,
        &d,
        &[proof(
            &rsha,
            3,
            plane(2, 0),
            "zero_on_locus",
            Some("where_regular"),
        )],
    )
    .unwrap();
    assert_eq!(st(&z, &set, 3, K2), ZeroStatus::NotProved);
    let mut v = proof(
        &rsha,
        3,
        plane(2, 0),
        "zero_on_locus",
        Some("where_regular"),
    );
    v["den"] = json!("(n0-");
    let z = load_p(&set, &d, &[v]).unwrap();
    assert_eq!(st(&z, &set, 3, K2), ZeroStatus::NotProved);
    // a file truncated at a line boundary: the dropped proofs are missing (fails closed)
    let z = load_p(
        &set,
        &d,
        &[proof(
            &rsha,
            4,
            plane(2, -1),
            "zero_on_locus",
            Some("nowhere_zero"),
        )],
    )
    .unwrap();
    assert_eq!(st(&z, &set, 4, K), ZeroStatus::NotProved);
    // not_proved / excluded lines never prove
    let z = load_p(
        &set,
        &d,
        &[
            proof(&rsha, 2, plane(2, 0), "not_proved", None),
            proof(&rsha, 2, plane(2, 0), "excluded", None),
        ],
    )
    .unwrap();
    assert_eq!(st(&z, &set, 2, K), ZeroStatus::NotProved);
    let _ = std::fs::remove_dir_all(&d);
}

// ---------------------------------------------------------------------------------------------------------------------
// Forgeries the loader ACCEPTS (findings).
// ---------------------------------------------------------------------------------------------------------------------

/// F1 (loader): a proved line for a term whose numerator does NOT vanish on the locus (t2: "1+n2" = 1 on n2 = 0) is
/// bound and answers `Proved`: the loader verifies the binding (sha, term range, parameter, echo, vocabulary) but never
/// the CLAIM, although the record's coefficient text is in the set (`set.bodies`).
#[test]
fn f1_loader_refuses_a_proof_whose_numerator_does_not_vanish() {
    let d = tmp("f1");
    let (set, _, rsha) = p_material(&d, &[K], &["nonzero", "zero", "nonzero", "unknown", "zero"]);
    let r = load_p(
        &set,
        &d,
        &[proof(
            &rsha,
            2,
            plane(2, 0),
            "zero_on_locus",
            Some("nowhere_zero"),
        )],
    );
    let ok = match r {
        Err(_) => true,
        Ok(z) => st(&z, &set, 2, K) != ZeroStatus::Proved,
    };
    let _ = std::fs::remove_dir_all(&d);
    assert!(
        ok,
        "a false zero (coefficient 1 at the key) is accepted as Proved"
    );
}

/// F1b (loader, same root): an honest proof of term 1 re-labelled as term 2 is accepted for term 2.
#[test]
fn f1b_loader_refuses_a_proof_rebound_to_another_term() {
    let d = tmp("f1b");
    let (set, _, rsha) = p_material(&d, &[K], &["nonzero", "zero", "nonzero", "unknown", "zero"]);
    let mut v = proof(&rsha, 1, plane(2, 0), "zero_on_locus", Some("nowhere_zero"));
    v["term"] = json!(2);
    let r = load_p(&set, &d, &[v]);
    let ok = match r {
        Err(_) => true,
        Ok(z) => st(&z, &set, 2, K) != ZeroStatus::Proved,
    };
    let _ = std::fs::remove_dir_all(&d);
    assert!(
        ok,
        "term 1's proof answers Proved for term 2 (coefficient 1)"
    );
}

/// F2 (loader): a `where_regular` line's `den` is taken from the PROOF LINE and never compared with the record's own
/// denominator text: `den = "1"` turns the 0/0 key [3,1,0] of t3 into `Proved` (the verifier compares the text, the
/// loader every consumer uses does not).
#[test]
fn f2_loader_checks_the_den_text_against_the_record() {
    let d = tmp("f2");
    let (set, _, rsha) = p_material(&d, &[K], &["nonzero", "zero", "nonzero", "unknown", "zero"]);
    let mut v = proof(
        &rsha,
        3,
        plane(2, 0),
        "zero_on_locus",
        Some("where_regular"),
    );
    v["den"] = json!("1");
    let r = load_p(&set, &d, &[v]);
    let ok = match r {
        Err(_) => true,
        Ok(z) => st(&z, &set, 3, K) != ZeroStatus::Proved,
    };
    let _ = std::fs::remove_dir_all(&d);
    assert!(
        ok,
        "0/0 at [3,1,0] answers Proved through a forged den text"
    );
}

/// F3 (loader): two lines for ONE (record, term, locus) that contradict each other are both accepted, and the
/// strongest wins: (a) `nowhere_zero` + `where_regular` -> the 0/0 key is `Proved`; (b) `zero_on_locus` +
/// `not_proved` -> `Proved`. (The verifier keeps only the LAST line per locus, so such a file can VERIFY.)
#[test]
fn f3_loader_refuses_contradicting_lines_for_one_locus() {
    let d = tmp("f3");
    let (set, _, rsha) = p_material(&d, &[K], &["nonzero", "zero", "nonzero", "unknown", "zero"]);
    let mut honest = proof(
        &rsha,
        3,
        plane(2, 0),
        "zero_on_locus",
        Some("where_regular"),
    );
    honest["den"] = json!("(n0-3)");
    let forged = proof(&rsha, 3, plane(2, 0), "zero_on_locus", Some("nowhere_zero"));
    let a = match load_p(&set, &d, &[forged, honest]) {
        Err(_) => true,
        Ok(z) => st(&z, &set, 3, K) != ZeroStatus::Proved,
    };
    let b = match load_p(
        &set,
        &d,
        &[
            proof(&rsha, 2, plane(2, 0), "zero_on_locus", Some("nowhere_zero")),
            proof(&rsha, 2, plane(2, 0), "not_proved", None),
        ],
    ) {
        Err(_) => true,
        Ok(z) => st(&z, &set, 2, K) != ZeroStatus::Proved,
    };
    let _ = std::fs::remove_dir_all(&d);
    assert!(
        a,
        "nowhere_zero + where_regular for one locus: the 0/0 key answers Proved"
    );
    assert!(
        b,
        "zero_on_locus + not_proved for one locus: answers Proved"
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// The closing loop's accept path (ZerosAnswers::load + bind + compare_zeros).
// ---------------------------------------------------------------------------------------------------------------------

fn verdict(rsha: &str, key: [i32; 3], terms: Value) -> Value {
    json!({"schema": VERDICT_SCHEMA, "producer": "exact_eval", "backend": "symbolica", "method": "zeros:structural",
        "record_sha256": rsha, "rule": 0, "index": 0, "key": key, "in_case": true, "branch": null, "n_terms": 5,
        "terms": terms, "seconds": 0.0})
}

fn answers(set: &ImportedSet, dir: &Path, lines: &[Value]) -> ZerosAnswers {
    let p = write_lines(dir, "zeros2.answer.jsonl", lines);
    let mut z = ZerosAnswers::default();
    z.load(set, &p).unwrap();
    z.bind(set);
    z
}

/// The honest answer of the zeros child at [3,1,0] (first route: t1, t3, t4 zero): t1 / t4 by a plane proof, t3
/// `unknown` (its where_regular proof's denominator vanishes at the key: `+den`) -> t3 DISPUTED; at [4,1,0] all agree.
#[test]
fn ok_accept_path_honest_answers_dispute_the_0_over_0_key_and_confirm_the_rest() {
    let d = tmp("ok-accept");
    let (set, ev, rsha) = p_material(
        &d,
        &[K, K2],
        &["nonzero", "zero", "nonzero", "zero", "zero"],
    );
    let mut wr = proof(
        &rsha,
        3,
        plane(2, 0),
        "zero_on_locus",
        Some("where_regular"),
    );
    wr["den"] = json!("(n0-3)");
    let z = answers(
        &set,
        &d,
        &[
            verdict(
                &rsha,
                K,
                json!([
                    [1, "zero", "plane:2=0"],
                    [3, "unknown", "plane:2=0+den"],
                    [4, "zero", "plane:2=0"]
                ]),
            ),
            verdict(
                &rsha,
                K2,
                json!([
                    [1, "zero", "plane:2=0"],
                    [3, "zero", "plane:2=0+den"],
                    [4, "zero", "plane:2=0"]
                ]),
            ),
            proof(&rsha, 1, plane(2, 0), "zero_on_locus", Some("nowhere_zero")),
            wr,
            proof(&rsha, 4, plane(2, 0), "zero_on_locus", Some("nowhere_zero")),
        ],
    );
    assert!(z.unbound.is_empty(), "{:?}", z.unbound);
    let c = compare_zeros(&set, &ev, &z, &rsha, &K).unwrap();
    assert_eq!(c.first_zero, 3);
    assert_eq!(c.differ.iter().map(|x| x.0).collect::<Vec<_>>(), vec![3]);
    let c2 = compare_zeros(&set, &ev, &z, &rsha, &K2).unwrap();
    assert!(c2.agree(), "{c2:?}");
    assert_eq!((c2.by_structural, c2.by_structural_den), (2, 1));
    let _ = std::fs::remove_dir_all(&d);
}

/// Structural words that no proof line backs become `unknown` (blocker all_zeros_unbound); a `nonzero` word or a
/// missing term never confirms.
#[test]
fn ok_accept_path_unbacked_contradicted_or_missing_words_never_confirm() {
    let d = tmp("ok-unbound");
    let (set, ev, rsha) = p_material(&d, &[K2], &["nonzero", "zero", "nonzero", "zero", "zero"]);
    let z = answers(
        &set,
        &d,
        &[verdict(
            &rsha,
            K2,
            json!([
                [1, "zero", "plane:2=0"],
                [3, "zero", "domain"],
                [4, "nonzero", "key"]
            ]),
        )],
    );
    assert_eq!(z.unbound.len(), 2, "{:?}", z.unbound);
    let c = compare_zeros(&set, &ev, &z, &rsha, &K2).unwrap();
    assert!(!c.agree());
    assert_eq!(c.differ.len(), 3, "{c:?}");
    // a proof of the right term on the WRONG plane does not back the word
    let z = answers(
        &set,
        &d,
        &[
            verdict(
                &rsha,
                K2,
                json!([
                    [1, "zero", "plane:2=-1"],
                    [3, "zero", "key"],
                    [4, "zero", "key"]
                ]),
            ),
            proof(
                &rsha,
                1,
                plane(2, -1),
                "zero_on_locus",
                Some("nowhere_zero"),
            ),
        ],
    );
    assert_eq!(
        z.unbound.len(),
        1,
        "the plane n2 = -1 does not hold the key's rule point"
    );
    // a missing term
    let z = answers(
        &set,
        &d,
        &[verdict(
            &rsha,
            K2,
            json!([[1, "zero", "key"], [4, "zero", "key"]]),
        )],
    );
    let c = compare_zeros(&set, &ev, &z, &rsha, &K2).unwrap();
    assert_eq!(c.missing, vec![3]);
    assert!(!c.agree());
    let _ = std::fs::remove_dir_all(&d);
}

/// F4 (closing loop): `bind` backs a structural word with ANY proved line of the same (term, locus) and ignores that
/// line's denominator check: a `zero` word WITHOUT `+den`, bound to the HONEST where_regular proof of t3 whose
/// denominator (n0 - 3) vanishes at [3,1,0], confirms a 0/0 first-route zero. The core's `status_at_point` (the ONE
/// semantics) answers DenominatorVanishes there; the accept path never asks it.
#[test]
fn f4_accept_path_applies_the_one_semantics_to_bound_proofs() {
    let d = tmp("f4");
    let (set, ev, rsha) = p_material(&d, &[K], &["nonzero", "zero", "nonzero", "zero", "zero"]);
    let mut wr = proof(
        &rsha,
        3,
        plane(2, 0),
        "zero_on_locus",
        Some("where_regular"),
    );
    wr["den"] = json!("(n0-3)");
    let z = answers(
        &set,
        &d,
        &[
            verdict(
                &rsha,
                K,
                json!([
                    [1, "zero", "plane:2=0"],
                    [3, "zero", "plane:2=0"],
                    [4, "zero", "plane:2=0"]
                ]),
            ),
            proof(&rsha, 1, plane(2, 0), "zero_on_locus", Some("nowhere_zero")),
            wr,
            proof(&rsha, 4, plane(2, 0), "zero_on_locus", Some("nowhere_zero")),
        ],
    );
    // the core's own lookup on the same proofs: not a proof at this key
    assert_eq!(
        z.proofs.is_zero_term(&set, 0, 0, 3, &K),
        ZeroStatus::DenominatorVanishes
    );
    let c = compare_zeros(&set, &ev, &z, &rsha, &K).unwrap();
    let _ = std::fs::remove_dir_all(&d);
    assert!(
        !c.agree(),
        "the 0/0 first-route zero of term 3 is CONFIRMED through a where_regular proof: {c:?}"
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// The ship gate with ZeroProofFacts (int7 wire-in B2): the proofs are the ONLY route there.
// ---------------------------------------------------------------------------------------------------------------------

/// F5 (ship gate, consequence of F1): int7_gate_zero_proofs' toy with the positive-numerator term's coefficient "1"
/// (truly non-zero): a forged proof line drops the term and the gate PASSES.
#[test]
#[ignore = "AUDIT-R2-ZEROS F5: ship gate + ZeroProofFacts drops a truly non-zero positive-numerator term on a forged proof"]
fn f5_ship_gate_keeps_a_nonzero_positive_numerator_term_despite_a_forged_proof() {
    let d = tmp("f5");
    let rule_line = |index: i64, k: i32, extra: bool| -> String {
        let mut rhs = vec![
            json!([format!("I({},1,0)", k - 1), "1"]),
            json!([format!("I({k},0,0)"), "1"]),
        ];
        if extra {
            rhs.push(json!([format!("I({k},1,1)"), "1"]));
        }
        json!({"type":"rule","chart":"110","index":index,"fixed":[k,1,0],"equations":[],
            "target": format!("I({k},1,0)"), "rhs": rhs, "exceptions":[],"dispatch":"Partition","witness_loci":[]})
        .to_string()
    };
    let lines = [rule_line(0, 3, true), rule_line(1, 2, false)];
    let mut t = format!("{HEADER}\n{CHART}\n");
    for l in &lines {
        t.push_str(l);
        t.push('\n');
    }
    t.push_str("{\"type\":\"end\",\"charts\":1}\n");
    let tp = d.join("A.table.jsonl");
    std::fs::write(&tp, &t).unwrap();
    std::fs::write(d.join("start.jsonl"), "[3,1,0]\n").unwrap();
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": d.join("start.jsonl").display().to_string(),"format":"keys_jsonl"},
        "required_evidence":"imported","policies":{"tier":"K","witness_check":"off_diagnostic"},
        "sources":[{"id":"A","kind":"engine_table","class":"base","path": tp.display().to_string(),
                    "sha256": sha(t.as_bytes())}]});
    let spec = TableSetSpec::from_json(&spec.to_string()).unwrap();
    let set = load(&spec, Path::new("/")).unwrap();
    let ev = Evidence::from_spec(&set, Path::new("/")).unwrap();
    let res = check(
        &set,
        &ev,
        &CheckOptions {
            threads: 1,
            tier: Some(Tier::K),
            max_rounds: None,
        },
    )
    .unwrap();
    let (r0, _) = gate_check(&set, &ev, &res, GateConfig::memory()).unwrap();
    assert_ne!(
        r0.verdict,
        GateVerdict::Pass,
        "without proofs the positive-numerator edge blocks the gate"
    );
    let forged = json!({"schema": ZERO_PROOF_SCHEMA, "record_sha256": sha(lines[0].as_bytes()), "rule": 0, "term": 2,
        "locus": "domain", "verdict": "zero_on_locus", "denominator": "nowhere_zero", "tool": "forger@1"});
    let pp = write_lines(&d, "forged.zero-proofs.jsonl", &[forged]);
    let verdict_with_forgery = match ZeroProofs::load(&set, &[pp], &LoadOptions::default()) {
        Err(_) => None,
        Ok(zp) => {
            let facts = ZeroProofFacts::new(&set, &zp, &ev);
            Some(
                gate_check_with(&set, &ev, &facts, &res, GateConfig::memory())
                    .unwrap()
                    .0
                    .verdict,
            )
        }
    };
    let _ = std::fs::remove_dir_all(&d);
    assert_ne!(
        verdict_with_forgery,
        Some(GateVerdict::Pass),
        "the gate passes on a forged proof of a coefficient that is the constant 1"
    );
}
