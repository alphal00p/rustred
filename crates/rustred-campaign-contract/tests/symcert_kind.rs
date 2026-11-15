//! Kind `proof-symrows` (lane symcert; interfaces/proof-symrows-job.md): spec rules (roles, empty allow-list, legacy
//! constraints), the argv derived from the pinned request, the relation digest and the table binding, the kernel
//! answer checks of the wrapper, and the decision rows: 40 (certificate sealed and bound: `certificates`), 41 (the
//! kernel refused a symmetry row), 42 (reader error), 43 (certificate not sealed / not bound: a FORGED certificate with
//! a VALID seal about another relation, key or symrows input is refused), 13 (another refusal), 15b (host resource),
//! signals as for every kind.
//! Test data only (L8): keys of arity 3, made-up digests.

mod common;

use common::*;
use rustred_campaign_contract::classify::symcert::{
    self, CERT_SCHEMA, CatalogueUse, REQUEST_SCHEMA, SPAN_SCHEMA, SymrowsRequest,
};
use rustred_campaign_contract::classify::{
    self, Accept, Action, KillReason, OutcomeKind, ROWS, WaitStatus,
};
use rustred_campaign_contract::exit::ExitCode;
use rustred_campaign_contract::layout::{RESULTS, RunLayout};
use rustred_campaign_contract::outcome::{
    EndKind, ErrorClass, ErrorInfo, ExtraOutput, JobOutcome, Outcome, ProgramInfo, StopInfo,
};
use rustred_campaign_contract::seal::RecordWriter;
use rustred_campaign_contract::spec::*;
use rustred_campaign_contract::stop::StopCode;
use serde_json::{Value, json};

fn b3(b: &[u8]) -> String {
    rustred_campaign_contract::blake3_hex(b)
}

const SHA: &str = "abababababababababababababababababababababababababababababababab";
const KSHA: &str = "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";
const KEY: [i64; 3] = [1, 1, -2];

fn rhs() -> Vec<(Vec<i64>, String)> {
    vec![
        (vec![1, 1, -1], "-2".into()),
        (vec![1, 0, -2], "1/2".into()),
        (vec![0, 1, -2], "1/2".into()),
    ]
}

/// The table the supervisor writes: header (family_source) + one point record. Returns (bytes, offset, record line).
fn table(rhs: &[(Vec<i64>, String)]) -> (Vec<u8>, u64, Vec<u8>) {
    let head = json!({"type": "header", "schema": "table_dump.v1", "family_source": "I(name(toy),loops(k1,k2),externals(q))"});
    let rec = json!({"type": "rule", "chart": "110", "index": 0, "case": "point", "fixed": KEY, "equations": [],
        "target": symcert::key_pattern(&KEY),
        "rhs": rhs.iter().map(|(k, c)| json!([symcert::key_pattern(k), c])).collect::<Vec<_>>(),
        "exceptions": [], "seeds": [], "recenter": [0, 0, 0], "dispatch": "Point", "witness_loci": []});
    // the kernel needs the line to START with {"type":"rule"
    let rec_line = format!(
        "{{\"type\":\"rule\",{}",
        &rec.to_string()[1..].replacen("\"type\":\"rule\",", "", 1)
    );
    let mut b = format!("{head}\n").into_bytes();
    let off = b.len() as u64;
    b.extend_from_slice(rec_line.as_bytes());
    b.push(b'\n');
    (b, off, rec_line.into_bytes())
}

fn request(off: u64, line: &[u8]) -> SymrowsRequest {
    let (key, r) = symcert::relation_of_record(line).unwrap();
    SymrowsRequest {
        schema: REQUEST_SCHEMA.into(),
        relation_digest: symcert::relation_digest(&key, &r).unwrap(),
        rule: 0,
        offset: off,
        record_sha256: SHA.into(),
        record_blake3: b3(line),
        key: key.clone(),
        producer: "test".into(),
        producer_record_sha256: Some(SHA.into()),
        group_digest: Some("g".into()),
        elements: vec![2],
        symrows_lines: 1,
        symrows_only: true,
        rows: "none".into(),
        drop_zero_sectors: false,
        zero_census: None,
        catalogue: vec![CatalogueUse {
            source: "own".into(),
            label: json!(7),
            record_sha256: SHA.into(),
        }],
        kernel_sha256: KSHA.into(),
        max_primes: None,
    }
}

struct Job {
    _t: TempDir,
    spec: JobSpec,
    digest: String,
    layout: RunLayout,
    req: SymrowsRequest,
}

