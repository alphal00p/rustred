//! Lane t266run: the disk-backed EVIDENCE STORE (interfaces/evidence-store.md) on a toy node (arity 3, nothing
//! node-specific). The check with the store must equal the check with the same evidence in memory (rows, levels,
//! bases, verdict, counts), for every fact form (instantiate.v1, compact digests, the new index form) and proof form;
//! results never depend on the sort budget or the thread count; reuse / rebuild; refusals (term-list mismatch,
//! conflicting facts, a wrong index echo).

use rustred_campaign_core::{closure, import};

use std::path::{Path, PathBuf};

use import::evidence::EvidenceOptions;
use import::store::{EvidenceStore, StoreOptions};
use import::{CheckOptions, Evidence, ImportError, RowStatus, TableSetSpec, check, load};
use serde_json::{Value, json};

/// lane closeloop4 (audit-closeloop M1 / m4): the spec text with its evidence PINNED (each file's sha256, a
/// SHA256SUMS manifest beside a directory) and the family of its tables named (spec evidence is read only so).
#[allow(dead_code)]
fn pin_ev(spec: &serde_json::Value) -> String {
    use std::io::BufRead;
    let mut v = spec.clone();
    let fam = v["sources"].as_array().and_then(|a| {
        a.iter().find_map(|s| {
            let p = s["path"].as_str()?;
            let f = std::fs::File::open(p).ok()?;
            let mut l = String::new();
            std::io::BufReader::new(f).read_line(&mut l).ok()?;
            let h: serde_json::Value = serde_json::from_str(l.trim()).ok()?;
            h["family"].as_str().map(str::to_string)
        })
    });
    let sha = |p: &std::path::Path| rustred_campaign_core::import::lines::file_sha256(p).unwrap();
    if let Some(ev) = v.get_mut("evidence").and_then(|e| e.as_object_mut()) {
        for k in ["facts", "proofs", "solved"] {
            if let Some(a) = ev.get_mut(k).and_then(|x| x.as_array_mut()) {
                for e in a {
                    let p = std::path::PathBuf::from(e["path"].as_str().unwrap_or(""));
                    if p.is_file() && e.get("sha256").is_none() {
                        e["sha256"] = serde_json::json!(sha(&p));
                    }
                    if p.is_dir() && e.get("manifest").is_none() {
                        let mut names: Vec<String> = std::fs::read_dir(&p)
                            .unwrap()
                            .flatten()
                            .map(|x| x.file_name().to_string_lossy().into_owned())
                            .filter(|n| n.ends_with(".json") || n.ends_with(".jsonl"))
                            .collect();
                        names.sort();
                        let body: String = names
                            .iter()
                            .map(|n| format!("{}  {n}\n", sha(&p.join(n))))
                            .collect();
                        let m = std::env::temp_dir().join(format!(
                            "pin-ev-{}-{}.SHA256SUMS",
                            std::process::id(),
                            rustred_campaign_core::closure::sha256::sha256_tag(
                                p.display().to_string().as_bytes()
                            )
                        ));
                        std::fs::write(&m, body).unwrap();
                        e["manifest"] =
                            serde_json::json!({"path": m.display().to_string(), "sha256": sha(&m)});
                    }
                    if e.get("family").is_none()
                        && let Some(f) = &fam
                    {
                        e["family"] = serde_json::json!(f);
                    }
                }
            }
        }
    }
    v.to_string()
}

const HEADER: &str =
    r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":3,"mask":"110"}"#;

