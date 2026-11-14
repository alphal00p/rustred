//! Adapter (a): kind `solve-recipe` (interfaces/adapters.md 2.2). Requests are (recipe, key) pairs; jobs are batches
//! of keys of ONE recipe; outcomes are classified by the contract crate (rows 1 / 28 / 29 accept or type per key);
//! accepted relations become [`SolvedRelation`]s (and, through a [`RelationSink`], relation files); every unsolved key
//! becomes the R48 addendum 3b structured error [`RecipeFailure`] with a ONE-LINE reproduction command.

use super::conformance::{Mismatch, diagnose};
use super::job::{
    AdapterError, Ended, InputFile, JobAdapter, JobPlan, Prepared, Program, classify_attempt,
    create_run, shell_quote, write_input, write_spec,
};
use rustred_campaign_contract::classify::recipe::{FailureClass, KeyOutcome, KeyRoute};
use rustred_campaign_contract::classify::{self, Accept, Action, Classification, OutcomeState};
use rustred_campaign_contract::fake::FakeScript;
use rustred_campaign_contract::spec::{
    Budgets, Determinism, JobKind, JobSpec, OnCaseBudget, OutputOptions, ProgramRef, SeedSpec,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Schema of [`RecipeFailure`] lines.
pub const RECIPE_FAILURE_SCHEMA: &str = "rustred.campaign.recipe-failure.v1";
/// The rung tag of solve-recipe job ids.
pub const RUNG: &str = "sr";

/// A recipe file (absolute path, raw BLAKE3) with its id and, when the caller knows it, its recipe digest (BLAKE3 of
/// the canonical JSON of the PARSED recipe, coord-recipe-solve.md 3: computed by the engine, compared here).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeInput {
    pub id: String,
    pub file: InputFile,
    #[serde(default)]
    pub digest: Option<String>,
}

/// The caller's context of a key (R48 3b): the ORIGINAL input key of the reduction request and the chain of rules
/// from it to this key.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FailureContext {
    #[serde(default)]
    pub request_key: Option<Vec<i64>>,
    #[serde(default)]
    pub rule_chain: Vec<Value>,
}

/// One solve request: a recipe at a key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolveRequest {
    pub recipe: RecipeInput,
    pub key: Vec<i64>,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub context: Option<FailureContext>,
}

/// How keys are batched (data): at most `max_keys_per_job` keys per job; threads = min(`max_threads`,
/// ceil(keys / `min_keys_per_thread`)), at least 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchPolicy {
    pub max_keys_per_job: usize,
    pub max_threads: u32,
    pub min_keys_per_thread: usize,
}

impl BatchPolicy {
    pub fn threads_for(&self, keys: usize) -> u32 {
        let per = self.min_keys_per_thread.max(1);
        let want = keys.div_ceil(per).max(1);
        (want.min(self.max_threads.max(1) as usize)) as u32
    }
}

/// Package identity (with revision) the recipe belongs to (R48 3b).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageId {
    pub name: String,
    pub revision: String,
}

/// The adapter's data.
#[derive(Debug, Clone)]
pub struct SolveRecipeConfig {
    pub program: Program,
    /// The node family in its ORIGINAL presentation.
    pub family: InputFile,
    /// The node order (name or NodeOrder JSON), passed as `--order`.
    pub order: String,
    pub batch: BatchPolicy,
    pub max_trace_rows: Option<u64>,
    pub key_budget_s: Option<u64>,
    /// Certified digests to check against (`--expect` = input role `expect`).
    pub expect: Option<InputFile>,
    /// The determinism id the caller expects (refuses records of another id when set).
    pub determinism_id: Option<String>,
    pub heartbeat_s: u32,
    pub stop_grace_s: u32,
    pub package: Option<PackageId>,
    pub live_repair: bool,
    /// The licence variable's NAME (never a value).
    pub licence_env: Option<String>,
    /// Test stand: a fake script (program `rr_fake_child` emulates the kind).
    pub fake: Option<FakeScript>,
}

/// An accepted relation (row 1 or 28), checked again against its request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolvedRelation {
    pub recipe_id: String,
    pub recipe_digest: String,
    pub determinism_id: String,
    pub key: Vec<i64>,
    pub tag: Option<String>,
    pub rhs_keys: Vec<Vec<i64>>,
    pub relation_digest: String,
    /// closure.md 12 verdict bits (regular | descends | exact): an accepted record has all three.
    pub verdict_bits: u8,
    /// The verified canonical line of the record.
    pub canonical: String,
    /// Request indices served by this relation.
    pub requests: Vec<usize>,
}

