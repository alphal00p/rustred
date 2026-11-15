//! lane layers (interfaces/closeloop.md 23; TRICKS item 10, first half): close the box LAYER BY LAYER, every earlier
//! layer serving as LEAVES (interfaces/closure.md 18) of the later walks.
//!
//! - The layers are SPEC DATA (`layers`: an ordered list, or by default the import start set split by the coarse
//!   coordinate D + r ascending, computed from each key and the node's line mask: nothing about a node is known here).
//! - A pass walks, in order, every layer that is not frozen; each walk's leaves are the prior clauses' leaf sets (other
//!   runs, pinned), the frozen layers and the layers walked before it in this pass. Right after its walk a layer is
//!   judged on its OWN keys (every row certified on a strict exact basis, no blocker of its check, its decisive zeros and
//!   elided pairs confirmed by the second route, no non-descending edge, acyclic, its relation proofs bound, every leaf it
//!   reached certified in a closed set): a closed layer is FROZEN at once (leaf set + rows files, pinned by sha256 in a
//!   small record that the source registers in the ledger while the pass still runs), so an interrupt never loses it and
//!   no later pass, round or resumed run walks it again. At recovery the frozen layers come back ONLY from the ledger
//!   (record BLAKE3, then the sha256 of its files, the leaf-set digest, the start digest, and the walk binding in the
//!   closure): never from a file on trust.
//! - The node's statement is COMPOSED: every layer's rows in layer order (frozen rows files verified on the way), with
//!   `in_start` = membership in the node's start set; it equals the statement of one walk of the union.

