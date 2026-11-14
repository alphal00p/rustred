//! The adapter trait (interfaces/adapters.md 2.1): a job adapter turns work-source requests into job specs of the
//! child-job contract, prepares one attempt (run directory, inputs, `job.json`, launch spec), and after the child
//! ended classifies the attempt through the contract crate ONLY and converts what may be accepted, reading records
//! ONLY through `seal::VerifiedHandle`. [`run_attempt`] drives one attempt through any [`ChildLauncher`] (the real
//! launcher: own session, PDEATHSIG, marker, verified kill, end-of-job hunt).
//!
//! Generic (rule L8): no node, arity, count or path is known here; programs, inputs, budgets are data.

use crate::launch::{
    ChildHandle, ChildLauncher, ChildProgress, ExitState, FinalReport, KillReason as LaunchKill,
    LaunchError, LaunchSpec,
};
use rustred_campaign_contract::classify::{
    self, Classification, Evidence, KillReason as CKill, WaitStatus,
};
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::spec::{InputRef, JobKind, JobSpec};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How a program is started for one attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "form")]
pub enum Invocation {
    /// `<program> job RUN/job.json` (child-job 3.1.1; the default).
    Contract,
    /// `<program> <verb> --job RUN/job.json` (the engine line's `solve-recipe` v1.1 job mode). A LAUNCH difference
    /// only: what is accepted goes through the same strict classification (interfaces/adapters.md 2.1).
    VerbJobFlag { verb: String },
    /// The legacy argv the adapter builds itself (a legacy child: rr_proof). No `job.json` is passed.
    Legacy,
}

/// A program file (data): name (the contract's `program.name`), absolute path, raw BLAKE3 of the file, invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Program {
    pub name: String,
    pub path: PathBuf,
    pub blake3: String,
    pub invocation: Invocation,
    /// `program.features_required` of every spec this adapter writes.
    #[serde(default)]
    pub features_required: Vec<String>,
}

impl Program {
    /// The program with its BLAKE3 computed from the file.
    pub fn from_file(
        name: &str,
        path: &Path,
        invocation: Invocation,
    ) -> Result<Program, AdapterError> {
        let b3 = file_blake3(path)?;
        Ok(Program {
            name: name.into(),
            path: path.to_path_buf(),
            blake3: b3,
            invocation,
            features_required: vec![],
        })
    }
}

/// An input file by absolute path with its raw BLAKE3 (digest-checked by the child at preflight step 10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputFile {
    pub path: PathBuf,
    pub blake3: String,
}

impl InputFile {
    pub fn from_file(path: &Path) -> Result<InputFile, AdapterError> {
        Ok(InputFile {
            path: path.to_path_buf(),
            blake3: file_blake3(path)?,
        })
    }
    pub fn input(&self, role: &str) -> InputRef {
        InputRef {
            role: role.into(),
            path: self.path.display().to_string(),
            blake3: self.blake3.clone(),
            upto: None,
        }
    }
}

/// Raw BLAKE3 (hex) of a file.
pub fn file_blake3(p: &Path) -> Result<String, AdapterError> {
    let b = std::fs::read(p).map_err(|e| AdapterError::Io(format!("{}: {e}", p.display())))?;
    Ok(rustred_campaign_contract::blake3_hex(&b))
}

/// Errors of planning / preparing (nothing launched).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterError {
    /// A request the adapter cannot turn into a valid job (the reason).
    Request(String),
    /// The spec the adapter built is refused by the contract crate (a bug of the adapter or of its data).
    Spec(String),
    Io(String),
}

impl fmt::Display for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdapterError::Request(m) => write!(f, "request: {m}"),
            AdapterError::Spec(m) => write!(f, "job spec: {m}"),
            AdapterError::Io(m) => write!(f, "io: {m}"),
        }
    }
}
impl std::error::Error for AdapterError {}

/// One job of a plan: its id, the request indices it serves, and its content id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobPlan {
    pub job_id: String,
    /// Indices into the request list given to [`JobAdapter::plan`] (every member gets the job's outcome).
    pub members: Vec<usize>,
    /// The job's content id (deterministic in the requests and the adapter's data).
    pub content: String,
}

/// One prepared attempt.
#[derive(Debug, Clone)]
pub struct Prepared {
    pub spec: JobSpec,
    /// The contract's spec digest of the `job.json` bytes written (the supervisor's own value, never re-read).
    pub spec_blake3: String,
    pub layout: RunLayout,
    pub launch: LaunchSpec,
}

/// How the child ended, as the contract's decision table needs it (from the launcher's [`FinalReport`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ended {
    pub wait: WaitStatus,
    pub kill: CKill,
    pub oom: bool,
}

