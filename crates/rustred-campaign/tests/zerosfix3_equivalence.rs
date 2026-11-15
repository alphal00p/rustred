//! Lane zerosfix3 (audit-r2-zeros F6 / F7, brief item 4): EQUIVALENCE of the closing loop's pass in MEMORY mode and in
//! BIG-NODE mode (`scale`: the spec's facts / proofs and the node's own fact answers live in disk-backed evidence
//! stores). This is the differential test that would have caught F6: the store dropped the PRODUCER of every fact
//! (basis `store`), so an elided pair got no second route and a false first-route zero closed the node.
//!
//! Seeded random small tables on a toy node (the two-loop sunset family: arity 5, lines n0 n1 n2, numerators n3 n4;
//! its symmetry group VERIFIED from the family text; nothing five-loop specific): point records with decisive and
//! non-decisive zero terms, ELIDED records (first route = the exact evaluation, served with a per-key proof),
//! POSITIVE-NUMERATOR and hand-down terms, SYMMETRY-IMAGE credits, a RELATION-SET credit, terminals; true zeros, FALSE
//! first-route zeros, missing facts, duplicate facts from two producers; second-route answers (closeloop4's
//! term-by-term route and the zeros route) present / missing / issued-unanswered / dependent. Each instance runs in
//! memory mode and in big-node mode (keys-only first; and exact tier at once, compared when the memory run reached
//! the exact tier): SAME verdict, SAME blocker words, SAME second-route statuses, SAME statement rows (fact basis
//! included), SAME decisive-zero table, SAME new batches.
//!
//! Pure (no process): `closeloop::pass::run_pass` only.

use rustred_campaign::closeloop::config::{AllZeros, ScaleCfg, SecondRouteCfg};
use rustred_campaign::closeloop::pass::{PassIn, ProofPlan, run_pass};
use rustred_campaign_core::import::{TableSetSpec, load};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const N: usize = 5;
const MASK: &str = "11100";
const FAMILY: &str = "I(\n  name(S),\n  loops(k1,k2),\n  externals(q),\n  dimension(d),\n  prop(D1,(k1)^2,1),\n  prop(D2,(k2)^2,1),\n  prop(D3,(k1+k2+q)^2,1),\n  prop(D4,(k1+q)^2,0),\n  prop(D5,(k2+q)^2,0),\n  gram(q,q,1)\n)\n";
const SEEDS: u64 = 128;

type Key = [i32; N];

/// splitmix64: a fixed stream per seed (no external crate).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    fn chance(&mut self, p: f64) -> bool {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64 <= p
    }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[self.below(v.len())]
    }
}

fn sha(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

fn kt(k: &[i32]) -> String {
    format!(
        "I({})",
        k.iter().map(i32::to_string).collect::<Vec<_>>().join(",")
    )
}

/// The term-key digest of the compact fact lines (core `import::lines::term_key_digest`).
fn digest(k: &[i32]) -> String {
    format!(
        "{:016x}",
        rustred_campaign_core::import::lines::term_key_digest(k)
    )
}

fn b3(p: &Path) -> String {
    rustred_campaign::atomic::blake3_file(p).unwrap().1
}

fn header() -> String {
    json!({"type":"header","schema":"table_dump.v1","family":"S","denominators":N,"mask":MASK,
        "family_source":FAMILY})
    .to_string()
}

fn table_text(rules: &[String]) -> String {
    let mut s = header();
    s.push('\n');
    s.push_str(&json!({"type":"chart","chart":MASK,"guards":["n>=1","n>=1","n>=1","n<=0","n<=0"],"zero":false}).to_string());
    s.push('\n');
    for r in rules {
        s.push_str(r);
        s.push('\n');
    }
    s.push_str(&format!("{}\n", json!({"type":"end","charts":1})));
    s
}

fn point_rule(index: usize, key: &Key, terms: &[Key], elided: bool) -> String {
    json!({"type":"rule","chart":MASK,"index":index,"fixed":key,"equations":[],"target":kt(key),
        "rhs": terms.iter().map(|k| json!([kt(k), "1"])).collect::<Vec<_>>(),
        "exceptions":[],"dispatch":"Partition",
        "witness_loci": if elided { json!(["elided"]) } else { json!([]) }})
    .to_string()
}

#[allow(dead_code)]
fn is_top(k: &Key) -> bool {
    k[..3].iter().all(|&x| x >= 1) && k[3..].iter().all(|&x| x <= 0)
}

/// The node comparator's `lower` (from a probe load: it depends on the frame only, never on the records).
struct Cmp(rustred_campaign_core::import::ImportedSet);
impl Cmp {
    fn probe(dir: &Path) -> Cmp {
        std::fs::create_dir_all(dir).unwrap();
        let t = table_text(&[point_rule(0, &[2, 1, 1, 0, 0], &[[1, 1, 1, 0, 0]], false)]);
        std::fs::write(dir.join("A.table.jsonl"), &t).unwrap();
        std::fs::write(dir.join("start.jsonl"), "[2,1,1,0,0]\n").unwrap();
        let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A",
            "start":{"path": dir.join("start.jsonl").display().to_string(),"format":"keys_jsonl"},
            "sources":[{"id":"A","kind":"engine_table","class":"base",
                        "path": dir.join("A.table.jsonl").display().to_string()}]});
        let s = TableSetSpec::from_json(&spec.to_string()).unwrap();
        Cmp(load(&s, Path::new("/")).unwrap())
    }
    fn lower(&self, a: &Key, b: &Key) -> bool {
        self.0.cmp.lower(a, b)
    }
}

