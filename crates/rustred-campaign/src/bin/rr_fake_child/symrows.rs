//! `rr_fake_child` emulating kind `proof-symrows` (lane symcert; interfaces/proof-symrows-job.md): rr_proof
//! `--mode instance-span --symrows` WITHOUT the kernel. It reads job.json, the pinned request, table, keys and symrows,
//! recomputes the record's SHA-256 at the request's offset with the campaign core's own code (independent of the
//! request's claim), counts the symrows lines, and writes ONE `rustred.proof-kernel.span.v1` record to
//! `out/span.jsonl`, then exits 0 WITHOUT OUTCOME (a legacy child: the supervisor-side wrapper checks and seals).
//! Script ops: `prepare {ms}`; `fail {message}` (rr_proof's refusal: an ERROR line on stdout, exit 2; e.g. a
//! `SYMROW_REFUSED: ...` message); `proof_answer {status, record_sha256, rule, key, ms}` (the per-key status and lies
//! about the binding); `proof_answer {status: "forge_sealed"}` = the child writes its OWN OUTCOME and a VALIDLY sealed
//! certificate for ANOTHER relation (first coefficient changed): the contract's S check must refuse it (row 43).
//! A signal ends the child like rr_proof (killed by it).

use super::SIGNALS;
use rustred_campaign_contract::classify::symcert as sc;
use rustred_campaign_contract::exit::ExitCode;
use rustred_campaign_contract::fake::FakeOp;
use rustred_campaign_contract::layout::{RESULTS, RunLayout};
use rustred_campaign_contract::outcome::{
    EndKind, ExtraOutput, JobOutcome, Outcome, ProgramInfo, StopInfo,
};
use rustred_campaign_contract::seal::RecordWriter;
use rustred_campaign_contract::spec::JobSpec;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::atomic::Ordering::SeqCst;
use std::time::Duration;

fn die(msg: &str) -> ! {
    println!("{}", json!({"verdict": "ERROR", "error": msg}));
    std::process::exit(2)
}

fn check_signal() {
    if SIGNALS.load(SeqCst) > 0 {
        let s = super::LAST_SIG.load(SeqCst);
        super::die_by_signal(if s > 0 { s } else { libc::SIGTERM });
    }
}

fn nap(ms: u64) {
    let mut left = ms;
    while left > 0 {
        check_signal();
        let d = left.min(20);
        std::thread::sleep(Duration::from_millis(d));
        left -= d;
    }
    check_signal();
}

