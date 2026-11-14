//! The child's start checks in job mode (child-job.md 3.1.2, 3.1.3, 3.2.2, 3.8.2), in a fixed order, each ending
//! in exit 40, 41 or 42. File reads only; the caller decides how to exit.
//!
//! Revised by audit A C4 / C1 / M7 / m10 and audit B F2 / F7: NOTHING may be written into the run directory before
//! it is verified to be THIS attempt's (location, owner, `job.json` with this digest, no OUTCOME, not started yet, no
//! symlink); a refusal before that point has `write_outcome = false` and is reported on stderr only. A resume is
//! verified completely (RESUME digest and consistency, spec binding, job identity, every committed prefix) before
//! the child writes anything into its tables.

use crate::exit::ExitCode;
use crate::layout::{self, RunLayout};
use crate::outcome::{ErrorClass, InputCheck};
use crate::resume::{ResumeRef, ResumeState};
use crate::seal::{TABLE_END_TYPES, file_blake3, open_in_run, verify_prefix};
use crate::spec::{GATED_ENV, JobKind, JobSpec, SpecError};
use crate::stop::StopCode;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// What the program is (from its own build): name, features, kinds it runs, and optionally its own BLAKE3.
#[derive(Debug, Clone)]
pub struct ProgramSelf<'a> {
    pub name: &'a str,
    pub features: &'a [&'a str],
    pub kinds: &'a [JobKind],
    /// BLAKE3 of the running executable; when given it must equal `spec.program.blake3` (42).
    pub blake3: Option<&'a str>,
}

/// A verified resume source (3.8.2): the RESUME record, the previous run directory and the RESUME file's digest.
#[derive(Debug, Clone)]
pub struct VerifiedResume {
    pub state: ResumeState,
    pub prev: RunLayout,
    pub resume_blake3: String,
}

/// A prepared job.
#[derive(Debug, Clone)]
pub struct Prepared {
    pub spec: JobSpec,
    pub spec_blake3: String,
    pub layout: RunLayout,
    pub inputs: Vec<InputCheck>,
    /// Present iff `spec.resume` is set; verified completely.
    pub resume: Option<VerifiedResume>,
}

/// A refused start.
#[derive(Debug, Clone)]
pub struct Refusal {
    pub code: ExitCode,
    pub stop: StopCode,
    pub class: ErrorClass,
    pub message: String,
    /// The parsed spec and its digest, when the spec could be read and parsed.
    pub spec: Option<(Box<JobSpec>, String)>,
    /// Whether the child may write OUTCOME.json: only once the run directory is verified to be this attempt's and
    /// has no OUTCOME (the write must still be no-clobber, [`crate::outcome::Outcome::write_new`]).
    pub write_outcome: bool,
}

fn refuse(
    code: ExitCode,
    message: impl Into<String>,
    spec: Option<(Box<JobSpec>, String)>,
    write_outcome: bool,
) -> Refusal {
    let (stop, class) = match code {
        ExitCode::Usage => (StopCode::Refused, ErrorClass::Usage),
        ExitCode::Input => (StopCode::InputError, ErrorClass::Input),
        _ => (StopCode::Refused, ErrorClass::Refused),
    };
    Refusal {
        code,
        stop,
        class,
        message: message.into(),
        spec,
        write_outcome,
    }
}

fn canon(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Our effective uid without libc: the owner of `/proc/self`.
fn own_uid() -> Option<u32> {
    std::fs::metadata("/proc/self").ok().map(|m| m.uid())
}

/// Read a file of a run directory through the safe opener (no symlink, regular file).
fn read_in_run(root: &Path, rel: &str) -> Result<Vec<u8>, String> {
    let (mut f, _, _) = open_in_run(root, rel).map_err(|e| format!("{rel}: {e}"))?;
    let mut b = Vec::new();
    f.read_to_end(&mut b).map_err(|e| format!("{rel}: {e}"))?;
    Ok(b)
}

/// Step 4: the run directory is this attempt's and unused (C4, F7). `Err(message)` = refuse 42 writing nothing.
fn verify_run_dir(layout: &RunLayout, spec_digest: &str) -> Result<(), String> {
    let root = &layout.root;
    let md = std::fs::symlink_metadata(root).map_err(|e| format!("run_dir: {e}"))?;
    if !md.file_type().is_dir() {
        return Err("run_dir is not a directory".into());
    }
    match own_uid() {
        Some(u) if u == md.uid() => {}
        Some(_) => return Err("run_dir is not owned by this user".into()),
        None => return Err("cannot determine the own uid (/proc/self)".into()),
    }
    let job = read_in_run(root, layout::JOB_SPEC)?;
    if crate::spec::spec_blake3(&job) != spec_digest {
        return Err("RUN/job.json is not the spec file that was read".into());
    }
    if std::fs::symlink_metadata(layout.outcome()).is_ok() {
        return Err(
            "OUTCOME.json already exists (a finished job is final; nothing written)".into(),
        );
    }
    for f in [layout::EVENTS, layout::RESUME, layout::RESERVE] {
        if std::fs::symlink_metadata(root.join(f)).is_ok() {
            return Err(format!("{f} exists: the run directory was already started"));
        }
    }
    // no symlink anywhere a child might write or read (F7)
    for dir in [
        root.clone(),
        layout.out_dir(),
        layout.tmp_dir(),
        layout.in_dir(),
    ] {
        let rd = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && dir != *root => continue,
            Err(e) => return Err(format!("{}: {e}", dir.display())),
        };
        if dir != *root {
            let m = std::fs::symlink_metadata(&dir).map_err(|e| e.to_string())?;
            if !m.file_type().is_dir() {
                return Err(format!("{} is not a directory", dir.display()));
            }
        }
        for e in rd {
            let e = e.map_err(|e| e.to_string())?;
            let t = e.file_type().map_err(|e| e.to_string())?;
            if t.is_symlink() {
                return Err(format!(
                    "{} is a symlink (refused in a run directory)",
                    e.path().display()
                ));
            }
            if dir == layout.out_dir() {
                // m10: out/ must be empty, also for a resume (a resume runs in a NEW attempt directory)
                return Err("out/ is not empty".into());
            }
        }
    }
    Ok(())
}

