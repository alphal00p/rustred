//! Lane zerosfix3 (audit-r2-zeros F6 / F7): the disk-backed evidence store keeps the PRODUCER of every fact
//! (STORE_VERSION 3), so `Evidence::fact_producer` / `fact_basis` answer the same word in big-node mode as in memory;
//! two producers of the same fact give the earlier one of `FACT_PRODUCERS` in both modes and in any file order; a
//! confirmed basis is kept for store facts too; a store of another version is never read (`open` refuses it,
//! `open_or_build` rebuilds it). Toy node: arity 3, mask 110; nothing node-specific.

use rustred_campaign_core::closure::{CoefFact, FactSource, RuleId};
use rustred_campaign_core::import;

use std::path::{Path, PathBuf};

use import::evidence::{EvidenceOptions, FACT_PRODUCERS};
use import::store::{EvidenceStore, STORE_VERSION, StoreOptions};
use import::{Evidence, TableSetSpec, load};
use serde_json::json;

const HEADER: &str =
    r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":3,"mask":"110"}"#;
const CHART: &str =
    r#"{"type":"chart","chart":"110","guards":["n>=1","n>=1","n<=0"],"zero":false}"#;

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

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("zerosfix3-store-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Point records at [k,1,0] (k = 2..=6), each with terms [k-1,1,0] (non-zero) and [k,1,1] (zero).
fn record(k: i32) -> String {
    json!({"type":"rule","chart":"110","index":k,"fixed":[k,1,0],"equations":[],"target":format!("I({k},1,0)"),
        "rhs":[[format!("I({},1,0)", k - 1), "1"], [format!("I({k},1,1)"), "n2"]],
        "exceptions":[],"dispatch":"Partition","witness_loci":[]})
    .to_string()
}

fn compact(k: i32, producer: Option<&str>) -> String {
    let mut v = json!({"schema":"rustred.campaign.fact-verdict.v1","record_sha256": sha(record(k).as_bytes()),
        "index": k, "key": [k,1,0], "verdict":"regular", "nonzero":[digest(&[k - 1, 1, 0])], "unknown":[],
        "n_terms": 2});
    if let Some(p) = producer {
        v["producer"] = json!(p);
    }
    v.to_string()
}

fn instantiate(k: i32) -> String {
    json!({"schema":"rustred.proof-kernel.instantiate.v1","record_sha256": sha(record(k).as_bytes()), "rule": k,
        "key": [k,1,0], "in_case": true, "chart_ok": true, "branch": null, "regular": "regular",
        "irregular_terms": [], "hidden_loci": [],
        "rhs":[{"term":0,"key":[k - 1,1,0],"coef":"nonzero"},{"term":1,"key":[k,1,1],"coef":"zero"}],
        "zero_test":"exact","error":null})
    .to_string()
}

/// The material: table A with the five records; fact files `files` (name, lines) pinned in the spec, in that order.
fn spec(d: &Path, files: &[(&str, Vec<String>)]) -> TableSetSpec {
    let mut t = format!("{HEADER}\n{CHART}\n");
    for k in 2..=6 {
        t.push_str(&record(k));
        t.push('\n');
    }
    t.push_str("{\"type\":\"end\",\"charts\":1}\n");
    std::fs::write(d.join("A.table.jsonl"), &t).unwrap();
    std::fs::write(d.join("start.jsonl"), "[6,1,0]\n").unwrap();
    let mut facts = vec![];
    for (name, lines) in files {
        let body: String = lines.iter().map(|l| format!("{l}\n")).collect();
        std::fs::write(d.join(name), &body).unwrap();
        facts.push(
            json!({"path": d.join(name).display().to_string(), "sha256": sha(body.as_bytes()),
                          "family": "TOY"}),
        );
    }
    let s = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": d.join("start.jsonl").display().to_string(),"format":"keys_jsonl"},
        "sources":[{"id":"A","kind":"engine_table","class":"base","path": d.join("A.table.jsonl").display().to_string(),
                    "sha256": sha(t.as_bytes())},
                   {"id":"T","kind":"terminals","keys":[[1,1,0]]}],
        "evidence":{"facts": facts}});
    TableSetSpec::from_json(&s.to_string()).unwrap()
}

fn rid(set: &import::ImportedSet, k: i32) -> RuleId<'_> {
    let r = (k - 2) as usize;
    let i = &set.rules[0][r];
    RuleId {
        table: &i.table_name,
        record: r,
        label: i.label,
        digest: "",
    }
}

