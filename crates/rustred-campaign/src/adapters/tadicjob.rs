//! Adapter of kind `tadic-replay` (lane tadicjob; interfaces/tadic-job.md): helper VH 2's frozen t-adic replay driver as
//! a LEGACY DIAGNOSTIC child under the job contract. One [`TadicJobRequest`] = one driver job (keys or weighted groups at
//! package or node level); the adapter writes the driver's job file as the pinned input `tadic_job` (the frozen binary
//! pinned by sha256 inside it), the frozen driver as the pinned input `driver`, and the terminal / master set, the
//! dependence and known-depth tables as explicit inputs of the record. The WRAPPER (here, after the launcher reaped the
//! child) re-hashes every pinned input, binds the driver's result to the job (`tadicjob::check_result`: sha256 of the job
//! bytes, labels, binary and driver digests; every answer re-derived from the per-prime series), maps the driver's exit
//! (`tadicjob::decide`: rows 50 / 51 / 52, 9, 10, 17) and seals one line per request. Answers are read ONLY through the
//! verified handle (accept mode `diagnostic_records`): never rule or relation evidence. Resumable: the content id is
//! deterministic in the job's content, and a verified attempt can be kept in a [`ResultStore`].
//!
//! Generic (L8): nodes, keys, masters, primes, seeds, packages and tables are data.

use super::job::{
    AdapterError, Ended, InputFile, JobAdapter, JobPlan, Prepared, Program, classify_attempt,
    create_run, sha256_hex, write_input, write_spec,
};
use rustred_campaign_contract::classify::{Classification, WaitStatus};
use rustred_campaign_contract::exit::ExitCode;
use rustred_campaign_contract::fake::FakeScript;
use rustred_campaign_contract::layout::{RESULTS, RunLayout};
use rustred_campaign_contract::outcome::{
    EndKind, ErrorClass, ErrorInfo, ExtraOutput, JobOutcome, Outcome, ProgramInfo, StopInfo,
};
use rustred_campaign_contract::seal::RecordWriter;
use rustred_campaign_contract::spec::{
    Budgets, Determinism, InputRef, JobKind, JobSpec, OnCaseBudget, OutputOptions, ProgramRef,
    SeedSpec,
};
use rustred_campaign_contract::stop::StopCode;
use rustred_campaign_contract::tadicjob::{
    self, CheckedResult, Pins, RequestOutcome, WrapDecision,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};

/// The rung tag of tadic-replay job ids.
pub const RUNG: &str = "tr";

/// One input of a request: a package key (`top` = family name or `pres:<id>`, `powers` in its frame) or a node key
/// (`key` = `I(..)` text in the table's frame), with an integer weight constant in d.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TadicInput {
    Package {
        top: String,
        powers: Vec<i64>,
        weight: i64,
    },
    Node {
        key: String,
        weight: i64,
    },
}

/// One request = ONE weighted sum of inputs (a single key has one input of weight 1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TadicRequest {
    pub label: String,
    pub inputs: Vec<TadicInput>,
}

/// The level of a job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "level", rename_all = "snake_case")]
pub enum TadicLevel {
    /// A package directory (`load_p_library`), its shipped relation root and its `library.json` (bound by sha256).
    Package {
        path: PathBuf,
        relations: Option<PathBuf>,
        library_sha256: String,
    },
    /// ONE candidate table (bound by sha256), the node's masters (None = the table's own), exits kept or not.
    Node {
        table: PathBuf,
        table_sha256: String,
        masters: Option<Vec<String>>,
        keep_exits: bool,
        allow_ignored_records: bool,
    },
}

/// Limits of one driver job (interfaces/tadic-job.md 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TadicLimits {
    pub wall_seconds: u64,
    pub max_length: u64,
    pub threads: u32,
    /// The driver's own systemd scope (`limits.memory_gib`): only outside a private pid namespace.
    pub memory_gib: Option<u64>,
    pub no_step_cache: bool,
}

/// An explicit data input of the record: path, raw BLAKE3 (contract pin) and SHA-256 (the record's binding).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataFile {
    pub path: PathBuf,
    pub blake3: String,
    pub sha256: String,
}

