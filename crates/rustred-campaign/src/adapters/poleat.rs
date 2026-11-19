//! Adapter of kind `pole-regular-at` (lane coreseed; interfaces/coreseed.md 10): lane poles' `rr_poles regular-at` as
//! a LEGACY DIAGNOSTIC child under the job contract. One job = MANY (certified relation record, key) pairs, written into
//! the pinned `requests` input; the WRAPPER (here, in the supervisor, after the launcher reaped the child) checks every
//! answer against its request ([`poleat::check_answers`]), seals `out/results.jsonl` and writes OUTCOME; the contract
//! classifies (rows 37 / 38 / 39, 15b, signals) and the answers are read ONLY through the verified handle (accept mode
//! `diagnostic_records`: never rule or relation evidence). A job that gives nothing readable leaves every relation
//! `undecided` (never attention, never a held key). Generic: keys are integer vectors of any length.

use super::job::{
    AdapterError, Ended, JobAdapter, JobPlan, Prepared, Program, classify_attempt, create_run,
    write_input, write_spec,
};
use super::proof::legacy_wait;
use rustred_campaign_contract::classify::poleat::{self, PoleAnswer};
use rustred_campaign_contract::classify::{Classification, WaitStatus};
use rustred_campaign_contract::exit::ExitCode;
use rustred_campaign_contract::fake::FakeScript;
use rustred_campaign_contract::layout::{RESULTS, RunLayout};
use rustred_campaign_contract::outcome::{
    EndKind, ErrorClass, ErrorInfo, ExtraOutput, JobOutcome, Outcome, ProgramInfo, StopInfo,
};
use rustred_campaign_contract::seal::RecordWriter;
use rustred_campaign_contract::spec::{
    Budgets, Determinism, JobKind, JobSpec, OnCaseBudget, OutputOptions, ProgramRef, SeedSpec,
};
use rustred_campaign_contract::stop::StopCode;
use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;

/// The rung tag of pole-regular-at job ids.
pub const RUNG: &str = "pr";

/// One (relation, key) pair: the certified engine record (verified canonical JSON) at its key.
#[derive(Debug, Clone, PartialEq)]
pub struct PoleRequest {
    pub id: String,
    pub key: Vec<i32>,
    pub record: Value,
}

/// The adapter (data).
#[derive(Debug, Clone)]
pub struct PoleAdapter {
    pub program: Program,
    pub d0: String,
    pub threads: u32,
    pub licence_env: Option<String>,
    /// Test stand: the fake child emulates the kind.
    pub fake: Option<FakeScript>,
}

/// The output of one attempt: the checked answers (row 37) or why nothing was read.
#[derive(Debug, Clone)]
pub struct PoleBatch {
    pub job_id: String,
    pub classification: Classification,
    pub answers: Vec<PoleAnswer>,
    /// Why no answer was read (rows 38 / 39, 15b, signals, a refused attempt): the relations stay undecided.
    pub unread: Option<String>,
}

/// The request bytes (one `rustred.campaign.pole-request.v1` line per pair, in order).
pub fn requests_bytes(reqs: &[PoleRequest], members: &[usize]) -> Vec<u8> {
    let mut b = Vec::new();
    for &m in members {
        let r = &reqs[m];
        let v =
            json!({"schema": poleat::REQUEST_SCHEMA, "id": r.id, "key": r.key, "record": r.record});
        b.extend_from_slice(v.to_string().as_bytes());
        b.push(b'\n');
    }
    b
}

fn program_info(spec: &JobSpec) -> ProgramInfo {
    ProgramInfo {
        name: spec.program.name.clone(),
        version: "legacy".into(),
        git_commit: rustred_campaign_contract::UNKNOWN_COMMIT.into(),
        features: vec!["legacy-wrapped".into()],
        exe: None,
    }
}

