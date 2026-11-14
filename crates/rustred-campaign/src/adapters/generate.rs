//! Adapter (c): `rr_node job` kind `generate` (interfaces/adapters.md 2.4), with the seed options of EC-1 v2
//! (coord-ec1-seed-centre.md section 9) as typed fields mapped 1:1 to flags (no policy here: the ladder decides what to
//! ask for). Outputs are read ONLY through verified handles (the table, the optional open list `F.subcases.jsonl`).

use super::conformance::diagnose;
use super::job::{
    AdapterError, Ended, InputFile, JobAdapter, JobPlan, Prepared, Program, classify_attempt,
    create_run, write_input, write_spec,
};
use super::solve_recipe::AdapterRefusal;
use rustred_campaign_contract::classify::{Accept, Classification};
use rustred_campaign_contract::fake::FakeScript;
use rustred_campaign_contract::layout::{RunLayout, SUBCASES, TABLE};
use rustred_campaign_contract::seal::VerifiedHandle;
use rustred_campaign_contract::spec::{
    Budgets, Determinism, JobKind, JobSpec, OnCaseBudget, OutputOptions, ProgramRef, SeedSpec,
    WallBudgets, WorkBudgets,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

/// The rung tag of generate job ids.
pub const RUNG: &str = "gen";

/// Seed and search options, each mapped to exactly one flag of the generate allow-list (None / false = absent).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedOptions {
    pub max_depth: Option<u64>,
    pub numeric_depth: Option<u64>,
    pub numeric_depth_max: Option<u64>,
    /// `--seed-centre-offset` (`S:D[,..][;..]`).
    pub centres: Option<String>,
    pub face_floor: Option<u64>,
    /// `--seed-layer-cap D,R[,B]` (needs `max_depth`).
    pub layer_cap: Option<String>,
    pub symmetry_symbolic: bool,
    pub symmetry_rows: bool,
    pub open_on_refusal: bool,
    pub no_screen: bool,
    /// `--screen MIN_ROWS,MAX_POINTS,MAX_DEG,MAX_RHS`.
    pub screen: Option<String>,
    /// `--on-heavy auto|refuse|sampled|proceed`.
    pub on_heavy: Option<String>,
    /// `--seed-schedule`.
    pub seed_schedule: Option<String>,
}

impl SeedOptions {
    /// "Symmetry rows at the key alone" (EC-1 v2 section 9): `--max-depth 0 --numeric-depth 0 --symmetry-symbolic`
    /// with `--open-on-refusal` (a depth-0 exhaustion is an open obligation, not a node error).
    pub fn symmetry_at_key_alone() -> SeedOptions {
        SeedOptions {
            max_depth: Some(0),
            numeric_depth: Some(0),
            symmetry_symbolic: true,
            open_on_refusal: true,
            ..SeedOptions::default()
        }
    }

    /// The flags, in a fixed order.
    pub fn args(&self) -> Vec<String> {
        let mut a: Vec<String> = Vec::new();
        let mut kv = |f: &str, v: Option<String>| {
            if let Some(v) = v {
                a.push(f.into());
                a.push(v);
            }
        };
        kv("--max-depth", self.max_depth.map(|x| x.to_string()));
        kv("--numeric-depth", self.numeric_depth.map(|x| x.to_string()));
        kv(
            "--numeric-depth-max",
            self.numeric_depth_max.map(|x| x.to_string()),
        );
        kv("--seed-centre-offset", self.centres.clone());
        kv("--seed-face-floor", self.face_floor.map(|x| x.to_string()));
        kv("--seed-layer-cap", self.layer_cap.clone());
        kv("--screen", self.screen.clone());
        kv("--on-heavy", self.on_heavy.clone());
        kv("--seed-schedule", self.seed_schedule.clone());
        for (on, f) in [
            (self.symmetry_symbolic, "--symmetry-symbolic"),
            (self.symmetry_rows, "--symmetry-rows"),
            (self.open_on_refusal, "--open-on-refusal"),
            (self.no_screen, "--no-screen"),
        ] {
            if on {
                a.push(f.into());
            }
        }
        a
    }
}

/// One generate job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerateRequest {
    pub family: InputFile,
    /// PRESENTED case lines (each a valid line of a `cases` input); empty = the whole chart.
    #[serde(default)]
    pub cases: Vec<Value>,
    /// The node order (name or NodeOrder JSON).
    pub order: String,
    #[serde(default)]
    pub seed: SeedOptions,
    /// `budgets.work.cases` (old `--max-cases`).
    #[serde(default)]
    pub max_cases: Option<u64>,
    /// `--descend open`.
    #[serde(default)]
    pub descend_open: bool,
    #[serde(default)]
    pub wall_case_s: Option<u64>,
    #[serde(default)]
    pub wall_job_s: Option<u64>,
}

/// The output of one generate attempt.
#[derive(Debug, Clone)]
pub struct GenerateResult {
    pub job_id: String,
    pub classification: Classification,
    /// Canonical lines accepted per the row's accept column (canonical: every record; committed_rules: the rules).
    pub accepted: Vec<Vec<u8>>,
    /// Canonical lines of the verified open list (`F.subcases.jsonl`), when listed.
    pub subcases: Vec<Vec<u8>>,
    pub refusal: Option<AdapterRefusal>,
}

/// Adapter (c).
pub struct GenerateAdapter {
    pub program: Program,
    pub licence_env: Option<String>,
    pub heartbeat_s: u32,
    pub stop_grace_s: u32,
    /// Test stand: a fake script (kind fake with program rr_fake_child).
    pub fake: Option<FakeScript>,
}