/// One generated record (a point record at `key`).
struct Rec {
    key: Key,
    terms: Vec<Key>,
    elided: bool,
    /// per term: the TRUE coefficient is identically zero at the key
    truth: Vec<bool>,
    sha: String,
}

/// The material of one instance and the pass inputs both modes share.
struct Inst {
    dir: PathBuf,
    spec: String,
    fact_files: Vec<(PathBuf, String)>,
    fact2_files: Vec<(PathBuf, String)>,
    zeros2_files: Vec<(PathBuf, String)>,
    issued_facts2: BTreeSet<(String, Vec<i32>)>,
    issued_zeros2: BTreeSet<(String, Vec<i32>)>,
    proof: Option<ProofPlan>,
    second_route: SecondRouteCfg,
    /// generator facts (coverage only)
    n_elided: usize,
    n_false_zero: usize,
    n_credit_keys: usize,
    relation_set: bool,
}

fn write_lines(p: &Path, lines: &[String]) {
    let mut s = String::new();
    for l in lines {
        s.push_str(l);
        s.push('\n');
    }
    std::fs::write(p, s).unwrap();
}

fn generate(seed: u64, cmp: &Cmp, root: &Path) -> Inst {
    let mut g = Rng(seed.wrapping_mul(0x2545_f491_4f6c_dd1d) ^ 0x5eed);
    let dir = root.join(format!("seed{seed}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let terminal: Key = [1, 1, 1, 0, 0];
    let mut universe: Vec<Key> = vec![];
    for a in 1..=3 {
        for b in 1..=3 {
            for c in 1..=3 {
                for d in [0, -1] {
                    universe.push([a, b, c, d, 0]);
                }
            }
        }
    }
    universe.retain(|k| *k != terminal);
    // records: a random subset of the universe
    let n_rec = 5 + g.below(8);
    let mut rkeys: Vec<Key> = vec![];
    while rkeys.len() < n_rec {
        let k = *g.pick(&universe);
        if !rkeys.contains(&k) {
            rkeys.push(k);
        }
    }
    rkeys.sort();
    let mut recs: Vec<Rec> = vec![];
    for k in &rkeys {
        // lower top keys: any (a zero term there is DECISIVE when nothing else reaches it) / covered ones (a record
        // or the terminal: a non-zero term there keeps the node closable)
        let lower: Vec<Key> = universe
            .iter()
            .chain(std::iter::once(&terminal))
            .filter(|t| cmp.lower(t, k))
            .copied()
            .collect();
        let covered: Vec<Key> = lower
            .iter()
            .filter(|t| rkeys.contains(t) || **t == terminal)
            .copied()
            .collect();
        let (mut terms, mut truth): (Vec<Key>, Vec<bool>) = (vec![], vec![]);
        for _ in 0..1 + g.below(4) {
            let r = g.below(100);
            let (t, zero): (Key, bool) = if r < 60 && !lower.is_empty() {
                let zero = g.chance(0.35);
                let t = if zero || covered.is_empty() || g.chance(0.1) {
                    *g.pick(&lower)
                } else {
                    *g.pick(&covered)
                };
                (t, zero)
            } else if r < 80 {
                // a hand-down (lower sector: line n0 removed)
                ([0, k[1], k[2], k[3], 0], g.chance(0.3))
            } else {
                // a POSITIVE-NUMERATOR term (never an edge; a non-zero one fails the rule's acceptance)
                ([k[0], k[1], k[2], 1, 0], g.chance(0.75))
            };
            if !terms.contains(&t) {
                terms.push(t);
                truth.push(zero);
            }
        }
        recs.push(Rec {
            key: *k,
            terms,
            elided: g.chance(0.25),
            truth,
            sha: String::new(),
        });
    }
    // symmetry-image credits: start keys WITHOUT a record whose line permutation is a record key
    let mut credit_keys: Vec<Key> = vec![];
    for r in &recs {
        if credit_keys.len() >= 2 || r.key[3] != 0 || !g.chance(0.5) {
            continue;
        }
        for p in [[1, 0, 2], [0, 2, 1], [2, 1, 0], [1, 2, 0], [2, 0, 1]] {
            let im: Key = [r.key[p[0]], r.key[p[1]], r.key[p[2]], 0, 0];
            if im != r.key && !rkeys.contains(&im) && im != terminal && !credit_keys.contains(&im) {
                credit_keys.push(im);
                break;
            }
        }
    }
    let rules: Vec<String> = recs
        .iter()
        .enumerate()
        .map(|(i, r)| point_rule(i, &r.key, &r.terms, r.elided))
        .collect();
    for (r, l) in recs.iter_mut().zip(&rules) {
        r.sha = sha(l.as_bytes());
    }
    let ta = table_text(&rules);
    std::fs::write(dir.join("A.table.jsonl"), &ta).unwrap();
    std::fs::write(dir.join("S.sym"), FAMILY).unwrap();
    // a RELATION-SET credit: a supplied (symmetry-class) one-term relation at a key without a record
    let mut sources = vec![
        json!({"id":"A","kind":"engine_table","class":"base","path": dir.join("A.table.jsonl").display().to_string(),
               "sha256": sha(ta.as_bytes())}),
        json!({"id":"G","kind":"symmetry_credit","family":{"path": dir.join("S.sym").display().to_string()}}),
    ];
    let mut start: Vec<Key> = recs
        .iter()
        .filter(|_| g.chance(0.6))
        .map(|r| r.key)
        .collect();
    if start.is_empty() {
        start.push(recs[0].key);
    }
    start.extend(credit_keys.iter().copied());
    let mut relation_set = false;
    if g.chance(0.5) {
        let free: Vec<Key> = universe
            .iter()
            .filter(|k| !rkeys.contains(k) && !credit_keys.contains(k))
            .copied()
            .collect();
        let x = *g.pick(&free);
        let to_cands: Vec<Key> = rkeys
            .iter()
            .chain(std::iter::once(&terminal))
            .filter(|t| cmp.lower(t, &x))
            .copied()
            .collect();
        if !to_cands.is_empty() {
            let to = *g.pick(&to_cands);
            let rel = json!({"type":"rule","chart":MASK,"index":0,"fixed":x,"equations":[],"target":kt(&x),
                "rhs":[[kt(&to), "1"]],"exceptions":[],"dispatch":"Point","kind":"symmetry-one-term",
                "witness_loci":[]})
            .to_string();
            std::fs::write(dir.join("R.table.jsonl"), table_text(&[rel])).unwrap();
            std::fs::write(dir.join("verify.json"), "{\"proved\":true}\n").unwrap();
            sources.push(
                json!({"id":"rel","kind":"supplied_relations","class":"symmetry",
                "path": dir.join("R.table.jsonl").display().to_string(),
                "verification":{"path": dir.join("verify.json").display().to_string()}}),
            );
            start.push(x);
            relation_set = true;
        }
    }
    sources.push(json!({"id":"T","kind":"terminals","keys":[terminal]}));
    start.sort();
    start.dedup();
    write_lines(
        &dir.join("start.jsonl"),
        &start
            .iter()
            .map(|k| json!(k).to_string())
            .collect::<Vec<_>>(),
    );

    // FIRST route: one fact per record at its key (or none), split over the spec's evidence and two node answer files
    let complete = g.chance(0.6);
    let honest = g.chance(0.5);
    let (mut spec_facts, mut node_a, mut node_b) = (vec![], vec![], vec![]);
    let mut proofs: Vec<String> = vec![];
    let mut facts2: Vec<String> = vec![];
    let mut zeros2: Vec<String> = vec![];
    let (mut issued_facts2, mut issued_zeros2) = (BTreeSet::new(), BTreeSet::new());
    let (mut n_elided, mut n_false_zero) = (0usize, 0usize);
    for (i, r) in recs.iter().enumerate() {
        n_elided += usize::from(r.elided);
        let prob_proof = if r.elided { 0.9 } else { 0.5 };
        if g.chance(prob_proof) {
            proofs.push(
                json!({"record_sha256": r.sha, "index": i, "key": r.key, "verdict": "EXACT_IN_SPAN_AT_KEY"})
                    .to_string(),
            );
        }
        if !complete && g.chance(0.2) {
            continue; // missing fact
        }
        // the first route's claimed words: the truth, except FALSE zeros
        let claimed: Vec<bool> = r
            .truth
            .iter()
            .map(|&z| z || (!honest && g.chance(0.2)))
            .collect();
        n_false_zero += claimed
            .iter()
            .zip(&r.truth)
            .filter(|(c, t)| **c && !**t)
            .count();
        let producer: &str = if r.elided {
            "exact_eval"
        } else {
            *g.pick(&["rr_proof_full", "rr_proof", "exact_eval", "compact"])
        };
        let verdict = if r.elided { "hidden" } else { "regular" };
        let compact_line = |producer: &str| -> String {
            let nonzero: Vec<String> = r
                .terms
                .iter()
                .zip(&claimed)
                .filter(|(_, z)| !**z)
                .map(|(t, _)| digest(t))
                .collect();
            let mut v = json!({"schema":"rustred.campaign.fact-verdict.v1","record_sha256": r.sha,
                "index": i, "key": r.key, "verdict": verdict, "nonzero": nonzero, "unknown": [],
                "n_terms": r.terms.len()});
            if producer != "compact" {
                v["producer"] = json!(producer);
            }
            v.to_string()
        };
        let line = if producer == "rr_proof_full" {
            json!({"schema":"rustred.proof-kernel.instantiate.v1","record_sha256": r.sha, "rule": i,
                "key": r.key, "in_case": true, "chart_ok": true, "branch": null, "regular": "regular",
                "irregular_terms": [], "hidden_loci": [],
                "rhs": r.terms.iter().zip(&claimed).enumerate()
                    .map(|(j, (t, z))| json!({"term": j, "key": t, "coef": if *z { "zero" } else { "nonzero" }}))
                    .collect::<Vec<_>>(),
                "zero_test": "exact", "error": null})
            .to_string()
        } else {
            compact_line(producer)
        };
        let first_producer = if producer == "rr_proof_full" {
            "rr_proof"
        } else {
            producer
        };
        let mut place = |l: String, g: &mut Rng| match g.below(3) {
            0 => spec_facts.push(l),
            1 => node_a.push(l),
            _ => node_b.push(l),
        };
        place(line, &mut g);
        // a DUPLICATE of the same fact from another (compact) producer, in another file
        if producer != "rr_proof_full" && g.chance(0.15) {
            let other = *g.pick(&["rr_proof", "exact_eval", "compact"]);
            place(compact_line(other), &mut g);
        }
        // the SECOND route (closeloop4's): the truth, from the producer independent of the first (or, rarely, not)
        if g.chance(0.8) {
            let indep = g.chance(0.95);
            let p2 = match (first_producer, indep, r.elided) {
                ("exact_eval", true, true) => "rr_proof_elided",
                ("exact_eval", true, false) => "rr_proof",
                (_, true, _) => "exact_eval",
                ("compact", false, _) => "exact_eval_dependent_probe",
                (p, false, _) => p,
            };
            // (a dependent probe for `compact` is written as a second compact line: no named producer)
            let p2 = if p2 == "exact_eval_dependent_probe" {
                "compact"
            } else {
                p2
            };
            let nonzero: Vec<String> = r
                .terms
                .iter()
                .zip(&r.truth)
                .filter(|(_, z)| !**z)
                .map(|(t, _)| digest(t))
                .collect();
            let v2 = if p2 == "rr_proof_elided" {
                "hidden"
            } else {
                verdict
            };
            let mut v = json!({"schema":"rustred.campaign.fact-verdict.v1","record_sha256": r.sha,
                "index": i, "key": r.key, "verdict": v2, "nonzero": nonzero, "unknown": [],
                "n_terms": r.terms.len()});
            if p2 != "compact" {
                v["producer"] = json!(p2);
            }
            facts2.push(v.to_string());
        } else if g.chance(0.25) {
            issued_facts2.insert((r.sha.clone(), r.key.to_vec()));
        }
        // the ZEROS route: every first-route zero term answered with its truth (decided at the key)
        if claimed.iter().any(|z| *z) {
            if g.chance(0.8) {
                let terms: Vec<Value> = claimed
                    .iter()
                    .enumerate()
                    .filter(|(_, z)| **z)
                    .map(|(j, _)| json!([j, if r.truth[j] { "zero" } else { "nonzero" }, "key"]))
                    .collect();
                zeros2.push(
                    json!({"schema": rustred_campaign::adapters::legacy::ZERO_VERDICT_SCHEMA,
                        "producer": "exact_eval", "record_sha256": r.sha, "rule": i, "key": r.key,
                        "terms": terms})
                    .to_string(),
                );
            } else if g.chance(0.25) {
                issued_zeros2.insert((r.sha.clone(), r.key.to_vec()));
            }
        }
    }
    write_lines(&dir.join("facts.jsonl"), &spec_facts);
    write_lines(&dir.join("proofs.jsonl"), &proofs);
    let pin = |name: &str| {
        let p = dir.join(name);
        json!({"path": p.display().to_string(), "sha256": sha(&std::fs::read(&p).unwrap()), "family": "S"})
    };
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A",
        "start":{"path": dir.join("start.jsonl").display().to_string(),"format":"keys_jsonl"},
        "required_evidence":"imported","policies":{"tier":"C","elided":"lift_with_per_key_proof"},
        "sources": sources,
        "evidence":{"facts":[pin("facts.jsonl")], "proofs":[pin("proofs.jsonl")]}})
    .to_string();
    let mut fact_files = vec![];
    for (name, lines) in [
        ("node_a.facts.jsonl", &node_a),
        ("node_b.facts.jsonl", &node_b),
    ] {
        if !lines.is_empty() {
            let p = dir.join(name);
            write_lines(&p, lines);
            fact_files.push((p.clone(), b3(&p)));
        }
    }
    let mut fact2_files = vec![];
    if !facts2.is_empty() {
        let p = dir.join("second.facts2.jsonl");
        write_lines(&p, &facts2);
        fact2_files.push((p.clone(), b3(&p)));
    }
    let mut zeros2_files = vec![];
    if !zeros2.is_empty() {
        let p = dir.join("second.zeros2.jsonl");
        write_lines(&p, &zeros2);
        zeros2_files.push((p.clone(), b3(&p)));
    }
    let proof = g.chance(0.8).then_some(ProofPlan {
        import_requests: false,
        relations: false,
        facts: true,
        max_keys_per_job: 64,
        max_fact_keys_per_job: 64,
        exact_keys_per_job: Some(64),
        max_fact_terms_per_job: None,
    });
    let all_zeros = match g.below(10) {
        0 | 1 => Some(AllZeros::Off),
        2 | 3 => Some(AllZeros::PerKey),
        _ => None,
    };
    Inst {
        dir,
        spec,
        fact_files,
        fact2_files,
        zeros2_files,
        issued_facts2,
        issued_zeros2,
        proof,
        second_route: SecondRouteCfg {
            all_zeros,
            ..SecondRouteCfg::default()
        },
        n_elided,
        n_false_zero,
        n_credit_keys: credit_keys.len(),
        relation_set,
    }
}