impl Ended {
    /// The mapping of supervisor.md section 5: Stop / ForceStop / HostEmergency / Reattach -> Stop, MemoryCap ->
    /// Memory, Timeout -> Time, Unresponsive -> Unresponsive, none / other -> None; exit state -> wait status
    /// (unknown when the launcher could not know it: execution UNKNOWN, classified from the files, 6.6).
    pub fn from_report(r: &FinalReport) -> Ended {
        let kill = match &r.kill_reason {
            None | Some(LaunchKill::Other(_)) => CKill::None,
            Some(
                LaunchKill::Stop
                | LaunchKill::ForceStop
                | LaunchKill::Reattach
                | LaunchKill::HostEmergency,
            ) => CKill::Stop,
            Some(LaunchKill::MemoryCap { .. }) => CKill::Memory,
            Some(LaunchKill::Timeout) => CKill::Time,
            Some(LaunchKill::Unresponsive) => CKill::Unresponsive,
        };
        let wait = match r.exit {
            Some(ExitState::Exited(c)) => WaitStatus::Exited(c),
            Some(ExitState::Signaled { signal, .. }) => WaitStatus::Signaled(signal),
            _ => WaitStatus::Unknown,
        };
        Ended {
            wait,
            kill,
            oom: false,
        }
    }

    /// Execution UNKNOWN (recovery hazard rule: an unconfirmed launch is never "did not run").
    pub fn unknown() -> Ended {
        Ended {
            wait: WaitStatus::Unknown,
            kill: CKill::None,
            oom: false,
        }
    }
}

/// The contract's classification of a finished attempt (`classify::assess` + `classify::classify`) with the evidence.
pub fn classify_attempt(p: &Prepared, end: &Ended) -> (Evidence, Classification) {
    let ev = classify::assess(
        &p.layout,
        &p.spec,
        &p.spec_blake3,
        end.wait,
        end.kill,
        end.oom,
    );
    let c = classify::classify(&ev);
    (ev, c)
}

/// The adapter trait (interfaces/adapters.md 2.1).
pub trait JobAdapter {
    type Request;
    type Output;
    fn kind(&self) -> JobKind;
    /// Group requests into jobs, deterministically.
    fn plan(&self, reqs: &[Self::Request]) -> Result<Vec<JobPlan>, AdapterError>;
    /// One attempt of `plan`: RUN = `layout::attempt_dir(work_root, job_id, attempt)`.
    fn prepare(
        &self,
        reqs: &[Self::Request],
        plan: &JobPlan,
        work_root: &Path,
        attempt: u32,
    ) -> Result<Prepared, AdapterError>;
    /// After the child ended: classification through the contract crate, records through verified handles.
    fn finish(
        &self,
        reqs: &[Self::Request],
        plan: &JobPlan,
        prep: &Prepared,
        end: &Ended,
    ) -> Self::Output;
}

/// Create a fresh attempt directory with `in/ out/ tmp/ sup/` (refuses an existing one: an attempt directory is
/// never reused).
pub fn create_run(
    work_root: &Path,
    job_id: &str,
    attempt: u32,
) -> Result<RunLayout, AdapterError> {
    let run = rustred_campaign_contract::layout::attempt_dir(work_root, job_id, attempt);
    if run.exists() {
        return Err(AdapterError::Io(format!(
            "{} exists (an attempt directory is never reused)",
            run.display()
        )));
    }
    let layout = RunLayout::new(&run);
    for d in [
        layout.in_dir(),
        layout.out_dir(),
        layout.tmp_dir(),
        layout.sup_dir(),
    ] {
        std::fs::create_dir_all(&d)
            .map_err(|e| AdapterError::Io(format!("{}: {e}", d.display())))?;
    }
    Ok(layout)
}

/// Write a generated input into `RUN/in/<name>` (no clobber) and return its input reference (path relative to RUN).
pub fn write_input(
    layout: &RunLayout,
    name: &str,
    role: &str,
    bytes: &[u8],
) -> Result<InputRef, AdapterError> {
    let p = layout.in_dir().join(name);
    crate::atomic::create_new_atomic(&p, bytes)
        .map_err(|e| AdapterError::Io(format!("{}: {e}", p.display())))?;
    Ok(InputRef {
        role: role.into(),
        path: format!("{}/{name}", rustred_campaign_contract::layout::IN_DIR),
        blake3: rustred_campaign_contract::blake3_hex(bytes),
        upto: None,
    })
}