impl GenerateAdapter {
    fn args(&self, r: &GenerateRequest) -> Vec<String> {
        let mut a = vec!["--order".to_string(), r.order.clone()];
        if r.descend_open {
            a.extend(["--descend".into(), "open".into()]);
        }
        a.extend(r.seed.args());
        a
    }
}

fn cases_bytes(cases: &[Value]) -> Vec<u8> {
    let mut b = Vec::new();
    for c in cases {
        b.extend_from_slice(c.to_string().as_bytes());
        b.push(b'\n');
    }
    b
}

impl JobAdapter for GenerateAdapter {
    type Request = GenerateRequest;
    type Output = GenerateResult;

    fn kind(&self) -> JobKind {
        if self.fake.is_some() {
            JobKind::Fake
        } else {
            JobKind::Generate
        }
    }

    fn plan(&self, reqs: &[GenerateRequest]) -> Result<Vec<JobPlan>, AdapterError> {
        Ok(reqs
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let id = json!({"kind": "generate", "family": r.family.blake3, "args": self.args(r),
                    "cases": rustred_campaign_contract::blake3_hex(&cases_bytes(&r.cases)),
                    "max_cases": r.max_cases, "wall": [r.wall_case_s, r.wall_job_s]});
                let content =
                    rustred_campaign_contract::blake3_hex(id.to_string().as_bytes())[..16].to_string();
                JobPlan {
                    job_id: rustred_campaign_contract::layout::job_id(&content, RUNG),
                    members: vec![i],
                    content,
                }
            })
            .collect())
    }

    fn prepare(
        &self,
        reqs: &[GenerateRequest],
        plan: &JobPlan,
        work_root: &Path,
        attempt: u32,
    ) -> Result<Prepared, AdapterError> {
        let root = std::path::absolute(work_root).map_err(|e| AdapterError::Io(e.to_string()))?;
        let r = &reqs[plan.members[0]];
        let layout = create_run(&root, &plan.job_id, attempt)?;
        let mut inputs = vec![r.family.input("family")];
        if !r.cases.is_empty() {
            inputs.push(write_input(&layout, "cases.jsonl", "cases", &cases_bytes(&r.cases))?);
        }
        let fake = self.fake.is_some();
        let spec = JobSpec {
            schema: rustred_campaign_contract::CONTRACT.into(),
            job_id: plan.job_id.clone(),
            attempt,
            kind: self.kind(),
            run_dir: layout.root.display().to_string(),
            program: ProgramRef {
                name: self.program.name.clone(),
                blake3: self.program.blake3.clone(),
                features_required: self.program.features_required.clone(),
            },
            inputs,
            args: if fake { vec![] } else { self.args(r) },
            seed: SeedSpec::default(),
            budgets: Budgets {
                work: WorkBudgets {
                    cases: r.max_cases,
                    ..WorkBudgets::default()
                },
                wall: WallBudgets {
                    case_s: r.wall_case_s,
                    job_s: r.wall_job_s,
                },
                mem_soft_bytes: None,
            },
            on_case_budget: OnCaseBudget::Stop,
            determinism: Determinism::Recorded,
            output: OutputOptions::default(),
            heartbeat_s: self.heartbeat_s.max(1),
            stop_grace_s: self.stop_grace_s.max(1),
            resume_every_s: 0,
            resume: None,
            env_allow: vec![],
            fake: self.fake.clone(),
        };
        write_spec(&spec, &self.program, self.licence_env.as_deref(), &[])
    }

    fn finish(
        &self,
        _reqs: &[GenerateRequest],
        plan: &JobPlan,
        prep: &Prepared,
        end: &Ended,
    ) -> GenerateResult {
        let (ev, c) = classify_attempt(prep, end);
        let mut out = GenerateResult {
            job_id: plan.job_id.clone(),
            classification: c.clone(),
            accepted: vec![],
            subcases: vec![],
            refusal: None,
        };
        let read = |rel: &str| -> Option<Result<Vec<Vec<u8>>, String>> {
            let v = ev.verified.as_ref()?;
            let s = v.streams.iter().find(|s| s.rel == rel)?;
            Some(
                VerifiedHandle::reopen(&prep.layout.root, s)
                    .and_then(|h| h.canonical_lines())
                    .map_err(|e| e.to_string()),
            )
        };
        if c.accept != Accept::Nothing {
            let lines = match read(&RunLayout::out_rel(TABLE)) {
                Some(r) => r,
                None => Err("no verified table".to_string()),
            };
            match lines {
                Ok(lines) => {
                    out.accepted = match c.accept {
                        Accept::CommittedRules => lines
                            .into_iter()
                            .filter(|l| {
                                rustred_campaign_contract::canon::line_type(l)
                                    .ok()
                                    .flatten()
                                    .as_deref()
                                    == Some("rule")
                            })
                            .collect(),
                        _ => lines,
                    }
                }
                Err(why) => {
                    out.refusal = Some(AdapterRefusal {
                        row: c.row,
                        sub: c.sub.clone(),
                        why,
                        unsealed_why: ev.unsealed_why.clone(),
                        conformance: vec![],
                    });
                    return out;
                }
            }
        } else if !matches!(c.row, 3 | 24 | 27 | 9 | 10 | 11 | 18 | 19 | 20 | 21) {
            out.refusal = Some(AdapterRefusal {
                row: c.row,
                sub: c.sub.clone(),
                why: c.why.clone(),
                unsealed_why: ev.unsealed_why.clone(),
                conformance: diagnose(&prep.layout, &prep.spec, &prep.spec_blake3),
            });
        }
        if let Some(Ok(l)) = read(&RunLayout::out_rel(SUBCASES)) {
            out.subcases = l;
        }
        out
    }
}
