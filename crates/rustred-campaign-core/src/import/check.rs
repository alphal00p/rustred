//! `check`: the closure of the start set under the STRICT core dispatcher over the imported set (Tier C where facts
//! exist), classified per key: which keys are served at which evidence level and on what basis, which are blocked and
//! why, and the requests still missing (recipe solves, facts, per-key proofs).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use serde::Serialize;

use super::error::ImportError;
use super::evidence::Evidence;
use super::load::{ImportedSet, WitnessKind};
use super::spec::{Level, TierChoice};
use crate::closure::relations::{RelationSet, RelationStoreBuilder, VERDICT_CERTIFIED};
use crate::closure::{
    ClosureConfig, ClosureSummary, HandDowns, KeySetBuilder, OpenStatus, TerminalSets, Tier,
    WitnessAbsent, closure,
};

/// Options of a check (never part of a result's identity except `tier`).
#[derive(Clone, Debug)]
pub struct CheckOptions {
    pub threads: usize,
    /// Overrides the spec's tier choice.
    pub tier: Option<Tier>,
    pub max_rounds: Option<u32>,
}

impl Default for CheckOptions {
    fn default() -> Self {
        CheckOptions {
            threads: 4,
            tier: None,
            max_rounds: None,
        }
    }
}

/// What serves a key (indices into the set; names are materialised when written).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Server {
    Rule {
        table: usize,
        record: usize,
    },
    /// A relation set (supplied point relations or a recipe's certified relations): its name.
    Relation {
        set: String,
        recipe: bool,
    },
    /// A symmetry credit through the image's rule.
    Credit {
        table: usize,
        record: usize,
    },
}

/// The per-key row.
#[derive(Clone, Debug)]
pub struct KeyRow {
    pub key: Vec<i32>,
    pub in_start: bool,
    pub status: RowStatus,
    pub server: Option<Server>,
    pub evidence: Option<Level>,
    pub basis: Option<&'static str>,
    /// Tier C: the winner's regularity fact at the key is missing / some coefficient fact is missing.
    pub regularity_unknown: bool,
    pub coefficient_unknown: bool,
    /// Open keys: the core status word, the reason and the blocking rule.
    pub open_status: Option<String>,
    pub open_reason: Option<String>,
    pub open_rule: Option<(usize, usize)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RowStatus {
    Served,
    Terminal,
    Open,
}

/// One recipe solve request (interfaces/import.md 6.1).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct SolveRequest {
    pub schema: &'static str,
    pub spec_digest: String,
    pub source: String,
    pub recipe_path: String,
    pub recipe_sha256: String,
    pub engine_digest: Option<String>,
    pub key: Vec<i32>,
    pub reason: String,
    pub in_start: bool,
}

/// One fact / proof request (per record, keys collected).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RecordRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<&'static str>,
    pub table: String,
    pub offset: u64,
    pub rule: i64,
    pub record_sha256: String,
    pub keys: Vec<Vec<i32>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rows: Option<&'static str>,
}

/// Why a solved relation did not certify its key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SolveRefusal {
    pub source: String,
    pub key: Vec<i32>,
    pub reason: String,
}

/// The result of a check.
pub struct CheckResult {
    pub rows: Vec<KeyRow>,
    pub summary: ClosureSummary,
    pub tier: Tier,
    pub spec_digest: String,
    pub solve_requests: Vec<SolveRequest>,
    pub fact_requests: Vec<RecordRequest>,
    pub proof_requests: Vec<RecordRequest>,
    pub solve_refusals: Vec<SolveRefusal>,
    pub recipe_binary_unpinned: bool,
    pub seconds: f64,
    pub key_set_digest: String,
    pub required: Level,
}

fn is_top(lines: &[bool], k: &[i32]) -> bool {
    k.iter()
        .zip(lines)
        .all(|(&x, &l)| if l { x >= 1 } else { x <= 0 })
}

/// Build the recipe relation sets from the certified solved relations.
/// Recipe relation sets, the refusals of solved relations, and whether some certification used an unpinned binary.
type RecipeSets = (Vec<Arc<RelationSet>>, Vec<SolveRefusal>, bool);

