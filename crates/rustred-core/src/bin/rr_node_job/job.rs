//! `rr_node job RUN/job.json` and `rr_node version --json` (child-job contract `rustred.child-job.v1`, engine change
//! C1). A module of the `rr_node` binary so that the job spec's `args` go through the SAME flag parser as the legacy
//! command line (`node_options`, `order_arg`). The job itself runs in `rustred::node::childjob`.

use std::path::Path;

use rustred::node::childjob::{self, FEATURES, JobStart, PROGRAM, Resolved};
use rustred_campaign_contract as cc;
use cc::outcome::ProgramInfo;
use cc::preflight::{ProgramSelf, preflight};
use cc::spec::{JobKind, JobSpec};

use super::{Args, node_options, order_arg};

/// The node driver's compiled arities (`node::driver::generate`).
const ARITIES: [u32; 5] = [5, 9, 12, 14, 20];

/// The git commit captured at BUILD time (ruling R-e); "unknown" when the build environment had none.
pub fn git_commit() -> String {
    cc::git_commit_from_build_env(option_env!("RUSTRED_BUILD_GIT_COMMIT"))
}

fn version_info() -> cc::VersionInfo {
    cc::VersionInfo {
        program: PROGRAM.into(),
        package_version: env!("CARGO_PKG_VERSION").into(),
        git_commit: git_commit(),
        contract: vec![cc::CONTRACT.into()],
        features: FEATURES.iter().map(|s| s.to_string()).collect(),
        job_kinds: vec![JobKind::Generate],
        arities: ARITIES.to_vec(),
    }
}

/// The version line as JSON (also merged into `--capabilities`).
pub fn version_json() -> serde_json::Value {
    serde_json::to_value(version_info()).unwrap_or(serde_json::Value::Null)
}

fn program_info() -> ProgramInfo {
    ProgramInfo {
        name: PROGRAM.into(),
        version: env!("CARGO_PKG_VERSION").into(),
        git_commit: git_commit(),
        features: FEATURES.iter().map(|s| s.to_string()).collect(),
        exe: childjob::exe_info(),
    }
}

/// `rr_node version --json` / `rr_node --version --json`: one line (3.2.1).
pub fn cmd_version(mut a: Args) -> i32 {
    if !a.flag("--json") {
        super::die("version --json");
    }
    a.done();
    println!("{}", serde_json::to_string(&version_info()).unwrap());
    0
}

/// The legacy parser's `die` in job mode: an error OUTCOME (usage, exit 40) instead of exit 2. Returns in legacy
/// mode.
pub fn on_die(msg: &str) {
    childjob::fail_now(cc::ExitCode::Usage, msg);
}

/// `rr_node job RUN/job.json`.
pub fn cmd_job(a: Args) -> i32 {
    // first thing, before any thread or fork (3.4.1)
    childjob::proc::install_signals();
    let [path] = a.items.as_slice() else {
        eprintln!("rr_node: usage: rr_node job RUN/job.json");
        std::process::exit(cc::ExitCode::Usage.code());
    };
    let me = ProgramSelf {
        name: PROGRAM,
        features: &FEATURES,
        kinds: &[JobKind::Generate],
        blake3: None,
    };
    let prep = match preflight(Path::new(path), &me, &|k| std::env::var(k).ok()) {
        Ok(p) => p,
        Err(r) => childjob::refuse_exit(r, &program_info()),
    };
    childjob::run_generate(
        JobStart {
            spec: prep.spec,
            spec_blake3: prep.spec_blake3,
            layout: prep.layout,
            inputs: prep.inputs,
            program: program_info(),
        },
        &resolve,
    )
}

/// The node options of a `generate` job: `args` through the legacy parser, then the typed fields of the spec.
fn resolve(spec: &JobSpec) -> Resolved {
    let (family_text, family_path) = childjob::family_input(spec);
    let mut a = Args {
        items: spec.args.clone(),
    };
    let order = a
        .take("--order")
        .unwrap_or_else(|| super::die("a generate job needs --order (an explicit order; auto is not a job kind)"));
    if order == "auto" {
        super::die("--order auto is the generate-auto ladder, not a generate job");
    }
    let mut options = node_options(&mut a, &family_text);
    options.order = order_arg(&order, &family_text, None);
    a.done();
    options.max_cases = spec.budgets.work.cases.map(|c| c as usize);
    options.initial = childjob::initial_cases(spec);
    options.cover_tables = childjob::cover_tables(spec);
    options.case_budget_exit = None;
    Resolved {
        options,
        family_text,
        family_path,
    }
}
