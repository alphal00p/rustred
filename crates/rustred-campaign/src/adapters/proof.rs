//! Adapter (b): `rr_proof instantiate` as a LEGACY child (interfaces/adapters.md 2.3; child-job.md R-d applied to
//! rr_proof): no job mode in the child; the WRAPPER (this module, in the supervisor, after the launcher reaped the child
//! and proved its session empty) checks the child's answers against the requests, seals them into `out/results.jsonl`
//! and writes OUTCOME; then the contract crate classifies the attempt like any other (kind `proof-instantiate`).
//! Answers are bound to their rule by `record_sha256` (SHA-256 of the raw rule line, e13ebcb1) AND the key.

use super::conformance::Mismatch;
use super::job::{
    AdapterError, Ended, InputFile, JobAdapter, JobPlan, Prepared, Program, classify_attempt,
    create_run, is_refusal, write_input, write_spec,
};
use super::solve_recipe::AdapterRefusal;
use rustred_campaign_contract::classify::{Classification, OutcomeState, WaitStatus};
use rustred_campaign_contract::exit::ExitCode;
use rustred_campaign_contract::layout::{RESULTS, RunLayout};
use rustred_campaign_contract::outcome::{
    EndKind, ErrorClass, ErrorInfo, ExtraOutput, JobOutcome, Outcome, ProgramInfo, StopInfo,
};
use rustred_campaign_contract::seal::{RecordWriter, VerifiedHandle};
use rustred_campaign_contract::spec::{
    Budgets, Determinism, JobKind, JobSpec, OnCaseBudget, OutputOptions, ProgramRef, SeedSpec,
};
use rustred_campaign_contract::stop::StopCode;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// The rung tag of proof-instantiate job ids.
pub const RUNG: &str = "pi";
/// The child's output file in `out/` (the wrapper seals its checked lines into `out/results.jsonl`).
pub const CHILD_OUT: &str = "instantiate.jsonl";
/// `schema` of an instantiate answer line.
pub const ANSWER_SCHEMA: &str = "rustred.proof-kernel.instantiate.v1";

/// One fact request: a rule record of a table (by byte offset) at some keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactRequest {
    pub table: InputFile,
    pub offset: u64,
    /// The record's `index` (checked when the request is built).
    pub rule: u64,
    /// SHA-256 (hex) of the raw rule line (without its newline).
    pub record_sha256: String,
    pub keys: Vec<Vec<i64>>,
}

/// Read the rule line at `offset` of `table` (no newline), check its `index`, and build the request.
pub fn fact_request(
    table: &InputFile,
    offset: u64,
    rule: u64,
    keys: Vec<Vec<i64>>,
) -> Result<FactRequest, AdapterError> {
    let mut f = std::fs::File::open(&table.path)
        .map_err(|e| AdapterError::Io(format!("{}: {e}", table.path.display())))?;
    f.seek(SeekFrom::Start(offset))
        .map_err(|e| AdapterError::Io(e.to_string()))?;
    let mut line = Vec::new();
    let mut buf = [0u8; 1 << 16];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| AdapterError::Io(e.to_string()))?;
        if n == 0 {
            break;
        }
        if let Some(p) = buf[..n].iter().position(|&c| c == b'\n') {
            line.extend_from_slice(&buf[..p]);
            break;
        }
        line.extend_from_slice(&buf[..n]);
    }
    let v: Value = serde_json::from_slice(&line)
        .map_err(|e| AdapterError::Request(format!("record at offset {offset}: {e}")))?;
    if v["type"] != "rule" || v["index"].as_u64() != Some(rule) {
        return Err(AdapterError::Request(format!(
            "the record at offset {offset} is not rule {rule}"
        )));
    }
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(&line);
    Ok(FactRequest {
        table: table.clone(),
        offset,
        rule,
        record_sha256: h.finish_hex(),
        keys,
    })
}