/// Node identity in a failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeRef {
    pub family: String,
    pub family_blake3: String,
    pub order: String,
}

/// Recipe identity in a failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeRef {
    pub id: String,
    pub digest: String,
    pub path: String,
    pub blake3: String,
}

/// Binary identity in a failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinaryRef {
    pub path: String,
    pub blake3: String,
}

/// The R48 addendum 3b structured error of one unsolved key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecipeFailure {
    pub schema: String,
    pub node: NodeRef,
    pub package: Option<PackageId>,
    pub recipe: RecipeRef,
    pub key: Vec<i64>,
    pub request_key: Option<Vec<i64>>,
    pub rule_chain: Vec<Value>,
    pub class: FailureClass,
    pub route: KeyRoute,
    pub message: String,
    pub detail: Value,
    pub determinism_id: String,
    pub binary: BinaryRef,
    pub live_repair: bool,
    /// ONE shell line that reruns exactly this solve stand-alone (absolute paths of the spec's own inputs).
    pub reproduce: String,
    /// The child's own reproduction line, verbatim (job mode writes it relative to the run directory).
    pub child_reproduce: Option<String>,
    pub run_dir: String,
    pub requests: Vec<usize>,
}

impl RecipeFailure {
    /// The log / caller form: one JSON line.
    pub fn to_json_line(&self) -> String {
        serde_json::to_string(self).expect("RecipeFailure serialises")
    }
}

/// A key of the batch without a per-key decision (the job-level outcome decides: rerun, ladder, attention).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotDecided {
    pub key: Vec<i64>,
    pub row: u8,
    pub sub: String,
    pub action: Action,
    pub requests: Vec<usize>,
}

/// A record the contract accepted but the adapter refuses against its request (never silently dropped).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordRefusal {
    pub key: Vec<i64>,
    pub why: String,
    pub requests: Vec<usize>,
}

/// Why an attempt is not acceptable: the row, the classifier's reason, and the conformance diagnosis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterRefusal {
    pub row: u8,
    pub sub: String,
    pub why: String,
    pub unsealed_why: Option<String>,
    pub conformance: Vec<Mismatch>,
}

/// The output of one solve-recipe attempt.
#[derive(Debug, Clone)]
pub struct SolveBatch {
    pub job_id: String,
    pub run_dir: PathBuf,
    pub classification: Classification,
    pub relations: Vec<SolvedRelation>,
    pub failures: Vec<RecipeFailure>,
    pub not_decided: Vec<NotDecided>,
    pub refused_records: Vec<RecordRefusal>,
    pub refusal: Option<AdapterRefusal>,
}

/// Adapter (a).
pub struct SolveRecipeAdapter {
    pub config: SolveRecipeConfig,
}

/// The unique keys of a plan (first-appearance order) with the request indices of each.
fn unique_keys(reqs: &[SolveRequest], members: &[usize]) -> Vec<(Vec<i64>, Option<String>, Vec<usize>)> {
    let mut out: Vec<(Vec<i64>, Option<String>, Vec<usize>)> = Vec::new();
    let mut at: HashMap<Vec<i64>, usize> = HashMap::new();
    for &m in members {
        let r = &reqs[m];
        match at.get(&r.key) {
            Some(&i) => out[i].2.push(m),
            None => {
                at.insert(r.key.clone(), out.len());
                out.push((r.key.clone(), r.tag.clone(), vec![m]));
            }
        }
    }
    out
}

fn keys_bytes(keys: &[(Vec<i64>, Option<String>, Vec<usize>)]) -> Vec<u8> {
    let mut b = Vec::new();
    for (k, t, _) in keys {
        let mut v = json!({ "key": k });
        if let Some(t) = t {
            v["tag"] = Value::from(t.clone());
        }
        b.extend_from_slice(serde_json::to_string(&v).unwrap().as_bytes());
        b.push(b'\n');
    }
    b
}

impl SolveRecipeAdapter {
    pub fn new(config: SolveRecipeConfig) -> Self {
        SolveRecipeAdapter { config }
    }

    fn args(&self, threads: u32) -> Vec<String> {
        let c = &self.config;
        let mut a = vec![
            "--order".to_string(),
            c.order.clone(),
            "--threads".into(),
            threads.to_string(),
        ];
        if let Some(r) = c.max_trace_rows {
            a.extend(["--max-trace-rows".into(), r.to_string()]);
        }
        if let Some(s) = c.key_budget_s {
            a.extend(["--key-budget-s".into(), s.to_string()]);
        }
        a
    }

