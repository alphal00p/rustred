//! Lane symcert (interfaces/proof-symrows-job.md 5): `certify_symmetry_relations` through the REAL process layer with
//! the fake child (`rr_fake_child` emulating kind `proof-symrows`): every verdict word, the findings file with both
//! sides, a FORGED certificate with a valid seal refused (row 43), a lie about the record refused (row 43), the 15b
//! resource retry, a not-expressible relation without a job, resume from the journal, and the import hook built from
//! the verdicts. Spawns children: run with LIGHT_PIDNS=1 (L10). Test data only (L8): arity-3 keys, made-up digests.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

use rustred_campaign::adapters::job::{Invocation, Program};
use rustred_campaign::closeloop::symcert::{
    CertifyConfig, ImportBinding, RelationVerdict, certify_symmetry_relations,
    kernel_certificate_files, summary,
};
use rustred_campaign_contract::exit::ExitCode;
use rustred_campaign_contract::fake::{FakeOp, FakeScript};
use rustred_campaign_core::symcert::{
    CatalogueSeeds, KernelCaps, SymRelation, SymRow, ZeroSectors,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

const FAKE: &str = env!("CARGO_BIN_EXE_rr_fake_child");

fn tmp(tag: &str) -> PathBuf {
    let base = std::env::var_os("TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let p = base.join(format!("symcert-fake-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn rel(id: &str, c: i64) -> SymRelation {
    SymRelation {
        id: id.into(),
        producer: "test".into(),
        producer_record_sha256: Some(format!("{:0>64}", c)),
        key: vec![1, 1, -c],
        rhs: vec![
            (vec![1, 1, -c + 1], format!("{c}")),
            (vec![1, 0, -c], "1/2".into()),
        ],
        rows: vec![SymRow {
            seed: vec![1, 1, -c],
            element: 1,
            matrix: vec![1, 0, 0, 0, -1, 0, 0, 0, 1],
        }],
        catalogue: vec![],
        group_digest: Some("g".into()),
        zero_sectors: None,
        own: json!({"verdict": "accepted", "by": "test"}),
    }
}

fn script(op: FakeOp) -> FakeScript {
    FakeScript {
        script: vec![op],
        on_sigterm: Default::default(),
        slow_ms: 0,
        start_delay_ms: 0,
        rss: Default::default(),
        helpers: vec![],
        faults: vec![],
        time_scale: 1.0,
        volatile_jitter: false,
    }
}

fn answer(status: &str) -> FakeOp {
    FakeOp::ProofAnswer {
        ms: 0,
        status: Some(status.into()),
        record_sha256: None,
        rule: None,
        key: None,
        line: None,
    }
}

fn fail(msg: &str) -> FakeOp {
    FakeOp::Fail {
        code: ExitCode::Input,
        message: msg.into(),
    }
}

fn config(root: &Path, fake_by_id: BTreeMap<String, FakeScript>) -> CertifyConfig {
    CertifyConfig {
        work_root: root.to_path_buf(),
        // a fake script needs the fake program name (contract: a real engine refuses it at preflight)
        program: Program::from_file("rr_fake_child", Path::new(FAKE), Invocation::Contract)
            .unwrap(),
        licence_env: None,
        family_source: "I(name(toy),loops(k1,k2),externals(q))".into(),
        caps: KernelCaps {
            arity: 3,
            max_rows: None,
            max_seeds: None,
        },
        parallel: 3,
        cpus: vec![],
        memory_cap_bytes: Some(1 << 30),
        wall_cap: Some(Duration::from_secs(60)),
        resource_retries: 1,
        resource_backoff: Duration::from_millis(50),
        max_primes: None,
        fake: Some(script(FakeOp::Prepare { ms: 0 })),
        fake_by_id,
    }
}

fn by_id(v: &[RelationVerdict]) -> BTreeMap<String, RelationVerdict> {
    v.iter().map(|x| (x.id.clone(), x.clone())).collect()
}

#[test]
fn every_verdict_through_the_process_layer() {
    let root = tmp("all");
    let mut f = BTreeMap::new();
    f.insert(
        "refused".into(),
        script(answer("EXACT_NOT_IN_SPAN_AT_KEY (exact certificate u(d))")),
    );
    f.insert(
        "symrow".into(),
        script(fail(
            "SYMROW_REFUSED: loop part not unimodular (symrow e1#0 [1, 1, -3])",
        )),
    );
    f.insert(
        "capacity".into(),
        script(fail(
            "SYMROW_REFUSED: expansion above 5,000,000 terms (symrow e1#0)",
        )),
    );
    f.insert(
        "undecided".into(),
        script(answer("UNDECIDED (too many singular points)")),
    );
    f.insert("forged".into(), script(answer("forge_sealed")));
    f.insert(
        "lie".into(),
        script(FakeOp::ProofAnswer {
            ms: 0,
            status: None,
            record_sha256: Some("f".repeat(64)),
            rule: None,
            key: None,
            line: None,
        }),
    );
    f.insert(
        "resource".into(),
        script(fail("open table: Too many open files (os error 24)")),
    );
    f.insert(
        "plain_error".into(),
        script(fail("key does not assign every free index")),
    );
    let mut rels = vec![
        rel("ok", 1),
        rel("refused", 2),
        rel("symrow", 3),
        rel("capacity", 4),
        rel("undecided", 5),
        rel("forged", 6),
        rel("lie", 7),
        rel("resource", 8),
        rel("plain_error", 9),
    ];
    let mut nx = rel("nx", 10);
    nx.key = vec![1, 1];
    rels.push(nx);
    // a composite with catalogue seeds and a census: rows engine, zero sectors dropped
    let mut comp = rel("composite", 11);
    comp.catalogue.push(CatalogueSeeds {
        source: "own".into(),
        label: json!(3),
        record_sha256: "a".repeat(64),
        term: vec![1, 0, -11],
        seeds: vec![vec![1, 1, -11], vec![1, 0, -10]],
    });
    comp.zero_sectors = Some(ZeroSectors {
        producer: "test census".into(),
        scope: "chart_subsets".into(),
        masks: vec!["000".into(), "100".into(), "010".into()],
    });
    rels.push(comp);
    let cfg = config(&root, f);
    let v = certify_symmetry_relations(&rels, &cfg).unwrap();
    let b = by_id(&v);
    let word = |id: &str| b[id].verdict.as_str();
    assert_eq!(word("ok"), "certified", "{:?}", b["ok"]);
    assert!(b["ok"].certificate.is_some() && b["ok"].certificate_sha256.is_some());
    assert_eq!(b["ok"].row.as_deref(), Some("40"));
    assert_eq!(word("composite"), "certified", "{:?}", b["composite"]);
    let cert: serde_json::Value =
        serde_json::from_str(b["composite"].certificate.as_ref().unwrap()).unwrap();
    assert_eq!(cert["rows"], "engine");
    assert_eq!(cert["drop_zero_sectors"], true);
    assert_eq!(cert["catalogue"][0]["source"], "own");
    assert_eq!(word("refused"), "kernel_refused");
    assert!(b["refused"].finding);
    assert_eq!(word("symrow"), "kernel_refused");
    assert_eq!(b["symrow"].row.as_deref(), Some("41"));
    assert!(b["symrow"].finding);
    assert_eq!(word("capacity"), "over_kernel_capacity");
    assert!(!b["capacity"].finding);
    assert_eq!(word("undecided"), "undecided");
    // a forged certificate with a valid seal (another relation) and a lie about the record: row 43, nothing read
    assert_eq!(word("forged"), "failed");
    assert_eq!(b["forged"].row.as_deref(), Some("43"), "{:?}", b["forged"]);
    assert!(
        b["forged"].detail.contains("relation_digest"),
        "{}",
        b["forged"].detail
    );
    assert_eq!(word("lie"), "failed");
    assert_eq!(b["lie"].row.as_deref(), Some("43"));
    // a host-resource end: retried once, then failed 15b
    assert_eq!(word("resource"), "failed");
    assert_eq!(b["resource"].row.as_deref(), Some("15b"));
    assert_eq!(b["resource"].attempts, 2);
    assert_eq!(word("plain_error"), "failed");
    assert_eq!(b["plain_error"].row.as_deref(), Some("13"));
    assert_eq!(word("nx"), "not_expressible");
    assert!(b["nx"].job_id.is_none());
    // findings: both sides
    let findings = std::fs::read_to_string(root.join("findings.jsonl")).unwrap();
    let fl: Vec<serde_json::Value> = findings
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(fl.len(), 2);
    for x in &fl {
        assert!(x["kernel"]["status"].as_str().is_some());
        assert_eq!(x["ours"]["verdict"], "accepted");
    }
    let s = summary(&v);
    assert_eq!(s["by_verdict"]["certified"], 2);
    assert_eq!(s["findings"], 2);
    // THE CERTIFICATES HANDED TO THE IMPORT (audit r2-sym F4; lane symfix's route (b) re-checks each against its own
    // record): a verdict is only a pointer to a sealed, re-verified run directory
    let binding = ImportBinding {
        kernels: vec![cfg.program.sha256.clone().unwrap()],
        family_blake3: rustred_campaign_contract::blake3_hex(cfg.family_source.as_bytes()),
        roots: vec![root.clone()],
    };
    let reason = |no: &[rustred_campaign::closeloop::symcert::NoLift], id: &str| {
        no.iter().find(|x| x.id == id).map(|x| x.reason)
    };
    let (files, no) = kernel_certificate_files(&v, &binding);
    let mut ids: Vec<&str> = files.iter().map(|f| f.relation_id.as_str()).collect();
    ids.sort();
    assert_eq!(ids, vec!["composite", "ok"], "{no:?}");
    assert_eq!(reason(&no, "refused"), Some("not_decided"));
    assert_eq!(reason(&no, "symrow"), Some("not_decided"));
    for f in &files {
        let bytes = std::fs::read(&f.path).unwrap();
        assert_eq!(
            rustred_campaign::adapters::job::sha256_hex(&bytes),
            f.sha256
        );
        // the stream symfix's import reads: certificate line(s) then ONE end record with request_blake3, last
        let lines: Vec<&[u8]> = bytes
            .split(|&c| c == b'\n')
            .filter(|l| !l.is_empty())
            .collect();
        assert_eq!(lines.len(), 2);
        let end: serde_json::Value = serde_json::from_slice(lines[1]).unwrap();
        assert_eq!(
            (end["type"].as_str(), end["records"].as_u64()),
            (Some("end"), Some(1))
        );
        assert_eq!(end["request_blake3"].as_str().map(str::len), Some(64));
        let line: serde_json::Value = serde_json::from_slice(lines[0]).unwrap();
        let rel = rels.iter().find(|r| r.id == f.relation_id).unwrap();
        let canon =
            rustred_campaign_contract::classify::symcert::relation_canonical(&rel.key, &rel.rhs)
                .unwrap();
        assert_eq!(
            line["relation_sha256"].as_str().unwrap(),
            rustred_campaign::adapters::job::sha256_hex(&canon)
        );
        assert_eq!(
            line["family_sha256"].as_str().unwrap(),
            rustred_campaign::adapters::job::sha256_hex(cfg.family_source.as_bytes())
        );
        assert_eq!(
            (line["class"].as_str(), line["verdict"].as_str()),
            (Some("certified"), Some("EXACT_IN_SPAN_AT_KEY"))
        );
    }
    // (1) an UNSEALED verdict line: "certified" pointing at the run whose answers the wrapper refused (row 43), and
    // one without a run directory
    let mut fake = b["ok"].clone();
    fake.run_dir = b["lie"].run_dir.clone();
    let mut bare = b["ok"].clone();
    bare.run_dir = None;
    let (f1, n1) = kernel_certificate_files(&[fake, bare], &binding);
    assert!(f1.is_empty());
    assert_eq!(n1[0].reason, "certificate_not_sealed", "{n1:?}");
    assert_eq!(n1[1].reason, "no_run_dir");
    // (2) a verdict line that says "certified" for the run the kernel REFUSED: the sealed class decides
    let mut lied = b["refused"].clone();
    lied.verdict = "certified".into();
    let (f2, n2) = kernel_certificate_files(&[lied], &binding);
    assert!(f2.is_empty());
    assert_eq!(n2[0].reason, "not_decided");
    // (3) ANOTHER FAMILY and (4) ANOTHER KERNEL
    let mut fam = binding.clone();
    fam.family_blake3 = rustred_campaign_contract::blake3_hex(b"I(name(other))");
    assert_eq!(
        kernel_certificate_files(&[b["ok"].clone()], &fam).1[0].reason,
        "family_differs"
    );
    let mut ker = binding.clone();
    ker.kernels = vec!["e".repeat(64)];
    assert_eq!(
        kernel_certificate_files(&[b["ok"].clone()], &ker).1[0].reason,
        "kernel_not_pinned"
    );
    // (5) REPLAY ACROSS CAMPAIGNS: certificates of another campaign's work tree are never handed on
    let root_b = tmp("replay");
    let mut cfg_b = config(&root_b, BTreeMap::new());
    cfg_b.parallel = 1;
    let vb = certify_symmetry_relations(&rels[..1], &cfg_b).unwrap();
    assert_eq!(vb[0].verdict, "certified");
    let (f5, n5) = kernel_certificate_files(&vb, &binding);
    assert!(f5.is_empty());
    assert_eq!(n5[0].reason, "outside_certificate_roots");
    // (6) a seal tampered after the fact (the certificate line edited): not handed on
    let run_ok = PathBuf::from(b["ok"].run_dir.clone().unwrap());
    let res = run_ok.join("out/results.jsonl");
    let txt = std::fs::read_to_string(&res).unwrap();
    std::fs::write(
        &res,
        txt.replace("EXACT_IN_SPAN_AT_KEY", "EXACT_IN_SPAN_AT_KEX"),
    )
    .unwrap();
    let (f6, n6) = kernel_certificate_files(&[b["ok"].clone()], &binding);
    assert!(f6.is_empty());
    assert_eq!(n6[0].reason, "certificate_not_sealed", "{n6:?}");
    std::fs::write(&res, txt).unwrap();
    let _ = std::fs::remove_dir_all(&root_b);
    // RESUME: a second call answers the decided relations from the journal (same certificate, no new attempt)
    let v2 = certify_symmetry_relations(&rels, &cfg).unwrap();
    let b2 = by_id(&v2);
    for id in [
        "ok",
        "refused",
        "symrow",
        "capacity",
        "undecided",
        "composite",
    ] {
        assert_eq!(b2[id], b[id], "{id}");
    }
    let _ = std::fs::remove_dir_all(&root);
}