/// Every producer word, an unnamed compact producer, and duplicates (same fact, two producers, two files, both
/// orders): the store answers the in-memory producer and basis for every (record, key).
#[test]
fn the_store_answers_the_in_memory_producer_for_every_fact_and_merges_duplicates_alike() {
    let want = [
        (2, "rr_proof"),
        (3, "exact_eval"),
        (4, "rr_proof_elided"),
        (5, "compact"),
        (6, "exact_eval"),
    ];
    for order in [false, true] {
        let d = scratch(&format!("producers-{order}"));
        let mut f1 = vec![
            instantiate(2),
            compact(3, Some("exact_eval")),
            compact(4, Some("rr_proof_elided")),
            compact(5, None),
            compact(6, None),
        ];
        // the same fact of record 6 from a second producer, in a second file: the earlier of FACT_PRODUCERS wins
        let mut f2 = vec![
            compact(6, Some("exact_eval")),
            compact(5, Some("unknown-tool")),
        ];
        if order {
            std::mem::swap(&mut f1, &mut f2);
        }
        let s = spec(&d, &[("f1.jsonl", f1), ("f2.jsonl", f2)]);
        let set = load(&s, &d).unwrap();
        let mem = Evidence::from_spec(&set, &d).unwrap();
        let so = StoreOptions {
            dir: d.join("store"),
            budget_bytes: 1 << 20,
            threads: 1,
        };
        let mut sto =
            Evidence::from_spec_with(&set, &d, &EvidenceOptions { store: Some(so) }).unwrap();
        assert_eq!(sto.counts.stores.len(), 1);
        for (k, p) in want {
            let key = [k, 1, 0];
            assert_eq!(
                mem.fact_producer(&rid(&set, k), &key),
                Some(p),
                "memory {k} {order}"
            );
            assert_eq!(
                sto.fact_producer(&rid(&set, k), &key),
                Some(p),
                "store {k} {order}"
            );
            assert_eq!(
                sto.fact_basis(&rid(&set, k), &key),
                Some(p),
                "store basis {k}"
            );
            assert_eq!(
                sto.coefficient(&rid(&set, k), &key, 1, &[k, 1, 1]),
                CoefFact::Zero
            );
        }
        // a confirmation changes the BASIS of a store fact (as in memory), never its producer
        assert!(sto.confirm_basis(
            &set.rules[0][1].record_sha256,
            &[3, 1, 0],
            "exact_eval + rr_proof"
        ));
        assert_eq!(
            sto.fact_basis(&rid(&set, 3), &[3, 1, 0]),
            Some("exact_eval + rr_proof")
        );
        assert_eq!(
            sto.fact_producer(&rid(&set, 3), &[3, 1, 0]),
            Some("exact_eval")
        );
        assert!(!sto.confirm_basis(&set.rules[0][1].record_sha256, &[4, 1, 0], "x"));
        let _ = std::fs::remove_dir_all(&d);
    }
    assert_eq!(
        FACT_PRODUCERS,
        ["rr_proof", "exact_eval", "rr_proof_elided", "compact"]
    );
}

/// The same fact in two STORES (big-node mode: the node's answer files are chunked into stores) keeps the earlier
/// producer; DIFFERENT facts in two stores give an unusable fact and are reported (memory refuses them at load).
#[test]
fn facts_across_two_stores_merge_like_memory_and_contradictions_fail_closed() {
    let d = scratch("two-stores");
    let s = spec(
        &d,
        &[(
            "f1.jsonl",
            vec![compact(5, None), compact(6, Some("exact_eval"))],
        )],
    );
    let set = load(&s, &d).unwrap();
    let so = StoreOptions {
        dir: d.join("store"),
        budget_bytes: 1 << 20,
        threads: 1,
    };
    let mut ev = Evidence::from_spec_with(
        &set,
        &d,
        &EvidenceOptions {
            store: Some(so.clone()),
        },
    )
    .unwrap();
    // a second store: the same fact of record 5 from rr_proof (earlier producer), a CONTRADICTING fact of record 6
    let mut bad6: serde_json::Value = serde_json::from_str(&compact(6, Some("rr_proof"))).unwrap();
    bad6["nonzero"] = json!([digest(&[5, 1, 0]), digest(&[6, 1, 1])]);
    let body = format!("{}\n{bad6}\n", compact(5, Some("rr_proof")));
    std::fs::write(d.join("answers.jsonl"), &body).unwrap();
    let st = EvidenceStore::open_or_build(&set, &[(d.join("answers.jsonl"), None)], &so).unwrap();
    ev.attach_store(st);
    assert_eq!(
        ev.fact_producer(&rid(&set, 5), &[5, 1, 0]),
        Some("rr_proof")
    );
    assert_eq!(
        ev.coefficient(&rid(&set, 5), &[5, 1, 0], 1, &[5, 1, 1]),
        CoefFact::Zero
    );
    assert_eq!(
        ev.coefficient(&rid(&set, 6), &[6, 1, 0], 1, &[6, 1, 1]),
        CoefFact::Unknown
    );
    assert!(!ev.has_fact(&set, 0, 4, &[6, 1, 0]));
    assert_eq!(
        ev.fact_conflicts_used(),
        vec![(set.rules[0][4].record_sha256.clone(), vec![6, 1, 0])]
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// A store whose meta names another version is NEVER read as this version: `open` refuses it, `open_or_build`
/// rebuilds it in place (origin `built`) and the producers are right again.
#[test]
fn a_store_of_another_version_is_refused_by_open_and_rebuilt_by_open_or_build() {
    let d = scratch("version");
    let s = spec(&d, &[("f1.jsonl", vec![compact(3, Some("exact_eval"))])]);
    let set = load(&s, &d).unwrap();
    let so = StoreOptions {
        dir: d.join("store"),
        budget_bytes: 1 << 20,
        threads: 1,
    };
    let opts = EvidenceOptions {
        store: Some(so.clone()),
    };
    let ev = Evidence::from_spec_with(&set, &d, &opts).unwrap();
    assert_eq!(ev.counts.stores[0].origin, "built");
    let dir = PathBuf::from(&ev.counts.stores[0].dir);
    drop(ev);
    let ev = Evidence::from_spec_with(&set, &d, &opts).unwrap();
    assert!(ev.counts.stores[0].origin.starts_with("reused"));
    drop(ev);
    // an older build's meta (the entry layout differs: no producer code)
    let mp = dir.join("meta.json");
    let mut m: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&mp).unwrap()).unwrap();
    assert_eq!(m["version"], STORE_VERSION);
    m["version"] = json!(STORE_VERSION - 1);
    std::fs::write(&mp, serde_json::to_string(&m).unwrap()).unwrap();
    let e = EvidenceStore::open(&dir)
        .err()
        .expect("an old-version store is refused");
    assert!(e.to_string().contains("version"), "{e}");
    let ev = Evidence::from_spec_with(&set, &d, &opts).unwrap();
    assert_eq!(ev.counts.stores[0].origin, "built");
    assert_eq!(
        ev.fact_producer(&rid(&set, 3), &[3, 1, 0]),
        Some("exact_eval")
    );
    assert!(EvidenceStore::open(&dir).is_ok());
    let _ = std::fs::remove_dir_all(&d);
}