/// Validate `spec` with the contract crate, write `job.json` (0444, no clobber), and build the launch spec of the
/// program's invocation with the contract environment, the licence variable by NAME and stdout / stderr into RUN.
pub fn write_spec(
    spec: &JobSpec,
    program: &Program,
    licence_env: Option<&str>,
    legacy_args: &[String],
) -> Result<Prepared, AdapterError> {
    spec.validate()
        .map_err(|e| AdapterError::Spec(e.to_string()))?;
    let layout = RunLayout::new(&spec.run_dir);
    let p = layout.job_spec();
    let bytes = spec.to_json_line();
    let digest = rustred_campaign_contract::spec::spec_blake3(&bytes);
    crate::atomic::create_new_atomic(&p, &bytes)
        .map_err(|e| AdapterError::Io(format!("{}: {e}", p.display())))?;
    let mut perm = std::fs::metadata(&p)
        .map_err(|e| AdapterError::Io(e.to_string()))?
        .permissions();
    perm.set_readonly(true);
    std::fs::set_permissions(&p, perm).map_err(|e| AdapterError::Io(e.to_string()))?;
    let mut ls = LaunchSpec::new(spec.job_id.clone(), &program.path, &layout.root);
    match &program.invocation {
        Invocation::Contract => ls = ls.arg("job").arg(p.as_os_str()),
        Invocation::VerbJobFlag { verb } => {
            ls = ls.arg(verb.as_str()).arg("--job").arg(p.as_os_str())
        }
        Invocation::Legacy => {
            for a in legacy_args {
                ls = ls.arg(a.as_str());
            }
        }
    }
    let mut ls = ls.with_contract_env(&layout.tmp_dir());
    if let Some(n) = licence_env {
        ls.env_passthrough.push(n.to_string());
    }
    ls.stdout = layout.stdout_log();
    ls.stderr = layout.stderr_log();
    Ok(Prepared {
        spec: spec.clone(),
        spec_blake3: digest,
        layout,
        launch: ls,
    })
}

/// Launch options of one attempt (data).
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    pub cpus: Vec<usize>,
    pub memory_cap_bytes: Option<u64>,
    /// The licence variable's NAME (copied by the launcher from the supervisor's environment; never a value).
    pub licence_env: Option<String>,
    /// Wall cap: `terminate(grace, Timeout)` after it (K = time).
    pub wall_cap: Option<Duration>,
    /// Grace of a wall-cap termination (default 5 s).
    pub grace: Option<Duration>,
}

/// What [`run_attempt`] returns besides the adapter's output.
#[derive(Debug, Clone)]
pub struct AttemptReport {
    pub prepared: Prepared,
    pub ended: Ended,
    pub report: Option<FinalReport>,
    pub launch_error: Option<String>,
}

/// Prepare, launch through `launcher`, drive to the end (wall cap -> terminate), map the report and finish.
/// A launch error (nothing started) is reported with `WaitStatus::Unknown` and classified from the (empty) run
/// directory: never "did not run" by assumption (recovery hazard rule).
pub fn run_attempt<L: ChildLauncher, A: JobAdapter>(
    launcher: &mut L,
    adapter: &A,
    reqs: &[A::Request],
    plan: &JobPlan,
    work_root: &Path,
    attempt: u32,
    opts: &RunOptions,
) -> Result<(A::Output, AttemptReport), AdapterError> {
    let mut prep = adapter.prepare(reqs, plan, work_root, attempt)?;
    prep.launch.cpus = opts.cpus.clone();
    prep.launch.memory_cap_bytes = opts.memory_cap_bytes;
    if let Some(n) = &opts.licence_env
        && !prep.launch.env_passthrough.contains(n)
    {
        prep.launch.env_passthrough.push(n.clone());
    }
    let (ended, report, launch_error) = match launcher.launch(&prep.launch) {
        Ok(mut h) => {
            let r = drive(&mut h, opts);
            (Ended::from_report(&r), Some(r), None)
        }
        Err(e) => (Ended::unknown(), None, Some(launch_error_text(&e))),
    };
    let out = adapter.finish(reqs, plan, &prep, &ended);
    Ok((
        out,
        AttemptReport {
            prepared: prep,
            ended,
            report,
            launch_error,
        },
    ))
}

fn launch_error_text(e: &LaunchError) -> String {
    e.to_string()
}

/// Poll a handle until it reports `Finished`, applying the wall cap.
pub fn drive<H: ChildHandle>(h: &mut H, opts: &RunOptions) -> FinalReport {
    let t0 = Instant::now();
    let mut capped = false;
    loop {
        match h.poll() {
            Ok(ChildProgress::Finished(r)) => return r,
            Ok(_) => {}
            Err(_) => {}
        }
        if !capped && opts.wall_cap.is_some_and(|w| t0.elapsed() >= w) {
            h.terminate(
                opts.grace.unwrap_or(Duration::from_secs(5)),
                LaunchKill::Timeout,
            );
            capped = true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Quote one shell word with single quotes (`'` -> `'\''`).
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting() {
        assert_eq!(shell_quote("a b"), "'a b'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }
}