use super::config::{ClauseCfg, LayersCfg, LeafSetRef};
use super::pass::{PassIn, row_certified};
use super::zeros::{self, SecondRoute, ZeroScan};
use rustred_campaign_core::closure::{
    ClosureSummary, KeySet, KeySetBuilder, LeafBinding, LeafSet, LeafSetBuilder, Tier, Verdict,
};
use rustred_campaign_core::import::check::RecordRequest;
use rustred_campaign_core::import::check::{LeafHits, check_streamed_with_leaves};
use rustred_campaign_core::import::load::read_start;
use rustred_campaign_core::import::{
    CheckOptions, CheckResult, Evidence, ImportedSet, KeyRow, Level, RowStatus, TableSetSpec,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::Sender;

pub const LAYER_RECORD_SCHEMA: &str = "rustred.campaign.closeloop-layer.v1";
pub const ROLE_LAYER: &str = "closeloop_layer";

/// Ledger input id of a frozen layer.
pub fn input_layer(n: &str, idx: usize) -> String {
    format!("closeloop:{n}:layer:{idx}")
}

/// A message of the pass thread to the source: a layer was frozen (register its record now).
#[derive(Clone, Debug)]
pub struct LayerMsg {
    pub idx: usize,
    pub body: Vec<u8>,
}

/// The record of a frozen layer (ledger input, role `closeloop_layer`): it pins every file of the layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayerRecord {
    pub schema: String,
    pub node: String,
    pub idx: usize,
    pub id: String,
    pub round: u32,
    pub start_digest: String,
    pub start_keys: u64,
    pub verdict: String,
    pub keys: u64,
    pub certified: u64,
    /// {path, sha256, digest}
    pub leaf_set: FileDigest,
    /// the layer's statement rows (canonical lines)
    pub rows: FileDigest,
    /// the keys-only closure of the pass that froze it (reference rule: keys reached only through zero edges)
    pub keys_only: Option<FileDigest>,
    /// aggregates of the layer's rows (counts, rule class x level)
    pub counts: BTreeMap<String, u64>,
    pub by_class: BTreeMap<String, BTreeMap<String, u64>>,
    /// declared terminals among the layer's keys (key, in_start)
    pub terminals: Vec<(Vec<i32>, bool)>,
    /// leaf sets this layer was closed relative to (digests)
    pub relative_to: Vec<String>,
    pub seconds: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileDigest {
    pub path: String,
    pub sha256: String,
    #[serde(default)]
    pub digest: Option<String>,
}

/// One layer of the box.
#[derive(Clone, Debug)]
pub struct Layer {
    pub idx: usize,
    pub id: String,
    pub start: KeySet,
}

/// A frozen layer (from the ledger, verified).
#[derive(Clone, Debug)]
pub struct Frozen {
    pub rec: LayerRecord,
    pub leaves: Arc<LeafSet>,
    pub keys_only: Option<KeySet>,
}

/// The layered view of one pass.
#[derive(Debug)]
pub struct Plan {
    pub layers: Vec<Layer>,
    pub frozen: BTreeMap<usize, Frozen>,
    /// prior clauses' leaf sets (other runs; verified), with their ids
    pub external: Vec<(String, Arc<LeafSet>)>,
    pub freeze: bool,
    /// whether the node has a `layers` configuration (else one layer = the start set, external leaves only)
    pub configured: bool,
    pub node_dir: PathBuf,
}

fn sha256_file(p: &Path) -> Result<String, String> {
    rustred_campaign_core::import::lines::file_sha256(p)
        .map_err(|e| format!("{}: {e}", p.display()))
}

fn strip(d: &str) -> String {
    d.trim_start_matches("sha256:").to_ascii_lowercase()
}

/// The coarse coordinate D + r of a key (support = line slots with power >= 1).
pub fn coarse(lines: &[bool], k: &[i32]) -> i64 {
    let mut d = 0i64;
    let mut r = 0i64;
    for (&x, &l) in k.iter().zip(lines) {
        if l && x >= 1 {
            d += (x - 1) as i64;
        } else {
            r += x.unsigned_abs() as i64;
        }
    }
    d + r
}

/// The layers of the box (closeloop.md 23.1).
pub fn layer_starts(
    cfg: &LayersCfg,
    set: &ImportedSet,
    spec_text: &str,
    base: &Path,
) -> Result<Vec<Layer>, String> {
    let start = &set.start;
    let mut out: Vec<Layer> = vec![];
    match cfg.order.as_str() {
        "coarse" => {
            let lines = set.frame.mask().to_vec();
            let mut by: BTreeMap<i64, KeySetBuilder> = BTreeMap::new();
            for k in start.iter() {
                by.entry(coarse(&lines, &k))
                    .or_insert_with(|| KeySetBuilder::new(set.codec))
                    .push(&k)
                    .map_err(|e| format!("layer key {k:?}: {e:?}"))?;
            }
            for (i, (c, b)) in by.into_iter().enumerate() {
                out.push(Layer {
                    idx: i,
                    id: format!("D+r={c}"),
                    start: b.finish(),
                });
            }
        }
        "list" => {
            if cfg.list.is_empty() {
                return Err("layers: order `list` with an empty list".into());
            }
            let mut seen = KeySet::empty(set.codec);
            for (i, c) in cfg.list.iter().enumerate() {
                let ks = clause_start(c, spec_text, base, set)?;
                let own = ks.difference(&seen);
                seen = seen.union(&own);
                out.push(Layer {
                    idx: i,
                    id: c.id.clone(),
                    start: own,
                });
            }
            let missing = start.difference(&seen);
            if !missing.is_empty() {
                return Err(format!(
                    "layers: {} key(s) of the import start set are in no layer (e.g. {:?})",
                    missing.len(),
                    missing.get(0)
                ));
            }
            let extra = seen.difference(start);
            if !extra.is_empty() {
                return Err(format!(
                    "layers: {} layer key(s) are not in the import start set (e.g. {:?})",
                    extra.len(),
                    extra.get(0)
                ));
            }
        }
        o => return Err(format!("layers: unknown order {o} (coarse | list)")),
    }
    Ok(out)
}

fn clause_start(
    c: &ClauseCfg,
    spec_text: &str,
    base: &Path,
    set: &ImportedSet,
) -> Result<KeySet, String> {
    let mut spec = TableSetSpec::from_json(spec_text).map_err(|e| format!("import spec: {e}"))?;
    spec.start = serde_json::from_value(c.start.clone())
        .map_err(|e| format!("layer {}: start: {e}", c.id))?;
    read_start(&spec, base, set.codec).map_err(|e| format!("layer {}: {e}", c.id))
}

/// A leaf set of another run: file sha256 pinned, digest verified by the reader, statement digest bound when given.
pub fn load_leaf_ref(r: &LeafSetRef, base: &Path) -> Result<LeafSet, String> {
    let p = rustred_campaign_core::import::spec::resolve(base, &r.path.display().to_string());
    let d = sha256_file(&p)?;
    if d != strip(&r.sha256) {
        return Err(format!(
            "leaf set {}: sha256 {d} differs from the pinned {}: refused",
            p.display(),
            r.sha256
        ));
    }
    let s = LeafSet::read_path(&p).map_err(|e| format!("leaf set {}: {e}", p.display()))?;
    if let Some(st) = &r.statement {
        let sp = rustred_campaign_core::import::spec::resolve(base, &st.display().to_string());
        let sd = sha256_file(&sp)?;
        if sd != strip(&s.binding().statement_digest) {
            return Err(format!(
                "leaf set {}: its statement digest {} is not the sha256 {sd} of {}: refused",
                p.display(),
                s.binding().statement_digest,
                sp.display()
            ));
        }
    }
    Ok(s)
}

/// The layered plan of a pass, or None (no layers configured and no prior leaf set: one walk, as before).
pub fn plan(inp: &PassIn, set: &ImportedSet) -> Result<Option<Plan>, String> {
    if inp.layers.is_none() && inp.prior_leaf_sets.is_empty() {
        return Ok(None);
    }
    let node_dir = inp
        .final_dir
        .parent()
        .unwrap_or_else(|| Path::new("/"))
        .to_path_buf();
    let mut external = vec![];
    for (id, r) in &inp.prior_leaf_sets {
        let s = load_leaf_ref(r, &inp.base).map_err(|e| format!("prior clause {id}: {e}"))?;
        if s.codec().arity() != set.arity() {
            return Err(format!("prior clause {id}: leaf set of another arity"));
        }
        external.push((id.clone(), Arc::new(s)));
    }
    let (layers, configured, freeze) = match &inp.layers {
        Some(c) => (
            layer_starts(c, set, &inp.spec_text, &inp.base)?,
            true,
            c.freeze,
        ),
        None => (
            vec![Layer {
                idx: 0,
                id: inp.box_clause.clone(),
                start: set.start.clone(),
            }],
            false,
            false,
        ),
    };
    let mut frozen = BTreeMap::new();
    for (idx, body) in &inp.frozen_layers {
        let rec: LayerRecord =
            serde_json::from_slice(body).map_err(|e| format!("frozen layer {idx}: record: {e}"))?;
        if rec.schema != LAYER_RECORD_SCHEMA || rec.idx != *idx {
            return Err(format!("frozen layer {idx}: record schema / index"));
        }
        let l = layers
            .get(*idx)
            .ok_or_else(|| format!("frozen layer {idx}: no such layer in the configuration"))?;
        if l.start.digest() != rec.start_digest || l.id != rec.id {
            return Err(format!(
                "frozen layer {idx} ({}): its start set {} is not the configured layer's {} ({}): refused",
                rec.id,
                rec.start_digest,
                l.start.digest(),
                l.id
            ));
        }
        for f in [&rec.leaf_set, &rec.rows] {
            let d = sha256_file(Path::new(&f.path))?;
            if d != f.sha256 {
                return Err(format!(
                    "frozen layer {idx}: {} changed since it was recorded (sha256 {d}, recorded {}): refused",
                    f.path, f.sha256
                ));
            }
        }
        let ls = LeafSet::read_path(Path::new(&rec.leaf_set.path))
            .map_err(|e| format!("frozen layer {idx}: {e}"))?;
        if Some(ls.digest().to_string()) != rec.leaf_set.digest
            || strip(&ls.binding().statement_digest) != rec.rows.sha256
            || !ls.is_closed()
        {
            return Err(format!(
                "frozen layer {idx}: leaf set digest / statement binding / verdict differ from the record: refused"
            ));
        }
        let keys_only = match &rec.keys_only {
            Some(f) => {
                let d = sha256_file(Path::new(&f.path))?;
                if d != f.sha256 {
                    return Err(format!(
                        "frozen layer {idx}: keys-only set {} changed: refused",
                        f.path
                    ));
                }
                let r = std::fs::File::open(&f.path).map_err(|e| format!("{}: {e}", f.path))?;
                Some(
                    KeySet::read(std::io::BufReader::new(r))
                        .map_err(|e| format!("{}: {e:?}", f.path))?,
                )
            }
            None => None,
        };
        frozen.insert(
            *idx,
            Frozen {
                rec,
                leaves: Arc::new(ls),
                keys_only,
            },
        );
    }
    Ok(Some(Plan {
        layers,
        frozen,
        external,
        freeze,
        configured,
        node_dir,
    }))
}

impl Plan {
    /// The leaves every walk of this pass starts with: prior clauses' leaf sets, then the frozen layers (layer order).
    pub fn base_leaves(&self) -> Vec<Arc<LeafSet>> {
        self.external
            .iter()
            .map(|(_, s)| s.clone())
            .chain(self.frozen.values().map(|f| f.leaves.clone()))
            .collect()
    }
    /// The start keys of the layers that are not frozen.
    pub fn open_start(&self) -> KeySet {
        let mut u: Option<KeySet> = None;
        for l in &self.layers {
            if self.frozen.contains_key(&l.idx) {
                continue;
            }
            u = Some(match u {
                None => l.start.clone(),
                Some(x) => x.union(&l.start),
            });
        }
        u.unwrap_or_else(|| {
            KeySet::empty(
                self.layers
                    .first()
                    .map(|l| *l.start.codec())
                    .expect("at least one layer"),
            )
        })
    }
}

/// The keys-only pass (Tier K) of a layered node: ONE walk from the starts of the layers that are not frozen, the prior
/// clauses' leaf sets and the frozen layers as leaves (their facts are all in).
pub fn check_k(
    set: &ImportedSet,
    ev: &Evidence,
    opts: &CheckOptions,
    stream_to: Option<&Path>,
    plan: &Plan,
) -> Result<CheckResult, String> {
    let start = plan.open_start();
    let leaves = plan.base_leaves();
    match stream_to {
        Some(p) => check_streamed_with_leaves(set, ev, opts, p, &start, &leaves),
        None => rustred_campaign_core::import::check::check_from_with_leaves(
            set, ev, opts, &start, &leaves,
        ),
    }
    .map_err(|e| format!("keys-only check (layers): {e}"))
}

/// What the per-layer judgement needs from the pass.
pub struct Gates<'a> {
    pub required: Level,
    pub strict: bool,
    pub second: &'a SecondRoute,
    pub second_decisive: bool,
    pub second_elided: bool,
    /// keys of our relation point records without their bound per-key proof
    pub unproved_relation_keys: &'a BTreeSet<Vec<i32>>,
    pub threads: usize,
}

