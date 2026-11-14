//! Phase C: what a settled solve-recipe attempt certifies (interfaces/closeloop.md 5). Input = the contract-VERIFIED
//! canonical lines of the sealed results stream (never a path re-read); per key: the contract's `key_outcomes` (key
//! and ordinal bound to the job's keys input), then the core's recipe acceptance chain `accept_recipe` (dispatch to the
//! recipe, an answer of THIS recipe digest and binary, exact proof, regularity, descent under the full comparator).
//! Output = the answers lines (`rustred.campaign.solved-relation.v1`, interfaces/import.md 6.2) for every decided key:
//! certified relations and typed failures carrying the R48 addendum 3b record with a one-line reproduction command.

use crate::adapters::job::shell_quote;
use crate::adapters::solve_recipe::RECIPE_FAILURE_SCHEMA;
use rustred_campaign_contract::classify::recipe::{
    FailureClass, KeyLine, KeyOutcome, KeyRoute, key_outcomes,
};
use rustred_campaign_core::accept::recipe::{
    Determinism, OpenReason, Recipe, RecipeFile, RecipeInput, RecipeVerdict, SolveAnswers,
    accept_recipe,
};
use rustred_campaign_core::accept::{NodeTarget, NodeView, RequiredEvidence, node_identity};
use rustred_campaign_core::closure::sha256::Sha256;
use rustred_campaign_core::cover::Dispatcher;
use rustred_campaign_core::import::ImportedSet;
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const SOLVED_SCHEMA: &str = "rustred.campaign.solved-relation.v1";

/// What one batch is (from its batch file).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BatchFile {
    pub schema: String,
    pub node: String,
    pub item: String,
    pub round: u32,
    pub source: String,
    pub recipe_path: String,
    pub recipe_sha256: String,
    pub recipe_blake3: String,
    pub engine_digest: Option<String>,
    /// The relation rule in force for this batch (config `solve.relation_rule`).
    #[serde(default)]
    pub relation_rule: String,
    pub keys: Vec<Vec<i32>>,
}

pub const BATCH_SCHEMA: &str = "rustred.campaign.closeloop-batch.v1";

/// Context of the acceptance of one attempt.
pub struct AcceptCtx<'a> {
    pub set: &'a ImportedSet,
    pub before: Option<&'a Dispatcher>,
    pub boxed: bool,
    pub batch: &'a BatchFile,
    /// The engine digest of this recipe: the spec's, else pinned by an earlier answer of the node; None = pin now.
    pub pinned_digest: Option<String>,
    /// The determinism id required by the config (None = the answers' own).
    pub determinism_id: Option<String>,
    /// Identity of the binary that solved (supervisor program digest) and its sha256 (the import's binary pin).
    pub binary_blake3: String,
    pub binary_sha256: String,
    pub binary_path: String,
    pub family_path: String,
    pub family_blake3: String,
    pub order: String,
    pub results_path: String,
    pub results_blake3: String,
    pub run_dir: String,
}

/// The per-key decisions of one attempt.
#[derive(Debug, Default)]
pub struct Decided {
    /// One solved-relation v1 line per decided key (certified first-class, failures typed), in batch order.
    pub lines: Vec<String>,
    pub certified: Vec<Vec<i32>>,
    pub failed: Vec<(Vec<i32>, String)>,
    /// The engine digest the answers carry (pinned by this attempt when none was pinned).
    pub engine_digest: Option<String>,
    pub determinism_id: Option<String>,
    /// Why the whole attempt was refused (every SOLVED record became a failure), if so.
    pub refused: Option<String>,
}

fn sha256_hex(b: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b);
    h.finish_hex()
}

fn key_i64(k: &[i32]) -> Vec<i64> {
    k.iter().map(|&x| x as i64).collect()
}