/// What both modes must agree on.
#[derive(Debug, PartialEq)]
struct Sum {
    verdict: String,
    blockers: Vec<String>,
    second_route: Value,
    fact_basis: Value,
    batches: Vec<String>,
    proof_batches: Vec<String>,
    statement_rows: Vec<String>,
    decisive_zeros: String,
    tier_c: bool,
}

fn second_route_fields(v: &Value) -> Value {
    let mut out = json!({});
    for k in [
        "decisive_zeros",
        "decisive_by_kind",
        "decisive_pairs",
        "elided_pairs",
        "pairs",
        "confirmed",
        "disagreements",
        "pending",
        "unanswered",
        "unavailable",
        "first_route_missing",
        "basis_unknown",
        "basis_unknown_words",
        "not_independent",
        "zero_terms",
        "compared_pairs",
    ] {
        out[k] = v[k].clone();
    }
    let az = &v["all_zeros"];
    let mut z = json!({});
    for k in [
        "mode",
        "pairs",
        "terms",
        "confirmed_pairs",
        "confirmed_terms",
        "confirmed_by",
        "confirmed_by_class",
        "disagreements",
        "pending",
        "unanswered",
        "incomplete",
        "unavailable",
        "unbound",
        "same_backend_to_rr_proof",
        "compared_by_route1",
        "basis_unknown",
    ] {
        z[k] = az[k].clone();
    }
    out["all_zeros"] = z;
    out["disagreement_list"] = v["disagreement_list"].clone();
    out
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Memory,
    BigKeysOnlyFirst,
    BigExactAtOnce,
}