/// One layer walked in this pass.
pub struct Walked {
    pub idx: usize,
    pub id: String,
    pub res: CheckResult,
    pub leaves: Arc<LeafSet>,
    pub scan: ZeroScan,
    pub rel_edges: u64,
    pub rel_bad: Vec<(Vec<i32>, String, Vec<i32>)>,
    pub closed: bool,
    pub why: Vec<String>,
    pub keys: u64,
    pub certified: u64,
    pub frozen_now: Option<LayerRecord>,
    pub rows_path: PathBuf,
}

/// Per-key class / level words of a statement row (as the clause view words them).
fn row_words(v: &Value) -> (String, String) {
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
    (class, level)
}

/// Is a statement row certified (closeloop.md 8 / 13.2; the JSON form of `row_certified`)?
fn row_json_certified(v: &Value, required: Level, basis_ok: bool) -> bool {
    match v["status"].as_str() {
        Some("terminal") => true,
        Some("served") => {
            basis_ok
                && serde_json::from_value::<Level>(v["evidence"].clone())
                    .ok()
                    .is_some_and(|e| e >= required)
                && v["facts"]["regularity_unknown"] != true
                && v["facts"]["coefficient_unknown"] != true
        }
        _ => false,
    }
}

/// The leaf set of a checked layer, from its statement rows (`rows_path`, canonical lines) and the check's binding.
#[allow(clippy::too_many_arguments)]
pub fn leafset_of(
    set: &ImportedSet,
    res: &CheckResult,
    rows_path: &Path,
    required: Level,
    basis_ok: bool,
    node: &str,
    clause: &str,
    verdict: &str,
    start_digest: &str,
) -> Result<LeafSet, String> {
    let mut b = LeafSetBuilder::new(set.codec, vec![], vec![]);
    let f = std::fs::File::open(rows_path).map_err(|e| format!("{}: {e}", rows_path.display()))?;
    for l in std::io::BufReader::with_capacity(1 << 22, f).lines() {
        let l = l.map_err(|e| e.to_string())?;
        if l.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(&l).map_err(|e| e.to_string())?;
        let key: Vec<i32> = serde_json::from_value(v["key"].clone()).map_err(|e| e.to_string())?;
        let (class, level) = row_words(&v);
        b.push(
            &key,
            &class,
            &level,
            row_json_certified(&v, required, basis_ok),
        )
        .map_err(|e| e.to_string())?;
    }
    for im in &res.credit_images {
        b.aux(im).map_err(|e| e.to_string())?;
    }
    b.finish(LeafBinding {
        statement_digest: res
            .streamed
            .as_ref()
            .map(|s| s.sha256.clone())
            .unwrap_or_default(),
        source_verdict: verdict.into(),
        node: node.into(),
        clause: clause.into(),
        start_digest: start_digest.into(),
        walk: res.leaf_walk.clone(),
        relative_to: res
            .leaves
            .as_ref()
            .map(|h| h.summary.relative_to.clone())
            .unwrap_or_default(),
        provenance: BTreeMap::from([("spec_digest".to_string(), res.spec_digest.clone())]),
    })
    .map_err(|e| e.to_string())
}

/// Certified / total keys of a check (as the node verdict counts them).
fn certified_of(res: &CheckResult, required: Level, basis_ok: bool) -> (u64, u64) {
    let kept = res.rows.len() as u64;
    let keys = res.streamed.as_ref().map(|s| s.rows).unwrap_or(kept);
    let kept_cert = res
        .rows
        .iter()
        .filter(|r| row_certified(r, required, basis_ok))
        .count() as u64;
    let plain = if basis_ok && required <= res.required {
        keys - kept
    } else {
        0
    };
    (kept_cert + plain, keys)
}