impl DataFile {
    pub fn from_file(path: &Path) -> Result<DataFile, AdapterError> {
        let b = std::fs::read(path)
            .map_err(|e| AdapterError::Io(format!("{}: {e}", path.display())))?;
        Ok(DataFile {
            path: path.to_path_buf(),
            blake3: rustred_campaign_contract::blake3_hex(&b),
            sha256: sha256_hex(&b),
        })
    }
    fn input(&self, role: &str) -> InputRef {
        InputRef {
            role: role.into(),
            path: self.path.display().to_string(),
            blake3: self.blake3.clone(),
            upto: None,
        }
    }
}

/// One driver job (the adapter's request).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TadicJobRequest {
    pub level: TadicLevel,
    pub requests: Vec<TadicRequest>,
    pub d0: i64,
    pub order: i64,
    pub primes: u32,
    pub max_primes: u32,
    pub seeds: Vec<u64>,
    pub limits: TadicLimits,
    pub crt: bool,
    /// The terminal / master set (explicit input of the record, TRICKS 34).
    pub terminals: Option<DataFile>,
    /// The dependence table (bound by sha256 in the driver job too; NOT applied by the driver).
    pub dependences: Option<DataFile>,
    /// The known-depth table (used by the rank step; bound here so the record names it).
    pub depths: Option<DataFile>,
}

/// The adapter (data).
#[derive(Debug, Clone)]
pub struct TadicAdapter {
    /// The program file: the Python interpreter that runs the frozen driver.
    pub program: Program,
    /// The frozen driver `tadic_job.py` (raw BLAKE3 and SHA-256 pinned).
    pub driver: DataFile,
    /// The frozen binaries the jobs pin (package level, node level) with their SHA-256.
    pub package_binary: (PathBuf, String),
    pub node_binary: (PathBuf, String),
    pub licence_env: Option<String>,
    /// Test stand: the fake child emulates the kind.
    pub fake: Option<FakeScript>,
}

/// The output of one attempt.
#[derive(Debug, Clone)]
pub struct TadicBatch {
    pub job_id: String,
    pub classification: Classification,
    /// The bound, structurally checked result (row 50 only).
    pub result: Option<CheckedResult>,
    /// The raw result JSON (row 50 only; for the certificate checker and the rank step).
    pub result_json: Option<Value>,
    /// Why nothing was read (rows 51 / 52, 9, 10, 17, signals): every request stays not decided.
    pub unread: Option<String>,
}

impl TadicBatch {
    /// Per request label: the typed outcome (NOT_DECIDED with the attempt's reason when nothing was read).
    pub fn outcomes(&self, labels: &[String]) -> Vec<(String, RequestOutcome)> {
        match &self.result {
            Some(r) => r
                .requests
                .iter()
                .map(|q| (q.label.clone(), q.outcome.clone()))
                .collect(),
            None => {
                let code = match self.classification.row {
                    9 | 19 => "TIMEOUT".to_string(),
                    10 | 18 => "OOM_KILLED".to_string(),
                    52 => "JOB_REFUSED".to_string(),
                    r => format!("NOT_VERIFIED:row{r}"),
                };
                labels
                    .iter()
                    .map(|l| {
                        (
                            l.clone(),
                            RequestOutcome::NotDecided {
                                code: code.clone(),
                                message: self.unread.clone().unwrap_or_default(),
                            },
                        )
                    })
                    .collect()
            }
        }
    }
}

impl TadicAdapter {
    fn binary(&self, level: &TadicLevel) -> &(PathBuf, String) {
        match level {
            TadicLevel::Package { .. } => &self.package_binary,
            TadicLevel::Node { .. } => &self.node_binary,
        }
    }

