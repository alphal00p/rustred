//! Phase A / the re-check of phase C / phase D: the in-process pass (interfaces/closeloop.md 1, 7, 8). A PURE function
//! of its input (the effective import spec, the answers files named by the ledger with their digests, the requests
//! already issued, the budgets): a resumed campaign recomputes an interrupted pass and takes the same decisions (U-1).
//! It runs on a source-owned thread; nothing here touches the ledger.

use super::config::{Budgets, HandCfg};
use rustred_campaign_core::closure::relations::DispatchSlot;
use rustred_campaign_core::closure::{Tier, Verdict};
use rustred_campaign_core::import::load::RelEntry;
use rustred_campaign_core::import::spec::FileRef as ImportFileRef;
use rustred_campaign_core::import::{
    CheckOptions, CheckResult, Evidence, HandFormat, ImportedSet, Level, RowStatus, Server,
    TableSetSpec, check, compare, load, read_hand_statement, write_statement,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const CHECK_SCHEMA: &str = "rustred.campaign.closeloop-check.v1";
pub const RESULT_SCHEMA: &str = "rustred.campaign.closeloop-result.v1";
pub const DISPATCH_SCHEMA: &str = "rustred.campaign.closeloop-dispatch.v1";
pub const IMPORT_SCHEMA: &str = "rustred.campaign.closeloop-import.v1";

/// Input of one pass.
#[derive(Clone, Debug)]
pub struct PassIn {
    pub node: String,
    pub round: u32,
    /// The effective import spec (JSON text; its digest is the node's `import_spec` input).
    pub spec_text: String,
    pub base: PathBuf,
    /// Answers files of the node (absolute path, recorded BLAKE3), in ledger order.
    pub answers: Vec<(PathBuf, String)>,
    pub threads: usize,
    pub required: Option<Level>,
    /// (recipe source, key) pairs already given to a solve item (any phase).
    pub issued: BTreeSet<(String, Vec<i32>)>,
    pub keys_issued: u64,
    pub jobs_issued: u64,
    pub budgets: Budgets,
    pub max_keys_per_job: usize,
    pub hand: Option<HandCfg>,
    /// Absolute directory of the phase-D outputs.
    pub final_dir: PathBuf,
    /// Solve items in attention (their keys stay open).
    pub attention_items: u64,
    /// The relation rule in force for the solves (label, closeloop.md 5).
    pub relation_rule: String,
}

/// One new solve batch (one recipe, <= K keys).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Batch {
    pub source: String,
    pub recipe_path: String,
    pub recipe_sha256: String,
    pub engine_digest: Option<String>,
    pub keys: Vec<Vec<i32>>,
}

/// The phase-D result of a node.
#[derive(Clone, Debug)]
pub struct FinalOut {
    /// `closed` | `open` | `budget`.
    pub verdict: String,
    pub result: Value,
}

/// Output of one pass.
pub struct PassOut {
    pub round: u32,
    pub set: Arc<ImportedSet>,
    pub check: Value,
    pub defect: u64,
    pub closure_keys: u64,
    pub rules_used: u64,
    pub rules_total: u64,
    pub batches: Vec<Batch>,
    pub final_: Option<FinalOut>,
    pub dispatch_order: Value,
    pub import_manifest: Value,
    pub spec_digest: String,
    pub seconds: f64,
}

impl std::fmt::Debug for PassOut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PassOut")
            .field("round", &self.round)
            .field("defect", &self.defect)
            .field("batches", &self.batches.len())
            .field("final", &self.final_.as_ref().map(|x| x.verdict.clone()))
            .finish()
    }
}

/// The tables of `set` placed before a relation-set slot, as their own dispatcher (the dispatch check of
/// `accept_recipe`); `None` when no table precedes the slot.
pub fn prefix_dispatcher(
    set: &ImportedSet,
    slot: &DispatchSlot,
) -> Result<Option<rustred_campaign_core::cover::Dispatcher>, String> {
    let tables = set.disp.tables();
    let mut picked = vec![];
    for &t in set.disp.priority() {
        if let DispatchSlot::BeforeTable(name) = slot
            && tables[t].name == *name
        {
            break;
        }
        picked.push(tables[t].clone());
    }
    if picked.is_empty() {
        return Ok(None);
    }
    let n = picked.len();
    let mut d = rustred_campaign_core::cover::Dispatcher::new(picked, (0..n).collect())
        .map_err(|e| format!("prefix dispatcher: {e}"))?;
    if let Some(c) = set.disp.chart() {
        d = d
            .with_chart(c.to_vec())
            .map_err(|e| format!("prefix dispatcher chart: {e}"))?;
    }
    Ok(Some(d))
}