/// One import fact-request line (interfaces/import.md 6.3: form (A) + `record_sha256` + `reasons`) -> a request; the
/// record at `offset` is read again and its SHA-256 must equal the line's (a stale request is refused).
pub fn fact_request_from_import(line: &[u8]) -> Result<FactRequest, AdapterError> {
    let v: Value = serde_json::from_slice(line)
        .map_err(|e| AdapterError::Request(format!("fact request: {e}")))?;
    let table = v["table"]
        .as_str()
        .ok_or_else(|| AdapterError::Request("fact request: table".into()))?;
    let (Some(offset), Some(rule), Some(sha)) = (
        v["offset"].as_u64(),
        v["rule"].as_u64(),
        v["record_sha256"].as_str(),
    ) else {
        return Err(AdapterError::Request(
            "fact request: offset / rule / record_sha256".into(),
        ));
    };
    let keys: Vec<Vec<i64>> = serde_json::from_value(v["keys"].clone())
        .map_err(|e| AdapterError::Request(format!("fact request: keys: {e}")))?;
    let r = fact_request(&InputFile::from_file(Path::new(table))?, offset, rule, keys)?;
    if r.record_sha256 != sha {
        return Err(AdapterError::Request(format!(
            "fact request: the record at offset {offset} of {table} has sha256 {} (request: {sha})",
            r.record_sha256
        )));
    }
    Ok(r)
}

/// One answer (instantiate line), bound to its request by (record_sha256, key).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactAnswer {
    pub record_sha256: String,
    pub rule: u64,
    pub key: Vec<i64>,
    pub line: Value,
    /// Request indices this answer serves.
    pub requests: Vec<usize>,
}

/// The output of one proof-instantiate attempt.
#[derive(Debug, Clone)]
pub struct FactBatch {
    pub job_id: String,
    pub classification: Classification,
    pub facts: Vec<FactAnswer>,
    pub refusal: Option<AdapterRefusal>,
    /// The legacy exit status the wrapper saw and the status it gave the table.
    pub legacy_wait: WaitStatus,
    pub mapped_wait: WaitStatus,
}

/// Adapter (b).
pub struct ProofInstantiateAdapter {
    pub program: Program,
    pub licence_env: Option<String>,
}

/// The legacy exit mapping (adapters.md 2.3): 0 -> 0 (sealed only if the answers check), 2 -> 41 (input refused);
/// any other code or a signal stays as it is (no OUTCOME: rows 18-21, 23).
pub fn legacy_wait(w: WaitStatus) -> WaitStatus {
    match w {
        WaitStatus::Exited(2) => WaitStatus::Exited(ExitCode::Input.code()),
        w => w,
    }
}

fn requests_bytes(reqs: &[FactRequest], members: &[usize]) -> Vec<u8> {
    let mut b = Vec::new();
    for &m in members {
        let r = &reqs[m];
        let v = json!({"table": r.table.path.display().to_string(), "offset": r.offset, "rule": r.rule,
                       "keys": r.keys});
        b.extend_from_slice(v.to_string().as_bytes());
        b.push(b'\n');
    }
    b
}

/// One answer line (raw bytes) with its binding.
type CheckedAnswer = (Vec<u8>, FactAnswer);
/// Why the child's answers do not check, with the conformance mismatch when one applies.
type CheckFailure = (String, Option<Mismatch>);