    /// The driver job file of `req` (canonical key order by serde_json's map; `job_id` = the plan's job id).
    pub fn driver_job(&self, req: &TadicJobRequest, job_id: &str) -> Value {
        let (bin, bin_sha) = self.binary(&req.level);
        let mut limits = json!({"wall_seconds": req.limits.wall_seconds, "max_length": req.limits.max_length,
            "threads": req.limits.threads});
        if let Some(g) = req.limits.memory_gib {
            limits["memory_gib"] = json!(g);
        }
        if req.limits.no_step_cache {
            limits["no_step_cache"] = json!(true);
        }
        let requests: Vec<Value> = req
            .requests
            .iter()
            .map(|r| json!({"label": r.label, "inputs": r.inputs.iter().map(|i| match i {
                TadicInput::Package { top, powers, weight } => json!({"top": top, "powers": powers, "weight": weight}),
                TadicInput::Node { key, weight } => json!({"key": key, "weight": weight}),
            }).collect::<Vec<_>>()}))
            .collect();
        let mut job = json!({"schema": tadicjob::JOB_SCHEMA, "job_id": job_id,
            "requests": requests, "expansion": {"d0": req.d0, "order": req.order},
            "primes": {"count": req.primes, "max_primes": req.max_primes, "seeds": req.seeds},
            "limits": limits, "crt": req.crt,
            "binary": {"path": bin.display().to_string(), "sha256": bin_sha}});
        match &req.level {
            TadicLevel::Package {
                path,
                relations,
                library_sha256,
            } => {
                job["level"] = json!("package");
                let mut p = json!({"path": path.display().to_string()});
                if let Some(r) = relations {
                    p["relations"] = json!(r.display().to_string());
                }
                job["package"] = p;
                job["inputs_sha256"] =
                    json!({path.join("library.json").display().to_string(): library_sha256});
            }
            TadicLevel::Node {
                table,
                table_sha256,
                masters,
                keep_exits,
                allow_ignored_records,
            } => {
                job["level"] = json!("node");
                let mut n = json!({"table": table.display().to_string(), "keep_exits": keep_exits,
                    "allow_ignored_records": allow_ignored_records});
                if let Some(m) = masters {
                    n["masters"] = json!(m);
                }
                job["node"] = n;
                job["inputs_sha256"] = json!({table.display().to_string(): table_sha256});
            }
        }
        if let Some(d) = &req.dependences {
            job["dependence_table"] =
                json!({"path": d.path.display().to_string(), "sha256": d.sha256});
        }
        job
    }

    /// The content id of a request: BLAKE3 of everything that changes the answers (no run path).
    pub fn content_of(&self, req: &TadicJobRequest) -> String {
        let (_, bin_sha) = self.binary(&req.level);
        let level = match &req.level {
            TadicLevel::Package {
                library_sha256,
                relations,
                ..
            } => {
                json!({"package": library_sha256, "relations": relations.is_some()})
            }
            TadicLevel::Node {
                table_sha256,
                masters,
                keep_exits,
                allow_ignored_records,
                ..
            } => json!({"table": table_sha256, "masters": masters, "keep_exits": keep_exits,
                "allow_ignored_records": allow_ignored_records}),
        };
        let id = json!({"kind": "tadic-replay", "binary": bin_sha, "driver": self.driver.sha256, "level": level,
            "requests": req.requests, "d0": req.d0, "order": req.order, "primes": req.primes,
            "max_primes": req.max_primes, "seeds": req.seeds, "max_length": req.limits.max_length, "crt": req.crt,
            "terminals": req.terminals.as_ref().map(|d| &d.sha256),
            "dependences": req.dependences.as_ref().map(|d| &d.sha256),
            "depths": req.depths.as_ref().map(|d| &d.sha256)});
        rustred_campaign_contract::blake3_hex(id.to_string().as_bytes())[..16].to_string()
    }