/// Whether the dispatcher is strict, and why not (closure.md 15.2; the coordinator's decision of 2026-11-14:
/// hand-built `Assurance::Caller` records refuse too).
pub fn strictness(set: &ImportedSet) -> (bool, Vec<String>) {
    let s = set.disp.summary();
    let mut why = vec![];
    if !s.chart_checked {
        why.push("chart_not_checked".to_string());
    }
    for t in &s.tables {
        let mut parts = vec![];
        if t.witness_check_off > 0 {
            parts.push(format!("witness_check_off:{}", t.witness_check_off));
        }
        if t.witness_attested_unchecked > 0 {
            parts.push(format!(
                "witness_attested_unchecked:{}",
                t.witness_attested_unchecked
            ));
        }
        if t.target_attested_unchecked > 0 {
            parts.push(format!(
                "target_attested_unchecked:{}",
                t.target_attested_unchecked
            ));
        }
        if t.caller > 0 {
            parts.push(format!("caller_built:{}", t.caller));
        }
        if !parts.is_empty() {
            why.push(format!("{}:{}", t.name, parts.join(",")));
        }
    }
    (s.is_strict(), why)
}

fn rel_name(set: &ImportedSet, e: &RelEntry) -> String {
    match *e {
        RelEntry::Supplied(i) => set.supplied[i].name.clone(),
        RelEntry::Recipe(i) => set.recipes[i].id.clone(),
    }
}