impl ProofInstantiateAdapter {
    /// The wrapper's check of the child's answers against the requests (every answer bound by record_sha256 and a
    /// requested key of a request with that rule; every requested (record_sha256, key) answered exactly once).
    fn check_answers(
        &self,
        reqs: &[FactRequest],
        members: &[usize],
        text: &[u8],
    ) -> Result<Vec<CheckedAnswer>, CheckFailure> {
        let mut want: BTreeMap<(String, Vec<i64>), Vec<usize>> = BTreeMap::new();
        let mut rule_of: BTreeMap<String, u64> = BTreeMap::new();
        for &m in members {
            let r = &reqs[m];
            rule_of.insert(r.record_sha256.clone(), r.rule);
            for k in &r.keys {
                want.entry((r.record_sha256.clone(), k.clone()))
                    .or_default()
                    .push(m);
            }
        }
        let mut seen: BTreeSet<(String, Vec<i64>)> = BTreeSet::new();
        let mut out = Vec::new();
        for (i, l) in text.split(|&c| c == b'\n').enumerate() {
            if l.is_empty() {
                continue;
            }
            let at = |m: String| (format!("answer line {}: {m}", i + 1), None);
            rustred_campaign_contract::canon::canonical_line(l).map_err(|e| at(e.to_string()))?;
            let v: Value = serde_json::from_slice(l).map_err(|e| at(e.to_string()))?;
            if v["schema"] != ANSWER_SCHEMA {
                return Err(at(format!("schema {} is not {ANSWER_SCHEMA}", v["schema"])));
            }
            let Some(sha) = v["record_sha256"].as_str() else {
                return Err((
                    format!("answer line {}: no record_sha256 echo", i + 1),
                    Some(Mismatch::AnswerWithoutBinding {
                        field: "record_sha256".into(),
                    }),
                ));
            };
            let key: Vec<i64> =
                serde_json::from_value(v["key"].clone()).map_err(|e| at(format!("key: {e}")))?;
            let rule = v["rule"].as_u64();
            if rule_of.get(sha).copied() != rule || rule.is_none() {
                return Err(at("rule / record_sha256 of no request".into()));
            }
            let id = (sha.to_string(), key.clone());
            let Some(ms) = want.get(&id) else {
                return Err(at("a key no request asked for".into()));
            };
            if !seen.insert(id.clone()) {
                return Err(at("a (record, key) answered twice".into()));
            }
            out.push((
                l.to_vec(),
                FactAnswer {
                    record_sha256: sha.into(),
                    rule: rule.unwrap_or(0),
                    key,
                    line: v.clone(),
                    requests: ms.clone(),
                },
            ));
        }
        if seen.len() != want.len() {
            return Err((
                format!("{} of {} requested answers present", seen.len(), want.len()),
                None,
            ));
        }
        Ok(out)
    }