    /// The pins of an attempt: the job bytes' SHA-256 and the frozen binary / driver SHA-256 (the adapter's own values).
    pub fn pins(&self, job_bytes: &[u8], level: &TadicLevel) -> Pins {
        Pins {
            job_sha256: sha256_hex(job_bytes),
            binary_sha256: self.binary(level).1.clone(),
            driver_sha256: self.driver.sha256.clone(),
        }
    }
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

/// The SHA-256 of the frozen binary named by a driver job (`binary.sha256`), the adapter's pin.
fn pinned_binary(job: &Value) -> Option<String> {
    job["binary"]["sha256"].as_str().map(str::to_string)
}

/// THE WRAPPER of the kind (before the contract classifies; also the supervisor settle's dispatch from
/// `legacy::wrap`): a no-op for an unknown wait status and when OUTCOME exists. Raw driver exits 0 / 1 / 2 / 3: every
/// pinned input re-hashed (a mismatch is OUTCOME 41 naming `TADIC_INPUT_MISMATCH`: row 52), the result bound to the job
/// and checked (`tadicjob::check_result`), the decision of `tadicjob::decide` written as OUTCOME (with the sealed
/// request lines for row 50). Signals and any other exit stay as they are.
pub fn wrap(layout: &RunLayout, spec: &JobSpec, spec_blake3: &str, wait: WaitStatus) -> WaitStatus {
    if matches!(wait, WaitStatus::Unknown) || layout.outcome().exists() {
        return wait;
    }
    let WaitStatus::Exited(driver_exit) = wait else {
        return wait;
    };
    if !(0..=3).contains(&driver_exit) {
        return wait;
    }
    let write = |j: JobOutcome| {
        if let Err(e) = Outcome::new(j).write_new(&layout.outcome()) {
            let _ = writeln!(std::io::stderr(), "tadic wrapper: OUTCOME: {e}");
        }
    };
    let ending = |code: ExitCode, stop: StopCode, class: Option<ErrorClass>, msg: String| {
        let mut j = JobOutcome::for_spec(
            spec,
            spec_blake3,
            program_info(spec),
            code,
            StopInfo {
                code: Some(stop),
                ..StopInfo::none()
            },
        );
        if stop == StopCode::BudgetWall {
            j.time_decided = true;
        }
        if code == ExitCode::BudgetWall || code == ExitCode::BudgetMemory {
            j.stop.graceful = true;
        }
        if let Some(c) = class {
            j.error = Some(ErrorInfo::new(c, msg));
        }
        write(j);
        WaitStatus::Exited(code.code())
    };
    // every pinned input re-read and re-hashed (the job input is needed below)
    let mut job_bytes = None;
    for i in &spec.inputs {
        match std::fs::read(spec.input_path(i)) {
            Ok(b) if rustred_campaign_contract::blake3_hex(&b) == i.blake3 => {
                if i.role == tadicjob::ROLE_JOB {
                    job_bytes = Some(b);
                }
            }
            Ok(_) => {
                return ending(
                    ExitCode::Input,
                    StopCode::InputError,
                    Some(ErrorClass::Input),
                    format!("{}: input {} is not the spec's", tadicjob::INPUT_MISMATCH, i.role),
                );
            }
            Err(e) => {
                return ending(
                    ExitCode::Input,
                    StopCode::InputError,
                    Some(ErrorClass::Input),
                    format!("{}: input {}: {e}", tadicjob::INPUT_MISMATCH, i.role),
                );
            }
        }
    }
    let Some(job_bytes) = job_bytes else {
        return ending(
            ExitCode::Input,
            StopCode::InputError,
            Some(ErrorClass::Input),
            format!("{}: no tadic_job input", tadicjob::INPUT_MISMATCH),
        );
    };
    let job: Value = match serde_json::from_slice(&job_bytes) {
        Ok(v) => v,
        Err(e) => {
            return ending(
                ExitCode::Input,
                StopCode::InputError,
                Some(ErrorClass::Input),
                format!("{}: job input: {e}", tadicjob::INPUT_MISMATCH),
            );
        }
    };
    let driver_sha = spec
        .inputs
        .iter()
        .find(|i| i.role == tadicjob::ROLE_DRIVER)
        .and_then(|i| std::fs::read(spec.input_path(i)).ok())
        .map(|b| sha256_hex(&b))
        .unwrap_or_default();
    let pins = Pins {
        job_sha256: sha256_hex(&job_bytes),
        binary_sha256: pinned_binary(&job).unwrap_or_default(),
        driver_sha256: driver_sha,
    };
    let result_path = layout.out_dir().join(tadicjob::CHILD_OUT);
    let result_bytes = std::fs::read(&result_path);
    let checked: Result<CheckedResult, String> = match &result_bytes {
        Ok(b) => serde_json::from_slice::<Value>(b)
            .map_err(|e| format!("result: {e}"))
            .and_then(|r| tadicjob::check_result(&job, &r, &pins)),
        Err(e) => Err(format!("no result file: {e}")),
    };
    match tadicjob::decide(driver_exit, &checked) {
        WrapDecision::Seal => {
            let (Ok(c), Ok(rb)) = (&checked, &result_bytes) else {
                unreachable!("decide seals only a checked result")
            };
            let mut j = JobOutcome::for_spec(
                spec,
                spec_blake3,
                program_info(spec),
                ExitCode::Complete,
                StopInfo::none(),
            );
            j.counters.cases = c.requests.len() as u64;
            let sealed = (|| -> Result<_, String> {
                let mut w = RecordWriter::create(&layout.results()).map_err(|e| e.to_string())?;
                for q in &c.requests {
                    let ts = match &q.outcome {
                        RequestOutcome::Series { terms, .. } => {
                            Some(sha256_hex(tadicjob::terms_json(terms).to_string().as_bytes()))
                        }
                        RequestOutcome::NotDecided { .. } => None,
                    };
                    let line = tadicjob::request_line(q, ts.as_deref());
                    let canon = rustred_campaign_contract::canonical_line(line.to_string().as_bytes())
                        .map_err(|e| format!("{e:?}"))?;
                    w.append(&canon).map_err(|e| e.to_string())?;
                }
                let end = json!({"type": "end", "records": c.requests.len(),
                    "job_blake3": rustred_campaign_contract::blake3_hex(&job_bytes), "job_sha256": pins.job_sha256,
                    "result_blake3": rustred_campaign_contract::blake3_hex(rb), "result_sha256": sha256_hex(rb),
                    "result_status": c.status, "error_code": c.error_code});
                w.seal(end.to_string().as_bytes()).map_err(|e| e.to_string())
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
                    let _ = std::fs::write(layout.sup_dir().join("wrapper-refusal.txt"), &e);
                }
            }
            write(j);
            WaitStatus::Exited(0)
        }
        WrapDecision::Unverified { why } => {
            // OUTCOME without a sealed stream: row 51 (nothing read)
            let _ = std::fs::write(layout.sup_dir().join("wrapper-refusal.txt"), &why);
            let j = JobOutcome::for_spec(
                spec,
                spec_blake3,
                program_info(spec),
                ExitCode::Complete,
                StopInfo::none(),
            );
            write(j);
            WaitStatus::Exited(0)
        }
        WrapDecision::Killed { exit: 21, why } => {
            ending(ExitCode::BudgetWall, StopCode::BudgetWall, None, why)
        }
        WrapDecision::Killed { why, .. } => {
            ending(ExitCode::BudgetMemory, StopCode::BudgetMemory, None, why)
        }
        WrapDecision::Refused { why } => ending(
            ExitCode::Input,
            StopCode::InputError,
            Some(ErrorClass::Input),
            why,
        ),
        WrapDecision::Crashed { why } => ending(
            ExitCode::Internal,
            StopCode::InternalError,
            Some(ErrorClass::Internal),
            why,
        ),
    }
}

impl JobAdapter for TadicAdapter {
    type Request = TadicJobRequest;
    type Output = TadicBatch;