/// THE WRAPPER of the kind (before the contract classifies; also the supervisor settle's dispatch from
/// `legacy::wrap`): a no-op for an unknown wait status and when OUTCOME exists. Legacy exit 0: the pinned requests
/// re-read (digest = the spec's, else an error OUTCOME 41 naming [`poleat::INPUT_MISMATCH`]: row 39), every answer
/// checked against its request; checked answers sealed + OUTCOME (row 37), otherwise OUTCOME without a sealed stream
/// (row 38). Legacy exit 2 (rr_poles' refusal): error OUTCOME 41 (row 39), or 51 when the message names a host
/// resource errno (row 15b). Anything else stays as it is (signals, no OUTCOME).
pub fn wrap(layout: &RunLayout, spec: &JobSpec, spec_blake3: &str, wait: WaitStatus) -> WaitStatus {
    if matches!(wait, WaitStatus::Unknown) {
        return legacy_wait(wait);
    }
    // procfix7 round 19 (landing, legacy::wrap's form): OUTCOME is written only when PROVEN absent (an unreadable one is
    // left to the classification, which reads it: never a second outcome written on a failed stat)
    match crate::fsx::presence_lstat("adapters.outcome", &layout.outcome()) {
        crate::fsx::Presence::Absent => {}
        crate::fsx::Presence::Present(_) | crate::fsx::Presence::Unknown(_) => {
            return legacy_wait(wait);
        }
    }
    let mapped = legacy_wait(wait);
    let write = |j: JobOutcome| {
        if let Err(e) = Outcome::new(j).write_new(&layout.outcome()) {
            let _ = writeln!(std::io::stderr(), "pole wrapper: OUTCOME: {e}");
        }
    };
    let input_error = |msg: String| -> WaitStatus {
        let mut j = JobOutcome::for_spec(
            spec,
            spec_blake3,
            program_info(spec),
            ExitCode::Input,
            StopInfo {
                code: Some(StopCode::InputError),
                ..StopInfo::none()
            },
        );
        j.error = Some(ErrorInfo::new(ErrorClass::Input, msg.clone()));
        if rustred_campaign_contract::classify::resource_errno_of(&j).is_some() {
            let mut io = JobOutcome::for_spec(
                spec,
                spec_blake3,
                program_info(spec),
                ExitCode::Io,
                StopInfo {
                    code: Some(StopCode::IoError),
                    ..StopInfo::none()
                },
            );
            io.error = Some(ErrorInfo::new(ErrorClass::Io, msg));
            write(io);
            return WaitStatus::Exited(ExitCode::Io.code());
        }
        write(j);
        WaitStatus::Exited(ExitCode::Input.code())
    };
    // procfix7 round 19 (landing): a file that cannot be READ is an io end (lost, retried after the back-off), never
    // evidence about the requests (legacy::wrap's form)
    let io_error = |msg: String| -> WaitStatus {
        let mut io = JobOutcome::for_spec(
            spec,
            spec_blake3,
            program_info(spec),
            ExitCode::Io,
            StopInfo {
                code: Some(StopCode::IoError),
                ..StopInfo::none()
            },
        );
        io.error = Some(ErrorInfo::new(ErrorClass::Io, msg));
        write(io);
        WaitStatus::Exited(ExitCode::Io.code())
    };
    match mapped {
        WaitStatus::Exited(0) => {
            let Some(rq) = spec.inputs.iter().find(|i| i.role == "requests") else {
                return input_error("no requests input".into());
            };
            let requests = match std::fs::read(spec.input_path(rq)) {
                Ok(b) => b,
                Err(e) => return input_error(format!("requests input: {e}")),
            };
            if rustred_campaign_contract::blake3_hex(&requests) != rq.blake3 {
                return input_error(format!(
                    "{}: the requests input is not the spec's",
                    poleat::INPUT_MISMATCH
                ));
            }
            // procfix7 round 19 (landing): answers that cannot be READ are never an empty answer set (absent = none)
            let raw = match crate::fsx::read(
                "adapters.outcome",
                &layout.out_dir().join(poleat::CHILD_OUT),
            ) {
                crate::fsx::Read::Content(b) => b,
                crate::fsx::Read::Absent => Vec::new(),
                crate::fsx::Read::Failed(e) => {
                    return io_error(format!("the child's answers could not be read: {e}"));
                }
            };
            let mut j = JobOutcome::for_spec(
                spec,
                spec_blake3,
                program_info(spec),
                ExitCode::Complete,
                StopInfo::none(),
            );
            match poleat::check_answers(&requests, &raw) {
                Ok(answers) => {
                    j.counters.cases = answers.len() as u64;
                    let sealed = (|| -> Result<_, String> {
                        let mut w =
                            RecordWriter::create(&layout.results()).map_err(|e| e.to_string())?;
                        for a in &answers {
                            w.append(&a.canonical).map_err(|e| e.to_string())?;
                        }
                        let end = json!({"type": "end", "records": answers.len(), "requests_blake3": rq.blake3});
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
                            let _ =
                                writeln!(std::io::stderr(), "pole wrapper: sealing failed: {e}");
                        }
                    }
                    write(j);
                }
                Err(why) => {
                    // OUTCOME without a sealed stream: row 38 (nothing read, the relations stay undecided)
                    let _ = std::fs::write(layout.sup_dir().join("wrapper-refusal.txt"), &why);
                    write(j);
                }
            }
            mapped
        }
        WaitStatus::Exited(41) => {
            // procfix7 round 19 (landing): a log that cannot be READ leaves the cause unknown: an io end, never input
            // evidence
            let msg = match crate::fsx::read_to_string("adapters.outcome", &layout.stdout_log()) {
                crate::fsx::Read::Content(s) => s
                    .lines()
                    .last()
                    .map(str::to_string)
                    .unwrap_or_else(|| "rr_poles exit 2".into()),
                crate::fsx::Read::Absent => "rr_poles exit 2".into(),
                crate::fsx::Read::Failed(e) => {
                    return io_error(format!("rr_poles exit 2, its log could not be read: {e}"));
                }
            };
            input_error(msg)
        }
        w => w,
    }
}

