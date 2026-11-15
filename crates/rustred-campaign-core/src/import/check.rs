//! `check`: the closure of the start set under the STRICT core dispatcher over the imported set (Tier C where facts
//! exist), classified per key: which keys are served at which evidence level and on what basis, which are blocked and
//! why, and the requests still missing (recipe solves, facts, per-key proofs).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use serde::Serialize;

use super::error::ImportError;
use super::evidence::Evidence;
use super::load::{ImportedSet, WitnessKind};
use super::spec::{ElidedPolicy, Level, TierChoice};
use crate::closure::relations::{RelationSet, RelationStoreBuilder, VERDICT_CERTIFIED};
use crate::closure::{
    ClosureConfig, ClosureSummary, HandDowns, KeySetBuilder, OpenStatus, PerKeyLift, TerminalSets,
    Tier, WitnessAbsent, closure,
};

/// Basis word of a key served by a witness-absent record under the DIAGNOSTIC `witness_check: off_diagnostic`.
pub const BASIS_WITNESS_UNCHECKED: &str = "witness_unchecked_diagnostic";

/// Basis word of a key served by a supplied symmetry relation VERIFIED by re-derivation from the verified group.
pub const BASIS_SYMMETRY_RELATION: &str = "symmetry_relation";

/// Basis word of a key served through the per-key lift (elided / witness-absent record + bound per-key proof).
pub const BASIS_PER_KEY_PROOF: &str = "per_key_proof";

/// Basis word of a key served by a DIAGNOSTIC keys-only relation (an external checker's; never certified).
pub const BASIS_KEYS_ONLY_DIAGNOSTIC: &str = "keys_only_relation_diagnostic";

/// Basis word (diagnostic dispatch only) of a key served by a record with complete witness loci whose STRICT domain
/// does not apply at the key (on a witness pole): a strict run blocks it there.
pub const BASIS_WITNESS_POLE_DIAGNOSTIC: &str = "witness_pole_diagnostic";

/// Basis word (diagnostic dispatch only) of a key served by an ELIDED record without a per-key proof.
pub const BASIS_ELIDED_UNCHECKED: &str = "elided_unchecked_diagnostic";

/// Basis word of a key credited through a symmetry element whose IMAGE is served by a relation set (closure.md 15.6;
/// server = that set): level none for a diagnostic set, else per-key (verified group) / imported.
pub const BASIS_CREDIT_VIA_RELATION: &str = "symmetry_credit_via_relation";

/// Basis word of a key credited through a VERIFIED symmetry group (I(key) = I(image) re-derived from the family,
/// closure.md 15.7): its evidence level is the image's (per-key iff a per-key proof is bound to (image record, image)).
pub const BASIS_SYMMETRY_CREDIT_VERIFIED: &str = "symmetry_credit_verified";

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
    /// lane closeloop3: Tier C, a rule-served key with a bound fact line: who produced it (`rr_proof` | `exact_eval` |
    /// `compact`; interfaces/closeloop.md 16).
    pub fact_basis: Option<&'static str>,
    /// Open keys: the core status word, the reason and the blocking rule.
    pub open_status: Option<String>,
    pub open_reason: Option<String>,
    pub open_rule: Option<(usize, usize)>,
    /// Credited keys (server `Credit`, or a `Relation` with basis [`BASIS_CREDIT_VIA_RELATION`]): the image I(key) is
    /// credited to. Its level is never below this row's (a credit never raises a level; lane import4).
    pub image: Option<Vec<i32>>,
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

/// Rows streamed to a file by [`check_streamed`] (the statement's `statement.jsonl`, closure order).
#[derive(Clone, Debug, Serialize)]
pub struct StreamedRows {
    pub path: String,
    pub sha256: String,
    pub rows: u64,
}

/// Aggregates of a streamed check (what `counts` / `evidence_by_class` return when the rows are not all kept).
#[derive(Clone, Debug, Default)]
pub struct Aggregates {
    pub counts: BTreeMap<String, u64>,
    pub by_class: BTreeMap<String, BTreeMap<String, u64>>,
}

