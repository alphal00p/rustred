//! Phase A / the re-check of phase C / phase D: the in-process pass (interfaces/closeloop.md 1, 7, 8, 13). A PURE
//! function of its input (the effective import spec, the answers / proofs / facts / relation files named by the ledger
//! with their digests, the requests already issued, the budgets): a resumed campaign recomputes an interrupted pass and
//! takes the same decisions (U-1). It runs on a source-owned thread; nothing here touches the ledger.
//!
//! lane closeloop2: the EXACT TIER (13.2). A pass first checks at Tier K (keys-only, every row in memory, cheap); the
//! facts of every served rule key are asked from fact jobs; once every such (record, key) was asked, the pass checks
//! again at Tier C with the rows SPILLED to a file (`check_streamed`) and decides on that. Per-key proofs are asked
//! for the import's proof requests and for the point records of our own relations (13.1); a relation is per-key only
//! with its bound proof (import hook `Evidence::recipe_proofs`).

use super::config::{Budgets, HandCfg};
use super::proofs::{
    ProofBatch, REASON_FACTS, REASON_IMPORT_PROOF, REASON_RELATION, RecordReq, relation_records,
};
use rustred_campaign_core::closure::relations::DispatchSlot;
use rustred_campaign_core::closure::{Tier, Verdict};
use rustred_campaign_core::import::load::RelEntry;
use rustred_campaign_core::import::spec::FileRef as ImportFileRef;
use rustred_campaign_core::import::{
    CheckOptions, CheckResult, Evidence, HandFormat, ImportedSet, KeyRow, Level, RowStatus,
    Server, TableSetSpec, check, compare, load, read_hand_statement, write_statement,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const CHECK_SCHEMA: &str = "rustred.campaign.closeloop-check.v1";
pub const RESULT_SCHEMA: &str = "rustred.campaign.closeloop-result.v1";
pub const DISPATCH_SCHEMA: &str = "rustred.campaign.closeloop-dispatch.v1";
pub const IMPORT_SCHEMA: &str = "rustred.campaign.closeloop-import.v1";

/// What proofs a pass may ask (from the node's `proof` config; None = no proof job).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofPlan {
    pub import_requests: bool,
    pub relations: bool,
    pub facts: bool,
    pub max_keys_per_job: usize,
    pub max_fact_keys_per_job: usize,
}

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
    // ---- lane closeloop2 ----
    /// Proof evidence files of the node's settled proof jobs (compact proof verdict lines), recorded BLAKE3.
    pub proof_files: Vec<(PathBuf, String)>,
    /// Fact evidence files of the node's settled fact jobs (compact fact verdict lines), recorded BLAKE3.
    pub fact_files: Vec<(PathBuf, String)>,
    /// Point-record tables of our relations: (path, recorded BLAKE3, recipe source).
    pub relation_tables: Vec<(PathBuf, String, String)>,
    /// (record_sha256, key) pairs already given to a proof (span) item / a fact item.
    pub issued_proofs: BTreeSet<(String, Vec<i32>)>,
    pub issued_facts: BTreeSet<(String, Vec<i32>)>,
    pub proof_jobs_issued: u64,
    /// Proof / fact items in attention.
    pub proof_attention: u64,
    pub proof: Option<ProofPlan>,
    pub exact_tier: bool,
    /// The spill file of this round's exact-tier rows.
    pub rows_path: PathBuf,
    /// `boxed` | `full` (closeloop.md 13.3: boxed from the first record).
    pub target: String,
}