fn reproduce(c: &AcceptCtx<'_>, key: &[i64]) -> String {
    let k = key
        .iter()
        .map(|x| x.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let out = format!(
        "{}.repro-{}",
        c.run_dir,
        key.iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join("_")
    );
    format!(
        "{} solve-recipe --family {} --order {} --recipe {} --key {k} --out {}",
        shell_quote(&c.binary_path),
        shell_quote(&c.family_path),
        shell_quote(&c.order),
        shell_quote(&c.batch.recipe_path),
        shell_quote(&out),
    )
}

/// The R48 addendum 3b record of one key that is not certified (field names as the adapters lane's
/// `RecipeFailure`, schema `rustred.campaign.recipe-failure.v1`; built here as data so the two stay decoupled).
#[allow(clippy::too_many_arguments)]
fn failure_record(
    c: &AcceptCtx<'_>,
    key: &[i64],
    ordinal: u64,
    verdict: &str,
    class: Option<FailureClass>,
    route: Option<KeyRoute>,
    message: String,
    detail: Value,
    child_failure: Value,
    digest: &str,
    det: &str,
) -> Value {
    json!({
        "schema": RECIPE_FAILURE_SCHEMA,
        "node": {"family": c.family_path, "family_blake3": c.family_blake3, "order": c.order},
        "package": null,
        "recipe": {"id": c.batch.source, "digest": digest, "path": c.batch.recipe_path,
                   "blake3": c.batch.recipe_blake3, "sha256": c.batch.recipe_sha256},
        "key": key, "ordinal": ordinal, "request_key": null, "rule_chain": [],
        "verdict": verdict,
        "class": class.map(|x| x.name()).unwrap_or_else(|| "refused".into()),
        "route": route.unwrap_or(KeyRoute::Attention),
        "message": message, "detail": detail, "determinism_id": det,
        "binary": {"path": c.binary_path, "blake3": c.binary_blake3},
        "live_repair": false,
        "reproduce": reproduce(c, key),
        "child_reproduce": child_failure["reproduce"].clone(),
        "run_dir": c.run_dir,
        "child_failure": child_failure,
        "results": {"path": c.results_path, "blake3": c.results_blake3, "ordinal": ordinal},
    })
}

fn line(v: &Value) -> String {
    rustred_campaign_core::schema::canon::canonical_json(v, false).unwrap_or_else(|_| v.to_string())
}

/// Decide every key of the batch from the verified canonical lines of its sealed results stream.
pub fn decide(c: &AcceptCtx<'_>, canonical_lines: &[String]) -> Result<Decided, String> {
    let keys: Vec<KeyLine> = c
        .batch
        .keys
        .iter()
        .map(|k| KeyLine {
            key: key_i64(k),
            tag: None,
        })
        .collect();
    let bytes: Vec<Vec<u8>> = canonical_lines.iter().map(|l| l.as_bytes().to_vec()).collect();
    let (outcomes, tally) = key_outcomes(&bytes, &keys)?;
    let digest = tally
        .recipe_digest
        .clone()
        .ok_or("results without a recipe digest")?;
    let det = tally
        .determinism_id
        .clone()
        .ok_or("results without a determinism id")?;
    let mut out = Decided {
        engine_digest: Some(digest.clone()),
        determinism_id: Some(det.clone()),
        ..Decided::default()
    };
    // refusals of the whole attempt (a lying or stale child): every SOLVED record becomes a failure
    let mut refused: Option<String> = None;
    if let Some(p) = &c.pinned_digest
        && *p != digest
    {
        refused = Some(format!(
            "recipe digest {digest} is not the recipe's pinned engine digest {p}"
        ));
    }
    if let Some(d) = &c.determinism_id
        && *d != det
    {
        refused = Some(format!("determinism id {det} is not the required {d}"));
    }
    // the core chain over every key of the batch
    let mut verdicts: BTreeMap<Vec<i32>, Result<(), String>> = BTreeMap::new();
    if refused.is_none() {
        let text = canonical_lines.join("\n");
        let file = RecipeFile::parse(
            &std::fs::read_to_string(&c.batch.recipe_path)
                .map_err(|e| format!("recipe {}: {e}", c.batch.recipe_path))?,
        )
        .map_err(|e| format!("recipe {}: {e}", c.batch.recipe_path))?;
        let order_digest = node_identity(&c.set.frame, &c.set.cmp);
        let recipe = Recipe::from_file(&file, &order_digest, &det, &c.set.frame)?;
        let mut answers = SolveAnswers::new(recipe.digest());
        match answers.load_engine_results(&text, 1, &digest, &c.binary_blake3) {
            Err(e) => refused = Some(format!("answers refused by the acceptance chain: {e}")),
            Ok(_) => {
                let terminals = c
                    .set
                    .terminals_preferred
                    .iter()
                    .chain(c.set.terminals_other.iter())
                    .cloned()
                    .collect();
                let node = NodeView {
                    node: c.batch.node.clone(),
                    frame: &c.set.frame,
                    cmp: &c.set.cmp,
                    target: if c.boxed {
                        NodeTarget::Boxed {
                            box_id: "spec-box".into(),
                        }
                    } else {
                        NodeTarget::Full
                    },
                    required: RequiredEvidence::PerKey,
                    zero_sectors: None,
                    terminals,
                };
                let input = RecipeInput {
                    node,
                    before: c.before,
                    after: None,
                    recipe: &recipe,
                    keys: &c.batch.keys,
                    determinism: Determinism::None,
                    expected_binary: Some(c.binary_blake3.clone()),
                };
                match accept_recipe(&input, &answers).map_err(|e| format!("{e:?}"))? {
                    RecipeVerdict::Certified {
                        certified, open, ..
                    } => {
                        for k in certified {
                            verdicts.insert(k, Ok(()));
                        }
                        for (k, why) in open {
                            verdicts.insert(k, Err(open_word(&why)));
                        }
                    }
                    RecipeVerdict::Rejected { what } => {
                        for k in &c.batch.keys {
                            verdicts.insert(k.clone(), Err(format!("rejected: {what}")));
                        }
                    }
                    RecipeVerdict::Undecided { what } => {
                        refused = Some(format!("acceptance chain undecided: {what}"))
                    }
                    RecipeVerdict::NeedsExternal { .. } => {
                        refused = Some("acceptance chain needs answers the job did not give".into())
                    }
                }
            }
        }
    }
    for o in outcomes {
        match o {
            KeyOutcome::Accepted(a) => {
                let k32: Vec<i32> = a.key.iter().map(|&x| x as i32).collect();
                let rec: Value = serde_json::from_slice(&a.canonical).unwrap_or(Value::Null);
                let taken = rec["relation_rule"].as_str().map(str::to_string);
                let decision = match &refused {
                    Some(w) => Err(w.clone()),
                    None if c.batch.relation_rule == "m1_first"
                        && taken.as_deref() != Some("m1_first") =>
                    {
                        Err(format!(
                            "relation rule m1_first required, the answer says {taken:?} (engine without the field)"
                        ))
                    }
                    None => verdicts
                        .get(&k32)
                        .cloned()
                        .unwrap_or_else(|| Err("not judged by the acceptance chain".into())),
                };
                let rule_label = match &taken {
                    Some(t) => format!(
                        "{t} (taken: {})",
                        rec["relation_rule_taken"].as_str().unwrap_or("-")
                    ),
                    None => format!(
                        "{} (engine answer without a relation_rule field: the comparator's first hit)",
                        c.batch.relation_rule
                    ),
                };
                match decision {
                    Ok(()) => {
                        let rhs: Vec<Value> = rec["rhs"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|t| {
                                let key = t[0]
                                    .as_str()
                                    .and_then(rustred_campaign_core::assemble::parse_key_text)
                                    .map(Value::from)
                                    .unwrap_or(Value::Null);
                                json!({"key": key, "coef_sha256": sha256_hex(t[1].as_str().unwrap_or("").as_bytes()),
                                       "mark": t[2].clone()})
                            })
                            .collect();
                        let v = json!({"schema": SOLVED_SCHEMA, "source": c.batch.source,
                            "recipe_sha256": c.batch.recipe_sha256, "recipe_digest": a.recipe_digest,
                            "determinism_id": a.determinism_id, "binary_sha256": c.binary_sha256,
                            "key": k32, "verdict": "SOLVED", "relation_digest": a.relation_digest, "rhs": rhs,
                            "proof": rec["proof"].clone(), "regular": true,
                            "results": {"path": c.results_path, "blake3": c.results_blake3, "ordinal": a.ordinal},
                            "relation_rule": rule_label, "failure": null});
                        out.lines.push(line(&v));
                        out.certified.push(k32);
                    }
                    Err(why) => {
                        let f = failure_record(
                            c,
                            &a.key,
                            a.ordinal,
                            "REFUSED",
                            None,
                            Some(KeyRoute::Attention),
                            format!("SOLVED record not certified: {why}"),
                            json!({"reason": why}),
                            Value::Null,
                            &a.recipe_digest,
                            &a.determinism_id,
                        );
                        let v = json!({"schema": SOLVED_SCHEMA, "source": c.batch.source,
                            "recipe_sha256": c.batch.recipe_sha256, "recipe_digest": a.recipe_digest,
                            "determinism_id": a.determinism_id, "binary_sha256": c.binary_sha256,
                            "key": k32, "verdict": "REFUSED", "relation_digest": null, "rhs": [],
                            "proof": null, "regular": false,
                            "results": {"path": c.results_path, "blake3": c.results_blake3, "ordinal": a.ordinal},
                            "relation_rule": rule_label,
                            "failure": {"class": "refused", "message": why}, "failure_record": f});
                        out.lines.push(line(&v));
                        out.failed.push((k32, "refused".into()));
                    }
                }
            }
            KeyOutcome::Unsolved(u) => {
                let k32: Vec<i32> = u.key.iter().map(|&x| x as i32).collect();
                let f = failure_record(
                    c,
                    &u.key,
                    u.ordinal,
                    &u.verdict.name(),
                    Some(u.class),
                    Some(u.route),
                    u.failure["message"].as_str().unwrap_or("").to_string(),
                    u.failure["detail"].clone(),
                    u.failure.clone(),
                    &u.recipe_digest,
                    &u.determinism_id,
                );
                let v = json!({"schema": SOLVED_SCHEMA, "source": c.batch.source,
                    "recipe_sha256": c.batch.recipe_sha256, "recipe_digest": u.recipe_digest,
                    "determinism_id": u.determinism_id, "binary_sha256": c.binary_sha256,
                    "key": k32, "verdict": u.verdict.name(), "relation_digest": null, "rhs": [],
                    "proof": null, "regular": false,
                    "results": {"path": c.results_path, "blake3": c.results_blake3, "ordinal": u.ordinal},
                    "relation_rule": c.batch.relation_rule,
                    "failure": {"class": u.class.name(), "route": u.route, "message": u.failure["message"].clone()},
                    "failure_record": f});
                out.lines.push(line(&v));
                out.failed.push((k32, u.class.name().to_string()));
            }
        }
    }
    out.refused = refused;
    Ok(out)
}

fn open_word(r: &OpenReason) -> String {
    match r {
        OpenReason::NotDispatched(w) => format!("not dispatched to the recipe: {w}"),
        OpenReason::SolveFailed(w) => format!("solve failed: {w}"),
        OpenReason::ExactCheck(w) => format!("exact check {w}"),
        OpenReason::Irregular(w) => format!("irregular: {w}"),
        OpenReason::NotDescending { rhs } => format!("rhs key {rhs:?} not lower under the node comparator"),
        OpenReason::Malformed(w) => format!("malformed: {w}"),
    }
}