fn job(tag: &str) -> Job {
    let t = TempDir::new(tag);
    let run = t.path().join("run");
    let layout = RunLayout::new(&run);
    for d in [
        layout.in_dir(),
        layout.out_dir(),
        layout.tmp_dir(),
        layout.sup_dir(),
    ] {
        std::fs::create_dir_all(d).unwrap();
    }
    let (tb, off, line) = table(&rhs());
    let req = request(off, &line);
    let files: Vec<(&str, &str, Vec<u8>)> = vec![
        ("request", "request.json", serde_json::to_vec(&req).unwrap()),
        ("table", "relation.table.jsonl", tb),
        (
            "keys",
            "keys.txt",
            format!("{}\n", symcert::key_text_named(&KEY)).into_bytes(),
        ),
        (
            "symrows",
            "symrows.jsonl",
            b"{\"seed\":[1,1,-2],\"matrix\":[1,0,0,0,1,0,0,0,1],\"id\":\"e2@K\"}\n".to_vec(),
        ),
    ];
    let mut spec = sample_spec(&run.display().to_string());
    spec.kind = JobKind::ProofSymrows;
    spec.program = ProgramRef {
        name: "rr_proof".into(),
        blake3: D0.into(),
        features_required: vec![],
    };
    spec.inputs = files
        .iter()
        .map(|(role, name, b)| {
            let p = layout.in_dir().join(name);
            std::fs::write(&p, b).unwrap();
            InputRef {
                role: role.to_string(),
                path: p.display().to_string(),
                blake3: b3(b),
                upto: None,
            }
        })
        .collect();
    spec.args = vec![];
    spec.budgets = Budgets::default();
    spec.validate().unwrap();
    let bytes = spec.to_json_line();
    let digest = spec_blake3(&bytes);
    std::fs::write(layout.job_spec(), &bytes).unwrap();
    Job {
        _t: t,
        spec,
        digest,
        layout,
        req,
    }
}

fn program() -> ProgramInfo {
    ProgramInfo {
        name: "rr_proof".into(),
        version: "legacy".into(),
        git_commit: rustred_campaign_contract::UNKNOWN_COMMIT.into(),
        features: vec!["legacy-wrapped".into()],
        exe: None,
    }
}

fn symrows_b3(j: &Job) -> String {
    j.spec
        .inputs
        .iter()
        .find(|i| i.role == "symrows")
        .unwrap()
        .blake3
        .clone()
}

fn request_b3(j: &Job) -> String {
    j.spec
        .inputs
        .iter()
        .find(|i| i.role == "request")
        .unwrap()
        .blake3
        .clone()
}

/// The kernel's key entry (what rr_proof writes) for the request.
fn entry(r: &SymrowsRequest, status: &str) -> Value {
    let mut key = serde_json::Map::new();
    for (i, x) in r.key.iter().enumerate() {
        key.insert(format!("n{i}"), json!(x));
    }
    json!({"rule": r.rule, "record_sha256": r.record_sha256, "key": key, "status": status,
        "instance": {"rows": 1, "columns": 4, "symmetry_rows": 1}, "seconds": 0.01})
}