    /// The wrapper: seal + OUTCOME for a legacy exit 0 whose answers check; OUTCOME without a sealed file when they
    /// do not (row 22); error OUTCOME for a legacy refusal (2 -> 41); nothing otherwise.
    fn wrap(
        &self,
        reqs: &[FactRequest],
        plan: &JobPlan,
        prep: &Prepared,
        end: &Ended,
    ) -> (WaitStatus, Option<Mismatch>) {
        let layout = &prep.layout;
        let mapped = legacy_wait(end.wait);
        let program = ProgramInfo {
            name: self.program.name.clone(),
            version: "legacy".into(),
            git_commit: rustred_campaign_contract::UNKNOWN_COMMIT.into(),
            features: vec!["legacy-wrapped".into()],
            exe: None,
        };
        let write = |j: JobOutcome| {
            let _ = Outcome::new(j).write_new(&layout.outcome());
        };
        match mapped {
            WaitStatus::Exited(0) => {
                // procfix7 round 5: answers that cannot be READ are never an empty answer set (an io end)
                let raw =
                    match crate::fsx::read("adapters.outcome", &layout.out_dir().join(CHILD_OUT)) {
                        crate::fsx::Read::Content(b) => b,
                        crate::fsx::Read::Absent => Vec::new(),
                        crate::fsx::Read::Failed(e) => {
                            let mut io = JobOutcome::for_spec(
                                &prep.spec,
                                &prep.spec_blake3,
                                program,
                                ExitCode::Io,
                                StopInfo {
                                    code: Some(StopCode::IoError),
                                    ..StopInfo::none()
                                },
                            );
                            io.error = Some(ErrorInfo::new(
                                ErrorClass::Io,
                                format!("the child's answers could not be read: {e}"),
                            ));
                            write(io);
                            return (WaitStatus::Exited(ExitCode::Io.code()), None);
                        }
                    };
                let mut j = JobOutcome::for_spec(
                    &prep.spec,
                    &prep.spec_blake3,
                    program,
                    ExitCode::Complete,
                    StopInfo::none(),
                );
                j.counters.cases = plan.members.len() as u64;
                match self.check_answers(reqs, &plan.members, &raw) {
                    Ok(answers) => {
                        let sealed = (|| -> Result<_, String> {
                            let mut w = RecordWriter::create(&layout.results())
                                .map_err(|e| e.to_string())?;
                            for (l, _) in &answers {
                                w.append(l).map_err(|e| e.to_string())?;
                            }
                            let req_b3 = prep
                                .spec
                                .inputs
                                .iter()
                                .find(|i| i.role == "requests")
                                .map(|i| i.blake3.clone());
                            let end = json!({"type": "end", "records": answers.len(),
                                             "answers": answers.len(), "requests_blake3": req_b3});
                            w.seal(end.to_string().as_bytes())
                                .map_err(|e| e.to_string())
                        })();
                        match sealed {
                            Ok(s) => j.outputs.extra.push(ExtraOutput {
                                role: "results".into(),
                                path: RunLayout::out_rel(RESULTS),
                                bytes: s.bytes,
                                blake3: s.blake3,
                                records: Some(s.records),
                                canonical_blake3: Some(s.canonical_blake3),
                                end: Some(EndKind::End),
                                sealed: true,
                            }),
                            Err(e) => {
                                // gate3 m1: no panic on a closed stderr
                                let _ = writeln!(
                                    std::io::stderr(),
                                    "proof wrapper: sealing failed: {e}"
                                );
                            }
                        }
                        write(j);
                        (mapped, None)
                    }
                    // the child's answers do not check: an OUTCOME without a sealed file (row 22, nothing accepted)
                    Err((why, m)) => {
                        let _ = writeln!(std::io::stderr(), "proof wrapper: {why}");
                        write(j);
                        (mapped, m)
                    }
                }
            }
            WaitStatus::Exited(41) => {
                let mut j = JobOutcome::for_spec(
                    &prep.spec,
                    &prep.spec_blake3,
                    program,
                    ExitCode::Input,
                    StopInfo {
                        code: Some(StopCode::InputError),
                        ..StopInfo::none()
                    },
                );
                // procfix7 round 5: a log that cannot be READ leaves the cause unknown: an io end, never input evidence
                let msg = match crate::fsx::read_to_string("adapters.outcome", &layout.stdout_log())
                {
                    crate::fsx::Read::Content(s) => s
                        .lines()
                        .last()
                        .map(str::to_string)
                        .unwrap_or_else(|| "rr_proof exit 2".into()),
                    crate::fsx::Read::Absent => "rr_proof exit 2".into(),
                    crate::fsx::Read::Failed(e) => {
                        let mut io = JobOutcome::for_spec(
                            &prep.spec,
                            &prep.spec_blake3,
                            j.program.clone(),
                            ExitCode::Io,
                            StopInfo {
                                code: Some(StopCode::IoError),
                                ..StopInfo::none()
                            },
                        );
                        io.error = Some(ErrorInfo::new(
                            ErrorClass::Io,
                            format!("rr_proof exit 2, its log could not be read: {e}"),
                        ));
                        write(io);
                        return (WaitStatus::Exited(ExitCode::Io.code()), None);
                    }
                };
                j.error = Some(ErrorInfo::new(ErrorClass::Input, msg));
                write(j);
                (mapped, None)
            }
            w => (w, None),
        }
    }
}

impl JobAdapter for ProofInstantiateAdapter {
    type Request = FactRequest;
    type Output = FactBatch;

    fn kind(&self) -> JobKind {
        JobKind::ProofInstantiate
    }

    fn plan(&self, reqs: &[FactRequest]) -> Result<Vec<JobPlan>, AdapterError> {
        let mut groups: Vec<((String, String), Vec<usize>)> = Vec::new();
        for (i, r) in reqs.iter().enumerate() {
            if r.keys.is_empty() {
                return Err(AdapterError::Request(format!("request {i}: no key")));
            }
            let id = (r.table.path.display().to_string(), r.table.blake3.clone());
            match groups.iter_mut().find(|(g, _)| *g == id) {
                Some((_, v)) => v.push(i),
                None => groups.push((id, vec![i])),
            }
        }
        Ok(groups
            .into_iter()
            .map(|(_, members)| {
                let id = json!({"kind": "proof-instantiate", "program": self.program.blake3,
                    "requests": rustred_campaign_contract::blake3_hex(&requests_bytes(reqs, &members))});
                let content =
                    rustred_campaign_contract::blake3_hex(id.to_string().as_bytes())[..16].to_string();
                JobPlan {
                    job_id: rustred_campaign_contract::layout::job_id(&content, RUNG),
                    members,
                    content,
                }
            })
            .collect())
    }