fn run(inst: &Inst, mode: Mode, tag: &str) -> Sum {
    let rd = inst.dir.join(tag);
    let _ = std::fs::remove_dir_all(&rd);
    std::fs::create_dir_all(&rd).unwrap();
    let scale = (mode != Mode::Memory).then(|| ScaleCfg {
        index_dir: Some(rd.join("index")),
        store_dir: Some(rd.join("store")),
        store_budget_mib: 16,
        store_threads: 1,
        store_chunk_files: 1,
        stream_keys_only: true,
        keys_only_first: mode == Mode::BigKeysOnlyFirst,
        require_pinned_evidence: true,
    });
    let out = run_pass(
        &PassIn {
            threads: 2,
            exact_tier: true,
            rows_path: rd.join("rows.jsonl"),
            scale,
            proof: inst.proof.clone(),
            second_route: inst.second_route,
            fact_files: inst.fact_files.clone(),
            fact2_files: inst.fact2_files.clone(),
            zeros2_files: inst.zeros2_files.clone(),
            issued_facts2: inst.issued_facts2.clone(),
            issued_zeros2: inst.issued_zeros2.clone(),
            ..PassIn::first("toy", inst.spec.clone(), inst.dir.clone(), rd.join("final"))
        },
        None,
    )
    .unwrap_or_else(|e| panic!("{}: run_pass: {e}", inst.dir.display()));
    let (verdict, blockers) = match &out.final_ {
        Some(f) => {
            let mut b: Vec<String> = f.result["blockers"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            b.sort();
            (f.verdict.clone(), b)
        }
        None => ("none".to_string(), vec![]),
    };
    let read = |p: PathBuf| std::fs::read_to_string(p).unwrap_or_default();
    // (as a sorted set: a keys-only verdict lists its rows in walk order, streamed or in memory)
    let mut statement_rows: Vec<String> = read(rd.join("final/statement/statement.jsonl"))
        .lines()
        .filter(|l| !l.contains("\"type\":\"header\""))
        .map(str::to_string)
        .collect();
    statement_rows.sort();
    let mut pb: Vec<String> = out.proof_batches.iter().map(|b| format!("{b:?}")).collect();
    pb.sort();
    let mut sb: Vec<String> = out.batches.iter().map(|b| format!("{b:?}")).collect();
    sb.sort();
    Sum {
        verdict,
        blockers,
        second_route: second_route_fields(&out.check["second_route"]),
        fact_basis: out.check["fact_basis"].clone(),
        batches: sb,
        proof_batches: pb,
        statement_rows,
        decisive_zeros: read(rd.join("final/statement/decisive_zeros.jsonl")),
        tier_c: out.check["tier"] == "C" || out.check["second_route"].is_object(),
    }
}

/// The differential test (brief item 4).
#[test]
fn memory_mode_and_big_node_mode_give_the_same_verdicts_blockers_and_confirmations() {
    let root = std::env::temp_dir().join(format!("zerosfix3-equiv-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let cmp = Cmp::probe(&root.join("probe"));
    let mut cov: BTreeMap<&str, u64> = BTreeMap::new();
    for seed in 0..SEEDS {
        let inst = generate(seed, &cmp, &root);
        let mem = run(&inst, Mode::Memory, "mem");
        let big = run(&inst, Mode::BigKeysOnlyFirst, "big1");
        assert_eq!(
            mem,
            big,
            "seed {seed}: memory mode and big-node mode (keys-only first) differ ({})",
            inst.dir.display()
        );
        // with every fact present the memory run reaches the exact tier at once: so does big-node mode without the
        // keys-only pass (T266's setting `keys_only_first: false`)
        if mem.verdict != "none"
            && !mem
                .blockers
                .iter()
                .any(|b| b.starts_with("exact_tier_not_reached"))
        {
            let big2 = run(&inst, Mode::BigExactAtOnce, "big2");
            assert_eq!(
                (
                    &mem.verdict,
                    &mem.blockers,
                    &mem.second_route,
                    &mem.statement_rows,
                    &mem.decisive_zeros
                ),
                (
                    &big2.verdict,
                    &big2.blockers,
                    &big2.second_route,
                    &big2.statement_rows,
                    &big2.decisive_zeros
                ),
                "seed {seed}: memory mode and big-node mode (exact tier at once) differ ({})",
                inst.dir.display()
            );
            *cov.entry("compared_exact_at_once").or_default() += 1;
        }
        // coverage of the generated situations (summed over the seeds)
        let sr = &mem.second_route;
        let n = |v: &Value| v.as_u64().unwrap_or(0);
        *cov.entry(match mem.verdict.as_str() {
            "closed" => "verdict_closed",
            "none" => "verdict_none_more_to_do",
            _ => "verdict_open",
        })
        .or_default() += 1;
        *cov.entry("decisive_pairs").or_default() += n(&sr["decisive_pairs"]);
        *cov.entry("elided_pairs").or_default() += n(&sr["elided_pairs"]);
        *cov.entry("closeloop4_confirmed").or_default() += n(&sr["confirmed"]);
        *cov.entry("disagreements").or_default() += n(&sr["disagreements"]);
        *cov.entry("not_independent").or_default() += n(&sr["not_independent"]);
        *cov.entry("pending").or_default() += n(&sr["pending"]);
        *cov.entry("unanswered").or_default() += n(&sr["unanswered"]);
        *cov.entry("zeros_pairs").or_default() += n(&sr["all_zeros"]["pairs"]);
        *cov.entry("zeros_confirmed_pairs").or_default() += n(&sr["all_zeros"]["confirmed_pairs"]);
        *cov.entry("zeros_positive_numerator_confirmed").or_default() +=
            n(&sr["all_zeros"]["confirmed_by_class"]["positive_numerator"]);
        *cov.entry("same_backend_to_rr_proof").or_default() +=
            n(&sr["all_zeros"]["same_backend_to_rr_proof"]);
        *cov.entry("credit_rows").or_default() += mem
            .statement_rows
            .iter()
            .filter(|l| l.contains("symmetry_credit"))
            .count() as u64;
        *cov.entry("relation_rows").or_default() += mem
            .statement_rows
            .iter()
            .filter(|l| l.contains("\"rel\""))
            .count() as u64;
        *cov.entry("terminal_rows").or_default() += mem
            .statement_rows
            .iter()
            .filter(|l| l.contains("\"terminal\""))
            .count() as u64;
        *cov.entry("fact_batches").or_default() += mem.proof_batches.len() as u64;
        *cov.entry("gen_elided_records").or_default() += inst.n_elided as u64;
        *cov.entry("gen_false_zeros").or_default() += inst.n_false_zero as u64;
        *cov.entry("gen_credit_keys").or_default() += inst.n_credit_keys as u64;
        *cov.entry("gen_relation_sets").or_default() += u64::from(inst.relation_set);
        let _ = std::fs::remove_dir_all(&inst.dir);
    }
    println!("coverage over {SEEDS} seeds: {cov:#?}");
    for k in [
        "verdict_closed",
        "verdict_open",
        "verdict_none_more_to_do",
        "decisive_pairs",
        "elided_pairs",
        "closeloop4_confirmed",
        "disagreements",
        "not_independent",
        "zeros_confirmed_pairs",
        "zeros_positive_numerator_confirmed",
        "same_backend_to_rr_proof",
        "credit_rows",
        "relation_rows",
        "terminal_rows",
        "compared_exact_at_once",
    ] {
        assert!(
            cov.get(k).copied().unwrap_or(0) > 0,
            "the generator never produced `{k}`: {cov:#?}"
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}