/// The result of a check.
pub struct CheckResult {
    /// Every closure row (sorted by key); after [`check_streamed`] only the EXCEPTIONAL rows (open, terminal, served
    /// by a relation or a credit, below the required level, with a missing fact): the rest are in `streamed`.
    pub rows: Vec<KeyRow>,
    /// [`check_streamed`]: the file holding every row, and the aggregates over all of them.
    pub streamed: Option<StreamedRows>,
    pub aggregates: Option<Aggregates>,
    /// [`check_streamed`]: the closure's key set (packed; membership of keys whose row is not kept).
    pub key_set: Option<crate::closure::KeySet>,
    pub summary: ClosureSummary,
    pub tier: Tier,
    pub spec_digest: String,
    pub solve_requests: Vec<SolveRequest>,
    pub fact_requests: Vec<RecordRequest>,
    pub proof_requests: Vec<RecordRequest>,
    pub solve_refusals: Vec<SolveRefusal>,
    pub recipe_binary_unpinned: bool,
    /// The DIAGNOSTIC policy `witness_check: off_diagnostic` was used (nothing is certified).
    pub witness_unchecked: bool,
    /// Some source is a DIAGNOSTIC keys-only relation set (its keys are followed, never certified).
    pub keys_only_diagnostic: bool,
    pub seconds: f64,
    pub key_set_digest: String,
    pub required: Level,
    /// lane layers (interfaces/closure.md 18, closeloop.md 23): the leaves this check's walk met (None without leaf
    /// sets). Leaf keys have NO row (they belong to the earlier clause), are not in `key_set` and not counted.
    pub leaves: Option<LeafHits>,
    /// lane layers: the walk components of this check's OWN keys and credit images, and those images: what a leaf set
    /// built from this result is bound to (closure.md 18).
    pub leaf_walk: BTreeMap<String, String>,
    pub credit_images: Vec<Vec<i32>>,
}

/// lane layers: the leaves a check met.
#[derive(Clone, Debug, Serialize)]
pub struct LeafHits {
    pub summary: crate::closure::LeafSummary,
    /// Start keys that are leaves (the composed statement marks them `in_start`).
    pub in_start: Vec<Vec<i32>>,
    /// Reached leaves that are not certified (they block: `leaf_open:N`).
    pub open: Vec<Vec<i32>>,
    /// The leaf sets themselves (membership of the composed closure: a key of a leaf set is a closure key).
    #[serde(skip)]
    pub parts: Vec<Arc<crate::closure::LeafSet>>,
}