    fn kind(&self) -> JobKind {
        JobKind::TadicReplay
    }

    /// ONE job per request (each request is one driver job: its requests share one engine run per seed).
    fn plan(&self, reqs: &[TadicJobRequest]) -> Result<Vec<JobPlan>, AdapterError> {
        let mut out = vec![];
        for (n, r) in reqs.iter().enumerate() {
            if r.requests.is_empty() {
                return Err(AdapterError::Request(format!("job {n}: no request")));
            }
            if r.seeds.is_empty() {
                return Err(AdapterError::Request(format!("job {n}: no seed")));
            }
            let mut labels = std::collections::BTreeSet::new();
            for q in &r.requests {
                if q.label.is_empty() || q.label.chars().any(char::is_whitespace) {
                    return Err(AdapterError::Request(format!("label {:?}", q.label)));
                }
                if !labels.insert(&q.label) {
                    return Err(AdapterError::Request(format!("label {} twice", q.label)));
                }
                if q.inputs.is_empty() {
                    return Err(AdapterError::Request(format!("request {}: no input", q.label)));
                }
                for i in &q.inputs {
                    let w = match i {
                        TadicInput::Package { weight, .. } | TadicInput::Node { weight, .. } => {
                            *weight
                        }
                    };
                    let ok_kind = matches!(
                        (i, &r.level),
                        (TadicInput::Package { .. }, TadicLevel::Package { .. })
                            | (TadicInput::Node { .. }, TadicLevel::Node { .. })
                    );
                    if !ok_kind || w.unsigned_abs() >= 1u64 << 62 {
                        return Err(AdapterError::Request(format!(
                            "request {}: input of another level or weight out of range",
                            q.label
                        )));
                    }
                }
            }
            let content = self.content_of(r);
            out.push(JobPlan {
                job_id: rustred_campaign_contract::layout::job_id(&content, RUNG),
                members: vec![n],
                content,
            });
        }
        Ok(out)
    }