/// Verify a resume source completely (3.8.2; audit A C1, M7, m7; audit B F1, F2). `Err((code, message))`: 41, or 42
/// when the previous attempt ran another program.
pub fn verify_resume(spec: &JobSpec, r: &ResumeRef) -> Result<VerifiedResume, (ExitCode, String)> {
    let e41 = |m: String| (ExitCode::Input, m);
    r.validate().map_err(e41)?;
    let prev_dir = r.prev_run();
    if canon(&prev_dir) == canon(Path::new(&spec.run_dir)) {
        return Err(e41("resume source is this run directory".into()));
    }
    let bytes = read_in_run(&prev_dir, layout::RESUME).map_err(e41)?;
    let digest = crate::blake3_hex(&bytes);
    if digest != r.blake3 {
        return Err(e41("RESUME.json digest mismatch".into()));
    }
    crate::canon::check_document(&bytes).map_err(|e| e41(format!("RESUME.json: {e}")))?;
    let state: ResumeState =
        serde_json::from_slice(&bytes).map_err(|e| e41(format!("RESUME.json: {e}")))?;
    state
        .check()
        .map_err(|e| e41(format!("RESUME.json: {e}")))?;
    if state.job_id != spec.job_id || state.attempt >= spec.attempt {
        return Err(e41(format!(
            "RESUME.json is of job {:?} attempt {}, not an earlier attempt of job {:?}",
            state.job_id, state.attempt, spec.job_id
        )));
    }
    let job = read_in_run(&prev_dir, layout::JOB_SPEC).map_err(e41)?;
    if crate::spec::spec_blake3(&job) != state.spec_blake3 {
        return Err(e41(
            "RESUME.json spec_blake3 is not the digest of the previous job.json".into(),
        ));
    }
    crate::canon::check_document(&job).map_err(|e| e41(format!("previous job.json: {e}")))?;
    let prev: JobSpec =
        serde_json::from_slice(&job).map_err(|e| e41(format!("previous job.json: {e}")))?;
    if prev.job_id != state.job_id
        || prev.attempt != state.attempt
        || canon(Path::new(&prev.run_dir)) != canon(&prev_dir)
    {
        return Err(e41(
            "previous job.json does not describe the attempt of RESUME.json".into(),
        ));
    }
    if prev.program != spec.program {
        return Err((
            ExitCode::Refused,
            "the previous attempt ran another program (same binary required)".into(),
        ));
    }
    if !spec.same_job(&prev) {
        return Err(e41(
            "the previous attempt is not the same job (spec differs beyond attempt / run_dir / resume)".into(),
        ));
    }
    verify_prefix(&prev_dir, &state.table, &TABLE_END_TYPES)
        .map_err(|e| e41(format!("committed table prefix: {e}")))?;
    if let Some(q) = &state.queue {
        verify_prefix(&prev_dir, q, &TABLE_END_TYPES)
            .map_err(|e| e41(format!("committed queue prefix: {e}")))?;
    }
    Ok(VerifiedResume {
        state,
        prev: RunLayout::new(prev_dir),
        resume_blake3: digest,
    })
}

