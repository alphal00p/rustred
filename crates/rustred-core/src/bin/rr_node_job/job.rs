//! `rr_node job RUN/job.json` and `rr_node version --json` (child-job contract `rustred.child-job.v1`, engine change
//! C1). A module of the `rr_node` binary so that the job spec's `args` go through the SAME flag parser as the legacy
//! command line (`node_options`, `order_arg`). The job itself runs in `rustred::node::childjob`.
//!
//! ONE job entry (lane engbase2): kinds `generate` and `solve-recipe` both start here, behind the same contract
//! preflight; `rr_node solve-recipe --job RUN/job.json` (the engine line's spelling) is an alias of [`cmd_job_at`].

use std::path::Path;

use cc::outcome::ProgramInfo;
use cc::preflight::{ProgramSelf, preflight};
use cc::spec::{JobKind, JobSpec};
use rustred::node::childjob::recipe_job::{self, RecipeArgs};
use rustred::node::childjob::{self, FEATURES, JobStart, PROGRAM, Resolved};
use rustred_campaign_contract as cc;

use super::{Args, node_options, order_arg};

/// The job kinds this build runs (`version --json`, preflight step 6).
const JOB_KINDS: [JobKind; 2] = [JobKind::Generate, JobKind::SolveRecipe];

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
        job_kinds: JOB_KINDS.to_vec(),
        // the node driver's compiled arities, from the driver's own list (audit-engjob m9)
        arities: rustred::node::driver::GENERATE_ARITIES
            .iter()
            .map(|&a| a as u32)
            .collect(),
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
    let [path] = a.items.as_slice() else {
        eprintln!("rr_node: usage: rr_node job RUN/job.json");
        std::process::exit(cc::ExitCode::Usage.code());
    };
    cmd_job_at(path)
}

/// The single job entry (`rr_node job P`, and the alias `rr_node solve-recipe --job P`): contract preflight, then
/// the kind's runner. Never returns.
pub fn cmd_job_at(path: &str) -> i32 {
    // first thing, before any thread or fork (3.4.1)
    childjob::proc::install_signals();
    // preflight step 6 checks the running binary's raw BLAKE3 against `spec.program.blake3` only when it is given
    // (lane engbase2: it was None since lane eng2, so a spec naming another binary ran; adapters' probe
    // `foreign_program_digest`). Unreadable executable: unchecked, as before.
    let own = std::fs::read("/proc/self/exe")
        .ok()
        .map(|b| cc::blake3_hex(&b));
    let me = ProgramSelf {
        name: PROGRAM,
        features: &FEATURES,
        kinds: &JOB_KINDS,
        blake3: own.as_deref(),
    };
    let prep = match preflight(Path::new(path), &me, &|k| std::env::var(k).ok()) {
        Ok(p) => p,
        Err(r) => childjob::refuse_exit(r, &program_info()),
    };
    let kind = prep.spec.kind;
    let start = JobStart {
        spec: prep.spec,
        spec_blake3: prep.spec_blake3,
        layout: prep.layout,
        inputs: prep.inputs,
        program: program_info(),
        resume: prep.resume,
    };
    match kind {
        JobKind::SolveRecipe => recipe_job::run(start, &resolve_recipe),
        _ => childjob::run_generate(start, &resolve),
    }
}

/// The batch parameters of a `solve-recipe` job from the spec's `args` (allow-list checked by the preflight:
/// `--order` required, `--threads`, `--max-trace-rows`, `--key-budget-s`) with the legacy parsers; the per-key wall
/// budget is `--key-budget-s` or `budgets.wall.case_s` (never both: spec validation).
fn resolve_recipe(spec: &JobSpec, family_text: &str) -> RecipeArgs {
    let mut a = Args {
        items: spec.args.clone(),
    };
    let order = a
        .take("--order")
        .unwrap_or_else(|| super::die("a solve-recipe job needs --order"));
    let threads: usize = a.parse("--threads").unwrap_or(1);
    let max_trace_rows: Option<usize> = a.parse("--max-trace-rows");
    let key_budget_s = a
        .parse::<u64>("--key-budget-s")
        .or(spec.budgets.wall.case_s)
        .map(|s| s as f64);
    a.done();
    RecipeArgs {
        order: order_arg(&order, family_text, None),
        threads,
        max_trace_rows,
        key_budget_s,
    }
}

/// The node options of a `generate` job: `args` through the legacy parser, then the typed fields of the spec.
fn resolve(spec: &JobSpec) -> Resolved {
    let (family_text, family_path) = childjob::family_input(spec);
    let mut a = Args {
        items: spec.args.clone(),
    };
    if let Some(d) = a.take("--descend") {
        if d != "open" {
            super::die("--descend open");
        }
        childjob::set_descend_open();
    }
    let order = a.take("--order").unwrap_or_else(|| {
        super::die("a generate job needs --order (an explicit order; auto is not a job kind)")
    });
    if order == "auto" {
        super::die("--order auto is the generate-auto ladder, not a generate job");
    }
    let mut options = node_options(&mut a, &family_text);
    options.order = order_arg(&order, &family_text, None);
    a.done();
    if options.symmetry {
        // sym-gen closes cases by `sym` / `sym_zero` table records: not part of the child-job contract v1 (no
        // acceptance rule, no case result, no resume of their indices / cover entries): fail closed
        childjob::fail(
            cc::ExitCode::Refused,
            "--symmetry (sym-gen case records) is not supported in job mode: the contract has no rule for sym records",
        );
    }
    options.max_cases = spec.budgets.work.cases.map(|c| c as usize);
    let (initial, centres) = childjob::initial_cases(spec);
    options.initial = initial;
    options.case_seed_centres = centres;
    options.cover_tables = childjob::cover_tables(spec);
    options.case_budget_exit = None;
    Resolved {
        options,
        family_text,
        family_path,
    }
}