/// Walk the layers that are not frozen (Tier C, in order), judge and freeze each at once (closeloop.md 23.2).
#[allow(clippy::too_many_arguments)]
pub fn walk_c(
    set: &ImportedSet,
    ev: &Evidence,
    inp: &PassIn,
    plan: &Plan,
    gates: &Gates,
    keys_only: Option<&KeySet>,
    tx: Option<&Sender<LayerMsg>>,
) -> Result<Vec<Walked>, String> {
    let opts = CheckOptions {
        threads: gates.threads,
        tier: Some(Tier::C),
        max_rounds: None,
    };
    let dir = plan
        .node_dir
        .join("checks")
        .join(format!("r{}.layers", inp.round));
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut leaves = plan.base_leaves();
    let mut out: Vec<Walked> = vec![];
    let mut k_file: Option<FileDigest> = None;
    for l in &plan.layers {
        if plan.frozen.contains_key(&l.idx) {
            continue;
        }
        let t0 = std::time::Instant::now();
        let rows_path = dir.join(format!("{}.rows.jsonl", l.idx));
        let res = check_streamed_with_leaves(set, ev, &opts, &rows_path, &l.start, &leaves)
            .map_err(|e| format!("layer {} ({}): exact-tier check: {e}", l.idx, l.id))?;
        let diagnostic = res.summary.diagnostic || res.summary.verdict == Verdict::Diagnostic;
        let basis_ok = gates.strict && !diagnostic && res.tier == Tier::C;
        let (certified, keys) = certified_of(&res, gates.required, basis_ok);
        // the layer's own edges: decisive zeros + descent (one scan of its rows), relation descent
        let scan = zeros::scan(set, ev, &res, gates.threads)?;
        let (rel_edges, rel_bad) = zeros::relation_descent(set, ev, &res);
        let mut why: Vec<String> = vec![];
        if certified != keys {
            why.push(format!("uncertified:{}", keys - certified));
        }
        let mut b = res.blocking();
        if gates.required != res.required {
            b.retain(|x| !x.starts_with("below_required_evidence:"));
        }
        why.extend(b);
        if !basis_ok {
            why.push("basis_not_strict_exact".into());
        }
        if !res.fact_requests.is_empty() {
            why.push(format!("fact_requests:{}", res.fact_requests.len()));
        }
        if !res.proof_requests.is_empty() {
            why.push(format!("proof_requests:{}", res.proof_requests.len()));
        }
        if !scan.non_descending.is_empty() || !rel_bad.is_empty() {
            why.push(format!(
                "non_descending:{}",
                scan.non_descending.len() + rel_bad.len()
            ));
        }
        match res.summary.cycles {
            Some(0) => {}
            Some(c) => why.push(format!("cycles:{c}")),
            None => why.push("cycles_not_checked".into()),
        }
        // the second route of the layer's zeros (closeloop.md 18): every pair confirmed, no disagreement at its keys
        let mut pairs: BTreeSet<(String, Vec<i32>)> = BTreeSet::new();
        if gates.second_decisive {
            for z in &scan.decisive {
                pairs.insert((
                    set.rules[z.table][z.record].record_sha256.clone(),
                    z.applied.clone(),
                ));
            }
        }
        if gates.second_elided {
            for (_, applied, t, r) in zeros::served_rule_rows(set, &res)? {
                if set.rules[t][r].witness
                    == rustred_campaign_core::import::load::WitnessKind::Elided
                {
                    pairs.insert((set.rules[t][r].record_sha256.clone(), applied));
                }
            }
        }
        let unconfirmed = pairs
            .iter()
            .filter(|p| !gates.second.compared.get(*p).is_some_and(|c| c.agree()))
            .count();
        if unconfirmed > 0 {
            why.push(format!("second_route_unconfirmed:{unconfirmed}"));
        }
        let member = |k: &[i32]| res.key_set.as_ref().is_some_and(|s| s.contains(k));
        let disputed = gates
            .second
            .disagreements()
            .filter(|c| member(&c.key))
            .count();
        if disputed > 0 {
            why.push(format!("second_route_disagreement:{disputed}"));
        }
        let unproved = gates
            .unproved_relation_keys
            .iter()
            .filter(|k| member(k))
            .count();
        if unproved > 0 {
            why.push(format!("relation_proofs_unbound:{unproved}"));
        }
        let closed = why.is_empty();
        let verdict = if closed { "closed" } else { "open" };
        let ls = Arc::new(leafset_of(
            set,
            &res,
            &rows_path,
            gates.required,
            basis_ok,
            &inp.node,
            &l.id,
            verdict,
            &l.start.digest(),
        )?);
        let mut w = Walked {
            idx: l.idx,
            id: l.id.clone(),
            leaves: ls.clone(),
            scan,
            rel_edges,
            rel_bad,
            closed,
            why,
            keys,
            certified,
            frozen_now: None,
            rows_path: rows_path.clone(),
            res,
        };
        if closed && plan.freeze && plan.configured {
            if k_file.is_none()
                && let Some(k) = keys_only
            {
                let p = plan
                    .node_dir
                    .join("layers")
                    .join(format!("keys-only.r{}.keyset", inp.round));
                std::fs::create_dir_all(p.parent().expect("dir"))
                    .map_err(|e| format!("{}: {e}", p.display()))?;
                let f = std::fs::File::create(&p).map_err(|e| format!("{}: {e}", p.display()))?;
                k.write(std::io::BufWriter::new(f))
                    .map_err(|e| format!("{}: {e}", p.display()))?;
                k_file = Some(FileDigest {
                    path: p.display().to_string(),
                    sha256: sha256_file(&p)?,
                    digest: Some(k.digest()),
                });
            }
            let rec = freeze(
                inp,
                plan,
                l,
                &mut w,
                k_file.clone(),
                t0.elapsed().as_secs_f64(),
            )?;
            if let Some(tx) = tx {
                let body = serde_json::to_vec_pretty(&rec).map_err(|e| e.to_string())?;
                // the source registers it while the pass still runs (an interrupt never loses a closed layer)
                let _ = tx.send(LayerMsg { idx: l.idx, body });
            }
            w.frozen_now = Some(rec);
        }
        // TESTS ONLY: a pause after each walked layer (the interrupt-between-layers test stops the run here)
        if let Some(ms) = std::env::var("RR_LAYERS_TEST_LAYER_DELAY_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
        {
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }
        leaves.push(ls);
        out.push(w);
    }
    Ok(out)
}

/// Move a closed layer's rows into its own directory, write its leaf set, build its record.
fn freeze(
    inp: &PassIn,
    plan: &Plan,
    l: &Layer,
    w: &mut Walked,
    keys_only: Option<FileDigest>,
    seconds: f64,
) -> Result<LayerRecord, String> {
    let d = plan
        .node_dir
        .join("layers")
        .join(format!("{}.r{}", l.idx, inp.round));
    std::fs::create_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
    let rows = d.join("rows.jsonl");
    std::fs::rename(&w.rows_path, &rows)
        .or_else(|_| std::fs::copy(&w.rows_path, &rows).map(|_| ()))
        .map_err(|e| format!("{}: {e}", rows.display()))?;
    w.rows_path = rows.clone();
    if let Some(s) = w.res.streamed.as_mut() {
        s.path = rows.display().to_string();
    }
    let lp = d.join("leafset.v1");
    let f = std::fs::File::create(&lp).map_err(|e| format!("{}: {e}", lp.display()))?;
    w.leaves
        .write(std::io::BufWriter::new(f))
        .map_err(|e| format!("{}: {e}", lp.display()))?;
    let terminals: Vec<(Vec<i32>, bool)> = w
        .res
        .rows
        .iter()
        .filter(|r| r.status == RowStatus::Terminal)
        .map(|r| (r.key.clone(), r.in_start))
        .collect();
    let agg = w.res.aggregates.clone().unwrap_or_default();
    Ok(LayerRecord {
        schema: LAYER_RECORD_SCHEMA.into(),
        node: inp.node.clone(),
        idx: l.idx,
        id: l.id.clone(),
        round: inp.round,
        start_digest: l.start.digest(),
        start_keys: l.start.len() as u64,
        verdict: "closed".into(),
        keys: w.keys,
        certified: w.certified,
        leaf_set: FileDigest {
            path: lp.display().to_string(),
            sha256: sha256_file(&lp)?,
            digest: Some(w.leaves.digest().to_string()),
        },
        rows: FileDigest {
            path: rows.display().to_string(),
            sha256: sha256_file(&rows)?,
            digest: None,
        },
        keys_only,
        counts: agg.counts,
        by_class: agg.by_class,
        terminals,
        relative_to: w.leaves.binding().relative_to.clone(),
        seconds,
    })
}

// ------------------------------------------------------------------------------------------------ composition

const VERDICT_ORDER: [Verdict; 8] = [
    Verdict::Diagnostic,
    Verdict::Failed,
    Verdict::Open,
    Verdict::TerminalInRuleDomain,
    Verdict::SupersetClean,
    Verdict::Undecided,
    Verdict::ClosedProvisional,
    Verdict::Closed,
];

fn worse(a: Verdict, b: Verdict) -> Verdict {
    let r = |v: Verdict| VERDICT_ORDER.iter().position(|x| *x == v).unwrap_or(0);
    if r(a) <= r(b) { a } else { b }
}

/// Merge blocker words: `name:N` counts add up, others are kept once.
fn merge_words(a: &[String], b: &[String]) -> Vec<String> {
    let mut num: BTreeMap<String, u64> = BTreeMap::new();
    let mut other: BTreeSet<String> = BTreeSet::new();
    let mut order: Vec<String> = vec![];
    for w in a.iter().chain(b) {
        match w.rsplit_once(':') {
            Some((p, n)) if n.parse::<u64>().is_ok() && !p.contains(':') => {
                if !num.contains_key(p) {
                    order.push(p.to_string());
                }
                *num.entry(p.to_string()).or_default() += n.parse::<u64>().unwrap_or(0);
            }
            _ => {
                if other.insert(w.clone()) {
                    order.push(w.clone());
                }
            }
        }
    }
    order
        .into_iter()
        .map(|p| match num.get(&p) {
            Some(n) => format!("{p}:{n}"),
            None => p,
        })
        .collect()
}

/// Sum the numbers of two JSON values of the same shape (objects recursively, numbers added, booleans or-ed,
/// string lists merged, everything else kept from `a`).
fn add_json(a: &mut Value, b: &Value) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for (k, v) in y {
                match x.get_mut(k) {
                    Some(av) => add_json(av, v),
                    None => {
                        x.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (Value::Number(x), Value::Number(y)) => {
            if let (Some(p), Some(q)) = (x.as_u64(), y.as_u64()) {
                *x = (p + q).into();
            } else if let (Some(p), Some(q)) = (x.as_f64(), y.as_f64())
                && let Some(n) = serde_json::Number::from_f64(p + q)
            {
                *x = n;
            }
        }
        (Value::Bool(x), Value::Bool(y)) => *x = *x || *y,
        (av @ Value::Null, v) => *av = v.clone(),
        (Value::Array(x), Value::Array(y)) => {
            if x.iter().chain(y).all(Value::is_string) {
                let xs: Vec<String> = x
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                let ys: Vec<String> = y
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                *x = merge_words(&xs, &ys).into_iter().map(Value::from).collect();
            }
        }
        _ => {}
    }
}

/// The composed closure summary of several walks (numbers added; verdict the worst; cycles added, unknown if one is).
pub fn merge_summaries(parts: &[&ClosureSummary]) -> Option<ClosureSummary> {
    let first = parts.first()?;
    let mut v = serde_json::to_value(first).ok()?;
    let mut verdict = first.verdict;
    let mut rounds = first.rounds;
    let mut cycles = first.cycles;
    let mut chain = first.longest_chain.clone();
    let mut facts_exact = first.facts_exact;
    let mut leaves = first.leaves.clone();
    for p in &parts[1..] {
        let pv = serde_json::to_value(p).ok()?;
        add_json(&mut v, &pv);
        verdict = worse(verdict, p.verdict);
        rounds = rounds.max(p.rounds);
        cycles = match (cycles, p.cycles) {
            (Some(a), Some(b)) => Some(a + b),
            _ => None,
        };
        if p.longest_chain.as_ref().map(|c| c.edges) > chain.as_ref().map(|c| c.edges) {
            chain = p.longest_chain.clone();
        }
        facts_exact &= p.facts_exact;
        leaves = match (leaves, &p.leaves) {
            (None, x) => x.clone(),
            (Some(a), None) => Some(a),
            (Some(mut a), Some(b)) => {
                a.reached += b.reached;
                a.in_start += b.in_start;
                a.open_reached += b.open_reached;
                for d in &b.relative_to {
                    if !a.relative_to.contains(d) {
                        a.relative_to.push(d.clone());
                    }
                }
                Some(a)
            }
        };
    }
    let mut s: ClosureSummary = serde_json::from_value(v).ok()?;
    s.verdict = verdict;
    s.rounds = rounds;
    s.cycles = cycles;
    s.longest_chain = chain;
    s.facts_exact = facts_exact;
    s.leaves = leaves;
    s.config_digest = first.config_digest.clone();
    s.dispatch_order_digest = first.dispatch_order_digest.clone();
    s.fact_source = first.fact_source.clone();
    s.start_set_digest = first.start_set_digest.clone();
    s.witness_absent = first.witness_absent;
    Some(s)
}

fn merge_requests(into: &mut Vec<RecordRequest>, from: &[RecordRequest]) {
    for r in from {
        match into
            .iter_mut()
            .find(|x| x.record_sha256 == r.record_sha256 && x.table == r.table)
        {
            Some(x) => {
                let mut ks: BTreeSet<Vec<i32>> = x.keys.drain(..).collect();
                ks.extend(r.keys.iter().cloned());
                x.keys = ks.into_iter().collect();
                let mut rs: BTreeSet<String> = x.reasons.drain(..).collect();
                rs.extend(r.reasons.iter().cloned());
                x.reasons = rs.into_iter().collect();
            }
            None => into.push(r.clone()),
        }
    }
}

/// Copy `src` into `w` (hashing), with `in_start` set for keys of `start` that a layer's walk met as leaves (their row
/// lives in the layer that reached them first). Returns the number of lines.
fn append_rows(
    w: &mut impl Write,
    h: &mut rustred_campaign_core::closure::sha256::Sha256,
    src: &Path,
    start: &KeySet,
    expect_sha: Option<&str>,
) -> Result<u64, String> {
    let f = std::fs::File::open(src).map_err(|e| format!("{}: {e}", src.display()))?;
    let mut own = rustred_campaign_core::closure::sha256::Sha256::new();
    let mut n = 0u64;
    for l in std::io::BufReader::with_capacity(1 << 22, f).lines() {
        let l = l.map_err(|e| format!("{}: {e}", src.display()))?;
        own.update(l.as_bytes());
        own.update(b"\n");
        if l.trim().is_empty() {
            continue;
        }
        let mut line = l;
        if line.contains("\"in_start\":false")
            && let Some(k) = key_of_line(&line)
            && start.contains(&k)
        {
            line = line.replacen("\"in_start\":false", "\"in_start\":true", 1);
        }
        w.write_all(line.as_bytes())
            .and_then(|_| w.write_all(b"\n"))
            .map_err(|e| e.to_string())?;
        h.update(line.as_bytes());
        h.update(b"\n");
        n += 1;
    }
    if let Some(e) = expect_sha {
        let got = own.finish_hex();
        if got != e {
            return Err(format!(
                "{}: sha256 {got} differs from the recorded {e}: refused",
                src.display()
            ));
        }
    }
    Ok(n)
}

/// The key of a canonical statement line (`"key":[..]`) without a full parse.
pub fn key_of_line(l: &str) -> Option<Vec<i32>> {
    let i = l.find("\"key\":[")? + 7;
    let j = l[i..].find(']')? + i;
    l[i..j]
        .split(',')
        .map(|x| x.trim().parse::<i32>().ok())
        .collect()
}

/// The node's composed check (closeloop.md 23.3): the walked layers' results + the frozen layers' rows, aggregates and
/// terminal rows; streamed rows file = every layer's rows in layer order at `rows_path`; `leaves` = the prior clauses'
/// leaf sets only (frozen keys are in `key_set`).
pub fn compose(
    set: &ImportedSet,
    plan: &Plan,
    walked: &[Walked],
    rows_path: &Path,
    base: &CheckResult,
) -> Result<CheckResult, String> {
    if let Some(d) = rows_path.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let f =
        std::fs::File::create(rows_path).map_err(|e| format!("{}: {e}", rows_path.display()))?;
    let mut w = std::io::BufWriter::with_capacity(1 << 20, f);
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    let mut n = 0u64;
    let start = &set.start;
    for l in &plan.layers {
        if let Some(fz) = plan.frozen.get(&l.idx) {
            n += append_rows(
                &mut w,
                &mut h,
                Path::new(&fz.rec.rows.path),
                start,
                Some(&fz.rec.rows.sha256),
            )?;
        } else if let Some(x) = walked.iter().find(|x| x.idx == l.idx) {
            n += append_rows(&mut w, &mut h, &x.rows_path, start, None)?;
        }
    }
    w.flush().map_err(|e| e.to_string())?;
    let summaries: Vec<&ClosureSummary> = walked.iter().map(|x| &x.res.summary).collect();
    // no layer walked in this pass (every layer frozen): the summary of the binding check (empty walk, all leaves)
    let mut summary = merge_summaries(&summaries).unwrap_or_else(|| base.summary.clone());
    let mut rows: Vec<KeyRow> = vec![];
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut by_class: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    let mut ks = KeySet::empty(set.codec);
    let mut fact_requests: Vec<RecordRequest> = vec![];
    let mut proof_requests: Vec<RecordRequest> = vec![];
    let mut solve_requests = vec![];
    let mut seconds = 0.0;
    let (mut unpinned, mut unchecked, mut kod) = (false, false, false);
    for x in walked {
        rows.extend(x.res.rows.iter().cloned());
        if let Some(a) = &x.res.aggregates {
            for (k, v) in &a.counts {
                *counts.entry(k.clone()).or_default() += v;
            }
            for (c, m) in &a.by_class {
                let e = by_class.entry(c.clone()).or_default();
                for (k, v) in m {
                    *e.entry(k.clone()).or_default() += v;
                }
            }
        }
        if let Some(k) = &x.res.key_set {
            ks = ks.union(k);
        }
        merge_requests(&mut fact_requests, &x.res.fact_requests);
        merge_requests(&mut proof_requests, &x.res.proof_requests);
        solve_requests.extend(x.res.solve_requests.iter().cloned());
        seconds += x.res.seconds;
        unpinned |= x.res.recipe_binary_unpinned;
        unchecked |= x.res.witness_unchecked;
        kod |= x.res.keys_only_diagnostic;
    }
    for fz in plan.frozen.values() {
        for (k, v) in &fz.rec.counts {
            *counts.entry(k.clone()).or_default() += v;
        }
        for (c, m) in &fz.rec.by_class {
            let e = by_class.entry(c.clone()).or_default();
            for (k, v) in m {
                *e.entry(k.clone()).or_default() += v;
            }
        }
        ks = ks.union(&fz.leaves.keyset());
        for (k, _) in &fz.rec.terminals {
            rows.push(KeyRow {
                key: k.clone(),
                in_start: start.contains(k),
                status: RowStatus::Terminal,
                server: None,
                evidence: None,
                basis: None,
                regularity_unknown: false,
                coefficient_unknown: false,
                fact_basis: None,
                open_status: None,
                open_reason: None,
                open_rule: None,
                image: None,
            });
        }
        summary.keys_total += fz.rec.keys;
        summary.terminals_other += fz.rec.terminals.len() as u64;
    }
    for r in rows.iter_mut() {
        r.in_start = start.contains(&r.key);
    }
    rows.sort_by(|a, b| a.key.cmp(&b.key));
    solve_requests.sort();
    // leaves of the node = the prior clauses' leaf sets (every frozen / walked layer is in `key_set`)
    let n_ext = plan.external.len();
    let leaves = if n_ext == 0 {
        None
    } else {
        let mut s = summary.leaves.clone();
        if let Some(s) = s.as_mut() {
            s.parts.truncate(n_ext);
            s.reached = s.parts.iter().map(|p| p.reached).sum();
            s.in_start = s.parts.iter().map(|p| p.in_start).sum();
            s.open_reached = s.parts.iter().map(|p| p.open_reached).sum();
            let ext: BTreeSet<&str> = plan.external.iter().map(|(_, l)| l.digest()).collect();
            s.relative_to.retain(|d| ext.contains(d.as_str()));
        }
        s.map(|summary| LeafHits {
            summary,
            in_start: vec![],
            open: walked
                .iter()
                .flat_map(|x| x.res.leaves.iter().flat_map(|h| h.open.iter().cloned()))
                .collect(),
            parts: plan.external.iter().map(|(_, l)| l.clone()).collect(),
        })
    };
    let first = Some(walked.first().map(|x| &x.res).unwrap_or(base));
    Ok(CheckResult {
        rows,
        streamed: Some(rustred_campaign_core::import::check::StreamedRows {
            path: rows_path.display().to_string(),
            sha256: h.finish_hex(),
            rows: n,
        }),
        aggregates: Some(rustred_campaign_core::import::check::Aggregates { counts, by_class }),
        key_set_digest: ks.digest(),
        key_set: Some(ks),
        summary,
        tier: Tier::C,
        spec_digest: set.report.spec_digest.clone(),
        solve_requests,
        fact_requests,
        proof_requests,
        solve_refusals: first.map(|r| r.solve_refusals.clone()).unwrap_or_default(),
        recipe_binary_unpinned: unpinned,
        witness_unchecked: unchecked,
        keys_only_diagnostic: kod,
        seconds,
        required: first
            .map(|r| r.required)
            .unwrap_or(set.spec.required_evidence),
        leaves,
        leaf_walk: BTreeMap::new(),
        credit_images: vec![],
    })
}

/// The merged scan of the walked layers (node-level decisive zeros and descent).
pub fn merged_scan(walked: &[Walked]) -> (ZeroScan, u64, Vec<(Vec<i32>, String, Vec<i32>)>) {
    let mut z = ZeroScan::default();
    let mut rel_edges = 0u64;
    let mut rel_bad = vec![];
    for w in walked {
        z.decisive.extend(w.scan.decisive.iter().cloned());
        z.followed_top += w.scan.followed_top;
        z.non_descending
            .extend(w.scan.non_descending.iter().cloned());
        z.dropped_non_descending += w.scan.dropped_non_descending;
        z.keys_scanned += w.scan.keys_scanned;
        z.zero_top += w.scan.zero_top;
        z.zero_positive_numerator += w.scan.zero_positive_numerator;
        z.zero_other += w.scan.zero_other;
        z.seconds += w.scan.seconds;
        rel_edges += w.rel_edges;
        rel_bad.extend(w.rel_bad.iter().cloned());
    }
    (z, rel_edges, rel_bad)
}

/// The per-layer view of a pass (check / result records).
pub fn layers_json(plan: &Plan, walked: &[Walked]) -> Value {
    let mut v = vec![];
    for l in &plan.layers {
        if let Some(fz) = plan.frozen.get(&l.idx) {
            v.push(json!({"idx": l.idx, "id": l.id, "start_keys": l.start.len(), "status": "frozen",
                "frozen_round": fz.rec.round, "verdict": fz.rec.verdict, "keys": fz.rec.keys,
                "certified": fz.rec.certified, "leaf_set": fz.rec.leaf_set, "relative_to": fz.rec.relative_to,
                "label": format!("{}: closed (frozen r{})", l.id, fz.rec.round)}));
        } else if let Some(w) = walked.iter().find(|x| x.idx == l.idx) {
            let h = w.res.leaves.as_ref().map(|h| &h.summary);
            v.push(json!({"idx": l.idx, "id": l.id, "start_keys": l.start.len(),
                "status": if w.frozen_now.is_some() { "walked, frozen" } else { "walked" },
                "verdict": if w.closed { "closed" } else { "open" }, "keys": w.keys, "certified": w.certified,
                "blocking": w.why, "leaves_reached": h.map(|s| s.reached), "leaves_in_start": h.map(|s| s.in_start),
                "leaf_open_keys": w.res.leaves.as_ref().map(|h| h.open.iter().take(100).cloned().collect::<Vec<_>>()),
                "relative_to": h.map(|s| s.relative_to.clone()), "leaf_set_digest": w.leaves.digest(),
                "frozen": w.frozen_now.as_ref().map(|r| &r.leaf_set), "check_s": w.res.seconds,
                "edges": w.res.summary.edges, "rounds": w.res.summary.rounds,
                "label": if w.closed {
                    let rel = h.map(|s| s.relative_to.clone()).unwrap_or_default();
                    if rel.is_empty() { format!("{}: closed", l.id) }
                    else { format!("{}: closed relative to leaf set(s) {}", l.id, rel.join(", ")) }
                } else { format!("{}: open ({})", l.id, w.why.join("; ")) }}));
        }
    }
    json!({"order": plan.layers.iter().map(|l| l.id.clone()).collect::<Vec<_>>(),
           "external": plan.external.iter().map(|(id, l)| json!({"id": id, "digest": l.digest(),
               "keys": l.len(), "source_verdict": l.binding().source_verdict, "clause": l.binding().clause}))
               .collect::<Vec<_>>(),
           "frozen_before": plan.frozen.len(), "walked": walked.len(),
           "frozen_now": walked.iter().filter(|w| w.frozen_now.is_some()).count(),
           "layers": v})
}

/// The leaf set of the whole node (phase D): every layer's leaves (frozen + walked), bound to the composed statement.
#[allow(clippy::too_many_arguments)]
pub fn node_leafset(
    set: &ImportedSet,
    ev: &Evidence,
    plan: &Plan,
    walked: &[Walked],
    statement_sha: &str,
    verdict: &str,
    node: &str,
    clause: &str,
) -> Result<LeafSet, String> {
    let mut b = LeafSetBuilder::new(set.codec, vec![], vec![]);
    let mut aux = KeySetBuilder::new(set.codec);
    let mut parts: Vec<Arc<LeafSet>> = plan.frozen.values().map(|f| f.leaves.clone()).collect();
    parts.extend(walked.iter().map(|w| w.leaves.clone()));
    let mut relative: Vec<String> = vec![];
    let ext: BTreeSet<String> = plan
        .external
        .iter()
        .map(|(_, l)| l.digest().to_string())
        .collect();
    for p in &parts {
        for i in 0..p.len() {
            let e = p.entry(i);
            b.push(&e.key, &e.class, &e.level, e.certified)
                .map_err(|e| e.to_string())?;
        }
        for d in &p.binding().relative_to {
            if ext.contains(d) && !relative.contains(d) {
                relative.push(d.clone());
            }
        }
    }
    for w in walked {
        for im in &w.res.credit_images {
            b.aux(im).map_err(|e| e.to_string())?;
            let _ = aux.push(im);
        }
    }
    let keys: Vec<&Arc<LeafSet>> = parts.iter().collect();
    let aux = aux.finish();
    let member = |k: &[i32]| aux.contains(k) || keys.iter().any(|p| p.contains(k));
    let walk = rustred_campaign_core::import::check::leaf_walk_for(set, ev, Tier::C, &member)
        .map_err(|e| format!("node leaf set: {e}"))?;
    b.finish(LeafBinding {
        statement_digest: statement_sha.into(),
        source_verdict: verdict.into(),
        node: node.into(),
        clause: clause.into(),
        start_digest: set.start.digest(),
        walk,
        relative_to: relative,
        provenance: BTreeMap::from([("spec_digest".to_string(), set.report.spec_digest.clone())]),
    })
    .map_err(|e| e.to_string())
}

// ------------------------------------------------------------------------------------------------ equality

/// The route-free per-key view of a statement (key -> canonical row without `in_start`) and its start keys.
pub fn statement_view(
    path: &Path,
) -> Result<(BTreeMap<Vec<i32>, String>, BTreeSet<Vec<i32>>), String> {
    let f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut m = BTreeMap::new();
    let mut st = BTreeSet::new();
    for l in std::io::BufReader::with_capacity(1 << 22, f).lines() {
        let l = l.map_err(|e| e.to_string())?;
        if l.trim().is_empty() {
            continue;
        }
        let mut v: Value = serde_json::from_str(&l).map_err(|e| e.to_string())?;
        let key: Vec<i32> = serde_json::from_value(v["key"].clone()).map_err(|e| e.to_string())?;
        if v["in_start"] == true {
            st.insert(key.clone());
        }
        if let Some(o) = v.as_object_mut() {
            o.remove("in_start");
        }
        let t = rustred_campaign_core::schema::canon::canonical_json(&v, false)
            .map_err(|e| e.to_string())?;
        if m.insert(key.clone(), t).is_some() {
            return Err(format!("{}: key {key:?} twice", path.display()));
        }
    }
    Ok((m, st))
}

/// The equality of a COMPOSED statement (several statements, each key in exactly one) with the statement of ONE walk
/// (closeloop.md 23.4): identical key sets, identical rows key for key without `in_start`, identical terminals, and the
/// route-free digests (sha256 over the sorted rows without `in_start`) equal.
pub fn compare_composed(single: &Path, parts: &[PathBuf]) -> Result<Value, String> {
    let (one, one_start) = statement_view(single)?;
    let mut comp: BTreeMap<Vec<i32>, String> = BTreeMap::new();
    let mut comp_start: BTreeSet<Vec<i32>> = BTreeSet::new();
    let mut dup = 0u64;
    let mut per_part = vec![];
    for p in parts {
        let (m, st) = statement_view(p)?;
        per_part
            .push(json!({"file": p.display().to_string(), "keys": m.len(), "in_start": st.len()}));
        comp_start.extend(st);
        for (k, v) in m {
            if comp.insert(k, v).is_some() {
                dup += 1;
            }
        }
    }
    let digest = |m: &BTreeMap<Vec<i32>, String>| {
        let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
        for v in m.values() {
            h.update(v.as_bytes());
            h.update(b"\n");
        }
        h.finish_hex()
    };
    let only_one: Vec<&Vec<i32>> = one.keys().filter(|k| !comp.contains_key(*k)).collect();
    let only_comp: Vec<&Vec<i32>> = comp.keys().filter(|k| !one.contains_key(*k)).collect();
    let differ: Vec<&Vec<i32>> = one
        .iter()
        .filter(|(k, v)| comp.get(*k).is_some_and(|c| c != *v))
        .map(|(k, _)| k)
        .collect();
    let term = |m: &BTreeMap<Vec<i32>, String>| -> BTreeSet<Vec<i32>> {
        m.iter()
            .filter(|(_, v)| v.contains("\"status\":\"terminal\""))
            .map(|(k, _)| k.clone())
            .collect()
    };
    let class = |m: &BTreeMap<Vec<i32>, String>| -> BTreeMap<String, u64> {
        let mut c = BTreeMap::new();
        for v in m.values() {
            let j: Value = serde_json::from_str(v).unwrap_or(Value::Null);
            let (cl, lv) = row_words(&j);
            *c.entry(format!("{cl}:{lv}")).or_default() += 1;
        }
        c
    };
    let (d1, d2) = (digest(&one), digest(&comp));
    let equal = only_one.is_empty() && only_comp.is_empty() && differ.is_empty() && dup == 0;
    Ok(json!({
        "equal": equal && d1 == d2,
        "single": {"file": single.display().to_string(), "keys": one.len(), "in_start": one_start.len(),
                   "route_free_digest": d1, "terminals": term(&one).len(), "by_class_level": class(&one)},
        "composed": {"parts": per_part, "keys": comp.len(), "in_start_union": comp_start.len(),
                     "duplicates": dup, "route_free_digest": d2, "terminals": term(&comp).len(),
                     "by_class_level": class(&comp)},
        "key_sets_equal": only_one.is_empty() && only_comp.is_empty(),
        "rows_equal_without_in_start": differ.is_empty(),
        "terminals_equal": term(&one) == term(&comp),
        "start_sets_note": "in_start is route data (the clause a key's start belongs to): compared as sets separately",
        "only_single": only_one.iter().take(20).collect::<Vec<_>>(),
        "only_composed": only_comp.iter().take(20).collect::<Vec<_>>(),
        "differing_rows": differ.iter().take(20).collect::<Vec<_>>(),
        "n_only_single": only_one.len(), "n_only_composed": only_comp.len(), "n_differing": differ.len(),
    }))
}