fn recipe_sets(set: &ImportedSet, ev: &Evidence) -> Result<RecipeSets, ImportError> {
    let lines = set.frame.mask().to_vec();
    let expected = set.spec.evidence.expected_recipe_binary.clone();
    let mut unpinned = false;
    let mut out = vec![];
    let mut refusals = vec![];
    for rc in &set.recipes {
        let mut b = RelationStoreBuilder::new(set.codec);
        let solved = ev.solved.get(&rc.id);
        let det: BTreeSet<Option<String>> = solved
            .map(|m| {
                m.values()
                    .filter(|r| r.verdict == "SOLVED")
                    .map(|r| r.determinism_id.clone())
                    .collect()
            })
            .unwrap_or_default();
        let mixed = det.len() > 1;
        for (key, r) in solved.into_iter().flatten() {
            let mut why: Option<String> = None;
            let mut fail = |w: String| {
                if why.is_none() {
                    why = Some(w);
                }
            };
            if r.verdict != "SOLVED" {
                fail(format!("verdict {}", r.verdict));
            }
            if !r.proof_ok {
                fail("exact proof not ok".into());
            }
            if !r.regular {
                fail("not regular".into());
            }
            if mixed {
                fail("several determinism ids for one recipe".into());
            }
            match &r.recipe_sha256 {
                Some(s)
                    if s.trim_start_matches("sha256:")
                        .eq_ignore_ascii_case(&rc.recipe_sha256) => {}
                Some(s) => fail(format!("recipe_sha256 {s} is not the recipe file's")),
                None => fail("no recipe_sha256".into()),
            }
            if let Some(e) = &rc.engine_digest
                && r.recipe_digest.as_deref() != Some(e.as_str())
            {
                fail("engine recipe digest differs".into());
            }
            match (&expected, &r.binary_sha256) {
                (Some(e), Some(b)) if !e.eq_ignore_ascii_case(b) => {
                    fail(format!("solved by binary {b}, expected {e}"))
                }
                (Some(_), None) => fail("no binary_sha256".into()),
                (None, _) => unpinned = true,
                _ => {}
            }
            if key.len() != set.arity() || r.rhs.iter().any(|k| k.len() != set.arity()) {
                fail("key of the wrong arity".into());
            }
            if r.rhs.is_empty() && r.verdict == "SOLVED" {
                fail("empty right-hand side".into());
            }
            for k in &r.rhs {
                if k.iter().zip(&lines).any(|(&x, &l)| !l && x > 0) {
                    fail(format!("rhs key {k:?} has a positive numerator"));
                } else if is_top(&lines, k) && !set.cmp.lower(k, key) {
                    fail(format!("rhs key {k:?} is not lower than the key"));
                }
            }
            match why {
                Some(w) => refusals.push(super::check::SolveRefusal {
                    source: rc.id.clone(),
                    key: key.clone(),
                    reason: w,
                }),
                None => {
                    let mut d = [0u8; 32];
                    if let Some(h) = &r.relation_digest {
                        let h = h.trim_start_matches("blake3:");
                        if h.len() == 64 {
                            for (i, byte) in d.iter_mut().enumerate() {
                                *byte = u8::from_str_radix(&h[2 * i..2 * i + 2], 16).unwrap_or(0);
                            }
                        }
                    }
                    b.push(key, &r.rhs, d, VERDICT_CERTIFIED)
                        .map_err(|e| ImportError::Core(format!("recipe {}: {e:?}", rc.id)))?;
                }
            }
        }
        let relations = b
            .finish()
            .map_err(|e| ImportError::Core(format!("recipe {}: {e:?}", rc.id)))?;
        out.push(Arc::new(RelationSet {
            name: rc.id.clone(),
            recipe_digest: rc
                .engine_digest
                .clone()
                .unwrap_or_else(|| format!("sha256:{}", rc.recipe_sha256)),
            domain: rc.domain.clone(),
            relations,
            slot: rc.slot.clone(),
        }));
    }
    Ok((out, refusals, unpinned))
}