    fn reproduce(&self, recipe: &RecipeInput, key: &[i64], run_dir: &Path) -> String {
        let c = &self.config;
        let k = key
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let out = format!(
            "{}.repro-{}",
            run_dir.display(),
            key.iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join("_")
        );
        let mut line = format!(
            "{} solve-recipe --family {} --order {} --recipe {} --key {k} --out {}",
            shell_quote(&c.program.path.display().to_string()),
            shell_quote(&c.family.path.display().to_string()),
            shell_quote(&c.order),
            shell_quote(&recipe.file.path.display().to_string()),
            shell_quote(&out),
        );
        if let Some(r) = c.max_trace_rows {
            line.push_str(&format!(" --max-trace-rows {r}"));
        }
        if let Some(s) = c.key_budget_s {
            line.push_str(&format!(" --key-budget-s {s}"));
        }
        if let Some(e) = &c.expect {
            line.push_str(&format!(
                " --expect {}",
                shell_quote(&e.path.display().to_string())
            ));
        }
        line
    }

    fn failure(
        &self,
        recipe: &RecipeInput,
        u: &rustred_campaign_contract::classify::recipe::UnsolvedKey,
        ctx: Option<&FailureContext>,
        run_dir: &Path,
        requests: Vec<usize>,
    ) -> RecipeFailure {
        let c = &self.config;
        RecipeFailure {
            schema: RECIPE_FAILURE_SCHEMA.into(),
            node: NodeRef {
                family: c.family.path.display().to_string(),
                family_blake3: c.family.blake3.clone(),
                order: c.order.clone(),
            },
            package: c.package.clone(),
            recipe: RecipeRef {
                id: recipe.id.clone(),
                digest: u.recipe_digest.clone(),
                path: recipe.file.path.display().to_string(),
                blake3: recipe.file.blake3.clone(),
            },
            key: u.key.clone(),
            request_key: ctx.and_then(|x| x.request_key.clone()),
            rule_chain: ctx.map(|x| x.rule_chain.clone()).unwrap_or_default(),
            class: u.class,
            route: u.route,
            message: u.failure["message"].as_str().unwrap_or("").to_string(),
            detail: u.failure["detail"].clone(),
            determinism_id: u.determinism_id.clone(),
            binary: BinaryRef {
                path: c.program.path.display().to_string(),
                blake3: c.program.blake3.clone(),
            },
            live_repair: c.live_repair,
            reproduce: self.reproduce(recipe, &u.key, run_dir),
            child_reproduce: u.failure["reproduce"].as_str().map(str::to_string),
            run_dir: run_dir.display().to_string(),
            requests,
        }
    }
}

impl JobAdapter for SolveRecipeAdapter {
    type Request = SolveRequest;
    type Output = SolveBatch;

    fn kind(&self) -> JobKind {
        JobKind::SolveRecipe
    }

    fn plan(&self, reqs: &[SolveRequest]) -> Result<Vec<JobPlan>, AdapterError> {
        let max = self.config.batch.max_keys_per_job.max(1);
        // group by recipe file (path + raw digest), first-appearance order
        let mut groups: Vec<((String, String), Vec<usize>)> = Vec::new();
        for (i, r) in reqs.iter().enumerate() {
            if r.key.is_empty() {
                return Err(AdapterError::Request(format!("request {i}: empty key")));
            }
            let id = (
                r.recipe.file.path.display().to_string(),
                r.recipe.file.blake3.clone(),
            );
            match groups.iter_mut().find(|(g, _)| *g == id) {
                Some((_, v)) => v.push(i),
                None => groups.push((id, vec![i])),
            }
        }
        let mut plans = Vec::new();
        for (_, members) in groups {
            let uk = unique_keys(reqs, &members);
            let arity = uk[0].0.len();
            if let Some((k, _, _)) = uk.iter().find(|(k, _, _)| k.len() != arity) {
                return Err(AdapterError::Request(format!(
                    "keys of one recipe differ in length ({} vs {arity})",
                    k.len()
                )));
            }
            for chunk in uk.chunks(max) {
                let threads = self.config.batch.threads_for(chunk.len());
                let id = json!({
                    "kind": "solve-recipe",
                    "recipe": reqs[chunk[0].2[0]].recipe.file.blake3,
                    "family": self.config.family.blake3,
                    "expect": self.config.expect.as_ref().map(|e| e.blake3.clone()),
                    "keys": rustred_campaign_contract::blake3_hex(&keys_bytes(chunk)),
                    "args": self.args(threads),
                });
                let content = rustred_campaign_contract::blake3_hex(id.to_string().as_bytes())
                    [..16]
                    .to_string();
                plans.push(JobPlan {
                    job_id: rustred_campaign_contract::layout::job_id(&content, RUNG),
                    members: chunk.iter().flat_map(|(_, _, m)| m.clone()).collect(),
                    content,
                });
            }
        }
        Ok(plans)
    }