    fn prepare(
        &self,
        reqs: &[TadicJobRequest],
        plan: &JobPlan,
        work_root: &Path,
        attempt: u32,
    ) -> Result<Prepared, AdapterError> {
        let req = &reqs[plan.members[0]];
        let root = std::path::absolute(work_root).map_err(|e| AdapterError::Io(e.to_string()))?;
        let layout = create_run(&root, &plan.job_id, attempt)?;
        let job = self.driver_job(req, &plan.job_id);
        let mut job_bytes = serde_json::to_vec_pretty(&job).map_err(|e| AdapterError::Io(e.to_string()))?;
        job_bytes.push(b'\n');
        let mut inputs = vec![write_input(
            &layout,
            tadicjob::JOB_INPUT,
            tadicjob::ROLE_JOB,
            &job_bytes,
        )?];
        inputs.push(self.driver.input(tadicjob::ROLE_DRIVER));
        if let Some(d) = &req.terminals {
            inputs.push(d.input("terminals"));
        }
        if let Some(d) = &req.dependences {
            inputs.push(d.input("dependences"));
        }
        if let Some(d) = &req.depths {
            inputs.push(d.input("depths"));
        }
        if let TadicLevel::Node { table, .. } = &req.level {
            inputs.push(InputFile::from_file(table)?.input("table"));
        }
        let spec = JobSpec {
            schema: rustred_campaign_contract::CONTRACT.into(),
            job_id: plan.job_id.clone(),
            attempt,
            kind: JobKind::TadicReplay,
            run_dir: layout.root.display().to_string(),
            program: ProgramRef {
                name: self.program.name.clone(),
                blake3: self.program.blake3.clone(),
                features_required: vec![],
            },
            inputs,
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
            fake: self.fake.clone(),
        };
        let argv = tadicjob::argv(&spec).map_err(AdapterError::Spec)?;
        write_spec(&spec, &self.program, self.licence_env.as_deref(), &argv)
    }

    fn finish(
        &self,
        reqs: &[TadicJobRequest],
        plan: &JobPlan,
        prep: &Prepared,
        end: &Ended,
    ) -> TadicBatch {
        let mapped = wrap(&prep.layout, &prep.spec, &prep.spec_blake3, end.wait);
        let e2 = Ended {
            wait: mapped,
            kill: end.kill,
            oom: end.oom,
        };
        let (ev, c) = classify_attempt(prep, &e2);
        let mut out = TadicBatch {
            job_id: plan.job_id.clone(),
            classification: c.clone(),
            result: None,
            result_json: None,
            unread: None,
        };
        if c.row != 50 {
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
        let read = (|| -> Result<(CheckedResult, Value), String> {
            let v = ev.verified.as_ref().ok_or("no verified outputs")?;
            rustred_campaign_contract::classify::tadic_replay_lines(&prep.layout, &prep.spec, v)?;
            let jr = prep
                .spec
                .inputs
                .iter()
                .find(|i| i.role == tadicjob::ROLE_JOB)
                .ok_or("no job input")?;
            let jb = std::fs::read(prep.spec.input_path(jr)).map_err(|e| e.to_string())?;
            let job: Value = serde_json::from_slice(&jb).map_err(|e| e.to_string())?;
            let rb = std::fs::read(prep.layout.out_dir().join(tadicjob::CHILD_OUT))
                .map_err(|e| e.to_string())?;
            let r: Value = serde_json::from_slice(&rb).map_err(|e| e.to_string())?;
            let req = &reqs[plan.members[0]];
            let checked = tadicjob::check_result(&job, &r, &self.pins(&jb, &req.level))?;
            Ok((checked, r))
        })();
        match read {
            Ok((c, r)) => {
                out.result = Some(c);
                out.result_json = Some(r);
            }
            Err(why) => out.unread = Some(why),
        }
        out
    }
}

/// A store of verified attempts keyed by content id (resume: a stored job is never rerun).
#[derive(Debug, Clone)]
pub struct ResultStore {
    pub root: PathBuf,
}

/// The record of one stored job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreRecord {
    pub schema: String,
    pub content: String,
    pub job_id: String,
    pub job_sha256: String,
    pub result_sha256: String,
    pub run_dir: String,
}

/// The schema of `STORE.json`.
pub const STORE_SCHEMA: &str = "rustred.campaign.tadic-store.v1";

impl ResultStore {
    fn dir(&self, content: &str) -> PathBuf {
        self.root.join(content)
    }

