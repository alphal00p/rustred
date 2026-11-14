#![allow(dead_code)]
//! Shared helpers of the contract crate's integration tests.

use rustred_campaign_contract::outcome::*;
use rustred_campaign_contract::spec::*;
use rustred_campaign_contract::stop::*;
use rustred_campaign_contract::{ExitCode, OUTCOME_SCHEMA};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static N: AtomicU32 = AtomicU32::new(0);

/// A fresh directory under the system temp dir, removed on drop.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(tag: &str) -> TempDir {
        let p = std::env::temp_dir().join(format!(
            "rr-contract-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub const D0: &str = "0000000000000000000000000000000000000000000000000000000000000000";
pub const D1: &str = "1111111111111111111111111111111111111111111111111111111111111111";

pub fn sample_spec(run_dir: &str) -> JobSpec {
    JobSpec {
        schema: "rustred.child-job.v1".into(),
        job_id: "a71d0c9e".into(),
        attempt: 1,
        kind: JobKind::Generate,
        run_dir: run_dir.into(),
        program: ProgramRef {
            name: "rr_node".into(),
            blake3: D0.into(),
            features_required: vec!["initial_presented".into()],
        },
        inputs: vec![InputRef {
            role: "family".into(),
            path: "/abs/T162.sym".into(),
            blake3: D1.into(),
            upto: None,
        }],
        args: vec![
            "--order".into(),
            "{\"name\":\"phys_swap_0_3_rot7\"}".into(),
            "--symmetry-rows".into(),
        ],
        seed: SeedSpec { sample_seed: 0 },
        budgets: Budgets {
            work: WorkBudgets {
                cases: Some(1),
                seeds_per_case: Some(200000),
                ..Default::default()
            },
            wall: WallBudgets {
                case_s: Some(300),
                job_s: Some(10800),
            },
            mem_soft_bytes: Some(4_500_000_000),
        },
        on_case_budget: OnCaseBudget::Stop,
        determinism: Determinism::Recorded,
        output: OutputOptions::default(),
        heartbeat_s: 10,
        stop_grace_s: 20,
        resume_every_s: 30,
        resume: None,
        env_allow: vec![],
        fake: None,
    }
}

pub fn program_info() -> ProgramInfo {
    ProgramInfo {
        name: "rr_node".into(),
        version: "0.1.0".into(),
        git_commit: "unknown".into(),
        features: vec!["initial_presented".into()],
        exe: None,
    }
}

/// A valid OUTCOME `job` for `spec` with exit `code` and stop `stop`, sealed table if `sealed`.
pub fn job_outcome(
    spec: &JobSpec,
    code: ExitCode,
    stop: Option<StopCode>,
    sealed: bool,
) -> JobOutcome {
    let mut j = JobOutcome::for_spec(
        spec,
        D0,
        program_info(),
        code,
        StopInfo {
            code: stop,
            detail: serde_json::Value::Null,
            source: None,
            graceful: true,
        },
    );
    assert_eq!(j.schema, OUTCOME_SCHEMA);
    j.outputs.table = Some(TableOutput {
        path: "out/F.table.jsonl".into(),
        bytes: sealed.then_some(10),
        blake3: sealed.then(|| D1.to_string()),
        records: sealed.then_some(2),
        rules: Some(0),
        rules_committed: 0,
        committed_bytes: 0,
        prefix_blake3: None,
        sealed,
        end: sealed.then_some(if stop.is_none() {
            EndKind::End
        } else {
            EndKind::Partial
        }),
        canonical_blake3: sealed.then(|| D1.to_string()),
        witness_loci_elided: 0,
    });
    if code == ExitCode::BudgetWall {
        j.time_decided = true;
    }
    j
}