fn scratch(tag: &str) -> PathBuf {
    let base = std::env::var_os("TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let d = base.join(format!("import-store-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn rule(index: i64, fixed: [Option<i64>; 3], rhs: &[&str], extra: Value) -> String {
    let t: Vec<String> = (0..3)
        .map(|i| fixed[i].map(|v| v.to_string()).unwrap_or(format!("n{i}")))
        .collect();
    let mut v = json!({"type":"rule","chart":"110","index":index,"fixed":fixed,"equations":[],
        "target": format!("I({})", t.join(",")),
        "rhs": rhs.iter().map(|k| json!([k, "1"])).collect::<Vec<_>>(),
        "exceptions":[],"dispatch":"Partition"});
    if let Value::Object(m) = extra {
        for (k, x) in m {
            v[k] = x;
        }
    }
    v.to_string()
}

fn write_table(dir: &Path, rules: &[String]) -> PathBuf {
    let p = dir.join("A.table.jsonl");
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

/// Toy: witness-ABSENT face rule 0 on I(n0,1,0), n0 >= 2 (no witness_loci field): -> I(n0-1,1,0) + I(n0,0,0) +
/// I(n0,1,1) (positive numerator: must be zero) + I(n0-1,0,0); point rule 1 at I(1,1,0) (complete witness) -> I(1,0,0).
fn toy(d: &Path, top: i32) -> (Value, PathBuf) {
    let a = write_table(
        d,
        &[
            rule(
                0,
                [None, Some(1), Some(0)],
                &["I(n0-1,1,0)", "I(n0,0,0)", "I(n0,1,1)", "I(n0-1,0,0)"],
                json!({"exceptions":[["n0-1"]]}),
            ),
            rule(
                1,
                [Some(1), Some(1), Some(0)],
                &["I(1,0,0)"],
                json!({"witness_loci": []}),
            ),
        ],
    );
    let st = d.join("start.jsonl");
    let s: String = (2..=top)
        .map(|n| format!("{}\n", json!([n, 1, 0])))
        .collect();
    std::fs::write(&st, s).unwrap();
    (
        json!({"schema":"rustred.campaign.import-spec.v1","frame_from":"A","bits":6,"required_evidence":"per-key",
        "start":{"path": st.display().to_string(),"format":"keys_jsonl"},
        "policies":{"tier":"C"},
        "sources":[{"id":"A","kind":"engine_table","class":"table","path":a.display().to_string()}]}),
        a,
    )
}

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

fn terms(n0: i32) -> Vec<[i32; 3]> {
    vec![[n0 - 1, 1, 0], [n0, 0, 0], [n0, 1, 1], [n0 - 1, 0, 0]]
}

/// The evidence of the toy: for each key n0 in 2..=top a fact in one of three forms (n0 mod 3) with the top term zero
/// for n0 = 0 mod 4 (an edge dropped: the keys below are reached only through other keys), the positive-numerator
/// term always zero; a proof for n0 != 5 (one key without proof -> below per-key); the point rule at I(1,1,0) gets a
/// proof too.
fn evidence_files(d: &Path, sha0: &str, sha1: &str, top: i32) -> (PathBuf, PathBuf) {
    use import::lines::term_key_digest;
    let hex = |k: [i32; 3]| format!("{:016x}", term_key_digest(&k));
    let mut facts = String::new();
    let mut proofs = String::new();
    for n0 in 2..=top {
        let t = terms(n0);
        let zero: Vec<usize> = (0..4)
            .filter(|&j| j == 2 || (j == 0 && n0 % 4 == 0))
            .collect();
        let line = match n0 % 3 {
            0 => {
                json!({"schema":"rustred.proof-kernel.instantiate.v1","record_sha256":sha0,"rule":0,"key":[n0,1,0],
                "in_case":true,"chart_ok":true,"branch":null,"regular":"regular","error":null,
                "rhs": t.iter().enumerate().map(|(j,k)| json!({"term":j,"key":k,
                    "coef": if zero.contains(&j) {"zero"} else {"nonzero"}})).collect::<Vec<_>>()})
            }
            1 => {
                json!({"schema":"rustred.campaign.fact-verdict.v1","index":0,"record_sha256":sha0,"key":[n0,1,0],
                "verdict":"regular","n_terms":4,
                "nonzero": (0..4).filter(|j| !zero.contains(j)).map(|j| hex(t[j])).collect::<Vec<_>>()})
            }
            _ => {
                let flat: Vec<i32> = t.iter().flatten().copied().collect();
                json!({"schema":"rustred.campaign.fact-verdict.v1","index":0,"record_sha256":sha0,"key":[n0,1,0],
                "verdict":"regular","n_terms":4,"zero_idx":zero,"unknown_idx":[],"terms_fnv64":fnv(&flat),
                "producer":"test"})
            }
        };
        facts.push_str(&format!("{line}\n"));
        if n0 != 5 {
            proofs.push_str(&format!(
                "{}\n",
                json!({"record_sha256":sha0,"key":[n0,1,0],"verdict":"EXACT_IN_SPAN_AT_KEY","index":0})
            ));
        }
    }
    proofs.push_str(&format!(
        "{}\n",
        json!({"record_sha256":sha1,"key":[1,1,0],"verdict":"EXACT_IN_SPAN_AT_KEY","index":1})
    ));
    // a foreign line and a second, weaker proof for a proved key (the strongest is kept)
    proofs.push_str(&format!(
        "{}\n",
        json!({"record_sha256":"ab".repeat(32),"key":[2,1,0],"verdict":"EXACT_IN_SPAN_AT_KEY"})
    ));
    proofs.push_str(&format!(
        "{}\n",
        json!({"record_sha256":sha0,"key":[2,1,0],"verdict":"NOT_DECIDED","index":0})
    ));
    let fp = d.join("facts.jsonl");
    let pp = d.join("proofs.jsonl");
    std::fs::write(&fp, facts).unwrap();
    std::fs::write(&pp, proofs).unwrap();
    (fp, pp)
}

type RowSig = (Vec<i32>, String, Option<String>, Option<String>, String);

fn sig(r: &import::CheckResult) -> (Vec<RowSig>, String, Vec<String>, u64) {
    let mut v: Vec<RowSig> = r
        .rows
        .iter()
        .map(|x| {
            (
                x.key.clone(),
                format!("{:?}", x.status),
                x.evidence.map(|l| l.word().to_string()),
                x.basis.map(str::to_string),
                format!("{:?}", x.server),
            )
        })
        .collect();
    v.sort();
    (
        v,
        format!("{:?}", r.summary.verdict),
        r.blocking(),
        r.summary.edges,
    )
}

fn checked(set: &import::ImportedSet, ev: &Evidence) -> import::CheckResult {
    check(
        set,
        ev,
        &CheckOptions {
            threads: 2,
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
fn store_check_equals_in_memory_check_for_every_form() {
    let d = scratch("eq");
    let top = 30;
    let (spec0, _a) = toy(&d, top);
    let s = TableSetSpec::from_json(&pin_ev(&spec0)).unwrap();
    let set = load(&s, &d).unwrap();
    let sha0 = set.rules[0][0].record_sha256.clone();
    let sha1 = set.rules[0][1].record_sha256.clone();
    let (fp, pp) = evidence_files(&d, &sha0, &sha1, top);
    let mut spec = spec0.clone();
    spec["evidence"] = json!({"facts":[{"path": fp.display().to_string()}],
                              "proofs":[{"path": pp.display().to_string()}]});
    let s = TableSetSpec::from_json(&pin_ev(&spec)).unwrap();
    let set = load(&s, &d).unwrap();
    // the in-memory evidence cannot read the index form (it is new): compare against an in-memory run whose index-form
    // lines are rewritten as compact digest lines (same facts)
    let mem_facts = d.join("facts.mem.jsonl");
    {
        use import::lines::term_key_digest;
        let mut out = String::new();
        for l in std::fs::read_to_string(&fp).unwrap().lines() {
            let mut v: Value = serde_json::from_str(l).unwrap();
            if v.get("zero_idx").is_some() {
                let n0 = v["key"][0].as_i64().unwrap() as i32;
                let zero: Vec<u64> = serde_json::from_value(v["zero_idx"].clone()).unwrap();
                let t = terms(n0);
                v["nonzero"] = json!(
                    (0..4u64)
                        .filter(|j| !zero.contains(j))
                        .map(|j| format!("{:016x}", term_key_digest(&t[j as usize])))
                        .collect::<Vec<_>>()
                );
                let o = v.as_object_mut().unwrap();
                o.remove("zero_idx");
                o.remove("unknown_idx");
                o.remove("terms_fnv64");
            }
            out.push_str(&format!("{v}\n"));
        }
        std::fs::write(&mem_facts, out).unwrap();
    }
    let mut spec_mem = spec.clone();
    spec_mem["evidence"]["facts"] = json!([{"path": mem_facts.display().to_string()}]);
    let set_mem = load(&TableSetSpec::from_json(&pin_ev(&spec_mem)).unwrap(), &d).unwrap();
    let ev_mem = Evidence::from_spec(&set_mem, &d).unwrap();
    let r_mem = checked(&set_mem, &ev_mem);
    assert_eq!(r_mem.tier, closure::Tier::C);
    // with the store: budgets and threads must not matter
    let mut digests = vec![];
    for (i, (budget, threads)) in [(1usize << 30, 1usize), (1, 4), (1, 1)].iter().enumerate() {
        let so = StoreOptions {
            dir: d.join(format!("stores{i}")),
            budget_bytes: *budget,
            threads: *threads,
        };
        let ev = Evidence::from_spec_with(&set, &d, &EvidenceOptions { store: Some(so) }).unwrap();
        let st = &ev.counts.stores[0];
        assert_eq!(st.origin, "built");
        assert_eq!(st.counts.facts_stored, (top - 1) as u64);
        assert_eq!(st.counts.terms_mismatch, 0);
        assert_eq!(st.counts.foreign, 1);
        // per key one entry: 39 face keys + the point key
        assert_eq!(st.entries, top as u64);
        digests.push(st.data_sha256.clone());
        let r = checked(&set, &ev);
        assert_eq!(
            sig(&r),
            sig(&r_mem),
            "store run {i} differs from the in-memory run"
        );
    }
    assert!(digests.windows(2).all(|w| w[0] == w[1]), "{digests:?}");
    // the facts matter: the key without proof is below per-key, the dropped edges shrink nothing here (every key is a
    // start key) but the positive-numerator term is decided zero
    let r = r_mem;
    let x = r.rows.iter().find(|x| x.key == [5, 1, 0]).unwrap();
    assert_eq!(x.status, RowStatus::Served);
    assert_eq!(x.basis, Some("regularity_fact"));
    let y = r.rows.iter().find(|x| x.key == [6, 1, 0]).unwrap();
    assert_eq!(y.basis, Some("per_key_proof"));
    assert_eq!(
        r.summary.verdict,
        closure::Verdict::Closed,
        "{:?}",
        r.summary.blocking
    );
    // reuse: the same inputs -> reused; a changed input -> rebuilt (the spec's inputs are sha256-pinned (audit-closeloop M1),
    // so a reuse re-hashes them in full: origin `reused_reverified`, t266run 1f1ea34b; merge interaction fixed by lane layers)
    let so = StoreOptions::new(d.join("stores0"));
    let ev = Evidence::from_spec_with(
        &set,
        &d,
        &EvidenceOptions {
            store: Some(so.clone()),
        },
    )
    .unwrap();
    // int6: every spec evidence input is pinned (closeloop4, audit-closeloop M1), and a store whose inputs are pinned
    // re-hashes them in full on reuse (t266run 1f1ea34b): reused, re-verified
    assert_eq!(ev.counts.stores[0].origin, "reused_reverified");
    let mut t = std::fs::read_to_string(&pp).unwrap();
    t.push_str(&format!(
        "{}\n",
        json!({"record_sha256":sha0,"key":[5,1,0],"verdict":"EXACT_PROVED_AT_KEY","index":0})
    ));
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(&pp, t).unwrap();
    // lane closeloop4 (audit-closeloop M1): the changed file no longer matches its pin: refused under the old spec;
    // the RE-PINNED spec (new sha256) rebuilds the store
    assert!(
        Evidence::from_spec_with(
            &set,
            &d,
            &EvidenceOptions {
                store: Some(so.clone())
            }
        )
        .is_err()
    );
    let set = load(&TableSetSpec::from_json(&pin_ev(&spec)).unwrap(), &d).unwrap();
    let ev = Evidence::from_spec_with(&set, &d, &EvidenceOptions { store: Some(so) }).unwrap();
    assert_eq!(ev.counts.stores[0].origin, "built");
    let r = checked(&set, &ev);
    let x = r.rows.iter().find(|x| x.key == [5, 1, 0]).unwrap();
    assert_eq!(x.basis, Some("per_key_proof"));
}

#[test]
fn store_refuses_a_foreign_term_list_and_conflicts() {
    let d = scratch("ref");
    let (spec0, _a) = toy(&d, 6);
    let set = load(&TableSetSpec::from_json(&pin_ev(&spec0)).unwrap(), &d).unwrap();
    let sha0 = set.rules[0][0].record_sha256.clone();
    let good: Vec<i32> = terms(3).iter().flatten().copied().collect();
    let line = |fnv64: &str, n: u64, zero: Vec<u32>| {
        json!({"schema":"rustred.campaign.fact-verdict.v1","index":0,"record_sha256":sha0,"key":[3,1,0],
            "verdict":"regular","n_terms":n,"zero_idx":zero,"unknown_idx":[],"terms_fnv64":fnv64})
    };
    let build =
        |lines: &[Value], tag: &str| -> Result<std::sync::Arc<EvidenceStore>, ImportError> {
            let p = d.join(format!("f-{tag}.jsonl"));
            let s: String = lines.iter().map(|l| format!("{l}\n")).collect();
            std::fs::write(&p, s).unwrap();
            EvidenceStore::open_or_build(
                &set,
                &[(p, None)],
                &StoreOptions::new(d.join(format!("s-{tag}"))),
            )
        };
    // another term list (fnv of a different list) or another term count: refused, counted, not stored
    let st = build(
        &[
            line(&fnv(&good[3..]), 4, vec![2]),
            line(&fnv(&good), 5, vec![2]),
        ],
        "mm",
    )
    .unwrap();
    assert_eq!(st.meta().counts.terms_mismatch, 2);
    assert_eq!(st.meta().entries, 0);
    // the right list: stored, and the lookup answers it
    let st = build(&[line(&fnv(&good), 4, vec![2])], "ok").unwrap();
    let e = st.get(&sha0, &[3, 1, 0]).unwrap();
    assert_eq!(e.fact.as_ref().unwrap().zero.as_ref(), &[2]);
    assert!(st.get(&sha0, &[4, 1, 0]).is_none());
    // two different facts for one (record, key): an error (fail closed)
    assert!(matches!(
        build(
            &[
                line(&fnv(&good), 4, vec![2]),
                line(&fnv(&good), 4, vec![0, 2])
            ],
            "cf"
        ),
        Err(ImportError::Evidence { .. })
    ));
    // a wrong index echo: an error
    let mut bad = line(&fnv(&good), 4, vec![2]);
    bad["index"] = json!(9);
    assert!(matches!(
        build(&[bad], "ix"),
        Err(ImportError::Evidence { .. })
    ));
    // an index >= n_terms: an error
    assert!(matches!(
        build(&[line(&fnv(&good), 4, vec![4])], "oob"),
        Err(ImportError::Evidence { .. })
    ));
}

#[test]
fn store_lookups_over_many_blocks_equal_a_full_scan() {
    let d = scratch("blk");
    let top = 30;
    let (spec0, _a) = toy(&d, top);
    let set = load(&TableSetSpec::from_json(&pin_ev(&spec0)).unwrap(), &d).unwrap();
    let sha0 = set.rules[0][0].record_sha256.clone();
    // many proof lines (keys need not be in the closure): 5,000 keys -> > 19 blocks of 256
    let mut s = String::new();
    for a in 2..=51 {
        for b in -50..50 {
            s.push_str(&format!(
                "{}\n",
                json!({"record_sha256":sha0,"key":[a,1,b],"verdict": if (a + b) % 7 == 0 {"X"} else {"EXACT_IN_SPAN_AT_KEY"}})
            ));
        }
    }
    let p = d.join("many.jsonl");
    std::fs::write(&p, s).unwrap();
    let st = EvidenceStore::open_or_build(
        &set,
        &[(p, None)],
        &StoreOptions {
            dir: d.join("s"),
            budget_bytes: 1,
            threads: 3,
        },
    )
    .unwrap();
    assert_eq!(st.meta().entries, 5000);
    assert!(st.meta().blocks >= 19);
    let mut seen = 0;
    st.for_each(&mut |sha, k, e| {
        assert_eq!(sha, sha0);
        let got = st.get(sha, k).unwrap();
        assert_eq!(*got, *e);
        let word = &st.meta().statuses[e.proof.unwrap() as usize].0;
        assert_eq!(word == "X", (k[0] + k[2]) % 7 == 0);
        seen += 1;
    })
    .unwrap();
    assert_eq!(seen, 5000);
    assert!(st.get(&sha0, &[52, 1, 0]).is_none());
    assert!(st.get(&sha0, &[1, 1, 0]).is_none());
    assert!(st.get(&sha0, &[2, 1, -51]).is_none());
    assert!(st.get(&sha0, &[51, 1, 50]).is_none());
}

/// closeloop4 open item m1 in STORE mode (lane t266run): a per-key proof and a DECIDED contrary verdict for the same
/// (record, key) block the key's per-key level (never "the positive one wins"), in memory and in a store, whether the
/// two lines meet inside one sort run or only in the final merge; an undecided attempt (ERROR) stands beside a proof.
#[test]
fn contradicting_proofs_deny_the_per_key_level_in_memory_and_in_the_store() {
    let d = scratch("contra");
    let top = 9;
    let (spec0, _a) = toy(&d, top);
    let set0 = load(&TableSetSpec::from_json(&spec0.to_string()).unwrap(), &d).unwrap();
    let sha0 = set0.rules[0][0].record_sha256.clone();
    let sha1 = set0.rules[0][1].record_sha256.clone();
    let (fp, pp) = evidence_files(&d, &sha0, &sha1, top);
    // a second proofs file: a decided contrary verdict for [6,1,0], an undecided attempt for [7,1,0]
    let pp2 = d.join("proofs2.jsonl");
    std::fs::write(
        &pp2,
        format!(
            "{}\n{}\n",
            json!({"record_sha256":sha0,"key":[6,1,0],"verdict":"IRREGULAR_KEY","index":0}),
            json!({"record_sha256":sha0,"key":[7,1,0],"verdict":"ERROR","index":0})
        ),
    )
    .unwrap();
    let mut spec = spec0.clone();
    spec["evidence"] = json!({"facts":[{"path": fp.display().to_string()}],
        "proofs":[{"path": pp.display().to_string()}, {"path": pp2.display().to_string()}]});
    // int6: spec evidence is read only pinned and with its family (closeloop4 M1 / m4): `pin_ev`, as the other tests
    let set = load(&TableSetSpec::from_json(&pin_ev(&spec)).unwrap(), &d).unwrap();
    let basis = |r: &import::CheckResult, n0: i32| {
        r.rows
            .iter()
            .find(|x| x.key == [n0, 1, 0])
            .and_then(|x| x.basis)
            .map(str::to_string)
    };
    // the import ITSELF (independent of the closing loop's blocker): the conflicted pair is not per-key even though
    // its regularity fact would lift it; the verdict names the missing level
    let level = |r: &import::CheckResult, n0: i32| {
        r.rows
            .iter()
            .find(|x| x.key == [n0, 1, 0])
            .and_then(|x| x.evidence)
    };
    let not_per_key = |r: &import::CheckResult| {
        // the witness-not-carried key 6 can no longer be lifted through its fact: OPEN in the import's Tier C (key 5,
        // without proof, stays served below per-key); key 7 (ERROR beside the proof) stays per-key
        let row6 = r.rows.iter().find(|x| x.key == [6, 1, 0]);
        assert!(
            row6.is_none_or(
                |x| x.status == RowStatus::Open || x.evidence != Some(import::Level::PerKey)
            ),
            "the conflicted pair is served per-key: {row6:?}"
        );
        assert_eq!(level(r, 7), Some(import::Level::PerKey));
        assert_ne!(
            r.summary.verdict,
            closure::Verdict::Closed,
            "{:?}",
            r.blocking()
        );
        assert!(
            r.blocking().iter().any(|b| b.starts_with("open_keys:")),
            "{:?}",
            r.blocking()
        );
    };
    // in memory
    let ev = Evidence::from_spec(&set, &d).unwrap();
    assert_eq!(ev.counts.proof_conflicts, 1);
    let r = checked(&set, &ev);
    assert_ne!(basis(&r, 6).as_deref(), Some("per_key_proof"));
    assert_eq!(basis(&r, 7).as_deref(), Some("per_key_proof"));
    assert_eq!(ev.conflicts_used(), vec![(sha0.clone(), vec![6, 1, 0])]);
    not_per_key(&r);
    // (rows are not compared with the store runs: the in-memory evidence does not read the index fact form)
    // in a store: one sort run (big budget) and one run per line (budget 1: the lines meet in the final merge)
    for (i, budget) in [1usize << 30, 1].iter().enumerate() {
        let so = StoreOptions {
            dir: d.join(format!("cs{i}")),
            budget_bytes: *budget,
            threads: 2,
        };
        let ev = Evidence::from_spec_with(
            &set,
            &d,
            &EvidenceOptions {
                store: Some(so.clone()),
            },
        )
        .unwrap();
        assert_eq!(
            ev.counts.stores[0].counts.proof_conflicts, 1,
            "budget {budget}"
        );
        let st = EvidenceStore::open(
            &so.dir.join(
                std::fs::read_dir(&so.dir)
                    .unwrap()
                    .next()
                    .unwrap()
                    .unwrap()
                    .file_name(),
            ),
        )
        .unwrap();
        assert!(st.get(&sha0, &[6, 1, 0]).unwrap().proof_conflict);
        assert!(!st.get(&sha0, &[7, 1, 0]).unwrap().proof_conflict);
        assert!(!st.get(&sha0, &[8, 1, 0]).unwrap().proof_conflict);
        let r = checked(&set, &ev);
        assert_ne!(
            basis(&r, 6).as_deref(),
            Some("per_key_proof"),
            "budget {budget}"
        );
        assert_eq!(
            basis(&r, 7).as_deref(),
            Some("per_key_proof"),
            "budget {budget}"
        );
        assert_eq!(
            ev.conflicts_used(),
            vec![(sha0.clone(), vec![6, 1, 0])],
            "budget {budget}"
        );
        not_per_key(&r);
    }
}

/// lane t266run (read path): the per-thread block cache of the stores never changes a check (capacity 0 / 1 / default,
/// 1 / 4 / 16 threads), and it removes reads: with the cache the same check reads fewer blocks.
#[test]
fn store_block_cache_never_changes_a_check_and_saves_reads() {
    let d = scratch("blkcache");
    let top = 30;
    let (spec0, _a) = toy(&d, top);
    let set0 = load(&TableSetSpec::from_json(&spec0.to_string()).unwrap(), &d).unwrap();
    let sha0 = set0.rules[0][0].record_sha256.clone();
    let sha1 = set0.rules[0][1].record_sha256.clone();
    let (fp, pp) = evidence_files(&d, &sha0, &sha1, top);
    let mut spec = spec0.clone();
    spec["evidence"] = json!({"facts":[{"path": fp.display().to_string()}],
                              "proofs":[{"path": pp.display().to_string()}]});
    // spec evidence pinned and with its family (as the other tests: `pin_ev`)
    let set = load(&TableSetSpec::from_json(&pin_ev(&spec)).unwrap(), &d).unwrap();
    let so = StoreOptions {
        dir: d.join("s"),
        budget_bytes: 1,
        threads: 2,
    };
    let mut reference = None;
    let mut reads = std::collections::BTreeMap::new();
    for cap in [0usize, 1, 64] {
        import::store::set_block_cache_capacity(cap);
        for threads in [1usize, 4, 16] {
            let ev = Evidence::from_spec_with(
                &set,
                &d,
                &EvidenceOptions {
                    store: Some(so.clone()),
                },
            )
            .unwrap();
            let r = check(
                &set,
                &ev,
                &CheckOptions {
                    threads,
                    ..Default::default()
                },
            )
            .unwrap();
            let s = sig(&r);
            match &reference {
                None => reference = Some(s),
                Some(x) => assert_eq!(&s, x, "cache {cap} threads {threads}"),
            }
            let st: u64 = ev
                .store_read_stats()
                .iter()
                .map(|(_, s)| s.block_reads)
                .sum();
            reads.insert((cap, threads), st);
        }
    }
    import::store::set_block_cache_capacity(64);
    assert!(reads[&(64, 1)] <= reads[&(0, 1)], "{reads:?}");
}

/// lane t266run (read path 2): the per-thread entry cache keeps one slot PER STORE however many stores are attached.
/// The evidence layer asks every store for the same (record, key) twice (store_fact, then store_proof); with more
/// stores than the old 16 round-robin slots, every slot was overwritten before its reuse (T266's closing run: 39 stores,
/// 0 entry hits in 1.6e11 lookups). Here 40 handles on one store, swept twice in the same order: every handle's second
/// lookup is an entry hit, and every answer equals the first sweep's.
#[test]
fn store_entry_cache_keeps_a_slot_per_store_beyond_sixteen_stores() {
    let d = scratch("entrycache");
    let top = 12;
    let (spec0, _a) = toy(&d, top);
    let set0 = load(&TableSetSpec::from_json(&spec0.to_string()).unwrap(), &d).unwrap();
    let sha0 = set0.rules[0][0].record_sha256.clone();
    let sha1 = set0.rules[0][1].record_sha256.clone();
    let (fp, pp) = evidence_files(&d, &sha0, &sha1, top);
    let mut spec = spec0.clone();
    spec["evidence"] = json!({"facts":[{"path": fp.display().to_string()}],
                              "proofs":[{"path": pp.display().to_string()}]});
    let set = load(&TableSetSpec::from_json(&pin_ev(&spec)).unwrap(), &d).unwrap();
    let so = StoreOptions { dir: d.join("s"), budget_bytes: 1, threads: 1 };
    let ev = Evidence::from_spec_with(&set, &d, &EvidenceOptions { store: Some(so.clone()) }).unwrap();
    drop(ev);
    let sdir = std::fs::read_dir(&so.dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.is_dir())
        .expect("a store directory");
    let stores: Vec<EvidenceStore> = (0..40).map(|_| EvidenceStore::open(&sdir).unwrap()).collect();
    let key = [6, 1, 0];
    let first: Vec<_> = stores.iter().map(|s| s.get(&sha0, &key).map(|e| (*e).clone())).collect();
    assert!(first.iter().all(Option::is_some), "the toy stores (sha0, [6,1,0])");
    let second: Vec<_> = stores.iter().map(|s| s.get(&sha0, &key).map(|e| (*e).clone())).collect();
    assert_eq!(first, second);
    for (i, s) in stores.iter().enumerate() {
        let r = s.read_stats();
        assert_eq!((r.lookups, r.entry_hits), (2, 1), "store handle {i}: {r:?}");
    }
}