    fn prepare(
        &self,
        reqs: &[SolveRequest],
        plan: &JobPlan,
        work_root: &Path,
        attempt: u32,
    ) -> Result<Prepared, AdapterError> {
        let c = &self.config;
        let root = std::path::absolute(work_root).map_err(|e| AdapterError::Io(e.to_string()))?;
        let first = plan
            .members
            .first()
            .ok_or_else(|| AdapterError::Request("empty plan".into()))?;
        let recipe = &reqs[*first].recipe;
        let uk = unique_keys(reqs, &plan.members);
        let layout = create_run(&root, &plan.job_id, attempt)?;
        let keys = write_input(&layout, "keys.jsonl", "keys", &keys_bytes(&uk))?;
        let mut inputs = vec![c.family.input("family"), recipe.file.input("recipe"), keys];
        if let Some(e) = &c.expect {
            inputs.push(e.input("expect"));
        }
        let spec = JobSpec {
            schema: rustred_campaign_contract::CONTRACT.into(),
            job_id: plan.job_id.clone(),
            attempt,
            kind: JobKind::SolveRecipe,
            run_dir: layout.root.display().to_string(),
            program: ProgramRef {
                name: c.program.name.clone(),
                blake3: c.program.blake3.clone(),
                features_required: c.program.features_required.clone(),
            },
            inputs,
            args: self.args(c.batch.threads_for(uk.len())),
            seed: SeedSpec::default(),
            budgets: Budgets::default(),
            on_case_budget: OnCaseBudget::Stop,
            determinism: Determinism::Recorded,
            output: OutputOptions::default(),
            heartbeat_s: c.heartbeat_s.max(1),
            stop_grace_s: c.stop_grace_s.max(1),
            resume_every_s: 30,
            resume: None,
            env_allow: vec![],
            fake: c.fake.clone(),
        };
        write_spec(&spec, &c.program, c.licence_env.as_deref(), &[])
    }

    fn finish(
        &self,
        reqs: &[SolveRequest],
        plan: &JobPlan,
        prep: &Prepared,
        end: &Ended,
    ) -> SolveBatch {
        let (ev, c) = classify_attempt(prep, end);
        let uk = unique_keys(reqs, &plan.members);
        let recipe = &reqs[plan.members[0]].recipe;
        let mut out = SolveBatch {
            job_id: plan.job_id.clone(),
            run_dir: prep.layout.root.clone(),
            classification: c.clone(),
            relations: vec![],
            failures: vec![],
            not_decided: vec![],
            refused_records: vec![],
            refusal: None,
        };
        let per_key = matches!((c.row, c.sub.as_str()), (1, "") | (28, "") | (29, ""));
        let outcomes = match (&ev.outcome, &ev.verified, per_key) {
            (OutcomeState::Valid(o), Some(v), true) => {
                classify::solve_recipe_tally(&prep.layout, &prep.spec, o, v).map(|(x, _)| x)
            }
            _ => Err("no per-key decision for this row".to_string()),
        };
        let outcomes = match outcomes {
            Ok(x) => x,
            Err(why) => {
                for (k, _, m) in &uk {
                    out.not_decided.push(NotDecided {
                        key: k.clone(),
                        row: c.row,
                        sub: c.sub.clone(),
                        action: c.action,
                        requests: m.clone(),
                    });
                }
                if c.accept == Accept::Nothing && !matches!(c.row, 9 | 10 | 11 | 18 | 19 | 20 | 21) {
                    out.refusal = Some(AdapterRefusal {
                        row: c.row,
                        sub: c.sub.clone(),
                        why: if per_key { why } else { c.why.clone() },
                        unsealed_why: ev.unsealed_why.clone(),
                        conformance: diagnose(&prep.layout, &prep.spec, &prep.spec_blake3),
                    });
                }
                return out;
            }
        };
        for o in outcomes {
            let i = o.ordinal() as usize;
            let requests = uk.get(i).map(|x| x.2.clone()).unwrap_or_default();
            match o {
                KeyOutcome::Accepted(a) => {
                    let mut why = None;
                    if c.accept == Accept::Nothing {
                        why = Some("the row accepts nothing".to_string());
                    }
                    if let Some(d) = &recipe.digest
                        && *d != a.recipe_digest
                    {
                        why = Some(format!(
                            "recipe digest {} is not the request's {d}",
                            a.recipe_digest
                        ));
                    }
                    if let Some(d) = &self.config.determinism_id
                        && *d != a.determinism_id
                    {
                        why = Some(format!(
                            "determinism id {} is not the expected {d}",
                            a.determinism_id
                        ));
                    }
                    match why {
                        Some(why) => out.refused_records.push(RecordRefusal {
                            key: a.key,
                            why,
                            requests,
                        }),
                        None => out.relations.push(SolvedRelation {
                            recipe_id: recipe.id.clone(),
                            recipe_digest: a.recipe_digest,
                            determinism_id: a.determinism_id,
                            key: a.key,
                            tag: a.tag,
                            rhs_keys: a.rhs_keys,
                            relation_digest: a.relation_digest,
                            verdict_bits: VERDICT_ALL,
                            canonical: String::from_utf8_lossy(&a.canonical).into_owned(),
                            requests,
                        }),
                    }
                }
                KeyOutcome::Unsolved(u) => {
                    let ctx = requests.first().and_then(|&r| reqs[r].context.as_ref());
                    let f = self.failure(recipe, &u, ctx, &prep.layout.root, requests);
                    out.failures.push(f);
                }
            }
        }
        out
    }
}