    /// Keep a row-50 attempt: job file, result file and sealed lines copied, then `STORE.json` written LAST (atomic).
    pub fn put(&self, content: &str, prep: &Prepared) -> Result<StoreRecord, String> {
        let d = self.dir(content);
        std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
        let jr = prep
            .spec
            .inputs
            .iter()
            .find(|i| i.role == tadicjob::ROLE_JOB)
            .ok_or("no job input")?;
        let jb = std::fs::read(prep.spec.input_path(jr)).map_err(|e| e.to_string())?;
        let rb = std::fs::read(prep.layout.out_dir().join(tadicjob::CHILD_OUT))
            .map_err(|e| e.to_string())?;
        let lb = std::fs::read(prep.layout.results()).map_err(|e| e.to_string())?;
        for (name, b) in [
            (tadicjob::JOB_INPUT, &jb),
            (tadicjob::CHILD_OUT, &rb),
            (RESULTS, &lb),
        ] {
            let tmp = d.join(format!("{name}.tmp"));
            std::fs::write(&tmp, b).map_err(|e| e.to_string())?;
            std::fs::rename(&tmp, d.join(name)).map_err(|e| e.to_string())?;
        }
        let rec = StoreRecord {
            schema: STORE_SCHEMA.into(),
            content: content.into(),
            job_id: prep.spec.job_id.clone(),
            job_sha256: sha256_hex(&jb),
            result_sha256: sha256_hex(&rb),
            run_dir: prep.layout.root.display().to_string(),
        };
        let tmp = d.join("STORE.json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&rec).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, d.join("STORE.json")).map_err(|e| e.to_string())?;
        Ok(rec)
    }

    /// A stored job, re-verified: the files' SHA-256 equal the record's, and the result binds to the job and the
    /// adapter's pins again. `Ok(None)` = not stored.
    pub fn get(
        &self,
        adapter: &TadicAdapter,
        req: &TadicJobRequest,
    ) -> Result<Option<(CheckedResult, Value, StoreRecord)>, String> {
        let content = adapter.content_of(req);
        let d = self.dir(&content);
        let Ok(sb) = std::fs::read(d.join("STORE.json")) else {
            return Ok(None);
        };
        let rec: StoreRecord = serde_json::from_slice(&sb).map_err(|e| e.to_string())?;
        if rec.schema != STORE_SCHEMA || rec.content != content {
            return Err("STORE.json is not this content's".into());
        }
        let jb = std::fs::read(d.join(tadicjob::JOB_INPUT)).map_err(|e| e.to_string())?;
        let rb = std::fs::read(d.join(tadicjob::CHILD_OUT)).map_err(|e| e.to_string())?;
        if sha256_hex(&jb) != rec.job_sha256 || sha256_hex(&rb) != rec.result_sha256 {
            return Err("stored files differ from STORE.json".into());
        }
        let job: Value = serde_json::from_slice(&jb).map_err(|e| e.to_string())?;
        let r: Value = serde_json::from_slice(&rb).map_err(|e| e.to_string())?;
        let checked = tadicjob::check_result(&job, &r, &adapter.pins(&jb, &req.level))?;
        Ok(Some((checked, r, rec)))
    }
}