impl JobAdapter for PoleAdapter {
    type Request = PoleRequest;
    type Output = PoleBatch;

    fn kind(&self) -> JobKind {
        JobKind::PoleRegularAt
    }

    /// ONE job for all requests (many pairs per invocation).
    fn plan(&self, reqs: &[PoleRequest]) -> Result<Vec<JobPlan>, AdapterError> {
        if reqs.is_empty() {
            return Ok(vec![]);
        }
        let members: Vec<usize> = (0..reqs.len()).collect();
        let id = json!({"kind": "pole-regular-at", "program": self.program.blake3, "d0": self.d0,
            "requests": rustred_campaign_contract::blake3_hex(&requests_bytes(reqs, &members))});
        let content =
            rustred_campaign_contract::blake3_hex(id.to_string().as_bytes())[..16].to_string();
        Ok(vec![JobPlan {
            job_id: rustred_campaign_contract::layout::job_id(&content, RUNG),
            members,
            content,
        }])
    }

    fn prepare(
        &self,
        reqs: &[PoleRequest],
        plan: &JobPlan,
        work_root: &Path,
        attempt: u32,
    ) -> Result<Prepared, AdapterError> {
        let root = std::path::absolute(work_root).map_err(|e| AdapterError::Io(e.to_string()))?;
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
            kind: JobKind::PoleRegularAt,
            run_dir: layout.root.display().to_string(),
            program: ProgramRef {
                name: self.program.name.clone(),
                blake3: self.program.blake3.clone(),
                features_required: vec![],
            },
            inputs: vec![rq],
            args: vec![
                "--d0".into(),
                self.d0.clone(),
                "--threads".into(),
                self.threads.clamp(1, 4).to_string(),
            ],
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
            fake: self.fake.clone(),
        };
        let argv = poleat::argv(&spec).map_err(AdapterError::Spec)?;
        write_spec(&spec, &self.program, self.licence_env.as_deref(), &argv)
    }

    fn finish(
        &self,
        _reqs: &[PoleRequest],
        plan: &JobPlan,
        prep: &Prepared,
        end: &Ended,
    ) -> PoleBatch {
        let mapped = wrap(&prep.layout, &prep.spec, &prep.spec_blake3, end.wait);
        let e2 = Ended {
            wait: mapped,
            kill: end.kill,
            oom: end.oom,
        };
        let (ev, c) = classify_attempt(prep, &e2);
        let mut out = PoleBatch {
            job_id: plan.job_id.clone(),
            classification: c.clone(),
            answers: vec![],
            unread: None,
        };
        if c.row != 37 {
            out.unread = Some(format!(
                "row {}{}: {}{}",
                c.row,
                c.sub,
                c.why,
                ev.unsealed_why
                    .map(|w| format!(" ({w})"))
                    .unwrap_or_default()
            ));
            return out;
        }
        match ev.verified.as_ref().map(|v| {
            rustred_campaign_contract::classify::pole_regular_at_answers(
                &prep.layout,
                &prep.spec,
                v,
            )
        }) {
            Some(Ok(a)) => out.answers = a,
            Some(Err(why)) => out.unread = Some(why),
            None => out.unread = Some("no verified outputs".into()),
        }
        out
    }
}