/// Run the check.
pub fn check(
    set: &ImportedSet,
    ev: &Evidence,
    opts: &CheckOptions,
) -> Result<CheckResult, ImportError> {
    let t0 = std::time::Instant::now();
    let tier = opts.tier.unwrap_or(match set.spec.policies.tier {
        TierChoice::K => Tier::K,
        TierChoice::C => Tier::C,
        TierChoice::Auto => {
            if ev.has_facts() {
                Tier::C
            } else {
                Tier::K
            }
        }
    });
    let (rsets, solve_refusals, unpinned) = recipe_sets(set, ev)?;
    let relation_sets: Vec<Arc<RelationSet>> = set
        .rel_order
        .iter()
        .map(|e| match *e {
            super::load::RelEntry::Supplied(i) => set.supplied[i].clone(),
            super::load::RelEntry::Recipe(i) => rsets[i].clone(),
        })
        .collect();
    let ks = |v: &[Vec<i32>]| -> Result<Option<crate::closure::KeySet>, ImportError> {
        if v.is_empty() {
            return Ok(None);
        }
        let mut b = KeySetBuilder::new(set.codec);
        for k in v {
            b.push(k)
                .map_err(|e| ImportError::Core(format!("terminal {k:?}: {e:?}")))?;
        }
        Ok(Some(b.finish()))
    };
    let terminals = if set.terminals_preferred.is_empty() && set.terminals_other.is_empty() {
        None
    } else {
        Some(TerminalSets {
            preferred: ks(&set.terminals_preferred)?,
            other: ks(&set.terminals_other)?,
        })
    };
    let cfg = ClosureConfig {
        tier,
        threads: opts.threads.max(1),
        max_rounds: opts.max_rounds,
        bits: set.spec.bits,
        hand_downs: HandDowns::Counts,
        credits: set.credits.clone(),
        terminals,
        fact_tables: set.fact_tables.clone(),
        table_digests: set.table_digests.clone(),
        relation_sets,
        witness_absent: match set.spec.policies.witness_absent {
            super::spec::WitnessAbsentPolicy::LiftWithRegularFact => {
                WitnessAbsent::LiftWithRegularFact
            }
            super::spec::WitnessAbsentPolicy::Block => WitnessAbsent::Block,
        },
        ..Default::default()
    };
    let c = closure(&set.disp, &set.bodies, &set.cmp, &set.start, &cfg, ev)
        .map_err(|e| ImportError::Core(format!("closure: {e}")))?;
    let summary = c.summary();
    // name -> table index; open info by key
    let tindex: HashMap<&str, usize> = set
        .disp
        .tables()
        .iter()
        .enumerate()
        .map(|(i, t)| (t.name.as_str(), i))
        .collect();
    let recipe_names: BTreeSet<String> = set.recipes.iter().map(|r| r.id.clone()).collect();
    let open: HashMap<Vec<i32>, crate::closure::OpenKey> = c
        .open_keys()
        .into_iter()
        .map(|o| (o.key.clone(), o))
        .collect();
    // coefficient fact requests per (table path, label, key)
    let freq = c.fact_requests();
    let mut coef_missing: BTreeSet<(String, i64, Vec<i32>)> = BTreeSet::new();
    for r in &freq {
        if r.kind == crate::closure::FactKind::Coefficient {
            coef_missing.insert((r.table.clone(), r.label, r.key.clone()));
        }
    }
    let path_label: HashMap<(String, i64), (usize, usize)> = set
        .rules
        .iter()
        .enumerate()
        .flat_map(|(t, v)| {
            v.iter()
                .enumerate()
                .map(move |(r, i)| ((i.path.clone(), i.label), (t, r)))
        })
        .collect();
    let mut rows = Vec::with_capacity(c.len());
    let required = set.spec.required_evidence;
    for i in 0..c.len() {
        let key = c.key(i);
        let in_start = c.in_start(i);
        let mut row = KeyRow {
            key,
            in_start,
            status: RowStatus::Open,
            server: None,
            evidence: None,
            basis: None,
            regularity_unknown: false,
            coefficient_unknown: false,
            open_status: None,
            open_reason: None,
            open_rule: None,
        };
        let rule_of = |n: &crate::closure::RuleName| -> Option<(usize, usize)> {
            tindex.get(n.table.as_str()).map(|&t| (t, n.record))
        };
        if c.is_terminal(i) {
            row.status = RowStatus::Terminal;
        } else if c.is_open(i) {
            let o = open.get(&row.key);
            row.open_status = Some(
                o.map(|o| format!("{:?}", o.status).to_lowercase())
                    .unwrap_or_else(|| "open".into()),
            );
            row.open_reason = o.map(|o| o.detail.clone()).filter(|d| !d.is_empty());
            row.open_rule = o.and_then(|o| o.rule.as_ref()).and_then(rule_of);
            if let (Some(o), Some((t, r))) = (o, row.open_rule)
                && o.status == OpenStatus::Blocked
            {
                let w = set.rules[t][r].witness;
                let why = match w {
                    WitnessKind::Elided => "elided witness loci (policy block)".to_string(),
                    WitnessKind::NotCarried => {
                        "witness loci not carried: no per-key regularity fact (ruling Q2)"
                            .to_string()
                    }
                    _ => o.detail.clone(),
                };
                row.open_reason = Some(why);
            }
            if let Some(o) = o
                && o.status == OpenStatus::Uncovered
                && !o.recipe_candidates.is_empty()
            {
                row.open_reason = Some(format!(
                    "uncovered; in the domain of recipe(s) {} without a certified relation",
                    o.recipe_candidates.join(",")
                ));
            }
        } else if c.is_recipe_served(i) {
            let w = c.winner(i);
            let name = w.map(|w| w.table).unwrap_or_default();
            let recipe = recipe_names.contains(&name);
            row.status = RowStatus::Served;
            row.server = Some(Server::Relation { set: name, recipe });
            if recipe {
                row.evidence = Some(Level::PerKey);
                row.basis = Some("recipe_certified");
            } else {
                row.evidence = Some(Level::Imported);
                row.basis = Some("supplied");
            }
        } else if let Some(w) = c.winner(i) {
            let Some((t, r)) = rule_of(&w) else {
                return Err(ImportError::Core(format!(
                    "winner table {} not in the set",
                    w.table
                )));
            };
            row.status = RowStatus::Served;
            if c.is_credited(i) {
                row.server = Some(Server::Credit {
                    table: t,
                    record: r,
                });
                row.evidence = Some(Level::Imported);
                row.basis = Some("symmetry_credit");
            } else {
                row.server = Some(Server::Rule {
                    table: t,
                    record: r,
                });
                let info = &set.rules[t][r];
                let proof = ev.proof(set, t, r, &row.key).is_some_and(|p| p.per_key);
                row.evidence = Some(if proof {
                    Level::PerKey
                } else {
                    Level::Imported
                });
                row.basis = Some(match info.witness {
                    WitnessKind::Complete => "witness_loci",
                    WitnessKind::NotCarried => "regularity_fact",
                    WitnessKind::ElidedLifted => "elided_per_key_proof",
                    WitnessKind::Elided => "elided",
                });
                if info.witness == WitnessKind::ElidedLifted && !proof {
                    row.status = RowStatus::Open;
                    row.open_status = Some("blocked".into());
                    row.open_reason = Some("elided_witness_loci_no_per_key_proof".into());
                    row.open_rule = Some((t, r));
                }
                if tier == Tier::C {
                    use crate::closure::{FactSource, RegFact, RuleId};
                    let digest = String::new();
                    let rid = RuleId {
                        table: &info.path,
                        record: r,
                        label: info.label,
                        digest: &digest,
                    };
                    row.regularity_unknown = ev.regularity(&rid, &row.key) == RegFact::Unknown;
                    row.coefficient_unknown =
                        coef_missing.contains(&(info.path.clone(), info.label, row.key.clone()));
                }
            }
        } else {
            row.open_status = Some("undecided".into());
            row.open_reason = Some("symmetry credit pending on unknown facts".into());
        }
        rows.push(row);
    }
    rows.sort_by(|a, b| a.key.cmp(&b.key));
    // key-set digest
    let key_set_digest = c.keys().digest();
    // solve requests
    let mut solve_requests = vec![];
    for o in open.values() {
        if o.status != OpenStatus::Uncovered {
            continue;
        }
        // economy (R48 addendum 4): request the FIRST candidate recipe in dispatch order that has no answer for this
        // key yet; a candidate whose answer did not certify is skipped (the next one is asked); none left -> no request
        for cand in &o.recipe_candidates {
            if let Some(rc) = set.recipes.iter().find(|r| &r.id == cand) {
                let prev = ev.solved.get(&rc.id).and_then(|m| m.get(&o.key));
                if prev.is_some() {
                    continue;
                }
                solve_requests.push(SolveRequest {
                    schema: super::SOLVE_REQUEST_SCHEMA,
                    spec_digest: set.report.spec_digest.clone(),
                    source: rc.id.clone(),
                    recipe_path: rc.path.clone(),
                    recipe_sha256: rc.recipe_sha256.clone(),
                    engine_digest: rc.engine_digest.clone(),
                    key: o.key.clone(),
                    reason: "in_domain_uncovered".into(),
                    in_start: o.in_start,
                });
                break;
            }
        }
    }
    solve_requests.sort();
    // fact requests: per record
    type KeysReasons = (BTreeSet<Vec<i32>>, BTreeSet<String>);
    let mut fr: BTreeMap<(usize, usize), KeysReasons> = BTreeMap::new();
    for r in &freq {
        if let Some(&(t, rr)) = path_label.get(&(r.table.clone(), r.label)) {
            let e = fr.entry((t, rr)).or_default();
            e.0.insert(r.key.clone());
            e.1.insert(
                serde_json::to_value(r.reason)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default(),
            );
        }
    }
    let fact_requests = fr
        .into_iter()
        .map(|((t, r), (keys, reasons))| {
            let info = &set.rules[t][r];
            RecordRequest {
                schema: None,
                table: info.path.clone(),
                offset: info.offset,
                rule: info.index.unwrap_or(info.label),
                record_sha256: info.record_sha256.clone(),
                keys: keys.into_iter().collect(),
                reasons: reasons.into_iter().collect(),
                rows: None,
            }
        })
        .collect();
    // proof requests: served rule keys below the required level (per-key is the only producible level above imported)
    let mut pr: BTreeMap<(usize, usize), BTreeSet<Vec<i32>>> = BTreeMap::new();
    if required == Level::PerKey {
        for row in &rows {
            let below = row.evidence.is_none_or(|e| e < Level::PerKey);
            let rule = match (&row.server, row.status, row.open_rule) {
                (Some(Server::Rule { table, record }), RowStatus::Served, _) if below => {
                    Some((*table, *record))
                }
                (_, RowStatus::Open, Some(tr))
                    if row.open_reason.as_deref()
                        == Some("elided_witness_loci_no_per_key_proof") =>
                {
                    Some(tr)
                }
                _ => None,
            };
            if let Some(tr) = rule {
                pr.entry(tr).or_default().insert(row.key.clone());
            }
        }
    }
    let proof_requests = pr
        .into_iter()
        .map(|((t, r), keys)| {
            let info = &set.rules[t][r];
            RecordRequest {
                schema: Some(super::PROOF_REQUEST_SCHEMA),
                table: info.path.clone(),
                offset: info.offset,
                rule: info.index.unwrap_or(info.label),
                record_sha256: info.record_sha256.clone(),
                keys: keys.into_iter().collect(),
                reasons: vec![],
                rows: Some("engine"),
            }
        })
        .collect();
    Ok(CheckResult {
        rows,
        summary,
        tier,
        spec_digest: set.report.spec_digest.clone(),
        solve_requests,
        fact_requests,
        proof_requests,
        solve_refusals,
        recipe_binary_unpinned: unpinned,
        seconds: t0.elapsed().as_secs_f64(),
        key_set_digest,
        required,
    })
}