fn sha(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

fn key_object(k: &[i64]) -> Value {
    let mut m = serde_json::Map::new();
    for (i, x) in k.iter().enumerate() {
        m.insert(format!("n{i}"), json!(x));
    }
    Value::Object(m)
}

/// The forged attempt: OUTCOME + a validly sealed certificate about another relation of the same key.
fn forge(spec: &JobSpec, layout: &RunLayout, r: &sc::SymrowsRequest, sym_b3: &str) -> ! {
    let mut other = r.clone();
    other.relation_digest = rustred_campaign_contract::blake3_hex(b"another relation");
    let entry = json!({"rule": r.rule, "record_sha256": r.record_sha256, "key": key_object(&r.key),
        "status": "EXACT_IN_SPAN_AT_KEY", "instance": {"forged": true}, "seconds": 0.0});
    let line = sc::certificate_line(&other, sym_b3, &entry).unwrap_or_else(|e| die(&e));
    let spec_b3 = rustred_campaign_contract::spec::spec_blake3(
        &std::fs::read(layout.job_spec()).unwrap_or_else(|_| die("job.json")),
    );
    let mut o = JobOutcome::for_spec(
        spec,
        &spec_b3,
        ProgramInfo {
            name: spec.program.name.clone(),
            version: "legacy".into(),
            git_commit: rustred_campaign_contract::UNKNOWN_COMMIT.into(),
            features: vec!["legacy-wrapped".into()],
            exe: None,
        },
        ExitCode::Complete,
        StopInfo::none(),
    );
    let mut w = RecordWriter::create(&layout.results()).unwrap_or_else(|e| die(&e.to_string()));
    w.append(&line).unwrap_or_else(|e| die(&e.to_string()));
    let req_b3 = spec
        .inputs
        .iter()
        .find(|i| i.role == "request")
        .map(|i| i.blake3.clone());
    let s = w
        .seal(
            json!({"type": "end", "records": 1, "request_blake3": req_b3})
                .to_string()
                .as_bytes(),
        )
        .unwrap_or_else(|e| die(&e.to_string()));
    o.counters.cases = 1;
    o.outputs.extra.push(ExtraOutput {
        role: "results".into(),
        path: RunLayout::out_rel(RESULTS),
        bytes: s.bytes,
        blake3: s.blake3,
        records: Some(s.records),
        canonical_blake3: Some(s.canonical_blake3),
        end: Some(EndKind::End),
        sealed: true,
    });
    Outcome::new(o)
        .write_new(&layout.outcome())
        .unwrap_or_else(|e| die(&e.to_string()));
    std::process::exit(0)
}

pub fn run(spec_path: &Path) -> ! {
    let Ok(spec) = std::fs::read(spec_path)
        .map_err(|e| e.to_string())
        .and_then(|b| serde_json::from_slice::<JobSpec>(&b).map_err(|e| e.to_string()))
    else {
        die("job.json unreadable");
    };
    let layout = RunLayout::new(&spec.run_dir);
    let input = |role: &str| -> Vec<u8> {
        spec.inputs
            .iter()
            .find(|i| i.role == role)
            .and_then(|i| std::fs::read(spec.input_path(i)).ok())
            .unwrap_or_else(|| die(&format!("input {role} missing")))
    };
    let ops: Vec<FakeOp> = spec
        .fake
        .as_ref()
        .map(|f| f.script.clone())
        .unwrap_or_default();
    for op in &ops {
        match op {
            FakeOp::Prepare { ms } => nap(*ms),
            FakeOp::Fail { message, .. } => die(message),
            _ => {}
        }
    }
    let answer = ops
        .iter()
        .find(|o| matches!(o, FakeOp::ProofAnswer { .. }))
        .cloned();
    let r: sc::SymrowsRequest =
        serde_json::from_slice(&input("request")).unwrap_or_else(|e| die(&format!("request: {e}")));
    let table = input("table");
    let line = sc::line_at(&table, r.offset).unwrap_or_else(|e| die(&e));
    let rec: Value = serde_json::from_slice(line).unwrap_or_else(|_| die("record unreadable"));
    if rec["index"].as_i64() != Some(r.rule) {
        die("record at offset is not the requested rule index");
    }
    let symrows = input("symrows");
    let given = symrows
        .split(|&c| c == b'\n')
        .filter(|l| !l.iter().all(u8::is_ascii_whitespace))
        .count();
    let keys = String::from_utf8_lossy(&input("keys")).to_string();
    let key: Vec<i64> = keys
        .lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| {
            l.split(',')
                .map(|p| {
                    p.split_once('=')
                        .and_then(|(_, v)| v.trim().parse().ok())
                        .unwrap_or_else(|| die("key"))
                })
                .collect()
        })
        .unwrap_or_else(|| die("no key"));
    let mut status = "EXACT_IN_SPAN_AT_KEY".to_string();
    let mut esha = sha(line);
    let (mut erule, mut ekey) = (r.rule, key.clone());
    if let Some(FakeOp::ProofAnswer {
        ms,
        status: s,
        record_sha256,
        rule,
        key: k,
        ..
    }) = answer
    {
        nap(ms);
        if s.as_deref() == Some("forge_sealed") {
            forge(
                &spec,
                &layout,
                &r,
                &rustred_campaign_contract::blake3_hex(&symrows),
            );
        }
        if let Some(s) = s {
            status = s;
        }
        if let Some(x) = record_sha256 {
            esha = x;
        }
        if let Some(x) = rule {
            erule = x;
        }
        if let Some(x) = k {
            ekey = x;
        }
    }
    check_signal();
    let rec = json!({"schema": sc::SPAN_SCHEMA, "mode": "instance-span", "rule": r.rule,
        "record_sha256": sha(line), "symrows": {"given": given, "only": r.symrows_only},
        "zero_sectors": {"dropped_from_identity": r.drop_zero_sectors},
        "keys": [{"rule": erule, "record_sha256": esha, "key": key_object(&ekey), "status": status,
                  "instance": {"fake": true, "symmetry_rows": given}, "seconds": 0.001}],
        "fake": true});
    let p = layout.out_dir().join(sc::CHILD_OUT);
    if let Err(e) = std::fs::write(&p, format!("{rec}\n")) {
        die(&format!("{}: {e}", p.display()));
    }
    println!("{}", json!({"fake": true, "written": sc::CHILD_OUT}));
    std::process::exit(0)
}