impl PassIn {
    /// A first-round pass with nothing issued, no proof job and no exact-tier step (tests and measurements set what
    /// differs with struct-update syntax).
    pub fn first(node: &str, spec_text: String, base: PathBuf, final_dir: PathBuf) -> PassIn {
        PassIn {
            node: node.into(),
            round: 1,
            spec_text,
            base,
            answers: vec![],
            threads: 4,
            required: None,
            issued: BTreeSet::new(),
            keys_issued: 0,
            jobs_issued: 0,
            budgets: Budgets::default(),
            max_keys_per_job: 16,
            hand: None,
            rows_path: final_dir.join("rows.jsonl"),
            final_dir,
            attention_items: 0,
            relation_rule: "m1_first".into(),
            proof_files: vec![],
            fact_files: vec![],
            relation_tables: vec![],
            issued_proofs: BTreeSet::new(),
            issued_facts: BTreeSet::new(),
            proof_jobs_issued: 0,
            proof_attention: 0,
            proof: None,
            exact_tier: false,
            target: "boxed".into(),
        }
    }
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
    /// lane closeloop2: new proof (span) and fact batches.
    pub proof_batches: Vec<ProofBatch>,
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
            .field("proof_batches", &self.proof_batches.len())
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

fn verify_b3(p: &Path, b3: &str) -> Result<(), String> {
    let (_, d) =
        crate::atomic::blake3_file(p).map_err(|e| format!("file {}: {e}", p.display()))?;
    if d != b3 {
        return Err(format!(
            "file {} changed since it was recorded (blake3 {d}, ledger {b3}): refused",
            p.display()
        ));
    }
    Ok(())
}

fn fref(p: &Path) -> ImportFileRef {
    ImportFileRef {
        path: p.display().to_string(),
        sha256: None,
    }
}

/// One relation point record: (table, rule index, offset, record_sha256, recipe source, key).
#[derive(Clone, Debug)]
struct RelRec {
    table: String,
    rule: i64,
    offset: u64,
    sha: String,
    source: String,
    key: Vec<i32>,
}

/// Whether a row is CERTIFIED (closeloop.md 8 / 13.2): a declared terminal, or served at >= the required level with
/// the facts decided at the key, on a basis that allows it (Tier C, strict, not diagnostic).
fn row_certified(r: &KeyRow, required: Level, basis_ok: bool) -> bool {
    match r.status {
        RowStatus::Terminal => true,
        RowStatus::Served => {
            basis_ok
                && r.evidence.is_some_and(|e| e >= required)
                && !r.regularity_unknown
                && !r.coefficient_unknown
        }
        RowStatus::Open => false,
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
        verify_b3(p, b3)?;
        ev.load_solved(&set, Path::new("/"), &fref(p))
            .map_err(|e| format!("answers {}: {e}", p.display()))?;
    }
    for (p, b3) in &inp.proof_files {
        verify_b3(p, b3)?;
        ev.load_proofs(&set, Path::new("/"), &fref(p))
            .map_err(|e| format!("proofs {}: {e}", p.display()))?;
    }
    for (p, b3) in &inp.fact_files {
        verify_b3(p, b3)?;
        ev.load_facts(&set, Path::new("/"), &fref(p))
            .map_err(|e| format!("facts {}: {e}", p.display()))?;
    }
    // our relations: their point records and the per-key proofs bound to (record_sha256, key)
    let mut rels: Vec<RelRec> = vec![];
    for (p, b3, source) in &inp.relation_tables {
        verify_b3(p, b3)?;
        let text = std::fs::read(p).map_err(|e| format!("relations {}: {e}", p.display()))?;
        for (rule, offset, sha, key) in relation_records(&text) {
            rels.push(RelRec {
                table: p.display().to_string(),
                rule,
                offset,
                sha,
                source: source.clone(),
                key,
            });
        }
    }
    let rel_by_sha: BTreeMap<&str, &RelRec> = rels.iter().map(|r| (r.sha.as_str(), r)).collect();
    let mut proved: HashSet<(String, Vec<i32>)> = HashSet::new();
    let mut relation_proof_status: BTreeMap<String, u64> = BTreeMap::new();
    for (p, _) in &inp.proof_files {
        for l in std::fs::read_to_string(p).unwrap_or_default().lines() {
            let Ok(v) = serde_json::from_str::<Value>(l) else {
                continue;
            };
            let (Some(sha), Some(key)) = (
                v["record_sha256"].as_str(),
                serde_json::from_value::<Vec<i32>>(v["key"].clone()).ok(),
            ) else {
                continue;
            };
            if let Some(r) = rel_by_sha.get(sha)
                && r.key == key
            {
                let w = v["verdict"].as_str().unwrap_or("").to_string();
                *relation_proof_status.entry(w.clone()).or_default() += 1;
                if w == "EXACT_IN_SPAN_AT_KEY" {
                    proved.insert((r.source.clone(), key));
                }
            }
        }
    }
    // a relation of our own solve is per-key ONLY with its bound proof (closeloop.md 13.1)
    ev.recipe_proofs = Some(proved.iter().cloned().collect());
    let threads = inp.threads.max(1);
    // pass 1: keys-only when the exact tier is wanted (cheap; every row kept: the fact requests come from it)
    let res_k = check(
        &set,
        &ev,
        &CheckOptions {
            threads,
            tier: if inp.exact_tier { Some(Tier::K) } else { None },
            max_rounds: None,
        },
    )
    .map_err(|e| format!("check: {e}"))?;
    // the facts the exact tier needs: every served rule key's (record, key) not asked yet
    let mut missing_facts: BTreeMap<(usize, usize), BTreeSet<Vec<i32>>> = BTreeMap::new();
    if inp.exact_tier {
        for r in &res_k.rows {
            if let (RowStatus::Served, Some(Server::Rule { table, record })) = (r.status, &r.server)
            {
                let sha = &set.rules[*table][*record].record_sha256;
                // a usable fact already bound (imported evidence or an earlier fact job) is never asked again
                if !ev.has_lift(&set, *table, *record, &r.key)
                    && !inp.issued_facts.contains(&(sha.clone(), r.key.clone()))
                {
                    missing_facts
                        .entry((*table, *record))
                        .or_default()
                        .insert(r.key.clone());
                }
            }
        }
    }
    let n_missing: usize = missing_facts.values().map(BTreeSet::len).sum();
    let can_ask_facts = inp.proof.as_ref().is_some_and(|p| p.facts);
    let tier_c_now = inp.exact_tier && n_missing == 0;
    let streamed_res;
    let res: &CheckResult = if tier_c_now {
        streamed_res = rustred_campaign_core::import::check::check_streamed(
            &set,
            &ev,
            &CheckOptions {
                threads,
                tier: Some(Tier::C),
                max_rounds: None,
            },
            &inp.rows_path,
        )
        .map_err(|e| format!("exact-tier check: {e}"))?;
        &streamed_res
    } else {
        &res_k
    };
    let required = inp.required.unwrap_or(res.required);
    let (strict, strict_why) = strictness(&set);
    let diagnostic = res.summary.diagnostic || res.summary.verdict == Verdict::Diagnostic;
    let tier_c = res.tier == Tier::C;
    let basis_ok = tier_c && strict && !diagnostic;
    // CERTIFIED keys: plain streamed rows are served by a rule at >= the required level with the facts decided (the
    // import keeps every other row); kept rows are judged one by one
    let counts = res.counts();
    let kept = res.rows.len() as u64;
    let keys = res.streamed.as_ref().map(|s| s.rows).unwrap_or(kept);
    let plain = keys - kept;
    let kept_cert = res
        .rows
        .iter()
        .filter(|r| row_certified(r, required, basis_ok))
        .count() as u64;
    // the import judged plain rows against ITS required level: they also meet a lower override; a higher override
    // cannot be judged from the counts (conservative: not certified)
    let plain_cert = if basis_ok && required <= res.required {
        plain
    } else {
        0
    };
    let certified = kept_cert + plain_cert;
    let served = counts.get("status:served").copied().unwrap_or(0);
    let terminal = counts.get("status:terminal").copied().unwrap_or(0);
    let open = counts.get("status:open").copied().unwrap_or(0);
    let evidence_by_class = res.evidence_by_class(&set);
    let by_rule_class_level = class_level(&evidence_by_class);
    let mut blocking = res.blocking();
    if required != res.required {
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
    if inp.exact_tier && !tier_c_now {
        blocking.push(format!(
            "exact_tier_not_reached: facts of {n_missing} served rule keys not asked{}",
            if can_ask_facts {
                ""
            } else {
                " (no fact jobs configured)"
            }
        ));
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
    let unproved_rel = rels
        .iter()
        .filter(|r| !proved.contains(&(r.source.clone(), r.key.clone())))
        .count();
    if unproved_rel > 0 {
        blocking.push(format!("relation_proofs_unbound:{unproved_rel}"));
    }
    if inp.attention_items > 0 {
        blocking.push(format!("solve_items_in_attention:{}", inp.attention_items));
    }
    if inp.proof_attention > 0 {
        blocking.push(format!("proof_items_in_attention:{}", inp.proof_attention));
    }
    let closed = certified == keys && blocking.is_empty();
    // a verdict blocker without a per-key count counts as one defect (the node machine needs defect > 0 to end)
    let defect = if closed { 0 } else { (keys - certified).max(1) };
    // new solve requests: those of no item yet (termination: finitely many keys x recipes; descent, R49)
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
    // new proof / fact batches (closeloop.md 13.1)
    let mut proof_batches: Vec<ProofBatch> = vec![];
    if let (Some(pp), false) = (&inp.proof, closed) {
        if pp.import_requests {
            for r in &res.proof_requests {
                let keys: Vec<Vec<i32>> = r
                    .keys
                    .iter()
                    .filter(|k| !inp.issued_proofs.contains(&(r.record_sha256.clone(), (*k).clone())))
                    .cloned()
                    .collect();
                for chunk in keys.chunks(pp.max_keys_per_job.max(1)) {
                    proof_batches.push(ProofBatch {
                        kind: "span".into(),
                        reason: REASON_IMPORT_PROOF.into(),
                        table: r.table.clone(),
                        records: vec![RecordReq {
                            rule: r.rule,
                            offset: r.offset,
                            record_sha256: r.record_sha256.clone(),
                            keys: chunk.to_vec(),
                        }],
                        relation_source: None,
                    });
                }
            }
        }
        if pp.relations {
            for r in &rels {
                let id = (r.source.clone(), r.key.clone());
                if proved.contains(&id) || inp.issued_proofs.contains(&(r.sha.clone(), r.key.clone()))
                {
                    continue;
                }
                proof_batches.push(ProofBatch {
                    kind: "span".into(),
                    reason: REASON_RELATION.into(),
                    table: r.table.clone(),
                    records: vec![RecordReq {
                        rule: r.rule,
                        offset: r.offset,
                        record_sha256: r.sha.clone(),
                        keys: vec![r.key.clone()],
                    }],
                    relation_source: Some(r.source.clone()),
                });
            }
        }
        if pp.facts && inp.exact_tier {
            // the fact requests of the exact tier itself (Tier C) and the served rule keys of the keys-only pass
            for fr in &res.fact_requests {
                if let Some((t, r)) = set.rules.iter().enumerate().find_map(|(t, v)| {
                    v.iter()
                        .position(|x| x.record_sha256 == fr.record_sha256)
                        .map(|r| (t, r))
                }) {
                    for k in &fr.keys {
                        if !inp.issued_facts.contains(&(fr.record_sha256.clone(), k.clone())) {
                            missing_facts.entry((t, r)).or_default().insert(k.clone());
                        }
                    }
                }
            }
            let mut by_table: BTreeMap<String, Vec<RecordReq>> = BTreeMap::new();
            for ((t, r), ks) in &missing_facts {
                let info = &set.rules[*t][*r];
                by_table.entry(info.path.clone()).or_default().push(RecordReq {
                    rule: info.index.unwrap_or(info.label),
                    offset: info.offset,
                    record_sha256: info.record_sha256.clone(),
                    keys: ks.iter().cloned().collect(),
                });
            }
            let cap = pp.max_fact_keys_per_job.max(1);
            for (table, recs) in by_table {
                let mut cur: Vec<RecordReq> = vec![];
                let mut n = 0usize;
                for rr in recs {
                    for chunk in rr.keys.chunks(cap) {
                        if n + chunk.len() > cap && !cur.is_empty() {
                            proof_batches.push(ProofBatch {
                                kind: "facts".into(),
                                reason: REASON_FACTS.into(),
                                table: table.clone(),
                                records: std::mem::take(&mut cur),
                                relation_source: None,
                            });
                            n = 0;
                        }
                        n += chunk.len();
                        cur.push(RecordReq {
                            keys: chunk.to_vec(),
                            ..rr.clone()
                        });
                    }
                }
                if !cur.is_empty() {
                    proof_batches.push(ProofBatch {
                        kind: "facts".into(),
                        reason: REASON_FACTS.into(),
                        table,
                        records: cur,
                        relation_source: None,
                    });
                }
            }
        }
    }
    // work-counter budgets: issue batches up to what is left (a batch may be cut); a budget stop is recorded when
    // requests remain and nothing more may be issued
    let mut budget_why: Option<String> = None;
    let wanted: u64 = batches.iter().map(|b| b.keys.len() as u64).sum();
    if !batches.is_empty() {
        let keys_left = inp.budgets.max_solve_keys.saturating_sub(inp.keys_issued);
        let jobs_left = inp.budgets.max_solve_jobs.saturating_sub(inp.jobs_issued);
        if inp.round > inp.budgets.max_rounds {
            budget_why = Some(format!("max_rounds {} reached", inp.budgets.max_rounds));
            batches.clear();
        } else {
            let mut kept_b = vec![];
            let mut k = 0u64;
            for mut b in std::mem::take(&mut batches) {
                if kept_b.len() as u64 >= jobs_left || k >= keys_left {
                    break;
                }
                let room = (keys_left - k) as usize;
                b.keys.truncate(room);
                k += b.keys.len() as u64;
                kept_b.push(b);
            }
            if k < wanted {
                budget_why = Some(format!(
                    "budget: {k} of {wanted} wanted keys issued (max_solve_keys {}, {} issued before; max_solve_jobs {}, {} issued before)",
                    inp.budgets.max_solve_keys,
                    inp.keys_issued,
                    inp.budgets.max_solve_jobs,
                    inp.jobs_issued
                ));
            }
            batches = kept_b;
        }
    }
    let proof_wanted = proof_batches.len() as u64;
    if proof_wanted > 0 {
        let left = inp
            .budgets
            .max_proof_jobs
            .saturating_sub(inp.proof_jobs_issued);
        if inp.round > inp.budgets.max_rounds {
            proof_batches.clear();
            budget_why.get_or_insert(format!("max_rounds {} reached", inp.budgets.max_rounds));
        } else if proof_wanted > left {
            proof_batches.truncate(left as usize);
            budget_why.get_or_insert(format!(
                "budget: {left} of {proof_wanted} wanted proof jobs issued (max_proof_jobs {}, {} issued before)",
                inp.budgets.max_proof_jobs, inp.proof_jobs_issued
            ));
        }
    }
    // the budget stops the loop only when nothing could be issued
    let budget_stop = batches.is_empty() && proof_batches.is_empty() && budget_why.is_some();
    if budget_stop {
        blocking.push(format!(
            "budget_stop: {}",
            budget_why.as_deref().unwrap_or("")
        ));
    }
    // the check record
    let ds = set.disp.summary();
    let rules_total: u64 = ds.tables.iter().map(|t| t.records as u64).sum();
    let solve_by_source: BTreeMap<String, usize> =
        res.solve_requests.iter().fold(BTreeMap::new(), |mut m, r| {
            *m.entry(r.source.clone()).or_default() += 1;
            m
        });
    let pb_count = |kind: &str, reason: &str| {
        proof_batches
            .iter()
            .filter(|b| b.kind == kind && b.reason == reason)
            .map(ProofBatch::keys)
            .sum::<u64>()
    };
    let level_word = required.word();
    let check_v = json!({
        "schema": CHECK_SCHEMA, "node": inp.node, "round": inp.round, "target": inp.target,
        "spec_digest": res.spec_digest, "key_set_digest": res.key_set_digest,
        "tier": format!("{:?}", res.tier), "exact_tier": {"wanted": inp.exact_tier, "reached": tier_c_now,
            "facts_missing_keys": n_missing, "keys_only_closure": res_k.rows.len()},
        "required_evidence": level_word,
        "keys": keys, "in_start": res.summary.keys_in_start, "served": served, "terminal": terminal, "open": open,
        "certified": certified, "defect": defect, "closed": closed,
        "closure": {"verdict": format!("{:?}", res.summary.verdict), "rounds": res.summary.rounds,
                    "edges": res.summary.edges, "used_rules": res.summary.used_rules,
                    "descent_violations": res.summary.descent_violations, "blocking": res.summary.blocking,
                    "diagnostic": diagnostic, "relaxed_checks": res.summary.relaxed_checks},
        "strict": strict, "strict_why": strict_why, "blocking": blocking, "counts": counts,
        "evidence_by_class": evidence_by_class, "by_rule_class_level": by_rule_class_level,
        "relation_rule": inp.relation_rule,
        "relations": {"records": rels.len(), "proved": proved.len(), "proof_verdicts": relation_proof_status},
        "evidence_import": serde_json::to_value(&ev.counts).unwrap_or(Value::Null),
        "requests": {"solve": res.solve_requests.len(), "solve_new": new_reqs.len(),
                     "solve_by_source": solve_by_source, "fact_records": res.fact_requests.len(),
                     "fact_keys": fact_keys, "proof_records": res.proof_requests.len(), "proof_keys": proof_keys},
        "solve_refusals": res.solve_refusals.len(),
        "batches": batches.iter().map(|b| json!({"source": b.source, "keys": b.keys.len()})).collect::<Vec<_>>(),
        "proof_batches": {"span_import": pb_count("span", REASON_IMPORT_PROOF),
                          "span_relation": pb_count("span", REASON_RELATION),
                          "facts": pb_count("facts", REASON_FACTS), "jobs": proof_batches.len()},
        "budget": budget_why, "budget_stop": budget_stop,
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
    let final_ = if closed || (batches.is_empty() && proof_batches.is_empty()) {
        let verdict = if closed {
            "closed"
        } else if budget_stop {
            "budget"
        } else {
            "open"
        };
        Some(write_final(
            inp,
            &set,
            &ev,
            res,
            Some(&res_k).filter(|_| tier_c_now),
            FinalCtx {
                verdict,
                blocking: &blocking,
                budget_why: if budget_stop {
                    budget_why.as_deref()
                } else {
                    None
                },
                check: &check_v,
                elapsed: t0.elapsed().as_secs_f64(),
                required,
                basis_ok,
            },
        )?)
    } else {
        None
    };
    Ok(PassOut {
        round: inp.round,
        spec_digest: set.report.spec_digest.clone(),
        check: check_v,
        defect,
        closure_keys: keys,
        rules_used: res.summary.used_rules,
        rules_total: rules_total.max(res.summary.used_rules),
        batches,
        proof_batches,
        final_,
        dispatch_order,
        import_manifest,
        seconds: t0.elapsed().as_secs_f64(),
        set,
    })
}

/// Rule class x level (closeloop.md 13.2) from the import's evidence by class: the class is the import source's
/// `class` (data), refined by the basis for symmetry relations / images and our recipe relations; levels `imported` /
/// `replay` / `per-key` / `terminal` / `open` (and `none` for a served key without a level).
fn class_level(
    by_class: &BTreeMap<String, BTreeMap<String, u64>>,
) -> BTreeMap<String, BTreeMap<String, u64>> {
    let mut m: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    for (class, e) in by_class {
        for (k, n) in e {
            let (c, level) = if let Some(rest) = k.strip_prefix("served:") {
                let (lvl, basis) = rest.split_once(':').unwrap_or((rest, "-"));
                let c = match basis {
                    "symmetry_relation" => "symmetry relation".to_string(),
                    "symmetry_credit" => "symmetry image".to_string(),
                    _ => class.clone(),
                };
                (c, lvl.to_string())
            } else if k == "terminal" {
                ("terminal".to_string(), "terminal".to_string())
            } else if k.starts_with("open:") {
                (class.clone(), "open".to_string())
            } else {
                continue;
            };
            *m.entry(c).or_default().entry(level).or_default() += n;
        }
    }
    m
}

/// The rule class of a row's server (the import spec's `class` of its source; supplied / recipe sets by their id).
fn class_of(set: &ImportedSet, r: &KeyRow) -> String {
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

struct FinalCtx<'a> {
    verdict: &'a str,
    blocking: &'a [String],
    budget_why: Option<&'a str>,
    check: &'a Value,
    elapsed: f64,
    required: Level,
    basis_ok: bool,
}

/// Phase D: our statement, the comparison with the hand statement, the open list, the economy numbers, the result.
fn write_final(
    inp: &PassIn,
    set: &ImportedSet,
    ev: &Evidence,
    res: &CheckResult,
    keys_only: Option<&CheckResult>,
    f: FinalCtx<'_>,
) -> Result<FinalOut, String> {
    std::fs::create_dir_all(&inp.final_dir)
        .map_err(|e| format!("{}: {e}", inp.final_dir.display()))?;
    // the open list (typed): every kept row that is not certified (plain streamed rows are certified)
    let mut open_lines = String::new();
    let mut open_n = 0u64;
    for r in &res.rows {
        if row_certified(r, f.required, f.basis_ok) {
            continue;
        }
        open_n += 1;
        let v = json!({"key": r.key, "status": r.status, "class": class_of(set, r),
            "evidence": r.evidence.map(|e| e.word()), "basis": r.basis,
            "open_status": r.open_status, "open_reason": r.open_reason,
            "regularity_unknown": r.regularity_unknown, "coefficient_unknown": r.coefficient_unknown});
        open_lines.push_str(&v.to_string());
        open_lines.push('\n');
    }
    std::fs::write(inp.final_dir.join("open_keys.jsonl"), &open_lines)
        .map_err(|e| format!("open_keys: {e}"))?;
    let sdir = inp.final_dir.join("statement");
    let header = json!({"check_pass_s": f.elapsed, "closeloop": {"target": inp.target,
        "relation_rule": inp.relation_rule, "required_evidence": f.required.word(),
        "tier": format!("{:?}", res.tier), "verdict": f.verdict}});
    let dig = write_statement(set, ev, res, &sdir, Some(header))
        .map_err(|e| format!("statement: {e}"))?;
    let compare_into = |r: &CheckResult, dir: &str| -> Result<Value, String> {
        let Some(h) = &inp.hand else {
            return Ok(Value::Null);
        };
        let fmt: HandFormat =
            serde_json::from_value(h.format.clone()).map_err(|e| format!("hand format: {e}"))?;
        let hand = read_hand_statement(&h.path, &fmt, set.arity())
            .map_err(|e| format!("hand statement: {e}"))?;
        let c = compare(set, r, &hand);
        c.write(&inp.final_dir.join(dir))
            .map_err(|e| format!("comparison: {e}"))?;
        let mut v = serde_json::to_value(&c).map_err(|e| e.to_string())?;
        if let Some(o) = v.as_object_mut() {
            o.remove("differences");
            o.insert("differences_listed".into(), json!(c.differences.len()));
        }
        Ok(v)
    };
    let comparison = compare_into(res, "comparison")?;
    // the keys-only superset check keeps every row: its comparison lists the server of EVERY key
    let comparison_k = match keys_only {
        Some(k) => compare_into(k, "comparison-keys-only")?,
        None => Value::Null,
    };
    // economy (R48 addendum 4): closure size, used rules, recipes used and their solved keys
    let mut served_by_recipe: BTreeMap<String, u64> = BTreeMap::new();
    let mut served_by_supplied: BTreeMap<String, u64> = BTreeMap::new();
    for r in &res.rows {
        if let (RowStatus::Served, Some(Server::Relation { set: s, recipe })) =
            (r.status, &r.server)
        {
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
        .map(|m| {
            m.iter()
                .filter(|(v, _)| *v != "SOLVED")
                .map(|(_, n)| n)
                .sum::<u64>()
        })
        .sum();
    let label = if f.verdict == "closed" {
        format!("closed, {}, {}", inp.target, f.required.word())
    } else {
        f.verdict.to_string()
    };
    let result = json!({
        "schema": RESULT_SCHEMA, "node": inp.node, "verdict": f.verdict, "label": label,
        "target": inp.target, "level": f.required.word(), "rounds": inp.round,
        "blockers": f.blocking, "budget_stop": f.budget_why,
        "open_keys": {"file": "open_keys.jsonl", "count": open_n},
        "check": f.check,
        "statement": {"dir": sdir.display().to_string(), "files": dig.files},
        "comparison": comparison, "comparison_keys_only": comparison_k,
        "economy": {"closure_keys": f.check["keys"].clone(), "used_rules": res.summary.used_rules,
                    "recipes_used": served_by_recipe.len(), "keys_by_recipe": served_by_recipe,
                    "supplied_sets_used": served_by_supplied, "solved_keys": solved_keys, "failed_keys": failed_keys,
                    "answers_by_recipe": answered, "relation_rule": inp.relation_rule},
        "evidence_by_class": f.check["evidence_by_class"].clone(),
        "by_rule_class_level": f.check["by_rule_class_level"].clone(),
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