/// Run one pass. `cached` = the node's set loaded by an earlier pass (the load dominates small passes).
pub fn run_pass(inp: &PassIn, cached: Option<Arc<ImportedSet>>) -> Result<PassOut, String> {
    let t0 = std::time::Instant::now();
    let spec = TableSetSpec::from_json(&inp.spec_text).map_err(|e| format!("import spec: {e}"))?;
    let set = match cached {
        Some(s) => s,
        None => Arc::new(load(&spec, &inp.base).map_err(|e| format!("import load: {e}"))?),
    };
    let mut ev = Evidence::from_spec(&set, &inp.base).map_err(|e| format!("evidence: {e}"))?;
    for (p, b3) in &inp.answers {
        let (_, d) = crate::atomic::blake3_file(p)
            .map_err(|e| format!("answers file {}: {e}", p.display()))?;
        if d != *b3 {
            return Err(format!(
                "answers file {} changed since it was recorded (blake3 {d}, ledger {b3}): refused",
                p.display()
            ));
        }
        ev.load_solved(
            &set,
            Path::new("/"),
            &ImportFileRef {
                path: p.display().to_string(),
                sha256: None,
            },
        )
        .map_err(|e| format!("answers {}: {e}", p.display()))?;
    }
    let res = check(
        &set,
        &ev,
        &CheckOptions {
            threads: inp.threads.max(1),
            tier: None,
            max_rounds: None,
        },
    )
    .map_err(|e| format!("check: {e}"))?;
    let required = inp.required.unwrap_or(res.required);
    let (strict, strict_why) = strictness(&set);
    let diagnostic = res.summary.diagnostic || res.summary.verdict == Verdict::Diagnostic;
    let tier_c = res.tier == Tier::C;
    // CERTIFIED (closeloop.md 8): a declared terminal, or served at >= the required level, on a strict basis, with the
    // closure's facts decided at the key (Tier C).
    let mut certified = 0u64;
    let mut served = 0u64;
    let mut terminal = 0u64;
    let mut open = 0u64;
    for r in &res.rows {
        match r.status {
            RowStatus::Terminal => {
                terminal += 1;
                certified += 1;
            }
            RowStatus::Served => {
                served += 1;
                if r.evidence.is_some_and(|e| e >= required)
                    && tier_c
                    && strict
                    && !diagnostic
                    && !r.regularity_unknown
                    && !r.coefficient_unknown
                {
                    certified += 1;
                }
            }
            RowStatus::Open => open += 1,
        }
    }
    let keys = res.rows.len() as u64;
    let evidence_by_class = by_class(&set, &res, required, tier_c && strict && !diagnostic);
    let mut blocking = res.blocking();
    if required != res.required {
        // the import counted against its own level: recount against the override
        blocking.retain(|b| !b.starts_with("below_required_evidence:"));
        let below = res
            .rows
            .iter()
            .filter(|r| r.status == RowStatus::Served && r.evidence.is_none_or(|e| e < required))
            .count();
        if below > 0 {
            blocking.push(format!("below_required_evidence:{below}"));
        }
    }
    if !strict {
        blocking.push(format!("dispatch_not_strict:{}", strict_why.join(";")));
    }
    if diagnostic {
        blocking.push("closure_verdict_diagnostic".into());
    }
    let fact_keys: usize = res.fact_requests.iter().map(|r| r.keys.len()).sum();
    if !res.fact_requests.is_empty() {
        blocking.push(format!(
            "fact_requests_unserved:{}/{}",
            res.fact_requests.len(),
            fact_keys
        ));
    }
    let proof_keys: usize = res.proof_requests.iter().map(|r| r.keys.len()).sum();
    if !res.proof_requests.is_empty() {
        blocking.push(format!(
            "proof_requests_unserved:{}/{}",
            res.proof_requests.len(),
            proof_keys
        ));
    }
    if inp.attention_items > 0 {
        blocking.push(format!("solve_items_in_attention:{}", inp.attention_items));
    }
    let closed = certified == keys && blocking.is_empty();
    // a verdict blocker without a per-key count counts as one defect (the node machine needs defect > 0 to end)
    let defect = if closed {
        0
    } else {
        (keys - certified).max(1)
    };
    // new requests: those of no item yet (termination: finitely many keys x recipes; descent, R49)
    let new_reqs: Vec<_> = res
        .solve_requests
        .iter()
        .filter(|r| !inp.issued.contains(&(r.source.clone(), r.key.clone())))
        .collect();
    let mut batches: Vec<Batch> = vec![];
    if !closed {
        let mut by_source: BTreeMap<&str, Vec<&rustred_campaign_core::import::SolveRequest>> =
            BTreeMap::new();
        for r in &new_reqs {
            by_source.entry(r.source.as_str()).or_default().push(r);
        }
        // recipe sources in DISPATCH order (spec order), keys sorted (import order)
        let order: Vec<String> = set.rel_order.iter().map(|e| rel_name(&set, e)).collect();
        let mut srcs: Vec<&str> = by_source.keys().copied().collect();
        srcs.sort_by_key(|s| order.iter().position(|o| o == s).unwrap_or(usize::MAX));
        for s in srcs {
            let v = &by_source[s];
            for chunk in v.chunks(inp.max_keys_per_job.max(1)) {
                batches.push(Batch {
                    source: s.to_string(),
                    recipe_path: chunk[0].recipe_path.clone(),
                    recipe_sha256: chunk[0].recipe_sha256.clone(),
                    engine_digest: chunk[0].engine_digest.clone(),
                    keys: chunk.iter().map(|r| r.key.clone()).collect(),
                });
            }
        }
    }
    let new_keys: u64 = batches.iter().map(|b| b.keys.len() as u64).sum();
    let mut budget_why: Option<String> = None;
    if !batches.is_empty() {
        if inp.round >= inp.budgets.max_rounds {
            budget_why = Some(format!("max_rounds {} reached", inp.budgets.max_rounds));
        } else if inp.keys_issued + new_keys > inp.budgets.max_solve_keys {
            budget_why = Some(format!(
                "max_solve_keys {}: {} issued + {new_keys} new",
                inp.budgets.max_solve_keys, inp.keys_issued
            ));
        } else if inp.jobs_issued + batches.len() as u64 > inp.budgets.max_solve_jobs {
            budget_why = Some(format!(
                "max_solve_jobs {}: {} issued + {} new",
                inp.budgets.max_solve_jobs,
                inp.jobs_issued,
                batches.len()
            ));
        }
    }
    if budget_why.is_some() {
        batches.clear();
    }
    // the check record
    let ds = set.disp.summary();
    let rules_total: u64 = ds.tables.iter().map(|t| t.records as u64).sum();
    let counts = res.counts();
    let solve_by_source: BTreeMap<String, usize> =
        res.solve_requests
            .iter()
            .fold(BTreeMap::new(), |mut m, r| {
                *m.entry(r.source.clone()).or_default() += 1;
                m
            });
    let check_v = json!({
        "schema": CHECK_SCHEMA, "node": inp.node, "round": inp.round,
        "spec_digest": res.spec_digest, "key_set_digest": res.key_set_digest,
        "tier": format!("{:?}", res.tier), "required_evidence": required.word(),
        "keys": keys, "in_start": res.summary.keys_in_start, "served": served, "terminal": terminal, "open": open,
        "certified": certified, "defect": defect, "closed": closed,
        "closure": {"verdict": format!("{:?}", res.summary.verdict), "rounds": res.summary.rounds,
                    "edges": res.summary.edges, "used_rules": res.summary.used_rules,
                    "descent_violations": res.summary.descent_violations, "blocking": res.summary.blocking,
                    "diagnostic": diagnostic, "relaxed_checks": res.summary.relaxed_checks},
        "strict": strict, "strict_why": strict_why, "blocking": blocking, "counts": counts,
        "evidence_by_class": evidence_by_class, "relation_rule": inp.relation_rule,
        "requests": {"solve": res.solve_requests.len(), "solve_new": new_reqs.len(),
                     "solve_by_source": solve_by_source, "fact_records": res.fact_requests.len(),
                     "fact_keys": fact_keys, "proof_records": res.proof_requests.len(), "proof_keys": proof_keys},
        "solve_refusals": res.solve_refusals.len(),
        "batches": batches.iter().map(|b| json!({"source": b.source, "keys": b.keys.len()})).collect::<Vec<_>>(),
        "budget_stop": budget_why,
    });
    let dispatch_order = json!({
        "schema": DISPATCH_SCHEMA, "node": inp.node, "spec_digest": set.report.spec_digest,
        "tables": set.disp.priority().iter().map(|&t| set.disp.tables()[t].name.clone()).collect::<Vec<_>>(),
        "relation_sets": set.rel_order.iter().map(|e| {
            let slot = match e {
                RelEntry::Recipe(i) => format!("{:?}", set.recipes[*i].slot),
                RelEntry::Supplied(i) => format!("{:?}", set.supplied[*i].slot),
            };
            json!({"name": rel_name(&set, e), "slot": slot})
        }).collect::<Vec<_>>(),
    });
    let import_manifest = json!({
        "schema": IMPORT_SCHEMA, "node": inp.node, "report": serde_json::to_value(set.report()).unwrap_or(Value::Null),
    });
    let final_ = if closed || budget_why.is_some() || batches.is_empty() {
        let verdict = if closed {
            "closed"
        } else if budget_why.is_some() {
            "budget"
        } else {
            "open"
        };
        Some(write_final(
            inp,
            &set,
            &ev,
            &res,
            FinalCtx {
                verdict,
                blocking: &blocking,
                budget_why: budget_why.as_deref(),
                check: &check_v,
                elapsed: t0.elapsed().as_secs_f64(),
            },
        )?)
    } else {
        None
    };
    Ok(PassOut {
        round: inp.round,
        spec_digest: set.report.spec_digest.clone(),
        set,
        check: check_v,
        defect,
        closure_keys: keys,
        rules_used: res.summary.used_rules,
        rules_total: rules_total.max(res.summary.used_rules),
        batches,
        final_,
        dispatch_order,
        import_manifest,
        seconds: t0.elapsed().as_secs_f64(),
    })
}