/// FNV-1a over the i32 LE bytes (the index form's `terms_fnv64`, as the store computes it).
fn fnv(vals: &[i32]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for v in vals {
        for b in v.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("{h:016x}")
}

fn index_form(k: i32, n_terms: u64, fnv64: &str) -> String {
    json!({"schema":"rustred.campaign.fact-verdict.v1","record_sha256": sha(record(k).as_bytes()), "index": k,
        "key": [k,1,0], "verdict":"regular", "n_terms": n_terms, "zero_idx":[1], "unknown_idx":[],
        "terms_fnv64": fnv64, "producer":"rr_proof"})
    .to_string()
}

/// The compact INDEX form (`zero_idx`, `terms_fnv64`) in MEMORY mode: read by index exactly as the store reads it (it
/// was read as "no non-zero term": every term zero); a wrong `terms_fnv64` or `n_terms` and a digest line with a wrong
/// `n_terms` are refused in both modes alike.
#[test]
fn the_index_form_and_term_count_checks_read_the_same_in_memory_and_in_the_store() {
    let d = scratch("index-form");
    let terms_of = |k: i32| [k - 1, 1, 0, k, 1, 1];
    let mut bad_n: serde_json::Value = serde_json::from_str(&compact(5, Some("rr_proof"))).unwrap();
    bad_n["n_terms"] = json!(3);
    let s = spec(
        &d,
        &[(
            "f1.jsonl",
            vec![
                index_form(2, 2, &fnv(&terms_of(2))),
                index_form(3, 2, &fnv(&[9, 9, 9])),
                index_form(4, 3, &fnv(&terms_of(4))),
                bad_n.to_string(),
                compact(6, Some("exact_eval")),
            ],
        )],
    );
    let set = load(&s, &d).unwrap();
    let mem = Evidence::from_spec(&set, &d).unwrap();
    let so = StoreOptions {
        dir: d.join("store"),
        budget_bytes: 1 << 20,
        threads: 1,
    };
    let sto = Evidence::from_spec_with(&set, &d, &EvidenceOptions { store: Some(so) }).unwrap();
    for (name, ev) in [("memory", &mem), ("store", &sto)] {
        // record 2: bound, term 0 non-zero, term 1 zero
        assert_eq!(
            ev.fact_producer(&rid(&set, 2), &[2, 1, 0]),
            Some("rr_proof"),
            "{name}"
        );
        assert_eq!(
            ev.coefficient(&rid(&set, 2), &[2, 1, 0], 0, &[1, 1, 0]),
            CoefFact::NonZero,
            "{name}"
        );
        assert_eq!(
            ev.coefficient(&rid(&set, 2), &[2, 1, 0], 1, &[2, 1, 1]),
            CoefFact::Zero,
            "{name}"
        );
        // records 3 (fnv), 4 (n_terms in the index form), 5 (n_terms in the digest form): refused, no fact
        for k in [3, 4, 5] {
            assert_eq!(
                ev.fact_producer(&rid(&set, k), &[k, 1, 0]),
                None,
                "{name} {k}"
            );
            assert_eq!(
                ev.coefficient(&rid(&set, k), &[k, 1, 0], 0, &[k - 1, 1, 0]),
                CoefFact::Unknown,
                "{name} {k}"
            );
        }
        assert_eq!(
            ev.fact_producer(&rid(&set, 6), &[6, 1, 0]),
            Some("exact_eval"),
            "{name}"
        );
    }
    assert_eq!(mem.counts.facts_terms_mismatch, 3);
    assert_eq!(sto.counts.stores[0].counts.terms_mismatch, 3);
    let _ = std::fs::remove_dir_all(&d);
}