impl CheckResult {
    /// Counts by status / evidence / basis (for summaries and tests).
    pub fn counts(&self) -> BTreeMap<String, u64> {
        let mut m: BTreeMap<String, u64> = BTreeMap::new();
        for r in &self.rows {
            let st = match r.status {
                RowStatus::Served => "served",
                RowStatus::Terminal => "terminal",
                RowStatus::Open => "open",
            };
            *m.entry(format!("status:{st}")).or_default() += 1;
            if let Some(e) = r.evidence
                && r.status == RowStatus::Served
            {
                *m.entry(format!("evidence:{}", e.word())).or_default() += 1;
            }
            if let Some(b) = r.basis
                && r.status == RowStatus::Served
            {
                *m.entry(format!("basis:{b}")).or_default() += 1;
            }
            if let Some(o) = &r.open_status {
                *m.entry(format!("open:{o}")).or_default() += 1;
            }
            if r.regularity_unknown {
                *m.entry("facts:regularity_unknown".into()).or_default() += 1;
            }
            if r.coefficient_unknown {
                *m.entry("facts:coefficient_unknown".into()).or_default() += 1;
            }
            if r.status == RowStatus::Served && r.evidence.is_some_and(|e| e >= self.required) {
                *m.entry("certified".into()).or_default() += 1;
            }
        }
        m
    }

    /// Every reason the import's statement is not a certified close (empty iff certified closed).
    pub fn blocking(&self) -> Vec<String> {
        let mut b = self.summary.blocking.clone();
        let c = self.counts();
        let served = c.get("status:served").copied().unwrap_or(0);
        let cert = c.get("certified").copied().unwrap_or(0);
        if cert < served {
            b.push(format!("below_required_evidence:{}", served - cert));
        }
        let open = c.get("status:open").copied().unwrap_or(0);
        if open > 0 && !b.iter().any(|s| s.starts_with("open_keys")) {
            b.push(format!("open_keys:{open}"));
        }
        if self.recipe_binary_unpinned {
            b.push("recipe_binary_unpinned".into());
        }
        b
    }
}