/// The rule class of a row's server (the import spec's `class` of its source; supplied / recipe sets by their id).
fn class_of(set: &ImportedSet, r: &rustred_campaign_core::import::KeyRow) -> String {
    match &r.server {
        Some(Server::Rule { table, record }) | Some(Server::Credit { table, record }) => {
            set.rules[*table][*record].class.clone()
        }
        Some(Server::Relation { set: name, .. }) => set
            .classes
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.clone()),
        None => match r.open_rule {
            Some((t, k)) => set.rules[t][k].class.clone(),
            None => "-".into(),
        },
    }
}

/// Evidence per rule class (the directing session's ruling: the level must be VISIBLE per class with counts):
/// class -> {"served:<level>:<basis>", "open:<status>", "terminal", "certified"} -> keys.
fn by_class(
    set: &ImportedSet,
    res: &CheckResult,
    required: Level,
    basis_ok: bool,
) -> BTreeMap<String, BTreeMap<String, u64>> {
    let mut m: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    for r in &res.rows {
        let e = m.entry(class_of(set, r)).or_default();
        let k = match r.status {
            RowStatus::Served => format!(
                "served:{}:{}",
                r.evidence.map(|x| x.word()).unwrap_or("none"),
                r.basis.unwrap_or("-")
            ),
            RowStatus::Terminal => "terminal".into(),
            RowStatus::Open => format!("open:{}", r.open_status.as_deref().unwrap_or("open")),
        };
        *e.entry(k).or_default() += 1;
        let cert = match r.status {
            RowStatus::Terminal => true,
            RowStatus::Served => {
                basis_ok
                    && r.evidence.is_some_and(|x| x >= required)
                    && !r.regularity_unknown
                    && !r.coefficient_unknown
            }
            RowStatus::Open => false,
        };
        if cert {
            *e.entry("certified".into()).or_default() += 1;
        }
    }
    m
}