/// A sealed results stream with the given certificate lines + OUTCOME exit 0 (what the wrapper writes).
fn seal_and_outcome(j: &Job, certs: &[Vec<u8>], seal: bool) {
    let mut o = JobOutcome::for_spec(
        &j.spec,
        &j.digest,
        program(),
        ExitCode::Complete,
        StopInfo::none(),
    );
    if seal {
        let mut w = RecordWriter::create(&j.layout.results()).unwrap();
        for c in certs {
            w.append(c).unwrap();
        }
        let s = w
            .seal(
                json!({"type": "end", "records": certs.len(), "request_blake3": request_b3(j)})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap();
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
    }
    Outcome::new(o).write_new(&j.layout.outcome()).unwrap();
}

fn refusal_outcome(j: &Job, code: ExitCode, msg: &str) {
    let mut o = JobOutcome::for_spec(
        &j.spec,
        &j.digest,
        program(),
        code,
        StopInfo {
            code: Some(if code == ExitCode::Io {
                StopCode::IoError
            } else {
                StopCode::InputError
            }),
            ..StopInfo::none()
        },
    );
    let class = if code == ExitCode::Io {
        ErrorClass::Io
    } else {
        ErrorClass::Input
    };
    o.error = Some(ErrorInfo::new(class, msg));
    Outcome::new(o).write_new(&j.layout.outcome()).unwrap();
}

fn classify_job(j: &Job, wait: WaitStatus) -> classify::Classification {
    let ev = classify::assess(&j.layout, &j.spec, &j.digest, wait, KillReason::None, false);
    classify::classify(&ev)
}

#[test]
fn the_kind_is_a_legacy_kind_with_pinned_request_table_keys_and_symrows() {
    let k = JobKind::ProofSymrows;
    assert_eq!(k.level(), Level::Legacy);
    assert_eq!(k.program(), "rr_proof");
    assert!(k.legacy_wrapped() && k.fake_emulated() && k.sealed_outputs().is_some());
    assert!(k.allowed_args().is_empty());
    let roles = k.input_roles().unwrap();
    assert_eq!(roles.required, &["request", "table", "keys", "symrows"]);
    assert_eq!(roles.optional, &["family"]);
    assert!(JobKind::ALL.contains(&k));
    let j = job("sym-spec");
    let mut bad = j.spec.clone();
    bad.args = vec!["--symrows-only".into(), "x".into()];
    assert!(
        bad.validate().is_err(),
        "every flag is outside the allow-list"
    );
    let mut bad = j.spec.clone();
    bad.inputs.retain(|i| i.role != "symrows");
    assert!(bad.validate().is_err(), "the symrows input is required");
    let mut bad = j.spec.clone();
    bad.seed.sample_seed = 3;
    assert!(bad.validate().is_err(), "legacy constraints apply");
    // rows 40-43 exist with their semantics
    let r = |n: u8| ROWS.iter().find(|r| r.row == n).unwrap();
    assert_eq!(
        (r(40).kind, r(40).accept, r(40).action),
        (
            OutcomeKind::Complete,
            Accept::Certificates,
            Action::ItemDone
        )
    );
    assert_eq!(
        (r(41).accept, r(41).action),
        (Accept::Nothing, Action::CertificateRefused)
    );
    assert_eq!(
        (r(42).accept, r(42).action),
        (Accept::Nothing, Action::TableUnreadableByTool)
    );
    assert_eq!(
        (r(43).accept, r(43).action),
        (Accept::Nothing, Action::Attention)
    );
}

#[test]
fn the_argv_comes_from_the_pinned_request_only() {
    let j = job("sym-argv");
    let a = symcert::argv(&j.spec).unwrap();
    let s = a.join(" ");
    assert!(s.starts_with("--mode instance-span --table "), "{s}");
    assert!(s.contains(" --rule 0 --offset "), "{s}");
    assert!(s.contains(" --symrows-only --rows cert "), "{s}");
    assert!(!s.contains("--drop-zero-sectors"), "{s}");
    assert!(s.ends_with("--out out/span.jsonl"), "{s}");
    // a request that changed after the spec is refused (pinned)
    let p = j.spec.inputs.iter().find(|i| i.role == "request").unwrap();
    std::fs::write(&p.path, b"{}").unwrap();
    assert!(symcert::argv(&j.spec).is_err());
}

#[test]
fn requests_are_internally_consistent() {
    let (_, off, line) = table(&rhs());
    let r = request(off, &line);
    assert!(r.check().is_ok());
    let mut x = r.clone();
    x.rows = "none".into();
    x.symrows_only = false;
    assert!(x.check().is_err(), "no rows at all needs symrows_only");
    let mut x = r.clone();
    x.elements = vec![1, 2];
    assert!(x.check().is_err(), "one element per symrows line");
    let mut x = r.clone();
    x.drop_zero_sectors = true;
    assert!(x.check().is_err(), "dropping zero sectors needs the census");
    let mut x = r.clone();
    x.rows = "cert".into();
    assert!(x.check().is_err());
}

#[test]
fn the_table_binds_the_relation_by_digest() {
    let (tb, off, line) = table(&rhs());
    let r = request(off, &line);
    assert!(symcert::table_relation(&tb, &r).is_ok());
    // another coefficient text: another relation
    let mut other = rhs();
    other[1].1 = "2/4".into();
    let (tb2, off2, _) = table(&other);
    assert_eq!(off2, off);
    assert!(symcert::table_relation(&tb2, &r).is_err());
    // an offset inside a line
    let mut r2 = r.clone();
    r2.offset += 1;
    assert!(symcert::table_relation(&tb, &r2).is_err());
}

#[test]
fn the_wrapper_checks_the_kernel_answer_against_the_request() {
    let (_, off, line) = table(&rhs());
    let r = request(off, &line);
    let rec = |e: Value, given: u64, only: bool, zero: bool, sha: &str| -> Vec<u8> {
        json!({"schema": SPAN_SCHEMA, "rule": 0, "record_sha256": sha,
            "symrows": {"given": given, "only": only}, "zero_sectors": {"dropped_from_identity": zero},
            "keys": [e]})
        .to_string()
        .into_bytes()
    };
    let good = entry(&r, "EXACT_IN_SPAN_AT_KEY");
    let e = symcert::check_span_output(&r, &rec(good.clone(), 1, true, false, SHA)).unwrap();
    let line = symcert::certificate_line(&r, "s", &e).unwrap();
    let c = symcert::check_certificate(&r, "s", &line).unwrap();
    assert_eq!(
        (c.class.as_str(), c.verdict.as_str()),
        ("certified", "EXACT_IN_SPAN_AT_KEY")
    );
    // another record, another symrows count / flag, another key, two entries: refused
    assert!(symcert::check_span_output(&r, &rec(good.clone(), 1, true, false, KSHA)).is_err());
    assert!(symcert::check_span_output(&r, &rec(good.clone(), 2, true, false, SHA)).is_err());
    assert!(symcert::check_span_output(&r, &rec(good.clone(), 1, false, false, SHA)).is_err());
    assert!(symcert::check_span_output(&r, &rec(good.clone(), 1, true, true, SHA)).is_err());
    let mut wrong_key = good.clone();
    wrong_key["key"]["n2"] = json!(-3);
    assert!(symcert::check_span_output(&r, &rec(wrong_key, 1, true, false, SHA)).is_err());
    let two = json!({"schema": SPAN_SCHEMA, "rule": 0, "record_sha256": SHA,
        "symrows": {"given": 1, "only": true}, "zero_sectors": {"dropped_from_identity": false},
        "keys": [good.clone(), good.clone()]});
    assert!(symcert::check_span_output(&r, two.to_string().as_bytes()).is_err());
    // a refusal answer is a decided certificate of class kernel_refused
    let e = symcert::check_span_output(
        &r,
        &rec(
            entry(&r, "EXACT_NOT_IN_SPAN_AT_KEY (exact certificate u(d))"),
            1,
            true,
            false,
            SHA,
        ),
    )
    .unwrap();
    let line = symcert::certificate_line(&r, "s", &e).unwrap();
    let c = symcert::check_certificate(&r, "s", &line).unwrap();
    assert_eq!(c.class, "kernel_refused");
}

#[test]
fn a_sealed_bound_certificate_is_row_40() {
    let j = job("sym-40");
    let e = entry(&j.req, "EXACT_IN_SPAN_AT_KEY");
    let line = symcert::certificate_line(&j.req, &symrows_b3(&j), &e).unwrap();
    seal_and_outcome(&j, &[line], true);
    let c = classify_job(&j, WaitStatus::Exited(0));
    assert_eq!((c.row, c.accept), (40, Accept::Certificates), "{c:?}");
    let ev = classify::assess(
        &j.layout,
        &j.spec,
        &j.digest,
        WaitStatus::Exited(0),
        KillReason::None,
        false,
    );
    let certs =
        symcert::certificates_of_run(&j.layout.root, &j.spec, ev.verified.as_ref().unwrap())
            .unwrap();
    assert_eq!(certs.len(), 1);
    assert_eq!(certs[0].class, "certified");
    assert_eq!(certs[0].relation_digest, j.req.relation_digest);
}

#[test]
fn a_forged_certificate_with_a_valid_seal_is_refused_row_43() {
    // (a) a certificate about ANOTHER relation (digest of a changed coefficient), sealed validly
    let j = job("sym-43a");
    let mut other = j.req.clone();
    let mut r = rhs();
    r[0].1 = "-3".into();
    other.relation_digest = symcert::relation_digest(&KEY, &r).unwrap();
    let e = entry(&other, "EXACT_IN_SPAN_AT_KEY");
    let line = symcert::certificate_line(&other, &symrows_b3(&j), &e).unwrap();
    seal_and_outcome(&j, &[line], true);
    let c = classify_job(&j, WaitStatus::Exited(0));
    assert_eq!((c.row, c.accept), (43, Accept::Nothing), "{c:?}");
    assert!(c.why.contains("relation_digest"), "{}", c.why);
    // (b) the right relation at ANOTHER key
    let j = job("sym-43b");
    let mut other = j.req.clone();
    other.key = vec![1, 1, -3];
    let line = symcert::certificate_line(
        &other,
        &symrows_b3(&j),
        &entry(&other, "EXACT_IN_SPAN_AT_KEY"),
    )
    .unwrap();
    seal_and_outcome(&j, &[line], true);
    assert_eq!(classify_job(&j, WaitStatus::Exited(0)).row, 43);
    // (c) certified against ANOTHER symrows input (its digest)
    let j = job("sym-43c");
    let line = symcert::certificate_line(
        &j.req,
        &"9".repeat(64),
        &entry(&j.req, "EXACT_IN_SPAN_AT_KEY"),
    )
    .unwrap();
    seal_and_outcome(&j, &[line], true);
    assert_eq!(classify_job(&j, WaitStatus::Exited(0)).row, 43);
    // (d) another kernel
    let j = job("sym-43d");
    let mut other = j.req.clone();
    other.kernel_sha256 = SHA.into();
    let line = symcert::certificate_line(
        &other,
        &symrows_b3(&j),
        &entry(&other, "EXACT_IN_SPAN_AT_KEY"),
    )
    .unwrap();
    seal_and_outcome(&j, &[line], true);
    assert_eq!(classify_job(&j, WaitStatus::Exited(0)).row, 43);
    // (e) a lying class (a refusal relabelled certified)
    let j = job("sym-43e");
    let line = symcert::certificate_line(
        &j.req,
        &symrows_b3(&j),
        &entry(&j.req, "NOT_IN_SPAN_MODULAR (residual nonzero)"),
    )
    .unwrap();
    let mut v: Value = serde_json::from_slice(&line).unwrap();
    v["class"] = json!("certified");
    seal_and_outcome(&j, &[v.to_string().into_bytes()], true);
    assert_eq!(classify_job(&j, WaitStatus::Exited(0)).row, 43);
    // (f) not sealed at all (the wrapper refused the answers)
    let j = job("sym-43f");
    seal_and_outcome(&j, &[], false);
    assert_eq!(classify_job(&j, WaitStatus::Exited(0)).row, 43);
    // (g) the table changed after the spec (pinned): S fails
    let j = job("sym-43g");
    let line = symcert::certificate_line(
        &j.req,
        &symrows_b3(&j),
        &entry(&j.req, "EXACT_IN_SPAN_AT_KEY"),
    )
    .unwrap();
    seal_and_outcome(&j, &[line], true);
    let t = j.spec.inputs.iter().find(|i| i.role == "table").unwrap();
    std::fs::write(&t.path, b"{}\n").unwrap();
    assert_eq!(classify_job(&j, WaitStatus::Exited(0)).row, 43);
}

#[test]
fn refusals_are_rows_41_42_13_and_resource_ends_15b() {
    let j = job("sym-41");
    refusal_outcome(
        &j,
        ExitCode::Input,
        "SYMROW_REFUSED: loop part not unimodular (symrow e2@K [1, 1, -2])",
    );
    let c = classify_job(&j, WaitStatus::Exited(41));
    assert_eq!((c.row, c.action), (41, Action::CertificateRefused), "{c:?}");
    assert!(!c.attention);
    let j = job("sym-42");
    refusal_outcome(&j, ExitCode::Input, "UNREADABLE_RECORD: line 3");
    let c = classify_job(&j, WaitStatus::Exited(41));
    assert_eq!(
        (c.row, c.action),
        (42, Action::TableUnreadableByTool),
        "{c:?}"
    );
    let j = job("sym-13");
    refusal_outcome(&j, ExitCode::Input, "key does not assign every free index");
    assert_eq!(classify_job(&j, WaitStatus::Exited(41)).row, 13);
    let j = job("sym-15b");
    refusal_outcome(
        &j,
        ExitCode::Io,
        "open table: Too many open files (os error 24)",
    );
    let c = classify_job(&j, WaitStatus::Exited(51));
    assert_eq!((c.row, c.sub.as_str()), (15, "b"), "{c:?}");
    // exit 10 with an OUTCOME: not an exit code of the kind (43)
    let j = job("sym-10");
    let o = JobOutcome::for_spec(
        &j.spec,
        &j.digest,
        program(),
        ExitCode::Incomplete,
        StopInfo {
            code: Some(StopCode::MaxCases),
            ..StopInfo::none()
        },
    );
    Outcome::new(o).write_new(&j.layout.outcome()).unwrap();
    assert_eq!(classify_job(&j, WaitStatus::Exited(10)).row, 43);
    // a signal as for every kind
    let j = job("sym-sig");
    let ev = classify::assess(
        &j.layout,
        &j.spec,
        &j.digest,
        WaitStatus::Signaled(15),
        KillReason::Stop,
        false,
    );
    assert_eq!(classify::classify(&ev).row, 20);
}

#[test]
fn certificate_schema_constant_is_stable() {
    assert_eq!(CERT_SCHEMA, "rustred.campaign.symrows-certificate.v1");
}