/// closure.md 12 verdict bits: regular 1 | descends 2 | exact 4.
pub const VERDICT_ALL: u8 = 7;

/// Where accepted relations go (interfaces/adapters.md 2.2): one implementation per file format.
pub trait RelationSink {
    /// Write the relations; returns the files written (path, digest).
    fn write(&mut self, rels: &[SolvedRelation]) -> Result<Vec<(PathBuf, String)>, AdapterError>;
}

/// closure.md section 12's `rustred.campaign.relations.v1` (one file per (recipe digest, determinism id)), through
/// campaign-core's `RelationStoreBuilder`.
pub struct ClosureRelationsV1 {
    pub dir: PathBuf,
    /// Bits per key slot of the key codec (4..=16).
    pub bits: u32,
}

impl RelationSink for ClosureRelationsV1 {
    fn write(&mut self, rels: &[SolvedRelation]) -> Result<Vec<(PathBuf, String)>, AdapterError> {
        use rustred_campaign_core::closure::{KeyCodec, RelationStoreBuilder};
        let mut groups: Vec<((String, String), Vec<&SolvedRelation>)> = Vec::new();
        for r in rels {
            let id = (r.recipe_digest.clone(), r.determinism_id.clone());
            match groups.iter_mut().find(|(g, _)| *g == id) {
                Some((_, v)) => v.push(r),
                None => groups.push((id, vec![r])),
            }
        }
        let to32 = |k: &[i64]| -> Result<Vec<i32>, AdapterError> {
            k.iter()
                .map(|&x| i32::try_from(x).map_err(|_| AdapterError::Request(format!("key value {x}"))))
                .collect()
        };
        std::fs::create_dir_all(&self.dir).map_err(|e| AdapterError::Io(e.to_string()))?;
        let mut files = Vec::new();
        for ((recipe_digest, _det), rs) in groups {
            let arity = rs[0].key.len();
            let codec = KeyCodec::new(arity, self.bits)
                .map_err(|e| AdapterError::Request(format!("{e:?}")))?;
            let mut b = RelationStoreBuilder::new(codec);
            for r in rs {
                let mut d = [0u8; 32];
                for (i, byte) in d.iter_mut().enumerate() {
                    *byte = u8::from_str_radix(&r.relation_digest[2 * i..2 * i + 2], 16)
                        .map_err(|e| AdapterError::Request(e.to_string()))?;
                }
                let rhs: Vec<Vec<i32>> = r
                    .rhs_keys
                    .iter()
                    .map(|k| to32(k))
                    .collect::<Result<_, _>>()?;
                b.push(&to32(&r.key)?, &rhs, d, r.verdict_bits)
                    .map_err(|e| AdapterError::Request(format!("{e:?}")))?;
            }
            let store = b
                .finish()
                .map_err(|e| AdapterError::Request(format!("{e:?}")))?;
            let name = format!("recipe-{}", &recipe_digest[..recipe_digest.len().min(16)]);
            let mut bytes = Vec::new();
            let digest = store
                .write(&name, &recipe_digest, &mut bytes)
                .map_err(|e| AdapterError::Io(e.to_string()))?;
            let p = self.dir.join(format!("{name}.relations"));
            crate::atomic::create_new_atomic(&p, &bytes)
                .map_err(|e| AdapterError::Io(format!("{}: {e}", p.display())))?;
            files.push((p, digest));
        }
        Ok(files)
    }
}