struct FinalCtx<'a> {
    verdict: &'a str,
    blocking: &'a [String],
    budget_why: Option<&'a str>,
    check: &'a Value,
    elapsed: f64,
}

/// Phase D: our statement, the comparison with the hand statement, the economy numbers, the result record.
fn write_final(
    inp: &PassIn,
    set: &ImportedSet,
    ev: &Evidence,
    res: &CheckResult,
    f: FinalCtx<'_>,
) -> Result<FinalOut, String> {
    let sdir = inp.final_dir.join("statement");
    let dig = write_statement(
        set,
        ev,
        res,
        &sdir,
        Some(json!({"check_pass_s": f.elapsed})),
    )
    .map_err(|e| format!("statement: {e}"))?;
    let comparison = match &inp.hand {
        None => Value::Null,
        Some(h) => {
            let fmt: HandFormat = serde_json::from_value(h.format.clone())
                .map_err(|e| format!("hand format: {e}"))?;
            let hand = read_hand_statement(&h.path, &fmt, set.arity())
                .map_err(|e| format!("hand statement: {e}"))?;
            let c = compare(set, res, &hand);
            c.write(&inp.final_dir.join("comparison"))
                .map_err(|e| format!("comparison: {e}"))?;
            let mut v = serde_json::to_value(&c).map_err(|e| e.to_string())?;
            if let Some(o) = v.as_object_mut() {
                o.remove("differences");
                o.insert("differences_listed".into(), json!(c.differences.len()));
            }
            v
        }
    };
    // economy (R48 addendum 4): closure size, used rules, recipes used and their solved keys
    let mut served_by_recipe: BTreeMap<String, u64> = BTreeMap::new();
    let mut served_by_supplied: BTreeMap<String, u64> = BTreeMap::new();
    for r in &res.rows {
        if let (RowStatus::Served, Some(Server::Relation { set: s, recipe })) = (r.status, &r.server) {
            if *recipe {
                *served_by_recipe.entry(s.clone()).or_default() += 1;
            } else {
                *served_by_supplied.entry(s.clone()).or_default() += 1;
            }
        }
    }
    let mut answered: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    for (src, m) in &ev.solved {
        for r in m.values() {
            *answered
                .entry(src.clone())
                .or_default()
                .entry(r.verdict.clone())
                .or_default() += 1;
        }
    }
    let solved_keys: u64 = answered
        .values()
        .map(|m| m.get("SOLVED").copied().unwrap_or(0))
        .sum();
    let failed_keys: u64 = answered
        .values()
        .map(|m| m.iter().filter(|(v, _)| *v != "SOLVED").map(|(_, n)| n).sum::<u64>())
        .sum();
    let result = json!({
        "schema": RESULT_SCHEMA, "node": inp.node, "verdict": f.verdict, "rounds": inp.round,
        "blockers": f.blocking, "budget_stop": f.budget_why,
        "check": f.check,
        "statement": {"dir": sdir.display().to_string(), "files": dig.files},
        "comparison": comparison,
        "economy": {"closure_keys": res.rows.len(), "used_rules": res.summary.used_rules,
                    "recipes_used": served_by_recipe.len(), "keys_by_recipe": served_by_recipe,
                    "supplied_sets_used": served_by_supplied, "solved_keys": solved_keys, "failed_keys": failed_keys,
                    "answers_by_recipe": answered},
        "evidence_by_class": f.check["evidence_by_class"].clone(),
        "relation_rule": inp.relation_rule,
        "requests_left": {"solve": res.solve_requests.len(), "fact_records": res.fact_requests.len(),
                          "proof_records": res.proof_requests.len()},
        "writes": "CDIR/closeloop only (never P5L_STATUS.json / P5L_TERMINALS.json)",
    });
    Ok(FinalOut {
        verdict: f.verdict.to_string(),
        result,
    })
}
