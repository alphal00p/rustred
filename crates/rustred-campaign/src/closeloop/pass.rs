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

use super::config::{
    Budgets, EscalationCfg, HandCfg, ScaleCfg, SecondRouteCfg, TerminalAccept, TerminalsCfg,
};
use super::derived::{DerivedState, EscOut};
use super::proofs::{
    ProofBatch, REASON_EXACT, REASON_FACTS, REASON_IMPORT_PROOF, REASON_RELATION,
    REASON_SECOND_ROUTE, RecordReq, relation_records,
};
use super::terminals::TerminalDecision;
use super::zeros;
use rustred_campaign_core::closure::relations::DispatchSlot;
use rustred_campaign_core::closure::{Tier, Verdict};
use rustred_campaign_core::import::evidence::EvidenceOptions;
use rustred_campaign_core::import::load::RelEntry;
use rustred_campaign_core::import::spec::FileRef as ImportFileRef;
use rustred_campaign_core::import::store::{EvidenceStore, StoreOptions};
use rustred_campaign_core::import::{
    CheckOptions, CheckResult, Evidence, HandFormat, ImportedSet, KeyRow, Level, RowStatus, Server,
    TableSetSpec, WitnessKind, check, compare, load, read_hand_statement, write_statement,
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
    /// lane closeloop3: (record, key) pairs per `coefficient-facts` job; None = no exact evaluation configured (the
    /// records rr_proof instantiate refuses get no fact job).
    pub exact_keys_per_job: Option<usize>,
    /// lane t266run: instantiated terms of one facts job (None = no term cap).
    pub max_fact_terms_per_job: Option<u64>,
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
    /// lane closeloop3 (closeloop.md 16.6): (recipe, key) pairs whose solves all ended ERROR / IRREGULAR, asked again
    /// this round, and those whose attempts are used up (attention entries: source, key, verdict, attempts).
    pub engine_retried: u64,
    pub engine_attention: Vec<Value>,
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
    /// lane closeloop3 (closeloop.md 16.9): this run's box clause id and the earlier clauses (id, result.json).
    pub box_clause: String,
    pub prior_clauses: Vec<(String, PathBuf)>,
    // ---- lane closeloop4 ----
    /// Second-route fact answer files (role `closeloop_facts2`), recorded BLAKE3 (closeloop.md 18).
    pub fact2_files: Vec<(PathBuf, String)>,
    /// (record_sha256, key) pairs already given to a second-route fact item.
    pub issued_facts2: BTreeSet<(String, Vec<i32>)>,
    pub second_route: SecondRouteCfg,
    /// closeloop.md 19: the terminal policy and the recorded decisions (ledger order).
    pub terminals: TerminalsCfg,
    pub terminal_decisions: Vec<TerminalDecision>,
    /// lane closeloop4 (audit-closeloop M1): the digests of every file an earlier pass of the node read (tables, start
    /// set, terminals, spec evidence, hand statement, prior results), as recorded in the ledger (`closeloop:<n>:pins`):
    /// path -> sha256. A file whose digest differs refuses the pass; None = first pass.
    pub pins_recorded: Option<BTreeMap<String, String>>,
    /// lane closeloop4 (audit-closeloop M3): `block` (default) | `report`: a closed node's unexplained reference keys.
    pub reference_mismatch: String,
    /// The prior clauses' expected sha256 (config), by id.
    pub prior_sha256: BTreeMap<String, String>,
    /// lane t266run: big-node mode (evidence store, streamed keys-only pass); None = as before.
    pub scale: Option<ScaleCfg>,
    /// lane t266run: the node's own escalation of uncovered keys (closeloop.md 17); None = as before.
    pub escalation: Option<EscalationCfg>,
    /// lane t266run: the escalation recorded so far (from the ledger's inputs).
    pub derived: DerivedState,
    /// lane t266run: absolute directory of this round's escalation recipe files.
    pub recipe_dir: PathBuf,
    /// lane t266run: the clauses of the box (closeloop.md 17.4); None = one box.
    pub clauses: Option<Vec<super::config::ClauseCfg>>,
    // ---- lane layers (closeloop.md 23) ----
    /// The layers of the box (None = one walk).
    pub layers: Option<super::config::LayersCfg>,
    /// Prior clauses' leaf sets (id, pinned reference): leaves of every walk.
    pub prior_leaf_sets: Vec<(String, super::config::LeafSetRef)>,
    /// Frozen layers as recorded in the ledger: (layer index, record bytes, BLAKE3-verified by the source).
    pub frozen_layers: Vec<(usize, Vec<u8>)>,
    /// Where the pass sends the record of a layer it freezes (registered by the source while the pass runs).
    pub layer_tx: Option<std::sync::mpsc::Sender<super::layers::LayerMsg>>,
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
            engine_retried: 0,
            engine_attention: vec![],
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
            box_clause: "box".into(),
            prior_clauses: vec![],
            fact2_files: vec![],
            issued_facts2: BTreeSet::new(),
            second_route: SecondRouteCfg::default(),
            terminals: TerminalsCfg::default(),
            terminal_decisions: vec![],
            pins_recorded: None,
            reference_mismatch: "block".into(),
            prior_sha256: BTreeMap::new(),
            scale: None,
            escalation: None,
            derived: DerivedState::default(),
            recipe_dir: PathBuf::from("/nonexistent-escalation-dir"),
            clauses: None,
            layers: None,
            prior_leaf_sets: vec![],
            frozen_layers: vec![],
            layer_tx: None,
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
    /// lane closeloop4 (closeloop.md 19): terminal proposals waiting for an operator decision (no final verdict yet).
    pub terminal_proposals: Option<Value>,
    /// lane closeloop4: proposals accepted automatically now (`terminals.accept = auto`, at most `max_auto`).
    pub terminal_decision: Option<TerminalDecision>,
    /// lane closeloop4 (audit-closeloop M1): every file this pass read with its sha256 (recorded + new); registered as
    /// the node's `pins` input when it differs from the recorded set.
    pub pins: BTreeMap<String, String>,
    /// lane t266run: the escalation output of the pass (closeloop.md 17).
    pub escalation: Option<EscOut>,
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
    let (_, d) = crate::atomic::blake3_file(p).map_err(|e| format!("file {}: {e}", p.display()))?;
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
        manifest: None,
        family: None,
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
pub(crate) fn row_certified(r: &KeyRow, required: Level, basis_ok: bool) -> bool {
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
    // TESTS ONLY (lane closeloop4): a slow pass (the supervisor must stay responsive while a pass runs)
    if let Some(ms) = std::env::var("RR_CLOSELOOP_TEST_PASS_DELAY_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
    {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
    let spec = TableSetSpec::from_json(&inp.spec_text).map_err(|e| format!("import spec: {e}"))?;
    let set = match cached {
        Some(s) => s,
        None => Arc::new(
            match &inp.scale {
                // lane t266run: a persistent table index in big-node mode
                Some(sc) => rustred_campaign_core::import::load_with(
                    &spec,
                    &inp.base,
                    &rustred_campaign_core::import::LoadOptions {
                        index_dir: Some(sc.index_dir.clone().unwrap_or_else(|| {
                            inp.final_dir
                                .parent()
                                .unwrap_or_else(|| Path::new("/"))
                                .join("index")
                        })),
                    },
                ),
                None => load(&spec, &inp.base),
            }
            .map_err(|e| format!("import load: {e}"))?,
        ),
    };
    // lane t266run: in big-node mode the spec's facts / proofs and the node's own answer files live in disk-backed
    // evidence stores (interfaces/evidence-store.md), never in memory
    let store_opts = inp.scale.as_ref().map(|sc| StoreOptions {
        dir: sc.store_dir.clone().unwrap_or_else(|| {
            inp.final_dir
                .parent()
                .unwrap_or_else(|| Path::new("/"))
                .join("store")
        }),
        budget_bytes: (sc.store_budget_mib as usize) << 20,
        threads: sc.store_threads,
    });
    let mut ev = Evidence::from_spec_with(
        &set,
        &inp.base,
        &EvidenceOptions {
            store: store_opts.clone(),
        },
    )
    .map_err(|e| format!("evidence: {e}"))?;
    // lane closeloop4 (audit-closeloop M1): the spec's evidence was read PINNED; the node's own answer / proof / fact
    // files below are verified against the BLAKE3 the ledger recorded, right before each is read
    ev.accept_ledger_verified_files();
    // every file the load / spec evidence read, with its sha256: checked against the ledger's record of the first use
    let mut pins: BTreeMap<String, String> = BTreeMap::new();
    let strip = |d: &str| d.trim_start_matches("sha256:").to_string();
    for (p, d) in set
        .report
        .sources
        .iter()
        .flat_map(|s| s.files.iter())
        .chain(ev.counts.files.iter())
    {
        pins.insert(p.clone(), strip(d));
    }
    // the start set's file (its digest of KEYS is in the report; the file itself is pinned here)
    {
        let p = rustred_campaign_core::import::spec::resolve(&inp.base, &set.spec.start.path);
        if p.is_file() {
            let d = rustred_campaign_core::import::lines::file_sha256(&p)
                .map_err(|e| format!("start {}: {e}", p.display()))?;
            pins.insert(p.display().to_string(), d);
        }
    }
    for su in &ev.counts.stores {
        for (p, d) in &su.inputs {
            pins.insert(p.clone(), strip(d));
        }
    }
    // the reference and the earlier clauses' results (read at phase D)
    let mut pin_file = |p: &Path, want: Option<&str>, what: &str| -> Result<(), String> {
        if !p.is_file() {
            return Ok(()); // a missing file is phase D's error
        }
        let d = rustred_campaign_core::import::lines::file_sha256(p)
            .map_err(|e| format!("{what} {}: {e}", p.display()))?;
        if let Some(w) = want
            && !w.trim_start_matches("sha256:").eq_ignore_ascii_case(&d)
        {
            return Err(format!(
                "{what} {}: sha256 {d} differs from the configured {w}: refused",
                p.display()
            ));
        }
        pins.insert(p.display().to_string(), d);
        Ok(())
    };
    if let Some(h) = &inp.hand {
        pin_file(&h.path, h.sha256.as_deref(), "hand statement")?;
    }
    for (id, p) in &inp.prior_clauses {
        pin_file(
            p,
            inp.prior_sha256.get(id).map(String::as_str),
            "prior clause result",
        )?;
    }
    if let Some(rec) = &inp.pins_recorded {
        let changed: Vec<String> = pins
            .iter()
            .filter(|(p, d)| rec.get(*p).is_some_and(|r| r != *d))
            .map(|(p, d)| format!("{p} (now {d}, recorded {})", rec[p]))
            .collect();
        if !changed.is_empty() {
            return Err(format!(
                "{} input file(s) changed since their first use recorded in the ledger (refused): {}",
                changed.len(),
                changed.join("; ")
            ));
        }
        for (p, d) in rec {
            pins.entry(p.clone()).or_insert_with(|| d.clone());
        }
    }
    for (p, b3) in &inp.answers {
        verify_b3(p, b3)?;
        ev.load_solved(&set, Path::new("/"), &fref(p))
            .map_err(|e| format!("answers {}: {e}", p.display()))?;
    }
    if let (Some(so), Some(sc)) = (&store_opts, &inp.scale) {
        for files in [&inp.proof_files, &inp.fact_files] {
            for chunk in files.chunks(sc.store_chunk_files.max(1)) {
                for (p, b3) in chunk {
                    verify_b3(p, b3)?;
                }
                let list: Vec<(PathBuf, Option<String>)> =
                    chunk.iter().map(|(p, _)| (p.clone(), None)).collect();
                let st = EvidenceStore::open_or_build(&set, &list, so)
                    .map_err(|e| format!("evidence store: {e}"))?;
                ev.attach_store(st);
            }
        }
    } else {
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
    }
    // lane closeloop4 (closeloop.md 18): the SECOND route's answers in their OWN evidence, compared with the first
    // route pair by pair over every term BEFORE the check: agreement confirms the fact (basis of both routes), a
    // disagreement DISPUTES it (every term of the pair is unknown: an edge, the key uncertified) and blocks the node
    let t_sr = std::time::Instant::now();
    let mut ev2 = Evidence::new(&set);
    ev2.accept_ledger_verified_files();
    let mut answered2: BTreeSet<(String, Vec<i32>)> = BTreeSet::new();
    for (p, b3) in &inp.fact2_files {
        verify_b3(p, b3)?;
        ev2.load_facts(&set, Path::new("/"), &fref(p))
            .map_err(|e| format!("second-route facts {}: {e}", p.display()))?;
        answered2.extend(zeros::answered_pairs(p)?);
    }
    let mut second = zeros::SecondRoute::default();
    for (sha, key) in &answered2 {
        let Some(c) = zeros::compare_pair(&set, &ev, &ev2, sha, key) else {
            continue;
        };
        let elided = set
            .find_sha(sha)
            .is_some_and(|(t, r)| set.rules[t][r].witness == WitnessKind::Elided);
        if c.agree() {
            ev.confirm_basis(sha, key, zeros::confirmed_basis(c.first, elided));
        } else {
            ev.dispute(sha, key);
        }
        second.compared.insert((sha.clone(), key.clone()), c);
    }
    second.seconds = t_sr.elapsed().as_secs_f64();
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
    // lane layers (closeloop.md 23): the layers of the box, the frozen ones (from the ledger, verified), the prior
    // clauses' leaf sets (pinned); None = one walk as before
    let layer_plan = super::layers::plan(inp, &set)?;
    // pass 1: keys-only when the exact tier is wanted (cheap; every row kept: the fact requests come from it; in
    // big-node mode the rows go to a file and only the exceptional ones stay in memory)
    let stream_k = inp.scale.as_ref().is_some_and(|s| s.stream_keys_only);
    let krows = inp.rows_path.with_extension("k.jsonl");
    let opts_k = CheckOptions {
        threads,
        tier: if inp.exact_tier { Some(Tier::K) } else { None },
        max_rounds: None,
    };
    // lane t266run: with `scale.keys_only_first: false` the exact tier runs at once (its own fact / proof requests
    // drive the jobs; only sensible when the evidence is mostly in, else the requests are many)
    let skip_k = inp.exact_tier && inp.scale.as_ref().is_some_and(|s| !s.keys_only_first);
    let res_k: Option<CheckResult> = if skip_k {
        None
    } else {
        Some(match &layer_plan {
            // lane layers: one keys-only walk of the layers that are not frozen, the frozen ones as leaves
            Some(lp) => super::layers::check_k(
                &set,
                &ev,
                &opts_k,
                stream_k.then_some(krows.as_path()),
                lp,
            )?,
            None => if stream_k {
                rustred_campaign_core::import::check::check_streamed(&set, &ev, &opts_k, &krows)
            } else {
                check(&set, &ev, &opts_k)
            }
            .map_err(|e| format!("check: {e}"))?,
        })
    };
    let keys_only_n = res_k
        .as_ref()
        .map(|r| {
            r.streamed
                .as_ref()
                .map(|s| s.rows as usize)
                .unwrap_or(r.rows.len())
        })
        .unwrap_or(0);
    // the facts the exact tier needs: every served rule key's (record, key) not asked yet
    let mut missing_facts: BTreeMap<(usize, usize), BTreeSet<Vec<i32>>> = BTreeMap::new();
    // records with ELIDED witness loci: rr_proof instantiate refuses them as a whole (fail closed, R-f). Lane
    // closeloop3 (closeloop.md 16): with `proof.exact_eval` their facts come from the EXACT evaluation of their own
    // coefficient text (`coefficient-facts` jobs, producer exact_eval; regularity from the bound per-key proof);
    // without it they get no fact job and the exact tier lists what stays unknown
    let exact_eval = inp
        .proof
        .as_ref()
        .is_some_and(|p| p.exact_keys_per_job.is_some());
    let mut missing_exact: BTreeMap<(usize, usize), BTreeSet<Vec<i32>>> = BTreeMap::new();
    let mut elided_no_facts = 0u64;
    if inp.exact_tier && stream_k && !skip_k {
        super::scale::missing_facts_streamed(
            &set,
            &ev,
            &krows,
            &inp.issued_facts,
            &mut missing_facts,
            &mut elided_no_facts,
        )?;
    } else if inp.exact_tier {
        for r in res_k.iter().flat_map(|x| x.rows.iter()) {
            if let (RowStatus::Served, Some(Server::Rule { table, record })) = (r.status, &r.server)
            {
                if set.rules[*table][*record].witness == WitnessKind::Elided {
                    let sha = &set.rules[*table][*record].record_sha256;
                    if !exact_eval {
                        elided_no_facts += 1;
                    } else if !ev.has_fact(&set, *table, *record, &r.key)
                        && !inp.issued_facts.contains(&(sha.clone(), r.key.clone()))
                    {
                        missing_exact
                            .entry((*table, *record))
                            .or_default()
                            .insert(r.key.clone());
                    }
                    continue;
                }
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
    let n_missing: usize = missing_facts.values().map(BTreeSet::len).sum::<usize>()
        + missing_exact.values().map(BTreeSet::len).sum::<usize>();
    let can_ask_facts = inp.proof.as_ref().is_some_and(|p| p.facts);
    let tier_c_now = inp.exact_tier && n_missing == 0;
    // lane layers (closeloop.md 23): the exact tier LAYER BY LAYER (each judged and, when closed, frozen at once), then
    // the node's composed check
    let mut layer_walked: Option<Vec<super::layers::Walked>> = None;
    let streamed_res: Option<CheckResult> = if let (true, Some(lp)) = (tier_c_now, &layer_plan) {
        let (strict0, _) = strictness(&set);
        let unproved_keys: BTreeSet<Vec<i32>> = rels
            .iter()
            .filter(|r| !proved.contains(&(r.source.clone(), r.key.clone())))
            .map(|r| r.key.clone())
            .collect();
        let gates = super::layers::Gates {
            required: inp.required.unwrap_or(set.spec.required_evidence),
            strict: strict0,
            second: &second,
            second_decisive: inp.second_route.decisive_zeros,
            second_elided: inp.second_route.elided,
            unproved_relation_keys: &unproved_keys,
            threads,
        };
        let k_keys: Option<rustred_campaign_core::closure::KeySet> = res_k.as_ref().map(|r| {
            r.key_set.clone().unwrap_or_else(|| {
                rustred_campaign_core::closure::KeySet::from_keys(
                    set.codec,
                    r.rows.iter().map(|x| x.key.as_slice()),
                )
                .unwrap_or_else(|_| rustred_campaign_core::closure::KeySet::empty(set.codec))
            })
        });
        // the binding of every frozen layer / prior leaf set, verified even when nothing is walked (empty walk)
        let base = rustred_campaign_core::import::check::check_from_with_leaves(
            &set,
            &ev,
            &CheckOptions {
                threads,
                tier: Some(Tier::C),
                max_rounds: None,
            },
            &rustred_campaign_core::closure::KeySet::empty(set.codec),
            &lp.base_leaves(),
        )
        .map_err(|e| format!("leaf sets: {e}"))?;
        let walked = super::layers::walk_c(
            &set,
            &ev,
            inp,
            lp,
            &gates,
            k_keys.as_ref(),
            inp.layer_tx.as_ref(),
        )?;
        let composed = super::layers::compose(&set, lp, &walked, &inp.rows_path, &base)?;
        layer_walked = Some(walked);
        Some(composed)
    } else if tier_c_now {
        Some(
            rustred_campaign_core::import::check::check_streamed(
                &set,
                &ev,
                &CheckOptions {
                    threads,
                    tier: Some(Tier::C),
                    max_rounds: None,
                },
                &inp.rows_path,
            )
            .map_err(|e| format!("exact-tier check: {e}"))?,
        )
    } else {
        None
    };
    let res: &CheckResult = streamed_res
        .as_ref()
        .or(res_k.as_ref())
        .ok_or("no check result")?;
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
    if !inp.engine_attention.is_empty() {
        blocking.push(format!(
            "solve_keys_engine_failure_attention:{}",
            inp.engine_attention.len()
        ));
    }
    if inp.proof_attention > 0 {
        blocking.push(format!("proof_items_in_attention:{}", inp.proof_attention));
    }
    // lane closeloop4 (closeloop.md 18): the decisive zeros of the decided exact tier and their second route
    let mut second_missing: BTreeMap<(usize, usize, &'static str), BTreeSet<Vec<i32>>> =
        BTreeMap::new();
    let mut second_v = Value::Null;
    let mut decisive_lines: Vec<Value> = vec![];
    // lane closeloop4 (closeloop.md 21): ONE scan of the decided exact tier's edges: the decisive zeros AND the
    // standing DESCENT gate (every followed edge strictly lower under the node's FULL comparator; the closure's own
    // Tarjan pass for cycles): `closed` needs non_descending_edges == 0 and cycles == 0
    let scan_all = if tier_c_now {
        Some(match &layer_walked {
            // lane layers: the walked layers' own scans (frozen layers are never scanned again)
            Some(w) => super::layers::merged_scan(w).0,
            None => zeros::scan(&set, &ev, res, threads)?,
        })
    } else {
        None
    };
    let mut descent_v = Value::Null;
    if let Some(sc) = &scan_all {
        let (rel_edges, rel_bad) = match &layer_walked {
            Some(w) => {
                let m = super::layers::merged_scan(w);
                (m.1, m.2)
            }
            None => zeros::relation_descent(&set, &ev, res),
        };
        let mut bad: Vec<Value> = sc
            .non_descending
            .iter()
            .chain(rel_bad.iter())
            .map(|(k, srv, t)| json!({"key": k, "server": srv, "term": t}))
            .collect();
        bad.sort_by_key(|v| v.to_string());
        let n_bad = bad.len() as u64;
        let core_violations = res.summary.descent_violations;
        let cycles = res.summary.cycles;
        if n_bad > 0 || core_violations > 0 {
            blocking.push(format!("non_descending:{}", n_bad.max(core_violations)));
        }
        match cycles {
            Some(0) => {}
            Some(c) => blocking.push(format!("cycles:{c}")),
            None => blocking.push("cycles_not_checked".into()),
        }
        descent_v = json!({"comparator": "the node's full NodeComparator (every tie-break)",
            "followed_edges": sc.followed_top + rel_edges, "non_descending_edges": n_bad,
            "core_descent_violations": core_violations, "cycles": cycles,
            "dropped_zero_edges_non_descending": sc.dropped_non_descending,
            "non_descending_list": bad.into_iter().take(1000).collect::<Vec<_>>()});
    }
    if tier_c_now && (inp.second_route.decisive_zeros || inp.second_route.elided) {
        let mut scan = scan_all.clone().unwrap_or_default();
        if !inp.second_route.decisive_zeros {
            scan.decisive.clear();
        }
        // the pairs to confirm: every decisive zero's (record, applied key); with `elided`, every served pair of a
        // record with elided loci (its first route is the exact evaluation)
        // (record sha, applied key) -> (table, record, decisive, elided)
        type PairInfo = (usize, usize, bool, bool);
        let mut pairs: BTreeMap<(String, Vec<i32>), PairInfo> = BTreeMap::new();
        for z in &scan.decisive {
            let sha = set.rules[z.table][z.record].record_sha256.clone();
            pairs
                .entry((sha, z.applied.clone()))
                .or_insert((z.table, z.record, true, false))
                .2 = true;
        }
        let has_elided = set
            .rules
            .iter()
            .flatten()
            .any(|r| r.witness == WitnessKind::Elided);
        if inp.second_route.elided && has_elided {
            for (_, applied, t, r) in zeros::served_rule_rows(&set, res)? {
                if set.rules[t][r].witness == WitnessKind::Elided {
                    let sha = set.rules[t][r].record_sha256.clone();
                    pairs.entry((sha, applied)).or_insert((t, r, false, true)).3 = true;
                }
            }
        }
        let rr_proof_ok = inp.proof.as_ref().is_some_and(|p| p.facts);
        let exact_ok = inp
            .proof
            .as_ref()
            .is_some_and(|p| p.exact_keys_per_job.is_some());
        let (
            mut confirmed,
            mut disagree,
            mut pending,
            mut unanswered,
            mut unavailable,
            mut no_first,
        ) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
        let mut status_of: BTreeMap<(String, Vec<i32>), &'static str> = BTreeMap::new();
        for ((sha, key), (t, r, dec, _)) in &pairs {
            let st = if let Some(c) = second.compared.get(&(sha.clone(), key.clone())) {
                if c.agree() {
                    confirmed += 1;
                    "confirmed"
                } else {
                    disagree += 1;
                    "disagreement"
                }
            } else {
                let rid = zeros::rule_id(&set, *t, *r);
                let first = ev.fact_basis(&rid, key);
                match first.and_then(zeros::second_route_of) {
                    // no fact bound: no zero was used at this pair (its terms are edges); a DECISIVE zero always
                    // has one, so this is only an elided pair whose first route did not run (nothing to confirm)
                    None => {
                        if *dec {
                            no_first += 1;
                        }
                        "no_fact"
                    }
                    Some(route) => {
                        if inp.issued_facts2.contains(&(sha.clone(), key.clone())) {
                            unanswered += 1;
                            "unanswered"
                        } else if (route == "rr_proof" && !rr_proof_ok)
                            || (route == "exact_eval" && !exact_ok)
                        {
                            unavailable += 1;
                            "unavailable"
                        } else {
                            pending += 1;
                            second_missing
                                .entry((*t, *r, route))
                                .or_default()
                                .insert(key.clone());
                            "pending"
                        }
                    }
                }
            };
            status_of.insert((sha.clone(), key.clone()), st);
        }
        for z in &scan.decisive {
            let info = &set.rules[z.table][z.record];
            let id = (info.record_sha256.clone(), z.applied.clone());
            let c = second.compared.get(&id);
            let first = c
                .map(|c| c.first)
                .or_else(|| ev.fact_basis(&zeros::rule_id(&set, z.table, z.record), &z.applied))
                .unwrap_or("none");
            decisive_lines.push(zeros::decisive_line(
                &set,
                z,
                first,
                c,
                status_of.get(&id).copied().unwrap_or("pending"),
            ));
        }
        let dis_list: Vec<Value> = second
            .disagreements()
            .map(zeros::PairCompare::json)
            .collect();
        // the disputed pairs of earlier answers stay blocking even when no longer decisive (their terms are edges now)
        let disputed_all = second.disagreements().count() as u64;
        if disputed_all > 0 {
            blocking.push(format!("second_route_disagreement:{disputed_all}"));
        }
        if pending > 0 {
            blocking.push(format!("second_route_pending:{pending}"));
        }
        if unanswered > 0 {
            blocking.push(format!("second_route_unanswered:{unanswered}"));
        }
        if unavailable > 0 {
            blocking.push(format!("second_route_unavailable:{unavailable}"));
        }
        if no_first > 0 {
            blocking.push(format!("second_route_first_fact_missing:{no_first}"));
        }
        let by_kind = scan.decisive.iter().fold(BTreeMap::new(), |mut m, z| {
            *m.entry(z.kind).or_insert(0u64) += 1;
            m
        });
        let decisive_pairs = pairs.values().filter(|p| p.2).count();
        let elided_pairs = pairs.values().filter(|p| p.3).count();
        let _ = disagree;
        second_v = json!({"decisive_zeros": scan.decisive.len(), "decisive_by_kind": by_kind,
            "decisive_pairs": decisive_pairs, "elided_pairs": elided_pairs, "pairs": pairs.len(),
            "confirmed": confirmed, "disagreements": disputed_all, "pending": pending, "unanswered": unanswered,
            "unavailable": unavailable, "first_route_missing": no_first,
            "zero_terms": {"top": scan.zero_top, "positive_numerator": scan.zero_positive_numerator,
                           "other": scan.zero_other},
            "keys_scanned": scan.keys_scanned, "scan_s": scan.seconds, "compare_s": second.seconds,
            "compared_pairs": second.compared.len(), "disagreement_list": dis_list,
            "config": {"decisive_zeros": inp.second_route.decisive_zeros, "elided": inp.second_route.elided}});
    }
    let closed = certified == keys && blocking.is_empty();
    // a verdict blocker without a per-key count counts as one defect (the node machine needs defect > 0 to end)
    let defect = if closed { 0 } else { (keys - certified).max(1) };
    // lane t266run (closeloop.md 17): the node's own escalation of the keys nothing serves
    let mut uncovered: Vec<Vec<i32>> = res
        .rows
        .iter()
        .filter(|r| r.status == RowStatus::Open && r.open_status.as_deref() == Some("uncovered"))
        .map(|r| r.key.clone())
        .collect();
    uncovered.sort();
    uncovered.dedup();
    let mut esc_out: Option<EscOut> = None;
    // (at the exact tier only when it is wanted: a key of the keys-only superset may be reached through zero terms)
    let decided_tier = !inp.exact_tier || tier_c_now;
    if let Some(esc) = &inp.escalation
        && !closed
        && decided_tier
        && inp.round <= inp.budgets.max_rounds
    {
        let group = if esc.symmetry_at_key
            && uncovered
                .iter()
                .any(|k| !inp.derived.symrel_tried.contains(k))
        {
            super::derived::group_of(esc, &inp.spec_text, &inp.base, &set)?
        } else {
            None
        };
        let fs = super::derived::face_successes(&ev.solved, set.frame.mask());
        esc_out = Some(super::derived::escalate(
            &set,
            esc,
            &inp.derived,
            &uncovered,
            group.as_ref(),
            &inp.recipe_dir,
            &fs,
        )?);
    }
    let hold: BTreeSet<Vec<i32>> = esc_out.as_ref().map(|e| e.hold.clone()).unwrap_or_default();
    // new solve requests: those of no item yet (termination: finitely many keys x recipes; descent, R49)
    let new_reqs: Vec<_> = res
        .solve_requests
        .iter()
        .filter(|r| {
            !inp.issued.contains(&(r.source.clone(), r.key.clone())) && !hold.contains(&r.key)
        })
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
        // lane t266run: per-key proofs only for the closure that is decided (the exact tier when it is wanted): the
        // keys-only superset holds keys reached only through zero terms
        if pp.import_requests && (decided_tier || inp.scale.is_none()) {
            for r in &res.proof_requests {
                let keys: Vec<Vec<i32>> = r
                    .keys
                    .iter()
                    .filter(|k| {
                        !inp.issued_proofs
                            .contains(&(r.record_sha256.clone(), (*k).clone()))
                    })
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
                if proved.contains(&id)
                    || inp.issued_proofs.contains(&(r.sha.clone(), r.key.clone()))
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
                    let elided = set.rules[t][r].witness == WitnessKind::Elided;
                    if elided && !exact_eval {
                        continue;
                    }
                    for k in &fr.keys {
                        if !inp
                            .issued_facts
                            .contains(&(fr.record_sha256.clone(), k.clone()))
                        {
                            let m = if elided {
                                &mut missing_exact
                            } else {
                                &mut missing_facts
                            };
                            m.entry((t, r)).or_default().insert(k.clone());
                        }
                    }
                }
            }
            let mut by_table: BTreeMap<String, Vec<(RecordReq, u64)>> = BTreeMap::new();
            for ((t, r), ks) in &missing_facts {
                let info = &set.rules[*t][*r];
                by_table.entry(info.path.clone()).or_default().push((
                    RecordReq {
                        rule: info.index.unwrap_or(info.label),
                        offset: info.offset,
                        record_sha256: info.record_sha256.clone(),
                        keys: ks.iter().cloned().collect(),
                    },
                    (set.bodies.n_terms(*t, *r) as u64).max(1),
                ));
            }
            let cap = pp.max_fact_keys_per_job.max(1);
            // lane t266run: a term cap too (keys x the record's term count), at least one key per job
            let tcap = pp.max_fact_terms_per_job.unwrap_or(u64::MAX).max(1);
            for (table, recs) in by_table {
                let mut cur: Vec<RecordReq> = vec![];
                let mut n = 0usize;
                let mut nt = 0u64;
                for (rr, terms) in recs {
                    let per = ((tcap / terms) as usize).clamp(1, cap);
                    for chunk in rr.keys.chunks(per) {
                        let ct = chunk.len() as u64 * terms;
                        if (n + chunk.len() > cap || nt.saturating_add(ct) > tcap)
                            && !cur.is_empty()
                        {
                            proof_batches.push(ProofBatch {
                                kind: "facts".into(),
                                reason: REASON_FACTS.into(),
                                table: table.clone(),
                                records: std::mem::take(&mut cur),
                                relation_source: None,
                            });
                            n = 0;
                            nt = 0;
                        }
                        n += chunk.len();
                        nt = nt.saturating_add(ct);
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
            // lane closeloop3: the elided records' facts, exact evaluation (one `coefficient-facts` job per table
            // and at most `exact_keys_per_job` (record, key) pairs)
            if let Some(cap) = pp.exact_keys_per_job {
                proof_batches.extend(exact_batches(&set, &missing_exact, cap.max(1)));
            }
        }
        // lane closeloop4: the second route of the facts to confirm (kept apart: reason second_route)
        if !second_missing.is_empty() {
            let pick = |route: &str| -> BTreeMap<(usize, usize), BTreeSet<Vec<i32>>> {
                second_missing
                    .iter()
                    .filter(|((_, _, r), _)| *r == route)
                    .map(|((t, r, _), ks)| ((*t, *r), ks.clone()))
                    .collect()
            };
            if let Some(cap) = pp.exact_keys_per_job {
                proof_batches.extend(record_batches(
                    &set,
                    &pick("exact_eval"),
                    cap.max(1),
                    "exact",
                    REASON_SECOND_ROUTE,
                ));
            }
            proof_batches.extend(record_batches(
                &set,
                &pick("rr_proof"),
                pp.max_fact_keys_per_job.max(1),
                "facts",
                REASON_SECOND_ROUTE,
            ));
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
    let mut esc_wait: Option<Value> = None;
    // lane t266run: TERMINAL PROPOSALS (closeloop.md 17): nothing more can be issued and the escalation made no
    // progress -> every uncovered key is a proposal; accepted under the spec's finite-terminal rule, else open
    if let Some(esc) = &inp.escalation
        && !closed
        && decided_tier
        && batches.is_empty()
        && proof_batches.is_empty()
        && !uncovered.is_empty()
        && !esc_out.as_ref().is_some_and(EscOut::progress)
    {
        let eo = esc_out.get_or_insert_with(EscOut::default);
        let lines_m: Vec<bool> = set.frame.mask().to_vec();
        let attempts: Vec<Value> = uncovered
            .iter()
            .map(|k| {
                let recipes: Vec<Value> = ev
                    .solved
                    .iter()
                    .filter_map(|(src, m)| m.get(k).map(|r| json!({"recipe": src, "verdict": r.verdict})))
                    .collect();
                let pending: Vec<&str> = res
                    .solve_requests
                    .iter()
                    .filter(|r| &r.key == k)
                    .map(|r| r.source.as_str())
                    .collect();
                let face = super::derived::face_id(k, &lines_m);
                json!({"key": k, "face": face, "symmetry_at_key_tried": inp.derived.symrel_tried.contains(k),
                       "face_escalated": inp.derived.faces.contains(&face), "recipe_answers": recipes,
                       "unanswered_candidates": pending,
                       "refusals": res.solve_refusals.iter().filter(|r| &r.key == k)
                           .map(|r| json!({"recipe": r.source, "reason": r.reason})).collect::<Vec<_>>()})
            })
            .collect();
        let doc = |status: &str| {
            json!({"schema": "rustred.campaign.closeloop-terminals.v1", "node": inp.node, "round": inp.round,
                "status": status, "terminals": uncovered, "attempts": attempts})
        };
        match &esc.terminals {
            None => blocking.push(format!(
                "terminal_proposals_without_rule: {} uncovered keys exhausted (no terminal rule configured)",
                uncovered.len()
            )),
            Some(rule) if uncovered.len() > rule.max_proposals => {
                // more proposals than the cap: the escalation is not working; stop with a typed reason
                let w = format!(
                    "terminal_proposals_over_cap: {} uncovered keys exhausted, the cap is {} (the escalation is not working)",
                    uncovered.len(),
                    rule.max_proposals
                );
                eo.over_rule = Some(w.clone());
                eo.proposals = Some(doc("over_cap"));
                blocking.push(w);
            }
            Some(rule) => {
                let approved: BTreeSet<&Vec<i32>> = rule.approved.iter().collect();
                let all_approved = uncovered.iter().all(|k| approved.contains(k));
                // lane closeloop4 (closeloop.md 19): automatic acceptance only under `terminals.accept = auto` (or the
                // escalation's explicit `max_terminals`), and never beyond `terminals.max_auto` (default 8) automatically
                // accepted terminals of the node in all; a key the operator rejected is never proposed again
                let auto_cap = match (inp.terminals.accept, rule.max_terminals) {
                    (TerminalAccept::Auto, m) => {
                        Some(m.unwrap_or(usize::MAX).min(inp.terminals.max_auto))
                    }
                    (TerminalAccept::DeclaredOnly, Some(m)) => Some(m.min(inp.terminals.max_auto)),
                    (TerminalAccept::DeclaredOnly, None) => None,
                };
                let auto_before = inp.derived.terminals.len()
                    + inp
                        .terminal_decisions
                        .iter()
                        .filter(|d| d.accepted_by == "auto")
                        .map(|d| d.accept.len())
                        .sum::<usize>();
                let auto = auto_cap.is_some_and(|m| auto_before + uncovered.len() <= m);
                if all_approved || auto {
                    eo.terminals = Some(doc(if all_approved { "accepted_pre_approved" } else { "accepted_auto" }));
                } else {
                    // OPERATOR DECISION (coordinator 2026-11-14): the proposals are reported, never accepted here; they
                    // wait (no final verdict, exit 8) for `rr_campaign control CDIR resolve-terminals` (closeloop.md 19)
                    esc_wait = Some(doc("awaiting_operator_decision"));
                }
            }
        }
    }
    let progress = esc_out.as_ref().is_some_and(EscOut::progress);
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
    // lane closeloop3 (closeloop.md 16.4): keys whose solve hit a per-key cap (each went to its next recipe step)
    let (mut cap_wall, mut cap_work) = (0u64, 0u64);
    for m in ev.solved.values() {
        for r in m.values() {
            match r.verdict.as_str() {
                "BUDGET_WALL" => cap_wall += 1,
                "BUDGET_WORK" => cap_work += 1,
                _ => {}
            }
        }
    }
    let check_v = json!({
        "schema": CHECK_SCHEMA, "node": inp.node, "round": inp.round, "target": inp.target,
        "spec_digest": res.spec_digest, "key_set_digest": res.key_set_digest,
        "tier": format!("{:?}", res.tier), "exact_tier": {"wanted": inp.exact_tier, "reached": tier_c_now,
            "facts_missing_keys": n_missing, "keys_only_closure": keys_only_n,
            "facts_unavailable_elided_keys": elided_no_facts,
            "exact_eval": exact_eval,
            "exact_eval_missing_keys": missing_exact.values().map(BTreeSet::len).sum::<usize>()},
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
                          "facts": pb_count("facts", REASON_FACTS),
                          "exact_eval": pb_count("exact", REASON_EXACT), "jobs": proof_batches.len()},
        "fact_basis": fact_basis_counts(&counts),
        "budget": budget_why, "budget_stop": budget_stop,
        "cap_hits": {"wall_time_decided": cap_wall, "work": cap_work},
        "engine_failures": {"retried_this_round": inp.engine_retried, "attention": inp.engine_attention},
        "second_route": second_v,
        "descent": descent_v,
    });
    let mut check_v = check_v;
    check_v["escalation"] = esc_summary(esc_out.as_ref(), uncovered.len());
    // lane layers (closeloop.md 23): per layer frozen / walked, verdict, leaves met, relative to which leaf sets
    if let Some(lp) = &layer_plan {
        check_v["layers"] =
            super::layers::layers_json(lp, layer_walked.as_deref().unwrap_or(&[]));
    }
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
    // lane closeloop4 (closeloop.md 19): an uncovered key that nothing more can serve is a terminal PROPOSAL; it becomes
    // a terminal only by a decision (operator, or `auto` within `max_auto`); until then the node has no final verdict
    let mut terminal_proposals: Option<Value> = None;
    let mut terminal_decision: Option<TerminalDecision> = None;
    if !closed && batches.is_empty() && proof_batches.is_empty() && !budget_stop && !progress {
        let rejected: BTreeSet<Vec<i32>> = inp
            .terminal_decisions
            .iter()
            .flat_map(|d| d.reject.iter().cloned())
            .collect();
        let is_uncovered = |r: &KeyRow| {
            r.status == RowStatus::Open && r.open_status.as_deref() == Some("uncovered")
        };
        let not_cert: Vec<&KeyRow> = res
            .rows
            .iter()
            .filter(|r| !row_certified(r, required, basis_ok))
            .collect();
        let only_uncovered = !not_cert.is_empty()
            && not_cert.iter().all(|r| is_uncovered(r))
            && blocking
                .iter()
                .all(|b| b.starts_with("open_keys:") || b.starts_with("import_open_keys:"));
        let proposals: Vec<Vec<i32>> = not_cert
            .iter()
            .filter(|r| is_uncovered(r) && !rejected.contains(&r.key))
            .map(|r| r.key.clone())
            .collect();
        let n_rejected = not_cert
            .iter()
            .filter(|r| rejected.contains(&r.key))
            .count();
        if n_rejected > 0 {
            blocking.push(format!("terminal_proposals_rejected:{n_rejected}"));
        }
        if inp.escalation.is_some() {
            // lane t266run's escalation decided that its exhausted keys need a decision: the same proposals path
            if let Some(doc) = esc_wait.take() {
                let keys: Vec<Vec<i32>> = uncovered
                    .iter()
                    .filter(|k| !rejected.contains(*k))
                    .cloned()
                    .collect();
                if !keys.is_empty() {
                    let mut rec =
                        super::terminals::proposals_record(&inp.node, inp.round, &keys, &res.rows);
                    rec["attempts"] = doc["attempts"].clone();
                    terminal_proposals = Some(rec);
                }
            }
        } else if only_uncovered && !proposals.is_empty() {
            let rec =
                super::terminals::proposals_record(&inp.node, inp.round, &proposals, &res.rows);
            // `max_auto` bounds ALL automatically accepted terminals of the node (earlier rounds included)
            let auto_before: usize = inp
                .terminal_decisions
                .iter()
                .filter(|d| d.accepted_by == "auto")
                .map(|d| d.accept.len())
                .sum();
            let auto_ok = inp.terminals.accept == TerminalAccept::Auto
                && auto_before + proposals.len() <= inp.terminals.max_auto;
            if auto_ok {
                terminal_decision = Some(super::terminals::auto_decision(
                    &inp.node,
                    inp.round,
                    &proposals,
                    inp.terminals.max_auto,
                ));
            } else {
                terminal_proposals = Some(rec);
            }
        }
    }
    let mut final_ = if terminal_proposals.is_some() || terminal_decision.is_some() {
        None
    } else if closed || (batches.is_empty() && proof_batches.is_empty() && !progress) {
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
            res_k.as_ref().filter(|_| tier_c_now && !stream_k),
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
                decisive: &decisive_lines,
                layers: layer_plan
                    .as_ref()
                    .map(|lp| (lp, layer_walked.as_deref().unwrap_or(&[]))),
            },
        )?)
    } else {
        None
    };
    // lane t266run: terminal proposals that were not accepted (operator decision / over the cap)
    if let (Some(f), Some(p)) = (
        final_.as_mut(),
        esc_out.as_ref().and_then(|e| e.proposals.as_ref()),
    ) {
        let path = inp.final_dir.join("terminal_proposals.json");
        std::fs::write(&path, serde_json::to_string_pretty(p).unwrap_or_default())
            .map_err(|e| format!("terminal proposals: {e}"))?;
        let n = p["terminals"].as_array().map(Vec::len).unwrap_or(0);
        f.result["terminal_proposals"] = json!({"file": path.display().to_string(), "count": n,
            "status": p["status"], "keys": p["terminals"]});
        if p["status"] == "awaiting_operator_decision" {
            f.result["awaiting_operator"] = json!(format!(
                "open: awaiting operator decision on {n} terminal proposals ({})",
                path.display()
            ));
        }
    }
    // lane t266run: the face table of the escalated keys (final pass)
    if let (Some(f), Some(_)) = (final_.as_mut(), inp.escalation.as_ref()) {
        let open: BTreeSet<Vec<i32>> = uncovered.iter().cloned().collect();
        let ft = super::derived::face_table(&set, &inp.derived, &ev.solved, &open);
        let p = inp.final_dir.join("faces.json");
        std::fs::write(&p, serde_json::to_string_pretty(&ft).unwrap_or_default())
            .map_err(|e| format!("faces: {e}"))?;
        f.result["faces"] = json!({"file": p.display().to_string(), "faces": ft["faces"],
            "closed_by_one_recipe_or_symmetry": ft["closed_by_one_recipe_or_symmetry"],
            "needed_several_recipes": ft["needed_several_recipes"],
            "with_open_or_terminal_keys": ft["with_open_or_terminal_keys"]});
    }
    let rules_used = res.summary.used_rules;
    let res_tier = res.tier;
    // the union's rows: write_statement ADOPTS the streamed rows file as the statement's statement.jsonl
    let union_rows = res.streamed.as_ref().map(|s| {
        let st = inp.final_dir.join("statement").join("statement.jsonl");
        if st.is_file() {
            st
        } else {
            PathBuf::from(&s.path)
        }
    });
    // lane t266run (closeloop.md 17.4): per-clause verdicts and attribution at the final pass, one closure at a time
    // (the union's results are dropped first)
    if let (Some(f), Some(cl)) = (final_.as_mut(), inp.clauses.as_ref()) {
        drop(streamed_res);
        drop(res_k);
        match union_rows {
            Some(ur) => {
                let v = super::clauses::per_clause(
                    cl,
                    &inp.spec_text,
                    &inp.base,
                    &set,
                    &ev,
                    res_tier,
                    threads,
                    required,
                    basis_ok,
                    &ur,
                    &inp.final_dir.join("clauses"),
                    &inp.final_dir.join("statement"),
                )?;
                // the clause of every open key
                let op = inp.final_dir.join("open_keys.jsonl");
                let text = std::fs::read_to_string(&op).unwrap_or_default();
                let mut lines: Vec<Value> = text
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .filter_map(|l| serde_json::from_str(l).ok())
                    .collect();
                let keys: Vec<Vec<i32>> = lines
                    .iter()
                    .map(|v| serde_json::from_value(v["key"].clone()).unwrap_or_default())
                    .collect();
                let cof =
                    super::clauses::clause_of_keys(cl, &inp.final_dir.join("clauses"), &keys)?;
                let mut out = String::new();
                for (v, c) in lines.iter_mut().zip(cof) {
                    v["clause"] = json!(c);
                    out.push_str(&v.to_string());
                    out.push('\n');
                }
                std::fs::write(&op, out).map_err(|e| format!("open_keys: {e}"))?;
                f.result["clauses"] = v["clauses"].clone();
                f.result["clause_attribution"] = json!({"default_clause": v["default_clause"],
                    "clause_keys_file": v["clause_keys_file"],
                    "union_keys_outside_every_clause": v["union_keys_outside_every_clause"]});
            }
            None => {
                f.result["clauses"] =
                    json!("not computed: the union check kept its rows in memory (no rows file)");
            }
        }
    }
    // lane closeloop4: phase D may still open a node (the reference rule): the node machine needs a defect then
    let defect = if final_.as_ref().is_some_and(|f| f.verdict != "closed") {
        defect.max(1)
    } else {
        defect
    };
    // lane closeloop4: the digests of the tables the new proof batches name, computed HERE (pass thread) once per
    // (path, size, mtime): the supervisor's main thread then finds them in the cache when it creates the items
    let tables: BTreeSet<&str> = proof_batches.iter().map(|b| b.table.as_str()).collect();
    for t in tables {
        super::proofs::file_blake3_cached(Path::new(t))?;
    }
    Ok(PassOut {
        round: inp.round,
        spec_digest: set.report.spec_digest.clone(),
        check: check_v,
        defect,
        closure_keys: keys,
        rules_used,
        rules_total: rules_total.max(rules_used),
        batches,
        proof_batches,
        final_,
        dispatch_order,
        import_manifest,
        seconds: t0.elapsed().as_secs_f64(),
        set,
        terminal_proposals,
        terminal_decision,
        pins,
        escalation: esc_out,
    })
}

/// lane t266run: the escalation numbers of a check record (closeloop.md 17).
fn esc_summary(e: Option<&EscOut>, uncovered: usize) -> Value {
    let Some(e) = e else {
        return Value::Null;
    };
    let proposed = e
        .terminals
        .as_ref()
        .and_then(|t| t["terminals"].as_array().map(Vec::len));
    json!({"uncovered": uncovered, "symmetry_attempts": e.symrel_lines.len(),
        "symmetry_relations": e.symrel_relations, "recipe_keys": e.recipe_manifest.len(),
        "recipe_files": e.recipe_files.len(), "held": e.hold.len(), "terminals_proposed": proposed,
        "over_rule": e.over_rule})
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
    /// lane closeloop4: the decisive-zero table (both sources per zero), written into the statement directory.
    decisive: &'a [Value],
    /// lane layers: the layered view of the pass (the plan, the layers walked in it).
    layers: Option<(&'a super::layers::Plan, &'a [super::layers::Walked])>,
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
    // the recipes' per-key refusals (R48 3b; `RecipeVerdict::Rejected` keeps them, closeloop.md 13.4) by key
    // from our own answers lines: verdict, failure class and message per (recipe, key), else the import's reason
    let mut refusals: BTreeMap<Vec<i32>, Vec<Value>> = BTreeMap::new();
    for (p, _) in &inp.answers {
        for l in std::fs::read_to_string(p).unwrap_or_default().lines() {
            let Ok(v) = serde_json::from_str::<Value>(l) else {
                continue;
            };
            if v["verdict"] == "SOLVED" {
                continue;
            }
            if let Ok(k) = serde_json::from_value::<Vec<i32>>(v["key"].clone()) {
                refusals.entry(k).or_default().push(json!({"recipe": v["source"],
                    "verdict": v["verdict"], "class": v["failure"]["class"],
                    "reason": v["failure"]["message"], "reproduce": v["failure_record"]["reproduce"]}));
            }
        }
    }
    for s in &res.solve_refusals {
        let e = refusals.entry(s.key.clone()).or_default();
        if !e.iter().any(|x| x["recipe"] == s.source.as_str()) {
            e.push(json!({"recipe": s.source, "reason": s.reason}));
        }
    }
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
            "recipe_refusals": refusals.get(&r.key).cloned().unwrap_or_default(),
            "regularity_unknown": r.regularity_unknown, "coefficient_unknown": r.coefficient_unknown,
            // lane closeloop3: the key's solves failed in the engine (ERROR / IRREGULAR) until attention
            "engine_failure": inp.engine_attention.iter().filter(|e| e["key"] == json!(r.key)).cloned().collect::<Vec<_>>()});
        open_lines.push_str(&v.to_string());
        open_lines.push('\n');
    }
    std::fs::write(inp.final_dir.join("open_keys.jsonl"), &open_lines)
        .map_err(|e| format!("open_keys: {e}"))?;
    let sdir = inp.final_dir.join("statement");
    let mut header = json!({"check_pass_s": f.elapsed, "closeloop": {"target": inp.target,
        "relation_rule": inp.relation_rule, "required_evidence": f.required.word(),
        "tier": format!("{:?}", res.tier), "verdict": f.verdict, "box_clause": inp.box_clause,
        "fact_basis": f.check["fact_basis"].clone(),
        "descent": {"non_descending_edges": f.check["descent"]["non_descending_edges"].clone(),
                    "cycles": f.check["descent"]["cycles"].clone()}}});
    let compare_into = |r: &CheckResult, dir: &str| -> Result<(Value, Vec<Vec<i32>>), String> {
        let Some(h) = &inp.hand else {
            return Ok((Value::Null, vec![]));
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
        let only_hand: Vec<Vec<i32>> = c
            .differences
            .iter()
            .filter(|d| d.kind == rustred_campaign_core::import::DiffKind::OnlyHand)
            .map(|d| d.key.clone())
            .collect();
        Ok((v, only_hand))
    };
    let (mut comparison, only_hand) = compare_into(res, "comparison")?;
    // the keys-only superset check keeps every row: its comparison lists the server of EVERY key
    let comparison_k = match keys_only {
        Some(k) => compare_into(k, "comparison-keys-only")?.0,
        None => Value::Null,
    };
    // lane closeloop4 (audit-closeloop M3): a CLOSED node whose reference (hand statement) has keys ours lacks must
    // explain each: it is in the keys-only closure (reached there only through zero edges, i.e. through DECISIVE zeros,
    // every one confirmed by two routes: else the node is not closed anyway); an unexplained key, or an incomplete
    // comparison, makes the node open under `reference.mismatch = block` (default)
    let mut ref_block: Vec<String> = vec![];
    if f.verdict == "closed" && inp.hand.is_some() {
        let complete = comparison["complete"] != false;
        let in_k = |k: &Vec<i32>| {
            (match keys_only {
                Some(kr) => match &kr.key_set {
                    Some(ks) => ks.contains(k),
                    None => kr.rows.iter().any(|r| &r.key == k),
                },
                None => false,
            })
            // lane layers: the keys-only closures of the passes that froze layers (their walks stopped there now)
            || f.layers.is_some_and(|(lp, _)| {
                lp.frozen
                    .values()
                    .any(|z| z.keys_only.as_ref().is_some_and(|ks| ks.contains(k)))
            })
        };
        let unexplained: Vec<&Vec<i32>> = only_hand.iter().filter(|k| !in_k(k)).collect();
        comparison["reference_check"] = json!({"policy": inp.reference_mismatch, "only_hand": only_hand.len(),
            "explained_by_decisive_zeros": only_hand.len() - unexplained.len(), "unexplained": unexplained,
            "complete": complete});
        if inp.reference_mismatch == "block" {
            if !unexplained.is_empty() {
                ref_block.push(format!(
                    "reference_mismatch_unexplained:{}",
                    unexplained.len()
                ));
            }
            if !complete {
                ref_block.push("reference_comparison_incomplete".into());
            }
        }
    }
    let mut blocking_now: Vec<String> = f.blocking.to_vec();
    let mut verdict_now: &str = f.verdict;
    if !ref_block.is_empty() {
        blocking_now.extend(ref_block);
        verdict_now = "open";
        header["closeloop"]["verdict"] = json!("open");
    }
    // lane closeloop4 (audit-closeloop m7): an earlier box clause that is not closed keeps the node open (decided
    // before the statement header is written)
    let prior_open: Vec<String> = inp
        .prior_clauses
        .iter()
        .filter(|(_, p)| {
            std::fs::read(p)
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                .is_none_or(|v| v["verdict"] != "closed")
        })
        .map(|(id, _)| id.clone())
        .collect();
    if verdict_now == "closed" && !prior_open.is_empty() {
        verdict_now = "open";
        blocking_now.push(format!("prior_clause_not_closed:{}", prior_open.join(",")));
        header["closeloop"]["verdict"] = json!("open");
    }
    // lane closeloop4 (audit-closeloop M2): the statement writer ADOPTS the streamed rows file: it is written after the
    // comparisons, which read every row of it
    let dig = write_statement(set, ev, res, &sdir, Some(header))
        .map_err(|e| format!("statement: {e}"))?;
    // lane closeloop4 (closeloop.md 18.4): the decisive zeros with BOTH sources, in the statement's evidence
    let mut dz = String::new();
    for v in f.decisive {
        dz.push_str(&v.to_string());
        dz.push('\n');
    }
    std::fs::write(sdir.join("decisive_zeros.jsonl"), &dz)
        .map_err(|e| format!("decisive_zeros: {e}"))?;
    // lane closeloop4 (closeloop.md 19.3): who accepted each terminal of the closure
    let terminals_accepted = super::terminals::accepted_terminals(
        set,
        res,
        &inp.terminal_decisions,
        &inp.derived.terminals,
        inp.escalation
            .as_ref()
            .and_then(|e| e.terminals.as_ref())
            .map(|t| t.approved.as_slice())
            .unwrap_or(&[]),
    );
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
    // lane closeloop3 (closeloop.md 16.9): the CLAUSE view: every start key of this run is a key of its box clause;
    // per key what serves it (from the spilled rows = the statement), class x level of the clause's keys, and a verdict
    // per clause (the earlier clauses' from their recorded results)
    let mut clause_keys: Vec<Value> = vec![];
    let mut clause_by_class: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    let mut clause_open = 0u64;
    // the statement just written holds every row (the spilled rows file is adopted by the statement writer)
    let stmt_rows = sdir.join("statement.jsonl");
    let rows_src = if stmt_rows.is_file() {
        std::fs::read_to_string(&stmt_rows).unwrap_or_default()
    } else if inp.rows_path.is_file() {
        std::fs::read_to_string(&inp.rows_path).unwrap_or_default()
    } else {
        String::new()
    };
    let mut from_rows = false;
    for l in rows_src.lines() {
        let Ok(v) = serde_json::from_str::<Value>(l) else {
            continue;
        };
        if v["in_start"] != true {
            continue;
        }
        from_rows = true;
        let class = v["server"]["class"]
            .as_str()
            .unwrap_or(if v["status"] == "terminal" {
                "terminal"
            } else {
                "-"
            })
            .to_string();
        let level = match v["status"].as_str() {
            Some("served") => v["evidence"].as_str().unwrap_or("none").to_string(),
            Some("terminal") => "terminal".into(),
            _ => "open".into(),
        };
        if level == "open" || (level != "terminal" && level != f.required.word()) {
            clause_open += 1;
        }
        *clause_by_class
            .entry(class.clone())
            .or_default()
            .entry(level.clone())
            .or_default() += 1;
        clause_keys.push(json!({"key": v["key"], "clause": inp.box_clause, "status": v["status"], "class": class,
            "level": level, "basis": v["basis"], "fact_basis": v["facts"]["basis"], "server": v["server"]["hand_name"],
            "open": v["open"]}));
    }
    if !from_rows {
        for r in res.rows.iter().filter(|r| r.in_start) {
            let class = class_of(set, r);
            let level = match r.status {
                RowStatus::Served => r.evidence.map(|e| e.word()).unwrap_or("none").to_string(),
                RowStatus::Terminal => "terminal".into(),
                RowStatus::Open => "open".into(),
            };
            if level == "open" || (level != "terminal" && level != f.required.word()) {
                clause_open += 1;
            }
            *clause_by_class
                .entry(class.clone())
                .or_default()
                .entry(level.clone())
                .or_default() += 1;
            clause_keys.push(
                json!({"key": r.key, "clause": inp.box_clause, "status": r.status, "class": class,
                "level": level, "basis": r.basis, "fact_basis": r.fact_basis}),
            );
        }
    }
    let clause_word = |verdict: &str, open: u64| -> String {
        if verdict == "closed" {
            "closed".into()
        } else {
            format!("{verdict} ({open})")
        }
    };
    let mut clauses = serde_json::Map::new();
    for (id, p) in &inp.prior_clauses {
        let pv: Value = std::fs::read(p)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or(Value::Null);
        let w = pv["verdict"].as_str().unwrap_or("unknown");
        let n = pv["open_keys"]["count"].as_u64().unwrap_or(0);
        clauses.insert(id.clone(), json!(clause_word(w, n)));
    }
    let this_verdict = if verdict_now == "closed" && clause_open == 0 {
        "closed"
    } else if clause_open == 0 {
        verdict_now
    } else {
        "open"
    };
    clauses.insert(
        inp.box_clause.clone(),
        json!(clause_word(this_verdict, clause_open)),
    );
    let mut ck = String::new();
    for v in &clause_keys {
        ck.push_str(&v.to_string());
        ck.push('\n');
    }
    std::fs::write(inp.final_dir.join("clause_keys.jsonl"), ck)
        .map_err(|e| format!("clause_keys: {e}"))?;
    // lane layers (closure.md 18): a walk that met prior clauses' leaf sets is closed RELATIVE to them
    let relative_to: Vec<String> = res
        .leaves
        .as_ref()
        .map(|h| h.summary.relative_to.clone())
        .unwrap_or_default();
    let label = if verdict_now == "closed" && !relative_to.is_empty() {
        format!(
            "closed, {}, {}, relative to leaf set(s) {}",
            inp.target,
            f.required.word(),
            relative_to.join(", ")
        )
    } else if verdict_now == "closed" {
        format!("closed, {}, {}", inp.target, f.required.word())
    } else {
        verdict_now.to_string()
    };
    let result = json!({
        "schema": RESULT_SCHEMA, "node": inp.node, "verdict": verdict_now, "label": label,
        "target": inp.target, "level": f.required.word(), "rounds": inp.round,
        "blockers": &blocking_now, "budget_stop": f.budget_why,
        "open_keys": {"file": "open_keys.jsonl", "count": open_n},
        "check": f.check,
        "statement": {"dir": sdir.display().to_string(), "files": dig.files},
        "comparison": comparison, "comparison_keys_only": comparison_k,
        "economy": {"closure_keys": f.check["keys"].clone(), "used_rules": res.summary.used_rules,
                    "recipes_used": served_by_recipe.len(), "keys_by_recipe": served_by_recipe,
                    "supplied_sets_used": served_by_supplied, "solved_keys": solved_keys, "failed_keys": failed_keys,
                    "answers_by_recipe": answered, "relation_rule": inp.relation_rule,
                    "cap_hits": f.check["cap_hits"].clone()},
        "evidence_by_class": f.check["evidence_by_class"].clone(),
        "by_rule_class_level": f.check["by_rule_class_level"].clone(),
        // lane closeloop3: who produced the facts of the served keys (rr_proof / exact_eval / compact)
        "fact_basis": f.check["fact_basis"].clone(),
        "relation_rule": inp.relation_rule,
        "requests_left": {"solve": res.solve_requests.len(), "fact_records": res.fact_requests.len(),
                          "proof_records": res.proof_requests.len()},
        "writes": "CDIR/closeloop only (never P5L_STATUS.json / P5L_TERMINALS.json)",
        // lane closeloop3 (closeloop.md 16.9): a verdict per box clause; this clause's keys by class x level
        "box_clause": inp.box_clause, "clauses": clauses,
        "clause_keys": {"file": "clause_keys.jsonl", "count": clause_keys.len(), "not_certified": clause_open,
                        "by_rule_class_level": clause_by_class},
        // lane closeloop4 (closeloop.md 18 / 19): the second route of the zero test and who accepted each terminal
        "second_route": f.check["second_route"].clone(),
        "descent": f.check["descent"].clone(),
        "decisive_zeros": {"file": "statement/decisive_zeros.jsonl", "count": f.decisive.len()},
        "terminals_accepted": terminals_accepted,
        "terminal_policy": {"accept": inp.terminals.accept, "max_auto": inp.terminals.max_auto},
        // lane layers (closeloop.md 23)
        "relative_to": relative_to,
        "leaves": res.leaves.as_ref().map(|h| serde_json::to_value(&h.summary).unwrap_or(Value::Null)),
    });
    let mut result = result;
    // lane layers (closeloop.md 23.5): the per-layer view and the node's own LEAF SET (every layer's keys, bound to the
    // composed statement): what a later run (prior clause) or another node takes as leaves
    if let Some((lp, walked)) = f.layers {
        result["layers"] = f.check["layers"].clone();
        if lp.configured {
            let st_sha = dig.files.get("statement.jsonl").cloned().unwrap_or_default();
            let ls = super::layers::node_leafset(
                set,
                ev,
                lp,
                walked,
                &st_sha,
                verdict_now,
                &inp.node,
                &inp.box_clause,
            )?;
            let lpath = inp.final_dir.join("leafset.v1");
            let fh = std::fs::File::create(&lpath).map_err(|e| format!("{}: {e}", lpath.display()))?;
            ls.write(std::io::BufWriter::new(fh))
                .map_err(|e| format!("{}: {e}", lpath.display()))?;
            let fsha = rustred_campaign_core::import::lines::file_sha256(&lpath)
                .map_err(|e| format!("{}: {e}", lpath.display()))?;
            result["leaf_set"] = json!({"file": "leafset.v1", "path": lpath.display().to_string(),
                "sha256": fsha, "digest": ls.digest(), "keys": ls.len(), "uncertified": ls.n_uncertified(),
                "statement_digest": st_sha, "source_verdict": verdict_now,
                "memory_bytes": ls.memory_bytes()});
        }
    }
    // lane closeloop4 (audit-closeloop m8): every file of the phase-D outputs, with its BLAKE3 (computed here, in the
    // pass thread): registered as ledger records by the source; resume and `rr_campaign verify` re-check them
    let mut files: BTreeMap<String, String> = BTreeMap::new();
    let mut stack = vec![inp.final_dir.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d)
            .map_err(|e| format!("{}: {e}", d.display()))?
            .flatten()
        {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.file_name().is_some_and(|n| n != "result.json")
                && !p.to_string_lossy().contains("terminal_proposals")
            {
                let (_, b) =
                    crate::atomic::blake3_file(&p).map_err(|e| format!("{}: {e}", p.display()))?;
                let rel = p
                    .strip_prefix(&inp.final_dir)
                    .map(|r| r.display().to_string())
                    .unwrap_or_else(|_| p.display().to_string());
                files.insert(rel, b);
            }
        }
    }
    result["final_files_blake3"] = json!(files);
    Ok(FinalOut {
        verdict: verdict_now.to_string(),
        result,
    })
}

/// lane closeloop3: `coefficient-facts` batches of the (record, key) pairs of `missing`: one table per batch, at most
/// `cap` pairs (a record's keys may be split over batches).
fn exact_batches(
    set: &rustred_campaign_core::import::ImportedSet,
    missing: &BTreeMap<(usize, usize), BTreeSet<Vec<i32>>>,
    cap: usize,
) -> Vec<ProofBatch> {
    record_batches(set, missing, cap, "exact", REASON_EXACT)
}

/// Batches of kind `kind` (reason `reason`) of the (record, key) pairs of `missing`: one table per batch, at most `cap`
/// pairs (a record's keys may be split over batches).
fn record_batches(
    set: &rustred_campaign_core::import::ImportedSet,
    missing: &BTreeMap<(usize, usize), BTreeSet<Vec<i32>>>,
    cap: usize,
    kind: &str,
    reason: &str,
) -> Vec<ProofBatch> {
    let mut by_table: BTreeMap<String, Vec<RecordReq>> = BTreeMap::new();
    for ((t, r), ks) in missing {
        let info = &set.rules[*t][*r];
        by_table
            .entry(info.path.clone())
            .or_default()
            .push(RecordReq {
                rule: info.index.unwrap_or(info.label),
                offset: info.offset,
                record_sha256: info.record_sha256.clone(),
                keys: ks.iter().cloned().collect(),
            });
    }
    let mut out = vec![];
    for (table, recs) in by_table {
        let mut cur: Vec<RecordReq> = vec![];
        let mut n = 0usize;
        for rr in recs {
            for chunk in rr.keys.chunks(cap) {
                if n + chunk.len() > cap && !cur.is_empty() {
                    out.push(ProofBatch {
                        kind: kind.into(),
                        reason: reason.into(),
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
            out.push(ProofBatch {
                kind: kind.into(),
                reason: reason.into(),
                table,
                records: cur,
                relation_source: None,
            });
        }
    }
    out
}

/// lane closeloop3: the fact basis counts of the served keys (`facts:basis:<producer>` of the import's counts).
pub fn fact_basis_counts(counts: &BTreeMap<String, u64>) -> BTreeMap<String, u64> {
    counts
        .iter()
        .filter_map(|(k, v)| k.strip_prefix("facts:basis:").map(|b| (b.to_string(), *v)))
        .collect()
}
