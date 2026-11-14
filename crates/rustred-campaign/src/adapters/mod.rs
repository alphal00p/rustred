//! Adapters: the mapping of a [`JobRequest`] to a child command line (interfaces/supervisor.md section 6).
//!
//! Every child runs under the child-job contract: `<program> job RUN/job.json`. The FAKE adapter produces a `kind: fake`
//! spec for `rr_fake_child` carrying the request's script; the CONTRACT adapter produces the spec of any contract
//! program (rr_node / rr_proof once the engine side exists). Real rr_node argv tables (plan 2.3) come later.

use crate::launch::LaunchSpec;
use crate::supervisor::worksource::JobRequest;
use rustred_campaign_contract::fake::FakeScript;
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::resume::ResumeRef;
use rustred_campaign_contract::spec::{
    Determinism, JobKind, JobSpec, OnCaseBudget, OutputOptions, ProgramRef, SeedSpec,
};
use std::path::Path;

/// Which adapter builds the child command line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdapterKind {
    /// `rr_fake_child job RUN/job.json` with the request's script (or a default script).
    Fake,
    /// Any contract program.
    Contract,
}

/// What the adapter needs besides the request.
pub struct AdapterInput<'a> {
    pub run_dir: &'a Path,
    pub attempt: u32,
    pub program_name: &'a str,
    pub program_path: &'a Path,
    pub program_blake3: &'a str,
    pub resume: Option<ResumeRef>,
}

/// A default fake script for a request without one (fake-child mode over a non-synthetic source): one rule case.
pub fn default_fake_script(req: &JobRequest) -> FakeScript {
    use rustred_campaign_contract::fake::{FakeOp, FakeRule, OnSigterm, RssPlan};
    use rustred_campaign_contract::stop::{CaseResult, StopCode};
    FakeScript {
        script: vec![
            FakeOp::Prepare { ms: 20 },
            FakeOp::Case {
                ordinal: 0,
                ms: 200,
                work: Default::default(),
                result: CaseResult::Rule,
                rule: Some(FakeRule {
                    record: serde_json::json!({"item": req.item, "rung": req.rung.0, "target": "I(fake)", "rhs": []}),
                    loci: None,
                }),
                children: vec![],
            },
            FakeOp::End {
                stop: Some(StopCode::MaxCases),
                verdict: None,
            },
        ],
        on_sigterm: OnSigterm::Graceful,
        slow_ms: 0,
        start_delay_ms: 0,
        rss: RssPlan::default(),
        helpers: vec![],
        faults: vec![],
        time_scale: 1.0,
        volatile_jitter: false,
    }
}

/// The job spec of one attempt.
pub fn job_spec(kind: AdapterKind, req: &JobRequest, a: &AdapterInput<'_>) -> JobSpec {
    let (job_kind, fake, features) = match kind {
        AdapterKind::Fake => (
            JobKind::Fake,
            Some(req.fake.clone().unwrap_or_else(|| default_fake_script(req))),
            vec!["fake".to_string()],
        ),
        AdapterKind::Contract => (req.kind, None, vec![]),
    };
    JobSpec {
        schema: rustred_campaign_contract::CONTRACT.into(),
        job_id: req.job.clone(),
        attempt: a.attempt,
        kind: job_kind,
        run_dir: a.run_dir.to_string_lossy().into_owned(),
        program: ProgramRef {
            name: a.program_name.to_string(),
            blake3: a.program_blake3.to_string(),
            features_required: features,
        },
        inputs: req.inputs.clone(),
        args: if kind == AdapterKind::Fake {
            vec![]
        } else {
            req.args.clone()
        },
        seed: SeedSpec {
            sample_seed: req.sample_seed,
        },
        budgets: req.budgets.clone(),
        on_case_budget: OnCaseBudget::Stop,
        determinism: Determinism::Recorded,
        output: OutputOptions::default(),
        heartbeat_s: req.heartbeat_s.max(1),
        stop_grace_s: req.stop_grace_s.max(1),
        resume_every_s: 0,
        resume: a.resume.clone(),
        env_allow: vec![],
        fake,
    }
}

/// Creates the run directory, writes `job.json` (0444) and returns the launch spec (without marker, CPUs, licence: the
/// supervisor adds those).
pub fn prepare_run(spec: &JobSpec, program_path: &Path) -> std::io::Result<LaunchSpec> {
    let layout = RunLayout::new(&spec.run_dir);
    for d in [layout.in_dir(), layout.out_dir(), layout.tmp_dir(), layout.sup_dir()] {
        std::fs::create_dir_all(d)?;
    }
    let p = layout.job_spec();
    crate::atomic::create_new_atomic(&p, &spec.to_json_line()).map_err(std::io::Error::other)?;
    let mut perm = std::fs::metadata(&p)?.permissions();
    perm.set_readonly(true);
    std::fs::set_permissions(&p, perm)?;
    let ls = LaunchSpec::new(spec.job_id.clone(), program_path, &layout.root)
        .arg("job")
        .arg(p.as_os_str())
        .with_contract_env(&layout.tmp_dir());
    Ok(LaunchSpec {
        stdout: layout.stdout_log(),
        stderr: layout.stderr_log(),
        ..ls
    })
}