/// Run the checks, in this order (audit A C4):
/// 1. read the spec (40; nothing written);
/// 2. parse (40; nothing written);
/// 3. the spec file is `<run_dir>/job.json` (42; nothing written);
/// 4. the run directory is ours and unused: a directory owned by us, `job.json` a regular file with the digest read,
///    no OUTCOME, no EVENTS / RESUME / .reserve, no symlink in RUN, out/, tmp/, in/, out/ empty (42; nothing written);
///
/// from here on a refusal may write an OUTCOME (no-clobber):
/// 5. validate (40 / 42);
/// 6. program name, kind, own digest (42);
/// 7. `features_required` (42);
/// 8. gated environment variables not in `env_allow` (42);
/// 9. resume ([`verify_resume`]: 41, 42 for another program);
/// 10. every input exists and has its BLAKE3 (41).
pub fn preflight(
    spec_path: &Path,
    program: &ProgramSelf<'_>,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Prepared, Refusal> {
    let bytes = std::fs::read(spec_path).map_err(|e| {
        refuse(
            ExitCode::Usage,
            format!("cannot read job spec {}: {e}", spec_path.display()),
            None,
            false,
        )
    })?;
    let digest = crate::spec::spec_blake3(&bytes);
    let spec: JobSpec = crate::canon::check_document(&bytes)
        .map_err(|e| e.to_string())
        .and_then(|()| serde_json::from_slice(&bytes).map_err(|e| e.to_string()))
        .map_err(|e| {
            refuse(
                ExitCode::Usage,
                SpecError::Parse(e.to_string()).to_string(),
                None,
                false,
            )
        })?;
    let with = |s: &JobSpec| Some((Box::new(s.clone()), digest.clone()));
    if spec_path.file_name().and_then(|n| n.to_str()) != Some(layout::JOB_SPEC) {
        return Err(refuse(
            ExitCode::Refused,
            format!("the job spec file must be named {}", layout::JOB_SPEC),
            with(&spec),
            false,
        ));
    }
    if !Path::new(&spec.run_dir).is_absolute() {
        return Err(refuse(
            ExitCode::Refused,
            "run_dir is not absolute",
            with(&spec),
            false,
        ));
    }
    if let Err(e) = spec.check_location(spec_path) {
        return Err(refuse(e.exit_code(), e.to_string(), with(&spec), false));
    }
    let layout = RunLayout::new(&spec.run_dir);
    if let Err(m) = verify_run_dir(&layout, &digest) {
        return Err(refuse(ExitCode::Refused, m, with(&spec), false));
    }
    if let Err(e) = spec.validate() {
        return Err(refuse(e.exit_code(), e.to_string(), with(&spec), true));
    }
    if spec.program.name != program.name || !program.kinds.contains(&spec.kind) {
        return Err(refuse(
            ExitCode::Refused,
            format!(
                "program {} does not run kind {:?} of program {}",
                program.name, spec.kind, spec.program.name
            ),
            with(&spec),
            true,
        ));
    }
    if let Some(me) = program.blake3
        && me != spec.program.blake3
    {
        return Err(refuse(
            ExitCode::Refused,
            "the running binary's BLAKE3 is not spec.program.blake3",
            with(&spec),
            true,
        ));
    }
    let missing: Vec<&str> = spec
        .program
        .features_required
        .iter()
        .map(String::as_str)
        .filter(|f| !program.features.contains(f))
        .collect();
    if !missing.is_empty() {
        return Err(refuse(
            ExitCode::Refused,
            format!("features not supported: {}", missing.join(", ")),
            with(&spec),
            true,
        ));
    }
    for v in GATED_ENV {
        if env(v).is_some() && !spec.env_allow.iter().any(|a| a == v) {
            return Err(refuse(
                ExitCode::Refused,
                format!("environment variable {v} is set but not in env_allow"),
                with(&spec),
                true,
            ));
        }
    }
    let resume = match &spec.resume {
        None => None,
        Some(r) => match verify_resume(&spec, r) {
            Ok(v) => Some(v),
            Err((code, m)) => {
                return Err(refuse(code, format!("resume: {m}"), with(&spec), true));
            }
        },
    };
    let mut inputs = Vec::new();
    for i in &spec.inputs {
        let p = spec.input_path(i);
        match file_blake3(&p) {
            Ok((n, d)) if d == i.blake3 => inputs.push(InputCheck {
                role: i.role.clone(),
                path: i.path.clone(),
                bytes: n,
                blake3: d,
                verified: true,
            }),
            Ok(_) => {
                return Err(refuse(
                    ExitCode::Input,
                    format!("input {:?} ({}) digest mismatch", i.role, p.display()),
                    with(&spec),
                    true,
                ));
            }
            Err(e) => {
                return Err(refuse(
                    ExitCode::Input,
                    format!("input {:?} ({}): {e}", i.role, p.display()),
                    with(&spec),
                    true,
                ));
            }
        }
    }
    Ok(Prepared {
        spec,
        spec_blake3: digest,
        layout,
        inputs,
        resume,
    })
}