impl LeafHits {
    /// Whether `key` is a key of one of the leaf sets.
    pub fn contains(&self, key: &[i32]) -> bool {
        self.parts.iter().any(|p| p.contains(key))
    }
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
            if rc.assigned.as_ref().is_some_and(|a| !a.contains(key)) {
                fail("key not assigned to the recipe (domain.keys / assignment side file)".into());
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

/// The closure's per-key lift for this set (policy `elided`).
pub(crate) fn per_key_lift(set: &ImportedSet) -> PerKeyLift {
    match set.spec.policies.elided {
        ElidedPolicy::LiftWithPerKeyProof => PerKeyLift::Proof,
        ElidedPolicy::Block => PerKeyLift::Off,
    }
}

/// The open reason of a key blocked by an elided / witness-absent record (statement word).
pub(crate) fn blocked_reason(set: &ImportedSet, w: WitnessKind, core_detail: &str) -> String {
    let lift = per_key_lift(set) == PerKeyLift::Proof;
    match (w, lift) {
        (WitnessKind::Elided, true) => REASON_NO_PER_KEY_PROOF.to_string(),
        (WitnessKind::Elided, false) => "elided witness loci (policy block)".to_string(),
        (WitnessKind::NotCarried, true) => {
            "witness loci not carried: no per-key proof and no per-key regularity fact".to_string()
        }
        (WitnessKind::NotCarried, false) => {
            "witness loci not carried: no per-key regularity fact (ruling Q2)".to_string()
        }
        _ => core_detail.to_string(),
    }
}

/// Open reason of a key whose elided record has no per-key proof bound to (record_sha256, key).
pub const REASON_NO_PER_KEY_PROOF: &str = "elided_witness_loci_no_per_key_proof";

/// Run the check (every row kept in memory).
pub fn check(
    set: &ImportedSet,
    ev: &Evidence,
    opts: &CheckOptions,
) -> Result<CheckResult, ImportError> {
    check_inner(set, ev, opts, None, &set.start, &[])
}

/// lane layers (closure.md 18): the walk components of this set's configuration (tables, relation sets of the
/// evidence, terminals, credits) for the key set `member` (e.g. a node's composed statement): an empty walk computes
/// the identity, nothing is dispatched.
pub fn leaf_walk_for(
    set: &ImportedSet,
    ev: &Evidence,
    tier: Tier,
    member: &dyn Fn(&[i32]) -> bool,
) -> Result<BTreeMap<String, String>, ImportError> {
    let (rsets, _, _) = recipe_sets(set, ev)?;
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
        threads: 1,
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
        per_key_lift: per_key_lift(set),
        longest_chain: false,
        ..Default::default()
    };
    let empty = crate::closure::KeySet::empty(set.codec);
    let c = closure(&set.disp, &set.bodies, &set.cmp, &empty, &cfg, ev)
        .map_err(|e| ImportError::Core(format!("closure: {e}")))?;
    Ok(c.walk_for(member))
}

/// lane layers (closure.md 18): [`check`] from another START set with LEAF SETS (every row kept in memory). The walk
/// stops at a leaf; leaf keys get no row; every leaf set's binding is verified by the closure (refused on a mismatch).
pub fn check_from_with_leaves(
    set: &ImportedSet,
    ev: &Evidence,
    opts: &CheckOptions,
    start: &crate::closure::KeySet,
    leaves: &[Arc<crate::closure::LeafSet>],
) -> Result<CheckResult, ImportError> {
    check_inner(set, ev, opts, None, start, leaves)
}

/// A row sink of [`check_streamed`].
type RowSink<'a> = dyn FnMut(&KeyRow) -> Result<(), ImportError> + 'a;

/// Run the check with BOUNDED row memory (interfaces/import.md 12, scale): every row is written at once as its
/// canonical statement line to `rows_path` (closure order: start keys first, then by BFS round), the counts and the
/// evidence by class are aggregated on the fly, and only the exceptional rows are kept in `rows` (see
/// [`CheckResult::rows`]). [`super::write_statement`] then records that file instead of writing the rows again.
pub fn check_streamed(
    set: &ImportedSet,
    ev: &Evidence,
    opts: &CheckOptions,
    rows_path: &std::path::Path,
) -> Result<CheckResult, ImportError> {
    check_streamed_from(set, ev, opts, rows_path, &set.start)
}

/// [`check_streamed`] from another START set (lane t266run: one clause of a box made of several clauses; everything
/// else, tables, relations, evidence, terminals, is the set's).
pub fn check_streamed_from(
    set: &ImportedSet,
    ev: &Evidence,
    opts: &CheckOptions,
    rows_path: &std::path::Path,
    start: &crate::closure::KeySet,
) -> Result<CheckResult, ImportError> {
    check_streamed_with_leaves(set, ev, opts, rows_path, start, &[])
}

/// lane layers (closure.md 18): [`check_streamed_from`] with LEAF SETS (leaf keys get no row).
pub fn check_streamed_with_leaves(
    set: &ImportedSet,
    ev: &Evidence,
    opts: &CheckOptions,
    rows_path: &std::path::Path,
    start: &crate::closure::KeySet,
    leaves: &[Arc<crate::closure::LeafSet>],
) -> Result<CheckResult, ImportError> {
    use std::io::Write;
    if let Some(d) = rows_path.parent() {
        std::fs::create_dir_all(d).map_err(|e| super::error::io_err(d, e))?;
    }
    let f = std::fs::File::create(rows_path).map_err(|e| super::error::io_err(rows_path, e))?;
    let mut w = std::io::BufWriter::with_capacity(1 << 20, f);
    let mut h = crate::closure::sha256::Sha256::new();
    let mut n = 0u64;
    let mut res = {
        let mut sink = |row: &KeyRow| -> Result<(), ImportError> {
            let t =
                crate::schema::canon::canonical_json(&super::statement::row_json(set, row), false)
                    .map_err(ImportError::Core)?;
            w.write_all(t.as_bytes())
                .and_then(|_| w.write_all(b"\n"))
                .map_err(|e| super::error::io_err(rows_path, e))?;
            h.update(t.as_bytes());
            h.update(b"\n");
            n += 1;
            Ok(())
        };
        check_inner(set, ev, opts, Some(&mut sink), start, leaves)?
    };
    w.flush().map_err(|e| super::error::io_err(rows_path, e))?;
    res.streamed = Some(StreamedRows {
        path: rows_path.display().to_string(),
        sha256: h.finish_hex(),
        rows: n,
    });
    Ok(res)
}

/// The evidence level and basis of `key` served by the relation set `name` (`recipe`: one of our recipes), exactly as a
/// relation-served row is scored; also the IMAGE level of a credit through that set (audit-credits M1). `None` = a
/// keys-only diagnostic relation (no evidence).
pub(crate) fn relation_level(
    set: &ImportedSet,
    ev: &Evidence,
    name: &str,
    recipe: bool,
    key: &[i32],
) -> (Option<Level>, &'static str) {
    let sinfo = set.supplied_info.get(name).and_then(|m| m.get(key));
    if sinfo.is_some_and(|i| i.diagnostic) {
        return (None, BASIS_KEYS_ONLY_DIAGNOSTIC);
    }
    if recipe {
        // lane closeloop2 (closeloop.md 13.6): our own relations need their bound per-key proof when asked to
        let proved = ev
            .recipe_proofs
            .as_ref()
            .is_none_or(|s| s.contains(&(name.to_string(), key.to_vec())));
        return if proved {
            (Some(Level::PerKey), "recipe_certified")
        } else {
            (Some(Level::Imported), "recipe_relation_unproved")
        };
    }
    if sinfo
        .and_then(|i| i.check.as_ref())
        .is_some_and(|c| c.verified())
    {
        // re-derived from the verified group at the key: exact, at the key (interfaces/import.md 13.2)
        (Some(Level::PerKey), BASIS_SYMMETRY_RELATION)
    } else {
        (Some(Level::Imported), "supplied")
    }
}

fn check_inner(
    set: &ImportedSet,
    ev: &Evidence,
    opts: &CheckOptions,
    mut sink: Option<&mut RowSink<'_>>,
    start: &crate::closure::KeySet,
    leaves: &[Arc<crate::closure::LeafSet>],
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
        per_key_lift: per_key_lift(set),
        leaves: leaves.to_vec(),
        ..Default::default()
    };
    let c = closure(&set.disp, &set.bodies, &set.cmp, start, &cfg, ev)
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
    // credited keys -> image (the credit's level is the image's: closure.md 17 / interfaces/import.md 15)
    let credit_images: HashMap<Vec<i32>, Vec<i32>> = c
        .credits()
        .into_iter()
        .map(|cr| (cr.key, cr.image))
        .collect();
    let verified_credits = set.credits.as_ref().is_some_and(|p| p.is_verified());
    // the credit's own level: the identity I(key) = I(image) is exact at the key only for a verified group
    let credit_level = if verified_credits {
        Level::PerKey
    } else {
        Level::Imported
    };
    let mut rows = Vec::with_capacity(if sink.is_some() { 0 } else { c.len() });
    let mut agg = Aggregates::default();
    let required = set.spec.required_evidence;
    let mut leaf_in_start: Vec<Vec<i32>> = vec![];
    for i in 0..c.len() {
        // lane layers: a leaf has no row here (it belongs to the earlier clause's statement)
        if c.is_leaf(i) {
            if c.in_start(i) {
                leaf_in_start.push(c.key(i));
            }
            continue;
        }
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
            fact_basis: None,
            open_status: None,
            open_reason: None,
            open_rule: None,
            image: None,
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
                row.open_reason = Some(blocked_reason(set, set.rules[t][r].witness, &o.detail));
            }
            if let Some(o) = o
                && o.status == OpenStatus::Uncovered
            {
                // only RECIPE sources are candidates (a supplied point set's chart-wide domain says nothing)
                let cands: Vec<&str> = o
                    .recipe_candidates
                    .iter()
                    .filter(|c| {
                        set.recipes.iter().any(|r| {
                            &r.id == *c && r.assigned.as_ref().is_none_or(|a| a.contains(&o.key))
                        })
                    })
                    .map(String::as_str)
                    .collect();
                row.open_reason = Some(if cands.is_empty() {
                    "uncovered; no recipe domain contains the key".to_string()
                } else {
                    format!(
                        "uncovered; in the domain of recipe(s) {} without a certified relation",
                        cands.join(",")
                    )
                });
            }
        } else if c.is_recipe_served(i) {
            let w = c.winner(i);
            let name: String = w.map(|w| w.table).unwrap_or_default();
            let recipe = recipe_names.contains(&name);
            row.status = RowStatus::Served;
            row.server = Some(Server::Relation {
                set: name.clone(),
                recipe,
            });
            let (level, basis) = relation_level(set, ev, &name, recipe, &row.key);
            row.evidence = level;
            row.basis = Some(basis);
        } else if let Some(w) = c
            .winner(i)
            .filter(|w| c.is_credited(i) && !tindex.contains_key(w.table.as_str()))
        {
            // a credit whose image is served by a relation set (closure.md 15.6). Its level is the LOWER of the
            // credit's own level and the level at which the relation serves the IMAGE (a credit never raises a level;
            // audit-credits M1): the image is scored exactly as a key the relation serves directly.
            let name = w.table.clone();
            let recipe = recipe_names.contains(&name);
            let image_level = credit_images
                .get(&row.key)
                .and_then(|im| relation_level(set, ev, &name, recipe, im).0);
            row.status = RowStatus::Served;
            row.server = Some(Server::Relation { set: name, recipe });
            row.evidence = image_level.map(|l| l.min(credit_level));
            row.image = credit_images.get(&row.key).cloned();
            row.basis = Some(BASIS_CREDIT_VIA_RELATION);
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
                let image_proof = credit_images
                    .get(&row.key)
                    .is_some_and(|im| ev.proof(set, t, r, im).is_some_and(|p| p.per_key));
                // the image's level exactly as the direct-rule branch scores a key served by (t, r): per-key iff an
                // exact per-key proof is bound at the image (which is also what lifts an image, closure.md 17)
                let image_level = if image_proof {
                    Level::PerKey
                } else {
                    Level::Imported
                };
                // exact identity I(key) = I(image) (verified group) + the image's own evidence: the lower of the two
                row.evidence = Some(image_level.min(credit_level));
                row.image = credit_images.get(&row.key).cloned();
                row.basis = Some(if verified_credits {
                    BASIS_SYMMETRY_CREDIT_VERIFIED
                } else {
                    "symmetry_credit"
                });
            } else {
                row.server = Some(Server::Rule {
                    table: t,
                    record: r,
                });
                let info = &set.rules[t][r];
                let lifted = c.is_per_key_lifted(i);
                let proof = lifted || ev.proof(set, t, r, &row.key).is_some_and(|p| p.per_key);
                row.evidence = Some(if proof {
                    Level::PerKey
                } else {
                    Level::Imported
                });
                let unchecked = set.spec.policies.witness_check != super::spec::WitnessCheck::On;
                let strict_blocks = unchecked
                    && info.witness == WitnessKind::Complete
                    && set
                        .strict_domains
                        .get(&(t, r))
                        .is_some_and(|d| !d.applies(&row.key).applies());
                row.basis = Some(match (info.witness, lifted) {
                    (WitnessKind::Elided, false) if unchecked && !proof => BASIS_ELIDED_UNCHECKED,
                    (WitnessKind::Elided, _) | (WitnessKind::NotCarried, true) => {
                        BASIS_PER_KEY_PROOF
                    }
                    (WitnessKind::Complete, _) if strict_blocks => BASIS_WITNESS_POLE_DIAGNOSTIC,
                    (WitnessKind::Complete, _) => "witness_loci",
                    (WitnessKind::NotCarried, false) if unchecked => BASIS_WITNESS_UNCHECKED,
                    (WitnessKind::NotCarried, false) => "regularity_fact",
                });
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
                    // lane closeloop3: the fact basis of the key (who produced its fact line)
                    row.fact_basis = ev.fact_basis(&rid, &row.key);
                }
            }
        } else {
            row.open_status = Some("undecided".into());
            row.open_reason = Some("symmetry credit pending on unknown facts".into());
        }
        match sink.as_mut() {
            None => rows.push(row),
            Some(s) => {
                count_row(&mut agg.counts, &row, required);
                class_row(&mut agg.by_class, set, &row, required);
                s(&row)?;
                let plain = row.status == RowStatus::Served
                    && matches!(row.server, Some(Server::Rule { .. }))
                    && row.evidence.is_some_and(|e| e >= required)
                    && !row.regularity_unknown
                    && !row.coefficient_unknown;
                if !plain {
                    rows.push(row);
                }
            }
        }
    }
    rows.sort_by(|a, b| a.key.cmp(&b.key));
    // key-set digest (lane layers: the walk's OWN keys; reached leaves belong to their leaf set)
    let ks = if leaves.is_empty() {
        c.keys()
    } else {
        c.own_keys()
    };
    let key_set_digest = ks.digest();
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
                if rc.assigned.as_ref().is_some_and(|a| !a.contains(&o.key)) {
                    continue; // not assigned to this recipe: never asked
                }
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
            let rule = match (&row.server, row.status) {
                (Some(Server::Rule { table, record }), RowStatus::Served) if below => {
                    Some((*table, *record))
                }
                _ => None,
            };
            if let Some(tr) = rule {
                pr.entry(tr).or_default().insert(row.key.clone());
            }
        }
    }
    // under the per-key lift every key blocked by an elided / witness-absent record needs its per-key proof, whatever
    // the required level (it is the only evidence that lets the record serve there)
    if per_key_lift(set) == PerKeyLift::Proof {
        for row in &rows {
            if let (RowStatus::Open, Some((t, r)), Some("blocked")) =
                (row.status, row.open_rule, row.open_status.as_deref())
                && matches!(
                    set.rules[t][r].witness,
                    WitnessKind::Elided | WitnessKind::NotCarried
                )
            {
                pr.entry((t, r)).or_default().insert(row.key.clone());
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
    let streaming = sink.is_some();
    let leaf_hits = summary.leaves.clone().map(|s| {
        leaf_in_start.sort();
        LeafHits {
            summary: s,
            in_start: leaf_in_start,
            open: c.leaf_open_keys(),
            parts: leaves.to_vec(),
        }
    });
    Ok(CheckResult {
        leaves: leaf_hits,
        leaf_walk: c.leaf_walk().clone(),
        credit_images: c.credit_images().iter().collect(),
        rows,
        streamed: None,
        aggregates: streaming.then_some(agg),
        key_set: streaming.then_some(ks),
        summary,
        tier,
        spec_digest: set.report.spec_digest.clone(),
        solve_requests,
        fact_requests,
        proof_requests,
        solve_refusals,
        recipe_binary_unpinned: unpinned,
        witness_unchecked: set.spec.policies.witness_check != super::spec::WitnessCheck::On,
        keys_only_diagnostic: set.spec.sources.iter().any(|s| s.diagnostic),
        seconds: t0.elapsed().as_secs_f64(),
        key_set_digest,
        required,
    })
}

impl CheckResult {
    /// After [`check_streamed`]: whether `key` is in the closure but not among the kept (exceptional) rows, i.e.
    /// served by a plain rule at the required level (its row is only in the streamed file). Always false otherwise.
    pub fn rows_complete_for(&self, key: &[i32]) -> bool {
        !(self.streamed.is_some() && self.key_set.as_ref().is_some_and(|k| k.contains(key)))
    }

    /// Counts by status / evidence / basis (for summaries and tests); over EVERY row also after [`check_streamed`].
    pub fn counts(&self) -> BTreeMap<String, u64> {
        if let Some(a) = &self.aggregates {
            return a.counts.clone();
        }
        let mut m: BTreeMap<String, u64> = BTreeMap::new();
        for r in &self.rows {
            count_row(&mut m, r, self.required);
        }
        m
    }

    /// The evidence level VISIBLE per rule class (the coordinator's ruling: per class, with counts): class -> {
    /// `served:<level>:<basis>`, `terminal`, `open:<status>`, `at_required` (served at >= the required level) } ->
    /// keys. The class of a served key is its server's (a rule's source class, a relation set's class); of an open key
    /// its blocking rule's (else `none`); terminals count under `terminal`.
    pub fn evidence_by_class(&self, set: &ImportedSet) -> BTreeMap<String, BTreeMap<String, u64>> {
        if let Some(a) = &self.aggregates {
            return a.by_class.clone();
        }
        let mut m: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
        for r in &self.rows {
            class_row(&mut m, set, r, self.required);
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
        // the import's own open rows (they include keys the core serves but the import does not certify, e.g. an
        // elided record lifted without its per-key proof)
        let open = c.get("status:open").copied().unwrap_or(0);
        if open > 0 {
            b.push(format!("import_open_keys:{open}"));
        }
        if self.recipe_binary_unpinned {
            b.push("recipe_binary_unpinned".into());
        }
        if self.witness_unchecked {
            b.push("witness_check_off_diagnostic".into());
        }
        if self.keys_only_diagnostic {
            b.push("keys_only_relations_diagnostic".into());
        }
        b
    }
}

fn count_row(m: &mut BTreeMap<String, u64>, r: &KeyRow, required: Level) {
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
    // lane closeloop3: fact basis of the served keys (Tier C only; absent otherwise)
    if let Some(b) = r.fact_basis
        && r.status == RowStatus::Served
    {
        *m.entry(format!("facts:basis:{b}")).or_default() += 1;
    }
    if r.status == RowStatus::Served && r.evidence.is_some_and(|e| e >= required) {
        *m.entry("certified".into()).or_default() += 1;
    }
}

fn class_row(
    m: &mut BTreeMap<String, BTreeMap<String, u64>>,
    set: &ImportedSet,
    r: &KeyRow,
    required: Level,
) {
    let class = match (&r.server, r.open_rule) {
        (Some(Server::Rule { table, record }), _) | (Some(Server::Credit { table, record }), _) => {
            set.rules[*table][*record].class.clone()
        }
        (Some(Server::Relation { set: name, .. }), _) => set
            .classes
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.clone()),
        (None, Some((t, k))) => set.rules[t][k].class.clone(),
        (None, None) if r.status == RowStatus::Terminal => "terminal".into(),
        (None, None) => "none".into(),
    };
    let e = m.entry(class).or_default();
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
    if r.status == RowStatus::Served && r.evidence.is_some_and(|x| x >= required) {
        *e.entry("at_required".into()).or_default() += 1;
    }
}
