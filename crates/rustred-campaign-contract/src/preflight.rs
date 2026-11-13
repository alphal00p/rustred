//! The child's start checks in job mode (child-job.md 3.1.2, 3.1.3, 3.2.2, 3.8.2), in a fixed order, each ending
//! in exit 40, 41 or 42. File reads only; the caller decides how to exit.

use crate::exit::ExitCode;
use crate::layout::RunLayout;
use crate::outcome::{ErrorClass, InputCheck};
use crate::seal::{file_blake3, prefix_blake3};
use crate::spec::{GATED_ENV, JobKind, JobSpec, SpecError};
use crate::stop::StopCode;
use std::path::Path;

/// What the program is (from its own build): name, features, kinds it runs, and optionally its own BLAKE3.
#[derive(Debug, Clone)]
pub struct ProgramSelf<'a> {
    pub name: &'a str,
    pub features: &'a [&'a str],
    pub kinds: &'a [JobKind],
    /// BLAKE3 of the running executable; when given it must equal `spec.program.blake3` (42).
    pub blake3: Option<&'a str>,
}

/// A prepared job.
#[derive(Debug, Clone)]
pub struct Prepared {
    pub spec: JobSpec,
    pub spec_blake3: String,
    pub layout: RunLayout,
    pub inputs: Vec<InputCheck>,
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
    /// Whether the child may write OUTCOME.json: false when the run directory is unknown or OUTCOME already exists.
    pub write_outcome: bool,
}

fn refuse(
    code: ExitCode,
    message: impl Into<String>,
    spec: Option<(Box<JobSpec>, String)>,
) -> Refusal {
    let (stop, class) = match code {
        ExitCode::Usage => (StopCode::Refused, ErrorClass::Usage),
        ExitCode::Input => (StopCode::InputError, ErrorClass::Input),
        _ => (StopCode::Refused, ErrorClass::Refused),
    };
    let write_outcome = spec.is_some();
    Refusal {
        code,
        stop,
        class,
        message: message.into(),
        spec,
        write_outcome,
    }
}

/// Run the checks, in this order:
/// 1. read the spec (40, run dir unknown);
/// 2. parse (40);
/// 3. validate (40 / 42);
/// 4. `run_dir` = the spec's directory (42);
/// 5. program name, kind, own digest (42);
/// 6. `features_required` (42);
/// 7. OUTCOME.json exists (42, OUTCOME not written);
/// 8. `out/` non-empty without `resume` (42);
/// 9. gated environment variables not in `env_allow` (42);
/// 10. resume digests: RESUME file and table prefix (41);
/// 11. every input exists and has its BLAKE3 (41).
pub fn preflight(
    spec_path: &Path,
    program: &ProgramSelf<'_>,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Prepared, Refusal> {
    let bytes = std::fs::read(spec_path).map_err(|e| {
        let mut r = refuse(
            ExitCode::Usage,
            format!("cannot read job spec {}: {e}", spec_path.display()),
            None,
        );
        r.write_outcome = false;
        r
    })?;
    let digest = crate::spec::spec_blake3(&bytes);
    let spec: JobSpec = serde_json::from_slice(&bytes).map_err(|e| {
        refuse(
            ExitCode::Usage,
            SpecError::Parse(e.to_string()).to_string(),
            None,
        )
    })?;
    let with = |s: &JobSpec| Some((Box::new(s.clone()), digest.clone()));
    if let Err(e) = spec.validate().and_then(|_| spec.check_location(spec_path)) {
        let mut r = refuse(e.exit_code(), e.to_string(), with(&spec));
        if matches!(e, SpecError::RunDirMismatch { .. }) {
            // the run dir named in the spec is not ours: write nothing there
            r.write_outcome = false;
        }
        return Err(r);
    }
    if spec.program.name != program.name || !program.kinds.contains(&spec.kind) {
        return Err(refuse(
            ExitCode::Refused,
            format!(
                "program {} does not run kind {:?} of program {}",
                program.name, spec.kind, spec.program.name
            ),
            with(&spec),
        ));
    }
    if let Some(me) = program.blake3
        && me != spec.program.blake3
    {
        return Err(refuse(
            ExitCode::Refused,
            "the running binary's BLAKE3 is not spec.program.blake3",
            with(&spec),
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
        ));
    }
    let layout = RunLayout::new(&spec.run_dir);
    if layout.outcome().exists() {
        let mut r = refuse(
            ExitCode::Refused,
            "OUTCOME.json already exists (a sealed job is final)",
            with(&spec),
        );
        r.write_outcome = false;
        return Err(r);
    }
    if spec.resume.is_none()
        && let Ok(mut d) = std::fs::read_dir(layout.out_dir())
        && d.next().is_some()
    {
        return Err(refuse(
            ExitCode::Refused,
            "out/ is not empty and the spec has no resume",
            with(&spec),
        ));
    }
    for v in GATED_ENV {
        if env(v).is_some() && !spec.env_allow.iter().any(|a| a == v) {
            return Err(refuse(
                ExitCode::Refused,
                format!("environment variable {v} is set but not in env_allow"),
                with(&spec),
            ));
        }
    }
    if let Some(r) = &spec.resume {
        match file_blake3(Path::new(&r.from)) {
            Ok((_, d)) if d == r.blake3 => {}
            Ok(_) => {
                return Err(refuse(
                    ExitCode::Input,
                    "RESUME.json digest mismatch",
                    with(&spec),
                ));
            }
            Err(e) => {
                return Err(refuse(
                    ExitCode::Input,
                    format!("cannot read {}: {e}", r.from),
                    with(&spec),
                ));
            }
        }
        match prefix_blake3(Path::new(&r.table), r.committed_bytes) {
            Ok(Some(d)) if d == r.prefix_blake3 => {}
            Ok(_) => {
                return Err(refuse(
                    ExitCode::Input,
                    "committed table prefix digest mismatch",
                    with(&spec),
                ));
            }
            Err(e) => {
                return Err(refuse(
                    ExitCode::Input,
                    format!("cannot read {}: {e}", r.table),
                    with(&spec),
                ));
            }
        }
    }
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
                ));
            }
            Err(e) => {
                return Err(refuse(
                    ExitCode::Input,
                    format!("input {:?} ({}): {e}", i.role, p.display()),
                    with(&spec),
                ));
            }
        }
    }
    Ok(Prepared {
        spec,
        spec_blake3: digest,
        layout,
        inputs,
    })
}