    fn prepare(
        &self,
        reqs: &[FactRequest],
        plan: &JobPlan,
        work_root: &Path,
        attempt: u32,
    ) -> Result<Prepared, AdapterError> {
        let root = std::path::absolute(work_root).map_err(|e| AdapterError::Io(e.to_string()))?;
        let first = plan
            .members
            .first()
            .ok_or_else(|| AdapterError::Request("empty plan".into()))?;
        let layout = create_run(&root, &plan.job_id, attempt)?;
        let rq = write_input(
            &layout,
            "requests.jsonl",
            "requests",
            &requests_bytes(reqs, &plan.members),
        )?;
        let spec = JobSpec {
            schema: rustred_campaign_contract::CONTRACT.into(),
            job_id: plan.job_id.clone(),
            attempt,
            kind: JobKind::ProofInstantiate,
            run_dir: layout.root.display().to_string(),
            program: ProgramRef {
                name: self.program.name.clone(),
                blake3: self.program.blake3.clone(),
                features_required: vec![],
            },
            inputs: vec![rq, reqs[*first].table.input("table")],
            args: vec![],
            seed: SeedSpec::default(),
            budgets: Budgets::default(),
            on_case_budget: OnCaseBudget::Stop,
            determinism: Determinism::Recorded,
            output: OutputOptions::default(),
            heartbeat_s: 10,
            stop_grace_s: 5,
            resume_every_s: 30,
            resume: None,
            env_allow: vec![],
            fake: None,
        };
        let argv = vec![
            "instantiate".to_string(),
            "--requests".into(),
            layout.in_dir().join("requests.jsonl").display().to_string(),
            "--out".into(),
            layout.out_dir().join(CHILD_OUT).display().to_string(),
        ];
        write_spec(&spec, &self.program, self.licence_env.as_deref(), &argv)
    }

    fn finish(
        &self,
        reqs: &[FactRequest],
        plan: &JobPlan,
        prep: &Prepared,
        end: &Ended,
    ) -> FactBatch {
        let (mapped, binding) = self.wrap(reqs, plan, prep, end);
        let e2 = Ended {
            wait: mapped,
            kill: end.kill,
            oom: end.oom,
        };
        let (ev, c) = classify_attempt(prep, &e2);
        let mut out = FactBatch {
            job_id: plan.job_id.clone(),
            classification: c.clone(),
            facts: vec![],
            refusal: None,
            legacy_wait: end.wait,
            mapped_wait: mapped,
        };
        let refuse = |why: String, conf: Vec<Mismatch>| AdapterRefusal {
            row: c.row,
            sub: c.sub.clone(),
            why,
            unsealed_why: ev.unsealed_why.clone(),
            conformance: conf,
        };
        let lines = match (&ev.outcome, &ev.verified, c.row) {
            (OutcomeState::Valid(_), Some(v), 1) => v
                .streams
                .iter()
                .find(|s| s.rel == RunLayout::out_rel(RESULTS))
                .ok_or_else(|| "no verified results stream".to_string())
                .and_then(|s| {
                    VerifiedHandle::reopen(&prep.layout.root, s)
                        .and_then(|h| h.canonical_lines())
                        .map_err(|e| e.to_string())
                }),
            _ => Err(c.why.clone()),
        };
        let lines = match lines {
            Ok(l) => l,
            Err(why) => {
                if is_refusal(&c) {
                    out.refusal = Some(refuse(why, binding.into_iter().collect()));
                }
                return out;
            }
        };
        // answers through the verified handle, bound again to the requests (the end record is last)
        let body: Vec<u8> = lines[..lines.len().saturating_sub(1)]
            .iter()
            .flat_map(|l| l.iter().copied().chain(std::iter::once(b'\n')))
            .collect();
        match self.check_answers(reqs, &plan.members, &body) {
            Ok(a) => out.facts = a.into_iter().map(|(_, f)| f).collect(),
            Err((why, m)) => out.refusal = Some(refuse(why, m.into_iter().collect())),
        }
        out
    }
}
